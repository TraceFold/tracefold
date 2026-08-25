// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1c** (`req/551` §4) — the detach face: AC-6, AC-7, AC-15 through AC-18, AC-20.
//!
//! # What these probes are for
//!
//! `req/535` §2 sold an attach as "a reversible operation". This suite is where that word is made
//! to survive a machine, and the interesting assertions are the ones about what does **not** come
//! back: an operation that could only demonstrate its successes would be demonstrating the easy
//! half. Each probe below has a control that shows the assertion can go red.
//!
//! # The measurement that changed the design (`req/551` G-1 / G-2)
//!
//! Two of the reqdef's four falsifiers fired when they were measured rather than derived, and both
//! are pinned here so that they cannot quietly un-fire:
//!
//! * **G-1** — a wrap flag whose *value* is the string `--` puts a `--` in front of the separator
//!   `--adopt-config` writes. A reverse operation that searched for the first `--` would read the
//!   separator as the command and write `"command": "--"` into an operator's file.
//!   [`a_flag_whose_value_is_the_separator_still_round_trips`] is that case, and it round-trips
//!   because `ADOPT_FLAG_NAMES` lets the split read the flags positionally instead of guessing.
//! * **G-2** — `req/551` §1-3 derived from a grep over this workspace's manifests that key order is
//!   scrambled by an adoption. It is not: `serde_json` is built here with `preserve_order`, by way
//!   of a dependency rather than by this workspace's own declaration.
//!   [`the_document_keeps_its_key_order_through_a_round_trip`] measures the property itself, so
//!   that if the dependency that enables it ever drops it, this suite says so rather than the
//!   `not_restored` declaration quietly becoming an understatement.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

mod support;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use support::{gx, run, scratch};

/// Write a config file and hand back its path.
fn config(dir: &Path, name: &str, body: &Value) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_vec_pretty(body).expect("serialise")).expect("write");
    path
}

/// A one-server document, in the shape the stdio clients keep.
fn one_server(args: &[&str]) -> Value {
    json!({"mcpServers": {"s": {"command": "/usr/bin/orig", "args": args}}})
}

/// Read a config file back.
fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("read")).expect("parse")
}

/// `Run` carries no `Debug` and `support/mod.rs` is not this lane's to edit, so failures print the
/// two fields that matter from here.
fn why(out: &support::Run) -> String {
    format!(
        "exit={} stdout={:?} stderr={:?}",
        out.code, out.stdout, out.stderr
    )
}

/// `gx wrap --<mode>-config <path> --server-name s`, plus whatever else the caller wants.
fn wrap_mode(mode: &str, path: &Path, extra: &[&str]) -> support::Run {
    let mut cmd = gx();
    cmd.arg("wrap")
        .arg(format!("--{mode}-config"))
        .arg(path)
        .arg("--server-name")
        .arg("s");
    for arg in extra {
        cmd.arg(arg);
    }
    run(&mut cmd)
}

// ---------------------------------------------------------------------------------------------
// AC-6 — the two readings of "idempotent", both printed
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-6** (`req/38` §320 ruling 3, request 1) — an adoption is idempotent **in its state**, and
/// deliberately not in its exit.
///
/// The canon is state idempotence: applying `--adopt-config` twice leaves the file exactly as one
/// application left it, byte for byte. The second application still refuses, because putting gx in
/// front of gx is not a thing an operator can have meant, and a refusal that changed nothing is the
/// honest way to say so.
///
/// 🔴 **The exit is 1, not 2.** `req/551` §1-2 and `req/38` §320 both wrote 2 (`DENIED`), and both
/// are wrong on the number: `crate::exit`'s module documentation reserves 2 for `Verdict::Deny` and
/// nothing else, and `exit_map.rs` asserts no usage error takes it. The substance of the ruling is
/// untouched — the second application refuses and changes nothing — but the number a script would
/// branch on is 1, so this probe prints both readings rather than asserting the one the reqdef
/// expected.
#[test]
fn adopting_twice_is_idempotent_in_state_and_refuses_in_exit() {
    let dir = scratch("p1c_ac6");
    let path = config(&dir, "c.json", &one_server(&["one"]));

    let first = wrap_mode("adopt", &path, &[]);
    assert_eq!(
        first.code,
        0,
        "the first adoption succeeds: {}",
        why(&first)
    );
    let after_first = std::fs::read(&path).expect("read");

    let second = wrap_mode("adopt", &path, &[]);
    let after_second = std::fs::read(&path).expect("read");

    println!("AC6_STATE_IDEMPOTENT={}", after_first == after_second);
    println!("AC6_SECOND_EXIT={}", second.code);
    println!("AC6_FIRST_EXIT={}", first.code);

    assert_eq!(
        after_first, after_second,
        "state idempotence is the canon: a second adoption must not change one byte of the file"
    );
    assert_eq!(
        second.code,
        gx_cli::exit::ERROR as i32,
        "a second adoption refuses as invalid input. It is **not** {} (`DENIED`), which this \
         binary reserves for a gate's `Verdict::Deny`",
        gx_cli::exit::DENIED
    );
    assert!(
        second.stderr.contains("in front of gx"),
        "the refusal says why: {}",
        second.stderr
    );
}

