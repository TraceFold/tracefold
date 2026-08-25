// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-038 (FR-038) — a failed `apply`, the best-effort rollback, and where the outcome is written.
//!
//! 34 AC-038, verbatim: "Given: a Committing-state T with the inverse escrowed (T-10b already run),
//! using a mock adapter whose `adapter.apply` fails deliberately. When: the commit pipeline runs.
//! Then: an automatic rollback attempt occurs, and the result is recorded as `Aborted(ApplyFailed)`,
//! and journal/Receipt records whether a rollback attempt occurred (success/failure)." (sem:
//! SEM-gx-engine-471)
//!
//! 43 T-10c: "if there was a partial apply and an escrowed inverse exists, attempt an automatic
//! rollback (best-effort; move on regardless of the outcome); journal: `Aborted{id, ApplyFailed}`"
//! (sem: SEM-gx-engine-471).
//!
//! # 🔴 The last clause had no seat, and this is where it went (**M5H4-2**)
//!
//! "...is recorded in journal/Receipt" (sem: SEM-gx-engine-472) names two places and neither
//! could take it:
//!
//! * **Receipt** — ASM-14 defines two kinds, `VerdictReceipt` and `CommitReceipt`. An aborted
//!   transformation gets neither, so there is no receipt for a rollback outcome to be written on.
//!   `ac_038_an_aborted_commit_issues_no_receipt` measures that rather than assuming it.
//! * **journal** — 42 §3.13's `Aborted` row is `{transformation, reason, at}` and 43 T-10c's cell
//!   writes the same three.
//!
//! So the record 43 T-10c itself names gains a field, in the shape **M5H2-1 / E-M5-7** used for
//! `Verdict`. The divergence from 42 §3.13 is asserted in `tests/journal_vocabulary.rs` and raised
//! as a ticket; what is measured here is that the value is true.
//!
//! # Three outcomes, and one of them is unreachable on purpose
//!
//! `Rollback::Succeeded` and `Rollback::Failed` are both constructed below.
//! `Rollback::NotAttempted` is **not**, because v0.1 cannot reach it: E-M3-4 makes an inverse that
//! cannot be constructed an `Escalate` at T-3, so nothing without an escrowed inverse is ever
//! `Canonicalized`. `ac_038_a_transformation_without_an_inverse_never_reaches_the_critical_section`
//! measures the unreachability instead of leaving it as a claim.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, Timestamp};
use gx_engine::{Engine, EngineJournalRecord, InjectedEvidence, Lifecycle, Rollback};
use support::{gate, intent, scratch, signing_key, CommitAdapter, Counts, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const APPLY: usize = 4;

/// An engine, the transformation under test, the call counters, and the world behind the substrate.
type Fixture = (
    Engine<InjectedEvidence>,
    gx_core::TransformationId,
    Arc<Counts>,
    Arc<std::sync::Mutex<Vec<u8>>>,
);

/// 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — a canonicalized transformation whose
/// adapter **performs** the applications in `script` and *then* answers an error.
///
/// This is the fixture the three criteria below now use, and the change is not cosmetic. Until R30
/// they used [`canonicalised_refusing`], whose forward apply returns before it touches the world —
/// and the engine now reads the object the instant an apply fails and declines to send a
/// compensation for an effect that does not exist. On a forward apply that changed nothing there is
/// nothing to take back, so `NotAttempted(WorldNeverMoved)` is the answer and no inverse is sent.
///
/// AC-038 is about what happens when the escrowed inverse **is** applied, so the fixture has to
/// produce a world that actually moved. `the_compensation_is_not_sent_for_an_apply_that_moved_nothing`
/// below holds the other half, so both roads are measured rather than one being replaced.
fn canonicalised(name: &str, script: &[bool]) -> Fixture {
    canonicalised_with(name, &[], script)
}

/// A canonicalized transformation whose adapter will refuse the applications in `script` **without
/// touching the world**.
fn canonicalised_refusing(name: &str, script: &[bool]) -> Fixture {
    canonicalised_with(name, script, &[])
}

/// The body of both: `refuse` returns before the world is written, `after` returns after it.
fn canonicalised_with(name: &str, refuse: &[bool], after: &[bool]) -> Fixture {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(
        Arc::new(adapter.refusing(refuse).failing_after_the_effect(after)),
        "commit-adapter-1",
    );

    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    (engine, id, counts, world)
}

/// 🔴 The criterion: the apply fails, the escrowed inverse is applied, and both facts are recorded.
#[test]
fn ac_038_a_failed_apply_rolls_back_and_the_outcome_is_journalled() {
    // The forward application lands and then errors; the rollback's is clean.
    let (mut e, id, counts, world) = canonicalised("ac038_succeeded", &[true]);
    let state = e.commit(&id, AT, &signing_key()).expect("commit runs");
    let totals = counts.totals();

    println!(
        "STATE={state:?} APPLY_CALLS={} ROLLBACK={:?} WORLD={:?}",
        totals[APPLY],
        e.rollback(&id),
        String::from_utf8_lossy(&world.lock().expect("the world"))
    );
    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::ApplyFailed),
        "43 T-10c: \"move on regardless of the outcome\" -- a successful rollback does not rescue \
         the commit (sem: SEM-gx-engine-473)"
    );
    assert_eq!(
        totals[APPLY], 2,
        "two walks down Rule 2's one road (sem: SEM-gx-engine-474): the forward delta, and the \
         escrowed inverse"
    );
    assert_eq!(
        e.rollback(&id),
        Some(Rollback::Succeeded),
        "AC-038: \"whether a rollback attempt occurred (success/failure)\" (sem: SEM-gx-engine-475)"
    );

    let (reason, rollback) = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Aborted {
                reason, rollback, ..
            } => Some((*reason, *rollback)),
            _ => None,
        })
        .expect("the abort is journalled");
    assert_eq!(reason, AbortReason::ApplyFailed);
    assert_eq!(
        rollback,
        Some(Rollback::Succeeded),
        "the journal is where AC-038's outcome lives (M5H4-2)"
    );
    assert_eq!(e.ledger().log().len(), 0, "INV-S4: no leaf for an abort");
}

