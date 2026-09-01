// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `plan`, with no filesystem in it (**E-M4-29**).
//!
//! 41 §4 calls `plan` "a pure function, no side effects" and §30 M4H2-3, adopted (b) read that for the trait as "determinism over the (intent,
//! pre) pair + zero **writes** to the substrate", deliberately leaving reads open so that M7's (sem: SEM-gx-adapter-fs-091)
//! git adapter may consult an object store. Then it added a clause for this adapter alone:
//!
//! > "**however, for the fs adapter v0.1, `plan` achieving zero I/O holds** (for a single whole-file replacement, the target digest is
//! > derivable from the goal bytes), so **hand 4's DoD carries a machine check that 'fs's plan does not call std::fs'**
//! > (a machine-fixed implementation stronger than the contract)" (sem: SEM-gx-adapter-fs-092)
//!
//! # Why a module rather than a function in `adapter.rs`
//!
//! Because "calls no filesystem operation" is a claim about source, and the cheapest honest way to (sem: SEM-gx-adapter-fs-093)
//! measure it is a file that never names one. `adapter.rs` opens files -- `snapshot` and
//! `precondition` must -- so a scan of that file could only ever be a scan of a function body.
//! `tests/plan_purity.rs` reads both: this module for filesystem tokens, and the trait method's body
//! for the single line that delegates here.
//!
//! The `pre` argument is unused, and that is the finding rather than an oversight: a whole-file
//! replacement is a function of the goal alone. **E-M4-4** put `pre` in the signature because 43 T-2
//! quantifies determinism over "the same snapshot" and a future adapter (or this one, once v0.2 plans a (sem: SEM-gx-adapter-fs-094)
//! patch instead of a replacement) needs it; L1 in the harness is the law, and it holds a fortiori
//! for an adapter that ignores the argument.

use gx_core::{Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::adapter::content_digest;
use crate::delta::{FsDelta, FsOp, MAX_FORWARD_PAYLOAD_BYTES};
use crate::locator;

/// Work out the change an intent asks for, without making it (41 §4, FR-042).
///
/// The delta is the one-operation sequence of **M4-13, adopted (a)**: replace the whole file at the
/// intent's normalised locator with the intent's goal bytes. `goal` is carried opaquely by
/// [`gx_core::GoalBytes`] (**E-M4-2**), and what this adapter's grammar says about those bytes is
/// "this is the file's new content" -- which is the whole of the interpretation P-6 reserves to an (sem: SEM-gx-adapter-fs-095)
/// adapter.
///
/// # Errors
/// [`Error::NotPlannable`] when the intent is for another substrate, or names a position this
/// adapter cannot accept (empty, or relative: **ASM-69-3**), or asks for a change larger than
/// [`MAX_FORWARD_PAYLOAD_BYTES`] (**M4H5-4, adopted (b)**), or when the sequence has no canonical form. All
/// four are "no delta plans this intent against this snapshot" rather than failures of the world, (sem: SEM-gx-adapter-fs-096)
/// which is why none of them is [`Error::Unreadable`].
pub fn plan(intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
    if intent.substrate() != &SubstrateKind::Fs {
        return Err(Error::NotPlannable {
            detail: format!(
                "the intent is for {:?} and this adapter speaks {:?}",
                intent.substrate(),
                SubstrateKind::Fs
            ),
        });
    }

    let target = locator::normalize(intent.locator());
    if !locator::is_absolute(&target) {
        return Err(Error::NotPlannable {
            detail: format!(
                "{:?} normalises to {target:?}, which is not a position from the root; v0.1 names \
                 positions absolutely (ASM-69-3)",
                intent.locator()
            ),
        });
    }

    let payload = FsDelta::one(FsOp::write(target, intent.goal().0.clone())).encode()?;
    // **M4H5-4, adopted (b)**: the bound is on the payload rather than on the goal, because the payload is (sem: SEM-gx-adapter-fs-097)
    // what a gate carries and a journal keeps (E-M4-8) -- a bound on the content would leave the
    // encoding's own overhead outside the number that was declared, which is the reading hand 5 gave
    // the escrow ceiling for the same reason.
    if payload.len() > MAX_FORWARD_PAYLOAD_BYTES {
        return Err(Error::NotPlannable {
            detail: format!(
                "the payload of this change would be {} bytes and this adapter plans at most \
                 {MAX_FORWARD_PAYLOAD_BYTES} (M4H5-4(b)); a delta is kept once it is planned (42 §5, \
                 E-M4-8), so the size is a cost the whole pipeline pays",
                payload.len()
            ),
        });
    }
    // 🔴 **WM-5a Phase 1** (`req/1011` §4, ruled by `req/1016`): the prophecy seat, filled.
    //
    // The module header has quoted the reason since **E-M4-29** was written -- "for a single
    // whole-file replacement, the target digest is derivable from the goal bytes" -- and until now
    // that sentence was an argument nobody cashed: `promised_target` stayed `None` in production
    // and only the conformance fixture computed the value, for L5's benefit. `req/1016` measured
    // the claim (`AGREE=true`, one digest reached by two roads) and this line is what it bought.
    //
    // What it guarantees: the post-state digest a commit of this delta will observe, computed from
    // the goal and from nothing else -- no snapshot is consulted, no position is read, and the zero
    // I/O `tests/plan_purity.rs` fixes as a machine check is untouched, because
    // [`crate::adapter::content_digest`] is `gx-canon`'s mint and not a filesystem call. It is the
    // **same function** `apply`'s observation goes through (41 §6 admits one), so promise and
    // measurement cannot disagree by digesting differently -- only by the world not becoming what
    // was asked, which is precisely what the comparison exists to catch.
    //
    // What it does not guarantee: that the promise is kept. A separate writer between `plan` and
    // `apply`, a partial write, a rename into the wrong position -- each makes this prediction
    // wrong, and being wrong is the point (`AbortReason::PostconditionMismatch`, and the record
    // `Engine::prediction_outcome` keeps of both outcomes).
    //
    // 🔴 This moves `Transformation.target` from `None` to `Some` for every fs transformation, and
    // `target` is inside the identity view -- so the `TransformationId` of an fs plan is not the
    // one this adapter minted before. Declared here rather than discovered: the ids were never
    // frozen anywhere (they are derived on every road), but a reader comparing an id written down
    // before this lane will find it moved.
    Ok(PlannedDelta::new(SubstrateKind::Fs, payload)?
        .with_promised_target(content_digest(&intent.goal().0)))
}
