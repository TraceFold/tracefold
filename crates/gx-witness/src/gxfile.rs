// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-922-F2 phase 1** — the `.gx` object file: a kind-tagged wrapper around one canonical
//! body, whose identity a reader recomputes rather than believes.
//!
//! Spec: `req/922_GX_FORMAT_REQDEF_2026-08-29.md` §1-§3 and §7-§8, and the proposed chapter
//! `req/922_artifacts/spec42_fileformat_chapter_draft.md` §3.16. **That chapter is a proposal and
//! not spec canon**, so nothing here may be cited as 42; the citation is the reqdef.
//!
//! # What this is, in one line
//!
//! `magic || format_version || kind || claimed identity || the existing DSSE-envelope document`.
//!
//! # The three rules that decide every line below
//!
//! 1. **Only the body is canonical.** `enc()` applies to what is inside the envelope — for this
//!    phase, `canonical_dagcbor(ReceiptPayload)`, the bytes 42 §3.10 already signs. The envelope
//!    itself is written with `serde_json`, which is the spelling `.gx/receipts/<TID>.json` has
//!    always had. Canonicalising the wrapper would put a second canonical layer outside the
//!    signature, which is the "one fact, two preimages" shape `req/920` §6 forbids.
//! 2. **The stored identity is a claim.** `req/922` §7-3: a reader recomputes
//!    `BLAKE3(enc(body))` and refuses the file when the two disagree. A stored CID treated as
//!    authority *is* the second identity path the same ruling forbids.
//! 3. **An unrecognised `kind` is a refusal, not a shrug.** Refusing to admit a shape this build
//!    cannot verify is fail-closed; it is not the "could not check" value, and the two are kept
//!    apart — [`Refusal::UnknownKind`] (not in the registry at all) and
//!    [`Refusal::KindNotShipped`] (in the registry, no codec in this build) are two sentences
//!    because they are two facts.
//!
//! # 🔴 The fourth rule, added when the second kind shipped (**R-930-B1**, `req/939` §2)
//!
//! 4. **A kind is named inside the bytes its identity covers.** `req/930` §4 Q3 named the defect
//!    (C-1): the identity is `BLAKE3(enc(body))` and `kind` sits in the header, outside it, so a
//!    body alone does not say which kind it is. `Receipt` was safe by accident — its payload type
//!    is inside the DSSE pre-authentication encoding, so its signature witnesses its kind, which is
//!    the reason [`crate::dsse`] gives for each of its five payload types ("it is inside the signed
//!    bytes, so a signature over one cannot be replayed as a signature over another"). A kind with
//!    no signer has no such bytes and carries the name inside its canonical body instead
//!    ([`KindWitness`]). Two kinds then cannot share a preimage, which is what `gx_log::tile`'s
//!    `0x00`/`0x01` prefixes buy for leaves and nodes.
//!
//! In-band rather than as a prefix byte, for two reasons recorded in `req/939` §2-B: prefixing
//! would move the identity of every `.gx` file already written, and the prefixed road in gx-canon
//! is closed at exactly the two domains 42 §3.11 defines, so opening it is a spec change and not an
//! implementation. What the rule does **not** do is make an unsigned file authentic — an attacker
//! may rewrite the header and the tag together, and the result is a different file with a different
//! identity that nobody signed (`req/939` §2-F).
//!
//! # Why one kind ships and twelve are named
//!
//! `req/922` §5 splits the format into a registry (F1, a document) and an implementation (F2,
//! this file), and this phase implements **`Receipt` only** — the one object that already meets
//! all three of "one file, one object", "carries its own signature" and "has a `Cid` over
//! canonical DAG-CBOR" (`gx_format_design_scout.md` §2). The other eleven names are declared so
//! that a file written by a later build is refused *by name* rather than as gibberish, and so that
//! the registry's numbers are fixed before anything is exported. Naming a kind is not shipping it,
//! and [`GxKind::is_shipped`] is the line between the two.
//!
//! 🔴 **What fixing the numbers before exporting anything cost** (`req/930` §4 Q6, placed here by
//! R-930-9 because a retraction belongs in the file it retracts): "before anything is exported" was
//! also "before anything was verified". The split into twelve was frozen while the shipped set was
//! zero, so neither the sufficiency of the twelve nor the difference between `Candidate` and
//! `Transformation` was ever tested against a written file, and appending is now the only move
//! left. The numbers themselves are not withdrawn — a stranger's file may already carry one — but
//! the reasoning above is: fixing a registry protects readers, and it does not make the division it
//! fixes correct.
//!
//! # Errors do not enter the `gx_code` funnel here
//!
//! [`Refusal`] is a local vocabulary, deliberately outside [`crate::Error`]: adding variants to
//! that enum moves `ERROR_KINDS`, `gx-api`'s `REFUSALS` table and its length assertion, which is a
//! spec-44 §2.3 change and not a phase-1 one. The CLI folds a `Refusal` into its existing
//! `Malformed { what: "gx object file", detail }` exactly as it already folds a `serde_json`
//! error, and the fold is written down here rather than left to be discovered (req/88 §3 Λ4).

