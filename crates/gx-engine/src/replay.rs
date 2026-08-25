// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Reading a journal back: the records it holds, and how much of the tail did not survive.
//!
//! Spec: 43 §7-1 ("replay at startup"), 42 §3.13 for what a record is, 41 §6 for the canonical form
//! (sem: SEM-gx-engine-305).
//!
//! # A pure function over bytes
//!
//! [`replay`] takes a `&[u8]` and returns values. It opens nothing, calls no adapter, and cannot
//! reach a substrate — which is **E-M5-2**'s ruling made structural rather than promised:
//!
//! > **E-M5-2**: replay is **a read-only operation that reconstructs Σ only**…it never calls the
//! > adapter (sem: SEM-gx-engine-306)
//!
//! `store.rs` reads the file and hands the bytes over. Splitting it this way is also what makes a
//! torn tail testable without a crash: the case is "these bytes", not "this machine lost power".
//!
//! # Why the refusal stops at the first bad record
//!
//! A journal is a sequence, and a record that cannot be read is a hole in it. Skipping the hole and
//! carrying on would produce a record list that no execution ever had — the transition after the
//! hole would appear to follow the one before it. So the prefix that replays is kept, everything
//! after the first refusal is reported as torn, and `store.rs` truncates the file to the prefix so
//! that the next append continues a sequence rather than extending a contradiction. gx-log's ledger
//! takes the same decision for the same reason, and the two files fail identically on purpose.
//!
//! What counts as "did not survive" is deliberately wide (sem: SEM-gx-engine-307): a header shorter than four bytes, a
//! length of zero, a length over [`crate::MAX_RECORD_BYTES`] (M5-20's ceiling, refused **before**
//! the allocation it asks for), a payload shorter than its header promised, and bytes that do not
//! decode as canonical DAG-CBOR. Every one of those is what a half-written record looks like from
//! the outside, and none of them is distinguishable from deliberate damage — which is why the
//! answer to all five is the same and is reported rather than logged.

use std::collections::BTreeMap;

use serde::Serialize;

use gx_canon::cbor;
use gx_core::{Cid, IntentId, Timestamp, TransformationId, VerdictKind};
use gx_log::Recovery;

use gx_witness::Provenance;

use crate::pipeline::Lifecycle;
use crate::store::{EngineJournalRecord, FingerprintRecord, InverseStatus, Rollback};
use crate::MAX_RECORD_BYTES;

/// Bytes of the length header in front of every record.
pub(crate) const LENGTH_BYTES: usize = 4;

/// 🔴 **R5 / DR-43-9** — the marker a chained journal begins with, and the whole of the format
/// version.
///
/// Eight bytes at offset zero, once per file, never repeated inside a record. Two properties chose
/// the spelling and both are mechanical:
///
/// * its first four bytes read as a big-endian `u32` of 1,196,772,178, which is past
///   [`MAX_RECORD_BYTES`], so a binary that predates this format stops at the first header rather
///   than decoding half a chained record as a legacy one;
/// * a legacy journal cannot begin with it, because a legacy file begins with a length header whose
///   value is at most 1 MiB and whose first byte is therefore `0x00`.
///
/// ∴ sniffing is exact in both directions and no side has to be told which format it is holding.
/// The version lives in the last two bytes: a second framing gets `GXJRNL02` and the genesis link
/// below changes with it, so records cannot be carried across formats.
pub const JOURNAL_MAGIC: &[u8; 8] = b"GXJRNL01";

/// 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the second framing, and the reason it
/// exists is **the record vocabulary**, not the bytes around the records.
///
/// # The defect this closes
///
/// The framing was versioned from the start ([`JOURNAL_MAGIC`]'s own doc, above, says a second one
/// "gets `GXJRNL02`"). The **vocabulary of the records inside it** was not, and R29 added a word to
/// it — `Rollback::Diverged`. The twenty-ninth audit built the journal that follows from that and
/// pointed a pre-R29 binary at it, and the pre-R29 binary did not refuse it. It read one record of
/// three, called the remaining 270 bytes a **torn tail** — the ordinary shape of a crash — copied
/// them aside, cut the live file from 415 bytes to 145, and after one ordinary append the journal
/// looked *healthy* with two records in it. Two records of history were gone from the live file and
/// the sentence the operator was shown named a remedy (`gx repair`) whose own documentation says it
/// cannot put them back.
///
/// `serde` refusing an unknown variant is correct. Everything downstream of that refusal was
/// reading "this record will not decode" as "this file was being written when the power went".
///
/// # Why a framing bump is the fix, and what it does and does not reach
///
/// A reader that cannot decode a record cannot be taught anything by that record — so the fact has
/// to be carried where a reader looks **before** decoding, which is the eight bytes at offset zero.
/// A journal written by this build carries `GXJRNL02`, and a binary that predates this format
/// cannot mistake it for its own: it does not match `GXJRNL01`, so such a binary sniffs it as
/// [`JournalFormat::Legacy`], and the first four bytes then read as a length header of 1,196,772,178
/// — past `MAX_RECORD_BYTES`, so the walk stops at the very first header rather than decoding
/// anything.
///
/// 🔴 **What that is worth depends on whether the project declared its format, and the honest answer
/// is written here rather than in a footnote.** A project made by `gx` declares `chained` in
/// `.gx/VERSION`, and R6 / `req/229` H-02 already turns *declared chained, file is not* into a
/// refusal that truncates nothing, quarantines nothing and appends nothing. That is fail-closed and
/// it is the road every shipped CLI takes. An **embedder** that calls `EngineJournal::open` with no
/// declaration has no such guard, and on that road an older binary cuts the file to nothing instead
/// of cutting it in half. Neither is a repair to those binaries — **they cannot be repaired from
/// here** — and `CHANGELOG.md` §3 and `docs/LIMITS.md` say so in those words. What this buys is the
/// next window, and the one after: a build that meets a framing it does not know now says so.
pub const JOURNAL_MAGIC_V2: &[u8; 8] = b"GXJRNL02";

/// The six bytes every framing marker this project has ever written begins with.
///
/// Kept separate from the two whole markers so that "a framing, but not one I know" is a question
/// this build can ask (see [`framing_this_build_does_not_know`]). Without it the only two answers
/// available are *mine* and *legacy*, and the second one is a lie that costs a file.
pub const JOURNAL_MAGIC_PREFIX: &[u8; 6] = b"GXJRNL";

/// 🔴 **R5 / DR-43-9** — bytes of the chain link stored after every record's payload.
pub const CHAIN_BYTES: usize = 32;

/// The domain string mixed into every link (42 §3.11's separation, one seam further out).
///
/// `gx_canon::cid::mint` prefixes `Domain::Leaf`'s `0x00`, so a link's preimage is
/// `0x00 || "gx.journal.chain.v1" || previous || payload`. The tag is what keeps a link from
/// colliding with a ledger leaf, whose preimage is `0x00 || canonical_dagcbor(leaf)`: the ledger's
/// second byte is a CBOR major type and this one is `g`.
const CHAIN_TAG: &[u8] = b"gx.journal.chain.v1";

/// 🔴 **R5 / DR-43-9** — the link a chained journal starts from.
///
/// Derived from [`JOURNAL_MAGIC`] rather than being a constant of zeroes, so that the first
/// record's link already commits to the format it was written under.
#[must_use]
pub fn genesis_link() -> [u8; 32] {
    gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[CHAIN_TAG, JOURNAL_MAGIC]).0
}

/// 🔴 **R30 / `req/372` M-02** — the link a [`JournalFormat::ChainedV2`] journal starts from.
///
/// A different value from [`genesis_link`] because it is minted over a different marker, which is
/// [`JOURNAL_MAGIC`]'s own doc keeping its promise: *"the genesis link below changes with it, so
/// records cannot be carried across formats."* Lifting a run of frames out of a v1 journal and
/// dropping them into a v2 one does not verify, and the mismatch is a chain break rather than a
/// silent acceptance.
#[must_use]
pub fn genesis_link_v2() -> [u8; 32] {
    gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[CHAIN_TAG, JOURNAL_MAGIC_V2]).0
}

