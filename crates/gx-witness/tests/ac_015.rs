// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-015 (FR-015) — `Provenance::derive_from` is a deterministic derivation. (sem:
//! SEM-gx-witness-193, SEM-gx-witness-194, SEM-gx-witness-195, SEM-gx-witness-196,
//! SEM-gx-witness-197, SEM-gx-witness-198, SEM-gx-witness-199, SEM-gx-witness-200,
//! SEM-gx-witness-201, SEM-gx-witness-202, SEM-gx-witness-203, SEM-gx-witness-204,
//! SEM-gx-witness-205)
//!
//! AC-015 verbatim: "Given: an existing Committed Transformation T. When: call
//! `Provenance::derive_from(&T)` twice. Then: the same Provenance value both times (a deterministic
//! derivation)." Judgement method: `unit + property`, M2.
//!
//! # The second argument (E-M2-5, `req/38_ERRATA_2026-08-07.md` §8)
//!
//! The AC writes a one-argument call and 42 §3.9 makes it impossible: `Provenance` carries
//! `input_objects` ("the set of input snapshots the adapter read during plan/verify") and an
//! `Environment` of engine version, adapter kind and version, host and correlation id — none of
//! which appears among 41 §3's ten fields. req/49 §3 M2-11 raised it and E-M2-5 ruled it the same
//! way E-A7-2 ruled composition: "the non-derivable part is a caller-supplied argument (`ProvenanceInputs`)". So the call is
//! `derive_from(&t, inputs)` and what AC-015 asks for is unchanged — the same inputs twice give the
//! same value.
//!
//! Determinism is asserted two ways, because they fail differently. Calling twice catches a
//! derivation that reads a clock, a counter or a hash seed. Permuting `input_objects` catches a
//! derivation that is a function of the *order the caller happened to collect in* — which is what
//! req/49 §3 M2-19 raised ("`Vec`'s order affects the CID and there is no canonicalisation rule") and what req/26 §2's
//! id-sort-for-determinism closes.

mod support;

use gx_core::{Cid, ObjectId, Subject, Transformation, TransformationId};
use gx_witness::provenance::{Environment, Provenance, ProvenanceInputs};
use proptest::prelude::*;
use support::{cid, environment, inputs, meta, oid, submitted, tid, with_parents};

// ---------------------------------------------------------------------------
// The criterion itself
// ---------------------------------------------------------------------------

/// AC-015 verbatim: derive twice, compare.
#[test]
fn ac_015_deriving_twice_from_one_transformation_gives_one_value() {
    let t = submitted(1);
    let first = Provenance::derive_from(&t, inputs(vec![oid(2), oid(3)]));
    let second = Provenance::derive_from(&t, inputs(vec![oid(2), oid(3)]));
    assert_eq!(first, second, "two derivations disagreed");
}

proptest! {
    /// The property half of 34's judgement-method column. Many shapes of arrow and input set, each derived
    /// twice: a derivation that consulted anything outside its two arguments fails here and not
    /// above, because the unit case above uses one fixed value and could pass by luck.
    #[test]
    fn ac_015_the_derivation_is_a_function_of_its_two_arguments(
        seed in 0u64..10_000,
        order in 0u8..=2,
        parents in prop::collection::vec(0u64..100, 0..4),
        objects in prop::collection::vec(0u64..1_000, 0..8),
        host in prop::option::of("[a-z]{1,8}"),
    ) {
        let t = Transformation::new(
            tid(seed),
            order,
            Subject::Object(oid(seed)),
            Some(cid(seed)),
            parents.iter().map(|p| tid(*p)).collect(),
            meta(seed),
        ).expect("order is bounded by MAX_ORDER");

        let env = Environment { host_id: host, ..environment() };
        let build = || ProvenanceInputs {
            input_objects: objects.iter().map(|o| oid(*o)).collect(),
            environment: env.clone(),
        };

        prop_assert_eq!(
            Provenance::derive_from(&t, build()),
            Provenance::derive_from(&t, build())
        );
    }
}

// ---------------------------------------------------------------------------
// M2-19 — the order a caller collected in is not part of the answer
// ---------------------------------------------------------------------------

