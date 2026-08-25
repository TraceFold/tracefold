// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-054 (FR-049)** — the four verbs of 44 §1.2's pipeline, normal and abnormal, with the
//! expected status written as a **number**.
//!
//! 34 AC-054 verbatim (sem: SEM-gx-cli-1960):
//!
//! > Given: each subcommand of `gx submit|verify|commit|receipt|replay|undo|log`. When: run with normal-case input
//! > (e.g. `gx submit --intent valid.json`) and abnormal-case input (e.g. `gx commit 0000…` a nonexistent ID),
//! > each in turn. Then: the normal case exits code 0, the abnormal case exits non-0 (each subcommand: 1 normal + 1 abnormal = 14 cases total) (sem: SEM-gx-cli-1961).
//!
//! Hand 2 covered `receipt`, `replay` and `log`; this file covers `submit`, `plan`, `verify` and
//! `commit`, and hand 4 covers `undo`.
//!
//! # 🔴 "non-0" (sem: SEM-gx-cli-1962) is not an assertion
//!
//! req/88 §8.2: "an exit-code test measures 'the correct non-0', not 'it was non-0'. Passing on
//! 'it was non-0' would let M6-25's hole (deny and unable-to-execute sharing the same 1) slip through the test — **write the expected value as a number**" (sem: SEM-gx-cli-1963). Every abnormal case below
//! names the code 44 §1.2 gives it, and the two that are **2** are the point of the exercise: a
//! transformation the gate refused and a commit of one, which are the first two places in this
//! workspace where 44 §1.4's "refused (denied)" (sem: SEM-gx-cli-1964) is returned by anything. discipline 52 exists so that number
//! cannot also mean "you mistyped a flag" (sem: SEM-gx-cli-1965).

mod support;

use support::{pipeline, run, Pipeline, DENIED_LOCATOR};

