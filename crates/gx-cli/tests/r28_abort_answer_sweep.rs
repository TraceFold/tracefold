// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 1 (`req/338` §0-1, from `req/334` M-01)** — every road on **every surface** that
//! answers *this transformation aborted* asks Σ what became of the roll-back.
//!
//! # Why this file exists beside `r27_reentrant_abort.rs`
//!
//! R27 built a sibling sweep for exactly this property and it was right about the shape: derive the
//! roads from the source rather than enumerate them, so a road added tomorrow enters the census
//! that day. The twenty-seventh audit then walked straight through it, and the reason is the
//! sweep's **domain**, not its idea:
//!
//! * the directory it walks is `env!("CARGO_MANIFEST_DIR")/src` = **gx-cli, one crate**; and
//! * the thing it selects on is `line.contains("\"aborted\"")` = **one surface's JSON member name**.
//!
//! `gx-api` is outside the first and does not write the second — its refusals are RFC 9457 problem
//! objects, which have no `aborted` member and never will. So `halted_undo` answered an aborted undo
//! with no word about the roll-back, and no census could see it. Widening the directory would not
//! have helped: *a census whose selector is one surface's wire vocabulary cannot see another
//! surface, however wide its directory is made.*
//!
//! This sweep is therefore built on two derivations and no literals of either kind:
//!
//! 1. **Which abort reasons can carry a roll-back at all** is read out of `gx-engine`'s own source —
//!    the `self.abort(id, AbortReason::X, <rollback>, at)` call sites whose rollback argument is not
//!    `None`. Today that is exactly one reason; if a second is given one tomorrow, every answering
//!    road on every surface enters this denominator on that day, with nobody remembering.
//! 2. **Which crate owns the accessor** is read out of the source too (whoever declares
//!    `pub fn rollback(`), and is excluded — the engine *writes* the value, and requiring the writer
//!    to ask its own reader would be requiring a function to call itself.
//!
//! `r27_reentrant_abort.rs` is **not edited**. `req/38` §234 ruling 2 made it a standing rule that a
//! calibrated instrument is not edited to satisfy another one, and R27's arm holds a real property
//! (the gx-cli roads, on the wire vocabulary gx-cli actually speaks) that stays held.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Measured values go to stdout **and** to a file.
fn record(line: &str) {
    println!("{line}");
    let path = std::env::var("R28_MEAS")
        .unwrap_or_else(|_| "/mnt/c/work/r28_logs/r28_measurements.txt".to_string());
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

/// `crates/`, from the crate this suite is compiled into.
fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gx-cli sits in crates/")
        .to_path_buf()
}

/// Source with `//` lines dropped: a road named in prose is not a road.
fn code_of(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(str::to_string)
        .collect()
}

/// Every `.rs` file under `crates/*/src`, with its crate name.
fn workspace_sources() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let root = crates_dir();
    let mut crate_dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("crates/ is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    crate_dirs.sort();
    for dir in crate_dirs {
        let name = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let src = dir.join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src];
        while let Some(here) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&here) else {
                continue;
            };
            let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for path in paths {
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push((name.clone(), path));
                }
            }
        }
    }
    out
}

/// 🔴 **Derivation 1** — the abort reasons the engine ever records a roll-back for.
///
/// Read off the `Engine::abort` call sites: the fourth argument is `Option<Rollback>`, and a site
/// that passes `None` is a site where no roll-back was ever in question. This is the narrowing R27
/// applied by hand when it excluded `canonicalise_refusal` ("asking what became of a roll-back that
/// was never in question"), computed instead of remembered.
fn reasons_that_carry_a_rollback() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for (_, path) in workspace_sources() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let joined = code_of(&text).join("\n");
        // `self.abort(` … `AbortReason::X,` … `<rollback>,` — read as a flat token walk so that a
        // call wrapped over five lines by rustfmt reads the same as one written on one.
        for piece in joined.split(".abort(").skip(1) {
            let head: String = piece.chars().take(400).collect();
            let Some(after) = head.split("AbortReason::").nth(1) else {
                continue;
            };
            let reason: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let rest = after
                .split_once(',')
                .map(|(_, r)| r.trim_start())
                .unwrap_or("");
            if reason.is_empty() || rest.starts_with("None") {
                continue;
            }
            found.insert(reason);
        }
    }
    found
}

