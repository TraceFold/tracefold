// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-031 (FR-031) — `plan` records the delta and `Fingerprint₀`, and the fingerprint survives.
//!
//! 34 AC-031, verbatim (sem: SEM-gx-engine-408): "Given: T in the `Draft` state. When: the
//! `snapshot+plan` step runs. Then: `PlannedDelta` and Fingerprint₀ are recorded and it transitions
//! to the `Candidate` state, and **`Fingerprint₀` can be retrieved back out of the store at the
//! later commit stage**."
//!
//! # The last clause is the criterion
//!
//! The first two are shape. The one that matters is "can be retrieved at the later commit stage"
//! (sem: SEM-gx-engine-409), because
//! `Fingerprint₀` is one half of CON-2's answer: 43 §7 compares it against `Fingerprint₁` at T-10a
//! and 43 INV-S7 forbids `apply` when they differ. A fingerprint that were recomputed at commit
//! time instead of retrieved would compare the substrate against itself and always agree, which is
//! the CAS silently doing nothing -- the single defect this field exists to prevent.
//!
//! So this file measures **retrieval**, not recording: the value comes back out of the engine after
//! other transitions have run, and it is the value the adapter gave at plan time.
//!
//! # M5-22, adopted (b): what "absence" answers (sem: SEM-gx-engine-410)
//!
//! req/38 §37 rules that a missing object is not a state but a refusal the engine reads in context:
//!
//! > **M5-22, adopted (b)**: absence is answered by `snapshot` with `Err(NotFound)`, and the engine
//! > reads it as the normal case **only when it is a creation intent** (this hand's engine-side
//! > counterpart to E-M4-35; document explicitly that "one more place reads `Err` as the normal
//! > case") (sem: SEM-gx-engine-411)
//!
//! The doc line the ruling asks for is
//! [`ac_031_an_absent_object_is_a_refusal_and_this_hand_reads_no_creation_intent`], and the honest
//! statement is that **this hand adds no such place**: it has no notion of a creation intent to
//! read the refusal in the context of, so `snapshot` refusing is `Error::Adapter` and nothing else.
//! The count of places where an `Err` is read as a normal case is still **zero** after hand 2.

mod support;

use std::sync::Arc;

use gx_core::{Fingerprint, SubstrateKind, Timestamp};
use gx_engine::{Engine, EngineJournalRecord, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, StubAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

fn engine(name: &str) -> Engine<InjectedEvidence> {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    engine
}

/// The three things T-2 fixes, and the state it lands in.
#[test]
fn ac_031_plan_fixes_the_delta_the_fingerprint_and_the_state() {
    let i = intent("/tmp/x", "v1");
    let mut e = engine("ac031_shape");
    e.submit(&i, 42, AT).expect("submit");

    assert!(
        e.transformation_ids().is_empty(),
        "M5-17, adopted (b): a draft is not in the table (sem: SEM-gx-engine-412)"
    );
    assert!(e.is_drafted(&gx_core::IntentId(
        gx_canon::cid::compute(&i).expect("canonical")
    )));

    let id = e.plan(&i, AT).expect("plan");

    assert_eq!(e.state(&id), Some(Lifecycle::Candidate));
    let delta = e.planned_delta(&id).expect("the PlannedDelta is recorded");
    assert_eq!(delta.payload(), b"v1", "the delta is the adapter's, unread");
    let fp0 = e
        .precondition_fingerprint(&id)
        .expect("Fingerprint₀ is recorded");
    assert_eq!(fp0.substrate(), &SubstrateKind::Fs);
    assert_eq!(fp0.scope(), "/tmp/x");
}

/// 🔴 The criterion: `Fingerprint₀` comes back **after** later transitions have run.
///
/// `verify` and `canonicalize` are run in between, so what is retrieved is what T-2 stored and not
/// something the last transition happened to leave behind. Hand 4's `commit` is the real caller;
/// this is that call made early, which is the only way to state the criterion before the caller
/// exists.
#[test]
fn ac_031_the_fingerprint_is_retrievable_at_the_later_stage() {
    let i = intent("/tmp/x", "v1");
    let mut e = engine("ac031_retrieve");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");

    let at_plan = e
        .precondition_fingerprint(&id)
        .expect("recorded at T-2")
        .clone();

    e.verify(&id, AT, &signing_key(), None).expect("verify");
    e.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(e.state(&id), Some(Lifecycle::Canonicalized));

    let at_the_later_stage = e
        .precondition_fingerprint(&id)
        .expect("still retrievable where T-10a will want it");
    assert!(
        at_plan
            .cas_eq(at_the_later_stage)
            .expect("the same scope and substrate, so the comparison is defined"),
        "the fingerprint T-10a compares against is the one T-2 recorded"
    );
}

/// The journal carries the same `Fingerprint₀`, through the checked constructor.
///
/// The row above is memory and hand 3's blob store is not built yet, so the journal is the only
/// durable copy. `FingerprintRecord` is the mirror hand 1 introduced (**M5H1-4**) because
/// `Fingerprint` has no serde face, and "reading it back requires the checked constructor"
/// (E-6; sem: SEM-gx-engine-413) is why the way
/// back is `Fingerprint::new` -- which is what this probe exercises.
#[test]
fn ac_031_the_journal_carries_the_fingerprint_and_hands_it_back_checked() {
    let i = intent("/tmp/x", "v1");
    let mut e = engine("ac031_journal");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");

    let planned = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Planned {
                transformation,
                fp0,
                delta_cid,
                ..
            } if *transformation == id => Some((fp0.clone(), *delta_cid)),
            _ => None,
        })
        .expect("T-2 wrote a Planned record");

    let (fp0_record, delta_cid) = planned;
    let round_tripped: Fingerprint = fp0_record
        .into_fingerprint()
        .expect("a scope this short is inside MAX_SCOPE_BYTES");
    assert!(round_tripped
        .cas_eq(e.precondition_fingerprint(&id).expect("in the row"))
        .expect("defined"));
    assert_eq!(
        delta_cid,
        e.planned_delta(&id).expect("in the row").reference().cid,
        "the journal names the delta the row holds"
    );
}

