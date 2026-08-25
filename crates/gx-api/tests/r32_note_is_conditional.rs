// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R32 / `req/392` M-02 — the paragraph a face prints is a function of **which** term is false.
//!
//! # What the audit measured
//!
//! `gx_api::journal_note` took a `bool` that `Engine::journal_intact` folds **seven** terms into,
//! and returned one paragraph making four checkable factual claims. The audit built one bed per
//! condition it could reach and measured each claim rather than reading it:
//!
//! | bed | what the paragraph asserted | true? |
//! |---|---|---|
//! | a payload byte flipped (**control**) | the per-record chain is refusing to verify | yes (`journal_chain_break_at: 834`) |
//! | the eight marker bytes removed | the same | **no** (`chain_break_at: null`, `legacy`) |
//! | `GXJRNL99` written over the marker | the same | **no** — and this build's own source says of that condition *"nothing is wrong with the file, and this binary cannot verify it"* |
//! | all four | look for `<journal>.torn.<n>-<m>` after `gx repair --yes` | **no**, `4/4`, with the signing key supplied so the repair really ran |
//!
//! And `4/4` beds printed the roll-back clause — *"the journal and the ledger agree with each
//! other here ... Comparing the two files will find nothing"* — in the **same string** as the
//! paragraph saying the journal had moved, because six sites spelled `format!("{}{}", ..)`. The
//! roll-back clause's own doc, written when `req/229` H-01 was repaired, says it exists because in
//! its condition *"every word `journal_note` would print is false"*.
//!
//! # What is asserted
//!
//! This is the text-level gate, driven straight against the two functions. The end-to-end beds are
//! `crates/gx-cli/tests/r32_conditional_diagnosis.rs`.

use gx_engine::JournalDeparture;

const ALL: [JournalDeparture; 7] = [
    JournalDeparture::FromANewerGx,
    JournalDeparture::Downgraded,
    JournalDeparture::ChainBroken,
    JournalDeparture::Shortened,
    JournalDeparture::TailRewritten,
    JournalDeparture::PrefixRewritten,
    JournalDeparture::TornTail,
];

/// The list this suite walks is the list the engine declares, so a departure added there and not
/// here is a compile-time or an assertion failure rather than a silent hole.
#[test]
fn r32_every_declared_departure_is_covered_by_this_suite() {
    assert_eq!(
        ALL.len(),
        JournalDeparture::ALL_DEPARTURES.len(),
        "the engine declares {} departures and this suite walks {}",
        JournalDeparture::ALL_DEPARTURES.len(),
        ALL.len()
    );
    for (departure, name) in ALL.iter().zip(JournalDeparture::ALL_DEPARTURES) {
        assert_eq!(departure.kind(), name, "and in the same order");
    }
}

/// 🔴 Claim C2. *"since DR-43-9 this is the per-record chain refusing to verify"* is printed only
/// where a chain is refusing to verify.
#[test]
fn r32_the_chain_clause_is_printed_only_where_there_is_a_chain_to_refuse() {
    for departure in ALL {
        let note = gx_api::journal_note(Some(departure));
        let says_chain_refuses = note.contains("per-record chain refusing to verify");
        println!(
            "R32_C2 departure={} says_the_chain_is_refusing={says_chain_refuses}",
            departure.kind()
        );
        let should = matches!(departure, JournalDeparture::ChainBroken);
        assert_eq!(
            says_chain_refuses, should,
            "🔴 `req/392` M-02: this clause is a factual claim about a link that did not verify. \
             The audit measured it printed over a `legacy` file with no links at all, on 2 of the \
             3 beds it drove"
        );
    }
}

/// 🔴 Claim C3. The `.torn.` instruction is printed only for the one condition that produces one.
#[test]
fn r32_the_torn_file_instruction_is_printed_only_for_a_torn_tail() {
    for departure in ALL {
        let note = gx_api::journal_note(Some(departure));
        let sends_to_torn = note.contains("<journal>.torn.<n>-<m>");
        println!(
            "R32_C3 departure={} sends_the_operator_to_a_torn_file={sends_to_torn}",
            departure.kind()
        );
        assert_eq!(
            sends_to_torn,
            matches!(departure, JournalDeparture::TornTail),
            "🔴 `req/392` M-02 §3-2-2: DR-43-7 quarantines a **torn tail** and only when there is \
             no chain break. The audit drove `gx repair --yes` with a key on four beds and \
             measured `before=[] after=[]` on all four, while the paragraph told every one of them \
             to go and look for the file"
        );
    }
}

