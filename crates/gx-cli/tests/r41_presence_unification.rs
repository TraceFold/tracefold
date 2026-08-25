// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R41 / `req/561` — the `is_file()` fold's last five mouths ask [`presence_of`]'s question.**
//!
//! # What `req/559` measured
//!
//! `keys.rs`'s `load` spelled "is this key there" as `!path.is_file()`, and `Path::is_file()` folds
//! **every** `stat` failure into "no". Under attempt_6's eleven-target parallel floor a transient
//! `stat` failure on a key that was sitting on disk made `gx undo` refuse `NOT_FOUND` about a key
//! that existed — the flake `req/559` diagnosed. The same fold stood at `ledger::open`,
//! `replay::open` and `verdict::list_from_file`, each a read door that does not pass through
//! `Layout::open`'s R40 gate, so R40's fix never reached them.
//!
//! # What this suite pins (req/561 §4 AC-1/AC-2)
//!
//! Three arms per site, each with the fold's three answers separated:
//!
//! * `(a)` — truly absent: the site's **existing** word is unchanged (`NotFound` for S-1..S-4, an
//!   empty `Ok` for S-5). The word did not move; only its firing condition narrowed.
//! * `(b)` — a directory standing where the file is declared: **not** the absence word. The site
//!   passes `Present` through to the open it always called, and that open's own existing word
//!   comes out.
//! * `(c)` — a `stat` this process may not make (parent directory unreadable): **not** the absence
//!   word. `Undetermined` passes through the same way. This is `req/559`'s flake shape, and the
//!   probe req/561 AC-8 asks for.
//!
//! # The KA-1 probes (req/561 §5)
//!
//! `ka1_*` below ask the four downstream doors **directly** — `KeyPair::load`,
//! `LedgerStore::open_read_only`, `EngineJournal::open_read_only`,
//! `VerdictCheckpointStore::open_read_only` — with no `is_file()` short-circuit in front of them,
//! and assert each answers `Err` (rather than succeeding silently, panicking or blocking) for both
//! non-absent shapes. They pin the contract R-1b's pass-through rests on, so a later change to a
//! downstream door that starts swallowing these shapes fails here by name.
//!
//! # What this suite does **not** claim
//!
//! That the words the downstream doors answer with are the *right* words for these shapes. R41
//! moves no word and mints none (req/561 §0); which word a directory-shaped ledger deserves is a
//! spec question and not this lane's.

use std::path::{Path, PathBuf};

#[path = "support/mod.rs"]
mod support;

use gx_cli::keys::KeyStore;
use gx_cli::layout::Layout;
use gx_cli::{ledger, replay, verdict, Error};
use gx_engine::store::EngineJournal;
use gx_log::store::VerdictCheckpointStore;
use gx_log::LedgerStore;
use gx_witness::KeyPair;
use support::run;

/// An empty directory under the cargo target directory — `gx_layout.rs`'s convention, for its
/// reason: `/tmp` on this project's WSL2 setup is cleared while the machine sits idle. Cleared on
/// entry, not on exit, so a failing test leaves its tree to be read.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set the mode");
}

/// `permissions_do_not_bind` and its skip carrier live in `support/mod.rs` now (SS552 worst-1
/// fold: this file and `r43_presence_and_head.rs` carried ~90 duplicated lines of this machinery).
/// This wrapper is `#[track_caller]` so the printed site and the carrier line still name whichever
/// of *this file's* several call sites asked, not `support/mod.rs`.
#[cfg(unix)]
#[track_caller]
fn permissions_do_not_bind(parent: &Path, child: &Path) -> bool {
    support::permissions_do_not_bind("r41", parent, child)
}

