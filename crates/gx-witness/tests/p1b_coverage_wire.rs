// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1b / AC-4 and AC-5 (b)** (`req/544` §5) — `unknown` is a **value** on the wire, and no
//! road turns a declaration into a measurement.
//!
//! # Why the bytes and not the type
//!
//! `req/510` §3-1 measured the failure this probe exists to keep closed: four different absences of
//! a read-set were four `Option::None`s, and canonical DAG-CBOR encodes every one of them as the
//! single byte `0xf6`. Distinct at the type level, identical on the wire — and the wire is what a
//! third party reads. So the question here is not "are the variants different", it is "does an
//! encoder that a verifier runs produce different bytes for them", and the answer is a count of
//! pairwise-distinct encodings in `req/510`'s own `DR4634_PAIRWISE_DISTINCT` shape.
//!
//! # The control is the omission implementation
//!
//! `req/544` AC-4's negative control is spelled out: an implementation that says `unknown` by
//! **dropping the key** makes "this question was not answered" and "this question was not asked"
//! the same bytes. [`the_omission_spelling_collapses_two_facts`] builds that implementation and
//! measures the collapse, so the count above is a measurement rather than a hope.

use gx_canon::cbor;
use gx_witness::coverage::{Answer, Declared, Measured, Question, ReceiptAnswer, Unknown};

/// Every reading the read column can produce, in the three measured spellings `req/544` R-3b names.
fn read_column_measured() -> Vec<(&'static str, ReceiptAnswer)> {
    let from: &[&str] = &["read_set"];
    let not_covered = "the agent's own read traffic";
    vec![
        (
            "g3",
            ReceiptAnswer::Measured(Measured {
                from,
                reading: "G3: one entry per distinct object".to_string(),
                not_covered,
            }),
        ),
        (
            "g4",
            ReceiptAnswer::Measured(Measured {
                from,
                reading: "G4: a root over the entries".to_string(),
                not_covered,
            }),
        ),
        (
            "nothing",
            ReceiptAnswer::Measured(Measured {
                from,
                reading: "the escrow ran and read nothing".to_string(),
                not_covered,
            }),
        ),
    ]
}

/// The three absences the read column can carry — `req/510`'s vocabulary, not a second one.
fn read_column_unknown() -> Vec<(&'static str, ReceiptAnswer)> {
    vec![
        (
            "no_escrow_record",
            ReceiptAnswer::Unknown {
                why: Unknown::NoEscrowRecord,
            },
        ),
        (
            "reads_not_journalled",
            ReceiptAnswer::Unknown {
                why: Unknown::ReadsNotJournalled,
            },
        ),
        (
            "not_yet_asked",
            ReceiptAnswer::Unknown {
                why: Unknown::NotYetAsked,
            },
        ),
    ]
}

/// How many of the pairs in `values` encode to different bytes, and how many pairs there were.
fn pairwise_distinct(values: &[(&str, Vec<u8>)]) -> (usize, usize) {
    let mut distinct = 0;
    let mut pairs = 0;
    for i in 0..values.len() {
        for j in (i + 1)..values.len() {
            pairs += 1;
            if values[i].1 != values[j].1 {
                distinct += 1;
            } else {
                println!(
                    "COVERAGE_COLLAPSE {} and {} encode identically",
                    values[i].0, values[j].0
                );
            }
        }
    }
    (distinct, pairs)
}

