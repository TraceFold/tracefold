// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! M2-10 (req/49 §3) — what a receipt records for each of the three verdicts.
//!
//! 42 §3.10 gives `ReceiptPayload.verdict` a `VerdictSummary { kind, proof_digest: Cid }` and says
//! only "embed not the whole `Verdict` but its CID'd digest" (sem: SEM-gx-gate-309) -- but "proof" names something only the
//! `Admit` arm has. M2 hand 5 met that and could implement none of it, because all three payload
//! types are this crate's (req/49 §1 N-01), so it wrote the rule into `VerdictSummary`'s own
//! documentation and left the ticket open:
//!
//! > "**M2-10**: the source of Deny/Escalate's `proof_digest` is undefined ... spell out the rule
//! > for taking Deny=`Vec<Reason>`'s and Escalate=`EscalationTicket`'s CID (**the type itself is
//! > M3**)" (sem: SEM-gx-gate-310)
//!
//! This is the hand the types exist in. [`Verdict::proof_digest`] is the rule as a function, and
//! this file is where it is put back together with the gx-witness side.
//!
//! # What the two crates each know
//!
//! gx-witness types the field and refuses a receipt without one; it cannot compute one. gx-gate
//! computes it and knows nothing about receipts. Neither crate can check the pairing alone, and
//! the edge runs gx-gate → gx-witness (`GateInput.evidence`, 41 §4), so the check belongs here.

use gx_canon::cid;
use gx_core::{Cid, TransformationId, VerdictKind};
use gx_gate::verdict::{DenyReasons, InvariantResult, PolicyDecisionRecord};
use gx_gate::{
    AdmitProof, ApprovalRequirement, EscalationTicket, Reason, ReasonSource, TicketId, Verdict,
    INVARIANT_VIOLATED, INVERSE_UNAVAILABLE, POLICY_DENIED,
};
use gx_witness::{PolicyDecision, VerdictSummary};

