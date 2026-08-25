// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-045** — TTL, and the liveness invariants that depend on it (FR-036, INV-L1/L2/L4, 43 T-6).
//!
//! 34 AC-045, verbatim: "Given: a test configuration with `verify_ttl`/`escalation_ttl` set short
//! (e.g. 100ms). When: a Candidate/Verifying/Escalated state is deliberately left alone, with no
//! Gate evaluation or human ruling triggered. Then: once the TTL elapses, it always reaches
//! `Aborted(Expired)` (no indefinite hold occurs). Also confirm that TTL acts on a Transformation
//! waiting via `Conflicts` too, and it does not become indefinite." (sem: SEM-gx-engine-552)
//!
//! # 🔴 No test in this file sleeps, and that is 41 §6 rather than a shortcut
//!
//! "after the TTL elapses" (sem: SEM-gx-engine-553) is `now - since >= ttl` for the `now` a
//! caller injects — 41 §6: "randomness/time are injected at the engine boundary" — so the
//! criterion's 100 ms is a **number in a builder** and the passage of time is a later timestamp. A
//! suite that slept would be measuring the scheduler and would be flaky on a loaded machine (51
//! §13-4). The 100 ms is written down all the same, because the criterion names it and because the
//! arithmetic is what is being checked.
//!
//! # "deliberately ... left alone" (sem: SEM-gx-engine-554) needs two roads, and M5-10
//! adopted (a)+(b) is both
//!
//! A transformation nobody touches again is exactly the one a lazy check cannot see. So the two
//! halves are measured separately: [`ac_045_an_untouched_candidate_is_expired_by_the_reaper`] never
//! calls an entry point, and [`ac_045_a_late_caller_finds_the_deadline_already_passed`] never calls
//! the reaper.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, Timestamp};
use gx_engine::{
    Engine, InjectedEvidence, Lifecycle, DEFAULT_ESCALATION_TTL_NANOS, DEFAULT_VERIFY_TTL_NANOS,
};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// AC-045's "short (e.g. 100ms)" (sem: SEM-gx-engine-555), in the nanoseconds [`Timestamp`]
/// counts.
const MS: i64 = 1_000_000;
const TTL: i64 = 100 * MS;

/// `AT` plus `ms` milliseconds.
fn later(ms: i64) -> Timestamp {
    Timestamp(AT.0 + ms * MS)
}

/// 33 NFR-028's defaults are what the engine runs with unless a deployment says otherwise.
#[test]
fn ac_045_the_defaults_are_asm_12s_twenty_four_and_seventy_two_hours() {
    println!(
        "TTL_DEFAULTS verify={DEFAULT_VERIFY_TTL_NANOS} escalation={DEFAULT_ESCALATION_TTL_NANOS}"
    );
    assert_eq!(DEFAULT_VERIFY_TTL_NANOS, 24 * 60 * 60 * 1_000_000_000);
    assert_eq!(DEFAULT_ESCALATION_TTL_NANOS, 72 * 60 * 60 * 1_000_000_000);
    // A `const` block, because both operands are constants and clippy is right that an ordinary
    // `assert!` over two of them cannot fail at run time. Moving it into `const` makes the claim a
    // **compile-time** one, which is stronger than the assertion it replaces.
    const {
        assert!(
            DEFAULT_ESCALATION_TTL_NANOS > DEFAULT_VERIFY_TTL_NANOS,
            "a person is slower than a gate, and ASM-12 says so"
        );
    }

    // The defaults, read off 33 rather than off memory. The numbers live in two places and this is
    // the one that says they agree.
    let nfr = support::read_repo("req/spec/30-requirements/33-non-functional.md");
    assert!(
        nfr.contains("verify_ttl") && nfr.contains("escalation_ttl"),
        "33 NFR-028 no longer names the two deadlines"
    );
}

