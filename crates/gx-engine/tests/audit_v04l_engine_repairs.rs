// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 v0.4-l small repairs (`req/189`) — the engine half of `req/182`'s H-04 / H-05 / M-01.
//!
//! Each test below is a **refusing test**: it was run against the coordinates `req/182` §1 names
//! before the repair (RED, the raw failure lines are quoted in `req/189` §3) and after (GREEN).
//! Nothing here asserts a happy path that was already green somewhere else.
//!
//! | # | detection (`req/182`) | what is measured here |
//! |---|---|---|
//! | H-04 | the escalation ticket never left the row after a ruling / cancel, and Σ rehydration rebuilt it for a ruled row | ticket ⇔ `Escalated`, live and across a restart |
//! | H-05 | a second `undo(T_o)` re-minted the same `T_u.id`, appended a second `Planned` and rewound a denied `T_u` to `Candidate` (Σ ≠ live) | refused by name; Σ and the live table stay bit-equal |
//! | M-01 | `verdict_checkpoint`'s window was an unchecked `u64` subtraction; a journal shorter than the chain would sign a wrapped count | refused by name after a torn tail |

mod support;

use std::io::Write;
use std::sync::Arc;

use gx_core::{Timestamp, VerdictKind};
use gx_engine::{reconstruct, Engine, HumanRuling, InjectedEvidence, Lifecycle};
use support::{
    gate, gate_refusing, intent, record_boundaries, ruler, scratch, signing_key, CommitAdapter,
    MaybeEvidence, PERMIT_ALL,
};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const LATER: Timestamp = Timestamp(1_754_000_120_000_000_000);

// ---------------------------------------------------------------------------
// H-04
// ---------------------------------------------------------------------------

/// 🔴 H-04, live: a ruling clears the ticket; so does a cancel.
///
/// Before the repair `entry.ticket` had exactly one writer (T-4c) and no eraser, so
/// `Engine::ticket` kept answering `Some` for a row that was `Admitted` — and `GET /escalations`,
/// which filtered on the ticket alone, listed it as still waiting.
#[test]
fn h04_a_ruling_and_a_cancel_both_take_the_ticket_off_the_row() {
    let dir = scratch("v04l_h04_live");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter.without_inverse());
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(adapter, "commit-adapter-1");

    // Row 1: escalated (E-M3-4: no inverse) then ruled.
    let ruled = intent("/tmp/v04l_h04_ruled.txt", "after");
    engine.submit(&ruled, 1, AT).expect("submit");
    let ruled_id = engine.plan(&ruled, AT).expect("plan");
    assert_eq!(
        engine
            .verify(&ruled_id, AT, &signing_key(), None)
            .expect("T-4c"),
        Lifecycle::Escalated
    );
    let ticket_while_waiting = engine.ticket(&ruled_id).cloned();
    let after_ruling = engine
        .escalation(
            &ruled_id,
            &HumanRuling {
                decision: VerdictKind::Admit,
                reason: "v0.4-l H-04: ruled, so the queue entry is spent".to_string(),
                actor: ruler(1),
            },
            LATER,
            &signing_key(),
        )
        .expect("T-5");
    let ticket_after_ruling = engine.ticket(&ruled_id).cloned();

    // Row 2: escalated then cancelled (T-7 → `abort`).
    let cancelled = intent("/tmp/v04l_h04_cancelled.txt", "after");
    engine.submit(&cancelled, 2, AT).expect("submit");
    let cancelled_id = engine.plan(&cancelled, AT).expect("plan");
    assert_eq!(
        engine
            .verify(&cancelled_id, AT, &signing_key(), None)
            .expect("T-4c"),
        Lifecycle::Escalated
    );
    let after_cancel = engine.cancel(&cancelled_id, LATER).expect("T-7");
    let ticket_after_cancel = engine.ticket(&cancelled_id).cloned();

    println!(
        "H04_LIVE waiting_ticket={} after_ruling_state={after_ruling:?} ticket_after_ruling={} \
         after_cancel_state={after_cancel:?} ticket_after_cancel={}",
        u8::from(ticket_while_waiting.is_some()),
        u8::from(ticket_after_ruling.is_some()),
        u8::from(ticket_after_cancel.is_some())
    );
    assert!(
        ticket_while_waiting.is_some(),
        "the fixture escalates (control: the ticket exists while somebody is waiting)"
    );
    assert_eq!(after_ruling, Lifecycle::Admitted);
    assert_eq!(
        ticket_after_ruling, None,
        "H-04: a ruling resolves the escalation, so the ticket is gone from the row"
    );
    assert!(matches!(after_cancel, Lifecycle::Aborted(_)));
    assert_eq!(
        ticket_after_cancel, None,
        "H-04: a cancel resolves the escalation the same way"
    );
    // Denominator, stated: the reverse map (`transformation_of_ticket`, 44 §1.2's `<TICKET_ID>`)
    // already answered `None` for a ruled row before this repair — Σ's `verdict` becomes the
    // ruling's kind at T-5, so the scan skips the row ("6 = not found"). Not asserted here as a
    // change; measured on this run: spent ticket → None.
    let spent = ticket_while_waiting.expect("checked above").id;
    println!(
        "H04_SPENT_TICKET_RESOLVES={:?}",
        engine.transformation_of_ticket(&spent).expect("consistent")
    );
}

