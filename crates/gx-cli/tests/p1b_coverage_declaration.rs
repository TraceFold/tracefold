// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1b / AC-5 (a), AC-9, AC-11, AC-14** (`req/544` §5) — the coverage declaration as a
//! **process** answers it, with a control beside every claim.
//!
//! # What this suite is about
//!
//! `req/544` §0 asks three things of the face P-1a placed: that it answer the four questions with
//! values rather than omissions, that it never let a declaration become a measurement, and that it
//! say out loud which of its own conditions are unmet. The first is bytes and lives in
//! `gx-witness/tests/p1b_coverage_wire.rs`. The other two are what a person runs, so they are
//! measured here by running the binary.
//!
//! # 🔴 Every probe carries its control, and the controls came first
//!
//! `req/538` §2-2 is this lane's inherited lesson: P-1a's tracked-state comparison was green while
//! measuring nothing, and the control is what found it. So each probe below states what would have
//! to be true for it to be vacuous, and then builds that state and watches the same predicate
//! refuse it.

// 🔴 `req/817`: every test here drives `gx attach's face`, whose mechanism is
// `gx-mcp-wire` -- one of the four crates `req/789` §3 holds private. The public
// distribution does not carry the verb, so the suite compiles away rather than failing against
// a subcommand that is deliberately absent. The private build runs it exactly as before.
#![cfg(feature = "mcp")]

mod support;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use support::{run, scratch, write_json, Run};

/// The frozen attach answer, from the specimen this lane froze before it changed anything.
fn frozen_attach() -> Value {
    let raw = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("attach_face_frozen")
            .join("issued_2026_08_22")
            .join("attach.json"),
    )
    .expect("the frozen attach answer is here");
    serde_json::from_slice(&raw).expect("it is `gx attach`'s answer")
}

/// One frozen receipt, by name.
fn frozen_receipt(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("attach_face_frozen")
        .join("issued_2026_08_22")
        .join(name)
}

/// An agent configuration whose entry for `notes` runs through `gx wrap` and has no second entry
/// starting the same server directly — B-1's passing state, written the way an agent writes it.
fn routed_config(dir: &Path) -> PathBuf {
    write_json(
        &dir.join("agent.json"),
        &json!({
            "mcpServers": {
                "notes": {
                    "command": "gx",
                    "args": ["wrap", "--", "notes-server", "--stdio"],
                }
            }
        }),
    )
}

/// `gx attach` against a fresh project, with whatever flags the caller adds.
fn attach(name: &str, extra: &[&str]) -> (PathBuf, Run) {
    let project = scratch(name);
    let mut cmd = support::gx();
    cmd.arg("--project").arg(&project).arg("attach");
    for arg in extra {
        cmd.arg(arg);
    }
    let out = run(&mut cmd);
    (project, out)
}

/// 🔴 The predicate AC-14 is measured with: are the two surviving sentences the ones the specimen
/// froze, and is the third gone from the list and named as answered?
///
/// The expected strings come from the **frozen document**, never from a constant in this file. A
/// probe that pinned its own copy would go green the day somebody edited both.
fn the_two_survivors_are_verbatim(answer: &Value, frozen: &Value) -> Result<(), String> {
    let frozen_three: Vec<String> = frozen["not_carried_by_this_face"]
        .as_array()
        .ok_or("the frozen answer has no `not_carried_by_this_face`")?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    if frozen_three.len() != 3 {
        return Err(format!(
            "the frozen answer named {} unanswered items, not three",
            frozen_three.len()
        ));
    }
    let now: Vec<String> = answer["not_carried_by_this_face"]
        .as_array()
        .ok_or("this answer has no `not_carried_by_this_face`")?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    // 🔴 **P-1c** (`req/551` D-11 / AC-20) — the frozen third sentence said the operation had no
    // inverse. One exists now, and `req/38` §320 gives that sentence to the detach face and to no
    // other. So the list still holds two, the **first** is still the frozen first verbatim — that
    // half of this probe's claim is untouched — and the second is the detach face's answer, which
    // is checked here for the properties P-1b's claim depends on rather than pinned as a string
    // this file would then own a second copy of. `p1c_detach.rs` pins its wording.
    if now.len() != 2 {
        return Err(format!("two sentences should survive, not {}", now.len()));
    }
    if now[0] != frozen_three[0] {
        return Err(format!(
            "the first surviving sentence is not the frozen first, verbatim. now={now:?}"
        ));
    }
    if !now[1].starts_with("how to leave: ") {
        return Err(format!(
            "the second surviving sentence is no longer the one about leaving. now={now:?}"
        ));
    }
    if now[1] == frozen_three[2] {
        return Err(
            "the sentence about leaving still says there is no inverse, but `--detach-config` \
             exists (`req/551` D-11)"
                .to_string(),
        );
    }
    let replaced = answer["now_answered_by_coverage"]
        .as_str()
        .ok_or("the answer does not say which item became a table")?;
    if replaced != frozen_three[1] {
        return Err("the item named as answered is not the frozen second one".to_string());
    }
    // 🔴 And the retired third is readable in the answer, the way the retired second is.
    let retired = answer["now_answered_by_detach"]
        .as_str()
        .ok_or("the answer does not say what the sentence about leaving used to say")?;
    if retired != frozen_three[2] {
        return Err(
            "the sentence named as answered by the detach face is not the frozen third one"
                .to_string(),
        );
    }
    if answer["coverage"].is_null() {
        return Err("there is no coverage table, so nothing replaced the second item".to_string());
    }
    Ok(())
}

