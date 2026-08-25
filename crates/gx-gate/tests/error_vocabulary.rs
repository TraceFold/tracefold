// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The error-vocabulary window: one declaration, one refusal, and the containment that ties it to
//! 44 §2.3 (**E-M3-6** / M3-08 / **E-M2-23** / C-5 / D-3 / **M3-21**).
//!
//! req/38 §19 puts six items in one hand because they are one decision -- "they are all subordinate
//! to the same design judgment of **how far to check an error's value**, and splitting them would
//! mean making that same judgment four times" (sem: SEM-gx-gate-170) -- and
//! this file is where the gate-side half of that decision is measured. The gx-canon half (F-3, A-8,
//! A-9: how far into an error's *payload* a negative vector reaches) is in
//! `crates/gx-canon/tests/negative_vectors.rs`, and the two halves answer the same question at
//! their own layers.
//!
//! | ruling | what it asks for | what measures it here |
//! |---|---|---|
//! | M3-08 / **E-M3-6** | ~~three~~ **two** gate codes, and "the gate's reason codes ⊇ the API's gx_code" | the containment, read out of 44 §2.3 | (sem: SEM-gx-gate-171)
//! | **E-7 fires** (§37 M5-04 adopted (b)) | `EVIDENCE_INSUFFICIENT` out of the vocabulary, the const kept with a retirement note | [`the_retired_code_is_out_of_the_vocabulary_and_refused_at_construction`] | (sem: SEM-gx-gate-172)
//! | **E-M2-23** | "put a crate's Error vocabulary table in 41 §6" | [`gx_gate::ERROR_KINDS`] against the enum in the source | (sem: SEM-gx-gate-173)
//! | C-5 (hand 2) / D-3 (hand 3) | `PolicySetUnreadable` and `RegistryUnusable` in that table | two rows of the same assertion |
//! | **M3-21** / ASM-60-4 | one constant, and "an overrun is diverted to a digest at construction" | the bound, at both places 42 §3.8 asks for a message | (sem: SEM-gx-gate-174)
//!
//! # What a vocabulary is worth, and what it costs
//!
//! The check runs at construction rather than at verification. That is the H5-8 shape M2 used for
//! `VERDICT_KINDS` and E-M3-2 later turned into a type, and the reason is the same: a check that
//! runs when a value is *read* can be reached by a value that was already written down. A `Reason`
//! carrying `POLCIY_DENIED` would reach an `AdmitProof`, a CID and a receipt before anything
//! looked at it.

use std::path::{Path, PathBuf};

use gx_gate::{
    verdict::{elide, InvariantResult},
    Error, Reason, ReasonSource, ERROR_KINDS, EVIDENCE_INSUFFICIENT, INVARIANT_VIOLATED,
    INVERSE_UNAVAILABLE, MAX_MESSAGE_BYTES, POLICY_DENIED, REASON_CODES,
};

