// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-43-1, adopted (a)** and **DR-43-3** through the real `gx` binary — the undo CAS, its
//! refusal taxonomy, and the rows the taxonomy declares it does not judge.
//!
//! # What this suite exists to invert
//!
//! `req/182` §7-3 H-15 measured the behaviour this file now forbids, on two substrates:
//!
//! > **measurement A (fs)**: forward `AAA -> BBB`, a third party writes `CCC`, `gx undo --settle 6`
//! > times out and then fires — **RC 0 · Committed · file back at `AAA`** (`CCC` gone).
//! > **measurement B (git)**: a third party commit `db704a3` is dropped by the force reset (only the
//! > reflog keeps it).
//!
//! `req/38` §132 ruling 2 adopted DR-43-1 **(a)**: an undo is compared against `T_o`'s own signed
//! `postcondition_fingerprint` (42 §3.10) and **refused** if the world moved. The two arms below are
//! measurements A and B with their answers inverted, driven through the shipped binary rather than
//! through an in-process engine, because the fact being measured is about a fresh process reading a
//! receipt off disk.
//!
//! # And what it exists to keep honest
//!
//! DR-43-3 (`req/38` §144 ruling 2, material `req/207` §3) turns the CAS from one check into a
//! **closed table**: `gx_engine::UNDO_REFUSALS`, twelve rows, each naming its judging material, its
//! `gx_code`, its exit status and which of `aider/commands.py:553-625`'s seven preconditions it
//! answers. A table in a source file is a table until something compares it with the rest of the
//! system, so [`every_refusal_variant_owns_one_row_and_its_code_is_one_44_declares`] reads it
//! against `gx_api::gx_code`'s transcription of 44 §2.3 and against `gx_cli::exit`'s constants —
//! this crate is the lowest one that can see all three.
//!
//! # 🔴 The residual this suite pins deliberately
//!
//! A refused undo writes **nothing**: no journal record, no receipt, no ledger entry. That is
//! aider's property ("refuse and do nothing") and the only shape 42 §3.10 leaves available, since a
//! receipt is signed evidence about a transformation and a refused undo has none. It also means a
//! third party cannot verify from the ledger that an undo *was refused* — only that none happened.
//! [`a_third_party_write_after_the_commit_refuses_the_undo`] asserts the silence rather than leaving
//! it to be discovered, so that the day somebody rules that a refusal must be minted, this is the
//! assertion that fails.

mod support;

use std::path::PathBuf;

use support::{pipeline, run};

