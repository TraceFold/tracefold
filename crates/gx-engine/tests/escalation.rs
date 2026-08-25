// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-5 / E-6 / M5H4-6** — the ticket's clock, the ticket's name, and ASM-14's second receipt.
//!
//! Spec: 43 T-4c for where a ticket comes from, 43 T-5/T-5b for what a person does with it, 42 §3.8
//! for the ticket's fields, 42 §1.3 for its `IdentityView`, 42 §3.10 and ASM-14 for the two receipt
//! kinds, 41 §6 for who owns the clock.
//!
//! Three rulings land here and each one is a probe rather than a comment:
//!
//! > **E-5** (`req/38_ERRATA_2026-08-07.md` §23): the ticket's `created_at` is injected by the
//! >   engine
//! > **E-6** (§23): reading a ticket back requires a checked constructor (the same shape as M2's
//! >   `ReceiptPayload`)
//! > **M5H4-6** (§41): `VerdictReceipt` is **implemented for T-4a/b/c too, in the same turn as
//! >   hand 6's T-5/T-5b implementation** (sem: SEM-gx-engine-725)
//!
//! # Why E-5 needs a probe and not just a line of code
//!
//! gx-gate says what it cannot do, in its own source: "`created_at` is the one field with no honest
//! source. 41 §6 keeps clocks out of this layer…so the value written is `Timestamp(0)` -- the epoch,
//! as a placeholder the engine overwrites when it records the ticket" (sem: SEM-gx-engine-726).
//! `Timestamp(0)` is a **legal
//! value**, so an engine that forgot to overwrite it would produce a ticket that verifies, encodes,
//! and claims to have been raised in 1970. That is E-M4-31's shape one type over, and hand 4's
//! answer there was the same: a probe that fails when the placeholder survives.

mod support;

use std::sync::Arc;

use gx_canon::cid;
use gx_core::{Timestamp, VerdictKind};
use gx_engine::{Engine, HumanRuling, InjectedEvidence, Lifecycle};
use gx_gate::TicketId;
use support::{gate, intent, ruler, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const RULED_AT: Timestamp = Timestamp(1_754_000_060_000_000_000);

/// An engine holding one `Escalated` transformation, and its id.
fn escalated(
    name: &str,
) -> (
    Engine<InjectedEvidence>,
    gx_core::TransformationId,
    std::sync::Arc<support::Counts>,
) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/ticketed.txt", "after");
    engine.submit(&i, 90, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &signing_key(), None).expect("T-4c"),
        Lifecycle::Escalated
    );
    (engine, id, counts)
}

// ---------------------------------------------------------------------------
// E-5
// ---------------------------------------------------------------------------

/// 🔴 **E-5**: the engine's clock reaches the ticket, and gx-gate's placeholder does not survive.
#[test]
fn e_5_the_engine_injects_the_tickets_created_at() {
    let (engine, id, _counts) = escalated("esc_e5");
    let ticket = engine.ticket(&id).expect("T-4c raised one");
    println!(
        "E5_TICKET id={:?} created_at={:?} transformation={:?} reasons={}",
        ticket.id,
        ticket.created_at,
        ticket.transformation,
        ticket.reasons.len()
    );
    assert_eq!(
        ticket.created_at, AT,
        "E-5: \"the ticket's `created_at` is injected by the engine\" (sem: SEM-gx-engine-727)"
    );
    assert_ne!(
        ticket.created_at,
        Timestamp(0),
        "the placeholder gx-gate writes reached the store -- the same failure E-M4-31 names for \
         `AppliedDelta.applied_at`"
    );
    assert_eq!(ticket.transformation, id, "42 §3.8's `transformation`");
    assert_eq!(
        ticket.reasons.len(),
        1,
        "E-M3-4's `INVERSE_UNAVAILABLE` is the one reason v0.1 escalates for"
    );
}

