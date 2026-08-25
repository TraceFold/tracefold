// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-M4-28** — gx-substrate's own error vocabulary, in the E-M2-23 form. (sem:
//! SEM-gx-substrate-126, SEM-gx-substrate-127, SEM-gx-substrate-128, SEM-gx-substrate-129,
//! SEM-gx-substrate-130, SEM-gx-substrate-131, SEM-gx-substrate-132, SEM-gx-substrate-133,
//! SEM-gx-substrate-134, SEM-gx-substrate-135, SEM-gx-substrate-136, SEM-gx-substrate-137,
//! SEM-gx-substrate-138, SEM-gx-substrate-139, SEM-gx-substrate-140)
//!
//! `req/38_ERRATA_2026-08-07.md` §30 M4H2-2, adopted (a), verbatim: "declare `gx_substrate::Error`+
//! `Result` (as the repo's 5 other crates already do); the E-M2-23-shaped vocabulary table at the
//! same time (one declaration + a cross-check test + `kind` with no `_` arm); wrap the lower layer's
//! refusal with `From<gx_core::Error>`. Putting it **at the start of hand 3 rather than hand 4** is
//! to avoid the rework of building the conformance harness on the wrong `Result` type and then
//! swapping it out".
//!
//! # The layer split this file is really about
//!
//! §30 states it as a rule about who owns which failure: "a failure of the outside world ('could not
//! be read') is the adapter layer's vocabulary; a rejected argument is gx-core's vocabulary -- **that
//! layer split is 41 §6's implementation form**". gx-core's own documentation says the same thing
//! from the other side -- "The crate does no I/O (41 §6), so there is no error here that comes from
//! the outside world -- every variant is a rejected argument" -- which is why
//! hand 2 could compile a trait against `gx_core::Result` and still not have a type an fs adapter
//! could report a missing file with (req/71 §2 M4H2-2).
//!
//! So the vocabulary here is **the outside world**, plus one variant that carries a lower layer's
//! refusal across the boundary without relabelling it.
//!
//! # The instrument is gx-core's, deliberately
//!
//! The scans below are lifted from `crates/gx-core/tests/core_error_vocabulary.rs`, which lifted
//! them from `crates/gx-gate/tests/error_vocabulary.rs`. Three crates measured the same way is the
//! point: E-M2-23 is one form, and a third parser would be a third answer to what a variant is.

use std::path::{Path, PathBuf};

use gx_substrate::{Error, ERROR_KINDS};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn error_rs() -> String {
    let path = repo_root().join("crates/gx-substrate/src/error.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. **E-M4-28** puts `gx_substrate::Error` and `Result` at the front \
             of hand 3, so that the conformance harness is not built on `gx_core::Result` and \
             swapped afterwards",
            path.display()
        )
    })
}

/// The whole of this crate's `src/`, concatenated -- used only for counting declarations.
fn src_text() -> String {
    let dir = repo_root().join("crates/gx-substrate/src");
    let mut text = String::new();
    for entry in std::fs::read_dir(&dir).expect("gx-substrate/src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|x| x == "rs") {
            text.push_str(&std::fs::read_to_string(&path).expect("readable"));
        }
    }
    text
}

