// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-070 (FR-018, CommitReceipt) — the ledger claim, checked offline against a known checkpoint.
//! (sem: SEM-gx-witness-168, SEM-gx-witness-169, SEM-gx-witness-170, SEM-gx-witness-171,
//! SEM-gx-witness-172)
//!
//! AC-070 verbatim: "Given: a `CommitReceipt` for a Transformation that already committed
//! successfully (a DSSE envelope carrying `VerdictReceipt`'s content plus an inclusion proof and an
//! inverse CID). When: signature verification + inclusion proof verification (matched against a
//! known checkpoint) is run in an offline environment. Then: verification succeeds (`Ok(true)`,
//! `checks.inclusion=true`). Also confirm that an invalid CommitReceipt with the inclusion proof
//! missing fails verification (a schema violation or `Ok(false)`)." Judgement method:
//! `integration`, M2.
//!
//! # What makes "offline" possible, and where it stopped being free
//!
//! A verifier here holds a receipt and a checkpoint. It does not hold the log, the entry, or the
//! neighbouring leaves. That works because 42 §3.11 makes `LedgerLeaf` `{transformation,
//! receipt_digest, index}` and a `CommitReceipt` carries all three -- the transformation in its
//! payload, the index in `inclusion_proof.leaf_index`, and the digest through
//! [`ReceiptPayload::ledger_digest`]. So the leaf is *rebuilt* rather than fetched, and the audit
//! path walk is `gx_log`'s (`verify_inclusion_of`, added by this hand for exactly this caller).
//!
//! 🔴 The third of those is a **derivation**, because 42 §3.11's literal "the whole of the DSSE envelope bytes"
//! has no value at the moment 43 T-11 appends: the append precedes the receipt, and the proof it
//! produces goes inside the payload the digest would have to cover. `ledger_digest`'s documentation
//! is the argument and req/54 §4 the ticket. `the_ledger_digest_excludes_exactly_two_things` below
//! measures what is and is not inside it, so the deviation is a number rather than a paragraph.
//!
//! # The checkpoint's own signature is a different question
//!
//! [`verify_offline`] does not check it: the ledger key may differ from the receipt key (45
//! ASM-45-1), and one `Ok` meaning two things is how a verifier comes to be trusted for something
//! it never did. `checkpoint_signature.rs` is where a checkpoint's signature is checked.

mod support;

use gx_core::{Checkpoint, Cid, InclusionProof, VerdictKind};
use gx_witness::receipt::{verify_offline, InclusionCheck, Receipt, ReceiptKind};
use gx_witness::Error;
use support::{commit_receipt_in_a_log, issue, keypair, tid, verdict_payload};

/// One named edit to a payload; see `receipt_kind_branch.rs` for why it is a `type`.
type Edit = Box<dyn Fn(&mut gx_witness::ReceiptPayload)>;

// ---------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------

/// AC-070 verbatim: signature plus inclusion proof, against a known checkpoint, with no ledger.
#[test]
fn ac_070_a_commit_receipt_verifies_against_a_known_checkpoint() {
    let key = keypair(1);
    let (receipt, head) = commit_receipt_in_a_log(&key, 42, 6);

    let checks =
        verify_offline(&receipt, &key.verifying(), Some(&head)).expect("a commit receipt verifies");
    assert!(checks.verified(), "AC-070's Ok(true): {checks:?}");
    assert_eq!(
        checks.inclusion,
        InclusionCheck::Verified,
        "AC-070's `checks.inclusion=true`"
    );
}

/// The path is not the trivial one. A log of a single leaf has an empty audit path, and a verifier
/// that ignored the path entirely would pass the test above.
#[test]
fn ac_070_the_audit_path_is_actually_walked() {
    let key = keypair(2);
    for others in [0u64, 1, 3, 7, 8, 15] {
        let (receipt, head) = commit_receipt_in_a_log(&key, 100 + others, others);
        let payload = receipt.payload().expect("decodes");
        let proof = payload
            .inclusion_proof
            .clone()
            .expect("a commit receipt has one");

        assert_eq!(proof.tree_size, others + 1);
        let checks = verify_offline(&receipt, &key.verifying(), Some(&head)).expect("verifies");
        assert_eq!(checks.inclusion, InclusionCheck::Verified);
        println!(
            "AC070_TREE_SIZE={} AUDIT_PATH_LEN={}",
            proof.tree_size,
            proof.audit_path.len()
        );
    }
}

