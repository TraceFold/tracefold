// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The seven contracts of `SubstrateAdapter`, measured as written obligations. (sem:
//! SEM-gx-substrate-026, SEM-gx-substrate-027, SEM-gx-substrate-028, SEM-gx-substrate-029,
//! SEM-gx-substrate-030)
//!
//! req/69 §6.2 hand 2, verbatim (sem: SEM-gx-substrate-025): "**write each of the 7 methods'
//! contract text in one table** -- verbatim, especially 'which quantifier is idempotence' and
//! 'what is `plan` deterministic against'". This suite is what keeps that table a requirement rather
//! than a paragraph: `crates/gx-substrate/tests/substrate_contract.rs` does it for the locator
//! contract in the crate root (H-2), and this is the same instrument for the per-method contracts
//! in the trait documentation.
//!
//! # Why the quantifications are the load-bearing part
//!
//! req/69 §3.2 shows in three lines that reading 41 §4's "application is idempotent" and AC-049's
//! round trip as laws about a state map at once collapses every delta to the identity:
//!
//! ```text
//! ⟦δ⁻¹⟧∘⟦δ⟧ = id (total) and ⟦δ⟧∘⟦δ⟧ = ⟦δ⟧ (total) implies ⟦δ⟧ = id
//! ```
//!
//! **E-M4-3** closed that by quantifying each one narrowly -- idempotence over "the same delta
//! re-entering (retry)" and the round trip over "the one point of `pre` handed to `invert`" -- and
//! **E-M4-4** did the same for `plan`, whose determinism is "for the pair (intent, snapshot)" rather
//! than over an intent
//! alone. A contract table that states the obligations but not their quantifiers would hand hand 3's
//! conformance harness a property whose generator can falsify a correct adapter, which is M3-05's
//! defect one milestone later. So each row is checked for the words its own method needs, not merely
//! for being present.
//!
//! # What this suite deliberately does not check
//!
//! That an adapter obeys the contracts. There is no adapter: `gx-adapter-fs` is hand 4 and the
//! shared harness is hand 3 (req/69 §6.2). Behaviour is measured where behaviour exists.

use std::path::{Path, PathBuf};

/// The heading the table sits under, so that a second table in this documentation cannot be read as
/// this one.
const CONTRACT_HEADING: &str = "# The seven contracts";

/// What each row has to say, word for word, beyond naming its method.
///
/// The pairing is the point. A list of clauses checked against the whole table would pass if every
/// quantifier were written under the wrong method, and a table that says "the same delta
/// re-entering" about
/// `plan` is worse than one that says nothing, because it reads as a decision. (sem:
/// SEM-gx-substrate-053)
const REQUIRED_CLAUSES: [(&str, &[&str]); 7] = [
    ("kind", &["Every product's `substrate` is this same value"]),
    ("snapshot", &["The locator arrives already normalised"]),
    (
        "plan",
        &["for the pair (intent, snapshot)", "No side effects"],
    ),
    (
        "precondition",
        &[
            "defined only between products of the same adapter",
            "A different value before and after a change",
        ],
    ),
    ("apply", &["the same delta re-entering"]),
    (
        "invert",
        &[
            "the one point of `pre` handed to `invert`",
            "Exceeding the ceiling makes `Ok(None)`",
        ],
    ),
    (
        "commutation",
        &[
            "commutation(a,b) == commutation(b,a)",
            "`commutation(a,a)` is `Conflicts`",
        ],
    ),
];

/// Every ruling the table stands on, named in the documentation so a reader can follow a cell back
/// to the decision that put it there.
const CITED_RULINGS: [&str; 7] = [
    "E-M4-3",
    "E-M4-4",
    "E-M4-27",
    "M4-21",
    "M4-25",
    "N-08",
    "# Locator normalisation (normative)",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn adapter_rs() -> String {
    std::fs::read_to_string(repo_root().join("crates/gx-substrate/src/adapter.rs"))
        .expect("crates/gx-substrate/src/adapter.rs is readable")
}

/// The documentation of a source file, with the markers removed.
///
/// Both `//!` and `///`: the contract table is the trait's own documentation and the reasons around
/// it are the module's, and a reader on docs.rs meets both.
fn doc_text(source: &str) -> String {
    let mut out = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let body = trimmed
            .strip_prefix("//!")
            .or_else(|| trimmed.strip_prefix("///"));
        if let Some(body) = body {
            out.push_str(body.strip_prefix(' ').unwrap_or(body));
            out.push('\n');
        }
    }
    out
}

