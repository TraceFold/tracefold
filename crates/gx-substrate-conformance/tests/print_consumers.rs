//! 🔴 **L-5 / 裁定 #17** — does `Report::print` have a consumer? Answered once, here (**M7 hand 4**).
//!
//! The ticket is four hands old and its shape is this project's standard one: an item exists,
//! nothing outside a test calls it, and 「a later hand will use it」 is not an answer anybody is
//! allowed to give twice.
//!
//! | window | what it said | source |
//! |---|---|---|
//! | M4 fix | the mutation `Report::print → ()` survives, because nobody asserts stdout | req/77 §L-5 |
//! | M4 fix | two ways out: (a) classify it a permanent survivor, (b) add `to_lines()` so printing becomes measurable — 「(b) は M5 の CLI/HTTP 面が print を要求した時に一緒に決めるのが安い」 | req/77 §L-5 |
//! | M5 | not decided: M5 builds no CLI (N-01) | req/78 §5 row 23 |
//! | M6 | **measured**: no consumer grew. Two options raised — retirement mark, or 「M7 の adapter 2 本が conformance harness を実走する時が最後の窓」. Recommended (b), 「M7 に実消費者候補が実在するため。M6 で殺すのは早い」 | req/88 §5 row 35 |
//! | M6 检收 | 採(b): 「`Report::print` は M7(adapter 2 本が conformance harness を実走する時)が最後の窓。M6 で殺すのは早い」 | req/38 §46 (§5 行 35) |
//! | M7 reqdef | 「消費者が生えたか否かを **1 度だけ**判定し、生えなければ退役印(E-7 の形)を打つ」 | req/98 §3-4 row 8 / 裁定 #17 |
//!
//! # The answer, and the number it rests on
//!
//! **The consumer grew, so no retirement mark.** The candidate M6 named — the adapters running the
//! harness for real — is no longer a candidate: `gx-adapter-fs` (M4 hand 6), `gx-adapter-git` (M7
//! hand 1) and `gx-adapter-mcp` (M7 hand 3) each call it, and every conformance number quoted in
//! req/99, req/100 and req/101 (`CHECKS=16 PASS=16 FAIL=0 無い=0 …`) is a line this function printed.
//! An instrument whose output is the evidence three reports are written from is not an unused item.
//!
//! [`the_consumers_of_report_print_are_counted_rather_than_remembered`] is that judgement as a
//! measurement, and it is armed the other way as well: it requires **every adapter crate** to be
//! among the callers, so a hand that quietly stopped printing conformance reports re-opens the
//! question in red instead of leaving a dead function behind.
//!
//! # What is **not** resolved, and is not being re-decided here
//!
//! L-5's *other* half: the mutation `Report::print → ()` is still a survivor, because asserting
//! stdout needs a capture that collides with libtest's. req/38 §36 already classified it — 「L-5 記録:
//! `Report::print` survivor=印字は assert 対象外(等価変異扱い)」 — and this hand leaves that
//! classification exactly where it is. The two questions were fused in the ticket and they are
//! different: 「is anybody calling it」 is answered above with a number, and 「can a mutation of it be
//! caught」 is answered by the equivalence class it was put in. Option (b) of req/77 (a `to_lines()`
//! the suites assert on) would move the measurable content out of `print` without making `print`
//! itself measurable — the survivor would still be there, one function along — so it buys a new
//! published API and no property. Not taken; recorded so the next reader does not re-derive it.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-substrate-conformance")
        .to_path_buf()
}

/// Every `.rs` file under `crates/` and `probes/`, as (repo-relative path, text).
fn rust_sources() -> Vec<(String, String)> {
    let root = repo_root();
    let mut found = Vec::new();
    let mut stack: Vec<(PathBuf, String)> = vec![
        (root.join("crates"), "crates".to_string()),
        (root.join("probes"), "probes".to_string()),
    ];
    while let Some((dir, prefix)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = format!("{prefix}/{name}");
            let path = entry.path();
            if path.is_dir() {
                if name != "target" {
                    stack.push((path, rel));
                }
            } else if name.ends_with(".rs") {
                if let Ok(text) = fs::read_to_string(&path) {
                    found.push((rel, text));
                }
            }
        }
    }
    found.sort();
    found
}

/// Call sites of `Report::print`, as (file, line), on code lines only.
///
/// Derived by walking the tree rather than by naming the files, 裁定 #10's reason one crate over: a
/// declared list of consumers is a list that agrees with itself.
///
/// 🔴 **The instrument's limit, printed rather than implied**: it matches `report.print(`, the
/// binding every call site in this workspace uses, because `.print(` alone also catches
/// `gx-cli`'s `e.print()` on an error type — a different function with the same name. So a caller
/// who bound the report to some other name would be invisible here, and the failure direction is
/// **under**-counting, which is the safe one: this test's job is to notice when the count reaches
/// zero, and an under-count reaches zero first.
fn call_sites() -> Vec<(String, usize)> {
    let mut sites = Vec::new();
    for (file, text) in rust_sources() {
        // Neither the definition nor this probe is a call site.
        if file == "crates/gx-substrate-conformance/src/lib.rs"
            || file == "crates/gx-substrate-conformance/tests/print_consumers.rs"
        {
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || code.starts_with("///") {
                continue;
            }
            if code.contains("report.print(") {
                sites.push((file.clone(), i + 1));
            }
        }
    }
    sites
}

/// 🔴 **裁定 #17, answered once**: the consumers are counted, and every adapter is among them.
#[test]
fn the_consumers_of_report_print_are_counted_rather_than_remembered() {
    let sites = call_sites();
    let files: Vec<&String> = {
        let mut f: Vec<&String> = sites.iter().map(|(file, _)| file).collect();
        f.dedup();
        f
    };
    println!(
        "REPORT_PRINT_CONSUMERS sites={} files={files:?}",
        sites.len()
    );
    assert!(
        !sites.is_empty(),
        "裁定 #17: M7 is the last window. Zero call sites is the answer that fires the retirement \
         mark (E-7's form: the item stays, carrying a 退役 doc that says what it was), and it has to \
         be taken deliberately rather than by this test being deleted"
    );
    for adapter in [
        "crates/gx-adapter-fs/tests/",
        "crates/gx-adapter-git/tests/",
        "crates/gx-adapter-mcp/tests/",
    ] {
        assert!(
            files.iter().any(|f| f.starts_with(adapter)),
            "{adapter} runs the harness of 51 §7 and does not print its report. That is the exact \
             condition req/88 §5 row 35 measured in M6 (「消費者は生えない」) and req/38 §46 deferred \
             to M7 — if it is true again, the question is open again"
        );
    }
}

/// The judgement is written where a reader of the code lands, not only in a report.
///
/// `crates/gx-cli/src/consumers.rs` is the shape: a decision about an item belongs beside the item,
/// because a decision that lives only in `req/` is a decision the next hand re-derives.
#[test]
fn the_decision_is_recorded_beside_the_function() {
    let text = fs::read_to_string(repo_root().join("crates/gx-substrate-conformance/src/lib.rs"))
        .expect("the crate this test is about");
    for clause in ["裁定 #17", "L-5", "退役印", "M7 hand 4"] {
        assert!(
            text.contains(clause),
            "the consumer judgement must name {clause:?} beside `Report::print`"
        );
    }
}