/// The variants of `pub enum <name>`, read out of the source.
///
/// Byte-for-byte the reader `core_error_vocabulary.rs` uses: a variant line sits at one indent level
/// and starts with an upper-case letter, so attributes, doc comments and field lines are none of
/// those.
fn variants_of(source: &str, name: &str) -> Vec<String> {
    let opening = format!("pub enum {name} {{");
    let body = source
        .split(&opening)
        .nth(1)
        .unwrap_or_else(|| panic!("the source no longer declares `{opening}`"));
    let body = body.split("\n}").next().expect("the enum is closed");

    let mut out: Vec<String> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !line.starts_with("    ") || line.starts_with("     ") {
            continue;
        }
        let Some(first) = trimmed.chars().next() else {
            continue;
        };
        if !first.is_ascii_uppercase() {
            continue;
        }
        let variant: String = trimmed
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !variant.is_empty() {
            out.push(variant);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The string literals of `pub const <name>: [&str; N] = [ .. ];`, read out of the source.
fn table_in_source(source: &str, name: &str) -> Vec<String> {
    let needle = format!("pub const {name}: [&str; ");
    let after = source.split(&needle).nth(1).unwrap_or_else(|| {
        panic!("gx-substrate's `src/` does not declare `{needle}..]` (E-M2-23's one-place declaration)")
    });
    let body = after
        .split_once('[')
        .expect("the array literal opens")
        .1
        .split_once(']')
        .expect("the array literal closes")
        .0;
    body.split(',')
        .filter_map(|cell| {
            let cell = cell.trim();
            cell.strip_prefix('"')
                .and_then(|c| c.strip_suffix('"'))
                .map(str::to_string)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// One declaration
// ---------------------------------------------------------------------------

/// `ERROR_KINDS` is declared once in this crate, sorted, with no duplicate.
#[test]
fn the_error_table_is_one_sorted_declaration() {
    let count = src_text().matches("pub const ERROR_KINDS").count();
    assert_eq!(
        count, 1,
        "ERROR_KINDS is declared {count} times in gx-substrate/src; the vocabulary is one \
         declaration (E-M2-23)"
    );

    let declared = table_in_source(&error_rs(), "ERROR_KINDS");
    let mut sorted = declared.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted, declared,
        "ERROR_KINDS is sorted and holds no duplicate"
    );
    println!("GX_SUBSTRATE_ERROR_KINDS={} ({declared:?})", declared.len());
}

// ---------------------------------------------------------------------------
// The reconciliation
// ---------------------------------------------------------------------------

/// The table names every variant of `Error`, and no others.
#[test]
fn the_error_table_names_every_variant() {
    let source = error_rs();
    let variants = variants_of(&source, "Error");
    let mut declared = table_in_source(&source, "ERROR_KINDS");
    declared.sort();
    assert_eq!(
        variants, declared,
        "the Error enum and ERROR_KINDS disagree; one of them was edited alone"
    );
}

/// `kind` has no wildcard arm, and names every variant.
///
/// The whole of the refusal on this crate's side, exactly as in gx-core: the vocabulary **is** the
/// enum, so a kind outside the table is unconstructible rather than refused, and a variant added
/// later does not compile until its name is in both places -- which is how hand 6's two arrived.
#[test]
fn the_kind_function_is_exhaustive_without_a_wildcard() {
    let source = error_rs();
    let body = source
        .split("pub fn kind(&self) -> &'static str {")
        .nth(1)
        .expect("error.rs declares `pub fn kind(&self) -> &'static str`");
    let body = body
        .split("\n    }")
        .next()
        .expect("the function is closed");
    assert!(
        !body.contains("_ =>"),
        "`Error::kind` has a wildcard arm, so a new variant would be absorbed instead of named"
    );
    for variant in variants_of(&source, "Error") {
        assert!(
            body.contains(&format!("Error::{variant}")),
            "`Error::kind` does not name {variant}"
        );
    }
}

// ---------------------------------------------------------------------------
// The layer split, as source
// ---------------------------------------------------------------------------

/// The lower layer's refusals are wrapped rather than restated.
///
/// **E-M4-28**, verbatim: "wrap the lower layer's refusal with `From<gx_core::Error>`". Two halves.
/// The conversion exists,
/// so an adapter writing `snap.locator().parse()?` carries a gx-core refusal outward unchanged; and
/// no variant of this enum re-spells one, which is what would make the two vocabularies overlap and
/// leave a caller unable to tell "the argument was wrong" from "the world would not answer".
#[test]
fn the_lower_layers_refusals_are_wrapped_and_not_restated() {
    let source = error_rs();
    assert!(
        source.contains("impl From<gx_core::Error> for Error")
            || source.contains("#[from]\n        gx_core::Error")
            || source.contains("#[from] gx_core::Error"),
        "gx-substrate's Error does not wrap gx-core's; E-M4-28 asks for \
         `From<gx_core::Error>` so that a lower refusal crosses the boundary as itself"
    );

    // The gx-core vocabulary, read from gx-core rather than restated here, so that a variant added
    // there is compared against this crate on the next run.
    let core = std::fs::read_to_string(repo_root().join("crates/gx-core/src/error.rs"))
        .expect("gx-core's error.rs is readable");
    let core_kinds = table_in_source(&core, "ERROR_KINDS");
    let mine = table_in_source(&source, "ERROR_KINDS");
    let overlap: Vec<&String> = mine.iter().filter(|k| core_kinds.contains(k)).collect();
    println!(
        "GX_CORE_KINDS={} GX_SUBSTRATE_KINDS={} OVERLAP={}",
        core_kinds.len(),
        mine.len(),
        overlap.len()
    );
    assert!(
        overlap.is_empty(),
        "these names are in both vocabularies: {overlap:?}. 41 §6's layer split (§30: \"a failure of \
         the outside world is the adapter layer's vocabulary; a rejected argument is gx-core's \
         vocabulary\") is only real while the two tables are \
         disjoint"
    );
}

/// Every variant is the vocabulary for a failure some trait method documents.
///
/// The A-10 shape applied to a vocabulary: a table that grows a variant nobody's `# Errors` section
/// asks for is 52 contract 2's "adding a feature outside the requirements" at the size of one enum.
/// `error.rs` carries the mapping
/// as a table -- one row per variant, naming the method whose documented failure it spells -- and
/// this reads the mapping back and checks the two agree.
#[test]
fn every_variant_is_the_vocabulary_of_a_documented_failure() {
    let source = error_rs();
    // The markers are stripped before anything is split, so that the same reader works whether the
    // section lives in the module documentation (`//!`) or an item's (`///`). `adapter_contract.rs`
    // reads the trait's contract table the same way.
    let mut doc = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(body) = trimmed
            .strip_prefix("//!")
            .or_else(|| trimmed.strip_prefix("///"))
        {
            doc.push_str(body.strip_prefix(' ').unwrap_or(body));
            doc.push('\n');
        }
    }
    let section = doc
        .split("# Which failure each variant is for")
        .nth(1)
        .expect("error.rs has no `# Which failure each variant is for` section");
    let section = section.split("\n# ").next().expect("the section ends");

    let mut rows: Vec<String> = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().trim_matches('`').to_string())
            .collect();
        if cells
            .iter()
            .all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        rows.push(cells[0].clone());
    }
    assert!(!rows.is_empty(), "the section holds no table");
    rows.remove(0); // the header row
    rows.sort();

    let variants = variants_of(&source, "Error");
    println!(
        "ERROR_VARIANTS={} MAPPED_ROWS={}",
        variants.len(),
        rows.len()
    );
    assert_eq!(
        rows, variants,
        "the variant-to-method table and the enum disagree; a variant with no documented failure \
         behind it is a word nobody has to say (52 contract 2)"
    );
}

// ---------------------------------------------------------------------------
// The vocabulary as values, not as source
// ---------------------------------------------------------------------------

/// The table a caller imports is the table the source declares.
///
/// The scans above read `error.rs` as text, which is what catches a table edited alone. This reads
/// the constant the crate actually exports, which is what catches a *second* table shadowing it --
/// two claims that look alike and fail differently.
#[test]
fn the_exported_table_is_the_declared_one() {
    let declared = table_in_source(&error_rs(), "ERROR_KINDS");
    assert_eq!(
        ERROR_KINDS.to_vec(),
        declared,
        "`gx_substrate::ERROR_KINDS` and the array in `error.rs` are not the same ten words"
    );
}

/// A gx-core refusal crosses the boundary as itself (**E-M4-28**: "wrap the lower layer's refusal
/// with `From<gx_core::Error>`").
///
/// Both halves of what wrapping means. The conversion exists and is reachable through `?`, and the
/// message is not rewritten on the way -- `#[error(transparent)]`, so an adapter author reading a
/// failure sees gx-core's own sentence rather than a paraphrase of it.
#[test]
fn a_lower_refusal_crosses_the_boundary_without_being_relabelled() {
    fn refuse() -> gx_substrate::Result<()> {
        Err(gx_core::Error::OrderExceeded { got: 7, max: 2 })?
    }

    let error = refuse().expect_err("the conversion is what this measures");
    assert_eq!(error.kind(), "Core");
    assert_eq!(
        error.to_string(),
        gx_core::Error::OrderExceeded { got: 7, max: 2 }.to_string()
    );
}

/// Every variant answers with a kind, and the ten kinds are the ten of the table.
///
/// The behavioural mirror of the `_`-arm scan: that one says a wildcard cannot absorb a new variant,
/// and this one says the arms that exist answer with names the table holds.
#[test]
fn every_variant_answers_with_a_kind_the_table_holds() {
    let all = [
        Error::Core(gx_core::Error::TargetMissing),
        Error::Unreadable {
            locator: "/tmp/x".to_string(),
            detail: "no such file".to_string(),
        },
        Error::NotPlannable {
            detail: "already met".to_string(),
        },
        Error::ApplyFailed {
            detail: "rename refused".to_string(),
        },
        Error::ForeignDelta {
            expected: gx_core::SubstrateKind::Fs,
            got: gx_core::SubstrateKind::Git,
        },
        Error::PayloadUnreadable {
            detail: "unknown version".to_string(),
        },
        Error::NotDigestible {
            detail: "no canonical form".to_string(),
        },
        // M4 hand 6's ninth and tenth. **E-M4-32** separates a mis-wired call from a state with no
        // inverse, and **M4H5-5, adopted (b)**, separates "the argument is not a position" from "the
        // delta could not be applied" -- both of them the same refusal to let a defect wear a business
        // condition's face that E-M4-27 made about `cas_eq`.
        Error::LocatorMismatch {
            expected: "/tmp/x".to_string(),
            got: "/tmp/y".to_string(),
        },
        Error::NotAPosition {
            locator: "relative/x".to_string(),
            normalised: "relative/x".to_string(),
        },
        // M4 hand 4's eighth. An adapter built one hand at a time has to be able to say "not yet"
        // as a value -- the variant's own documentation says why the three alternatives (panic, a
        // borrowed variant, an invented `Ok`) are each worse -- and the conformance harness reads
        // exactly this one to report "NOT_SUPPLIED" rather than "failed" (§31 M4H3-4 (b)).
        Error::Unimplemented {
            method: "apply".to_string(),
            detail: "M4 hand 5 supplies it".to_string(),
        },
    ];

    let mut kinds: Vec<&str> = all.iter().map(Error::kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    println!("GX_SUBSTRATE_ERROR_VARIANTS={}", all.len());
    assert_eq!(
        kinds,
        ERROR_KINDS.to_vec(),
        "one value per variant answered with fewer distinct kinds than the table declares, so two \
         variants share a name"
    );
}

/// The vocabulary is **ten** words, and the two hand 6 added are the two the rulings named.
///
/// req/38 §33, verbatim: **E-M4-32** "new variant **`LocatorMismatch{expected, got}`** (a
/// vocabulary-table update, compliant with the E-M2-23 instrument)" and **M4H5-5, adopted (b)**,
/// "add the **`NotAPosition`** variant ... at the start of hand 6; the vocabulary table goes 8->10
/// (together with E-M4-32's share)".
///
/// The count is asserted because the scans above are **relative**: they say the enum, the table and
/// `kind` agree with each other, which they would also do if a variant had been dropped. Two rulings
/// named two words, and this is the probe that says both arrived. Both are refusals of an **argument**
/// and would in another workspace have been gx-core's; they are here because gx-core cannot name a
/// locator convention (**ASM-69-3** is an adapter's, and 41 §4 hands `invert` two values that only an
/// adapter can compare).
#[test]
fn the_vocabulary_is_the_ten_words_two_rulings_left_it() {
    let declared = table_in_source(&error_rs(), "ERROR_KINDS");
    println!(
        "GX_SUBSTRATE_ERROR_KINDS_AFTER_HAND6={} {declared:?}",
        declared.len()
    );
    assert_eq!(
        declared.len(),
        10,
        "§33 puts the vocabulary at ten: eight from hands 3-5, plus `LocatorMismatch` (E-M4-32) and \
         `NotAPosition` (M4H5-5, adopted (b))"
    );
    for word in ["LocatorMismatch", "NotAPosition"] {
        assert!(
            declared.contains(&word.to_string()),
            "the table does not hold {word:?}, so a ruling named a refusal the enum cannot spell"
        );
    }
}

/// Every refusal carries the value that makes it diagnosable.
///
/// gx-core's `OrderExceeded` carries the order "so a caller can report the ceiling it actually hit",
/// and the same reason applies one layer up: an adapter author reading "the substrate would not
/// answer" needs to see *which* locator, and an engine author reading `ForeignDelta` needs to see
/// which two substrates. A message that only names the class is a message that sends the reader back
/// to the code.
#[test]
fn every_message_names_the_value_it_is_about() {
    assert!(Error::Unreadable {
        locator: "/tmp/nowhere".to_string(),
        detail: "no such file".to_string(),
    }
    .to_string()
    .contains("/tmp/nowhere"));

    let foreign = Error::ForeignDelta {
        expected: gx_core::SubstrateKind::Fs,
        got: gx_core::SubstrateKind::Git,
    }
    .to_string();
    assert!(foreign.contains("Fs") && foreign.contains("Git"));

    // Hand 6's two are diagnosed by seeing **two** values side by side: which object the delta is
    // about against which one the snapshot names, and what a spelling normalised to against what was
    // written. A message carrying one of each pair would send the reader back to the code.
    let mismatch = Error::LocatorMismatch {
        expected: "/tmp/x".to_string(),
        got: "/tmp/y".to_string(),
    }
    .to_string();
    assert!(mismatch.contains("/tmp/x") && mismatch.contains("/tmp/y"));

    let not_a_position = Error::NotAPosition {
        locator: "a/../b".to_string(),
        normalised: "b".to_string(),
    }
    .to_string();
    assert!(not_a_position.contains("a/../b") && not_a_position.contains("\"b\""));
}