/// The link a journal of this framing starts from, or `None` for [`JournalFormat::Legacy`], which
/// has no chain.
///
/// One function rather than a `match` at each call site, for `req/38` §227's reason: the question
/// *which chain does this format start from* is asked in four places and a question spelled four
/// times is a question repaired in one of them.
#[must_use]
pub fn genesis_link_of(format: JournalFormat) -> Option<[u8; 32]> {
    match format {
        JournalFormat::Legacy => None,
        JournalFormat::Chained => Some(genesis_link()),
        JournalFormat::ChainedV2 => Some(genesis_link_v2()),
    }
}

/// 🔴 **R30 / `req/372` M-02** — do these bytes carry a framing marker that this build has never
/// heard of.
///
/// This is the question the twenty-ninth audit's pre-R29 binary could not ask, and the reason it
/// mistook an intact journal for a crashed one. `true` means: the file begins with this project's
/// marker prefix and the version after it is neither of the two this build writes — so the records
/// inside are framed by a **newer** `gx`, and every conclusion this build could draw by walking
/// them as legacy frames would be false.
///
/// The caller's duty on `true` is to touch nothing: not truncate, not quarantine, not append. A
/// file from the future is not a damaged file, and the difference is the whole finding.
#[must_use]
pub fn framing_this_build_does_not_know(bytes: &[u8]) -> bool {
    bytes.len() >= JOURNAL_MAGIC.len()
        && bytes.starts_with(JOURNAL_MAGIC_PREFIX)
        && !bytes.starts_with(JOURNAL_MAGIC)
        && !bytes.starts_with(JOURNAL_MAGIC_V2)
}

/// 🔴 **R5 / DR-43-9** — the link one record adds to the chain.
///
/// `link(previous, payload)` is what the writer stores after `payload` and what a reader recomputes
/// and compares. The recursion is the whole property: a link commits to its record **and** to every
/// record before it, so the value after the last record is a 32-byte statement about the entire
/// prefix. `req/227` H-01 measured what the absence cost — the detector R4 put here compared "the
/// same number of bytes came back as the same number of whole records", which a record copied from
/// elsewhere in the same file, two adjacent records swapped, and one bit flipped inside a payload
/// all satisfy.
#[must_use]
pub fn link(previous: &[u8; 32], payload: &[u8]) -> [u8; 32] {
    gx_canon::cid::mint(
        gx_canon::cid::Domain::Leaf,
        &[CHAIN_TAG, previous.as_slice(), payload],
    )
    .0
}

/// 🔴 **R12 / `req/242` H-01 (d)** — may this open create the journal it names.
///
/// Three audits in a row found something other than a repair writing a project's own declarations
/// (`req/238` H-01, `req/240` H-01, `req/242` H-01), and the fourth face of it was this: a journal
/// that had been **lost** was re-created, empty, by the next writer verb, and the diagnosis about
/// the loss disappeared with it. `req/38` §181 ruling 2 asks for the answer in a type rather than
/// in a call order, and this is the engine's half of it — the CLI's half is
/// `gx_cli::declaration::DeclarationWriter`.
///
/// 🔴 ~~[`JournalCreation::Permitted`] is [`Default`] so that an embedder, and this crate's own
/// tests, keep the behaviour `EngineJournal::open` has always had: a caller with no `.gx/` to
/// consult cannot tell a new project from a damaged one.~~ — **reversed in R13 (`req/244` L-07).**
///
/// # Why the default moved to `Refused`
///
/// R12's sentence is a good argument for the *call* and a bad one for the **default**, and
/// `req/244` L-07 named the difference. `EngineJournal::open_declared` still passes `Permitted` as
/// a constant, so an embedder that calls it gets exactly the behaviour it always had; what changed
/// is what a caller gets when it says nothing. Saying nothing is `ProjectAnchor { .. }`'s
/// `..Default::default()`, and `ProjectAnchor::none()`, and every struct literal a later hand
/// writes in a hurry.
///
/// The audit measured what stood between those and a re-created journal, and the answer was a
/// **string**: `probes/doubt/tests/declaration_writer_doubt.rs` D-3 asserts that `gx-cli` builds
/// `ProjectAnchor {` in exactly one place and mentions `JournalCreation::Permitted` in none. One
/// `..Default::default()` satisfies both greps and turns the barrier off, because neither string
/// appears. A default that fails **safe** is the same barrier with the type holding it: a caller
/// that means "create it" writes the word, and a caller that wrote nothing gets the refusal a
/// missing journal deserves (`req/242` H-01 (d): a journal that was lost is a fact `gx repair`
/// reports, not a file the next writer invents).
///
/// What it costs, said plainly: an embedder that built a `ProjectAnchor` with `..Default::default()`
/// and relied on the engine to create a fresh journal now gets a refusal instead. The migration is
/// one word — `journal_creation: JournalCreation::Permitted` — and `CHANGELOG.md` §3 carries it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JournalCreation {
    /// Create the file if it is not there and stamp [`JOURNAL_MAGIC`] on it. What
    /// [`crate::store::EngineJournal::open`] and `open_declared` pass, by name.
    Permitted,
    /// Refuse a path that is not there. What `gx-cli` passes on every road except the one that
    /// turns a directory into a project — and, since R13, what a caller that names nothing gets.
    #[default]
    Refused,
}

/// 🔴 **R5 / DR-43-9** — which framing a journal's bytes are in.
///
/// Sniffed from the file rather than configured: [`JOURNAL_MAGIC`] is present or it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalFormat {
    /// Before R5: `[u32 length][payload]`, repeated, with no marker and no link. A journal written
    /// by an earlier binary. It is read, and what may be trusted in it is narrower — see
    /// [`crate::store::EngineJournal::format`].
    Legacy,
    /// R5: [`JOURNAL_MAGIC`], then `[u32 length][payload][32-byte link]`, repeated.
    Chained,
    /// 🔴 **R30 / `req/372` M-02** — [`JOURNAL_MAGIC_V2`], then the same frames.
    ///
    /// **The framing bytes are identical to [`JournalFormat::Chained`]'s.** What the version
    /// distinguishes is the **record vocabulary**: a v2 journal may contain words a pre-R30 binary
    /// has never been taught, `Rollback::Diverged` first among them. The marker is the only place
    /// that fact can live, because a reader that cannot decode a record cannot be told anything
    /// by that record.
    ///
    /// This is what a journal created by this build is. A [`JournalFormat::Chained`] file made by
    /// an earlier binary keeps its own framing and is read and appended to exactly as before —
    /// what is refused is a **transition**, not a format, which is the same shape R6 chose for the
    /// downgrade.
    ChainedV2,
}

impl JournalFormat {
    /// The name a report prints.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            JournalFormat::Legacy => "legacy",
            JournalFormat::Chained => "chained",
            JournalFormat::ChainedV2 => "chained-v2",
        }
    }

    /// The marker a journal of this framing carries at offset zero, or `None` for
    /// [`JournalFormat::Legacy`], which carries none.
    #[must_use]
    pub const fn marker(self) -> Option<&'static [u8; 8]> {
        match self {
            JournalFormat::Legacy => None,
            JournalFormat::Chained => Some(JOURNAL_MAGIC),
            JournalFormat::ChainedV2 => Some(JOURNAL_MAGIC_V2),
        }
    }

    /// Whether this framing carries a per-record chain link.
    #[must_use]
    pub const fn is_chained(self) -> bool {
        matches!(self, JournalFormat::Chained | JournalFormat::ChainedV2)
    }

    /// 🔴 **R30 / `req/372` M-02** — how much of this build's record vocabulary a journal of this
    /// framing is allowed to carry, as a rank: a record whose own rank is higher than the
    /// journal's may not be appended to it.
    ///
    /// The ordering is the point rather than the numbers: `Legacy` and `Chained` were both written
    /// by binaries that had never heard of the words R29 minted, so a record carrying one of those
    /// words does not belong in either. See `EngineJournalRecord::minimum_format`.
    #[must_use]
    pub const fn vocabulary_rank(self) -> u8 {
        match self {
            JournalFormat::Legacy | JournalFormat::Chained => 1,
            JournalFormat::ChainedV2 => 2,
        }
    }
}

