//! AC-034 (FR-034) — the commit-time CAS, and the `apply` that is never reached.
//!
//! 34 AC-034 逐語: 「Given: Canonicalized状態のT（対象`/tmp/target.txt`、Fingerprint₀記録済み）。
//! When: テストハーネスが`gx commit <T.id>`呼び出し**直前**に別プロセスで対象ファイルへ書き込みを行い
//! `Fingerprint₁≠Fingerprint₀`となる状況を注入したうえでcommitを実行。Then:
//! `Aborted(PreconditionChanged)`が返り、モックadapterの`apply`呼び出し回数=0（一切呼ばれない）。」
//!
//! 32 FR-034: 「commit直前に `adapter.precondition(now)` でFingerprint₁を取得し、Fingerprint₀と不一致
//! なら `Aborted(PreconditionChanged)` を返さなければならない（MUST）」. 43 INV-S7 states the same
//! thing as a safety invariant: 「`Fingerprint₁≠Fingerprint₀`のとき、いかなる場合も`adapter.apply`は
//! 呼ばれない（CAS優先）」.
//!
//! # 🔴 What was injected, said plainly
//!
//! The criterion says 「別プロセスで対象ファイルへ書き込み」. This crate ships no adapter and takes
//! none as a dev-dependency (N-13, `ENGINE_ADAPTER_DECLARATIONS=0`), so there is no file and no
//! process to write to it. The substrate here is [`support::CommitAdapter`]'s in-memory world, and
//! the injection is a **separate thread** holding the same handle, joined before `commit` is called.
//!
//! What the criterion is measuring survives the substitution exactly: the CAS sees a state change
//! the engine did not make and did not observe. What does **not** survive is the process boundary —
//! a real `gx commit` against a real file with a real concurrent writer is 51 §8.1's E2E and hand
//! 5's. The gap is recorded here rather than in a report only, because a reader of this file should
//! not have to be told twice.
//!
//! # The two halves, and why the second one needs a counter
//!
//! 「`Aborted(PreconditionChanged)`が返り」 is a value, and any implementation that returns it looks
//! right. 「apply呼び出し回数=0」 is what says the abort happened **before** the world moved rather
//! than after, and only a counting adapter can answer it: an engine that applied and then noticed
//! would return the same value with the same journal record and a substrate that had changed.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, Timestamp};
use gx_engine::{Engine, EngineJournalRecord, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// An engine, the transformation under test, the call counters, and the world behind the substrate.
type Fixture = (
    Engine<InjectedEvidence>,
    gx_core::TransformationId,
    Arc<support::Counts>,
    Arc<std::sync::Mutex<Vec<u8>>>,
);

/// A canonicalized transformation over a mutable world, with the world handle and the counters.
fn canonicalised(name: &str) -> Fixture {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(engine.state(&id), Some(Lifecycle::Canonicalized));
    (engine, id, counts, world)
}

/// 🔴 The criterion: a concurrent writer between `canonicalize` and `commit` aborts the commit, and
/// `apply` is not called.
#[test]
fn ac_034_a_concurrent_mutation_aborts_the_commit_without_applying() {
    let (mut engine, id, counts, world) = canonicalised("ac034_cas");
    let before = counts.totals();

    // 「commit呼び出し直前に別プロセスで対象ファイルへ書き込み」 -- a writer the engine does not
    // know about, running on its own thread and finished before the call.
    let handle = Arc::clone(&world);
    std::thread::spawn(move || {
        let mut world = handle.lock().expect("the world is not poisoned");
        world.clear();
        world.extend_from_slice(b"somebody else was here");
    })
    .join()
    .expect("the injected writer finishes");

    let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");
    let after = counts.totals();

    println!(
        "CAS_STATE={state:?} APPLY_CALLS_BEFORE={} APPLY_CALLS_AFTER={} COUNTS={after:?}",
        before[4], after[4]
    );
    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::PreconditionChanged),
        "43 T-10a: 「`Fingerprint₁` ≠ `Fingerprint₀`」 is PreconditionChanged"
    );
    assert_eq!(
        after[4], 0,
        "INV-S7: 「いかなる場合も`adapter.apply`は呼ばれない」 -- the counter says it was"
    );
    assert_eq!(
        engine.ledger().log().len(),
        0,
        "INV-S4: an Aborted transformation does not appear in the ledger"
    );
    assert!(
        engine.receipt(&id).is_none(),
        "ASM-14 issues a CommitReceipt for a commit, and this one did not happen"
    );
}

