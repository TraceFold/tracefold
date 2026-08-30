// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-928-ATT** — the attach interface: reading what somebody *else* attested, and saying
//! exactly how much of it survived the trip.
//!
//! Spec: `req/929_ATTACH_INTERFACE_REQDEF_2026-08-30.md`. `req/359` calls the same contract by its
//! other name — an attach interface read backwards is an import — and fixes the one rule both
//! directions share: **what the other format did not attest arrives as an absence, and is never
//! guessed at.**
//!
//! # What this is, in one line
//!
//! `foreign attested document → (each of [`Question::ALL`] → a claim, or a named reason there is
//! none)`.
//!
//! # 🔴 Why nothing here can be `Measured`
//!
//! [`crate::coverage`] already holds this workspace's three-valued vocabulary, and its
//! [`crate::coverage::Measured`] arm means *taken from bytes this workspace signed and checked*.
//! Verifying a Sigstore bundle needs ECDSA P-256, an X.509 chain, a Fulcio trust root and a Rekor
//! inclusion proof; this crate's cryptography is `ed25519-dalek` and nothing else. So a document
//! that arrives here is **unverified by construction**, and [`AttachedAnswer`] therefore has two
//! arms rather than three.
//!
//! That is the honest shape and not a shortcut: the missing arm is a *fact about this build*, and
//! writing it into the type means no later hand can accidentally promote a foreign claim to a
//! measurement without deleting an arm and breaking every match. `coverage.rs` withholds
//! `Deserialize` from `Measured` for the same reason; this is that rule one layer out.
//!
//! # 🔴 Three absences, because they have three different futures
//!
//! [`NotAttested`] refuses to collapse:
//!
//! * [`NotAttested::FormatHasNoField`] — the other format has nowhere to put this. **Permanent.**
//!   No later build, and no better-behaved publisher, can fill it.
//! * [`NotAttested::DocumentSilent`] — the format has the field and *this* document left it empty.
//!   A different document may answer.
//! * [`NotAttested::NotReadByThisBuild`] — the bytes may well say it and this reader does not look.
//!   **Releasable**, by this project, on purpose.
//!
//! Folding these into one "unknown" would make a gap that a later lane can close look identical to
//! one that physics forbids. `req/510` spent a lane undoing exactly that collapse one layer in.
//!
//! # 🔴 The bundle is usually not in the response (`req/969`)
//!
//! Every attestation entry observed in a real response carries `"bundle": null` and a `bundle_url`
//! pointing at storage, so the inline road this module was first written for is the road real
//! responses do **not** take. That is [`Refusal::BundleExternalized`], and it is a separate arm
//! from [`Refusal::NoBundle`] for the same reason [`NotAttested`] keeps three members: an absence a
//! caller can act on must not look like one it cannot. The caller fetches and decompresses
//! (`Content-Type: application/x-snappy`), then comes back through [`read_resolved_bundle`].
//!
//! This crate performs no I/O and grows no decompressor for it. `Measured` is out of reach because
//! of a missing dependency and the type says so; retrieval is out of reach because a network client
//! in this dependency graph trips a floor gate, and the refusal vocabulary says that.
//!
//! # 🔴 What is deliberately *not* checked
//!
//! No signature is verified, no certificate is parsed, and no transparency-log inclusion is
//! confirmed. This reader answers "what does this document say, and about what" — never "is it
//! true". A caller that needs the second must not read a [`Declared`] as though it were the first,
//! which is why every row carries its origin in [`Declared::source`].
//!
//! # The `.gx` connection is a name, and naming is not shipping
//!
//! [`AttachedEvidence::GX_KIND`] is [`GxKind::AttachSource`], the number `req/922`'s registry
//! already reserved. [`GxKind::is_shipped`] stays `false`: this lane adds no codec and rewrites no
//! `.gx` file, so nothing already written changes meaning. The link exists so that the eventual
//! codec has a fixed address, not so that this module can claim one.

use core::fmt;

use serde::Serialize;
use serde_json::Value;

use crate::coverage::{Declared, Question};
use crate::gxfile::GxKind;

/// The format this reader understands, as it is named in [`AttachedSource::format`].
const GITHUB_ATTESTATION: &str = "github-attestation/in-toto-statement";

