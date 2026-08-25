// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `gx undo` (44 §1.2) — the last verb of AC-054, and **44 §1.4's 2 on a third command**.
//!
//! 34 AC-054 lists `undo` among the subcommands it wants one normal and one abnormal case for, and
//! hands 2 and 3 covered the other six. This is the seventh, and it is the one that closes the TTFV
//! road: `tools/ttfv.sh` declared `gx undo` as step 8 and reported it **unimplemented** rather than
//! dropping it, exactly so that the road could not get shorter by omission (req/29 §4).
//!
//! # 🔴 What is measured that a "it exited 0" test would miss (sem: SEM-gx-cli-1358)
//!
//! * the **substrate went back** — a `gx undo` that returned a receipt and left the file alone would
//!   pass every status check in this file;
//! * the original is **`Superseded`** and not rewritten (P-5, 43 T-12: "`T_o`'s canonical record and
//!   receipt stay unchanged"; sem: SEM-gx-cli-1359);
//! * a **second** undo of one commit is refused (42 §3.12's `Consumed`), which is what makes
//!   "only once" (sem: SEM-gx-cli-1360) a fact rather than a hope — and it is refused **across processes**, which is the
//!   half a single-process engine test cannot see;
//! * a **denied** undo exits **2** and not 1 (M6-25 adopted (a)+(c); sem: SEM-gx-cli-1361), and the original stays `Committed`.

mod support;

use support::{deny_writable_pack, pipeline, pipeline_named, run, DENIED_FRAGMENT};

/// A transformation id that parses and names nothing (AC-054's shape, hand 3's constant).
const ABSENT: &str = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// 🔴 **AC-054's normal case for `undo`** — five processes, and the world comes back.
#[test]
fn ac_054_undo_returns_the_substrate_to_what_it_was() {
    let fixture = pipeline("m6h4_undo_normal", "before\n");
    let committed = fixture.commit_one("after\n");
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "the fixture has to have moved the world before this test means anything"
    );

    let undone = run(fixture.gx().args(["undo", &committed]));
    println!(
        "UNDO_NORMAL exit={} target={:?} superseded_state={:?} undone={:?} records={}",
        undone.code,
        fixture.target_contents(),
        undone.json()["superseded_state"],
        undone.json()["undone"],
        fixture.journal_records()
    );
    assert_eq!(
        undone.code, 0,
        "44 §1.2 `gx undo`: \"0=success\" (sem: SEM-gx-cli-1362). stderr: {}",
        undone.stderr
    );
    // 44 §1.2: "stdout: the new `Transformation`'s `Receipt`" (sem: SEM-gx-cli-1363).
    assert!(
        undone.json()["envelope"]["payload"].is_string(),
        "an undo prints the new transformation's receipt: {}",
        undone.stdout
    );
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "🔴 P-5: the undo is a new commit of the escrowed inverse, and the escrowed inverse is what \
         restores the old content. A status check alone would pass on an implementation that \
         printed a receipt and touched nothing"
    );
    assert_eq!(
        undone.json()["superseded_state"],
        "Superseded",
        "43 T-12: `T_o` moves from `Committed` to `Superseded` when the undo commits"
    );
    assert_eq!(
        undone.json()["undone"],
        committed,
        "and the output names what was taken back"
    );

    // 42 §3.12's `Consumed`, across processes: the escrow row is rebuilt from Σ by
    // `Engine::rehydrate_committed`, **status included**, so a second undo is refused rather than
    // being allowed by a restart. A rebuild that reset the status to `Available` would make
    // "only once" false exactly where nobody was looking. (sem: SEM-gx-cli-1364)
    let again = run(fixture.gx().args(["undo", &committed]));
    println!(
        "UNDO_TWICE exit={} stderr={}",
        again.code,
        again.stderr.trim()
    );
    assert_ne!(
        again.code, 0,
        "a second undo of one commit is 42 §3.12's `Consumed`: {}",
        again.stdout
    );
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "and it changed nothing"
    );
}

/// 🔴 **AC-054's abnormal case for `undo`** — "6=not-found" (sem: SEM-gx-cli-1365), by number.
#[test]
fn ac_054_undo_of_an_unknown_transformation_is_six() {
    let fixture = pipeline("m6h4_undo_absent", "before\n");
    let undone = run(fixture.gx().args(["undo", ABSENT]));
    println!("UNDO_ABSENT exit={}", undone.code);
    assert_eq!(
        undone.code, 6,
        "44 §1.2 `gx undo`: \"6=not-found\" (sem: SEM-gx-cli-1366). stderr: {}",
        undone.stderr
    );
    assert!(
        undone.stdout.trim().is_empty(),
        "44 §1.3: a refusal writes nothing to stdout"
    );
    assert!(undone.stderr.contains("gx_code"));
}

