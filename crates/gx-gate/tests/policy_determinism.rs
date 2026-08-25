// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The negative half of hand 2 (req/60 §5.2): a decision does not wobble, and everything that is
//! not a decision leaves through `Err`.
//!
//! req/60 §5.2, hand 2's DoD: "**negative case**: the same policy + the same request does not waver
//! in its decision (determinism), as a property" (sem: SEM-gx-gate-293). What that property is worth depends on what could make it fail, and the answer is
//! not Cedar -- 46 §1 puts Cedar's own semantics outside this project, and re-testing them here
//! would be writing upstream's suite (52 / 05 §4 R-4). It is gx's own mapping: a `HashMap` iterated
//! into a context, a `HashSet` of diagnostics read in whatever order it yields, a policy id taken
//! from a position in a file. Each of those is a place where the same question could get two
//! answers, and the property is what stands over them.
//!
//! The rest of the file is the ⊥ of **E-M3-3** one step earlier than `Unevaluable`: a policy set
//! that cannot be read has decided nothing, and every way of not being readable is named.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId,
};
use gx_gate::{Error, Gate, GateInput, PolicyEngine};
use gx_witness::{Evidence, PolicyDecision, TestOutcome};
use proptest::prelude::*;

/// The pack, read at run time rather than embedded.
///
/// `include_str!` would be a second road into the file, and FR-028's "there is exactly one road by
/// which a pack's file gets embedded into the build" (sem: SEM-gx-gate-294) is hand 5's check to make. Today the count of embedded paths is zero,
/// and a test that quietly made it one would be the thing that check is for.
fn pack_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-gate")
        .join("policies")
        .join("fs")
        .join("deny-etc.cedar");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn change(order: u8, context: ChangeContext, key: &str) -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        order,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context,
            actor: Actor::Agent {
                key: key.to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("orders 0..=2 are below MAX_ORDER")
}

fn object(substrate: SubstrateKind, locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        substrate,
        locator.to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

fn any_substrate() -> impl Strategy<Value = SubstrateKind> {
    prop_oneof![
        Just(SubstrateKind::Fs),
        Just(SubstrateKind::Git),
        Just(SubstrateKind::Mcp),
        "[a-z]{1,6}".prop_map(SubstrateKind::Custom),
    ]
}

fn any_context() -> impl Strategy<Value = ChangeContext> {
    prop_oneof![
        Just(ChangeContext::Time),
        Just(ChangeContext::Evidence),
        Just(ChangeContext::Policy),
        Just(ChangeContext::Model),
        Just(ChangeContext::Representation),
        Just(ChangeContext::Substrate),
        "[a-zA-Z ]{0,8}".prop_map(ChangeContext::Custom),
    ]
}

/// Paths that reach both sides of the pattern, plus arbitrary ones. `/etc/*` is the boundary
/// AC-025 sits on, so half the sample is deliberately near it.
fn any_locator() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("/etc".to_string()),
        "/etc/[a-z]{1,8}".prop_map(String::from),
        "/etcetera/[a-z]{1,8}".prop_map(String::from),
        "/tmp/[a-z]{1,8}".prop_map(String::from),
        "[ -~]{0,24}".prop_map(String::from),
    ]
}

fn any_evidence() -> impl Strategy<Value = Vec<Evidence>> {
    proptest::collection::vec(
        prop_oneof![
            Just(Evidence::TestResult {
                case: "c".to_string(),
                suite: "s".to_string(),
                outcome: TestOutcome::Pass,
                log_digest: None,
                duration_ms: 1,
            }),
            Just(Evidence::PolicyEvaluation {
                decision: PolicyDecision::Allow,
                policy_id: "elsewhere".to_string(),
                explanation_digest: None,
            }),
        ],
        0..4,
    )
}

