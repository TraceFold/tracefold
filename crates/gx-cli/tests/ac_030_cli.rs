//! **AC-030 / AC-011, re-confirmed through the CLI** — the same intent names the same id in two
//! processes, and the same plan names the same transformation.
//!
//! 34 AC-030 逐語 ends with the instruction this file carries out:
//!
//! > CLIレベル（`gx submit`/`gx plan`）での再確認はM6のE2E AC（AC-054）で行う。
//!
//! and AC-011 says the same thing about `gx_canon::cid::compute` (「CLIレベル…での同一ID再確認は
//! AC-030およびM6のE2E AC（AC-054）で行う」). M5 measured the library API in two operating-system
//! processes; this measures the **binary**, which is the thing an operator runs.
//!
//! # 🔴 Two projects, not one
//!
//! Running `gx submit` twice against one `.gx/` would measure T-1's create-if-absent — 「同一canonical
//! encodeのintent再送は同一`IntentId`を返す（副作用なし）」 — which is a fact about a **map lookup**.
//! ASM-11 is a stronger claim: the id is a function of the value and of nothing else, so two
//! directories that have never seen each other have to arrive at the same name. That is the shape
//! AC-011 uses for its two processes (「別バイナリ・キャッシュ非共有・別ワーキングディレクトリ」) and
//! it is the shape here.
//!
//! The `plan` half needs one more thing to be honest: the two projects must plan against the **same
//! substrate state**, because 43 T-2 fixes the `TransformationId` over a canonical form that
//! includes `subject` — the id of the object the adapter snapshotted. So both fixtures point at one
//! file, and `SUBJECT` in the output is what shows they did.

mod support;

use support::{pipeline, run};

/// 🔴 AC-030 (1): the same intent, two processes with nothing in common, one `IntentId`.
#[test]
fn ac_030_cli_the_same_intent_is_the_same_intent_id_in_two_processes() {
    let a = pipeline("ac030_cli_a", "before\n");
    let b = pipeline("ac030_cli_b", "before\n");

    // The two fixtures generated **different keys**, and the actor is part of 42 §3.3's `Intent`, so
    // the ids would differ for a reason that has nothing to do with what is being measured. `b`
    // submits under `a`'s key id: the key is a name in the intent, and naming it is all the intent
    // does with it (the private half is only reached at `verify`).
    let submitted_a = a.submit("after\n");
    let submitted_b = run(b
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .arg("--locator")
        .arg(&a.target)
        .arg("--intent")
        .arg({
            let goal = b.project.join("goal.txt");
            std::fs::write(&goal, "after\n").expect("write the goal");
            goal
        })
        .args(["--context", "Evidence"])
        .args(["--actor-key", &a.key_id]));

    assert_eq!(submitted_a.code, 0, "{}", submitted_a.stderr);
    assert_eq!(submitted_b.code, 0, "{}", submitted_b.stderr);
    let id_a = submitted_a.json()["intent_id"].clone();
    let id_b = submitted_b.json()["intent_id"].clone();
    println!("AC030_CLI_INTENT a={id_a} b={id_b}");
    assert_eq!(
        id_a, id_b,
        "ASM-11: 「同一intent→同一IntentId」, across two `.gx/` directories that share nothing"
    );

    // The negative half, without which the assertion above is satisfied by a constant. One
    // character of the goal differs and the id has to move.
    let different = a.submit("aftex\n");
    assert_eq!(different.code, 0, "{}", different.stderr);
    println!(
        "AC030_CLI_INTENT_DIFFERENT={}",
        different.json()["intent_id"]
    );
    assert_ne!(
        different.json()["intent_id"],
        id_a,
        "42 §3.3 puts the goal in the identity; a different goal is a different intent"
    );
}