/// 🔴 **AC-14** — the second sentence became a table, and the first and third did not move.
#[test]
fn only_the_second_unanswered_sentence_was_replaced() {
    let (_, out) = attach("p1b_ac14", &[]);
    assert_eq!(out.code, 0, "attach runs: {}", out.stderr);
    let answer = out.json();
    let frozen = frozen_attach();
    println!(
        "AC14_NOT_CARRIED n={} coverage_present={}",
        answer["not_carried_by_this_face"]
            .as_array()
            .map_or(0, Vec::len),
        !answer["coverage"].is_null()
    );
    the_two_survivors_are_verbatim(&answer, &frozen).unwrap_or_else(|why| {
        panic!(
            "🔴 R-3g: P-1b answers the second of P-1a's three unanswered items and leaves the \
             other two exactly as they were. {why}"
        )
    });

    // 🔴 The control `req/544` AC-14 names: an implementation that removed all three. It reads as
    // an attach that answers the route and the exit as well, and the predicate refuses it.
    let mut all_three_gone = answer.clone();
    all_three_gone["not_carried_by_this_face"] = json!([]);
    let refused = the_two_survivors_are_verbatim(&all_three_gone, &frozen);
    println!("AC14_CONTROL_ALL_THREE_GONE={refused:?}");
    assert!(
        refused.is_err(),
        "🔴 an answer that dropped all three unanswered items passed the predicate, so the \
         predicate is not measuring what survived"
    );

    // And the second control: the first sentence quietly reworded.
    let mut reworded = answer.clone();
    reworded["not_carried_by_this_face"] = json!(["something else entirely", "how to leave: ..."]);
    assert!(
        the_two_survivors_are_verbatim(&reworded, &frozen).is_err(),
        "a reworded survivor is not a verbatim survivor"
    );
}

