// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — one arm per **cause**, the way R25 built one
//! arm per **value**.
//!
//! # What broke
//!
//! R25 replaced a clause that was false on one of three values of `Engine::rollback` with one arm
//! per value, and then had the `NotAttempted` arm assert a **cause**:
//!
//! > *the escrowed one was still partial — a member of it is filled from what the call answers*
//!
//! That clause is a function of `detail` alone, and `detail` is the word `NotAttempted`. The
//! engine constructs that word at **three** places and the clause describes **one** of them. The
//! other two are different facts: an escrow that was never built at all (`invert` answered `None`,
//! which `gx-engine`'s own comment says E-M3-4 makes reachable), and `gx repair`'s
//! `RecoveryPath::ApplyWasAnnounced`. On those roads the sentence hands the agent a confident,
//! specific account of an escrow row that does not exist.
//!
//! The twenty-fifth audit filed this as structure and refused to grade it, because it did not
//! drive the other two roads (`req/324` §9-5). `req/38` §231 ruling 5 splits the work: the
//! **structure** is repaired here, and the twenty-sixth audit's first target is driving the roads.
//!
//! # What this file requires
//!
//! The cause travels with the value. Every place the engine constructs `Rollback::NotAttempted`
//! declares **why**, the set of causes is enumerable, and the proxy has one arm for each of them
//! plus the arm for a cause this build does not know — which is the same shape, and the same
//! reason, as R25's arm for a `Rollback` value this build does not know.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

use std::path::{Path, PathBuf};

use gx_cli::wrap::apply_failed_clause;
use serde_json::json;

fn engine_src(file: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gx-engine")
        .join("src")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn cli_src(file: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Lines that are not comments: the record of *why* an arm exists names the variants too.
fn code_of(src: &str) -> Vec<String> {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(str::to_string)
        .collect()
}

/// Every line of `pipeline.rs` that **constructs** the third rollback value, with its index.
///
/// A construction, not a match arm: `Some((_, _, true)) => Rollback::NotAttempted` constructs;
/// `Some("NotAttempted") =>` in the proxy matches on the word. The two are told apart by the
/// `Rollback::` path, which only the constructing side spells.
fn construction_sites(lines: &[String]) -> Vec<(usize, String)> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("Rollback::NotAttempted"))
        .filter(|(_, l)| !l.contains("enum ") && !l.contains("=> \"NotAttempted\""))
        .map(|(i, l)| (i, l.trim().to_string()))
        .collect()
}

/// 🔴 The bed control: the sites this file is about are still there and still three.
#[test]
fn the_engine_still_constructs_the_third_value_at_three_places() {
    let lines = code_of(&engine_src("pipeline.rs"));
    let sites = construction_sites(&lines);
    println!(
        "R26_NOTATTEMPTED construction_sites={} {:?}",
        sites.len(),
        sites
            .iter()
            .map(|(i, l)| format!("{}: {l}", i + 1))
            .collect::<Vec<_>>()
    );
    assert!(
        sites.len() >= 3,
        "🔴 `req/324` §5(d) rests on the engine constructing this value at more than one place. \
         Found {}: if the count changed, this file has to be re-derived rather than re-run.",
        sites.len()
    );
}

/// 🔴 **`req/324` §5(d)** — every construction of the value declares its cause.
#[test]
fn every_construction_of_the_third_value_names_its_cause() {
    let lines = code_of(&engine_src("pipeline.rs"));
    let sites = construction_sites(&lines);
    let mut silent: Vec<String> = Vec::new();
    let mut named: Vec<String> = Vec::new();
    for (at, line) in &sites {
        // The cause is declared beside the construction: the window is small on purpose, because a
        // cause recorded ten lines away is a cause a reader has to hunt for.
        let lo = at.saturating_sub(8);
        let hi = (at + 8).min(lines.len());
        let window = lines[lo..hi].join("\n");
        if window.contains("NotAttemptedBecause::") {
            named.push(format!("{}: {line}", at + 1));
        } else {
            silent.push(format!("{}: {line}", at + 1));
        }
    }
    println!("R26_NOTATTEMPTED named={named:?} silent={silent:?}");
    assert!(
        silent.is_empty(),
        "🔴 `req/38` §231 ruling 5: the proxy's sentence for this value asserts a cause, and the \
         value is constructed at {} places with {} different causes -- a partial escrow, an escrow \
         that was never built, and a recovery path that rebuilt nothing. A construction that does \
         not say which one it is leaves the sentence free to describe the wrong one: {silent:?}",
        sites.len(),
        sites.len()
    );
}

