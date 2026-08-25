// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `A(f)`: the predicate that decides whether a transformation is admissible.
//!
//! Spec: `req/spec/40-architecture/41-architecture.md` §2 for where this crate sits and what its
//! modules are called, §4 for the three signatures it owes (`GateInput`, `InvariantCheck`,
//! `Gate::verify`), §6 for what every crate here may not do. 42 §3.8 holds the field tables and
//! 34 §E holds the five acceptance criteria (AC-025..029).
//!
//! # What a gate is not
//!
//! It is **not** the state machine. 43's transitions belong to gx-engine (M5): a `Verdict` says
//! what is true of a transformation, and what a system *does* about that -- refuse the commit,
//! record it anyway under `EnforcementMode::RecordOnly`, wait for a human -- is decided one layer
//! up. req/60 §1 lists the twelve things M3 deliberately does not build and why each one is
//! somebody else's; the two that shape this file are N-01 (no state machine) and N-02 (no
//! `EnforcementMode` / `FailPosture`).
//!
//! It also **cannot see the change it is judging**. 42 §3.4 makes a delta's payload opaque to
//! everything below the adapter (P-6), so what a policy may reason over is the locator, the actor,
//! the change context, the order, whether an inverse exists, and the evidence -- and req/38 §19's
//! M3-10 ruling requires that limit to be said out loud rather than discovered by a user whose
//! a "row-count preserved" invariant cannot be written: "the v0.1 pack's effective reach is
//! **stated explicitly as the locator/actor/context/order class** (no overclaiming)". (sem: SEM-gx-gate-031)
//!
//! # What is built, and by which hand
//!
//! | hand | module | what arrived | state |
//! |---|---|---|---|
//! | 1 | `lib.rs`, `verdict.rs` | the crate, its place in the workspace, the measured cedar-policy dependency, the types | done (req/61) |
//! | 2 | `policy.rs` | the Cedar `PolicySet`, ASM-60-1's request mapping, AC-025 | done (req/62) |
//! | 3 | `invariant.rs` | `InvariantCheck` (as `check(&GateInput)`, **E-M3-7**), the registry, AC-026/029 | done (req/63) |
//! | 4 | `verdict.rs` | the composition -- AC-027's Deny-precedence, E-M3-4's escalation rule, the error vocabulary | done (req/64) |
//! | 5 | `packs.rs` | the shipped fs pack, its one road into a build, and AC-028's conformance | done (req/65) |
//!
//! So a gate decides with **both** of the systems 41 §4 gives it, and folds them the one way
//! AC-029 requires: [`Gate::verify`] evaluates the policy set, runs every registered invariant, and
//! refuses if either refused. AC-027 states that fold over all four quadrants as a property, in the
//! `verdict_meet` suite E-M3-10 names, and the third arm (E-M3-4's `invert_available=false →
//! Escalate`) is the one hand 4 wrote. (This paragraph said "AC-027 is still not claimed" until (sem: SEM-gx-gate-032)
//! hand 5; hand 4 claimed it and left the sentence behind, which `req/65` §4 records.)
//! A gate that was never given a policy set still answers `Err`, because an empty `policies/`
//! directory and a working deployment must not look the same (req/29 §4).
//!
//! `packs/` is 41 §2's fifth module and hand 5 declares it: [`packs`] holds the shipped fs pack, the
//! single `include_str!` that puts it in a build, and the range a pack may reason over (M3-10).
//!
//! # T1 is not a property of this crate (**E-M3-5**)
//!
//! 51 §3 asks gx-gate for a property test named `admissible_composition` asserting
//! `A(f) ∧ A(g) → A(g∘f)`, and **that statement is false of an arbitrary Cedar policy set**. The
//! counterexample is one policy: forbid any transformation that touches `/a` and `/b` together.
//! `f` (only `/a`) is admitted, `g` (only `/b`) is admitted, `g∘f` is denied. Lean's
//! `T1_composition_sound` does not say otherwise -- it takes `Admissible A idOf` (identity plus
//! closure under composition) as a *hypothesis* and unfolds it, and 12 F0 likewise requires
//! `A.IsMultiplicative`. req/38 §19 rules accordingly:
//!
//! > "★M3-05 (adopted = option a+b combined): **T1 is not a property of the gate implementation**
//! > ... (1) a composed transformation's verdict is defined not as re-evaluation but as **structural
//! > inheritance of the lower verdicts** (2) 51 §3's `admissible_composition` proptest only has
//! > meaning in the shape "restricted to the multiplicative class + a declared class precondition",
//! > into **M8's restricted suite** (3) the doc states explicitly that it does not say "T1 is a
//! > property of the gate"" (sem: SEM-gx-gate-033)
//!
//! This paragraph is that third clause. **Nothing this crate produces is warranted by T1**, and
//! [`AdmitProof::composed_from`] is the vessel the first clause asks for (sem: SEM-gx-gate-034) -- the place a composed verdict
//! records the lower verdicts it inherited from, rather than a claim that re-evaluating would have
//! agreed.

