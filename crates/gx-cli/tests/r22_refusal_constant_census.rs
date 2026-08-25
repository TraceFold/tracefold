// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/303` L-02** (`req/309` §1 item 5) — the parity gate holds **where every refusal
//! sentence lives**, not the shape of its name.
//!
//! # What the twenty-first adversarial audit measured
//!
//! `r20_refusal_vocabulary_is_whole.rs` scans `gx-adapter-mcp/src/invert.rs` for lines beginning
//! `pub const …_REFUSAL` and holds that set equal to `gx-cli`'s `DECLARATION_REFUSALS`. Its third
//! arm — *no other module of the adapter declares a declaration refusal* — closes the road of moving
//! a constant somewhere the first scan cannot see it.
//!
//! In the same lane, R20 added two refusal sentences and put them in `catalogue.rs` under the names
//! `RESTORE_TOOL_UNNAMED` and `TEMPLATE_NAMES_NO_PRIOR`. Neither ends in `_REFUSAL`, so the scan did
//! not see them and the third arm passed **vacuously**. DR-46-16 then added three more of the same
//! family (`CAS_READ_TOOL_UNNAMED`, `CAS_READ_PATTERN_EMPTY`, `CAS_READ_FACE_IS_AN_EFFECT`). The
//! audit could produce no misclassification from this — 5/5 of the wired constants classified
//! correctly on the real road — so the finding is **L**: it is chance holding this, not the gate.
//!
//! # What this file holds instead
//!
//! A **census**: every `pub const` in `gx-adapter-mcp/src/**` whose value is a refusal sentence, by
//! the crate's own marker for one (`What to fix:` — the wording rule R17 established and every
//! sentence in this family follows). Each is required to be in a declared table saying which family
//! it belongs to and which file it lives in, and each family carries its own obligation:
//!
//! * **runtime** — reaches an agent through `gx wrap`'s classifier. Must live in `invert.rs` and be
//!   wired into `DECLARATION_REFUSALS`, which is what R20's gate already says.
//! * **parse-time** — cannot reach the classifier because `Catalogue::from_json` turns it into a
//!   start-up error. Must live in `catalogue.rs` and must **not** be in the classifier's list,
//!   because a sentence there would be one the classifier can never see.
//!
//! A constant added to either file, or to a third one, is red here until someone says which it is.
//! That is the difference between holding a name shape and holding an inventory.
//!
//! Read rather than linked, for `r20_refusal_vocabulary_is_whole.rs`'s reason (its
//! `wired_refusal_names` doc): naming a symbol this lane created would turn the red into a compiler
//! message.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The marker every refusal sentence in this crate carries: the remedy clause R17 made the rule.
const REMEDY_MARKER: &str = "What to fix:";

/// Which road a refusal sentence travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    /// Reaches an agent as a `tools/call` answer, through `gx-cli`'s `declaration_refusal`.
    Runtime,
    /// Turns into a start-up error inside `Catalogue::from_json`; no agent ever sees it.
    ParseTime,
    /// 🔴 **`req/312` M-01 (R23)** — reaches an agent, and is **not** about a declaration.
    ///
    /// The third road, added because R23 needed a sentence the two above cannot hold honestly.
    /// `OBSERVATION_NOT_ANSWERED` is produced by `apply.rs` **after** an admitted call has been
    /// sent: the catalogue may be perfect and the effect is not in doubt. Wiring it into
    /// `DECLARATION_REFUSALS` would make `gx wrap` answer the agent
    /// `gx/verdict: "refused-before-verdict"` about a call that reached the server — the same class
    /// of false record `req/291` M-03 opened this census to stop, pointing the other way.
    ///
    /// So the obligation is the **opposite** of `Runtime`'s and it is written down rather than
    /// inherited: a row here must live in `apply.rs` and must not be in the classifier's list, and
    /// both halves are arms below. A future sentence wanting this family argues the same two
    /// things.
    PostApply,

    /// 🔴 **`req/320` M-01 (R25)** — the fourth road: reaches an agent, is not about a declaration,
    /// and is written **before any call is sent**.
    ///
    /// `READ_ANSWERED_THIS_LOCATOR_IS_ABSENT` is produced by `adapter.rs` inside `snapshot` and
    /// `precondition`, which run before a plan exists. Wiring it into `DECLARATION_REFUSALS` would
    /// tell an agent `gx/verdict: "refused-before-verdict"` about a file that is not at fault —
    /// `req/291` M-03's defect with the subject changed — and the honest classification is the one
    /// `gx wrap` already reaches on this road: *no verdict was reached, so this is not a refusal —
    /// it is a change gx could not describe*.
    ///
    /// The obligations are `PostApply`'s two, one file over: a row here must live in `adapter.rs`
    /// and must not be in the classifier's list, and both halves are arms below.
    PreVerdict,
}