/// 🔴 **Derivation 2** — the crate that declares the Σ accessor, which is excluded from the sweep.
fn crate_that_owns_the_accessor() -> Option<String> {
    for (name, path) in workspace_sources() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.contains("pub fn rollback(") {
            return Some(name);
        }
    }
    None
}

/// One road: a function, its file, and whether its body asks Σ about the roll-back.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Road {
    krate: String,
    road: String,
    asks: bool,
}

/// Every function on every surface whose body is keyed on a reason that can carry a roll-back.
///
/// `lines` is taken rather than read so that the mutation arm can fire this at text that is not the
/// shipped file — which is what makes it a mutation rather than a description.
fn roads_in(krate: &str, file: &str, lines: &[String], reasons: &BTreeSet<String>) -> Vec<Road> {
    let mut out = Vec::new();
    let heads: Vec<usize> = (0..lines.len())
        .filter(|i| {
            let t = lines[*i].trim_start();
            t.starts_with("fn ")
                || t.starts_with("pub fn ")
                || t.starts_with("async fn ")
                || t.starts_with("pub async fn ")
                || t.starts_with("pub(crate) fn ")
        })
        .collect();
    for start in heads {
        let header = &lines[start];
        // A `#[test]` inside `src` is a test, not a road an operator reads.
        let attributed_test = start > 0
            && lines[start.saturating_sub(3)..start]
                .iter()
                .any(|l| l.trim_start().starts_with("#[test]"));
        if attributed_test {
            continue;
        }
        let Some(name) = header
            .split("fn ")
            .nth(1)
            .and_then(|r| r.split(['(', '<']).next())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let indent = header.len() - header.trim_start().len();
        let mut body = String::new();
        for candidate in lines.iter().skip(start + 1) {
            let c_trim = candidate.trim_start();
            let c_indent = candidate.len() - c_trim.len();
            if c_trim.starts_with('}') && c_indent <= indent {
                break;
            }
            body.push_str(candidate);
            body.push('\n');
        }
        if !reasons
            .iter()
            .any(|r| body.contains(&format!("AbortReason::{r}")))
        {
            continue;
        }
        out.push(Road {
            krate: krate.to_string(),
            road: format!("{file}::{name}"),
            asks: asks_the_mechanism(&body),
        });
    }
    out
}

/// 🔴 **R29 / `req/361` M-01** — whether this body **calls** the machinery that answers about the
/// roll-back, rather than whether the word appears somewhere in it.
///
/// # The defect this closes
///
/// R28's predicate was `body.contains("rollback")`, and the twenty-eighth audit satisfied it with
/// prose. `code_of()` drops `//` lines — the instrument's own doc says *"a road named in prose is
/// not a road"* — but a **string literal** is not a comment, and R28 shipped a `detail` sentence
/// containing the word `rollback` on the very road the sweep was built to watch. The audit deleted
/// the two mechanism lines from a copy of the source and the predicate still answered `true`:
///
/// ```text
/// A28_SWEEP_PROSE asks_today=true lines_removed=2 asks_without_the_mechanism=true
/// ```
///
/// So the gate could not have caught the removal it exists to catch. The road really was asking —
/// today's coverage had no hole in it — and what was broken was the **guarantee**, which is why
/// `req/38` §238 ruling 3 took it as an M rather than an H.
///
/// # Why call-shapes and not a literal-stripper
///
/// `req/361` §9-5 recommended this side explicitly: stripping string literals means writing a Rust
/// lexer inside a test (raw strings, escapes, nested quotes, `r#"…"#`), and a census that needs a
/// parser to be correct is the next audit's finding. A call is a narrower thing to look for and it
/// is the thing actually meant: *does this road go and get the fact?* Prose cannot wear these
/// shapes by accident, because they carry the punctuation of a call rather than of a sentence.
///
/// The shapes are the ones the shipped tree uses, and the bed control below asserts each is
/// findable on the real source, so a rename cannot quietly empty this list.
const MECHANISM_CALLS: [&str; 4] = [
    // the two surfaces' shared assembler, and the builder it feeds (gx-api)
    "rollback_facts(",
    "with_rollback_facts",
    // the engine accessors, as called (gx-cli's twin road, and anything reading Σ directly)
    ".rollback(",
    ".rollback_not_attempted_because(",
];

