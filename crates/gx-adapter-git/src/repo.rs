// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The gix boundary: every call into gitoxide this crate makes, in one place.
//!
//! Spec: 41 §2's dependency line ("`dep: gix (gitoxide)`") and 32 FR-045's "implemented via gix". 41 §6 gives (sem: SEM-gx-adapter-git-055)
//! this workspace one encoder and one hash for **gx's own** values; git's object ids are git's, and
//! this module is the only place the two vocabularies meet.
//!
//! # One place, and what that buys
//!
//! `gx-adapter-fs` names `std::fs` in three modules and pays for it with a source scan per module
//! (`tests/plan_purity.rs`). Here every gitoxide call is behind this module's functions, so "`plan`
//! writes nothing" is checkable by reading one import list rather than by classifying call sites -- (sem: SEM-gx-adapter-git-056)
//! and the day gitoxide's API moves, one file moves with it.
//!
//! # No repository is held (**AC-046**)
//!
//! [`crate::GitAdapter`] holds nothing and every function here opens the repository it was given.
//! `Send + Sync` is then free rather than argued for, which matters more for git than for a
//! filesystem: `gix::Repository` carries caches that are not `Sync`, so an adapter that kept one open
//! could not have crossed the `Box<dyn SubstrateAdapter>` boundary 41 §4 requires at all. What it
//! costs is an open per call, and the measurement of that cost belongs to M7 hand 5's bench rather
//! than to a guess here.
//!
//! # The clock, in the one place it would have got in
//!
//! [`gx_signature`] is the whole of the identity this adapter writes into git, and its time is **the
//! epoch**. 41 §6: "randomness and time are injected at the engine boundary (for deterministic replay)" -- and a commit's timestamp is (sem: SEM-gx-adapter-git-057)
//! part of its object id, so an adapter that read a clock would mint a different commit for the same
//! change on every attempt. 51 §7 contract 7's idempotence would then be unreachable by construction,
//! not merely unimplemented.
//!
//! 🔴 The identity is **gx's own** and not the intent's actor, and that is a decision rather than an
//! omission. A commit header is an unauthenticated string: anybody can write any name into one. The
//! actor of a gx change is a **signed** fact in the receipt (42 §3.10, gx-witness), so putting it in
//! a commit header as well would publish an unverifiable copy of a verifiable claim, in the place a
//! reader is most likely to trust it. `git log` says gx made the change; the receipt says who asked.

use gx_canon::cid::{self, Domain};
use gx_core::Cid;
use gx_substrate::{Error, Result};

use gix::bstr::{BStr, BString, ByteSlice};
use gix::objs::tree::{Entry, EntryKind};
use gix::objs::{Commit, Tree};
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
use gix::refs::Target;
use gix::ObjectId;

use crate::locator::Position;

/// The name and address this adapter writes into every commit and reflog entry it makes.
pub const GX_NAME: &str = "gx";
/// The address, in a domain reserved for the purpose (RFC 2606 `.invalid`) so that nobody's mail
/// server ever receives a message about a commit gx wrote.
pub const GX_EMAIL: &str = "gx@glovrex.invalid";

/// The identity and the moment, both fixed (see the module documentation).
#[must_use]
pub fn gx_signature() -> gix::actor::Signature {
    gix::actor::Signature {
        name: BString::from(GX_NAME),
        email: BString::from(GX_EMAIL),
        time: gix::date::Time {
            seconds: 0,
            offset: 0,
        },
    }
}

/// The digest of a byte string: the whole of what an object's digest covers.
///
/// Through gx-canon, because 41 §6 admits no second place where bytes become a digest, and
/// `Domain::Leaf` for the reason the fs adapter gives -- content is bytes and not a projected value,
/// so there is no `IdentityView` to go through. It is the **same function** the fs adapter uses, so a
/// file with the same bytes has the same object digest whichever substrate holds it.
#[must_use]
pub fn content_digest(content: &[u8]) -> Cid {
    cid::mint(Domain::Leaf, &[content])
}

/// The digest of "there is nothing here".
///
/// 🔴 The same value as the digest of an **empty** entry, and the same residue `gx-adapter-fs` records
/// for its own absence: "digest = content only" leaves an adapter nothing with which to distinguish "no entry
/// at this path" from "an entry with no bytes". Inventing a marker would not help -- any byte string a (sem: SEM-gx-adapter-git-058)
/// marker used is also a possible content -- so the fix belongs with a wider `Fingerprint` (v0.2) and
/// this paragraph is the disclosure rather than a workaround.
#[must_use]
pub fn absent_digest() -> Cid {
    content_digest(&[])
}

/// Open the repository a position names.
///
/// # Errors
/// [`Error::Unreadable`] when the path is not a repository this process can open.
pub fn open(position: &Position) -> Result<gix::Repository> {
    gix::open(position.repository()).map_err(|e| Error::Unreadable {
        locator: position.repository().to_string(),
        detail: format!("gix could not open this repository: {e}"),
    })
}

