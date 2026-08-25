// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R37 / `req/501` M-01 + M-02** — the red bed for two `Err`-road sentences that overshoot,
//! written **before** the repair (`req/38` §226).
//!
//! # The bed
//!
//! R36's own: `a_project_cut_inside_the_window(drop_the_leaf = true)`, a third party's
//! `THIRD PARTY\n` written to the target afterwards, and `.gx/checkpoints/` sealed `0o555` so that
//! [`gx_engine::pipeline::Engine::record_head`] raises. 43 §7-3c re-applies the delta, walks eight
//! fallible steps, writes the **`Committed` journal record**, and then fails on the head.
//!
//! # M-01 — the sentence says a record that exists does not
//!
//! `applied_unrecorded_note_for` tells the operator this run "left **no terminal record** of having
//! done so. The row stays resumable", and `gx repair`'s remedy says "those object(s) have been
//! changed and **no terminal record says so**". Audit 36 (`req/496` §4-1) counted the journal on
//! this exact bed: `Committed` records went from 1 to **2**. The record is on the disk. Carrying
//! out the remedy's instruction — run a write verb again — answers `terminal: 2, resumed: 0`,
//! because there was never a resumable row to close.
//!
//! The repair owes a **third** sentence, for the shape that actually happened: the delta was
//! applied, the terminal record **was** written, and the head alone did not move.
//!
//! # M-02 — the counter counts rows this run never touched
//!
//! `engine_open_failed.finished_before_failure` is `Engine::recover`'s `out` vector, and that
//! vector is also where the outer loop pushes `RecoveryPath::Terminal` — commits that closed
//! **before this process started**. The remedy calls them "row(s) had already been finished by
//! this same recovery". On this bed the recovery closes **zero** rows and the field says 1.
//!
//! # The three wirings
//!
//! `req/496` §8 re-confirmed the three call sites (`session.rs:1231`, `repair.rs:786`,
//! `serve.rs:713`), and `req/501` §0 requires the repaired text to be measured at each rather than
//! inferred from one. `gx verify` reaches the first, `gx repair --yes` the second, `gx serve` the
//! third.
//!
//! `cfg(unix)` for the `chmod`, as R36's bed is.

#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use support::{pipeline, run, Pipeline};

/// R36's `Err`-road sentence: the clause that is true on this bed.
const APPLIED_MARK: &str = "wrote its delta and then could not record it";
/// R36's clause that is false on this bed, and the one the repair must stop printing here.
const NO_TERMINAL_RECORD_MARK: &str = "left no terminal record of having done so";
/// The remedy's spelling of the same false clause.
const REMEDY_NO_TERMINAL_MARK: &str = "no terminal record says so";
/// 🔴 The sentence the repair owes: the record landed and the head did not.
const RECORDED_MARK: &str = "recorded the commit and could not move the head";

// ---------------------------------------------------------------------------
// Byte surgery — copied verbatim from `r36_error_road.rs`, which is where audit 36 had to go
// after re-deriving it from memory cost two runs (`req/496` §7 item 1).
// ---------------------------------------------------------------------------

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

fn ledger_frames(bytes: &[u8]) -> Vec<(usize, usize)> {
    const CEILING: u32 = 1 << 20;
    let mut at = 0usize;
    let mut out = Vec::new();
    while at + 4 <= bytes.len() {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > CEILING as usize || at + 4 + length > bytes.len() {
            break;
        }
        out.push((at, 4 + length));
        at += 4 + length;
    }
    out
}

fn truncate_at(path: &Path, at: u64) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open to truncate");
    f.set_len(at).expect("truncate");
    f.sync_all().ok();
}

fn layout_of(fixture: &Pipeline) -> gx_cli::layout::Layout {
    gx_cli::layout::Layout::open(&fixture.project).expect("the project is open")
}

