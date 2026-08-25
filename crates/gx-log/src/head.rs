// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R6 / DR-43-11** — the head a project keeps, so that "this ledger is intact" can become
//! "this ledger has not gone backwards".
//!
//! # Why identity is not monotonicity
//!
//! DR-43-9 gave the journal an identity: every record carries a link over its own bytes and its
//! predecessor's, so "is this the file I read" is one comparison of 32 bytes. `req/229` H-01
//! measured the sentence that machine cannot say. A chain is **prefix-closed** — the first `i`
//! records of a chained journal are themselves a perfectly chained journal, because `link_i`
//! depends only on `link_{i-1}` and `payload_i` — and the ledger is a sequence of framed leaves, so
//! its prefixes are perfect ledgers too. ∴ an attacker who can write to both files needs **no hash,
//! no codec and no key**: `truncate(2)` at two frame boundaries produces a pair that agrees with
//! itself, passes every gate, reports `journal_intact: true` and `remedy: null`, and — when the cut
//! falls between a `ledger.append` and its `Committed` record — hands 43 §7-3b's recovery a row it
//! is willing to re-apply. The audit watched an operator's file go from `three` back to `two` with
//! `recover.refused: 0` on the start-up line.
//!
//! Nothing **inside** the pair can answer this. Both files are, by construction, consistent with
//! the shorter history. The missing statement is about the project's past: *this project has
//! already reached a tree of `n` leaves whose root was `r`*. So it is written down, once per write,
//! and read back at every door.
//!
//! # What is signed and what is not, stated rather than implied
//!
//! The [`PersistedHead::checkpoint`] is a signed [`Checkpoint`] — 42 §3.11's "a signed statement
//! about the tree at that moment" — and it is the artefact an auditor takes **out of the box**
//! (`gx checkpoint export`). Its signature is what makes a copy verifiable by somebody who holds no
//! key of ours.
//!
//! The two journal numbers beside it ([`PersistedHead::journal_len`], [`PersistedHead::
//! journal_head`]) are **not** covered by that signature. They are a local witness, kept because
//! the journal is the file 43 §7's recovery reads and a tree statement says nothing about it. An
//! attacker who can rewrite this file can rewrite those two numbers; the same attacker can delete
//! the file outright. ∴ this file is a **detector**, not a proof, and the proof is the copy that
//! left the machine. That asymmetry is the whole of `req/229` §7-4 and it is written into the
//! product's limits rather than left for a reader to derive.
//!
//! # 🔴 **R7 / 43 §7.9** — which threat model each half of this file is for
//!
//! The paragraphs above are kept because they are what R6 claimed, and the seventh adversarial
//! audit (`req/232`) measured two things they did not say.
//!
//! **H-01**: nothing checked the checkpoint's signature at a door, so the detector did not have to
//! be *deleted* to be switched off — a `head.json` with `tree_size: 0` written over the top left
//! `head_recorded: true` in the report, every gate open, and an operator's file back at `two`. The
//! repair is here and it is cheap: the checkpoint is verified when a key for it can be found, the
//! local numbers travel in a signed [`HeadWitness`] rather than beside the signature, and the
//! `.gx/VERSION` digest joins them so that R6's downgrade refusal cannot be lifted by *rewriting*
//! the declaration (M-02).
//!
//! **H-02**: an *older head this project itself signed*, put back in place, verifies perfectly —
//! and gx accepts it, because a signature says **who** and not **when**. That is a freshness
//! question, and freshness is not decidable inside the project: every witness to it is in the same
//! write scope as the thing it witnesses.
//!
//! ∴ 43 §7.9 splits the models and this file is honest about which one it serves:
//!
//! * **Model A** — accidents, crashes, partial writes, power loss, a second process, a restart, a
//!   mis-edit, an older binary, and any third party who **cannot write to `.gx/`**. Everything in
//!   this module is a real detector here, and H = 0 is the target.
//! * **Model B** — an adversary who can write to `.gx/`. Every detector in this file is inside that
//!   write scope, so they can be deleted, replaced or **rolled back to an older genuine version**.
//!   Signature checking raises the price and does not change the answer. What answers in Model B is
//!   the copy that left the machine (`gx checkpoint export`, `gx repair --against`, the commit
//!   receipts) and operating-system permissions, and 43 §7.9 says so in the same words.

use std::path::{Path, PathBuf};

