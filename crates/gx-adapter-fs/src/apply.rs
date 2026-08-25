// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `apply`: the three steps around a `rename`, and the observation that comes back.
//!
//! Spec: 41 §4 for the method ("only called after commit approval" / "apply is designed to be idempotent"), 42 §3.4 for
//! [`AppliedDelta`]'s fields, 45 §3 for the residual condition this shape closes. The rulings are
//! **M4-13, adopted (a)** (single-file whole replacement, atomicity from `rename`, `len==1` only),
//! **E-M4-3** (idempotence is quantified over the retry), **E-M4-31** (the `Timestamp(0)`
//! placeholder) and **M4-06, adopted (b)** (the observed digest is what L5 compares with a promise). (sem: SEM-gx-adapter-fs-010)
//!
//! # The three steps, and which of them is the primary's
//!
//! This hand fetched all three primaries in full (`Desktop/GitRepo/REFERENCES.md`, 2026-08-09; the
//! entry records that hand 4's confirmation of the same three was a WebSearch snippet). What LWN
//! 457667 gives, verbatim, is an ordered list of five:
//!
//! > "The following steps are required to perform this type of update: create a new temp file (on the
//! > same file system!) / write data to the temp file / fsync() the temp file / rename the temp file
//! > to the appropriate name / fsync() the containing directory"
//!
//! `write_whole_file` (private) is that list. `SURVIVORS.md` §A-2 calls the last three "3 moves", which is the (sem: SEM-gx-adapter-fs-011)
//! same sequence counted from the first thing an implementation can get wrong.
//!
//! 🔴 **What the primary does not say.** `SURVIVORS.md` §A-2 and `req/73` both write that omitting
//! the directory `fsync` loses the rename. That sentence is **not in LWN 457667** -- the article
//! lists the steps and never discusses omitting one. It follows from what a directory entry is (data,
//! which a crash can lose if it reached no stable storage), so this crate keeps the three steps and
//! marks the failure modes as **derivation** rather than as citation. Saying which is which costs one
//! word and is the difference between a source and a belief.
//!
//! # Why `rename` and not a write in place
//!
//! POSIX's normative sentence is about **other observers**: "a directory entry named new shall remain
//! visible to other threads throughout the renaming operation and refer either to the file referred
//! to by new or old before the operation began" (the word "atomic" itself is in the RATIONALE). A (sem: SEM-gx-adapter-fs-012)
//! writer that truncated and rewrote would expose a half-written file to any reader in between, and
//! 45 §3's TH-3 residual condition is "`adapter.apply` is not a single syscall" -- one `rename` is one (sem: SEM-gx-adapter-fs-013)
//! syscall, and that is the whole reason [`crate::delta::MAX_OPS`] is one.
//!
//! `std::fs::rename` documents the replacement ("replacing the original file if `to` already exists") (sem: SEM-gx-adapter-fs-014)
//! and documents **no atomicity at all**, because the guarantee belongs to the filesystem underneath.
//! That is why `tests/support/mod.rs` proves the filesystem from `/proc/self/mountinfo` before a byte
//! is written, and why nothing here may be read as a claim about `/mnt/c`.
//!
//! # What comes back is an observation
//!
//! req/69 §3.1: "post is an observation, not a return value". So `observe` (private) **reads the position back** after the (sem: SEM-gx-adapter-fs-015)
//! rename instead of digesting the buffer that was written: the buffer is what this process intended
//! and the read is what the substrate holds. It is also what gives L5 two roads to compare -- the
//! fixture derives its promise from the goal, and this derives the observation from the disk.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use gx_core::{Cid, Fingerprint, SubstrateKind, Timestamp};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result};

use crate::adapter::content_digest;
use crate::delta::FsDelta;

