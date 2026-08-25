// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The DSSE pre-authentication encoding, pinned to bytes. (sem: SEM-gx-witness-224,
//! SEM-gx-witness-225, SEM-gx-witness-226, SEM-gx-witness-227, SEM-gx-witness-228,
//! SEM-gx-witness-229)
//!
//! 42 §3.10 verbatim: "the DSSE signing subject (PAE: Pre-Authentication Encoding) follows the DSSE
//! standard and uses the concatenated default form of `payload_type` and `payload` (compatible with
//! the in-toto/Sigstore reference implementations, research/02)".
//!
//! # Why this file exists, in the words of gx's own observation
//!
//! `req/06_OSS_OBSERVATION_2026-08-06.md` looked at `secure-systems-lab/dsse` and found: "the spec
//! repo (protocol.md = the PAE algorithm description). Implementations are scattered across
//! separate-language repos. **The spec repo has no dedicated test vectors**. The Go implementation's
//! sign_test.gs has 3 PAE examples embedded inline", scored it 2/5 on the last principle -- "no
//! machine-verifiable reference vector exists in the spec, so the risk of drift between
//! implementations remains structural" -- and named what gx should do instead: "**ship
//! value-bearing test vectors (input → expected byte string) at the spec level inside the crate,
//! and do not separate the spec repo from the implementation repo** (1 source of truth)".
//!
//! This is that. Every vector below is a full expected byte string, written out by hand from the
//! formula, next to the implementation and shipped with it. None was produced by running [`pae`]:
//! a golden file generated from the code it tests records only what the code already does.
//!
//! # Where the formula came from, stated plainly
//!
//! 42 §3.10 names the algorithm and does not state it, so the bytes below are the DSSE
//! specification's -- a specification, in the standing RFC 6962 §2.1 has in `gx-log/src/tile.rs`.
//! **No implementation was read or copied** (52 / 05 §4 R-4), and req/49 §3 M2-17 recorded the
//! choice in advance: "the source for implementing DSSE PAE: because research/02 notes sigstore-rs
//! as "pre-1.0...", implementing it ourselves is the default line". The gap in the canonical source is raised as a ticket in req/54 §4 -- a byte formula that a
//! reader has to fetch from another repository is the same defect req/06 scored the DSSE spec 2 on.

mod support;

use gx_witness::dsse::{pae, DsseEnvelope, RECEIPT_PAYLOAD_TYPE};
use support::keypair;

/// `PAE(type, body) = "DSSEv1" ‖ SP ‖ LEN(type) ‖ SP ‖ type ‖ SP ‖ LEN(body) ‖ SP ‖ body`.
///
/// Each entry is `(payload_type, payload, the whole expected encoding)`. The last two are the ones
/// a length-free concatenation would collapse, and they are why the lengths are in the formula.
#[allow(clippy::type_complexity)]
const VECTORS: &[(&str, &[u8], &[u8])] = &[
    // The empty case: both lengths are zero and both are still written.
    ("", b"", b"DSSEv1 0  0 "),
    // One byte of each.
    ("a", b"b", b"DSSEv1 1 a 1 b"),
    // A payload with a space in it -- the separators are positions, not delimiters, so a space
    // inside the payload changes nothing about how the encoding is read.
    (
        "http://example.com/HelloWorld",
        b"hello world",
        b"DSSEv1 29 http://example.com/HelloWorld 11 hello world",
    ),
    // A length that needs two digits, written in ASCII decimal without leading zeros.
    ("x", b"0123456789", b"DSSEv1 1 x 10 0123456789"),
    // gx's own type, which is what every receipt in this workspace carries.
    (
        "application/vnd.glovrex.receipt+dagcbor",
        b"\x01\x02",
        b"DSSEv1 39 application/vnd.glovrex.receipt+dagcbor 2 \x01\x02",
    ),
    // The ambiguity the lengths remove. These two differ only in where the boundary falls, and a
    // plain concatenation would give both the buffer `abc`.
    ("ab", b"c", b"DSSEv1 2 ab 1 c"),
    ("a", b"bc", b"DSSEv1 1 a 2 bc"),
];

// ---------------------------------------------------------------------------
// The golden vectors
// ---------------------------------------------------------------------------

/// Every vector, byte for byte.
#[test]
fn pae_produces_the_declared_bytes() {
    for (payload_type, payload, expected) in VECTORS {
        let produced = pae(payload_type, payload);
        assert_eq!(
            produced,
            *expected,
            "PAE({payload_type:?}, {payload:?})\n  produced {}\n  expected {}",
            String::from_utf8_lossy(&produced),
            String::from_utf8_lossy(expected)
        );
    }
    println!("PAE_GOLDEN_VECTORS={}", VECTORS.len());
}

/// The two vectors that share a naive concatenation must not share an encoding. Stated separately
/// because it is the *reason* for the format and not merely one more row of the table.
#[test]
fn pae_separates_pairs_a_plain_concatenation_would_confuse() {
    let left = pae("ab", b"c");
    let right = pae("a", b"bc");
    assert_ne!(
        left, right,
        "two different (type, payload) pairs share a pre-authentication encoding"
    );

    let naive_left = [b"ab".as_slice(), b"c"].concat();
    let naive_right = [b"a".as_slice(), b"bc"].concat();
    assert_eq!(
        naive_left, naive_right,
        "the premise of this test is wrong: the two do not collide without lengths"
    );
}

