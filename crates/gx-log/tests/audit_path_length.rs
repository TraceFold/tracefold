//! H2-5 — an audit path has one lawful length, and it is checked before anything is hashed.
//!
//! req/38 §11 逐語: 「(leaf_index, tree_size) から path 長は数学的に一意——hash 計算の**前に**長さ
//! 照合し、不一致は即 reject。全 hash 後拒否は計算資源の無駄+挙動が入力長に依存する形」.
//!
//! # What is being fixed
//!
//! Hand 2's `verify_inclusion` answered `Ok(false)` for a path of the wrong length, which is the
//! right answer -- it reached it by hashing every element first. For a proof arriving from outside
//! (M6's third-party verification) the work a verifier does would then be chosen by whoever sent
//! the proof. The length a proof of leaf `i` in a tree of `n` leaves must have is fixed by `i` and
//! `n` alone, so it is decidable in `O(log n)` integer operations and no hash at all.
//!
//! # Why the ordering is asserted from the source
//!
//! 「hash 計算の前に」 is not observable in the return value: both orders answer `Ok(false)`. A
//! timing assertion would be flaky, and a hash counter would be a second implementation of the
//! thing being measured. So the behaviour is tested for its answers and the *ordering* is read off
//! `proof.rs` -- the same split `ac_069.rs` makes for the fsync barrier, and for the same reason.

mod support;

use gx_core::InclusionProof;
use gx_log::proof::{prove_inclusion_at, verify_inclusion};
use support::{cid, code_lines, log_of, source};

/// The generated path always has the lawful length, over every tree the tests reach.
///
/// Exhaustive rather than random: sizes 1..=64 with every leaf is 2080 cases, which is cheaper
/// than the property runs in `ac_022.rs` and covers every ragged shape below 64 exactly once.
#[test]
fn a_generated_path_has_the_length_the_two_indices_fix() {
    for size in 1..=64u64 {
        let log = log_of(size);
        for index in 0..size {
            let proof = prove_inclusion_at(&log, index, size).expect("proof");
            assert_eq!(
                proof.audit_path.len(),
                expected_len(index, size),
                "leaf {index} of a tree of {size}"
            );
        }
    }
}

/// A path one element too long or too short is refused, at every shape.
#[test]
fn a_path_of_the_wrong_length_is_refused() {
    for size in 1..=32u64 {
        let log = log_of(size);
        let root = log.root().expect("a non-empty tree has a root");
        for index in 0..size {
            let proof = prove_inclusion_at(&log, index, size).expect("proof");
            let entry = log.entry(index).expect("entry");

            let mut longer = proof.clone();
            longer.audit_path.push(cid(7_777));
            assert_eq!(
                verify_inclusion(&longer, &root, entry),
                Ok(false),
                "a padded path verified at leaf {index} of {size}"
            );

            if !proof.audit_path.is_empty() {
                let mut shorter = proof.clone();
                shorter.audit_path.pop();
                assert_eq!(
                    verify_inclusion(&shorter, &root, entry),
                    Ok(false),
                    "a truncated path verified at leaf {index} of {size}"
                );
            }
        }
    }
}

/// A proof that claims a leaf outside its own tree is refused whatever it carries.
#[test]
fn a_proof_outside_its_own_tree_is_refused() {
    let log = log_of(8);
    let root = log.root().expect("root");
    let entry = log.entry(3).expect("entry");

    for (leaf_index, tree_size) in [(3u64, 0u64), (8, 8), (9, 8), (u64::MAX, 8)] {
        let proof = InclusionProof {
            leaf_index,
            tree_size,
            audit_path: vec![cid(1), cid(2), cid(3)],
        };
        assert_eq!(
            verify_inclusion(&proof, &root, entry),
            Ok(false),
            "leaf_index {leaf_index} in a declared tree of {tree_size}"
        );
    }
}

/// An oversized path is refused without the work it asks for.
///
/// Ten thousand elements against a tree of eight: a verifier that hashed first would do ten
/// thousand BLAKE3 compressions to answer a question three integer operations settle. The test
/// asserts the answer; that it is reached without hashing is the ordering test below.
#[test]
fn an_oversized_path_is_refused() {
    let log = log_of(8);
    let root = log.root().expect("root");
    let entry = log.entry(0).expect("entry");
    let proof = InclusionProof {
        leaf_index: 0,
        tree_size: 8,
        audit_path: (0..10_000u64).map(cid).collect(),
    };
    assert_eq!(verify_inclusion(&proof, &root, entry), Ok(false));
}

/// The length gate stands before the first hash in `verify_inclusion`.
#[test]
fn the_length_gate_precedes_the_first_hash() {
    let proof_rs = source("proof.rs");
    let lines = code_lines(&proof_rs);

    let start = lines
        .iter()
        .position(|(_, l)| l.contains("pub fn verify_inclusion("))
        .expect("proof.rs has verify_inclusion");
    let body: Vec<&(usize, String)> = lines[start..].iter().collect();

    let gate = body
        .iter()
        .position(|(_, l)| l.contains("audit_path_len("))
        .expect("verify_inclusion consults the lawful path length");
    let hash = body
        .iter()
        .position(|(_, l)| l.contains("leaf_hash("))
        .expect("verify_inclusion recomputes the leaf hash");

    assert!(
        gate < hash,
        "the path length is checked at line {} and the first hash is taken at line {}; H2-5 \
         requires the gate first",
        body[gate].0,
        body[hash].0
    );
}

/// The lawful length, written out independently of the implementation.
///
/// A second road to the same number, in the shape RFC 6962 §2.1.1 states the path in: the tree of
/// `n` splits at the largest power of two strictly below `n`, the leaf falls in one half, and one
/// sibling is contributed per split. Written here rather than imported so that a mistake in
/// `tile.rs` is not compared against itself.
fn expected_len(index: u64, tree_size: u64) -> usize {
    let (mut i, mut n, mut len) = (index, tree_size, 0usize);
    while n > 1 {
        let k = 1u64 << (u64::BITS - 1 - (n - 1).leading_zeros());
        if i < k {
            n = k;
        } else {
            i -= k;
            n -= k;
        }
        len += 1;
    }
    len
}
