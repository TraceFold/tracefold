//! ASM-14's two receipts, and the branch offline verification takes between them.
//!
//! The hand's DoD asks that 「VerdictReceipt(3 verdict)と CommitReceipt の 2 種が offline 検証で
//! 分岐する事を test で示す」. AC-018 and AC-070 each show one side; this file is the table, so that
//! 「they differ」 is a checked claim rather than an inference from two files that never meet.
//!
//! 42 §3.10 逐語: 「両者とも同一の`DsseEnvelope`/`ReceiptPayload`スキーマを共有し、
//! `ReceiptPayload.receipt_kind`で判別する」. One schema, one discriminant, and four differences --
//! three fields whose permitted values depend on the kind, and one verification outcome.
//!
//! # The three fields, and where each rule comes from
//!
//! | field | `VerdictReceipt` | source |
//! |---|---|---|
//! | `inclusion_proof` | `None` | ASM-14 / 42 §3.10 「`VerdictReceipt`は常に`None`」 |
//! | `postcondition_fingerprint` | `None` | 42 §3.10 「`VerdictReceipt`では常にNone」 |
//! | `inverse_delta` | `None` | 42 §3.10 「escrowはcommit中の43 T-10bで行われる」 |
//!
//! # And the field that is deliberately *not* checked
//!
//! `enforced`. 42 §3.10 says 「`VerdictReceipt`では意味を持たないため`true`固定」 and 35 ASM-13 with
//! 43 T-4e say a verdict-stage receipt records `enforced=false` together with
//! `fail_posture_engaged=true`. req/49 §3 M2-9's 既定案 -- 「field を 1 本足し、`VerdictReceipt` の
//! true 固定を条件つきに読み替える」 -- is taken in both halves: E-M2-7 added the flag, and this hand
//! reads the 固定 conditionally by leaving it out of the schema. A check that enforced it would
//! refuse the receipt ASM-13 requires, which is why `both_postures_are_legal` exists.

mod support;

use gx_core::VerdictKind;
use gx_witness::receipt::{verify_offline, InclusionCheck, Receipt, ReceiptKind, ReceiptPayload};
use gx_witness::Error;
use support::{commit_receipt_in_a_log, fingerprint, issue, issued_at, keypair, verdict_payload};

/// One named edit to a payload, for the table-driven cases below. A `type` because clippy counts
/// the boxed closure inside an array as a complex type, and it is right that the name reads better.
type Edit = Box<dyn Fn(&mut ReceiptPayload)>;

// ---------------------------------------------------------------------------
// The branch
// ---------------------------------------------------------------------------

/// The same verifier, the same key, the same anchor -- and two different answers, chosen by
/// `receipt_kind` alone.
#[test]
fn the_two_kinds_take_different_roads_through_one_verifier() {
    let key = keypair(1);
    let (commit, head) = commit_receipt_in_a_log(&key, 5, 4);
    let verdict = issue(&verdict_payload(VerdictKind::Admit, &key, 5), &key);

    let with_anchor = verify_offline(&verdict, &key.verifying(), Some(&head)).expect("verifies");
    let commit_checks = verify_offline(&commit, &key.verifying(), Some(&head)).expect("verifies");

    // A verdict receipt ignores the anchor entirely: there is nothing in the ledger to check.
    assert_eq!(with_anchor.inclusion, InclusionCheck::NotApplicable);
    let without_anchor = verify_offline(&verdict, &key.verifying(), None).expect("verifies");
    assert_eq!(
        with_anchor, without_anchor,
        "the anchor changed a verdict receipt's answer"
    );

    // A commit receipt does not.
    assert_eq!(commit_checks.inclusion, InclusionCheck::Verified);
    assert_eq!(
        verify_offline(&commit, &key.verifying(), None)
            .expect("verifies")
            .inclusion,
        InclusionCheck::Unanchored,
        "the anchor made no difference to a commit receipt"
    );

    assert!(with_anchor.verified() && commit_checks.verified());
}

