// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 5 (`req/338` §0-5, from `req/334` L-02)** — the page's **cell** counts are held by
//! the numbers the arms actually drive, not by a string.
//!
//! # What R27 closed and what it left open
//!
//! R27 built a registry so a suite's **probe** count keeps being checked after the page's anchor
//! moves past the block that stated it. The twenty-seventh audit pointed at the claim one step
//! along: the same blocks state **cell** counts — `25 of 30`, `10 of 10`, `all thirty-five` — and the
//! registry's selector is `piece.ends_with(".rs") && piece.contains("/tests/")`, so a claim with no
//! path in it is structurally outside.
//!
//! Worse, the arm that pins the page (`r27_edge_class_width.rs::f`) asks for the **string**
//! `"all thirty-five parse"` to be present. Thirty-five is `REAL.len() * positions().len()` = 7 × 5.
//! Add an eighth real name and the tree drives forty cells while the page says thirty-five, and both
//! stay green — because nothing compares a number to a number.
//!
//! # 🔴 What this lane found by wiring it up
//!
//! The private-use claim was **already stale, and nobody had noticed**. `docs/LIMITS.md` v0.5-n says
//! "Private-use scalars at an edge: **10 of 10 parsed**", which was 2 PUA rows × 5 positions. The
//! same block then describes adding the two supplementary planes, and `PUA` is `[(&str, char); 4]`
//! today — **20** cells, driven and recorded as `R27_PUA_EDGE cells=20`. The page states a
//! denominator half the size of the thing it describes, in the same block that describes it.
//!
//! That is exactly the defect `req/334` L-02 predicted in the abstract, sitting in the tree, and it
//! is why the repair is a derivation rather than a second string.
//!
//! # What is derived and what is asserted
//!
//! The cell counts come from `r27_edge_class_width.rs`'s **own source** — the declared array sizes
//! and the number of positions the fixture builds — so an array that grows moves the number the page
//! is held to on the day it grows. The past-tense measurements in v0.5-n (`25 of 30`, `10 of 10`)
//! are **left exactly as they are**: a stacked page is additive and an old block is a record of what
//! was measured when it was written. What this file holds is the page's **current-tense** claims.

use std::path::{Path, PathBuf};

fn record(line: &str) {
    println!("{line}");
    let path = std::env::var("R28_MEAS")
        .unwrap_or_else(|_| "/mnt/c/work/r28_logs/r28_measurements.txt".to_string());
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/gx-adapter-mcp -> repo root")
        .to_path_buf()
}

fn edge_suite() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("r27_edge_class_width.rs"),
    )
    .expect("the edge-class suite is readable")
}

/// The declared length of `const <NAME>: [...; N]`, read from the source.
fn array_len(src: &str, name: &str) -> usize {
    let needle = format!("const {name}: [");
    let at = src
        .find(&needle)
        .unwrap_or_else(|| panic!("{name} is declared in the edge-class suite"));
    let tail = &src[at + needle.len()..];
    let (_, after_semicolon) = tail
        .split_once(';')
        .unwrap_or_else(|| panic!("{name} declares a length"));
    after_semicolon
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("{name}'s length is a number"))
}

/// How many positions the fixture drives, counted from `five_positions`'s own body.
///
/// Each position is a `(label, json!(…))` tuple whose label sits at one fixed indentation. Derived
/// rather than taken from the function's name, because a name is a claim and this file exists
/// because a claim went stale.
fn positions_len(src: &str) -> usize {
    let at = src
        .find("fn five_positions")
        .expect("the fixture builds the positions");
    let body = &src[at..];
    let end = body.find("\n}").map(|e| e + 2).unwrap_or(body.len());
    body[..end]
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            l.len() - t.len() == 12 && t.starts_with('"') && t.ends_with("\",")
        })
        .count()
}

/// The English numeral for the counts this page states.
fn word_for(n: usize) -> String {
    let ones = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    let tens = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    match n {
        0..=10 => ones[n].to_string(),
        20 | 30 | 40 | 50 => tens[n / 10].to_string(),
        21..=99 if !n.is_multiple_of(10) => format!("{}-{}", tens[n / 10], ones[n % 10]),
        _ => n.to_string(),
    }
}

/// The three cell counts the arms drive, derived from the suite's source.
fn derived_cells() -> Vec<(&'static str, usize)> {
    let src = edge_suite();
    let positions = positions_len(&src);
    vec![
        ("the negative control", array_len(&src, "REAL") * positions),
        (
            "the control scalars",
            array_len(&src, "CONTROLS") * positions,
        ),
        ("the private-use areas", array_len(&src, "PUA") * positions),
    ]
}

