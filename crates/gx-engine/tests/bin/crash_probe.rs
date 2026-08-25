// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 51 §8.1's real process: a commit that can be stopped dead at three points, and the restart that
//! recovers it.
//!
//! 51 §8.1 verbatim (sem: SEM-gx-engine-609): "send `kill -9` to a **real process** at each of
//! the following three injection points" ... "restart the process and confirm that 43 §7's
//! recovery procedure runs automatically". `gx-cli` is M6's
//! (req/78 N-01), so the "real binary" is this one — gated behind `probe-bin` exactly the way
//! `engine_id_probe` is, so `cargo build`, `cargo install` and `cargo package` never see it
//! (**M5-13, adopted (a)**).
//!
//! # The injection points are in the adapter, and the report says what that costs
//!
//! 51 §8.1 allows "test-only hooks or injection-point environment variables" (sem: SEM-gx-engine-610). This binary uses neither in the
//! engine: the seam is the **adapter**, because the alternative is an env-var read inside shipping
//! code, and a `commit` that consults the environment is a `commit` whose behaviour depends on
//! something no journal records. The four adapter calls inside the critical section are `snapshot`,
//! `precondition`, `invert` and `apply`, and they bracket the section as follows:
//!
//! | argument | blocks inside | journal tail when it blocks | world |
//! |---|---|---|---|
//! | `t9` | `snapshot` (T-10a's fresh read) | `CommittingStarted`, `ProvenanceDerived` | untouched |
//! | `escrow` | `apply`, before the write | …, `InverseEscrowed`, `ApplyStarted` | untouched |
//! | `applied` | `apply`, after the write and its fsync | …, `InverseEscrowed`, `ApplyStarted` | **changed** |
//!
//! 🔴 The middle row is 51 §8.1's second point "T-10b, right after `InverseEscrowed` (before apply)" (sem: SEM-gx-engine-611) **plus one
//! record**: with **E-M5-1** in force there is no adapter call between `InverseEscrowed` and
//! `ApplyStarted`, so a seam in the adapter cannot land between them. The fact the point exists to
//! measure — the inverse is escrowed and the world has not moved — is measured exactly; the tail is
//! one record longer than 51 §8.1 was written to expect. Raised as **M5H5-4**.
//!
//! # Stopping is a marker file and a sleep, and the parent sends the signal
//!
//! The child writes `<dir>/ready` (with an fsync) and then sleeps forever; the parent polls for that
//! file and calls `Child::kill`, which is `SIGKILL` on Unix. Two properties follow, and both matter
//! for "10 attempts at each injection point" (sem: SEM-gx-engine-612): the kill lands **after** everything the point promises has
//! reached the disk, and it lands there every time rather than in a race the test would have to
//! retry (51 §13-4).
//!
//! # Protocol
//!
//! ```text
//! crash_probe run     <dir> <t9|escrow|applied|none> <goal>
//! crash_probe recover <dir>
//! crash_probe mutate  <dir> <bytes>
//! ```
//!
//! `run` writes `<dir>/tid` before it opens the critical section, so a parent whose child never
//! returns can still name the transformation. Every mode prints `KEY=VALUE` lines; anything else on
//! stdout is a failure and the exit code says so.

#![forbid(unsafe_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gx_core::{
    Actor, ChangeContext, Cid, Fingerprint, GoalBytes, Intent, ObjectId, ObjectSnapshot, ReprKind,
    SubstrateKind, Timestamp,
};
use gx_engine::{Engine, InjectedEvidence};
use gx_substrate::{AppliedDelta, PlannedDelta, SubstrateAdapter};

/// Where the run's clock is (41 §6: injected, never read).
const RUN_AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
/// The restart's clock, a day later — which is what makes a re-issued receipt's `issued_at`
/// visibly different from the original's (**M5H4-7**).
const RECOVER_AT: Timestamp = Timestamp(1_754_086_400_000_000_000);

/// A policy set that admits everything, so the verdict is decided by the invariants and by
/// `invert_available` rather than by Cedar.
const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

