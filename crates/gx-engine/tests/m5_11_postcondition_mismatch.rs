// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M5-11 / blocker item 5** — the prophecy and its refusal, driven in both directions.
//!
//! `req/38` §37 filed this ticket in 2026-08-09 with an instruction that held for twenty days:
//! *"until the ruling, do not write the engine-side refusal check; put one line in the doc naming
//! the absence of the check"*. §41 extended it to the pair — the check and **the prediction's
//! supplier** — because ruling one without the other writes nothing. `req/919` A1 ruled the
//! supplier as **M5H2-2 (b)**: an opt-in `PlannedDelta::promised_target`, not an eighth trait
//! method. This file is the check.
//!
//! | probe | promise | what `apply` leaves behind | expected |
//! |---|---|---|---|
//! | kept | the true post-state digest | what the payload says | `Committed` |
//! | broken | a digest of something else | what the payload says | `Aborted(PostconditionMismatch)` |
//! | plausible | the **pre**-state digest (a wrong value that looks right) | what the payload says | `Aborted(PostconditionMismatch)` |
//! | tampered | the true digest of the payload | something else entirely | `Aborted(PostconditionMismatch)` |
//! | silent | none | something else entirely | `Committed` — the negative control |
//!
//! The last row is the one worth reading twice. It commits, and it **should**: an adapter that
//! promises nothing is asking for no check, and `docs/LIMITS.md`'s discipline is to say where the
//! floor is rather than imply it is everywhere. It is also the control that keeps the four rows
//! above it honest — if the fixture rather than the engine were catching the tampering, this row
//! would fail too.
//!
//! N-13 keeps adapters out of this crate, so the fixture implements `gx-substrate`'s contract
//! directly, in the arrangement `tests/dr4634_read_set_absence.rs` uses.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gx_core::{AbortReason, Cid, Fingerprint, SubstrateKind, Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use gx_substrate::{AppliedDelta, InvertOutcome, PlannedDelta, SubstrateAdapter};