/// `input_objects` is a `Vec<ObjectId>` (42 §3.9) and 42 writes no ordering rule for it, so two
/// adapters that read the same three snapshots in two orders would produce two `Provenance` values
/// and — once anything hashes one — two identities. req/49 §3 M2-19's default proposal is
/// "canonicalise by ascending id, not by collection order", which is req/26 §2's id-sort-for-determinism, and this is the assertion of it.
#[test]
fn ac_015_the_collection_order_of_input_objects_does_not_reach_the_value() {
    let forwards = Provenance::derive_from(&submitted(1), inputs(vec![oid(9), oid(4), oid(7)]));
    let backwards = Provenance::derive_from(&submitted(1), inputs(vec![oid(7), oid(9), oid(4)]));
    assert_eq!(forwards, backwards);
    assert_eq!(
        forwards.input_objects,
        vec![oid(4), oid(7), oid(9)],
        "the canonical order is ascending by id"
    );
}

proptest! {
    /// Any rearrangement, not the two written above.
    #[test]
    fn ac_015_any_rearrangement_of_the_inputs_derives_the_same_value(
        objects in prop::collection::vec(0u64..64, 0..8),
        rotate in 0usize..16,
    ) {
        let a: Vec<ObjectId> = objects.iter().map(|o| oid(*o)).collect();
        let mut b = a.clone();
        let len = b.len();
        if len > 0 {
            b.rotate_left(rotate % len);
            b.reverse();
        }

        let from_a = Provenance::derive_from(&submitted(1), inputs(a));
        let from_b = Provenance::derive_from(&submitted(1), inputs(b));
        prop_assert_eq!(&from_a, &from_b);

        // and whatever came in, what comes out is ascending
        let mut sorted = from_a.input_objects.clone();
        sorted.sort_unstable();
        prop_assert_eq!(from_a.input_objects, sorted);
    }
}

/// A repeated id is **kept**, not folded away. Dropping one would be a second decision (a `Vec` in
/// 42 §3.9 is a sequence, and "the set of input snapshots the adapter read" does not say whether
/// reading the same snapshot twice is one fact or two), and this hand makes the ordering decision
/// M2-19 raised and no other. Raised as H4-3 in req/53 §4.
#[test]
fn ac_015_a_repeated_input_object_survives_the_canonicalisation() {
    let p = Provenance::derive_from(&submitted(1), inputs(vec![oid(5), oid(2), oid(5)]));
    assert_eq!(p.input_objects, vec![oid(2), oid(5), oid(5)]);
}

// ---------------------------------------------------------------------------
// 42 §3.9 — the field tables
// ---------------------------------------------------------------------------

/// 42 §3.9 verbatim: `transformation: TransformationId`, `intent_digest: Option<Cid>`,
/// `input_objects: Vec<ObjectId>`, `environment: Environment`.
#[test]
fn ac_015_provenance_carries_the_four_fields_42_3_9_names() {
    let t = submitted(1);
    let p = Provenance::derive_from(&t, inputs(vec![oid(2)]));

    let _: TransformationId = p.transformation;
    let _: Option<Cid> = p.intent_digest;
    let _: Vec<ObjectId> = p.input_objects.clone();
    let _: Environment = p.environment.clone();

    assert_eq!(p.transformation, t.id, "the subject of the provenance is T");
    assert_eq!(p.input_objects, vec![oid(2)]);
    assert_eq!(p.environment, environment());
}

/// 42 §3.9's `Environment` verbatim: `engine_version: String`, `adapter_kind: SubstrateKind`,
/// `adapter_version: String`, `host_id: Option<String>`, `correlation_id: Option<String>`.
#[test]
fn ac_015_environment_carries_the_five_fields_42_3_9_names() {
    let e = environment();
    let _: String = e.engine_version.clone();
    let _: gx_core::SubstrateKind = e.adapter_kind.clone();
    let _: String = e.adapter_version.clone();
    let _: Option<String> = e.host_id.clone();
    let _: Option<String> = e.correlation_id.clone();

    // ASM-10 admits a single-node run with no host; the type has to say so rather than fill in a
    // hostname the crate cannot read (41 §6: gx-witness may touch files for keys, not the machine).
    let anonymous = Environment {
        host_id: None,
        correlation_id: None,
        ..environment()
    };
    let p = Provenance::derive_from(
        &submitted(1),
        ProvenanceInputs {
            input_objects: Vec::new(),
            environment: anonymous.clone(),
        },
    );
    assert_eq!(p.environment, anonymous);
}

