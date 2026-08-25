// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-073** — owner cancel (FR-059, DR-11, 43 T-7).
//!
//! 34 AC-073 verbatim (sem: SEM-gx-engine-582):
//!
//! > Given: a Transformation T in any state before reaching `Committing` (Draft/Candidate/Verifying/
//! > Admitted/Canonicalized/Escalated). When: `gx cancel <T.id>` (or the API ...) is run. Then: T
//! > transitions to `Aborted(OwnerCancelled)`. When: the same command is run against a T already at
//! > `Committing` or beyond (including Committed). Then: it is refused as an invalid operation and
//! > the existing state is unchanged.
//!
//! # 🔴 Four of the six from-states are reachable; `Draft` has no id and `Verifying` is transient
//!
//! `gx cancel <T.id>` takes a `TransformationId`, and 43 T-1 says a draft has none
//! ("`TransformationId` is not yet fixed", **E-M5-3**; sem: SEM-gx-engine-583). 42 §3.13's
//! `Aborted` record is keyed on one, and **M5-17, adopted (b)** keeps the draft phase in the
//! journal with no row to move. So there is nothing
//! this engine could write about cancelling a draft and nothing it could change — the case is
//! **unrepresentable in v0.1**, raised as **M5H6-1**, and measured below rather than skipped:
//! [`ac_073_a_draft_has_no_id_to_cancel`] shows the shape of the gap instead of leaving a blank.
//!
//! The cost is bounded and worth writing down: a draft holds no `PlannedDelta`, has caused no
//! adapter call, appears in no ledger, and 43 T-6 does not reach it either — so an uncancelled
//! draft is one `DraftCreated` record and an entry in a set.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, Timestamp};
use gx_engine::{Engine, HumanRuling, InjectedEvidence, Lifecycle};
use support::{gate, intent, ruler, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const CANCELLED_AT: Timestamp = Timestamp(1_754_000_030_000_000_000);

/// The from-states of 43 T-7 a caller can leave a transformation resting in.
///
/// Four of 43 T-7's six. `Draft` has no id (see the module note), and 🔴 **`Verifying` is
/// transient**: [`Engine::verify`] runs T-3 and one of T-4a..T-4e in one call, so no caller can
/// hold a transformation there and no `cancel` can arrive while it is. That is 43's own shape --
/// T-3's side effect is "starting the evidence collector" (sem: SEM-gx-engine-584) and the verdict follows in the same row -- rather
/// than a limitation of this engine, and [`ac_073_verifying_is_never_a_resting_state`] measures it
/// instead of leaving a gap in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Candidate,
    Admitted,
    Canonicalized,
    Escalated,
}

impl Stop {
    const ALL: [Stop; 4] = [
        Stop::Candidate,
        Stop::Admitted,
        Stop::Canonicalized,
        Stop::Escalated,
    ];
}

