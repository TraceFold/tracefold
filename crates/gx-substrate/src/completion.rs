// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The optional late-binding escrow completion boundary (two-phase escrow, DR-1 option A). (sem:
//! SEM-gx-substrate-054, SEM-gx-substrate-055, SEM-gx-substrate-056, SEM-gx-substrate-057,
//! SEM-gx-substrate-058, SEM-gx-substrate-059)
//!
//! Spec: `req/160` §1-1 (the five-step mechanism) and `req/38` §98 ruling 1 / §99 ruling 2-② are the
//! rulings this trait implements. The problem it exists for is physical: **E-M4-30** constructs an
//! escrow **before** `apply` (43 T-10b) because pre-state material is only observable then — and a
//! *do-time server-assigned value* (a created issue's number, `req/153` §6) is, by definition, not
//! pre-state material. It does not exist yet. So a declared inverse that needs one can only be
//! escrowed as a **partial** delta and completed after `apply`, inside the same Committing critical
//! section, from the call's own observed answer.
//!
//! # 🔴 Why this is a separate trait and not an eighth method
//!
//! **N-08** (req/69 §1): "do not grow `SubstrateAdapter` any methods beyond 41 §4's 7", and
//! `crates/gx-substrate/tests/adapter_spec.rs` measures the count against the canon. §99 ruling 2-②
//! rules the same for this capability: an eighth method would move 41 §4, the conformance contract
//! table and every adapter at once, for a capability only one adapter (today) has. A separate
//! optional trait touches none of them — an engine holds these behind its own registry
//! (`Engine::register_completion`), a substrate with no registration behaves exactly as before,
//! and "seven methods, no eighth" stays a measured fact.
//!
//! # What the engine does with the two answers
//!
//! `needs_completion` is asked at T-10b, after `invert` answered `Some`: `true` marks the escrow
//! row `Pending` and journals the flag (`InverseEscrowed { pending: true }`). `complete_inverse`
//! is asked once, after a successful `apply`, with the journalled observation — and **every**
//! failure of that step (an `Err`, an `Ok(None)`, a missing observation) folds to
//! `InverseCompleted { inverse_cid: None }` → `InverseStatus::Unavailable` while the commit
//! continues (§99 ruling 2-④): the world has already moved, and aborting after a successful apply
//! would record "Aborted" about a change that happened.

use crate::delta::PlannedDelta;
use crate::error::Result;

/// Completes a partial escrowed inverse from the applied call's observed answer.
///
/// Implemented by an adapter (or beside one) whose catalogue vocabulary can declare do-time
/// members (`gx-adapter-mcp`'s `do_result` / `do_result_number_from`), and registered with an
/// engine **optionally**: an engine with no registration for a substrate treats every `Some`
/// answer of `invert` as a complete escrow, which is the pre-existing behaviour, unchanged.
///
/// `Send + Sync` for [`crate::SubstrateAdapter`]'s own reason (AC-046): the engine holds these
/// behind an `Arc<dyn InverseCompletion>` beside the adapter they complete for.
pub trait InverseCompletion: Send + Sync {
    /// Whether this inverse delta is a **partial** escrow that will need the applied call's
    /// observation to become executable.
    ///
    /// A pure question about the payload (the implementor's own grammar, P-6): no I/O, no clock,
    /// deterministic for one delta. The engine asks it at T-10b and journals the answer.
    ///
    /// # Errors
    /// When the payload is not this implementor's grammar — the same refusal `invert` gives a
    /// foreign delta. Asked before `apply`, so an `Err` here fails the commit closed rather than
    /// after the world moved.
    fn needs_completion(&self, inverse: &PlannedDelta) -> Result<bool>;

    /// Resolve the partial escrow's remaining members from the observation and mint the complete
    /// inverse delta.
    ///
    /// `Ok(None)` is "the observation does not carry what the declaration names" — a legitimate
    /// non-construction, the same family as E-M4-32's `Ok(None)` — and the engine folds it (and
    /// every `Err`) to `InverseStatus::Unavailable` with the commit continuing (§99 ruling 2-④).
    /// The failure is visible, not silent: the journal carries `InverseCompleted { None }` and the
    /// receipt carries `inverse_delta: None` beside its Admit — the failure's fingerprint.
    ///
    /// # Errors
    /// When the partial payload is not this implementor's grammar. The engine treats `Err` and
    /// `Ok(None)` identically at this stage (fail-safe fold; no abort after a successful apply).
    fn complete_inverse(
        &self,
        partial: &PlannedDelta,
        observation: &[u8],
    ) -> Result<Option<PlannedDelta>>;
}
