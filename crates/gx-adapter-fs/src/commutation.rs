// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `commutation`: whether two changes are independent, decided from the payloads alone.
//!
//! Spec: 41 §4 for the method, 42 §3.6 for `Conflicts{residual}` ("`residual` stands for the `DeltaRef` that represents
//! the 'part that is not independent'"), 43 §8 for what an engine does with the answer, ASM-2 for why the question is
//! independence and not a difference. The rulings are **M4-25, adopted (a) + reflexive = Conflicts** (symmetry, and
//! `commutation(a,a)` is `Conflicts`), **M4-14** (the residual's referent), **E-M4-8** (a payload is
//! kept, which is what gives the referent somewhere to be) and **M4-13, adopted (a)** (the grammar that makes (sem: SEM-gx-adapter-fs-019)
//! the question decidable at all).
//!
//! # The whole question, in one line of this grammar
//!
//! Under **M4-13, adopted (a)** every operation is a whole-file replacement or a whole-file removal, so an (sem: SEM-gx-adapter-fs-020)
//! operation's footprint **is** its position. Two changes are then parallel-independent exactly when
//! they name two positions, and the comparison is made after normalisation because two spellings of
//! one path are one file (**E-M4-12**; M3-10 fixed a policy pack's effective range at the "locator level", so (sem: SEM-gx-adapter-fs-021)
//! an adapter that compared the spellings would let a gate see two subjects for one file).
//!
//! That is why nothing here reads a substrate. It is a property of this grammar rather than a virtue
//! of this module -- an adapter whose deltas were patches would have to know what a patch touched --
//! and it is what makes AC-053's "outside the engine pipeline" easy: there is nothing a pipeline could supply. (sem: SEM-gx-adapter-fs-022)
//!
//! # Symmetry (**M4-25, adopted (a)**), and where the symmetry stops (sem: SEM-gx-adapter-fs-023)
//!
//! The **verdict** is symmetric by construction: it is `position(a) == position(b)`, a question about
//! an unordered pair, computed in one place. Neither direction has a branch the other lacks, which is
//! stronger than two arms that agree today.
//!
//! 🔴 The **residual** is not, and cannot be. `Conflicts` carries the change that is held back, and
//! "held back" is a fact about an order: 43 §8 has the engine keep `T2` in a waiting queue with (sem: SEM-gx-adapter-fs-024)
//! `blocked_by: T1`, and it calls `adapter.commutation(T1.delta, T2.delta)`. So `commutation(a, b)`
//! names `b` and `commutation(b, a)` names `a`. The two readings this leaves open are raised in
//! `req/75` §2 rather than settled here:
//!
//! * the trait's contract row (hand 2) writes the obligation as `commutation(a,b) == commutation(b,a)`
//!   -- **value** equality, which this implementation does not satisfy;
//! * the shared harness's **L6** (hand 3) compares the two as *answers* and says in its own comment
//!   that "M4-25 says nothing about the two directions naming the same obstruction" -- which this (sem: SEM-gx-adapter-fs-025)
//!   implementation does satisfy, as does the mock, which is biased the other way (it mints the
//!   residual from its **first** argument).
//!
//! Two artefacts of this workspace read one ruling differently, and neither of them is this hand's to
//! change (no ruling permitted here). (sem: SEM-gx-adapter-fs-026)
//!
//! # Why the residual is the whole of the second delta
//!
//! 42 §3.6 calls it "the part that is not independent". For whole-file replacement there is no proper part: if two (sem: SEM-gx-adapter-fs-027)
//! changes meet at one position, **nothing** of the later one is independent of the earlier. So the
//! part is the whole, and the residual is `b` re-minted at the normalised spelling of its position --
//! which for everything [`crate::plan`] writes is `b`'s own reference, byte for byte.
//!
//! **M4-14** asked for "the residual CID be resolvable within the test harness" and this is what answers it. There
//! is no delta store in M4 (that is the engine's, in M5), so "resolvable" cannot mean a lookup; it means (sem: SEM-gx-adapter-fs-028)
//! the CID has a referent somebody holds. It does: **E-M4-8** requires a `PlannedDelta.payload` to be
//! kept, and the residual names a delta the caller passed in. `tests/ac_052.rs` measures both halves --
//! the reference agrees with the delta it names, and a reference nobody minted is not it.

use gx_core::{Commutation, DeltaRef, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{FsDelta, FsOp};

/// Decide whether two deltas are independent (41 §4, ASM-2, C-4).
///
/// # Errors
/// [`Error::ForeignDelta`] when either delta belongs to another adapter, [`Error::PayloadUnreadable`]
/// for bytes this grammar did not write, [`Error::Unimplemented`] for a sequence longer than v0.1
/// runs, [`Error::NotAPosition`] when a payload's locator is not a position, and
/// [`Error::NotDigestible`] if the residual has no canonical form.
///
/// Both arguments are read before anything is decided, so a refusal is never the answer to half the
/// question. Which refusal comes back when both are wrong depends on the order -- that is a property
/// of `?` and not a statement about the pair -- and it does not reach the verdict, which is computed
/// from two values that both parsed.
pub(crate) fn commutation(a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
    let left = operation_of(a)?;
    let right = operation_of(b)?;

    if left.0 != right.0 {
        return Ok(Commutation::Commutes);
    }
    Ok(Commutation::Conflicts {
        residual: mint(&right)?,
    })
}

/// The position a delta acts on, and the operation it performs there.
///
/// One helper for both arguments, which is the mechanical half of the symmetry: a verdict computed
/// from two values produced by one function cannot depend on which slot they arrived in.
fn operation_of(delta: &PlannedDelta) -> Result<(String, FsOp)> {
    if delta.substrate() != &SubstrateKind::Fs {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Fs,
            got: delta.substrate().clone(),
        });
    }
    let decoded = FsDelta::decode(delta.payload())?;
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
/// The re-mint rather than `b.reference()` is deliberate: a payload written by hand can spell a
/// position any way L7 admits, and a residual naming an unnormalised delta would put a second name
/// for one change into a receipt. For everything `plan` writes the two are the same value, which
/// `tests/ac_052.rs` asserts rather than assumes.
fn mint(operation: &(String, FsOp)) -> Result<DeltaRef> {
    let (position, op) = operation;
    let restated = match op.content() {
        Some(content) => FsOp::write(position.clone(), content.to_vec()),
        None => FsOp::remove(position.clone()),
    };
    let payload = FsDelta::one(restated).encode()?;
    Ok(PlannedDelta::new(SubstrateKind::Fs, payload)?
        .reference()
        .clone())
}
