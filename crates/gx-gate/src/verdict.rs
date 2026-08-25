// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What a gate answers, and the record it keeps of how it got there (42 §3.8).
//!
//! Spec: 41 §4 for `Verdict`'s three arms, 42 §3.8 for the field tables, 42 §1.3 for what of them
//! reaches a CID. Rulings: **E-M3-3** (the error path), **E-M3-8** (`ProofRef` is gx-core's),
//! **E-M3-9** (the order of the vectors), **E-M3-10** (which "composition" this file is about). (sem: SEM-gx-gate-138)
//!
//! # This hand builds the shapes and none of the evaluation
//!
//! req/60 §5.2 gives hand 1 "types and constructors only" (sem: SEM-gx-gate-139): Cedar evaluation is hand 2 (`policy.rs`), the
//! invariant registry is hand 3 (`invariant.rs`), and composing the two into a `Verdict` --
//! AC-027's Deny-precedence over the four quadrants -- is hand 4. So nothing here decides
//! anything. What is here is the vocabulary those hands will fill, plus the one invariant that
//! belongs to the *shape* rather than to the evaluation: the canonical order of E-M3-9.
//!
//! # Two things called "composition" (E-M3-10) (sem: SEM-gx-gate-140)
//!
//! [`AdmitProof::composed_from`] is the composition of **transformations** (T5's witness chain, 42
//! §3.8), and AC-027's Deny-precedence is the meet of the two **evaluation systems**. 51 §3 wrote
//! one row that mixed them, and req/38 §19's M3-22 separates them: `admissible_composition` is T1
//! and carries no AC, AC-027 belongs to a `verdict_meet` suite (hand 4). Naming them apart here is
//! what stops hand 4 from writing the wrong property.
//!
//! # Hand 4: the two doors, and why they are the only ones
//!
//! Three rulings land in this file at once and they are one design: a value that reaches a
//! receipt's CID may not be assembled field by field.
//!
//! * **E-M3-6 / M3-08** gives the crate a code vocabulary, declared in [`crate::REASON_CODES`] and
//!   refused at construction by [`Reason::new`].
//! * **M3-21 / ASM-60-4** bounds `Reason.message` and `InvariantResult.detail` at
//!   [`crate::MAX_MESSAGE_BYTES`], and sends anything longer to a digest instead of into the bytes
//!   a receipt is identified by.
//! * **E-M3-12** puts a `Deny`'s reasons in one order, so that two evaluations that refused for
//!   the same reasons produce the same `proof_digest` (M2-10) whatever order the two systems were
//!   consulted in.
//!
//! None of the three can be enforced by a public field, so [`Reason`] and [`InvariantResult`]
//! carry private ones with a fallible constructor each -- the shape [`AdmitProof`] has had since
//! hand 1 -- and neither derives `Deserialize`, which would be a second door that skips all three.

use gx_canon::cid::{self, IdentityView};
use gx_core::{Cid, ProofRef, TransformationId, VerdictKind};
use gx_witness::PolicyDecision;
use serde::{Deserialize, Serialize};

use crate::policy::EscalationTicket;
use crate::{Error, Result, MAX_MESSAGE_BYTES, REASON_CODES};

/// 41 §4 verbatim: `Verdict { Admit(AdmitProof), Deny(Vec<Reason>), Escalate(EscalationTicket) }`. (sem: SEM-gx-gate-141)
///
/// Three arms and no fourth. **Evaluation that could not be performed is not an arm** -- that is
/// `Err` from [`crate::Gate::verify`], and E-M3-3 is explicit about why: "unevaluable (⊥) is neither
/// Deny nor Escalate -- naming 'refused' and 'broken' with the same verdict muddies the semantics of
/// the decision" (sem: SEM-gx-gate-142). A
/// caller that treats an `Err` as a `Deny` has made a policy decision the engine makes for it
/// (43 T-4d: fail-closed lives up there, not in here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The transformation is admissible, with the record of what said so.
    Admit(AdmitProof),
    /// It is not, with at least one reason. An empty `Vec` is a `Deny` that says nothing and is
    /// refused at construction by [`Verdict::deny`].
    Deny(Vec<Reason>),
    /// Nobody in this process can answer; a human is asked (43 T-5).
    Escalate(EscalationTicket),
}

