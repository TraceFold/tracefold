// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-027 (FR-027) — the meet of the two systems, over all four quadrants, and the third arm
//! **E-M3-4** adds to it.
//!
//! AC-027 verbatim: "Given: all 4 combinations of the Cedar evaluation and the invariant evaluation
//! (Admit×Admit, Admit×Deny, Deny×Admit, Deny×Deny). When: `Gate::verify` is called. Then: if either
//! side includes even one Deny, the overall Verdict must be `Deny(Vec<Reason>)`. `Admit` only when
//! both are Admit." Judgment method: "property (exhaustive over the 4 quadrants)", M3. (sem: SEM-gx-gate-373)
//!
//! # Why the file is called this
//!
//! 51 §3 filed AC-027 under a property named `admissible_composition` asserting
//! `A(f) ∧ A(g) → A(g∘f)`, which is a different statement about a different composition. req/38
//! §19's M3-22 separates them:
//!
//! > "M3-22 (adopted = an erratum on 51's side): AC-027 is tied to the `verdict_meet` suite (the
//! > meet of two systems, Deny precedence); `admissible_composition` carries no AC (T1 only). The
//! > two senses of 'composition' (composing a transformation = T1 / the meet of the two systems =
//! > AC-027) are separated in the GLOSSARY" -> **E-M3-10** (sem: SEM-gx-gate-374)
//!
//! So this is the suite that ruling names. T1 is not measured here and is not a property of this
//! crate at all (**E-M3-5**, and `lib.rs`'s module header says so at length).
//!
//! # Three verdicts, one order
//!
//! The quadrant table is AC-027's, and the third arm is AC-048's gate half, promoted into M3:
//!
//! > "M3-04 (adopted = option a): raise AC-048's gate-side half to an M3 requirement -- make
//! > **`invert_available=false -> Escalate`** the minimal generation rule for M3" -> **E-M3-4** (sem: SEM-gx-gate-375)
//!
//! The two interact on exactly one input -- both systems admit and no inverse exists -- where
//! AC-027's second sentence would say `Admit` and E-M3-4 says `Escalate`. The tests below hold
//! `invert_available = true` while measuring AC-027's four quadrants, and measure the escalation
//! separately, which is the reading that keeps both statements true: an `Admit` still implies that
//! both systems admitted. The tension itself is reported in `req/64_M3_HAND4_REPORT_2026-08-08.md`
//! §4 rather than settled here.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::verdict::InvariantResult;
use gx_gate::{
    ApprovalRequirement, Gate, GateInput, InvariantCheck, InvariantRegistry, PolicyEngine, Reason,
    ReasonSource, Result, Verdict, INVARIANT_VIOLATED, INVERSE_UNAVAILABLE, POLICY_DENIED,
};
use proptest::prelude::*;

/// Permits everything, so the policy half of a quadrant is chosen by the locator alone below.
const PACK: &str = r#"@id("permit-default")
permit (principal, action, resource);

@id("forbid-etc")
forbid (principal, action, resource)
when { resource.locator like "/etc/*" };
"#;

/// An invariant that answers the same way every time, and counts.
struct Fixed {
    id: &'static str,
    holds: bool,
    calls: Arc<AtomicUsize>,
}

impl InvariantCheck for Fixed {
    fn id(&self) -> &str {
        self.id
    }

