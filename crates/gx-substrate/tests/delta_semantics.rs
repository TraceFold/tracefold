// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **M4-07, adopted (c)** — the composite-delta section of the crate documentation, measured clause
//! by clause. (sem: SEM-gx-substrate-141, SEM-gx-substrate-142, SEM-gx-substrate-143,
//! SEM-gx-substrate-144, SEM-gx-substrate-145, SEM-gx-substrate-146, SEM-gx-substrate-147,
//! SEM-gx-substrate-148, SEM-gx-substrate-149, SEM-gx-substrate-150, SEM-gx-substrate-151,
//! SEM-gx-substrate-152, SEM-gx-substrate-153, SEM-gx-substrate-154, SEM-gx-substrate-155,
//! SEM-gx-substrate-156, SEM-gx-substrate-157, SEM-gx-substrate-158)
//!
//! `req/38_ERRATA_2026-08-07.md` §28, verbatim: "**M4-07, adopted (c)** (mathematics; D-8's hole
//! settled): a composite delta is a **free monoid** -- the fs payload is a 'sequence of single-file
//! operations', and the concatenation of the sequence is the witness of composition. Not one line of
//! the trait changes (P-6 untouched, F1 unfired). Document plainly that **no general law is claimed
//! (the delta of a composite arrow = the composition of its parts)**".
//!
//! # Why the ruling lands in prose and gets a test anyway
//!
//! There is nothing to implement. The whole content of M4-07 (c) is that the trait does **not** grow
//! a `compose_delta`, that a composite lives inside a payload no layer above the adapter may read
//! (P-6), and that gx makes no general claim relating the delta of a composite arrow to the deltas of
//! its parts. Every one of those is an absence, and an absence is exactly what a later hand deletes
//! without noticing. So the decision is written where an implementor meets it -- the crate root, for
//! the reason `substrate_contract.rs` gives about the locator contract -- and this file is what makes
//! it a requirement.
//!
//! The precedent is `substrate_contract.rs` itself (H-2 / E-M4-12, hand 1) and, one crate over,
//! `crates/gx-gate/tests/pack_embedding.rs`'s `the_pack_module_states_its_effective_range`.
//!
//! # What is deliberately not checked
//!
//! That any adapter builds a composite. No adapter exists (hand 4), and the free monoid is a
//! statement about what a payload is *allowed* to be rather than about what any payload *is*. The
//! law that can be measured over an implementation is L1-L7 in `gx-substrate-conformance`, and none
//! of them is "the composite of two deltas is the concatenation" -- because M4-07 (c) rules that
//! claim out rather than in.

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