/// R45-b / `req/635` box M-2 — a probe of the shared skip-carrier mechanism in `support/mod.rs`,
/// not of any door under test. It calls the real `permissions_do_not_bind` once and asserts the
/// carrier's line count moved by exactly what the return value promised: +1 if the arm skipped, +0
/// if it bound. The **delta** (not the carrier's absolute count) is what is asserted, so this
/// passes deterministically under either euid without knowing in advance which one it is running
/// as — the machine-readable skip signal `req/621` §3-2 asked for, demonstrated in-process rather
/// than only claimed.
#[cfg(unix)]
#[test]
fn skip_carrier_delta_matches_the_arm_outcome() {
    let dir = scratch("r41_skip_carrier_probe");
    let child = dir.join("child");
    std::fs::write(&child, b"x").expect("seed a file the arm will probe");
    let carrier = support::skip_carrier_path("r41");
    let before = support::carrier_line_count(&carrier);
    let skipped = permissions_do_not_bind(&dir, &child);
    let after = support::carrier_line_count(&carrier);
    println!("R41_SKIP_PROBE skipped={skipped} before={before} after={after}");
    assert_eq!(
        after.saturating_sub(before),
        if skipped { 1 } else { 0 },
        "a skip must add exactly one carrier line and a bound arm must add none \
         (skipped={skipped} before={before} after={after})"
    );
    set_mode(&dir, 0o700);
}

/// A project with a `.gx/`, and its `Layout`, captured while the tree is healthy — the door
/// functions take a `Layout`, and `Layout::open` itself refuses some of the shapes below (R40),
/// which is exactly why these sites, which do **not** pass through it, need their own answers.
fn project(name: &str) -> (PathBuf, Layout) {
    let root = scratch(name);
    let layout = Layout::create(&root).expect("create the project layout");
    (root, layout)
}

// ---------------------------------------------------------------------------------------------
// KA-1 — the downstream doors, asked directly (req/561 §5).
// ---------------------------------------------------------------------------------------------

#[test]
fn ka1_keypair_load_answers_err_for_a_directory() {
    let dir = scratch("r41_ka1_key_dir");
    let path = dir.join("k.key");
    std::fs::create_dir(&path).expect("a directory where the key file is expected");
    let out = KeyPair::load(&path);
    println!("R41_KA1 keypair_load dir observed={out:?}");
    assert!(out.is_err(), "KeyPair::load must refuse a directory");
}

#[cfg(unix)]
#[test]
fn ka1_keypair_load_answers_err_under_an_unreadable_parent() {
    let dir = scratch("r41_ka1_key_perm");
    let path = dir.join("k.key");
    std::fs::write(&path, b"not a key").expect("write");
    if permissions_do_not_bind(&dir, &path) {
        return;
    }
    let out = KeyPair::load(&path);
    set_mode(&dir, 0o700);
    println!("R41_KA1 keypair_load unreadable observed={out:?}");
    assert!(
        out.is_err(),
        "KeyPair::load must refuse when it cannot stat"
    );
}

#[test]
fn ka1_keypair_load_encrypted_answers_err_for_a_directory() {
    let dir = scratch("r41_ka1_enc_dir");
    let path = dir.join("k.key");
    std::fs::create_dir(&path).expect("a directory where the key file is expected");
    let out = KeyPair::load_encrypted(&path, "pw");
    println!("R41_KA1 keypair_load_encrypted dir observed={out:?}");
    assert!(
        out.is_err(),
        "KeyPair::load_encrypted must refuse a directory"
    );
}

#[cfg(unix)]
#[test]
fn ka1_keypair_load_encrypted_answers_err_under_an_unreadable_parent() {
    let dir = scratch("r41_ka1_enc_perm");
    let path = dir.join("k.key");
    std::fs::write(&path, b"not a key").expect("write");
    if permissions_do_not_bind(&dir, &path) {
        return;
    }
    let out = KeyPair::load_encrypted(&path, "pw");
    set_mode(&dir, 0o700);
    println!("R41_KA1 keypair_load_encrypted unreadable observed={out:?}");
    assert!(
        out.is_err(),
        "KeyPair::load_encrypted must refuse when it cannot stat"
    );
}

#[test]
fn ka1_ledger_store_answers_err_for_a_directory() {
    let dir = scratch("r41_ka1_ledger_dir");
    let path = dir.join("journal.ledger");
    std::fs::create_dir(&path).expect("a directory where the ledger file is expected");
    let out = LedgerStore::open_read_only(&path);
    println!(
        "R41_KA1 ledger_open_read_only dir observed={:?}",
        out.as_ref().err()
    );
    assert!(
        out.is_err(),
        "LedgerStore::open_read_only must refuse a directory"
    );
}

