//! 🔴 **FR-M7-2** — `prove_inclusion` は tile の cached hash で incremental になった。**答えは 1 bit も
//! 動いていない**、を独立な oracle と wire の両側から測る。
//!
//! req/98 §3-2 の AC 逐語: 「同一 bucket 計器で**前/後の median+回数+分母**を対照実験(req/95 §1 の 2 arm
//! 形)で示し、`verify_inclusion_of` と第三者検証器を **1 行も変えずに** 通る」。
//!
//! 2 arm の**時間**は `benches/inclusion_proof.rs` が測る(2 build・同一計器 file・commit を記録)。この
//! file が測るのは残りの半分——**同じ木について同じ proof が出る**事であり、それは 1 つの build の中で
//! 決着できる。
//!
//! # なぜ oracle をここに書き直すのか
//!
//! `proof.rs` の module doc が既に理由を書いている: 「[`prove_inclusion`] walks the tree;
//! [`verify_inclusion`] walks the *index* … A verifier expressed in terms of the prover would pass
//! whenever the prover was self-consistent」。同じ規律が最適化にも効く——新しい実装を、それが置き換えた
//! 実装の**再輸入**で検査すれば、両方が同じ間違いをした時に緑になる。∴ ここの [`reference_path`] は
//! **RFC 6962 §2.1.1 の `PATH(m, D[n])` を、この crate の code を 1 行も見ずに書き写した物**である:
//!
//! ```text
//! PATH(0, {d(0)}) = {}
//! PATH(m, D[n])   = PATH(m, D[0..k]) : MTH(D[k..n])      if m < k
//!                 = PATH(m-k, D[k..n]) : MTH(D[0..k])     otherwise
//! ```
//!
//! `MTH` も同じ節から書き写す(1 枚なら葉そのもの、それ以外は `k` で割って `0x01 || left || right`)。
//! 唯一 crate から借りるのは **葉の hash** と **node hash** で、それは 42 §3.11 の domain byte が
//! gx-canon の mint に在る (E-M2-12) からである——ここで blake3 を直に呼べば 41 §6 の「全 canonical
//! encode は gx-canon 経由のみ」を test 側で破る事になる。
//!
//! # 分母
//!
//! 木の形は size で決まるので、**tile 境界の両側**(255/256/257・511/512/513)と、ragged が最も効く小さい
//! 側を全 index 歩く。1,000 と 4,096 は index を間引く(全部歩くと probe が bench になる)。

use gx_canon::cid;
use gx_core::{Cid, InclusionProof, Timestamp, TransformationId};
use gx_log::proof::{prove_inclusion, prove_inclusion_at, verify_inclusion_of};
use gx_log::tile::{LedgerLeaf, TileLog, TILE_WIDTH};

/// A distinguishable receipt digest. Not a hash of anything: the tree's shape is the subject.
fn digest(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn log_of(n: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        log.append(
            TransformationId(digest(i)),
            digest(1_000_000 + i),
            Timestamp(i as i64),
        )
        .expect("a leaf of three ids has a canonical form");
    }
    log
}

// ---------------------------------------------------------------------------
// The oracle: RFC 6962 §2.1 / §2.1.1, transcribed
// ---------------------------------------------------------------------------

/// The largest power of two **strictly** below `n` (RFC 6962 §2.1's `k`).
fn k_of(n: usize) -> usize {
    let mut k = 1usize;
    while k * 2 < n {
        k *= 2;
    }
    k
}

/// `MTH(D[n])`, from the leaf hashes.
fn reference_mth(leaves: &[Cid]) -> Option<Cid> {
    match leaves.len() {
        0 => None,
        1 => Some(leaves[0]),
        n => {
            let k = k_of(n);
            Some(cid::mint_node(
                &reference_mth(&leaves[..k])?,
                &reference_mth(&leaves[k..])?,
            ))
        }
    }
}

