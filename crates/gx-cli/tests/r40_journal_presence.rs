// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R40 / `req/553` M-01 (`req/38` §322-2 (11-2), §328) — the escape hatch's reach is an
//! existence and not a capability.**
//!
//! # What audit 39 measured
//!
//! R39 narrowed the read road's discriminator to `ledger_agrees` alone and left one escape: a
//! project this binary cannot open an engine over is answered from the ledger as before. The reason
//! it wrote beside the escape was *"there is no second answer for this one to disagree with"*, and
//! the cost it wrote beside **that** was that the arm "rests on being able to tell 'there is no
//! second file' from 'there is a second file this build cannot read'".
//!
//! Audit 39 built the second case three ways — `chmod 0000` on the journal, the journal replaced by
//! a directory of the same name, and the same again on a project with no recorded head — and
//! measured `gx log proof`, `gx log consistency` and `gx log checkpoint` answering **exit 0** on all
//! three, `checkpoint` with a **signature**, on a project the same binary had refused
//! `LEDGER_DISAGREES` one second earlier and refuses again the moment the file is readable. The
//! stated reason was false where it mattered: the second file is present, it holds the
//! disagreement, and the product says so out of its own mouth on the write road.
//!
//! # What this suite pins
//!
//! The escape now asks `layout::presence_of`, and only `Presence::Absent` passes. Every arm below
//! carries its negative control on the same project, because "refuse everything and call it safe"
//! answers all three of audit 39's forms too:
//!
//! * `c1`..`c3` — the three forms refuse, and no signature comes out of any of them.
//! * `c4` — 🔴 **the third-party verifier is untouched.** A ledger file with no journal still
//!   answers, still exits 0, and still produces a proof. This is the caller `req/540` R-1b left the
//!   escape open for, and an R40 that closed it would have broken a buyer's road to fix an
//!   attacker's.
//! * `c5` — 🔴 **the argument questions keep their exit.** `--leaf 99` is a fact about the ledger
//!   file's own size, and R39's `r39_the_argument_questions_keep_their_exit_only_while_the_project
//!   _opens` made the but-for clause explicit: exit 6 holds **while `Layout::open` succeeds**. Both
//!   halves are driven here — 6 where the project opens, and not 6 where R40 now refuses it.
//! * `c6` — 🔴 **the write road and the read road answer with one word.** Measured in the same run
//!   on the same project, because `req/38` §156 ruling 2(a) is about one condition having one word
//!   and audit 39's own write-road numbers turned out to be a `cp -a` artefact (`req/558` §2) —
//!   which is why this suite never copies a bed and never measures a copy.
//! * `c7` — 🔴 **"could not look" is not "not there".** `.gx/ledger/` unreadable used to make
//!   `gx repair --json` print `journal_absent: true` beside a journal holding 1,798 bytes, because
//!   `Path::exists()` folds every `Err` to `false`. Only `NotFound` answers `true` now.
//! * `c8` — the word each form wears, pinned by name, so that a later lane moving one of them has
//!   to say so here.
//!
//! # 🔴 What this suite does **not** claim
//!
//! That `INTERNAL` is the right word for a journal that is a regular file this process cannot open.
//! It is not — the operating system named the path and the reason, and 44 §2.3 keeps `INTERNAL` for
//! what cannot be classified. `req/38` §328 ruling 2 ③ chose a generic over a falsehood (both
//! `JOURNAL_ABSENT`'s "is not there" and `LAYOUT_BLOCKED`'s "is not what the declaration says" are
//! false of that file) and ④ filed the thirteenth word as a DR against spec 44 §2.3 rather than
//! minting one here. `docs/LIMITS.md` carries it as a limit. `c8` pins the current answer so the DR
//! landing is a visible change rather than a quiet one.

use std::path::{Path, PathBuf};

#[path = "support/mod.rs"]
mod support;

use support::{run, Pipeline};

