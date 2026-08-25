// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R27 item 5 (`req/331` §0-5, from `req/329` L-02, `req/38` §233 ruling 5)** — the
//! remedy-parity gate's allowlist constrains **values**, not just names.
//!
//! # What broke
//!
//! `r26_refusal_remedy_parity.rs` closes *"a sentence could be invisible to the detector"* with two
//! directions. Direction 2 — every constant the detector cannot see is a declared non-sentence — is
//! the half that closes the condition, and it rests on a list of **five names**. A name binds
//! nothing about what the constant says, so the slot a wire key occupies is a slot a sentence can
//! sit in: rename nothing, change the value, and a refusal written with no remedy passes the
//! census by being on the list of things that are not sentences.
//!
//! `READ_ANSWERED_ABSENT` is already the shape of it. It is on the list as a token a transport
//! writes, and its value is a seventy-six character sentence in English. Nothing today is wrong
//! about that entry — it *is* a token — but the entry would be just as satisfied if the value
//! became a refusal, and the gate would not notice.
//!
//! # What this file requires
//!
//! The allowlist binds each name to the **exact value** the crate ships for it. A future constant
//! that takes an allowlisted name and says something else is not on the list, so it falls to
//! direction 2 and has to carry a remedy or be declared afresh — which is the decision a reviewer
//! gets to see, and the whole point of the list being written by hand.
//!
//! # What this file deliberately does **not** do
//!
//! `req/329` L-02's other half is that direction 1's family filter — `name.contains("REFUSAL")` —
//! selects six of the twenty-one prose constants in the directory. The arm the audit wrote for it
//! (`i_the_remedy_parity_gate_is_one_directory_wide`) can only be satisfied by **renaming** the
//! other fifteen, because it counts constant names in the source and no change to a gate moves it.
//! That rename would force an edit to `r22_refusal_constant_census.rs`, a frozen census that names
//! those constants in a table — editing one instrument so that a cosmetic change made for another
//! instrument passes. `req/332` reports that refusal and the classification of the fifteen instead.
//! What is closed here is the half that is a mechanism rather than a convention, and the coverage
//! claim direction 1 was being read as making is given its own arm below.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const REMEDY_MARKER: &str = "What to fix:";

fn record(line: &str) {
    println!("{line}");
    if let Ok(path) = std::env::var("R27_MEASUREMENTS") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn adapter_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gx-adapter-mcp")
        .join("src")
}

fn parity_gate_source() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("r26_refusal_remedy_parity.rs"),
    )
    .expect("the shipped parity gate is readable")
}

// ---------------------------------------------------------------------------
// 🔴 R29 / `req/361` L-02 — the const walk, with its two defects closed
// ---------------------------------------------------------------------------

/// 🔴 **R29 / `req/361` L-02** — whether this `pub const …` declares a **string constant**.
///
/// R22/R26/R27 all asked `rest.contains("&str")`, and the twenty-eighth audit found what that
/// misses: `&'static str`, a spelling this repository uses (`crates/gx-cli/src/keys.rs`'s
/// `pub const ALGORITHM: &str = gx_witness::keys::KEY_ALGORITHM;` is the same family), does not
/// contain the substring `&str`. A recon agent first reported the walk was dormant because the
/// scanned directory holds no array constants; that was wrong — `gx-adapter-mcp/src/adapter.rs`
/// declares `pub const ALL_FACTS: [&'static str; 4]` — and the real reason is this predicate.
/// A right conclusion for a wrong reason breaks on a different day, so the reason is fixed here.
///
/// The type is read between the `:` and the `=`, and a string constant is a **reference ending in
/// `str`**. `[&'static str; 4]` is deliberately still excluded: it is an array, these censuses map
/// one name to one sentence, and admitting it would change what they measure rather than fix it.
/// Measured on the tree that shipped this repair: 27 constants selected, exactly as before.
fn declares_a_str_constant(rest: &str) -> bool {
    if rest.trim_start().starts_with("fn ") {
        return false;
    }
    let Some((_, after_name)) = rest.split_once(':') else {
        return false;
    };
    let Some((ty, _)) = after_name.split_once('=') else {
        return false;
    };
    let ty = ty.trim();
    ty.starts_with('&') && ty.ends_with("str")
}

