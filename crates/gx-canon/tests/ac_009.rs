// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-009 (FR-009) — encode then decode returns the value, every field intact.
//!
//! AC-009 verbatim: "Given: a randomly generated `Transformation` value x (proptest arbitrary).
//! When: `dagcbor::encode(x)` → `dagcbor::decode`. Then: the recovered value is bit-equal to x
//! (all fields)." property (roundtrip, ≥1000 cases/PR) (sem: SEM-gx-canon-023).
//!
//! This is the wire face (A-4 face 1, `req/38_ERRATA_2026-08-07.md` §1): every field, including the
//! two that the identity face drops. The identity face is AC-011/AC-013 and does not exist yet.
//!
//! "bit-equal (all fields)" is checked three ways, because value equality alone is weak:
//! `PartialEq` on the decoded value, a field-by-field comparison that a future `PartialEq`
//! change cannot quietly loosen, and a re-encode that must reproduce the original bytes. The
//! third is what catches an encoding that normalises on the way in -- it would still decode to
//! an equal value while having thrown information away.

mod support;

use gx_canon::cbor;
use gx_core::Transformation;
use proptest::prelude::*;
use support::{any_object_snapshot, any_transformation, hex};

proptest! {
    #[test]
    fn ac_009_transformation_round_trip_is_bit_equal(x in any_transformation()) {
        let bytes = cbor::encode(&x).expect("encode");
        let back: Transformation = cbor::decode(&bytes).expect("decode");
        prop_assert_eq!(&back, &x, "decoded value differs; bytes = {}", hex(&bytes));
    }

    /// The ten fields of 41 §3, one assertion each. Nine are named in the pattern and `order`
    /// is read through its accessor -- F-2 (`req/46D_AUDIT_RULING_2026-08-07.md` §1) made that
    /// field private, and this is a separate crate. The no-`..` guard against an eleventh field
    /// lives in `gx-core/src/transformation.rs::field_set`, where all ten are still visible.
    #[test]
    fn ac_009_every_one_of_the_ten_fields_survives(x in any_transformation()) {
        let bytes = cbor::encode(&x).expect("encode");
        let back: Transformation = cbor::decode(&bytes).expect("decode");
        let order = back.order();
        let Transformation {
            id, intent_id, subject, target, delta, context, actor, parents, created_at, ..
        } = back;
        prop_assert_eq!(id, x.id);
        prop_assert_eq!(intent_id, x.intent_id);
        prop_assert_eq!(order, x.order());
        prop_assert_eq!(subject, x.subject);
        prop_assert_eq!(target, x.target);
        prop_assert_eq!(delta, x.delta);
        prop_assert_eq!(context, x.context);
        prop_assert_eq!(actor, x.actor);
        prop_assert_eq!(parents, x.parents);
        prop_assert_eq!(created_at, x.created_at);
    }

    /// Re-encoding the decoded value reproduces the bytes. A round trip that merely returns an
    /// equal value can still be lossy at the byte level; this is the half that notices.
    #[test]
    fn ac_009_re_encoding_the_decoded_value_reproduces_the_bytes(x in any_transformation()) {
        let bytes = cbor::encode(&x).expect("encode");
        let back: Transformation = cbor::decode(&bytes).expect("decode");
        let again = cbor::encode(&back).expect("re-encode");
        prop_assert_eq!(hex(&again), hex(&bytes));
    }

    /// `ObjectSnapshot` is the other persistent structure M1 has (42 §1.3 lists both). AC-009
    /// names `Transformation`, but a wire face that only worked for one struct would be a
    /// property of that struct rather than of the encoding.
    #[test]
    fn ac_009_object_snapshot_round_trips_too(x in any_object_snapshot()) {
        let bytes = cbor::encode(&x).expect("encode");
        let back: gx_core::ObjectSnapshot = cbor::decode(&bytes).expect("decode");
        prop_assert_eq!(&back, &x);
    }
}

/// 42 §2.1-4 keeps floats out of canonical structures, and req/26 §3 says the way to treat an
/// input outside the supported range is to refuse it rather than guess. Both directions of
/// "unsupported" are checked: a value CBOR cannot represent at all (NaN), and one it can
/// represent but gx does not admit (a finite f64).
#[test]
fn ac_009_unsupported_input_fails_loudly_rather_than_quietly() {
    assert!(cbor::encode(&f64::NAN).is_err(), "NaN must not encode");
    assert!(
        cbor::encode(&f64::INFINITY).is_err(),
        "Infinity must not encode"
    );
    assert!(
        cbor::encode(&f64::NEG_INFINITY).is_err(),
        "-Infinity must not encode"
    );
    assert!(
        cbor::encode(&1.0f64).is_err(),
        "42 §2.1-4: no floats in canonical structures"
    );
    // Beyond CBOR's integer range. Major type 0 reaches u64::MAX and major type 1 reaches
    // -1-u64::MAX, and the bignum tag that would carry anything wider is a tag, which 42 §2.1-5
    // forbids. The "BigInt tier" case of the step-3 instruction (sem: SEM-gx-canon-024).
    let smallest = -i128::from(u64::MAX) - 1; // -2^64, the last value major type 1 can spell
    let largest = i128::from(u64::MAX);
    assert!(
        cbor::encode(&largest).is_ok(),
        "u64::MAX is inside the range"
    );
    assert!(cbor::encode(&smallest).is_ok(), "-2^64 is inside the range");
    assert!(
        cbor::encode(&(largest + 1)).is_err(),
        "integers past u64::MAX have no tagless DAG-CBOR form"
    );
    assert!(
        cbor::encode(&(smallest - 1)).is_err(),
        "integers below -2^64 have no tagless DAG-CBOR form"
    );
}
