//! AC-023 (FR-023) — consistency proofs: the new tree still contains the old one.
//!
//! AC-023 逐語: 「Given: root_0（50件）とroot_1（root_0を含む形で100件へ追記後）。When:
//! `verify_consistency(root_0, root_1)`を呼ぶ。Then: `Ok(true)`。root_1を改変したものでは
//! `Ok(false)`。」 判定方法欄: 「property」.
//!
//! # The signature is the erratum, not the behaviour
//!
//! **E-M2-8** (`req/38_ERRATA_2026-08-07.md` §8): 「`verify_consistency` は `ConsistencyProof`
//! 引数を取る形が正・AC の 2 引数呼びは 34 erratum」. Two roots alone carry no information about
//! how one grew into the other -- 42 §3.11 defines `ConsistencyProof { old_size, new_size, path }`
//! precisely because the check needs the intermediate hashes. A two-argument
//! `verify_consistency(root_0, root_1)` could only ever return `true`, which is why the AC as
//! written cannot be implemented and why the erratum is a correction rather than a relaxation.
//! Everything else in AC-023 -- the 50/100 setup, the `Ok(true)`, the `Ok(false)` on a modified
//! root -- is implemented literally below.
//!
//! # Why this matters more than inclusion
//!
//! An inclusion proof says an entry is in *a* tree. A consistency proof is what makes the log
//! append-only in a sense a reader can check: it fails exactly when the operator rewrote history,
//! which is the failure AC-021's API scan cannot see (nothing stops a *storage layer* from being
//! rebuilt from scratch).

use gx_core::{Cid, Timestamp, TransformationId};
use gx_log::proof::{prove_consistency, verify_consistency};
use gx_log::tile::TileLog;
use proptest::prelude::*;

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
// The literal case of AC-023
// ---------------------------------------------------------------------------

#[test]
fn ac_023_fifty_grown_to_a_hundred_is_consistent() {
    let log = log_of(100);
    let root_0 = log.root_at(50).expect("a 50-leaf root");
    let root_1 = log.root().expect("the 100-leaf root");

    let proof = prove_consistency(&log, 50, 100).expect("proof");
    assert_eq!(proof.old_size, 50);
    assert_eq!(proof.new_size, 100);
    assert_eq!(verify_consistency(&proof, &root_0, &root_1), Ok(true));
}

/// 「root_1を改変したものでは `Ok(false)`」, and the same for root_0.
#[test]
fn ac_023_a_modified_root_rejects() {
    let log = log_of(100);
    let root_0 = log.root_at(50).expect("root");
    let root_1 = log.root().expect("root");
    let proof = prove_consistency(&log, 50, 100).expect("proof");

    assert_eq!(verify_consistency(&proof, &root_0, &cid(7_777)), Ok(false));
    assert_eq!(verify_consistency(&proof, &cid(7_777), &root_1), Ok(false));
}

/// A rewritten history fails, which is the whole purpose of the proof.
///
/// Two logs of 100 entries that agree on the first 49 and differ at entry 49: the second one is
/// what an operator who edited a committed entry and re-appended everything after it would hold.
/// Its root at 100 is a perfectly well-formed root; what it is not is a tree that contains the
/// original 50-leaf tree, and no proof can make it verify as one.
#[test]
fn ac_023_a_rewritten_prefix_cannot_be_made_consistent() {
    let honest = log_of(100);
    let mut rewritten = TileLog::new();
    for i in 0..100u64 {
        let receipt = if i == 49 {
            cid(6_666)
        } else {
            cid(1_000_000 + i)
        };
        rewritten
            .append(TransformationId(cid(i)), receipt, Timestamp(i as i64))
            .expect("canonical");
    }

    let honest_root_0 = honest.root_at(50).expect("root");
    let rewritten_root_1 = rewritten.root().expect("root");

    // The forger's best effort: a proof generated from their own tree.
    let forged = prove_consistency(&rewritten, 50, 100).expect("proof");
    assert_eq!(
        verify_consistency(&forged, &honest_root_0, &rewritten_root_1),
        Ok(false)
    );

    // And the honest proof does not verify the rewritten root either.
    let honest_proof = prove_consistency(&honest, 50, 100).expect("proof");
    assert_eq!(
        verify_consistency(&honest_proof, &honest_root_0, &rewritten_root_1),
        Ok(false)
    );
}

/// A truncated tree is not a consistent successor. Dropping entries is the other half of rewriting.
#[test]
fn ac_023_a_truncated_tree_is_not_a_successor() {
    let log = log_of(100);
    let root_100 = log.root().expect("root");
    let root_75 = log.root_at(75).expect("root");

    // Growing 100 -> 75 is not growth at all; the proof cannot be generated.
    assert!(prove_consistency(&log, 100, 75).is_err());

    // And a well-formed 75->100 proof, presented with the roles swapped, fails.
    let proof = prove_consistency(&log, 75, 100).expect("proof");
    assert_eq!(verify_consistency(&proof, &root_100, &root_75), Ok(false));
}

