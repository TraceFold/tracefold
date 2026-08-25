// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-4627-03 / E-DR4627-1** — the `at` handed to `Engine::verify` is the `at` an invariant sees.
//!
//! # Why the gate-side suite cannot answer this
//!
//! `crates/gx-gate/tests/decided_at_seat.rs` proves that `GateInput.decided_at` reaches a
//! registered invariant and that no verdict derives from it. What it cannot prove is that anything
//! **puts a real moment there**: every `GateInput` in that file is built by the test itself. A
//! `pipeline.rs` that passed `Timestamp(0)`, or `t.created_at`, or a clock it read locally would
//! leave every assertion in that file green while delivering a field that carries the wrong time —
//! which is the failure `req/447` §1 found in the first place. Plan time was *already* reachable
//! through `t.created_at`; a `decided_at` that also carries plan time is the old reachability
//! wearing a new name, and DR-46-27 would have shipped nothing.
//!
//! So the subject here is the **one production construction site** — `crates/gx-engine/src/
//! pipeline.rs`, the only `GateInput { .. }` in any `src/` outside `gx-gate`'s own pack runner —
//! and the instrument is a deployment-shaped one: an invariant registered on the gate the engine
//! was opened with, recording what it was handed.
//!
//! # The discriminator
//!
//! Three moments, deliberately distinct, and 41 §6's "randomness and time are injected at the
//! engine boundary" is what makes distinguishing them possible at all:
//!
//! | given to | value |
//! |:--|:--|
//! | `submit` / `plan` (T-1, T-2) | [`PLAN_AT`] |
//! | `verify` (T-4) | [`VERIFY_AT`], an hour later (inside ASM-12's 24h `verify_ttl`) |
//!
//! `t.created_at` becomes `PLAN_AT`, because that is when the transformation was composed. If the
//! invariant sees `PLAN_AT` the wire is wrong in the specific way that matters; if it sees
//! `VERIFY_AT` the wire is right. Asserting both — equal to one, unequal to the other — is what
//! makes this a measurement rather than a coincidence: a hard-coded constant would have to be
//! `VERIFY_AT` by name to pass, and there is no reason for `pipeline.rs` to contain that.

mod support;

use std::sync::{Arc, Mutex};

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, StubAdapter, PERMIT_ALL};

/// When the transformation was composed — `submit` and `plan`, so `t.created_at`.
const PLAN_AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// When the gate is asked: **one hour** after [`PLAN_AT`], and the hour is not arbitrary.
///
/// The two moments must differ, or the assertions below cannot tell the right wire from the wrong
/// one. They must also differ by **less than ASM-12's `verify_ttl`** (24 hours, 33 NFR-028,
/// `DEFAULT_VERIFY_TTL_NANOS`): a `Candidate` that has sat longer than that is `Aborted(Expired)`
/// before `verify` looks at it, and the call comes back `InvalidState` without any gate having been
/// asked anything. The first draft of this test used a full day and measured exactly that -- a
/// reminder that on this road "the gate was not asked" and "the gate was asked and answered" are
/// distinguishable only if the fixture stays inside the TTL.
const VERIFY_AT: Timestamp = Timestamp(1_754_003_600_000_000_000);

/// An invariant that records `decided_at` and `t.created_at` together, and decides by neither.
///
/// The pair is recorded rather than just the first, because "the field carries the right value"
/// and "the field carries a value different from the one that was already reachable" are two
/// claims and only the pair supports both. It answers `holds: true` always — this file measures a
/// wire, and an invariant that could refuse would put a second reason in the verdict and make
/// `Lifecycle::Admitted` below ambiguous.
struct RecordsBothMoments {
    seen: Arc<Mutex<Vec<(Timestamp, Timestamp)>>>,
}

impl gx_gate::InvariantCheck for RecordsBothMoments {
    fn id(&self) -> &str {
        "dr-46-27-wire-probe"
    }

    fn check(&self, input: &gx_gate::GateInput<'_>) -> gx_gate::Result<gx_gate::InvariantResult> {
        self.seen
            .lock()
            .expect("single-threaded")
            .push((input.decided_at, input.t.created_at));
        gx_gate::InvariantResult::new(
            "dr-46-27-wire-probe".to_string(),
            true,
            Some("records the two moments and decides by neither".to_string()),
        )
    }
}

/// 🔴 `Engine::verify(id, at, ..)`'s `at` is what arrives as `GateInput.decided_at`.
#[test]
fn the_engine_hands_the_gate_the_moment_it_was_asked() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut registry = gx_gate::InvariantRegistry::new();
    registry
        .register(Box::new(RecordsBothMoments {
            seen: Arc::clone(&seen),
        }))
        .expect("one invariant, one id");

    let dir = scratch("decided_at_wire");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL).with_invariants(registry),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "decided-at-wire-1");

    let i = intent("/tmp/decided-at-wire.txt", "v1");
    engine.submit(&i, 42, PLAN_AT).expect("T-1");
    let id = engine.plan(&i, PLAN_AT).expect("T-2");
    let state = engine
        .verify(&id, VERIFY_AT, &signing_key(), None)
        .expect("T-4a");

    let observed = seen.lock().expect("single-threaded").clone();
    println!("DECIDED_AT_WIRE state={state:?} observed={observed:?} plan={PLAN_AT:?} verify={VERIFY_AT:?}");

    assert_eq!(
        state,
        Lifecycle::Admitted,
        "the fixture must reach the gate for this to measure the gate's input at all"
    );
    assert_eq!(
        observed.len(),
        1,
        "one `verify` calls each registered invariant exactly once (AC-026); a different count \
         makes the single reading below meaningless"
    );

    let (decided_at, created_at) = observed[0];
    assert_eq!(
        decided_at, VERIFY_AT,
        "`pipeline.rs` did not pass `Engine::verify`'s `at` into `GateInput.decided_at`. This is \
         the one production construction site, so the field is a seat with nothing sitting in it"
    );
    assert_eq!(
        created_at, PLAN_AT,
        "`t.created_at` is not plan time in this fixture, so the inequality below would not \
         discriminate between the two clocks"
    );
    assert_ne!(
        decided_at, created_at,
        "`decided_at` carries plan time. That is the defect `req/447` §1 measured -- the only \
         clock reaching a gate was `t.created_at` -- and a field that re-delivers it has added a \
         name and no reachability: a `now ∈ [a, b]` invariant would still be unwritable"
    );
}