/// 🔴 **R5 / DR-43-9** — where a chain stopped agreeing with itself, and what the two values were.
///
/// A break is **not** a torn tail and the two are not folded: a torn tail is the ordinary shape of
/// a crash (a record that was being written when the power went), and a break is a record that is
/// whole and is not the record this chain reached. `EngineJournal::open` truncates the first and
/// refuses to touch the second, because cutting a file at a break would delete every record after
/// somebody else's edit and call it a repair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainBreak {
    /// The offset of the frame whose stored link did not verify, inside the bytes replayed.
    pub at: u64,
    /// The link the file stores after that record's payload.
    pub found: [u8; 32],
    /// The link the record's payload and the chain up to it produce.
    pub expected: [u8; 32],
}

/// Where a replay begins: the format, and the link to continue from.
///
/// Handed to [`replay_onward`] by a caller reading only the bytes that arrived after its own read
/// offset, which is the one road where the format cannot be sniffed (the marker is at the file's
/// start and those bytes are past it).
#[derive(Clone, Copy, Debug)]
pub struct Resume {
    /// The framing the file was opened in.
    pub format: JournalFormat,
    /// The link after the last record already consumed, for [`JournalFormat::Chained`].
    pub chain: Option<[u8; 32]>,
}

/// What a replay produced.
///
/// The counts are in a [`Recovery`], gx-log's struct, because "how many records came back and how
/// many bytes after them did not" (sem: SEM-gx-engine-308) is one question about append-only files and not two (see the
/// crate documentation).
#[derive(Clone, Debug)]
pub struct Replay {
    records: Vec<EngineJournalRecord>,
    recovery: Recovery,
    good_bytes: u64,
    /// 🔴 **R4 / `req/225` H-03** — where the last record of the good prefix lies, as
    /// `(offset, framed_length)` inside the bytes this replay was given.
    ///
    /// Offsets rather than the bytes themselves: the caller already holds the buffer, and a
    /// `Vec<u8>` here would copy every record's payload on a road that runs at every open and
    /// every catch-up. `None` for an empty prefix, where a length check is exact on its own.
    tail_span: Option<(u64, u64)>,
    /// 🔴 **R5 / DR-43-9** — which framing these bytes were read in.
    format: JournalFormat,
    /// 🔴 **R5 / DR-43-9** — the link after the last record that replayed, for a chained file.
    ///
    /// `None` for [`JournalFormat::Legacy`], where there is nothing to carry.
    chain: Option<[u8; 32]>,
    /// 🔴 **R5 / DR-43-9** — the first record whose stored link did not verify, if there is one.
    chain_break: Option<ChainBreak>,
}

impl Replay {
    /// The records that replayed, oldest first.
    #[must_use]
    pub fn records(&self) -> &[EngineJournalRecord] {
        &self.records
    }

    /// Take the records out.
    #[must_use]
    pub fn into_records(self) -> Vec<EngineJournalRecord> {
        self.records
    }

    /// How many records replayed, and how many bytes after them did not.
    #[must_use]
    pub fn recovery(&self) -> Recovery {
        self.recovery
    }

    /// The length of the prefix that replayed — what the file is truncated to.
    #[must_use]
    pub fn good_bytes(&self) -> u64 {
        self.good_bytes
    }

    /// 🔴 **R4 / `req/225` H-03** — `(offset, framed_length)` of the last record that replayed.
    ///
    /// `gx_log::LedgerStore` has carried the same value since R3 and it is what catches a rewrite
    /// that keeps the file's length exactly. `req/225` H-03 measured the journal side of that gap:
    /// one bit flipped in the tail of a live project's journal, `/healthz` `200`, `POST
    /// /candidates` `201`, a **signed** checkpoint, and the next start-up refusing to open the
    /// project. The offsets are relative to the buffer `replay` was handed, so a caller reading
    /// only the new bytes adds its own read offset.
    #[must_use]
    pub fn tail_span(&self) -> Option<(u64, u64)> {
        self.tail_span
    }

    /// 🔴 **R5 / DR-43-9** — the framing these bytes were read in.
    #[must_use]
    pub fn format(&self) -> JournalFormat {
        self.format
    }

    /// 🔴 **R5 / DR-43-9** — the link after the last record that replayed.
    ///
    /// This is the value a caller keeps and compares on the next read: it is a digest of the whole
    /// prefix, so "the file still holds what I read" becomes one comparison of 32 bytes instead of
    /// a count of bytes and a count of records.
    #[must_use]
    pub fn chain(&self) -> Option<[u8; 32]> {
        self.chain
    }

    /// 🔴 **R5 / DR-43-9** — where the chain stopped agreeing with itself, if it did.
    ///
    /// `Some` means a whole record was read whose stored link is not the link its payload and its
    /// predecessors produce: the bytes were rewritten after they were written. The replay stops
    /// there for the reason it stops at an undecodable record — everything after a hole is a
    /// sequence no execution ever had — but the caller's response is **not** the same, because a
    /// break is not a crash's trace and must not be truncated.
    #[must_use]
    pub fn chain_break(&self) -> Option<ChainBreak> {
        self.chain_break
    }

    /// Σ, rebuilt from the records that replayed (**E-M5-2**).
    ///
    /// The torn tail is not in it, and that is the point of stopping at the first bad record: a
    /// state rebuilt from a prefix is a state the execution actually passed through, while one
    /// rebuilt from a sequence with a hole is a state no execution ever had.
    #[must_use]
    pub fn sigma(&self) -> Sigma {
        reconstruct(&self.records)
    }
}

/// Read a journal's bytes back into the records they hold, from the start of the file.
///
/// Infallible by construction: a torn tail is the ordinary shape of a crash, not a failure of this
/// function, so it is *reported* in the [`Recovery`] rather than raised. The only thing that could
/// be an error here — an unreadable file — belongs to the caller that opened it.
///
/// 🔴 **R5 / DR-43-9** — the framing is sniffed here and nowhere else: [`JOURNAL_MAGIC`] at offset
/// zero means [`JournalFormat::Chained`] and its absence means [`JournalFormat::Legacy`]. The
/// marker's bytes count as replayed (`good_bytes` includes them), so a caller that truncates to
/// `good_bytes` keeps the file readable and one that compares `good_bytes` against the file's
/// length is still asking "did everything on this file come back".
#[must_use]
pub fn replay(bytes: &[u8]) -> Replay {
    // 🔴 **R30 / `req/372` M-02** — the newer marker is tried **first**. Order matters only for
    // readability here (the two markers cannot both match), but the shape is the one a third
    // version has to extend, and putting the newest at the top is what keeps that extension from
    // being an insertion into the middle of an `else if` chain.
    let (format, from) = if bytes.starts_with(JOURNAL_MAGIC_V2) {
        (JournalFormat::ChainedV2, JOURNAL_MAGIC_V2.len())
    } else if bytes.starts_with(JOURNAL_MAGIC) {
        (JournalFormat::Chained, JOURNAL_MAGIC.len())
    } else {
        // 🔴 **R32 / `req/392` M-01** — a file of **zero** bytes reaches this arm too, and that is
        // the repair rather than an accident of the `else`.
        //
        // Until R32 there was an arm above this one keyed on `bytes.is_empty()` answering
        // `(JournalFormat::ChainedV2, 0)`, whose comment read: *"A file with no bytes has no
        // framing to sniff and no records to lose. It is reported as the format a writer is about
        // to give it, so that an empty project and a project with one record answer the same
        // question the same way."* It said what a **writer** is about to do, inside the function
        // whose whole contract is that the framing is sniffed **from the bytes**, and the
        // thirty-first audit measured what that cost on the door that cannot write: a project
        // declaring `chained` whose journal is zero bytes was reported by
        // `EngineJournal::open_read_only_declared` as `chained-v2`, R6's downgrade guard compared
        // `1 > 2` and did not fire, and `gx repair --json` printed `journal_intact: true` with
        // `journal_intact_basis: "chain"` over a file holding no chain — while the **same project
        // with four unmarked bytes** was refused. Less information reported as more health.
        //
        // R31 removed the second source on the writer's door by extending `bytes` with the marker
        // it had just written, so no caller of this function reaches it with an empty buffer and
        // then asks what a writer would do. What is left is the honest reading: zero bytes carry
        // no marker, absence of a marker is `Legacy`, and a project that declares a chain over a
        // file with none is a **downgrade** — which is the question R6 built and which this arm now
        // lets it be asked. `crates/gx-engine/tests/r32_readers_door_zero_byte.rs` asserts it on
        // the reader's door and `crates/gx-cli/tests/r32_zero_byte_report.rs` on the shipped verb.
        //
        // 🔴 A file carrying a marker this build does not know reaches this arm and is reported as
        // `Legacy`, which is wrong about it — and is *deliberately* left wrong here, because this
        // function is infallible by construction and has no vocabulary for "from the future". The
        // fact is recovered one layer out, where it can be acted on:
        // `EngineJournal::open_declared_creating` asks `framing_this_build_does_not_know` before
        // it cuts anything, and cuts nothing when the answer is yes.
        (JournalFormat::Legacy, 0)
    };
    let chain = genesis_link_of(format);
    walk(bytes, from, Resume { format, chain })
}