/// 🔴 **AC-5 (a)** — a declaration cannot put a measurement anywhere, and the refusal says so.
#[test]
fn a_declaration_offering_a_measurement_is_refused() {
    let dir = scratch("p1b_ac5a");

    // The legitimate file: somebody writes down what they believe about two questions.
    let honest = write_json(
        &dir.join("declared.json"),
        &json!({
            "what_was_read": {
                "value": "declared",
                "source": "the project's own README",
                "claim": "this server only ever reads the notes directory",
            },
            "by_whose_authority": {"value": "unknown", "why": "actor_not_in_receipt"},
        }),
    );
    let (project, out) = attach(
        "p1b_ac5a_project",
        &["--declared", honest.to_str().expect("a path")],
    );
    assert_eq!(
        out.code, 0,
        "an honest declaration is accepted: {}",
        out.stderr
    );
    let answer = out.json();

    // 🔴 The measured column is untouched by it. The declaration named `what_was_read`, and the
    // face still says it cannot measure that question, because no route was offered.
    let rows = answer["coverage"]["posture"]
        .as_array()
        .expect("the posture table");
    let read_row = rows
        .iter()
        .find(|row| row["question"] == json!("what_was_read"))
        .expect("the read row");
    println!("AC5A_ROW_AFTER_DECLARATION={read_row}");
    assert_eq!(
        read_row["posture"],
        json!("cannot-measure"),
        "🔴 a declaration about a question does not move the face's posture on it. The posture is \
         derived from the route, and no route was offered here"
    );
    assert_eq!(
        read_row["declared"]["value"],
        json!("declared"),
        "and what was written down is carried, in its own column"
    );
    // Every word in the file is a face-level word, and none of them is `measured`.
    let printed = serde_json::to_string(&answer["coverage"]).expect("serialises");
    assert!(
        !printed.contains("\"measured\""),
        "🔴 `req/38` §313 ruling 2: no measured value appears in a face declaration. {printed}"
    );
    // The side-car on disk carries the same, and nothing more.
    let side_car = answer["coverage_side_car"]
        .as_str()
        .expect("a side-car path");
    let written = std::fs::read_to_string(side_car).expect("the side-car is on disk");
    println!("AC5A_SIDE_CAR_BYTES={}", written.len());
    assert!(
        !written.contains("\"measured\""),
        "and the file on disk carries no measurement either: {written}"
    );
    assert!(
        Path::new(side_car).starts_with(&project),
        "the side-car is inside the project it is about"
    );

    // 🔴 The control: the same file, offering a measurement.
    let promoting = write_json(
        &dir.join("promoting.json"),
        &json!({
            "what_was_read": {
                "value": "measured",
                "from": ["read_set"],
                "reading": "G3 over everything",
                "not_covered": "nothing at all",
            }
        }),
    );
    let (_, refused) = attach(
        "p1b_ac5a_control",
        &["--declared", promoting.to_str().expect("a path")],
    );
    println!(
        "AC5A_CONTROL exit={} stderr={:?}",
        refused.code,
        refused.stderr.trim()
    );
    assert_ne!(
        refused.code, 0,
        "🔴 AC-5: a declaration file offering a measured value is refused. If this is accepted, \
         the measured column has a second address and `req/38` §313's design (C) is not what is \
         implemented"
    );
    assert!(
        refused.stderr.contains("measured"),
        "and the refusal names what was wrong rather than failing generically: {}",
        refused.stderr
    );
}

/// 🔴 The predicate AC-9 is measured with: is the unmet list a **derivation** of the table, or a
/// list somebody wrote?
///
/// It recomputes the list from the posture rows and compares. A change that hides an unmet row has
/// to change the table it was derived from, and the table is what the other probes pin.
fn unmet_is_derived_from_the_table(coverage: &Value, unmet: &[String]) -> Result<(), String> {
    let mut expected: Vec<String> = coverage["posture"]
        .as_array()
        .ok_or("no posture table")?
        .iter()
        .filter(|row| row["posture"] != json!("can-measure"))
        .map(|row| row["question"].as_str().unwrap_or_default().to_string())
        .collect();
    expected.sort();
    let mut offered = unmet.to_vec();
    offered.sort();
    if expected == offered {
        return Ok(());
    }
    Err(format!(
        "the unmet list is not the table's own projection: table says {expected:?}, list says \
         {offered:?}"
    ))
}

