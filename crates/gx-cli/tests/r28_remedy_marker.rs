// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 4 (`req/338` §0-4, from `req/334` L-01)** — the remedy marker has **one** spelling,
//! and the census that says so rejoins line continuations before it counts.
//!
//! # The ruling this file implements
//!
//! The twenty-seventh audit found two spellings of the marker a refusal uses to introduce its
//! remedy — `What to fix:` and `What to do:` — and filed the question rather than answering it:
//! *is the difference a design, or an accident?* The parity gate knows only the first and scans one
//! directory, so widening the scan (which `req/329` L-02 asks for) would file real remedies as
//! absent ones.
//!
//! R28 read all of the `What to do:` sentences before ruling. The distinction does **not** hold:
//!
//! * four of them are coping actions where the cause is not the reader's to repair (restore `.gx/`
//!   from a backup; move an untrusted `head.json` aside);
//! * four are fully specified corrective actions the reader controls — the same kind of sentence
//!   the `What to fix:` constants carry;
//! * two are mixed, with one clause of each in a single sentence.
//!
//! The decisive pair is `gx-engine/src/pipeline.rs`'s receipt-archive failure (`What to fix:` — "the
//! write permission on `.gx/receipts/`, or the disk it is on") against `gx-api/src/handlers.rs`'s
//! **same failure mode** (`What to do:` — "read the sentence in brackets above … then `gx repair`").
//! One cause class, two markers. There is no rule being followed, so there is nothing to declare —
//! and a marker's job is to say *a remedy is here*, not to classify it. The kind of remedy is the
//! sentence's business and the sentences keep their own words; only the marker is unified.
//!
//! # 🔴 The census had to be repaired before it could be trusted
//!
//! Counted naively, the two spellings are 25 and 9. Counted after rejoining Rust's `\`+newline
//! continuations — which is what `r26_refusal_remedy_parity.rs` already does, because `req/324`
//! §9-1 filed this exact class once before — they are **26 and 10**: one marker of each spelling is
//! split across a continuation and invisible to a line-oriented scan
//! (`gx-adapter-mcp/src/catalogue.rs`'s `TEMPLATE_NAMES_NO_PRIOR`, and
//! `gx-cli/src/repair.rs`'s `declaration_changed`). The audit's own numbers were the naive ones.
//! A census that cannot see a marker is a census that would have reported this repair complete
//! while one occurrence of the retired spelling was still shipping.
//!
//! # 🔴 R29 / `req/361` L-01 — correction to the paragraph above (additive; the old text stands)
//!
//! **Counted after rejoining, they are **27 and 10** — not `26 and 10` — and the markers split
//! across a continuation are `3`, not "one of each spelling".** The twenty-eighth audit filed it and R29
//! re-derived it independently by re-implementing this file's own [`rejoined`] over the same base
//! (`f1fbd9d`, `crates/*/src`, 138 files):
//!
//! ```text
//! base f1fbd9d : naive fix=25 do=9  →  rejoined fix=27 do=10  (total 37)
//!   crates/gx-adapter-mcp/src/catalogue.rs  fix 15→16
//!   crates/gx-adapter-mcp/src/invert.rs     fix  5→6     ← named by neither sentence
//!   crates/gx-cli/src/repair.rs             do   3→4
//! ```
//!
//! The naive `25 / 9` above is right; the rejoined pair and the count of files are not. The third
//! split is `invert.rs`'s `DECLARATION_WITHOUT_A_READ_FACE_REFUSAL`, whose marker breaks as
//! `What \` + `to fix:` — so the sentence "one marker of **each** spelling" undercounts the kept
//! spelling by one and names two of three files.
//!
//! **The arithmetic of this gate's own verdict only closes on the corrected number.** `census()`
//! prints `kept=40` on today's tree. Against the wrong base that is `26 + 10 = 36`, plus R28's
//! three new M-03 sentences = `39`, and the gate's 40 is unexplained. Against the measured base it
//! closes exactly: **`27 + 10 = 37`, `+3` new sentences = `40`.** And 37 is the number R28's own
//! *report* (`req/349` §1, `kept=27 retired=10`) already carried — so what drifted was never the
//! measurement or the gate, but the step that engraves the report into the shipped doc and onto
//! `docs/LIMITS.md`. That is where this correction is aimed.
//!
//! R29 re-measured today's tree the same way: **naive 37 / 0, rejoined 40 / 0, the same 3 files.**
//! `arm d` below holds the corrected numbers so that this paragraph cannot drift again silently.
//!
//! # 🔴 DR-46-21 / `req/836` — the forty-first sentence (additive; every paragraph above stands)
//!
//! Commit `c8a0b38d` (DR-46-21, `req/680`, merged `fbe5c97b` 2026-08-22) added
//! `CAS_OBJECT_DIGEST_REFUSAL` to `crates/gx-adapter-mcp/src/invert.rs`, and its body carries one
//! more `What to fix:` sentence — **intact on one physical line**, so naive and rejoined move
//! together (`37 → 38` naive, `40 → 41` rejoined) and the hidden-marker set stays the same three
//! files. Today's derivation therefore closes as **`27 + 10 = 37`, `+3` R28 sentences, `+1`
//! DR-46-21 = `41`**, and `census()` prints `kept=41`. What DR-46-21's own commit omitted was
//! exactly the engrave step this paragraph exists for — the population gates in
//! `r29_instrument_repairs.rs` (arms `d`/`e`) stood red in both trees until `req/836` supplied
//! the re-derivation, which is those gates doing precisely their job.