/// Only the crate documentation, for the reason `substrate_contract.rs` states: an implementor of
/// `SubstrateAdapter` opens the crate root, and a normative section folded under one item binds
/// whoever happened to open that item.
fn crate_doc(source: &str) -> String {
    source
        .lines()
        .filter(|l| l.trim_start().starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The heading the section sits under, so that a later section cannot be read as this one.
const SECTION_HEADING: &str = "# Composite deltas (normative)";

/// The section itself, **with the quoted ruling removed**.
///
/// 🔴 The restriction was earned. Written against the whole crate documentation, the clause probes
/// below survived mutation (a) of `tools/verify_m4h3.sh`: gx's own sentence "no general law is
/// claimed (the delta of a composite arrow = the composition of its parts)" was rewritten into a
/// description, and every clause was still found -- in the block quote of **M4-07, adopted (c)**
/// three paragraphs above, where the same words appear because the section quotes the ruling
/// verbatim.
///
/// That is req/71 §2 M4H2-6's lesson arriving a second time: "**'written somewhere' is not 'written
/// in that place'**". A quoted ruling is evidence of what was decided; what binds an implementor
/// is the crate's own prose. So the quotation (`> ` lines) is dropped, and what remains is what a
/// reader would act on.
fn ruling_section(source: &str) -> String {
    let doc = crate_doc(source);
    let after = doc
        .split(SECTION_HEADING)
        .nth(1)
        .unwrap_or_else(|| panic!("the crate documentation has no `{SECTION_HEADING}` section"));
    after
        .lines()
        .map(|l| l.trim_start_matches("//!").trim())
        .filter(|l| !l.starts_with('>'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What **M4-07, adopted (c)** decided, each clause word for word.
///
/// Split into a list rather than checked as a paragraph so that dropping one is a failing test
/// naming the clause that went missing. The first four are the ruling itself; the last one is the
/// sentence the ruling asks to be written down explicitly, which is a *refusal* to state a law and
/// therefore the clause most likely to be quietly improved away.
///
/// 🔴 The seventh token is **the minimal form of K-2, adopted (a)** (`req/38` §35), and it was earned
/// by a mutation the audit hand fired at this section. req/76 §2.1 (B-2) rewrote numbered clause 4
/// into "T2 was established in this milestone" and **every probe in this file stayed green**: the
/// six tokens above are all about clauses 1-3, and the one probe that mentions T2 asks only that the
/// string `T2` appear -- which an overclaim keeps. So 45 §4's "the sentence that must not be said"
/// was, in this section, unguarded in the
/// exact direction 45 §4 forbids: a milestone could claim a formal result it does not have by
/// editing one line of prose. The token below is the clause's own heading, so the rewrite that
/// claims the result deletes the token that says it is a candidate.
const RULING_CLAUSES: [&str; 7] = [
    "free monoid",
    "single-file operations",
    "the concatenation of the sequence is the witness",
    "No general law is claimed",
    "not one line of the trait changes",
    "M4-07",
    "T2 stays a candidate rather than a result",
];

/// The three systems req/69 §9 lists as the comparison, and the word that carries each contrast.
///
/// `research_bus/glovrex_m4/SURVIVORS.md` §B is a **raw reference** under the external-source rule,
/// so the primary URLs behind these three were re-opened by this hand and recorded in
/// `Desktop/GitRepo/REFERENCES.md` before any of them became load-bearing. What is taken is the
/// *shape of the difference* and never a line of code: gx's inverse is partial where darcs's is
/// total, gx stops at `Conflicts` where pijul resolves by pushout, and gx's `pre` argument is the
/// explicit base point git spells as a merge base.
const CONTRAST_CLAUSES: [(&str, &str); 3] = [
    ("darcs", "totality"),
    ("pijul", "pushout"),
    ("git", "snapshot"),
];

/// The two sentences that say what gx does instead. Named separately from the contrasts because a
/// comparison that describes three other systems and forgets to state gx's own position is a
/// literature review.
const GX_POSITION: [&str; 2] = [
    "gx's inverse = partial",
    "does not resolve the merge; stops at `Conflicts`",
];

// ---------------------------------------------------------------------------
// The section exists and is named
// ---------------------------------------------------------------------------

/// The ruling has a heading of its own, so a later hand can point at it by name.
#[test]
fn the_composite_delta_ruling_has_a_section_of_its_own() {
    assert!(
        crate_doc(&lib_rs()).contains(SECTION_HEADING),
        "the crate documentation has no `{SECTION_HEADING}` section; M4-07 (c) asks for the \
         decision to be written \"in the doc, plainly\" and a decision without a place is a \
         decision without a reader"
    );
}

// ---------------------------------------------------------------------------
// The ruling, clause by clause
// ---------------------------------------------------------------------------

/// Every clause of M4-07 (c) is in the section's **own prose**, word for word.
///
/// [`ruling_section`] is what makes "own" mean something: the quoted ruling is dropped first, so a
/// clause found here is a clause the crate says rather than one it cites.
#[test]
fn the_section_states_the_free_monoid_ruling() {
    let doc = ruling_section(&lib_rs());
    let mut missing: Vec<&str> = Vec::new();
    for clause in RULING_CLAUSES {
        if !doc.contains(clause) {
            missing.push(clause);
        }
    }
    println!(
        "M4_07_RULING_CLAUSES={} MISSING={}",
        RULING_CLAUSES.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "the composite-delta section does not state {missing:?} -- M4-07, adopted (c), is the decision that \
         closed D-8's hole, and the clause it is most tempting to drop is the one that refuses to \
         state a general law"
    );
}

/// The refusal is stated as a refusal, not implied by silence.
///
/// Separated from the loop above because it is the load-bearing half of the ruling. "the relation
/// between a composite arrow's delta and its parts' deltas" is exactly what req/69 §4 M4-07 measured
/// as undefined: the trait has
/// no composition, `gx_core::compose` takes the composite's delta as an argument (E-A7-2), so a
/// composite arrow may carry a delta unrelated to its parts and the `TransformationId` is still
/// well-formed. M4-07 (c) does not close that by adding a law -- it closes it by saying out loud that
/// there is none, which is the only honest reading while `compose` keeps its E-A7-2 signature.
#[test]
fn the_section_refuses_the_general_law_rather_than_omitting_it() {
    let doc = ruling_section(&lib_rs());
    assert!(
        doc.contains("No general law is claimed"),
        "the section does not refuse the general law in so many words; an unstated law and a \
         refused law read the same in prose and differently in a review (D-8 / E-M3-14)"
    );
    assert!(
        doc.contains("T2"),
        "the section does not name T2, whose first candidate this ruling decides (E-M3-14 (D-8): \
         \"the first candidate when M4 decides the composite delta's semantics\")"
    );
}

// ---------------------------------------------------------------------------
// The comparison, and gx's own position in it
// ---------------------------------------------------------------------------

/// The three systems req/69 §9 asks about are each named with the word that makes the contrast.
#[test]
fn the_section_places_gx_against_the_three_patch_theories() {
    let doc = ruling_section(&lib_rs());
    let mut missing: Vec<String> = Vec::new();
    for (system, word) in CONTRAST_CLAUSES {
        if !doc.contains(system) {
            missing.push(format!("{system} (not named)"));
        } else if !doc.contains(word) {
            missing.push(format!("{system}: {word}"));
        }
    }
    println!(
        "DELTA_CONTRASTS={} MISSING={}",
        CONTRAST_CLAUSES.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "the comparison is missing {missing:?} -- req/69 §9 items 1-3 exist so that \"the reason not \
         adopted\" \
         is written from the primary sources rather than assumed"
    );
}

/// gx's own two sentences are there, so the comparison says something about gx.
#[test]
fn the_section_states_where_gx_stands() {
    let doc = ruling_section(&lib_rs());
    let mut missing: Vec<&str> = Vec::new();
    for clause in GX_POSITION {
        if !doc.contains(clause) {
            missing.push(clause);
        }
    }
    assert!(
        missing.is_empty(),
        "the section describes other systems but does not state {missing:?}; a contrast with no \
         position is a reading list"
    );
}

// ---------------------------------------------------------------------------
// The absence the ruling depends on
// ---------------------------------------------------------------------------

/// P-6 survives the ruling: this crate still reads no payload.
///
/// M4-07 (c) is only free of the trait because the composite lives **inside** the payload, which is
/// opaque above the adapter. A shared list type in this crate -- `enum DeltaOp`, a decoder, anything
/// that gives the sequence a shape here -- would move the fs delta grammar into the boundary crate
/// and make the ruling's own premise false. `substrate_contract.rs` asserts that this crate opens no
/// file; this asserts that it decodes no payload.
#[test]
fn the_boundary_crate_still_reads_no_payload() {
    let dir = repo_root().join("crates/gx-substrate/src");
    let mut offenders: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("readable");
        for (n, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for forbidden in ["cbor::decode", "from_slice", "from_reader"] {
                if line.contains(forbidden) {
                    offenders.push(format!("{}:{}: {forbidden}", path.display(), n + 1));
                }
            }
        }
    }
    println!("SUBSTRATE_PAYLOAD_DECODES={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "the boundary crate decodes something: {offenders:?}. M4-07 (c) puts the composite in the \
         payload precisely because \"not one line of the trait changes (P-6 untouched)\", and a \
         decoder here would be \
         the delta grammar living above the adapters"
    );
}