/// `PATH(m, D[n])`, bottom-up.
fn reference_path(m: usize, leaves: &[Cid], out: &mut Vec<Cid>) {
    let n = leaves.len();
    if n <= 1 {
        return;
    }
    let k = k_of(n);
    if m < k {
        reference_path(m, &leaves[..k], out);
        out.push(reference_mth(&leaves[k..]).expect("a right half is non-empty"));
    } else {
        reference_path(m - k, &leaves[k..], out);
        out.push(reference_mth(&leaves[..k]).expect("a left half is non-empty"));
    }
}

/// The leaf hashes of a log, computed the way 42 §3.11 defines them — from the entries, through
/// gx-canon's mint, without reading `leaf_cid`.
fn reference_leaves(log: &TileLog) -> Vec<Cid> {
    log.entries()
        .iter()
        .map(|e| {
            cid::mint_leaf(&LedgerLeaf {
                index: e.index,
                receipt_digest: e.receipt_digest,
                transformation: e.transformation,
            })
            .expect("a leaf has a canonical form")
        })
        .collect()
}

// ---------------------------------------------------------------------------

/// 🔴 **The proof the cache produces is the proof RFC 6962 defines**, at every index of every size
/// where the tile boundary can bite.
#[test]
fn every_proof_equals_the_transcribed_rfc_6962_path() {
    let mut compared = 0usize;
    for size in [1u64, 2, 3, 4, 7, 8, 255, 256, 257, 511, 512, 513] {
        let log = log_of(size);
        let leaves = reference_leaves(&log);
        assert_eq!(
            leaves.as_slice(),
            log.leaf_hashes(),
            "leaf hashes at size {size}"
        );
        assert_eq!(
            log.root(),
            reference_mth(&leaves),
            "the root at size {size} is not the transcribed MTH"
        );
        for index in 0..size {
            let proof = prove_inclusion(&log, index).expect("the leaf is in the tree");
            let mut want = Vec::new();
            reference_path(index as usize, &leaves, &mut want);
            assert_eq!(proof.audit_path, want, "leaf {index} of {size}");
            assert_eq!(proof.leaf_index, index);
            assert_eq!(proof.tree_size, size);
            compared += 1;
        }
    }
    println!("FRM72_PATHS_VS_RFC compared={compared} sizes=12 oracle=RFC-6962-§2.1.1-transcribed");
}

/// The same equality for **prefix** trees: a proof issued against a checkpoint the log has passed.
///
/// This is where a cache is most likely to be wrong and least likely to be noticed — the cached
/// tiles belong to the whole log, and the question is about a shorter tree that ends inside one.
#[test]
fn a_proof_against_an_older_tree_size_is_the_same_proof() {
    let log = log_of(1_100);
    let leaves = reference_leaves(&log);
    let mut compared = 0usize;
    for tree_size in [1u64, 255, 256, 257, 512, 700, 1_024, 1_025, 1_100] {
        let root = log
            .root_at(tree_size)
            .expect("the log has reached that size");
        assert_eq!(
            Some(root),
            reference_mth(&leaves[..tree_size as usize]),
            "the prefix root at {tree_size}"
        );
        for index in (0..tree_size).step_by(37) {
            let proof =
                prove_inclusion_at(&log, index, tree_size).expect("the leaf is in that tree");
            let mut want = Vec::new();
            reference_path(index as usize, &leaves[..tree_size as usize], &mut want);
            assert_eq!(proof.audit_path, want, "leaf {index} of prefix {tree_size}");
            assert!(
                verify_inclusion_of(
                    &proof,
                    &root,
                    &log.entry(index).expect("the entry is there").leaf()
                )
                .expect("the leaf has a canonical form"),
                "the verifier refused a proof it produced for leaf {index} of {tree_size}"
            );
            compared += 1;
        }
    }
    println!(
        "FRM72_PREFIX_PROOFS compared={compared} sizes=9 verifier=verify_inclusion_of(unchanged)"
    );
}