// ---------------------------------------------------------------------------
// `intent_digest` — the one field that is genuinely derived
// ---------------------------------------------------------------------------

/// 42 §1.3-3 verbatim: "`IntentId` is `Intent`'s (§3.3) CID and is fixed at the moment of `submit`
/// (43 T-1, the Draft transition)". So 42 §3.9's "the canonical digest of the Intent at the moment of `submit`" is exactly `t.intent_id`'s digest —
/// the one part of a `Provenance` that comes out of the arrow rather than out of the caller.
#[test]
fn ac_015_a_submitted_arrow_carries_the_digest_of_its_intent() {
    let t = submitted(1);
    let p = Provenance::derive_from(&t, inputs(Vec::new()));
    assert_eq!(p.intent_digest, Some(t.intent_id.0));
}

/// 42 §3.9 verbatim: "set only when a Draft/Candidate is generated (`None` for a composite transformation of order >= 1)". The predicate this
/// implements is `parents.is_empty()` and the reason is measured in the test below.
#[test]
fn ac_015_an_arrow_with_parents_has_no_intent_digest() {
    let p = Provenance::derive_from(&with_parents(1), inputs(Vec::new()));
    assert_eq!(p.intent_digest, None);
}

/// 🔴 The measured reason `order >= 1` cannot be the predicate.
///
/// `gx_core::compose` sets `order = max(f.order, g.order)` — **not** `+ 1`. Composing two order-0
/// arrows therefore yields an order-0 arrow, so 42 §3.9's parenthetical "a composite transformation of order >= 1"
/// describes a set that does not contain every composition, and reading it as the rule would attach
/// a submitted Intent's digest to a composite. What `compose` does write is `parents = [f.id, g.id]`,
/// and `identity` writes an empty one, so `parents.is_empty()` is the predicate available on a
/// `Transformation` that separates the two. It is a derivation and not a transcription; req/53 §4
/// H4-1 asks the Owner to rule on it.
#[test]
fn ac_015_composition_does_not_raise_the_order_so_order_cannot_be_the_predicate() {
    let f = submitted(1);
    let g = Transformation::new(
        tid(2),
        0,
        Subject::Object(ObjectId(f.target.expect("f has a target"))),
        Some(cid(3)),
        Vec::new(),
        meta(2),
    )
    .expect("order 0");

    let resolve = |s: &Subject| match s {
        Subject::Object(ObjectId(c)) => Some(*c),
        Subject::Transformation(_) => None,
    };
    let composite = gx_core::compose(&f, &g, resolve, meta(3), |t| t.id)
        .expect("f.target is what g.subject denotes");

    assert_eq!(f.order(), 0);
    assert_eq!(g.order(), 0);
    assert_eq!(
        composite.order(),
        0,
        "compose takes the max of the two orders; a composite is not order>=1 by construction"
    );
    assert_eq!(composite.parents, vec![f.id, g.id]);
    assert_eq!(
        Provenance::derive_from(&composite, inputs(Vec::new())).intent_digest,
        None,
        "the composite must not carry a submitted Intent's digest"
    );
}

/// The false-negative direction, stated rather than hidden: an arrow that lists provenance parents
/// without being a composition also loses its intent digest. 41 §3 calls `parents` "the provenance
/// and composition DAG", so the predicate cannot tell those apart — and this hand refuses to invent
/// a second field to tell them apart (H4-1).
#[test]
fn ac_015_a_submitted_arrow_that_lists_parents_also_loses_the_digest() {
    let t = with_parents(1);
    assert_eq!(t.order(), 0);
    assert_eq!(
        Provenance::derive_from(&t, inputs(Vec::new())).intent_digest,
        None
    );
}

// ---------------------------------------------------------------------------
// The value survives being written down
// ---------------------------------------------------------------------------

/// A provenance that cannot be recorded records nothing. JSON here rather than DAG-CBOR because
/// 42 §1.3 gives `Provenance` **no IdentityView row** — the structure has no CID in the canon, and
/// minting one would be inventing a row (H4-2). serde round trip is what the type owes today.
#[test]
fn ac_015_provenance_round_trips_through_serde() {
    let p = Provenance::derive_from(&submitted(1), inputs(vec![oid(2), oid(3)]));
    let text = serde_json::to_string(&p).expect("serialises");
    let back: Provenance = serde_json::from_str(&text).expect("reads back");
    assert_eq!(back, p);
}
