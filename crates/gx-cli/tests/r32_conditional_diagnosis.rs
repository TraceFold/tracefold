// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R32 / `req/392` M-02 — end to end: the refusal an operator reads names the condition they have.
//!
//! # The four beds are the audit's four beds
//!
//! `crates/gx-cli/tests/a31_false_diagnosis.rs` built one project per condition it could reach,
//! damaged it, and measured each factual claim in the paragraph rather than reading it. Three of
//! the four printed the paragraph and **two of those three** were told a cause nobody had
//! measured. This suite rebuilds the same four beds and asserts the sentence, because the sentence
//! is what the operator acts on — and because `req/38` §231's ④ (a repair whose own correction
//! text repeats the defect) is what put the grade on this.
//!
//! # Read this beside `r32_note_is_conditional.rs`
//!
//! That suite drives the two functions directly and is the text-level gate. This one drives the
//! shipped binary, so it also covers the third site of the family (`gx repair`'s `remedy`, which
//! had no arm at all for two of these conditions) and the join that `req/392` §3-3 measured.
//!
//! Self-directed test of this repository's own CLI. Every project built here lives inside this
//! worktree's `CARGO_TARGET_TMPDIR`; no network is used.

mod support;

use std::path::Path;

use support::{pipeline, run, Pipeline};

/// The clauses this suite branches on, spelled once.
const MOVED: &str = "The journal is the file that moved";
const CHAIN_REFUSING: &str = "per-record chain refusing to verify";
const TORN_FILE: &str = "<journal>.torn.<n>-<m>";
const FILES_AGREE: &str = "Comparing the two files will find nothing";
/// Both faces spell the fact; the verb's remedy says "carry" and the note says "carries",
/// because one is about the eight bytes and the other about the file. The common substring is the
/// claim itself.
const NO_MARKER: &str = "no framing marker";
const NEWER_GX: &str = "never heard of";

/// One bed's damage, so the table below reads as a table.
type Damage = Box<dyn Fn(&Path)>;

fn journal_of(project: &Path) -> std::path::PathBuf {
    project.join(".gx").join("ledger").join("journal")
}

/// What `gx submit` printed and what `gx repair --json` put in `remedy`, over one damaged project.
fn diagnose(label: &str, p: &Pipeline, damage: impl FnOnce(&Path)) -> (String, String) {
    let journal = journal_of(&p.project);
    damage(&journal);
    let submitted = p.submit("a second goal");
    let reported = run(p.gx().args(["repair", "--json"]));
    let remedy = serde_json::from_str::<serde_json::Value>(&reported.stdout)
        .ok()
        .and_then(|v| v.get("remedy").cloned())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    println!(
        "R32_BED label={label} submit_exit={} repair_exit={} moved={} chain_refusing={} \
         torn_file={} files_agree={}",
        submitted.code,
        reported.code,
        submitted.stderr.contains(MOVED),
        submitted.stderr.contains(CHAIN_REFUSING),
        submitted.stderr.contains(TORN_FILE),
        submitted.stderr.contains(FILES_AGREE),
    );
    println!(
        "R32_BED_STDERR label={label} stderr={:?}",
        submitted.stderr.trim()
    );
    println!("R32_BED_REMEDY label={label} remedy={remedy:?}");
    (submitted.stderr, remedy)
}

/// 🔴 The byte the two `rewritten_record` beds flip: the **middle of the first record's payload**,
/// read out of that record's own length header rather than guessed at from the file's length.
///
/// `bytes.len() / 2` named it until this repair, and that is a coordinate in a file whose length
/// moves. The framing is `GXJRNL0n` and then `[u32 length][payload][32-byte link]` repeated
/// (`gx_engine::replay`), and the records carry this fixture's **absolute paths**, so a checkout at
/// a different path writes a journal a few bytes longer or shorter and the halfway mark slides
/// across the frames. Where it lands decides which condition the bed is in — so the bed was a
/// property of the directory the suite was run from.
///
/// Measured, over every structural spot of a ten-record journal this fixture writes: 31 of 100
/// flips answer with `Comparing the two files will find nothing` and **neither** of the two clauses
/// this bed asserts on, and all 31 are in the **top three bytes of a `u32` length header**. A flip
/// there does not rewrite a record, it unframes one: the length becomes absurd, the walk stops at
/// that header, and the file reads as a project *behind* its head rather than one whose bytes
/// moved. The remaining 69 — every payload byte and every link byte — all answer alike.
///
/// The control bed wants the other condition: a **whole record that is not the record that belongs
/// there**, which is a payload byte and nothing else. So the offset is derived rather than
/// measured off the end: past the marker, past the four length bytes, half way into the payload
/// those four bytes measure. The first record's frame begins at a fixed offset by construction, so
/// neither the offset nor its distance from the two nearest boundaries (~44 bytes each way here)
/// moves with the path.
fn a_payload_byte_of_the_first_record(bytes: &[u8]) -> usize {
    // `gx_engine::replay` sniffs the marker at offset zero and both framings it writes are eight
    // bytes wide; `LENGTH_BYTES` is that crate's own private constant, restated here because a
    // test cannot reach it.
    let marker = gx_engine::JOURNAL_MAGIC.len();
    const LENGTH_BYTES: usize = 4;
    let mut header = [0u8; LENGTH_BYTES];
    header.copy_from_slice(&bytes[marker..marker + LENGTH_BYTES]);
    let payload = u32::from_be_bytes(header) as usize;
    assert!(
        payload >= 2 * LENGTH_BYTES,
        "🔴 the first record's payload is {payload} byte(s), so no flip inside it clears the \
         frame's own boundaries and this bed is no longer the condition it names: re-derive the \
         offset against whatever framing `gx_engine::replay` now writes"
    );
    marker + LENGTH_BYTES + payload / 2
}

