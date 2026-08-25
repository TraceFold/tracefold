// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! T-18 / T-19 — the shape of `compose` and `identity`, against 46 §2.1 line by line.
//!
//! AC-059 and AC-060 measure the composite; nothing in them looks at what the composite *is*.
//! This file is the other half: every field 46 §2.1 fixes, checked against the Lean definition it
//! is a translation of.
//!
//! ```lean
//! def compose (f g : Transformation) (h : composable f g) : Transformation :=
//!   { id := composeId f.id g.id, order := max f.order g.order, src := f.src, dst := g.dst }
//! def identity (x : ObjectSnapshot) (idOf : ObjectSnapshot → TransformationId) : Transformation :=
//!   { id := idOf x, order := 0, src := x, dst := x }
//! ```
//!
//! What is deliberately **not** here: associativity and the unit laws. `composeId` is an `axiom`
//! (46 §2.1, ASM-03-2), so neither model proves them, and a Rust test asserting
//! `compose(compose(f,g),h) == compose(f,compose(g,h))` would be asserting something about the
//! injected callback rather than about this crate. M8 owns those laws.

mod conformance;

use conformance::{arrow, metadata, snapshot, World};
use gx_core::{
    ancestors, composable, compose, identity, Cid, Error, Subject, Transformation,
    TransformationId, MAX_ORDER,
};

fn constant_id(seed: u8) -> impl FnOnce(&Transformation) -> TransformationId {
    move |_| TransformationId(conformance::cid(seed))
}

#[test]
fn t_018_the_composite_carries_the_four_fields_of_46_2_1() {
    let world = World::new(2, 3, 4);
    let gf = compose(
        &world.f,
        &world.g,
        world.resolve(),
        metadata(1),
        constant_id(77),
    )
    .expect("composable");

    // src := f.src
    assert_eq!(gf.subject, world.f.subject);
    // dst := g.dst
    assert_eq!(gf.target, world.g.target);
    // order := max f.order g.order
    assert_eq!(gf.order(), world.f.order().max(world.g.order()));
    // id := (injected)
    assert_eq!(gf.id, TransformationId(conformance::cid(77)));
    // and 41's addition: the composite remembers what it was made of.
    assert_eq!(gf.parents, vec![world.f.id, world.g.id]);
}

#[test]
fn t_018_order_is_the_maximum_and_goes_through_with_order() {
    let world = World::new(1, 2, 3);

    for (a, b) in [(0u8, 2u8), (2u8, 0u8), (1, 1), (2, 2), (0, 0)] {
        let f = arrow(40, &world.x, &world.y, a);
        let g = arrow(41, &world.y, &world.z, b);
        let composed = compose(&f, &g, world.resolve(), metadata(1), constant_id(1))
            .expect("both orders are within the ceiling");
        assert_eq!(composed.order(), a.max(b), "order := max f.order g.order");
    }

    // Above the ceiling the composite is refused rather than clamped: the check is
    // `Transformation::with_order`'s, not a copy (req/31 §7).
    //
    // Since F-2 (req/46D §1) no constructor will build such an argument, so this one is decoded
    // rather than built -- which is the shape the remaining threat actually has. `compose` cannot
    // assume its arguments came through `new`, because a value that arrived over a wire did not.
    let over = conformance::arrow_above_the_ceiling(42, &world.x, &world.y, MAX_ORDER + 1);
    let err = compose(
        &over,
        &world.g,
        world.resolve(),
        metadata(1),
        constant_id(1),
    )
    .expect_err("an order above the ceiling cannot compose");
    assert_eq!(
        err,
        Error::OrderExceeded {
            got: MAX_ORDER + 1,
            max: MAX_ORDER
        }
    );
}

#[test]
fn t_018_the_provisional_id_cannot_reach_the_result() {
    let world = World::new(2, 3, 4);

    // Whatever the callback is handed, the field it must not consult is `id`. If it did, the two
    // composites below would differ only in what the callback saw -- and they must not.
    let seen: std::cell::Cell<Option<TransformationId>> = std::cell::Cell::new(None);
    let gf = compose(&world.f, &world.g, world.resolve(), metadata(1), |p| {
        seen.set(Some(p.id));
        TransformationId(conformance::cid(9))
    })
    .expect("composable");

    assert_eq!(
        seen.get(),
        Some(TransformationId(Cid([0u8; 32]))),
        "the provisional id is all zeros, so a callback that reads it returns a constant"
    );
    assert_eq!(gf.id, TransformationId(conformance::cid(9)));
    assert_ne!(gf.id, TransformationId(Cid([0u8; 32])));
}