/// `true` when the body **calls** one of [`MECHANISM_CALLS`].
///
/// Deliberately not `contains("rollback")`: a sentence carrying the word is a sentence, and this
/// question is about machinery.
fn asks_the_mechanism(body: &str) -> bool {
    MECHANISM_CALLS.iter().any(|call| body.contains(call))
}

/// The whole denominator, over the shipped tree.
fn shipped_roads() -> (BTreeSet<String>, Vec<Road>) {
    let reasons = reasons_that_carry_a_rollback();
    let owner = crate_that_owns_the_accessor();
    let mut roads = Vec::new();
    for (krate, path) in workspace_sources() {
        if Some(&krate) == owner.as_ref() {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        roads.extend(roads_in(&krate, &file, &code_of(&text), &reasons));
    }
    roads.sort();
    (reasons, roads)
}

// ---------------------------------------------------------------------------
// a — the bed: both derivations answer, and neither answers with everything
// ---------------------------------------------------------------------------

/// 🔴 Without this arm the sweep below could be green because it found **nothing**.
///
/// The twenty-seventh audit's own ninth confession is the reason it is here: "the instrument
/// returned zero three times and three times the instrument was wrong". A derivation that selects
/// no reasons would make the sweep vacuous and green.
#[test]
fn a_bed_control_both_derivations_answer_from_the_source() {
    let reasons = reasons_that_carry_a_rollback();
    let owner = crate_that_owns_the_accessor();
    record(&format!(
        "R28_SWEEP_DERIVATIONS reasons_that_carry_a_rollback={reasons:?} accessor_owner={owner:?}"
    ));
    assert!(
        !reasons.is_empty(),
        "🔴 no abort reason was found to carry a roll-back, which would make the sweep vacuous. \
         Either `Engine::abort`'s call sites changed shape or this walk is wrong — re-read before \
         trusting anything below: {reasons:?}"
    );
    assert!(
        reasons.len() < 6,
        "🔴 the derivation selected every reason there is, which means it is not discriminating: \
         {reasons:?}"
    );
    assert!(
        owner.is_some(),
        "🔴 no crate declares `pub fn rollback(`, so the exclusion below excludes nothing"
    );
    // 🔴 **R29 / `req/361` M-01** — the third derivation this arm now holds: every shape in
    // `MECHANISM_CALLS` is findable in the shipped tree. Without it a rename could empty the
    // predicate and arm `b` would go green because **nothing asks**, which is the vacuity this
    // whole file was built against — the same trap `req/334` §9-3 confessed to three times.
    let shipped: String = workspace_sources()
        .iter()
        .map(|(_, path)| std::fs::read_to_string(path).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    let unfindable: Vec<&str> = MECHANISM_CALLS
        .iter()
        .copied()
        .filter(|call| !shipped.contains(call))
        .collect();
    record(&format!(
        "R29_SWEEP_MECHANISMS shapes={MECHANISM_CALLS:?} unfindable={unfindable:?}"
    ));
    assert!(
        unfindable.is_empty(),
        "🔴 `req/361` M-01: the `asks` predicate looks for call shapes, and these are not in the \
         shipped tree at all — a renamed mechanism would make every road answer `false` and the \
         sweep would be vacuous rather than red: {unfindable:?}"
    );
}

// ---------------------------------------------------------------------------
// b — the sweep itself
// ---------------------------------------------------------------------------

/// 🔴 **`req/334` M-01** — every answering road, on every surface, asks what became of the
/// roll-back.
///
/// The denominator crosses crates by construction, and the arm asserts that it *did* cross: a sweep
/// that found roads in one crate only has reproduced R27's defect with more code.
#[test]
fn b_every_road_on_every_surface_asks_what_became_of_the_rollback() {
    let (reasons, roads) = shipped_roads();
    let crates: BTreeSet<&str> = roads.iter().map(|r| r.krate.as_str()).collect();
    let without: Vec<&str> = roads
        .iter()
        .filter(|r| !r.asks)
        .map(|r| r.road.as_str())
        .collect();
    record(&format!(
        "R28_ABORT_ROADS reasons={reasons:?} crates={crates:?} denominator={} \
         roads={:?} without_the_rollback={without:?}",
        roads.len(),
        roads.iter().map(|r| r.road.as_str()).collect::<Vec<_>>()
    ));
    assert!(
        crates.len() >= 2,
        "🔴 the denominator is one crate again, which is the exact shape `req/334` M-01 walked \
         through: {crates:?}"
    );
    assert!(
        without.is_empty(),
        "🔴 `req/334` M-01: {without:?} answer *this transformation aborted* without asking Σ what \
         became of the roll-back. The value is on a signed record and the accessor is one call \
         away. Two terminal states — the object is back where it was, and the object is half undone \
         — are one answer to a caller that cannot see it."
    );
}

// ---------------------------------------------------------------------------
// c — the mutation: the sweep is a function, not a description
// ---------------------------------------------------------------------------

/// 🔴 A road spliced into a **copy** of the source text, answering about an abort and asking
/// nothing, is caught.
///
/// Without this arm, arm `b` is green on a tree that has no defect and would stay green on a
/// predicate that is broken.
#[test]
fn c_a_road_that_does_not_ask_is_caught_when_one_is_spliced_in() {
    let reasons = reasons_that_carry_a_rollback();
    let reason = reasons
        .iter()
        .next()
        .expect("arm a holds that the set is not empty");
    let silent = format!(
        "
fn a_road_this_lane_spliced_in(state: Lifecycle) -> String {{
    match state {{
        Lifecycle::Aborted(AbortReason::{reason}) => \"it aborted\".to_string(),
        _ => String::new(),
    }}
}}
"
    );
    let lines = code_of(&silent);
    let caught = roads_in("spliced", "spliced.rs", &lines, &reasons);
    record(&format!("R28_SWEEP_MUTATION caught={caught:?}"));
    assert_eq!(
        caught.len(),
        1,
        "🔴 a road keyed on `AbortReason::{reason}` was not seen at all: {caught:?}"
    );
    assert!(
        !caught[0].asks,
        "🔴 the spliced road asks nothing and the predicate says it does: {caught:?}"
    );
}

// ---------------------------------------------------------------------------
// d — the property R27's sweep did not have: independence from any wire vocabulary
// ---------------------------------------------------------------------------

/// 🔴 The selector is the **abort taxonomy**, not any surface's member name.
///
/// Measured rather than asserted about this file's own prose: a road is written twice — once
/// writing gx-cli's `"aborted"` member and once as an RFC 9457 problem object that never will — and
/// the sweep has to see both. That is the exact difference between this sweep and the one
/// `req/334` M-01 walked through.
#[test]
fn d_the_selector_is_the_taxonomy_and_not_one_surfaces_member_name() {
    let reasons = reasons_that_carry_a_rollback();
    let reason = reasons
        .iter()
        .next()
        .expect("arm a holds that the set is not empty");
    let cli_shaped = format!(
        "
fn a_cli_shaped_road(state: Lifecycle) -> Value {{
    let reason = match state {{
        Lifecycle::Aborted(AbortReason::{reason}) => \"{reason}\",
        _ => \"NotAborted\",
    }};
    json!({{ \"aborted\": true, \"reason\": reason, \"detail\": engine.rollback(id) }})
}}
"
    );
    let api_shaped = format!(
        "
fn an_api_shaped_road(state: Lifecycle) -> ApiError {{
    match state {{
        Lifecycle::Aborted(AbortReason::{reason}) => ApiError::new(\"APPLY_FAILED\", \"t\", \"d\"),
        _ => ApiError::internal(\"d\"),
    }}
}}
"
    );
    let cli = roads_in("cli", "cli.rs", &code_of(&cli_shaped), &reasons);
    let api = roads_in("api", "api.rs", &code_of(&api_shaped), &reasons);
    record(&format!(
        "R28_SWEEP_VOCABULARY cli_shaped={cli:?} api_shaped={api:?}"
    ));
    assert_eq!(
        cli.len(),
        1,
        "🔴 the road that does write the `aborted` member was not seen: {cli:?}"
    );
    assert_eq!(
        api.len(),
        1,
        "🔴 `req/334` M-01, in one line: a road keyed on the abort taxonomy that writes no \
         `aborted` member is invisible to a census that selects on that member. This sweep must see \
         it: {api:?}"
    );
    assert!(
        !api[0].asks,
        "🔴 the api-shaped road asks nothing, so the sweep must say so: {api:?}"
    );
    assert!(
        !std::fs::read_to_string(std::path::Path::new(file!()))
            .unwrap_or_default()
            .contains("line.contains(\"\\\"aborted\\\"\")"),
        "🔴 this sweep selects on a wire member name, which is the defect it exists to close"
    );
}