/// 🔴 **M5-22, adopted (b)**'s doc line: this hand reads no `Err` as a normal case
/// (sem: SEM-gx-engine-414).
///
/// The ruling permits exactly one such place -- `snapshot` answering `Err(NotFound)` when the intent
/// is a *creation* -- and requires the addition to be written down. Nothing is added: hand 2 has no
/// creation intent to condition on (42 §3.3's `Intent` carries a `goal` an adapter interprets, and
/// no field says "this object does not exist yet" (sem: SEM-gx-engine-415)), so an adapter that cannot read the object is an
/// `Error::Adapter` and the transformation does not exist. The count stays **zero**, and a hand that
/// raises it owes this comment an edit.
#[test]
fn ac_031_an_absent_object_is_a_refusal_and_this_hand_reads_no_creation_intent() {
    /// An adapter whose `snapshot` always refuses, the way E-M4-35 says an fs adapter reports
    /// "does not exist" (sem: SEM-gx-engine-416).
    #[derive(Debug)]
    struct Absent;

    impl gx_substrate::SubstrateAdapter for Absent {
        fn kind(&self) -> SubstrateKind {
            SubstrateKind::Fs
        }
        fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
            Err(gx_substrate::Error::Unreadable {
                locator: locator.to_string(),
                detail: "NotFound".to_string(),
            })
        }
        fn plan(
            &self,
            _i: &gx_core::Intent,
            _p: &gx_core::ObjectSnapshot,
        ) -> gx_substrate::Result<gx_substrate::PlannedDelta> {
            unreachable!("snapshot refuses first")
        }
        fn precondition(
            &self,
            _s: &gx_core::ObjectSnapshot,
        ) -> gx_substrate::Result<gx_core::Fingerprint> {
            unreachable!("snapshot refuses first")
        }
        fn apply(
            &self,
            _d: &gx_substrate::PlannedDelta,
        ) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
            unreachable!("hand 2 never applies")
        }
        fn invert(
            &self,
            _d: &gx_substrate::PlannedDelta,
            _p: &gx_core::ObjectSnapshot,
        ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
            unreachable!("snapshot refuses first")
        }
        fn commutation(
            &self,
            _a: &gx_substrate::PlannedDelta,
            _b: &gx_substrate::PlannedDelta,
        ) -> gx_substrate::Result<gx_core::Commutation> {
            unreachable!("snapshot refuses first")
        }
    }

    let dir = scratch("ac031_absent");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    e.register_adapter(Arc::new(Absent), "stub-1");

    let i = intent("/tmp/does-not-exist", "v1");
    e.submit(&i, 42, AT)
        .expect("submit does not touch a substrate");
    let refused = e
        .plan(&i, AT)
        .expect_err("snapshot refused, so there is no candidate");
    assert_eq!(refused.kind(), "Adapter", "{refused}");
    assert!(
        e.transformation_ids().is_empty(),
        "a refused plan leaves no row, so nothing downstream can find a half-built candidate"
    );
    println!("ERR_READ_AS_NORMAL_CASE=0");
}