    fn check(&self, _input: &GateInput<'_>) -> Result<InvariantResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        InvariantResult::new(
            self.id.to_string(),
            self.holds,
            (!self.holds).then(|| format!("{} was asked and said no", self.id)),
        )
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
            actor: Actor::Human {
                key: "key-human-1".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 0 is under the ceiling")
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

/// A gate holding the pack and one invariant with the given answer.
fn gate(holds: bool) -> Gate {
    let registry = InvariantRegistry::new()
        .with(Box::new(Fixed {
            id: "the-invariant",
            holds,
            calls: Arc::new(AtomicUsize::new(0)),
        }))
        .expect("one id");
    Gate::with_policies(PolicyEngine::parse(PACK).expect("the pack parses"))
        .with_invariants(registry)
}

/// One quadrant: `policy_admits` chooses the locator, `invariant_holds` chooses the mock.
fn quadrant(policy_admits: bool, invariant_holds: bool, invert_available: bool) -> Verdict {
    let t = change();
    let pre = object(if policy_admits {
        "/tmp/x"
    } else {
        "/etc/passwd"
    });
    let planned = PlannedDeltaBytes(Vec::new());
    gate(invariant_holds)
        .verify(GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available,
            // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
            // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
            // varying this field alone moves no verdict) about the field and not about this fixture.
            decided_at: Timestamp(0),
        })
        .expect("every input here is evaluable; ⊥ is E-M3-3's and not a quadrant")
}

// ---------------------------------------------------------------------------
// The four quadrants, one test each (34: "all 4 combinations") (sem: SEM-gx-gate-376)
// ---------------------------------------------------------------------------

/// Admit × Admit → `Admit`, and the proof records both systems.
#[test]
fn ac_027_admit_times_admit_is_the_only_admit() {
    let verdict = quadrant(true, true, true);
    assert_eq!(verdict.kind(), VerdictKind::Admit);

    let Verdict::Admit(proof) = verdict else {
        panic!("both systems admitted")
    };
    assert_eq!(proof.policy_decisions().len(), 1, "the policy that decided");
    assert_eq!(
        proof.invariant_results().len(),
        1,
        "the invariant that held"
    );
    assert!(proof.invariant_results()[0].holds());
}

/// Admit × Deny → `Deny`, carrying the invariant's reason and no policy's.
#[test]
fn ac_027_admit_times_deny_is_a_deny() {
    let verdict = quadrant(true, false, true);
    assert_eq!(verdict.kind(), VerdictKind::Deny);

    let Verdict::Deny(reasons) = verdict else {
        panic!("an invariant refused")
    };
    assert_eq!(reasons.len(), 1);
    assert_eq!(reasons[0].code(), INVARIANT_VIOLATED);
    assert_eq!(
        *reasons[0].source(),
        ReasonSource::Invariant {
            invariant_id: "the-invariant".to_string()
        }
    );
}

/// Deny × Admit → `Deny`, carrying the policy's reason and no invariant's.
#[test]
fn ac_027_deny_times_admit_is_a_deny() {
    let verdict = quadrant(false, true, true);
    assert_eq!(verdict.kind(), VerdictKind::Deny);

    let Verdict::Deny(reasons) = verdict else {
        panic!("the policy refused")
    };
    assert_eq!(reasons.len(), 1);
    assert_eq!(reasons[0].code(), POLICY_DENIED);
    assert_eq!(
        *reasons[0].source(),
        ReasonSource::Policy {
            policy_id: "forbid-etc".to_string()
        }
    );
}

/// Deny × Deny → `Deny`, carrying **both** reasons.
///
/// A fold that returned at the first refusal would pass every other quadrant and lose half of this
/// one's record. 42 §3.8 types the arm `Vec<Reason>` for that reason: a refusal can have more than
/// one cause, and an operator fixing only the one that was reported would be refused again.
#[test]
fn ac_027_deny_times_deny_keeps_both_reasons() {
    let verdict = quadrant(false, false, true);
    assert_eq!(verdict.kind(), VerdictKind::Deny);

    let Verdict::Deny(reasons) = verdict else {
        panic!("both systems refused")
    };
    assert_eq!(reasons.len(), 2, "both systems are recorded");
    let codes: Vec<&str> = reasons.iter().map(Reason::code).collect();
    assert!(codes.contains(&POLICY_DENIED) && codes.contains(&INVARIANT_VIOLATED));
}

// ---------------------------------------------------------------------------
// E-M3-12: the order a refusal is recorded in
// ---------------------------------------------------------------------------

/// The reasons of a `Deny` come out in `(source kind, id, code)` order, whatever order they were
/// built in (**E-M3-12**).
#[test]
fn e_m3_12_a_deny_is_recorded_in_one_order() {
    let invariant = |id: &str| {
        Reason::new(
            INVARIANT_VIOLATED,
            format!("{id} does not hold"),
            ReasonSource::Invariant {
                invariant_id: id.to_string(),
            },
        )
        .expect("in the vocabulary")
    };
    let policy = |id: &str| {
        Reason::new(
            POLICY_DENIED,
            format!("{id} forbids this"),
            ReasonSource::Policy {
                policy_id: id.to_string(),
            },
        )
        .expect("in the vocabulary")
    };

    let one = Verdict::deny(vec![
        invariant("zeta"),
        policy("bravo"),
        invariant("alpha"),
        policy("alpha"),
    ])
    .expect("four reasons");
    let other = Verdict::deny(vec![
        policy("alpha"),
        invariant("alpha"),
        policy("bravo"),
        invariant("zeta"),
    ])
    .expect("the same four, shuffled");

    assert_eq!(
        one, other,
        "the order the reasons arrived in is not recorded"
    );

    let Verdict::Deny(reasons) = one else {
        panic!("a Deny")
    };
    let seen: Vec<(&str, &str)> = reasons
        .iter()
        .map(|r| (r.source().id(), r.code()))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("alpha", POLICY_DENIED),
            ("bravo", POLICY_DENIED),
            ("alpha", INVARIANT_VIOLATED),
            ("zeta", INVARIANT_VIOLATED),
        ],
        "policies before invariants (source kind), then id, then code"
    );
}