// ---------------------------------------------------------------------------------------------
// AC-7 — the face returns to the answer it gave before the attach
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-7** — `--check-config` gives byte-identical JSON before an adoption and after a detach,
/// **and the same exit with it**.
///
/// The JSON is compared as bytes rather than as prose, and the exit code is compared too: a face
/// that returned the right document under a different status would be a face a script read
/// differently. The pre-attach answer is `7` here and not `0`, because before an adoption the direct
/// road is exactly what is there — that the *failing* answer comes back unchanged is the point.
#[test]
fn a_detach_returns_the_check_to_its_pre_attach_answer() {
    let dir = scratch("p1c_ac7");
    let before_doc = json!({
        "other_top": {"x": 1},
        "mcpServers": {
            "other": {"command": "/bin/other", "args": []},
            "s": {"command": "/usr/bin/orig", "args": ["--flag", "one"], "env": {"Z": "1"}},
        },
    });
    let path = config(&dir, "c.json", &before_doc);

    let before = wrap_mode("check", &path, &[]);
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);
    let attached = wrap_mode("check", &path, &[]);
    assert_eq!(attached.code, 0, "while attached, the direct road is gone");
    let detached = wrap_mode("detach", &path, &[]);
    assert_eq!(detached.code, 0, "{}", why(&detached));
    let after = wrap_mode("check", &path, &[]);

    println!(
        "AC7_EXIT_BEFORE={} AC7_EXIT_AFTER={}",
        before.code, after.code
    );
    assert_eq!(
        before.stdout, after.stdout,
        "the check's answer must come back byte for byte"
    );
    assert_eq!(before.code, after.code, "and under the same exit status");

    // 🔴 The control the reqdef names: an implementation that merely removed the gx entry would
    // leave the document without the command it used to run, and this is what that looks like.
    let restored = read(&path);
    assert_eq!(
        restored["mcpServers"]["s"], before_doc["mcpServers"]["s"],
        "the entry runs what it ran before, rather than having been deleted"
    );
    assert_eq!(
        restored, before_doc,
        "and so does the rest of the document, structurally"
    );
}

// ---------------------------------------------------------------------------------------------
// AC-17 — one entry moves and nothing else does
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-17** (`req/551` §3-2, the heart of the face) — a detach does not touch a member it was
/// not asked about, **including members written after the adoption**.
///
/// This is the probe that separates the design that was chosen from the one that was rejected. A
/// detach that wrote back a preserved copy of the document would pass every other probe in this
/// suite and delete the two members added below, which is a strange thing for the reverse of a
/// "reversible" operation to do to an operator's file.
#[test]
fn a_detach_does_not_touch_what_the_operator_wrote_after_the_adoption() {
    let dir = scratch("p1c_ac17");
    let path = config(&dir, "c.json", &one_server(&["one"]));
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);

    // The operator edits the file while gx is in front of the server: a new server, and a
    // top-level member this module has never heard of.
    let mut edited = read(&path);
    edited["mcpServers"]["added_by_user"] = json!({"command": "/bin/new", "args": ["k"]});
    edited["brand_new_top"] = json!("written after the adoption");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&edited).expect("serialise"),
    )
    .expect("write");

    assert_eq!(wrap_mode("detach", &path, &[]).code, 0);
    let after = read(&path);

    assert_eq!(
        after["mcpServers"]["added_by_user"],
        json!({"command": "/bin/new", "args": ["k"]}),
        "a server added after the adoption survives the detach"
    );
    assert_eq!(
        after["brand_new_top"],
        json!("written after the adoption"),
        "and so does an unknown top-level member"
    );
    assert_eq!(
        after["mcpServers"]["s"],
        json!({"command": "/usr/bin/orig", "args": ["one"]}),
        "while the entry the detach was about is the one that moved"
    );
}