/// 🔴 Why a question has no claim against it. Three members, three futures (module header).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotAttested {
    /// The foreign format defines no field that answers this question. Permanent: a later build
    /// cannot read what was never written down, and a better publisher has nowhere to write it.
    FormatHasNoField,
    /// The format has the field and this document left it absent or empty. A statement about this
    /// document, not about the format and not about the operation.
    DocumentSilent,
    /// The document may carry it and **this build does not look**. Releasable by this project; the
    /// reason names the work rather than implying the answer does not exist.
    NotReadByThisBuild,
}

impl NotAttested {
    /// The reason, spelled out. An exhaustive match: a fourth kind of absence stops the build.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            NotAttested::FormatHasNoField => {
                "the format this document is written in has no field that answers this question, \
                 so no publisher could have stated it and no later reader can recover it"
            }
            NotAttested::DocumentSilent => {
                "the format has a field for this and this document left it empty, so the silence \
                 is this document's and not the format's"
            }
            NotAttested::NotReadByThisBuild => {
                "this build does not read the part of the document that would answer it; the \
                 absence is this reader's and is releasable, not the publisher's"
            }
        }
    }
}

/// 🔴 What a foreign document says to one question: a claim somebody wrote, or a named absence.
///
/// There is **no `Measured` arm**, and its absence is the module header's whole argument.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "value", rename_all = "snake_case")]
pub enum AttachedAnswer {
    /// The document states this. Unverified: [`Declared::source`] says which member it was read
    /// off, so a reader can go and look rather than being asked to trust the translation.
    Declared(Declared),
    /// The document does not state this, and here is which absence it is.
    Absent {
        /// Which absence.
        why: NotAttested,
    },
}

/// Where one row-set came from, in enough detail to go back and look.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AttachedSource {
    /// The format that was read, as [`GITHUB_ATTESTATION`] spells it.
    pub format: &'static str,
    /// The path through the response body, e.g. `attestations[0]`.
    pub locator: String,
    /// The statement's own `predicateType`, which says what *kind* of claim this is. `None` when
    /// the statement omitted it — carried rather than defaulted, because a missing predicate type
    /// means the claim's meaning is undeclared and that is worth seeing.
    pub predicate_type: Option<String>,
}

/// 🔴 One foreign document, projected onto the four questions this workspace judges a face on.
///
/// The questions are [`Question`]'s and not new ones, so an attached document and a receipt issued
/// here can be laid side by side and compared row for row. That comparability is the point of
/// reusing the vocabulary rather than minting a second one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AttachedEvidence {
    /// Which document this is.
    pub source: AttachedSource,
    /// The four rows, in [`Question::ALL`]'s order. Total: a question never simply fails to appear.
    pub rows: Vec<(Question, AttachedAnswer)>,
}

impl AttachedEvidence {
    /// The `.gx` registry entry this shape belongs to (module header: a name, not a codec).
    pub const GX_KIND: GxKind = GxKind::AttachSource;

    /// The answer to one question, or `None` if the table were ever not total.
    #[must_use]
    pub fn answer(&self, question: Question) -> Option<&AttachedAnswer> {
        self.rows
            .iter()
            .find(|(q, _)| *q == question)
            .map(|(_, answer)| answer)
    }

    /// The questions this document does not answer, with the reason for each.
    ///
    /// A projection of [`AttachedEvidence::rows`] rather than a hand-kept list, so hiding a gap
    /// requires changing the table the probes pin.
    #[must_use]
    pub fn unanswered(&self) -> Vec<(Question, NotAttested)> {
        self.rows
            .iter()
            .filter_map(|(question, answer)| match answer {
                AttachedAnswer::Absent { why } => Some((*question, *why)),
                AttachedAnswer::Declared(_) => None,
            })
            .collect()
    }
}

