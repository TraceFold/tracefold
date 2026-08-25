// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **K-6** — two threads, one critical section (req/38 §35, and §37 §5 row 13).
//!
//! §35 K-6 sent the concurrency case forward with a reason: "concurrency, once the engine layer
//! has a sync hook" (sem: SEM-gx-engine-680). The engine layer exists now, and this suite is
//! what that ruling asked for — "the two-thread CAS race test: the engine's first layer with a
//! hook."
//!
//! # 🔴 The hook is `&mut self`, and that is a finding rather than a shortcut
//!
//! `Engine::commit` takes `&mut self`, so two threads cannot both be inside it: the compiler will
//! not let two `&mut` exist, and any caller who wants two threads has to supply the mutual exclusion
//! itself. v0.1 therefore has **no lock of its own** — the synchronisation is the borrow checker's,
//! and the shape it forces on a caller is exactly one commit at a time per engine.
//!
//! That is enough for 43 §8's per-object serialisation (ASM-2) and it is not enough for M6's
//! `gx serve`, which will want concurrent commits against *different* objects. Whether the engine
//! should own a finer lock is raised as **M5H5-6** rather than decided here.
//!
//! # What the two probes measure
//!
//! 1. **The same transformation, twice** — 43 T-9's idempotency column (a duplicate commit_start
//!    request is ignored if already Committing, sem: SEM-gx-engine-681). One thread does the
//!    work; the other must find it done and write nothing.
//! 2. **Two transformations, one object** — 43 §7's CAS. One wins; the other's `Fingerprint₀` is
//!    stale by the time it reaches T-10a and it is `Aborted(PreconditionChanged)` **without an
//!    apply**. This is AC-034's property arrived at by racing rather than by injection.

mod support;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use gx_core::{AbortReason, Timestamp};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};

use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

/// The one clock, injected (41 §6).
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// 43 T-9's idempotency, under a real race: one `CommittingStarted`, one ledger entry, one apply.
#[test]
fn k6_two_threads_committing_one_transformation_pass_t9_once() {
    let dir = scratch("k6_same");
    let (adapter, counts, world) = CommitAdapter::new("before");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter), "k6-1");

    let intent = intent("/tmp/k6-same", "after");
    engine.submit(&intent, 42, AT).expect("T-1");
    let id = engine.plan(&intent, AT).expect("T-2");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("T-3/T-4a");
    engine.canonicalize(&id, AT, None).expect("T-8");

    let engine = Arc::new(Mutex::new(engine));
    let mut states = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let engine = Arc::clone(&engine);
                scope.spawn(move || {
                    let mut engine = engine.lock().expect("the engine is not poisoned");
                    engine.commit(&id, AT, &signing_key())
                })
            })
            .collect();
        for handle in handles {
            states.push(handle.join().expect("the thread does not panic"));
        }
    });

    let engine = engine.lock().expect("the engine is not poisoned");
    let starts = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.kind() == "CommittingStarted")
        .count();
    let commits = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.kind() == "Committed")
        .count();
    println!(
        "K6_SAME STATES={:?} COMMITTING_STARTED={starts} COMMITTED={commits} \
         APPLY_CALLS={} LEDGER_LEAVES={} WORLD={:?}",
        states,
        counts.apply.load(Ordering::SeqCst),
        engine.ledger().log().len(),
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );

    for state in &states {
        assert_eq!(
            *state.as_ref().expect("neither thread is refused"),
            Lifecycle::Committed,
            "both callers are told the same true thing"
        );
    }
    assert_eq!(
        starts, 1,
        "43 T-9: a duplicate commit_start request is ignored if already Committing (sem: SEM-gx-engine-682)"
    );
    assert_eq!(commits, 1, "one T-11");
    assert_eq!(
        counts.apply.load(Ordering::SeqCst),
        1,
        "Rule 2, once (sem: SEM-gx-engine-683)"
    );
    assert_eq!(engine.ledger().log().len(), 1, "INV-S3");
    assert!(engine.ledger_agrees());
}

/// 43 §7's CAS under a race: two transformations over one object, one winner.
///
/// Both are planned before either commits, so both hold a `Fingerprint₀` of the same world. The
/// first to reach T-10a applies; the second recomputes `Fingerprint₁`, finds the world moved by
/// **somebody else** — which is what T-10a exists to catch — and aborts without applying.
///
/// This is the honest form of the difference hand 4 recorded: AC-034's injection makes the CAS
/// notice a writer the engine does not know about, and this makes it notice a writer the engine
/// *does* know about. Both must abort, and the reason is the same one.
#[test]
fn k6_two_threads_racing_one_object_leave_exactly_one_commit() {
    let dir = scratch("k6_race");
    let (adapter, counts, world) = CommitAdapter::new("before");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter), "k6-2");

    let first = intent("/tmp/k6-race", "first");
    let second = intent("/tmp/k6-race", "second");
    let mut ids = Vec::new();
    for intent in [&first, &second] {
        engine.submit(intent, 42, AT).expect("T-1");
        let id = engine.plan(intent, AT).expect("T-2");
        engine
            .verify(&id, AT, &signing_key(), None)
            .expect("T-3/T-4a");
        engine.canonicalize(&id, AT, None).expect("T-8");
        ids.push(id);
    }

    let engine = Arc::new(Mutex::new(engine));
    let mut states = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .iter()
            .map(|id| {
                let engine = Arc::clone(&engine);
                let id = *id;
                scope.spawn(move || {
                    let mut engine = engine.lock().expect("the engine is not poisoned");
                    engine.commit(&id, AT, &signing_key())
                })
            })
            .collect();
        for handle in handles {
            states.push(handle.join().expect("the thread does not panic"));
        }
    });

    let engine = engine.lock().expect("the engine is not poisoned");
    let outcomes: Vec<Lifecycle> = states
        .iter()
        .map(|s| *s.as_ref().expect("neither thread is refused"))
        .collect();
    let committed = outcomes
        .iter()
        .filter(|s| **s == Lifecycle::Committed)
        .count();
    let aborted = outcomes
        .iter()
        .filter(|s| **s == Lifecycle::Aborted(AbortReason::PreconditionChanged))
        .count();
    println!(
        "K6_RACE OUTCOMES={outcomes:?} COMMITTED={committed} PRECONDITION_CHANGED={aborted} \
         APPLY_CALLS={} LEDGER_LEAVES={} WORLD={:?}",
        counts.apply.load(Ordering::SeqCst),
        engine.ledger().log().len(),
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );

    assert_eq!(committed, 1, "one winner");
    assert_eq!(aborted, 1, "and the loser is told why");
    assert_eq!(
        counts.apply.load(Ordering::SeqCst),
        1,
        "INV-S7: the loser never reached the adapter"
    );
    assert_eq!(
        engine.ledger().log().len(),
        1,
        "INV-S4: an abort is not witnessed"
    );
    assert!(engine.ledger_agrees());
}