/// 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the other half of the criterion above,
/// and the road the twenty-ninth audit's worst finding runs down: when the forward apply fails
/// **without moving the world**, the escrowed inverse is *not* sent.
///
/// # Why this is a criterion and not a regression
///
/// 43 T-10c is "roll back on a best-effort basis", and until this window that was read as *send the
/// inverse whatever happened*. The audit measured what the two words leave out. The escrowed
/// inverses every shipped adapter mints are **absolute**, so they restore from any world — and one
/// of the worlds they restore from is a world a third party legitimately created. With the shipped
/// git adapter, on a real branch: a colleague's commit `d2d09b5` was taken off the branch by a
/// compensation for a change that had never landed, and `Succeeded` was recorded over it.
///
/// A transformation that did nothing has nothing to take back. Sending an absolute inverse anyway
/// cannot restore anything (the object is already at `fp0`) and can only overwrite whatever is
/// there now, so declining is not a weaker best effort — there was no effort available to make.
///
/// This arm is what keeps that true: it counts the adapter's **own** apply calls rather than asking
/// the engine whether the engine behaved, and it asserts the announcement count too, because a
/// compensation that was announced and not sent would leave recovery re-applying a delta nobody
/// applied.
#[test]
fn the_compensation_is_not_sent_for_an_apply_that_moved_nothing() {
    // The forward application refuses **before touching the world**, which is a permission denial,
    // a policy refusal at the far end, or any call the substrate declined outright.
    let (mut e, id, counts, world) = canonicalised_refusing("ac038_never_moved", &[true]);
    let state = e.commit(&id, AT, &signing_key()).expect("commit runs");
    let announced = e
        .journal()
        .records()
        .iter()
        .filter(|r| matches!(r, EngineJournalRecord::ApplyStarted { .. }))
        .count();

    println!(
        "R30_AC038 STATE={state:?} APPLY_CALLS={} ROLLBACK={:?} CAUSE={:?} ANNOUNCED={announced} \
         WORLD={:?}",
        counts.totals()[APPLY],
        e.rollback(&id),
        e.rollback_not_attempted_because(&id).map(|c| c.kind()),
        String::from_utf8_lossy(&world.lock().expect("the world"))
    );

    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::ApplyFailed),
        "the abort itself is unchanged: what the compensation did does not rescue the commit"
    );
    assert_eq!(
        counts.totals()[APPLY],
        1,
        "🔴 the finding, taken from the **adapter's own** counter rather than from gx's account of \
         itself: the forward apply is the only application that happened. Before R30 this was 2, \
         and the second one was an absolute inverse written over whatever the object held"
    );
    assert_eq!(
        e.rollback(&id),
        Some(Rollback::NotAttempted),
        "and the word says so: no compensating inverse was sent"
    );
    assert_eq!(
        e.rollback_not_attempted_because(&id).map(|c| c.kind()),
        Some("WorldNeverMoved"),
        "🔴 with the cause beside the value (`req/324` §5(d)) -- `NotAttempted` is reached by five \
         other roads and a reader given the value alone cannot tell which"
    );
    assert_eq!(
        announced, 1,
        "🔴 and the compensation was not **announced** either: an `ApplyStarted` for a delta that \
         was never sent is exactly the record E-M5-1 exists to prevent, because recovery would \
         re-apply a delta nobody applied"
    );
    assert_eq!(
        String::from_utf8_lossy(&world.lock().expect("the world")),
        "before",
        "the premise this arm rests on: the refused apply left the world where it found it"
    );
}

