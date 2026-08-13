//! Durability: the file the log lives in, the fsync that makes an answer true, and the recovery
//! that reads it back.
//!
//! Spec: 33 NFR-009 for the barrier (「`ledger.append`は成功応答を返す前に基盤ストレージへfsyncを
//! 完了する（fsync-on-append）」), 34 AC-069 for how that is judged, 43 ASM-43-1 for the key the
//! append is idempotent under, 42 §3.11 for the record's field table. 41 §2 names this module.
//!
//! # The four declarations 05 §4 asks of a storage layer
//!
//! **Where ACID sits.** Atomicity and durability are per `append` and nothing wider: one call
//! writes one length-framed record and returns only after `fsync`, so a caller that was answered
//! holds a promise about that entry alone. There is **no transaction** spanning two appends, no
//! rollback, and no isolation mechanism — a `LedgerStore` is owned by one writer (`&mut self`) and
//! two processes opening the same file would both believe they own it. Consistency is the tree's
//! invariant, not the file's: a record is replayed only if the leaf hash recomputed from its
//! fields is the `leaf_cid` it carries. A crash between the `write` and the `fsync` may leave a
//! record that a later open accepts; NFR-009 requires that a **successful** append be present, and
//! says nothing about one that never answered. That direction is the safe one and it is the only
//! one this file promises.
//!
//! **Which indexes exist.** One, in memory, built at open and maintained on append:
//! `TransformationId -> leaf index`, which is what makes ASM-43-1's idempotence decidable without
//! a scan. It is **not** on disk. There is no secondary index of any kind — no lookup by receipt
//! digest, no time range, no origin — because nothing in 32 FR-021..024 or 44 asks for one and an
//! index that is written is an index that can disagree with what it indexes.
//!
//! **What is source and what is derived.** The file is the source. The tree, the leaf hashes, the
//! roots, every proof, and the key index above are **derived**: all of them are rebuilt from the
//! file at open, and none of them is written to it. That is why recovery is a replay rather than a
//! load, and why a damaged record is refused rather than repaired — there is nothing to repair
//! *from*, and a log whose derived state disagreed with its source would answer two ways.
//!
//! # The framing, and why there is no checksum
//!
//! ```text
//! record := length : u32 big-endian || payload
//! payload := canonical DAG-CBOR of a LedgerEntry (42 §3.11)
//! ```
//!
//! 批8 (req/11) records three implementations converging on 「length-prefix + checksum を entry
//! 先頭・torn-write は不一致 tail を truncate して復旧」. gx takes the framing and the recovery and
//! **not** the checksum, on the ground that the record already carries one: `LedgerEntry.leaf_cid`
//! is `BLAKE3(0x00 || canonical_dagcbor(leaf))` over three of the five fields, and replay
//! recomputes it. Adding a CRC would mean a second integrity algorithm in a workspace whose whole
//! digest story is one function (35 DR-3, `gx_canon::cid`), and a dependency for it — the same
//! argument that removed `rs_merkle` in hand 2 (M2H1-6), applied to the same kind of second answer.
//! The alternative of a BLAKE3 digest over the whole record was weighed and dropped too: it would
//! need a third domain byte, which 42 §3.11 does not define, so it would buy coverage of
//! `appended_at` at the price of a spelling the canon does not have. What that costs is written
//! down rather than hidden: **`appended_at` is inside no digest**, so a bit flipped in a
//! timestamp is not detected. It is metadata (ASM-4), outside the leaf by construction, and no
//! root, proof or identity moves when it changes.
//!
//! # Why there is no typestate
//!
//! 批8 also observes 「typestate で『fsync 前に成功を返せない』を型で強制」 (the squirrelfs shape).
//! It is **not** adopted, and the reason is that the state it would name has no public inhabitant:
//! the window between the `write` and the `fsync` opens and closes inside one non-async function
//! body, so an `Unflushed`/`Flushed` marker would be a type parameter that is `Flushed` at every
//! call site in the crate and at every call site any caller could write. A phantom that is never
//! observed in two states costs readability (規律 ①) and buys a check the borrow checker is
//! already making. What the ordering *does* get is a weaker, cheaper form of the same idea:
//! [`crate::tile::TileLog`] splits its append into a crate-private `stage` (pure, returns the
//! entry) and `commit` (pushes it), so 「durable before visible」 is expressible as two statements
//! in one function and `ac_069.rs` reads their order off the source. If group commit ever lands —
//! 批8's 「応答を fsync 完了まで遅延」 — the state gains a second inhabitant and the typestate
//! becomes worth its cost; that is the condition to revisit this on, and it is not met today.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use gx_canon::cbor;
use gx_core::{Cid, Timestamp, TransformationId, VerdictCheckpoint};

