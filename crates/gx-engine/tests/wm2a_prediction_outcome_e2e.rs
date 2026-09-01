// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **WM-2a** (`req/1007` §4 item 2, `req/1010`) — the prediction record, observed at runtime.
//!
//! `req/1003` landed the **negative** half: a promise the apply does not keep aborts with
//! `NotAttemptedBecause::PromisedPostStateWasWrong`, and `r1003_seventh_cause_e2e.rs` watches it
//! happen through the engine's own surface. What that left behind is an asymmetry rather than a
//! gap in coverage: a prediction that **failed** produced an abort, a rollback account and a
//! cause, while a prediction that **held** produced nothing at all. From outside the engine, a
//! commit that kept its promise and a commit that never made one were the same event.
//!
//! `Engine::prediction_outcome` closes that, and this file is the measurement. The bed is
//! `r1003_seventh_cause_e2e.rs`'s, widened by one row: the fixture's `plan` either promises the
//! true post-state digest, promises a digest its own honest `apply` will not produce, or promises
//! **nothing** — which is what every adapter this workspace ships does.
//!
//! | probe | `promised_target` | outcome | `matched()` |
//! |---|---|---|---|
//! | kept | the true post-state digest | `Some` | `true` — and `Committed` |
//! | mispredicted | a digest the apply does not produce | `Some` | `false` — and `Aborted` |
//! | silent (negative control) | not filled | `None` | — no comparison was taken |
//!
//! # The two `None`s this file keeps apart
//!
//! The third row is the negative control **and** the point: `None` means no comparison was taken,
//! not that one was taken and failed. A record that appeared on the silent road would make the
//! accessor a fact about the accessor rather than about the world, and folding "not measured" into
//! "measured false" is the failure this workspace's own three-valued vocabulary exists to refuse.
//! So the third probe asserts the absence, and the first asserts that the absence is not simply
//! what the accessor always answers.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use gx_core::{AbortReason, Cid, Fingerprint, SubstrateKind, Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, Lifecycle, PredictionOutcome};
use gx_substrate::{AppliedDelta, InvertOutcome, PlannedDelta, SubstrateAdapter};

use support::{digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";
const BEFORE: &str = "before";
const GOAL: &str = "after";

/// What the fixture's `plan` puts in the prophecy seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Promise {
    /// The true post-state digest — the plan is right.
    Kept,
    /// A digest the honest `apply` will not produce — the plan is wrong.
    Broken,
    /// The seat is left empty, which is what all six shipped adapters do.
    Silent,
}

/// `apply` is always honest: the only thing under test is the prophecy.
#[derive(Clone, Debug)]
struct PredictingAdapter {
    world: Arc<Mutex<Vec<u8>>>,
    promise: Promise,
}

impl PredictingAdapter {
    fn new(promise: Promise) -> Self {
        Self {
            world: Arc::new(Mutex::new(BEFORE.as_bytes().to_vec())),
            promise,
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for PredictingAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
        Ok(gx_core::ObjectSnapshot::new(
            gx_core::ObjectId(digest_of(locator.as_bytes())),
            SubstrateKind::Fs,
            locator.to_string(),
            digest_of(&self.world()),
            gx_core::ReprKind::Bytes,
        ))
    }

    fn plan(
        &self,
        intent: &gx_core::Intent,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<PlannedDelta> {
        let delta = PlannedDelta::new(SubstrateKind::Fs, intent.goal().0.clone())?;
        Ok(match self.promise {
            // The apply writes the goal bytes, so this is the digest it will report.
            Promise::Kept => delta.with_promised_target(digest_of(&intent.goal().0)),
            Promise::Broken => {
                delta.with_promised_target(digest_of(b"a world the apply will not produce"))
            }
            Promise::Silent => delta,
        })
    }

    fn precondition(&self, snap: &gx_core::ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        Ok(Fingerprint::new(
            SubstrateKind::Fs,
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<AppliedDelta> {
        let mut world = self.world.lock().expect("not poisoned");
        world.clone_from(&delta.payload().to_vec());
        let digest = digest_of(&world);
        Ok(AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(SubstrateKind::Fs, SUBJECT.to_string(), digest)?,
            digest,
            Timestamp(0),
        ))
    }

    fn invert(
        &self,
        _delta: &PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<InvertOutcome> {
        let prior = PlannedDelta::new(SubstrateKind::Fs, self.world())?;
        Ok(InvertOutcome::inverted(prior, Vec::new()))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

fn engine_over(dir: &Path, adapter: &PredictingAdapter) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "wm2a-predicting-fixture/1");
    engine
}

/// Drive one intent through the whole pipeline (submit → plan → verify → canonicalize → commit,
/// no hand-built state) and hand back what the engine says of it.
fn drive(
    name: &str,
    promise: Promise,
) -> (
    PredictingAdapter,
    Engine<InjectedEvidence>,
    TransformationId,
    Lifecycle,
) {
    let dir = scratch(name);
    let adapter = PredictingAdapter::new(promise);
    let mut engine = engine_over(&dir, &adapter);
    let one = intent(SUBJECT, GOAL);
    let key = signing_key();

    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &key, None).expect("verify"),
        Lifecycle::Admitted,
        "the fixture constructs an inverse, so C-25 answers `True` and T-4a is the door"
    );
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &key).expect("commit answers");
    (adapter, engine, id, state)
}

/// 🔴 WM-2a's whole claim: a promise that is **kept** is now a record and not a silence.
///
/// Valid when the fixture's `plan` fills `promised_target` with the digest its honest `apply`
/// does produce — with the seat empty the comparison is never taken and this test measures
/// nothing, which is exactly what [`a_transformation_that_predicts_nothing_records_nothing`]
/// holds the other end of.
#[test]
fn a_kept_promise_is_recorded_as_a_prediction_that_held() {
    let (adapter, engine, id, state) = drive("wm2a_kept", Promise::Kept);
    let outcome = engine.prediction_outcome(&id);
    println!("WM2A_KEPT state={state:?} outcome={outcome:?}");

    assert_eq!(
        state,
        Lifecycle::Committed,
        "a kept promise is the unchanged road — if this aborts, the bed is catching something \
         else and the assertions below are reading a road this test did not take"
    );
    assert_eq!(
        adapter.world(),
        GOAL.as_bytes(),
        "the apply moved the world"
    );

    let outcome = outcome.expect(
        "🔴 WM-2a: the comparison was taken on this road, so it is recorded — before this lane a \
         commit that kept its promise was indistinguishable from one that never made one",
    );
    assert!(
        outcome.matched(),
        "the plan predicted the digest the apply reported: {outcome:?}"
    );
    assert_eq!(
        outcome.predicted, outcome.observed,
        "`matched()` is derived from exactly these two digests and must not be able to say \
         otherwise"
    );
    assert_eq!(
        outcome.observed,
        digest_of(GOAL.as_bytes()),
        "and the digest recorded is the world's, not an artefact of the record"
    );
    assert_eq!(
        outcome.observed_at, AT,
        "the moment is the engine's `at`, not a clock read"
    );
}

/// The mispredicted road records the comparison too — and records that it failed.
///
/// The record is written **before** the `!=` branch returns, so the abort does not cost the
/// prediction its account. `req/1003`'s seventh cause says this same event from the rollback's
/// side; this asserts the two agree, which is what one write site buys.
#[test]
fn a_broken_promise_is_recorded_as_a_prediction_that_failed() {
    let (adapter, engine, id, state) = drive("wm2a_broken", Promise::Broken);
    let outcome = engine.prediction_outcome(&id);
    println!(
        "WM2A_BROKEN state={state:?} outcome={outcome:?} cause={:?}",
        engine.rollback_not_attempted_because(&id)
    );

    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::PostconditionMismatch),
        "a promise the apply does not keep aborts (m5-11's row)"
    );
    assert_eq!(
        adapter.world(),
        GOAL.as_bytes(),
        "the promise was wrong, not the apply"
    );

