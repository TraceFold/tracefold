// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-007 (FR-007) — walking the provenance DAG through `parents`.
//!
//! AC-007, verbatim (quoted in SEM-gx-core-116): "Given: `T1(parents=[])`, `T2(parents=[T1.id])`,
//! `T3(parents=[T2.id])`. When: the parent-walking utility `ancestors(T3.id)` is called. Then: the
//! return value matches `[T2.id, T1.id]` in that order."
//!
//! The AC writes the call with one argument. It cannot literally have one: reaching T2 from
//! T3.id means looking a transformation up, and 41 §6 forbids this crate from doing any I/O. The
//! resolver is therefore a parameter, which is the shape req/31 §7 already fixed -- "gx-core cannot
//! have I/O, so like T-14's `ancestors` it is a pure function that takes the resolver as an
//! argument" (quoted in SEM-gx-core-117) -- and the
//! store that backs the closure is the caller's problem, as it must be.

use gx_core::{
    ancestors, Actor, ChangeContext, Cid, DeltaRef, IntentId, ObjectId, Subject, SubstrateKind,
    Timestamp, Transformation, TransformationId,
};
use std::collections::BTreeMap;

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn node(id: u8, parents: Vec<TransformationId>) -> Transformation {
    Transformation::new(
        TransformationId(cid(id)),
        0,
        Subject::Object(ObjectId(cid(0xAA))),
        None,
        parents,
        gx_core::CompositionMetadata {
            intent_id: IntentId(cid(0xEE)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(0xBB),
            },
            context: ChangeContext::Time,
            actor: Actor::Process {
                key: "k".to_string(),
            },
            created_at: Timestamp(0),
        },
    )
    .expect("order 0 is within the ceiling")
}

fn store(nodes: Vec<Transformation>) -> BTreeMap<TransformationId, Transformation> {
    nodes.into_iter().map(|t| (t.id, t)).collect()
}

#[test]
fn ac_007_linear_chain_returns_parents_nearest_first() {
    let t1 = node(1, vec![]);
    let t2 = node(2, vec![t1.id]);
    let t3 = node(3, vec![t2.id]);
    let (id1, id2, id3) = (t1.id, t2.id, t3.id);
    let db = store(vec![t1, t2, t3]);

    let got = ancestors(&id3, |id| db.get(id));
    assert_eq!(got, vec![id2, id1]);
}

#[test]
fn ac_007_a_root_has_no_ancestors() {
    let t1 = node(1, vec![]);
    let id1 = t1.id;
    let db = store(vec![t1]);
    assert!(ancestors(&id1, |id| db.get(id)).is_empty());
}

#[test]
fn ac_007_merge_visits_each_ancestor_once() {
    // Composition produces `parents = [f.id, g.id]` (req/31 §7), so a diamond is the normal
    // shape here, not an exotic one. The shared grandparent must appear once.
    let t1 = node(1, vec![]);
    let a = node(2, vec![t1.id]);
    let b = node(3, vec![t1.id]);
    let merged = node(4, vec![a.id, b.id]);
    let (id1, ida, idb, idm) = (t1.id, a.id, b.id, merged.id);
    let db = store(vec![t1, a, b, merged]);

    let got = ancestors(&idm, |id| db.get(id));
    assert_eq!(got, vec![ida, idb, id1]);
}

#[test]
fn ac_007_unresolvable_parent_is_reported_but_not_followed() {
    // The id is known from the child's `parents` list whether or not the store holds the value.
    let t2 = node(2, vec![TransformationId(cid(1))]);
    let id2 = t2.id;
    let db = store(vec![t2]);
    assert_eq!(
        ancestors(&id2, |id| db.get(id)),
        vec![TransformationId(cid(1))]
    );
}

#[test]
fn ac_007_a_cycle_terminates() {
    // A cycle is malformed input, and 41 §6 makes a panic (or a hang) a bug rather than a
    // defensible response to one. Two nodes naming each other must still return.
    let mut a = node(1, vec![]);
    let mut b = node(2, vec![]);
    a.parents = vec![b.id];
    b.parents = vec![a.id];
    let (ida, idb) = (a.id, b.id);
    let db = store(vec![a, b]);

    let got = ancestors(&ida, |id| db.get(id));
    assert_eq!(got, vec![idb, ida]);
}
