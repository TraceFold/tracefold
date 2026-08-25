// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1b / AC-12 / F-1** (`req/544` §5, R-3h) — the coverage projection is a **total function**,
//! measured over every receipt document in this tree rather than over a handful somebody chose.
//!
//! # Why the denominator is the subject and not the numerator
//!
//! `req/544` F-1's kill condition is "some column of the table cannot be derived from the fifteen
//! members". A probe that ran the projection over three hand-picked payloads would answer that
//! question about three payloads. The lane's own memory of this failure shape is explicit: a
//! judgement taken on twelve hand-picked rows was reversed the same day when forty-five were
//! opened. So the enumeration here is a **walk of the tree**, its denominators are printed, and
//! [`a_hand_picked_denominator_is_refused`] is the control that makes the walk load-bearing.
//!
//! # The three denominators, and what each one can and cannot say
//!
//! | printed | what it counts | what it cannot say |
//! |---|---|---|
//! | `COVERAGE_JSON_SCANNED` | every `*.json` file in the working tree | nothing about receipts that are not files |
//! | `COVERAGE_DOCUMENTS` | those that decode as a `Receipt` | nothing about payloads inside `.rs` fixtures |
//! | `COVERAGE_TOTAL_OVER` | those whose **payload** decodes, and so have a table | — |
//!
//! 🔴 The gap between the last two is a real finding and is printed as `COVERAGE_UNDECODABLE`, with
//! the file named. The 2026-08-18 frozen specimen does not decode in this binary (`docs/LIMITS.md`
//! declares it; `frozen_receipt_corpus.rs` shows on every run that the limit is real), so a
//! *document* exists in this tree for which no coverage table can be produced. That is a limit of
//! the decoder and not of the projection — [`ReceiptCoverage::of`] is total over every
//! `ReceiptPayload` — but reporting `COVERAGE_TOTAL_OVER` without the gap beside it would be
//! reporting a numerator as a denominator.
//!
//! # The second half of AC-12: nothing outside the receipt is read
//!
//! [`the_projection_reads_nothing_outside_the_receipt`] is a census over the module's own source:
//! the projection takes a `&ReceiptPayload` and the file names no filesystem, no environment, no
//! clock and no path. This is the structural half of F-1 — the runtime half is that every arm of
//! every column below is reached and answers.

mod support;

use std::path::{Path, PathBuf};

use gx_witness::coverage::{Question, ReceiptAnswer, ReceiptCoverage, Unknown};
use gx_witness::receipt::{ReadSet, ReceiptKind};
use gx_witness::{Receipt, ReceiptPayload};

/// The workspace root, from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the workspace root resolves")
}

/// Directories a walk of a working tree has no business descending into.
const SKIPPED: [&str; 5] = [".git", "target", "node_modules", ".gx", ".sg"];

/// Every `*.json` file in the tree, sorted. No filter on name or directory beyond [`SKIPPED`]:
/// a walk that knew where receipts live would be a hand-picked list with extra steps.
fn every_json(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !SKIPPED.contains(&name.as_str()) {
                every_json(&path, out);
            }
        } else if name.ends_with(".json") {
            out.push(path);
        }
    }
}

/// A receipt document found on disk, and whether its payload decodes in this binary.
struct Found {
    path: PathBuf,
    payload: Option<ReceiptPayload>,
}

/// Walk the tree and decode. The return value carries the scan's own denominator so that no caller
/// can report the numerator alone.
fn receipts_in_the_tree() -> (usize, Vec<Found>) {
    let mut json = Vec::new();
    every_json(&workspace_root(), &mut json);
    json.sort();
    let scanned = json.len();
    let found = json
        .into_iter()
        .filter_map(|path| {
            let raw = std::fs::read(&path).ok()?;
            let receipt: Receipt = serde_json::from_slice(&raw).ok()?;
            // A `Receipt` deserialises from anything carrying an envelope of the right shape, so
            // the payload type is what separates a receipt from a checkpoint or a catalogue.
            if receipt.envelope.payload_type != gx_witness::RECEIPT_PAYLOAD_TYPE {
                return None;
            }
            let payload = receipt.payload().ok();
            Some(Found { path, payload })
        })
        .collect();
    (scanned, found)
}

/// 🔴 The predicate both arms of the denominator probes run: is this list the whole of what the
/// walk found?
///
/// Written as a function over two lists rather than as an assertion inside the walk, so that the
/// control can hand it a hand-picked subset and get the same question asked.
fn denominator_is_whole(offered: &[PathBuf], walked: &[PathBuf]) -> Result<(), String> {
    let missing: Vec<&PathBuf> = walked.iter().filter(|p| !offered.contains(p)).collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the denominator dropped {} of {} documents the walk found: {:?}",
        missing.len(),
        walked.len(),
        missing
            .iter()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
            .collect::<Vec<_>>()
    ))
}