/// `true` when `text` holds an even number of unescaped `"` — i.e. we are not inside a literal.
///
/// The minimum parse that lets the terminator below be a `;` rather than the spelling `";`.
fn quotes_balanced(text: &str) -> bool {
    let mut open = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let _ = chars.next();
            }
            '"' => open = !open,
            _ => {}
        }
    }
    !open
}

/// Accumulate a `pub const` body from `at` and return it with the line it closed on.
///
/// 🔴 **R29 / `req/361` L-02** — R22/R26/R27 closed this walk on a line ending in the two
/// characters `";`, and `req/334` §5 had already filed the same walk in a frozen artifact. The
/// twenty-eighth audit found the live copies and measured the honest verdict: **today nothing is
/// swallowed** (27 selected, 27 reaching a terminator), and the walk is correct only because this
/// one directory happens to spell every constant the same way. The gate's own comment said so —
/// *"Walked rather than parsed: this crate writes them all one way"* — and declaring an assumption
/// is not enforcing it. Splicing in one ordinary alternative spelling made the walk run past the
/// end of the declaration and swallow everything after it, silently.
///
/// Two changes. The terminator is now **a statement's `;`** taken outside a string literal, which
/// closes `= OTHER::VALUE;` and `= concat!(…);` as well as `= "…";`. And a walk that reaches the
/// next item without ever closing **panics by name** instead of returning a truncated body:
/// swallowing is allowed to happen only where somebody has to read about it.
fn walk_const_body(lines: &[&str], at: usize, name: &str, file: &str) -> (String, usize) {
    let mut value = String::new();
    let mut cursor = at;
    while cursor < lines.len() {
        let line = lines[cursor];
        let trimmed = line.trim_start();
        if cursor > at && starts_a_new_item(trimmed) {
            break;
        }
        value.push_str(trimmed);
        if line.trim_end().ends_with(';') && quotes_balanced(&value) {
            return (value, cursor);
        }
        cursor += 1;
    }
    panic!(
        "🔴 **R29 / `req/361` L-02** — the declaration of `{name}` in {file} was walked to the end \
         of its item without ever reaching a terminator, so this census was about to count a \
         truncated body as the whole of it. That is the swallow `req/334` §5 and `req/361` L-02 \
         both filed, and it is no longer allowed to be silent: read the declaration and teach this \
         walk the shape it is written in."
    )
}

/// The starts this walk treats as "the declaration ended before its terminator".
fn starts_a_new_item(trimmed: &str) -> bool {
    trimmed.starts_with("pub const ")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("#[")
        || trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("fn ")
        || trimmed.starts_with("impl ")
        || trimmed.starts_with("pub struct ")
        || trimmed.starts_with("pub enum ")
}

/// Every `pub const NAME: &str = "…"` in the adapter, with its file and its whole declaration.
///
/// The `\` line continuations are undone first, exactly as the compiler does — see the shipped
/// gate's note, and `req/324` §9-1 for the audit that filed a defect against a copy of it written
/// with the two characters `\` and `n`.
fn public_string_constants() -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(adapter_src_dir())
        .expect("the adapter's src is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    for path in files {
        let raw = std::fs::read_to_string(&path).expect("readable");
        let source = raw
            .split("\\\n")
            .map(|part| part.trim_start_matches(' '))
            .collect::<Vec<_>>()
            .join("");
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let lines: Vec<&str> = source.lines().collect();
        let mut at = 0usize;
        while at < lines.len() {
            let trimmed = lines[at].trim_start();
            let Some(rest) = trimmed.strip_prefix("pub const ") else {
                at += 1;
                continue;
            };
            if !declares_a_str_constant(rest) {
                at += 1;
                continue;
            }
            let name = rest
                .split(':')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            let (value, cursor) = walk_const_body(&lines, at, &name, &file);
            out.insert(name, (file.clone(), value));
            at = cursor + 1;
        }
    }
    out
}

/// What a declaration says, as opposed to what it is called: everything after the first `=`.
fn payload(declaration: &str) -> String {
    declaration
        .split_once('=')
        .map_or_else(String::new, |(_, rest)| rest.trim().to_string())
}