/// Which injection point this run is armed for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Point {
    /// Inside the fresh snapshot of T-10a: the section is open and nothing else has happened.
    T9,
    /// At the top of `apply`: the inverse is escrowed, the application is announced, the world is
    /// as `plan` left it.
    Escrow,
    /// At the bottom of `apply`: the world has changed and been fsynced, and nothing has recorded
    /// it yet. **req/78 §3.2 Λ4's window.**
    Applied,
    /// 🔴 **AC-034 with the process boundary it asks for**: at T-10a's fresh read, a **separate
    /// process** rewrites the world, and then this one carries on into the CAS. Nothing is killed.
    Race,
    /// Not armed. The commit completes.
    None,
}

impl Point {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "t9" => Some(Point::T9),
            "escrow" => Some(Point::Escrow),
            "applied" => Some(Point::Applied),
            "race" => Some(Point::Race),
            "none" => Some(Point::None),
            _ => None,
        }
    }
}

/// A substrate that is a **file**, so that what it holds survives the death of this process.
///
/// 🔴 Still not `gx-adapter-fs` (**N-13**): this crate declares no adapter dependency, not even for
/// tests, and this is thirty lines against `SubstrateAdapter` rather than an import. It is also the
/// honest instrument — an E2E that failed because a real adapter's locator normalisation surprised
/// it would be measuring the adapter, and 51 §8.1 is about the engine's recovery.
struct FileAdapter {
    dir: PathBuf,
    world: PathBuf,
    ready: PathBuf,
    point: Point,
    armed: Arc<AtomicBool>,
    /// So that the one-shot injections fire once. `snapshot` is called once inside the critical
    /// section, but a fixture that relied on that would be relying on the shape of the code it is
    /// testing.
    fired: AtomicBool,
}

impl FileAdapter {
    /// Announce that this process has reached the injection point, and then stop.
    ///
    /// The marker is written and fsynced **after** everything the point promises is durable, so a
    /// parent that sees it knows the disk is in the state the point names. The sleep is unbounded:
    /// the parent's `SIGKILL` is what ends this process, and a timeout here would turn a missing
    /// signal into a passing test.
    fn stop_here(&self, what: &str) -> ! {
        let mut f = std::fs::File::create(&self.ready).expect("the ready marker can be written");
        f.write_all(what.as_bytes()).expect("the marker is written");
        f.sync_all().expect("the marker is durable");
        drop(f);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn armed_at(&self, point: Point) -> bool {
        self.point == point && self.armed.load(Ordering::SeqCst)
    }
}

impl SubstrateAdapter for FileAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<ObjectSnapshot> {
        if self.armed_at(Point::T9) {
            self.stop_here("t9");
        }
        // 34 AC-034 verbatim (sem: SEM-gx-engine-613): "the test harness injects a situation, via a **separate process**, writing to the target file **immediately before** the `gx commit <T.id>` call, that makes `Fingerprint₁≠Fingerprint₀`". Hand 4 could only
        // spawn a thread against an in-memory world; this is the sentence as written.
        if self.armed_at(Point::Race) && !self.fired.swap(true, Ordering::SeqCst) {
            let status = std::process::Command::new(
                std::env::current_exe().expect("this binary knows its own path"),
            )
            .args(["mutate", &self.dir.display().to_string(), "elsewhere"])
            .status()
            .expect("the second process starts");
            println!("INJECTED_BY_PROCESS={}", status.success());
        }
        let bytes = std::fs::read(locator).unwrap_or_default();
        Ok(ObjectSnapshot::new(
            ObjectId(digest_of(locator.as_bytes())),
            SubstrateKind::Fs,
            locator.to_string(),
            digest_of(&bytes),
            ReprKind::Bytes,
        ))
    }

    fn plan(&self, intent: &Intent, _pre: &ObjectSnapshot) -> gx_substrate::Result<PlannedDelta> {
        PlannedDelta::new(SubstrateKind::Fs, intent.goal().0.clone())
    }