use gx_core::{Checkpoint, Cid, DsseSignature};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The shape version of `.gx/checkpoints/head.json`.
///
/// A number rather than a marker byte, because this file is JSON an operator may read and a
/// document whose version is a field is one they can read the version of. A file whose version is
/// **newer** than this constant is refused rather than guessed at ([`HeadStore::read`]).
pub const HEAD_VERSION: u32 = 1;

/// The name [`HeadStore`] gives the file inside `.gx/checkpoints/`.
///
/// req/56 §2 already declares that directory — its cells read "signed checkpoints", "derived but
/// signed, therefore kept", "re-signing required" — so this lane adds a **file to a declared row**
/// rather than a row. `req/229` L-02 measured what
/// that row held before today: nothing, in every project the audit built, healthy or attacked.
pub const HEAD_FILE: &str = "head.json";

/// The furthest this project has been observed to reach.
///
/// Kept as plain numbers rather than as a [`Checkpoint`] so that the comparison at a door does not
/// depend on the document's shape, and so that a caller that has no persisted head at all (an
/// engine opened by a test, a project older than this release) can be handed `None` and behave
/// exactly as it did before. **Absence is not a rollback**: a project with no recorded head has
/// made no statement about its past, and inventing one from the files in front of us would be the
/// laundering `req/225` H-01 is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadFloor {
    /// The tree size the head was signed over.
    pub tree_size: u64,
    /// The Merkle root at that size.
    pub root_hash: Cid,
    /// How many bytes the journal held when the head was written.
    pub journal_len: u64,
    /// The journal's chain head at that length, or `None` for a legacy journal.
    pub journal_head: Option<[u8; 32]>,
    /// 🔴 **R7 / 43 §7.9 Model A** — the digest of `.gx/VERSION` when the head was written, in
    /// `Cid` text form ([`declaration_digest`]), or `None` for a head written before R7.
    ///
    /// `req/232` M-02 measured the hole this closes: `.gx/VERSION` could be **rewritten** (not
    /// deleted) to `journal_format=legacy`, and the whole of R6's downgrade refusal came off with
    /// it. Binding the declaration's bytes into the head means an old tool, a mis-edit or a restore
    /// from the wrong backup is caught by the same comparison as a shortened journal. It does
    /// **not** catch an adversary who can write to `.gx/` — that is Model B, and 43 §7.9 says so
    /// rather than leaving a reader to derive it.
    pub version_digest: Option<String>,
}

/// 🔴 **R7 / DR-43-11 (b)** — the local numbers a head carries, in the shape they are **signed** in.
///
/// # Why this exists as its own document
///
/// R6 put `journal_len` and `journal_head` beside a signed [`Checkpoint`] and said, in
/// [`PersistedHead`]'s own note, that the signature did not cover them. `req/232` H-01 measured the
/// consequence: a `head.json` whose `checkpoint.tree_size` was edited to `0` disabled every
/// detector while still reporting `head_recorded: true`, because nothing ever checked a signature
/// at a door. Signing the tree statement is not enough on its own either — the journal numbers and
/// the declaration digest are the half that says *which files* the tree statement is about.
///
/// So the numbers travel as a payload with a signature over them, and the payload repeats
/// `tree_size` and `root_hash` so that a verifier comparing this document with the checkpoint
/// beside it can tell that the two halves are about one moment rather than two.
///
/// **What this does not do**: an adversary holding the project's signing key can sign a witness of
/// their own, and an *older* witness this project itself signed verifies perfectly (`req/232`
/// H-02 — freshness, not authenticity). Both are Model B in 43 §7.9's sense.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadWitness {
    /// The tree size the checkpoint beside this witness states.
    pub tree_size: u64,
    /// The root at that size, in `Cid` text form.
    pub root_hash: String,
    /// The journal's length in bytes at that moment.
    pub journal_len: u64,
    /// Hex of the journal's chain head at that length, or `null` for a legacy journal.
    pub journal_head: Option<String>,
    /// `"chained"` or `"legacy"`.
    pub journal_format: String,
    /// The digest of `.gx/VERSION` in `Cid` text form, or `null` where the caller had no layout.
    pub version_digest: Option<String>,
    /// The last leaf hash at `tree_size` in `Cid` text form, or `null` for an empty tree.
    pub ledger_leaf_hash: Option<String>,
}

