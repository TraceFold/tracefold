// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-1003-E2E** — the seventh cause, observed at runtime rather than in source.
//!
//! `req/1003` §7c named the honest remainder of R-1001-1's acceptance: the wiring of
//! `NotAttemptedBecause::PromisedPostStateWasWrong` at `pipeline.rs`'s fourth construction site
//! was guarded only by a **source-scan** gate (`crates/gx-cli/tests/r26_not_attempted_causes.rs`,
//! `every_construction_of_the_third_value_names_its_cause`) — the §7b adversarial injection
//! removed the `insert` and every *runtime* test stayed green. No test drove a mispredicted
//! apply through the real engine and watched the seventh cause come out.
//!
//! This file is that test. The bed is `m5_11_postcondition_mismatch.rs`'s, engine-driven end to
//! end (submit → plan → verify → canonicalize → commit — no hand-built state): a fixture adapter
//! whose `plan` fills `PlannedDelta::promised_target` with a digest its own honest `apply` does
//! not produce. What this file asserts and m5-11 does not: **the cause**. m5-11 stops at
//! `Aborted(PostconditionMismatch)`; here the abort's rollback account is read back through the
//! engine's own surface (`Engine::rollback`, `Engine::rollback_not_attempted_because`) and must
//! carry `PromisedPostStateWasWrong` — the same accessor pair the CLI/HTTP layers consume, so the
//! `kind()` string asserted here is the wire word (`crates/gx-cli/src/wrap.rs`'s seventh arm).
//!
//! | probe | promise | expected cause |
//! |---|---|---|
//! | mispredicted | a digest the apply does not produce | `Some(PromisedPostStateWasWrong)` |
//! | kept (negative control) | the true post-state digest | `None` — and `Committed` |
//!
//! The negative control keeps the positive row honest: if the seventh cause appeared on a kept
//! promise too, the positive assertion would be measuring the accessor, not the mispredict.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use gx_core::{AbortReason, Fingerprint, SubstrateKind, Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, Lifecycle, NotAttemptedBecause, Rollback};
use gx_substrate::{AppliedDelta, InvertOutcome, PlannedDelta, SubstrateAdapter};

use support::{digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";
const BEFORE: &str = "before";
const GOAL: &str = "after";

/// The m5-11 fixture, reduced to the two rows this file needs. `apply` is always honest — the
/// wrongness under test lives entirely in the prophecy, which is exactly the fact the seventh
/// cause's doc claims to name ("the inverse is available and the engine declines").
#[derive(Clone, Debug)]
struct MispredictingAdapter {
    world: Arc<Mutex<Vec<u8>>>,
    /// `true` — `plan` promises a digest of something the apply will not write.
    /// `false` — `plan` promises the true post-state digest (the negative control).
    mispredict: bool,
}

impl MispredictingAdapter {
    fn new(mispredict: bool) -> Self {
        Self {
            world: Arc::new(Mutex::new(BEFORE.as_bytes().to_vec())),
            mispredict,
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for MispredictingAdapter {
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
        Ok(if self.mispredict {
            delta.with_promised_target(digest_of(b"a world the apply will not produce"))
        } else {
            delta.with_promised_target(digest_of(&intent.goal().0))
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

fn engine_over(dir: &Path, adapter: &MispredictingAdapter) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "r1003-mispredicting-fixture/1");
    engine
}

/// Drive one intent through the whole pipeline and hand back what the engine says of it.
fn drive(
    name: &str,
    mispredict: bool,
) -> (
    MispredictingAdapter,
    Engine<InjectedEvidence>,
    TransformationId,
    Lifecycle,
) {
    let dir = scratch(name);
    let adapter = MispredictingAdapter::new(mispredict);
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

/// 🔴 The positive row — the one `req/1003` §7c said nothing was watching at runtime.
///
/// Valid when the fixture's `plan` fills `promised_target` with a digest its honest `apply` does
/// not produce (`mispredict: true`) — with the seat empty or the promise kept, the fourth
/// construction site is never reached and this test measures nothing.
#[test]
fn a_mispredicted_apply_travels_with_the_seventh_cause() {
    let (adapter, engine, id, state) = drive("r1003_mispredicted", true);
    let cause = engine.rollback_not_attempted_because(&id);
    println!(
        "R1003_MISPREDICTED state={state:?} rollback={:?} cause={cause:?}",
        engine.rollback(&id)
    );

    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::PostconditionMismatch),
        "the bed itself: a promise the apply does not keep must abort (m5-11's row), or the \
         cause assertions below are reading a road this test did not take"
    );
    assert_eq!(
        adapter.world(),
        GOAL.as_bytes(),
        "and the apply really did move the world — the promise was wrong, not the apply, which \
         is the exact fact the seventh cause exists to name"
    );
    assert_eq!(
        engine.rollback(&id),
        Some(Rollback::NotAttempted),
        "the escrowed inverse exists and was deliberately not sent — fail-closed on a model the \
         engine just measured to be wrong (`pipeline.rs`'s fourth construction site)"
    );
    assert_eq!(
        cause,
        Some(NotAttemptedBecause::PromisedPostStateWasWrong),
        "🔴 R-1003-E2E's whole claim: the seventh cause is not only inserted in source (r26's \
         scan) but actually reaches the engine's account of a real mispredicted apply"
    );
    assert_eq!(
        cause.map(|c| c.kind()),
        Some("PromisedPostStateWasWrong"),
        "and `kind()` — the string the CLI proxy and the HTTP extension member carry — spells \
         the wire word for it"
    );
}

/// The negative control — the seventh cause appears **only** on the mispredicted road.
///
/// Valid when the fixture's `plan` promises the true post-state digest (`mispredict: false`) —
/// the comparison at the fourth construction site runs and passes, so a cause showing up here
/// would mean the accessor answers regardless of what happened.
#[test]
fn a_kept_promise_commits_and_carries_no_seventh_cause() {
    let (adapter, engine, id, state) = drive("r1003_kept", false);
    let cause = engine.rollback_not_attempted_because(&id);
    println!("R1003_KEPT state={state:?} cause={cause:?}");

    assert_eq!(
        state,
        Lifecycle::Committed,
        "a kept promise is the unchanged road — if this aborts, the fixture is catching \
         something the engine should not be charged with"
    );
    assert_eq!(adapter.world(), GOAL.as_bytes());
    assert_eq!(
        cause, None,
        "🔴 the negative control: no abort, no `NotAttempted`, no cause — the seventh word is \
         reached by exactly one road and this is not it"
    );
}