/// 🔴 H-04, across a restart: Σ rehydration rebuilds a ticket **only** for a row still `Escalated`.
///
/// `tests/ticket_rehydration.rs` measures the positive half (an `Escalated` row gets its ticket
/// back). This is the negative half that was missing: before the repair the rebuild keyed on the
/// journalled **verdict** alone. Measured before the repair (`req/189` §3): a *ruled* row was
/// already clean (T-5 rewrites Σ's verdict), but a *cancelled* escalation keeps `Escalate` as its
/// verdict and came back from every restart **with a ticket** — the H-04 leak, permanent. So the
/// fixture holds three rows: ruled, cancelled, still waiting.
#[test]
fn h04_a_ruled_row_rehydrates_without_a_ticket_and_an_escalated_row_with_one() {
    let dir = scratch("v04l_h04_rehydrate");
    let journal = dir.join("journal.bin");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter.without_inverse());
    let ruled = intent("/tmp/v04l_h04_r_ruled.txt", "after");
    let cancelled = intent("/tmp/v04l_h04_r_cancelled.txt", "after");
    let waiting = intent("/tmp/v04l_h04_r_waiting.txt", "after");

    let (ruled_id, cancelled_id, waiting_id) = {
        let mut engine = Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        engine.register_adapter(adapter.clone(), "commit-adapter-1");
        engine.submit(&ruled, 3, AT).expect("submit");
        let ruled_id = engine.plan(&ruled, AT).expect("plan");
        engine
            .verify(&ruled_id, AT, &signing_key(), None)
            .expect("T-4c");
        engine
            .escalation(
                &ruled_id,
                &HumanRuling {
                    decision: VerdictKind::Deny,
                    reason: "v0.4-l H-04: ruled before the restart".to_string(),
                    actor: ruler(2),
                },
                LATER,
                &signing_key(),
            )
            .expect("T-5b");
        engine.submit(&cancelled, 12, AT).expect("submit");
        let cancelled_id = engine.plan(&cancelled, AT).expect("plan");
        engine
            .verify(&cancelled_id, AT, &signing_key(), None)
            .expect("T-4c");
        engine.cancel(&cancelled_id, LATER).expect("T-7");
        engine.submit(&waiting, 4, AT).expect("submit");
        let waiting_id = engine.plan(&waiting, AT).expect("plan");
        engine
            .verify(&waiting_id, AT, &signing_key(), None)
            .expect("T-4c");
        (ruled_id, cancelled_id, waiting_id)
    };

    let mut engine =
        Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none()).expect("reopen");
    engine.register_adapter(adapter, "commit-adapter-1");
    assert_eq!(engine.plan(&ruled, AT).expect("rehydrate"), ruled_id);
    assert_eq!(engine.plan(&waiting, AT).expect("rehydrate"), waiting_id);
    // The cancelled row rehydrates too (a resume re-plans and takes the state from Σ), and its
    // ticket is also read through the reverse map, the road a resumed `gx escalation` takes.
    assert_eq!(
        engine.plan(&cancelled, AT).expect("rehydrate"),
        cancelled_id
    );
    let cancelled_ticket = engine.ticket(&cancelled_id).cloned();
    let cancelled_ticket_resolves = engine
        .transformation_of_ticket(
            &gx_gate::escalation_ticket(cancelled_id)
                .expect("canonical")
                .id,
        )
        .expect("consistent");
    let ruled_ticket = engine.ticket(&ruled_id).cloned();
    let waiting_ticket = engine.ticket(&waiting_id).cloned();
    println!(
        "H04_REHYDRATE ruled_state={:?} ruled_verdict={:?} ruled_ticket={} cancelled_state={:?}          cancelled_ticket={} cancelled_ticket_resolves={:?} waiting_state={:?} waiting_ticket={}",
        engine.state(&ruled_id),
        engine.verdict(&ruled_id),
        u8::from(ruled_ticket.is_some()),
        engine.state(&cancelled_id),
        u8::from(cancelled_ticket.is_some()),
        cancelled_ticket_resolves,
        engine.state(&waiting_id),
        u8::from(waiting_ticket.is_some())
    );
    assert!(matches!(
        engine.state(&cancelled_id),
        Some(Lifecycle::Aborted(_))
    ));
    assert_eq!(
        cancelled_ticket, None,
        "H-04: a cancelled escalation (verdict still `Escalate`, state `Aborted`) rehydrates          without a ticket — the arm that leaked before the repair"
    );
    assert_eq!(
        cancelled_ticket_resolves, None,
        "H-04: a cancelled escalation's ticket is spent — the reverse map does not resurrect it          from Σ (verdict `Escalate`, state `Aborted`)"
    );
    assert_eq!(engine.state(&ruled_id), Some(Lifecycle::Denied));
    assert_eq!(
        engine.verdict(&ruled_id),
        Some(VerdictKind::Deny),
        "the ruling is the row's verdict now (T-5b), and that is not what decides the ticket"
    );
    assert_eq!(
        ruled_ticket, None,
        "H-04: a ruled row comes back from a restart with no ticket"
    );
    assert_eq!(engine.state(&waiting_id), Some(Lifecycle::Escalated));
    assert!(
        waiting_ticket.is_some(),
        "and the row still waiting comes back with its ticket (ticket_rehydration.rs's positive half)"
    );
}