/// 🔴 **R7 / `req/38` §171 ruling 2(c)** — an operator's recorded decision to take a shorter tree.
///
/// `req/232` M-01 measured `gx repair --yes` re-creating a head **on top of** a tree a rollback had
/// shortened: the rolled-back state became the project's new attested floor, with no record that it
/// had ever been higher. That is laundering, and the repair is not to forbid the operation — an
/// operator restoring from a backup genuinely does need to move the floor — but to make it an
/// explicit, recorded act that requires evidence from outside the project (`gx repair
/// --accept-rollback --against <FILE>`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptedRollback {
    /// The tree size this project had already published before the rollback was accepted.
    pub was_tree_size: u64,
    /// The root at that size, in `Cid` text form.
    pub was_root_hash: String,
    /// The external checkpoint the operator offered as evidence.
    pub against: String,
    /// When the acceptance was recorded, as the engine's injected clock read it.
    pub at: i64,
}

/// What a door found when it compared the project in front of it with [`HeadFloor`].
///
/// Four shapes rather than one boolean, because the remedy differs and because a refusal that
/// cannot say *which* number went backwards is the refusal `req/227` M-01 named one file up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RolledBack {
    /// The tree holds fewer leaves than the project has already published.
    TreeShorter { was: u64, now: u64 },
    /// The tree is long enough and is **not** an extension of the published head: the root over
    /// the first `was` leaves is not the root that was signed.
    TreeDiverged {
        was: u64,
        expected: Cid,
        found: Option<Cid>,
    },
    /// The journal is shorter than it was when the head was written.
    JournalShorter { was: u64, now: u64 },
    /// The journal is long enough and its chain over the recorded prefix is not the recorded one.
    JournalDiverged { at: u64 },
    /// 🔴 **R7 / `req/232` M-02** — `.gx/VERSION` is not the file this project's head was written
    /// beside.
    VersionChanged { was: String, now: Option<String> },
}

impl RolledBack {
    /// The sentence a refusal carries, naming the two numbers.
    ///
    /// One spelling for every face — the start-up refusal, `/healthz`, the write gate and
    /// `gx repair` — for the reason `req/38` §156 ruling 2(a) gives about `LEDGER_DISAGREES`: a
    /// monitor that has to know which of five faces spells a condition differently is a monitor
    /// that will get it wrong.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            RolledBack::TreeShorter { was, now } => format!(
                "rolled_back: this project has already published a signed head over {was} \
                 leaf/leaves and its ledger now holds {now}. A shorter ledger is not a repair and \
                 not a crash — a crash leaves a torn tail, and this file ends on a frame boundary \
                 (DR-43-11, req/229 H-01)"
            ),
            RolledBack::TreeDiverged {
                was,
                expected,
                found,
            } => {
                format!(
                "rolled_back: this project published a signed head over {was} leaf/leaves whose \
                 root was {}, and the tree in front of us has a different root at that size ({}). \
                 The ledger is not an extension of the history this project has already attested \
                 (DR-43-11, req/229 H-01)",
                expected.to_text(),
                found.as_ref().map_or_else(|| "none".to_string(), Cid::to_text),
            )
            }
            RolledBack::JournalShorter { was, now } => format!(
                "rolled_back: this project's journal held {was} byte(s) when its last head was \
                 signed and holds {now} now. A journal that shrank on a frame boundary is not a \
                 torn tail (DR-43-11, req/229 H-01)"
            ),
            RolledBack::JournalDiverged { at } => format!(
                "rolled_back: this project's journal is long enough but the chain over its first \
                 {at} byte(s) is not the chain the last signed head recorded: the prefix was \
                 rewritten (DR-43-11, req/229 H-01)"
            ),
            RolledBack::VersionChanged { was, now } => format!(
                "rolled_back: this project's `.gx/VERSION` digested to {was} when its last head was \
                 signed and digests to {} now. The declaration that says which framing this \
                 project's journal is in has been rewritten — R6 refused a *deleted* declaration \
                 and this is the same fact one edit further in (req/232 M-02, 43 §7.9 Model A)",
                now.as_deref().unwrap_or("nothing (the file is gone)")
            ),
        }
    }
}

