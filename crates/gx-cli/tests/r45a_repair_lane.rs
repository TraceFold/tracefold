// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R45-a / `req/621` M-1, L-2, L-3, ruling `req/38` §394** — the three code-bearing boxes of
//! the repair.rs-cluster repair lane, each with its own red-first bed.
//!
//! # M-1 — `repair.rs:2348`'s `ledger_present`, the last of R41/R43's fold on the
//! `report_without_engine` road
//!
//! Bed-D (`r41_presence_unification.rs::s6_bed_d_a_lost_journal_does_not_unmeasure_its_siblings`)
//! already establishes that deleting `.gx/ledger/journal` forces `report_without_engine`'s road
//! (the journal is `Absent` and `logged`, so `Layout::open` refuses `journal_absent` and `gx repair`
//! catches that and reports without an engine). This file reuses the same road and varies each of
//! the three declared siblings — `journal.ledger`, `journal.verdicts`, `.gx/checkpoints/head.json`
//! — independently across three shapes in one test body each: healthy (a regular file), a symbolic
//! link that resolves to nothing, and genuinely deleted. `ledger_present` was the one sibling still
//! answering `false` for the middle shape before this lane; `verdict_chain_present` and
//! `head_recorded`/`head_authenticity` are carried alongside as the positive/regression controls
//! `req/635` §1 M-1-2 asks for, in the same test body, on the same road.
//!
//! # L-2 — the verdict chain's read road and write road disagree about a loop
//!
//! `req/621` §6-2 C4: a `.gx/ledger/journal.verdicts` that is a symbolic-link **loop** (not merely
//! dangling — `ELOOP`, not `ENOENT`) answered `rc0` in silence on the read road and `INTERNAL` on
//! the write road. Traced to `gx_log::store::VerdictCheckpointStore::open_read_only_or_absent`
//! (`crates/gx-log/src/store.rs`), which asked `Path::exists()` — a call that follows the link and
//! folds every `stat` failure, `ELOOP` included, into `false` — and answered "no chain yet" about a
//! path that in fact could not be resolved, in silence. The writer's door
//! (`VerdictCheckpointStore::open`) makes no such check and gets the loop back as a real `Io` error
//! from `OpenOptions::open`, which the caller (`gx_engine::pipeline::Engine::open`) turns into
//! `Error::Ledger` on both doors identically — so after the fix, the read road hits the loop for
//! real too, on the same `open_read_only` the writer's door already wears the words of.
//!
//! # L-3 — `CONFIG_ABSENT`/`DECLARATION_ABSENT`'s titles, widened
//!
//! `req/621` §6-3: a dangling symbolic link at `.gx/config.toml` or `.gx/VERSION` reaches these two
//! refusals with a title that reads "is not there" — false of a path that holds a link, by
//! `attach.rs::present`'s own rule. The fix widens the title text only (no detection-logic change,
//! no new `gx_code`); this bed asserts the title no longer states pure absence for the link shape,
//! while the genuinely-absent shape keeps whatever sentence it already had.

use std::path::{Path, PathBuf};

#[path = "support/mod.rs"]
mod support;

use support::run;

fn report(out: &support::Run) -> serde_json::Value {
    serde_json::from_str(&out.stdout).expect("the repair report is JSON")
}

/// A refusal's problem object — printed to **stderr** (44 §1.3), unlike `gx repair --json`'s
/// report, which is always stdout.
fn refusal(out: &support::Run) -> serde_json::Value {
    serde_json::from_str(&out.stderr).expect("the refusal is a JSON problem object on stderr")
}

/// Delete the journal (bed-D's construction) so `gx repair --json` takes the
/// `report_without_engine` road, and hand back the ledger/verdicts/head.json paths beside it.
fn bed_d_project(name: &str) -> (support::Pipeline, PathBuf, PathBuf, PathBuf) {
    let p = support::pipeline(name, "before\n");
    p.commit_one("first");
    let dot_gx = p.project.join(".gx");
    let journal = dot_gx.join("ledger").join("journal");
    let ledger = dot_gx.join("ledger").join("journal.ledger");
    let verdicts = dot_gx.join("ledger").join("journal.verdicts");
    let head = dot_gx.join("checkpoints").join("head.json");
    std::fs::remove_file(&journal).expect("remove the journal (bed-D)");
    (p, ledger, verdicts, head)
}

#[cfg(unix)]
fn dangling_symlink(at: &Path) {
    if at.exists() || at.symlink_metadata().is_ok() {
        std::fs::remove_file(at).expect("remove the file before linking over it");
    }
    std::os::unix::fs::symlink(at.with_file_name("no-such-target"), at)
        .expect("a symbolic link that resolves to nothing");
}

