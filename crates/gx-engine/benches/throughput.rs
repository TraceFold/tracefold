//! AC-066 (NFR-003) — sustained commits per second over sixty seconds, measured and **recorded**.
//!
//! 34 AC-066 逐語: 「Given: `gx-cli`バッチsubmitまたは専用load generator。When: 60秒間の持続負荷試験を
//! 実行する。Then: 単一ノード持続スループット ≥ 100 commits/s（p50/p99レイテンシとエラー率を併記）。」
//! 33 NFR-003 adds why the number exists: 「redteam §3 adoption-physics attack（verify-before-commit
//! taxがボトルネックになるという指摘）への定量反証材料」.
//!
//! # 「`gx-cli`バッチsubmit**または**専用load generator」 — the second one, because the first is M6
//!
//! `gx-cli` does not exist: req/78 N-01 keeps the CLI out of M5 (「CLI/HTTP を結線しない」) and 51 §15
//! puts it in M6. The criterion's own 「または」 gives the alternative, and this file is it — a load
//! generator in-process, driving the same eight transitions AC-065 drives, against the same fs
//! adapter on the same tmpfs.
//!
//! What that costs in fidelity is worth naming: a CLI batch would pay for process start, argument
//! parsing and a fresh engine per call, and this does not. So the figure below is an **upper bound on
//! what the engine can sustain**, not a measurement of what `gx commit` will sustain. When M6 wires
//! the CLI, the same sixty seconds run again through it and the two numbers can be compared —
//! that difference is the CLI's cost, and it is not measurable today.
//!
//! # What is recorded, and what is not compared
//!
//! **M3-15** again: median + count + denominator, and no comparison with 100 commits/s. 33 marks that
//! value 暫定 (ASM), so it is printed as a budget beside the measurement and the gate is 「measured and
//! recorded」. `tools/ci.sh`'s bench stage fails on nothing here.
//!
//! Three things are printed rather than one, because 34 asks for three: throughput, the latency
//! distribution (p50/p99, nearest-rank, with `n`) and the **error rate**. A load test that reported
//! only a rate could be reporting a fast failure loop.
//!
//! # 🔴 Per-bucket, because 「持続」 is the claim
//!
//! A sixty-second average hides a slope. The engine keeps every transformation it has seen in one
//! table and 43 §8's conflict check walks it (`conflicting_predecessor`), so the cost of a commit at
//! second 60 is not the cost at second 1 — and an average would report the mean of a line as if it
//! were a level. So the run is bucketed by ten seconds and each bucket prints its own rate and its
//! own p50/p99. 「持続スループット」 is a statement about the last bucket at least as much as about the
//! mean.
//!
//! The duration is `GLOVREX_BENCH_SECONDS` if set, and 60 otherwise — set for a smoke run, never for
//! the recorded one, and the value used is printed on the same line as the numbers.

mod support;

use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use criterion::Criterion;
use gx_adapter_fs::FsAdapter;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent_for, measuring, report, signing_key, Sandbox, AT};

/// 34 AC-066: 「60秒間の持続負荷試験」.
const DEFAULT_SECONDS: u64 = 60;

/// The bucket width. Six buckets over the default run, which is enough for a slope to be a shape
/// rather than two points.
const BUCKET: Duration = Duration::from_secs(10);

/// The five entry points one lifecycle walks, named so that a bucket can be read per stage.
///
/// 🔴 The reason this is not one number: the first measured run showed the rate falling from 916 to
/// 205 commits/s across the same sixty seconds, and a total tells you **that** it fell rather than
/// **where**. With five stages the same run answers both, and the answer is a diagnosis instead of a
/// symptom (req/85 §2.2, and the ticket it raises).
const STAGES: [&str; 5] = ["submit", "plan", "verify", "canonicalize", "commit"];

/// What one commit attempt answers: how long each stage took, and whether it reached `Committed`.
struct Attempt {
    took: Duration,
    stages: [Duration; STAGES.len()],
    committed: bool,
}