use core::fmt;

use gx_canon::{cbor, cid};
use gx_core::Cid;

use crate::design_token::{DesignToken, DESIGN_TOKEN_TAG};
use crate::dsse::RECEIPT_PAYLOAD_TYPE;
use crate::receipt::Receipt;

/// The two bytes every `.gx` file opens with.
pub const MAGIC: [u8; 2] = *b"gx";

/// The envelope-format version this build writes and the only one it reads.
///
/// Independent of `ReceiptPayload.payload_version` (the body's own generation counter, added by
/// `req/868`): one says how the wrapper is spelled, the other says which schema the wrapped value
/// was written against, and folding them would make a wrapper change look like a schema change.
pub const FORMAT_VERSION: u16 = 1;

/// `magic(2) + format_version(2) + kind(2) + claimed identity(32)`.
pub const HEADER_LEN: usize = 38;

/// 🔴 The kind registry (`req/922` F1; proposed chapter §3.16.1).
///
/// Twelve names, whose numbers are fixed here and are not to be reordered — a code is what a
/// stranger's file carries, so moving one silently re-labels every file already written. New kinds
/// are appended (additive, `req/922` §0 principle ④).
///
/// The denominator is the proposed chapter's: every object the SDK's 25 methods can reach, plus
/// the three kinds that live only inside `.gx/` (`EngineJournal`, `LedgerLeaf`, `EscrowedInverse`)
/// and are named because a registry's job is to name every kind a file can hold, not every kind an
/// HTTP route returns.
///
/// 🔴 **Thirteen since R-930-B1**, and the numbers are no longer the positions: `DesignToken` is
/// **15**, and 13 and 14 are left unclaimed because `req/930` §6-12 proposes them for `Revocation`
/// and `Standing` — two kinds whose payload types already exist in [`crate::dsse`] and which that
/// file's own §6-12 shows the twelve's denominator missed. An unclaimed number is refused as
/// [`Refusal::UnknownKind`], which is the whole cost of reserving one, and appending here is what
/// `req/922` §0 principle ④ makes cheap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum GxKind {
    /// 42 §3.10. **The one kind this build reads and writes.**
    Receipt,
    /// 42 §3.2's `Transformation` in its pre-verify phase (43 §0's narrow `Candidate`).
    Candidate,
    /// 42 §3.2.
    Transformation,
    /// 42 §3.11's `InclusionProof`.
    LedgerProof,
    /// 42 §3.11's `ConsistencyProof`.
    ConsistencyProof,
    /// 42 §3.11.
    Checkpoint,
    /// 42 §3.14.
    VerdictCheckpoint,
    /// 42 §3.8.
    EscalationTicket,
    /// 🔴 **Provisional** (`is_shipped() == false`). Until `req/824` landed A4/A5, `AttachSource`
    /// had no type table in 42 and no endpoint row in 44 — the number was reserved so that closing
    /// that debt later would not have to renumber anything. Both now exist: 42 §3.16 gives the type
    /// table (`AttachedEvidence`/`AttachedSource`/`AttachedAnswer`), and 44 §2.1 carries the 4 route
    /// rows (`POST`/`GET /attach-sources`, `GET /attach-sources/{id}`, `POST
    /// /attach-sources/{id}/observations`, L509-512) plus the 2 `gx_code` rows tied to them
    /// (`SOURCE_UNKNOWN`/`SOURCE_KEY_INVALID`, L870-871).
    AttachSource,
    /// 42 §3.13.
    EngineJournal,
    /// 42 §3.11.
    LedgerLeaf,
    /// 42 §3.12.
    EscrowedInverse,
    /// 🔴 **R-930-B1**, code **15** — the character kernel of a design board, or one four-layer
    /// declaration ([`crate::design_token`]). The first entry whose number is not its position, and
    /// the first whose body names its own kind (rule 4).
    DesignToken,
}