/// 🔴 The six names, **each bound to the value this crate ships for it**.
///
/// This is the shape the shipped gate has to have. Kept here as well so the mutation below can be
/// fired at a table that is not the shipped one — an arm that could only read the real files could
/// say "the file as it stands is fine" and nothing about the file that does not stand yet, which is
/// the difference `r25_declaration_axes.rs` was built around.
const NOT_A_SENTENCE: [(&str, &str); 6] = [
    ("SERVER_METADATA_KEY", "\"$server\""),
    ("ON_READ_FAILURE_KEY", "\"$on_read_failure\""),
    ("CAS_READ_KEY", "\"$cas_read\""),
    // 🔴 **DR-46-28** — the fourth reserved slot (`req/459` ruling 1). A JSON key, never a
    // sentence: the sentence that names it is `from_json`'s refusal, and R17's remedy rule reaches
    // that one where it lives.
    ("DETERMINISM_BOUNDARY_KEY", "\"$determinism_boundary\""),
    ("SCHEME_SEPARATOR", "\"://\""),
    (
        "READ_ANSWERED_ABSENT",
        "\"[gx: the server answered, and its answer is that this locator holds nothing]\"",
    ),
];

/// Direction 2, as a pure function of a constant table: a constant the detector cannot see has to
/// be a declared non-sentence **and still be saying what it was declared to say**.
fn invisible_and_undeclared(all: &BTreeMap<String, (String, String)>) -> Vec<String> {
    all.iter()
        .filter(|(_, (_, v))| !v.contains(REMEDY_MARKER))
        .filter(|(name, (_, v))| {
            !NOT_A_SENTENCE
                .iter()
                .any(|(n, value)| n == name && payload(v) == format!("{value};"))
        })
        .map(|(n, (f, _))| format!("{n} ({f})"))
        .collect()
}

/// 🔴 **Bed control** — the scan sees this crate's constants and every allowlisted name is one of
/// them, so a zero below is not an empty scan.
#[test]
fn a_bed_control_the_scan_sees_the_constants_and_the_allowlisted_names() {
    let all = public_string_constants();
    let missing: Vec<&str> = NOT_A_SENTENCE
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !all.contains_key(*n))
        .collect();
    record(&format!(
        "R27_PARITY_SCAN constants={} allowlisted_missing={missing:?}",
        all.len()
    ));
    assert!(
        all.len() >= 20,
        "a scan that finds nothing satisfies every arm below: {}",
        all.len()
    );
    assert!(
        missing.is_empty(),
        "allowlisted names that do not exist: {missing:?}"
    );
}

/// 🔴 **`req/329` L-02** — every allowlisted name still says what the list says it says.
///
/// This is the arm that turns the list from a set of names into a set of facts. A name on it that
/// no longer exists was already caught by the shipped gate; a name that exists and now says
/// something else was not.
#[test]
fn b_every_allowlisted_name_carries_the_value_the_list_binds_it_to() {
    let all = public_string_constants();
    let drifted: Vec<String> = NOT_A_SENTENCE
        .iter()
        .filter_map(|(n, value)| {
            all.get(*n).and_then(|(f, v)| {
                (payload(v) != format!("{value};"))
                    .then(|| format!("{n} ({f}) says {} not {value};", payload(v)))
            })
        })
        .collect();
    record(&format!("R27_PARITY_DRIFT {drifted:?}"));
    assert!(
        drifted.is_empty(),
        "🔴 an allowlist entry whose value has moved is a slot that was declared not to be a \
         sentence and may now hold one: {drifted:?}"
    );
}

/// 🔴 **The mutation, fired** — a refusal that takes an allowlisted **name** does not pass.
///
/// `READ_ANSWERED_ABSENT` is the entry this is about: a seventy-six character English sentence
/// already sits in a slot the list calls a token. Here its value is replaced with a refusal that
/// carries no remedy, the name is left exactly as it was, and direction 2 has to catch it.
#[test]
fn c_a_sentence_wearing_an_allowlisted_name_is_caught() {
    let mut table = public_string_constants();
    let before = invisible_and_undeclared(&table);
    assert!(
        before.is_empty(),
        "the crate as it stands has to be clean, or this mutation proves nothing: {before:?}"
    );
    table.insert(
        "READ_ANSWERED_ABSENT".to_string(),
        (
            "transport.rs".to_string(),
            "pub const READ_ANSWERED_ABSENT: &str = \"this call was refused because the server \
             would not answer for the object, and gx will not guess on its behalf\";"
                .to_string(),
        ),
    );
    let after = invisible_and_undeclared(&table);
    record(&format!(
        "R27_PARITY_MUTATION before={} after={after:?}",
        before.len()
    ));
    assert!(
        after.iter().any(|c| c.contains("READ_ANSWERED_ABSENT")),
        "🔴 `req/329` L-02: a refusal with no remedy took an allowlisted name and the gate did not \
         see it. The list binds names, so the slot a wire key occupies is a slot a sentence can sit \
         in: {after:?}"
    );
}

