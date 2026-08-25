// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The shape 41 §4 asks for, and the one answer this hand's gate is able to give.
//!
//! No acceptance criterion is claimed. AC-025..029 are hands 2-5 (34 §E), and every one of them
//! asks a gate to *decide* something. What is checkable now is the boundary: the five fields go in,
//! the three arms exist, `verify` is fallible (**E-M3-3**), and a gate that has been given no
//! policies answers `Err` rather than `Admit`.
//!
//! # Why "answers Err" is a criterion and not a placeholder
//!
//! The failure this file is written against is the one req/29 §4 keeps naming: a gate that returns
//! a permissive default while nothing is wired up looks exactly like a gate that examined the input
//! and approved it. `Gate::unconfigured()` is the state a misconfigured deployment can be in -- an
//! empty `policies/` directory, a `PolicySet` that failed to load -- and it stays `Err` now that
//! hand 2 has given the configured gate something to say instead. The permissive default never
//! exists to be forgotten about.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::verdict::{InvariantResult, PolicyDecisionRecord, Reason, ReasonSource};
use gx_gate::{
    AdmitProof, ApprovalRequirement, Error, EscalationTicket, Gate, GateInput, InvariantRegistry,
    PolicyEngine, TicketId, Verdict,
};
use gx_witness::evidence::{Evidence, PolicyDecision, TestOutcome};

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn tid(seed: u64) -> TransformationId {
    TransformationId(cid(9_000_000 + seed))
}

/// A transformation of the given order, built through gx-core's checked constructor.
fn candidate(order: u8) -> Transformation {
    Transformation::new(
        tid(1),
        order,
        Subject::Object(ObjectId(cid(5))),
        Some(cid(7)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(8)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(9),
            },
            context: ChangeContext::Policy,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("0, 1 and 2 are all at or below MAX_ORDER")
}

fn snapshot() -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(5)),
        SubstrateKind::Fs,
        "/etc/passwd".to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

fn evidence() -> Vec<Evidence> {
    vec![
        Evidence::PolicyEvaluation {
            decision: PolicyDecision::Allow,
            policy_id: "deny-etc".to_string(),
            explanation_digest: Some(cid(11)),
        },
        Evidence::TestResult {
            case: "writes_outside_etc".to_string(),
            suite: "unit".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: None,
            duration_ms: 3,
        },
    ]
}

// ---------------------------------------------------------------------------
// 41 §4's five fields
// ---------------------------------------------------------------------------

/// The five fields bind, with `Evidence` going into the slice slot "as-is" (FR-016). (sem: SEM-gx-gate-265)
///
/// AC-016 already states this for a stand-in in gx-witness -- and that file has to keep its
/// stand-in, since gx-witness naming this crate is the cycle **E-M3-2** exists to avoid. This is
/// the same criterion on the real struct, which is the one an engine will build in M5.
#[test]
fn a_gate_input_takes_the_five_fields_41_4_declares() {
    let t = candidate(0);
    let pre = snapshot();
    let planned = PlannedDeltaBytes(b"opaque-to-everyone-here".to_vec());
    let collected = evidence();

    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
        // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
        // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
        // varying this field alone moves no verdict) about the field and not about this fixture.
        decided_at: Timestamp(0),
    };

    assert_eq!(input.t.order(), 0);
    assert_eq!(input.pre.locator(), "/etc/passwd");
    assert_eq!(input.planned.0.len(), 23);
    assert_eq!(input.evidence.len(), 2);
    assert!(input.invert_available);
}

/// An order-2 transformation goes in through the same struct and the same function.
///
/// AC-029's "there is no order-2-only bypass path" is hand 3's to prove with a call trace (sem: SEM-gx-gate-266). What is
/// visible in hand 1 is the half that makes such a proof possible at all: `order` is a field of the
/// input rather than a parameter of a second entry point, so there is nowhere else to hand one to.
#[test]
fn an_order_2_transformation_uses_the_same_entry_point() {
    let pre = snapshot();
    let planned = PlannedDeltaBytes(Vec::new());
    let gate = Gate::unconfigured();

    for order in [0u8, 1, 2] {
        let t = candidate(order);
        let input = GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: false,
            decided_at: Timestamp(0),
        };
        assert_eq!(input.t.order(), order);
        // One function, three orders, and the same answer for all of them in this hand.
        assert!(matches!(gate.verify(input), Err(Error::Unevaluable { .. })));
    }
}