impl Verdict {
    /// Which of the three this is, in the vocabulary a receipt records (**E-M3-2**).
    ///
    /// This is the whole reason `VerdictKind` sits in gx-core: `gx-witness` types
    /// `VerdictSummary.kind` with it without naming this crate, and this method is the bridge the
    /// engine will walk in M5. A `match` written at the call site instead would be a second
    /// definition of the mapping.
    #[must_use]
    pub const fn kind(&self) -> VerdictKind {
        match self {
            Verdict::Admit(_) => VerdictKind::Admit,
            Verdict::Deny(_) => VerdictKind::Deny,
            Verdict::Escalate(_) => VerdictKind::Escalate,
        }
    }

    /// A `Deny` that carries at least one reason, in **E-M3-12**'s canonical order.
    ///
    /// # The order, and why a refusal has one
    ///
    /// req/38 §22's D-1 ruling:
    ///
    /// > "D-1 (directional ruling = option a, implemented in hand 4): `Verdict::Deny(Vec<Reason>)`'s
    /// > canonical order **totally orders every Reason by (source kind, id, code), lexicographic**.
    /// > Since M2-10 makes a Deny's `proof_digest` the CID of the Vec, order is part of identity --
    /// > leaving a meaningless order inside identity is a road to 'the decision is fixed, the record
    /// > is mutable'" -> **E-M3-12** (sem: SEM-gx-gate-143)
    ///
    /// Hand 3 concatenated the policy reasons before the invariant ones, which is a fact about
    /// which system [`crate::Gate::verify`] asks first rather than about the refusal. Sorting is
    /// what makes [`Verdict::proof_digest`] a function of *what was refused*.
    ///
    /// The ruled key is `(source kind, id, code)` and this constructor adds `message` as a final
    /// tie-break, because the ruled triple is not injective over values: two reasons that agree on
    /// all three and differ in their message would keep the order they arrived in, and arrival
    /// order inside the CID is the defect the ruling exists to remove. The addition orders only
    /// within an equivalence class the ruling leaves unordered; it is reported in req/64 §4 rather
    /// than treated as settled.
    ///
    /// Duplicates are kept, not folded (H4-3: "multiplicity is not folded; only order is canonicalized"). (sem: SEM-gx-gate-144)
    ///
    /// # Errors
    /// [`crate::Error::EmptyDeny`] if `reasons` is empty. 42 §3.8 gives `Deny` a `Vec<Reason>` and
    /// 44 §2.3 expects the API to answer with a code and a message; a refusal that named nothing
    /// would be a verdict no operator could act on, and M2-10 (req/49 §3) makes the vector the
    /// thing `proof_digest` is taken over -- a digest of an empty list identifies nothing.
    pub fn deny(mut reasons: Vec<Reason>) -> Result<Self> {
        if reasons.is_empty() {
            return Err(Error::EmptyDeny);
        }
        reasons.sort_by(Reason::canonical_cmp);
        Ok(Verdict::Deny(reasons))
    }

    /// What a receipt records this verdict as (42 §3.10's `VerdictSummary.proof_digest`).
    ///
    /// # The rule M2-10 asked for, as a function (req/49 §3)
    ///
    /// > "**M2-10**: the source of Deny/Escalate's `proof_digest` is undefined. 42 §3.10 gives
    /// > `verdict: VerdictSummary { kind, proof_digest: Cid }` but `AdmitProof` is Admit-only ...
    /// > spell out the rule for taking Deny=`Vec<Reason>`'s and Escalate=`EscalationTicket`'s CID
    /// > (the type itself is M3)" (sem: SEM-gx-gate-145)
    ///
    /// So each arm digests **the payload it carries**, and nothing else:
    ///
    /// | arm | what is digested | projection |
    /// |---|---|---|
    /// | `Admit` | [`AdmitProof`] | 42 §1.3: "every field" | (sem: SEM-gx-gate-146)
    /// | `Deny` | the `Vec<Reason>` | every reason, in E-M3-12's order |
    /// | `Escalate` | the [`EscalationTicket`] | 42 §1.3: `{transformation, reasons, required_approval}` |
    ///
    /// gx-witness carries the value and refuses a receipt without one (`VerdictSummary.proof_digest`
    /// is not an `Option`); what it could not do in M2 is *produce* one, because all three types are
    /// this crate's. `crates/gx-gate/tests/proof_digest.rs` is where the two halves are put back
    /// together.
    ///
    /// The digest is [`gx_canon::cid::compute`] -- BLAKE3-256 over the canonical DAG-CBOR of the
    /// projection (42 §1.1) -- so it is the same kind of value as every other `Cid` in the system
    /// and not a second hashing scheme (41 §6, AC-014).
    ///
    /// # Errors
    /// [`crate::Error::NotDigestible`] when gx-canon has no canonical form for the projection.
    pub fn proof_digest(&self) -> Result<Cid> {
        match self {
            Verdict::Admit(proof) => digest_of(proof),
            Verdict::Deny(reasons) => digest_of(&DenyReasons(reasons)),
            Verdict::Escalate(ticket) => digest_of(ticket),
        }
    }
}

