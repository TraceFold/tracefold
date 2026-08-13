//! AC-065 (NFR-002) — how long one whole commit takes, measured and **recorded**.
//!
//! 34 AC-065 逐語: 「Given: fs-adapter経由・tmpfs上のcommit pipeline（submit→Committed, 43のT-1〜T-11
//! 全通過）。When: `gx-engine`統合ベンチを実行する。Then: end-to-end overhead p99 ≤ 250ms。」
//! 33 NFR-002 names the transition列 exactly: 「T-1→T-2→T-3→T-4a→T-8→T-9→T-10b→T-11」.
//!
//! # What the gate of this AC is, and what it is not
//!
//! **M3-15** (req/38 §19), which §37 extends to this hand's three benches: 「AC-064 は「p99 を
//! median+回数+分母つきで測って記録した」を gate とし、閾値比較は記録のみ(設計予算)。「50ms を
//! 満たした」と主張しない。bench-check の fail 化は Owner 閾値確定後」. The 250 ms here is 33's
//! provisional ASM value in the same sense, so **nothing in this file compares a measurement against
//! it** and nothing in `tools/ci.sh` fails on it.
//!
//! # Two instruments, because they answer two different questions
//!
//! **criterion** estimates the mean cost of one iteration from a sample of batch timings. That is the
//! instrument 34 names and the right one for regression detection (51 §9). It is **not** an
//! instrument for a percentile of per-commit latency: averaging inside a batch removes the tail a p99
//! is asked about. So a raw instrument runs first — one commit per sample, `Instant` around it,
//! sorted, printed with `n`. Its own bias is stated rather than subtracted: the `Instant` pair costs
//! tens of nanoseconds against a commit that costs hundreds of microseconds, and it is **included**
//! in every number below, because subtracting an estimate would make the reported figure derived.
//!
//! # 🔴 The fs adapter is here, and N-13 is intact
//!
//! req/78 N-13 forbids gx-engine from **shipping** an adapter — 「ここを破ると『どの substrate でも
//! 同じ engine』が実装で嘘になる」 — and `probes/doubt/tests/workspace_doubt.rs` checks the
//! `[dependencies]` section for exactly that. AC-065's Given is 「fs-adapter経由」, so this bench needs
//! a real one, and it is a **dev-dependency**: `cargo tree -p gx-engine -e normal` still shows zero
//! adapters, `ENGINE_SHIPPED_ADAPTERS=0` still holds, and no published artefact contains a line of
//! `gx-adapter-fs`. What did change is hand 1's stricter reading of its own instrument
//! (`ENGINE_ADAPTER_DECLARATIONS=0`, 「not even a dev-dependency」), which this hand moves to 1 and
//! reports rather than quietly re-scopes — see req/85 §5.2.
//!
//! Every byte is written to a **tmpfs**, proved from `/proc/self/mountinfo` (`support::tmpfs_root`),
//! because 34's Given says so and because this repository lives on 9p over NTFS.
//!
//! # What the number is not
//!
//! One node, warm cache, no external evidence collector, this machine's CPU, a policy set that
//! admits everything and an empty invariant registry. 34's Given fixes the first three; the rest is
//! why the figures are recorded with the toolchain and the host in req/85 rather than published as a
//! property of gx. And a tmpfs `fsync` is close to free, so **none of this is evidence about
//! durability** — the same limit req/52 §5 carries.

mod support;

use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use criterion::{BatchSize, Criterion};
use gx_adapter_fs::FsAdapter;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent_for, measuring, report, signing_key, Sandbox, AT};

/// One commit per sample. 1,000 puts the p99 ten samples from the top, which is the smallest count
/// at which the figure is not one observation.
const RAW_SAMPLES: usize = 1_000;

/// The warm cache 34's Given asks for. Cedar parses the pack once; what warms here is the allocator,
/// the page cache of the sandbox directory and the branch predictors.
const WARMUP: usize = 100;