/// Why a foreign document was not read.
///
/// Every variant is a stated reason. The refusal that matters most is the one that is *not* here:
/// there is no "returned nothing", because a caller cannot tell an empty success from an artifact
/// that genuinely carries no attestation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The bytes are not JSON at all.
    NotJson {
        /// The decoder's own words.
        detail: String,
    },
    /// The body carries no `attestations` array — an error page, or a different endpoint.
    NoAttestationsMember,
    /// One entry carries no `bundle`.
    NoBundle {
        /// Which entry.
        index: usize,
    },
    /// 🔴 One entry does not carry its bundle: it carries the address of one.
    ///
    /// Distinct from [`Refusal::NoBundle`] for the reason [`NotAttested::NotReadByThisBuild`] is
    /// distinct from [`NotAttested::DocumentSilent`]: **this one is a fetch away**. A caller told
    /// "no bundle" stops; a caller told "elsewhere, here" continues. Reporting an externalised
    /// bundle as an absent one is the answer-vocabulary's collapse committed one layer out, and it
    /// is what the collected response actually provoked (`req/969` §1-3).
    ///
    /// This crate does not fetch. Doing so would put a network client in `gx-witness`'s dependency
    /// graph, which a floor gate refuses on purpose, so retrieval — and the Snappy decompression
    /// the storage layer applies — belong to the caller, which then returns here through
    /// [`read_resolved_bundle`].
    BundleExternalized {
        /// Which entry.
        index: usize,
        /// Where the entry says its bundle lives. Never empty: an address that addresses nothing
        /// would send a caller to fetch it (`req/969` INV-E2).
        url: String,
    },
    /// One bundle carries no `dsseEnvelope`.
    NoDsseEnvelope {
        /// Which entry.
        index: usize,
    },
    /// One envelope carries no string `payload`.
    NoPayload {
        /// Which entry.
        index: usize,
    },
    /// One payload is not the base64 the DSSE envelope promises.
    PayloadNotBase64 {
        /// Which entry.
        index: usize,
        /// The decoder's own words.
        detail: String,
    },
    /// One decoded payload is not a JSON statement.
    StatementNotJson {
        /// Which entry.
        index: usize,
        /// The decoder's own words.
        detail: String,
    },
    /// 🔴 The statement names no subject at all, so it cannot be tied to the digest that was asked
    /// about. Refused rather than returned with an empty write row: an unbindable document
    /// presented as evidence about a particular artifact is a false association wearing the
    /// clothes of an honest absence.
    SubjectAbsent {
        /// Which entry.
        index: usize,
    },
    /// 🔴 The statement names subjects and none of them is the one that was asked for.
    ///
    /// The subject a document carries is a **claim**. Admitting it unchecked would file one
    /// artifact's provenance under another — the shape [`crate::gxfile::Refusal::IdentityMismatch`]
    /// refuses one layer in.
    SubjectMismatch {
        /// Which entry.
        index: usize,
        /// The digest the caller asked about.
        expected: String,
        /// What the statement's subjects actually name.
        found: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotJson { detail } => {
                write!(f, "the response body is not JSON: {detail}")
            }
            Refusal::NoAttestationsMember => write!(
                f,
                "the response body carries no `attestations` array; this is refused rather than \
                 read as an empty list, because no attestations and not an attestations response \
                 send a caller to two different places"
            ),
            Refusal::NoBundle { index } => {
                write!(f, "attestations[{index}] carries no `bundle`")
            }
            Refusal::BundleExternalized { index, url } => write!(
                f,
                "attestations[{index}] holds no bundle inline and names {url} as where its bundle \
                 lives; this is not an absent attestation but an unfetched one, and this crate \
                 does not fetch. Retrieve and decompress it, then read it with \
                 `read_resolved_bundle`"
            ),
            Refusal::NoDsseEnvelope { index } => {
                write!(f, "attestations[{index}].bundle carries no `dsseEnvelope`")
            }
            Refusal::NoPayload { index } => write!(
                f,
                "attestations[{index}].bundle.dsseEnvelope carries no string `payload`"
            ),
            Refusal::PayloadNotBase64 { index, detail } => {
                write!(f, "attestations[{index}]'s payload is not base64: {detail}")
            }
            Refusal::StatementNotJson { index, detail } => write!(
                f,
                "attestations[{index}]'s payload decodes but is not a JSON statement: {detail}"
            ),
            Refusal::SubjectAbsent { index } => write!(
                f,
                "attestations[{index}]'s statement names no subject, so nothing in it ties the \
                 document to the digest that was asked about"
            ),
            Refusal::SubjectMismatch {
                index,
                expected,
                found,
            } => write!(
                f,
                "attestations[{index}] attests {found} and the digest asked about is {expected}; \
                 the subject a document names is a claim, and attaching it to a different artifact \
                 is what comparing them prevents"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// A digest split into its algorithm — when the spelling carries one — and its hex, both folded to
/// lower case so two spellings of one digest compare equal.
///
/// `None` for the algorithm means the caller wrote a bare hex and has chosen not to constrain it;
/// it does not mean "any algorithm is as good as any other" as a matter of policy.
fn split_digest(text: &str) -> (Option<String>, String) {
    match text.split_once(':') {
        Some((alg, hex)) => (
            Some(alg.trim().to_ascii_lowercase()),
            hex.trim().to_ascii_lowercase(),
        ),
        None => (None, text.trim().to_ascii_lowercase()),
    }
}

/// 🔴 Read a GitHub attestations response, one row-set per attestation.
///
/// `expected_subject` is the digest the caller asked about — the same value the endpoint's own path
/// is keyed on — and every statement is checked against it (module header; [`Refusal`]).
///
/// # Errors
/// One [`Refusal`] per condition. A document that cannot be read is never answered with an empty
/// success.
pub fn read_github_attestations(
    bytes: &[u8],
    expected_subject: &str,
) -> Result<Vec<AttachedEvidence>, Refusal> {
    let body: Value = serde_json::from_slice(bytes).map_err(|e| Refusal::NotJson {
        detail: e.to_string(),
    })?;
    let attestations = body
        .get("attestations")
        .and_then(Value::as_array)
        .ok_or(Refusal::NoAttestationsMember)?;

    // Carried through as written: the split into (algorithm, hex) happens where the comparison
    // does, so there is one place that decides what "the same digest" means.
    let wanted = expected_subject;
    let mut out = Vec::with_capacity(attestations.len());
    for (index, attestation) in attestations.iter().enumerate() {
        let bundle = bundle_of(index, attestation)?;
        let statement = statement_in(index, bundle)?;
        let initiator = attestation.get("initiator").and_then(Value::as_str);
        out.push(project(
            index,
            format!("attestations[{index}]"),
            initiator,
            &statement,
            wanted,
        )?);
    }
    Ok(out)
}

/// 🔴 The inline bundle, or the reason there is not one — told apart by whether the entry named a
/// place to find it.
///
/// `serde_json` returns `Some(&Value::Null)` for a key that is present and null, so the first
/// version of this walk fell through a `null` bundle into the `dsseEnvelope` lookup and refused
/// with [`Refusal::NoDsseEnvelope`] — announcing that a document with a perfectly good envelope had
/// none. **Every real entry observed so far takes this road** (`req/948b_artifacts`), so it was not
/// an edge case; it was the case.
fn bundle_of(index: usize, attestation: &Value) -> Result<&Value, Refusal> {
    match attestation.get("bundle") {
        Some(bundle) if !bundle.is_null() => Ok(bundle),
        // Present-and-empty is not an address: a caller sent to fetch `""` is worse off than one
        // told there is nothing, because it will spend a request finding that out (INV-E2).
        _ => match attestation.get("bundle_url").and_then(Value::as_str) {
            Some(url) if !url.trim().is_empty() => Err(Refusal::BundleExternalized {
                index,
                url: url.to_string(),
            }),
            _ => Err(Refusal::NoBundle { index }),
        },
    }
}

/// The in-toto statement inside one bundle's DSSE envelope.
///
/// Shared by both entry points, so an inline bundle and a fetched one are read by the same code and
/// cannot drift into two dialects of the same format.
fn statement_in(index: usize, bundle: &Value) -> Result<Value, Refusal> {
    let envelope = bundle
        .get("dsseEnvelope")
        .ok_or(Refusal::NoDsseEnvelope { index })?;
    let payload = envelope
        .get("payload")
        .and_then(Value::as_str)
        .ok_or(Refusal::NoPayload { index })?;
    let decoded = gx_core::b64::decode(payload).map_err(|detail| Refusal::PayloadNotBase64 {
        index,
        detail: detail.to_string(),
    })?;
    serde_json::from_slice(&decoded).map_err(|e| Refusal::StatementNotJson {
        index,
        detail: e.to_string(),
    })
}

/// 🔴 Read a bundle the caller fetched from the address a [`Refusal::BundleExternalized`] handed
/// back, after decompressing it.
///
/// The other half of the externalised road. Without it the repair would be a politer dead end — a
/// refusal that names a place and no door to bring the answer back to.
///
/// `initiator` is the value from the *outer* response entry, because the bundle does not carry one;
/// passing `None` is honest and costs the authority row, which then reports
/// [`NotAttested::NotReadByThisBuild`] rather than claiming nobody is named.
///
/// # 🔴 What this does not check
///
/// That these bytes came from that URL. This is translation, not verification: a caller who hands
/// over different bytes gets an answer about different bytes. The same reason nothing here reaches
/// `Measured` applies one level up — [`Declared::source`] records where each row was read off so a
/// reader can go and look instead of being asked to trust the trip.
///
/// # Errors
/// One [`Refusal`] per condition, at `index` 0 since a resolved bundle stands alone.
pub fn read_resolved_bundle(
    bytes: &[u8],
    expected_subject: &str,
    initiator: Option<&str>,
) -> Result<AttachedEvidence, Refusal> {
    let bundle: Value = serde_json::from_slice(bytes).map_err(|e| Refusal::NotJson {
        detail: e.to_string(),
    })?;
    let statement = statement_in(0, &bundle)?;
    project(
        0,
        "bundle.dsseEnvelope".to_string(),
        initiator,
        &statement,
        expected_subject,
    )
}

/// The projection itself: four questions, four answers, no road that returns fewer.
///
/// 🔴 This is the seam. It takes a statement and an initiator — never a transport shape — so an
/// inline bundle and a fetched one reach the four questions by the same road. `req/948c` predicted
/// a second *format* would test the seam; a second *carriage* of the same format tested it first,
/// and the return type did not have to widen (`req/969` §6).
fn project(
    index: usize,
    locator: String,
    initiator: Option<&str>,
    statement: &Value,
    wanted: &str,
) -> Result<AttachedEvidence, Refusal> {
    let written = what_was_written(index, &locator, statement, wanted)?;

    Ok(AttachedEvidence {
        source: AttachedSource {
            format: GITHUB_ATTESTATION,
            locator: locator.clone(),
            predicate_type: statement
                .get("predicateType")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        rows: vec![
            // 🔴 The permanent one. in-toto states what a build *declared it depended on*; it has
            // no field for what the build read, at any granularity. `resolvedDependencies` is the
            // near-miss that makes this worth writing down: it looks like a read-set and is a
            // declaration of intent, so translating it into one would manufacture the very fact
            // this product exists to measure (`req/929` L-1).
            (
                Question::WhatWasRead,
                AttachedAnswer::Absent {
                    why: NotAttested::FormatHasNoField,
                },
            ),
            (Question::WhatWasWritten, written),
            (Question::When, when(statement)),
            (
                Question::ByWhoseAuthority,
                by_whose_authority(&locator, initiator),
            ),
        ],
    })
}

/// The write column: the statement's subjects, checked against the digest that was asked about.
///
/// 🔴 A missing or empty `subject` is a **refusal**, not a quiet absence. The caller asked what is
/// attested about one digest; a document that names no subject cannot be tied to that digest, so
/// returning it as evidence *about* it would assert a link nothing in the bytes supports. The row
/// would read as an honest absence while the row-set as a whole made a false association — which is
/// worse than a refusal, because it looks careful.
fn what_was_written(
    index: usize,
    locator: &str,
    statement: &Value,
    wanted: &str,
) -> Result<AttachedAnswer, Refusal> {
    let subjects = match statement.get("subject").and_then(Value::as_array) {
        Some(subjects) if !subjects.is_empty() => subjects,
        _ => return Err(Refusal::SubjectAbsent { index }),
    };

    let (want_alg, want_hex) = split_digest(wanted);
    let mut named = Vec::with_capacity(subjects.len());
    let mut matched = false;
    for subject in subjects {
        let name = subject
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>");
        // 🔴 Every digest on the subject, not the first one. in-toto lets a subject carry several
        // algorithms, and stopping at whichever the map yields first would refuse a document that
        // does name the requested digest, under a key that happened to sort second.
        let mut spellings = Vec::new();
        if let Some(digests) = subject.get("digest").and_then(Value::as_object) {
            for (alg, value) in digests {
                let Some(hex) = value.as_str() else { continue };
                // 🔴 The algorithm is part of the comparison. Matching on the hex alone would let
                // `sha512:X` satisfy a request for `sha256:X` -- a collision no attacker needs to
                // find, because it is the *label* that was dropped. When the caller supplied no
                // algorithm the hex alone decides, and that is the caller's choice, not this
                // function widening it.
                let same_alg = want_alg
                    .as_ref()
                    .is_none_or(|want| want == &alg.to_ascii_lowercase());
                if same_alg && hex.to_ascii_lowercase() == want_hex {
                    matched = true;
                }
                spellings.push(format!("{alg}:{hex}"));
            }
        }
        if spellings.is_empty() {
            named.push(format!("{name} (no digest)"));
        } else {
            named.push(format!("{name} ({})", spellings.join(", ")));
        }
    }

    if !matched {
        return Err(Refusal::SubjectMismatch {
            index,
            expected: wanted.to_string(),
            found: named.join(", "),
        });
    }

    Ok(AttachedAnswer::Declared(Declared {
        source: format!("{locator}.bundle.dsseEnvelope.payload -> in-toto subject[]"),
        claim: format!(
            "{} subject(s) attested: {}. This names what was produced, not how it was produced, \
             and it is the publisher's statement rather than a checked fact",
            named.len(),
            named.join(", ")
        ),
    }))
}

/// 🔴 The when column — and the one place the two *releasable* absences are told apart.
///
/// Build timestamps live inside the `predicate`, whose schema varies by `predicateType` and which
/// the published API description does not constrain. This reader therefore does not read the
/// predicate's interior. But it can still tell two different situations apart **without knowing
/// that schema at all**:
///
/// * the predicate is absent or empty — the document carries no build detail of any kind, so the
///   silence is the *document's* ([`NotAttested::DocumentSilent`]);
/// * the predicate has content — something is in there and *this reader* declines to open it
///   ([`NotAttested::NotReadByThisBuild`]).
///
/// The distinction costs one `is_empty` and buys a caller the difference between "asking a better
/// publisher" and "waiting for a better build".
fn when(statement: &Value) -> AttachedAnswer {
    let has_content = match statement.get("predicate") {
        Some(Value::Object(map)) => !map.is_empty(),
        Some(Value::Null) | None => false,
        // A non-object predicate is still content; this reader does not judge its shape.
        Some(_) => true,
    };
    AttachedAnswer::Absent {
        why: if has_content {
            NotAttested::NotReadByThisBuild
        } else {
            NotAttested::DocumentSilent
        },
    }
}

/// The authority column.
///
/// The response's own `initiator` is a name somebody wrote down. When it is absent the identity
/// lives in the bundle's Fulcio certificate, which this build does not parse — releasable, and said
/// as such rather than reported as "no authority".
///
/// Taken as an `Option<&str>` rather than dug out of a response body, because a bundle fetched from
/// `bundle_url` carries no `initiator`: the name lives in the outer entry, and the caller that held
/// both is the one that can supply it. Passing `None` loses the name honestly; inventing a place
/// for it to live in the bundle would not.
fn by_whose_authority(locator: &str, initiator: Option<&str>) -> AttachedAnswer {
    match initiator {
        Some(initiator) => AttachedAnswer::Declared(Declared {
            source: format!("{locator}.initiator"),
            claim: format!(
                "the response names `{initiator}` as the initiator. This is a name in the \
                 response body, not an identity bound by a verified signature"
            ),
        }),
        None => AttachedAnswer::Absent {
            why: NotAttested::NotReadByThisBuild,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Case and surrounding space do not make two digests into two digests.
    #[test]
    fn a_digest_compares_equal_however_it_was_spelled() {
        assert_eq!(split_digest("sha256:AB12"), split_digest(" sha256 : ab12 "));
        assert_eq!(split_digest("ab12"), (None, "ab12".to_string()));
    }

    /// 🔴 The algorithm survives the split, so a later comparison can use it.
    ///
    /// The bug this pins was in this file: the first version kept only the hex, which would have
    /// let `sha512:X` answer a request for `sha256:X`.
    #[test]
    fn the_algorithm_is_kept_and_not_thrown_away_with_the_prefix() {
        assert_eq!(
            split_digest("sha512:ab12"),
            (Some("sha512".to_string()), "ab12".to_string())
        );
        assert_ne!(split_digest("sha512:ab12"), split_digest("sha256:ab12"));
    }

    /// The absence vocabulary is three distinct sentences (`req/929` AC-A6, unit half).
    #[test]
    fn every_absence_explains_itself() {
        for why in [
            NotAttested::FormatHasNoField,
            NotAttested::DocumentSilent,
            NotAttested::NotReadByThisBuild,
        ] {
            assert!(!why.because().is_empty(), "{why:?} has no reason");
        }
    }
}
