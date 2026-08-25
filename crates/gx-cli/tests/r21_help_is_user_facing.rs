// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R21 / `req/304` D1 (`req/306` §1 item 2)** — the first thing anyone sees.
//!
//! `req/304`'s dogfood walk typed `gx --help` before opening a single file, which is what a
//! first-time user does, and read this back:
//!
//! ```text
//! M6 hand 2 implements 44 §1.2's read side: receipt show/verify, log
//! proof/consistency/checkpoint, key gen/list, replay. The write side (submit,
//! plan, verify, commit, undo, cancel, escalation, policy, serve) is hands 3
//! onwards.
//! ```
//!
//! Its finding D1, severity **high**: "internal build-phase language in the literal help text of a
//! released binary … reads as unfinished/leaked internal notes, not a description of the tool". Its
//! third top-3 remedy: "It should say what the tool is and does, or nothing."
//!
//! Two of the sentence's claims were also **false of the shipped binary**, which is the part that
//! makes it worse than untidy: the write side is not "hands 3 onwards", it is implemented and
//! exercised in this very tree, and "M6 hand 2" is a phase that ended many milestones ago.
//!
//! # What this suite holds, and what it deliberately does not
//!
//! It holds the **banner** — `Cli`'s `long_about`, everything `clap` prints before `Usage:` — to
//! two properties: it carries no internal build-phase or specification vocabulary, and it names
//! the verbs a reader is looking for.
//!
//! 🔴 It does **not** hold the per-subcommand summaries in the `Commands:` block, and they are in
//! the same condition or worse (`demo`'s one-line summary is a `req/134` citation with a `sem:`
//! tag in it; `repair`'s names DR-43-8; `serve`'s quotes 44 §1.1). `req/306` §1 item 2 scopes this
//! lane to `main.rs`'s `long_about`, and widening it silently would be a lane deciding its own
//! scope. `req/307` §3 files the rest as a measured row with a count.
//!
//! 🔴 It also does not touch the **doc comments** that carry the same words. `consumers.rs`'s "M6
//! hand" notes are a historical record of which hand decided what, `req/306` §1 item 2 says so in
//! as many words, and the rule is the one that keeps test function names unchanged after a
//! finding: a record of what happened is not a user-facing string, and editing it would delete
//! provenance to tidy prose nobody reads at a terminal.

mod support;

use support::run;

/// The banner `clap` prints before `Usage:` — `Cli`'s `long_about`, and nothing else.
fn banner(stdout: &str) -> String {
    stdout
        .split("\nUsage:")
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 🔴 Vocabulary that belongs in `req/`, in a doc comment, or in a commit message — never in the
/// banner of a shipped binary.
///
/// Each of these was in the sentence `req/304` measured, or is the same class of thing one word
/// over. The list is a **predicate**, not a copy of the old string: a rewrite that swapped
/// "M6 hand 2" for "P5 lane 3" would satisfy a `!= old_text` assertion and change nothing for a
/// reader.
const INTERNAL_VOCABULARY: [&str; 10] = [
    "M6 hand", "M7 hand", "hand 2", "hands 3",
    "§",     // any specification section reference: 44 §1.2, 43 §3, 42 §3.12
    "44 ",   // the API specification, by number
    "43 ",   // the state-machine specification, by number
    "req/",  // this repository's requirement documents, which are not shipped
    "DR-",   // a design ruling
    "sem: ", // the semantic-anchor tags the doc comments carry
];

/// 🔴 **`req/304` D1** — the banner says what `gx` is and does, in words a newcomer has.
#[test]
fn the_help_banner_carries_no_internal_build_vocabulary() {
    let out = run(support::gx().arg("--help"));
    assert_eq!(
        out.code, 0,
        "`--help` is a normal termination: {}",
        out.stderr
    );
    let banner = banner(&out.stdout);
    println!("BANNER_BYTES={}\n---\n{banner}\n---", banner.len());

    assert!(
        !banner.is_empty(),
        "🔴 `req/304`'s remedy is \"say what the tool is and does, **or nothing**\", and this arm \
         reads the first branch: the banner exists and is about the product"
    );

    let found: Vec<&str> = INTERNAL_VOCABULARY
        .iter()
        .copied()
        .filter(|needle| banner.contains(needle))
        .collect();
    println!("INTERNAL_VOCABULARY_IN_BANNER={found:?}");
    assert!(
        found.is_empty(),
        "🔴 `req/304` D1 (severity high): {found:?} is internal build-phase or specification \
         vocabulary, and it is in the literal help text of a shipped binary — the first thing \
         every single person who runs `gx --help` reads, before they open any file. It also \
         disagrees in register with everything in `README.md` and `docs/`.\n\nBANNER:\n{banner}"
    );
}

/// 🔴 The other half — a banner that says nothing would pass the arm above.
///
/// `req/306` §1 item 2 asks for "a product sentence plus the main verbs", so both halves are
/// measured: the sentence has to name what gx *does to a change*, and the verb list has to be the
/// binary's own rather than a subset somebody remembered. The verbs are read from `clap`'s own
/// `Commands:` block, so a verb added tomorrow and left out of the banner is red here.
#[test]
fn the_help_banner_names_the_product_and_every_verb_the_binary_has() {
    let out = run(support::gx().arg("--help"));
    let banner = banner(&out.stdout);

    // What gx does, in the words `README.md` uses.
    for word in ["commit", "refuse", "receipt", "undo"] {
        assert!(
            banner.to_lowercase().contains(word),
            "the banner has to say what happens to a change; it does not say {word:?}:\n{banner}"
        );
    }

    // 🔴 Every verb `clap` lists, read off the binary rather than transcribed.
    let commands: Vec<String> = out
        .stdout
        .split("\nCommands:\n")
        .nth(1)
        .unwrap_or_default()
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .filter(|v| *v != "help")
        .map(str::to_string)
        .collect();
    assert!(
        commands.len() >= 15,
        "the verb list parsed short: {commands:?}"
    );
    let missing: Vec<&String> = commands.iter().filter(|v| !banner.contains(*v)).collect();
    println!("VERBS={} MISSING_FROM_BANNER={missing:?}", commands.len());
    assert!(
        missing.is_empty(),
        "🔴 `req/306` §1 item 2 asks the banner to enumerate the main verbs. {missing:?} are verbs \
         this binary has and the banner does not name, so a reader who read only the banner would \
         not know they exist:\n{banner}"
    );
}

/// 🔴 The record is not the banner — `consumers.rs`'s "M6 hand" notes stay exactly where they are.
///
/// `req/306` §1 item 2, verbatim: the doc comments naming "M6 hand" are a **historical record** and
/// are not to be touched, the same rule that keeps a test function's name unchanged after the
/// finding it was written for is repaired. This arm is the guard on the repair rather than on the
/// defect: a later lane doing a well-meaning global search-and-replace for the words `req/304`
/// complained about would delete provenance, and it will be red here when it does.
#[test]
fn the_historical_record_in_the_doc_comments_is_untouched() {
    let consumers =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/consumers.rs"))
            .expect("consumers.rs is readable");
    let hands = consumers.matches("M6 hand").count();
    println!("CONSUMERS_M6_HAND_MENTIONS={hands}");
    assert!(
        hands >= 1,
        "🔴 `req/306` §1 item 2: `consumers.rs`'s \"M6 hand\" comments are the record of which \
         hand settled the two accessor decisions M5 left open. A banner is a user-facing string \
         and a doc comment is provenance; R21 changes the first and may not touch the second"
    );
}
