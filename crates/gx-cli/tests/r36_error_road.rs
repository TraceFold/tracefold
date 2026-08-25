// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/476` H-01, red-first** (`req/38` §271 ruling 5 item 1; reqdef `req/479` §0-1).
//!
//! # The claim this suite exists to make true
//!
//! R35 wired 43 §7-3c's sentence to every shipped verb — on the `Ok` road. Audit 35 walked the
//! **`Err`** road and found the other half unrepaired. Its measurement, from
//! `C:/work/a35_logs/error_road.log` (verbatim):
//!
//! ```text
//! A35E_SUMMARY moved=["gx repair", "gx undo", "gx verify", "gx commit"]
//!              moved_and_silent=["gx repair", "gx undo", "gx verify", "gx commit"]
//! ```
//!
//! The mechanism is `Engine::recover`'s loop: `out.push(self.resume(..)?)`. `Engine::resume` has
//! **eight** fallible steps *after* `apply_once` has written the delta to somebody's substrate, and
//! the `?` throws away both the rows already recovered and the fact that a delta was applied. All
//! three shipped announcement sites sit on the `Ok` arm, so nothing is said.
//!
//! And `gx repair --yes --json` answers `repaired: false`, `recover: null`,
//! `engine_open_failed.stage: "recover"` with a remedy that begins "the engine **refused** at
//! `recover`" — while the same product, on 43 §7-3b's road, defines that word for the operator:
//!
//! ```text
//! **Nothing was applied**: on this road the `postcondition_fingerprint` is *read* off the
//! substrate rather than produced by re-applying the delta, so `adapter.apply` was never called
//! ```
//!
//! ∴ a destructive terminal state is presented in the shape of a non-occurrence.
//!
//! # 🔴 This bed is a **reconstruction**, and the reason is on the record
//!
//! `req/38` §271-B: the audit-35 probe sources were deleted by the reviewer's
//! `git worktree remove --force` before they had been copied to a scripts directory, so there is no
//! sha to check this file against. What survives is `req/476` §1-3 and the raw log above, both
//! untouched. This bed is therefore rebuilt from `r35_shared_road_sentence.rs`'s construction (the
//! same one audit 34, audit 35 and R35 all cut) plus the audit's own failure injection, and its
//! equivalence to audit 35's bed is shown the only way it can be: by **reproducing audit 35's two
//! negative controls** — `unsealed` (rc 0, moved, 838 bytes of sentence on stderr) and
//! `healthy_sealed` (sealed, `moved=false`, silent). If those two reproduce, this bed is on the
//! same road audit 35 measured.
//!
//! # The failure injection is not synthetic
//!
//! `.gx/receipts/` loses its write bit (`0o755` -> `0o555`), which makes `Engine::file_receipt`
//! fail — the **last** of the eight steps, so `apply_once`, `rebuilt_attest`, `ledger.append` and
//! `prove_inclusion` all really run first. The engine's own error sentence names "a permission, a
//! full disk and a path occupied by something that is not a directory" as the three causes it
//! expects here, so this is an operating condition the product predicts rather than one this bed
//! invented.
//!
//! # Red-first (`req/38` §226)
//!
//! No symbol this lane creates is named anywhere in this file. Every arm drives the shipped `gx`
//! binary and reads bytes that binary produced (JSON keys are read as strings), so the suite
//! compiles at the commit that precedes the repair and fails on its assertions there.
//!
//! `cfg(unix)` for the `chmod`, as every sibling suite says.

#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::Value;
use support::{pipeline, run, Pipeline, Run};

/// A fragment of R35's closed-row sentence: the clause that says the delta was written.
const NOTE_MARK: &str = "applying its delta";
/// The clause that carries what a counter cannot: what was not compared.
const NOT_CHECKED_MARK: &str = "was **not** checked";
/// The clause that carries the other half: that this run may have destroyed somebody's bytes.
const OVERWROTE_MARK: &str = "written over it and cannot tell you so";
/// 🔴 The clause this lane owes the `Err` road: the delta landed and the recording did not.
const UNRECORDED_MARK: &str = "wrote its delta and then could not record it";
/// 🔴 **R37 / `req/496` M-01** — the sentence a row past the `Committed` record earns instead.
const RECORDED_MARK: &str = "recorded the commit and could not move the head";
/// The word `gx repair`'s remedy uses today, and which this product defines elsewhere as
/// "Nothing was applied".
const REFUSED_AT_RECOVER: &str = "refused at `recover`";