/// `.gx/checkpoints/head.json`, as it lies on the disk.
///
/// # Errors of shape are refusals, not guesses
///
/// A file that will not parse is an [`Error::Malformed`] rather than an absence. The difference
/// matters exactly here: absence means "this project never recorded a head" and is safe, and a
/// broken document means somebody replaced the detector — reading the second as the first would let
/// an attacker disable the check by writing one byte of rubbish.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedHead {
    /// [`HEAD_VERSION`].
    pub head_version: u32,
    /// The journal's length in bytes when this head was written.
    pub journal_len: u64,
    /// Hex of the journal's 32-byte chain head at that length; `null` for a legacy journal.
    pub journal_head: Option<String>,
    /// `"chained"` or `"legacy"` — 42 §3.13 v0.4-r's framing, recorded beside the numbers so that a
    /// reader of this file alone can tell which of them the chain head belongs to.
    pub journal_format: String,
    /// The signed head. This is the field `gx checkpoint export` copies out of the box, and — with
    /// [`PersistedHead::witness_signature`] beside it — one of the **two** signatures a door now checks.
    pub checkpoint: Checkpoint,
    /// 🔴 **R7 / `req/232` M-02** — the digest of `.gx/VERSION` (`Cid` text) at the moment this head was
    /// written, or `None` for a head written by R6 or by a caller with no layout.
    #[serde(default)]
    pub version_digest: Option<String>,
    /// 🔴 **R7** — the last leaf hash at `checkpoint.tree_size` in `Cid` text form, or `None` for an empty
    /// tree or an R6 head. A number a third party can recompute from the ledger, kept beside the
    /// root so that the witness names the ledger's **end** and not only its shape.
    #[serde(default)]
    pub ledger_leaf_hash: Option<String>,
    /// 🔴 **R7 / DR-43-11 (b)** — the signature over [`PersistedHead::witness_payload`].
    ///
    /// The **payload is not stored**: it is rebuilt from the fields of this document
    /// ([`PersistedHead::stated`]), so a head whose numbers were edited fails the check by
    /// construction rather than by a comparison somebody has to remember to write. gx-log names
    /// gx-core and gx-canon and **not** gx-witness (the edge E-M2-1 made one-way), so the type here
    /// is gx-core's and the crate that signs and verifies is the one above.
    ///
    /// `None` is an R6-era head: it is read, and it is reported as `unwitnessed` rather than
    /// treated as checked.
    #[serde(default)]
    pub witness_signature: Option<DsseSignature>,
    /// 🔴 **R7 / `req/232` M-01** — the rollback an operator explicitly accepted, if one was.
    ///
    /// Written by `gx repair --accept-rollback --against <FILE>` and by nothing else. It is kept in
    /// the head rather than in a log beside it because the question it answers — *why is this
    /// project's floor lower than the one I remember?* — is asked of the head.
    #[serde(default)]
    pub accepted_rollback: Option<AcceptedRollback>,
}

impl PersistedHead {
    /// The numbers a door compares, taken off the document.
    ///
    /// # Errors
    /// [`Error::Malformed`] if `journal_head` is present and is not 32 bytes of hex — a value this
    /// process wrote and cannot read back is a corrupted detector, and the refusal says so.
    pub fn floor(&self) -> Result<HeadFloor> {
        let journal_head = match &self.journal_head {
            None => None,
            Some(text) => Some(from_hex(text).ok_or_else(|| Error::Malformed {
                detail: format!(
                    "the recorded head `{text}` is not 32 bytes of hexadecimal; \
                     `.gx/checkpoints/head.json` was written by something other than gx \
                     (DR-43-11)"
                ),
            })?),
        };
        Ok(HeadFloor {
            tree_size: self.checkpoint.tree_size,
            root_hash: self.checkpoint.root_hash,
            journal_len: self.journal_len,
            journal_head,
            version_digest: self.version_digest.clone(),
        })
    }

    /// 🔴 **R7 / DR-43-11 (b)** — the numbers this head states, in the shape they are signed in.
    ///
    /// Built from the document rather than stored twice, so that a head whose `witness` payload
    /// disagrees with the fields beside it is a **mismatch a caller can see** rather than two
    /// sources of truth. The caller that holds a key compares this with the payload it verified.
    /// 🔴 **R7 / DR-43-11 (b)** — the bytes [`PersistedHead::witness_signature`] is computed over.
    ///
    /// A **read** in FR-021's sense: it hashes nothing, writes nothing and touches no log — it
    /// renders this document's own numbers as the payload a DSSE envelope carries. Public because a
    /// verifier needs it: the crate that checks the signature is one crate up (gx-log does not name
    /// gx-witness), and asking it to rebuild the payload from a formula in a doc comment is how two
    /// implementations end up checking two different things — `gx_witness::dsse::
    /// checkpoint_signing_message`'s own reason, one document along.
    ///
    /// # Errors
    /// [`Error::Malformed`] if the numbers have no JSON form, which they always do; the result is
    /// fallible so that a caller cannot mistake an encoding failure for an unsigned head.
    pub fn witness_payload(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(&self.stated()).map_err(|e| Error::Malformed {
            detail: format!("the head's witness has no JSON form: {e}"),
        })
    }