use crate::tile::{LedgerEntry, TileLog};
use crate::{Error, Result};

/// Bytes of the length header in front of every record.
const LENGTH_BYTES: usize = 4;

/// The largest record this reader will allocate for.
///
/// A `LedgerEntry` is five small fields and encodes to under two hundred bytes, so this is three
/// orders of magnitude of headroom rather than a tuning parameter. It exists because the length
/// header is read from a file that may have been damaged: without a ceiling, four corrupted bytes
/// ask for a four-gigabyte allocation before anything has had a chance to refuse them. The same
/// shape as H2-5's path-length gate — decide from the declared size, before doing the work it asks
/// for.
pub const MAX_RECORD_BYTES: u32 = 1 << 20;

/// What an append did (43 ASM-43-1).
///
/// The two are different answers and a caller that cannot tell them apart cannot implement
/// exactly-once: 「the entry is in the log」 and 「the entry went in just now」 differ exactly when a
/// retry after a crash is being handled, which is the case the assumption exists for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    /// The record was written and synced by this call.
    Appended(LedgerEntry),
    /// The same transformation was already in the log with the same receipt digest, so nothing
    /// was written. The entry is the one the log holds, clock included — not the one offered.
    AlreadyPresent(LedgerEntry),
}

impl AppendOutcome {
    /// The entry the log holds, either way.
    #[must_use]
    pub fn entry(&self) -> &LedgerEntry {
        match self {
            AppendOutcome::Appended(entry) | AppendOutcome::AlreadyPresent(entry) => entry,
        }
    }
}

/// What opening the file found (AC-069).
///
/// Reported rather than logged. A store that silently dropped a torn tail would make 「the log has
/// n entries」 the only observable fact, and the difference between 「it was closed cleanly」 and
/// 「the last write was cut in half」 is precisely what a reader of an audit log needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recovery {
    /// Records replayed into the tree.
    pub records: u64,
    /// Bytes after the last replayable record, removed from the file at open. Zero for a file
    /// that was closed cleanly.
    pub torn_tail_bytes: u64,
}

/// An append-only ledger on a file (NFR-009, AC-069).
///
/// Holds the file, the tree replayed from it, and the key index of ASM-43-1. Every read of the
/// tree goes through [`LedgerStore::log`], so there is one road to a root and one road to a proof
/// whether the log came from a file or from memory.
#[derive(Debug)]
pub struct LedgerStore {
    file: File,
    path: PathBuf,
    tree: TileLog,
    by_key: HashMap<TransformationId, u64>,
    recovery: Recovery,
}