    let outcome = outcome.expect(
        "the comparison was taken here too — an abort must not swallow the record, or the model \
         is scored only on the commits that went well",
    );
    assert!(
        !outcome.matched(),
        "the plan predicted a digest the apply did not report: {outcome:?}"
    );
    assert_ne!(outcome.predicted, outcome.observed);
    assert_eq!(
        outcome.observed,
        digest_of(GOAL.as_bytes()),
        "`observed` is what the world actually became, not what was promised"
    );
    assert_eq!(
        engine.rollback_not_attempted_because(&id).map(|c| c.kind()),
        Some("PromisedPostStateWasWrong"),
        "🔴 the two halves are written at one site and must not disagree: R-1001-1's seventh \
         cause and `matched() == false` are the same event seen from two sides"
    );
}

/// 🔴 The negative control — no prediction, no record. `None` is "not measured", not "wrong".
///
/// This is the road **every adapter this workspace ships** takes, so it is also the compatibility
/// claim: the `let Some` guard at the comparison site does not open, and nothing is written.
#[test]
fn a_transformation_that_predicts_nothing_records_nothing() {
    let (adapter, engine, id, state) = drive("wm2a_silent", Promise::Silent);
    let outcome = engine.prediction_outcome(&id);
    println!("WM2A_SILENT state={state:?} outcome={outcome:?}");

    assert_eq!(
        state,
        Lifecycle::Committed,
        "an adapter that promises nothing commits exactly as it did before this lane"
    );
    assert_eq!(adapter.world(), GOAL.as_bytes());
    assert_eq!(
        outcome, None,
        "🔴 no comparison was taken, so there is no outcome to report. Folding this into a \
         `matched: false` would report a measurement nobody made — the defect the three-valued \
         vocabulary of this workspace exists to refuse"
    );
    assert_eq!(
        engine.rollback_not_attempted_because(&id),
        None,
        "and no seventh cause either: the mispredicted road was not taken"
    );
}

/// `matched()` is derived, so it cannot disagree with the digests beside it.
///
/// A unit-level statement of what the three probes above assert through the engine: the predicate
/// reads the two fields and holds nothing of its own. A stored flag would need this test to catch
/// it going stale; a derived one makes the staleness unrepresentable, and this records that the
/// choice was made rather than defaulted.
#[test]
fn matched_is_a_function_of_the_two_digests_and_holds_no_state() {
    let same = Cid([7u8; 32]);
    let other = Cid([9u8; 32]);

    let held = PredictionOutcome {
        predicted: same,
        observed: same,
        observed_at: AT,
    };
    let failed = PredictionOutcome {
        predicted: same,
        observed: other,
        observed_at: AT,
    };

    println!("WM2A_DERIVED held={held:?} failed={failed:?}");
    assert!(held.matched());
    assert!(!failed.matched());
    assert_ne!(
        held, failed,
        "the two records are distinguishable by the digests alone"
    );
}