fn workspace_root() -> PathBuf {
    // <root>/crates/gx-gate -> <root>
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

fn spec(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The `gx_code` column of 44 §2.3, read from the spec rather than restated.
fn gx_codes_from_44() -> Vec<String> {
    let text = spec("req/spec/40-architecture/44-api-spec.md");
    let table = text
        .split("`gx_code`\u{4e00}\u{89a7}:") // (sem: SEM-gx-gate-175)
        .nth(1)
        .expect("44 §2.3 still introduces the table with this line");
    let mut codes = Vec::new();
    for line in table.lines() {
        let line = line.trim();
        if !line.starts_with("| `") {
            if !codes.is_empty() {
                break; // the table ended
            }
            continue;
        }
        let cell = line
            .trim_start_matches("| `")
            .split('`')
            .next()
            .expect("a fenced code in the first column");
        codes.push(cell.to_string());
    }
    assert_eq!(
        codes.len(),
        12,
        "44 §2.3 held twelve gx_codes when req/60 §3 measured it (M3-08); it now holds {}",
        codes.len()
    );
    codes
}

// ---------------------------------------------------------------------------
// One declaration (**E-M3-6**)
// ---------------------------------------------------------------------------

/// The table is sorted, deduplicated, and spelled the way its constants are.
#[test]
fn the_reason_codes_are_one_sorted_declaration() {
    let mut sorted = REASON_CODES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted,
        REASON_CODES.to_vec(),
        "REASON_CODES is sorted and holds no duplicate"
    );
    assert_eq!(
        REASON_CODES.to_vec(),
        vec![INVARIANT_VIOLATED, INVERSE_UNAVAILABLE, POLICY_DENIED],
        "every live constant in the crate is in the table and nothing else is -- \
         EVIDENCE_INSUFFICIENT was retired by E-7 (§37 M5-04 adopted (b)) and is checked separately" // (sem: SEM-gx-gate-176)
    );
}

// ---------------------------------------------------------------------------
// 🔴 **E-7 fires** — the retirement, and what a retirement has to be able to show (sem: SEM-gx-gate-177)
// ---------------------------------------------------------------------------

/// `EVIDENCE_INSUFFICIENT` is out of the vocabulary, still spelled once, and now refused.
///
/// req/38 §37's ruling on **M5-04 adopted (b)** fires the kill condition E-7 armed in M3 and re-armed in
/// M4, and it fixes the shape a retirement takes here ("retirement mark"): (sem: SEM-gx-gate-178)
///
/// > `EVIDENCE_INSUFFICIENT` is **retired** from the gate vocabulary (retirement mark = removed from
/// > the vocabulary table; the const stays, with a retirement doc; error_vocabulary test updated the
/// > same turn) (sem: SEM-gx-gate-179)
///
/// Three assertions, because a retirement that only deleted a name would be indistinguishable from
/// a name that was never there:
///
/// 1. **Out of [`REASON_CODES`]** — the vocabulary is the three that have producers.
/// 2. **Still declared, exactly once** — "the const stays, with a retirement doc" (sem: SEM-gx-gate-180). A reader who finds the
///    word in an old receipt, an old req doc or `tools/verify_m3h4.sh` can still land on a
///    definition that says when it stopped being usable and why. Deleting it would turn every one
///    of those references into a dead end, which is what `no-delete` exists to prevent.
/// 3. **Refused at construction** — this is the half that makes the retirement a *fact about the
///    build* rather than a note. `Reason::new` checks membership of [`REASON_CODES`], so a code out
///    of the table cannot reach an `AdmitProof`, a CID or a receipt. Before this hand the same call
///    succeeded; that difference is the whole of what "retired" buys. (sem: SEM-gx-gate-181)
#[test]
fn the_retired_code_is_out_of_the_vocabulary_and_refused_at_construction() {
    assert!(
        !REASON_CODES.contains(&EVIDENCE_INSUFFICIENT),
        "E-7 fired: EVIDENCE_INSUFFICIENT is retired from the gate's Reason vocabulary"
    );
    println!(
        "RETIRED_CODES=[\"{EVIDENCE_INSUFFICIENT}\"] LIVE_REASON_CODES={}",
        REASON_CODES.len()
    );

    let src = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/lib.rs"))
        .expect("lib.rs is readable");
    assert_eq!(
        src.matches("pub const EVIDENCE_INSUFFICIENT").count(),
        1,
        "the const stays, with its retirement note: no-delete, and every old reference to the \
         word must still resolve to a definition that explains it"
    );
    assert!(
        src.contains("🔴 **retired**"), // (sem: SEM-gx-gate-169)
        "the surviving const carries the retirement mark in its own documentation"
    );

    let built = Reason::new(
        EVIDENCE_INSUFFICIENT,
        "a policy wanted evidence".to_string(),
        ReasonSource::Precondition,
    );
    match built {
        Err(Error::UnknownReasonCode { code }) => assert_eq!(code, EVIDENCE_INSUFFICIENT),
        other => panic!(
            "a retired code that can still be constructed is not retired: {other:?}\n\
             (this call succeeded until §37 M5-04 adopted (b) fired E-7)" // (sem: SEM-gx-gate-182)
        ),
    }
}

/// Each code is spelled once in `src/`, and the spelling is its own name.
///
/// The idiom is hand 2's `POLICY_DENIED in src = 1` check, widened to the whole vocabulary now that
/// there is one. A second literal is how two vocabularies start.
#[test]
fn each_code_is_spelled_once_in_the_source() {
    let src = workspace_root().join("crates/gx-gate/src");
    let mut text = String::new();
    for entry in std::fs::read_dir(&src).expect("src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|x| x == "rs") {
            text.push_str(&std::fs::read_to_string(&path).expect("readable"));
        }
    }
    for code in REASON_CODES {
        let literal = format!("\"{code}\"");
        let count = text.matches(&literal).count();
        assert_eq!(
            count, 1,
            "{code} is written as a string literal {count} times; the vocabulary is one \
             declaration (E-M3-6)"
        );
    }
}

// ---------------------------------------------------------------------------
// The refusal ("a code outside the declaration is refused at construction") (sem: SEM-gx-gate-183)
// ---------------------------------------------------------------------------

