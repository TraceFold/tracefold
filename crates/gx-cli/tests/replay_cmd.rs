// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx replay` — 44 §1.2, **E-M5-2**, **M6-26 adopted (a)** (sem: SEM-gx-cli-1318).
//!
//! # What is actually being compared
//!
//! E-M5-2 made replay "a read-only operation that reconstructs only Σ" (sem: SEM-gx-cli-1319), so `gx_engine::reconstruct` is the whole
//! of it and a reconstruction compared against itself would answer `matches: true` about nothing.
//! What a single-shot process holds is two durable artefacts written by different code — the engine
//! journal and the ledger file — and Σ's `ledger` component is the journal's **claim** about the
//! second (`CommittedRow`'s own documentation: "This is the journal's claim about the ledger, not
//! the ledger's own root"; sem: SEM-gx-cli-1320). So `matches` is that claim checked, and `unchecked` names the three
//! components of Σ that have no second copy on disk.
//!
//! The probes below are therefore in pairs: one where the two agree, one where they are made to
//! disagree. Without the second, `matches: true` is a constant.

mod support;

use gx_core::Timestamp;
use gx_engine::store::{EngineJournal, EngineJournalRecord};
use support::{cid, iid, keypair, project, run, tid};

/// Write a journal holding one draft and `commits` commit records, at the sequence numbers given.
fn seed_journal(layout: &gx_cli::layout::Layout, commits: &[(u64, u64)]) {
    let path = layout.journal_path();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create");
    let mut journal = EngineJournal::open(&path).expect("open the journal");
    journal
        .append(EngineJournalRecord::DraftCreated {
            intent_id: iid(1),
            rng_seed: 7,
            at: Timestamp(1),
        })
        .expect("append");
    for (seed, seq) in commits {
        journal
            .append(EngineJournalRecord::Committed {
                transformation: tid(*seed),
                ledger_seq: *seq,
                at: Timestamp(2),
            })
            .expect("append");
    }
}

/// 🔴 The journal's claim about the ledger agrees with the ledger, and `matches` says so.
#[test]
fn replay_matches_when_the_journal_and_the_ledger_agree() {
    let (dir, layout) = project("replay_match");
    let key = keypair(1);
    // Six leaves land first, then the receipt's own at index 6.
    let (_receipt, index) = support::seed_ledger(&layout, &key, 30, 6);
    seed_journal(&layout, &[(30, index)]);

    let out = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    let json = out.json();
    println!("REPLAY_MATCH exit={} {json}", out.code);
    assert_eq!(
        out.code, 0,
        "44 §1.2: \"exit: 0=match\" (sem: SEM-gx-cli-1321)"
    );
    assert_eq!(json["matches"], serde_json::json!(true));
    assert_eq!(json["diffs"], serde_json::json!([]));
    assert_eq!(json["ledger_consulted"], serde_json::json!(true));
    assert_eq!(
        json["unchecked"],
        serde_json::json!(["drafts", "transformations", "escrow"]),
        "M6-26 adopted (a): the three components of Σ with no independent witness are named, so \
         `matches: true` cannot be read as \"all of Σ was compared\" (sem: SEM-gx-cli-1322)"
    );
    assert_eq!(json["records_replayed"], serde_json::json!(2));
}

/// 🔴 A journal that claims the wrong sequence number is caught, and `diffs` names the component.
///
/// M6-26 adopted (a): "`diffs` is the list of names of the first component that disagreed, for when there is a mismatch" (sem: SEM-gx-cli-1323). The transformation and both
/// numbers travel with the component name, because "the ledger disagrees" on its own is not
/// something an operator can act on.
#[test]
fn replay_reports_a_component_and_a_name_when_they_disagree() {
    let (dir, layout) = project("replay_diff");
    let key = keypair(2);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 31, 4);
    // The journal says 99; the ledger says `index`.
    seed_journal(&layout, &[(31, 99)]);

    let out = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    let json = out.json();
    println!("REPLAY_DIFF exit={} {json}", out.code);
    assert_eq!(
        out.code, 1,
        "44 §1.2: \"1=mismatch or unable to execute\" (sem: SEM-gx-cli-1324)"
    );
    assert_eq!(json["matches"], serde_json::json!(false));
    let diffs = json["diffs"].as_array().expect("a list");
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0]["component"], serde_json::json!("ledger"));
    assert_eq!(diffs[0]["journal_ledger_seq"], serde_json::json!(99));
    assert_eq!(diffs[0]["ledger_index"], serde_json::json!(index));
    assert_eq!(
        diffs[0]["transformation"],
        serde_json::json!(tid(31).0.to_text())
    );

    // A commit the ledger has never heard of: `ledger_index` is `null` rather than absent, so the
    // two failures are told apart ("wrong place" and "not there at all"; sem: SEM-gx-cli-1325).
    seed_journal(&layout, &[(999, 3)]);
    let out = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    let json = out.json();
    println!("REPLAY_ABSENT {json}");
    let missing = json["diffs"]
        .as_array()
        .expect("a list")
        .iter()
        .filter(|d| d["ledger_index"].is_null())
        .count();
    assert_eq!(missing, 1, "the commit the ledger does not hold");
}

