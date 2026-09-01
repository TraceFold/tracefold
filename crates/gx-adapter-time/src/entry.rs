// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The grammar: what a schedule entry is, and what a delta over one looks like.
//!
//! Canonical DAG-CBOR through `gx-canon`, which 41 §6 makes the one road bytes become a canonical
//! form on. Both types derive their codec, the shape `gx_adapter_fs::delta::FsOp` already proved
//! against `cbor::scan_strict`.

use gx_canon::cbor;
use gx_substrate::{Error, Result};
use serde::{Deserialize, Serialize};

/// How large an entry payload this adapter carries, in either direction.
///
/// # Why one constant where `gx-adapter-fs` argued for two
///
/// That adapter's two bounds ask different questions of different bytes: the forward one bounds a
/// whole new file, the escrow one bounds the *old* file's content, and the two can be expected to
/// move apart. Here both directions carry the same shape -- one entry, or nothing -- so a forward
/// payload this adapter accepts is one it could equally escrow, which is the relation the fs crate
/// has to argue for (`MAX_FORWARD <= MAX_INVERSE`) and this one gets by construction.
///
/// 64 KiB is chosen against what a schedule entry is: a command line and a moment. The largest
/// entry this crate's own tests build is under 200 bytes. The number is a declared ceiling and not
/// a measured maximum, and it is enforced where a payload is made -- [`crate::plan`] for the
/// forward direction, [`crate::invert`] for the escrow -- and not at [`TimeOp::decode`], so a
/// payload written by hand still decodes. That split is `gx-adapter-fs`'s and is kept deliberately:
/// a longer value is legal *as a value* and refused *as a v0.1 payload*.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// One entry of a schedule: what is to be run, when, and whether the schedule says it has run.
///
/// # The three states of `fired`, and why the third is not a missing value
///
/// * `Some(false)` -- the schedule records this entry as not yet run.
/// * `Some(true)` -- the schedule records that it ran.
/// * `None` -- **the schedule does not record firedness at all.** Many do not; a directory of
///   command files has no place to write it.
///
/// The third is a fact about the schedule and not an absence of data about the entry, which is why
/// it is kept apart from `Some(false)` rather than defaulted into it. What turns on the difference
/// is [`crate::invert`]'s verdict: a schedule that records firedness makes a later firing visible
/// to the engine's compare-and-set, and one that does not never will, so the second case is
/// `Unknown` and not `True`. Collapsing them would be this crate answering a question nobody asked
/// the world -- the shape `feedback_untestable_is_not_failed` names.
///
/// # `fire_at` is carried, not read
///
/// 41 §6 keeps clocks out of this layer and this crate names none (`req/1038` INV-WM4a-2). No
/// branch in this crate reads this field: comparing it against a moment would need the moment, and
/// an adapter that had one would answer differently on two runs of one `(intent, snapshot)` pair.
/// It is the runner's datum, carried through gx unread, the way a payload is.
///
/// The unit is 42's `Timestamp`: nanoseconds since the Unix epoch. Spelled `i64` rather than
/// [`gx_core::Timestamp`] because this is a value inside an adapter's own grammar and the core's
/// type is the engine's; the two agreeing on the integer is what matters, and the doc is where
/// that agreement is stated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// What the runner is to run. Opaque to this adapter: it is never parsed, matched or executed
    /// here, and this crate starts no process (crate root).
    pub action: String,
    /// When it is due, as 42's `Timestamp` integer. Carried and never branched on -- see above.
    pub fire_at: i64,
    /// What the schedule says about whether it has run, in three values -- see above.
    pub fired: Option<bool>,
}

impl Entry {
    /// The canonical bytes of this entry: what stands at the position when the entry is in the
    /// schedule, and what a digest of the position covers.
    ///
    /// # Errors
    /// [`Error::NotDigestible`] when the value has no canonical DAG-CBOR form.
    pub fn encode(&self) -> Result<Vec<u8>> {
        cbor::encode(self).map_err(|e| Error::NotDigestible {
            detail: format!("the schedule entry has no canonical DAG-CBOR form: {e}"),
        })
    }

