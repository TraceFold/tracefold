// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R43 / `req/578` §2/§3/§6, ruling `req/38` §350 items 1, 2 and 5** — three more mouths of
//! the fold R40 opened, on the roads R41 did not reach.
//!
//! # What R41 left standing
//!
//! `req/561`'s S-6 fixed `report_without_engine`'s `ledger_present` and `verdict_chain_present`.
//! Three siblings were left, each in a different function and each folding "I could not look" into
//! an answer about the world:
//!
//! * **S-7** (`repair.rs`, `repair_and_report`) — `verdict_chain_present` on the **healthy** road,
//!   the one an operator sees when the engine opened. Literally the same expression S-6 fixed, one
//!   function away (`req/578` §2).
//! * **S-8** (`repair.rs`, `report_without_engine`) — `head_recorded: head.is_some()`, where `head`
//!   came through `.ok().flatten()`. A head that will not read and a project that never recorded
//!   one both answered `false` (`req/578` §3).
//! * **S-9** (`layout.rs`, `declared_directories_are_directories`) — `if let Ok(found)` swallowed
//!   every `stat` failure, so a declared directory this process could not examine passed the check
//!   that exists to refuse the ones that are not directories (`req/578` §6).
//!
//! # 🔴 What the KA-1 bed actually measured (`req/578` §10, ruling §350 item 1)
//!
//! The bed the ruling named — "the `verdicts` file individually unreadable" — **is not
//! constructible by file mode**. `stat(2)` does not consult the file's own permission bits: a
//! `chmod 000` file is still `stat`-able and `Path::exists()` still answers `true` for it (measured
//! on this machine: `f_mode000 | stat OK | exists()=True`). Only the *directory* around it gates
//! the lookup, and making `.gx/ledger/` unreadable takes the engine down with it, which is bed-E
//! and is `report_without_engine`'s road, not this one.
//!
//! What **is** constructible on the healthy road, and is the same fold, is the shape this
//! repository has already written down twice: a **symbolic link** where the chain is declared.
//! `attach.rs::present` states the rule in words — "a symbolic link where a declared path belongs
//! is something that is **there**, whatever it points at, and an attach that called it absent would
//! go on to report having created a path it did not" — and `Path::exists()` follows the link, so it
//! answers `false` about a path that holds a link. `presence_of` asks `symlink_metadata`, which is
//! the question the door meant to ask.
//!
//! # What this suite does not claim
//!
//! That `true` is the *right* word for a link pointing at nothing. It is the word S-6 already fixed
//! this key to say for `Presence::Present(_)` (`req/561` §11), and R43 mints none.

use std::path::{Path, PathBuf};

#[path = "support/mod.rs"]
mod support;

use gx_cli::layout::Layout;
use support::run;

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
/// fold: this file and `r41_presence_unification.rs` carried ~90 duplicated lines of this
/// machinery). This wrapper is `#[track_caller]` so the printed site and the carrier line still
/// name whichever of *this file's* several call sites asked, not `support/mod.rs`.
#[cfg(unix)]
#[track_caller]
fn permissions_do_not_bind(parent: &Path, child: &Path) -> bool {
    support::permissions_do_not_bind("r43", parent, child)
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
    let dir = scratch("r43_skip_carrier_probe");
    let child = dir.join("child");
    std::fs::write(&child, b"x").expect("seed a file the arm will probe");
    let carrier = support::skip_carrier_path("r43");
    let before = support::carrier_line_count(&carrier);
    let skipped = permissions_do_not_bind(&dir, &child);
    let after = support::carrier_line_count(&carrier);
    println!("R43_SKIP_PROBE skipped={skipped} before={before} after={after}");
    assert_eq!(
        after.saturating_sub(before),
        if skipped { 1 } else { 0 },
        "a skip must add exactly one carrier line and a bound arm must add none \
         (skipped={skipped} before={before} after={after})"
    );
    set_mode(&dir, 0o700);
}

fn report(out: &support::Run) -> serde_json::Value {
    serde_json::from_str(&out.stdout).expect("the repair report is JSON")
}

// ---------------------------------------------------------------------------------------------
// S-7 — `repair_and_report`'s `verdict_chain_present` (the healthy road).
// ---------------------------------------------------------------------------------------------