impl GxKind {
    /// The registry, in code order.
    ///
    /// 🔴 The codes were the positions for the first twelve and are not any more: `DesignToken` is
    /// 15. The order here is still the order of the numbers, so a reader can see the gap.
    pub const REGISTRY: [GxKind; 13] = [
        GxKind::Receipt,
        GxKind::Candidate,
        GxKind::Transformation,
        GxKind::LedgerProof,
        GxKind::ConsistencyProof,
        GxKind::Checkpoint,
        GxKind::VerdictCheckpoint,
        GxKind::EscalationTicket,
        GxKind::AttachSource,
        GxKind::EngineJournal,
        GxKind::LedgerLeaf,
        GxKind::EscrowedInverse,
        GxKind::DesignToken,
    ];

    /// The number this kind is written as. `0` is reserved and belongs to no kind, so a header of
    /// zeroed bytes is refused rather than read as the first entry.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            GxKind::Receipt => 1,
            GxKind::Candidate => 2,
            GxKind::Transformation => 3,
            GxKind::LedgerProof => 4,
            GxKind::ConsistencyProof => 5,
            GxKind::Checkpoint => 6,
            GxKind::VerdictCheckpoint => 7,
            GxKind::EscalationTicket => 8,
            GxKind::AttachSource => 9,
            GxKind::EngineJournal => 10,
            GxKind::LedgerLeaf => 11,
            GxKind::EscrowedInverse => 12,
            // 13 and 14 are unclaimed on purpose (the registry note above).
            GxKind::DesignToken => 15,
        }
    }

    /// The kind this number names, or `None` for a number no registry entry holds.
    #[must_use]
    pub fn from_code(code: u16) -> Option<Self> {
        Self::REGISTRY.into_iter().find(|k| k.code() == code)
    }

    /// The name the registry gives it, and the word a refusal prints.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            GxKind::Receipt => "Receipt",
            GxKind::Candidate => "Candidate",
            GxKind::Transformation => "Transformation",
            GxKind::LedgerProof => "LedgerProof",
            GxKind::ConsistencyProof => "ConsistencyProof",
            GxKind::Checkpoint => "Checkpoint",
            GxKind::VerdictCheckpoint => "VerdictCheckpoint",
            GxKind::EscalationTicket => "EscalationTicket",
            GxKind::AttachSource => "AttachSource",
            GxKind::EngineJournal => "EngineJournal",
            GxKind::LedgerLeaf => "LedgerLeaf",
            GxKind::EscrowedInverse => "EscrowedInverse",
            GxKind::DesignToken => "DesignToken",
        }
    }

    /// Whether **this build** has an encoder and a decoder for it.
    ///
    /// The whole of phase 1's honesty in one predicate: the registry is twelve and the shipped set
    /// is one, and a caller can tell which it is holding without reading a changelog.
    ///
    /// 🔴 Since R-930-B1 the registry is thirteen and the shipped set is two. The sentence above is
    /// left standing because the predicate it describes has not changed — only its answer has — and
    /// because the ratio was never the point: what a caller needs is to be told which side of the
    /// line the kind in its hand is on. A kind may only cross that line if
    /// [`GxKind::body_witness`] answers for it (`req/939` §2-C), which
    /// `tests/r939_kind_binding.rs` asserts over the whole registry in both directions.
    #[must_use]
    pub const fn is_shipped(self) -> bool {
        matches!(self, GxKind::Receipt | GxKind::DesignToken)
    }

    /// 🔴 **R-939-1** — how a file of this kind names its kind inside the bytes its identity
    /// covers, or `None` when this build reads no file of that kind and so binds nothing.
    ///
    /// `None` is a statement about **this build**, not about the kind: [`crate::dsse`] already
    /// mints payload types for five kinds, and each successor lane names its own here as it ships.
    /// What must never happen is the other order — a kind that ships without an answer here is the
    /// C-1 defect returning, so the two lists are asserted equal rather than merely compatible.
    ///
    /// Exhaustive on purpose: a fourteenth kind stops the build here, and the decision it forces is
    /// exactly the one that must not be skipped.
    #[must_use]
    pub const fn body_witness(self) -> Option<KindWitness> {
        match self {
            GxKind::Receipt => Some(KindWitness::PayloadType(RECEIPT_PAYLOAD_TYPE)),
            GxKind::DesignToken => Some(KindWitness::InBandTag(DESIGN_TOKEN_TAG)),
            GxKind::Candidate
            | GxKind::Transformation
            | GxKind::LedgerProof
            | GxKind::ConsistencyProof
            | GxKind::Checkpoint
            | GxKind::VerdictCheckpoint
            | GxKind::EscalationTicket
            | GxKind::AttachSource
            | GxKind::EngineJournal
            | GxKind::LedgerLeaf
            | GxKind::EscrowedInverse => None,
        }
    }
}

