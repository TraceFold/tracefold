//! **AC-052**'s git third, and the case that decides whether a scope is a branch or a file.
//!
//! 34 AC-052 逐語: 「Given: fs/git/mcp各adapterについて可換な2delta…と非可換な2delta…。When:
//! `adapter.commutation(a,b)`を呼ぶ。Then: 可換ペアで`Commutes`、非可換ペアで`Conflicts{residual}`（各
//! adapter最低1組ずつ、**計6ケース**）。」 `req/38` §35 M4H6-9: 「AC-052 の行の完了は M7 の **6/6** 時」 —
//! this file supplies the git pair (2 of the 6), and `gx-adapter-mcp` supplies the last two in hand 3.
//!
//! # 🔴 The case the fs adapter has no analogue for
//!
//! `gx-adapter-fs` compares **positions**, so two changes to two files are independent. This adapter
//! compares **branches**, so two changes to two files on one branch are *not* — and that difference is
//! not a nuance, it is the whole reason the scope exists as a concept separate from the object
//! (42 §3.5). [`two_files_on_one_branch_conflict`] is the probe that measures it, and it would be red
//! for an adapter that copied the fs one.
//!
//! `Commutes` is the fail-open direction: 43 §8 acts on the answer by letting both proceed, and the
//! second commit would then be built on a tip the first had already moved.

mod support;

use gx_core::Commutation;
use gx_substrate::SubstrateAdapter;
use support::{planned, reset_to, GitFixture, BRANCH, OTHER_BRANCH};

/// **AC-052 (git, 可換)**: two changes on two branches.
#[test]
fn two_branches_commute() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();

    let a = planned(adapter, &sandbox.locator_on(BRANCH), b"one\n");
    let b = planned(adapter, &sandbox.locator_on(OTHER_BRANCH), b"two\n");
    let verdict = adapter.commutation(&a, &b).expect("the pair is comparable");
    println!("AC052_GIT_COMMUTES {verdict:?}");
    assert!(matches!(verdict, Commutation::Commutes));
}

/// **AC-052 (git, 非可換)**: two changes on one branch, with a residual that names the second.
#[test]
fn one_branch_conflicts_and_the_residual_names_the_second_delta() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);

    let a = planned(adapter, &locator, b"one\n");
    let b = planned(adapter, &locator, b"two\n");
    let verdict = adapter.commutation(&a, &b).expect("the pair is comparable");
    println!("AC052_GIT_CONFLICTS {verdict:?}");
    let Commutation::Conflicts { residual } = verdict else {
        panic!("two changes to one branch are not parallel-independent");
    };
    // **M4-14**: 「residual CID が test harness 内で解決可能」. There is no delta store here, so 「解決
    // 可能」 means the reference has a referent somebody holds — and it does: it is `b`, which the
    // caller passed in (**E-M4-8** keeps the payload).
    assert_eq!(
        &residual,
        b.reference(),
        "the residual is the whole of the second delta: on one branch there is no proper part of a \
         change that is independent of an earlier one (42 §3.6)"
    );
    assert_ne!(&residual, a.reference(), "and it is not the first");
}

/// 🔴 The case that separates this adapter from a copy of the fs one.
///
/// Two **different files** on **one branch**. An adapter comparing whole locators answers `Commutes`
/// here, and 43 §8 would then let both proceed — the second commit built on a tip the first had
/// already moved. An adapter comparing branches answers `Conflicts`, one of them waits, and that is
/// what a branch actually is.
#[test]
fn two_files_on_one_branch_conflict() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let repo = fixture.sandbox().dir().display().to_string();

    let a = support::spelled(&format!("{repo}#{BRANCH}:subject.txt"), b"one\n");
    let b = support::spelled(&format!("{repo}#{BRANCH}:other.txt"), b"two\n");
    let verdict = adapter.commutation(&a, &b).expect("the pair is comparable");
    println!("GIT_TWO_FILES_ONE_BRANCH {verdict:?}");
    assert!(
        matches!(verdict, Commutation::Conflicts { .. }),
        "a git change's footprint is its branch, not its entry: both of these rewrite the tree, \
         mint a commit and move `{BRANCH}`"
    );
}

