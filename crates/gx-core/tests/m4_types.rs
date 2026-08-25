// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The types M4 hand 1 adds to gx-core, and the two rulings that put them here.
//!
//! `m2_types.rs` and `m3_types.rs` are the same file one and two milestones back: a milestone that
//! moves a type down into this crate says so in one suite rather than scattering the claim.
//!
//! | ruling | what moved | why it is here and not in gx-substrate |
//! |---|---|---|
//! | **E-M4-1** | `Fingerprint` (42 §3.5) | `ReceiptPayload.precondition_fingerprint` (42 §3.10) names it, and gx-witness is inside the trust boundary 45 §1 draws while an adapter is outside it. E-M2-1: "types down, computation up" (sem: SEM-gx-core-181) |
//! | **E-M4-2** | `Intent`, `GoalBytes` (42 §3.3) | 42 §0 already filed `Intent` here; what was missing was the type, and `goal` had to stop being a `serde_json::Value` for 41 §2's "serde, thiserror and not much more" (sem: SEM-gx-core-182) to hold |
//!
//! # What M4 hand 2 changed here (**E-M4-27**)
//!
//! Hand 1 raised whether a `substrate` mismatch really is an ordinary `Ok(false)` while a `scope`
//! mismatch is an error (req/70 §2 M4H1-1), and req/38 §29 ruled case (b) (quoted in
//! SEM-gx-core-183):
//!
//! > `cas_eq` returns **`Err` on a substrate mismatch too** (the same "implementation error" class
//! > as a scope mismatch -- comparing another adapter's fingerprint is an engine wiring bug, and if
//! > it turned into an `Ok(false)` -> "the state moved" Abort the bug would be hidden; the same
//! > argument by which E-M4-15 refused PartialEq). 42 §3.5's equality is read as "**defined only
//! > between the products of one adapter**".
//!
//! So the probe that pinned the old answer was reversed rather than deleted, on the precedent
//! §29 M4H1-9 (confirmed; sem: SEM-gx-core-184) set for the same operation one hand earlier: same
//! fixture, the answer the ruling
//! requires, and a name that says which ruling moved it. The pin is not weaker for having moved --
//! it is what makes E-M4-27 a behaviour rather than a sentence in a ledger.
//!
//! # What is measured and what is not
//!
//! The **shape** of both types, and the whole of [`Fingerprint::cas_eq`] -- which is the one piece
//! of behaviour E-M4-15 rules on and the one thing a CAS check will stand on. Not measured here:
//! the projection (`crates/gx-canon/tests/intent_identity.rs`, which is the crate that can compute
//! a CID), and nothing at all about what a `scope` should cover -- that is ASM-69-1, hand 4's, and
//! a test written now would be a test of a fixture.

use gx_core::{
    Actor, ChangeContext, Cid, Error, Fingerprint, GoalBytes, Intent, SubstrateKind, ERROR_KINDS,
};

