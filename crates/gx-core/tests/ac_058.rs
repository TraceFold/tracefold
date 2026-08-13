//! AC-058 (FR-053) — the two measure traits carry exactly the signatures of 41 §3.
//!
//! AC-058 逐語: 「Given: `ObjectMeasure`/`MorphismMeasure`実装。When: 41 §3のシグネチャ
//! （`measure(&ObjectSnapshot) -> f64` / `measure(&Transformation) -> f64`）と照合する型検査
//! コードをコンパイルする。Then: 両trait実装ともコンパイル成功する。」
//!
//! 41 §3 逐語:
//! `pub trait ObjectMeasure   { fn measure(&self, x: &ObjectSnapshot) -> f64; }`
//! `pub trait MorphismMeasure { fn measure(&self, f: &Transformation) -> f64; }`
//!
//! The laws attached to them in the 41 §3 comment -- `θ(g∘f) ≤ θ(g)+θ(f)`, and `m(Y) ≤ m(X)+θ(f)`
//! as opt-in -- are AC-059 and FR-055, which need `compose`; that is step 5 of req/31 §9, not
//! this one. What AC-058 asks for is the shape, and the fn-pointer coercions below are where
//! that is actually checked: a coercion to `fn(&T, &ObjectSnapshot) -> f64` fails to compile if
//! the trait's parameter or return type has drifted by so much as a reference.

use gx_core::{
    Actor, ChangeContext, Cid, DeltaRef, IntentId, MorphismMeasure, ObjectId, ObjectMeasure,
    ObjectSnapshot, ReprKind, Subject, SubstrateKind, Timestamp, Transformation, TransformationId,
};

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

/// One type implementing both, which is the case FR-055's opt-in law will need later.
struct ByteCount;

impl ObjectMeasure for ByteCount {
    fn measure(&self, x: &ObjectSnapshot) -> f64 {
        x.locator().len() as f64
    }
}

impl MorphismMeasure for ByteCount {
    fn measure(&self, f: &Transformation) -> f64 {
        f64::from(f.order())
    }
}

fn snapshot() -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(1)),
        SubstrateKind::Fs,
        "/tmp/x".to_string(),
        cid(2),
        ReprKind::Bytes,
    )
}

fn transformation() -> Transformation {
    Transformation::new(
        TransformationId(cid(3)),
        2,
        Subject::Object(ObjectId(cid(1))),
        Some(cid(5)),
        vec![],
        gx_core::CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(6),
            },
            context: ChangeContext::Model,
            actor: Actor::Human {
                key: "k".to_string(),
            },
            created_at: Timestamp(0),
        },
    )
    .expect("order 2 is the ceiling itself")
}

#[test]
fn ac_058_both_signatures_match_41_section_3() {
    let om: fn(&ByteCount, &ObjectSnapshot) -> f64 = ObjectMeasure::measure;
    let mm: fn(&ByteCount, &Transformation) -> f64 = MorphismMeasure::measure;
    assert_eq!(om(&ByteCount, &snapshot()), 6.0);
    assert_eq!(mm(&ByteCount, &transformation()), 2.0);
}

#[test]
fn ac_058_both_traits_are_object_safe() {
    // Not stated by the AC, but 41 §3 puts these in the same crate as a gate that will hold a
    // registry of them; a trait that cannot be boxed could not be registered.
    let o: &dyn ObjectMeasure = &ByteCount;
    let m: &dyn MorphismMeasure = &ByteCount;
    assert_eq!(o.measure(&snapshot()), 6.0);
    assert_eq!(m.measure(&transformation()), 2.0);
}