/// 🔴 **R-939-1** — the two spellings by which a body names its own kind.
///
/// Both were already in the workspace before this enum named them. A signed kind is witnessed by
/// the payload type inside its pre-authentication encoding, which is where [`crate::dsse`] puts it
/// so that a signature over one load cannot be replayed as a signature over another. A kind with no
/// signer has no signed bytes to hide a witness in, so the witness is a member of the canonical
/// body itself — the same separation `gx_log::tile`'s domain byte gives a leaf, obtained with a
/// member instead of a prefix (`req/939` §2-D).
///
/// The distinction is kept in the type rather than folded into one string because the two are read
/// from different places, and a reader that could not tell them apart would look for a payload type
/// in a document that has no envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum KindWitness {
    /// The DSSE `payloadType`, inside the pre-authentication encoding the signature covers.
    PayloadType(&'static str),
    /// A member of the canonical body, inside the bytes the identity covers.
    InBandTag(&'static str),
}

impl KindWitness {
    /// The string a body of this kind must carry, whichever place carries it.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            KindWitness::PayloadType(value) | KindWitness::InBandTag(value) => value,
        }
    }
}

impl fmt::Display for GxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a `.gx` file was not admitted.
///
/// Every variant is a *stated reason*: a reader is told which condition refused, because "this
/// file is bad" and "this file names a kind I have never heard of" send an operator to two
/// different places.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The bytes do not open as a `.gx` file at all — too short for a header, or the wrong magic.
    NotGxObjectFile {
        /// What was seen, in the shape a reader can act on.
        detail: String,
    },
    /// The wrapper declares a format version this build does not read. Fail-closed on purpose:
    /// a newer wrapper may carry fields in places this build would misread.
    FormatVersion {
        /// The declared version.
        found: u16,
    },
    /// The `kind` number is in no registry entry.
    UnknownKind {
        /// The number as written.
        code: u16,
    },
    /// The kind is registered and this build has no codec for it (`req/922` §5: F2 phase 1 ships
    /// `Receipt` alone). Distinct from [`Refusal::UnknownKind`] because the two are answered
    /// differently — one waits for a later build, the other is not a gx object at all.
    KindNotShipped {
        /// The registered kind.
        kind: GxKind,
    },
    /// The wrapped document does not decode.
    Body {
        /// The decoder's own words.
        detail: String,
    },
    /// The `kind` in the header and the payload type inside the envelope disagree. A file whose
    /// wrapper says one thing and whose body says another is refused rather than resolved in
    /// either direction — the proposed chapter §3.16.1's "naming trap" made mechanical.
    PayloadType {
        /// What the header's kind requires.
        expected: &'static str,
        /// What the envelope carries.
        found: String,
    },
    /// The body is not canonical DAG-CBOR, so it has no identity to compare against.
    BodyNotCanonical {
        /// gx-canon's own words.
        detail: String,
    },
    /// 🔴 **R-939-1** — the header's kind and the name inside the body's own bytes disagree.
    ///
    /// A sibling of [`Refusal::PayloadType`] and deliberately not the same sentence: that one is
    /// about an envelope carried beside the body, this one about a member inside the bytes the
    /// identity covers. A file that reaches this refusal has a **correct** identity — it was
    /// re-encoded and re-digested after the tag was changed — which is exactly why the check earns
    /// its place: nothing else in this reader would have noticed.
    KindTag {
        /// What the header's kind requires the body to say.
        expected: &'static str,
        /// What the body says.
        found: String,
    },
    /// The stored identity claim and the recomputed identity disagree (`req/922` §7-3).
    IdentityMismatch {
        /// What the file says it is.
        claimed: String,
        /// What its body actually digests to.
        recomputed: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotGxObjectFile { detail } => {
                write!(f, "not a gx object file: {detail}")
            }
            Refusal::FormatVersion { found } => write!(
                f,
                "the file declares object-format version {found} and this build reads \
                 {FORMAT_VERSION}"
            ),
            Refusal::UnknownKind { code } => write!(
                f,
                "kind {code} is in no entry of this build's registry of {}, so the file is \
                 refused rather than read as something it may not be",
                GxKind::REGISTRY.len()
            ),
            Refusal::KindNotShipped { kind } => write!(
                f,
                "kind {kind} is registered and this build has no codec for it; the only kind it \
                 reads and writes is {}",
                GxKind::Receipt
            ),
            Refusal::Body { detail } => write!(f, "the wrapped document does not decode: {detail}"),
            Refusal::PayloadType { expected, found } => write!(
                f,
                "the header names a kind whose payload type is {expected} and the envelope \
                 carries {found}"
            ),
            Refusal::BodyNotCanonical { detail } => {
                write!(f, "the body is not canonical DAG-CBOR: {detail}")
            }
            Refusal::KindTag { expected, found } => write!(
                f,
                "the header names a kind whose body must carry {expected} and the body carries \
                 {found}; the identity recomputes either way, so this is the only place the two \
                 could be told apart"
            ),
            Refusal::IdentityMismatch {
                claimed,
                recomputed,
            } => write!(
                f,
                "the file claims identity {claimed} and its body digests to {recomputed}; the \
                 stored value is a claim and the recomputed one decides"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// The document a `.gx` file carries, tagged by the kind that decided how to read it.
///
/// 🔴 The upgrade the struct below predicted, taken when the second kind shipped (**R-930-B1**):
/// "when a second kind ships this becomes a kind-tagged body enum and this struct keeps its name —
/// the upgrade is one field, not one format". It was one field.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GxBody {
    /// 42 §3.10's signed receipt, in the shape `gx receipt verify` already takes.
    Receipt(Receipt),
    /// A design-token document ([`crate::design_token`]). Carries no signature, and says so.
    DesignToken(DesignToken),
}

impl GxBody {
    /// Which registry entry this body belongs to.
    ///
    /// Derived from the value rather than carried beside it, so the two cannot come apart — which
    /// is the same reason [`GxObjectFile::cid`] is the recomputed identity and not the stored one.
    #[must_use]
    pub const fn kind(&self) -> GxKind {
        match self {
            GxBody::Receipt(_) => GxKind::Receipt,
            GxBody::DesignToken(_) => GxKind::DesignToken,
        }
    }
}

/// A `.gx` file that has been read, and whose identity claim has been checked against its body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GxObjectFile {
    /// The wrapper version the file declared (always [`FORMAT_VERSION`] today; carried rather
    /// than assumed so a reader can print what it read).
    pub format_version: u16,
    /// Which registry entry the file names.
    pub kind: GxKind,
    /// The **recomputed** identity, never the stored one. The stored bytes are compared against
    /// this and then dropped, so nothing downstream can accidentally use the claim.
    pub cid: Cid,
    /// The document itself.
    pub body: GxBody,
}