/// All four `InclusionCheck` values are reachable, each from the situation it names. A variant no
/// input produces is a variant a reader can misread without consequence.
#[test]
fn every_inclusion_outcome_is_reachable() {
    use gx_core::{Checkpoint, Cid};

    let key = keypair(2);
    let (commit, head) = commit_receipt_in_a_log(&key, 6, 3);
    let verdict = issue(&verdict_payload(VerdictKind::Deny, &key, 6), &key);
    let elsewhere = Checkpoint {
        root_hash: Cid([1u8; 32]),
        ..head.clone()
    };

    let seen = [
        verify_offline(&verdict, &key.verifying(), None)
            .expect("v")
            .inclusion,
        verify_offline(&commit, &key.verifying(), Some(&head))
            .expect("v")
            .inclusion,
        verify_offline(&commit, &key.verifying(), Some(&elsewhere))
            .expect("v")
            .inclusion,
        verify_offline(&commit, &key.verifying(), None)
            .expect("v")
            .inclusion,
    ];
    assert_eq!(
        seen,
        [
            InclusionCheck::NotApplicable,
            InclusionCheck::Verified,
            InclusionCheck::Refuted,
            InclusionCheck::Unanchored,
        ]
    );
    // Two of the four are passes and two are not, which is the whole content of `verified()`.
    assert!(matches!(seen[0], InclusionCheck::NotApplicable));
    assert!(!matches!(seen[3], InclusionCheck::Verified));
}

// ---------------------------------------------------------------------------
// The three kind-dependent fields
// ---------------------------------------------------------------------------

/// A `VerdictReceipt` carrying anything ASM-14 puts on the commit side is refused, one field at a
/// time so a failure names which rule was broken.
#[test]
fn a_verdict_receipt_carrying_a_commit_field_is_refused() {
    let key = keypair(3);
    let (commit, _) = commit_receipt_in_a_log(&key, 8, 2);
    let borrowed = commit.payload().expect("decodes");

    let cases: [(&str, Edit); 3] = [
        (
            "inclusion proof",
            Box::new({
                let proof = borrowed.inclusion_proof.clone();
                move |p: &mut ReceiptPayload| p.inclusion_proof = proof.clone()
            }),
        ),
        (
            "postcondition fingerprint",
            Box::new(|p: &mut ReceiptPayload| p.postcondition_fingerprint = Some(fingerprint(3))),
        ),
        (
            "inverse delta",
            Box::new(|p: &mut ReceiptPayload| p.inverse_delta = Some(support::cid(1))),
        ),
    ];

    for (name, mutate) in cases {
        let mut payload = verdict_payload(VerdictKind::Admit, &key, 8);
        mutate(&mut payload);
        match payload.check_schema() {
            Err(Error::Schema { detail }) => assert!(
                detail.contains(name),
                "the refusal does not name {name}: {detail}"
            ),
            other => panic!("a verdict receipt with {name} was accepted: {other:?}"),
        }
        assert!(matches!(
            Receipt::issue(&payload, issued_at(), &key),
            Err(Error::Schema { .. })
        ));
    }
}

/// And the commit side's own obligation, from the other direction.
#[test]
fn a_commit_receipt_without_its_proof_is_refused() {
    let key = keypair(4);
    let (commit, _) = commit_receipt_in_a_log(&key, 9, 2);
    let mut payload = commit.payload().expect("decodes");
    payload.inclusion_proof = None;
    assert!(matches!(payload.check_schema(), Err(Error::Schema { .. })));
}

/// The commit side may leave the two optional fields empty. 42 §3.10 makes only the proof
/// mandatory -- 「構成不能なら`CommitReceipt`でもNone」 for the inverse, and the postcondition is set
/// 「record-onlyモードで適用された場合」 -- so a schema that required all three would refuse legal
/// receipts.
#[test]
fn a_commit_receipt_may_omit_the_two_optional_fields() {
    let key = keypair(5);
    let (commit, head) = commit_receipt_in_a_log(&key, 10, 3);
    let mut payload = commit.payload().expect("decodes");
    payload.inverse_delta = None;
    payload.postcondition_fingerprint = None;

    // Legal: only the proof is mandatory on this kind.
    payload
        .check_schema()
        .expect("both fields are optional (42 §3.10)");
    let leaner = issue(&payload, &key);
    verify_offline(&leaner, &key.verifying(), None).expect("the signature is good");

    // And the ledger notices. Editing either field after the append changes the leaf, so the
    // proof no longer reaches the root -- which is the *other* half of the same fact and worth
    // asserting here rather than only in `ac_070`: a producer that fills these in must do it
    // before it appends, and a tamperer who can sign still cannot move them afterwards.
    let checks = verify_offline(&leaner, &key.verifying(), Some(&head)).expect("a verdict");
    assert_eq!(checks.inclusion, InclusionCheck::Refuted);
}