/// **ASM-4**: injecting the clock cannot move the ticket's identity.
///
/// 42 §1.3 gives `EscalationTicket` the projection `{transformation, reasons, required_approval}`,
/// and `created_at` is outside it. That is what makes E-5 safe: the engine writes a field the CID
/// does not cover, so a ticket recorded at two different moments is one ticket. Measured rather
/// than assumed, because if the projection ever grew the field this probe is where it would show.
#[test]
fn the_injected_clock_is_outside_the_tickets_identity() {
    let (engine, id, _counts) = escalated("esc_asm4");
    let ticket = engine.ticket(&id).expect("one").clone();
    let with_now = cid::compute(&ticket).expect("canonical");
    let mut moved = ticket.clone();
    moved.created_at = Timestamp(0);
    let with_epoch = cid::compute(&moved).expect("canonical");
    println!(
        "TICKET_CID with_now={} with_epoch={} equal={}",
        cid::to_text(&with_now),
        cid::to_text(&with_epoch),
        with_now == with_epoch
    );
    assert_eq!(
        with_now, with_epoch,
        "ASM-4 keeps `created_at` out of the IdentityView"
    );
    assert_eq!(
        TicketId(with_now),
        ticket.id,
        "and the id the engine kept is the digest of what the ticket holds (E-6)"
    );
}

// ---------------------------------------------------------------------------
// E-6
// ---------------------------------------------------------------------------

/// 🔴 **E-6**: a ticket whose name disagrees with its contents is refused at the door.
///
/// "reading back requires a checked constructor" (sem: SEM-gx-engine-728). The door is the
/// boundary between gx-gate and the engine,
/// and the check is [`gx_engine::Error::InconsistentTicket`].
///
/// # 🔴 The refusal has no reachable producer in v0.1, and that is said rather than hidden
///
/// The only party that builds an `EscalationTicket` in this workspace is gx-gate's own `escalate`,
/// which mints the id from the projection two lines after building the value — so no ticket
/// arriving at this engine can disagree with its own name, and the `Err` arm is unreachable in
/// v0.1. It is written anyway for the reason `Rollback::NotAttempted` and `InverseStatus::Expired`
/// are: the day a second producer exists (44's `POST /v1/candidates/{id}/escalation` carries a
/// ticket id from a request body — M6), the door is already there. Raised as **M5H6-8**.
///
/// What this probe can measure is the invariant itself, over the value the engine holds: the
/// projection's digest **is** the id, a tampered id no longer agrees with it, and the refusal is a
/// named kind in the declared vocabulary rather than a `Malformed`.
#[test]
fn e_6_a_ticket_that_does_not_hash_to_its_own_name_is_refused() {
    let (engine, id, _counts) = escalated("esc_e6");
    let good = engine.ticket(&id).expect("one").clone();

    // The checked constructor's two directions, exercised over the value the engine holds.
    let mut tampered = good.clone();
    tampered.id = TicketId(gx_core::Cid([9u8; 32]));
    let recomputed = TicketId(cid::compute(&tampered).expect("canonical"));
    println!(
        "E6_TICKET good={:?} tampered={:?} recomputed={:?} agrees={}",
        good.id,
        tampered.id,
        recomputed,
        recomputed == tampered.id
    );
    assert_eq!(
        recomputed, good.id,
        "the projection did not change, so the honest name is still the old one"
    );
    assert_ne!(
        recomputed, tampered.id,
        "which is exactly the disagreement E-6 asks to be caught"
    );

    // And the error the engine answers with exists, is in the declared vocabulary, and is its own
    // variant rather than a `Malformed` (see `lib.rs` for why).
    assert!(
        gx_engine::ERROR_KINDS.contains(&"InconsistentTicket"),
        "E-6's refusal is not in the vocabulary table"
    );
    let refusal = gx_engine::Error::InconsistentTicket {
        detail: "example".to_string(),
    };
    assert_eq!(refusal.kind(), "InconsistentTicket");
    assert_ne!(refusal.kind(), "Malformed");
}

// ---------------------------------------------------------------------------
// M5H4-6 -- ASM-14's two kinds, both real
// ---------------------------------------------------------------------------

