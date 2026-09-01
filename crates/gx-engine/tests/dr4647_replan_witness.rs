// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-47 (`req/973` §9-3, filed 2026-08-31, repaired here)** — a second `undo` of the same
//! `T_o` may not be answered with the **first** call's disposition.
//!
//! Spec: 42 §3.13 for `Planned.undo_witness`, 43 T-2 for the re-plan/idempotency column, 43 §5.2 for
//! the refusal table.
//!
//! # What was measured, and which direction the old defect lied in
//!
//! `T_u`'s id is a CID over the IdentityView and `created_at` is outside it, so a second
//! `undo(T_o)` re-mints **the same** id (H-05, `req/182` §1-1). H-05's guard therefore returns early
//! for a `T_u` still seated as a `Candidate` — and that early `return` sits *before* the `Planned`
//! append, so the second call's witness never reached the journal. Since DR-46-45 the journalled
//! witness is what T-11 signs into the receipt, so the receipt named a comparison that a *different*
//! call had made.
//!
//! Both calls run the compare-and-swap (the `match` on `witness` is above the guard), so no undo
//! could claim `Attested` without having compared: the lie was an **under**-claim, which is the
//! fail-closed side. That is why this is a repair and not an incident. It is still a lie in signed
//! bytes, and DR-46-45's whole subject is that a reader holding the receipt alone can tell the two
//! apart.
//!
//! # The repair this file pins, and the one it refuses
//!
//! `req/973` §9-3's release condition offered two roads: the second call updates the seat, or a
//! re-plan whose witness differs from the journalled one is refused. This lane took the **refusal**,
//! for two reasons a reader should be able to check rather than take:
//!
//! 1. "Update the seat" means appending a second `Planned` for an id that already has one, which is
//!    exactly what 43 T-2's idempotency column forbids on this road ("answered with its own id and
//!    no second record") and what an HTTP retry storm would turn into unbounded journal growth.
//! 2. A witness that disagrees is not a duplicate request. The two calls read the world at two
//!    different moments and got two different answers about whether it could be compared at all;
//!    answering the second with the first's receipt would be the engine choosing which of the two
//!    observations to publish. Refusing hands that choice back, which is 43 §5.2's posture.
//!
//! An **agreeing** second call keeps the old answer exactly — same id, no second record — because
//! there the journalled disposition already names what this call compared. That arm is the control
//! below, and without it this file would be measuring a refusal that had eaten idempotency.
//!
//! # The one legacy shape that is deliberately not refused
//!
//! `Planned.undo_witness` is `Option` and is `None` both for every `plan()` and for any journal
//! written before DR-46-45. `None` is "nobody recorded a comparison", not "a different comparison",
//! and folding it into the refusal would turn every pre-erratum project's re-plan into an
//! `InvalidState` on the strength of a field that did not exist when it was written. Unknown is not
//! False; the three-valued discipline is this system's own first principle.

mod support;

use std::sync::Arc;

use gx_core::{Timestamp, TransformationId};
use gx_engine::{
    Engine, EngineJournalRecord, InjectedEvidence, Lifecycle, UndoWitness, Unobservable,
};
use gx_witness::receipt::UndoDisposition;
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const LATER: Timestamp = Timestamp(1_754_000_120_000_000_000);

