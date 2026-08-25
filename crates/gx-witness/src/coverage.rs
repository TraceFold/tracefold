// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1b / R-3** (`req/544`, ruled in `req/38` §313) — the four questions a face is judged on,
//! and which of them a receipt actually answers.
//!
//! # The question this module answers
//!
//! `req/405` §7: **adequacy is not "how much can be taken", it is "can what cannot be taken be
//! written down".** A receipt already carries fifteen members; what it did not carry was a place
//! where a reader is told, in one table, which of the four questions — what was read, what was
//! written, when, by whose authority — this document answers and which it does not.
//!
//! # 🔴 Design (C): the table is **derived and never stored**
//!
//! `req/38` §313 ruling 2 fixed the shape. There are three ways to carry a coverage table and two
//! of them are wrong:
//!
//! | | where the table lives | why it was not taken |
//! |---|---|---|
//! | (A) | a sixteenth member of [`ReceiptPayload`] | `ledger_digest` re-encodes the struct, so a new member moves **every historical leaf** (`req/519` §7-5, measured) |
//! | (B) | a side-car file beside the receipt | two fields can disagree; 42 §3.10's rule is that a discriminator lives *inside* the structure it discriminates |
//! | **(C)** | **a total projection of the fifteen members that are already there** | taken |
//!
//! The property that makes (C) worth the name: **a declaration that is not stored cannot disagree
//! with the receipt.** [`ReceiptCoverage`] has no independent degrees of freedom — it is a function
//! of the payload, so the "the table says measured and the receipt says null" state is not refused,
//! it is unconstructible. And because no member was added, receipts minted before this lane get a
//! coverage table too: the projection is over fields they already carry.
//!
//! # 🔴 The asymmetry that makes `measured` trustworthy (`req/544` §4)
//!
//! `req/535` KA-3 is that "it was taken and reported unknown" and "it was not taken and reported
//! measured" are indistinguishable after the fact. The kill is not a check, it is a **shape**:
//!
//! * [`Measured`] is constructed on exactly one road — [`ReceiptCoverage::of`], out of a
//!   `&ReceiptPayload`. There is no `From<Declared>`, and this module deliberately does not write
//!   one.
//! * [`Measured`], [`ReceiptAnswer`] and [`Answer`] do **not** derive `Deserialize`. A side-car
//!   file, a hint from the project being attached, or anything else that arrives as bytes from
//!   outside cannot land in a `Measured` — there is no road for it to travel.
//! * [`Declared`] does derive it, because a declaration is exactly the thing somebody else writes.
//!
//! This matters more here than it would elsewhere. P-1a measured that `gx attach` writes `*` into
//! `.gx/.gitignore`, so `git status` is **empty** after an attach (`req/538` §3-4): there is no
//! second place a reader can check the declaration against. Where nothing can be cross-checked
//! afterwards, the only defence left is that the untrue statement could not be written in the first
//! place.
//!
//! # What this module does not do
//!
//! It does not decide the **face**-level question. "What could this route observe in principle" is
//! a capability claim, it can be wrong, and `req/544` §2-1 keeps it in a separate vocabulary
//! ([`crate::coverage::FacePosture`]) precisely so that a claim can never be printed as an answer.
//! A face that claims it can measure reads, and a receipt from that face that answers `unknown`
//! because that run had nothing to attest, are **both correct at once**.

use serde::{Deserialize, Serialize};

use crate::receipt::{ReadSet, ReceiptKind, ReceiptPayload};

/// The four questions `req/405` §6 judges a face on.
///
/// An enum rather than four strings: `req/544` R-3f asks that a fifth question stop the build
/// rather than silently narrow the table, which is what an exhaustive match over this type does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Question {
    /// What did the transformation read?
    WhatWasRead,
    /// What did it write?
    WhatWasWritten,
    /// When did it happen — in time, or at least in order?
    When,
    /// By whose authority was it done?
    ByWhoseAuthority,
}

impl Question {
    /// All four, in the order a coverage table prints them.
    pub const ALL: [Question; 4] = [
        Question::WhatWasRead,
        Question::WhatWasWritten,
        Question::When,
        Question::ByWhoseAuthority,
    ];

    /// The question, as a sentence.
    #[must_use]
    pub const fn asked(self) -> &'static str {
        match self {
            Question::WhatWasRead => "what did this read",
            Question::WhatWasWritten => "what did this write",
            Question::When => "when did this happen",
            Question::ByWhoseAuthority => "by whose authority was this done",
        }
    }
}