#[cfg(unix)]
#[test]
fn ka1_ledger_store_answers_err_under_an_unreadable_parent() {
    let dir = scratch("r41_ka1_ledger_perm");
    let path = dir.join("journal.ledger");
    std::fs::write(&path, b"").expect("write");
    if permissions_do_not_bind(&dir, &path) {
        return;
    }
    let out = LedgerStore::open_read_only(&path);
    set_mode(&dir, 0o700);
    println!(
        "R41_KA1 ledger_open_read_only unreadable observed={:?}",
        out.as_ref().err()
    );
    assert!(
        out.is_err(),
        "LedgerStore::open_read_only must refuse what it cannot open"
    );
}

#[test]
fn ka1_engine_journal_answers_err_for_a_directory() {
    let dir = scratch("r41_ka1_journal_dir");
    let path = dir.join("journal");
    std::fs::create_dir(&path).expect("a directory where the journal file is expected");
    let out = EngineJournal::open_read_only(&path);
    println!(
        "R41_KA1 journal_open_read_only dir observed={:?}",
        out.as_ref().err()
    );
    assert!(
        out.is_err(),
        "EngineJournal::open_read_only must refuse a directory"
    );
}

#[cfg(unix)]
#[test]
fn ka1_engine_journal_answers_err_under_an_unreadable_parent() {
    let dir = scratch("r41_ka1_journal_perm");
    let path = dir.join("journal");
    std::fs::write(&path, b"").expect("write");
    if permissions_do_not_bind(&dir, &path) {
        return;
    }
    let out = EngineJournal::open_read_only(&path);
    set_mode(&dir, 0o700);
    println!(
        "R41_KA1 journal_open_read_only unreadable observed={:?}",
        out.as_ref().err()
    );
    assert!(
        out.is_err(),
        "EngineJournal::open_read_only must refuse what it cannot open"
    );
}

#[test]
fn ka1_verdict_store_answers_err_for_a_directory() {
    let dir = scratch("r41_ka1_verdict_dir");
    let path = dir.join("journal.verdicts");
    std::fs::create_dir(&path).expect("a directory where the chain file is expected");
    let out = VerdictCheckpointStore::open_read_only(&path);
    println!(
        "R41_KA1 verdict_open_read_only dir observed={:?}",
        out.as_ref().err()
    );
    assert!(
        out.is_err(),
        "VerdictCheckpointStore::open_read_only must refuse a directory"
    );
}

#[cfg(unix)]
#[test]
fn ka1_verdict_store_answers_err_under_an_unreadable_parent() {
    let dir = scratch("r41_ka1_verdict_perm");
    let path = dir.join("journal.verdicts");
    std::fs::write(&path, b"").expect("write");
    if permissions_do_not_bind(&dir, &path) {
        return;
    }
    let out = VerdictCheckpointStore::open_read_only(&path);
    set_mode(&dir, 0o700);
    println!(
        "R41_KA1 verdict_open_read_only unreadable observed={:?}",
        out.as_ref().err()
    );
    assert!(
        out.is_err(),
        "VerdictCheckpointStore::open_read_only must refuse what it cannot open"
    );
}

// ---------------------------------------------------------------------------------------------
// S-1 / S-2 — `KeyStore::load` / `KeyStore::load_encrypted` (`keys.rs`).
// ---------------------------------------------------------------------------------------------

/// `(a)` — a key that is not there keeps today's word, on both loaders.
#[test]
fn s1_s2_an_absent_key_keeps_not_found() {
    let dir = scratch("r41_s1_absent");
    let store = KeyStore::at(&dir);
    let plain = store.load("ed25519-absent").unwrap_err();
    println!("R41_S1 (a) observed={plain:?}");
    assert!(
        matches!(&plain, Error::NotFound { what: "key", .. }),
        "an absent key is still NotFound: {plain:?}"
    );
    let enc = store.load_encrypted("ed25519-absent", "pw").unwrap_err();
    println!("R41_S2 (a) observed={enc:?}");
    assert!(
        matches!(&enc, Error::NotFound { what: "key", .. }),
        "an absent key is still NotFound on the encrypted road: {enc:?}"
    );
}

