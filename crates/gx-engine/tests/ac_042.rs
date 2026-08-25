// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-042** — INV-S1 over generated execution traces (FR-032/033/034/036/037).
//!
//! 34 AC-042, verbatim: "Given: a model-based test (proptest state-machine strategy) that generates
//! execution traces of a state machine, including a random verdict sequence, TTL elapse, and
//! record-only toggling. When: extract, from every generated trace, the Transformations that
//! reached `Committed`. Then: each one must pass through `Admitted ∧ Canonicalized` on its path (or,
//! under the RecordOnly exception, `Denied ∧ enforced=false ∧ Canonicalized`). If even one
//! violating path is generated, the test fails (43 INV-S1). | property (model-based state machine
//! test)" (sem: SEM-gx-engine-522)
//!
//! # Why this file is plain `proptest` and not `proptest-state-machine`
//!
//! **M5-15, adopted (b)** (req/38 §37): "a hand-rolled model generating `Vec<Event>` with plain
//! proptest (**external-count stays 238** -- no model-based crate is added)" (sem:
//! SEM-gx-engine-523). A `proptest-state-machine` dependency would move the external package
//! count for a generator this file can write in forty lines, and 34's "proptest state-machine
//! strategy" names a *strategy*, not a crate. So the trace is a `Vec<Event>`, the model is the
//! [`Model`] struct below, and the two are stepped side by side.
//!
//! The ruling also asked for honesty about what the hand-rolled form costs: "any degradation in
//! shrinking quality is honestly recorded" (sem: SEM-gx-engine-524). What proptest shrinks
//! here is the `Vec<Event>` — it removes events and shrinks the numbers inside them — so a
//! counterexample arrives as a *short trace*, which is the readable form. What it cannot do is
//! shrink across the engine-level configuration and the trace at once with any notion of which is
//! "simpler" (sem: SEM-gx-engine-525); that axis is a plain 4-tuple of booleans and is printed
//! verbatim beside the trace when a case fails ([`Config`]'s `Debug`).
//!
//! # Two claims, and only the second one is AC-042
//!
//! 1. **The model agrees with the engine.** After every event, the model's predicted [`Lifecycle`]
//!    for every transformation equals `Engine::state`. This is the "model-based" half (sem:
//!    SEM-gx-engine-526): a property that only read the engine's own answers would be asking
//!    the implementation to grade itself.
//! 2. **INV-S1 holds of every `Committed` transformation** — and this is read out of the
//!    **journal**, not out of the in-memory table, because 43 §7 makes the journal the truth and the
//!    table a cache. [`path_of`] rebuilds the road each transformation walked from the records it
//!    left, and [`inv_s1`] is 34's sentence over that road.
//!
//! # What the generator does not vary, and why
//!
//! `escalating` (an adapter that cannot invert, E-M3-4) and a `/etc` locator (which the invariant
//! denies) are **mutually exclusive** in [`Config`]: the gate is asked one question with both
//! `invert_available=false` and a denying payload, and which of `Deny` and `Escalate` it answers is
//! gx-gate's precedence rather than 43's. Modelling a guess at it would make this file fail on a
//! gx-gate change that AC-042 says nothing about. The 2×4 grid of modes and verdicts is walked
//! exhaustively next door in `tests/ac_037.rs`; what this file adds is the *sequence*.

mod support;

use std::sync::{Arc, Mutex};

use gx_core::{AbortReason, Actor, EnforcementMode, FailPosture, Timestamp, TransformationId};
use gx_engine::{Engine, EngineJournalRecord, HumanRuling, Lifecycle};
use proptest::prelude::*;
use support::{
    gate, intent, ruler, scratch, signing_key, CommitAdapter, MaybeEvidence, FORBID_ETC,
};

/// Where the simulated clock starts. Every `at` this file passes is derived from it, so no test
/// here waits for real time (43 T-6 is a comparison against an injected value, 41 §6).
const T0: i64 = 1_754_000_000_000_000_000;

/// One tick of [`Event::Elapse`], in nanoseconds.
const TICK: i64 = 400_000_000;