use std::path::{Path, PathBuf};

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

/// The marker this build uses. One value, and this file is the gate that keeps it one.
const MARKER: &str = "What to fix:";

/// The spelling R28 retired. Named so the gate can prove it is gone rather than assume it.
const RETIRED: &str = "What to do:";

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gx-cli sits in crates/")
        .to_path_buf()
}

/// Rust source with `\`+newline continuations rejoined, so a marker split across two physical lines
/// is one string again.
///
/// This is the repair `req/324` §9-1 filed and `req/332` §5 found again in the twenty-sixth audit's
/// own instrument. A scan without it undercounts silently, which is the worst way for a census to
/// be wrong.
fn rejoined(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    // Set by a line that ended in `\`: the next line's indentation is not part of the literal.
    let mut continuing = false;
    for line in text.lines() {
        let piece = if continuing { line.trim_start() } else { line };
        let trimmed = piece.trim_end();
        if let Some(without_backslash) = trimmed.strip_suffix('\\') {
            out.push_str(without_backslash);
            continuing = true;
        } else {
            out.push_str(piece);
            out.push('\n');
            continuing = false;
        }
    }
    out
}

/// Every `.rs` file under `crates/*/src`.
fn workspace_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut crate_dirs: Vec<PathBuf> = std::fs::read_dir(crates_dir())
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

/// How many times each spelling occurs across `crates/*/src`, counted on rejoined text.
fn census() -> (usize, usize, Vec<String>) {
    let (mut kept, mut retired) = (0usize, 0usize);
    let mut offenders = Vec::new();
    for path in workspace_sources() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let joined = rejoined(&text);
        kept += joined.matches(MARKER).count();
        let here = joined.matches(RETIRED).count();
        if here > 0 {
            retired += here;
            offenders.push(format!(
                "{}:{here}",
                path.strip_prefix(crates_dir())
                    .unwrap_or(&path)
                    .to_string_lossy()
            ));
        }
    }
    (kept, retired, offenders)
}

// ---------------------------------------------------------------------------
// a — the census can see a marker a line-oriented scan cannot
// ---------------------------------------------------------------------------

/// 🔴 The bed, and the reason the audit's own numbers were one short on each side.
#[test]
fn a_bed_control_the_census_rejoins_continuations_before_it_counts() {
    let split =
        "pub const X: &str = \"words and then. What \\\n             to fix: do the thing\";\n";
    let naive = split.matches(MARKER).count();
    let joined = rejoined(split).matches(MARKER).count();
    record(&format!(
        "R28_MARKER_REJOIN naive={naive} rejoined={joined}"
    ));
    assert_eq!(
        naive, 0,
        "🔴 a line-oriented scan has to miss this one, or the arm is not measuring the defect"
    );
    assert_eq!(
        joined, 1,
        "🔴 `req/324` §9-1's class, for the third time: a marker split across a `\\`+newline is \
         still a marker, and a census that cannot see it reports a clean sweep over a source that \
         still carries the retired spelling"
    );
}

// ---------------------------------------------------------------------------
// b — the ruling, mechanised
// ---------------------------------------------------------------------------

/// 🔴 **`req/334` L-01** — one spelling, workspace-wide.
///
/// Workspace-wide **on purpose**: the parity gate's one-directory scan is what let a second spelling
/// grow in three crates without anything noticing. This census is the widening `req/329` L-02 asked
/// for, made safe by settling the vocabulary first — which was the whole point of ruling before
/// widening rather than after.
#[test]
fn b_the_retired_spelling_is_gone_from_every_crate() {
    let (kept, retired, offenders) = census();
    record(&format!(
        "R28_MARKER_CENSUS kept={kept} retired={retired} offenders={offenders:?}"
    ));
    assert!(
        kept > 30,
        "🔴 the census found almost no markers at all, which means it is broken rather than the \
         source being clean: kept={kept}"
    );
    assert_eq!(
        retired, 0,
        "🔴 `req/334` L-01: the retired spelling `{RETIRED}` is still in {offenders:?}. Two \
         spellings mean the parity gate either misses real remedies or has to know both, and the \
         sentences themselves showed the distinction is not a rule anyone was following: the same \
         receipt-archive failure carries one marker in gx-engine and the other in gx-api"
    );
}

// ---------------------------------------------------------------------------
// c — the marker constant does not drift into a fourth copy
// ---------------------------------------------------------------------------

/// 🔴 Three suites declare `REMEDY_MARKER` independently. They have to declare the **same** value,
/// or the gate that widened is checking a different word from the gate that did not.
#[test]
fn c_every_gate_that_declares_the_marker_declares_the_same_one() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut declared: Vec<(String, String)> = Vec::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&tests)
        .expect("gx-cli/tests is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for line in text.lines() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("const REMEDY_MARKER: &str = \"") else {
                continue;
            };
            let Some(value) = rest.split('"').next() else {
                continue;
            };
            declared.push((
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                value.to_string(),
            ));
        }
    }
    let values: Vec<&str> = declared.iter().map(|(_, v)| v.as_str()).collect();
    record(&format!("R28_MARKER_DECLARATIONS {declared:?}"));
    assert!(
        declared.len() >= 2,
        "🔴 fewer declarations than this arm was written against; re-read before trusting: \
         {declared:?}"
    );
    assert!(
        values.iter().all(|v| *v == MARKER),
        "🔴 the gates do not agree on the marker they are gating: {declared:?}"
    );
}