/// The rows of the table under [`CONTRACT_HEADING`], each one split into its cells.
///
/// The header row and the `|---|` rule are dropped; what comes back is the seven statements.
fn contract_rows(doc: &str) -> Vec<Vec<String>> {
    let after = doc
        .split(CONTRACT_HEADING)
        .nth(1)
        .unwrap_or_else(|| panic!("the trait documentation has no `{CONTRACT_HEADING}` section"));
    let section = after.split("\n# ").next().expect("the section ends");

    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells
            .iter()
            .all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue; // the rule under the header
        }
        rows.push(cells);
    }
    assert!(!rows.is_empty(), "the section holds no table at all");
    rows.remove(0); // the header row
    rows
}

/// `` `kind` `` -> `kind`.
fn row_method(row: &[String]) -> String {
    row.first()
        .expect("a row has cells")
        .trim_matches('`')
        .to_string()
}

// ---------------------------------------------------------------------------
// One row per method
// ---------------------------------------------------------------------------

/// The table has exactly seven rows and they name the seven methods the trait declares.
///
/// The names are taken from the source rather than from a constant, so a method added without a row
/// -- or a row written for a method that does not exist -- is a failure here and not a discrepancy
/// somebody notices later. `adapter_spec.rs` is what ties those seven names back to 41 §4.
#[test]
fn the_documentation_carries_one_row_per_method() {
    let source = adapter_rs();
    let rows = contract_rows(&doc_text(&source));
    let documented: Vec<String> = rows.iter().map(|r| row_method(r)).collect();

    let declared: Vec<String> = source
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("fn "))
        .map(|l| {
            l.strip_prefix("fn ")
                .expect("checked above")
                .split('(')
                .next()
                .expect("split yields one part")
                .to_string()
        })
        .collect();

    println!(
        "ADAPTER_CONTRACT_ROWS={} METHODS={}",
        rows.len(),
        declared.len()
    );
    assert_eq!(
        documented, declared,
        "the contract table and the trait no longer name the same seven methods"
    );
    assert_eq!(rows.len(), 7, "seven contracts, one per method (N-08)");
}

// ---------------------------------------------------------------------------
// Each row states its own quantification
// ---------------------------------------------------------------------------

/// Every row says the thing its own method needs said, word for word, **in its contract cell**.
///
/// The cell matters and the restriction was earned. Written against the whole row, this probe
/// survived mutation (f) of `tools/verify_m4h2.sh`: the `apply` row's contract was given `plan`'s
/// quantifier, and the words "the same delta re-entering" were still found -- in the third column,
/// where the
/// same row cites the ruling they come from. A check that a row *mentions* a phrase somewhere is
/// satisfied by the citation and says nothing about the sentence a reader would act on, which is the
/// B-3 shape (req/67 §2.1) in prose. The obligation is the contract cell; the third column is where
/// it is traced back to a ruling, and [`every_cited_ruling_is_named_where_the_contract_is_written`]
/// is what measures that half.
#[test]
fn each_row_states_the_quantification_its_method_needs() {
    let rows = contract_rows(&doc_text(&adapter_rs()));
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for (method, clauses) in REQUIRED_CLAUSES {
        let row = rows
            .iter()
            .find(|r| row_method(r) == method)
            .unwrap_or_else(|| panic!("the table has no row for `{method}`"));
        let text = row
            .get(1)
            .unwrap_or_else(|| panic!("the row for `{method}` has no contract cell"));
        for clause in clauses {
            checked += 1;
            if !text.contains(clause) {
                missing.push(format!("{method}: {clause}"));
            }
        }
    }

    println!(
        "ADAPTER_CONTRACT_CLAUSES={checked} MISSING={}",
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "the contract table does not state {missing:?} -- E-M4-3 and E-M4-4 are quantifications, \
         and a contract that drops them is the one req/69 §3.2 shows collapses every delta to the \
         identity"
    );
}

/// Each ruling the table rests on is named in the documentation.
#[test]
fn every_cited_ruling_is_named_where_the_contract_is_written() {
    let doc = doc_text(&adapter_rs());
    let mut missing: Vec<&str> = Vec::new();
    for ruling in CITED_RULINGS {
        if !doc.contains(ruling) {
            missing.push(ruling);
        }
    }
    assert!(
        missing.is_empty(),
        "the trait documentation does not cite {missing:?}; a contract whose reason cannot be \
         followed back is a house rule"
    );
}

/// No two rows say the same thing.
///
/// The **B-3** shape (req/67 §2.1) applied to prose: a table that names every method and repeats one
/// sentence passes a row count while carrying no contract at all. Distinctness is the cheapest
/// assertion that the seven rows are seven statements.
#[test]
fn no_two_rows_state_the_same_contract() {
    let rows = contract_rows(&doc_text(&adapter_rs()));
    let mut seen: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for row in &rows {
        let method = row_method(row);
        let body = row[1..].join(" | ");
        if let Some(other) = seen.insert(body.clone(), method.clone()) {
            panic!("`{method}` and `{other}` carry the same contract text: {body:?}");
        }
    }
    assert_eq!(seen.len(), rows.len());
}