// ---------------------------------------------------------------------------
// Byte surgery - the same construction `r35_shared_road_sentence.rs` uses, which took it from
// `a34_silent_roads.rs`, which took it from `a33_shipping_verbs.rs`. Four lanes, one cut.
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
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
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

/// Two commits, then the journal cut back to just after the second's `ApplyStarted`, the head put
/// back, and the second commit's receipts removed. `drop_the_leaf` makes it 43 §7-3c's road.
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
    assert_eq!(
        spans.len(),
        kinds.len(),
        "instrument: one frame per record ({} vs {})",
        spans.len(),
        kinds.len()
    );
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

// ---------------------------------------------------------------------------
// The failure injection, and the snapshot that says whether the road was there
// ---------------------------------------------------------------------------

/// Take the write bit off `.gx/receipts/`, which is audit 35's injection.
fn seal(fixture: &Pipeline, bed: &str) -> PathBuf {
    let dir = fixture.project.join(".gx").join("receipts");
    let was = std::fs::metadata(&dir)
        .expect("the archive is there")
        .permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("seal");
    let now = std::fs::metadata(&dir).expect("still there").permissions();
    println!(
        "R36E_SEAL bed={bed} dir={:?} was={:o} now={:o}",
        dir.display().to_string(),
        was.mode(),
        now.mode()
    );
    dir
}

