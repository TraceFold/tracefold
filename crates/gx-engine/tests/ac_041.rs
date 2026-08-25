// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-041** — a `Deny` under `Enforce` never reaches `apply` (FR-027, FR-036, INV-S1).
//!
//! 34 AC-041, verbatim:
//!
//! > Given: `EnforcementMode::Enforce` (default). When: `Gate::verify` returns Deny for an
//! > arbitrary Candidate T. Then: T stays at the terminal `Denied` state, and none of
//! > `canonicalize` (T-8r), `commit_start`, or `adapter.apply` is ever called (mock call count = 0).
//! > Verify the property `mode=Enforce ∧ verdict=Deny ⇒ status=Denied ∧ apply_called=false` over
//! > every generated case. (sem: SEM-gx-engine-516)
//!
//! # Three refusals, not one
//!
//! The criterion names three things that must not happen, and they fail differently:
//! `canonicalize` must **refuse** (43 §1 makes `Denied` terminal outside record-only), `commit`
//! must refuse for the same reason, and `adapter.apply` must not be reached — which is a **count**
//! rather than a return value. So each run reads all three: the two `Err`s and the counter.
//!
//! `AC-035` measured "apply is not called from a non-`Committing` state" (sem:
//! SEM-gx-engine-517) in general; this measures
//! the one road a policy refusal takes, which is the road FR-027 exists for.

mod support;

use std::sync::Arc;

use gx_core::{EnforcementMode, Timestamp, VerdictKind};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use proptest::prelude::*;
use support::{gate, intent, scratch, signing_key, CommitAdapter, FORBID_ETC};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// What one denied transformation left behind.
#[derive(Debug)]
struct Denied {
    state: Lifecycle,
    verdict: Option<VerdictKind>,
    canonicalize_refused: Option<String>,
    commit_refused: Option<String>,
    applies: usize,
    leaves: u64,
    committing_started: usize,
    enforced: Option<bool>,
    verdict_receipts: usize,
}

fn deny_once(name: &str, seed: u64) -> Denied {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(FORBID_ETC),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens")
    // Explicit rather than defaulted: the criterion's Given is the mode, and a probe that relied on
    // the default would stop measuring the day the default moved.
    .with_mode(EnforcementMode::Enforce);
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent(&format!("/etc/forbidden-{seed}.txt"), "after");
    engine.submit(&i, seed, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let state = engine.verify(&id, AT, &signing_key(), None).expect("T-4b");

    let canonicalize_refused = engine
        .canonicalize(&id, AT, None)
        .err()
        .map(|e| e.kind().to_string());
    let commit_refused = engine
        .commit(&id, AT, &signing_key())
        .err()
        .map(|e| e.kind().to_string());

    let committing_started = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.kind() == "CommittingStarted")
        .count();
    // The world is read as well as the counter: a counter that was never incremented and a
    // substrate that changed anyway would be the one failure a call count cannot see.
    assert_eq!(
        &*world.lock().expect("the world is not poisoned"),
        b"before",
        "the substrate moved for a transformation that was denied"
    );

    Denied {
        state,
        verdict: engine.verdict(&id),
        canonicalize_refused,
        commit_refused,
        applies: counts.totals()[4],
        leaves: engine.ledger().log().len(),
        committing_started,
        enforced: engine.enforced(&id),
        verdict_receipts: engine.verdict_receipts(&id).len(),
    }
}

/// 🔴 AC-041, in one run, with every one of the three refusals read.
#[test]
fn ac_041_a_denial_under_enforce_stops_at_denied() {
    let out = deny_once("ac041_one", 7);
    println!(
        "AC041 state={:?} verdict={:?} canonicalize={:?} commit={:?} applies={} leaves={} \
         committing_started={} enforced={:?} verdict_receipts={}",
        out.state,
        out.verdict,
        out.canonicalize_refused,
        out.commit_refused,
        out.applies,
        out.leaves,
        out.committing_started,
        out.enforced,
        out.verdict_receipts
    );
    assert_eq!(out.state, Lifecycle::Denied);
    assert_eq!(out.verdict, Some(VerdictKind::Deny));
    assert_eq!(
        out.canonicalize_refused.as_deref(),
        Some("InvalidState"),
        "43 T-8r does not fire under `Enforce`, so T-8 has no from-state here"
    );
    assert_eq!(
        out.commit_refused.as_deref(),
        Some("InvalidState"),
        "and `commit_start` has none either"
    );
    assert_eq!(
        out.applies, 0,
        "\"mock call count = 0\" (sem: SEM-gx-engine-518)"
    );
    assert_eq!(
        out.leaves, 0,
        "INV-S4: a `Denied` does not appear in the ledger"
    );
    assert_eq!(
        out.committing_started, 0,
        "T-9 writes `CommittingStarted` before anything; the record is the proof it never ran"
    );
    assert_eq!(
        out.enforced,
        Some(true),
        "nothing degraded this one: it was refused and stayed refused"
    );
    // The one thing that **is** issued: ASM-14's verdict receipt for the `Deny` (42 §3.10:
    // "issued for every `Verdict` = Admit/Deny/Escalate", sem: SEM-gx-engine-519). A denial
    // nobody can prove happened is a denial an operator cannot audit.
    assert_eq!(out.verdict_receipts, 1, "M5H4-6: T-4b issues one");
}

/// 🔴 The property AC-041 names, over generated cases (**M5-15, adopted (b)**; sem: SEM-gx-engine-520).
///
/// The generator varies the seed, which varies the locator, the `IntentId`, the
/// `TransformationId` and the delta CID. What it cannot vary is the mode or the verdict, because
/// the criterion fixes both in its Given — so "every generated case" (sem: SEM-gx-engine-521)
/// is over the transformations, not
/// over the quadrants (those are AC-037's grid).
#[test]
fn ac_041_the_property_holds_for_every_generated_case() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: 24,
        ..ProptestConfig::default()
    });
    runner
        .run(&(1u64..100_000), |seed| {
            let out = deny_once(&format!("ac041_prop_{seed}"), seed);
            prop_assert_eq!(out.verdict, Some(VerdictKind::Deny));
            prop_assert_eq!(out.state, Lifecycle::Denied);
            prop_assert_eq!(out.applies, 0);
            prop_assert_eq!(out.leaves, 0);
            prop_assert_eq!(out.committing_started, 0);
            Ok(())
        })
        .expect("the property holds");
    println!("AC041_PROPERTY_CASES=24");
}