/// The commit a branch points at, or `None` when the branch has no commits (git's "unborn"). (sem: SEM-gx-adapter-git-059)
///
/// The absence is a **state** and not a failure, which is why it is an `Option` here and an error
/// only where a caller needs a commit. It is the state `git init` leaves and the state
/// [`crate::invert`] answers `Ok(None)` about.
///
/// # Errors
/// [`Error::Unreadable`] when the reference store will not answer, or when the reference is symbolic
/// and does not resolve to an object.
pub fn tip(repo: &gix::Repository, position: &Position) -> Result<Option<ObjectId>> {
    let unreadable = |detail: String| Error::Unreadable {
        locator: position.scope(),
        detail,
    };
    let found = repo
        .try_find_reference(position.reference())
        .map_err(|e| unreadable(format!("the reference store would not answer: {e}")))?;
    let Some(mut reference) = found else {
        return Ok(None);
    };
    // Peeled rather than read raw: a symbolic reference names another reference and a tag names a
    // commit through an object, and both are "where does this branch point" to a caller. The (sem: SEM-gx-adapter-git-060)
    // deprecated in-place form is the one gitoxide renamed in 0.86; `peel_to_id` is the same walk
    // under the name that is not deprecated, and it still borrows mutably because peeling caches
    // what it resolved.
    let id = reference
        .peel_to_id()
        .map_err(|e| unreadable(format!("the reference does not resolve to an object: {e}")))?;
    Ok(Some(id.detach()))
}

/// The bytes of the entry at a position's path, under a given commit.
///
/// `None` when the tree holds no entry of that name, or when the entry is not a blob -- a directory
/// where a file was expected is "there is no file here" to this adapter, which is the same answer (sem: SEM-gx-adapter-git-061)
/// `gx-adapter-fs` gives for a path that is not a regular file.
///
/// # Errors
/// [`Error::Unreadable`] when the commit, its tree or the blob cannot be read.
pub fn entry_content(
    repo: &gix::Repository,
    commit: ObjectId,
    position: &Position,
) -> Result<Option<Vec<u8>>> {
    let unreadable = |detail: String| Error::Unreadable {
        locator: position.locator(),
        detail,
    };
    let tree = repo
        .find_object(commit)
        .map_err(|e| {
            unreadable(format!(
                "commit {commit} is not in this object database: {e}"
            ))
        })?
        .try_into_commit()
        .map_err(|e| unreadable(format!("{commit} is not a commit: {e}")))?
        .tree()
        .map_err(|e| unreadable(format!("commit {commit} has no readable tree: {e}")))?;

    let Some(entry) = tree
        .lookup_entry_by_path(position.path())
        .map_err(|e| unreadable(format!("the tree would not answer for this path: {e}")))?
    else {
        return Ok(None);
    };
    if !entry.mode().is_blob() {
        return Ok(None);
    }
    let object = repo.find_object(entry.oid()).map_err(|e| {
        unreadable(format!(
            "the entry's blob is not in the object database: {e}"
        ))
    })?;
    Ok(Some(object.data.clone()))
}

/// The entries of a commit's tree, in git's own order.
///
/// # Errors
/// [`Error::Unreadable`] as [`entry_content`].
fn entries_of(repo: &gix::Repository, commit: ObjectId, position: &Position) -> Result<Vec<Entry>> {
    let unreadable = |detail: String| Error::Unreadable {
        locator: position.scope(),
        detail,
    };
    let tree = repo
        .find_object(commit)
        .map_err(|e| {
            unreadable(format!(
                "commit {commit} is not in this object database: {e}"
            ))
        })?
        .try_into_commit()
        .map_err(|e| unreadable(format!("{commit} is not a commit: {e}")))?
        .tree()
        .map_err(|e| unreadable(format!("commit {commit} has no readable tree: {e}")))?;
    let mut out = Vec::new();
    for entry in tree.iter() {
        let entry = entry.map_err(|e| unreadable(format!("a tree entry would not decode: {e}")))?;
        out.push(Entry {
            mode: entry.mode(),
            filename: entry.filename().to_owned(),
            oid: entry.oid().to_owned(),
        });
    }
    Ok(out)
}

