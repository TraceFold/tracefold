// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R29 items 2, 3 and 4 (`req/364` §0, from `req/361` M-01 / L-01 / L-02)** — the three
//! repairs this lane made to **instruments** rather than to the product, held from the outside.
//!
//! # Why these three sit in one file
//!
//! Because they are one failure wearing three costumes, and `req/361` §2 named the pattern while
//! filing them: *a census that walks into the same trap it was built to detect.* The sweep that
//! asks "does this road go and get the roll-back fact" answered yes to a road that only **mentioned**
//! the word. The census that announces "we now count markers a line-oriented scan cannot see"
//! could not see one of them itself. The walks that accumulate a constant's body stopped on a
//! spelling habit and would have swallowed the rest of a file in silence. In every case the gate's
//! verdict was right and its **guarantee** was not, which is exactly why `req/38` §238 took the
//! first as an M and the other two as L: today's coverage had no hole in it, and tomorrow's had no
//! floor under it.
//!
//! # 🔴 The rule this file follows about copying
//!
//! An instrument gate that restates the instrument's own data has produced the drift it was built
//! to catch — `req/324` §9-1's lineage is four examples long now. So arm `b` below **parses the
//! call shapes out of the shipped sweep's source** rather than declaring its own list, and arm `e`
//! **re-implements the marker gate's rejoin algorithm** deliberately, because for that one the
//! second implementation *is* the independent derivation: the twenty-eighth audit's first attempt
//! at this measurement used a byte-level `split("\\\n")`, disagreed with the gate by three, and was
//! wrong. It only agreed once it reproduced the gate's line-oriented rejoin exactly. A number that
//! two different hands reach the same way is worth more here than a number reached once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn record(line: &str) {
    println!("{line}");
    let Ok(path) = std::env::var("R29_MEAS") else {
        return;
    };
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("gx-cli sits in crates/")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("🔴 {rel} is not readable, so nothing below measures it: {e}"))
}