fn cid_of(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn fingerprint(scope: &str, digest: u8) -> Fingerprint {
    Fingerprint::new(SubstrateKind::Fs, scope.to_string(), cid_of(digest))
        .expect("a short scope is inside M4H1-2's bound")
}

// ---------------------------------------------------------------------------
// Fingerprint (E-M4-1, E-M4-15)
// ---------------------------------------------------------------------------

/// The three fields of 42 §3.5, read back through the accessors F-6 asks for.
#[test]
fn a_fingerprint_is_the_three_fields_of_42_3_5() {
    let f = fingerprint("/tmp/x", 7);
    assert_eq!(f.substrate(), &SubstrateKind::Fs);
    assert_eq!(f.scope(), "/tmp/x");
    assert_eq!(f.digest(), &cid_of(7));
}

/// "still in the state of the same name?" (sem: SEM-gx-core-185): same scope, same substrate, same
/// digest.
#[test]
fn cas_eq_is_true_for_two_fingerprints_of_the_same_state() {
    let before = fingerprint("/tmp/x", 7);
    let after = fingerprint("/tmp/x", 7);
    assert!(before
        .cas_eq(&after)
        .expect("the scopes agree, so the question has a meaning"));
}

/// "it moved" (sem: SEM-gx-core-186): the scope is the same and the digest is not, which is what
/// CON-2 aborts on.
#[test]
fn cas_eq_is_false_when_the_state_under_one_scope_moved() {
    let before = fingerprint("/tmp/x", 7);
    let after = fingerprint("/tmp/x", 8);
    assert!(!before.cas_eq(&after).expect("the scopes agree"));
}

/// Two adapters answering about the same scope name is not a comparison at all (**E-M4-27**).
///
/// The fixture is hand 1's, and the answer is the one §29 M4H1-1 (b) ruled: an `fs` fingerprint and
/// a `git` fingerprint that happen to spell their scope the same way are not "the state moved"
/// (sem: SEM-gx-core-187),
/// they are a wiring bug in whatever handed one to the other. Reported as `Ok(false)` it would
/// abort a commit with `PreconditionChanged` -- a reason that names a change nobody made -- and the
/// real defect would leave no trace. That is E-M4-15's argument against `PartialEq` applied to the
/// second field, which is why the two refusals are the same shape.
#[test]
fn cas_eq_refuses_a_comparison_across_substrates() {
    let fs = fingerprint("/tmp/x", 7);
    let git = Fingerprint::new(SubstrateKind::Git, "/tmp/x".to_string(), cid_of(7))
        .expect("a short scope is inside M4H1-2's bound");

    let got = fs.cas_eq(&git).expect_err(
        "E-M4-27: 42 §3.5's equality is 'defined only between the products of one adapter' \
             (sem: SEM-gx-core-188)",
    );
    assert_eq!(got.kind(), "FingerprintSubstrateMismatch");
    assert!(ERROR_KINDS.contains(&got.kind()));

    // The same refusal either way round, with each side reporting the substrate it was called on,
    // as `FingerprintScopeMismatch` already does for the scopes.
    let back = git.cas_eq(&fs).expect_err("the same disagreement");
    assert_eq!(back.kind(), got.kind());
    assert_ne!(got, back, "each side reports its own substrate as `left`");
}

/// When both fields disagree, the refusal names the substrate.
///
/// Order matters here because the two errors carry different values and an adapter author reads the
/// first one. Two adapters do not share a scope grammar -- `/tmp/x` and `refs/heads/main` are not a
/// scope that widened, they are two vocabularies -- so "another adapter's fingerprint was compared"
/// is the whole diagnosis and "the scopes differ" would be a symptom of it (sem: SEM-gx-core-189).
#[test]
fn cas_eq_names_the_substrate_when_both_fields_disagree() {
    let fs = fingerprint("/tmp/x", 7);
    let git = Fingerprint::new(SubstrateKind::Git, "refs/heads/main".to_string(), cid_of(7))
        .expect("a short scope is inside M4H1-2's bound");
    assert_eq!(
        fs.cas_eq(&git)
            .expect_err("two adapters, and therefore no comparison")
            .kind(),
        "FingerprintSubstrateMismatch"
    );
}

/// "that comparison has no meaning" (sem: SEM-gx-core-190): the third answer, which a `bool` could
/// not carry (**E-M4-15**).
#[test]
fn cas_eq_refuses_a_comparison_across_scopes() {
    let narrow = fingerprint("/tmp/x", 7);
    let wide = fingerprint("/tmp/x+lockfiles", 7);

    let got = narrow.cas_eq(&wide).expect_err(
        "42 §3.5: 'a comparison across differing `scope`s ... is treated as an adapter \
             implementation error' (sem: SEM-gx-core-191)",
    );
    assert_eq!(
        got,
        Error::FingerprintScopeMismatch {
            left: "/tmp/x".to_string(),
            right: "/tmp/x+lockfiles".to_string(),
        },
        "both spellings are carried, so the adapter author sees which two disagreed"
    );
    assert_eq!(got.kind(), "FingerprintScopeMismatch");
    assert!(ERROR_KINDS.contains(&got.kind()));

    // Not symmetric in the values it reports, and symmetric in the answer it gives: a refusal
    // either way round, with the two sides named as the caller wrote them.
    let back = wide.cas_eq(&narrow).expect_err("the same disagreement");
    assert_ne!(got, back, "each side reports its own scope as `left`");
    assert_eq!(back.kind(), got.kind());
}

/// A fingerprint whose digest is equal is not, by that alone, the same fingerprint.
///
/// This is the sentence `FingerprintBytes` has carried since M2 -- "**A receipt whose two
/// `FingerprintBytes` compare equal has not passed the CAS check**" (sem: SEM-gx-core-192) --
/// restated now that the type
/// with the other two fields exists. `digest()` is public because a receipt carries exactly that
/// component (42 §3.10), and reading it is not comparing.
#[test]
fn the_digest_alone_is_not_the_comparison() {
    let narrow = fingerprint("/tmp/x", 7);
    let wide = fingerprint("/tmp/x+lockfiles", 7);
    assert_eq!(narrow.digest(), wide.digest());
    assert!(narrow.cas_eq(&wide).is_err());
}

// ---------------------------------------------------------------------------
// Intent (E-M4-2)
// ---------------------------------------------------------------------------

fn intent(locator: &str, goal: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        locator.to_string(),
        GoalBytes(goal.to_vec()),
        ChangeContext::Evidence,
        Actor::Human {
            key: "operator".to_string(),
        },
    )
}