    #[must_use]
    pub fn stated(&self) -> HeadWitness {
        HeadWitness {
            tree_size: self.checkpoint.tree_size,
            root_hash: self.checkpoint.root_hash.to_text(),
            journal_len: self.journal_len,
            journal_head: self.journal_head.clone(),
            journal_format: self.journal_format.clone(),
            version_digest: self.version_digest.clone(),
            ledger_leaf_hash: self.ledger_leaf_hash.clone(),
        }
    }
}

/// The file, and the namespace its checkpoints are signed under.
///
/// The origin travels with the store because 42 §3.11 makes it the thing "that stops a checkpoint
/// of one log from verifying against another's key", and a constant compiled in here would give an
/// operator running two logs one namespace.
#[derive(Clone, Debug)]
pub struct HeadStore {
    path: PathBuf,
    origin: String,
}

impl HeadStore {
    /// The store at an explicit path.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>, origin: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            origin: origin.into(),
        }
    }

    /// Where the file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The namespace [`PersistedHead::checkpoint`] is signed under.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Read the recorded head, or `None` if this project has never recorded one.
    ///
    /// # Errors
    /// [`Error::Io`] if the file exists and cannot be read. [`Error::Malformed`] if it will not
    /// parse or carries a version this binary does not read — both are refusals rather than
    /// absences, for the reason [`PersistedHead`]'s own note gives.
    pub fn read(&self) -> Result<Option<PersistedHead>> {
        let raw = match std::fs::read(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(Error::Io {
                    action: "cannot read the recorded head",
                    path: self.path.clone(),
                    kind: e.kind(),
                    detail: e.to_string(),
                })
            }
        };
        let head: PersistedHead = serde_json::from_slice(&raw).map_err(|e| Error::Malformed {
            detail: format!(
                "{} is not a head this binary can read ({e}). A head that will not parse is \
                     **not** treated as an absent one: absence means this project never recorded a \
                     head, and a broken document means the detector was replaced (DR-43-11)",
                self.path.display()
            ),
        })?;
        if head.head_version > HEAD_VERSION {
            return Err(Error::Malformed {
                detail: format!(
                    "{} records head version {} and this binary reads {HEAD_VERSION}: a newer \
                     project may state its past in a shape this binary would misread, so it \
                     refuses rather than comparing (DR-43-11, 47 §4's fail-closed)",
                    self.path.display(),
                    head.head_version
                ),
            });
        }
        Ok(Some(head))
    }

    /// Write the head, atomically.
    ///
    /// Temporary file in the same directory, `fsync`, `rename`, then `fsync` of the directory. The
    /// order is the one `EngineJournal::append` uses one crate over and it is here for the same
    /// reason: a detector that can be found half-written is a detector that fails open on exactly
    /// the crash it exists to survive. The rename is what makes the old head the fallback rather
    /// than a truncated file.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory cannot be made, or the file cannot be written, synced or
    /// renamed. [`Error::Malformed`] if the head has no JSON form.
    pub fn write(&self, head: &PersistedHead) -> Result<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| Error::Io {
            action: "cannot create the checkpoint directory",
            path: parent.to_path_buf(),
            kind: e.kind(),
            detail: e.to_string(),
        })?;
        let body = serde_json::to_vec_pretty(head).map_err(|e| Error::Malformed {
            detail: format!("the head has no JSON form: {e}"),
        })?;
        let tmp = self.path.with_extension("json.tmp");
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp).map_err(|e| Error::Io {
                action: "cannot create the head's temporary file",
                path: tmp.clone(),
                kind: e.kind(),
                detail: e.to_string(),
            })?;
            file.write_all(&body).map_err(|e| Error::Io {
                action: "cannot write the head",
                path: tmp.clone(),
                kind: e.kind(),
                detail: e.to_string(),
            })?;
            file.sync_all().map_err(|e| Error::Io {
                action: "cannot fsync the head",
                path: tmp.clone(),
                kind: e.kind(),
                detail: e.to_string(),
            })?;
        }
        std::fs::rename(&tmp, &self.path).map_err(|e| Error::Io {
            action: "cannot install the head",
            path: self.path.clone(),
            kind: e.kind(),
            detail: e.to_string(),
        })?;
        sync_dir(parent);
        Ok(())
    }
}