/// The one place this crate asks gx-canon for a digest.
///
/// # Errors
/// [`crate::Error::NotDigestible`], carrying gx-canon's own words about why the projection has no
/// canonical form. A value that cannot be encoded has no identity, and saying so is better than
/// hashing an approximation of it (`req/26` §3, quoted by [`gx_canon::cid::compute`]).
pub(crate) fn digest_of<T: IdentityView + ?Sized>(value: &T) -> Result<Cid> {
    cid::compute(value).map_err(|e| Error::NotDigestible {
        detail: e.to_string(),
    })
}

/// The `Deny` arm's payload, as something with an identity (M2-10).
///
/// A newtype rather than an impl on `Vec<Reason>`: the projection is "the reasons themselves" and (sem: SEM-gx-gate-147)
/// wrapping them says so at the call site, where `digest_of(&DenyReasons(reasons))` reads as the
/// rule M2-10 wrote.
pub struct DenyReasons<'r>(pub &'r [Reason]);

impl IdentityView for DenyReasons<'_> {
    type View<'a>
        = &'a [Reason]
    where
        Self: 'a;

    fn identity_view(&self) -> Self::View<'_> {
        self.0
    }
}

/// The record behind an `Admit` (42 §3.8).
///
/// # Why the fields are private
///
/// 42 §1.3 puts `AdmitProof` in the IdentityView table as "every field" (sem: SEM-gx-gate-148), so **the order of every
/// vector in here reaches the CID**. 42 fixes no order, and req/38 §19's M3-20 ruling does:
///
/// > "Vec canonicalization = ascending id (policy_id/invariant_id lexicographic; evidence_digests
/// > ascending CID byte order). **Only `composed_from` keeps composition order** (order carries
/// > meaning for T5's verdict-chain reconstruction -- it must not be folded into ascending)" ->
/// > **E-M3-9** (sem: SEM-gx-gate-149)
///
/// A rule that reorders on construction cannot be enforced by a public field, so the fields are
/// private and [`AdmitProof::new`] is the only road in. This is the same trade `ObjectSnapshot`
/// makes in gx-core. The test that the road cannot be walked around is hand 4's, per req/60 §5.2
/// ("the implementation is enforced by the constructor, tested in hand 4") (sem: SEM-gx-gate-150); what hand 1 owes is that there *is* one road.
///
/// # Why no `Serialize`
///
/// An `AdmitProof` reaches a CID through `IdentityView` in gx-canon, and gx-gate does not name
/// gx-canon yet (nothing in this hand mints a digest). Deriving `Deserialize` in particular would
/// open a second door into the struct that skips [`AdmitProof::new`] and therefore skips E-M3-9 --
/// the exact hole the private fields are here to close. Hand 4 adds the wire face together with
/// the canonicalisation it has to preserve.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AdmitProof {
    policy_decisions: Vec<PolicyDecisionRecord>,
    invariant_results: Vec<InvariantResult>,
    evidence_digests: Vec<Cid>,
    composed_from: Vec<TransformationId>,
    proof_ref: Option<ProofRef>,
}

