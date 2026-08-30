// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `invert`: the branch goes back where it was, built while it is still there.
//!
//! Spec: 41 §4 for the method and DR-1(a), 42 §5 for why an escrow carries a body, 34 AC-048 and
//! **AC-050** for what is measured. The rulings are **E-M4-30** (the escrow is constructed **before**
//! `apply`), **E-M4-3** (the round trip is quantified at the one `pre` handed in), **E-M4-32** (which
//! facts may take the `Ok(None)` form) and **E-M4-5** (an engine folds `Some`/`None` into
//! `GateInput.invert_available` at verify time).
//!
//! # 🔴 Why the inverse is a reset and not a revert
//!
//! **AC-050**, verbatim: "`apply`->`invert`->`apply(inverse)`. Then: the repo's **HEAD and tree hash** return
//! bit-equal to before the operation (2 cases)". A revert commit restores the *tree* and moves HEAD **forward**, so it (sem: SEM-gx-adapter-git-046)
//! satisfies half of that sentence and fails the other half. Only putting the reference back where it
//! was returns both, so that is what an inverse of this adapter is: a `reset` operation naming the tip
//! the branch had at the escrow point.
//!
//! What this does **not** do is unmake the commit. The objects stay in the database and become
//! unreachable; `git reflog` shows both moves. So an undo here means "the branch is where it was" and
//! not "the change never existed", and the crate root says so where a reader of the disclosure will (sem: SEM-gx-adapter-git-047)
//! find it.
//!
//! # Why this module reads the repository, and why the order is not negotiable
//!
//! **E-M4-30** (req/38 §31 M4H3-1, adopted (a)), verbatim: "escrow (invert) comes before apply (43 T-10b is state machine's
//! canonical source)... **invert can only be constructed at the point where pre is observable**". This adapter keeps no history: the tip a
//! reset has to name is "where the branch is now", and after `apply` it is somewhere else. So `invert` (private) (sem: SEM-gx-adapter-git-048)
//! reads the reference, and 43 T-10b requires the caller to have called it first.
//!
//! 🔴 What git changes about 42 §5's reasoning is worth stating, because it is the one place this
//! adapter is **structurally** better off than the filesystem one. 42 §5 requires the escrowed inverse
//! to carry the body -- "because a digest-only inverse makes an actual undo physically impossible" -- and `gx-adapter-fs` therefore (sem: SEM-gx-adapter-git-049)
//! carries the whole old file and declares a ceiling on it (**M4-21**). A git object database is
//! content-addressed and keeps what it was given, so **the body is already escrowed**: the inverse
//! carries an object id and the undo is a pointer move. The ceiling that fires for the fs adapter has
//! no input here that could reach it, and 52 contract 2 forbids inventing a refusal nobody asked for, so (sem: SEM-gx-adapter-git-050)
//! this adapter declares no escrow ceiling — and the `Ok(None)` of 51 §7 contract 5 has a different
//! instance instead.

use gx_core::{ObjectSnapshot, ReadEntry, SubstrateKind};
use gx_substrate::{Error, InvertOutcome, PlannedDelta, Result};

use crate::delta::{GitDelta, GitOp};
use crate::locator;
use crate::repo;

/// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
///
/// # The one reason for `Ok(None)` in this adapter
///
/// 41 §4 separates "the question itself cannot be answered" (`Err`) from "the answer is no inverse"
/// (`Ok(None)`), and **E-M3-4** makes the second an escalation to a human rather than a refusal to
/// act. **E-M4-32** fixes which facts may take it: "**`Ok(None)` is limited to 'a legitimate construction of the
/// same object is not possible' (over the ceiling, or the old content already discarded)**". For git, in v0.1, that leaves exactly one: (sem: SEM-gx-adapter-git-051)
///
/// * **An unborn branch.** A branch with no commits on it — the state `git init` leaves — is a branch
///   a change *creates*. Its inverse would have to **delete** the reference, and v0.1's grammar spells
///   no deletion: `content` and `reset` both leave a branch pointing somewhere. So the answer is
///   `Ok(None)`, E-M3-4 escalates, and a person is asked before a branch disappears — which is the
///   right person to ask.
///
/// It is "a legitimate construction not possible" in E-M4-32's sense and not a defect: the object exists, the question is (sem: SEM-gx-adapter-git-052)
/// well-formed, and the honest answer is that this version cannot construct the inverse.
///
/// # Errors
/// [`Error::LocatorMismatch`] when `pre` is a snapshot of another object (**E-M4-32**): a delta and a
/// snapshot of two different objects is a wiring bug in whoever assembled the call, and answering
/// `Ok(None)` would send it down the escalation path wearing the face of a legitimate business
/// condition (**E-M4-27**'s argument). [`Error::ForeignDelta`] for another adapter's delta,
/// [`Error::PayloadUnreadable`] for bytes this grammar did not write, [`Error::Unimplemented`] for a
/// sequence v0.1 does not run, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// and [`Error::Unreadable`] when the repository will not answer.
///
/// # 🔴 **DEFECT-892-1** (`req/895` §1) — the read this escrow performs, and the object it is about
///
/// This used to answer `Result<Option<PlannedDelta>>` and `adapter.rs` folded it through
/// `InvertOutcome::from_option`, which fixed an empty read-set on the stated grounds that this
/// adapter has "no read that could fail separately from the call itself". [`repo::tip`] below is
/// that read, and its own failure road is an [`Error::Unreadable`] naming the branch. The empty
/// list travelled into signed receipts as `ReadSet::Nothing`, which `gx-witness` documents as
/// deciding "was this read?" with `Some(false)` for **every** locator.
///
/// The entry names [`crate::locator::Position::scope`] — the **branch** — and not the full locator,
/// because the branch is what was read: `tip` resolves `position.reference()`, and the read's own
/// `Unreadable` names the scope for the same reason. The path inside the tree is not touched on this
/// road at all, so attesting the entry's locator would put an object in the signed bytes that this
/// escrow did not read. That is DR-46-15's rule, applied to a substrate whose scope and object are
/// different strings.
pub(crate) fn invert(delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
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

    let named = locator::normalize(pre.locator());
    if named != position.locator() {
        return Err(Error::LocatorMismatch {
            expected: position.locator(),
            got: named,
        });
    }

    let repository = repo::open(&position)?;
    let answered = repo::tip(&repository, &position)?;
    // 🔴 **DEFECT-892-1** — built here, at the one place in this function where the read has
    // answered, so that both roads below carry it. An unborn branch is an answer too, and its
    // digest is [`repo::absent_digest`], whose collision with an empty value is disclosed there.
    let reads = vec![ReadEntry {
        digest: match answered {
            Some(tip) => repo::content_digest(repo::object_text(tip).as_bytes()),
            None => repo::absent_digest(),
        },
        locator: position.scope(),
    }];
    let Some(tip) = answered else {
        // The unborn branch, and the whole of this adapter's "no inverse".
        return Ok(InvertOutcome::none(reads));
    };

    let restore = GitOp::reset(position.locator(), repo::object_text(tip));
    let payload = GitDelta::one(restore).encode()?;
    PlannedDelta::new(SubstrateKind::Git, payload)
        .map(|inverse| InvertOutcome::inverted(inverse, reads))
}
