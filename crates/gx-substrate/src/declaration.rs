// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The optional input-generation declaration (DR-46-33 / DR-46-28).
//!
//! Spec: `req/38` §255 ruling 4 and `req/459` split one boundary into two stages, and `req/467`
//! (DR-46-28) landed the value a receipt carries. `req/590` / `req/38` §413 rule the wiring of the
//! **declaration** half — where a deployment says its inputs come from — into the engine. This
//! trait is that half's carrier.
//!
//! # 🔴 Why this is a separate trait and not an eighth method
//!
//! **N-08** (`req/69` §1): "do not grow `SubstrateAdapter` any methods beyond 41 §4's 7", and
//! `crates/gx-substrate/tests/adapter_spec.rs` measures the count against the canon. §413 rules the
//! same for this capability: an eighth method would move 41 §4, the conformance contract table and
//! every adapter at once, for a value only a catalogue-backed adapter can answer. A separate
//! optional trait touches none of them — an engine holds these behind its own registry
//! (`Engine::register_input_stage_declaration`), a substrate with no registration attests the
//! `unknown` `req/467` already shipped, and "seven methods, no eighth" stays a measured fact. This
//! is `InverseCompletion`'s decision one erratum over, taken for the same reason.
//!
//! # What the engine does with the answer
//!
//! One question, asked once, at plan time (43 T-2): "where does an input for this substrate come
//! from". The engine joins it with the transformation's `Actor` — an `Actor::Agent` is an LLM
//! origin whatever a static file declared (`gx_core::DeterminismBoundary::Mixed`'s
//! `input_generation` doc) — and journals the **join's result**, not the declaration. A rebuild
//! (43 §7-3b) then reproduces the boundary from that one value, without the actor (which is not in
//! Σ) and without reaching the catalogue (which the engine crate does not depend on).

use gx_core::BoundaryStage;

/// Where a deployment's inputs for this substrate come from, as one stage class.
///
/// Implemented by an adapter (or beside one) whose catalogue can declare it — `gx-adapter-mcp`'s
/// `$determinism_boundary` reserved slot — and registered with an engine **optionally**: an engine
/// with no declaration for a substrate attests `BoundaryStage::Unknown` for input generation, which
/// is the pre-existing v0 behaviour, unchanged.
///
/// `Send + Sync` for [`crate::SubstrateAdapter`]'s own reason (AC-046): the engine holds these
/// behind an `Arc<dyn InputStageDeclaration>` beside the adapter they declare for.
pub trait InputStageDeclaration: Send + Sync {
    /// The declared input-generation stage — a deployment's own statement, with no I/O and no
    /// clock, deterministic for one deployment (the value is a property of the catalogue this
    /// adapter was built with, P-6). `BoundaryStage::Unknown` is the honest default a deployment
    /// that declared nothing carries; the engine never mints a determinism claim over it.
    fn declared_input_stage(&self) -> BoundaryStage;
}