/// Distinguishes the temporary files of two threads inside one process.
///
/// Not adapter state: [`crate::FsAdapter`] still holds nothing and is still `Send + Sync` for free
/// (AC-046). A counter is needed because the tests of this crate run their binaries in parallel and
/// two applies in one directory must not choose one name.
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Perform a delta a gate has already admitted (41 §4).
///
/// # Errors
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::Unimplemented`] for a sequence longer than v0.1 runs
/// (**M4-13, adopted (a)**'s fail-closed refusal, raised by [`FsDelta::decode`]), and (sem: SEM-gx-adapter-fs-016)
/// [`Error::ApplyFailed`] when the filesystem refused a step.
pub(crate) fn apply(delta: &PlannedDelta) -> Result<AppliedDelta> {
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
        .expect("decode refuses the empty sequence");
    let position = op.position()?;

    match op.content() {
        Some(content) => write_whole_file(&position, content)?,
        None => remove_whole_file(&position)?,
    }

    let digest = observe(&position)?;
    let postcondition = Fingerprint::new(SubstrateKind::Fs, elide_scope(position)?, digest)?;
    Ok(AppliedDelta::new(
        delta.reference().clone(),
        postcondition,
        digest,
        // **E-M4-31** (req/38 §31): "`applied_at` is overwritten by the engine at commit time... the adapter writes a
        // `Timestamp(0)` placeholder". 41 §4 gives `apply` no parameter an engine could inject a (sem: SEM-gx-adapter-fs-017)
        // moment through, and 41 §6 keeps clocks out of this layer, so the honest value is the one
        // an engine is expected to overwrite. A `Timestamp(0)` that reached a journal is an engine
        // that skipped the step, not a filesystem with no clock.
        Timestamp(0),
    ))
}

/// The five steps of LWN 457667, in order.
///
/// The temporary file is made **in the target's own directory** because POSIX `rename` is defined
/// within one filesystem -- across one it fails with `EXDEV` -- and because a copy across a mount
/// point would not be a rename at all.
fn write_whole_file(position: &str, content: &[u8]) -> Result<()> {
    let target = Path::new(position);
    let parent = target.parent().ok_or_else(|| Error::ApplyFailed {
        detail: format!("{position:?} has no directory to write in"),
    })?;
    let temp = parent.join(format!(
        ".gx-apply-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));

    let attempt = || -> std::io::Result<()> {
        // 1-2. create the temporary file beside the target, and write the whole content.
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(content)?;
        // 3. fsync the temporary file: its bytes before its name.
        file.sync_all()?;
        drop(file);
        // 4. rename it over the target. This is the step the atomicity comes from.
        std::fs::rename(&temp, target)?;
        // 5. fsync the containing directory, so that the entry naming the new file is durable too.
        //    (Derivation, not citation: see the module documentation.)
        std::fs::File::open(parent)?.sync_all()
    };

    if let Err(e) = attempt() {
        // Whatever failed, the temporary file must not be left in the substrate. If the rename
        // succeeded and a later step did not, there is nothing under this name to remove and the
        // removal is a no-op -- which is why the result is discarded rather than reported.
        let _ = std::fs::remove_file(&temp);
        return Err(Error::ApplyFailed {
            detail: format!(
                "the whole-file replacement of {position:?} did not complete ({e}); the temporary \
                 file was removed"
            ),
        });
    }
    Ok(())
}

/// Take the file away, and make the directory's new shape durable.
///
/// `unlink` is itself one directory operation, so a removal needs no temporary file and no rename.
/// **A removal of what is already gone is not a failure**: 43 T-10c's recovery is running the same
/// delta again, and the second run of a removal finds nothing -- which is the state the delta asked
/// for. That is [`gx_substrate::SubstrateAdapter::apply`]'s idempotence as **E-M4-3** quantifies it,
/// over the retry rather than over every state.
fn remove_whole_file(position: &str) -> Result<()> {
    let target = Path::new(position);
    match std::fs::remove_file(target) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(Error::ApplyFailed {
                detail: format!("{position:?} could not be removed: {e}"),
            })
        }
    }
    let parent = target.parent().ok_or_else(|| Error::ApplyFailed {
        detail: format!("{position:?} has no directory to sync"),
    })?;
    std::fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| Error::ApplyFailed {
            detail: format!("the directory of {position:?} could not be synced: {e}"),
        })
}

/// What the substrate holds at `position` **now**, as a digest (ASM-69-1: content only).
///
/// # An absence and an empty file have one digest, and that follows from ASM-69-1
///
/// A removal leaves nothing to read, and `AppliedDelta` still needs a `resulting_digest` and a
/// postcondition. The value written is the digest of no content -- which is the same digest a file
/// with no bytes has, because "digest = **content only** (metadata excluded)" leaves an adapter nothing else to
/// tell the two apart. It is the narrow scope's cost, of the same family as the metadata residue the
/// crate root already discloses, and it is raised as **filed** in `req/74` §2 rather than papered over (sem: SEM-gx-adapter-fs-018)
/// with an invented marker: any marker byte string is also a file's possible content.
///
/// # Errors
/// [`Error::Unreadable`] when the position exists and will not answer.
fn observe(position: &str) -> Result<Cid> {
    match std::fs::read(position) {
        Ok(bytes) => Ok(content_digest(&bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(content_digest(&[])),
        Err(e) => Err(Error::Unreadable {
            locator: position.to_string(),
            detail: e.to_string(),
        }),
    }
}