/// `verify_ttl` and `escalation_ttl`.
///
/// 🔴 Chosen against [`TICK`] rather than for readability. At `TTL = 3 * TICK` the first `Elapse`
/// in a trace killed every waiting transformation, and the first tuning run reached `Committed`
/// **once in sixty-four traces** — a property that is technically true and measures nothing (34's
/// Then quantifies over "a Transformation that reached `Committed`" (sem: SEM-gx-engine-527).
/// At twelve ticks an expiry takes
/// several `Elapse` events, so both the expiry road and the commit road are walked. The counts are
/// printed by the test so that a future change to either constant shows up as a number rather than
/// as a silent loss of coverage.
const TTL: i64 = 12 * TICK;

// ---------------------------------------------------------------------------
// The trace alphabet
// ---------------------------------------------------------------------------

/// One step of a generated execution trace.
///
/// 34's Given names three ingredients and each has an event: "a random verdict sequence" (sem:
/// SEM-gx-engine-528) is [`Event::Submit`]'s `denied` flag plus [`Config`], "TTL elapse" is
/// [`Event::Elapse`] and [`Event::Reap`], and "record-only toggling" is [`Event::Toggle`].
#[derive(Clone, Debug)]
enum Event {
    /// T-1 and T-2 for a new transformation. `denied` picks a locator the invariant refuses.
    Submit { denied: bool },
    /// Advance transformation `i` by whichever entry point 43 §3 offers from where it is.
    Step { i: usize },
    /// T-5 / T-5b — a person answers an escalation.
    Decide { i: usize, approve: bool },
    /// T-7 — the owner cancels.
    Cancel { i: usize },
    /// The clock moves `ticks` ticks forward. Nothing else happens.
    Elapse { ticks: u8 },
    /// T-6 as a sweep (`Engine::reap`).
    Reap,
    /// DR-2's `EnforcementMode` axis is flipped mid-flight.
    Toggle,
}

/// The engine-level configuration one trace runs under.
///
/// Sampled per trace rather than per event because each of these is fixed at `Engine::open` or is a
/// property of the registered adapter, which is exactly the shape of a deployment.
#[derive(Clone, Copy, Debug)]
struct Config {
    /// The starting `EnforcementMode`. [`Event::Toggle`] moves it from here.
    record_only: bool,
    /// The evidence collector is unreachable and the posture is `FailOpen` — 43 T-4e for every
    /// transformation in the trace.
    degraded: bool,
    /// The adapter cannot build an inverse, which **E-M3-4** turns into `Escalate` at T-4c.
    escalating: bool,
}

fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        2 => any::<bool>().prop_map(|denied| Event::Submit { denied }),
        12 => (0usize..4).prop_map(|i| Event::Step { i }),
        2 => (0usize..4, any::<bool>()).prop_map(|(i, approve)| Event::Decide { i, approve }),
        1 => (0usize..4).prop_map(|i| Event::Cancel { i }),
        2 => (0u8..4).prop_map(|ticks| Event::Elapse { ticks }),
        1 => Just(Event::Reap),
        1 => Just(Event::Toggle),
    ]
}

fn config() -> impl Strategy<Value = Config> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(record_only, degraded, escalating)| {
        Config {
            record_only,
            degraded,
            escalating,
        }
    })
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// What the model believes about one transformation.
#[derive(Clone, Debug)]
struct Row {
    id: TransformationId,
    /// The locator is under `/etc`, which `FORBID_ETC` refuses.
    denied: bool,
    state: Lifecycle,
    /// When the current state was entered — `Engine`'s `entry.since`, which is the left half of 43
    /// T-6's deadline.
    since: i64,
    /// The bytes the world held when T-2 recorded `Fingerprint₀`. The CAS at T-10a recomputes the
    /// fingerprint over the world *now*, so a commit succeeds only if these still match
    /// (`support::CommitAdapter` digests one shared world, which is what makes the trace's
    /// transformations able to invalidate each other).
    world_at_plan: Vec<u8>,
    /// What this transformation will write if it commits.
    payload: Vec<u8>,
    /// `enforced`, once T-8 or T-8r has stamped it.
    enforced: Option<bool>,
}

/// The whole model: the engine's two axes, the clock, the world and the rows.
#[derive(Debug)]
struct Model {
    cfg: Config,
    record_only: bool,
    now: i64,
    world: Vec<u8>,
    rows: Vec<Row>,
}

impl Model {
    fn new(cfg: Config) -> Self {
        Self {
            cfg,
            record_only: cfg.record_only,
            now: T0,
            world: b"before".to_vec(),
            rows: Vec::new(),
        }
    }