/// 🔴 **R5 / DR-43-9** — the same read, continuing a file a caller has already read part of.
///
/// [`crate::store::EngineJournal::catch_up`] hands over only the bytes past its read offset, and
/// those bytes hold no marker: the format and the link to continue from come from the store, which
/// learnt both when it opened the file.
#[must_use]
pub fn replay_onward(bytes: &[u8], resume: Resume) -> Replay {
    walk(bytes, 0, resume)
}

/// 🔴 **R5 / DR-43-9** — what a link-only walk of a chained journal found.
///
/// The values [`crate::store::EngineJournal::prefix_intact`] compares, obtained **without decoding
/// a single record**. That is the whole reason this type exists beside [`Replay`]: the question
/// "is this the file I read" is answered by the chain, and answering it through a full replay
/// would pay CBOR for every record on a road that runs on every write **and** every read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainWalk {
    /// The link after the last whole record walked, or `None` for a file with no marker.
    pub head: Option<[u8; 32]>,
    /// How many whole, linked records were walked.
    pub records: u64,
    /// How many bytes they occupy, the marker included.
    pub good_bytes: u64,
    /// Where a stored link stopped matching, if one did.
    pub break_at: Option<u64>,
}

/// 🔴 **R5 / DR-43-9** — walk a chained journal's links from the marker, decoding nothing.
///
/// `head: None` for a file that does not begin with [`JOURNAL_MAGIC`]: there is no chain to walk
/// and the caller has to fall back on what a file without links can offer (see
/// [`crate::store::EngineJournal::prefix_intact`]).
///
/// The framing rules are [`replay`]'s, exactly — a zero or over-ceiling length, a payload that runs
/// past the buffer, a link that runs past it — because a walk that stopped at a different place
/// from the replay would make the two disagree about how much of a file is good.
#[must_use]
pub fn walk_links(bytes: &[u8]) -> ChainWalk {
    // 🔴 **R30 / `req/372` M-02** — both framings walk, each from its own genesis. A v2 file walked
    // from v1's genesis would report a break at the first record, which is a *false* report of
    // somebody having rewritten the file.
    let format = if bytes.starts_with(JOURNAL_MAGIC_V2) {
        JournalFormat::ChainedV2
    } else if bytes.starts_with(JOURNAL_MAGIC) {
        JournalFormat::Chained
    } else {
        return ChainWalk {
            head: None,
            records: 0,
            good_bytes: 0,
            break_at: None,
        };
    };
    let mut at = JOURNAL_MAGIC.len();
    let mut chain = genesis_link_of(format).unwrap_or_else(genesis_link);
    let mut records = 0u64;
    loop {
        if at + LENGTH_BYTES > bytes.len() {
            break;
        }
        let mut header = [0u8; LENGTH_BYTES];
        header.copy_from_slice(&bytes[at..at + LENGTH_BYTES]);
        let length = u32::from_be_bytes(header);
        if length == 0 || length > MAX_RECORD_BYTES {
            break;
        }
        let start = at + LENGTH_BYTES;
        let Some(end) = start.checked_add(length as usize) else {
            break;
        };
        let Some(frame_end) = end.checked_add(CHAIN_BYTES) else {
            break;
        };
        if frame_end > bytes.len() {
            break;
        }
        let expected = link(&chain, &bytes[start..end]);
        if bytes[end..frame_end] != expected {
            return ChainWalk {
                head: Some(chain),
                records,
                good_bytes: at as u64,
                break_at: Some(at as u64),
            };
        }
        chain = expected;
        records += 1;
        at = frame_end;
    }
    ChainWalk {
        head: Some(chain),
        records,
        good_bytes: at as u64,
        break_at: None,
    }
}

/// The body of [`replay`] and [`replay_onward`].
///
/// `from` is where the records start inside `bytes` — past the marker on a whole chained file, and
/// zero everywhere else.
fn walk(bytes: &[u8], from: usize, resume: Resume) -> Replay {
    let total = bytes.len() as u64;
    let mut records = Vec::new();
    let mut good: u64 = from as u64;
    let mut at = from;
    // 🔴 **R4 / `req/225` H-03** — the last whole record's frame, tracked as it is passed.
    let mut tail_span: Option<(u64, u64)> = None;
    // 🔴 **R5 / DR-43-9** — the chain, carried record by record.
    let mut chain = resume.chain;
    let mut chain_break = None;
    let chained = resume.format.is_chained();
    // A chained read with no link to continue from cannot verify anything, and a verifier that
    // cannot verify must not report success: it is treated as a break at the first record rather
    // than as a pass. `EngineJournal` always supplies one, so this is a guard and not a road.
    let link_bytes = if chained { CHAIN_BYTES } else { 0 };

    loop {
        if at + LENGTH_BYTES > bytes.len() {
            break;
        }
        let mut header = [0u8; LENGTH_BYTES];
        header.copy_from_slice(&bytes[at..at + LENGTH_BYTES]);
        let length = u32::from_be_bytes(header);
        if length == 0 || length > MAX_RECORD_BYTES {
            break;
        }
        let start = at + LENGTH_BYTES;
        let Some(end) = start.checked_add(length as usize) else {
            break;
        };
        let Some(frame_end) = end.checked_add(link_bytes) else {
            break;
        };
        if frame_end > bytes.len() {
            break;
        }
        // 🔴 **R5 / DR-43-9** — the link is checked **before** the payload is decoded, so that a
        // record which is both rewritten and undecodable is reported as the rewrite it is.
        let mut verified = None;
        if chained {
            let Some(previous) = chain else {
                chain_break = Some(ChainBreak {
                    at: at as u64,
                    found: [0u8; CHAIN_BYTES],
                    expected: [0u8; CHAIN_BYTES],
                });
                break;
            };
            let expected = link(&previous, &bytes[start..end]);
            let mut found = [0u8; CHAIN_BYTES];
            found.copy_from_slice(&bytes[end..frame_end]);
            if found != expected {
                chain_break = Some(ChainBreak {
                    at: at as u64,
                    found,
                    expected,
                });
                break;
            }
            verified = Some(expected);
        }
        let Ok(record) = cbor::decode::<EngineJournalRecord>(&bytes[start..end]) else {
            break;
        };
        // The chain advances only over a record that is **in** `records`: a link accepted for a
        // payload that then failed to decode would leave the head describing a prefix the caller
        // does not hold.
        if verified.is_some() {
            chain = verified;
        }
        records.push(record);
        tail_span = Some((
            at as u64,
            (LENGTH_BYTES + length as usize + link_bytes) as u64,
        ));
        good += (LENGTH_BYTES + length as usize + link_bytes) as u64;
        at = frame_end;
    }

    Replay {
        recovery: Recovery {
            records: records.len() as u64,
            torn_tail_bytes: total - good,
        },
        records,
        good_bytes: good,
        tail_span,
        format: resume.format,
        chain: if chained { chain } else { None },
        chain_break,
    }
}

