// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `apply`: put the entry in the schedule, or take it out.
//!
//! 42 §3.4 for [`AppliedDelta`]'s fields, **E-M4-31** for the `Timestamp(0)` placeholder, **M4-06,
//! adopted (b)** for the observed digest being what L5 compares against a promise.
//!
//! # Why the write is the temp-fsync-rename dance and not a `write`
//!
//! Because a torn entry is worse here than a torn file usually is. A schedule is read by a runner
//! that is not gx and that may read at any moment, including between the two halves of a
//! non-atomic write; a half-written entry is a schedule entry with no meaning being handed to
//! something whose job is to execute it. `rename` is the operation that replaces a file entire, and
//! the `fsync` of the file and then of the directory are what make the replacement survive a crash
//! -- the sequence `gx-adapter-fs::apply` cites LWN 457667 for.
//!
//! **Idempotence** (41 §4, quantified over the retry by **E-M4-3**): applying the same delta twice
//! reaches the same state, because both directions are whole-position operations -- the entry is
//! written entire, and a removal of something already gone is a no-op that this module reports as
//! success rather than as a failure of the world.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use gx_core::{Cid, Fingerprint, Timestamp};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result};

use crate::adapter::{absent_digest, content_digest, kind};
use crate::entry::TimeOp;

/// Distinguishes the temporary files of two applies in one process.
///
/// Not adapter state: [`crate::TimeAdapter`] holds nothing and is `Send + Sync` for free (AC-046).
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Perform a delta a gate has already admitted (41 §4).
///
/// # Errors
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// [`Error::ApplyFailed`] when the filesystem refused a step, and [`Error::Unreadable`] when the
/// position will not answer the observation that follows the write.
pub fn apply(delta: &PlannedDelta) -> Result<AppliedDelta> {
    if delta.substrate() != &kind() {
        return Err(Error::ForeignDelta {
            expected: kind(),
            got: delta.substrate().clone(),
        });
    }
    let op = TimeOp::decode(delta.payload())?;
    let position = op.position()?;

    match op.entry() {
        Some(entry) => write_whole_entry(&position, &entry.encode()?)?,
        None => remove_entry(&position)?,
    }

    let digest = observe(&position)?;
    let postcondition = Fingerprint::new(kind(), elide_scope(position)?, digest)?;
    Ok(AppliedDelta::new(
        delta.reference().clone(),
        postcondition,
        digest,
        // **E-M4-31**: the engine overwrites `applied_at` at commit time, and 41 §6 keeps clocks
        // out of this layer, so the honest value is the one an engine is expected to replace. A
        // `Timestamp(0)` that reached a journal is an engine that skipped the step -- not, here, an
        // adapter that could not find the time, which would be the reading this substrate's name
        // invites and which is wrong.
        Timestamp(0),
    ))
}

/// Create the entry beside the target, flush it, rename it into place, flush the directory.
fn write_whole_entry(position: &str, bytes: &[u8]) -> Result<()> {
    let target = Path::new(position);
    let parent = target.parent().ok_or_else(|| Error::ApplyFailed {
        detail: format!("{position:?} has no directory to write in"),
    })?;
    let temp = parent.join(format!(
        ".gx-schedule-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));

    let attempt = || -> std::io::Result<()> {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, target)?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    };

    attempt().map_err(|e| {
        // A failed attempt leaves no half-entry in the schedule: the temporary file is the only
        // thing that can be partial and it never had the target's name.
        let _ = std::fs::remove_file(&temp);
        Error::ApplyFailed {
            detail: format!("{position:?} could not be written: {e}"),
        }
    })
}

/// Take the entry out of the schedule; already gone is already done.
fn remove_entry(position: &str) -> Result<()> {
    match std::fs::remove_file(position) {
        Ok(()) => flush_parent(position),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::ApplyFailed {
            detail: format!("{position:?} could not be removed: {e}"),
        }),
    }
}

/// The directory entry is data too, so a removal that has not reached stable storage can come back.
fn flush_parent(position: &str) -> Result<()> {
    let Some(parent) = Path::new(position).parent() else {
        return Ok(());
    };
    std::fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| Error::ApplyFailed {
            detail: format!("the directory of {position:?} could not be flushed: {e}"),
        })
}

/// What stands at the position now: the digest L5 compares with the plan's promise.
///
/// An absent position answers [`absent_digest`] rather than an error, because "nothing is scheduled
/// here" is the post-state of a cancellation and not a failure to read.
fn observe(position: &str) -> Result<Cid> {
    match std::fs::read(position) {
        Ok(bytes) => Ok(content_digest(&bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(absent_digest()),
        Err(e) => Err(Error::Unreadable {
            locator: position.to_string(),
            detail: e.to_string(),
        }),
    }
}
