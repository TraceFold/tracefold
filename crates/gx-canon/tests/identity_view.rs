//! T-22 — the IdentityView projection: what reaches a CID and what does not.
//!
//! Spec: 42 §1.3, whose table is the whole of this file's expectation —— `Transformation` keeps
//! `order`, `intent_id`, `subject`, `target`, `delta`, `context`, `actor`, `parents` and drops
//! `id` (self-reference) and `created_at` (ASM-4, metadata); `ObjectSnapshot` keeps `substrate`,
//! `locator`, `digest`, `representation` and drops `id`.
//!
//! The trait lives in gx-canon and so do the two impls, which is what keeps gx-core unaware that
//! any of this exists (A-4, `req/38_ERRATA_2026-08-07.md` §1; the orphan rule would have forced
//! one of the two into gx-core otherwise).
//!
//! Two directions are checked, because only one of them is a real property. That the excluded
//! fields do not change the projection is the requirement. That every *included* field does
//! change it is the guard against satisfying the requirement by projecting nothing —— a view
//! that dropped `delta` as well would pass every test in the first direction.

mod support;

use gx_canon::cbor;
use gx_canon::cid::IdentityView;
use gx_core::{ObjectSnapshot, Subject, Timestamp, Transformation, TransformationId};
use ipld_core::ipld::Ipld;
use proptest::prelude::*;
use std::collections::BTreeMap;
use support::{
    any_actor, any_change_context, any_cid, any_delta_ref, any_object_snapshot, any_transformation,
    any_transformation_id, cid_of, sample_object_snapshot, sample_transformation,
};

/// The projected bytes. Going through [`cbor::encode`] is not a convenience here: 42 §1.1 defines
/// a CID as BLAKE3 over the *canonical* form, so a projection that could be encoded some other
/// way would not be the thing the spec hashes.
fn view_bytes<T: IdentityView>(value: &T) -> Vec<u8> {
    cbor::encode(&value.identity_view()).expect("the projection of a valid value must encode")
}

fn view_keys<T: IdentityView>(value: &T) -> Vec<String> {
    let map: BTreeMap<String, Ipld> =
        cbor::decode(&view_bytes(value)).expect("the projection is a map of named fields");
    map.into_keys().collect()
}

#[test]
fn t_022_transformation_view_is_the_eight_fields_of_42_1_3() {
    let mut expected = vec![
        "order",
        "intent_id",
        "subject",
        "target",
        "delta",
        "context",
        "actor",
        "parents",
    ];
    expected.sort_unstable();
    assert_eq!(view_keys(&sample_transformation()), expected);
}

#[test]
fn t_022_object_snapshot_view_is_the_four_fields_of_42_1_3() {
    let mut expected = vec!["substrate", "locator", "digest", "representation"];
    expected.sort_unstable();
    assert_eq!(view_keys(&sample_object_snapshot()), expected);
}

/// The projection is not a second encoding. Whatever it produces is bytes the wire face would
/// have written, which is what makes `cid::compute` a composition rather than a parallel road
/// (AC-014, and 42 §2.1-6's rule that canonical bytes come from one place).
#[test]
fn t_022_the_projection_lands_on_the_wire_face() {
    assert!(cbor::is_canonical(&view_bytes(&sample_transformation())));
    assert!(cbor::is_canonical(&view_bytes(&sample_object_snapshot())));
}

