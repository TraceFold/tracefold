// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1b** (`req/544` §2-1, §3 R-3d/R-3e, ruled in `req/38` §313) — the **face** level of a
//! coverage declaration: what a route could observe, said in words that are not a receipt's.
//!
//! # Two levels, and why they may never share a vocabulary
//!
//! `req/544` §2-1 splits the declaration in two, and the split is the reason this module exists as
//! something other than a copy of [`gx_witness::coverage`]:
//!
//! | level | what it is a statement about | when it can be made | it can be |
//! |---|---|---|---|
//! | **face** (here) | this route, and what it could observe in principle | the moment a project is attached, with no receipt in existence | **wrong** |
//! | **receipt** ([`gx_witness::coverage`]) | one document, and what it actually answers | only when a receipt exists | derived |
//!
//! A face that claims it can observe reads, and a receipt from that face whose read column answers
//! `unknown` because nothing was read on that run, are **both correct**. So the two are spelled in
//! different vocabularies — [`gx_witness::FacePosture`] against [`gx_witness::ReceiptAnswer`] — and
//! there is no conversion between them anywhere in this workspace. A reader cannot mistake a claim
//! for an observation because the words do not overlap, and an implementation cannot print the
//! first where the second belongs without the probes seeing a word that does not belong there.
//!
//! # 🔴 The route half is **not written by a person** (R-3e)
//!
//! [`posture_from_route`] takes [`gx_mcp_wire::config::Report`] — what `gx wrap --check-config`
//! already computes off an agent's own configuration file — and nothing else. The passing state it
//! reads is the one B-1 already defined and defended: *routed through gx **and** no entry starts
//! the server directly*. A second definition of "the route is in place" would be a second opinion
//! about a question that already has an answer in this binary.
//!
//! `req/538` §3-1 measured that this lives in `gx-mcp-wire/src/config.rs` and **not** in
//! `crates/gx-cli/src/wrap.rs`, which holds the proxy and rewrites no configuration. `req/535` §9
//! pointed the next lane at `wrap.rs`; that is the file this module deliberately does not read.
//!
//! # 🔴 And the declared half is the **only** place a person can write
//!
//! [`Declared`] values arrive from a side-car file the operator or the project being attached
//! supplies. They can say anything. What they cannot do is become a measurement:
//! [`gx_witness::Measured`] has no `Deserialize` and no constructor outside the receipt projection,
//! so there is no road from this file into the measured column — not a rule that is checked, a road
//! that does not exist.
//!
//! # What is deliberately absent from the side-car
//!
//! **Any measured value.** `req/38` §313 ruling 2 fixes design (C): the receipt is the only address
//! at which a measurement lives. A face file carrying one would be a second field that could
//! disagree with the receipt, which is the state (C) was chosen to make unconstructible.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use gx_witness::coverage::{DeclaredAnswer, FacePosture, Question, Unknown};

use crate::{io, Error, Result};

/// The directory a face declaration is written to, under `.gx/`.
///
/// Not a member of `GX_PATHS`: the eleven declared paths are what `Layout::create` places on every
/// project, and a face file exists only where somebody offered a route or a declaration. Adding a
/// twelfth row would move `req/538`'s AC-1 denominator for every project in the world in order to
/// describe a file most of them do not have.
pub const FACES_DIR: &str = "faces";

/// A face's coverage declaration: what this route claims, and what somebody wrote down.
#[derive(Debug, Clone)]
pub struct FaceDeclaration {
    /// Which face this is about. The name of the agent's server entry when a route was offered,
    /// and `unrouted` when none was — never a generated identifier, so that two runs over the same
    /// configuration answer with the same name.
    pub face: String,
    /// 🔴 What `gx wrap --check-config` sees in the agent's configuration, carried **verbatim**.
    /// `None` when no configuration was offered, which is itself a fact about the run.
    pub route: Option<Value>,
    /// The capability claim, one word per question, derived from `route`.
    pub posture: Vec<(Question, FacePosture)>,
    /// What somebody wrote down, one entry per question they wrote about.
    pub declared: BTreeMap<Question, DeclaredAnswer>,
}