/// 🔴 **AC-4** — every spelling of every column is a different document.
#[test]
fn every_value_of_every_question_is_distinct_in_canonical_bytes() {
    // The read column: six values, and `req/544` AC-4 asks for the columns to be counted
    // separately because they do not all carry the same number.
    let read: Vec<(&str, Vec<u8>)> = read_column_measured()
        .into_iter()
        .chain(read_column_unknown())
        .map(|(word, answer)| {
            (
                word,
                cbor::encode(&Answer::from(answer)).expect("an answer has a canonical form"),
            )
        })
        .collect();
    let (distinct, pairs) = pairwise_distinct(&read);
    println!("COVERAGE_PAIRWISE_DISTINCT_read={distinct}/{pairs}");
    assert_eq!(
        (distinct, pairs),
        (15, 15),
        "🔴 six spellings make fifteen pairs and every one of them has to be two documents"
    );

    // The three values of `req/535` R-3, on one question. This is the row AC-4 is named for: a
    // measurement, a declaration and an absence are three, not two and a shrug.
    let three: Vec<(&str, Vec<u8>)> = vec![
        (
            "measured",
            cbor::encode(&Answer::Measured(Measured {
                from: &["postcondition_fingerprint"],
                reading: "a fingerprint before and after".to_string(),
                not_covered: "anything outside that scope",
            }))
            .expect("canonical"),
        ),
        (
            "declared",
            cbor::encode(&Answer::Declared(Declared {
                source: "the project's own declaration".to_string(),
                claim: "writes are observed".to_string(),
            }))
            .expect("canonical"),
        ),
        (
            "unknown",
            cbor::encode(&Answer::Unknown {
                why: Unknown::NoPostconditionObserved,
            })
            .expect("canonical"),
        ),
    ];
    let (distinct, pairs) = pairwise_distinct(&three);
    println!("COVERAGE_PAIRWISE_DISTINCT_three_values={distinct}/{pairs}");
    assert_eq!((distinct, pairs), (3, 3));

    // And the six ways of not knowing, which is the whole `Unknown` vocabulary. An exhaustive list
    // built from a match, so a seventh member cannot be added without being encoded here.
    let all_unknown = [
        Unknown::NoEscrowRecord,
        Unknown::ReadsNotJournalled,
        Unknown::NotYetAsked,
        Unknown::NoPostconditionObserved,
        Unknown::NoInclusionProof,
        Unknown::ActorNotInReceipt,
    ];
    for u in all_unknown {
        // The exhaustive match that makes the array above complete rather than remembered.
        let _: &'static str = match u {
            Unknown::NoEscrowRecord => "no_escrow_record",
            Unknown::ReadsNotJournalled => "reads_not_journalled",
            Unknown::NotYetAsked => "not_yet_asked",
            Unknown::NoPostconditionObserved => "no_postcondition_observed",
            Unknown::NoInclusionProof => "no_inclusion_proof",
            Unknown::ActorNotInReceipt => "actor_not_in_receipt",
        };
    }
    let absences: Vec<(&str, Vec<u8>)> = all_unknown
        .iter()
        .map(|u| {
            (
                match u {
                    Unknown::NoEscrowRecord => "no_escrow_record",
                    Unknown::ReadsNotJournalled => "reads_not_journalled",
                    Unknown::NotYetAsked => "not_yet_asked",
                    Unknown::NoPostconditionObserved => "no_postcondition_observed",
                    Unknown::NoInclusionProof => "no_inclusion_proof",
                    Unknown::ActorNotInReceipt => "actor_not_in_receipt",
                },
                cbor::encode(&Answer::Unknown { why: *u }).expect("canonical"),
            )
        })
        .collect();
    let (distinct, pairs) = pairwise_distinct(&absences);
    println!("COVERAGE_PAIRWISE_DISTINCT_absences={distinct}/{pairs}");
    assert_eq!((distinct, pairs), (15, 15));

    // Four questions, four keys: the same answer under two questions is two documents, or a table
    // could lose a row and encode the same.
    let questions: Vec<(&str, Vec<u8>)> = Question::ALL
        .iter()
        .map(|q| {
            (
                q.asked(),
                cbor::encode(&(
                    q,
                    &Answer::Unknown {
                        why: Unknown::ActorNotInReceipt,
                    },
                ))
                .expect("canonical"),
            )
        })
        .collect();
    let (distinct, pairs) = pairwise_distinct(&questions);
    println!("COVERAGE_PAIRWISE_DISTINCT_questions={distinct}/{pairs}");
    assert_eq!((distinct, pairs), (6, 6));
}

