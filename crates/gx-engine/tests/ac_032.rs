// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-032 (FR-032) — the three verdicts land in the three states, and the two that are not verdicts
//! land somewhere else.
//!
//! 34 AC-032, verbatim (sem: SEM-gx-engine-417): "Given: T in the `Candidate` state. When: the
//! `verify` step's mocked `Gate::verify` returns Admit/Deny/Escalate respectively, in three cases.
//! Then: T's state transitions uniquely to Admitted/Denied/Escalated respectively." unit (3 cases)
//!
//! # "mocked `Gate::verify`", and why these gates are real (sem: SEM-gx-engine-417)
//!
//! 41 §4 declares `pub struct Gate`, not a trait, and **E-M3-3** made `verify` fallible without
//! making it replaceable. There is nothing to mock. What there is instead is a gate that can be
//! *configured* to reach each arm, and the three configurations below are the ones gx-gate's own
//! suites use: a permissive policy set admits, a `forbid` clause on the locator denies, and
//! **E-M3-4**'s rule (`invert_available == false → Escalate`) escalates.
//!
//! That is a stronger reading of the criterion than a mock would give, and it is worth saying why
//! rather than treating it as a convenience: a mock would prove that this file's `match` has three
//! arms. Configuring the real gate proves that the three arms are reachable *through* the gate the
//! engine actually calls, which is the claim FR-032 makes ("calls `Gate::verify` ... and reflects
//! the result in the Transformation's state"; sem: SEM-gx-engine-418).
//!
//! # The other two answers
//!
//! `Gate::verify` returns `Result<Verdict>`, so there are four outcomes and not three, and the
//! evidence collector adds two more before the gate is reached at all. 34 gives them to AC-036 and
//! AC-041 (hand 6); the *mechanism* is this hand's, because T-4d and T-4e are in req/78 §6.2's row
//! for it. They are measured here, labelled as what they are, and **not claimed as an AC**.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, FailPosture, Timestamp, VerdictKind};
use gx_engine::{
    Engine, EngineJournalRecord, EvidenceSource, InjectedEvidence, Lifecycle, UnreachableEvidence,
};
use gx_witness::evidence::{Evidence, PolicyDecision};
use support::{gate, intent, scratch, signing_key, StubAdapter, FORBID_ETC, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// An engine with a chosen gate, a chosen adapter and a chosen evidence source.
fn engine<E: EvidenceSource>(
    name: &str,
    policies: &str,
    adapter: StubAdapter,
    evidence: E,
) -> Engine<E> {
    let dir = scratch(name);
    let mut engine =
        Engine::open(dir.join("journal.bin"), gate(policies), evidence).expect("a fresh journal");
    engine.register_adapter(Arc::new(adapter), "stub-1");
    engine
}

/// Run `submit` → `plan` → `verify` and answer with the state and the transformation id.
fn run<E: EvidenceSource>(
    e: &mut Engine<E>,
    locator: &str,
) -> (gx_core::TransformationId, Lifecycle) {
    let i = intent(locator, "v1");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");
    let state = e
        .verify(&id, AT, &signing_key(), None)
        .expect("verify answers with a state");
    (id, state)
}

// ---------------------------------------------------------------------------
// AC-032's three cases
// ---------------------------------------------------------------------------

/// Case 1: the gate admits, and the transformation is `Admitted` (T-4a).
#[test]
fn ac_032_admit_goes_to_admitted() {
    let mut e = engine(
        "ac032_admit",
        PERMIT_ALL,
        StubAdapter::default(),
        InjectedEvidence::none(),
    );
    let (id, state) = run(&mut e, "/tmp/x");

    assert_eq!(state, Lifecycle::Admitted);
    assert_eq!(e.state(&id), Some(Lifecycle::Admitted));
    assert_eq!(e.verdict(&id), Some(VerdictKind::Admit));
    assert_eq!(e.enforced(&id), Some(true), "no degradation happened");
    assert_eq!(e.fail_posture_engaged(&id), Some(false));
}

/// Case 2: the gate refuses, and the transformation is `Denied` (T-4b).
#[test]
fn ac_032_deny_goes_to_denied() {
    let mut e = engine(
        "ac032_deny",
        FORBID_ETC,
        StubAdapter::default(),
        InjectedEvidence::none(),
    );
    let (id, state) = run(&mut e, "/etc/passwd");

    assert_eq!(state, Lifecycle::Denied);
    assert_eq!(e.verdict(&id), Some(VerdictKind::Deny));
    assert_eq!(
        e.enforced(&id),
        Some(true),
        "a Deny under Enforce is still enforced -- `enforced` is about T-8r, not about the verdict"
    );
}

/// Case 3: the gate escalates, and the transformation is `Escalated` (T-4c).
///
/// The trigger is **E-M3-4**: "`invert_available=false → Escalate` is M3's minimal generating
/// rule" (sem: SEM-gx-engine-419), so
/// the adapter is the one whose `invert` answers `Ok(None)`. That the *engine* is what asks the
/// adapter is this hand's addition -- 41 §4 gives `GateInput` the field and 43 schedules
/// `adapter.invert` at T-10b, so the question has to be asked earlier than 43 asks it (M5H2-6).
#[test]
fn ac_032_escalate_goes_to_escalated() {
    let mut e = engine(
        "ac032_escalate",
        PERMIT_ALL,
        StubAdapter::without_inverse(),
        InjectedEvidence::none(),
    );
    let (id, state) = run(&mut e, "/tmp/x");

    assert_eq!(state, Lifecycle::Escalated);
    assert_eq!(e.verdict(&id), Some(VerdictKind::Escalate));
}

/// The three are **distinct** states, and the journal says which one each was.
///
/// "transitions uniquely" (sem: SEM-gx-engine-420) is a statement about a function, and a function is not measured by three
/// separate assertions that each hold on their own: an implementation that answered `Admitted`
/// always would pass case 1 and fail cases 2 and 3, but one that keyed off the locator would pass
/// all three. So the three runs are compared with each other, and the `Verdict` records are read
/// back out of the journal, which is where a later replay will look.
#[test]
fn ac_032_the_three_cases_are_three_distinct_recorded_verdicts() {
    let mut admit = engine(
        "ac032_all_admit",
        PERMIT_ALL,
        StubAdapter::default(),
        InjectedEvidence::none(),
    );
    let mut deny = engine(
        "ac032_all_deny",
        FORBID_ETC,
        StubAdapter::default(),
        InjectedEvidence::none(),
    );
    let mut escalate = engine(
        "ac032_all_escalate",
        PERMIT_ALL,
        StubAdapter::without_inverse(),
        InjectedEvidence::none(),
    );

    let (a, sa) = run(&mut admit, "/tmp/x");
    let (d, sd) = run(&mut deny, "/etc/passwd");
    let (x, sx) = run(&mut escalate, "/tmp/x");

    let names = [sa.name(), sd.name(), sx.name()];
    println!("AC032_STATES={names:?}");
    assert_eq!(
        names.len(),
        names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "three cases, three states"
    );

    for (engine, id, kind) in [
        (&admit, a, VerdictKind::Admit),
        (&deny, d, VerdictKind::Deny),
        (&escalate, x, VerdictKind::Escalate),
    ] {
        let recorded = engine
            .journal()
            .records()
            .iter()
            .find_map(|r| match r {
                EngineJournalRecord::Verdict {
                    transformation,
                    kind,
                    verdict_digest,
                    fail_posture_engaged,
                    ..
                } if *transformation == id => Some((*kind, *verdict_digest, *fail_posture_engaged)),
                _ => None,
            })
            .expect("T-4a/b/c wrote a Verdict record");
        assert_eq!(recorded.0, kind);
        assert!(
            recorded.1.is_some(),
            "a verdict the gate reached has a digest to identify it"
        );
        assert!(!recorded.2, "no fail posture was engaged");
    }
}

/// The evidence a source hands over is the evidence the gate is given (FR-032's "after evidence
/// is collected"; sem: SEM-gx-engine-421).
///
/// AC-016 states this for `GateInput`'s slot; this is the same claim one layer up, where the slot
/// is filled. The gate is given a policy set that reads `context.evidence_count` through ASM-60-1's
/// mapping, so a source that dropped its items would change the verdict rather than only the shape.
#[test]
fn ac_032_the_collected_evidence_reaches_the_gate() {
    const NEEDS_EVIDENCE: &str = r#"@id("permit-with-evidence")
permit (principal, action, resource)
when { context.evidence_count > 0 };
"#;

    let evidence = vec![Evidence::PolicyEvaluation {
        decision: PolicyDecision::Allow,
        policy_id: "upstream-check".to_string(),
        explanation_digest: None,
    }];

    let mut with = engine(
        "ac032_evidence_yes",
        NEEDS_EVIDENCE,
        StubAdapter::default(),
        InjectedEvidence::new(evidence),
    );
    let mut without = engine(
        "ac032_evidence_no",
        NEEDS_EVIDENCE,
        StubAdapter::default(),
        InjectedEvidence::none(),
    );

    let (_, admitted) = run(&mut with, "/tmp/x");
    let (_, denied) = run(&mut without, "/tmp/x");
    assert_eq!(admitted, Lifecycle::Admitted, "one item reached the policy");
    assert_eq!(
        denied,
        Lifecycle::Denied,
        "and the empty source is a source that succeeded with nothing, not a source that failed"
    );
}

// ---------------------------------------------------------------------------
// Not AC-032: the two answers that are not verdicts (T-4d, T-4e, ⊥)
// ---------------------------------------------------------------------------

/// 🔴 **T-4d**: the collector cannot be reached and the posture is `FailClosed` (the default).
///
/// This is the behavioural half of **M5-03, adopted (a)**'s "`Err` is the sole producer of
/// `VerifierUnavailable`" (sem: SEM-gx-engine-422) -- the source scan in `engine_shape.rs` says the word is written once, and this says
/// the one road is the collector's. AC-036 is hand 6's; the mechanism is here.
#[test]
fn t_4d_an_unreachable_collector_aborts_fail_closed() {
    let mut e = engine(
        "ac032_t4d",
        PERMIT_ALL,
        StubAdapter::default(),
        UnreachableEvidence::new("the collector's socket is closed"),
    );
    assert_eq!(e.posture(), FailPosture::FailClosed, "DR-2's default");
    let (id, state) = run(&mut e, "/tmp/x");

    assert_eq!(state, Lifecycle::Aborted(AbortReason::VerifierUnavailable));
    assert_eq!(e.verdict(&id), None, "no verdict exists to record");
    assert!(
        e.journal().records().iter().any(|r| matches!(
            r,
            EngineJournalRecord::Aborted {
                reason: AbortReason::VerifierUnavailable,
                ..
            }
        )),
        "T-4d's journal cell"
    );
}

/// 🔴 **T-4e**: the same failure under an explicit `FailOpen` opt-in.
///
/// 43 T-4e: "for this Transformation alone, degrade to the record-only-mode equivalent and carry
/// on; `enforced=false` and `fail_posture_engaged=true` must be stamped into the receipt;
/// journal: `Verdict{id, Admit, fail_posture_engaged=true}`" (sem: SEM-gx-engine-423).
/// The receipt is hand 4's -- **E-M2-7** already put the field in
/// `ReceiptPayload`, which is why req/78's M5-12 was ruled not adopted -- and the journal is this hand's.
///
/// `verdict_digest` is `None`, and that is the point of the field being an `Option`: the gate was
/// never asked, so there is no verdict to identify.
#[test]
fn t_4e_an_unreachable_collector_degrades_under_an_explicit_fail_open() {
    let dir = scratch("ac032_t4e");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        UnreachableEvidence::new("the collector's socket is closed"),
    )
    .expect("a fresh journal")
    .with_posture(FailPosture::FailOpen);
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let (id, state) = run(&mut e, "/tmp/x");

    assert_eq!(state, Lifecycle::Admitted, "degraded, and continuing");
    assert_eq!(e.enforced(&id), Some(false), "43 T-4e: `enforced=false`");
    assert_eq!(
        e.fail_posture_engaged(&id),
        Some(true),
        "43 T-4e: `fail_posture_engaged=true`"
    );
    assert_eq!(
        e.verdict(&id),
        None,
        "nothing decided this; the gate was never reached"
    );

    let recorded = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Verdict {
                transformation,
                kind,
                verdict_digest,
                fail_posture_engaged,
                ..
            } if *transformation == id => Some((*kind, *verdict_digest, *fail_posture_engaged)),
            _ => None,
        })
        .expect("T-4e writes a Verdict record");
    assert_eq!(recorded.0, VerdictKind::Admit);
    assert_eq!(recorded.1, None, "no verdict was computed, so no digest");
    assert!(recorded.2, "and the record says why");
}