// ---------------------------------------------------------------------------
// E-M3-3: the error path is not a verdict
// ---------------------------------------------------------------------------

/// A gate with no policies and no invariants refuses to answer, and does not admit.
///
/// E-M3-3 verbatim: "unevaluable (⊥) is neither Deny nor Escalate -- naming 'refused' and 'broken'
/// with the same verdict muddies the semantics of the decision ... fail-closed is realized on the
/// engine side as 'Err -> application forbidden + POLICY_ERROR' (consistent with 43 T-4d)". (sem: SEM-gx-gate-267)
#[test]
fn an_unconfigured_gate_answers_neither_admit_nor_deny_nor_escalate() {
    let t = candidate(0);
    let pre = snapshot();
    let planned = PlannedDeltaBytes(b"x".to_vec());
    let collected = evidence();

    let outcome = Gate::unconfigured().verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
        decided_at: Timestamp(0),
    });

    match outcome {
        Err(Error::Unevaluable { transformation, .. }) => {
            assert_eq!(
                transformation, t.id,
                "the error names the transformation that could not be evaluated"
            );
        }
        other => panic!("an unconfigured gate answered {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 41 §4's three arms
// ---------------------------------------------------------------------------

/// Each arm is constructible and reports the discriminant gx-witness will record (**E-M3-2**).
#[test]
fn the_three_arms_map_onto_the_three_verdict_kinds() {
    let admit = Verdict::Admit(AdmitProof::default());
    let deny = Verdict::deny(vec![reason("POLICY_DENIED")]).expect("one reason is enough");
    let escalate = Verdict::Escalate(ticket());

    assert_eq!(admit.kind(), VerdictKind::Admit);
    assert_eq!(deny.kind(), VerdictKind::Deny);
    assert_eq!(escalate.kind(), VerdictKind::Escalate);

    // The mapping is total: three arms, three kinds, and nothing else to map.
    let kinds: Vec<VerdictKind> = [&admit, &deny, &escalate]
        .iter()
        .map(|v| v.kind())
        .collect();
    assert_eq!(kinds, VerdictKind::ALL.to_vec());
}

/// A `Deny` that names no reason is refused at construction.
///
/// 42 §3.8 types the arm as `Vec<Reason>` and 44 §2.3 expects an operator to be told a code and a
/// message; M2-10 (req/49 §3) additionally makes that vector what `proof_digest` is taken over,
/// and a digest of nothing identifies nothing.
#[test]
fn a_deny_must_carry_a_reason() {
    assert!(matches!(Verdict::deny(Vec::new()), Err(Error::EmptyDeny)));
    assert!(Verdict::deny(vec![reason("INVARIANT_VIOLATED")]).is_ok());
}

fn reason(code: &str) -> Reason {
    Reason::new(
        code,
        "a short human-readable line".to_string(),
        ReasonSource::Policy {
            policy_id: "deny-etc".to_string(),
        },
    )
    .expect("hand 4 declared this code in gx_gate::REASON_CODES")
}

/// The escalation fixture, carrying the code a real escalation carries.
///
/// It said `EVIDENCE_INSUFFICIENT` until **E-7** fired (§37 M5-04 adopted (b)) (sem: SEM-gx-gate-268) and `Reason::new` began
/// refusing the retired word. The replacement is not arbitrary: `INVERSE_UNAVAILABLE` is what
/// `gx_gate::escalate` actually writes, because **E-M3-4** makes "no inverse could be built" the (sem: SEM-gx-gate-269)
/// only escalation rule in v0.1. The fixture was the more speculative of the two spellings, and the
/// retirement is what forced it to become the true one.
fn ticket() -> EscalationTicket {
    EscalationTicket {
        id: TicketId(cid(77)),
        transformation: tid(1),
        reasons: vec![reason("INVERSE_UNAVAILABLE")],
        required_approval: ApprovalRequirement::default(),
        created_at: Timestamp(1_754_600_000_000_000_000),
    }
}

// ---------------------------------------------------------------------------
// ASM-60-3
// ---------------------------------------------------------------------------

/// The default approval requirement is ASM-60-3's, in one place.
///
/// 42 §3.8 defers the meaning to "32 (unconfirmed)" (sem: SEM-gx-gate-270) and 32 says nothing about it (req/60 §3 M3-11
/// measured zero occurrences). An empty `eligible_actors` means **unrestricted**: the other reading
/// would make the default ticket unresolvable, and 43's liveness invariants (AC-045) send an
/// unresolvable escalation to `Aborted(Expired)` and nowhere else.
#[test]
fn the_default_approval_requirement_is_asm_60_3() {
    let default = ApprovalRequirement::default();
    assert_eq!(default.min_approvers, 1);
    assert!(default.eligible_actors.is_empty());
}

/// What a gate holds, and what it does not.
///
/// The third spelling of one assertion, and the reasoning for moving it is written down each time
/// rather than the change being made quietly (52 contract 4: "do not change a test in order to make it pass"). (sem: SEM-gx-gate-271)
///
/// | hand | assertion | why it moved |
/// |---|---|---|
/// | 1 | `size_of::<Gate>() == 0` | "something gave the gate state without giving it the code that uses it" -- hand 2 gave it both at once | (sem: SEM-gx-gate-272)
/// | 2 | `size_of::<Gate>() == size_of::<Option<PolicyEngine>>()` | the same sentence again: hand 3 added the registry **and** the loop that runs it (req/38 §21 C-9 confirmed) |
/// | 3 | the two halves of 41 §4, and nothing else | this |
/// | 5 (M5 hand 2) | the same, with the **number two read out of 41 §4** | **I-11**, and D-11's moment |
///
/// 41 §4 writes the gate as "Cedar PolicySet + Vec<Box<dyn InvariantCheck>>" -- two things (sem: SEM-gx-gate-273). The
/// comparison is against a tuple rather than a sum of two `size_of`s so that padding cannot make it
/// fail for a reason that has nothing to do with the claim: a tuple and a struct are laid out by
/// the same rules, so the equality holds exactly when `Gate` carries those two fields and no third
/// one that occupies space.
///
/// # 🔴 **I-11**: where the number two comes from
///
/// Until this hand the `2` was in the test. req/67 §4's **I-11** is precisely that complaint --
/// "`gate_shape`'s right-hand side is derived from the implementation, so the moment it is updated
/// alongside the implementation it says nothing" (sem: SEM-gx-gate-274) -- and
/// it names the remedy: **B-4's `gate_input_spec.rs` precedent**, which parses 41 §4's markdown and
/// compares the field list with the real type. req/38 §26 reserved it as "the required DoD for
/// M4/M5's 'hand that gives the gate a third field'" (sem: SEM-gx-gate-275), and §37/§38 fire it here because E-7's retirement makes this the hand with
/// a gx-gate diff.
///
/// So [`halves_declared_by_41_4`] reads the line out of the spec file and counts the summands. The
/// assertion now falls in **two** directions instead of one: a third field in `Gate` (the original
/// claim) and a third summand in 41 §4 (a spec change nobody transcribed). A hand that edits both
/// together is a hand that meant to.
///
/// # 🔴 **D-11**: the moment it named, and what actually arrived
///
/// req/63 §4's **D-11** recorded that this assertion had been rewritten twice in two hands and
/// asked for the next rewrite to be argued: "the next time this falls is when M4/M5 gives the gate a
/// third field = **the moment to argue it**" (req/38 §22 confirmed). M4 did not touch gx-gate at all
/// (req/72 §-- "gate diff 0 lines"), so this is the first hand with the diff, and the argument owed is this paragraph. (sem: SEM-gx-gate-276)
///
/// **A third half did not arrive.** What arrived is the opposite: a *removal* from a vocabulary
/// (`REASON_CODES` 4 → 3, E-7), which changes no field of `Gate` and no field of `GateInput`. The
/// measurement is right here -- the equality below still holds against the same two halves, and
/// `gate_input_spec.rs` still finds five fields. D-11's moment is therefore **held and closed
/// without a rewrite of the claim**: the only thing that changed is where the number lives, which
/// is I-11's ask and makes the assertion stronger rather than looser. The next hand to add a third
/// half still owes the argument D-11 asked for; nothing here spends it.
#[test]
fn the_gate_holds_the_two_halves_41_4_names_and_nothing_else() {
    let halves = halves_declared_by_41_4();
    println!("HALVES_DECLARED_BY_41_4={halves:?} ({})", halves.len());
    assert_eq!(
        halves.len(),
        2,
        "41 §4 no longer writes the gate as two things; the type below was built against {halves:?}"
    );
    assert!(
        halves[0].contains("PolicySet") && halves[1].contains("InvariantCheck"),
        "41 §4's two summands are the policy set and the invariant list, in that order: {halves:?}"
    );

    assert_eq!(
        std::mem::size_of::<Gate>(),
        std::mem::size_of::<(Option<PolicyEngine>, InvariantRegistry)>(),
        "`Gate` carries a field 41 §4 does not name (D-11: this is the moment to argue for it)"
    );

    let unconfigured = Gate::unconfigured();
    assert!(unconfigured.policies().is_none());
    assert!(
        unconfigured.invariants().is_empty(),
        "an unconfigured gate holds an empty registry, which is \"nobody was asked\" rather than \
         \"everything held\"" // (sem: SEM-gx-gate-277)
    );
}

/// The summands of 41 §4's `pub struct Gate` line, read out of the spec file (**I-11**).
///
/// The line is `pub struct Gate { /* Cedar PolicySet + Vec<Box<dyn InvariantCheck>> */ }`, and the
/// halves are what the comment adds together. Parsing a comment rather than a field list is forced
/// by the spec: 41 §4 declares this one type with its contents *elided*, which is itself the fact
/// that makes `size_of` the only available check on the Rust side. One reader, one source of the
/// number -- the shape `gate_input_spec.rs` established for the five fields of `GateInput`.
fn halves_declared_by_41_4() -> Vec<String> {
    let path = workspace_root().join("req/spec/40-architecture/41-architecture.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    // `pub struct Gate` is a prefix of `pub struct GateInput`, which 41 §4 declares eleven lines
    // earlier -- so the needle carries the brace. The first draft of this helper did not, matched
    // `GateInput<'a> {`, and failed looking for a comment that is not on that line: §30's lesson
    // (a scan that silently measures the wrong thing) arriving in the instrument written to answer
    // it. It failed loudly here only because the elision comment is what it went on to read.
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("pub struct Gate {"))
        .expect("41 §4 still declares `pub struct Gate {`");
    let body = line
        .split_once("/*")
        .and_then(|(_, rest)| rest.split_once("*/"))
        .expect("41 §4 writes the gate's contents as a comment")
        .0;
    body.split('+')
        .map(|half| half.trim().to_string())
        .collect()
}

/// `<root>` from this crate's manifest directory, for the spec reads above.
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

// ---------------------------------------------------------------------------
// The vessels of 42 §3.8, filled by nobody in this hand
// ---------------------------------------------------------------------------

/// `PolicyDecisionRecord` uses gx-witness's `PolicyDecision`, which is Cedar's own vocabulary.
///
/// 42 §3.7 verbatim: "the same vocabulary as `cedar_policy::Decision`". One vocabulary defined once means a (sem: SEM-gx-gate-278)
/// decision crossing the boundary in hand 2 needs no translation table -- the place a mapping table
/// would silently invert `Allow` and `Deny`.
#[test]
fn a_policy_decision_record_carries_cedars_two_words() {
    let allowed = PolicyDecisionRecord {
        policy_id: "allow-tmp".to_string(),
        decision: PolicyDecision::Allow,
        diagnostics_digest: None,
    };
    let denied = PolicyDecisionRecord {
        policy_id: "deny-etc".to_string(),
        decision: PolicyDecision::Deny,
        diagnostics_digest: Some(cid(3)),
    };

    assert_ne!(allowed.decision, denied.decision);
    assert_eq!(denied.diagnostics_digest, Some(cid(3)));
}

/// `InvariantResult.invariant_id` is the id hand 3's trait will answer with.
#[test]
fn an_invariant_result_carries_an_id_and_a_verdict_of_its_own() {
    let held = InvariantResult::new("line-count-preserved".to_string(), true, None)
        .expect("a detail of None is inside every bound");
    let broken = InvariantResult::new(
        "line-count-preserved".to_string(),
        false,
        Some("3 lines became 0".to_string()),
    )
    .expect("a sixteen-byte detail is inside ASM-60-4's bound");

    assert_eq!(held.invariant_id(), broken.invariant_id());
    assert!(held.holds() && !broken.holds());
}