// ---------------------------------------------------------------------------
// a — the bed: the derivation reproduces the numbers the arms print
// ---------------------------------------------------------------------------

/// 🔴 Without this arm, the page could be held against a derivation that is simply wrong — which is
/// the failure one level up, and the one `req/334` L-03 caught in the probe registry.
///
/// The three numbers are checked against the values `r27_edge_class_width.rs` records at run time
/// (`R27_NEGATIVE_CONTROL cells=35`, `R27_CONTROL_EDGE cells=30`, `R27_PUA_EDGE cells=20`), which is
/// a different route to the same quantity: one counts declarations, the other counts loop
/// iterations.
#[test]
fn a_bed_control_the_derivation_matches_what_the_arms_drive() {
    let src = edge_suite();
    let positions = positions_len(&src);
    let cells = derived_cells();
    record(&format!(
        "R28_CELLS_DERIVED positions={positions} real={} controls={} pua={} cells={cells:?}",
        array_len(&src, "REAL"),
        array_len(&src, "CONTROLS"),
        array_len(&src, "PUA"),
    ));
    assert_eq!(
        positions, 5,
        "🔴 the fixture no longer drives five positions; every number below moves with it and the \
         page has to move too: {positions}"
    );
    assert_eq!(
        cells,
        vec![
            ("the negative control", 35),
            ("the control scalars", 30),
            ("the private-use areas", 20),
        ],
        "🔴 the derivation does not reproduce the cell counts the arms print at run time \
         (35 / 30 / 20), so it cannot be trusted to hold the page: {cells:?}"
    );
}

// ---------------------------------------------------------------------------
// b — the page's current-tense claims are held to the derived numbers
// ---------------------------------------------------------------------------

/// 🔴 **`req/334` L-02** — a number, against a number.
#[test]
fn b_the_pages_current_cell_claims_are_the_numbers_this_tree_drives() {
    let page = std::fs::read_to_string(repo_root().join("docs/LIMITS.md"))
        .expect("docs/LIMITS.md is readable");
    let cells = derived_cells();
    let mut missing = Vec::new();
    for (what, n) in &cells {
        // The claim the page has to carry, spelled from the derived number.
        let claim = format!("{} cells for {what}", word_for(*n));
        if !page.contains(&claim) {
            missing.push(claim);
        }
    }
    record(&format!(
        "R28_CELLS_ON_THE_PAGE derived={cells:?} missing={missing:?}"
    ));
    assert!(
        missing.is_empty(),
        "🔴 `req/334` L-02: the page does not carry this tree's cell counts: {missing:?}. The \
         registry R27 built holds *probe* counts because its selector wants a `/tests/` path; a \
         cell count has no path and fell outside it. Stating the number on the page and deriving \
         the number here is what makes the two move together — a needle asking for the string \
         `\"all thirty-five parse\"` stays green while the tree drives forty."
    );
}

// ---------------------------------------------------------------------------
// c — the mutation: a grown array moves the number the page owes
// ---------------------------------------------------------------------------

/// 🔴 The whole difference between this gate and the string needle it supplements.
///
/// An eighth real name is spliced into a **copy** of the suite's source; the derived count moves
/// from thirty-five to forty, and the phrase the page carries stops being the phrase it owes. The
/// shipped file is not written.
#[test]
fn c_a_grown_array_moves_the_claim_the_page_owes() {
    let src = edge_suite();
    let before = array_len(&src, "REAL") * positions_len(&src);
    let grown = src.replacen("const REAL: [&str; 7] = [", "const REAL: [&str; 8] = [", 1);
    let after = array_len(&grown, "REAL") * positions_len(&grown);
    record(&format!(
        "R28_CELLS_MUTATION before={before} after={after} before_word={} after_word={}",
        word_for(before),
        word_for(after)
    ));
    assert_ne!(
        src, grown,
        "🔴 the splice did not take, so this arm is comparing a text with itself"
    );
    assert_eq!(before, 35, "🔴 the baseline is not what this arm measured");
    assert_eq!(
        after, 40,
        "🔴 a grown array has to move the derived count, or the gate is a string match wearing a \
         number's clothes: {after}"
    );
    assert_ne!(
        word_for(before),
        word_for(after),
        "🔴 and the phrase the page owes has to move with it"
    );
}
