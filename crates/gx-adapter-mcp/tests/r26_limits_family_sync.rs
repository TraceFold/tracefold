// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/324` H-01 (`req/38` §231 ruling 1)** — the page and the classifier are one width.
//!
//! # What broke
//!
//! `ArgSource::draws_from_outside_the_forward_call` classifies the restore-template vocabulary
//! into "draws on something the forward call does not carry" and "does not". Six words answer
//! `true`, and **one arm of that match carries two of them**:
//!
//! ```text
//! ArgSource::Const(_) | ArgSource::ConstJson(_) => true,
//! ```
//!
//! `docs/LIMITS.md`'s accepted-residual paragraph — the page a buyer reads — named exactly **one**
//! of those two (a `const` member), and `req/38` §230 ruling 1's own correction, added to that
//! same paragraph *in order to record that the residual is spelling-dependent*, named the same
//! single spelling again. The twenty-fifth audit drove the other one on a real road: a
//! `const_json` member holding `""` and one holding `null` reach a terminal state that is not one
//! character different — accepted, `Admit`, effect landed, and the `gx undo` gx itself printed
//! empties the object with `rc=0`.
//!
//! # What this file is
//!
//! The drift that produced that is mechanical — a page counted words by hand while a `match` arm
//! counted them by construction — so the gate against it is mechanical too. This arm reads the
//! classifier's `true` arms out of `catalogue.rs`, derives the **wire spelling** of every variant
//! in them (`#[serde(rename_all = "snake_case")]`), and requires `docs/LIMITS.md` to name each one.
//! A word added to the vocabulary and classified as drawing from outside makes this arm red until
//! the page moves in the same commit.
//!
//! It is a *source* scan on purpose, and it is a scan of the arms rather than of the enum: what a
//! buyer needs named is the set the gate lets through on its own, which is the `true` side.

use std::path::{Path, PathBuf};

/// The repository root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `snake_case`, the way `serde`'s `rename_all` derives it from a variant name.
fn snake_case(variant: &str) -> String {
    let mut out = String::new();
    for (i, c) in variant.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Every `ArgSource::` variant named on a line of `draws_from_outside_the_forward_call` that
/// answers `true`, in source order, with the arm index it came from.
///
/// Comment lines are dropped first: the *reason* an arm exists names variants too, and counting
/// those would let a page satisfy this arm by being mentioned in prose.
fn variants_that_draw_from_outside(src: &str) -> Vec<(usize, String)> {
    let body = src
        .split_once("pub fn draws_from_outside_the_forward_call")
        .expect("the classifier is in this file")
        .1;
    let body = body
        .split_once("\n    }\n")
        .expect("the classifier's match ends")
        .0;
    let mut out = Vec::new();
    let mut arm = 0usize;
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("//") || !line.contains("=> true") {
            continue;
        }
        arm += 1;
        for piece in line.split("ArgSource::").skip(1) {
            let name: String = piece
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                out.push((arm, name));
            }
        }
    }
    out
}

/// 🔴 The bed control. An arm that finds nothing must not pass by finding nothing.
#[test]
fn the_classifier_names_more_than_one_word_per_family() {
    let src = read("crates/gx-adapter-mcp/src/catalogue.rs");
    let found = variants_that_draw_from_outside(&src);
    let arms = found.iter().map(|(a, _)| *a).max().unwrap_or(0);
    println!(
        "R26_LIMITS_SYNC variants={} arms={} list={:?}",
        found.len(),
        arms,
        found.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>()
    );
    assert!(
        found.len() >= 6,
        "🔴 the scan found {} variants classified as drawing from outside the forward call, and \
         the build this file was written against has six. A scan that finds nothing passes by not \
         seeing the thing it is looking for: {found:?}",
        found.len()
    );
    assert!(
        arms >= 1 && found.len() > arms,
        "🔴 the premise of `req/324` H-01 is that at least one arm carries more than one word \
         (a `Const` and a `ConstJson` on one line). variants={} arms={arms}",
        found.len()
    );
}

