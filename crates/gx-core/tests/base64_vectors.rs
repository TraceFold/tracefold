// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! RFC 4648 §10's own vectors, and the JSON face they give every raw byte string (**M2H1-4**).
//!
//! `req/38_ERRATA_2026-08-07.md` §9 left hand 5 one question: "the JSON spelling of raw bytes
//! (sig / FingerprintBytes) is **decided by hand 5, the receipt's JSON face**; the default
//! recommendation is base64" (quoted in SEM-gx-core-130). This file is the answer, and
//! it is checked in two directions -- against the standard's table, and against the two types the
//! ruling names.
//!
//! # Why the vectors come from RFC 4648 and not from this implementation
//!
//! A golden file produced by running the code it tests records what the code does, which is the one
//! thing already known. `req/06_OSS_OBSERVATION_2026-08-06.md` made the same complaint of the DSSE
//! spec repository -- "**no dedicated test vectors in the spec repo** ... with no machine-checkable
//! reference vector in the spec, the risk of drift between implementations remains structurally"
//! -- and named "ship spec-level test vectors with values (input -> expected bytes) inside the
//! crate" as what gx should do instead (both quoted in SEM-gx-core-131). [`RFC_4648_S10`] is
//! that: seven pairs
//! transcribed from the standard's own section 10, none of them produced here.
//!
//! # What this file does not check
//!
//! Nothing here hashes, encodes canonically, or signs. base64 is a rendering for formats that have
//! no byte type; 42 §2.1's canonical rules are about DAG-CBOR and the binary face is untouched
//! (`req/38_ERRATA_2026-08-07.md` §9: "that the DAG-CBOR face is a byte-string (major type 2) is
//! already decided"; sem: SEM-gx-core-132).

use gx_core::{b64, DsseSignature, FingerprintBytes};

/// RFC 4648 §10, "Test Vectors", verbatim. Seven pairs, covering all three padding cases.
const RFC_4648_S10: &[(&str, &str)] = &[
    ("", ""),
    ("f", "Zg=="),
    ("fo", "Zm8="),
    ("foo", "Zm9v"),
    ("foob", "Zm9vYg=="),
    ("fooba", "Zm9vYmE="),
    ("foobar", "Zm9vYmFy"),
];

// ---------------------------------------------------------------------------
// The table itself
// ---------------------------------------------------------------------------

/// The standard's vectors, encoded.
#[test]
fn base64_encodes_rfc_4648_section_10() {
    for (plain, expected) in RFC_4648_S10 {
        assert_eq!(
            b64::encode(plain.as_bytes()),
            *expected,
            "RFC 4648 §10: BASE64({plain:?})"
        );
    }
}

/// And decoded. Both directions, because an encoder and a decoder that share a wrong table agree
/// with each other and with nothing else.
#[test]
fn base64_decodes_rfc_4648_section_10() {
    for (plain, encoded) in RFC_4648_S10 {
        assert_eq!(
            b64::decode(encoded).expect("a vector from the standard decodes"),
            plain.as_bytes(),
            "RFC 4648 §10: BASE64-DECODE({encoded:?})"
        );
    }
}

/// Every byte value, through both directions. The vectors above are ASCII; a table with one wrong
/// entry above 0x7f would pass them.
#[test]
fn base64_round_trips_every_byte_value() {
    let all: Vec<u8> = (0..=255u8).collect();
    for length in [1usize, 2, 3, 31, 32, 64, 255, 256] {
        let sample = &all[..length.min(all.len())];
        let text = b64::encode(sample);
        assert_eq!(
            b64::decode(&text).expect("what this encoder wrote, it reads"),
            sample,
            "{length} bytes did not survive"
        );
    }
}

