//! AC-068 (NFR-008) — how long a restart takes to get its state back, measured and **recorded**.
//!
//! 34 AC-068 逐語: 「Given: 合成journal（10,000エントリ、一部が`Committing`未解決状態）。When: engine
//! 起動〜全Transformation状態復元完了までの時間を測定する。Then: RTO ≤ 5秒。」 33 NFR-008 says the
//! same with the population named: 「in-flight Transformation 10,000件までのjournal replay」.
//!
//! # What 「全Transformation状態復元完了」 is in v0.1, said before the number
//!
//! **E-M5-2** (req/38 §37, ruling M5-02 (a)) defines the reconstructed state: 「replay は **Σ のみを
//! 再構成する read-only 操作**・AC-039 の「結果状態」=Σ(状態表+ledger root+escrow index)と読む。
//! adapter は呼ばない」. So the thing this bench times is:
//!
//! 1. [`Engine::open`] — the journal's bytes are replayed, a torn tail is truncated once, the blob
//!    store and the ledger are opened (43 §7-1); and
//! 2. [`gx_engine::reconstruct`] — the records become Σ: every transformation's state, the escrow
//!    index, the drafts and the ledger frontier.
//!
//! Both are reported, separately and together, because they fail differently: (1) is I/O and framing
//! and (2) is a fold over records, and an RTO that grew would grow in one of them.
//!
//! **What is not timed, and why.** `Engine::recover` — 43 §7-3's resume — is a *side-effecting*
//! procedure: it re-applies deltas through registered adapters, appends to the ledger and issues
//! receipts. Timing it against a synthetic journal would require synthetic blobs, a registered
//! adapter and a real substrate to move, and the number would then be dominated by the adapter rather
//! than by the replay 33 NFR-008 names. So the unresolved rows below are **classified** (Σ reports
//! them as `Committing`) and not resumed, and this limit is written here rather than left in a
//! reader's assumption. `tests/crash_recovery.rs` measures the resume for correctness; its cost at
//! 10,000 entries is not measured by anything yet, and that is the honest state.
//!
//! # The synthetic journal
//!
//! Ten records per committed transformation (T-1, T-2, T-3, T-4a, T-8, T-9, `ProvenanceDerived`,
//! T-10b, `ApplyStarted`, T-11) and nine for an unresolved one — the same prefix stopped after
//! `ApplyStarted`, which is 51 §8.1's **third** injection point and the hardest window: the adapter
//! was asked and nothing says whether it answered. **10% of the transformations are left there**, so
//! the 10,000 entries hold roughly 900 finished lifecycles and 100 open ones.
//!
//! Records are appended through [`gx_engine::EngineJournal::append`] — the same road the engine
//! writes by, including the `fsync` per record — so the file the benchmark reads is a real journal
//! and not a hand-rolled imitation of one. Building it is **not** in the measurement.
//!
//! # M3-15
//!
//! median + count + denominator; the 5 s of 33 NFR-008 is a provisional design budget printed beside
//! the figures and compared with nothing.

mod support;

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::Criterion;
use gx_engine::{
    reconstruct, Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle,
};
use support::{committed_records, gate, measuring, report, unresolved_records, Sandbox};

/// 34 AC-068: 「合成journal（10,000エントリ」.
const ENTRIES: usize = 10_000;

/// 「一部が`Committing`未解決状態」 — one transformation in ten.
const UNRESOLVED_IN: usize = 10;

/// How many times the restart is timed. Fewer than AC-065's thousand because each sample replays ten
/// thousand records; thirty puts the p99 on the top sample, which is said rather than implied.
const SAMPLES: usize = 30;

/// What the synthetic journal turned out to hold.
///
/// Counted from the records **actually appended**, not from the loop's intent: 10,000 is not a
/// multiple of ten, so the last lifecycle is cut short by the entry budget and lands in whatever
/// state its last record names. A benchmark that asserted the populations it *meant* to write would
/// have been reporting the plan; these are the world.
struct Journal {
    path: std::path::PathBuf,
    entries: usize,
    /// Transformations Σ can name — one per `Planned` record (43 T-2 is where the id exists).
    planned: usize,
    /// `Committed` records, i.e. lifecycles that finished.
    committed: usize,
    /// `CommittingStarted` without a `Committed`: 34's 「一部が`Committing`未解決状態」.
    unresolved: usize,
}

