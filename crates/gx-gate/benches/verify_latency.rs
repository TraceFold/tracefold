// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-064 (NFR-001) — how long `Gate::verify` takes, measured and **recorded**.
//!
//! 34 AC-064 verbatim: "Given: a PolicySet+InvariantCheck set equivalent to the shipped fs policy
//! pack under `policies/`, single node, warm cache, no external evidence collector. When: `gx-gate`'s
//! `criterion` bench `benches/verify_latency.rs` is run. Then: p99 ≤ 50ms." (sem: SEM-gx-gate-010)
//!
//! # What the gate of this AC is, and what it is not
//!
//! req/38 §19's **M3-15** ruling: "AC-064's gate is 'measured and recorded p99 with median + count +
//! denominator'; threshold comparison is record-only (a design budget). Do not claim '50ms was met'.
//! Making the bench-check fail is for the Owner, after the threshold is confirmed". The 50 ms is 33's
//! ASM-33-1 provisional value -- "until confirmed, treat this value as a 'design budget', not a
//! 'passing line'" (sem: SEM-gx-gate-011) -- so nothing in this file compares a measurement against it, and
//! nothing in `tools/ci.sh` fails on it.
//!
//! # Two instruments, because they answer two different questions
//!
//! **criterion** estimates the *mean cost of one iteration* from a sample of batch timings. That is
//! the right instrument for regression detection (51 §9's "performance-budget regression detection" (sem: SEM-gx-gate-012)) and it is the one
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
//! # The InvariantCheck set of the Given (sem: SEM-gx-gate-013)
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
            // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
            // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
            // varying this field alone moves no verdict) about the field and not about this fixture.
            decided_at: Timestamp(0),
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
/// req/58 §2.3's form: "median + count" with the denominator on the line, never a single value. (sem: SEM-gx-gate-014)
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

/// req/446 principle 2 -- what a fourth pack costs.
///
/// The reqdef writes the condition as "declare the latency effect of the pack count going 3 -> 4
/// by measurement (**0 bench declarations = not met**)", so this is the declaration. It is not the
/// same measurement as the four cases above: those hold one pack and vary the invariant count,
/// while this holds the invariant count at zero and varies **how many packs are composed**, which
/// is the only variable a shipped pack adds.
///
/// The request is on the fs substrate deliberately. It is the pack that is first in the composed
/// text either way, so what the extra statements cost is the cost of Cedar carrying them past a
/// decision that was already made -- the pessimistic reading, and the one a deployment on three
/// substrates actually pays when a fourth ships.
///
/// Same form as `latency_distribution`: median with the denominator on the line (req/58 2.3), and
/// no comparison against a budget.
#[cfg(feature = "pg")]
fn composition_cost() {
    let three: Vec<packs::ShippedPack> = packs::SHIPPED_PACKS
        .into_iter()
        .filter(|p| p.substrate != packs::POSTGRES_SUBSTRATE_TAG)
        .collect();
    let four: Vec<packs::ShippedPack> = packs::SHIPPED_PACKS.into_iter().collect();

    let compose = |set: &[packs::ShippedPack]| -> Gate {
        let mut text = String::new();
        for pack in set {
            text.push_str(pack.source);
            text.push('\n');
        }
        Gate::with_policies(gx_gate::PolicyEngine::parse(&text).expect("the packs compose"))
    };

    println!(
        "PACK_V0_COMPOSITION_COST samples_per_arm={RAW_SAMPLES} warmup={WARMUP} invariants=0          (one Instant pair per call; overhead included, not subtracted)"
    );
    let mut medians: Vec<(usize, usize, Duration)> = Vec::new();
    for set in [&three[..], &four[..]] {
        let gate = compose(set);
        let statements: usize = set.iter().map(|p| p.policy_ids.len()).sum();
        let pre = object("/tmp/x");
        let t = change();
        for _ in 0..WARMUP {
            one(&gate, &pre, &t, true, VerdictKind::Admit);
        }
        let mut samples: Vec<Duration> = Vec::with_capacity(RAW_SAMPLES);
        for _ in 0..RAW_SAMPLES {
            let started = Instant::now();
            one(&gate, &pre, &t, true, VerdictKind::Admit);
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let at = |q: f64| -> Duration {
            let rank = ((q * RAW_SAMPLES as f64).ceil() as usize).clamp(1, RAW_SAMPLES);
            samples[rank - 1]
        };
        println!(
            "PACK_V0_COMPOSITION packs={} statements={} n={} p50={:>9.3?} p90={:>9.3?}              p99={:>9.3?}",
            set.len(),
            statements,
            RAW_SAMPLES,
            at(0.50),
            at(0.90),
            at(0.99),
        );
        medians.push((set.len(), statements, at(0.50)));
    }
    if let [(_, _, before), (_, _, after)] = medians[..] {
        // Signed, and printed either way. A fourth pack making the median *fall* is a measurement
        // artefact rather than a speed-up, and printing only improvements would hide that.
        let delta = after.as_secs_f64() - before.as_secs_f64();
        println!(
            "PACK_V0_COMPOSITION_DELTA p50_3pack={before:.3?} p50_4pack={after:.3?}              delta={:+.3} us",
            delta * 1e6
        );
    }
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
        // 🔴 `req/832` ATOM -1: the composition measurement is "what does a fourth pack cost",
        // so on a build with no fourth pack there is nothing to measure -- and a run that
        // silently compared the three-pack set against itself would report a cost of zero as
        // though it were a finding.
        #[cfg(feature = "pg")]
        composition_cost();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_verify(&mut criterion);
    criterion.final_summary();
}
