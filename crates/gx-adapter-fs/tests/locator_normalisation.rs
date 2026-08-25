// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-M4-12** as behaviour, **L7** as a property, and **M4-22** as a scan of the crate's own
//! documentation.
//!
//! Three obligations meet here, and they are different in kind:
//!
//! * **E-M4-12** (req/38 §28 M4-12, adopted (a)) fixes *what* the normalisation is -- "**purely lexical**...
//!   dot-segment folding, duplicate-separator removal, trailing-separator convention" -- and files symlink resolution as v0.2+ with
//!   "the TH-2 residue... is made explicit in the doc and the receipt-side disclosure". (sem: SEM-gx-adapter-fs-250)
//! * **L7** (req/69 §3.4) is the property: `normalize(normalize(l)) == normalize(l)` and
//!   `l ≈ l' → normalize(l) == normalize(l')`. It is T3's shape one layer over -- gx-canon chooses a
//!   representative for a value, an adapter chooses one for a position.
//! * **M4-22** (§28) makes 42 §2.3's "documentation of a `SubstrateAdapter` implementation is **required** to record it" a (sem: SEM-gx-adapter-fs-251)
//!   machine check rather than an intention: the `≈` this adapter implements has to be written where
//!   an implementor reads it.
//!
//! # The scan is limited to a section, and it skips quotations
//!
//! §31 M4H3-5 made that a standing rule after a hand-3 mutation survived: a normative section that
//! quotes its own ruling satisfies a naive clause scan from the quotation, so gx's own sentence can
//! be rewritten while every clause stays green. So [`clause_body`] takes one heading's text and drops
//! every quoted line (`//! > `), and the mutation battery in `tools/verify_m4h4.sh` re-runs the
//! experiment on this crate.

use std::path::PathBuf;

use gx_adapter_fs::{locator, normalize};
use proptest::prelude::*;

fn crate_root_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", path.display()))
}

/// The body of one `//! # ...` section, with quoted lines removed (§31 M4H3-5).
///
/// Lines are joined with a space rather than a newline and runs of whitespace collapse, because
/// prose wraps: "duplicate separator" written across two lines is the same clause as "duplicate
/// separator" written on one, and a scanner that said otherwise would be measuring the line width. (sem: SEM-gx-adapter-fs-252)
fn clause_body(source: &str, heading: &str) -> String {
    let mut body = String::new();
    let mut inside = false;
    for line in source.lines() {
        let Some(doc) = line.strip_prefix("//!") else {
            if inside {
                break;
            }
            continue;
        };
        let text = doc.trim_start();
        if text.starts_with("# ") {
            inside = text == heading;
            continue;
        }
        if inside && !text.starts_with("> ") {
            body.push_str(text);
            body.push(' ');
        }
    }
    let body = body.split_whitespace().collect::<Vec<&str>>().join(" ");
    assert!(
        !body.trim().is_empty(),
        "the crate root has no section headed `{heading}`"
    );
    body
}

// ---------------------------------------------------------------------------
// E-M4-12, clause by clause
// ---------------------------------------------------------------------------

/// Clause 2: `.` disappears and `..` cancels the segment before it, as text.
#[test]
fn dot_segments_fold() {
    assert_eq!(normalize("/a/./b"), "/a/b");
    assert_eq!(normalize("/a/b/../c"), "/a/c");
    assert_eq!(normalize("/a/b/.."), "/a");
    assert_eq!(normalize("/a/./././b"), "/a/b");
    assert_eq!(normalize("/etc/../etc/passwd"), "/etc/passwd");
}

/// Clause 3: repeated separators collapse.
#[test]
fn duplicate_separators_collapse() {
    assert_eq!(normalize("/a//b"), "/a/b");
    assert_eq!(normalize("/a///b////c"), "/a/b/c");
    assert_eq!(normalize("///"), "/");
}

/// Clause 4: this adapter drops a trailing separator, except at the root.
///
/// E-M4-12 leaves the choice to each adapter and forbids only "deciding it per call site". The root (sem: SEM-gx-adapter-fs-253)
/// keeps its separator because `""` is not a position and `/` is.
#[test]
fn a_trailing_separator_is_dropped_except_at_the_root() {
    assert_eq!(normalize("/a/b/"), "/a/b");
    assert_eq!(normalize("/a/b//"), "/a/b");
    assert_eq!(normalize("/"), "/");
    assert_eq!(normalize("/a/"), "/a");
}