/// The same text with every comment line dropped — `r28_abort_answer_sweep.rs`'s `code_of`, and
/// for its stated reason: *a road named in prose is not a road*.
///
/// 🔴 **Written after this file caught itself.** The first version of arm `d` asked whether the
/// three gates still contain `rest.contains("&str")` and read the whole file to find out — so it
/// went red on all three the moment the repair's own doc comment **quoted the old predicate to
/// explain what had been wrong with it**. That is `req/361` M-01 exactly, committed by the gate
/// written to hold `req/361` M-01, three arms further down the same file. Recorded rather than
/// quietly fixed, because the lesson is the file's subject: any assertion of the form *this
/// spelling is gone from the source* has to be made against code, and prose that discusses a
/// spelling is not that spelling.
fn code_of(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The three live gates that carried the `";`-terminated walk `req/361` L-02 filed.
const WALKING_GATES: [&str; 3] = [
    "crates/gx-cli/tests/r22_refusal_constant_census.rs",
    "crates/gx-cli/tests/r26_refusal_remedy_parity.rs",
    "crates/gx-cli/tests/r27_parity_allowlist.rs",
];

/// The shipped sweep whose `asks` predicate `req/361` M-01 satisfied with prose.
const SWEEP: &str = "crates/gx-cli/tests/r28_abort_answer_sweep.rs";

/// The road whose `detail` sentence carried the word that satisfied the old predicate.
const ANSWERING_ROAD: &str = "crates/gx-api/src/handlers.rs";

/// The gate whose announcing paragraph `req/361` L-01 found undercounting.
const MARKER_GATE: &str = "crates/gx-cli/tests/r28_remedy_marker.rs";

// ---------------------------------------------------------------------------
// a — bed control
// ---------------------------------------------------------------------------

/// 🔴 **Bed control** — every file the arms below measure is present and substantial.
///
/// `req/334` §9-3's confession is the reason this arm exists: *"the instrument returned zero three
/// times and three times the instrument was wrong."* An arm that reads an empty string finds no
/// defect in it, and every assertion below is of the shape "this text does **not** contain the old
/// spelling" — the shape that passes most eagerly when the text is missing.
#[test]
fn a_bed_control_every_instrument_this_file_measures_is_present() {
    let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
    for rel in WALKING_GATES
        .iter()
        .chain([&SWEEP, &ANSWERING_ROAD, &MARKER_GATE])
    {
        sizes.insert(rel, read(rel).len());
    }
    record(&format!("R29_PRED_BED sizes={sizes:?}"));
    let thin: Vec<&&str> = sizes
        .iter()
        .filter(|(_, n)| **n < 2000)
        .map(|(k, _)| k)
        .collect();
    assert!(
        thin.is_empty(),
        "🔴 an arm below would read one of these as 'the old spelling is gone' when what is gone \
         is the file: {thin:?}"
    );
}

// ---------------------------------------------------------------------------
// b — `req/361` M-01: the sweep's `asks` is about machinery, not about words
// ---------------------------------------------------------------------------

/// The call shapes the shipped sweep looks for, **read out of the sweep itself**.
///
/// Deliberately parsed rather than restated: a second declaration here would be a second source of
/// truth about what "asks" means, and the two would part company on the day one of them was
/// renamed — which is the class of defect this whole file is about.
fn mechanism_shapes(sweep: &str) -> Vec<String> {
    let Some((_, after)) = sweep.split_once("const MECHANISM_CALLS") else {
        return Vec::new();
    };
    let Some((body, _)) = after.split_once("];") else {
        return Vec::new();
    };
    body.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// 🔴 **`req/361` M-01** — deleting the mechanism makes the sweep say `false`.
///
/// # The measurement this reverses
///
/// The twenty-eighth audit took the shipped body of `halted_undo`, removed **the two mechanism
/// lines** — the `rollback_facts(` call and the `with_rollback_facts` builder — from a copy of the
/// text, and applied R28's predicate `body.contains("rollback")` to what was left:
///
/// ```text
/// A28_SWEEP_PROSE asks_today=true lines_removed=2 asks_without_the_mechanism=true
/// ```
///
/// **True with the machinery gone**, because R28 had shipped a `detail` sentence containing the
/// word `rollback` on that very road. `code_of()` drops `//` lines — the sweep's own doc says *"a
/// road named in prose is not a road"* — but a string literal is not a comment. So the gate could
/// not have caught the removal it exists to catch: the road was genuinely asking, and the promise
/// that it would keep asking was worth nothing.
///
/// This arm is that measurement with the expected answer flipped, and it holds both halves: the
/// shipped road still asks (or the sweep is measuring nothing), and the stripped copy does not.
#[test]
fn b_the_sweeps_asks_predicate_is_no_longer_satisfied_by_prose() {
    let sweep = read(SWEEP);
    let shapes = mechanism_shapes(&sweep);
    assert!(
        shapes.len() >= 2,
        "🔴 the call shapes could not be parsed out of {SWEEP}, so this arm is measuring nothing \
         rather than measuring a repair: {shapes:?}"
    );
    let asks = |text: &str| shapes.iter().any(|shape| text.contains(shape.as_str()));

    let road = read(ANSWERING_ROAD);
    let stripped: String = road
        .lines()
        .filter(|l| !shapes.iter().any(|shape| l.contains(shape.as_str())))
        .collect::<Vec<_>>()
        .join("\n");
    let removed = road.lines().count() - stripped.lines().count();

    let prose_literal_present = stripped.contains("rollback");
    let substring_predicate_would_still_say_yes = prose_literal_present;

    record(&format!(
        "R29_SWEEP_PROSE asks_today={} lines_removed={removed} \
         asks_without_the_mechanism={} prose_literal_present={prose_literal_present} \
         old_substring_predicate_would_say={substring_predicate_would_still_say_yes}",
        asks(&road),
        asks(&stripped),
    ));

    assert!(
        asks(&road),
        "🔴 the shipped answering road does not call any mechanism shape, so this sweep is vacuous \
         rather than satisfied: {shapes:?}"
    );
    assert!(
        removed >= 2,
        "🔴 removing the mechanism removed {removed} lines; the audit removed two. If the shapes \
         stopped matching the road, this arm is not the mutation it claims to be"
    );
    assert!(
        !asks(&stripped),
        "🔴 `req/361` M-01 is **not** repaired: the machinery was deleted from a copy of \
         {ANSWERING_ROAD} and the predicate still answers `true`. A gate that cannot notice the \
         removal it exists to notice is a gate that will report this road as answering on the day \
         it stops."
    );
    assert!(
        prose_literal_present,
        "🔴 the control for the arm above: the word `rollback` must still be present in the \
         stripped copy, or `!asks` is true because the word is gone rather than because the \
         predicate stopped believing prose — which would make this arm pass for the wrong reason"
    );
    assert!(
        !code_of(&sweep).contains("asks: body.contains(\"rollback\")"),
        "🔴 the shipped sweep still spells its `asks` predicate as a bare substring test"
    );
}

// ---------------------------------------------------------------------------
// c — `req/361` L-02: the walk closes on syntax, and refuses to swallow in silence
// ---------------------------------------------------------------------------

/// 🔴 **`req/361` L-02** — the three live walks no longer end on a spelling, and cannot swallow
/// quietly.
///
/// # What was measured, and what was **not** wrong
///
/// The audit ran the shipped walk over its own scan directory and reported the honest result:
///
/// ```text
/// A28_TERMINATOR files=12 selected=27 unterminated_today=[]
/// A28_TERMINATOR spliced_selected=1 spliced_unterminated=["A28_SPLICED"]
/// ```
///
/// **Nothing is swallowed today.** Twenty-seven constants are selected and all twenty-seven reach a
/// terminator, so no census in this repository is currently wrong. What the second line says is
/// that one ordinary alternative spelling — `pub const NAME: &str = other_module::VALUE;`, a form
/// this repository already uses in `crates/gx-cli/src/keys.rs` — makes the walk run past the end of
/// the declaration and keep going, silently. The gates' own comment declared the assumption
/// (*"Walked rather than parsed: this crate writes them all one way"*) and declaring an assumption
/// is not enforcing it. `req/324` §9-1 and `req/332` §5 had each filed this shape in a **frozen**
/// artifact; the twenty-eighth audit found the third instance alive.
///
/// Two things therefore have to be true of each of the three, and this arm holds both: the
/// terminator is no longer the two characters `";`, and a body that reaches the next item without
/// closing **panics by name** instead of being returned truncated.
#[test]
fn c_the_three_live_walks_close_on_syntax_and_refuse_to_swallow() {
    let mut still_spelling: Vec<&str> = Vec::new();
    let mut without_the_refusal: Vec<&str> = Vec::new();
    let mut not_sharing_the_walk: Vec<&str> = Vec::new();
    for rel in WALKING_GATES {
        let text = code_of(&read(rel));
        if text.contains("ends_with(\"\\\";\")") {
            still_spelling.push(rel);
        }
        if !text.contains("fn walk_const_body(") || !text.contains("walk_const_body(&lines,") {
            not_sharing_the_walk.push(rel);
        }
        // The refusal has to be reachable from the "we reached the next item" road, or it is a
        // `panic!` that decorates the file without guarding anything.
        let reaches_the_panic = text.contains("starts_a_new_item(trimmed)") && {
            let after = text.split_once("fn walk_const_body(").map(|(_, r)| r);
            after.is_some_and(|r| {
                r.split_once("fn starts_a_new_item")
                    .map_or(r, |(body, _)| body)
                    .contains("panic!(")
            })
        };
        if !reaches_the_panic {
            without_the_refusal.push(rel);
        }
    }
    record(&format!(
        "R29_TERMINATOR still_spelling={still_spelling:?} without_the_refusal={without_the_refusal:?} \
         not_sharing_the_walk={not_sharing_the_walk:?}"
    ));
    assert!(
        still_spelling.is_empty(),
        "🔴 `req/361` L-02: these gates still close a constant's body on the two characters `\";`, \
         so they are correct only for as long as one directory keeps spelling its constants one \
         way: {still_spelling:?}"
    );
    assert!(
        not_sharing_the_walk.is_empty(),
        "🔴 these gates no longer go through the repaired walk, so the repair is not theirs: \
         {not_sharing_the_walk:?}"
    );
    assert!(
        without_the_refusal.is_empty(),
        "🔴 the walk in these gates can still reach the end of an item without a terminator and \
         return what it accumulated. Swallowing is allowed to happen only where somebody has to \
         read about it: {without_the_refusal:?}"
    );
}

// ---------------------------------------------------------------------------
// d — `req/361` L-02, the other half: the selection predicate
// ---------------------------------------------------------------------------

/// 🔴 **`req/361` L-02 (second half)** — `&'static str` is a string constant, and an array is not.
///
/// # 🔴 The corrected reason, kept because the wrong one had the same conclusion
///
/// A delegated recon agent reported the walk was dormant because *the scanned directory holds no
/// array-typed constants*. That is false — `crates/gx-adapter-mcp/src/adapter.rs` declares
/// `pub const ALL_FACTS: [&'static str; 4]` — and the audit caught it by grepping rather than
/// inheriting the claim. The real reason is this arm's subject: the selector was
/// `rest.contains("&str")`, and the spelling `&'static str` does not contain that substring. Same
/// verdict, different reason, and a different day on which it breaks — which is why the reason is
/// recorded and not just the verdict.
///
/// The repaired selector reads the type between the `:` and the `=` and asks for a reference ending
/// in `str`. This arm measures what that admits **on the real directory**: the count must not move
/// today (27, exactly as the audit measured), the two array constants must still be excluded, and
/// the old substring must be gone from all three gates.
#[test]
fn d_the_selection_predicate_admits_static_str_and_still_excludes_arrays() {
    let dir = repo_root().join("crates/gx-adapter-mcp/src");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the scanned directory is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    paths.sort();

    // The repaired predicate, applied here exactly as the gates apply it.
    let selects = |rest: &str| -> bool {
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
    };

    let (mut selected, mut arrays, mut declarations) = (0usize, Vec::new(), 0usize);
    for path in &paths {
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub const ") else {
                continue;
            };
            if rest.trim_start().starts_with("fn ") {
                continue;
            }
            declarations += 1;
            if selects(rest) {
                selected += 1;
            }
            if rest.contains(": [") {
                arrays.push(
                    rest.split(':')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                );
            }
        }
    }
    let old_predicate_still_shipped: Vec<&str> = WALKING_GATES
        .into_iter()
        .filter(|rel| code_of(&read(rel)).contains("rest.contains(\"&str\")"))
        .collect();
    record(&format!(
        "R29_SELECTION files={} declarations={declarations} selected={selected} arrays={arrays:?} \
         old_predicate_still_shipped={old_predicate_still_shipped:?}",
        paths.len()
    ));
    assert_eq!(
        // 🔴 **DR-46-28** — 27 until this window; `catalogue.rs` gained
        // `DETERMINISM_BOUNDARY_KEY`, the fourth reserved slot (`req/459` ruling 1). The number
        // this arm holds is a **population**, not the repair's own claim: `req/361` L-02 ruled
        // that the repaired *selector* must not move the population by itself, and the audit's
        // 27 is kept in the sentence below so a reader can see which of the two moved it. A
        // slot added to the declaration face is exactly what this count exists to notice.
        selected, 28,
        "🔴 the repaired selector changed what this directory yields. `req/361` L-02 is a repair to \
         a **latent** defect — the audit measured 27 selected and 27 terminating (DR-46-28 added the \
         twenty-eighth), and a repair that moves today's population is doing something other \
         than what was ruled"
    );
    assert!(
        !arrays.is_empty(),
        "🔴 the control that kills the recon claim this repair's reason was corrected from: if this \
         directory really held no array constants, the reason `&'static str` was missed would be \
         unmeasurable here"
    );
    assert!(
        arrays.len() < selected,
        "🔴 the arrays must be a strict minority and must not be what was selected: {arrays:?}"
    );
    assert!(
        old_predicate_still_shipped.is_empty(),
        "🔴 these gates still select constants with `contains(\"&str\")`, which does not match \
         `&'static str`: {old_predicate_still_shipped:?}"
    );
}