/// The engine journal's frame boundaries — copied verbatim from `r38_ledger_face_width.rs` rather
/// than reconstructed, for the reason `req/496` §7-1 records: a cut computed from memory is a cut
/// this suite invented. A test binary is its own crate, so the copy is the convention here.
fn frames(bytes: &[u8]) -> Vec<(usize, usize)> {
    const CEILING: u32 = 1 << 20;
    let chained = bytes.len() >= 8 && {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[..4]);
        u32::from_be_bytes(header) > CEILING
    };
    let link = usize::from(chained) * 32;
    let mut at = usize::from(chained) * 8;
    let mut out = Vec::new();
    while at + 4 <= bytes.len() {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > CEILING as usize || at + 4 + length + link > bytes.len() {
            break;
        }
        out.push((at, 4 + length + link));
        at += 4 + length + link;
    }
    out
}

fn journal_of(p: &Pipeline) -> PathBuf {
    p.project.join(".gx").join("ledger").join("journal")
}

/// Drop the last frame off the journal, so the two files describe different trees.
///
/// Returns the byte counts either side, because a cut that removed nothing is a bed failure and not
/// a finding.
fn cut_last_frame(p: &Pipeline) -> (u64, u64) {
    let journal = journal_of(p);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let all = frames(&bytes);
    let idx = all.len().checked_sub(1).expect("the journal holds a frame");
    let at = if idx == 0 { all[0].0 } else { all[idx].0 } as u64;
    let before = bytes.len() as u64;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&journal)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
    let after = std::fs::metadata(&journal).expect("stat").len();
    assert!(
        after < before,
        "the cut removed nothing: {before} -> {after}"
    );
    (before, after)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set the mode");
}

/// The `gx_code` on a refusal's stderr, or `None` when the run said nothing.
fn code_of(stderr: &str) -> Option<String> {
    let at = stderr.find("\"gx_code\"")?;
    let rest = &stderr[at + "\"gx_code\"".len()..];
    let open = rest.find('"')? + 1;
    let close = rest[open..].find('"')? + open;
    Some(rest[open..close].to_string())
}

/// The three read verbs, and whether a signature came out.
struct Reads {
    proof: (i32, Option<String>),
    consistency: (i32, Option<String>),
    checkpoint: (i32, Option<String>),
    signed: bool,
    leaf_99: i32,
}

fn reads(p: &Pipeline, leaf: &str, label: &str) -> Reads {
    let proof = run(p.gx().args(["log", "proof", "--leaf", leaf]));
    let consistency = run(p
        .gx()
        .args(["log", "consistency", "--from", "1", "--to", "1"]));
    let checkpoint = run(p.gx().args(["log", "checkpoint", "--key", &p.key_id]));
    let signed = checkpoint.stdout.contains("\"signature\"");
    let leaf_99 = run(p.gx().args(["log", "proof", "--leaf", "99"])).code;
    let out = Reads {
        proof: (proof.code, code_of(&proof.stderr)),
        consistency: (consistency.code, code_of(&consistency.stderr)),
        checkpoint: (checkpoint.code, code_of(&checkpoint.stderr)),
        signed,
        leaf_99,
    };
    println!(
        "R40_READS {label} proof={:?} consistency={:?} checkpoint={:?} signed={} leaf99={}",
        out.proof, out.consistency, out.checkpoint, out.signed, out.leaf_99
    );
    out
}

/// The write road, asked of **this** project rather than of a copy.
///
/// 🔴 `req/558` §2: audit 39 asked the write road on a `cp -a` duplicate so that the measured
/// project stayed read-only, and `cp -a` cannot copy a file it may not read — so the duplicate had
/// no journal at all and answered `JOURNAL_ABSENT` about a bed that no longer existed. Every arm
/// here therefore does its reads first and its one write last, on the project itself.
fn write_road(p: &Pipeline, label: &str) -> (i32, Option<String>) {
    let submitted = p.submit("a goal for the write road");
    let out = (submitted.code, code_of(&submitted.stderr));
    println!("R40_WRITE {label} submit={out:?}");
    out
}