    /// 43 T-6's deadline for a row, or `None` where T-6 does not reach (`Engine::deadline_of`).
    fn deadline(&self, row: &Row) -> Option<i64> {
        match row.state {
            Lifecycle::Candidate | Lifecycle::Verifying => Some(row.since + TTL),
            Lifecycle::Escalated => Some(row.since + TTL),
            _ => None,
        }
    }

    /// Whether 43 T-6 has already fallen due on row `i` at the current clock.
    ///
    /// 🔴 Read **before** the entry point is chosen, never after. The engine's lazy half of
    /// **M5-10** runs inside each entry point (`Engine::verify`/`escalation`/`cancel`/`canonicalize`
    /// /`commit` all open with `expire_if_due`), so a transformation whose deadline has passed is
    /// expired *by the call* and the call then refuses with `InvalidState`. A model that expired the
    /// row first and then decided there was nothing to call would never make the call — and the
    /// engine, uncalled, would still be sitting in `Candidate`. That is exactly the disagreement
    /// this file's first red run reported, and it is a fact about lazy expiry rather than a
    /// modelling detail: **nothing expires a transformation nobody asks about** until `reap` sweeps.
    fn due(&self, i: usize) -> bool {
        let now = self.now;
        self.rows
            .get(i)
            .and_then(|row| self.deadline(row))
            .is_some_and(|deadline| now >= deadline)
    }

    /// What the entry point did to a row whose deadline had already passed: T-6, then a refusal.
    fn expire(&mut self, i: usize) {
        let now = self.now;
        let row = &mut self.rows[i];
        row.state = Lifecycle::Aborted(AbortReason::Expired);
        row.since = now;
    }

    /// What `verify` answers for this row, given the trace's configuration.
    fn verdict_state(&self, row: &Row) -> Lifecycle {
        if self.cfg.degraded {
            // T-4e: the collector is unreachable and the posture is `FailOpen`, so the gate is
            // never asked and the transformation is admitted degraded.
            Lifecycle::Admitted
        } else if row.denied {
            Lifecycle::Denied
        } else if self.cfg.escalating {
            Lifecycle::Escalated
        } else {
            Lifecycle::Admitted
        }
    }
}

// ---------------------------------------------------------------------------
// Reading the road out of the journal (43 §7: the journal is the truth)
// ---------------------------------------------------------------------------

/// The states one transformation visited, in order, rebuilt from its journal records.
///
/// Only the records that 43 §3 pairs with a state change are read; `InverseEscrowed`,
/// `ApplyStarted` and `ProvenanceDelivered` happen *inside* `Committing` and add no state.
fn path_of(records: &[EngineJournalRecord], id: &TransformationId) -> Vec<Lifecycle> {
    let mut path = Vec::new();
    for record in records {
        match record {
            EngineJournalRecord::Planned { transformation, .. } if transformation == id => {
                path.push(Lifecycle::Candidate);
            }
            EngineJournalRecord::VerifyStarted { transformation, .. } if transformation == id => {
                path.push(Lifecycle::Verifying);
            }
            EngineJournalRecord::Verdict {
                transformation,
                kind,
                ..
            } if transformation == id => path.push(match kind {
                gx_core::VerdictKind::Admit => Lifecycle::Admitted,
                gx_core::VerdictKind::Deny => Lifecycle::Denied,
                gx_core::VerdictKind::Escalate => Lifecycle::Escalated,
            }),
            EngineJournalRecord::HumanDecision {
                transformation,
                kind,
                ..
            } if transformation == id => path.push(match kind {
                gx_core::VerdictKind::Deny => Lifecycle::Denied,
                _ => Lifecycle::Admitted,
            }),
            EngineJournalRecord::Canonicalized { transformation, .. } if transformation == id => {
                path.push(Lifecycle::Canonicalized);
            }
            EngineJournalRecord::CommittingStarted { transformation, .. }
                if transformation == id =>
            {
                path.push(Lifecycle::Committing);
            }
            EngineJournalRecord::Committed { transformation, .. } if transformation == id => {
                path.push(Lifecycle::Committed);
            }
            EngineJournalRecord::Aborted {
                transformation,
                reason,
                ..
            } if transformation == id => path.push(Lifecycle::Aborted(*reason)),
            _ => {}
        }
    }
    path
}