#![forbid(unsafe_code)]
// NFR-022 (33): a gate's refusal vocabulary is operator-facing, so every public item says when
// it fires and which ruling shaped it. `deny` keeps that closed the way gx-core's is
// (req/159 §B-2, req/166).
#![deny(missing_docs)]

pub mod invariant;
pub mod packs;
pub mod policy;
pub mod verdict;

pub use invariant::{InvariantCheck, InvariantRegistry};
pub use policy::{
    ApprovalRequirement, ContextValue, EscalationTicket, PolicyEngine, PolicyOutcome, RequestView,
    TicketId,
};
pub use verdict::{
    AdmitProof, InvariantResult, PolicyDecisionRecord, Reason, ReasonSource, Verdict,
};

// ---------------------------------------------------------------------------
// The error vocabulary window (**E-M3-6** / M3-08 / E-M2-23 / C-5 / D-3)
// ---------------------------------------------------------------------------

/// A policy in the set refused the transformation (**E-M3-6**).
///
/// One of the three names req/38 §19's M3-08 creates. None of the three is in 44 §2.3's `gx_code`
/// table, and that absence is the reason they exist -- see [`REASON_CODES`].
pub const POLICY_DENIED: &str = "POLICY_DENIED";

/// A registered invariant did not hold (**E-M3-6**).
pub const INVARIANT_VIOLATED: &str = "INVARIANT_VIOLATED";

/// 🔴 **retired** (2026-08-09, M5 hand 2) — the evidence a policy required was not shown. (sem: SEM-gx-gate-035)
///
/// **Not in [`REASON_CODES`]. [`Reason::new`] refuses it.** The name is kept, not deleted, so that
/// every reference that already exists -- req/60 §3, req/64 §4, req/65 §5, `tools/verify_m3h4.sh`,
/// and any receipt written while it was live -- still resolves to a definition that says what it
/// meant and when it stopped being usable. Deleting a retired name turns its history into dead
/// ends; that is what `no-delete` is for.
///
/// # Why it was retired
///
/// **E-7** declared it in M3 with no producer and armed a kill condition in the same breath
/// (req/38 §23): "if no producer grows even in the M4 window, drop it from the vocabulary (the same
/// discipline as D-7)" (sem: SEM-gx-gate-036). M4 did not grow
/// one -- **M4-09 adopted (b)** kept the facts path as design only and re-armed the condition for M5
/// (req/38 §28) -- and req/78 §4's M5-04 row measured the third window and found it empty:
///
/// > This batch's observation: **0 of M5's 22 ACs require facts** (sem: SEM-gx-gate-037)
///
/// req/38 §37 fires it:
///
/// > **M5-04 adopted (b), with a correction**: **E-7 fires** -- `EVIDENCE_INSUFFICIENT` is
/// > **retired** from the gate vocabulary (retirement mark = removed from the vocabulary table; the
/// > const stays, with a retirement doc; error_vocabulary test updated the same turn) (sem: SEM-gx-gate-038)
///
/// # What was true of it, and stays true
///
/// A policy could always *state* an evidence requirement: ASM-60-1 maps the evidence summary into
/// `context.evidence_count` and `context.evidence_kinds`, so "this change needs a TestResult" is a (sem: SEM-gx-gate-039)
/// Cedar `when` clause today, and its refusal arrives as [`POLICY_DENIED`]. What never existed is a
/// way for a policy to say *which* of its clauses was about evidence. That is a policy-side
/// annotation, and after three milestones with no acceptance criterion asking for one, the honest
/// record is that the code named a distinction the system does not draw.
///
/// # What would reopen it
///
/// Not a wish: a fact. `error_vocabulary.rs` still asserts that 44 §2.3 carries no evidence code,
/// because 44 growing one is the single observation that would make this word an API-side name the
/// gate has to borrow rather than a gate-side name with nobody to say it.
pub const EVIDENCE_INSUFFICIENT: &str = "EVIDENCE_INSUFFICIENT";

/// No inverse could be built for this change (**E-M3-4**; 44 §2.3's own spelling).
///
/// This one is *not* new: it is 44 §2.3's `INVERSE_UNAVAILABLE` (409, "the inverse delta for the
/// undo target is unavailable"), used as written. M3-08's containment -- "the gate's reason codes
/// ⊇ the API's gx_code" -- is what admits it: (sem: SEM-gx-gate-040)
/// the gate's vocabulary is the wider set, so where the API already has the word the gate uses that
/// word rather than a synonym. It is the code an escalation reason carries, and
/// `crates/gx-gate/tests/error_vocabulary.rs` reads 44 §2.3 out of the spec file to check that this
/// name is in it and that the three above are not.
pub const INVERSE_UNAVAILABLE: &str = "INVERSE_UNAVAILABLE";