// ---------------------------------------------------------------------------
// H-05
// ---------------------------------------------------------------------------

/// 🔴 H-05: undo → verify (Deny) → undo again is refused, and Σ stays bit-equal to the live table.
///
/// `T_u`'s id is a CID over the IdentityView and `created_at` is outside it, so the second `undo`
/// mints the same id. Before the repair it appended a second `Planned` and re-seated `T_u` as a
/// `Candidate`: the live table forgot the `Deny`, Σ did not — AC-039 broken by an HTTP retry.
#[test]
fn h05_a_second_undo_of_the_same_commit_is_refused_and_sigma_stays_bit_equal() {
    let dir = scratch("v04l_h05");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate_refusing(PERMIT_ALL, "no-rollback", "before"),
        MaybeEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/v04l_h05.txt", "after");
    engine.submit(&i, 5, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    engine.commit(&t_o, AT, &signing_key()).expect("T-11");

    let (_, t_u) = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 6, LATER)
        .expect("the first undo plans T_u");
    assert_eq!(
        engine
            .verify(&t_u, LATER, &signing_key(), None)
            .expect("T-4b"),
        Lifecycle::Denied,
        "the fixture gate denies the inverse payload (ac_040's Deny road)"
    );
    let records_before = engine.journal().len();
    let live_before = engine
        .sigma()
        .canonical_bytes()
        .expect("Σ has a canonical form");

    let second = engine.undo(&t_o, &engine.attested_postcondition(&t_o), 7, LATER);
    let live_after = engine
        .sigma()
        .canonical_bytes()
        .expect("Σ has a canonical form");
    let replayed = reconstruct(engine.journal().records())
        .canonical_bytes()
        .expect("Σ has a canonical form");
    println!(
        "H05 second_undo={:?} t_u_state={:?} records_before={records_before} records_after={} \
         live_unchanged={} live_eq_replay={}",
        second.as_ref().map(|_| "Ok").map_err(|e| e.kind()),
        engine.state(&t_u),
        engine.journal().len(),
        u8::from(live_before == live_after),
        u8::from(live_after == replayed)
    );
    let refusal = second.expect_err("H-05: a second undo re-mints T_u and T_u has left Candidate");
    assert_eq!(refusal.kind(), "InvalidState");
    assert_eq!(
        engine.state(&t_u),
        Some(Lifecycle::Denied),
        "H-05: the denied undo is not rewound to Candidate"
    );
    assert_eq!(
        engine.journal().len(),
        records_before,
        "H-05: no second `Planned` for the same id"
    );
    assert_eq!(live_before, live_after);
    assert_eq!(
        live_after, replayed,
        "AC-039: `sigma().canonical_bytes() == reconstruct(journal).canonical_bytes()`"
    );
}