/// 🔴 **AC-4's negative control** — the omission spelling, built and measured.
///
/// This is the implementation `req/544` AC-4 says must be red: `unknown` said by leaving the key
/// out. Two facts that the value spelling keeps apart — *this question was asked and not answered*
/// and *this question was not asked* — become one document, and the count above is what refuses it.
#[test]
fn the_omission_spelling_collapses_two_facts() {
    use serde_json::json;

    // The value spelling: the key is there and carries which absence this is.
    let answered_unknown = cbor::encode(&json!({
        "question": "what_was_read",
        "answer": {"value": "unknown", "why": "not_yet_asked"},
    }))
    .expect("canonical");

    // The omission spelling: `unknown` is said by dropping the key…
    let omitted = cbor::encode(&json!({ "question": "what_was_read" })).expect("canonical");
    // …and a question nobody asked drops it too.
    let never_asked = cbor::encode(&json!({ "question": "what_was_read" })).expect("canonical");

    println!(
        "AC4_CONTROL value_bytes={} omission_bytes={} never_asked_bytes={}",
        answered_unknown.len(),
        omitted.len(),
        never_asked.len()
    );
    assert_ne!(
        answered_unknown, omitted,
        "the two spellings are different documents, which is the premise of the control"
    );
    assert_eq!(
        omitted, never_asked,
        "🔴 AC-4's control: under the omission spelling, 'asked and not answered' and 'not asked' \
         are the same bytes. That is why `unknown` is a value here and not a missing key — the \
         same collapse `req/510` §3-1 measured one layer down"
    );
}

/// 🔴 **AC-5 (b)** — the structural half: no road exists from a declaration to a measurement.
///
/// A census over the source of the two modules that hold the vocabulary. What is counted is not
/// "did somebody write a conversion", it is whether the **shapes that would let one be written**
/// are there: a `From<Declared>` impl, a `Deserialize` on `Measured`, or a constructor of
/// `Measured` outside the receipt projection.
#[test]
fn no_road_turns_a_declaration_into_a_measurement() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("coverage.rs"),
    )
    .expect("the module is here");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // The three shapes, as a predicate a control can also be run through.
    let promotions = |text: &str| -> Vec<&'static str> {
        let mut found = Vec::new();
        if text.contains("impl From<Declared> for Measured") {
            found.push("From<Declared> for Measured");
        }
        if text.contains("impl From<DeclaredAnswer> for ReceiptAnswer") {
            found.push("From<DeclaredAnswer> for ReceiptAnswer");
        }
        // The derive line that would let a `Measured` be read out of a file.
        for line in text.lines() {
            if line.contains("Deserialize") && line.contains("derive") {
                // Which type does this derive sit on? The next non-empty line names it.
                let index = text.find(line).unwrap_or(0);
                let after = &text[index + line.len()..];
                let next = after
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or_default();
                if next.contains("struct Measured")
                    || next.contains("enum ReceiptAnswer")
                    || next.contains("enum Answer ")
                {
                    found.push("Deserialize on a measured-carrying type");
                }
            }
        }
        found
    };

    let sites = promotions(&code);
    println!("COVERAGE_PROMOTION_SITES={} {:?}", sites.len(), sites);
    assert!(
        sites.is_empty(),
        "🔴 AC-5 (b): a road from a declaration to a measurement exists ({sites:?}). `req/544` §4 \
         kills KA-3 by shape and not by rule — the road is supposed to be absent, not guarded"
    );

    // The control: one line added, and the census sees it.
    let injected = format!("{code}\nimpl From<Declared> for Measured {{}}\n");
    let with_promotion = promotions(&injected);
    println!("AC5B_CONTROL_INJECTED={with_promotion:?}");
    assert_eq!(
        with_promotion.len(),
        1,
        "the census finds a promotion that is really there, so its zero above is a measurement"
    );
}
