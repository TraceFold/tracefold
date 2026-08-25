// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **ruling #20** (`req/38` §56, req/98 §7-2 hand 5 / §9 row d-1) — **a per-object lock is not (sem: SEM-gx-engine-029)
//! implemented**. Keeping D-7's firing condition as is, this only **measures commits/sec over
//! disjoint object groups**.
//!
//! > **20**: **keep D-7's firing condition** and do not implement a lock -- **add commits/sec
//! > measurement over disjoint object groups to hand 5** (sem: SEM-gx-engine-029)
//!
//! The firing condition (`req/38:930`): "when AC-066, taken through serve, falls below SLA" (quoted
//! in SEM-gx-engine-029). SLA = 33 NFR-003's 100 commits/s (a provisional value). ∴ the one question
//! this file answers is: **does a single `Mutex` fall below 100 commits/s even when N concurrent
//! agents touch mutually disjoint objects?**
//!
//! # Three arms, and what each is an upper bound on (sem: SEM-gx-engine-029)
//!
//! | arm | shape | what it answers (sem: SEM-gx-engine-029) |
//! |---|---|---|
//! | `single` | 1 thread, 1 engine | the serial baseline (the same shape as AC-066's in-process version) (sem: SEM-gx-engine-029) |
//! | `shared` | N threads, **one `Arc<Mutex<Engine>>`** | **the shape M6-06 adopted (a) shipped**. D-7's firing condition stands against this number (sem: SEM-gx-engine-029) |
//! | `disjoint` | N threads, **N engines** (each with its own journal, ledger and lock) | the **upper bound** on what a per-object lock could buy (sem: SEM-gx-engine-029) |
//!
//! 🔴 **`disjoint` is not an implementation of a per-object lock, it is its upper bound.** A real
//! per-object lock takes the shape "inside one engine, threads touching different objects do not
//! wait for each other" (quoted in SEM-gx-engine-029), with **the ledger and the journal staying
//! single** (the append-only log has one writer). This arm splits those two apart as well, so what a
//! per-object lock could actually buy sits **between** `shared` and `disjoint`. The upper bound is
//! measured because, if it lands close to `shared`, "the lock is not the problem" can be said
//! **without measuring the lower bound** -- the reverse cannot be said, and that is written below too.
//!
//! # Measuring the wait itself (sem: SEM-gx-engine-029)
//!
//! The `shared` arm records, per commit, **the time spent waiting to take the lock** and **the time (sem: SEM-gx-engine-029)
//! spent holding it**, separately. A per-object lock can only remove the former. ∴ "what percentage
//! of the total is waiting" is the one direct measurement readable without trusting the upper-bound
//! arm, and `LOCK_WAIT_SHARE` is it.
//!
//! # M3-15 / req/98 §6-8
//!
//! median + count + denominator. 100 commits/s is 33's **provisional** value, and this file does not
//! compare against it as a pass mark -- it prints it **as a firing-condition judgement**. That is
//! because ruling #20 explicitly asked for that judgement, in the shape of "whether it fell below is
//! answerable with a number" (E-M7-6, verbatim; quoted in SEM-gx-engine-030).
//!
//! # Denominator (sem: SEM-gx-engine-030)
//!
//! tmpfs (`/dev/shm`). Since fsync is close to free there, this number is **a number about CPU and (sem: SEM-gx-engine-030)
//! the lock**, not a number about disk (the same declaration as `support`'s module note).

mod support;

use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gx_adapter_fs::FsAdapter;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent_for, measuring, report, signing_key, Sandbox, AT};

/// The default number of concurrent agents. Tunable via `GLOVREX_BENCH_AGENTS`. (sem: SEM-gx-engine-031)
const DEFAULT_AGENTS: usize = 4;

/// Commits per agent. Tunable via `GLOVREX_BENCH_COMMITS`. (sem: SEM-gx-engine-032)
const DEFAULT_COMMITS: usize = 150;

