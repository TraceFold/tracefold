// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Ruling #14's comparison half — a split view across pooled verdict checkpoints (`req/964` B-6).
//!
//! `gx-core/src/ledger.rs` declares the limit these cases answer: a signature attests "this key
//! stated these counts", never "these are the only counts this key stated", so one key can sign two
//! internally consistent chains for two verifiers. `req/98` §9 row v-6 is the row that sent the
//! comparison to v0.2.1, and 32-functional's FR-M04 carries it as declared limit ②.
//!
//! red-first (AC4 type, `req/661` §2): every adversarial case here fails against a no-op detector,
//! and only the real scan turns it green. The three negative controls pass against the no-op too,
//! which is what makes them controls: they measure that the detector does not cry contradiction
//! where none exists.
//!
//! `a_single_view_cannot_see_the_fork` is a different kind of case — it is the *residual* written
//! as a measurement, in `ac_vc.rs::policy_relaxation_is_not_detected`'s standing (ruling #3's limit
//! measured rather than asserted). It stays green whether or not the detector exists, because what
//! it measures is `audit_verdict_chain` being blind to a fork by construction.
//!
//! Signatures are not exercised here: the operator holds the key and signs both chains, so a valid
//! signature says nothing about which chain is the history. What these cases measure is the
//! arithmetic no signature can rescue.

use gx_core::{Cid, DsseSignature, KeyId, Timestamp, VerdictCheckpoint, VerdictTally};
use gx_log::proof::audit_verdict_chain;
use gx_log::{detect_verdict_equivocation, VerdictContradiction};

const ORIGIN: &str = "glovrex-verdicts/v1";
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn tally(deny: u64, admit: u64) -> VerdictTally {
    VerdictTally {
        deny,
        admit,
        escalate: 0,
        unverdicted: 0,
    }
}

/// A checkpoint over `[start, end)` under `origin`, bound to `ledger`. The signature is filler:
/// this detector never reads it.
fn checkpoint(
    origin: &str,
    start: u64,
    end: u64,
    counts: VerdictTally,
    ledger: (u64, Option<Cid>),
) -> VerdictCheckpoint {
    VerdictCheckpoint {
        origin: origin.to_string(),
        tally: counts,
        window_start: start,
        window_end: end,
        ledger_root_hash: ledger.1,
        ledger_tree_size: ledger.0,
        timestamp: AT,
        signature: DsseSignature {
            keyid: KeyId::from("fixture-key"),
            sig: vec![7u8; 64],
        },
    }
}

/// One checkpoint of the deployment under test, on the one ledger head.
fn window(start: u64, end: u64, counts: VerdictTally, leaves: u64) -> VerdictCheckpoint {
    checkpoint(ORIGIN, start, end, counts, (leaves, Some(cid(9))))
}

// ---------------------------------------------------------------------------
// The fork, named
// ---------------------------------------------------------------------------

/// 🔴 **B-6** — one window, two signed tallies. The decisive shape: a window is a half-open range
/// of verdict sequence numbers and it held what it held, so two attested counts of one range are
/// irreconcilable in the signed fields alone.
#[test]
fn one_window_two_tallies_is_named_once_the_views_are_pooled() {
    let shown_to_a = window(0, 10, tally(4, 6), 6);
    let shown_to_b = window(0, 10, tally(0, 10), 6);

    let found = detect_verdict_equivocation(&[shown_to_a, shown_to_b]);

    assert_eq!(
        found,
        vec![VerdictContradiction::Equivocation {
            origin: ORIGIN.to_string(),
            window_start: 0,
            window_end: 10,
            tally_a: tally(4, 6),
            tally_b: tally(0, 10),
        }],
        "two signed counts of one window are what no key reconciles"
    );
}

/// 🔴 **B-6, the boundary is not a hiding place** — an operator who signs a second, different set
/// of counts and *moves the seam* to dodge the same-window case lands on the overlap instead. A
/// single honest chain lays its windows end to end (`audit_verdict_chain` calls anything else a
/// `Gap`), so a partial overlap under one origin is two chains.
#[test]
fn shifting_the_window_boundary_does_not_evade_the_detector() {
    let shown_to_a = window(0, 10, tally(4, 6), 6);
    let shown_to_b = window(5, 12, tally(0, 7), 6);

    let found = detect_verdict_equivocation(&[shown_to_a, shown_to_b]);

    assert_eq!(
        found,
        vec![VerdictContradiction::OverlappingWindows {
            origin: ORIGIN.to_string(),
            a_window: (0, 10),
            b_window: (5, 12),
        }],
        "an operator who moves the seam to dodge the same-window case is still holding two chains"
    );
}

/// 🔴 **B-6 / AC-VC-5** — the binding field forks too. `ledger_tree_size` is what FR-M04 binds a
/// verdict chain to the commit ledger by, and a verifier pooling verdict checkpoints alone never
/// calls `detect_equivocation`: without this, the fork it is holding the evidence of goes unnamed.
#[test]
fn two_ledger_roots_at_one_tree_size_are_named() {
    let shown_to_a = checkpoint(ORIGIN, 0, 10, tally(4, 6), (6, Some(cid(1))));
    let shown_to_b = checkpoint(ORIGIN, 10, 20, tally(4, 6), (6, Some(cid(2))));

    let found = detect_verdict_equivocation(&[shown_to_a, shown_to_b]);

    assert_eq!(
        found,
        vec![VerdictContradiction::LedgerHeadEquivocation {
            origin: ORIGIN.to_string(),
            ledger_tree_size: 6,
            root_a: cid(1),
            root_b: cid(2),
        }],
        "a tree of one size has one root; two signed roots at that size is the ledger forking \
         underneath the counts"
    );
}