/// 🔴 §3-2-2's other half: the paragraph and `gx repair`'s remedy must not give opposite orders.
#[test]
fn r32_no_note_promises_a_cut_the_repair_says_it_will_not_make() {
    let broken = gx_api::journal_note(Some(JournalDeparture::ChainBroken));
    println!("R32_CUT chain_break_note={broken:?}");
    assert!(
        broken.contains("does not cut it"),
        "🔴 `gx repair --yes`'s remedy for this condition reads \"gx does not repair this and does \
         not cut it ... `--yes` leaves these bytes alone too\". Two shipped surfaces gave opposite \
         instructions about the same file"
    );
    assert!(
        !broken.contains("<journal>.torn.<n>-<m>"),
        "and it no longer sends the operator to a file DR-43-9 forbids creating here"
    );
}

/// Every arm says something, and no two arms say the same thing — a fold repaired into seven
/// copies of one sentence would pass every test above.
#[test]
fn r32_the_seven_arms_are_seven_sentences() {
    let mut seen: Vec<&str> = Vec::new();
    for departure in ALL {
        let note = gx_api::journal_note(Some(departure));
        assert!(!note.is_empty(), "{} has a sentence", departure.kind());
        assert!(
            note.starts_with(' '),
            "every note is a clause appended after a sentence that ends in a full stop, so it \
             opens with the space that separates them"
        );
        assert!(
            note.contains("Nothing was re-applied to any substrate"),
            "🔴 claim C4 held on 4/4 beds the audit drove and is kept on every arm: {}",
            departure.kind()
        );
        assert!(
            !seen.contains(&note),
            "🔴 two departures answering with the same paragraph is the fold this lane removed, \
             put back one level down: {}",
            departure.kind()
        );
        seen.push(note);
    }
    assert_eq!(seen.len(), 7);
    assert_eq!(
        gx_api::journal_note(None),
        "",
        "and an intact journal is silent"
    );
}

/// 🔴 §3-3. The two clauses are **chosen**, not concatenated.
#[test]
fn r32_the_rollback_clause_and_a_departure_are_never_both_asserted() {
    // The arm the audit measured: a departure and a roll-back at once. Before this lane the two
    // paragraphs were joined with `format!("{}{}", ..)`.
    let both = gx_api::journal_and_head_note(
        Some(JournalDeparture::ChainBroken),
        Some("this project holds 2 leaf/leaves and its head names 3"),
    );
    println!("R32_BOTH note={both:?}");
    assert!(
        both.contains("The journal is the file that moved"),
        "the departure is what an operator has to act on, so it is what is printed"
    );
    assert!(
        !both.contains("Comparing the two files will find nothing"),
        "🔴 `req/392` §3-3: this clause is true only when the two files agree, and it was printed \
         in the same `detail` string as a sentence saying one of them had been rewritten, on 4/4 \
         beds"
    );
    assert!(
        !both.contains("the journal and the ledger agree with each other here"),
        "same clause, the half the audit's probe matched on"
    );

    // The roll-back on its own is unchanged, byte for byte, because its condition is unchanged.
    let alone = gx_api::journal_and_head_note(None, Some("a reason"));
    assert_eq!(
        alone,
        gx_api::rolled_back_note(Some("a reason")),
        "🔴 `req/229` H-01's sentence is not touched by this lane"
    );
    assert!(
        alone.contains("Comparing the two files will find nothing"),
        "and it still says the thing that is true in its own condition"
    );

    // A departure on its own is the note and nothing else.
    assert_eq!(
        gx_api::journal_and_head_note(Some(JournalDeparture::Downgraded), None),
        gx_api::journal_note(Some(JournalDeparture::Downgraded))
    );
    // And a project with neither says nothing at all.
    assert_eq!(gx_api::journal_and_head_note(None, None), "");
}