/// bed-L — the chain replaced by a symbolic link that resolves to nothing. The engine still opens,
/// so this is `repair_and_report` and not `report_without_engine`, and the key must not say the
/// chain is absent about a path that holds a link.
#[cfg(unix)]
#[test]
fn s7_a_symlinked_chain_on_the_healthy_road_is_not_called_absent() {
    let p = support::pipeline("r43_s7_link", "before\n");
    p.commit_one("first");
    let chain = p
        .project
        .join(".gx")
        .join("ledger")
        .join("journal.verdicts");

    let healthy = run(p.gx().args(["repair", "--json"]));
    let before = report(&healthy);
    println!(
        "R43_S7 healthy rc={} verdict_chain_present={} engine_open_failed={}",
        healthy.code, before["verdict_chain_present"], before["engine_open_failed"]
    );
    assert_eq!(
        before["verdict_chain_present"], true,
        "the healthy project holds a chain file"
    );

    std::fs::remove_file(&chain).expect("remove the chain file");
    std::os::unix::fs::symlink(p.project.join("no-such-chain"), &chain)
        .expect("a symbolic link where the chain is declared");

    let blind = run(p.gx().args(["repair", "--json"]));
    let after = report(&blind);
    println!(
        "R43_S7 bedL rc={} verdict_chain_present={} engine_open_failed={} ledger_present={}",
        blind.code,
        after["verdict_chain_present"],
        after["engine_open_failed"],
        after["ledger_present"]
    );
    assert!(
        after["engine_open_failed"].is_null(),
        "this bed is the healthy road: the engine must still have opened ({})",
        after["engine_open_failed"]
    );
    assert_eq!(
        blind.code, healthy.code,
        "the exit this bed answers does not move (req/38 §148)"
    );
    assert_ne!(
        after["verdict_chain_present"], false,
        "a path holding a symbolic link is not an absent chain"
    );
}

// ---------------------------------------------------------------------------------------------
// S-8 — `report_without_engine`'s `head_recorded`.
// ---------------------------------------------------------------------------------------------

/// bed-M — a head that will not parse, on bed-E's road (`.gx/ledger/` unreadable, so the engine
/// does not open). `HeadStore::read` answers `Err(Malformed)` — a refusal and not an absence, in
/// its own words — and `.ok().flatten()` turned that into "this project records no head".
///
/// The exit is asserted against the same bed with a healthy head: `witnessed` is computed from the
/// value this fix does **not** touch, so the code must not move (ruling §350 item 2).
#[cfg(unix)]
#[test]
fn s8_an_unreadable_head_is_not_reported_as_no_head() {
    let p = support::pipeline("r43_s8_head", "before\n");
    p.commit_one("first");
    let ledger_dir = p.project.join(".gx").join("ledger");
    let head = p.project.join(".gx").join("checkpoints").join("head.json");
    assert!(
        head.exists(),
        "the fixture records a head at {}",
        head.display()
    );

    if permissions_do_not_bind(&ledger_dir, &ledger_dir.join("journal")) {
        return;
    }
    let control = run(p.gx().args(["repair", "--json"]));
    set_mode(&ledger_dir, 0o700);
    let control_report = report(&control);
    println!(
        "R43_S8 control rc={} head_recorded={} engine_open_failed_is_object={}",
        control.code,
        control_report["head_recorded"],
        control_report["engine_open_failed"].is_object()
    );
    assert!(
        control_report["engine_open_failed"].is_object(),
        "the control is bed-E's road: the engine did not open"
    );
    assert_eq!(
        control_report["head_recorded"], true,
        "with a readable head, this project records one"
    );

    let good = std::fs::read(&head).expect("keep the head");
    std::fs::write(&head, b"{ this is not a head }").expect("a head that will not parse");
    set_mode(&ledger_dir, 0o000);
    let blind = run(p.gx().args(["repair", "--json"]));
    set_mode(&ledger_dir, 0o700);
    let blind_report = report(&blind);
    println!(
        "R43_S8 bedM rc={} head_recorded={} head_authenticity={}",
        blind.code, blind_report["head_recorded"], blind_report["head_authenticity"]
    );
    assert_ne!(
        blind_report["head_recorded"], false,
        "a head this process could not read is not a project that records none"
    );
    assert_eq!(
        blind.code, control.code,
        "the exit does not move: `witnessed` is not computed from the reported key \
         (req/38 §148, ruling §350 item 2)"
    );

    std::fs::write(&head, good).expect("restore the head");
    let after = run(p.gx().args(["repair", "--json"]));
    assert_eq!(
        report(&after)["head_recorded"],
        true,
        "restored, the answer is true again"
    );
}

// ---------------------------------------------------------------------------------------------
// S-9 — `Layout::declared_directories_are_directories`.
// ---------------------------------------------------------------------------------------------