/// A transformation id that parses and names nothing (`undo_cmd.rs`'s constant, same shape).
const ABSENT: &str = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// How many files the receipt store holds for this project.
fn stored_receipts(project: &std::path::Path) -> usize {
    std::fs::read_dir(project.join(".gx").join("receipts"))
        .map(|d| d.filter_map(Result::ok).count())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// The CAS -- `req/182` H-15 measurement A, inverted
// ---------------------------------------------------------------------------

/// 🔴 **The measurement `req/182` H-15 made, with the ruling's answer.**
///
/// Five processes: submit, plan, verify, commit, undo. Between the fourth and the fifth a third
/// party writes the target behind gx's back, which is exactly what an agent's neighbour, a human
/// editor or a second tool does in the minute after a change lands.
///
/// What is asserted beyond the status: the file still holds the third party's bytes (the whole
/// point — H-15's finding was that they were gone), the refusal **names the path** so an operator
/// can look at it (`req/207` §3-2's "do not proceed on a guess", aider `commands.py:593`'s shape),
/// and the refusal left no trace in `.gx/` — no new receipt, no new journal record, no supersede
/// edge on the original.
#[test]
fn a_third_party_write_after_the_commit_refuses_the_undo() {
    let fixture = pipeline("undo_cas_moved", "before\n");
    let committed = fixture.commit_one("after\n");
    assert_eq!(fixture.target_contents(), "after\n");

    let receipts_before = stored_receipts(&fixture.project);
    let records_before = fixture.journal_records();
    std::fs::write(&fixture.target, "a third party wrote this\n").expect("move the world");

    let undone = run(fixture.gx().args(["undo", &committed, "--settle", "1"]));
    println!(
        "UNDO_CAS_MOVED exit={} target={:?} receipts={}->{} records={}->{} stderr={}",
        undone.code,
        fixture.target_contents(),
        receipts_before,
        stored_receipts(&fixture.project),
        records_before,
        fixture.journal_records(),
        undone.stderr.trim()
    );

    assert_eq!(
        undone.code, 3,
        "44 §1.4's 3 (`PRECONDITION_CHANGED`), which 44 §1.2 already lists for `gx undo`. \
         `req/38` §132 ruling 2 mints no new number. stderr: {}",
        undone.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "a third party wrote this\n",
        "🔴 `req/182` H-15 measurement A, inverted: the third party's write survives. It did not \
         before this ruling"
    );
    assert!(
        undone.stdout.trim().is_empty(),
        "44 §1.3: a refusal writes nothing to stdout, so a script piping stdout gets no receipt \
         for an undo that did not happen: {}",
        undone.stdout
    );
    assert!(
        undone
            .stderr
            .contains("\"gx_code\":\"PRECONDITION_CHANGED\""),
        "44 §1.3's problem object carries 44 §2.3's code, shared with the HTTP surface: {}",
        undone.stderr
    );
    assert!(
        undone
            .stderr
            .contains(&fixture.target.display().to_string()),
        "the refusal names the place that moved (42 §3.5's scope is the readable half of a \
         fingerprint, and the only half printed): {}",
        undone.stderr
    );
    assert_eq!(
        stored_receipts(&fixture.project),
        receipts_before,
        "🔴 a refused undo mints no receipt — the residual this suite pins on purpose"
    );
    assert_eq!(
        fixture.journal_records(),
        records_before,
        "🔴 and appends no journal record: the CAS runs before T-1 mints the intent, so a refusal \
         is not a draft somebody has to reap"
    );

    let shown = run(fixture.gx().args(["receipt", "show", &committed]));
    assert!(
        !shown.stdout.contains("Superseded"),
        "no supersede edge was drawn: the original is exactly where it was. {}",
        shown.stdout
    );
}

/// The control arm: an **unmoved** world still undoes, and still leaves a receipt.
///
/// Without this, the arm above would pass on an implementation that refused every undo.
#[test]
fn an_unmoved_world_still_undoes_and_still_mints_its_receipt() {
    let fixture = pipeline("undo_cas_unmoved", "before\n");
    let committed = fixture.commit_one("after\n");
    let receipts_before = stored_receipts(&fixture.project);

    let undone = run(fixture.gx().args(["undo", &committed]));
    println!(
        "UNDO_CAS_UNMOVED exit={} target={:?} receipts={}->{} stderr={}",
        undone.code,
        fixture.target_contents(),
        receipts_before,
        stored_receipts(&fixture.project),
        undone.stderr.trim()
    );
    assert_eq!(undone.code, 0, "stderr: {}", undone.stderr);
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "the escrowed inverse is what an undo applies"
    );
    assert!(
        undone.stderr.contains("result=matched") && undone.stderr.contains("polls=1"),
        "an in-process substrate matches on poll 1 — the CAS costs one snapshot and no sleep, \
         which is `req/38` §98 ruling 2's \"zero behavioural difference\" claim measured again \
         under the ruling that made the comparison load-bearing: {}",
        undone.stderr
    );
    assert!(
        stored_receipts(&fixture.project) > receipts_before,
        "and the undo that happened left its own receipt"
    );
    assert_eq!(
        undone.json()["superseded_state"],
        "Superseded",
        "43 T-12's edge is drawn on the road that is not refused"
    );
}

/// 🔴 `--settle 0` is not an override of the ruling.
///
/// `req/213` §8-7's standing refusal ("an override flag turns a ruling into a checkbox") applied to
/// this one: the flag disables **polling**, which is all it ever meant, and the receipt is still
/// read and the world still compared.
#[test]
fn settle_zero_disables_the_poll_and_not_the_cas() {
    let fixture = pipeline("undo_cas_settle_zero", "before\n");
    let committed = fixture.commit_one("after\n");
    std::fs::write(&fixture.target, "a third party wrote this\n").expect("move the world");

    let undone = run(fixture.gx().args(["undo", &committed, "--settle", "0"]));
    println!(
        "UNDO_CAS_SETTLE_ZERO exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert!(
        undone.stderr.contains("gx undo settle: disabled"),
        "the flag still does what it says: {}",
        undone.stderr
    );
    assert!(
        !undone.stderr.contains("result="),
        "and nothing polled: {}",
        undone.stderr
    );
    assert_eq!(
        undone.code, 3,
        "but the CAS still ran. A `--settle 0` that skipped it would be an operator flag that \
         turns DR-43-1 off. stderr: {}",
        undone.stderr
    );
    assert_eq!(fixture.target_contents(), "a third party wrote this\n");
}