    /// Read an entry back from the bytes at a position, or from an intent's goal.
    ///
    /// # Errors
    /// [`Error::PayloadUnreadable`] for bytes this grammar did not write.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        cbor::decode(bytes).map_err(|e| Error::PayloadUnreadable {
            detail: format!("not a schedule entry (42 §2.1 canonical DAG-CBOR): {e}"),
        })
    }

    /// Whether the schedule records firedness for this entry at all.
    ///
    /// Derived rather than stored, for the reason `req/1010` §2b gives for `PredictionOutcome`:
    /// one fact living in two places is one fact that can disagree with itself.
    #[must_use]
    pub const fn records_firedness(&self) -> bool {
        self.fired.is_some()
    }

    /// Whether the schedule says the action has already run.
    ///
    /// `false` for an entry with no firedness record -- **which is not the same claim** as "it has
    /// not run", and no caller in this crate reads it as one. It exists for a reader of a report;
    /// the decisions are made on [`Self::records_firedness`].
    #[must_use]
    pub fn is_recorded_as_fired(&self) -> bool {
        self.fired == Some(true)
    }
}

/// One change to one position of a schedule.
///
/// `None` is "leave nothing here" -- a cancellation. `Some(entry)` is "let this stand here".
///
/// # Why there is no sequence
///
/// `gx-adapter-fs` carries a one-element sequence because its grammar is the free monoid of a
/// ruling that anticipated multi-file deltas. Nothing rules that way here, and a delta that could
/// carry two entries would be a delta whose escrow is two entries: the size of an undo would then
/// depend on how many changes a caller happened to bundle. One entry per delta keeps the inverse
/// the size of the thing it undoes, and a schedule-wide change is several transformations, each
/// with its own gate and its own record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeOp {
    /// What is to stand at the position, or `None` to leave nothing there.
    entry: Option<Entry>,
    /// The position, normalised (`crate::locator`).
    locator: String,
}

impl TimeOp {
    /// Put `entry` at `locator`.
    #[must_use]
    pub const fn write(locator: String, entry: Entry) -> Self {
        Self {
            entry: Some(entry),
            locator,
        }
    }

    /// Leave nothing at `locator` -- a cancellation, and the inverse of a creation.
    #[must_use]
    pub const fn remove(locator: String) -> Self {
        Self {
            entry: None,
            locator,
        }
    }

    /// What is to stand at the position, or `None` for a cancellation.
    #[must_use]
    pub const fn entry(&self) -> Option<&Entry> {
        self.entry.as_ref()
    }

    /// The position this operation applies at, normalised and checked.
    ///
    /// # Errors
    /// [`Error::NotAPosition`] when the locator does not normalise to a position from the root.
    pub fn position(&self) -> Result<String> {
        let normalised = crate::locator::normalize(&self.locator);
        if !crate::locator::is_absolute(&normalised) {
            return Err(Error::NotAPosition {
                locator: self.locator.clone(),
                normalised,
            });
        }
        Ok(normalised)
    }

    /// The canonical payload of a delta carrying this operation.
    ///
    /// # Errors
    /// [`Error::NotDigestible`] when the value has no canonical DAG-CBOR form.
    pub fn encode(&self) -> Result<Vec<u8>> {
        cbor::encode(self).map_err(|e| Error::NotDigestible {
            detail: format!("the time delta has no canonical DAG-CBOR form: {e}"),
        })
    }

    /// Read a payload back.
    ///
    /// # Errors
    /// [`Error::PayloadUnreadable`] for bytes this grammar did not write.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        cbor::decode(payload).map_err(|e| Error::PayloadUnreadable {
            detail: format!("not a time delta (42 §2.1 canonical DAG-CBOR): {e}"),
        })
    }
}