/// Two invariants refusing in either registration order produce the same `Deny`.
///
/// The registry keeps registration order (hand 3) and `run_all` sorts the results; this is the
/// other half -- that the *reasons* built from those results are canonical too, so a deployment
/// that registered its invariants in a different order records the same refusal and therefore the
/// same `proof_digest` (M2-10).
#[test]
fn e_m3_12_registration_order_does_not_reach_the_deny() {
    let build = |first: &'static str, second: &'static str| {
        let registry = InvariantRegistry::new()
            .with(Box::new(Fixed {
                id: first,
                holds: false,
                calls: Arc::new(AtomicUsize::new(0)),
            }))
            .and_then(|r| {
                r.with(Box::new(Fixed {
                    id: second,
                    holds: false,
                    calls: Arc::new(AtomicUsize::new(0)),
                }))
            })
            .expect("two ids");
        let gate = Gate::with_policies(PolicyEngine::parse(PACK).expect("parses"))
            .with_invariants(registry);
        let t = change();
        let pre = object("/tmp/x");
        let planned = PlannedDeltaBytes(Vec::new());
        gate.verify(GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
            decided_at: Timestamp(0),
        })
        .expect("evaluable")
    };

    let forwards = build("aaa-check", "zzz-check");
    let backwards = build("zzz-check", "aaa-check");
    assert_eq!(forwards, backwards);
    assert_eq!(
        forwards.proof_digest().expect("digestible"),
        backwards.proof_digest().expect("digestible"),
        "M2-10's digest is a function of what was refused, not of registration order"
    );
}

// ---------------------------------------------------------------------------
// E-M3-4: the third arm
// ---------------------------------------------------------------------------

/// Nothing refused and no inverse exists → `Escalate` (**E-M3-4**, AC-048's gate half).
#[test]
fn e_m3_4_an_uninvertible_change_that_nothing_refused_escalates() {
    let verdict = quadrant(true, true, false);
    assert_eq!(verdict.kind(), VerdictKind::Escalate);

    let Verdict::Escalate(ticket) = verdict else {
        panic!("the third arm")
    };
    assert_eq!(ticket.transformation, change().id);
    assert_eq!(ticket.reasons.len(), 1, "one reason: there is one trigger");
    assert_eq!(ticket.reasons[0].code(), INVERSE_UNAVAILABLE);
    assert!(
        matches!(ticket.reasons[0].source(), ReasonSource::Adapter { .. }),
        "the adapter is who answered Ok(None) to invert (41 §4, FR-043)"
    );
    assert_eq!(
        ticket.required_approval,
        ApprovalRequirement::default(),
        "ASM-60-3: one approver, nobody excluded, and M5 is who checks it"
    );
}

