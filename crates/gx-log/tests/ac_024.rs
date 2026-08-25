// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-024 (FR-024) — the tile layout against a Rekor v2 reference vector, difference included.
//!
//! AC-024 verbatim: "Given: one tile gx-log produced. When: a test-vector comparison is run
//! against the Rekor v2 tile spec's main fields (tile height, hash algorithm, leaf encoding).
//! Then: the corresponding fields agree." FR-024 itself is a SHOULD. (sem: SEM-gx-log-118)
//!
//! # The erratum this file implements
//!
//! **E-M2-9** (`req/38_ERRATA_2026-08-07.md` §8): "with gx=BLAKE3 and Rekor v2=SHA-256, 'hash
//! algorithm agreement' is impossible in principle. erratum = assert agreement on the structural
//! fields (tile height/leaf encoding) and mechanically assert the hash algorithm as 'a declared
//! difference' -- stating the difference beats faking agreement" (sem: SEM-gx-log-119).
//!
//! So this file asserts two different things, and keeps them apart:
//!
//! * the **structure** agrees -- tile height, the width that follows from it, level 0 meaning
//!   leaves, the two domain-separation bytes, the digest length;
//! * the **hash algorithm** differs, and the difference is asserted as such. A file that quietly
//!   dropped the comparison would read the same as one where the algorithms happened to agree.
//!
//! # What "leaf encoding" (sem: SEM-gx-log-120) does and does not mean here (**E-M2-27**)
//!
//! `req/38_ERRATA_2026-08-07.md` §15 verbatim: "(a) the substance of the 'leaf encoding' assert is
//! a domain-byte check, and that is a different thing from the upstream's 'leaf encoding' (a
//! big-endian uint16 length-prefixed entry bundle -- gx does not implement it) -- **correct the
//! assert's name to match its substance** (the N-1 disease: a name that lies) + state plainly that
//! the entry-bundle type is outside gx's claim" (sem: SEM-gx-log-121).
//!
//! The upstream's "leaf encoding" (sem: SEM-gx-log-122) is a *record format*: how one entry's bytes are laid out inside a
//! tile's data file, length-prefixed with a big-endian `uint16`. **gx does not implement it and does
//! not claim it.** A gx leaf is `BLAKE3(0x00 || canonical_dagcbor(LedgerLeaf))` and the tile holds
//! digests, so there is no entry bundle in this workspace for a comparison to be made against.
//!
//! What this file compares under that heading is the **domain separation byte** -- `0x00` for a leaf
//! and `0x01` for an internal node, RFC 6962 §2.1's, which 42 §3.11 says it reuses verbatim. The
//! test that does it was called `ac_024_leaf_encoding_uses_the_reference_domain_bytes` until this
//! fix, a name claiming an agreement about the entry bundle that no assertion in it had ever made
//! (req/08 N-1: a name that is a lie). It is now
//! `ac_024_the_domain_separation_bytes_match_the_reference`, and the absent comparison is printed
//! rather than left to be inferred.
//!
//! # Where the reference vector comes from, and what that is worth
//!
//! 🔴 The reference below is **restated from gx's own canon**, not fetched from the upstream
//! document. 42 §3.11 states the design it follows ("the tile design follows Rekor v2 (Trillian-
//! independent, tile-backed, research/02 §3)", `width: u16` 1..=256 "the default tile width is 256",
//! `level: u8` "0=the leaf layer", and RFC 6962 §2.1's `0x00`/`0x01` domain separation "reused as-is") (sem: SEM-gx-log-123),
//! research/02 §3 records Rekor v2 as tile-backed and SHA-256, and 35 DR-3 fixes gx on BLAKE3.
//! No C2SP `tlog-tiles` or Rekor v2 specification text was fetched or read while writing this
//! file, so what it compares is gx's implementation against gx's reading of the upstream design --
//! which catches an implementation that drifts from the canon, and would not catch a canon that
//! misreads the upstream. That limit is raised in req/51 §4 rather than papered over; closing it
//! means fetching the upstream vector, which is an observation task and not this hand's.

use gx_canon::cid::{Domain, DIGEST_ALGORITHM};
use gx_core::{Cid, Timestamp, TransformationId};
use gx_log::tile::{TileLog, TILE_WIDTH};

