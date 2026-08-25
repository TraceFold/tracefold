// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The domain-separated mint API (**E-M2-12**, `req/38_ERRATA_2026-08-07.md` §8).
//!
//! No acceptance criterion is claimed here. AC-021..AC-024 are gx-log's and this file is the
//! layer underneath them: 42 §3.11 writes the ledger's two hashes as
//!
//! ```text
//! leaf_hash = BLAKE3(0x00 || canonical_dagcbor(LedgerLeaf))
//! node_hash = BLAKE3(0x01 || left_hash || right_hash)
//! ```
//!
//! and until this hand there was no way to produce either without reaching for the digest
//! directly. 41 §6 forbids that — "every canonical encode goes through gx-canon only" (sem: SEM-gx-canon-100) — and AC-014 checks
//! it mechanically, so a gx-log that hashed its own leaves would either violate the rule or make
//! the check that guards it stop meaning anything. E-M2-12 rules the other way round: gx-canon
//! gains the mint, gx-log calls it.
//!
//! # What is checked, and against what
//!
//! The expected values are computed here with `blake3` directly. That is deliberate and it is the
//! only place in the workspace besides `cid.rs` where it is allowed (AC-014 exempts gx-canon's own
//! tests, for the same reason `ac_011` calls the digest by hand: a check that goes through the
//! thing under test proves the thing agrees with itself). So each assertion below is
//! `mint_*` against an independently written expression of the formula 42 §3.11 states, not
//! against another call of `mint_*`.
//!
//! # What this API is not
//!
//! It is not a second road to an *identity*. [`gx_canon::cid::compute`] takes an `IdentityView`
//! and answers "what is this value" (sem: SEM-gx-canon-101) (42 §1.1, §1.3); the mint takes bytes and a domain tag and
//! answers "what is this position in a Merkle tree" (42 §3.11). The two never produce the same
//! digest for the same input — the domain byte is exactly what stops them — and
//! `ac_014.rs` pins the set of functions that may take a digest at all so that a third road has to
//! be declared before it can exist.

use gx_canon::cbor;
use gx_canon::cid::{self, Domain};
use gx_core::Cid;
use serde::Serialize;

/// A stand-in for 42 §3.11's `LedgerLeaf` — the real one is gx-log's (hand 2) and gx-canon may not
/// depend on it. Field order is bytewise-sorted so the encoder's output is canonical as written
/// (42 §2.1-2); the mint refuses anything that is not, which `a_value_with_no_canonical_form`
/// checks below.
#[derive(Debug, Serialize)]
struct Leaf {
    index: u64,
    receipt_digest: Cid,
    transformation: Cid,
}

fn leaf(index: u64) -> Leaf {
    Leaf {
        index,
        receipt_digest: Cid([0xAA; 32]),
        transformation: Cid([0xBB; 32]),
    }
}

/// The formula of 42 §3.11, written out here rather than borrowed from the code under test.
fn blake3_of(domain: u8, parts: &[&[u8]]) -> Cid {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[domain]);
    for part in parts {
        hasher.update(part);
    }
    Cid(*hasher.finalize().as_bytes())
}

// ---------------------------------------------------------------------------
// The two domain bytes
// ---------------------------------------------------------------------------

/// 42 §3.11 verbatim: "the prefix byte (`0x00`=leaf, `0x01`=internal node)" (sem: SEM-gx-canon-102).
///
/// A constant rather than a literal at each call site, because the whole purpose of the two bytes
/// is that leaf hashing and node hashing can never be made to agree by accident (RFC 6962 §2.1,
/// second-preimage resistance). A value repeated in two files is a value that can drift in one.
#[test]
fn the_two_domains_carry_the_bytes_42_3_11_names() {
    assert_eq!(Domain::Leaf.byte(), 0x00);
    assert_eq!(Domain::Node.byte(), 0x01);
    assert_ne!(Domain::Leaf, Domain::Node);
}

/// The reason the prefixes exist at all: the same bytes hashed in the two domains must differ.
///
/// Without them, a 64-byte leaf could be presented as an internal node whose two children are its
/// halves, and an inclusion proof for a leaf that was never appended could be constructed from a
/// tree that never contained it. RFC 6962 §2.1 states the attack; 42 §3.11 inherits the fix.
#[test]
fn one_input_hashes_to_two_different_digests_in_the_two_domains() {
    let bytes = [7u8; 64];
    assert_ne!(
        cid::mint(Domain::Leaf, &[&bytes]),
        cid::mint(Domain::Node, &[&bytes]),
    );
}

// ---------------------------------------------------------------------------
// leaf_hash = BLAKE3(0x00 || canonical_dagcbor(leaf))
// ---------------------------------------------------------------------------