impl LedgerStore {
    /// Open the ledger at `path`, creating it if it is not there, and replay what it holds.
    ///
    /// A tail that cannot be replayed is removed from the file before this returns, so the next
    /// append lands at the index the tree actually reached. See [`Recovery`] for what is reported
    /// about that, and the module docs for why the refusal stops at the first bad record instead
    /// of skipping it.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be opened, read, truncated or synced. [`Error::Canon`] if
    /// a replayed entry has no canonical form, which would mean the encoder disagrees with the
    /// encoder that wrote it.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let existed = path.exists();
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| io_error("cannot open the ledger", &path, &e))?;

        let replayed = replay(&file, &path)?;
        if replayed.torn_tail_bytes > 0 {
            file.set_len(replayed.good_bytes)
                .map_err(|e| io_error("cannot remove the torn tail of the ledger", &path, &e))?;
            barrier(&file, &path, "cannot fsync the truncated ledger")?;
        }
        if !existed {
            // The file's own directory entry has to reach the device too, or a crash can take the
            // whole ledger while every record inside it was synced. Unix only: opening a directory
            // as a `File` is how fsync reaches a directory there, and Windows refuses it. The gap
            // on other platforms is recorded in req/52 §5 rather than papered over.
            sync_parent_directory(&path)?;
        }

        let recovery = Recovery {
            records: replayed.tree.len(),
            torn_tail_bytes: replayed.torn_tail_bytes,
        };
        Ok(Self {
            file,
            path,
            tree: replayed.tree,
            by_key: replayed.by_key,
            recovery,
        })
    }

    /// Append one record, durably (FR-021, NFR-009), and idempotently (ASM-43-1).
    ///
    /// The order is the requirement. The entry is staged without touching the tree, encoded,
    /// written, and synced; only then is it committed, so nothing a reader can see exists before
    /// the bytes behind it are on the device. `ac_069.rs` reads that order off this function.
    ///
    /// Keyed on `transformation` (ASM-43-1). A repeat carrying the same `receipt_digest` is a
    /// no-op and answers with the entry already in the log; one carrying a different digest is
    /// refused, because two receipts for one transformation is the state INV-S3 exists to
    /// prevent, and picking either of them would be this crate deciding which is true.
    /// `receipt_digest` and not `leaf_cid` is what 「同 CID」 means here: a leaf hash covers the
    /// index, so a second offer of the same transformation would carry a different one by
    /// construction and every repeat would read as a conflict.
    ///
    /// # Errors
    /// [`Error::Conflict`] for a second, different answer under one key. [`Error::Canon`] if the
    /// leaf has no canonical form. [`Error::Io`] if the record cannot be written or synced.
    /// [`Error::Malformed`] if the encoded record is larger than [`MAX_RECORD_BYTES`].
    pub fn append(
        &mut self,
        transformation: TransformationId,
        receipt_digest: Cid,
        appended_at: Timestamp,
    ) -> Result<AppendOutcome> {
        if let Some(&index) = self.by_key.get(&transformation) {
            let recorded = self
                .tree
                .entry(index)
                .expect("the key index names a leaf the tree holds")
                .clone();
            if recorded.receipt_digest == receipt_digest {
                return Ok(AppendOutcome::AlreadyPresent(recorded));
            }
            return Err(Error::Conflict {
                transformation,
                recorded: recorded.receipt_digest,
                offered: receipt_digest,
            });
        }

        let staged = self
            .tree
            .stage(transformation, receipt_digest, appended_at)?;
        let payload = cbor::encode(&staged)?;
        self.write_and_sync(&payload)?;
        self.tree.commit(staged.clone());
        self.by_key.insert(transformation, staged.index);
        Ok(AppendOutcome::Appended(staged))
    }

    /// The tree this ledger holds, for roots, tiles and proofs.
    #[must_use]
    pub fn log(&self) -> &TileLog {
        &self.tree
    }

    /// The file this ledger is stored in.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What opening the file found.
    #[must_use]
    pub fn recovery(&self) -> Recovery {
        self.recovery
    }

    /// Frame the payload, put it on the file, and wait for the device.
    ///
    /// One `write_all` for header and payload together, so a record is one syscall and the window
    /// a crash can land inside is as small as the platform allows. It is not zero — a partially
    /// written record is exactly what [`replay`] refuses — and pretending otherwise is what a
    /// checksum-free framing would be if the recovery did not exist.
    fn write_and_sync(&mut self, payload: &[u8]) -> Result<()> {
        frame_write_and_sync(
            &self.file,
            &self.path,
            payload,
            FrameLabels {
                noun: "a record",
                write: "cannot write to the ledger",
                sync: "cannot fsync the ledger",
            },
        )
    }
}

/// What a framing failure is called, per file (**FR-M04**, M7 hand 6).
///
/// Three `&'static str`s rather than one, because [`io_error`] takes the whole action as a static
/// and a reader of an I/O error is entitled to know *which* file refused. They are labels: nothing
/// below branches on them.
#[derive(Clone, Copy)]
struct FrameLabels {
    noun: &'static str,
    write: &'static str,
    sync: &'static str,
}