/// AC-070's second half: "an invalid CommitReceipt with the inclusion proof missing fails
/// verification (a schema violation or `Ok(false)`)". The first alternative is the one taken -- a `CommitReceipt` with no proof
/// is not a receipt at all under ASM-14, and refusing it at the schema is stronger than answering
/// `Ok(false)` about a claim it never made.
#[test]
fn ac_070_a_commit_receipt_with_no_inclusion_proof_is_refused() {
    let key = keypair(3);
    let (receipt, head) = commit_receipt_in_a_log(&key, 7, 3);
    let mut payload = receipt.payload().expect("decodes");
    payload.inclusion_proof = None;

    // Refused at issue, so the invalid receipt is never signed...
    assert!(matches!(
        Receipt::issue(&payload, support::issued_at(), &key),
        Err(Error::Schema { .. })
    ));

    // ...and refused at verification too, for a receipt that arrived from somewhere else. Built by
    // signing the payload through the envelope directly, which is the road an outside producer has.
    let forged = forge(&payload, &key);
    match verify_offline(&forged, &key.verifying(), Some(&head)) {
        Err(Error::Schema { detail }) => assert!(detail.contains("inclusion proof")),
        other => panic!("expected a schema refusal, got {other:?}"),
    }
}

/// A proof that does not reach the anchor's root is `Ok(false)`, not an error: the receipt is
/// well-formed and its claim about the ledger is untrue, which is a verdict rather than a fault.
#[test]
fn ac_070_a_proof_that_does_not_reach_the_root_is_refuted() {
    let key = keypair(4);
    let (receipt, head) = commit_receipt_in_a_log(&key, 11, 5);

    let elsewhere = Checkpoint {
        root_hash: Cid([0xcc; 32]),
        ..head.clone()
    };
    let checks = verify_offline(&receipt, &key.verifying(), Some(&elsewhere))
        .expect("a well-formed receipt gives a verdict, not an error");
    assert_eq!(checks.inclusion, InclusionCheck::Refuted);
    assert!(!checks.verified(), "Ok(false)");
}

/// A tampered audit path is refuted. The proof is inside the signed payload, so this receipt is
/// forged rather than edited -- an attacker who can sign is the only one who can offer it, which is
/// the case the merkle check exists for after the signature has already passed.
#[test]
fn ac_070_a_forged_audit_path_is_refuted() {
    let key = keypair(5);
    let (receipt, head) = commit_receipt_in_a_log(&key, 13, 7);
    let mut payload = receipt.payload().expect("decodes");
    let mut proof = payload.inclusion_proof.clone().expect("some");
    assert!(!proof.audit_path.is_empty(), "the fixture must have a path");
    proof.audit_path[0] = Cid([0xee; 32]);
    payload.inclusion_proof = Some(proof);

    let forged = issue(&payload, &key);
    let checks = verify_offline(&forged, &key.verifying(), Some(&head)).expect("a verdict");
    assert_eq!(checks.inclusion, InclusionCheck::Refuted);
}

/// A proof naming another leaf's index is refuted, and so is one whose audit path was emptied.
///
/// 🔴 **v0.4 · H-09 changed one of the three, and the change is disclosed rather than absorbed.**
/// The middle case — a proof claiming `tree_size + 1` — used to be asserted as `Refuted` and is now
/// [`InclusionCheck::Unbridged`]. Nothing about the forgery changed; what changed is that the
/// verifier no longer pretends to have an opinion it cannot hold. The anchor commits to a tree of
/// `n` leaves and the proof is about a tree of `n + 1`: this head is **older** than the statement,
/// and an older head is silent about a later tree whether the receipt is honest or not. Saying
/// "refuted" there is the same false negative `req/222` measured in the other direction (a head
/// *newer* than the receipt), and a third party holding a stale checkpoint would have received an
/// accusation of tampering for a receipt with nothing wrong with it.
///
/// AC-070's own text asks for two things — a known checkpoint verifies, and a `CommitReceipt`
/// without a proof is refused (`req/spec/30-requirements/34-acceptance.md` line 49) — and says
/// nothing about tree sizes, so this is a probe's expectation moving and not an acceptance
/// criterion weakening. What must not move is that **none of the three is a pass**, and that is now
/// asserted directly, on `verified()`, rather than implied by the word.
#[test]
fn ac_070_a_proof_for_another_position_is_refuted() {
    let key = keypair(6);
    let (receipt, head) = commit_receipt_in_a_log(&key, 17, 7);
    let payload = receipt.payload().expect("decodes");
    let original = payload.inclusion_proof.clone().expect("some");

    for (wrong, expected) in [
        (
            InclusionProof {
                leaf_index: original.leaf_index + 1,
                ..original.clone()
            },
            InclusionCheck::Refuted,
        ),
        (
            InclusionProof {
                tree_size: original.tree_size + 1,
                ..original.clone()
            },
            InclusionCheck::Unbridged,
        ),
        (
            InclusionProof {
                audit_path: Vec::new(),
                ..original.clone()
            },
            InclusionCheck::Refuted,
        ),
    ] {
        let mut tampered = payload.clone();
        tampered.inclusion_proof = Some(wrong);
        let forged = issue(&tampered, &key);
        let checks = verify_offline(&forged, &key.verifying(), Some(&head)).expect("a verdict");
        println!("AC070_WRONG_PROOF {:?}", checks.inclusion);
        assert_eq!(checks.inclusion, expected);
        assert!(
            !checks.verified(),
            "whatever the word, a forged proof is not a pass"
        );
    }
}