/// A tampered path fails.
#[test]
fn ac_023_a_tampered_path_rejects() {
    let log = log_of(100);
    let root_0 = log.root_at(50).expect("root");
    let root_1 = log.root().expect("root");
    let proof = prove_consistency(&log, 50, 100).expect("proof");
    assert!(!proof.path.is_empty(), "50 -> 100 needs a non-empty path");

    let mut edited = proof.clone();
    edited.path[0] = cid(4_321);
    assert_eq!(verify_consistency(&edited, &root_0, &root_1), Ok(false));

    let mut shortened = proof.clone();
    shortened.path.pop();
    assert_eq!(verify_consistency(&shortened, &root_0, &root_1), Ok(false));

    let mut lengthened = proof.clone();
    lengthened.path.push(cid(2));
    assert_eq!(verify_consistency(&lengthened, &root_0, &root_1), Ok(false));

    // A proof whose declared sizes were edited to something it does not prove.
    let mut relabelled = proof.clone();
    relabelled.old_size = 49;
    assert_eq!(verify_consistency(&relabelled, &root_0, &root_1), Ok(false));
}

/// The identity case: a tree is consistent with itself, and its proof is empty.
#[test]
fn ac_023_a_tree_is_consistent_with_itself() {
    let log = log_of(100);
    let root = log.root().expect("root");
    let proof = prove_consistency(&log, 100, 100).expect("proof");

    assert!(proof.path.is_empty());
    assert_eq!(verify_consistency(&proof, &root, &root), Ok(true));
    assert_eq!(verify_consistency(&proof, &root, &cid(1)), Ok(false));
}

/// Sizes that cannot describe growth are refused rather than answered.
///
/// `old_size = 0` has no root to be consistent *with*: 42 §3.11's `Checkpoint` carries a
/// `root_hash` and an empty tree has none, so the question is malformed rather than false
/// (req/26 §3 -- state the range, refuse the rest).
#[test]
fn ac_023_impossible_sizes_are_refused() {
    let log = log_of(100);
    assert!(prove_consistency(&log, 0, 100).is_err());
    assert!(prove_consistency(&log, 50, 101).is_err());
    assert!(prove_consistency(&log, 101, 200).is_err());

    let root = log.root().expect("root");
    let malformed = gx_log::proof::ConsistencyProof {
        new_size: 100,
        old_size: 0,
        path: Vec::new(),
    };
    assert!(verify_consistency(&malformed, &root, &root).is_err());
}

// ---------------------------------------------------------------------------
// The property AC-023's 判定方法 column asks for
// ---------------------------------------------------------------------------

proptest! {
    /// Every pair of sizes in a growing log is consistent.
    #[test]
    fn ac_023_every_prefix_is_consistent_with_every_later_size(
        new_size in 1u64..=64,
        pick in 0u64..64,
    ) {
        let log = log_of(new_size);
        let old_size = (pick % new_size) + 1;

        let old_root = log.root_at(old_size).expect("a prefix root");
        let new_root = log.root().expect("root");
        let proof = prove_consistency(&log, old_size, new_size).expect("proof");

        prop_assert_eq!(verify_consistency(&proof, &old_root, &new_root), Ok(true));
    }

    /// A proof between two sizes does not verify a third root.
    #[test]
    fn ac_023_a_proof_is_about_the_two_trees_it_names(
        new_size in 4u64..=64,
        pick in 0u64..64,
        other in 0u64..64,
    ) {
        let log = log_of(new_size);
        let old_size = (pick % (new_size - 1)) + 1;
        let third = (other % new_size) + 1;
        prop_assume!(third != old_size && third != new_size);

        let old_root = log.root_at(old_size).expect("root");
        let third_root = log.root_at(third).expect("root");
        let proof = prove_consistency(&log, old_size, new_size).expect("proof");

        prop_assert_eq!(verify_consistency(&proof, &old_root, &third_root), Ok(false));
    }

    /// Any single edit to the path breaks the proof.
    #[test]
    fn ac_023_any_edit_to_the_path_breaks_it(
        new_size in 3u64..=64,
        pick in 0u64..64,
        position in 0usize..8,
    ) {
        let log = log_of(new_size);
        let old_size = (pick % (new_size - 1)) + 1;
        let old_root = log.root_at(old_size).expect("root");
        let new_root = log.root().expect("root");
        let proof = prove_consistency(&log, old_size, new_size).expect("proof");
        prop_assume!(!proof.path.is_empty());

        let mut edited = proof.clone();
        let at = position % edited.path.len();
        edited.path[at] = cid(900_000 + at as u64);

        prop_assert_eq!(verify_consistency(&edited, &old_root, &new_root), Ok(false));
    }
}