/// bed-N — `.gx/` itself unreadable, so every declared row underneath it is a `stat` this process
/// cannot make. `if let Ok(found)` passed all eleven rows silently; the door has to say it could
/// not look, in the word `layout.rs` already uses one function above for the journal
/// (`Error::Io { action: "read the shape of", .. }`) — no word is minted here.
#[cfg(unix)]
#[test]
fn s9_a_declared_directory_that_cannot_be_stated_is_not_passed() {
    let root = scratch("r43_s9_blind");
    Layout::create(&root).expect("create the project layout");
    let gx_dir = root.join(".gx");
    if permissions_do_not_bind(&gx_dir, &gx_dir.join("ledger")) {
        return;
    }
    let out = Layout::open(&root).map(|_| ());
    set_mode(&gx_dir, 0o700);
    let err = out.expect_err("an unreadable `.gx/` is not an openable project");
    let text = format!("{err:?}");
    println!("R43_S9 bedN observed={text}");
    assert!(
        text.contains("read the shape of"),
        "the refusal is the word this file already uses for a shape it could not read: {text}"
    );
    // 🔴 The discriminating half. Before the fix this door passed all seven rows and the refusal
    // came from the **journal** check further in — about `.gx/ledger/journal`, a path this
    // function is not even asked about. The first door has to be the one that speaks.
    assert!(
        text.contains("/.gx/ledger\"") || text.contains("\\.gx\\ledger\""),
        "the door asked first must name the declared directory it could not examine, not a path \
         a later check happened to reach: {text}"
    );
}

// ---------------------------------------------------------------------------------------------
// Probe — `one_dir_state` (`repair.rs:2628`), the third `is_dir()` coordinate.
// ---------------------------------------------------------------------------------------------

/// Not an assertion about the fold: a measurement of whether any bed **reaches** it. The path
/// examined is `<root>/.gx/<rel>`, so `Presence::Undetermined` there needs `.gx/` itself
/// unstatable — the state S-9's door now refuses on the way in. This arm prints what a repair over
/// bed-N does, and is the evidence for the per-coordinate verdict `req/38` §350 item 5 allows.
#[cfg(unix)]
#[test]
fn probe_one_dir_state_reachability_under_an_unreadable_gx() {
    let p = support::pipeline("r43_probe_dirstate", "before\n");
    p.commit_one("first");
    let gx_dir = p.project.join(".gx");

    // req/635 box L-4 (req/38 §394) — positive control. Before any permission is touched, a
    // healthy `.gx/` must produce a non-empty repair report. Without this arm, the two
    // `stdout.is_empty()` asserts below (mode 0000, mode 0400) are indistinguishable from a probe
    // that always sees empty stdout regardless of what it examines — the discriminating half was
    // missing (`req/621` §5-0-2's positive-control gap, same shape).
    let healthy = run(p.gx().args(["repair", "--json"]));
    println!(
        "R43_PROBE one_dir_state healthy rc={} stdout_len={}",
        healthy.code,
        healthy.stdout.len()
    );
    assert!(
        !healthy.stdout.is_empty(),
        "a healthy `.gx/` must produce a non-empty repair report, or the emptiness asserted below \
         proves nothing"
    );

    if permissions_do_not_bind(&gx_dir, &gx_dir.join("ledger")) {
        return;
    }
    let out = run(p.gx().args(["repair", "--json"]));
    // `r--------`: `read_dir` succeeds and `stat` on the children does not, which is the only mode
    // that separates the two. It is the shape `attach.rs::walk`'s fold needs as well, and it is
    // measured here so the addendum's per-coordinate verdict rests on a run rather than on
    // reading.
    set_mode(&gx_dir, 0o400);
    let readable_not_searchable = run(p.gx().args(["repair", "--json"]));
    set_mode(&gx_dir, 0o700);
    println!(
        "R43_PROBE one_dir_state mode0000 rc={} stdout_len={} stderr={}",
        out.code,
        out.stdout.len(),
        out.stderr.trim()
    );
    println!(
        "R43_PROBE one_dir_state mode0400 rc={} stdout_len={} stderr={}",
        readable_not_searchable.code,
        readable_not_searchable.stdout.len(),
        readable_not_searchable.stderr.trim()
    );
    assert!(
        out.stdout.is_empty() && readable_not_searchable.stdout.is_empty(),
        "no repair report is produced at all under either shape, so `repair_dir_state` — and \
         `one_dir_state`'s `symlink_metadata(..).ok()?` inside it — is not reached: the door \
         refuses first. This is the measurement behind leaving `repair.rs`'s third `is_dir()` \
         coordinate unrepaired (ruling req/38 §350 item 5, per-coordinate)"
    );
}