impl AdmitProof {
    /// Build one, putting the three unordered vectors into E-M3-9's canonical order.
    ///
    /// `policy_decisions` sort by `policy_id`, `invariant_results` by `invariant_id`,
    /// `evidence_digests` by their thirty-two bytes. `composed_from` is **not** sorted: it is the
    /// order the transformations were composed in, and T5 asks for the chain back.
    ///
    /// Duplicates are kept rather than folded away. H4-3 settled that shape for `Provenance`
    /// ("multiplicity is not folded; only order is canonicalized") (sem: SEM-gx-gate-151) and the reason carries: two policies that both
    /// answered, or one evidence item cited twice, are facts about the evaluation, and a
    /// constructor that deleted them would make the proof describe a different evaluation from the
    /// one that happened.
    #[must_use]
    pub fn new(
        mut policy_decisions: Vec<PolicyDecisionRecord>,
        mut invariant_results: Vec<InvariantResult>,
        mut evidence_digests: Vec<Cid>,
        composed_from: Vec<TransformationId>,
        proof_ref: Option<ProofRef>,
    ) -> Self {
        // Stable sorts, all three. Equal keys keep the order they arrived in, which is what makes
        // the duplicate case below well defined rather than dependent on the sort's internals.
        policy_decisions.sort_by(|a, b| a.policy_id.cmp(&b.policy_id));
        invariant_results.sort_by(|a, b| a.invariant_id.cmp(&b.invariant_id));
        evidence_digests.sort_by_key(|c| c.0);
        Self {
            policy_decisions,
            invariant_results,
            evidence_digests,
            composed_from,
            proof_ref,
        }
    }

    /// Cedar's answers, in `policy_id` order (E-M3-9).
    #[must_use]
    pub fn policy_decisions(&self) -> &[PolicyDecisionRecord] {
        &self.policy_decisions
    }

    /// The registry's answers, in `invariant_id` order (E-M3-9).
    #[must_use]
    pub fn invariant_results(&self) -> &[InvariantResult] {
        &self.invariant_results
    }

    /// The evidence the decision rested on, by CID and in byte order (42 §3.8: "a reference, not
    /// the body itself. T4 grounds"). (sem: SEM-gx-gate-152)
    #[must_use]
    pub fn evidence_digests(&self) -> &[Cid] {
        &self.evidence_digests
    }

    /// The lower proofs this one was composed from, **in composition order** (T5, E-M3-9).
    #[must_use]
    pub fn composed_from(&self) -> &[TransformationId] {
        &self.composed_from
    }

    /// The Lean correspondence, if there is one (42 §3.8; the type is gx-core's, **E-M3-8**).
    ///
    /// 42 §0 files `ProofRef` under this crate's `verdict.rs`; E-M2-12 had already implemented it
    /// in gx-core, and E-M3-8 makes the table the erratum rather than growing a second type with
    /// the same name.
    #[must_use]
    pub fn proof_ref(&self) -> Option<&ProofRef> {
        self.proof_ref.as_ref()
    }
}

/// `AdmitProof`'s projection: 42 §1.3 lists it as "every field", so every one is here. (sem: SEM-gx-gate-153)
///
/// Borrowed, like gx-canon's own views, so that taking a digest copies nothing. The field names
/// are the struct's, which makes the map keys of the projection the keys a reader already knows
/// from 42 §3.8's table.
#[derive(Debug, Serialize)]
pub struct AdmitProofView<'a> {
    /// [`AdmitProof::policy_decisions`], identity-bearing.
    pub policy_decisions: &'a [PolicyDecisionRecord],
    /// [`AdmitProof::invariant_results`], identity-bearing.
    pub invariant_results: &'a [InvariantResult],
    /// [`AdmitProof::evidence_digests`], identity-bearing.
    pub evidence_digests: &'a [Cid],
    /// [`AdmitProof::composed_from`], identity-bearing and order-preserving (T5's composition
    /// order is part of what the proof says).
    pub composed_from: &'a [TransformationId],
    /// [`AdmitProof::proof_ref`], identity-bearing.
    pub proof_ref: Option<&'a ProofRef>,
}

impl IdentityView for AdmitProof {
    type View<'a>
        = AdmitProofView<'a>
    where
        Self: 'a;

