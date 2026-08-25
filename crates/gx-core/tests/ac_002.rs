// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-002 (FR-002) — `Transformation` is built with every field, at both orders.
//!
//! AC-002, verbatim (quoted in SEM-gx-core-104): "Given: code that builds an order=0
//! `Transformation` (`subject=Subject::Object(oid)`) and an order=1 `Transformation`
//! (`subject=Subject::Transformation(tid)`) with every field of 41 §3 (id, order, subject, target,
//! delta, context, actor, parents, created_at). When: both are built and their fields accessed.
//! Then: the `Subject` enum holds the correct variant at both orders, and both compilation and
//! execution succeed."
//!
//! The parenthesised list has nine names; the field count is **ten**. A-3
//! (`req/38_ERRATA_2026-08-07.md` §1) rules 41 §3 correct and the AC-002 enumeration an erratum,
//! so `intent_id` is a field here even though the AC text omits it. 42 §1.3 depends on that:
//! `intent_id` is inside the IdentityView, and it is what keeps the Draft a Candidate came from
//! reachable (ASM-11's two-stage identity). The AC's own name is unchanged, per req/31 §3.
//!
//! `ac_002_field_set_is_exactly_ten` reads all ten through the public surface. The *mechanical*
//! half -- a destructuring with no `..`, so that adding or dropping a field stops something from
//! compiling -- moved into `gx-core/src/transformation.rs`'s `field_set` unit test when F-2
//! (`req/46D_AUDIT_RULING_2026-08-07.md` §1) made `order` private: a test binary is a separate
//! crate and can no longer name a private field. Same check, one visibility boundary in.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, Subject,
    SubstrateKind, Timestamp, Transformation, TransformationId,
};

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn sample(order: u8, subject: Subject) -> Transformation {
    Transformation::new(
        TransformationId(cid(0x00)),
        order,
        subject,
        Some(cid(0x22)),
        vec![],
        CompositionMetadata {
            intent_id: IntentId(cid(0x11)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(0x33),
            },
            context: ChangeContext::Policy,
            actor: Actor::Agent {
                key: "k".to_string(),
                model: "claude-x".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("the sample orders are within the ceiling")
}

#[test]
fn ac_002_subject_holds_the_right_variant_at_both_orders() {
    let oid = ObjectId(cid(1));
    let tid = TransformationId(cid(2));

    let order0 = sample(0, Subject::Object(oid));
    let order1 = sample(1, Subject::Transformation(tid));

    assert_eq!(order0.order(), 0);
    match &order0.subject {
        Subject::Object(o) => assert_eq!(*o, oid),
        Subject::Transformation(_) => panic!("order=0 must carry an ObjectId (41 §3)"),
    }

    assert_eq!(order1.order(), 1);
    match &order1.subject {
        Subject::Transformation(t) => assert_eq!(*t, tid),
        Subject::Object(_) => panic!("order>=1 must carry a TransformationId (41 §3)"),
    }
}

#[test]
fn ac_002_field_set_is_exactly_ten() {
    let t = sample(2, Subject::Transformation(TransformationId(cid(9))));
    // Nine fields by name, the tenth through its accessor. The no-`..` destructuring that makes
    // an eleventh field a compile error lives in `gx-core/src/transformation.rs::field_set`.
    let order = t.order();
    let Transformation {
        id,
        intent_id,
        subject,
        target,
        delta,
        context,
        actor,
        parents,
        created_at,
        ..
    } = t;

    assert_eq!(id, TransformationId(cid(0)));
    assert_eq!(intent_id, IntentId(cid(0x11)));
    assert_eq!(order, 2);
    assert!(matches!(subject, Subject::Transformation(_)));
    assert_eq!(target, Some(cid(0x22)));
    assert_eq!(delta.substrate, SubstrateKind::Fs);
    assert_eq!(context, ChangeContext::Policy);
    assert_eq!(actor.key(), "k");
    assert!(parents.is_empty());
    assert_eq!(created_at, Timestamp(1_754_000_000_000_000_000));
}

#[test]
fn ac_002_target_is_optional_and_parents_accumulate() {
    // `target: Option<Cid>` is 41 §3: a Draft has not planned yet, so there is no expected
    // post-state to record. `parents` is the DAG edge set exercised by AC-007.
    let mut t = sample(0, Subject::Object(ObjectId(cid(3))));
    t.target = None;
    t.parents = vec![TransformationId(cid(4)), TransformationId(cid(5))];
    assert!(t.target.is_none());
    assert_eq!(t.parents.len(), 2);
}