/// A code outside the table is refused when the `Reason` is built.
#[test]
fn a_code_outside_the_table_is_refused_at_construction() {
    for outside in [
        "POLCIY_DENIED",       // a misspelling
        "policy_denied",       // the right word, the wrong case
        "NOT_ADMITTED",        // 44 §2.3's, but a commit-side code and not a reason
        "INVARIANT_VIOLATED ", // trailing space
        "",                    // nothing at all
    ] {
        let built = Reason::new(outside, "a message".to_string(), ReasonSource::Precondition);
        match built {
            Err(Error::UnknownReasonCode { code }) => assert_eq!(code, outside),
            other => panic!("{outside:?} was accepted or refused wrongly: {other:?}"),
        }
    }
}

/// Every code in the table is accepted, and comes back as itself.
#[test]
fn every_declared_code_builds_a_reason() {
    for code in REASON_CODES {
        let reason = Reason::new(code, "why".to_string(), ReasonSource::Precondition)
            .unwrap_or_else(|e| panic!("{code} is in the table and was refused: {e}"));
        assert_eq!(reason.code(), code);
        assert_eq!(reason.message(), "why");
    }
}

// ---------------------------------------------------------------------------
// The containment: "the gate's reason codes ⊇ the API's gx_code" (M3-08) (sem: SEM-gx-gate-184)
// ---------------------------------------------------------------------------

/// The names M3-08 creates are **not** in 44 §2.3, which is why they had to be created.
///
/// req/60 §3's measurement, held as an assertion: 44 §2.3's twelve codes have "not a single code
/// that represents 'a policy denied' / 'an invariant broke'". `NOT_ADMITTED` is the commit side
/// ("cannot commit when Verdict≠Admit") and `POLICY_ERROR` is the broken side ("a failure of the
/// Cedar evaluation itself") (sem: SEM-gx-gate-185). If a future
/// edit of 44 adds one of these, this test says so -- an erratum that has been fixed should stop
/// being carried as one.
///
/// M3-08 created **three**; **two** are left. The retired third is still checked here, because the
/// reason it is absent from 44 has not changed and a future 44 that grew an evidence code would be
/// the one fact that could revive it (§37's ruling closes the window, but it closes it on a
/// premise, and a premise is worth a probe).
#[test]
fn the_gate_codes_are_absent_from_44() {
    let api = gx_codes_from_44();
    for code in [POLICY_DENIED, INVARIANT_VIOLATED] {
        assert!(
            !api.contains(&code.to_string()),
            "{code} is now in 44 §2.3; M3-08's premise has changed"
        );
    }
    assert!(
        !api.contains(&EVIDENCE_INSUFFICIENT.to_string()),
        "44 §2.3 grew an evidence code after E-7 retired the gate's; that is the one fact that \
         would reopen §37 M5-04's window"
    );
}

/// The one code the gate borrows **is** 44 §2.3's, spelled identically.
///
/// This is the containment in the only direction a test can check from here: where the API already
/// has the word, the gate uses that word. `INVERSE_UNAVAILABLE` is 44's "the inverse delta for the
/// undo target is unavailable" (sem: SEM-gx-gate-186) and E-M3-4's escalation reason is exactly that fact, so a synonym would have been a second
/// name for one thing.
#[test]
fn the_borrowed_code_is_44s_own_spelling() {
    let api = gx_codes_from_44();
    assert!(
        api.contains(&INVERSE_UNAVAILABLE.to_string()),
        "44 §2.3 no longer carries INVERSE_UNAVAILABLE, which E-M3-4's reason borrows"
    );
}

// ---------------------------------------------------------------------------
// **E-M2-23**: the crate's error table
// ---------------------------------------------------------------------------