/// Write the objects a change needs and answer the commit that carries it.
///
/// `parent` is the branch tip **read at this moment**, which is the state the engine's CAS has just
/// declared current (41 §5-5b). It is deliberately not in the payload: the crate root argues why (L1
/// quantifies `plan`'s determinism over `(intent, pre)`, and a tip in the payload would make the same
/// intent plan a different delta whenever the branch moved).
///
/// The tree is the parent's, with one entry replaced, inserted or removed. Sorted before it is
/// written, because git's tree format is ordered and an unsorted tree is a different object that
/// every other git implementation would refuse.
///
/// # Errors
/// [`Error::ApplyFailed`] when an object cannot be written, [`Error::Unreadable`] when the parent's
/// tree cannot be read.
pub fn commit_entry(
    repo: &gix::Repository,
    parent: Option<ObjectId>,
    position: &Position,
    content: Option<&[u8]>,
) -> Result<ObjectId> {
    let failed = |detail: String| Error::ApplyFailed { detail };

    let mut entries = match parent {
        Some(parent) => entries_of(repo, parent, position)?,
        None => Vec::new(),
    };
    let filename = BString::from(position.path());
    entries.retain(|e| e.filename != filename);
    if let Some(content) = content {
        let blob = repo
            .write_blob(content)
            .map_err(|e| failed(format!("the blob would not be written: {e}")))?
            .detach();
        entries.push(Entry {
            mode: EntryKind::Blob.into(),
            filename,
            oid: blob,
        });
    }
    entries.sort();

    let tree = repo
        .write_object(&Tree { entries })
        .map_err(|e| failed(format!("the tree would not be written: {e}")))?
        .detach();
    let signature = gx_signature();
    let commit = Commit {
        tree,
        parents: parent.into_iter().collect(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        // Deterministic, and it names the position rather than the moment: two applications of one
        // delta on one tip have to produce one commit id, and a message carrying anything that
        // varies would break that before the timestamp got the chance to.
        message: BString::from(format!("gx: {}\n", position.locator())),
        extra_headers: Vec::new(),
    };
    repo.write_object(&commit)
        .map(gix::Id::detach)
        .map_err(|e| failed(format!("the commit would not be written: {e}")))
}

/// Move a branch, refusing if it is not where the caller last saw it.
///
/// The `expected` value is the tip read a few lines earlier by the caller, so this is a
/// **compare-and-swap local to `apply`** and not the engine's: 41 §5-5b's CAS is over a
/// `Fingerprint` and happens before `apply` is called at all. This one closes the window between the
/// read and the write, which is the window a second process editing the same branch would land in.
///
/// The reflog entry carries [`gx_signature`], so no repository configuration is consulted and the
/// entry is the same on every machine. gitoxide's own `Repository::edit_reference` would read
/// `user.name`/`user.email` and fail where they are unset -- which is most automation.
///
/// # Errors
/// [`Error::ApplyFailed`] when the reference cannot be locked, or when it is not where `expected`
/// says.
pub fn move_branch(
    repo: &gix::Repository,
    position: &Position,
    expected: Option<ObjectId>,
    new: ObjectId,
) -> Result<()> {
    let name = position.reference();
    let full: gix::refs::FullName =
        name.try_into()
            .map_err(|e: gix::refs::name::Error| Error::NotAPosition {
                locator: position.locator(),
                normalised: format!("{name} ({e})"),
            })?;
    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: BString::from(format!("gx: {}", position.locator())),
            },
            expected: match expected {
                Some(old) => PreviousValue::MustExistAndMatch(Target::Object(old)),
                None => PreviousValue::MustNotExist,
            },
            new: Target::Object(new),
        },
        name: full,
        deref: false,
    };
    // The two stages have two error types and neither converts into the other, so both are folded
    // into one sentence here rather than through a `?` that would have to pick one of them.
    let refused = |e: String| Error::ApplyFailed {
        detail: format!(
            "{} would not move to {new}: {e}. The expected previous value is the tip this apply \
             read a moment earlier, so a refusal here is somebody else editing the same branch",
            position.scope()
        ),
    };
    let signature = gx_signature();
    let prepared = repo
        .refs
        .transaction()
        .prepare(
            vec![edit],
            gix::lock::acquire::Fail::Immediately,
            gix::lock::acquire::Fail::Immediately,
        )
        .map_err(|e| refused(e.to_string()))?;
    prepared
        .commit(signature.to_ref(&mut gix::date::parse::TimeBuf::default()))
        .map_err(|e| refused(e.to_string()))?;
    Ok(())
}

/// Read a commit id out of a payload's text field.
///
/// # Errors
/// [`Error::PayloadUnreadable`] for text that is not an object id this repository's hash could have
/// produced.
pub fn object_id(text: &str) -> Result<ObjectId> {
    ObjectId::from_hex(text.as_bytes()).map_err(|e| Error::PayloadUnreadable {
        detail: format!("{text:?} is not a git object id: {e}"),
    })
}

/// The text form of a commit id, which is git's own lower-case hexadecimal.
#[must_use]
pub fn object_text(id: ObjectId) -> String {
    id.to_string()
}

/// A borrowed byte string as text, for a message.
#[must_use]
pub fn show(bytes: &BStr) -> String {
    bytes.to_str_lossy().into_owned()
}
