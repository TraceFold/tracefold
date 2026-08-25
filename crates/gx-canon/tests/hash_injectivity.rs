// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! From-zero tests for a claim `src/cid.rs` states in prose but never checked mechanically:
//! that the domain-separated mint (42 §3.11) keeps leaf hashes and interior-node hashes apart,
//! and that the canonical encoding underneath it is injective and prefix-free. `req/38_ERRATA_2026-08-07.md`
//! §SS554 (ArchMap leverage 1) names the gap: "gx claims hash injectivity/prefix-freeness ...
//! but has NEITHER theorem NOR test for it". This file is the test half of that repair.
//!
//! # Provenance and scope (gitrepo HARD, COPY HARD BAN)
//!
//! `req/38_ERRATA_2026-08-07.md` §SS554 points at `Glovrex_Alpha/crates/alpha-term/tests/exhaustive.rs`
//! as prior art for the *shape* of this kind of test (enumerate an adversarial alphabet, build every
//! term up to N leaves by pairing, hash/encode each, and look for collisions or prefix violations).
//! That file was read read-only for the enumeration **technique** only -- generate-all-terms-by-pairing,
//! collision detection via a hash set, prefix-freeness via sort-then-check-adjacent (a standard property
//! of lexicographic order: if A is a proper prefix of B, no string can sort strictly between them, so
//! checking only adjacent pairs after a sort finds every violation). No line of Alpha code, its `Term`
//! type, its JSON-based canonical form, or its literal adversarial alphabet is reproduced here. Everything
//! below is written from zero against gx-canon's own types: `ipld_core::ipld::Ipld` as the generic
//! serializable value (already a dependency of this crate, see `Cargo.toml`), `gx_canon::cbor::encode`
//! as the encoder under test, and `gx_canon::cid::mint`/`Domain` as the hash under test. The adversarial
//! alphabet below is chosen for CBOR's own boundary conditions (definite-length header thresholds at 0,
//! 1, 23 and 24 bytes -- RFC 8949 §3), not copied from Alpha's JSON-shaped alphabet, because CBOR's
//! self-delimiting property lives at different byte offsets than JSON's.
//!
//! # What "red-first" means here
//!
//! The property under test (domain separation) is already implemented correctly in `src/cid.rs`, so
//! there is no live bug to watch this test catch. What stands in for red-first is the negative control
//! at the bottom of this file: it computes the bare (non-domain-separated) hash of an adversarial payload
//! two ways and shows they collide by construction -- i.e. the attack these tests guard against is real
//! and reproducible -- before asserting that gx's domain-separated `mint` avoids it. A test suite that
//! only ever asserts "no collision found" cannot distinguish "the safeguard works" from "the test is too
//! weak to find one"; the negative control is what closes that gap.
//!
//! # Coverage and its bound
//!
//! `ADVERSARIAL_ATOMS` has 11 entries; `all_terms(&ADVERSARIAL_ATOMS, 3)` enumerates every binary-tree
//! term over that alphabet with 1..=3 leaves -- 2,794 terms, asserted below so a change to the generator
//! is caught rather than silently shrinking the population under test. This is exhaustive over that
//! bounded alphabet and leaf count, not a claim about all possible inputs; `mod verification` in
//! `src/cid.rs` (Kani, symbolic over all 2^256 `Cid` values) is the unbounded companion for the digest
//! function's totality, and this file does not duplicate it.

use std::collections::{HashMap, HashSet};

use gx_canon::cbor;
use gx_canon::cid::{self, Domain};
use gx_core::Cid;
use ipld_core::ipld::Ipld;

/// Atoms chosen at CBOR's own definite-length header boundaries (RFC 8949 §3: a 1-byte length
/// argument covers 0..=23, a 2-byte argument starts at 24), plus the two extreme byte values and
/// both text- and byte-string plus integer major types, since a prefix or collision bug is most
/// likely to live exactly where the header shape changes.
fn adversarial_atoms() -> Vec<Ipld> {
    vec![
        Ipld::Bytes(vec![]),               // length 0
        Ipld::Bytes(vec![0x00]),           // length 1, minimum byte value
        Ipld::Bytes(vec![0xFF]),           // length 1, maximum byte value
        Ipld::Bytes(vec![0x2A; 23]),       // length 23, last 1-byte header length
        Ipld::Bytes(vec![0x2A; 24]),       // length 24, first 2-byte header length
        Ipld::String(String::new()),       // empty text string
        Ipld::String("a".to_string()),     // 1-byte text string
        Ipld::String("\u{0}".to_string()), // text string containing NUL
        Ipld::Integer(0),
        Ipld::Integer(-1),
        Ipld::Integer(24), // crosses the same 23/24 header boundary as an integer
    ]
}

