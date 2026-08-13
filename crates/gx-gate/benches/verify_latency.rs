//! AC-064 (NFR-001) — how long `Gate::verify` takes, measured and **recorded**.
//!
//! 34 AC-064 逐語: 「Given: `policies/`出荷fs policy pack相当のPolicySet+InvariantCheck群、単一ノード・
//! ウォームキャッシュ、外部evidence collectorなし。When: `gx-gate`の`criterion`ベンチ
//! `benches/verify_latency.rs`を実行する。Then: p99 ≤ 50ms。」
//!
//! # What the gate of this AC is, and what it is not
//!
//! req/38 §19's **M3-15** ruling: 「AC-064 は「p99 を median+回数+分母つきで測って記録した」を gate と
//! し、閾値比較は記録のみ(設計予算)。「50ms を満たした」と主張しない。bench-check の fail 化は Owner
//! 閾値確定後」. The 50 ms is 33's ASM-33-1 provisional value — 「確定前はこの値を『合格ライン』では
//! なく『設計予算』として扱う」 — so nothing in this file compares a measurement against it, and
//! nothing in `tools/ci.sh` fails on it.
//!
//! # Two instruments, because they answer two different questions
//!
//! **criterion** estimates the *mean cost of one iteration* from a sample of batch timings. That is
//! the right instrument for regression detection (51 §9's 「性能予算の回帰検出」) and it is the one
//! AC-064 names. It is **not** an instrument for a percentile of per-call latency: its sample is a
//! sample of batch means, and the p99 of batch means is not the p99 of calls — averaging inside a
//! batch is exactly what removes the tail a p99 is asked about.
//!
//! So a second, smaller instrument is run first: `latency_distribution` times **one call per
//! sample** with `Instant`, sorts, and prints p50 / p90 / p99 / max with `n`. That is a percentile
//! of what NFR-001 is about. Its own bias is stated rather than assumed: a `Instant::now()` pair
//! costs tens of nanoseconds on this machine against a call that costs tens of microseconds, so the
//! per-sample overhead is under 0.1% — and it is *included* in every number below rather than
//! subtracted, because subtracting an estimate would make the reported figure a derived one.
//!
//! Neither instrument is a claim about a deployment: one node, warm cache, no evidence collector,
//! this machine's CPU. 34's Given fixes the first three; the fourth is why the numbers are recorded
//! with the toolchain and the host in the report rather than published as a property of gx.
//!
//! # The InvariantCheck群 of the Given
//!
//! **D-9** ruled that no ready-made invariant ships (req/38 §24), so a bench that used the shipped
//! artifact alone would measure a gate with an empty registry — which is not what AC-064's Given
//! describes. Both are therefore measured: the pack with no invariants, and the pack with three, so
//! that the cost of the registry is a number rather than an assumption. The three are the shape a
//! deployment writes (`tests/ac_026.rs`'s mocks are the same shape) and they live here, in a bench
//! target that no published artifact contains.

use criterion::{BenchmarkId, Criterion};
use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::{packs, Gate, GateInput, InvariantCheck, InvariantRegistry, InvariantResult, Result};
use std::hint::black_box;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// The subject
// ---------------------------------------------------------------------------

/// A check that holds, and that reads the input it is given (E-M3-7) so that it is not optimised
/// into a constant.
struct Holds {
    id: String,
}

impl InvariantCheck for Holds {
    fn id(&self) -> &str {
        &self.id
    }

    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult> {
        let holds = !input.pre.locator().is_empty() && input.t.order() <= 2;
        InvariantResult::new(self.id.clone(), holds, None)
    }
}

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn change() -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        0,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: ChangeContext::Substrate,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("the fixture is inside E-M3-13's range")
}