// ---------------------------------------------------------------------------
// E-M5-2 -- Σ, and the read-only operation that rebuilds it
// ---------------------------------------------------------------------------

/// One draft: an `IntentId` and the seed 41 §6 injected with it.
///
/// A draft has no `TransformationId` (43 T-1, **E-M5-3**) and therefore no row in a table keyed on
/// one (**M5-17, adopted (b)**), so it is its own component of Σ. The seed is here because 42 §3.13 says
/// what it is for -- "`rng_seed` (`DraftCreated`) records the seed the engine injects for the
/// randomness/clock it injects at the boundary, and replaying with the same seed re-executes
/// deterministically" (sem: SEM-gx-engine-309) -- and a Σ that dropped it would
/// make AC-039's control experiment ("agreement is not guaranteed under a different seed") unwritable: the seed would
/// reach nothing that is compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DraftRow {
    /// The intent this draft is of.
    pub intent_id: IntentId,
    /// The seed injected at 41 §6's boundary.
    pub rng_seed: u64,
}

/// One row of the state table: everything the journal fixes about a transformation, by name.
///
/// **Names and digests, never bodies** (ASM-9). A `PlannedDelta` is here as its CID and the body is
/// in the [`crate::store::BlobStore`]; an `ObjectSnapshot` is not here at all, because the journal
/// records `Fingerprint₀` rather than the snapshot it was taken from.
///
/// # Why nearly everything is an `Option`
///
/// 42 §5: "`EngineJournalRecord` … may be trimmed once a commit is settled" (sem: SEM-gx-engine-310). A trimmed journal legitimately begins in
/// the middle, so a surviving prefix can hold an `Aborted` for a transformation whose `Planned`
/// record is gone. Reconstructing an `intent_id` for it would be the engine telling a story its
/// records do not support, and `None` is the true answer. `state` is an `Option` for the same
/// reason and one further: a journal can name a transformation with a record that fixes no state at
/// all (`ApplyStarted`, `InverseEscrowed`), and 43 §1 has no value meaning "unknown" to borrow
/// (sem: SEM-gx-engine-311).
/// # No `PartialEq`, and the compiler is why
///
/// This row holds a [`FingerprintRecord`], which hand 1 left without `PartialEq` because
/// **E-M4-15** took it off `Fingerprint` -- 42 §3.5's comparison has three answers and `==` has two.
/// Deriving it here was tried and refused by the compiler, which is the ruling holding across a type
/// that did not exist when it was made. Σ is compared as **bytes** ([`Sigma::canonical_bytes`]), and
/// a probe that wants to name the differing row compares the rows' canonical encodings the same way.
#[derive(Clone, Debug, Serialize)]
pub struct StateRow {
    /// The transformation this row is about.
    pub transformation: TransformationId,
    /// The intent it came from (T-2).
    pub intent_id: Option<IntentId>,
    /// The CID of the `PlannedDelta` T-2 fixed. The body is in the blob store.
    pub delta_cid: Option<Cid>,
    /// `Fingerprint₀`, the precondition T-10a's CAS will compare against.
    pub fp0: Option<FingerprintRecord>,
    /// Where 43 §1 says it is.
    pub state: Option<Lifecycle>,
    /// What the gate answered, where a gate was asked. `None` after T-4e.
    pub verdict: Option<VerdictKind>,
    /// The digest of that verdict's proof. `None` after T-4e, where no verdict exists (E-M5-7).
    pub verdict_digest: Option<Cid>,
    /// DR-2's `enforced`. `false` after T-8r and after T-4e's degraded admission.
    pub enforced: bool,
    /// 43 T-4e's flag: this admission happened because the collector could not be reached.
    pub fail_posture_engaged: bool,
    /// The canonical CID T-8 fixed.
    pub canonical_cid: Option<Cid>,
    /// 🔴 **E-M5-1**: the delta the adapter was asked to apply, if it was asked.
    ///
    /// The one fact that separates "the world did not move" from "the world moved and nothing
    /// recorded it" (req/78 §3.2 Λ4; sem: SEM-gx-engine-312). Hand 5's recovery is the consumer; Σ carries it because a
    /// reconstruction that dropped it would leave the recovery blind to exactly the record that was
    /// added for it.
    pub apply_started: Option<Cid>,
    /// 🔴 Two-phase escrow (`req/38` §98, ruling 1; sem: SEM-gx-engine-313): the journalled observation's CID, where the
    /// applied call's answer was recorded (`ApplyObserved`). Recovery's second consumer-fact for
    /// a `Pending` escrow: present → the completion can be re-resolved from the observation
    /// store; absent → the observation died with the process and the escrow folds to
    /// `Unavailable` honestly (the crash window `req/160` §1-1 names as structurally unclosable).
    pub observation_cid: Option<Cid>,
    /// 🔴 **AC-038**: what became of 43 T-10c's best-effort rollback, where one was in question.
    ///
    /// `None` on every row that has not aborted under T-10c. Σ carries it because the journal
    /// records it: a reconstruction that dropped the field would make "the inverse was applied and
    /// the adapter refused it" (sem: SEM-gx-engine-314) invisible to a replay, which is the one fact an operator reading an
    /// `ApplyFailed` needs. See [`crate::store::Rollback`].
    pub rollback: Option<Rollback>,
    /// 🔴 **M5-25, adopted (a)** -- the provenance the engine derived for this transformation (42 §3.9) (sem: SEM-gx-engine-315).
    ///
    /// `None` until the commit critical section opens, which is where the record is written. Σ
    /// carries it for the same reason it carries everything else here: the journal fixes it, so a
    /// state rebuilt from the journal has it.
    pub provenance: Option<Provenance>,
    /// T-12's edge: the transformation whose commit superseded this one.
    pub superseded_by: Option<TransformationId>,
}

impl StateRow {
    /// A row that knows only which transformation it is about.
    fn about(transformation: TransformationId) -> Self {
        Self {
            transformation,
            intent_id: None,
            delta_cid: None,
            fp0: None,
            state: None,
            verdict: None,
            verdict_digest: None,
            enforced: true,
            fail_posture_engaged: false,
            canonical_cid: None,
            apply_started: None,
            observation_cid: None,
            rollback: None,
            provenance: None,
            superseded_by: None,
        }
    }
}

/// One row of the escrow index (42 §3.12), by name.
///
/// The body it points at is in the blob store, and [`crate::store::BlobStore::escrowed`] is where
/// the two are put back together through the checked constructor (**E-M5-6**).
///
/// `retained_until` is always `None` when this row comes from a journal, and that is not an
/// omission being hidden: 42 §3.13's `InverseEscrowed` record has three fields and none of them is
/// a deadline, while DR-9 makes "unlimited" the OSS default (sem: SEM-gx-engine-316) and req/78 N-06 keeps the enforcement of
/// deadlines out of v0.1 entirely. The seat is here because 42 §3.12 has one; that the journal has
/// none is raised in the report rather than papered over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct EscrowRow {
    /// The committed transformation this inverse undoes.
    pub transformation: TransformationId,
    /// The CID of the inverse delta. `None` is 42 §3.12's `Unavailable`.
    pub inverse_cid: Option<Cid>,
    /// DR-9's deadline. See the type documentation for why a journal never supplies one.
    pub retained_until: Option<Timestamp>,
    /// What has become of it.
    pub status: InverseStatus,
}

/// One row of Σ's ledger component: a commit, as the journal witnesses it.
///
/// 🔴 **This is the journal's claim about the ledger, not the ledger's own root.** E-M5-2 reads
/// AC-039's "resulting state" as "Σ (state table + ledger root + escrow index)" (sem: SEM-gx-engine-317), and in v0.1 the engine reaches the
/// ledger only at T-11 -- which is hand 4. What a journal can witness today is which
/// transformations reached `Committed` and at which sequence number, and that is what is here. When
/// hand 4 wires `gx_log::LedgerStore`, the agreement between this component and the ledger's own
/// root becomes a checkable claim rather than a definition; the report raises it as such.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CommittedRow {
    /// The transformation that committed.
    pub transformation: TransformationId,
    /// The ledger sequence number T-11 recorded.
    pub ledger_seq: u64,
}