// ---------------------------------------------------------------------------------------------
// AC-15 — nothing to undo is an answer, not a fault
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-15** (`req/551` D-5) — a detach pointed at an entry that never ran gx exits `0`, reports
/// `not-attached`, declares a coverage of zero, and does not write.
#[test]
fn detaching_something_that_was_never_attached_is_not_an_error() {
    let dir = scratch("p1c_ac15");
    let before = one_server(&["one"]);
    let path = config(&dir, "c.json", &before);
    let bytes_before = std::fs::read(&path).expect("read");

    let out = wrap_mode("detach", &path, &[]);
    assert_eq!(out.code, 0, "there was nothing here to undo: {}", why(&out));
    let answer = out.json();

    assert_eq!(answer["outcome"], json!(gx_cli::detach::NOT_ATTACHED));
    assert_eq!(
        answer["now_runs"],
        Value::Null,
        "an entry this run did not change gets no reported command"
    );

    // D-11: the coverage after a detach is zero, and it is zero as four values.
    let coverage = answer["coverage"].as_array().expect("four rows");
    assert_eq!(coverage.len(), 4, "four questions, always");
    for row in coverage {
        assert_eq!(
            row["posture"],
            json!("cannot-measure"),
            "a face with no route measures nothing: {row}"
        );
    }

    assert_eq!(
        std::fs::read(&path).expect("read"),
        bytes_before,
        "and the file is not rewritten on the way past"
    );
}

/// A detach applied twice reports `not-attached` the second time, rather than failing.
#[test]
fn a_second_detach_reports_that_there_is_nothing_left_to_undo() {
    let dir = scratch("p1c_ac15b");
    let path = config(&dir, "c.json", &one_server(&["one"]));
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);

    let first = wrap_mode("detach", &path, &[]);
    assert_eq!(first.json()["outcome"], json!(gx_cli::detach::RESTORED));
    let second = wrap_mode("detach", &path, &[]);
    assert_eq!(second.code, 0, "{}", why(&second));
    assert_eq!(
        second.json()["outcome"],
        json!(gx_cli::detach::NOT_ATTACHED)
    );
}

// ---------------------------------------------------------------------------------------------
// AC-16 — what cannot come back is a value, never a silence
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-16** (`req/551` D-8) — every detach that restores an entry names what it did not restore,
/// on the runs that worked as much as on the ones that did not.
///
/// The empty-arguments case is the third declaration and the sharpest one: `--adopt-config` writes
/// an `args` member onto every entry it touches, so an entry that had no `args` at all comes back
/// with `"args": []`. Whether the operator wrote an empty list or wrote nothing is not recorded
/// anywhere, and the honest thing is to say so rather than to pick one.
#[test]
fn what_did_not_come_back_is_named_every_time() {
    let dir = scratch("p1c_ac16");

    // An entry with arguments: the two standing declarations.
    let with_args = config(&dir, "with.json", &one_server(&["one"]));
    assert_eq!(wrap_mode("adopt", &with_args, &[]).code, 0);
    let named = wrap_mode("detach", &with_args, &[]).json();
    let declarations = named["not_restored"].as_array().expect("a list").clone();
    let text: Vec<&str> = declarations.iter().filter_map(Value::as_str).collect();
    println!("AC16_DECLARATIONS={}", text.len());
    assert!(
        text.iter()
            .any(|d| d.starts_with("no_byte_level_restoration")),
        "the file's own bytes are not what came back: {text:?}"
    );
    assert!(
        text.iter().any(|d| d.starts_with("no_preserved_body")),
        "and nothing here was recovered from a backup: {text:?}"
    );

    // An entry with no `args` member at all: the third.
    let no_args = config(
        &dir,
        "without.json",
        &json!({"mcpServers": {"s": {"command": "/usr/bin/orig"}}}),
    );
    assert_eq!(wrap_mode("adopt", &no_args, &[]).code, 0);
    let out = wrap_mode("detach", &no_args, &[]).json();
    let text: Vec<&str> = out["not_restored"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        text.iter()
            .any(|d| d.starts_with("empty_args_indistinguishable")),
        "an entry that had no `args` says that its empty list is a guess: {text:?}"
    );

    // 🔴 The control (`req/544` AC-4's shape): "said nothing" and "said it cannot" must not be the
    // same document. A declaration list with the member omitted is a different document from one
    // that carries it, and this is the comparison that shows it.
    let mut silent = out.clone();
    silent
        .as_object_mut()
        .expect("an object")
        .remove("not_restored");
    assert_ne!(
        serde_json::to_vec(&silent).expect("bytes"),
        serde_json::to_vec(&out).expect("bytes"),
        "an answer that omitted the declarations would be a different document"
    );
}