/// Compare the project in front of us with the head it has already published.
///
/// `now_root_at` is the caller's `TileLog::root_at(floor.tree_size)`: the root over the **first**
/// `floor.tree_size` leaves of the tree as it is now. Passing it in rather than taking a tree keeps
/// this function free of the log's shape and keeps the comparison in one place.
///
/// `None` means the project is at or ahead of its published head **and** the published head is a
/// prefix of it. Anything else is a rollback and says which number moved.
///
/// 🔴 **R7** — `now_version_digest` is the sixth argument and the last comparison: the declaration
/// beside the two files is part of what the head was written about (`req/232` M-02).
#[must_use]
pub fn compare(
    floor: &HeadFloor,
    now_tree_size: u64,
    now_root_at: Option<Cid>,
    now_journal_len: u64,
    now_journal_head_at: Option<Option<[u8; 32]>>,
    now_version_digest: Option<&str>,
) -> Option<RolledBack> {
    if now_tree_size < floor.tree_size {
        return Some(RolledBack::TreeShorter {
            was: floor.tree_size,
            now: now_tree_size,
        });
    }
    if floor.tree_size > 0 && now_root_at != Some(floor.root_hash) {
        return Some(RolledBack::TreeDiverged {
            was: floor.tree_size,
            expected: floor.root_hash,
            found: now_root_at,
        });
    }
    if now_journal_len < floor.journal_len {
        return Some(RolledBack::JournalShorter {
            was: floor.journal_len,
            now: now_journal_len,
        });
    }
    // `None` for "the caller did not walk the prefix" — the journal comparison is then the length
    // alone, and that is declared rather than silently skipped. `Some(x)` is a walk that happened
    // and `x` is what it found.
    if let (Some(found), Some(expected)) = (now_journal_head_at, floor.journal_head) {
        if found != Some(expected) {
            return Some(RolledBack::JournalDiverged {
                at: floor.journal_len,
            });
        }
    }
    // 🔴 **R7 / `req/232` M-02** — and the declaration those two files were declared under. A head
    // written before R7 records no digest and this comparison does not happen, which is the same
    // backward-compatible shape `journal_head` has for a legacy journal.
    if let Some(expected) = &floor.version_digest {
        if now_version_digest != Some(expected.as_str()) {
            return Some(RolledBack::VersionChanged {
                was: expected.clone(),
                now: now_version_digest.map(std::string::ToString::to_string),
            });
        }
    }
    None
}

/// 🔴 **R7 / `req/232` M-02** — the digest a project's `.gx/VERSION` is recorded under.
///
/// A **read** in FR-021's sense: a pure function of some bytes. It is here rather than in the
/// binary that owns the layout because 41 §6 puts every hash in gx-canon and gx-cli deliberately
/// does not name that crate (`gx_cli::keys`'s own note) — so the one place that already hashes on
/// gx-cli's behalf is the crate holding the document the digest goes into.
///
/// `Domain::Leaf` rather than a third domain byte: 42 §3.11 defines two, and inventing one for a
/// digest that never enters a Merkle tree would be a canonical-form change for no verifier's
/// benefit.
///
/// # 🔴 **R8 / `req/234` H-02** — the digest is over the **declaration**, not over the bytes
///
/// R7 hashed the file as it lay. `req/234` H-02 measured the three ways an ordinary editor breaks
/// that with no adversary and no damage: one extra trailing newline, a save that turned the line
/// endings into CRLF, and one trailing space. Each produced a project whose journal, ledger, chain
/// and head were provably intact and which **would not start** — and whose diagnosis said "its
/// ledger, its journal, or both are shorter" and "restore from a backup" while reporting
/// `journal_commits: 2` and `ledger_leaves: 2`. 43 §7.9 (a) puts mis-edits in Model A, where the
/// target is H = 0, so a detector that fires on whitespace is the finding rather than the defence.
///
/// The fix is to hash what the file **says**: [`normalise_declaration`] below. The value being
/// defended is unchanged — `journal_format=chained` rewritten to `legacy` still changes the digest,
/// which is `req/232` M-02's true positive and the reason this digest exists.
///
/// 🔴 **No compatibility break, and it is structural rather than lucky.** `Layout::declare_journal_format`
/// writes `lines.join("\n") + "\n"` — LF, no blank lines, no surrounding space — so the normal form
/// below is byte-identical to what gx itself writes. A head recorded by an R7 binary beside a
/// gx-written `.gx/VERSION` therefore digests to the same value under this function as it did under
/// the old one. What changes is only which *other* byte strings now agree with it.
#[must_use]
pub fn declaration_digest(bytes: &[u8]) -> String {
    let meaning = normalise_declaration(bytes);
    gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[meaning.as_slice()]).to_text()
}