/// Frame the payload, put it on the file, and wait for the device.
///
/// One `write_all` for header and payload together, so a record is one syscall and the window a
/// crash can land inside is as small as the platform allows. It is not zero — a partially written
/// record is exactly what [`replay`] refuses — and pretending otherwise is what a checksum-free
/// framing would be if the recovery did not exist.
///
/// # 🔴 Why this is a free function and not two methods (M7 hand 6)
///
/// `tests/ac_069.rs::ac_069_the_durability_barrier_has_exactly_one_call_site` asserts that
/// `store.rs` holds **one** `.write_all(` and **one** `.sync_all()`: 「one road to the file, so that
/// no write can bypass the framing or the barrier」. FR-M04 adds a second append-only file to this
/// module, and the honest way to add it is to send it down the road that already exists rather than
/// to widen the gate to two — **the gate caught this hand's first shape and this is the answer to
/// it**, which is the order that says the gate works (the same order `ac_021` produced in M7 hand 5).
///
/// gotcha55 applies and is answered: a pair pulled out into a function needs the **callers**
/// measured, and both are — `tests/ac_069.rs` for the ledger's, and
/// `tests/verdict_checkpoint_store.rs` for the verdict log's, whose torn-tail and continuation
/// probes only hold if the record that reached the file was framed and synced.
fn frame_write_and_sync(
    file: &File,
    path: &Path,
    payload: &[u8],
    labels: FrameLabels,
) -> Result<()> {
    let length = u32::try_from(payload.len())
        .ok()
        .filter(|n| *n <= MAX_RECORD_BYTES)
        .ok_or_else(|| Error::Malformed {
            detail: format!(
                "{} of {} bytes is over the {MAX_RECORD_BYTES}-byte ceiling",
                labels.noun,
                payload.len()
            ),
        })?;

    let mut record = Vec::with_capacity(LENGTH_BYTES + payload.len());
    record.extend_from_slice(&length.to_be_bytes());
    record.extend_from_slice(payload);

    let mut sink: &File = file;
    sink.write_all(&record)
        .map_err(|e| io_error(labels.write, path, &e))?;
    barrier(file, path, labels.sync)
}

// ---------------------------------------------------------------------------
// FR-M04 -- the parallel log of aggregate verdict checkpoints (M7 hand 6, 案 A)
// ---------------------------------------------------------------------------

/// The append-only file a deployment's signed verdict counts live in (**FR-M04**).
///
/// # Why this is a second file rather than a second kind of record
///
/// FR-M04 asks for 「件数集計値のcommitmentを**ledgerへ追記**」 and `FRM04_DESIGN_MATERIAL` §3's 案 A
/// leaves the writer's shape open between a separate index and a second record type in the
/// ledger's own file. The second is not available here: [`replay`] decodes **every** record as a
/// [`LedgerEntry`], so a foreign record in that file is a file the current reader stops at. Making
/// the reader tolerant means `LedgerEntry` becomes a sum type and `LedgerStore::append` changes
/// shape — which is 案 B, and `req/98` §3-3 chose 案 A precisely so that adapter work and
/// canonical-structure work do not move in one milestone.
///
/// So: the same framing, the same recovery, the same barrier, a different file. What is **not**
/// shared is the Merkle tree, and that is the cost, stated rather than buried: a verdict
/// checkpoint carries no inclusion proof and is covered by no consistency proof. It is a signed
/// statement whose only defences are its signature, the contiguity of its window and the ledger
/// size it names (`gx_core::VerdictCheckpoint`, and `proof::audit_verdict_chain` for what those
/// three catch).
///
/// # No key index, unlike [`LedgerStore`]
///
/// 43 ASM-43-1's idempotence is about a transformation being committed twice. A checkpoint is a
/// statement about a *window*, and two checkpoints over the same window are not a retry — they are
/// a contradiction, which is `audit_verdict_chain`'s to report rather than this file's to
/// silently collapse.
#[derive(Debug)]
pub struct VerdictCheckpointStore {
    file: File,
    path: PathBuf,
    checkpoints: Vec<VerdictCheckpoint>,
    recovery: Recovery,
}

