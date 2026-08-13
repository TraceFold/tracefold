//! Fixtures shared by M5 hand 7's three benches (AC-065, AC-066, AC-068).
//!
//! Not a test target and not a bench target: cargo compiles this file only as a module of a
//! `[[bench]]` that declares it, so it raises no `test result:` line and adds nothing to the floor
//! `tools/e2e.sh` counts.
//!
//! # Why this is not `tests/support/mod.rs`
//!
//! That module is the right one and it cannot be used: it calls `env!("CARGO_BIN_EXE_crash_probe")`,
//! and cargo defines that variable for an **integration test** of a package that declares a `[[bin]]`
//! — not for a bench. A bench that included it would not compile. So the pieces the benches need are
//! written again here, small, and the duplication is named rather than hidden.
//!
//! # 🔴 tmpfs, proved rather than spelled
//!
//! 34 AC-065 逐語: 「fs-adapter経由・**tmpfs上**のcommit pipeline」, and 33 NFR-002 says the same
//! (「`gx-adapter-fs`をtmpfs上で使用し」). The repository itself lives on `/mnt/c`, which WSL mounts
//! as **9p/DrvFs over NTFS** — a filesystem whose rename is not the POSIX one this pipeline's `apply`
//! is built on (req/38 §32, `gx-adapter-fs/tests/support`). A number measured there would be a number
//! about DrvFs printed under AC-065's name, so [`tmpfs_root`] reads `/proc/self/mountinfo` and
//! **refuses** anything that is not a tmpfs. Fail-closed: a machine that cannot produce the evidence
//! says so instead of producing a different number.
//!
//! What a tmpfs does **not** give is durability — `fsync` there is close to free — so every figure
//! these benches print is a figure about CPU and allocation, not about a disk. That is stated here
//! rather than left for a reader (req/29 §4), and it is also what 34's Given asks for.

// Each bench binary compiles the whole file and none of them uses all of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gx_core::{
    Actor, ChangeContext, Cid, GoalBytes, Intent, IntentId, ObjectId, SubstrateKind, Timestamp,
    TransformationId, VerdictKind,
};
use gx_engine::store::FingerprintRecord;
use gx_engine::EngineJournalRecord;
use gx_witness::{Environment, KeyPair, Provenance};

/// The environment variable that moves the sandbox, for a machine whose tmpfs is elsewhere.
pub const ROOT_ENV: &str = "GLOVREX_BENCH_ROOT";

/// Where a sandbox is made unless [`ROOT_ENV`] says otherwise.
pub const DEFAULT_ROOT: &str = "/dev/shm";

/// A fixed instant, so that nothing here reads a clock the engine is supposed to be given (41 §6).
pub const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The filesystem type mounted over `path`, read from `/proc/self/mountinfo`.
///
/// The line format is `id parent major:minor root mountpoint options... - fstype source superopts`,
/// so the type is the field after the lone `-`, and the mount point that answers is the **longest**
/// prefix of `path` — which is what makes `/dev/shm` win over `/`. The same reader
/// `gx-adapter-fs/tests/support/mod.rs` carries; a second copy rather than a shared crate, because
/// making one would put a package in the graph so that two fixtures could agree.
#[must_use]
pub fn filesystem_of(path: &Path) -> String {
    let target = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .expect("/proc/self/mountinfo is readable on Linux");

    let mut best: Option<(usize, String)> = None;
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(dash) = fields.iter().position(|f| *f == "-") else {
            continue;
        };
        let (Some(point), Some(fstype)) = (fields.get(4), fields.get(dash + 1)) else {
            continue;
        };
        let covers = target == *point
            || (target.starts_with(point)
                && (point.ends_with('/') || target[point.len()..].starts_with('/')));
        if covers && best.as_ref().is_none_or(|(len, _)| point.len() > *len) {
            best = Some((point.len(), (*fstype).to_string()));
        }
    }
    best.map_or_else(|| "unknown".to_string(), |(_, fstype)| fstype)
}