    fn identity_view(&self) -> AdmitProofView<'_> {
        AdmitProofView {
            policy_decisions: &self.policy_decisions,
            invariant_results: &self.invariant_results,
            evidence_digests: &self.evidence_digests,
            composed_from: &self.composed_from,
            proof_ref: self.proof_ref.as_ref(),
        }
    }
}

/// One policy's answer (42 §3.8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionRecord {
    /// Cedar's policy id, as the `PolicySet` spells it.
    pub policy_id: String,
    /// `Allow | Deny`, gx-witness's type because 42 §3.7 already fixed it there and "the same
    /// vocabulary as `cedar_policy::Decision`" (sem: SEM-gx-gate-154) -- one vocabulary, defined once.
    pub decision: PolicyDecision,
    /// Cedar's diagnostics, by digest rather than inline (42 §3.8).
    pub diagnostics_digest: Option<Cid>,
}

/// One invariant's answer (42 §3.8).
///
/// The fields are private for **M3-21**: `detail` is written by a third party -- an
/// `InvariantCheck` a deployment registered -- and it reaches a receipt's CID through
/// `AdmitProof`'s IdentityView (42 §1.3). A bound that a caller could skip by building the struct
/// literally would not be a bound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvariantResult {
    invariant_id: String,
    holds: bool,
    detail: Option<String>,
}

impl InvariantResult {
    /// One answer, with `detail` held to [`crate::MAX_MESSAGE_BYTES`] (**M3-21**, ASM-60-4).
    ///
    /// A `detail` longer than the bound is **not** a refusal: an invariant that says too much has
    /// still decided, and turning verbosity into ⊥ would collide with E-M3-3's line between "refused"
    /// and "broken". It is replaced by [`elide`]'s one-line reference to the digest of what it
    /// said, which is 42 §3.8's own instruction ("a short human-readable message (long text gets
    /// digested)") made (sem: SEM-gx-gate-155)
    /// mechanical.
    ///
    /// # Errors
    /// [`crate::Error::NotDigestible`] if an over-long `detail` has no canonical form -- see
    /// [`elide`], which is the only path that can produce it.
    pub fn new(invariant_id: String, holds: bool, detail: Option<String>) -> Result<Self> {
        let detail = detail.map(elide).transpose()?;
        Ok(Self {
            invariant_id,
            holds,
            detail,
        })
    }

    /// Equal to `InvariantCheck::id()` (42 §3.8), which
    /// [`crate::InvariantRegistry::run_all`] enforces at run time.
    #[must_use]
    pub fn invariant_id(&self) -> &str {
        &self.invariant_id
    }

    /// Whether the invariant held.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.holds
    }

    /// 42 §3.8: "a short human-readable message (long text gets digested)", at most (sem: SEM-gx-gate-156)
    /// [`crate::MAX_MESSAGE_BYTES`] bytes.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// Why a `Deny` (42 §3.8).
///
/// # `code` is not a free string any more (**E-M3-6**, req/38 §19's M3-08)
///
/// 42 §3.8 types it `String` and says "align the vocabulary with 44's gx_code", and req/60 §3
/// measured that 44 §2.3's twelve codes contain no way to say "a policy refused this" (sem: SEM-gx-gate-157). The ruling makes three
/// names and a containment:
///
/// > "M3-08 (adopted): 3 new words in the gate's own vocabulary (`POLICY_DENIED`/`INVARIANT_VIOLATED`/
/// > `EVIDENCE_INSUFFICIENT`) plus the containment 'the gate's reason codes ⊇ the API's gx_code'. In
/// > hand 4's error-vocabulary window, **declare a crate's Error vocabulary table (the substance of
/// > what E-M2-23 filed) in one place; anything outside it is refused at construction**" (sem: SEM-gx-gate-158)
///
/// Three names were created; **two survive**. `EVIDENCE_INSUFFICIENT` was retired by **E-7** in M5
/// hand 2 (req/38 §37, M5-04 adopted (b)) (sem: SEM-gx-gate-159) after three milestones with no producer, and the containment is
/// unaffected -- it was never one of 44's words. See [`crate::EVIDENCE_INSUFFICIENT`].
///
/// [`crate::REASON_CODES`] is that one declaration and [`Reason::new`] is the refusal. The fields
/// are private because a public one is a road around the check, and there is no `Deserialize`
/// derive for the same reason -- the shape hand 1 gave [`AdmitProof`] for E-M3-9.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reason {
    code: String,
    message: String,
    source: ReasonSource,
}

