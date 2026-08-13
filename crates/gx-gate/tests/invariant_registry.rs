//! The registry's own properties: the order it is built in cannot reach the answer, and a check
//! that cannot decide is not a refusal.
//!
//! req/60 §5.2 asks this hand for one statement in particular -- 「registry の**順序が結果に効かない**
//! 事(または効く事)を明示」 -- and the answer is that it does not. This file is where that is
//! measured rather than asserted in prose, because it is not free: it holds only while three
//! separate things are true, and each of them is one test below.
//!
//! | what holds it up | test |
//! |---|---|
//! | every check is called exactly once, so a permutation changes only *when* | `ac_026.rs` |
//! | the answers come back in `invariant_id` order (**E-M3-9**), not registration order | `the_answers_come_back_in_invariant_id_order` |
//! | two checks may not claim one id, so that order is total | `a_second_invariant_may_not_claim_an_id` |
//! | a failure reports the same way from either direction | `two_failing_invariants_report_the_same_way_in_either_order` |
//!
//! The third row is the one that is easy to miss. A stable sort orders equal keys by arrival, so a
//! duplicate `invariant_id` would put registration order back inside `AdmitProof`'s canonical bytes
//! -- and 42 §1.3 sends those into a receipt's CID. It is the same defect hand 2 refused in
//! `@id`-less policies (`req/62_M3_HAND2_REPORT_2026-08-08.md` §3.7), arrived at from the other
//! half of 41 §4.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId,
};
use gx_gate::verdict::InvariantResult;
use gx_gate::{
    Error, Gate, GateInput, InvariantCheck, InvariantRegistry, PolicyEngine, Result, Verdict,
};

const PERMIT_EVERYTHING: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

// ---------------------------------------------------------------------------
// Doubles
// ---------------------------------------------------------------------------

/// Answers under its own id, holding or not, counting the calls.
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
        InvariantResult::new(self.id.to_string(), self.holds, None)
    }
}

fn fixed(id: &'static str, holds: bool) -> Box<dyn InvariantCheck> {
    Box::new(Fixed {
        id,
        holds,
        calls: Arc::new(AtomicUsize::new(0)),
    })
}

/// Cannot decide. The ⊥ of **E-M3-3**, one layer in from `Gate::verify`.
struct Undecidable {
    id: &'static str,
    calls: Arc<AtomicUsize>,
}

impl InvariantCheck for Undecidable {
    fn id(&self) -> &str {
        self.id
    }

    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(Error::Unevaluable {
            transformation: input.t.id,
            detail: format!("{} needs something it was not given", self.id),
        })
    }
}

/// Answers under somebody else's id. 42 §3.8 says `invariant_id` 「= `InvariantCheck::id()`」, and
/// this is what breaking that looks like from the registry's side.
struct Misattributing;

impl InvariantCheck for Misattributing {
    fn id(&self) -> &str {
        "honest-name"
    }