/// 🔴 An answer that was **taken from the signed bytes**, and the bound on what it covers.
///
/// There is no constructor for this type outside [`ReceiptCoverage::of`], and it does not derive
/// `Deserialize`. Both facts are load-bearing rather than incidental — see this module's header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Measured {
    /// The payload members the answer was read off, by name. A reader who doubts the answer is
    /// told where to look rather than asked to trust the projection.
    pub from: &'static [&'static str],
    /// What those members say, in their own vocabulary.
    pub reading: String,
    /// 🔴 What this reading does **not** cover, said in the same breath as the reading.
    ///
    /// `req/405` §7's condition applied one level in: an answer that named only what it covers
    /// would be read as covering everything.
    pub not_covered: &'static str,
}

/// An answer somebody **wrote down**: a hint from the project being attached, never a measurement.
///
/// This is the one member of the vocabulary that derives `Deserialize`, because a declaration is by
/// definition a thing that arrives from outside.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declared {
    /// Who said it. Not a key and not an identity — the name of the file or the field the sentence
    /// came out of, so that a reader can go and look at it.
    pub source: String,
    /// What was claimed.
    pub claim: String,
}

/// 🔴 The vocabulary of **not knowing**, with a reason attached to every member.
///
/// # The three read-side members are `d34`'s and not new ones
///
/// `req/510` closed the collapse in which four different absences of a read-set were one `null`.
/// Re-spelling them here would rebuild it one layer up, so [`Unknown::NoEscrowRecord`],
/// [`Unknown::ReadsNotJournalled`] and [`Unknown::NotYetAsked`] are that vocabulary carried
/// through unchanged.
///
/// 🔴 And [`ReadSet::Nothing`] is deliberately **not** here: "the escrow ran and read nothing" is a
/// positive statement that decides the question for every locator in the universe
/// (`ReadSet::names` answers `Some(false)` to all of them), which is a *stronger* answer than G3
/// gives. Folding the four absences plus `Nothing` into one "cannot tell" is exactly the collapse
/// `req/510` spent a lane undoing.
// `Deserialize` is derived here and **not** on [`Measured`], and the difference is the whole of
// §4's asymmetry: a declaration may name an absence (it is saying it does not know, which needs no
// evidence), and may not name a measurement (which does).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unknown {
    /// A rebuild found no escrow record for this transformation: the journal no longer holds what
    /// was read. A damaged or trimmed journal, and `gx repair`'s question — not a change with no
    /// reads.
    NoEscrowRecord,
    /// The journal that produced this receipt predates the erratum that recorded reads at all.
    /// A statement about the journal, not about the change.
    ReadsNotJournalled,
    /// Nobody has asked yet. The escrow runs at 43 T-10b, inside commit, so a `VerdictReceipt`
    /// carries no read-set because at verdict time the question has not been put.
    NotYetAsked,
    /// Nothing was applied, so no postcondition was observed. The ordinary state of a
    /// `VerdictReceipt`.
    NoPostconditionObserved,
    /// The receipt carries no inclusion proof, so not even the order is fixed.
    NoInclusionProof,
    /// 🔴 The actor is **not in the receipt**, on any road.
    ///
    /// `key_id` is the signing key's id and not an actor; `receipt.rs` says so in its own field
    /// documentation, and the actor lives on `Transformation.actor`, which a receipt references by
    /// id and does not carry. This is `req/405` §2-1 fact ③ — "by whose authority" is answerable at
    /// the agent's chokepoint and nowhere else — showing up as an output row rather than as a
    /// sentence in a document.
    ActorNotInReceipt,
}

impl Unknown {
    /// The reason, spelled out. An exhaustive match: a seventh way of not knowing stops the build.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Unknown::NoEscrowRecord => {
                "a rebuild found no escrow record for this transformation, so this document holds \
                 nothing about what was read"
            }
            Unknown::ReadsNotJournalled => {
                "the journal this receipt was rebuilt from is older than the record of reads, so \
                 the absence is the journal's and not the change's"
            }
            Unknown::NotYetAsked => {
                "the escrow runs during commit, so at verdict time nobody has asked this question \
                 yet"
            }
            Unknown::NoPostconditionObserved => {
                "nothing was applied, so no postcondition fingerprint was taken"
            }
            Unknown::NoInclusionProof => {
                "no inclusion proof is carried, so this document does not even fix the order"
            }
            Unknown::ActorNotInReceipt => {
                "the actor is on the transformation, which this receipt names by id and does not \
                 carry; `key_id` is the id of the key that signed, which is a different question"
            }
        }
    }
}

