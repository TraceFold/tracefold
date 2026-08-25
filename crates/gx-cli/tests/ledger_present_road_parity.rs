// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/654` M-1 fast-follow, ruling `req/38` §394** — `ledger_present` is one wire key with
//! two producers, and the two must answer the same question.
//!
//! `gx repair --json` writes `"ledger_present"` on two roads:
//!
//! * `repair.rs:1798`, in `repair_and_report`, on the road where the engine opened. Before this
//!   lane it read `engine.ledger().present()`, which is `LedgerStore::present()` =
//!   `self.file.is_some()`, and `self.file` is `None` whenever `LedgerStore::open_read_only_or_absent`
//!   found `path.exists()` false — a call that **follows** the final symbolic link, so a dangling
//!   link where the ledger is declared reads `false`.
//! * `repair.rs:2357`, in `report_without_engine`, on the road where the engine could not open. R45
//!   (`req/621` M-1) unified it to `crate::layout::presence_of` (`symlink_metadata`, `lstat`), so a
//!   dangling link reads `true` — a declared path holding a link is something that is there
//!   (`attach.rs::present`), whatever it points at.
//!
//! Same physical shape — a dangling symbolic link at `.gx/ledger/journal.ledger` — but the answer
//! flipped on which road the run took, and which road it took turned on an unrelated fact (is the
//! journal present). `req/650`'s independent verify measured the split side by side: the with-engine
//! road answered `ledger_present: false`, the without-engine road `true`. `req/38` §394 already ruled
//! the meaning of this key — "something is at this path", one key, no shape split (YAGNI) — so the
//! two roads must carry that one meaning. This file reaches both roads on the identical shape and
//! pins them equal.

use std::path::{Path, PathBuf};

#[path = "support/mod.rs"]
mod support;

use support::run;

fn report(out: &support::Run) -> serde_json::Value {
    serde_json::from_str(&out.stdout).expect("the repair report is JSON on stdout")
}

/// A dangling symbolic link where a declared file belongs — the shape both roads must agree about.
#[cfg(unix)]
fn dangling_symlink(at: &Path) {
    if at.exists() || at.symlink_metadata().is_ok() {
        std::fs::remove_file(at).expect("remove the file before linking over it");
    }
    std::os::unix::fs::symlink(at.with_file_name("no-such-target"), at)
        .expect("a symbolic link that resolves to nothing");
}

/// The ledger path beside a fresh single-commit project.
fn project_and_ledger(name: &str) -> (support::Pipeline, PathBuf) {
    let p = support::pipeline(name, "before\n");
    p.commit_one("first");
    let ledger = p.project.join(".gx").join("ledger").join("journal.ledger");
    (p, ledger)
}

/// Delete the journal so `gx repair --json` cannot open the engine and takes the
/// `report_without_engine` road (road B). Hand back the same ledger path beside it.
fn without_engine_project(name: &str) -> (support::Pipeline, PathBuf) {
    let (p, ledger) = project_and_ledger(name);
    let journal = p.project.join(".gx").join("ledger").join("journal");
    std::fs::remove_file(&journal).expect("remove the journal to force the without-engine road");
    (p, ledger)
}

// ---------------------------------------------------------------------------------------------
// The two roads, on the identical dangling-link shape, must answer the same `ledger_present`.
// ---------------------------------------------------------------------------------------------

/// The box's primary target (`req/654` §2 KA-1): the with-engine road (`repair_and_report`,
/// `repair.rs:1798`) and the without-engine road (`report_without_engine`, `repair.rs:2357`) put
/// the same key on the wire, and a monitor reading it cannot know which producer it came from.
/// Before this lane they diverged on a dangling link (road A `false`, road B `true`, `req/650`); the
/// key is a lie the moment two producers under one name disagree about one file.
#[cfg(unix)]
#[test]
fn ledger_present_is_road_independent_on_a_dangling_link() {
    // Road A — the journal is intact, so the engine opens and `repair_and_report` produces the key.
    let (a, a_ledger) = project_and_ledger("req654_parity_road_a");
    dangling_symlink(&a_ledger);
    let a_out = run(a.gx().args(["repair", "--json"]));
    let road_a = report(&a_out);
    assert_eq!(
        road_a["journal_absent"],
        serde_json::Value::Bool(false),
        "road A must be the with-engine road (the journal is intact), so `journal_absent` is false: {}",
        road_a["journal_absent"]
    );

    // Road B — the journal is gone, so the engine cannot open and `report_without_engine` produces
    // the key, over the identical dangling-link ledger.
    let (b, b_ledger) = without_engine_project("req654_parity_road_b");
    dangling_symlink(&b_ledger);
    let b_out = run(b.gx().args(["repair", "--json"]));
    let road_b = report(&b_out);
    assert_eq!(
        road_b["journal_absent"],
        serde_json::Value::Bool(true),
        "road B must be the without-engine road (the journal is gone), so `journal_absent` is true: {}",
        road_b["journal_absent"]
    );

    println!(
        "REQ654 road_a(ledger_present)={} road_b(ledger_present)={}",
        road_a["ledger_present"], road_b["ledger_present"]
    );
    assert_eq!(
        road_a["ledger_present"], road_b["ledger_present"],
        "the same physical shape (a dangling symbolic link where the ledger is declared) must give \
         the same `ledger_present` on both producers of the key; road A={}, road B={}",
        road_a["ledger_present"], road_b["ledger_present"]
    );
    assert_eq!(
        road_a["ledger_present"],
        serde_json::Value::Bool(true),
        "and the shared answer is `true`: `presence_of` sees the link (attach.rs::present, req/38 §394)"
    );
}

/// Discrimination control for road A (the one this lane converts): the key still tells the three
/// shapes apart. A real ledger file is `true`, a genuinely absent one is `false`; only the dangling
/// link moves. Without this control the fix could pass by answering `true` unconditionally.
#[cfg(unix)]
#[test]
fn road_a_ledger_present_still_discriminates_the_three_shapes() {
    let (p, ledger) = project_and_ledger("req654_road_a_shapes");

    // A real file is present.
    let healthy = report(&run(p.gx().args(["repair", "--json"])));
    assert_eq!(
        healthy["journal_absent"],
        serde_json::Value::Bool(false),
        "the with-engine road (the journal is intact)"
    );
    assert_eq!(
        healthy["ledger_present"],
        serde_json::Value::Bool(true),
        "a real ledger file is present"
    );

    // A dangling link is present (the shape this lane fixes on road A).
    dangling_symlink(&ledger);
    let linked = report(&run(p.gx().args(["repair", "--json"])));
    println!(
        "REQ654 road_a link ledger_present={}",
        linked["ledger_present"]
    );
    assert_eq!(
        linked["ledger_present"],
        serde_json::Value::Bool(true),
        "a declared path holding a symbolic link is something that is there (req/38 §394)"
    );

    // A genuinely absent ledger is, and only is, `false` — the `serve_runtime_r6::m02` invariant,
    // asserted here on the converted road so the conversion cannot quietly lose it.
    std::fs::remove_file(&ledger).expect("genuinely delete the ledger link");
    let gone = report(&run(p.gx().args(["repair", "--json"])));
    println!(
        "REQ654 road_a absent ledger_present={}",
        gone["ledger_present"]
    );
    assert_eq!(
        gone["ledger_present"],
        serde_json::Value::Bool(false),
        "a genuinely absent ledger is still, and only, `false` (presence_of's Absent arm)"
    );
}