// ---------------------------------------------------------------------------
// The posture flags (E-M2-7, and the half of M2-9 that is a non-check)
// ---------------------------------------------------------------------------

/// Both postures are legal on both kinds. This is the conditional reading of 42 §3.10's 「`true`
/// 固定」, and the case that matters is the first: 35 ASM-13's verdict-stage receipt, with
/// `enforced=false` and `fail_posture_engaged=true`, which a literal reading of 42 would refuse.
#[test]
fn both_postures_are_legal_on_a_verdict_receipt() {
    let key = keypair(6);
    for (enforced, engaged) in [(false, true), (true, false), (true, true), (false, false)] {
        let mut payload = verdict_payload(VerdictKind::Admit, &key, 12);
        payload.enforced = enforced;
        payload.fail_posture_engaged = engaged;

        let receipt = Receipt::issue(&payload, issued_at(), &key)
            .unwrap_or_else(|e| panic!("enforced={enforced} engaged={engaged} refused: {e}"));
        assert!(verify_offline(&receipt, &key.verifying(), None)
            .expect("verifies")
            .verified());
        assert_eq!(
            receipt.payload().expect("decodes").fail_posture_engaged,
            engaged
        );
    }
}

/// E-M2-7's field is inside the signed core, which is the whole reason it was allowed in: it is
/// deterministic, unlike `issued_at`. Flipping it changes the payload bytes and therefore the
/// signature, so a receipt cannot be re-labelled after the fact.
#[test]
fn the_posture_flag_is_covered_by_the_signature() {
    let key = keypair(7);
    let engaged = issue(&verdict_payload(VerdictKind::Escalate, &key, 13), &key);

    let mut payload = engaged.payload().expect("decodes");
    payload.fail_posture_engaged = false;
    let relabelled = issue(&payload, &key);

    assert_ne!(
        engaged.envelope.payload, relabelled.envelope.payload,
        "the flag is not in the signed bytes"
    );
    assert_ne!(
        engaged.envelope.signatures[0].sig, relabelled.envelope.signatures[0].sig,
        "two different payloads share a signature"
    );
}

/// And `issued_at` is outside it, which is E-M2-6. The two tests together are CM-5's rule stated as
/// a pair of measurements: deterministic fields in, clock reads out.
#[test]
fn the_clock_is_not_covered_by_the_signature() {
    let key = keypair(8);
    let payload = verdict_payload(VerdictKind::Admit, &key, 14);
    let early = Receipt::issue(&payload, gx_core::Timestamp(1), &key).expect("legal");
    let late = Receipt::issue(&payload, gx_core::Timestamp(9_999), &key).expect("legal");

    assert_eq!(
        early.envelope, late.envelope,
        "the clock reached the envelope"
    );
    assert_ne!(early.issued_at, late.issued_at);
    // Each verifies, and neither verification looked at the field.
    for receipt in [&early, &late] {
        assert!(verify_offline(receipt, &key.verifying(), None)
            .expect("verifies")
            .verified());
    }
}

/// The kind itself is signed. A `VerdictReceipt` relabelled as a `CommitReceipt` -- which is how an
/// attacker would claim a ledger entry that does not exist -- changes the payload and breaks the
/// signature; and a forger who can sign still meets the schema check.
#[test]
fn the_kind_cannot_be_changed_after_signing() {
    let key = keypair(9);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 15), &key);

    let mut payload = receipt.payload().expect("decodes");
    payload.receipt_kind = ReceiptKind::CommitReceipt;
    assert!(
        matches!(payload.check_schema(), Err(Error::Schema { .. })),
        "a relabelled verdict receipt passed the schema"
    );
}

// ---------------------------------------------------------------------------
// M5H6-8① — the pairing rule 42 §3.10 never wrote down
// ---------------------------------------------------------------------------

