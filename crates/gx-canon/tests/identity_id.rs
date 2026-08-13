//! F-1 — the id of an identity arrow, computed the way 42 §1.3 defines it.
//!
//! `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §2 B-1 measured two natural implementations of
//! `identity()`'s `id_of` and got **two different `TransformationId`s for one identity arrow**.
//! The cause was the signature: `id_of` received an `&ObjectSnapshot`, so the value whose
//! identity was being minted -- the `Transformation` -- was not in scope inside the callback, and
//! 42 §1.3's 「生成される Transformation 自身の IdentityView を BLAKE3」 could not be evaluated
//! there at all. Every caller had to invent a substitute, and substitutes differ.
//! `req/46D_AUDIT_RULING_2026-08-07.md` §1 F-1 rules the callback unified with [`compose`]'s:
//! it receives the provisional `Transformation`.
//!
//! This suite is the fixture for that ruling. It lives in gx-canon rather than gx-core because
//! the spec-correct id is a CID, and gx-core may not name canonical encoding (A-1) -- which is
//! also why `gx-core/tests/compose.rs` cannot host the canonical example and had to reach for a
//! shortcut instead.

mod support;

use gx_canon::cid;
use gx_core::{
    identity, Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectSnapshot,
    Subject, SubstrateKind, Timestamp, TransformationId,
};
use std::cell::Cell;
use support::{cid_of, sample_object_snapshot};

/// The five fields `identity` cannot invent (A-7 erratum, `req/38` §1).
fn metadata() -> CompositionMetadata {
    CompositionMetadata {
        intent_id: IntentId(cid_of(0x02)),
        delta: DeltaRef {
            substrate: SubstrateKind::Fs,
            cid: cid_of(0x44),
        },
        context: ChangeContext::Substrate,
        actor: Actor::Human {
            key: "identity-id".to_string(),
        },
        created_at: Timestamp(1_700_000_000_000_000_000),
    }
}

/// The one decision path. Because the callback is handed the arrow, 42 §1.3 can be evaluated
/// inside it, and the id that comes back is the id **of the value that carries it**.
///
/// This is the equality 46C could not obtain: its "snapshot-shortcut" and its
/// "spec-correct, hand-reconstructed" readings disagreed because only one of them had the arrow.
#[test]
fn f_001_the_id_of_an_identity_arrow_is_the_cid_of_that_arrow() {
    let x = sample_object_snapshot();
    let arrow = identity(&x, metadata(), |provisional| {
        TransformationId(cid::compute(provisional).expect("the projection has a canonical form"))
    })
    .expect("the fixture metadata is inside E-M3-13's range");

    assert_eq!(
        arrow.id,
        TransformationId(cid::compute(&arrow).expect("the projection has a canonical form")),
        "the id minted inside the callback must be the id of the finished arrow (42 §1.3)"
    );
}

/// The provisional id cannot reach the answer, exactly as `compose`'s does not (T-18).
///
/// `id` is outside the IdentityView (42 §1.3), so the CID of the provisional value and of the
/// final value are the same bytes -- and a callback that wrongly consults `.id` sees a constant
/// rather than something that looks plausible.
#[test]
fn f_001_the_callback_sees_the_provisional_id_and_it_cannot_reach_the_result() {
    let x = sample_object_snapshot();
    let seen: Cell<Option<TransformationId>> = Cell::new(None);

    let arrow = identity(&x, metadata(), |provisional| {
        seen.set(Some(provisional.id));
        TransformationId(cid_of(0x9))
    })
    .expect("the fixture metadata is inside E-M3-13's range");

    assert_eq!(
        seen.get(),
        Some(TransformationId(Cid([0u8; 32]))),
        "the provisional id is all zeros, so a callback that reads it returns a constant"
    );
    assert_eq!(arrow.id, TransformationId(cid_of(0x9)));
}

/// Non-vacuity: the canonical route discriminates. Two snapshots that differ anywhere the
/// IdentityView looks give identity arrows with different ids, so the equality above is not
/// satisfied by a constant.
#[test]
fn f_001_two_snapshots_give_two_identity_arrows() {
    let mint = |x: &ObjectSnapshot| {
        identity(x, metadata(), |p| {
            TransformationId(cid::compute(p).expect("canonical form"))
        })
        .expect("the fixture metadata is inside E-M3-13's range")
    };

    let a = sample_object_snapshot();
    let b = ObjectSnapshot::new(
        gx_core::ObjectId(cid_of(0x77)),
        SubstrateKind::Fs,
        "/tmp/y".to_string(),
        cid_of(0x88),
        gx_core::ReprKind::Bytes,
    );

    assert_ne!(mint(&a).id, mint(&b).id);
}

/// The shape 46 §2.1 fixes, restated here so the signature change cannot quietly take anything
/// else with it: `order := 0`, `src := x`, `dst := x`, and no parents.
#[test]
fn f_001_the_shape_of_46_2_1_is_unchanged() {
    let x = sample_object_snapshot();
    let arrow = identity(&x, metadata(), |p| {
        TransformationId(cid::compute(p).expect("canonical form"))
    })
    .expect("the fixture metadata is inside E-M3-13's range");

    assert_eq!(arrow.order(), 0);
    assert_eq!(arrow.subject, Subject::Object(*x.id()));
    assert_eq!(arrow.target, Some(*x.digest()));
    assert!(arrow.parents.is_empty(), "an identity is made of nothing");
}