// ---------------------------------------------------------------------------------------------
// AC-18 — the words, and the one that is not among them
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-18** (`req/551` D-9) — the whole vocabulary of a detach, counted, with no word for a
/// removal in it.
///
/// `crate::attach` gives the placement three words and refuses to invent a fourth for "was there
/// and is gone". This face is held to the same rule from the other direction: if a detach had a word
/// for deleting records, it would be a detach that could delete them.
#[test]
fn the_detach_vocabulary_is_three_words_and_none_of_them_deletes() {
    let words = gx_cli::detach::DETACH_WORDS;
    println!("DETACH_WORDS={}", words.len());
    assert_eq!(words.len(), 3, "three words: {words:?}");
    assert_eq!(
        words,
        ["restored", "left-in-place", "not-attached"],
        "and these three"
    );

    // The control: no word in the set means a removal, in this vocabulary or in the placement's.
    let forbidden = ["removed", "deleted", "cleared", "purged", "erased", "wiped"];
    for word in words {
        assert!(
            !forbidden.contains(&word),
            "`{word}` would be a word for something no road in this binary builds"
        );
    }
    let together: Vec<&str> = words
        .iter()
        .copied()
        .chain(["created", "already-present", "not-placed"])
        .collect();
    println!("ATTACH_AND_DETACH_WORDS={}", together.len());
    assert_eq!(
        together.len(),
        6,
        "the two faces together: three placement words and three route words"
    );
}

// ---------------------------------------------------------------------------------------------
// AC-20 — the attach face's third sentence, and the two that stay
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-20** (`req/551` D-11) — a detach exists, so the sentence saying one does not must change,
/// and **only** that sentence.
///
/// `req/551` wrote this as "replace the third item, keep the first two verbatim". P-1b had already
/// moved what was then the second item out into its own member, so the array holds two sentences by
/// the time this hand reaches it: the one about routes, which is untouched, and the one about
/// leaving, which is this face's to answer. The retired sentence is kept in the answer rather than
/// deleted, which is the shape P-1b set when it retired the other one.
#[test]
fn only_the_sentence_about_leaving_changed() {
    let dir = scratch("p1c_ac20");
    let out = run(gx().arg("attach").arg("--project").arg(&dir));
    assert_eq!(out.code, 0, "{}", why(&out));
    let answer = out.json();

    let items = answer["not_carried_by_this_face"]
        .as_array()
        .expect("the array");
    println!("NOT_CARRIED_ITEMS={}", items.len());
    assert_eq!(items.len(), 2, "two sentences: {items:?}");

    // The first is pinned verbatim: this face does not get to reword a claim it did not make.
    assert_eq!(
        items[0].as_str().expect("text"),
        "which effects reach the membrane: this operation points no route at gx. `gx wrap \
         --adopt-config` is the road that does, and it is a separate invocation",
        "the sentence about routes is not this hand's to touch"
    );

    // The second now names the inverse, and still says `.gx/` survives it.
    let leaving = items[1].as_str().expect("text");
    assert!(
        leaving.starts_with("how to leave: `gx wrap --detach-config"),
        "the second sentence names the inverse that now exists: {leaving}"
    );
    assert!(
        !leaving.contains("no inverse yet"),
        "and no longer says there is none: {leaving}"
    );
    assert!(
        leaving.contains("`.gx/` is not removed by any verb of this binary"),
        "while the half that is still true is still there: {leaving}"
    );

    // P-1b's retired sentence is untouched, and this hand's is beside it.
    assert_eq!(
        answer["now_answered_by_coverage"].as_str().expect("text"),
        "what this project can and cannot observe about a change: nothing here states it, so \
         nothing here should be read as stating it",
        "P-1b's retired sentence is not this hand's to touch either"
    );
    assert_eq!(
        answer["now_answered_by_detach"].as_str().expect("text"),
        "how to leave: this operation has no inverse yet. `.gx/` is not removed by any verb of \
         this binary, so what it holds survives whatever happens next",
        "and what this hand retired is readable in the answer rather than only in a diff"
    );
}