/// A `..` that would climb past the root is dropped, and cannot escape it.
///
/// The fail-open direction this closes is the reason the gate needs normalisation at all: M3-10
/// fixed a v0.1 policy pack's effective range at the "locator level", so `/etc/../../etc/passwd` and (sem: SEM-gx-adapter-fs-254)
/// `/etc/passwd` have to be one subject or a `/etc/**` forbid is a spelling contest.
#[test]
fn a_double_dot_cannot_climb_past_the_root() {
    assert_eq!(normalize("/.."), "/");
    assert_eq!(normalize("/../.."), "/");
    assert_eq!(normalize("/a/../../b"), "/b");
    assert_eq!(normalize("/etc/../../etc/passwd"), "/etc/passwd");
}

/// A relative locator keeps the `..` it cannot cancel, and stays relative.
///
/// E-M4-12 clause 2 leaves this to the adapter -- "A leading `..` that cannot cancel is the
/// adapter's to define and to write down" -- and this adapter keeps it, because dropping it would (sem: SEM-gx-adapter-fs-255)
/// turn `../secret` into `secret` and invent a position the caller did not name. The refusal happens
/// where the locator is *used*, not where it is spelled: [`gx_adapter_fs::locator::is_absolute`] is
/// what `snapshot` and `plan` consult.
#[test]
fn a_relative_locator_keeps_what_it_cannot_cancel() {
    assert_eq!(normalize("../a"), "../a");
    assert_eq!(normalize("a/../../b"), "../b");
    assert_eq!(normalize("./a"), "a");
    assert!(!locator::is_absolute("../a"));
    assert!(locator::is_absolute("/a"));
}

/// Clause 1: no filesystem is consulted, so a locator that does not exist normalises anyway.
#[test]
fn normalisation_needs_no_filesystem() {
    let missing = "/no/such/path/../place/./here/";
    assert_eq!(normalize(missing), "/no/such/place/here");
}

/// The bytes are the bytes: no case folding and no Unicode normalisation (**ASM-69-2**).
///
/// A filesystem's own answer to both questions is a mount option, so an adapter that folded case
/// would make its own idea of identity disagree with the kernel's on exactly the systems where it
/// mattered. E-M4-12 leaves the question to the adapter and this is the answer, written down.
#[test]
fn the_locator_is_bytes_and_not_a_language() {
    assert_ne!(normalize("/A"), normalize("/a"));
    // U+00E9 against `e` + U+0301: one grapheme, two byte strings, and no NFC here.
    assert_ne!(normalize("/caf\u{e9}"), normalize("/cafe\u{301}"));
    assert_eq!(normalize("/caf\u{e9}"), "/caf\u{e9}");
}

// ---------------------------------------------------------------------------
// L7
// ---------------------------------------------------------------------------

/// L7, first half: normalising twice changes nothing.
#[test]
fn normalisation_is_idempotent_on_the_table_above() {
    for spelling in [
        "/a/./b",
        "/a//b",
        "/a/b/../c",
        "/",
        "///",
        "/..",
        "../a",
        "/a/b/",
        "/etc/../etc/passwd",
    ] {
        let once = normalize(spelling);
        assert_eq!(
            normalize(&once),
            once,
            "normalising {spelling:?} twice moved"
        );
    }
}

proptest! {
    /// L7, first half, over generated spellings.
    ///
    /// The alphabet is the one that makes the clauses collide -- separators, `.`, `..` and ordinary
    /// segments -- because a generator over arbitrary strings would spend its cases on inputs where
/// nothing folds. 34's judgment-method column for AC-047 reads "unit + property" and `tools/ci.sh`
/// stage 4i runs this file at the declared count (51 §3: "≥1000 cases/PR"). (sem: SEM-gx-adapter-fs-256)
    #[test]
    fn normalisation_is_idempotent(spelling in prop::collection::vec(
        prop_oneof![
            Just("/".to_string()),
            Just(".".to_string()),
            Just("..".to_string()),
            "[a-z]{1,3}",
        ],
        0..12usize,
    ).prop_map(|parts| parts.concat())) {
        let once = normalize(&spelling);
        prop_assert_eq!(normalize(&once), once);
    }

    /// L7, second half, over generated spellings: inserting a `/.` never changes the representative.
    ///
    /// `l ≈ l'` is decided by this adapter's own `≈` (42 §2.3) and the crate root defines it; this is
    /// the half of the relation a generator can produce witnesses for.
    #[test]
    fn an_inserted_dot_segment_is_the_same_position(
        head in "(/[a-z]{1,3}){1,4}",
        tail in "(/[a-z]{1,3}){0,3}",
    ) {
        prop_assert_eq!(normalize(&format!("{head}/.{tail}")), normalize(&format!("{head}{tail}")));
    }
}