fn receipt_files(fixture: &Pipeline) -> Vec<PathBuf> {
    let dir = fixture.project.join(".gx").join("receipts");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn a_project_cut_inside_the_window(fixture: &Pipeline, drop_the_leaf: bool) -> String {
    let first = fixture.commit_one("one\n");
    let l0 = layout_of(fixture);
    let head_before = std::fs::read(l0.head_path()).ok();
    let receipts_before = receipt_files(fixture);
    fixture.commit_one("two\n");
    for p in receipt_files(fixture) {
        if !receipts_before.contains(&p) {
            let _ = std::fs::remove_file(&p);
        }
    }
    match &head_before {
        Some(bytes) => {
            std::fs::write(l0.head_path(), bytes).expect("put the head back");
        }
        None => {
            let _ = std::fs::remove_file(l0.head_path());
        }
    }

    let l = layout_of(fixture);
    let journal_path = l.journal_path();
    let ledger_path = l.ledger_path();
    let journal = std::fs::read(&journal_path).expect("read the journal");
    let kinds: Vec<&'static str> = gx_engine::replay(&journal)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    let spans = frames(&journal);
    assert_eq!(spans.len(), kinds.len(), "instrument: one frame per record");
    let last_apply = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "ApplyStarted")
        .map(|(i, _)| i)
        .next_back()
        .expect("instrument: the second commit announced its apply");
    truncate_at(
        &journal_path,
        (spans[last_apply].0 + spans[last_apply].1) as u64,
    );

    if drop_the_leaf {
        let ledger = std::fs::read(&ledger_path).expect("read the ledger");
        let leaves = ledger_frames(&ledger);
        let at = leaves.get(1).map_or(0, |leaf| leaf.0 as u64);
        truncate_at(&ledger_path, at);
    }
    first
}

fn journal_kinds(fixture: &Pipeline) -> Vec<&'static str> {
    let l = layout_of(fixture);
    let bytes = std::fs::read(l.journal_path()).unwrap_or_default();
    gx_engine::replay(&bytes)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect()
}

fn seal(fixture: &Pipeline) -> PathBuf {
    let dir = fixture.project.join(".gx").join("checkpoints");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555))
        .expect("seal the head directory");
    dir
}

