// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R37 / `req/501` M-03 + L-01** — the red bed for two spelling-dependent judgements,
//! written **before** the repair (`req/38` §226).
//!
//! # M-03 — `declares_its_default`'s header read
//!
//! Audit 36 (`req/496` §4-3) measured five shapes of one sentence against
//! [`gx_gate::packs::declares_its_default`]. Four of the five answers were wrong in the same
//! direction the clause exists to prevent:
//!
//! | shape | before the repair | what it must be |
//! |---|---|---|
//! | `control_own_line` | accepted | accepted |
//! | `trailing_comment` | **refused** | accepted |
//! | `wrapped_comment` | **refused** | accepted |
//! | `annotation_value` | **refused** | accepted |
//! | `negated_comment` | **accepted** | refused |
//!
//! The last row is the one that matters: a header that says the pack does *not* declare a
//! deny-default was read as declaring one, because `str::contains` is a substring test with no
//! subject and no polarity.
//!
//! # L-01 — the completeness arm's extractor
//!
//! `crates/gx-gate/tests/decided_at_seat.rs`'s `reads_of` finds a read by the literal `input.`,
//! under a note claiming *"there is no spelling it could take that this would not see"*. Audit 36
//! (`req/496` §4-5) spelled the same read three ways without that literal and the arm saw none of
//! them.
//!
//! The repair this bed is written against does **not** make a text scan into a Rust parser. It
//! makes the projection's convention a checkable thing: every spelling is either **caught** by the
//! extractor or **refused** by a convention guard, so no spelling is silently invisible. This
//! suite asserts that disjunction rather than asserting the extractor grew.

use gx_gate::packs::{declares_its_default, ShippedPack, DENY_DEFAULT_DECLARATION};

// ---------------------------------------------------------------------------
// M-03 — one sentence, five places to put it
// ---------------------------------------------------------------------------

fn judge(name: &str, source: &'static str) -> bool {
    let pack = ShippedPack {
        path: "policies/r37/edge.cedar",
        source,
        policy_ids: &["fs-forbid-something"],
        substrate: "fs",
    };
    let verdict = declares_its_default(&pack);
    println!(
        "R37_F3 shape={name} accepted={} detail={:?}",
        verdict.is_ok(),
        verdict.as_ref().err().map(std::string::ToString::to_string)
    );
    verdict.is_ok()
}

/// The five shapes of `req/496` §4-3, with the answers the repair owes each of them.
#[test]
fn r37_f3_reads_the_declaration_rather_than_its_typography() {
    println!("R37_F3_DECL needle={DENY_DEFAULT_DECLARATION:?}");

    // The control. If this is refused the bed broke before the product did.
    let control = judge(
        "control_own_line",
        "// # This pack declares a **deny-default**. It has no permit statement.\n\
         @id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
    );
    assert!(
        control,
        "the bed failed before the product did: the shape R36 ships as accepted was refused"
    );

    let trailing = judge(
        "trailing_comment",
        "@id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" }; \
         // This pack declares a **deny-default**. It has no permit statement.\n",
    );
    let wrapped = judge(
        "wrapped_comment",
        "// # This pack declares a\n\
         // **deny-default**. It has no permit statement.\n\
         @id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
    );
    let annotated = judge(
        "annotation_value",
        "@id(\"fs-forbid-something\")\n\
         @doc(\"This pack declares a **deny-default**. It has no permit statement.\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
    );
    let negated = judge(
        "negated_comment",
        "// This pack does NOT say that it \"declares a **deny-default**\" - nobody decided its\n\
         // default and a reader must not rely on one.\n\
         @id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
    );

    println!(
        "R37_F3_SUMMARY control={control} trailing={trailing} wrapped={wrapped} \
         annotated={annotated} negated={negated}"
    );

    assert!(
        trailing,
        "🔴 req/496 M-03: the declaration written as a trailing comment on a statement's own line \
         is the same declaration. `policies/PACK_FORMAT.md` asks for `a comment line containing` \
         the phrase and this is one"
    );
    assert!(
        wrapped,
        "🔴 req/496 M-03: a comment a formatter wrapped across two lines carries the same sentence"
    );
    assert!(
        annotated,
        "🔴 req/496 M-03: the declaration carried in a Cedar annotation value is inside the pack \
         and a reader of the pack sees it"
    );
    assert!(
        !negated,
        "🔴 req/496 M-03 (fail-open): a header that says the pack does **not** declare a \
         deny-default was accepted as declaring one. F3 exists so that a pack picks its default \
         out loud, and this header picks nothing"
    );
}