/// 🔴 **R8 / `req/234` H-02** — `.gx/VERSION` reduced to what it declares.
///
/// The normal form is the one [`crate`]'s writer already produces: the layout version on the first
/// line, then one `key=value` per line, LF endings, one trailing newline, nothing else. Getting
/// there is four rules and each of them is a byte difference `req/234` measured or one that follows
/// from the same cause:
///
/// * **line endings** — `\r\n` and a bare `\r` are the same line break as `\n` (a Windows or
///   OneDrive round trip; the environment this project's own denominator has never measured);
/// * **surrounding space** — a line is trimmed, and a `key=value` is trimmed on both halves, so
///   `journal_format=chained ` and `journal_format = chained` declare what `journal_format=chained`
///   declares;
/// * **blank lines** — a line with nothing on it declares nothing, so it is dropped;
/// * **a byte-order mark** — an editor's, not an operator's.
///
/// 🔴 What is deliberately **not** normalised: the **order** of the lines and duplicate keys. Both
/// would be this function deciding what a declaration means rather than reading it, and
/// `Layout::declared_journal_format` resolves a duplicate by taking the first — so a normal form
/// that reordered or de-duplicated could make two files agree here and disagree there, which is a
/// worse bug than the one being fixed. A reordered file is still refused, and that is a true
/// statement about a file no tool in this project produces.
///
/// Invalid UTF-8 is passed through **unchanged** rather than lossily decoded: a file that is not
/// text is not a declaration this function can read, and guessing at it would be the fail-open half
/// of the same mistake.
/// 🔴 **R9 / `req/236` H-04** — `.gx/VERSION`'s bytes as text, whatever an editor saved them as.
///
/// R8 read UTF-8 and passed anything else through untouched, which was the honest half of a
/// decision: "a file that is not text is not a declaration this function can read". `req/236` H-04
/// measured the other half — **Notepad's "Save as UTF-16 LE"** is the single most likely way a file
/// in this project acquires bytes that are not UTF-8, and 43 §7.9 (h) (C1) names Windows and
/// OneDrive as the surface with the least measurement behind it. So the two byte-order marks that
/// say what the encoding is are honoured, and only bytes that announce nothing and do not decode as
/// UTF-8 are refused.
///
/// `None` is "this is not text", and every caller turns it into a **classified** refusal with a
/// remedy rather than into "`\"\\u{feff}1\"` is not a number".
fn decode_declaration(bytes: &[u8]) -> Option<String> {
    fn utf16(units: &[u8], little: bool) -> Option<String> {
        if !units.len().is_multiple_of(2) {
            return None;
        }
        let points: Vec<u16> = units
            .chunks_exact(2)
            .map(|p| {
                if little {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                }
            })
            .collect();
        String::from_utf16(&points).ok()
    }
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, true),
        [0xfe, 0xff, rest @ ..] => utf16(rest, false),
        _ => core::str::from_utf8(bytes)
            .ok()
            .map(|text| text.strip_prefix('\u{feff}').unwrap_or(text).to_string()),
    }
}

/// 🔴 **R9 / `req/236` H-04** — `.gx/VERSION` as the two kinds of line it holds.
///
/// The bare lines first (the layout version is the first of them) and then the `key=value` pairs,
/// both already decoded, trimmed and de-blanked. A named type rather than a tuple because it is the
/// **shared** return of the parse and of the digest, and the whole of H-04 is that those two used to
/// read the same bytes differently.
pub type Declaration = (Vec<String>, Vec<(String, String)>);

