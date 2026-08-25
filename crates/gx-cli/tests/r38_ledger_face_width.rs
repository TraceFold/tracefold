// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R38 / `req/513` M-01** — the CLI half of the `ledger_agrees` family.
//!
//! # What audit 37 measured
//!
//! `req/502` closed `req/496` M-04 by putting the gate on `GET /ledger/proof`, `GET
//! /ledger/consistency` and `GET /ledger/checkpoint`, and named the family "four spellings of one
//! sentence". Its acceptance test is called `..._every_ledger_read_refuses_...`. Audit 37 cut the
//! last `Committed` frame out of a project's journal and asked the **CLI** the same three
//! questions: `gx log proof`, `gx log consistency` and `gx log checkpoint` all answered exit 0, the
//! proof was byte-identical to the healthy one, and `checkpoint` printed a signature — in the same
//! project, at the same instant that `gx repair --json` was answering `ledger_agrees_before:
//! false`. The family had been counted as four HTTP routes when it is seven mouths — those four
//! and these three. An eighth candidate (`gx receipt verify`'s local-ledger anchor) was proposed by
//! this suite and **retracted**; the note at the end of the test carries why.
//!
//! This suite is that measurement, moved inside the tree so it runs on every floor. It drives the
//! shipped binary through `support::Pipeline` (four processes: submit, plan, verify, commit), cuts
//! the journal with the same `frames`/`truncate_at` arithmetic `r37_ledger_gate_and_state_shape.rs`
//! uses, and then asks the CLI the questions.
//!
//! # The two negative controls, and why the suite is worthless without them
//!
//! A gate that refuses *everything* would pass an assertion that says "refuse after the cut", and
//! would be a worse product than the one this repairs. `req/501` §0 declared the controls for the
//! HTTP side and they are the same three here:
//!
//! 1. **The argument questions keep their answers on both sides of the cut.** An out-of-range
//!    `--leaf 99` and a well-formed `gx1:` id this ledger has never held are facts about the ledger
//!    file's own size, which a journal in any state does not change. They exit 6 before the cut and
//!    6 after it. This is what pins the gate's *position*: below the caller's argument and above
//!    the answer.
//! 2. **A project that was never cut still answers.** A second project, built the same way and left
//!    alone, answers all three verbs with exit 0 after the first one has been cut.
//! 3. **Healthy first.** Every verb is asked before the cut and asserted at exit 0, so "refused
//!    after" is a change this suite watched happen rather than a state it found.
//!
//! # The unknown id is built the way `req/516` says, not the way `req/496` did
//!
//! 🔴 **R38 / `req/38` §292** — `r37_ledger_gate_and_state_shape.rs` built "an id this ledger never
//! held" by flipping the last character between `'a'` and `'b'`. `Cid::from_text` refuses a final
//! character whose unused bits are set, and the base32 body's last character carries four unused
//! bits, so **`'a'` (value 0) and `'q'` (value 16) are the only legal spellings there**. The flip
//! produced a *malformed* id whenever the real one ended in `'a'`, and a malformed id is a
//! different refusal road (422 / exit 1) from an unresolvable one (404 / exit 6). Which road got
//! taken depended on the scratch directory's absolute path, because the id is content-addressed
//! over a locator that contains it — `req/516` reproduced green and red from one commit by changing
//! `CARGO_TARGET_DIR` alone.
//!
//! [`an_id_this_ledger_never_held`] reads the tail's value and swaps it for the *other legal* one,
//! so the negative control is well-formed by construction in every environment. The assertion that
//! it parses is kept beside it: a control whose construction silently stops preserving the grammar
//! of the thing it is a control for goes back to measuring the wrong refusal.

mod support;

use std::path::Path;

use support::{pipeline, run, Pipeline, Run};

/// The `gx_code` a refusal carried, read off 44 §1.3's problem object on stderr.
///
/// 🔴 **R39 / `req/533` L-02** — `assert_ne!(code, 0)` is "refused for some reason", and audit 38
/// counted that the discriminator `req/519` §2-3 calls load-bearing appeared in **no test in the
/// tree**. A suite that measures only the number cannot tell this refusal from a usage error, a
/// missing file or a panic mapped to the same exit. Empty for a run that refused without one, which
/// is itself a fact worth failing on rather than a `None` a caller can ignore.
fn gx_code(out: &Run) -> String {
    out.stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|v| v["gx_code"].as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The engine journal's frame boundaries — copied verbatim from
/// `crates/gx-api/tests/r37_ledger_gate_and_state_shape.rs` rather than reconstructed, for the
/// reason `req/496` §7-1 records: a cut computed from memory is a cut this suite invented.
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

fn truncate_at(path: &Path, at: u64) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
}