// ---------------------------------------------------------------------------------------------
// G-1 and G-2 — the two falsifiers that fired, pinned
// ---------------------------------------------------------------------------------------------

/// 🔴 **`req/551` G-1** — a wrap flag whose value is the string `--`.
///
/// `--actor-model=--` is accepted by the parser, and it puts a `--` into the adopted `args` **before**
/// the separator. Measured on this binary: `--check-config` reports `"command": "--"` for such an
/// entry, which is a display fault today and would have been a data fault the moment a reverse
/// operation wrote that guess back. The split reads the flags positionally instead, so the value is
/// consumed without being interpreted and the round trip is exact.
#[test]
fn a_flag_whose_value_is_the_separator_still_round_trips() {
    let dir = scratch("p1c_g1");
    let before = one_server(&["one"]);
    let path = config(&dir, "c.json", &before);

    let adopted = wrap_mode("adopt", &path, &["--actor-model=--"]);
    assert_eq!(adopted.code, 0, "{}", why(&adopted));
    let wrapped = read(&path);
    let args = wrapped["mcpServers"]["s"]["args"]
        .as_array()
        .expect("args")
        .clone();
    println!("G1_WRAPPED_ARGS={args:?}");
    assert_eq!(
        args.iter().filter(|a| a.as_str() == Some("--")).count(),
        2,
        "the flag's value and the separator are both `--`: {args:?}"
    );

    assert_eq!(wrap_mode("detach", &path, &[]).code, 0);
    assert_eq!(
        read(&path),
        before,
        "and the entry comes back exactly, rather than coming back as `--`"
    );
}

/// A wrapped entry whose *original* arguments contain a `--` of their own.
#[test]
fn original_arguments_containing_a_separator_round_trip() {
    let dir = scratch("p1c_g1a");
    let before = one_server(&["one", "--", "two"]);
    let path = config(&dir, "c.json", &before);
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);
    assert_eq!(wrap_mode("detach", &path, &[]).code, 0);
    assert_eq!(read(&path), before, "the arguments come back as they were");
}

/// 🔴 **`req/551` G-2** — key order survives an adoption, measured rather than derived.
///
/// The reqdef derived the opposite from a grep over this workspace's manifests, and the derivation
/// was wrong: `preserve_order` is on, enabled through a dependency. That makes the property real but
/// **inherited** — this workspace does not declare it and does not control it. So it is measured
/// here on every run: if the dependency that enables it ever stops doing so, this goes red, rather
/// than the `no_byte_level_restoration` declaration quietly becoming an understatement.
#[test]
fn the_document_keeps_its_key_order_through_a_round_trip() {
    let dir = scratch("p1c_g2");
    // Deliberately not in alphabetical order, at three depths.
    let raw = br#"{
  "zebra_top": {"keep": "me"},
  "mcpServers": {
    "zulu": {"command": "/bin/zulu", "args": ["z"]},
    "s": {"command": "/usr/bin/orig", "args": ["one"], "env": {"Z": "1", "A": "2"}}
  },
  "apple_top": [1, 2, 3]
}
"#;
    let path = dir.join("c.json");
    std::fs::write(&path, raw).expect("write");

    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);
    let after = read(&path);
    let top: Vec<&String> = after.as_object().expect("object").keys().collect();
    println!("G2_TOP_ORDER={top:?}");
    assert_eq!(
        top,
        ["zebra_top", "mcpServers", "apple_top"],
        "an adoption keeps the operator's key order; if this ever goes red, the declaration in \
         `gx_mcp_wire::config::NO_BYTE_LEVEL_RESTORATION` understates what is lost and must be \
         widened to say that key order is scrambled too"
    );
    let env: Vec<&String> = after["mcpServers"]["s"]["env"]
        .as_object()
        .expect("object")
        .keys()
        .collect();
    assert_eq!(env, ["Z", "A"], "at depth as well");
}

