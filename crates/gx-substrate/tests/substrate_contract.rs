// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **H-2** / **E-M4-12** — the locator-normalisation contract, measured where a later hand has to
//! meet it. (sem: SEM-gx-substrate-121, SEM-gx-substrate-122, SEM-gx-substrate-123,
//! SEM-gx-substrate-124, SEM-gx-substrate-125)
//!
//! `req/38_ERRATA_2026-08-07.md` §25, verbatim: "🔴**H-2 (adopted = both option c)**: ① reserves the
//! mandatory ticket of the M4 adapter contract, 'the locator is passed to the gate already
//! normalised'". §28 M4-12 then fixed what the contract says: "put locator normalisation in the
//! contract as **lexical only** -- dot-segment folding, duplicate-separator removal, the
//! trailing-separator convention. Zero I/O = does not taint `plan`'s purity or the CAS. **File
//! symlink/realpath resolution as a v0.2+ ticket, and make the TH-2 residue (the symlink road is not
//! closed lexically) explicit in both the doc and the receipt-side disclosure**".
//!
//! # Why a documentation obligation gets a test
//!
//! The contract is prose by construction: 41 §4's trait has no normalisation method, and M4-12 (a)
//! rules the normalisation lexical precisely so that it stays inside the adapter rather than
//! becoming a road of its own. Prose nobody checks is a promise, and the M3 precedent for turning
//! one into a measurement is `crates/gx-gate/tests/pack_embedding.rs`'s
//! `the_pack_module_states_its_effective_range`, which asserts the clauses M3-10 named by name.
//! This is that shape one crate over.
//!
//! What it buys is not literary. Hand 4 writes the fs normalisation and hand 2 writes the trait
//! doc that points at it; both are graded against the clauses below, so "that a later hand
//! complies" is a
//! failing test rather than a reviewer's memory.
//!
//! # What it deliberately does not check
//!
//! That the *implementation* normalises. There is none yet -- L7 (idempotence and the equivalence
//! map, req/69 §3.4) is hand 4's property, over `gx-adapter-fs`. A clause check that pretended to
//! measure behaviour would be the worse half of both.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<repo>/crates/gx-substrate`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn lib_rs() -> String {
    std::fs::read_to_string(repo_root().join("crates/gx-substrate/src/lib.rs"))
        .expect("the crate root this test is about")
}

/// Only the crate documentation: the lines a reader of `docs.rs/gx-substrate` lands on.
///
/// `//!` and not `///`: an implementor of `SubstrateAdapter` reads the crate root, and a contract
/// filed under one item's documentation is a contract that binds whoever happened to open that
/// item.
fn crate_doc(source: &str) -> String {
    source
        .lines()
        .filter(|l| l.trim_start().starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The clauses **E-M4-12** fixes, each one word for word.
///
/// A list rather than a paragraph read by eye, so that dropping one is a failing test. The six
/// after the ticket are the ruling's own decomposition: what the normalisation is (lexical), the
/// three operations it consists of, what it is not (symlink resolution), and the residual threat
/// that not-being leaves behind.
const NORMATIVE_CLAUSES: [&str; 8] = [
    "the locator is passed to the gate already normalised",
    "Lexical only",
    "Dot-segment folding",
    "Duplicate-separator removal",
    "Trailing-separator convention",
    "Symlink/realpath resolution is v0.2+",
    "TH-2 residue",
    "E-M4-12",
];

/// The contract is in the crate documentation, clause by clause.
#[test]
fn the_crate_root_states_the_locator_normalisation_contract() {
    let doc = crate_doc(&lib_rs());
    let mut missing: Vec<&str> = Vec::new();
    for clause in NORMATIVE_CLAUSES {
        if !doc.contains(clause) {
            missing.push(clause);
        }
    }
    println!(
        "H2_NORMATIVE_CLAUSES={} MISSING={}",
        NORMATIVE_CLAUSES.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "the crate documentation does not state {missing:?} -- H-2 makes the locator contract a \
         written requirement and E-M4-12 fixes its wording"
    );
}

/// The section is named, so that a later hand can point at it rather than at the whole file.
#[test]
fn the_contract_has_a_section_of_its_own() {
    let doc = crate_doc(&lib_rs());
    assert!(
        doc.contains("# Locator normalisation (normative)"),
        "the contract is not under a heading of its own; hand 2's trait documentation has nothing \
         to reference"
    );
}

/// The contract says lexical, and this hand ships no code that could make it false.
///
/// "zero I/O = does not taint `plan`'s purity or the CAS" is the reason M4-12 (a) was taken over
/// (b), and the
/// reason is measurable while the crate is still a skeleton: no path resolution, no file read, no
/// clock. Asserting the absence now is what makes hand 4 add each of those deliberately, if it adds
/// them at all -- the same shape as `AC-053`'s "the absence" checks (req/69 §8.2).
#[test]
fn the_crate_reads_no_path_no_file_and_no_clock() {
    let dir = repo_root().join("crates/gx-substrate/src");
    let mut offenders: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("readable");
        let code: String = source
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "std::fs",
            "canonicalize",
            "read_link",
            "SystemTime",
            "Instant::now",
        ] {
            if code.contains(forbidden) {
                offenders.push(format!("{}: {forbidden}", path.display()));
            }
        }
    }
    println!("SUBSTRATE_IO_AND_CLOCK_READS={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "the skeleton already reaches outside itself: {offenders:?}. M4-12 (a) keeps normalisation \
         lexical and M4-17 keeps the clock at the engine boundary (41 §6)"
    );
}