/// 🔴 **`req/38` §292** — a well-formed `gx1:` id this ledger has never held.
///
/// The last character of a `gx1:` body carries four unused bits which `Cid::from_text` requires to
/// be zero, so only two spellings are legal there. This reads which one the real id ended with and
/// returns the other, which keeps the control inside the grammar in every environment. The panic is
/// the assertion `req/38` §292 asks for: **the construction of a negative control must itself
/// assert that it preserved the grammar of what it is a control for.**
fn an_id_this_ledger_never_held(real: &str) -> String {
    let mut spelling = real.to_string();
    let last = spelling.pop().expect("a `gx1:` id is not empty");
    spelling.push(if last == 'a' { 'q' } else { 'a' });
    assert_ne!(
        spelling, real,
        "the swap produced the id it was meant to differ from"
    );
    gx_core::Cid::from_text(&spelling).unwrap_or_else(|e| {
        panic!(
            "the negative control stopped being well-formed: {spelling} does not parse ({e}). \
             `req/38` §292: an unparseable id measures the *malformed* refusal road, not the \
             *unresolved* one, and the two carry different exit codes"
        )
    });
    spelling
}

/// Every answer the CLI gives about this project's tree, asked in one place so that "healthy" and
/// "after the cut" are asked the same questions in the same order.
struct Answers {
    proof_by_id: i32,
    proof_by_index: i32,
    proof_unknown_index: i32,
    proof_unknown_id: i32,
    consistency: i32,
    checkpoint: i32,
    checkpoint_signed: bool,
    proof_bytes: usize,
    verify_local_anchor: i32,
    /// 🔴 **R39 / `req/533` L-02** — the word each of the three gated verbs answered with, so that
    /// "it refused" and "it refused *for this reason*" are different assertions.
    proof_by_id_code: String,
    proof_by_index_code: String,
    consistency_code: String,
    checkpoint_code: String,
}

fn ask_the_cli(p: &Pipeline, when: &str, id: &str) -> Answers {
    let unknown_id = an_id_this_ledger_never_held(id);

    let by_id = run(p.gx().args(["log", "proof", "--leaf", id]));
    let by_index = run(p.gx().args(["log", "proof", "--leaf", "0"]));
    let unknown_index = run(p.gx().args(["log", "proof", "--leaf", "99"]));
    let unknown = run(p.gx().args(["log", "proof", "--leaf", &unknown_id]));
    let consistency = run(p
        .gx()
        .args(["log", "consistency", "--from", "1", "--to", "1"]));
    let checkpoint = run(p.gx().args(["log", "checkpoint", "--key", &p.key_id]));
    // With neither `--offline` nor `--checkpoint`, `gx receipt verify` takes its anchor from this
    // project's own ledger (`main.rs`'s `local-ledger` arm). Measured on every row and **not**
    // asserted — see the retraction note at the end of the test.
    let receipt = p.project.join(".gx").join("receipts");
    let receipt_file = std::fs::read_dir(&receipt)
        .ok()
        .and_then(|dir| {
            dir.filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .find(|path| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains("commit"))
                })
        })
        .unwrap_or_else(|| receipt.join("absent.json"));
    let key_file = p
        .home
        .join(".gx")
        .join("keys")
        .join(format!("{}.key", p.key_id));
    let verified = run(p
        .gx()
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_file)
        .arg("--key")
        .arg(&key_file));

    let signed = checkpoint.code == 0 && checkpoint.stdout.contains("\"signature\"");

    println!(
        "R38_CLI when={when} proof_by_id={} proof_by_index={} proof_unknown_index={} \
         proof_unknown_id={} consistency={} checkpoint={} checkpoint_signed={signed} \
         proof_bytes={} verify_local_anchor={} proof_by_id_code={} proof_by_index_code={} \
         consistency_code={} checkpoint_code={}",
        by_id.code,
        by_index.code,
        unknown_index.code,
        unknown.code,
        consistency.code,
        checkpoint.code,
        by_id.stdout.len(),
        verified.code,
        gx_code(&by_id),
        gx_code(&by_index),
        gx_code(&consistency),
        gx_code(&checkpoint),
    );

    Answers {
        proof_by_id: by_id.code,
        proof_by_index: by_index.code,
        proof_unknown_index: unknown_index.code,
        proof_unknown_id: unknown.code,
        consistency: consistency.code,
        checkpoint: checkpoint.code,
        checkpoint_signed: signed,
        proof_bytes: by_id.stdout.len(),
        verify_local_anchor: verified.code,
        proof_by_id_code: gx_code(&by_id),
        proof_by_index_code: gx_code(&by_index),
        consistency_code: gx_code(&consistency),
        checkpoint_code: gx_code(&checkpoint),
    }
}