/// Bed 1 — the **control**. A payload byte is flipped, so the chain really is refusing to verify
/// and every clause in the old paragraph was about this bed. It has to be unchanged.
#[test]
fn r32_the_control_bed_still_gets_the_paragraph_that_was_written_for_it() {
    let p = pipeline("r32_control", "hello\n");
    p.commit_one("first goal");
    let (stderr, remedy) = diagnose("rewritten_record", &p, |journal| {
        let mut bytes = std::fs::read(journal).expect("read");
        let at = a_payload_byte_of_the_first_record(&bytes);
        bytes[at] ^= 0xff;
        std::fs::write(journal, &bytes).expect("write the flipped byte");
    });
    assert!(
        stderr.contains(MOVED),
        "🔴 the sentence `serve_runtime_r4` and `serve_runtime_r5` assert on is unmoved: {stderr}"
    );
    assert!(
        stderr.contains(CHAIN_REFUSING),
        "and on this bed the clause is true — the audit measured `journal_chain_break_at: 834` \
         beside it: {stderr}"
    );
    assert!(
        !stderr.contains(TORN_FILE),
        "🔴 `req/392` §3-2-2: two shipped surfaces gave opposite orders about this file. `gx \
         repair --yes`'s own remedy says it does not cut here, and the audit measured `.torn.` \
         files `before=[] after=[]` on 4/4 beds: {stderr}"
    );
    assert!(
        remedy.contains("does not verify from byte"),
        "and `gx repair` still names the byte: {remedy}"
    );
}

/// Bed 2 — the eight marker bytes are removed. `downgraded()` is true, `chain_intact()` is true,
/// and the file has no chain at all: there is nothing for a link to refuse.
#[test]
fn r32_a_stripped_marker_is_not_told_its_chain_stopped_verifying() {
    let p = pipeline("r32_stripped", "hello\n");
    p.commit_one("first goal");
    let (stderr, remedy) = diagnose("stripped_marker", &p, |journal| {
        let bytes = std::fs::read(journal).expect("read");
        std::fs::write(journal, &bytes[8..]).expect("write without the marker");
    });
    assert!(
        !stderr.contains(CHAIN_REFUSING),
        "🔴 `req/392` M-02: a `legacy` file carries no links, so \"the per-record chain refusing to \
         verify\" has no subject. The audit measured `journal_chain_break_at: null` beside this \
         sentence: {stderr}"
    );
    assert!(
        !stderr.contains(TORN_FILE),
        "and nothing is torn here either: {stderr}"
    );
    assert!(
        stderr.contains(NO_MARKER),
        "🔴 what is true of this bed is what it is told: the declaration says chained and the \
         first eight bytes carry no marker: {stderr}"
    );
    assert!(
        remedy.contains(NO_MARKER),
        "🔴 `req/392` §3-5 site 3: `gx repair`'s remedy had **no arm** for this condition and fell \
         into the one about a file that had grown shorter or been rewritten: {remedy}"
    );
}