/// 33 NFR-003's provisional SLA. The value D-7's firing condition refers to. (sem: SEM-gx-engine-033)
const SLA_COMMITS_PER_SEC: f64 = 100.0;

/// 🔴 **§62 R-7**: this bench's judgement becomes an **exit code** (wired to `tools/ci.sh` stage 10, (sem: SEM-gx-engine-034)
/// default off).
///
/// 🔴 **this stage going red is not D-7 firing.** The firing condition, verbatim, is "when AC-066,
/// **taken through serve**, falls below SLA" (`req/38:930`; quoted in SEM-gx-engine-034), and this
/// instrument is in-process -- it does not include HTTP, the router, Bearer, or JSON (req/103 §4-1).
/// Ruling #20 is "do not implement the lock, only measure", and that has not moved.
///
/// ∴ the threshold here is, in status, **a regression gate**: measured, `shared` runs 1,903-2,163
/// commits/s (req/103 §4-2) -- 19-21x the SLA -- and falling below 100 has no explanation other than
/// "the concurrent-commit path got 20x slower". A threshold with 20x headroom is the one kind of
/// threshold that can safely turn a noisy bench red, and that is why this value was chosen (quoted
/// in SEM-gx-engine-034) -- not because it is the firing condition's own number.
///
/// Tunable via `GLOVREX_DISJOINT_MIN_RATE`; the value used and its source print next to the number. (sem: SEM-gx-engine-034)
fn min_rate() -> (f64, String) {
    match std::env::var("GLOVREX_DISJOINT_MIN_RATE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        Some(overridden) => (overridden, "GLOVREX_DISJOINT_MIN_RATE".into()),
        None => (SLA_COMMITS_PER_SEC, "declared(33 NFR-003)".into()),
    }
}