/// 🔴 **R39 / `req/540` R-4a** — the four gated answers, asserted as *one exit code and one word*.
///
/// `req/533` L-02: the finding assertions this suite shipped were `assert_ne!(code, 0)`, and
/// `LEDGER_DISAGREES` — the discriminator `req/519` §2-3 calls load-bearing, and the one word
/// `crate::Error::refusal` maps this condition to on both faces — appeared in no test in the tree.
/// "Non-zero" is satisfied by a usage error, a missing project and a panic alike.
fn assert_refused_as_disagreement(a: &Answers, label: &str) {
    for (verb, code, word) in [
        ("log proof --leaf <id>", a.proof_by_id, &a.proof_by_id_code),
        (
            "log proof --leaf 0",
            a.proof_by_index,
            &a.proof_by_index_code,
        ),
        ("log consistency", a.consistency, &a.consistency_code),
        ("log checkpoint", a.checkpoint, &a.checkpoint_code),
    ] {
        assert_eq!(
            (code, word.as_str()),
            (1, "LEDGER_DISAGREES"),
            "🔴 `req/540` AC-1 ({label}): `gx {verb}` must answer the disagreement with the exit \
             and the word the write road answers it with. Got exit {code} / `{word}`"
        );
    }
    assert!(
        !a.checkpoint_signed,
        "🔴 `req/540` AC-2 ({label}): `gx log checkpoint` put a **signature** on a tree this \
         project's own journal contradicts. Exit code alone does not measure this: a signature \
         outlives the mistake that produced it"
    );
}

/// 🔴 **`req/513` M-01** — every CLI read of the ledger refuses a project whose two files disagree.
///
/// The name is the HTTP acceptance test's name with the face changed, deliberately: `req/502`'s
/// `r37_m04_every_ledger_read_refuses_a_project_whose_two_files_disagree` said *every* and meant
/// *four HTTP routes*. This is the rest of the sentence.
#[test]
fn r38_m01_every_cli_ledger_read_refuses_a_project_whose_two_files_disagree() {
    let p = pipeline("r38_cli_face_width", "before\n");
    let id = p.commit_one("widen the family");

    // Control 3: healthy, first, so that the refusals below are a change this suite watched.
    let healthy = ask_the_cli(&p, "healthy", &id);
    assert_eq!(
        healthy.proof_by_id, 0,
        "the bed failed before the product did"
    );
    assert_eq!(
        healthy.proof_by_index, 0,
        "the bed failed before the product did"
    );
    assert_eq!(
        healthy.consistency, 0,
        "the bed failed before the product did"
    );
    assert_eq!(
        healthy.checkpoint, 0,
        "the bed failed before the product did"
    );
    assert!(
        healthy.checkpoint_signed,
        "the bed failed before the product did: a healthy project's checkpoint is signed"
    );
    assert_eq!(
        healthy.proof_unknown_index, 6,
        "the bed failed before the product did: an index past the tree is a not-found"
    );
    assert_eq!(
        healthy.proof_unknown_id, 6,
        "the bed failed before the product did: an id this ledger never held is a not-found. \
         `req/38` §292: if this is 1, the control is malformed rather than unresolvable"
    );

    // Control 2's other half: a second project, built the same way, never cut.
    let untouched = pipeline("r38_cli_never_cut", "before\n");
    let untouched_id = untouched.commit_one("the control project");

    // The cut: the last `Committed` frame out of the journal. The ledger file is not touched.
    let layout = gx_cli::layout::Layout::open(&p.project).expect("the project is open");
    let journal = layout.journal_path();
    let bytes = std::fs::read(&journal).expect("read the journal");
    let all = frames(&bytes);
    let (at, _len) = *all.last().expect("the journal holds at least one frame");
    let kinds_before = p.journal().len();
    truncate_at(&journal, at as u64);
    let kinds_after = p.journal().len();
    println!("R38_CLI cut_at={at} records_before={kinds_before} records_after={kinds_after}");
    assert!(
        kinds_after < kinds_before,
        "the bed failed before the product did: the cut removed no record"
    );

    // The product's own instrument, read **after** the questions below would have been asked, so
    // that this suite's claim about the state is the product's and not this suite's.
    let after = ask_the_cli(&p, "after_the_cut", &id);
    let repair = run(p.gx().args(["repair", "--json"]));
    let agrees = repair.json()["ledger_agrees_before"].clone();
    println!("R38_CLI repair ledger_agrees_before={agrees}");
    assert_eq!(
        agrees,
        serde_json::json!(false),
        "the bed failed before the product did: `gx repair` does not see the disagreement this \
         suite is about, so there is nothing here to refuse"
    );

    // Control 1: the argument questions are unchanged by the cut. This is what says the gate sits
    // below the caller's argument, and stops "refuse everything" from passing this suite.
    assert_eq!(
        after.proof_unknown_index, 6,
        "an index past the tree is a fact about the ledger file's size, which the journal's state \
         does not change"
    );
    assert_eq!(
        after.proof_unknown_id, 6,
        "an id this ledger never held keeps its not-found on both sides of the cut"
    );

    // Control 2: the untouched project still answers.
    let control = ask_the_cli(&untouched, "never_cut_control", &untouched_id);
    assert_eq!(
        (control.proof_by_id, control.consistency, control.checkpoint),
        (0, 0, 0),
        "🔴 a project that was never cut must still answer: a gate that silenced the whole verb \
         would be a worse product than the one this repairs"
    );

    // The finding itself. 🔴 **R39 / `req/533` L-02** — was four `assert_ne!(code, 0)` and one
    // `!signed`; it is now the exit **and** the word, which is what `req/519` §2-3 claimed this
    // suite was measuring.
    println!(
        "R38_CLI proof_bytes healthy={} after={} verify_local_anchor healthy={} after={}",
        healthy.proof_bytes,
        after.proof_bytes,
        healthy.verify_local_anchor,
        after.verify_local_anchor,
    );
    assert_refused_as_disagreement(&after, "the cut this suite makes");
    // 🔴 **R38 — the mouth this suite proposed and then retracted.** `verify_local_anchor` is
    // printed on every row above and is deliberately **not** asserted. R38 gated
    // `gx receipt verify`'s local-ledger anchor as an eighth member of the family, and
    // `serve_runtime_r6::dr4310_an_exported_head_refuses_a_project_that_went_backwards` refused the
    // change: DR-43-10's whole demonstration is a removed commit's receipt answering `verified`
    // against the auditor's checkpoint and `refuted` against the project's own ledger, and a gate
    // there deletes the second half. The rows stay in the log because the retraction is itself a
    // measurement. `main.rs`'s `local-ledger` arm carries the argument.
}