    fn precondition(&self, snap: &ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        Ok(Fingerprint::new(
            SubstrateKind::Fs,
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<AppliedDelta> {
        if self.armed_at(Point::Escrow) {
            self.stop_here("escrow");
        }
        // Written and fsynced, because the point of `applied` is that the world moved **durably**
        // before the crash. A buffered write would make the injection measure the page cache.
        let mut f =
            std::fs::File::create(&self.world).map_err(|e| gx_substrate::Error::ApplyFailed {
                detail: format!("cannot open the world: {e}"),
            })?;
        f.write_all(delta.payload())
            .and_then(|()| f.sync_all())
            .map_err(|e| gx_substrate::Error::ApplyFailed {
                detail: format!("cannot write the world: {e}"),
            })?;
        drop(f);
        if self.armed_at(Point::Applied) {
            self.stop_here("applied");
        }
        let digest = digest_of(delta.payload());
        Ok(AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(SubstrateKind::Fs, self.world.display().to_string(), digest)?,
            digest,
            // E-M4-31: the adapter has no clock, so it answers zero and the engine overwrites.
            Timestamp(0),
        ))
    }

    fn invert(
        &self,
        _delta: &PlannedDelta,
        _pre: &ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        let world = std::fs::read(&self.world).unwrap_or_default();
        Ok(gx_substrate::InvertOutcome::inverted(
            PlannedDelta::new(SubstrateKind::Fs, world)?,
            Vec::new(),
        ))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

/// A distinguishable digest of some bytes, deterministic across processes.
fn digest_of(bytes: &[u8]) -> Cid {
    let mut raw = [0u8; 32];
    for (i, b) in bytes.iter().enumerate() {
        raw[i % 32] ^= *b;
    }
    Cid(raw)
}

/// The signing key, from a fixed seed so that two processes issue receipts under one identity.
fn key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-engine-1", &[7u8; 32])
}

fn intent(locator: &str, goal: &str) -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        locator.to_string(),
        GoalBytes(goal.as_bytes().to_vec()),
        ChangeContext::Policy,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

fn adapter(dir: &Path, point: Point) -> (Arc<FileAdapter>, Arc<AtomicBool>) {
    let armed = Arc::new(AtomicBool::new(false));
    (
        Arc::new(FileAdapter {
            dir: dir.to_path_buf(),
            world: dir.join("world"),
            ready: dir.join("ready"),
            point,
            armed: Arc::clone(&armed),
            fired: AtomicBool::new(false),
        }),
        armed,
    )
}

fn open(dir: &Path, point: Point) -> (Engine<InjectedEvidence>, Arc<AtomicBool>) {
    let gate = gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(PERMIT_ALL).expect("the fixture policy set parses"),
    );
    let mut engine = match Engine::open(dir.join("journal.bin"), gate, InjectedEvidence::none()) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("crash_probe: cannot open the engine: {e}");
            std::process::exit(3);
        }
    };
    let (adapter, armed) = adapter(dir, point);
    engine.register_adapter(adapter, "crash-probe-1");
    (engine, armed)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("crash_probe: usage: crash_probe <run|recover|mutate> <dir> [..]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[2]);
    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("crash_probe: cannot make {}", dir.display());
        std::process::exit(2);
    }

    match args[1].as_str() {
        "run" => run(&dir, &args),
        "recover" => recover(&dir),
        "mutate" => mutate(&dir, &args),
        other => {
            eprintln!("crash_probe: no mode named {other:?}");
            std::process::exit(2);
        }
    }
}

/// `submit` → `plan` → `verify` → `canonicalize` → `commit`, armed for one injection point.
fn run(dir: &Path, args: &[String]) {
    let point = match args.get(3).map(String::as_str).and_then(Point::parse) {
        Some(point) => point,
        None => {
            eprintln!("crash_probe: the injection point is t9|escrow|applied|none");
            std::process::exit(2);
        }
    };
    let goal = args.get(4).map_or("after", String::as_str);
    let world = dir.join("world");
    if !world.exists() {
        std::fs::write(&world, b"before").expect("the world can be created");
    }

    let (mut engine, armed) = open(dir, point);
    let intent = intent(&world.display().to_string(), goal);
    let intent_id = engine.submit(&intent, 42, RUN_AT).expect("T-1");
    let id = engine.plan(&intent, RUN_AT).expect("T-2");
    // Before the section opens, so that a parent whose child never returns can name it.
    std::fs::write(dir.join("tid"), gx_canon::cid::to_text(&id.0)).expect("the id is written");
    println!("INTENT_ID={}", gx_canon::cid::to_text(&intent_id.0));
    println!("TID={}", gx_canon::cid::to_text(&id.0));
    let state = engine.verify(&id, RUN_AT, &key(), None).expect("T-3/T-4a");
    println!("VERIFIED={state:?}");
    let state = engine.canonicalize(&id, RUN_AT, None).expect("T-8");
    println!("CANONICALIZED={state:?}");

    armed.store(true, Ordering::SeqCst);
    match engine.commit(&id, RUN_AT, &key()) {
        Ok(state) => println!("COMMITTED={state:?}"),
        Err(e) => {
            println!("COMMIT_REFUSED={e}");
            std::process::exit(4);
        }
    }
    report(&engine, dir);
}

