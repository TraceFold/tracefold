// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-036** — fail-closed, across many candidates at once (FR-036, NFR-005, 43 T-4d/T-4e).
//!
//! 34 AC-036, verbatim (sem: SEM-gx-engine-452):
//!
//! > Given: `FailPosture::FailClosed` (default). When: multiple Candidates are `verify_start`ed
//! > while the gx-gate process is `kill -9`ed. Then: all resolve to `Aborted(VerifierUnavailable)`,
//! > and not one Committed entry is generated in the `ledger`. Also confirm via the configuration
//! > API that behavior differs when `FailPosture::FailOpen` is set per-substrate. (sem:
//! > SEM-gx-engine-452)
//!
//! # 🔴 "kill -9 the gx-gate process" names a process that does not exist (**E-M5-4**)
//! # (sem: SEM-gx-engine-452)
//!
//! 41 §2 makes gx-gate a **library**: `Gate::verify` is a function call in this process, and a
//! function call cannot become unreachable. §37 rules the reading rather than leaving the criterion
//! unconstructible:
//!
//! > **M5-19, adopted (a)** = **E-M5-4**: the only source of unreachability is the evidence
//! > collector. AC-036's "kill -9 the gx-gate process" is read as the erratum "the evidence
//! > collector is unreachable" (gate is a library -- it cannot become unreachable -- and NFR-005's
//! > verbatim "verifier/evidence collector unreachable" already permits this reading) (sem:
//! > SEM-gx-engine-453)
//!
//! So the unreachable thing here is [`gx_engine::UnreachableEvidence`], and
//! `tests/engine_shape.rs::verifier_unavailable_has_exactly_one_producer` is what keeps that the
//! **only** road to the reason — without which this suite would be measuring one of several.
//!
//! # "multiple" is the part a single-transformation probe would miss (sem: SEM-gx-engine-454)
//!
//! Hand 2 measured one candidate reaching T-4d. What this suite adds is that the posture is a
//! property of the **deployment** and not of a call: five candidates over three objects all abort,
//! the ledger stays empty, and `adapter.apply` is never reached. A fail-closed engine that leaked
//! one commit out of five would pass every single-transformation probe ever written.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, FailPosture, Timestamp};
use gx_engine::{Engine, EvidenceSource, InjectedEvidence, Lifecycle, UnreachableEvidence};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// Plan `n` candidates over `n` objects in one engine, then verify all of them.
///
/// Returns `(states, apply_calls, ledger_leaves, journal_records)`.
fn chaos<E: EvidenceSource>(
    name: &str,
    evidence: E,
    posture: FailPosture,
    n: usize,
) -> (Vec<Lifecycle>, usize, u64, usize) {
    let dir = scratch(name);
    let mut engine = Engine::open(dir.join("journal.bin"), gate(PERMIT_ALL), evidence)
        .expect("a fresh journal opens")
        .with_posture(posture);
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let mut ids = Vec::new();
    for k in 0..n {
        let i = intent(&format!("/tmp/target-{k}.txt"), "after");
        engine.submit(&i, 40 + k as u64, AT).expect("submit");
        ids.push(engine.plan(&i, AT).expect("plan"));
    }
    let states: Vec<Lifecycle> = ids
        .iter()
        .map(|id| {
            engine
                .verify(id, AT, &signing_key(), None)
                .expect("verify answers with a state")
        })
        .collect();
    (
        states,
        counts.totals()[4],
        engine.ledger().log().len(),
        engine.journal().len(),
    )
}

/// 🔴 AC-036: every candidate aborts, and the ledger stays empty.
#[test]
fn ac_036_an_unreachable_collector_aborts_every_candidate_and_commits_none() {
    let (states, applies, leaves, records) = chaos(
        "ac036_closed",
        UnreachableEvidence::new("the collector was killed"),
        FailPosture::FailClosed,
        5,
    );
    println!(
        "AC036_CLOSED STATES={states:?} APPLY_CALLS={applies} LEDGER_LEAVES={leaves} \
         JOURNAL_RECORDS={records}"
    );
    assert_eq!(
        states.len(),
        5,
        "\"multiple Candidates\" is more than one (sem: SEM-gx-engine-455)"
    );
    for state in &states {
        assert_eq!(
            *state,
            Lifecycle::Aborted(AbortReason::VerifierUnavailable),
            "43 T-4d: \"`FailPosture = FailClosed` (DR-2 default, all substrates)\" (sem: \
             SEM-gx-engine-456)"
        );
    }
    assert_eq!(
        leaves, 0,
        "AC-036: \"not one Committed entry is generated in the `ledger`\" (sem: SEM-gx-engine-457)"
    );
    assert_eq!(applies, 0, "nothing was applied, so P-4 was never at risk");
}

/// 🔴 AC-036's control: with the posture flipped, the same run behaves differently.
///
/// "also confirm via the configuration API that behavior differs when `FailPosture::FailOpen` is
/// set per-substrate" (sem: SEM-gx-engine-458).
/// The configuration API is [`Engine::with_posture`], and 43 §4 admits either scope
/// ("per-substrate or global setting", sem: SEM-gx-engine-458) — v0.1 takes the whole-deployment
/// reading, for the reason
/// `with_mode` gives: nothing in 42 declares a place to store a setting per `SubstrateKind`.
///
/// **Without this control the criterion is not measured.** A fail-closed engine that aborted
/// everything unconditionally would satisfy the probe above and fail the one property DR-2 is
/// about; the difference between the two runs is what says the posture is being read.
#[test]
fn ac_036_the_same_run_under_fail_open_is_admitted_instead() {
    let (states, applies, leaves, _records) = chaos(
        "ac036_open",
        UnreachableEvidence::new("the collector was killed"),
        FailPosture::FailOpen,
        5,
    );
    println!("AC036_OPEN STATES={states:?} APPLY_CALLS={applies} LEDGER_LEAVES={leaves}");
    for state in &states {
        assert_eq!(
            *state,
            Lifecycle::Admitted,
            "43 T-4e: \"downgrade to record-only-equivalent mode for this Transformation alone and \
             continue\" (sem: SEM-gx-engine-459)"
        );
    }
    assert_eq!(leaves, 0, "T-4e admits; it does not commit");
    assert_eq!(applies, 0);
}

/// And the reachable collector is the third leg: the same five candidates get verdicts.
///
/// The two probes above differ in the posture; this one differs in the **collector**, which is
/// what shows that neither of them is measuring "this engine aborts". "do not give skip and pass
/// the same face" (req/29 §4, sem: SEM-gx-engine-460) applied to a suite rather than to a value.
#[test]
fn ac_036_a_reachable_collector_leaves_the_same_candidates_admitted() {
    let (states, applies, leaves, _records) = chaos(
        "ac036_reachable",
        InjectedEvidence::none(),
        FailPosture::FailClosed,
        5,
    );
    println!("AC036_REACHABLE STATES={states:?} APPLY_CALLS={applies} LEDGER_LEAVES={leaves}");
    for state in &states {
        assert_eq!(*state, Lifecycle::Admitted, "PERMIT_ALL admits");
    }
    assert_eq!(applies, 0, "verify does not apply");
}