// ---------------------------------------------------------------------------
// 🔴 **R39 / `req/540`** — audit 38's beds, moved inside the tree
//
// `req/533` §2-3 and §2-4 built eight projects with shell scripts outside the repository and
// measured that the guard above is disarmed by the **absence of one file**. Scripts outside the
// tree do not run on a floor, so the finding could come back the moment the shape was repaired.
// These are those eight beds, driven through the shipped binary, with the expectations set to
// what `req/540` R-1a makes true rather than to what audit 38 measured.
//
// The naming keeps audit 38's labels (`b1`..`b4`, `c0`..`c3`) so that a reader holding the audit
// can put a row here beside a row there without a translation table.
// ---------------------------------------------------------------------------

/// The head document a project publishes on its commit road (`Engine::record_head`).
fn head_of(p: &Pipeline) -> std::path::PathBuf {
    gx_cli::layout::Layout::open(&p.project)
        .expect("the project is open")
        .head_path()
}

/// The journal file this project's engine appends to.
fn journal_of(p: &Pipeline) -> std::path::PathBuf {
    gx_cli::layout::Layout::open(&p.project)
        .expect("the project is open")
        .journal_path()
}

/// Drop the last `n` frames off the journal, or every frame when `n` is `None`.
///
/// Returns the record counts on both sides, because a cut that removed nothing is a bed failure and
/// not a finding — the same guard the suite above keeps beside its own cut.
fn cut_frames(p: &Pipeline, n: Option<usize>) -> (usize, usize) {
    let journal = journal_of(p);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let all = frames(&bytes);
    let prologue = if all.is_empty() { 0 } else { all[0].0 };
    let at = match n {
        None => prologue,
        Some(n) => {
            let idx = all
                .len()
                .checked_sub(n)
                .expect("the journal holds that many frames");
            if idx == 0 {
                prologue
            } else {
                all[idx].0
            }
        }
    };
    let before = p.journal().len();
    truncate_at(&journal, at as u64);
    let after = p.journal().len();
    assert!(
        after < before,
        "the bed failed before the product did: the cut removed no record ({before} -> {after})"
    );
    (before, after)
}

/// `gx repair --json`, which is the product's own instrument for the state these beds build.
fn repair_report(p: &Pipeline) -> serde_json::Value {
    run(p.gx().args(["repair", "--json"])).json()
}

/// 🔴 **R39 / `req/540` KA-2** — which `JournalDeparture`, if any, a **new process** sees over a
/// journal that was cut after it was written.
///
/// `req/540` §1-3 read the source and predicted `None`: `Shortened` and `PrefixRewritten` compare a
/// file against bytes *this process has already read*, and a process that opens a cut file for the
/// first time has read nothing to compare it against. The prediction is not the measurement.
/// `remedy` is keyed on exactly that value (`repair.rs`'s `base_remedy`), so the sentence it prints
/// names the branch the product took, and this prints it on every bed rather than asserting a guess.
fn departure_of(report: &serde_json::Value) -> String {
    let remedy = report["remedy"].as_str().unwrap_or("<none>");
    let head: String = remedy.chars().take(72).collect();
    format!(
        "journal_intact={} chain_break={} remedy_head={head:?}",
        report["journal_intact"], report["journal_chain_break_at"]
    )
}