/// Bed 3 — a marker from a build that does not exist. This build's own source says of the
/// condition: *nothing is wrong with the file, and this binary cannot verify it.*
#[test]
fn r32_a_marker_from_the_future_is_not_told_its_journal_was_rewritten() {
    let p = pipeline("r32_future", "hello\n");
    p.commit_one("first goal");
    let (stderr, remedy) = diagnose("marker_from_the_future", &p, |journal| {
        let mut bytes = std::fs::read(journal).expect("read");
        bytes[..8].copy_from_slice(b"GXJRNL99");
        std::fs::write(journal, &bytes).expect("write the unknown marker");
    });
    assert!(
        !stderr.contains(MOVED),
        "🔴 `req/392` M-02: `pipeline.rs` folds `from_a_newer_gx` into `journal_intact` on a line \
         whose own comment says \"nothing is wrong with the file, and this binary cannot verify \
         it\", and four lines later the paragraph said the journal had moved: {stderr}"
    );
    assert!(
        !stderr.contains(CHAIN_REFUSING),
        "and this build did not walk a chain it cannot frame: {stderr}"
    );
    assert!(
        !stderr.contains(TORN_FILE),
        "and it removed nothing, so there is nothing to look for: {stderr}"
    );
    assert!(
        stderr.contains(NEWER_GX),
        "🔴 what is true of this bed is what it is told: {stderr}"
    );
    assert!(
        remedy.contains(NEWER_GX),
        "🔴 `req/392` §3-5 site 3: the second arm `gx repair`'s remedy did not have: {remedy}"
    );
}

/// Bed 4 — the file is cut at a frame boundary, leaving the marker. The journal has **not**
/// departed here: the two files agree with each other and disagree with the signed head, which is
/// exactly the condition `rolled_back_note` was written for by `req/229` H-01.
#[test]
fn r32_a_project_behind_its_head_gets_the_clause_written_for_it_and_no_other() {
    let p = pipeline("r32_behind", "hello\n");
    p.commit_one("first goal");
    let (stderr, _remedy) = diagnose("cut_at_a_frame_boundary", &p, |journal| {
        let bytes = std::fs::read(journal).expect("read");
        std::fs::write(journal, &bytes[..8]).expect("write the marker alone");
    });
    assert!(
        stderr.contains(FILES_AGREE),
        "🔴 `req/229` H-01's sentence, unmoved by this lane: {stderr}"
    );
    assert!(
        !stderr.contains(MOVED),
        "and it is not printed beside a sentence saying one of the two files moved: {stderr}"
    );
}

/// 🔴 `req/392` §3-3, as one assertion over all four beds: the two clauses are never both there.
///
/// The audit measured `rolled_back_clause=true` on `4/4` and `note_printed=true` on `3/4`, in the
/// same `detail` string, because six sites spelled `format!("{}{}", journal_note(..),
/// rolled_back_note(..))` and the second one's own doc says it exists because in its condition
/// every word the first prints is false.
#[test]
fn r32_no_refusal_says_a_file_moved_and_that_comparing_the_files_finds_nothing() {
    let beds: Vec<(&str, Damage)> = vec![
        (
            "rewritten_record",
            Box::new(|journal: &Path| {
                let mut bytes = std::fs::read(journal).expect("read");
                let at = a_payload_byte_of_the_first_record(&bytes);
                bytes[at] ^= 0xff;
                std::fs::write(journal, &bytes).expect("write");
            }),
        ),
        (
            "stripped_marker",
            Box::new(|journal: &Path| {
                let bytes = std::fs::read(journal).expect("read");
                std::fs::write(journal, &bytes[8..]).expect("write");
            }),
        ),
        (
            "marker_from_the_future",
            Box::new(|journal: &Path| {
                let mut bytes = std::fs::read(journal).expect("read");
                bytes[..8].copy_from_slice(b"GXJRNL99");
                std::fs::write(journal, &bytes).expect("write");
            }),
        ),
        (
            "cut_at_a_frame_boundary",
            Box::new(|journal: &Path| {
                let bytes = std::fs::read(journal).expect("read");
                std::fs::write(journal, &bytes[..8]).expect("write");
            }),
        ),
    ];
    for (label, damage) in beds {
        let p = pipeline(&format!("r32_join_{label}"), "hello\n");
        p.commit_one("first goal");
        let journal = journal_of(&p.project);
        damage(&journal);
        let submitted = p.submit("a second goal");
        let says_moved = submitted.stderr.contains(MOVED)
            || submitted.stderr.contains("The journal is not the journal");
        let says_agree = submitted.stderr.contains(FILES_AGREE);
        println!(
            "R32_JOIN label={label} says_a_file_moved={says_moved} says_files_agree={says_agree}"
        );
        assert!(
            !(says_moved && says_agree),
            "🔴 `req/392` §3-3: one `detail` string told an operator to go and compare two files \
             and, in the next sentence, that comparing them would find nothing. Measured on 4/4 \
             beds. Bed {label}: {}",
            submitted.stderr
        );
    }
}
