//! AC-022 (FR-022) — inclusion proofs, and what happens to a tampered one.
//!
//! AC-022 逐語: 「Given: 100件commit済みのgx-log。When: seq=42のentryについてinclusion proofを生成し
//! `verify_inclusion(proof, root, entry)`を呼ぶ。Then: `Ok(true)`。entryを改ざんした場合は
//! `Ok(false)`または`Err`。」 判定方法欄: 「property（ランダムseq×複数root状態）」.
//!
//! The literal case (100 entries, seq 42) is the first test; the rest is the property the AC's
//! own 判定方法 column asks for, plus the negatives that make a positive worth anything. A verifier
//! that returns `Ok(true)` for everything passes the literal case.
//!
//! # What is being verified
//!
//! `verify_inclusion` recomputes the leaf hash from the entry it is handed (42 §3.11:
//! `leaf_hash = BLAKE3(0x00 || canonical_dagcbor(LedgerLeaf))`) and walks the audit path up to a
//! root. It never reads `entry.leaf_cid` -- if it did, an entry whose fields were edited and whose
//! cached hash was edited to match would verify, which is exactly the tamper AC-022 asks about.

use gx_core::{Cid, Timestamp, TransformationId};
use gx_log::proof::{prove_inclusion, prove_inclusion_at, verify_inclusion};
use gx_log::tile::TileLog;
use proptest::prelude::*;

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// A log of `n` entries, each distinguishable from the others by eye in a failure message.
fn log_of(n: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        log.append(
            TransformationId(cid(i)),
            cid(1_000_000 + i),
            Timestamp(i as i64),
        )
        .expect("a leaf of ids and digests has a canonical form");
    }
    log
}

// ---------------------------------------------------------------------------
// The literal case of AC-022
// ---------------------------------------------------------------------------

#[test]
fn ac_022_seq_42_of_a_hundred_entries_verifies() {
    let log = log_of(100);
    let root = log.root().expect("a log of 100 has a root");
    let entry = log.entry(42).expect("seq 42 exists").clone();
    let proof = prove_inclusion(&log, 42).expect("proof");

    assert_eq!(proof.leaf_index, 42);
    assert_eq!(proof.tree_size, 100);
    assert_eq!(verify_inclusion(&proof, &root, &entry), Ok(true));
}

/// A tampered entry does not verify. 「entryを改ざんした場合は `Ok(false)` または `Err`」.
///
/// Each field of the leaf is edited in turn, because a verifier that recomputed only part of the
/// leaf would still reject the others.
#[test]
fn ac_022_a_tampered_entry_does_not_verify() {
    let log = log_of(100);
    let root = log.root().expect("root");
    let proof = prove_inclusion(&log, 42).expect("proof");
    let original = log.entry(42).expect("seq 42").clone();

    let mut swapped_transformation = original.clone();
    swapped_transformation.transformation = TransformationId(cid(9_999));
    assert_eq!(
        verify_inclusion(&proof, &root, &swapped_transformation),
        Ok(false)
    );

    let mut swapped_receipt = original.clone();
    swapped_receipt.receipt_digest = cid(9_999);
    assert_eq!(verify_inclusion(&proof, &root, &swapped_receipt), Ok(false));

    let mut swapped_index = original.clone();
    swapped_index.index = 43;
    assert_eq!(verify_inclusion(&proof, &root, &swapped_index), Ok(false));

    // The cached hash is not an input. Editing the entry AND its stored `leaf_cid` to agree is the
    // tamper a verifier that trusted the cache would accept.
    let mut consistent_lie = original.clone();
    consistent_lie.transformation = TransformationId(cid(9_999));
    consistent_lie.leaf_cid = gx_log::tile::leaf_hash(&consistent_lie.leaf()).expect("hash");
    assert_eq!(verify_inclusion(&proof, &root, &consistent_lie), Ok(false));
}

/// A tampered *path* does not verify either -- the proof is as much an input as the entry.
#[test]
fn ac_022_a_tampered_audit_path_does_not_verify() {
    let log = log_of(100);
    let root = log.root().expect("root");
    let entry = log.entry(42).expect("seq 42").clone();
    let proof = prove_inclusion(&log, 42).expect("proof");

    let mut edited = proof.clone();
    edited.audit_path[0] = cid(4_242);
    assert_eq!(verify_inclusion(&edited, &root, &entry), Ok(false));

    // Too short: the walk ends before the tree does.
    let mut truncated = proof.clone();
    truncated.audit_path.pop();
    assert_eq!(verify_inclusion(&truncated, &root, &entry), Ok(false));

    // Too long: the walk runs past the root.
    let mut extended = proof.clone();
    extended.audit_path.push(cid(1));
    assert_eq!(verify_inclusion(&extended, &root, &entry), Ok(false));

    // A path whose order is reversed reaches a different root, unless the tree is trivial.
    let mut reversed = proof.clone();
    reversed.audit_path.reverse();
    assert_eq!(verify_inclusion(&reversed, &root, &entry), Ok(false));
}