/// The three gated verbs plus the write road, asked of one project in one place.
fn ask_both_roads(p: &Pipeline, label: &str, id: &str) -> (Answers, i32, String) {
    let reads = ask_the_cli(p, label, id);
    let submitted = p.submit(&format!("a second change for {label}"));
    let code = gx_code(&submitted);
    println!(
        "R39_BED {label} submit={} submit_code={code}",
        submitted.code
    );
    (reads, submitted.code, code)
}

/// 🔴 **`req/540` AC-3 + `req/533` §2-3 b2** — removing the head alone changes nothing.
///
/// This is the negative control that stops R-1a from being "refuse everything and call it safe".
/// `.gx/checkpoints/head.json` is not a record of the tree; it is a record that this project once
/// published one. A project whose two files still agree answers, head or no head — and audit 38
/// measured exactly that (`b2`: exit 0, `ledger_agrees_before: true`), which is what made `b4`'s
/// opening the *combination* of a cut and an absent head rather than a second refusal road.
#[test]
fn r39_b2_removing_the_head_alone_leaves_every_answer_where_it_was() {
    let p = pipeline("r39_b2_head_absent_no_cut", "before\n");
    let id = p.commit_one("b2");
    let healthy = ask_the_cli(&p, "b2_healthy", &id);
    assert_eq!(
        healthy.proof_by_id, 0,
        "the bed failed before the product did"
    );
    assert!(
        healthy.checkpoint_signed,
        "the bed failed before the product did"
    );

    let head = head_of(&p);
    assert!(
        head.is_file(),
        "the bed failed before the product did: a committed project has published a head, which \
         is the fact R38's second escape read"
    );
    std::fs::remove_file(&head).expect("remove the head");

    let report = repair_report(&p);
    println!(
        "R39_BED b2 {} agrees={}",
        departure_of(&report),
        report["ledger_agrees_before"]
    );
    assert_eq!(
        report["ledger_agrees_before"],
        serde_json::json!(true),
        "the bed failed before the product did: deleting the head is not supposed to make the two \
         files disagree, and if it does then this bed is measuring a different thing"
    );

    let after = ask_the_cli(&p, "b2_head_absent", &id);
    assert_eq!(
        (
            after.proof_by_id,
            after.proof_by_index,
            after.consistency,
            after.checkpoint
        ),
        (0, 0, 0, 0),
        "🔴 `req/540` AC-3: a project whose two files agree answers, with or without a head. A \
         build that refuses here has replaced a wrong answer with no answer"
    );
    assert!(
        after.checkpoint_signed,
        "🔴 `req/540` AC-3: and its checkpoint is still signed"
    );
}

/// 🔴 **`req/540` AC-1 + `req/533` §2-3** — the head file does not decide whether the two files
/// disagree.
///
/// Audit 38's `b1`, `b3` and `b4`, healthy first, each asked the four gated questions and the two
/// argument questions:
///
/// | bed | head | cut | audit 38 measured | this asserts |
/// |---|---|---|---|---|
/// | b1 | present | every frame | refused | refused, with the word |
/// | b3 | present, zero bytes | last frame | refused | refused, with the word |
/// | b4 | **absent** | last frame | **exit 0, signed checkpoint** | refused, with the word |
///
/// b4 is the row this lane exists for. b1 and b3 stay because a repair that only moved b4 would
/// leave the other two passing for the old reason, and the three together say the answer no longer
/// depends on the file.
#[test]
fn r39_b1_b3_b4_the_head_file_does_not_decide_the_refusal() {
    for (label, empty_head, remove_head, cut) in [
        ("b1_head_present_full_truncate", false, false, None),
        ("b3_head_empty_partial_cut", true, false, Some(1)),
        ("b4_head_absent_partial_cut", false, true, Some(1)),
    ] {
        let p = pipeline(&format!("r39_{label}"), "before\n");
        let id = p.commit_one(label);
        let healthy = ask_the_cli(&p, &format!("{label}_healthy"), &id);
        assert_eq!(
            healthy.proof_by_id, 0,
            "{label}: the bed failed before the product did"
        );
        assert!(
            healthy.checkpoint_signed,
            "{label}: the bed failed before the product did"
        );

        let head = head_of(&p);
        if empty_head {
            std::fs::write(&head, b"").expect("empty the head");
            assert_eq!(
                std::fs::metadata(&head).expect("stat the head").len(),
                0,
                "{label}: the bed failed before the product did: `is_file()` is an existence check \
                 and this bed is about that"
            );
        }
        if remove_head {
            std::fs::remove_file(&head).expect("remove the head");
        }
        let (before, after_records) = cut_frames(&p, cut);
        let report = repair_report(&p);
        println!(
            "R39_BED {label} head_present={} records={before}->{after_records} {} agrees={}",
            head.is_file(),
            departure_of(&report),
            report["ledger_agrees_before"],
        );
        assert_eq!(
            report["ledger_agrees_before"],
            serde_json::json!(false),
            "{label}: the bed failed before the product did: `gx repair` does not see a \
             disagreement here, so there is nothing for the read road to refuse"
        );

        let answers = ask_the_cli(&p, label, &id);
        assert_refused_as_disagreement(&answers, label);

        // The argument questions, on this bed too: `req/501` §0's negative control is what says the
        // gate sits below the caller's argument, and it has to hold on every bed rather than only
        // on the one the suite above cuts.
        assert_eq!(
            (answers.proof_unknown_index, answers.proof_unknown_id),
            (6, 6),
            "🔴 `req/540` AC-4 ({label}): an index past the tree and an id this ledger never held \
             are facts about the ledger file's own size. A gate that swallowed them would be \
             refusing everything and calling it safe"
        );
    }
}