// ---------------------------------------------------------------------------
// The taxonomy -- DR-43-3's other rows, by number
// ---------------------------------------------------------------------------

/// 43 §5.2's `not-ours` row: an id this ledger never committed (aider's (1) + (3)).
#[test]
fn an_id_this_ledger_never_committed_is_refused_as_not_ours() {
    let fixture = pipeline("undo_cas_not_ours", "before\n");
    let _ = fixture.commit_one("after\n");
    let undone = run(fixture.gx().args(["undo", ABSENT]));
    println!(
        "UNDO_CAS_NOT_OURS exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert_eq!(
        undone.code, 6,
        "44 §1.2's 6 for `gx undo`, unchanged by this ruling: {}",
        undone.stderr
    );
    assert!(undone.stderr.contains("\"gx_code\":\"NOT_FOUND\""));
}

/// 🔴 ~~**H-16's current form**~~ **H-16, closed** — an undo of an undo (`req/38` §148 ruling
/// 1(iii), lane R2).
///
/// `req/182` H-16 found that `Engine::undo` mints `T_u`'s intent in memory and writes no
/// `.gx/drafts/` entry, so `gx undo <T_u>` cannot rehydrate one. `req/38` §132 ruling 2 put the
/// draft archive in DR-43-2 lane R2, ~~which has not landed, so the honest thing is to pin **what
/// the binary does today** and let the assertion fail on the day R2 changes it~~ — **and R2
/// landed**. The struck words are kept because the test they describe is the reason this one
/// exists: `req/216` pinned exit **6** and wrote "the day DR-43-2 lane R2 lands a draft archive
/// this becomes a different number, and this assertion is where that shows up". This is that day
/// and this is that number.
///
/// What changed under it: `Engine::undo_intent` is the one definition of an undo's intent, and both
/// callers file what it gives them (`gx_cli::lifecycle::undo` into `.gx/drafts/`, `gx-api`'s
/// handler into its `DraftArchive`). So `T_u` has a body to be rebuilt from and 43 §5 applies to it
/// exactly as to any other `Committed` transformation — an undo of an undo is a re-application of
/// the original change, and it is not a special case.
///
/// **What must never change is the second half, and it is asserted the same way**: whatever the
/// status is, the world ends up somewhere a reader can name. Before: `before\n` (nothing moved).
/// Now: `after\n` (the original change, re-applied) — and the refusal that used to stand here is
/// gone, so the file being back at `before\n` would mean the second undo silently did nothing.
#[test]
fn an_undo_of_an_undo_re_applies_the_original_change() {
    let fixture = pipeline("undo_cas_undo_of_undo", "before\n");
    let committed = fixture.commit_one("after\n");
    let first = run(fixture.gx().args(["undo", &committed]));
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);
    let t_u = first.json()["transformation"]
        .as_str()
        .expect("the undo names its own transformation")
        .to_string();
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "the first undo took the change back"
    );

    let second = run(fixture.gx().args(["undo", &t_u]));
    println!(
        "UNDO_CAS_UNDO_OF_UNDO exit={} target={:?} stderr={}",
        second.code,
        fixture.target_contents(),
        second.stderr.trim()
    );
    assert_eq!(
        second.code, 0,
        "🔴 H-16 closed. Until lane R2 this was 44 §1.2's **6**, because the road stopped at a \
         draft nobody had written; `gx undo` now files `T_u`'s own intent, so the row can be \
         rebuilt and 43 §5 applies to it like any other commit. stderr: {}",
        second.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "and it undid the undo: the original change is back, which is what undoing an inverse is"
    );
}