// ---------------------------------------------------------------------------------------------
// (D) — the refusal, and what it protects
// ---------------------------------------------------------------------------------------------

/// 🔴 **`req/551` §3-2 (D)** — an entry that runs `wrap` in a shape this binary did not write is
/// refused, not repaired, and the document is not touched on the way out.
///
/// This is the whole of the hand-edit gate that survives the ruling that P-1c keeps no preserved
/// copy (`req/38` §320 ruling 3, request 2): with no saved body to compare against, the evidence
/// that an entry was edited by hand is that it does not parse as something `--adopt-config` writes.
/// Guessing here would write the guess into the operator's file, so it refuses and says what it
/// could not read.
///
/// 🔴 The exit is **1**. A refusal is not a `Verdict::Deny`, and `2` is spoken for.
#[test]
fn an_entry_in_a_shape_adopt_never_wrote_is_refused_rather_than_guessed_at() {
    let dir = scratch("p1c_handedit");
    let before = json!({"mcpServers": {"s": {
        "command": "gx",
        "args": ["wrap", "--who-knows", "x", "--", "/usr/bin/orig"],
    }}});
    let path = config(&dir, "c.json", &before);
    let bytes_before = std::fs::read(&path).expect("read");

    let out = wrap_mode("detach", &path, &[]);
    println!("HANDEDIT_EXIT={}", out.code);
    assert_eq!(
        out.code,
        gx_cli::exit::ERROR as i32,
        "refused as invalid input, and not as {} (`DENIED`): {}",
        gx_cli::exit::DENIED,
        why(&out)
    );
    assert!(
        out.stderr.contains("refused rather than guessed at"),
        "the refusal says why it will not guess: {}",
        out.stderr
    );
    assert_eq!(
        std::fs::read(&path).expect("read"),
        bytes_before,
        "and a refusal writes nothing at all"
    );
}

/// The two errors that are about the document rather than about the entry.
#[test]
fn a_detach_that_cannot_find_what_it_was_pointed_at_says_which() {
    let dir = scratch("p1c_missing");

    let no_servers = config(&dir, "a.json", &json!({"something_else": {}}));
    let out = wrap_mode("detach", &no_servers, &[]);
    assert_eq!(out.code, gx_cli::exit::ERROR as i32);
    assert!(out.stderr.contains("names no MCP server"), "{}", out.stderr);

    let no_entry = config(&dir, "b.json", &json!({"mcpServers": {"other": {}}}));
    let out = wrap_mode("detach", &no_entry, &[]);
    assert_eq!(out.code, gx_cli::exit::ERROR as i32);
    assert!(
        out.stderr.contains("is not in this document"),
        "{}",
        out.stderr
    );
}

/// `--detach-config` without `--server-name` refuses, the way the other two modes do.
#[test]
fn a_detach_needs_to_be_told_which_entry_it_is_about() {
    let dir = scratch("p1c_noname");
    let path = config(&dir, "c.json", &one_server(&["one"]));
    let out = run(gx().arg("wrap").arg("--detach-config").arg(&path));
    assert_eq!(out.code, gx_cli::exit::ERROR as i32, "{}", why(&out));
    assert!(
        out.stderr.contains("--detach-config needs --server-name"),
        "{}",
        out.stderr
    );
}

// ---------------------------------------------------------------------------------------------
// AC-8 — the receipts outlive the arrangement that produced them
// ---------------------------------------------------------------------------------------------

/// The frozen attach-face specimen P-1b minted on 2026-08-22 (`req/548` §0). Read, never re-minted.
fn frozen(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("attach_face_frozen")
        .join("issued_2026_08_22")
        .join(name)
}