/// **Σ** — the engine's state, and the whole of what AC-039 compares (**E-M5-2**).
///
/// > **M5-02, adopted (a)** = **E-M5-2**: replay is **a read-only operation that reconstructs Σ
/// > only** -- AC-039's "resulting state" is read as Σ (state table + ledger root + escrow index).
/// > It never calls the adapter (sem: SEM-gx-engine-318)
///
/// Four components, in the order the ruling names them plus the draft phase that 43 T-1 puts
/// outside the table:
///
/// | component | 43 / 42 | where it comes from |
/// |---|---|---|
/// | `drafts` | 43 T-1, **M5-17, adopted (b)** | `DraftCreated` |
/// | `transformations` | 42 §1.3-3's "external table" (sem: SEM-gx-engine-319) | every record naming a `TransformationId` |
/// | `escrow` | 42 §3.12 | `InverseEscrowed`, and `Superseded` for the status |
/// | `ledger` | 42 §3.11 via T-11 | `Committed` |
///
/// # Bit-equality is byte equality of the canonical form
///
/// AC-039 asks for "bit-equal" (sem: SEM-gx-engine-320), and this type answers it with [`Sigma::canonical_bytes`] rather
/// than with a derived comparison. Two reasons, and the first is hand 1's: Σ holds a
/// [`FingerprintRecord`], and **E-M4-15** took `==` off `Fingerprint` because 42 §3.5's comparison
/// has three answers. The second is that "bit-equal" (sem: SEM-gx-engine-321) is a claim about *bytes*, and comparing the
/// canonical encoding is that claim rather than a proxy for it -- the encoding is a function of the
/// value (42 §2.1), so byte equality implies field equality and says more than a derived `==`
/// would. The second reason is not a preference: `#[derive(PartialEq)]` on this type **does not
/// compile**, because [`StateRow`] holds a [`FingerprintRecord`] and E-M4-15 is why that has no
/// `==` either. The ruling reaches Σ through the type system rather than through a comment.
///
/// # The order is fixed here and nowhere else
///
/// The vectors are sorted by [`Sigma::new`], not by the callers. A Σ built from a `BTreeMap` and a
/// Σ built from a `Vec` in journal order would encode to different bytes while holding the same
/// state, and AC-039 would then be measuring iteration order.
#[derive(Clone, Debug, Serialize)]
pub struct Sigma {
    drafts: Vec<DraftRow>,
    transformations: Vec<StateRow>,
    escrow: Vec<EscrowRow>,
    ledger: Vec<CommittedRow>,
}

impl Sigma {
    /// Build Σ from its four components, in the one order that makes them comparable.
    #[must_use]
    pub fn new(
        mut drafts: Vec<DraftRow>,
        mut transformations: Vec<StateRow>,
        mut escrow: Vec<EscrowRow>,
        mut ledger: Vec<CommittedRow>,
    ) -> Self {
        drafts.sort_by_key(|d| d.intent_id.0 .0);
        transformations.sort_by_key(|t| t.transformation.0 .0);
        escrow.sort_by_key(|e| e.transformation.0 .0);
        // By sequence first: the ledger's own order is the one thing a ledger has that a set does
        // not, and sorting it by id would throw the fact away before comparing it.
        ledger.sort_by_key(|c| (c.ledger_seq, c.transformation.0 .0));
        Self {
            drafts,
            transformations,
            escrow,
            ledger,
        }
    }

    /// The drafts, by `IntentId`.
    #[must_use]
    pub fn drafts(&self) -> &[DraftRow] {
        &self.drafts
    }

    /// The state table, by `TransformationId`.
    #[must_use]
    pub fn transformations(&self) -> &[StateRow] {
        &self.transformations
    }

    /// The escrow index, by `TransformationId`.
    #[must_use]
    pub fn escrow(&self) -> &[EscrowRow] {
        &self.escrow
    }

    /// The commits, in ledger order.
    #[must_use]
    pub fn ledger(&self) -> &[CommittedRow] {
        &self.ledger
    }

    /// One row of the state table.
    #[must_use]
    pub fn state_of(&self, id: &TransformationId) -> Option<&StateRow> {
        self.transformations
            .iter()
            .find(|r| r.transformation == *id)
    }

    /// The bytes AC-039 compares (42 §2.1, through gx-canon and nothing else -- 41 §6).
    ///
    /// # Errors
    /// [`crate::Error::Canon`] if Σ has no canonical form.
    pub fn canonical_bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(cbor::encode(self)?)
    }
}

/// Rebuild Σ from journal records: **a pure function, and the whole of FR-039's replay**.
///
/// > **E-M5-2**: replay is a read-only operation that reconstructs Σ only…it never calls the adapter
/// > (sem: SEM-gx-engine-322)
///
/// It takes records and returns a value. There is no substrate to reach, no clock to read and no
/// randomness to draw: "replaying with the same seed/clock is bit-equal" (sem: SEM-gx-engine-323) (32 FR-039) holds here for a
/// reason stronger than care, which is that the seed and the clock **are in the records** -- the
/// seed in `DraftCreated`, the clock in every record's `at`. The `TransformationId` carries no
/// clock: gx-canon's `TransformationView` **excludes** `created_at` (42 §1.3 row 3, ASM-4), so the
/// same transformation written down tomorrow keeps the same identity. (An earlier version of this
/// sentence claimed the id carried the clock through `CompositionMetadata.created_at`; req/176 §7
/// measured the opposite and req/177 repaired it.) A replay does not re-inject them; it reads what
/// was injected.
///
/// # The rules, one per record
///
/// Each arm is 43 §3's own reading of its journal cell, and two of them carry a judgement worth
/// naming:
///
/// * A `Verdict` with `fail_posture_engaged = true` is **T-4e**, where the gate was never asked. The
///   row it produces is `Admitted` with **no verdict**, `enforced = false` and the flag set -- which
///   is what the live engine holds after the same transition, and what INV-S5 requires to stay
///   visible.
/// * `Superseded` writes T-12's edge in two places at once: the original's state, and the escrow
///   status `Consumed { by }`. **M5-09, adopted (a)** and **M5-16, adopted (a)** put both at T-12 "one place" (sem: SEM-gx-engine-324); firing
///   that transition is hand 6's, and reading a record that says it fired is this function's.
#[must_use]
pub fn reconstruct(records: &[EngineJournalRecord]) -> Sigma {
    let mut shadow = SigmaShadow::default();
    for record in records {
        shadow.fold(record);
    }
    shadow.sigma()
}