/// A `CommitReceipt` for a *different* transformation does not verify against this leaf, even
/// though the proof and the anchor are genuine. The leaf covers the transformation as well as the
/// digest (42 §3.11), and this is what that buys.
#[test]
fn ac_070_a_proof_moved_onto_another_transformation_is_refuted() {
    let key = keypair(7);
    let (receipt, head) = commit_receipt_in_a_log(&key, 19, 4);
    let mut payload = receipt.payload().expect("decodes");
    payload.transformation = tid(999);
    payload.canonical_cid = payload.transformation.0;

    let forged = issue(&payload, &key);
    let checks = verify_offline(&forged, &key.verifying(), Some(&head)).expect("a verdict");
    assert_eq!(checks.inclusion, InclusionCheck::Refuted);
}

// ---------------------------------------------------------------------------
// The anchor, and the refusal to fail open without one
// ---------------------------------------------------------------------------

/// A `CommitReceipt` verified with no anchor is **not** a pass.
///
/// Its signature is good and its ledger claim was never examined, and req/29 §4's "a skip and a
/// pass must not look the same" is the whole reason [`InclusionCheck::Unanchored`] is a value rather than a
/// silent `NotApplicable`. This is the difference between AC-018's `"skipped"` (nothing to check)
/// and a check that could not be made.
#[test]
fn ac_070_a_commit_receipt_without_an_anchor_does_not_pass() {
    let key = keypair(8);
    let (receipt, _) = commit_receipt_in_a_log(&key, 23, 3);

    let checks = verify_offline(&receipt, &key.verifying(), None).expect("the signature is good");
    assert_eq!(checks.inclusion, InclusionCheck::Unanchored);
    assert!(
        !checks.verified(),
        "an unchecked ledger claim reported as verified"
    );
}

// ---------------------------------------------------------------------------
// What CM-5 buys the ledger
// ---------------------------------------------------------------------------

/// `issued_at` does not reach the ledger digest (E-M2-6).
///
/// The consequence is the one that matters operationally: `ledger.append` is keyed on the
/// transformation and treats a repeat carrying the same digest as a no-op (43 ASM-43-1), so a retry
/// that re-issues the same decision at a later clock is idempotent. A timestamp inside the digest
/// would have turned it into `Error::Conflict`.
#[test]
fn ac_070_two_receipts_differing_only_in_their_clock_share_a_digest() {
    let key = keypair(9);
    let payload = verdict_payload(VerdictKind::Admit, &key, 31);
    let early = Receipt::issue(&payload, gx_core::Timestamp(1), &key).expect("legal");
    let late = Receipt::issue(&payload, gx_core::Timestamp(2_000_000_000), &key).expect("legal");

    assert_ne!(early.issued_at, late.issued_at);
    assert_eq!(
        early.ledger_digest().expect("canonical"),
        late.ledger_digest().expect("canonical"),
        "the clock reached the ledger digest"
    );
}