/// 🔴 **M6-25's body** — a **denied** undo exits **2**, and the original stays `Committed`.
///
/// req/38 §47 M6-25 adopted (a)+(c) reads 44 §1.2's list (0/1/3/5/6) as an excerpt and §1.4's common table
/// as the answer, because AC-040's second case — "in the case where the invariant/policy corresponding
/// to T_u is deliberately set to Deny, T_u fails to reach Committed and stays Denied" (sem: SEM-gx-cli-1367) — has existed in the engine since M5 and
/// had no status of its own at the surface. Folding it into 1 would put "the escrowed inverse is
/// unusable" and "the gate refused to let you undo" under one number, and an operator's next move
/// differs completely between them.
///
/// The Given is built with **E-M6-12**'s `--policy`: the commit happens under the shipped pack, and
/// the undo is verified against a pack that refuses the locator. That is also the only honest way to
/// state "the policy changed between the commit and the undo" (sem: SEM-gx-cli-1368), which is the real-world shape of
/// this case.
#[test]
fn a_denied_undo_exits_two_and_leaves_the_original_committed() {
    let fixture = pipeline_named(
        "m6h4_undo_denied",
        "before\n",
        &format!("{DENIED_FRAGMENT}.txt"),
    );
    let committed = fixture.commit_one("after\n");
    assert_eq!(fixture.target_contents(), "after\n");

    let pack = deny_writable_pack();
    let undone = run(fixture
        .gx()
        .args(["undo", &committed])
        .arg("--policy")
        .arg(&pack));
    println!(
        "UNDO_DENIED exit={} state={:?} kind={:?} superseded_state={:?} target={:?}",
        undone.code,
        undone.json()["state"],
        undone.json()["kind"],
        undone.json()["superseded_state"],
        fixture.target_contents()
    );
    assert_eq!(
        undone.code, 2,
        "🔴 M6-25 adopted (a)+(c): 44 §1.4's 2 is \"refused (denied)\" and an undo has a verdict of its own \
         (43 §5-2: \"an undo is not exempt from verification either\"; sem: SEM-gx-cli-1369). stdout: {} stderr: {}",
        undone.stdout, undone.stderr
    );
    assert_eq!(undone.json()["state"], "Denied");
    assert_eq!(undone.json()["kind"], "Deny");
    assert_eq!(
        undone.json()["superseded_state"],
        "Committed",
        "43 §5-3 draws the supersede edge only when T_u reaches `Committed`; it did not"
    );
    // 🔴 **R44 item 4 (`req/324` §5(d), `req/334` M-01)** — the cause travels with the value on
    // **this** surface too. `halted` reads `Engine::rollback` into `detail`, and the paired
    // `rollback_not_attempted_because` is the member `rollback_facts` (gx-api's assembler) and
    // `pipeline.rs` (the commit road's twin) both carry beside the value, so a reader that branches
    // on the word can also see why it is what it is. It is `null` on a denied undo — this process
    // reached no `NotAttempted` abort of its own — which is the same "null when there is nothing to
    // say" the two other surfaces use, and the point is that the **member is here to read** rather
    // than only in the journal.
    assert!(
        undone.json().get("not_attempted_because").is_some(),
        "🔴 `req/334` M-01: the undo-refused surface put the roll-back word in `detail` and dropped \
         the cause. The member the two other surfaces carry is absent, so a reader here cannot tell \
         a `NotAttempted` that has a reason from one that does not — the value is on a signed record \
         and the accessor is one call away. stdout: {}",
        undone.stdout
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "and the world is where the commit left it"
    );
}

/// 🔴 **43 T-12's guard, checkable from the journal alone** — the probe req/88's brief asks for.
///
/// > `T_u.parents` contains `T_o.id`; undo's journal record is `Superseded`. Once E-M5-13 put
/// > `Planned.parents` into the journal (hand 1), "checkable from the journal alone" (sem: SEM-gx-cli-1370) became true -- that probe is placed
///
/// Before E-M5-13 the supersede metadata lived only in the in-memory `Transformation`, so a crash
/// between `Planned` and the commit lost it and the guard could not be re-checked from the log
/// (M5H6-6, which is why the field was added). This reads **bytes off disk** — no engine, no row —
/// and finds both halves: the parent edge the undo declared, and the record T-12 wrote when it fired.
#[test]
fn t_12s_guard_is_checkable_from_the_journal_alone() {
    let fixture = pipeline("m6h4_undo_journal", "before\n");
    let original = fixture.commit_one("after\n");
    let undone = run(fixture.gx().args(["undo", &original]));
    assert_eq!(undone.code, 0, "stderr: {}", undone.stderr);
    let undoing = undone.json()["transformation"]
        .as_str()
        .expect("the undo names its own transformation")
        .to_string();

    let records = fixture.journal();
    let mut parents_edge = 0usize;
    let mut supersede_records = 0usize;
    for record in &records {
        match record {
            gx_engine::EngineJournalRecord::Planned {
                transformation,
                parents,
                ..
            } if transformation.0.to_text() == undoing
                && parents.iter().any(|p| p.0.to_text() == original) =>
            {
                parents_edge += 1;
            }
            gx_engine::EngineJournalRecord::Superseded {
                transformation, by, ..
            } if transformation.0.to_text() == original && by.0.to_text() == undoing => {
                supersede_records += 1;
            }
            _ => {}
        }
    }
    println!(
        "T12_FROM_JOURNAL records={} parents_edge={parents_edge} superseded={supersede_records}",
        records.len()
    );
    assert_eq!(
        parents_edge, 1,
        "43 T-12's guard is \"`T_u.parents` contains `T_o.id`\" (sem: SEM-gx-cli-1371), and **E-M5-13** put `parents` in the \
         `Planned` record so that the guard survives a crash between planning and committing"
    );
    assert_eq!(
        supersede_records, 1,
        "and the edge itself is journalled once: \"journal: `Superseded{{T_o.id, by: T_u.id}}`\" (sem: SEM-gx-cli-1372). \
         Twice would mean T-12's idempotency column (\"if `superseded_by` is already set, do not re-set \
         it\") stopped holding"
    );
}