/// `enforced` as the `Canonicalized` record wrote it (43 T-8r stamps `Some(false)`).
fn canonicalized_enforced(
    records: &[EngineJournalRecord],
    id: &TransformationId,
) -> Option<Option<bool>> {
    records.iter().find_map(|record| match record {
        EngineJournalRecord::Canonicalized {
            transformation,
            enforced,
            ..
        } if transformation == id => Some(*enforced),
        _ => None,
    })
}

/// 🔴 34 AC-042's Then, as a function of one transformation's road.
///
/// > each one must pass through `Admitted ∧ Canonicalized` on its path (under the RecordOnly
/// > exception, `Denied ∧ enforced=false ∧ Canonicalized`) (sem: SEM-gx-engine-529)
///
/// Read as an ordered claim rather than a set membership: `Canonicalized` must be **on** the road,
/// and the state immediately before it must be the admission (T-8) or the record-only denial
/// (T-8r). A road that visited `Admitted`, walked away to `Aborted` and reached `Committed` by some
/// other door would satisfy a set test and violate INV-S1.
fn inv_s1(path: &[Lifecycle], enforced: Option<bool>) -> Result<(), String> {
    let at = path
        .iter()
        .position(|state| *state == Lifecycle::Canonicalized)
        .ok_or_else(|| format!("no `Canonicalized` on the road to `Committed`: {path:?}"))?;
    let before = at
        .checked_sub(1)
        .and_then(|j| path.get(j))
        .ok_or_else(|| format!("`Canonicalized` is the first state on the road: {path:?}"))?;
    match before {
        Lifecycle::Admitted => Ok(()),
        Lifecycle::Denied if enforced == Some(false) => Ok(()),
        Lifecycle::Denied => Err(format!(
            "T-8r fired from `Denied` without stamping `enforced=false` (enforced={enforced:?}): \
             {path:?}"
        )),
        other => Err(format!(
            "`Canonicalized` was entered from {other:?}, which 43 §3 has no edge for: {path:?}"
        )),
    }
}

// ---------------------------------------------------------------------------
// One trace, stepped in both worlds
// ---------------------------------------------------------------------------

/// What one trace left behind, for the property to read.
struct Run {
    /// How many transformations reached `Committed`, by the engine's own table.
    committed: usize,
    /// How many of those were record-only denials (T-8r).
    record_only_commits: usize,
    /// Every state the model and the engine were compared at.
    comparisons: usize,
}