/// The negative controls for the repair: shapes that must **stay** refused, so that the fix is
/// not "accept anything that mentions the phrase".
#[test]
fn r37_f3_still_refuses_a_pack_that_says_nothing() {
    let silent = judge(
        "no_declaration_at_all",
        "@id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
    );
    assert!(
        !silent,
        "negative control: a pack with no permit statement and no declaration must stay refused"
    );

    // A pack that declares a deny-default **and** ships a permit is the contradiction R36 added,
    // and it must survive a change to how the declaration is read.
    let pack = ShippedPack {
        path: "policies/r37/contradiction.cedar",
        source:
            "// This pack declares a **deny-default**.\n\
                 @id(\"fs-permit-default\")\n\
                 permit ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
        policy_ids: &["fs-permit-default"],
        substrate: "fs",
    };
    let verdict = declares_its_default(&pack);
    println!(
        "R37_F3 shape=declares_and_permits accepted={} detail={:?}",
        verdict.is_ok(),
        verdict.as_ref().err().map(std::string::ToString::to_string)
    );
    assert!(
        verdict.is_err(),
        "negative control: the both-ways contradiction R36 added must stay refused"
    );
}

// ---------------------------------------------------------------------------
// L-01 — every spelling is either seen or refused
// ---------------------------------------------------------------------------

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the workspace root is two levels above this crate")
        .to_path_buf()
}

/// `body_of`, copied from `decided_at_seat.rs` so this suite reads the same span that arm reads.
fn body_of(source: &str, signature: &str) -> String {
    let at = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature:?} is not in this file"));
    let open = source[at..]
        .find('{')
        .expect("a function signature is followed by a brace")
        + at;
    let bytes = source.as_bytes();
    let mut depth = 0i32;
    let mut end = open;
    while end < bytes.len() {
        match bytes[end] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        end += 1;
    }
    source[open..=end.min(bytes.len() - 1)].to_string()
}

/// The three spellings of `req/496` §4-5, plus R36's own control spelling.
///
/// This suite reads the two functions **out of `decided_at_seat.rs`'s source** rather than
/// re-implementing them, because a copy is the thing that goes stale: audit 36 re-implemented
/// `frames()` from memory and burned two runs on it (`req/496` §7 item 1). The disjunction under
/// test is a property of the shipped arm, so the shipped arm is what is driven — through the
/// public census below, which asserts the guard exists and is called.
#[test]
fn r37_l01_the_extractor_and_its_convention_guard_are_wired_together() {
    let seat =
        std::fs::read_to_string(workspace_root().join("crates/gx-gate/tests/decided_at_seat.rs"))
            .expect("decided_at_seat.rs is readable");

    // 1. The over-claim `req/496` §4-5 quoted must no longer be **asserted**. It may still be
    //    quoted: no-delete asks a repair to keep the sentence it is correcting where a reader can
    //    see what was wrong, and the first version of this arm could not tell the two apart — it
    //    refused the repair's own note explaining the retraction. So the test is that every
    //    occurrence sits inside quotation marks.
    let over_claim = "there is no spelling it could take that this would not see";
    let occurrences: Vec<usize> = seat.match_indices(over_claim).map(|(at, _)| at).collect();
    let asserted: Vec<usize> = occurrences
        .iter()
        .copied()
        .filter(|at| *at == 0 || seat.as_bytes()[at - 1] != b'"')
        .collect();
    println!(
        "R37_PS1_CENSUS over_claim_occurrences={} asserted_unquoted={}",
        occurrences.len(),
        asserted.len()
    );
    assert!(
        asserted.is_empty(),
        "🔴 req/496 L-01: `decided_at_seat.rs` still **asserts** the universal claim three \
         spellings falsify (unquoted at {asserted:?}). The repair is allowed to keep the simple \
         scan and to quote the retracted sentence; it is not allowed to go on saying it"
    );

    // 2. The guard must exist and `ps_1` must call it, or the disjunction "caught or refused" has
    //    no second half.
    let guard = "fn spellings_the_scan_cannot_see";
    println!("R37_PS1_CENSUS guard_present={}", seat.contains(guard));
    assert!(
        seat.contains(guard),
        "🔴 req/496 L-01: no convention guard in `decided_at_seat.rs`. Without one, a read spelled \
         `let g = input; g.judged_at.0` is invisible to the completeness arm and refused by nobody"
    );
    let ps1 = body_of(
        &seat,
        "fn ps_1_the_cedar_projection_does_not_read_decided_at() {",
    );
    println!(
        "R37_PS1_CENSUS ps_1_calls_guard={}",
        ps1.contains("spellings_the_scan_cannot_see")
    );
    assert!(
        ps1.contains("spellings_the_scan_cannot_see"),
        "🔴 req/496 L-01: the guard exists and `ps_1` does not call it, which is a gate a release \
         can be cut around"
    );

    // 3. And the mutant suite that drives the three spellings must be there — a guard nobody has
    //    seen refuse is a guard on paper (`decided_at_seat.rs`'s own words about `ps_1b`).
    let driven = "fn ps_1c_every_spelling_is_either_seen_or_refused";
    println!(
        "R37_PS1_CENSUS three_spelling_suite_present={}",
        seat.contains(driven)
    );
    assert!(
        seat.contains(driven),
        "🔴 req/496 L-01: nothing drives `rebound_local` / `destructured` / \
         `whitespace_around_the_dot` against the shipped arm"
    );
}