/// The other outcome: the rollback is attempted and the adapter refuses it too.
///
/// 43 T-10c is best-effort, so this is not a second failure mode of the engine — it is the same
/// transition with a different fact recorded. The distinction is the whole reason the field is not a
/// `bool`: an operator reading `ApplyFailed` needs to know whether the world was left changed.
#[test]
fn ac_038_a_rollback_that_fails_is_recorded_as_a_failure() {
    let (mut e, id, counts, _world) = canonicalised("ac038_failed", &[true, true]);
    let state = e.commit(&id, AT, &signing_key()).expect("commit runs");

    println!(
        "STATE={state:?} APPLY_CALLS={} ROLLBACK={:?}",
        counts.totals()[APPLY],
        e.rollback(&id)
    );
    assert_eq!(state, Lifecycle::Aborted(AbortReason::ApplyFailed));
    assert_eq!(counts.totals()[APPLY], 2, "the attempt was still made");
    assert_eq!(e.rollback(&id), Some(Rollback::Failed));
}

/// 🔴 **E-M5-1**: the rollback is announced before it is attempted.
///
/// Two `ApplyStarted` records, naming two different deltas: the forward one and the escrowed
/// inverse. A rollback that ran without a record would leave a crash inside it indistinguishable
/// from a crash before it — which is the exact hole `ApplyStarted` was added to close, one step
/// further along the section than the place req/78 §3.2 Λ4 drew (sem: SEM-gx-engine-476).
#[test]
fn ac_038_both_applications_are_announced_and_they_name_different_deltas() {
    let (mut e, id, _counts, _world) = canonicalised("ac038_records", &[true]);
    e.commit(&id, AT, &signing_key()).expect("commit runs");

    let announced: Vec<gx_core::Cid> = e
        .journal()
        .records()
        .iter()
        .filter_map(|r| match r {
            EngineJournalRecord::ApplyStarted { delta_cid, .. } => Some(*delta_cid),
            _ => None,
        })
        .collect();
    let escrowed = e.escrowed_inverse(&id).expect("T-10b escrowed an inverse");
    let forward = e
        .planned_delta(&id)
        .expect("T-2 planned one")
        .reference()
        .cid;

    println!(
        "APPLY_STARTED_RECORDS={} FORWARD={forward:?} ESCROWED={escrowed:?}",
        announced.len()
    );
    assert_eq!(announced.len(), 2, "one record per application");
    assert_eq!(
        announced[0], forward,
        "the forward delta is announced first"
    );
    assert_eq!(announced[1], escrowed, "then the inverse that undoes it");
    assert_ne!(
        forward, escrowed,
        "the two deltas differ, so \"the same record twice\" would be visible (sem: \
         SEM-gx-engine-477)"
    );

    let kinds: Vec<&str> = e
        .journal()
        .records()
        .iter()
        .map(EngineJournalRecord::kind)
        .collect();
    println!("JOURNAL_AFTER_ROLLBACK={kinds:?}");
    let escrow_at = kinds
        .iter()
        .position(|k| *k == "InverseEscrowed")
        .expect("T-10b ran");
    let first_apply = kinds
        .iter()
        .position(|k| *k == "ApplyStarted")
        .expect("the forward apply was announced");
    assert!(
        escrow_at < first_apply,
        "43 T-10b escrows before the apply, so the rollback has something to use"
    );
}