/// Every `Reason.code` this crate may produce, declared once (**E-M3-6**, req/38 §19's M3-08).
///
/// > "in hand 4's error-vocabulary window, **declare a crate's Error vocabulary table (the
/// > substance of what E-M2-23 filed) in one place; anything outside it is refused at
/// > construction**" (sem: SEM-gx-gate-041)
///
/// [`Reason::new`] refuses anything outside this array, which is the same shape H5-8 gave
/// `VERDICT_KINDS` in M2 and E-M3-2 later turned into a type. A type is not available here: 42 §3.8
/// types `code` as `String` and 44 §2.3's containment means the set is open at the API end, so the
/// vocabulary is a checked list rather than an enum -- and the check runs at construction, which is
/// the earliest moment the fact is knowable.
///
/// Sorted, and asserted to be sorted, so that a name added out of order is a failing test rather
/// than a list nobody can scan.
///
/// **Three, not four.** [`EVIDENCE_INSUFFICIENT`] was here until M5 hand 2 and was retired by
/// **E-7** (req/38 §37, M5-04 adopted (b)) (sem: SEM-gx-gate-042); see that constant for why. The three that remain are the two
/// M3-08 created with producers behind them and the one borrowed from 44 §2.3.
pub const REASON_CODES: [&str; 3] = [INVARIANT_VIOLATED, INVERSE_UNAVAILABLE, POLICY_DENIED];

/// Every variant of this crate's [`Error`], by name (**E-M2-23**).
///
/// E-M2-23's erratum asks for "put a crate's Error vocabulary table in 41 §6" (sem: SEM-gx-gate-043). The spec is not this hand's
/// to write (52: `req/spec/` unmodified), so the table lives here, next to the enum it describes,
/// and `crates/gx-gate/tests/error_vocabulary.rs` counts the enum's variants out of the source and
/// compares -- a variant added without a row here fails that test. The erratum against 41 §6 stays
/// open and is recorded in req/64 §4.
///
/// Two of the six were raised by earlier hands and are named in the ruling: `PolicySetUnreadable`
/// is C-5's (hand 2) and `RegistryUnusable` is D-3's (hand 3). Both map to 44 §2.3's `POLICY_ERROR`
/// (500) at the API boundary; **44 has no code for the invariant side**, and that gap is req/64 §4's
/// erratum rather than a name invented here.
pub const ERROR_KINDS: [&str; 6] = [
    "EmptyDeny",
    "NotDigestible",
    "PolicySetUnreadable",
    "RegistryUnusable",
    "Unevaluable",
    "UnknownReasonCode",
];

/// The bound on a recorded message, in bytes (**M3-21**, ASM-60-4's initial value).
///
/// > "M3-21 (adopted): the detail/message upper bound is one constant (filed with **1024 bytes as
/// > ASM-60-4's initial value**; Owner may change it); an overrun is diverted to a digest at
/// > construction. H6-7 basis" (sem: SEM-gx-gate-044)
///
/// One constant for both places 42 §3.8 asks for a short message -- [`Reason`]'s and
/// [`InvariantResult`]'s -- because two constants agree until the day they do not. What happens at
/// the boundary is [`verdict::elide`]'s, and the value itself is an assumption Owner may change:
/// nothing in 42 or 44 fixes a number, so this line is the whole of ASM-60-4 in code.
pub const MAX_MESSAGE_BYTES: usize = 1024;

// The discriminant `Verdict::kind` answers with, published rather than redefined (**E-M3-2**; see
// the module note in `gx_core::verdict` for why it lives one layer down).
pub use gx_core::VerdictKind;

use gx_core::{ObjectSnapshot, PlannedDeltaBytes, Timestamp, Transformation, TransformationId};
use gx_witness::{Evidence, PolicyDecision};