/// 🔴 The table is compared with the rest of the system rather than with itself.
///
/// Three properties, none of which gx-engine can check for itself (it sits below both gx-cli and
/// gx-api): every `UndoRefusal` variant owns exactly one judged row; every judged row's `gx_code` is
/// one 44 §2.3 declares or one `req/38` ruled as an addition; and every judged row's status pair is
/// the pair that declaration gives it — so no row can invent a number.
#[test]
fn every_refusal_variant_owns_one_row_and_its_code_is_one_44_declares() {
    use gx_api::gx_code::{GX_CODES, RULED_ADDITIONS};
    use gx_engine::UNDO_REFUSALS;

    let judged: Vec<&str> = UNDO_REFUSALS
        .iter()
        .filter(|r| r.judged)
        .map(|r| r.reason)
        .collect();
    let variants = [
        gx_engine::UndoRefusal::NotOurs,
        gx_engine::UndoRefusal::NoBody { state: "Committed" },
        gx_engine::UndoRefusal::NotCommitted { state: "Denied" },
        gx_engine::UndoRefusal::NoEscrow,
        gx_engine::UndoRefusal::InverseUnavailable,
        gx_engine::UndoRefusal::InversePending,
        gx_engine::UndoRefusal::AlreadyUndone,
        gx_engine::UndoRefusal::NoAdapter {
            substrate: "fs".to_string(),
        },
        gx_engine::UndoRefusal::WorldMoved {
            expected: gx_core::FingerprintBytes([0u8; 32]),
            found: gx_core::FingerprintBytes([1u8; 32]),
            scope: "/tmp/x".to_string(),
        },
        gx_engine::UndoRefusal::AlreadyPlanned { state: "Denied" },
        // 🔴 **R3 / `req/222` H-01, H-02** — the eleventh judged row. Before it, "there is no
        // evidence" was not a refusal at all: `Engine::undo` skipped the comparison and fired, and
        // one `rm` under `.gx/receipts/` was enough to reach that from outside the process.
        gx_engine::UndoRefusal::WitnessMissing {
            missing: gx_engine::WitnessMissing::NoReceipt,
        },
        // 🔴 **DR-46-47 (`req/973` §9-3)** — the twelfth judged row. A re-plan of a `T_u` still
        // seated as a `Candidate` used to be answered with the **first** call's journalled
        // disposition, which since DR-46-45 is what T-11 signs. It mints no code and no number: the
        // row takes `already-planned`'s, and that is what this test's second and third properties
        // check rather than take on trust.
        gx_engine::UndoRefusal::WitnessDiffers,
    ];
    let named: Vec<&str> = variants
        .iter()
        .map(gx_engine::UndoRefusal::reason)
        .collect();
    println!(
        "UNDO_TAXONOMY rows={} judged={judged:?}",
        UNDO_REFUSALS.len()
    );
    assert_eq!(
        named, judged,
        "every judged row is a variant and every variant is a judged row, in declaration order"
    );

    for row in UNDO_REFUSALS.iter().filter(|r| r.judged) {
        let declared = GX_CODES
            .iter()
            .chain(RULED_ADDITIONS.iter())
            .find(|c| c.code == row.gx_code)
            .unwrap_or_else(|| {
                panic!(
                    "43 §5.2's `{}` row answers with `{}`, which is neither one of 44 §2.3's twelve \
                     nor one of the ruled additions — a taxonomy may not invent a code",
                    row.reason, row.gx_code
                )
            });
        assert_eq!(
            (row.http_status, row.cli_exit),
            (declared.status, declared.cli_exit),
            "43 §5.2's `{}` row disagrees with the declaration of `{}`",
            row.reason,
            row.gx_code
        );
        assert!(
            !row.material.is_empty() && !row.aider.is_empty() && !row.test.is_empty(),
            "every row states what it judges, which of aider's seven it answers, and what measures \
             it: {}",
            row.reason
        );
    }

    // The numbers the CLI actually returns are the ones the rows claim.
    assert_eq!(gx_cli::exit::PRECONDITION_CHANGED, 3);
    assert_eq!(gx_cli::exit::NOT_FOUND, 6);
    assert_eq!(gx_cli::exit::DENIED, 2);
}

