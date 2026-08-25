// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Phase B (Model B witness) — the offline contradiction detector (`req/682`).
//!
//! red-first (AC4 type, `req/661` §2): every adversarial case here fails against a no-op detector —
//! the "no detector silently passes" world `req/682` §5 names — and only the real scan turns it
//! green. The two negative controls (AC-B3, AC-B6) pass against the no-op too, which is what makes
//! them controls: they measure that the detector does not cry contradiction where none exists.
//!
//! Signatures are not exercised here. `req/682` §2-2 puts signature verification in
//! `gx_witness::dsse`, before a checkpoint reaches this detector; what these cases measure is the
//! arithmetic no signature can rescue (same shape as `proof::audit_verdict_chain`).

use gx_core::{Checkpoint, Cid, DsseSignature, Timestamp};
use gx_log::proof::prove_consistency;
use gx_log::tile::TileLog;
use gx_log::{classify_extension, detect_equivocation, CheckpointContradiction};

const ORIGIN: &str = "glovrex-ledger/v1";

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

/// A signed checkpoint. The signature is filler: this detector never reads it (§2-2).
fn checkpoint(origin: &str, tree_size: u64, root: Cid) -> Checkpoint {
    Checkpoint {
        origin: origin.to_string(),
        tree_size,
        root_hash: root,
        timestamp: Timestamp(1_700_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "ledger-key-1".to_string(),
            sig: vec![0xAB; 64],
        },
    }
}

/// A log of `n` leaves seeded by `base`, so two different `base`s give two divergent histories that
/// share no leaf.
fn log_seeded(n: u64, base: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&(base + i).to_be_bytes());
        log.append(
            gx_core::TransformationId(Cid(raw)),
            cid(u8::try_from((base + i) % 251).expect("in range")),
            Timestamp(i as i64),
        )
        .expect("canonical");
    }
    log
}

/// **AC-B1** — equivocation is named. Two checkpoints of one origin sign the same `tree_size` to
/// different roots. A no-op detector silently passes (`req/682` §5); the real scan returns
/// `Equivocation` naming both roots.
#[test]
fn ac_b1_equivocation_at_one_size_is_named() {
    let a = checkpoint(ORIGIN, 100, cid(7));
    let b = checkpoint(ORIGIN, 100, cid(9));

    let found = detect_equivocation(&[a, b]);

    assert_eq!(
        found,
        vec![CheckpointContradiction::Equivocation {
            origin: ORIGIN.to_string(),
            tree_size: 100,
            root_a: cid(7),
            root_b: cid(9),
        }],
        "two attested histories of length 100 with different roots is equivocation"
    );
}

/// A different origin is a different log: two size-100 checkpoints under two origins are not a
/// contradiction, and the detector must not fold them into one.
#[test]
fn ac_b1b_different_origins_do_not_equivocate() {
    let a = checkpoint("glovrex-ledger/v1", 100, cid(7));
    let b = checkpoint("some-other-log/v1", 100, cid(9));

    assert!(
        detect_equivocation(&[a, b]).is_empty(),
        "the same size under two namespaces is two logs, not one contradicting itself"
    );
}

/// **AC-B2** — a fork is named. An older checkpoint from history A is offered as the prefix of a
/// newer checkpoint from a divergent history B, with B's own consistency proof over the two sizes.
/// The prefix relation is refuted, so the detector returns `Fork`.
#[test]
fn ac_b2_a_non_prefix_extension_is_a_fork() {
    let hist_a = log_seeded(5, 0);
    let hist_b = log_seeded(10, 5_000); // shares no leaf with A

    let old = checkpoint(ORIGIN, 5, hist_a.root_at(5).expect("a 5-leaf root"));
    let new = checkpoint(ORIGIN, 10, hist_b.root_at(10).expect("a 10-leaf root"));
    // B's internally valid consistency proof, offered to bridge A@5 -> B@10.
    let proof = prove_consistency(&hist_b, 5, 10).expect("canonical proof");

    let verdict = classify_extension(&old, &new, &proof).expect("well-formed inputs");

    assert_eq!(
        verdict,
        Some(CheckpointContradiction::Fork {
            origin: ORIGIN.to_string(),
            old_size: 5,
            new_size: 10,
            old_root: hist_a.root_at(5).expect("root"),
            new_root: hist_b.root_at(10).expect("root"),
        }),
        "A@5 is not a prefix of B@10, so this is a fork rather than an extension"
    );
}

/// **AC-B3** — negative control (symmetry). A genuine append-only extension of one history, with
/// that history's own consistency proof, is **not** a contradiction. False-positive rate is zero.
#[test]
fn ac_b3_a_real_extension_is_not_a_contradiction() {
    let log = log_seeded(10, 0);

    let old = checkpoint(ORIGIN, 5, log.root_at(5).expect("root"));
    let new = checkpoint(ORIGIN, 10, log.root_at(10).expect("root"));
    let proof = prove_consistency(&log, 5, 10).expect("canonical proof");

    assert_eq!(
        classify_extension(&old, &new, &proof).expect("well-formed inputs"),
        None,
        "5 -> 10 within one log is an extension, not a fork"
    );
    // And the set-level scan over two different sizes finds no equivocation.
    assert!(
        detect_equivocation(&[old, new]).is_empty(),
        "different sizes are not an equivocation"
    );
}

/// The set-level scan on two identical checkpoints (same size, same root) is not a contradiction:
/// only a *differing* root at one size is.
#[test]
fn ac_b3b_identical_copies_do_not_equivocate() {
    let a = checkpoint(ORIGIN, 100, cid(7));
    let b = checkpoint(ORIGIN, 100, cid(7));
    assert!(
        detect_equivocation(&[a, b]).is_empty(),
        "two copies of one checkpoint agree; agreement is not equivocation"
    );
}

/// **AC-B6** — the emptiness soundness note. Zero or one checkpoint is trivially self-consistent,
/// and an empty answer means "nothing here contradicts itself", never "this is the true history".
#[test]
fn ac_b6_zero_or_one_checkpoint_is_self_consistent() {
    assert!(
        detect_equivocation(&[]).is_empty(),
        "no checkpoints, no contradiction"
    );
    let one = checkpoint(ORIGIN, 42, cid(3));
    assert!(
        detect_equivocation(std::slice::from_ref(&one)).is_empty(),
        "one checkpoint cannot contradict itself"
    );
}

/// `classify_extension` refuses ill-posed inputs rather than answering: a size that is not strictly
/// below the other, or a proof whose sizes do not match the two checkpoints, is `Malformed` — a
/// non-answer, not a false "no fork".
#[test]
fn classify_refuses_ill_posed_inputs() {
    let log = log_seeded(10, 0);
    let old = checkpoint(ORIGIN, 5, log.root_at(5).expect("root"));
    let new = checkpoint(ORIGIN, 10, log.root_at(10).expect("root"));

    // Proof over the wrong sizes (2 -> 8, not 5 -> 10).
    let wrong = prove_consistency(&log, 2, 8).expect("canonical proof");
    assert!(
        classify_extension(&old, &new, &wrong).is_err(),
        "a proof whose sizes do not match the checkpoints is a non-answer"
    );

    // Not strictly ordered by size.
    let same = prove_consistency(&log, 5, 10).expect("canonical proof");
    assert!(
        classify_extension(&new, &old, &same).is_err(),
        "old must be strictly shorter than new"
    );
}
