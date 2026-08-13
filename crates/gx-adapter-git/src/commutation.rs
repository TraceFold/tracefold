//! `commutation`: whether two changes are independent, decided from the payloads alone.
//!
//! Spec: 41 §4 for the method, 42 §3.6 for `Conflicts{residual}` (「`residual`は「独立でない部分」を表す
//! `DeltaRef`」), 43 §8 for what an engine does with the answer, ASM-2 for why the question is
//! independence and not a difference. The rulings are **M4-25 採(a)+反射=Conflicts**, **M4-14** (the
//! residual's referent) and **E-M4-8** (a payload is kept, which gives the referent somewhere to be).
//!
//! # 🔴 The footprint of a git change is its **branch**, and this is where that costs something
//!
//! `gx-adapter-fs` answers 「do these two name two positions?」, because a whole-file replacement's
//! footprint is its file. A git change's footprint is larger than its object: rewriting one entry
//! rewrites its tree, mints a commit and **moves the branch**, so two changes to two different files
//! on one branch are not parallel-independent — the second has to be rebuilt on the first.
//!
//! So this compares [`crate::locator::Position::scope`], which is the branch, and the crate root
//! argues the design at length. The consequence is concrete and is not softened here: **two changes
//! to two different files on one branch conflict**, and 43 §8 makes one of them wait. That is the true
//! shape of a branch. The alternative — comparing the whole locator — would answer `Commutes` for
//! them, and `Commutes` is the **fail-open** direction (M4-25's reason for fixing the reflexive case
//! at `Conflicts`): 43 §8 acts on the answer by letting both proceed, and the second commit would then
//! be built on a tip the first had already moved.
//!
//! Nothing here reads a repository. That is a property of this grammar rather than a virtue of this
//! module — the footprint is in the locator — and it is what makes AC-053's 「engineパイプライン外」
//! easy: there is nothing a pipeline could supply.
//!
//! # 対称 (**M4-25 採(a)**), and where the symmetry stops
//!
//! The **verdict** is symmetric by construction: it is `scope(a) == scope(b)`, a question about an
//! unordered pair, computed in one place from two values one function produced.
//!
//! 🔴 The **residual** is not, and cannot be. `Conflicts` carries the change that is held back, and
//! 「held back」 is a fact about an order: 43 §8 has the engine keep `T2` waiting with `blocked_by: T1`
//! and calls `adapter.commutation(T1.delta, T2.delta)`. So `commutation(a, b)` names `b`. That
//! satisfies the harness's **L6** (which compares the two directions as *answers*) and not the literal
//! `==` in the trait's contract row — the same two readings `gx-adapter-fs` raised in `req/75` §2, and
//! this adapter inherits the seam rather than deciding it (裁定禁).

use gx_core::{Commutation, DeltaRef, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{GitDelta, GitOp};
use crate::locator::Position;

/// Decide whether two deltas are independent (41 §4, ASM-2, C-4).
///
/// # Errors
/// [`Error::ForeignDelta`] when either delta belongs to another adapter, [`Error::PayloadUnreadable`]
/// for bytes this grammar did not write, [`Error::Unimplemented`] for a sequence longer than v0.1
/// runs, [`Error::NotAPosition`] when a payload's locator is not a position, and
/// [`Error::NotDigestible`] if the residual has no canonical form.
///
/// Both arguments are read before anything is decided, so a refusal is never the answer to half the
/// question.
pub(crate) fn commutation(a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
    let left = operation_of(a)?;
    let right = operation_of(b)?;

    if left.0.scope() != right.0.scope() {
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
fn operation_of(delta: &PlannedDelta) -> Result<(Position, GitOp)> {
    if delta.substrate() != &SubstrateKind::Git {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Git,
            got: delta.substrate().clone(),
        });
    }
    let decoded = GitDelta::decode(delta.payload())?;
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
/// The re-mint rather than `b.reference()` is deliberate, and it is `gx-adapter-fs`'s reason: a
/// payload written by hand can spell a position any way L7 admits, and a residual naming an
/// unnormalised delta would put a second name for one change into a receipt. For everything
/// [`crate::plan`] writes the two are the same value, which `tests/git_commutation.rs` asserts rather
/// than assumes.
///
/// 🔴 The residual is the **whole** of the second delta. 42 §3.6 calls it 「独立でない部分」, and on one
/// branch there is no proper part: every operation this grammar writes moves the branch, so nothing of
/// the later change is independent of the earlier one.
fn mint(operation: &(Position, GitOp)) -> Result<DeltaRef> {
    let (position, op) = operation;
    let restated = match (op.kind(), op.content(), op.target()) {
        (crate::delta::KIND_RESET, _, Some(target)) => {
            GitOp::reset(position.locator(), target.to_string())
        }
        (_, Some(content), _) => GitOp::write(position.locator(), content.to_vec()),
        _ => GitOp::remove(position.locator()),
    };
    let payload = GitDelta::one(restated).encode()?;
    Ok(PlannedDelta::new(SubstrateKind::Git, payload)?
        .reference()
        .clone())
}