/// 🔴 **`req/540` AC-1 + `req/533` §2-4** — the read road and the write road answer one project
/// with one word.
///
/// This is the sentence `req/540` §0 opens with, driven: audit 38 asked `gx submit` and `gx log
/// checkpoint` of the same project in the same second and got `LEDGER_DISAGREES` from one and a
/// **signature** from the other. `c0` is the healthy control (both roads answer), `c1` is the state
/// R38 already refused, and `c2`/`c3` are the same construction twice — audit 38 took the opening
/// four times and called it not a flake, so the repair is asserted twice for the same reason.
#[test]
fn r39_c_the_read_road_and_the_write_road_answer_one_project_with_one_word() {
    // c0: healthy. Both roads answer, and the checkpoint is signed.
    let c0 = pipeline("r39_c0_healthy", "before\n");
    let c0_id = c0.commit_one("c0");
    let (reads, submit_code, submit_word) = ask_both_roads(&c0, "c0_healthy", &c0_id);
    assert_eq!(
        (
            reads.proof_by_id,
            reads.consistency,
            reads.checkpoint,
            submit_code
        ),
        (0, 0, 0, 0),
        "the bed failed before the product did: a healthy project answers on both roads"
    );
    assert!(
        reads.checkpoint_signed,
        "the bed failed before the product did"
    );
    assert_eq!(submit_word, "", "a run that exited 0 carries no gx_code");

    for (label, remove_head) in [
        ("c1_head_present_cut", false),
        ("c2_head_absent_cut", true),
        ("c3_head_absent_cut_repeat", true),
    ] {
        let p = pipeline(&format!("r39_{label}"), "before\n");
        let id = p.commit_one(label);
        let healthy = ask_the_cli(&p, &format!("{label}_healthy"), &id);
        assert_eq!(
            healthy.proof_by_id, 0,
            "{label}: the bed failed before the product did"
        );
        if remove_head {
            std::fs::remove_file(head_of(&p)).expect("remove the head");
        }
        cut_frames(&p, Some(1));
        let report = repair_report(&p);
        println!(
            "R39_BED {label} {} agrees={}",
            departure_of(&report),
            report["ledger_agrees_before"]
        );

        let (reads, submit_code, submit_word) = ask_both_roads(&p, label, &id);
        assert_refused_as_disagreement(&reads, label);
        assert_eq!(
            (submit_code, submit_word.as_str()),
            (1, "LEDGER_DISAGREES"),
            "🔴 `req/540` AC-1 ({label}): the write road's answer is the negative control for the \
             read road's. If these two ever differ again, the product holds two answers about one \
             state, which is the whole of `req/533` M-01"
        );
    }
}