/// The ticket's id is what the ticket **is** (42 §1.3), not a value someone passed in.
///
/// `IdentityView = {transformation, reasons, required_approval}`, so two escalations of the same
/// change for the same reason are one ticket -- and `created_at`, which ASM-4 keeps out of the
/// view, cannot make them differ. That is what makes the id usable as a key in the engine table 42
/// §3.8 puts the resolution state in.
#[test]
fn e_m3_4_the_ticket_is_identified_by_what_it_is() {
    let Verdict::Escalate(first) = quadrant(true, true, false) else {
        panic!("escalates")
    };
    let Verdict::Escalate(second) = quadrant(true, true, false) else {
        panic!("escalates")
    };
    assert_eq!(first.id, second.id, "the same escalation is one ticket");
    assert_ne!(
        first.id.0,
        Cid([0u8; 32]),
        "the placeholder was replaced by a computed identity"
    );

    let digest = Verdict::Escalate(first.clone())
        .proof_digest()
        .expect("digestible");
    assert_eq!(
        first.id.0, digest,
        "M2-10 digests the ticket, and 42 §1.3 identifies it the same way -- one value, not two"
    );
}

/// **Deny outranks Escalate.** AC-027's first sentence carries no condition.
#[test]
fn ac_027_a_refused_change_is_denied_even_when_it_cannot_be_undone() {
    for (policy_admits, invariant_holds) in [(false, true), (true, false), (false, false)] {
        let verdict = quadrant(policy_admits, invariant_holds, false);
        assert_eq!(
            verdict.kind(),
            VerdictKind::Deny,
            "policy_admits={policy_admits} invariant_holds={invariant_holds}: a refusal is not \
             sent to a human for approval"
        );
    }
}

// ---------------------------------------------------------------------------
// The property (34: "property (exhaustive over the 4 quadrants)") (sem: SEM-gx-gate-377)
// ---------------------------------------------------------------------------

proptest! {
    /// AC-027 as it is written, over the whole product: "if either side includes even one Deny, the
    /// overall Verdict must be `Deny(Vec<Reason>)`. `Admit` only when both are Admit". (sem: SEM-gx-gate-378)
    ///
    /// `invert_available` is held at `true` so that the statement being measured is AC-027's own;
    /// the other value is the subject of the escalation tests above and of the second property
    /// below. Every sample also checks that the number of reasons equals the number of systems
    /// that refused, which is what stops a fold from reporting one refusal twice or losing one.
    #[test]
    fn ac_027_deny_precedence_over_all_four_quadrants(
        policy_admits in any::<bool>(),
        invariant_holds in any::<bool>(),
    ) {
        let verdict = quadrant(policy_admits, invariant_holds, true);
        let refusals = usize::from(!policy_admits) + usize::from(!invariant_holds);

        match &verdict {
            Verdict::Admit(proof) => {
                prop_assert!(policy_admits && invariant_holds, "an Admit with a refusal in it");
                prop_assert_eq!(proof.invariant_results().len(), 1);
            }
            Verdict::Deny(reasons) => {
                prop_assert!(refusals > 0, "a Deny nobody asked for");
                prop_assert_eq!(reasons.len(), refusals, "one reason per refusing system");
            }
            Verdict::Escalate(_) => prop_assert!(
                false,
                "E-M3-4's arm needs invert_available=false, which this property never sets"
            ),
        }
    }

    /// The same product with no inverse available: `Deny` wherever anything refused, `Escalate`
    /// where nothing did (**E-M3-4** meeting AC-027).
    #[test]
    fn e_m3_4_escalation_sits_between_admit_and_deny(
        policy_admits in any::<bool>(),
        invariant_holds in any::<bool>(),
    ) {
        let verdict = quadrant(policy_admits, invariant_holds, false);
        let expected = if policy_admits && invariant_holds {
            VerdictKind::Escalate
        } else {
            VerdictKind::Deny
        };
        prop_assert_eq!(verdict.kind(), expected);
    }

    /// Every verdict this fold produces can be recorded (M2-10), and the three arms are three
    /// digests: an `Admit` and an `Escalate` of the same change do not collide.
    #[test]
    fn every_verdict_has_a_proof_digest(
        policy_admits in any::<bool>(),
        invariant_holds in any::<bool>(),
        invert_available in any::<bool>(),
    ) {
        let verdict = quadrant(policy_admits, invariant_holds, invert_available);
        let digest = verdict.proof_digest();
        prop_assert!(digest.is_ok(), "a verdict that cannot be recorded is a verdict nobody can act on");

        let again = quadrant(policy_admits, invariant_holds, invert_available);
        prop_assert_eq!(digest.unwrap(), again.proof_digest().unwrap(), "the digest is a function of the evaluation");
    }
}