/// 🔴 What the ledger's commitment does and does not cover, measured field by field.
///
/// Two exclusions, both forced (see [`ReceiptPayload::ledger_digest`]): the signature, and the
/// inclusion proof. **Every other field changes the digest**, which is the half that keeps the leaf
/// binding -- a reader who knows only that the rule deviates from 42 §3.11 cannot tell whether it
/// deviates by two fields or by nine, and this counts them.
///
/// # req/1053 F-1 (`req/994` "ac_070 mutant旧11 field"): the count is derived, not written
///
/// This probe used to close with `assert_eq!(moved, 10)` against a struct the header above once
/// called "eleven fields" -- and the struct's own doc comment (`src/receipt.rs`, just above the
/// `pub struct ReceiptPayload` line) already names hand-written field counts as a defect class: "a
/// hand-written count in prose is a second copy of a number that is already measured mechanically
/// -- the same defect this lane retracted in 44". `ReceiptPayload` grew from eleven fields to
/// twenty across four errata (`ac_018.rs`'s own `assert_eq!(fields.len(), 20, ...)` measures the
/// same struct the same way) while this probe's `mutations` array and its `assert_eq!(moved, 10)`
/// stayed at the shape written for the eleven-field struct -- nine fields (`confinement`,
/// `catalogue_hash`, `read_set`, `reversibility`, `undo`, `determinism_boundary`, `receipt_kind`,
/// `fingerprint_scope`, `payload_version`, `engine_version` minus the one this window adds no
/// coverage for) were silently unmeasured: had a refactor dropped one of them from
/// `ledger_digest`'s input, this test would still have printed `AC070_LEDGER_DIGEST_COVERS=10_OF_11`
/// and passed. The fix below is the same one `ac_018.rs` already applies to 42 §3.10's table: parse
/// the field list off `src/receipt.rs` at test time and assert the *set* of covered names against
/// it, so a field added to or dropped from the struct without a matching mutation here fails loudly
/// instead of leaving a coverage hole under a green checkmark.
#[test]
fn ac_070_the_ledger_digest_excludes_exactly_two_things() {
    let key = keypair(10);
    let (receipt, _) = commit_receipt_in_a_log(&key, 37, 3);
    let payload = receipt.payload().expect("decodes");
    let baseline = payload.ledger_digest().expect("canonical");

    // Excluded 1: the inclusion proof. Any proof, and none at all, give one digest.
    let mut without = payload.clone();
    without.inclusion_proof = None;
    assert_eq!(without.ledger_digest().expect("canonical"), baseline);

    // Excluded 2: the signature, and with it `issued_at` -- neither is in `ReceiptPayload` at all,
    // which is 42 §1.3-4 and E-M2-6 respectively rather than anything this hand chose.
    let resigned = issue(&payload, &keypair(11));
    assert_ne!(resigned.envelope.signatures, receipt.envelope.signatures);
    assert_eq!(resigned.ledger_digest().expect("canonical"), baseline);

    // Included: everything else. One field at a time, each must move the digest.
    let mutations: [(&str, Edit); 18] = [
        ("key_id", Box::new(|p| p.key_id = "other".to_string())),
        // 🔴 **E-M5-11**: the seat is an `Option`, so the mutations reach through it. A payload
        // whose verdict is `None` here would be a fixture that is not the one under test, which is
        // why these `expect` rather than falling back to a default.
        (
            "verdict.kind",
            Box::new(|p| {
                p.verdict.as_mut().expect("the fixture has a verdict").kind = VerdictKind::Deny;
            }),
        ),
        (
            "verdict.proof_digest",
            Box::new(|p| {
                p.verdict
                    .as_mut()
                    .expect("the fixture has a verdict")
                    .proof_digest = Cid([9; 32]);
            }),
        ),
        ("enforced", Box::new(|p| p.enforced = !p.enforced)),
        (
            "canonical_cid",
            Box::new(|p| p.canonical_cid = Cid([8; 32])),
        ),
        ("inverse_delta", Box::new(|p| p.inverse_delta = None)),
        ("transformation", Box::new(|p| p.transformation = tid(4242))),
        (
            "fail_posture_engaged",
            Box::new(|p| p.fail_posture_engaged = !p.fail_posture_engaged),
        ),
        // 🔴 req/1053 F-1 -- the ten fields below this line had no mutation before this window.
        // `commit_receipt_in_a_log` builds a `CommitReceipt` (`support::commit_payload`), so the
        // baseline each closure moves away from is that fixture's, not `verdict_payload`'s.
        ("confinement", Box::new(|p| p.confinement = None)),
        (
            "catalogue_hash",
            Box::new(|p| p.catalogue_hash = Some("ac070-mutant-hash".to_string())),
        ),
        (
            "read_set",
            Box::new(|p| p.read_set = Some(gx_witness::receipt::ReadSet::Nothing)),
        ),
        (
            "reversibility",
            Box::new(|p| p.reversibility = Some(gx_core::Reversibility::False)),
        ),
        (
            "undo",
            Box::new(|p| {
                p.undo = Some(gx_witness::receipt::UndoAttestation {
                    undoes: tid(4343),
                    witness: gx_witness::receipt::UndoDisposition::Attested,
                });
            }),
        ),
        (
            "determinism_boundary",
            Box::new(|p| p.determinism_boundary = gx_core::DeterminismBoundary::Unknown),
        ),
        (
            "receipt_kind",
            Box::new(|p| p.receipt_kind = ReceiptKind::VerdictReceipt),
        ),
        (
            "fingerprint_scope",
            Box::new(|p| p.fingerprint_scope = "ac070-mutant-scope".to_string()),
        ),
        ("payload_version", Box::new(|p| p.payload_version = None)),
        ("engine_version", Box::new(|p| p.engine_version = None)),
    ];
    let mut covered: std::collections::BTreeSet<String> = mutations
        .iter()
        .map(|(name, _)| {
            name.split('.')
                .next()
                .expect("a mutation name has at least one segment")
                .to_string()
        })
        .collect();
    for (name, mutate) in mutations {
        let mut altered = payload.clone();
        mutate(&mut altered);
        assert_ne!(
            altered.ledger_digest().expect("canonical"),
            baseline,
            "{name} is not covered by the ledger digest"
        );
    }
    // The two fingerprints too, which the closure list cannot express without a second type.
    let mut altered = payload.clone();
    altered.precondition_fingerprint = support::fingerprint(99);
    assert_ne!(altered.ledger_digest().expect("canonical"), baseline);
    covered.insert("precondition_fingerprint".to_string());
    let mut altered = payload.clone();
    altered.postcondition_fingerprint = None;
    assert_ne!(altered.ledger_digest().expect("canonical"), baseline);
    covered.insert("postcondition_fingerprint".to_string());

    // The derived check: parse `ReceiptPayload`'s field names off the source `ac_018.rs` already
    // reads mechanically, and require the covered set (above) plus the one deliberate exclusion to
    // equal it exactly -- not a count, a set, so an added field lands as a named miss rather than an
    // off-by-one.
    let struct_src = include_str!("../src/receipt.rs");
    let body = struct_src
        .split("pub struct ReceiptPayload {")
        .nth(1)
        .expect("receipt.rs declares ReceiptPayload")
        .split("\n}")
        .next()
        .expect("split always yields one");
    let struct_fields: std::collections::BTreeSet<String> = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub ") && l.ends_with(','))
        .map(|l| {
            l.trim_start_matches("pub ")
                .split(':')
                .next()
                .expect("a field declaration line has a colon")
                .trim()
                .to_string()
        })
        .collect();
    let excluded: std::collections::BTreeSet<String> =
        std::iter::once("inclusion_proof".to_string()).collect();
    let accounted: std::collections::BTreeSet<String> =
        covered.union(&excluded).cloned().collect();
    println!(
        "AC070_LEDGER_DIGEST_COVERS={}_OF_{} EXCLUDED={:?}",
        covered.len(),
        struct_fields.len(),
        excluded
    );
    assert_eq!(
        accounted,
        struct_fields,
        "ReceiptPayload's fields and this test's coverage disagree -- in the struct but not \
         measured here: {:?}; measured here but not in the struct: {:?}",
        struct_fields.difference(&accounted).collect::<Vec<_>>(),
        accounted.difference(&struct_fields).collect::<Vec<_>>()
    );
    assert_eq!(
        receipt.payload().expect("decodes").receipt_kind,
        ReceiptKind::CommitReceipt
    );
}

// ---------------------------------------------------------------------------

/// Sign a payload the schema refuses, the way an outside producer could.
///
/// [`Receipt::issue`] checks the schema before signing, so an invalid receipt cannot be produced by
/// this crate. A verifier still has to refuse one that arrived from elsewhere, and this builds that
/// article: the envelope is assembled and signed directly, which is what any DSSE implementation
/// would do.
fn forge(payload: &gx_witness::ReceiptPayload, key: &gx_witness::KeyPair) -> Receipt {
    use gx_witness::dsse::{DsseEnvelope, RECEIPT_PAYLOAD_TYPE};

    let mut envelope = DsseEnvelope {
        payload_type: RECEIPT_PAYLOAD_TYPE.to_string(),
        payload: gx_canon::cbor::encode(payload).expect("canonical"),
        signatures: Vec::new(),
    };
    envelope.sign(key.signing_key(), key.key_id());
    Receipt {
        envelope,
        issued_at: support::issued_at(),
    }
}