fn repair_json(p: &Pipeline) -> serde_json::Value {
    let out = run(p.gx().args(["repair", "--json"]));
    serde_json::from_str(&out.stdout).unwrap_or(serde_json::Value::Null)
}

/// 🔴 `c1` — the journal made unreadable, with its healthy and its cut state as forerunners.
#[cfg(unix)]
#[test]
fn c1_a_journal_this_process_cannot_open_ends_the_read_road() {
    let p = support::pipeline("r40_c1", "before\n");
    let tid = p.commit_one("first");

    let healthy = reads(&p, &tid, "c1_healthy");
    assert_eq!(healthy.proof.0, 0, "the healthy project answers");
    assert!(healthy.signed, "the healthy project signs a checkpoint");

    cut_last_frame(&p);
    let cut = reads(&p, &tid, "c1_cut");
    assert_eq!(
        cut.checkpoint.1.as_deref(),
        Some("LEDGER_DISAGREES"),
        "the cut is what the product already refuses"
    );
    assert!(!cut.signed, "a disagreeing project does not sign");

    set_mode(&journal_of(&p), 0o000);
    let blind = reads(&p, &tid, "c1_unreadable");
    set_mode(&journal_of(&p), 0o600);

    assert_ne!(blind.proof.0, 0, "🔴 audit 39 M-01: this answered exit 0");
    assert_ne!(blind.consistency.0, 0, "the same question, second mouth");
    assert_ne!(blind.checkpoint.0, 0, "the same question, third mouth");
    assert!(
        !blind.signed,
        "🔴 audit 39 M-01: a **signature** came out of this project"
    );
}

/// 🔴 `c2` — the journal replaced by a directory of the same name. No permissions involved.
#[test]
fn c2_a_directory_where_the_journal_is_declared_ends_the_read_road() {
    let p = support::pipeline("r40_c2", "before\n");
    let tid = p.commit_one("first");
    cut_last_frame(&p);
    assert_eq!(
        reads(&p, &tid, "c2_cut").checkpoint.1.as_deref(),
        Some("LEDGER_DISAGREES")
    );

    let journal = journal_of(&p);
    std::fs::rename(&journal, journal.with_extension("kept")).expect("set the journal aside");
    std::fs::create_dir(&journal).expect("put a directory where the file is declared");

    let blocked = reads(&p, &tid, "c2_directory");
    assert!(!blocked.signed, "🔴 audit 39 M-01: this signed");
    for (verb, got) in [
        ("proof", &blocked.proof),
        ("consistency", &blocked.consistency),
        ("checkpoint", &blocked.checkpoint),
    ] {
        assert_eq!(
            got.1.as_deref(),
            Some("LAYOUT_BLOCKED"),
            "`{verb}` wears the word `req/38` §328 ruling 2 ② widened for it"
        );
    }
}

/// 🔴 `c3` — the same, on a project whose recorded head is gone (audit 39's `b4` shape).
#[cfg(unix)]
#[test]
fn c3_the_head_less_shape_does_not_reopen_the_escape() {
    let p = support::pipeline("r40_c3", "before\n");
    let tid = p.commit_one("first");
    std::fs::remove_file(p.project.join(".gx").join("checkpoints").join("head.json"))
        .expect("remove the recorded head");
    cut_last_frame(&p);
    assert_eq!(
        reads(&p, &tid, "c3_b4").checkpoint.1.as_deref(),
        Some("LEDGER_DISAGREES"),
        "R39 closed this shape and it is still closed"
    );

    set_mode(&journal_of(&p), 0o000);
    let blind = reads(&p, &tid, "c3_b4_unreadable");
    set_mode(&journal_of(&p), 0o600);
    assert!(!blind.signed, "🔴 audit 39 M-01, third form");
    assert_ne!(blind.checkpoint.0, 0);
}

