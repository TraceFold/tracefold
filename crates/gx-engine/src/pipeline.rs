// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The transitions: `submit` → `plan` → `verify` → `canonicalize` (43 §3, T-1 through T-8r).
//!
//! Spec: 43 §1 for the eleven states and §3 for the transition table this file is a transcription
//! of, 41 §5 for the commit protocol the four entry points here open, 32 FR-030..FR-033 for what
//! they must do, 34 AC-030..AC-033 for how that is judged. 41 §2 names this file, and **M5H1-5
//! adopted (a)** (req/38 §38; sem: SEM-gx-engine-095) settles that all eight entry points belong in it rather than in a module
//! split of this hand's invention.
//!
//! # Four of the eight, and the line between them
//!
//! | entry point | transitions | hand |
//! |---|---|---|
//! | [`Engine::submit`] | T-1 | **2** |
//! | [`Engine::plan`] | T-2 | **2** |
//! | [`Engine::verify`] | T-3, T-4a, T-4b, T-4c, T-4d, T-4e | **2** |
//! | [`Engine::canonicalize`] | T-8, T-8r | **2** |
//! | [`Engine::commit`] | T-9, T-10a, T-10b, T-10c, T-11 | **4** |
//! | `undo` | T-12 | 6 |
//! | `cancel` | T-7 | 6 |
//! | `escalation` | T-5, T-5b | 6 |
//!
//! The three that are not here are **absent, not stubbed**. `tests/engine_shape.rs` asserts both
//! halves, so a hand reaching forward into T-12 fails a probe rather than leaving a reviewer to
//! notice.
//!
//! # What the state is, and where it lives
//!
//! 42 §1.3-3: "the state lives in an external table on the engine side (the engine store), keyed
//! by `TransformationId`" (sem: SEM-gx-engine-096). So there is a table, and its key is a `TransformationId`.
//!
//! **A draft has no key.** 43 T-1 writes "the `TransformationId` is not yet fixed (delta/target
//! undecided)" (sem: SEM-gx-engine-096), and **M5-17 adopted (b)** settles what follows: "the Draft phase is held by the journal alone; the state table starts at Candidate".
//! There is therefore no draft table in this file. [`Engine::submit`] writes a journal record and
//! nothing else; [`Engine::plan`] is handed the same `Intent` again and re-derives its `IntentId`
//! rather than looking a body up. That is not an inconvenience worked around -- it is 42 §1.3-3 and
//! ASM-9 agreeing: the engine holds names and digests, not bodies.
//!
//! The one thing kept about drafts is a **set of the `IntentId`s the journal has seen**, and it is a
//! cache in the strict sense: [`Engine::open`] rebuilds it by replaying, and deleting it would cost
//! speed and no truth. req/78 §3.3's Rule 1 is the rule -- "`L` (the state table) is a function of
//! the journal. The in-memory table is a cache, and not the state" (sem: SEM-gx-engine-097).
//!
//! # Journal-first, in every transition
//!
//! 43 §7: every transition is journalled **before** its side effect. In this hand the side effects
//! are in-memory (the table), and the ordering is still what the code does: append, then mutate. It
//! matters more here than it looks, because hand 4's side effect is `adapter.apply` and the shape a
//! hand learns in the cheap case is the shape it writes in the expensive one.
//!
//! Two calls read the substrate before any of that: `adapter.snapshot` and `adapter.precondition`
//! in T-2, and `adapter.invert` in T-3. All three are reads. FR-035's "the engine itself must not
//! carry out changes to the substrate" (sem: SEM-gx-engine-098) is about writes, and hand 4 adds the **one** write there is:
//! `Engine::apply_once` (private), reached from [`Engine::commit`] and from nowhere else. FR-035 is not
//! "no apply" — it is "the engine does not do the changing itself" — and one call site behind a
//! CAS is the shape that makes the distinction measurable rather than asserted.
//!
//! # Three injection points, and why each one is a trait
//!
//! 41 §6: "randomness and clock are injected at the engine boundary" (sem: SEM-gx-engine-099), which req/78 §3.3's Rule 3 reads as the shape of the type.
//! The clock and the seed arrive as arguments to every entry point rather than as a `Clock` object,
//! because every transition already carries an `at` into its journal record and a second road to the
//! same value would be a second answer. What are traits are the three things a *test* has to be able
//! to replace:
//!
//! * [`EvidenceSource`] — **M5-03 adopted (a)** (sem: SEM-gx-engine-100). Its `Err` is the only producer of
//!   `AbortReason::VerifierUnavailable` in the workspace, which is what makes T-4d, T-4e and AC-036
//!   constructible; **E-M5-4** settles that "the one source of unreachability is the evidence collector" because
//!   gx-gate is a library and cannot be unreachable.
//! * [`Canonicalizer`] — AC-033 asks for "an abnormal case that injects a broken canon implementation returning an idempotence violation" (sem: SEM-gx-engine-100), so canon
//!   has to be replaceable. See the type for how 41 §6's "every canonical encode goes through gx-canon
//!   alone, no bypass" survives that.
//! * `SubstrateAdapter` — **M5-07 adopted (a)** (sem: SEM-gx-engine-100): the engine holds a registry and a caller registers into
//!   it, so gx-engine ships no adapter (N-13) and "the same engine, whatever the substrate" stays true of the
//!   artefact and not only of the prose.
//!
//! # What is deliberately not decided here
//!
//! 43 §8's waiting queue (a `Conflicts` transformation held at `Candidate`) is hand 5's, because it
//! needs the synchronisation hook §35 K-6 reserved for the engine layer. TTL (T-6) is hand 6's.
//! Neither is stubbed and neither is silently skipped: [`Engine::verify`] refuses a state it is not
//! written for rather than falling through it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use gx_core::{
    AbortReason, Actor, Cid, Commutation, CompositionMetadata, EnforcementMode, FailPosture,
    Fingerprint, GoalBytes, Intent, IntentId, ObjectId, ObjectSnapshot, PlannedDeltaBytes, Subject,
    SubstrateKind, Timestamp, Transformation, TransformationId, VerdictKind,
};
use gx_core::{
    BoundaryStage, DeterminismBoundary, FingerprintBytes, InclusionProof, Reversibility,
    VerdictCheckpoint, VerdictTally,
};
// 🔴 req/824 A5 — the observation road's vocabulary (the section at the end of this file).
use gx_core::{ChangeContext, EnvsetAdmission, ObservationClass, ObservationRecord, ReprKind};
use gx_gate::{AdmitProof, EscalationTicket, Gate, GateInput, Reason, TicketId, Verdict};
use gx_log::{proof::prove_inclusion, store::VerdictCheckpointStore, LedgerStore};
use gx_substrate::{
    elide_scope, AppliedDelta, InputStageDeclaration, InverseCompletion, InvertOutcome,
    PlannedDelta, SubstrateAdapter,
};
use gx_witness::receipt::{ReadSet, UndoAttestation, UndoDisposition};
use gx_witness::{
    Environment, Evidence, KeyPair, Provenance, ProvenanceInputs, Receipt, ReceiptKind,
    ReceiptPayload, VerdictSummary, CURRENT_PAYLOAD_VERSION,
};

use crate::replay::{reconstruct, CommittedRow, DraftRow, EscrowRow, Sigma, SigmaShadow, StateRow};
use crate::store::{
    BlobStore, EngineJournal, EngineJournalRecord, FingerprintRecord, InverseStatus,
    NotAttemptedBecause, ObservationStore, Rollback, SupersedeIndex,
};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// 43 §1 -- the eleven states
// ---------------------------------------------------------------------------

/// Where a transformation is in 43 §1's lifecycle.
///
/// Eleven values, and the enum is the whole vocabulary rather than the part this hand reaches:
/// `Committing`, `Committed` and `Superseded` are named here and written by hands 4 and 6. Naming
/// them now is what lets [`LIFECYCLE_STATES`] be compared against 43 §1's table today, which is the
/// check that would otherwise arrive after the states it was supposed to constrain.
///
/// `Draft` is in the vocabulary and **never in the table**. 43 §1 lists it and 42 §1.3-3 keys the
/// table on `TransformationId`, which a draft does not have (**M5-17 adopted (b)**; sem: SEM-gx-engine-101); the two facts sit
/// beside each other rather than being reconciled by dropping one, because dropping `Draft` would
/// make the enum disagree with 43 §1 and adding a draft key would make the table disagree with 42.
///
/// `Aborted` carries its reason, which is 43 §1's "must always carry an `AbortReason`" (sem: SEM-gx-engine-102) in the type: there is no
/// way to spell an abort without saying why.
/// `Serialize` because Σ holds one (**E-M5-2**): AC-039 compares the canonical bytes of the state
/// table, and a state written as a `String` beside a separate `Option<AbortReason>` could spell
/// "aborted, for no reason" (sem: SEM-gx-engine-103). The enum carries the reason where 43 §1 puts it, so the encoding
/// cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Lifecycle {
    /// T-1 has run. Journal only -- see the type documentation.
    Draft,
    /// T-2 has run: `PlannedDelta`, `Fingerprint₀` and `TransformationId` are fixed.
    Candidate,
    /// T-3 has run: evidence is being collected and the gate is being asked.
    Verifying,
    /// T-4a, T-4e, or hand 6's T-5.
    Admitted,
    /// T-4b, or hand 6's T-5b. Terminal unless `EnforcementMode::RecordOnly` opens T-8r.
    Denied,
    /// T-4c. Hand 6 resolves it.
    Escalated,
    /// T-8 or T-8r has run.
    Canonicalized,
    /// Hand 4's critical section (T-9 onward).
    Committing,
    /// Hand 4's terminal (T-11).
    Committed,
    /// Terminal, with the reason gx-core defines (ASM-15).
    Aborted(AbortReason),
    /// Hand 6's terminal (T-12).
    Superseded,
}

/// The eleven state names, declared once, in 43 §1's order.
///
/// The **E-M2-23 / A-10** shape this workspace uses everywhere: one declared list, one `name()`
/// written without a `_` arm, and `tests/lifecycle_states.rs` reading 43 §1's table out of the spec
/// file to compare against both. A twelfth state added without a row is a compile error at
/// [`Lifecycle::name`] and a failing probe at the table.
pub const LIFECYCLE_STATES: [&str; 11] = [
    "Draft",
    "Candidate",
    "Verifying",
    "Admitted",
    "Denied",
    "Escalated",
    "Canonicalized",
    "Committing",
    "Committed",
    "Aborted",
    "Superseded",
];

impl Lifecycle {
    /// Which of [`LIFECYCLE_STATES`] this is. No `_` arm.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Lifecycle::Draft => "Draft",
            Lifecycle::Candidate => "Candidate",
            Lifecycle::Verifying => "Verifying",
            Lifecycle::Admitted => "Admitted",
            Lifecycle::Denied => "Denied",
            Lifecycle::Escalated => "Escalated",
            Lifecycle::Canonicalized => "Canonicalized",
            Lifecycle::Committing => "Committing",
            Lifecycle::Committed => "Committed",
            Lifecycle::Aborted(_) => "Aborted",
            Lifecycle::Superseded => "Superseded",
        }
    }

    /// Whether 43 §1 marks this state terminal.
    ///
    /// `Denied` is "**terminal** (but only under record-only mode does §3's exception branch move it on to Canonicalized)" (sem: SEM-gx-engine-104), so
    /// the answer depends on a setting and the caller that knows the setting asks the question.
    /// [`Engine::canonicalize`] is that caller.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Lifecycle::Aborted(_) | Lifecycle::Committed | Lifecycle::Superseded
        )
    }
}

// ---------------------------------------------------------------------------
// 43 T-6 / ASM-12 -- the two deadlines
// ---------------------------------------------------------------------------

/// ASM-12's `verify_ttl`, in nanoseconds: **24 hours** (33 NFR-028).
///
/// [`Timestamp`] is an `i64` of nanoseconds, so the default is written as one rather than as a
/// `Duration` a caller would have to convert. 43 T-6 measures it from the moment a transformation
/// entered `Candidate` or `Verifying`; [`Engine::with_ttl`] is how a test asks for AC-045's 100 ms.
pub const DEFAULT_VERIFY_TTL_NANOS: i64 = 24 * 60 * 60 * 1_000_000_000;

/// ASM-12's `escalation_ttl`, in nanoseconds: **72 hours** (33 NFR-028).
///
/// Longer than [`DEFAULT_VERIFY_TTL_NANOS`] because the thing being waited for is a person. INV-L2
/// is what makes it finite at all: "any `Escalated` reaches ... within finite time (no indefinite hold)" (sem: SEM-gx-engine-105).
pub const DEFAULT_ESCALATION_TTL_NANOS: i64 = 72 * 60 * 60 * 1_000_000_000;

// ---------------------------------------------------------------------------
// 43 T-5 / T-5b -- what a person decided
// ---------------------------------------------------------------------------

/// A human ruling on an escalated transformation (43 T-5 / T-5b, DR-11, 44 §1.2's `--reason`).
///
/// # Three fields, because AC-071 asks for three
///
/// > confirm that the issued receipt trail (journal / Receipt metadata) contains `Evidence(HumanDecision)`
/// > (decision=Admit, reason, the ruling actor) (sem: SEM-gx-engine-106)
///
/// **E-M2-3** retired the `Evidence` variant that sentence names — "43 T-5's signed human-ruling
/// receipt is a receipt" (sem: SEM-gx-engine-106) — so the three facts live in the journal's `HumanDecision` record and
/// in the [`ReceiptKind::VerdictReceipt`] the transition issues. This struct is what the caller
/// hands in, and what its **digest** is: see [`Engine::escalation`] for why a human ruling needs
/// one at all.
///
/// # It is a value with an identity, and that is what makes the receipt honest
///
/// 42 §3.10's `VerdictSummary.proof_digest` is "not the whole `Verdict`, but its CID-form digest" (sem: SEM-gx-engine-107), and after a
/// human ruling there is no `Verdict` to digest: the gate answered `Escalate` and a person answered
/// something else. Carrying the *ticket's* digest under an `Admit` would say the gate admitted it;
/// minting an empty one is what §32 M4H4-2 refused twice. So the digest is of **this value** —
/// decision, reason, ruler — and nothing else, which is the one thing that is true.
///
/// `at` is not in it, so the digest is clock-free (CM-5: "clock reads excluded from the signed payload"; sem: SEM-gx-engine-108)
/// and two identical rulings made a day apart summarise identically. Raised as **M5H6-3**: 42
/// §3.10 gives no rule for this digest and req/49 §3 M2-10 left the `Deny`/`Escalate` rules open in
/// the same way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HumanRuling {
    /// 43 T-5 is `Admit` and T-5b is `Deny`. 42 §3.13: "kind is Admit or Deny only" (sem: SEM-gx-engine-109).
    pub decision: VerdictKind,
    /// 44 §1.2's `--reason <text>`. Non-empty; see [`Engine::escalation`].
    pub reason: String,
    /// Who ruled — **not** the submitter. `Transformation.actor` is who asked.
    pub actor: Actor,
}

impl IdentityView for HumanRuling {
    type View<'a> = &'a HumanRuling;

    fn identity_view(&self) -> &HumanRuling {
        self
    }
}

// ---------------------------------------------------------------------------
// M5-03 adopted (a) (sem: SEM-gx-engine-110) -- the evidence entry point
// ---------------------------------------------------------------------------

/// Where the evidence a gate decides on comes from (**M5-03 adopted (a)**; sem: SEM-gx-engine-111).
///
/// 32 FR-032 writes "call `Gate::verify` after collecting evidence" and 43 T-3 writes "launch the
/// evidence collector" (sem: SEM-gx-engine-111), and neither 41 §2's crate table nor 42 §3.7 says what a collector *is* -- 42 defines the
/// `Evidence` type and no producer of it. req/78 §4's M5-03 measured the consequence (the
/// constructors of `Evidence` have zero shipping callers) and req/38 §37 rules the shape:
///
/// > **M5-03, adopted (a)**: put a single `EvidenceSource` trait in gx-engine (append A-6 to 41 §2).
/// > `Err` is the sole producer of `VerifierUnavailable`. Confirmed consistent with 44's `--evidence`
/// > injection form (feeding in externally-collected material) (sem: SEM-gx-engine-112).
///
/// # The `Err` is the point
///
/// 43 T-4d and T-4e both fire on "verifier / evidence collector unreachable" (sem: SEM-gx-engine-113), and until this trait existed
/// **there was no way to be unreachable**: `Gate::verify` is a function call in the same process.
/// AC-036 asks for "`kill -9` the gx-gate process" (sem: SEM-gx-engine-113), which names a process that does not exist, and
/// **E-M5-4** (M5-19 adopted (a)) reads it as "the evidence collector is unreachable" instead. This trait is what
/// makes that reading implementable, so its failure is the workspace's **only** road to
/// `AbortReason::VerifierUnavailable` -- measured by
/// `tests/engine_shape.rs::verifier_unavailable_has_exactly_one_producer` and, from the behaviour
/// side, by `tests/ac_032.rs`.
///
/// Every `Err` is unreachability, not only [`Error::EvidenceUnavailable`]: a collector that failed
/// did not collect, and the engine has no way to tell "I could not reach it" from "it could not
/// answer" (sem: SEM-gx-engine-114) that would not be the collector's own claim about itself.
///
/// # Two implementations, and why those two
///
/// [`InjectedEvidence`] is 44 §1.2's `--evidence` in library form -- "feed in already-collected
/// `Evidence` (42 §3.7) as extra JSONL, for wiring in external collection tools such as test
/// results" (sem: SEM-gx-engine-115) -- and its empty case is 44's "when omitted, only gx-gate's built-in InvariantCheck/Cedar evaluation runs". [`UnreachableEvidence`] is the other side
/// of the `Result`, and a v0.1 that shipped only the first would be a v0.1 in which T-4d is
/// unreachable code.
pub trait EvidenceSource {
    /// Collect what the gate should decide on.
    ///
    /// # Errors
    /// Anything the collector cannot do. Every one of them reaches
    /// `AbortReason::VerifierUnavailable` under `FailPosture::FailClosed`, or T-4e's degraded
    /// admission under `FailPosture::FailOpen`.
    fn collect(&self, t: &Transformation, pre: &ObjectSnapshot) -> Result<Vec<Evidence>>;
}

/// Evidence handed in from outside, which is 44 §1.2's `--evidence` (**M5-03 adopted (a)**; sem: SEM-gx-engine-116).
#[derive(Clone, Debug, Default)]
pub struct InjectedEvidence {
    evidence: Vec<Evidence>,
}

impl InjectedEvidence {
    /// A source that answers with these items.
    #[must_use]
    pub fn new(evidence: Vec<Evidence>) -> Self {
        Self { evidence }
    }

    /// A source that answers with nothing, successfully.
    ///
    /// 44 §1.2's "when omitted, only gx-gate's built-in InvariantCheck/Cedar evaluation runs" (sem: SEM-gx-engine-117). **Not the same as a source
    /// that could not be reached** -- this one succeeds and the answer is the empty list, which is
    /// req/29 §4's rule ("do not give skip and pass the same face"; sem: SEM-gx-engine-117) at the one place a v0.1 would be most
    /// tempted to blur it.
    #[must_use]
    pub fn none() -> Self {
        Self {
            evidence: Vec::new(),
        }
    }
}

impl EvidenceSource for InjectedEvidence {
    fn collect(&self, _t: &Transformation, _pre: &ObjectSnapshot) -> Result<Vec<Evidence>> {
        Ok(self.evidence.clone())
    }
}

/// A source that cannot be reached — 43 T-4d and T-4e's precondition, as a value.
///
/// It carries what a deployment would have said, because "the collector is down" and "the
/// collector rejected our credentials" (sem: SEM-gx-engine-118) are different operational facts and the engine records the
/// distinction it was told rather than inventing one.
#[derive(Clone, Debug)]
pub struct UnreachableEvidence {
    detail: String,
}

impl UnreachableEvidence {
    /// A source that always refuses, saying this.
    #[must_use]
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl EvidenceSource for UnreachableEvidence {
    fn collect(&self, _t: &Transformation, _pre: &ObjectSnapshot) -> Result<Vec<Evidence>> {
        Err(Error::EvidenceUnavailable {
            detail: self.detail.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// AC-033 -- canon, and the one thing that may be replaced about it
// ---------------------------------------------------------------------------

/// `canon(T)`: the bytes 43 T-8 checks T3 over.
///
/// # Why this is a trait, and how 41 §6 survives it
///
/// 41 §6 is unambiguous: "every canonical encode goes through gx-canon alone, no bypass" (sem: SEM-gx-engine-119). A replaceable encoder
/// looks exactly like the second road that sentence forbids. AC-033 is equally unambiguous the other
/// way: "in the abnormal case that injects a broken canon implementation returning an idempotence violation, return an error and do not transition to Canonicalized",
/// which cannot be written unless something about canon can be broken on purpose.
///
/// Both hold because the two roads carry different things. **The identity is never injected**:
/// [`Engine::canonicalize`] takes the `canonical_cid` it journals from `gx_canon::cid::compute`, in
/// one place, whatever this trait says. What is injected is the *evidence for T-8's guard* -- the
/// bytes the idempotence check runs over. A broken canonicalizer can therefore make the engine
/// refuse to canonicalize; it cannot make the engine mint a CID that gx-canon did not compute.
///
/// The check itself is `gx_canon::cbor::is_canonical`, which is 42 §2.3's criterion
/// (`encode_canonical(decode(encode_canonical(x))) == encode_canonical(x)`) in the form gx-canon
/// already publishes and `gx-canon/tests/ac_012.rs` already measures. Asking gx-canon what it would
/// have written is a stronger question than re-running an encoder against itself, and it is the one
/// T-21 says is worth asking: an encoder that normalises everything satisfies idempotence
/// vacuously, and `is_canonical` is the function that refuses the spellings it would not have
/// written.
///
/// Raised as **M5H2-4**: the tension is real even though it resolves, and a later hand tempted to
/// widen this trait into "the engine's encoder" (sem: SEM-gx-engine-120) should meet the sentence rather than the shape.
pub trait Canonicalizer {
    /// The canonical bytes of the transformation's `IdentityView` (42 §1.1, §2.1).
    ///
    /// # Errors
    /// Whatever the encoder refuses. gx-canon refuses a value with no canonical form.
    fn canonical_form(&self, t: &Transformation) -> Result<Vec<u8>>;
}

/// The only shipping [`Canonicalizer`]: gx-canon, and nothing else (41 §6).
#[derive(Clone, Copy, Debug, Default)]
pub struct CanonEncoder;

impl Canonicalizer for CanonEncoder {
    fn canonical_form(&self, t: &Transformation) -> Result<Vec<u8>> {
        Ok(cbor::encode(&t.identity_view())?)
    }
}

// ---------------------------------------------------------------------------
// The table's rows
// ---------------------------------------------------------------------------

/// 🔴 **R3 / `req/222` H-03** — what 43 T-2 derives from a substrate it has only *read*.
///
/// Everything [`Engine::plan`] knows before it writes anything, and the whole of what
/// [`Engine::planned_id`] answers. Private: it is an internal shape and not a surface, and the two
/// callers are one function apart.
struct PlanShape {
    /// The `TransformationId` this plan names.
    id: TransformationId,
    /// The value that CID was computed over, with `id` filled in.
    transformation: Transformation,
    /// What `adapter.plan` answered.
    delta: PlannedDelta,
    /// `Fingerprint₀` (42 §3.5), as `adapter.precondition` answered it.
    fp0: Fingerprint,
    /// The snapshot the other two were derived from.
    pre: ObjectSnapshot,
}

/// 🔴 **DR-46-28 / DR-46-33** — the boundary every road on this file attests, derived from two
/// facts every road already holds: the input-generation stage the `Planned` record fixed at T-2,
/// and whether a verdict was derived.
///
/// # Where the input-generation stage comes from, and why a rebuild can reproduce it
///
/// `req/459` ruling 1 splits the work in two: a deployment **declares** where its inputs come from
/// (`gx_adapter_mcp::catalogue`'s reserved slot) and a receipt **attests** the boundary for one
/// transformation. Before DR-46-33 this function could only fill the verdict stage, for two walls:
/// the declaration is in a crate `gx-engine` does not depend on, and the actor that overrides it is
/// not in Σ, so a boundary derived from either could not survive 43 §7-3b's rebuild — which
/// compares a rebuilt payload's digest against the leaf the ledger already holds. That is the same
/// failure `crates/gx-cli/tests/model_a_probes.rs` measured as `payload_mismatch` for `read_set`,
/// and closing it there took a journal erratum (42 §3.13's `reads`).
///
/// DR-46-33 (`req/38` §413) closes it the same way: the declaration is carried into the engine by
/// the optional `InputStageDeclaration` registry, joined with the actor **once, at plan time**
/// (`Engine::joined_input_generation`), and the join's **result** is journalled on the `Planned`
/// record. So the `input_generation` this function receives is read back from the journal by
/// `Engine::journalled_input_generation` — the same value on the live road and the rebuild road,
/// neither of which needs the actor or the catalogue. `Unknown` for a substrate that declares
/// nothing and for a journal written before the erratum, which reproduces v0's value exactly and
/// keeps every pre-DR-46-33 commit's rebuild byte-identical.
///
/// The verdict-derivation stage *is* observed here, by the only question that decides it: did a
/// gate derive a verdict at all. 43 T-4e reaches a commit having called none.
fn attested_boundary(input_generation: BoundaryStage, has_verdict: bool) -> DeterminismBoundary {
    DeterminismBoundary::of_stages(
        input_generation,
        if has_verdict {
            BoundaryStage::DeterministicReplay
        } else {
            BoundaryStage::Unknown
        },
    )
}

/// One row of the engine store: everything T-2 fixed, plus what has happened since.
///
/// The bodies (`transformation`, `delta`, `pre`) are here because the transitions need them; the
/// **names** are what Σ is made of ([`Engine::sigma`]). `verdict_digest` is held rather than
/// recomputed for the reason hand 4 will need it: 42 §3.10's `ReceiptPayload` carries it, and a
/// digest recomputed at commit time from a verdict rebuilt at commit time would be a second answer
/// to a question the journal already recorded.
#[derive(Clone, Debug)]
struct Entry {
    intent_id: IntentId,
    transformation: Transformation,
    state: Lifecycle,
    /// 🔴 **T-6**: when this row entered `state`, which is what the two TTLs are measured from.
    ///
    /// Not in Σ, and that is deliberate rather than an omission: it is a **function of the
    /// journal** — the `at` of the record that fixed the current state — so a reconstruction can
    /// recompute it and AC-039's comparison would be measuring the same bytes twice. What Σ holds
    /// is the state; what this holds is when the clock 41 §6 injected said it was reached.
    since: Timestamp,
    /// 43 §8's "only an internal annotation, `blocked_by: TransformationId`" (sem: SEM-gx-engine-121) (hand 6).
    ///
    /// Not in Σ either, and for a *different* reason: no journal record carries it. 43 §8 is
    /// explicit that waiting adds "no new state" (sem: SEM-gx-engine-122), so a blocked transformation is a
    /// `Candidate` that has not been allowed to start verifying, and the annotation is a fact
    /// about the live in-flight set rather than about the log. A restart loses it, which is
    /// correct — the in-flight table is empty after one (M5H3-5).
    blocked_by: Option<TransformationId>,
    delta: PlannedDelta,
    fp0: Fingerprint,
    pre: ObjectSnapshot,
    verdict: Option<VerdictKind>,
    verdict_digest: Option<Cid>,
    enforced: bool,
    fail_posture_engaged: bool,
    canonical_cid: Option<Cid>,
    /// The ticket T-4c raised, with 41 §6's clock in it (**E-5**) and its id re-minted (**E-6**).
    ticket: Option<EscalationTicket>,
    /// 🔴 **M6H3-2 adopted (a)** (sem: SEM-gx-engine-123) — T-4a's `AdmitProof`, kept so that something can be *asked* for it.
    ///
    /// The third seat beside [`Entry::ticket`], and it exists for the reason req/38 §50 gives:
    /// 44 §1.2's stdout for `gx verify` is `{"kind":"Admit","proof":AdmitProof}` and 44 §2.3's
    /// problem `detail` is "detailed explanation" (sem: SEM-gx-engine-124), and until this field existed an operator asking "why" got a
    /// **digest** — a value that proves the proof was the one hashed and says nothing about what it
    /// contained. M6H3-2's ruling is "a read of the table; no effect on Σ" (sem: SEM-gx-engine-124) and that is exactly the shape: the
    /// journal records `verdict_digest` and not this, so nothing about Σ moves and a row rebuilt
    /// from the journal answers `None` here, the way it already answers `None` for
    /// [`Entry::verdict_receipts`].
    admit_proof: Option<AdmitProof>,
    /// 🔴 **M6H3-2 adopted (a)** (sem: SEM-gx-engine-125) — T-4b's reasons, for [`Entry::admit_proof`]'s reason one verdict along.
    ///
    /// 44 §1.2: `{"kind":"Deny","reasons":[Reason]}`. Separate from the proof rather than one
    /// `Option<Verdict>` because [`Entry::ticket`] already took the third variant's seat in M5, and
    /// a field that held a whole `Verdict` would carry the ticket twice.
    deny_reasons: Option<Vec<Reason>>,
    /// ASM-14's first kind, in order of issue: T-4a/b/c or T-4e, then T-5/T-5b (**M5H4-6**).
    ///
    /// A `Vec` because 43 T-5's side effect is "append the signed human-ruling receipt to the **provenance chain**" (sem: SEM-gx-engine-126)
    /// — an escalated transformation ends with two, signed by two different keys, and a field that
    /// held one would have to choose which of them to forget.
    verdict_receipts: Vec<Receipt>,
    /// T-12's edge, from `T_o`'s side. Written once, by the commit of the transformation that
    /// carried this one's escrowed inverse.
    superseded_by: Option<TransformationId>,
    // Hand 4. Everything below is written inside the commit critical section, and every one of them
    // is also a component of Σ -- the row and the reconstruction have to agree (AC-039), so a field
    // added here without an arm in `crate::replay::reconstruct` breaks a probe rather than drifting.
    apply_started: Option<Cid>,
    /// Two-phase escrow (`req/38` §98 ruling 1; sem: SEM-gx-engine-127): the journalled observation's CID, where one was
    /// recorded. A component of Σ like everything above it, so a field added here has its arm in
    /// `crate::replay::reconstruct` (`StateRow.observation_cid`).
    observation_cid: Option<Cid>,
    rollback: Option<Rollback>,
    provenance: Option<Provenance>,
    /// The escrowed inverse's CID (T-10b), which 42 §3.10 puts on the receipt.
    inverse_cid: Option<Cid>,
    /// 🔴 **E-M4-31 / M5-18 adopted (a)** (sem: SEM-gx-engine-128): the moment the engine says the apply happened, not the one the
    /// adapter returned. `Timestamp(0)` reaching this field is the bug the ruling names.
    applied_at: Option<Timestamp>,
    /// The receipt T-11 issued. Held in memory: 44's `gx receipt show` reads a store, and that
    /// store is M6's (req/78 N-01).
    receipt: Option<Receipt>,
}

/// An adapter and the version the deployment registered it under (**M5-07 adopted (a)**; sem: SEM-gx-engine-129).
///
/// 42 §3.9's `Environment.adapter_version` is a `String` and 41 §4's trait has **seven methods,
/// none of which reports a version**. N-07 forbids an eighth, so the value comes from the only
/// party that has it: whoever calls [`Engine::register_adapter`]. Raised as **M5H4-4**.
struct Registered {
    adapter: Arc<dyn SubstrateAdapter>,
    version: String,
}

/// 🔴 **R8 / `req/234` H-01** — where a commit receipt becomes durable, inside the critical section.
///
/// # Why this trait exists at all
///
/// Before R8 the receipt archive was written by the **caller**, after `Engine::commit` returned.
/// `req/234` H-01 measured what that cost with a real power cut: a commit that was durable in the
/// journal *and* in the ledger came back after the crash as a `Committed` row that would never have
/// a receipt — `gx undo` refused it forever (exit 3), `GET /v1/receipts/{tid}` answered 404,
/// `gx receipt verify` exited 6, and `gx repair` answered `rc=0 remedy: null head_authenticity:
/// "verified"`. The window was the whole of `ApplyStarted` → the caller's `write(2)`, measured at
/// ~44% of one commit.
///
/// The repair is not another detector. It is the **order**: the archive write now happens where
/// every other durable step of T-11 happens, and the `Committed` record — the one record that makes
/// the row terminal and therefore stops 43 §7-3b's recovery from finishing it — is written
/// **after** it. So a crash anywhere in the section leaves a row the recovery already knows how to
/// close, and the receipt the recovery re-issues (43 §7-3b, "re-issue the receipt from the existing
/// `InclusionProof` (if not yet issued)") is filed on the same road.
///
/// `req/38` §154 is the rule this implements: a commit whose receipt cannot be filed is not a
/// commit. `store` returning `Err` fails the call **before** the `Committed` record, which leaves
/// the row `Committing` with its leaf on the ledger — 43 §7-3b's own window, retried at the next
/// start-up and closed the moment the directory takes a file again.
///
/// # What it is not
///
/// It is **not** a second copy of `.gx/receipts/`'s naming. The engine hands over a
/// `TransformationId` and a `Receipt`; where that goes and under what name is the caller's layout
/// knowledge (req/56 §2), which is the same seam `Engine::open`'s note defends for the head store.
/// An engine with no sink registered behaves exactly as every engine did before R8 — that is the
/// honest denominator for the tests of this crate, and the two product roads (`gx serve`, the CLI)
/// register one at the writer's door.
pub trait CommitReceiptSink: Send + Sync {
    /// File one commit receipt durably. `Err` carries a sentence and **fails the commit**.
    ///
    /// # Errors
    /// Whatever the archive could not do, as a sentence an operator can act on.
    fn store(&self, id: &TransformationId, receipt: &Receipt) -> core::result::Result<(), String>;

    /// 🔴 **R9 / `req/236` H-03** — which key signed the commit receipt already filed for this row.
    ///
    /// `None` for a row with no receipt on the disk, for an archive that will not read, and for the
    /// default implementation — a sink that only writes is still a legal sink, and the recovery
    /// then falls back to the key it was handed and says so.
    ///
    /// # Why the engine asks a **sink** this
    ///
    /// 43 §7-3b rebuilds a `ReceiptPayload` and compares its digest with the leaf the ledger holds.
    /// `key_id` is a field of that payload, so the rebuild only reproduces the leaf when it is
    /// built with the key that signed the original. Until R9 it was built with the key the
    /// *recovering process* was handed, which made a recovery run under a different key a
    /// structural mismatch — and the mismatch wrote a terminal record, so the row could never be
    /// resumed again by anybody. `req/236` H-03 measured it: ACTOR key 7 runs, 0 bricked; OTHER key
    /// 8 runs, 8 bricked; `gx serve --signing-key <other>` 7 runs, 7 bricked.
    ///
    /// The key that signed the commit is written down in exactly one place a recovering process can
    /// read — the receipt R8 moved **inside** the critical section, in front of the `Committed`
    /// record. So the archive is asked, and where the archive has nothing (the narrow window in
    /// which the leaf landed and the receipt had not yet been filed) the recovery refuses instead
    /// of writing a record that closes the door.
    fn filed_key_id(&self, id: &TransformationId) -> Option<gx_core::KeyId> {
        let _ = id;
        None
    }

    /// 🔴 **R13 / `req/244` H-03** — the commit receipt this project already holds for this row.
    ///
    /// `None` for a row with no receipt on the disk, for an archive that will not read, for a
    /// document that will not decode, and for the default implementation.
    ///
    /// # Why the engine asks a **sink** for the whole document
    ///
    /// 43 §7-3b says a leaf in the ledger means "the commit had already completed before the
    /// crash; only the journal's `Committed` entry is missing". [`Engine::resume`] closed that
    /// window by **re-applying** the delta and rebuilding the payload, because 42 §3.10's
    /// `postcondition_fingerprint` is a fact about the world that no journal record carries — and
    /// `req/244` H-03 measured what that costs when the world cannot be reached. A `gx wrap`
    /// commit killed inside the window leaves the leaf on the ledger and no `Committed` record; the
    /// `gx repair --yes` that follows has no MCP server to ask, so `adapter.apply` refuses, and the
    /// old road answered that refusal with `Aborted(ApplyFailed)` — a **terminal** record, which is
    /// exactly the record 43 §7-2 makes the recovery stop at. The row was then unclosable forever,
    /// every writer verb answered `LEDGER_DISAGREES`, and the remedy told the operator their two
    /// files came from different projects. Measured: 6 of 40 runs in a 128–164 ms sweep.
    ///
    /// The fingerprint the rebuild could not obtain is written down in exactly one place a
    /// recovering process can read: the commit receipt R8 moved **inside** the critical section and
    /// in front of the `Committed` record. So the archive is asked for the document rather than for
    /// one field of it, and where it has one whose payload digests to the leaf the ledger already
    /// witnessed, the recovery writes the missing record and asks no adapter anything. Where it has
    /// none — the narrower window between `ledger.append` and the archive write — nothing is
    /// invented: see [`Engine::resume`]'s refusal.
    fn filed_receipt(&self, id: &TransformationId) -> Option<Receipt> {
        let _ = id;
        None
    }
}

/// 🔴 **R8 / `req/234` H-01 (b)** — what [`Engine::reissue_receipt`] could do about one row.
///
/// Five answers rather than a `bool`, because the remedy differs for each and because "no receipt
/// was written" is the finding `req/234` H-01 is about: an answer that could not say *why* would
/// reproduce the `remedy: null` it measured one level up.
#[derive(Debug)]
pub enum Reissued {
    /// The receipt was rebuilt, proved against the ledger's own digest, signed, and filed.
    Filed(Box<Receipt>),
    /// The world at this row's locator no longer digests to the postcondition the ledger
    /// witnessed, so no reading of it can be evidence of what this commit left behind.
    ///
    /// Not a failure of the re-issue: it is the true statement that the observation is gone.
    WorldMoved,
    /// Σ does not hold this row as `Committed`/`Superseded`, so there is nothing to re-issue for.
    NotCommitted,
    /// The row is committed and the ledger holds no leaf for it — 43 §7-3b's other half, which
    /// [`Engine::recover`] is what closes.
    NoLeaf,
    /// A journal trimmed past the records a payload is rebuilt from (42 §5).
    NoMaterial,
    /// 🔴 **R9 / `req/236` M-05** — the rebuild does not reproduce the leaf, and this run is holding
    /// a key that is not the one this project has been written under.
    ///
    /// The **same root cause as H-03**, wearing the harmless face: `key_id` is a field of the
    /// payload, so a re-issue attempted under another key cannot digest to what the ledger
    /// witnessed no matter what the world holds. `req/236` M-05 measured every row of a project
    /// whose substrate had not moved a byte being reported as `world_moved`, which sends an
    /// operator to look at a file that is exactly as they left it.
    ///
    /// The evidence for this answer is the project's **recorded head** (`Engine::recorded_head_key_id`):
    /// a signed statement of which key has been writing here. Where there is no head, or where it
    /// names this run's key, the answer stays [`Reissued::WorldMoved`] — nothing is claimed on a
    /// guess.
    KeyMismatch,
}

impl Reissued {
    /// The one word a report prints. No `_` arm, for [`crate::store::InverseStatus::kind`]'s reason.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Reissued::Filed(_) => "filed",
            Reissued::WorldMoved => "world_moved",
            Reissued::NotCommitted => "not_committed",
            Reissued::NoLeaf => "no_leaf",
            Reissued::NoMaterial => "no_material",
            Reissued::KeyMismatch => "key_mismatch",
        }
    }
}

// ---------------------------------------------------------------------------
// 43 §7 -- what a recovery did, per transformation
// ---------------------------------------------------------------------------

/// Which of 43 §7's roads [`Engine::recover`] walked for one transformation.
///
/// Four values for a section 43 writes as three steps, and the fourth is **E-M5-1**'s: §7-3 splits
/// on "a matching entry exists in the ledger" (sem: SEM-gx-engine-130) alone, and the ruling adds a second question — "was the
/// adapter asked" — whose answer decides whether the CAS may be re-run at all.
///
/// | value | 43 | what recovery found |
/// |---|---|---|
/// | [`RecoveryPath::Terminal`] | §7-2 | the last record is terminal: rebuild, re-run nothing |
/// | [`RecoveryPath::LedgerHeldTheCommit`] | §7-3b | a ledger entry exists: the commit completed |
/// | [`RecoveryPath::ApplyWasAnnounced`] | §7-3c + **E-M5-1** | an `ApplyStarted` exists and no ledger entry: re-apply, **do not re-run the CAS** |
/// | [`RecoveryPath::NothingWasApplied`] | §7-3c, refused | no `ApplyStarted` and no ledger entry: the world did not move, and the journal does not carry what re-running T-10a needs (see [`Engine::recover`]) |
/// | [`RecoveryPath::ClosedFromFiledReceipt`] | §7-3b, **R13 / `req/244` H-03** | a ledger entry exists **and** its commit receipt is already on the disk: the record is written from the document, and no adapter is asked |
/// | [`RecoveryPath::ClosedFromLedgerLeaf`] | §7-3b, **R13 / `req/244` H-03** | a ledger entry exists, no receipt was filed and the substrate could not be read: the record is written from the leaf and **no receipt is issued** |
/// | [`RecoveryPath::NotResumed`] | **R5 / `req/227` H-01** | the row was left alone: the journal is not the journal this process read, or the ledger no longer puts this commit last |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RecoveryPath {
    /// 43 §7-2: "just rebuild that state as correct in memory (no side effect is re-run)" (sem: SEM-gx-engine-131).
    Terminal,
    /// 43 §7-3b: "when a matching entry exists in the ledger, the commit had already completed before the crash" (sem: SEM-gx-engine-131).
    LedgerHeldTheCommit,
    /// 43 §7-3c as **E-M5-1** rewrites it: the apply was announced, so the CAS is not re-run.
    ApplyWasAnnounced,
    /// 43 §7-3c's re-run, refused because its inputs are not in the journal.
    NothingWasApplied,
    /// 🔴 **R13 / `req/244` H-03** — 43 §7-3b closed off the document the critical section had
    /// already filed, with no adapter asked and nothing re-applied.
    ///
    /// Distinct from [`RecoveryPath::LedgerHeldTheCommit`] and deliberately: that one re-applies
    /// the delta (because 42 §3.10's `postcondition_fingerprint` is a fact about the world) and
    /// then compares the rebuild with the leaf. This one never touches the world at all — the leaf
    /// says the commit completed, and the receipt R8 moved inside the section says what it
    /// completed with. An operator reading a report must be able to tell the two apart: one of them
    /// wrote to their substrate and one of them did not.
    ClosedFromFiledReceipt,
    /// 🔴 **R13 / `req/244` H-03** — 43 §7-3b's `Committed` record, written from the leaf alone,
    /// because there was no filed receipt and the world could not be read.
    ///
    /// # What this road claims, and what it does not
    ///
    /// It claims what 43 §7-3b claims and nothing more: "when a matching entry exists in the
    /// ledger, the commit had already completed before the crash; only the journal's `Committed`
    /// entry is missing". That entry is `{transformation, ledger_seq, at}` — it carries no digest
    /// and no fingerprint, so the leaf determines it completely. It does **not** claim a receipt:
    /// none is issued here, `gx repair`'s `receipts_missing` counts the row, and
    /// `--reissue-receipts` is the road to one from a process that can read the substrate.
    ///
    /// # Why the record is written without the re-apply that used to gate it
    ///
    /// The re-apply on [`RecoveryPath::LedgerHeldTheCommit`] is not there to find out whether the
    /// commit happened — the leaf answers that — but to obtain 42 §3.10's
    /// `postcondition_fingerprint`, which is a reading of the world and which the journal has no
    /// seat for. `req/244` H-03 measured what happens when the process holding the recovery cannot
    /// perform that reading: a `gx wrap` commit is applied through an MCP server, `gx repair` has
    /// no server, `adapter.apply` refuses, and the old road answered that refusal with
    /// `Aborted(ApplyFailed)` — a terminal record, over a commit that had completed, which made the
    /// project permanently unwritable and its remedy blame a `.gx/ledger/` copied in from
    /// elsewhere. 25 of 27 projects in a 109–125 ms sweep.
    ///
    /// Three answers were available and this is the least dishonest of them. `Aborted` says the
    /// commit failed and it did not. `NotResumed` says nothing and leaves every writer verb
    /// refusing `LEDGER_DISAGREES` for ever. This one writes the entry the ledger already
    /// determines, and says — in the report, in this path's own name — that no receipt was issued
    /// and no substrate was touched.
    ///
    /// # What still guards it
    ///
    /// Everything [`Engine::recover`] already refuses on: a journal that is not the journal this
    /// process read, a project behind its published head, a head this binary will not read numbers
    /// off. Plus [`not_resumed::LEDGER_MOVED_ON`] — this leaf must be the **last** one, which is
    /// what §7-3b's window means — and the row must carry an `ApplyStarted`, because a leaf whose
    /// apply was never announced is not this window at all.
    ClosedFromLedgerLeaf,
    /// 🔴 **R5 / `req/227` H-01** — the recovery asked no adapter, wrote no record and left the row
    /// where it found it, because what it was about to act on is not evidence.
    ///
    /// Two conditions reach it and [`Recovered::refusal`] says which. Both are about the **inputs**
    /// rather than about the transformation, which is why they are one road: a recovery reads a
    /// journal and a ledger, and where those two do not support each other the honest recovery is
    /// the one that does not run.
    NotResumed,
}

/// The names of [`RecoveryPath`], declared once (**E-M2-23 / A-10**).
pub const RECOVERY_PATHS: [&str; 7] = [
    "Terminal",
    "LedgerHeldTheCommit",
    "ApplyWasAnnounced",
    "NothingWasApplied",
    "ClosedFromFiledReceipt",
    "ClosedFromLedgerLeaf",
    "NotResumed",
];

impl RecoveryPath {
    /// Which of [`RECOVERY_PATHS`] this is. No `_` arm, for [`crate::Error::kind`]'s reason.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            RecoveryPath::Terminal => "Terminal",
            RecoveryPath::LedgerHeldTheCommit => "LedgerHeldTheCommit",
            RecoveryPath::ApplyWasAnnounced => "ApplyWasAnnounced",
            RecoveryPath::NothingWasApplied => "NothingWasApplied",
            RecoveryPath::ClosedFromFiledReceipt => "ClosedFromFiledReceipt",
            RecoveryPath::ClosedFromLedgerLeaf => "ClosedFromLedgerLeaf",
            RecoveryPath::NotResumed => "NotResumed",
        }
    }
}

/// 🔴 **R32 / `req/392` M-02** — which one of [`Engine::journal_intact`]'s terms is false.
///
/// # Why this type exists
///
/// `journal_intact` is a `&&` chain over **seven** facts, and until this lane the sentence every
/// face printed about it was one paragraph asserting **one** of them as the cause. The
/// thirty-first audit drove three of the seven and measured the paragraph false on two: a journal
/// whose marker had been stripped and one carrying a marker from a build that does not exist were
/// both told that *"since DR-43-9 this is the per-record chain refusing to verify"* — over files
/// that carry no chain for a link to refuse — and both were told to look for a
/// `<journal>.torn.<n>-<m>` that this build's own `gx repair --yes` says in the same breath it
/// will never write.
///
/// The repair is the one this repository already uses where a fold is unavoidable:
/// `gx_engine::NotAttemptedBecause` gives each cause its own sentence (six when this was written;
/// seven since R-1001-1, `req/1001` §4) and `gx-cli`'s
/// `not_attempted_cause_clause` prints them one per arm. This is that shape for the journal.
///
/// # One spelling, asked twice
///
/// `req/38` §227's sibling-sweep rule is why the terms live in [`JournalTerms`] and the bool is
/// **derived** from this value rather than computed beside it: `Engine::journal_intact` is now
/// `self.journal_departure.is_none()`, so the sentence and the refusal cannot disagree about what
/// happened. The two places that evaluate the terms (`Engine::open` and the catch-up) hand them to
/// the same function.
///
/// # Order
///
/// Declaration order is the order [`JournalTerms::departure`] asks in, and it is not arbitrary:
/// the two facts about the **framing** come first because they explain the ones after them. A file
/// from a newer `gx` and a file whose marker was removed both replay as `Legacy`, so a file from
/// the future would otherwise be reported as a downgrade — which is the more specific fact losing
/// to the more general one, the failure mode this whole type is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalDeparture {
    /// [`crate::EngineJournal::from_a_newer_gx`]: the file carries a framing marker this build has
    /// never heard of. Nothing is wrong with it; this binary cannot verify it.
    FromANewerGx,
    /// [`crate::EngineJournal::downgraded`]: `.gx/VERSION` declares a chained journal and the file
    /// carries no marker. 🔴 **R32 / `req/392` M-01** — a journal of zero bytes reaches this arm
    /// now, because `replay` reports the framing the disk has rather than the one a writer is about
    /// to give it.
    Downgraded,
    /// [`crate::EngineJournal::chain_intact`] is false: a whole record is on the file whose stored
    /// link is not the link its payload and its predecessors produce (DR-43-9).
    ChainBroken,
    /// The file is shorter than the bytes this process had already read back from it.
    Shortened,
    /// [`crate::EngineJournal::tail_unchanged`] is false: the last framed record no longer reads
    /// the way it read when this process read it, at the same length.
    TailRewritten,
    /// `prefix_intact` is false: the consumed prefix no longer produces the head this process has
    /// been carrying, so a byte somewhere behind the frontier moved.
    PrefixRewritten,
    /// The reader's door met a torn tail — bytes after the last whole record that did not replay —
    /// and removed nothing, because a read has no lock. The writer's door quarantines and cuts one
    /// on its way through (DR-43-7), so it never reaches this arm.
    TornTail,
}

impl JournalDeparture {
    /// The seven, in declaration order. One arm per entry everywhere they are printed, and no `_`
    /// arm, for [`crate::Error::kind`]'s reason and for `req/392` L-01's.
    pub const ALL_DEPARTURES: [&'static str; 7] = [
        "FromANewerGx",
        "Downgraded",
        "ChainBroken",
        "Shortened",
        "TailRewritten",
        "PrefixRewritten",
        "TornTail",
    ];

    /// Which of [`JournalDeparture::ALL_DEPARTURES`] this is. No `_` arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            JournalDeparture::FromANewerGx => "FromANewerGx",
            JournalDeparture::Downgraded => "Downgraded",
            JournalDeparture::ChainBroken => "ChainBroken",
            JournalDeparture::Shortened => "Shortened",
            JournalDeparture::TailRewritten => "TailRewritten",
            JournalDeparture::PrefixRewritten => "PrefixRewritten",
            JournalDeparture::TornTail => "TornTail",
        }
    }
}

/// 🔴 **R32 / `req/392` M-02** — the terms of the `&&` chain, named, so the fold is asked in one
/// place.
///
/// Not public: the answer is, the question is this crate's. Every field is "the term as the old
/// chain spelled it", so a reader can put this struct beside the `&&` chain the audit quoted and
/// see the same seven facts.
struct JournalTerms {
    /// `!journal.from_a_newer_gx()` in the old chain.
    not_from_a_newer_gx: bool,
    /// `!journal.downgraded()`.
    not_downgraded: bool,
    /// `journal.chain_intact()`.
    chain_intact: bool,
    /// `!on_disk.is_some_and(|len| len < read_offset())`.
    not_shorter_than_read: bool,
    /// `journal.tail_unchanged()`.
    tail_unchanged: bool,
    /// `prefix_intact`.
    prefix_intact: bool,
    /// `matches!(door, Door::Writer) || recovery().torn_tail_bytes == 0`.
    no_unrepaired_torn_tail: bool,
}

impl JournalTerms {
    /// The first term that is false, in [`JournalDeparture`]'s declaration order, or `None` when
    /// every one of them holds — which is exactly the old `&&` chain's `true`.
    const fn departure(&self) -> Option<JournalDeparture> {
        if !self.not_from_a_newer_gx {
            Some(JournalDeparture::FromANewerGx)
        } else if !self.not_downgraded {
            Some(JournalDeparture::Downgraded)
        } else if !self.chain_intact {
            Some(JournalDeparture::ChainBroken)
        } else if !self.not_shorter_than_read {
            Some(JournalDeparture::Shortened)
        } else if !self.tail_unchanged {
            Some(JournalDeparture::TailRewritten)
        } else if !self.prefix_intact {
            Some(JournalDeparture::PrefixRewritten)
        } else if !self.no_unrepaired_torn_tail {
            Some(JournalDeparture::TornTail)
        } else {
            None
        }
    }
}

/// 🔴 **R5 / `req/227` H-01** — why a [`RecoveryPath::NotResumed`] row was left alone.
///
/// The sentences are `&'static str` and are printed by every face, so an operator who reads one in
/// `gx serve`'s start-up and one in `gx repair`'s JSON is reading the same words about the same
/// fact.
pub mod not_resumed {
    /// The journal is not the journal this process read (DR-43-9's chain, or a torn or shortened
    /// file).
    pub const JOURNAL_MOVED: &str = "the journal on the disk is not the journal this process read         (DR-43-9): 43 §7's recovery re-applies a delta to the substrate, and a delta named by bytes         that were rewritten is not evidence of anything. Nothing was applied and nothing was         recorded; `gx repair` reports which file moved";
    /// 🔴 **R32 / `req/392` M-02** — [`JOURNAL_MOVED`]'s five siblings.
    ///
    /// The const above is unchanged, byte for byte, and is now printed for the three conditions it
    /// is **true** of: a link that does not verify, a rewritten tail and a rewritten prefix. Those
    /// are the ones where *"a delta named by bytes that were rewritten"* names what happened. The
    /// four below are the conditions where it did not, and where printing it sent an operator to
    /// look for a rewrite nobody had made.
    ///
    /// The file is shorter than the bytes this process read. Nothing was rewritten; records are
    /// gone.
    pub const JOURNAL_SHORTER: &str = "the journal on the disk is shorter than the bytes this \
         process had already read back from it, so records this process folded are no longer on \
         the file: 43 §7's recovery re-applies a delta to the substrate, and a delta named by \
         bytes that are gone is not evidence of anything. Nothing was rewritten, so there is no \
         chain break to look at; nothing was applied and nothing was recorded, and `gx repair` \
         prints both lengths";
    /// `.gx/VERSION` declares a chained journal and the file carries no framing marker.
    pub const JOURNAL_DOWNGRADED: &str =
        "this project declares a chained journal in `.gx/VERSION` \
         and the file on the disk carries no framing marker (req/229 H-02): a recovery re-applies \
         a delta to the substrate, and this build will not do that from a file whose framing is \
         not the framing its project declares. **No byte of the journal is claimed to have been \
         rewritten** — what disagrees is the declaration and the marker — so the two things to \
         compare are `.gx/VERSION` and the first eight bytes of the journal. Nothing was applied \
         and nothing was recorded";
    /// The marker on the file belongs to a build this one has never heard of.
    pub const JOURNAL_FROM_A_NEWER_GX: &str =
        "the journal carries a framing marker this build has \
         never heard of, so the records inside it were written by a newer `gx` (req/372 M-02). \
         **Nothing is wrong with the file**: this binary cannot verify it, which is a different \
         fact from damage, and every conclusion it could draw by walking those bytes as frames it \
         knows would be false. Nothing was applied, nothing was recorded and nothing was removed \
         from the file; run the `gx` that wrote it";
    /// The reader's door met bytes after the last whole record that did not replay.
    pub const JOURNAL_TORN_TAIL: &str = "the journal ends part-way through a record — the bytes \
         after its last whole record did not replay, which is the ordinary shape of a process \
         that died while it was writing (DR-43-7). This door read the file without the project \
         lock, so it removed nothing and those bytes are still on it; a recovery that re-applied a \
         delta from a file it has not finished reading would be acting on a prefix nobody has \
         accounted for. Nothing was applied and nothing was recorded";

    /// 🔴 **R32 / `req/392` M-02** — the sentence for a departure, one arm each and no `_` arm.
    ///
    /// The proxy `gx-cli`'s `not_attempted_cause_clause` is for
    /// `gx_engine::NotAttemptedBecause`, in this crate because the value is this crate's.
    #[must_use]
    pub const fn journal_departed(departure: super::JournalDeparture) -> &'static str {
        match departure {
            super::JournalDeparture::FromANewerGx => JOURNAL_FROM_A_NEWER_GX,
            super::JournalDeparture::Downgraded => JOURNAL_DOWNGRADED,
            super::JournalDeparture::Shortened => JOURNAL_SHORTER,
            super::JournalDeparture::TornTail => JOURNAL_TORN_TAIL,
            // The three the unchanged const is true of.
            super::JournalDeparture::ChainBroken
            | super::JournalDeparture::TailRewritten
            | super::JournalDeparture::PrefixRewritten => JOURNAL_MOVED,
        }
    }

    /// The ledger backs this commit at a leaf that is no longer the last one.
    pub const LEDGER_MOVED_ON: &str = "the ledger witnesses this commit at a leaf that later leaves         follow, so the critical section it appears to be inside closed and the world moved on         afterwards (req/227 H-01). Re-applying here would write an old delta over a newer state,         which is the accident this refusal exists for. Nothing was applied";
    /// 🔴 **R7 / `req/232` H-01/M-07** — the recorded head is not a document this binary believes.
    pub const HEAD_INVALID: &str = "this project's recorded head is not a document this binary will         read numbers off (head_invalid, req/232 H-01): its signature does not check out under the         key it names, its signed witness disagrees with the fields beside it, or the file will not         parse. A replaced detector is not an absent one — the audit wrote `tree_size: 0` over this         file, left the signature alone, and watched an operator's file go from `three` back to         `two`. Nothing was applied and nothing was recorded";
    /// 🔴 **R-923-1** — the machine-readable tag [`HeadReading::invalid`]'s four call sites in
    /// `read_head` prefix their own per-error sentence with, hand-written four times before this
    /// constant existed (`req/923` §1d found the drift: [`HEAD_INVALID`] above is a full canned
    /// sentence with a different consumer — `not_resumed::HEAD_INVALID` at the bottom of this file
    /// — and none of the four call sites read from either constant). This is the tag alone, no
    /// colon, so `format!("{HEAD_INVALID_PREFIX}: {detail}")` reproduces the old literal byte for
    /// byte.
    pub const HEAD_INVALID_PREFIX: &str = "head_invalid";
    /// 🔴 **R8 / `req/234` H-02 + L-03** — the *declaration* moved, and nothing else did.
    ///
    /// A separate sentence from [`ROLLED_BACK`] because the two are different facts and R7 gave
    /// them one voice. `req/234` measured the cost: an editor's trailing newline made a project
    /// whose journal, ledger and chain were provably whole answer "its ledger, its journal, or
    /// both are shorter" — a diagnosis that is false about the files it names and that sends an
    /// operator to restore a backup they do not need.
    pub const DECLARATION_CHANGED: &str = "this project's `.gx/VERSION` does not declare what it         declared when its last head was signed (43 §7.9 Model A, req/232 M-02): the file says which         framing this project's journal is in, and a head is signed beside it. **The ledger and the         journal are not what moved** — `gx repair` prints both counts, and if they match then this         is the only difference. Since R8 the digest is taken over what the file *declares* rather         than over its bytes, so line endings, trailing space and a trailing newline no longer         matter; a changed value does. Nothing was applied and nothing was recorded";
    /// 🔴 **R6 / `req/229` H-01** — the project is behind the head it has already published.
    /// 🔴 **R9 / `req/236` H-03** — the recovery was run under a key that did not sign this commit.
    ///
    /// The one refusal on this list that is about **who is running the recovery** rather than about
    /// what the project holds. It is recoverable by definition: run the same verb again with the
    /// key the commit receipt names, and the rebuild reproduces the leaf.
    pub const RECOVERY_KEY_MISMATCH: &str = "the receipt payload rebuilt here does not digest to the         leaf the ledger already holds for this commit, and the difference is the signing key         (req/236 H-03, 43 §7-3b): `key_id` is a field of the payload, so a recovery run under a         key other than the one that signed the commit cannot reproduce the leaf. Nothing was         applied and **no terminal record was written**, so this row is still resumable. What to fix:         run `gx repair --yes --signing-key <the key that committed>` — the commit receipt under         `.gx/receipts/` names it, and `gx serve --signing-key` chooses it for a server. If no         receipt was filed for this row (the window between the leaf and the archive write), the         key is the one the committing process used";
    /// 🔴 **R33 / `req/397` H-01** — the rebuild does not digest to the leaf, and this run cannot
    /// say why.
    ///
    /// [`RECOVERY_KEY_MISMATCH`]'s honest sibling, and the reason it is a second const rather than
    /// a rewrite of the first. The old sentence was printed here **and** meant the key: it asserted
    /// "the difference is the signing key" as a plain fact on an arm that had compared one digest
    /// against one digest and therefore knew only that they differed. `req/397` §2-3 drove a bed
    /// whose two sessions used a byte-identical key (`KeyPair::from_seed("key-engine-1", &[7u8;
    /// 32])`) and read the sentence back verbatim; monitoring 31 M-02 named the shape — a fold that
    /// prints one cause in the indicative — and R32 answered it for the journal by giving each
    /// condition its own arm ([`journal_departed`]). This is the same answer for this fold, with
    /// the difference that the causes here are **not** separable by any measurement this run can
    /// make, so they are listed instead of chosen.
    ///
    /// The key case that *is* separable is separated: where the project's recorded head names a
    /// different key, [`super::Engine::resume`] establishes that by comparison **before** it asks
    /// the substrate anything, and returns [`RECOVERY_KEY_MISMATCH`] from there.
    ///
    /// # 🔴 **R34 / `req/449` M-01** — a fourth cause, and why a three-member list was a bug
    ///
    /// R33 replaced an assertion with a disjunction, which was the right move and left the next
    /// question standing: **is the disjunction exhaustive?** The thirty-third audit answered no,
    /// with a bed in which all three listed causes are false. A `StampingAdapter` — one that
    /// digests *what it was sent* when it writes and *what the substrate now holds* when it reads,
    /// which is every server that normalises, re-encodes or stamps metadata on write — reaches
    /// this refusal on an untouched project, with the key that signed the commit, a chain that
    /// verifies, and the world exactly where the commit left it (`req/449` §4-1:
    /// `path=NotResumed payload_matched=Some(false) applies=0 reads=1 moved=false`).
    ///
    /// The direction is right — it fails **closed** — and `docs/LIMITS.md` v0.5-s residue (2)
    /// declared the case. What was wrong was the sentence: an operator was handed three things to
    /// check, all three would come back clean, and the fourth was not in the vocabulary. The old
    /// three-member text is preserved verbatim in `req/451` and in the audit report.
    ///
    /// # The cost this names and does not fix
    ///
    /// Re-running reads the same world and rebuilds the same payload, so `payload_matched` stays
    /// `Some(false)` for ever: `gx repair --yes`, `--reissue-receipts` and `gx serve` all land
    /// here. **No verb closes the row.** That is the shape `req/38` §238 / spec 43 §7.11's R10 row
    /// declared for a *lost key*, reached here with the key in hand and nothing damaged. R34
    /// declares it in the sentence rather than adding an exit, for R10's own reason: the exit is a
    /// verb that writes a `Committed` record over a comparison that failed, and which comparison a
    /// human may overrule is a decision for a DR and not for a refusal string (`req/451` §M-01
    /// raises it).
    pub const RECOVERY_REBUILD_DISAGREES: &str =
        "the receipt payload rebuilt here does not digest \
         to the leaf the ledger already holds for this commit (43 §7-3b, req/397 H-01), and this \
         run cannot tell you which field moved: a digest that does not match says the document is \
         not the one the ledger witnessed, and nothing more. **Nothing was applied**: on this road \
         the `postcondition_fingerprint` is *read* off the substrate rather than produced by \
         re-applying the delta, so `adapter.apply` was never called, no terminal record was \
         written, and this row is still resumable. The causes worth ruling out, in the order they \
         are cheap to check: (1) **the signing key** — `key_id` is a field of the payload, so a \
         recovery run under a key other than the one that signed the commit cannot reproduce the \
         leaf; the commit receipt under `.gx/receipts/` names it and `gx repair --yes \
         --signing-key <that key>` uses it. (2) **the world moved after the crash** — this row's \
         object is not what the commit left behind, so the reading is a different fingerprint; \
         `gx replay <ID>` names the object and the commit receipt names the digest it should hold. \
         (3) **the journal names a delta the commit did not apply** — a journal whose links are \
         recomputed end to end verifies perfectly, and the ledger beside it is what catches that \
         (`docs/LIMITS.md`); the leaf is the witness that disagrees, and it is disagreeing now. \
         (4) **the adapter's read and its apply are not the same computation** — the fingerprint \
         in the leaf is the one `apply` answered with, and the one rebuilt here is what a *read* \
         of the object answers now; a substrate that normalises, re-encodes or stamps what it is \
         given returns two different digests for one unchanged object, and then none of (1)-(3) \
         is true and this refusal is permanent under re-running (`req/449` M-01, \
         `docs/LIMITS.md` v0.5-s residue (2)). Which of the four it is, this run cannot say: it \
         compared one digest with one digest";
    /// 🔴 **R13 / `req/244` H-03** — the ledger holds this row's leaf, no commit receipt was
    /// filed for it, and the world could not be reached to rebuild one.
    ///
    /// 43 §7-3b's window with its narrower half taken: the crash landed between `ledger.append`
    /// and the archive write R8 put in front of the `Committed` record, so there is no document to
    /// close the record from, and the rebuild needs 42 §3.10's `postcondition_fingerprint`, which
    /// is a fact about a substrate this process cannot ask (an MCP server that is not running, an
    /// adapter that is not registered, a locator that is gone).
    ///
    /// **No terminal record is written**, which is the whole of the repair. The old road answered
    /// this with `Aborted(ApplyFailed)`, and `req/244` H-03 measured what that cost: a terminal
    /// record is the record 43 §7-2 makes the recovery stop at, so the row became unclosable by
    /// anybody, every writer verb answered `LEDGER_DISAGREES` forever, and the remedy told the
    /// operator their two files were from different projects. R9 established the shape for exactly
    /// this situation on the neighbouring arm ([`RECOVERY_KEY_MISMATCH`]): leave the row where it
    /// is, say which fact stopped the run, and stay resumable.
    ///
    /// 🔴 **Not reached from [`Engine::resume`] as this release ships.** It was written first, as
    /// the honest "provable but not closable" answer, and the sweep that measured it said the
    /// answer was not good enough: 25 of 27 projects in the window landed here, and every one of
    /// them was a project no `gx repair --yes` could ever make writable. The road now closes the
    /// record from the leaf ([`super::RecoveryPath::ClosedFromLedgerLeaf`]) and issues no receipt.
    /// The sentence is kept because it is the one to print if a future gate decides that writing
    /// the entry without a world reading is more than 43 §7-3b licenses — the argument is in
    /// `ClosedFromLedgerLeaf`'s own doc, and this is the other side of it.
    pub const LEDGER_HELD_NO_RECEIPT: &str = "the ledger holds this commit's leaf and the journal \
         does not witness it, so this is 43 §7-3b's window — and the commit receipt that would \
         close it was not filed before the crash (the narrower window between `ledger.append` and \
         the archive write). Rebuilding the receipt here needs 42 §3.10's \
         `postcondition_fingerprint`, which is a reading of the substrate, and this run could not \
         obtain one: the adapter refused or is not registered (a `gx wrap` commit is closed \
         through an MCP server, and `gx repair` is not connected to one). Nothing was applied and \
         **no terminal record was written**, so this row is still resumable. What to fix: run the \
         repair from a process that can reach the substrate this row names — for a `gx wrap` \
         commit, `gx repair --yes` under the same `--mcp-server` — or accept the leaf as proved \
         and unclosable: `gx receipt verify` still answers for every leaf that has a receipt, and \
         `gx repair` prints `journal_behind_by` so the difference is one number (req/244 H-03)";
    /// 🔴 **R6 / `req/229` H-01** — the project is behind the head it has already published.
    pub const ROLLED_BACK: &str = "this project is behind the signed head it has already published         (rolled_back, DR-43-11): its ledger, its journal, or both are shorter than the furthest         point `.gx/checkpoints/head.json` records. `req/229` H-01 measured what running here costs         — the recovery re-applied a delta from before two later commits and an operator's file went         from `three` back to `two`. Nothing was applied and nothing was recorded";
}

/// 🔴 ~~**R6 / DR-43-11** — `read_floor`, which raised `Error::Ledger` when the head would not
/// read.~~ **R7**: the refusal is kept and the **raise** is not. `req/232` M-07 measured what the
/// raise cost — one byte of rubbish in `head.json` and `gx repair`'s report mode printed no JSON at
/// all, `gx_code: "INTERNAL"` — so the reading below carries the same fail-closed answer in a value
/// that the diagnosis can print. The function that raised is gone; the sentence it raised with is
/// in [`HeadReading::invalid`].
///
/// 🔴 **R7 / `req/232` H-01 + M-07** — everything a door learns from `.gx/checkpoints/head.json`.
///
/// Three facts, and none of them is an error: the floor to compare against (`None` when there is no
/// head **or** when the head is not one to compare against), what the signature check answered, and
/// the sentence to print when the answer was `Refuted`.
///
/// # Why this cannot raise
///
/// `req/232` M-07 measured `gx repair`'s **report** mode dying with `gx_code: "INTERNAL"` and no
/// JSON at all on a `head.json` holding one byte of rubbish — the same failure `req/227` M-04 and
/// `req/229` M-02 closed for the ledger and the verdict chain, reappearing in the file R6 added.
/// 44 §2.3's `INTERNAL` is "not classifiable" and "the head is corrupt" is entirely classifiable.
/// ∴ a malformed head is `Refuted` with the parser's own sentence, every gate refuses (fail-closed:
/// a corrupt detector is not an absent one), and the reader's door still opens so that the rest of
/// the diagnosis — the ledger, the journal, the counts — can be printed.
#[derive(Debug, Default)]
struct HeadReading {
    floor: Option<gx_log::HeadFloor>,
    authenticity: Option<HeadAuthenticity>,
    invalid: Option<String>,
    /// 🔴 **R9 / `req/236` M-05** — the DSSE `keyid` the recorded head was signed under.
    ///
    /// The one place a *reading* process can find out which key this project has been written with
    /// when no commit receipt is on the disk. `Engine::reissue_receipt` uses it to tell "the world
    /// moved" from "you are holding the wrong key", which `req/236` M-05 measured being reported as
    /// the former for every row of a project whose substrate had not moved a byte.
    key_id: Option<String>,
}

fn read_head(
    store: &gx_log::HeadStore,
    keys: Option<&Arc<dyn HeadKeys>>,
    version_digest: Option<&str>,
) -> HeadReading {
    let head = match store.read() {
        Ok(None) => {
            return HeadReading {
                floor: None,
                authenticity: Some(HeadAuthenticity::Absent),
                invalid: None,
                key_id: None,
            }
        }
        Ok(Some(head)) => head,
        Err(e) => {
            return HeadReading {
                floor: None,
                authenticity: Some(HeadAuthenticity::Refuted),
                invalid: Some(format!(
                    "{prefix}: {} will not read as a head this binary wrote ({e}). A detector \
                     that cannot be read is not an absent one — absence means this project never \
                     recorded a head, and an unreadable document means somebody replaced it, so \
                     this refuses rather than passing (req/232 M-07, 43 §7.9 Model A)",
                    store.path().display(),
                    prefix = not_resumed::HEAD_INVALID_PREFIX,
                )),
                key_id: None,
            }
        }
    };
    let floor = match head.floor() {
        Ok(floor) => floor,
        Err(e) => {
            return HeadReading {
                floor: None,
                authenticity: Some(HeadAuthenticity::Refuted),
                invalid: Some(format!(
                    "{prefix}: {e} (req/232 H-01)",
                    prefix = not_resumed::HEAD_INVALID_PREFIX
                )),
                key_id: Some(head.checkpoint.signature.keyid.clone()),
            }
        }
    };
    let authenticity = match keys {
        None => HeadAuthenticity::Unverified,
        Some(keys) => match keys.verifying(&head.checkpoint.signature.keyid) {
            None => HeadAuthenticity::Unverified,
            Some(key) => {
                match gx_witness::dsse::verify_checkpoint(&head.checkpoint, &key.verifying()) {
                    Err(e) => {
                        return HeadReading {
                            floor: None,
                            authenticity: Some(HeadAuthenticity::Refuted),
                            invalid: Some(format!(
                                "{prefix}: the signed head in {} does not verify under the key \
                                 it names ({}): {e}. `req/232` H-01 wrote `tree_size: 0` over this \
                                 file, left the signature where it was, and every gate opened — so \
                                 a head whose signature does not cover its numbers is refused \
                                 rather than compared against (43 §7.9 Model A)",
                                store.path().display(),
                                head.checkpoint.signature.keyid,
                                prefix = not_resumed::HEAD_INVALID_PREFIX,
                            )),
                            key_id: Some(head.checkpoint.signature.keyid.clone()),
                        }
                    }
                    Ok(()) => match verify_head_witness(&head, &key) {
                        Ok(()) => HeadAuthenticity::Verified,
                        Err(why) => {
                            return HeadReading {
                                floor: None,
                                authenticity: Some(HeadAuthenticity::Refuted),
                                invalid: Some(format!(
                                    "{prefix}: {why} (req/232 H-01)",
                                    prefix = not_resumed::HEAD_INVALID_PREFIX
                                )),
                                key_id: Some(head.checkpoint.signature.keyid.clone()),
                            }
                        }
                    },
                }
            }
        },
    };
    // The declaration digest travels **in** the floor, so a head written before R7 (which records
    // none) compares exactly as it did and a head written after it carries the file it was written
    // beside. The comparison itself is `gx_log::head::compare`'s last arm.
    let _ = version_digest;
    HeadReading {
        floor: Some(floor),
        authenticity: Some(authenticity),
        invalid: None,
        key_id: Some(head.checkpoint.signature.keyid.clone()),
    }
}

/// 🔴 **R7 / DR-43-11 (b)** — the witness beside the checkpoint, checked and compared.
///
/// Two failures, and they are different failures: a signature that does not verify (somebody else
/// wrote this), and a payload that verifies but does not say what the document around it says
/// (somebody kept a genuine witness and edited the fields beside it). A head that carries **no**
/// witness is an R6-era document: it is not refused — refusing it would make this release unable to
/// open the projects the last one wrote — and it does not count as verified evidence about the
/// journal either, which is what the `unwitnessed` sentence in `gx repair`'s report says.
fn verify_head_witness(
    head: &gx_log::PersistedHead,
    key: &gx_witness::PublicKey,
) -> std::result::Result<(), String> {
    let Some(signature) = &head.witness_signature else {
        return Ok(());
    };
    let payload = head
        .witness_payload()
        .map_err(|e| format!("the head's witness has no payload: {e}"))?;
    let envelope = gx_witness::dsse::DsseEnvelope {
        payload_type: HEAD_WITNESS_PAYLOAD_TYPE.to_string(),
        payload,
        signatures: vec![signature.clone()],
    };
    envelope.verify(&key.verifying()).map_err(|e| {
        format!(
            "the head's signed witness — the journal's length and chain head, the `.gx/VERSION` \
             digest and the last leaf — does not verify under {}: {e}. The payload is rebuilt from \
             the numbers in the document, so a signature that was kept while the numbers around it \
             were edited fails here by construction",
            key.key_id()
        )
    })
}

/// 🔴 **R6 / DR-43-11 / `req/229` H-01** — is this pair behind the head this project published?
///
/// The three questions, in the order a cheap one should come first: the tree's **size**, the tree's
/// **root at the published size** (which is what separates "we grew" from "we are a different
/// history of the same length"), and the journal's length and chain over the published prefix.
///
/// The journal walk is skipped when the recorded prefix is longer than the file — that case is
/// already the `JournalShorter` answer — and when the file is legacy, which records no head to
/// compare. Skipping is declared through `gx_log::head::compare`'s `Option<Option<_>>` argument
/// rather than silently folding into a pass.
fn rollback_of(
    floor: &gx_log::HeadFloor,
    ledger: &LedgerStore,
    journal: &EngineJournal,
    version_digest: Option<&str>,
) -> Option<gx_log::RolledBack> {
    let now_tree_size = ledger.log().len();
    let now_root_at = ledger.log().root_at(floor.tree_size);
    let now_journal_len = std::fs::metadata(journal.path())
        .map(|m| m.len())
        .unwrap_or(0);
    let now_journal_head = if floor.journal_head.is_some() && now_journal_len >= floor.journal_len {
        Some(journal.chain_head_through(floor.journal_len))
    } else {
        None
    };
    gx_log::head::compare(
        floor,
        now_tree_size,
        now_root_at,
        now_journal_len,
        now_journal_head.map(|head| head.unwrap_or(None)),
        version_digest,
    )
}

/// What one [`Engine::catch_up`] found: how many records another process had appended, and which of
/// this process's own rows were dropped by the eviction rule.
///
/// Returned rather than logged so that a caller can put the numbers in its start-up line: "I read
/// three records I did not write and dropped one body" is the fact an operator needs when a `GET`
/// that used to answer with a `transformation` starts answering `null` (`req/190` §2-1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaughtUp {
    /// How many records this catch-up folded in.
    pub records: usize,
    /// The rows whose bodies this process dropped, in the order they were named.
    pub evicted: Vec<TransformationId>,
}

impl CaughtUp {
    /// Whether nothing had arrived.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records == 0
    }
}

/// What [`Engine::recover`] did about one transformation.
///
/// A value rather than a log line: AC-043 asks for "at most one `ledger` entry per identical
/// `TransformationId`" (sem: SEM-gx-engine-132) over thirty runs, and a probe that had to parse prose to count them would be measuring
/// the prose.
///
/// `receipt` is returned rather than filed in the engine's table because recovery does not rebuild
/// that table — see [`Engine::recover`] for what it does and does not need.
#[derive(Clone, Debug)]
pub struct Recovered {
    /// The transformation this is about.
    pub transformation: TransformationId,
    /// Which road 43 §7 sent the recovery down.
    pub path: RecoveryPath,
    /// Where it left the transformation.
    pub state: Lifecycle,
    /// The ledger sequence number, where the transformation reached one.
    pub ledger_seq: Option<u64>,
    /// `Some("Appended")` or `Some("AlreadyPresent")` when this recovery appended; `None` when it
    /// found the entry already there (§7-3b) or never reached the ledger.
    pub appended: Option<&'static str>,
    /// 🔴 **M5H4-7**, mechanically: `Some(true)` when the payload rebuilt from the journal digests
    /// to exactly what the ledger already holds for this transformation. `None` when there was
    /// nothing to compare against.
    ///
    /// This is what makes "an idempotent reconstruction, not a double commit" (sem: SEM-gx-engine-133) (43 §7-3b) a measurement: a
    /// re-issued receipt whose payload had drifted would hash differently, and the ledger — whose
    /// key idempotency refuses a second digest under one key (ASM-43-1) — would be the one refusing.
    pub payload_matched: Option<bool>,
    /// The receipt this recovery issued, where it issued one (43 §7-3b's "re-issue if not yet issued"; sem: SEM-gx-engine-134).
    pub receipt: Option<Receipt>,
    /// 🔴 **R5 / `req/227` H-01** — why a [`RecoveryPath::NotResumed`] row was not resumed, and
    /// `None` on every other road.
    ///
    /// A sentence rather than a second enum: the two conditions are already distinguished by the
    /// constant a caller prints, nothing branches on the difference, and `req/38` §132 ruling 2's
    /// "no new surface for a refusal that has a word already" applies to vocabulary as well as to
    /// exit numbers.
    pub refusal: Option<&'static str>,
}

impl Recovered {
    /// 🔴 **R34 / `req/449` H-02** — did this row end in a terminal `Aborted`?
    ///
    /// # Why the question is answered here rather than by the caller
    ///
    /// `gx serve` has to tell an `ApplyWasAnnounced` row that **closed** from one that ended
    /// `Aborted(ApplyFailed)`, because the audit measured the second being announced as a resume.
    /// Written in `gx-cli` that is `matches!(row.state, Lifecycle::Aborted(_))`, and
    /// `crates/gx-canon/tests/authority_boundary.rs`'s Rule 1 (iii) stops exactly that: 42 §1.3-3
    /// puts the state table on the engine side, and a secondary surface that spells a lifecycle
    /// has a second answer to "what state is this in". The rule fired on the first draft of R34's
    /// repair and is right to have: the question belongs to the row, so the row answers it.
    #[must_use]
    pub fn ended_aborted(&self) -> bool {
        matches!(self.state, Lifecycle::Aborted(_))
    }
}

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// 🔴 **FR-M04**: rebuild the verdict counter from the journal, for [`Engine::open`].
///
/// The rule is 43's transition table and not this crate's call graph: three rows issue a
/// `VerdictReceipt`, and this fold names all three.
///
/// * T-4a / T-4b / T-4c — `Verdict { verdict_digest: Some(_) }`, one per gate answer.
/// * T-4e — `Verdict { kind: Admit, verdict_digest: None }`. No gate ran, so the fourth bucket.
/// * T-5 / T-5b — `HumanDecision`, where a person answered.
///
/// 🔴 **This function is why the recount in `tests/ac_vc.rs` is only independent inside a session,
/// and the test file says so in its own header.** Within a process the counter is incremented at
/// the receipt and the test folds the journal, which are two roads; across a restart both start
/// here. Stating it beats a claim of independence that a restart quietly retires.
fn tally_from_the_journal(records: &[EngineJournalRecord]) -> VerdictTally {
    let mut tally = VerdictTally::default();
    for record in records {
        let kind = match record {
            EngineJournalRecord::Verdict {
                kind,
                verdict_digest: Some(_),
                ..
            }
            | EngineJournalRecord::HumanDecision { kind, .. } => Some(*kind),
            EngineJournalRecord::Verdict {
                verdict_digest: None,
                ..
            } => None,
            _ => continue,
        };
        match kind {
            Some(VerdictKind::Admit) => tally.admit += 1,
            Some(VerdictKind::Deny) => tally.deny += 1,
            Some(VerdictKind::Escalate) => tally.escalate += 1,
            None => tally.unverdicted += 1,
        }
    }
    tally
}

/// 43's state machine, running.
///
/// Generic over the two things a test replaces and holding a registry of the third. See the module
/// documentation for why the clock and the seed are arguments instead of type parameters.
pub struct Engine<E: EvidenceSource, C: Canonicalizer = CanonEncoder> {
    journal: EngineJournal,
    blobs: BlobStore,
    /// 🔴 Two-phase escrow's raw-bytes mouth (`req/38` §99 ruling 2-③; sem: SEM-gx-engine-135): the journalled observations,
    /// beside the blobs and deliberately not inside them (`store.rs` argues the separation).
    observations: ObservationStore,
    /// 🔴 The public witness ledger of 42 §3.11, wired in hand 4. A **different file** from the
    /// journal and with a different audience (42 §3.13: "the Ledger is the public witness ledger after
    /// a commit is finalised; the Journal is the engine's internal in-progress pipeline record"; sem: SEM-gx-engine-136).
    ledger: LedgerStore,
    /// 🔴 **FR-M04** (M7 hand 6): the parallel append-only log of signed verdict counts, a third
    /// file beside the journal and the ledger. `gx_log::store::VerdictCheckpointStore` carries
    /// why it is a file of its own.
    verdict_log: VerdictCheckpointStore,
    /// 🔴 **FR-M04**: how many verdicts of each kind this deployment has issued **since the log
    /// began**, and how far the last published checkpoint reached.
    ///
    /// Incremented where the receipt is *issued*, and rebuilt at `open` from the journal. Those
    /// are not the same road, and the difference is the whole reason `ac_vc.rs` can recount from
    /// the journal and call the comparison independent inside a session — see that file's header
    /// for where the independence stops.
    verdicts: VerdictTally,
    /// The window boundary: the counts the published chain has already spoken for. A window's
    /// tally is this subtracted from [`Engine::verdicts`], which is why it is a tally rather than
    /// a single number — a checkpoint states four counts, not one.
    published: VerdictTally,
    adapters: BTreeMap<SubstrateKind, Registered>,
    /// 🔴 Two-phase escrow's optional registry (`req/38` §99 ruling 2-②; sem: SEM-gx-engine-137): which substrates can
    /// complete a `Pending` escrow. Unregistered = the pre-existing behaviour, untouched — every
    /// `Some` from `invert` is a complete escrow. Beside `adapters` rather than inside
    /// [`Registered`] so that registering an adapter says nothing it does not mean.
    completions: BTreeMap<SubstrateKind, Arc<dyn InverseCompletion>>,
    /// 🔴 **DR-46-33 / DR-46-28** (`req/38` §413): which substrates declare where their inputs come
    /// from. Unregistered = the pre-existing behaviour, untouched — `attested_boundary` fills
    /// `input_generation` with `Unknown` for a substrate that declares nothing. Beside
    /// `completions` and `adapters` for `completions`' reason: a separate registry rather than an
    /// eighth `SubstrateAdapter` method (N-08), so registering an adapter says nothing it does not
    /// mean.
    input_stage_declarations: BTreeMap<SubstrateKind, Arc<dyn InputStageDeclaration>>,
    /// 🔴 **R8 / `req/234` H-01** — where T-11's receipt becomes durable, inside the section.
    ///
    /// `None` is the pre-R8 behaviour byte for byte: the receipt is issued, put on the row, and the
    /// caller files it afterwards if it files it at all. See [`CommitReceiptSink`] for why the
    /// product roads register one and why the record order changed with it.
    receipt_sink: Option<Arc<dyn CommitReceiptSink>>,
    /// 🔴 **`req/824` A5** — the engine-internal observation road (the section at the end of this
    /// file): chain heads + `observation_id` replay map, shared with the adapter this engine
    /// lazily registers under `custom:observation` on first ingest. In-memory, like the state it
    /// mirrors; the candidates themselves are in the journal (the module-header restart note).
    observation_road: Arc<ObservationRoad>,
    gate: Gate,
    evidence: E,
    canon: C,
    mode: EnforcementMode,
    posture: FailPosture,
    /// 🔴 **`req/493` §0 / AC-6** — what this process's kernel confinement is, as a receipt says it.
    ///
    /// # Set by the caller, never read out of the environment here
    ///
    /// `gx confine` is a launcher: it takes a Landlock ruleset and `exec`s, so the fact reaches this
    /// process through its environment. **This crate does not read it.** `grep -rn "env::var"
    /// crates/gx-engine/src/` answered zero before this field and answers zero after it, and that is
    /// deliberate rather than incidental: an engine that read a process variable would derive a
    /// signed claim from something no test could set without moving the whole process's state, and
    /// two engines in one binary could not disagree. `crates/gx-cli/src/session.rs`'s `open_engine`
    /// — the writer's door every road to T-11 comes through — parses `GX_CONFINEMENT` and calls
    /// [`Engine::with_confinement`].
    ///
    /// # The default is a statement and not an absence
    ///
    /// `kernel_confined: false, ruleset_hash: None` — "this process was not held by the kernel",
    /// which is true of every process that was not launched under `gx confine`. So every receipt
    /// this binary issues carries a `Some`, and the `None` on the payload keeps its one meaning
    /// (bytes written before the erratum).
    confinement: gx_witness::receipt::ConfinementContext,
    /// The drafts, **with their seeds** (42 §3.13: "re-run under the same seed at replay time"; sem: SEM-gx-engine-138). A set until hand
    /// 3, when Σ made the seed part of what a replay has to reproduce.
    drafted: BTreeMap<IntentId, u64>,
    table: BTreeMap<TransformationId, Entry>,
    /// 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — why [`Rollback::NotAttempted`] was
    /// reached, for the sentence the proxy writes about it.
    ///
    /// Deliberately **not** a component of Σ and deliberately not on [`Entry`]: see
    /// [`NotAttemptedBecause`] for the argument. Nothing reads this to decide anything.
    not_attempted_because: BTreeMap<TransformationId, NotAttemptedBecause>,
    /// 🔴 **M6-02 adopted (a)** (sem: SEM-gx-engine-139): 44 §0's id-resolution, inverted. See [`Engine::resolved`].
    ///
    /// Derived from the journal's `Planned` records and from nothing else, which is why it is a
    /// cache rather than a fifth component of Σ (req/88 Λ1): [`Engine::open`] rebuilds it by
    /// replaying them in order, so a restart cannot make it disagree with the journal, and
    /// [`Engine::sigma`] does not report it.
    ///
    /// # 🔴🔴 **DEFECT-891-1** (`req/895` §2) — why the value is a list
    ///
    /// It was a `BTreeMap<IntentId, TransformationId>` and the replay above ended in
    /// last-write-wins. That is a **function**, and the relation it inverts stopped being one the
    /// day `undo` shipped:
    ///
    /// * `Intent`'s identity is substrate / locator / goal / context / actor. `parents` is not in
    ///   it.
    /// * `Transformation`'s identity **is** quantified over `parents` — [`Engine::undo`] mints
    ///   `T_u` with `parents = vec![T_o]`, and that is 43 T-12's guard.
    ///
    /// So two undos of two different transformations that restore the same bytes at the same
    /// locator under the same context and actor have **one** `IntentId` and **two**
    /// `TransformationId`s. Under last-write-wins the second silently evicted the first, and
    /// `gx-cli`'s `Session::intent_of` — which asks this index "does that intent resolve to this
    /// transformation?" to find the draft a rehydrate needs — then answered `None` for a
    /// transformation whose **signed commit receipt was on disk in the same project**. `gx undo`
    /// on it exited 6, `NOT_FOUND`, "the named object is not here". `req/895` §2 has the
    /// reproduction and the discriminating experiment: changing only `--context` on the second
    /// branch makes the same sequence succeed.
    ///
    /// A `Vec` in journal order is the same fold with the same rule — Λ3(ii)'s "in append order,
    /// last write winning" decides *which one is the latest*, and [`Engine::resolved`] still
    /// answers that. What it no longer does is **discard the others**. Nothing about either
    /// identity moves, so no receipt, ledger leaf or journal record changes by one byte: this
    /// index is not part of Σ, which is exactly why it is the cheap place to repair.
    ///
    /// Entries are unique: a re-`plan` of one intent lands on the same `TransformationId` (43 T-2's
    /// idempotency), and appending it twice would make "how many transformations does this intent
    /// have" a count of retries.
    resolved: BTreeMap<IntentId, Vec<TransformationId>>,
    /// 🔴 **M6-07 adopted (b)** (sem: SEM-gx-engine-140) — [`Engine::table`](Engine) keyed the other way: which rows are about which
    /// subject.
    ///
    /// # Why it exists, with the measurement rather than the reasoning
    ///
    /// 43 §8's conflict check asks "is there an in-flight transformation of **this** subject that
    /// does not commute with mine" (sem: SEM-gx-engine-141), and until this hand [`Engine::conflicting_predecessor`] answered
    /// it by walking every row in `table` and discarding the ones whose subject did not match. The
    /// discard is cheap and the walk is `O(n)`, and `n` is "every transformation this process has
    /// ever seen" (sem: SEM-gx-engine-141) — which for a single-shot CLI is a handful and for `gx serve` is the whole day.
    ///
    /// M5 hand 8 identified that as the shape of AC-066's decay (n-ratio 4.95x against a measured
    /// verify-cost ratio of 5.08x, a 3% agreement), §45 M5H8-16 registered the index against the
    /// firing condition "when a long-lived engine actually grows the table" (sem: SEM-gx-engine-142), and §47 M6-07 adopted (b) fixed the order:
    /// measure through `gx serve` **first**, then index, then measure again. `req/95` carries both
    /// halves; hand 6 deliberately left the engine unindexed so that this hand had a control.
    ///
    /// # What it is not
    ///
    /// **Not part of Σ.** Same standing as `resolved` above: a second reading of the state table,
    /// derived from it, living and dying with it. [`Engine::open`] leaves the table empty (M5H3-5)
    /// and therefore leaves this empty, [`Engine::sigma`] does not report it, and AC-039's
    /// live-vs-replayed comparison is unmoved.
    ///
    /// 🔴 A `BTreeSet` per subject rather than one id: the case the index exists for is **two**
    /// transformations of one object, so a map that held one id per subject would be wrong exactly
    /// where it matters and right everywhere a benchmark looks.
    /// `crates/gx-engine/tests/subject_index.rs` compares this against a full scan.
    by_subject: BTreeMap<Subject, BTreeSet<TransformationId>>,
    /// 43 T-6's two deadlines, in nanoseconds. See [`Engine::with_ttl`].
    verify_ttl: i64,
    escalation_ttl: i64,
    /// Σ's escrow component, live (42 §3.12). Written at T-10b; T-12 moves the status.
    escrow: BTreeMap<TransformationId, EscrowRow>,
    /// 🔴 **M5-09 adopted (a)** (sem: SEM-gx-engine-143): ASM-43-2's `superseded_by`, in the type `store.rs` declares for it.
    supersedes: SupersedeIndex,
    /// Σ's ledger component, live: which transformation reached `Committed` at which leaf.
    ///
    /// 🔴 **M5H3-4** (sem: SEM-gx-engine-144): this is "journal-witnessed frontier" and *not* the ledger's own root, and
    /// hand 4 is where the difference stops being a definition. [`Engine::ledger_agrees`] is the
    /// probe-facing form of the agreement.
    committed: BTreeMap<TransformationId, u64>,
    /// 🔴 **T6 condition ① (Σ-shadow)** — every row the **journal** holds, whether or not this
    /// process holds a body for it (`req/38` §148 ruling 1(i)).
    ///
    /// [`Engine::table`](Engine) is what this process built: it starts empty at [`Engine::open`]
    /// (M5H3-5) and it holds `Transformation` bodies, snapshots and receipts, none of which the
    /// journal records (ASM-9). This is the other half — the fold of every record, kept live, so
    /// that "the journal knows and I do not" stops being the same answer as "there is no such
    /// transformation". `req/182` H-02 measured what the missing half cost: a restarted `gx serve`
    /// answered `404` for its own committed rows and `GET /transformations` returned a page of
    /// nulls.
    ///
    /// **It is not a second state machine.** No transition reads it as a precondition and no
    /// transition writes it directly; [`Engine::journal_append`] is the only writer and it applies
    /// exactly what went on the disk. The read accessors consult it only where the table misses, and
    /// [`Engine::catch_up`]'s eviction rule has one clause. Two rules would be 43's transition table
    /// with a rival, which `req/190` §9-1 names as the way this design fails.
    shadow: SigmaShadow,
    /// 🔴 **R4 / `req/225` H-03** — whether the journal on the disk is still the journal this
    /// process read.
    ///
    /// R3 gave the **ledger** a detector for a rewrite that keeps the file's length
    /// (`gx_log::LedgerStore::tail_unchanged`, plus an unconditional re-open under the lock) and
    /// `req/219` §5(h) wrote down that gx's two files are a pair. The pair had one detector.
    /// `req/225` H-03 measured the missing half end to end: one bit flipped in the tail of a live
    /// project's journal, `/healthz` `200 ledger_agrees:true`, `POST /candidates` `201`,
    /// `GET /ledger/checkpoint` `200` **signed**, and the next start-up refusing to open the
    /// project because its journal witnessed two commits its ledger held one leaf for. From the
    /// middle of the file it was worse: the next start-up quarantined all 1,636 bytes.
    ///
    /// Set on every [`Engine::catch_up`] and [`Engine::catch_up_unlocked`], and read through
    /// [`Engine::journal_intact`] — and, so that no gate has to be told about it twice, folded
    /// into [`Engine::ledger_agrees`], which every writer, `/healthz` and the checkpoint signer
    /// already pass through.
    ///
    /// 🔴 **R32 / `req/392` M-02** — a `bool` until this lane, and now the **reason** with the
    /// bool derived from it ([`Engine::journal_intact`] is `self.journal_departure.is_none()`).
    /// The audit measured a paragraph asserting one of seven folded terms as the cause and being
    /// false about two of the three it drove; a field that carries which term it was is what lets
    /// each face print a sentence that is true of the file in front of it. `req/38` §227's
    /// sibling-sweep rule is why the bool is derived rather than kept beside this: two fields
    /// answering one question is two answers.
    journal_departure: Option<JournalDeparture>,
    /// 🔴 **R4 / `req/225` H-01** — which door this engine came through, kept because
    /// [`Engine::catch_up`] opens the files **again**.
    ///
    /// This is the trap the first version of H-01's repair fell into, and it is worth the field to
    /// say so: opening through [`Engine::open_read_only`] is not enough, because `catch_up` under
    /// the lock re-opens the ledger through `LedgerStore::open` — R3's answer to `req/222` H-05 —
    /// and *that* is a writer's door too. Measured on this lane's own probe: `gx repair` with the
    /// read-only open in place still took a 522-byte ledger to 0, because the very next line
    /// caught up.
    ///
    /// It cannot be inferred from `LedgerStore::is_read_only` either: a live `gx serve` alternates
    /// between `GET`s that re-open the ledger read-only and writes that re-open it through the
    /// writer's door, so "the store is read-only right now" is a fact about the last request and
    /// not about this engine.
    door: Door,
    /// 🔴 **R6 / DR-43-11** — where this project records the furthest it has been, or `None` for a
    /// caller that has not pointed us at one.
    ///
    /// Injected rather than derived: the engine is handed a journal path and derives `.blobs`,
    /// `.ledger` and `.verdicts` beside it, and `.gx/checkpoints/` is one directory **up and
    /// across**. Deriving it here would put `gx_cli::layout`'s knowledge of req/56 §2 inside the
    /// engine, which is the seam `Engine::open`'s own note defends ("a caller who could point them
    /// at different directories…"). So the caller that knows the layout supplies the store, and an
    /// engine opened without one behaves exactly as every engine did before this release —
    /// including every test in this crate, which is the honest denominator rather than a coverage
    /// gap: a project with no recorded head is not protected, and `docs/LIMITS.md` says so.
    head: Option<gx_log::HeadStore>,
    /// 🔴 **R6 / DR-43-11** — the numbers read out of that file when this engine opened.
    ///
    /// A **floor**, not a mirror. It is raised when this process writes a new head and never
    /// lowered, so a second process that rolls the files back while we hold them is compared
    /// against the highest point we have seen rather than against whatever the file says now.
    head_floor: Option<gx_log::HeadFloor>,
    /// 🔴 **R6 / `req/229` H-01** — why this project is refused, when it is refused for going
    /// backwards.
    ///
    /// A sentence rather than a code: `req/38` §156 ruling 2(a) fixes the word at
    /// `LEDGER_DISAGREES` on both faces and minting a second one is a surface addition. What is new
    /// is the `detail`, which names `rolled_back` and the two numbers. Recomputed at every
    /// [`Engine::catch_up`] for the reason `journal_intact` is: a condition evaluated once at open
    /// is a condition a long-lived server stops asking about.
    rolled_back: Option<String>,
    /// 🔴 **R7 / `req/232` H-01** — what the door learned about the head document itself.
    head_authenticity: HeadAuthenticity,
    /// 🔴 **R7 / `req/232` H-01/M-07** — why the recorded head is not one this binary will compare
    /// against: a signature that did not check out, a witness that disagrees with the document it
    /// travels in, or a file that will not parse.
    ///
    /// Folded into [`Engine::ledger_agrees`] beside `rolled_back`, for the same reason and with the
    /// same word (`LEDGER_DISAGREES`): a detector nobody can trust is not an absent detector, and
    /// reading the second as the first is exactly the substitution `req/232` H-01 measured.
    head_invalid: Option<String>,
    /// 🔴 **R9 / `req/236` M-05** — the DSSE `keyid` this project's recorded head was signed under.
    ///
    /// `None` for a project with no head, and for one whose head will not read. Read once at the
    /// door, like every other fact about the head, and used by [`Engine::reissue_receipt`] to tell
    /// a wrong key from a moved world.
    head_key_id: Option<String>,
    /// 🔴 **R7 / `req/232` M-02** — the digest of `.gx/VERSION` as this process last read it.
    version_digest: Option<String>,
    /// 🔴 **R8 / `req/234` H-02 / M-03 / L-03** — the rollback in `rolled_back`, if there is one,
    /// is `RolledBack::VersionChanged` and nothing else.
    ///
    /// Kept as a fact rather than recovered by reading the sentence, because every face that has to
    /// tell the two apart (the start-up refusal, `gx repair`'s remedy, the recovery's per-row
    /// reason) would otherwise be matching on prose.
    declaration_changed: bool,
    /// 🔴 **R7 / `req/38` §171 ruling 2(c)** — the operator accepted a rollback with evidence from
    /// outside the project, so this engine may proceed over one and re-base the head.
    accept_rollback: bool,
    /// 🔴 **R7 / `req/38` §171 ruling 2(c)** — a rollback the caller *may* accept, measured at open.
    ///
    /// `Some` only when the caller passed `accept_rollback` **and** there is something to accept.
    /// It is not an acceptance: the caller still has to check its evidence and call
    /// [`Engine::accept_rollback`], and until it does, this project is refused exactly as any other
    /// rolled-back one is.
    pending_rollback: Option<gx_log::AcceptedRollback>,
    /// 🔴 **R7 / `req/232` M-01** — the rollback this engine is proceeding over, kept so that
    /// [`Engine::record_head`] can write down **what was given up** rather than silently minting a
    /// floor over the shorter tree.
    accepted_rollback: Option<gx_log::AcceptedRollback>,
    /// 🔴 **R7 / `req/232` M-01** — how many rows this process's `recover` has moved forward.
    ///
    /// A project with no recorded head does not get its **first** head minted by a run that a
    /// recovery has just written through: that is how the audit watched a shortened tree become a
    /// project's new attested floor with nothing saying it had ever been higher.
    resumed_rows: usize,
    /// 🔴 **R36 / `req/476` H-01** — what a [`Engine::recover`] that ended in `Err` had already
    /// done to the world, kept so that the `?` cannot take it away with it.
    ///
    /// See [`Engine::recovery_before_error`] for why this is a field rather than a richer return
    /// type, and [`Engine::applied_unrecorded`] for the half that matters most.
    recover_partial: RecoverPartial,
}

/// 🔴 **R36 / `req/476` H-01** — the two facts a `recover` that raised still owes its caller.
///
/// Audit 35 drove the `Err` road on all four write verbs and measured the same thing four times:
/// the delta reached the substrate (`"THIRD PARTY\n"` became `"two\n"`) and **not one byte** was
/// said about it. The mechanism is one `?`: [`Engine::recover`]'s loop is
/// `out.push(self.resume(..)?)`, and [`Engine::resume`] has eight fallible steps *after*
/// `apply_once` has written. When any of them raises, the `Vec<Recovered>` built so far is dropped
/// and the row that wrote is not in it — it never became a `Recovered` at all.
///
/// So there are two losses and this type carries both. `finished` is the rows that completed
/// before the failing one, which the `?` was discarding. `applied_unrecorded` is the row (or rows)
/// whose delta **landed** and whose record did not, which nothing anywhere had.
///
/// # Why not put this in the error
///
/// `Error` is `gx-core`'s and does not know this crate's `Recovered`; widening the return to
/// `Result<Vec<Recovered>, (Vec<Recovered>, Error)>` would change six shipped call sites plus the
/// benches and `crash_probe`, and every one of them would have to opt in to seeing it — which is
/// the shape of the defect being repaired (R35's own module header: "silence is the thing that
/// takes work"). A field on the engine is readable by every caller that still holds the engine,
/// which is all three of the ones that announce, and it is cleared at the top of each `recover`
/// so it can never describe a previous call.
#[derive(Clone, Debug, Default)]
pub struct RecoverPartial {
    /// Every row this `recover` had accumulated when one of them raised.
    ///
    /// 🔴 **R37 / `req/496` M-02** — the name is R36's and the sentence that stood here was
    /// *"rows this `recover` finished"*, which is not what the vector holds. `Engine::recover`'s
    /// outer loop pushes a [`RecoveryPath::Terminal`] row for every commit that was **already
    /// closed when the journal was replayed**, and those rows reach here untouched by any
    /// recovery. Audit 36 measured `gx repair` reporting `finished_before_failure: 1` on a bed
    /// where the recovery closed **nothing**, and the remedy calling that row one that "had
    /// already been finished by this same recovery".
    ///
    /// The vector still holds all of them, because that is what it is read for:
    /// `gx_cli::recovery::announce_recovery` walks it and owes `req/449` H-02's sentence to a row
    /// whose apply was announced and then aborted. The narrower question — how many rows this run
    /// closed — is [`RecoverPartial::closed_by_this_run`], and that is what a count published to
    /// an operator must come from.
    pub finished: Vec<Recovered>,
    /// 🔴 Rows whose delta was applied to the substrate and whose commit could **not** be
    /// recorded, because a step after `apply_once` raised.
    ///
    /// A row is pushed here the moment `apply_once` answers `Ok` and removed the moment the row is
    /// returned as `Recovered`, so what survives an `Err` is exactly the set that wrote and was not
    /// recorded. Empty on every road that applies nothing — 43 §7-3b reads the substrate rather
    /// than re-applying (R33, `req/397` H-01), and a `recover` that raises before reaching an
    /// apply leaves this empty, which is the honest answer for it.
    pub applied_unrecorded: Vec<TransformationId>,
    /// 🔴 **R37 / `req/496` M-01** — rows whose commit **was** recorded and whose head was not.
    ///
    /// The third shape of an interrupted resume, and the one R36's two lists could not express.
    /// [`RecoverPartial::applied_unrecorded`] is emptied for a row the moment
    /// `journal_append(Committed)` returns `Ok`, because from that instant 43 §7-2's terminal
    /// record is on the disk and a sentence saying otherwise is false. `record_head` is the one
    /// write that comes after it, so a row that reaches here has: a delta on the substrate, a
    /// terminal record in the journal, and a signed head that has not moved.
    ///
    /// Audit 36 measured why the distinction is not cosmetic (`req/496` §4-1). On its bed the
    /// operator was told the run "left no terminal record" and that "the row stays resumable", and
    /// was instructed to run a write verb again — which answered `terminal: 2, resumed: 0`, because
    /// the row had been closed by the run that said it had not been. The remedy named no reachable
    /// action.
    ///
    /// A row is removed from this list when `resume` returns it as `Recovered`, so what survives an
    /// `Err` is exactly the set whose head write did not land.
    pub recorded_without_head: Vec<TransformationId>,
}

impl RecoverPartial {
    /// 🔴 **R37 / `req/496` M-02** — how many rows **this** recovery closed as commits.
    ///
    /// [`RecoverPartial::finished`] is everything [`Engine::recover`] had accumulated when it
    /// raised, and that vector is deliberately wider than this number: `announce_recovery` reads it
    /// and owes `req/449` H-02's sentence to a row whose apply was announced and then **aborted**,
    /// which is not a row anybody finished. It is also where the outer loop pushes
    /// [`RecoveryPath::Terminal`] — rows that were already `Committed` when this process opened the
    /// journal, and which no recovery did anything to.
    ///
    /// So the vector keeps every row and the count answers the narrower question, which is the one
    /// `gx repair`'s `finished_before_failure` and its remedy both claim to be answering: audit 36
    /// measured that field saying **1** on a bed where the recovery closed **0**, and the row it
    /// counted had been terminal since before the process started.
    #[must_use]
    pub fn closed_by_this_run(&self) -> usize {
        self.finished
            .iter()
            .filter(|row| {
                // Two conditions, and both are load-bearing. `Terminal` is the outer loop's push
                // for a row that was already `Committed` when the journal was replayed — this
                // recovery read it and did nothing to it. `state == Committed` then excludes the
                // rows that walked a road and did **not** end closed: a refusal
                // (`NothingWasApplied`) and an apply that the adapter then declined
                // (`ApplyWasAnnounced` ending `Aborted(ApplyFailed)`, which is `req/449` H-02's
                // row). Neither was finished, and both stay in `finished` because
                // `gx_cli::recovery::announce_recovery` owes the second one a sentence.
                row.path != RecoveryPath::Terminal && row.state == Lifecycle::Committed
            })
            .count()
    }
}

/// 🔴 **R6 / DR-43-11 + `req/229` H-02** — what a project says about itself, handed to the engine
/// by the caller that can read `.gx/`.
///
/// Two facts, and both are about the **project** rather than about the three append-only files:
/// where the signed head lives, and which journal framing this project has declared. Bundled into
/// one struct rather than added as two parameters for `McpWiring`'s reason (M5H5-1): a parameter
/// every caller ignores is a parameter every caller has to read, and [`ProjectAnchor::none`] is
/// what the ones that ignore it pass.
#[derive(Clone, Debug, Default)]
pub struct ProjectAnchor {
    /// `.gx/checkpoints/head.json` and the origin its checkpoints are signed under.
    pub head: Option<gx_log::HeadStore>,
    /// What `.gx/VERSION` records about this project's journal framing, or `None` for a project
    /// that has never said.
    pub declared_format: Option<crate::replay::JournalFormat>,
    /// 🔴 **R7 / `req/232` H-01** — where a door finds the public key a recorded head was signed
    /// under, or `None` for a caller that holds no key store.
    ///
    /// Injected for [`ProjectAnchor::head`]'s reason: `~/.gx/keys/` is req/56 §3's directory and
    /// the engine has never known where it is. `None` is the honest pre-R7 answer and it produces
    /// `head_authenticity: "unverified"` rather than a silent pass — the audit's finding was
    /// precisely that "a head is present" was being reported as if it meant "a head that checks
    /// out".
    pub keys: Option<Arc<dyn HeadKeys>>,
    /// 🔴 **R7 / `req/232` M-02** — hex of the digest of `.gx/VERSION` as it stands right now.
    ///
    /// Supplied by the caller that can read the layout, compared against the digest the recorded
    /// head was written with. `None` where the caller has no layout, which is every engine opened
    /// without an anchor.
    pub version_digest: Option<String>,
    /// 🔴 **R7 / `req/38` §171 ruling 2(c)** — the operator has decided, explicitly and with a
    /// document from outside this project, to take the shorter tree.
    ///
    /// `false` everywhere except `gx repair --accept-rollback --against <FILE>`. It does not
    /// disable the comparison — the rollback is still measured and still reported — it makes the
    /// engine willing to proceed over it and to record what was accepted.
    pub accept_rollback: bool,
    /// 🔴 **R12 / `req/242` H-01 (d)** — may the writer's door create the journal.
    ///
    /// 🔴 **R13 / `req/244` L-07** — [`crate::replay::JournalCreation::Refused`] by [`Default`].
    ///
    /// It was `Permitted`, and `req/244` L-07 measured what was actually holding the barrier up: a
    /// census that greps this crate's callers for the literal `ProjectAnchor {` and for the literal
    /// `JournalCreation::Permitted`. One `..Default::default()` produces neither string and turns
    /// the barrier off. `gx-cli` still writes `Refused` by name on every road it builds, so nothing
    /// about this binary changes; what changes is what an anchor that names nothing means, and it
    /// now means the safe thing. The one place that may bring `.gx/ledger/journal` into existence
    /// is `gx_cli::declaration::DeclarationWriter::initialise`, and a journal that was **lost** is
    /// a fact `gx repair` reports rather than a file the next `gx submit` invents (`req/242` H-01
    /// (d)).
    pub journal_creation: crate::replay::JournalCreation,
}

impl ProjectAnchor {
    /// A project that has recorded nothing about itself — the pre-R6 behaviour, exactly.
    ///
    /// 🔴 **R13 / `req/244` L-07 + M-02 (v)** — with one field that is no longer "pre-R6": since
    /// R13 this carries [`crate::replay::JournalCreation::Refused`], because `Default` does.
    ///
    /// The audit's M-02 (v) named this constructor as a road back into journal creation that the
    /// census cannot see — it produces neither the string `ProjectAnchor {` nor the string
    /// `JournalCreation::Permitted`, so a `gx-cli` that reached for it would restore the behaviour
    /// `req/242` H-01 (d) closed with both greps still green. It now fails safe. A caller that
    /// wants the old meaning writes the word: `ProjectAnchor { journal_creation:
    /// JournalCreation::Permitted, ..ProjectAnchor::none() }`, which is what [`Engine::open`] does
    /// one screen up.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

/// 🔴 **R7 / `req/232` H-01** — where a door asks for the public key a head was signed under.
///
/// A trait rather than a path because req/56 §3 puts the key store in the operator's home directory
/// and 41 §6's boundary keeps that knowledge in the binary that owns the layout. The engine asks by
/// `key_id` — the one the document itself names — and gets back a key or nothing; **nothing is not
/// a failure**, it is a deployment that cannot verify, and it is reported as `unverified` rather
/// than folded into either a pass or a refusal.
pub trait HeadKeys: std::fmt::Debug + Send + Sync {
    /// The public key for `key_id`, if this environment holds one.
    fn verifying(&self, key_id: &str) -> Option<gx_witness::PublicKey>;
}

/// 🔴 **R7 / DR-43-11 (b)** — the DSSE payload type the head's witness is signed under.
///
/// A type of its own beside `gx_witness::dsse::CHECKPOINT_PAYLOAD_TYPE` for E-M2-26's reason, which
/// is the reason DSSE has payload types at all: one key signing two byte formats with nothing
/// between them saying which is which is a separation that holds by accident of two encodings.
pub const HEAD_WITNESS_PAYLOAD_TYPE: &str = "application/vnd.glovrex.head-witness+json";

/// 🔴 **R7 / `req/232` H-01** — what a door learned about the head document in front of it.
///
/// Four answers rather than a boolean, because `req/232` H-01's whole finding is that
/// `head_recorded: true` was being read as "the detector is sound" when it only ever meant "a file
/// was there". The names are the ones every face prints (`gx repair`'s `head_authenticity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadAuthenticity {
    /// No head file: this project has never recorded where it reached.
    Absent,
    /// The file is there and this environment holds no key for the id it names, so nothing about it
    /// was checked. **Not** a pass.
    Unverified,
    /// The checkpoint's signature checks out under the key it names, and — where the head carries
    /// one — so does the witness over the local numbers.
    Verified,
    /// A signature that did not check out, a witness that disagrees with the document it is in, or
    /// a file that will not parse. Fails closed: every gate refuses and the reader's door still
    /// opens so that `gx repair` can say so (`req/232` M-07).
    Refuted,
}

impl HeadAuthenticity {
    /// The word every face prints.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            HeadAuthenticity::Absent => "absent",
            HeadAuthenticity::Unverified => "unverified",
            HeadAuthenticity::Verified => "verified",
            HeadAuthenticity::Refuted => "refuted",
        }
    }
}

/// 🔴 **DR-43-7 / R4** — which door an [`Engine`] was opened through.
///
/// `gx_log::LedgerStore`, `gx_engine::store::EngineJournal` and
/// `gx_log::VerdictCheckpointStore` have each carried an `open`/`open_read_only` pair since
/// DR-43-7; the engine that holds all three had only the writer's. `req/225` H-01 measured what
/// that cost one level up: `gx repair` without `--yes` — the mode 44 §1.2, the CLI's own help and
/// `repair.rs`'s module documentation all describe as writing nothing — opened the writer's door,
/// which quarantines a tail that will not replay and then **cuts it**, and took a 522-byte ledger
/// to 0 bytes. Beside a live `gx serve`, `/healthz` went from `200` to `500` on the strength of a
/// diagnosis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Door {
    /// Create what is absent, replay, and repair a torn tail before returning. The caller is about
    /// to append and the append has to land where the record sequence actually reached.
    Writer,
    /// Open what is there, replay it, and change nothing — no create, no truncate, no repair. A
    /// torn tail is *counted* and reported; the caller decides.
    Reader,
}

/// Written by hand rather than derived, because `Arc<dyn SubstrateAdapter>` has no `Debug` (41 §4
/// asks the trait for seven methods and no formatting) and because a derived one would print two
/// collaborators' internals in place of the thing an operator wants: what is registered, how many
/// transformations are in flight, and which of DR-2's two axes this deployment set.
impl<E: EvidenceSource, C: Canonicalizer> std::fmt::Debug for Engine<E, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("journal", &self.journal.path())
            .field("records", &self.journal.len())
            .field("blobs", &self.blobs.root())
            .field("ledger", &self.ledger.path())
            .field("leaves", &self.ledger.log().len())
            .field("adapters", &self.adapters.keys().collect::<Vec<_>>())
            .field("mode", &self.mode)
            .field("posture", &self.posture)
            .field("verify_ttl", &self.verify_ttl)
            .field("escalation_ttl", &self.escalation_ttl)
            .field("drafts", &self.drafted.len())
            .field("transformations", &self.table.len())
            .field("superseded", &self.supersedes.len())
            .finish()
    }
}

impl<E: EvidenceSource> Engine<E, CanonEncoder> {
    /// Open the journal at `path` and rebuild what it holds.
    ///
    /// 43 §7-1's "replay at start-up" (sem: SEM-gx-engine-145). This rebuilds the **draft phase** from the journal -- every
    /// `IntentId` a `DraftCreated` names, with the seed it was submitted under -- and opens the blob
    /// store beside it.
    ///
    /// It does **not** rebuild the in-flight table, and hand 3 does not change that: a row holds a
    /// `Transformation` and an `ObjectSnapshot`, and the journal holds names and digests rather than
    /// bodies (ASM-9). What hand 3 adds is that the *state* those rows would carry is recoverable
    /// without them -- [`Engine::sigma`] and [`crate::replay::reconstruct`] agree on Σ, which is what
    /// **E-M5-2** defines AC-039's "resulting state" (sem: SEM-gx-engine-146) to be. Resuming an in-flight commit after a restart is
    /// hand 5's, and 43 §7-3c's inputs are exactly the two things that survive: `Fingerprint₀` from
    /// the journal and the delta body from the blob store.
    ///
    /// # Where the blobs and the ledger live
    ///
    /// `<journal>.blobs` and `<journal>.ledger`, beside the journal. Derived rather than taken as
    /// arguments because the three files are one engine's state: a caller who could point them at
    /// different directories could open a journal against another engine's bodies, and every
    /// `delta_cid` in it would resolve to something that was never planned. The ledger is a
    /// separate **file** and not a separate idea of the same thing (42 §3.13).
    ///
    /// 🔴 Hand 4 wires the ledger and **does not** rebuild the in-flight table from it. The ledger
    /// replays itself (`gx_log::LedgerStore::open`), so the public log survives a restart; Σ's view
    /// of it does not, because that view lives in a table `open` still leaves empty (M5H3-5, hand
    /// 5's window). Reporting a rebuilt ledger component beside an empty state table would be a
    /// partly-rebuilt Σ presented as a whole one.
    ///
    /// # Errors
    /// [`Error::Io`] if the journal cannot be opened, read, truncated or synced, or if the blob
    /// directory cannot be created. [`Error::Ledger`] if the ledger cannot be opened or replayed.
    pub fn open(path: impl AsRef<std::path::Path>, gate: Gate, evidence: E) -> Result<Self> {
        // 🔴 **R13 / `req/244` L-07** — this door names `Permitted`, because it always meant it.
        //
        // `JournalCreation`'s `Default` moved to `Refused` in R13, so an anchor a caller built with
        // `..Default::default()` no longer turns journal creation on by accident. That is a change
        // about anchors a caller **writes**; this function is the library door whose whole contract
        // is "open a journal at this path, making one if it is not there", and it keeps that
        // contract by saying so. An embedder calling `Engine::open` sees no difference at all.
        //
        // Said as a constant rather than inherited from a default, which is the whole lesson: the
        // permission is now visible at every site that has it.
        Self::open_through(
            path,
            gate,
            evidence,
            Door::Writer,
            ProjectAnchor {
                journal_creation: crate::replay::JournalCreation::Permitted,
                ..ProjectAnchor::none()
            },
        )
    }

    /// 🔴 **R6 / DR-43-11** — the same opening, with what the project records about itself.
    ///
    /// # Errors
    /// As [`Engine::open`].
    pub fn open_anchored(
        path: impl AsRef<std::path::Path>,
        gate: Gate,
        evidence: E,
        anchor: ProjectAnchor,
    ) -> Result<Self> {
        Self::open_through(path, gate, evidence, Door::Writer, anchor)
    }

    /// 🔴 **R6 / DR-43-11** — the reader's door, with what the project records about itself.
    ///
    /// # Errors
    /// As [`Engine::open_read_only`].
    pub fn open_read_only_anchored(
        path: impl AsRef<std::path::Path>,
        gate: Gate,
        evidence: E,
        anchor: ProjectAnchor,
    ) -> Result<Self> {
        Self::open_through(path, gate, evidence, Door::Reader, anchor)
    }

    /// 🔴 **R4 / `req/225` H-01** — the same opening, through [`Door::Reader`].
    ///
    /// The three append-only files are opened with `open_read_only`, so nothing is created and no
    /// torn tail is quarantined or cut. The blob and observation **directories** are still created
    /// if absent, and that is the one write this door does not prevent: they hold no record, a
    /// project that has ever planned anything already has them, and refusing to make a directory
    /// would turn a diagnosis into a second error message about a third thing. Stated rather than
    /// discovered.
    ///
    /// An engine opened this way still has every read on it. It also still has every **write**,
    /// which the type system does not prevent and the files do: `EngineJournal::append` and
    /// `LedgerStore::append` refuse a store opened read-only (DR-43-7), so a caller that tried
    /// would get a refusal rather than a silent success. The caller this exists for —
    /// `gx repair`'s report mode — takes neither road.
    ///
    /// # Errors
    /// [`Error::Io`] if the journal is **absent** or cannot be read (absent is a refusal here and a
    /// creation on the writer's door), [`Error::Ledger`] if the ledger or the verdict chain cannot
    /// be opened or replayed.
    pub fn open_read_only(
        path: impl AsRef<std::path::Path>,
        gate: Gate,
        evidence: E,
    ) -> Result<Self> {
        Self::open_through(path, gate, evidence, Door::Reader, ProjectAnchor::none())
    }

    /// The body of [`Engine::open`] and [`Engine::open_read_only`].
    ///
    /// # Errors
    /// As the two above.
    fn open_through(
        path: impl AsRef<std::path::Path>,
        gate: Gate,
        evidence: E,
        door: Door,
        anchor: ProjectAnchor,
    ) -> Result<Self> {
        let path = path.as_ref();
        let mut blobs_root = path.as_os_str().to_os_string();
        blobs_root.push(".blobs");
        let blobs_root = std::path::PathBuf::from(blobs_root);
        // 🔴 **R5 / `req/227` M-02** — the reader's door names these directories and does not make
        // them. See [`BlobStore::open_read_only`].
        let blobs = match door {
            Door::Writer => BlobStore::open(blobs_root),
            Door::Reader => BlobStore::open_read_only(blobs_root),
        }?;
        // Two-phase escrow's observations: `<journal>.observations`, beside the blobs, for the
        // same one-engine's-state reason the other two derived paths give.
        let mut observations_root = path.as_os_str().to_os_string();
        observations_root.push(".observations");
        let observations_root = std::path::PathBuf::from(observations_root);
        let observations = match door {
            Door::Writer => ObservationStore::open(observations_root),
            Door::Reader => ObservationStore::open_read_only(observations_root),
        }?;
        // 🔴 **R6 / `req/229` M-04** — the journal is opened **before** the ledger, and what it
        // found decides which door the ledger comes through.
        //
        // The audit measured a `gx repair --yes` that had already decided the journal was not
        // evidence (`journal_intact: false`, `journal_chain_break_at: 2754`) and then, in the same
        // run, cut the ledger from 522 bytes to 174 to match the number of commits that journal
        // witnesses — moving two of three leaves, one of them undamaged, into `.torn.`. "I do not
        // trust this file" and "I will trim the other file to agree with it" cannot both be true of
        // one run. DR-43-9 (c-3) already says a chain break is never truncated; this extends the
        // same sentence to the *other* file of the pair, which is where the cutting actually
        // happened.
        //
        // The reorder is what makes it expressible: before this, the ledger was opened first and
        // the journal's condition was not yet known.
        let journal = match door {
            Door::Writer => EngineJournal::open_declared_creating(
                path,
                anchor.declared_format,
                anchor.journal_creation,
            )?,
            Door::Reader => EngineJournal::open_read_only_declared(path, anchor.declared_format)?,
        };
        let journal_suspect = !journal.chain_intact() || journal.downgraded();
        let mut ledger_path = path.as_os_str().to_os_string();
        ledger_path.push(".ledger");
        let ledger_path = std::path::PathBuf::from(ledger_path);
        let ledger = match (door, journal_suspect) {
            (Door::Writer, false) => LedgerStore::open(&ledger_path),
            // 🔴 **R6 / `req/229` M-02** — and an absent ledger is read as an empty one on every
            // door that is not repairing, so that a diagnosis opens on at least the set of projects
            // a repair opens on.
            (Door::Writer, true) | (Door::Reader, _) => {
                LedgerStore::open_read_only_or_absent(&ledger_path)
            }
        }
        .map_err(|e| Error::Ledger {
            action: "open",
            detail: e.to_string(),
        })?;
        let mut verdict_path = path.as_os_str().to_os_string();
        verdict_path.push(".verdicts");
        let verdict_path = std::path::PathBuf::from(verdict_path);
        // 🔴 **R4** — the reader's door does not create the chain. Every project whose journal
        // exists has been through the writer's door at least once and therefore has this file;
        // ~~one that does not is reported as an I/O refusal naming the path, which is the truth,
        // rather than being silently given a new empty chain by a verb that claims to write
        // nothing.~~
        //
        // 🔴 **R5 / `req/227` M-04** — the struck half was true about the file and wrong about the
        // verb. A project missing `.gx/ledger/journal.verdicts` answered `INTERNAL` to
        // `gx repair` (the *report*) and `0` to `gx repair --yes`, which grew the file back: the
        // diagnosis opened on a narrower set of projects than the repair, and the one project that
        // needs a diagnosis most is the one that is missing a file. An absent chain is now read as
        // an **empty** chain — no file is created, and the absence is a fact the report carries —
        // because "this project has no verdict checkpoints" and "this project has a file holding
        // none" are the same answer to every question asked of it here.
        let verdict_log = match (door, journal_suspect) {
            (Door::Writer, false) => VerdictCheckpointStore::open(&verdict_path),
            (Door::Writer, true) | (Door::Reader, _) => {
                VerdictCheckpointStore::open_read_only_or_absent(&verdict_path)
            }
        }
        .map_err(|e| Error::Ledger {
            action: "open the verdict checkpoint log",
            detail: e.to_string(),
        })?;
        // 🔴 **FR-M04**: the counter is rebuilt from the journal, the same way `drafted` and
        // `resolved` are, and for the same reason — `open` leaves the table empty, so a counter
        // that lived only in the table would reopen window zero after every restart and publish a
        // chain full of holes. What the rebuild costs is written in `ac_vc.rs`'s header: across a
        // restart the producer and the recount share a source.
        let verdicts = tally_from_the_journal(journal.records());
        // Folded from the chain rather than read off its last entry: the last entry carries its
        // own window, not the sum of the ones before it.
        let published = verdict_log
            .checkpoints()
            .iter()
            .fold(VerdictTally::default(), |acc, c| VerdictTally {
                deny: acc.deny + c.tally.deny,
                admit: acc.admit + c.tally.admit,
                escalate: acc.escalate + c.tally.escalate,
                unverdicted: acc.unverdicted + c.tally.unverdicted,
            });
        let drafted = journal
            .records()
            .iter()
            .filter_map(|r| match r {
                EngineJournalRecord::DraftCreated {
                    intent_id,
                    rng_seed,
                    ..
                } => Some((*intent_id, *rng_seed)),
                _ => None,
            })
            .collect();
        // 🔴 **M6-02 adopted (a)** (sem: SEM-gx-engine-147), rebuilt the same way `drafted` is: from the journal, in append order,
        // last write winning. That order is the rule req/88 Λ3(ii) asks for — see
        // [`Engine::resolved`] — and taking it from the journal rather than from the table is what
        // makes the answer survive a restart, since `open` deliberately leaves the table empty.
        // 🔴 **DEFECT-891-1** (`req/895` §2) — `.collect()` into a `BTreeMap` here was the
        // last-write-wins that dropped a branch. The fold is the same one, in the same order; what
        // changed is that an entry for an intent that already has one is **appended** rather than
        // substituted. `Engine::resolved` still answers the last, so every reader that wanted the
        // latest still gets it.
        let mut resolved: BTreeMap<IntentId, Vec<TransformationId>> = BTreeMap::new();
        for record in journal.records() {
            if let EngineJournalRecord::Planned {
                intent_id,
                transformation,
                ..
            } = record
            {
                let seen = resolved.entry(*intent_id).or_default();
                if !seen.contains(transformation) {
                    seen.push(*transformation);
                }
            }
        }
        // 🔴 **T6 condition ① (Σ-shadow), at the one place a process learns what is already on
        // disk** (`req/38` §148). `drafted` and `resolved` above are journal-derived indexes and
        // have been since M6-02/FR-M04; this is the same road for the rest of Σ, and the reason it
        // was not taken earlier is that until DR-43-2 nobody had measured what its absence cost
        // (`req/182` H-02: `404` after a restart for a row the journal held).
        let mut shadow = SigmaShadow::default();
        for record in journal.records() {
            shadow.fold(record);
        }
        // 🔴 **M-12** (`req/182`): `ledger_agrees` compares the journal-witnessed frontier against
        // the ledger's own tree, and a frontier rebuilt from nothing is empty — so on every restart
        // the check answered `false` for a ledger that was perfectly sound, and the gate `req/38`
        // §148 asks for would have refused every start. The frontier is journal-derived exactly like
        // `drafted`, so it is rebuilt here rather than waiting for `recover` to supply it; what
        // `recover` still owes is the `Committing` window (43 §7-3), which needs a key and an
        // adapter and therefore cannot happen at `open` (M5H5-1).
        let committed: BTreeMap<TransformationId, u64> =
            shadow.committed().map(|(id, seq)| (*id, *seq)).collect();
        // 🔴 **R5 / `req/227` M-01 / DR-43-9** — see the field's comment below.
        // 🔴 **R6 / `req/229` H-02** — a declared-chained project holding a legacy file folds in
        // here, beside the chain break, because it is the same fact: the journal in front of us is
        // not the journal this project wrote. No new `gx_code`, no new exit — 43 §7.6's "one word"
        // is unmoved.
        // 🔴 **R32 / `req/392` M-02** — the same three terms, handed to the same function the
        // catch-up hands its seven to, so that "is this journal the journal" is spelled once.
        //
        // The four terms this fold does **not** ask are passed as the value that does not depart,
        // and that is a statement about this door rather than a convenience: `read_offset`,
        // `tail_unchanged` and `prefix_intact` are all comparisons against *what this process has
        // already read*, and at `open` this process has read nothing yet. They are asked on the
        // very next catch-up, which every road that prints a sentence goes through. The fold is
        // therefore byte-for-byte the one R6 left here — what is new is that it now says which of
        // the three it was.
        let departure_at_open = JournalTerms {
            not_from_a_newer_gx: true,
            not_downgraded: !journal.downgraded(),
            chain_intact: journal.chain_intact(),
            not_shorter_than_read: true,
            tail_unchanged: true,
            prefix_intact: true,
            no_unrepaired_torn_tail: matches!(door, Door::Writer)
                || journal.recovery().torn_tail_bytes == 0,
        }
        .departure();
        // 🔴 **R6 / DR-43-11** — the first of the two places the floor is compared. The second is
        // `read_to_the_end`, so a server that has been up for a week asks again on every write.
        // 🔴 **R7 / `req/232` H-01** — and what the *document* is, before its numbers are believed.
        let reading = match &anchor.head {
            Some(store) => read_head(
                store,
                anchor.keys.as_ref(),
                anchor.version_digest.as_deref(),
            ),
            None => HeadReading::default(),
        };
        let head_floor = reading.floor;
        let head_authenticity = reading.authenticity.unwrap_or(HeadAuthenticity::Absent);
        let head_invalid = reading.invalid;
        let head_key_id = reading.key_id;
        let rolled_back_why = head_floor.as_ref().and_then(|floor| {
            rollback_of(floor, &ledger, &journal, anchor.version_digest.as_deref())
        });
        // 🔴 **R8 / `req/234` M-03 + L-03** — which of the five it is, kept beside the sentence.
        let declaration_changed = matches!(
            rolled_back_why,
            Some(gx_log::RolledBack::VersionChanged { .. })
        );
        // 🔴 **R7 / `req/38` §171 ruling 2(c)** — an accepted rollback is measured and then
        // proceeded over, and what was given up is kept so that the next head says it.
        // The rollback is **not** cleared here: an operator who asked to accept one still gets the
        // whole diagnosis, and a run that turns out to have the wrong evidence must not have
        // hidden the finding on its way to refusing. What the flag buys is a *pending* acceptance
        // that `Engine::accept_rollback` can honour once the caller has checked its document.
        let pending_rollback = match (&rolled_back_why, anchor.accept_rollback, &head_floor) {
            (Some(_), true, Some(floor)) => Some(gx_log::AcceptedRollback {
                was_tree_size: floor.tree_size,
                was_root_hash: floor.root_hash.to_text(),
                against: String::new(),
                at: 0,
            }),
            _ => None,
        };
        let rolled_back = rolled_back_why.map(|why| why.detail());
        Ok(Self {
            journal,
            blobs,
            observations,
            ledger,
            verdict_log,
            verdicts,
            published,
            adapters: BTreeMap::new(),
            completions: BTreeMap::new(),
            input_stage_declarations: BTreeMap::new(),
            receipt_sink: None,
            observation_road: Arc::new(ObservationRoad::default()),
            gate,
            evidence,
            canon: CanonEncoder,
            mode: EnforcementMode::default(),
            posture: FailPosture::default(),
            // 🔴 **`req/493` §0 / AC-6** — the unconfined statement, not an absence. See the field.
            confinement: gx_witness::receipt::ConfinementContext::unconfined(),
            verify_ttl: DEFAULT_VERIFY_TTL_NANOS,
            escalation_ttl: DEFAULT_ESCALATION_TTL_NANOS,
            drafted,
            resolved,
            table: BTreeMap::new(),
            not_attempted_because: BTreeMap::new(),
            by_subject: BTreeMap::new(),
            escrow: BTreeMap::new(),
            supersedes: SupersedeIndex::new(),
            committed,
            shadow,
            // 🔴 ~~**R4 / `req/225` H-03** — an open has just read the file, so at this instant the
            // journal on the disk **is** the journal this process holds. Every later answer comes
            // from `read_to_the_end`, which asks the file.~~
            //
            // 🔴 **R5 / `req/227` M-01** — the struck sentence is what made this field structurally
            // unable to be `false` in `gx repair`: the verb opens, catches up once, and reads the
            // answer, so "the file has not moved since I read it" is a tautology on that road. What
            // an open actually learnt is now what it says — whether the chain the file carries
            // verifies end to end (DR-43-9), and, on the reader's door, whether every byte on the
            // file came back as a record. The writer's door repairs a torn tail on its way through
            // (DR-43-7), so there the count is history rather than damage.
            journal_departure: departure_at_open,
            door,
            head: anchor.head,
            head_floor,
            rolled_back,
            head_authenticity,
            head_invalid,
            head_key_id,
            version_digest: anchor.version_digest,
            declaration_changed,
            accept_rollback: anchor.accept_rollback,
            pending_rollback,
            accepted_rollback: None,
            resumed_rows: 0,
            recover_partial: RecoverPartial::default(),
        })
    }
}

impl<E: EvidenceSource, C: Canonicalizer> Engine<E, C> {
    /// Replace the canonicalizer (AC-033's abnormal case; see [`Canonicalizer`]).
    #[must_use]
    pub fn with_canonicalizer<C2: Canonicalizer>(self, canon: C2) -> Engine<E, C2> {
        Engine {
            journal: self.journal,
            blobs: self.blobs,
            observations: self.observations,
            ledger: self.ledger,
            verdict_log: self.verdict_log,
            verdicts: self.verdicts,
            published: self.published,
            adapters: self.adapters,
            completions: self.completions,
            input_stage_declarations: self.input_stage_declarations,
            receipt_sink: self.receipt_sink,
            observation_road: self.observation_road,
            gate: self.gate,
            evidence: self.evidence,
            canon,
            mode: self.mode,
            posture: self.posture,
            // 🔴 **`req/493` §0 / AC-6** — carried across, so a road that rebuilds the engine does
            // not silently answer "unconfined" for a process the kernel is still holding.
            confinement: self.confinement.clone(),
            verify_ttl: self.verify_ttl,
            escalation_ttl: self.escalation_ttl,
            drafted: self.drafted,
            resolved: self.resolved,
            table: self.table,
            not_attempted_because: self.not_attempted_because,
            by_subject: self.by_subject,
            escrow: self.escrow,
            supersedes: self.supersedes,
            committed: self.committed,
            shadow: self.shadow,
            journal_departure: self.journal_departure,
            door: self.door,
            head: self.head,
            head_floor: self.head_floor,
            rolled_back: self.rolled_back,
            head_authenticity: self.head_authenticity,
            head_invalid: self.head_invalid,
            head_key_id: self.head_key_id,
            version_digest: self.version_digest,
            declaration_changed: self.declaration_changed,
            accept_rollback: self.accept_rollback,
            pending_rollback: self.pending_rollback,
            accepted_rollback: self.accepted_rollback,
            resumed_rows: self.resumed_rows,
            recover_partial: self.recover_partial,
        }
    }

    /// Register an adapter for a substrate (**M5-07 adopted (a)**; sem: SEM-gx-engine-148).
    ///
    /// The registry is the whole of N-13 as a design rather than as a rule: gx-engine declares no
    /// adapter dependency, so the only way a substrate reaches this engine is a caller putting one
    /// here. 43 §1 gives no home for "the substrate is unknown" (sem: SEM-gx-engine-149), so a `plan` for an unregistered
    /// substrate is [`Error::NotFound`] and never a state.
    ///
    /// `SubstrateKind::Custom` dispatch is **not** interpreted (req/78 N-10, ASM-1): a custom kind
    /// registers and resolves by its string like any other, and no rule is attached to it.
    ///
    /// # 🔴 The version, and why it is an argument (hand 4, **M5H4-4**)
    ///
    /// 42 §3.9's `Environment.adapter_version` is required and 41 §4's trait cannot answer it: the
    /// seven methods report a kind, a snapshot, a plan, a precondition, an application, an inverse
    /// and a commutation, and N-07 forbids an eighth. The registrant knows which build it wired in,
    /// so the registrant says. A default of `"unknown"` was the alternative and is the same mistake
    /// as an empty verdict digest — a made-up value in a signed provenance record.
    pub fn register_adapter(
        &mut self,
        adapter: Arc<dyn SubstrateAdapter>,
        version: impl Into<String>,
    ) {
        self.adapters.insert(
            adapter.kind(),
            Registered {
                adapter,
                version: version.into(),
            },
        );
    }

    /// 🔴 Register a late-binding escrow completion for a substrate (two-phase escrow, `req/38`
    /// §99 ruling 2-②; sem: SEM-gx-engine-150).
    ///
    /// **Optional**, and the option is the design: an engine with no completion registered for a
    /// substrate treats every `Some` answer of `invert` as a complete escrow — the pre-existing
    /// behaviour, byte for byte. A registered one is asked two questions and nothing else:
    /// "is this escrow partial" at T-10b, and "complete it from this observation" (sem: SEM-gx-engine-151) once, after a
    /// successful apply, inside the same critical section. A separate registry rather than an
    /// eighth `SubstrateAdapter` method, because N-08 fixes that trait at seven and
    /// `adapter_spec.rs` measures it.
    pub fn register_completion(
        &mut self,
        kind: SubstrateKind,
        completion: Arc<dyn InverseCompletion>,
    ) {
        self.completions.insert(kind, completion);
    }

    /// 🔴 Register a substrate's input-generation declaration (DR-46-33 / DR-46-28, `req/38` §413).
    ///
    /// **Optional**, and the option is the design: an engine with no declaration for a substrate
    /// attests `input_generation: unknown` — v0's behaviour, byte for byte
    /// (`Engine::joined_input_generation` answers `Unknown` for an unregistered kind). A registered
    /// one is asked one question, at plan time: "where does an input for this substrate come from",
    /// whose answer the engine joins with the transformation's `Actor` and journals on the `Planned`
    /// record. A separate registry rather than an eighth `SubstrateAdapter` method, because N-08
    /// fixes that trait at seven and `adapter_spec.rs` measures it — `register_completion`'s reason.
    pub fn register_input_stage_declaration(
        &mut self,
        kind: SubstrateKind,
        declaration: Arc<dyn InputStageDeclaration>,
    ) {
        self.input_stage_declarations.insert(kind, declaration);
    }

    /// 🔴 **R8 / `req/234` H-01** — hand the engine the archive its commit receipts go to.
    ///
    /// One sink for the whole engine rather than one per substrate: a receipt is 42 §3.10's
    /// document about a transformation and the archive is req/56 §2's one directory, so a registry
    /// keyed by `SubstrateKind` would be inventing a distinction neither specification makes.
    ///
    /// Registering one changes **when** the commit is finished, not what it means: see
    /// [`CommitReceiptSink`] for the record order and `req/38` §154 for the rule.
    pub fn register_receipt_sink(&mut self, sink: Arc<dyn CommitReceiptSink>) {
        self.receipt_sink = Some(sink);
    }

    /// Whether this engine files its own commit receipts (**R8**).
    ///
    /// The honest half of the sentence above: a caller with no sink still has to file the receipt
    /// itself, and `gx repair`'s `receipts_missing` is what says whether anybody did.
    #[must_use]
    pub fn files_receipts(&self) -> bool {
        self.receipt_sink.is_some()
    }

    /// 🔴 **R8 / `req/234` H-01** — file one receipt, or fail the commit that produced it.
    ///
    /// `Ok(())` for an engine with no sink, which is the pre-R8 road and is declared rather than
    /// silently equivalent: the caller is then the writer and the window `req/234` measured is that
    /// caller's to close.
    ///
    /// 🔴 **R16 / `req/262` M-01** — and the sentence no longer asserts a cause it did not measure.
    /// It ended "What to fix: the write permission on `.gx/receipts/`, or the disk it is on", and
    /// the audit drove the HTTP commit road on a project where `.gx/receipts` was a one-byte file:
    /// the operating system said `File exists (os error 17)`, which is neither of the two things
    /// named. `req/244` H-03's standing lesson is that a refusal asserting a cause sends an
    /// operator to the wrong place; the string the archive returned is already in this message, so
    /// what replaces the guess is a pointer to it.
    fn file_receipt(&self, id: &TransformationId, receipt: &Receipt) -> Result<()> {
        match &self.receipt_sink {
            Some(sink) => sink.store(id, receipt).map_err(|detail| Error::Witness {
                action: "file the commit receipt",
                detail: format!(
                    "{} reached the journal and the ledger, and its commit receipt could not be \
                     made durable ({detail}). req/38 §154: a commit whose receipt cannot be filed \
                     is not a commit — so this row is left in `Committing` with its leaf on the \
                     ledger, which is 43 §7-3b's own window and is closed by the next start-up \
                     once the archive will take a file again. Until then this change cannot be \
                     undone (DR-43-1 has no signed postcondition to compare against) and cannot \
                     be proved to a third party. What to fix is in the brackets above, which is \
                     what the operating system said about `.gx/receipts/` — a permission, a full \
                     disk and a path occupied by something that is not a directory are three \
                     different repairs and this sentence does not guess between them",
                    id.0.to_text()
                ),
            }),
            None => Ok(()),
        }
    }

    /// Set `EnforcementMode` (DR-2). `RecordOnly` is what opens T-8r.
    ///
    /// Global rather than per substrate. 43 §4 allows either ("per-substrate or a whole-deployment setting"; sem: SEM-gx-engine-152) and
    /// v0.1 takes the whole-deployment reading, because the per-substrate one needs a place to store
    /// a setting per `SubstrateKind` and nothing in 42 declares one.
    #[must_use]
    pub fn with_mode(mut self, mode: EnforcementMode) -> Self {
        self.mode = mode;
        self
    }

    /// 🔴 **`req/493` §0 / AC-6** — declare what the kernel is holding this process to.
    ///
    /// A builder for the reason `with_posture` is one: the value is a claim that ends up inside a
    /// **signature**, so the thing that knows it has to say it rather than the engine guessing. The
    /// default (see the field) is the true statement for a process nobody confined, which is why
    /// there is no road on which a receipt carries no answer.
    ///
    /// 🔴 What this does **not** verify: that the caller is telling the truth. `gx confine` sets
    /// `GX_CONFINEMENT` after the kernel has answered, so within a `gx confine` the value is
    /// measured; a process that sets the variable by hand can put `kernel_confined: true` on its
    /// own receipts. That is the same trust boundary `docs/LIMITS.md` already states for the rest of
    /// this build — an attacker who can write around gx can write around gx — and it is stated here
    /// rather than left to be discovered because the field's name invites the stronger reading.
    #[must_use]
    pub fn with_confinement(
        mut self,
        confinement: gx_witness::receipt::ConfinementContext,
    ) -> Self {
        self.confinement = confinement;
        self
    }

    /// Set `FailPosture` (DR-2). The default is `FailClosed`, for every substrate.
    ///
    /// `FailOpen` is 43 T-4e's "in effect only where the substrate has explicitly opted in" (sem: SEM-gx-engine-153) and ASM-13's, and this is
    /// that opt-in. It is a builder rather than a default because a fail-open default is the one
    /// misconfiguration that looks like a working system.
    #[must_use]
    pub fn with_posture(mut self, posture: FailPosture) -> Self {
        self.posture = posture;
        self
    }

    /// 🔴 Set 43 T-6's two deadlines, in nanoseconds (ASM-12, 33 NFR-028).
    ///
    /// The defaults are 24 h and 72 h and AC-045 asks for "a test configuration set to a short duration (e.g. 100 ms)" (sem: SEM-gx-engine-154),
    /// which is this. A builder rather than a constant because the two values are a *deployment's*
    /// answer to "how long may a change wait" (sem: SEM-gx-engine-154) and 33 NFR-028 gives them as defaults rather than as
    /// fixed points.
    ///
    /// # No wall clock is involved, in the engine or in the test
    ///
    /// 41 §6 injects the clock, so "TTL elapsed" (sem: SEM-gx-engine-155) is `now - since >= ttl` for the `now` a caller hands
    /// in, and a test reaches AC-045's condition by passing a later timestamp rather than by
    /// sleeping. That is the same property AC-039 rests on and it is worth stating out loud: the
    /// liveness criterion is measured deterministically, and a suite that slept would be measuring
    /// the scheduler.
    #[must_use]
    pub fn with_ttl(mut self, verify_ttl_nanos: i64, escalation_ttl_nanos: i64) -> Self {
        self.verify_ttl = verify_ttl_nanos;
        self.escalation_ttl = escalation_ttl_nanos;
        self
    }

    /// The journal, for a caller that wants to read what was written.
    #[must_use]
    pub fn journal(&self) -> &EngineJournal {
        &self.journal
    }

    /// The witness ledger of 42 §3.11, for a caller that wants the root, a proof or a leaf.
    ///
    /// Read-only on purpose: 43 T-11 is the one place a leaf is appended, and an accessor handing
    /// out `&mut` would be a second road to the exactly-once property INV-S3 asks for.
    #[must_use]
    pub fn ledger(&self) -> &LedgerStore {
        &self.ledger
    }

    /// 🔴 **FR-M04**: how many verdicts of each kind this deployment has issued in total.
    ///
    /// Cumulative since the log began — not since the last checkpoint. A caller that wants a
    /// window subtracts, which is what [`Engine::verdict_checkpoint`] does.
    #[must_use]
    pub fn verdict_tally(&self) -> VerdictTally {
        self.verdicts
    }

    /// 🔴 **FR-M04**: the chain of aggregate verdict checkpoints this deployment has published.
    #[must_use]
    pub fn verdict_checkpoints(&self) -> &[VerdictCheckpoint] {
        self.verdict_log.checkpoints()
    }

    /// 🔴 **FR-M04**: close the current window, sign the counts, and append them (SHOULD).
    ///
    /// # What this buys
    ///
    /// A `VerdictReceipt` is signed and then lives nowhere an outsider can reach — ASM-14 fixes its
    /// `inclusion_proof` to `None`, so an operator who does not export the refusals can show an
    /// auditor a hundred-percent-Admit record and the ledger will not contradict it, because the
    /// ledger only ever held the commits. This publishes the **count**, so that withholding the
    /// receipts stops being free.
    ///
    /// # What it does not buy
    ///
    /// Two things, and they are on `gx_core::VerdictCheckpoint` in full: a gate widened until
    /// nothing is refused publishes `deny = 0` honestly (ruling #3; sem: SEM-gx-engine-156), and one key can sign two
    /// internally consistent chains for two verifiers (ruling #14, v0.2.1's consistency-proof
    /// window). What is closed is **non-disclosure**, and the AC says exactly that much.
    ///
    /// # The window closes even when it is empty
    ///
    /// Two calls with no verdict between them produce a second checkpoint whose window is empty
    /// rather than a repeat of the first, because a verifier folds the chain and a repeat would
    /// double every count in it. An empty window is a true statement about a quiet period.
    ///
    /// # Errors
    /// [`Error::Witness`] if the core cannot be signed, [`Error::Ledger`] if the checkpoint cannot
    /// be appended.
    pub fn verdict_checkpoint(
        &mut self,
        origin: &str,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<VerdictCheckpoint> {
        // 🔴 M-01 (`req/182` §1-2, repaired in `req/189`): the window is `verdicts − published`,
        // and `published` is folded from a chain that survives on disk while `verdicts` is
        // recounted from the journal — so a journal that lost its tail (torn-tail truncation at
        // `open`, a swapped-in shorter journal) reopens with `published > verdicts`. Unchecked
        // `u64` subtraction here would panic in debug and **sign a wrapped count into the chain**
        // in release. Refused by name instead: a checkpoint is a statement about the log it was
        // folded from, and a log the chain has already out-counted is not that log. Saturating
        // subtraction is kept for the arithmetic so the refusal is the only exit.
        let published_ahead = [
            ("deny", self.published.deny, self.verdicts.deny),
            ("admit", self.published.admit, self.verdicts.admit),
            ("escalate", self.published.escalate, self.verdicts.escalate),
            (
                "unverdicted",
                self.published.unverdicted,
                self.verdicts.unverdicted,
            ),
        ]
        .into_iter()
        .find(|(_, published, counted)| published > counted);
        if let Some((kind, published, counted)) = published_ahead {
            return Err(Error::Malformed {
                detail: format!(
                    "the published verdict chain already accounts for {published} `{kind}` \
                     verdicts and this journal holds only {counted}: the journal is shorter than \
                     the chain folded from it (a torn tail truncated at open, or a replaced \
                     journal), and a checkpoint signed over a negative window would be a lie \
                     (M-01, req/189)"
                ),
            });
        }
        let window = VerdictTally {
            deny: self.verdicts.deny.saturating_sub(self.published.deny),
            admit: self.verdicts.admit.saturating_sub(self.published.admit),
            escalate: self
                .verdicts
                .escalate
                .saturating_sub(self.published.escalate),
            unverdicted: self
                .verdicts
                .unverdicted
                .saturating_sub(self.published.unverdicted),
        };
        let unsigned = gx_log::proof::unsigned_verdict_checkpoint(
            self.ledger.log(),
            origin,
            (self.published.total(), self.verdicts.total()),
            window,
            at,
        );
        let signed =
            gx_witness::dsse::sign_verdict_checkpoint(&unsigned, key.signing_key(), key.key_id())
                .map_err(|e| Error::Witness {
                action: "sign the verdict checkpoint",
                detail: e.to_string(),
            })?;
        // Append **before** the boundary moves: a window that was declared closed by a call which
        // then failed to write it is a hole in the chain that this process would go on to deny
        // ever making.
        self.verdict_log
            .append(signed.clone())
            .map_err(|e| Error::Ledger {
                action: "append the verdict checkpoint",
                detail: e.to_string(),
            })?;
        self.published = self.verdicts;
        Ok(signed)
    }

    /// The blob store this engine's delta bodies are in (**M5-05 adopted (a)**; sem: SEM-gx-engine-157).
    ///
    /// Every `delta_cid` in the journal is a name this store resolves, which is what makes a
    /// `Planned` record enough to plan again from -- and what hand 4's T-10b will escrow an inverse
    /// into, through the same door.
    #[must_use]
    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }

    /// The enforcement mode this engine runs under.
    #[must_use]
    pub fn mode(&self) -> EnforcementMode {
        self.mode
    }

    /// The fail posture this engine runs under.
    #[must_use]
    pub fn posture(&self) -> FailPosture {
        self.posture
    }

    /// Whether an `IntentId` has a `DraftCreated` record.
    ///
    /// The draft phase's whole observable surface (**M5-17 adopted (b)**; sem: SEM-gx-engine-158): a draft is a journal record and
    /// a membership question, not a row.
    #[must_use]
    pub fn is_drafted(&self, intent_id: &IntentId) -> bool {
        self.drafted.contains_key(intent_id)
    }

    /// Where a transformation is (43 §1).
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn state(&self, id: &TransformationId) -> Option<Lifecycle> {
        self.table
            .get(id)
            .map(|e| e.state)
            .or_else(|| self.shadow.row(id).and_then(|r| r.state))
    }

    /// The `Fingerprint₀` T-2 recorded (**AC-031**).
    ///
    /// AC-031 verbatim: "`Fingerprint₀` can be re-fetched from the store at the later commit step". This accessor is the
    /// "re-fetch" (sem: SEM-gx-engine-159), and hand 4's `commit` is the "later commit step" that will call it before T-10a's CAS.
    #[must_use]
    pub fn precondition_fingerprint(&self, id: &TransformationId) -> Option<&Fingerprint> {
        self.table.get(id).map(|e| &e.fp0)
    }

    /// The `PlannedDelta` T-2 fixed.
    ///
    /// Held in the row for now. **E-M4-8**'s durable CID-keyed blob store is **M5-05 adopted (a)** (sem: SEM-gx-engine-160) and
    /// hand 3's; what is here is the in-memory Σ, and a restart loses it. `EngineJournal` already
    /// holds the delta's *CID* in every `Planned` record, so hand 3's store is what turns that name
    /// back into a body.
    #[must_use]
    pub fn planned_delta(&self, id: &TransformationId) -> Option<&PlannedDelta> {
        self.table.get(id).map(|e| &e.delta)
    }

    /// The transformation itself.
    #[must_use]
    pub fn transformation(&self, id: &TransformationId) -> Option<&Transformation> {
        self.table.get(id).map(|e| &e.transformation)
    }

    /// The snapshot T-2 read.
    #[must_use]
    pub fn precondition_snapshot(&self, id: &TransformationId) -> Option<&ObjectSnapshot> {
        self.table.get(id).map(|e| &e.pre)
    }

    /// The verdict the gate reached, if one was reached.
    ///
    /// `None` after T-4e, where the gate was never asked. See [`Engine::fail_posture_engaged`].
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn verdict(&self, id: &TransformationId) -> Option<VerdictKind> {
        match self.table.get(id) {
            Some(e) => e.verdict,
            None => self.shadow.row(id).and_then(|r| r.verdict),
        }
    }

    /// Whether this transformation is being enforced (DR-2's `enforced`, default `true`).
    ///
    /// `false` after T-8r (a `Denied` carried through under `RecordOnly`) and after T-4e (a
    /// degraded admission). Both are 43 §4's "equivalent to record-only mode" (sem: SEM-gx-engine-161), and INV-S5 requires the
    /// difference to be visible.
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn enforced(&self, id: &TransformationId) -> Option<bool> {
        self.table
            .get(id)
            .map(|e| e.enforced)
            .or_else(|| self.shadow.row(id).map(|r| r.enforced))
    }

    /// Whether `FailPosture::FailOpen` was exercised for this transformation (43 T-4e).
    ///
    /// The receipt seat for this already exists -- **E-M2-7** put `fail_posture_engaged` in
    /// `ReceiptPayload` in M2, which is why req/78's M5-12 was ruled not adopted, as a misfiling (sem: SEM-gx-engine-162) (req/38 §37).
    /// Issuing the receipt is hand 4's; recording the fact is this hand's.
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn fail_posture_engaged(&self, id: &TransformationId) -> Option<bool> {
        self.table
            .get(id)
            .map(|e| e.fail_posture_engaged)
            .or_else(|| self.shadow.row(id).map(|r| r.fail_posture_engaged))
    }

    /// The canonical CID T-8 fixed.
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn canonical_cid(&self, id: &TransformationId) -> Option<Cid> {
        match self.table.get(id) {
            Some(e) => e.canonical_cid,
            None => self.shadow.row(id).and_then(|r| r.canonical_cid),
        }
    }

    /// The receipt T-11 issued, if this transformation committed (42 §3.10, ASM-14's
    /// `CommitReceipt`).
    #[must_use]
    pub fn receipt(&self, id: &TransformationId) -> Option<&Receipt> {
        self.table.get(id).and_then(|e| e.receipt.as_ref())
    }

    /// 🔴 **DR-43-1, adopted (a)** — the [`UndoWitness`] this process can state **from its own
    /// table**, for a caller that committed in the same process.
    ///
    /// The road for in-process callers and for every engine-level test: the receipt is still seated
    /// (`Entry.receipt`), so the value 42 §3.10 signed is at hand and no archive is needed. It is
    /// deliberately *not* the road `gx undo` or `POST …/undo` take — both of those may be running in
    /// a process that never committed the row (`Engine::open` leaves the table empty, M5H3-5), and a
    /// server that consulted only its own memory would silently answer `Unobservable` for every
    /// transformation it did not commit itself. Those two read the durable receipt instead
    /// (`gx-cli/src/lifecycle.rs::settle_preflight`, `gx-api/src/handlers.rs::undo_witness`).
    ///
    /// A row with no seated receipt, or a receipt whose payload will not decode or carries no
    /// postcondition, is [`Unobservable`] by name — never a silent skip.
    ///
    /// 🔴 **R3 (`req/38` §160 ruling 2)** — two of those three are now [`UndoWitness::Missing`] and
    /// therefore refusals. The one that stays [`Unobservable`] is `NoPostcondition`: a receipt this
    /// process signed itself, which honestly says nothing was observed when the change was applied,
    /// is `req/38` §123 ruling 1's tools-only face and is declared rather than refused. The other
    /// two say the **evidence** is not here, and evidence that is not here does not license a write.
    #[must_use]
    pub fn attested_postcondition(&self, id: &TransformationId) -> UndoWitness {
        let Some(receipt) = self.receipt(id) else {
            return UndoWitness::Missing(WitnessMissing::NoReceipt);
        };
        match receipt.payload() {
            Ok(payload) => match payload.postcondition_fingerprint {
                Some(bytes) => UndoWitness::Attested(bytes),
                None => UndoWitness::Unobservable(Unobservable::NoPostcondition),
            },
            Err(_) => UndoWitness::Missing(WitnessMissing::Unreadable),
        }
    }

    /// What became of 43 T-10c's rollback, where one was in question (**AC-038**).
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn rollback(&self, id: &TransformationId) -> Option<Rollback> {
        match self.table.get(id) {
            Some(e) => e.rollback,
            None => self.shadow.row(id).and_then(|r| r.rollback),
        }
    }

    /// 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — why [`Rollback::NotAttempted`] was
    /// reached, where this process is the one that reached it.
    ///
    /// `None` is an honest answer and not a missing one: the cause is not a component of Σ (see
    /// [`NotAttemptedBecause`]), so a row this process did not abort — one read back off the
    /// journal after a restart — has no cause to give, and the proxy has an arm for exactly that.
    #[must_use]
    pub fn rollback_not_attempted_because(
        &self,
        id: &TransformationId,
    ) -> Option<NotAttemptedBecause> {
        self.not_attempted_because.get(id).copied()
    }

    /// The moment the **engine** says the apply happened (**E-M4-31**, **M5-18 adopted (a)**; sem: SEM-gx-engine-163).
    ///
    /// Not the adapter's: 41 §4's `apply` returns an `AppliedDelta` carrying an `applied_at` the
    /// adapter had no clock to fill, and gx-adapter-fs answers `Timestamp(0)` for exactly that
    /// reason. The engine rebuilds the value with the moment it was given.
    #[must_use]
    pub fn applied_at(&self, id: &TransformationId) -> Option<Timestamp> {
        self.table.get(id).and_then(|e| e.applied_at)
    }

    /// The CID of the inverse T-10b escrowed, if one could be constructed.
    #[must_use]
    pub fn escrowed_inverse(&self, id: &TransformationId) -> Option<Cid> {
        self.table.get(id).and_then(|e| e.inverse_cid)
    }

    /// 🔴 A read-only probe of the world an undo of `id` would run against (`req/38` §98 ruling 2 (sem: SEM-gx-engine-164) —
    /// the settle pre-flight's engine half, `req/160` §2-1).
    ///
    /// The registered adapter's `snapshot(locator)` → `precondition` → the fingerprint's digest,
    /// and **nothing else**: no journal record, no row change, no clock read, no sleep. The poll
    /// loop that calls this repeatedly — with its deadline and its wall clock — lives on the CLI
    /// side (`gx-cli/src/lifecycle.rs`), which is 41 §6's line: "randomness and clock are injected at
    /// the engine boundary" (sem: SEM-gx-engine-165), so an engine whose replay is deterministic may answer "what does the world look like now"
    /// and may not decide "how long to wait for it".
    ///
    /// Two calls with the same `id` may answer differently — that is the point. The caller
    /// compares the answer against the **commit receipt's** `postcondition_fingerprint` (the
    /// signed observation T_o itself made of the world right after its apply, 42 §3.10), which is
    /// the same value space: both are the adapter's content digest of the position
    /// (`precondition` and `apply`'s read-back share one digest function per adapter).
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown transformation or an unregistered adapter,
    /// [`Error::Adapter`] when the world cannot be read. A probe that cannot read the world is
    /// **not** "the world is stale" (sem: SEM-gx-engine-166) — the caller treats it as "polling will not help" and falls
    /// back to the pre-existing behaviour (fire once, let T-10a/T-10c answer).
    pub fn live_digest(&self, id: &TransformationId) -> Result<Cid> {
        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        let substrate = entry.delta.substrate();
        let adapter = self
            .adapters
            .get(substrate)
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{substrate:?}"),
            })?
            .adapter
            .clone();
        let snap = adapter
            .snapshot(entry.pre.locator())
            .map_err(|e| Error::Adapter {
                action: "snapshot",
                detail: e.to_string(),
            })?;
        let fp = adapter.precondition(&snap).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;
        Ok(*fp.digest())
    }

    /// The provenance the engine derived (42 §3.9, **M5-25 adopted (a)**; sem: SEM-gx-engine-167).
    #[must_use]
    pub fn provenance(&self, id: &TransformationId) -> Option<&Provenance> {
        self.table.get(id).and_then(|e| e.provenance.as_ref())
    }

    /// The escalation ticket T-4c raised, with the clock **E-5** injected into it.
    #[must_use]
    pub fn ticket(&self, id: &TransformationId) -> Option<&EscalationTicket> {
        self.table.get(id).and_then(|e| e.ticket.as_ref())
    }

    /// 🔴 **M6H3-2 adopted (a)** (sem: SEM-gx-engine-168) — T-4a's `AdmitProof`, for the surface that has to say **why**.
    ///
    /// > add `admit_proof(&TID)`/`deny_reasons(&TID)` to the engine (a read of the table; no effect on
    /// > Σ). Shares a window with hand 5's problem+json `detail` (sem: SEM-gx-engine-168)
    ///
    /// Two consumers, and they are the two 44 gives the word to: `gx verify`'s stdout
    /// (44 §1.2: `{"kind":"Admit","proof":AdmitProof}`) and the HTTP problem object's `detail`
    /// (44 §2.3: "detailed explanation"; sem: SEM-gx-engine-169). Before this accessor both had [`Engine::verdict`]'s three-valued
    /// discriminant and a digest, which is enough to prove that a proof was hashed and not enough
    /// to tell anyone what it said.
    ///
    /// # 🔴 `None` means three different things, and only one of them is "it was an Admit and the
    /// proof is missing" (sem: SEM-gx-engine-170)
    ///
    /// A row that has not been verified, a row whose verdict was `Deny` or `Escalate`, and a row
    /// **rebuilt from the journal** all answer `None`. The third is the one worth naming: 42 §3.13
    /// records `verdict_digest` and never the proof, so a second process reading Σ has the digest
    /// and not the value — the same limit [`Engine::verdict_receipts`] already carries. A caller
    /// that needs to tell the three apart reads [`Engine::verdict`] first, which Σ does restore.
    #[must_use]
    pub fn admit_proof(&self, id: &TransformationId) -> Option<&AdmitProof> {
        self.table.get(id).and_then(|e| e.admit_proof.as_ref())
    }

    /// 🔴 **M6H3-2 adopted (a)** (sem: SEM-gx-engine-171) — T-4b's reasons, for [`Engine::admit_proof`]'s reason one verdict along.
    ///
    /// 44 §1.2: `{"kind":"Deny","reasons":[Reason]}`. Each [`Reason`] carries a `code` from
    /// gx-gate's declared vocabulary, a bounded `message` and a `ReasonSource`, which is what makes
    /// 44 §2.3's `detail` for a `NOT_ADMITTED` refusal a sentence about the policy that refused
    /// rather than about the request that arrived.
    ///
    /// The same three-way `None` as `admit_proof`, and 42 §3.13 is the same reason.
    #[must_use]
    pub fn deny_reasons(&self, id: &TransformationId) -> Option<&[Reason]> {
        self.table.get(id).and_then(|e| e.deny_reasons.as_deref())
    }

    /// The `VerdictReceipt`s issued for this transformation, in the order they were issued
    /// (**M5H4-6**, ASM-14's first kind).
    ///
    /// One after T-4a/b/c or T-4e; a second after T-5/T-5b, signed by the ruler's key. Empty
    /// before a verdict and after a `plan` that has not been verified.
    #[must_use]
    pub fn verdict_receipts(&self, id: &TransformationId) -> &[Receipt] {
        self.table
            .get(id)
            .map_or(&[], |e| e.verdict_receipts.as_slice())
    }

    /// 43 T-12: which transformation's commit superseded this one (ASM-43-2, **M5-09 adopted (a)**; sem: SEM-gx-engine-172).
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    #[must_use]
    pub fn superseded_by(&self, id: &TransformationId) -> Option<TransformationId> {
        self.supersedes
            .superseded_by(id)
            .or_else(|| self.shadow.row(id).and_then(|r| r.superseded_by))
    }

    /// How many supersede edges have been drawn.
    #[must_use]
    pub fn supersede_count(&self) -> usize {
        self.supersedes.len()
    }

    /// 42 §3.12's status of the inverse escrowed for this transformation, if one was escrowed.
    /// 🔴 **T6 condition ① (Σ-shadow)** — the table first, then the journal's own fold.
    ///
    /// The fall-through is what turns a restarted process from "this transformation does not exist"
    /// into "this transformation is here and I hold no body for it" (`req/182` H-02, `req/38` §148).
    ///
    /// # 🔴 **R8 / `req/234` B-5** — `Available` is now a fact about the blob store, not about a row
    ///
    /// The escrow row is Σ's fold of the journal, and the journal records that an inverse **was**
    /// escrowed under a CID. It cannot record that the body is still there. `req/234` B-5 deleted
    /// every file under `.gx/ledger/journal.blobs/` and measured the result: `gx repair` answered
    /// `rc=0`, `remedy: null`, `head_authenticity: "verified"`, `GET /v1/transformations` reported
    /// `inverse_status: "Available"` for a body that was gone, and only `gx undo` fell over — with
    /// `INTERNAL` and "no blob named gx1:…", which is 44 §2.3's word for "not classifiable" put on
    /// a state that is entirely classifiable.
    ///
    /// So this reads. [`crate::store::BlobStore::contains`] is a `metadata` call on a
    /// content-addressed path, and the answer replaces `Available` with
    /// [`InverseStatus::BodyMissing`] when the store does not hold the CID. Every caller that
    /// gated on `Available` — the HTTP undo handler, the CLI settle pre-flight — now refuses by
    /// name instead of reaching a road that cannot finish.
    ///
    /// Deleting those files is **Model B** (43 §7.9 (b), the row R8 added): a third party who can
    /// write inside `.gx/` is outside what local detectors can answer, and the receipt still proves
    /// what happened even though the undo can no longer be performed. What this repairs is not the
    /// deletion — it is gx *saying* the body is there.
    #[must_use]
    pub fn inverse_status(&self, id: &TransformationId) -> Option<InverseStatus> {
        let (status, cid) = self
            .escrow
            .get(id)
            .map(|row| (row.status, row.inverse_cid))
            .or_else(|| {
                self.shadow
                    .escrow_of(id)
                    .map(|row| (row.status, row.inverse_cid))
            })?;
        match (status, cid) {
            // 🔴 **R9 / `req/236` H-01** — `holds_body`, not `contains`.
            //
            // R8 wrote "`Available` is now a fact about the blob store, not about a row" and reached
            // for `contains`, which is a `metadata` call. `req/236` H-01 built four damaged bodies —
            // zero bytes, half a body, one flipped bit, a deletion — and measured `Available` for
            // three of them, because a `metadata` call is a fact about the **name**. The body is
            // read now: the size, the decode and the digest, which is the same door `Engine::undo`
            // has to walk through anyway. What this costs is stated on `BlobStore::holds_body`.
            (InverseStatus::Available, Some(cid)) if !self.blobs.holds_body(&cid) => {
                Some(InverseStatus::BodyMissing)
            }
            _ => Some(status),
        }
    }

    /// 43 §8's "only an internal annotation, `blocked_by: TransformationId`" (sem: SEM-gx-engine-173), if this transformation is waiting.
    #[must_use]
    pub fn blocked_by(&self, id: &TransformationId) -> Option<TransformationId> {
        self.table.get(id).and_then(|e| e.blocked_by)
    }

    /// When 43 T-6 will expire this transformation, if T-6 applies to the state it is in.
    ///
    /// `None` for every state outside `{Candidate, Verifying, Escalated}` — 43 T-6's from-column is
    /// those three and no others, so a `Canonicalized` transformation waiting for a `commit` call
    /// has no deadline. That is 43's design and not an oversight of this hand: the states T-6
    /// covers are the ones where the engine is waiting on somebody else.
    #[must_use]
    pub fn deadline(&self, id: &TransformationId) -> Option<Timestamp> {
        self.table.get(id).and_then(|e| self.deadline_of(e))
    }

    /// 43 T-6's deadline for one row, or `None` where T-6 does not reach.
    fn deadline_of(&self, entry: &Entry) -> Option<Timestamp> {
        self.deadline_from(entry.state, entry.since)
    }

    /// 🔴 **R3 / `req/222` H-04** — 43 T-6's deadline for a row this process holds **no body**
    /// for, out of the Σ-shadow.
    ///
    /// The same arithmetic as [`Engine::deadline_of`] over the same two inputs; what differs is
    /// where the inputs come from. After a restart the table is empty (M5H3-5) and every live row
    /// is a shadow row, so before R3 this was where T-6 stopped: `Engine::reap` walked
    /// `self.table.keys()`, found nothing, and 43 §9.1.1's INV-L1/L2 became a sentence about one
    /// process's memory. `req/222` H-04 measured it — TTL 1 ms, 200 rows, sweep expired **0**.
    ///
    /// `None` for a row with no state, for the states T-6 does not reach, and for a row whose
    /// entry-time the journal does not carry (there is none: every record has an `at`, and
    /// [`crate::replay::SigmaShadow::since_of`] is a fold over them).
    fn shadow_deadline(&self, id: &TransformationId) -> Option<Timestamp> {
        let state = self.shadow.row(id).and_then(|row| row.state)?;
        let since = self.shadow.since_of(id)?;
        self.deadline_from(state, since)
    }

    /// 43 T-6's arithmetic, over a state and the moment it was entered.
    ///
    /// `None` for every state outside `{Candidate, Verifying, Escalated}` — 43 T-6's from-column is
    /// those three and no others. One function so that the live table and the Σ-shadow cannot
    /// answer differently about what a deadline is (**R3**).
    fn deadline_from(&self, state: Lifecycle, since: Timestamp) -> Option<Timestamp> {
        let ttl = match state {
            Lifecycle::Candidate | Lifecycle::Verifying => self.verify_ttl,
            Lifecycle::Escalated => self.escalation_ttl,
            _ => return None,
        };
        Some(Timestamp(since.0.saturating_add(ttl)))
    }

    /// 🔴 **T6 condition ① (Σ-shadow)** — the only road from a transition to the journal.
    ///
    /// Two statements, and their order is the point: the record reaches the **device** first
    /// (43 §7's write-ahead property, held by [`EngineJournal::append`]) and only then does it reach
    /// the shadow. A shadow updated first would be a process that believed a fact the disk did not
    /// carry, which is the failure mode 43 §7 exists to forbid, one layer up.
    ///
    /// Every one of this file's twenty-three appends goes through here, which is what makes the
    /// shadow's claim structural: there is no arm of 43's transition table that can write a record
    /// and leave the shadow behind, because there is no other way to write a record.
    fn journal_append(&mut self, record: EngineJournalRecord) -> Result<u64> {
        let seq = self.journal.append(record)?;
        if let Some(written) = self.journal.records().last() {
            self.shadow.fold(written);
        }
        Ok(seq)
    }

    /// 🔴 **T6 condition ② (catch-up)** — read to the end of the log, then let the caller write
    /// (`req/38` §148 ruling 1(i)/(ii), designed in `req/190` §2-1).
    ///
    /// Called by whoever holds `.gx/LOCK` ([`crate::store::ProcessLock`]), immediately after taking
    /// it and before the operation. Three things happen, in this order:
    ///
    /// 1. **the journal** is read from this process's own byte offset to the end of the file, and
    ///    every new record is folded into the Σ-shadow;
    /// 2. **the ledger** is re-opened from its path, because `gx-log`'s tree is an in-memory copy
    ///    and a copy taken before another process appended is a copy that will stage the next leaf
    ///    at an index the file already holds (`req/190` F-5 — the measured shape of `req/182` H-01);
    /// 3. **the live table is evicted**, by the one rule below.
    ///
    /// # 🔴 The eviction rule, and there is exactly one of it
    ///
    /// > **If a record read here names a transformation this process holds a row for, that row
    /// > leaves the table.**
    ///
    /// Nothing else. The row is not patched, not re-derived, not partially updated: it is dropped,
    /// and from then on the Σ-shadow answers for it — with a state and no body, which is what this
    /// process honestly has. `req/190` §9-1 names the alternative as the way this design dies: an
    /// engine that applied a foreign record to its own live row would be a second state machine
    /// living beside 43's transition table, and 43's guards would no longer be the only guards.
    /// The cost of the single rule is that a long-lived server loses the bodies of rows another
    /// writer touched and answers `transformation: null` for them until L2 (`req/190` §4-1) rebuilds
    /// bodies from the draft archive. The benefit is the one that matters: a stale body can never be
    /// used to authorise a second effect. `gx serve` holding `Committed`/`Available` for a row a CLI
    /// has just undone would offer a **second door onto an undo** of an inverse already consumed.
    ///
    /// `supersedes` is deliberately not evicted: T-12's edge is monotone (once drawn, never
    /// withdrawn), so a locally-drawn edge cannot disagree with the log, and `superseded_by` falls
    /// through to the shadow when no local edge exists.
    ///
    /// # Errors
    /// [`Error::Malformed`] if the bytes another process appended do not replay whole.
    /// [`Error::Io`]/[`Error::Ledger`] if either file cannot be re-read.
    pub fn catch_up(&mut self) -> Result<CaughtUp> {
        self.read_to_the_end(true)
    }

    /// 🔴 **DR-43-6 / `req/215` H-05** -- the same read, for a `GET` that holds no project lock.
    ///
    /// Read handlers answer under gx-api's `Mutex` alone, because a read that took `.gx/LOCK` would
    /// answer `503` while a CLI verb was writing and `/healthz` would fail for a project that is
    /// perfectly well. The limit that bought was declared -- "between two writes, a `GET` can be one
    /// CLI commit behind" -- and `req/215` H-05 measured it and found the declaration too kind: five
    /// CLI commits later the server still answered with one row and still **signed** a checkpoint
    /// saying `tree_size: 1` over a ledger holding six leaves. Nothing bounded the staleness in
    /// either commits or seconds; only the server's own next write ended it.
    ///
    /// So a read may catch up too. What it may not do is repair: the journal stops in front of a
    /// half-written record instead of refusing it, and the ledger is re-opened **read-only**, so a
    /// torn tail is counted rather than cut ([`crate::store::EngineJournal::catch_up_unlocked`],
    /// `gx_log::LedgerStore::open_read_only`). The next write takes the lock, comes back through
    /// [`Engine::catch_up`], and re-opens the ledger through the writer's door.
    ///
    /// This does not make a `GET` *correct* on its own -- a caught-up server can still be looking at
    /// a journal and a ledger that disagree. `handlers::ledger_checkpoint` therefore asks
    /// [`Engine::ledger_agrees`] before it signs, and refuses rather than signing over a tree its
    /// own journal contradicts (`req/215` H-01's second half).
    ///
    /// # Errors
    /// As [`Engine::catch_up`], minus the two refusals that describe a race rather than damage.
    pub fn catch_up_unlocked(&mut self) -> Result<CaughtUp> {
        self.read_to_the_end(false)
    }

    /// The body of [`Engine::catch_up`] and [`Engine::catch_up_unlocked`].
    ///
    /// 🔴 **DR-43-6: the ledger is the second file, and it moves on its own.** This used to re-open
    /// the ledger inside `if !arrived.is_empty()` -- that is, only when the *journal* had grown. The
    /// unwritten assumption was that gx's two files always move together, and `req/215` H-02/H-03
    /// broke it from two directions at once: a third party repairing the ledger by hand, and gx's
    /// own read verbs truncating it on the way past. A running server never looked again. Measured:
    /// a ledger cut to 0 leaves under a live `gx serve`, which went on answering `201`, `200` and a
    /// **signed receipt** for a commit whose leaf was not on the disk.
    ///
    /// The two files are therefore read as a pair, and the ledger's own change-detector is its
    /// length against `gx_log::LedgerStore::read_offset` -- ruling `req/38` §153 2(a)'s option (b),
    /// chosen over re-opening unconditionally (option (a), O(file) inside the lock on every write)
    /// and over an incremental read API in gx-log (option (c), a lane of its own). What the cheap
    /// detector misses is a rewrite of exactly the same length; `Engine::ledger_agrees`, which every
    /// write now passes through on both sides, is what catches that.
    fn read_to_the_end(&mut self, under_lock: bool) -> Result<CaughtUp> {
        // 🔴 **R4 / `req/225` H-01** — holding the lock is not the same as being a writer.
        //
        // `under_lock` used to answer two different questions at once: "may I refuse what I cannot
        // explain" and "may I repair what I find". They come apart at exactly one caller —
        // `gx repair`'s report, which holds `.gx/LOCK` (so that a diagnosis is not read off a file
        // somebody is halfway through appending to) and is forbidden to write. The first version of
        // this lane's repair opened the engine read-only and then lost the whole ledger on the next
        // line, because this function re-opened it through `LedgerStore::open`.
        let writer_road = under_lock && matches!(self.door, Door::Writer);
        let arrived = if writer_road {
            self.journal.catch_up()?.to_vec()
        } else {
            self.journal.catch_up_unlocked()?.to_vec()
        };
        // 🔴 **R4 / `req/225` H-03 — the same two questions, asked of the other file.**
        //
        // `req/219` §5(h) wrote down that gx's durable state is a **pair** of append-only files,
        // and R3 gave one of the pair a detector for a rewrite that keeps its length. `req/225`
        // H-03 measured the half that was missing: `catch_up` above reads only the bytes past
        // `read_offset` and never looks at the ones it has already folded, so a bit flipped inside
        // a record this process has read is invisible to reading **and** to writing. Live:
        // `/healthz` `200 ledger_agrees:true`, `POST /candidates` `201`, a signed checkpoint at
        // `tree_size: 1`, and the next start-up refusing to open the project.
        //
        // Three questions, and which are asked depends on the road, exactly as on the ledger side:
        //
        // 1. **shorter than what we read** — an append-only log cannot shrink. Under the lock
        //    `EngineJournal::catch_up` has already refused this; without one it returns quietly
        //    (the caller cannot tell damage from a race and must not accuse anybody), and until
        //    now `/healthz` answered `200` and `GET /ledger/checkpoint` **signed** over it
        //    (`req/222` M-01, still real in `req/225` §1-4). It is not silence any more.
        // 2. **the tail record** — one record's worth of I/O, on every road, read included. This
        //    is what catches the same-length rewrite `req/225` H-03 fired.
        // 3. **the whole prefix replays** — `O(file)`, on the writer's road only, for the
        //    middle-of-the-file rewrite the tail check cannot see. Its cost is the window 43 §7.5
        //    already accepted for the ledger, and its absence is unbounded: the measured shape is
        //    a good record appended on top of a broken one until the next start-up loses the lot.
        //
        // 🔴 **R5 / `req/227` H-01 — question 3 is now about identity, and it is asked on both
        // roads.**
        //
        // R4 asked "did the same byte count come back as the same number of whole records", and
        // `req/227` measured three rewrites that satisfy both counts: a record overwritten with the
        // bytes of another record of the same framed length from the same file, two adjacent
        // records swapped, and one bit flipped inside a payload. All three left a live server
        // answering `200`, `201` and a **signed** checkpoint. Since DR-43-9 a chained journal
        // carries a link per record, so the question is "is the head of the chain over the bytes I
        // consumed still the head I have been carrying" — one comparison of 32 bytes, and no
        // rewrite anywhere in the prefix survives it.
        //
        // The cost moved with it, which is what lets the read road ask too: verifying the chain
        // re-reads the prefix and hashes it, and **decodes nothing** — the CBOR decode R4's version
        // paid for on every write is gone. That matters because `/healthz` is the face `req/227`
        // fired through: a detector only the writer's road runs is a detector that lets a server
        // keep answering `ledger_agrees: true` over rewritten bytes until its next write.
        //
        // [`crate::replay::JournalFormat::Legacy`] — a journal written before DR-43-9 — has no
        // links, so it keeps R4's shape comparison on the writer's road and nothing on the read
        // road. That is weaker and is declared rather than smoothed over: 43 §7.6's R5 note says
        // what a legacy journal's records are worth, and `Engine::recover` is where the difference
        // is paid for.
        let on_disk_journal = std::fs::metadata(self.journal.path()).map(|m| m.len()).ok();
        let prefix_intact = match self.journal.format() {
            crate::replay::JournalFormat::Chained | crate::replay::JournalFormat::ChainedV2 => {
                self.journal.prefix_intact()
            }
            crate::replay::JournalFormat::Legacy => !under_lock || self.journal.prefix_intact(),
        };
        // 🔴 **R32 / `req/392` M-02** — the seven terms of R5's `&&` chain, named and handed to the
        // one function that folds them, so that the bool and the sentence printed about it come
        // from the same evaluation. The terms themselves are unchanged; what was a chain of `&&`
        // whose result was a `bool` is now a chain of `&&` whose result carries **which** term
        // stopped it. `Engine::journal_intact` is `is_none()` over this.
        //
        // 🔴 The audit's own report and its probe's header both wrote "five" about this chain
        // before counting it off the source (`req/392` §3-1). Seven is the number, and the
        // struct below has seven fields for it, which is a shape that cannot be miscounted by
        // reading a comment.
        self.journal_departure = JournalTerms {
            // 🔴 **R30 / `req/372` M-02** — a journal from a **newer** `gx` is not intact *for this
            // build*, which is the honest reading of the word: nothing is wrong with the file, and
            // this binary cannot verify it. Folded in here rather than given a gate of its own for
            // R6's reason — the five existing gates already refuse on this line, and a new
            // `gx_code` for "not mine to read" would be a sixth road to the same stop.
            //
            // 🔴 **R32** — asked **first**, and only the order moved. A file from the future
            // replays as `Legacy`, so a project declaring `chained` over one is `downgraded` as
            // well; the bool is the same either way, and the sentence is not.
            not_from_a_newer_gx: !self.journal.from_a_newer_gx(),
            // 🔴 **R6 / `req/229` H-02** — carried from the open, where the declaration in
            // `.gx/VERSION` was compared with the file's framing. A marker removed while this
            // process is **running** is caught by `prefix_intact` below instead: it walks the
            // consumed bytes as a chained file, finds no marker, and answers `false`. This term is
            // for the marker that was already gone when we opened.
            not_downgraded: !self.journal.downgraded(),
            chain_intact: self.journal.chain_intact(),
            not_shorter_than_read: !on_disk_journal
                .is_some_and(|len| len < self.journal.read_offset()),
            tail_unchanged: self.journal.tail_unchanged(),
            prefix_intact,
            // 🔴 **R5 / `req/227` M-01** — bytes that did not come back are bytes that did not come
            // back. On the writer's door DR-43-7 quarantines and removes a torn tail before `open`
            // returns, so the count is history; on the reader's door it is left exactly where it
            // lies, and a report that answered `journal_intact: true` beside its own
            // `torn_tail_bytes: 2315` sent the operator to look at the wrong file (measured).
            no_unrepaired_torn_tail: matches!(self.door, Door::Writer)
                || self.journal.recovery().torn_tail_bytes == 0,
        }
        .departure();
        let mut evicted = Vec::new();
        for record in &arrived {
            self.shadow.fold(record);
            let id = record.transformation();
            if let Some(id) = id {
                if self.table.contains_key(&id) && !evicted.contains(&id) {
                    evicted.push(id);
                }
            }
        }
        for id in &evicted {
            if let Some(entry) = self.table.remove(id) {
                if let Some(peers) = self.by_subject.get_mut(&entry.transformation.subject) {
                    peers.remove(id);
                    if peers.is_empty() {
                        self.by_subject.remove(&entry.transformation.subject);
                    }
                }
            }
            self.escrow.remove(id);
        }
        // 🔴 The ledger's own question, asked whatever the journal did. `read_offset` counts the
        // bytes this store has turned into leaves -- its own appends included -- so a difference is
        // somebody else's writing, somebody else's repair, or a torn tail.
        let on_disk = std::fs::metadata(self.ledger.path()).map(|m| m.len()).ok();
        // 🔴 **R3 / `req/222` H-05** — the length **and** the last record.
        //
        // The length alone was the whole detector, and its own doc named what it missed: "a rewrite
        // that keeps the length exactly". `req/222` H-05 measured that miss end to end — one bit
        // flipped in the tail of a live project's ledger, `/healthz` 200 `ledger_agrees:true`, a
        // commit answered 200 with a signed receipt, a signed checkpoint at `tree_size: 2`, and the
        // next start-up refusing to open the project because none of its 348 bytes would replay.
        // `req/219` §5(h) had said `ledger_agrees` would catch it; it did not, because
        // `ledger_agrees` compares this process's in-memory tree with this process's in-memory
        // frontier and neither had been re-read.
        //
        // So: a tail that no longer reads back is "moved" (`LedgerStore::tail_unchanged`, one
        // record's worth of I/O), and — see below — a writer re-reads whatever the cheap detector
        // says.
        let moved = on_disk.is_some_and(|len| len != self.ledger.read_offset())
            || !self.ledger.tail_unchanged();
        // 🔴 **R3 / `req/222` H-05, second half** — under the lock, re-open unconditionally.
        //
        // This is ruling `req/38` §153 2(a)'s option (a), taken for the road where its cost is
        // bounded and its absence is unbounded: a **write** is about to happen, and writing on top
        // of a tree the disk does not hold is what turned one damaged leaf into every leaf lost.
        // The cost is `O(file)` per write inside the lock, which is the M-08 window `req/222`
        // already names, and it buys the property the cheap detector cannot have at any price: the
        // `ledger_agrees` gate every writer passes through now compares the journal's frontier
        // against a tree that was **read from the disk during this hold**, so a rewrite anywhere in
        // the file — not only in its tail — refuses the write instead of extending it.
        //
        // A store opened read-only by a previous `GET` cannot append, so a writer that finds one
        // takes the writer's door again even when nothing moved. That clause is now subsumed and is
        // subsumed by `under_lock`; the clause is left in so that a reader who narrows the
        // unconditional re-open one day does not lose the reason it was there.
        let reopen = moved || under_lock;
        if reopen {
            // gx-log holds its tree in memory and offers no incremental read, so the whole file is
            // re-read. Under the lock there is no concurrent writer, so the replay that `open`
            // performs is the same replay this performs, and a torn tail here would be a crash's
            // trace rather than a live writer's half-record -- which is why the writer's door is the
            // one that repairs and the reader's door is the one that only counts.
            let path = self.ledger.path().to_path_buf();
            // 🔴 **R6 / `req/229` M-04** — the writer's door is refused to a run that has already
            // decided the journal is not evidence. This is the line the audit's raw came off:
            // `gx repair --yes` on a project with `journal_chain_break_at: 2754` re-opened the
            // ledger here, through the door that quarantines and truncates, and moved 348 of 522
            // bytes — two leaves, one of them undamaged — into `journal.ledger.torn.174-522`. The
            // condition is the journal's, deliberately: DR-43-9 (c-3)'s "a chain break is never
            // cut" is a statement about the **pair**, and the pair's other file is where the cut
            // was happening.
            let journal_suspect = !self.journal.chain_intact() || self.journal.downgraded();
            self.ledger = if writer_road && !journal_suspect {
                LedgerStore::open(&path)
            } else {
                LedgerStore::open_read_only_or_absent(&path)
            }
            .map_err(|e| Error::Ledger {
                action: "re-read the ledger after it moved underneath this process",
                detail: e.to_string(),
            })?;
        }
        // 🔴 **R6 / DR-43-11** — the floor, asked again now that both files have been re-read.
        //
        // A condition evaluated once at `open` is a condition a server that has been up for a week
        // stops asking about, which is exactly the shape `req/215` H-05 measured about staleness
        // and `req/225` H-03 measured about the journal's prefix. The floor itself never falls: it
        // is the highest of what the file said at open and what this process has written since.
        if let Some(floor) = self.head_floor.clone() {
            // 🔴 **R7 / `req/232` M-02** — the declaration's digest is the one the caller handed in
            // at open and is **not** re-read here. The engine is handed a journal path and derives
            // its siblings; `.gx/VERSION` is one directory up and across, and reaching for it would
            // put `gx_cli::layout`'s knowledge inside the engine — the seam `ProjectAnchor` exists
            // to hold. ∴ a declaration rewritten **under a running server** is caught at that
            // server's next start rather than at its next write, and `docs/LIMITS.md` v0.4-t says
            // so rather than leaving a reader to find it.
            let why = rollback_of(
                &floor,
                &self.ledger,
                &self.journal,
                self.version_digest.as_deref(),
            );
            self.declaration_changed =
                matches!(why, Some(gx_log::RolledBack::VersionChanged { .. }));
            self.rolled_back = why.map(|why| why.detail());
        }
        if !arrived.is_empty() || reopen {
            self.committed = self
                .shadow
                .committed()
                .map(|(id, seq)| (*id, *seq))
                .collect();
        }
        Ok(CaughtUp {
            records: arrived.len(),
            evicted,
        })
    }

    /// The Σ-shadow: every row the journal holds, bodies excluded.
    #[must_use]
    pub fn shadow(&self) -> &SigmaShadow {
        &self.shadow
    }

    /// 🔴 **M5H3-4**: whether Σ's ledger component and the ledger's own log say the same thing.
    ///
    /// §40 rules "the turn hand 4 wires up LedgerStore is the same turn that turns 'the frontier
    /// agrees with the real root' into a probe" (sem: SEM-gx-engine-174), and this is the function a probe calls. Σ's `ledger` component is what the
    /// **journal** witnessed — a `(transformation, ledger_seq)` for every `Committed` record — and
    /// the log is the append-only tree gx-log holds. Three things are compared, because they can
    /// disagree in three ways:
    ///
    /// 1. **the count** — a `Committed` record with no leaf is a journal claiming a commit the
    ///    ledger never took, and a leaf with no `Committed` record is 43 §7-3b's crash window (the
    ///    append landed and the record did not);
    /// 2. **each row** — the leaf at `ledger_seq` names that transformation, so a sequence number
    ///    copied from the wrong place is visible;
    /// 3. **the root** — the tree's root at the frontier's size equals its current root, which is
    ///    what makes "the frontier" and "the log" (sem: SEM-gx-engine-175) the same tree rather than two lists of the same
    ///    length.
    ///
    /// Turning that check into a *repair* is 43 §7-3b's recovery and hand 5's. What hand 4 owes is
    /// the observation, running all the time, so that the recovery has something to be a repair of.
    ///
    /// 🔴 **R4 / `req/225` H-03 — and whether the journal is the one this process read.**
    ///
    /// The three comparisons below are all between **this process's** frontier and **this
    /// process's** tree, so all three are silent about a journal that was rewritten underneath
    /// them: the frontier is folded from records already in memory, and the records do not change
    /// when the file does. `req/222` H-05 made exactly that discovery about the ledger and R3
    /// answered it by re-reading the ledger from the disk; `req/225` H-03 made it again about the
    /// journal.
    ///
    /// [`Engine::journal_intact`] is that answer, and it is folded in **here** rather than wired
    /// into each gate, because every road that must not proceed over a damaged pair already asks
    /// this one question: `Session::settle`, `AppState::engine_for_write`, `handlers::healthz`,
    /// `handlers::ledger_checkpoint` and `gx repair`. A gate added tomorrow inherits it. What the
    /// fold costs is precision in the *word*: the condition is named `LEDGER_DISAGREES` on both
    /// faces (`req/38` §156 ruling 2(a)) and minting a second code is a surface addition and
    /// therefore a DR — so the code is unchanged and each refusal's `detail` says which of the two
    /// files moved. See [`Engine::journal_intact`] for the caller's road to that distinction.
    #[must_use]
    pub fn ledger_agrees(&self) -> bool {
        if self.journal_departure.is_some() {
            return false;
        }
        // 🔴 **R6 / DR-43-11 / `req/229` H-01** — folded here for the reason `journal_intact` is
        // folded here: every road that must not proceed over a project that has gone backwards
        // already asks this one question, and a gate added tomorrow inherits it. The two files
        // **agree with each other** in the rolled-back case — that is the whole finding — so no
        // amount of comparing them to one another could have produced this `false`.
        if self.rolled_back.is_some() {
            return false;
        }
        // 🔴 **R7 / `req/232` H-01/M-07** — and for a project whose recorded head is not a document
        // this binary will believe. Folded in the same place for the same reason: a detector that
        // has been replaced, forged or corrupted is **not** an absent one, and the gates that must
        // not run over a project in that state are the gates that already ask this question.
        if self.head_invalid.is_some() {
            return false;
        }
        let frontier = self.committed.len() as u64;
        if frontier != self.ledger.log().len() {
            return false;
        }
        for (transformation, seq) in &self.committed {
            match self.ledger.log().entry(*seq) {
                Some(entry) if entry.transformation == *transformation => {}
                _ => return false,
            }
        }
        self.ledger.log().root_at(frontier) == self.ledger.log().root()
    }

    /// 🔴 **R10 / `req/238` M-06** — how many commits this engine's frontier witnesses, without
    /// building Σ.
    ///
    /// `Engine::sigma()` is a **reconstruction**: it allocates four vectors, copies every row of
    /// the state table, merges the escrow view's two maps and sorts all four. That is the right
    /// shape for a caller comparing two Σ, and it is the wrong shape for a caller who wants one
    /// number. `req/238` M-06 measured the difference on the one endpoint 44 §2.5 keeps **outside**
    /// the bearer guard: `GET /v1/healthz` called `sigma().ledger().len()` on every request and
    /// took 1.39 ms at five commits, 3.67 ms at a hundred and 10.29 ms at four hundred — linear in
    /// the project, on an unauthenticated socket.
    ///
    /// This is the same number: `Sigma`'s ledger component is built from `self.committed` and
    /// nothing else, and `Engine::ledger_agrees` has always read `self.committed.len()` directly
    /// for its own frontier. So the endpoint costs a `len()` and the meaning is unchanged.
    #[must_use]
    pub fn committed_len(&self) -> usize {
        self.committed.len()
    }

    /// 🔴 **R4 / `req/225` H-03** — whether the journal on the disk is still the one this process
    /// read to the end of.
    ///
    /// `false` after [`Engine::catch_up`] or [`Engine::catch_up_unlocked`] found the file shorter
    /// than the bytes already folded, its last framed record rewritten, or (under the lock) its
    /// consumed prefix no longer replaying as the same whole records. It is the journal's half of
    /// what `gx_log::LedgerStore::tail_unchanged` is for the ledger, and it is what makes
    /// [`Engine::ledger_agrees`] a statement about the **pair** rather than about one file.
    ///
    /// Callers read it to say *which* file moved. The refusal itself needs no branch:
    /// `ledger_agrees` is already `false` whenever this is.
    #[must_use]
    pub fn journal_intact(&self) -> bool {
        self.journal_departure.is_none()
    }

    /// 🔴 **R32 / `req/392` M-02** — **which** of the seven terms behind
    /// [`Engine::journal_intact`] is false, or `None` for a journal that has not departed.
    ///
    /// The bool above is derived from this, so a face that prints a sentence and a gate that
    /// refuses are reading one evaluation. `gx_api::journal_note` has one arm per value and
    /// `gx-cli`'s `gx repair` names the file to compare from the same place.
    #[must_use]
    pub fn journal_departure(&self) -> Option<JournalDeparture> {
        self.journal_departure
    }

    /// 🔴 **R6 / DR-43-11 / `req/229` H-01** — why this project is behind its own published head,
    /// or `None` if it is not.
    ///
    /// The sentence every face prints. `Some` implies [`Engine::ledger_agrees`] is `false`; the
    /// converse does not hold, which is exactly why the accessor exists — an operator told only
    /// that the two files disagree would go looking for a disagreement that is not there.
    #[must_use]
    pub fn rolled_back(&self) -> Option<&str> {
        self.rolled_back.as_deref()
    }

    /// 🔴 **R8 / `req/234` M-03 + L-03** — the refusal is about `.gx/VERSION` and about nothing
    /// else.
    ///
    /// `true` only when [`Engine::rolled_back`] is `Some` **and** the reason is
    /// `RolledBack::VersionChanged`. A face that prints a remedy asks this before it says anything
    /// about the ledger or the journal: `req/234` H-02/M-03 measured `gx repair` telling an
    /// operator that their two files were shorter while printing `journal_commits: 2` and
    /// `ledger_leaves: 2` on the same object.
    #[must_use]
    pub fn declaration_changed(&self) -> bool {
        self.declaration_changed
    }

    /// 🔴 **R8 / `req/234` M-03** — do the journal and the ledger agree **with each other**.
    ///
    /// [`Engine::ledger_agrees`] is the *gate*, and R4/R6/R7 folded three further conditions into
    /// it (a moved journal, a rollback, an unbelievable head) precisely so that every road that
    /// must not write inherits them. That fold is right for a gate and wrong for a **report**: a
    /// project refused because its declaration changed has two files that match leaf for leaf, and
    /// printing `ledger_agrees: false` beside `journal_commits == ledger_leaves` is a field saying
    /// something untrue about the pair it names.
    ///
    /// So the two questions now have two names. Nothing branches on this one — it is printed.
    #[must_use]
    pub fn files_agree(&self) -> bool {
        let frontier = self.committed.len() as u64;
        if frontier != self.ledger.log().len() {
            return false;
        }
        for (transformation, seq) in &self.committed {
            match self.ledger.log().entry(*seq) {
                Some(entry) if entry.transformation == *transformation => {}
                _ => return false,
            }
        }
        self.ledger.log().root_at(frontier) == self.ledger.log().root()
    }

    /// 🔴 **R6 / DR-43-11** — the head this project has published, if it has published one.
    ///
    /// `None` means "this project has never recorded a head", which is the honest answer for every
    /// project written before this release and for every engine opened without a
    /// [`ProjectAnchor`]. It is **not** the same as "this project has not moved".
    #[must_use]
    pub fn head_floor(&self) -> Option<&gx_log::HeadFloor> {
        self.head_floor.as_ref()
    }

    /// 🔴 **R7 / `req/232` H-01** — what the door learned about the head **document**.
    ///
    /// [`Engine::head_floor`] answers "is there a statement about the past to compare against";
    /// this answers "is that statement one this binary checked". The audit's finding is the gap
    /// between the two: `head_recorded: true` was true of a file whose signature covered a
    /// different tree entirely.
    #[must_use]
    pub fn head_authenticity(&self) -> HeadAuthenticity {
        self.head_authenticity
    }

    /// 🔴 **R7 / `req/232` H-01/M-07** — why the recorded head is not one to compare against.
    ///
    /// `Some` implies [`Engine::ledger_agrees`] is `false`, exactly as [`Engine::rolled_back`] does,
    /// and the two are different sentences on purpose: a rolled-back project has a head it no longer
    /// lives up to, and this one has a head that is not gx's.
    #[must_use]
    pub fn head_invalid(&self) -> Option<&str> {
        self.head_invalid.as_deref()
    }

    /// 🔴 **R9 / `req/236` M-05** — the key this project's recorded head was signed under.
    ///
    /// A fact about the project, offered so that a verb which is about to blame the *world* can
    /// first check whether it is holding the wrong *key*. `None` for a project with no head or one
    /// whose head document will not read — in which case nothing is claimed either way.
    #[must_use]
    pub fn recorded_head_key_id(&self) -> Option<&str> {
        self.head_key_id.as_deref()
    }

    /// 🔴 **R7** — the rollback this engine was told to proceed over, if it was told to.
    #[must_use]
    pub fn accepted_rollback(&self) -> Option<&gx_log::AcceptedRollback> {
        self.accepted_rollback.as_ref()
    }

    /// 🔴 **R7 / `req/38` §171 ruling 2(c)** — record the operator's acceptance of a rollback.
    ///
    /// The **only** road on which a head is written over a tree shorter than the one this project
    /// already published. It exists because the alternative measured worse: `req/232` M-01 watched
    /// `gx repair --yes` do exactly this silently, so that the shortened tree became the new floor
    /// with nothing anywhere saying it had ever been higher. Here it takes an explicit flag, a
    /// checkpoint from **outside** the project, and it writes down what was given up.
    ///
    /// # Errors
    /// [`Error::Ledger`] if no rollback was accepted at open, or if the head cannot be written.
    pub fn accept_rollback(&mut self, against: &str, at: Timestamp, key: &KeyPair) -> Result<()> {
        let Some(mut accepted) = self.pending_rollback.clone() else {
            return Err(Error::Ledger {
                action: "accept the rollback",
                detail: "this project is not behind the head it published, so there is nothing to \
                         accept. `--accept-rollback` is for a project that has gone backwards and \
                         whose operator holds a checkpoint from outside it (req/232 M-01)"
                    .to_string(),
            });
        };
        accepted.against = against.to_string();
        accepted.at = at.0;
        self.accepted_rollback = Some(accepted);
        // The floor is dropped **after** the acceptance is built, so the head written below states
        // the tree in front of us and carries the tree it replaced.
        self.head_floor = None;
        self.rolled_back = None;
        self.record_head(at, key)
    }

    /// 🔴 **R6 / DR-43-11** — record where this project has reached, and sign the statement.
    ///
    /// Called after a write has made both files durable, from [`Engine::commit`] and from the
    /// recovery's resume road. Three refusals live here rather than at the call sites:
    ///
    /// 1. **no store** — an engine opened without a [`ProjectAnchor`] records nothing, and that is
    ///    the pre-R6 behaviour rather than a silent failure.
    /// 2. **the project is behind its floor** — nothing is signed over a tree we have already
    ///    refused. A rolled-back project must not be given a fresh head that ratifies the rollback.
    /// 3. **the size has already been signed** — 🔴 **equivocation.** `req/229` §1-1 measured a
    ///    rolled-back project committing again and producing a *second* signed root for
    ///    `tree_size: 3` under the same key (`EQUIVOCATION same_size=True same_root=False
    ///    same_keyid=True`). Two signed statements about one size is the failure a transparency log
    ///    exists to make impossible, so a head is written only when the tree has actually grown —
    ///    and when the size is unchanged, the root must be the one already recorded or this
    ///    refuses.
    ///
    /// # Errors
    /// [`Error::Ledger`] if the head cannot be built or written, or if a second root is offered for
    /// a size this project has already signed.
    fn record_head(&mut self, at: Timestamp, key: &KeyPair) -> Result<()> {
        let Some(store) = self.head.clone() else {
            return Ok(());
        };
        if self.rolled_back.is_some() {
            return Ok(());
        }
        // 🔴 **R7 / `req/232` H-01** — a head this binary refused to believe is not a head this
        // binary quietly replaces. Overwriting it would destroy the evidence an operator needs and
        // would turn "somebody replaced the detector" into "the detector is fine now".
        if self.head_invalid.is_some() {
            return Ok(());
        }
        // 🔴 **R7 / `req/232` M-01** — the laundering arm. A project with **no** floor gets no
        // first head out of a run that the recovery has written through: that is precisely how the
        // audit watched `gx repair --yes` re-apply an old delta to a project whose head had been
        // deleted and then mint a head over the shortened tree, so that the rollback became the
        // attested past. A run that accepted a rollback explicitly (`Engine::accept_rollback`) is
        // the one exception, and it writes down what it replaced.
        if self.head_floor.is_none() && self.resumed_rows > 0 && self.accepted_rollback.is_none() {
            return Ok(());
        }
        let tree_size = self.ledger.log().len();
        if tree_size == 0 {
            return Ok(());
        }
        let root = self.ledger.log().root();
        if let Some(floor) = &self.head_floor {
            if tree_size < floor.tree_size {
                return Ok(());
            }
            if tree_size == floor.tree_size {
                if root == Some(floor.root_hash) {
                    return Ok(());
                }
                return Err(Error::Ledger {
                    action: "record the head",
                    detail: format!(
                        "this project has already signed a head over {tree_size} leaf/leaves with \
                         root {}, and a different root is offered at the same size. Signing both \
                         would be an equivocation — two attested histories of one length under one \
                         key (req/229 §1-1, DR-43-11)",
                        floor.root_hash.to_text()
                    ),
                });
            }
        }
        let unsigned = gx_log::proof::unsigned_checkpoint(self.ledger.log(), store.origin(), at)
            .map_err(|e| Error::Ledger {
                action: "build the head",
                detail: e.to_string(),
            })?;
        let checkpoint =
            gx_witness::dsse::sign_checkpoint(&unsigned, key.signing_key(), key.key_id()).map_err(
                |e| Error::Witness {
                    action: "sign the head",
                    detail: e.to_string(),
                },
            )?;
        let journal_len = std::fs::metadata(self.journal.path())
            .map(|m| m.len())
            .unwrap_or(0);
        let journal_head = self
            .journal
            .chain_head_through(journal_len)
            .flatten()
            .map(|head| gx_log::head::to_hex(&head));
        // 🔴 **R7 / DR-43-11 (b)** — the ledger's **end**, beside its shape. `root_hash` says what
        // the tree is; this says which leaf it ends on, which is the number a third party
        // recomputes without walking the tree.
        let ledger_leaf_hash = tree_size
            .checked_sub(1)
            .and_then(|last| {
                usize::try_from(last)
                    .ok()
                    .and_then(|last| self.ledger.log().leaf_hashes().get(last).copied())
            })
            .map(|hash| hash.to_text());
        let mut head = gx_log::PersistedHead {
            head_version: gx_log::HEAD_VERSION,
            journal_len,
            journal_head,
            journal_format: self.journal.format().kind().to_string(),
            checkpoint: checkpoint.clone(),
            version_digest: self.version_digest.clone(),
            ledger_leaf_hash,
            witness_signature: None,
            accepted_rollback: self.accepted_rollback.clone(),
        };
        // 🔴 **R7 / DR-43-11 (b) / `req/232` H-01** — the local numbers, signed.
        //
        // R6 wrote them beside a signature that did not cover them and said so in a doc comment.
        // The audit did not need to read the comment: it edited `tree_size` and left the signature
        // alone, and every door opened. What is signed here is the whole witness — the tree
        // statement repeated, the journal's length and chain head, the declaration's digest and the
        // last leaf — so that a door can tell that the two halves of this file are about **one**
        // moment. It does not make the file unforgeable to somebody holding this project's key
        // (Model B, 43 §7.9); it makes it unforgeable to somebody who is not gx.
        let payload = head.witness_payload().map_err(|e| Error::Ledger {
            action: "build the head's witness",
            detail: e.to_string(),
        })?;
        let mut envelope = gx_witness::dsse::DsseEnvelope {
            payload_type: HEAD_WITNESS_PAYLOAD_TYPE.to_string(),
            payload,
            signatures: Vec::new(),
        };
        envelope.sign(key.signing_key(), key.key_id());
        head.witness_signature = envelope.signatures.into_iter().next();
        store.write(&head).map_err(|e| Error::Ledger {
            action: "record the head",
            detail: e.to_string(),
        })?;
        self.head_floor = Some(head.floor().map_err(|e| Error::Ledger {
            action: "record the head",
            detail: e.to_string(),
        })?);
        // A head this process has just written and signed is one it has checked as far as it can:
        // the next door reads the file again and says so for itself.
        self.head_authenticity = HeadAuthenticity::Verified;
        Ok(())
    }

    /// 🔴 **M6H5-12 adopted (a)** (sem: SEM-gx-engine-176) — the engine's version, from the engine.
    ///
    /// An associated function rather than a method: the answer does not depend on which engine is
    /// asked, and a `&self` would suggest it might. See [`crate::VERSION`] for why the borrowed
    /// constant hand 5 wrote in gx-api had to move here even though the two strings are equal today.
    #[must_use]
    pub fn version() -> &'static str {
        crate::VERSION
    }

    /// Every transformation the table holds, in `TransformationId` order.
    ///
    /// 🔴 **Deliberately *not* widened by DR-43-2** (`req/38` §148). The Σ-shadow knows every row
    /// the journal holds, and this accessor still answers only the rows this process holds a
    /// **body** for — because that is the question its callers ask (`tests/subject_index.rs`
    /// compares it against a full scan of the table; `tests/id_resolution.rs` and
    /// `tests/crash_recovery.rs` read it as "what `open` rebuilt"). The wider answer has a wider
    /// name: `gx-api`'s three list endpoints walk `journal().records()` for their ids and ask
    /// [`Engine::state`] about each, which is the road the shadow made honest — the ids were always
    /// there and the answers were `null` (`req/182` H-02). [`Engine::shadow`] is the direct road.
    #[must_use]
    pub fn transformation_ids(&self) -> Vec<TransformationId> {
        self.table.keys().copied().collect()
    }

    /// 🔴 **M6-07 adopted (b)** (sem: SEM-gx-engine-177) — the rows about `subject`, in `TransformationId` order.
    ///
    /// The subject index read out loud. Equal, always, to filtering [`Engine::transformation_ids`]
    /// by `transformation(&id).subject`, and `crates/gx-engine/tests/subject_index.rs` asserts that
    /// equality rather than describing it — an index is a second answer to a question the table
    /// already answers, and two answers are two things that drift.
    ///
    /// Empty for a subject the table has never seen. **Not** "every row" (sem: SEM-gx-engine-178): a miss that fell back to a
    /// full scan would still be correct and would put back exactly the cost the index removed, and no
    /// correctness probe would ever notice.
    #[must_use]
    pub fn transformations_on(&self, subject: &Subject) -> Vec<TransformationId> {
        self.by_subject
            .get(subject)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// The one door into the state table, so that the subject index cannot be forgotten at three of
    /// four call sites.
    ///
    /// Four callers: T-2's `plan`, `plan`'s rehydrating branch, `undo`'s inverse candidate, and
    /// [`Engine::rehydrate_committed`] (M6H4-4). The last is the one a reader misses, because a
    /// later hand wrote it; making the insert private and the index automatic is the structural
    /// answer to "remember to update both" (sem: SEM-gx-engine-179), which is the shape M6-09's `_`-arm ban takes in a
    /// different corner of this workspace.
    ///
    /// Re-seating an id that is already there moves it between subject buckets rather than leaving a
    /// stale entry behind. A re-plan (43 T-2's idempotency column) replaces the row, and a
    /// re-plan whose `Fingerprint₀` was taken over a different object would otherwise leave this id
    /// in two buckets at once.
    fn seat(&mut self, id: TransformationId, entry: Entry) {
        if let Some(previous) = self.table.get(&id) {
            let previous = previous.transformation.subject;
            if previous != entry.transformation.subject {
                if let Some(bucket) = self.by_subject.get_mut(&previous) {
                    bucket.remove(&id);
                    if bucket.is_empty() {
                        self.by_subject.remove(&previous);
                    }
                }
            }
        }
        self.by_subject
            .entry(entry.transformation.subject)
            .or_default()
            .insert(id);
        self.table.insert(id, entry);
    }

    /// 🔴 **Σ** — this engine's state, from its own tables (**E-M5-2**, AC-039's "original resulting state"; sem: SEM-gx-engine-180).
    ///
    /// > **E-M5-2**: AC-039's "resulting state" = Σ (state table + ledger root + escrow index) (sem: SEM-gx-engine-180)
    ///
    /// # This function must not read the journal, and a probe says so
    ///
    /// AC-039 compares this value with [`crate::replay::reconstruct`]'s. If Σ were built here by
    /// replaying the journal, both sides would come from the same bytes and the criterion would
    /// hold however wrong the reconstruction was. So every field below comes from `self.drafted`
    /// and `self.table` -- the caches the transitions maintain as they run -- and
    /// `tests/store_shape.rs::the_engine_builds_sigma_from_its_tables_and_not_from_its_journal`
    /// reads this body to check it. Rule 1 ("the in-memory table is a cache, and not the state"; sem: SEM-gx-engine-181) is why
    /// the comparison is worth making: a cache that has drifted from the journal is a bug the
    /// journal cannot show on its own.
    ///
    /// # The two components that stopped being empty (hand 4)
    ///
    /// Hand 3 returned empty vectors for `escrow` and `ledger` and said why: no transition it had
    /// implemented wrote them. T-10b and T-11 are this hand's, so both are live now, and the
    /// consequence is what hand 3 asked for -- `tests/ac_039.rs` compares Σ against a journal
    /// **an execution wrote** rather than one a test assembled, which is "the two roads agreeing"
    /// (sem: SEM-gx-engine-182) rather than "the reconstruction itself".
    ///
    /// `superseded_by` is still `None` on every row: T-12 is hand 6's, and the reconstruction side
    /// already reads the record that will carry it.
    /// 🔴 **R9 / `req/236` H-02** — Σ's escrow component, read the way [`Engine::inverse_status`]
    /// reads it: the live table over the journal's own fold.
    ///
    /// Until R9 this was `self.escrow.values()`, a **live map** that only three transitions write
    /// to (`commit`, `settle_pending_escrow`, `rehydrate_committed`) and that `Engine::open` leaves
    /// empty. So `gx repair`'s `escrow_bodies_missing`, which filters this list, was filtering an
    /// empty list in every reading process — **structurally always 0** — while the same binary's
    /// `inverse_status` fell through to the shadow and answered `BodyMissing` correctly.
    /// `req/236` H-02 measured both answers coming out of one process at one moment.
    ///
    /// The repair is not a second detector; it is the two reading doors being made into one. A row
    /// the live table holds wins over the shadow's copy of it — the live one is the later fold of
    /// the same records — and every row the journal witnessed is present either way.
    fn escrow_view(&self) -> Vec<EscrowRow> {
        let mut merged: BTreeMap<TransformationId, EscrowRow> = self
            .shadow
            .escrow_rows()
            .map(|row| (row.transformation, *row))
            .collect();
        for (id, row) in &self.escrow {
            merged.insert(*id, *row);
        }
        merged.into_values().collect()
    }

    #[must_use]
    pub fn sigma(&self) -> Sigma {
        Sigma::new(
            self.drafted
                .iter()
                .map(|(intent_id, rng_seed)| DraftRow {
                    intent_id: *intent_id,
                    rng_seed: *rng_seed,
                })
                .collect(),
            self.table
                .iter()
                .map(|(id, e)| StateRow {
                    transformation: *id,
                    intent_id: Some(e.intent_id),
                    delta_cid: Some(e.delta.reference().cid),
                    fp0: Some(FingerprintRecord::of(&e.fp0)),
                    state: Some(e.state),
                    verdict: e.verdict,
                    verdict_digest: e.verdict_digest,
                    enforced: e.enforced,
                    fail_posture_engaged: e.fail_posture_engaged,
                    canonical_cid: e.canonical_cid,
                    apply_started: e.apply_started,
                    observation_cid: e.observation_cid,
                    rollback: e.rollback,
                    provenance: e.provenance.clone(),
                    // 🔴 T-12's edge, live since hand 6. Hand 4 wrote `None` here and said why:
                    // the reconstruction side already read the record that carries it, and the
                    // live side had no transition that wrote one.
                    superseded_by: e.superseded_by,
                })
                .collect(),
            self.escrow_view(),
            self.committed
                .iter()
                .map(|(transformation, ledger_seq)| CommittedRow {
                    transformation: *transformation,
                    ledger_seq: *ledger_seq,
                })
                .collect(),
        )
    }

    // -----------------------------------------------------------------------
    // T-1
    // -----------------------------------------------------------------------

    /// **T-1** `submit(intent)` — mint the `IntentId` and record a draft.
    ///
    /// 43 T-1's row, field by field: the guard is "the intent conforms to schema" (which the `Intent` type
    /// carries, having been built through gx-core's constructor), the side effect is "the canonical
    /// encode fixes the intent CID, which fixes the `IntentId` (ASM-11); journal: `DraftCreated{intent_id}`" (sem: SEM-gx-engine-183), and the
    /// idempotency rule is "resubmitting an intent under the same canonical encode returns the same
    /// `IntentId` (no side effect, create-if-absent)".
    ///
    /// **The idempotency is not a promise, it is the return path.** A resubmitted intent takes the
    /// early return below and never reaches [`EngineJournal::append`], so "no side effect" (sem: SEM-gx-engine-184) is a fact
    /// about which statements ran. `tests/ac_030.rs` measures it by counting journal records, which
    /// is the only way to tell "it returned the same id" from "it returned the same id and wrote
    /// another record" (sem: SEM-gx-engine-184).
    ///
    /// `rng_seed` is 41 §6's injected randomness. It reaches the journal and nothing else in this
    /// hand -- FR-039's replay is what consumes it, and that is hand 3.
    ///
    /// # Errors
    /// [`Error::Canon`] if the intent has no canonical form. [`Error::Io`] if the journal cannot be
    /// appended to.
    pub fn submit(&mut self, intent: &Intent, rng_seed: u64, at: Timestamp) -> Result<IntentId> {
        let intent_id = IntentId(cid::compute(intent)?);
        if self.drafted.contains_key(&intent_id) {
            return Ok(intent_id);
        }
        self.journal_append(EngineJournalRecord::DraftCreated {
            intent_id,
            rng_seed,
            at,
        })?;
        self.drafted.insert(intent_id, rng_seed);
        Ok(intent_id)
    }

    // -----------------------------------------------------------------------
    // T-2
    // -----------------------------------------------------------------------

    /// **T-2** `plan()` — snapshot, plan, fix `Fingerprint₀` and the `TransformationId`.
    ///
    /// The `Intent` is handed in again rather than looked up, because there is nowhere to look it
    /// up from: **M5-17 adopted (b)** (sem: SEM-gx-engine-185) keeps the draft phase in the journal and the journal records an
    /// `IntentId`, not a body. 44 §1.2's `gx plan <ID>` resolves an id to a session the CLI is
    /// holding; a library API resolves it to the value the caller still has.
    ///
    /// # Where `target` comes from, and 🔴 why it is `None`
    ///
    /// 43 T-2 says the `TransformationId` is "the CID of the canonical form, including delta/target"
    /// (sem: SEM-gx-engine-186), so `target` -- "the expected post-state digest, fixed by `plan()`" (41 §3) -- has to be known here. **It is
    /// not knowable.** `SubstrateAdapter` has seven methods and none of them returns a predicted
    /// post-state: `plan` returns a `PlannedDelta` of `{substrate, payload, reference}`, and the only
    /// type carrying a `resulting_digest` is `AppliedDelta`, which exists *after* `apply`. So v0.1
    /// fixes `target = None` and the canonical form includes the absence.
    ///
    /// This is worth stating plainly because of what it does to **M5-11 / blocker item 5**. That ticket asks
    /// how the engine should refuse when "plan's prediction and apply's measurement" (sem: SEM-gx-engine-187) disagree, and req/38 §37 sent
    /// it to the Owner desk with an instruction for this milestone: "until the ruling, do not write the
    /// engine-side refusal check; put one line in the doc naming the absence of the check (do not hide it)". Here is that line, and one more:
    /// **the check is absent because the prediction is absent** -- there is no `target` for an
    /// `AppliedDelta.resulting_digest` to disagree with, so the comparison blocker item 5 (sem: SEM-gx-engine-187) is about cannot be
    /// written against today's trait at all. Raised as **M5H2-2**.
    ///
    /// ## 🔴 Superseded in part — **M5H2-2, adopted (b)** (`req/919` A1)
    ///
    /// The paragraphs above are kept because a reader of the repair should see the claim it
    /// replaced, and one clause of that claim was always narrower than it read. "Not knowable"
    /// was true of the **trait**, not of the world: `plan` is handed `{intent, pre}`, and an
    /// adapter that can compute a post-state digest from those two has had everything it needs
    /// all along. What was missing was somewhere to put the answer.
    ///
    /// So `PlannedDelta` grew an opt-in `promised_target` seat (`gx-substrate`'s `delta.rs`
    /// carries the ruling and its three refused alternatives), and this function fills 41 §3's
    /// `target` from it. Two things are deliberately unchanged: `SubstrateAdapter` still has
    /// **seven** methods (N-07/N-08/N-09, and §34 M4H6-4's refusal reserved by name for exactly
    /// this request), and an adapter that promises nothing still produces `target = None` and
    /// bit-for-bit the same `TransformationId` it produced before.
    ///
    /// **Blocker item 5's comparison is now written**, in [`Engine::commit`], and it is reachable
    /// only from the far side of a prediction: no promise, no check, and the road every shipped
    /// adapter takes has not moved. The seventh `AbortReason` is `PostconditionMismatch`.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the intent has no draft or its substrate has no registered adapter.
    /// [`Error::Adapter`] if `snapshot`, `plan` or `precondition` refuses. [`Error::Canon`] if the
    /// transformation has no canonical form. [`Error::Io`] from the journal.
    /// 🔴 **R3 / `req/222` H-03** — the `TransformationId` a [`Engine::plan`] of this intent would
    /// mint **right now**, computed without writing anything.
    ///
    /// # What it is for
    ///
    /// A surface that holds a `TransformationId` and wants to put a body back on the row (gx-api's
    /// `with_a_body`) has to ask "does re-planning this intent still name the row the caller sent?"
    /// — 43 §8 forces a re-`plan` once `Fingerprint₀` has gone stale, and a re-plan that lands on a
    /// **different** id is a different transformation. `req/222` H-03 measured what asking that
    /// question with `plan` itself costs: the answer arrives one journal record too late. The
    /// handler refused with `409` and the sentence "Nothing was written to either row", and the
    /// journal had grown by a `Planned` for an id nobody had asked for — a row that then answered
    /// `GET` 200, `verify` 200 and `commit` 200, with no CAS anywhere on that road because
    /// DR-43-1's is on `undo` alone. So the question is asked here, where it costs nothing.
    ///
    /// # Why this is not a second definition of identity
    ///
    /// It is the same code. [`Engine::plan`] computes the identity through [`Engine::plan_shape`]
    /// and so does this; there is one `Transformation::new` and one `cid::compute` on this road,
    /// and a change to either moves both answers together. What this function does *not* have is
    /// `plan`'s two guards (the draft must be known, a re-plan is refused once the row has left
    /// `Candidate`) — those are about **writing**, and this writes nothing.
    ///
    /// # What it costs
    ///
    /// The adapter's read-only three (`snapshot`, `plan`, `precondition`, 41 §4) run twice when the
    /// caller goes on to plan: once here and once inside `plan`. That is the price of learning the
    /// answer before the record instead of after it, and it is paid on the rebuild road only — the
    /// common case (`engine.transformation(id).is_some()`) never reaches either call.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the intent's substrate has no registered adapter, [`Error::Adapter`]
    /// if `snapshot`/`plan`/`precondition` refuses, [`Error::Canon`]/[`Error::Core`] if the value
    /// has no canonical form.
    pub fn planned_id(&self, intent: &Intent, at: Timestamp) -> Result<TransformationId> {
        Ok(self.plan_shape(intent, at)?.id)
    }

    /// The read-only half of [`Engine::plan`]: everything 43 T-2 derives before anything is written.
    ///
    /// 41 §4's read-only three and one CID. Held as one function so that [`Engine::plan`] and
    /// [`Engine::planned_id`] cannot answer differently about what a plan of this intent is
    /// (**R3**, `req/222` H-03).
    fn plan_shape(&self, intent: &Intent, at: Timestamp) -> Result<PlanShape> {
        let intent_id = IntentId(cid::compute(intent)?);
        let adapter = self
            .adapters
            .get(intent.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", intent.substrate()),
            })?
            .adapter
            .clone();

        let pre = adapter
            .snapshot(intent.locator())
            .map_err(|e| Error::Adapter {
                action: "snapshot",
                detail: e.to_string(),
            })?;
        let delta = adapter.plan(intent, &pre).map_err(|e| Error::Adapter {
            action: "plan",
            detail: e.to_string(),
        })?;
        let fp0 = adapter.precondition(&pre).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;

        // `id` is outside the IdentityView (42 §1.3, ASM-4), so the CID computed over the value
        // carrying a placeholder is bit-for-bit the CID computed over the finished one. This is
        // gx-core's own `PROVISIONAL_ID` convention; the constant is private there, so the zero
        // value is written here with the same reasoning rather than reached for.
        let mut transformation = Transformation::new(
            TransformationId(Cid([0u8; 32])),
            0,
            Subject::Object(*pre.id()),
            // 🔴 **M5H2-2, adopted (b)** (`req/919` A1) — 41 §3's `target`, filled from the plan
            // that was just made instead of fixed at `None`. An adapter that works out a
            // post-state digest says so on the `PlannedDelta` it returns; every adapter that does
            // not answers `None` here, which is bit-for-bit the value this line held before, so
            // the `TransformationId` of every transformation this workspace can make today is
            // unchanged. See `Engine::plan`'s doc for the ruling and what it did to blocker
            // item 5.
            delta.promised_target(),
            Vec::new(),
            CompositionMetadata {
                intent_id,
                delta: delta.reference().clone(),
                context: intent.context().clone(),
                actor: intent.actor().clone(),
                created_at: at,
            },
        )?;
        let id = TransformationId(cid::compute(&transformation)?);
        transformation.id = id;
        Ok(PlanShape {
            id,
            transformation,
            delta,
            fp0,
            pre,
        })
    }

    pub fn plan(&mut self, intent: &Intent, at: Timestamp) -> Result<TransformationId> {
        let intent_id = IntentId(cid::compute(intent)?);
        if !self.drafted.contains_key(&intent_id) {
            return Err(Error::NotFound {
                what: "draft",
                id: format!("{intent_id:?}"),
            });
        }
        // 43 T-2's idempotency column: "re-running against the same snapshot yields the same
        // `PlannedDelta` and the same `TransformationId` (safe to retry)" (sem: SEM-gx-engine-188). Safe to retry is not safe to *rewind*: a candidate
        // that has since been verified, denied or canonicalised would have its row replaced by a
        // fresh `Candidate` and its verdict forgotten. So a re-plan is allowed only while the row is
        // still where T-2 left it, and refused otherwise.
        if let Some(existing) = self
            .table
            .values()
            .find(|e| e.intent_id == intent_id && e.state != Lifecycle::Candidate)
        {
            return Err(Error::InvalidState {
                id: format!("{:?}", existing.transformation.id),
                state: existing.state.name(),
                attempted: "plan",
            });
        }

        // 🔴 **R3** — 41 §4's read-only three and the identity they fix, in the one function
        // [`Engine::planned_id`] also calls, so that "what would a plan of this name" and "what a
        // plan does name" cannot drift (`req/222` H-03).
        let PlanShape {
            id,
            transformation,
            delta,
            fp0,
            pre,
        } = self.plan_shape(intent, at)?;

        // 🔴 **M6 hand 3 — the same intent, planned again in another process.**
        //
        // Nothing above this line has written anything: `snapshot`, `plan` and `precondition` are
        // 41 §4's read-only three, and the identity is a function of what they returned. So this is
        // the first point at which the two cross-process questions can be asked, and the last point
        // at which they can be answered without a side effect.
        //
        // 44 §1.2 runs `gx submit`, `gx plan`, `gx verify` and `gx commit` as **four processes**,
        // and `Engine::open` rebuilds the draft phase and the resolution index from the journal
        // while leaving the in-flight table empty (M5H3-5). A row's *body* — the `Transformation`,
        // the `ObjectSnapshot`, the `PlannedDelta` — is not in the journal (ASM-9), so the only way
        // a later process can hold one is to plan it again; and 43 T-2's idempotency column is what
        // makes that legal: "re-running against the same snapshot yields the same `PlannedDelta` and the
        // same `TransformationId` (safe to retry)" (sem: SEM-gx-engine-189).
        //
        // Two things follow, and both are refinements of guards this function already had for the
        // single-process case.
        // 🔴 **DEFECT-891-1** (`req/895` §2) — `the latest`, which is what this guard always meant
        // and what a `BTreeMap` value happened to be while the relation was still a function.
        let recorded = self
            .resolved
            .get(&intent_id)
            .and_then(|ids| ids.last())
            .copied();
        let rehydrating = recorded == Some(id) && !self.table.contains_key(&id);
        if let Some(recorded) = recorded {
            if !rehydrating && !self.table.contains_key(&recorded) {
                // The guard above this one — "a re-plan is allowed only while the row is still
                // where T-2 left it" (sem: SEM-gx-engine-190) — reads the **table**, and the table is empty after a restart.
                // Read from the journal instead and the same rule holds across processes: an intent
                // whose transformation has already been verified, denied or committed may not be
                // re-planned into a second one, because doing so would leave the first row's
                // verdict behind and answer a later `gx verify <TID>` about a transformation the
                // operator did not name. The refusal is 43 §8's "force a re-`plan()`" (sem: SEM-gx-engine-191) seen from
                // the other side, and it costs no journal record.
                let sigma = reconstruct(self.journal.records());
                if let Some(state) = sigma.state_of(&recorded).and_then(|row| row.state) {
                    if !matches!(state, Lifecycle::Candidate) {
                        return Err(Error::InvalidState {
                            id: format!("{recorded:?}"),
                            state: state.name(),
                            attempted: "plan",
                        });
                    }
                }
            }
        }
        if rehydrating {
            // 🔴 The body comes from the re-plan and the **state comes from the journal**.
            //
            // This is the split M5H3-5 left open, answered where the answer exists rather than in
            // `Engine::open`: `open` cannot rebuild a row because it has no adapter to ask, and by
            // the time this line runs the adapter has answered. What the journal does hold is every
            // transition, which is req/78 Λ1's whole claim about Σ — so a row rebuilt here carries
            // the state, the verdict, the flags and the canonical CID the log recorded, and no
            // second `Planned` record is appended for something that already happened.
            //
            // req/88 §3 Λ2 is the reason for this shape rather than a re-drive: "N runs of the CLI"
            // and "N calls to one long-lived engine" (sem: SEM-gx-engine-192) are observationally equal on Σ **only
            // if** the second process writes nothing the first would not have written twice. A
            // resume that re-verified would put a second `VerifyStarted` and a second `Verdict` in
            // the log and issue a second verdict receipt — one long-lived engine's journal and a
            // single-shot CLI's journal disagreeing about how many times the gate was asked.
            //
            // What it does **not** restore is what the journal does not hold: the verdict receipts
            // and the in-flight annotations `blocked_by` and `since`.
            //
            // 🔴 **The escalation ticket is no longer on that list** (M6H3-10, settled by
            // measurement in M6 hand 4). The journal records the verdict and not the ticket, and
            // 42 §3.13's vocabulary does **not** grow, because the ticket did not have to be
            // recorded to be recovered: `gx_gate::escalation_ticket` is the one road E-M3-4 takes
            // and it reads nothing but the `TransformationId`, so a row whose journalled verdict is
            // `Escalate` can rebuild the ticket it raised. See [`Engine::rebuilt_ticket`].
            self.blobs.put(&delta)?;
            let sigma = reconstruct(self.journal.records());
            let row = sigma.state_of(&id);
            // 🔴 H-04 (`req/182`, `req/189`): the ticket is a **queue entry**, and a queue entry
            // exists only while somebody is still waiting on it — the row's `state` is `Escalated`.
            // The verdict alone is not enough: after T-5 the journalled verdict is still `Escalate`
            // (the gate's word is not rewritten by a ruling) but the row has moved on, and a
            // ticket rebuilt from the verdict alone would put a ruled row back in `GET
            // /escalations` after every restart. Same rule as `set_state` below: ticket ⇔ Escalated.
            let ticket = match (row.and_then(|r| r.verdict), row.and_then(|r| r.state)) {
                (Some(VerdictKind::Escalate), Some(Lifecycle::Escalated)) => {
                    self.rebuilt_ticket(&id)?
                }
                _ => None,
            };
            self.seat(
                id,
                Entry {
                    intent_id,
                    transformation,
                    state: row.and_then(|r| r.state).unwrap_or(Lifecycle::Candidate),
                    // 🔴 **R3 / `req/222` H-04** — the deadline comes back with the state.
                    //
                    // This read `since: at`, which set 43 T-6's clock to *now* every time a row was
                    // rebuilt: an hour-old `Candidate` came back looking one millisecond old, and
                    // `req/222` measured the consequence — a row whose TTL had passed answering
                    // `verify` 200 and `commit` 200 after a restart. The state on this line is the
                    // journal's, and so is the moment it was entered; taking one from the log and
                    // the other from the clock was the mismatch.
                    //
                    // The fallback is `at` and it is unreachable in practice — the branch is only
                    // entered when `recorded == Some(id)`, which means a `Planned` record named
                    // this row and every record carries an `at`. Written rather than `expect`ed
                    // because 41 §6 counts a panic as a bug.
                    since: self.shadow.since_of(&id).unwrap_or(at),
                    blocked_by: None,
                    delta,
                    fp0,
                    pre,
                    verdict: row.and_then(|r| r.verdict),
                    verdict_digest: row.and_then(|r| r.verdict_digest),
                    enforced: row.is_none_or(|r| r.enforced),
                    fail_posture_engaged: row.is_some_and(|r| r.fail_posture_engaged),
                    canonical_cid: row.and_then(|r| r.canonical_cid),
                    ticket,
                    // Not in Σ, so not restored: the journal records the verdict's digest and
                    // never the proof (42 §3.13). A rebuilt row therefore answers `None` here, the
                    // same way it answers with an empty `verdict_receipts` below, and M6H3-2's
                    // "no effect on Σ" (sem: SEM-gx-engine-193) is what that costs.
                    admit_proof: None,
                    deny_reasons: None,
                    verdict_receipts: Vec::new(),
                    superseded_by: row.and_then(|r| r.superseded_by),
                    apply_started: row.and_then(|r| r.apply_started),
                    observation_cid: row.and_then(|r| r.observation_cid),
                    rollback: row.and_then(|r| r.rollback),
                    provenance: row.and_then(|r| r.provenance.clone()),
                    inverse_cid: None,
                    applied_at: None,
                    receipt: None,
                },
            );
            return Ok(id);
        }

        // 🔴 **DR-46-33 / DR-46-28** — the input-generation join, computed here at T-2 where the
        // actor is fixed, and journalled as its result so the rebuild roads reproduce the boundary.
        let input_generation = self.joined_input_generation(intent.substrate(), intent.actor());
        self.journal_append(EngineJournalRecord::Planned {
            transformation: id,
            intent_id,
            // **E-M5-13**, the locator half (M5H5-2): read off the intent the caller still has, so
            // that 43 §7-3c can name what it was planning against without the body ASM-9 discards.
            locator: intent.locator().to_string(),
            delta_cid: delta.reference().cid,
            fp0: FingerprintRecord::of(&fp0),
            // **E-M5-13**, the parents half (M5H6-6). Empty here: a `plan` of order 0 has no
            // predecessor, and the one producer of a non-empty list is `undo`.
            parents: transformation.parents.clone(),
            input_generation,
            // 🔴 **DR-46-45 (`req/973` §B-1)** — `None` here and `Some` on the undo road, which is
            // what makes this field the discriminator the receipt-construction sites use. A `plan`
            // compared nothing against a prior attestation because there is no prior to attest.
            undo_witness: None,
            at,
        })?;
        self.remember_resolution(intent_id, id);

        // 🔴 **E-M4-8 / M5-05 adopted (a)** (sem: SEM-gx-engine-194), and journal-first in the order of two statements: the name
        // goes into the journal, then the body goes into the store. 42 §5 makes keeping the body
        // mandatory ("store it (mandatory)"; sem: SEM-gx-engine-194) for the escrowed inverse, and E-M4-8 extends it to the
        // planned delta so that replay and undo are constructible at all). A re-plan of the same
        // intent lands on the same CID and is answered `AlreadyPresent` without a second write,
        // which is **M4H6-3** on the live path rather than only in a probe.
        self.blobs.put(&delta)?;

        self.seat(
            id,
            Entry {
                intent_id,
                transformation,
                state: Lifecycle::Candidate,
                // 43 T-6 starts counting here. A re-plan (43 T-2's idempotency column) replaces
                // the row, which restarts the clock -- correct, because `Fingerprint₀` was taken
                // again and the transformation is waiting on a fresh precondition.
                since: at,
                blocked_by: None,
                delta,
                fp0,
                pre,
                verdict: None,
                verdict_digest: None,
                enforced: true,
                fail_posture_engaged: false,
                canonical_cid: None,
                ticket: None,
                admit_proof: None,
                deny_reasons: None,
                verdict_receipts: Vec::new(),
                superseded_by: None,
                apply_started: None,
                observation_cid: None,
                rollback: None,
                provenance: None,
                inverse_cid: None,
                applied_at: None,
                receipt: None,
            },
        );
        Ok(id)
    }

    // -----------------------------------------------------------------------
    // T-3, T-4a..T-4e
    // -----------------------------------------------------------------------

    /// **T-3 → T-4a/T-4b/T-4c/T-4d/T-4e** — collect evidence, ask the gate, record what came back.
    ///
    /// One entry point for six transitions, because 43 gives them one trigger and one from-state:
    /// everything below `Verifying` in the table is a branch on what the two collaborators answered,
    /// and splitting them into six functions would put the branch in the caller.
    ///
    /// | what answered | verdict | to | transition |
    /// |---|---|---|---|
    /// | collector `Ok`, gate `Admit` | `Admit` | `Admitted` | T-4a |
    /// | collector `Ok`, gate `Deny` | `Deny` | `Denied` | T-4b |
    /// | collector `Ok`, gate `Escalate` | `Escalate` | `Escalated` | T-4c |
    /// | collector `Err`, `FailClosed` | — | `Aborted(VerifierUnavailable)` | T-4d |
    /// | collector `Err`, `FailOpen` | `Admit`, degraded | `Admitted`, `enforced=false` | T-4e |
    /// | collector `Ok`, gate `Err` (⊥) | — | `Aborted(InternalError)` | **see below** |
    ///
    /// # 🔴 The gate's ⊥ is not the collector's `Err` (**M5-23 adopted (a)**; sem: SEM-gx-engine-195 = **E-M5-5**)
    ///
    /// **E-M3-3** made `Gate::verify` fallible and said what the failure is not: "unevaluable (⊥) is
    /// neither Deny nor Escalate" (sem: SEM-gx-engine-195). **M5-23 adopted (a)** settles what it *is* on this side:
    ///
    /// > "`RecordOnly` bears only on `Deny`, not on ⊥ (Err) -- ⊥ means 'no verdict exists', so it is always" (sem: SEM-gx-engine-195)
    /// > `Aborted`(fail-closed)
    ///
    /// So ⊥ aborts whatever the enforcement mode says, and the code below reads the mode nowhere on
    /// that arm. What 43 does not say is **which** `AbortReason` it carries, and there are only two
    /// candidates. It is **not** `VerifierUnavailable`: **M5-03 adopted (a)** (sem: SEM-gx-engine-196) makes the collector's `Err`
    /// that reason's only producer, and a second producer would delete the property AC-036 is
    /// measured by. It is `InternalError`, which is 43 T-13's "an unexpected internal engine failure
    /// ... a bug-class failure" -- and the same reading **M5-24 adopted (a)** (sem: SEM-gx-engine-196) already applied to `cas_eq`'s `Err`, where a lower
    /// layer's "this is a wiring bug" must not be dressed as an ordinary business condition. A gate
    /// that cannot evaluate is a deployment with an unreadable policy set or a registry that refused
    /// to build; both are "broken" (sem: SEM-gx-engine-196) rather than "refused", which is the distinction E-M3-3
    /// exists to keep. Raised as **M5H2-5**, because deriving a reason from two rulings is still a
    /// reading.
    ///
    /// # T-4e writes a `Verdict` record for a verdict that does not exist
    ///
    /// 43 T-4e's journal cell is `Verdict{id, Admit, fail_posture_engaged=true}`, and no gate ran.
    /// The record's `verdict_digest` is therefore `None` -- see [`EngineJournalRecord::Verdict`] for
    /// why an empty `AdmitProof` was not minted to fill it.
    ///
    /// # `invert_available` costs a call 43 does not schedule
    ///
    /// 41 §4 gives `GateInput` a field for it and **E-M3-4** makes `false` the one condition that
    /// produces an `Escalate` in v0.1, so the gate cannot be asked without it. 43 schedules
    /// `adapter.invert` at T-10b (escrow, before `apply`), which is hand 4's and far too late. So
    /// this hand calls it here as well: it is a read, it takes `(delta, pre)` and no clock, and 41
    /// §4 asks the adapter for "the inverse delta (an undo guarantee, DR-1(a)); `None` if it cannot be constructed" (sem: SEM-gx-engine-197) rather than for a
    /// side effect. Two calls to one pure function is the cost of 41 §4 and 43 §3 scheduling the
    /// same question differently; recorded as **M5H2-6** rather than absorbed silently.
    ///
    /// # 🔴 Three things hand 6 adds, and none of them is a new state
    ///
    /// * **T-6, lazily** (M5-10 adopted (a); sem: SEM-gx-engine-198). The deadline is evaluated before the guard, so a
    ///   `Candidate` that has sat past `verify_ttl` becomes `Aborted(Expired)` and this call then
    ///   refuses it as an invalid state. ~~INV-L1 without a resident process.~~ L-02
    ///   (`req/182` §1-3 / H-03, `req/189`): what this buys is INV-L1 **for a row somebody
    ///   touches** — `verify` / `escalation` / `cancel` evaluate the deadline; nothing walks the
    ///   table for rows nobody asks about, and `Engine::reap` (the resident half) has no
    ///   production caller (`req/182` H-03, DR-43-2). "INV-L1 for every row" is a claim this
    ///   sentence must not make until a reaper runs; the struck words are kept so the
    ///   overstatement stays visible where it was made.
    /// * **43 §8's waiting** (AC-045's second clause). Before `VerifyStarted` is written, the
    ///   engine asks `adapter.commutation` about every in-flight transformation on the same
    ///   `Subject` that has already started verifying, and a `Conflicts` holds this one at
    ///   `Candidate` with `blocked_by` set — "no new state is added; only an internal annotation,
    ///   `blocked_by: TransformationId`" (sem: SEM-gx-engine-199). **The TTL keeps running while it waits**, which is
    ///   INV-L4.
    /// * **ASM-14's verdict receipt** (M5H4-6). Every road out of this function that reaches a
    ///   verdict issues one, T-4e included — 43 T-4e requires "always carve `enforced=false` and
    ///   `fail_posture_engaged=true` into the receipt" (sem: SEM-gx-engine-200) and the only receipt that exists at that
    ///   moment is a verdict-stage one. That receipt is what **E-M5-11** made writable.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the id is not in the table, [`Error::InvalidState`] if it is not a
    /// `Candidate` (which includes a row 43 §8 is holding behind a conflicting commit),
    /// [`Error::NotFound`] if its adapter has been unregistered, [`Error::Adapter`] if `invert` or
    /// `commutation` refuses, [`Error::Io`] from the journal, [`Error::Witness`] if the verdict
    /// receipt cannot be issued, and [`Error::Canon`] if a verdict has no canonical form. A
    /// collector that refuses and a gate that cannot evaluate are **not** errors here: they are
    /// transitions, and they come back in the `Ok`.
    pub fn verify(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
        mode: Option<EnforcementMode>,
    ) -> Result<Lifecycle> {
        // 🔴 **M6-08 adopted (a)** (req/38 §47; sem: SEM-gx-engine-201) — the per-call override 44 §1.2 asks for in as many words:
        //
        // > "`--record-only`: force DR-2's record-only mode **per this command** (overriding the global setting)" (sem: SEM-gx-engine-201)
        //
        // [`Engine::with_mode`] is a builder that consumes `self` at `open`, which a single-shot CLI
        // can use and a long-lived `gx serve` cannot: 44 §2.2's `POST /candidates/{id}/verify` body
        // carries `record_only: bool|null` **per request**, and a server that answered it by
        // reassigning a field on shared state would leak one request's posture into another's — the
        // fail-open M6-08(b) was written down as the form "must not be adopted" (sem: SEM-gx-engine-202). So the override is an
        // argument, `None` means "use the engine's setting", and no state moves.
        let mode = mode.unwrap_or(self.mode);
        // 43 T-6, before anything else: a deadline that has passed is a transition that already
        // happened, and answering a request about a row without evaluating it would make liveness
        // depend on who called.
        self.expire_if_due(id, at)?;

        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        if entry.state != Lifecycle::Candidate {
            return Err(Error::InvalidState {
                id: format!("{id:?}"),
                state: entry.state.name(),
                attempted: "verify",
            });
        }

        // 43 §8. A row that was waiting is re-evaluated first: "the moment `T1` reaches a terminal
        // state ..., `T2` is re-evaluated: if `T1` is `Committed`, `T2`'s `Fingerprint₀` is stale, so
        // a re-`plan()` (re-fingerprint) is forced" (sem: SEM-gx-engine-203).
        if let Some(blocker) = entry.blocked_by {
            if self.table.get(&blocker).map(|e| e.state) == Some(Lifecycle::Committed) {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: "Candidate (blocked)",
                    attempted: "verify after a conflicting commit; 43 §8 forces a re-plan",
                });
            }
            if let Some(entry) = self.table.get_mut(id) {
                entry.blocked_by = None;
            }
        }
        if let Some(blocker) = self.conflicting_predecessor(id, mode)? {
            if let Some(entry) = self.table.get_mut(id) {
                entry.blocked_by = Some(blocker);
            }
            // Still `Candidate`, still on the clock. No journal record: 43 §8 adds no transition,
            // and a log entry for "nothing happened" (sem: SEM-gx-engine-204) would make a replay report waiting as an
            // event.
            return Ok(Lifecycle::Candidate);
        }

        // T-3, journal-first.
        self.journal_append(EngineJournalRecord::VerifyStarted {
            transformation: *id,
            at,
        })?;
        self.set_state(id, Lifecycle::Verifying, at);

        let entry = &self.table[id];
        let collected = self.evidence.collect(&entry.transformation, &entry.pre);

        let evidence = match collected {
            Ok(evidence) => evidence,
            Err(_) => return self.unreachable_collector(id, at, key),
        };

        let entry = &self.table[id];
        let adapter = self
            .adapters
            .get(entry.delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", entry.delta.substrate()),
            })?
            .adapter
            .clone();
        // 🔴 **DR-46-26** — `invert` answers an `InvertOutcome` and **E-M4-5**'s fold is now the
        // `inverse` projection of it. What the gate is handed does not move: M4-05 asked for a
        // `can_invert` method and E-M4-5 answered "the engine calls `invert` and folds
        // `Some`/`None`", which is exactly what this still does. The verdict beside it is
        // deliberately **not** forwarded into `GateInput`: that is 41 §5's frozen face and a gate
        // that could see three values would be a different contract, raised rather than taken.
        let invert_available = adapter
            .invert(&entry.delta, &entry.pre)
            .map_err(|e| Error::Adapter {
                action: "invert",
                detail: e.to_string(),
            })?
            .inverse()
            .is_some();

        let planned = PlannedDeltaBytes(entry.delta.payload().to_vec());
        let answered = self.gate.verify(GateInput {
            t: &entry.transformation,
            pre: &entry.pre,
            planned: &planned,
            evidence: &evidence,
            // E-DR4627-1 (DR-46-27). **The one production construction site**, and `at` is this
            // call's `at` -- the moment the caller says the gate is being asked -- rather than a
            // clock read here. 41 §6 injects time at the engine boundary for exactly this reason:
            // a replay hands the recorded `at` back and gets the recorded answer. Note what this
            // is *not*: `entry.transformation.metadata().created_at` is **plan** time (ASM-4
            // metadata), which is what an invariant used to be able to reach and why a window over
            // "now" was unwritable before this field.
            decided_at: at,
            invert_available,
        });

        let verdict = match answered {
            Ok(verdict) => verdict,
            // ⊥ -- see the section above. Not the collector's road, not RecordOnly's business.
            Err(_) => {
                self.journal_append(EngineJournalRecord::Aborted {
                    transformation: *id,
                    reason: AbortReason::InternalError,
                    // No rollback question arises before the critical section: nothing has been
                    // escrowed and nothing has been applied (see `Rollback` for why `None` and
                    // `Some(NotAttempted)` are different facts).
                    rollback: None,
                    at,
                })?;
                return Ok(self.set_state(id, Lifecycle::Aborted(AbortReason::InternalError), at));
            }
        };

        let kind = verdict.kind();
        let digest = verdict.proof_digest().map_err(|e| Error::Malformed {
            detail: format!("the verdict has no canonical form: {e}"),
        })?;
        self.journal_append(EngineJournalRecord::Verdict {
            transformation: *id,
            kind,
            verdict_digest: Some(digest),
            fail_posture_engaged: false,
            at,
        })?;

        let to = match &verdict {
            Verdict::Admit(_) => Lifecycle::Admitted,
            Verdict::Deny(_) => Lifecycle::Denied,
            Verdict::Escalate(_) => Lifecycle::Escalated,
        };
        // T-4c's side effect: "creation of an `EscalationTicket`" (sem: SEM-gx-engine-205). The
        // ticket the gate built carries `Timestamp(0)` because 41 §6 keeps clocks out of that layer,
        // and its `id` is a value the gate minted -- **E-5** injects the one and **E-6** checks the
        // other.
        //
        // 🔴 **M6H3-2 adopted (a)** (sem: SEM-gx-engine-206): the other two arms are kept as well,
        // in the two fields beside `ticket`. Until this hand the `Verdict` was consumed here for its
        // `kind` and its digest and then dropped, so 44 §1.2's `{"kind":"Admit","proof":AdmitProof}`
        // and `{"kind":"Deny","reasons":[Reason]}` had nothing behind them and the HTTP surface's
        // problem `detail` (44 §2.3: "a detailed explanation") had a digest to offer an operator
        // asking "why".
        // The `match` moves the value rather than cloning it, so nothing is stored twice.
        let (ticket, admit_proof, deny_reasons) = match verdict {
            Verdict::Escalate(ticket) => (Some(self.checked_ticket(ticket, at)?), None, None),
            Verdict::Admit(proof) => (None, Some(proof), None),
            Verdict::Deny(reasons) => (None, None, Some(reasons)),
        };
        if let Some(entry) = self.table.get_mut(id) {
            entry.verdict = Some(kind);
            entry.verdict_digest = Some(digest);
            entry.ticket = ticket;
            entry.admit_proof = admit_proof;
            entry.deny_reasons = deny_reasons;
        }
        let state = self.set_state(id, to, at);
        // ASM-14's first kind, for all three verdicts (42 §3.10: "issued for every `Verdict` =
        // Admit/Deny/Escalate, 43 T-4a/T-4b/T-4c") (sem: SEM-gx-engine-207).
        self.issue_verdict_receipt(
            id,
            Some(VerdictSummary {
                kind,
                proof_digest: digest,
            }),
            at,
            key,
        )?;
        Ok(state)
    }

    /// **E-5 / E-6**: the ticket 43 T-4c raised, with the engine's clock in it and its id checked.
    ///
    /// > **E-5**: the ticket's `created_at` is injected by the engine
    /// > **E-6**: reading a ticket back requires the checked constructor (sem: SEM-gx-engine-208)
    ///
    /// The two are one function because the ticket crosses one boundary. gx-gate builds it and
    /// says so in as many words -- "`created_at` is the one field with no honest source. 41 §6
    /// keeps clocks out of this layer...so the value written is `Timestamp(0)` -- the epoch, as a
    /// placeholder the engine overwrites" (sem: SEM-gx-engine-209) -- and this is the overwrite (**E-5**, the same shape
    /// E-M4-31 gave `AppliedDelta.applied_at`).
    ///
    /// **E-6** is the other half. 42 §1.3 makes `TicketId` a digest of the ticket's projection and
    /// gx-gate mints it there; a value arriving from outside is a *claim* that the ticket hashes to
    /// that name until something recomputes it. This does, and refuses a disagreement rather than
    /// filing it — which is the `ReceiptPayload` shape E-6 names, one type over. `created_at` is
    /// outside the projection (ASM-4), so injecting the clock cannot move the id.
    ///
    /// # Errors
    /// [`Error::Canon`] if the ticket has no canonical form, [`Error::InconsistentTicket`] if its
    /// `id` is not the digest of what it holds.
    fn checked_ticket(
        &self,
        mut ticket: EscalationTicket,
        at: Timestamp,
    ) -> Result<EscalationTicket> {
        let minted = TicketId(cid::compute(&ticket)?);
        if minted != ticket.id {
            return Err(Error::InconsistentTicket {
                detail: format!(
                    "the ticket is filed as {} and its contents hash to {}",
                    cid::to_text(&ticket.id.0),
                    cid::to_text(&minted.0)
                ),
            });
        }
        ticket.created_at = at;
        Ok(ticket)
    }

    /// 🔴 **M6H3-10 adopted (b), the answer** (sem: SEM-gx-engine-210) — the ticket a journalled
    /// `Escalate` raised, rebuilt.
    ///
    /// req/38 §50 sent hand 4 to measure before ruling: "measure whether an `EscalationTicket` can
    /// be reconstructed from the row before deciding on the journal-vocabulary growth (a)"
    /// (sem: SEM-gx-engine-210). The measurement is
    /// `crates/gx-engine/tests/ticket_rehydration.rs` and the answer is that it can, because
    /// [`gx_gate::escalation_ticket`] is a function of the `TransformationId` and of two constants
    /// (E-M3-4's one reason and ASM-60-3's approval requirement). ∴ **42 §3.13 does not grow.**
    ///
    /// The rebuild goes through [`Engine::checked_ticket`], which is the point rather than a
    /// formality: E-6's rule is "reading a ticket back requires the checked constructor"
    /// (sem: SEM-gx-engine-211), and this **is** a read
    /// back — of a value nothing stored. It is also the **second producer** of
    /// [`Error::InconsistentTicket`] that §43 M5H6-8② predicted the CLI/API surface would reach:
    /// the first is a gate handing the engine a ticket whose name disagrees with its contents, and
    /// this is a rebuild that hashed to something other than the name it minted — which can only
    /// happen if the one road and the digest have drifted apart, and is precisely the drift that
    /// would make a resumed `gx escalation` operate on a ticket that never existed.
    ///
    /// `created_at` comes from the journalled `Verdict` record's own `at`, so the rebuilt ticket
    /// carries the moment the gate answered rather than the moment somebody resumed. ASM-4 keeps it
    /// out of the identity, so a missing record cannot move the id — it can only leave the field at
    /// the epoch, and that case is the one where no verdict was journalled at all.
    ///
    /// # Errors
    /// [`Error::Canon`] via gx-gate if the ticket has no canonical form,
    /// [`Error::InconsistentTicket`] if the rebuild does not hash to the id it was minted under.
    /// 🔴 The ticket T-4c raised for `id` **as it was raised**, whether or not the row is still
    /// waiting on it (`req/189` H-04, the `GET /stream` half).
    ///
    /// [`Engine::ticket`] answers the **queue**: `Some` exactly while the row is `Escalated`
    /// (`set_state` clears it on the way out). A stream replay needs the other question — "what
    /// did the `Verdict{Escalate}` record at this position name?" — for a row that has since been
    /// ruled, cancelled or forgotten by a restart, and answering it from the live table made
    /// `escalated` events vanish from a replay the moment the row moved on (`req/182` H-04 /
    /// H-02: cursor ordinals shifted). This is that answer, rebuilt from Σ the way a resume
    /// rebuilds it, so the API layer never constructs a ticket itself (rule 1) and the one road
    /// (`gx_gate::escalation_ticket` through [`Engine::checked_ticket`]) stays one.
    ///
    /// `None` when the journal holds no `Escalate` verdict for `id` — the caller asked about a
    /// row that never escalated, and inventing a ticket for it would be the M6H3-10 rebuild
    /// applied to a row it does not describe.
    ///
    /// # Errors
    /// [`Engine::rebuilt_ticket`]'s.
    pub fn ticket_as_raised(&self, id: &TransformationId) -> Result<Option<EscalationTicket>> {
        if let Some(live) = self.table.get(id).and_then(|e| e.ticket.as_ref()) {
            return Ok(Some(live.clone()));
        }
        let escalated = self.journal.records().iter().any(|record| {
            matches!(
                record,
                EngineJournalRecord::Verdict {
                    transformation,
                    kind: VerdictKind::Escalate,
                    ..
                } if transformation == id
            )
        });
        if !escalated {
            return Ok(None);
        }
        self.rebuilt_ticket(id)
    }

    fn rebuilt_ticket(&self, id: &TransformationId) -> Result<Option<EscalationTicket>> {
        // gx-gate's refusal has one cause here — the ticket's projection has no canonical form —
        // and it is spelled as this crate's `Malformed` rather than given a `From`, for the reason
        // E-M3-3 gives: a new `From` on the engine's enum would silently widen what a gate failure
        // can look like from every call site at once.
        let ticket = gx_gate::escalation_ticket(*id).map_err(|e| Error::Malformed {
            detail: format!("the escalation ticket for {id:?} has no canonical form: {e}"),
        })?;
        let at = self.journalled_verdict_at(id).unwrap_or(Timestamp(0));
        self.checked_ticket(ticket, at).map(Some)
    }

    /// When the journal says the gate answered about this transformation.
    ///
    /// The last such record, because 43 T-4a's determinism makes a second one a re-evaluation and
    /// the ticket an operator is holding is the one the latest verdict raised.
    fn journalled_verdict_at(&self, id: &TransformationId) -> Option<Timestamp> {
        self.journal
            .records()
            .iter()
            .rev()
            .find_map(|record| match record {
                EngineJournalRecord::Verdict {
                    transformation, at, ..
                } if transformation == id => Some(*at),
                _ => None,
            })
    }

    /// 🔴 **M6-04 adopted (a)** (sem: SEM-gx-engine-212) — 44 §1.2's `<TICKET_ID>`, resolved to
    /// the transformation it names.
    ///
    /// > 43 T-4c, verbatim: "the ticket id is bound 1:1 to a `TransformationId`"
    /// > (sem: SEM-gx-engine-212)
    ///
    /// The declaration was 1:1 and the mapping was one-directional: [`Engine::ticket`] answers
    /// "which ticket does this transformation have" (sem: SEM-gx-engine-213) and nothing answered the question 44 §1.2's
    /// command line actually asks. This is the inverse, and it is computed from **Σ** rather than
    /// from the in-flight table, so it works in the single-shot process that is the whole of
    /// req/88 §3 Λ2: a fresh `gx escalation approve <TICKET_ID>` has planned nothing and holds no
    /// row, and it still resolves.
    ///
    /// # 🔴 It is a scan, and the cost is written down rather than hidden
    ///
    /// One rebuild per `Escalated` row, so `O(e)` in the number of open escalations rather than
    /// `O(n)` in the ledger — 43's INV-L2 makes every escalation resolve in finite time, so `e` is
    /// the count of what is genuinely awaiting a person. M5H7-3's disease is the other shape (a
    /// full-table walk per call), and the `.gx/index/` cache M6-04(b) describes is available if
    /// this ever measures badly. It is not built today: a cache in front of a function nobody has
    /// measured is the thing M5H8-15 refused.
    ///
    /// # Errors
    /// Anything `Engine::rebuilt_ticket` (private) refuses, unchanged — a rebuild that does not hash to its
    /// own name is a fact about this build, not a "not found" (sem: SEM-gx-engine-214).
    pub fn transformation_of_ticket(&self, ticket: &TicketId) -> Result<Option<TransformationId>> {
        for row in reconstruct(self.journal.records()).transformations() {
            // 🔴 H-04 (`req/189`): the state is asked as well as the verdict. A cancelled
            // escalation keeps `Escalate` as its journalled verdict (T-7 writes `Aborted`, not a
            // ruling), and resolving its ticket here would let a spent ticket name a row —
            // "one rebuild per `Escalated` row" is what the doc above already promised. A ruled
            // row was never reached (T-5 rewrites Σ's verdict), so "6 = not found" for a spent ticket
            // is now the answer for both roads out of `Escalated`.
            if row.verdict != Some(VerdictKind::Escalate) || row.state != Some(Lifecycle::Escalated)
            {
                continue;
            }
            if let Some(rebuilt) = self.rebuilt_ticket(&row.transformation)? {
                if rebuilt.id == *ticket {
                    return Ok(Some(row.transformation));
                }
            }
        }
        Ok(None)
    }

    /// Issue ASM-14's `VerdictReceipt` and file it on the row (**M5H4-6**).
    ///
    /// 42 §3.10's three obligations for the kind are met by construction rather than by a check:
    /// `inclusion_proof` and `postcondition_fingerprint` and `inverse_delta` are all `None`,
    /// because nothing has been appended, nothing has been applied and nothing has been escrowed.
    /// `Receipt::issue` runs `check_schema` before signing, so a hand that changed one of them
    /// would get an [`Error::Witness`] rather than a signed impossible receipt.
    ///
    /// `verdict` is an `Option` and `None` is 43 T-4e's — the case **E-M5-11** exists for.
    /// `canonical_cid` is `id.0`, which is what 42 §3.10 asks for ("`Transformation.id`"
    /// (sem: SEM-gx-engine-215)) and what
    /// `verify_offline` compares against `transformation`; T-8 has not run, and the value it will
    /// fix is the same one, because `id` is outside the IdentityView (42 §1.3, ASM-4).
    ///
    /// # Errors
    /// [`Error::Witness`] if the payload violates ASM-14 or the signature cannot be made.
    /// 🔴 **DR-46-26** — the two attested seats, for a road that **rebuilds** a receipt instead of
    /// issuing one, each taken from where its value survives a crash.
    ///
    /// # The first shape of this was wrong, and the beds said so
    ///
    /// It read both fields back out of `CommitReceiptSink::filed_receipt`. That is the road R13
    /// (`req/244` H-03) added for the `postcondition_fingerprint`, and reading it here looked like
    /// the same move — [`Engine::resume`]'s own comment even says "a rebuild carries what the filed
    /// receipt carried". But R13 **closes the row from that document** when it exists, *before* the
    /// rebuild is attempted, so the rebuild road is by construction the road on which there is no
    /// filed receipt. The two seats were therefore supplied on the road that never runs and left
    /// empty on the road that does, and `crates/gx-cli/tests/model_a_probes.rs` measured the
    /// consequence as `payload_mismatch` on both crash-window beds.
    ///
    /// # Where each value actually is
    ///
    /// * **`reversibility` is derived, not stored.** `InverseStatus` and C-25's verdict are in
    ///   bijection at T-10b by construction: an inverse was escrowed (`Available` or `Pending`,
    ///   and `Consumed` later) exactly when the verdict was `True`; `Unavailable` is `False`;
    ///   `Undetermined` is `Unknown`. The escrow row is journalled, so this reproduces the value
    ///   the commit signed rather than recovering it from anywhere.
    /// * **`read_set` is journalled**, on `InverseEscrowed` (42 §3.13, the `serde(default)` shape
    ///   E-M5-13 gave `pending`). It cannot be derived from anything else: the prior it digests
    ///   stopped existing when `apply` fired, which is the whole reason T-10b runs where it does.
    ///   The **entries** are journalled and `ReadSet::from_reads` re-chooses the granularity here,
    ///   so the tag on a rebuilt receipt is the same function of the same entries (`req/441` §4).
    ///
    /// `BodyMissing` cannot appear: it is produced by [`Engine::inverse_status`]'s read and is
    /// never written into a row, so no journal ever replays one.
    fn rebuilt_attest(
        &self,
        id: &TransformationId,
        status: Option<InverseStatus>,
    ) -> Result<(Option<ReadSet>, Option<Reversibility>)> {
        let verdict = status.map(|status| match status {
            InverseStatus::Unavailable => Reversibility::False,
            InverseStatus::Undetermined => Reversibility::Unknown,
            // 🔴 **R35 / `req/470` L-03** — `Expired` gets its own arm, and the arm is a refusal.
            //
            // It used to sit in the `True` list above, folded in with `Available`, `Consumed`,
            // `Pending` and `BodyMissing`. The exhaustive `match` was written for a good reason —
            // no `_` arm, so an eighth word of the vocabulary cannot be given `True` by default —
            // and it was **giving the seventh word `True` by default** in exactly that way. The
            // doc above ("Where each value actually is") lists five values and never mentions this
            // one, which is what an unexamined arm looks like from the outside.
            //
            // Why a refusal is sound rather than reckless: v0.1 has **no writer** for this value.
            // `store.rs` says so at the declaration ("nothing in this crate moves a status to this
            // value", DR-9 / req/78 N-06), `lifecycle_transitions.rs`'s
            // `three_of_the_four_inverse_statuses_are_written_and_the_fourth_is_dr_9s` asserts the
            // absence against the two shapes a status can be written in and pins the mention count
            // at one, and the replay road cannot produce it either: a journalled status is derived
            // from `(inverse_cid, pending, undetermined)` (`replay.rs:1279-1282`), whose four arms
            // are `Available`, `Pending`, `Undetermined` and `Unavailable`. So no journal, foreign
            // or otherwise, can spell it.
            //
            // What this buys is the day DR-9's commercial tier adds the writer. Folded into `True`,
            // the first thing that happens is a **signed receipt** saying `reversibility: true`
            // about an escrow whose retention has expired — the inverse is gone and the receipt
            // attests it is there. With this arm, that day is a stopped process at the seam
            // instead, next to the two tests that have to be edited to let the writer exist at all.
            InverseStatus::Expired => unreachable!(
                "43 T-10b: this status has no writer in v0.1 (DR-9, req/78 N-06) and \
                 no journal can replay one, so the rebuild cannot meet it. Reaching this is a \
                 writer having been added: give `Expired` its own answer here before it reaches a \
                 signed receipt -- folding it into `True` would attest an inverse that retention \
                 has already dropped (req/470 L-03)"
            ),
            InverseStatus::Available
            | InverseStatus::Consumed { .. }
            | InverseStatus::Pending
            | InverseStatus::BodyMissing => Reversibility::True,
        });
        // 🔴 **DR-46-34** (`req/38` §268 ruling 5, `req/472` §6) — the record **and** whether it
        // records reads, because this is the coordinate at which the two were the same thing.
        //
        // `req/498` measured the funnel: this `find_map` used to end in `unwrap_or_default()`, so a
        // transformation with no `InverseEscrowed` record at all handed an empty `Vec` to
        // `ReadSet::from_reads`, which answered `Ok(None)`, which reached the wire as the `null`
        // an escrow that genuinely read nothing also produced. Three roads, one spelling, three
        // different remedies. The three arms below are those three roads, and each names itself.
        let recorded = self
            .journal()
            .records()
            .iter()
            .find_map(|record| match record {
                EngineJournalRecord::InverseEscrowed {
                    transformation,
                    reads,
                    reads_attested,
                    ..
                } if transformation == id => Some((reads.clone(), *reads_attested)),
                _ => None,
            });
        let read_set = match recorded {
            // The record is there and it records reads. `from_reads` chooses the granularity here
            // and only here (`req/441` §4), and answers `Nothing` for a recorded empty list — the
            // same function of the same entries the issuing road ran, which is what makes the
            // rebuilt payload digest to the leaf the ledger already holds.
            //
            // 🔴 A non-empty list **is** the attestation, so it takes this arm whatever the flag
            // says: a foreign journal spelling `reads_attested: false` beside entries is folded to
            // the honest side here, in `undetermined`'s shape one field over.
            Some((reads, attested)) if attested || !reads.is_empty() => ReadSet::from_reads(reads)
                .map_err(|e| Error::Witness {
                    action: "rebuild the read-set",
                    detail: e.to_string(),
                })?,
            // The record is there and predates 42 §3.13's `reads` (E-M5-13's `serde(default)`
            // shape). What the escrow read is not in this journal, and saying it read nothing
            // would be reporting a gap in a file as a fact about the world.
            Some(_) => ReadSet::ReadsNotJournalled,
            // No record at all: a journal trimmed past it (42 §5), or one that never held it.
            None => ReadSet::NoEscrowRecord,
        };
        Ok((Some(read_set), verdict))
    }

    fn issue_verdict_receipt(
        &mut self,
        id: &TransformationId,
        verdict: Option<VerdictSummary>,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<()> {
        let Some(entry) = self.table.get(id) else {
            return Ok(());
        };
        let payload_kind = verdict.as_ref().map(|v| v.kind);
        // 🔴 **DR-46-28 / DR-46-33** — the attest, from the two facts this road holds: the
        // input-generation stage the `Planned` record fixed at T-2 (read back from the journal so
        // the rebuild roads reproduce it) and whether a verdict was derived. See
        // [`attested_boundary`].
        let determinism_boundary =
            attested_boundary(self.journalled_input_generation(id), verdict.is_some());
        let payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict,
            enforced: entry.enforced,
            // 🔴 **`req/493` §0 / AC-6** — on both kinds and with no kind-dependent rule, unlike
            // the three seats above it. The question this answers is asked of the **process**, and
            // the process that signs a verdict receipt at T-4a is the one that signs the commit
            // receipt at T-11; there is no moment at which the kernel is holding one and not the
            // other. A `Some` unconditionally: see the field's doc for what a `None` would mean.
            confinement: Some(self.confinement.clone()),
            catalogue_hash: None,
            // F7 / R-868-6 (`req/919` W5): every receipt this build issues is `Some`, on both
            // kinds and with no kind-dependent rule -- see the field's own doc comment.
            payload_version: Some(CURRENT_PAYLOAD_VERSION),
            // 🔴 **A2 (`req/910`, `req/919` W8)** — live, for `confinement`'s reason two lines up:
            // T-4a's signing process is the process, and this road has no `StateRow` to read a
            // journalled provenance out of yet. The same string `derive_provenance` will write.
            engine_version: Some(crate::VERSION.to_string()),
            receipt_kind: ReceiptKind::VerdictReceipt,
            canonical_cid: id.0,
            inverse_delta: None,
            transformation: *id,
            inclusion_proof: None,
            fail_posture_engaged: entry.fail_posture_engaged,
            // 🔴 **DR-46-24(A)** — the two seats the read-set erratum opened. `read_set` is `None`
            // on a verdict receipt by ASM-14 (`check_schema` refuses a `Some`); `fingerprint_scope`
            // is P2, and is what a `cas_eq` at the undo road would need (see line 5347).
            //
            // 🔴 **DR-46-26 — this seat stays `None`, and it is the one of the four that stays.**
            // `req/452`'s AC-S1 asked for "the four absent read-sets in `pipeline.rs`: 4 → 0"
            // (the phrase is not spelled here, so that the AC's own `grep` counts code and not
            // this sentence). The lane
            // measured the four before implementing anything and the arithmetic is wrong here:
            // ASM-14 makes a verdict receipt's read-set **always** absent, `check_schema` refuses a
            // `Some`, and `tests/receipt_kind_branch.rs` holds it. A producer written here would
            // not close a gap, it would make the schema refuse every verdict receipt gx issues. The
            // deviation is reported in `req/453` rather than implemented.
            read_set: None,
            reversibility: None,
            // 🔴 **DR-46-45 (`req/973` §B-2)** — `None`, and `ReceiptPayload::check_schema` refuses
            // any other value on this kind. The undo's `parents` exist by T-2 and could be named
            // here; its witness could not, because the CAS guards an apply and a verdict receipt
            // applies nothing. Half a pair is not a smaller claim, it is a different one.
            undo: None,
            // 🔴 **DR-46-28** — the boundary attest, on both kinds: the question it answers
            // (did a gate derive this, and from what kind of input) is asked at verdict time as
            // much as at commit time, so this is the first of the erratum fields with **no**
            // kind-dependent rule.
            determinism_boundary,
            fingerprint_scope: entry.fp0.scope().to_string(),
            precondition_fingerprint: FingerprintBytes(entry.fp0.digest().0),
            postcondition_fingerprint: None,
        };
        let receipt = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
            action: "issue the verdict receipt",
            detail: e.to_string(),
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.verdict_receipts.push(receipt);
            // 🔴 **FR-M04**: the counter is incremented **here**, where the receipt is issued,
            // rather than where the journal record is written. The two are one row apart on every
            // road, and keeping them apart is what lets `tests/ac_vc.rs` recount from the journal
            // and have that be a second opinion instead of a restatement.
            //
            // `None` is 43 T-4e, and it gets the fourth bucket rather than being folded into
            // `admit`: no gate ran, so nothing admitted (M4H4-2, the third application).
            match payload_kind {
                Some(VerdictKind::Admit) => self.verdicts.admit += 1,
                Some(VerdictKind::Deny) => self.verdicts.deny += 1,
                Some(VerdictKind::Escalate) => self.verdicts.escalate += 1,
                None => self.verdicts.unverdicted += 1,
            }
        }
        Ok(())
    }

    /// T-4d and T-4e: the collector could not be reached, and the posture decides which.
    ///
    /// The **only** place `AbortReason::VerifierUnavailable` is written in this workspace
    /// (**M5-03 adopted (a)**, **E-M5-4**) (sem: SEM-gx-engine-216).
    fn unreachable_collector(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Lifecycle> {
        match self.posture {
            // T-4d. 43's guard: "`FailPosture = FailClosed` (DR-2's default, every substrate)"
            // (sem: SEM-gx-engine-217).
            FailPosture::FailClosed => {
                self.journal_append(EngineJournalRecord::Aborted {
                    transformation: *id,
                    reason: AbortReason::VerifierUnavailable,
                    rollback: None,
                    at,
                })?;
                Ok(self.set_state(id, Lifecycle::Aborted(AbortReason::VerifierUnavailable), at))
            }
            // T-4e. 43: "downgrade to record-only mode for this Transformation only and continue;
            // always carve `enforced=false` and `fail_posture_engaged=true` into the receipt"
            // (sem: SEM-gx-engine-218).
            //
            // 🔴 **The receipt is issued here, and hand 6 is the first hand that can issue it.**
            // Until **E-M5-11** the payload had a required `VerdictSummary` and no gate had run, so
            // the only shapes available were a minted empty digest (§32 M4H4-2, refused twice) or
            // no receipt at all -- and 43 T-4e says "always ... carve it in" (sem: SEM-gx-engine-219).
            // The `None` below is the erratum
            // paying out on the exact transition it was ruled for.
            FailPosture::FailOpen => {
                self.journal_append(EngineJournalRecord::Verdict {
                    transformation: *id,
                    kind: VerdictKind::Admit,
                    verdict_digest: None,
                    fail_posture_engaged: true,
                    at,
                })?;
                if let Some(entry) = self.table.get_mut(id) {
                    entry.verdict = None;
                    entry.verdict_digest = None;
                    entry.enforced = false;
                    entry.fail_posture_engaged = true;
                }
                let state = self.set_state(id, Lifecycle::Admitted, at);
                self.issue_verdict_receipt(id, None, at, key)?;
                Ok(state)
            }
        }
    }

    // -----------------------------------------------------------------------
    // T-6 -- the deadlines, and who evaluates them
    // -----------------------------------------------------------------------

    /// **T-6**, for one transformation: abort it if its deadline has passed.
    ///
    /// > **M5-10, adopted (a)+(b) together**: lazy TTL evaluation (liveness) + an explicit
    /// > `reap(now)` API (sweep) (sem: SEM-gx-engine-220)
    ///
    /// 43 T-6's idempotency column is "the reaper fires exactly once per id (idempotent via a
    /// journal-presence check)" (sem: SEM-gx-engine-220), and this shape answers it without a
    /// check: after the abort the state is `Aborted`, which [`Engine::deadline_of`] gives no
    /// deadline, so a second call finds nothing due. The "journal-presence check" is the state
    /// table being a function of the journal (Rule 1) rather than a second query.
    ///
    /// Answers whether it fired, which is what makes [`Engine::reap`] able to report a count rather
    /// than a promise.
    ///
    /// # Errors
    /// [`Error::Io`] from the journal.
    fn expire_if_due(&mut self, id: &TransformationId, now: Timestamp) -> Result<bool> {
        // 🔴 **R3 / `req/222` H-04** — the table first, then the Σ-shadow, which is the same
        // fall-through `Engine::state`, `Engine::intent_of` and `Engine::inverse_status` have had
        // since T6 condition ①. A row this process planned is judged from its own `since`; a row
        // another process planned, or one this process planned before it restarted, is judged from
        // the journal's. Before this line the second kind had no deadline at all and 43 T-6 was a
        // property of a process rather than of a project.
        let deadline = self
            .table
            .get(id)
            .and_then(|entry| self.deadline_of(entry))
            .or_else(|| self.shadow_deadline(id));
        if !deadline.is_some_and(|deadline| now.0 >= deadline.0) {
            return Ok(false);
        }
        // `abort` journals first and moves the table row second (`set_state` is a no-op for a row
        // that is not seated), so a shadow row is expired by writing the record 43 T-6 asks for and
        // nothing else. That is exactly what the row is: a name with a state and no body.
        self.abort(id, AbortReason::Expired, None, now)?;
        Ok(true)
    }

    /// 🔴 **T-6** as a sweep: expire everything whose deadline has passed (**M5-10 adopted (b)**)
    /// (sem: SEM-gx-engine-220).
    ///
    /// The half of M5-10 that lazy evaluation cannot do. INV-L1 and INV-L2 are about *every*
    /// `Candidate`/`Verifying`/`Escalated` reaching a terminal state in finite time, and a
    /// transformation nobody calls an entry point about would otherwise wait forever with a
    /// deadline nothing evaluated. 43 T-6 names no trigger — "the reaper fires exactly once per id" (sem: SEM-gx-engine-222)
    /// (sem: SEM-gx-engine-221) is
    /// the whole of its idempotency column and nothing says who runs it — and v0.1 has no resident
    /// process to run one (`gx serve` is M6, req/78 N-01). So the trigger is a call, and M6's
    /// server is one of the callers it will have.
    ///
    /// Answers the transformations it expired, in `TransformationId` order.
    ///
    /// # Errors
    /// [`Error::Io`] from the journal. A sweep that cannot write stops at the first refusal rather
    /// than continuing: a partially journalled sweep would leave the table and the log disagreeing
    /// about which rows had expired.
    pub fn reap(&mut self, now: Timestamp) -> Result<Vec<TransformationId>> {
        // 🔴 **R3 / `req/222` H-04** — the sweep's domain is the **project**, not this process's
        // memory. Before R3 it was `self.table.keys()`, and after a restart that is empty: a
        // `gx serve` that came up on a project with two hundred expired candidates swept none of
        // them, and the fallback 43 §7.4 (h) named ("the next write that touches the row evaluates
        // it") did not fire either, because the rebuild re-seated `since` at *now*.
        //
        // The shadow is a superset of the table for every row either holds a state for, so it alone
        // would do; the union is written out because a row seated in this process without a record
        // of its own would otherwise be missed silently. Terminal rows cost one `match` each
        // (`deadline_from` answers `None` for eight of the eleven states) and no I/O.
        let mut candidates: Vec<TransformationId> = self.table.keys().copied().collect();
        for id in self.shadow.transformation_ids() {
            if !self.table.contains_key(id) {
                candidates.push(*id);
            }
        }
        let mut expired = Vec::new();
        for id in candidates {
            if self.expire_if_due(&id, now)? {
                expired.push(id);
            }
        }
        expired.sort_by_key(|id| id.0 .0);
        Ok(expired)
    }

    // -----------------------------------------------------------------------
    // 43 §8 -- waiting, and the annotation that is not a state
    // -----------------------------------------------------------------------

    /// 43 §8: is an in-flight transformation on the same `Subject` in conflict with this one?
    ///
    /// > `Commutation::Conflicts{residual}` -> the engine holds `T2` in the wait queue while it
    /// > stays `Candidate` or `Verifying` (no new state is added; only an internal annotation,
    /// > `blocked_by: TransformationId`) (sem: SEM-gx-engine-222)
    ///
    /// # What "precedes" means here, and why the definition matters
    ///
    /// 43 T-3's guard is "there is no **preceding** Transformation, among those in `Conflicts` on
    /// the same Subject" (sem: SEM-gx-engine-223), and a
    /// definition is needed or two `Candidate`s block each other forever — a deadlock that would
    /// satisfy INV-L4 only because the TTL eventually kills both. So "precedes" is defined as
    /// **"has already passed T-3"** (sem: SEM-gx-engine-223): a transformation still at `Draft`
    /// or `Candidate` has not started verifying and
    /// blocks nobody. That makes the waiting a queue with an order rather than a symmetric refusal.
    ///
    /// A `Denied` blocks under `RecordOnly` and not under `Enforce`, because 43 §1 makes `Denied`
    /// terminal "except that under record-only mode, §3's exception branch lets it continue on to
    /// Canonicalized" (sem: SEM-gx-engine-223) — under
    /// `RecordOnly` it is still going to apply something. 🔴 `mode` is an **argument** since M6
    /// hand 3 (**M6-08 adopted (a)**) (sem: SEM-gx-engine-223): the caller is [`Engine::verify`], whose own mode may be a per-call
    /// override of the engine's, and a helper that read `self.mode` would answer this question
    /// under a posture the request did not ask for.
    ///
    /// **M4H6-4** is why the answer may be asked of the adapter each time rather than cached:
    /// "independence is a property of the delta; state transition is the engine's job"
    /// (sem: SEM-gx-engine-224), so `commutation` is a function of the two
    /// deltas and calling it again cannot change its mind. req/78 §3.2 Λ8 is the same statement.
    ///
    /// # Errors
    /// [`Error::NotFound`] if no adapter is registered, [`Error::Adapter`] if `commutation` refuses.
    fn conflicting_predecessor(
        &self,
        id: &TransformationId,
        mode: EnforcementMode,
    ) -> Result<Option<TransformationId>> {
        let Some(this) = self.table.get(id) else {
            return Ok(None);
        };
        let adapter = self
            .adapters
            .get(this.delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", this.delta.substrate()),
            })?
            .adapter
            .clone();

        // 🔴 **M6-07 adopted (b)** (sem: SEM-gx-engine-225) — the rows on this subject, not every row this process has seen.
        //
        // What changed is the **search** and not the answer: the loop below was `for (other_id,
        // other) in &self.table` with the subject comparison as its first `continue`, so the set it
        // reaches is identical and the order is identical (`BTreeSet<TransformationId>` iterates in
        // the same order `BTreeMap<TransformationId, _>` does). What is gone is walking `n` rows to
        // find `k`. `tests/subject_index.rs` asserts the equality against a full scan, and `req/95`
        // carries the before/after measurement §47 M6-07 adopted (b) ordered (sem: SEM-gx-engine-226).
        //
        // `self.by_subject` is read into a `Vec` first because the loop borrows `self.table`
        // immutably while `adapter` is already cloned out — the same reason the old loop could not
        // call anything on `&mut self` either.
        let siblings = self
            .by_subject
            .get(&this.transformation.subject)
            .map(|ids| ids.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for other_id in &siblings {
            if other_id == id {
                continue;
            }
            let Some(other) = self.table.get(other_id) else {
                continue;
            };
            debug_assert_eq!(
                other.transformation.subject, this.transformation.subject,
                "the subject index put a row under a subject that is not its own"
            );
            let in_flight = match other.state {
                Lifecycle::Draft | Lifecycle::Candidate => continue,
                Lifecycle::Denied => mode == EnforcementMode::RecordOnly,
                state => !state.is_terminal(),
            };
            if !in_flight {
                continue;
            }
            let answer = adapter
                .commutation(&other.delta, &this.delta)
                .map_err(|e| Error::Adapter {
                    action: "commutation",
                    detail: e.to_string(),
                })?;
            if matches!(answer, Commutation::Conflicts { .. }) {
                return Ok(Some(*other_id));
            }
        }
        Ok(None)
    }

    // -----------------------------------------------------------------------
    // T-5, T-5b
    // -----------------------------------------------------------------------

    /// **T-5 / T-5b** `escalation` — a person answers, and the answer is signed (AC-071, AC-072).
    ///
    /// 43 T-5's row: from `Escalated`, guard "the ruler holds a valid signing key", side effect
    /// "append a signed human-ruling receipt to the provenance chain; journal:
    /// `HumanDecision{id, Admit}`" (sem: SEM-gx-engine-227), to `Admitted`.
    /// T-5b is the same row with `Deny` and `Denied`. The guard is the `key` argument: an engine
    /// that took no key could not have issued the receipt the row requires, so "holds a valid
    /// signing key" is a type rather than a check.
    ///
    /// # INV-S6 is what this function is for
    ///
    /// > `Escalated` does not automatically transition to `Admitted`/`Denied` without going through
    /// > T-5/T-5b's signed human-ruling receipt (sem: SEM-gx-engine-228)
    ///
    /// There is no other road out of `Escalated` except T-6's expiry and T-7's cancel, and both
    /// land in `Aborted`. `tests/ac_071.rs` measures the absence from the other side.
    ///
    /// # Three refusals, and each one is a fact that would otherwise be invented
    ///
    /// * **not `Escalated`** — [`Error::InvalidState`]. 43 has no `Escalated → Escalated` edge and
    ///   no human ruling on anything else.
    /// * **`decision = Escalate`** — 42 §3.13: "kind is Admit|Deny only" (sem: SEM-gx-engine-229). A person escalating an
    ///   escalation is a request for a state 43 §1 does not have.
    /// * **an empty reason** — 44 §1.2's trigger is `--reason <text>` and AC-071/072 both require
    ///   the reason to reach the trail. `Verdict::deny` refuses an empty `Vec<Reason>` in gx-gate
    ///   for the same reason: a refusal that says nothing is a refusal nobody can audit.
    ///
    /// # Errors
    /// [`Error::InvalidState`] for the first two above, [`Error::Malformed`] for the third,
    /// [`Error::NotFound`] for an unknown id, [`Error::Canon`] if the ruling has no canonical form,
    /// [`Error::Io`] from the journal, [`Error::Witness`] if the receipt cannot be issued.
    pub fn escalation(
        &mut self,
        id: &TransformationId,
        ruling: &HumanRuling,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Lifecycle> {
        // 43 T-6 first, so that INV-L2's deadline is not overtaken by a late ruling. Which of the
        // two wins is not written in 43; the engine takes the earlier one, because the expiry
        // already happened at the moment the deadline passed and this call is later.
        self.expire_if_due(id, at)?;

        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        if entry.state != Lifecycle::Escalated {
            return Err(Error::InvalidState {
                id: format!("{id:?}"),
                state: entry.state.name(),
                attempted: "escalation",
            });
        }
        let to = match ruling.decision {
            VerdictKind::Admit => Lifecycle::Admitted,
            VerdictKind::Deny => Lifecycle::Denied,
            VerdictKind::Escalate => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: "Escalated",
                    attempted: "escalation(Escalate); 42 §3.13 admits Admit and Deny only",
                })
            }
        };
        if ruling.reason.trim().is_empty() {
            return Err(Error::Malformed {
                detail: "a human ruling with no reason cannot be audited (44 §1.2's `--reason`, \
                         AC-071/072); `Verdict::deny` refuses the same emptiness in gx-gate"
                    .to_string(),
            });
        }

        // The digest of what the person decided -- see [`HumanRuling`] for why it is of this value
        // and not of the ticket the gate raised.
        let proof_digest = cid::compute(ruling)?;

        // Journal-first (43 §7). The receipt is the external side effect and follows.
        self.journal_append(EngineJournalRecord::HumanDecision {
            transformation: *id,
            kind: ruling.decision,
            reason: ruling.reason.clone(),
            actor: ruling.actor.clone(),
            // 🔴 **DR-46-31** — the same value the receipt below is issued under, written to the
            // journal so that a Σ rebuilt without this process can name it. The table assignment
            // three lines down is what `replay.rs` had no way to reproduce.
            verdict_digest: Some(proof_digest),
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.verdict = Some(ruling.decision);
            entry.verdict_digest = Some(proof_digest);
        }
        let state = self.set_state(id, to, at);
        self.issue_verdict_receipt(
            id,
            Some(VerdictSummary {
                kind: ruling.decision,
                proof_digest,
            }),
            at,
            key,
        )?;
        Ok(state)
    }

    // -----------------------------------------------------------------------
    // T-7
    // -----------------------------------------------------------------------

    /// **T-7** `cancel` — the owner stops it before the critical section (AC-073, DR-11, FR-059).
    ///
    /// 43 T-7's from-set is `{Draft, Candidate, Verifying, Admitted, Canonicalized, Escalated}`,
    /// its guard is "the actor holds owner authority (equivalent to `Actor::Human{key}`); before
    /// `Committing` is reached", and its idempotency column is "a duplicate cancel is ignored as a
    /// no-op (already `Aborted`)" (sem: SEM-gx-engine-230).
    ///
    /// # 🔴 `Draft` is in 43's from-set and is not reachable here
    ///
    /// A draft has no `TransformationId` (43 T-1, **E-M5-3**) and `Aborted` is keyed on one, so
    /// there is no record this engine could write about cancelling a draft — and no row to move,
    /// because **M5-17 adopted (b)** (sem: SEM-gx-engine-231) keeps the draft phase in the journal alone. Cancelling a draft is
    /// therefore **unrepresentable in v0.1**, and it is written down rather than quietly dropped:
    /// raised as **M5H6-1**. The cost is bounded — a draft holds no `PlannedDelta`, has read
    /// nothing and will expire from nothing, because 43 T-6 does not reach `Draft` either.
    ///
    /// # 🔴 The owner-permission guard has no enforcement point, and that is stated rather than faked
    ///
    /// 43 T-7 requires the actor to hold owner permission. v0.1 has no authorization layer: 44's
    /// API surface is M6 (req/78 N-01), the `Aborted` record has no actor field, and nothing in the
    /// engine knows who owns a transformation. Taking an `Actor` argument and dropping it would be
    /// worse than not taking one — a value the caller supplied and nothing recorded. So the guard
    /// is **unenforced**, this sentence is the disclosure §37 asks for when a check is absent
    /// (write "the check's absence" as one line in the doc, undisguised (sem: SEM-gx-engine-232)), and **M5H6-4** is the ticket.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown id. [`Error::InvalidState`] from `Committing`,
    /// `Committed`, `Superseded` and from `Denied` (which 43 T-7's from-set does not include).
    /// [`Error::Io`] from the journal.
    pub fn cancel(&mut self, id: &TransformationId, at: Timestamp) -> Result<Lifecycle> {
        self.expire_if_due(id, at)?;

        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        match entry.state {
            // 43 T-7's idempotency column. No second record: a journal that grew on every repeated
            // cancel would report re-entries as events (T-1 and T-8 take the same early return).
            Lifecycle::Aborted(reason) => return Ok(Lifecycle::Aborted(reason)),
            Lifecycle::Candidate
            | Lifecycle::Verifying
            | Lifecycle::Admitted
            | Lifecycle::Canonicalized
            | Lifecycle::Escalated => {}
            state => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: state.name(),
                    attempted: "cancel",
                })
            }
        }
        // No rollback question: T-7 cannot fire from `Committing`, so nothing has been escrowed and
        // nothing applied (see `Rollback` for why `None` and `Some(NotAttempted)` differ).
        self.abort(id, AbortReason::OwnerCancelled, None, at)
    }

    /// 🔴 **The `Committed` row a second process does not have** (M6 hand 4's finding, M6H4-4).
    ///
    /// M6H3-1 named the hole one state earlier — "`gx verify`/`gx commit` hold no row across a
    /// separate process" — and answered it with "the body is a re-plan; the state is the journal"
    /// (sem: SEM-gx-engine-233). That answer **stops at
    /// `Committed`**, and `gx undo <TID>` is the verb that walks into the wall: a re-plan reads the
    /// substrate, a successful commit has *moved* the substrate, so the recomputed `TransformationId`
    /// is a different one and [`Engine::plan`] refuses (correctly). 43 §5 nonetheless makes `undo` an
    /// operation on a **`Committed`** transformation, so 44 §1.2's `gx undo` is unreachable from a
    /// single-shot CLI without this.
    ///
    /// # 🔴 Nothing is invented and **the journal's vocabulary does not grow**
    ///
    /// Every field of the row comes from Σ, from the blob store, or from the caller's `Intent` — the
    /// draft `.gx/drafts/` is holding, which is where 44 §0's id-resolution already sends a CLI:
    ///
    /// | field | where it comes from |
    /// |---|---|
    /// | `subject` | **`Provenance.input_objects[0]`** — 42 §3.9's list is "the set of input snapshots the adapter read" (sem: SEM-gx-engine-234) and in v0.1 the engine watches the adapter read exactly one, T-2's, whose `ObjectId` *is* the subject `Engine::derive_provenance` (private) |
    /// | `intent_id`, `delta_cid`, `fp0`, `superseded_by` | the `StateRow` |
    /// | `parents`, `locator`, `created_at` | the `Planned` record (**E-M5-13**'s two fields do the work they were added for) |
    /// | `context`, `actor`, `substrate` | the `Intent` the caller passes |
    /// | the delta body | the blob store (**E-M4-8**: keeping it is what makes replay and undo constructible at all) |
    ///
    /// # 🔴 The rebuild proves itself
    ///
    /// A reconstruction that guessed would be worse than none, so the rebuilt `Transformation` is
    /// **re-identified**: its CID is computed and compared with the id that was asked for, and a
    /// disagreement is [`Error::InvalidState`] rather than a row. Content addressing makes that a
    /// proof rather than a check — 42 §1.3 puts every field of the identity view into the digest, so
    /// a rebuild that hashes to the recorded name differs from the original in nothing that matters.
    ///
    /// `pre` is the one field that is **not** the historical value, and it is not fabricated either:
    /// it is a fresh `adapter.snapshot(locator)`, which is what an `ObjectSnapshot` is defined to be
    /// ("The object as it is now" (sem: SEM-gx-engine-235)). ASM-9 does not store content, so the snapshot T-2 took is gone
    /// and its *digest* survives in `fp0`. Nothing reads a committed row's `pre` except `undo`, which
    /// reads its locator; a rebuilt row that pretended to hold the old snapshot would be the lie this
    /// avoids.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the transformation has no adapter, no delta body, or (for a committed
    /// row) no provenance — the last is a journal written by something that is not this code.
    /// [`Error::InvalidState`] if the rebuild does not re-identify. [`Error::Io`] from the blobs.
    pub fn rehydrate_committed(&mut self, id: &TransformationId, intent: &Intent) -> Result<bool> {
        if self.table.contains_key(id) {
            return Ok(true);
        }
        let sigma = reconstruct(self.journal.records());
        let Some(row) = sigma.state_of(id).cloned() else {
            return Ok(false);
        };
        if !matches!(
            row.state,
            Some(Lifecycle::Committed | Lifecycle::Superseded)
        ) {
            return Ok(false);
        }

        let missing = |what: &'static str| Error::NotFound {
            what,
            id: format!("{id:?}"),
        };
        let provenance = row.provenance.clone().ok_or_else(|| {
            missing("provenance for a committed transformation (42 §3.9, M5-25 adopted (a); sem: SEM-gx-engine-236)")
        })?;
        let intent_id = row.intent_id.ok_or_else(|| missing("intent id"))?;
        let delta_cid = row
            .delta_cid
            .ok_or_else(|| missing("planned delta reference"))?;
        let delta = self.blobs.get(&delta_cid)?;
        let fp0 = row
            .fp0
            .clone()
            .ok_or_else(|| missing("precondition fingerprint"))?
            .into_fingerprint()?;
        let (locator, parents, created_at) = self
            .planned_record(id)
            .ok_or_else(|| missing("the Planned record"))?;
        let subject = self
            .rebuilt_subject(&sigma, id)
            .ok_or_else(|| missing("the subject snapshot in the provenance record"))?;

        let mut transformation = Transformation::new(
            TransformationId(Cid([0u8; 32])),
            0,
            Subject::Object(subject),
            None,
            parents,
            CompositionMetadata {
                intent_id,
                delta: delta.reference().clone(),
                context: intent.context().clone(),
                actor: intent.actor().clone(),
                created_at,
            },
        )?;
        let rebuilt = TransformationId(cid::compute(&transformation)?);
        if rebuilt != *id {
            return Err(Error::InvalidState {
                id: format!("{id:?}"),
                state: "Committed",
                attempted: "rehydrate: the rebuilt transformation names another id, so the intent \
                            supplied is not the one this transformation was planned from",
            });
        }
        transformation.id = *id;

        let adapter = self
            .adapters
            .get(delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", delta.substrate()),
            })?
            .adapter
            .clone();
        let pre = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;

        self.seat(
            *id,
            Entry {
                intent_id,
                transformation,
                state: row.state.unwrap_or(Lifecycle::Committed),
                since: created_at,
                blocked_by: None,
                delta,
                fp0,
                pre,
                verdict: row.verdict,
                verdict_digest: row.verdict_digest,
                enforced: row.enforced,
                fail_posture_engaged: row.fail_posture_engaged,
                canonical_cid: row.canonical_cid,
                ticket: None,
                admit_proof: None,
                deny_reasons: None,
                verdict_receipts: Vec::new(),
                superseded_by: row.superseded_by,
                apply_started: row.apply_started,
                observation_cid: row.observation_cid,
                rollback: row.rollback,
                provenance: Some(provenance),
                inverse_cid: None,
                applied_at: None,
                receipt: None,
            },
        );
        // 42 §3.12's row, verbatim from Σ. `status` matters as much as the CID: a `Consumed` inverse
        // is what stops a second undo of one commit, and a rebuild that reset it to `Available`
        // would make "only once" false across a restart (sem: SEM-gx-engine-237).
        if let Some(escrow) = sigma
            .escrow()
            .iter()
            .find(|e| e.transformation == *id)
            .cloned()
        {
            self.escrow.insert(*id, escrow);
        }
        // T-12's idempotency column reads this index, and Σ carries the edge.
        if let Some(by) = row.superseded_by {
            self.supersedes.record(*id, by);
        }
        Ok(true)
    }

    /// 🔴 **Lane R2** — the `Subject` a committed row is rebuilt with, and why it is not always
    /// this row's own provenance.
    ///
    /// For an ordinary transformation the two are the same thing: [`Engine::plan`] takes
    /// `Subject::Object(*pre.id())` from the snapshot it read, and the commit's provenance records
    /// that snapshot as its first input object (42 §3.9). So `provenance.input_objects[0]` **is**
    /// the subject, and [`Engine::rehydrate_committed`] has read it that way since M6H4-4.
    ///
    /// An undo is the exception, and it is an exception 43 §5-1 asks for: `T_u` carries `T_o`'s
    /// context, actor **and subject** (P-5 — the undo is a change to the same thing, by whoever's
    /// change is being taken back), while `T_u`'s own precondition snapshot is the world *after*
    /// `T_o` applied. ∴ for `T_u` the two disagree, and rebuilding it from its own provenance
    /// produces a `Transformation` whose CID is not `T_u`'s — which is exactly what `req/216`
    /// measured as "the road stops at the missing draft": the draft was the first wall, and this
    /// was the second one behind it.
    ///
    /// So the subject is taken from the **root of the parents chain**. 43 T-12's guard makes
    /// `T_u.parents` contain `T_o.id` and [`Engine::plan`] leaves `parents` empty for everything
    /// else, so a non-empty list means "this is an undo of the row it names", and walking to the
    /// row that has none reaches the transformation whose provenance recorded the subject they all
    /// share. An undo of an undo walks two steps; the chain is bounded by the number of rows Σ
    /// holds, and the walk is bounded by that number rather than trusted to terminate.
    ///
    /// The identity check in [`Engine::rehydrate_committed`] is still the proof: a subject this
    /// function got wrong cannot produce the CID the caller asked for, so a wrong answer here is a
    /// refusal and never a silently mis-rebuilt row.
    fn rebuilt_subject(&self, sigma: &Sigma, id: &TransformationId) -> Option<ObjectId> {
        let mut at = *id;
        for _ in 0..=sigma.transformations().len() {
            let parents = self.planned_record(&at).map(|(_, parents, _)| parents);
            match parents.as_deref().and_then(<[TransformationId]>::first) {
                Some(parent) => at = *parent,
                None => {
                    return sigma
                        .state_of(&at)
                        .and_then(|row| row.provenance.as_ref())
                        .and_then(|p| p.input_objects.first())
                        .copied()
                }
            }
        }
        None
    }

    /// The `Planned` record's locator, parents and moment (**E-M5-13**).
    ///
    /// Not in `StateRow`: Σ's reconstruction keeps what a *state* is made of, and these three are
    /// facts about the planning event. The last such record wins, for [`Engine::journalled_verdict_at`]'s
    /// reason.
    fn planned_record(
        &self,
        id: &TransformationId,
    ) -> Option<(String, Vec<TransformationId>, Timestamp)> {
        self.journal
            .records()
            .iter()
            .rev()
            .find_map(|record| match record {
                EngineJournalRecord::Planned {
                    transformation,
                    locator,
                    parents,
                    at,
                    ..
                } if transformation == id => Some((locator.clone(), parents.clone(), *at)),
                _ => None,
            })
    }

    /// 🔴 **DR-46-33 / DR-46-28** (`req/38` §413) — the input-generation stage of a transformation
    /// being planned, the join `gx_core::DeterminismBoundary::Mixed`'s `input_generation` doc names.
    ///
    /// An `Actor::Agent` is an LLM origin whatever a static file declared, so the actor wins the
    /// join; otherwise the answer is the substrate's declaration, or `Unknown` for a substrate that
    /// registered none. Computed at plan time (T-2) because the actor is fixed then and the result
    /// — not the actor — is what the `Planned` record carries across a crash window (see the field's
    /// doc in `store.rs` and `attested_boundary`).
    fn joined_input_generation(&self, substrate: &SubstrateKind, actor: &Actor) -> BoundaryStage {
        if matches!(actor, Actor::Agent { .. }) {
            return BoundaryStage::LlmOriginated;
        }
        self.input_stage_declarations
            .get(substrate)
            .map_or(BoundaryStage::Unknown, |d| d.declared_input_stage())
    }

    /// 🔴 **DR-46-33** — the input-generation stage the `Planned` record fixed for `id`, read back
    /// from the journal so the live road and the rebuild road answer alike (43 §7-3b).
    ///
    /// The last `Planned` for `id` wins, for [`Engine::planned_record`]'s reason (an `undo` re-plan
    /// writes a second one). `Unknown` for a transformation with no `Planned` record and for a
    /// journal written before this field, which reproduces v0's `attested_boundary` value exactly.
    fn journalled_input_generation(&self, id: &TransformationId) -> BoundaryStage {
        self.journal
            .records()
            .iter()
            .rev()
            .find_map(|record| match record {
                EngineJournalRecord::Planned {
                    transformation,
                    input_generation,
                    ..
                } if transformation == id => Some(*input_generation),
                _ => None,
            })
            .unwrap_or(BoundaryStage::Unknown)
    }

    /// 🔴 **DR-46-45 (`req/973` §B-1/§B-2)** — the undo attestation a committed receipt carries,
    /// read back out of the `Planned` record so the live road and the two rebuild roads answer
    /// alike (43 §7-3b).
    ///
    /// `None` for a transformation that is not an undo, and for an undo planned by a build that
    /// predates this erratum — the second reproduces the absence in the filed receipt rather than
    /// inventing a claim about a comparison nobody recorded.
    ///
    /// The last `Planned` for `id` wins, for [`Engine::planned_record`]'s reason: `req/38` §98
    /// ruling 2's `--retry` road re-plans an `Aborted(ApplyFailed)` undo and writes a second record.
    /// Each attempt performed its own CAS, so the last one is the one that guarded the apply this
    /// receipt is about.
    ///
    /// **Both halves come from the same record.** Taking `undoes` from the in-memory
    /// `Transformation` and the witness from Σ would give the two rebuild roads a seat they cannot
    /// fill — the repairing process holds no table — which is the shape `req/440` §0-3 warns about
    /// (two sources for one fact can disagree; one source cannot).
    #[must_use]
    fn journalled_undo(&self, id: &TransformationId) -> Option<UndoAttestation> {
        let (parents, witness) =
            self.journal
                .records()
                .iter()
                .rev()
                .find_map(|record| match record {
                    EngineJournalRecord::Planned {
                        transformation,
                        parents,
                        undo_witness,
                        ..
                    } if transformation == id => Some((parents.clone(), undo_witness.clone())),
                    _ => None,
                })?;
        // 43 T-12 fixes `T_u.parents` as `[T_o.id]` and `Engine::undo` is its one producer, so the
        // first element is the original. A `Planned` that carries a witness and no parent is a
        // journal this build cannot have written; it is answered `None` rather than `expect`ed,
        // because 41 §6 counts a panic as a bug.
        Some(UndoAttestation {
            undoes: *parents.first()?,
            witness: witness?,
        })
    }

    // -----------------------------------------------------------------------
    // T-12 -- undo
    // -----------------------------------------------------------------------

    /// **T-12, first half** `undo` — build the transformation that will undo a committed one.
    ///
    /// 43 §5 is unambiguous about what this is and is not:
    ///
    /// > 1. Undo is a **normal Transformation** (T_u) that begins with a fresh `submit(intent)` ...
    /// > 2. T_u independently passes through its own `Draft->Candidate->Verifying->...->Committed`
    /// >    (being an undo does not exempt it from verification -- fail-closed, P-4 keeps applying)
    /// >    (sem: SEM-gx-engine-238)
    ///
    /// So this function **does not undo anything**. It creates a `Candidate`, and the caller then
    /// drives [`Engine::verify`], [`Engine::canonicalize`] and [`Engine::commit`] exactly as for any
    /// other transformation — which is what AC-040's second case measures, where a policy denies
    /// the undo and `T_o` stays `Committed`. The supersede edge is drawn by that commit, not by
    /// this call: see `Engine::supersede_after_commit` (private).
    ///
    /// # 🔴 Why it is not literally a fresh `submit(intent)` (sem: SEM-gx-engine-239)
    ///
    /// 43 §5-1 says T_u's intent means "apply T_o's escrowed inverse (already escrowed at T-10b)"
    /// (sem: SEM-gx-engine-239),
    /// and 41 §4 gives no way to write that intent down: `plan(intent, pre)` is the adapter's
    /// function from a goal to a delta, and there is no goal that an arbitrary adapter is
    /// guaranteed to plan into *this particular* escrowed delta. An engine that called `plan` and
    /// hoped would be undoing something else on the day the two disagreed.
    ///
    /// So the escrowed delta is used **directly** as T_u's delta, and the `Intent` is minted for
    /// its identity alone (`IntentId` is the CID of all five fields, 42 §1.3): same substrate, same
    /// locator, `GoalBytes` = the inverse's payload, and T_o's context and actor. Both journal
    /// records 43 §5-1 implies are written — `DraftCreated` then `Planned` — so a replay sees a
    /// transformation that began normally, because it did. What is skipped is `adapter.plan`, and
    /// the skip is raised as **M5H6-5**.
    ///
    /// `Fingerprint₀` is taken **now**, against a fresh snapshot: the undo's precondition is the
    /// world as `T_o` left it, not the world `T_o` was planned against. Without that, T-10a's CAS
    /// would refuse every undo.
    ///
    /// # 🔴 The world `T_o` left it in is now **checked**, not assumed (**DR-43-1, adopted (a)**)
    ///
    /// The paragraph above states what `Fingerprint₀` is; until `req/38` §132 ruling 2 it also
    /// stated the whole of the guard, and `req/182` H-15 measured what that cost: a forward commit
    /// `AAA -> BBB`, a third party writing `CCC`, and `gx undo` answering `RC 0 / Committed` with
    /// the file back at `AAA` -- `CCC` gone, and nothing anywhere saying so. `Fingerprint₀` taken
    /// *now* means T-10a's CAS only ever guards the window between this call and its commit; it
    /// says nothing about the window between `T_o`'s commit and this call, which is the window an
    /// operator is actually asking about when they press undo.
    ///
    /// So the caller now brings [`UndoWitness`] -- `T_o`'s own signed `postcondition_fingerprint`
    /// (42 §3.10), the value `T_o` attested about the world immediately after its apply -- and this
    /// function compares it against the fresh snapshot **before** anything is minted or journalled.
    /// A mismatch is [`UndoRefusal::WorldMoved`]: 44's `PRECONDITION_CHANGED` (exit 3, HTTP 409),
    /// no new number. What is deliberately accepted with it is the other side of the ruling: a
    /// third party who moved the world *legitimately* now blocks the undo, and the operator has to
    /// say what to do about it. That is P-3 (do not give a guarantee that is not there) paid for in
    /// refusals rather than in silence.
    ///
    /// The witness may also be [`UndoWitness::Unobservable`], which is **declared and not refused**
    /// (DR-46-7, `req/38` §123 ruling 1): a receipt that never carried a postcondition, or a
    /// substrate whose position cannot be read, leaves nothing to compare, and the undo proceeds
    /// exactly as it did before this ruling. The caller says which absence it is out loud.
    ///
    /// # 🔴 Nothing is written on a refusal
    ///
    /// The snapshot, the CAS and every one of [`UNDO_REFUSALS`]'s judged rows are evaluated
    /// **before** the `DraftCreated` record and before the intent is minted, so a refused undo
    /// appends no journal record, mints no `Transformation`, touches no ledger and issues no
    /// receipt. That is aider's property ("refuse and do nothing") and it is also the only shape 42
    /// §3.10 leaves available: a receipt is signed evidence *about a transformation*, and a refused
    /// undo has none. The cost is recorded rather than hidden -- a third party cannot verify from
    /// the ledger that an undo was refused, only that none happened -- and is `req/216` §7's
    /// residual.
    ///
    /// # Errors
    /// Every refusal is a [`UndoRefusal`] carried by [`UndoRefusal::into_error`], so the surface
    /// vocabulary is unchanged: [`Error::NotFound`] if `original` is unknown, if it escrowed no
    /// inverse, if the inverse's body is not in the blob store, or if its substrate has no
    /// registered adapter; [`Error::InvalidState`] if `original` is not `Committed`, or if its
    /// inverse has already been consumed by another undo (42 §3.12's `Consumed`);
    /// 🔴 [`Error::WorldMoved`] if the witness and the live world disagree. [`Error::Adapter`] from
    /// `snapshot` or `precondition`. [`Error::Canon`], [`Error::Core`] and [`Error::Io`] as `plan`
    /// raises them.
    /// 🔴 **Lane R2 (`req/38` §148 ruling 1(iii))** — the intent an undo of `original` is planned
    /// from, computed without minting, journalling or applying anything.
    ///
    /// The single definition of "what an undo's intent is", and [`Engine::undo`] itself is one of
    /// its two callers. The other is the surface that owns the draft archive: `req/182` H-16
    /// measured that `undo` minted this value in memory and wrote no draft, so a second process
    /// asked to undo `T_u` had a `Committed` row in Σ, a body nowhere, and 44 §1.4's 6 as its
    /// answer (`req/216` §3). A caller that recomputed the five fields for itself would be a
    /// second definition of an identity — 42 §1.3 row 2 puts all five in the `IntentId` — so the
    /// caller asks here and files what it is given.
    ///
    /// `None` for a transformation this process holds no row for, one with no escrow row, and one
    /// whose escrow names no inverse. Those are three of [`UNDO_REFUSALS`]' judged rows and this
    /// function does **not** classify them: it is a read, the refusals belong to `undo`, and a
    /// second classifier would be a second table.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::NotFound`] from the blob store, if the escrowed body is named and
    /// absent.
    pub fn undo_intent(&self, original: &TransformationId) -> Result<Option<Intent>> {
        let Some(entry) = self.table.get(original) else {
            return Ok(None);
        };
        let Some(inverse_cid) = self.escrow.get(original).and_then(|row| row.inverse_cid) else {
            return Ok(None);
        };
        let inverse = self.blobs.get(&inverse_cid)?;
        Ok(Some(Intent::new(
            entry.delta.substrate().clone(),
            entry.pre.locator().to_string(),
            GoalBytes(inverse.payload().to_vec()),
            entry.transformation.context.clone(),
            entry.transformation.actor.clone(),
        )))
    }

    pub fn undo(
        &mut self,
        original: &TransformationId,
        witness: &UndoWitness,
        rng_seed: u64,
        at: Timestamp,
    ) -> Result<(IntentId, TransformationId)> {
        // 🔴 **T6 condition ①, and the honest half of what it does not close** (`req/38` §148,
        // `req/190` §4-1 L2). After a restart the Σ-shadow knows this row's *state* and holds no
        // body for it, and an undo needs the body: the subject, the context, the actor, the locator
        // and the substrate all come from the `Transformation`, and 42 §3.13 records names and
        // digests rather than bodies (ASM-9). So the refusal names which of the two it is. A caller
        // reading "no transformation" for a row `GET /transformations/{id}` had just answered `200`
        // about would be reading a contradiction; a caller reading "the state is Committed and this
        // process holds no body" is reading the truth, and knows that the answer is a draft archive
        // (lane R2) rather than a retry.
        let entry = match self.table.get(original) {
            Some(entry) => entry,
            None => {
                return Err(match self.shadow.row(original).and_then(|r| r.state) {
                    Some(state) => UndoRefusal::NoBody {
                        state: state.name(),
                    },
                    None => UndoRefusal::NotOurs,
                }
                .into_error(original))
            }
        };
        if entry.state != Lifecycle::Committed {
            return Err(UndoRefusal::NotCommitted {
                state: entry.state.name(),
            }
            .into_error(original));
        }
        let subject = entry.transformation.subject;
        let context = entry.transformation.context.clone();
        let actor = entry.transformation.actor.clone();
        let locator = entry.pre.locator().to_string();
        let substrate = entry.delta.substrate().clone();

        // 🔴 **`req/824` A5** — an observation's undo is refused by TYPE, before the escrow is
        // even consulted. The escrow row exists (the record-level restore was escrowed at
        // T-10b), so falling through would execute a "successful" undo — and un-reporting an
        // observation is either a write to a platform this system can never write (SS273) or,
        // for a log window, an un-attestation of history. Both kinds fold onto
        // `INVERSE_UNAVAILABLE` at the surface with the distinction in `detail` (A3's ruled
        // fold, Λ4); both are constructible engine-side only (`authority_boundary.rs` holds the
        // secondary surfaces at zero constructions).
        if is_observation_substrate(&substrate) {
            let id_text = original.0.to_text();
            return Err(
                match cbor::decode::<ObservationDelta>(entry.delta.payload()) {
                    Ok(delta) if delta.class == ObservationClass::LogWindow => {
                        Error::AppendOnlyClass { id: id_text }
                    }
                    _ => Error::InverseNotExecutableAtSubstrate { id: id_text },
                },
            );
        }

        let row = self
            .escrow
            .get(original)
            .ok_or_else(|| UndoRefusal::NoEscrow.into_error(original))?;
        // 42 §3.12's `Consumed` is what makes "only once" a fact rather than a hope (sem: SEM-gx-engine-240): a second undo
        // of the same commit would be a second transformation claiming the same inverse.
        if matches!(row.status, InverseStatus::Consumed { .. }) {
            return Err(UndoRefusal::AlreadyUndone.into_error(original));
        }
        // 🔴 Two-phase escrow: a `Pending` row outside the critical section is a crash's
        // trace (the completion never journalled its outcome). A partial inverse is not an
        // executable one — refusing by name here is the fail-safe side, and `Engine::recover`
        // is the road that completes or folds it.
        if matches!(row.status, InverseStatus::Pending) {
            return Err(UndoRefusal::InversePending.into_error(original));
        }
        let inverse_cid = row
            .inverse_cid
            .ok_or_else(|| UndoRefusal::InverseUnavailable.into_error(original))?;
        let inverse = self.blobs.get(&inverse_cid)?;

        let adapter = self
            .adapters
            .get(&substrate)
            .ok_or_else(|| {
                UndoRefusal::NoAdapter {
                    substrate: format!("{substrate:?}"),
                }
                .into_error(original)
            })?
            .adapter
            .clone();

        // 🔴 **DR-43-1(a)** — T-2 is read *before* T-1 mints anything, because the answer it
        // produces may be a refusal and a refused undo must leave no record (see this function's
        // doc). The two calls and their order are unchanged from the version that ran after the
        // mint; what moved is the mint, not the read, so `Fingerprint₀` is still
        // `precondition(snapshot(now))` and still the value T-10a will compare against at commit.
        let pre = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        let fp0 = adapter.precondition(&pre).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;

        // 🔴 The CAS `req/182` H-15 found missing (`req/38` §132 ruling 2, 43 §5.2 row
        // `world-moved`). The comparison is on the **digest** alone rather than through
        // `Fingerprint::cas_eq`, and that is forced rather than chosen: 42 §3.10 stores a
        // `postcondition_fingerprint` as `FingerprintBytes` — 32 bytes with no substrate and no
        // scope — so the two other components `cas_eq` insists on are simply not in the receipt.
        // It is the same value space the settle pre-flight already polls against
        // (`Engine::live_digest`), which is why an in-process substrate that matches on poll 1 also
        // passes here with zero behavioural difference.
        //
        // 🔴 **R3 (`req/38` §160 ruling 2, `req/222` H-01/H-02)** — the `match` is exhaustive and
        // the third arm is a **refusal**. Until R3 this was `if let Attested(..)`, which made every
        // other answer a silent skip: `rm .gx/receipts/<TID>.commit.json` turned a
        // `409 PRECONDITION_CHANGED` into a `200` that overwrote a third party's write, and
        // `archive_commit_receipt`'s `let _ =` reached the same state with no attacker at all
        // (measured 3/3). [`UndoWitness::Unobservable`] keeps the old behaviour and now means only
        // what `req/38` §123 ruling 1 ruled it means: **the adapter** cannot read a position (a
        // tools-only MCP server), or another row of [`UNDO_REFUSALS`] has already fixed the answer.
        match witness {
            UndoWitness::Attested(expected) => {
                if fp0.digest().0 != expected.0 {
                    return Err(UndoRefusal::WorldMoved {
                        expected: *expected,
                        found: FingerprintBytes(fp0.digest().0),
                        scope: fp0.scope().to_string(),
                    }
                    .into_error(original));
                }
            }
            UndoWitness::Missing(missing) => {
                return Err(UndoRefusal::WitnessMissing { missing: *missing }.into_error(original))
            }
            UndoWitness::Unobservable(_) => {}
        }

        // T-1. The intent exists for its identity: 42 §1.3 row 2 puts all five fields in the CID,
        // so two undos of one commit mint one `IntentId` and the second is answered without a
        // second record (43 T-1's create-if-absent, unchanged).
        //
        // 🔴 **Lane R2** — the five fields are assembled in [`Engine::undo_intent`] and nowhere
        // else. `req/182` H-16 measured what having only this spelling cost: the intent lived for
        // the length of the call, no draft was written, and `gx undo <T_u>` therefore had nothing
        // to rebuild `T_u`'s row from (`req/216`'s exit 6). The archive's writer is the caller —
        // gx-cli's `lifecycle::undo` and gx-api's `undo_transformation` — and a caller that has to
        // *recompute* what to file would be a second definition of what an undo's intent is. Every
        // refusal above this line has already run, so the `None` arm is unreachable by
        // construction; it is answered rather than `expect`ed because 41 §6 counts a panic as a
        // bug.
        let intent = self.undo_intent(original)?.ok_or_else(|| Error::NotFound {
            what: "the escrowed inverse an undo's intent is built from",
            id: format!("{original:?}"),
        })?;
        let intent_id = IntentId(cid::compute(&intent)?);
        if !self.drafted.contains_key(&intent_id) {
            self.journal_append(EngineJournalRecord::DraftCreated {
                intent_id,
                rng_seed,
                at,
            })?;
            self.drafted.insert(intent_id, rng_seed);
        }

        // 🔴 **DR-46-33 / DR-46-28** — the undo's input-generation join, computed before `actor`
        // moves into the metadata below and journalled as its result for the plan road's reason
        // (`store.rs`'s `Planned.input_generation` doc).
        let input_generation = self.joined_input_generation(&substrate, &actor);
        let mut transformation = Transformation::new(
            TransformationId(Cid([0u8; 32])),
            0,
            subject,
            None,
            // 43 T-12's guard: "`T_u.parents` includes `T_o.id`" (sem: SEM-gx-engine-241). This
            // is where that becomes true, and it is also C-2's provenance edge: the undo names
            // what it undoes.
            vec![*original],
            CompositionMetadata {
                intent_id,
                delta: inverse.reference().clone(),
                context,
                actor,
                created_at: at,
            },
        )?;
        let id = TransformationId(cid::compute(&transformation)?);
        transformation.id = id;

        // 🔴 H-05 (`req/182` §1-1, repaired in `req/189`): `T_u`'s id is a CID over the
        // IdentityView, and `created_at` is outside it (42 §1.3-2, ASM-4) — so a second
        // `undo(T_o)` mints **the same** `T_u.id` as the first. Without this guard the second call
        // appended a second `Planned` and re-seated the row as a fresh `Candidate`, and a `T_u`
        // the gate had already denied was silently rewound: the live table said `Candidate` while
        // Σ (`reconstruct`) still said `Denied` — AC-039's bit-equality broken by one HTTP retry
        // (`handlers.rs` lets an undo through whenever `inverse_status == Available`, and a
        // denied undo leaves it `Available`). `plan()` has had the same guard since M5
        // ("a re-plan is allowed only while the row is still where T-2 left it"); this is that
        // rule on the one other road that mints a `TransformationId`.
        //
        // In-process: the row is in the table. Cross-process (a restarted engine has an empty
        // table): the journal is asked, exactly as `plan()` asks it. A `T_u` still `Candidate` is
        // answered with its own id and **no second record** (43 T-2's idempotency column, and
        // what a re-`plan` of the same intent already does); a `T_u` that **aborted** may be
        // planned again (§98 ruling 2's `--retry` road, see the arm below); anything further
        // along is refused.
        let in_table = self.table.contains_key(&id);
        let existing_state = self.table.get(&id).map(|e| e.state).or_else(|| {
            // 🔴 **DEFECT-891-1** (`req/895` §2) — membership, not equality. This is the
            // expression that killed the branch: two undos sharing an `IntentId` differ in
            // `parents`, so the second's id is never the one an equality test found here.
            if self
                .resolved
                .get(&intent_id)
                .is_some_and(|ids| ids.contains(&id))
            {
                reconstruct(self.journal.records())
                    .state_of(&id)
                    .and_then(|row| row.state)
            } else {
                None
            }
        });
        match existing_state {
            None => {}
            Some(Lifecycle::Candidate) if in_table => {
                // 🔴 **DR-46-47 (open, `req/973` §8-6, filed 2026-08-31)** — this return is before
                // the `Planned` append, so a second `undo` of the same `T_o` does not re-journal
                // its witness: the receipt names the **first** call's disposition, not the last.
                // Both calls did run the CAS (the `match` on `witness` is above this line), so no
                // undo can claim `Attested` without having compared -- the direction is
                // fail-closed. What it can do is keep saying `Unobservable` after a later call
                // compared successfully, i.e. under-claim. Owner: the next cargo lane on gx-engine.
                // Release condition: either the second call updates the seat, or `undo` refuses a
                // re-plan whose witness differs from the journalled one, with a probe that drives
                // both orders (Attested-then-Unobservable and its reverse) and asserts the receipt.
                return Ok((intent_id, id));
            }
            Some(Lifecycle::Candidate) => {
                // Journalled as `Candidate`, absent from the table (restart): fall through and
                // re-seat — Σ already holds a `Planned` for this id, and a second one leaves Σ's
                // last-write-wins reconstruction where it was (`replay.rs`), which is what makes
                // this the one arm where a second record is harmless rather than a rewind.
            }
            Some(Lifecycle::Aborted(_)) => {
                // 🔴 The one **verdicted-or-later** state a second undo may re-plan over, and it is
                // a ruling rather than a gap: `req/38` §98 ruling 2's `--retry` (the D-complement)
                // is exactly "undo again after `Aborted(ApplyFailed)`", and 44 §1.2's `gx undo
                // --retry` documents each attempt as "its own T_u honestly in the journal". An
                // aborted T_u applied nothing and holds no verdict a re-plan could forget — the
                // rewind H-05 forbids is of a row the gate has *answered about* (Denied /
                // Admitted / Escalated / Canonicalized …) or that is in or past the critical
                // section. `crates/gx-cli/tests/undo_settle.rs::retry_refires_on_apply_failed…`
                // measures this arm; the arms above and below measure the refusal.
            }
            Some(state) => {
                return Err(UndoRefusal::AlreadyPlanned {
                    state: state.name(),
                }
                .into_error(&id));
            }
        }

        self.journal_append(EngineJournalRecord::Planned {
            transformation: id,
            intent_id,
            locator: pre.locator().to_string(),
            delta_cid: inverse_cid,
            fp0: FingerprintRecord::of(&fp0),
            // 🔴 **E-M5-13**'s reason, on the one path that has one: T-12's guard is
            // "`T_u.parents` includes `T_o.id`" (sem: SEM-gx-engine-242), and this is where the list stops being in-memory
            // only. A crash between here and `Committed` used to lose the edge (M5H6-6's window,
            // which the `M5H6_6` probe measured); now the journal carries it.
            parents: transformation.parents.clone(),
            input_generation,
            // 🔴 **DR-46-45 (`req/973` §B-1)** — the answer the CAS above gave, written down here
            // because T-11 is where it has to be readable and this call does not reach T-11. Every
            // road that reaches this line has already passed the `match` on `witness`, so the
            // `Missing` arm — a refusal — cannot be recorded: what is journalled is exactly the two
            // outcomes a committed undo can have.
            undo_witness: witness.disposition(),
            at,
        })?;
        self.remember_resolution(intent_id, id);
        // **M4H6-3** on the live path for the second time: the body is already filed under this
        // CID, so the store answers `AlreadyPresent` and writes nothing. "storage happens only
        // once" (sem: SEM-gx-engine-243) is what
        // makes an undo's delta the *same* blob as the commit's escrowed inverse rather than a copy.
        self.blobs.put(&inverse)?;

        self.seat(
            id,
            Entry {
                intent_id,
                transformation,
                state: Lifecycle::Candidate,
                since: at,
                blocked_by: None,
                delta: inverse,
                fp0,
                pre,
                verdict: None,
                verdict_digest: None,
                enforced: true,
                fail_posture_engaged: false,
                canonical_cid: None,
                ticket: None,
                admit_proof: None,
                deny_reasons: None,
                verdict_receipts: Vec::new(),
                superseded_by: None,
                apply_started: None,
                observation_cid: None,
                rollback: None,
                provenance: None,
                inverse_cid: None,
                applied_at: None,
                receipt: None,
            },
        );
        Ok((intent_id, id))
    }

    /// **T-12, second half** — draw the supersede edge, once, when an inverse reaches `Committed`.
    ///
    /// 43 T-12: from `Committed(T_o)`, trigger "another Transformation `T_u` reaches `Committed`,
    /// and `T_u.delta == T_o`'s escrowed inverse", guard "`T_u.parents` includes `T_o.id`, and
    /// `T_u`'s `Subject` matches `T_o`'s", side effect "append `superseded_by = T_u.id` to `T_o`'s
    /// metadata (journal: `Superseded{T_o.id, by: T_u.id}`). **`T_o`'s canonical record and receipt
    /// stay unchanged**" (sem: SEM-gx-engine-244).
    ///
    /// Every clause is a line below, and the matching is **M5-09 adopted (a)**'s: "T-12's matching
    /// is the equality of escrow's `inverse_delta` CID and `T_u.delta` CID" (sem: SEM-gx-engine-244).
    /// Three facts move together (M5-16 adopted (a): "one place" (sem: SEM-gx-engine-244)) —
    /// `T_o`'s state, the [`SupersedeIndex`] entry, and 42 §3.12's `InverseStatus` — and
    /// this is the only place any of them is written.
    ///
    /// # What is deliberately *not* written
    ///
    /// `T_o`'s canonical record, its receipt and its ledger entry. INV-S2 and P-5 ("an undo is a
    /// new commit, not a rewrite" (sem: SEM-gx-engine-245)) are the whole of AC-044, and the way to satisfy them is to touch none of
    /// the three — which is why the row's `receipt` and `transformation` fields do not appear here.
    ///
    /// # Errors
    /// [`Error::Io`] from the journal.
    fn supersede_after_commit(
        &mut self,
        t_u: &TransformationId,
        at: Timestamp,
    ) -> Result<Option<TransformationId>> {
        let Some(entry) = self.table.get(t_u) else {
            return Ok(None);
        };
        let delta_cid = entry.delta.reference().cid;
        let subject = entry.transformation.subject;
        let parents = entry.transformation.parents.clone();

        let mut found = None;
        for parent in parents {
            let Some(row) = self.escrow.get(&parent) else {
                continue;
            };
            if row.inverse_cid != Some(delta_cid) || !matches!(row.status, InverseStatus::Available)
            {
                continue;
            }
            let Some(original) = self.table.get(&parent) else {
                continue;
            };
            if original.state != Lifecycle::Committed
                || original.transformation.subject != subject
                // 43 T-12's idempotency column: "if `superseded_by` is already set, do not set it
                // again" (sem: SEM-gx-engine-246).
                || self.supersedes.superseded_by(&parent).is_some()
            {
                continue;
            }
            found = Some(parent);
            break;
        }
        let Some(t_o) = found else {
            return Ok(None);
        };

        // Journal-first, then the three facts.
        self.journal_append(EngineJournalRecord::Superseded {
            transformation: t_o,
            by: *t_u,
            at,
        })?;
        self.supersedes.record(t_o, *t_u);
        if let Some(row) = self.escrow.get_mut(&t_o) {
            row.status = InverseStatus::Consumed { by: *t_u };
        }
        if let Some(entry) = self.table.get_mut(&t_o) {
            entry.superseded_by = Some(*t_u);
        }
        self.set_state(&t_o, Lifecycle::Superseded, at);
        Ok(Some(t_o))
    }

    // -----------------------------------------------------------------------
    // T-8, T-8r
    // -----------------------------------------------------------------------

    /// **T-8 / T-8r** `canonicalize` — check T3, fix the canonical CID, record `enforced`.
    ///
    /// 43 T-8's side effects are "fix the canonical CID; confirm `canon(canon(x))=canon(x)` (T3)"
    /// and its from-state is `Admitted`. T-8r adds `Denied` under `EnforcementMode::RecordOnly`,
    /// with "carve the `enforced=false` flag into the Transformation's attached metadata"
    /// (sem: SEM-gx-engine-247).
    ///
    /// The idempotence check runs **before** anything is written, which is what AC-033's abnormal
    /// case asks for: "return an error and do not transition to Canonicalized"
    /// (sem: SEM-gx-engine-248). A refusal leaves the state where it
    /// was and the journal untouched, so a caller can look at the transformation afterwards and a
    /// replay never sees a canonicalisation that did not happen.
    ///
    /// # 🔴 `enforced = Some(false)` is reachable from T-8 as well as T-8r
    ///
    /// 42 §3.13 annotates the record "only T-8r carries enforced=Some(false)"
    /// (sem: SEM-gx-engine-249). That is narrower than 43 §4,
    /// which degrades a **T-4e** transformation to "record-only-mode equivalent" while leaving it
    /// `Admitted` -- so it reaches canonicalisation through T-8, carrying `enforced=false`. Writing
    /// `None` there to satisfy 42's parenthetical would hide exactly the fact INV-S5 requires to be
    /// visible ("a Committed with `enforced=false` is ... carved into the receipt in a
    /// distinguishable form" (sem: SEM-gx-engine-250)). The flag follows
    /// the transformation, not the transition. Raised as **M5H2-3**.
    ///
    /// # 🔴 `mode` is **E-M6-20**'s argument, and it is the shape M6-08 already ruled
    ///
    /// `None` means "whatever this engine was opened with" (sem: SEM-gx-engine-251) and
    /// `Some(..)` overrides it **for this
    /// call**. 44 §2.2's commit body grew a `record_only` field under E-M6-20 (req/38 §52, "the HTTP
    /// version of E-M6-10; making the [DR-2 sensitivity] paragraph executable"
    /// (sem: SEM-gx-engine-251)), and a long-lived server cannot express it any other
    /// way: [`Engine::with_mode`] is a builder that consumes `self` at `open` time, and the
    /// alternative — "`serve` swaps `mode` via `&mut self` per request" (sem: SEM-gx-engine-251)
    /// — is the form §47
    /// M6-08 ruled "**must not be adopted**" because a posture written onto shared state leaks into the
    /// next request, and a leaked `RecordOnly` is a fail-open. [`Engine::verify`] took the same
    /// argument for the same reason one hand earlier; this is its other half, because DR-2's
    /// "whether to apply even on Deny" (sem: SEM-gx-engine-251) is decided at **T-8r**, not at T-4.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown id. [`Error::InvalidState`] for any state that is not
    /// `Admitted`, and for `Denied` when the effective mode is `Enforce` (where 43 §1 makes `Denied`
    /// terminal). [`Error::NotIdempotent`] when the canonical form is not a fixed point.
    /// [`Error::Canon`] if the transformation has no canonical form. [`Error::Io`] from the journal.
    pub fn canonicalize(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        mode: Option<EnforcementMode>,
    ) -> Result<Lifecycle> {
        // 43 T-6 reaches `Candidate`/`Verifying`/`Escalated` and not `Admitted`/`Denied`, so this
        // call fires nothing today. It is here because M5-10 adopted (a) is "evaluate the TTL
        // whenever the state advances" (sem: SEM-gx-engine-252)
        // and an entry point that skipped the evaluation would be relying on the from-state list
        // never widening.
        self.expire_if_due(id, at)?;
        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;

        // 43 T-8's idempotency column: "canonicalize is idempotent (T3); recomputing gives the same
        // canon_cid" (sem: SEM-gx-engine-253). The
        // honest form of "the same" is "the same one, not recomputed" -- an early return, so a second
        // call writes no second `Canonicalized` record. Same shape as T-1's create-if-absent, and
        // for the same reason: a journal that grew on every re-entry would report re-entries as
        // events, which is right for `VerifyStarted` (a real second attempt) and wrong here (a
        // caller asking again for a value that is already fixed).
        if entry.state == Lifecycle::Canonicalized {
            return Ok(Lifecycle::Canonicalized);
        }

        let record_only = mode.unwrap_or(self.mode) == EnforcementMode::RecordOnly;
        match entry.state {
            Lifecycle::Admitted => {}
            Lifecycle::Denied if record_only => {}
            state => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: state.name(),
                    attempted: "canonicalize",
                })
            }
        }

        let bytes = self.canon.canonical_form(&entry.transformation)?;
        if !cbor::is_canonical(&bytes) {
            return Err(Error::NotIdempotent {
                transformation: *id,
                detail: format!(
                    "canon produced {} bytes that gx-canon would not have written, so \
                     canon(canon(x)) != canon(x) (42 §2.3, 12 F0 T3)",
                    bytes.len()
                ),
            });
        }

        // The identity is gx-canon's, always, whatever the canonicalizer above is (41 §6).
        let canonical_cid = cid::compute(&entry.transformation)?;

        // T-8r's flag, and T-4e's -- see the section above.
        let enforced = if entry.state == Lifecycle::Denied {
            Some(false)
        } else if entry.enforced {
            None
        } else {
            Some(false)
        };

        self.journal_append(EngineJournalRecord::Canonicalized {
            transformation: *id,
            canonical_cid,
            enforced,
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.canonical_cid = Some(canonical_cid);
            if enforced == Some(false) {
                entry.enforced = false;
            }
        }
        Ok(self.set_state(id, Lifecycle::Canonicalized, at))
    }

    // -----------------------------------------------------------------------
    // T-9, T-10a, T-10b, T-10c, T-11 -- the commit critical section
    // -----------------------------------------------------------------------

    /// **T-9 → T-10a/T-10b/T-10c → T-11** `commit` — the critical section 43 §1 calls `Committing`.
    ///
    /// One entry point for five transitions, for the reason [`Engine::verify`] is one for six: 43
    /// gives them one trigger, and the branches are what two collaborators answered. The order of
    /// the statements below **is** 41 §5's protocol, and every one of them is journalled before the
    /// side effect it describes.
    ///
    /// | step | 43 | what it does |
    /// |---|---|---|
    /// | T-9 | `commit_start` | `CommittingStarted`, then the state moves. "**always journal before the side effect runs**" (sem: SEM-gx-engine-254) |
    /// | — | **M5-25 adopted (a)** | `ProvenanceDerived` — before the world moves, so a crash cannot lose it |
    /// | T-10a | CAS | `Fingerprint₁ := adapter.precondition(now)`, compared with `Fingerprint₀` |
    /// | T-10b | escrow | `adapter.invert`, `InverseEscrowed`, then the body into the blob store |
    /// | — | **E-M5-1** | `ApplyStarted`, then the **one** call to `adapter.apply` |
    /// | T-10c | apply failed | best-effort rollback, then `Aborted(ApplyFailed)` with what happened |
    /// | T-11 | apply succeeded | `ledger.append` → `InclusionProof` → receipt → `Committed` |
    ///
    /// # 🔴 The CAS has three answers and only two of them are transitions (**M5-24 adopted (a)**)
    /// (sem: SEM-gx-engine-255)
    ///
    /// `Fingerprint::cas_eq` returns `Result<bool>` because 42 §3.5's comparison has three answers,
    /// and **E-M4-15 / E-M4-27** made the third one an `Err`: two fingerprints from different
    /// adapters, or over different scopes, cannot be compared at all. §37 rules where it goes:
    ///
    /// > **M5-24, adopted (a)**: `cas_eq`'s `Err` is `Aborted(InternalError)` (an exact match to 43
    /// > T-13's wording; the first road to walking T-13 = closes together with M5-14). A one-line
    /// > doc obligation. (sem: SEM-gx-engine-256)
    ///
    /// **Here is that line**: an `Err` from the CAS is `Aborted(InternalError)` and never
    /// `PreconditionChanged`. The difference is the whole value of the `Result`: `PreconditionChanged`
    /// says "someone else moved the world" and `InternalError` says "this deployment is wired
    /// wrong" (sem: SEM-gx-engine-257), and folding the second into the first would retire a bug as a business condition —
    /// the same mistake **E-M4-32** refused for `Ok(None)`. It is also the **first road to T-13**,
    /// which 51 §14's branch coverage needs (M5-14).
    ///
    /// # 🔴 Journal-first has exactly one exception, and 43 §7-3b is the price paid for it
    ///
    /// Every record above is written before its side effect. `Committed` is not, and cannot be: it
    /// carries `ledger_seq`, which does not exist until `ledger.append` has answered. 43 T-11's own
    /// cell writes the journal last for that reason, and §7-3b is the recovery that exists because
    /// of it — "if the corresponding entry exists in the ledger, the commit had already completed
    /// before the crash; only the journal's `Committed` entry is missing" (sem: SEM-gx-engine-258).
    /// The exception and its compensation are one design,
    /// and naming it here is what keeps a later hand from "fixing" the ordering.
    ///
    /// # What is refused rather than invented
    ///
    /// A T-4e degraded admission has **no verdict** — the gate was never asked — and 42 §3.10's
    /// `ReceiptPayload.verdict` has no way to say so. The engine refuses with
    /// [`Error::Unrepresentable`] **before** T-9 opens, so nothing is journalled for a commit that
    /// cannot be completed. Raised as **M5H4-3**.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown id or an unregistered adapter. [`Error::InvalidState`]
    /// from any state that is not `Canonicalized`. [`Error::Unrepresentable`] for the T-4e case
    /// above. [`Error::Adapter`] if `snapshot`, `precondition` or `invert` refuses — note that a
    /// refusal from **`apply`** is not an error here, it is T-10c and comes back in the `Ok`.
    /// [`Error::Ledger`] and [`Error::Witness`] from T-11's two collaborators. [`Error::Io`] from
    /// the journal or the blob store.
    pub fn commit(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Lifecycle> {
        self.expire_if_due(id, at)?;
        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;

        // 43 T-9's idempotency column: "a duplicate `commit_start` request is ignored if already
        // `Committing`" (sem: SEM-gx-engine-259), and 43 §1
        // makes `Committed` terminal. Both are answered without writing anything, which is what
        // "ignore" means in a journal: a second request that appended a second `CommittingStarted`
        // would report a re-entry as an event. Resuming an interrupted critical section is 43
        // §7-3's recovery and hand 5's; in this hand a `Committing` row means the call above is
        // still on the stack.
        match entry.state {
            Lifecycle::Committed => return Ok(Lifecycle::Committed),
            Lifecycle::Committing => return Ok(Lifecycle::Committing),
            Lifecycle::Canonicalized => {}
            state => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: state.name(),
                    attempted: "commit",
                })
            }
        }

        // Everything the receipt needs, resolved before the section opens (see the note above).
        let canonical_cid = entry.canonical_cid.ok_or_else(|| Error::InvalidState {
            id: format!("{id:?}"),
            state: entry.state.name(),
            attempted: "commit",
        })?;
        // 🔴 **E-M5-11**. Hand 4 refused every degraded admission here, because 42 §3.10 required a
        // `VerdictSummary` and 43 T-4e has none; §41 made the seat an `Option` and the refusal
        // moved rather than vanishing. What is refused now is the shape that would be **untrue**:
        // no verdict and no reason for there being none. `Error::Unrepresentable` keeps a producer,
        // and it is the honest one — a commit with neither a verdict nor an engaged fail-open
        // posture is a receipt that says a change was allowed and cannot say by what.
        let verdict = match (entry.verdict, entry.verdict_digest) {
            (Some(kind), Some(proof_digest)) => Some(VerdictSummary { kind, proof_digest }),
            (None, None) if entry.fail_posture_engaged => None,
            _ => {
                return Err(Error::Unrepresentable {
                    what: "a CommitReceipt with no verdict and no engaged fail-open posture",
                    detail: format!(
                        "{id:?} has verdict={:?} digest={:?} fail_posture_engaged={}; 43 T-4e is \
                         the one road to a commit without a verdict and it sets the flag, so this \
                         row is a half-filled pair rather than a degraded admission",
                        entry.verdict, entry.verdict_digest, entry.fail_posture_engaged
                    ),
                })
            }
        };

        let adapter = self
            .adapters
            .get(entry.delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", entry.delta.substrate()),
            })?
            .adapter
            .clone();
        let delta = entry.delta.clone();
        let pre = entry.pre.clone();
        let fp0 = entry.fp0.clone();
        let locator = pre.locator().to_string();

        // --- T-9, journal-first ------------------------------------------------------------
        self.journal
            .append(EngineJournalRecord::CommittingStarted {
                transformation: *id,
                at,
            })?;
        self.set_state(id, Lifecycle::Committing, at);

        // --- M5-25 adopted (a): the provenance, before anything can be lost (sem: SEM-gx-engine-260) ---
        let provenance = self.derive_provenance(id, &pre);
        self.journal
            .append(EngineJournalRecord::ProvenanceDerived {
                transformation: *id,
                provenance: provenance.clone(),
                at,
            })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.provenance = Some(provenance);
        }

        // --- T-10a: CAS ----------------------------------------------------------------------
        // 43 §7's "`Fingerprint₁ := adapter.precondition(now)`" (sem: SEM-gx-engine-261). Two
        // calls, because 41 §4's
        // `precondition` takes a snapshot rather than a locator: "now" is a **fresh** snapshot,
        // and reusing T-2's would make the comparison a value against itself.
        let fresh = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        let fp1 = adapter.precondition(&fresh).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;
        match fp0.cas_eq(&fp1) {
            // INV-S7: "under no circumstances is `adapter.apply` called" (sem: SEM-gx-engine-262).
            // The return is the enforcement --
            // there is no path from here to the call below.
            Ok(false) => return self.abort(id, AbortReason::PreconditionChanged, None, at),
            // M5-24 adopted (a). See the section above.
            Err(_) => return self.abort(id, AbortReason::InternalError, None, at),
            Ok(true) => {}
        }

        // --- T-10b: escrow the inverse if it can be constructed, before the world moves --- (sem: SEM-gx-engine-263) ---
        //
        // 🔴 **DR-46-26** — three values come back from this one call now, and all three were
        // already computed inside the adapter before this window: the inverse, **what the escrow
        // read** to build it, and **C-25's three-valued verdict**. `req/38` §198 ruling (b) named
        // this line as the funnel that flattened the last two away.
        let outcome = adapter.invert(&delta, &pre).map_err(|e| Error::Adapter {
            action: "invert",
            detail: e.to_string(),
        })?;
        let verdict_c25 = outcome.verdict();
        let reads = outcome.reads().to_vec();
        let inverse = outcome.into_inverse();
        // 🔴 **E-M5-9**. Both arms journal, and that is the whole erratum: 43 T-10b's guard is
        // "the inverse can be constructed (`Some`)" (sem: SEM-gx-engine-263) and hand 4 wrote
        // nothing at all when the answer was `None`,
        // because 42 §3.13 typed the record's CID as required. The `None` arm is **reachable now**
        // — E-M3-4 escalates a transformation whose `invert` answers `None`, and T-5 is what lets a
        // person approve one — so "we asked and there is none" has to be a record. Without it a
        // replay would find no escrow row for the commit and report the undo guarantee as
        // "never asked" (sem: SEM-gx-engine-264), which is §32 M4H4-2's refusal in the log rather than in a type.
        let escrowed = match inverse {
            Some(inverse) => {
                let inverse_cid = inverse.reference().cid;
                // 🔴 Two-phase escrow (`req/38` §98 ruling 1) (sem: SEM-gx-engine-265): a registered completion is asked
                // whether this escrow is partial. Unregistered = `false` = the pre-existing road,
                // untouched. An `Err` here is pre-apply, so failing the commit closed is honest.
                let pending = match self.completions.get(delta.substrate()) {
                    Some(completion) => {
                        completion
                            .needs_completion(&inverse)
                            .map_err(|e| Error::Adapter {
                                action: "needs_completion",
                                detail: e.to_string(),
                            })?
                    }
                    None => false,
                };
                self.journal_append(EngineJournalRecord::InverseEscrowed {
                    transformation: *id,
                    inverse_cid: Some(inverse_cid),
                    pending,
                    // 🔴 **DR-46-26** — journalled here so 43 §7-3b's rebuild can reach it.
                    reads: reads.clone(),
                    // 🔴 **DR-46-34** — and that the list above is a **reading** rather than a gap.
                    // The adapter answered, so an empty `reads` here is the fact "this escrow read
                    // nothing" and not the fact "this journal does not record reads".
                    reads_attested: true,
                    // An inverse was constructed, so C-25 answered `True` and the negative
                    // discriminator is not in play.
                    undetermined: false,
                    at,
                })?;
                // 43 T-10b: "if already escrowed, do not write again (idempotent)"
                // (sem: SEM-gx-engine-266) -- which the store answers
                // by content addressing rather than by a flag (`PutOutcome::AlreadyPresent`).
                self.blobs.put(&inverse)?;
                self.escrow.insert(
                    *id,
                    EscrowRow {
                        transformation: *id,
                        inverse_cid: Some(inverse_cid),
                        // DR-9: the OSS default is unbounded (sem: SEM-gx-engine-267), and the
                        // journal has no seat for a deadline anyway (M5H3-3).
                        retained_until: None,
                        status: if pending {
                            InverseStatus::Pending
                        } else {
                            InverseStatus::Available
                        },
                    },
                );
                if let Some(entry) = self.table.get_mut(id) {
                    entry.inverse_cid = Some(inverse_cid);
                }
                Some((inverse_cid, inverse, pending))
            }
            None => {
                self.journal_append(EngineJournalRecord::InverseEscrowed {
                    transformation: *id,
                    inverse_cid: None,
                    pending: false,
                    // 🔴 **DR-46-26** — a commit with no inverse still read, and the read is still
                    // what a receipt attests. "gx looked and then found no restore to build" is a
                    // different fact from "gx never looked".
                    reads: reads.clone(),
                    // 🔴 **DR-46-34** — as above. The adapter was asked on this arm too, so the
                    // list is a reading; "gx looked and read nothing" is what an empty one means.
                    reads_attested: true,
                    // 🔴 And **which** absence this is, so that a replay can tell E-M5-9's
                    // `Unavailable` from DR-46-13's `Undetermined`. Without it the seventh word
                    // lives only in this process's memory and a restart reports the sixth.
                    undetermined: matches!(verdict_c25, Reversibility::Unknown),
                    at,
                })?;
                self.escrow.insert(
                    *id,
                    EscrowRow {
                        transformation: *id,
                        inverse_cid: None,
                        retained_until: None,
                        // 42 §3.12: "the case where `invert()` returns None (cannot be
                        // constructed)" (sem: SEM-gx-engine-268).
                        //
                        // 🔴 **DR-46-26 / DR-46-13 — the writer `InverseStatus::Undetermined` was
                        // waiting for.** D24 seated the seventh word and its own documentation
                        // named the block by coordinate: "`Reversibility` does not cross the crate
                        // boundary ... giving this word a writer means widening
                        // `SubstrateAdapter::invert`". It is widened, so the two facts that used to
                        // arrive here as one `None` are two again. `Unknown` is "the prior would
                        // not be read and this deployment declared `OnReadFailure::Unknown`", which
                        // is a property of a **posture** an operator chose and can unchoose;
                        // `False` is a property of the **change**, and no remedy an operator has
                        // will alter it.
                        status: match verdict_c25 {
                            Reversibility::Unknown => InverseStatus::Undetermined,
                            // `True` is unreachable in this arm -- the constructor of
                            // `InvertOutcome` binds `True` to a `Some` inverse and this is the
                            // `None` arm -- so it folds with `False` rather than getting an arm
                            // that no road can enter. `tests/lifecycle_transitions.rs` holds the
                            // binding itself.
                            Reversibility::False | Reversibility::True => {
                                InverseStatus::Unavailable
                            }
                        },
                    },
                );
                None
            }
        };

        // --- 🔴 R9 / `req/236` H-01: the escrowed body, read back, before the world moves --------
        //
        // 43 T-10b's guard is "the inverse can be constructed"; the promise an operator hears is
        // "this change can be taken back". Between the two there is a **body**, and `req/236` H-01
        // measured a commit that had the first two and not the third: the blob store held a
        // fragment at the inverse's content address, the escrow row said `Available`, the receipt
        // was signed over that CID, and the undo failed for ever.
        //
        // `BlobStore::put` no longer leaves such a body (tmp + rename, and `AlreadyPresent` now
        // compares bytes), so this is the check that says so rather than assumes so. It is placed
        // **before** `ApplyStarted` for E-M5-1's reason: a refusal after the announcement would
        // make the recovery re-apply a delta over a world nobody was told about. A commit that
        // cannot escrow a readable inverse fails closed, with the reason in the sentence, and the
        // world has not moved.
        if let Some((inverse_cid, _, _)) = &escrowed {
            if !self.blobs.holds_body(inverse_cid) {
                return Err(Error::Malformed {
                    detail: format!(
                        "the inverse escrowed for this commit ({}) is not readable back out of the \
                         blob store, so committing would promise an undo this project cannot \
                         perform. Nothing has been applied (req/236 H-01, 43 T-10b)",
                        gx_canon::cid::to_text(inverse_cid)
                    ),
                });
            }
        }

        // --- 🔴 T-10a′ (R8 / `req/234` H-04): the CAS, again, with nothing left in between ------
        //
        // 43 §7's TOCTOU paragraph says the mismatch "always ends in `Aborted(PreconditionChanged)`
        // and is never silently applied over a stale premise", and until R8 that sentence had no
        // time in it. `req/234` H-04 put time in it: T-10a is checked, then `invert` runs, then the
        // inverse is escrowed, blobbed and journalled, and only then does the world move — and a
        // third party who wrote **inside** that gap had their bytes overwritten by an `rc=0`
        // commit carrying a signed receipt that said nothing about it. Measured on this machine at
        // ~21 ms of a ~108 ms commit before the repair below.
        //
        // 🔴 What this is and what it is not. It is a **second read of the same fingerprint**,
        // placed after every step that can take time and immediately before the announcement that
        // precedes the one call that changes the world. It is **not** atomicity: the remaining gap
        // is the `ApplyStarted` append plus the adapter's own syscalls up to its `rename`, and that
        // gap is measured and declared (43 §7's layered note, `docs/LIMITS.md`) rather than argued
        // away. Closing it entirely needs the compare and the write in one syscall, which is an
        // adapter contract change (41 §4 fixes `apply`'s signature at one argument) and is raised
        // rather than made here.
        //
        // Position: **before** the `ApplyStarted` record, not after. E-M5-1's whole purpose is that
        // the record means "the adapter was asked", and a refusal recorded after an announcement
        // would make the recovery re-apply a delta nobody applied.
        let guard = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        let fp1b = adapter.precondition(&guard).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;
        match fp0.cas_eq(&fp1b) {
            Ok(false) => return self.abort(id, AbortReason::PreconditionChanged, None, at),
            Err(_) => return self.abort(id, AbortReason::InternalError, None, at),
            Ok(true) => {}
        }

        // --- E-M5-1, then the one call that changes the world ---------------------------------
        self.journal_append(EngineJournalRecord::ApplyStarted {
            transformation: *id,
            delta_cid: delta.reference().cid,
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.apply_started = Some(delta.reference().cid);
        }
        let applied = match self.apply_once(adapter.as_ref(), &delta) {
            Ok(applied) => applied,
            // --- T-10c ---------------------------------------------------------------------
            Err(_) => {
                let rollback = match &escrowed {
                    // 🔴 A `Pending` escrow is not yet an executable inverse — its do-time
                    // member is unresolved, and with the apply failed there is no observation to
                    // resolve it from. Sending the partial to the server could at best be refused
                    // and at worst land a call with a member missing, so the honest answer is 43
                    // T-10c's guard read strictly: no constructible inverse, no attempt.
                    Some((_, _, true)) => {
                        // 🔴 **`req/324` §5(d)** — the cause travels with the value. This is the
                        // one road the proxy's sentence used to describe on all three.
                        self.not_attempted_because
                            .insert(*id, NotAttemptedBecause::EscrowStillPartial);
                        Rollback::NotAttempted
                    }
                    // 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the two reads that
                    // decide whether the compensation is sent **at all**, before a single byte of
                    // it reaches the substrate.
                    //
                    // # The two reads, and why one would not do
                    //
                    // The first is taken the instant the forward `apply` answers `Err`, and it
                    // asks *what did our own apply leave behind*. The second is taken immediately
                    // before the compensation, and it asks *is the world still where our apply
                    // left it*. Between them the answer to "may this inverse be sent" is complete:
                    //
                    // | first read | second read | what is done |
                    // |---|---|---|
                    // | at `fp0` | — | **nothing is sent.** The apply moved nothing, so there is nothing to take back |
                    // | moved | still there | the compensation runs, as it always has |
                    // | moved | moved again | **nothing is sent.** Somebody wrote after our apply and before our compensation |
                    // | unreadable | — | **nothing is sent.** An absolute inverse is not fired into a substrate that will not say where it is |
                    //
                    // 🔴 **A single read compared with `fp0` cannot do this, and the first draft of
                    // this repair proved it by breaking.** *Somebody else wrote* and *our own
                    // apply landed* are the same observation — the object is not at `fp0` — so a
                    // guard on `fp0` alone has to choose which one to serve, and either choice is
                    // a defect: refuse, and the ordinary case this whole road exists for (the call
                    // landed and then errored) silently stops being compensated, which
                    // `r29_rollback_is_verified.rs`'s negative control caught in one run; permit,
                    // and the third party's write is still erased. The information the two answers
                    // differ by is **time**, not fingerprints, so the fix is a second read rather
                    // than a cleverer comparison.
                    //
                    // # The defect
                    //
                    // The escrowed inverses the shipped adapters mint are **absolute** — the
                    // twenty-ninth audit drove all four and found no shipped payload whose effect
                    // is a function of the state it starts from. An absolute inverse restores
                    // from *any* world, and "any world" includes one a third party legitimately
                    // created. The audit measured a colleague's commit going off a real branch
                    // with `Succeeded` printed over it (`A29_GIT_THIRD_PARTY theirs=d2d09b5
                    // their_commit_is_still_the_tip=false`). The word was true about `fp0` and
                    // said nothing about whose work `fp0` was standing on.
                    //
                    // The same failure is already **measured and repaired** one road over:
                    // `docs/LIMITS.md` v0.4-o says of `gx undo` that "before this, the escrowed
                    // inverse was written over the top and the other change was gone with no
                    // message (measured: `req/182` H-15, on a file and on a git branch)". The
                    // repair went into the road a **person** starts and never reached the road
                    // **nobody** starts — and this one has no operator standing over it to stop
                    // it. This line is that repair's sibling, which is what `req/38` §227's sweep
                    // asks for: one question, every site that asks it.
                    //
                    // # Why the reads come before the `ApplyStarted`
                    //
                    // Same reason the forward side's R8 CAS does (see the block above E-M5-1): the
                    // record means *the adapter was asked*, and announcing a compensation that is
                    // then refused would make recovery re-apply a delta nobody applied. The cost
                    // is paid where it is visible rather than hidden: the journal append sits
                    // **inside** the residual window, exactly as it does on the forward side, so
                    // the adapter-level number `docs/LIMITS.md` v0.5-q publishes is a lower bound
                    // and says so.
                    //
                    // # 🔴 What this deliberately does **not** close
                    //
                    // It is a compare-and-set spelled as two calls, so the window between the read
                    // and the `apply` is real and a third party who writes inside it is still
                    // overwritten. That is R8's residue exactly, in R8's own words — "it is
                    // **not** atomicity" — and `docs/LIMITS.md` v0.5-q carries its **measured**
                    // width on both fs and mcp rather than an adjective. It is also not
                    // attribution: one fingerprint cannot tell a third party's write from this
                    // transformation's own forward `apply` having landed half of itself. Both
                    // land on `WorldMovedBeneath`, and that fold is argued in the cause's own doc
                    // — it is favourable in both directions, because the alternative for the
                    // half-applied world was an absolute inverse landing on top of it and
                    // carrying the object **further** from home (`req/361` H-01's `A B D C D`).
                    // 🔴 The read is taken **once**. A guard that asked and an arm that asked
                    // again would be two windows where the design declares one, and the second
                    // answer could differ from the first — so the answer is computed here and
                    // carried into the branch.
                    Some((inverse_cid, inverse, false)) => {
                        match self.world_the_failed_apply_left(adapter.as_ref(), &locator, &fp0) {
                            // 🔴 The cause is spelled **at** the construction, not carried into it in a
                            // variable (`req/324` §5(d), and `r26_not_attempted_causes.rs` is the gate
                            // that keeps it that way). Three roads, three facts, three words: "the
                            // apply left nothing behind", "somebody wrote after it did" and "I could
                            // not look" are different, and a reader told one of them when another
                            // happened has been handed a confident account of an observation nobody
                            // made.
                            CompensationVerdict::Unreadable => {
                                self.not_attempted_because
                                    .insert(*id, NotAttemptedBecause::WorldCouldNotBeRead);
                                Rollback::NotAttempted
                            }
                            // 🔴 **The audit's worst shape, closed at its root.** `req/372` §2's fourth
                            // rebuttal: when the forward `apply` fails **without moving the world**
                            // there is no effect to compensate, and the engine sent the escrowed
                            // inverse anyway — so a transformation that did nothing erased a third
                            // party's write and nothing else. It is not sent any more, and the reason
                            // is not a guess about who else is out there: it is a **measurement of our
                            // own apply**, taken the instant it failed and before anyone else can have
                            // reacted to it. Nothing is lost by declining, because an absolute inverse
                            // over a world at `fp0` is a no-op — and a **relative** inverse over one
                            // is worse than a no-op (`{remove C, remove D}` against a world that never
                            // received `C` or `D`), so this is the only correct answer for both
                            // grammars rather than a trade.
                            CompensationVerdict::NothingToTakeBack => {
                                self.not_attempted_because
                                    .insert(*id, NotAttemptedBecause::WorldNeverMoved);
                                Rollback::NotAttempted
                            }
                            // The forward apply did move the world, so there **is** something to take
                            // back. `left_at` is where it left it, and it — not `fp0` — is what the
                            // compensation is guarded on. Guarding on `fp0` here would refuse every
                            // legitimate roll-back this road exists for, which is the trap the first
                            // draft of this repair fell into and `r29_rollback_is_verified.rs`'s
                            // negative control caught.
                            CompensationVerdict::TakeBackFrom(left_at) => match self
                                .world_is_still_at(adapter.as_ref(), &locator, &left_at)
                            {
                                None => {
                                    self.not_attempted_because
                                        .insert(*id, NotAttemptedBecause::WorldCouldNotBeRead);
                                    Rollback::NotAttempted
                                }
                                Some(false) => {
                                    self.not_attempted_because
                                        .insert(*id, NotAttemptedBecause::WorldMovedBeneath);
                                    Rollback::NotAttempted
                                }
                                Some(true) => {
                                    // The rollback **is** an apply, so it gets the record every apply gets: a
                                    // crash inside it must not look like a crash before it. This is also what
                                    // keeps Rule 2 honest (sem: SEM-gx-engine-269) -- one call site, reached twice.
                                    self.journal_append(EngineJournalRecord::ApplyStarted {
                                        transformation: *id,
                                        delta_cid: *inverse_cid,
                                        at,
                                    })?;
                                    if let Some(entry) = self.table.get_mut(id) {
                                        entry.apply_started = Some(*inverse_cid);
                                    }
                                    // 43 T-10c: "best-effort; proceed regardless of outcome"
                                    // (sem: SEM-gx-engine-270) -- the outcome is
                                    // recorded and does not change the reason.
                                    //
                                    // 🔴 **R29 / `req/361` H-01** — `Ok` used to be the whole answer here, and
                                    // the twenty-eighth audit showed what that word was worth. A
                                    // contract-conforming adapter whose `apply` fails halfway (its own
                                    // `ApplyFailed` doc says so in as many words: "a non-atomic `apply` can
                                    // fail halfway") left the object at `A B D` after the forward apply, the
                                    // escrowed inverse `{add: C, D}` was then applied **completely and
                                    // honestly**, and the world came to rest at `A B D C D`. The adapter's
                                    // `Ok` was true. `Succeeded` was not.
                                    //
                                    // The asymmetry that made it possible is the one this line closes: the
                                    // **forward** apply has had a second read of the same fingerprint in front
                                    // of it since R8 / `req/234` H-04, and the roll-back's apply had nothing —
                                    // the engine wrote to a world it had just been told it could not reason
                                    // about, and then reported on that world without looking at it. So the
                                    // roll-back now gets the forward side's shape, reflected: the object is
                                    // read again and compared with the `fp0` this transformation started from.
                                    match self.apply_once(adapter.as_ref(), inverse) {
                                        Ok(_) => {
                                            self.rollback_landed(adapter.as_ref(), &locator, &fp0)
                                        }
                                        Err(_) => Rollback::Failed,
                                    }
                                }
                            },
                        }
                    }
                    None => {
                        // 🔴 **`req/324` §5(d)** — a different fact: there is no escrow row to be
                        // partial. `invert` answered `None`, which the comment above `NotAttempted`
                        // in `store.rs` records as reachable through E-M3-4 and T-5.
                        self.not_attempted_because
                            .insert(*id, NotAttemptedBecause::NoInverseWasEscrowed);
                        Rollback::NotAttempted
                    }
                };
                return self.abort(id, AbortReason::ApplyFailed, Some(rollback), at);
            }
        };

        // 🔴 **E-M4-31 / M5-18 adopted (a)**: the moment is the engine's. `AppliedDelta` has four
        // accessors and no setter, so the value is **rebuilt** rather than mutated -- which is the
        // ruling's own form ("not one line added to gx-substrate; one place on the engine side"
        // (sem: SEM-gx-engine-271)). req/78 §4 M5-18
        // writes the call as `AppliedDelta::new(*d.delta(), ...)`; `DeltaRef` is not `Copy` (it
        // holds a `SubstrateKind`, which holds a `String`), so it is cloned.
        // The observation is read off the adapter's answer **before** E-M4-31's rebuild (the
        // rebuild is deliberately a four-field re-mint and would drop the seat).
        let observation = applied.observation().map(<[u8]>::to_vec);
        let applied = AppliedDelta::new(
            applied.delta().clone(),
            applied.postcondition().clone(),
            *applied.resulting_digest(),
            at,
        );

        // --- Two-phase escrow: observe, then complete, inside the same critical section --------
        // (`req/38` §98 ruling 1's five-step mechanism, steps 2-3; §99 ruling 2-④ for the fold.)
        // (sem: SEM-gx-engine-272)
        let final_inverse = match &escrowed {
            Some((_, partial, true)) => {
                let observed = match observation.as_deref() {
                    Some(bytes) if bytes.len() as u64 <= crate::store::MAX_OBSERVATION_BYTES => {
                        self.record_observation(id, bytes, at)?;
                        Some(bytes)
                    }
                    // Over the ceiling, or no answer was captured (a retry's re-entry): nothing
                    // an `ApplyObserved` record could honestly name, so the fold below says what
                    // happened instead.
                    _ => None,
                };
                self.settle_pending_escrow(id, Some(partial), observed, at)?
            }
            Some((cid, _, false)) => Some(*cid),
            None => None,
        };

        // --- 🔴 M5-11 / blocker item 5: the prophecy, checked ------------------------------------
        //
        // **M5H2-2 adopted (b)** (`req/919` A1) gave `Transformation.target` a producer, and this
        // is the refusal `req/38` §37 filed and deferred: "how should the engine refuse when
        // plan's prediction and apply's measurement disagree". Until this window the doc on
        // `Engine::plan` named the *absence* of this check, and the honest reason it named was
        // that there was no prediction to check.
        //
        // **The guard is the `Option`, and it is the whole compatibility story.** `target` is
        // `None` for every adapter that fills no `promised_target`, which is all six shipped ones,
        // so the `let Some` below does not open and this block costs one `Option` discriminant
        // read on the road every existing commit takes.
        //
        // **Why here.** It is as early as the comparison can be — a post-state digest exists only
        // after `apply` — and as late as it must be to leave the undo material intact: the
        // two-phase escrow above has just completed the inverse, so a mispredicted world is one
        // an operator can still act on. Nothing has been signed yet; T-11's payload is built
        // below, and a transformation that leaves here never reaches it.
        //
        // **What is deliberately not done.** No compensation is sent. The engine has just
        // measured that its model of this object's post-state is wrong, and 43 T-10c's road sends
        // an escrowed inverse on the strength of exactly that model; acting on a model this abort
        // exists to distrust is what fail-closed forbids. `Rollback::NotAttempted` is therefore
        // the true word, and it travels without a `NotAttemptedBecause`: that vocabulary's five
        // causes are all "the inverse was unavailable or unsafe to send", and this one is "the
        // inverse is available and the engine declines". Naming a sixth cause is a wire change of
        // its own (`req/38` §231 ruling 5's one-arm-per-cause gate, measured by
        // `crates/gx-cli/tests/r26_not_attempted_causes.rs`) and is raised in `req/919` A1's
        // report rather than made here.
        //
        // 🔴 **Superseded in part — R-1001-1 (`req/1001` §4, the else-arm of D-999-F2,
        // 2026-08-31).** The paragraph above is kept as the record of the window in which the
        // value travelled bare; the deferral it names has since been ruled. The cause now travels:
        // `NotAttemptedBecause::PromisedPostStateWasWrong` names exactly the fact the paragraph
        // spells out ("the inverse is available and the engine declines"), the r26 gate it cites
        // measures the line below, and everything else the paragraph says — no compensation, the
        // escrow completed first, fail-closed on a distrusted model — is unchanged.
        if let Some(promised) = self.table[id].transformation.target {
            let observed = *applied.resulting_digest();
            if promised != observed {
                self.not_attempted_because
                    .insert(*id, NotAttemptedBecause::PromisedPostStateWasWrong);
                return self.abort(
                    id,
                    AbortReason::PostconditionMismatch,
                    Some(Rollback::NotAttempted),
                    at,
                );
            }
        }

        // --- T-11 ------------------------------------------------------------------------------
        // 🔴 **DR-46-28** — read before `verdict` moves into the literal below.
        let verdict_present = verdict.is_some();
        let mut payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict,
            enforced: self.table[id].enforced,
            // 🔴 **`req/493` §0 / AC-6** — the same value that went into the journal at
            // `derive_provenance` a few hundred lines up, from the same field, so that the rebuild
            // roads reading it back out of Σ reproduce these bytes rather than approximate them.
            confinement: Some(self.confinement.clone()),
            catalogue_hash: None,
            // F7 / R-868-6 (`req/919` W5): every receipt this build issues is `Some`, on both
            // kinds and with no kind-dependent rule -- see the field's own doc comment.
            payload_version: Some(CURRENT_PAYLOAD_VERSION),
            // 🔴 **A2 (`req/910`, `req/919` W8)** — the same value `derive_provenance` journalled a
            // few hundred lines up, from the same constant, so the rebuild roads reading it back
            // out of Σ reproduce these bytes rather than approximate them. Exactly `confinement`'s
            // argument, on the field that answers `#435`.
            engine_version: Some(crate::VERSION.to_string()),
            receipt_kind: ReceiptKind::CommitReceipt,
            canonical_cid,
            // Two-phase escrow: the receipt names the **completed** inverse (`req/160` §1-1
            // step 4 — the seal finishes before the receipt issues), or `None` beside its
            // Admit when the completion folded — the failure's visible fingerprint.
            inverse_delta: final_inverse,
            transformation: *id,
            // Filled below. `ReceiptPayload::ledger_digest` clears it in any case -- see there for
            // the circularity in 42 §3.11 that makes the leaf a digest of the payload without its
            // own proof.
            inclusion_proof: None,
            fail_posture_engaged: self.table[id].fail_posture_engaged,
            // 🔴 **DR-46-26** — the producer D24 declared missing, arriving. `reads` is what
            // `SubstrateAdapter::invert` reported at T-10b above, and **the granularity is chosen
            // here and only here**: `ReadSet::from_reads` sorts, de-duplicates, answers `None` for
            // an escrow that read nothing, and spills G3 → G4 at `READ_SET_SPILL_THRESHOLD`. The
            // adapter reported objects and did not pick a variant (`req/441` §4), so the tag on a
            // receipt stays a function of the number of distinct objects.
            // 🔴 **DR-46-34** — and it answers `ReadSet::Nothing` rather than `None` for an escrow
            // that read nothing, so this road's absence is a **fact** and not the four-way `null`
            // `req/472` §6 measured. The `Some` is unconditional here for that reason: every
            // `CommitReceipt` this binary issues carries one of the spellings.
            read_set: Some(ReadSet::from_reads(reads).map_err(|e| Error::Witness {
                action: "build the read-set",
                detail: e.to_string(),
            })?),
            // 🔴 **DR-46-26 / DR-46-13** — and C-25's answer beside it, which is the half of
            // `req/38` §198 ruling (b) that the escrow row alone does not close: a reader who holds
            // the receipt and nothing else saw `inverse_delta: null` for both "there is no undo"
            // and "nobody found out".
            reversibility: Some(verdict_c25),
            // 🔴 **DR-46-45 (`req/973` §B-1/§B-2)** — read back out of the `Planned` record, not
            // taken from this process's table, and for `determinism_boundary`'s reason one field
            // down rather than by analogy with it: the two rebuild roads must reproduce these bytes
            // from Σ alone (43 §7-3b), so the live road reads the same seat they will.
            undo: self.journalled_undo(id),
            // 🔴 **DR-46-28 / DR-46-33** — the verdict stage is derived here; the input-generation
            // stage is read back from the `Planned` record (`journalled_input_generation`), which
            // is where the plan-time join was journalled precisely so the two rebuild roads below
            // reproduce this boundary without reaching the actor (not in Σ) or the catalogue.
            determinism_boundary: attested_boundary(
                self.journalled_input_generation(id),
                verdict_present,
            ),
            fingerprint_scope: fp0.scope().to_string(),
            precondition_fingerprint: FingerprintBytes(fp0.digest().0),
            postcondition_fingerprint: Some(FingerprintBytes(applied.postcondition().digest().0)),
        };
        let receipt_digest = payload.ledger_digest().map_err(|e| Error::Witness {
            action: "digest the receipt payload",
            detail: e.to_string(),
        })?;
        let outcome = self
            .ledger
            .append(*id, receipt_digest, at)
            .map_err(|e| Error::Ledger {
                action: "append",
                detail: e.to_string(),
            })?;
        let leaf = outcome.entry().index;
        let proof: InclusionProof =
            prove_inclusion(self.ledger.log(), leaf).map_err(|e| Error::Ledger {
                action: "prove inclusion",
                detail: e.to_string(),
            })?;
        payload.inclusion_proof = Some(proof);
        let receipt = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
            action: "issue the receipt",
            detail: e.to_string(),
        })?;

        // 🔴 **R8 / `req/234` H-01 — the receipt is durable before the row is terminal.**
        //
        // The order of this section is now journal → ledger → **receipt archive** → `Committed`
        // record → head, and the position of this one line is the whole repair. `req/234` measured
        // the previous order end to end: the caller filed the receipt *after* `Engine::commit`
        // returned, so a power cut between the `Committed` record and that `write(2)` left a row
        // that was terminal in Σ and had no receipt anywhere — permanently, because a terminal row
        // is exactly the row 43 §7-3b's recovery does **not** finish.
        //
        // Placing the archive write **before** the `Committed` record moves every crash in this
        // section back inside a window the recovery already closes: the leaf is on the ledger, no
        // `Committed` record follows it, so `Engine::resume` walks §7-3b, rebuilds the payload,
        // checks it against the digest the ledger witnessed, re-issues the receipt — and files it
        // here, on this same line, one function down.
        //
        // A failure here therefore fails the commit (`req/38` §154). The world has moved and the
        // ledger holds the leaf, and both of those are true and recorded; what is *not* true is
        // that the commit finished, and saying so is the difference between this and the `500` the
        // HTTP face used to answer over a row that could never be undone.
        self.file_receipt(id, &receipt)?;

        self.journal_append(EngineJournalRecord::Committed {
            transformation: *id,
            ledger_seq: leaf,
            at,
        })?;
        self.committed.insert(*id, leaf);
        // 🔴 **R6 / DR-43-11** — the project records where it has reached, **after** both files are
        // durable and before this call answers.
        //
        // The order is the argument. A head written before the `Committed` record would attest a
        // tree the journal does not yet witness; a head written after this function returns would
        // leave a window in which a crash loses the statement and the next start-up has a lower
        // floor than the one this commit earned. Losing a head is safe (the floor only
        // under-detects), attesting ahead of the files is not — so it goes last among the writes
        // and inside the call.
        self.record_head(at, key)?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.applied_at = Some(applied.applied_at());
            entry.receipt = Some(receipt);
        }
        let state = self.set_state(id, Lifecycle::Committed, at);
        // 🔴 **T-12**, from the other side: this commit may be the inverse of an earlier one. The
        // edge is drawn **after** `Committed` because 43 T-12's trigger is "another Transformation
        // `T_u` reaches `Committed`" (sem: SEM-gx-engine-273) -- a transformation that has not
        // committed supersedes nothing. A
        // crash in between leaves `T_u` committed and `T_o` still `Committed`, which is the window
        // discipline 48's intermediate probe stops in (`tests/supersede.rs`).
        self.supersede_after_commit(id, at)?;
        Ok(state)
    }

    /// 🔴 **43 §7's recovery**, run after a restart (AC-043, 51 §8.1).
    ///
    /// 43 §7 writes it as three numbered steps and this function is those steps:
    ///
    /// 1. **§7-1** — "replay the `EngineJournal` ... in order to the end, and for each
    ///    `TransformationId` reconstruct the last transition recorded" (sem: SEM-gx-engine-274).
    ///    [`Engine::open`] read the file; [`crate::replay::reconstruct`]
    ///    turns those records into Σ, and Σ's rows *are* "the last transition recorded".
    /// 2. **§7-2** — a terminal last record is rebuilt and nothing is re-run. For `Committed` that
    ///    means restoring Σ's ledger component (the frontier [`Engine::ledger_agrees`] compares), and
    ///    for the aborts it means nothing at all.
    /// 3. **§7-3** — a `CommittingStarted` with no terminal record after it is a crash **inside the
    ///    critical section**, and the exactly-once judgement runs.
    ///
    /// # 🔴 The judgement has two questions, not one, and the second is E-M5-1's
    ///
    /// 43 §7-3 asks the ledger and branches on the answer. Under **E-M5-1** the engine asks a second
    /// question first — "was the adapter asked to apply" (sem: SEM-gx-engine-275) — because req/78 §3.2 Λ4 is a three-line
    /// proof that the one-question version breaks:
    ///
    /// > Suppose the crash happens "after apply succeeded, before `ledger.append`" ... recovery
    /// > step 3c recomputes `Fingerprint₁ := adapter.precondition(now)` ... but **apply has already
    /// > succeeded, so `Fingerprint₁ ≠ Fingerprint₀`** ... the substrate has changed; the
    /// > Transformation is Aborted ... = "it was applied, but nothing recorded it"
    /// > (sem: SEM-gx-engine-276)
    ///
    /// The recovery **never re-runs the CAS**. Where an `ApplyStarted` exists the comparison would
    /// be against the engine's own footprint, and where it does not exist there is nothing to
    /// compare *for* — the world did not move, so no apply may follow. "does not mistake its own
    /// partial apply for outside interference" (sem: SEM-gx-engine-277) is therefore structural
    /// here: `cas_eq` is not called in this function, and
    /// `tests/crash_recovery.rs` measures both halves (a source scan, and the shim of Λ4's mistaken
    /// recovery run against the same bytes).
    ///
    /// # What 43 §7-3c cannot do in v0.1, and it is an input rather than an intention
    ///
    /// §7-3c says "re-run the T-10a-onward procedure ... **from the beginning**"
    /// (sem: SEM-gx-engine-278), and T-10a needs
    /// `adapter.precondition(now)` — which needs a **fresh snapshot**, which needs a **locator**.
    /// The journal does not hold one. It holds `Fingerprint₀`, whose `scope` is 42 §3.5's "an
    /// adapter-defined scope identifier" and is explicitly allowed to be **wider** than the object
    /// ("may cover a range wider than a single `ObjectSnapshot.digest`" (sem: SEM-gx-engine-278)), so reading a locator out of it would
    /// be the engine parsing an adapter's grammar. ∴ a `Committing` row with **no** `ApplyStarted`
    /// and **no** ledger entry is folded to `Aborted(InternalError)`: nothing was applied (so P-4
    /// and INV-S4 hold), and INV-L3 forbids leaving it `Committing`. Raised as **M5H5-2**.
    ///
    /// # 🔴 Both roads re-apply, and 43 §7-3b did not expect to
    ///
    /// §7-3b reads as though the ledger entry alone finishes the job: "re-issue the receipt from
    /// the existing `InclusionProof` (if not yet issued)" (sem: SEM-gx-engine-279). Re-issuing
    /// needs the **payload**, and 42 §3.10's payload
    /// carries `postcondition_fingerprint` — a value produced by `apply` and recorded **nowhere**.
    /// The journal holds `Fingerprint₀` (in `Planned`) and no fingerprint of the result. So the
    /// recovery obtains it the only way v0.1 can: by applying again, which 41 §4 contracts to be
    /// idempotent and which 43 §7-3c already relies on. The cost is one write to a world that had
    /// already reached that state, and the alternative — a journal record carrying the postcondition
    /// — is a change to 42 §3.13 that this hand raises rather than makes (**M5H5-3**).
    ///
    /// # What recovery needs, measured (**M5H3-5**)
    ///
    /// The four inputs are the journal, the blob store, the ledger and a registered adapter — plus
    /// the signing key, which is why this is a call and not part of [`Engine::open`]: 41 §6 injects
    /// the adapter and the key **after** the engine exists, and an `open` that recovered would have
    /// to recover before either arrived. What it does **not** need is the state table:
    /// `Transformation` bodies, `ObjectSnapshot`s and in-memory `PlannedDelta`s are all absent after
    /// a restart and none of them is read here. That is M5H3-5's measurement, and it is why `open`
    /// still rebuilds only the draft phase. Raised as **M5H5-1**.
    ///
    /// # 🔴 What a recovery still does not do: T-12 (hand 6)
    ///
    /// A crash between `T_u`'s `Committed` record and its `Superseded` record leaves the supersede
    /// edge undrawn, and this function does not draw it. It cannot check 43 T-12's guard: "`T_u.
    /// parents` includes `T_o.id`" (sem: SEM-gx-engine-280) is a fact about the `Transformation`
    /// body, and the journal holds
    /// names and digests rather than bodies (ASM-9) -- the state table a recovery works from has no
    /// `parents`. Firing T-12 on the escrow CID match **alone** would be dropping half the guard,
    /// which is the sort of shortcut §32 M4H4-2 keeps refusing. So the window is left open,
    /// measured (`tests/supersede.rs`), and raised as **M5H6-6**.
    ///
    /// # Errors
    /// [`Error::NotFound`] when no adapter is registered for a delta's substrate.
    /// [`Error::Unrepresentable`] for a `Committing` row with no verdict and no engaged fail-open
    /// posture. [`Error::Io`] from the journal or the blob store, [`Error::Ledger`] and
    /// [`Error::Witness`] from T-11's two collaborators.
    pub fn recover(&mut self, at: Timestamp, key: &KeyPair) -> Result<Vec<Recovered>> {
        // 🔴 **R36 / `req/476` H-01** — cleared here so that what a caller reads after an `Err` is
        // this call's own work and never a previous one's.
        self.recover_partial = RecoverPartial::default();
        let sigma = reconstruct(self.journal.records());
        // 🔴 **R5 / `req/227` H-01 - the gate that stands in front of the substrate.**
        //
        // This function is the one road on which gx writes to somebody's world **without being
        // asked**: it runs at start-up, before a request exists, and 43 §7-3c re-applies. `req/227`
        // measured what that costs when its input is not trustworthy - one `Committed` record in a
        // three-commit journal was overwritten with the bytes of another `Committed` record from
        // the same file, and the next start-up read the row as an unfinished commit, asked the
        // adapter again, and took the operator's file from `three` back to `one`. `/healthz`
        // answered `200` throughout and `gx repair` called the project healthy.
        //
        // DR-43-9's chain is what makes that rewrite visible; this is the second half, and it is
        // deliberately not the same mechanism: a recovery that refuses whenever the pair does not
        // support each other is a recovery that cannot be steered by a file, whatever the file
        // says. `journal_intact` is `false` for a broken chain, a shortened file and a rewritten
        // tail, and - on a [`crate::replay::JournalFormat::Legacy`] journal, which has no chain to
        // break - the second gate inside `resume` is the whole of the protection.
        //
        // 🔴 **R6 / DR-43-11 / `req/229` H-01** — and the same refusal for a project that has gone
        // **backwards**, which `journal_intact` cannot see. A pair truncated at two frame
        // boundaries is internally perfect: the chain over the shorter journal verifies, the ledger
        // over the shorter tree verifies, and the two agree. What they do not agree with is the
        // signed head this project already published. Without this arm the recovery reads the last
        // surviving commit as an unfinished one — the cut falls between `ledger.append` and the
        // `Committed` record on purpose, so gate ② below is *satisfied* — and re-applies its delta.
        // Measured: `three` → `two`, with `recover.refused: 0` on the start-up line.
        // 🔴 **R7 / `req/232` H-01/M-07** — and a third condition with the same standing: the head
        // in front of us is not a document this binary will read numbers off. A recovery is the one
        // road on which gx writes to somebody's world unasked, so "the detector was replaced" is
        // not a state to run it in.
        if self.journal_departure.is_some()
            || self.rolled_back.is_some()
            || self.head_invalid.is_some()
        {
            // 🔴 **R32 / `req/392` M-02** — one sentence per departure, from the same value the
            // faces print. Until this lane every one of the seven landed on `JOURNAL_MOVED`,
            // whose words are about bytes that were **rewritten**: a project told this after its
            // marker was stripped, or after meeting a journal from a newer `gx`, was told a cause
            // nobody had measured.
            let why = if let Some(departure) = self.journal_departure {
                not_resumed::journal_departed(departure)
            } else if self.rolled_back.is_some() {
                // 🔴 **R8 / `req/234` L-03** — the declaration and the files are two facts.
                if self.declaration_changed {
                    not_resumed::DECLARATION_CHANGED
                } else {
                    not_resumed::ROLLED_BACK
                }
            } else {
                not_resumed::HEAD_INVALID
            };
            return Ok(sigma
                .transformations()
                .iter()
                .filter(|row| {
                    matches!(
                        row.state,
                        Some(Lifecycle::Committed) | Some(Lifecycle::Committing)
                    )
                })
                .map(|row| Recovered {
                    transformation: row.transformation,
                    path: RecoveryPath::NotResumed,
                    state: row.state.unwrap_or(Lifecycle::Committing),
                    ledger_seq: None,
                    appended: None,
                    payload_matched: None,
                    receipt: None,
                    refusal: Some(why),
                })
                .collect());
        }
        // The whole row, not just a CID: a resume must see `Pending` (two-phase escrow's crash
        // window) to complete or fold it, and `Unavailable` must stay `None` on the receipt.
        let escrowed: BTreeMap<TransformationId, EscrowRow> = sigma
            .escrow()
            .iter()
            .map(|e| (e.transformation, *e))
            .collect();
        let committed: BTreeMap<TransformationId, u64> = sigma
            .ledger()
            .iter()
            .map(|c| (c.transformation, c.ledger_seq))
            .collect();
        let rows: Vec<StateRow> = sigma.transformations().to_vec();

        let mut out = Vec::new();
        for row in rows {
            match row.state {
                // §7-2, and the one terminal that leaves a trace in Σ.
                Some(Lifecycle::Committed) => {
                    let seq = committed.get(&row.transformation).copied();
                    if let Some(seq) = seq {
                        self.committed.insert(row.transformation, seq);
                    }
                    out.push(Recovered {
                        transformation: row.transformation,
                        path: RecoveryPath::Terminal,
                        state: Lifecycle::Committed,
                        ledger_seq: seq,
                        appended: None,
                        payload_matched: None,
                        receipt: None,
                        refusal: None,
                    });
                }
                // §7-3.
                //
                // 🔴 **R36 / `req/476` H-01** — this was `out.push(self.resume(..)?)`, and the `?`
                // is where four verbs' silence came from: the rows already recovered went with the
                // error, and the row that had just written its delta was not among them because it
                // never became a `Recovered`. Both are kept now, and the `Err` still propagates
                // unchanged — this repair adds telling and takes nothing away.
                Some(Lifecycle::Committing) => {
                    match self.resume(&row, escrowed.get(&row.transformation).copied(), at, key) {
                        Ok(recovered) => out.push(recovered),
                        Err(why) => {
                            self.recover_partial.finished = out;
                            return Err(why);
                        }
                    }
                }
                // Every other last record is either terminal with nothing to restore (§7-2) or an
                // in-flight state outside the critical section, which 43 §7 does not resume.
                _ => {}
            }
        }
        Ok(out)
    }

    /// 🔴 **R36 / `req/476` H-01** — what the last [`Engine::recover`] had already done when it
    /// raised.
    ///
    /// Meaningful only after that call answered `Err`: it is cleared at the top of every `recover`,
    /// so a caller that reads it after an `Ok` reads an empty pair, which is the truth for one.
    ///
    /// The two members are not interchangeable and a caller that prints only the first is still
    /// silent about the thing that matters. See [`RecoverPartial`].
    #[must_use]
    pub fn recovery_before_error(&self) -> &RecoverPartial {
        &self.recover_partial
    }

    /// 🔴 The half of [`Engine::recovery_before_error`] a face must never drop: the rows whose
    /// delta reached the substrate and whose commit was not recorded.
    ///
    /// Named separately because `req/476` H-01 is precisely about a fact that existed in the
    /// process and reached no mouth. A caller that has this and says nothing is making a choice
    /// rather than missing a field.
    #[must_use]
    pub fn applied_unrecorded(&self) -> &[TransformationId] {
        &self.recover_partial.applied_unrecorded
    }

    /// 🔴 **R37 / `req/496` M-01** — the rows whose terminal record landed and whose head did not.
    ///
    /// Named separately for [`Engine::applied_unrecorded`]'s reason: a face that has this and says
    /// nothing is choosing to, and the two lists are answers to different questions. A row is in
    /// **exactly one** of them at any moment — `journal_append(Committed)` is the line between —
    /// so a caller printing both is not printing one row twice.
    #[must_use]
    pub fn recorded_without_head(&self) -> &[TransformationId] {
        &self.recover_partial.recorded_without_head
    }

    /// 43 §7-3 for one transformation. See [`Engine::recover`] for the two questions it asks.
    fn resume(
        &mut self,
        row: &StateRow,
        escrow_row: Option<EscrowRow>,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Recovered> {
        let id = row.transformation;
        let refused = |state: Lifecycle| Recovered {
            transformation: id,
            path: RecoveryPath::NothingWasApplied,
            state,
            ledger_seq: None,
            appended: None,
            payload_matched: None,
            receipt: None,
            refusal: None,
        };

        // 43 §7-3a: "query the `ledger` by `TransformationId` ..." (sem: SEM-gx-engine-281).
        let held = self
            .ledger
            .log()
            .entries()
            .iter()
            .find(|e| e.transformation == id)
            .cloned();

        // The two questions. Neither road below may be walked when both answers are "no"
        // (sem: SEM-gx-engine-282): the
        // adapter was never asked and the ledger holds nothing, so the world is as `plan` left it.
        if held.is_none() && row.apply_started.is_none() {
            let state = self.abort(&id, AbortReason::InternalError, None, at)?;
            return Ok(refused(state));
        }

        // 🔴 **R5 / `req/227` H-01 - is this row's commit still the last thing the ledger saw?**
        //
        // 43 §7-3b's window is a crash **between** `ledger.append` and the `Committed` record, and
        // a crash there leaves the interrupted commit's leaf as the newest leaf in the tree: the
        // process died, so nothing came after it. A row that reaches here with **later leaves
        // behind it** is therefore not in that window at all. Something else removed its
        // `Committed` record, and the section it appears to be inside closed long enough ago for
        // other commits to follow.
        //
        // The difference matters because the road below **re-applies** - 43 §7-3b did not expect
        // to, and the reason it does is written above (42 §3.10 needs a `postcondition_fingerprint`
        // the journal has no seat for). Re-applying a delta from before two later commits is not an
        // idempotent repeat of the last thing that happened; it is an old write landing on a newer
        // world, and `req/227` measured it as `three` -> `one` on an operator's file.
        //
        // So the row is left exactly as it was found. The frontier is **not** restored either: a
        // recovery that quietly made `ledger_agrees` true would hand a start-up a project whose
        // journal is missing a record nobody has been told about, which is how this accident stayed
        // invisible for a whole restart in the first place.
        if let Some(entry) = &held {
            if entry.index + 1 < self.ledger.log().len() {
                return Ok(Recovered {
                    transformation: id,
                    path: RecoveryPath::NotResumed,
                    state: row.state.unwrap_or(Lifecycle::Committing),
                    ledger_seq: Some(entry.index),
                    appended: None,
                    payload_matched: None,
                    receipt: None,
                    refusal: Some(not_resumed::LEDGER_MOVED_ON),
                });
            }
        }

        // 🔴 **R13 / `req/244` H-03** — 43 §7-3b, closed off the document the section already
        // filed, before anything is re-applied.
        //
        // # What the audit measured
        //
        // A `gx wrap` commit killed inside the window between `ledger.append` and the `Committed`
        // record leaves the leaf on the ledger and the record missing. That is the window §7-3b
        // exists for, and it is 21 ms wide on this machine — `strace` puts `ledger.append`'s fsync
        // at `.060164` and the `Committed` record's at `.081285`, with the commit receipt becoming
        // durable at `.074708` in between. The recovery below closes it by re-applying the delta,
        // because 42 §3.10's `postcondition_fingerprint` is a reading of the world and no journal
        // record carries one. But the process that could perform that reading is the one that had
        // the MCP server as a child, and it is dead: `gx repair --yes` has no server,
        // `adapter.apply` refuses, and until R13 that refusal was answered with
        // `Aborted(ApplyFailed)` — a **terminal** record, which is exactly the record 43 §7-2 makes
        // a recovery stop at. The row was unclosable by anybody afterwards, every writer verb
        // answered `LEDGER_DISAGREES`, and `gx repair`'s remedy said the two files were from
        // different projects. 6 of 40 runs in a 128–164 ms sweep.
        //
        // # Why the receipt is the right document, and why this is not Model B
        //
        // Nothing here is invented and nothing is re-derived. The ledger has already witnessed a
        // digest for this row, and the receipt R8 moved **inside** the critical section is the
        // document that digests to it — signed, filed, and answered `valid: true`,
        // `inclusion: "verified"` by `gx receipt verify` throughout the audit's sweep. The
        // comparison below is the same one §7-3b makes (the leaf's `receipt_digest` against a
        // payload's `ledger_digest`); what changes is where the payload comes from. So the record
        // this writes is a statement the project's own two witnesses already agree on, which is the
        // line between finishing a commit and composing one.
        //
        // A mismatch is not closed. The document exists, the ledger witnessed something else, and
        // "provable but not closable" is the honest answer: the row falls through to the road
        // below, which re-applies where it can and refuses without a terminal record where it
        // cannot.
        if let Some(entry) = &held {
            if let Some(receipt) = self
                .receipt_sink
                .as_ref()
                .and_then(|sink| sink.filed_receipt(&id))
            {
                // 🔴 Two questions, and the second one is the whole gate.
                //
                // The document has to be *this row's* — the sink is keyed by id, and the payload
                // says so itself — and it has to digest to the leaf the ledger already witnessed.
                // The digest is what makes a wrong document impossible rather than unlikely: a
                // verdict receipt, a receipt from another row, or a receipt rebuilt under another
                // key all fail it, because `receipt_kind`, `transformation` and `key_id` are
                // fields of the payload the digest is taken over. That is the same comparison 43
                // §7-3b makes below; what differs is where the payload came from.
                //
                // The kind is therefore **not** compared as a separate condition. It would be a
                // third spelling of a fact the digest already carries — and, said plainly, this
                // file's own `ReceiptKind::CommitReceipt` occurrences are counted by
                // `crates/gx-engine/tests/commit_protocol.rs` as the number of roads that *issue*
                // one. A read that borrowed the literal would move that count and make the probe
                // report four issuers where there are three.
                //
                // 🔴 **`req/38` §324 ruling 3** — the digest comes from `Receipt::ledger_digest`,
                // which reads the **signed bytes**, and not from the decoded payload. This road
                // meets documents the engine filed under an earlier release: re-encoding what they
                // decode into asks what *this* build's schema would have written, so a member added
                // since would make every such recovery disagree with a leaf nobody had touched.
                let agrees = receipt.payload().ok().is_some_and(|payload| {
                    payload.transformation == id
                        && receipt.ledger_digest().ok() == Some(entry.receipt_digest)
                });
                if agrees {
                    self.journal_append(EngineJournalRecord::Committed {
                        transformation: id,
                        ledger_seq: entry.index,
                        at,
                    })?;
                    self.committed.insert(id, entry.index);
                    // Counted before the head is written, for `req/232` M-01's reason: a project
                    // with no floor does not get its first one minted by a road that has just
                    // written to somebody's world — and this road has written to nobody's.
                    self.resumed_rows += 1;
                    // 🔴 **R37 / `req/496` M-01** — the terminal record is on the disk from the
                    // line above, so from here the honest sentence for a failure is "recorded, head
                    // not moved" rather than "no terminal record". See
                    // [`RecoverPartial::recorded_without_head`].
                    self.recover_partial.recorded_without_head.push(id);
                    // 🔴 **R6 / DR-43-11** — the tree reached this leaf, so the head moves with it.
                    // Last among the writes, exactly as `Engine::commit` and the road below place
                    // it.
                    self.record_head(at, key)?;
                    self.recover_partial
                        .recorded_without_head
                        .retain(|row| *row != id);
                    let state = self.set_state(&id, Lifecycle::Committed, at);
                    return Ok(Recovered {
                        transformation: id,
                        path: RecoveryPath::ClosedFromFiledReceipt,
                        state,
                        ledger_seq: Some(entry.index),
                        appended: None,
                        payload_matched: Some(true),
                        // The document was already filed; this run issued nothing and signed
                        // nothing. `None` is what says so — `gx repair`'s `receipts_missing` is
                        // the count that answers whether anybody wrote one.
                        receipt: None,
                        refusal: None,
                    });
                }
            }
        }

        // Everything the receipt needs, from the journal and the blob store alone.
        let Some(delta_cid) = row.delta_cid else {
            // A journal trimmed past its own `Planned` record (42 §5) cannot name the delta.
            let state = self.abort(&id, AbortReason::InternalError, None, at)?;
            return Ok(refused(state));
        };
        let (Some(canonical_cid), Some(fp0)) = (row.canonical_cid, row.fp0.clone()) else {
            let state = self.abort(&id, AbortReason::InternalError, None, at)?;
            return Ok(refused(state));
        };
        // 🔴 **E-M5-11**, on the recovery's side of the door. Hand 4 refused a `Committing` row
        // with no verdict outright; since §41 the degraded admission of 43 T-4e is representable
        // and **reachable** (hand 6 commits one), so a crash inside its critical section has to be
        // recoverable too -- refusing here would make T-4e the one transition a restart could not
        // finish. What is still refused is the half-filled pair, for `commit`'s reason.
        let verdict =
            match (row.verdict, row.verdict_digest) {
                (Some(kind), Some(proof_digest)) => Some(VerdictSummary { kind, proof_digest }),
                (None, None) if row.fail_posture_engaged => None,
                _ => return Err(Error::Unrepresentable {
                    what:
                        "a CommitReceipt rebuilt with no verdict and no engaged fail-open posture",
                    detail: format!(
                        "{id:?} was in `Committing` with verdict={:?} digest={:?} \
                         fail_posture_engaged={}; the recovery has nothing true to put there",
                        row.verdict, row.verdict_digest, row.fail_posture_engaged
                    ),
                }),
            };
        let delta = self.blobs.get(&delta_cid)?;
        let adapter = self
            .adapters
            .get(delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", delta.substrate()),
            })?
            .adapter
            .clone();

        // 🔴 Two-phase escrow's crash window (`req/38` §98 ruling 1's named residue, §99
        // ruling 2) (sem: SEM-gx-engine-283): a `Pending` row here means the process died between T-10b and the completion's
        // outcome record. Two sub-cases, both journalled now so Σ converges:
        //   * an `ApplyObserved` exists → the observation survived; re-resolve from it (the same
        //     settle road the live commit takes — 43 §7-3's re-resolution, side-effect-free
        //     because the material is the journal's and the stores');
        //   * none exists → the answer died with the process and is not re-obtainable (the
        //     idempotency contract never re-issues the call, `req/160` 1-0 fact 3) (sem: SEM-gx-engine-284) → the honest
        //     fold: `InverseCompleted { None }` → `Unavailable`.
        let inverse_cid = match escrow_row {
            Some(er) if matches!(er.status, InverseStatus::Pending) => {
                let partial = er.inverse_cid.and_then(|cid| self.blobs.get(&cid).ok());
                let observed = row
                    .observation_cid
                    .and_then(|cid| self.observations.get(&cid).ok());
                self.settle_pending_escrow(&id, partial.as_ref(), observed.as_deref(), at)?
            }
            Some(er) => er.inverse_cid,
            None => None,
        };

        // 🔴 **R9 / `req/236` H-03** — the payload is rebuilt under the key the **leaf** was signed
        // with, which is the one the already-filed receipt names. See
        // [`CommitReceiptSink::filed_key_id`] for the measurement that made this necessary.
        //
        // 🔴 **R33 / `req/397` H-01** — read here rather than after the world is touched, because
        // the refusal below chooses its sentence by asking whether this value is present.
        let filed_key_id = self
            .receipt_sink
            .as_ref()
            .and_then(|sink| sink.filed_key_id(&id));

        // 🔴 **R33 / `req/397` H-01** — where 42 §3.10's `postcondition_fingerprint` comes from,
        // and the whole of the repair.
        //
        // # What the audit measured
        //
        // Until R33 this road announced an `ApplyStarted` record and called `apply_once` **before**
        // it compared the rebuilt payload against the leaf the ledger already held. A recovery that
        // was going to refuse had therefore already written to somebody's substrate by the time it
        // refused — and the sentence it refused with said "Nothing was applied". `req/397` §2-2
        // measured `adapter_apply_calls=1` on an untampered project recovered under a second key
        // (`RECOVERY_KEY_MISMATCH`'s designed trigger, with no tampering at all), and §2-3 measured
        // a world going `"two\n"` -> `"one\n"` on a journal whose open row had been pointed at a
        // delta the project already held.
        //
        // # Why the re-apply was here, and why it does not have to be
        //
        // 43 §7-3b's window is a crash **between** `ledger.append` and the `Committed` record, and
        // `Engine::commit` reaches the ledger **after** `apply_once` returns. A leaf in the ledger
        // for this row is therefore proof that the apply already happened: the world is already
        // where the commit left it, and the only thing the re-apply ever produced was 42 §3.10's
        // `postcondition_fingerprint`. 42 §3.10 calls that a **reading** of the world, and a
        // reading is what [`Engine::observed_postcondition`] takes — the same reading
        // [`Engine::reissue_receipt`] has rebuilt the same payload from since R8, on a road that
        // applies nothing at all.
        //
        // So the two roads separate, and the separation is the repair:
        //
        // * **the ledger holds the leaf (43 §7-3b)** — read the world, rebuild, compare, and then
        //   close or refuse. `adapter.apply` is never reached, so a refusal on this road is a
        //   statement about a world this run did not touch.
        // * **the ledger holds nothing (43 §7-3c)** — the commit did not complete, the delta may
        //   never have landed, and finishing it is what the recovery is for. That road announces
        //   and applies exactly as it always did.
        //
        // # What changed for an operator, said plainly
        //
        // A §7-3b row whose world somebody else moved after the crash used to be silently
        // overwritten by the re-apply and then closed. It is now **refused**, with
        // [`not_resumed::RECOVERY_REBUILD_DISAGREES`] naming what disagrees and what did not
        // happen. That is a behaviour change and it is the point: a recovery that quietly rewrites
        // a file nobody asked it about is `req/227` H-01's accident with the ledger's check placed
        // one step too late.
        let postcondition = match &held {
            // 43 §7-3b — the ledger already witnessed this commit.
            Some(entry) => {
                // 🔴 **The world, read rather than written.**
                //
                // The locator is the one the `Planned` record names, which is the field
                // `reissue_receipt` reads for the same purpose. A journal trimmed past that record
                // cannot name it, and that is the same nothing-to-work-from the `delta_cid` guard
                // above answers.
                let Some((locator, _, _)) = self.planned_record(&id) else {
                    let state = self.abort(&id, AbortReason::InternalError, None, at)?;
                    return Ok(refused(state));
                };
                match self.observed_postcondition(adapter.as_ref(), &locator) {
                    Ok(observed) => observed,
                    // 🔴 **R13 / `req/244` H-03** — unchanged in substance, and now reached by a
                    // failure to **read** rather than by a failure to write.
                    //
                    // Where the ledger holds the leaf, 43 §7-3b has already said the commit
                    // completed before the crash; the rebuild is only how 42 §3.10's
                    // `postcondition_fingerprint` is obtained, and a process that cannot reach the
                    // substrate has failed to read, not discovered that the commit failed. Writing
                    // `Aborted` there is gx recording a falsehood, and recording it terminally:
                    // `req/244` H-03 measured the whole consequence — `gx wrap` commits killed
                    // inside §7-3b's window became projects no `gx repair --yes` could ever close,
                    // over leaves whose commit receipts `gx receipt verify` answers `valid: true`,
                    // `inclusion: "verified"` for.
                    //
                    // R33 narrows what "cannot reach" means and does not widen it: the call that
                    // fails here is `snapshot`/`precondition` rather than `apply`, so the arm is
                    // taken by an adapter that cannot be **asked** at all — an MCP server that is
                    // not running, an adapter that is not registered, a locator that is gone —
                    // which is the population `req/244` H-03 measured in the first place.
                    Err(_) => {
                        self.journal_append(EngineJournalRecord::Committed {
                            transformation: id,
                            ledger_seq: entry.index,
                            at,
                        })?;
                        self.committed.insert(id, entry.index);
                        self.resumed_rows += 1;
                        // 🔴 **R37 / `req/496` M-01** — recorded from here; only the head is left.
                        self.recover_partial.recorded_without_head.push(id);
                        // 🔴 **R6 / DR-43-11** — the tree reached this leaf, so the head moves with
                        // it.
                        self.record_head(at, key)?;
                        self.recover_partial
                            .recorded_without_head
                            .retain(|row| *row != id);
                        let state = self.set_state(&id, Lifecycle::Committed, at);
                        return Ok(Recovered {
                            transformation: id,
                            path: RecoveryPath::ClosedFromLedgerLeaf,
                            state,
                            ledger_seq: Some(entry.index),
                            appended: None,
                            // Nothing was compared, because nothing was rebuilt. `Some(true)` here
                            // would be this run claiming a check it did not make.
                            payload_matched: None,
                            // And no receipt was issued. `gx repair`'s `receipts_missing` counts
                            // this row and `--reissue-receipts` is the road to one.
                            receipt: None,
                            refusal: None,
                        });
                    }
                }
            }
            // 43 §7-3c — the ledger holds nothing for this row, so the commit did not complete and
            // the delta may never have landed. This is the road that applies.
            None => {
                // 43 §7-3c: "`adapter.apply` is designed to be idempotent under the adapter
                // contract, so re-running it is safe" (sem: SEM-gx-engine-285).
                // Announced again for the reason it was announced the first time (E-M5-1): a crash
                // inside *this* call must be distinguishable from a crash before it.
                self.journal_append(EngineJournalRecord::ApplyStarted {
                    transformation: id,
                    delta_cid,
                    at,
                })?;
                match self.apply_once(adapter.as_ref(), &delta) {
                    Ok(applied) => {
                        // 🔴 **R36 / `req/476` H-01** — the substrate has just been written, and
                        // from here to the `Ok` at the bottom of this function there are eight
                        // fallible steps. The row is recorded as *applied and not yet recorded*
                        // now, and taken off that list only when it is returned as `Recovered`
                        // below. Whatever survives an `Err` is exactly the set that wrote and was
                        // never accounted for — which is the set audit 35 found nobody had.
                        self.recover_partial.applied_unrecorded.push(id);
                        // E-M4-31 / M5-18 adopted (a) (sem: SEM-gx-engine-286), on the recovery's
                        // side of the door as well.
                        let applied = AppliedDelta::new(
                            applied.delta().clone(),
                            applied.postcondition().clone(),
                            *applied.resulting_digest(),
                            at,
                        );
                        applied.postcondition().clone()
                    }
                    // The adapter refused an application it is contracted to accept twice (41 §4).
                    // The rollback of T-10c is not attempted: the escrowed inverse restores the
                    // state the snapshot was taken over, and applying it after a *successful*
                    // earlier apply would undo a commit the ledger may already hold. Raised as
                    // **M5H5-5**.
                    //
                    // 🔴 **R33 / `req/397` H-01** — the `held.is_some()` half of this arm moved up
                    // to the read, which is where it belongs. With no leaf in the ledger, an
                    // adapter that refuses is a commit that did not happen, and
                    // `Aborted(ApplyFailed)` is the true record.
                    Err(_) => {
                        // 🔴 **`req/324` §5(d)** — the third fact: `gx repair` closed a row it
                        // never rebuilt, so no inverse existed in this process to attempt.
                        self.not_attempted_because
                            .insert(id, NotAttemptedBecause::RecoveredWithoutRebuilding);
                        let state = self.abort(
                            &id,
                            AbortReason::ApplyFailed,
                            Some(Rollback::NotAttempted),
                            at,
                        )?;
                        return Ok(Recovered {
                            transformation: id,
                            path: RecoveryPath::ApplyWasAnnounced,
                            state,
                            ledger_seq: None,
                            appended: None,
                            payload_matched: None,
                            receipt: None,
                            refusal: None,
                        });
                    }
                }
            }
        };

        // 🔴 **DR-46-26** — the two seats, from the journal and from the escrow row. See
        // [`Engine::rebuilt_attest`] for why neither comes from the filed receipt.
        let (filed_read_set, filed_reversibility) =
            self.rebuilt_attest(&id, escrow_row.map(|row| row.status))?;
        // 🔴 **DR-46-28** — read before `verdict` moves into the literal below.
        let verdict_present = verdict.is_some();
        let mut payload = ReceiptPayload {
            key_id: filed_key_id.clone().unwrap_or_else(|| key.key_id().clone()),
            verdict,
            enforced: row.enforced,
            // 🔴 **`req/493` §0 / AC-6** — read back out of Σ, not out of this process.
            //
            // 43 §7-3b compares this payload's digest against the leaf the ledger already holds, and
            // the process doing the repair is not the process that committed: re-reading
            // `self.confinement` here would answer `payload_mismatch` — the word for tampering — on
            // every crash-window recovery of a commit made inside a `gx confine` and repaired
            // outside one. `ProvenanceDerived` is journalled before the world moves (M5-25 adopted
            // (a)) precisely for this window, so the answer is in Σ whenever the commit reached
            // T-9. `None` for a journal written before the erratum, which reproduces the absence in
            // the filed receipt rather than inventing a `false` about a process nobody observed.
            confinement: row
                .provenance
                .as_ref()
                .and_then(|p| p.environment.confinement.clone()),
            catalogue_hash: None,
            // F7 / R-868-6 (`req/919` W5): every receipt this build issues is `Some`, on both
            // kinds and with no kind-dependent rule -- see the field's own doc comment.
            payload_version: Some(CURRENT_PAYLOAD_VERSION),
            // 🔴 **A2 (`req/910`, `req/919` W8)** — read back out of Σ, not out of this process,
            // and for `confinement`'s reason four lines up rather than by analogy with it: 43
            // §7-3b digests this payload against the leaf the ledger already holds, and answering
            // from `crate::VERSION` here would make every repair performed by a build other than
            // the committing one report `payload_mismatch` -- the word for tampering -- for the
            // ordinary upgrade 47 §4 describes. `None` for a journal written before M5-25 carried
            // a provenance record, which reproduces the absence rather than inventing a version
            // nobody wrote down.
            engine_version: row
                .provenance
                .as_ref()
                .map(|p| p.environment.engine_version.clone()),
            receipt_kind: ReceiptKind::CommitReceipt,
            canonical_cid,
            inverse_delta: inverse_cid,
            transformation: id,
            inclusion_proof: None,
            fail_posture_engaged: row.fail_posture_engaged,
            // 🔴 **DR-46-24(A)** — as above; a rebuild carries what the filed receipt carried.
            // 🔴 **DR-46-26** — and now it does. See [`Engine::filed_attest`] for why these two are
            // taken from one source rather than one taken and one derived.
            read_set: filed_read_set,
            reversibility: filed_reversibility,
            // 🔴 **DR-46-45 (`req/973` §B-1/§B-2)** — reproduced from the same `Planned` record the
            // live road read, which is the whole reason the witness was journalled: a rebuild that
            // re-derived it would answer `payload_mismatch` for every crash-window recovery of an
            // undo, and `payload_mismatch` is the word for tampering.
            undo: self.journalled_undo(&id),
            // 🔴 **DR-46-28 / DR-46-33** — reproduced rather than read back. The verdict stage
            // comes from `verdict`, which Σ carries (`StateRow::verdict`); the input-generation
            // stage comes from `journalled_input_generation` — the `Planned` record's field, which
            // is where the plan-time join was journalled for exactly this road. Neither reaches the
            // actor (not in Σ), and 43 §7-3b compares this payload's digest against the leaf the
            // ledger already holds; a boundary from the actor could not be reproduced, one from the
            // journalled result can.
            determinism_boundary: attested_boundary(
                self.journalled_input_generation(&id),
                verdict_present,
            ),
            fingerprint_scope: fp0.scope().to_string(),
            precondition_fingerprint: FingerprintBytes(fp0.digest().0),
            // 🔴 **R33 / `req/397` H-01** — a reading on 43 §7-3b's road, the apply's own answer on
            // §7-3c's. Which one it was is decided above, before anything is written.
            postcondition_fingerprint: Some(FingerprintBytes(postcondition.digest().0)),
        };
        let receipt_digest = payload.ledger_digest().map_err(|e| Error::Witness {
            action: "digest the rebuilt receipt payload",
            detail: e.to_string(),
        })?;

        let (leaf, appended, payload_matched, path) = match &held {
            // 43 §7-3b.
            Some(entry) => {
                let matched = entry.receipt_digest == receipt_digest;
                if !matched {
                    // The rebuild is not the thing the ledger witnessed, so re-issuing would put a
                    // second answer to "what was committed" (sem: SEM-gx-engine-287) into the world. Fail-closed.
                    //
                    // 🔴 **R9 / `req/236` H-03** — fail-closed, and **without a terminal record**.
                    //
                    // Until R9 this arm called `self.abort(InternalError)`. That is a terminal
                    // record, and a terminal record is what 43 §7-2 makes the recovery stop at — so
                    // the first run under the wrong key permanently removed the row's only way out.
                    // The audit measured the whole shape: 8 of 8 runs bricked, `gx serve` then
                    // refusing to start with `LEDGER_DISAGREES` over a world that had already
                    // moved, and a later run **with the correct key** answering `resumed: 0`.
                    //
                    // The state is left exactly as it was. `RecoveryPath::NotResumed` is `req/227`
                    // H-01's answer for "the row was left alone", `gx repair` counts it under
                    // `refused` rather than under `resumed`, and the refusal below is the sentence
                    // that says what disagrees. Running the recovery again after the disagreement
                    // is settled arrives here with a payload that matches, and the row closes.
                    //
                    // 🔴 **R33 / `req/397` H-01** — two things changed here, and both are about
                    // what the sentence is entitled to say.
                    //
                    // 1. **`adapter.apply` has not run.** The `postcondition` above was *read* off
                    //    the substrate on this road, so "nothing was applied" is now a property of
                    //    the code rather than a claim the code makes about itself. `req/397` §2-2
                    //    measured the old order at `adapter_apply_calls=1` and §2-3 measured a
                    //    world going backwards on it.
                    // 2. **The cause is a disjunction, not a diagnosis.** The old sentence here
                    //    said "the difference is the signing key" as a plain assertion, and §2-3
                    //    measured it false on a bed where both sessions used a byte-identical key
                    //    (`req/397` §2-4, the fourth member of monitoring 31 M-02's family). A
                    //    digest that does not match says the payload is not the witnessed one; it
                    //    does not say which field moved. The key case that *can* be established is
                    //    established, **by measurement**, and only then: the
                    //    project's recorded head is a *signed* statement of which key it has been
                    //    written under, and [`Engine::reissue_receipt`] has consulted it since R9
                    //    (`req/236` M-05) for exactly this order — before blaming the world, check
                    //    the key. Where the head names a different key, the difference really is
                    //    the signing key and [`not_resumed::RECOVERY_KEY_MISMATCH`] says so. Where
                    //    it does not, or where there is no head to ask,
                    //    [`not_resumed::RECOVERY_REBUILD_DISAGREES`] lists the causes rather than
                    //    picking one, in the shape R32 gave [`not_resumed::journal_departed`].
                    //
                    //    The check is **after** the comparison and not in front of it, and the
                    //    difference is what `payload_matched` is allowed to say: a gate that
                    //    returned before the rebuild would have to answer `None` there, and
                    //    `gx repair`'s `payload_mismatch` — which `model_a_probes.rs` reads as
                    //    "the refusal is counted as itself" — would stop counting the very row it
                    //    exists for. Nothing is lost by asking later: this whole road applies
                    //    nothing, so there is no write for an earlier gate to prevent.
                    let refusal = if filed_key_id.is_none()
                        && self
                            .recorded_head_key_id()
                            .is_some_and(|recorded| recorded != key.key_id())
                    {
                        not_resumed::RECOVERY_KEY_MISMATCH
                    } else {
                        not_resumed::RECOVERY_REBUILD_DISAGREES
                    };
                    return Ok(Recovered {
                        transformation: id,
                        path: RecoveryPath::NotResumed,
                        state: Lifecycle::Committing,
                        ledger_seq: Some(entry.index),
                        appended: None,
                        payload_matched: Some(false),
                        receipt: None,
                        refusal: Some(refusal),
                    });
                }
                (
                    entry.index,
                    None,
                    Some(true),
                    RecoveryPath::LedgerHeldTheCommit,
                )
            }
            // 43 §7-3c. `ledger.append` is key-idempotent (ASM-43-1), so "even if a past attempt
            // partially reached the ledger, no duplicate entry results" (sem: SEM-gx-engine-288)
            // is the collaborator's guarantee and the
            // outcome is *reported* rather than branched on.
            None => {
                let outcome =
                    self.ledger
                        .append(id, receipt_digest, at)
                        .map_err(|e| Error::Ledger {
                            action: "append",
                            detail: e.to_string(),
                        })?;
                let kind = match outcome {
                    gx_log::AppendOutcome::Appended(_) => "Appended",
                    gx_log::AppendOutcome::AlreadyPresent(_) => "AlreadyPresent",
                };
                (
                    outcome.entry().index,
                    Some(kind),
                    None,
                    RecoveryPath::ApplyWasAnnounced,
                )
            }
        };

        let proof: InclusionProof =
            prove_inclusion(self.ledger.log(), leaf).map_err(|e| Error::Ledger {
                action: "prove inclusion",
                detail: e.to_string(),
            })?;
        payload.inclusion_proof = Some(proof);

        // 🔴 **R8 / `req/234` H-01** — 43 §7-3b says "re-issue the receipt from the existing
        // `InclusionProof` (if not yet issued)", and until R8 this function did the re-issuing and
        // then handed the document back to five callers who all dropped it. It is filed here, on
        // the same line of the same order the live commit uses, so that "the recovery re-issued it"
        // and "the project holds it" stop being two different facts. `Recovered::receipt` is still
        // returned — a caller with no sink registered is still the writer, and `gx repair`'s
        // `receipts_missing` is the count that says whether anybody wrote.
        //
        // 🔴 **R9 / `req/236` H-03** — "(if not yet issued)" is now read literally.
        //
        // The payload above names the key the archive already holds a receipt under, so signing it
        // with *this* process's key would mint a document whose `key_id` and whose signature name
        // two different keys — a receipt that fails `verify_offline`'s fourth condition and that
        // gx would have written itself. Where the archive holds the receipt, it **is** the issued
        // receipt: this run has proved that the leaf and the world agree with it and files nothing.
        // Where the archive holds none, the payload was built under this run's key, and issuing it
        // is what R8 put here.
        let receipt = match &filed_key_id {
            Some(_) => None,
            None => {
                let issued = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
                    action: "re-issue the receipt",
                    detail: e.to_string(),
                })?;
                self.file_receipt(&id, &issued)?;
                Some(issued)
            }
        };

        self.journal_append(EngineJournalRecord::Committed {
            transformation: id,
            ledger_seq: leaf,
            at,
        })?;
        self.committed.insert(id, leaf);
        // 🔴 **R7 / `req/232` M-01** — counted **before** the head is written, because the head's
        // laundering guard is about this very run: a project with no floor does not get its first
        // one minted by a road that has just written to somebody's world.
        self.resumed_rows += 1;
        // 🔴 **R37 / `req/496` M-01** — the two lines below were **one** line under R36 and both
        // sat after `record_head`, which is what made the `Err` road's sentence false.
        //
        // R36's note read: "the row is recorded, so it is no longer *applied and unrecorded*.
        // Everything above this line can raise, and that is the point of the list." The first
        // clause was right about the wrong line. `journal_append(Committed)` above is where the row
        // stops being unrecorded — 43 §7-2's terminal record is durable from there — and
        // `record_head` is one further write after it. Audit 36 sealed `.gx/checkpoints/`, watched
        // the `Committed` record land, and read the operator being told this run "left no terminal
        // record of having done so" and that "the row stays resumable"; the remedy's instruction,
        // carried out, answered `terminal: 2, resumed: 0`.
        //
        // So the row leaves `applied_unrecorded` at the record and joins
        // [`RecoverPartial::recorded_without_head`] until the head lands. Both lists are still
        // emptied only by the row reaching an `Ok`, so an `Err` at any step still leaves exactly
        // one true description of where the row got to.
        self.recover_partial
            .applied_unrecorded
            .retain(|row| *row != id);
        self.recover_partial.recorded_without_head.push(id);
        // 🔴 **R6 / DR-43-11** — a resume that finished a commit moved the tree, so the head moves
        // with it. `Engine::commit`'s note says why this is the last write of the sequence.
        self.record_head(at, key)?;
        self.recover_partial
            .recorded_without_head
            .retain(|row| *row != id);
        Ok(Recovered {
            transformation: id,
            path,
            state: Lifecycle::Committed,
            ledger_seq: Some(leaf),
            appended,
            payload_matched,
            receipt,
            refusal: None,
        })
    }

    // 🔴 **R8 / `req/234` H-01 (b)** — `Engine::reissue_receipt` is defined **below**
    // `apply_once`, and the position is load-bearing rather than tidy. `tests/crash_recovery.rs`
    // reads the span from `pub fn recover(` to `fn apply_once(` and asserts that neither
    // `cas_eq` nor `.precondition(` appears in it — that is req/78 §3.2 Λ4 as a scan, and Λ4 is
    // about a road that **applies**. `reissue_receipt` reads the world and never applies, so it
    // would satisfy Λ4's intent and break Λ4's instrument; putting it outside the span keeps the
    // scan measuring the thing it was built to measure.

    /// 🔴 **Rule 2**: the only place in this crate that asks a substrate to change (req/78 §3.3).
    ///
    /// > **Rule 2 (there is one road to `S`)**: the call site for `adapter.apply` must be
    /// > **exactly one place** across the whole engine (sem: SEM-gx-engine-289)
    ///
    /// Both of T-10c's applications go through it -- the forward delta and, when that fails, the
    /// escrowed inverse -- so the single road is a fact about the source and not about how many
    /// times it is walked. `tests/ac_035.rs` measures both halves: a scan that finds exactly one
    /// invocation line in `src/`, and a counting adapter that says how many times it was reached.
    ///
    /// The function is deliberately thin. Anything it did beyond calling and naming the failure
    /// would be work happening on the far side of the one door.
    fn apply_once(
        &self,
        adapter: &dyn SubstrateAdapter,
        delta: &PlannedDelta,
    ) -> Result<AppliedDelta> {
        adapter.apply(delta).map_err(|e| Error::Adapter {
            action: "apply",
            detail: e.to_string(),
        })
    }

    /// 🔴 **R33 / `req/397` H-01** — 42 §3.10's `postcondition_fingerprint`, taken as a reading.
    ///
    /// # Why this is defined **below** `apply_once`
    ///
    /// For the identical reason R8 put [`Engine::reissue_receipt`] here and R29 put
    /// [`Engine::world_is_still_at`] here, and the comment above `apply_once` spells it out:
    /// `tests/crash_recovery.rs` reads the span from `pub fn recover(` to `fn apply_once(` and
    /// asserts that neither `cas_eq` nor `.precondition(` appears in it — req/78 §3.2 Λ4 as a scan,
    /// about a road that **applies**. This function reads the world and never applies, so it
    /// satisfies Λ4's intent and would break Λ4's instrument. Outside the span is where it belongs.
    ///
    /// # What this is, and what it is not
    ///
    /// It is **one read** of the object this row names, folded into a [`Fingerprint`] by the same
    /// two calls `Engine::reissue_receipt` has made since R8 — `snapshot` then `precondition`, both
    /// declared side-effect-free by 41 §4 and 51 §7 contract 1.
    ///
    /// It is **not** the CAS of 43 T-10a. Nothing here is compared against `fp0`, and no
    /// `Aborted(PreconditionChanged)` can come out of it: what the value feeds is the
    /// `postcondition_fingerprint` field of a payload whose digest is then held against the leaf
    /// the ledger already witnessed. That is a comparison against a **signed past**, which is the
    /// distinction Λ4 is about — a recovery must not mistake its own footprint for interference,
    /// and this road leaves no footprint to mistake.
    ///
    /// It is **not** atomicity either, and the residue is the one R8 and R29 both declare: between
    /// this read and anything else, a third party can write. What that costs here is a refusal
    /// where a close was due, which is the direction a recovery should fail in.
    ///
    /// # Errors
    /// [`Error::Adapter`] when the substrate cannot be asked — a server that is not running, an
    /// adapter that is not registered, a locator that is gone. `Engine::resume` reads that as
    /// `req/244` H-03's "provable but not closable" and closes the row from the leaf without
    /// rebuilding, rather than as a disagreement.
    fn observed_postcondition(
        &self,
        adapter: &dyn SubstrateAdapter,
        locator: &str,
    ) -> Result<Fingerprint> {
        let snap = adapter.snapshot(locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        adapter.precondition(&snap).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })
    }

    /// 🔴 **R29 / `req/361` H-01** — after 43 T-10c's roll-back was accepted, ask the **object**
    /// whether it is home, rather than believing the call that was just made about it.
    ///
    /// # Why this is defined **below** `apply_once`
    ///
    /// For the identical reason R8 put [`Engine::reissue_receipt`] here, and the comment above
    /// `apply_once` already spells it out: `tests/crash_recovery.rs` reads the span from
    /// `pub fn recover(` to `fn apply_once(` and asserts that neither `cas_eq` nor `.precondition(`
    /// appears in it — req/78 §3.2 Λ4 as a scan, about a road that **applies**. This function reads
    /// the world and never applies, so it satisfies Λ4's intent and would break Λ4's instrument.
    /// Outside the span is where it belongs; the position is load-bearing rather than tidy.
    ///
    /// # 🔴 What this is, and what it is not
    ///
    /// It is **one read of the object after the roll-back**, compared against the fingerprint the
    /// transformation started from. It is deliberately the mirror of R8's second forward CAS, and
    /// it inherits R8's honesty about its own limits: **it is not atomicity, and it is not
    /// attribution.** Three residues are declared rather than argued away, and `docs/LIMITS.md`
    /// carries them where a buyer reads:
    ///
    /// 1. **The window.** Between the inverse's `apply` returning and this read there is a gap. A
    ///    third party who writes in it makes a homecoming look like a divergence, or the reverse.
    /// 2. **Attribution.** `Diverged` says *the object is not at `fp0`*. It does not say the
    ///    roll-back moved it. One fingerprint cannot tell "the compensation overshot" from
    ///    "somebody else wrote".
    /// 3. **Coarseness.** A fingerprint is an equality, not a distance. An object one byte from
    ///    home and an object unrecognisable both answer `Diverged`, and the word is the same.
    ///
    /// What it removes is narrower than any of those, and is the whole finding: **the terminal
    /// state a reader is handed can no longer be `Succeeded` over an object that is demonstrably
    /// not where it started.** `req/361` H-01 produced that exact body on disk.
    ///
    /// # Why a read failure is `Failed` rather than a fifth word
    ///
    /// Because the sentence for `Failed` was already true of it. `crates/gx-cli/src/wrap.rs` has
    /// said since R25 that a compensation "whose bytes landed and whose read-back died lands here
    /// too" — the adapter's own `apply` is the call together with its read-back. A snapshot that
    /// will not answer is the same fact one layer up, and minting a word for it would split a
    /// reader's attention without changing what they can do next.
    /// 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the **first** of the two reads that
    /// stand in front of 43 T-10c's compensation: taken the instant the forward `apply` answered
    /// `Err`, it says what our own apply left behind.
    ///
    /// # Why this exists at all
    ///
    /// [`Engine::rollback_landed`] is the read **after** the compensation, and R29 put it there
    /// because `Ok(())` says nothing about the world. This is the read **before** it, and it is a
    /// different question: not *did the compensation work* but *may the compensation run*.
    ///
    /// The twenty-ninth audit is what made the second question load-bearing. It drove all four
    /// shipped adapters and established that every shipped delta grammar is **absolute** — no
    /// payload's effect is a function of the state it starts from (`req/372` §1). That is a good
    /// property for a compensation and a dangerous one for an *unconditional* compensation:
    /// an absolute inverse comes home from **any** world, and one of the worlds it comes home from
    /// is a world a third party legitimately created. `A29_GIT_THIRD_PARTY` is that sentence with
    /// a hash on it — `theirs=d2d09b5` off the branch, `their_commit_is_still_the_tip=false`, and
    /// `Succeeded` in the record.
    ///
    /// # The shape is deliberately R8's, reflected a second time
    ///
    /// R8 / `req/234` H-04 put a second read of `fp0` in front of the **forward** apply. R29 put a
    /// read behind the **roll-back's** apply. This is the fourth corner of that square, and after
    /// it every write this engine issues has a `fp0` comparison in front of it. `req/38` §227's
    /// sweep is why it is phrased as one predicate in one function rather than three lines inlined
    /// at the call site: the same question is asked at `gx undo` (v0.4-o), at the forward apply
    /// (R8) and here, and a question spelled three times is a question repaired in one place.
    ///
    /// # Errors are a refusal, not a shrug
    ///
    /// A `snapshot` or `precondition` that will not answer, and a `cas_eq` whose two fingerprints
    /// are not comparable, all return [`NotAttemptedBecause::WorldCouldNotBeRead`]. Sending an
    /// absolute inverse into a substrate that will not say where it is, is the unconditional write
    /// this function exists to stop — the fact that the reason is *ignorance* rather than
    /// *movement* makes it worse, not better. It is reported as its own cause because "I looked
    /// and it had moved" is not "I could not look" (see the two causes' own docs).
    ///
    /// # 🔴 The residue
    ///
    /// Two calls, so there is a window between this read and the `apply` that follows it, and a
    /// third party writing inside that window is still overwritten. `docs/LIMITS.md` v0.5-q
    /// carries the **measured** width on fs and on mcp. This is not atomicity, and it is not
    /// attribution: this transformation's own half-landed forward `apply` is indistinguishable
    /// from somebody else's write, and both are reported as
    /// [`NotAttemptedBecause::WorldMovedBeneath`].
    fn world_the_failed_apply_left(
        &self,
        adapter: &dyn SubstrateAdapter,
        locator: &str,
        fp0: &Fingerprint,
    ) -> CompensationVerdict {
        let Some(fp) = self.read_world(adapter, locator) else {
            return CompensationVerdict::Unreadable;
        };
        match fp0.cas_eq(&fp) {
            Ok(true) => CompensationVerdict::NothingToTakeBack,
            Ok(false) => CompensationVerdict::TakeBackFrom(fp),
            // 42 §3.5's third case, the same one `rollback_landed` meets on the way out: the two
            // fingerprints are not comparable. Not "it is home" and not "it moved" -- the read
            // declining to answer, which is the road above.
            Err(_) => CompensationVerdict::Unreadable,
        }
    }

    /// 🔴 **R30 / `req/372` M-01** — the second of the two reads: *is the world still exactly
    /// where the failed apply left it*.
    ///
    /// `None` is the read declining to answer, kept distinct from `Some(false)` for the reason
    /// [`NotAttemptedBecause::WorldCouldNotBeRead`] exists at all: "I looked and it had moved" is
    /// not "I could not look".
    ///
    /// This is the compare half of a compare-and-set spelled as two calls, so the window between
    /// it and the `apply` that follows is real. `docs/LIMITS.md` v0.5-q carries its measured width.
    fn world_is_still_at(
        &self,
        adapter: &dyn SubstrateAdapter,
        locator: &str,
        expected: &Fingerprint,
    ) -> Option<bool> {
        let fp = self.read_world(adapter, locator)?;
        expected.cas_eq(&fp).ok()
    }

    /// One reading of the object through the adapter, or `None` if it will not answer.
    ///
    /// The two reads above and [`Engine::rollback_landed`]'s read after the compensation all go
    /// through here, so "how this engine looks at the world" is one road rather than three
    /// spellings of it (`req/38` §227's sweep, applied to a read instead of to a gate).
    fn read_world(&self, adapter: &dyn SubstrateAdapter, locator: &str) -> Option<Fingerprint> {
        let guard = adapter.snapshot(locator).ok()?;
        adapter.precondition(&guard).ok()
    }

    fn rollback_landed(
        &self,
        adapter: &dyn SubstrateAdapter,
        locator: &str,
        fp0: &Fingerprint,
    ) -> Rollback {
        let Ok(guard) = adapter.snapshot(locator) else {
            return Rollback::Failed;
        };
        let Ok(fp2) = adapter.precondition(&guard) else {
            return Rollback::Failed;
        };
        match fp0.cas_eq(&fp2) {
            Ok(true) => Rollback::Succeeded,
            Ok(false) => Rollback::Diverged,
            // 42 §3.5's third case: the two fingerprints are not comparable. That is not "the
            // object is home" and it is not "the object moved" -- it is the read declining to
            // answer, which is the road above.
            Err(_) => Rollback::Failed,
        }
    }

    /// 🔴 **R8 / `req/234` H-01 (b)** — re-issue the receipt of a **terminal** commit, without
    /// asking the substrate to change anything.
    ///
    /// # The row this exists for
    ///
    /// `Engine::commit` now files the receipt before the `Committed` record, so every crash from
    /// R8 onward lands in 43 §7-3b's window and [`Engine::resume`] finishes it. What that does not
    /// reach is a row **already** terminal when this binary meets it: a project committed by a
    /// pre-R8 binary, or one whose caller registered no [`CommitReceiptSink`], where the row is
    /// `Committed` in Σ and no receipt was ever filed. `req/234` H-01 measured that row from four
    /// sides — `gx undo` exit 3 forever, `/v1/receipts/{tid}` 404, `gx receipt verify` exit 6, and
    /// `gx repair` calling the project healthy.
    ///
    /// # 🔴 Why this does not re-apply, and why that is not a weaker answer
    ///
    /// [`Engine::resume`] rebuilds the payload by **applying the delta again**, because 42 §3.10's
    /// `postcondition_fingerprint` is produced by `apply` and recorded nowhere (M5H5-3). Doing that
    /// for a terminal row is the exact road `req/227` H-01 measured as an operator's file going
    /// from `three` back to `one`: an old delta landing on a newer world.
    ///
    /// So this function obtains the postcondition the other way — it **reads** the world through
    /// `snapshot` + `precondition` (the locator is in the `Planned` record, which is also where
    /// [`Engine::rehydrate_committed`] gets it) and then **proves** the reading: the rebuilt
    /// payload's `ledger_digest` has to equal the digest the ledger witnessed at this row's leaf.
    /// The ledger is 42 §3.11's public witness and the digest is what a third party checks, so a
    /// payload that matches it *is* the document that was committed — nothing is being guessed and
    /// nothing is being minted. A world that has moved since simply fails the comparison and is
    /// reported as [`Reissued::WorldMoved`], which is the honest answer: the postcondition of that
    /// commit is no longer observable and no re-issue can be evidence of it.
    ///
    /// ∴ this call writes to `.gx/receipts/` and to nothing else. It never calls `apply`.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown row, a row with no `Planned` record, or an unregistered
    /// adapter. [`Error::Adapter`] if the substrate will not answer a read. [`Error::Witness`] from
    /// the signing and from the sink (see [`Engine::file_receipt`]).
    pub fn reissue_receipt(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Reissued> {
        let sigma = reconstruct(self.journal.records());
        let Some(row) = sigma.state_of(id).cloned() else {
            return Ok(Reissued::NotCommitted);
        };
        if !matches!(
            row.state,
            Some(Lifecycle::Committed | Lifecycle::Superseded)
        ) {
            return Ok(Reissued::NotCommitted);
        }
        let Some(entry) = self
            .ledger
            .log()
            .entries()
            .iter()
            .find(|e| e.transformation == *id)
            .cloned()
        else {
            return Ok(Reissued::NoLeaf);
        };
        let (Some(canonical_cid), Some(fp0), Some(delta_cid)) =
            (row.canonical_cid, row.fp0.clone(), row.delta_cid)
        else {
            return Ok(Reissued::NoMaterial);
        };
        let Some((locator, _, _)) = self.planned_record(id) else {
            return Ok(Reissued::NoMaterial);
        };
        let verdict = match (row.verdict, row.verdict_digest) {
            (Some(kind), Some(proof_digest)) => Some(VerdictSummary { kind, proof_digest }),
            (None, None) if row.fail_posture_engaged => None,
            _ => return Ok(Reissued::NoMaterial),
        };
        let delta = self.blobs.get(&delta_cid)?;
        let adapter = self
            .adapters
            .get(delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", delta.substrate()),
            })?
            .adapter
            .clone();
        // The one thing this function asks the substrate: what does it hold now. A read.
        let snap = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        let observed = adapter.precondition(&snap).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;
        let inverse_cid = sigma
            .escrow()
            .iter()
            .find(|e| e.transformation == *id)
            .and_then(|e| e.inverse_cid);
        // 🔴 **DR-46-26** — the same rule as the resume road's, and for the same reason.
        let escrow_status = sigma
            .escrow()
            .iter()
            .find(|e| e.transformation == *id)
            .map(|e| e.status);
        let (filed_read_set, filed_reversibility) = self.rebuilt_attest(id, escrow_status)?;
        // 🔴 **DR-46-28** — read before `verdict` moves into the literal below.
        let verdict_present = verdict.is_some();
        let mut payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict,
            enforced: row.enforced,
            // 🔴 **`req/493` §0 / AC-6** — read back out of Σ, not out of this process.
            //
            // 43 §7-3b compares this payload's digest against the leaf the ledger already holds, and
            // the process doing the repair is not the process that committed: re-reading
            // `self.confinement` here would answer `payload_mismatch` — the word for tampering — on
            // every crash-window recovery of a commit made inside a `gx confine` and repaired
            // outside one. `ProvenanceDerived` is journalled before the world moves (M5-25 adopted
            // (a)) precisely for this window, so the answer is in Σ whenever the commit reached
            // T-9. `None` for a journal written before the erratum, which reproduces the absence in
            // the filed receipt rather than inventing a `false` about a process nobody observed.
            confinement: row
                .provenance
                .as_ref()
                .and_then(|p| p.environment.confinement.clone()),
            catalogue_hash: None,
            // F7 / R-868-6 (`req/919` W5): every receipt this build issues is `Some`, on both
            // kinds and with no kind-dependent rule -- see the field's own doc comment.
            payload_version: Some(CURRENT_PAYLOAD_VERSION),
            // 🔴 **A2 (`req/910`, `req/919` W8)** — read back out of Σ, as on the rebuild road
            // above and for the same reason: a re-issue carries what the filed receipt carried, and
            // the process re-issuing is not necessarily the process that committed.
            engine_version: row
                .provenance
                .as_ref()
                .map(|p| p.environment.engine_version.clone()),
            receipt_kind: ReceiptKind::CommitReceipt,
            canonical_cid,
            inverse_delta: inverse_cid,
            transformation: *id,
            inclusion_proof: None,
            fail_posture_engaged: row.fail_posture_engaged,
            // 🔴 **DR-46-24(A)** — as above; a re-issue carries what the filed receipt carried.
            // 🔴 **DR-46-26** — and now it does. See [`Engine::filed_attest`].
            read_set: filed_read_set,
            reversibility: filed_reversibility,
            // 🔴 **DR-46-45 (`req/973` §B-1/§B-2)** — the re-issue road reads the same seat as the
            // resume road above and for the same reason: a re-issue carries what the filed receipt
            // carried, and R9's whole point is that the two digests agree.
            undo: self.journalled_undo(id),
            // 🔴 **DR-46-28 / DR-46-33** — reproduced rather than read back. The verdict stage
            // comes from `verdict`, which Σ carries (`StateRow::verdict`); the input-generation
            // stage comes from `journalled_input_generation` — the `Planned` record's field, where
            // the plan-time join was journalled for exactly this road. Neither reaches the actor
            // (not in Σ), and 43 §7-3b compares this payload's digest against the leaf; a boundary
            // from the actor could not be reproduced, one from the journalled result can.
            determinism_boundary: attested_boundary(
                self.journalled_input_generation(id),
                verdict_present,
            ),
            fingerprint_scope: fp0.scope().to_string(),
            precondition_fingerprint: FingerprintBytes(fp0.digest().0),
            postcondition_fingerprint: Some(FingerprintBytes(observed.digest().0)),
        };
        let rebuilt = payload.ledger_digest().map_err(|e| Error::Witness {
            action: "digest the re-issued receipt payload",
            detail: e.to_string(),
        })?;
        // 🔴 The proof. Everything above this line is a reconstruction; this is what makes it the
        // document the ledger witnessed rather than a new one that resembles it.
        if rebuilt != entry.receipt_digest {
            // 🔴 **R9 / `req/236` M-05** — before blaming the world, check the key.
            //
            // `key_id` is one of the payload's fields, so a re-issue run under a key other than
            // the one this project has been written under cannot reproduce the leaf whatever the
            // substrate holds. The recorded head is a **signed** statement of which key that is;
            // where it disagrees with this run's, the honest answer names the key.
            if self
                .recorded_head_key_id()
                .is_some_and(|recorded| recorded != key.key_id())
            {
                return Ok(Reissued::KeyMismatch);
            }
            return Ok(Reissued::WorldMoved);
        }
        let proof: InclusionProof =
            prove_inclusion(self.ledger.log(), entry.index).map_err(|e| Error::Ledger {
                action: "prove inclusion",
                detail: e.to_string(),
            })?;
        payload.inclusion_proof = Some(proof);
        let receipt = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
            action: "re-issue the receipt of a terminal commit",
            detail: e.to_string(),
        })?;
        self.file_receipt(id, &receipt)?;
        if let Some(row) = self.table.get_mut(id) {
            row.receipt = Some(receipt.clone());
        }
        Ok(Reissued::Filed(Box::new(receipt)))
    }

    /// Two-phase escrow, step 2 (`req/38` §98 ruling 1) (sem: SEM-gx-engine-290): journal the applied call's observed answer
    /// and file its bytes, content-addressed.
    ///
    /// Journal-first, then the body — a crash in between leaves a name whose body is gone, which
    /// the recovery folds to `Unavailable` honestly ([`ObservationStore::put`]'s note). Written
    /// only for a `Pending` escrow: a complete escrow needs no observation and gets no record.
    fn record_observation(
        &mut self,
        id: &TransformationId,
        bytes: &[u8],
        at: Timestamp,
    ) -> Result<()> {
        let observation_cid = ObservationStore::address(bytes);
        self.journal_append(EngineJournalRecord::ApplyObserved {
            transformation: *id,
            observation_cid,
            at,
        })?;
        self.observations.put(bytes)?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.observation_cid = Some(observation_cid);
        }
        Ok(())
    }

    /// Two-phase escrow, step 3 (`req/38` §98 ruling 1; §99 ruling 2-④ for the fold) (sem: SEM-gx-engine-291): resolve a
    /// `Pending` escrow's do-time members and journal the outcome — one road for the live commit
    /// and for a recovery resuming the same window.
    ///
    /// Every failure on this road — no observation captured, no completion registered, the
    /// completion answering `Ok(None)` **or `Err`** — is one fold: `InverseCompleted { None }` →
    /// `Unavailable`, and the caller's commit continues. An abort here would record "Aborted"
    /// about a world that already moved (the apply succeeded), which is the lie §99 ruling 2-④
    /// names (sem: SEM-gx-engine-292); the failure stays visible instead, on the journal and on the receipt
    /// (`inverse_delta: None` beside its Admit).
    fn settle_pending_escrow(
        &mut self,
        id: &TransformationId,
        partial: Option<&PlannedDelta>,
        observation: Option<&[u8]>,
        at: Timestamp,
    ) -> Result<Option<Cid>> {
        let completed = match (partial, observation) {
            (Some(partial), Some(bytes)) => {
                match self.completions.get(partial.substrate()).cloned() {
                    Some(completion) => match completion.complete_inverse(partial, bytes) {
                        Ok(Some(full)) => Some(full),
                        Ok(None) | Err(_) => None,
                    },
                    None => None,
                }
            }
            // No partial body (a crash between the escrow record and its blob), or no
            // observation: nothing to complete from, and the fold below says so.
            _ => None,
        };
        match completed {
            Some(full) => {
                let inverse_cid = full.reference().cid;
                self.journal_append(EngineJournalRecord::InverseCompleted {
                    transformation: *id,
                    inverse_cid: Some(inverse_cid),
                    at,
                })?;
                self.blobs.put(&full)?;
                self.escrow.insert(
                    *id,
                    EscrowRow {
                        transformation: *id,
                        inverse_cid: Some(inverse_cid),
                        retained_until: None,
                        status: InverseStatus::Available,
                    },
                );
                if let Some(entry) = self.table.get_mut(id) {
                    entry.inverse_cid = Some(inverse_cid);
                }
                Ok(Some(inverse_cid))
            }
            None => {
                self.journal_append(EngineJournalRecord::InverseCompleted {
                    transformation: *id,
                    inverse_cid: None,
                    at,
                })?;
                self.escrow.insert(
                    *id,
                    EscrowRow {
                        transformation: *id,
                        inverse_cid: None,
                        retained_until: None,
                        status: InverseStatus::Unavailable,
                    },
                );
                if let Some(entry) = self.table.get_mut(id) {
                    entry.inverse_cid = None;
                }
                Ok(None)
            }
        }
    }

    /// Journal an abort, move the row, and answer with the state (43 T-10a / T-10c / T-13).
    ///
    /// One helper so that "what does an abort write" (sem: SEM-gx-engine-293) has one answer, and so that `rollback` cannot
    /// be forgotten at a call site: the parameter is required and every caller has to say which of
    /// [`Rollback`]'s facts applies, `None` included.
    fn abort(
        &mut self,
        id: &TransformationId,
        reason: AbortReason,
        rollback: Option<Rollback>,
        at: Timestamp,
    ) -> Result<Lifecycle> {
        self.journal_append(EngineJournalRecord::Aborted {
            transformation: *id,
            reason,
            rollback,
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.rollback = rollback;
        }
        Ok(self.set_state(id, Lifecycle::Aborted(reason), at))
    }

    /// Derive 42 §3.9's `Provenance` for a transformation (**M5-25 adopted (a)**, D-7's third window)
    /// (sem: SEM-gx-engine-294).
    ///
    /// The engine is the producer because it is the only party that saw what the adapter read.
    /// `Provenance::derive_from` does the rest -- including deciding when `intent_digest` is `None`,
    /// which is gx-witness's judgement and not this crate's to re-take.
    ///
    /// # `input_objects` is one object in v0.1, and that is a measurement rather than a stub
    ///
    /// 42 §3.9 asks for "the set of input snapshots the adapter read during plan/verify (including
    /// secondary inputs besides `subject`)" (sem: SEM-gx-engine-295). In v0.1 the engine watches
    /// the adapter read **exactly one**: T-2's
    /// `adapter.snapshot(locator)`. `verify` reads nothing further (T-3's `invert` is handed the
    /// snapshot T-2 took), and 41 §4 gives an adapter no way to report a secondary read. So the
    /// list has one element, and the day an adapter reads two is the day 41 §4 needs a way to say
    /// so -- raised with the version question in **M5H4-4**.
    fn derive_provenance(&self, id: &TransformationId, pre: &ObjectSnapshot) -> Provenance {
        let entry = &self.table[id];
        let version = self
            .adapters
            .get(entry.delta.substrate())
            .map_or_else(String::new, |registered| registered.version.clone());
        Provenance::derive_from(
            &entry.transformation,
            ProvenanceInputs {
                input_objects: vec![*pre.id()],
                environment: Environment {
                    // ASM-10 (single-node operation) (sem: SEM-gx-engine-296) makes it omissible, and an engine that invented a
                    // hostname would be putting an unverifiable claim into a provenance record.
                    host_id: None,
                    adapter_kind: entry.delta.substrate().clone(),
                    // 42 §3.9's example is an MCP session id, which arrives with an API this
                    // milestone does not build (N-01).
                    correlation_id: None,
                    engine_version: env!("CARGO_PKG_VERSION").to_string(),
                    adapter_version: version,
                    // 🔴 **`req/493` §0 / AC-6** — journalled here, before the world moves, because
                    // this is the record M5-25 adopted (a) wrote for exactly that window and
                    // because the rebuild roads cannot re-read the process they are rebuilding.
                    confinement: Some(self.confinement.clone()),
                },
            },
        )
    }

    // -----------------------------------------------------------------------

    /// Move a row's state. The one mutation, so that a reader looking for "who changes state"
    /// (sem: SEM-gx-engine-297) finds
    /// one answer and every caller of it is a journalled transition above.
    ///
    /// 🔴 Takes the moment as well, since hand 6: 43 T-6 measures its two deadlines from "the state
    /// was entered" (sem: SEM-gx-engine-298), and a `since` maintained anywhere but here would be a second answer to when
    /// that was. The value is the `at` of the record that was just appended, so the deadline is a
    /// function of the journal even though the field is not in Σ.
    fn set_state(&mut self, id: &TransformationId, to: Lifecycle, at: Timestamp) -> Lifecycle {
        if let Some(entry) = self.table.get_mut(id) {
            entry.state = to;
            entry.since = at;
            // 🔴 H-04 (`req/182` §1-1, repaired in `req/189`): the escalation ticket lives exactly
            // as long as the row is `Escalated`. Before this line the only writer of `ticket` was
            // T-4c and nothing ever cleared it, so a ruling (`escalation()`), a cancel and a T-6
            // expiry (both `abort()`) left the ticket on the row and `GET /escalations` reported a
            // ruled row as still waiting — forever. Cleared here, in the one place state moves,
            // rather than in each caller, so the invariant "ticket ⇒ Escalated" has one writer.
            // The Σ side (`plan`'s rehydration) applies the same rule when it rebuilds a ticket.
            if to != Lifecycle::Escalated {
                entry.ticket = None;
            }
        }
        to
    }

    /// The `IntentId` a transformation came from.
    ///
    /// 🔴 **T6 condition ① (Σ-shadow), and the key the draft archive is filed under** (`req/38`
    /// §148 ruling 1(iii), lane R2). The table first, then the journal's own fold — the same
    /// fall-through [`Engine::state`] has, and it is what makes a rebuild possible at all: a
    /// restarted process holds no row, so without this it could name the state of a transformation
    /// and not the intent that produced it, and the draft archive is keyed on the intent. 42
    /// §3.13's `Planned` record carries the `IntentId`, so the answer is in the journal whether or
    /// not this process planned anything.
    #[must_use]
    pub fn intent_of(&self, id: &TransformationId) -> Option<IntentId> {
        self.table
            .get(id)
            .map(|e| e.intent_id)
            .or_else(|| self.shadow.row(id).and_then(|r| r.intent_id))
    }

    /// 🔴 **M6-02 adopted (a)** (sem: SEM-gx-engine-299) — the `TransformationId` an intent was
    /// planned into, if it was.
    ///
    /// The inverse of [`Engine::intent_of`], and the thing 44 §0's id-resolution rule needs in order
    /// to exist at all: "commands and endpoints like `gx plan` that specify a target across
    /// Draft/Candidate accept either an `IntentId` or a `TransformationId`'s `gx1:...` value ...
    /// and resolve to the canonical `TransformationId` once `plan()` completes"
    /// (sem: SEM-gx-engine-300). Before this accessor a caller holding an `IntentId` could only
    /// walk [`Engine::transformation_ids`] comparing `intent_of` — the O(n) shape M5H7-3 identified
    /// as a measured decay rather than a theoretical one.
    ///
    /// # 🔴 The rule when the answer is not unique (req/88 §3 Λ3(ii))
    ///
    /// Resolution is a **partial** map, and re-planning can make one intent name more than one
    /// transformation. 43 §8 forces a re-plan when a predecessor commits, and [`Engine::plan`]
    /// permits it "while the row is still where T-2 left it" (sem: SEM-gx-engine-301) — so a second `plan` of the same intent
    /// against a moved world mints a second `TransformationId` while the first row is still in the
    /// table. **This accessor answers with the most recently planned one**, which is the last
    /// `Planned` record in journal order.
    ///
    /// Journal order rather than table order, and the difference is not cosmetic: the table is a
    /// `BTreeMap<TransformationId, _>`, so its order is CID order — content-addressed and therefore
    /// arbitrary with respect to time. "The latest" has to mean "the latest thing that happened"
    /// (sem: SEM-gx-engine-302),
    /// and the journal is the only structure that records happening. The same order is what
    /// [`Engine::open`] replays, so the answer after a restart is the answer before it.
    ///
    /// It answers `None` for a **draft** and that is E-M5-3 rather than an omission: before `plan`
    /// there is no `TransformationId` to resolve to (43 T-1: "a `TransformationId` is not yet
    /// fixed" (sem: SEM-gx-engine-303)),
    /// which is why 44 L101's `gx cancel` from-set could not be satisfied by id-resolution and
    /// **E-M6-1** removed `Draft` from it instead (req/38 §47).
    ///
    /// # 🔴 **DEFECT-891-1** (`req/895` §2) — the paragraph above was right and the type was not
    ///
    /// "Resolution is a **partial** map, and re-planning can make one intent name more than one
    /// transformation" has been in this doc comment since M5, and the backing store was a
    /// `BTreeMap<IntentId, TransformationId>` that could hold exactly one. The two disagreed for
    /// as long as nothing asked, and `undo` is what asked: it mints a second transformation for
    /// one intent **without** re-planning, by putting `parents` inside a `Transformation`'s
    /// identity and leaving it outside an `Intent`'s. The evicted branch became unreachable, and
    /// `gx undo` answered exit 6 `NOT_FOUND` about a transformation holding a signed commit
    /// receipt. The store is a list now; this accessor's own contract — **the most recently
    /// planned one** — is unchanged, and [`Engine::resolved_all`] / [`Engine::resolves_to`] are
    /// how a caller asks the other questions.
    #[must_use]
    pub fn resolved(&self, intent_id: &IntentId) -> Option<TransformationId> {
        self.resolved
            .get(intent_id)
            .and_then(|ids| ids.last())
            .copied()
    }

    /// 🔴 **DEFECT-891-1** (`req/895` §2) — record that `intent_id` was planned into
    /// `transformation`, without evicting whatever it was planned into before.
    ///
    /// The one writer of [`Engine::resolved`]'s backing map on the live path, so that the two
    /// roads that mint a `TransformationId` (`plan` and `undo`) cannot disagree about what
    /// "remembering a resolution" means. It is the same fold [`Engine::open`] replays, which is
    /// what makes the answer after a restart the answer before it.
    ///
    /// Idempotent: 43 T-2's "re-running against the same snapshot yields … the same
    /// `TransformationId`" is the case this must not turn into a second entry.
    fn remember_resolution(&mut self, intent_id: IntentId, transformation: TransformationId) {
        let seen = self.resolved.entry(intent_id).or_default();
        if !seen.contains(&transformation) {
            seen.push(transformation);
        }
    }

    /// 🔴 **DEFECT-891-1** (`req/895` §2) — every transformation this intent was planned into,
    /// in journal order.
    ///
    /// Longer than one element exactly when the same `Intent` was reached by two different
    /// roads to a `Transformation`, and `undo` is the only producer of such a road in v0.1: it
    /// mints `T_u` with `parents = vec![T_o]`, and `parents` is inside a `Transformation`'s
    /// identity and outside an `Intent`'s. Two undos that restore the same bytes at the same
    /// locator under the same context and actor are therefore one intent and two
    /// transformations — a **tree**, and this is the seat that stopped flattening it.
    ///
    /// [`Engine::resolved`] answers the last of these and is the road 44 §0's id-resolution
    /// takes. A caller that needs to know whether a *particular* transformation belongs to an
    /// intent asks [`Engine::resolves_to`] rather than comparing against the last one.
    #[must_use]
    pub fn resolved_all(&self, intent_id: &IntentId) -> &[TransformationId] {
        self.resolved.get(intent_id).map_or(&[], Vec::as_slice)
    }

    /// 🔴 Was `transformation` planned from `intent_id`?
    ///
    /// The predicate `gx-cli`'s `Session::intent_of` wanted. It used to be spelled
    /// `engine.resolved(&id) == Some(transformation)`, which is the same question **only while
    /// an intent has at most one transformation** — and once it has two, that spelling answers
    /// `false` about a committed transformation whose receipt is on disk, which is what
    /// DEFECT-891-1 was seen as: `gx undo <T_u>` → exit 6, `NOT_FOUND`.
    #[must_use]
    pub fn resolves_to(&self, intent_id: &IntentId, transformation: &TransformationId) -> bool {
        self.resolved_all(intent_id).contains(transformation)
    }
}

// ---------------------------------------------------------------------------
// DR-43-1(a) / DR-43-3 -- what an undo is judged against, and the closed table
// of ways it is refused
// ---------------------------------------------------------------------------

/// 🔴 What the caller knows about the world `T_o` left behind (**DR-43-1, adopted (a)**,
/// `req/38` §132 ruling 2; 43 §5.2).
///
/// # Why this is a parameter and not something the engine reads for itself
///
/// The value an undo has to be judged against is `T_o`'s own signed observation of the world right
/// after its apply -- 42 §3.10's `postcondition_fingerprint` -- and the engine cannot obtain it.
/// The journal does not carry it ([`Engine::recover`]'s own note says so: a value produced by
/// `apply` and recorded **nowhere**, M5H5-3), the in-memory receipt is gone after a restart
/// (`Engine::open` leaves the table empty, M5H3-5), and `.gx/receipts/` is req/56 §2's layout,
/// which is gx-cli's to spell. So the party that holds the receipt states what it knows, and this
/// crate judges.
///
/// # Why the absence is a variant rather than an `Option`
///
/// `Option<FingerprintBytes>` would let "we did not look" and "we looked and there is nothing to
/// compare" wear one face, which is the standing refusal of §32 M4H4-2. [`Unobservable`] names
/// *which* absence it is, and every one of its variants is a sentence a caller can be held to.
///
/// **The absence is not a refusal** (`req/38` §123 ruling 1, DR-46-7): a substrate whose position
/// cannot be observed -- a tools-only MCP server has no `resources/read`, so `snapshot` answers
/// `Unreadable` and there is no digest to attest -- is declared, not refused. Turning "we cannot
/// see" into "you may not undo" would make the fail-closed posture punish the honest half of
/// DR-46-7 and would take away the one road tools-only servers have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoWitness {
    /// `T_o`'s signed `postcondition_fingerprint` (42 §3.10), as the expected digest of the
    /// position an undo is about to write over.
    Attested(FingerprintBytes),
    /// 🔴 **R3 / `req/222` H-01, H-02** — no attestation this process is willing to trust, and
    /// this is which trust is missing. The undo is **refused** (fail-closed).
    ///
    /// The variant `req/38` §160 ruling 2 asks for. Before it, every "there is nothing to compare
    /// against" answer was [`UndoWitness::Unobservable`], and `Engine::undo` skipped the CAS for
    /// all of them — so deleting one file under `.gx/receipts/` turned a `409 PRECONDITION_CHANGED`
    /// into a `200` that overwrote a third party's change (`req/222` H-01, measured 3/3), and the
    /// archive's writer was `let _ =` so no attacker was needed to get there.
    ///
    /// The line between this and [`UndoWitness::Unobservable`] is **whose absence it is**: an
    /// adapter that cannot read a position (a tools-only MCP server: `req/38` §123 ruling 1) is a
    /// property of the substrate and is declared; a receipt that is absent, unreadable, unsigned or
    /// about another transformation is a property of the **evidence**, and evidence that is not
    /// there is not a reason to proceed.
    Missing(WitnessMissing),
    /// There is nothing to compare against, and this is which nothing it is.
    Unobservable(Unobservable),
}

impl UndoWitness {
    /// 🔴 **DR-46-45 (`req/973` §B-1)** — what a receipt may say about this witness, or `None` if
    /// this witness produces no receipt.
    ///
    /// `Missing` is a **refusal** (R3): it mints no `TransformationId`, appends no `Planned` and
    /// issues no receipt, so there is no signed payload for it to appear in. Returning `None` here
    /// is that fact, not the three-valued discipline being folded into two — the third value lives
    /// in the refusal surface (exit 3 / HTTP 409 `PRECONDITION_CHANGED`) where `req/38` §132
    /// ruling 2 put it. `crates/gx-engine/tests/r973_undo_attestation.rs` asserts the mapping
    /// arm by arm so a fourth variant cannot be added without a decision about this function.
    #[must_use]
    pub fn disposition(&self) -> Option<UndoDisposition> {
        match self {
            UndoWitness::Attested(_) => Some(UndoDisposition::Attested),
            UndoWitness::Unobservable(why) => Some(UndoDisposition::Unobservable {
                reason: why.reason().to_string(),
            }),
            UndoWitness::Missing(_) => None,
        }
    }

    /// 🔴 **DR-46-45 (`req/973` §B-1)** — the word every surface prints for this witness.
    ///
    /// Minted here, once, so that CLI stdout, HTTP's `witness` field and the signed payload cannot
    /// drift into three formatters that agree only by inspection. `gx_api`'s `witness_word` and
    /// `gx_cli`'s undo stdout both call this; the receipt reaches the same string through
    /// [`UndoDisposition::word`], and the `Missing` arm — which reaches no receipt — is the one this
    /// function answers that [`UndoDisposition::word`] cannot.
    #[must_use]
    pub fn word(&self) -> String {
        match (self, self.disposition()) {
            (_, Some(disposition)) => disposition.word(),
            // The refusal surface's own word. Kept here rather than at the call sites because a
            // second spelling of it is exactly what this function exists to prevent.
            (UndoWitness::Missing(why), None) => format!("missing:{}", why.reason()),
            // A variant added later without deciding what a receipt may say about it lands here.
            // Named rather than guessed at, and not a panic: 41 §6 counts a panic as a bug, and a
            // word invented for an undecided case would be the "green that lies" this workspace
            // keeps measuring.
            (other, None) => format!("undecided:{other:?}"),
        }
    }
}

/// Which trust an [`UndoWitness::Missing`] lacks (**R3**, `req/38` §160 ruling 2).
///
/// Every variant is a **refusal**, and the refusal is one row of [`UNDO_REFUSALS`]
/// (`witness-missing`) carrying 44's existing `PRECONDITION_CHANGED` / exit 3 / HTTP 409 — no new
/// exit number and no new `gx_code` is minted here (`req/38` §132 ruling 2's standing rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessMissing {
    /// No commit receipt is archived for `T_o`. Since R3 a commit whose receipt the archive would
    /// not take is a **failed** commit (`req/222` H-01(a)), so this is somebody having removed one.
    NoReceipt,
    /// The archived receipt's payload would not decode.
    Unreadable,
    /// The archived receipt's DSSE signature does not verify under the key the receipt names
    /// (`req/222` H-02; 🔴 **R4** widened *which* key that is — see [`WitnessMissing::UnknownKey`]).
    Unsigned,
    /// 🔴 **R4 / `req/225` H-02** — the receipt names a signing key this deployment holds no
    /// public key for, so its signature cannot be checked at all.
    ///
    /// A **refusal**, and the choice is `req/38` §160 ruling 2's rule applied one step further
    /// out: "no evidence, no undo". A receipt whose signature nobody here can check is not
    /// evidence, and it is emphatically not [`Unobservable`] — that word is reserved for the
    /// **substrate's** inability to observe (`req/38` §123 ruling 1), and this is gx's inability
    /// to verify. Reporting it as `Unsigned` would be the other available lie: it would send an
    /// operator looking for tampering when what they have is a key store missing a key.
    UnknownKey,
    /// The archived receipt is about **another** transformation (`req/222` H-02: a receipt copied
    /// over `T_o`'s file name attested a world that had moved).
    WrongSubject,
    /// This deployment keeps no receipt archive at all, so no undo on it can be checked.
    NoArchive,
}

impl WitnessMissing {
    /// The sentence a refusal prints, and the one a reader of [`UNDO_REFUSALS`] looks up.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            WitnessMissing::NoReceipt => "no archived commit receipt for the original",
            WitnessMissing::Unreadable => "the archived commit receipt would not decode",
            WitnessMissing::Unsigned => {
                "the archived commit receipt's signature does not verify under the key it names"
            }
            WitnessMissing::UnknownKey => {
                "the archived commit receipt names a signing key this deployment does not hold"
            }
            WitnessMissing::WrongSubject => {
                "the archived commit receipt is about another transformation"
            }
            WitnessMissing::NoArchive => "this deployment keeps no receipt archive",
        }
    }

    /// 🔴 **R5 / `req/227` M-07** — what the operator of *this* deployment can do about it.
    ///
    /// Five of the six absences are a missing or wrong document under `.gx/receipts/`, and the
    /// answer is to put the right one back. [`WitnessMissing::NoArchive`] is not one of them: the
    /// deployment declared that it keeps no receipts, so there is no directory to restore into and
    /// filling one would change nothing — `NoArchive::load_commit` answers `None` by construction.
    /// `req/227` M-07 measured the old sentence sending an embedder's operator to
    /// `.gx/receipts/`, which that project does not have.
    ///
    /// # 🔴 What is **not** changed here, and why it is a clause rather than a repair
    ///
    /// `req/227` M-07 also names the face on the other side: on such a deployment `commit`
    /// answers `200` with a signed receipt and says nothing about the undo that can never run.
    /// Making the commit *fail* would turn every embedder that has deliberately chosen `NoArchive`
    /// into a deployment that cannot commit at all, and saying it in the commit's answer is a
    /// field added to 44 §2.2's composite — a wire surface addition, which is a DR and is refused
    /// to a repair lane (`req/38` §132 ruling 2's standing rule). So the difference is written
    /// where the difference is decided: 43 §5.2's R5 note says that an archive which **refuses** a
    /// receipt fails the commit (R3, unchanged) while an archive that is **not kept** commits and
    /// can never undo, and that the second is a deployment's standing choice rather than an
    /// accident. Raised for a ruling rather than smoothed over.
    #[must_use]
    pub const fn remedy(self) -> &'static str {
        match self {
            WitnessMissing::NoReceipt
            | WitnessMissing::Unreadable
            | WitnessMissing::Unsigned
            | WitnessMissing::UnknownKey
            | WitnessMissing::WrongSubject => {
                "restore the commit receipt under .gx/receipts/ or accept that this change cannot \
                 be taken back by gx"
            }
            WitnessMissing::NoArchive => {
                "this deployment keeps no receipt archive, so there is nothing to restore: an undo \
                 needs the signed postcondition of the commit it is undoing, and this server was \
                 built without a place to keep one (gx_api::AppState::with_archive is where a \
                 deployment declares one). Every commit made here is final as far as gx is \
                 concerned"
            }
        }
    }
}

/// Which absence an [`UndoWitness::Unobservable`] is (**DR-46-7**'s vocabulary at this seam).
///
/// Every variant is a *declaration*: the undo proceeds exactly as it did before DR-43-1, and the
/// caller is expected to say so out loud (gx-cli writes a `gx undo settle: skipped (...)` line).
/// Silence is the one thing this type exists to prevent.
///
/// # 🔴 **R3 (`req/38` §160 ruling 2)** — three variants are no longer produced, and are kept
///
/// `NoReceipt`, `ReceiptUnreadable` and `NoArchive` moved to [`WitnessMissing`], where the same
/// three absences are **refusals**. They are kept here rather than removed for the reason the
/// no-delete rule exists: a reader who finds `Unobservable::NoReceipt` in a log line from before
/// this repair, or in `req/216`/`req/222`, has to be able to look it up and find out that it used
/// to mean "the CAS was skipped" and now cannot happen. Nothing in this workspace constructs them
/// after R3 (`grep -rn "Unobservable::NoReceipt"` is the check), and the two live variants are
/// `NoPostcondition` (a receipt that honestly attests nothing was observed -- `req/38` §123 ruling
/// 1's tools-only face) and `LaunchAlreadyDecided`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unobservable {
    /// No commit receipt is archived for `T_o` -- the pair gotcha95 asks operators to keep was not
    /// kept, or the commit predates the receipt store.
    ///
    /// 🔴 **Retired by R3**: [`WitnessMissing::NoReceipt`] is the answer now, and it refuses.
    NoReceipt,
    /// A receipt exists and carries no `postcondition_fingerprint`: nothing was applied (a
    /// `VerdictReceipt`), or the apply produced no observation.
    NoPostcondition,
    /// The archived receipt's payload would not decode.
    ///
    /// 🔴 **Retired by R3**: [`WitnessMissing::Unreadable`] is the answer now, and it refuses.
    ReceiptUnreadable,
    /// The launch's answer is already fixed by another row of [`UNDO_REFUSALS`] (the original is
    /// not `Committed`, or the escrow is not `Available`), so no digest was fetched. Comparing
    /// would answer a question nobody is asking.
    LaunchAlreadyDecided,
    /// This deployment keeps no receipt archive at all.
    ///
    /// 🔴 **Retired by R3**: [`WitnessMissing::NoArchive`] is the answer now, and it refuses. What
    /// that costs a deployment that genuinely keeps none is written down where the cost lands --
    /// `gx_api::NoArchive` and 43 §5.2's note -- rather than paid silently at every undo.
    NoArchive,
}

impl Unobservable {
    /// The sentence a caller prints, and the one a reader of [`UNDO_REFUSALS`] looks up.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Unobservable::NoReceipt => "no archived commit receipt for the original",
            Unobservable::NoPostcondition => "the commit receipt carries no postcondition",
            Unobservable::ReceiptUnreadable => "the archived commit receipt would not decode",
            Unobservable::LaunchAlreadyDecided => "the launch is already answered by another row",
            Unobservable::NoArchive => "this deployment keeps no receipt archive",
        }
    }
}

/// 🔴 **DR-43-3** -- the closed table of ways `undo` refuses *before it begins*, as a type.
///
/// # Where the shape comes from
///
/// `req/207` §3 read `aider/commands.py:553-625` (Apache-2.0, observation only, no code taken) and
/// measured the one property worth having: an undo that is not safe is refused **by kind**, with a
/// different sentence per kind, and nothing is guessed. Its seven git-shaped preconditions do not
/// translate -- they are written in commit/parent/origin/dirty -- but the *form* does, and
/// [`UNDO_REFUSALS`] is that form with our own material in it and with the rows we cannot fill
/// marked rather than dropped.
///
/// # Why it converts into [`crate::Error`] instead of replacing it
///
/// Every branch below already had an answer at the surface -- 44 §1.2 gives `gx undo` `0/1/3/5/6`
/// and §2.3 gives the HTTP side its `gx_code` -- and `req/38` §132 ruling 2 mints no new exit
/// number. So this type is the *judgement* and [`crate::Error`] stays the *carrier*: one vocabulary
/// at the wire, one table in the source, and [`UNDO_REFUSALS`] is where a reader sees which is
/// which. The only branch that had no carrier is [`UndoRefusal::WorldMoved`], and it takes the code
/// 44 already has for a CAS that failed (`PRECONDITION_CHANGED`, exit 3, HTTP 409) rather than a
/// new one.
/// 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — what one read of the object, taken
/// immediately before 43 T-10c's compensation, found.
///
/// # Why three values and not a `bool`
///
/// Because the three answers a compensation can get are three, not two, and the same arithmetic
/// that made [`crate::store::Rollback`] grow a fourth word applies on the way in. A `bool` would
/// have to fold *the substrate would not say where it is* into either "there is nothing to take
/// back" (which loses a real effect) or "take it back" (which fires an absolute inverse into a
/// world nobody has looked at). Both are the error `req/324` §5(d) exists to stop, one direction
/// each.
///
/// 🔴 **[`CompensationVerdict::TakeBackFrom`] carries the fingerprint**, and that is the whole
/// repair rather than a convenience. The state to guard the compensation on is *where our failed
/// apply left the object*, not *where the transformation started* — a guard on `fp0` cannot tell
/// our own landed apply from somebody else's write and has to sacrifice one of them.
///
/// Deliberately **private and not a component of Σ**: it decides one branch inside one call and is
/// never serialised. What survives the call is the [`NotAttemptedBecause`] the branch records,
/// which is itself an annotation on this process's account rather than state (see its own doc).
#[derive(Clone, Debug)]
enum CompensationVerdict {
    /// The object was read and it is at the fingerprint this transformation started from: the
    /// forward `apply` moved nothing, so there is no effect to compensate and no inverse is sent.
    NothingToTakeBack,
    /// The object was read and the forward `apply` **did** move it. The fingerprint is where it
    /// left it, and the compensation is guarded on this value rather than on `fp0`.
    TakeBackFrom(Fingerprint),
    /// The read could not be taken: `snapshot` refused, `precondition` refused, or the two
    /// fingerprints were not comparable.
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoRefusal {
    /// This ledger holds no such transformation. aider's (1) and (3) at once: there is no
    /// repository, and the effect is not ours to take back.
    NotOurs,
    /// The shadow has the row and this process holds no body for it (a restart; the draft archive
    /// that rebuilds a `Transformation` is DR-43-2 lane R2).
    NoBody {
        /// The state the shadow reports.
        state: &'static str,
    },
    /// The original is not `Committed`. `Superseded` is the common case and means "already undone"
    /// one row over from [`UndoRefusal::AlreadyUndone`]: the edge was drawn, so the world is
    /// already back.
    NotCommitted {
        /// The state the row is in.
        state: &'static str,
    },
    /// No escrow row exists for the original at all.
    NoEscrow,
    /// 42 §3.12's `Unavailable`: `invert` answered `None`, so there is no inverse to apply.
    InverseUnavailable,
    /// 42 §3.12's `Pending`: two-phase escrow's completion never journalled its outcome, which is
    /// a crash's trace. A partial inverse is not an executable one.
    InversePending,
    /// 42 §3.12's `Consumed`: another undo already took this inverse. This is what makes "only
    /// once" a fact rather than a hope.
    AlreadyUndone,
    /// No adapter is registered for the original's substrate, so nothing can read or write it.
    NoAdapter {
        /// The substrate nobody registered.
        substrate: String,
    },
    /// 🔴 **DR-43-1(a)** -- the world moved after `T_o` committed, so the escrowed inverse would
    /// overwrite a change this system cannot account for. aider's (5) "has uncommitted changes",
    /// with the digest of a position where aider has `git status`.
    WorldMoved {
        /// `T_o`'s signed observation (42 §3.10).
        expected: FingerprintBytes,
        /// What the adapter reads now.
        found: FingerprintBytes,
        /// 42 §3.5's scope -- the *readable* half, and the only half a refusal prints (the canon
        /// fixes no readable spelling for a fingerprint: `gx_core::FingerprintBytes`'s `Debug` is
        /// deliberately opaque, so a message that printed one would be minting a spelling).
        scope: String,
    },
    /// `T_u` was already minted and has left `Candidate` (H-05's guard: a `TransformationId` is a
    /// CID over the identity view, so a second `undo(T_o)` re-mints the same id).
    AlreadyPlanned {
        /// Where `T_u` already is.
        state: &'static str,
    },
    /// 🔴 **R3 / `req/222` H-01, H-02** — the CAS could not run, so it is answered as a CAS that
    /// failed rather than skipped.
    ///
    /// `req/38` §160 ruling 2: "the witness being absent is a separate variant = refuse". The world
    /// may or may not have moved; what is certain is that this process cannot tell, and an undo
    /// that cannot tell writes over whatever is there.
    WitnessMissing {
        /// Which trust is missing.
        missing: WitnessMissing,
    },
}

impl UndoRefusal {
    /// The row of [`UNDO_REFUSALS`] this refusal is, by name.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            UndoRefusal::NotOurs => "not-ours",
            UndoRefusal::NoBody { .. } => "no-body",
            UndoRefusal::NotCommitted { .. } => "not-committed",
            UndoRefusal::NoEscrow => "no-escrow",
            UndoRefusal::InverseUnavailable => "inverse-unavailable",
            UndoRefusal::InversePending => "inverse-pending",
            UndoRefusal::AlreadyUndone => "already-undone",
            UndoRefusal::NoAdapter { .. } => "no-adapter",
            UndoRefusal::WorldMoved { .. } => "world-moved",
            UndoRefusal::AlreadyPlanned { .. } => "already-planned",
            UndoRefusal::WitnessMissing { .. } => "witness-missing",
        }
    }

    /// The refusal as this crate's carrier, with the id the caller named.
    ///
    /// One `match`, no `_` arm: a variant added tomorrow stops this from compiling, which is the
    /// same shape [`crate::Error::kind`] has and the reason [`UNDO_REFUSALS`] cannot quietly fall
    /// behind the type.
    #[must_use]
    pub fn into_error(self, original: &TransformationId) -> Error {
        let id = format!("{original:?}");
        match self {
            UndoRefusal::NotOurs => Error::NotFound {
                what: "transformation",
                id,
            },
            UndoRefusal::NoBody { state } => Error::InvalidState {
                id,
                state,
                attempted: "undo a transformation this process holds no body for (the journal has \
                            the row, the draft archive that rebuilds the Transformation is DR-43-2 \
                            lane R2 / req/190 §4-1 L2)",
            },
            UndoRefusal::NotCommitted { state } => Error::InvalidState {
                id,
                state,
                attempted: "undo",
            },
            UndoRefusal::NoEscrow => Error::NotFound {
                what: "escrowed inverse",
                id,
            },
            UndoRefusal::InverseUnavailable => Error::NotFound {
                what: "escrowed inverse (42 §3.12 Unavailable: `invert` answered None)",
                id,
            },
            UndoRefusal::InversePending => Error::InvalidState {
                id,
                state: "Committed",
                attempted: "undo an inverse whose completion never finished (Pending); recovery \
                            completes it from the journalled observation or folds it",
            },
            UndoRefusal::AlreadyUndone => Error::InvalidState {
                id,
                state: "Superseded",
                attempted: "undo an inverse another transformation already consumed",
            },
            UndoRefusal::NoAdapter { substrate } => Error::NotFound {
                what: "adapter",
                id: substrate,
            },
            UndoRefusal::WorldMoved {
                expected,
                found,
                scope,
            } => Error::WorldMoved {
                id,
                expected,
                found,
                scope,
            },
            UndoRefusal::AlreadyPlanned { state } => Error::InvalidState {
                id,
                state,
                attempted: "undo (a second undo of the same commit re-mints this T_u, and T_u has \
                            already left Candidate; 43 T-2's re-plan rule)",
            },
            UndoRefusal::WitnessMissing { missing } => Error::WitnessMissing {
                id,
                reason: missing.reason(),
                remedy: missing.remedy(),
            },
        }
    }
}

/// One row of 43 §5.2's refusal table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UndoRefusalRow {
    /// [`UndoRefusal::reason`], or -- for a row this system does not judge -- the name the row
    /// would have.
    pub reason: &'static str,
    /// What the judgement is made of.
    pub material: &'static str,
    /// The `gx_code` the surface answers with (44 §2.3, or its ruled additions). `"none"` for a
    /// row nothing is answered about.
    pub gx_code: &'static str,
    /// The CLI exit status (44 §1.4). **No number is minted here** (`req/38` §132 ruling 2); `0`
    /// marks a row with no answer at all.
    pub cli_exit: u8,
    /// The HTTP status 44 pairs with `gx_code`; `0` marks a row with no answer at all.
    pub http_status: u16,
    /// Which of `aider/commands.py:553-625`'s seven preconditions this row answers, or why we have
    /// no row for it (`req/207` §3-1). Kept because a taxonomy that dropped the rows it cannot
    /// fill would be claiming a completeness nobody measured.
    pub aider: &'static str,
    /// The test that measures the refusal, so that a row cannot become a sentence nobody re-reads.
    pub test: &'static str,
    /// `false` for a row this system declares rather than judges.
    pub judged: bool,
}

/// 🔴 **43 §5.2's table** -- thirteen rows: eleven this engine judges and two it declares it does
/// not (twelve/ten before **R3**, which added `witness-missing`).
///
/// The two unjudged rows are the honest half. `req/207` §3-4 asked for "our side of aider's seven,
/// with the rows that do not fill marked rather than dropped", and these are they.
///
/// `crates/gx-cli/tests/undo_cas_e2e.rs` is where the table stops being a table: it asserts that
/// every [`UndoRefusal`] variant owns exactly one row, that every row's `gx_code` is one 44 already
/// declares (`gx_api::gx_code::GX_CODES` or `RULED_ADDITIONS`), and that each row's status pair
/// agrees with that declaration. gx-engine cannot make that comparison itself -- it sits below both
/// gx-cli and gx-api -- which is why the table is `pub` and the probe is one crate up.
pub const UNDO_REFUSALS: [UndoRefusalRow; 13] = [
    UndoRefusalRow {
        reason: "not-ours",
        material: "the state table and the shadow both answer `None` for the named id",
        gx_code: "NOT_FOUND",
        cli_exit: 6,
        http_status: 404,
        aider: "(1) no repository + (3) the last commit was not made by us -- one row here, because \
                a transformation absent from this ledger is exactly \"not ours\"",
        test: "undo_cas_e2e::an_id_this_ledger_never_committed_is_refused_as_not_ours",
        judged: true,
    },
    UndoRefusalRow {
        reason: "no-body",
        material: "the shadow holds the row's state and this process holds no `Transformation`",
        gx_code: "INVALID_STATE",
        cli_exit: 2,
        http_status: 409,
        aider: "no counterpart -- aider has no restart to survive (its provenance set is process \
                memory, `aider_commit_hashes`)",
        test: "serve_runtime_e2e::the_runtime_survives_a_restart_and_a_neighbour (undo arm)",
        judged: true,
    },
    UndoRefusalRow {
        reason: "not-committed",
        material: "the row's lifecycle state",
        gx_code: "INVALID_STATE",
        cli_exit: 2,
        http_status: 409,
        aider: "(2) there is no previous commit",
        test: "undo_cas_e2e::an_undo_of_an_undo_is_refused_by_name",
        judged: true,
    },
    UndoRefusalRow {
        reason: "no-escrow",
        material: "42 §3.12's escrow row is absent",
        gx_code: "NOT_FOUND",
        cli_exit: 6,
        http_status: 404,
        aider: "no counterpart -- aider re-derives the inverse from git rather than escrowing one",
        test: "two_phase_escrow::an_undo_of_a_pending_escrow_is_refused_rather_than_fired",
        judged: true,
    },
    UndoRefusalRow {
        reason: "inverse-unavailable",
        material: "42 §3.12's `Unavailable` -- `invert` answered `None`",
        gx_code: "NOT_FOUND",
        cli_exit: 6,
        http_status: 404,
        aider: "(6) the file was not in the previous commit -- the same fact from the other end: \
                there is nothing to put back",
        test: "two_phase_escrow::a_folded_completion_is_unavailable_and_the_receipt_says_so",
        judged: true,
    },
    UndoRefusalRow {
        reason: "inverse-pending",
        material: "42 §3.12's `Pending` -- a crash between the call and its observation",
        gx_code: "INVALID_STATE",
        cli_exit: 2,
        http_status: 409,
        aider: "no counterpart -- two-phase escrow is ours",
        test: "two_phase_escrow::an_undo_of_a_pending_escrow_is_refused_rather_than_fired",
        judged: true,
    },
    UndoRefusalRow {
        reason: "already-undone",
        material: "42 §3.12's `Consumed`",
        gx_code: "INVALID_STATE",
        cli_exit: 2,
        http_status: 409,
        aider: "no counterpart -- aider's undo is not itself a commit it can consume",
        test: "undo_cmd::ac_054_undo_returns_the_substrate_to_what_it_was (second undo arm)",
        judged: true,
    },
    UndoRefusalRow {
        reason: "no-adapter",
        material: "the substrate registry",
        gx_code: "NOT_FOUND",
        cli_exit: 6,
        http_status: 404,
        aider: "no counterpart -- one substrate, compiled in",
        test: "defaults::a_git_intent_reaches_the_git_adapter_rather_than_an_empty_registry",
        judged: true,
    },
    UndoRefusalRow {
        reason: "world-moved",
        material: "`T_o`'s signed `postcondition_fingerprint` (42 §3.10) against \
                   `adapter.precondition(adapter.snapshot(locator))` taken now",
        gx_code: "PRECONDITION_CHANGED",
        cli_exit: 3,
        http_status: 409,
        aider: "(5) the target has uncommitted changes -- \"stash them before undoing\"",
        test: "undo_cas_e2e::a_third_party_write_after_the_commit_refuses_the_undo",
        judged: true,
    },
    UndoRefusalRow {
        reason: "already-planned",
        material: "the state of the `T_u` this call would re-mint (H-05's guard)",
        gx_code: "INVALID_STATE",
        cli_exit: 2,
        http_status: 409,
        aider: "no counterpart -- aider mints no identifier for the undo",
        test: "audit_v04l_engine_repairs (the H-05 arms)",
        judged: true,
    },
    UndoRefusalRow {
        reason: "witness-missing",
        material: "the `.gx/receipts/` commit receipt for `T_o`: present, decodable, signed by \
                   this project's key, and about `T_o` -- all four, or none of the CAS runs",
        gx_code: "PRECONDITION_CHANGED",
        cli_exit: 3,
        http_status: 409,
        aider: "no counterpart -- aider re-derives the previous state from git, so it has no \
                separate artefact whose absence could silently disable the check. That absence is \
                exactly what `req/222` H-01 measured here",
        test: "serve_runtime_r3::a_deleted_commit_receipt_refuses_the_undo_instead_of_firing_it",
        judged: true,
    },
    UndoRefusalRow {
        reason: "downstream-dependent",
        material: "**not judged separately.** A later *committed* transformation on the same \
                   locator has already moved the world, so `world-moved` answers it. A later commit \
                   that moved the world and moved it back again is not caught, and a commutation \
                   check over committed rows is a DR rather than this lane's work",
        gx_code: "PRECONDITION_CHANGED",
        cli_exit: 3,
        http_status: 409,
        aider: "(7) the commit has already been pushed to origin",
        test: "undo_cas_e2e::the_table_declares_the_two_rows_it_does_not_judge",
        judged: false,
    },
    UndoRefusalRow {
        reason: "ambiguous-predecessor",
        material: "**not judged.** 43 §5 makes `T_u.parents` name exactly one original, so an undo \
                   has no \"which parent\" question to answer",
        gx_code: "none",
        cli_exit: 0,
        http_status: 0,
        aider: "(4) the commit has more than one parent (a merge)",
        test: "undo_cas_e2e::the_table_declares_the_two_rows_it_does_not_judge",
        judged: false,
    },
];

// ---------------------------------------------------------------------------
// 🔴 req/824 A5 — the observation road
// ---------------------------------------------------------------------------
//
// How an attach-source's report becomes an **ordinary candidate** on the existing
// candidate → verify → canonicalize → commit road (R-1: one road, not a second pipeline).
// `Engine::ingest_observation` builds an `Intent` on the engine-internal substrate below and
// drives 43's own T-1/T-2; nothing here writes a `Lifecycle`, constructs a `Verdict`, or mints a
// receipt kind, and the returned id is a candidate every existing surface answers about
// unchanged. **No new journal kind is added by this atom** — the candidate road's own records
// (`DraftCreated`, `Planned`, `VerifyStarted`, `Verdict`, …) are the observation's records, so
// gx-api's `EVENT_MAP` does not move and `MAP_COVERS_THE_JOURNAL` keeps proving the count it
// already proves (req/824 A5's EVENT_MAP duty, discharged by statement).
//
// # What this substrate is, and what it is NOT (SS273)
//
// `ObservationRoad` is a `SubstrateAdapter` over **our own observation-record space**: the
// per-scope chain of committed observation records this engine holds. `apply` advances that
// chain — a write to *our* state, never to the platform the record is about. It is exactly not
// `gx-adapter-vercel`, which req/824 §2 kills by name: the platform an attach-source reports
// about is not writable by this system and never will be, and this adapter never touches it.
// The undo of a committed observation is refused with the **typed** engine refusals
// (`InverseNotExecutableAtSubstrate` / `AppendOnlyClass`, req/824 A1/A3) inside `Engine::undo`,
// before any inverse would execute.
//
// # How the third state is reached without touching the gate
//
// A chain gap (the source's `prev_ref` does not continue what we hold) makes `invert` answer
// **`None`**: the inverse of "append to a chain whose prior we do not hold" cannot be
// constructed, because record-level escrow of an unknown prior would be a fabricated chain.
// E-M3-4 then does the rest — nothing refused + `invert_available == false` ⇒ `Escalate` — so
// the gap reaches `Escalated`, raises a ticket, and is reachable at `GET /escalations` through
// machinery that already existed. The gate is not modified and no second escalation rule is
// minted (req/824 §5-Q6: Escalate stays a third state, asserted as its own variant).
//
// # The policy seat, declared rather than silently decided
//
// Cedar is default-deny and every shipped pack scopes its permit to its own substrate, so **no
// shipped pack permits `custom:observation` today**: a deployment that ingests observations
// composes its own permit statement into its policy set (gx-api's observation test bed does
// exactly that). A shipped observation pack is deliberately NOT added by this atom: the default
// policy set and the default adapter registry are decided **as a pair** (req/38 §60), and A5
// must not decide that pair as a side effect. Declared in docs/LIMITS.md's A5 row.
//
// # Restart honesty (declared delta, req/824 §0 protocol)
//
// The chain heads and the `observation_id` replay map are in-memory state of this road, like the
// A4 registry one membrane up: they do not survive a restart. The *candidates* do — they are in
// the journal — so what a restart costs is the replay short-circuit and the chain-continuity
// memory, not the records. A rebuild from the journal is deferred to the atom that needs it,
// with this sentence as the marker.

/// The one spelling of the observation substrate's name. `SubstrateKind::Custom` carries it, and
/// the policy layer's `substrate_tag` renders it `custom:observation` — the string a permit must
/// compare `resource.substrate` against.
pub const OBSERVATION_SUBSTRATE: &str = "observation";

/// The [`SubstrateKind`] every observation intent, delta and fingerprint carries.
#[must_use]
pub fn observation_substrate() -> SubstrateKind {
    SubstrateKind::Custom(OBSERVATION_SUBSTRATE.to_string())
}

/// Whether a substrate is the observation road's.
#[must_use]
pub fn is_observation_substrate(kind: &SubstrateKind) -> bool {
    matches!(kind, SubstrateKind::Custom(name) if name == OBSERVATION_SUBSTRATE)
}

/// What an observation delta's payload carries, canonical CBOR through gx-canon (41 §6: every
/// canonical encode goes through gx-canon only). `held_at_plan` is the chain head this engine
/// held when the delta was planned — embedded so [`ObservationRoad::invert`]'s answer is a pure
/// function of `(delta, pre)` and a replay hands back the recorded answer.
#[derive(Clone, Debug, serde::Deserialize, Serialize)]
pub(crate) struct ObservationDelta {
    pub class: ObservationClass,
    pub source: String,
    pub observation_id: String,
    pub scope: String,
    pub prev_claimed: Option<String>,
    pub held_at_plan: Option<String>,
    pub record: ObservationRecord,
}

/// The inverse's grammar: put the chain head for `scope` back to `head`. Constructed by
/// [`ObservationRoad::invert`] as the record-level escrow body; never executed in v0.1 —
/// [`Engine::undo`] refuses an observation undo with the typed refusal before any apply.
#[derive(Clone, Debug, serde::Deserialize, Serialize)]
struct ObservationRestore {
    restore: String,
    head: Option<String>,
}

/// The chain-continuity rule, one spelling for all four classes — the same three-way shape as
/// [`gx_core::EnvsetFingerprint::admit`]'s chain arm: continuous when nothing was claimed and
/// nothing is held, or when the claim equals the holding; a gap otherwise.
#[must_use]
fn chain_continuous(claimed: Option<&str>, held: Option<&str>) -> bool {
    match (claimed, held) {
        (None, None) => true,
        (Some(c), Some(h)) => c == h,
        _ => false,
    }
}

/// A remembered ingest, for the `observation_id` idempotency contract (`req/824` §2-1: a CI job
/// that retries re-POSTs the same operation for every class; the retry must be an idempotent
/// no-op, not a second candidate).
#[derive(Clone, Debug)]
struct RememberedIngest {
    id: TransformationId,
    chain_ref: String,
    gap: bool,
}

#[derive(Default)]
struct ObservationRoadState {
    /// scope → the chain head: the delta reference CID (as text) of the last **committed**
    /// observation on that scope. This is the value a source must quote as `prev_ref` for its
    /// next record, and the ingest response hands it out as `chain_ref`.
    chains: BTreeMap<String, String>,
    /// `(source, observation_id)` → the candidate already minted for it.
    replays: BTreeMap<(String, String), RememberedIngest>,
}

/// 🔴 The engine-internal observation adapter (the section header above for what it is and is
/// not). `Send + Sync` because the engine requires it of every adapter; the lock is uncontended
/// in practice — the engine itself sits behind its caller's lock.
#[derive(Default)]
pub struct ObservationRoad {
    state: Mutex<ObservationRoadState>,
}

impl std::fmt::Debug for ObservationRoad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ObservationRoad")
    }
}

impl ObservationRoad {
    /// The committed chain head for a scope, if any.
    fn chain_head(&self, scope: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.chains.get(scope).cloned())
    }

    /// The candidate already minted for `(source, observation_id)`, if any.
    fn replay(&self, source: &str, observation_id: &str) -> Option<RememberedIngest> {
        self.state.lock().ok().and_then(|s| {
            s.replays
                .get(&(source.to_string(), observation_id.to_string()))
                .cloned()
        })
    }

    /// Remember an ingest for the replay contract.
    fn remember(&self, source: &str, observation_id: &str, entry: RememberedIngest) {
        if let Ok(mut s) = self.state.lock() {
            s.replays
                .insert((source.to_string(), observation_id.to_string()), entry);
        }
    }

    /// The chain-state digest for a scope: what `snapshot` and `precondition` both report, so
    /// T-10a's CAS compares plan-time state with commit-time state and refuses when another
    /// observation committed on this scope in between.
    fn state_digest(&self, scope: &str) -> Cid {
        let held = self.chain_head(scope);
        let held_bytes: &[u8] = match &held {
            Some(h) => h.as_bytes(),
            // A marker no chain head can collide with: heads are CID texts, never NUL-prefixed.
            None => b"\x00absent",
        };
        cid::mint(
            cid::Domain::Leaf,
            &[b"gx-observation-chain", scope.as_bytes(), held_bytes],
        )
    }
}

/// The locator grammar: `observation://<scope>?obs=<source>:<observation_id>`. The scope half is
/// what chain state is keyed on; the query half makes each observation's object identity its
/// own, so two observations on one scope are different `Subject`s and 43 §8's commutation hold
/// does not serialise a bed of independent ingests.
fn observation_locator(scope: &str, source: &str, observation_id: &str) -> String {
    format!("observation://{scope}?obs={source}:{observation_id}")
}

/// The scope half of a locator this road minted.
fn observation_scope_of(locator: &str) -> gx_substrate::Result<String> {
    let rest =
        locator
            .strip_prefix("observation://")
            .ok_or_else(|| gx_substrate::Error::Unreadable {
                locator: locator.to_string(),
                detail: "not an observation locator (the grammar is \
                     observation://<scope>?obs=<source>:<id>)"
                    .to_string(),
            })?;
    Ok(rest.split('?').next().unwrap_or(rest).to_string())
}

fn decode_observation_delta(payload: &[u8]) -> gx_substrate::Result<ObservationDelta> {
    cbor::decode(payload).map_err(|e| gx_substrate::Error::Unreadable {
        locator: "<observation delta payload>".to_string(),
        detail: format!("not this road's grammar: {e}"),
    })
}

impl SubstrateAdapter for ObservationRoad {
    fn kind(&self) -> SubstrateKind {
        observation_substrate()
    }

    /// The chain state for the locator's scope, as an object. Two-phase id mint, the fs
    /// adapter's own shape: the placeholder id is outside the projection, so it cannot reach the
    /// digest.
    fn snapshot(&self, locator: &str) -> gx_substrate::Result<ObjectSnapshot> {
        let scope = observation_scope_of(locator)?;
        let digest = self.state_digest(&scope);
        let placeholder = ObjectSnapshot::new(
            ObjectId(Cid([0u8; 32])),
            observation_substrate(),
            locator.to_string(),
            digest,
            ReprKind::Bytes,
        );
        let id = cid::compute(&placeholder).map_err(|e| gx_substrate::Error::NotDigestible {
            detail: e.to_string(),
        })?;
        Ok(ObjectSnapshot::new(
            ObjectId(id),
            observation_substrate(),
            locator.to_string(),
            digest,
            ReprKind::Bytes,
        ))
    }

    /// The delta is the intent's goal bytes, validated against this road's grammar. Pure — 41
    /// §4's "no side effects" holds because `Engine::ingest_observation` assembled the payload
    /// before T-1 ran.
    fn plan(&self, intent: &Intent, _pre: &ObjectSnapshot) -> gx_substrate::Result<PlannedDelta> {
        let bytes = intent.goal().0.clone();
        decode_observation_delta(&bytes)?;
        PlannedDelta::new(observation_substrate(), bytes)
    }

    /// The chain-state fingerprint for the snapshot's scope, read **now** — which is what makes
    /// T-10a's CAS a real check: a second observation committed on this scope between plan and
    /// commit moves this value.
    fn precondition(&self, snap: &ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        let scope = observation_scope_of(snap.locator())?;
        let digest = self.state_digest(&scope);
        Ok(Fingerprint::new(
            observation_substrate(),
            elide_scope(snap.locator().to_string())?,
            digest,
        )?)
    }

    /// Advance the chain: a write to **our** record space, never to any platform (SS273). The
    /// new head is the delta's own reference CID — the value the ingest response already handed
    /// the source as `chain_ref`, so the source can quote it as its next `prev_ref`.
    fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<AppliedDelta> {
        // The inverse grammar first: an executed restore puts the head back (record-level; not
        // reachable through `Engine::undo` in v0.1, which refuses observation undo by type).
        if let Ok(restore) = cbor::decode::<ObservationRestore>(delta.payload()) {
            if let Ok(mut s) = self.state.lock() {
                match &restore.head {
                    Some(h) => s.chains.insert(restore.restore.clone(), h.clone()),
                    None => s.chains.remove(&restore.restore),
                };
            }
            let digest = self.state_digest(&restore.restore);
            let postcondition = Fingerprint::new(
                observation_substrate(),
                elide_scope(restore.restore.clone())?,
                digest,
            )?;
            return Ok(AppliedDelta::new(
                delta.reference().clone(),
                postcondition,
                digest,
                Timestamp(0),
            ));
        }
        let decoded = decode_observation_delta(delta.payload())?;
        let head = delta.reference().cid.to_text();
        if let Ok(mut s) = self.state.lock() {
            s.chains.insert(decoded.scope.clone(), head);
        }
        let digest = self.state_digest(&decoded.scope);
        let postcondition = Fingerprint::new(
            observation_substrate(),
            elide_scope(decoded.scope.clone())?,
            digest,
        )?;
        Ok(AppliedDelta::new(
            delta.reference().clone(),
            postcondition,
            digest,
            Timestamp(0),
        ))
    }

    /// 🔴 Record-level escrow, or the honest `None` that makes E-M3-4 escalate (section header).
    ///
    /// Continuous chain ⇒ the inverse is "restore the head this engine held at plan time" —
    /// constructible entirely from the delta, so `Some`. A gap ⇒ `None`: the prior is not held,
    /// and an invented one would be exactly the fabricated chain the third state exists to
    /// prevent.
    fn invert(
        &self,
        delta: &PlannedDelta,
        _pre: &ObjectSnapshot,
    ) -> gx_substrate::Result<InvertOutcome> {
        let decoded = decode_observation_delta(delta.payload())?;
        if !chain_continuous(
            decoded.prev_claimed.as_deref(),
            decoded.held_at_plan.as_deref(),
        ) {
            return Ok(InvertOutcome::none(Vec::new()));
        }
        let restore = ObservationRestore {
            restore: decoded.scope,
            head: decoded.held_at_plan,
        };
        let payload = cbor::encode(&restore).map_err(|e| gx_substrate::Error::NotDigestible {
            detail: e.to_string(),
        })?;
        let inverse = PlannedDelta::new(observation_substrate(), payload)?;
        Ok(InvertOutcome::inverted(inverse, Vec::new()))
    }

    /// Two observations commute exactly when their scopes differ: one chain is one total order.
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> gx_substrate::Result<Commutation> {
        let sa = decode_observation_delta(a.payload())?;
        let sb = decode_observation_delta(b.payload())?;
        if sa.scope == sb.scope {
            Ok(Commutation::Conflicts {
                residual: b.reference().clone(),
            })
        } else {
            Ok(Commutation::Commutes)
        }
    }
}

/// What [`Engine::ingest_observation`] answers.
#[derive(Clone, Debug)]
pub struct ObservationIngest {
    /// The ordinary candidate this observation became (R-1).
    pub id: TransformationId,
    /// Whether this call replayed an earlier ingest of the same `(source, observation_id)` —
    /// the idempotent no-op, exactly one candidate (`req/824` A5's control).
    pub replayed: bool,
    /// Whether the chain claim was a gap. The surface renders the Escalate arm's `gx_code`
    /// (`CHAIN_GAP_ESCALATE`) from the **verdict**, never from this flag alone; the flag is
    /// carried so a response can print both sides of the disagreement.
    pub gap: bool,
    /// The value the source must quote as `prev_ref` for its next record on this scope once
    /// this candidate commits: the delta reference CID, which is also what `apply` writes as
    /// the chain head.
    pub chain_ref: String,
}

impl<E: EvidenceSource, C: Canonicalizer> Engine<E, C> {
    /// The engine-side half of `OBSERVATION_CLASS_UNKNOWN` (`req/824` A3: origin "gx-core class
    /// decode", surfaced through this crate's error type so the membrane never constructs an
    /// engine refusal of its own).
    ///
    /// # Errors
    /// [`Error::ObservationClassUnknown`] for a value outside the four `req/812` §1 classes —
    /// refused, never defaulted.
    pub fn observation_class(&self, class: &str) -> Result<ObservationClass> {
        ObservationClass::from_wire_str(class).ok_or_else(|| Error::ObservationClassUnknown {
            class: class.to_string(),
        })
    }

    /// 🔴 **`req/824` A5** — ingest one observation as an **ordinary candidate** (T-1 + T-2 on
    /// the observation substrate; the section header above carries the whole design).
    ///
    /// The caller (gx-api's ingest route) has already parsed the wire JSON into a typed
    /// [`ObservationRecord`]; this accessor owns everything semantic: the A2 admission (plaintext
    /// ⇒ refusal, chain gap ⇒ the Escalate road), the `observation_id` replay, the lazy adapter
    /// registration, and the intent construction. The candidate it returns is then verified and
    /// committed by the same calls as any other candidate.
    ///
    /// # Errors
    /// [`Error::Malformed`] for an empty `observation_id`; [`Error::PlaintextSecret`] when an
    /// envset value field is not of the declared digest form (`req/824` A2 — the refusal that
    /// keeps this from becoming a secrets store); whatever [`Engine::submit`] and
    /// [`Engine::plan`] refuse with.
    pub fn ingest_observation(
        &mut self,
        source: &str,
        observation_id: &str,
        prev_ref: Option<&str>,
        record: ObservationRecord,
        rng_seed: u64,
        at: Timestamp,
    ) -> Result<ObservationIngest> {
        if observation_id.is_empty() {
            return Err(Error::Malformed {
                detail: "`observation_id` must be a non-empty string: it is the source's own id \
                         for the operation, and without it a retry is indistinguishable from a \
                         second operation (req/824 §2-1)"
                    .to_string(),
            });
        }
        // The per-class chain scope. Envset carries its own scope; the other three are keyed on
        // the reporting source so two sources' chains never interleave.
        let scope = match &record {
            ObservationRecord::Envset(fp) => {
                format!("envset/{}/{}", fp.scope().project, fp.scope().environment)
            }
            ObservationRecord::Deploy(r) => format!("deploy/{source}/{}", r.target_env),
            ObservationRecord::Config(_) => format!("config/{source}"),
            ObservationRecord::LogWindow(r) => format!("logw/{source}/{}", r.stream_id),
        };
        let held = self.observation_road.chain_head(&scope);
        // 🔴 The A2 admission, engine-side. Deny is checked before the chain (a plaintext value
        // must never ride an Escalate into the ledger); the Escalate arm is NOT an error — the
        // candidate is created and the gate reaches the third state through invert (section
        // header). The chain rule for the other three classes is the same three-way shape,
        // through `chain_continuous`.
        let gap = match &record {
            ObservationRecord::Envset(fp) => match fp.admit(held.as_deref()) {
                EnvsetAdmission::Deny { name } => {
                    return Err(Error::PlaintextSecret { name });
                }
                EnvsetAdmission::Escalate { .. } => true,
                EnvsetAdmission::Allow => false,
            },
            _ => !chain_continuous(prev_ref, held.as_deref()),
        };
        if let Some(remembered) = self.observation_road.replay(source, observation_id) {
            return Ok(ObservationIngest {
                id: remembered.id,
                replayed: true,
                gap: remembered.gap,
                chain_ref: remembered.chain_ref,
            });
        }
        if !self.adapters.contains_key(&observation_substrate()) {
            self.register_adapter(
                self.observation_road.clone(),
                "engine-internal observation road (req/824 A5)",
            );
        }
        let class = record.class();
        let delta = ObservationDelta {
            class,
            source: source.to_string(),
            observation_id: observation_id.to_string(),
            scope: scope.clone(),
            prev_claimed: prev_ref.map(str::to_string),
            held_at_plan: held,
            record,
        };
        let payload = cbor::encode(&delta).map_err(Error::Canon)?;
        let intent = Intent::new(
            observation_substrate(),
            observation_locator(&scope, source, observation_id),
            GoalBytes(payload),
            // 42 §3.2's second row, literally: "the change responds to new evidence -- an
            // observation arrived and the state follows it".
            ChangeContext::Evidence,
            Actor::Process {
                key: format!("attach-source:{source}"),
            },
        );
        self.submit(&intent, rng_seed, at)?;
        let id = self.plan(&intent, at)?;
        let chain_ref = self
            .table
            .get(&id)
            .map(|entry| entry.delta.reference().cid.to_text())
            .unwrap_or_default();
        self.observation_road.remember(
            source,
            observation_id,
            RememberedIngest {
                id,
                chain_ref: chain_ref.clone(),
                gap,
            },
        );
        Ok(ObservationIngest {
            id,
            replayed: false,
            gap,
            chain_ref,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declared list is the variants, in order (**E-M2-23 / A-10**).
    #[test]
    fn the_declared_states_are_the_variants_in_order() {
        let variants = [
            Lifecycle::Draft,
            Lifecycle::Candidate,
            Lifecycle::Verifying,
            Lifecycle::Admitted,
            Lifecycle::Denied,
            Lifecycle::Escalated,
            Lifecycle::Canonicalized,
            Lifecycle::Committing,
            Lifecycle::Committed,
            Lifecycle::Aborted(AbortReason::Expired),
            Lifecycle::Superseded,
        ];
        let names: Vec<&str> = variants.iter().map(Lifecycle::name).collect();
        assert_eq!(names, LIFECYCLE_STATES.to_vec());
    }

    /// 43 §1's terminal column, for the three that do not depend on a setting.
    #[test]
    fn the_terminal_states_are_the_ones_43_1_marks_terminal() {
        assert!(Lifecycle::Committed.is_terminal());
        assert!(Lifecycle::Superseded.is_terminal());
        assert!(Lifecycle::Aborted(AbortReason::Expired).is_terminal());
        // `Denied` is terminal "but only under record-only mode" (sem: SEM-gx-engine-304), so the answer needs a mode and this
        // function does not take one -- `canonicalize` is where the setting is read.
        assert!(!Lifecycle::Denied.is_terminal());
        assert!(!Lifecycle::Candidate.is_terminal());
    }
}