/// 🔴 **AC-9** — gx says which of its own conditions this face does not meet, and the count is not
/// asserted to be zero.
///
/// `req/544` AC-9 is explicit about the last part: a probe that required `n == 0` would reward an
/// implementation that stopped counting. What is asserted is that the number is **printed**, that
/// every row is named, and that the list is derived from the table rather than typed out.
#[test]
fn the_attach_face_says_which_of_its_own_conditions_are_unmet() {
    let dir = scratch("p1b_ac9");
    let config = routed_config(&dir);

    for (name, extra, label) in [
        ("p1b_ac9_unrouted", vec![], "no route"),
        (
            "p1b_ac9_routed",
            vec![
                "--route-config".to_string(),
                config.to_str().expect("a path").to_string(),
                "--server-name".to_string(),
                "notes".to_string(),
            ],
            "routed through gx",
        ),
    ] {
        let borrowed: Vec<&str> = extra.iter().map(String::as_str).collect();
        let (_, out) = attach(name, &borrowed);
        assert_eq!(out.code, 0, "attach runs ({label}): {}", out.stderr);
        let answer = out.json();
        let coverage = &answer["coverage"];
        let unmet: Vec<String> = coverage["posture"]
            .as_array()
            .expect("the posture table")
            .iter()
            .filter(|row| row["posture"] != json!("can-measure"))
            .map(|row| row["question"].as_str().unwrap_or_default().to_string())
            .collect();
        println!("ATTACH_FACE_UNMET={} face={label}", unmet.len());
        for question in &unmet {
            let row = coverage["posture"]
                .as_array()
                .expect("rows")
                .iter()
                .find(|row| row["question"] == json!(question.as_str()))
                .expect("the row");
            println!(
                "ATTACH_FACE_UNMET_ROW {question} posture={}",
                row["posture"]
            );
        }
        unmet_is_derived_from_the_table(coverage, &unmet).expect("the list is the table's own");
    }

    // 🔴 The first known unmet row, on the receipt side, and it is known for a reason that is in
    // the type: no receipt this binary can build answers `by whose authority`.
    let out = run(support::gx()
        .arg("receipt")
        .arg("coverage")
        .arg(frozen_receipt("commit_receipt.json")));
    assert_eq!(out.code, 0, "the coverage of a receipt: {}", out.stderr);
    let table = out.json();
    let unmet: Vec<&str> = table["unmet"]
        .as_array()
        .expect("the unmet list")
        .iter()
        .map(|row| row["question"].as_str().unwrap_or_default())
        .collect();
    println!("RECEIPT_FACE_UNMET={} {unmet:?}", unmet.len());
    assert_eq!(
        unmet,
        vec!["by_whose_authority"],
        "🔴 AC-9's first known row: a `CommitReceipt` from an attached project answers three of \
         the four questions and says so about the fourth"
    );

    // 🔴 The control `req/544` AC-9 names: an implementation that quietly passed a face whose
    // conditions are unmet. Here it is the same predicate with a row hidden from the list.
    let (_, out) = attach("p1b_ac9_control", &[]);
    let coverage = out.json()["coverage"].clone();
    let hidden: Vec<String> = vec!["when".to_string()];
    let refused = unmet_is_derived_from_the_table(&coverage, &hidden);
    println!("AC9_CONTROL_HIDDEN_ROWS={refused:?}");
    assert!(
        refused.is_err(),
        "🔴 a shortened unmet list passed the derivation check, so the check is not tying the list \
         to the table and a row could be hidden without the table moving"
    );
}