/// Everything gx-gate can refuse to do.
///
/// # ⊥, and the two things that are not ⊥
///
/// **E-M3-3** made `verify` fallible and said what the failure means: "unevaluable (⊥) is neither
/// Deny nor Escalate ... fail-closed is realized on the engine side as 'Err -> application forbidden
/// + POLICY_ERROR' (consistent with 43 T-4d)" (sem: SEM-gx-gate-045). The first four variants are that ⊥ -- an evaluation that could not be performed.
///
/// The last two are not. [`Error::UnknownReasonCode`] and [`Error::NotDigestible`] are refusals to
/// *build a record*, which happen while a verdict is being written down rather than while one is
/// being reached. They are in the same enum because a caller has one `Result` to handle and a
/// second error type would make it two, and they are distinguishable by name for the same reason
/// E-M3-3 kept ⊥ out of `Verdict`: an operator reading a failure has to be able to tell "the
/// evaluator is broken" from "this crate refused to record a code it does not know". (sem: SEM-gx-gate-046)
///
/// [`ERROR_KINDS`] is the vocabulary table E-M2-23 asks for, and it names every variant here.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The gate could not evaluate the input at all.
    ///
    /// 44 §2.3 has the API-side code this becomes: `POLICY_ERROR` (500), "a failure of the Cedar
    /// evaluation itself (a policy bug, etc.)" (sem: SEM-gx-gate-047). It is deliberately *not* a `Verdict`: an operator reading a receipt has to
    /// be able to tell "the policy refused this" from "the evaluator is broken", and 42 §3.10
    /// records a verdict in every receipt (ASM-14).
    #[error("the gate cannot evaluate {transformation:?}: {detail}")]
    Unevaluable {
        /// Which transformation was on the table when evaluation failed -- carried so the
        /// engine's `POLICY_ERROR` names the change it now must not apply.
        transformation: TransformationId,
        /// What went wrong, in words, for the operator separating "the policy refused this"
        /// from "the evaluator is broken". (sem: SEM-gx-gate-048)
        detail: String,
    },

    /// The policy set itself could not be read (hand 2).
    ///
    /// The same ⊥ as [`Error::Unevaluable`] one step earlier: a set that does not parse, or that
    /// names its policies in a way gx cannot record, has decided nothing about anything. It is a
    /// separate variant because it happens before there is a transformation to name -- 44 §2.3's
    /// `POLICY_ERROR` covers both ("a failure of the Cedar evaluation itself (a policy bug, etc.)") (sem: SEM-gx-gate-049) and hand 4's window
    /// decides how the crate's error table is declared, not whether the load step may fail.
    #[error("the policy set cannot be read: {detail}")]
    PolicySetUnreadable {
        /// Why the set could not be read -- Cedar's parse diagnostics, or which policy id was
        /// unrecordable.
        detail: String,
    },

    /// The invariant registry itself could not be built (hand 3).
    ///
    /// The same ⊥ as [`Error::PolicySetUnreadable`] on the other half of 41 §4's gate: a registry
    /// that cannot name its invariants apart has decided nothing about anything, and the refusal
    /// happens at registration rather than at evaluation because that is where the fact is knowable.
    /// See [`InvariantRegistry::register`] for why a duplicate id is a refusal and not a warning.
    #[error("the invariant registry cannot be used: {detail}")]
    RegistryUnusable {
        /// Why the registry could not be built, e.g. which id was registered twice.
        detail: String,
    },

    /// A `Deny` was built with no reasons (see [`Verdict::deny`]).
    #[error("a Deny must carry at least one Reason (42 §3.8)")]
    EmptyDeny,

    /// A [`Reason`] was built with a code outside [`REASON_CODES`] (**E-M3-6**, M3-08).
    ///
    /// The refusal "a code outside the declaration is refused at construction" asks for, as a value (sem: SEM-gx-gate-050). It names the code it refused
    /// so that a misspelling is readable in the failure rather than only in a diff.
    #[error("{code:?} is not one of gx-gate's Reason codes {REASON_CODES:?} (E-M3-6)")]
    UnknownReasonCode {
        /// The code as offered, so a misspelling is readable in the failure itself.
        code: String,
    },

    /// gx-canon has no canonical form for something this crate had to digest.
    ///
    /// Reachable from [`Verdict::proof_digest`] (M2-10) and from the elision M3-21 performs. Not a
    /// statement about the transformation: a value that cannot be encoded has no identity, which is
    /// a fact about the record and not about whether the change was admissible.
    #[error("gx-canon cannot give this value an identity: {detail}")]
    NotDigestible {
        /// gx-canon's own account of which clause of 42 §2.1 the value broke.
        detail: String,
    },
}

impl Error {
    /// Which of [`ERROR_KINDS`] this refusal is (**M6-09**).
    ///
    /// The array has existed since M3 and nothing could *ask* a value which row it was, so a
    /// consumer mapping gx-gate's refusals onto 44 §2.3 had to match on the variants and keep its
    /// own spelling of the vocabulary. That second spelling is what E-M2-23's "declare in one place"
    /// is against, and it is what M6-09's "the mapping lives in one place" would have produced (sem: SEM-gx-gate-051). One line per variant,
    /// no `_` arm, in the array's order.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Error::EmptyDeny => "EmptyDeny",
            Error::NotDigestible { .. } => "NotDigestible",
            Error::PolicySetUnreadable { .. } => "PolicySetUnreadable",
            Error::RegistryUnusable { .. } => "RegistryUnusable",
            Error::Unevaluable { .. } => "Unevaluable",
            Error::UnknownReasonCode { .. } => "UnknownReasonCode",
        }
    }
}

/// What gx-gate answers with when it cannot answer with a [`Verdict`].
pub type Result<T> = core::result::Result<T, Error>;