impl Reason {
    /// One reason, with a code from the vocabulary and a message inside the bound.
    ///
    /// # Errors
    /// [`crate::Error::UnknownReasonCode`] when `code` is not in [`crate::REASON_CODES`]. That is
    /// the whole of "a code outside the declaration is refused at construction": a receipt that carried `POLCIY_DENIED` would be (sem: SEM-gx-gate-160)
    /// unreadable by anything matching on the vocabulary, and the misspelling would survive every
    /// test that only asks whether a refusal happened.
    ///
    /// [`crate::Error::NotDigestible`] when an over-long `message` cannot be digested (see
    /// [`elide`]).
    pub fn new(code: &str, message: String, source: ReasonSource) -> Result<Self> {
        if !REASON_CODES.contains(&code) {
            return Err(Error::UnknownReasonCode {
                code: code.to_string(),
            });
        }
        Ok(Self {
            code: code.to_string(),
            message: elide(message)?,
            source,
        })
    }

    /// 42 §3.8's "machine-readable error code", always one of [`crate::REASON_CODES`]. (sem: SEM-gx-gate-161)
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// 42 §3.8's "human-readable", at most [`crate::MAX_MESSAGE_BYTES`] bytes. (sem: SEM-gx-gate-162)
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Where the refusal came from.
    #[must_use]
    pub fn source(&self) -> &ReasonSource {
        &self.source
    }

    /// **E-M3-12**'s order: `(source kind, id, code)`, then `message` as the tie-break that makes
    /// it total over values.
    ///
    /// Not `Ord` on the type. An ordering is a claim about what a value *is* comparable by, and
    /// two reasons are not ranked -- one refusal is not worse than another. This is the canonical
    /// order a `Deny` is recorded in and nothing else, so it is a named function used in one place
    /// ([`Verdict::deny`]).
    pub(crate) fn canonical_cmp(a: &Reason, b: &Reason) -> core::cmp::Ordering {
        (a.source.rank(), a.source.id(), a.code.as_str(), a.message()).cmp(&(
            b.source.rank(),
            b.source.id(),
            b.code.as_str(),
            b.message(),
        ))
    }
}

/// 42 §3.8's "long text gets digested", with M3-21's threshold. (sem: SEM-gx-gate-163)
///
/// Under the bound the text is itself. Over it, the text is replaced by a line naming how many
/// bytes were elided and the CID of what they were, so the record still identifies the message
/// without carrying it into the bytes a receipt is hashed over. The digest is
/// [`gx_canon::cid::compute`] over the text as a value -- the same road as every other identity in
/// gx (41 §6) -- so an operator holding the original can check that this is the line it produces.
///
/// The bound is on **bytes**, as ASM-60-4 words it, and a multi-byte character is never cut in
/// half: the whole string is replaced or none of it is.
///
/// **Not for use outside the gate** (req/38 §23's E-14). It is public because the boundary has two
/// sides and a test that could only reach one of them would measure half of it -- "a test that
/// measures a boundary measures it from the public face" (sem: SEM-gx-gate-164) -- not because a caller elsewhere has a reason to elide text. The constructors
/// call it; everything a consumer of this crate needs happens on the way into a [`Reason`] or an
/// [`InvariantResult`].
///
/// # Errors
/// [`crate::Error::NotDigestible`] if gx-canon cannot encode the text. A `String` always has a
/// canonical DAG-CBOR form, so this arm is unreachable today; it is typed rather than unwrapped
/// because 41 §6 counts a panic as a bug.
pub fn elide(text: String) -> Result<String> {
    if text.len() <= MAX_MESSAGE_BYTES {
        return Ok(text);
    }
    let digest = digest_of(&ElidedText(&text))?;
    Ok(format!(
        "<elided: {} bytes over the {MAX_MESSAGE_BYTES}-byte bound of ASM-60-4, {}>",
        text.len(),
        cid::to_text(&digest)
    ))
}