impl VerdictCheckpointStore {
    /// Open the file at `path`, creating it if it is not there, and replay what it holds.
    ///
    /// A tail that cannot be replayed is removed from the file before this returns and reported
    /// through [`VerdictCheckpointStore::recovery`], for [`LedgerStore::open`]'s reason: the next
    /// append has to land where the sequence actually reached, and a reader has to be able to tell
    /// 「the chain ends here」 from 「the last write was cut in half」.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be opened, read, truncated or synced.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let existed = path.exists();
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| io_error("cannot open the verdict checkpoint log", &path, &e))?;

        let (checkpoints, good, total) = replay_verdict_checkpoints(&file, &path)?;
        let torn = total - good;
        if torn > 0 {
            file.set_len(good).map_err(|e| {
                io_error("cannot remove the torn tail of the verdict log", &path, &e)
            })?;
            barrier(&file, &path, "cannot fsync the truncated verdict log")?;
        }
        if !existed {
            sync_parent_directory(&path)?;
        }
        Ok(Self {
            file,
            path,
            recovery: Recovery {
                records: checkpoints.len() as u64,
                torn_tail_bytes: torn,
            },
            checkpoints,
        })
    }

    /// Append one checkpoint, durably, and answer with its position.
    ///
    /// Encode, write, wait for the device, **then** make it visible — the same order and the same
    /// reason as [`LedgerStore::append`], through the same [`barrier`].
    ///
    /// # Errors
    /// [`Error::Canon`] if the checkpoint has no canonical form, [`Error::Malformed`] if the
    /// encoded record is over [`MAX_RECORD_BYTES`], [`Error::Io`] if it cannot be written or
    /// synced.
    pub fn append(&mut self, checkpoint: VerdictCheckpoint) -> Result<u64> {
        let payload = cbor::encode(&checkpoint)?;
        frame_write_and_sync(
            &self.file,
            &self.path,
            &payload,
            FrameLabels {
                noun: "a verdict checkpoint",
                write: "cannot write to the verdict checkpoint log",
                sync: "cannot fsync the verdict checkpoint log",
            },
        )?;
        self.checkpoints.push(checkpoint);
        Ok(self.checkpoints.len() as u64 - 1)
    }

    /// The chain this file holds, oldest first.
    #[must_use]
    pub fn checkpoints(&self) -> &[VerdictCheckpoint] {
        &self.checkpoints
    }

    /// The file the chain is stored in.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What opening the file found.
    #[must_use]
    pub fn recovery(&self) -> Recovery {
        self.recovery
    }
}

/// Read the verdict checkpoint file back, stopping at the first record that does not hold up.
///
/// Three things have to be true of a record: its header is complete, its payload is as long as the
/// header promised, and it decodes as canonical DAG-CBOR (strict, CM-6). There is no fourth check
/// here where [`replay`] has two — an index and a recomputed leaf hash — and the absence is worth a
/// sentence rather than a shrug: a checkpoint carries no position of its own and is covered by no
/// Merkle hash, so bytes altered inside a record after it was written are caught by its
/// **signature** and by nothing in this file. Answers: the chain, the bytes that replayed, the
/// bytes the file holds.
fn replay_verdict_checkpoints(
    file: &File,
    path: &Path,
) -> Result<(Vec<VerdictCheckpoint>, u64, u64)> {
    let total = file
        .metadata()
        .map_err(|e| io_error("cannot read the verdict log's size", path, &e))?
        .len();
    let mut reader = std::io::BufReader::new(file);
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| io_error("cannot rewind the verdict log", path, &e))?;

    let mut out: Vec<VerdictCheckpoint> = Vec::new();
    let mut good: u64 = 0;
    loop {
        let mut header = [0u8; LENGTH_BYTES];
        let read = fill(&mut reader, &mut header)
            .map_err(|e| io_error("cannot read the verdict log", path, &e))?;
        if read < LENGTH_BYTES {
            break;
        }
        let length = u32::from_be_bytes(header);
        if length == 0 || length > MAX_RECORD_BYTES {
            break;
        }
        let mut payload = vec![0u8; length as usize];
        let read = fill(&mut reader, &mut payload)
            .map_err(|e| io_error("cannot read the verdict log", path, &e))?;
        if read < payload.len() {
            break;
        }
        let Ok(checkpoint) = cbor::decode::<VerdictCheckpoint>(&payload) else {
            break;
        };
        out.push(checkpoint);
        good += (LENGTH_BYTES + payload.len()) as u64;
    }
    Ok((out, good, total))
}

/// The durability barrier NFR-009 names (「基盤ストレージへfsyncを完了する」).
///
/// `sync_all` and not `sync_data`: the two differ on whether the file's metadata is pushed as
/// well, and a record appended past the old end of file is only reachable if the new length
/// reached the device with it. This is the one place in the crate where a write is waited on, and
/// `ac_069.rs` asserts that it is one place — a second barrier elsewhere would mean a second
/// answer to 「what makes an append durable」.
fn barrier(file: &File, path: &Path, action: &'static str) -> Result<()> {
    let synced = file.sync_all();
    #[cfg(test)]
    {
        if synced.is_ok() {
            fsync_meter::note_barrier();
        }
    }
    synced.map_err(|e| io_error(action, path, &e))
}