impl GxObjectFile {
    /// The receipt, when this file holds one.
    ///
    /// An `Option` and not a panic: "this file is not a receipt" is an ordinary answer now that
    /// more than one kind ships, and a caller that wants to insist can say so at its own call site.
    #[must_use]
    pub fn receipt(&self) -> Option<&Receipt> {
        match &self.body {
            GxBody::Receipt(receipt) => Some(receipt),
            GxBody::DesignToken(_) => None,
        }
    }

    /// The design-token document, when this file holds one.
    #[must_use]
    pub fn design_token(&self) -> Option<&DesignToken> {
        match &self.body {
            GxBody::DesignToken(token) => Some(token),
            GxBody::Receipt(_) => None,
        }
    }
}

/// `BLAKE3(enc(body))` over bytes that are audited to 42 §2.1 on the way past.
///
/// One line, and it is `gx_canon`'s: 41 §6 gives the canonical encode one door, and a second
/// digest road here would be the bypass AC-014 exists to prevent.
///
/// # Errors
/// [`Refusal::BodyNotCanonical`] when the bytes are not canonical DAG-CBOR.
pub fn body_cid(body: &[u8]) -> Result<Cid, Refusal> {
    cid::of_canonical_bytes(body).map_err(|e| Refusal::BodyNotCanonical {
        detail: e.to_string(),
    })
}

