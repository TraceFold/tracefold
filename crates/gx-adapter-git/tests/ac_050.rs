// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-050**, both cases, measured through gitoxide rather than through the adapter.
//!
//! 34, verbatim: "Given: git-adapter with a **commit-operation delta** and a **branch-operation delta**. When: `apply`->`invert`->
//! `apply(inverse)`. Then: the repo's **HEAD and tree hash** return to before the operation, **bit-equal** (2 cases)."
//! Judgment method: "integration (a temporary repo generated with gix)". (sem: SEM-gx-adapter-git-092)
//!
//! # 🔴 Why `bit-equal` decides the design of `invert`
//!
//! A revert commit restores the *tree* and moves HEAD **forward**, so it satisfies half of this
//! sentence and fails the other half. Only putting the reference back where it was returns both. That
//! is the whole reason [`gx_adapter_git::invert`] produces a `reset` operation, and this file is where
//! the reason is checked rather than argued.
//!
//! # The two numbers come from git, not from gx
//!
//! `Sandbox::head_and_tree` reads `HEAD` and the commit's `tree` id through gitoxide. The adapter's
//! own digests are `gx-canon` blake3 over an entry's bytes and would answer a different question; an
//! assertion built on them would be asking the adapter whether it was right. The two routes are
//! deliberately in different hash functions.
//!
//! # What comes back and what does not
//!
//! The reference comes back. The **objects do not go away**: the commit the forward delta made is
//! still in the database and is now unreachable, and `git reflog` shows both moves. AC-050 asks about
//! HEAD and the tree hash and this is exactly what it gets; the crate root states the residue where a
//! reader of the disclosure will find it, and this note is the same sentence at the point of
//! measurement.

mod support;

use gx_substrate::SubstrateAdapter;
use support::{planned, reset_to, GitFixture, BRANCH, GOAL, SUBJECT};

/// **AC-050 case 1** -- "commit-operation delta": an entry change, undone. (sem: SEM-gx-adapter-git-093)
#[test]
fn a_commit_operation_returns_head_and_the_tree_hash_bit_equal() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();
    let locator = sandbox.locator_on(BRANCH);

    let (head_before, tree_before) = sandbox.head_and_tree();
    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let delta = planned(adapter, &locator, GOAL);

    // ORDER=T-10b (E-M4-30): the inverse is constructed while the pre-state is still live.
    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("a branch with a commit on it has an inverse");

    adapter.apply(&delta).expect("the entry change applies");
    let (head_after, tree_after) = sandbox.head_and_tree();
    println!("AC050_CASE1_FORWARD head {head_before} -> {head_after}  tree {tree_before} -> {tree_after}");
    assert_ne!(
        head_after, head_before,
        "a commit operation that did not move HEAD measured nothing afterwards"
    );
    assert_ne!(
        tree_after, tree_before,
        "a commit operation that did not move the tree hash measured nothing afterwards"
    );

    adapter.apply(&inverse).expect("the inverse applies");
    let (head_back, tree_back) = sandbox.head_and_tree();
    println!("AC050_CASE1_BACK head={head_back} tree={tree_back} ORDER=T-10b");
    assert_eq!(head_back, head_before, "AC-050: HEAD is bit-equal");
    assert_eq!(tree_back, tree_before, "AC-050: the tree hash is bit-equal");
}

/// **AC-050 case 2** -- "branch-operation delta": a reference reset, undone. (sem: SEM-gx-adapter-git-094)
///
/// The delta is written in the grammar rather than planned, for the division §32 M4H4-5 confirmed fixed:
/// 42 §3.3 gives an [`gx_core::Intent`] a goal and no spelling for "put this branch there", so a (sem: SEM-gx-adapter-git-095)
/// branch operation is a payload a caller constructs. The fixture's second branch supplies a commit
/// that is not on `main`'s history, so the reset moves both numbers.
#[test]
fn a_branch_operation_returns_head_and_the_tree_hash_bit_equal() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();
    let locator = sandbox.locator_on(BRANCH);

    let (head_before, tree_before) = sandbox.head_and_tree();
    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let delta = reset_to(&locator, sandbox.elsewhere());

    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("a branch with a commit on it has an inverse");

    adapter.apply(&delta).expect("the reset applies");
    let (head_after, tree_after) = sandbox.head_and_tree();
    println!("AC050_CASE2_FORWARD head {head_before} -> {head_after}  tree {tree_before} -> {tree_after}");
    assert_eq!(
        head_after,
        sandbox.elsewhere(),
        "the branch is where the reset put it"
    );
    assert_ne!(tree_after, tree_before, "the two commits hold two trees");

    adapter.apply(&inverse).expect("the inverse applies");
    let (head_back, tree_back) = sandbox.head_and_tree();
    println!("AC050_CASE2_BACK head={head_back} tree={tree_back}");
    assert_eq!(head_back, head_before, "AC-050: HEAD is bit-equal");
    assert_eq!(tree_back, tree_before, "AC-050: the tree hash is bit-equal");
}

/// 🔴 The negative control both cases need: a round trip that returns HEAD **without** the adapter
/// having moved it would pass the two tests above for an adapter that does nothing.
///
/// So the forward step is asserted to move both numbers (it is, above) and this probe asserts the
/// other end: an `apply` of the forward delta a second time — the retry of 51 §7 contract 7 — moves
/// **neither**, because the change is already made. An adapter that minted a second commit on the
/// retry would move HEAD here and would still pass every bit-equality above.
#[test]
fn the_retry_moves_neither_number() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();
    let locator = sandbox.locator_on(BRANCH);

    let delta = planned(adapter, &locator, GOAL);
    adapter.apply(&delta).expect("the first apply");
    let (head_once, tree_once) = sandbox.head_and_tree();
    adapter.apply(&delta).expect("the retry (43 T-10c)");
    let (head_twice, tree_twice) = sandbox.head_and_tree();

    println!("AC050_RETRY head {head_once} -> {head_twice}  tree {tree_once} -> {tree_twice}");
    assert_eq!(
        head_twice, head_once,
        "the retry minted a second commit: the quantifier is 'the same delta re-entering' (E-M4-3), and a \
         branch that moves twice for one delta has not been applied idempotently (sem: SEM-gx-adapter-git-096)"
    );
    assert_eq!(tree_twice, tree_once);
}

/// The entry really holds the goal afterwards, read out of the object database.
///
/// HEAD and a tree hash are the AC's subject; this is the sentence underneath them. A commit whose
/// tree hash moved while carrying the wrong bytes would satisfy every assertion above.
#[test]
fn the_entry_holds_the_goal_after_the_forward_apply() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();
    let locator = sandbox.locator_on(BRANCH);

    let delta = planned(adapter, &locator, GOAL);
    let applied = adapter.apply(&delta).expect("the entry change applies");

    let repo = sandbox.repository();
    let head = repo.head_id().expect("HEAD resolves").detach();
    let tree = repo
        .find_object(head)
        .expect("HEAD is an object")
        .into_commit()
        .tree()
        .expect("a commit has a tree");
    let entry = tree
        .lookup_entry_by_path(SUBJECT)
        .expect("the tree answers")
        .expect("the entry is in the tree");
    let blob = repo.find_object(entry.oid()).expect("the blob is there");
    println!(
        "AC050_CONTENT bytes={:?} resulting_digest={:?}",
        String::from_utf8_lossy(&blob.data),
        applied.resulting_digest()
    );
    assert_eq!(blob.data.as_slice(), GOAL);
}
