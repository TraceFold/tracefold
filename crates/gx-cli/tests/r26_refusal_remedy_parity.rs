// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/324` L-02 (`req/38` §231 ruling 3)** — the convention the census stands on, promoted
//! to a gate.
//!
//! # What the twenty-fifth audit actually found
//!
//! `r22_refusal_constant_census.rs` guards completeness by scanning the adapter for
//! `pub const … : &str` **whose value contains `What to fix:`**. `req/38` §230 ruling 2 (d) then
//! admitted a fourth family on the grounds that *"this number moves with it"*.
//!
//! The audit measured the shipped scan and found **no defect**: every refusal sentence this build
//! ships carries the remedy marker, so the number does move. What is left is a **condition**: a
//! second sentence written without the marker would be invisible to the detector, and the census
//! would stay green at the same number while the crate grew a sentence nobody declared. The
//! audit's own first instrument fell into that hole in reverse (its §9-1: a line-continuation bug
//! made two constants invisible and it nearly filed them as a defect).
//!
//! # What this file freezes
//!
//! The convention itself, mechanically, in two directions:
//!
//! 1. every constant whose **name** puts it in the refusal family carries the remedy marker; and
//! 2. every constant the detector **cannot** see is on a declared list of things that are not
//!    sentences — keys, separators and the absence token. A new sentence written without a remedy
//!    is then red here rather than silent there.
//!
//! It is the cheap half of `req/38` §231 ruling 3 and it is worth exactly what it costs: the gate
//! is the reason the number the census reports keeps meaning what it says.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The crate's own marker for a remedy clause (R17's wording rule).
const REMEDY_MARKER: &str = "What to fix:";

/// 🔴 Constants that are **not** sentences: a wire key, a separator, and the token a transport
/// writes to say that a read answered with absence.
///
/// Declared by name rather than inferred, because "is this a sentence" has no mechanical form that
/// does not also let a sentence through. Adding a name here is a decision a reviewer sees.
/// 🔴 **Bound to values by `req/329` L-02 (`req/38` §233 ruling 5)** — a name constrains nothing
/// about what the constant says.
///
/// The list used to hold five names. A name is a slot, and the slot a wire key occupies is a slot a
/// sentence can sit in: rename nothing, change the value, and a refusal written with no remedy
/// passes the completeness census by being on the list of things that are not sentences.
/// `READ_ANSWERED_ABSENT` is already the shape of it — a seventy-six character English sentence in
/// a slot this list calls a token. Nothing is wrong with that entry today; the point is that the
/// entry would be just as satisfied if it stopped being true.
///
/// Each entry now binds the **exact value** this crate ships. A future constant that takes one of
/// these names and says something else is not on the list, falls to direction 2, and has to carry a
/// remedy or be declared afresh — which is the decision a reviewer gets to see.
const NOT_A_SENTENCE: [(&str, &str); 6] = [
    ("SERVER_METADATA_KEY", "\"$server\""),
    ("ON_READ_FAILURE_KEY", "\"$on_read_failure\""),
    ("CAS_READ_KEY", "\"$cas_read\""),
    // 🔴 **DR-46-28** — the fourth reserved slot, and a key for the three others' reason: it is
    // the JSON name a deployment writes, never a sentence anyone is shown. The refusal that names
    // it *is* a sentence and lives in `from_json`, where R17's remedy rule applies to it.
    ("DETERMINISM_BOUNDARY_KEY", "\"$determinism_boundary\""),
    ("SCHEME_SEPARATOR", "\"://\""),
    (
        "READ_ANSWERED_ABSENT",
        "\"[gx: the server answered, and its answer is that this locator holds nothing]\"",
    ),
];

/// What a declaration says, as opposed to what it is called: everything after the first `=`.
fn payload(declaration: &str) -> String {
    declaration
        .split_once('=')
        .map_or_else(String::new, |(_, rest)| rest.trim().to_string())
}

/// Whether `name` is on the allowlist **and still saying what the allowlist binds it to**.
fn is_a_declared_non_sentence(name: &str, declaration: &str) -> bool {
    NOT_A_SENTENCE
        .iter()
        .any(|(n, value)| *n == name && payload(declaration) == format!("{value};"))
}

fn adapter_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gx-adapter-mcp")
        .join("src")
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

