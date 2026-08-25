// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The harness's **identity**, measured against 51 §7 and against the rulings that added to it. (sem:
//! SEM-gx-substrate-conformance-077, SEM-gx-substrate-conformance-078, SEM-gx-substrate-conformance-079,
//! SEM-gx-substrate-conformance-080, SEM-gx-substrate-conformance-081, SEM-gx-substrate-conformance-082,
//! SEM-gx-substrate-conformance-083, SEM-gx-substrate-conformance-084, SEM-gx-substrate-conformance-085,
//! SEM-gx-substrate-conformance-086, SEM-gx-substrate-conformance-087, SEM-gx-substrate-conformance-088,
//! SEM-gx-substrate-conformance-089)
//!
//! `req/38_ERRATA_2026-08-07.md` §30 M4H2-1, adopted (a), verbatim: "L6 (commutation symmetry +
//! reflexive = Conflicts) goes into **the harness's 'law' section**. Split the harness's identity
//! into **'contract 7 (1:1 with 51 §7)' + 'law n (from the L-series the rulings produced)'**, and
//! distinguish their origin in print (so as not to muddy the 1:1 correspondence's self-proof)".
//!
//! # Why the split needs a test rather than a convention
//!
//! 51 §7 ends with a completion condition: "no adapter satisfies the M4/M7 completion condition
//! unless it passes all seven of the above contracts". That sentence is only usable while "the above
//! seven contracts" and what this crate runs are the same
//! seven things. The moment a law is filed among them -- L6 is the one that asked, since 51 §7 has no
//! symmetry row -- the count that the DoD quotes stops meaning what 51 §7 wrote. So the seven are
//! read out of `req/spec/50-delivery/51-test-strategy.md` on every run and compared cell by cell,
//! and everything the rulings added lives in a second table that says where it came from.
//!
//! This is the `gate_input_spec.rs` / `adapter_spec.rs` instrument (B-4, then I-11) applied to a
//! table instead of to a struct or a trait.
//!
//! # What this file does not measure
//!
//! Whether an adapter passes anything. That is `contracts_seven.rs` and `laws.rs`, over the mock in
//! `tests/support`. This file is about what the harness *claims to be*.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<repo>/crates/gx-substrate-conformance`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The documentation of a source file, markers removed.
fn doc_text(source: &str) -> String {
    let mut out = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(body) = trimmed
            .strip_prefix("//!")
            .or_else(|| trimmed.strip_prefix("///"))
        {
            out.push_str(body.strip_prefix(' ').unwrap_or(body));
            out.push('\n');
        }
    }
    out
}

/// The rows of the first markdown table after `anchor`, header and rule dropped.
///
/// One reader for both sides -- the canon's markdown and this crate's documentation -- for the
/// reason `adapter_spec.rs` gives about signatures: a second parser is a second answer to what a row
/// is.
fn table_after(text: &str, anchor: &str) -> Vec<Vec<String>> {
    let after = text
        .split(anchor)
        .nth(1)
        .unwrap_or_else(|| panic!("the text has no anchor `{anchor}`"));
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut started = false;
    for line in after.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            if started {
                break; // the table ended
            }
            continue;
        }
        started = true;
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue; // the rule under the header
        }
        rows.push(cells);
    }
    assert!(!rows.is_empty(), "no table follows `{anchor}`");
    rows.remove(0); // the header row
    rows
}

// ---------------------------------------------------------------------------
// contract 7 -- 1:1 with 51 §7
// ---------------------------------------------------------------------------

/// The seven contract rows are 51 §7's seven, cell for cell.
///
/// Verbatim rather than "equivalent": 51 §7 is what the M4 and M7 completion conditions quote, so a
/// paraphrase here would make the DoD's "all seven contracts pass" refer to a list only this crate
/// holds.
#[test]
fn the_seven_contracts_are_51_7s_seven() {
    // This anchor string is kept in Japanese, byte-for-byte identical with the untouchable canon
    // file `req/spec/50-delivery/51-test-strategy.md` (sem: SEM-gx-substrate-conformance-058):
    // `.split(anchor)` below fails to find the table at all if this drifts from that file's text.
    let canon = table_after(
        &read("req/spec/50-delivery/51-test-strategy.md"),
        "契約テストの内容（41 §4 trait契約＋43準拠）:",
    );
    let mine = table_after(
        &doc_text(&read("crates/gx-substrate-conformance/src/contracts.rs")),
        "# The seven contracts of 51 §7",
    );

    println!(
        "CONFORMANCE_CONTRACTS_CANON={} CONFORMANCE_CONTRACTS_IMPL={}",
        canon.len(),
        mine.len()
    );
    assert_eq!(canon.len(), 7, "51 §7 no longer lists seven contracts");
    assert_eq!(
        mine, canon,
        "the harness's contract table and 51 §7's are not the same seven rows. 51 §7, verbatim: \
         \"no adapter satisfies the M4/M7 completion condition unless it passes all seven of the \
         above contracts\" -- \"the above seven contracts\" has to \
         be resolvable to survive a paraphrase"
    );
}

