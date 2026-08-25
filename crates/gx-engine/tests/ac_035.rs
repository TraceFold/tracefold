// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-035 (FR-035) — `apply` is called from the `Committing` state, after the CAS, once.
//!
//! 34 AC-035, verbatim: "Given: every non-Committing state of the pipeline (Candidate/Verifying/
//! Admitted/Canonicalized). When: ordinary execution. Then: the mock adapter's `apply` call count
//! = 0. `apply` is called exactly once, only in the Committing state after the CAS passes."
//! (sem: SEM-gx-engine-447)
//!
//! 32 FR-035: "gx-engine MUST call `adapter.apply(delta)` only after a commit is approved, and the
//! engine itself MUST NOT perform the substrate change" (sem: SEM-gx-engine-447).
//!
//! # Rule 2, with two instruments (req/78 §6.2, hand 4)
//!
//! > AC-035 green (apply 0 times across every non-Committing state, **and once in Committing after
//! > the CAS passes** -- Rule 2's singleness measured by **two instruments**, a source scan and a
//! > counter) (sem: SEM-gx-engine-447)
//!
//! The counter is here. The scan is `tests/commit_protocol.rs`
//! (`adapter_apply_is_invoked_from_one_line_in_the_crate`), and the two answer different questions (sem: SEM-gx-engine-448): a
//! counter says how many times a road was walked in one scenario, and a scan says how many roads
//! exist. Neither implies the other — a second call site that this scenario does not reach is
//! invisible to the counter, and a road walked twice is invisible to the scan.
//!
//! # Why the state list is walked rather than asserted at the end
//!
//! "every non-Committing state" (sem: SEM-gx-engine-449) is four states, and a probe that ran the whole pipeline and then checked
//! the counter would be measuring the sum. The counter is read **after each transition**, so a
//! `verify` that applied and a `canonicalize` that applied are different failures rather than one.

mod support;

use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, CommitAdapter, Counts, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The index of `apply` in `Counts::totals` (41 §4's trait order).
const APPLY: usize = 4;

fn engine(name: &str) -> (Engine<InjectedEvidence>, Arc<Counts>) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
    (engine, counts)
}

/// 🔴 The criterion: zero in every non-`Committing` state, and one after the CAS passes.
#[test]
fn ac_035_apply_is_zero_until_committing_and_one_after_the_cas() {
    let (mut e, counts) = engine("ac035_counter");
    let i = intent("/tmp/target.txt", "after");

    e.submit(&i, 42, AT).expect("submit");
    println!("APPLY_AFTER_SUBMIT={}", counts.totals()[APPLY]);
    assert_eq!(counts.totals()[APPLY], 0, "T-1 touches no substrate");

    let id = e.plan(&i, AT).expect("plan");
    assert_eq!(e.state(&id), Some(Lifecycle::Candidate));
    println!("APPLY_IN_CANDIDATE={}", counts.totals()[APPLY]);
    assert_eq!(counts.totals()[APPLY], 0, "Candidate: T-2 reads only");

    e.verify(&id, AT, &signing_key(), None).expect("verify");
    assert_eq!(e.state(&id), Some(Lifecycle::Admitted));
    println!("APPLY_IN_ADMITTED={}", counts.totals()[APPLY]);
    assert_eq!(
        counts.totals()[APPLY],
        0,
        "Verifying and Admitted: T-3 and T-4a read only"
    );

    e.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(e.state(&id), Some(Lifecycle::Canonicalized));
    println!("APPLY_IN_CANONICALIZED={}", counts.totals()[APPLY]);
    assert_eq!(
        counts.totals()[APPLY],
        0,
        "Canonicalized: T-8 is a computation over the transformation"
    );

    let state = e.commit(&id, AT, &signing_key()).expect("commit");
    let totals = counts.totals();
    println!(
        "STATE={state:?} APPLY_AFTER_COMMIT={} COUNTS={totals:?}",
        totals[APPLY]
    );
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(
        totals[APPLY], 1,
        "43 T-11: exactly one application, and it is the whole of what the engine does to the world"
    );
}

/// The `Verifying` state, caught in the middle rather than inferred from its neighbours.
///
/// `verify` runs T-3 and one of T-4a..T-4e in one call, so the state between them is never returned
/// to a caller. What can be measured is the journal: `VerifyStarted` is on the device, and the
/// counter was zero when the transition that follows it ran. A `Verifying` in which `apply` had been
/// called would leave the two records with a call between them, which is what the count says did not
/// happen.
#[test]
fn ac_035_the_verifying_state_is_covered_by_the_journal_rather_than_by_a_return() {
    let (mut e, counts) = engine("ac035_verifying");
    let i = intent("/tmp/target.txt", "after");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");
    e.verify(&id, AT, &signing_key(), None).expect("verify");

    let kinds: Vec<&str> = e
        .journal()
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    println!(
        "JOURNAL_THROUGH_VERIFY={kinds:?} APPLY={}",
        counts.totals()[APPLY]
    );
    assert!(kinds.contains(&"VerifyStarted"), "T-3 ran");
    assert!(kinds.contains(&"Verdict"), "T-4a ran");
    assert_eq!(counts.totals()[APPLY], 0);
}

/// A commit refused from a non-`Canonicalized` state applies nothing.
///
/// 43's transition table gives `commit_start` one from-state, and the states either side of it are
/// the ones a caller is most likely to reach for: `Candidate` (before verification) and `Committed`
/// (again). Neither may touch the substrate, and the second is 43 T-9's idempotency rule rather than
/// a refusal — "a duplicate `commit_start` request is ignored once already Committing" (sem: SEM-gx-engine-450), with `Committed` terminal under 43 §1.
#[test]
fn ac_035_a_commit_from_the_wrong_state_applies_nothing() {
    let (mut e, counts) = engine("ac035_wrong_state");
    let i = intent("/tmp/target.txt", "after");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");

    let refused = e.commit(&id, AT, &signing_key());
    println!(
        "COMMIT_FROM_CANDIDATE={:?} APPLY={}",
        refused.as_ref().err().map(gx_engine::Error::kind),
        counts.totals()[APPLY]
    );
    assert_eq!(
        refused
            .expect_err("43 T-9's from-state is Canonicalized")
            .kind(),
        "InvalidState"
    );
    assert_eq!(counts.totals()[APPLY], 0);
    assert_eq!(
        e.journal()
            .records()
            .iter()
            .filter(|r| r.kind() == "CommittingStarted")
            .count(),
        0,
        "a refused commit opens no section"
    );

    e.verify(&id, AT, &signing_key(), None).expect("verify");
    e.canonicalize(&id, AT, None).expect("canonicalize");
    e.commit(&id, AT, &signing_key()).expect("commit");
    assert_eq!(counts.totals()[APPLY], 1);

    // 43 T-9's idempotency, and 43 §1's terminality: the second request writes nothing.
    let records = e.journal().len();
    let again = e.commit(&id, AT, &signing_key()).expect("a second request");
    println!(
        "SECOND_COMMIT={again:?} APPLY={} RECORDS_BEFORE={records} RECORDS_AFTER={}",
        counts.totals()[APPLY],
        e.journal().len()
    );
    assert_eq!(again, Lifecycle::Committed);
    assert_eq!(counts.totals()[APPLY], 1, "the world moved once, not twice");
    assert_eq!(
        e.journal().len(),
        records,
        "\"ignored\" in a journal means no record: a re-entry recorded as an event is a re-entry \
         reported as a second commit (sem: SEM-gx-engine-451)"
    );
    assert_eq!(e.ledger().log().len(), 1, "INV-S3: at most one leaf per id");
}