fn unseal(dir: &Path) {
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

/// The bed, ready for a verb: `Committed` record count before the run, and the id the verb is
/// given.
struct Bed {
    fixture: Pipeline,
    dir: PathBuf,
    committed_before: usize,
    subject: String,
}

/// `plan_after` is R36's own bed parameter and the reason is worth keeping in front of a reader:
/// `gx verify` and `gx commit` are planned **after** the third party writes, because otherwise the
/// stale `Fingerprint0` refuses in front of `recover` and the bed never reaches the road at all
/// (audit 34 §7-3). The first red run of this suite proved it again — `gx verify` handed the
/// already-terminal id answered `rc 1` with **0 bytes on stderr** and the journal untouched.
fn a_sealed_bed(name: &str, plan_after: bool) -> Bed {
    let fixture = pipeline(name, "before\n");
    let first = a_project_cut_inside_the_window(&fixture, true);
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    let subject = if plan_after {
        fixture.planned_one("three\n")
    } else {
        first
    };
    let kinds = journal_kinds(&fixture);
    let committed_before = kinds.iter().filter(|k| **k == "Committed").count();
    println!("R37_BED bed={name} kinds_before={kinds:?} committed_before={committed_before}");
    assert_eq!(
        committed_before, 1,
        "the bed failed before the product did: exactly one row must already be terminal, so that \
         `finished_before_failure` has something wrong to count"
    );
    let dir = seal(&fixture);
    Bed {
        fixture,
        dir,
        committed_before,
        subject,
    }
}

/// What one verb said, and what the journal said afterwards.
struct Walk {
    verb: &'static str,
    code: i32,
    moved: bool,
    stderr: String,
    stdout: String,
    committed_after: usize,
}

fn measure(
    verb: &'static str,
    bed: &Bed,
    run_it: impl FnOnce(&Pipeline, &str) -> support::Run,
) -> Walk {
    let before = bed.fixture.target_contents();
    let r = run_it(&bed.fixture, &bed.subject);
    let after = bed.fixture.target_contents();
    let committed_after = journal_kinds(&bed.fixture)
        .iter()
        .filter(|k| **k == "Committed")
        .count();
    println!(
        "R37_WALK verb={verb} rc={} moved={} committed_before={} committed_after={}",
        r.code,
        before != after,
        bed.committed_before,
        committed_after
    );
    println!("R37_WALK verb={verb} stderr=<<{}>>", r.stderr);
    Walk {
        verb,
        code: r.code,
        moved: before != after,
        stderr: r.stderr,
        stdout: r.stdout,
        committed_after,
    }
}

/// The two questions every wiring is asked, so that no wiring is inferred from another.
fn the_sentence_matches_the_disk(walk: &Walk, committed_before: usize) {
    assert!(
        walk.moved,
        "{}: the bed failed before the product did — the recovery never reached the road that \
         applies",
        walk.verb
    );
    assert_eq!(
        walk.committed_after,
        committed_before + 1,
        "{}: the bed failed before the product did — `journal_append(Committed)` did not run, so \
         there is no false sentence to measure",
        walk.verb
    );
    assert!(
        !walk.stderr.contains(NO_TERMINAL_RECORD_MARK),
        "🔴 req/496 M-01 ({}): the sentence says this run `left no terminal record of having done \
         so` while the journal it just wrote holds one more `Committed` record than it did before. \
         The operator is told to run a write verb again to close a row that is already closed",
        walk.verb
    );
    assert!(
        walk.stderr.contains(RECORDED_MARK),
        "🔴 req/496 M-01 ({}): the `Err` road that got past the `Committed` record owes its own \
         sentence — the delta landed, the terminal record landed, and the head alone did not — and \
         nothing on stderr says it",
        walk.verb
    );
}

// ---------------------------------------------------------------------------
// Wiring 1 — `repair.rs:786`
// ---------------------------------------------------------------------------

#[test]
fn r37_m01_m02_gx_repair_tells_the_truth_about_the_record_and_the_counter() {
    let bed = a_sealed_bed("r37_repair_sealed", false);
    let walk = measure("gx repair", &bed, |f, _| {
        run(f
            .gx()
            .args(["repair", "--json", "--yes"])
            .args(["--signing-key", &f.key_id]))
    });
    unseal(&bed.dir);
    assert!(
        walk.stderr.contains(APPLIED_MARK) || walk.stderr.contains(RECORDED_MARK),
        "the bed failed before the product did: neither `Err`-road sentence appeared at all"
    );
    the_sentence_matches_the_disk(&walk, bed.committed_before);

    // ---- M-02, and the remedy that quotes it ----
    let j: Value = serde_json::from_str(walk.stdout.trim()).unwrap_or(Value::Null);
    let failed = &j["engine_open_failed"];
    let finished = failed["finished_before_failure"].as_u64();
    let remedy = j["remedy"].as_str().unwrap_or_default().to_string();
    println!(
        "R37_M02 stage={} applied={} finished={finished:?} recorded={}",
        failed["stage"], failed["applied_before_failure"], failed["recorded_before_failure"]
    );
    println!("R37_M02 remedy=<<{remedy}>>");

    assert_eq!(
        finished,
        Some(0),
        "🔴 req/496 M-02: `finished_before_failure` is documented as `real commits that this run \
         closed` and the remedy calls them `row(s) had already been finished by this same \
         recovery`. This run closed none: the row it counted was `Committed` before the process \
         started and reached `out` through the outer loop's `RecoveryPath::Terminal` push"
    );
    assert!(
        !remedy.contains(REMEDY_NO_TERMINAL_MARK),
        "🔴 req/496 M-01: the JSON remedy repeats the clause the journal falsifies"
    );

    // ---- A3: carry out what the remedy tells the operator to do ----
    let r2 = run(bed
        .fixture
        .gx()
        .args(["repair", "--json", "--yes"])
        .args(["--signing-key", &bed.fixture.key_id]));
    let j2: Value = serde_json::from_str(r2.stdout.trim()).unwrap_or(Value::Null);
    println!(
        "R37_SECOND_RUN rc={} repaired={} recover={} stderr_bytes={} kinds={:?}",
        r2.code,
        j2["repaired"],
        j2["recover"],
        r2.stderr.len(),
        journal_kinds(&bed.fixture)
    );
    // Negative control: the healthy re-run must stay exactly what R36 measured — nothing to
    // resume, nothing said, and the journal unchanged.
    assert_eq!(
        r2.code, 0,
        "negative control: the unsealed re-run must succeed"
    );
    assert_eq!(
        j2["recover"]["resumed"], 0,
        "negative control: there is nothing left to resume, which is the fact M-01's sentence \
         denies"
    );
    assert_eq!(
        journal_kinds(&bed.fixture)
            .iter()
            .filter(|k| **k == "Committed")
            .count(),
        bed.committed_before + 1,
        "negative control: the second run wrote no further terminal record"
    );
}

// ---------------------------------------------------------------------------
// Wiring 2 — `session.rs:1231`, entered by `gx verify`
// ---------------------------------------------------------------------------

#[test]
fn r37_m01_gx_verify_says_it_too() {
    let bed = a_sealed_bed("r37_verify_sealed", true);
    let walk = measure("gx verify", &bed, |f, id| run(f.gx().args(["verify", id])));
    unseal(&bed.dir);
    the_sentence_matches_the_disk(&walk, bed.committed_before);
    assert_ne!(
        walk.code, 0,
        "the verb still fails; only what it says changes"
    );
}

// ---------------------------------------------------------------------------
// Wiring 3 — `serve.rs:713`
// ---------------------------------------------------------------------------

#[test]
fn r37_m01_gx_serve_says_it_too() {
    let bed = a_sealed_bed("r37_serve_sealed", false);
    let token_file = bed.fixture.project.join("token");
    std::fs::write(&token_file, "r37-serve-token\n").expect("write the token file");
    let before = bed.fixture.target_contents();

    let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
        .env("HOME", &bed.fixture.home)
        .env("USERPROFILE", &bed.fixture.home)
        .arg("--project")
        .arg(&bed.fixture.project)
        .arg("serve")
        .args(["--bind", "127.0.0.1:0"])
        .arg("--token-file")
        .arg(&token_file)
        .args(["--signing-key", &bed.fixture.key_id])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("gx serve starts");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let exited = loop {
        match child.try_wait().expect("wait on gx serve") {
            Some(_) => break true,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                break false;
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let out = child.wait_with_output().expect("collect gx serve output");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let after = bed.fixture.target_contents();
    unseal(&bed.dir);

    let committed_after = journal_kinds(&bed.fixture)
        .iter()
        .filter(|k| **k == "Committed")
        .count();
    println!(
        "R37_WALK verb=gx serve exited={exited} moved={} committed_before={} committed_after={}",
        before != after,
        bed.committed_before,
        committed_after
    );
    println!("R37_WALK verb=gx serve stderr=<<{stderr}>>");

    assert!(
        exited,
        "the bed failed before the product did: `gx serve` did not exit"
    );
    let walk = Walk {
        verb: "gx serve",
        code: 1,
        moved: before != after,
        stderr,
        stdout: String::new(),
        committed_after,
    };
    the_sentence_matches_the_disk(&walk, bed.committed_before);
}

// ---------------------------------------------------------------------------
// The control `req/496` §6 declared non-discriminating, re-run rather than dropped
// ---------------------------------------------------------------------------

/// 🔴 A project with **no earlier commit**. Audit 36 built this to tell "the counter counts
/// pre-existing terminals" from "the counter is right", and reported (`req/496` §7 item 3) that it
/// does not discriminate: with no floor, R7/`req/232` M-01's laundering guard means `record_head`
/// never runs, so the `Err` road is unreachable and the bed answers `rc 0`.
///
/// It is kept and re-measured rather than deleted, because the reason it does not discriminate is
/// a fact about a **different** guard and is worth having on the record. The discriminating
/// control for the counter is `crates/gx-engine/tests/r37_recover_partial.rs`, which drives the
/// count over a constructed set — the two-`Committing`-row bed this instrument cannot build, since
/// truncation only removes a suffix and so can leave exactly one row mid-flight.
#[test]
fn r37_control_the_counter_on_a_project_with_no_earlier_commit() {
    let bed = "r37_one_commit";
    let fixture = pipeline(bed, "before\n");
    fixture.commit_one("two\n");
    let l = layout_of(&fixture);
    for p in receipt_files(&fixture) {
        let _ = std::fs::remove_file(&p);
    }
    let _ = std::fs::remove_file(l.head_path());
    let journal = std::fs::read(l.journal_path()).expect("read the journal");
    let kinds: Vec<&'static str> = gx_engine::replay(&journal)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    let spans = frames(&journal);
    let last_apply = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "ApplyStarted")
        .map(|(i, _)| i)
        .next_back()
        .expect("instrument: the commit announced its apply");
    truncate_at(
        &l.journal_path(),
        (spans[last_apply].0 + spans[last_apply].1) as u64,
    );
    let ledger = std::fs::read(l.ledger_path()).expect("read the ledger");
    let leaves = ledger_frames(&ledger);
    let at = leaves.first().map_or(0, |leaf| leaf.0 as u64);
    truncate_at(&l.ledger_path(), at);

    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    let dir = fixture.project.join(".gx").join("checkpoints");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("seal");
    let r = run(fixture
        .gx()
        .args(["repair", "--json", "--yes"])
        .args(["--signing-key", &fixture.key_id]));
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
    let j: Value = serde_json::from_str(r.stdout.trim()).unwrap_or(Value::Null);
    println!(
        "R37_CONTROL_NO_FLOOR rc={} reached_err_road={} finished={}",
        r.code,
        !j["engine_open_failed"].is_null(),
        j["engine_open_failed"]["finished_before_failure"]
    );
}