// ---------------------------------------------------------------------------------------------
// M-1 — `ledger_present` (repair.rs:2348), on the `report_without_engine` road.
// ---------------------------------------------------------------------------------------------

/// The box's primary target: `journal.ledger` a dangling link must not answer `ledger_present:
/// false` — that is exactly the `.gx/ledger/journal.ledger` this `ledger_present` key measures.
#[cfg(unix)]
#[test]
fn m1_ledger_present_discriminates_healthy_link_and_absent() {
    let (p, ledger, _verdicts, _head) = bed_d_project("r45a_m1_ledger");

    let healthy = run(p.gx().args(["repair", "--json"]));
    let before = report(&healthy);
    println!(
        "R45A_M1 healthy rc={} ledger_present={}",
        healthy.code, before["ledger_present"]
    );
    assert_eq!(
        before["ledger_present"], true,
        "bed-D: the ledger file is still a real file even with the journal gone"
    );

    dangling_symlink(&ledger);
    let linked = run(p.gx().args(["repair", "--json"]));
    let after = report(&linked);
    println!(
        "R45A_M1 link rc={} ledger_present={} against={:?}",
        linked.code, after["ledger_present"], after["against"]
    );
    assert_eq!(
        linked.code, healthy.code,
        "the exit this bed answers does not move (req/38 §148)"
    );
    assert_eq!(
        after["ledger_present"], true,
        "req/621 M-1: a declared path holding a symbolic link is something that is there \
         (attach.rs::present), whatever it points at — `false` here is the fold this lane closes"
    );

    std::fs::remove_file(&ledger).expect("genuinely delete the ledger");
    let gone = run(p.gx().args(["repair", "--json"]));
    let after_gone = report(&gone);
    println!(
        "R45A_M1 deleted rc={} ledger_present={}",
        gone.code, after_gone["ledger_present"]
    );
    assert_eq!(
        after_gone["ledger_present"], false,
        "a genuinely absent ledger is still, and only, `false`"
    );
}

/// Regression control — `verdict_chain_present` (R43 S-7's own fix) must still discriminate the
/// same three shapes on the same road, unmoved by this lane's edit to its sibling key.
#[cfg(unix)]
#[test]
fn m1_verdict_chain_present_control_unmoved() {
    let (p, _ledger, verdicts, _head) = bed_d_project("r45a_m1_verdicts");

    let healthy = report(&run(p.gx().args(["repair", "--json"])));
    assert_eq!(healthy["verdict_chain_present"], true);

    dangling_symlink(&verdicts);
    let linked = report(&run(p.gx().args(["repair", "--json"])));
    println!(
        "R45A_M1_CTRL link verdict_chain_present={}",
        linked["verdict_chain_present"]
    );
    assert_ne!(
        linked["verdict_chain_present"], false,
        "R43 S-7's fix: unchanged by this lane"
    );

    std::fs::remove_file(&verdicts).expect("genuinely delete the chain file");
    let gone = report(&run(p.gx().args(["repair", "--json"])));
    assert_eq!(gone["verdict_chain_present"], false);
}

/// Regression control — `head_recorded`/`head_authenticity` (R41 S-8's own fix) on the same road.
#[cfg(unix)]
#[test]
fn m1_head_recorded_control_unmoved() {
    let (p, _ledger, _verdicts, head) = bed_d_project("r45a_m1_head");

    let healthy = report(&run(p.gx().args(["repair", "--json"])));
    assert_eq!(healthy["head_recorded"], true);

    dangling_symlink(&head);
    let linked = report(&run(p.gx().args(["repair", "--json"])));
    println!(
        "R45A_M1_CTRL link head_recorded={} head_authenticity={}",
        linked["head_recorded"], linked["head_authenticity"]
    );
    // R41 S-8 already answers `null`/absence-safe here; this lane does not touch it. Recorded here
    // as a fact, not re-litigated as a finding — `req/635` scopes head.json's own word to a future
    // band if it is ever revisited.
    assert_ne!(linked["head_recorded"], true);

    std::fs::remove_file(&head).expect("genuinely delete head.json");
    let gone = report(&run(p.gx().args(["repair", "--json"])));
    assert_eq!(gone["head_recorded"], false);
}

// ---------------------------------------------------------------------------------------------
// L-2 — `journal.verdicts` ELOOP: read road vs. write road (`req/621` §6-2 C4).
// ---------------------------------------------------------------------------------------------