/// Drive a transformation to `stop`, cancel it, and answer with what happened.
fn cancel_at(stop: Stop) -> (Lifecycle, Lifecycle, usize, u64, usize) {
    let dir = scratch(&format!("ac073_{stop:?}"));
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    // `Escalated` needs E-M3-4's condition; the other four do not care.
    let adapter = if stop == Stop::Escalated {
        adapter.without_inverse()
    } else {
        adapter
    };
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/cancel.txt", "after");
    engine.submit(&i, 50, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let reached = match stop {
        Stop::Candidate => Lifecycle::Candidate,
        Stop::Admitted => engine.verify(&id, AT, &signing_key(), None).expect("T-4a"),
        Stop::Canonicalized => {
            engine.verify(&id, AT, &signing_key(), None).expect("T-4a");
            engine.canonicalize(&id, AT, None).expect("T-8")
        }
        Stop::Escalated => engine.verify(&id, AT, &signing_key(), None).expect("T-4c"),
    };
    let after = engine.cancel(&id, CANCELLED_AT).expect("T-7");
    (
        reached,
        after,
        counts.totals()[4],
        engine.ledger().log().len(),
        engine.journal().len(),
    )
}

/// 🔴 AC-073's first half: every reachable from-state of 43 T-7 cancels.
#[test]
fn ac_073_every_state_before_committing_can_be_cancelled() {
    for stop in Stop::ALL {
        let (reached, after, applies, leaves, records) = cancel_at(stop);
        println!(
            "AC073 stop={stop:?} reached={reached:?} after={after:?} applies={applies} \
             leaves={leaves} records={records}"
        );
        assert_eq!(
            after,
            Lifecycle::Aborted(AbortReason::OwnerCancelled),
            "43 T-7: \"journal: `Aborted{{id, OwnerCancelled}}`\" (sem: SEM-gx-engine-585) from {stop:?}"
        );
        assert_eq!(applies, 0, "T-7 cannot fire from `Committing`");
        assert_eq!(leaves, 0, "INV-S4");
    }
}

/// 🔴 AC-073's second half: from `Committed` the same call is refused and changes nothing.
///
/// > the same command is run against a T already at `Committing` or beyond (including Committed).
/// > Then: it is refused as an invalid operation and the existing state is unchanged. (sem: SEM-gx-engine-586)
///
/// "the existing state is unchanged" (sem: SEM-gx-engine-586) is read as three things and all three are measured: the state, the
/// journal length, and the ledger. A refusal that wrote an `Aborted` record and then reported an
/// error would satisfy the first and break the other two.
#[test]
fn ac_073_a_committed_transformation_refuses_the_cancel_and_is_unchanged() {
    let dir = scratch("ac073_committed");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/done.txt", "after");
    engine.submit(&i, 51, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine.verify(&id, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&id, AT, None).expect("T-8");
    let committed = engine.commit(&id, AT, &signing_key()).expect("T-11");

    let records = engine.journal().len();
    let leaves = engine.ledger().log().len();
    let receipt = engine.receipt(&id).expect("T-11 issued one").clone();
    let refused = engine
        .cancel(&id, CANCELLED_AT)
        .expect_err("43 T-7's guard is \"before reaching `Committing`\" (sem: SEM-gx-engine-587)");
    println!(
        "AC073_COMMITTED committed={committed:?} refused={:?} state={:?} \
         records={records}/{} leaves={leaves}/{} receipt_same={} applies={}",
        refused.kind(),
        engine.state(&id),
        engine.journal().len(),
        engine.ledger().log().len(),
        engine.receipt(&id) == Some(&receipt),
        counts.totals()[4]
    );
    assert_eq!(committed, Lifecycle::Committed);
    assert_eq!(
        refused.kind(),
        "InvalidState",
        "\"refused as an invalid operation\" (sem: SEM-gx-engine-588)"
    );
    assert_eq!(
        engine.state(&id),
        Some(Lifecycle::Committed),
        "\"the existing state is unchanged\" (sem: SEM-gx-engine-589)"
    );
    assert_eq!(engine.journal().len(), records, "and nothing was written");
    assert_eq!(engine.ledger().log().len(), leaves);
    assert_eq!(engine.receipt(&id), Some(&receipt));
    assert_eq!(
        &*world.lock().expect("the world is not poisoned"),
        b"after",
        "P-5: a cancel is not an undo, and the substrate is untouched by the refusal"
    );
}

/// 43 T-7's idempotency column: "a duplicate cancel is ignored as a no-op (already Aborted)" (sem: SEM-gx-engine-590).
///
/// Ignored, not refused -- 43 says "ignored" (sem: SEM-gx-engine-590) and the difference is observable: an `Err` would make a
/// retrying client's second call look like a failure. What must not happen is a **second record**,
/// which is what a journal would report as a second event.
#[test]
fn ac_073_a_second_cancel_is_ignored_rather_than_recorded() {
    let dir = scratch("ac073_twice");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/twice.txt", "after");
    engine.submit(&i, 52, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let first = engine.cancel(&id, CANCELLED_AT).expect("T-7");
    let records = engine.journal().len();
    let second = engine
        .cancel(&id, Timestamp(CANCELLED_AT.0 + 1))
        .expect("\"a duplicate cancel is ignored as a no-op\" (sem: SEM-gx-engine-591)");
    println!(
        "AC073_TWICE first={first:?} second={second:?} records={records}/{}",
        engine.journal().len()
    );
    assert_eq!(first, second);
    assert_eq!(first, Lifecycle::Aborted(AbortReason::OwnerCancelled));
    assert_eq!(engine.journal().len(), records, "no second record");
}

/// 🔴 **M5H6-1**: a draft has no id, so `gx cancel <T.id>` has nothing to name.
///
/// The gap, as a measurement rather than as a blank. Three facts are read: the draft exists (so
/// this is not a probe about an absent thing), the only name it has is an `IntentId`, and the only
/// journal record about it is a `DraftCreated` — which is keyed on that `IntentId` (**E-M5-3**)
/// while `Aborted` is keyed on a `TransformationId`.
#[test]
fn ac_073_a_draft_has_no_id_to_cancel() {
    let dir = scratch("ac073_draft");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/draft.txt", "after");
    let intent_id = engine.submit(&i, 53, AT).expect("T-1");
    let ids = engine.transformation_ids();
    let keyed: Vec<&str> = engine
        .journal()
        .records()
        .iter()
        .map(|r| {
            if r.transformation().is_some() {
                "T"
            } else {
                "I"
            }
        })
        .collect();
    println!(
        "AC073_DRAFT drafted={} transformation_ids={} record_keys={keyed:?}",
        engine.is_drafted(&intent_id),
        ids.len()
    );
    assert!(engine.is_drafted(&intent_id), "the draft exists");
    assert!(
        ids.is_empty(),
        "M5-17, adopted (b) (sem: SEM-gx-engine-592): \"the Draft phase is held only by the journal; the state table starts at Candidate\""
    );
    assert_eq!(
        keyed,
        vec!["I"],
        "E-M5-3: the one record about a draft carries an `IntentId` and no `TransformationId`, \
         which is why 43 T-7's `Aborted{{id, OwnerCancelled}}` cannot be written about it"
    );
}

/// 🔴 `Verifying` is a state 43 §1 names and no caller can rest in.
///
/// 43 T-7's from-set includes it and this engine can never be asked to cancel one, because T-3 and
/// T-4a..T-4e are one call: the state exists between two statements of [`Engine::verify`] and the
/// journal records its beginning (`VerifyStarted`) and its end (`Verdict` / `Aborted`) with nothing
/// in between that a caller could interleave with. Measured from three sides -- the state is never
/// observed, the record is written, and 43 §1 lists the state -- so that a hand which later splits
/// `verify` into two entry points meets this probe rather than a blank row in AC-073's table.
///
/// Not raised as a defect: 43's from-set is written for a state machine, and a state machine that
/// passes through a state is entitled to list it. What would be a defect is a table claiming the
/// case was tested.
#[test]
fn ac_073_verifying_is_never_a_resting_state() {
    let dir = scratch("ac073_verifying");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/verifying.txt", "after");
    engine.submit(&i, 56, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let before = engine.state(&id);
    let returned = engine.verify(&id, AT, &signing_key(), None).expect("T-4a");
    let after = engine.state(&id);
    let started = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.kind() == "VerifyStarted")
        .count();
    println!(
        "AC073_VERIFYING before={before:?} returned={returned:?} after={after:?}          verify_started_records={started}"
    );
    assert_eq!(before, Some(Lifecycle::Candidate));
    assert_eq!(returned, Lifecycle::Admitted);
    assert_ne!(after, Some(Lifecycle::Verifying));
    assert_eq!(started, 1, "T-3 happened; it just did not stop there");
    assert!(
        gx_engine::LIFECYCLE_STATES.contains(&"Verifying"),
        "43 §1 lists it, which is why the gap is worth a probe"
    );
}

/// A cancel does not need a ruling, and a ruling does not cancel — the two entry points are separate.
///
/// Written because both take an `Escalated` transformation and both end it, and a reader could
/// reasonably wonder whether one is the other with a flag. They are not: T-5b ends in `Denied`
/// (a verdict, with a signed receipt) and T-7 ends in `Aborted(OwnerCancelled)` (no verdict, no
/// receipt). 43 §1 marks only the second terminal unconditionally.
#[test]
fn ac_073_a_cancel_and_a_rejection_are_different_endings() {
    let dir = scratch("ac073_vs_reject");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let one = intent("/tmp/a.txt", "after");
    engine.submit(&one, 54, AT).expect("submit");
    let a = engine.plan(&one, AT).expect("plan");
    engine.verify(&a, AT, &signing_key(), None).expect("T-4c");
    let cancelled = engine.cancel(&a, CANCELLED_AT).expect("T-7");

    let two = intent("/tmp/b.txt", "after");
    engine.submit(&two, 55, AT).expect("submit");
    let b = engine.plan(&two, AT).expect("plan");
    engine.verify(&b, AT, &signing_key(), None).expect("T-4c");
    let rejected = engine
        .escalation(
            &b,
            &HumanRuling {
                decision: gx_core::VerdictKind::Deny,
                reason: "not this one".to_string(),
                actor: ruler(4),
            },
            CANCELLED_AT,
            &signing_key(),
        )
        .expect("T-5b");

    println!(
        "AC073_VS cancelled={cancelled:?} receipts_a={} verdict_a={:?} \
         rejected={rejected:?} receipts_b={} verdict_b={:?}",
        engine.verdict_receipts(&a).len(),
        engine.verdict(&a),
        engine.verdict_receipts(&b).len(),
        engine.verdict(&b)
    );
    assert_eq!(cancelled, Lifecycle::Aborted(AbortReason::OwnerCancelled));
    assert_eq!(rejected, Lifecycle::Denied);
    assert_eq!(
        engine.verdict_receipts(&a).len(),
        1,
        "T-4c's receipt only: a cancel is not a verdict and issues none"
    );
    assert_eq!(
        engine.verdict_receipts(&b).len(),
        2,
        "T-4c's and T-5b's: a rejection is a verdict and is signed"
    );
    assert_eq!(engine.verdict(&a), Some(gx_core::VerdictKind::Escalate));
    assert_eq!(engine.verdict(&b), Some(gx_core::VerdictKind::Deny));
}