/// Everything a gate is handed (41 §4 verbatim, five fields; **E-DR4627-1** appends a sixth). (sem: SEM-gx-gate-052)
///
/// ```text
/// pub struct GateInput<'a> {
///     pub t: &'a Transformation,
///     pub pre: &'a ObjectSnapshot,
///     pub planned: &'a PlannedDelta,
///     pub evidence: &'a [Evidence],
///     pub invert_available: bool,
///     pub decided_at: Timestamp,   // E-DR4627-1 (DR-46-27)
/// }
/// ```
///
/// Field for field, with one substitution the rulings make: `planned` is
/// [`gx_core::PlannedDeltaBytes`], the opaque carrier **E-M3-1** put in gx-core because 42 §0 files
/// the real `PlannedDelta` under gx-substrate (M4). `gx-witness/tests/ac_016.rs` holds a stand-in
/// with the same five fields and reads 41 §4 to check the list; that file stays where it is, since
/// gx-witness naming this crate is the cycle **E-M3-2** exists to avoid.
///
/// audit #10 is why this struct exists at all -- "it carries the input an invariant needs to run" (sem: SEM-gx-gate-053) -- and E-M3-7
/// makes that literal: hand 3's `InvariantCheck::check` takes `&GateInput` rather than
/// `(&ObjectSnapshot, &PlannedDelta)`, because AC-029 asks an invariant to refuse an **order-2**
/// transformation and `order` is a field of `t`.
///
/// # What is not in here, and is asked for elsewhere
///
/// 45 TH-12 wants a gate to answer `Escalate` when a change conflicts with a later one, and
/// commutation is not among the five fields -- there is no input path for it. req/38 §19 keeps
/// that as a field-addition proposal for M4 rather than growing 41 §4's struct here.
///
/// **`Provenance` is not here either, and that is a decision (H5-5).** req/38 §13 H4-2 asked M2 to
/// propose how 43 T-5's "provenance chain" is consumed rather than to grow a field for it, req/38 §14
/// shelved the answer as "a leading candidate for M3's gate design", and req/60 §4 routes it to this
/// hand: "decide whether the gate consumes `Provenance` (i.e. whether it belongs on `GateInput`), and
/// **if it does not, write honestly that 'M3 too has 0 consumers'**" (sem: SEM-gx-gate-054). It is not loaded, so the honest sentence is the one the ruling asks for: **in M3
/// as in M2, `Provenance` has no consumer.** Three measurements decided it, and none of them is
/// "it would not compile" -- the dependency already runs this way (gx-gate -> gx-witness) (sem: SEM-gx-gate-055), so
/// carrying one would have cost a field and nothing else:
///
/// 1. **A policy could not see it.** ASM-60-1's mapping has no row for provenance, and req/38 §21's
///    C-1 ruling refuses to add rows to that mapping in M3 ("do not grow it in M3 ... if the need for
///    expressiveness is real, do it once, in the same window as M4's facts path") (sem: SEM-gx-gate-056). A field a policy cannot read and an invariant is
///    the only consumer of is an M4 question wearing an M3 field.
/// 2. **An invariant could not derive it.** `Provenance::derive_from(t, inputs)` needs
///    `ProvenanceInputs` -- the input objects an adapter read and the environment -- which nobody
///    below the engine collects (42 §3.9, E-M2-5). Handing a gate the transformation and expecting
///    provenance out of it is the derivation that ruling already refused.
/// 3. **The chain is a chain of receipts.** 43 T-5 appends a signed human decision *to the chain*,
///    and a gate issues no receipt: the engine does (42 §3.10, M5). So the consumer M2 could not
///    find is one layer above this one, not this one.
///
/// The three proposals M2 recorded (a §1.3 row for `Provenance`, `ReceiptPayload.provenance:
/// Option<Cid>`, and the traversal order) are therefore M5's window, and the alternative it recorded
/// beside them -- an index on the evidence store side -- is M4's. Both are raised, neither is ruled,
/// and 41 §4 still has five fields.
#[derive(Debug)]
pub struct GateInput<'a> {
    /// The transformation being judged. `order` (0, 1 or 2) is read from here, which is what makes
    /// AC-029's "there is no order-2-only bypass path" checkable at all. (sem: SEM-gx-gate-057)
    pub t: &'a Transformation,
    /// The object as it is now.
    pub pre: &'a ObjectSnapshot,
    /// The planned change, as bytes nobody here may interpret (P-6, **E-M3-1**).
    pub planned: &'a PlannedDeltaBytes,
    /// What gx-witness collected. AC-016 is the compile-time criterion that this slot takes those
    /// values "as-is" -- no wrapper, no conversion. (sem: SEM-gx-gate-058)
    pub evidence: &'a [Evidence],
    /// Whether the adapter could build an inverse (FR-043). **E-M3-4** makes `false` the one
    /// condition that produces an `Escalate` in v0.1; the rule itself is hand 4's.
    pub invert_available: bool,
    /// **The sixth field, appended by E-DR4627-1 (DR-46-27).** The moment the gate is being asked,
    /// which is `Engine::verify`'s `at` and not `t.created_at` (that one is *plan* time, ASM-4
    /// metadata). An invariant that wants to say "now is inside this window" had no input path for
    /// "now" before this field: the only clock reaching a gate was plan time, so a time window was
    /// unwritable even on the invariant face. DR-46-27 ships **the seat and nothing else** -- no
    /// window predicate evaluates it in v0.
    ///
    /// **PS-1 holds: this field is not Cedar vocabulary.** [`policy::RequestView::of`] projects
    /// seven things out of this struct and `decided_at` is not among them, which
    /// `crates/gx-gate/tests/decided_at_seat.rs` scans for rather than trusts. What that buys is the
    /// determinism claim replay rests on: varying `decided_at` alone cannot move a `Verdict`,
    /// because nothing downstream reads it. When a real window predicate is written, that
    /// differential test is *supposed* to fail -- it is evidence about v0, not a law.
    ///
    /// **What it does not do (req/447 §5-5, verbatim in DR-46-27 §2-4):** it does not stop a
    /// deployment-authored `InvariantCheck` from calling `SystemTime::now()` itself. That road is
    /// unclosed by types before this field and unclosed after it. This field offers a lawful way in;
    /// it revokes nothing.
    pub decided_at: Timestamp,
}

