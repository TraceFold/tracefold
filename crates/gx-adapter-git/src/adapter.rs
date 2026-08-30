// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The seven methods of 41 §4, all seven in one hand.
//!
//! There is no [`Error::Unimplemented`] in this file, which is the fact 51 §7's completion condition
//! turns on: the harness reports "none" for that variant and for nothing else, so an adapter with none
//! of them is an adapter every obligation was **run** against (**§31 M4H3-4 (b)**). The word stays in
//! the vocabulary regardless — `gx-adapter-mcp` is M7 hand 3 and may stand up one method at a time,
//! which is what §32 M4H4-2 confirmed made the word permanent for. (sem: SEM-gx-adapter-git-010)

use gx_canon::cid;
use gx_core::{
    Cid, Commutation, Fingerprint, Intent, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind,
};
use gx_substrate::{
    elide_scope, AppliedDelta, Error, InvertOutcome, PlannedDelta, Result, SubstrateAdapter,
};

use crate::locator::{self, Position};
use crate::repo;

/// The git adapter (41 §2, 41 §4).
///
/// It holds nothing, and here that is load-bearing rather than tidy. `gix::Repository` carries caches
/// that are not `Sync`, so an adapter that kept one open could not satisfy 41 §4's `Send + Sync`
/// bound and could never cross the `Box<dyn SubstrateAdapter>` boundary **AC-046** measures. Every
/// method opens the repository its locator names; what that costs is a bench's question (M7 hand 5)
/// and not a guess here.
#[derive(Clone, Copy, Debug, Default)]
pub struct GitAdapter;

impl GitAdapter {
    /// One adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// The state of one entry: its bytes, or the refusal that says why there are none.
    ///
    /// A position whose branch is unborn, or whose tree holds no such entry, answers
    /// [`Error::Unreadable`] — the same answer `gx-adapter-fs` gives for a file that is not there, and
    /// for the same reason: `snapshot` **reads**, and v0.1 has no way to describe "this position is
    /// empty" as an `ObjectSnapshot`. A creation is therefore planned against a snapshot the caller (sem: SEM-gx-adapter-git-011)
    /// builds (the fs adapter's `absent_snapshot`, and this crate's fixture does the same).
    fn read(position: &Position) -> Result<Vec<u8>> {
        let repository = repo::open(position)?;
        let tip = repo::tip(&repository, position)?.ok_or_else(|| Error::Unreadable {
            locator: position.locator(),
            detail: format!(
                "{} has no commits on it (git calls the state 'unborn'), so there is no tree to \
                 read an entry out of (sem: SEM-gx-adapter-git-012)",
                position.scope()
            ),
        })?;
        repo::entry_content(&repository, tip, position)?.ok_or_else(|| Error::Unreadable {
            locator: position.locator(),
            detail: format!("commit {tip} has no blob at this path"),
        })
    }
}

impl SubstrateAdapter for GitAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Git
    }

    /// The state of one entry, named by its own projection.
    ///
    /// The locator is parsed on the way in and the snapshot reports the **normalised** spelling: 41 §4
    /// has `snapshot` receive one already normalised (**H-2**), normalising again is free (L7's
    /// idempotence), and it stops a caller's spelling from reaching a gate through a snapshot.
    /// `representation` is [`ReprKind::Bytes`]: this adapter reads blobs and does not parse them.
    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        let position = locator::parse(locator)?;
        let content = Self::read(&position)?;
        let digest = repo::content_digest(&content);

        // 42 §1.3 row 1 excludes `id` from the projection, so the placeholder cannot reach the
        // digest — the same argument `PlannedDelta::new` makes about `reference`.
        let placeholder = ObjectSnapshot::new(
            ObjectId(Cid([0u8; 32])),
            SubstrateKind::Git,
            position.locator(),
            digest,
            ReprKind::Bytes,
        );
        let id = cid::compute(&placeholder).map_err(|e| Error::NotDigestible {
            detail: e.to_string(),
        })?;
        Ok(ObjectSnapshot::new(
            ObjectId(id),
            SubstrateKind::Git,
            position.locator(),
            digest,
            ReprKind::Bytes,
        ))
    }

    /// Delegates to [`crate::plan`], which names no gitoxide call at all (**E-M4-29**, L1).
    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        crate::plan::plan(intent, pre)
    }

    /// Name the state a commit is conditional on — and for git that state is the **branch**.
    ///
    /// 42 §3.5 lets a scope reach past the object itself to "the surrounding state that could interfere with the target", and a branch is (sem: SEM-gx-adapter-git-013)
    /// exactly that for an entry in its tree: any change to any file on it moves the tip, so a CAS
    /// over the entry alone would admit a commit built on a tip that had already moved. The digest is
    /// over the tip's own object id, so the fingerprint changes when the branch does and not
    /// otherwise.
    ///
    /// An over-long scope becomes a digest line before it reaches [`Fingerprint::new`]
    /// (**M4H1-2**, [`elide_scope`]); the bound itself is gx-core's and refuses anything that arrives
    /// past it. That is the single road **req/98** §3-4's reserved item 6 asks the M7 adapters to take. (sem: SEM-gx-adapter-git-014)
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        let position = locator::parse(snap.locator())?;
        let repository = repo::open(&position)?;
        let tip = repo::tip(&repository, &position)?.ok_or_else(|| Error::Unreadable {
            locator: position.scope(),
            detail: "the branch has no commits on it, so it names no state a commit could be \
                     conditional on"
                .to_string(),
        })?;
        Ok(Fingerprint::new(
            SubstrateKind::Git,
            elide_scope(position.scope())?,
            repo::content_digest(tip.as_bytes()),
        )?)
    }

    /// Delegates to [`crate::apply`], where the short circuit that makes a retry a retry lives.
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        crate::apply::apply(delta)
    }

    /// Delegates to [`crate::invert`]. **E-M4-30**: the escrowed inverse is constructed **before**
    /// `apply` (43 T-10b), because the tip a reset names is "where the branch is now". (sem: SEM-gx-adapter-git-015)
    /// 🔴 **E-DR4626-1 (DR-46-26)** -- the free function is untouched and the outcome is derived
    /// here, in one line, by `InvertOutcome::from_option`.
    ///
    /// 🔴🔴 **RETRACTED by DEFECT-892-1 (`req/895` §1).** The paragraph that stood here said this
    /// adapter "builds its inverse out of the snapshot it was handed" and that "`reads` is empty
    /// because the escrow read nothing through a transport". It reads `repo::tip` — the branch —
    /// and the empty list became `ReadSet::Nothing` in signed receipts, which is a positive claim
    /// that no object was read. `crate::invert` now returns the outcome with the entry in it.
    ///
    /// C-25's `Unknown` remains unreachable here and that half of the old paragraph stands: the
    /// `OnReadFailure::Unknown` posture is a `gx-adapter-mcp` catalogue's construct, and a read
    /// failure on this substrate leaves as an [`Error::Unreadable`].
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
        crate::invert::invert(delta, pre)
    }

    /// Delegates to [`crate::commutation`], which compares **branches** and touches no repository
    /// (**M4-25, adopted (a)**, AC-052, AC-053). (sem: SEM-gx-adapter-git-016)
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        crate::commutation::commutation(a, b)
    }
}