/// Write a signed receipt as a `.gx` file.
///
/// The receipt is not re-encoded: `envelope.payload` is carried through byte for byte, which is
/// what makes the round trip an identity rather than a re-derivation (`req/38` §324 ruling 3 —
/// asking what *this build's* schema would have written is the question three lanes were sent back
/// over).
///
/// # Errors
/// [`Refusal::BodyNotCanonical`] when the receipt's signed bytes are not canonical DAG-CBOR — a
/// document that could not have been issued by this workspace, refused at the door rather than
/// exported with an identity nobody can recompute.
pub fn write_receipt(receipt: &Receipt) -> Result<Vec<u8>, Refusal> {
    let cid = body_cid(&receipt.envelope.payload)?;
    let document = serde_json::to_vec(receipt).map_err(|e| Refusal::Body {
        detail: e.to_string(),
    })?;
    Ok(frame(GxKind::Receipt, &cid, &document))
}

/// `magic || format_version || kind || claimed identity || document`, in one place.
///
/// One writer per kind and one header for all of them: the header is the part every reader parses
/// before it knows what it is holding, so a second hand-rolled copy of these five lines is a second
/// chance to write a different one.
fn frame(kind: GxKind, cid: &Cid, document: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + document.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&kind.code().to_be_bytes());
    out.extend_from_slice(&cid.0);
    out.extend_from_slice(document);
    out
}