/// The prefix and the separators, checked as bytes rather than as a story about them.
#[test]
fn pae_opens_with_dssev1_and_separates_with_single_spaces() {
    let produced = pae("t", b"p");
    assert_eq!(&produced[..6], b"DSSEv1");
    assert_eq!(
        produced.iter().filter(|&&b| b == 0x20).count(),
        4,
        "the encoding has exactly four separators, whatever the fields contain"
    );
}

/// A payload full of spaces adds spaces to the count, which is the point of the previous test being
/// about a payload that has none. Nothing here parses the encoding; the lengths are what make it
/// parseable, and this is the case that shows why.
#[test]
fn a_payload_of_spaces_does_not_confuse_the_encoding() {
    let produced = pae("t", b"   ");
    assert_eq!(produced, b"DSSEv1 1 t 3    ");
    assert_ne!(produced, pae("t", b"  "));
}

/// Lengths are byte lengths and not character counts. A payload type with a multi-byte character in
/// it is the case where the two differ, and a signature computed against the wrong one would verify
/// nowhere.
#[test]
fn the_lengths_count_bytes_and_not_characters() {
    let produced = pae("é", b"");
    assert_eq!(
        produced,
        "DSSEv1 2 é 0 ".as_bytes(),
        "the type is one character and two bytes"
    );
}

// ---------------------------------------------------------------------------
// What the envelope actually signs
// ---------------------------------------------------------------------------

/// The envelope's signing bytes are `pae` of its two fields, and nothing else.
///
/// Worth asserting because it is the hinge AC-019 turns on: the signature covers the payload *as
/// bytes*, so a flipped bit in it is a signature failure rather than a parse failure.
#[test]
fn an_envelope_signs_the_pae_of_its_own_two_fields() {
    let envelope = DsseEnvelope {
        payload_type: RECEIPT_PAYLOAD_TYPE.to_string(),
        payload: vec![0xa1, 0x62, 0x68, 0x69, 0x01],
        signatures: Vec::new(),
    };
    assert_eq!(
        envelope.signing_bytes(),
        pae(RECEIPT_PAYLOAD_TYPE, &envelope.payload)
    );
    assert!(envelope.signing_bytes().ends_with(&envelope.payload));
}

/// The signatures are outside the signed bytes (42 §1.3-4: "signatures are excluded"). A signature that covered
/// the signature list could not be added to it.
#[test]
fn the_signature_list_is_not_inside_what_is_signed() {
    let key = keypair(1);
    let mut envelope = DsseEnvelope {
        payload_type: RECEIPT_PAYLOAD_TYPE.to_string(),
        payload: b"payload".to_vec(),
        signatures: Vec::new(),
    };
    let before = envelope.signing_bytes();
    envelope.sign(key.signing_key(), key.key_id());
    assert_eq!(
        envelope.signing_bytes(),
        before,
        "signing changed what a signature covers"
    );
    assert_eq!(envelope.signatures.len(), 1);
}

/// 42 §3.10's fixed value, from the spec file rather than from a memory of it.
///
/// The constant is inside the signed bytes, so a typo in it would make every receipt this
/// workspace issues unverifiable by anyone else -- and unit tests that used the same constant on
/// both sides would all pass. Same shape as `ac_016`'s read of 41 §4 (H4-6 keeps that pattern
/// deliberately rare; this is the second and last place it is used).
#[test]
fn the_payload_type_is_the_one_42_3_10_fixes() {
    let spec = include_str!("../../../req/spec/40-architecture/42-data-model.md");
    // (sem: SEM-gx-witness-228) untranslated: "固定値" here must byte-match text inside
    // req/spec/40-architecture/42-data-model.md (untouchable canon), so it stays as the spec's own
    // Japanese rather than being translated.
    let row = spec
        .lines()
        .find(|l| l.contains("`payload_type`") && l.contains("固定値"))
        .expect("42 §3.10 has a payload_type row");
    assert!(
        row.contains(RECEIPT_PAYLOAD_TYPE),
        "42 §3.10's fixed value is not {RECEIPT_PAYLOAD_TYPE}: {row}"
    );
}

// ---------------------------------------------------------------------------
// One alphabet, one table
// ---------------------------------------------------------------------------

/// gx-witness spells base64 through `gx_core::b64` and owns no table of its own.
///
/// 44 §2.2 puts the payload in base64 and M2H1-4 puts the signature there too; two tables in one
/// workspace is how a receipt comes to have two JSON spellings. Read off the source, because the
/// property is "this crate does not contain an alphabet" and no runtime value shows that. Same
/// shape as the hash/codec scan `ac_014` performs across the workspace.
#[test]
fn gx_witness_names_no_alphabet_of_its_own() {
    const SOURCES: &[(&str, &str)] = &[
        ("dsse.rs", include_str!("../src/dsse.rs")),
        ("evidence.rs", include_str!("../src/evidence.rs")),
        ("keys.rs", include_str!("../src/keys.rs")),
        ("lib.rs", include_str!("../src/lib.rs")),
        ("provenance.rs", include_str!("../src/provenance.rs")),
        ("receipt.rs", include_str!("../src/receipt.rs")),
    ];
    for (name, src) in SOURCES {
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            "{name} contains a base64 alphabet; `gx_core::b64` is the one table"
        );
        assert!(
            !code.contains("abcdefghijklmnopqrstuvwxyz234567"),
            "{name} contains the base32 alphabet; `gx_core::Cid::to_text` is the one spelling"
        );
    }
}