/// The same scenario across a restart — **denominator arm**: measured, the second undo is
/// refused *before* the H-05 guard is reached (`undo` asks the escrow table for `T_o`'s inverse
/// and a restarted engine holds no escrow row — `NotFound`, the H-02 family `req/182` names).
/// The journal-reading arm of the guard is therefore exercised by code path only, not by this
/// fixture; kept because the assertion that matters (no second `Planned` for a journalled
/// `Denied` T_u) holds either way, and so that the day H-02 rehydrates escrow rows this test
/// starts measuring the guard itself.
#[test]
fn h05_across_a_restart_no_second_planned_is_appended_for_a_denied_t_u() {
    let dir = scratch("v04l_h05_restart");
    let journal = dir.join("journal.bin");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter);
    let i = intent("/tmp/v04l_h05_restart.txt", "after");
    let (t_o, t_u) = {
        let mut engine = Engine::open(
            &journal,
            gate_refusing(PERMIT_ALL, "no-rollback", "before"),
            MaybeEvidence::none(),
        )
        .expect("a fresh journal opens");
        engine.register_adapter(adapter.clone(), "commit-adapter-1");
        engine.submit(&i, 8, AT).expect("submit");
        let t_o = engine.plan(&i, AT).expect("plan");
        engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
        engine.canonicalize(&t_o, AT, None).expect("T-8");
        engine.commit(&t_o, AT, &signing_key()).expect("T-11");
        let (_, t_u) = engine
            .undo(&t_o, &engine.attested_postcondition(&t_o), 9, LATER)
            .expect("first undo");
        engine
            .verify(&t_u, LATER, &signing_key(), None)
            .expect("T-4b");
        (t_o, t_u)
    };
    let mut engine = Engine::open(
        &journal,
        gate_refusing(PERMIT_ALL, "no-rollback", "before"),
        MaybeEvidence::none(),
    )
    .expect("reopen");
    engine.register_adapter(adapter, "commit-adapter-1");
    // `T_o` has to be back on the table for `undo` to reach the guard at all (M5H3-5: `open`
    // leaves the table empty; the CLI's resume road is a re-plan).
    assert_eq!(engine.plan(&i, AT).expect("rehydrate T_o"), t_o);
    let records_before = engine.journal().len();
    let second = engine.undo(&t_o, &engine.attested_postcondition(&t_o), 10, LATER);
    println!(
        "H05_RESTART second_undo={:?} records_before={records_before} records_after={} sigma_t_u={:?}",
        second.as_ref().map(|_| "Ok").map_err(|e| e.kind()),
        engine.journal().len(),
        reconstruct(engine.journal().records())
            .state_of(&t_u)
            .and_then(|r| r.state)
    );
    // Whether the guard reaches `InvalidState` here or `undo` refuses earlier is not the point —
    // what must not happen is a second `Planned` for a `T_u` the journal already holds as `Denied`.
    assert!(
        second.is_err(),
        "the denied T_u is not re-planned across a restart"
    );
    assert_eq!(
        engine.journal().len(),
        records_before,
        "no second `Planned`"
    );
}