/// 🔴 **The wire form is `{leaf_index, tree_size, audit_path}` and nothing else.**
///
/// 追加裁定 a 逐語: 「`InclusionProof` の wire 形は変えない」. A cache that leaked into the proof — a
/// tile index, a hint, a version — would be a change every third-party verifier had to be told
/// about, so the field set is read off the encoded value rather than off the struct definition.
#[test]
fn the_proof_carries_three_fields_and_the_cache_is_not_one_of_them() {
    let log = log_of(600);
    let proof = prove_inclusion(&log, 42).expect("the leaf is in the tree");
    let json = serde_json::to_value(&proof).expect("a proof serialises");
    let object = json.as_object().expect("a proof is a map");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["audit_path", "leaf_index", "tree_size"]);

    // And the canonical bytes are the canonical bytes of the same three values assembled by hand:
    // a decoder that never saw this crate reads what it always read.
    let by_hand = InclusionProof {
        leaf_index: proof.leaf_index,
        tree_size: proof.tree_size,
        audit_path: proof.audit_path.clone(),
    };
    assert_eq!(
        gx_canon::cbor::encode(&proof).expect("canonical"),
        gx_canon::cbor::encode(&by_hand).expect("canonical"),
    );
    println!(
        "FRM72_WIRE keys={keys:?} bytes={} audit_path={}",
        gx_canon::cbor::encode(&proof).expect("canonical").len(),
        proof.audit_path.len()
    );
}

/// 🔴 **The cache is maintained, and what it holds is what it claims to hold.**
///
/// `SHIPPED_PACKS`'s shape (M7 hand 4 §3-3): a declaration and the thing it declares, compared. A
/// `commit` that stopped pushing would be caught by the root probes too — the cache is
/// authoritative — but this one names *which* invariant broke instead of only that a root moved.
#[test]
fn the_completed_tiles_are_cached_and_the_partial_tail_is_not() {
    let width = u64::from(TILE_WIDTH);
    for size in [0u64, 1, 255, 256, 257, 512, 513, 1_100] {
        let log = log_of(size);
        let leaves = reference_leaves(&log);
        let cached = log.cached_subtree_roots();
        assert_eq!(
            cached.len() as u64,
            size / width,
            "a log of {size} leaves has {} complete tiles",
            size / width
        );
        for (t, root) in cached.iter().enumerate() {
            let start = t * width as usize;
            assert_eq!(
                Some(*root),
                reference_mth(&leaves[start..start + width as usize]),
                "tile {t} of a log of {size}"
            );
        }
    }
    println!("FRM72_TILE_CACHE width={TILE_WIDTH} sizes=8 declaration=len()/{TILE_WIDTH}");
}

/// Appending does not disturb a proof already issued, and the log keeps answering for the old head.
///
/// The property the whole tile layout rests on (「appending never rewrites a hash that was already
/// published」), now with a cache in the way of it.
#[test]
fn an_append_leaves_an_issued_proof_standing() {
    let mut log = log_of(300);
    let root_at_300 = log.root().expect("a non-empty log has a head");
    let proof = prove_inclusion(&log, 7).expect("the leaf is in the tree");
    let leaf = log.entry(7).expect("the entry is there").leaf();
    assert!(verify_inclusion_of(&proof, &root_at_300, &leaf).expect("canonical"));

    for i in 300..600u64 {
        log.append(
            TransformationId(digest(i)),
            digest(1_000_000 + i),
            Timestamp(i as i64),
        )
        .expect("canonical");
    }
    assert_eq!(log.root_at(300), Some(root_at_300), "the old head moved");
    assert!(
        verify_inclusion_of(&proof, &root_at_300, &leaf).expect("canonical"),
        "a proof issued at 300 stopped verifying after 300 more appends"
    );
    // And against the **new** head it must not verify: the proof names its tree.
    let new_root = log.root().expect("a non-empty log has a head");
    assert!(
        !verify_inclusion_of(&proof, &new_root, &leaf).expect("canonical"),
        "a proof against a tree of 300 verified against a tree of 600"
    );
    println!(
        "FRM72_APPEND_STABILITY old_size=300 new_size={} old_head_unmoved=true",
        log.len()
    );
}