/// A committed `T_o` on a fresh journal, with the engine that made it.
fn committed(name: &str) -> (Engine<InjectedEvidence>, TransformationId) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent(&format!("/tmp/{name}.txt"), "after");
    engine.submit(&i, 470, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    assert_eq!(
        engine.commit(&t_o, AT, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );
    (engine, t_o)
}

/// The disposition the journal holds for `id`'s most recent `Planned` record.
///
/// Read off the journal rather than off a receipt because the point of the defect is what a *future*
/// T-11 would sign: the record is where the witness waits between the plan and the commit.
fn journalled(
    engine: &Engine<InjectedEvidence>,
    id: &TransformationId,
) -> Option<Option<UndoDisposition>> {
    engine
        .journal()
        .records()
        .iter()
        .rev()
        .find_map(|record| match record {
            EngineJournalRecord::Planned {
                transformation,
                undo_witness,
                ..
            } if transformation == id => Some(undo_witness.clone()),
            _ => None,
        })
}

/// Order 1: the journal holds `Attested`, and the second call offers `Unobservable`.
///
/// This is the direction that used to under-claim in the other direction's clothing: the receipt
/// would have said "checked, then restored" while the last caller had nothing to check with.
#[test]
fn a_replan_that_offers_unobservable_over_a_journalled_attestation_is_refused() {
    let (mut engine, t_o) = committed("dr4647_attested_then_unobservable");

    let attested = engine.attested_postcondition(&t_o);
    assert!(
        matches!(attested, UndoWitness::Attested(_)),
        "the fixture's premise: T_o's own receipt carries a postcondition"
    );
    let (_, t_u) = engine
        .undo(&t_o, &attested, 471, LATER)
        .expect("first undo");
    let records_before = engine.journal().len();
    let journalled_before = journalled(&engine, &t_u);

    let second = engine.undo(
        &t_o,
        &UndoWitness::Unobservable(Unobservable::NoPostcondition),
        472,
        LATER,
    );
    println!(
        "DR4647_ORDER1 second={:?} state={:?} records={}→{} journalled={:?}",
        second
            .as_ref()
            .map(|_| "Ok")
            .map_err(gx_engine::Error::kind),
        engine.state(&t_u),
        records_before,
        engine.journal().len(),
        journalled(&engine, &t_u)
    );

    let refusal = second.expect_err(
        "DR-46-47: a re-plan whose witness disagrees with the journalled one is refused",
    );
    assert_eq!(refusal.kind(), "InvalidState");
    assert_eq!(
        engine.journal().len(),
        records_before,
        "the refusal leaves no record, 43 §5.2's posture"
    );
    assert_eq!(
        journalled(&engine, &t_u),
        journalled_before,
        "and it does not overwrite what the first plan attested"
    );
    assert_eq!(
        engine.state(&t_u),
        Some(Lifecycle::Candidate),
        "the seat the first call took is not moved by a refused second one"
    );
}

/// Order 2: the journal holds `Unobservable`, and the second call offers `Attested`.
///
/// The reverse of the arm above and the one the old code called "fail-closed": the receipt would
/// have said "fired without checking" when the last caller *had* checked. Under-claiming is safer
/// than over-claiming and is still not what happened.
#[test]
fn a_replan_that_offers_an_attestation_over_a_journalled_unobservable_is_refused() {
    let (mut engine, t_o) = committed("dr4647_unobservable_then_attested");

    let (_, t_u) = engine
        .undo(
            &t_o,
            &UndoWitness::Unobservable(Unobservable::NoPostcondition),
            473,
            LATER,
        )
        .expect("first undo");
    let records_before = engine.journal().len();
    let journalled_before = journalled(&engine, &t_u);
    assert_eq!(
        journalled_before,
        Some(Some(UndoDisposition::Unobservable {
            reason: Unobservable::NoPostcondition.reason().to_string(),
        })),
        "the fixture's premise: the first call's disposition is what the journal holds"
    );

    let attested = engine.attested_postcondition(&t_o);
    let second = engine.undo(&t_o, &attested, 474, LATER);
    println!(
        "DR4647_ORDER2 second={:?} state={:?} records={}→{} journalled={:?}",
        second
            .as_ref()
            .map(|_| "Ok")
            .map_err(gx_engine::Error::kind),
        engine.state(&t_u),
        records_before,
        engine.journal().len(),
        journalled(&engine, &t_u)
    );

    let refusal = second.expect_err(
        "DR-46-47: the disagreement is refused in this direction too, not only the other",
    );
    assert_eq!(refusal.kind(), "InvalidState");
    assert_eq!(engine.journal().len(), records_before);
    assert_eq!(journalled(&engine, &t_u), journalled_before);
}

/// 🔴 The control: an **agreeing** second call is still answered, with its own id and no second
/// record.
///
/// Without this arm the two tests above would pass just as well against an implementation that had
/// broken 43 T-2's idempotency outright, which is the failure mode that looks like a repair.
#[test]
fn a_replan_that_offers_the_same_witness_is_still_idempotent() {
    let (mut engine, t_o) = committed("dr4647_agreeing_replan");

    let attested = engine.attested_postcondition(&t_o);
    let (intent_one, t_u) = engine
        .undo(&t_o, &attested, 475, LATER)
        .expect("first undo");
    let records_before = engine.journal().len();

    let attested_again = engine.attested_postcondition(&t_o);
    let (intent_two, t_u_again) = engine
        .undo(&t_o, &attested_again, 476, LATER)
        .expect("43 T-2: an agreeing re-plan is answered, not refused");
    println!(
        "DR4647_CONTROL same_id={} same_intent={} records={}→{}",
        u8::from(t_u == t_u_again),
        u8::from(intent_one == intent_two),
        records_before,
        engine.journal().len()
    );

    assert_eq!(t_u, t_u_again, "the same T_o mints the same T_u (H-05)");
    assert_eq!(intent_one, intent_two);
    assert_eq!(
        engine.journal().len(),
        records_before,
        "43 T-2's idempotency column: no second record"
    );
    assert_eq!(engine.state(&t_u), Some(Lifecycle::Candidate));
}