/// A message too long to record, as something with an identity.
struct ElidedText<'t>(&'t str);

impl IdentityView for ElidedText<'_> {
    type View<'a>
        = &'a str
    where
        Self: 'a;

    fn identity_view(&self) -> Self::View<'_> {
        self.0
    }
}

/// Where a reason came from (42 §3.8 verbatim: `Policy{policy_id} | Invariant{invariant_id} | (sem: SEM-gx-gate-165)
/// Precondition | Adapter{detail}`), plus the one **E-M3-11** adds.
///
/// # The fifth variant (C-3, req/38 §21)
///
/// Cedar's third rule refuses a request that no policy satisfied, and "If the decision is Deny
/// because no policies were satisfied (rule 3), then the list of determining policies is empty" (sem: SEM-gx-gate-166)
/// (<https://docs.cedarpolicy.com/auth/authorization.html>). Hand 2 met that case and had no
/// truthful source to name: the refusal is not `Policy{..}` -- there is no policy -- and it is
/// certainly not `Precondition` or `Adapter`. It spelled a sentinel policy id
/// (`cedar:default-deny`) as an interim and raised the gap; the ruling closed it:
///
/// > "🔴C-3 (adopted = option a, erratum E-M3-11): **add a variant equivalent to
/// > `NoPolicyApplied`** to 42 §3.8's `ReasonSource` (an erratum for a spec defect) ... the
/// > implementation is replaced by a variant in hand 4 (the only window that touches the
/// > ReasonSource vocabulary), and the sentinel spelling is deleted at the same time" (sem: SEM-gx-gate-167)
///
/// Both halves are done here: the variant exists and the sentinel is gone from the crate. What the
/// sentinel cost while it existed was a policy id that a policy could *claim* -- hand 2 had to
/// refuse `@id("cedar:default-deny")` at parse time to stop a pack from impersonating the absence
/// of a decision. With a variant there is nothing to impersonate: a policy may now be called
/// anything at all, and `crates/gx-gate/tests/ac_025.rs` measures that the two cases stay
/// distinguishable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ReasonSource {
    /// A policy in the set refused (42 §3.8).
    Policy {
        /// Cedar's own id of the determining policy, as the `PolicySet` spells it.
        policy_id: String,
    },
    /// A registered invariant did not hold (42 §3.8).
    Invariant {
        /// The refusing check's `InvariantCheck::id()` (41 §4).
        invariant_id: String,
    },
    /// The CAS precondition failed: the state the plan was made against is not the state found
    /// (42 §3.8, CON-2's vocabulary).
    Precondition,
    /// The substrate adapter refused (42 §3.8).
    Adapter {
        /// The adapter's account of the refusal, in words.
        detail: String,
    },
    /// Cedar's rule 3: the request satisfied nothing, so no policy decided (**E-M3-11**).
    NoPolicyApplied,
}

impl ReasonSource {
    /// The first component of E-M3-12's key: which kind of source this is.
    ///
    /// 42 §3.8's declaration order, with E-M3-11's variant last -- the position it holds in the
    /// enum. A rank rather than a `derive(Ord)` so that the number the order depends on is written
    /// down where it can be read against the ruling.
    pub(crate) const fn rank(&self) -> u8 {
        match self {
            ReasonSource::Policy { .. } => 0,
            ReasonSource::Invariant { .. } => 1,
            ReasonSource::Precondition => 2,
            ReasonSource::Adapter { .. } => 3,
            ReasonSource::NoPolicyApplied => 4,
        }
    }

    /// The second component: the id the source names, or the empty string for the two variants
    /// that name nothing.
    ///
    /// Public because a reader of a refusal wants "which one" without matching on five variants, (sem: SEM-gx-gate-168)
    /// and because the ordering above is easier to check against E-M3-12 when the key it uses is
    /// one a test can read.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            ReasonSource::Policy { policy_id } => policy_id,
            ReasonSource::Invariant { invariant_id } => invariant_id,
            ReasonSource::Adapter { detail } => detail,
            ReasonSource::Precondition | ReasonSource::NoPolicyApplied => "",
        }
    }
}
