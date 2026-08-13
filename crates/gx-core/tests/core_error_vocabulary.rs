//! **H-3** / **E-M2-23** — gx-core's error vocabulary, declared in one place and compared with the
//! enum it is supposed to name.
//!
//! `req/38_ERRATA_2026-08-07.md` §25 逐語: 「**H-3(M4 窓)**: gx-core の Error 語彙表は次に gx-core を
//! 触る手(M4 冒頭=H-1 実装と同窓)で E-M2-23 形に。compile 網羅 match は当面の実効体として可」. The
//! interim body is `compose_range.rs`'s `the_error_vocabulary_this_hand_widened_is_eight_variants`,
//! a `match` with no `_` arm; req/66 §4 raised the missing table and this file is the table's other
//! half.
//!
//! # The form, and the one place it differs from gx-gate's
//!
//! E-M2-23 asks for 「1 箇所宣言+宣言外 code は構成時拒否+宣言と実 variant の突合」.
//!
//! * **One declaration** and **the reconciliation** are the same here as in
//!   `crates/gx-gate/tests/error_vocabulary.rs`: `ERROR_KINDS` is declared once, and the variants
//!   are read out of the source rather than matched on -- a `match` would be updated in the same
//!   edit that added a variant and would therefore never notice one (req/64 §2, A-10's reason).
//! * **The refusal** cannot be the same, because the two crates carry different things.
//!   `gx_gate::Reason` carries a `code: String` that a caller supplies, so there is a moment where
//!   a code outside the table can be offered and refused. `gx_core::Error` carries no such string:
//!   the vocabulary *is* the enum, which is closed, so a kind outside the table cannot be
//!   constructed at all. The refusal is therefore by type rather than by check, and what has to be
//!   measured is that the type and the table say the same thing -- [`gx_core::Error::kind`] is
//!   total onto `ERROR_KINDS` and onto nothing else, with no `_` arm to absorb a new variant.
//!
//! The stronger form is the one that needs the smaller test, which is why it is worth saying out
//! loud rather than leaving the reader to notice the asymmetry.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn error_rs() -> String {
    std::fs::read_to_string(repo_root().join("crates/gx-core/src/error.rs"))
        .expect("error.rs is readable")
}

/// The whole of this crate's `src/`, concatenated -- used only for counting declarations.
fn src_text() -> String {
    let dir = repo_root().join("crates/gx-core/src");
    let mut text = String::new();
    for entry in std::fs::read_dir(&dir).expect("gx-core/src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|x| x == "rs") {
            text.push_str(&std::fs::read_to_string(&path).expect("readable"));
        }
    }
    text
}

/// The variants of `pub enum <name>`, read out of the source.
///
/// A variant line sits at one indent level and starts with an upper-case letter: `NotComposable,`,
/// `OrderExceeded {`. Attributes, doc comments and field lines are none of those. Lifted from
/// `crates/gx-gate/tests/error_vocabulary.rs` so that the two crates are measured the same way.
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
///
/// Read rather than imported for the half of the claim that is about the source: a test that only
/// imported the constant could not tell a table declared once from a table declared twice.
fn table_in_source(source: &str, name: &str) -> Vec<String> {
    let needle = format!("pub const {name}: [&str; ");
    let after = source.split(&needle).nth(1).unwrap_or_else(|| {
        panic!(
            "gx-core's `src/` does not declare `{needle}..]` -- H-3 asks for the table E-M2-23 \
             describes"
        )
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

/// `ERROR_KINDS` is declared once, and it is sorted with no duplicate.
///
/// Sorted so that the reconciliation below compares two sets rather than two orderings, and so
/// that adding a variant is an insertion a reviewer can see rather than an append.
#[test]
fn the_error_table_is_one_sorted_declaration() {
    let count = src_text().matches("pub const ERROR_KINDS").count();
    assert_eq!(
        count, 1,
        "ERROR_KINDS is declared {count} times in gx-core/src; the vocabulary is one declaration \
         (E-M2-23)"
    );

    let declared = table_in_source(&error_rs(), "ERROR_KINDS");
    let mut sorted = declared.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted, declared,
        "ERROR_KINDS is sorted and holds no duplicate"
    );
    println!("GX_CORE_ERROR_KINDS={} ({declared:?})", declared.len());
}

// ---------------------------------------------------------------------------
// The reconciliation: the table and the enum say the same thing
// ---------------------------------------------------------------------------

/// `ERROR_KINDS` names every variant of `Error`, and no others.
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

/// The kind function has no `_` arm.
///
/// This is the whole of the refusal on this crate's side: with the arms exhaustive, a variant added
/// later stops the crate compiling until somebody writes its name down, so no `Error` can carry a
/// kind the table does not hold. A `_ => "Other"` would make the table a description instead of a
/// definition, and the compiler would stop asking.
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