    fn check(&self, _input: &GateInput<'_>) -> Result<InvariantResult> {
        InvariantResult::new("somebody-elses-name".to_string(), true, None)
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn change() -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        1,
        Subject::Transformation(TransformationId(cid(9))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: ChangeContext::Policy,
            actor: Actor::Human {
                key: "key-human-1".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 1 is below MAX_ORDER")
}

fn object() -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        SubstrateKind::Fs,
        "/tmp/x".to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

fn permissive_gate(registry: InvariantRegistry) -> Gate {
    Gate::with_policies(PolicyEngine::parse(PERMIT_EVERYTHING).expect("parses"))
        .with_invariants(registry)
}

/// One verdict, from a gate holding the checks the builder produces in the given order.
fn verdict_of(order: &[usize], checks: &[fn() -> Box<dyn InvariantCheck>]) -> Result<Verdict> {
    let mut registry = InvariantRegistry::new();
    for index in order {
        registry
            .register(checks[*index]())
            .expect("the fixtures have distinct ids");
    }
    let gate = permissive_gate(registry);
    let t = change();
    let pre = object();
    let planned = PlannedDeltaBytes(Vec::new());
    gate.verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    })
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Two invariants may not answer under one name.
#[test]
fn a_second_invariant_may_not_claim_an_id() {
    let mut registry = InvariantRegistry::new();
    registry
        .register(fixed("line-count-preserved", true))
        .expect("the first is fine");

    let err = registry
        .register(fixed("line-count-preserved", false))
        .expect_err("the second claims a name that is taken");

    match err {
        Error::RegistryUnusable { detail } => {
            assert!(
                detail.contains("line-count-preserved"),
                "the refusal names the id: {detail:?}"
            );
            assert!(
                detail.contains("E-M3-9"),
                "and says which rule it is protecting: {detail:?}"
            );
        }
        other => panic!("a duplicate id is a configuration fault, not {other:?}"),
    }
    assert_eq!(
        registry.len(),
        1,
        "the refused registration left nothing behind"
    );
}

/// An invariant with no name cannot be recorded, so it is not registered.
#[test]
fn an_invariant_without_an_id_is_refused() {
    let mut registry = InvariantRegistry::new();
    let err = registry
        .register(fixed("", true))
        .expect_err("the empty id names nothing in a receipt");
    assert!(matches!(err, Error::RegistryUnusable { .. }));
    assert!(registry.is_empty());
}

// ---------------------------------------------------------------------------
// Order
// ---------------------------------------------------------------------------

/// The answers arrive in `invariant_id` order however the registry was built (**E-M3-9**).
#[test]
fn the_answers_come_back_in_invariant_id_order() {
    let registry = InvariantRegistry::new()
        .with(fixed("zzz-last", true))
        .and_then(|r| r.with(fixed("mmm-middle", true)))
        .and_then(|r| r.with(fixed("aaa-first", true)))
        .expect("three distinct ids");
    assert_eq!(
        registry.ids(),
        vec!["zzz-last", "mmm-middle", "aaa-first"],
        "registration order is what the registry reports, because it is the call order"
    );

    let t = change();
    let pre = object();
    let planned = PlannedDeltaBytes(Vec::new());
    let results = registry
        .run_all(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        })
        .expect("three holding invariants");

    let ids: Vec<&str> = results.iter().map(InvariantResult::invariant_id).collect();
    assert_eq!(
        ids,
        vec!["aaa-first", "mmm-middle", "zzz-last"],
        "the answers are canonicalised, and the sort is not the caller's to remember"
    );
}

/// All six permutations of three invariants produce one verdict.
///
/// One of the three refuses, so the verdict is a `Deny` whose reason vector is where an
/// order-dependence would show. `AdmitProof` is covered by the all-holding case below.
#[test]
fn registration_order_does_not_reach_the_verdict() {
    let checks: [fn() -> Box<dyn InvariantCheck>; 3] = [
        || fixed("aaa-first", true),
        || fixed("bbb-second", false),
        || fixed("ccc-third", true),
    ];
    let permutations = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    let first = verdict_of(&permutations[0], &checks).expect("evaluable");
    assert!(
        matches!(first, Verdict::Deny(_)),
        "one invariant refused, so the fold refuses"
    );
    for permutation in &permutations[1..] {
        let other = verdict_of(permutation, &checks).expect("evaluable");
        assert_eq!(
            first, other,
            "the verdict changed with the registration order {permutation:?}"
        );
    }
}

/// The same, when everything holds: the `AdmitProof` is one value whichever way it was built.
#[test]
fn registration_order_does_not_reach_the_admit_proof() {
    let checks: [fn() -> Box<dyn InvariantCheck>; 3] = [
        || fixed("aaa-first", true),
        || fixed("bbb-second", true),
        || fixed("ccc-third", true),
    ];

    let forwards = verdict_of(&[0, 1, 2], &checks).expect("evaluable");
    let backwards = verdict_of(&[2, 1, 0], &checks).expect("evaluable");
    assert!(matches!(forwards, Verdict::Admit(_)));
    assert_eq!(
        forwards, backwards,
        "an AdmitProof reaches a receipt's CID (42 §1.3); two build orders must not be two proofs"
    );
}

// ---------------------------------------------------------------------------
// ⊥: a check that cannot decide
// ---------------------------------------------------------------------------

/// An invariant that returns `Err` makes the verdict unreachable, not negative.
///
/// **E-M3-3** 逐語: 「評価不能(⊥)は Deny でも Escalate でもない」. The other invariants still ran --
/// FR-026 is unconditional -- and the error names who could not answer.
#[test]
fn an_invariant_that_cannot_decide_is_not_a_deny() {
    let ran = Arc::new(AtomicUsize::new(0));
    let asked = Arc::new(AtomicUsize::new(0));
    let registry = InvariantRegistry::new()
        .with(Box::new(Undecidable {
            id: "needs-a-store",
            calls: Arc::clone(&asked),
        }))
        .and_then(|r| {
            r.with(Box::new(Fixed {
                id: "holds-anyway",
                holds: true,
                calls: Arc::clone(&ran),
            }))
        })
        .expect("two distinct ids");

    let gate = permissive_gate(registry);
    let t = change();
    let pre = object();
    let planned = PlannedDeltaBytes(Vec::new());
    let outcome = gate.verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    });

    match outcome {
        Err(Error::Unevaluable {
            transformation,
            detail,
        }) => {
            assert_eq!(transformation, t.id);
            assert!(
                detail.contains("needs-a-store"),
                "the ⊥ names the invariant that could not answer: {detail:?}"
            );
        }
        other => panic!("an undecidable invariant produced {other:?}"),
    }
    assert_eq!(
        asked.load(Ordering::SeqCst),
        1,
        "the failing invariant was asked once"
    );
    assert_eq!(
        ran.load(Ordering::SeqCst),
        1,
        "and the other one still ran: stopping early would make the error depend on the order"
    );
}