/// 🔴 **The shipped gate has this shape**, not just this file.
///
/// An arm that held only its own copy of the predicate would leave the gate that runs on every
/// clone free to keep binding names — the same trap `r26_reach_census.rs` exists to avoid for the
/// other census.
#[test]
fn d_the_shipped_gate_binds_values_and_not_only_names() {
    let gate = parity_gate_source();
    let code: String = gate
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    record(&format!(
        "R27_PARITY_SHIPPED tuple_list={} binds_a_value={}",
        code.contains("NOT_A_SENTENCE: [(&str, &str); 6]"),
        code.contains("$on_read_failure")
    ));
    assert!(
        code.contains("NOT_A_SENTENCE: [(&str, &str); 6]"),
        "🔴 `req/329` L-02: the shipped allowlist is still a list of names. A name constrains \
         nothing about what the constant says."
    );
    for (_, value) in NOT_A_SENTENCE {
        let bare = value.trim_matches('"');
        assert!(
            code.contains(bare),
            "🔴 the shipped allowlist does not bind the value `{bare}`, so that entry is still a \
             name with an open slot behind it"
        );
    }
}

/// 🔴 **The coverage claim, given its own arm** (`req/329` L-02's other half, closed as a mechanism
/// rather than as a naming convention).
///
/// Direction 1 selects the family by `name.contains("REFUSAL")` and the audit measured that this is
/// six of twenty-one. What a reader takes from a *remedy-parity* gate is not that the six are held
/// but that **every refusal this crate ships is held**, and that claim is direction 1 plus direction
/// 2 together. Written out here as one predicate over the whole directory, it can neither be read
/// off a naming convention nor be satisfied by a list of names.
#[test]
fn e_every_prose_constant_either_carries_a_remedy_or_is_a_value_bound_non_sentence() {
    let all = public_string_constants();
    let uncovered = invisible_and_undeclared(&all);
    let prose = all.values().filter(|(_, v)| v.len() > 120).count();
    let named_family = all.keys().filter(|n| n.contains("REFUSAL")).count();
    record(&format!(
        "R27_PARITY_COVERAGE constants={} prose={prose} REFUSAL_named={named_family} uncovered={uncovered:?}",
        all.len()
    ));
    assert!(
        uncovered.is_empty(),
        "🔴 these constants are invisible to the completeness census and are not value-bound \
         declared non-sentences: {uncovered:?}"
    );
    assert!(
        named_family < prose,
        "this arm exists because the named family is a minority of the prose constants \
         ({named_family} of {prose}); if that stops being true the finding has changed shape and \
         this file should be re-derived rather than re-run"
    );
}

/// 🔴 **`req/331` §0-5's classification**, recorded rather than asserted.
///
/// The reqdef asks this lane to classify the fifteen prose constants that carry the remedy marker
/// without `REFUSAL` in the name. The classification is in `req/332`; this arm prints the list it
/// was made from, so the table in the report is checkable against a run rather than against my
/// reading.
#[test]
fn f_the_fifteen_are_listed_for_the_report() {
    let all = public_string_constants();
    let mut named: Vec<String> = Vec::new();
    let mut marker_unnamed: Vec<String> = Vec::new();
    for (name, (file, value)) in &all {
        if value.len() <= 120 {
            continue;
        }
        if name.contains("REFUSAL") {
            named.push(format!("{file}::{name}"));
        } else if value.contains(REMEDY_MARKER) {
            marker_unnamed.push(format!("{file}::{name}"));
        }
    }
    named.sort();
    marker_unnamed.sort();
    record(&format!("R27_PARITY_FAMILY_NAMED {named:?}"));
    record(&format!(
        "R27_PARITY_FAMILY_MARKER_ONLY count={} {marker_unnamed:?}",
        marker_unnamed.len()
    ));
    assert!(
        !marker_unnamed.is_empty(),
        "the report's table is built from this list, so an empty one means the scan moved"
    );
}