#[allow(clippy::too_many_lines)]
fn run_trace(name: &str, cfg: Config, trace: &[Event]) -> Result<Run, TestCaseError> {
    let dir = scratch(name);
    let evidence = if cfg.degraded {
        MaybeEvidence::down()
    } else {
        MaybeEvidence::none()
    };
    let posture = if cfg.degraded {
        FailPosture::FailOpen
    } else {
        FailPosture::FailClosed
    };
    let mut engine = Some(
        Engine::open(dir.join("journal.bin"), gate(FORBID_ETC), evidence)
            .expect("a fresh journal opens")
            .with_mode(mode_of(cfg.record_only))
            .with_posture(posture)
            .with_ttl(TTL, TTL),
    );
    let (adapter, _counts, world) = CommitAdapter::new("before");
    let adapter = if cfg.escalating {
        adapter.without_inverse()
    } else {
        adapter
    };
    engine
        .as_mut()
        .expect("the engine is in hand")
        .register_adapter(Arc::new(adapter), "commit-adapter-1");

    let mut model = Model::new(cfg);
    let mut comparisons = 0usize;
    let mut record_only_commits = 0usize;
    let key = signing_key();

    for (n, ev) in trace.iter().enumerate() {
        let at = Timestamp(model.now);
        match ev {
            Event::Submit { denied } => {
                let seq = model.rows.len() as u64;
                let locator = if *denied {
                    format!("/etc/trace-{name}-{seq}.txt")
                } else {
                    format!("/tmp/trace-{name}-{seq}.txt")
                };
                let goal = format!("after-{seq}");
                let i = intent(&locator, &goal);
                let engine = engine.as_mut().expect("the engine is in hand");
                engine.submit(&i, seq, at).expect("T-1");
                let id = engine.plan(&i, at).expect("T-2");
                model.rows.push(Row {
                    id,
                    denied: *denied,
                    state: Lifecycle::Candidate,
                    since: model.now,
                    world_at_plan: model.world.clone(),
                    payload: goal.into_bytes(),
                    enforced: None,
                });
            }
            Event::Step { i } => {
                if *i >= model.rows.len() {
                    continue;
                }
                let engine = engine.as_mut().expect("the engine is in hand");
                let id = model.rows[*i].id;
                let due = model.due(*i);
                match model.rows[*i].state {
                    Lifecycle::Candidate => {
                        let answered = engine.verify(&id, at, &key, None);
                        if due {
                            prop_assert_eq!(
                                answered.err().map(|e| e.kind()),
                                Some("InvalidState"),
                                "T-6 fell due before T-3 at event {}",
                                n
                            );
                            model.expire(*i);
                        } else {
                            let want = model.verdict_state(&model.rows[*i]);
                            prop_assert_eq!(
                                answered.expect("T-3..T-4e"),
                                want,
                                "verify at event {} of {:?}",
                                n,
                                cfg
                            );
                            model.rows[*i].state = want;
                            model.rows[*i].since = model.now;
                        }
                    }
                    Lifecycle::Admitted => {
                        let got = engine.canonicalize(&id, at, None).expect("T-8");
                        prop_assert_eq!(got, Lifecycle::Canonicalized);
                        model.rows[*i].state = Lifecycle::Canonicalized;
                        model.rows[*i].since = model.now;
                        // T-4e degraded the transformation to record-only for its own sake
                        // (ASM-13), so `enforced` is false even though the road was `Admitted`.
                        model.rows[*i].enforced = Some(!cfg.degraded);
                    }
                    Lifecycle::Denied => {
                        if model.record_only {
                            let got = engine.canonicalize(&id, at, None).expect("T-8r");
                            prop_assert_eq!(got, Lifecycle::Canonicalized);
                            model.rows[*i].state = Lifecycle::Canonicalized;
                            model.rows[*i].since = model.now;
                            model.rows[*i].enforced = Some(false);
                        } else {
                            // 43 §1: `Denied` is terminal outside record-only, so T-8 has no
                            // from-state here. The refusal is the measurement (AC-041's road).
                            let refused = engine.canonicalize(&id, at, None).err();
                            prop_assert_eq!(
                                refused.map(|e| e.kind()),
                                Some("InvalidState"),
                                "a `Denied` under `Enforce` must refuse T-8"
                            );
                        }
                    }
                    Lifecycle::Canonicalized => {
                        let unchanged = model.world == model.rows[*i].world_at_plan;
                        let got = engine.commit(&id, at, &key).expect("T-9..T-11 or T-10a");
                        if unchanged {
                            prop_assert_eq!(got, Lifecycle::Committed, "CAS held at event {}", n);
                            model.rows[*i].state = Lifecycle::Committed;
                            model.world = model.rows[*i].payload.clone();
                            if model.rows[*i].enforced == Some(false) {
                                record_only_commits += 1;
                            }
                        } else {
                            prop_assert_eq!(
                                got,
                                Lifecycle::Aborted(AbortReason::PreconditionChanged),
                                "the world moved under this transformation at event {}",
                                n
                            );
                            model.rows[*i].state =
                                Lifecycle::Aborted(AbortReason::PreconditionChanged);
                        }
                        model.rows[*i].since = model.now;
                    }
                    // `Escalated` waits for `Decide`; the terminals wait for nothing.
                    _ => {}
                }
            }
            Event::Decide { i, approve } => {
                if *i >= model.rows.len() {
                    continue;
                }
                let engine = engine.as_mut().expect("the engine is in hand");
                let id = model.rows[*i].id;
                let due = model.due(*i);
                let ruling = HumanRuling {
                    decision: if *approve {
                        gx_core::VerdictKind::Admit
                    } else {
                        gx_core::VerdictKind::Deny
                    },
                    reason: "a generated trace ruled on this one".to_string(),
                    actor: ruler_actor(),
                };
                let answered = engine.escalation(&id, &ruling, at, &key);
                if due {
                    prop_assert_eq!(
                        answered.err().map(|e| e.kind()),
                        Some("InvalidState"),
                        "T-6 fell due before the ruling at event {}",
                        n
                    );
                    model.expire(*i);
                } else if model.rows[*i].state == Lifecycle::Escalated {
                    let want = if *approve {
                        Lifecycle::Admitted
                    } else {
                        Lifecycle::Denied
                    };
                    prop_assert_eq!(answered.expect("T-5 / T-5b"), want);
                    model.rows[*i].state = want;
                    model.rows[*i].since = model.now;
                } else {
                    prop_assert_eq!(
                        answered.err().map(|e| e.kind()),
                        Some("InvalidState"),
                        "43 has no human ruling from {:?}",
                        model.rows[*i].state
                    );
                }
            }
            Event::Cancel { i } => {
                if *i >= model.rows.len() {
                    continue;
                }
                let engine = engine.as_mut().expect("the engine is in hand");
                let id = model.rows[*i].id;
                let due = model.due(*i);
                let answered = engine.cancel(&id, at);
                // T-6 already happened at the moment the deadline passed, and this call is later —
                // `Engine::escalation`'s own note, which `cancel` shares.
                if due {
                    model.expire(*i);
                }
                match model.rows[*i].state {
                    // 🔴 43 T-7's idempotency column: "a duplicate cancel is ignored as a no-op
                    // (already Aborted)" (sem: SEM-gx-engine-530). `cancel` answers `Ok` with
                    // the abort already recorded and writes
                    // no second record — which is **not** the `InvalidState` the model first
                    // predicted, and is the second disagreement this file's red run reported.
                    Lifecycle::Aborted(reason) => {
                        prop_assert_eq!(
                            answered.expect("T-7's idempotency answers rather than refuses"),
                            Lifecycle::Aborted(reason),
                            "T-7 on an already-aborted row at event {}",
                            n
                        );
                    }
                    state if cancellable(state) => {
                        prop_assert_eq!(
                            answered.expect("T-7"),
                            Lifecycle::Aborted(AbortReason::OwnerCancelled)
                        );
                        model.rows[*i].state = Lifecycle::Aborted(AbortReason::OwnerCancelled);
                        model.rows[*i].since = model.now;
                    }
                    _ => {
                        prop_assert_eq!(
                            answered.err().map(|e| e.kind()),
                            Some("InvalidState"),
                            "43 T-7 stops at `Committing`"
                        );
                    }
                }
            }
            Event::Elapse { ticks } => {
                model.now += TICK * i64::from(*ticks);
            }
            Event::Reap => {
                let engine = engine.as_mut().expect("the engine is in hand");
                let swept = engine.reap(at).expect("a sweep writes to the journal");
                let mut expected: Vec<TransformationId> = Vec::new();
                for i in 0..model.rows.len() {
                    let due = model
                        .deadline(&model.rows[i])
                        .is_some_and(|deadline| model.now >= deadline);
                    if due {
                        expected.push(model.rows[i].id);
                        model.rows[i].state = Lifecycle::Aborted(AbortReason::Expired);
                        model.rows[i].since = model.now;
                    }
                }
                let mut swept_sorted = swept;
                swept_sorted.sort_unstable();
                expected.sort_unstable();
                prop_assert_eq!(swept_sorted, expected, "T-6's sweep at event {}", n);
            }
            Event::Toggle => {
                model.record_only = !model.record_only;
                let taken = engine.take().expect("the engine is in hand");
                engine = Some(taken.with_mode(mode_of(model.record_only)));
            }
        }

        // Claim 1: the model and the engine agree about every row, after every event.
        let live = engine.as_ref().expect("the engine is in hand");
        for row in &model.rows {
            comparisons += 1;
            prop_assert_eq!(
                live.state(&row.id),
                Some(row.state),
                "model and engine disagree about {:?} after event {} ({:?}) under {:?}",
                row.id,
                n,
                ev,
                cfg
            );
        }
    }

    // Claim 2: **AC-042** — INV-S1 over every transformation that reached `Committed`.
    let live = engine.as_ref().expect("the engine is in hand");
    let records = live.journal().records();
    let mut committed = 0usize;
    for row in &model.rows {
        if live.state(&row.id) != Some(Lifecycle::Committed) {
            continue;
        }
        committed += 1;
        let path = path_of(records, &row.id);
        let enforced = canonicalized_enforced(records, &row.id).flatten();
        if let Err(why) = inv_s1(&path, enforced) {
            return Err(TestCaseError::fail(format!(
                "AC-042 (INV-S1) violated under {cfg:?} for {:?}: {why}",
                row.id
            )));
        }
    }

    // The world the adapter holds is the world the model predicted: a table that agreed with the
    // model while the substrate went somewhere else would be the one failure state comparison
    // cannot see (AC-041's `world` read, generalised over the trace).
    let seen = world.lock().expect("the world is not poisoned").clone();
    prop_assert_eq!(
        String::from_utf8_lossy(&seen).into_owned(),
        String::from_utf8_lossy(&model.world).into_owned(),
        "the substrate and the model disagree about the world under {:?}",
        cfg
    );

    Ok(Run {
        committed,
        record_only_commits,
        comparisons,
    })
}

