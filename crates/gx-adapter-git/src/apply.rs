// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `apply`: the two operations, and the short circuit that makes a retry a retry.
//!
//! Spec: 41 §4 ("only called after commit approval", "apply is designed to be idempotent"), 51 §7 contract 7, 43 T-10c for what (sem: SEM-gx-adapter-git-017)
//! the idempotence is *for*, 42 §3.4 for the observation that comes back. The rulings are **E-M4-3**
//! (the quantifier), **E-M4-31** (`applied_at` is the engine's) and **M4-17** (no clock here).
//!
//! # The quantifier, and the two short circuits that implement it
//!
//! **E-M4-3**: idempotence is quantified over "**the same delta re-entering** (retry)" and not over every state. (sem: SEM-gx-adapter-git-018)
//! 43 T-10c is the situation it exists for — a crash between the write and the journal record, whose
//! recovery is running the same delta again — so what has to hold is that the second run leaves the
//! repository, and the observation, exactly where the first did.
//!
//! Both operations reach that by asking whether the change is **already made** before making it:
//!
//! | operation | already made when | second run |
//! |---|---|---|
//! | `content` | the entry at the path already holds these bytes | writes nothing, moves nothing |
//! | `reset` | the branch already points at this commit | writes nothing, moves nothing |
//!
//! Without the short circuit a `content` retry would rebuild the same tree onto the **new** tip and
//! mint a second commit, so the branch would move twice for one delta. That is the failure this table
//! is here to prevent, and `tests/git_conformance.rs` is where it is measured rather than reasoned
//! about.
//!
//! 🔴 A consequence worth stating: "already made" is a fact about the **state**, so a `content` apply (sem: SEM-gx-adapter-git-019)
//! is also a no-op when somebody else independently gave the entry those bytes. That is the meaning
//! of the quantifier and not a hole in it — the delta says what the state should be, and it is.
//!
//! # What comes back is an observation
//!
//! 41 §4 gives `apply` no pre-state and no post-state, so [`AppliedDelta`] carries what the adapter
//! *saw* afterwards (req/69 §3.1: "post is an observation, not a return value"): the digest of the entry, read back (sem: SEM-gx-adapter-git-020)
//! out of the object database, and a fingerprint of the **branch**. Both are read after the reference
//! moved, through the same functions `snapshot` and `precondition` use, so a bug that wrote the
//! change somewhere else shows up as a digest that is not the one the plan promised (L5).

use gx_core::{Fingerprint, SubstrateKind, Timestamp};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result};

use crate::delta::{GitDelta, GitOp, KIND_CONTENT, KIND_RESET};
use crate::locator::Position;
use crate::repo;

/// Perform a delta a gate has already admitted (41 §4).
///
/// # Errors
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::Unimplemented`] for a sequence longer than v0.1 runs,
/// [`Error::NotAPosition`] for a payload whose locator is not a position, [`Error::Unreadable`] when
/// the repository will not answer, and [`Error::ApplyFailed`] when an object or the reference cannot
/// be written. 43 T-11 turns the last into `AbortReason::ApplyFailed`.
pub(crate) fn apply(delta: &PlannedDelta) -> Result<AppliedDelta> {
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
        .expect("decode refuses the empty sequence");
    let position = op.position()?;
    let repository = repo::open(&position)?;

    match op.kind() {
        KIND_CONTENT => apply_content(&repository, &position, op)?,
        KIND_RESET => apply_reset(&repository, &position, op)?,
        // `GitDelta::decode` and `GitOp::well_formed` have already refused every other spelling; the
        // arm exists because a `match` on a `&str` cannot be exhaustive and 41 §6 counts a panic as a
        // bug.
        other => {
            return Err(Error::PayloadUnreadable {
                detail: format!("this grammar writes no {other:?} operation"),
            })
        }
    }

    observe(&repository, &position, delta)
}

/// "The entry at this path becomes these bytes" -- AC-050's "commit-operation delta". (sem: SEM-gx-adapter-git-021)
fn apply_content(repository: &gix::Repository, position: &Position, op: &GitOp) -> Result<()> {
    let tip = repo::tip(repository, position)?;
    let current = match tip {
        Some(tip) => repo::entry_content(repository, tip, position)?,
        None => None,
    };
    if current.as_deref() == op.content() {
        // The retry, and the reason the table in the module documentation exists.
        return Ok(());
    }
    let commit = repo::commit_entry(repository, tip, position, op.content())?;
    repo::move_branch(repository, position, tip, commit)
}

/// "This branch points at this commit" -- AC-050's "branch-operation delta", and what an inverse is. (sem: SEM-gx-adapter-git-022)
fn apply_reset(repository: &gix::Repository, position: &Position, op: &GitOp) -> Result<()> {
    let target = repo::object_id(op.target().ok_or_else(|| Error::PayloadUnreadable {
        detail: "a reset operation names the commit it resets to".to_string(),
    })?)?;
    let tip = repo::tip(repository, position)?;
    if tip == Some(target) {
        return Ok(());
    }
    repo::move_branch(repository, position, tip, target)
}

/// What the adapter saw after the reference moved (42 §3.4).
///
/// The object's digest and the branch's fingerprint, read back rather than remembered. An absent
/// entry answers [`repo::absent_digest`], whose collision with an empty entry is disclosed there.
fn observe(
    repository: &gix::Repository,
    position: &Position,
    delta: &PlannedDelta,
) -> Result<AppliedDelta> {
    let tip = repo::tip(repository, position)?;
    let digest = match tip {
        Some(tip) => match repo::entry_content(repository, tip, position)? {
            Some(content) => repo::content_digest(&content),
            None => repo::absent_digest(),
        },
        None => repo::absent_digest(),
    };
    let scope_digest = match tip {
        Some(tip) => repo::content_digest(tip.as_bytes()),
        None => repo::absent_digest(),
    };
    let postcondition = Fingerprint::new(
        SubstrateKind::Git,
        elide_scope(position.scope())?,
        scope_digest,
    )?;
    Ok(AppliedDelta::new(
        delta.reference().clone(),
        postcondition,
        digest,
        // **E-M4-31** (req/38 §31): "`applied_at` is overwritten by the engine at commit time... the adapter writes a
        // `Timestamp(0)` placeholder". The same value this adapter writes into a commit header, and (sem: SEM-gx-adapter-git-023)
        // for the same reason (41 §6): the moment is the engine's to inject.
        Timestamp(0),
    ))
}