/// 🔴 **T6 condition ① (Σ-shadow)** — [`reconstruct`]'s fold, held open so that a process can keep
/// it (`req/38` §148 ruling 1(i), designed in `req/190` §2-1).
///
/// [`reconstruct`] is a fold over a record list, and until DR-43-2 the only way to have Σ was to run
/// the whole fold again. That is why a restarted `gx serve` answered `404` for a transformation its
/// own journal held (`req/182` H-02, measured): the engine's live table starts empty and nothing
/// carried the journal's answer across the restart. This type is the accumulator with a name and a
/// door: `Engine` keeps one, applies each record it appends, and reads rows out of it when its own
/// table has no body for them.
///
/// # Why it is the same code and not a second reading of the journal
///
/// [`reconstruct`] is *defined* as `SigmaShadow::default()` folded over the records, so the
/// incremental Σ and the full-fold Σ cannot disagree about an arm: there is one `match` in this file
/// and both roads run it. That is the functor law `reconstruct(rs ++ [r]) = fold(reconstruct(rs), r)`
/// made structural rather than tested (`req/190` §6, row 2) — a property test could only ever
/// re-measure a fact the construction already fixes. AC-039 is unaffected for the same reason: it
/// compares the engine's **live** Σ (built from the table, [`crate::pipeline::Engine::sigma`])
/// against this one, and neither side changed.
///
/// # What it cannot answer, and why that is not hidden
///
/// **Names and digests, never bodies** (ASM-9). A shadow row carries what [`StateRow`] carries — a
/// state, a verdict, an `enforced` flag, a canonical CID, a supersede edge — and no `Transformation`,
/// no `ObjectSnapshot`, no `PlannedDelta` body, no `EscalationTicket` and no `Receipt`. A caller that
/// needs one of those still gets `None` after a restart, which is 44's honest answer rather than a
/// reconstructed one: `GET /transformations/{id}` answers `200` with `transformation: null` instead
/// of `404`, because "the state is known and the body is not held here" is true and "this engine has
/// never heard of it" was not. Rebuilding the bodies is L2 (`req/190` §4-1, lane R2).
#[derive(Clone, Debug, Default)]
pub struct SigmaShadow {
    drafts: BTreeMap<IntentId, u64>,
    rows: BTreeMap<TransformationId, StateRow>,
    escrow: BTreeMap<TransformationId, EscrowRow>,
    ledger: BTreeMap<TransformationId, u64>,
    /// 🔴 **R3 / `req/222` H-04** — when each row entered the state it is in, **beside**
    /// Σ rather than inside it.
    ///
    /// 43 T-6 measures its two deadlines from "the state was entered", and until R3 the only holder
    /// of that moment was `Entry.since` in the live table — which `Engine::open` leaves empty. So a
    /// restart deleted every deadline in the project: `req/222` H-04 measured a sweep expiring
    /// **0** of 200 rows whose TTL was 1 ms, and a row past its deadline answering `verify` 200 and
    /// `commit` 200 afterwards. 43 §9.1.1's "INV-L1/L2 are enforced" was true inside one process
    /// lifetime and false across a restart, which is the most ordinary thing an operator does.
    ///
    /// # Why it is a side map and not a field of [`StateRow`]
    ///
    /// [`Sigma::canonical_bytes`] is what AC-039 compares, and Σ is built from [`StateRow`]s. A
    /// `since` inside the row would move both sides of AC-039 on the same day and make the repair a
    /// Σ change (`req/221` §7(a) costed exactly that). It is not needed there either: Σ is the
    /// **state** the journal witnesses, and this is a *derived* reading of the same records —
    /// the `at` of the record that last moved the row. [`SigmaShadow::sigma`] does not read it, so
    /// Σ's bytes are bit-for-bit what they were before R3.
    since: BTreeMap<TransformationId, Timestamp>,
}

impl SigmaShadow {
    /// The row this shadow holds about `id`, if any record has named it.
    #[must_use]
    pub fn row(&self, id: &TransformationId) -> Option<&StateRow> {
        self.rows.get(id)
    }

    /// 🔴 **R3 / `req/222` H-04** — when the journal says this row entered the state it is in.
    ///
    /// The `at` of the record that last moved it, which is 43 T-6's "the state was entered" read
    /// out of the log rather than out of a process's memory. `None` for a row no record has named
    /// and for one whose records never set a state (a `DraftCreated` alone).
    #[must_use]
    pub fn since_of(&self, id: &TransformationId) -> Option<Timestamp> {
        self.since.get(id).copied()
    }

    /// The escrow row this shadow holds about `id`, if one was escrowed.
    #[must_use]
    pub fn escrow_of(&self, id: &TransformationId) -> Option<&EscrowRow> {
        self.escrow.get(id)
    }

    /// 🔴 **R9 / `req/236` H-02** — every escrow row the journal witnessed, in `TransformationId`
    /// order.
    ///
    /// The plural of [`SigmaShadow::escrow_of`], and the input `Engine::sigma` needs so that the
    /// census `gx repair` runs and the answer `Engine::inverse_status` gives come from the same
    /// place. See `Engine::escrow_view` for what having two of them cost.
    pub fn escrow_rows(&self) -> impl Iterator<Item = &EscrowRow> {
        self.escrow.values()
    }

    /// Every transformation any record has named, in `TransformationId` order.
    pub fn transformation_ids(&self) -> impl Iterator<Item = &TransformationId> {
        self.rows.keys()
    }

    /// The ledger sequence the journal witnessed for `id`, if it committed.
    #[must_use]
    pub fn ledger_seq(&self, id: &TransformationId) -> Option<u64> {
        self.ledger.get(id).copied()
    }

    /// Every `(transformation, ledger_seq)` the journal witnessed, in `TransformationId` order.
    pub fn committed(&self) -> impl Iterator<Item = (&TransformationId, &u64)> {
        self.ledger.iter()
    }

    /// How many transformations it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Σ, as [`reconstruct`] would have built it from the same records.
    #[must_use]
    pub fn sigma(&self) -> Sigma {
        Sigma::new(
            self.drafts
                .iter()
                .map(|(intent_id, rng_seed)| DraftRow {
                    intent_id: *intent_id,
                    rng_seed: *rng_seed,
                })
                .collect(),
            self.rows.values().cloned().collect(),
            self.escrow.values().copied().collect(),
            self.ledger
                .iter()
                .map(|(transformation, ledger_seq)| CommittedRow {
                    transformation: *transformation,
                    ledger_seq: *ledger_seq,
                })
                .collect(),
        )
    }