/// The escrowed inverse's **body** is in the blob store, not only its name.
///
/// 42 §5's exception to ASM-9 exists for this moment: "because a digest-only record cannot actually
/// execute an undo" (sem: SEM-gx-engine-478).
/// The rollback above applied a delta, and this is where that delta came from.
#[test]
fn ac_038_the_escrowed_inverse_body_is_retrievable() {
    let (mut e, id, _counts, _world) = canonicalised("ac038_escrow", &[true]);
    e.commit(&id, AT, &signing_key()).expect("commit runs");

    let cid = e.escrowed_inverse(&id).expect("T-10b escrowed an inverse");
    let body = e.blobs().get(&cid).expect("42 §5: the payload is kept");
    println!(
        "ESCROWED_CID={cid:?} BODY={:?} BLOBS={}",
        String::from_utf8_lossy(body.payload()),
        e.blobs().len()
    );
    assert_eq!(
        body.payload(),
        b"before",
        "the inverse restores the world the snapshot was taken over"
    );
    assert_eq!(
        body.reference().cid,
        cid,
        "content addressing: the body hashes to the name it was filed under"
    );
}

/// ASM-14 issues no receipt for an abort, which is why the journal had to take AC-038's outcome.
#[test]
fn ac_038_an_aborted_commit_issues_no_receipt() {
    let (mut e, id, _counts, _world) = canonicalised("ac038_receipt", &[true]);
    e.commit(&id, AT, &signing_key()).expect("commit runs");
    println!(
        "RECEIPT_AFTER_ABORT={:?} LEDGER_LEAVES={}",
        e.receipt(&id).is_some(),
        e.ledger().log().len()
    );
    assert!(
        e.receipt(&id).is_none(),
        "42 §3.10 / ASM-14: a `CommitReceipt` is issued \"only on a successful commit\" (sem: \
         SEM-gx-engine-479)"
    );
}

/// 🔴 `Rollback::NotAttempted` is unreachable in v0.1, and here is the measurement.
///
/// **E-M3-4** makes "no inverse" (sem: SEM-gx-engine-480) the one condition that produces an
/// `Escalate` in v0.1, so a
/// transformation whose `invert` answers `None` stops at `Escalated` and never reaches T-9 at all.
/// Naming the value and never writing it is the shape 42 §3.12's `InverseStatus::Expired` already
/// has; measuring the reason is what keeps it from being an excuse.
#[test]
fn ac_038_a_transformation_without_an_inverse_never_reaches_the_critical_section() {
    let dir = scratch("ac038_no_inverse");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    e.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/target.txt", "after");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");
    let state = e.verify(&id, AT, &signing_key(), None).expect("verify");

    let refused = e.commit(&id, AT, &signing_key());
    println!(
        "STATE_WITHOUT_INVERSE={state:?} COMMIT={:?} APPLY={}",
        refused.as_ref().err().map(gx_engine::Error::kind),
        counts.totals()[APPLY]
    );
    assert_eq!(
        state,
        Lifecycle::Escalated,
        "E-M3-4: `invert_available = false` is v0.1's one road to Escalate"
    );
    assert_eq!(
        refused
            .expect_err("43 T-9 has one from-state and this is not it")
            .kind(),
        "InvalidState"
    );
    assert_eq!(counts.totals()[APPLY], 0);
}
