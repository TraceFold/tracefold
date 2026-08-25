// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-18 adopted (a) = AC-M6-b** — what a restart costs when it has work to finish, measured and (sem: SEM-gx-engine-048)
//! **recorded**. 51 §9's bench table gains its sixth row here.
//!
//! > **§44 M5H7-4 adopted (a) = E-M5-16**, verbatim: "the measured cost of `Engine::recover`
//! > (43 §7-3's resume) is **recorded as unmeasured** -- the implementation window is M6" (quoted in
//! > SEM-gx-engine-049)
//! > **§47**'s new AC: "**AC-M6-b** (recover cost = median + count + denominator)"
//!
//! # Why this is not `journal_recovery.rs` with one more number
//!
//! AC-068 (`benches/journal_recovery.rs`) times `Engine::open` + `reconstruct`: the **read** side,
//! "startup through every Transformation's state fully restored" (quoted in SEM-gx-engine-050), which
//! **E-M5-2** defines as Σ and nothing else. That file
//! says in its own header why it stops there:
//!
//! > `Engine::recover` — 43 §7-3's resume — is a *side-effecting* procedure: it re-applies deltas
//! > through registered adapters, appends to the ledger and issues receipts… its cost at 10,000
//! > entries is not measured by anything yet, and that is the honest state.
//!
//! This file is that sentence being paid off. The recovery-flow ledger, §1.2's reason for caring is
//! not tidiness: "the time to resume after a crash is the flight recorder's product promise itself"
//! (quoted in SEM-gx-engine-051) — DR-1(a)'s wedge is a recorder that can
//! be trusted to come back, and the number that claim rests on had never been taken.
//!
//! # 🔴 The journal is **real**, and that is what cost this file its complexity
//!
//! AC-068's ten thousand records are synthesised: `support::committed_records` writes plausible
//! records with made-up CIDs, which is exactly right for timing a fold over records and exactly
//! wrong here. A `resume` reads the **blob store** for the planned delta, hands it to a registered
//! adapter, applies it to a real substrate, rebuilds a receipt and appends to the ledger; against
//! synthetic CIDs every one of those steps refuses in microseconds and the number produced would be
//! the cost of failing, printed under the name of the cost of recovering.
//!
//! So the fixture drives the engine for real — `submit → plan → verify → canonicalize → commit` on a
//! tmpfs through `gx-adapter-fs` — and the unresolved rows are made by **stopping between T-8 and
//! T-11** and appending the two records the critical section would have written:
//!
//! | record | why it is the one appended |
//! |---|---|
//! | `CommittingStarted` (T-9) | opens the critical section; `reconstruct` reads it as `Committing` |
//! | `ApplyStarted` (**E-M5-1**) | "the adapter was asked and nothing says whether it answered" (sem: SEM-gx-engine-052) |
//!
//! That pair is 51 §8.1's **third** injection point and the hardest window — the one where the
//! recovery cannot know whether the world moved, so it takes 43 §7-3c's road and applies again. A
//! fixture that stopped earlier (no `ApplyStarted`, no ledger entry) would take the cheap road
//! instead: `resume` folds it to `Aborted(InternalError)` in a few instructions and the benchmark
//! would report the cost of giving up.
//!
//! **What is deliberately not injected**: the T-9-only row. Both roads exist and only one is
//! expensive, so the recorded number is the expensive one and the mix is printed beside it.
//!
//! # 🔴 One sample destroys its own fixture
//!
//! `recover` finishes the work. Run it twice on one directory and the second run has nothing to do,
//! so a naive loop would report a median of "no rows to resume" (sem: SEM-gx-engine-053). Each sample therefore gets a
//! **copy** of the prepared project, and building the template is outside the timed region — the
//! same discipline `journal_recovery.rs` applies to writing its synthetic journal.
//!
//! # M3-15 / M5H7-6
//!
//! median + count + denominator; 33 NFR-008's five seconds is AC-068's budget for the *replay* and
//! is printed here as context, compared with nothing. p99 is recorded and is not a gate.

mod support;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use criterion::Criterion;
use gx_adapter_fs::FsAdapter;
use gx_engine::{
    reconstruct, Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle,
};
use support::{gate, intent_for, measuring, report, signing_key, Sandbox, AT};

/// 34 AC-068's population, borrowed by AC-M6-b: "a synthetic journal (10,000 entries)" (quoted in SEM-gx-engine-054).
const ENTRIES: usize = 10_000;

