// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `commutation`: whether two calls are independent, decided from the payloads alone.
//!
//! Spec: 41 §4 for the method, 42 §3.6 for `Conflicts{residual}` ("`residual` denotes 'the part that is not independent', a (sem: SEM-gx-adapter-mcp-074)
//! `DeltaRef`"), 43 §8 for what an engine does with the answer, ASM-2 for why the question is (sem: SEM-gx-adapter-mcp-075)
//! independence and not a difference. The rulings are **M4-25, adopted (a) + reflexive = Conflicts**, **M4-14** (the (sem: SEM-gx-adapter-mcp-076)
//! residual's referent) and **E-M4-8** (a payload is kept, which gives the referent somewhere to be).
//!
//! # 🔴 The footprint of a tool call is its **server**, and here that is an admission of ignorance
//!
//! `gx-adapter-fs` compares positions, because a whole-file replacement's footprint is its file.
//! `gx-adapter-git` compares branches, because rewriting one entry rewrites a tree, mints a commit and
//! moves the reference -- a **derivable** fact about what a change touches.
//!
//! Neither road is open here. What a tool does is the server's semantics: `write_file` touches the file
//! it names, `reindex` touches everything, `send_email` touches nothing gx can read, and **the protocol
//! offers a proxy no map from a tool to the resources it disturbs**. So this compares
//! [`crate::locator::Position::server`] -- the whole server -- and the answer is `Conflicts` for any
//! two changes on one server.
//!
//! That is the conservative side, which is the side M4-25 fixed the reflexive case at for the same
//! reason: `Commutes` is **fail-open**, and 43 §8 acts on the answer by letting both proceed. An adapter
//! that compared resources would answer `Commutes` for two calls to one server and would be trusting a
//! map it does not have.
//!
//! **The cost is not softened.** Two changes to two different resources on one MCP server conflict, and
//! one of them waits. That is not a limitation of this version -- it is what a proxy in front of opaque
//! effects can honestly say -- and the day a server publishes what its tools touch is the day this
//! comparison could narrow. `req/101` records that as the firing condition rather than as a wish.
//!
//! # 🔴 What actually reaches this function, today (v0.2.8 cleanup batch, gotcha64 / `req/38` §74, ruling 4) (sem: SEM-gx-adapter-mcp-077)
//!
//! The paragraphs above are what this function decides, not what asks it. `gx-engine` calls
//! [`commutation`] only for two deltas that already share one `Subject`
//! (`Engine::conflicting_predecessor`'s `by_subject` siblings, M6-07's index), and a `Subject` is
//! minted **per resource**, not per server: two transformations on the same server but different
//! resources never share a `Subject` and so never reach this comparison. So the server-wide
//! `Conflicts` this module computes is correct wherever it is asked, and it is the index upstream
//! of it that decides how often that is -- "a change on the same server is serialised" is true of what this (sem: SEM-gx-adapter-mcp-078)
//! function would answer, not of what happens across two different resources on one server today.
//! Widening `by_subject` to server granularity is a conflict-domain question (41/42/43) this
//! adapter does not decide, and it is tracked as a spec question rather than implemented here.
//! `45-threat-model.md` §3.1, candidate 14, already names the family this narrowing sits in (a tool call's (sem: SEM-gx-adapter-mcp-079)
//! effect landing outside the object the transformation is about, invisible to the CAS) and calls
//! what this module offers against it "serialisation, not detection" -- this doc addition is the (sem: SEM-gx-adapter-mcp-080)
//! same limit, said one layer closer to the code that has it.
//!
//! Nothing here reads a server. That is a property of this grammar rather than a virtue of this module
//! -- the footprint is in the locator -- and it is what makes AC-053's "outside the engine pipeline" easy: there (sem: SEM-gx-adapter-mcp-081)
//! is nothing a pipeline could supply.
//!
//! # Symmetry (**M4-25, adopted (a)**), and where the symmetry stops (sem: SEM-gx-adapter-mcp-082)
//!
//! The **verdict** is symmetric by construction: it is `server(a) == server(b)`, a question about an
//! unordered pair, computed in one place from two values one function produced.
//!
//! 🔴 The **residual** is not, and cannot be. `Conflicts` carries the change that is held back, and
//! "held back" is a fact about an order: 43 §8 has the engine keep `T2` waiting with `blocked_by: T1` (sem: SEM-gx-adapter-mcp-083)
//! and calls `adapter.commutation(T1.delta, T2.delta)`. So `commutation(a, b)` names `b`. That satisfies
//! the harness's **L6** (which compares the two directions as *answers*) and not the literal `==` in
//! the trait's contract row -- the two readings `gx-adapter-fs` raised in `req/75` §2, and this adapter
//! inherits the seam rather than deciding it (no ruling permitted). (sem: SEM-gx-adapter-mcp-084)

use gx_core::{Commutation, DeltaRef, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{McpDelta, McpOp};
use crate::locator::Position;

/// Decide whether two deltas are independent (41 §4, ASM-2, C-4).
///
/// # Errors
/// [`Error::ForeignDelta`] when either delta belongs to another adapter, [`Error::PayloadUnreadable`]
/// for bytes this grammar did not write, [`Error::Unimplemented`] for a sequence longer than v0.1 runs,
/// [`Error::NotAPosition`] when a payload's locator is not a position, and [`Error::NotDigestible`] if
/// the residual has no canonical form.
///
/// Both arguments are read before anything is decided, so a refusal is never the answer to half the
/// question.
pub(crate) fn commutation(a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
    let left = operation_of(a)?;
    let right = operation_of(b)?;

    if left.0.server() != right.0.server() {
        return Ok(Commutation::Commutes);
    }
    Ok(Commutation::Conflicts {
        residual: mint(&right)?,
    })
}

/// The position a delta acts on, and the operation it performs there.
///
/// One helper for both arguments, which is the mechanical half of the symmetry: a verdict computed from
/// two values produced by one function cannot depend on which slot they arrived in.
fn operation_of(delta: &PlannedDelta) -> Result<(Position, McpOp)> {
    if delta.substrate() != &SubstrateKind::Mcp {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Mcp,
            got: delta.substrate().clone(),
        });
    }
    let decoded = McpDelta::decode(delta.payload())?;
    let op = decoded
        .ops()
        .first()
        .expect("decode refuses the empty sequence")
        .clone();
    let position = op.position()?;
    Ok((position, op))
}

/// Re-mint an operation at its normalised position, and name the delta it makes.
///
/// The re-mint rather than `b.reference()` is deliberate, and it is `gx-adapter-fs`'s reason: a payload
/// written by hand can spell a position any way L7 admits, and a residual naming an unnormalised delta
/// would put a second name for one change into a receipt. For everything [`crate::plan`] writes the two
/// are the same value, which `tests/mcp_commutation.rs` asserts rather than assumes.
///
/// 🔴 The residual is the **whole** of the second delta. 42 §3.6 calls it "the part that is not independent", and on one (sem: SEM-gx-adapter-mcp-085)
/// server there is no proper part this adapter can name: it does not know which resources the first
/// call touched, so it cannot say which part of the second is unaffected.
fn mint(operation: &(Position, McpOp)) -> Result<DeltaRef> {
    let (position, op) = operation;
    let restated = McpOp::call(
        position.locator(),
        op.tool().to_string(),
        op.arguments().to_vec(),
    );
    let payload = McpDelta::one(restated).encode()?;
    Ok(PlannedDelta::new(SubstrateKind::Mcp, payload)?
        .reference()
        .clone())
}