/// 🔴 The two rows this system **declares it does not judge** (`req/207` §3-4's discipline).
///
/// aider checks seven preconditions and we fill five of them. The other two are written down with
/// the reason rather than dropped, because a table that silently omitted the rows it could not fill
/// would be claiming a completeness nobody measured — which is the exact failure `req/207` §3-4
/// named when it asked for "the rows that do not fill marked rather than dropped".
#[test]
fn the_table_declares_the_two_rows_it_does_not_judge() {
    use gx_engine::UNDO_REFUSALS;
    let unjudged: Vec<&str> = UNDO_REFUSALS
        .iter()
        .filter(|r| !r.judged)
        .map(|r| r.reason)
        .collect();
    println!("UNDO_TAXONOMY_UNJUDGED {unjudged:?}");
    assert_eq!(
        unjudged,
        vec!["downstream-dependent", "ambiguous-predecessor"],
        "the two rows aider has and we do not judge separately"
    );
    for row in UNDO_REFUSALS.iter().filter(|r| !r.judged) {
        assert!(
            row.material.contains("not judged"),
            "an unjudged row says so in its own material rather than looking like a judged one: {}",
            row.reason
        );
    }
    // 🔴 ~~13 — **eleven** judged plus two declared, since R3 added `witness-missing`
    // (`req/38` §160 ruling 2).~~ The number is the ratchet and it worked exactly as written: this
    // assertion is what stopped **DR-46-47** from adding a row without a decision about which kind
    // it is. The ruling that moves it is `req/973` §9-3's DR-46-47 (filed 2026-08-31, repaired
    // 2026-09-01), which mints `witness-differs` as a **judged** row taking `already-planned`'s
    // `gx_code`, exit status and HTTP status — so the count moves and the *shape* of the table does
    // not. The struck number is kept because it is the true record of the era `req/216`/`req/222`
    // describe.
    assert_eq!(
        UNDO_REFUSALS.len(),
        14,
        "🔴 **twelve** judged plus two declared, since DR-46-47 (`req/973` §9-3) added \
         `witness-differs`. A row added without a decision about which it is would change this \
         number"
    );
}

// ---------------------------------------------------------------------------
// git -- `req/182` H-15 measurement B, inverted
// ---------------------------------------------------------------------------

/// A repository on a tmpfs holding one commit on `refs/heads/main` with a `README.md` in it.
///
/// The reason for the tmpfs is `crates/gx-adapter-git/tests/support/mod.rs`'s and `defaults.rs`
/// repeats it: `/mnt/c` is 9p over NTFS and does not give the POSIX rename semantics a reference
/// lock is renamed with. Written with `gix` rather than by shelling out, for the same reason
/// `defaults.rs` gives — an external `git` in a test is a second road to the substrate.
fn tmpfs_repo(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var("GLOVREX_GIT_TEST_ROOT").unwrap_or_else(|_| "/dev/shm".to_string()),
    );
    assert!(
        root.is_dir(),
        "{} is not a directory; set GLOVREX_GIT_TEST_ROOT to a tmpfs",
        root.display()
    );
    let dir = root.join(format!("glovrex-undo-cas-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a tmpfs accepts a directory");

    let repo = gix::init(&dir).expect("gitoxide creates a repository");
    write_entry(&repo, b"before\n", "fixture");
    dir
}