/// 🔴 **R9 / `req/236` H-04** — the declaration split into the lines that are `key=value` and the
/// lines that are not, both already trimmed and de-blanked.
///
/// One reader for both halves of the door: the **digest** ([`normalise_declaration`]) and the
/// **parse** (`gx_cli::layout`) now derive from this one function, which is what stops "what the
/// file declares" and "what the file parses as" from being two answers. `req/236` H-04 is the
/// measurement of them being two: R8's `normalise_declaration` stripped a byte-order mark and said
/// so in its own doc comment, and the parse in front of it read the raw first line and refused the
/// project with `VALIDATION_ERROR` before the digest was ever taken.
///
/// `None` for bytes `decode_declaration` will not read.
#[must_use]
pub fn declaration_lines(bytes: &[u8]) -> Option<Declaration> {
    let text = decode_declaration(bytes)?;
    let mut bare = Vec::new();
    let mut pairs = Vec::new();
    for raw in text.split(['\n', '\r']) {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match line.split_once('=') {
            Some((key, value)) => pairs.push((key.trim().to_string(), value.trim().to_string())),
            None => bare.push(line.to_string()),
        }
    }
    Some((bare, pairs))
}

#[must_use]
pub fn normalise_declaration(bytes: &[u8]) -> Vec<u8> {
    let Some((bare, mut pairs)) = declaration_lines(bytes) else {
        // Bytes that are not text are passed through unchanged, exactly as R8 left them: guessing
        // at them would be the fail-open half of the same mistake. What changed in R9 is that
        // fewer byte strings reach this arm (UTF-16 is decoded above) and that the caller which
        // *parses* the file no longer dies on the ones that do.
        return bytes.to_vec();
    };
    // 🔴 **R9 / `req/236` H-04** — the **order of the lines** is normalised, and duplicate keys are
    // still not.
    //
    // R8 declined to normalise either, and wrote the reason down: `Layout::declared_journal_format`
    // resolves a duplicate by taking the first, so a normal form that de-duplicated could make two
    // files agree here and disagree there. That reasoning is about **duplicates** and it still
    // holds — the sort below is `sort_by_key`, which Rust guarantees is stable, so two lines with
    // the same key keep the order the file gave them and "the first one wins" means the same thing
    // on both sides.
    //
    // The reasoning never applied to lines with *different* keys, and `req/236` H-04 measured what
    // leaving them alone cost: a `.gx/VERSION` whose two lines were swapped stopped `gx repair`,
    // `gx log proof`, `gx replay` and `gx serve` alike. Two lines in a different order declare the
    // same thing, and this is the function whose job is to say what a file declares.
    //
    // Bare lines (the layout version) come first and keep their order, which makes the normal form
    // byte-identical to what `Layout::declare_journal_format` writes — so no head signed by R7 or
    // R8 beside a gx-written file changes value.
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for line in bare {
        out.push_str(&line);
        out.push('\n');
    }
    for (key, value) in pairs {
        out.push_str(&key);
        out.push('=');
        out.push_str(&value);
        out.push('\n');
    }
    out.into_bytes()
}

/// Lowercase hex, for [`PersistedHead::journal_head`].
#[must_use]
pub fn to_hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// The inverse of [`to_hex`], strict about length.
#[must_use]
pub fn from_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Best-effort directory fsync — the same shape `gx_engine::store` uses, and off unix a no-op.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let bytes = [7u8; 32];
        assert_eq!(from_hex(&to_hex(&bytes)), Some(bytes));
        assert_eq!(from_hex("00"), None);
    }

    #[test]
    fn a_project_at_its_published_head_is_not_rolled_back() {
        let root = gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[b"r"]);
        let floor = HeadFloor {
            tree_size: 3,
            root_hash: root,
            journal_len: 100,
            journal_head: None,
            version_digest: None,
        };
        assert_eq!(compare(&floor, 3, Some(root), 100, None, None), None);
    }

    #[test]
    fn a_shorter_tree_is_named_as_one() {
        let root = gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[b"r"]);
        let floor = HeadFloor {
            tree_size: 3,
            root_hash: root,
            journal_len: 100,
            journal_head: None,
            version_digest: None,
        };
        assert_eq!(
            compare(&floor, 2, None, 100, None, None),
            Some(RolledBack::TreeShorter { was: 3, now: 2 })
        );
    }
}