// ---------------------------------------------------------------------------
// The residual, measured (ruling #14's own sentence)
// ---------------------------------------------------------------------------

/// 🔴 **ruling #14, as a measurement** — a verifier holding *one* view sees nothing wrong.
///
/// `ac_vc.rs::policy_relaxation_is_not_detected` is ruling #3's limit written as a test rather than
/// as a sentence; this is ruling #14's. Both chains below are internally consistent, and
/// `audit_verdict_chain` — the whole of what a single verifier can run — returns empty on each. The
/// fork exists only in the union, which is why the act that closes it is the pooling and not the
/// arithmetic.
#[test]
fn a_single_view_cannot_see_the_fork() {
    let view_a = [window(0, 10, tally(4, 6), 6)];
    let view_b = [window(0, 10, tally(0, 10), 6)];

    assert!(
        audit_verdict_chain(&view_a, &tally(0, 0), 6).is_empty(),
        "verifier A's chain adds up on its own -- that is the whole of ruling #14"
    );
    assert!(
        audit_verdict_chain(&view_b, &tally(0, 0), 6).is_empty(),
        "so does verifier B's, and neither verifier can reach the other's"
    );

    let pooled = [view_a[0].clone(), view_b[0].clone()];
    assert_eq!(
        detect_verdict_equivocation(&pooled).len(),
        1,
        "and the same two checkpoints, pooled, are a contradiction: the comparison is the act that \
         closes it, not the audit"
    );
}

// ---------------------------------------------------------------------------
// Negative controls -- the detector must not cry contradiction where none exists
// ---------------------------------------------------------------------------

/// A pool from two honest verifiers of one chain: overlapping *sets* of the same checkpoints, plus
/// a quiet period's empty window sitting on a seam. Nothing here contradicts itself.
#[test]
fn an_honest_pool_from_two_verifiers_is_silent() {
    let w0 = window(0, 10, tally(4, 6), 6);
    let w1 = window(10, 20, tally(1, 9), 15);
    let quiet = window(20, 20, tally(0, 0), 15);
    let w2 = window(20, 25, tally(0, 5), 20);

    // A held the first three, B held the last three; both handed over what they had, duplicates
    // included. An empty window on a seam is a true statement about a quiet period, not an overlap.
    let pooled = [w0, w1.clone(), quiet.clone(), w1, quiet, w2];

    assert!(
        detect_verdict_equivocation(&pooled).is_empty(),
        "two verifiers of one chain overlap by construction; a detector that called that a fork \
         would be unusable on the only input it is ever handed"
    );
}

/// One key, two namespaces. `origin` is what stops one deployment's counts from being read as
/// another's, and folding the two together is the false positive `detect_equivocation`'s AC-B1b
/// guards against on the commit side.
#[test]
fn one_key_signing_two_origins_is_not_a_contradiction() {
    let ours = checkpoint(ORIGIN, 0, 10, tally(4, 6), (6, Some(cid(1))));
    let theirs = checkpoint(
        "other-deployment/v1",
        0,
        10,
        tally(0, 10),
        (6, Some(cid(2))),
    );

    assert!(
        detect_verdict_equivocation(&[ours, theirs]).is_empty(),
        "a checkpoint of one deployment carries no claim about another's counts"
    );
}

/// Zero and one are trivially self-consistent: there is no pair to compare.
#[test]
fn an_empty_or_single_pool_is_trivially_consistent() {
    assert!(detect_verdict_equivocation(&[]).is_empty());
    let one = window(0, 10, tally(4, 6), 6);
    assert!(detect_verdict_equivocation(std::slice::from_ref(&one)).is_empty());
}

/// The scan visits unordered pairs, so which verifier handed its view over first cannot change
/// whether a contradiction is found. Field order *inside* a finding does follow the input — naming
/// which tally is `a` is a rendering, and the presence is the judgement.
#[test]
fn the_finding_does_not_depend_on_who_reported_first() {
    let a = window(0, 10, tally(4, 6), 6);
    let b = window(0, 10, tally(0, 10), 6);
    let c = checkpoint(ORIGIN, 5, 12, tally(0, 7), (6, Some(cid(3))));

    let forward = detect_verdict_equivocation(&[a.clone(), b.clone(), c.clone()]);
    let reversed = detect_verdict_equivocation(&[c, b, a]);

    assert!(
        !forward.is_empty(),
        "the fixture must actually contradict itself, or this measures nothing"
    );
    assert_eq!(
        forward.len(),
        reversed.len(),
        "the pair scan is symmetric: reversing the pool cannot change how many contradictions it \
         holds"
    );
}

/// Non-vacuity: agree the two views and every finding disappears. Without this the cases above
/// would still pass against a detector that returned a finding for anything at all.
#[test]
fn agreeing_the_two_views_silences_every_finding() {
    let a = window(0, 10, tally(4, 6), 6);
    let disagreeing = window(0, 10, tally(0, 10), 6);
    let agreeing = a.clone();

    assert!(
        !detect_verdict_equivocation(&[a.clone(), disagreeing]).is_empty(),
        "the disagreeing pair fires"
    );
    assert!(
        detect_verdict_equivocation(&[a, agreeing]).is_empty(),
        "and the identical pair does not -- the difference is what is being detected"
    );
}