/// The directory sandboxes are made in, refusing anything that is not a tmpfs.
///
/// # Panics
/// When the root is missing or is not a tmpfs — see the module note on why that is a refusal rather
/// than a fallback.
#[must_use]
pub fn tmpfs_root() -> PathBuf {
    let root = PathBuf::from(std::env::var(ROOT_ENV).unwrap_or_else(|_| DEFAULT_ROOT.to_string()));
    assert!(
        root.is_dir(),
        "{} is not a directory; set {ROOT_ENV} to a tmpfs",
        root.display()
    );
    let fstype = filesystem_of(&root);
    assert_eq!(
        fstype,
        "tmpfs",
        "{} is a {fstype}. 34 AC-065's Given is 「tmpfs上のcommit pipeline」 and this repository \
         lives on 9p over NTFS; set {ROOT_ENV}.",
        root.display()
    );
    root
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A directory of one bench's own, on a tmpfs, removed when it is dropped.
pub struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    /// Make one.
    ///
    /// # Panics
    /// If the tmpfs cannot be written to, which is a broken machine rather than a slow one.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let dir = tmpfs_root().join(format!(
            "glovrex-bench-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a tmpfs accepts a directory");
        Self { dir }
    }

    /// The sandbox root.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The absolute locator of a name inside the sandbox.
    #[must_use]
    pub fn locator(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    /// Write a file, creating parents.
    pub fn write(&self, name: &str, content: &[u8]) {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a tmpfs accepts a directory");
        }
        std::fs::write(&path, content).expect("a tmpfs accepts a file");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A policy set that admits everything, so that the number measured is the pipeline's and not
/// Cedar's answer to an interesting question.
pub const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

/// A gate that decides with [`PERMIT_ALL`] and holds no invariants.
///
/// **D-9** ships no ready-made invariant (req/38 §24), so an empty registry is what a deployment
/// starts with. AC-064's bench measures the cost of a populated one in gx-gate; here the subject is
/// the pipeline around the gate, and the gate's own cost is already a recorded number.
#[must_use]
pub fn gate() -> gx_gate::Gate {
    gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(PERMIT_ALL).expect("the fixture policy set parses"),
    )
}

/// The engine's signing key. Seeded, not generated: 41 §6 keeps entropy at the boundary and a bench
/// that generated one per run would be measuring `getrandom`.
#[must_use]
pub fn signing_key() -> KeyPair {
    KeyPair::from_seed("key-engine-1", &[7u8; 32])
}

