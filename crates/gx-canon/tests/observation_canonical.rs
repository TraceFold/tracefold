// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/824` A1 + A2, the canonical-encode half** — the four observation classes round-trip
//! through the canonical encoder bit-equal, and the envset fingerprint has a golden vector.
//!
//! Here and not in gx-core, because 41 §6 gives the canonical encode one door and gx-core is
//! forbidden to name this crate (A-1). The behavioral half (detector, chain check, decode
//! refusal) is `crates/gx-core/tests/observation_class.rs`.

use gx_canon::cbor;
use gx_core::{
    EnvsetEntry, EnvsetFingerprint, EnvsetScope, ObservationClass, ObservationId,
    ObservationSubstrate, OBSERVATION_CLASSES,
};

/// One value's round trip: encode, decode, encode again, and the two byte strings agree.
fn roundtrip_bit_equal<T>(value: &T) -> Vec<u8>
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + core::fmt::Debug,
{
    let first = cbor::encode(value).expect("the value has a canonical form");
    let back: T = cbor::decode(&first).expect("the canonical form decodes");
    assert_eq!(&back, value, "decode returned a different value");
    let second = cbor::encode(&back).expect("re-encode");
    assert_eq!(first, second, "the round trip is not bit-equal");
    first
}

/// 🔴 **A1's AC**: canonical-encode round-trip bit-equal for all four classes — and the wire
/// spelling inside the bytes is the schema's, so the CBOR and JSON faces cannot disagree on it.
#[test]
fn every_observation_class_roundtrips_bit_equal() {
    for class in OBSERVATION_CLASSES {
        let bytes = roundtrip_bit_equal(&class);
        let spelled = String::from_utf8_lossy(&bytes);
        assert!(
            spelled.contains(class.as_wire_str()),
            "{class:?}: the canonical bytes do not carry the wire spelling {:?}",
            class.as_wire_str()
        );
    }
    // The refusal side of the same coin, at this layer: bytes carrying a fifth class decode to an
    // error, not to a default. "metrics" as a CBOR text string:
    let outside = cbor::encode(&"metrics").expect("a bare string encodes");
    let decoded: Result<ObservationClass, _> = cbor::decode(&outside);
    assert!(decoded.is_err(), "a fifth class decoded: {decoded:?}");
}

/// The two substrate modes and the id carrier, same property.
#[test]
fn substrate_and_id_roundtrip_bit_equal() {
    for substrate in [
        ObservationSubstrate::Adapter,
        ObservationSubstrate::Declared,
    ] {
        roundtrip_bit_equal(&substrate);
    }
    roundtrip_bit_equal(&ObservationId("vercel-envrev-8831".to_string()));
}

/// The fixture bed's happy-road fingerprint (w824-observation-00000), as the codec builds it.
fn the_golden_input() -> EnvsetFingerprint {
    EnvsetFingerprint::new(
        EnvsetScope {
            project: "acme-web".to_string(),
            environment: "production".to_string(),
        },
        vec![
            // Arrival order is deliberately NOT sorted: the constructor's canonical ordering is
            // part of what the golden bytes pin.
            EnvsetEntry::new(
                "STRIPE_KEY".to_string(),
                "blake3:1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f809"
                    .to_string(),
                None,
            ),
            EnvsetEntry::new(
                "DATABASE_URL".to_string(),
                "blake3:9f2c1a7e4b8d3f6015a2c9e8d7b4f1032e5a8c7b9d0f3e6a1c4b7d8e9f0a2b3c"
                    .to_string(),
                None,
            ),
        ],
        Some("gx1:envset-acme-prod-0007".to_string()),
    )
}

/// 🔴 **A2's AC**: golden vector — fixed input set ⇒ fixed canonical bytes, byte-stable across
/// runs *and across builds*, which is what the hardcoded digest of the bytes holds. The digest is
/// asserted rather than the full byte string so the golden line stays readable; a single moved
/// byte still moves it.
#[test]
fn the_envset_fingerprint_golden_vector_is_byte_stable() {
    let bytes = roundtrip_bit_equal(&the_golden_input());
    let cid =
        gx_canon::cid::of_canonical_bytes(&bytes).expect("the encoder's own bytes are canonical");
    let spelled = gx_canon::cid::to_text(&cid);

    // Golden, measured at first landing (req/824 A2, this commit; printed by this test's own
    // first red run and engraved). If this moves, the canonical form of a chain-anchoring value
    // moved, and that is a wire-shape change somebody must rule on -- never a number to freshen.
    const GOLDEN: &str = "gx1:sasae334v4fcxkmxns64qmvmidsogaezizmucildxgxh2emh27xq";
    assert_eq!(
        spelled,
        GOLDEN,
        "the envset fingerprint's canonical bytes moved (len={})",
        bytes.len()
    );

    // The same set arriving in the other order produces the same bytes -- the ordering is the
    // constructor's, so arrival order cannot fork a chain (the gx-core half asserts value
    // equality; this asserts the bytes a signature would cover).
    let other_arrival = EnvsetFingerprint::new(
        the_golden_input().scope().clone(),
        vec![
            the_golden_input().entries()[1].clone(),
            the_golden_input().entries()[0].clone(),
        ],
        the_golden_input().prev().map(str::to_string),
    );
    assert_eq!(bytes, cbor::encode(&other_arrival).expect("encodes"));
}
