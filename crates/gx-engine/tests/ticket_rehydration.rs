//! 🔴 **M6H3-10 採(b)** — measured first, and the measurement is the ruling's evidence.
//!
//! req/38 §50: 「**M6H3-10 採(b)・手4 が先に測る**: `EscalationTicket` が row から再構成できるかを
//! 実測してから journal 語彙増(a)を判定。**手4 brief の必須項**」.
//!
//! Hand 3 found that a rehydrated row loses four things the journal does not hold — the escalation
//! ticket, the verdict receipts, `blocked_by` and `since` — and named `gx escalation` as the verb
//! that would meet the first of them. This file is what was measured before anything was decided,
//! and it is kept as the standing statement of **why 42 §3.13 did not have to grow**.
//!
//! # The answer, and what it rests on
//!
//! `TicketId` is a digest of `{transformation, reasons, required_approval}` (42 §1.3, E-6), and in
//! v0.1 the last two are **constants**: `gx_gate::escalation_ticket` is the one road E-M3-4 takes and
//! it reads nothing but the id. ∴ a `TicketId` is a pure function of a `TransformationId`, and a
//! process holding only Σ can rebuild the ticket a journalled `Verdict::Escalate` raised.
//!
//! The structural half of that claim — 「exactly one generator, and it reads nothing else」 — is
//! `probes/doubt/tests/m6_surface_doubt.rs::exactly_one_road_builds_an_escalation_ticket`, because a
//! property of the source is not something a test run can establish. This file measures the
//! behaviour: what a second process loses, that it can get back, and that the value it gets back is
//! **the same value** rather than one that merely looks like it.

mod support;

use std::sync::Arc;

use gx_canon::cid;
use gx_core::{Timestamp, VerdictKind};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// 🔴 A second process loses the ticket and gets **the same one** back.
#[test]
fn a_rehydrated_row_recovers_the_ticket_the_journal_never_held() {
    let dir = scratch("m6h4_ticket_rehydration");
    let journal = dir.join("journal.bin");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter.without_inverse());

    let i = intent("/tmp/m6h4_escalated.txt", "after");
    let (id, live) = {
        let mut engine = Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        engine.register_adapter(adapter.clone(), "commit-adapter-1");
        engine.submit(&i, 42, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        let state = engine.verify(&id, AT, &signing_key(), None).expect("T-4c");
        assert_eq!(state, Lifecycle::Escalated, "E-M3-4's one condition");
        let ticket = engine.ticket(&id).cloned().expect("T-4c raised one");
        println!(
            "MEASURE_LIVE state={state:?} ticket_id={} reasons={} approval={:?} created_at={:?}",
            cid::to_text(&ticket.id.0),
            ticket.reasons.len(),
            ticket.required_approval,
            ticket.created_at
        );
        (id, ticket)
    };

    // A second process over the same journal: `Engine::open` + the CLI's resume (a re-plan).
    let mut engine =
        Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none()).expect("reopen");
    engine.register_adapter(adapter, "commit-adapter-1");
    let replanned = engine.plan(&i, AT).expect("rehydrate");
    assert_eq!(replanned, id, "43 T-2's idempotency column");

    let recovered = engine.ticket(&id).cloned();
    println!(
        "MEASURE_REHYDRATED same_id={} state={:?} verdict={:?} ticket_present={} equal={}",
        u8::from(replanned == id),
        engine.state(&id),
        engine.verdict(&id),
        u8::from(recovered.is_some()),
        u8::from(recovered.as_ref() == Some(&live))
    );
    assert_eq!(engine.verdict(&id), Some(VerdictKind::Escalate));
    let recovered = recovered.expect(
        "🔴 M6H3-10: the journal records the verdict and not the ticket (42 §3.13), and the ticket \
         is recovered rather than recorded — see `Engine::rebuilt_ticket`",
    );
    assert_eq!(
        recovered.id, live.id,
        "the identity is what the operator types, so it is the identity that has to survive"
    );
    assert_eq!(
        recovered, live,
        "🔴 and every field, not only the name: `created_at` comes from the journalled `Verdict` \
         record's own `at`, so a rebuilt ticket says when the gate answered rather than when \
         somebody resumed"
    );
}

/// 🔴 The reverse map 44 §1.2's `<TICKET_ID>` needs (**M6-04 採(a)**), out of Σ alone.
///
/// 43 T-4c declares 「ticket idは`TransformationId`に1:1紐付け」 and until this hand the mapping ran
/// one way. The other direction is asserted here in the process that has planned nothing, because
/// that is the process `gx escalation approve <TICKET_ID>` actually is.
#[test]
fn a_ticket_id_resolves_to_its_transformation_in_a_process_that_planned_nothing() {
    let dir = scratch("m6h4_ticket_resolution");
    let journal = dir.join("journal.bin");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter.without_inverse());

    let i = intent("/tmp/m6h4_resolution.txt", "after");
    let (id, ticket_id) = {
        let mut engine = Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        engine.register_adapter(adapter, "commit-adapter-1");
        engine.submit(&i, 42, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        engine.verify(&id, AT, &signing_key(), None).expect("T-4c");
        let ticket = engine.ticket(&id).expect("T-4c raised one").id;
        (id, ticket)
    };

    // No adapter is registered on purpose: resolving a ticket reads Σ and asks no substrate.
    let engine =
        Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none()).expect("reopen");
    let resolved = engine
        .transformation_of_ticket(&ticket_id)
        .expect("the rebuild is consistent");
    println!(
        "TICKET_RESOLUTION ticket={} resolved={:?} expected={} table_rows={}",
        cid::to_text(&ticket_id.0),
        resolved.map(|t| cid::to_text(&t.0)),
        cid::to_text(&id.0),
        u8::from(engine.transformation(&id).is_some())
    );
    assert_eq!(
        resolved,
        Some(id),
        "M6-04 採(a): the inverse of 43 T-4c's 1:1 declaration, computed from Σ"
    );
    assert!(
        engine.transformation(&id).is_none(),
        "and computed **without** rebuilding a row — the resolution has to work before a resume, \
         because a resume needs the id the resolution produces"
    );

    // A name nothing raised resolves to nothing, which is 44 §1.2's 「6=未検出（チケット不明）」 and
    // not an error: 「the ticket is unknown」 and 「the rebuild is broken」 are different facts.
    let absent = gx_gate::TicketId(gx_core::Cid([7u8; 32]));
    assert_eq!(
        engine
            .transformation_of_ticket(&absent)
            .expect("a miss is not a failure"),
        None
    );
}