/// 🔴 AC-030 (2): the same plan against the same substrate state, one `TransformationId`.
#[test]
fn ac_030_cli_the_same_plan_is_the_same_transformation_id_in_two_processes() {
    let a = pipeline("ac030_cli_plan_a", "before\n");
    let b = pipeline("ac030_cli_plan_b", "unused\n");

    let goal_b = b.project.join("goal.txt");
    std::fs::write(&goal_b, "after\n").expect("write the goal");
    let submitted_a = a.submit("after\n");
    let submitted_b = run(b
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .arg("--locator")
        .arg(&a.target) // one file, so one snapshot, so one subject
        .arg("--intent")
        .arg(&goal_b)
        .args(["--context", "Evidence"])
        .args(["--actor-key", &a.key_id]));
    assert_eq!(submitted_a.code, 0, "{}", submitted_a.stderr);
    assert_eq!(submitted_b.code, 0, "{}", submitted_b.stderr);

    let intent = submitted_a.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned_a = run(a.gx().args(["plan", &intent]));
    let planned_b = run(b.gx().args(["plan", &intent]));
    assert_eq!(planned_a.code, 0, "{}", planned_a.stderr);
    assert_eq!(planned_b.code, 0, "{}", planned_b.stderr);

    let tid_a = planned_a.json()["transformation"]["id"].clone();
    let tid_b = planned_b.json()["transformation"]["id"].clone();
    println!(
        "AC030_CLI_PLAN a={tid_a} b={tid_b} SUBJECT_A={} SUBJECT_B={}",
        planned_a.json()["transformation"]["subject"],
        planned_b.json()["transformation"]["subject"]
    );
    assert_eq!(
        tid_a, tid_b,
        "ASM-11: 「plan()完了後は同一`TransformationId`」, in two processes with separate journals"
    );
    assert_eq!(
        planned_a.json()["transformation"]["subject"],
        planned_b.json()["transformation"]["subject"],
        "and they agree about what they were planning against, which is why the ids can agree"
    );
    assert_eq!(
        planned_a.json()["state"],
        "Candidate",
        "43 T-2: Draft → Candidate"
    );
}

/// 🔴 The **third** process: `gx plan` reached by the `TransformationId` rather than the `IntentId`.
///
/// 44 §0's id-resolution rule is 「`IntentId`と`TransformationId`のいずれの`gx1:...`値も受理し…
/// `plan()`完了後は正準の`TransformationId`へ解決する」, and a rule that accepts two spellings has to
/// be measured with both. The second spelling reaches a transformation the process it is running in
/// has never seen in memory, which is the whole of `session::Session::resume`.
#[test]
fn ac_030_cli_both_spellings_of_the_id_resolve_to_one_transformation() {
    let fixture = pipeline("ac030_cli_resolution", "before\n");
    let submitted = fixture.submit("after\n");
    assert_eq!(submitted.code, 0, "{}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let by_intent = run(fixture.gx().args(["plan", &intent]));
    assert_eq!(by_intent.code, 0, "{}", by_intent.stderr);
    let tid = by_intent.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();

    let by_transformation = run(fixture.gx().args(["plan", &tid]));
    println!(
        "ID_RESOLUTION intent={intent} tid={tid} by_transformation_rc={} same={}",
        by_transformation.code,
        u8::from(by_transformation.json()["transformation"]["id"] == tid.as_str())
    );
    assert_eq!(by_transformation.code, 0, "{}", by_transformation.stderr);
    assert_eq!(
        by_transformation.json()["transformation"]["id"],
        tid,
        "44 §0: both spellings resolve to the canonical `TransformationId`"
    );

    // 🔴 And the resolution wrote **nothing**: 43 T-2's idempotency is 「安全に再試行可」 and req/88
    // §3 Λ2 needs it to be silent. Two `Planned` records for one plan would be a single-shot CLI
    // and a long-lived engine disagreeing about how many times T-2 fired.
    println!(
        "JOURNAL_RECORDS_AFTER_TWO_PLANS={}",
        fixture.journal_records()
    );
    assert_eq!(
        fixture.journal_records(),
        2,
        "one `DraftCreated` and one `Planned`, however many processes asked"
    );
}