/// 🔴 **AC-12** — every receipt document in the tree, and a coverage table for every one whose
/// payload this binary can decode.
#[test]
fn every_receipt_document_in_this_tree_gets_a_coverage_table() {
    let (scanned, found) = receipts_in_the_tree();
    let undecodable: Vec<&Found> = found.iter().filter(|f| f.payload.is_none()).collect();

    println!("COVERAGE_JSON_SCANNED={scanned}");
    println!("COVERAGE_DOCUMENTS={}", found.len());
    println!("COVERAGE_TOTAL_OVER={}", found.len() - undecodable.len());
    println!(
        "COVERAGE_UNDECODABLE={} {:?}",
        undecodable.len(),
        undecodable
            .iter()
            .map(|f| f.path.file_name().unwrap_or_default().to_string_lossy())
            .collect::<Vec<_>>()
    );

    assert!(
        !found.is_empty(),
        "the walk found no receipt documents at all, which means it is measuring the walk and not \
         the projection"
    );

    for f in &found {
        let Some(payload) = &f.payload else {
            // Named above, and not silently skipped: `docs/LIMITS.md` owns this one.
            continue;
        };
        let coverage = ReceiptCoverage::of(payload);
        assert_eq!(
            coverage.rows.len(),
            4,
            "a coverage table is four rows whatever the receipt is: {}",
            f.path.display()
        );
        let asked: Vec<Question> = coverage.rows.iter().map(|(q, _)| *q).collect();
        assert_eq!(
            asked,
            Question::ALL.to_vec(),
            "and the same four in the same order: {}",
            f.path.display()
        );
        println!(
            "COVERAGE_ROW file={} kind={:?} unmet={}",
            f.path.file_name().unwrap_or_default().to_string_lossy(),
            coverage.kind,
            coverage.unmet().len()
        );
    }
}

/// 🔴 **AC-12's negative control** — a hand-picked denominator is refused.
///
/// `req/544` AC-12 asks for this by name. Without it, [`every_receipt_document_in_this_tree_gets_a_coverage_table`]
/// is satisfied by a later edit that quietly narrows the walk to the one fixture that still works.
#[test]
fn a_hand_picked_denominator_is_refused() {
    let (_, found) = receipts_in_the_tree();
    let walked: Vec<PathBuf> = found.iter().map(|f| f.path.clone()).collect();

    denominator_is_whole(&walked, &walked).expect("the walk is whole against itself");

    let hand_picked: Vec<PathBuf> = walked.iter().take(1).cloned().collect();
    let refused = denominator_is_whole(&hand_picked, &walked);
    println!(
        "AC12_CONTROL_HAND_PICKED n={} of {}",
        hand_picked.len(),
        walked.len()
    );
    assert!(
        refused.is_err(),
        "🔴 a denominator of one passed the whole-denominator predicate, so the predicate is not \
         measuring the denominator"
    );
    println!("AC12_CONTROL_REFUSAL={}", refused.unwrap_err());
}