#[test]
fn mint_leaf_is_the_zero_byte_followed_by_the_canonical_form() {
    let value = leaf(42);
    let encoded = cbor::encode(&value).expect("the leaf has a canonical form");

    assert_eq!(
        cid::mint_leaf(&value).expect("mint"),
        blake3_of(0x00, &[&encoded]),
    );
}

/// The prefix is actually applied — a mint that forgot it would equal the bare digest.
#[test]
fn mint_leaf_is_not_the_bare_digest_of_the_canonical_form() {
    let value = leaf(42);
    let encoded = cbor::encode(&value).expect("the leaf has a canonical form");

    let mut hasher = blake3::Hasher::new();
    hasher.update(&encoded);
    let bare = Cid(*hasher.finalize().as_bytes());

    assert_ne!(cid::mint_leaf(&value).expect("mint"), bare);
}

/// Two leaves that differ in one field must not share a hash; the encoder is what carries that,
/// and the mint must not flatten it.
#[test]
fn mint_leaf_separates_leaves_that_differ() {
    assert_ne!(
        cid::mint_leaf(&leaf(41)).expect("mint"),
        cid::mint_leaf(&leaf(42)).expect("mint"),
    );
}

/// Determinism, which is the property the whole ledger rests on: the same leaf, appended twice,
/// hashes the same or an inclusion proof means nothing.
#[test]
fn mint_leaf_is_deterministic() {
    assert_eq!(
        cid::mint_leaf(&leaf(42)).expect("mint"),
        cid::mint_leaf(&leaf(42)).expect("mint"),
    );
}

/// A value with no canonical form has no leaf hash, and the mint says so rather than hashing an
/// approximation of it (42 §2.1-4 keeps floats out; `req/26` §3 is the posture).
#[test]
fn mint_leaf_refuses_a_value_with_no_canonical_form() {
    #[derive(Serialize)]
    struct HasAFloat {
        value: f64,
    }

    let err = cid::mint_leaf(&HasAFloat { value: 1.5 }).expect_err("a float has no canonical form");
    assert!(
        matches!(
            err,
            gx_canon::Error::NotCanonicalizable(_) | gx_canon::Error::Encode(_)
        ),
        "unexpected refusal: {err}"
    );
}

// ---------------------------------------------------------------------------
// node_hash = BLAKE3(0x01 || left_hash || right_hash)
// ---------------------------------------------------------------------------

#[test]
fn mint_node_is_the_one_byte_followed_by_both_children() {
    let left = Cid([1u8; 32]);
    let right = Cid([2u8; 32]);

    assert_eq!(
        cid::mint_node(&left, &right),
        blake3_of(0x01, &[&left.0, &right.0]),
    );
}

/// Order carries meaning: `node(l, r)` and `node(r, l)` are different positions in the tree, and a
/// verifier that could swap them could move a leaf from one side to the other.
#[test]
fn mint_node_is_not_commutative() {
    let left = Cid([1u8; 32]);
    let right = Cid([2u8; 32]);

    assert_ne!(cid::mint_node(&left, &right), cid::mint_node(&right, &left));
}

/// The general form and the two named ones agree — the named functions are spellings of `mint`,
/// not second implementations of it.
#[test]
fn the_named_mints_agree_with_the_general_one() {
    let value = leaf(3);
    let encoded = cbor::encode(&value).expect("canonical");
    let left = Cid([9u8; 32]);
    let right = Cid([8u8; 32]);

    assert_eq!(
        cid::mint_leaf(&value).expect("mint"),
        cid::mint(Domain::Leaf, &[&encoded]),
    );
    assert_eq!(
        cid::mint_node(&left, &right),
        cid::mint(Domain::Node, &[&left.0, &right.0]),
    );
}

/// Concatenation, not framing: `mint(d, [a, b])` hashes `d || a || b` and nothing else.
///
/// Stated as a test because it is a property a caller depends on when it splits a value across
/// parts — and because a mint that inserted a length prefix would still pass every assertion
/// above, while disagreeing with 42 §3.11's `0x01 || left_hash || right_hash`.
#[test]
fn mint_concatenates_its_parts_without_framing_them() {
    let whole = [3u8, 1, 4, 1, 5, 9, 2, 6];
    assert_eq!(
        cid::mint(Domain::Node, &[&whole]),
        cid::mint(Domain::Node, &[&whole[..3], &whole[3..]]),
    );
}

/// The algorithm name gx-log's AC-024 compares against a Rekor v2 reference vector (E-M2-9).
///
/// It lives in gx-canon because gx-canon is the crate that owns the digest: a name declared where
/// the hash is not called could disagree with the hash and nothing would notice. 35 DR-3 is the
/// ruling it states.
#[test]
fn the_digest_algorithm_is_named_where_the_digest_is_taken() {
    assert_eq!(cid::DIGEST_ALGORITHM, "BLAKE3-256");
    assert_eq!(cid::mint(Domain::Leaf, &[b"anything"]).0.len(), 32);
}
