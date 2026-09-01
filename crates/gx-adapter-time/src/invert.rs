// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `invert`: the delta that puts the schedule back, and the three answers it can give.
//!
//! **E-M4-30** / 43 T-10b: the escrow is constructed **before** `apply`, because the inverse of a
//! replacement carries the old content and after the apply there is none.
//!
//! # 🔴 What this adapter can and cannot know at escrow time
//!
//! The question a reader arrives with is "has it fired yet?", and the honest answer is that at the
//! moment `invert` is called **the effect being escrowed has not happened at all**, so it cannot
//! have fired. An adapter that answered a verdict about the firing of its own delta would be
//! answering about a future it has not reached. `req/1038` §6b records that a first design did
//! exactly that and why it was withdrawn.
//!
//! What *is* observable here is the schedule's **form**: whether it records firedness. That is what
//! decides whether the window can ever be seen to close:
//!
//! * A schedule that records firedness puts that record inside the position's bytes, so this
//!   adapter's digest covers it, so the engine's compare-and-set stops matching the escrowed
//!   fingerprint the moment the runner marks the entry fired. An undo attempted after the firing is
//!   refused by machinery that already exists. The inverse is then worth carrying:
//!   [`Reversibility::True`].
//! * A schedule that does not record firedness offers nothing that changes when the action runs.
//!   Restoring the bytes might restore the world or might restore a record of something that has
//!   already happened, and **no observation available to gx separates the two, now or later**. That
//!   is [`Reversibility::Unknown`] in the words `gx-substrate` gives it: whether an inverse exists
//!   was never established. Answering `False` would claim a measurement was taken and came back
//!   negative; answering `True` would claim the undo restores the world. Both are stronger than what
//!   is known.
//! * Over the escrow ceiling there is no inverse to carry at all, which is [`Reversibility::False`]
//!   for the reason **M4-21** gives the fs adapter.
//!
//! The middle row is the one no other adapter in this workspace reaches from its own source, and it
//! is reached here without a posture flag: it is a property of the schedule in hand.
//!
//! # The read, and the entry that attests it (**DEFECT-892-1**)
//!
//! Every road out of this function carries a [`ReadEntry`] minted where the read answered, absent
//! positions included. An empty read-set on a signed receipt is a positive claim about every
//! locator in the universe, which is the defect `req/895` §1 measured on four adapters at once.

use gx_core::{ObjectSnapshot, ReadEntry};
use gx_substrate::{Error, InvertOutcome, PlannedDelta, Result};

use crate::adapter::{absent_digest, content_digest, kind};
use crate::entry::{Entry, TimeOp, MAX_PAYLOAD_BYTES};
use crate::locator;

/// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
///
/// # Errors
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// [`Error::LocatorMismatch`] when `pre` is a snapshot of another object (**E-M4-32**), and
/// [`Error::Unreadable`] when the position answers with bytes that are not a schedule entry -- a
/// position gx cannot read is a question that cannot be answered, which 41 §4 separates from "the
/// answer is no inverse".
pub fn invert(delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
    if delta.substrate() != &kind() {
        return Err(Error::ForeignDelta {
            expected: kind(),
            got: delta.substrate().clone(),
        });
    }
    let op = TimeOp::decode(delta.payload())?;
    let position = op.position()?;

    let named = locator::normalize(pre.locator());
    if named != position {
        return Err(Error::LocatorMismatch {
            expected: position,
            got: named,
        });
    }

    let Some(bytes) = read_if_present(&position)? else {
        // Nothing is scheduled here, so the change is a creation and its inverse is a cancellation.
        // `True` without qualification: taking out an entry that gx itself put in, and that nothing
        // has yet been able to run because it did not exist, restores the schedule exactly.
        let entry = ReadEntry {
            digest: absent_digest(),
            locator: position.clone(),
        };
        let payload = TimeOp::remove(position).encode()?;
        return PlannedDelta::new(kind(), payload)
            .map(|inverse| InvertOutcome::inverted(inverse, vec![entry]));
    };

    let read = ReadEntry {
        digest: content_digest(&bytes),
        locator: position.clone(),
    };
    let standing = Entry::decode(&bytes).map_err(|e| Error::Unreadable {
        locator: position.clone(),
        detail: format!(
            "the position answered, but not with a schedule entry this adapter wrote ({e}); an \
             unreadable prior is a question that cannot be answered rather than an absent inverse"
        ),
    })?;

    if !standing.records_firedness() {
        // 🔴 The `Unknown` row of the crate root's table. The read happened and is attested -- "gx
        // read the entry and could not establish whether an undo would un-run something" is a
        // different fact from "gx never looked", and the read-set says which.
        return Ok(InvertOutcome::undetermined(vec![read]));
    }

    let payload = TimeOp::write(position, standing).encode()?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        // **M4-21**: over the ceiling there is no escrow, and the change is escalated instead of
        // being quietly made unundoable. The read still happened and is still attested.
        return Ok(InvertOutcome::none(vec![read]));
    }
    PlannedDelta::new(kind(), payload).map(|inverse| InvertOutcome::inverted(inverse, vec![read]))
}

/// The bytes at a position, or `None` when there are none.
///
/// The absence is a state and not a failure: it is what a creation is planned against.
fn read_if_present(position: &str) -> Result<Option<Vec<u8>>> {
    match std::fs::read(position) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Unreadable {
            locator: position.to_string(),
            detail: e.to_string(),
        }),
    }
}