/// Put the write bit back, so the scratch tree can be cleaned up by whoever comes next.
fn unseal(dir: &Path) {
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

/// `gx repair --json` **without** `--yes`: reads, recovers nothing. Audit 34 §7-3's instrument, and
/// the only thing that tells "this verb left the world alone" from "this bed missed the road".
fn snapshot(fixture: &Pipeline, bed: &str, when: &str) -> Value {
    let r = run(fixture.gx().args(["repair", "--json"]));
    let j: Value = serde_json::from_str(r.stdout.trim()).unwrap_or(Value::Null);
    println!(
        "R36E_SNAP bed={bed} when={when} rc={} journal_commits={} ledger_leaves={} world={:?}",
        r.code,
        j["journal_commits"],
        j["ledger_leaves"],
        fixture.target_contents()
    );
    j
}

struct Walk {
    bed: String,
    verb: &'static str,
    code: i32,
    world_before: String,
    world_after: String,
    stdout: String,
    stderr: String,
}

impl Walk {
    fn moved(&self) -> bool {
        self.world_before != self.world_after
    }

    /// Did **anything** this run printed, on either stream, say that a delta was applied?
    fn sentence_anywhere(&self) -> bool {
        let all = format!("{}{}", self.stdout, self.stderr);
        all.contains(NOTE_MARK) || all.contains(UNRECORDED_MARK)
    }

    /// The acceptance condition: on **stderr**, where a human reads it.
    fn sentence_on_stderr(&self) -> bool {
        self.stderr.contains(NOTE_MARK) || self.stderr.contains(UNRECORDED_MARK)
    }

    fn report(&self) {
        println!(
            "R36E_WALK bed={} verb={} rc={} world_before={:?} world_after={:?} moved={} \
             sentence_anywhere={} on_stderr={} stdout_bytes={} stderr_bytes={}",
            self.bed,
            self.verb,
            self.code,
            self.world_before,
            self.world_after,
            self.moved(),
            self.sentence_anywhere(),
            self.sentence_on_stderr(),
            self.stdout.len(),
            self.stderr.len()
        );
        println!(
            "R36E_RAW_STDOUT bed={} verb={} <<{}>>",
            self.bed, self.verb, self.stdout
        );
        println!(
            "R36E_RAW_STDERR bed={} verb={} <<{}>>",
            self.bed, self.verb, self.stderr
        );
    }
}

fn walk(fixture: &Pipeline, bed: &str, verb: &'static str, r: Run, before: String) -> Walk {
    let w = Walk {
        bed: bed.to_string(),
        verb,
        code: r.code,
        world_before: before,
        world_after: fixture.target_contents(),
        stdout: r.stdout,
        stderr: r.stderr,
    };
    w.report();
    w
}

/// One bed: cut inside the window, let a third party write, seal the archive, drive the verb.
fn sealed_road<F>(bed: &'static str, verb: &'static str, plan_after: bool, f: F) -> Walk
where
    F: FnOnce(&Pipeline, &str) -> Run,
{
    let fixture = pipeline(bed, "before\n");
    let first = a_project_cut_inside_the_window(&fixture, true);
    snapshot(&fixture, bed, "before");
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    // `gx verify` / `gx commit` are planned **after** the third party writes: otherwise the stale
    // `Fingerprint0` refuses in front of `recover` and the bed never reaches the road (audit 34
    // §7-3, carried into `r35_shared_road_sentence.rs`).
    let subject = if plan_after {
        fixture.planned_one("three\n")
    } else {
        first
    };
    let dir = seal(&fixture, bed);
    let before = fixture.target_contents();
    let w = walk(&fixture, bed, verb, f(&fixture, &subject), before);
    unseal(&dir);
    snapshot(&fixture, bed, "after");
    w
}

// ---------------------------------------------------------------------------
// 1. The four verbs, on the road that writes and then fails
// ---------------------------------------------------------------------------

/// 🔴 `req/476` H-01. The delta lands, the recording does not, and every verb goes quiet.
#[test]
fn r36_the_error_road_after_the_delta_was_applied_is_told() {
    let repair = sealed_road(
        "r36_repair_sealed",
        "gx repair --yes --json",
        false,
        |f, _| {
            run(f
                .gx()
                .args(["repair", "--json", "--yes"])
                .args(["--signing-key", &f.key_id]))
        },
    );
    let undo = sealed_road("r36_undo_sealed", "gx undo", false, |f, id| {
        run(f.gx().args(["undo", id, "--settle", "1"]))
    });
    let verify = sealed_road("r36_verify_sealed", "gx verify", true, |f, id| {
        run(f.gx().args(["verify", id]))
    });
    let commit = sealed_road("r36_commit_sealed", "gx commit", true, |f, id| {
        run(f.gx().args(["commit", id]))
    });

    let roads: Vec<(&'static str, &'static str, &Walk)> = vec![
        ("gx repair", "gx repair:", &repair),
        ("gx undo", "gx undo:", &undo),
        ("gx verify", "gx verify:", &verify),
        ("gx commit", "gx commit:", &commit),
    ];

    let mut never_on_the_road: Vec<&str> = Vec::new();
    let mut silent: Vec<&str> = Vec::new();
    let mut mislabelled: Vec<&str> = Vec::new();
    for (verb, prefix, w) in &roads {
        if !w.moved() {
            never_on_the_road.push(verb);
        }
        if !w.sentence_on_stderr() {
            silent.push(verb);
        } else if !w.stderr.contains(prefix) {
            mislabelled.push(verb);
        }
    }
    println!(
        "R36E_SUMMARY moved={:?} silent={silent:?} mislabelled={mislabelled:?}",
        roads
            .iter()
            .filter(|(_, _, w)| w.moved())
            .map(|(v, _, _)| *v)
            .collect::<Vec<_>>()
    );

    assert!(
        never_on_the_road.is_empty(),
        "the bed failed before the product did: {never_on_the_road:?} did not move the world, so \
         this run measures the bed's limit and not the verb's behaviour (audit 34 §7-3)"
    );
    assert!(
        silent.is_empty(),
        "req/476 H-01: {silent:?} walked 43 §7-3c's road, wrote a delta over whatever the \
         substrate held, failed while recording it, and printed no sentence on stderr. The `Ok` \
         arm has said this since R35; the `Err` arm is the half `Engine::recover`'s `?` throws away"
    );
    assert!(
        mislabelled.is_empty(),
        "req/479 §0-1: {mislabelled:?} printed the sentence under another verb's name"
    );

    for (verb, _, w) in &roads {
        assert!(
            w.stderr.contains(UNRECORDED_MARK),
            "{verb}: the `Err` road's sentence must say the delta landed and the record did not — \
             R35's `finished ... by applying its delta` is false here, because the row was not \
             finished: <<{}>>",
            w.stderr
        );
        assert!(
            w.stderr.contains(NOT_CHECKED_MARK),
            "{verb}: the sentence must still say what was **not** compared"
        );
        assert!(
            w.stderr.contains(OVERWROTE_MARK),
            "{verb}: the sentence must still say that this run may have written over somebody \
             else's bytes"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. ③ — the destructive terminal state must not be offered as a non-occurrence
// ---------------------------------------------------------------------------

/// 🔴 `req/476` §1-4. `gx repair --yes --json` answered `repaired: false`, `recover: null`,
/// `engine_open_failed.stage: "recover"` and a remedy beginning "the engine refused at `recover`"
/// — over a world it had just overwritten. The word is not free: this same binary, on 43 §7-3b's
/// road, prints "**Nothing was applied** ... `adapter.apply` was never called".
#[test]
fn r36_repair_json_does_not_name_a_write_as_a_non_occurrence() {
    let w = sealed_road(
        "r36_repair_sealed_json",
        "gx repair --yes --json",
        false,
        |f, _| {
            run(f
                .gx()
                .args(["repair", "--json", "--yes"])
                .args(["--signing-key", &f.key_id]))
        },
    );
    assert!(
        w.moved(),
        "the bed failed before the product did: the world did not move"
    );
    let j: Value = serde_json::from_str(w.stdout.trim()).expect("one JSON object on stdout");
    let failed = &j["engine_open_failed"];
    println!(
        "R36E_JSON stage={} applied={}",
        failed["stage"], failed["applied_before_failure"]
    );

    let applied = failed["applied_before_failure"].as_array();
    assert!(
        applied.is_some_and(|rows| !rows.is_empty()),
        "req/476 §1-4: the report must name the rows whose delta this run wrote before it failed. \
         An operator reading `repaired: false` / `recover: null` / `stage: \"recover\"` concludes \
         the recovery did not run, and the file on their disk says otherwise: {j}"
    );
    let remedy = j["remedy"].as_str().unwrap_or_default();
    let reason = failed["reason"].as_str().unwrap_or_default();
    assert!(
        !remedy.contains(REFUSED_AT_RECOVER) && !reason.contains(REFUSED_AT_RECOVER),
        "req/476 §1-4: `refused` is this product's own word for \"Nothing was applied\", and a \
         delta was applied on this run. remedy=<<{remedy}>>"
    );
}

// ---------------------------------------------------------------------------
// 2b. A **second** of the eight steps, so the claim is not about one of them
// ---------------------------------------------------------------------------

/// 🔴 `req/476` §7-2, which audit 35 declared against itself: of the eight fallible steps after
/// `apply_once`, it drove **one** (`file_receipt`, the last), and "all eight are silent" was an
/// inference from the structure.
///
/// This drives a second, and deliberately not a neighbouring one: sealing `.gx/checkpoints/` makes
/// `record_head` fail. It is the step *after* the ledger append, the inclusion proof, the receipt
/// and the `Committed` journal record — so on this bed the row is very nearly closed and the world
/// is very definitely moved. If `applied_unrecorded` were an artifact of the archive being
/// unwritable rather than of the delta having been applied, this bed would not reproduce it.
///
/// Two of eight is still not eight, and the report says so.
///
/// # 🔴 **R37 / `req/496` M-01 — what this test asserted, and why it changed**
///
/// This suite shipped asserting that a failure at `record_head` "leaves the same world and owes the
/// **same sentence**", and R36's own doc above says of this bed that "the row is very nearly
/// closed". Audit 36 took the two sentences at their word and measured the difference between them:
/// `Committed` records on this bed go from 1 to **2**, so the row is not *nearly* closed, it **is**
/// closed — and the sentence being asserted told the operator this run "left no terminal record of
/// having done so" and that "the row stays resumable". Carrying out the remedy answered
/// `terminal: 2, resumed: 0`.
///
/// The name of this test is R36's and stays: the claim it makes — a second of the eight steps is
/// told, in this verb's own name, over a world that moved — is still the claim, and it still holds.
/// What changed is that the two steps do **not** owe the same sentence, because they leave the row
/// in different states, and the assertion now says which one this bed earns.
/// `r37_error_road_telling.rs` is where the three wirings are driven against it.
#[test]
fn r36_a_second_step_after_the_apply_is_told_the_same_way() {
    let bed = "r36_head_sealed";
    let fixture = pipeline(bed, "before\n");
    a_project_cut_inside_the_window(&fixture, true);
    snapshot(&fixture, bed, "before");
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");

    let dir = fixture.project.join(".gx").join("checkpoints");
    let was = std::fs::metadata(&dir)
        .expect("the checkpoint directory is there")
        .permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("seal the head");
    println!(
        "R36E_SEAL bed={bed} dir={:?} was={:o} now=40555",
        dir.display().to_string(),
        was.mode()
    );

    let before = fixture.target_contents();
    let r = run(fixture
        .gx()
        .args(["repair", "--json", "--yes"])
        .args(["--signing-key", &fixture.key_id]));
    let w = walk(&fixture, bed, "gx repair --yes --json", r, before);
    unseal(&dir);
    snapshot(&fixture, bed, "after");

    assert!(
        w.moved(),
        "the bed failed before the product did: the recovery did not reach the road that applies"
    );
    // 🔴 **R37 / `req/496` M-01** — the same world, and its **own** sentence. See this test's doc.
    assert!(
        w.stderr.contains(RECORDED_MARK),
        "a failure at `record_head` leaves the same world and owes the sentence for the state it \
         actually leaves: the delta landed, 43 §7-2's terminal record landed, and the head did \
         not: <<{}>>",
        w.stderr
    );
    assert!(
        !w.stderr.contains(UNRECORDED_MARK),
        "req/496 M-01: this bed's `Committed` record is on the disk, so the sentence for a row \
         whose commit was **not** recorded is false here. It is the sentence this suite asserted \
         until R37: <<{}>>",
        w.stderr
    );
    let j: Value = serde_json::from_str(w.stdout.trim()).expect("one JSON object on stdout");
    let failed = &j["engine_open_failed"];
    assert!(
        failed["recorded_before_failure"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty()),
        "the report must name the row whose delta was written, wherever in the tail the run \
         stopped — under `recorded_before_failure` where the record landed too: {j}"
    );
    assert!(
        failed["applied_before_failure"]
            .as_array()
            .is_some_and(|rows| rows.is_empty()),
        "req/496 M-01: the two lists are disjoint, and this row is past the `Committed` record: {j}"
    );
    assert_eq!(
        failed["finished_before_failure"], 0,
        "req/496 M-02: this bed's recovery closed one row and it is the row that raised, which is \
         not a row it finished; the `1` this field used to answer was the commit that closed \
         before the process started: {j}"
    );
}

// ---------------------------------------------------------------------------
// 3. `gx serve` — the fifth verb, undriven by three consecutive audits
// ---------------------------------------------------------------------------

/// 🔴 `req/476` §7-1 and §8-1 declare this gap in as many words: audit 34, R35 and audit 35 all
/// inherited `gx serve`'s `Err` road from a source reading, and `req/38` §271 ruling 5 asks the
/// repair lane to close it with its own instrument. `gx serve` is the road a project comes back on
/// after a power cut, so it is the likeliest place in the product to meet this failure.
///
/// The server never binds here: `recover` raises during start-up and the process exits. The arm
/// polls rather than blocking, so a repair that accidentally made the start-up *succeed* fails this
/// test instead of hanging the suite.
#[test]
fn r36_gx_serve_says_it_too() {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let bed = "r36_serve_sealed";
    let fixture = pipeline(bed, "before\n");
    a_project_cut_inside_the_window(&fixture, true);
    snapshot(&fixture, bed, "before");
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    let token_file = fixture.project.join("token");
    std::fs::write(&token_file, "r36-serve-token\n").expect("write the token file");
    let dir = seal(&fixture, bed);
    let before = fixture.target_contents();

    let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
        .env("HOME", &fixture.home)
        .env("USERPROFILE", &fixture.home)
        .arg("--project")
        .arg(&fixture.project)
        .arg("serve")
        .args(["--bind", "127.0.0.1:0"])
        .arg("--token-file")
        .arg(&token_file)
        .args(["--signing-key", &fixture.key_id])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("gx serve starts");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        match child.try_wait().expect("wait on gx serve") {
            Some(status) => break Some(status),
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                break None;
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    let w = Walk {
        bed: bed.to_string(),
        verb: "gx serve",
        code: status.and_then(|s| s.code()).unwrap_or(-1),
        world_before: before,
        world_after: fixture.target_contents(),
        stdout,
        stderr,
    };
    w.report();
    unseal(&dir);
    snapshot(&fixture, bed, "after");

    assert!(
        status.is_some(),
        "gx serve was still running after the recovery raised, so this arm measured nothing"
    );
    assert!(
        w.moved(),
        "the bed failed before the product did: the start-up did not reach 43 §7-3c's road"
    );
    assert!(
        w.stderr.contains(UNRECORDED_MARK),
        "req/476 §7-1: `gx serve` walked the road that writes, failed while recording, and said \
         nothing about the delta it had applied: <<{}>>",
        w.stderr
    );
    assert!(
        w.stderr.contains("gx serve:"),
        "the sentence must be in this verb's own name: <<{}>>",
        w.stderr
    );
}

// ---------------------------------------------------------------------------
// 4. The shape, not the row: a census of the `Err` arm
// ---------------------------------------------------------------------------

/// 🔴 The census `r35_shared_road_sentence.rs` has for the `Ok` road, for the `Err` road.
///
/// R35's module header states the design this repair inherits: *"silence is the thing that takes
/// work"*, and its census fails the build when a new `.recover(` site appears on neither footing.
/// That census asks one question — does this file name the announcer? — and a file that announces
/// on its `Ok` arm answers yes with its `Err` arm bare. **Which is exactly the state audit 35
/// found**, in the files R35 had just wired.
///
/// So repairing the three sites and stopping would leave the shape intact for the fourth: a write
/// verb added tomorrow that holds an `Engine` directly would be loud when the recovery succeeds and
/// silent when it writes and then fails. This arm is the census for that half — every site that
/// calls the **engine's** `recover` must reach the interrupted announcement in the same file.
#[test]
fn r36_every_engine_recover_site_announces_on_the_err_arm_too() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .to_path_buf();

    let mut engine_sites: Vec<(String, String)> = Vec::new();
    let mut files_naming_the_interrupted_announcer: Vec<String> = Vec::new();

    for crate_dir in std::fs::read_dir(&root).expect("crates/").flatten() {
        let src = crate_dir.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)
                .expect("a source directory")
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let mut names_it = false;
                for (n, line) in text.lines().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    if line.contains("announce_interrupted_recovery(") && !line.contains("pub fn ")
                    {
                        names_it = true;
                    }
                    // The engine's own road. `session.recover(..)` delegates to the wrapper, which
                    // is itself one of these sites and is checked as one; `layout.recover()` is a
                    // different function on a different type (it repairs `.gx/`'s directory shape).
                    if line.contains(".recover(")
                        && !line.contains("session.recover(")
                        && !line.contains("sess.recover(")
                        && !line.contains("layout.recover(")
                    {
                        engine_sites
                            .push((format!("{}:{}", path.display(), n + 1), line.trim().into()));
                    }
                }
                if names_it {
                    files_naming_the_interrupted_announcer.push(path.display().to_string());
                }
            }
        }
    }

    engine_sites.sort();
    files_naming_the_interrupted_announcer.sort();
    println!("R36E_DENOM engine_recover_sites={}", engine_sites.len());
    for (where_, what) in &engine_sites {
        println!("R36E_RECOVER_SITE {where_} :: {what}");
    }
    println!(
        "R36E_ERR_ANNOUNCERS count={} files={:?}",
        files_naming_the_interrupted_announcer.len(),
        files_naming_the_interrupted_announcer
    );

    let mut bare: Vec<String> = Vec::new();
    for (where_, what) in &engine_sites {
        let file = where_
            .rsplit_once(':')
            .map(|(f, _)| f.to_string())
            .unwrap_or_default();
        if !files_naming_the_interrupted_announcer.contains(&file) {
            bare.push(format!("{where_} :: {what}"));
        }
    }
    assert!(
        bare.is_empty(),
        "req/476 H-01: these sites reach `Engine::recover` and their `Err` arm names nothing, so a \
         recovery that writes a delta and then fails goes past them in silence — which is the \
         defect this lane repaired, re-appearing at a new site: {bare:?}"
    );
    assert!(
        engine_sites.len() >= 3,
        "instrument: the census found {} engine call sites and there are three shipped ones \
         (session.rs, repair.rs, serve.rs). A scan that finds fewer is scanning the wrong tree",
        engine_sites.len()
    );
}