/// 🔴 `c4` — **the negative control that matters most.** The third-party verifier still answers.
///
/// A ledger file and no journal is the caller `req/540` R-1b left the escape open for, and R40's
/// whole claim is that the escape's reach is an existence rather than a capability. If this arm
/// ever goes red, R40 broke a buyer's road to close an attacker's, and the repair is wrong rather
/// than the fixture.
#[test]
fn c4_a_ledger_with_no_journal_still_answers_and_still_proves() {
    let p = support::pipeline("r40_c4", "before\n");
    let tid = p.commit_one("first");
    std::fs::remove_file(journal_of(&p)).expect("remove the journal outright");

    let third_party = reads(&p, &tid, "c4_journal_deleted");
    assert_eq!(
        third_party.proof.0, 0,
        "🔴 the third-party verifier is not collateral damage"
    );
    assert_eq!(third_party.consistency.0, 0);
    assert_eq!(third_party.checkpoint.0, 0);
    assert!(
        third_party.signed,
        "and the checkpoint it asks for is signed, as it was before R40"
    );
}

/// 🔴 `c5` — the argument questions keep their exit **while the project opens**, and not otherwise.
#[test]
fn c5_the_argument_questions_keep_their_exit_on_the_projects_that_open() {
    let p = support::pipeline("r40_c5", "before\n");
    let tid = p.commit_one("first");
    assert_eq!(reads(&p, &tid, "c5_healthy").leaf_99, 6);
    cut_last_frame(&p);
    assert_eq!(
        reads(&p, &tid, "c5_cut").leaf_99,
        6,
        "a cut journal does not change the ledger file's own size"
    );
    std::fs::remove_file(journal_of(&p)).expect("remove the journal");
    assert_eq!(
        reads(&p, &tid, "c5_deleted").leaf_99,
        6,
        "neither does an absent one — this project still opens"
    );

    // The other half of R39's but-for clause, driven rather than asserted in prose.
    let q = support::pipeline("r40_c5b", "before\n");
    let qid = q.commit_one("first");
    let journal = journal_of(&q);
    std::fs::rename(&journal, journal.with_extension("kept")).expect("set the journal aside");
    std::fs::create_dir(&journal).expect("block the declared path");
    assert_ne!(
        reads(&q, &qid, "c5b_blocked").leaf_99,
        6,
        "a project `Layout::open` refuses is never far enough along for the ledger's size to be a \
         fact anybody consults"
    );
}

/// 🔴 `c6` — one condition, one word, measured on one project in one run.
#[cfg(unix)]
#[test]
fn c6_the_read_road_and_the_write_road_answer_with_the_same_word() {
    // Form A: the journal is a regular file this process cannot open.
    let a = support::pipeline("r40_c6a", "before\n");
    let atid = a.commit_one("first");
    cut_last_frame(&a);
    set_mode(&journal_of(&a), 0o000);
    let a_read = reads(&a, &atid, "c6a_read").checkpoint.1;
    let a_write = write_road(&a, "c6a").1;
    set_mode(&journal_of(&a), 0o600);
    assert_eq!(
        a_read, a_write,
        "🔴 `req/38` §156 ruling 2(a): the read road and the write road name one condition once"
    );

    // Form B: the declared path holds something that is not what the declaration says.
    let b = support::pipeline("r40_c6b", "before\n");
    let btid = b.commit_one("first");
    cut_last_frame(&b);
    let journal = journal_of(&b);
    std::fs::rename(&journal, journal.with_extension("kept")).expect("set the journal aside");
    std::fs::create_dir(&journal).expect("block the declared path");
    let b_read = reads(&b, &btid, "c6b_read").checkpoint.1;
    let b_write = write_road(&b, "c6b").1;
    assert_eq!(b_read, b_write, "the same equality, second form");
    assert_eq!(b_read.as_deref(), Some("LAYOUT_BLOCKED"));

    // 🔴 The negative control: equality is not "make everything one word". A project with **no**
    // journal answers the read road with a proof and the write road with `JOURNAL_ABSENT`, and
    // those two disagreeing is the correct outcome rather than a defect.
    let c = support::pipeline("r40_c6c", "before\n");
    let ctid = c.commit_one("first");
    std::fs::remove_file(journal_of(&c)).expect("remove the journal");
    assert_eq!(reads(&c, &ctid, "c6c_read").proof.0, 0);
    assert_eq!(write_road(&c, "c6c").1.as_deref(), Some("JOURNAL_ABSENT"));
}