/// The journal says the same thing, in the order 43 §7 requires.
///
/// `CommittingStarted` is on the device **before** the CAS runs, which is what makes the abort a
/// recoverable state rather than a silence: 43 §7-3 reads a `CommittingStarted` with no terminal
/// record after it as 「the crash was inside the critical section」, and a T-10a abort that had
/// written nothing first would be indistinguishable from a commit that never started.
#[test]
fn ac_034_the_journal_records_the_section_it_opened_and_then_the_abort() {
    let (mut engine, id, _counts, world) = canonicalised("ac034_journal");
    world.lock().expect("the world").extend_from_slice(b"!");
    engine.commit(&id, AT, &signing_key()).expect("commit runs");

    let kinds: Vec<&str> = engine
        .journal()
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    println!("JOURNAL_AFTER_CAS_ABORT={kinds:?}");

    let committing = kinds
        .iter()
        .position(|k| *k == "CommittingStarted")
        .expect("T-9 ran");
    let aborted = kinds
        .iter()
        .position(|k| *k == "Aborted")
        .expect("T-10a ran");
    assert!(committing < aborted, "43 §7: the section is opened first");
    assert!(
        !kinds.contains(&"ApplyStarted"),
        "E-M5-1's record is written before an apply, and no apply was made"
    );
    assert!(
        !kinds.contains(&"InverseEscrowed"),
        "43 T-10b runs after the CAS passes, and it did not"
    );
    assert!(!kinds.contains(&"Committed"), "nothing committed");

    let reason = engine
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
    assert_eq!(reason.0, AbortReason::PreconditionChanged);
    assert_eq!(
        reason.1, None,
        "no rollback question arises when nothing was applied (see `Rollback`)"
    );
}

/// The control: an unchanged world commits, so the abort above is the injection and not the fixture.
///
/// §30's rule — an absence measured without a presence beside it is a measurement of nothing. If
/// `commit` aborted for every input, the probe above would pass while measuring the engine's
/// inability to commit at all.
#[test]
fn ac_034_an_untouched_world_commits_and_applies_once() {
    let (mut engine, id, counts, _world) = canonicalised("ac034_control");
    let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");
    let after = counts.totals();

    println!("CONTROL_STATE={state:?} COUNTS={after:?}");
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(after[4], 1, "43 T-11: the world moved exactly once");
    assert_eq!(engine.ledger().log().len(), 1);
}

/// 🔴 The CAS compares against `Fingerprint₀` and not against a value it just computed.
///
/// The single defect AC-031's 「後段commit時に再取得できる」 exists to prevent, measured from the
/// other side: after a commit, `Fingerprint₀` is still the fingerprint T-2 recorded, and the world
/// has moved past it. An engine that had refreshed the stored fingerprint before comparing would
/// pass AC-034's control case and fail this one — and would never abort, because it would always be
/// comparing the substrate with itself.
#[test]
fn ac_034_the_stored_fingerprint_is_the_one_t_2_recorded() {
    let (mut engine, id, _counts, world) = canonicalised("ac034_fp0");
    let at_plan = engine
        .precondition_fingerprint(&id)
        .expect("T-2 recorded it")
        .clone();

    engine
        .commit(&id, AT, &signing_key())
        .expect("the commit runs");
    let after = engine
        .precondition_fingerprint(&id)
        .expect("still there after the commit");

    let world_now = world.lock().expect("the world").clone();
    println!(
        "FP0_SCOPE={} WORLD_AFTER={:?} SAME_FP0={}",
        after.scope(),
        String::from_utf8_lossy(&world_now),
        at_plan.cas_eq(after).expect("same scope")
    );
    assert!(
        at_plan.cas_eq(after).expect("the same scope and substrate"),
        "`Fingerprint₀` is fixed at T-2 and the commit does not move it"
    );
    assert_eq!(
        world_now, b"after",
        "the control commit did move the world, so the fingerprint above is stale on purpose"
    );
}