fn mode_of(record_only: bool) -> EnforcementMode {
    if record_only {
        EnforcementMode::RecordOnly
    } else {
        EnforcementMode::Enforce
    }
}

/// 43 T-7's from-set, which 44 §1.2 repeats verbatim — **minus `Draft`**.
///
/// `Draft` is in both documents and is not here, because **M5-17, adopted (b)** (sem:
/// SEM-gx-engine-531) puts the draft phase in the
/// journal alone: the engine's table has no row until T-2, so `cancel` on a draft answers
/// `NotFound` rather than `InvalidState`. This model never holds a `Draft` row for the same reason,
/// so the arm would be unreachable as well as wrong; **E-M5-14** removed `Draft` from T-7's sibling
/// (43 T-7's from-set for `undo`) on the same argument.
fn cancellable(state: Lifecycle) -> bool {
    matches!(
        state,
        Lifecycle::Candidate
            | Lifecycle::Verifying
            | Lifecycle::Admitted
            | Lifecycle::Canonicalized
            | Lifecycle::Escalated
    )
}

fn ruler_actor() -> Actor {
    ruler(3)
}

// ---------------------------------------------------------------------------
// The property
// ---------------------------------------------------------------------------

/// 🔴 **AC-042** over generated traces.
///
/// The case count is 34's "every generated case" (sem: SEM-gx-engine-532) with 51 §3's budget
/// in mind: each case opens an engine on
/// a fresh scratch directory and walks up to twenty entry points, so the cost per case is
/// milliseconds rather than microseconds and the count is set here rather than taken from
/// `ProptestConfig::default()` (256).
#[test]
fn ac_042_inv_s1_holds_over_every_generated_trace() {
    let cases = 64u32;
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases,
        // A counterexample here is a **trace**, and a seed is not one: 34 asks that the violating
        // path be visible, and proptest prints the shrunk `Vec<Event>` and the `Config` beside the
        // failure. Persisting a seed instead would make the next run's result depend on a file
        // whose contents say nothing a reader can act on. The siblings that *do* persist are canon
        // (req/38 §21 C-10) — see `ac_042_persists_no_counterexample_of_its_own`.
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let seen = Arc::new(Mutex::new(Stats::default()));
    let tally = Arc::clone(&seen);
    let n = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&n);
    runner
        .run(
            &(config(), prop::collection::vec(event(), 4..36)),
            move |(cfg, trace)| {
                let k = {
                    let mut c = counter.lock().expect("the counter is not poisoned");
                    *c += 1;
                    *c
                };
                let run = run_trace(&format!("ac042_{k}"), cfg, &trace)?;
                let mut stats = tally.lock().expect("the tally is not poisoned");
                stats.traces += 1;
                stats.events += trace.len();
                stats.committed += run.committed;
                stats.record_only_commits += run.record_only_commits;
                stats.comparisons += run.comparisons;
                Ok(())
            },
        )
        .expect("AC-042 holds");

    let stats = seen.lock().expect("the tally is not poisoned");
    println!(
        "AC042_CASES={cases} traces={} events={} committed={} record_only_commits={} \
         model_comparisons={}",
        stats.traces, stats.events, stats.committed, stats.record_only_commits, stats.comparisons
    );
    // A property that never reached the state it is about is a property that passed by vacuity.
    // 34's Then is over "a Transformation that reached `Committed`" (sem: SEM-gx-engine-533),
    // so at least one must exist, and the
    // record-only exception clause needs at least one of its own or the parenthesis is unwalked.
    assert!(
        stats.committed > 0,
        "no generated trace reached `Committed`: AC-042 would be vacuous"
    );
    assert!(
        stats.record_only_commits > 0,
        "no generated trace walked the T-8r exception AC-042's parenthesis names"
    );
}