/// 🔴 **R-3e** — the capability claim, derived from the route and from nothing else.
///
/// # The three readings, and the one that is not a shortcut
///
/// * **No configuration offered** — nothing routes effects at gx through this operation, so the
///   face observes nothing. `gx attach` itself is in this state by construction: its own answer
///   says it points no route.
/// * **Routed, and the direct road is gone** — effects reach the membrane, so a receipt issued
///   through it can carry an answer for the first three questions.
/// * **Routed, and some entry still starts the server directly** — `CannotMeasure`, and the route
///   JSON beside it names the entries. A face that claimed to observe changes while a second entry
///   ran the same server outside it would be claiming a property of a road that has a way around it.
///
/// The fourth question is never `CanMeasure` here. The actor reaches gx as a flag on the wrapped
/// entry (`--actor-key` / `--actor-model`), which is somebody writing a name down, so the strongest
/// honest word for it is [`FacePosture::OnlyDeclared`].
/// 🔴 **`cfg(feature = "mcp")`** (`req/817`): the argument is `gx_mcp_wire`'s route report, and
/// `gx-mcp-wire` is one of the four crates `req/789` §3 holds private, so the public distribution
/// builds without this function. Nothing else in this module depends on the wire — the postures and
/// the four questions above are the whole face and they stay.
#[cfg(feature = "mcp")]
#[must_use]
pub fn posture_from_route(
    route: Option<&gx_mcp_wire::config::Report>,
) -> Vec<(Question, FacePosture)> {
    let observes = route.is_some_and(|r| r.wrapped && r.direct.is_empty());
    let actor_named = route.is_some_and(|r| r.wrapped);
    Question::ALL
        .iter()
        .map(|question| {
            let posture = match question {
                Question::WhatWasRead | Question::WhatWasWritten | Question::When => {
                    if observes {
                        FacePosture::CanMeasure
                    } else {
                        FacePosture::CannotMeasure
                    }
                }
                Question::ByWhoseAuthority => {
                    if actor_named {
                        FacePosture::OnlyDeclared
                    } else {
                        FacePosture::CannotMeasure
                    }
                }
            };
            (*question, posture)
        })
        .collect()
}

/// The word a posture prints as. An exhaustive match, and **none of these words is `measured`**.
#[must_use]
pub const fn posture_word(posture: FacePosture) -> &'static str {
    match posture {
        FacePosture::CanMeasure => "can-measure",
        FacePosture::CannotMeasure => "cannot-measure",
        FacePosture::OnlyDeclared => "only-declared",
    }
}

/// The question, as the key a JSON object uses.
#[must_use]
pub const fn question_key(question: Question) -> &'static str {
    match question {
        Question::WhatWasRead => "what_was_read",
        Question::WhatWasWritten => "what_was_written",
        Question::When => "when",
        Question::ByWhoseAuthority => "by_whose_authority",
    }
}

/// The question a key names, or `None` for a key naming no question.
#[must_use]
pub fn question_of_key(key: &str) -> Option<Question> {
    Question::ALL
        .iter()
        .copied()
        .find(|question| question_key(*question) == key)
}