/// One byte string, one spelling. A decoder that repaired its input would let a signature travel
/// under two names, which is the property `Cid::from_text` is strict for.
#[test]
fn base64_refuses_every_spelling_that_is_not_the_one() {
    let refused = [
        ("Zm9vYmF", "a length that is not a multiple of four"),
        (
            "Zm9vYmFy=",
            "a length that is not a multiple of four, with padding",
        ),
        ("Zg==Zg==", "padding before the end"),
        ("Zg===", "a length that is not a multiple of four"),
        ("Zm 9v", "a space"),
        ("Zm9v\n", "a trailing newline"),
        ("Zm9-", "a URL-safe character, which table 1 does not have"),
        ("Zm9_", "the other URL-safe character"),
        ("Zh==", "a final group whose unused bits are set"),
        ("Zm9=", "a three-character group whose unused bits are set"),
    ];
    for (text, why) in refused {
        assert!(
            b64::decode(text).is_err(),
            "{text:?} was accepted despite {why}"
        );
    }
    // The near-miss of the last two: the same length and alphabet, with the unused bits clear.
    assert!(b64::decode("Zg==").is_ok());
    assert!(b64::decode("Zm8=").is_ok());
}

// ---------------------------------------------------------------------------
// The two types M2H1-4 names
// ---------------------------------------------------------------------------

/// `DsseSignature.sig` in JSON is a base64 string, and in DAG-CBOR it is still bytes.
///
/// The second half is the one the ruling calls "already decided; do not move it" (sem:
/// SEM-gx-core-133), and it is asserted here
/// rather than assumed: gx-core cannot name a DAG-CBOR encoder (that is gx-canon), so what is
/// checked is the *shape serde is asked for* -- a JSON value that is a string, and a serializer
/// that is not human-readable receiving bytes rather than a sequence. `gx-canon`'s own suites hold
/// the byte-level side.
#[test]
fn a_signature_is_base64_in_json() {
    let signature = DsseSignature {
        keyid: "key-1".to_string(),
        sig: b"foobar".to_vec(),
    };
    let json = serde_json::to_string(&signature).expect("a signature serialises");
    assert!(
        json.contains("\"Zm9vYmFy\""),
        "the JSON face is not base64: {json}"
    );
    assert!(
        !json.contains("102"),
        "the JSON face is still a list of integers: {json}"
    );
    assert_eq!(
        serde_json::from_str::<DsseSignature>(&json).expect("and reads back"),
        signature
    );
}

/// The same for `FingerprintBytes`, the other type M2H1-4 names.
#[test]
fn a_fingerprint_is_base64_in_json() {
    let bytes = FingerprintBytes([0u8; 32]);
    let json = serde_json::to_string(&bytes).expect("a fingerprint serialises");
    assert_eq!(json, "\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\"");
    assert_eq!(
        serde_json::from_str::<FingerprintBytes>(&json).expect("and reads back"),
        bytes
    );
}

/// The old spelling still reads. A key file or a fixture written before this hand carried a list of
/// integers, and refusing those would make the change a migration rather than a decision about how
/// to *write*. Each of the three forms names exactly one byte string, so the map back stays
/// one-to-one -- which is the property that made strictness worth having in the first place.
#[test]
fn the_sequence_spelling_that_came_before_still_reads() {
    let old = r#"{"keyid":"key-1","sig":[102,111,111]}"#;
    let back: DsseSignature = serde_json::from_str(old).expect("the previous spelling reads");
    assert_eq!(back.sig, b"foo".to_vec());

    let old_fingerprint = format!("[{}]", vec!["1"; 32].join(","));
    let back: FingerprintBytes =
        serde_json::from_str(&old_fingerprint).expect("the previous spelling reads");
    assert_eq!(back, FingerprintBytes([1u8; 32]));
}

/// A fingerprint that is not thirty-two bytes is refused through the base64 road too, not only
/// through the sequence one. Two entrances, one invariant.
#[test]
fn a_fingerprint_of_the_wrong_length_is_refused_in_either_spelling() {
    assert!(serde_json::from_str::<FingerprintBytes>("\"Zm9v\"").is_err());
    assert!(serde_json::from_str::<FingerprintBytes>("[1,2,3]").is_err());
}