/// 🔴 **AC-12, the arms** — every spelling of every column is reached and answers.
///
/// The walk above measures the documents that exist; this measures the **projection**, over an
/// exhaustive match. A seventh `ReadSet` spelling stops this file compiling, which is R-3f's
/// requirement applied to the probe rather than only to the implementation.
#[test]
fn every_spelling_of_every_column_is_answered() {
    let key = support::keypair(12);
    let verdict = support::verdict_payload(gx_core::VerdictKind::Admit, &key, 12);
    // The proof is built here rather than taken from a shared helper: this suite adds no line to a
    // file another lane owns (`req/544` §9-2).
    let proof = gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    };
    let commit = support::commit_payload(&key, 12, proof);

    // The two kinds, exhaustively. A third stops the build here.
    for kind in [ReceiptKind::VerdictReceipt, ReceiptKind::CommitReceipt] {
        let payload = match kind {
            ReceiptKind::VerdictReceipt => verdict.clone(),
            ReceiptKind::CommitReceipt => commit.clone(),
        };
        let coverage = ReceiptCoverage::of(&payload);
        println!(
            "COVERAGE_KIND {:?} rows={:?}",
            kind,
            coverage
                .rows
                .iter()
                .map(|(q, a)| (
                    q.asked(),
                    match a {
                        ReceiptAnswer::Measured(m) => format!("measured({})", m.reading),
                        ReceiptAnswer::Unknown { why } => format!("unknown({})", why.because()),
                    }
                ))
                .collect::<Vec<_>>()
        );
    }

    // Every spelling of the read column, exhaustively over `ReadSet` plus the `Option::None` that
    // `req/510` made the fourth spelling of an absent read-set.
    let spellings: Vec<(&str, Option<ReadSet>)> = vec![
        ("none", None),
        (
            "per_read",
            Some(
                ReadSet::from_reads(vec![gx_core::ReadEntry {
                    digest: support::cid(1),
                    locator: "fixture://one".to_string(),
                }])
                .expect("canonical"),
            ),
        ),
        (
            "per_effect_root",
            Some(
                ReadSet::from_reads(
                    (0..8)
                        .map(|i| gx_core::ReadEntry {
                            digest: support::cid(100 + i),
                            locator: format!("fixture://{i}"),
                        })
                        .collect(),
                )
                .expect("canonical"),
            ),
        ),
        ("nothing", Some(ReadSet::Nothing)),
        ("no_escrow_record", Some(ReadSet::NoEscrowRecord)),
        ("reads_not_journalled", Some(ReadSet::ReadsNotJournalled)),
    ];
    let mut measured = 0;
    let mut unknown = 0;
    for (word, read_set) in spellings {
        let payload = ReceiptPayload {
            read_set,
            ..commit.clone()
        };
        let coverage = ReceiptCoverage::of(&payload);
        let (_, answer) = coverage
            .rows
            .iter()
            .find(|(q, _)| *q == Question::WhatWasRead)
            .expect("the read column is always there");
        match answer {
            ReceiptAnswer::Measured(m) => {
                measured += 1;
                println!("COVERAGE_READ {word} => measured: {}", m.reading);
            }
            ReceiptAnswer::Unknown { why } => {
                unknown += 1;
                println!("COVERAGE_READ {word} => unknown: {why:?}");
            }
        }
    }
    println!("COVERAGE_READ_SPELLINGS measured={measured} unknown={unknown}");
    assert_eq!(
        (measured, unknown),
        (3, 3),
        "🔴 `req/544` R-3b: three of the six spellings are measurements — G3, G4 and `Nothing` — \
         and three are absences. Folding `Nothing` in with the absences is the collapse `req/510` \
         undid"
    );

    // The fourth column, whatever the payload.
    for payload in [&verdict, &commit] {
        let coverage = ReceiptCoverage::of(payload);
        let (_, answer) = coverage
            .rows
            .iter()
            .find(|(q, _)| *q == Question::ByWhoseAuthority)
            .expect("the authority column is always there");
        assert_eq!(
            answer,
            &ReceiptAnswer::Unknown {
                why: Unknown::ActorNotInReceipt
            },
            "🔴 no receipt this binary can build answers `by whose authority`, and the table says \
             so rather than reaching for `key_id`"
        );
    }
}

/// 🔴 **F-1's structural half** — the projection reads nothing that is not the receipt.
///
/// A census over the module's own source, in the shape `ac_057.rs`'s dependency-closure probe uses:
/// the answer is a count with its denominator, and the control shows the count can go up.
#[test]
fn the_projection_reads_nothing_outside_the_receipt() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("coverage.rs"),
    )
    .expect("the module is here");

    /// The roads by which a projection could reach outside the fifteen members it is given.
    const OUTSIDE: [&str; 7] = [
        "std::fs",
        "std::env",
        "std::time",
        "SystemTime",
        "File::open",
        "PathBuf",
        "reqwest",
    ];

    // Code only: the module's prose names `docs/LIMITS.md` and `.gx/.gitignore`, and a census that
    // counted words in comments would be measuring the documentation.
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let count = |haystack: &str| -> Vec<&'static str> {
        OUTSIDE
            .iter()
            .filter(|needle| haystack.contains(**needle))
            .copied()
            .collect()
    };

    let sites = count(&code);
    println!(
        "COVERAGE_OUTSIDE_RECEIPT_SITES={} of {} roads, over {} code lines",
        sites.len(),
        OUTSIDE.len(),
        code.lines().count()
    );
    assert!(
        sites.is_empty(),
        "🔴 F-1: the projection reaches outside the receipt through {sites:?}. `req/544` §9-4 step \
         3 says the lane stops here and the ruling goes back to Fable rather than falling through \
         to design (A)"
    );

    // The control: the census can go up. Without it, a predicate that never matches anything
    // passes the assertion above for the wrong reason.
    let injected = format!("{code}\nfn leak() {{ let _ = std::fs::read(\"x\"); }}\n");
    let with_leak = count(&injected);
    println!("F1_CONTROL_INJECTED={with_leak:?}");
    assert_eq!(
        with_leak.len(),
        1,
        "the census counts a road that is really there, so its zero above is a measurement"
    );
}