// ---------------------------------------------------------------------------
// 🔴 **R39 / `req/533` L-01** — the narrowing's error directions, all three of them
// ---------------------------------------------------------------------------

/// 🔴 **R39 / `req/533` L-01** — `declaration_surface` is wrong in three directions, and until this
/// suite none of them was driven.
///
/// R38 narrowed the F3 surface from the whole file to "the two places that are the pack talking
/// about itself": comment text, and annotation string values. Its doc-comment then declared **one**
/// residue — a `//` inside a string literal opens a comment as far as the reader is concerned — and
/// `req/519` §10-7 said in as many words that no fixture drove it. Audit 38 built one and it landed.
/// It also found two more that nobody had declared: the annotation sweep walks `source`, not the
/// comment surface, so an `@name("…")` **inside a string literal** and an `@name("…")` **inside a
/// comment body** are both read as the pack's own statement about itself.
///
/// All three are driven here, and the doc-comment on `declaration_surface` now names three.
///
/// # 🔴 Why this stays a declaration and not a repair
///
/// `req/540` R-5c: the narrowing is **not** strengthened. Closing these shapes means knowing where
/// Cedar's string literals and comments are, which means a second parser beside Cedar's own — the
/// drift `E-M2-12` exists to prevent — and the blast radius is nil: `declares_its_default` runs over
/// `SHIPPED_PACKS`, three packs embedded with `include_str!`, so no third party's text reaches it.
/// A residue that is declared, driven and bounded is a different object from one that is declared
/// and unmeasured, and this lane is about the difference.
#[test]
fn r39_the_narrowing_is_wrong_in_three_directions_and_every_one_of_them_is_driven() {
    // Direction (a): a `//` inside a string literal. `s3://` and `https://` appear in real locators,
    // so this is not a contrived spelling — it is the shape a pack author writes by accident.
    let slashes_in_string = judge(
        "a_declared_residue_slashes_in_a_string_literal",
        "@id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  \
         when { resource.locator like \"s3://bucket/ this pack declares a **deny-default** *\" };\n",
    );
    assert!(
        slashes_in_string,
        "🔴 `req/533` L-01(a): this is the residue R38 declared and `req/519` §10-7 said it had not \
         driven. If it is now refused the narrowing grew, which `req/540` R-5c says it may not have \
         without a ruling — check whether a second Cedar parser arrived with it"
    );

    // Direction (b): an annotation form inside a string literal. Nowhere declared before R39.
    let at_in_string = judge(
        "an_undeclared_residue_an_at_form_inside_a_string_literal",
        "@id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  \
         when { resource.locator like \"@doc(\\\"this pack declares a **deny-default**\\\")*\" };\n",
    );
    assert!(
        at_in_string,
        "🔴 `req/533` L-01(b): the annotation sweep reads `source` and not the comment surface, so \
         an `@name(\"…\")` written inside a condition's operand is taken as the pack's statement \
         about itself. If this is now refused, say so in `declaration_surface`'s doc — a residue \
         that closed silently is a doc-comment that has started lying in the safe direction"
    );

    // Direction (c): an annotation form inside a comment body. Also nowhere declared before R39.
    let at_in_comment = judge(
        "an_undeclared_residue_an_at_form_inside_a_comment",
        "// An example a reader is meant to ignore: @doc(\"this pack declares a **deny-default**\")\n\
         @id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
    );
    assert!(
        at_in_comment,
        "🔴 `req/533` L-01(c): the same sweep reads annotation forms out of comment text, so a \
         comment *about* a declaration is read as one"
    );

    // 🔴 The negative control. Without it every assertion above is satisfied by a function that
    // answers `true` for everything, which is the failure mode `declares_its_default` exists to
    // prevent in the first place — a pack that never picked its default being accepted.
    let silent = judge(
        "control_a_pack_that_says_nothing_at_all",
        "@id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  \
         when { resource.locator like \"s3://bucket/*\" };\n",
    );
    assert!(
        !silent,
        "🔴 the control: a pack whose source carries `//` inside a string literal and no \
         declaration anywhere must still be refused. If this is accepted the residues above are \
         measuring an accept-everything function rather than three specific holes"
    );

    // 🔴 The second control: polarity still works on the residue road. A residue that carried a
    // *negated* sentence into the surface would be accepted by a check that had stopped reading,
    // and `req/496` M-03 is the audit that found exactly that.
    let negated_in_string = judge(
        "control_a_negated_sentence_in_a_string_literal",
        "@id(\"fs-forbid-something\")\n\
         forbid ( principal, action, resource )\n  \
         when { resource.locator like \"s3://bucket/ this pack does not declare a **deny-default** *\" };\n",
    );
    assert!(
        !negated_in_string,
        "🔴 the residue road still reads polarity: `DENIALS` is applied to the sentence after the \
         surface is taken, so a residue cannot smuggle in a declaration the text denies"
    );
}