/// 🔴 **M5H7-3 採(b)** — how many directories the load generator spreads its subjects over.
///
/// `0` (the default) is hand 7's generator exactly: every subject file, for the whole sixty
/// seconds, in **one** directory. Hand 7 measured the rate falling 899.60 → 225.20 commits/s and
/// listed three candidate causes, of which the third was 「load generator が単一 dir に 24,000 file
/// を作る(=**engine ではなく計器**の性質)」. §44 ruled the cheap half first: 「まず load generator を
/// dir shard に割った**対照実験**(候補③=計器の性質かどうかが 1 走で決まる・安い)」.
///
/// So the arm is a **fixture** knob and nothing else. `crates/gx-engine/src` is untouched by this
/// experiment, and the two arms come out of one binary so that the comparison is not also a
/// comparison of two builds. Set `GLOVREX_BENCH_SHARDS=256` for the sharded arm.
fn shards() -> usize {
    std::env::var("GLOVREX_BENCH_SHARDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Where subject `n` lives, given the shard count.
///
/// Knuth's multiplicative constant rather than the low bits of `n`: consecutive `n` land in
/// consecutive directories under a modulo, which would keep each directory's *growth* sequential
/// and could reproduce the single-directory behaviour one level down. What the experiment needs is
/// for the 24,000 files to be spread, and a multiplicative hash spreads them.
fn subject(n: usize, shards: usize) -> String {
    if shards == 0 {
        return format!("subject-{n}");
    }
    let h = (n.wrapping_mul(2_654_435_761)) % shards;
    format!("sh{h:04}/subject-{n}")
}

fn one(
    engine: &mut Engine<InjectedEvidence>,
    sandbox: &Sandbox,
    n: usize,
    shards: usize,
) -> Attempt {
    let name = subject(n, shards);
    sandbox.write(&name, b"before");
    let locator = sandbox.locator(&name);
    let intent = intent_for(&locator, b"after");

    let mut stages = [Duration::ZERO; STAGES.len()];
    let started = Instant::now();
    // Every refusal counts as an error rather than stopping the run: 34 asks for an error **rate**,
    // and a generator that panicked on the first one would report a rate of zero for a broken
    // deployment (req/29 §4 -- a skip and a pass must not look alike).
    let committed = (|| -> bool {
        let mut mark = Instant::now();
        let mut lap = |stages: &mut [Duration; STAGES.len()], i: usize| {
            stages[i] = mark.elapsed();
            mark = Instant::now();
        };
        if engine.submit(&intent, n as u64, AT).is_err() {
            return false;
        }
        lap(&mut stages, 0);
        let Ok(id) = engine.plan(&intent, AT) else {
            return false;
        };
        lap(&mut stages, 1);
        if engine.verify(&id, AT, &signing_key(), None).is_err() {
            return false;
        }
        lap(&mut stages, 2);
        if engine.canonicalize(&id, AT, None).is_err() {
            return false;
        }
        lap(&mut stages, 3);
        let ok = matches!(
            engine.commit(&id, AT, &signing_key()),
            Ok(Lifecycle::Committed)
        );
        lap(&mut stages, 4);
        ok
    })();
    Attempt {
        took: started.elapsed(),
        stages,
        committed,
    }
}

fn sustained_load() {
    let seconds: u64 = std::env::var("GLOVREX_BENCH_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SECONDS);
    let sandbox = Sandbox::new("ac066");
    let shards = shards();
    println!(
        "AC066_LOAD_GENERATOR seconds={seconds} bucket={BUCKET:?} shards={shards} fs={} \
         (in-process generator; `gx-cli` is M6 -- 34's 「または専用load generator」; shards=0 is \
         hand 7's one-directory generator, which is M5H7-3 (b)'s control arm)",
        support::filesystem_of(sandbox.dir())
    );
    let mut engine = engine(&sandbox);

    let started = Instant::now();
    let budget = Duration::from_secs(seconds);
    let mut attempts: Vec<Attempt> = Vec::new();
    let mut bucket_of: Vec<usize> = Vec::new();
    let mut n = 0_usize;
    while started.elapsed() < budget {
        let bucket = (started.elapsed().as_secs_f64() / BUCKET.as_secs_f64()) as usize;
        let attempt = one(&mut engine, &sandbox, n, shards);
        black_box(attempt.committed);
        attempts.push(attempt);
        bucket_of.push(bucket);
        n += 1;
    }
    let elapsed = started.elapsed();

    let errors = attempts.iter().filter(|a| !a.committed).count();
    let mut all: Vec<Duration> = attempts.iter().map(|a| a.took).collect();
    println!(
        "AC066_TOTAL attempts={} committed={} errors={errors} error_rate={:.6} \
         elapsed={:.3?} throughput={:.2} commits/s",
        attempts.len(),
        attempts.len() - errors,
        errors as f64 / attempts.len() as f64,
        elapsed,
        (attempts.len() - errors) as f64 / elapsed.as_secs_f64(),
    );
    report("AC066_LATENCY", "whole-run", &mut all);

    let buckets = bucket_of.iter().copied().max().unwrap_or(0) + 1;
    for b in 0..buckets {
        let mut took: Vec<Duration> = attempts
            .iter()
            .zip(&bucket_of)
            .filter(|(_, bucket)| **bucket == b)
            .map(|(a, _)| a.took)
            .collect();
        let failed = attempts
            .iter()
            .zip(&bucket_of)
            .filter(|(a, bucket)| **bucket == b && !a.committed)
            .count();
        if took.is_empty() {
            continue;
        }
        let width = BUCKET
            .as_secs_f64()
            .min(elapsed.as_secs_f64() - b as f64 * BUCKET.as_secs_f64());
        println!(
            "AC066_BUCKET_{b} commits={} errors={failed} throughput={:.2} commits/s",
            took.len(),
            (took.len() - failed) as f64 / width,
        );
        report(
            "AC066_BUCKET",
            &format!("seconds {}..{}", b * 10, b * 10 + 10),
            &mut took,
        );
        // Where the time went, per stage, in this bucket. The first and last bucket read together
        // are the whole diagnosis: a stage whose p50 is flat is not the one that made the rate fall.
        for (i, stage) in STAGES.iter().enumerate() {
            let mut per: Vec<Duration> = attempts
                .iter()
                .zip(&bucket_of)
                .filter(|(_, bucket)| **bucket == b)
                .map(|(a, _)| a.stages[i])
                .collect();
            report("AC066_STAGE", &format!("{b}:{stage}"), &mut per);
        }
    }

    println!(
        "AC066_FINAL_TABLE shards={shards} transformations={n} journal_records={} ledger_leaves={}",
        engine.journal().len(),
        engine.ledger().log().len()
    );
    println!(
        "AC066_BUDGET 100 commits/s is 33's provisional NFR-003 value and is a design budget, not a \
         pass mark (M3-15). Nothing above is compared against it."
    );
}

/// An engine over a tmpfs sandbox with the fs adapter registered (AC-065's, same shape).
fn engine(sandbox: &Sandbox) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        sandbox.dir().join("journal.bin"),
        gate(),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens on the tmpfs");
    engine.register_adapter(Arc::new(FsAdapter::new()), "gx-adapter-fs 0.1.0");
    engine
}

/// criterion's view of the same work, for 51 §9's regression detection.
///
/// The sixty-second run answers 「持続スループット」 and is not comparable between machines or between
/// days; a criterion estimate of one commit is, which is why both exist. It is the same routine
/// AC-065 measures — the difference between the two files is the question, not the code path.
fn bench_one(c: &mut Criterion) {
    let sandbox = Sandbox::new("ac066-criterion");
    let shards = shards();
    let mut engine = engine(&sandbox);
    let mut n = 0_usize;
    let mut group = c.benchmark_group("throughput");
    group.bench_function("one_commit", |b| {
        b.iter(|| {
            n += 1;
            black_box(one(&mut engine, &sandbox, n, shards).committed)
        });
    });
    group.finish();
}

fn main() {
    if measuring() {
        sustained_load();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_one(&mut criterion);
    criterion.final_summary();
}
