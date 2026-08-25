// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-013 — representation independence: the same logical value, reached differently, has one CID.
//!
//! 34 AC-013 verbatim: "Given: a JSON representation `repr_json(x)` and a native struct
//! representation `repr_native(x)` of the same logical content. When: canonicalize both.
//! Then: the resulting canonical CIDs agree", with "at least two concrete example routes"
//! (sem: SEM-gx-canon-033) + property as the method. 42 §2.3's second bullet is the same requirement stated
//! as `repr₁(x) ≈ repr₂(x) → canon(repr₁ x) = canon(repr₂ x)`, and `51 §3` names the property
//! `repr_independence`.
//!
//! # The routes
//!
//! Three, one more than the minimum: the native struct; a JSON document parsed back into it; and
//! the canonical DAG-CBOR bytes decoded back into it. The third is worth having because it closes
//! the loop with step 3 -- the wire face round trip (AC-009) and the identity face have to agree,
//! or a value could survive storage and come back with a different identity.
//!
//! # Why the JSON is deliberately badly formatted
//!
//! 42 §2.3 defines the equivalence "JSON formatting differences are ignored" (sem: SEM-gx-canon-034). So one of the routes writes the JSON out
//! with its object keys reversed and whitespace scattered through it, which is a genuine second
//! *representation* rather than the same string twice. If the CID moved under that, canonical
//! form would be a property of the text a value arrived in rather than of the value.
//!
//! # The adapter clause that is not tested here
//!
//! 42 §2.3 also speaks of `ReprKind` differences normalised by a `SubstrateAdapter`. There is no
//! adapter in M1 (gx-substrate is M4), so nothing in this file claims that two substrate readings
//! of the same file converge -- only that two encodings of the same value do.

mod support;

use gx_canon::{cbor, cid};
use gx_core::{ObjectSnapshot, Transformation};
use proptest::prelude::*;
use serde_json::Value;
use support::{
    any_object_snapshot, any_transformation, sample_object_snapshot, sample_transformation,
};

/// Write a JSON value out again with every object's keys in the opposite order and the whitespace
/// moved around. Same document, different spelling -- which is exactly what 42 §2.3 asks the
/// canonical form to ignore.
fn respelled(value: &Value, depth: usize) -> String {
    match value {
        Value::Object(map) => {
            let pad = "  ".repeat(depth + 1);
            let inner: Vec<String> = map
                .iter()
                .rev()
                .map(|(k, v)| {
                    format!(
                        "\n{pad}{} :  {}",
                        Value::String(k.clone()),
                        respelled(v, depth + 1)
                    )
                })
                .collect();
            format!("{{{}\n{}}}", inner.join(" ,"), "  ".repeat(depth))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(|v| respelled(v, depth + 1)).collect();
            format!("[ {} ]", inner.join(" , "))
        }
        scalar => scalar.to_string(),
    }
}

fn repr_json<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let document = serde_json::to_value(value).expect("the value has a JSON form");
    let text = respelled(&document, 0);
    serde_json::from_str(&text).expect("the respelled document is still the same value")
}

fn repr_cbor<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    cbor::decode(&cbor::encode(value).expect("encodes")).expect("decodes")
}

#[test]
fn ac_013_a_transformation_has_one_cid_by_three_routes() {
    let native = sample_transformation();
    let from_json: Transformation = repr_json(&native);
    let from_cbor: Transformation = repr_cbor(&native);

    let a = cid::compute(&native).expect("native route");
    let b = cid::compute(&from_json).expect("json route");
    let c = cid::compute(&from_cbor).expect("cbor route");

    assert_eq!(a, b, "repr_json(x) and repr_native(x) disagree");
    assert_eq!(a, c, "the wire round trip changed the identity");
    println!("AC013_ROUTES=3 CID={}", cid::to_text(&a));
}

#[test]
fn ac_013_an_object_snapshot_has_one_cid_by_three_routes() {
    let native = sample_object_snapshot();
    let from_json: ObjectSnapshot = repr_json(&native);
    let from_cbor: ObjectSnapshot = repr_cbor(&native);

    let a = cid::compute(&native).expect("native route");
    assert_eq!(a, cid::compute(&from_json).expect("json route"));
    assert_eq!(a, cid::compute(&from_cbor).expect("cbor route"));
}

/// The respelling has to be a real one, or the test above compares a string with itself.
#[test]
fn ac_013_the_second_representation_is_actually_different() {
    let native = sample_transformation();
    let plain = serde_json::to_string(&native).expect("json");
    let other = respelled(&serde_json::to_value(&native).expect("json value"), 0);
    assert_ne!(plain, other, "the two JSON spellings must differ");
    println!(
        "AC013_JSON_BYTES_PLAIN={} AC013_JSON_BYTES_RESPELLED={}",
        plain.len(),
        other.len()
    );
}

/// Non-vacuity: representation independence must not be indifference to content.
#[test]
fn ac_013_different_content_still_gives_different_cids() {
    let a = sample_transformation();
    let mut b = a.clone();
    b.set_order(2).expect("2 is the ceiling itself");
    assert_ne!(
        cid::compute(&a).expect("a"),
        cid::compute(&b).expect("b"),
        "two different transformations shared one identity"
    );
}

proptest! {
    /// `repr_independence` (51 §3), over generated values.
    #[test]
    fn ac_013_repr_independence(value in any_transformation()) {
        let native = cid::compute(&value).expect("native");
        let json: Transformation = repr_json(&value);
        let wire: Transformation = repr_cbor(&value);
        prop_assert_eq!(cid::compute(&json).unwrap(), native);
        prop_assert_eq!(cid::compute(&wire).unwrap(), native);
    }

    #[test]
    fn ac_013_repr_independence_for_snapshots(value in any_object_snapshot()) {
        let native = cid::compute(&value).expect("native");
        let json: ObjectSnapshot = repr_json(&value);
        prop_assert_eq!(cid::compute(&json).unwrap(), native);
    }
}