// ---------------------------------------------------------------------------
// e — `req/361` L-01: the announcing paragraph, and the arithmetic that closes on it
// ---------------------------------------------------------------------------

/// The marker gate's own rejoin, re-implemented **line by line** as the gate writes it.
///
/// The twenty-eighth audit's §8-2 is the reason this is a faithful copy rather than a clever
/// equivalent: its first attempt rejoined at the byte level (`text.split("\\\n")`), got 37 where
/// the gate got 40, and could not explain the difference. The gate trims the end of each line and
/// then asks whether it ends in a backslash; a byte-level split misses every continuation whose
/// backslash has trailing whitespace after it. Reproducing the algorithm was what made the numbers
/// agree, and a number that disagrees with the gate is a finding about the measurer first.
fn rejoined(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut continuing = false;
    for line in text.lines() {
        let piece = if continuing { line.trim_start() } else { line };
        let trimmed = piece.trim_end();
        if let Some(head) = trimmed.strip_suffix('\\') {
            out.push_str(head);
            continuing = true;
        } else {
            out.push_str(piece);
            out.push('\n');
            continuing = false;
        }
    }
    out
}

/// Every `.rs` file under `crates/*/src`, which is the marker gate's own denominator.
fn workspace_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut crate_dirs: Vec<PathBuf> = std::fs::read_dir(repo_root().join("crates"))
        .expect("crates/ is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    crate_dirs.sort();
    for dir in crate_dirs {
        let mut stack = vec![dir.join("src")];
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
                    out.push(path);
                }
            }
        }
    }
    out
}

