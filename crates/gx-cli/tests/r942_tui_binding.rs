// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `req/942` g12c, g23 and P10 — the three probes of the terminal face that need something
//! **outside** the face, held here after #188/#189 rather than moved with the other fifty.
//!
//! # The rule that decided the split, stated once
//!
//! A probe belongs to `gx-tui` when it can be answered by the face alone. These three cannot:
//!
//! * **g12c / g23** read `gx tui --help` — text that lives in `crates/gx-cli/src/main.rs`, written
//!   by *this* crate — and hold it against the face's declarations. Keeping them in `gx-tui` would
//!   have made that package's suite read `../crates/gx-cli/src/main.rs`: a crate whose tests fail
//!   when its consumer drifts, and which cannot be tested in a tree that does not carry that
//!   consumer.
//! * **P10** is a *differential* between the face's own `rfc3339` and `gx_api::rfc3339::of`, so it
//!   needs both implementations in one binary. 🔴 Admitting `gx-api` and `gx-core` as
//!   `[dev-dependencies]` of `gx-tui` would have put the engine's crates back into that package's
//!   graph the day the extraction took them out — and because dev edges do not ship,
//!   `cargo tree -e normal` would have gone on saying the membrane was intact. That is precisely
//!   the shape of the crack the audit found the first time (`req/38` SS965 row (a)): one date
//!   formatter, imported where the obvious measurement does not look.
//!
//! Both crates are already dependencies here, so nothing new is admitted anywhere by this file.
//!
//! 🔴 **The arithmetic, stated rather than left to be reconstructed.** `req/942` had 53 probes in
//! one file. After the move: **51** in `tui/tests/r942_tui.rs` and **3** here — 54, because P10
//! was split in two. Its differential half is below; its *shape* half (is this face's own date
//! RFC 3339 to the nanosecond?) needs only the face and stayed there as `p10b`. No probe was
//! dropped, and the one that was added is a claim the old P10 already made in its last two lines.

#![cfg(feature = "tui")]

use std::path::Path;

use gx_tui::tui::acts;
use gx_tui::tui::live;
use gx_tui::tui::wire::{self, Nothing};

/// The help text, as one line.
///
/// 🔴 A phrase that has to appear can be word-wrapped across two source lines by the `\` string
/// continuations `main.rs` uses, and a search over the raw text would then report it missing — the
/// gate would be measuring the shape of the source rather than the presence of the sentence.
/// Identical to `tui/tests/r942_tui.rs`'s helper of the same name and deliberately not shared: a
/// four-line normaliser is not worth a dependency between the two suites, and the day one of them
/// wants a different normalisation is the day sharing it would have been the defect.
fn flat(text: &str) -> String {
    text.replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn help_source() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
        .expect("main.rs is readable")
}

/// 🔴 The failure mode this repository is named after: a document that describes a program that
/// does not exist. `gx tui --help` lists the keys, so the help and the declaration are required to
/// say the same thing, measured over the source rather than trusted.
#[test]
fn g12c_the_help_text_names_every_declared_act() {
    let flattened = flat(&help_source());
    let mut missing: Vec<String> = Vec::new();
    for act in acts::ACTS {
        let line = format!("{} {}", act.keys()[0], act.intent());
        if !flattened.contains(&line) {
            missing.push(line);
        }
    }
    println!("G12C_CHECKED={}", acts::ACTS.len());
    assert!(
        missing.is_empty(),
        "🔴 g12c: the help text does not spell {missing:?}. A key a person is not told about is a \
         capability that does not exist for them"
    );
}