fn object(locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        SubstrateKind::Fs,
        locator.to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

/// A gate holding the shipped pack and `n` invariants.
fn gate(n: usize) -> Gate {
    let mut registry = InvariantRegistry::new();
    for i in 0..n {
        registry
            .register(Box::new(Holds {
                id: format!("bench.holds.{i}"),
            }))
            .expect("distinct ids");
    }
    Gate::with_policies(packs::fs_pack().expect("the shipped pack parses"))
        .with_invariants(registry)
}

/// The four cases, each named by the arm of the fold it reaches.
///
/// `escalate` is the most expensive by construction: **E-M3-4**'s ticket is identified by what it
/// is, so that arm mints a CID through gx-canon while the other two do not.
struct Case {
    name: &'static str,
    locator: &'static str,
    invariants: usize,
    invert_available: bool,
    expected: VerdictKind,
}

const CASES: [Case; 4] = [
    Case {
        name: "admit-no-invariants",
        locator: "/tmp/x",
        invariants: 0,
        invert_available: true,
        expected: VerdictKind::Admit,
    },
    Case {
        name: "admit-3-invariants",
        locator: "/tmp/x",
        invariants: 3,
        invert_available: true,
        expected: VerdictKind::Admit,
    },
    Case {
        name: "deny-3-invariants",
        locator: "/etc/passwd",
        invariants: 3,
        invert_available: true,
        expected: VerdictKind::Deny,
    },
    Case {
        name: "escalate-3-invariants",
        locator: "/tmp/x",
        invariants: 3,
        invert_available: false,
        expected: VerdictKind::Escalate,
    },
];

/// One `Gate::verify`, with the verdict checked so that a benchmark measuring the wrong arm is
/// visible instead of fast.
fn one(gate: &Gate, pre: &ObjectSnapshot, t: &Transformation, invert: bool, want: VerdictKind) {
    let planned = PlannedDeltaBytes(b"opaque to everything below the adapter (P-6)".to_vec());
    let verdict = gate
        .verify(GateInput {
            t,
            pre,
            planned: &planned,
            evidence: &[],
            invert_available: invert,
        })
        .expect("the gate can evaluate this input");
    assert_eq!(verdict.kind(), want, "the benchmark measured the wrong arm");
    black_box(verdict);
}

// ---------------------------------------------------------------------------
// Instrument 1: per-call latency, for the percentile NFR-001 is stated in
// ---------------------------------------------------------------------------

/// `n` calls, one timing each, printed as a distribution with its denominator.
///
/// req/58 §2.3's form: 「median + 回数」 with the denominator on the line, never a single value.
/// 2,000 samples put the p99 twenty samples from the top, which is the smallest count at which the
/// figure is not one observation.
const RAW_SAMPLES: usize = 2_000;

/// The warm cache 34's Given asks for. Cedar parses the pack once at `fs_pack()`; what warms here
/// is the allocator and the branch predictors.
const WARMUP: usize = 200;

fn latency_distribution() {
    println!("AC064_RAW_INSTRUMENT samples_per_case={RAW_SAMPLES} warmup={WARMUP} (one Instant pair per call; overhead included, not subtracted)");
    for case in &CASES {
        let gate = gate(case.invariants);
        let pre = object(case.locator);
        let t = change();
        for _ in 0..WARMUP {
            one(&gate, &pre, &t, case.invert_available, case.expected);
        }
        let mut samples: Vec<Duration> = Vec::with_capacity(RAW_SAMPLES);
        for _ in 0..RAW_SAMPLES {
            let started = Instant::now();
            one(&gate, &pre, &t, case.invert_available, case.expected);
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let at = |q: f64| -> Duration {
            // Nearest-rank: the smallest sample at or above the q-th percentile. Named because a
            // percentile is a choice of rule and two rules disagree at n = 2,000 by one sample.
            let rank = ((q * RAW_SAMPLES as f64).ceil() as usize).clamp(1, RAW_SAMPLES);
            samples[rank - 1]
        };
        println!(
            "AC064_RAW {:<22} n={} min={:>9.3?} p50={:>9.3?} p90={:>9.3?} p99={:>9.3?} max={:>9.3?}",
            case.name,
            RAW_SAMPLES,
            samples[0],
            at(0.50),
            at(0.90),
            at(0.99),
            samples[RAW_SAMPLES - 1],
        );
    }
    println!(
        "AC064_BUDGET 50ms is 33's ASM-33-1 provisional value and is a design budget, not a pass \
         mark (M3-15). Nothing above is compared against it."
    );
}

// ---------------------------------------------------------------------------
// Instrument 2: criterion, which is the tool AC-064 names
// ---------------------------------------------------------------------------

fn bench_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_verify");
    for case in &CASES {
        let gate = gate(case.invariants);
        let pre = object(case.locator);
        let t = change();
        group.bench_with_input(BenchmarkId::from_parameter(case.name), case, |b, case| {
            b.iter(|| one(&gate, &pre, &t, case.invert_available, case.expected));
        });
    }
    group.finish();
}

fn main() {
    // `--bench` is the flag `cargo bench` passes and `cargo test --benches` does not, and it is the
    // same flag criterion itself reads to decide between measuring and its one-iteration test mode.
    // The raw instrument follows it rather than looking for `--test`: measured on 2026-08-08,
    // `cargo test -p gx-gate --benches` passes neither flag, so a guard written the other way round
    // ran 8,000 timed calls in an **unoptimised** build -- p50 186 µs against 7.8 µs here, a number
    // that means nothing except that the profile was wrong. A test run is a check that this file
    // still builds and still reaches the arms it claims; a measurement is `cargo bench`.
    let measuring = std::env::args().any(|a| a == "--bench");
    if measuring {
        latency_distribution();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_verify(&mut criterion);
    criterion.final_summary();
}