/// 🔴 **M5H4-6**: `VerdictReceipt` has a producer, and the two kinds are issued at different times.
///
/// §41 wrote the state of affairs this probe ends: "**the only receipt that actually exists in
/// v0.1 is `CommitReceipt`** (one of ASM-14's two kinds is unimplemented)" (sem:
/// SEM-gx-engine-729). Four claims:
///
/// 1. a verdict-stage receipt exists **before** anything is committed;
/// 2. it satisfies ASM-14's obligations for its kind (no proof, no postcondition, no inverse) —
///    which `Receipt::issue` checks before signing, so an unsatisfiable one is never signed;
/// 3. a second one is added by the human ruling, signed under the ruler's key (43 T-5's
///    "append to the provenance chain" (sem: SEM-gx-engine-730));
/// 4. the `CommitReceipt` that follows is a **different** kind, and both survive.
#[test]
fn m5h4_6_both_of_asm_14s_receipt_kinds_are_issued() {
    let (mut engine, id, counts) = escalated("esc_kinds");

    let at_verdict = engine.verdict_receipts(&id).to_vec();
    let first = at_verdict[0].payload().expect("decodes");
    println!(
        "ASM14_VERDICT count={} kind={:?} verdict={:?} inclusion={} postcondition={} inverse={} \
         leaves={} applies={}",
        at_verdict.len(),
        first.receipt_kind,
        first.verdict.as_ref().map(|v| v.kind),
        first.inclusion_proof.is_some(),
        first.postcondition_fingerprint.is_some(),
        first.inverse_delta.is_some(),
        engine.ledger().log().len(),
        counts.totals()[4]
    );
    assert_eq!(at_verdict.len(), 1, "T-4c issued one");
    assert_eq!(first.receipt_kind, gx_witness::ReceiptKind::VerdictReceipt);
    assert_eq!(
        first.verdict.as_ref().map(|v| v.kind),
        Some(VerdictKind::Escalate),
        "42 §3.10: \"every `Verdict` is issued as Admit/Deny/Escalate\" (sem: SEM-gx-engine-731)"
    );
    assert!(
        !first.inclusion_proof.is_some(),
        "ASM-14: \"always `None`\" (sem: SEM-gx-engine-731)"
    );
    assert!(first.postcondition_fingerprint.is_none());
    assert!(first.inverse_delta.is_none(), "escrow is 43 T-10b");
    assert_eq!(engine.ledger().log().len(), 0, "nothing is committed yet");

    let owner_key = gx_witness::KeyPair::from_seed("key-owner-9", &[19u8; 32]);
    engine
        .escalation(
            &id,
            &HumanRuling {
                decision: VerdictKind::Admit,
                reason: "the inverse can be rebuilt by hand".to_string(),
                actor: ruler(9),
            },
            RULED_AT,
            &owner_key,
        )
        .expect("T-5");
    engine.canonicalize(&id, RULED_AT, None).expect("T-8");
    engine.commit(&id, RULED_AT, &signing_key()).expect("T-11");

    let chain = engine.verdict_receipts(&id);
    let commit = engine
        .receipt(&id)
        .expect("T-11 issued one")
        .payload()
        .expect("decodes");
    let keys: Vec<String> = chain
        .iter()
        .map(|r| r.payload().expect("decodes").key_id)
        .collect();
    println!(
        "ASM14_CHAIN verdict_receipts={} keys={keys:?} commit_kind={:?} commit_inclusion={} \
         leaves={}",
        chain.len(),
        commit.receipt_kind,
        commit.inclusion_proof.is_some(),
        engine.ledger().log().len()
    );
    assert_eq!(chain.len(), 2, "T-4c's, then T-5's");
    assert_eq!(
        keys,
        vec![signing_key().key_id().clone(), owner_key.key_id().clone()],
        "43 T-5's receipt is the ruler's, not the engine's"
    );
    assert_eq!(commit.receipt_kind, gx_witness::ReceiptKind::CommitReceipt);
    assert!(
        commit.inclusion_proof.is_some(),
        "ASM-14: \"`CommitReceipt` is mandatory (`Some`)\" (sem: SEM-gx-engine-732)"
    );
    assert_eq!(engine.ledger().log().len(), 1);
}

/// A verdict receipt is checkable by a stranger with no ledger (AC-018's shape, one hand later).
///
/// The point of issuing them at all: 42 §3.10's two kinds exist so that a **verdict** can be
/// witnessed before anything is applied. `InclusionCheck::NotApplicable` is AC-018's "skipped"
/// (sem: SEM-gx-engine-733) and
/// not a pass that hides a missing check — `Checks::verified()` is what draws that line.
#[test]
fn a_verdict_receipt_verifies_offline_with_no_anchor() {
    let (engine, id, _counts) = escalated("esc_offline");
    let receipt = &engine.verdict_receipts(&id)[0];
    let checks = gx_witness::receipt::verify_offline(receipt, &signing_key().verifying(), None)
        .expect("a verdict receipt needs no anchor");
    println!(
        "VERDICT_RECEIPT_CHECKS {checks:?} verified={}",
        checks.verified()
    );
    assert!(checks.canonical_cid, "42 §3.10's `canonical_cid` agrees");
    assert_eq!(
        checks.inclusion,
        gx_witness::receipt::InclusionCheck::NotApplicable
    );
    assert!(checks.verified());
}
