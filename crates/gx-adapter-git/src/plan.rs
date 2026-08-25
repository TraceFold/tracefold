// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `plan`, with no repository in it, and the reason that is a law rather than a preference.
//!
//! 41 §4 calls `plan` "a pure function, no side effects" and §30 M4H2-3, adopted (b) read that for the trait as "determinism over the (intent, pre)
//! pair + zero **writes** to the substrate", adding, in this crate's name: "reading is not forbidden
//! -- it does not close the road for M7's git adapter to read an object store". (sem: SEM-gx-adapter-git-082)
//!
//! 🔴 **The road is open and this module does not take it.** The reason is **L1** (req/69 §3.4), which
//! quantifies determinism over the pair and makes the substrate move in between: the same
//! `(intent, pre)` planned twice, with a commit landing between the two calls, has to produce the
//! **same delta**. A plan that read the branch tip to use as its commit's parent would produce a
//! different commit id each time the branch moved, so the tip cannot be in the payload — and once the
//! tip is out, nothing else in a git repository is needed to say what the change *is*.
//!
//! What the payload says is therefore a **state** and not a commit: "the entry at this path, on this
//! branch, becomes these bytes". [`crate::apply`] reads the tip at the moment the engine's CAS has (sem: SEM-gx-adapter-git-083)
//! just declared current (41 §5-5b) and builds the commit there.
//!
//! # Why a module rather than a function in `adapter.rs`
//!
//! Because "calls no repository operation" is a claim about source, and the cheapest honest way to (sem: SEM-gx-adapter-git-084)
//! measure it is a file that never names one. `adapter.rs` reads objects — `snapshot` and
//! `precondition` must — so a scan of that file could only ever be a scan of a function body.
//! `tests/git_plan_purity.rs` reads both, and adds the measurement text cannot make: the repository's
//! `.git` directory is compared byte for byte across a `plan`.
//!
//! The `pre` argument is unused, and that is the finding rather than an oversight — the same one
//! `gx-adapter-fs` records for the same reason. **E-M4-4** put `pre` in the signature because 43 T-2
//! quantifies determinism over "the same snapshot"; an adapter whose payload is a target state does not (sem: SEM-gx-adapter-git-085)
//! need it, and L1 holds a fortiori for one that ignores it.

use gx_core::{Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{GitDelta, GitOp, MAX_FORWARD_PAYLOAD_BYTES};
use crate::locator;

/// Work out the change an intent asks for, without making it (41 §4, FR-042).
///
/// The delta is the one-operation sequence of **M4-13, adopted (a)**: the entry at the intent's normalised (sem: SEM-gx-adapter-git-086)
/// locator becomes the intent's goal bytes. `goal` is carried opaquely by [`gx_core::GoalBytes`]
/// (**E-M4-2**), and what this adapter's grammar says about those bytes is "this is the entry's new
/// content" -- the whole of the interpretation P-6 reserves to an adapter. (sem: SEM-gx-adapter-git-087)
///
/// # Errors
/// [`Error::NotPlannable`] when the intent is for another substrate or asks for a change larger than
/// [`MAX_FORWARD_PAYLOAD_BYTES`] (**M4H5-4, adopted (b)**). [`Error::NotAPosition`] when the locator does not (sem: SEM-gx-adapter-git-088)
/// name a repository, a reference and a path inside its tree -- "the argument is not a position" rather than "application
/// failed" (**M4H5-5, adopted (b)**). [`Error::NotDigestible`] when the sequence has no canonical form. (sem: SEM-gx-adapter-git-089)
pub fn plan(intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
    if intent.substrate() != &SubstrateKind::Git {
        return Err(Error::NotPlannable {
            detail: format!(
                "the intent is for {:?} and this adapter speaks {:?}",
                intent.substrate(),
                SubstrateKind::Git
            ),
        });
    }

    // Parsed rather than merely normalised: a locator that is not a position is refused **here**,
    // before a payload exists, so that no delta this adapter minted can carry a spelling `apply`
    // would have to refuse. The refusal is `NotAPosition` and not `NotPlannable` because the two
    // answer different questions -- "that is not a place" and "no change reaches that goal from here". (sem: SEM-gx-adapter-git-090)
    let position = locator::parse(intent.locator())?;

    let payload =
        GitDelta::one(GitOp::write(position.locator(), intent.goal().0.clone())).encode()?;
    // **M4H5-4, adopted (b)**: the bound is on the payload rather than on the goal, because the payload is (sem: SEM-gx-adapter-git-091)
    // what a gate carries and a journal keeps (E-M4-8) — a bound on the content would leave the
    // encoding's own overhead outside the number that was declared.
    if payload.len() > MAX_FORWARD_PAYLOAD_BYTES {
        return Err(Error::NotPlannable {
            detail: format!(
                "the payload of this change would be {} bytes and this adapter plans at most \
                 {MAX_FORWARD_PAYLOAD_BYTES} (M4H5-4(b)); a delta is kept once it is planned \
                 (42 §5, E-M4-8), so the size is a cost the whole pipeline pays",
                payload.len()
            ),
        });
    }
    PlannedDelta::new(SubstrateKind::Git, payload)
}