/// 🔴 The predicate AC-11 is measured with: do the two levels use two vocabularies?
fn the_two_levels_use_different_words(face: &Value, receipt: &Value) -> Result<(), String> {
    let face_words: Vec<String> = face["posture"]
        .as_array()
        .ok_or("no posture table")?
        .iter()
        .map(|row| row["posture"].as_str().unwrap_or_default().to_string())
        .collect();
    let receipt_words: Vec<String> = receipt["rows"]
        .as_array()
        .ok_or("no receipt table")?
        .iter()
        .map(|row| {
            row["answer"]["value"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    let shared: Vec<&String> = face_words
        .iter()
        .filter(|word| receipt_words.contains(word))
        .collect();
    if shared.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the two levels share {shared:?}, so a claim and an observation are spelled alike"
    ))
}

/// 🔴 **AC-11** — a face that claims it can measure and a receipt that answers `unknown` are both
/// printed, in different words, and the pair is not a refusal.
#[test]
fn a_face_claim_and_a_receipt_answer_are_two_different_statements() {
    let dir = scratch("p1b_ac11");
    let config = routed_config(&dir);
    let (project, out) = attach(
        "p1b_ac11_project",
        &[
            "--route-config",
            config.to_str().expect("a path"),
            "--server-name",
            "notes",
        ],
    );
    assert_eq!(out.code, 0, "attach runs: {}", out.stderr);
    let side_car = out.json()["coverage_side_car"]
        .as_str()
        .expect("a side-car path")
        .to_string();
    assert!(
        Path::new(&side_car).starts_with(&project),
        "the face declaration is inside the project"
    );

    // The `VerdictReceipt`: every one of its four questions is unanswered, by construction — the
    // escrow has not run at verdict time. The face above claims it can measure three of them.
    let out = run(support::gx()
        .arg("receipt")
        .arg("coverage")
        .arg(frozen_receipt("verdict_receipt.json"))
        .arg("--face")
        .arg(&side_car));
    println!("AC11 exit={} {}", out.code, out.stdout.trim());
    assert_eq!(
        out.code, 0,
        "🔴 AC-11: a face claiming `can-measure` and a receipt answering `unknown` is a legitimate \
         state and is answered, not refused: {}",
        out.stderr
    );
    let answer = out.json();
    let face = answer["face_claim"].clone();
    the_two_levels_use_different_words(&face, &answer)
        .expect("the two levels are spelled differently");

    // The pair AC-11 is named for, printed as a pair.
    let claim = face["posture"]
        .as_array()
        .expect("the posture table")
        .iter()
        .find(|row| row["question"] == json!("what_was_read"))
        .expect("the read row")["posture"]
        .clone();
    let answered = answer["rows"]
        .as_array()
        .expect("the receipt table")
        .iter()
        .find(|row| row["question"] == json!("what_was_read"))
        .expect("the read row")["answer"]["value"]
        .clone();
    println!("AC11_PAIR face={claim} receipt={answered}");
    assert_eq!(claim, json!("can-measure"));
    assert_eq!(answered, json!("unknown"));

    // 🔴 The control: an implementation that printed the face's claim as the receipt's answer.
    let mut collapsed = answer.clone();
    collapsed["rows"] = json!([{
        "question": "what_was_read",
        "answer": {"value": "can-measure"},
    }]);
    let refused = the_two_levels_use_different_words(&face, &collapsed);
    println!("AC11_CONTROL_COLLAPSED={refused:?}");
    assert!(
        refused.is_err(),
        "🔴 a receipt table printing a face word passed the predicate, so the predicate is not \
         measuring the separation the two vocabularies exist for"
    );
}

/// 🔴 The receipt table is a **reading**, not a report: two different face declarations over the
/// same receipt do not move a single answer.
///
/// `req/544` F-3's observation condition. If the receipt's answers moved with the face file, the
/// table would not be a projection of the receipt and design (C) would be false about its own
/// central property.
#[test]
fn the_receipt_answer_does_not_move_when_the_face_does() {
    let dir = scratch("p1b_f3");
    let config = routed_config(&dir);
    let (_, attached) = attach(
        "p1b_f3_project",
        &[
            "--route-config",
            config.to_str().expect("a path"),
            "--server-name",
            "notes",
        ],
    );
    let routed_face = attached.json()["coverage_side_car"]
        .as_str()
        .expect("a side-car")
        .to_string();

    // A second face over the same receipt: a configuration in which the direct road is still there.
    let bypassed = write_json(
        &dir.join("bypassed.json"),
        &json!({
            "mcpServers": {
                "notes": {"command": "gx", "args": ["wrap", "--", "notes-server", "--stdio"]},
                "notes-direct": {"command": "notes-server", "args": ["--stdio"]},
            }
        }),
    );
    let (_, second) = attach(
        "p1b_f3_second",
        &[
            "--route-config",
            bypassed.to_str().expect("a path"),
            "--server-name",
            "notes",
        ],
    );
    let bypassed_face = second.json()["coverage_side_car"]
        .as_str()
        .expect("a side-car")
        .to_string();

    let mut answers = Vec::new();
    for face in [&routed_face, &bypassed_face] {
        let out = run(support::gx()
            .arg("receipt")
            .arg("coverage")
            .arg(frozen_receipt("commit_receipt.json"))
            .arg("--face")
            .arg(face));
        assert_eq!(out.code, 0, "the coverage reads: {}", out.stderr);
        answers.push(out.json()["rows"].clone());
    }
    println!(
        "F3_ROWS_UNDER_TWO_FACES identical={}",
        answers[0] == answers[1]
    );
    assert_eq!(
        answers[0], answers[1],
        "🔴 F-1/F-3: the receipt's four answers moved when a different face declaration was \
         offered. That would make the table a report about two inputs rather than a projection of \
         one, and `req/544` §9-4 step 3 sends the design back to Fable rather than patching it here"
    );

    // The control: the two faces really are different documents, or the equality above is trivial.
    let first: Value = serde_json::from_slice(&std::fs::read(&routed_face).expect("read"))
        .expect("a face declaration");
    let second: Value = serde_json::from_slice(&std::fs::read(&bypassed_face).expect("read"))
        .expect("a face declaration");
    println!(
        "F3_CONTROL_FACES_DIFFER={}",
        first["posture"] != second["posture"]
    );
    assert_ne!(
        first["posture"], second["posture"],
        "the two face declarations have to differ, or the invariance above is measuring nothing"
    );
}