/// 🔴 The inventory. One row per refusal sentence this crate ships, its file and its family.
///
/// This is the thing `req/303` L-02 says the gate did not have: R20's scan knew about a name shape
/// in one file, and there were five sentences of the same kind sitting in another. Adding a
/// sentence means adding a row, and adding a row means answering "which road does it travel", which
/// is the question that decides whether it must be wired into the classifier.
const CENSUS: &[(&str, &str, Family)] = &[
    ("READ_FAILURE_REFUSAL", "invert.rs", Family::Runtime),
    ("DECLARATION_UNSOUND_REFUSAL", "invert.rs", Family::Runtime),
    (
        "DECLARATION_WITHOUT_A_READ_FACE_REFUSAL",
        "invert.rs",
        Family::Runtime,
    ),
    ("OBJECT_IDENTITY_REFUSAL", "invert.rs", Family::Runtime),
    ("OBJECT_UNNAMED_REFUSAL", "invert.rs", Family::Runtime),
    (
        "IDENTITY_IGNORES_THE_ANSWER_REFUSAL",
        "invert.rs",
        Family::Runtime,
    ),
    // 🔴 **DR-46-21** (`req/38` §218 ruling 2, §417/§421) — the content-addressed half's digest
    // re-verification refusal. Runtime like its escrow twin `OBJECT_IDENTITY_REFUSAL`: it reaches
    // an agent through `gx wrap`'s classifier as a refusal before a verdict (the read answered
    // about a different object), so it lives in `invert.rs` and is wired into `DECLARATION_REFUSALS`.
    ("CAS_OBJECT_DIGEST_REFUSAL", "invert.rs", Family::Runtime),
    ("RESTORE_TOOL_UNNAMED", "catalogue.rs", Family::ParseTime),
    ("TEMPLATE_NAMES_NO_PRIOR", "catalogue.rs", Family::ParseTime),
    ("TOOL_NAME_IS_DECOMPOSED", "catalogue.rs", Family::ParseTime),
    ("CAS_READ_TOOL_UNNAMED", "catalogue.rs", Family::ParseTime),
    ("CAS_READ_PATTERN_EMPTY", "catalogue.rs", Family::ParseTime),
    (
        "CAS_READ_FACE_IS_AN_EFFECT",
        "catalogue.rs",
        Family::ParseTime,
    ),
    // 🔴 **R23** — the four this lane adds. Three join the parse-time family above; the fourth is
    // the first `PostApply` sentence this crate has ever shipped.
    (
        "CAS_READ_FACE_IS_AN_INVERSE",
        "catalogue.rs",
        Family::ParseTime,
    ),
    (
        "CAS_READ_PREFIX_IS_DECOMPOSED",
        "catalogue.rs",
        Family::ParseTime,
    ),
    (
        "CAS_READ_PREFIXES_COLLIDE",
        "catalogue.rs",
        Family::ParseTime,
    ),
    ("OBSERVATION_NOT_ANSWERED", "apply.rs", Family::PostApply),
    // 🔴 **R24** — the two this lane adds, both parse-time: a tool name with an edge a reader
    // cannot see (`req/316` L-02) and a declaration written as a JSON array (`req/316` L-01).
    (
        "TOOL_NAME_CARRIES_EDGE_WHITESPACE",
        "catalogue.rs",
        Family::ParseTime,
    ),
    (
        "DECLARATION_IS_POSITIONAL",
        "catalogue.rs",
        Family::ParseTime,
    ),
    // 🔴 **R25** — the two this lane adds. `req/320` M-04's is the fifth position a declaration
    // spells a name in, and it is a parse error like its four siblings. `req/320` M-01's is the
    // first sentence of the fourth road (see [`Family::PreVerdict`]).
    (
        "CAS_READ_PREFIX_CARRIES_EDGE_WHITESPACE",
        "catalogue.rs",
        Family::ParseTime,
    ),
    (
        "READ_ANSWERED_THIS_LOCATOR_IS_ABSENT",
        "adapter.rs",
        Family::PreVerdict,
    ),
    // 🔴 **R26 (`req/324` M-04, `req/38` §231 ruling 2)** — the two this lane adds, both parse-time
    // and both the *unnamed* fault in a slot nobody had swept: the `$cas_read` **prefix** and the
    // `restores` **key**. R25 widened the unnamed gates to the invisible-scalar predicate and
    // reached the three slots that are values; these are the two that are keys.
    (
        "CAS_READ_PREFIX_NAMES_NOTHING",
        "catalogue.rs",
        Family::ParseTime,
    ),
    ("EFFECT_TOOL_UNNAMED", "catalogue.rs", Family::ParseTime),
];