/// Every field 42 §1.3 lists as included has to be load-bearing. Without this, a projection that
/// silently dropped a field would still satisfy the exclusion property below.
#[test]
fn t_022_each_included_field_changes_the_projection() {
    let base = sample_transformation();
    let baseline = view_bytes(&base);

    let mut mutants: Vec<(&str, Transformation)> = Vec::new();

    let mut m = base.clone();
    m.set_order(2).expect("2 is the ceiling itself");
    mutants.push(("order", m));

    let mut m = base.clone();
    m.intent_id = gx_core::IntentId(cid_of(0xAA));
    mutants.push(("intent_id", m));

    let mut m = base.clone();
    m.subject = Subject::Transformation(TransformationId(cid_of(0xAB)));
    mutants.push(("subject", m));

    let mut m = base.clone();
    m.target = None;
    mutants.push(("target", m));

    let mut m = base.clone();
    m.delta.cid = cid_of(0xAC);
    mutants.push(("delta", m));

    let mut m = base.clone();
    m.context = gx_core::ChangeContext::Policy;
    mutants.push(("context", m));

    let mut m = base.clone();
    m.actor = gx_core::Actor::Human {
        key: "someone-else".to_string(),
    };
    mutants.push(("actor", m));

    let mut m = base.clone();
    m.parents.push(TransformationId(cid_of(0xAD)));
    mutants.push(("parents", m));

    assert_eq!(mutants.len(), 8, "42 §1.3 lists eight included fields");
    for (name, mutant) in mutants {
        assert_ne!(
            view_bytes(&mutant),
            baseline,
            "changing `{name}` left the projection unchanged, so it is not really projected"
        );
    }

    let base = sample_object_snapshot();
    let baseline = view_bytes(&base);
    let mut snapshot_mutants: Vec<(&str, ObjectSnapshot)> = Vec::new();

    // Rebuilt rather than mutated: F-6 (`req/46D_AUDIT_RULING_2026-08-07.md` §1) made the five
    // fields private, so a snapshot is read through its accessors and written through `new`.
    let respun = |substrate, locator: &str, digest, representation| {
        ObjectSnapshot::new(
            *base.id(),
            substrate,
            locator.to_string(),
            digest,
            representation,
        )
    };

    snapshot_mutants.push((
        "substrate",
        respun(
            gx_core::SubstrateKind::Git,
            base.locator(),
            *base.digest(),
            *base.representation(),
        ),
    ));
    snapshot_mutants.push((
        "locator",
        respun(
            base.substrate().clone(),
            "/tmp/y",
            *base.digest(),
            *base.representation(),
        ),
    ));
    snapshot_mutants.push((
        "digest",
        respun(
            base.substrate().clone(),
            base.locator(),
            cid_of(0xAE),
            *base.representation(),
        ),
    ));
    snapshot_mutants.push((
        "representation",
        respun(
            base.substrate().clone(),
            base.locator(),
            *base.digest(),
            gx_core::ReprKind::Json,
        ),
    ));

    assert_eq!(snapshot_mutants.len(), 4);
    for (name, mutant) in snapshot_mutants {
        assert_ne!(
            view_bytes(&mutant),
            baseline,
            "changing `{name}` left the snapshot projection unchanged"
        );
    }
}

proptest! {
    /// 42 §1.3-1 and §1.3-2. Recording the same change twice, or storing it under a different
    /// id, must not produce a second identity.
    #[test]
    fn t_022_transformation_id_and_created_at_never_reach_the_projection(
        base in any_transformation(),
        other_id in any_transformation_id(),
        other_time in any::<i64>(),
    ) {
        let mut moved = base.clone();
        moved.id = other_id;
        moved.created_at = Timestamp(other_time);
        prop_assert_eq!(view_bytes(&moved), view_bytes(&base));
    }

    /// 42 §1.3-1 for `ObjectSnapshot`: `id` is the CID of this very projection, so including it
    /// would define the value in terms of itself.
    #[test]
    fn t_022_object_snapshot_id_never_reaches_the_projection(
        base in any_object_snapshot(),
        other in any_cid(),
    ) {
        let moved = ObjectSnapshot::new(
            gx_core::ObjectId(other),
            base.substrate().clone(),
            base.locator().to_string(),
            *base.digest(),
            *base.representation(),
        );
        prop_assert_eq!(view_bytes(&moved), view_bytes(&base));
    }

    /// The projection is a function of the value, not of the run.
    #[test]
    fn t_022_the_projection_is_deterministic(value in any_transformation()) {
        prop_assert_eq!(view_bytes(&value), view_bytes(&value));
    }

    /// The other half of the mutation test, done over generated values rather than one fixture:
    /// two transformations that differ in a projected field project differently.
    #[test]
    fn t_022_projected_differences_survive(
        base in any_transformation(),
        delta in any_delta_ref(),
        context in any_change_context(),
        actor in any_actor(),
    ) {
        let mut other = base.clone();
        other.delta = delta;
        other.context = context;
        other.actor = actor;
        if other.delta != base.delta || other.context != base.context || other.actor != base.actor {
            prop_assert_ne!(view_bytes(&other), view_bytes(&base));
        }
    }
}