/// The five fields of 42 §3.3, read back.
#[test]
fn an_intent_is_the_five_fields_of_42_3_3() {
    let i = intent("/tmp/x", b"\xa1\x61k\x01");
    assert_eq!(i.substrate(), &SubstrateKind::Fs);
    assert_eq!(i.locator(), "/tmp/x");
    assert_eq!(i.goal(), &GoalBytes(b"\xa1\x61k\x01".to_vec()));
    assert_eq!(i.context(), &ChangeContext::Evidence);
    assert_eq!(
        i.actor().key(),
        "operator",
        "P-7: the key is reachable without branching on the variant"
    );
}

/// The locator is taken as written. Normalising it is the adapter's (**E-M4-12**).
///
/// gx-core with a path grammar would be gx-core deciding what `..` means for a substrate it is
/// forbidden to know about (41 §6). The contract that says who does normalise is in
/// `gx-substrate`'s crate documentation, and `substrate_contract.rs` is what keeps it written.
#[test]
fn an_intent_does_not_normalise_its_locator() {
    let awkward = "/etc/../etc//passwd/";
    assert_eq!(intent(awkward, b"").locator(), awkward);
}

/// `GoalBytes` prints its length and not its content (P-6).
#[test]
fn a_goal_is_opaque_in_a_log_line() {
    let text = format!("{:?}", GoalBytes(vec![1, 2, 3]));
    assert_eq!(text, "GoalBytes(opaque, 3 bytes)");
    assert!(
        !text.contains('1') || text.contains("3 bytes"),
        "the bytes themselves are not printed"
    );
}

/// In a human-readable format a goal is one base64 string, not a list of numbers.
///
/// The same pair M2H1-4 settled for every raw byte string in this crate: a derive would write
/// `[161, 97, 107, 1]`, whose encoded length depends on the values (42 §1.1 makes the point about
/// `Cid`). The binary face is DAG-CBOR's byte string, which only gx-canon can name (A-1), so what
/// gx-core can check is the readable half -- `crates/gx-canon/tests/intent_identity.rs` checks that
/// the projection lands on the canonical wire face.
#[test]
fn a_goal_is_base64_in_a_human_readable_format() {
    let json = serde_json::to_string(&GoalBytes(b"\xa1\x61k\x01".to_vec()))
        .expect("a goal has a readable form");
    assert!(json.starts_with('"') && json.ends_with('"'), "{json}");
    assert_eq!(
        json,
        format!("\"{}\"", gx_core::b64::encode(b"\xa1\x61k\x01"))
    );
}