/// The fields AC-024 names, as gx's canon states the Rekor v2 / C2SP tlog-tiles design.
///
/// See the module docs for the provenance of every value here.
struct ReferenceVector {
    /// "tile height" (sem: SEM-gx-log-124) in the upstream's vocabulary: the log2 of the number of leaves a full tile
    /// covers. 42 §3.11's "the default tile width is 256" is 2^8.
    tile_height_log2: u32,
    /// The same number as a count, which is what 42 §3.11's `Tile.width` holds.
    tile_width: u16,
    /// RFC 6962 §2.1 domain separation, which 42 §3.11 reuses verbatim.
    ///
    /// **Not** the upstream's "leaf encoding" (sem: SEM-gx-log-125), which is the length-prefixed entry bundle gx does not
    /// implement (E-M2-27; see the module docs).
    leaf_domain_byte: u8,
    node_domain_byte: u8,
    /// Level 0 is the leaf layer (42 §3.11: `level: u8` "0=the leaf layer, N=an internal layer") (sem: SEM-gx-log-126).
    ///
    /// gx counts one level per doubling; the upstream counts one level per **eight** doublings. The
    /// two agree at 0 and nowhere else -- see `tile.rs`'s note on `Tile.level` (E-M2-27).
    level_zero_is_leaves: bool,
    /// research/02 §3 / 35 DR-3: the one field where gx and the reference are expected to differ.
    hash_algorithm: &'static str,
    /// Both are 256-bit digests, which is why the tile *shape* transfers at all.
    digest_len_bytes: usize,
}

const REKOR_V2: ReferenceVector = ReferenceVector {
    tile_height_log2: 8,
    tile_width: 256,
    leaf_domain_byte: 0x00,
    node_domain_byte: 0x01,
    level_zero_is_leaves: true,
    hash_algorithm: "SHA-256",
    digest_len_bytes: 32,
};

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn log_of(n: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        log.append(
            TransformationId(cid(i)),
            cid(1_000_000 + i),
            Timestamp(i as i64),
        )
        .expect("canonical");
    }
    log
}

// ---------------------------------------------------------------------------
// The structural fields: these must agree
// ---------------------------------------------------------------------------

/// "tile height" (sem: SEM-gx-log-127) and the width it implies.
#[test]
fn ac_024_tile_height_matches_the_reference() {
    assert_eq!(TILE_WIDTH, REKOR_V2.tile_width);
    assert_eq!(
        TILE_WIDTH.trailing_zeros(),
        REKOR_V2.tile_height_log2,
        "a tile width of {TILE_WIDTH} is not 2^{}",
        REKOR_V2.tile_height_log2
    );
}

/// A full tile is exactly `tile_width` hashes; a partial tile is shorter and says so.
///
/// 42 §3.11 admits partial tiles ("1..=256 (the default tile width is 256, a partial tile is also allowed)") (sem: SEM-gx-log-128), which is what makes
/// a log readable before it reaches a tile boundary.
#[test]
fn ac_024_a_generated_tile_has_the_reference_shape() {
    let log = log_of(300);

    let full = log.tile(0, 0).expect("the first leaf tile");
    assert_eq!(full.level, 0);
    assert_eq!(full.index, 0);
    assert_eq!(full.width, REKOR_V2.tile_width);
    assert_eq!(full.hashes.len(), usize::from(full.width));

    let partial = log.tile(0, 1).expect("the second, partial leaf tile");
    assert_eq!(partial.level, 0);
    assert_eq!(partial.index, 1);
    assert_eq!(partial.width, 44, "300 leaves less one full tile of 256");
    assert_eq!(partial.hashes.len(), usize::from(partial.width));

    assert!(
        log.tile(0, 2).is_none(),
        "a tile past the end of the log is absent, not empty"
    );
}

/// Level 0 is the leaf layer, and the hashes it holds are the leaf hashes themselves.
#[test]
fn ac_024_level_zero_is_the_leaf_layer() {
    let log = log_of(10);
    let tile = log.tile(0, 0).expect("tile");

    assert_eq!(
        tile.level == 0,
        REKOR_V2.level_zero_is_leaves,
        "the reference vector puts the leaves at level 0 and this tile does not"
    );
    assert_eq!(tile.hashes.len(), 10);
    for (i, hash) in tile.hashes.iter().enumerate() {
        let entry = log.entry(i as u64).expect("entry");
        assert_eq!(*hash, entry.leaf_cid);
    }
}

/// Level N holds the roots of the complete subtrees of 2^N leaves, and nothing else.
///
/// The ragged tail is deliberately absent: a partial subtree has no level-N hash yet, because
/// appending to it would change one. That is what makes a tile immutable once written, which is
/// what makes it cacheable -- 42 §3.11's "a fixed-size chunk that a CDN can cache" (sem: SEM-gx-log-129).
#[test]
fn ac_024_an_internal_level_holds_only_complete_subtrees() {
    let log = log_of(10);

    let level_1 = log.tile(1, 0).expect("tile");
    assert_eq!(level_1.width, 5, "10 leaves make five complete pairs");

    let level_2 = log.tile(2, 0).expect("tile");
    assert_eq!(level_2.width, 2, "10 leaves make two complete quads");

    let level_3 = log.tile(3, 0).expect("tile");
    assert_eq!(level_3.width, 1, "10 leaves make one complete octet");

    assert!(
        log.tile(4, 0).is_none(),
        "10 leaves make no complete subtree of 16"
    );
}