/// 🔴 AC-045, first half (**M5-10, adopted (b)**; sem: SEM-gx-engine-556): a `Candidate` nobody touches is reaped.
///
/// INV-L1: "any `Candidate`/`Verifying` reaches a terminal state or `Aborted(Expired)` within
/// finite time" (sem: SEM-gx-engine-556).
/// Nothing is called on this transformation between `plan` and `reap`, which is what "deliberately
/// ... left alone" (sem: SEM-gx-engine-556) means.
#[test]
fn ac_045_an_untouched_candidate_is_expired_by_the_reaper() {
    let dir = scratch("ac045_reap");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens")
    .with_ttl(TTL, TTL);
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/idle.txt", "after");
    engine.submit(&i, 1, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let deadline = engine.deadline(&id).expect("a Candidate has one");

    // Before the deadline: nothing happens, and "nothing happened" (sem: SEM-gx-engine-557)
    // is a value.
    let early = engine.reap(later(99)).expect("a sweep that finds nothing");
    let state_before = engine.state(&id);

    let expired = engine.reap(later(150)).expect("the sweep");
    println!(
        "AC045_REAP deadline={deadline:?} early={} state_before={state_before:?} \
         expired={} state_after={:?} applies={} leaves={}",
        early.len(),
        expired.len(),
        engine.state(&id),
        counts.totals()[4],
        engine.ledger().log().len()
    );
    assert_eq!(deadline, Timestamp(AT.0 + TTL), "43 T-6 counts from T-2");
    assert!(early.is_empty(), "the deadline had not passed");
    assert_eq!(state_before, Some(Lifecycle::Candidate));
    assert_eq!(expired, vec![id]);
    assert_eq!(
        engine.state(&id),
        Some(Lifecycle::Aborted(AbortReason::Expired)),
        "43 T-6: \"journal: `Aborted{{id, Expired}}`\" (sem: SEM-gx-engine-558)"
    );
    assert_eq!(
        engine.deadline(&id),
        None,
        "a terminal row has no deadline, which is 43 T-6's idempotency without a second query"
    );

    // 43 T-6: "the reaper fires only once per id" (sem: SEM-gx-engine-559). A second sweep
    // writes nothing.
    let records = engine.journal().len();
    let again = engine.reap(later(500)).expect("the second sweep");
    println!(
        "AC045_REAP_TWICE again={} records_before={records} records_after={}",
        again.len(),
        engine.journal().len()
    );
    assert!(again.is_empty());
    assert_eq!(engine.journal().len(), records, "no second record");
    assert_eq!(counts.totals()[4], 0, "nothing was ever applied");
}

/// 🔴 AC-045, second half (**M5-10, adopted (a)**; sem: SEM-gx-engine-560): the deadline is evaluated by whoever arrives
/// late.
///
/// No reaper runs here. A caller asks for `verify` after the deadline and finds the transformation
/// already aborted — which is what makes INV-L1 hold in a deployment that never sweeps.
#[test]
fn ac_045_a_late_caller_finds_the_deadline_already_passed() {
    let dir = scratch("ac045_lazy");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens")
    .with_ttl(TTL, TTL);
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/late.txt", "after");
    engine.submit(&i, 2, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");

    let refused = engine
        .verify(&id, later(150), &signing_key(), None)
        .expect_err("the row expired before the call arrived");
    println!(
        "AC045_LAZY refused={:?} state={:?}",
        refused.kind(),
        engine.state(&id)
    );
    assert_eq!(refused.kind(), "InvalidState");
    assert_eq!(
        engine.state(&id),
        Some(Lifecycle::Aborted(AbortReason::Expired)),
        "the expiry happened when the deadline passed, not when somebody noticed"
    );
}

/// 🔴 AC-045, third half: `Escalated` expires on the **other** deadline (INV-L2).
///
/// "any `Escalated` reaches one of `Admitted`/`Denied`/`Aborted(Expired)` within finite time (no
/// indefinite hold)" (sem: SEM-gx-engine-561). The two deadlines are set to different values
/// here, so a row that used the
/// wrong one would expire at the wrong moment rather than not at all — which a single-TTL fixture
/// could not tell apart.
#[test]
fn ac_045_an_escalated_transformation_expires_on_the_escalation_deadline() {
    let dir = scratch("ac045_escalated");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens")
    .with_ttl(100 * MS, 400 * MS);
    // E-M3-4: no inverse can be built, so the gate escalates.
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/escalated.txt", "after");
    engine.submit(&i, 3, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let state = engine.verify(&id, AT, &signing_key(), None).expect("T-4c");
    let deadline = engine.deadline(&id).expect("an Escalated has one");

    let not_yet = engine.reap(later(300)).expect("a sweep");
    let state_mid = engine.state(&id);
    let expired = engine.reap(later(500)).expect("a sweep");
    println!(
        "AC045_ESCALATED state={state:?} deadline={deadline:?} at300={} state_mid={state_mid:?} \
         at500={} state_end={:?}",
        not_yet.len(),
        expired.len(),
        engine.state(&id)
    );
    assert_eq!(state, Lifecycle::Escalated);
    assert_eq!(
        deadline,
        Timestamp(AT.0 + 400 * MS),
        "the escalation deadline, not the verify one"
    );
    assert!(
        not_yet.is_empty(),
        "300 ms is past `verify_ttl` and inside `escalation_ttl`; a row that took the wrong \
         deadline would have died here"
    );
    assert_eq!(state_mid, Some(Lifecycle::Escalated));
    assert_eq!(expired, vec![id]);
    assert_eq!(
        engine.state(&id),
        Some(Lifecycle::Aborted(AbortReason::Expired))
    );
}

/// 🔴 AC-045's last clause (INV-L4): a transformation **waiting on a `Conflicts`** still expires.
///
/// > Also confirm that TTL acts on a Transformation waiting via `Conflicts` too, and it does not
/// > become indefinite. (sem: SEM-gx-engine-562)
///
/// 43 §8 is where the waiting comes from: "`Commutation::Conflicts{residual}` → the engine keeps
/// `T2` in the wait queue as `Candidate` or `Verifying` (no new state is added; only an internal
/// annotation `blocked_by: TransformationId`)" and "the wait is not indefinite: `Candidate`/
/// `Verifying`'s TTL (T-6, §3) applies during the wait too, and it becomes `Aborted(Expired)` if
/// exceeded." (sem: SEM-gx-engine-562)
///
/// Both halves are read: that the second transformation really is held (`blocked_by` names the
/// first, no `VerifyStarted` was written for it, its state is still `Candidate`), and that the
/// clock kept running while it was.
#[test]
fn ac_045_a_transformation_waiting_on_a_conflict_still_expires() {
    let dir = scratch("ac045_conflict");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens")
    .with_ttl(TTL, TTL);
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.conflicting()), "commit-adapter-1");

    // Two transformations over **one object**: 43 §8's per-object rule is about the `Subject`, and
    // one locator is one `ObjectId` for this adapter.
    let first = intent("/tmp/shared.txt", "after-one");
    engine.submit(&first, 4, AT).expect("submit");
    let a = engine.plan(&first, AT).expect("plan");
    let state_a = engine.verify(&a, AT, &signing_key(), None).expect("T-4a");

    let second = intent("/tmp/shared.txt", "after-two");
    engine.submit(&second, 5, AT).expect("submit");
    let b = engine.plan(&second, AT).expect("plan");
    let held = engine
        .verify(&b, later(10), &signing_key(), None)
        .expect("43 §8 holds it rather than refusing");
    let verify_started_for_b = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.kind() == "VerifyStarted" && r.transformation() == Some(b))
        .count();

    let expired = engine.reap(later(150)).expect("the sweep");
    println!(
        "AC045_CONFLICT state_a={state_a:?} held={held:?} blocked_by={:?} \
         verify_started_for_b={verify_started_for_b} expired={:?} state_b={:?} deadline_b={:?}",
        engine.blocked_by(&b),
        expired,
        engine.state(&b),
        engine.deadline(&b)
    );
    assert_eq!(state_a, Lifecycle::Admitted, "the first one proceeds");
    assert_eq!(held, Lifecycle::Candidate, "43 §8 adds no state");
    assert_eq!(
        engine.blocked_by(&b),
        Some(a),
        "\"only an internal annotation `blocked_by: TransformationId`\" (sem: SEM-gx-engine-563)"
    );
    assert_eq!(
        verify_started_for_b, 0,
        "T-3 did not fire, so the journal records no attempt"
    );
    assert!(
        expired.contains(&b),
        "INV-L4: \"the wait does not become indefinite (TTL applies during the wait too)\" (sem: \
         SEM-gx-engine-564) -- {expired:?}"
    );
    assert_eq!(
        engine.state(&b),
        Some(Lifecycle::Aborted(AbortReason::Expired))
    );
    assert_eq!(engine.deadline(&b), None);
}
