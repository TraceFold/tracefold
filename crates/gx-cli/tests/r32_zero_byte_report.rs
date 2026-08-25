// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R32 / `req/392` M-01 — what the shipped verb says about a project whose journal is zero bytes.
//!
//! # The measurement this reverses
//!
//! The audit's §2-4 drove `gx repair --json` over a project declaring `journal_format=chained`
//! whose journal held no bytes, and got, all at once:
//!
//! ```text
//! A31_ZERO repair_json_rc=0
//!   "journal_intact": true
//!   "journal_format": "chained-v2"
//!   "journal_format_declared": "chained"
//!   "downgraded": false
//!   "journal_intact_basis": "chain"
//! ```
//!
//! Four things: a state the writer's door will not extend reported as **intact**; a framing named
//! that no byte on the disk carries; R6's guard silent because `1 > 2`; and an intactness whose
//! stated **basis** is a chain that does not exist. The control the audit ran beside it — the same
//! project with **four unmarked bytes** instead of none — answered `rc=1`, `journal_intact:
//! false`, with `downgraded` firing. The file holding strictly less was called the healthier one.
//!
//! Self-directed test of this repository's own CLI. Every project built here lives inside this
//! worktree's `CARGO_TARGET_TMPDIR`; no network is used.

mod support;

use std::path::{Path, PathBuf};

use support::{run, scratch, Run};

/// A project directory holding a `chained` declaration and a journal of exactly the bytes given.
///
/// The audit's bed and `r31_e2e_empty_journal_submit.rs`'s, built from the same bytes so the three
/// suites are comparable.
fn half_made(name: &str, journal: Option<&[u8]>) -> PathBuf {
    let root = scratch(name);
    let gx = root.join(".gx");
    std::fs::create_dir_all(&gx).expect("make .gx/");
    std::fs::write(gx.join("VERSION"), "1\njournal_format=chained\n").expect("the declaration");
    std::fs::write(gx.join("config.toml"), "# settings\n").expect("the settings");
    if let Some(bytes) = journal {
        std::fs::create_dir_all(gx.join("ledger")).expect("make .gx/ledger/");
        std::fs::write(gx.join("ledger").join("journal"), bytes).expect("the journal");
    }
    root
}

fn gx_at(project: &Path) -> std::process::Command {
    let mut cmd = support::gx();
    cmd.arg("--project").arg(project);
    cmd
}

/// The four fields the audit printed, read off `gx repair --json`.
fn report_of(project: &Path) -> (i32, serde_json::Value) {
    let r: Run = run(gx_at(project).args(["repair", "--json"]));
    let json: serde_json::Value =
        serde_json::from_str(&r.stdout).unwrap_or(serde_json::Value::Null);
    (r.code, json)
}

fn field(json: &serde_json::Value, key: &str) -> serde_json::Value {
    json.get(key).cloned().unwrap_or(serde_json::Value::Null)
}

/// 🔴 The bed, and the four values that have to change together.
#[test]
fn r32_a_zero_byte_journal_is_not_reported_as_an_intact_chain() {
    let project = half_made("r32_zero_bed", Some(b""));
    let (code, json) = report_of(&project);
    println!(
        "R32_ZERO rc={code} journal_intact={} journal_format={} journal_format_declared={} \
         downgraded={} journal_intact_basis={} journal_absent={}",
        field(&json, "journal_intact"),
        field(&json, "journal_format"),
        field(&json, "journal_format_declared"),
        field(&json, "downgraded"),
        field(&json, "journal_intact_basis"),
        field(&json, "journal_absent"),
    );

    assert_eq!(
        field(&json, "journal_intact"),
        serde_json::json!(false),
        "🔴 `req/392` M-01: the audit read `true` here, about a file the writer's door has to \
         stamp before it can be appended to"
    );
    assert_ne!(
        field(&json, "journal_intact_basis"),
        serde_json::json!("chain"),
        "🔴 `req/392` M-01: `\"chain\"` is a statement about **what the intactness was \
         established from** (audit 8 M-05). There is no chain on a file of zero bytes, so this was \
         a fact asserted about something that does not exist"
    );
    assert_eq!(
        field(&json, "journal_format"),
        serde_json::json!("legacy"),
        "🔴 the framing named is the framing on the disk. The audit read `\"chained-v2\"` beside \
         `journal_format_declared: \"chained\"` — the very shape (declaration and marker \
         disagreeing) that R31 was the repair for, printed to an operator"
    );
    assert_eq!(
        field(&json, "downgraded"),
        serde_json::json!(true),
        "🔴 `req/229` H-02's guard: this project declares a chain and its file carries no marker. \
         The audit measured `false`, because `2` was not greater than `2`"
    );
    assert_eq!(
        code, 1,
        "and the verb that exists to report on this file no longer exits 0 over it"
    );
    assert_eq!(
        std::fs::metadata(project.join(".gx").join("ledger").join("journal"))
            .expect("stat")
            .len(),
        0,
        "`gx repair` without `--yes` is a report: it wrote nothing, here or anywhere"
    );
}