/// 🔴 A replay with no ledger to consult is "unable to execute" and not "a match" (sem: SEM-gx-cli-1326).
///
/// The first reading of this command answered 0 when there was nothing to check, which is `matches`
/// meaning "nothing disagreed with me" (sem: SEM-gx-cli-1327) — the vacuous pass M6-26(c) was refused for.
#[test]
fn replay_without_a_ledger_is_not_a_match() {
    let (dir, layout) = project("replay_no_ledger");
    seed_journal(&layout, &[(32, 0)]);
    assert!(!layout.ledger_path().exists());

    let out = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    let json = out.json();
    println!("REPLAY_NO_LEDGER exit={} {json}", out.code);
    assert_eq!(out.code, 1);
    assert_eq!(json["matches"], serde_json::json!(false));
    assert_eq!(json["ledger_consulted"], serde_json::json!(false));
    assert!(
        !layout.ledger_path().exists(),
        "and the read created nothing"
    );
}

/// `<TID>` narrows to one transformation, and `--from/--to` narrows by journal record index.
///
/// 🔴 The indices are the **journal's** (M6H2-8). 44 writes `<INDEX>` and names neither sequence;
/// replay is defined on the journal, so the journal's append order is the reading that makes the
/// sentence true — and the other reading would silently replay a different set.
#[test]
fn the_two_ways_of_narrowing_select_what_they_say_they_select() {
    let (dir, layout) = project("replay_range");
    let key = keypair(3);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 33, 2);
    seed_journal(&layout, &[(33, index), (34, 77)]);

    // One transformation: the good one, so the bad row is excluded rather than merely outvoted.
    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("replay")
        .arg(tid(33).0.to_text()));
    let json = out.json();
    println!("REPLAY_BY_TID exit={} {json}", out.code);
    assert_eq!(out.code, 0);
    assert_eq!(json["records_replayed"], serde_json::json!(1));

    // The whole journal holds the bad row too.
    let out = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    println!("REPLAY_ALL exit={}", out.code);
    assert_eq!(out.code, 1, "the second commit's claim is wrong");
    assert_eq!(out.json()["records_replayed"], serde_json::json!(3));

    // Records 0..2 are the draft and the first commit.
    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("replay")
        .arg("--from")
        .arg("0")
        .arg("--to")
        .arg("2"));
    println!("REPLAY_RANGE exit={} {}", out.code, out.json());
    assert_eq!(out.code, 0);
    assert_eq!(out.json()["records_replayed"], serde_json::json!(2));

    // A range this journal has not got is "invalid input" (sem: SEM-gx-cli-1328) (1), and the message says which index space it
    // is in — because the whole risk of M6H2-8 is a caller using the other one.
    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("replay")
        .arg("--from")
        .arg("0")
        .arg("--to")
        .arg("99"));
    println!(
        "REPLAY_BAD_RANGE exit={} stderr={}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("journal"), "{}", out.stderr);
}

/// `--dry-run` is accepted and changes nothing, and the output says so rather than staying silent.
///
/// Replay writes nothing under E-M5-2, so 44 §1.2's flag names a difference this command does not
/// have. A flag that changes nothing and says nothing teaches an operator that it did something
/// (M6H2-9).
#[test]
fn dry_run_is_reported_rather_than_ignored() {
    let (dir, layout) = project("replay_dry");
    let key = keypair(4);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 35, 1);
    seed_journal(&layout, &[(35, index)]);

    let wet = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    let dry = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("replay")
        .arg("--dry-run"));
    println!("REPLAY_DRY exit={} {}", dry.code, dry.json());
    assert_eq!(dry.code, wet.code);
    assert_eq!(dry.json()["dry_run"], serde_json::json!(true));
    assert_eq!(wet.json()["dry_run"], serde_json::json!(false));
    assert_eq!(
        dry.json()["matches"],
        wet.json()["matches"],
        "the two runs answer the same question"
    );
    let _ = cid(0);
}
