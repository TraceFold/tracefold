// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **WM-5a Phase 1** (`req/1011` §4, ruled by `req/1016`) — the production `plan` promises a
//! post-state, and the promise is a function of the goal.
//!
//! `tests/ac_049.rs::the_promised_target_and_the_observed_digest_are_reached_by_different_roads`
//! already measured that the two roads agree, but it built the first road **itself**: the
//! goal-only digest was computed in the test body, and `PlannedDelta::promised_target` stayed
//! `None` in every production plan this workspace shipped. `req/1016` §5 found that by reading
//! `PlannedDelta::new` rather than trusting `req/1011` §1's summary, and corrected it: what was
//! free was the *arithmetic*, not the field.
//!
//! This file is the other half. It asks the adapter — not the fixture, not the test — what it
//! promises, and then holds it to the answer.
//!
//! | probe | what it fixes |
//! |---|---|
//! | [`the_production_plan_fills_the_prophecy_seat`] | the seat is `Some`, and it is the goal's digest |
//! | [`the_promise_and_the_observation_are_reached_by_different_roads`] | apply lands on exactly what plan promised |
//! | [`the_promise_is_a_function_of_the_goal_and_not_a_constant`] | the negative control — a different goal promises a different digest |
//! | [`planning_still_touches_no_filesystem`] | the promise did not cost the zero-I/O property |
//!
//! # What these cannot catch
//!
//! Both the promise and the observation end in the one digest function 41 §6 admits, so a bug
//! *inside* `cid::mint` is invisible here, exactly as `ac_049.rs` records. What they do catch is
//! everything between the goal and the bytes on the disk — and, new to this lane, an adapter that
//! stopped promising at all.

mod support;

use gx_adapter_fs::FsAdapter;
use gx_canon::cid::{self, Domain};
use gx_substrate::SubstrateAdapter;
use support::{intent_for, snapshot_of, Sandbox, GOAL, SUBJECT};

/// A goal that is not [`GOAL`], for the negative control.
const OTHER_GOAL: &[u8] = b"a different world";

/// 🔴 The lane's whole claim: the seat is filled on the ordinary road.
///
/// Before this lane the assertion below was false for all six shipped adapters, which is why
/// `req/1010` §4c could use "the seat is empty" as its negative control and call it "the road
/// every shipped adapter takes". That sentence is now about mcp/postgres/mysql only.
#[test]
fn the_production_plan_fills_the_prophecy_seat() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let pre = snapshot_of(&adapter, &locator);
    let delta = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans a whole-file replacement");

    let expected = cid::mint(Domain::Leaf, &[GOAL]);
    println!(
        "WM5A_FS_PROMISED={:?} EXPECTED={}",
        delta.promised_target().map(|c| cid::to_text(&c)),
        cid::to_text(&expected)
    );
    assert_eq!(
        delta.promised_target(),
        Some(expected),
        "🔴 WM-5a: `plan` promised nothing, or promised something other than the goal's digest. \
         The prophecy is derivable from the goal alone (E-M4-29's own sentence), so a `None` here \
         is a prediction withheld rather than a prediction that cannot be made"
    );
}

/// The dual-run: what was promised is what the world became.
///
/// `ac_049.rs` states this with the test supplying route 1; here route 1 is the **adapter's own**
/// answer, so a plan that promised a digest its apply never produces fails here and not only under
/// a fixture that was written to agree with it.
#[test]
fn the_promise_and_the_observation_are_reached_by_different_roads() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let pre = snapshot_of(&adapter, &locator);
    let delta = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans");
    let promised = delta.promised_target().expect("the seat is filled");
    let applied = adapter.apply(&delta).expect("the delta applies");

    println!(
        "WM5A_FS_DUAL_RUN promised={} observed={} agree={}",
        cid::to_text(&promised),
        cid::to_text(applied.resulting_digest()),
        applied.resulting_digest() == &promised
    );
    assert_eq!(
        applied.resulting_digest(),
        &promised,
        "the adapter promised one post-state and produced another (L5, M4-06, adopted (b)) — this \
         is the failure `AbortReason::PostconditionMismatch` exists for, seen one layer below the \
         engine"
    );
}

/// 🔴 The negative control: the promise is derived, not a constant.
///
/// A `promised_target` hard-wired to one value would pass both probes above whenever the fixture
/// happened to use that goal. Two goals, two promises, and neither is the other's.
#[test]
fn the_promise_is_a_function_of_the_goal_and_not_a_constant() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let pre = snapshot_of(&adapter, &locator);

    let one = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans the first goal")
        .promised_target()
        .expect("the seat is filled");
    let other = adapter
        .plan(&intent_for(&locator, OTHER_GOAL), &pre)
        .expect("the adapter plans the second goal")
        .promised_target()
        .expect("the seat is filled");

    println!(
        "WM5A_FS_DERIVED one={} other={} distinct={}",
        cid::to_text(&one),
        cid::to_text(&other),
        one != other
    );
    assert_ne!(
        one, other,
        "two different goals promised the same post-state, so the promise is not being computed \
         from the goal — a constant that happens to be right for one fixture is worse than no \
         prediction, because the engine would compare against it"
    );
    assert_eq!(
        other,
        cid::mint(Domain::Leaf, &[OTHER_GOAL]),
        "and the second promise is the second goal's digest rather than merely a different number"
    );
}

/// The promise cost no I/O, which is what made it free.
///
/// `tests/plan_purity.rs` fixes this as a scan of the source; this is the behavioural half for the
/// one line this lane added — a `plan` against a position that **does not exist** still promises,
/// because the answer never depended on the position.
#[test]
fn planning_still_touches_no_filesystem() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let absent = sandbox.locator("no-such-file");
    let present = sandbox.locator(SUBJECT);

    // A snapshot of a position that exists, used to plan against one that does not: `pre` is
    // unused by this adapter's `plan` (the module header says so), and the promise is the proof.
    let pre = snapshot_of(&adapter, &present);
    let delta = adapter
        .plan(&intent_for(&absent, GOAL), &pre)
        .expect("a plan for an absent position is still a plan");

    println!(
        "WM5A_FS_NO_IO absent={absent:?} promised={:?}",
        delta.promised_target().map(|c| cid::to_text(&c))
    );
    assert_eq!(
        delta.promised_target(),
        Some(cid::mint(Domain::Leaf, &[GOAL])),
        "the promise changed when the position did, so it is being read off the filesystem rather \
         than derived from the goal — which would make `plan` an I/O call and break E-M4-29"
    );
}