/// 🔴 **E-M5-5** (M5-23, adopted (a); sem: SEM-gx-engine-424): the gate's ⊥ is `Aborted`, and `RecordOnly` does not reach it.
///
/// > `RecordOnly` only takes effect on `Deny`, not on ⊥(Err) -- ⊥ is "no verdict exists", so it is
/// > always `Aborted` (fail-closed) (sem: SEM-gx-engine-424)
///
/// The gate here is `Gate::unconfigured()`, which answers `Err(Unevaluable)` -- req/29 §4's rule
/// that "an empty `policies/` directory and a working deployment must not look the same"
/// (sem: SEM-gx-engine-425). The
/// engine is put in `RecordOnly`, the mode that carries a *refusal* through to a commit, and it
/// makes no difference: ⊥ is not a refusal.
///
/// The reason is `InternalError` and not `VerifierUnavailable`, which keeps M5-03's single-producer
/// property intact. See `pipeline.rs` for the derivation and **M5H2-5** for the ticket.
#[test]
fn e_m5_5_the_gates_bottom_aborts_whatever_the_enforcement_mode_says() {
    for mode in [
        gx_core::EnforcementMode::Enforce,
        gx_core::EnforcementMode::RecordOnly,
    ] {
        let dir = scratch(&format!("ac032_bottom_{}", mode.as_str()));
        let mut e = Engine::open(
            dir.join("journal.bin"),
            gx_gate::Gate::unconfigured(),
            InjectedEvidence::none(),
        )
        .expect("a fresh journal")
        .with_mode(mode);
        e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

        let (id, state) = run(&mut e, "/tmp/x");
        assert_eq!(
            state,
            Lifecycle::Aborted(AbortReason::InternalError),
            "⊥ under {mode:?} is still an abort"
        );
        assert_eq!(e.verdict(&id), None);
        assert!(
            !e.journal().records().iter().any(|r| matches!(
                r,
                EngineJournalRecord::Aborted {
                    reason: AbortReason::VerifierUnavailable,
                    ..
                }
            )),
            "⊥ is not unreachability: M5-03's single producer stays the collector"
        );
    }
}

/// `verify` refuses a state 43 T-3 does not offer it from.
///
/// 43 T-3's from-state is `Candidate` and nothing else. Verifying twice, or verifying something
/// already denied, is a caller error and is refused rather than repeated -- "don't give skip and
/// pass the same face" (sem: SEM-gx-engine-426) at the level of a transition.
#[test]
fn verify_is_refused_from_every_state_but_candidate() {
    let mut e = engine(
        "ac032_states",
        PERMIT_ALL,
        StubAdapter::default(),
        InjectedEvidence::none(),
    );
    let (id, _) = run(&mut e, "/tmp/x");
    let again = e
        .verify(&id, AT, &signing_key(), None)
        .expect_err("Admitted is not Candidate");
    assert_eq!(again.kind(), "InvalidState", "{again}");

    let unknown = e
        .verify(
            &gx_core::TransformationId(gx_core::Cid([9u8; 32])),
            AT,
            &signing_key(),
            None,
        )
        .expect_err("nothing is there");
    assert_eq!(unknown.kind(), "NotFound", "{unknown}");
}