/// Build the journal, exactly [`ENTRIES`] records long.
fn synthesise(sandbox: &Sandbox) -> Journal {
    let path = sandbox.dir().join("journal.bin");
    let mut journal = EngineJournal::open(&path).expect("a fresh journal opens on the tmpfs");
    let mut out = Journal {
        path: path.clone(),
        entries: 0,
        planned: 0,
        committed: 0,
        unresolved: 0,
    };
    let mut committing_started = 0_usize;
    let mut seed = 1_u64;
    while out.entries < ENTRIES {
        let records = if (seed as usize).is_multiple_of(UNRESOLVED_IN) {
            unresolved_records(seed)
        } else {
            committed_records(seed)
        };
        for record in records {
            if out.entries == ENTRIES {
                break;
            }
            match &record {
                EngineJournalRecord::Planned { .. } => out.planned += 1,
                EngineJournalRecord::CommittingStarted { .. } => committing_started += 1,
                EngineJournalRecord::Committed { .. } => out.committed += 1,
                _ => {}
            }
            journal.append(record).expect("the tmpfs accepts a record");
            out.entries += 1;
        }
        seed += 1;
    }
    out.unresolved = committing_started - out.committed;
    out
}

fn recovery_distribution() {
    let sandbox = Sandbox::new("ac068");
    let built = synthesise(&sandbox);
    let path = &built.path;
    let bytes = std::fs::metadata(path).expect("the journal is there").len();
    println!(
        "AC068_JOURNAL entries={} planned={} committed={} unresolved_committing={} \
         bytes={bytes} fs={} (built outside the measurement)",
        built.entries,
        built.planned,
        built.committed,
        built.unresolved,
        support::filesystem_of(sandbox.dir())
    );

    // One measured restart, checked before it is timed a hundred times: a benchmark that replayed a
    // journal into an empty Σ would be fast and would be measuring nothing.
    let engine = Engine::open(path, gate(), InjectedEvidence::none()).expect("the journal reopens");
    let sigma = reconstruct(engine.journal().records());
    let in_committing = sigma
        .transformations()
        .iter()
        .filter(|row| row.state == Some(Lifecycle::Committing))
        .count();
    println!(
        "AC068_SIGMA drafts={} transformations={} escrow={} ledger={} committing={in_committing}",
        sigma.drafts().len(),
        sigma.transformations().len(),
        sigma.escrow().len(),
        sigma.ledger().len(),
    );
    assert_eq!(
        sigma.transformations().len(),
        built.planned,
        "Σ has to name every transformation the journal holds, or the RTO below is an RTO for less \
         than the population 33 NFR-008 states"
    );
    assert_eq!(
        in_committing, built.unresolved,
        "34's 「一部が`Committing`未解決状態」 is the population this benchmark exists for"
    );

    let mut opens: Vec<Duration> = Vec::with_capacity(SAMPLES);
    let mut folds: Vec<Duration> = Vec::with_capacity(SAMPLES);
    let mut totals: Vec<Duration> = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let engine =
            Engine::open(path, gate(), InjectedEvidence::none()).expect("the journal reopens");
        let opened = started.elapsed();
        let folding = Instant::now();
        let sigma = reconstruct(engine.journal().records());
        let folded = folding.elapsed();
        black_box(sigma.transformations().len());
        opens.push(opened);
        folds.push(folded);
        totals.push(started.elapsed());
    }
    report("AC068_OPEN", "Engine::open (replay)", &mut opens);
    report("AC068_FOLD", "reconstruct (Sigma)", &mut folds);
    report("AC068_RTO", "open + reconstruct", &mut totals);
    println!(
        "AC068_BUDGET 5s is 33's provisional NFR-008 value and is a design budget, not a pass mark \
         (M3-15). Nothing above is compared against it. `Engine::recover` (43 §7-3's resume) is NOT \
         in these figures -- see this file's header."
    );
}

fn bench_recovery(c: &mut Criterion) {
    let sandbox = Sandbox::new("ac068-criterion");
    let built = synthesise(&sandbox);
    let path = &built.path;
    let mut group = c.benchmark_group("journal_recovery");
    group.bench_function("open_and_reconstruct_10k", |b| {
        b.iter(|| {
            let engine =
                Engine::open(path, gate(), InjectedEvidence::none()).expect("the journal reopens");
            black_box(
                reconstruct(engine.journal().records())
                    .transformations()
                    .len(),
            )
        });
    });
    group.finish();
}

fn main() {
    if measuring() {
        recovery_distribution();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_recovery(&mut criterion);
    criterion.final_summary();
}