fn agents() -> usize {
    std::env::var("GLOVREX_BENCH_AGENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_AGENTS)
}

fn commits() -> usize {
    std::env::var("GLOVREX_BENCH_COMMITS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_COMMITS)
}

/// One agent's subject: its own directory, so the object sets are **disjoint by construction** and
/// not by luck. ruling #20 asks for "disjoint object groups" (quoted in SEM-gx-engine-035) and a shared directory would make the
/// filesystem the thing under measurement (M5H7-3 (b)'s finding, one substrate over).
fn subject(agent: usize, n: usize) -> String {
    format!("agent-{agent:02}/subject-{n}")
}

/// What one commit attempt answers.
struct Attempt {
    took: Duration,
    /// `shared` arm only: how long the thread waited before it held the lock.
    waited: Duration,
    committed: bool,
}

/// An engine over a sandbox, with the fs adapter registered (AC-065's shape).
fn engine(sandbox: &Sandbox, name: &str) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        sandbox.dir().join(format!("{name}.journal")),
        gate(),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens on the tmpfs");
    engine.register_adapter(Arc::new(FsAdapter::new()), "gx-adapter-fs 0.1.0");
    engine
}

/// One lifecycle, with the engine already held.
fn one(engine: &mut Engine<InjectedEvidence>, sandbox: &Sandbox, agent: usize, n: usize) -> bool {
    let name = subject(agent, n);
    sandbox.write(&name, b"before");
    let locator = sandbox.locator(&name);
    let intent = intent_for(&locator, b"after");
    let seed = (agent as u64) << 32 | n as u64;
    if engine.submit(&intent, seed, AT).is_err() {
        return false;
    }
    let Ok(id) = engine.plan(&intent, AT) else {
        return false;
    };
    if engine.verify(&id, AT, &signing_key(), None).is_err() {
        return false;
    }
    if engine.canonicalize(&id, AT, None).is_err() {
        return false;
    }
    matches!(
        engine.commit(&id, AT, &signing_key()),
        Ok(Lifecycle::Committed)
    )
}

/// The shipped shape: N threads, one engine, one lock (**M6-06 adopted (a)**, sem: SEM-gx-engine-036).
fn shared_arm(sandbox: &Sandbox, agents: usize, commits: usize) -> (Vec<Attempt>, Duration) {
    let engine = Arc::new(Mutex::new(engine(sandbox, "shared")));
    let started = Instant::now();
    let attempts = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..agents)
            .map(|agent| {
                let engine = Arc::clone(&engine);
                scope.spawn(move || {
                    let mut mine = Vec::with_capacity(commits);
                    for n in 0..commits {
                        let began = Instant::now();
                        // The wait and the hold are two facts. A per-object lock removes the first
                        // and leaves the second, so a number that mixed them would answer a
                        // different question than ruling #20's (sem: SEM-gx-engine-037).
                        let mut held = engine.lock().expect("not poisoned");
                        let waited = began.elapsed();
                        let committed = one(&mut held, sandbox, agent, n);
                        drop(held);
                        mine.push(Attempt {
                            took: began.elapsed(),
                            waited,
                            committed,
                        });
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("an agent thread does not panic"))
            .collect::<Vec<_>>()
    });
    (attempts, started.elapsed())
}

/// The ceiling: N threads, N engines, nothing shared at all.
fn disjoint_arm(sandbox: &Sandbox, agents: usize, commits: usize) -> (Vec<Attempt>, Duration) {
    let started = Instant::now();
    let attempts = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..agents)
            .map(|agent| {
                scope.spawn(move || {
                    let mut mine_engine = engine(sandbox, &format!("disjoint-{agent:02}"));
                    let mut mine = Vec::with_capacity(commits);
                    for n in 0..commits {
                        let began = Instant::now();
                        let committed = one(&mut mine_engine, sandbox, agent, n);
                        mine.push(Attempt {
                            took: began.elapsed(),
                            waited: Duration::ZERO,
                            committed,
                        });
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("an agent thread does not panic"))
            .collect::<Vec<_>>()
    });
    (attempts, started.elapsed())
}

/// One thread, one engine: the serial baseline the other two are read against.
fn single_arm(sandbox: &Sandbox, commits: usize) -> (Vec<Attempt>, Duration) {
    let mut engine = engine(sandbox, "single");
    let started = Instant::now();
    let attempts = (0..commits)
        .map(|n| {
            let began = Instant::now();
            let committed = one(&mut engine, sandbox, 0, n);
            Attempt {
                took: began.elapsed(),
                waited: Duration::ZERO,
                committed,
            }
        })
        .collect();
    (attempts, started.elapsed())
}

fn summarise(tag: &str, attempts: &[Attempt], elapsed: Duration) -> f64 {
    let errors = attempts.iter().filter(|a| !a.committed).count();
    let rate = (attempts.len() - errors) as f64 / elapsed.as_secs_f64();
    println!(
        "{tag}_TOTAL attempts={} committed={} errors={errors} elapsed={elapsed:.3?} \
         throughput={rate:.2} commits/s",
        attempts.len(),
        attempts.len() - errors,
    );
    let mut took: Vec<Duration> = attempts.iter().map(|a| a.took).collect();
    report(tag, "per-commit", &mut took);
    rate
}

fn main() {
    if !measuring() {
        // The build check: both arms are reached, on a tree small enough to cost nothing.
        let sandbox = Sandbox::new("disjoint-build-check");
        let (single, _) = single_arm(&sandbox, 1);
        let (shared, _) = shared_arm(&sandbox, 2, 1);
        let (disjoint, _) = disjoint_arm(&sandbox, 2, 1);
        assert_eq!((single.len(), shared.len(), disjoint.len()), (1, 2, 2));
        println!("DISJOINT_BENCH_BUILD_ONLY arms=3");
        return;
    }

    let agents = agents();
    let commits = commits();
    let sandbox = Sandbox::new("disjoint");
    println!(
        "DISJOINT_GENERATOR agents={agents} commits_per_agent={commits} total={} fs={} \
         cpus={} (ruling #20: a lock is not implemented, only measured; sem: SEM-gx-engine-038)",
        agents * commits,
        support::filesystem_of(sandbox.dir()),
        std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get),
    );

    let (single, single_elapsed) = single_arm(&sandbox, commits);
    let single_rate = summarise("DISJOINT_SINGLE", &single, single_elapsed);

    let (shared, shared_elapsed) = shared_arm(&sandbox, agents, commits);
    let shared_rate = summarise("DISJOINT_SHARED", &shared, shared_elapsed);
    let mut waits: Vec<Duration> = shared.iter().map(|a| a.waited).collect();
    report("DISJOINT_SHARED_WAIT", "lock acquisition", &mut waits);
    let total_wait: Duration = shared.iter().map(|a| a.waited).sum();
    let total_took: Duration = shared.iter().map(|a| a.took).sum();
    // 🔴 Print two shares. **A ratio of sums breaks on one outlier** -- in the first measurement, 1 of
    // 600 waited 301ms, and with p50 still 50ns the ratio of sums came out 0.69. Printing only the sum
    // would be read as "70% is waiting" (quoted in SEM-gx-engine-039), and that is a lie about the
    // distribution. Put the ratio of medians beside it, and name the longest wait.
    let mut sorted_waits: Vec<Duration> = shared.iter().map(|a| a.waited).collect();
    let mut sorted_took: Vec<Duration> = shared.iter().map(|a| a.took).collect();
    sorted_waits.sort_unstable();
    sorted_took.sort_unstable();
    let mid = sorted_waits.len() / 2;
    println!(
        "DISJOINT_LOCK_WAIT_SHARE sum={:.4} median={:.6} total_wait={total_wait:.3?} \
         total_wall={total_took:.3?} max_wait={:.3?}  \
         (a per-object lock only removes the wait; the sum breaks on one outlier, so the median sits beside it; sem: SEM-gx-engine-040)",
        total_wait.as_secs_f64() / total_took.as_secs_f64().max(f64::MIN_POSITIVE),
        sorted_waits[mid].as_secs_f64() / sorted_took[mid].as_secs_f64().max(f64::MIN_POSITIVE),
        sorted_waits[sorted_waits.len() - 1],
    );

    let (disjoint, disjoint_elapsed) = disjoint_arm(&sandbox, agents, commits);
    let disjoint_rate = summarise("DISJOINT_CEILING", &disjoint, disjoint_elapsed);

    black_box(&single);
    println!(
        "DISJOINT_COMPARISON single={single_rate:.2} shared={shared_rate:.2} \
         ceiling={disjoint_rate:.2} commits/s  headroom_x={:.2}",
        disjoint_rate / shared_rate.max(f64::MIN_POSITIVE),
    );
    let (floor, budget_source) = min_rate();
    let below = shared_rate < floor;
    println!(
        "DISJOINT_D7 sla={SLA_COMMITS_PER_SEC} commits/s  shared_below_sla={below}  \
         floor={floor} BUDGET_SOURCE={budget_source}  \
         (the firing condition is `req/38:930`, \"when AC-066, taken through serve, falls below SLA\" \
         (quoted in SEM-gx-engine-041). This instrument is **in-process** and is not serve -- HTTP, \
         the router, Bearer and JSON are not included. ∴ this line is not the firing condition's own \
         judgement, it is the number one step before it)"
    );

    // 🔴 §62 R-7: the judgement leaves the process. See `min_rate`'s documentation for why a red
    // stage here is a **regression**, not D-7 firing — the two are one sentence apart and the next
    // reader of a red CI log is the person who needs that sentence.
    if below {
        eprintln!(
            "DISJOINT_FAIL shared={shared_rate:.2} < floor={floor} commits/s ({budget_source}) — \
             a regression, not D-7 firing (the firing condition goes through serve; ruling #20 is \
             unchanged; sem: SEM-gx-engine-042)"
        );
        std::process::exit(1);
    }
}