use support::{digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";
const BEFORE: &str = "before";
const GOAL: &str = "after";

/// What the fixture's `plan` puts in the prophecy seat.
#[derive(Clone, Copy, Debug)]
enum Promise {
    /// Nothing — every adapter shipped in this workspace, and the road that must not move.
    None,
    /// The digest the payload really produces.
    Kept,
    /// A digest of something else entirely.
    Broken,
    /// The **pre**-state digest: wrong, and wrong in the shape a real bug produces (an adapter
    /// that predicted "no change" for a change).
    PreState,
}

/// What the fixture's `apply` writes.
#[derive(Clone, Copy, Debug)]
enum Behaviour {
    /// The payload, which is what a conforming adapter does.
    Honest,
    /// Something else. The adapter still answers `Ok` and still reports the digest of what it
    /// really wrote, so nothing here is a lie the engine could catch by reading the answer alone
    /// — the world simply is not where the plan said it would be.
    Tampering,
}

#[derive(Clone, Debug)]
struct PromisingAdapter {
    world: Arc<Mutex<Vec<u8>>>,
    promise: Promise,
    behaviour: Behaviour,
}

impl PromisingAdapter {
    fn new(promise: Promise, behaviour: Behaviour) -> Self {
        Self {
            world: Arc::new(Mutex::new(BEFORE.as_bytes().to_vec())),
            promise,
            behaviour,
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for PromisingAdapter {
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

    /// The whole of M5H2-2 (b), from an adapter's side: one builder call on the value `plan`
    /// already returns. `{intent, pre}` is what it had before this ruling too — the information
    /// was never missing, only the seat was.
    fn plan(
        &self,
        intent: &gx_core::Intent,
        pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<PlannedDelta> {
        let delta = PlannedDelta::new(SubstrateKind::Fs, intent.goal().0.clone())?;
        Ok(match self.promise {
            Promise::None => delta,
            Promise::Kept => delta.with_promised_target(digest_of(&intent.goal().0)),
            Promise::Broken => delta.with_promised_target(digest_of(b"something else")),
            Promise::PreState => delta.with_promised_target(*pre.digest()),
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
        match self.behaviour {
            Behaviour::Honest => world.clone_from(&delta.payload().to_vec()),
            Behaviour::Tampering => world.clone_from(&b"not what was planned".to_vec()),
        }
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

fn engine_over(dir: &Path, adapter: &PromisingAdapter) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "m5-11-promising-fixture/1");
    engine
}

/// Drive one intent as far as it goes and hand back the engine, the id and the final state.
fn drive(
    name: &str,
    promise: Promise,
    behaviour: Behaviour,
) -> (
    PromisingAdapter,
    Engine<InjectedEvidence>,
    TransformationId,
    Lifecycle,
) {
    let dir = scratch(name);
    let adapter = PromisingAdapter::new(promise, behaviour);
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

// ---------------------------------------------------------------------------
// The two directions
// ---------------------------------------------------------------------------

/// A promise the adapter keeps commits, and the engine records the promise it checked.
#[test]
fn a_kept_promise_reaches_committed_and_the_target_is_on_the_row() {
    let (adapter, engine, id, state) = drive("m5_11_kept", Promise::Kept, Behaviour::Honest);
    let target = engine.transformation(&id).expect("the row is held").target;
    println!("M5_11_KEPT state={state:?} target={target:?}");

    assert_eq!(
        target,
        Some(digest_of(GOAL.as_bytes())),
        "🔴 `Engine::plan` did not carry `PlannedDelta::promised_target` into 41 §3's `target`, \
         so M5H2-2 (b) is not wired and the check below can never fire"
    );
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(adapter.world(), GOAL.as_bytes());
    assert!(
        engine.receipt(&id).is_some(),
        "T-11 issues a commit receipt on this road"
    );
}

/// A promise the adapter breaks aborts, with M5-11's word and no receipt.
#[test]
fn a_broken_promise_aborts_with_postcondition_mismatch() {
    let (_, engine, id, state) = drive("m5_11_broken", Promise::Broken, Behaviour::Honest);
    println!("M5_11_BROKEN state={state:?}");

    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::PostconditionMismatch),
        "🔴 the plan promised a digest the apply did not produce and the commit went through \
         anyway — this is exactly blocker item 5"
    );
    assert!(
        engine.receipt(&id).is_none(),
        "🔴 fail-closed: a transformation that failed its own postcondition must not leave a \
         signed receipt behind"
    );
    assert_eq!(
        engine.state(&id),
        Some(Lifecycle::Aborted(AbortReason::PostconditionMismatch)),
        "43 §1 makes `Aborted` terminal, so the row keeps the word"
    );
}

// ---------------------------------------------------------------------------
// Adversarial probes (`req/188` §8-5: a repair lane owes at least three)
// ---------------------------------------------------------------------------

/// **Probe 1 — a false prophecy that looks like a true one.** The pre-state digest is the value a
/// real adapter bug produces (a plan that predicted "nothing changes" for a change), so a check
/// that only rejected obviously-foreign bytes would pass it.
#[test]
fn a_prophecy_of_the_pre_state_is_refused_like_any_other_wrong_one() {
    let (adapter, _, _, state) = drive("m5_11_prestate", Promise::PreState, Behaviour::Honest);
    println!("M5_11_PRESTATE state={state:?}");
    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::PostconditionMismatch)
    );
    assert_eq!(
        adapter.world(),
        GOAL.as_bytes(),
        "the fixture really did change the object — the promise was wrong, not the apply"
    );
}

/// **Probe 2 — the apply tampers after the plan.** The promise is the true digest of the payload
/// and the adapter writes something else, which is the case M4-06 called "adapter
/// self-consistency" and which L5 measures for a harness. Here it is measured for a **commit**.
#[test]
fn an_apply_that_writes_something_else_is_caught_by_the_promise() {
    let (adapter, engine, id, state) = drive("m5_11_tampered", Promise::Kept, Behaviour::Tampering);
    println!("M5_11_TAMPERED state={state:?} world={:?}", adapter.world());

    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::PostconditionMismatch)
    );
    assert!(engine.receipt(&id).is_none());
    assert_ne!(
        adapter.world(),
        GOAL.as_bytes(),
        "the fixture is supposed to have written something else"
    );
}

/// **Probe 3 — the empty promise, which is the whole compatibility claim.**
///
/// 🔴 This is the negative control, and it is deliberately a **commit**. The same tampering
/// adapter as the probe above, with the seat left empty, reaches `Committed`: with no prediction
/// there is no comparison, and the engine says nothing about a post-state nobody promised. Two
/// things follow, and both are the point.
///
/// * The check is doing the catching, not the fixture. If the tampering above had been caught by
///   something else in the pipeline, this row would abort too.
/// * `target` is `None` here, which is bit-for-bit what this road produced before M5H2-2 landed —
///   so the `TransformationId` of every transformation the shipped adapters can make is unmoved.
#[test]
fn no_promise_is_no_check_and_that_is_the_unchanged_road() {
    let (_, engine, id, state) = drive("m5_11_silent", Promise::None, Behaviour::Tampering);
    let target = engine.transformation(&id).expect("the row is held").target;
    println!("M5_11_SILENT state={state:?} target={target:?}");

    assert_eq!(
        target, None,
        "🔴 an adapter that promised nothing got a `target` anyway — the field is not opt-in and \
         every existing `TransformationId` has moved"
    );
    assert_eq!(
        state,
        Lifecycle::Committed,
        "with no prophecy there is no comparison; `docs/LIMITS.md`'s discipline is to say so"
    );
    assert!(engine.receipt(&id).is_some());
}

/// **Probe 4 — a promise moves the name.** Two engines, one intent, one difference: whether the
/// adapter fills the seat. If the ids matched, `target` would not be reaching the canonical form
/// 43 T-2 says it is in, and the value would be decoration.
#[test]
fn the_promise_reaches_the_transformation_id() {
    let (_, _, silent, _) = drive("m5_11_id_silent", Promise::None, Behaviour::Honest);
    let (_, _, promised, _) = drive("m5_11_id_promised", Promise::Kept, Behaviour::Honest);
    println!("M5_11_IDS silent={silent:?} promised={promised:?}");
    assert_ne!(
        silent, promised,
        "43 T-2: the `TransformationId` is the CID of the canonical form **including \
         delta/target**, so a plan that promises and one that does not are two transformations"
    );
}

// ---------------------------------------------------------------------------
// Principle 2 (lightweight and fast) — the runtime cost, measured, not asserted
// ---------------------------------------------------------------------------

/// What the check costs on the road that does not use it: one `Option` discriminant read.
///
/// **What this number is and is not.** It times the comparison itself, `n` times, on this
/// machine, warm — it is **not** a commit latency and does not replace
/// `benches/commit_pipeline.rs`'s p99, which is the instrument AC-065 names. Both loops carry a
/// `black_box` on each side, which forces a memory round trip the shipped code does not pay, so
/// the figures are an **upper bound** on the added work rather than an estimate of it. It is here
/// because the second design principle asks a lane that adds work to a hot path to measure the
/// work it added, and the work added is exactly these two lines.
#[test]
fn the_added_check_costs_what_a_compare_costs() {
    const N: u32 = 1_000_000;
    let absent: Option<Cid> = None;
    let present: Option<Cid> = Some(Cid([3u8; 32]));
    let observed = Cid([3u8; 32]);

    let mut hits = 0u32;
    let started = Instant::now();
    for _ in 0..N {
        if let Some(promised) = std::hint::black_box(absent) {
            if promised != std::hint::black_box(observed) {
                hits += 1;
            }
        }
    }
    let none_path = started.elapsed();

    let started = Instant::now();
    for _ in 0..N {
        if let Some(promised) = std::hint::black_box(present) {
            if promised != std::hint::black_box(observed) {
                hits += 1;
            }
        }
    }
    let some_path = started.elapsed();

    println!(
        "M5_11_BENCH n={N} none_path_total={none_path:?} some_path_total={some_path:?} \
         none_ns_per_op={:.3} some_ns_per_op={:.3} hits={hits}",
        none_path.as_nanos() as f64 / f64::from(N),
        some_path.as_nanos() as f64 / f64::from(N)
    );
    assert_eq!(
        hits, 0,
        "the fixture's promise is kept, so neither loop fires"
    );
}