/// `(b)` — a directory bearing the key's file name is not an absent key, on both loaders.
#[test]
fn s1_s2_a_directory_at_the_key_path_is_not_called_absent() {
    let dir = scratch("r41_s1_dir");
    let store = KeyStore::at(&dir);
    let key_id = "ed25519-0000000000000000";
    std::fs::create_dir(store.path_of(key_id)).expect("a directory where the key is filed");
    let plain = store.load(key_id).unwrap_err();
    println!("R41_S1 (b) observed={plain:?}");
    assert!(
        !matches!(&plain, Error::NotFound { .. }),
        "a directory at the key path must not be answered as absence: {plain:?}"
    );
    let enc = store.load_encrypted(key_id, "pw").unwrap_err();
    println!("R41_S2 (b) observed={enc:?}");
    assert!(
        !matches!(&enc, Error::NotFound { .. }),
        "the encrypted road must not answer absence either: {enc:?}"
    );
}

/// `(c)` — a store this process may not `stat` into is not an absent key (req/559's flake shape;
/// req/561 AC-8), on both loaders.
#[cfg(unix)]
#[test]
fn s1_s2_an_unreadable_store_is_not_called_absent() {
    let dir = scratch("r41_s1_perm");
    let store = KeyStore::at(&dir);
    let key_id = "ed25519-0000000000000000";
    std::fs::write(store.path_of(key_id), b"present but unstatable").expect("write");
    if permissions_do_not_bind(&dir, &store.path_of(key_id)) {
        return;
    }
    let plain = store.load(key_id);
    let enc = store.load_encrypted(key_id, "pw");
    set_mode(&dir, 0o700);
    let plain = plain.unwrap_err();
    println!("R41_S1 (c) observed={plain:?}");
    assert!(
        !matches!(&plain, Error::NotFound { .. }),
        "a key this process may not stat is not an absent key: {plain:?}"
    );
    let enc = enc.unwrap_err();
    println!("R41_S2 (c) observed={enc:?}");
    assert!(
        !matches!(&enc, Error::NotFound { .. }),
        "the encrypted road must not call it absent either: {enc:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// S-3 — `ledger::open` (`ledger.rs`).
// ---------------------------------------------------------------------------------------------

/// `(a)` — a project that has never grown a ledger keeps today's word.
#[test]
fn s3_an_absent_ledger_keeps_not_found() {
    let (_root, layout) = project("r41_s3_absent");
    let out = ledger::open(&layout).map(|_| ()).unwrap_err();
    println!("R41_S3 (a) observed={out:?}");
    assert!(
        matches!(&out, Error::NotFound { what: "ledger", .. }),
        "an absent ledger is still NotFound: {out:?}"
    );
}

/// `(b)` — a directory standing at the ledger's path is not an absent ledger.
#[test]
fn s3_a_directory_at_the_ledger_path_is_not_called_absent() {
    let (_root, layout) = project("r41_s3_dir");
    std::fs::create_dir(layout.ledger_path()).expect("a directory where the ledger is declared");
    let out = ledger::open(&layout).map(|_| ()).unwrap_err();
    println!("R41_S3 (b) observed={out:?}");
    assert!(
        !matches!(&out, Error::NotFound { .. }),
        "a directory at the ledger path must not be answered as absence: {out:?}"
    );
}

/// `(c)` — a ledger this process may not `stat` is not an absent ledger.
#[cfg(unix)]
#[test]
fn s3_an_unreadable_parent_is_not_called_absent() {
    let (_root, layout) = project("r41_s3_perm");
    std::fs::write(layout.ledger_path(), b"").expect("write");
    let parent = layout.ledger_path().parent().expect("parent").to_path_buf();
    if permissions_do_not_bind(&parent, &layout.ledger_path()) {
        return;
    }
    let out = ledger::open(&layout).map(|_| ());
    set_mode(&parent, 0o700);
    let out = out.unwrap_err();
    println!("R41_S3 (c) observed={out:?}");
    assert!(
        !matches!(&out, Error::NotFound { .. }),
        "a ledger this process may not stat is not an absent ledger: {out:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// S-4 — `replay::open` (`replay.rs`).
// ---------------------------------------------------------------------------------------------

/// `(a)` — a journal that is not there keeps today's word. The `Layout` is captured while the
/// tree is healthy, because this door is exactly the one that does not pass `Layout::open` again.
#[test]
fn s4_an_absent_journal_keeps_not_found() {
    let (_root, layout) = project("r41_s4_absent");
    std::fs::remove_file(layout.journal_path()).expect("remove the journal");
    let out = replay::open(&layout).map(|_| ()).unwrap_err();
    println!("R41_S4 (a) observed={out:?}");
    assert!(
        matches!(
            &out,
            Error::NotFound {
                what: "journal",
                ..
            }
        ),
        "an absent journal is still NotFound: {out:?}"
    );
}

/// `(b)` — a directory standing at the journal's path is not an absent journal.
#[test]
fn s4_a_directory_at_the_journal_path_is_not_called_absent() {
    let (_root, layout) = project("r41_s4_dir");
    std::fs::remove_file(layout.journal_path()).expect("remove the journal");
    std::fs::create_dir(layout.journal_path()).expect("a directory where the journal is declared");
    let out = replay::open(&layout).map(|_| ()).unwrap_err();
    println!("R41_S4 (b) observed={out:?}");
    assert!(
        !matches!(&out, Error::NotFound { .. }),
        "a directory at the journal path must not be answered as absence: {out:?}"
    );
}

/// `(c)` — a journal this process may not `stat` is not an absent journal.
#[cfg(unix)]
#[test]
fn s4_an_unreadable_parent_is_not_called_absent() {
    let (_root, layout) = project("r41_s4_perm");
    let parent = layout
        .journal_path()
        .parent()
        .expect("parent")
        .to_path_buf();
    if permissions_do_not_bind(&parent, &layout.journal_path()) {
        return;
    }
    let out = replay::open(&layout).map(|_| ());
    set_mode(&parent, 0o700);
    let out = out.unwrap_err();
    println!("R41_S4 (c) observed={out:?}");
    assert!(
        !matches!(&out, Error::NotFound { .. }),
        "a journal this process may not stat is not an absent journal: {out:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// S-5 — `verdict::list_from_file` (`verdict.rs`). Absence is a **normal** answer here, and stays
// one: the chain is issued by hand, and "no chain yet" is an empty list rather than an error.
// ---------------------------------------------------------------------------------------------

/// `(a)` — no chain yet keeps today's empty `Ok`.
#[test]
fn s5_an_absent_chain_keeps_the_empty_answer() {
    let (_root, layout) = project("r41_s5_absent");
    let out = verdict::list_from_file(&layout).expect("no chain yet is a normal answer");
    println!("R41_S5 (a) observed={}", out.json);
    assert_eq!(out.json["count"], 0, "an absent chain is an empty chain");
    assert_eq!(out.code, 0, "and it answers 0, as it always has");
}

/// `(b)` — a directory standing at the chain's path is not "no chain yet".
#[test]
fn s5_a_directory_at_the_chain_path_is_not_an_empty_chain() {
    let (_root, layout) = project("r41_s5_dir");
    std::fs::create_dir(layout.verdict_log_path())
        .expect("a directory where the chain is declared");
    let out = verdict::list_from_file(&layout);
    println!("R41_S5 (b) observed={:?}", out.as_ref().err());
    assert!(
        out.is_err(),
        "a directory at the chain path must not be answered as an empty chain"
    );
    assert!(
        !matches!(out.unwrap_err(), Error::NotFound { .. }),
        "and it is not an absence either"
    );
}

// ---------------------------------------------------------------------------------------------
// S-6 — `report_without_engine`'s `ledger_present` / `verdict_chain_present` (`repair.rs`).
// Audit 40 F-1 (`req/563` §2, ruling `req/38` §333, scope `req/561` §11): the fold R40 removed
// from `journal_absent` stood two lines below R40's own fix, in the same function. A bool that
// could not be measured now answers `null` — the key stays, and neither `true` nor `false` is
// claimed about a path this process could not `stat`.
// ---------------------------------------------------------------------------------------------

/// bed-E — `.gx/ledger/` mode 0000 with both files sitting inside it. The report must not call
/// either file absent, and the honest sibling (`engine_open_failed`) still says why. The healthy
/// negative control on the same project answers `true`/`true` before and after.
#[cfg(unix)]
#[test]
fn s6_bed_e_an_unstatable_ledger_is_not_reported_absent() {
    let p = support::pipeline("r41_s6_bede", "before\n");
    p.commit_one("first");
    let ledger_dir = p.project.join(".gx").join("ledger");

    let healthy = run(p.gx().args(["repair", "--json"]));
    let report: serde_json::Value =
        serde_json::from_str(&healthy.stdout).expect("the healthy report is JSON");
    println!(
        "R41_S6 bedE healthy ledger_present={} verdict_chain_present={}",
        report["ledger_present"], report["verdict_chain_present"]
    );
    assert_eq!(
        report["ledger_present"], true,
        "the healthy project holds a ledger"
    );
    assert_eq!(report["verdict_chain_present"], true, "and a chain file");

    if permissions_do_not_bind(
        &ledger_dir,
        &p.project.join(".gx").join("ledger").join("journal"),
    ) {
        return;
    }
    let blind = run(p.gx().args(["repair", "--json"]));
    set_mode(&ledger_dir, 0o700);
    let report: serde_json::Value =
        serde_json::from_str(&blind.stdout).expect("the bed-E report is JSON");
    println!(
        "R41_S6 bedE blind rc={} ledger_present={} verdict_chain_present={} engine_open_failed={}",
        blind.code,
        report["ledger_present"],
        report["verdict_chain_present"],
        report["engine_open_failed"]
    );
    assert!(
        report["ledger_present"].is_null(),
        "a ledger this process may not stat is neither present nor absent: {}",
        report["ledger_present"]
    );
    assert!(
        report["verdict_chain_present"].is_null(),
        "and neither is the chain: {}",
        report["verdict_chain_present"]
    );
    assert!(
        report["engine_open_failed"].is_object(),
        "the honest sibling still names the refusal"
    );
    assert_eq!(
        blind.code, 1,
        "the exit is the one this bed already answered"
    );

    let after = run(p.gx().args(["repair", "--json"]));
    let report: serde_json::Value =
        serde_json::from_str(&after.stdout).expect("the restored report is JSON");
    assert_eq!(
        report["ledger_present"], true,
        "restored, the answer is true again"
    );
    assert_eq!(report["verdict_chain_present"], true, "on both keys");
}

/// bed-D — the journal deleted, both sibling files still there. `journal_absent` answers `true`
/// (that fact is established), and `ledger_present` keeps answering honestly about the file that
/// **is** there — absence of one file is not permission to stop measuring the others.
#[test]
fn s6_bed_d_a_lost_journal_does_not_unmeasure_its_siblings() {
    let p = support::pipeline("r41_s6_bedd", "before\n");
    p.commit_one("first");
    let journal = p.project.join(".gx").join("ledger").join("journal");
    std::fs::remove_file(&journal).expect("remove the journal");

    let out = run(p.gx().args(["repair", "--json"]));
    let report: serde_json::Value =
        serde_json::from_str(&out.stdout).expect("the bed-D report is JSON");
    println!(
        "R41_S6 bedD rc={} journal_absent={} ledger_present={} verdict_chain_present={}",
        out.code,
        report["journal_absent"],
        report["ledger_present"],
        report["verdict_chain_present"]
    );
    assert_eq!(
        report["journal_absent"], true,
        "the journal is established absent"
    );
    assert_eq!(
        report["ledger_present"], true,
        "the ledger is still there and still measured"
    );
    assert_eq!(
        report["verdict_chain_present"], true,
        "so is the chain file"
    );
}

/// `(c)` — a chain this process may not `stat` is not "no chain yet".
#[cfg(unix)]
#[test]
fn s5_an_unreadable_parent_is_not_an_empty_chain() {
    let (_root, layout) = project("r41_s5_perm");
    std::fs::write(layout.verdict_log_path(), b"").expect("write");
    let parent = layout
        .verdict_log_path()
        .parent()
        .expect("parent")
        .to_path_buf();
    if permissions_do_not_bind(&parent, &layout.verdict_log_path()) {
        return;
    }
    let out = verdict::list_from_file(&layout);
    set_mode(&parent, 0o700);
    println!("R41_S5 (c) observed={:?}", out.as_ref().err());
    assert!(
        out.is_err(),
        "a chain this process may not stat must not be answered as an empty chain"
    );
}
