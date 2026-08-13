//! 🔴 **E-M2-23 / M6H5-2 (b)** — `ERROR_KINDS` is the variants of `Error`, and stays so.
//!
//! The gx-log half of the pair req/38 §55 (M6H8-16 採(a)) orders. Hand 8's mutation was
//! `"OutOfRange"` → `"OutOfRang"`: it compiles, `kind()` still returns the right word, and the array
//! that gx-api reads as a **denominator** now holds one entry nothing produces. Every suite in the
//! workspace stayed green (req/96 §7.3, `RC=0`), which is what makes the ghost dangerous — the
//! coverage check over `REFUSALS` passes *because* it covers the ghost, and the real refusal folds
//! into `INTERNAL` with nothing red on either side.
//!
//! The compiler already holds the other two doors (no `_` arm in `kind()`; a declared length on
//! `[&str; 5]`), so a misspelling is the whole of what this file catches. Same shape as
//! `crates/gx-engine/tests/engine_shape.rs`, which is the point: four crates carry this spelling
//! already.

use std::path::{Path, PathBuf};

use gx_log::ERROR_KINDS;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

/// The variant names of `pub enum Error`, in declaration order, read out of the source.
fn variants_in_source(relative: &str) -> Vec<String> {
    let path = repo_root().join(relative);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    let body = source
        .split("pub enum Error {")
        .nth(1)
        .unwrap_or_else(|| panic!("{relative} still declares `pub enum Error`"));
    let body = body.split("\n}").next().expect("the enum is closed");

    let mut variants = Vec::new();
    for line in body.lines() {
        if !line.starts_with("    ") || line.starts_with("     ") {
            continue;
        }
        let trimmed = line.trim();
        if !trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
        {
            continue;
        }
        let name: String = trimmed
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !name.is_empty() {
            variants.push(name);
        }
    }
    variants
}

/// 🔴 [`gx_log::ERROR_KINDS`] names every variant of `Error`, in order, and no others.
#[test]
fn the_error_vocabulary_is_the_error_enum() {
    let variants = variants_in_source("crates/gx-log/src/lib.rs");
    println!("GX_LOG_ERROR_KINDS={} ({variants:?})", ERROR_KINDS.len());
    assert_eq!(
        ERROR_KINDS.to_vec(),
        variants,
        "`ERROR_KINDS` is not the variants of `Error`, in order. gx-api reads this array as the \
         denominator for `Origin::Log`, so a misspelt entry makes 44 §2.3's coverage claim true \
         about a refusal that cannot happen"
    );
}

/// No empty and no duplicate word: two refusals sharing a name are one refusal a reader cannot see.
#[test]
fn the_table_holds_five_distinct_words() {
    let mut sorted = ERROR_KINDS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ERROR_KINDS.len());
    assert!(!ERROR_KINDS.iter().any(|k| k.is_empty()));
}