/// `gx receipt verify <file> --offline …`, in `p1b_attach_face_frozen.rs`'s hermetic posture.
fn verify_offline(receipt: &Path, cwd: &Path, home: &Path) -> support::Run {
    let mut cmd = gx();
    cmd.env_clear();
    cmd.env("HOME", home);
    cmd.current_dir(cwd);
    run(cmd
        .arg("receipt")
        .arg("verify")
        .arg(receipt)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(frozen("checkpoint.json"))
        .arg("--checkpoint-key")
        .arg(frozen("key.pub.json"))
        .arg("--key")
        .arg(frozen("key.pub.json")))
}

/// 🔴 **AC-8** (`req/551` D-4) — a receipt issued while gx was in front of the server still verifies
/// offline after the detach, and verifies to the same bytes.
///
/// # Why this is a probe about `.gx/` and not about the configuration
///
/// `gx receipt verify` never opens a project — the verifier is deliberately project-free, so that it
/// works in the one environment that has no project at all. So a detach cannot break a verification
/// by changing a configuration: **the only way it could break one is by deleting something**, and
/// the thing it would have to delete is the checkpoint that keeps `--offline` from falling back to
/// `unanchored`. That makes AC-8 a consequence of D-3 rather than an independent claim, and this
/// probe measures the consequence.
///
/// 🔴 What this does **not** close: the specimens were minted by this binary, so a change that moved
/// the encoder and the decoder together is invisible here, exactly as `req/544` §5-1 said. What is
/// closed is drift across a detach, and that is the whole of the claim.
#[test]
fn receipts_issued_while_attached_still_verify_after_a_detach() {
    let cwd = scratch("p1c_ac8_cwd");
    let home = scratch("p1c_ac8_home");
    let specimens = ["commit_receipt.json", "verdict_receipt.json"];

    let before: Vec<Vec<u8>> = specimens
        .iter()
        .map(|name| std::fs::read(frozen(name)).expect("the frozen specimen is there"))
        .collect();

    let mut answers_before = Vec::new();
    for name in specimens {
        let out = verify_offline(&frozen(name), &cwd, &home);
        assert_eq!(out.code, 0, "before the detach: {}", why(&out));
        answers_before.push(out.stdout);
    }

    let dir = scratch("p1c_ac8_config");
    let path = config(&dir, "c.json", &one_server(&["one"]));
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);
    let detached = wrap_mode("detach", &path, &[]);
    assert_eq!(detached.code, 0, "{}", why(&detached));
    assert_eq!(
        detached.json()["records"],
        json!(gx_cli::detach::LEFT_IN_PLACE),
        "the detach says it left the records alone"
    );

    for (i, name) in specimens.iter().enumerate() {
        assert_eq!(
            std::fs::read(frozen(name)).expect("read"),
            before[i],
            "🔴 the detach must not have touched the specimen {name} at all"
        );
        let out = verify_offline(&frozen(name), &cwd, &home);
        println!("AC8_OFFLINE_AFTER_DETACH file={name} exit={}", out.code);
        assert_eq!(
            out.code,
            0,
            "🔴 a receipt issued while gx was in front of the server stopped verifying after the \
             detach: {}",
            why(&out)
        );
        assert_eq!(
            out.stdout, answers_before[i],
            "and it verifies to the same answer, not merely to some passing one"
        );
        assert_eq!(out.json()["valid"], json!(true));
    }
}

/// 🔴 **AC-8's negative control** — the verification these probes run is one that a missing
/// checkpoint actually breaks.
///
/// Without this, "still verifies after a detach" would be compatible with a verification that never
/// consulted the checkpoint at all. `--offline` on its own reports `unanchored` and is not a pass,
/// which is what makes deleting `.gx/checkpoints/` a way to break AC-8 and therefore what makes D-4
/// a consequence of D-3.
#[test]
fn an_offline_verification_without_the_checkpoint_is_not_a_pass() {
    let cwd = scratch("p1c_ac8ctl_cwd");
    let home = scratch("p1c_ac8ctl_home");
    let mut cmd = gx();
    cmd.env_clear();
    cmd.env("HOME", &home);
    cmd.current_dir(&cwd);
    let out = run(cmd
        .arg("receipt")
        .arg("verify")
        .arg(frozen("commit_receipt.json"))
        .arg("--offline")
        .arg("--key")
        .arg(frozen("key.pub.json")));
    println!("AC8_CONTROL_NO_CHECKPOINT exit={}", out.code);
    assert_ne!(
        out.code,
        0,
        "🔴 a verification with no checkpoint passed, so AC-8 is not measuring what it claims: {}",
        why(&out)
    );
}