/// 🔴 `c7` — `gx repair` stops calling a journal it could not `stat` an absent one.
#[cfg(unix)]
#[test]
fn c7_a_journal_this_process_could_not_look_at_is_not_reported_absent() {
    let p = support::pipeline("r40_c7", "before\n");
    p.commit_one("first");
    let ledger_dir = p.project.join(".gx").join("ledger");
    let bytes = std::fs::metadata(journal_of(&p))
        .expect("stat the journal")
        .len();
    assert!(bytes > 0, "the bed has a journal with bytes in it");

    let healthy = repair_json(&p);
    assert_eq!(
        healthy["journal_absent"],
        serde_json::json!(false),
        "the control: this project's journal is there and the report says so"
    );

    set_mode(&ledger_dir, 0o000);
    let blind = repair_json(&p);
    set_mode(&ledger_dir, 0o700);
    println!(
        "R40_REPAIR_BLIND journal_absent={}",
        blind["journal_absent"]
    );
    assert_ne!(
        blind["journal_absent"],
        serde_json::json!(true),
        "🔴 `Path::exists()` folded EACCES to `false` and this printed `true` about a journal \
         holding {bytes} bytes. An operator reading that restores from a backup over a file that \
         was never lost"
    );

    // And the same project, made whole again, still reports the truth.
    let after = repair_json(&p);
    assert_eq!(after["journal_absent"], serde_json::json!(false));
}

/// 🔴 `c8` — the word each form wears today, pinned by name.
///
/// 🔴 **DR-B / `req/38` §337, `req/565` §3 — the thirteenth word landed.** Form A used to wear
/// `INTERNAL` (`req/38` §328 ruling 2 ③ chose a generic over a falsehood and ④ filed the DR); this
/// pin moved the moment the DR was ruled, so a later lane touching either side of the mint has to
/// say so here rather than sliding through unmeasured.
#[cfg(unix)]
#[test]
fn c8_each_form_wears_the_word_this_release_gives_it() {
    let a = support::pipeline("r40_c8a", "before\n");
    let atid = a.commit_one("first");
    cut_last_frame(&a);
    set_mode(&journal_of(&a), 0o000);
    let a_word = reads(&a, &atid, "c8a").checkpoint.1;
    set_mode(&journal_of(&a), 0o600);
    assert_eq!(
        a_word.as_deref(),
        Some("JOURNAL_UNREADABLE"),
        "🔴 DR-B (`req/38` §337) minted the word the vocabulary lacked for \"present, the declared \
         shape, and unopenable\" — see `docs/LIMITS.md` and `crates/gx-api/src/gx_code.rs`'s \
         `JOURNAL_UNREADABLE`"
    );

    let b = support::pipeline("r40_c8b", "before\n");
    let btid = b.commit_one("first");
    let journal = journal_of(&b);
    std::fs::rename(&journal, journal.with_extension("kept")).expect("set the journal aside");
    std::fs::create_dir(&journal).expect("block the declared path");
    assert_eq!(
        reads(&b, &btid, "c8b").checkpoint.1.as_deref(),
        Some("LAYOUT_BLOCKED"),
        "and this one does have a word, once its title says \"path\" rather than \"directory\""
    );
}