/// An engine over a tmpfs sandbox with the fs adapter registered.
fn engine(sandbox: &Sandbox) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        sandbox.dir().join("journal.bin"),
        gate(),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens on the tmpfs");
    // **M5H4-4**: the registrant declares the adapter's build, because 41 §4's seven methods cannot
    // and N-07 forbids an eighth. A bench that passed 「unknown」 would put a made-up value into the
    // provenance record every commit below writes.
    engine.register_adapter(Arc::new(FsAdapter::new()), "gx-adapter-fs 0.1.0");
    engine
}

/// The whole of 33 NFR-002's transition列, once, with the end state checked.
///
/// Checked rather than assumed: a benchmark that measured a pipeline aborting at T-10a would be fast
/// and wrong, and 「it returned」 does not say which arm it returned from.
fn one_commit(engine: &mut Engine<InjectedEvidence>, sandbox: &Sandbox, n: usize) {
    let name = format!("subject-{n}");
    sandbox.write(&name, b"before");
    let locator = sandbox.locator(&name);
    let intent = intent_for(&locator, b"after");

    engine.submit(&intent, n as u64, AT).expect("T-1");
    let id = engine.plan(&intent, AT).expect("T-2");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("T-3, T-4a");
    engine.canonicalize(&id, AT, None).expect("T-8");
    let state = engine
        .commit(&id, AT, &signing_key())
        .expect("T-9, T-10a, T-10b, T-11");
    assert_eq!(
        state,
        Lifecycle::Committed,
        "the benchmark measured an aborted pipeline, not NFR-002's happy path"
    );
    black_box(state);
}

// ---------------------------------------------------------------------------
// Instrument 1: per-commit latency, for the percentile NFR-002 is stated in
// ---------------------------------------------------------------------------

fn latency_distribution() {
    let sandbox = Sandbox::new("ac065-raw");
    println!(
        "AC065_RAW_INSTRUMENT samples={RAW_SAMPLES} warmup={WARMUP} fs={} root={} \
         (one Instant pair per commit; overhead included, not subtracted)",
        support::filesystem_of(sandbox.dir()),
        sandbox.dir().display()
    );
    let mut engine = engine(&sandbox);
    for n in 0..WARMUP {
        one_commit(&mut engine, &sandbox, n);
    }
    let mut samples: Vec<Duration> = Vec::with_capacity(RAW_SAMPLES);
    for n in 0..RAW_SAMPLES {
        let started = Instant::now();
        one_commit(&mut engine, &sandbox, WARMUP + n);
        samples.push(started.elapsed());
    }
    report("AC065_RAW", "submit-to-committed", &mut samples);

    // The table's in-flight rows are the reason a later commit is not a fresh one: the engine holds
    // every transformation it has seen, and `conflicting_predecessor` walks that table (43 §8). The
    // walk skips on a subject mismatch, so the cost is a comparison per row rather than a
    // `commutation` call per row -- but it is a cost that grows, and a distribution taken over a
    // growing table is the honest place to say so. AC-066's per-bucket figures are where the growth
    // would show if it mattered at this scale.
    println!(
        "AC065_TABLE_AT_END transformations={} journal_records={} ledger_leaves={}",
        WARMUP + RAW_SAMPLES,
        engine.journal().len(),
        engine.ledger().log().len()
    );
    println!(
        "AC065_BUDGET 250ms is 33's provisional NFR-002 value and is a design budget, not a pass \
         mark (M3-15). Nothing above is compared against it."
    );
}

// ---------------------------------------------------------------------------
// Instrument 2: criterion, which is the tool 34 names
// ---------------------------------------------------------------------------

fn bench_commit(c: &mut Criterion) {
    let sandbox = Sandbox::new("ac065-criterion");
    let mut engine = engine(&sandbox);
    let mut n = 0_usize;
    let mut group = c.benchmark_group("commit_pipeline");
    group.bench_function("submit_to_committed", |b| {
        b.iter_batched(
            || {
                n += 1;
                n
            },
            |n| one_commit(&mut engine, &sandbox, n),
            // Setup writes a file and mints a name, and neither belongs in the measurement.
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

fn main() {
    if measuring() {
        latency_distribution();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_commit(&mut criterion);
    criterion.final_summary();
}