/// 🔴 The proxy has one arm per cause, and the arm for a cause it does not know.
#[test]
fn the_proxy_has_one_arm_per_cause_and_one_for_a_cause_it_does_not_know() {
    let engine = engine_src("store.rs");
    let causes: Vec<String> = engine
        .lines()
        .skip_while(|l| !l.contains("pub enum NotAttemptedBecause"))
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('}'))
        .filter(|l| !l.trim_start().starts_with("//") && !l.trim_start().starts_with("#["))
        .map(|l| l.trim().trim_end_matches(',').to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let wrap = cli_src("wrap.rs");
    let mut without_an_arm: Vec<String> = Vec::new();
    for cause in &causes {
        if !wrap.contains(&format!("\"{cause}\"")) {
            without_an_arm.push(cause.clone());
        }
    }
    println!("R26_NOTATTEMPTED causes={causes:?} without_an_arm={without_an_arm:?}");
    assert!(
        causes.len() >= 3,
        "🔴 the engine declares the causes; this arm found {:?}",
        causes
    );
    assert!(
        without_an_arm.is_empty(),
        "🔴 `req/38` §231 ruling 5: one arm per cause. {without_an_arm:?} has no arm in `wrap.rs`, \
         so a road reaching it falls into whichever arm was written last -- which is the failure \
         this whole repair is about."
    );
}

// ---------------------------------------------------------------------------------------------
// The behavioural half: the sentence itself, one cause at a time
// ---------------------------------------------------------------------------------------------

fn clause(detail: &str, because: Option<&str>) -> String {
    let mut commit = json!({ "reason": "ApplyFailed", "detail": detail });
    if let Some(because) = because {
        commit["not_attempted_because"] = json!(because);
    }
    apply_failed_clause(&commit)
}

/// 🔴 The partial-escrow cause keeps R25's sentence, verbatim in its load-bearing half.
#[test]
fn the_partial_escrow_cause_says_the_escrow_was_partial() {
    let said = clause("NotAttempted", Some("EscrowStillPartial"));
    println!("R26_CLAUSE partial={said:?}");
    assert!(
        said.contains("no compensating inverse was sent"),
        "R25's repair stays: the sentence does not claim an attempt: {said}"
    );
    assert!(
        said.contains("still partial"),
        "and on this cause the account of *why* is the one R25 wrote: {said}"
    );
}

/// 🔴 **`req/324` §5(d)** — the cause where no escrow was ever built does **not** describe an
/// escrow row.
#[test]
fn the_cause_with_no_escrow_does_not_describe_a_partial_one() {
    let said = clause("NotAttempted", Some("NoInverseWasEscrowed"));
    println!("R26_CLAUSE no_escrow={said:?}");
    assert!(
        said.contains("no compensating inverse was sent"),
        "the two facts that do not depend on the cause stay put: {said}"
    );
    assert!(
        !said.contains("still partial"),
        "🔴 `req/324` §5(d): on this road `invert` answered `None` and there is no escrowed \
         inverse at all. Telling the agent that *the escrowed one was still partial* is a \
         confident, specific account of a row that does not exist: {said}"
    );
    assert!(
        said.contains("no inverse was escrowed") || said.contains("never built"),
        "and the arm says what did happen rather than only what did not: {said}"
    );
}

/// 🔴 The recovery cause is its own fact too: `gx repair` closed a row it never rebuilt.
#[test]
fn the_recovery_cause_says_the_recovery_rebuilt_nothing() {
    let said = clause("NotAttempted", Some("RecoveredWithoutRebuilding"));
    println!("R26_CLAUSE recovery={said:?}");
    assert!(
        !said.contains("still partial"),
        "🔴 this road is `gx repair`'s, not the apply path's: {said}"
    );
    assert!(
        said.contains("repair") || said.contains("recover"),
        "and it names the road it came from: {said}"
    );
}

/// 🔴 A cause this build does not know gets the arm R25 wrote for a value it does not know: the
/// proxy declines to put words in the engine's mouth.
#[test]
fn a_cause_this_build_does_not_know_is_not_guessed_at() {
    for because in [None, Some("SomethingNobodyHasWrittenYet")] {
        let said = clause("NotAttempted", because);
        println!("R26_CLAUSE unknown because={because:?} said={said:?}");
        assert!(
            !said.contains("still partial"),
            "🔴 an unrecognised cause must not inherit the arm written for another one: {said}"
        );
        assert!(
            said.contains("no compensating inverse was sent"),
            "the fact the value itself carries is still said: {said}"
        );
    }
}

/// 🔴 The control: the other two rollback values are untouched by this repair.
#[test]
fn the_other_two_rollback_values_are_unchanged() {
    let succeeded = clause("Succeeded", None);
    let failed = clause("Failed", None);
    println!("R26_CLAUSE succeeded={succeeded:?}");
    println!("R26_CLAUSE failed={failed:?}");
    assert!(
        succeeded.contains("the compensating inverse was sent")
            && succeeded.contains("the adapter"),
        "R25's `Succeeded` arm: {succeeded}"
    );
    assert!(
        failed.contains("came back an error"),
        "R25's `Failed` arm: {failed}"
    );
    assert!(
        !succeeded.contains("still partial") && !failed.contains("still partial"),
        "and neither of them borrows the partial-escrow account"
    );
}

/// 🔴 The control that keeps the clause a function of `ApplyFailed`: another reason gets nothing.
#[test]
fn a_commit_that_is_not_an_apply_failure_gets_no_clause() {
    let said = apply_failed_clause(&json!({ "reason": "PreconditionChanged", "detail": "x" }));
    println!("R26_CLAUSE not_apply_failed={said:?}");
    assert!(
        said.is_empty(),
        "the clause belongs to one reason: {said:?}"
    );
}