/// 🔴 The predicate a design-token document is held to, on the way **out** and on the way **in**.
///
/// A writer that could produce a document its own reader refuses would put files into the world
/// that this build cannot read back, so both directions run this. One question, one function
/// (`req/38` §227): the same question spelled in two places is the same question answered two ways.
///
/// # Errors
/// [`Refusal::KindTag`] when the body does not name its own kind (rule 4), [`Refusal::Body`] for
/// every structural refusal [`DesignToken::check`] raises.
fn check_design_token(token: &DesignToken) -> Result<(), Refusal> {
    let Some(witness) = GxKind::DesignToken.body_witness() else {
        // Unreachable while R-939-1 holds. A refusal rather than a panic, so that a build which
        // broke the rule turns files away instead of aborting on them.
        return Err(Refusal::KindNotShipped {
            kind: GxKind::DesignToken,
        });
    };
    if token.gx_kind != witness.value() {
        return Err(Refusal::KindTag {
            expected: witness.value(),
            found: token.gx_kind.clone(),
        });
    }
    token.check().map_err(|refusal| Refusal::Body {
        detail: refusal.to_string(),
    })
}

/// Write a design-token document as a `.gx` file.
///
/// Unlike [`write_receipt`], the body **is** the canonical encoding rather than a JSON envelope
/// around it. There is no signature to keep beside it, and wrapping canonical bytes in a second
/// spelling would put a second canonical layer outside nothing — rule 1 read for a kind that has no
/// signer.
///
/// # Errors
/// Whatever [`check_design_token`] refuses, and [`Refusal::BodyNotCanonical`] when the value has no
/// canonical form.
pub fn write_design_token(token: &DesignToken) -> Result<Vec<u8>, Refusal> {
    check_design_token(token)?;
    let document = cbor::encode(token).map_err(|e| Refusal::BodyNotCanonical {
        detail: e.to_string(),
    })?;
    let cid = body_cid(&document)?;
    Ok(frame(GxKind::DesignToken, &cid, &document))
}