/// 🔴 What **one receipt** answers to one question: a fact, or an honest absence.
///
/// There is no `Declared` arm. A receipt does not make claims about itself — it carries signed
/// bytes, and either they answer the question or they do not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "value", rename_all = "snake_case")]
pub enum ReceiptAnswer {
    /// Taken from the signed bytes.
    Measured(Measured),
    /// Not in the signed bytes, and here is which absence this is.
    Unknown {
        /// Which absence. Named `why` rather than carried bare so that the value survives a
        /// round-trip through an internally tagged encoding — measured, not assumed: a newtype
        /// variant holding a unit-variant enum does not decode back.
        why: Unknown,
    },
}

/// What a **declaration** says to one question: a hint somebody wrote, or an honest absence.
///
/// The mirror of [`ReceiptAnswer`], and the missing arm is the other one: a declaration cannot
/// measure anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "value", rename_all = "snake_case")]
pub enum DeclaredAnswer {
    /// Somebody wrote this down.
    Declared(Declared),
    /// Nobody wrote anything down, and here is which absence this is.
    Unknown {
        /// Which absence.
        why: Unknown,
    },
}

/// 🔴 The three values of `req/535` R-3, as one type — for printing and for encoding, never for
/// deciding.
///
/// The two halves above are what the rest of the system holds; this is where they meet, and the
/// only direction of travel is **into** it. There is no `Answer -> ReceiptAnswer`, and there is no
/// `Declared -> Measured` anywhere in this workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "value", rename_all = "snake_case")]
pub enum Answer {
    /// Taken from the signed bytes.
    Measured(Measured),
    /// Written down by somebody.
    Declared(Declared),
    /// Not answered, and here is which absence this is.
    Unknown {
        /// Which absence.
        why: Unknown,
    },
}

impl From<ReceiptAnswer> for Answer {
    fn from(answer: ReceiptAnswer) -> Self {
        match answer {
            ReceiptAnswer::Measured(m) => Answer::Measured(m),
            ReceiptAnswer::Unknown { why } => Answer::Unknown { why },
        }
    }
}

impl From<DeclaredAnswer> for Answer {
    fn from(answer: DeclaredAnswer) -> Self {
        match answer {
            DeclaredAnswer::Declared(d) => Answer::Declared(d),
            DeclaredAnswer::Unknown { why } => Answer::Unknown { why },
        }
    }
}

/// 🔴 The face-level vocabulary — **different words on purpose** (`req/544` AC-11).
///
/// A face says what it could observe if a run happened. A receipt says what one run observed. If
/// both were spelled `measured`, a reader could not tell a capability from an observation, and an
/// implementation could print the first where the second belongs and pass every test. So the words
/// do not overlap, and no conversion between the two vocabularies exists.
///
/// A face that answers [`FacePosture::CanMeasure`] and a receipt from that face that answers
/// [`Unknown::NotYetAsked`] are **both right**: nothing was read on that run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacePosture {
    /// Effects on this route reach a membrane that observes this question, so a receipt issued
    /// through it can carry an answer. It is a claim about the route, and it can be wrong.
    CanMeasure,
    /// This route cannot observe this question at all, whatever the run does.
    CannotMeasure,
    /// Whatever is known about this question came from somebody writing it down.
    OnlyDeclared,
}

/// 🔴 The coverage of one receipt: four questions, four answers, derived and not stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptCoverage {
    /// Which of ASM-14's two shapes the receipt is. Printed because three of the four answers
    /// below are kind-dependent, and a reader who does not know the kind cannot tell an absence
    /// that is normal from one that is a defect.
    pub kind: ReceiptKind,
    /// The four rows, in [`Question::ALL`]'s order.
    pub rows: Vec<(Question, ReceiptAnswer)>,
}

impl ReceiptCoverage {
    /// 🔴 The projection. **Total**: every payload this crate can decode has a coverage table, and
    /// no input reaches this function except the payload itself.
    ///
    /// `req/544` R-3h: a receipt for which the table could not be derived would make R-3c false,
    /// because "the declaration is derivable from the receipt" would have exceptions the reader
    /// cannot see. So there is no `Result` here and no panic road: every arm ends in an answer.
    #[must_use]
    pub fn of(payload: &ReceiptPayload) -> Self {
        let rows = vec![
            (Question::WhatWasRead, Self::what_was_read(payload)),
            (Question::WhatWasWritten, Self::what_was_written(payload)),
            (Question::When, Self::when(payload)),
            (Question::ByWhoseAuthority, Self::by_whose_authority()),
        ];
        Self {
            kind: payload.receipt_kind,
            rows,
        }
    }