/// **L6** at the adapter, not only through the harness: the verdict is symmetric and `(a, a)`
/// conflicts.
#[test]
fn the_verdict_is_symmetric_and_a_delta_conflicts_with_itself() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();

    let a = planned(adapter, &sandbox.locator_on(BRANCH), b"one\n");
    let b = planned(adapter, &sandbox.locator_on(OTHER_BRANCH), b"two\n");
    let c = planned(adapter, &sandbox.locator_on(BRANCH), b"three\n");

    for (left, right) in [(&a, &b), (&a, &c)] {
        let forward = adapter.commutation(left, right).expect("comparable");
        let backward = adapter.commutation(right, left).expect("comparable");
        let agree = matches!(
            (&forward, &backward),
            (Commutation::Commutes, Commutation::Commutes)
                | (Commutation::Conflicts { .. }, Commutation::Conflicts { .. })
        );
        println!("L6_GIT forward={forward:?} backward={backward:?} agree={agree}");
        assert!(agree);
    }

    // **M4-25 採(a)**: the reflexive case is `Conflicts`, which is the conservative side.
    let self_verdict = adapter.commutation(&a, &a).expect("comparable");
    println!("L6_GIT_REFLEXIVE {self_verdict:?}");
    assert!(matches!(self_verdict, Commutation::Conflicts { .. }));
}

/// A `reset` and an entry change on one branch conflict too — the kind of the operation is not the
/// footprint.
#[test]
fn a_reset_and_an_entry_change_on_one_branch_conflict() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();
    let locator = sandbox.locator_on(BRANCH);

    let entry = planned(adapter, &locator, b"one\n");
    let reset = reset_to(&locator, sandbox.elsewhere());
    let verdict = adapter
        .commutation(&entry, &reset)
        .expect("the pair is comparable");
    println!("GIT_RESET_VS_ENTRY {verdict:?}");
    assert!(matches!(verdict, Commutation::Conflicts { .. }));

    // And a reset on another branch is independent of both.
    let elsewhere = reset_to(&sandbox.locator_on(OTHER_BRANCH), sandbox.origin());
    assert!(matches!(
        adapter.commutation(&entry, &elsewhere).expect("comparable"),
        Commutation::Commutes
    ));
}

/// A delta from another adapter is refused rather than answered about.
///
/// [`gx_substrate::Error::ForeignDelta`] and not a verdict: a payload written in another grammar is a
/// mis-wired engine, and any answer other than a refusal reports the symptom instead of the fault
/// (**E-M4-27**'s argument, delta side).
#[test]
fn a_delta_from_another_adapter_is_refused() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let mine = planned(adapter, &fixture.sandbox().locator_on(BRANCH), b"one\n");
    let theirs = gx_substrate::PlannedDelta::new(gx_core::SubstrateKind::Fs, b"whatever".to_vec())
        .expect("the projection is encodable");

    let err = adapter
        .commutation(&mine, &theirs)
        .expect_err("two substrates are not comparable");
    println!("GIT_FOREIGN_DELTA kind={}", err.kind());
    assert_eq!(err.kind(), "ForeignDelta");
}

/// **E-M4-32**: an `invert` whose `pre` names another object is a wiring bug, not an `Ok(None)`.
#[test]
fn inverting_against_another_objects_snapshot_is_a_refusal() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();

    let delta = planned(adapter, &sandbox.locator_on(BRANCH), b"one\n");
    let other = adapter
        .snapshot(&sandbox.locator_on(OTHER_BRANCH))
        .expect("the other branch holds the entry too");
    let err = adapter
        .invert(&delta, &other)
        .expect_err("a snapshot of another object is a mis-wired call");
    println!("GIT_LOCATOR_MISMATCH kind={}", err.kind());
    assert_eq!(
        err.kind(),
        "LocatorMismatch",
        "answering `Ok(None)` would send a defect down E-M3-4's escalation path wearing the face \
         of a legitimate business condition"
    );
}

/// The `Ok(None)` this adapter actually has: an unborn branch.
#[test]
fn an_unborn_branch_has_no_inverse_and_says_so_with_ok_none() {
    use gx_substrate_conformance::Fixture;

    let fixture = GitFixture::new();
    let (delta, pre) = fixture
        .uninvertible()
        .expect("the fixture supplies the unborn branch");
    let answer = fixture
        .adapter()
        .invert(&delta, &pre)
        .expect("the question is answerable");
    println!("GIT_OK_NONE is_none={}", answer.is_none());
    assert!(
        answer.is_none(),
        "undoing the first commit on a branch means deleting the branch, and v0.1 spells no \
         deletion: E-M3-4 asks a person instead"
    );
}