/// The admissibility predicate (41 §4: "Cedar PolicySet + `Vec<Box<dyn InvariantCheck>>`"). (sem: SEM-gx-gate-059)
///
/// Both halves, as of hand 3. The `Option` on the first is not "fields to be decided" (sem: SEM-gx-gate-060) -- it is the
/// difference between a gate that was never given policies and a gate that was given an empty set,
/// and those two answer differently on purpose (see [`Gate::verify`]). The second has no `Option`
/// around it, because an empty registry is a registry: 41 §4 types it as a `Vec`, and "nobody was
/// asked" is a state a deployment can legitimately be in while "no policy set was loaded" is not (sem: SEM-gx-gate-061)
/// (see [`InvariantRegistry::is_empty`] for the difference that costs).
#[derive(Debug, Default)]
pub struct Gate {
    policies: Option<PolicyEngine>,
    invariants: InvariantRegistry,
}

impl Gate {
    /// A gate with no policies and no invariants.
    ///
    /// It is named for what it is rather than called `new`, so that a caller cannot read "I built
    /// a gate" into an object that cannot judge anything. (sem: SEM-gx-gate-062)
    #[must_use]
    pub fn unconfigured() -> Self {
        Self {
            policies: None,
            invariants: InvariantRegistry::new(),
        }
    }

    /// A gate that decides with this policy set (FR-025).
    ///
    /// Build the engine with [`PolicyEngine::parse`]; the source of the text is the caller's, and
    /// this crate reads no files. FR-028's shipped pack is embedded at build time by hand 5, which
    /// is where "there is exactly one road by which a pack's file gets embedded into the build" becomes a check. (sem: SEM-gx-gate-063)
    #[must_use]
    pub fn with_policies(policies: PolicyEngine) -> Self {
        Self {
            policies: Some(policies),
            invariants: InvariantRegistry::new(),
        }
    }

    /// The same gate, deciding with these invariants as well (FR-026).
    ///
    /// A whole registry rather than one check at a time, because [`InvariantRegistry::register`] is
    /// where a duplicate id is refused and a gate that took checks one by one would need to carry
    /// that refusal into its own signature. Building the registry first keeps the failure where the
    /// fact is: at registration.
    #[must_use]
    pub fn with_invariants(mut self, invariants: InvariantRegistry) -> Self {
        self.invariants = invariants;
        self
    }

    /// The set this gate decides with, if it has one.
    #[must_use]
    pub fn policies(&self) -> Option<&PolicyEngine> {
        self.policies.as_ref()
    }

    /// The invariants this gate runs, in registration order.
    #[must_use]
    pub fn invariants(&self) -> &InvariantRegistry {
        &self.invariants
    }