#[derive(Default)]
struct Stats {
    traces: usize,
    events: usize,
    committed: usize,
    record_only_commits: usize,
    comparisons: usize,
}

/// The model is only worth as much as its ability to be wrong, so here is a road it rejects.
///
/// `inv_s1` is the whole of AC-042's Then, and a probe that only ever fed it real journals would
/// pass whether or not the function said anything. These four roads are the ones 43 §3 does and
/// does not allow into `Canonicalized`.
#[test]
fn ac_042_the_criterion_function_rejects_the_roads_43_forbids() {
    use Lifecycle::{Admitted, Candidate, Canonicalized, Committed, Denied, Escalated, Verifying};

    // T-8, the ordinary road.
    assert!(inv_s1(
        &[Candidate, Verifying, Admitted, Canonicalized, Committed],
        Some(true)
    )
    .is_ok());
    // T-8r, the record-only exception, with the stamp 34's parenthesis requires.
    assert!(inv_s1(
        &[Candidate, Verifying, Denied, Canonicalized, Committed],
        Some(false)
    )
    .is_ok());
    // The same road **without** the stamp: `enforced=false` is part of the clause, not decoration.
    assert!(inv_s1(
        &[Candidate, Verifying, Denied, Canonicalized, Committed],
        Some(true)
    )
    .is_err());
    // A commit with no canonicalization at all.
    assert!(inv_s1(&[Candidate, Verifying, Admitted, Committed], Some(true)).is_err());
    // Canonicalized entered from a state 43 §3 has no edge from.
    assert!(inv_s1(
        &[Candidate, Verifying, Escalated, Canonicalized, Committed],
        Some(true)
    )
    .is_err());
    println!("AC042_CRITERION_NEGATIVES=3");
}