/// Write a one-entry tree holding `contents` at `README.md`, commit it and move `refs/heads/main`.
///
/// Used twice: once to build the fixture, and once as the **third party** — a commit that never
/// passed through gx, which is measurement B's disturbance.
fn write_entry(repo: &gix::Repository, contents: &[u8], message: &str) {
    let blob = repo
        .write_blob(contents)
        .expect("a blob is written")
        .detach();
    let tree = repo
        .write_object(&gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: "README.md".into(),
                oid: blob,
            }],
        })
        .expect("a tree is written")
        .detach();
    let signature = gix::actor::Signature {
        name: "gx-fixture".into(),
        email: "fixture@glovrex.invalid".into(),
        time: gix::date::Time::new(1_754_000_000, 0),
    };
    let parents: Vec<gix::hash::ObjectId> = repo
        .find_reference("refs/heads/main")
        .ok()
        .and_then(|mut r| r.peel_to_id().ok())
        .map(|id| vec![id.detach()])
        .unwrap_or_default();
    let commit = repo
        .write_object(&gix::objs::Commit {
            tree,
            parents: parents.into(),
            author: signature.clone(),
            committer: signature.clone(),
            encoding: None,
            message: message.into(),
            extra_headers: Vec::new(),
        })
        .expect("a commit is written")
        .detach();
    // Through a transaction with an explicit committer: `Repository::reference` takes the identity
    // from configuration and this repository has none (`defaults.rs` measured the same refusal).
    let edit = gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: message.into(),
            },
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Object(commit),
        },
        name: gix::refs::FullName::try_from("refs/heads/main").expect("a full reference name"),
        deref: false,
    };
    // Through the transaction with an explicit committer rather than `Repository::edit_reference`:
    // the latter takes the identity from configuration and this repository has none, so it answers
    // `CreateOrUpdateRefLog(MissingCommitter)` — measured here and already measured once in
    // `defaults.rs::tmpfs_repo`, whose comment carries the finding.
    repo.refs
        .transaction()
        .prepare(
            vec![edit],
            gix::lock::acquire::Fail::Immediately,
            gix::lock::acquire::Fail::Immediately,
        )
        .expect("the reference transaction prepares")
        .commit(signature.to_ref(&mut gix::date::parse::TimeBuf::default()))
        .expect("the reference transaction commits");
}

/// 🔴 **`req/182` H-15 measurement B, inverted** — a third party's git commit is not force-reset
/// away by an undo.
///
/// The finding was that the undo moved `refs/heads/main` back over a commit gx had never seen,
/// leaving it reachable only from the reflog. Here the same disturbance produces a refusal, and the
/// assertion that matters is the last one: the branch still points at the third party's commit.
#[test]
fn a_third_party_commit_after_the_commit_refuses_the_git_undo() {
    let repo_dir = tmpfs_repo("moved");
    let fixture = pipeline("undo_cas_git", "unused\n");
    let locator = format!("{}#refs/heads/main:README.md", repo_dir.display());

    let goal = fixture.project.join("goal-git.txt");
    std::fs::write(&goal, "after\n").expect("write the goal");
    let submitted = run(fixture
        .gx()
        .arg("submit")
        .args(["--substrate", "git"])
        .arg("--locator")
        .arg(&locator)
        .arg("--intent")
        .arg(&goal)
        .args(["--context", "Evidence"])
        .args(["--actor-key", &fixture.key_id]));
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent_id]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    let verified = run(fixture.gx().args(["verify", &tid]));
    assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
    let committed = run(fixture.gx().args(["commit", &tid]));
    assert_eq!(committed.code, 0, "commit: {}", committed.stderr);

    let repo = gix::open(&repo_dir).expect("the fixture repository opens");
    write_entry(&repo, b"a third party committed this\n", "third party");
    let third_party = head_contents(&repo_dir);
    assert_eq!(third_party, "a third party committed this\n");

    let undone = run(fixture.gx().args(["undo", &tid, "--settle", "1"]));
    println!(
        "UNDO_CAS_GIT exit={} head={:?} stderr={}",
        undone.code,
        head_contents(&repo_dir),
        undone.stderr.trim()
    );
    assert_eq!(
        undone.code, 3,
        "the git adapter's position moved as surely as the filesystem's. stderr: {}",
        undone.stderr
    );
    assert_eq!(
        head_contents(&repo_dir),
        "a third party committed this\n",
        "🔴 `req/182` H-15 measurement B, inverted: the third party's commit is still what \
         `refs/heads/main` points at, rather than something only the reflog remembers"
    );
    let _ = std::fs::remove_dir_all(&repo_dir);
}

/// What `refs/heads/main:README.md` holds, read straight out of the object database.
fn head_contents(dir: &std::path::Path) -> String {
    let repo = gix::open(dir).expect("the repository opens");
    let tree = repo
        .find_reference("refs/heads/main")
        .expect("the branch is there")
        .peel_to_id()
        .expect("it points at a commit")
        .object()
        .expect("the commit is in the odb")
        .into_commit()
        .tree()
        .expect("the commit has a tree");
    let entry = tree
        .lookup_entry_by_path("README.md")
        .expect("the tree can be walked")
        .expect("README.md is in the tree");
    let object = entry.object().expect("the blob is in the odb");
    String::from_utf8_lossy(&object.data).to_string()
}