/// 🔴 Read the declarations somebody wrote, and refuse anything that is not one.
///
/// # The refusal that matters
///
/// A file offering `"value": "measured"` is refused **by name**, with the reason, rather than
/// ignored. Silently dropping it would leave an operator believing a measurement had been recorded;
/// refusing it says the thing that is true, which is that this file is not an address a measurement
/// can have.
///
/// # Errors
/// [`Error::Malformed`] for a file that is not an object of questions, names a question this face
/// does not ask, or offers a measurement.
pub fn read_declared(path: &Path) -> Result<BTreeMap<Question, DeclaredAnswer>> {
    let raw = std::fs::read(path).map_err(io("read", path))?;
    let document: Value = serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
        what: "coverage declaration",
        path: path.display().to_string(),
        detail: detail.to_string(),
    })?;
    let Some(object) = document.as_object() else {
        return Err(Error::Malformed {
            what: "coverage declaration",
            path: path.display().to_string(),
            detail: "a declaration file is an object whose keys are the questions".to_string(),
        });
    };
    let mut out = BTreeMap::new();
    for (key, value) in object {
        let Some(question) = question_of_key(key) else {
            return Err(Error::Malformed {
                what: "coverage declaration",
                path: path.display().to_string(),
                detail: format!(
                    "`{key}` is not one of the four questions this face is judged on ({})",
                    Question::ALL
                        .iter()
                        .map(|q| question_key(*q))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        };
        // 🔴 The one refusal `req/544` §4 asks for by name. `DeclaredAnswer` has no `Measured` arm,
        // so serde would refuse this anyway with a message about a tagged union; the explicit check
        // is here so that the reason an operator reads is the reason that is true.
        if value.get("value").and_then(Value::as_str) == Some("measured") {
            return Err(Error::Malformed {
                what: "coverage declaration",
                path: path.display().to_string(),
                detail: format!(
                    "`{key}` offers a measured value. A declaration file is where somebody writes \
                     down what they believe; the only address a measurement has is the receipt, \
                     which is what makes the two impossible to disagree"
                ),
            });
        }
        let answer: DeclaredAnswer =
            serde_json::from_value(value.clone()).map_err(|detail| Error::Malformed {
                what: "coverage declaration",
                path: path.display().to_string(),
                detail: format!("`{key}`: {detail}"),
            })?;
        out.insert(question, answer);
    }
    Ok(out)
}

impl FaceDeclaration {
    /// The declaration, as the JSON `gx attach` prints and the side-car holds.
    ///
    /// 🔴 **R-3d** — nothing in here is a rendering decision. There is no badge, no summary and no
    /// colour: the same four rows come out whatever reads them, so a display layer cannot change
    /// what the declaration says by choosing how to say it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let posture: Vec<Value> = self
            .posture
            .iter()
            .map(|(question, posture)| {
                let declared = self.declared.get(question).map(|answer| match answer {
                    DeclaredAnswer::Declared(d) => json!({
                        "value": "declared",
                        "source": d.source,
                        "claim": d.claim,
                    }),
                    DeclaredAnswer::Unknown { why } => json!({
                        "value": "unknown",
                        "why": unknown_word(*why),
                        "because": why.because(),
                    }),
                });
                json!({
                    "question": question_key(*question),
                    "asked": question.asked(),
                    "posture": posture_word(*posture),
                    // 🔴 Present as a key on every row, with `null` where nobody wrote anything.
                    // `req/544` AC-4's shape one level up: an absent key and a stated absence are
                    // two different facts, and a row that dropped the key would spell them alike.
                    "declared": declared,
                })
            })
            .collect();
        json!({
            "face": self.face,
            "level": "face",
            // The sentence this table replaces, named so that a reader of `gx attach`'s answer can
            // see which of P-1a's three unanswered items is now answered and which two are not.
            "answers": "what this project can and cannot observe about a change",
            "route": self.route,
            "posture": posture,
            // 🔴 The bound on the whole table, in the answer rather than in a document elsewhere.
            "claim_not_observation": "every word above is a claim about this route, not a reading \
                                      of any change. What one receipt actually answered is a \
                                      different table with different words: `gx receipt coverage`.",
        })
    }

    /// Where this face's declaration is written under a project's `.gx/`.
    #[must_use]
    pub fn side_car_path(gx_dir: &Path, face: &str) -> PathBuf {
        gx_dir.join(FACES_DIR).join(format!("{face}.json"))
    }

    /// Write the side-car, creating `.gx/faces/` if it is not there.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory or the file cannot be written.
    pub fn write(&self, gx_dir: &Path) -> Result<PathBuf> {
        let path = Self::side_car_path(gx_dir, &self.face);
        let dir = path.parent().unwrap_or(gx_dir);
        std::fs::create_dir_all(dir).map_err(io("create", dir))?;
        let mut bytes =
            serde_json::to_vec_pretty(&self.to_json()).map_err(|detail| Error::Malformed {
                what: "coverage declaration",
                path: path.display().to_string(),
                detail: detail.to_string(),
            })?;
        bytes.push(b'\n');
        std::fs::write(&path, &bytes).map_err(io("write", &path))?;
        Ok(path)
    }

    /// 🔴 The questions this face does **not** claim to observe, derived from [`Self::posture`].
    ///
    /// `req/544` KA-4b: a list of known gaps written out by hand is a list that a change hiding a
    /// gap edits at the same time as the gap. This is a projection of the table above, so the only
    /// way to shorten it is to change the table, and the table is what the probes pin.
    #[must_use]
    pub fn unmet(&self) -> Vec<(Question, FacePosture)> {
        self.posture
            .iter()
            .filter(|(_, posture)| !matches!(posture, FacePosture::CanMeasure))
            .copied()
            .collect()
    }
}

/// 🔴 The receipt-level table, as JSON — the other half of `gx receipt coverage`'s answer.
///
/// It is written here rather than in gx-witness because the shape of a CLI answer is 44 §1.3's
/// question and not a witness crate's. What gx-witness owns is the projection; what this owns is
/// how it is printed.
#[must_use]
pub fn receipt_coverage_json(coverage: &gx_witness::ReceiptCoverage) -> Value {
    let rows: Vec<Value> = coverage
        .rows
        .iter()
        .map(|(question, answer)| {
            let body = match answer {
                gx_witness::ReceiptAnswer::Measured(m) => json!({
                    "value": "measured",
                    "from": m.from,
                    "reading": m.reading,
                    "not_covered": m.not_covered,
                }),
                gx_witness::ReceiptAnswer::Unknown { why } => json!({
                    "value": "unknown",
                    "why": unknown_word(*why),
                    "because": why.because(),
                }),
            };
            json!({
                "question": question_key(*question),
                "asked": question.asked(),
                "answer": body,
            })
        })
        .collect();
    json!({
        "level": "receipt",
        "receipt_kind": format!("{:?}", coverage.kind),
        "rows": rows,
        "unmet": coverage
            .unmet()
            .iter()
            .map(|(question, why)| json!({
                "question": question_key(*question),
                "why": unknown_word(*why),
            }))
            .collect::<Vec<_>>(),
    })
}

/// The word an absence prints as. Exhaustive: a seventh way of not knowing stops the build here as
/// well as in gx-witness, which is R-3f applied at the surface that a reader actually sees.
#[must_use]
pub const fn unknown_word(unknown: Unknown) -> &'static str {
    match unknown {
        Unknown::NoEscrowRecord => "no_escrow_record",
        Unknown::ReadsNotJournalled => "reads_not_journalled",
        Unknown::NotYetAsked => "not_yet_asked",
        Unknown::NoPostconditionObserved => "no_postcondition_observed",
        Unknown::NoInclusionProof => "no_inclusion_proof",
        Unknown::ActorNotInReceipt => "actor_not_in_receipt",
    }
}