/// L7, second half, as a table: every clause of E-M4-12 produces a pair of equivalent spellings.
#[test]
fn equivalent_spellings_share_a_representative() {
    for (left, right) in [
        ("/a/b", "/a/./b"),
        ("/a/b", "/a//b"),
        ("/a/b", "/a/b/"),
        ("/a/b", "/a/c/../b"),
        ("/", "///"),
        ("/", "/.."),
    ] {
        assert_eq!(
            normalize(left),
            normalize(right),
            "{left:?} and {right:?} are one position and normalised apart"
        );
    }
}

// ---------------------------------------------------------------------------
// M4-22: the documentation 42 §2.3 requires
// ---------------------------------------------------------------------------

/// 42 §2.3's "required", measured: the `≈` this adapter implements is written in its documentation. (sem: SEM-gx-adapter-fs-257)
///
/// One clause per row of E-M4-12, looked for **inside the section** rather than anywhere in the file
/// (§30 M4H2-6: "somewhere in the file" is not "written in that spot"). (sem: SEM-gx-adapter-fs-258)
///
/// 🔴 The "positions are absolute" row is **the minimal form of K-2, adopted (a)** (`req/38` §35). req/76 §2.10 counted the section's (sem: SEM-gx-adapter-fs-259)
/// five numbered clauses against this list and found clause 5 (**ASM-69-3**) named by no token; §2.1
/// (B-1) then deleted that clause from the crate root and **every suite stayed green**. The behaviour
/// it describes is guarded three times over (`scope.rs`'s two probes and `fs_delta.rs`'s
/// `NotAPosition`), so what the deletion removed was not the rule but the place 42 §2.3 requires the
/// rule to be written -- "documentation of a `SubstrateAdapter` implementation is **required** to record it" is a requirement (sem: SEM-gx-adapter-fs-260)
/// about the documentation, and only a token in this list measures it.
#[test]
fn the_equivalence_relation_is_documented_where_42_2_3_requires_it() {
    let body = clause_body(&crate_root_source(), "# The equivalence `≈` (normative)");
    let clauses = [
        ("lexical only", "purely lexical"),
        ("dot-segment", "dot-segment"),
        ("duplicate separator", "duplicate separator"),
        ("trailing separator", "trailing separator"),
        ("byte-literal", "byte-literal"),
        ("positions absolute", "Positions are absolute"),
        ("symlink unresolved", "symbolic link"), // (sem: SEM-gx-adapter-fs-261)
    ];
    let missing: Vec<&str> = clauses
        .iter()
        .filter(|(_, token)| !body.contains(token))
        .map(|(name, _)| *name)
        .collect();
    println!(
        "EQUIVALENCE_CLAUSES={} MISSING={:?}",
        clauses.len() - missing.len(),
        missing
    );
    assert!(
        missing.is_empty(),
        "42 §2.3 requires the adapter's own `≈` in its documentation; these clauses of E-M4-12 are \
         not in that section: {missing:?}"
    );
}

/// TH-2's residue is disclosed, not implied (**E-M4-12**: "made explicit in the doc and the receipt-side disclosure"). (sem: SEM-gx-adapter-fs-262)
///
/// 45 §4 forbids overclaiming, and a lexical normaliser that called its output a canonical path
/// would be doing exactly that: an actor who can create a symbolic link still chooses which spelling
/// the gate sees.
#[test]
fn the_residue_lexical_normalisation_leaves_is_disclosed() {
    let body = clause_body(&crate_root_source(), "# What v0.1 does not close");
    for token in ["TH-2", "symbolic link", "v0.2"] {
        assert!(
            body.contains(token),
            "the disclosure section does not name {token:?}"
        );
    }
}
