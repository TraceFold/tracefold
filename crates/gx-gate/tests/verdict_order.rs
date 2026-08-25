// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-M3-9**: the order every vector of an `AdmitProof` is put into, and the one that is left alone.
//!
//! 42 §1.3 lists `AdmitProof` as "every field" in the IdentityView table, so the order of its four (sem: SEM-gx-gate-379)
//! vectors reaches the CID a receipt records. 42 fixes no order. req/38 §19 does:
//!
//! > "M3-20 (adopted + separated): Vec canonicalization = ascending id (policy_id/invariant_id
//! > lexicographic; evidence_digests ascending CID byte order). **Only `composed_from` keeps
//! > composition order** (adopting the lane's point that order carries meaning for T5's
//! > verdict-chain reconstruction -- it must not be folded into ascending)" -> **E-M3-9** (sem: SEM-gx-gate-380)
//!
//! # What this file proves and what hand 4 owes
//!
//! req/60 §5.2 assigns hand 1 "the implementation is enforced by the constructor, tested in hand 4" (sem: SEM-gx-gate-381). So the tests here are of the
//! constructor's behaviour; what hand 4 adds is the harder half -- that the constructor cannot be
//! walked around once `AdmitProof` grows a wire face, since a `Deserialize` derive would be a
//! second door into the struct that skips this ordering entirely. Until then the fields are
//! private and there is one door.

use gx_core::{Cid, TransformationId};
use gx_gate::verdict::{InvariantResult, PolicyDecisionRecord};
use gx_gate::AdmitProof;
use gx_witness::PolicyDecision;

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn tid(seed: u8) -> TransformationId {
    TransformationId(cid(seed))
}

fn policy(id: &str) -> PolicyDecisionRecord {
    PolicyDecisionRecord {
        policy_id: id.to_string(),
        decision: PolicyDecision::Allow,
        diagnostics_digest: None,
    }
}

fn invariant(id: &str) -> InvariantResult {
    InvariantResult::new(id.to_string(), true, None).expect("a short detail and no code")
}

/// The three unordered vectors come out in ascending id order, whatever order they went in.
#[test]
fn the_three_id_ordered_vectors_are_sorted_on_construction() {
    let proof = AdmitProof::new(
        vec![policy("zeta"), policy("alpha"), policy("mid")],
        vec![invariant("z-check"), invariant("a-check")],
        vec![cid(9), cid(1), cid(5)],
        Vec::new(),
        None,
    );

    let policies: Vec<&str> = proof
        .policy_decisions()
        .iter()
        .map(|p| p.policy_id.as_str())
        .collect();
    assert_eq!(policies, ["alpha", "mid", "zeta"]);

    let invariants: Vec<&str> = proof
        .invariant_results()
        .iter()
        .map(InvariantResult::invariant_id)
        .collect();
    assert_eq!(invariants, ["a-check", "z-check"]);

    assert_eq!(proof.evidence_digests(), [cid(1), cid(5), cid(9)]);
}

/// Two evaluations that differ only in the order things were collected produce the same proof.
///
/// This is the property the CID needs and the reason the rule exists: an `AdmitProof` is hashed
/// through its IdentityView, so a registry that iterated in a different order on a different day
/// would produce a different digest for the same decision.
#[test]
fn collection_order_does_not_reach_the_value() {
    let one = AdmitProof::new(
        vec![policy("b"), policy("a")],
        vec![invariant("y"), invariant("x")],
        vec![cid(2), cid(1)],
        Vec::new(),
        None,
    );
    let other = AdmitProof::new(
        vec![policy("a"), policy("b")],
        vec![invariant("x"), invariant("y")],
        vec![cid(1), cid(2)],
        Vec::new(),
        None,
    );

    assert_eq!(one, other);
}

/// `composed_from` keeps the order it was given (**the separated half of E-M3-9**).
///
/// T5 asks for the chain of witnesses behind a composed transformation to be recoverable, and a
/// chain is an ordered thing: `g ∘ f` is not `f ∘ g`. Sorting this vector would destroy exactly the
/// information 42 §3.8 says it carries ("the source referenced when lower `AdmitProof`s were laxly composed") (sem: SEM-gx-gate-382). The two
/// cases below are the same set in two orders, and they must **not** be equal.
#[test]
fn composed_from_keeps_composition_order() {
    let forward = AdmitProof::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![tid(3), tid(1)],
        None,
    );
    let backward = AdmitProof::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![tid(1), tid(3)],
        None,
    );

    assert_eq!(
        forward.composed_from(),
        [tid(3), tid(1)],
        "an ascending sort would have moved tid(1) first and lost the composition order"
    );
    assert_ne!(
        forward, backward,
        "two different compositions of the same two transformations are two different proofs (T5)"
    );
}

/// Duplicates survive (H4-3: "multiplicity is not folded; only order is canonicalized"). (sem: SEM-gx-gate-383)
///
/// Two policies that both answered, or one evidence item cited twice, are facts about the
/// evaluation that happened. A constructor that deduplicated would make the proof describe a
/// different evaluation, which is the same reasoning H4-3 applied to `Provenance.input_objects`.
#[test]
fn multiplicity_is_not_folded_away() {
    let proof = AdmitProof::new(
        vec![policy("a"), policy("a")],
        vec![invariant("x"), invariant("x")],
        vec![cid(1), cid(1)],
        vec![tid(2), tid(2)],
        None,
    );

    assert_eq!(proof.policy_decisions().len(), 2);
    assert_eq!(proof.invariant_results().len(), 2);
    assert_eq!(proof.evidence_digests().len(), 2);
    assert_eq!(proof.composed_from().len(), 2);
}

/// `proof_ref` is gx-core's type, carried and not redefined (**E-M3-8**).
///
/// 42 §0 files `ProofRef` under this crate's `verdict.rs`; E-M2-12 had already implemented it in
/// gx-core, and E-M3-8 makes the table the erratum rather than growing a second type with the same
/// name in the crate that would import the first.
#[test]
fn the_proof_reference_is_the_one_gx_core_already_has() {
    let reference = gx_core::ProofRef {
        lean_spec_version: "GxSpec-0.1".to_string(),
        theorem_ids: vec![gx_core::TheoremId::T5],
    };
    let proof = AdmitProof::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Some(reference.clone()),
    );

    assert_eq!(proof.proof_ref(), Some(&reference));
    assert_eq!(
        AdmitProof::default().proof_ref(),
        None,
        "42 §3.8 types it `Option<ProofRef>`: a verdict with no Lean correspondence is the usual case"
    );
}