/// 🔴 **M5H6-8① 採(a)** (`req/38_ERRATA_2026-08-07.md` §43), verbatim:
///
/// > **M5H6-8 ①採(a)・実装窓=fix批**: gx-witness `check_schema` に「`verdict=None` ⇒
/// > `fail_posture_engaged=true`」の対規則を足す(fail-closed の二重防衛・42 §3.10 erratum 相当)。
///
/// # Why this is a *second* defence and not the first one
///
/// The first is the producer: `gx_engine::Error::Unrepresentable` refuses to build a payload whose
/// verdict and posture disagree (`pipeline.rs`'s `(None, None) if entry.fail_posture_engaged`
/// arm), and hand 6 recorded that as the whole of the enforcement. The gap that leaves is the one
/// every schema exists to close — **a receipt is a wire format, and a wire format has producers
/// this repository did not write**. A payload that reaches `check_schema` from a decoder, a fuzz
/// corpus, or M6's API face has never met the engine's refusal. `tests/receipt_verdict_wire.rs`
/// wrote the question down at the time (「Whether `check_schema` should carry the same rule is
/// raised in the hand's report as a ticket, not decided here」) and §43 answered it.
///
/// # What the rule says, in the direction it says it
///
/// `verdict = None` ⇒ `fail_posture_engaged = true`. One direction only. The converse is legal and
/// has to be: 43 T-4e engages the posture *and* an operator may run a degraded posture that still
/// reached a gate, so `fail_posture_engaged = true` with a verdict present is an ordinary receipt.
/// `both_postures_are_legal` above holds that half, and
/// `the_pairing_rule_refuses_only_the_shape_it_names` below holds it again against this rule.
#[test]
fn a_receipt_with_no_verdict_and_no_posture_flag_is_refused() {
    let key = keypair(21);
    let mut payload = support::degraded_payload(&key, 21);
    assert!(payload.verdict.is_none() && payload.fail_posture_engaged);
    assert!(
        payload.check_schema().is_ok(),
        "43 T-4e's own shape must stay legal"
    );

    payload.fail_posture_engaged = false;
    let refused = payload.check_schema();
    println!("NO_VERDICT_NO_POSTURE={refused:?}");
    assert!(
        matches!(refused, Err(Error::Schema { .. })),
        "a receipt with no verdict and no fail posture says a commit happened for no stated \
         reason; ASM-14 has no such shape and §43 M5H6-8① rules it a schema error -- got {refused:?}"
    );
}

/// The rule refuses **only** the pair it names, on both kinds.
///
/// A negative control, in the shape §45 5.0.1 says a battery line needs: without it, a
/// `check_schema` that returned `Err` unconditionally would pass the probe above.
#[test]
fn the_pairing_rule_refuses_only_the_shape_it_names() {
    let key = keypair(22);

    // verdict present, posture engaged -- legal (a degraded run that still reached a gate).
    let mut both = support::verdict_payload(VerdictKind::Deny, &key, 22);
    both.fail_posture_engaged = true;
    assert!(both.check_schema().is_ok(), "{:?}", both.check_schema());

    // verdict present, posture clear -- the ordinary receipt.
    let mut plain = support::verdict_payload(VerdictKind::Admit, &key, 23);
    plain.fail_posture_engaged = false;
    assert!(plain.check_schema().is_ok(), "{:?}", plain.check_schema());

    // no verdict, posture engaged -- 43 T-4e, on both kinds.
    let degraded = support::degraded_payload(&key, 24);
    assert!(degraded.check_schema().is_ok());
    let (proof_receipt, _anchor) = commit_receipt_in_a_log(&key, 25, 3);
    let mut commit = proof_receipt.payload().expect("decodes");
    commit.verdict = None;
    commit.fail_posture_engaged = true;
    assert!(commit.check_schema().is_ok(), "{:?}", commit.check_schema());

    // and the same commit receipt with the flag cleared is the one shape refused.
    commit.fail_posture_engaged = false;
    assert!(
        matches!(commit.check_schema(), Err(Error::Schema { .. })),
        "the rule must reach a CommitReceipt too -- ASM-14's kinds share one schema"
    );
}

/// The producer side of the same rule: `Receipt::issue` checks the schema **before** it signs.
///
/// A valid signature over an impossible receipt is worse than no receipt, because it is the thing a
/// verifier trusts. `issue` already calls `check_schema`; this probe is what makes that ordering
/// load-bearing for the new rule rather than only for ASM-14's three fields.
#[test]
fn an_unpairable_payload_is_never_signed() {
    let key = keypair(26);
    let mut payload = support::degraded_payload(&key, 26);
    payload.fail_posture_engaged = false;
    let refused = Receipt::issue(&payload, issued_at(), &key);
    assert!(
        matches!(refused, Err(Error::Schema { .. })),
        "issue signed a payload its own schema refuses: {refused:?}"
    );
}