/// 🔴 **§47's AC-M6-b**: "10,000 entries, **101** unresolved `Committing`" (quoted in SEM-gx-engine-055).
///
/// A hundred and one rather than a hundred, because the ruling says so, and because an odd number
/// makes an off-by-one in the fixture visible in the printed denominator instead of hiding inside a
/// round one.
const UNRESOLVED: usize = 101;

/// How many times the recovery is timed. Each sample copies a ten-thousand-record project first, so
/// the count is bounded by the copying rather than by the measuring.
const SAMPLES: usize = 20;

/// What the prepared project turned out to hold — counted from the journal that exists, never from
/// the loop's intent (`journal_recovery.rs`'s rule, and for its reason: 10,000 is not a multiple of
/// anything this fixture writes).
struct Prepared {
    dir: PathBuf,
    records: usize,
    committed: usize,
    committing: usize,
}

/// An engine over `dir` with the fs adapter registered.
fn engine_at(dir: &Path) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(dir.join("journal.bin"), gate(), InjectedEvidence::none())
        .expect("the journal opens");
    engine.register_adapter(Arc::new(FsAdapter::new()), "gx-adapter-fs 0.1.0");
    engine
}

/// A directory tree, copied. `std::fs::copy` per file: the tree is flat enough that a recursive
/// walk is three lines, and shelling out to `cp` would put a second program in the measurement's
/// dependency list.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a tmpfs accepts a directory");
    for entry in std::fs::read_dir(from).expect("the template is readable") {
        let entry = entry.expect("a directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a tmpfs accepts a copy");
        }
    }
}

/// Build the project AC-M6-b describes: ~10,000 real journal records, 101 of them unresolved inside
/// `Committing` with `ApplyStarted` written.
fn prepare(sandbox: &Sandbox) -> Prepared {
    let dir = sandbox.dir().join("template");
    std::fs::create_dir_all(&dir).expect("a tmpfs accepts a directory");

    // A committed lifecycle writes ten records (43 §3's order) and an unresolved one writes seven
    // (five here plus the two appended below), so the budget is spent backwards from the ruling's
    // 101 rather than forwards from a guess.
    let committed_target = (ENTRIES - UNRESOLVED * 7) / 10;
    let mut open_ids = Vec::with_capacity(UNRESOLVED);
    {
        let mut engine = engine_at(&dir);
        for n in 0..committed_target {
            let name = format!("done-{n}");
            let path = dir.join(&name);
            std::fs::write(&path, b"before").expect("a tmpfs accepts a file");
            let locator = path.to_string_lossy().into_owned();
            let intent = intent_for(&locator, format!("after-{n}").as_bytes());
            engine.submit(&intent, n as u64, AT).expect("submit");
            let id = engine.plan(&intent, AT).expect("plan");
            engine
                .verify(&id, AT, &signing_key(), None)
                .expect("verify");
            engine.canonicalize(&id, AT, None).expect("canonicalize");
            assert_eq!(
                engine.commit(&id, AT, &signing_key()).expect("commit"),
                Lifecycle::Committed
            );
        }
        for n in 0..UNRESOLVED {
            let name = format!("open-{n}");
            let path = dir.join(&name);
            std::fs::write(&path, b"before").expect("a tmpfs accepts a file");
            let locator = path.to_string_lossy().into_owned();
            let intent = intent_for(&locator, format!("resumed-{n}").as_bytes());
            engine
                .submit(&intent, (committed_target + n) as u64, AT)
                .expect("submit");
            let id = engine.plan(&intent, AT).expect("plan");
            engine
                .verify(&id, AT, &signing_key(), None)
                .expect("verify");
            engine.canonicalize(&id, AT, None).expect("canonicalize");
            open_ids.push(id);
        }
    }

    // 🔴 The two records the critical section would have written, appended **after** the engine has
    // let the file go. Writing them through `EngineJournal` rather than by hand is the same rule
    // `journal_recovery.rs` follows: the file a benchmark reads has to be a journal and not an
    // imitation of one, `fsync` per record included.
    {
        let mut journal =
            EngineJournal::open(dir.join("journal.bin")).expect("the journal reopens");
        let deltas: Vec<(gx_core::TransformationId, gx_core::Cid)> = journal
            .records()
            .iter()
            .filter_map(|r| match r {
                EngineJournalRecord::Planned {
                    transformation,
                    delta_cid,
                    ..
                } if open_ids.contains(transformation) => Some((*transformation, *delta_cid)),
                _ => None,
            })
            .collect();
        assert_eq!(
            deltas.len(),
            UNRESOLVED,
            "every unresolved row must have a Planned record to take its delta CID from"
        );
        for (transformation, delta_cid) in deltas {
            journal
                .append(EngineJournalRecord::CommittingStarted {
                    transformation,
                    at: AT,
                })
                .expect("T-9");
            journal
                .append(EngineJournalRecord::ApplyStarted {
                    transformation,
                    delta_cid,
                    at: AT,
                })
                .expect("E-M5-1");
        }
    }

    // Counted from the file, not from the plan above.
    let journal = EngineJournal::open(dir.join("journal.bin")).expect("the journal reopens");
    let sigma = reconstruct(journal.records());
    let committed = sigma
        .transformations()
        .iter()
        .filter(|row| row.state == Some(Lifecycle::Committed))
        .count();
    let committing = sigma
        .transformations()
        .iter()
        .filter(|row| row.state == Some(Lifecycle::Committing))
        .count();
    Prepared {
        dir,
        records: journal.len(),
        committed,
        committing,
    }
}