/// 🔴 **M5H8-10 採(a)** (`req/38_ERRATA_2026-08-07.md` §45) — the observation point, and no more.
///
/// The ruling, verbatim:
///
/// > **M5H8-10 採(a)**: gx-log `sync_parent_directory` の呼び出し計器(手 4 (m) が engine 側で取った
/// > 「呼んだ事を数える」形)=**fix 批**。NFR-009 の「fsync を呼ぶ事自体が誰にも観測されていない」は
/// > 「電源断を測らない」(req/52 §5)より安く塞げる別の欠落。
///
/// # What req/86 §3.2 measured, and what it means
///
/// `cargo-mutants` reduced [`sync_parent_directory`] to `Ok(())` and **all 247 of gx-log's own
/// probes stayed green**. This module's header, `ac_069.rs`'s static half and 33 NFR-009 all say
/// the barrier is the point of the crate, and nothing in the crate could tell a build that crosses
/// it from one that does not. Line coverage for this file is 88.59% — the lines run, and nobody
/// checks what they did.
///
/// # Why a counter, and why it is `#[cfg(test)]`
///
/// A fault is what M5 hand 4 already built one crate up: `gx-engine/tests/ledger_durability.rs`
/// makes the directory unreadable and the open fails. That probe is real and it kills this
/// survivor — under `--test-workspace`. `cargo-mutants` runs per package by default (req/86 §3.0),
/// so gx-log's own suite has to be able to answer the question on its own. Counting is the
/// cheapest thing that can: it says 「呼んだ」 and claims nothing about the platter, so req/52 §5's
/// declared limit is exactly where it was.
///
/// `#[cfg(test)]` because a counter that shipped would be a durability claim's worth of state in a
/// hot path, and a `pub` one would be a second, weaker answer to 「what makes an append durable」.
/// The cost is that the probes live in this file (`mod tests` at the bottom) rather than under
/// `tests/` — the same trade `gx-witness/src/dsse.rs` takes for `raw_bytes`.
///
/// Thread-local, not atomic: `cargo test` runs probes in parallel in one process, and a global
/// counter would make two probes' appends each other's noise. Every `LedgerStore` below is created
/// and used on the probe's own thread.
#[cfg(test)]
pub(crate) mod fsync_meter {
    use core::cell::Cell;

    thread_local! {
        /// Every durability barrier this thread completed.
        static BARRIERS: Cell<u64> = const { Cell::new(0) };
        /// The subset of those that pushed a *directory* entry to the device.
        static DIRECTORIES: Cell<u64> = const { Cell::new(0) };
    }

    pub(crate) fn note_barrier() {
        BARRIERS.with(|c| c.set(c.get() + 1));
    }

    pub(crate) fn note_directory() {
        DIRECTORIES.with(|c| c.set(c.get() + 1));
    }

    /// `(barriers, directory barriers)` completed on this thread so far.
    pub(crate) fn read() -> (u64, u64) {
        (BARRIERS.with(Cell::get), DIRECTORIES.with(Cell::get))
    }
}

/// Push the directory entry of a newly created ledger to the device.
///
/// `fsync` on a directory is what makes a file's *name* durable, and on Unix the way to reach it
/// is to open the directory and sync the handle.
#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let dir = File::open(parent)
        .map_err(|e| io_error("cannot open the ledger's directory", parent, &e))?;
    barrier(&dir, parent, "cannot fsync the ledger's directory")?;
    // M5H8-10's observation point. See [`fsync_meter`] for why it is a count and why it is
    // `#[cfg(test)]`. Under `cfg(not(test))` this function is the three lines above and `Ok(())`.
    #[cfg(test)]
    fsync_meter::note_directory();
    Ok(())
}

/// Elsewhere a directory has no handle to sync, so a newly created ledger's *name* is as durable
/// as the platform makes it and no more. A real gap, declared rather than silently skipped: v0.1
/// CI is x86_64 Linux (A-5), and req/52 §5 records that no other platform was measured.
#[cfg(not(unix))]
fn sync_parent_directory(path: &Path) -> Result<()> {
    let _ = path;
    Ok(())
}

/// What a replay produced.
struct Replayed {
    tree: TileLog,
    by_key: HashMap<TransformationId, u64>,
    /// Bytes of the prefix that replayed.
    good_bytes: u64,
    /// Bytes after it.
    torn_tail_bytes: u64,
}