/// A transformation id that parses and names nothing. 52 base32 characters, all `a`.
///
/// AC-054's own example is "`gx commit 0000…` a nonexistent ID" (sem: SEM-gx-cli-1966) and the shape matters: an id that does
/// not **parse** is "invalid input" (1) and an id that parses and is absent is "not-found" (6) (sem: SEM-gx-cli-1967). Testing the
/// first would measure the argument parser and call it the state machine.
const ABSENT: &str = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Drive `submit` → `plan` and hand back the two ids, as separate processes.
fn planned(fixture: &Pipeline, goal: &str) -> (String, String) {
    let submitted = fixture.submit(goal);
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let planned = run(fixture.gx().args(["plan", &intent_id]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let transformation = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    (intent_id, transformation)
}

/// 🔴 **AC-054, the four cases of the normal road** — one process per verb, exit 0 each time.
#[test]
fn ac_054_the_pipeline_runs_as_four_processes() {
    let fixture = pipeline("ac054_normal", "before\n");
    let (intent_id, transformation) = planned(&fixture, "after\n");

    let verified = run(fixture.gx().args(["verify", &transformation]));
    let committed = run(fixture.gx().args(["commit", &transformation]));

    println!(
        "AC054_NORMAL submit=0 plan=0 verify={} commit={} intent={intent_id} tid={transformation} \
         target={:?} journal_records={}",
        verified.code,
        committed.code,
        fixture.target_contents(),
        fixture.journal_records()
    );
    assert_eq!(
        verified.code, 0,
        "44 §1.2: \"0=Admit\" (sem: SEM-gx-cli-1968). stderr: {}",
        verified.stderr
    );
    assert_eq!(
        verified.json()["kind"],
        "Admit",
        "the shipped pack permits an fs change outside /etc"
    );
    assert_eq!(
        committed.code, 0,
        "44 §1.2: \"0=Committed\" (sem: SEM-gx-cli-1969). stderr: {}",
        committed.stderr
    );
    // 44 §1.2: "on success, `Receipt` (a DSSE envelope, JSON including a base64 payload)" (sem: SEM-gx-cli-1970).
    assert!(
        committed.json()["envelope"]["payload"].is_string(),
        "a commit prints the receipt: {}",
        committed.stdout
    );
    // The whole point of the exercise, in one line: the substrate moved.
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "T-11 applied the delta the four processes planned"
    );
}

/// 🔴 **AC-054, the four abnormal cases**, each with the number 44 §1.2 gives it.
#[test]
fn ac_054_the_abnormal_cases_exit_with_the_code_44_names() {
    let fixture = pipeline("ac054_abnormal", "before\n");

    // `gx submit` — "1=input error" (sem: SEM-gx-cli-1971). A substrate 44 does not name.
    let submit = run(fixture
        .gx()
        .arg("submit")
        .args(["--substrate", "quantum"])
        .arg("--locator")
        .arg(&fixture.target)
        .arg("--intent")
        .arg(&fixture.target)
        .args(["--context", "Evidence"])
        .args(["--actor-key", &fixture.key_id]));

    // `gx plan` — "6=the specified ID is not-found" (sem: SEM-gx-cli-1972). An id that parses and names nothing.
    let plan = run(fixture.gx().args(["plan", ABSENT]));

    // `gx verify` — "6=not-found" (sem: SEM-gx-cli-1973), same shape one verb along.
    let verify_absent = run(fixture.gx().args(["verify", ABSENT]));

    // `gx commit` — AC-054's own example: "a nonexistent ID" (sem: SEM-gx-cli-1974).
    let commit_absent = run(fixture.gx().args(["commit", ABSENT]));

    println!(
        "AC054_ABNORMAL submit={} plan={} verify={} commit={}",
        submit.code, plan.code, verify_absent.code, commit_absent.code
    );
    assert_eq!(
        submit.code, 1,
        "44 §1.2 `gx submit`: \"1=input error\" (sem: SEM-gx-cli-1975)"
    );
    assert_eq!(
        plan.code, 6,
        "44 §1.2 `gx plan`: \"6=the specified ID is not-found\" (sem: SEM-gx-cli-1976)"
    );
    assert_eq!(
        verify_absent.code, 6,
        "44 §1.2 `gx verify`: \"6=not-found\" (sem: SEM-gx-cli-1977)"
    );
    assert_eq!(
        commit_absent.code, 6,
        "44 §1.2 `gx commit`: \"6=not-found\" (sem: SEM-gx-cli-1978)"
    );

    // 44 §1.3: "an error is written to stderr … and stdout emits nothing" (sem: SEM-gx-cli-1979).
    for run in [&submit, &plan, &verify_absent, &commit_absent] {
        assert!(
            run.stdout.trim().is_empty(),
            "a refusal writes nothing to stdout: {:?}",
            run.stdout
        );
        assert!(
            run.stderr.contains("gx_code"),
            "and writes 44 §1.3's problem object to stderr: {:?}",
            run.stderr
        );
    }
}

/// 🔴 **The two places 44 §1.4's `2` is returned**, and neither of them is a mistyped flag.
///
/// The shipped pack (`policies/fs/deny-etc.cedar`) forbids `/etc` and everything under it, so a
/// transformation against [`DENIED_LOCATOR`] is the CLI's one route to `Verdict::Deny` in v0.1. Two
/// facts follow and both are asserted:
///
/// * `gx verify` exits **2** — 44 §1.2's "2=Deny", 44 §1.4's "refused (denied)" (sem: SEM-gx-cli-1980);
/// * `gx commit` of that transformation exits **2** as well — "2=refused because it is Deny and not Admit
///   (non-record-only and Verdict≠Admit)" (sem: SEM-gx-cli-1981) — and is refused by T-8 **before** `apply`, which is why
///   this test can name a path under `/etc` at all. The file is read and never written.
#[test]
fn ac_054_a_denied_transformation_exits_two_at_both_verbs() {
    let fixture = pipeline("ac054_denied", "unused\n");
    let goal = fixture.project.join("denied-goal.txt");
    std::fs::write(&goal, "this must never be written\n").expect("write the goal");

    let submitted = run(fixture
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .args(["--locator", DENIED_LOCATOR])
        .arg("--intent")
        .arg(&goal)
        .args(["--context", "Policy"])
        .args(["--actor-key", &fixture.key_id]));
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let planned = run(fixture.gx().args(["plan", &intent_id]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let transformation = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();

    let before = std::fs::read(DENIED_LOCATOR).expect("the locator is readable");
    let verified = run(fixture.gx().args(["verify", &transformation]));
    let committed = run(fixture.gx().args(["commit", &transformation]));
    let after = std::fs::read(DENIED_LOCATOR).expect("the locator is still readable");

    println!(
        "AC054_DENIED locator={DENIED_LOCATOR} verify={} commit={} kind={:?} state={:?} \
         substrate_unchanged={}",
        verified.code,
        committed.code,
        verified.json()["kind"],
        committed.json()["state"],
        u8::from(before == after)
    );
    assert_eq!(
        verified.code, 2,
        "44 §1.2 `gx verify`: \"2=Deny\" (sem: SEM-gx-cli-1982). stderr: {}",
        verified.stderr
    );
    assert_eq!(verified.json()["kind"], "Deny");
    assert_eq!(
        committed.code, 2,
        "44 §1.2 `gx commit`: \"2=refused because it is Deny and not Admit\" (sem: SEM-gx-cli-1983). stdout: {}",
        committed.stdout
    );
    assert_eq!(
        before, after,
        "🔴 T-8 refuses a `Denied` row before `apply` is reached, so the substrate is untouched. \
         If this ever fails, the record-only warning in `support::DENIED_LOCATOR` has become real"
    );
}