fn recovery_cost() {
    let sandbox = Sandbox::new("acm6b");
    println!(
        "RECOVER_FIXTURE building a real journal on fs={} (entries target={ENTRIES}, unresolved \
         target={UNRESOLVED}); the build is not timed",
        support::filesystem_of(sandbox.dir())
    );
    let built = Instant::now();
    let prepared = prepare(&sandbox);
    println!(
        "RECOVER_JOURNAL records={} committed={} committing={} build_took={:.3?}",
        prepared.records,
        prepared.committed,
        prepared.committing,
        built.elapsed()
    );
    assert_eq!(
        prepared.committing, UNRESOLVED,
        "AC-M6-b asks for {UNRESOLVED} unresolved rows"
    );

    let mut samples: Vec<Duration> = Vec::with_capacity(SAMPLES);
    let mut resumed_last = 0usize;
    for sample in 0..SAMPLES {
        let work = sandbox.dir().join(format!("run-{sample}"));
        copy_tree(&prepared.dir, &work);
        let mut engine = engine_at(&work);
        let started = Instant::now();
        let recovered = engine.recover(AT, &signing_key()).expect("recover");
        let took = started.elapsed();
        black_box(recovered.len());
        resumed_last = recovered.len();
        samples.push(took);
        // The copy is deleted rather than kept: twenty copies of a ten-thousand-record project is
        // the sort of tmpfs consumption that turns a benchmark into an out-of-memory report.
        drop(engine);
        let _ = std::fs::remove_dir_all(&work);
    }

    println!(
        "RECOVER_TOTAL samples={} rows_reported_per_run={resumed_last} of which unresolved={} \
         (the rest are 43 §7-2's terminal `Committed` rows, which the recovery walks and does not \
         resume)",
        samples.len(),
        prepared.committing
    );
    report("RECOVER_LATENCY", "Engine::recover", &mut samples);
    let per_row = samples[samples.len() / 2].as_secs_f64() / prepared.committing.max(1) as f64;
    println!(
        "RECOVER_PER_UNRESOLVED_ROW median_seconds={per_row:.6} (median of the whole call divided \
         by {} resumed rows -- an average, printed as one)",
        prepared.committing
    );
    println!(
        "RECOVER_BUDGET 33 NFR-008's RTO <= 5 s is AC-068's budget for the **replay** \
         (`Engine::open` + `reconstruct`, E-M5-2's Σ) and is printed here as context only. Nothing \
         above is compared against it: 34 gives `Engine::recover` no criterion at all, which is why \
         §47 raised AC-M6-b as a new one (M3-15: the gate is \"measured and recorded\", sem: SEM-gx-engine-056)."
    );
}

/// criterion's view, for 51 §9's regression detection. One sample per iteration is impossible here
/// (the fixture is consumed), so what criterion measures is the **replay** half on the same journal
/// — comparable between runs, and honest about being a different quantity from the one above.
fn bench_open(c: &mut Criterion) {
    let sandbox = Sandbox::new("acm6b-criterion");
    let prepared = prepare(&sandbox);
    let mut group = c.benchmark_group("recover_cost");
    group.bench_function("open_and_reconstruct", |b| {
        b.iter(|| {
            let journal =
                EngineJournal::open(prepared.dir.join("journal.bin")).expect("the journal reopens");
            black_box(reconstruct(journal.records()).transformations().len())
        });
    });
    group.finish();
}

fn main() {
    if measuring() {
        recovery_cost();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_open(&mut criterion);
    criterion.final_summary();
}