/// A **loop**, not a dangling link — `A -> A` — so `Path::exists()`'s `stat` fails `ELOOP` and not
/// `NotFound`, which is the one distinction `req/621` C4 turns on.
#[cfg(unix)]
fn self_loop(at: &Path) {
    if at.symlink_metadata().is_ok() {
        std::fs::remove_file(at).expect("remove the file before looping it");
    }
    let name = at.file_name().expect("a file name").to_owned();
    std::os::unix::fs::symlink(&name, at).expect("a self-referential symbolic link (ELOOP)");
    // Confirm the loop actually classifies as ELOOP-shaped (a stat failure other than NotFound) on
    // this filesystem before the bed is trusted: `symlink_metadata` (lstat) must still succeed
    // (the loop is not the final component from lstat's point of view) and a following `stat` must
    // fail with something other than `NotFound`.
    assert!(
        std::fs::symlink_metadata(at).is_ok(),
        "lstat itself must see the link (it does not follow the final component)"
    );
    let follow = std::fs::metadata(at);
    assert!(
        follow.is_err(),
        "a self-referential link must not resolve to a real file"
    );
    assert_ne!(
        follow.unwrap_err().kind(),
        std::io::ErrorKind::NotFound,
        "bed construction check: this must be a resolution failure (ELOOP-shaped), not a plain \
         absence, or C4 is not reproduced"
    );
}

#[cfg(unix)]
#[test]
fn l2_verdicts_eloop_read_road_and_write_road_agree() {
    let p = support::pipeline("r45a_l2_c4", "before\n");
    p.commit_one("first");
    let verdicts = p
        .project
        .join(".gx")
        .join("ledger")
        .join("journal.verdicts");
    self_loop(&verdicts);

    let read = run(p.gx().args(["repair", "--json"]));
    println!(
        "R45A_L2 READ(repair) rc={} stdout={}",
        read.code, read.stdout
    );
    let write = p.submit("second");
    println!(
        "R45A_L2 WRITE(submit) rc={} stdout={} stderr={}",
        write.code, write.stdout, write.stderr
    );

    let read_refused = read.code != 0;
    let write_refused = write.code != 0;
    println!("R45A_L2 UNIFIED read_refused={read_refused} write_refused={write_refused}");
    assert_eq!(
        read_refused, write_refused,
        "req/621 L-2: the two roads must agree about the same ELOOP condition on the same file \
         (read rc={}, write rc={})",
        read.code, write.code
    );
    assert!(
        write_refused,
        "the write road must still refuse a chain it cannot open (this is not this lane's edit)"
    );
}

// ---------------------------------------------------------------------------------------------
// L-3 — `CONFIG_ABSENT` / `DECLARATION_ABSENT` titles (`req/621` §6-3 C7/C8/C9).
// ---------------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn l3_config_absent_title_does_not_imply_pure_absence_for_a_link() {
    let p = support::pipeline("r45a_l3_config", "before\n");
    p.commit_one("first");
    let config = p.project.join(".gx").join("config.toml");

    dangling_symlink(&config);
    let out = p.submit("second");
    let body = refusal(&out);
    let title = body["title"].as_str().unwrap_or_default();
    println!("R45A_L3 CONFIG_ABSENT code={} title={title:?}", out.code);
    assert_eq!(body["gx_code"], "CONFIG_ABSENT");
    assert!(
        title.contains("does not resolve"),
        "req/621 L-3: the title must name the link shape rather than assert pure absence: {title:?}"
    );
}

#[cfg(unix)]
#[test]
fn l3_declaration_absent_title_does_not_imply_pure_absence_for_a_link() {
    let p = support::pipeline("r45a_l3_version", "before\n");
    p.commit_one("first");
    let version = p.project.join(".gx").join("VERSION");

    dangling_symlink(&version);
    let out = p.submit("second");
    let body = refusal(&out);
    let title = body["title"].as_str().unwrap_or_default();
    println!(
        "R45A_L3 DECLARATION_ABSENT code={} title={title:?}",
        out.code
    );
    assert_eq!(body["gx_code"], "DECLARATION_ABSENT");
    assert!(
        title.contains("does not resolve"),
        "req/621 L-3: the title must name the link shape rather than assert pure absence: {title:?}"
    );
}

/// Negative control — a genuinely absent `.gx/config.toml` keeps a title that is still true of it
/// (the disjunction covers both facts; this checks the "is not there" half survives).
#[cfg(unix)]
#[test]
fn l3_config_absent_title_still_true_of_genuine_absence() {
    let p = support::pipeline("r45a_l3_config_gone", "before\n");
    p.commit_one("first");
    let config = p.project.join(".gx").join("config.toml");
    std::fs::remove_file(&config).expect("genuinely delete config.toml");

    let out = p.submit("second");
    let body = refusal(&out);
    let title = body["title"].as_str().unwrap_or_default();
    println!("R45A_L3_CTRL CONFIG_ABSENT(gone) title={title:?}");
    assert_eq!(body["gx_code"], "CONFIG_ABSENT");
    assert!(
        title.contains("is not there"),
        "the disjunction's first half must remain true of a genuine absence: {title:?}"
    );
}