// ---------------------------------------------------------------------------------------------
// AC-19 — a detach reaches nothing and changes nothing outside the file it was given
// ---------------------------------------------------------------------------------------------

/// 🔴 **AC-19** (`req/551` D-10) — a detach opens no socket.
///
/// Measured the way `req/538` §1 measured it: the namespace's emptiness is established **first**,
/// with a control that proves the sandbox really has no network, and only then is the verb run
/// inside it. A probe that just ran the verb under `unshare` and saw it pass would be a probe that
/// passed on a machine where `unshare` silently did nothing.
#[test]
fn a_detach_makes_no_network_connection() {
    let probe = std::process::Command::new("unshare")
        .args(["-rn", "true"])
        .status();
    if !matches!(probe, Ok(status) if status.success()) {
        println!("AC19_NETWORK=skipped (no usable `unshare -rn` on this host)");
        return;
    }

    // The control first: the namespace has no network, so a thing that needs one fails in it.
    let control = std::process::Command::new("unshare")
        .args(["-rn", "getent", "ahosts", "example.com"])
        .output()
        .expect("unshare runs");
    println!(
        "AC19_CONTROL_RESOLVES_IN_NAMESPACE={}",
        control.status.success()
    );
    assert!(
        !control.status.success(),
        "🔴 a name resolved inside the namespace, so the namespace is not empty and this probe \
         would pass for the wrong reason"
    );

    let dir = scratch("p1c_ac19");
    let before = one_server(&["one"]);
    let path = config(&dir, "c.json", &before);
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);

    let out = std::process::Command::new("unshare")
        .args(["-rn"])
        .arg(env!("CARGO_BIN_EXE_gx"))
        .arg("wrap")
        .arg("--detach-config")
        .arg(&path)
        .arg("--server-name")
        .arg("s")
        .output()
        .expect("gx runs under unshare");
    println!("AC19_DETACH_IN_NAMESPACE exit={:?}", out.status.code());
    assert!(
        out.status.success(),
        "🔴 a detach needed something outside its own filesystem: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        read(&path),
        before,
        "and it did the whole job with no network at all"
    );
}

/// Every file under a directory, with its bytes hashed, as a sorted list.
///
/// A content hash rather than anything that reads a git index: `req/538` §2-2 measured that
/// `git ls-files -s` answers about a recorded state instead of about the files on disk.
fn census(root: &Path) -> Vec<(String, u64)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, u64)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if let Ok(bytes) = std::fs::read(&path) {
                let mut hash = 1_469_598_103_934_665_603u64;
                for byte in &bytes {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(1_099_511_628_211);
                }
                let name = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                out.push((name, hash));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// 🔴 **AC-19, the other half** — a detach writes to the file it was handed and to nothing else.
#[test]
fn a_detach_writes_only_the_file_it_was_given() {
    let dir = scratch("p1c_ac19b");
    // A project's `.gx/`, so that the thing D-3 protects is actually present to be protected.
    let project = dir.join("tree");
    std::fs::create_dir_all(&project).expect("mkdir");
    let attached = run(gx().arg("attach").arg("--project").arg(&project));
    assert_eq!(attached.code, 0, "{}", why(&attached));

    let path = config(&dir, "c.json", &one_server(&["one"]));
    assert_eq!(wrap_mode("adopt", &path, &[]).code, 0);
    let before = census(&project);
    assert!(
        !before.is_empty(),
        "the project has files in it, or this measures nothing"
    );

    assert_eq!(wrap_mode("detach", &path, &[]).code, 0);
    let after = census(&project);

    println!(
        "AC19_TREE_FILES_BEFORE={} AFTER={}",
        before.len(),
        after.len()
    );
    assert_eq!(
        before, after,
        "🔴 a detach changed something under `.gx/`. D-3 says it removes nothing there, and this \
         is the measurement that says whether it kept to that"
    );

    // The control: the census notices a one-byte change, so its silence above means something.
    let victim = project.join(".gx").join("control_probe");
    std::fs::write(&victim, b"x").expect("write");
    assert_ne!(
        census(&project),
        after,
        "the census must be able to see a change, or its equality above is not evidence"
    );
    std::fs::remove_file(&victim).expect("remove");
}