/// Every `pub const NAME: &str = "…"` in the adapter, with its value and file.
///
/// 🔴 The `\` line continuations are undone first, exactly as the compiler does — it drops the
/// newline and the indentation after it. Without this a sentence whose break falls inside the
/// remedy clause is invisible to the marker, which is a scan that passes by not seeing the thing
/// it is looking for. `r22_refusal_constant_census.rs` does the same undoing for the same reason,
/// and `req/324` §9-1 is the audit filing a defect against this file because its own copy of the
/// undoing was written with the two characters `\` and `n` instead.
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

/// 🔴 The instrument's own guard. A scan that found nothing would satisfy every arm below.
#[test]
fn the_scan_sees_the_constants_this_crate_ships() {
    let all = public_string_constants();
    let with_marker = all
        .values()
        .filter(|(_, v)| v.contains(REMEDY_MARKER))
        .count();
    println!(
        "R26_REMEDY total={} with_marker={with_marker} without={:?}",
        all.len(),
        all.iter()
            .filter(|(_, (_, v))| !v.contains(REMEDY_MARKER))
            .map(|(n, (f, _))| format!("{n} ({f})"))
            .collect::<Vec<_>>()
    );
    assert!(
        all.len() >= 20 && with_marker >= 15,
        "🔴 the scan found {} constants and {with_marker} carrying a remedy. The build this arm was \
         written against has twenty-five and twenty. A scan that finds nothing passes by not \
         seeing the thing it is looking for -- which is what `req/324` §9-1 was.",
        all.len()
    );
}

/// 🔴 **`req/324` L-02, direction 1** — the refusal family carries its remedy, by name.
#[test]
fn every_constant_in_the_refusal_family_carries_a_remedy() {
    let all = public_string_constants();
    let family: Vec<(&String, &(String, String))> = all
        .iter()
        .filter(|(name, _)| name.contains("REFUSAL"))
        .collect();
    let missing: Vec<String> = family
        .iter()
        .filter(|(_, (_, v))| !v.contains(REMEDY_MARKER))
        .map(|(n, (f, _))| format!("{n} ({f})"))
        .collect();
    println!("R26_REMEDY family={} missing={missing:?}", family.len());
    assert!(
        family.len() >= 5,
        "🔴 the family this arm freezes has to be found: {}",
        family.len()
    );
    assert!(
        missing.is_empty(),
        "🔴 `req/38` §231 ruling 3: the completeness census counts the constants that carry \
         `What to fix:`, so a refusal written without one is invisible to it. R17's wording rule is \
         the convention that number stands on; here it is the gate. {missing:?}"
    );
}

/// 🔴 **`req/324` L-02, direction 2** — nothing is invisible to the detector by accident.
///
/// This is the half that closes the condition rather than restating it. The detector sees a
/// constant when it carries the remedy marker; every constant it does **not** see has to be a
/// declared non-sentence, so a second `PreVerdict` sentence written without a remedy cannot pass
/// the census by being unseen.
#[test]
fn every_constant_the_detector_cannot_see_is_a_declared_non_sentence() {
    let all = public_string_constants();
    let undeclared: Vec<String> = all
        .iter()
        .filter(|(_, (_, v))| !v.contains(REMEDY_MARKER))
        .filter(|(name, (_, v))| !is_a_declared_non_sentence(name, v))
        .map(|(n, (f, _))| format!("{n} ({f})"))
        .collect();
    println!("R26_REMEDY_BLIND undeclared={undeclared:?}");
    assert!(
        undeclared.is_empty(),
        "🔴 `req/324` L-02 (`req/38` §231 ruling 3): {} constant(s) are invisible to \
         `r22_refusal_constant_census.rs`'s completeness arm and are not on this file's list of \
         things that are not sentences. Either the constant is a refusal -- give it the remedy \
         clause R17's rule asks for -- or it is a key, and naming it in `NOT_A_SENTENCE` is the \
         decision a reviewer gets to see: {undeclared:?}",
        undeclared.len()
    );
}

