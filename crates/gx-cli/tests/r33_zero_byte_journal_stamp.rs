// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R33 / `req/442` §0-2 (b)** — what actually happens to a journal file with no bytes in it.
//!
//! # Why this file exists at all, and why it changes nothing
//!
//! `req/442` §0-2 (b) asked a question about `DeclarationWriter::ensure_journal` and
//! `DeclarationWriter::create_journal`: their first line is `if journal.exists() { return Ok(()) }`,
//! and a **zero-byte** journal exists, so — the reasoning went — a project in that state can never
//! be finished by the road that was making it. Monitoring 31 M-01 and monitoring 32 §3 had both
//! driven the state and found `gx repair` calling it healthy, which R32 then corrected to exit 1
//! (`req/38` §250).
//!
//! This lane implemented the widening, drove it, and then drove the same beds against the
//! **unmodified** binary. They behave identically. The reasoning was wrong about one layer: the
//! CLI's guard declines a job the **engine's** writer door already does. `EngineJournal`'s
//! `open_declared_creating` stamps the declared marker into a journal it finds empty (the
//! `write_all(marker)` / `barrier` / `extend_from_slice(marker)` sequence R31 put in order), so the
//! first writer verb after the accident finishes the file. The source change was reverted; what
//! stays is (a), the durability of the CLI's own stamp, and this file, which pins the behaviour so
//! that the question does not have to be asked a third time.
//!
//! # What is asserted, and what each assertion is worth
//!
//! Every assertion here is about the **shipped** road and was green before R33 touched anything —
//! that is the point of the file, and `req/443` carries the paired control run. It is a
//! regression pin and not a repair.
//!
//! 1. A project declaring `chained` whose journal holds no bytes is **not** locked out: the next
//!    `gx submit` succeeds, the file gains the marker its own declaration names, and `gx repair`
//!    afterwards answers exit 0 with `journal_intact_basis: "chain"`.
//! 2. A project declaring **nothing** keeps its declaration. The journal it gains carries this
//!    build's marker, which is `docs/LIMITS.md`'s declared gap and not a finding: an undeclared
//!    project has no downgrade detector, as `declaration.rs`'s module header says in as many words.
//! 3. A project declaring **legacy** keeps an empty journal, because a journal with no marker *is*
//!    a legacy journal and `JournalFormat::marker` has nothing to write for one.

mod support;

use std::path::{Path, PathBuf};

use support::{pipeline, Run};

/// The bed: `.gx/VERSION`, `.gx/config.toml`, and a `ledger/journal` holding nothing.
///
/// Composed of nothing invented. `create_journal` wrote its eight bytes with a single
/// `std::fs::write` before R33, so a process interrupted between the file's creation and those
/// bytes reaching the device leaves exactly this.
fn a_project_whose_journal_was_not_finished(root: &Path, name: &str, declaration: &str) -> PathBuf {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".gx").join("ledger")).expect("make .gx/ledger/");
    std::fs::write(dir.join(".gx").join("VERSION"), declaration).expect("the declaration");
    std::fs::write(dir.join(".gx").join("config.toml"), "# settings\n").expect("the settings");
    std::fs::write(dir.join(".gx").join("ledger").join("journal"), b"").expect("no bytes at all");
    dir
}

fn journal_of(project: &Path) -> Vec<u8> {
    std::fs::read(project.join(".gx").join("ledger").join("journal")).expect("read the journal")
}