/// 🔴 **The rebuild's proof, exercised** — a draft that is not the one refuses rather than undoing
/// something else.
///
/// `Engine::rehydrate_committed` rebuilds a committed row out of Σ and the `Intent` the CLI holds,
/// and re-identifies the result: the rebuilt `Transformation`'s CID has to be the id that was asked
/// for. Nothing in the normal road can make that check fire — the draft the CLI finds is always the
/// right one — so the check is only worth having if something measures it, which is this.
///
/// # 🔴 Which field is corrupted, and why it is that one
///
/// The rebuild takes exactly **two** fields off the draft: `context` and `actor`. Everything else
/// comes from Σ (`intent_id`, `delta_cid`, `fp0`, `parents`, `locator`, the subject out of the
/// provenance) or from the blob store (the delta body). So `context` is corrupted here — and the
/// **goal deliberately is not**, because corrupting it would change nothing: a goal reaches the
/// identity only through `intent_id`, which the rebuild reads off the journal rather than
/// recomputing. That is not a hole. An undo applies the *escrowed inverse* from the blob store, so a
/// draft whose goal has rotted still undoes the right change; measured here as a fact about what
/// this check does and does not cover (the first spelling of this test corrupted the goal, and the
/// undo **succeeded** — correctly).
#[test]
fn a_draft_that_is_not_the_one_is_refused_rather_than_undone() {
    let fixture = pipeline("m6h4_undo_wrong_draft", "before\n");
    let original = fixture.commit_one("after\n");

    // The one draft this project holds, with a different `ChangeContext` inside it.
    let drafts = fixture.project.join(".gx").join("drafts");
    let entry = std::fs::read_dir(&drafts)
        .expect("the draft store exists")
        .filter_map(std::result::Result::ok)
        .find(|e| e.path().extension().is_some_and(|x| x == "json"))
        .expect("a draft was filed by `gx submit`");
    let raw = std::fs::read(entry.path()).expect("readable");
    let mut doc: serde_json::Value = serde_json::from_slice(&raw).expect("a draft is JSON");
    assert_eq!(
        doc["context"], "Evidence",
        "the fixture submits with `--context Evidence`: {doc}"
    );
    doc["context"] = serde_json::Value::String("Policy".to_string());
    std::fs::write(
        entry.path(),
        serde_json::to_vec_pretty(&doc).expect("serialises"),
    )
    .expect("write");

    let undone = run(fixture.gx().args(["undo", &original]));
    println!(
        "UNDO_WRONG_DRAFT exit={} target={:?} detail={:?}",
        undone.code,
        fixture.target_contents(),
        undone.stderr.trim()
    );
    assert_ne!(
        undone.code, 0,
        "the rebuild has to refuse a draft whose transformation is not the one named: {}",
        undone.stdout
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "🔴 and it refuses **before** applying anything. A rebuild that trusted the draft would \
         have undone a transformation the operator did not name, which is the failure the \
         re-identification exists to make impossible"
    );
}

/// 🔴 `--actor-key` is **refused**, because it selects nothing (M6H4-3).
///
/// M4H5-5's rule, one verb along from where hand 3 applied it to `--order`: an argument that changes
/// nothing must not look like an argument that worked. `Engine::undo` mints T_u's `Intent` with
/// T_o's own context and actor (43 §5-1, P-5), so the receipt is signed under the original actor's
/// key whatever this flag says.
#[test]
fn the_undo_flag_with_nowhere_to_go_is_refused() {
    let fixture = pipeline("m6h4_undo_actor_key", "before\n");
    let committed = fixture.commit_one("after\n");
    let refused = run(fixture
        .gx()
        .args(["undo", &committed])
        .args(["--actor-key", "key-somebody-else"]));
    println!(
        "UNDO_ACTOR_KEY exit={} detail={:?}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(
        refused.code, 1,
        "discipline 52: a usage error is \"invalid input\" (sem: SEM-gx-cli-1373)"
    );
    assert!(
        refused.stderr.contains("M6H4-3"),
        "the refusal names the ticket that carries the question: {}",
        refused.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "a refused flag undoes nothing"
    );
}