proptest! {
    /// One policy set, one request, one answer -- across two evaluations and two independently
    /// parsed engines.
    ///
    /// The second engine is the half that matters. A single engine could be deterministic because
    /// it froze a hash order at construction; parsing the same source twice and getting the same
    /// answer is what says the id a decision is recorded under does not depend on which parse it
    /// came from. 42 §1.3 puts that id inside `AdmitProof`'s IdentityView, so a wobble here would
    /// be a receipt whose CID depends on when the process started.
    #[test]
    fn the_answer_is_a_function_of_the_policy_set_and_the_request(
        key in "[a-z][a-z0-9-]{0,7}",
        order in 0u8..=2,
        context in any_context(),
        substrate in any_substrate(),
        locator in any_locator(),
        invert_available in any::<bool>(),
        evidence in any_evidence(),
    ) {
        let first = PolicyEngine::parse(&pack_source()).expect("the pack parses");
        let second = PolicyEngine::parse(&pack_source()).expect("the pack parses, again");

        let t = change(order, context, &key);
        let pre = object(substrate, &locator);
        let planned = PlannedDeltaBytes(Vec::new());
        let input = || GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &evidence,
            invert_available,
            // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
            // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
            // varying this field alone moves no verdict) about the field and not about this fixture.
            decided_at: Timestamp(0),
        };

        let a = first.evaluate(&input()).expect("evaluable");
        let b = first.evaluate(&input()).expect("evaluable");
        let c = second.evaluate(&input()).expect("evaluable");

        prop_assert_eq!(&a, &b, "the same engine answered twice and differed");
        prop_assert_eq!(&a, &c, "two parses of one source answered differently");

        // Sorted, and asserted rather than assumed: the diagnostics iterator's order is not part
        // of Cedar's contract, and this vector reaches a CID.
        let ids: Vec<&str> = a.determining().iter().map(|r| r.policy_id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        prop_assert_eq!(ids, sorted, "the deciding policies are recorded in id order");
    }

    /// The verdict follows the decision and the inverse flag, on every sample.
    ///
    /// Not a claim about AC-027 -- there are no invariants to meet here; that property is
    /// `tests/verdict_meet.rs`. It is the narrower fact that `Gate::verify` maps one system's
    /// answer onto the arm that means it.
    ///
    /// # Rewritten by hand 4, and why that is not "changing a test in order to make it pass" (sem: SEM-gx-gate-295)
    ///
    /// Hand 2 wrote this with two arms and a sentence saying the third was unreachable: "**E-M3-4** (sem: SEM-gx-gate-296)
    /// makes `invert_available=false → Escalate` a hand 4 rule, so an `Escalate` appearing here
    /// would be an arm nobody wrote". Hand 4 wrote it, so the test fell -- **at the moment it was
    /// supposed to fall**, which is the shape req/38 §21's C-9 and §22's D-11 already confirmed for (sem: SEM-gx-gate-297)
    /// `gate_shape`: the assertion's own documentation named the event that would invalidate it,
    /// the event happened, and the claim was moved forward rather than deleted. What it says now is
    /// strictly more: the arm is a function of the decision **and** the flag, so an implementation
    /// that escalated a refused change (or admitted an uninvertible one) fails here as well as in
    /// `verdict_meet`.
    #[test]
    fn the_verdict_arm_follows_the_policy_decision(
        order in 0u8..=2,
        locator in any_locator(),
        invert_available in any::<bool>(),
    ) {
        let engine = PolicyEngine::parse(&pack_source()).expect("the pack parses");
        let gate = Gate::with_policies(PolicyEngine::parse(&pack_source()).expect("parses"));

        let t = change(order, ChangeContext::Substrate, "key-1");
        let pre = object(SubstrateKind::Fs, &locator);
        let planned = PlannedDeltaBytes(Vec::new());
        let input = || GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available,
            decided_at: Timestamp(0),
        };

        let decision = engine.evaluate(&input()).expect("evaluable").decision();
        let verdict = gate.verify(input()).expect("evaluable");

        match (decision, invert_available, &verdict) {
            (PolicyDecision::Allow, true, gx_gate::Verdict::Admit(_)) => {}
            // **E-M3-4**: nothing refused, and no inverse exists.
            (PolicyDecision::Allow, false, gx_gate::Verdict::Escalate(ticket)) => {
                prop_assert_eq!(ticket.transformation, t.id);
                prop_assert_eq!(ticket.reasons.len(), 1);
                prop_assert_eq!(ticket.reasons[0].code(), gx_gate::INVERSE_UNAVAILABLE);
            }
            // Deny outranks Escalate: AC-027's first sentence carries no condition.
            (PolicyDecision::Deny, _, gx_gate::Verdict::Deny(reasons)) => {
                prop_assert!(!reasons.is_empty(), "42 §3.8 gives a Deny reasons");
            }
            (d, i, v) => prop_assert!(false, "{d:?} with invert_available={i} became {v:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// ⊥ one step earlier: a policy set that cannot be read
// ---------------------------------------------------------------------------

#[test]
fn a_source_cedar_refuses_is_not_a_policy_set() {
    let err = PolicyEngine::parse("permit (principal, action, resource)").unwrap_err();
    assert!(
        matches!(err, Error::PolicySetUnreadable { .. }),
        "a missing semicolon is not a decision about anything: {err:?}"
    );
}

/// Without `@id`, a policy's id is its position, and a position reaches a receipt's CID.
#[test]
fn a_policy_without_an_id_annotation_is_refused() {
    let err = PolicyEngine::parse("permit (principal, action, resource);").unwrap_err();
    match err {
        Error::PolicySetUnreadable { detail } => assert!(
            detail.contains("@id"),
            "the refusal names what is missing: {detail}"
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_empty_id_annotation_is_refused() {
    let err = PolicyEngine::parse("@id(\"\") permit (principal, action, resource);").unwrap_err();
    assert!(matches!(err, Error::PolicySetUnreadable { .. }));
}

#[test]
fn two_policies_claiming_one_id_are_refused() {
    let err = PolicyEngine::parse(
        r#"
        @id("same") permit (principal, action, resource);
        @id("same") forbid (principal, action, resource);
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(err, Error::PolicySetUnreadable { .. }),
        "one id must name one policy, or a record names two things"
    );
}

/// Hand 2's fourth refusal is gone, and nothing took its place (**E-M3-11**).
///
/// It refused a policy named `cedar:default-deny` -- the sentinel that stood for "no policy
/// decided" (sem: SEM-gx-gate-298) -- so that a pack could not impersonate the absence of a decision. C-3 replaced the
/// sentinel with `ReasonSource::NoPolicyApplied` and required the spelling to go in the same hand,
/// which leaves nothing to impersonate. This test is the mechanical form of that: the name parses
/// like any other, and the crate holds no reserved policy id at all.
#[test]
fn no_policy_id_is_reserved_any_more() {
    let engine =
        PolicyEngine::parse("@id(\"cedar:default-deny\") permit (principal, action, resource);")
            .expect("the sentinel is gone, so the name is ordinary");
    assert_eq!(engine.policy_ids(), vec!["cedar:default-deny".to_string()]);
}

/// A template would be dropped silently by rebuilding the set from its policies.
#[test]
fn a_template_is_refused_rather_than_dropped() {
    let err = PolicyEngine::parse(
        r#"
        @id("a-template")
        permit (principal == ?principal, action, resource);
        "#,
    )
    .unwrap_err();
    match err {
        Error::PolicySetUnreadable { detail } => assert!(detail.contains("template"), "{detail}"),
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The two empties, which are not the same empty
// ---------------------------------------------------------------------------

/// A set with no policies decides: Cedar's default-deny answers every request against it.
///
/// "default deny: no request is authorized (decision Allow) unless there is a specific permit
/// policy that grants it" (docs.cedarpolicy.com/auth/authorization.html). (sem: SEM-gx-gate-299)
#[test]
fn an_empty_policy_set_denies_and_says_no_policy_did_it() {
    let engine = PolicyEngine::parse("").expect("an empty source is an empty set");
    assert!(engine.is_empty());
    assert_eq!(engine.len(), 0);

    let t = change(0, ChangeContext::Substrate, "key-1");
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());
    let outcome = engine
        .evaluate(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
            decided_at: Timestamp(0),
        })
        .expect("evaluable");

    assert_eq!(outcome.decision(), PolicyDecision::Deny);
    assert!(outcome.determining().is_empty());
}

/// A gate that was never given a set does not decide at all -- hand 1's answer, unchanged.
///
/// The distinction is the point: "the operator loaded a set that permits nothing" and "nobody
/// wired a set up" are different facts, and only the first one is a verdict. **E-M3-3**. (sem: SEM-gx-gate-300)
#[test]
fn a_gate_with_no_policy_set_answers_neither_admit_nor_deny() {
    let t = change(0, ChangeContext::Substrate, "key-1");
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());
    let gate = Gate::unconfigured();

    assert!(gate.policies().is_none());
    let outcome = gate.verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
        decided_at: Timestamp(0),
    });
    assert!(
        matches!(outcome, Err(Error::Unevaluable { .. })),
        "{outcome:?}"
    );
}