/// 🔴 **`req/324` H-01** — every word the classifier lets through on its own is named on the page.
#[test]
fn the_page_names_every_word_that_draws_from_outside_the_forward_call() {
    let src = read("crates/gx-adapter-mcp/src/catalogue.rs");
    let limits = read("docs/LIMITS.md");
    let found = variants_that_draw_from_outside(&src);
    assert!(found.len() >= 6, "bed: {found:?}");

    let mut missing: Vec<String> = Vec::new();
    let mut present: Vec<String> = Vec::new();
    for (_, variant) in &found {
        let spelling = snake_case(variant);
        // The page spells a template member the way a catalogue file does: in backticks, as a
        // JSON key. Both spellings count -- what is measured is whether the word is on the page at
        // all, not its typography.
        if limits.contains(&format!("`{spelling}`")) || limits.contains(&format!("\"{spelling}\""))
        {
            present.push(spelling);
        } else {
            missing.push(spelling);
        }
    }
    println!(
        "R26_LIMITS_SYNC page_names={present:?} page_missing={missing:?} n={}",
        found.len()
    );
    assert!(
        missing.is_empty(),
        "🔴 `req/324` H-01 (`req/38` §231 ruling 1): `catalogue.rs` classifies {n} words as drawing \
         on something the forward call does not carry, and `docs/LIMITS.md` -- the page a buyer \
         reads the accepted residual off -- does not name {missing:?}. The audit drove the missing \
         half on a real road and it reached the same terminal state as the named half: accepted, \
         `Admit`, effect landed, and the printed `gx undo` emptied the object with `rc=0`. A page \
         that names one member of a family the gate admits as a family understates the residual \
         it is there to declare. Move the page in the same commit as the arm.",
        n = found.len()
    );
}

/// 🔴 **`req/324` §5(g)** — the page separates the family the gate admits from the half that was
/// measured to destroy, because the audit measured that those are **not** the same set.
///
/// Four words satisfy the gate while carrying nothing of the prior. Two of them (the two constant
/// forms) resolve at plan time and their printed undo empties the object; the do-result form's
/// printed undo refused with `rc=1` and left the object standing. The page said neither.
#[test]
fn the_page_separates_the_measured_destructive_half_from_the_rest() {
    let limits = read("docs/LIMITS.md");
    // 🔴 The **last** statement of the residual, not the first: this page stacks a block per
    // release and older blocks are the historical record rather than the current claim. Anchoring
    // on the first occurrence would hold the newest correction to the oldest paragraph's
    // neighbourhood, which is the opposite of what a buyer reads.
    let at = limits
        .rfind("accepted residual")
        .expect("🔴 the accepted-residual paragraph is the page's own words for this");
    // The window runs both ways and is generous on purpose: what is measured is that the
    // distinctions live *with* the declaration, not that they live on one line of it.
    let lo = limits[..at]
        .char_indices()
        .rev()
        .nth(6_000)
        .map_or(0, |(i, _)| i);
    let hi = limits[at..]
        .char_indices()
        .nth(6_000)
        .map_or(limits.len(), |(i, _)| at + i);
    let window = &limits[lo..hi];
    for needle in [
        "const_json",
        "do_result",
        "prior_contents_utf8",
        "prior_json",
    ] {
        assert!(
            window.contains(needle),
            "🔴 `req/324` H-01/§5(g): the accepted-residual declaration does not name {needle:?}. \
             The six words the gate lets through are not one class -- two resolve at plan time and \
             were measured to empty the object, one was measured **not** to destroy (undo `rc=1`, \
             object unmoved), and two draw the prior so the restore puts the object back. A \
             residual declared at the width of one word is the drift `req/38` §230 ruling 1 tried \
             to correct and repeated."
        );
    }
}