/// **This** suite persists no counterexample — and the siblings that do are canon, not accident.
///
/// # 🔴 The rule this probe first got wrong, and what it is now
///
/// Its first form asserted that `.gitignore` carried `proptest-regressions`, and it was **red on
/// arrival for the wrong reason**. This repository *deliberately* commits `*.proptest-regressions`:
/// req/38 §21 **C-10** ruled "`policy_determinism.proptest-regressions`'s 2 seeds are **kept**...
/// the condition for keeping them is that the provenance (the red-first empty projection) is
/// spelled out in §4 C-10" (sem: SEM-gx-engine-534), and §22 D-10 and §23 E-10 repeat
/// the ruling for two more files. Six are tracked today. A probe that demanded they be ignored
/// would have made a hand-8 preference outrank three rulings — so it was rewritten to the rule that
/// actually holds, and the near-miss is written down rather than quietly dropped (req/38 §21's own
/// sentence: "keeping something without writing down its provenance is the worst thing you can
/// do") (sem: SEM-gx-engine-535).
///
/// What is true of **this** suite is narrower and is a property of its own configuration:
/// [`ac_042_inv_s1_holds_over_every_generated_trace`] sets `failure_persistence: None`, because a
/// counterexample here is a *trace* and is only readable when printed (34 asks for the violating
/// path, and a seed is not one). So the file must not appear, and the count of the siblings is
/// recorded rather than judged.
#[test]
fn ac_042_persists_no_counterexample_of_its_own() {
    let all = persisted(&support::repo_root());
    let mine: Vec<&String> = all
        .iter()
        .filter(|p| p.contains("ac_042.proptest-regressions"))
        .collect();
    assert!(
        mine.is_empty(),
        "this suite sets `failure_persistence: None` and yet a seed file exists: {mine:?}"
    );
    // Record-only, per M3-15's shape: a number in the report, no threshold.
    println!(
        "AC042_OWN_REGRESSION_FILES=0 SIBLING_REGRESSION_FILES={}",
        all.len()
    );
    for p in &all {
        println!("AC042_SIBLING_SEED_FILE {p}");
    }
}

/// Every `*.proptest-regressions` file under `root`, `target/` and `.git/` excepted.
fn persisted(root: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "target" || name == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if name.ends_with(".proptest-regressions") {
                found.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    found.sort_unstable();
    found
}
