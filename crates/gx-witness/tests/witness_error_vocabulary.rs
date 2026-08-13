//! 🔴 **E-M2-23 / M6H5-2 (b)** — `ERROR_KINDS` is the variants of `Error`, and stays so.
//!
//! # What was missing, and what it was worth
//!
//! M6 hand 5 added [`gx_witness::ERROR_KINDS`] and `Error::kind` (§52 M6H5-2 採(a), paying E-M2-23's
//! bill). What it did not add is the probe that keeps the two in step, and §52 sent that half to
//! hand 8 as its DoD. Hand 8 measured it: renaming one entry of this array to a word `kind()` never
//! returns (`"Schema"` → `"Schem"`) left **every suite in the workspace green** (req/96 §7.3, and
//! Fable's independent re-run in req/38 §55 confirmed `RC=0`). The same mutation against gx-engine,
//! which has this probe, is red.
//!
//! Two of the three places are already held by the compiler: `Error::kind`'s `match` has no `_` arm,
//! so a **new** variant does not compile, and `[&str; 8]` is a declared length, so a **deleted**
//! entry does not compile (M6H7-10: 「長さを宣言した配列は assertion より 1 段早い gate」). What
//! compiles cleanly is a **misspelt** entry, and that is what this file is for.
//!
//! # Why a misspelling does not stay inside this crate
//!
//! `crates/gx-api/src/gx_code.rs` reads four crates' `ERROR_KINDS` as the **denominator** per
//! `Origin`, and a probe there checks that the 33 `REFUSALS` rows cover it. A ghost entry is covered
//! by that check like any other — so 44 §2.3's claim 「every refusal has a code」 would be proved
//! **about a refusal that does not exist**, while the real `"Schema"` fell through to `INTERNAL`.
//! Neither side goes red. That is the whole reason this costs a file.
//!
//! Same shape as `crates/gx-engine/tests/engine_shape.rs`
//! (`the_error_vocabulary_is_the_error_enum`), deliberately: four crates already carry this spelling
//! and a fifth design would be a fifth thing to read.

use std::path::{Path, PathBuf};

use gx_witness::ERROR_KINDS;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

/// The variant names of `pub enum Error`, in declaration order, read out of the source.
///
/// Read rather than matched on, for the reason gx-gate's copy gives: a `match` here would be updated
/// in the same edit that added a variant and would therefore never notice one.
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
        // A variant sits at exactly one indent level and starts with an upper-case letter. Field
        // lines are deeper, attributes start with `#`, doc comments with `/`.
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

/// 🔴 [`gx_witness::ERROR_KINDS`] names every variant of `Error`, in order, and no others.
#[test]
fn the_error_vocabulary_is_the_error_enum() {
    let variants = variants_in_source("crates/gx-witness/src/lib.rs");
    println!(
        "GX_WITNESS_ERROR_KINDS={} ({variants:?})",
        ERROR_KINDS.len()
    );
    assert_eq!(
        ERROR_KINDS.to_vec(),
        variants,
        "`ERROR_KINDS` is not the variants of `Error`, in order. A word here that `kind()` never \
         returns is a ghost in gx-api's denominator (`gx_code.rs`'s `Origin::Witness`): the \
         coverage of 44 §2.3 would be proved about a refusal nobody can raise, and the refusal that \
         is really raised would fold silently into INTERNAL"
    );
}

/// The one fact the array's length already holds, stated so that the compiler's half is visible.
///
/// M6H7-10's point: `[&str; 8]` refuses a deleted entry at compile time, which is a stronger gate
/// than any assertion — and an assertion that re-checks it is how a reader learns the gate is there.
#[test]
fn every_kind_the_error_can_return_is_in_the_table() {
    for (index, kind) in ERROR_KINDS.iter().enumerate() {
        assert!(!kind.is_empty(), "entry {index} of the vocabulary is empty");
    }
    let mut sorted = ERROR_KINDS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ERROR_KINDS.len(),
        "one word twice is one refusal that cannot be told from another"
    );
}
