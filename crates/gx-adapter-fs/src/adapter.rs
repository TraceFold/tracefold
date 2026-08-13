//! The seven methods of 41 §4, all seven of them as of hand 6.
//!
//! Four arrived in hand 4 (`kind`, `snapshot`, `plan`, `precondition`), two in hand 5 (`apply`,
//! `invert`) and `commutation` here. There is no [`Error::Unimplemented`] left in this file, which is
//! the fact 51 §7's completion condition turns on -- the harness reports 「無い」 for that variant and
//! for nothing else, so an adapter with none of them is an adapter every obligation was **run**
//! against (**§31 M4H3-4 (b)**). The word stays in the vocabulary regardless: §32 M4H4-2 追認 made it
//! permanent, because M7's git and mcp adapters will stand up one hand at a time as this one did.

use gx_canon::cid::{self, Domain};
use gx_core::{
    Cid, Commutation, Fingerprint, Intent, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind,
};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result, SubstrateAdapter};

use crate::locator;

/// The filesystem adapter (41 §2, 41 §4).
///
/// It holds nothing. A locator is an absolute path and the adapter has no root, no cache and no
/// handle: `Send + Sync` is then free rather than argued for, and two threads planning against one
/// adapter share no state to disagree about. **N-05** is why there is no confinement here either --
/// Landlock is v0.2+ -- and the crate root says what that leaves open.
#[derive(Clone, Copy, Debug, Default)]
pub struct FsAdapter;

/// The digest of a file's bytes: the whole of what a fingerprint covers (**ASM-69-1**).
///
/// Through gx-canon, because 41 §6 admits no second place where bytes become a digest.
/// `Domain::Leaf` rather than [`cid::compute`]: content is bytes and not a projected value, so
/// there is no `IdentityView` to go through, and the mint is the road E-M2-12 opened for it.
///
/// A free function since hand 5, because `snapshot`, `precondition` and [`crate::apply`]'s
/// observation all have to reach the same one -- an adapter with two ways of digesting a file would
/// be an adapter whose CAS check and whose `resulting_digest` could disagree about one state.
pub(crate) fn content_digest(content: &[u8]) -> Cid {
    cid::mint(Domain::Leaf, &[content])
}

impl FsAdapter {
    /// One adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Read a file, or say which locator would not answer.
    fn read(locator: &str) -> Result<Vec<u8>> {
        if !crate::locator::is_absolute(locator) {
            return Err(Error::Unreadable {
                locator: locator.to_string(),
                detail: "not a position from the root; v0.1 names positions absolutely (ASM-69-3)"
                    .to_string(),
            });
        }
        std::fs::read(locator).map_err(|e| Error::Unreadable {
            locator: locator.to_string(),
            detail: e.to_string(),
        })
    }
}

impl SubstrateAdapter for FsAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    /// The state of one file, named by its own projection.
    ///
    /// The locator is normalised on the way in and the snapshot reports the normalised spelling:
    /// 41 §4 has `snapshot` receive an already-normalised locator (**H-2**), and normalising again
    /// is free (L7's idempotence) and stops a caller's spelling from reaching a gate through a
    /// snapshot. `representation` is [`ReprKind::Bytes`]: this adapter reads files and does not
    /// parse them.
    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        let normalised = locator::normalize(locator);
        let content = Self::read(&normalised)?;
        let digest = content_digest(&content);

        // 42 §1.3 row 1 excludes `id` from the projection, so the placeholder cannot reach the
        // digest -- the same argument `PlannedDelta::new` makes about `reference`.
        let placeholder = ObjectSnapshot::new(
            ObjectId(Cid([0u8; 32])),
            SubstrateKind::Fs,
            normalised.clone(),
            digest,
            ReprKind::Bytes,
        );
        let id = cid::compute(&placeholder).map_err(|e| Error::NotDigestible {
            detail: e.to_string(),
        })?;
        Ok(ObjectSnapshot::new(
            ObjectId(id),
            SubstrateKind::Fs,
            normalised,
            digest,
            ReprKind::Bytes,
        ))
    }

    /// Delegates to [`crate::plan`], which names no filesystem operation (**E-M4-29**).
    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        crate::plan::plan(intent, pre)
    }

    /// Name the state a commit is conditional on (**ASM-69-1**).
    ///
    /// `scope` is the normalised absolute path and `digest` is the file's content -- 「metadata 除外」,
    /// for the reason the crate root gives at length: hand 5's `apply` renames, a rename always
    /// writes a new mtime, and a digest covering metadata would make the second `apply` of one delta
    /// observe a state the first never saw (L2).
    ///
    /// An over-long scope becomes a digest line before it reaches [`Fingerprint::new`]
    /// (**M4H1-2**); the bound itself is gx-core's and refuses anything that arrives past it.
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        let scope = locator::normalize(snap.locator());
        let content = Self::read(&scope)?;
        let digest = content_digest(&content);
        Ok(Fingerprint::new(
            SubstrateKind::Fs,
            elide_scope(scope)?,
            digest,
        )?)
    }

    /// Delegates to [`crate::apply`], which is where the three steps around the `rename` are and
    /// where **E-M4-31**'s `Timestamp(0)` placeholder is written.
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        crate::apply::apply(delta)
    }

    /// Delegates to [`crate::invert`]. **E-M4-30**: the escrowed inverse is constructed **before**
    /// `apply` (43 T-10b), because the inverse of an overwrite carries the old content and after the
    /// apply there is none.
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>> {
        crate::invert::invert(delta, pre)
    }

    /// Delegates to [`crate::commutation`], which decides independence from the two payloads and
    /// touches no substrate (**M4-25 採(a)**, AC-052, AC-053).
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        crate::commutation::commutation(a, b)
    }
}