/// Two failures report identically from either direction.
///
/// This is the half of the order-independence claim that lives on the error path. A registry that
/// returned the first failure it met would answer differently depending on how it was built, and
/// an operator reading two different messages for one broken deployment would chase the wrong one.
#[test]
fn two_failing_invariants_report_the_same_way_in_either_order() {
    let build = |ids: [&'static str; 2]| {
        let mut registry = InvariantRegistry::new();
        for id in ids {
            registry
                .register(Box::new(Undecidable {
                    id,
                    calls: Arc::new(AtomicUsize::new(0)),
                }))
                .expect("distinct ids");
        }
        let gate = permissive_gate(registry);
        let t = change();
        let pre = object();
        let planned = PlannedDeltaBytes(Vec::new());
        gate.verify(GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        })
        .expect_err("neither can decide")
    };

    assert_eq!(
        build(["aaa-first", "zzz-last"]),
        build(["zzz-last", "aaa-first"])
    );
}

/// A check that answers under another name is a record nobody can trust.
#[test]
fn a_result_that_misnames_its_invariant_is_refused() {
    let registry = InvariantRegistry::new()
        .with(Box::new(Misattributing))
        .expect("one id");
    let gate = permissive_gate(registry);
    let t = change();
    let pre = object();
    let planned = PlannedDeltaBytes(Vec::new());

    let outcome = gate.verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    });

    match outcome {
        Err(Error::Unevaluable { detail, .. }) => {
            assert!(
                detail.contains("honest-name") && detail.contains("somebody-elses-name"),
                "both names are in the message, because either could be the wrong one: {detail:?}"
            );
        }
        other => panic!("42 §3.8 requires invariant_id == id(), and this produced {other:?}"),
    }
}