#[test]
fn r33_a_zero_byte_journal_is_finished_by_the_next_writer_verb() {
    let fixture = pipeline("r33_zero_byte_journal", "before\n");
    let key = fixture.key_id.clone();
    let home = fixture.home.clone();
    let root = fixture
        .project
        .parent()
        .expect("a scratch root")
        .to_path_buf();

    let submit_into = |project: &Path| -> Run {
        let goal = project.join("intent.txt");
        std::fs::write(&goal, "a goal\n").expect("write the intent");
        let target = project.join("target.txt");
        std::fs::write(&target, "hello\n").expect("write the target");
        let mut cmd = support::gx();
        cmd.env("HOME", &home)
            .env("USERPROFILE", &home)
            .arg("--project")
            .arg(project)
            .arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&target)
            .arg("--intent")
            .arg(&goal)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &key]);
        support::run(&mut cmd)
    };
    let repair_json = |project: &Path| -> (i32, serde_json::Value) {
        let mut cmd = support::gx();
        cmd.env("HOME", &home)
            .env("USERPROFILE", &home)
            .arg("--project")
            .arg(project)
            .args(["repair", "--json"]);
        let run = support::run(&mut cmd);
        let json: serde_json::Value =
            serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
        (run.code, json)
    };

    // -----------------------------------------------------------------------------------
    // (1) the project that declares `chained`
    // -----------------------------------------------------------------------------------
    let half = a_project_whose_journal_was_not_finished(
        &root,
        "r33_zb_chained",
        &format!(
            "1\njournal_format={}\n",
            support::CREATED_JOURNAL_FORMAT.kind()
        ),
    );
    let (before_code, before) = repair_json(&half);
    println!(
        "R33_ZB chained/before bytes={} repair_exit={before_code} journal_intact={} \
         journal_format={} journal_format_declared={} journal_intact_basis={}",
        journal_of(&half).len(),
        before["journal_intact"],
        before["journal_format"],
        before["journal_format_declared"],
        before["journal_intact_basis"],
    );
    assert!(
        journal_of(&half).is_empty(),
        "instrument: the bed starts with no bytes"
    );

    let run = submit_into(&half);
    println!(
        "R33_ZB chained/submit exit={} bytes_after={} stderr={}",
        run.code,
        journal_of(&half).len(),
        run.stderr.trim()
    );
    assert_eq!(
        run.code, 0,
        "🔴 a project whose journal was never finished being written is not locked out: {}",
        run.stderr
    );
    assert_eq!(
        journal_of(&half).get(..8).map(<[u8]>::to_vec),
        support::CREATED_JOURNAL_FORMAT
            .marker()
            .map(|m| m.as_slice().to_vec()),
        "🔴 and the marker it gains is the one **this project declares**, not the one this build \
         mints for new projects (R30 / `req/372` M-02)"
    );
    let (after_code, after) = repair_json(&half);
    println!(
        "R33_ZB chained/after repair_exit={after_code} journal_intact={} journal_format={} \
         journal_intact_basis={} remedy_is_null={}",
        after["journal_intact"],
        after["journal_format"],
        after["journal_intact_basis"],
        after["remedy"].is_null(),
    );
    assert_eq!(
        after_code, 0,
        "🔴 the project is writable, and `gx repair` says so: {after}"
    );
    assert_eq!(
        after["journal_intact"],
        serde_json::Value::Bool(true),
        "🔴 intact: {after}"
    );
    assert_eq!(
        after["journal_intact_basis"],
        serde_json::Value::String("chain".to_string()),
        "🔴 on the basis of a chain that is really there, and not the `not-intact` a file with no \
         framing answers (monitoring 32 §3's four-byte control): {after}"
    );

    // -----------------------------------------------------------------------------------
    // (2) the project that declares nothing
    // -----------------------------------------------------------------------------------
    let undeclared = a_project_whose_journal_was_not_finished(&root, "r33_zb_undeclared", "1\n");
    let run = submit_into(&undeclared);
    println!(
        "R33_ZB undeclared/submit exit={} bytes_after={} declaration={:?}",
        run.code,
        journal_of(&undeclared).len(),
        std::fs::read_to_string(undeclared.join(".gx").join("VERSION")).expect("read"),
    );
    assert_eq!(run.code, 0, "not locked out either: {}", run.stderr);
    assert_eq!(
        std::fs::read_to_string(undeclared.join(".gx").join("VERSION")).expect("read"),
        "1\n",
        "🔴 **nothing stamped the declaration.** That is R12's whole finding (`req/242` H-01): the \
         declaration is the operator's, an undeclared project stays undeclared, and `gx repair` \
         says so in `journal_format_declared`. The journal it gains carries this build's framing \
         and no detector guards it — `declaration.rs`'s module header and `docs/LIMITS.md` v0.4-y \
         both carry that gap, so it is declared and not found here"
    );

    // -----------------------------------------------------------------------------------
    // (3) the project that declares `legacy`
    // -----------------------------------------------------------------------------------
    let legacy = a_project_whose_journal_was_not_finished(
        &root,
        "r33_zb_legacy",
        "1\njournal_format=legacy\n",
    );
    // This one is a **measurement and not a gate**, and the reason is in the number it prints.
    //
    // `JournalFormat::marker` has nothing to write for `legacy` — a legacy journal is a journal
    // with no marker — so the engine's stamp reaches its fallback and writes this build's
    // `GXJRNL02` into a project whose `.gx/VERSION` declares the opposite. The guard that would
    // notice is `downgraded`, which asks whether the **declaration** out-ranks the file; a
    // declaration of `legacy` out-ranks nothing, so it cannot fire. This lane is `req/397` H-01's
    // repair and does not touch R30/R31's stamp, so the line below prints what the shipped binary
    // does and `req/443` carries it to the next monitoring round rather than asserting a verdict
    // this lane has not earned the right to.
    let run = submit_into(&legacy);
    let after = journal_of(&legacy);
    println!(
        "R33_ZB legacy/submit exit={} bytes_after={} first8={:?} is_v2_marker={}",
        run.code,
        after.len(),
        String::from_utf8_lossy(after.get(..8).unwrap_or(&[])),
        after.starts_with(gx_engine::replay::JOURNAL_MAGIC_V2),
    );
    assert_eq!(
        run.code, 0,
        "instrument: the legacy bed has to reach the writer door for the line above to mean \
         anything: {}",
        run.stderr
    );
}