// ---------------------------------------------------------------------------
// M-01
// ---------------------------------------------------------------------------

/// 🔴 M-01: a journal shorter than the chain folded from it cannot issue a checkpoint.
///
/// Issue one checkpoint over one verdict, cut the journal back to before that verdict (a torn
/// tail, truncated by the next `open`), reopen, issue again: `published.admit == 1 >
/// verdicts.admit == 0`. Before the repair that was `0u64 - 1` — a debug panic, or in release a
/// wrapped count signed into the chain. Now a named refusal, and the chain does not grow.
#[test]
fn m01_a_journal_shorter_than_the_published_chain_refuses_to_issue() {
    let dir = scratch("v04l_m01");
    let journal = dir.join("journal.bin");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter);
    let key = signing_key();
    let i = intent("/tmp/v04l_m01.txt", "after");
    let chain_len = {
        let mut engine =
            Engine::open(&journal, gate(PERMIT_ALL), MaybeEvidence::none()).expect("open");
        engine.register_adapter(adapter.clone(), "commit-adapter-1");
        engine.submit(&i, 11, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        engine.verify(&id, AT, &signing_key(), None).expect("T-4a");
        let first = engine
            .verdict_checkpoint("glovrex-verdicts/v1", LATER, &key)
            .expect("the first checkpoint closes a window of one Admit");
        assert_eq!(first.tally.admit, 1);
        // Control: a second issue over an unchanged journal is an empty window, not a refusal.
        let second = engine
            .verdict_checkpoint("glovrex-verdicts/v1", LATER, &key)
            .expect("an empty window is a true statement about a quiet period");
        assert_eq!(second.tally.admit, 0);
        engine.verdict_checkpoints().len()
    };

    // Cut the journal back to before its last record (the `Verdict`): the tail is now torn.
    let whole = std::fs::read(&journal).expect("read the journal back");
    let boundaries = record_boundaries(&whole);
    let last = *boundaries.last().expect("at least one record");
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&journal)
            .expect("rewrite the journal");
        file.write_all(&whole[..last]).expect("write the prefix");
        file.write_all(&whole[last..last + 3])
            .expect("and three bytes of the torn last record");
    }

    let mut engine =
        Engine::open(&journal, gate(PERMIT_ALL), MaybeEvidence::none()).expect("reopen");
    engine.register_adapter(adapter, "commit-adapter-1");
    let torn = engine.journal().recovery().torn_tail_bytes;
    let refused = engine.verdict_checkpoint("glovrex-verdicts/v1", LATER, &key);
    println!(
        "M01 chain_before={chain_len} torn_tail_bytes={torn} issue_after_cut={:?} chain_after={}",
        refused
            .as_ref()
            .map(|c| c.tally.admit)
            .map_err(|e| e.kind()),
        engine.verdict_checkpoints().len()
    );
    assert!(torn > 0, "the fixture produced a torn tail");
    let refusal = refused.expect_err("M-01: published (1 admit) > counted (0 admit) is refused");
    assert_eq!(refusal.kind(), "Malformed");
    assert!(
        refusal.to_string().contains("shorter than the chain"),
        "the refusal names the cause: {refusal}"
    );
    assert_eq!(
        engine.verdict_checkpoints().len(),
        chain_len,
        "and the chain did not grow"
    );
}