/// 🔴 **`req/361` L-01** — the paragraph that announces the recount now states the numbers the
/// recount produces, and names all three files.
///
/// # The defect, and where exactly it lived
///
/// R28 found that a line-oriented scan cannot see a marker split across a `\`+newline continuation,
/// repaired the census to rejoin first, and then **announced the repair with the naive numbers plus
/// one**. The shipped doc comment and `docs/LIMITS.md` both said one occurrence of each spelling was
/// hidden — `26 and 10`. Measured on the base R28 was written against (`f1fbd9d`, `crates/*/src`,
/// 138 files) the rejoined pair is **`27 and 10`** and the hidden markers are **three**: two of the
/// kept spelling and one of the retired one, in `catalogue.rs`, `invert.rs` and `repair.rs`.
/// `invert.rs` is named by neither sentence.
///
/// 🔴 **The decisive evidence is arithmetic, and it is why this is L rather than nothing.** The
/// gate prints `kept=40` on today's tree. Against the announced base that is `26 + 10 = 36` plus
/// R28's three new sentences = `39`, and the gate's own verdict is unexplained by its own
/// paragraph. Against the measured base it closes exactly: `27 + 10 = 37`, `+3` = **40**. And `37`
/// is the number R28's *report* (`req/349` §1) already carried. **Nothing was wrong with the
/// measurement or with the gate** — `kept=40 retired=0` was true, and the unification really is
/// complete. What drifted was the step that engraves a report into a shipped doc and onto the page
/// a buyer reads, which is the step no instrument was watching.
#[test]
fn e_the_announcing_paragraph_states_the_numbers_the_recount_produces() {
    const MARKER: &str = "What to fix:";
    const RETIRED: &str = "What to do:";
    let (mut naive, mut joined, mut split_files) = (0usize, 0usize, Vec::new());
    for path in workspace_sources() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let rejoin = rejoined(&text);
        let before = text.matches(MARKER).count() + text.matches(RETIRED).count();
        let after = rejoin.matches(MARKER).count() + rejoin.matches(RETIRED).count();
        naive += before;
        joined += after;
        if after != before {
            split_files.push(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    split_files.sort();
    record(&format!(
        "R29_MARKER_SPLITS naive={naive} rejoined={joined} \
         files_with_a_hidden_marker={split_files:?}"
    ));

    let doc = read(MARKER_GATE);
    let page = read("docs/LIMITS.md");
    let states_27_and_10 = doc.contains("they are **27 and 10**");
    let names_invert = doc.contains("invert.rs");
    let page_corrects = page.contains("It misses three") || page.contains("**`27 / 10`**");
    record(&format!(
        "R29_MARKER_NARRATIVE doc_states_the_corrected_pair={states_27_and_10} \
         doc_names_invert_rs={names_invert} page_carries_the_correction={page_corrects}"
    ));

    assert_eq!(
        split_files.len(),
        3,
        "🔴 `req/361` L-01 rests on there being three hidden markers, not one of each spelling. If \
         this tree now has a different number, the paragraph has to be re-derived rather than \
         re-run: {split_files:?}"
    );
    assert_eq!(
        joined, 40,
        "🔴 the rejoined census of this tree is what the marker gate prints as `kept`; the two are \
         the same measurement and they have to agree"
    );
    assert!(
        joined > naive,
        "🔴 rejoining found nothing, so this arm is not measuring the thing `req/361` L-01 is about"
    );
    assert_eq!(
        27 + 10 + 3,
        joined,
        "🔴 the arithmetic that made L-01 decisive: the corrected base pair (27 + 10 = 37) plus \
         R28's three new sentences is exactly what the gate counts today. If this stops closing, \
         either the base numbers or the count of new sentences has moved and the paragraph is \
         stale again"
    );
    assert!(
        states_27_and_10,
        "🔴 `req/361` L-01: the paragraph announcing the recount still does not state the pair the          recount produces. The shipped sentence said `26 and 10`; the measurement says `27 and 10`,          and a correction that is not written in the announcing paragraph is a correction the next          reader will not find"
    );
    assert!(
        names_invert,
        "🔴 `req/361` L-01: the paragraph names the files whose markers are hidden and \
         `gx-adapter-mcp/src/invert.rs` is the one it left out. A census that names two of three \
         files is a census a reader will trust for the third"
    );
    assert!(
        page_corrects,
        "🔴 `docs/LIMITS.md` still tells a buyer the rejoining census misses 'one occurrence of \
         each spelling'. The page is additive, so the old block stays — but the reader's most \
         recent answer has to be true of the tree"
    );
}
