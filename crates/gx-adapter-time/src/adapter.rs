// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The seven methods of 41 §4, all seven of them.
//!
//! No method of this adapter answers [`gx_substrate::Error::Unimplemented`], which is the variant
//! the shared harness reads as "none" (**§31 M4H3-4 (b)**), so every one of 51 §7's obligations has
//! a subject here. The word stays in the vocabulary all the same: "unimplemented" and "failed" are
//! permanently different facts.

use gx_canon::cid::{self, Domain};
use gx_core::{
    Cid, Commutation, Fingerprint, Intent, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind,
};
use gx_substrate::{
    elide_scope, AppliedDelta, Error, InvertOutcome, PlannedDelta, Result, SubstrateAdapter,
};

use crate::locator;

/// This adapter's [`SubstrateKind`].
///
/// `Custom("time")` and not a new variant: 42 §3.1 fixes four values -- `Fs`, `Git`, `Mcp` and
/// `Custom` -- and the fourth is the seat a substrate outside the original three sits in.
/// `gx-adapter-postgres` and `gx-adapter-mysql` are already there. ∴ **a new substrate costs zero
/// changes to gx-core, to the wire, and to the journal vocabulary**: the line was laid before this
/// crate existed, which is the same finding `req/1020` made about `promised_target`.
#[must_use]
pub fn kind() -> SubstrateKind {
    SubstrateKind::Custom("time".to_string())
}

/// The time adapter (41 §2, 41 §4).
///
/// It holds nothing -- no root, no handle, no cache -- so `Send + Sync` is free rather than argued
/// for (AC-046), and two threads planning against one adapter share no state to disagree about.
#[derive(Clone, Copy, Debug, Default)]
pub struct TimeAdapter;

/// The digest of a position's bytes: the whole of what a fingerprint here covers.
///
/// 🔴 It covers **the entry entire, firedness included**. That is the load-bearing choice of this
/// crate (crate root): because the record of whether an action has run is inside the digested
/// bytes, the engine's compare-and-set stops matching an escrowed fingerprint the moment the runner
/// marks the entry fired, and an undo attempted after the firing is refused by machinery that was
/// already there. A digest that covered only the action and the moment would leave that undo
/// looking exactly like an undo of something that had not run.
///
/// Through `gx-canon`, because 41 §6 admits no second place where bytes become a digest, and as a
/// free function because `snapshot`, `precondition` and `apply`'s observation must all reach the
/// same one.
#[must_use]
pub fn content_digest(content: &[u8]) -> Cid {
    cid::mint(Domain::Leaf, &[content])
}

/// The digest of "nothing is scheduled here".
///
/// 🔴 The same value as the digest of an **empty** file, the residue `gx-adapter-fs`,
/// `gx-adapter-git` and `gx-adapter-postgres` each disclose for their own substrates: "digest =
/// content only" leaves an adapter nothing with which to separate "nothing here" from "here, and
/// empty", and any marker byte string would also be a possible content. Here the collision is
/// narrower than it is for a filesystem -- an empty file is not a schedule entry, so
/// [`crate::entry::Entry::decode`] refuses it and no road treats it as one -- but the digests still
/// coincide, and the disclosure travels with the value rather than being argued away.
#[must_use]
pub fn absent_digest() -> Cid {
    content_digest(&[])
}

impl TimeAdapter {
    /// One adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Read a position, or say which locator would not answer.
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

impl SubstrateAdapter for TimeAdapter {
    fn kind(&self) -> SubstrateKind {
        kind()
    }

    /// The state of one schedule entry, named by its own projection.
    ///
    /// `representation` is [`ReprKind::Bytes`]. 🔴 Not [`ReprKind::Json`]: the entry is canonical
    /// DAG-CBOR, the enum has no member for it, and naming the wrong one would be a hint that lies
    /// to every tool that reads it. `Bytes` is the member that assumes nothing, and the residue --
    /// that 42 §3.1's four members do not name this encoding -- is disclosed here rather than
    /// papered over.
    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        let normalised = locator::normalize(locator);
        let content = Self::read(&normalised)?;
        let digest = content_digest(&content);

        // 42 §1.3 row 1 excludes `id` from the projection, so the placeholder cannot reach the
        // digest.
        let placeholder = ObjectSnapshot::new(
            ObjectId(Cid([0u8; 32])),
            kind(),
            normalised.clone(),
            digest,
            ReprKind::Bytes,
        );
        let id = cid::compute(&placeholder).map_err(|e| Error::NotDigestible {
            detail: e.to_string(),
        })?;
        Ok(ObjectSnapshot::new(
            ObjectId(id),
            kind(),
            normalised,
            digest,
            ReprKind::Bytes,
        ))
    }

    /// Delegates to [`crate::plan`], which names no filesystem operation and no clock.
    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        crate::plan::plan(intent, pre)
    }

    /// Name the state a commit is conditional on.
    ///
    /// 🔴 This is where the undo window is enforced, and it is enforced by being ordinary. The
    /// digest is of the entry entire, so an entry the runner has marked fired does not compare equal
    /// to the entry the escrow was taken against, and the engine's CAS refuses the commit. Nothing
    /// in this method knows what firedness is; it does not have to.
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        let scope = locator::normalize(snap.locator());
        let content = Self::read(&scope)?;
        let digest = content_digest(&content);
        Ok(Fingerprint::new(kind(), elide_scope(scope)?, digest)?)
    }

    /// Delegates to [`crate::apply`].
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        crate::apply::apply(delta)
    }

    /// Delegates to [`crate::invert`], which is called **before** `apply` (43 T-10b, **E-M4-30**)
    /// and which answers all three of C-25's values -- see that module for which, and why the
    /// middle one is not a failure.
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
        crate::invert::invert(delta, pre)
    }

    /// Delegates to [`crate::commutation`], which decides independence from the two payloads and
    /// touches no substrate.
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        crate::commutation::commutation(a, b)
    }
}