/// 🔴 **The same question asked of the legend rather than of the keys**, and it is the failure this
/// face's own documentation names out loud: *"the subscription line would go on drawing a mark the
/// legend no longer explains"*.
///
/// It had already happened twice, and both were found by reading `gx tui --help` beside the
/// vocabulary rather than by any gate:
///
/// * `req/38` SS974's queue row Q4 added a seventh word (`''`) and the legend went on saying **six**
///   and listing six. A reader met a mark on the screen that the program's own help did not explain.
/// * The subscription (`5ab52de8`) added `<<`, a mark that is deliberately none of the seven, and
///   the help did not mention the connection at all.
///
/// So the check is over the declarations rather than over a hand-written list: every mark this face
/// can draw has a line in the legend. A legend that is complete because somebody remembered is a
/// legend that is complete until the next edit.
#[test]
fn g23_the_help_text_explains_every_mark_this_face_can_draw() {
    let flattened = flat(&help_source());
    // A legend entry in that string literal begins `\x20 <mark> `; matching the entry rather than
    // the mark is what keeps `0` from being satisfied by the digit in an unrelated sentence.
    let entry = |mark: &str| format!("\\x20 {mark} ");
    let mut missing: Vec<&str> = Vec::new();
    for mark in Nothing::ALL.into_iter().map(Nothing::mark) {
        if !flattened.contains(&entry(mark)) {
            missing.push(mark);
        }
    }
    if !flattened.contains(&entry(live::OPEN_MARK)) {
        missing.push(live::OPEN_MARK);
    }
    println!(
        "G23_CHECKED={} MARKS={:?}",
        Nothing::ALL.len() + 1,
        Nothing::ALL
            .into_iter()
            .map(Nothing::mark)
            .collect::<Vec<_>>()
    );
    assert!(
        missing.is_empty(),
        "🔴 g23: the help text has no legend line for {missing:?}. A mark on the screen that the \
         help does not explain is a symbol the reader has to guess, and guessing is the thing this \
         vocabulary exists to remove"
    );
    // The negative control: an entry for a mark this face does not have is not found, so the
    // matcher is discriminating rather than matching anything shaped like a legend line.
    assert!(
        !flattened.contains(&entry("=?")),
        "🔴 g23: the matcher finds a legend line for a mark that does not exist"
    );
    // And the count in the sentence above the list is the count of the vocabulary, because a
    // legend that says `six` over seven lines has taught the reader there are six.
    assert!(
        flattened.contains("seven kinds are told apart"),
        "🔴 g23: the legend's own sentence does not name {} kinds",
        Nothing::ALL.len()
    );
}

/// 🔴 **The cost of closing the crack, measured rather than promised.** `req/38` SS965 row (a)
/// buys the membrane with a second implementation of one date format, and two implementations of a
/// format drift. So the two are run over the same instants and required to agree — the epoch, an
/// instant before it, both sides of a leap day, a leap second's neighbourhood, and the ends of the
/// range an `i64` nanosecond clock can carry.
///
/// Moved here verbatim by #188/#189, for the reason in this file's header: the comparison needs
/// both crates, and the face may not have them.
#[test]
fn p10_the_faces_own_rfc3339_agrees_with_the_api_crates() {
    let instants: [i64; 12] = [
        0,
        1,
        -1,
        -86_400_000_000_000,
        1_756_543_200_123_456_789,
        951_782_400_000_000_000, // 2000-02-29, the leap day a wrong rule loses
        4_107_542_400_000_000_000, // 2100-03-01, on the far side of a century that is not a leap year
        1_234_567_890_000_000_000,
        -2_208_988_800_000_000_000,
        i64::MAX,
        i64::MIN + 1,
        i64::MIN,
    ];
    let mut disagreements: Vec<(i64, String, String)> = Vec::new();
    for nanos in instants {
        let mine = wire::rfc3339(nanos);
        let theirs = gx_api::rfc3339::of(gx_core::Timestamp(nanos));
        println!("P10 {nanos} -> {mine}");
        if mine != theirs {
            disagreements.push((nanos, mine, theirs));
        }
    }
    assert!(
        disagreements.is_empty(),
        "🔴 P10: this face's date and the API crate's date are the same fact spelled twice, and \
         they disagree: {disagreements:?}"
    );
}