/// Restart: open, register, and run 43 §7 immediately.
///
/// 43 §7 says "always run at startup" (sem: SEM-gx-engine-614) and this is that moment — with one thing between it and `open`,
/// which is the registration of the adapter and the signing key. 41 §6 injects both from outside,
/// so an `Engine::open` that recovered would have to do it before either arrived. Raised as
/// **M5H5-1**.
fn recover(dir: &Path) {
    let (mut engine, _armed) = open(dir, Point::None);
    println!("BEFORE_LEDGER_AGREES={}", engine.ledger_agrees());
    match engine.recover(RECOVER_AT, &key()) {
        Ok(rows) => {
            println!("RECOVERED_ROWS={}", rows.len());
            for row in &rows {
                println!(
                    "RECOVERED id={} path={} state={:?} seq={:?} appended={:?} payload_matched={:?} receipt={}",
                    gx_canon::cid::to_text(&row.transformation.0),
                    row.path.kind(),
                    row.state,
                    row.ledger_seq,
                    row.appended,
                    row.payload_matched,
                    row.receipt.is_some()
                );
                if let Some(receipt) = &row.receipt {
                    // `issued_at` is outside the signed core (E-M2-6) and outside the ledger
                    // digest, which is exactly why the re-issue of 43 §7-3b can differ in it and
                    // in nothing else (M5H4-7).
                    println!(
                        "REISSUED issued_at={:?} ledger_digest={}",
                        receipt.issued_at,
                        receipt
                            .ledger_digest()
                            .map(|c| gx_canon::cid::to_text(&c))
                            .unwrap_or_default()
                    );
                    // AC-018's road, walked against a receipt **no live commit issued**: the
                    // signature, the schema and the inclusion proof against this process's own
                    // view of the ledger. A recovery that produced an unverifiable receipt would
                    // have repaired the journal and broken P-7.
                    let anchor = gx_log::proof::unsigned_checkpoint(
                        engine.ledger().log(),
                        "glovrex-ledger/v1",
                        RECOVER_AT,
                    );
                    match anchor.map_err(|e| e.to_string()).and_then(|anchor| {
                        gx_witness::verify_offline(receipt, &key().verifying(), Some(&anchor))
                            .map_err(|e| e.to_string())
                    }) {
                        Ok(checks) => println!("REISSUED_CHECKS={checks:?}"),
                        Err(e) => println!("REISSUED_CHECKS_REFUSED={e}"),
                    }
                }
            }
        }
        Err(e) => {
            println!("RECOVER_REFUSED={e}");
            std::process::exit(5);
        }
    }
    report(&engine, dir);
}

/// 🔴 AC-034's "writing to the target file via a separate process" (sem: SEM-gx-engine-615), as a separate process.
///
/// Hand 4 could only spawn a thread: its substrate was an `Arc<Mutex<Vec<u8>>>`, which no second
/// process can reach. This binary's substrate is a file, so the injection is a real one — and what
/// AC-034 asks for is exactly this, a writer the engine shares nothing with.
fn mutate(dir: &Path, args: &[String]) {
    let bytes = args.get(3).map_or("elsewhere", String::as_str);
    let world = dir.join("world");
    let mut f = std::fs::File::create(&world).expect("the world can be opened");
    f.write_all(bytes.as_bytes()).expect("the world is written");
    f.sync_all().expect("the world is durable");
    println!("MUTATED={bytes}");
}

/// The three faces AC-043 checks: the journal, the ledger and the world.
fn report(engine: &Engine<InjectedEvidence>, dir: &Path) {
    println!("JOURNAL_RECORDS={}", engine.journal().len());
    println!(
        "JOURNAL_TAIL={:?}",
        engine
            .journal()
            .records()
            .iter()
            .map(gx_engine::EngineJournalRecord::kind)
            .collect::<Vec<_>>()
    );
    println!("LEDGER_LEAVES={}", engine.ledger().log().len());
    println!("LEDGER_AGREES={}", engine.ledger_agrees());
    println!(
        "WORLD={:?}",
        String::from_utf8_lossy(&std::fs::read(dir.join("world")).unwrap_or_default())
    );
}