    /// The read column, spelled in `req/510`'s vocabulary.
    fn what_was_read(payload: &ReceiptPayload) -> ReceiptAnswer {
        const FROM: &[&str] = &["read_set"];
        const NOT_COVERED: &str = "the agent's own read traffic. This is the set of objects gx read \
                                   to build an inverse, not what the agent asked the server for, so \
                                   'did B read what A wrote' is not decided here";
        match &payload.read_set {
            Some(ReadSet::PerRead(entries)) => ReceiptAnswer::Measured(Measured {
                from: FROM,
                reading: format!(
                    "G3: one entry per distinct object, {} of them, in the signed bytes",
                    entries.len()
                ),
                not_covered: NOT_COVERED,
            }),
            Some(ReadSet::PerEffectRoot { leaf_count, .. }) => ReceiptAnswer::Measured(Measured {
                from: FROM,
                reading: format!(
                    "G4: a root over {leaf_count} entries. Which object was read is not decidable \
                     from this document alone — the entries are beside it"
                ),
                not_covered: NOT_COVERED,
            }),
            // 🔴 `req/544` R-3b: this is **measured**, and it is the strongest of the three
            // readings rather than a kind of absence.
            Some(ReadSet::Nothing) => ReceiptAnswer::Measured(Measured {
                from: FROM,
                reading: "the escrow ran and read nothing, which answers the question for every \
                          locator there is"
                    .to_string(),
                not_covered: NOT_COVERED,
            }),
            Some(ReadSet::NoEscrowRecord) => ReceiptAnswer::Unknown {
                why: Unknown::NoEscrowRecord,
            },
            Some(ReadSet::ReadsNotJournalled) => ReceiptAnswer::Unknown {
                why: Unknown::ReadsNotJournalled,
            },
            None => ReceiptAnswer::Unknown {
                why: Unknown::NotYetAsked,
            },
        }
    }

    /// The write column. The scope travels with the answer, because a fingerprint without its
    /// scope is 32 bytes that could be about anything.
    fn what_was_written(payload: &ReceiptPayload) -> ReceiptAnswer {
        match &payload.postcondition_fingerprint {
            Some(_) => ReceiptAnswer::Measured(Measured {
                from: &[
                    "postcondition_fingerprint",
                    "precondition_fingerprint",
                    "fingerprint_scope",
                ],
                reading: format!(
                    "a fingerprint before and after, both taken over `{}`",
                    payload.fingerprint_scope
                ),
                not_covered: "anything outside that scope, and the contents themselves: a \
                              fingerprint says that the state changed, not what it changed to",
            }),
            None => ReceiptAnswer::Unknown {
                why: Unknown::NoPostconditionObserved,
            },
        }
    }

    /// 🔴 The when column, and the honest half of it.
    ///
    /// There is **no clock in the signed payload** — E-M2-6 took `issued_at` out of the signed core
    /// (CM-5, "no clock read in the signed payload"), and the `issued_at` on the document sits
    /// outside the signature, which `gx receipt verify` already reports as
    /// `issued_at_signed: false`. So the strongest honest answer here is an **order**, never a
    /// time, and that bound is carried in the answer rather than left for a reader to discover.
    fn when(payload: &ReceiptPayload) -> ReceiptAnswer {
        match &payload.inclusion_proof {
            Some(proof) => ReceiptAnswer::Measured(Measured {
                from: &["inclusion_proof"],
                reading: format!(
                    "leaf {} of a log of {} at the moment it was witnessed",
                    proof.leaf_index, proof.tree_size
                ),
                not_covered: "a time. No clock is inside the signature (E-M2-6), so what is fixed \
                              is the position in the log and not the hour it happened",
            }),
            None => ReceiptAnswer::Unknown {
                why: Unknown::NoInclusionProof,
            },
        }
    }

    /// 🔴 The authority column, which takes no payload because there is nothing in a payload to
    /// take.
    ///
    /// The signature of this function is the finding: `req/544` §1-3 item 3 read the fifteen
    /// members and none of them is an actor. This is `req/544` AC-9's first known unmet row, and it
    /// is printed rather than hidden — a face that answered this question from `key_id` would be
    /// reporting a key as a person.
    const fn by_whose_authority() -> ReceiptAnswer {
        ReceiptAnswer::Unknown {
            why: Unknown::ActorNotInReceipt,
        }
    }

    /// The questions this receipt does **not** answer, derived from the table above.
    ///
    /// 🔴 `req/544` KA-4b: this is a projection of [`ReceiptCoverage::rows`] and not a list written
    /// out by hand. A change that hides an unmet row has to change the table, and the table is what
    /// the coverage probes pin — so the two cannot be moved independently, which is the property a
    /// hand-written list of known gaps does not have.
    #[must_use]
    pub fn unmet(&self) -> Vec<(Question, Unknown)> {
        self.rows
            .iter()
            .filter_map(|(question, answer)| match answer {
                ReceiptAnswer::Unknown { why } => Some((*question, *why)),
                ReceiptAnswer::Measured(_) => None,
            })
            .collect()
    }
}