/// 🔴 **`req/540` AC-5 / R-1b** — a project with no journal still answers, and is refused the
/// moment one appears beside it that disagrees.
///
/// The first escape in `refuse_if_the_two_files_disagree` is kept on purpose and it is the one
/// asymmetry between the read road and the write road: a third party holding a ledger file and no
/// project has no second file making a competing claim, and `Session::settle` has no counterpart
/// because an engine that will not open never reaches it. The control is the second half: put a
/// journal beside the same ledger and the same question is refused, so the escape is "there is no
/// second file" and not "this shape is exempt".
#[test]
fn r39_a_ledger_with_no_journal_answers_and_a_journal_that_disagrees_ends_it() {
    let (dir, layout) = support::project("r39_escape_one");
    let key = support::keypair(39);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 39, 4);
    let journal = layout.journal_path();
    assert!(
        !journal.is_file(),
        "the bed failed before the product did: `seed_ledger` builds the third-party shape, which \
         is a ledger file and no project (`req/540` R-1c plan A)"
    );

    let answered = run(support::gx().arg("--project").arg(&dir).args([
        "log",
        "proof",
        "--leaf",
        &index.to_string(),
    ]));
    println!("R39_BED escape_one no_journal proof={}", answered.code);
    assert_eq!(
        answered.code, 0,
        "🔴 `req/540` AC-5: a ledger file with no project beside it is answered as it always was. \
         This gate's reach is exactly \"this project has a journal\": {}",
        answered.stderr
    );

    // The control: a journal that witnesses nothing, beside a ledger holding five leaves.
    gx_engine::EngineJournal::open(&journal).expect("create an empty journal");
    assert!(journal.is_file(), "the control put a journal there");
    let refused = run(support::gx().arg("--project").arg(&dir).args([
        "log",
        "proof",
        "--leaf",
        &index.to_string(),
    ]));
    println!(
        "R39_BED escape_one with_journal proof={} code={}",
        refused.code,
        gx_code(&refused)
    );
    assert_eq!(
        (refused.code, gx_code(&refused).as_str()),
        (1, "LEDGER_DISAGREES"),
        "🔴 `req/540` AC-5 control: the escape is \"there is no second file\", not \"a ledger \
         nobody committed through is exempt\". With a second file present and disagreeing, the \
         same question is refused"
    );
}

/// 🔴 **`req/540` AC-4 / R-7c / `req/533` L-03(b)** — the negative control's own precondition,
/// driven rather than declared.
///
/// `req/501` §0 says an out-of-range `--leaf` and an id this ledger never held keep their exit 6 on
/// both sides of a cut. That is true **while `Layout::open` succeeds**. Audit 38 made `.gx/VERSION`
/// unreadable and watched both questions move from 6 to 1, because the project never opens far
/// enough for the ledger's size to be a fact anybody reads. R39 does not repair that classification
/// (`req/540` §6 sends L-03(a) to its own box); what it refuses to do is ship a negative control
/// whose precondition is nowhere written down and nowhere driven.
#[test]
fn r39_the_argument_questions_keep_their_exit_only_while_the_project_opens() {
    let p = pipeline("r39_l03b_version_unreadable", "before\n");
    let id = p.commit_one("l03b");
    let healthy = ask_the_cli(&p, "l03b_healthy", &id);
    assert_eq!(
        (healthy.proof_unknown_index, healthy.proof_unknown_id),
        (6, 6),
        "the bed failed before the product did: this is `req/501` §0's control in its ordinary state"
    );

    let version = gx_cli::layout::Layout::open(&p.project)
        .expect("the project is open")
        .root()
        .join("VERSION");
    std::fs::remove_file(&version).expect("remove .gx/VERSION");
    std::fs::create_dir(&version).expect("put a directory where the file was");

    let unknown_index = run(p.gx().args(["log", "proof", "--leaf", "99"]));
    let unknown = an_id_this_ledger_never_held(&id);
    let unknown_id = run(p.gx().args(["log", "proof", "--leaf", &unknown]));
    println!(
        "R39_BED l03b unknown_index={} code={} unknown_id={} code={}",
        unknown_index.code,
        gx_code(&unknown_index),
        unknown_id.code,
        gx_code(&unknown_id),
    );
    assert_ne!(
        (unknown_index.code, unknown_id.code),
        (6, 6),
        "🔴 `req/540` R-7c: if these are 6 again, the but-for clause this lane wrote into \
         `ledger.rs` and `req/501` §0 is false and both should be deleted rather than left \
         describing a shape that no longer exists"
    );
}