/// Read the file back into a tree, stopping at the first record that does not hold up.
///
/// Five things have to be true of a record for it to be replayed: its header is complete, its
/// payload is as long as the header promised, it decodes as canonical DAG-CBOR (strict by default,
/// CM-6), its `index` is the next one the tree expects, and the leaf hash recomputed from its
/// fields is the `leaf_cid` it carries. The last is the one that catches a record whose bytes were
/// changed after they were written, and it is the reason the framing carries no checksum of its
/// own (see the module docs).
///
/// A free function rather than a method because it borrows the file while building a tree; as a
/// method the two would be one `&mut self`.
fn replay(file: &File, path: &Path) -> Result<Replayed> {
    let total = file
        .metadata()
        .map_err(|e| io_error("cannot read the ledger's size", path, &e))?
        .len();

    let mut reader = std::io::BufReader::new(file);
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| io_error("cannot rewind the ledger", path, &e))?;

    let mut tree = TileLog::new();
    let mut by_key: HashMap<TransformationId, u64> = HashMap::new();
    let mut good: u64 = 0;

    loop {
        let mut header = [0u8; LENGTH_BYTES];
        let read = fill(&mut reader, &mut header)
            .map_err(|e| io_error("cannot read the ledger", path, &e))?;
        if read < LENGTH_BYTES {
            break;
        }
        let length = u32::from_be_bytes(header);
        if length == 0 || length > MAX_RECORD_BYTES {
            break;
        }

        let mut payload = vec![0u8; length as usize];
        let read = fill(&mut reader, &mut payload)
            .map_err(|e| io_error("cannot read the ledger", path, &e))?;
        if read < payload.len() {
            break;
        }

        let Ok(recorded) = cbor::decode::<LedgerEntry>(&payload) else {
            break;
        };
        if recorded.index != tree.len() {
            break;
        }
        let staged = tree.stage(
            recorded.transformation,
            recorded.receipt_digest,
            recorded.appended_at,
        )?;
        if staged != recorded {
            break;
        }
        if by_key
            .insert(recorded.transformation, recorded.index)
            .is_some()
        {
            // Two records under one key: the file contradicts ASM-43-1, which the writer above
            // cannot produce. Whatever wrote it was not this code, so the prefix is kept and the
            // rest is not trusted.
            break;
        }
        tree.commit(staged);
        good += (LENGTH_BYTES + payload.len()) as u64;
    }

    Ok(Replayed {
        tree,
        by_key,
        good_bytes: good,
        torn_tail_bytes: total - good,
    })
}