/// Read a `.gx` file, refusing anything this build cannot admit.
///
/// The order is the requirement: the wrapper is checked before the body is parsed, so a file
/// naming a kind this build does not verify is refused **without** its contents being decoded, and
/// the identity is recomputed before the value is handed to a caller.
///
/// # Errors
/// One [`Refusal`] per condition, in the order above.
pub fn read(bytes: &[u8]) -> Result<GxObjectFile, Refusal> {
    if bytes.len() < HEADER_LEN {
        return Err(Refusal::NotGxObjectFile {
            detail: format!(
                "the file is {} byte(s) and a header is {HEADER_LEN}",
                bytes.len()
            ),
        });
    }
    if bytes[..2] != MAGIC {
        return Err(Refusal::NotGxObjectFile {
            detail: format!(
                "the first two bytes are {:#04x} {:#04x} and a gx object file opens with {:?}",
                bytes[0],
                bytes[1],
                core::str::from_utf8(&MAGIC).unwrap_or("gx")
            ),
        });
    }
    let format_version = u16::from_be_bytes([bytes[2], bytes[3]]);
    if format_version != FORMAT_VERSION {
        return Err(Refusal::FormatVersion {
            found: format_version,
        });
    }
    let code = u16::from_be_bytes([bytes[4], bytes[5]]);
    let kind = GxKind::from_code(code).ok_or(Refusal::UnknownKind { code })?;
    if !kind.is_shipped() {
        return Err(Refusal::KindNotShipped { kind });
    }
    let mut claimed = [0u8; 32];
    claimed.copy_from_slice(&bytes[6..HEADER_LEN]);
    let document = &bytes[HEADER_LEN..];

    // 🔴 One arm per shipped kind, and the kind decides both how the document is spelled and where
    // the identity is taken from. The last arm cannot be reached while `is_shipped` and this match
    // agree; it is a refusal rather than an `unreachable!` because a build in which they disagreed
    // should turn a file away, not abort on it.
    let (body, recomputed) = match kind {
        GxKind::Receipt => {
            let receipt: Receipt = serde_json::from_slice(document).map_err(|e| Refusal::Body {
                detail: e.to_string(),
            })?;
            if receipt.envelope.payload_type != RECEIPT_PAYLOAD_TYPE {
                return Err(Refusal::PayloadType {
                    expected: RECEIPT_PAYLOAD_TYPE,
                    found: receipt.envelope.payload_type.clone(),
                });
            }
            // The identity is over the signed bytes inside the envelope, not over the envelope.
            let cid = body_cid(&receipt.envelope.payload)?;
            (GxBody::Receipt(receipt), cid)
        }
        GxKind::DesignToken => {
            let token: DesignToken = cbor::decode(document).map_err(|e| Refusal::Body {
                detail: e.to_string(),
            })?;
            check_design_token(&token)?;
            // The whole document is the canonical body for this kind, so the identity is over it.
            let cid = body_cid(document)?;
            (GxBody::DesignToken(token), cid)
        }
        kind => return Err(Refusal::KindNotShipped { kind }),
    };

    if recomputed.0 != claimed {
        return Err(Refusal::IdentityMismatch {
            claimed: Cid(claimed).to_text(),
            recomputed: recomputed.to_text(),
        });
    }
    Ok(GxObjectFile {
        format_version,
        kind,
        cid: recomputed,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry's numbers, and the two it deliberately leaves unclaimed.
    ///
    /// 🔴 Rewritten from `the_registry_codes_are_one_to_twelve_in_order`. A test may only be
    /// changed against a named and dated ruling, and this is the reference: `req/939` §2-E of
    /// 2026-08-30 appends `DesignToken` at **15**, on the Owner instruction of the same day, and
    /// `req/930` §6-12 is why 13 and 14 are held. What the old name asserted — that the codes are
    /// the positions — was true of a registry of twelve and is what appending out of sequence gives
    /// up; the property that actually protects a stranger's file is that **no number moves**, and
    /// that is what is asserted below, entry by entry, against a written-out list rather than
    /// against an index.
    #[test]
    fn the_registry_numbers_its_entries_and_leaves_thirteen_and_fourteen_unclaimed() {
        let numbers = [1u16, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15];
        assert_eq!(GxKind::REGISTRY.len(), numbers.len());
        for (kind, code) in GxKind::REGISTRY.into_iter().zip(numbers) {
            assert_eq!(kind.code(), code, "{kind} moved");
            assert_eq!(GxKind::from_code(code), Some(kind));
        }
        assert_eq!(GxKind::from_code(0), None, "0 is reserved");
        for unclaimed in [13u16, 14, 16] {
            assert_eq!(
                GxKind::from_code(unclaimed),
                None,
                "{unclaimed} is not this build's to answer for"
            );
        }
    }

    /// Which kinds ship, and the rule that decides whether one may.
    ///
    /// 🔴 Rewritten from `exactly_one_registered_kind_is_shipped` under the same ruling
    /// (`req/939` §2-C, 2026-08-30). The old assertion named a **count**; this one names the
    /// **set**, and adds the predicate the count was standing in for: a kind may ship only if it
    /// names itself inside the bytes its identity covers. Asserted as an equality in both
    /// directions, because C-1 returns the moment a kind ships without a witness.
    #[test]
    fn the_shipped_set_is_named_and_every_member_of_it_names_itself_in_its_body() {
        let shipped: Vec<GxKind> = GxKind::REGISTRY
            .into_iter()
            .filter(|k| k.is_shipped())
            .collect();
        assert_eq!(shipped, vec![GxKind::Receipt, GxKind::DesignToken]);

        for kind in GxKind::REGISTRY {
            assert_eq!(
                kind.is_shipped(),
                kind.body_witness().is_some(),
                "{kind}: shipping and naming itself have come apart (R-939-1)"
            );
        }
    }
}