#[test]
fn t_018_a_draft_has_nothing_to_compose_onto() {
    let world = World::new(1, 2, 3);
    let mut draft = world.f.clone();
    draft.target = None;

    assert!(!composable(&draft, &world.g, world.resolve()));
    assert_eq!(
        compose(
            &draft,
            &world.g,
            world.resolve(),
            metadata(1),
            constant_id(1)
        )
        .expect_err("a Draft has no promised post-state"),
        Error::TargetMissing
    );
}

#[test]
fn t_018_an_unresolvable_subject_is_not_a_match() {
    let world = World::new(1, 2, 3);
    let nowhere = |_: &Subject| None;

    assert!(!composable(&world.f, &world.g, nowhere));
    assert_eq!(
        compose(&world.f, &world.g, nowhere, metadata(1), constant_id(1))
            .expect_err("an unknown subject is not a match"),
        Error::SubjectUnresolved,
        "treating `unknown` as `yes` would compose arrows that do not meet"
    );
}

#[test]
fn t_018_the_composite_is_walkable_by_ancestors() {
    let world = World::new(1, 2, 3);
    let gf = compose(
        &world.f,
        &world.g,
        world.resolve(),
        metadata(1),
        constant_id(5),
    )
    .expect("composable");

    let known = [&gf, &world.f, &world.g];
    let found = ancestors(&gf.id, |id| known.into_iter().find(|t| t.id == *id));
    assert_eq!(
        found,
        vec![world.f.id, world.g.id],
        "the provenance DAG of C-2/C-6 runs through `parents`, so composition has to feed it"
    );
}

#[test]
fn t_019_identity_is_the_arrow_from_a_snapshot_to_itself() {
    let x = snapshot(11, 5);
    // The callback is `compose`'s (F-1, req/46D §1): it receives the arrow carrying the
    // all-zero provisional id, so the id it mints is the id of the value it was handed.
    // Deriving one from `x.digest` was the shortcut 46C §2 B-1 measured -- it produced an id
    // that is not the arrow's, and this file was the only place in the repository teaching it.
    // The canonical derivation is a CID, which is gx-canon's (A-1); the worked example lives in
    // `gx-canon/tests/identity_id.rs`, and what gx-core can check is the seam itself.
    let seen: std::cell::Cell<Option<TransformationId>> = std::cell::Cell::new(None);
    let id = identity(&x, metadata(2), |provisional| {
        seen.set(Some(provisional.id));
        TransformationId(conformance::cid(11))
    })
    .expect("the fixture metadata is inside E-M3-13's range");

    assert_eq!(
        seen.get(),
        Some(TransformationId(Cid([0u8; 32]))),
        "the callback sees the arrow, and its id field is the provisional placeholder"
    );
    assert_eq!(id.order(), 0);
    assert_eq!(id.subject, Subject::Object(*x.id()));
    assert_eq!(id.target, Some(*x.digest()));
    assert_eq!(id.id, TransformationId(conformance::cid(11)));
    assert!(id.parents.is_empty(), "an identity is made of nothing");
}

#[test]
fn t_019_an_identity_composes_with_an_arrow_out_of_the_same_snapshot() {
    // Not the unit law -- that would need `composeId` to be more than an axiom. Only the weaker,
    // checkable fact that the identity on `x` meets any arrow whose subject is `x`.
    let world = World::new(4, 5, 6);
    let id = identity(&world.x, metadata(3), constant_id(31))
        .expect("the fixture metadata is inside E-M3-13's range");

    assert!(composable(&id, &world.f, world.resolve()));
    let composed =
        compose(&id, &world.f, world.resolve(), metadata(3), constant_id(21)).expect("composable");
    assert_eq!(composed.subject, Subject::Object(*world.x.id()));
    assert_eq!(composed.target, world.f.target);
    assert_eq!(composed.order(), 0);
}