    /// 41 §4's entry point, with **E-M3-3**'s return type.
    ///
    /// 41 §4 writes `fn verify(&self, input: GateInput) -> Verdict`. req/38 §19 makes it fallible
    /// and records the erratum against 41 §4 ("M3-03 (adopted = option a): `verify -> Result<Verdict>`") (sem: SEM-gx-gate-064),
    /// because `InvariantCheck::check` can fail and 44 §2.3 already carries `POLICY_ERROR` for a
    /// Cedar evaluation that fell over. The three-armed `Verdict` therefore keeps meaning "a
    /// decision was reached". (sem: SEM-gx-gate-065)
    ///
    /// AC-029 asks that order-2 transformations come through **this** function and no other, and
    /// the shape of that guarantee is what this function is written to keep: one entry point, one
    /// evaluation of the policy set, one run of the registry, and **no mention of `order` anywhere
    /// in this file**. `crates/gx-gate/tests/ac_029.rs` measures all three -- the behaviour, the
    /// call trace, and the source itself -- because "a bypass does not exist" is a claim about (sem: SEM-gx-gate-066)
    /// what is absent, and req/60 §7.2 requires such a claim to be backed by a mutation that adds
    /// the missing path and turns the suite red.
    ///
    /// # The fold (FR-027 / AC-027 / **E-M3-4**)
    ///
    /// One fold, three arms, in one order:
    ///
    /// | condition | verdict | source |
    /// |---|---|---|
    /// | the policy set refused, or any invariant did not hold | `Deny` | AC-027: "if either side includes even one Deny, the overall Verdict must be `Deny`" (sem: SEM-gx-gate-067) |
    /// | nothing refused, and `invert_available == false` | `Escalate` | AC-048 / **E-M3-4** |
    /// | nothing refused, and an inverse exists | `Admit` | AC-027 |
    ///
    /// **Deny outranks Escalate**, and the order is AC-027's rather than a preference: its first
    /// sentence is unconditional, so a refusal stays a refusal whether or not the change can be
    /// undone. A gate that escalated a refused change would ask a human to approve something two
    /// systems had already refused.
    ///
    /// AC-027's second sentence -- "`Admit` only when both are Admit" -- is read as a *necessary* (sem: SEM-gx-gate-068)
    /// condition (an `Admit` implies both admitted) and not a sufficient one, because E-M3-4 adds a
    /// third arm to the same case:
    ///
    /// > "M3-04 (adopted = option a): raise AC-048's gate-side half to an M3 requirement -- make
    /// > **`invert_available=false -> Escalate`** the minimal generation rule for M3 (a policy
    /// > firing an extra condition is M4's)" -> **E-M3-4** (sem: SEM-gx-gate-069)
    ///
    /// The two readings disagree on exactly one input -- both systems admit, no inverse exists --
    /// and the ruling is later and specific. `crates/gx-gate/tests/verdict_meet.rs` states the
    /// four quadrants as AC-027 writes them with `invert_available = true` held fixed, and the
    /// escalation arm separately; the tension is reported in req/64 §4 rather than resolved here.
    ///
    /// The registry runs whatever the policy said (FR-026 is unconditional), and both systems are
    /// consulted before either is folded, so the verdict does not depend on which was asked first.
    ///
    /// `AdmitProof` accordingly carries the deciding policies, the invariant results, and two empty
    /// vectors. `evidence_digests` stays empty, and hand 4 changed the reason rather than the fact:
    /// hands 1-3 could not mint a CID at all, and this hand can (M2-10 required the edge to
    /// gx-canon), so what keeps the vector empty now is that **nothing here uses evidence to
    /// decide**. A policy may already require evidence through ASM-60-1's `context.evidence_count`
    /// and `evidence_kinds`, and such a refusal arrives as [`POLICY_DENIED`]; until something in
    /// this crate rests a decision on a specific `Evidence` value, a list of digests would claim a
    /// dependency that does not exist. Nothing did, across M3, M4 and M5's acceptance criteria,
    /// which is what retired [`EVIDENCE_INSUFFICIENT`] (E-7, req/38 §37).
    ///
    /// `composed_from` stays empty because no composition happened -- **E-M3-5**: a composed
    /// verdict is inherited from lower verdicts, never re-derived here.
    ///
    /// On the `Deny` arm the invariant results are **not** carried: 42 §3.8 gives a `Deny` reasons
    /// and gives a proof only to an `Admit`, so an invariant that held is recorded when the change
    /// is admitted and forgotten when it is refused. That asymmetry is the spec's rather than this
    /// function's, and it is raised in `req/63_M3_HAND3_REPORT_2026-08-08.md` §4 rather than fixed
    /// here.
    ///
    /// # Errors
    /// [`Error::Unevaluable`] when this gate was never given a policy set, whenever the policy
    /// evaluation cannot be performed (the mapping refused, or Cedar's diagnostics carried an
    /// error -- see [`PolicyEngine::evaluate`]), and whenever an invariant could not decide (see
    /// [`InvariantRegistry::run_all`]). All three are the ⊥ of **E-M3-3** and none of them is a
    /// verdict. [`Error::EmptyDeny`] is unreachable from here and is not special-cased: a refusal
    /// arrives with at least one reason by construction, from `PolicyOutcome::deny_reasons` or from
    /// [`invariant::violations`], and a change that broke that should surface as the error it is
    /// rather than as a verdict.
    ///
    /// A gate with no policy set answers `Err` rather than `Admit`, which is the fail-open req/29
    /// §4 forbids in the one place it would be least visible: an empty `policies/` directory and a
    /// working deployment must not look the same. It is also not `Deny` -- **E-M3-3**: "naming
    /// 'refused' and 'broken' with the same verdict muddies the semantics of the decision". (sem: SEM-gx-gate-070)
    pub fn verify(&self, input: GateInput<'_>) -> Result<Verdict> {
        let Some(engine) = self.policies.as_ref() else {
            return Err(Error::Unevaluable {
                transformation: input.t.id,
                detail: "this gate holds no PolicySet, so nothing has been evaluated".to_string(),
            });
        };

        let outcome = engine.evaluate(&input)?;
        let results = self.invariants.run_all(&input)?;
        let violations = invariant::violations(&results)?;

        match (outcome.decision(), violations.is_empty()) {
            (PolicyDecision::Allow, true) if input.invert_available => {
                Ok(Verdict::Admit(AdmitProof::new(
                    outcome.determining().to_vec(),
                    results,
                    Vec::new(),
                    Vec::new(),
                    None,
                )))
            }
            // **E-M3-4**: nothing refused, and the adapter could not build an inverse (FR-043).
            (PolicyDecision::Allow, true) => escalate(&input),
            _ => {
                let mut reasons = outcome.deny_reasons()?;
                reasons.extend(violations);
                Verdict::deny(reasons)
            }
        }
    }
}