/// An intent `gx-adapter-fs` can plan: replace the whole file at `locator` with `goal`.
#[must_use]
pub fn intent_for(locator: &str, goal: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        locator.to_string(),
        GoalBytes(goal.to_vec()),
        ChangeContext::Policy,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

// ---------------------------------------------------------------------------
// The distribution, printed the way M3-15 fixes
// ---------------------------------------------------------------------------

/// A distinguishable digest for the synthetic journal. Not a hash of anything.
#[must_use]
pub fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// Print a sorted sample as `min / p50 / p90 / p99 / max` with its denominator.
///
/// **M3-15** (req/38 §19) is the form and the whole gate: 「p99 を median+回数+分母つきで測って記録
/// した」, and 「閾値比較は記録のみ」. Nothing here compares anything with 33's provisional value, and
/// the caller prints that value as a budget rather than as a pass mark.
///
/// Nearest-rank, named because a percentile is a choice of rule and two rules disagree by one sample.
pub fn report(tag: &str, name: &str, samples: &mut [Duration]) {
    assert!(!samples.is_empty(), "a distribution needs samples");
    samples.sort_unstable();
    let n = samples.len();
    let at = |q: f64| -> Duration {
        let rank = ((q * n as f64).ceil() as usize).clamp(1, n);
        samples[rank - 1]
    };
    println!(
        "{tag} {name:<26} n={n} min={:>10.3?} p50={:>10.3?} p90={:>10.3?} p99={:>10.3?} max={:>10.3?}",
        samples[0],
        at(0.50),
        at(0.90),
        at(0.99),
        samples[n - 1],
    );
}

/// Whether this process was started by `cargo bench` rather than by `cargo test`.
///
/// `--bench` is the flag `cargo bench` passes and `cargo test` does not, and criterion reads the
/// same one to choose between measuring and its one-iteration test mode. AC-064's bench records why
/// this matters in numbers: a raw instrument that ran under `cargo test` measured an **unoptimised**
/// build and reported a p50 twenty-four times the real one. A test run checks that a bench still
/// builds and still reaches the arms it claims; a measurement is `cargo bench`.
#[must_use]
pub fn measuring() -> bool {
    std::env::args().any(|a| a == "--bench")
}

// ---------------------------------------------------------------------------
// AC-068's synthetic journal
// ---------------------------------------------------------------------------

/// A provenance record of the shape the engine derives (42 §3.9).
#[must_use]
pub fn provenance(seed: u64, transformation: TransformationId) -> Provenance {
    Provenance {
        environment: Environment {
            host_id: None,
            adapter_kind: SubstrateKind::Fs,
            correlation_id: None,
            engine_version: "0.1.0".to_string(),
            adapter_version: "bench".to_string(),
        },
        input_objects: vec![ObjectId(cid(seed))],
        intent_digest: Some(cid(seed + 1)),
        transformation,
    }
}

/// The records one committed transformation writes, in the order 43 §3 writes them.
///
/// Ten records: T-1, T-2, T-3, T-4a, T-8, T-9, the provenance of **E-M5-10**, T-10b, the
/// `ApplyStarted` of **E-M5-1**, and T-11. This is the happy path 33 NFR-002 names
/// (「T-1→T-2→T-3→T-4a→T-8→T-9→T-10b→T-11」) with the two errata records that sit inside it.
#[must_use]
pub fn committed_records(seed: u64) -> Vec<EngineJournalRecord> {
    let t = TransformationId(cid(seed));
    let intent_id = IntentId(cid(seed + 1_000_000));
    vec![
        EngineJournalRecord::DraftCreated {
            intent_id,
            rng_seed: seed,
            at: AT,
        },
        EngineJournalRecord::Planned {
            transformation: t,
            intent_id,
            // **E-M5-13**: the two fields §47 M6-14 採(a) added. A bench fixture carries them for
            // the same reason it carries the rest -- the record it writes has to be the record the
            // engine writes, or the recovery it measures is a recovery of something else.
            locator: format!("/dev/shm/bench/{seed}"),
            delta_cid: cid(seed + 2_000_000),
            fp0: FingerprintRecord::of(
                &gx_core::Fingerprint::new(
                    SubstrateKind::Fs,
                    format!("/dev/shm/bench/{seed}"),
                    cid(seed + 3_000_000),
                )
                .expect("the scope is inside MAX_SCOPE_BYTES"),
            ),
            parents: Vec::new(),
            at: AT,
        },
        EngineJournalRecord::VerifyStarted {
            transformation: t,
            at: AT,
        },
        EngineJournalRecord::Verdict {
            transformation: t,
            kind: VerdictKind::Admit,
            verdict_digest: Some(cid(seed + 4_000_000)),
            fail_posture_engaged: false,
            at: AT,
        },
        EngineJournalRecord::Canonicalized {
            transformation: t,
            canonical_cid: cid(seed + 5_000_000),
            enforced: Some(true),
            at: AT,
        },
        EngineJournalRecord::CommittingStarted {
            transformation: t,
            at: AT,
        },
        EngineJournalRecord::ProvenanceDerived {
            transformation: t,
            provenance: provenance(seed, t),
            at: AT,
        },
        EngineJournalRecord::InverseEscrowed {
            transformation: t,
            inverse_cid: Some(cid(seed + 6_000_000)),
            at: AT,
        },
        EngineJournalRecord::ApplyStarted {
            transformation: t,
            delta_cid: cid(seed + 2_000_000),
            at: AT,
        },
        EngineJournalRecord::Committed {
            transformation: t,
            ledger_seq: seed,
            at: AT,
        },
    ]
}

/// The records of a transformation left **unresolved inside `Committing`** — AC-068's 「一部が
/// `Committing`未解決状態」.
///
/// Nine records: the same prefix, stopped after `ApplyStarted`. That is the hardest of 51 §8.1's three
/// injection points — the one where the adapter was asked and nothing said whether it answered — so a
/// recovery benchmark whose unresolved rows stopped earlier would be measuring the easy window.
#[must_use]
pub fn unresolved_records(seed: u64) -> Vec<EngineJournalRecord> {
    let mut records = committed_records(seed);
    records.truncate(9);
    records
}