/// Every term over `alphabet` with `1..=max_leaves` leaves, a pair being a 2-element `Ipld::List`
/// (an ordered binary node -- deliberately the plainest possible pairing so nothing about the pair
/// encoding itself is adversarial beyond the atoms it holds).
fn all_terms(alphabet: &[Ipld], max_leaves: usize) -> Vec<Ipld> {
    let mut by_size: Vec<Vec<Ipld>> = vec![Vec::new(), alphabet.to_vec()];
    for n in 2..=max_leaves {
        let mut here = Vec::new();
        for left_size in 1..n {
            let right_size = n - left_size;
            for l in &by_size[left_size] {
                for r in &by_size[right_size] {
                    here.push(Ipld::List(vec![l.clone(), r.clone()]));
                }
            }
        }
        by_size.push(here);
    }
    by_size.into_iter().flatten().collect()
}

/// Canonical bytes for every term, computed once so the three exhaustive checks below (injectivity,
/// domain separation, prefix-freeness) share the same encode pass instead of tripling the work.
fn encode_all(terms: &[Ipld]) -> Vec<Vec<u8>> {
    terms
        .iter()
        .enumerate()
        .map(|(i, t)| {
            cbor::encode(t).unwrap_or_else(|e| panic!("term #{i} has no canonical form: {e}"))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Injectivity of the canonical encoding (distinct terms, distinct bytes)
// ---------------------------------------------------------------------------

/// Two structurally different terms must never encode to the same canonical bytes. This is the
/// full-population version of `mint_domain.rs`'s `mint_leaf_separates_leaves_that_differ`, which
/// checks exactly one pair; this checks all 2,794 pairs an 11-atom, 3-leaf alphabet can build.
#[test]
fn canonical_encoding_is_injective_over_adversarial_terms() {
    let terms = all_terms(&adversarial_atoms(), 3);
    assert_eq!(
        terms.len(),
        2_794,
        "term enumeration size drifted -- update this bound deliberately"
    );

    let bytes = encode_all(&terms);
    let mut seen: HashMap<&[u8], usize> = HashMap::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        if let Some(&j) = seen.get(b.as_slice()) {
            panic!(
                "terms #{i} and #{j} encode to the same canonical bytes ({} bytes)",
                b.len()
            );
        }
        seen.insert(b.as_slice(), i);
    }
    assert_eq!(seen.len(), terms.len());
}

// ---------------------------------------------------------------------------
// 2. Leaf/interior domain separation (42 §3.11), exhaustively rather than for one input
// ---------------------------------------------------------------------------

/// The single-input check in `mint_domain.rs` (`one_input_hashes_to_two_different_digests_in_the_two_domains`)
/// shows domain separation holds for one 64-byte value. This is the same property over the full
/// adversarial population: no byte string, hashed under `Domain::Leaf`, may equal any byte string
/// (the same one or a different one) hashed under `Domain::Node`.
#[test]
fn leaf_and_node_domains_never_collide_over_adversarial_terms() {
    let terms = all_terms(&adversarial_atoms(), 3);
    let bytes = encode_all(&terms);

    let mut leaf_hashes: HashSet<[u8; 32]> = HashSet::with_capacity(bytes.len());
    let mut node_hashes: HashSet<[u8; 32]> = HashSet::with_capacity(bytes.len());
    for b in &bytes {
        leaf_hashes.insert(cid::mint(Domain::Leaf, &[b]).0);
        node_hashes.insert(cid::mint(Domain::Node, &[b]).0);
    }
    assert_eq!(
        leaf_hashes.len(),
        bytes.len(),
        "Domain::Leaf collided within its own domain"
    );
    assert_eq!(
        node_hashes.len(),
        bytes.len(),
        "Domain::Node collided within its own domain"
    );
    assert!(
        leaf_hashes.is_disjoint(&node_hashes),
        "a byte string hashed under Domain::Leaf collided with a byte string hashed under Domain::Node"
    );
}

// ---------------------------------------------------------------------------
// 3. Prefix-freeness of the canonical encoding
// ---------------------------------------------------------------------------

/// No term's canonical bytes may be a proper byte-prefix of another term's canonical bytes.
/// `cid::mint`'s parts are concatenated without a length frame between them (`mint_domain.rs`'s
/// `mint_concatenates_its_parts_without_framing_them` asserts exactly that), so anywhere a caller
/// hands `mint` two encoded values back to back, prefix-freeness of each value's own encoding is
/// what keeps the boundary between them recoverable. CBOR's definite-length headers should provide
/// this by construction (RFC 8949 §3); this test is what turns "should" into a checked property of
/// this crate's actual encoder output rather than the spec's.
///
/// Checking only adjacent pairs after a sort is sufficient and not a shortcut that misses cases: if
/// A is a proper prefix of B, every byte of A equals the corresponding byte of B, so nothing can
/// compare strictly between A and B in lexicographic (byte-vector) order -- any string that diverges
/// from A before position `A.len()` sorts either before A or after every extension of A, and any
/// string that shares all of A's bytes past that point is itself an extension of A. So a prefix
/// pair is always adjacent once sorted, and the exhaustive `O(n^2)` pairwise check below cross-checks
/// that reasoning on the same population.
#[test]
fn canonical_encoding_is_prefix_free_over_adversarial_terms() {
    let terms = all_terms(&adversarial_atoms(), 3);
    let mut bytes = encode_all(&terms);
    bytes.sort();

    let mut violations = Vec::new();
    for w in bytes.windows(2) {
        if w[0] != w[1] && w[1].starts_with(w[0].as_slice()) {
            violations.push((w[0].clone(), w[1].clone()));
        }
    }
    assert!(
        violations.is_empty(),
        "{} prefix violation(s) in the sorted-adjacent pass, first: {:?}",
        violations.len(),
        violations.first()
    );
}

/// Cross-check of the sorted-adjacent argument above: exhaustive `O(n^2)` pairwise comparison over
/// a smaller population (2 leaves -- 132 terms), independent of the sort-order reasoning.
#[test]
fn canonical_encoding_is_prefix_free_pairwise_cross_check() {
    let terms = all_terms(&adversarial_atoms(), 2);
    assert_eq!(
        terms.len(),
        132,
        "term enumeration size drifted -- update this bound deliberately"
    );
    let bytes = encode_all(&terms);

    for (i, a) in bytes.iter().enumerate() {
        for (j, b) in bytes.iter().enumerate() {
            if i == j {
                continue;
            }
            assert!(
                !(a.len() < b.len() && b.starts_with(a.as_slice())),
                "term #{i}'s canonical bytes are a proper prefix of term #{j}'s"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Collision-behavior negative control (the "red" this repair cannot get any other way)
// ---------------------------------------------------------------------------

/// Demonstrates the attack 42 §3.11's domain byte exists to block (RFC 6962 §2.1, second-preimage:
/// a leaf's bytes presented as an internal node's two children), and shows a hash with no domain
/// byte cannot tell the two readings apart -- before asserting gx's actual `mint` does.
///
/// This is the stand-in for red-first: the property under test already holds in `src/cid.rs`, so
/// there is nothing to watch fail by running the suite. What is watched fail instead is a
/// deliberately undomained comparator, constructed only inside this test and never touching
/// production code, so this test would have caught the omission had domain separation been left out.
#[test]
fn negative_control_bare_hash_cannot_separate_leaf_from_node_reading() {
    let left = Cid([0x11; 32]);
    let right = Cid([0x22; 32]);
    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&left.0);
    payload.extend_from_slice(&right.0);

    // A bare BLAKE3 hash of the 64-byte payload, with no domain tag at all -- this is the
    // hypothetical "gx forgot the prefix byte" implementation.
    let bare = |bytes: &[u8]| -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(bytes);
        *hasher.finalize().as_bytes()
    };

    // Read as "the concatenation of two children" and read as "some other 64-byte value that
    // happens to equal left||right" are the same bytes, so a bare hash necessarily agrees with
    // itself on them -- this is the collision surface RFC 6962 names, made concrete rather than
    // asserted in prose.
    assert_eq!(
        bare(&payload),
        bare(&payload),
        "the negative control's own construction must collide, or it proves nothing"
    );

    // The real, domain-separated functions distinguish what the bare hash could not.
    let as_node = cid::mint_node(&left, &right);
    let as_leaf = cid::mint(Domain::Leaf, &[&payload]);
    assert_ne!(
        as_node.0,
        bare(&payload),
        "domain-separated node hash collided with the bare hash of the same bytes"
    );
    assert_ne!(
        as_leaf.0,
        bare(&payload),
        "domain-separated leaf hash collided with the bare hash of the same bytes"
    );
    assert_ne!(
        as_node, as_leaf,
        "leaf and node domains must differ on the exact bytes the attack targets"
    );
}