// ---------------------------------------------------------------------------
// 5. The two negative controls audit 35 built, reproduced
// ---------------------------------------------------------------------------

/// 🔴 The control that says the probe can **see** a sentence: the same bed with no seal is R35's
/// `Ok` road, which has printed since `req/470` H-01 was repaired. Audit 35 measured 838 bytes of
/// stderr here (`A35E_WALK bed=a35_repair_unsealed_control ... stderr_bytes=838`).
#[test]
fn r36_control_the_unsealed_road_still_prints_r35s_sentence() {
    let fixture = pipeline("r36_repair_unsealed_control", "before\n");
    a_project_cut_inside_the_window(&fixture, true);
    snapshot(&fixture, "r36_repair_unsealed_control", "before");
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    let before = fixture.target_contents();
    let r = run(fixture
        .gx()
        .args(["repair", "--json", "--yes"])
        .args(["--signing-key", &fixture.key_id]));
    let w = walk(
        &fixture,
        "r36_repair_unsealed_control",
        "gx repair --yes --json",
        r,
        before,
    );
    snapshot(&fixture, "r36_repair_unsealed_control", "after");
    assert!(w.moved(), "the control must walk the same road");
    assert_eq!(w.code, 0, "the unsealed road succeeds: {}", w.stderr);
    assert!(
        w.stderr.contains(NOTE_MARK),
        "the probe cannot see a sentence at all if this arm is silent — every other assertion in \
         this file would then be measuring the probe rather than the product: <<{}>>",
        w.stderr
    );
}

/// 🔴 The control that says the **seal** does not move anything by itself: a healthy project with
/// the archive sealed writes nothing, so the `moved=true` above has nowhere else to come from.
#[test]
fn r36_control_a_healthy_project_with_the_seal_on_moves_nothing() {
    let fixture = pipeline("r36_healthy_sealed_control", "before\n");
    fixture.commit_one("one\n");
    snapshot(&fixture, "r36_healthy_sealed_control", "before");
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    let dir = seal(&fixture, "r36_healthy_sealed_control");
    let before = fixture.target_contents();
    let r = run(fixture
        .gx()
        .args(["repair", "--json", "--yes"])
        .args(["--signing-key", &fixture.key_id]));
    let w = walk(
        &fixture,
        "r36_healthy_sealed_control",
        "gx repair --yes --json",
        r,
        before,
    );
    unseal(&dir);
    snapshot(&fixture, "r36_healthy_sealed_control", "after");
    assert!(
        !w.moved(),
        "the seal alone moved the world, which would make every `moved=true` above unattributable"
    );
    assert!(
        !w.sentence_anywhere(),
        "a project with nothing to recover must not announce a recovery: <<{}>>",
        w.stderr
    );
}