/// **E-M3-4**'s ticket: the change is admissible and cannot be undone, so a human is asked.
///
/// AC-048 verbatim: "when gx-gate receives `GateInput.invert_available=false`, it makes an
/// additional-approval request (**promotion to Escalate** or a policy firing an extra condition)"
/// (sem: SEM-gx-gate-071). The second half -- a policy firing an
/// extra condition -- is M4's; this is the first, and it is the whole of the generation rule in
/// v0.1. 45 TH-12's other trigger (a change that conflicts with a later one) has no input path:
/// commutation is not among 41 §4's five fields, and req/38 §19 keeps that as an M4 field-addition
/// proposal rather than growing the struct here.
///
/// # What fills the ticket, and what cannot
///
/// `reasons` carries one [`Reason`], coded [`INVERSE_UNAVAILABLE`] with
/// [`ReasonSource::Adapter`] -- the adapter is who answered `Ok(None)` to `invert` (41 §4), so it
/// is the truthful source. `required_approval` is [`ApprovalRequirement::default`], which *is*
/// ASM-60-3 (min_approvers 1, no restriction on who), and checking it is M5's (43 T-5).
///
/// `id` is computed here, which hands 1-3 could not do: 42 §1.3 gives `EscalationTicket` the
/// IdentityView `{transformation, reasons, required_approval}`, and with the gx-canon edge in place
/// the ticket can be identified by what it *is* rather than by a value a caller passed in.
///
/// `created_at` is the one field with no honest source. 41 §6 keeps clocks out of this layer
/// ("randomness and time are injected at the engine boundary") (sem: SEM-gx-gate-072) and `GateInput` carries none, so the value written is
/// `Timestamp(0)` -- the epoch, as a placeholder the engine overwrites when it records the ticket.
///
/// **2026-08-21, E-DR4627-1: the second clause of that sentence stopped being true.** `GateInput`
/// now carries [`GateInput::decided_at`], so a source for this field exists where none did. It is
/// deliberately **not** read here, and the reason is DR-46-27's whole shape: the moment `decided_at`
/// reaches a value inside a `Verdict`, varying it varies the verdict, and
/// `crates/gx-gate/tests/decided_at_seat.rs`'s differential test -- the machine form of "replay is
/// determinate" -- goes red. Wiring the ticket's timestamp is therefore a **separate decision with
/// its own evidence**, not a tidy-up to fold into a field addition. The stale sentence is kept above
/// rather than rewritten so that the change of state is visible (no-delete).
/// ASM-4 keeps `created_at` out of the IdentityView, so the placeholder reaches no CID and the
/// ticket's identity is unaffected; that it is nonetheless a field with no producer in M3 is
/// reported in req/64 §4.
///
/// # Errors
/// [`Error::NotDigestible`] if the ticket's projection has no canonical form.
fn escalate(input: &GateInput<'_>) -> Result<Verdict> {
    Ok(Verdict::Escalate(escalation_ticket(input.t.id)?))
}

/// 🔴 **The one road to an escalation ticket** (E-M3-4), as a function of the transformation alone.
///
/// `escalate` (private) called this inline until M6 hand 4, and the extraction is a measurement rather than
/// tidying. req/38 §50 asked hand 4 to settle **M6H3-10** by experiment first -- "measure whether an
/// `EscalationTicket` can be reconstructed from a row, then decide the journal-vocabulary-growth
/// option (a)" (sem: SEM-gx-gate-073) -- and the answer is yes, for a
/// reason that lives in this function's **signature**: every field of 42 §1.3's IdentityView
/// (`{transformation, reasons, required_approval}`) is either the argument or a constant of v0.1.
///
/// * `reasons` is one [`Reason`], `INVERSE_UNAVAILABLE`, with the same text every time;
/// * `required_approval` is [`ApprovalRequirement::default`], which *is* ASM-60-3;
/// * `created_at` is outside the view (ASM-4) and is injected by the engine.
///
/// ∴ a `TicketId` is a **pure function of a `TransformationId`** in v0.1, and a process that holds
/// only Σ can rebuild the ticket that a Σ-recorded `Verdict::Escalate` raised. That is why the
/// journal's vocabulary does not grow for M6H3-10, and it is why the function exists in the open
/// instead of inside `escalate`: the property is "there is exactly one generator, and it reads
/// nothing but the id" (sem: SEM-gx-gate-074), and a second generator -- or a first one that read the evidence -- would
/// break the reconstruction silently.
/// `probes/doubt/tests/m6_surface_doubt.rs::exactly_one_road_builds_an_escalation_ticket` counts the
/// roads, so the day AC-048's second half arrives (a policy firing its own escalation, M4's) the
/// count moves and somebody has to decide again.
///
/// # Errors
/// [`Error::NotDigestible`] if the ticket's projection has no canonical form.
pub fn escalation_ticket(transformation: TransformationId) -> Result<EscalationTicket> {
    let reason = Reason::new(
        INVERSE_UNAVAILABLE,
        "no inverse delta could be built for this change, so it cannot be undone once applied"
            .to_string(),
        ReasonSource::Adapter {
            detail: "invert returned None (FR-043)".to_string(),
        },
    )?;

    let mut ticket = EscalationTicket {
        id: TicketId(gx_core::Cid([0u8; 32])),
        transformation,
        reasons: vec![reason],
        required_approval: ApprovalRequirement::default(),
        created_at: gx_core::Timestamp(0),
    };
    ticket.id = TicketId(verdict::digest_of(&ticket)?);
    Ok(ticket)
}
