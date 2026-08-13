//! AC-038 (FR-038) — a failed `apply`, the best-effort rollback, and where the outcome is written.
//!
//! 34 AC-038 逐語: 「Given: inverse escrow済み（T-10b実行済み）のCommitting状態Tで`adapter.apply`が
//! 意図的に失敗するモックadapterを使用。When: commitパイプラインを実行。Then: 自動巻き戻し試行が発生
//! し、結果は`Aborted(ApplyFailed)`として記録され、journal/Receiptに巻き戻し試行の有無（成功/失敗）が
//! 記録される。」
//!
//! 43 T-10c: 「部分適用がありescrow済みinverseがあれば自動巻き戻しを試行（ベストエフォート、結果に
//! 関わらず次へ）；journal: `Aborted{id, ApplyFailed}`」.
//!
//! # 🔴 The last clause had no seat, and this is where it went (**M5H4-2**)
//!
//! 「journal/Receiptに…記録される」 names two places and neither could take it:
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

/// A canonicalized transformation whose adapter will refuse the applications in `script`.
fn canonicalised(name: &str, script: &[bool]) -> Fixture {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.refusing(script)), "commit-adapter-1");

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
    // The forward application refuses; the rollback's does not.
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
        "43 T-10c: 「結果に関わらず次へ」 -- a successful rollback does not rescue the commit"
    );
    assert_eq!(
        totals[APPLY], 2,
        "two walks down 則 2's one road: the forward delta, and the escrowed inverse"
    );
    assert_eq!(
        e.rollback(&id),
        Some(Rollback::Succeeded),
        "AC-038: 「巻き戻し試行の有無（成功/失敗）」"
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
/// further along the section than req/78 §3.2 Λ4 描いた place.
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
        "the two deltas differ, so 「the same record twice」 would be visible"
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
/// 42 §5's exception to ASM-9 exists for this moment: 「digest-onlyでは実際のundoが実行不能なため」.
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
        "42 §3.10 / ASM-14: a `CommitReceipt` is issued 「commit成功時のみ」"
    );
}

/// 🔴 `Rollback::NotAttempted` is unreachable in v0.1, and here is the measurement.
///
/// **E-M3-4** makes 「no inverse」 the one condition that produces an `Escalate` in v0.1, so a
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