fn adapter_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-adapter-mcp/src")
}

fn cli_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
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

/// Every `pub const <NAME>: &str = …` in the adapter's source whose **value** carries the remedy
/// marker, as name → file.
///
/// The value rather than the name is the whole repair: R20's two constants are refusal sentences by
/// every property except their spelling.
fn refusal_sentences() -> BTreeMap<String, String> {
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
        // 🔴 Undo the `\` line continuations first, exactly as the compiler does — it drops the
        // newline and the indentation after it. Without this a sentence whose break falls inside
        // the remedy clause (`What to \` / `fix: …`) is invisible to the marker, which is a scan
        // that passes by not seeing the thing it is looking for. `r20_template_prior_soundness.rs`
        // does the same undoing for the same reason.
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
        // `pub const NAME: &str = "…"`, whose literal runs on with `\` continuations until a line
        // ends in `";`. Walked rather than parsed: this crate writes them all one way.
        let lines: Vec<&str> = source.lines().collect();
        let mut at = 0usize;
        while at < lines.len() {
            let trimmed = lines[at].trim_start();
            let Some(rest) = trimmed.strip_prefix("pub const ") else {
                at += 1;
                continue;
            };
            let name = rest
                .split(':')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !declares_a_str_constant(rest) {
                at += 1;
                continue;
            }
            let (value, cursor) = walk_const_body(&lines, at, &name, &file);
            if value.contains(REMEDY_MARKER) {
                out.insert(name, file.clone());
            }
            at = cursor + 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The census
// ---------------------------------------------------------------------------

/// 🔴 The instrument's own guard, first: a scan that found nothing would satisfy every arm below.
#[test]
fn the_scan_of_the_adapter_found_the_sentences_it_ships() {
    let found = refusal_sentences();
    println!("R22_L02_SCAN n={} {:?}", found.len(), found);
    assert!(
        found.len() >= CENSUS.len(),
        "🔴 the scan found {} refusal sentences and this repository declares {}. The scan has \
         drifted — most likely the crate stopped writing them as `pub const NAME: &str` with a \
         `{REMEDY_MARKER}` clause — and it would now pass by finding nothing. Rewrite it with \
         whatever rewrote them: {found:?}",
        found.len(),
        CENSUS.len()
    );
}

/// 🔴 `req/303` L-02: **every** refusal sentence in the crate is in the inventory, wherever it
/// lives and whatever it is called. This is the arm the old gate could not have: it does not look
/// for a name shape in one file.
#[test]
fn every_refusal_sentence_in_the_adapter_is_declared_here() {
    let found = refusal_sentences();
    let mut unaccounted: Vec<String> = Vec::new();
    for (name, file) in &found {
        match CENSUS.iter().find(|(n, _, _)| n == name) {
            None => unaccounted.push(format!("{file}: {name} (no row)")),
            Some((_, declared_file, _)) if declared_file != file => {
                unaccounted.push(format!(
                    "{name}: declared in {declared_file}, found in {file}"
                ));
            }
            Some(_) => {}
        }
    }
    println!("R22_L02_UNACCOUNTED {unaccounted:?}");
    assert!(
        unaccounted.is_empty(),
        "🔴 `req/303` L-02: a refusal sentence with no row in this file is a sentence nobody has \
         said whether an agent can see. R20 put two in `catalogue.rs` and the gate that was \
         supposed to hold them scanned `invert.rs` for names ending `_REFUSAL`, so it passed \
         without looking. Add a row saying which road each travels: {unaccounted:?}"
    );
}

/// 🔴 And the other direction: a row here names a constant that exists. Without this, the inventory
/// rots into a list of things that used to be true.
#[test]
fn every_row_in_the_census_names_a_sentence_this_crate_still_ships() {
    let found = refusal_sentences();
    let gone: Vec<&str> = CENSUS
        .iter()
        .filter(|(name, _, _)| !found.contains_key(*name))
        .map(|(name, _, _)| *name)
        .collect();
    println!("R22_L02_GONE {gone:?}");
    assert!(
        gone.is_empty(),
        "🔴 these rows name constants `gx-adapter-mcp` no longer declares (or no longer writes as \
         a remedy-bearing sentence): {gone:?}"
    );
}

/// 🔴 The obligation each family carries, checked against `wrap.rs`'s classifier list.
///
/// * A **runtime** sentence must be in `DECLARATION_REFUSALS`, or an agent is told `not-reached` —
///   *"this is not a refusal"* — about a call gx refused (`req/291` M-03's defect).
/// * A **parse-time** sentence must **not** be, because `Catalogue::from_json` turns it into a
///   start-up error the classifier can never be handed. A row there would be a claim about a road
///   that does not exist, and the next reader would have to work out which.
#[test]
fn each_family_meets_the_obligation_its_road_carries() {
    let wrap = cli_src("wrap.rs");
    let wired: Vec<String> = wrap
        .match_indices("gx_adapter_mcp::")
        .map(|(at, marker)| {
            let rest = &wrap[at + marker.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_uppercase() || c == '_'))
                .unwrap_or(rest.len());
            rest[..end].to_string()
        })
        .filter(|name| !name.is_empty())
        .collect();
    println!("R22_L02_WIRED n={} {:?}", wired.len(), wired);
    assert!(
        !wired.is_empty(),
        "🔴 the scan of `wrap.rs` found no `gx_adapter_mcp::…` constant at all, which is the one \
         way this gate passes by measuring nothing"
    );
    let mut wrong: Vec<String> = Vec::new();
    for (name, file, family) in CENSUS {
        let is_wired = wired.iter().any(|w| w == name);
        match (family, is_wired) {
            (Family::Runtime, false) => wrong.push(format!(
                "{file}: {name} travels to an agent and is not in DECLARATION_REFUSALS"
            )),
            (Family::ParseTime, true) => wrong.push(format!(
                "{file}: {name} is a parse error and is listed as a runtime refusal"
            )),
            // 🔴 **`req/312` M-01 (R23)** — a post-apply sentence in the classifier's list would
            // make `gx wrap` answer `refused-before-verdict` about a call that was admitted, sent
            // and arrived. See [`Family::PostApply`].
            (Family::PostApply, true) => wrong.push(format!(
                "{file}: {name} is produced after the call was sent and is listed as a declaration \
                 refusal"
            )),
            // 🔴 **`req/320` M-01 (R25)** — a pre-verdict sentence in the classifier's list would
            // make `gx wrap` answer `refused-before-verdict` about a declaration that is not at
            // fault. See [`Family::PreVerdict`].
            (Family::PreVerdict, true) => wrong.push(format!(
                "{file}: {name} is produced before a verdict exists and is listed as a declaration \
                 refusal"
            )),
            _ => {}
        }
        println!("R22_L02_FAMILY {name} file={file} family={family:?} wired={is_wired}");
    }
    assert!(
        wrong.is_empty(),
        "🔴 `req/303` L-02 / `req/291` M-03: {wrong:?}"
    );
    // 🔴 **`req/312` M-01 (R23)** — the new family's *other* half, which the loop above cannot
    // express. Without this arm `PostApply` is a way for any sentence to opt out of wiring by
    // naming itself, which is a gate with a door in it.
    let post_apply: Vec<&str> = CENSUS
        .iter()
        .filter(|(_, _, family)| *family == Family::PostApply)
        .map(|(name, file, _)| {
            assert_eq!(
                *file, "apply.rs",
                "a `PostApply` sentence is produced by the post-call observation, and that lives \
                 in `apply.rs`: {name} is declared in {file}"
            );
            *name
        })
        .collect();
    println!("R23_POST_APPLY {post_apply:?}");
    assert_eq!(
        post_apply.len(),
        1,
        "one post-apply sentence today (`OBSERVATION_NOT_ANSWERED`). A second is not forbidden and \
         is not silent either: this number moves with it, so whoever adds one has to say so"
    );
    // 🔴 **`req/320` M-01 (R25)** — the same two halves for the fourth family, for the same reason:
    // without them `PreVerdict` is a door any sentence can walk through by naming itself.
    let pre_verdict: Vec<&str> = CENSUS
        .iter()
        .filter(|(_, _, family)| *family == Family::PreVerdict)
        .map(|(name, file, _)| {
            assert_eq!(
                *file, "adapter.rs",
                "a `PreVerdict` sentence is produced by `snapshot` / `precondition`, and those live \
                 in `adapter.rs`: {name} is declared in {file}"
            );
            *name
        })
        .collect();
    println!("R25_PRE_VERDICT {pre_verdict:?}");
    assert_eq!(
        pre_verdict.len(),
        1,
        "one pre-verdict sentence today (`READ_ANSWERED_THIS_LOCATOR_IS_ABSENT`). A second is not \
         forbidden and is not silent either: this number moves with it"
    );
}