/// The two domain bytes are RFC 6962's, inherited unchanged (**E-M2-27**: this is what the
/// "leaf encoding" (sem: SEM-gx-log-130) line of AC-024 is actually able to compare).
///
/// The constants come from gx-canon, which is where the hash is actually taken; that a leaf hash
/// really is `BLAKE3(0x00 || …)` and a node hash `BLAKE3(0x01 || …)` is checked there, in
/// `tests/mint_domain.rs`, because AC-014 keeps the digest out of every crate but that one.
///
/// The upstream's own "leaf encoding" (sem: SEM-gx-log-131) -- the big-endian `uint16` length-prefixed entry bundle -- is
/// **outside gx's claim** and is printed as such. gx stores digests in tiles and has no entry bundle
/// at all, so there is nothing here to agree or disagree with it.
#[test]
fn ac_024_the_domain_separation_bytes_match_the_reference() {
    assert_eq!(Domain::Leaf.byte(), REKOR_V2.leaf_domain_byte);
    assert_eq!(Domain::Node.byte(), REKOR_V2.node_domain_byte);
    println!(
        "AC024_DOMAIN_BYTES leaf=0x{:02x} node=0x{:02x}  \
         AC024_ENTRY_BUNDLE_ENCODING=not_implemented_by_gx_and_not_claimed (E-M2-27)",
        REKOR_V2.leaf_domain_byte, REKOR_V2.node_domain_byte
    );
}

/// Both sides are 256-bit digests. That equality is why the tile geometry transfers at all despite
/// the algorithm below not matching.
#[test]
fn ac_024_the_digest_length_matches() {
    let log = log_of(1);
    let root = log.root().expect("root");
    assert_eq!(root.0.len(), REKOR_V2.digest_len_bytes);
}

// ---------------------------------------------------------------------------
// The one field that cannot agree (E-M2-9)
// ---------------------------------------------------------------------------

/// The hash algorithm differs, and the difference is the assertion.
///
/// AC-024 asks for "the corresponding fields agree" (sem: SEM-gx-log-132) over a list that includes "hash algorithm". gx is
/// BLAKE3 (35 DR-3 DEFAULT) and Rekor v2 is SHA-256; no implementation makes those equal, and an
/// implementation that reported them as equal would be lying about the one property a verifier of
/// somebody else's log would need first. E-M2-9 rules the comparison inverted here: the mismatch
/// is asserted, named, and printed, so that a future change to either side fails this test instead
/// of silently making the log look interoperable.
#[test]
fn ac_024_the_hash_algorithm_is_a_declared_difference() {
    assert_eq!(
        DIGEST_ALGORITHM, "BLAKE3-256",
        "gx's digest is 35 DR-3's BLAKE3; if this changed, the difference below changed with it"
    );
    assert_ne!(
        DIGEST_ALGORITHM, REKOR_V2.hash_algorithm,
        "gx and Rekor v2 are recorded as using the same hash algorithm. Either 35 DR-3 changed or \
         this reference vector is wrong -- E-M2-9 rules this a *declared difference*, and a \
         declared difference that stopped being one is a finding, not a pass."
    );
    println!(
        "AC024_HASH_ALGORITHM_DIFFERS gx={DIGEST_ALGORITHM} rekor_v2={} (E-M2-9)",
        REKOR_V2.hash_algorithm
    );
}

/// The consequence, stated so nobody reads the structural agreement as interoperability.
///
/// A Rekor v2 client handed a gx tile would parse its shape and compute a different root. FR-024
/// is a SHOULD about tile *format* (sem: SEM-gx-log-133); it is not a claim that the two logs verify each other, and 45
/// §4.1's overclaim rule applies to this file as much as to a README.
#[test]
fn ac_024_structural_correspondence_is_not_interoperability() {
    let structural_fields_agree = TILE_WIDTH == REKOR_V2.tile_width
        && Domain::Leaf.byte() == REKOR_V2.leaf_domain_byte
        && Domain::Node.byte() == REKOR_V2.node_domain_byte;
    let hashes_agree = DIGEST_ALGORITHM == REKOR_V2.hash_algorithm;

    assert!(structural_fields_agree);
    assert!(!hashes_agree);
    println!("AC024_STRUCTURAL_MATCH=true AC024_ROOT_INTEROPERABLE=false");
}
