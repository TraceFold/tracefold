// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **WM-5a Phase 1** (`req/1011` §4, ruled by `req/1016`) — the production `plan` promises the
//! entry's post-state, and the promise is the goal's digest by the crate's own function.
//!
//! `req/1016` §1 classified this adapter `digest-sufficient` **with a scope limit written into the
//! same row**, and the limit is repeated here because a test that quietly widened it would be the
//! more dangerous artefact: what is promised is `repo::content_digest` of the **entry's bytes**.
//! Not the commit id, not the tree hash. `apply`'s own observation is that same value
//! (`src/apply.rs`'s `observe`), which is why the promise can be made at all without opening the
//! repository.
//!
//! | probe | what it fixes |
//! |---|---|
//! | [`the_production_plan_fills_the_prophecy_seat`] | the seat is `Some`, and it is `repo::content_digest` of the goal |
//! | [`the_promise_and_the_observation_are_reached_by_different_roads`] | the entry apply wrote digests to what plan promised |
//! | [`the_promise_is_a_function_of_the_goal_and_not_a_constant`] | the negative control — a different goal promises a different digest |
//!
//! # 🔴 Why the first probe names `repo::content_digest`
//!
//! `src/plan.rs` may not: `tests/git_plan_purity.rs` bans the whole `repo::` boundary from that
//! module, so the plan spells the mint out. Two call sites of one function is a smaller risk than
//! a red purity gate, but it is not *no* risk — so this probe is the pin. If somebody changes what
//! a git entry's digest is, the plan's promise and the repository's answer part company here
//! rather than in production.

mod support;

use gx_adapter_git::repo;
use gx_canon::cid;
use gx_substrate::SubstrateAdapter;
use support::{intent_for, GitFixture, BRANCH, GOAL};

/// A goal that is not [`GOAL`], for the negative control.
const OTHER_GOAL: &[u8] = b"a different world\n";

/// 🔴 The lane's claim, and the pin between the two call sites of the one digest function.
#[test]
fn the_production_plan_fills_the_prophecy_seat() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);

    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let delta = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans");

    let expected = repo::content_digest(GOAL);
    println!(
        "WM5A_GIT_PROMISED={:?} EXPECTED={}",
        delta.promised_target().map(|c| cid::to_text(&c)),
        cid::to_text(&expected)
    );
    assert_eq!(
        delta.promised_target(),
        Some(expected),
        "🔴 WM-5a: `plan` promised nothing, or promised a value that is not this crate's own \
         `repo::content_digest` of the goal. The plan spells the mint out because the purity gate \
         bans the `repo::` boundary from that module — this assertion is the only thing keeping \
         the two spellings the same function"
    );
}

/// The dual-run: the entry `apply` left behind digests to what `plan` promised.
///
/// The scope limit of the promise is visible in what is *not* asserted: nothing here says the
/// commit id or the tree hash was predicted, because they were not.
#[test]
fn the_promise_and_the_observation_are_reached_by_different_roads() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);

    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let delta = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans");
    let promised = delta.promised_target().expect("the seat is filled");
    let applied = adapter.apply(&delta).expect("the delta applies");

    println!(
        "WM5A_GIT_DUAL_RUN promised={} observed={} agree={}",
        cid::to_text(&promised),
        cid::to_text(applied.resulting_digest()),
        applied.resulting_digest() == &promised
    );
    assert_eq!(
        applied.resulting_digest(),
        &promised,
        "the adapter promised one post-state and the object database holds another (L5, M4-06, \
         adopted (b)): a tree written without the entry, or a commit that never became the \
         branch's tip, both land here"
    );
}

/// 🔴 The negative control: the promise is derived, not a constant.
#[test]
fn the_promise_is_a_function_of_the_goal_and_not_a_constant() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);
    let pre = adapter.snapshot(&locator).expect("the entry is there");

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
        "WM5A_GIT_DERIVED one={} other={} distinct={}",
        cid::to_text(&one),
        cid::to_text(&other),
        one != other
    );
    assert_ne!(
        one, other,
        "two different goals promised the same post-state, so the promise is not being computed \
         from the goal"
    );
    assert_eq!(
        other,
        repo::content_digest(OTHER_GOAL),
        "and the second promise is the second goal's digest rather than merely a different number"
    );
}