/// 🔴 **`req/540` R-6d / KA-4 — T-6's premise, driven, and **withdrawn**.**
///
/// `req/533` §6 L-05 says in prose that `gx cancel` answers `NOT_FOUND` about a candidate a cut
/// removed while `gx submit` on the same project in the same second answers `LEDGER_DISAGREES`, and
/// the table on that same page prints `arm5 post_cut_submit rc=0`. Those cannot both be true of one
/// project. `req/540` R-6d made reproducing the state the precondition of repairing it, and this is
/// the reproduction attempt. It fails, in a way that is worth keeping.
///
/// Two branches exhaust the shape, and they are asked of one construction each:
///
/// **A — the two files disagree.** Truncate at the `Committed` frame, which is a suffix cut and so
/// takes the candidate's records with it. `gx cancel` answers **exit 1 `LEDGER_DISAGREES`**, not
/// exit 6 `NOT_FOUND`: the lock road's disagreement gate sits above the id lookup, so there is no
/// second answer for the write road to contradict. The state L-05 describes is not here.
///
/// **B — the two files still agree.** Cut only the candidate's own frames. `gx cancel` answers exit
/// 6 `NOT_FOUND` and `gx submit` answers 0 — one project, one answer, and the answer is true of
/// every file in front of it. The record loss is real and invisible, which is a finding, but it is
/// **not** the finding L-05 wrote: `ledger_agrees` is the predicate R-6a would have keyed the note
/// on, and on this branch it is `true`. The note R-6a specifies could never fire on the only shape
/// that shows the symptom.
///
/// So T-6 is withdrawn rather than shipped, and this test is why — a probe, so that a later author
/// who reads L-05 and reaches for the repair finds the measurement instead of the prose. If either
/// branch ever moves, this goes red and the question is open again.
#[test]
fn r39_l05_the_post_crash_cancel_finding_does_not_reproduce_on_either_branch() {
    // ---- Branch A: the cut takes the commit with it, so the two files disagree. ----
    let a = pipeline("r39_l05_branch_a", "before\n");
    let a_committed = a.commit_one("l05 first");
    let a_candidate = a.planned_one("l05 second");
    let records = a.journal();
    let committed_at = records
        .iter()
        .position(|r| matches!(r, gx_engine::EngineJournalRecord::Committed { .. }))
        .expect("the bed failed before the product did: nothing was committed");
    let total = records.len();
    assert!(
        total > committed_at + 1,
        "the bed failed before the product did: the candidate's records have to sit after the \
         commit for one suffix cut to remove both ({total} records, commit at {committed_at})"
    );
    cut_frames(&a, Some(total - committed_at));
    let a_report = repair_report(&a);
    let a_cancel = run(a.gx().args(["cancel", &a_candidate]));
    let a_submit = a.submit("l05 third");
    let a_undo = run(a.gx().args(["undo", &a_committed]));
    println!(
        "R39_L05 branch=A agrees={} cancel={} cancel_code={} submit={} submit_code={} undo={} \
         undo_code={}",
        a_report["ledger_agrees_before"],
        a_cancel.code,
        gx_code(&a_cancel),
        a_submit.code,
        gx_code(&a_submit),
        a_undo.code,
        gx_code(&a_undo),
    );
    assert_eq!(
        a_report["ledger_agrees_before"],
        serde_json::json!(false),
        "branch A: the bed failed before the product did"
    );
    assert_eq!(
        (a_cancel.code, gx_code(&a_cancel).as_str()),
        (1, "LEDGER_DISAGREES"),
        "🔴 `req/540` R-6d branch A: if `cancel` answers `NOT_FOUND` here again, `req/533` L-05's \
         state exists after all and T-6 goes back on the table"
    );
    assert_eq!(
        (a_submit.code, gx_code(&a_submit).as_str()),
        (1, "LEDGER_DISAGREES"),
        "branch A: and the write road says the same word, which is what makes this one answer"
    );

    // ---- Branch B: the cut takes only the candidate, so the two files still agree. ----
    let b = pipeline("r39_l05_branch_b", "before\n");
    b.commit_one("l05 first");
    let b_candidate = b.planned_one("l05 second");
    let b_records = b.journal();
    let b_committed_at = b_records
        .iter()
        .position(|r| matches!(r, gx_engine::EngineJournalRecord::Committed { .. }))
        .expect("the bed failed before the product did: nothing was committed");
    // Everything after the commit, and nothing before it.
    cut_frames(&b, Some(b_records.len() - b_committed_at - 1));
    let b_report = repair_report(&b);
    let b_cancel = run(b.gx().args(["cancel", &b_candidate]));
    let b_submit = b.submit("l05 third");
    println!(
        "R39_L05 branch=B agrees={} cancel={} cancel_code={} cancel_detail={:?} submit={} \
         submit_code={}",
        b_report["ledger_agrees_before"],
        b_cancel.code,
        gx_code(&b_cancel),
        b_cancel.stderr.trim(),
        b_submit.code,
        gx_code(&b_submit),
    );
    assert_eq!(
        b_report["ledger_agrees_before"],
        serde_json::json!(true),
        "🔴 `req/540` R-6d branch B: this is the half that carries the symptom, and the whole point \
         of recording it is that `ledger_agrees` — the predicate R-6a would have keyed a note on — \
         is `true` here. If this ever becomes `false`, R-6a becomes implementable and T-6 should be \
         re-opened"
    );
    assert_eq!(
        (b_cancel.code, gx_code(&b_cancel).as_str()),
        (6, "NOT_FOUND"),
        "branch B: the candidate's record is gone and the product says so"
    );
    assert_eq!(
        b_submit.code, 0,
        "🔴 branch B: the write road answers, so there is no second answer here either. `req/533` \
         §6's table row `post_cut_submit rc=0` is the honest one and its prose is the erratum: {}",
        b_submit.stderr
    );
}
