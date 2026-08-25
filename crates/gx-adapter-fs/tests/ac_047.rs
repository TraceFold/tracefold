// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-047**: two consecutive plans agree, and the substrate does not move.
//!
//! 34, verbatim: "Given: intent I. When: `adapter.plan(I)` is called twice in a row against one adapter instance.
//! Then: the same `PlannedDelta` is returned, and the substrate state (file mtime/content etc.) does not change before and after the call."
//! (judgment method: `unit + property`.) FR-042 is what it measures: "`plan` MUST be a pure function
//! (no side effects)". (sem: SEM-gx-adapter-fs-098)
//!
//! # Three rulings decide how this is written
//!
//! * **E-M4-4** put the pre-state in the signature -- `plan(&self, intent, pre)` -- because 32
//!   FR-042 says "the same `PlannedDelta` from the same intent" without saying against what and 43 T-2 says
//!   "**against the same snapshot**". So "the same intent" here means the pair. (sem: SEM-gx-adapter-fs-099)
//! * **M4-24** fixes what "does not change" is measured with: "content digest + mtime + size; atime
//!   is not measured (it is fs-dependent and unstable -- do not pretend to measure what cannot be measured)". `support::state_of` is that measurement and (sem: SEM-gx-adapter-fs-100)
//!   its documentation carries the reason atime is absent.
//! * **E-M4-29** adds the fs-specific half: "for the fs adapter v0.1, `plan` achieving zero I/O holds", which is
//!   stronger than "no side effects" and is measured separately in `tests/plan_purity.rs`. (sem: SEM-gx-adapter-fs-101)
//!
//! # The control
//!
//! req/69 §8.2: "AC-047's 'does not change' only becomes a claim once it has a control that goes RED when a change is injected".
//! [`the_measurement_moves_when_the_substrate_moves`] is that control: the same three numbers, taken
//! either side of a write, disagree. Without it "the state did not change" could be said by a (sem: SEM-gx-adapter-fs-102)
//! measurement that cannot see change at all.
//!
//! Everything here writes to a tmpfs; `support` says why and proves the filesystem type.

mod support;

use gx_adapter_fs::FsAdapter;
use gx_substrate::SubstrateAdapter;
use support::{intent_for, state_of, FsFixture, Sandbox, GOAL, SUBJECT};

/// The acceptance criterion itself.
#[test]
fn ac_047_two_consecutive_plans_agree_and_the_substrate_does_not_move() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let path = sandbox.dir().join(SUBJECT);

    println!(
        "AC_047_ROOT={} FS={}",
        sandbox.dir().display(),
        support::filesystem_of(sandbox.dir())
    );

    let intent = intent_for(&locator, GOAL);
    let pre = adapter
        .snapshot(&locator)
        .expect("the sandbox holds the subject");

    let before = state_of(&path);
    let first = adapter.plan(&intent, &pre).expect("the adapter plans");
    let second = adapter
        .plan(&intent, &pre)
        .expect("the adapter plans again");
    let after = state_of(&path);

    assert_eq!(
        first, second,
        "two consecutive plans over one (intent, snapshot) returned different deltas (FR-042, \
         E-M4-4)"
    );
    assert_eq!(
        before, after,
        "planning moved the substrate: 34 AC-047 asks that the substrate state (file\
         mtime/content etc.) not change before and after the call, measured as M4-24 fixed it (content digest + mtime + size) (sem: SEM-gx-adapter-fs-103)"
    );
    println!(
        "AC_047_DELTA_REFERENCE={:?} PAYLOAD_BYTES={}",
        first.reference().cid,
        first.payload().len()
    );
}

/// The control: the measurement can see a change, so its silence above means something.
#[test]
fn the_measurement_moves_when_the_substrate_moves() {
    let sandbox = Sandbox::new();
    let path = sandbox.dir().join(SUBJECT);

    let before = state_of(&path);
    sandbox.write(SUBJECT, b"a different length entirely");
    let after = state_of(&path);

    assert_ne!(
        before.digest, after.digest,
        "the content digest did not move when the content did"
    );
    assert_ne!(before.size, after.size, "the size did not move");
    // mtime is deliberately not asserted to differ: tmpfs timestamps have a resolution, and two
    // writes inside one tick share one mtime. A control that asserted it would be flaky about the
    // clock rather than strict about the state -- which is the same reason M4-24 leaves atime out.
}

/// The pair, not the intent alone: a different pre-state is allowed to plan a different delta.
///
/// This is the half of E-M4-4 that a determinism test can accidentally forbid. FR-042 read without
/// 43 T-2 would say two plans of one intent must agree **always**, and an adapter whose delta
/// depended on the pre-state would then be wrong for depending on its argument.
///
/// For this adapter, v0.1's whole-file replacement is a function of the intent alone, so the two
/// deltas do agree -- and that is recorded here as a *fact about this adapter* rather than as the
/// law, because L1 (the law) is quantified over the pair and lives in the harness.
#[test]
fn the_quantifier_is_the_pair_and_this_adapter_ignores_the_pre_state() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let intent = intent_for(&locator, GOAL);

    let early = adapter.snapshot(&locator).expect("a snapshot");
    sandbox.write(SUBJECT, b"and now something else");
    let late = adapter.snapshot(&locator).expect("a second snapshot");
    assert_ne!(early.digest(), late.digest(), "the snapshots differ");

    let from_early = adapter.plan(&intent, &early).expect("plan");
    let from_late = adapter.plan(&intent, &late).expect("plan");
    assert_eq!(
        from_early, from_late,
        "v0.1 replaces the whole file, so the delta is a function of the goal; if this ever stops \
         being true, L1 in the harness is the law that still holds and this probe is the one to \
         rewrite"
    );
}

/// Planning twice through the fixture the harness uses, so the two roads agree.
#[test]
fn the_fixture_the_harness_uses_plans_the_same_delta_twice() {
    use gx_substrate_conformance::Fixture;

    let fixture = FsFixture::new();
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let pre = adapter.snapshot(&locator).expect("a snapshot");
    let intent = fixture.intent();

    assert_eq!(
        adapter.plan(&intent, &pre).expect("plan"),
        adapter.plan(&intent, &pre).expect("plan"),
    );
}