/// Read until `buf` is full or the file ends, and report how much arrived.
///
/// `Read::read_exact` would answer 「unexpected end of file」, which is the same value for 「the log
/// ends here」 and 「something is wrong」. A short read at the tail is the ordinary shape of a crash,
/// so the count is what is wanted and the error is kept for real I/O failures.
fn fill(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Carry an [`std::io::Error`] into this crate's error type without losing what it said.
///
/// The kind is kept as a value and the message as a string because [`crate::Error`] is `Clone` and
/// `PartialEq` — a test compares two of them — and `std::io::Error` is neither. What is lost is the
/// OS error's identity as an object; what is kept is everything a reader needs.
fn io_error(action: &'static str, path: &Path, source: &std::io::Error) -> Error {
    Error::Io {
        action,
        path: path.to_path_buf(),
        kind: source.kind(),
        detail: source.to_string(),
    }
}

// ---------------------------------------------------------------------------
// M5H8-10 — the durability calls, counted (see `fsync_meter`)
// ---------------------------------------------------------------------------

/// The behavioural half of NFR-009's barrier, which until the M5 fix hand did not exist.
///
/// `tests/ac_069.rs` holds the *static* half (one call site, and the order of two lines inside
/// `append`) and says in its own module docs why: 「no behaviour visible to this process separates
/// the two builds」. That is true of a **power cut**, which is what AC-069 asks about, and it was
/// read one step too widely — 「did the call happen」 *is* visible to this process, and req/86 §3.2
/// measured the price of never asking (two survivors, in the one function the crate exists for).
///
/// So these probes claim exactly one thing each and no more: **the call was made, before the
/// answer**. Nothing here says a byte reached a platter; req/52 §5's declared limit is untouched.
#[cfg(test)]
mod tests {
    use super::{fsync_meter, LedgerStore};
    use gx_core::{Cid, Timestamp, TransformationId};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    /// A directory of this process's own, under `std::env::temp_dir()`.
    ///
    /// `CARGO_TARGET_TMPDIR` is an integration-test variable and is not set for a lib unit test, so
    /// the `scratch` helper the `tests/` suites share is not reachable from here.
    fn scratch(name: &str) -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gx_m5fix_{name}_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the temp directory is writable");
        dir
    }

    fn cid(seed: u64) -> Cid {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&seed.to_be_bytes());
        Cid(raw)
    }

    fn tid(seed: u64) -> TransformationId {
        TransformationId(cid(7_000_000 + seed))
    }

    /// 🔴 NFR-009 逐語 (33 §2): 「`ledger.append`は**成功応答を返す前に**基盤ストレージへfsyncを完了
    /// する（fsync-on-append）」.
    ///
    /// The reading that makes this checkable: when `append` hands back `Ok`, a barrier has already
    /// completed on this thread. Reading the counter *after* the call returns is what makes it an
    /// ordering claim rather than a presence claim — a build that synced lazily, on drop, or from
    /// another thread would answer `Ok` with the count unmoved.
    #[test]
    fn an_append_answers_only_after_a_barrier_it_completed_itself() {
        let dir = scratch("append_barrier");
        let mut store = LedgerStore::open(dir.join("ledger.log")).expect("a fresh ledger");

        let (before, _) = fsync_meter::read();
        store
            .append(tid(1), cid(11), Timestamp(1))
            .expect("the append answers");
        let (after, _) = fsync_meter::read();
        println!("APPEND_BARRIERS_BEFORE={before} AFTER={after}");
        assert!(
            after > before,
            "`append` returned Ok having completed no durability barrier -- NFR-009 requires the \
             fsync *before* the successful answer, and this build answered without one"
        );

        // Each further successful append is its own promise, so each crosses the barrier again.
        let (base, _) = fsync_meter::read();
        for i in 2..5u64 {
            store
                .append(tid(i), cid(10 + i), Timestamp(i as i64))
                .expect("append");
        }
        let (grown, _) = fsync_meter::read();
        assert_eq!(
            grown - base,
            3,
            "three appends, three barriers -- a build that batched them would promise durability \
             for an entry whose bytes are still in a buffer"
        );

        // And the idempotent repeat (43 ASM-43-1) writes nothing, so it syncs nothing.
        let (b, _) = fsync_meter::read();
        store
            .append(tid(1), cid(11), Timestamp(1))
            .expect("the repeat is Ok");
        let (a, _) = fsync_meter::read();
        assert_eq!(
            b, a,
            "an `AlreadyPresent` answer wrote nothing and must sync nothing"
        );
    }

    /// 🔴 The survivor req/86 §3.2 found: `sync_parent_directory` reduced to `Ok(())`, unnoticed.
    ///
    /// A ledger whose *records* are all synced and whose *name* is not is a file a crash can take
    /// whole. Unix only, for the reason the function is: opening a directory as a file is how fsync
    /// reaches a directory there. On other platforms the gap is declared (req/52 §5) and
    /// `ac_069.rs`'s source scan is what holds the declaration.
    #[cfg(unix)]
    #[test]
    fn creating_a_ledger_pushes_its_directory_entry_to_the_device() {
        let dir = scratch("dir_barrier");
        let path = dir.join("ledger.log");

        let (all_before, dir_before) = fsync_meter::read();
        let store = LedgerStore::open(&path).expect("a fresh ledger");
        let (all_after, dir_after) = fsync_meter::read();
        println!(
            "DIRECTORY_BARRIERS_BEFORE={dir_before} AFTER={dir_after} \
             ALL_BARRIERS_BEFORE={all_before} AFTER={all_after}"
        );
        assert_eq!(
            dir_after - dir_before,
            1,
            "creating a ledger must fsync the directory that now names it; this build created the \
             file and left its name in a buffer"
        );
        // Both counters, deliberately. `DIRECTORIES` says 「the function ran to its end」 and
        // `BARRIERS` says 「a barrier completed inside it」 — a build that kept the bookkeeping and
        // dropped the durability would move the first and not the second.
        assert_eq!(
            all_after - all_before,
            1,
            "the directory sync completed no barrier -- that call is bookkeeping, not durability"
        );
        drop(store);

        // Re-opening an existing ledger creates no directory entry, so it syncs none. The negative
        // control the rule needs: without it, a `sync_parent_directory` called unconditionally
        // would pass the assertion above and do work on every open.
        let (all_before, before) = fsync_meter::read();
        let store = LedgerStore::open(&path).expect("the ledger reopens");
        let (all_after, after) = fsync_meter::read();
        assert_eq!(
            after, before,
            "`open` synced a directory entry it did not create (`if !existed` in `open`)"
        );
        assert_eq!(all_after, all_before, "a clean reopen crosses no barrier");
        assert_eq!(store.recovery().torn_tail_bytes, 0);
    }
}