/// 🔴 And the list itself does not rot: a name on it that no longer exists is removed, so the
/// allowlist cannot grow into a place to hide a sentence.
#[test]
fn the_declared_non_sentences_all_still_exist() {
    let all = public_string_constants();
    let gone: Vec<&str> = NOT_A_SENTENCE
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !all.contains_key(*name))
        .collect();
    // 🔴 **`req/329` L-02** — and the entries that still exist say what they were declared to say.
    // A name that has gone was already a hole; a name that has stayed and changed its value is the
    // same hole with the door closed behind it.
    let drifted: Vec<String> = NOT_A_SENTENCE
        .iter()
        .filter_map(|(name, value)| {
            all.get(*name).and_then(|(file, declaration)| {
                (payload(declaration) != format!("{value};"))
                    .then(|| format!("{name} ({file}) now says {}", payload(declaration)))
            })
        })
        .collect();
    println!("R26_REMEDY_ALLOWLIST gone={gone:?} drifted={drifted:?}");
    assert!(
        gone.is_empty(),
        "🔴 a name on the not-a-sentence list that this crate no longer ships is a hole waiting \
         for a future constant with the same name: {gone:?}"
    );
    assert!(
        drifted.is_empty(),
        "🔴 `req/329` L-02: an allowlist entry whose value has moved was declared not to be a \
         sentence and may now hold one: {drifted:?}"
    );
}

// ---------------------------------------------------------------------------
// 🔴 R29 / `req/361` L-02 — the walk's refusal, driven rather than read
// ---------------------------------------------------------------------------

/// 🔴 **R29 / `req/361` L-02** — a constant this walk cannot close is a **panic**, not a truncation.
///
/// # Why the behavioural arm is here and not with the other R29 gates
///
/// Because this file owns the walk. `crates/gx-cli/tests/r29_instrument_repairs.rs` holds the
/// source-level half — that all three live gates go through the repaired walk and that the refusal
/// is reachable from the "we reached the next item" road — and a source scan can only say the
/// `panic!` is *present*. Whether it *fires* is a question you have to ask by running it, and the
/// only place the function exists to be run is the file that declares it.
///
/// # What the twenty-eighth audit measured, and what this reverses
///
/// ```text
/// A28_TERMINATOR files=12 selected=27 unterminated_today=[]
/// A28_TERMINATOR spliced_selected=1 spliced_unterminated=["A28_SPLICED"]
/// ```
///
/// The first line is the honest half: **nothing in this repository is swallowed today**, and the
/// walk's verdicts have all been correct. The second is the defect: splice in one ordinary
/// alternative spelling — `pub const NAME: &str = other_module::VALUE;`, a form this repository
/// already uses in `crates/gx-cli/src/keys.rs` — and the old walk ran past the end of the
/// declaration and kept accumulating, in silence, returning a body that was not the constant's.
///
/// So there are two things to hold, and folding them into one arm would let either pass for the
/// other's reason: the alternative spelling now **closes** (it ends in `;` outside a literal, which
/// is what the terminator became), and a body that genuinely never closes now **refuses** instead
/// of handing back whatever it had collected.
#[test]
fn r29_a_constant_the_walk_cannot_close_is_refused_rather_than_truncated() {
    // (a) The spelling that used to run away now closes on its own line.
    let spliced = [
        "pub const A28_SPLICED: &str = some_other_module::VALUE;",
        "pub const AFTER_IT: &str = \"this must not be swallowed into the one above\";",
    ];
    assert!(
        declares_a_str_constant(
            spliced[0]
                .trim_start()
                .strip_prefix("pub const ")
                .expect("the fixture is a `pub const`")
        ),
        "🔴 the repaired selector no longer sees the very spelling `req/361` L-02 is about"
    );
    let (value, cursor) = walk_const_body(&spliced, 0, "A28_SPLICED", "spliced.rs");
    println!("R29_SPLICED_CLOSES value={value:?} cursor={cursor}");
    assert_eq!(
        cursor, 0,
        "🔴 the declaration closes on its own line, so the walk must stop there. A cursor past it \
         is the swallow itself: {value:?}"
    );
    assert!(
        !value.contains("must not be swallowed"),
        "🔴 `req/361` L-02, exactly: the walk ran past the end of one declaration and took the \
         next one with it: {value:?}"
    );

    // (b) A body that never closes is refused by name rather than returned truncated.
    let never_closes = [
        "pub const UNCLOSED: &str = \"opened and never finished",
        "/// the next item begins here, and the constant above never terminated",
        "pub const NEXT: &str = \"unrelated\";",
    ];
    let refused =
        std::panic::catch_unwind(|| walk_const_body(&never_closes, 0, "UNCLOSED", "spliced.rs"));
    println!("R29_SPLICED_REFUSED is_err={}", refused.is_err());
    assert!(
        refused.is_err(),
        "🔴 a constant whose body never reaches a terminator was walked to the next item and the \
         accumulated text returned as if it were the whole declaration. That is the silent \
         swallow `req/324` §9-1, `req/332` §5 and `req/361` L-02 each filed; it has to be loud"
    );
}
