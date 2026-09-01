// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `plan`: what an intent asks of a schedule, worked out without touching one.
//!
//! 41 §4 calls `plan` a pure function. This module names no filesystem operation and no clock, so
//! for this adapter the stronger property holds -- **zero I/O** -- and `tests/wm4a_time_substrate.rs`
//! measures it as a source scan, the shape `gx-adapter-fs/tests/plan_purity.rs` established.
//!
//! # The intent's goal is the entry
//!
//! The goal bytes are the canonical form of the [`Entry`] the caller wants standing at the
//! position; **empty goal bytes mean "leave nothing there"**, which is a cancellation. Two shapes
//! rather than a verb field, because a schedule has exactly two states at a position -- an entry is
//! there, or it is not -- and a grammar with a third spelling would admit deltas that mean the same
//! thing two ways.
//!
//! # The promise, filled at birth
//!
//! `plan` fills [`PlannedDelta::with_promised_target`], which `req/1020` (WM-5a Phase 1) cashed for
//! the fs and git adapters. This adapter is the third to fill it and the first to have done so from
//! its first line: the post-state digest of a schedule position is the digest of the entry's
//! canonical bytes, and those come from the goal and from nothing else. The engine then compares
//! the promise with what `apply` observed and keeps the outcome either way
//! (`Engine::prediction_outcome`, `req/1010`), and a broken promise raises the seventh cause
//! (`PromisedPostStateWasWrong`, `req/1003`). None of that is new code here: the road existed and
//! this line puts a third adapter on it.
//!
//! 🔴 The promise is over the **re-encoded** entry rather than over the caller's goal bytes. A goal
//! that is a non-canonical encoding of a legal entry is accepted and written canonically, so what
//! stands at the position is what this adapter's own digest function is quantified over. `apply`
//! writes the same bytes through the same function, so promise and measurement cannot disagree by
//! encoding differently -- only by the world not becoming what was asked, which is what the
//! comparison exists to catch.

use gx_core::{Intent, ObjectSnapshot};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::adapter::{absent_digest, content_digest, kind};
use crate::entry::{Entry, TimeOp, MAX_PAYLOAD_BYTES};
use crate::locator;

/// Work out the change an intent asks for, without making it (41 §4).
///
/// # Errors
/// [`Error::NotPlannable`] when the intent is for another substrate, names a position this adapter
/// cannot accept, carries a goal that is not a schedule entry, claims the entry has already run
/// (**INV-WM4a-1**), or asks for a payload over [`MAX_PAYLOAD_BYTES`]; [`Error::NotDigestible`]
/// when the operation has no canonical form.
pub fn plan(intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
    if intent.substrate() != &kind() {
        return Err(Error::NotPlannable {
            detail: format!(
                "the intent is for {:?} and this adapter speaks {:?}",
                intent.substrate(),
                kind()
            ),
        });
    }

    let position = locator::normalize(intent.locator());
    if !locator::is_absolute(&position) {
        return Err(Error::NotPlannable {
            detail: format!(
                "{:?} normalises to {position:?}, which is not a position from the root; v0.1 \
                 names positions absolutely (ASM-69-3)",
                intent.locator()
            ),
        });
    }

    let goal = &intent.goal().0;
    let (op, target) = if goal.is_empty() {
        // A cancellation. The post-state of a position with nothing at it is the absent digest,
        // which this adapter discloses to be the digest of empty content as well -- the same
        // residue `gx-adapter-fs` carries and for the same reason (see `crate::adapter`).
        (TimeOp::remove(position), absent_digest())
    } else {
        let entry = Entry::decode(goal).map_err(|e| Error::NotPlannable {
            detail: format!(
                "the goal is not a schedule entry, so no delta of this adapter realises it: {e}"
            ),
        })?;
        // 🔴 **INV-WM4a-1** (`req/1038` §1b): gx is never the author of firedness. An entry gx
        // places may say it has not run -- true by construction, since it did not exist a moment
        // ago -- and may leave the record absent, but "this already ran" is a claim about the world
        // that gx did not observe. Refusing it here is what makes the firedness inside a digest a
        // fact one party writes: the runner. Were both parties able to write it, the compare-and-set
        // that closes the undo window (crate root) would be comparing gx's own assertion with gx's
        // own assertion.
        if entry.is_recorded_as_fired() {
            return Err(Error::NotPlannable {
                detail: "the goal claims the entry has already run; gx does not write that \
                         assertion (INV-WM4a-1, req/1038 §1b) -- it is the runner's to make, and \
                         gx reads it"
                    .to_string(),
            });
        }
        let written = entry.encode()?;
        let digest = content_digest(&written);
        (TimeOp::write(position, entry), digest)
    };

    let payload = op.encode()?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(Error::NotPlannable {
            detail: format!(
                "the payload of this change would be {} bytes and this adapter plans at most \
                 {MAX_PAYLOAD_BYTES}; a delta is kept once it is planned (42 §5, E-M4-8), so the \
                 size is a cost the whole pipeline pays",
                payload.len()
            ),
        });
    }

    Ok(PlannedDelta::new(kind(), payload)?.with_promised_target(target))
}