fn cid_of(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn reason(code: &str, id: &str) -> Reason {
    Reason::new(
        code,
        format!("{id} said so"),
        if code == POLICY_DENIED {
            ReasonSource::Policy {
                policy_id: id.to_string(),
            }
        } else {
            ReasonSource::Invariant {
                invariant_id: id.to_string(),
            }
        },
    )
    .expect("both codes are in the vocabulary")
}

fn admit() -> Verdict {
    Verdict::Admit(AdmitProof::new(
        vec![PolicyDecisionRecord {
            policy_id: "fs-permit-default".to_string(),
            decision: PolicyDecision::Allow,
            diagnostics_digest: None,
        }],
        vec![InvariantResult::new("a-check".to_string(), true, None).expect("short")],
        vec![cid_of(9)],
        vec![TransformationId(cid_of(1))],
        None,
    ))
}

fn deny() -> Verdict {
    Verdict::deny(vec![
        reason(POLICY_DENIED, "fs-deny-etc"),
        reason(INVARIANT_VIOLATED, "a-check"),
    ])
    .expect("two reasons")
}

fn escalation() -> EscalationTicket {
    EscalationTicket {
        id: TicketId(cid_of(0)),
        transformation: TransformationId(cid_of(1)),
        reasons: vec![Reason::new(
            INVERSE_UNAVAILABLE,
            "no inverse".to_string(),
            ReasonSource::Adapter {
                detail: "invert returned None (FR-043)".to_string(),
            },
        )
        .expect("in the vocabulary")],
        required_approval: ApprovalRequirement::default(),
        created_at: gx_core::Timestamp(1_754_600_000_000_000_000),
    }
}

// ---------------------------------------------------------------------------
// One rule per arm
// ---------------------------------------------------------------------------

/// `Admit` digests its [`AdmitProof`], over 42 §1.3's "every field". (sem: SEM-gx-gate-311)
#[test]
fn the_admit_arm_digests_its_proof() {
    let Verdict::Admit(proof) = admit() else {
        panic!("built as an Admit")
    };
    let expected = cid::compute(&proof).expect("digestible");
    assert_eq!(admit().proof_digest().expect("digestible"), expected);
}

/// `Deny` digests its `Vec<Reason>` -- M2-10's rule, and the reason E-M3-12 gives that vector an
/// order.
#[test]
fn the_deny_arm_digests_its_reasons() {
    let Verdict::Deny(reasons) = deny() else {
        panic!("built as a Deny")
    };
    let expected = cid::compute(&DenyReasons(&reasons)).expect("digestible");
    assert_eq!(deny().proof_digest().expect("digestible"), expected);
}

/// `Escalate` digests its [`EscalationTicket`], over 42 §1.3's `{transformation, reasons,
/// required_approval}`.
#[test]
fn the_escalate_arm_digests_its_ticket() {
    let ticket = escalation();
    let expected = cid::compute(&ticket).expect("digestible");
    assert_eq!(
        Verdict::Escalate(ticket)
            .proof_digest()
            .expect("digestible"),
        expected
    );
}

/// The ticket's own `id` is that same digest (42 §1.3), so a receipt and the engine's escalation
/// table name one thing.
#[test]
fn a_tickets_id_and_its_proof_digest_are_one_value() {
    let mut ticket = escalation();
    ticket.id = TicketId(cid::compute(&ticket).expect("digestible"));
    let digest = Verdict::Escalate(ticket.clone())
        .proof_digest()
        .expect("digestible");
    assert_eq!(ticket.id.0, digest);
}

/// Two fields 42 §1.3 excludes cannot move a digest.
///
/// `id` is self-referential and `created_at` is ASM-4 metadata. A ticket raised twice for the same
/// change, at different times, is one ticket.
#[test]
fn the_excluded_fields_do_not_reach_the_digest() {
    let one = escalation();
    let mut other = escalation();
    other.id = TicketId(cid_of(200));
    other.created_at = gx_core::Timestamp(1);

    assert_eq!(
        Verdict::Escalate(one).proof_digest().expect("digestible"),
        Verdict::Escalate(other).proof_digest().expect("digestible")
    );
}

/// The three arms are three different digests for the same change.
///
/// Not a tautology: a `proof_digest` that were taken over a common wrapper -- the `Verdict` enum
/// itself, say -- would still differ, but a digest taken over "the reasons" for both `Deny` and (sem: SEM-gx-gate-312)
/// `Escalate` would not, since an escalation carries reasons too. This asserts the arms are
/// distinguishable by the value the receipt holds, not only by the `kind` beside it.
#[test]
fn the_three_arms_do_not_collide() {
    let digests = [
        admit().proof_digest().expect("digestible"),
        deny().proof_digest().expect("digestible"),
        Verdict::Escalate(escalation())
            .proof_digest()
            .expect("digestible"),
    ];
    assert_ne!(digests[0], digests[1]);
    assert_ne!(digests[1], digests[2]);
    assert_ne!(digests[0], digests[2]);
}

// ---------------------------------------------------------------------------
// The pairing with gx-witness (42 §3.10)
// ---------------------------------------------------------------------------

/// Every arm produces a `VerdictSummary` a receipt can carry, with the discriminant E-M3-2 typed.
///
/// This is the whole of what M2 could not do. `VerdictSummary` has two fields and gx-gate answers
/// both: [`Verdict::kind`] and [`Verdict::proof_digest`]. A caller that had to build the pair by
/// hand -- match for the kind, digest for the payload -- could pair `Admit` with a `Deny`'s digest,
/// and nothing downstream would notice, because a `Cid` carries no evidence of what it was taken
/// over.
#[test]
fn every_verdict_becomes_a_verdict_summary() {
    for verdict in [admit(), deny(), Verdict::Escalate(escalation())] {
        let summary = VerdictSummary {
            kind: verdict.kind(),
            proof_digest: verdict.proof_digest().expect("digestible"),
        };
        assert_eq!(summary.kind, verdict.kind());
        assert_eq!(
            summary.proof_digest,
            verdict.proof_digest().expect("digestible")
        );
        assert!(matches!(
            summary.kind,
            VerdictKind::Admit | VerdictKind::Deny | VerdictKind::Escalate
        ));
    }
}

/// The digest is a function of the payload, so an `AdmitProof` that changes changes it.
#[test]
fn a_different_proof_is_a_different_digest() {
    let one = admit().proof_digest().expect("digestible");
    let other = Verdict::Admit(AdmitProof::new(
        vec![PolicyDecisionRecord {
            policy_id: "fs-permit-default".to_string(),
            decision: PolicyDecision::Allow,
            diagnostics_digest: None,
        }],
        vec![InvariantResult::new("a-check".to_string(), true, None).expect("short")],
        vec![cid_of(9)],
        // The one difference: a second lower verdict in the chain (E-M3-9 keeps its order).
        vec![TransformationId(cid_of(1)), TransformationId(cid_of(2))],
        None,
    ))
    .proof_digest()
    .expect("digestible");
    assert_ne!(one, other);
}

/// `composed_from` is order-bearing and the other three vectors are not (**E-M3-9**), and the
/// digest is where that difference is visible.
///
/// T5 asks for the chain of lower verdicts back, so `[f, g]` and `[g, f]` are different proofs;
/// `policy_decisions` given in either order is one proof. Hand 1 asserted the *order* of the
/// vectors after construction, and this is the same statement one layer out: what a receipt is
/// identified by.
#[test]
fn the_canonical_order_is_visible_in_the_digest() {
    let proof = |composed: Vec<TransformationId>, policies: Vec<&str>| {
        Verdict::Admit(AdmitProof::new(
            policies
                .into_iter()
                .map(|id| PolicyDecisionRecord {
                    policy_id: id.to_string(),
                    decision: PolicyDecision::Allow,
                    diagnostics_digest: None,
                })
                .collect(),
            Vec::new(),
            Vec::new(),
            composed,
            None,
        ))
        .proof_digest()
        .expect("digestible")
    };

    let chain = vec![TransformationId(cid_of(1)), TransformationId(cid_of(2))];
    let reversed = vec![TransformationId(cid_of(2)), TransformationId(cid_of(1))];

    assert_eq!(
        proof(chain.clone(), vec!["alpha", "bravo"]),
        proof(chain.clone(), vec!["bravo", "alpha"]),
        "the deciding policies are a set with an order imposed on it"
    );
    assert_ne!(
        proof(chain, vec!["alpha", "bravo"]),
        proof(reversed, vec!["alpha", "bravo"]),
        "composed_from is the chain T5 restores, and a chain read backwards is another chain"
    );
}