/// The harness declares the same seven ids in code as its table does in prose.
///
/// `CONTRACT_IDS` is what `run_contracts` labels its checks with, so this is the join between the
/// table above (which a reader trusts) and the report a caller reads.
#[test]
fn the_contract_ids_are_the_first_column_of_that_table() {
    let source = read("crates/gx-substrate-conformance/src/contracts.rs");
    let table = table_after(&doc_text(&source), "# The seven contracts of 51 §7");
    let from_table: Vec<String> = table.iter().map(|r| r[0].clone()).collect();

    let literal = source
        .split("pub const CONTRACT_IDS: [&str; 7] = [")
        .nth(1)
        .expect("contracts.rs declares `pub const CONTRACT_IDS: [&str; 7]`")
        .split_once("];")
        .expect("the array literal closes")
        .0;
    let declared: Vec<String> = literal
        .split(',')
        .filter_map(|cell| {
            let cell = cell.trim();
            cell.strip_prefix('"')
                .and_then(|c| c.strip_suffix('"'))
                .map(str::to_string)
        })
        .collect();

    assert_eq!(
        declared, from_table,
        "CONTRACT_IDS and the contract table disagree; the report would label a check with a name \
         51 §7 does not use"
    );
}

// ---------------------------------------------------------------------------
// law n -- the rulings' side, kept separate on purpose
// ---------------------------------------------------------------------------

/// The law table names every law, each with the ruling it comes from.
///
/// "distinguish the origin (contract or law) in print" (§30 M4H2-1 (a)) is only worth printing if
/// the second origin can be
/// traced: a law with no ruling behind it is a house rule wearing the harness's authority.
#[test]
fn every_law_names_the_ruling_that_created_it() {
    let doc = doc_text(&read("crates/gx-substrate-conformance/src/laws.rs"));
    let rows = table_after(&doc, "# The laws the rulings added");

    let ids: Vec<String> = rows.iter().map(|r| r[0].clone()).collect();
    println!("CONFORMANCE_LAWS={} ({ids:?})", rows.len());
    assert_eq!(
        ids,
        vec!["L1", "L2", "L3", "L4", "L5", "L6", "L7", "K1", "K2"],
        "the law list is req/69 §3.4's L1-L7 plus K1 (§30 M4H2-4: \"the products' substrate == \
         kind()\") plus \
         K2 (req/38 §58 R-4: the cross obligation whose absence let a real defect through the \
         other fifteen)"
    );

    let mut unsourced: Vec<String> = Vec::new();
    for row in &rows {
        let citation = row.last().expect("a row has cells");
        let cited = ["E-M4-", "M4-", "M4H2-", "51 §7", "AC-0"]
            .iter()
            .any(|marker| citation.contains(marker));
        if !cited {
            unsourced.push(row[0].clone());
        }
    }
    assert!(
        unsourced.is_empty(),
        "these laws cite no ruling: {unsourced:?}"
    );
}

/// The gap at L8 is explained rather than left as a missing number.
///
/// req/69 §3.4 lists eight laws and this table holds seven of them. L8 (opacity) is not a property of
/// an adapter at all -- it is a statement about the five crates *below* the boundary -- so running it
/// against a fixture would be measuring the wrong subject. "silently bounding" is the failure mode
/// the
/// harness's own doc rules out for contracts; a silently skipped law is the same thing one section
/// down.
#[test]
fn the_missing_l8_is_accounted_for() {
    let doc = doc_text(&read("crates/gx-substrate-conformance/src/laws.rs"));
    assert!(
        doc.contains("L8"),
        "the law table jumps from L7 to K1 and never says where L8 went (req/69 §3.4 lists eight)"
    );
    assert!(
        doc.contains("tests/opacity.rs"),
        "L8 is named but its measurement is not; a law with no instrument is a law nobody runs"
    );
}

// ---------------------------------------------------------------------------
// The crate's identity
// ---------------------------------------------------------------------------

/// The crate documentation states the two-section split and why it exists.
#[test]
fn the_crate_states_its_two_sections_and_their_reason() {
    let doc = doc_text(&read("crates/gx-substrate-conformance/src/lib.rs"));
    let mut missing: Vec<&str> = Vec::new();
    // "契約 7" / "法則 n" / the fifth clause below are kept in Japanese (sem:
    // SEM-gx-substrate-conformance-059): `src/lib.rs` keeps the matching headings in Japanese for
    // exactly this check.
    for clause in [
        "契約 7",
        "法則 n",
        "M4H2-1",
        "51 §7",
        "対応の無い契約は「無い」と印字",
    ] {
        if !doc.contains(clause) {
            missing.push(clause);
        }
    }
    assert!(
        missing.is_empty(),
        "the crate documentation does not state {missing:?}; the harness's identity is what keeps \
         its seven from drifting away from 51 §7's seven"
    );
}

/// **N-12**: the harness and `gx-conformance-gen` are different things, and the crate says so.
///
/// req/69 §1 N-12, verbatim: "do not build a differential-vector generator `gx-conformance-gen` ...
/// **it is a different thing from 51 §7's `gx-substrate-conformance` (the adapter contract
/// harness)**, and the doc states plainly that the names are confusingly similar".
/// One word apart, one milestone apart (M8), and different jobs.
#[test]
fn the_crate_distinguishes_itself_from_gx_conformance_gen() {
    let doc = doc_text(&read("crates/gx-substrate-conformance/src/lib.rs"));
    assert!(
        doc.contains("gx-conformance-gen") && doc.contains("N-12"),
        "the crate documentation does not distinguish itself from `gx-conformance-gen` (N-12); the \
         two names differ by one word and by one milestone"
    );
}

/// **E-M4-19**: the harness is not published.
#[test]
fn the_harness_is_not_a_published_crate() {
    let manifest = read("crates/gx-substrate-conformance/Cargo.toml");
    assert!(
        manifest.contains("publish = false"),
        "the harness declares no `publish = false`; E-M4-19, verbatim: \"conformance is not a \
         **publish target** (the §5 probe-bin precedent)\""
    );
}
