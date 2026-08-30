// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴🔴 **DEFECT-891-1** (`req/895` §2) — a second undo must not make the first one's branch
//! unreachable.
//!
//! `req/891` asked whether redo is guaranteed and found that it is: `gx undo <T_u>` is the redo,
//! because an undo is an ordinary transformation with an escrow of its own. Then it found the
//! condition under which the answer stops being true, and this file is that condition, measured
//! through the binary.
//!
//! # The mechanism, in one paragraph
//!
//! An `Intent`'s identity is substrate / locator / goal bytes / context / actor. A
//! `Transformation`'s identity additionally carries `parents`, and `Engine::undo` mints `T_u` with
//! `parents = vec![T_o]` (43 T-12's guard). So two undos that restore the same bytes at the same
//! locator under the same context and actor share **one** `IntentId` and have **two**
//! `TransformationId`s. `Engine`'s resolution index was a `BTreeMap<IntentId, TransformationId>`
//! rebuilt by last-write-wins, so the second undo evicted the first; `Session::intent_of` asks
//! that index "does this intent resolve to this transformation?" in order to find the draft a
//! rehydrate needs, and after the eviction it answered `None`.
//!
//! # What that looked like from outside, before the repair
//!
//! `gx undo <T_u>` exited **6** with `{"gx_code":"NOT_FOUND","title":"the named object is not
//! here"}` — about a transformation whose signed commit receipt was sitting in the same project's
//! `.gx/receipts/`. `pipeline.rs`'s own `undo` documentation names that shape as one a reader must
//! never be handed: "a caller reading 'no transformation' for a row `GET /transformations/{id}`
//! had just answered `200` about would be reading a contradiction".
//!
//! # The two tests, and why the second one is not decoration
//!
//! The first drives the branch and asserts the redo lands **by content** — the file's bytes, not
//! the exit code. The second is the discriminating control `req/891` found by accident and this
//! lane re-ran deliberately: changing **only** `--context` on the branch commit gives the two undos
//! different `IntentId`s, and the same sequence succeeded even before the repair. Keeping it here
//! means a future regression that broke undo in general would fail both, and one that re-collapsed
//! the index would fail only the first — the two failures are distinguishable, which is the whole
//! reason to run the control.

mod support;

use support::{pipeline, run, Pipeline};