/// A proof against one tree size does not verify against another tree's root (a truncated log).
///
/// This is the shape of the attack a consistency proof exists to stop, seen from the inclusion
/// side: a log that dropped its last fifty entries has a valid-looking root, and a proof issued
/// against the full tree must not verify against it.
#[test]
fn ac_022_a_proof_does_not_verify_against_a_truncated_tree() {
    let full = log_of(100);
    let entry = full.entry(42).expect("seq 42").clone();
    let proof_at_100 = prove_inclusion(&full, 42).expect("proof");

    let truncated_root = full.root_at(50).expect("a 50-leaf root");
    assert_eq!(
        verify_inclusion(&proof_at_100, &truncated_root, &entry),
        Ok(false)
    );

    // And the other way round: a proof issued when the tree was 50 leaves does not verify against
    // the tree of 100. Both roots are honestly produced; the proof names which one it is for.
    let proof_at_50 = prove_inclusion_at(&full, 42, 50).expect("proof at an earlier size");
    let root_at_100 = full.root().expect("root");
    assert_eq!(proof_at_50.tree_size, 50);
    assert_eq!(
        verify_inclusion(&proof_at_50, &root_at_100, &entry),
        Ok(false)
    );
    assert_eq!(
        verify_inclusion(&proof_at_50, &full.root_at(50).expect("root"), &entry),
        Ok(true)
    );
}

/// An index outside the tree has no proof.
#[test]
fn ac_022_an_index_past_the_end_has_no_proof() {
    let log = log_of(100);
    assert!(prove_inclusion(&log, 100).is_err());
    assert!(prove_inclusion(&log, u64::MAX).is_err());
    assert!(prove_inclusion(&TileLog::new(), 0).is_err());
}

/// The one-leaf tree: the root is the leaf hash and the path is empty. Not a degenerate case to be
/// tolerated -- it is the first state every log passes through.
#[test]
fn ac_022_the_single_leaf_tree_verifies_with_an_empty_path() {
    let log = log_of(1);
    let root = log.root().expect("root");
    let entry = log.entry(0).expect("the only entry").clone();
    let proof = prove_inclusion(&log, 0).expect("proof");

    assert!(proof.audit_path.is_empty());
    assert_eq!(root, entry.leaf_cid);
    assert_eq!(verify_inclusion(&proof, &root, &entry), Ok(true));
}

// ---------------------------------------------------------------------------
// The property AC-022's 判定方法 column asks for: random seq x several tree sizes
// ---------------------------------------------------------------------------

proptest! {
    /// Every leaf of every tree size verifies against that tree's root.
    ///
    /// Sizes 1..=64 rather than only 100, because the interesting cases are the ragged ones: RFC
    /// 6962 splits at the largest power of two below `n`, so a tree of 2^k is the *easy* shape and
    /// a tree of 2^k+1 is where an off-by-one lives.
    #[test]
    fn ac_022_every_leaf_of_every_size_verifies(size in 1u64..=64, seed in 0u64..1_000) {
        let log = log_of(size);
        let root = log.root().expect("a non-empty log has a root");
        let index = seed % size;

        let entry = log.entry(index).expect("index is inside the tree").clone();
        let proof = prove_inclusion(&log, index).expect("proof");

        prop_assert_eq!(proof.leaf_index, index);
        prop_assert_eq!(proof.tree_size, size);
        prop_assert_eq!(verify_inclusion(&proof, &root, &entry), Ok(true));
    }

    /// The proof of one leaf does not verify another leaf -- for any pair, at any size.
    #[test]
    fn ac_022_a_proof_is_about_the_leaf_it_names(size in 2u64..=64, a in 0u64..64, b in 0u64..64) {
        let log = log_of(size);
        let root = log.root().expect("root");
        let (i, j) = (a % size, b % size);
        prop_assume!(i != j);

        let proof = prove_inclusion(&log, i).expect("proof");
        let other = log.entry(j).expect("inside").clone();
        prop_assert_eq!(verify_inclusion(&proof, &root, &other), Ok(false));
    }

    /// A wrong root rejects, whatever the tree.
    #[test]
    fn ac_022_a_wrong_root_rejects(size in 1u64..=64, noise in 0u64..1_000) {
        let log = log_of(size);
        let entry = log.entry(0).expect("first").clone();
        let proof = prove_inclusion(&log, 0).expect("proof");
        prop_assert_eq!(verify_inclusion(&proof, &cid(500_000 + noise), &entry), Ok(false));
    }
}