/// [`gx_gate::ERROR_KINDS`] names every variant of `Error`, and no others.
///
/// The variants are read out of `src/lib.rs` rather than matched on, for the reason A-10's
/// `Checkpoint` key count is read out of an encoder: a `match` here would be updated in the same
/// edit that added a variant, and would therefore never notice one. E-M2-23 asks for the table to
/// live in 41 §6; the spec is not this hand's to write (52), so it lives beside the enum and this
/// assertion is what keeps the two in step. The erratum stays open (req/64 §4).
#[test]
fn the_error_table_names_every_variant() {
    let src = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/lib.rs"))
        .expect("lib.rs is readable");
    let body = src
        .split("pub enum Error {")
        .nth(1)
        .expect("lib.rs still declares `pub enum Error`");
    let body = body.split("\n}").next().expect("the enum is closed");

    let mut variants: Vec<String> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        // A variant line starts at one indent level with an upper-case letter: `Unevaluable {`,
        // `EmptyDeny,`. Attributes, doc comments and field lines are none of those.
        if !line.starts_with("    ") || line.starts_with("     ") {
            continue;
        }
        let Some(first) = trimmed.chars().next() else {
            continue;
        };
        if !first.is_ascii_uppercase() {
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
    variants.sort();
    variants.dedup();

    assert_eq!(
        variants,
        ERROR_KINDS.to_vec(),
        "the Error enum and ERROR_KINDS disagree; one of them was edited alone"
    );
    for expected in ["PolicySetUnreadable", "RegistryUnusable"] {
        assert!(
            ERROR_KINDS.contains(&expected),
            "{expected} is C-5's or D-3's and belongs in the table"
        );
    }
}

// ---------------------------------------------------------------------------
// **M3-21** / ASM-60-4: one bound, two places
// ---------------------------------------------------------------------------

/// The bound is 1024 bytes and it is declared once.
#[test]
fn the_bound_is_asm_60_4s_initial_value() {
    assert_eq!(MAX_MESSAGE_BYTES, 1024, "ASM-60-4's initial value");
    let src = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/lib.rs"))
        .expect("readable");
    assert_eq!(
        src.matches("pub const MAX_MESSAGE_BYTES").count(),
        1,
        "one declaration, so that Owner changing the number is one edit"
    );
}

/// A message inside the bound is kept exactly; one byte over it is replaced by a digest.
#[test]
fn a_message_over_the_bound_becomes_a_digest() {
    let at_bound = "a".repeat(MAX_MESSAGE_BYTES);
    let over = "a".repeat(MAX_MESSAGE_BYTES + 1);

    let kept = Reason::new(POLICY_DENIED, at_bound.clone(), ReasonSource::Precondition)
        .expect("at the bound");
    assert_eq!(kept.message(), at_bound, "the bound is inclusive");

    let elided = Reason::new(POLICY_DENIED, over.clone(), ReasonSource::Precondition)
        .expect("over the bound is elided, not refused");
    assert_ne!(elided.message(), over);
    assert!(elided.message().len() < MAX_MESSAGE_BYTES);
    assert!(
        elided.message().contains("gx1:"),
        "42 §3.8's \"long text gets digested\": the record still identifies what was said -- {}", // (sem: SEM-gx-gate-187)
        elided.message()
    );
    assert!(
        elided
            .message()
            .contains(&(MAX_MESSAGE_BYTES + 1).to_string()),
        "and how much of it there was"
    );
}

/// An invariant's `detail` is bounded by the same constant, at its own constructor.
///
/// This is the half that matters in practice: `detail` is written by a third party -- an
/// `InvariantCheck` a deployment registered -- and it reaches a receipt's CID through
/// `AdmitProof`'s IdentityView (42 §1.3).
#[test]
fn an_invariant_detail_over_the_bound_becomes_a_digest() {
    let over = "x".repeat(MAX_MESSAGE_BYTES * 4);
    let result = InvariantResult::new("chatty".to_string(), false, Some(over.clone()))
        .expect("verbosity is not ⊥");
    let detail = result.detail().expect("still recorded, in short");
    assert!(detail.len() < MAX_MESSAGE_BYTES);
    assert!(detail.contains("gx1:"));

    let short = InvariantResult::new("terse".to_string(), false, Some("no".to_string()))
        .expect("inside the bound");
    assert_eq!(short.detail(), Some("no"));
}

/// The same text elides to the same line, and different texts to different lines.
///
/// The elision is a digest and not a truncation, which is the whole of its value: two 4 KB details
/// that agree for their first kilobyte are told apart, and an operator holding the original can
/// recompute the line.
#[test]
fn the_elision_identifies_the_text_it_replaced() {
    let base = "y".repeat(MAX_MESSAGE_BYTES + 1);
    let mut other = base.clone();
    other.push('z');

    let one = elide(base.clone()).expect("digestible");
    let again = elide(base).expect("digestible");
    let different = elide(other).expect("digestible");

    assert_eq!(one, again, "the same message elides to the same line");
    assert_ne!(one, different, "a different message elides differently");
}

/// A multi-byte character is never cut in half: the whole message is replaced or none of it is.
#[test]
fn the_bound_never_splits_a_character() {
    let text = "\u{3042}".repeat(MAX_MESSAGE_BYTES); // three bytes each, well over the bound (sem: SEM-gx-gate-188)
    assert!(text.len() > MAX_MESSAGE_BYTES);
    let elided = elide(text).expect("digestible");
    assert!(
        elided.is_char_boundary(elided.len()) && !elided.contains('\u{3042}'), // (sem: SEM-gx-gate-189)
        "the text was replaced rather than truncated"
    );
}