/// `submit → plan → verify → commit` with a chosen `--context`, and the id it lands on.
///
/// `Pipeline::commit_one` fixes `--context Evidence`; the discriminating control needs a second
/// value, and the difference between the two runs has to be exactly that one flag.
fn commit_with_context(p: &Pipeline, goal: &str, context: &str, tag: &str) -> String {
    let goal_file = p.project.join(format!("goal-{tag}.txt"));
    std::fs::write(&goal_file, goal).expect("write the goal");
    let submitted = run(p
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .arg("--locator")
        .arg(&p.target)
        .arg("--intent")
        .arg(&goal_file)
        .args(["--context", context])
        .args(["--actor-key", &p.key_id]));
    assert_eq!(submitted.code, 0, "submit({context}): {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(p.gx().args(["plan", &intent_id]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    let verified = run(p.gx().args(["verify", &tid]));
    assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
    let committed = run(p.gx().args(["commit", &tid]));
    assert_eq!(committed.code, 0, "commit: {}", committed.stderr);
    tid
}

/// `gx undo <id> --settle 0`, and the id of the `T_u` it minted.
///
/// `--settle 0` because the default budget polls for two minutes before answering, and what is
/// being measured here is a decision rather than a wait (`req/38` §98 ruling 2's flag).
fn undo(p: &Pipeline, id: &str) -> (i32, String, String) {
    let out = run(p.gx().args(["undo", id, "--settle", "0"]));
    let minted = if out.code == 0 {
        out.json()["transformation"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    } else {
        String::new()
    };
    (out.code, minted, format!("{}{}", out.stdout, out.stderr))
}

/// 🔴 The defect: `V0 → V1 → undo → V2 → undo → redo(V1)`, all under one `--context`.
#[test]
fn a_second_undo_does_not_make_the_first_undos_branch_unreachable() {
    let p = pipeline("dr891_same_context", "V0\n");

    let t_o = commit_with_context(&p, "V1\n", "Evidence", "v1");
    assert_eq!(
        p.target_contents(),
        "V1\n",
        "the first commit moved the world"
    );

    let (undo_code, t_u, undo_body) = undo(&p, &t_o);
    assert_eq!(undo_code, 0, "undo(T_o): {undo_body}");
    assert!(
        !t_u.is_empty(),
        "the undo minted a transformation: {undo_body}"
    );
    assert_eq!(p.target_contents(), "V0\n", "the undo put the world back");

    // The branch: a different change from the same point, and its own undo. Same context, so this
    // undo's Intent is the one above's.
    let t_x = commit_with_context(&p, "V2\n", "Evidence", "v2");
    assert_eq!(p.target_contents(), "V2\n");
    let (branch_code, t_xu, branch_body) = undo(&p, &t_x);
    assert_eq!(branch_code, 0, "undo(T_x): {branch_body}");
    assert_ne!(
        t_xu, t_u,
        "the two undos differ in `parents`, so they are two transformations — if these are equal \
         the rest of this test is measuring nothing"
    );
    assert_eq!(p.target_contents(), "V0\n", "the world is back at the fork");

    // The redo: undo the first undo. The world is at the fork, so the compare-and-set is satisfied
    // and there is no honest reason to refuse.
    let (redo_code, _, redo_body) = undo(&p, &t_u);
    println!(
        "DR891_SAME_CONTEXT redo_code={redo_code} world={:?}",
        p.target_contents()
    );

    // 🔴 Content first, and on its own line: an exit code is what this defect was hiding behind.
    assert_eq!(
        p.target_contents(),
        "V1\n",
        "the redo did not restore the branch. redo said: {redo_body}"
    );
    assert_eq!(redo_code, 0, "redo: {redo_body}");
    assert!(
        !redo_body.contains("NOT_FOUND"),
        "the engine answered `NOT_FOUND` about a transformation it holds a signed commit receipt \
         for: {redo_body}"
    );
}

/// 🔴 The discriminating control: with a different `--context` the intents differ, and this road
/// was never broken.
///
/// It is what makes the test above a statement about **intent collision** rather than about undo
/// in general. Both tests failing means undo is broken; only the first failing means the index has
/// re-collapsed.
#[test]
fn the_same_branch_under_a_different_context_was_always_reachable() {
    let p = pipeline("dr891_diff_context", "V0\n");

    let t_o = commit_with_context(&p, "V1\n", "Evidence", "v1");
    let (undo_code, t_u, undo_body) = undo(&p, &t_o);
    assert_eq!(undo_code, 0, "undo(T_o): {undo_body}");

    let t_x = commit_with_context(&p, "V2\n", "Policy", "v2");
    let (branch_code, _, branch_body) = undo(&p, &t_x);
    assert_eq!(branch_code, 0, "undo(T_x): {branch_body}");
    assert_eq!(p.target_contents(), "V0\n");

    let (redo_code, _, redo_body) = undo(&p, &t_u);
    println!(
        "DR891_DIFF_CONTEXT redo_code={redo_code} world={:?}",
        p.target_contents()
    );
    assert_eq!(p.target_contents(), "V1\n", "redo: {redo_body}");
    assert_eq!(redo_code, 0, "redo: {redo_body}");
}

/// 🔴 The forward road is untouched: re-planning one intent stays idempotent.
///
/// The repair widens the resolution index's value from one id to a list, and the failure mode a
/// list invites is a second entry for a **retry**. 43 T-2's idempotency column says a re-`plan` of
/// the same intent against the same snapshot yields the same `TransformationId`, so the list must
/// still hold exactly one — measured here through the binary rather than asserted about the map.
#[test]
fn replanning_one_intent_still_lands_on_one_transformation() {
    let p = pipeline("dr891_forward", "V0\n");
    let goal_file = p.project.join("goal-forward.txt");
    std::fs::write(&goal_file, "V1\n").expect("write the goal");
    let submitted = run(p
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .arg("--locator")
        .arg(&p.target)
        .arg("--intent")
        .arg(&goal_file)
        .args(["--context", "Evidence"])
        .args(["--actor-key", &p.key_id]));
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("id")
        .to_string();

    let first = run(p.gx().args(["plan", &intent_id]));
    assert_eq!(first.code, 0, "plan: {}", first.stderr);
    let second = run(p.gx().args(["plan", &intent_id]));
    assert_eq!(second.code, 0, "re-plan: {}", second.stderr);

    let a = first.json()["transformation"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    let b = second.json()["transformation"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    println!("DR891_FORWARD first={a} second={b}");
    assert_eq!(
        a, b,
        "43 T-2: a re-plan of one intent is the same transformation"
    );
}