/// 🔴 §2-4's control, beside it: the asymmetry itself.
#[test]
fn r32_zero_bytes_is_not_reported_healthier_than_four_unmarked_bytes() {
    let zero = half_made("r32_asym_zero", Some(b""));
    let four = half_made("r32_asym_four", Some(&[0u8; 4]));
    let (zero_code, zero_json) = report_of(&zero);
    let (four_code, four_json) = report_of(&four);

    let shape = |json: &serde_json::Value| {
        (
            field(json, "journal_intact"),
            field(json, "downgraded"),
            field(json, "journal_format"),
            field(json, "journal_intact_basis"),
        )
    };
    println!(
        "R32_ASYMMETRY zero=(rc={zero_code}, {:?}) four=(rc={four_code}, {:?})",
        shape(&zero_json),
        shape(&four_json)
    );
    assert_eq!(
        shape(&zero_json),
        shape(&four_json),
        "🔴 `req/392` M-01: neither file carries a marker and both are declared `chained`, so the \
         report has the same four things to say about them. The audit measured `rc=0 / intact / \
         chained-v2 / basis \"chain\"` against `rc=1 / not intact / downgraded`"
    );
    assert_eq!(
        zero_code, four_code,
        "and the exit code an operator's script branches on is the same too"
    );
}

/// The neighbouring bed that must **not** move: a journal that is exactly its declared marker.
///
/// This is a real, healthy, empty project — the state `gx init` leaves — and calling it damaged
/// would be this lane's own false refusal.
#[test]
fn r32_negative_control_a_journal_that_is_only_its_marker_is_still_intact() {
    let project = half_made("r32_ctl_marker_only", Some(b"GXJRNL01"));
    let (code, json) = report_of(&project);
    println!(
        "R32_CTL rc={code} journal_intact={} journal_format={} downgraded={} basis={}",
        field(&json, "journal_intact"),
        field(&json, "journal_format"),
        field(&json, "downgraded"),
        field(&json, "journal_intact_basis"),
    );
    assert_eq!(
        field(&json, "journal_intact"),
        serde_json::json!(true),
        "eight bytes of the declared marker is a healthy empty journal, and this lane does not \
         make it a damaged one"
    );
    assert_eq!(field(&json, "journal_format"), serde_json::json!("chained"));
    assert_eq!(field(&json, "downgraded"), serde_json::json!(false));
    assert_eq!(
        field(&json, "journal_intact_basis"),
        serde_json::json!("chain"),
        "and where there **is** a chain, that is still what the intactness is established from"
    );
}

/// And the project with no journal file at all, which is a different fact and has its own key.
#[test]
fn r32_negative_control_a_project_with_no_journal_is_still_its_own_answer() {
    let project = half_made("r32_ctl_absent", None);
    let (code, json) = report_of(&project);
    println!(
        "R32_ABSENT rc={code} journal_absent={} journal_intact={}",
        field(&json, "journal_absent"),
        field(&json, "journal_intact"),
    );
    assert_eq!(
        field(&json, "journal_absent"),
        serde_json::json!(true),
        "🔴 `req/240` H-02's key: a journal that is not there is not a journal that is empty, and \
         this lane does not merge the two"
    );
}