    /// Fold one record in — 43 §3's own reading of its journal cell, one arm per record.
    ///
    /// See [`reconstruct`] for the two arms that carry a judgement worth naming (`Verdict` under an
    /// engaged fail-open posture, and `Superseded` writing T-12's edge in two places).
    #[allow(clippy::too_many_lines)]
    pub fn fold(&mut self, record: &EngineJournalRecord) {
        let Self {
            drafts,
            rows,
            escrow,
            ledger,
            since,
        } = self;
        // 🔴 **R3 / `req/222` H-04** — 43 T-6's clock, read here and in one place.
        //
        // The rule is `Engine::set_state`'s, one layer over: `since` moves when the **state**
        // moves, and the value is the `at` of the record that moved it. Written as a
        // before/after comparison rather than as a line in each of the eleven arms that assign a
        // state, because an arm added tomorrow would otherwise be an arm that silently keeps an
        // old deadline.
        let named = record.transformation();
        let state_before = named.and_then(|id| rows.get(&id)).and_then(|r| r.state);
        {
            // Every record but one is about a transformation, and `transformation()` is where E-M5-3
            // lives in the type system -- so the row is fetched once, here, rather than in twelve arms.
            let row = record
                .transformation()
                .map(|id| rows.entry(id).or_insert_with(|| StateRow::about(id)));

            match record {
                EngineJournalRecord::DraftCreated {
                    intent_id,
                    rng_seed,
                    ..
                } => {
                    drafts.insert(*intent_id, *rng_seed);
                }
                EngineJournalRecord::Planned {
                    intent_id,
                    delta_cid,
                    fp0,
                    ..
                } => {
                    let row = row.expect("`Planned` names a transformation");
                    row.intent_id = Some(*intent_id);
                    row.delta_cid = Some(*delta_cid);
                    row.fp0 = Some(fp0.clone());
                    row.state = Some(Lifecycle::Candidate);
                }
                EngineJournalRecord::VerifyStarted { .. } => {
                    let row = row.expect("`VerifyStarted` names a transformation");
                    row.state = Some(Lifecycle::Verifying);
                }
                EngineJournalRecord::Verdict {
                    kind,
                    verdict_digest,
                    fail_posture_engaged,
                    ..
                } => {
                    let row = row.expect("`Verdict` names a transformation");
                    row.fail_posture_engaged = *fail_posture_engaged;
                    row.verdict_digest = *verdict_digest;
                    if *fail_posture_engaged {
                        // T-4e: 43 §4 degrades this transformation to "the equivalent of record-only
                        // mode" (sem: SEM-gx-engine-325) and no
                        // gate ran, so there is no verdict to hold.
                        row.verdict = None;
                        row.enforced = false;
                        row.state = Some(Lifecycle::Admitted);
                    } else {
                        row.verdict = Some(*kind);
                        row.state = Some(state_of_verdict(*kind));
                    }
                }
                EngineJournalRecord::HumanDecision {
                    kind,
                    verdict_digest,
                    ..
                } => {
                    let row = row.expect("`HumanDecision` names a transformation");
                    row.verdict = Some(*kind);
                    // 🔴 **DR-46-31** — the ruling's own digest, where the record carries one.
                    //
                    // `if let` and not an assignment: a pre-DR-46-31 journal has no such field, and
                    // overwriting the seat with `None` would invent a third state ("a person ruled
                    // and no proof exists") for records that were written before the seat existed.
                    // Leaving T-4c's `Escalate` digest in place is what those journals have always
                    // replayed to, and `Engine::reissue_receipt` refuses them exactly as it did —
                    // the old degradation, kept honest rather than papered over.
                    if let Some(digest) = verdict_digest {
                        row.verdict_digest = Some(*digest);
                    }
                    row.state = Some(state_of_verdict(*kind));
                }
                EngineJournalRecord::Canonicalized {
                    canonical_cid,
                    enforced,
                    ..
                } => {
                    let row = row.expect("`Canonicalized` names a transformation");
                    row.canonical_cid = Some(*canonical_cid);
                    if *enforced == Some(false) {
                        row.enforced = false;
                    }
                    row.state = Some(Lifecycle::Canonicalized);
                }
                EngineJournalRecord::CommittingStarted { .. } => {
                    let row = row.expect("`CommittingStarted` names a transformation");
                    row.state = Some(Lifecycle::Committing);
                }
                EngineJournalRecord::ProvenanceDerived { provenance, .. } => {
                    let row = row.expect("`ProvenanceDerived` names a transformation");
                    row.provenance = Some(provenance.clone());
                }
                EngineJournalRecord::InverseEscrowed {
                    transformation,
                    inverse_cid,
                    pending,
                    undetermined,
                    ..
                } => {
                    escrow.insert(
                        *transformation,
                        EscrowRow {
                            transformation: *transformation,
                            inverse_cid: *inverse_cid,
                            retained_until: None,
                            // 🔴 **E-M5-9**: the record now says which of 42 §3.12's two openings this
                            // was. A CID is an inverse that was built and held (`Available`); its
                            // absence is `adapter.invert` having answered `None`, which 42 §3.12 spells
                            // `Unavailable` -- "the case where `invert()` returns None (cannot be
                            // constructed)" (sem: SEM-gx-engine-326). The two are
                            // one record apart and must not be one status apart, or a replay of a
                            // commit that could never be undone would report an undo as available.
                            //
                            // 🔴 Two-phase escrow (`req/38` §99, ruling 2, clause ①; sem: SEM-gx-engine-327): a `pending` flag on a held
                            // CID is a **partial** escrow — `Pending`, never `Available`, or a crash
                            // between escrow and completion would replay as an undo guarantee nobody
                            // finished constructing. A pending flag with no CID cannot be written by
                            // this crate; a foreign journal spelling it is folded to the honest side.
                            //
                            // 🔴 **DR-46-26** — and `None` now has **two** preimages. E-M5-9's rule
                            // above was complete while `SubstrateAdapter::invert` returned two
                            // values; the widened declaration gives the absence a second meaning,
                            // "the prior would not be read and this deployment said it would rather
                            // have the effect", which 42 §3.12 spells `Undetermined`. A replay that
                            // folded the two would answer "there is no undo" about a change nobody
                            // established anything about — DR-46-13's defect, arriving on the
                            // recovery road. The flag is `false` in every journal written before
                            // this lane, so those replay exactly as they did.
                            //
                            // `true` beside a **held** CID cannot be written by this crate (an
                            // inverse was constructed, so C-25 answered `True`); a foreign journal
                            // spelling it is folded to the CID's own side, which is the same
                            // fail-honest direction the `pending` arm above takes.
                            status: match (inverse_cid, pending, undetermined) {
                                (Some(_), false, _) => InverseStatus::Available,
                                (Some(_), true, _) => InverseStatus::Pending,
                                (None, _, true) => InverseStatus::Undetermined,
                                (None, _, false) => InverseStatus::Unavailable,
                            },
                        },
                    );
                }
                EngineJournalRecord::ApplyStarted { delta_cid, .. } => {
                    let row = row.expect("`ApplyStarted` names a transformation");
                    row.apply_started = Some(*delta_cid);
                }
                EngineJournalRecord::ApplyObserved {
                    observation_cid, ..
                } => {
                    let row = row.expect("`ApplyObserved` names a transformation");
                    row.observation_cid = Some(*observation_cid);
                }
                EngineJournalRecord::InverseCompleted {
                    transformation,
                    inverse_cid,
                    ..
                } => {
                    // Two-phase escrow's outcome record: the `Pending` row leaves the pending state
                    // the way the live engine left it — completed (`Available`, the finished CID) or
                    // folded (`Unavailable`, no CID), one record and one rule (§99, ruling 2, clause ④; sem: SEM-gx-engine-328).
                    escrow.insert(
                        *transformation,
                        EscrowRow {
                            transformation: *transformation,
                            inverse_cid: *inverse_cid,
                            retained_until: None,
                            status: match inverse_cid {
                                Some(_) => InverseStatus::Available,
                                None => InverseStatus::Unavailable,
                            },
                        },
                    );
                }
                EngineJournalRecord::Committed {
                    transformation,
                    ledger_seq,
                    ..
                } => {
                    let row = row.expect("`Committed` names a transformation");
                    row.state = Some(Lifecycle::Committed);
                    ledger.insert(*transformation, *ledger_seq);
                }
                EngineJournalRecord::Aborted {
                    reason, rollback, ..
                } => {
                    let row = row.expect("`Aborted` names a transformation");
                    row.state = Some(Lifecycle::Aborted(*reason));
                    row.rollback = *rollback;
                }
                EngineJournalRecord::Superseded {
                    transformation, by, ..
                } => {
                    let row = row.expect("`Superseded` names a transformation");
                    row.state = Some(Lifecycle::Superseded);
                    row.superseded_by = Some(*by);
                    if let Some(held) = escrow.get_mut(transformation) {
                        held.status = InverseStatus::Consumed { by: *by };
                    }
                }
            }
        }
        if let Some(id) = named {
            let state_after = rows.get(&id).and_then(|r| r.state);
            if state_after != state_before {
                since.insert(id, record.at());
            }
        }
    }
}

/// Which state a verdict lands a transformation in (43 T-4a/T-4b/T-4c, and T-5/T-5b for a human).
const fn state_of_verdict(kind: VerdictKind) -> Lifecycle {
    match kind {
        VerdictKind::Admit => Lifecycle::Admitted,
        VerdictKind::Deny => Lifecycle::Denied,
        VerdictKind::Escalate => Lifecycle::Escalated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing in, nothing out — and no torn tail, which is the difference between an empty journal
    /// and a damaged one.
    #[test]
    fn an_empty_journal_replays_to_nothing_and_reports_no_damage() {
        let out = replay(&[]);
        assert_eq!(out.records().len(), 0);
        assert_eq!(out.recovery(), Recovery::default());
        // 🔴 **R32 / `req/392` M-01** — and the framing it reports is the framing on the disk,
        // which is none. Before R32 this answered `ChainedV2` about a file carrying no marker.
        assert_eq!(
            out.format(),
            JournalFormat::Legacy,
            "zero bytes carry no marker, and this function sniffs the marker"
        );
        assert_eq!(out.chain(), None, "and a file with no marker has no chain");
    }

    /// A header claiming more than the ceiling is refused before the allocation it asks for
    /// (M5-20's ceiling, from the read side).
    #[test]
    fn a_header_over_the_ceiling_stops_the_replay() {
        let mut bytes = (MAX_RECORD_BYTES + 1).to_be_bytes().to_vec();
        bytes.extend_from_slice(&[0u8; 8]);
        let out = replay(&bytes);
        assert_eq!(out.records().len(), 0);
        assert_eq!(out.recovery().torn_tail_bytes, bytes.len() as u64);
    }
}
