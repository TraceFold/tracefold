// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! References to why a transformation was admitted: a Lean theorem, or a checker's result.
//!
//! Spec: 32 FR-017 for the requirement, 34 AC-017 for its test, 42 §3.8 for `ProofRef`, 46 §2.5
//! for the five theorems of F0, **E-M2-12** (`req/38_ERRATA_2026-08-07.md` §8) for the crate.
//!
//! # Why these are in gx-core
//!
//! FR-017 asks gx-witness for a `Proof`; 42 §0 puts the nearly identical `ProofRef` in
//! `gx-gate/verdict.rs` (M3). Two crates, two names, one idea -- req/49 §3 M2-3 raised it and
//! E-M2-12 rules it down here with the rest of E-M2-1's data, so gx-witness and gx-gate refer to
//! one set of types rather than to two spellings that can drift apart.
//!
//! # A proof reference is not a proof
//!
//! 46 §2.5 lists T1..T5 as F0's theorems, and `req/38_ERRATA_2026-08-07.md` §7 records what is
//! actually proved: `Admissible.lean` (T1) and `Canon.lean` (T3) exist, `T2_hoare_seq` is a
//! **different model** from 46 §2.3's Hoare rule, and `Invariant.lean` / `Receipt.lean` do not
//! exist at all -- so T2, T4 and T5 are unproven. Citing [`TheoremId::T4`] in a value of this type
//! records that a receipt *claims* T4's shape, not that T4 holds. The claim becomes checkable in
//! M8, where 46 §3.1's `receipt_verify` differential vectors run against the Lean side.

use crate::Cid;
use serde::{Deserialize, Serialize};

/// One of the five theorems of F0 (46 §2.5: "fixed at five"; sem: SEM-gx-core-077).
///
/// # Why an enum where 42 §3.8 writes `Vec<String>`
///
/// A `String` admits `"T6"`, and F0 has five theorems. AC-017 asks that all five be "individually
/// representable and recoverable" (sem: SEM-gx-core-078), which a free string satisfies only in the
/// sense that it also represents theorems that do not exist. This is E-FR055-1's move
/// (`req/38_ERRATA_2026-08-07.md` §6) applied to an identifier: "a flag can diverge from the
/// implementation" (sem: SEM-gx-core-079) was the reason a bool registry lost to a marker trait,
/// and a string identifier can diverge from the theorem list the same way.
///
/// The wire form does not change: each variant serializes as `"T1"`..`"T5"`, exactly what a
/// `Vec<String>` holding valid ids would carry. Only the invalid values are refused, which is the
/// erratum this raises against 42 §3.8 (req/50 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TheoremId {
    /// Composition preservation: `A(f) ∧ A(g) → A(g∘f)` (46 §2.2). Proved. (sem: SEM-gx-core-080)
    T1,
    /// Invariant composition: `{I}f{J} ∧ {J}g{K} → {I}(g∘f){K}` (46 §2.3). **Unproven** -- what Lean
    /// holds is a different composition model (`req/38_ERRATA_2026-08-07.md` §7). (sem: SEM-gx-core-081)
    T2,
    /// Canonicalisation idempotence + representation independence (46 §2.4). Proved. (sem: SEM-gx-core-082)
    T3,
    /// Receipt soundness (46 §2.5). **Unproven** -- `Receipt.lean` does not exist. (sem: SEM-gx-core-083)
    T4,
    /// Witness lax composition (46 §2.5). **Unproven** -- same file. (sem: SEM-gx-core-084)
    T5,
}

impl TheoremId {
    /// Every theorem of F0, in order. 46 §2.5 fixes the count at five, so a caller that wants to
    /// cover them all iterates this rather than writing the list a second time -- the shape
    /// AC-017 takes in hand 4.
    pub const ALL: [TheoremId; 5] = [
        TheoremId::T1,
        TheoremId::T2,
        TheoremId::T3,
        TheoremId::T4,
        TheoremId::T5,
    ];
}

/// A reference to the Lean formalisation (42 §3.8).
///
/// `lean_spec_version` is what makes the citation checkable later: a theorem id alone does not say
/// which text was proved, and 46 §3 compares implementations against a specific `GxSpec`.
/// An empty `theorem_ids` is legal and is the honest value today -- see this module's header.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProofRef {
    /// Which `GxSpec` text the cited theorems were proved against (46 §3) -- the field that
    /// makes the citation checkable rather than a bare name.
    pub lean_spec_version: String,
    /// The theorems cited. Empty is legal and is the honest value today (see the module doc:
    /// three of the five are unproven).
    pub theorem_ids: Vec<TheoremId>,
}

/// A reference to a checker's verification result.
///
/// FR-017's text names two forms -- "a reference structure to a Lean theorem or to a checker's
/// verification result" (sem: SEM-gx-core-085) -- and gives a
/// field table for neither. This one is derived rather than quoted, and req/50 §4 raises it: the
/// shape follows 42 §1.3's standing principle that a witness carries a digest and not a body (the
/// same reason `AdmitProof.evidence_digests` is `Vec<Cid>` and `verdict.proof_digest` is a `Cid`),
/// plus the identity of the checker, without which a digest names a result nobody can locate.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CheckerResultRef {
    /// Which checker produced the result, e.g. an `InvariantCheck::id()` (41 §4) or a policy engine.
    pub checker_id: String,
    /// Which theorems of F0 the result is offered against. May be empty for a checker that
    /// discharges no F0 theorem -- most do not.
    pub theorem_ids: Vec<TheoremId>,
    /// The result itself, by digest (42 §1.3).
    pub result_digest: Cid,
}

/// Why a verdict may be believed: FR-017's two forms.
///
/// Not a `Verdict` and not an `AdmitProof` -- both of those are gx-gate (M3, 42 §3.8), and this is
/// the reference an `AdmitProof.proof_ref` and a receipt both point through.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Proof {
    /// FR-017's first form: a citation of the Lean formalisation.
    Lean(ProofRef),
    /// FR-017's second form: a reference to a checker's verification result.
    Checked(CheckerResultRef),
}
