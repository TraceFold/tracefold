// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The engine's own records: the write-ahead journal (42 §3.13) and the escrowed inverse
//! (42 §3.12).
//!
//! Spec: 42 §3.12 and §3.13 for the field tables, 43 §3 for the journal vocabulary and §7 for the
//! ordering that makes it a *write-ahead* log, 41 §6 for "every canonical encode goes through
//! gx-canon only" (sem: SEM-gx-engine-329).
//! **E-M5-1** and **E-M5-3** (`req/38_ERRATA_2026-08-07.md` §37) are the two rulings that move this
//! file away from 42 §3.13's literal text, and both are implemented rather than commented on.
//!
//! # Journal-first is an ordering, not a data structure
//!
//! 43 §7: every transition is written to the journal **before** the side effect it describes. What
//! makes that true here is three statements in [`EngineJournal::append`] and their order: encode,
//! write-and-fsync, then push to the in-memory vector. A reader who wants to check the property
//! reads those three lines; a caller who wants to violate it cannot, because the vector is private
//! and the only road to it is through the barrier.
//!
//! # No transition is implemented
//!
//! This file defines what a transition *records*. Which transition may fire, under what guard, and
//! with what side effect is 43 §3 and is M5 hands 2 through 6. Nothing here reads a state and
//! decides a next one -- there is no state table in this hand at all, which is **M5-17, adopted (b)**'s
//! shape from the other side: "the Draft phase is held only by the journal; the state table starts
//! at Candidate" (sem: SEM-gx-engine-330). The journal
//! exists first because the journal is what the table will be rebuilt from.

use core::fmt;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize};

use gx_canon::cbor;
use gx_canon::cid::IdentityView;
use gx_core::{
    AbortReason, Actor, BoundaryStage, Cid, Fingerprint, IntentId, ReadEntry, SubstrateKind,
    Timestamp, TransformationId, VerdictKind,
};
use gx_log::Recovery;
use gx_substrate::PlannedDelta;
use gx_witness::Provenance;

use crate::replay::{
    replay, EscrowRow, JournalCreation, JournalFormat, CHAIN_BYTES, JOURNAL_MAGIC_V2, LENGTH_BYTES,
};
use crate::{io_error, Error, Result};

/// The largest journal record this crate will write or allocate for.
///
/// A journal record is a handful of ids, a timestamp and -- in `Planned` -- one [`Fingerprint`],
/// whose scope is already bounded by `gx_core::MAX_SCOPE_BYTES` (1 KiB). So this is three orders of
/// magnitude of headroom rather than a tuning parameter. It exists because the length header is
/// read from a file that may have been damaged: without a ceiling, four corrupted bytes ask for a
/// four-gigabyte allocation before anything has had a chance to refuse them.
///
/// **This is the first of M5-20's ceilings.** §37's ruling is "one pre-decode byte ceiling per
/// engine receiving mouth, and a 1:1 probe against the contract row" (sem: SEM-gx-engine-331); the journal is one receiving mouth and the blob store that hand 3
/// builds is the other. Deliberately a separate constant from `gx_log::MAX_RECORD_BYTES` even
/// though the number is the same: two files with different contents and different writers should
/// be able to move independently, and sharing the constant would make one ceiling a statement
/// about the other. The number being equal is a coincidence a probe is allowed to notice; the
/// *rule* is not shared.
pub const MAX_RECORD_BYTES: u32 = 1 << 20;

// ---------------------------------------------------------------------------
// 42 §3.13 -- the journal vocabulary
// ---------------------------------------------------------------------------

/// A [`Fingerprint`] in a form that survives being written down.
///
/// [`Fingerprint`] has no serde face. That is deliberate in gx-core: it is built through a checked
/// constructor that refuses a scope over `gx_core::MAX_SCOPE_BYTES`, and a derived `Deserialize`
/// would be a second door into the type that skips the check. 42 §3.13 nevertheless puts one inside
/// `Planned`, and a write-ahead journal has to be able to write what it holds. So the record
/// carries the three fields and hands them back **through the constructor**
/// ([`FingerprintRecord::into_fingerprint`]) -- which is E-6's rule ("reading a value back requires
/// a checked constructor" (sem: SEM-gx-engine-332)) applied one type over.
///
/// # No `PartialEq`, on purpose
///
/// **E-M4-15** took `PartialEq` off `Fingerprint` because 42 §3.5's comparison has three answers --
/// `Ok(true)`, `Ok(false)` and the two refusals `cas_eq` types -- and `==` has two. A mirror struct
/// with a derived `PartialEq` would hand that third answer back to any caller who reached for
/// `record.fp0 == other.fp0`, undoing the ruling through a type that did not exist when it was
/// made. So this struct has none, [`EngineJournalRecord`] has none because it holds one, and the
/// round-trip property in `tests/journal_roundtrip.rs` is stated on **canonical bytes** instead: a
/// sequence read back encodes to the bytes it was read from. That is the stronger statement anyway
/// -- byte equality implies field equality for a canonical form (42 §2.1), and the converse is what
/// a derived comparison would have been asserting.
///
/// Raised as **M5H1-4**: the alternative is a checked `Deserialize` on `Fingerprint` itself, which
/// would delete this struct. That is a gx-core change, and §37 admits exactly two new gx-core types
/// to this hand, so it is raised rather than taken.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerprintRecord {
    substrate: SubstrateKind,
    scope: String,
    digest: Cid,
}

impl FingerprintRecord {
    /// Project a fingerprint into the form that can be written down.
    #[must_use]
    pub fn of(fingerprint: &Fingerprint) -> Self {
        Self {
            substrate: fingerprint.substrate().clone(),
            scope: fingerprint.scope().to_string(),
            digest: *fingerprint.digest(),
        }
    }

    /// Rebuild the fingerprint, through the constructor that checks the scope bound.
    ///
    /// # Errors
    /// Whatever `Fingerprint::new` refuses -- a scope over `gx_core::MAX_SCOPE_BYTES`, which is how
    /// a journal written by something that is not this code is stopped at the door rather than
    /// after it.
    pub fn into_fingerprint(self) -> gx_core::Result<Fingerprint> {
        Fingerprint::new(self.substrate, self.scope, self.digest)
    }

    /// The substrate the fingerprint names.
    #[must_use]
    pub fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    /// The scope the fingerprint covers (42 §3.5).
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// The digest under that scope.
    #[must_use]
    pub fn digest(&self) -> &Cid {
        &self.digest
    }
}

/// What became of 43 T-10c's best-effort rollback (**AC-038**, hand 4).
///
/// # Why this exists at all
///
/// AC-038, verbatim: "an automatic rollback attempt occurs, the result is recorded as
/// `Aborted(ApplyFailed)`, and **journal/Receipt records whether a rollback attempt occurred
/// (succeeded/failed)**" (sem: SEM-gx-engine-333). Neither seat exists. 42 §3.10's
/// `ReceiptPayload` has no field for it, and ASM-14 issues no receipt at all for an `Aborted`
/// transformation -- there are two kinds and both belong to a verdict or to a commit that succeeded.
/// That leaves the journal, and 42 §3.13's `Aborted` row is three fields none of which is this.
///
/// So the fact is carried where 43 T-10c itself puts the record ("journal: `Aborted{id,
/// ApplyFailed}`" (sem: SEM-gx-engine-334)) and the row gains a field, which is exactly the shape **M5H2-1 / E-M5-7** took
/// for `Verdict`: an implementation cannot satisfy an acceptance criterion out of a table that has
/// no room for it, so it writes the true thing and raises the divergence. Raised as **M5H4-2**.
///
/// # Four states, not three
///
/// `Option<Rollback>` and not `Rollback`: an abort that is not T-10c has no rollback question to
/// answer, and `None` says so. [`Rollback::NotAttempted`] is the different fact that the question
/// arose and the answer was no -- 43 T-10c's guard is "**if** an escrowed inverse exists" (sem: SEM-gx-engine-335) -- which is
/// req/29 §4's "do not give skip and pass the same face" at the one place a v0.1 would blur it.
///
/// 🔴 ~~`NotAttempted` is **unreachable in v0.1** and is named anyway. `SubstrateAdapter::invert`
/// returning `None` makes a transformation `Escalated` at T-3 (**E-M3-4**), so nothing without an
/// escrowed inverse reaches `Committing` until hand 6 resolves an escalation. The same shape as
/// `InverseStatus::Expired`, which 42 §3.12 lists and v0.1 never writes.~~
///
/// L-02 (`req/182` §1-3 L-06, `req/189`, `req/38` §150 doc-integration): the struck paragraph is
/// stale -- `NotAttempted` **is** reachable in v0.1, and `pipeline.rs` constructs it in two
/// places downstream of a `Committing` row (not `Escalated`). First, T-10c's guard reads an
/// escrowed inverse that is still `Pending` (its do-time member unresolved, and an apply failure
/// leaves no observation to resolve it from) as no constructible inverse and skips the attempt:
/// `Some((_, _, true)) => Rollback::NotAttempted`. Second, M5H5-5's recovery path skips it on
/// purpose when the escrowed inverse would undo an apply the ledger may already hold --
/// `Some(Rollback::NotAttempted)` at the `RecoveryPath::ApplyWasAnnounced` arm. The struck
/// sentence's premise (nothing without an escrowed inverse reaches `Committing`) was true of
/// T-3's gate; it did not anticipate these two downstream paths, both reachable in v0.1.
///
/// 🔴 **R43 / `req/318` §(c), ruling `req/38` §350 item 7** — what a reader actually receives is
/// **this one word**: `crates/gx-cli/src/lifecycle.rs:641` and `crates/gx-cli/src/pipeline.rs:431`
/// and `:638` all put `engine.rollback(id)` straight into the `detail` key, so which tool refused
/// the compensation and why is in the journal and nowhere in the answer. Registered here rather
/// than repaired: the fix is an engine-window change and is a separate lane's (KA-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rollback {
    /// No inverse was escrowed, so 43 T-10c's guard did not open. Unreachable in v0.1.
    NotAttempted,
    /// The escrowed inverse was handed to the adapter, the adapter accepted it, **and the object
    /// was then read back and found at the fingerprint the transformation started from**.
    ///
    /// 🔴 **R29 / `req/361` H-01** — the second half of that sentence is new. Until this lane the
    /// word meant only *the adapter accepted it*, and the twenty-eighth audit drove a contract-
    /// conforming adapter whose `apply` fails halfway to show what that costs: the forward apply
    /// stopped mid-way (`A B C D` → `A B D`), the escrowed inverse was then applied **honestly and
    /// in full**, and the world ended at `A B D C D` — a record duplicated, the object further from
    /// where it started than when the roll-back began — with `Succeeded` printed over it. The
    /// adapter had not lied; the engine had asked the wrong question. `Succeeded` was a word about
    /// a **call**, and it was being read as a word about the **world**.
    Succeeded,
    /// The escrowed inverse was handed to the adapter and the adapter refused — **or** it accepted
    /// and the read-back that would confirm the object is home could not be taken at all. 43 T-10c
    /// is "best-effort, moves on regardless of the outcome" (sem: SEM-gx-engine-336), so this does not change the abort reason.
    ///
    /// 🔴 **R29 / `req/361` H-01** — the second road into this word is new, and it is deliberately
    /// **not** a fourth spelling. `crates/gx-cli/src/wrap.rs`'s arm for this value has said since
    /// R25 that "this adapter's apply is the call together with the read-back of it, so the error
    /// can be either one, and a compensation whose bytes landed and whose read-back died lands here
    /// too" — a snapshot or `precondition` that will not answer is exactly that case, one layer up.
    /// The sentence a reader gets was already true for it before this lane made the road explicit.
    Failed,
    /// 🔴 **R29 / `req/361` H-01** — the adapter accepted the escrowed inverse **and the object is
    /// not back where it started**: it was read again after the roll-back and its fingerprint is
    /// not `fp0`.
    ///
    /// # Why a fourth word and not a truer `Failed`
    ///
    /// Because the three worlds a reader has to tell apart are three, not two. After a failed
    /// apply the object can be (a) back where it was, (b) somewhere else because the roll-back was
    /// refused, or (c) somewhere else **because the roll-back itself moved it** — the case the
    /// twenty-eighth audit produced on disk. Folding (c) into `Failed` would tell an operator the
    /// compensation did not run when it ran completely, and folding it into `Succeeded` is the
    /// defect this word exists to close. `req/361` §9-2 asked for exactly this: *the world can be
    /// three ways and the vocabulary has two words for it.*
    ///
    /// # What it does **not** say
    ///
    /// It does not say the roll-back caused the difference. A third party writing to the same
    /// object inside the window lands here too, and this engine cannot tell the two apart from one
    /// fingerprint — `docs/LIMITS.md` declares that residue rather than arguing it away, in the
    /// same form R8 used for the forward CAS ("It is **not** atomicity").
    Diverged,
}

/// 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — why [`Rollback::NotAttempted`] was reached.
///
/// # The defect this exists to close
///
/// `Rollback` has three values and the proxy writes one sentence per value. R25 built that
/// arm-per-value shape after the twenty-fourth audit measured a clause that was false on one of
/// them — and then the `NotAttempted` arm asserted a **cause**: *the escrowed one was still partial
/// — a member of it is filled from what the call answers*. This engine constructs that value at
/// **three** places, and the clause describes one of them. On the other two an agent was handed a
/// confident, specific account of an escrow row that does not exist.
///
/// One value, three facts, and a sentence keyed on the value alone can be right about at most one.
/// So the cause travels beside the value.
///
/// 🔴 **R30 / `req/372` M-01** — **five** facts since this lane, and the arithmetic is the whole
/// argument for the shape: the two new causes are two more roads to the same value, and adding
/// them cost one arm each on the proxy instead of a fourth `Rollback` word. That the list can grow
/// this cheaply is the property `req/324` §5(d) was buying.
///
/// # 🔴 Why this is deliberately **not** a component of Σ
///
/// It is an annotation on *this process's* account of what happened, not a fact the state is
/// reconstructed from: `Entry`'s fields each have an arm in [`crate::replay::reconstruct`] because
/// they are Σ, and a fourth `Rollback` value or a journal field would be a schema change to a
/// record that is already written and already signed. Nothing here decides anything — the abort
/// reason, the verdict and the receipt are all exactly what they were.
///
/// The consequence is stated rather than hidden: after a restart the cause is **absent**, and the
/// proxy's arm for a cause it does not recognise is the honest answer there — it says what the
/// value itself carries and declines to put words in this engine's mouth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotAttemptedBecause {
    /// 43 T-10c's guard asks for an inverse this build can execute and the escrowed one was still
    /// `Pending`: a member of it is filled from what the call answers, and the failed apply left no
    /// answer to fill it from.
    EscrowStillPartial,
    /// There was no escrowed inverse at all — `SubstrateAdapter::invert` answered `None`. E-M3-4
    /// escalates such a transformation and T-5 is what lets a person approve one anyway, so the
    /// road exists and does not run through an escrow row.
    NoInverseWasEscrowed,
    /// `gx repair`'s `RecoveryPath::ApplyWasAnnounced`: the announcement was on disk and nothing
    /// was rebuilt, so there was never an inverse in this process to attempt.
    RecoveredWithoutRebuilding,
    /// 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the forward `apply` failed **without
    /// moving the object at all**, so there was no effect to compensate and the escrowed inverse
    /// was not sent.
    ///
    /// # This is the audit's worst shape, closed at its root
    ///
    /// `req/372` §2's fourth rebuttal, verbatim in its consequence: *a transformation that did
    /// nothing erases a third party's write and nothing else*. When an `apply` is refused before
    /// it touches anything — a permission denial, a policy refusal at the far end — the world is
    /// exactly where the transformation found it. Until this lane the engine sent the escrowed
    /// inverse anyway, on 43 T-10c's best-effort reading, and an **absolute** inverse sent into a
    /// world somebody else has since written to overwrites them and reports `Succeeded`.
    ///
    /// The read that establishes this is taken the instant the forward `apply` answers `Err` —
    /// before anyone else can have reacted to it — so this is a measurement of **our own call**,
    /// not a guess about who else is out there.
    ///
    /// # Nothing is given up by declining
    ///
    /// An absolute inverse applied to a world at `fp0` is a no-op, so the old behaviour bought
    /// nothing on this road. A **relative** inverse applied to it is worse than a no-op — a
    /// `{remove C, remove D}` against a world that never received `C` or `D` corrupts it — so
    /// declining is the only correct answer for both grammars rather than a trade between them.
    ///
    /// # What it does not claim
    ///
    /// A fingerprint is an equality, not a witness of inaction: an `apply` that moved the object
    /// and moved it back, or one whose effect the scope of `fp0` does not cover, is reported here
    /// too. `docs/LIMITS.md` carries that residue rather than arguing it away.
    WorldNeverMoved,
    /// 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the forward `apply` moved the object,
    /// and then **somebody else moved it again** before the compensation could run, so the
    /// escrowed inverse was **not** sent.
    ///
    /// # The defect this word exists to close
    ///
    /// The escrowed inverses the four shipped adapters mint are **absolute**: the twenty-ninth
    /// audit drove all of them and found no payload in any shipped grammar whose effect is a
    /// function of the state it starts from (`req/372` §1). An absolute inverse restores from
    /// *any* world — which is exactly what makes it dangerous here, because "any world" includes
    /// **a world somebody else legitimately created**. The audit measured it on a real branch:
    ///
    /// ```text
    /// A29_GIT_THIRD_PARTY head prior=de05de3 theirs=d2d09b5 after_rollback=de05de3
    /// A29_GIT_THIRD_PARTY word=Succeeded their_commit_is_still_the_tip=false
    /// ```
    ///
    /// A colleague's commit, off the branch, with `Succeeded` printed over it. The word was not
    /// lying — the object *was* back at `fp0` — and that is the point: `fp0` is a statement about
    /// this transformation's object, and it says nothing about whose work was standing on it.
    ///
    /// The worst shape is the audit's own fourth rebuttal, and it is the reason this is a refusal
    /// rather than a report: when the forward `apply` fails **without changing the world**, there
    /// is no effect to compensate at all — and the engine still sent the inverse. A
    /// transformation that did nothing would erase a third party's write and nothing else.
    ///
    /// # 🔴 Why the guard is on where the apply **left** the object, not on `fp0`
    ///
    /// Because *somebody else wrote* and *our own apply landed* are the same observation when the
    /// only thing compared is `fp0` — in both the object is simply not where the transformation
    /// started — and a guard that cannot tell them apart has to sacrifice one of them. The first
    /// draft of this repair guarded on `fp0` and sacrificed the wrong one: the ordinary case this
    /// entire road exists for, a call that **landed and then errored**, stopped being compensated.
    /// `crates/gx-cli/tests/r29_rollback_is_verified.rs`'s negative control caught it in one run.
    ///
    /// So the engine reads twice. The first read, taken the instant the forward `apply` fails,
    /// establishes where our own call left the object. The second, taken immediately before the
    /// compensation, asks whether it is still there. **This word is the second read disagreeing
    /// with the first** — which is a third party, because our own apply had already finished when
    /// the first read was taken. The information that separates the two cases is *time*, not
    /// fingerprints.
    ///
    /// # What it does not claim
    ///
    /// It does not name who wrote, and it cannot: a second writer of the operator's own, another
    /// agent and a CI job are one observation here. It also inherits the fingerprint's coarseness
    /// — an object one byte from where it was and an unrecognisable one give the same answer.
    ///
    /// # The residue, declared rather than argued away
    ///
    /// This is a compare-and-set spelled as two calls, so there is a window between the second
    /// read and the `apply`, and a third party who writes inside it is still overwritten — as is
    /// one who writes *during* the inverse's own apply. It is the same residue R8's forward CAS
    /// declares, in the same words, and `docs/LIMITS.md` v0.5-q carries its **measured** width on
    /// both fs and mcp rather than an adjective.
    WorldMovedBeneath,
    /// 🔴 **R30 / `req/372` M-01** — the read that would decide the line above could not be taken
    /// at all: `snapshot` or `precondition` refused, or the two fingerprints were not comparable.
    ///
    /// Deliberately **not** folded into [`NotAttemptedBecause::WorldMovedBeneath`]. "I looked and
    /// it had moved" and "I could not look" are different facts about the world, and a reader who
    /// is told the first when the second happened has been handed a confident account of an
    /// observation nobody made — the exact defect `req/324` §5(d) minted this whole type to close.
    /// Fail-closed is the same either way: an inverse is not sent into a substrate that will not
    /// say where it is.
    WorldCouldNotBeRead,
    /// 🔴 **R-1001-1 (`req/1001` §4, the else-arm of ruling D-999-F2, 2026-08-31)** — the forward
    /// `apply` **succeeded**, and the post-state digest the plan promised
    /// (`Transformation.target`) is not the post-state the apply itself reported
    /// (`AppliedDelta::resulting_digest`) — so the escrowed inverse was **not** sent.
    ///
    /// # The seventh fact, and why the other six cannot carry it
    ///
    /// Every cause above is a spelling of *the inverse was unavailable or unsafe to send*: the
    /// escrow was partial, or never built, or a repair rebuilt nothing, or the world never moved,
    /// moved beneath us, or could not be read. On this road none of that is true. The apply
    /// landed, the escrow is settled, the world was read and the two fingerprints compared — and
    /// the comparison is the problem: the engine has just measured that **its model of this
    /// object's post-state is wrong**. The escrowed inverse stands on exactly that model, and
    /// sending it on the strength of a model this abort exists to distrust is what fail-closed
    /// forbids. The inverse is *available*, and the engine *declines*.
    ///
    /// # What it guarantees
    ///
    /// The undo material survives: the two-phase escrow completes **before** this comparison is
    /// taken (`pipeline.rs` orders it so on purpose), so a mispredicted world is one an operator
    /// can still act on deliberately. The world holds what the apply put there — this is not a
    /// missing escrow and not a third-party write.
    ///
    /// # What it does **not** guarantee
    ///
    /// It does not say *which* side was wrong — a plan that promised badly and an adapter that
    /// answered badly are one observation here — and it does not say the object is back where it
    /// started, because nothing was taken back. `crates/gx-core/src/error.rs`'s
    /// `PostconditionMismatch` carries the abort's own account; this word is the roll-back's.
    PromisedPostStateWasWrong,
}

impl NotAttemptedBecause {
    /// The seven, in declaration order (six until R-1001-1 — the doc line below keeps the count's
    /// history). The proxy has one arm per entry and one for a word it does
    /// not know, the same shape [`Rollback::ALL_KINDS`] carries for the same reason.
    ///
    /// 🔴 **R30 / `req/372` M-01** — three until this lane. Unlike [`Rollback::ALL_KINDS`], this
    /// list is **not** a journal-schema fact: this type is an annotation on *this process's*
    /// account (see the type's own doc), it is not a component of Σ, and no journal record carries
    /// it. Adding to it costs an upgrader nothing.
    ///
    /// 🔴 **R-1001-1 (`req/1001` §4, D-999-F2, 2026-08-31)** — six until that ruling. The seventh
    /// is [`NotAttemptedBecause::PromisedPostStateWasWrong`], and the property R30 named above is
    /// the one being spent: adding it is one entry here, one `kind()` arm, one proxy arm.
    pub const ALL_CAUSES: [&'static str; 7] = [
        "EscrowStillPartial",
        "NoInverseWasEscrowed",
        "RecoveredWithoutRebuilding",
        "WorldNeverMoved",
        "WorldMovedBeneath",
        "WorldCouldNotBeRead",
        "PromisedPostStateWasWrong",
    ];

    /// Which of [`NotAttemptedBecause::ALL_CAUSES`] this is. No `_` arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            NotAttemptedBecause::EscrowStillPartial => "EscrowStillPartial",
            NotAttemptedBecause::NoInverseWasEscrowed => "NoInverseWasEscrowed",
            NotAttemptedBecause::RecoveredWithoutRebuilding => "RecoveredWithoutRebuilding",
            NotAttemptedBecause::WorldNeverMoved => "WorldNeverMoved",
            NotAttemptedBecause::WorldMovedBeneath => "WorldMovedBeneath",
            NotAttemptedBecause::WorldCouldNotBeRead => "WorldCouldNotBeRead",
            NotAttemptedBecause::PromisedPostStateWasWrong => "PromisedPostStateWasWrong",
        }
    }
}

impl Rollback {
    /// The four, in declaration order.
    ///
    /// 🔴 **R29 / `req/361` H-01** — three until this lane. The window that added the fourth is one
    /// row in `CHANGELOG.md` §3, because this value is serialised into the `Aborted` journal record
    /// and a word an older binary has never heard of is a journal-schema fact, not a cosmetic one.
    pub const ALL_KINDS: [&'static str; 4] = ["NotAttempted", "Succeeded", "Failed", "Diverged"];

    /// Which of [`Rollback::ALL_KINDS`] this is. No `_` arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Rollback::NotAttempted => "NotAttempted",
            Rollback::Succeeded => "Succeeded",
            Rollback::Failed => "Failed",
            Rollback::Diverged => "Diverged",
        }
    }
}

/// 🔴 **WM-2a (`req/1007` §4 item 2, `req/1010`)** — what the plan predicted the post-state would
/// be, what the apply measured it to be, and the moment the two were compared.
///
/// # Why the kept promise needs a value at all
///
/// [`NotAttemptedBecause::PromisedPostStateWasWrong`] gave the **broken** promise a word
/// (**R-1001-1**), and that left the arithmetic lopsided: a prediction that failed produced an
/// abort, a rollback account and a cause, while a prediction that **held** produced nothing at
/// all. A model whose hits are silent and whose misses are loud cannot be scored — every
/// commit that kept its promise was indistinguishable, from outside the engine, from a commit
/// that never made one. This value is the other half: the comparison is recorded whenever it is
/// **taken**, and which way it came out is a property of the record rather than the reason it
/// exists.
///
/// # `matched` is derived and not stored
///
/// [`PredictionOutcome::matched`] recomputes `predicted == observed` from the two digests the
/// record already carries. A stored flag would be a third place the same fact lives, free to
/// disagree with the two digests beside it; a derived one cannot go stale. The engine writes one
/// record at one site for both outcomes, so there is also no second write site for the two to
/// drift between.
///
/// # What `None` from [`Engine::prediction_outcome`] means — and what it does not
///
/// It is the **third value**, not a failure: no prediction was made (the adapter filled no
/// `promised_target`), or this process is not the one
/// that ran the commit. "The prediction was wrong" is `Some` with `matched() == false`, and
/// collapsing the two would be this workspace's own first principle broken in its own instrument
/// — the residue `docs/LIMITS.md` names for `NotAttemptedBecause` reads the same way here.
///
/// 🔴 **"which is what every shipped adapter does" was struck from the clause above** (WM-5a
/// Phase 1, `req/1011` §4, ruled by `req/1016`, 2026-09-01). It described the world on the day
/// `req/1010` landed and stopped being true one lane later: `gx-adapter-fs` and `gx-adapter-git`
/// now fill `promised_target` in production, so this accessor answers `Some` on the ordinary
/// road rather than only under a fixture. The clause is corrected in place because a doc comment
/// that has drifted from the implementation is not documentation, it is a claim — and this one
/// would have told a reader that `None` is the normal answer when it has become the exception.
///
/// # Deliberately not a component of Σ
///
/// The same standing [`NotAttemptedBecause`] argues for itself, and for the same reason: no
/// journal record carries this, nothing reads it to decide anything, and adding it costs an
/// upgrader nothing. Making it a journal record would be a **journal-schema** change — 42 §3.13's
/// enum, `JOURNAL_RECORD_KINDS`, `tests/journal_vocabulary.rs`'s fifteen and 47 §4's upgrade
/// precondition — which is a ruling of its own and is raised in `req/1010` §8 item 1 rather than
/// made here.
///
/// # The residue, declared rather than argued away
///
/// The map holding these grows with the transformations *this process* commits and is never
/// evicted — exactly the lifetime `not_attempted_because` already has, mirrored rather than
/// re-decided. A long-lived engine pays two digests and a timestamp per predicting commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PredictionOutcome {
    /// `Transformation.target` (41 §3), as `plan` fixed it — the prophecy.
    pub predicted: Cid,
    /// `AppliedDelta::resulting_digest`, as `apply` reported it — the measurement.
    pub observed: Cid,
    /// When the comparison was taken. The engine's `at`, not a clock read (the engine reads
    /// neither clock nor entropy; `tests/engine_shape.rs` holds it to that).
    pub observed_at: Timestamp,
}

impl PredictionOutcome {
    /// Whether the world arrived where the plan said it would.
    ///
    /// `false` is the road [`NotAttemptedBecause::PromisedPostStateWasWrong`] describes from the
    /// rollback's side; this says the same event from the model's side, and the two are written at
    /// one site so they cannot disagree about which happened.
    #[must_use]
    pub fn matched(&self) -> bool {
        self.predicted == self.observed
    }
}

/// One line of the engine's write-ahead log (42 §3.13, 43 §3).
///
/// **Fifteen variants**: 42 §3.13's eleven, plus the engine-internal records that write no
/// transition — `ApplyStarted` (**E-M5-1**), `ProvenanceDerived` (**M5-25, adopted (a)**, hand 4), and
/// the two-phase-escrow pair `ApplyObserved` / `InverseCompleted` (`req/38` §98, ruling 1, the same
/// "moment inside an edge" standing 43 §3.2 gave the first two) (sem: SEM-gx-engine-337). Each variant names the
/// transition or transitions of 43 §3 that write it, in the comment beside it, and
/// `tests/journal_vocabulary.rs` checks those comments against the canon rather than trusting
/// them.
///
/// # The two places this is not 42 §3.13's literal text
///
/// * **`DraftCreated` is keyed on `IntentId`** (**E-M5-3**). 42 §3.13 writes a `transformation`
///   field; 43 T-1 writes "`TransformationId` is not yet settled (delta/target undetermined)" and puts only
///   `intent_id` in its journal cell. 51 §8.1 rules the precedence -- "the canonical journal record
///   name is 43 §3's transition table; 42 §3.13 is...the old wording, and 43 wins when they
///   conflict" (sem: SEM-gx-engine-338) -- and §37 applies it for the
///   first time. Writing 42's version would mean minting a `TransformationId` before `plan` has run,
///   which is ASM-11's two-stage identity broken at the first step.
/// * **`ApplyStarted` exists** (**E-M5-1**). See the crate documentation for the three-line
///   counter-example (req/78 §3.2 Λ4) it closes. Its place in the order is between `InverseEscrowed`
///   (T-10b) and `Committed` (T-11): the record says "the adapter was asked", which is the fact
///   recovery needs and the only fact that separates "the world did not move" from "the world moved
///   and nothing recorded it" (sem: SEM-gx-engine-339).
///
/// # `Planned` carries `intent_id`, and that is 43's text rather than 42's
///
/// 42 §3.13 gives `Planned` four fields and no `intent_id`; 43 T-2's journal cell is
/// `Planned{intent_id, id, delta_cid, fp0}`. With **E-M5-3** the difference stops being cosmetic:
/// `DraftCreated` no longer carries a `TransformationId`, so `Planned` is the **only** record that
/// ever holds both ids, and a replay without it cannot say which draft became which candidate. The
/// two errata are one decision seen from two sides, which is why they are ruled in one section.
///
/// # No `PartialEq`
///
/// It holds a [`FingerprintRecord`]; see there for why (**E-M4-15** preserved across a new type).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EngineJournalRecord {
    /// T-1 `submit(intent)`. **E-M5-3**: keyed on `IntentId`, because no `TransformationId` exists
    /// yet. `rng_seed` is 41 §6's injected randomness, recorded so that FR-039's replay is
    /// deterministic.
    DraftCreated {
        intent_id: IntentId,
        rng_seed: u64,
        at: Timestamp,
    },
    /// T-2 `plan()`. The only record holding both ids (see the type documentation).
    ///
    /// # 🔴 Two fields 42 §3.13 does not write — **E-M5-13**, implemented under §47 M6-14, adopted (a) (sem: SEM-gx-engine-340)
    ///
    /// 42 §3.13 gives this record four fields and 43 T-2's cell gives it the second id. Two later
    /// hands each found a third thing missing, and §43 merged their tickets into one erratum —
    /// "**E-M5-13 is extended into one erratum: add locator (M5H5-2) and parents (this case) to the
    /// `Planned` record**" (sem: SEM-gx-engine-341):
    ///
    /// * **`locator`** (M5H5-2) — 43 §7-3c resumes an interrupted plan, and a resume needs to know
    ///   *what was being planned against*. Without it v0.1 folded the case into
    ///   `Aborted(InternalError)`, and §42 accepted in writing that this "lies and calls itself a wiring bug" (sem: SEM-gx-engine-342).
    ///   The locator is the adapter's own spelling of the position (42 §3.3) and is already inside
    ///   the `Intent` whose CID is `intent_id`; what the journal lacked was a way to read it back
    ///   without the body, which ASM-9 does not keep.
    /// * **`parents`** (M5H6-6) — 43 T-12's guard is "`T_u.parents` contains `T_o.id`" (sem: SEM-gx-engine-343), and until this
    ///   field the supersede metadata lived only in the in-memory `Transformation`. A crash between
    ///   `Planned` and the commit lost it, so the guard could not be re-checked from the journal
    ///   alone. `Engine::undo` is the one producer of a non-empty list.
    ///
    /// # Why the window was M6 hand 1 and not M7
    ///
    /// 47 §4 makes journal-schema compatibility an upgrade precondition and 33 NFR-024 permits a
    /// breaking change in `0.y.z` with a CHANGELOG entry. **M6 builds the first distributable.**
    /// Before it ships the cost of changing this shape is zero and after it the cost is every user's
    /// journal, so the decision is a *window* rather than a cost comparison (§47 M6-14's discipline-51 form) (sem: SEM-gx-engine-344).
    /// `.gx/VERSION` (req/56 §2) is where a reader records which shape a directory was written with.
    Planned {
        transformation: TransformationId,
        intent_id: IntentId,
        /// **E-M5-13**: the position the plan was made against, in the adapter's own spelling.
        locator: String,
        delta_cid: Cid,
        fp0: FingerprintRecord,
        /// **E-M5-13**: 43 T-12's guard, readable from the journal alone. Empty for every `plan`
        /// and non-empty for `undo`, which names what it undoes.
        parents: Vec<TransformationId>,
        /// 🔴 **DR-46-33 / DR-46-28** (`req/38` §413) — the input-generation stage this plan
        /// attests, fixed at T-2. The join `gx_core::DeterminismBoundary::Mixed`'s
        /// `input_generation` doc names — a deployment's declaration
        /// (`gx_adapter_mcp::catalogue`'s `$determinism_boundary` slot), overridden to
        /// `LlmOriginated` by an `Actor::Agent` — computed here and journalled as its **result**.
        ///
        /// # Why the result and not the actor
        ///
        /// `Actor` is not in Σ, and 43 §7-3b compares a rebuilt payload's digest against the leaf
        /// the ledger holds. A boundary derived from the actor at rebuild time could not be
        /// reproduced (`pipeline.rs`'s `attested_boundary` doc says so for exactly this reason), so
        /// the join is done once, at plan time, and only the answer crosses the crash window —
        /// `reads`' precedent one erratum over. The engine crate reads no catalogue either
        /// (`gx-engine` does not depend on `gx-adapter-mcp`); the declaration reaches here through
        /// the optional `InputStageDeclaration` registry.
        ///
        /// `serde(default)` and skipped when `Unknown`, in `pending`/`reads`' exact E-M5-13 shape:
        /// a journal written before this field decodes as `Unknown` — v0's `attested_boundary`
        /// value, so a rebuild over an old journal is byte-identical — **and re-encodes to the same
        /// bytes**, which is what keeps `journal_roundtrip.rs`/`r30_journal_backward_compat.rs` true
        /// across the version. **Σ does not move**: no `StateRow` field and no `reconstruct` arm
        /// reads it, so `Sigma::canonical_bytes` — AC-039's "bit-equal" — is untouched, for `reads`'
        /// reason.
        #[serde(default, skip_serializing_if = "is_stage_unknown")]
        input_generation: BoundaryStage,
        /// 🔴 **DR-46-45 (`req/973` §B-1)** — what the undo road's compare-and-swap answered,
        /// journalled here for `input_generation`'s reason one field up rather than by analogy
        /// with it.
        ///
        /// # Why the journal and not the receipt-construction site
        ///
        /// `Engine::undo` does not build a receipt — it returns a `Candidate` and the caller drives
        /// `verify`/`canonicalize`/`commit` (43 §5). So the witness is out of scope by the time
        /// T-11 assembles a payload, and the only way to get it there is to write it down. And the
        /// only place it may be written down is Σ: 43 §7-3b compares a rebuilt payload's digest
        /// against the leaf the ledger already holds, and the process that repairs is not the
        /// process that compared, so a witness re-derived at rebuild time would answer
        /// `payload_mismatch` — the word for tampering — for every crash-window recovery of an
        /// undo. This is `read_set`'s road (journalled on `InverseEscrowed`) and `confinement`'s
        /// (journalled on `ProvenanceDerived`), on the record that already carries `parents`.
        ///
        /// `None` for every `plan()` — which is the discriminator the payload uses: a `Planned`
        /// with no witness is not an undo's. `None` also for a journal written before this erratum,
        /// which reproduces the absence in the filed receipt rather than inventing a claim about a
        /// comparison nobody recorded.
        ///
        /// `serde(default)` and skipped when absent, in `input_generation`'s exact E-M5-13 shape,
        /// so a journal written before this field decodes **and re-encodes to the same bytes**
        /// (`journal_roundtrip.rs`, `r30_journal_backward_compat.rs`). **Σ does not move**: no
        /// `StateRow` field and no `reconstruct` arm reads it, so `Sigma::canonical_bytes` —
        /// AC-039's "bit-equal" — is untouched, for `reads`' reason.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        undo_witness: Option<gx_witness::receipt::UndoDisposition>,
        at: Timestamp,
    },
    /// T-3 `verify_start`.
    VerifyStarted {
        transformation: TransformationId,
        at: Timestamp,
    },
    /// T-4a / T-4b / T-4c -- the gate answered. `kind` is all three verdicts. **And T-4e**, where
    /// nothing answered; see below.
    ///
    /// # 🔴 Two fields 42 §3.13 does not write, and 43 T-4e does
    ///
    /// 42 §3.13 gives this record four fields. 43 T-4e's journal cell is
    /// `Verdict{id, Admit, fail_posture_engaged=true}`, and 43 §4 makes the flag mandatory rather
    /// than advisory: "always stamp `enforced=false` and `fail_posture_engaged=true` onto the
    /// receipt". The two texts disagree, and 51 §8.1 has already settled which wins -- "the
    /// canonical journal record name is 43 §3's transition table; 42 §3.13 is...the old wording,
    /// and 43 wins when they conflict" (sem: SEM-gx-engine-345). **E-M5-3** applied that clause to
    /// `DraftCreated`'s fields and hand 1 applied it to `Planned`'s in the same breath; this is its
    /// third application, and it is **raised as M5H2-1** rather than treated as settled, because
    /// applying a ruled clause to a case nobody has ruled on is still a hand's reading.
    ///
    /// * **`fail_posture_engaged`** is `false` for T-4a/T-4b/T-4c and `true` for T-4e alone. It is
    ///   what makes "admitted" and "admitted because the verifier could not be reached" different
    ///   records, which is INV-S5's requirement ("a Committed with `enforced=false` must be...
    ///   distinguishable" (sem: SEM-gx-engine-346))
    ///   reaching the journal rather than only the receipt.
    /// * **`verdict_digest` is an `Option`**, and `None` belongs to T-4e alone. A digest identifies
    ///   a verdict, and under T-4e **no verdict was computed** -- the gate was never reached. The
    ///   alternative was to mint an empty `AdmitProof` and digest that, which would put a proof in
    ///   the record for an admission no gate made: "not implemented" and "failed" wearing one face (sem: SEM-gx-engine-347), which
    ///   §32 M4H4-2 refused twice. `None` beside `fail_posture_engaged = true` says the true thing.
    Verdict {
        transformation: TransformationId,
        kind: VerdictKind,
        verdict_digest: Option<Cid>,
        fail_posture_engaged: bool,
        at: Timestamp,
    },
    /// T-5 / T-5b -- a human answered. 42 §3.13: "kind is Admit|Deny only" (sem: SEM-gx-engine-348), which is a fact about the
    /// two transitions rather than about the type; 43 has no `Escalated → Escalated` edge, and
    /// [`crate::Engine::escalation`] is the guard that says so.
    ///
    /// # 🔴 Two fields 42 §3.13 does not write, and AC-071/072 do (hand 6)
    ///
    /// 42 §3.13 gives this record three fields. AC-071, verbatim, asks for more (sem: SEM-gx-engine-349):
    ///
    /// > confirm that the issued receipt trail (journal/Receipt metadata) includes
    /// > `Evidence(HumanDecision)` (decision=Admit, reason, the ruler actor) (sem: SEM-gx-engine-349)
    ///
    /// and AC-072 asks the same of a rejection ("the journal record includes `Evidence(HumanDecision)`
    /// (decision=Deny, reason)" (sem: SEM-gx-engine-349)). **E-M2-3** retired the `Evidence` variant those two name -- "43
    /// T-5's 'human-ruling receipt (signed)' is a receipt" -- so the three facts have nowhere to go but
    /// the journal record and the signed receipt. `decision` is `kind`; `reason` and `actor` are
    /// these two fields; the signature over them is the [`gx_witness::ReceiptKind::VerdictReceipt`]
    /// T-5 issues.
    ///
    /// The same shape **M5H2-1 / E-M5-7** took for `Verdict` and **M5H4-2** for `Aborted`: an
    /// implementation cannot satisfy an acceptance criterion out of a table with no room for it, so
    /// it writes the true thing and raises the divergence. Raised as **M5H6-2**.
    ///
    /// `actor` is the ruler, which is **not** `Transformation.actor` (the submitter). A record that
    /// carried only the submitter would say who asked and never who allowed, which is the one fact
    /// an escalation exists to record (P-7, INV-S6).
    ///
    /// # 🔴 **DR-46-31** — a third field, and a re-issue is what needed it
    ///
    /// `Engine::escalation` digests the ruling (`cid::compute(ruling)`) and puts that digest into
    /// the `VerdictSummary` of the receipt T-5 signs and of the `CommitReceipt` the ledger later
    /// witnesses. Until this field the digest existed **only** in the in-memory table: the journal
    /// carried the ruling's `kind`, `reason` and `actor` and not the value taken over them, so
    /// `replay.rs` had nothing to move `StateRow.verdict_digest` with and left it holding T-4c's
    /// `Escalate` proof. Σ then named the human's `Admit` beside the escalation's digest, and
    /// [`crate::Engine::reissue_receipt`] — which rebuilds a payload from Σ alone — could not
    /// reproduce the leaf. **Every** commit that walked E-M3-4's road answered `world_moved`.
    /// Raised by `req/453` §10, confirmed at `replay.rs:1212-1216` by `req/470` §4-3, numbered by
    /// `req/38` §261 ruling 2b.
    ///
    /// **Recorded rather than re-derived.** [`crate::HumanRuling`]'s three fields are exactly the
    /// three this record already carried, so replay could have rebuilt the value instead. It does
    /// not, for the reason `req/38` §32 M4H4-2 refused twice: a re-derivation would *claim* the
    /// digest the receipt was issued under while in fact computing a fresh one, and the two part
    /// company the day `HumanRuling` gains a field or its canonical encoding moves — silently, on
    /// journals already written. The record is the evidence; deriving it would make Σ's agreement
    /// with the leaf a coincidence of two encoders rather than a fact this file holds.
    ///
    /// `serde(default)` and skipped when absent, in `InverseEscrowed.reads`' exact shape
    /// (**E-M5-13**'s precedent): a journal written before this field decodes as it always did —
    /// `None`, so replay leaves `verdict_digest` where T-4c put it and the old degradation is
    /// preserved rather than papered over — **and re-encodes to the same bytes**, which is what
    /// keeps `journal_roundtrip.rs` true across the version.
    HumanDecision {
        transformation: TransformationId,
        kind: VerdictKind,
        reason: String,
        actor: Actor,
        /// 🔴 **DR-46-31** — the digest of what the person decided, as `Engine::escalation`
        /// computed it and as the signed receipt carries it. `None` is a pre-DR-46-31 journal.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verdict_digest: Option<Cid>,
        at: Timestamp,
    },
    /// T-8 / T-8r `canonicalize`. `enforced` is `Some(false)` for T-8r only (42 §3.13), which is
    /// why it is an `Option` and not a `bool` -- "unknown" and "false" (sem: SEM-gx-engine-350) are different records.
    Canonicalized {
        transformation: TransformationId,
        canonical_cid: Cid,
        enforced: Option<bool>,
        at: Timestamp,
    },
    /// T-9 `commit_start`. 43 §4: "**always journal before the side effect runs**" (sem: SEM-gx-engine-351).
    CommittingStarted {
        transformation: TransformationId,
        at: Timestamp,
    },
    /// 🔴 **M5-25, adopted (a)** -- the provenance of this transformation, derived by the engine and
    /// written to the journal. **D-7's third resolution** (sem: SEM-gx-engine-352), and the second record 42 §3.13 does not
    /// list.
    ///
    /// > **M5-25, adopted (a) + journal**: **D-7's third resolution -- the engine becomes the
    /// > producer of `Provenance`** (the seat, `ProvenanceInputs`, already exists). It is written to
    /// > the **journal** (consistent with ASM-9's digest-only rule; the receipt wire is not moved)
    /// > (sem: SEM-gx-engine-353)
    ///
    /// gx-witness has carried `Provenance` and `Environment` since M2 with **zero code consumers**
    /// through M3 and M4, and 42 §3.9's `input_objects` -- "the input snapshots the adapter read
    /// during plan/verify" (sem: SEM-gx-engine-354) -- can only be collected by the thing that watched the adapter read them.
    /// That is this crate, and this is the record.
    ///
    /// # Its own record rather than a field on `Committed`, and the reason is a crash
    ///
    /// The alternative was a `provenance` field on `Committed`, which keeps the vocabulary at
    /// twelve. It also loses the provenance in exactly the window 43 §7-3b exists for: a crash
    /// after `ledger.append` and before the `Committed` record leaves a commit that happened with
    /// no provenance recorded, and recovery cannot re-derive one because deriving needs the
    /// `Transformation` body the journal does not hold (ASM-9). Written here, before the world
    /// moves, it survives every crash the journal survives. Raised as **M5H4-1**: which record
    /// carries it is the form 42 §3.13 has to gain, and this hand implements one and asks.
    ///
    /// The value is names and digests only -- object ids, an intent digest, version strings -- so
    /// ASM-9 is not bent by putting it in the log.
    ProvenanceDerived {
        transformation: TransformationId,
        provenance: Provenance,
        at: Timestamp,
    },
    /// T-10b -- the inverse was escrowed, before `apply` was called.
    ///
    /// # 🔴 `inverse_cid` is an `Option` (**E-M5-9**, hand 6)
    ///
    /// 43 T-10b's guard is "the inverse can be constructed (`Some`)" (sem: SEM-gx-engine-355) and its journal cell names a CID, so 42
    /// §3.13 typed one. 42 §3.12 nevertheless defines `InverseStatus::Unavailable` as
    /// "the case where `invert()` returns None (cannot be constructed)" (sem: SEM-gx-engine-355) -- a state the journal had no way to spell.
    /// §40 rules the fix and dates it:
    ///
    /// > **M5H3-2, direction adopted (a), implementation window = hand 6** = **E-M5-9 (reserved)**:
    /// > making `InverseEscrowed.inverse_cid` an `Option` (42 §3.13 erratum, the fourth use of 51
    /// > §8.1's precedence clause) is **implemented in the same turn that hand 6's escalation
    /// > approval makes the path real** (sem: SEM-gx-engine-356)
    ///
    /// The path is T-5's. **E-M3-4** escalates a transformation whose `invert` answers `None`, so
    /// until a human could approve one nothing without a constructible inverse ever reached
    /// `Committing`; hand 4 wrote no record at all in that arm and said so. Now the arm is live,
    /// and "we asked and there is none" has to be distinguishable from "we never asked" (sem: SEM-gx-engine-357) (§32
    /// M4H4-2) -- which, in a journal, means a record that says the first thing.
    ///
    /// Symmetric with **E-M5-6**, which made the same field optional in the in-memory
    /// [`EscrowedInverse`] and kept it in step with `status` through checked constructors. The two
    /// errata are one decision on both sides of the door: [`BlobStore::escrowed`] is where a row
    /// read back from here goes through [`EscrowedInverse::restore`].
    InverseEscrowed {
        transformation: TransformationId,
        inverse_cid: Option<Cid>,
        /// 🔴 Two-phase escrow's flag (`req/38` §99, ruling 2, clause ①, the E-M5-13 precedent form; sem: SEM-gx-engine-358): `true`
        /// marks a **partial** escrow (`InverseStatus::Pending`) whose do-time members the
        /// completion step will fill after `apply`. `serde(default)` so that every journal
        /// written before this field decodes as it always did (`false` = a complete escrow), and
        /// skipped when `false` so those journals' records still **encode** byte-identically
        /// (the round-trip property of `journal_roundtrip.rs` holds across the version). The
        /// vocabulary growing 13→15 alone would not have closed this: a replay must be able to
        /// tell a `Pending` row from an `Available` one *from this record*, or the crash window
        /// (`Pending` + no observation) could not fold honestly to `Unavailable`.
        #[serde(default, skip_serializing_if = "is_false")]
        pending: bool,
        /// 🔴 **DR-46-26** — what the escrow **read** to build this inverse, journalled because a
        /// rebuild cannot obtain it any other way.
        ///
        /// 42 §3.10 carries a `read_set` on the commit receipt since D24, and DR-46-26 gave it a
        /// producer. That producer is `SubstrateAdapter::invert`, which runs at T-10b — *before*
        /// `apply`, because the prior stops existing when the world moves. 43 §7-3b's recovery
        /// rebuilds a receipt payload and compares its digest against the leaf the ledger already
        /// holds; without this field the rebuild has no way to reach the value, and a payload that
        /// reproduces thirteen of fourteen fields does not reproduce a digest. Measured, not
        /// assumed: `crates/gx-cli/tests/model_a_probes.rs` answered `payload_mismatch` on the
        /// crash-window beds until this record carried the reads.
        ///
        /// **The entries and not the `ReadSet`.** `ReadSet::from_reads` is still the only thing
        /// that chooses a granularity (`req/441` §4), so a rebuild that re-derives the set from
        /// these entries lands on the same variant the commit landed on, by the same function.
        ///
        /// `serde(default)` and skipped when empty, exactly as `pending` above: every journal
        /// written before this field decodes as it always did (an escrow that read nothing), and
        /// those journals' records still **encode** byte-identically, which is what keeps
        /// `journal_roundtrip.rs` true across the version. **Σ does not move**: `EscrowRow` gains
        /// nothing, so `Sigma::canonical_bytes` — AC-039's "bit-equal" — is untouched.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reads: Vec<ReadEntry>,
        /// 🔴 **DR-46-34** — whether the field above is a **recorded** empty list or a gap.
        ///
        /// # The defect, in `req/472` §6's words
        ///
        /// > `CommitReceipt.read_set` being `null` currently spells three different facts with the
        /// > same bytes … ③ the record is there but has no `reads` field and `#[serde(default)]`
        /// > gave it an empty one.
        ///
        /// `reads` above is `skip_serializing_if = "Vec::is_empty"`, which is what keeps every
        /// pre-DR-46-26 journal encoding byte-identically — and is exactly why an empty `reads` on
        /// a decoded record carries no information at all. A rebuild reading one cannot tell
        /// "the escrow was asked and read nothing" from "this journal is older than the field",
        /// and it was writing the first onto a signed receipt on the strength of the second.
        ///
        /// # Why a `bool`, and why this shape
        ///
        /// `undetermined` below is the same move for `inverse_cid`'s absence, and this follows it
        /// exactly rather than inventing a second idiom: **`serde(default)` and skipped when
        /// `false`** (E-M5-13's shape), so a journal written before this lane decodes as it always
        /// did *and re-encodes to the same bytes*, which is what keeps `journal_roundtrip.rs` and
        /// `r30_journal_backward_compat.rs` true across the version. **Σ does not move**:
        /// `EscrowRow` gains nothing, so `Sigma::canonical_bytes` — AC-039's "bit-equal" — is
        /// untouched, for `reads`'s reason one field up.
        ///
        /// It is not the read-set itself for `undetermined`'s reason: a non-empty `reads` already
        /// says the reads were recorded, so a second encoding of that could disagree with its
        /// neighbour. The flag therefore carries information **only where `reads` is empty**, and
        /// the contradictory foreign combination (`false` beside a non-empty list) is folded to the
        /// honest side by the one road that reads it — `Engine::rebuilt_attest`, whose first arm is
        /// `attested || !reads.is_empty()`. That is a road and not `replay.rs` because **Σ never
        /// sees this field**: `EscrowRow` gains nothing from it, so there is no fold in the replay
        /// to put one in. `undetermined`'s fold is in `replay.rs` precisely because `undetermined`
        /// *does* reach Σ, through `EscrowRow::status`.
        #[serde(default, skip_serializing_if = "is_false")]
        reads_attested: bool,
        /// 🔴 **DR-46-26** — which of C-25's two negative answers this `inverse_cid: None` was.
        ///
        /// **E-M5-9** made `inverse_cid` an `Option` and gave the absence one meaning: "we asked
        /// and there is none", which 42 §3.12 spells `Unavailable`. DR-46-26 gives the absence a
        /// **second** preimage — `Reversibility::Unknown`, the prior that would not be read under
        /// `OnReadFailure::Unknown`, which 42 §3.12 spells `Undetermined` — and a replay that could
        /// not tell them apart would report "there is no undo" about a change nobody established
        /// anything about. That is DR-46-13's defect exactly, one road over from where this lane
        /// closed it, and it is the reason the flag is here rather than derived.
        ///
        /// It is a `bool` and not the verdict itself for the same reason `pending` is: the positive
        /// answer is already carried by `inverse_cid` being `Some`, so a second encoding of it
        /// would be a field that can disagree with its neighbour. `false` **with** a CID is the
        /// ordinary escrow and `false` **without** one is E-M5-9's `Unavailable`; a foreign journal
        /// spelling `true` beside a CID is folded to the honest side by `replay.rs`.
        ///
        /// `serde(default)` and skipped when `false`, in `pending`'s exact shape (**E-M5-13**), so
        /// every journal written before this lane decodes as it always did **and** re-encodes to
        /// the same bytes.
        #[serde(default, skip_serializing_if = "is_false")]
        undetermined: bool,
        at: Timestamp,
    },
    /// 🔴 **E-M5-1** -- the adapter was asked to apply. Written **before** the call, which is the
    /// whole of its purpose: recovery that finds this record knows the world may already have
    /// moved and must not re-run the CAS of T-10a against it.
    ApplyStarted {
        transformation: TransformationId,
        delta_cid: Cid,
        at: Timestamp,
    },
    /// 🔴 Two-phase escrow, record one of two (`req/38` §98, ruling 1; sem: SEM-gx-engine-359; no transition — a moment
    /// inside T-9's critical section, the `ApplyStarted`/`ProvenanceDerived` precedent of 43
    /// §3.2). The applied call's observed answer, content-addressed into the observation store
    /// (raw bytes, **not** the `PlannedDelta` blob store — §99, ruling 2, clause ③; sem: SEM-gx-engine-360), journalled because it
    /// is not re-obtainable: the idempotency contract never re-issues the call (`req/160` 1-0
    /// fact 3; sem: SEM-gx-engine-360), so an observation that lived only in memory would let a crash silently consume
    /// the undo guarantee. Written only when the escrow is `Pending` — a complete escrow needs
    /// no observation and gets no record.
    ApplyObserved {
        transformation: TransformationId,
        observation_cid: Cid,
        at: Timestamp,
    },
    /// 🔴 Two-phase escrow, record two of two (`req/38` §98, ruling 1; sem: SEM-gx-engine-361; no transition). The `Pending`
    /// escrow's outcome, inside the same critical section, before T-11's receipt: `Some` is the
    /// completed inverse's CID (row → `Available`; the receipt's `inverse_delta` names it) and
    /// `None` is every completion failure folded to `Unavailable` with the commit continuing
    /// (§99, ruling 2, clause ④ — an abort after a successful apply would record "Aborted" (sem: SEM-gx-engine-362) about a world
    /// that moved). `Admit` beside `inverse_delta: None` on the receipt is the failure's visible
    /// fingerprint.
    InverseCompleted {
        transformation: TransformationId,
        inverse_cid: Option<Cid>,
        at: Timestamp,
    },
    /// T-11 -- ledger append and receipt issue completed.
    Committed {
        transformation: TransformationId,
        ledger_seq: u64,
        at: Timestamp,
    },
    /// T-4d / T-6 / T-7 / T-10a / T-10c -- terminal, with the reason gx-core defines (ASM-15).
    ///
    /// 🔴 `rollback` is not in 42 §3.13's row. See [`Rollback`] for why AC-038 leaves nowhere else
    /// to put it, and **M5H4-2** for the divergence as a ticket. `None` for every abort that is not
    /// T-10c.
    Aborted {
        transformation: TransformationId,
        reason: AbortReason,
        rollback: Option<Rollback>,
        at: Timestamp,
    },
    /// T-12 -- an inverse of this transformation reached `Committed`. 43 §5: "an undo is a new
    /// commit, not a rewrite" (sem: SEM-gx-engine-363), so this record is written *about* `transformation` and changes none of
    /// its earlier records.
    Superseded {
        transformation: TransformationId,
        by: TransformationId,
        at: Timestamp,
    },
}

/// The variant names, declared once, in the order they are written above.
///
/// The **E-M2-23 / A-10** form: a list a test can compare against the variants read out of this
/// file, so that a thirteenth variant added without a row is a failing probe rather than a silent
/// addition. `tests/journal_vocabulary.rs` compares all three of this array, the source, and the
/// canon.
pub const JOURNAL_RECORD_KINDS: [&str; 15] = [
    "DraftCreated",
    "Planned",
    "VerifyStarted",
    "Verdict",
    "HumanDecision",
    "Canonicalized",
    "CommittingStarted",
    "ProvenanceDerived",
    "InverseEscrowed",
    "ApplyStarted",
    "ApplyObserved",
    "InverseCompleted",
    "Committed",
    "Aborted",
    "Superseded",
];

/// `serde(skip_serializing_if)`'s predicate for the `InverseEscrowed.pending` flag: skipping the
/// `false` spelling is what keeps every pre-two-phase journal record's canonical encoding
/// byte-identical across this version (the round-trip property, and 47 §4's schema-compatibility
/// precondition satisfied by construction rather than by a migration).
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(flag: &bool) -> bool {
    !*flag
}

/// `serde(skip_serializing_if)`'s predicate for `Planned.input_generation` (**DR-46-33**): skipping
/// the `Unknown` spelling keeps every pre-DR-46-33 `Planned` record's canonical encoding
/// byte-identical, in `is_false`'s exact role one field over. `Unknown` is `BoundaryStage`'s
/// `Default`, so `serde(default)` decodes an absent field to it, which is the value v0 attested.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_stage_unknown(stage: &BoundaryStage) -> bool {
    matches!(stage, BoundaryStage::Unknown)
}

impl EngineJournalRecord {
    /// Which of [`JOURNAL_RECORD_KINDS`] this record is.
    ///
    /// No `_` arm: a variant added tomorrow stops this function from compiling. This is also the
    /// name serde writes as the outer key of the encoded map, and `tests/journal_identity.rs`
    /// asserts the two agree -- a name that drifted from its wire form would make a journal
    /// unreadable by the code that wrote it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            EngineJournalRecord::DraftCreated { .. } => "DraftCreated",
            EngineJournalRecord::Planned { .. } => "Planned",
            EngineJournalRecord::VerifyStarted { .. } => "VerifyStarted",
            EngineJournalRecord::Verdict { .. } => "Verdict",
            EngineJournalRecord::HumanDecision { .. } => "HumanDecision",
            EngineJournalRecord::Canonicalized { .. } => "Canonicalized",
            EngineJournalRecord::CommittingStarted { .. } => "CommittingStarted",
            EngineJournalRecord::ProvenanceDerived { .. } => "ProvenanceDerived",
            EngineJournalRecord::InverseEscrowed { .. } => "InverseEscrowed",
            EngineJournalRecord::ApplyStarted { .. } => "ApplyStarted",
            EngineJournalRecord::ApplyObserved { .. } => "ApplyObserved",
            EngineJournalRecord::InverseCompleted { .. } => "InverseCompleted",
            EngineJournalRecord::Committed { .. } => "Committed",
            EngineJournalRecord::Aborted { .. } => "Aborted",
            EngineJournalRecord::Superseded { .. } => "Superseded",
        }
    }

    /// 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the oldest framing whose **record
    /// vocabulary** contains this record.
    ///
    /// # Why this is a property of the record and not of the enum
    ///
    /// Because the incompatibility R29 introduced was not a new variant — it was a new **value
    /// inside** one. `CHANGELOG.md` §3's row for R29 says so in as many words: *"No variant of
    /// `EngineJournalRecord` was added, removed or reshaped. What changed is a value inside one."*
    /// A guard keyed on the variant name would have let that through, and
    /// `probes/doubt/tests/journal_changelog_doubt.rs` says the same of itself — it watches the
    /// variant **name set**, and its own doc declares a change *inside* a variant as "not
    /// measured". So the question has to be asked of the record in hand.
    ///
    /// Today exactly one answer is not [`JournalFormat::Legacy`]: an `Aborted` whose roll-back is
    /// [`Rollback::Diverged`], the word R29 minted. Each future word added to a value that reaches
    /// the journal adds an arm here, and the arm is what stops it being written into a file an
    /// older binary was promised it could read.
    #[must_use]
    pub const fn minimum_format(&self) -> JournalFormat {
        match self {
            EngineJournalRecord::Aborted {
                rollback: Some(Rollback::Diverged),
                ..
            } => JournalFormat::ChainedV2,
            // Every other record is spelled in words every binary that ever wrote this journal
            // format already knows. `Legacy` rather than `Chained` because the answer is "no
            // constraint", and `Legacy` is the weakest framing there is.
            _ => JournalFormat::Legacy,
        }
    }

    /// When the record says the event happened.
    ///
    /// Every variant carries `at`, which is 41 §6's injected clock reaching the journal. One
    /// accessor rather than twelve `match` arms at each call site, and no `_` arm here either.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        match self {
            EngineJournalRecord::DraftCreated { at, .. }
            | EngineJournalRecord::Planned { at, .. }
            | EngineJournalRecord::VerifyStarted { at, .. }
            | EngineJournalRecord::Verdict { at, .. }
            | EngineJournalRecord::HumanDecision { at, .. }
            | EngineJournalRecord::Canonicalized { at, .. }
            | EngineJournalRecord::CommittingStarted { at, .. }
            | EngineJournalRecord::ProvenanceDerived { at, .. }
            | EngineJournalRecord::InverseEscrowed { at, .. }
            | EngineJournalRecord::ApplyStarted { at, .. }
            | EngineJournalRecord::ApplyObserved { at, .. }
            | EngineJournalRecord::InverseCompleted { at, .. }
            | EngineJournalRecord::Committed { at, .. }
            | EngineJournalRecord::Aborted { at, .. }
            | EngineJournalRecord::Superseded { at, .. } => *at,
        }
    }

    /// The `TransformationId` this record is about, where there is one.
    ///
    /// `None` for `DraftCreated` alone, and that `None` is **E-M5-3** in the type system: a draft
    /// has no `TransformationId` to be about. A state table keyed on `TransformationId` therefore
    /// cannot hold a draft, which is **M5-17, adopted (b)** -- "the Draft phase is held only by the
    /// journal; the state table starts at Candidate" (sem: SEM-gx-engine-364) -- expressed as a signature rather than as a convention.
    #[must_use]
    pub const fn transformation(&self) -> Option<TransformationId> {
        match self {
            EngineJournalRecord::DraftCreated { .. } => None,
            EngineJournalRecord::Planned { transformation, .. }
            | EngineJournalRecord::VerifyStarted { transformation, .. }
            | EngineJournalRecord::Verdict { transformation, .. }
            | EngineJournalRecord::HumanDecision { transformation, .. }
            | EngineJournalRecord::Canonicalized { transformation, .. }
            | EngineJournalRecord::CommittingStarted { transformation, .. }
            | EngineJournalRecord::ProvenanceDerived { transformation, .. }
            | EngineJournalRecord::InverseEscrowed { transformation, .. }
            | EngineJournalRecord::ApplyStarted { transformation, .. }
            | EngineJournalRecord::ApplyObserved { transformation, .. }
            | EngineJournalRecord::InverseCompleted { transformation, .. }
            | EngineJournalRecord::Committed { transformation, .. }
            | EngineJournalRecord::Aborted { transformation, .. }
            | EngineJournalRecord::Superseded { transformation, .. } => Some(*transformation),
        }
    }
}

// ---------------------------------------------------------------------------
// 42 §3.12 -- the escrowed inverse
// ---------------------------------------------------------------------------

/// What has become of an escrowed inverse (42 §3.12).
///
/// Four values, and hand 6 is where three of them acquire a writer.
///
/// 42 §3.12 defines the values and 43 §3 defines no edge that sets them, which is the gap M5-16
/// raised and §37 answered with adopted (a) (sem: SEM-gx-engine-365) -- `Consumed { by }` is written at T-12, in one place,
/// together with the [`SupersedeIndex`] entry. `Available` is T-10b's, and `Unavailable` is
/// **E-M5-9**'s: a T-10b whose `adapter.invert` answered `None`, which is reachable only after a
/// human approves an escalation (T-5), which is why the two errata land in one hand.
///
/// 🔴 `Expired` has **no writer at all**, and that is DR-9 rather than an omission: "the OSS core's
/// default is unlimited (deadline management/extension are commercial-tier features)" (sem: SEM-gx-engine-366) and req/78 N-06 keeps the enforcement of
/// `retained_until` out of v0.1. `tests/lifecycle_transitions.rs` asserts the absence, so the day
/// a hand adds one it has to say why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InverseStatus {
    /// The inverse is held and has not been used.
    Available,
    /// A transformation applied it and reached `Committed` (43 T-12).
    Consumed { by: TransformationId },
    /// `retained_until` passed. The *seat* only (sem: SEM-gx-engine-367): DR-9 puts enforcement of the deadline in the commercial tier and
    /// req/78 N-06 keeps it out of v0.1, so nothing in this crate moves a status to this value.
    Expired,
    /// `SubstrateAdapter::invert` returned `None` -- no inverse could be constructed.
    ///
    /// 🔴 Since two-phase escrow, also every completion failure of a `Pending` row (`req/38` §99
    /// ruling 2, clause ④; sem: SEM-gx-engine-368): the fold is honest — "the escrow was attempted and its outcome is recorded" —
    /// and the row's `inverse_cid` is `None` either way, so the checked constructors' invariant
    /// is one rule still.
    Unavailable,
    /// 🔴 A **partial** escrow held inside the Committing critical section (two-phase escrow,
    /// `req/38` §98, ruling 1; sem: SEM-gx-engine-369): every pre-state member is resolved and escrowed (T-10b, E-M4-30
    /// unmoved), and a declared do-time member awaits the applied call's observation. Normal
    /// runs leave the section as `Available` (completed) or `Unavailable` (folded); a `Pending`
    /// row visible *outside* the section is a crash's trace, and recovery folds or completes it
    /// (43 §7-3 + `ApplyObserved`). `Engine::undo` refuses it by name — a partial inverse is not
    /// yet an executable one.
    Pending,
    /// 🔴 **R8 / `req/234` B-5** — the escrow row names an inverse and the blob store does not
    /// hold it.
    ///
    /// Additive, and a **sixth** word rather than a re-use of [`InverseStatus::Unavailable`],
    /// because the two are different facts with different remedies. `Unavailable` is 42 §3.12's
    /// "`invert()` returned `None`" — gx asked and there was no inverse to build, which is a
    /// property of the change. This is "there was one, it was escrowed, and its body is gone from
    /// `.gx/ledger/journal.blobs/`" — which is 43 §7.9 (b)'s Model B, and which the receipt still
    /// proves happened even though nobody can run the undo any more.
    ///
    /// `req/234` B-5 measured the cost of not having the word: every blob deleted, `gx repair`
    /// answering `rc=0 remedy: null head_authenticity: "verified"`, `GET /v1/transformations`
    /// reporting `Available` about a body that was not there, and `gx undo` falling over with
    /// `INTERNAL` — the one code 44 §2.3 reserves for what cannot be classified.
    ///
    /// Nothing in this crate ever **writes** this value into an escrow row: it is produced by
    /// `Engine::inverse_status`, which reads the store before it answers. A row on the disk that
    /// carried it would be a fact recorded once about a directory that can change afterwards.
    BodyMissing,
    /// 🔴 **DR-46-13 / §237-5, folded into DR-46-24(A)'s erratum batch** — nobody established
    /// whether an inverse exists.
    ///
    /// # The defect, stated as `req/38` §198 ruling (b) states it
    ///
    /// > **A-4 is judged half done**: `unknown` reaches the adapter's return value, the refusal
    /// > sentence and the probe — **the receipt payload is still the same shape as `false`** →
    /// > **DR-46-13 raised** (a seventh `InverseStatus` word, or a field added to 42 §3.10 — a
    /// > change to a frozen face, Lean-side confirmation included).
    ///
    /// C-25 gives an adapter three answers and `gx_adapter_mcp::Reversibility` carries all three
    /// (`true`/`false`/`unknown`, `catalogue.rs`). `SubstrateAdapter::invert` carries **two**: it
    /// returns `Result<Option<PlannedDelta>>`, and `Ok(None)` is where both *no inverse exists* and
    /// *nobody could find out* arrive. Downstream of that funnel the two facts are one word, and a
    /// reader of an escrow row — or of the receipt whose `inverse_delta` is `null` — cannot tell a
    /// change that has no undo from a change whose undo was never determined. The remedies differ:
    /// the first is a property of the change, the second is a read that did not answer under the
    /// `OnReadFailure::Unknown` posture, which a deployment chose and can unchoose.
    ///
    /// # 🔴 This word had no writer, and that was declared rather than discovered
    ///
    /// **DR-46-26 gave it one** (`req/38` §258, E-DR4626-1). The section below is kept in its own
    /// tense (no-delete): it is the record of what was true between D24 and that lane, and — more
    /// to the point — it is the record of a block that was named **as a coordinate rather than as
    /// a memory**, which is what let the next lane walk straight to it. The producer is
    /// `gx-engine/src/pipeline.rs`'s T-10b escrow, in the arm where no inverse was constructed:
    /// `Reversibility::Unknown => InverseStatus::Undetermined`. It is the **only** writer, and
    /// `tests/lifecycle_transitions.rs` asserts both the count and the arm — the same two
    /// assertions that used to assert the absence, turned over.
    ///
    /// Exactly like [`InverseStatus::Expired`], which 42 §3.12 names and DR-9 keeps out of v0.1:
    /// `tests/lifecycle_transitions.rs` asserts the absence, so the day a hand adds a producer it
    /// has to say why. The producer is blocked on one thing and it is named here so that the block
    /// is a coordinate rather than a memory: **`Reversibility` does not cross the crate boundary**.
    /// `invert_with_verdict` is `pub(crate)` in gx-adapter-mcp and the trait method above it
    /// flattens the three answers into an `Option`, so giving this word a writer means widening
    /// `SubstrateAdapter::invert` — a declaration `gx-substrate/tests/adapter_spec.rs` and
    /// `gx-adapter-mcp/tests/ac_051.rs` both pin by its exact text, and one every adapter in the
    /// workspace implements. `req/441` §5 carries that as the next lane's first coordinate.
    ///
    /// Naming the seat now is what §237-5 ruled: the vocabulary erratum moves **with** 42 §3.12,
    /// in this batch, so that the wire and the word are one change and not two.
    Undetermined,
}

impl InverseStatus {
    /// The five: 42 §3.12's four in its order, then `Pending` (additive, `req/38` §98, ruling 1; sem: SEM-gx-engine-370).
    ///
    /// 🔴 **R8 / `req/234` B-5** — and a sixth, `BodyMissing`, additive in the same sense. The
    /// sentence above is kept exactly as it was written (no-delete): it is the true record of what
    /// this vocabulary was through R7, and the count in the type below is what a reader compares it
    /// with.
    /// 🔴 **DR-46-13 / §237-5** — and a seventh, `Undetermined`, additive in the same sense
    /// again. Six until DR-46-24(A)'s erratum batch.
    pub const ALL_KINDS: [&'static str; 7] = [
        "Available",
        "Consumed",
        "Expired",
        "Unavailable",
        "Pending",
        "BodyMissing",
        "Undetermined",
    ];

    /// Which of [`InverseStatus::ALL_KINDS`] this is. No `_` arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            InverseStatus::Available => "Available",
            InverseStatus::Consumed { .. } => "Consumed",
            InverseStatus::Expired => "Expired",
            InverseStatus::Unavailable => "Unavailable",
            InverseStatus::Pending => "Pending",
            InverseStatus::BodyMissing => "BodyMissing",
            InverseStatus::Undetermined => "Undetermined",
        }
    }
}

/// The undo guarantee's body: an inverse delta held for a committed transformation (42 §3.12,
/// DR-1(a)).
///
/// # The one place 42 §3.12 cannot be taken literally
///
/// The table types `inverse_delta` as a `PlannedDelta` and, two rows down, defines
/// `InverseStatus::Unavailable` as "the case where `invert()` returns None (cannot be constructed)" (sem: SEM-gx-engine-371). A value cannot both
/// hold a delta and record that none exists. So the field is an `Option` here and the two are kept
/// in step by the constructors: [`EscrowedInverse::held`] takes a delta and cannot be
/// `Unavailable`; [`EscrowedInverse::unavailable`] takes none and can be nothing else; and
/// [`EscrowedInverse::restore`], the door a hand-3 store read-back will come through, refuses the
/// contradiction with [`Error::InconsistentEscrow`]. **Raised as M5H1-3** -- the alternative
/// readings are "`Unavailable` is recorded somewhere that is not this struct" and "the escrow
/// table has no row when no inverse exists" (sem: SEM-gx-engine-372), and both are spec changes rather than implementation
/// choices.
///
/// # Why the payload is here at all
///
/// ASM-9 keeps bodies out of the system and stores digests. 42 §5 makes this the named exception --
/// "**the payload body itself is retained** (digest-only would make an actual undo impossible to
/// execute, so §5 spells it out as ASM-9's exception)" (sem: SEM-gx-engine-373)
/// -- because an undo that has only the digest of its own inverse cannot run. The exception is one
/// row wide and this is the row.
///
/// # Still no setters, now that T-12 exists
///
/// The status moves at T-12 and nowhere else (M5-16, adopted (a); sem: SEM-gx-engine-374), and hand 6 fires it -- **on the escrow
/// index, not on this value**. This struct is what [`BlobStore::escrowed`] rebuilds when a caller
/// wants the row *and* its body together; the index the engine mutates is
/// [`crate::EscrowRow`], which the journal reconstructs. So the promise hand 1 made ("`consumed_by`
/// will be added by the hand that fires the edge" (sem: SEM-gx-engine-375)) is kept by **not** adding one: the hand that
/// fires the edge found it had nothing here to set, and a setter would have been a second road to
/// a status the journal already fixes.
#[derive(Clone, Debug)]
pub struct EscrowedInverse {
    transformation: TransformationId,
    inverse_delta: Option<PlannedDelta>,
    retained_until: Option<Timestamp>,
    status: InverseStatus,
}

impl EscrowedInverse {
    /// Escrow an inverse that was constructed (43 T-10b).
    ///
    /// `retained_until` is `None` for the OSS default -- DR-9: "the OSS core's default is unlimited
    /// (deadline management/extension are commercial-tier features)" (sem: SEM-gx-engine-376) -- and `Some` is accepted so that the field is a *seat* rather than a
    /// promise. Nothing in v0.1 enforces the deadline (req/78 N-06).
    #[must_use]
    pub fn held(
        transformation: TransformationId,
        inverse_delta: PlannedDelta,
        retained_until: Option<Timestamp>,
    ) -> Self {
        Self {
            transformation,
            inverse_delta: Some(inverse_delta),
            retained_until,
            status: InverseStatus::Available,
        }
    }

    /// Record that no inverse could be constructed (42 §3.12, `invert()` returned `None`).
    ///
    /// A row with no delta rather than no row: "we asked and the answer was no" and "we never
    /// asked" are different facts, and a caller who has to tell them apart is exactly the caller
    /// an undo guarantee is for. The same distinction §32 M4H4-2 made between "not implemented" and
    /// "failed" (sem: SEM-gx-engine-377).
    #[must_use]
    pub fn unavailable(transformation: TransformationId) -> Self {
        Self {
            transformation,
            inverse_delta: None,
            retained_until: None,
            status: InverseStatus::Unavailable,
        }
    }

    /// Rebuild a row that was read back from a store, checking the invariant above.
    ///
    /// The checked constructor E-6 asks for ("reading a value back requires a checked constructor" (sem: SEM-gx-engine-378)), placed now
    /// because the store that will call it is hand 3 and a door built after its callers is a door
    /// somebody has already walked around.
    ///
    /// # Errors
    /// [`Error::InconsistentEscrow`] when `status` is `Unavailable` and a delta is present, or when
    /// it is any other value and none is.
    pub fn restore(
        transformation: TransformationId,
        inverse_delta: Option<PlannedDelta>,
        retained_until: Option<Timestamp>,
        status: InverseStatus,
    ) -> Result<Self> {
        let unavailable = matches!(status, InverseStatus::Unavailable);
        if unavailable != inverse_delta.is_none() {
            return Err(Error::InconsistentEscrow {
                detail: format!(
                    "status {} with {} inverse_delta",
                    status.kind(),
                    if inverse_delta.is_none() { "no" } else { "an" }
                ),
            });
        }
        Ok(Self {
            transformation,
            inverse_delta,
            retained_until,
            status,
        })
    }

    /// The committed transformation this inverse undoes.
    #[must_use]
    pub fn transformation(&self) -> TransformationId {
        self.transformation
    }

    /// The inverse itself, or `None` when [`InverseStatus::Unavailable`].
    #[must_use]
    pub fn inverse_delta(&self) -> Option<&PlannedDelta> {
        self.inverse_delta.as_ref()
    }

    /// The retention deadline, if a deployment set one (DR-9).
    #[must_use]
    pub fn retained_until(&self) -> Option<Timestamp> {
        self.retained_until
    }

    /// What has become of it.
    #[must_use]
    pub fn status(&self) -> &InverseStatus {
        &self.status
    }
}

// ---------------------------------------------------------------------------
// M5-09, adopted (a) -- the supersedes index (sem: SEM-gx-engine-379)
// ---------------------------------------------------------------------------

/// Which transformation's inverse superseded which (43 T-12, ASM-43-2).
///
/// # Why it is a type here rather than a map in the engine
///
/// ASM-43-2 asks for a `superseded_by` field "not included in the Transformation's own canonical
/// structure; treated as an index on the engine's side, so it does not break the receipt's
/// immutability (INV-S2)" (sem: SEM-gx-engine-380), and §37 fixes where it lives:
///
/// > **M5-09, adopted (a)**: the supersedes index is in store.rs; T-12's check is that the
/// > escrow's `inverse_delta` CID matches `T_u.delta`'s CID (sem: SEM-gx-engine-380)
///
/// A `BTreeMap` in `pipeline.rs` beside the state table would satisfy every behaviour and none of
/// that: the point of ASM-43-2 is that the edge is **not** part of the transformation, and a field
/// on the engine's own row is one refactor away from being written into one. Here it is a separate
/// structure with one writer and no way to unset an entry.
///
/// # 43 T-12's idempotency column, as a return value
///
/// "a duplicate supersede application by the same `T_u` is ignored (once `superseded_by` is set, it
/// is not set again)" (sem: SEM-gx-engine-381).
/// [`SupersedeIndex::record`] answers whether it wrote, so the caller can tell "the edge was drawn
/// now" from "the edge was already there" -- which is what keeps T-12 from journalling a second
/// `Superseded` record for one edge. req/29 §4's rule at the place a v0.1 would blur it.
#[derive(Clone, Debug, Default)]
pub struct SupersedeIndex {
    by: BTreeMap<TransformationId, TransformationId>,
}

impl SupersedeIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Draw the edge, unless it is already drawn. `true` when this call wrote it.
    ///
    /// Never overwrites: 43 §5-4 makes `T_o`'s record immutable and the index is the one place the
    /// supersede is recorded at all, so a second writer would be a second answer to "which change
    /// undid this one" (sem: SEM-gx-engine-382).
    pub fn record(&mut self, original: TransformationId, by: TransformationId) -> bool {
        if self.by.contains_key(&original) {
            return false;
        }
        self.by.insert(original, by);
        true
    }

    /// Which transformation superseded `original`, if one did.
    #[must_use]
    pub fn superseded_by(&self, original: &TransformationId) -> Option<TransformationId> {
        self.by.get(original).copied()
    }

    /// How many edges are drawn.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by.len()
    }

    /// Whether none are.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by.is_empty()
    }

    /// Every edge, by superseded transformation.
    pub fn iter(&self) -> impl Iterator<Item = (&TransformationId, &TransformationId)> {
        self.by.iter()
    }
}

// ---------------------------------------------------------------------------
// The journal itself
// ---------------------------------------------------------------------------

/// The engine's write-ahead log on a file (43 §7).
///
/// Holds the file, the records replayed from it, and what opening it found. The invariant the type
/// exists to hold is the ordering: a record is on the device before it is in the vector, so a
/// caller that can see a record can be sure the record survived the process.
#[derive(Debug)]
pub struct EngineJournal {
    file: File,
    path: PathBuf,
    records: Vec<EngineJournalRecord>,
    recovery: Recovery,
    /// 🔴 **T6 condition ② (catch-up)** — how many bytes of this file this process has already
    /// turned into records (`req/38` §148 ruling 1(i), `req/190` §2-1).
    ///
    /// Without it there is no answer to "what has arrived since I last looked": [`EngineJournal::
    /// open`] reads the file once and every later record comes from this process's own appends, so a
    /// second `gx` process writing to the same `.gx/` was invisible until a restart. That is
    /// `req/182` H-01, and its measured shape was worse than invisibility — `LedgerStore::append`
    /// stages the next index from an in-memory tree, so a stale reader appends a leaf at an index
    /// the file already holds and the replay after it truncates the tail (`req/190` F-5). The offset
    /// is what lets [`EngineJournal::catch_up`] read only the new bytes and stop guessing.
    consumed_bytes: u64,
    /// 🔴 **DR-43-7** — opened by [`EngineJournal::open_read_only`], so nothing here may write.
    read_only: bool,
    /// Where the torn tail this open removed was copied first, if it removed one (**DR-43-7**).
    quarantined: Option<PathBuf>,
    /// 🔴 **R4 / `req/225` H-03** — the last framed record of the prefix this process has read,
    /// as it lies on the file.
    ///
    /// `consumed_bytes` above answers "how much have I read", which is a length, and a length
    /// cannot tell a file that grew from a file that was rewritten at the same size.
    /// `gx_log::LedgerStore` has carried this second half since R3; the journal did not, and
    /// `req/225` H-03 measured the whole of what that cost — a live `gx serve` answering `200` on
    /// `/healthz`, `201` on `POST /candidates` and a **signed** checkpoint over a journal whose
    /// tail record had been rewritten, and a next start-up that refused to open the project.
    ///
    /// `None` for an empty journal, where the length check alone is exact.
    tail: Option<TailRecord>,
    /// 🔴 **R5 / DR-43-9** — which framing this file is in, sniffed when it was opened.
    ///
    /// [`JournalFormat::Legacy`] is a journal written before this format existed. It is read and
    /// appended to in its own framing — rewriting one into the chained framing would mean
    /// rewriting every byte of an append-only file, which is the one operation this type exists to
    /// make impossible — and what may be trusted in it is narrower by exactly the property the
    /// chain provides. 43 §7.6's R5 note carries the sentence: **in a legacy journal only the
    /// records the ledger backs are evidence**, because nothing on the file itself distinguishes a
    /// record gx wrote from a record somebody moved there.
    format: JournalFormat,
    /// 🔴 **R5 / DR-43-9** — the chain link after the last record this process has consumed.
    ///
    /// Thirty-two bytes that commit to the whole prefix, kept so that "is the file still the file I
    /// read" is a comparison of digests rather than of counts. `req/227` H-01 is the measurement of
    /// what counts miss: `prefix_replays` compared `good_bytes` and a record count, and a record
    /// copied from elsewhere in the same file preserves both — the audit did it with `cp` and no
    /// codec, and the same door took a payload's single flipped bit and two adjacent records
    /// swapped.
    ///
    /// `None` for [`JournalFormat::Legacy`].
    chain: Option<[u8; 32]>,
    /// 🔴 **R5 / DR-43-9** — where the file's chain stopped agreeing with itself, if it did.
    ///
    /// Absolute in the file. `Some` disables appending ([`EngineJournal::append`]) and makes
    /// `Engine::journal_intact` false, which folds into `Engine::ledger_agrees` and refuses at
    /// every gate that already asks it.
    chain_break: Option<u64>,
    /// 🔴 **R6 / `req/229` H-02** — this project declared itself chained and the file in front of
    /// us is not.
    ///
    /// The audit's second high finding is that **the chain can be taken off from the attacker's
    /// side**. Strip the eight-byte marker and every 32-byte link, write the records back in the
    /// old framing, and R5's whole apparatus stops existing: `gx repair` answered exit 0 with
    /// `journal_format: "legacy"` and `journal_intact: true`, `gx serve` started **without one word
    /// of warning**, appends continued in the old framing, and `req/227` H-01(a) — the rewrite the
    /// chain was built to catch — reproduced on the downgraded file with `/healthz` at `200`,
    /// `POST /candidates` at `201` and a signed checkpoint. Nothing anywhere recorded that the
    /// project had ever been chained.
    ///
    /// So the project records it, once, in `.gx/VERSION` (`gx_cli::layout`), and the declaration
    /// travels here as [`EngineJournal::open_declared`]'s argument. A project that has declared
    /// `chained` and presents a legacy file is treated exactly as a chain break is: **not
    /// truncated, not quarantined, not appended to**, and folded into `journal_intact` so the five
    /// existing gates refuse without a new `gx_code`. 42 §3.13 v0.4-r's backward compatibility is
    /// untouched — a project that has never declared a format opens as it always did — because what
    /// is refused is a **transition**, not a format.
    downgraded: bool,
    /// 🔴 **R30 / `req/372` M-02** — the file began with this project's marker prefix and a version
    /// this build does not know, so it was written by a newer `gx` and nothing here may touch it.
    from_a_newer_gx: bool,
}

/// 🔴 **R4 / `req/225` H-03** — where a journal's last framed record is, and what it says.
///
/// `gx_log::store`'s twin of this type, and separate from it for the reason the two `quarantine`
/// functions are separate: the crates do not depend on each other in that direction. Bytes rather
/// than a digest — a record is small, the comparison is exact, and 41 §6 keeps the hash in
/// gx-canon (`gx_canon::cid` is "the one place in the workspace where the hash is taken", and
/// `probes/doubt` asserts that call site is single).
#[derive(Clone, Debug)]
struct TailRecord {
    /// The offset the record's four-byte header starts at.
    at: u64,
    /// The framed bytes — header and payload — as they were written or replayed.
    framed: Vec<u8>,
}

impl EngineJournal {
    /// Open the journal at `path`, creating it if it is not there, and replay what it holds.
    ///
    /// A tail that cannot be replayed is removed from the file before this returns, so the next
    /// append lands where the record sequence actually reached. See [`Recovery`] for what is
    /// reported about it, and [`mod@crate::replay`] for why the refusal stops at the first bad record
    /// instead of skipping it. This is 43 §7-1's "replay at startup" (sem: SEM-gx-engine-383) with nothing decided on top: the
    /// records come back, and what they *mean* for the state table is hands 2 through 6.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be opened, read, truncated or synced.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_declared(path, None)
    }

    /// 🔴 **R6 / `req/229` H-02** — the same opening, with the format this project has declared.
    ///
    /// `None` is "this project has never said what it is", which is every project written before
    /// this release and every caller that has no `.gx/` to read a declaration out of (the engine's
    /// own tests). It behaves exactly as [`EngineJournal::open`] always has.
    ///
    /// `Some(JournalFormat::Chained)` over a file with no marker is the downgrade attack, and it is
    /// **not** a torn tail: stripping the marker leaves a file whose first four bytes are a legacy
    /// length header and whose remaining bytes are chained frames, so the legacy walk stops after
    /// one record and reports the other 98% as a tail. `req/229` M-01 measured the consequence —
    /// `gx serve` cut a 5,722-byte journal to 93 bytes on the way to refusing to start. So the
    /// truncation is skipped for the same reason DR-43-9 (c-3) skips it for a chain break: the
    /// bytes after the break are whole records, and cutting there deletes what nobody asked to
    /// lose.
    ///
    /// # Errors
    /// As [`EngineJournal::open`].
    pub fn open_declared(path: impl AsRef<Path>, declared: Option<JournalFormat>) -> Result<Self> {
        Self::open_declared_creating(path, declared, JournalCreation::Permitted)
    }

    /// 🔴 **R12 / `req/242` H-01 (d)** — the same opening, with "may this call bring the
    /// file into existence" as an argument instead of a constant.
    ///
    /// This is the only place in the workspace that creates `.gx/ledger/journal`. `req/242`
    /// measured what "always" cost: a project whose journal had been deleted was correctly refused
    /// a rebuild by `gx repair --yes` (the ledger's leaves were built *from* those records, so a
    /// journal composed here would be a witness statement gx wrote rather than one it kept) — and
    /// then a single `gx submit` created an eight-byte `GXJRNL01` through this door, after which
    /// `gx repair` reported `journal_absent: false`, `journal_commits: 0` and a rollback story
    /// instead of the loss.
    ///
    /// [`JournalCreation::Permitted`] is the default everywhere the engine is used as a library
    /// (`ProjectAnchor::none`, this crate's own fixtures): a caller with no `.gx/` to consult has
    /// no way to tell a new project from a damaged one, and answering `NotFound` to
    /// `EngineJournal::open` on a fresh path would break every embedder for a fact only the CLI
    /// holds. `gx-cli` passes [`JournalCreation::Refused`] on every road but
    /// `DeclarationWriter::initialise`.
    ///
    /// # Errors
    /// As [`EngineJournal::open`], and [`Error::Io`] with [`std::io::ErrorKind::NotFound`] when the
    /// file is not there and `creation` is [`JournalCreation::Refused`].
    pub fn open_declared_creating(
        path: impl AsRef<Path>,
        declared: Option<JournalFormat>,
        creation: JournalCreation,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let existed = path.exists();
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(matches!(creation, JournalCreation::Permitted))
            .open(&path)
            .map_err(|e| io_error("cannot open the journal", &path, &e))?;

        let mut bytes = Vec::new();
        {
            use std::io::Read;
            let mut reader = std::io::BufReader::new(&file);
            reader
                .seek(SeekFrom::Start(0))
                .map_err(|e| io_error("cannot rewind the journal", &path, &e))?;
            reader
                .read_to_end(&mut bytes)
                .map_err(|e| io_error("cannot read the journal", &path, &e))?;
        }

        // 🔴 **R5 / DR-43-9** — a file with no bytes gets the format marker, once, before anything
        // else is written to it. This is the only place the marker is written: every later append
        // continues a file that already carries it, and a journal that arrived without one stays in
        // its own framing (see [`EngineJournal::format`]).
        // 🔴 **R30 / `req/372` M-02** — a journal this build creates carries the marker the
        // project **declares**, and the v2 marker only when it declares nothing.
        //
        // The v2 default is because this build's record vocabulary is v2 (`Rollback::Diverged` is
        // in it). Obeying an existing declaration is because a project that declares less than its
        // journal holds can never trip the downgrade guard below — that guard fires when the
        // declared framing outranks the file's, so an under-stated declaration silently disarms
        // it. The first draft stamped v2 unconditionally and `model_a_probes.rs`'s half-made
        // project measured the result: `journal_format=chained` in `.gx/VERSION` over a
        // `GXJRNL02` file.
        //
        // Journals that already exist keep whatever marker they have: what is refused is a
        // transition, not a format.
        // 🔴 **R31 / `req/378` H-02** — the buffer follows the disk, so that the framing below has
        // exactly **one** source.
        //
        // Until R31 the marker was written to the file and `bytes` was left empty, and everything
        // after this point was derived from `replay(&bytes)` — which answers `ChainedV2` for an
        // empty buffer by definition. For a project declaring `chained` (every project made
        // between R6 and R29, which is precisely the population R30's vocabulary guard exists to
        // protect) the pair the thirtieth audit measured was `marker_on_disk="GXJRNL01"`,
        // `in_memory_format=ChainedV2`, `agree=false`, and three things followed from it: the
        // guard below compared `2 > 2` and did not fire, a `Diverged` record went into a v1-framed
        // file, and its link was minted over the **v2** genesis under a **v1** header — so the
        // next open walked from the v1 genesis and broke the chain at byte 8. One record written,
        // zero readable, and DR-43-9 forbids truncating at a break, so there was no road back.
        //
        // Extending `bytes` is the whole repair. It is not "handle the empty case": it removes the
        // second source. `format`, `chain`, `tail`, `downgraded` and `from_a_newer_gx` are all
        // read out of `replayed` below, `replayed` is `replay(&bytes)`, and `bytes` is now what
        // the file holds — including for the caller that arrives at a path with no file at all.
        // A later change that reintroduces a divergence has to put the second source back to do
        // it, and `crates/gx-engine/tests/r31_journal_format_from_disk.rs` asserts the single
        // predicate on every road into this door.
        if bytes.is_empty() {
            let marker = declared
                .and_then(JournalFormat::marker)
                .unwrap_or(JOURNAL_MAGIC_V2);
            (&file)
                .write_all(marker)
                .map_err(|e| io_error("cannot write the journal's format marker", &path, &e))?;
            barrier(&file, &path, "cannot fsync the journal's format marker")?;
            bytes.extend_from_slice(marker);
        }
        let replayed = replay(&bytes);
        let mut quarantined = None;
        let chain_break = replayed.chain_break();
        // 🔴 **R30 / `req/372` M-02** — asked **before anything is cut**, for the reason R6 asks
        // its own question there. `replay` reports a file from the future as `Legacy`, because it
        // is infallible and has no word for it, and walking a v3 file as legacy frames makes every
        // byte of it look like a torn tail. Cutting there is the twenty-ninth audit's finding with
        // the roles reversed — *this* build eating a newer one's history — so the answer is the
        // one R6 gave the downgrade: leave the bytes exactly where they lie and refuse.
        let from_a_newer_gx = crate::replay::framing_this_build_does_not_know(&bytes);
        // 🔴 **R6 / `req/229` H-02** — the declaration is compared before anything is cut.
        let downgraded = declared.is_some_and(|d| {
            d.is_chained() && d.vocabulary_rank() > replayed.format().vocabulary_rank()
        }) || (matches!(declared, Some(d) if d.is_chained())
            && replayed.format() == JournalFormat::Legacy);
        if replayed.recovery().torn_tail_bytes > 0 && !downgraded && !from_a_newer_gx {
            // 🔴 **DR-43-7 (`req/38` §153, `req/215` H-03/M-05)** — copy before cutting. Truncating
            // is still right here (this is the writer's door and the next append has to land where
            // the record sequence actually reached), and `gx_log::store`'s twin of this call says
            // the rest of why. What changes is that the removed bytes exist afterwards and that
            // `gx serve` prints where they went.
            //
            // 🔴 **R5 / DR-43-9** — unless the reason the bytes did not come back is a **chain
            // break**. A torn tail is a record that was being written when the process died; a
            // break is a whole record that is not the record this chain reached, and everything
            // after it is whole too. Cutting there would delete every record written after
            // somebody's edit and call it a repair — the amputation `req/225` H-01 measured one
            // verb up. The bytes are left exactly where they lie, the break is reported, and
            // `append` refuses (`Engine::journal_intact` is what every gate reads).
            if chain_break.is_none() {
                quarantined = Some(quarantine_torn_tail(
                    &path,
                    replayed.good_bytes(),
                    replayed.recovery().torn_tail_bytes,
                )?);
                file.set_len(replayed.good_bytes()).map_err(|e| {
                    io_error("cannot remove the torn tail of the journal", &path, &e)
                })?;
                barrier(&file, &path, "cannot fsync the truncated journal")?;
            }
        }
        if !existed {
            sync_parent_directory(&path)?;
        }

        let recovery = replayed.recovery();
        // 🔴 **R31 / `req/378` H-02** — no `+ stamped` term any more, and its absence is the same
        // repair rather than a separate one. `replay` counts a marker it read as replayed
        // (`good_bytes` includes it, which is what lets a caller truncate to `good_bytes` and keep
        // a readable file), so now that the stamped marker is in `bytes` it is already in this
        // number. Adding the old term back would double-count the eight bytes.
        let consumed_bytes = replayed.good_bytes();
        let tail = tail_record(&bytes, 0, replayed.tail_span());
        let format = replayed.format();
        let chain = replayed.chain();
        Ok(Self {
            file,
            path,
            records: replayed.into_records(),
            recovery,
            consumed_bytes,
            read_only: false,
            quarantined,
            tail,
            format,
            chain,
            chain_break: chain_break.map(|b| b.at),
            downgraded,
            from_a_newer_gx,
        })
    }

    /// 🔴 **DR-43-7** — open the journal to *read* it: no create, no truncate, no repair.
    ///
    /// [`EngineJournal::open`] is a writer's door and repairs a torn tail on the way through.
    /// `req/215` H-03 measured a reader — `gx replay <ID>`, the very verb the start-up gate's
    /// refusal message recommends — walking through it and shortening the file it was called to
    /// explain. A read has no lock, so what it would be cutting is not necessarily a crash's trace
    /// at all: it could be the record another `gx` is in the middle of appending.
    ///
    /// The count comes back through [`EngineJournal::recovery`] and the caller decides;
    /// `crates/gx-cli/src/replay.rs` refuses on a non-zero count rather than replaying a prefix and
    /// calling it the journal.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be opened or read.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_read_only_declared(path, None)
    }

    /// 🔴 **R6 / `req/229` H-02** — the reader's door, with the format this project has declared.
    ///
    /// The reader cuts nothing either way, so the declaration buys one thing here: `gx repair`'s
    /// *report* says `journal_intact: false` about a downgraded project instead of calling it
    /// healthy. That is the whole of the finding on this door — the audit's raw is a `gx repair`
    /// that answered exit 0 over a journal whose chain had been removed.
    ///
    /// 🔴 **R32 / `req/392` M-01** — and the declaration is now compared on a journal of **zero**
    /// bytes too, which it was not before this lane. Nothing on this door changed: what changed is
    /// [`crate::replay::replay`], which used to answer `ChainedV2` for an empty buffer and now
    /// answers `Legacy`, because that is what the disk says. The consequence here is that
    /// `downgraded` below is `true` for a project that declares a chain over a file carrying no
    /// marker — zero bytes included — instead of the guard comparing `1 > 2` and staying silent
    /// while `gx repair` printed `journal_intact_basis: "chain"` about a file holding no chain.
    /// This door still cuts nothing and writes nothing.
    ///
    /// # Errors
    /// As [`EngineJournal::open_read_only`].
    pub fn open_read_only_declared(
        path: impl AsRef<Path>,
        declared: Option<JournalFormat>,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|e| io_error("cannot open the journal", &path, &e))?;
        let mut bytes = Vec::new();
        {
            use std::io::Read;
            let mut reader = std::io::BufReader::new(&file);
            reader
                .read_to_end(&mut bytes)
                .map_err(|e| io_error("cannot read the journal", &path, &e))?;
        }
        let replayed = replay(&bytes);
        let recovery = replayed.recovery();
        let consumed_bytes = replayed.good_bytes();
        let tail = tail_record(&bytes, 0, replayed.tail_span());
        let format = replayed.format();
        let chain = replayed.chain();
        let chain_break = replayed.chain_break().map(|b| b.at);
        Ok(Self {
            file,
            path,
            records: replayed.into_records(),
            recovery,
            consumed_bytes,
            read_only: true,
            quarantined: None,
            tail,
            format,
            chain,
            chain_break,
            downgraded: declared
                .is_some_and(|d| d.is_chained() && d.vocabulary_rank() > format.vocabulary_rank())
                || (matches!(declared, Some(d) if d.is_chained())
                    && format == JournalFormat::Legacy),
            // 🔴 **R30 / `req/372` M-02** — a read-only door cuts nothing anyway, but the fact is
            // carried so that `gx repair`'s report can name it instead of describing a file from
            // the future as a damaged one.
            from_a_newer_gx: crate::replay::framing_this_build_does_not_know(&bytes),
        })
    }

    /// 🔴 **R30 / `req/372` M-02** — this file carries a framing marker this build has never heard
    /// of, so the records inside it were written by a newer `gx`.
    ///
    /// Folded into `Engine::journal_intact` beside `downgraded` and for the same reason: what a
    /// build cannot read it must not extend, and the five existing gates already refuse on that
    /// flag without needing a new `gx_code`. Nothing was truncated and nothing was quarantined —
    /// the bytes are exactly where the newer binary left them.
    #[must_use]
    pub fn from_a_newer_gx(&self) -> bool {
        self.from_a_newer_gx
    }

    /// 🔴 **R5 / DR-43-9** — which framing this journal is in.
    #[must_use]
    pub fn format(&self) -> JournalFormat {
        self.format
    }

    /// 🔴 **R5 / DR-43-9** — whether the chain on the file verifies end to end.
    ///
    /// `true` for a chained file whose every stored link is the link its payload and its
    /// predecessors produce, and `true` for a [`JournalFormat::Legacy`] file, which carries no
    /// links and therefore cannot contradict itself. The second `true` is a **declaration and not
    /// a pass**: a legacy journal has no identity of its own, which is why 43 §7.6's R5 note says
    /// only the records the ledger backs are evidence there, and why `Engine::recover` refuses to
    /// re-apply a delta the ledger does not still put last.
    #[must_use]
    pub fn chain_intact(&self) -> bool {
        self.chain_break.is_none()
    }

    /// 🔴 **R5 / DR-43-9** — where the chain broke, in the file.
    #[must_use]
    pub fn chain_break(&self) -> Option<u64> {
        self.chain_break
    }

    /// 🔴 **R6 / `req/229` H-02** — whether this project declared a chained journal and got a
    /// legacy one.
    #[must_use]
    pub fn downgraded(&self) -> bool {
        self.downgraded
    }

    /// 🔴 **R6 / DR-43-11** — the chain head over the file's first `len` bytes, re-read now.
    ///
    /// `None` for "there is no answer to give": the file is shorter than `len`, will not open, or
    /// its walk stopped before `len` (a break, or a frame that runs past the boundary). `Some(None)`
    /// is a legacy file, which carries no links and therefore no head. `Some(Some(head))` is the
    /// 32 bytes the prefix produces.
    ///
    /// The re-read is deliberate and it is paid once per open rather than per write: `open` has
    /// already dropped the bytes it replayed, and a field carrying "the head as of some earlier
    /// length" would be a second answer to a question the file can be asked directly. What it costs
    /// is one extra pass over the prefix at start-up — the same `O(file)` shape 43 §7.7 (c-2)
    /// already declares, on a road that runs once.
    #[must_use]
    pub fn chain_head_through(&self, len: u64) -> Option<Option<[u8; 32]>> {
        if self.format == JournalFormat::Legacy {
            return Some(None);
        }
        let Ok(mut file) = File::open(&self.path) else {
            return None;
        };
        let mut bytes = vec![0u8; usize::try_from(len).ok()?];
        match fill_exact(&mut file, &mut bytes) {
            Ok(read) if read == bytes.len() => {}
            _ => return None,
        }
        let walk = crate::replay::walk_links(&bytes);
        if walk.break_at.is_some() || walk.good_bytes != len {
            return None;
        }
        Some(walk.head)
    }

    /// 🔴 **R4 / `req/225` H-03** — whether the file's last framed record is still the one this
    /// store holds.
    ///
    /// `true` for an empty journal (the length check is exact there), `true` when the region
    /// re-reads byte-for-byte, and `false` for everything else **including an I/O failure**: a
    /// detector that cannot read is a detector that has not checked, and the caller's response to
    /// both is the same — treat the file as one this process has not read.
    ///
    /// A fresh read handle is opened rather than seeking `self.file`, because that handle is in
    /// append mode and shared with the writer; moving its cursor to answer a question would be a
    /// read with a side effect. `gx_log::LedgerStore::tail_unchanged` says the same thing about the
    /// other file, and this is deliberately its twin rather than its generalisation: the two
    /// crates hold their own framing.
    #[must_use]
    pub fn tail_unchanged(&self) -> bool {
        let Some(tail) = &self.tail else {
            return true;
        };
        let Ok(mut file) = File::open(&self.path) else {
            return false;
        };
        if file.seek(SeekFrom::Start(tail.at)).is_err() {
            return false;
        }
        let mut found = vec![0u8; tail.framed.len()];
        match fill_exact(&mut file, &mut found) {
            Ok(read) if read == found.len() => found == tail.framed,
            _ => false,
        }
    }

    /// 🔴 **R4 / `req/225` H-03, the half [`EngineJournal::tail_unchanged`] cannot have** — does
    /// everything this process has already read still replay as the same number of whole records?
    ///
    /// `O(file)` and therefore **for the writer's road only**. It is `gx_log`'s "under the lock,
    /// re-open unconditionally" (43 §7.5) in the shape this side can afford: the bytes are read
    /// and replayed, but the records are *not* re-folded into Σ — the shadow is an incremental
    /// fold and rebuilding it here would be a second reconstruction of the state table on every
    /// write. What is compared is the shape of the prefix: the same byte count came back as whole
    /// records, and there are the same number of them.
    ///
    /// What that catches is the failure `req/225` H-03 measured from the middle of the file
    /// (offset 40 of 1,636 bytes: the next start-up quarantined the **whole** journal). ~~What it
    /// does not catch is a rewrite that substitutes one whole, canonically-encoded record for
    /// another of exactly the same length in the middle of the file — that one needs a forger with
    /// this workspace's codec, and it is declared here rather than discovered later.~~ The tail is
    /// exact, so the same substitution **in the last record** is caught.
    ///
    /// # 🔴 **R5 / `req/227` H-01** — the struck sentence was false in three ways, and the shape
    /// check is gone
    ///
    /// It named an attacker who needs a codec. `req/227` measured that the codec is not needed:
    /// one commit lays down ten records of the same framed lengths every time, so from the second
    /// commit onward **every record has a same-length, canonically-encoded twin already inside the
    /// file**, and the tool is `cp`. It named one class. What the two comparisons actually admitted
    /// was every rewrite preserving a byte count and a record count — the audit walked through with
    /// a copy, with two adjacent records **swapped**, and with a single bit flipped inside a
    /// payload, and all three left `/healthz` at `200`, `POST /candidates` at `201` and the
    /// checkpoint **signed**. And it named a consequence that stopped at detection: where the
    /// substituted record was a `Committed`, the next start-up's `recover` read the row as an
    /// unfinished commit and **re-applied its delta to the substrate**, taking an operator's file
    /// from `three` back to `one`.
    ///
    /// So the prefix is compared by **identity**: [`crate::replay::link`]'s chain is walked over
    /// the consumed bytes and its head is compared with the 32 bytes this store has been carrying
    /// since it read them. Any single-byte difference anywhere in the prefix changes the head, and
    /// the walk needs no CBOR decode at all — which is also why the road is affordable enough to
    /// run on a read (see `Engine::read_to_the_end`).
    ///
    /// [`JournalFormat::Legacy`] keeps the old shape comparison, because a file with no links has
    /// nothing else to offer. It is the weaker answer and it is labelled as one.
    #[must_use]
    pub fn prefix_intact(&self) -> bool {
        if self.consumed_bytes == 0 {
            return true;
        }
        let Ok(mut file) = File::open(&self.path) else {
            return false;
        };
        let mut bytes = vec![0u8; self.consumed_bytes as usize];
        match fill_exact(&mut file, &mut bytes) {
            Ok(read) if read == bytes.len() => {}
            _ => return false,
        }
        match self.format {
            // 🔴 The chained road decodes **nothing**: the link over every record is the whole of
            // the comparison, and a CBOR decode per record would be paid on every write and every
            // read for an answer the chain already gives.
            JournalFormat::Chained | JournalFormat::ChainedV2 => {
                let walk = crate::replay::walk_links(&bytes);
                walk.break_at.is_none()
                    && walk.good_bytes == self.consumed_bytes
                    && walk.records == self.records.len() as u64
                    && walk.head == self.chain
            }
            // A file with no links has nothing but its shape, so this is R4's comparison, kept for
            // the files R4 wrote and no stronger than it was.
            JournalFormat::Legacy => {
                let replayed = replay(&bytes);
                replayed.good_bytes() == self.consumed_bytes
                    && replayed.recovery().records == self.records.len() as u64
            }
        }
    }

    /// Where [`EngineJournal::open`] copied the torn tail before it cut it, if it cut one.
    ///
    /// 🔴 **DR-43-7 / `req/215` M-05.** `gx serve` puts this in its start-up line beside
    /// `torn_tail_bytes`: `req/215` probe (b) appended 22 bytes of rubbish to a journal, watched
    /// `gx serve` start normally and cut the file from 3140 bytes to 3118, and found not one word
    /// about it in the start-up line or on stderr.
    #[must_use]
    pub fn quarantined(&self) -> Option<&Path> {
        self.quarantined.as_deref()
    }

    /// Append one record, durably, and answer with its sequence number.
    ///
    /// The three statements are the write-ahead property (43 §7): **encode**, **write and wait for
    /// the device**, **then** make it visible. `tests/journal_roundtrip.rs` reads that order off
    /// this source, because it is not observable from outside -- a caller cannot see the difference
    /// between a record that was pushed before the fsync and one pushed after, until the power
    /// fails.
    ///
    /// Not keyed and not idempotent, unlike `gx_log::LedgerStore::append`: a journal is a sequence
    /// of *events*, and two `VerifyStarted` records for one transformation is a fact about a
    /// re-entry rather than a contradiction. Idempotence lives at 43 ASM-43-1, which is the
    /// ledger's, and the guards that keep a transition from firing twice are hands 2 through 6.
    ///
    /// # Errors
    /// [`Error::Canon`] if the record has no canonical form. [`Error::Malformed`] if the encoded
    /// record is larger than [`MAX_RECORD_BYTES`]. [`Error::Io`] if it cannot be written or synced.
    pub fn append(&mut self, record: EngineJournalRecord) -> Result<u64> {
        // 🔴 **DR-43-7** — a journal opened to read is not a journal to write through.
        if self.read_only {
            return Err(io_error(
                "cannot append to a journal opened read-only (DR-43-7)",
                &self.path,
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ));
        }
        // 🔴 **R5 / DR-43-9** — a journal whose chain is broken is not one to extend. Appending
        // would put a good record on top of a file that already contradicts itself, which is the
        // shape `req/227` H-01 measured all the way to a signed checkpoint over rewritten bytes.
        // The refusal is here, at the write itself, so that no gate can be reached around.
        // 🔴 **R6 / `req/229` H-02** — a project that declared itself chained and presents a legacy
        // file is not one to extend either. Appending here would continue the attacker's framing
        // and make the downgrade permanent: the audit watched a fourth commit land in the old
        // framing and `chained=False` afterwards.
        if self.downgraded {
            return Err(Error::Malformed {
                detail: format!(
                    "{} declares a chained journal (`.gx/VERSION`) and the file on the disk has no \
                     format marker: the chain was removed after this project was written \
                     (req/229 H-02, DR-43-11). Appending would continue in the framing somebody \
                     else chose, so this refuses. The bytes are left exactly where they lie; \
                     `gx repair` reports it",
                    self.path.display(),
                ),
            });
        }
        // 🔴 **R30 / `req/372` M-02** — a journal from a newer `gx` is not one to extend either,
        // and it is the case the twenty-ninth audit measured from the other side: an older binary
        // that walked such a file as legacy frames called all of it a torn tail, cut it, and after
        // one append the result *looked healthy*. Appending here is the step that turns a
        // recoverable misreading into a rewritten history, so it is the step that refuses.
        if self.from_a_newer_gx {
            return Err(Error::Malformed {
                detail: format!(
                    "{} begins with a journal format marker this build does not know, so its \
                     records were written by a newer gx and this build cannot read them (req/372 \
                     M-02, DR-43-9). This is **not** a damaged file and nothing has been removed \
                     or copied: the bytes are exactly where the newer binary left them. Appending \
                     would frame new records in a format this build cannot verify, so this \
                     refuses. Use the gx that wrote it",
                    self.path.display(),
                ),
            });
        }
        // 🔴 **R30 / `req/372` M-02** — and a record whose vocabulary is newer than the journal's
        // framing does not go into that journal.
        //
        // This is the guard that makes the marker mean something. Without it a project created by
        // an older binary keeps its v1 marker forever, this build appends `Diverged` into it, and
        // the older binary meets exactly the file the audit measured — the marker having promised
        // a vocabulary the contents do not keep. The chain cannot be re-framed in place (the
        // genesis link is minted over the marker, so changing it invalidates every link after it),
        // so the honest answer is to refuse the record and say why.
        //
        // 🔴 What it costs, stated rather than hidden: on a project made before this release, an
        // outcome that needs a v2 word cannot be journalled, and the verb fails instead of
        // recording an abort. `CHANGELOG.md` §3 and `docs/LIMITS.md` carry it. The exposure is
        // small **and it is small for a measured reason, not a hopeful one**: R30's other half
        // removed the roads on which `Diverged` was easiest to reach, so the shipped adapters now
        // reach it only through the residual window v0.5-q measures.
        if record.minimum_format().vocabulary_rank() > self.format.vocabulary_rank() {
            return Err(Error::Malformed {
                detail: format!(
                    "{} is a `{}` journal and this record carries a word that framing does not \
                     cover, so it is not appended (req/372 M-02). A binary older than this one \
                     would not decode the record and would report the rest of the file as a torn \
                     tail -- the twenty-ninth audit measured that costing two records of live \
                     history. This project was created before the vocabulary grew; its journal \
                     keeps its own framing and cannot be re-framed in place, because the chain's \
                     genesis link is minted over the marker",
                    self.path.display(),
                    self.format.kind(),
                ),
            });
        }
        if let Some(at) = self.chain_break {
            return Err(Error::Malformed {
                detail: format!(
                    "{} holds a record at byte {at} whose chain link is not the link its contents \
                     produce: the file was rewritten after it was written (DR-43-9). Appending \
                     here would extend a contradiction, so this refuses. `gx repair` reports it; \
                     the bytes are left exactly where they lie",
                    self.path.display(),
                ),
            });
        }
        let payload = cbor::encode(&record)?;
        // 🔴 **R5 / DR-43-9** — the link that will follow the payload on the file, computed before
        // anything is written so that a failure to encode cannot advance the chain.
        // 🔴 **R30 / `req/372` M-02** — both chained framings seal a record the same way. The
        // version changes which genesis the chain **started** from, which is carried in
        // `self.chain`, and not how one link follows another.
        let sealed = match (self.format, self.chain) {
            (JournalFormat::Chained | JournalFormat::ChainedV2, Some(previous)) => {
                Some(crate::replay::link(&previous, &payload))
            }
            (JournalFormat::Chained | JournalFormat::ChainedV2, None) => {
                return Err(Error::Malformed {
                    detail: format!(
                        "{} is a chained journal with no chain head to continue from (DR-43-9)",
                        self.path.display(),
                    ),
                })
            }
            (JournalFormat::Legacy, _) => None,
        };
        self.write_and_sync(&payload, sealed.as_ref())?;
        self.records.push(record);
        // 🔴 **R4 / `req/225` H-03** — the record just written *is* the last record, so it is what
        // the next `tail_unchanged` expects to find. Framed exactly as `write_and_sync` framed it,
        // or the check would compare this process's idea of the framing against the file's.
        let mut framed = Vec::with_capacity(LENGTH_BYTES + payload.len() + CHAIN_BYTES);
        framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        framed.extend_from_slice(&payload);
        if let Some(link) = &sealed {
            framed.extend_from_slice(link);
        }
        self.tail = Some(TailRecord {
            at: self.consumed_bytes,
            framed,
        });
        // The bytes this record put on the file are bytes this process has already read: advancing
        // here is what keeps [`EngineJournal::catch_up`] from folding our own append a second time.
        self.consumed_bytes += (LENGTH_BYTES + payload.len()) as u64;
        if let Some(link) = sealed {
            self.consumed_bytes += CHAIN_BYTES as u64;
            self.chain = Some(link);
        }
        Ok(self.records.len() as u64 - 1)
    }

    /// 🔴 **T6 condition ② (catch-up)** — read whatever another process appended since we last
    /// looked, and answer with the new records (`req/38` §148 ruling 1(i)).
    ///
    /// Called by [`crate::pipeline::Engine::catch_up`] immediately after the writer's `.gx/LOCK` is
    /// held, which is the whole of the ordering rule: **a writer reads to the end of the log before
    /// it writes.** That sentence is the linearized-log axiom's implementation side (`req/190` §6
    /// row 1), and it is why the lock is per-operation rather than per-process — `gx serve` and
    /// `gx wrap` are both long-lived engines (`req/190` F-7), so a lock held for a process's life
    /// would make the two structurally unable to share a project.
    ///
    /// # 🔴 A torn tail here is refused, not truncated
    ///
    /// [`EngineJournal::open`] truncates a tail that will not replay, because at open time there is
    /// no other writer by construction and a half-written record is a crash's ordinary shape. Here
    /// there **is** another writer — that is why we are reading — and a short read of a record
    /// another process is in the middle of appending is not damage. Truncating it would delete a
    /// live writer's record, which is exactly the "silently cuts the following leaves" failure
    /// `req/190` F-5 measured on the ledger side. So the bytes are left alone, the offset does not
    /// advance past them, and the caller is told (`req/182` H-08's quarantine shape, in the small).
    ///
    /// # 🔴 A file shorter than our offset is refused, not shrugged at
    ///
    /// `req/215` M-07 read the sentence that used to be here — "`Engine::catch_up` asks
    /// `ledger_agrees` right after this and that is where the disagreement is refused" — and then
    /// measured the gx-api path, where nothing asked (`req/215` H-01). The doc was describing a
    /// property the code did not have. Both halves are fixed: `AppState::engine_for_write` now asks
    /// the same question `Session::settle` asks, **and** a journal that has become shorter than the
    /// bytes this process has already read is refused here, where it is *seen*, rather than being
    /// left to a later check that may or may not run. A log that is append-only cannot shrink; one
    /// that has shrunk was replaced or truncated underneath a live writer, and appending to it would
    /// write record `n` twice.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be read. [`Error::Malformed`] if the bytes past our offset
    /// do not replay whole, or if the file is shorter than the bytes already read.
    pub fn catch_up(&mut self) -> Result<&[EngineJournalRecord]> {
        self.read_to_the_end(true)
    }

    /// 🔴 **DR-43-6 / `req/215` H-05** — the same read, for a caller that holds no lock.
    ///
    /// `GET` handlers answer under gx-api's `Mutex` and deliberately do **not** take `.gx/LOCK`
    /// (a read that took it would answer `503` while a CLI verb was writing, and `/healthz` would
    /// fail for a project that is perfectly well). That makes two of this function's refusals wrong
    /// for them, because both describe a race rather than damage:
    ///
    /// * a **torn tail** is, without the lock, most likely the record another `gx` is appending
    ///   right now — so the whole records before it are folded and the offset stops in front of it;
    /// * a **shorter file** is a disagreement this caller cannot resolve and must not paper over —
    ///   so nothing is folded, the offset does not move, and `Engine::ledger_agrees` is left to say
    ///   so to whoever asks (`handlers::ledger_checkpoint` asks before it signs).
    ///
    /// Neither is silence: what a read gets is the log's whole-record prefix, which is what an
    /// append-only file guarantees a lockless reader and no more.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be read.
    pub fn catch_up_unlocked(&mut self) -> Result<&[EngineJournalRecord]> {
        self.read_to_the_end(false)
    }

    /// The body of [`EngineJournal::catch_up`] and [`EngineJournal::catch_up_unlocked`].
    ///
    /// `under_lock` is the only difference and it is a difference about *authority*, not about
    /// carefulness: a caller holding `.gx/LOCK` knows there is no concurrent writer, so anything
    /// unexpected in the bytes is damage and is refused; a caller without it cannot tell damage from
    /// a half-finished append, so it stops at the last whole record instead of accusing anybody.
    fn read_to_the_end(&mut self, under_lock: bool) -> Result<&[EngineJournalRecord]> {
        // 🔴 **R6 / `req/229` H-02 + M-01** — a downgraded journal has nothing to catch up on and
        // nothing to accuse anybody of.
        //
        // Stripping the eight-byte marker turns a chained file into a legacy one whose framing
        // breaks after the first record, so `open` reports 98% of it as a tail. Before R6 the
        // writer's door **cut** that tail (M-01: 5,722 bytes to 93). R6 stops the cut, which leaves
        // the bytes in place — and then this function, under the lock, called them "bytes appended
        // that do not replay" and refused with `Error::Malformed`, which `gx serve` answered as
        // `INTERNAL`. 44 §2.3's `INTERNAL` is "not classifiable" and this is entirely classifiable:
        // the answer is `journal_intact: false` folding into `LEDGER_DISAGREES`, which the gates
        // already give. So the read is a no-op and the refusal happens where refusals happen.
        if self.downgraded {
            let known = self.records.len();
            return Ok(&self.records[known..]);
        }
        let end = self
            .file
            .metadata()
            .map_err(|e| io_error("cannot measure the journal", &self.path, &e))?
            .len();
        let known = self.records.len();
        if end < self.consumed_bytes {
            if under_lock {
                return Err(Error::Malformed {
                    detail: format!(
                        "{} is {} bytes long and this process has already read {}: an append-only \
                         journal cannot shrink, so it was replaced or truncated underneath a live \
                         writer. Appending here would write a record index the file already holds \
                         (req/215 M-07, DR-43-6)",
                        self.path.display(),
                        end,
                        self.consumed_bytes,
                    ),
                });
            }
            return Ok(&self.records[known..]);
        }
        if end == self.consumed_bytes {
            return Ok(&self.records[known..]);
        }
        let mut bytes = vec![0u8; (end - self.consumed_bytes) as usize];
        {
            use std::io::Read;
            let mut reader = std::io::BufReader::new(&self.file);
            reader
                .seek(SeekFrom::Start(self.consumed_bytes))
                .map_err(|e| io_error("cannot seek the journal", &self.path, &e))?;
            reader
                .read_exact(&mut bytes)
                .map_err(|e| io_error("cannot read the journal", &self.path, &e))?;
        }
        // 🔴 **R5 / DR-43-9** — the arriving bytes hold no format marker (it is at the file's
        // start, behind this offset), so the framing and the link to continue from come from this
        // store. A record another process appended is verified against **our** head, which is what
        // makes a second writer's records evidence rather than assertion.
        let replayed = crate::replay::replay_onward(
            &bytes,
            crate::replay::Resume {
                format: self.format,
                chain: self.chain,
            },
        );
        if let Some(break_at) = replayed.chain_break() {
            self.chain_break = Some(self.consumed_bytes + break_at.at);
        }
        // 🔴 The count is bound before the branch, and not only for reading. `tests/
        // lifecycle_transitions.rs::the_replay_and_open_boundaries_stay_strict` pins the literal
        // `if replayed.recovery().torn_tail_bytes > 0 {` to **one** occurrence in this file, because
        // that line is `EngineJournal::open`'s truncation boundary and a K6 run left its `>` as a
        // surviving equivalent mutant. A second occurrence of the same text here would make the
        // scan count two and could not tell them apart — and they are not the same decision at all:
        // `open` **truncates** a torn tail, this **refuses** one. Different verb, different line.
        let torn = replayed.recovery().torn_tail_bytes;
        // 🔴 **R5 / DR-43-9** — a chain break is not "bytes another process is halfway through
        // appending", and it must not be answered with the sentence for that.
        //
        // The refusal below is `Error::Malformed`, which every surface maps to `INTERNAL` — 44
        // §2.3's word for "not classifiable" — and a rewritten journal is entirely classifiable:
        // it is `LEDGER_DISAGREES`, which is what `journal_intact` folds into and what every other
        // face of this condition already says (`req/38` §156 ruling 2(a)). Measured on this lane's
        // own probe: a `gx serve` on a project whose `Committed` record had been replaced refused
        // to start with `{"gx_code":"INTERNAL"}` and a stderr line about bytes that "do not replay
        // as whole records", which is a sentence about a crash. So the break is recorded and the
        // records before it are returned; the refusal happens one layer up, in the word the
        // operator already knows.
        let broken = replayed.chain_break().is_some();
        if torn > 0 && under_lock && !broken {
            return Err(Error::Malformed {
                detail: format!(
                    "{} bytes appended to {} by another process do not replay as whole records ({} did): a writer holding `.gx/LOCK` sees a complete log or refuses, because truncating here would delete a record this process did not write (req/190 F-5, DR-43-2)",
                    torn,
                    self.path.display(),
                    replayed.good_bytes(),
                ),
            });
        }
        // 🔴 **R4 / `req/225` H-03** — the arriving region's last whole record becomes this
        // store's tail. Taken **before** the offset moves, because `tail_record`'s base is where
        // the region began on the file. A region that held no whole record leaves the previous
        // tail standing, which is right: nothing was consumed, so nothing this store has read has
        // changed.
        if let Some(tail) = tail_record(&bytes, self.consumed_bytes, replayed.tail_span()) {
            self.tail = Some(tail);
        }
        // 🔴 **R5 / DR-43-9** — the head moves with the records, and only over the records that
        // were folded.
        if replayed.chain().is_some() {
            self.chain = replayed.chain();
        }
        self.consumed_bytes += replayed.good_bytes();
        self.records.extend(replayed.into_records());
        Ok(&self.records[known..])
    }

    /// How many bytes of the file this process has turned into records — where
    /// [`EngineJournal::catch_up`] starts reading.
    ///
    /// 🔴 Named `read_offset` and not `consumed_bytes` for a reason a reader would otherwise have to
    /// rediscover: `tests/escrow_types.rs::nothing_in_this_hand_moves_a_status` scans this file for
    /// `fn consume`, because M5-16 puts the escrow status write at T-12 in `pipeline.rs` and a
    /// mutator declared here would be a second place it could move. `fn consumed_bytes` matched that
    /// substring and turned the probe red for a function that has nothing to do with escrow. The
    /// probe is right to be blunt; the name moved instead.
    #[must_use]
    pub fn read_offset(&self) -> u64 {
        self.consumed_bytes
    }

    /// The records this journal holds, oldest first.
    #[must_use]
    pub fn records(&self) -> &[EngineJournalRecord] {
        &self.records
    }

    /// How many records it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The file the journal is stored in.
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
    /// One `write_all` for header and payload together, so a record is one syscall and the window a
    /// crash can land inside is as small as the platform allows. It is not zero -- a partially
    /// written record is exactly what [`crate::replay`] refuses.
    fn write_and_sync(&mut self, payload: &[u8], sealed: Option<&[u8; 32]>) -> Result<()> {
        let length = u32::try_from(payload.len())
            .ok()
            .filter(|n| *n <= MAX_RECORD_BYTES)
            .ok_or_else(|| Error::Malformed {
                detail: format!(
                    "a record of {} bytes is over the {MAX_RECORD_BYTES}-byte ceiling",
                    payload.len()
                ),
            })?;

        let mut record = Vec::with_capacity(LENGTH_BYTES + payload.len() + CHAIN_BYTES);
        record.extend_from_slice(&length.to_be_bytes());
        record.extend_from_slice(payload);
        // 🔴 **R5 / DR-43-9** — header, payload and link in the same `write_all`, so a record and
        // the statement that it is this record's turn in the chain reach the device together.
        if let Some(link) = sealed {
            record.extend_from_slice(link);
        }

        self.file
            .write_all(&record)
            .map_err(|e| io_error("cannot write to the journal", &self.path, &e))?;
        barrier(&self.file, &self.path, "cannot fsync the journal")
    }
}

// ---------------------------------------------------------------------------
// The writer's lock
// ---------------------------------------------------------------------------

/// 🔴 **T6 condition ② (single writer, per operation)** — the advisory lock a process holds while it
/// writes to one `.gx/` (`req/38` §148 ruling 1(ii), designed in `req/190` §3).
///
/// # What it is for, measured rather than feared
///
/// `req/182` H-01 ran `gx serve` and a `gx` CLI over one project and read the outcome off the files:
/// the second writer's leaf went in at an index the first writer's in-memory tree had already
/// staged, the next open truncated the tail, and `gx log proof --leaf 2` answered `found: false` for
/// a commit that had returned `200`. The append itself is atomic (both logs are `O_APPEND` and one
/// `write_all` per record), so what two processes corrupt is not bytes — it is the **index**, which
/// is derived from a copy of the log each process is holding in memory. The repair is therefore not
/// a better write but an order: hold the lock, read to the end of the log, then write.
///
/// # Per operation, and why a process-lifetime lock was refused
///
/// `req/190` F-7 measured that `gx wrap` holds an engine for the life of an MCP session, exactly as
/// `gx serve` holds one for the life of a server. The GUI premise ("the GUI runs `gx serve`, the
/// agent runs `gx wrap`") is therefore **two long-lived writers by construction**, and a lock taken
/// in `Engine::open` and held until the process exits would make that premise structurally
/// impossible. So the lock is taken around each engine operation and released after it: two writers
/// coexist, and only two writes at the same instant collide.
///
/// # A refusal, not a wait
///
/// [`ProcessLock::acquire`] never blocks. A `gx` that waited would turn a contended `.gx/` into a
/// hang with no output, and 44 §1.3 has no shape for "still waiting". `req/38` §148 rules the
/// refusal's face: `BUSY`, CLI exit **1** (no new exit number), HTTP `503` with a `Retry-After`, so
/// that a caller's retry policy has a word to branch on. Whoever holds it writes its pid and verb
/// into the file, which is diagnosis only — the meaning is carried by the lock and never by the
/// bytes.
///
/// # The engine does not take it (M5H5-6, adopted (a))
///
/// `Engine` holds no lock of its own and this type is not reachable from one: the layer that knows
/// there is more than one process is the layer that opened the project, which is `gx-cli` for a verb
/// and `gx-api`'s `AppState` for a server. The type lives beside the journal because the thing it
/// protects is the journal, not because the engine takes it.
///
/// # 🔴 Declared limits
///
/// * **`.gx/LOCK` is not in `GX_PATHS`.** req/56 §2, `gx-cli`'s `layout::GX_PATHS` and
///   `probes/doubt/tests/m6_surface_doubt.rs::the_dotgx_layout_is_req56_exactly` are one list checked
///   three ways, and two of those three are outside this lane's write scope. The row is owed and is
///   raised as part of **DR-43-5** rather than added on one side, which would turn the probe red for
///   a reason that has nothing to do with the lock.
/// * **Advisory on Unix, mandatory on Windows.** `std`'s lock is `flock` on Unix and `LockFileEx` on
///   Windows; a process that never asks is never stopped on Unix. Every `gx` writer asks.
/// * **Not measured on Windows, on 9p (`/mnt/c`) or under a sync client.** `req/190` §3-2 raised all
///   three and this lane did not close them: the E2E is `cfg(unix)` on ext4. A `.gx/` on a share
///   whose lock is not honoured degrades to today's behaviour (H-01), which is why `ledger_agrees`
///   is checked as well as the lock held.
#[derive(Debug)]
pub struct ProcessLock {
    file: File,
    path: PathBuf,
}

impl ProcessLock {
    /// Open (creating if absent) the lock file at `path`. Takes no lock.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be created or opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| io_error("cannot open the writer lock", &path, &e))?;
        Ok(Self { file, path })
    }

    /// Take the lock for one operation, or refuse.
    ///
    /// `holder` is written into the file for a human reading it later ("pid 4213, `gx commit`") and
    /// is never read back as meaning.
    ///
    /// # Errors
    /// [`Error::Busy`] if another process holds it. [`Error::Io`] if the platform refused for any
    /// other reason.
    pub fn acquire(&self, holder: &str) -> Result<LockHeld<'_>> {
        self.take(holder)?;
        Ok(LockHeld { lock: self })
    }

    /// The `try_lock` itself, shared by the borrowed and the owned form so that there is one
    /// refusal and one diagnostic note rather than two spellings of each.
    fn take(&self, holder: &str) -> Result<()> {
        match self.file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(Error::Busy {
                    path: self.path.clone(),
                    holder: std::fs::read_to_string(&self.path)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                })
            }
            Err(std::fs::TryLockError::Error(e)) => {
                return Err(io_error("cannot take the writer lock", &self.path, &e))
            }
        }
        // Diagnosis only, and best effort on purpose: a failure to write the note must not fail an
        // operation whose exclusion has already been granted.
        let _ = std::fs::write(
            &self.path,
            format!(
                "{} {holder}
",
                std::process::id()
            ),
        );
        Ok(())
    }

    /// [`ProcessLock::acquire`], for a holder that outlives the borrow.
    ///
    /// `gx wrap` and a `Session` hold the lock across a call stack they do not own — a `Session` is
    /// handed to twelve different verbs and cannot lend a `&self` to each of them — so the owned
    /// form exists beside the borrowed one. Same lock, same refusal, same release on drop.
    ///
    /// # Errors
    /// As [`ProcessLock::acquire`].
    pub fn acquire_owned(self: &std::sync::Arc<Self>, holder: &str) -> Result<OwnedLock> {
        self.take(holder)?;
        Ok(OwnedLock {
            lock: std::sync::Arc::clone(self),
        })
    }

    /// Where the lock file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The lock, held by an owner rather than by a borrow. Released when dropped.
#[derive(Debug)]
pub struct OwnedLock {
    lock: std::sync::Arc<ProcessLock>,
}

impl Drop for OwnedLock {
    fn drop(&mut self) {
        let _ = self.lock.file.unlock();
    }
}

/// The lock, held. Released when this value is dropped.
///
/// RAII rather than a paired call for the reason 43 §7's ordering is three statements rather than a
/// promise: an early `?` between "take" and "release" is the ordinary shape of a refused operation,
/// and a lock that leaked on every refusal would deadlock the next writer on the first `Deny`.
#[derive(Debug)]
pub struct LockHeld<'a> {
    lock: &'a ProcessLock,
}

impl Drop for LockHeld<'_> {
    fn drop(&mut self) {
        // Nothing to do about a failed unlock: the file descriptor is about to close, and closing it
        // releases the lock on both platforms.
        let _ = self.lock.file.unlock();
    }
}

/// 🔴 **R4 / `req/225` H-03** — the tail record of a replayed buffer, lifted out of it.
///
/// `base` is where `bytes` starts inside the file, so a caller that read only the region past its
/// own offset ([`EngineJournal::read_to_the_end`]) still records a file offset.
fn tail_record(bytes: &[u8], base: u64, span: Option<(u64, u64)>) -> Option<TailRecord> {
    let (at, len) = span?;
    let start = usize::try_from(at).ok()?;
    let end = start.checked_add(usize::try_from(len).ok()?)?;
    let framed = bytes.get(start..end)?.to_vec();
    Some(TailRecord {
        at: base + at,
        framed,
    })
}

/// Read until `buf` is full or the file ends, and answer how much arrived.
///
/// 🔴 **R4** — `Read::read_exact` answers "unexpected end of file" for both "the file is shorter
/// than I expected" and "the device failed", and this crate's detectors have to tell a short file
/// from a broken one. `gx_log::store::fill` is the same three lines about the other file; see
/// [`EngineJournal::tail_unchanged`] for why the two are not shared.
fn fill_exact(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    use std::io::Read;
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// 🔴 **DR-43-7 (`req/38` §153, `req/215` H-03/M-05)** — copy a journal that is about to lose its
/// tail, and answer with where the copy went.
///
/// The twin of `gx_log::store`'s function of the same job, and separate from it because the two
/// crates do not depend on each other in that direction. See that one for the whole argument; the
/// short version is that "the journal lost its tail" and "the journal never had one" were the same
/// observation before this existed, and one of the three verbs that erased the difference was the
/// start-up gate on its way to refusing to start and telling the operator to go and look.
///
/// The name carries the two byte counts -- how much replayed and how long the file was -- and **not**
/// a timestamp. Two reasons, and the second is the binding one: a repeat of the identical tear
/// should not litter the directory with identical copies, and `crates/gx-engine/tests/
/// engine_shape.rs::the_engine_reads_no_clock_and_no_entropy` holds 41 §6's "randomness and clock
/// time are injected at the engine boundary" as a source scan -- an engine that read `SystemTime` to
/// name a file would be an engine that reads the clock. The copy is written with `create_new`, so if
/// a file of that name is already there it is left alone and reported: the **first** evidence of a
/// tear is the one worth keeping.
///
/// A failure here stops the open, deliberately: a repair whose evidence could not be preserved is
/// the behaviour being removed.
fn quarantine_torn_tail(path: &Path, good: u64, torn: u64) -> Result<PathBuf> {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".torn.{good}-{}", good + torn));
    let target = PathBuf::from(name);
    if target.exists() {
        return Ok(target);
    }
    std::fs::copy(path, &target).map_err(|e| {
        io_error(
            "cannot quarantine the journal's torn tail before truncating it (DR-43-7)",
            &target,
            &e,
        )
    })?;
    Ok(target)
}

/// The durability barrier 43 §7's write-ahead ordering depends on.
///
/// `sync_all` and not `sync_data`, for `gx-log/src/store.rs`'s reason: a record appended past the
/// old end of file is only reachable if the new length reached the device with it.
/// 🔴 **R9 / `req/236` H-01** — the inverse of `BlobStore::path_of`'s name-building, for the census.
///
/// `None` for anything that is not exactly 64 lowercase-hex characters, which is what a file in the
/// blob directory that gx did not name looks like.
fn cid_from_hex(text: &str) -> Option<Cid> {
    if text.len() != 64 {
        return None;
    }
    let mut raw = [0u8; 32];
    for (i, slot) in raw.iter_mut().enumerate() {
        *slot = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(Cid(raw))
}

fn barrier(file: &File, path: &Path, action: &'static str) -> Result<()> {
    file.sync_all().map_err(|e| io_error(action, path, &e))
}

/// 🔴 **`req/859` G9 / `req/868` (2026-08-26, seat=Opus, provisional — open to re-adjudication)** — whether the *name* of
/// a newly written file is as durable as its bytes, on the platform this binary was built for.
///
/// [`sync_parent_directory`] is `#[cfg(not(unix))] -> Ok(())`. That is a real gap and it was
/// **declared only in a doc comment**, which means every caller — and every operator — was told
/// nothing. This lane's ruling: the Windows path is **not implemented**, because the honest
/// Windows answer is not a translation of `fsync(dirfd)`. `FlushFileBuffers` on a directory handle
/// is not a supported operation, and the usual claim in its place — "NTFS journals metadata, so
/// the entry is durable once the file's own flush returns" — is a property of a filesystem we have
/// **never measured** (`store.rs`'s `.gx/LOCK` note records the same about Windows, 9p and sync
/// clients). Shipping an `Ok(())` renamed as a Windows implementation would be exactly the
/// flattery `req/859` §6 caught twice; shipping an unmeasured guarantee would be worse.
///
/// So the gap stops being silent instead. The platform's answer is **data** a caller can read,
/// branch on, and print, rather than a sentence in a doc comment nobody compiles:
///
/// ```
/// use gx_engine::{NAME_DURABILITY, NameDurability};
/// if !NAME_DURABILITY.is_held() {
///     eprintln!("{}", NAME_DURABILITY.notice());
/// }
/// # assert_eq!(NAME_DURABILITY.is_held(), cfg!(unix));
/// ```
///
/// **What this is not.** It is a declaration, not a warning: nothing in the workspace prints it
/// yet, because gx-engine has no logging surface of its own and the operator-facing half (a line
/// at `gx serve` start, or a `/healthz` member) is a wire/CLI change this lane did not have the
/// box to land. That half is owed, and `req/868` carries it. What is closed here is that the
/// platform boundary can no longer be widened or narrowed **without a test noticing**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameDurability {
    /// The parent directory is fsynced after the rename that publishes a file, so the *name*
    /// survives a crash exactly as the bytes do. Measured platform: x86_64 Linux (`req/52` §5,
    /// A-5); every other unix inherits the call, not the measurement.
    ParentDirectorySynced,
    /// There is no directory handle to sync here, so a newly published file's **name** is as
    /// durable as the platform makes it and no more. The bytes are still `sync_all`'d; what is
    /// unheld is the directory entry that points at them.
    NotHeldOnThisPlatform,
}

impl NameDurability {
    /// Whether the guarantee holds. `false` is not a failure — it is the honest answer on a
    /// platform whose directory-entry durability we have not measured.
    #[must_use]
    pub const fn is_held(self) -> bool {
        matches!(self, Self::ParentDirectorySynced)
    }

    /// One sentence, fit to show an operator verbatim. Deliberately claims nothing about what
    /// *does* hold on the unmeasured platform, because we do not know.
    #[must_use]
    pub const fn notice(self) -> &'static str {
        match self {
            Self::ParentDirectorySynced => {
                "name durability: parent directories are fsynced after every publish"
            }
            Self::NotHeldOnThisPlatform => {
                "name durability is NOT held on this platform: file contents are fsynced, but the \
                 directory entry naming them is not, and this platform was never measured -- a \
                 crash can lose the name of a file whose bytes reached the device"
            }
        }
    }
}

/// This build's answer, decided by the same `cfg` that decides [`sync_parent_directory`], so the
/// declaration and the implementation cannot drift apart.
#[cfg(unix)]
pub const NAME_DURABILITY: NameDurability = NameDurability::ParentDirectorySynced;

/// See [`NAME_DURABILITY`] on unix. This is the arm that used to be a comment.
#[cfg(not(unix))]
pub const NAME_DURABILITY: NameDurability = NameDurability::NotHeldOnThisPlatform;

/// Push the directory entry of a newly created journal to the device.
#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let dir = File::open(parent)
        .map_err(|e| io_error("cannot open the journal's directory", parent, &e))?;
    barrier(&dir, parent, "cannot fsync the journal's directory")
}

/// Elsewhere a directory has no handle to sync, so a newly created journal's *name* is as durable
/// as the platform makes it and no more. The same declared gap gx-log records in req/52 §5: v0.1 CI
/// is x86_64 Linux (A-5), and no other platform was measured.
#[cfg(not(unix))]
fn sync_parent_directory(path: &Path) -> Result<()> {
    let _ = path;
    Ok(())
}

/// 🔴 **R9 / `req/236` H-01** — tmp + fsync + rename + directory fsync, and clean up on the way out.
///
/// The shape `gx_adapter_fs::apply`, `gx_log::head` and (since R8) `ReceiptStore::put` all
/// already use, arriving at the one store that had none. What it buys is that a body at a
/// content address is either the whole body or is not there: the partial write happens under a
/// name nobody looks up, and `rename(2)` is the step that publishes it.
///
/// The temporary name carries this process's id so that two writers racing on the same CID
/// cannot truncate each other's staging file. Both then rename onto the same final path with
/// the same bytes, which is what content addressing makes safe.
///
/// 🔴 **R11 / `req/240` L-02 (audit 10 L-02)** — and the id is a **name**, not a gate. Nothing
/// reads it back: `BlobStore::sweep_staging` removes every `<cid>.blob.tmp.*` it finds without
/// asking whether that process is alive, so a `gx repair --yes` running beside a live writer
/// would remove a staging file that writer is still filling. What makes that unreachable today
/// is one directory up — DR-43-2's per-operation `.gx/LOCK` excludes a second writer on the
/// same project, so there is no second `put` to race with — and saying so here is the point:
/// the safety is the lock's, and a later lane that relaxes the lock must not read this name as
/// a second defence.
///
/// **Cleanup is not the defence.** A power cut does not run the `remove_file` below, so the
/// residue is `<cid>.<kind>.tmp.<pid>` rather than a fragment at the content address — a name no
/// reader resolves and `gx repair` reports (`req/236` M-04's class, one directory over).
///
/// 🔴 **`req/859` G8 / `req/868` (2026-08-26, seat=Opus, provisional — open to re-adjudication)** — this was a private
/// method on [`BlobStore`], so R9's repair reached exactly one of the two content-addressed
/// stores in this file. [`ObservationStore::put`] wrote `File::create` → `write_all` straight at
/// the final path and therefore still had the window R9 closed: a crash between the create and
/// the last byte leaves a **truncated body sitting at its own content address**, a file that lies
/// about its own hash. It fails closed on read — `get` re-hashes — but quietly: the escrowed
/// inverse then folds to `Unavailable`. Making the writer a free function taking the noun for its
/// error message is the whole fix; the discipline now has **one body and two callers**, which is
/// the only shape in which a future third store cannot re-open the same window by omission.
fn write_atomically(path: &Path, bytes: &[u8], what: &'static str) -> Result<()> {
    let temp = {
        let mut name = path.as_os_str().to_os_string();
        name.push(format!(".tmp.{}", std::process::id()));
        PathBuf::from(name)
    };
    let write = || -> std::io::Result<()> {
        {
            let mut file = File::create(&temp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&temp, path)?;
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&temp);
        return Err(io_error(what, path, &e));
    }
    sync_parent_directory(path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// M5-05, adopted (a) -- the content-addressed blob store (sem: SEM-gx-engine-384)
// ---------------------------------------------------------------------------

/// The largest blob this store will write, or read back **before decoding it**.
///
/// 🔴 **The second of M5-20's ceilings.** §37's ruling is "one pre-decode byte ceiling per engine
/// receiving mouth, plus a 1:1 probe against the contract row (M4H2-8's form)" (sem: SEM-gx-engine-385), and the engine has exactly two mouths that take in bytes it did
/// not just write: the journal ([`MAX_RECORD_BYTES`], hand 1) and this store. A separate constant
/// for the same reason hand 1 gave for its own: two files with different contents and different
/// writers must be able to move independently, and sharing one constant would make one ceiling a
/// statement about the other.
///
/// A `u64` rather than hand 1's `u32` because this ceiling is compared against a **file length**
/// (`std::fs::Metadata::len`) before a single byte is read, which is what "pre-decode" (sem: SEM-gx-engine-386) means here.
/// The journal's is compared against a length header inside a buffer that is already in memory.
///
/// The number is the same 1 MiB, and that is not a coincidence to be shared but a coincidence to be
/// noticed: `gx-adapter-fs` bounds a forward payload at 1 MiB (`MAX_FORWARD_PAYLOAD_BYTES`) and an
/// inverse at 1 MiB, so a delta any shipped adapter can build fits, and one that does not is refused
/// at the door rather than after the allocation it asks for.
pub const MAX_BLOB_BYTES: u64 = 1 << 20;

/// What [`BlobStore::put`] did.
///
/// The shape `gx_log::AppendOutcome` established in M2, for the same reason: "it is stored" and
/// "it was already stored and nothing was written" are different facts about a call, and a caller
/// that cannot tell them apart cannot measure **M4H6-3** -- "a residual CID matching an existing
/// delta CID is the flip side of storage's once-only property, and that is an advantage"
/// (req/38 §34; sem: SEM-gx-engine-387), whose implementation form §37 folds into M5-05, adopted (a)
/// as "if the CID is the same, only register the reference".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PutOutcome {
    /// The blob was written and synced by this call.
    Stored,
    /// A blob with this CID was already here, so **nothing was written**. Content addressing makes
    /// that safe: the name is a digest of the bytes, so a file already under that name holds the
    /// same bytes or was tampered with, and [`BlobStore::get`] is what catches the second case.
    AlreadyPresent,
}

impl PutOutcome {
    /// Which of the two this is.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            PutOutcome::Stored => "Stored",
            PutOutcome::AlreadyPresent => "AlreadyPresent",
        }
    }
}

/// A `PlannedDelta` in the form it is written down in: 42 §1.3's projection, and nothing else.
///
/// The bytes on disk are `canonical_dagcbor(delta.identity_view())`, which is **the same preimage
/// the CID is minted over** (`PlannedDelta::new` computes its own `reference` from this projection,
/// M4H1-3, adopted (a); sem: SEM-gx-engine-388). So the file's name is a digest of the file's contents by construction rather than
/// by convention, and [`BlobStore::get`] can check it.
///
/// Reading them back needs a type serde can build, and `PlannedDelta` deliberately has no
/// `Deserialize`: it is minted through a constructor that computes `reference`, and a derived
/// `Deserialize` would be a second door that lets a caller name a delta something it is not. This
/// mirror is therefore the same shape [`FingerprintRecord`] has one module up, for the same reason,
/// and hands its contents back **through `PlannedDelta::new`** (E-6: "reading a value back requires
/// a checked constructor"; sem: SEM-gx-engine-389). **M5H1-4** is the ticket that asks whether mirrors or checked `Deserialize`
/// impls in the lower crates are the right long-run answer; hand 3 is the second window it was
/// promised, and §5 of the report compares the two rather than deciding.
#[derive(Debug, Deserialize)]
struct BlobRecord {
    payload: PayloadBytes,
    substrate: SubstrateKind,
}

/// A CBOR byte string, read back as bytes.
///
/// serde's default for `Vec<u8>` is a *sequence of integers*, and DAG-CBOR writes a payload as a
/// byte string (major type 2) because `gx_substrate::PayloadView` asks it to. Decoding one into the
/// other silently produces the wrong length, so the visitor is written out -- the same shape
/// `gx-adapter-fs`'s `Blob` and `gx-core`'s DSSE payload carry.
struct PayloadBytes(Vec<u8>);

impl fmt::Debug for PayloadBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PayloadBytes(opaque, {} bytes)", self.0.len())
    }
}

impl<'de> Deserialize<'de> for PayloadBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> core::result::Result<Self, D::Error> {
        struct Bytes;

        impl<'v> Visitor<'v> for Bytes {
            type Value = PayloadBytes;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a byte string")
            }

            fn visit_bytes<E: serde::de::Error>(
                self,
                v: &[u8],
            ) -> core::result::Result<PayloadBytes, E> {
                Ok(PayloadBytes(v.to_vec()))
            }

            fn visit_byte_buf<E: serde::de::Error>(
                self,
                v: Vec<u8>,
            ) -> core::result::Result<PayloadBytes, E> {
                Ok(PayloadBytes(v))
            }
        }

        deserializer.deserialize_bytes(Bytes)
    }
}

/// The one content-addressed store (**M5-05, adopted (a)**; sem: SEM-gx-engine-390): deltas in, deltas out, keyed by their CID.
///
/// # Why one store holds both kinds of delta
///
/// §37's ruling is "**one CID-keyed blob store** holds **both** `PlannedDelta` and `inverse_delta`"
/// (sem: SEM-gx-engine-391), and the reason is **M4H6-3**: an inverse whose payload happens to equal a forward delta's
/// has the same CID, and "storage's once-only property" means the second one is a *reference* rather than a copy.
/// Two stores would each hold their own idea of that one blob. There is no separate escrow store:
/// 42 §5's exception ("`EscrowedInverse.inverse_delta` (the payload body itself) … **is retained
/// (mandatory)**"; sem: SEM-gx-engine-391) is a
/// statement about a *body*, and a body is what this store keeps. The escrow **index** -- which
/// transformation the inverse belongs to, and what has become of it -- is reconstructed from the
/// journal by [`crate::replay::reconstruct`], and [`BlobStore::escrowed`] is where the two meet.
///
/// **M5H1-6's condition is met**: this store declares no third spelling of "what opening a file
/// found" (sem: SEM-gx-engine-392). A directory of whole files has no torn tail -- a blob is either there and complete, or
/// absent, or the wrong size, and the last two are refusals rather than recoveries -- so
/// `gx_log::Recovery` stays the one word for the one idea it names.
///
/// # The contract
///
/// | method | refuses when | reason |
/// |---|---|---|
/// | `put` | the encoded delta is over `MAX_BLOB_BYTES` | a blob that could be written and never read back is a trap the store sets for itself |
/// | `get` | the file is over `MAX_BLOB_BYTES` **before it is read**, or its contents do not rebuild into a delta whose CID is the name it was filed under | "the pre-decode byte ceiling" (M5-20, adopted (a); sem: SEM-gx-engine-393); and a blob that does not hash to its name is not the blob that was asked for |
///
/// # Layout
///
/// One file per blob, named with the CID in **lower-case hex**. Not `gx_canon::cid::to_text`'s
/// `gx1:<base32>`: that spelling carries a colon, which is a legal byte in a POSIX filename and a
/// forbidden one on the filesystem this repository's working tree lives on. The readable form is for
/// humans and wires; a file name is neither.
#[derive(Clone, Debug)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    /// Open (creating if absent) the directory the blobs live in.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory cannot be created.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)
            .map_err(|e| io_error("cannot create the blob store", &root, &e))?;
        Ok(Self { root })
    }

    /// 🔴 **R5 / `req/227` M-02** — name the directory without making it.
    ///
    /// DR-43-7's reader's door, on the one store that had none. `Engine::open_read_only` was
    /// written as "no create, no truncate, no repair" and then created two directories on its way
    /// past, which `req/227` M-02 measured by deleting `<journal>.blobs` and `<journal>.observations`
    /// from a project and watching a `gx repair` **report** grow them back. The verb's own
    /// declaration ("the one thing it writes is the lock's holder note") was a count of one and the
    /// truth was three.
    ///
    /// A read of a blob that is not there answers the same refusal it always did; the difference is
    /// that the absence of the directory is now a fact about the project rather than a fact about
    /// who looked at it last.
    ///
    /// # Errors
    /// None today. Fallible for [`BlobStore::open`]'s signature, so the two doors stay swappable.
    pub fn open_read_only(root: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            root: root.as_ref().to_path_buf(),
        })
    }

    /// The directory the blobs live in.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where a blob with this CID is filed.
    fn path_of(&self, cid: &Cid) -> PathBuf {
        let mut name = String::with_capacity(cid.0.len() * 2 + 5);
        for byte in cid.0 {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".blob");
        self.root.join(name)
    }

    /// Store a delta under its own CID, or notice that it is already here.
    ///
    /// Journal-first is preserved by the caller, not by this method: 43 §7 orders the journal
    /// *before* the side effect, and storing a body is a side effect. [`crate::Engine::plan`]
    /// appends `Planned` and then puts the blob, so a crash between them leaves a name in the
    /// journal with no body -- which is recoverable (the adapter can plan again from the same
    /// snapshot, 43 T-2's idempotency column) -- while the other order would leave a body nobody
    /// ever named.
    ///
    /// # Errors
    /// [`Error::Canon`] if the delta's projection has no canonical form. [`Error::Malformed`] if
    /// the encoded delta is over [`MAX_BLOB_BYTES`]. [`Error::Io`] if it cannot be written or
    /// synced.
    pub fn put(&self, delta: &PlannedDelta) -> Result<(Cid, PutOutcome)> {
        let cid = delta.reference().cid;
        let path = self.path_of(&cid);

        let bytes = cbor::encode(&delta.identity_view())?;
        if bytes.len() as u64 > MAX_BLOB_BYTES {
            return Err(Error::Malformed {
                detail: format!(
                    "a delta of {} bytes is over the {MAX_BLOB_BYTES}-byte blob ceiling",
                    bytes.len()
                ),
            });
        }

        // 🔴 ~~M4H6-3: "if the CID is the same, only register the reference" (sem: SEM-gx-engine-394). The early return **is** the reference: no file
        // is opened for writing, so a blob that is already here is untouched by a second put
        // even if the bytes offered differ (they cannot, without breaking the digest -- and
        // `get` is where that is caught).~~
        //
        // The struck paragraph is what stood here until R9, kept because a reader of the repair
        // should see the claim it replaced. Its last clause is the one that failed: bytes that
        // differ do not have to be **offered**. They can be what an accident left behind.
        //
        // 🔴 **R9 / `req/236` H-01** — the name being taken is not evidence that the body is here.
        //
        // M4H6-3's rule is kept and read on the body: "if the CID is the same" now means "if the
        // bytes are the same", and only that is `AlreadyPresent`. The old reading argued that
        // differing bytes "cannot [be offered], without breaking the digest -- and `get` is where
        // that is caught". `req/236` H-01 measured the half of that sentence which is about
        // *accidents* rather than about offers: a full disk during the `write_all` below left
        // **204,800 bytes of a 400,096-byte body at the content address**, the early return then
        // made it permanent, and the next entirely successful commit adopted the fragment as its
        // own escrowed inverse — `rc=0`, a signed receipt, `gx receipt verify` clean, and a
        // `gx undo` that failed for ever with "input ends 195,262 byte(s) early".
        //
        // So the reference is registered from the **body**: the bytes on disk are compared with the
        // bytes being offered, and only an exact match is `AlreadyPresent`. Anything else — a
        // fragment, a zero-length file, a flipped bit, a file that will not read — falls through to
        // the write below, which replaces it. The comparison is against bytes rather than against a
        // recomputed digest because the encode has already happened (it is the input to the
        // ceiling check above) and equal bytes is the strongest of the two answers.
        //
        // ~~Cost, measured rather than assumed: the read only happens on the path that used to
        // return immediately, which is a re-put of a body this project already holds. A first put —
        // every commit's escrow, every plan's delta — does no extra I/O at all.~~
        //
        // 🔴 **R11 / `req/240` M-08 (audit 10 M-05)** — the struck paragraph's arithmetic is right
        // and its premise is false, so the number it produces is wrong for the ordinary road.
        //
        // What audit 10 measured with `strace` (1 commit, both body sizes, `-e trace=openat,rename`):
        // **0** renames at the content address and **3** `O_RDONLY` opens of the `.blob`. The reason
        // is a property of the fs substrate rather than of this function: the inverse escrowed by
        // the *n*-th commit is the previous content of the file, which is byte-for-byte the delta
        // the *(n−1)*-th commit already stored — same bytes, same CID — so the escrow's `put` takes
        // the `AlreadyPresent` branch on **every** commit after the first, not on a rare re-put.
        //
        // ∴ the honest statement of the cost: a sequential edit of one file pays a full read-back
        // and byte comparison of the body on each commit, and that is the road, not the exception.
        // It is still the cheaper half of the trade R9 made (`req/236` H-01: a fragment adopted at
        // a content address is worse than any read), and the implementation does not move — what
        // moves is what the next lane budgeting this road is told (`req/237` §5 carries the same
        // correction).
        if path.exists() {
            match std::fs::read(&path) {
                Ok(held) if held == bytes => return Ok((cid, PutOutcome::AlreadyPresent)),
                _ => {}
            }
        }

        write_atomically(&path, &bytes, "cannot write the blob")?;
        Ok((cid, PutOutcome::Stored))
    }

    /// Read a delta back, through the checked constructor, and check it against its name.
    ///
    /// Three refusals, in the order they are cheap: the size on disk (**before** the bytes are
    /// read), the decode, and the digest. The last one is what content addressing buys -- a blob
    /// edited in place decodes perfectly well and is still not the blob that was asked for.
    ///
    /// # Errors
    /// [`Error::NotFound`] if no blob is filed under `cid`. [`Error::Io`] if it cannot be read.
    /// [`Error::Malformed`] if it is over [`MAX_BLOB_BYTES`], if its bytes do not decode, if the
    /// decoded value is not a delta `gx_substrate` will build, or if the delta it rebuilds does not
    /// hash to `cid`.
    pub fn get(&self, cid: &Cid) -> Result<PlannedDelta> {
        let path = self.path_of(cid);
        let metadata = std::fs::metadata(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound {
                    what: "blob",
                    id: gx_canon::cid::to_text(cid),
                }
            } else {
                io_error("cannot stat the blob", &path, &e)
            }
        })?;
        // 🔴 M5-20, adopted (a), "pre-decode" in one statement (sem: SEM-gx-engine-395): the length that decides comes from the
        // directory entry, so a hostile or damaged blob is refused before it is in memory.
        if metadata.len() > MAX_BLOB_BYTES {
            return Err(Error::Malformed {
                detail: format!(
                    "the blob {} is {} bytes, over the {MAX_BLOB_BYTES}-byte ceiling",
                    gx_canon::cid::to_text(cid),
                    metadata.len()
                ),
            });
        }

        let bytes =
            std::fs::read(&path).map_err(|e| io_error("cannot read the blob", &path, &e))?;
        let record: BlobRecord = cbor::decode(&bytes)?;
        let delta = PlannedDelta::new(record.substrate, record.payload.0).map_err(|e| {
            Error::Malformed {
                detail: format!("the blob does not rebuild into a delta: {e}"),
            }
        })?;
        if delta.reference().cid != *cid {
            return Err(Error::Malformed {
                detail: format!(
                    "the blob filed as {} rebuilds into {}",
                    gx_canon::cid::to_text(cid),
                    gx_canon::cid::to_text(&delta.reference().cid)
                ),
            });
        }
        Ok(delta)
    }

    /// Whether a blob is filed under this CID.
    ///
    /// A `metadata` call, and therefore a fact about the **name**. [`BlobStore::holds_body`] is the
    /// one that answers about the body; `req/236` H-01 is the measurement of what telling them
    /// apart is worth.
    #[must_use]
    pub fn contains(&self, cid: &Cid) -> bool {
        self.path_of(cid).exists()
    }

    /// 🔴 **R9 / `req/236` H-01** — whether the body filed under this CID **reads back as itself**.
    ///
    /// [`BlobStore::get`]'s three refusals with the value thrown away: the size on disk, the
    /// decode, and the digest. `req/236` H-01 measured every one of the four damaged shapes it
    /// built — zero bytes, half a body, one flipped bit, and a deletion — answering
    /// `inverse_status: "Available"` through [`BlobStore::contains`], because existence is not
    /// legibility. Three of those four fail here; the fourth (deletion) already failed there.
    ///
    /// **This reads the file.** It is called from `Engine::inverse_status`, which is what
    /// `GET /v1/transformations` and the undo pre-flight ask, so an answer of `Available` now costs
    /// a read of the body it is an answer about. That is the price of the answer being true.
    #[must_use]
    pub fn holds_body(&self, cid: &Cid) -> bool {
        self.get(cid).is_ok()
    }

    /// 🔴 **R9 / `req/236` H-01** — every blob filed here that does not read back as its own name.
    ///
    /// The census `gx repair` reports. A file under `<cid>.blob` whose bytes do not rebuild into
    /// `<cid>` is either damage a third party did (Model B) or a fragment a pre-R9 binary left
    /// behind on a full disk (Model A, and the reason this walk exists) — and either way it is a
    /// body no undo can use, which is a fact an operator has a right to be told **before** they
    /// need it.
    ///
    /// Names rather than `Cid`s, because a file whose name is not 64 hex characters is also
    /// something this directory should not hold, and there is no `Cid` to report it under. Staging
    /// files (`<cid>.blob.tmp.<pid>`) are **not** walked: their extension is not `blob`, they are
    /// [`BlobStore::write_atomically`]'s declared residue, and they are reported by name in the
    /// same report rather than counted as damage.
    #[must_use]
    pub fn unreadable_bodies(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "blob") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match cid_from_hex(stem) {
                Some(cid) if self.holds_body(&cid) => {}
                _ => out.push(stem.to_string()),
            }
        }
        out.sort();
        out
    }

    /// 🔴 **R9 / `req/236` M-04** — the staging files a crash left behind, by name.
    ///
    /// [`BlobStore::write_atomically`] writes `<cid>.blob.tmp.<pid>` and removes it on the error
    /// path; a power cut runs no error path. Nothing reads these files and nothing resolves their
    /// names, so they cost disk and nothing else — but a directory an operator is being asked to
    /// trust should not hold objects gx cannot account for, and "gx repair said nothing about it"
    /// is how `req/236` M-04 found the same class one directory over.
    ///
    /// **Reported, not removed, by the reader's door.** `Engine::sweep_staging` is where they are
    /// swept, and only under a verb that already writes.
    #[must_use]
    pub fn staging_residue(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return out;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if name.contains(".blob.tmp.") {
                out.push(name);
            }
        }
        out.sort();
        out
    }

    /// 🔴 **R9 / `req/236` M-04** — remove the staging files, and say which ones went.
    ///
    /// Called from the writer's door only. A file being swept here is one that no name resolves to
    /// and no record mentions: it is not evidence, and DR-43-7 (1)'s "no verb removes evidence" is
    /// not in tension with it. A file that will not delete is left and still reported.
    pub fn sweep_staging(&self) -> Vec<String> {
        let mut swept = Vec::new();
        for name in self.staging_residue() {
            if std::fs::remove_file(self.root.join(&name)).is_ok() {
                swept.push(name);
            }
        }
        swept
    }

    /// How many blobs are filed.
    ///
    /// Counted from the directory each time rather than cached: the cache would be a second answer
    /// to a question the filesystem already answers, and a store whose count disagreed with its
    /// contents would be the exact failure "reference only" (sem: SEM-gx-engine-396) is measured by.
    #[must_use]
    pub fn len(&self) -> usize {
        std::fs::read_dir(&self.root).map_or(0, |entries| {
            entries
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "blob"))
                .count()
        })
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Rebuild an [`EscrowedInverse`] from the journal's index row and this store's body.
    ///
    /// 🔴 **E-M5-6 across a restart.** §38 ruled 42 §3.12's contradiction closed by making
    /// `inverse_delta` an `Option` kept in step with `status` by three checked constructors; that
    /// held for values built in memory. This is the door those values come back through, and it is
    /// the same door: [`EscrowedInverse::restore`], which refuses both directions of the
    /// contradiction. An index row that says `Unavailable` and names a body, or says anything else
    /// and names none, is [`Error::InconsistentEscrow`] -- so "a store that stored something it
    /// said it did not have" (sem: SEM-gx-engine-397) cannot be reconstructed any more than it could be constructed.
    ///
    /// # Errors
    /// [`Error::InconsistentEscrow`] when the row's status and its body disagree, and whatever
    /// [`BlobStore::get`] refuses for the body itself.
    pub fn escrowed(&self, row: &EscrowRow) -> Result<EscrowedInverse> {
        let body = match row.inverse_cid {
            Some(cid) => Some(self.get(&cid)?),
            None => None,
        };
        EscrowedInverse::restore(row.transformation, body, row.retained_until, row.status)
    }
}

// ---------------------------------------------------------------------------
// Two-phase escrow (req/38 §98, ruling 1 / §99, ruling 2, clause ③; sem: SEM-gx-engine-398) -- the observation store
// ---------------------------------------------------------------------------

/// The largest observation this store will write, or read back **before decoding it**.
///
/// The third of M5-20's ceilings, for M5-20's reason ("one pre-decode byte ceiling per engine
/// receiving mouth"; sem: SEM-gx-engine-399):
/// an observation is bytes a *server* chose, arriving through `apply`, and a store without a
/// ceiling would let one answer ask for an unbounded allocation at read-back. A separate constant
/// from [`MAX_BLOB_BYTES`] for the same reason that one is separate from [`MAX_RECORD_BYTES`]:
/// different files, different writers, independently movable. The number being the same 1 MiB is
/// a coincidence a probe may notice; the rule is not shared.
pub const MAX_OBSERVATION_BYTES: u64 = 1 << 20;

/// 🔴 Raw observed bytes, content-addressed — **deliberately not** [`BlobStore`] (`req/38` §99
/// ruling 2, clause ③; sem: SEM-gx-engine-400).
///
/// The blob store's contract is `PlannedDelta` in, `PlannedDelta` out: its `get` *rebuilds a
/// delta* through the checked constructor and re-mints the CID over the delta's projection. An
/// observation is not a delta — it is whatever the server answered, kept verbatim so that the
/// completion step (and any later audit) reads exactly what arrived. Widening `BlobStore` to hold
/// both would have dissolved its "a name is a digest of a delta" (sem: SEM-gx-engine-401) guarantee to admit a second
/// content kind, so the raw bytes get their own mouth with the same three disciplines: a ceiling
/// checked against the directory entry before a byte is read, a digest check against the name
/// (`gx_canon::cid::mint(Domain::Leaf, ..)` — the same leaf road `content_digest` takes, because
/// raw bytes are a leaf and not a projected value), and content-addressed once-ness (`put` of a
/// present CID writes nothing).
#[derive(Clone, Debug)]
pub struct ObservationStore {
    root: PathBuf,
}

impl ObservationStore {
    /// Open (creating if absent) the directory the observations live in.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory cannot be created.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)
            .map_err(|e| io_error("cannot create the observation store", &root, &e))?;
        Ok(Self { root })
    }

    /// 🔴 **R5 / `req/227` M-02** — name the directory without making it. See
    /// [`BlobStore::open_read_only`], which is this door on the other store and carries the reason.
    ///
    /// # Errors
    /// None today; fallible so the two doors stay swappable.
    pub fn open_read_only(root: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            root: root.as_ref().to_path_buf(),
        })
    }

    /// The directory the observations live in.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where an observation with this CID is filed. `.obs`, not `.blob`: two content kinds, two
    /// spellings, so a directory listing cannot mistake one store's file for the other's.
    fn path_of(&self, cid: &Cid) -> PathBuf {
        let mut name = String::with_capacity(cid.0.len() * 2 + 4);
        for byte in cid.0 {
            use core::fmt::Write;
            let _ = write!(name, "{byte:02x}");
        }
        name.push_str(".obs");
        self.root.join(name)
    }

    /// The content address of observed bytes: the same function for every caller, so the CID the
    /// journal records and the file this store writes cannot disagree about what was observed.
    #[must_use]
    pub fn address(bytes: &[u8]) -> Cid {
        gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[bytes])
    }

    /// Store observed bytes under their own digest, or notice they are already here.
    ///
    /// 🔴 **`req/859` G8 / `req/868` (2026-08-26, seat=Opus, provisional — open to re-adjudication)** — the write goes
    /// through [`write_atomically`], the same one body [`BlobStore::put`] uses. Before this it was
    /// `File::create` → `write_all` → fsync straight at the final path, which left R9's window
    /// (`req/236` H-01) open on this store after it had been closed on the other: a crash mid-write
    /// published a truncated body **at its own content address**. `get` re-hashes and so fails
    /// closed, but quietly — the escrowed-inverse completion folds to `Unavailable` and the
    /// operator is told the observation is gone rather than that it was half-written. Measured
    /// rather than argued: `tests/g8_observation_atomicity.rs` caught 485 partial publications in
    /// 2.3 M observations of the old writer, and none of the new one.
    ///
    /// Journal-first is the caller's, exactly as [`BlobStore::put`] says: `Engine::commit`
    /// appends `ApplyObserved` and then puts the bytes, so a crash between the two leaves a name
    /// with no body — which recovery folds to `Unavailable` honestly (the observation is gone and
    /// says so), never a body nobody named.
    ///
    /// # Errors
    /// [`Error::Malformed`] if the bytes are over [`MAX_OBSERVATION_BYTES`]. [`Error::Io`] if they
    /// cannot be written or synced.
    pub fn put(&self, bytes: &[u8]) -> Result<(Cid, PutOutcome)> {
        if bytes.len() as u64 > MAX_OBSERVATION_BYTES {
            return Err(Error::Malformed {
                detail: format!(
                    "an observation of {} bytes is over the {MAX_OBSERVATION_BYTES}-byte ceiling",
                    bytes.len()
                ),
            });
        }
        let cid = Self::address(bytes);
        let path = self.path_of(&cid);
        // 🔴 **`req/871` F4 / `req/868`** (2026-08-26, seat=Opus, provisional — open to re-adjudication) — the *other*
        // half of G8, and landing only the first half was a real defect. `path.exists()` alone
        // trusts the **name**; content addressing is a promise about the **bytes**. A tree that
        // already holds a body truncated by a crash under the old writer would answer
        // `AlreadyPresent` for ever and never heal, so the atomicity repair above would protect
        // only trees that had never yet been hurt. `BlobStore::put` has re-read and byte-compared
        // at this exact point since R9; this is that sibling's discipline, arriving where it was
        // missing. A mismatch falls through and republishes atomically, which is the repair.
        if path.exists() {
            match std::fs::read(&path) {
                Ok(held) if held == bytes => return Ok((cid, PutOutcome::AlreadyPresent)),
                _ => {}
            }
        }
        write_atomically(&path, bytes, "cannot write the observation")?;
        Ok((cid, PutOutcome::Stored))
    }

    /// Read observed bytes back, and check them against their name.
    ///
    /// # Errors
    /// [`Error::NotFound`] if no observation is filed under `cid`. [`Error::Malformed`] if the
    /// file is over [`MAX_OBSERVATION_BYTES`] (checked against the directory entry, before a byte
    /// is read) or does not hash to `cid`. [`Error::Io`] if it cannot be read.
    pub fn get(&self, cid: &Cid) -> Result<Vec<u8>> {
        let path = self.path_of(cid);
        let metadata = std::fs::metadata(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound {
                    what: "observation",
                    id: gx_canon::cid::to_text(cid),
                }
            } else {
                io_error("cannot stat the observation", &path, &e)
            }
        })?;
        if metadata.len() > MAX_OBSERVATION_BYTES {
            return Err(Error::Malformed {
                detail: format!(
                    "the observation {} is {} bytes, over the {MAX_OBSERVATION_BYTES}-byte ceiling",
                    gx_canon::cid::to_text(cid),
                    metadata.len()
                ),
            });
        }
        let bytes =
            std::fs::read(&path).map_err(|e| io_error("cannot read the observation", &path, &e))?;
        if Self::address(&bytes) != *cid {
            return Err(Error::Malformed {
                detail: format!(
                    "the observation filed as {} hashes to {}",
                    gx_canon::cid::to_text(cid),
                    gx_canon::cid::to_text(&Self::address(&bytes))
                ),
            });
        }
        Ok(bytes)
    }

    /// Whether an observation is filed under this CID.
    #[must_use]
    pub fn contains(&self, cid: &Cid) -> bool {
        self.path_of(cid).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cid_of(b: u8) -> Cid {
        Cid([b; 32])
    }

    /// `DraftCreated` is the one record with no `TransformationId` (**E-M5-3**, **M5-17, adopted (b)**; sem: SEM-gx-engine-402).
    #[test]
    fn only_a_draft_has_no_transformation_id() {
        let draft = EngineJournalRecord::DraftCreated {
            intent_id: IntentId(cid_of(1)),
            rng_seed: 7,
            at: Timestamp(1),
        };
        let started = EngineJournalRecord::VerifyStarted {
            transformation: TransformationId(cid_of(2)),
            at: Timestamp(2),
        };
        assert!(draft.transformation().is_none());
        assert!(started.transformation().is_some());
    }

    /// The escrow invariant of 42 §3.12, in both directions (**M5H1-3**).
    #[test]
    fn an_unavailable_inverse_may_not_carry_a_delta() {
        let id = TransformationId(cid_of(3));
        let delta = PlannedDelta::new(SubstrateKind::Fs, vec![1, 2, 3])
            .expect("a three-byte payload is inside the bound");
        let bad = EscrowedInverse::restore(id, Some(delta), None, InverseStatus::Unavailable);
        assert_eq!(
            bad.expect_err("a delta with Unavailable is the contradiction")
                .kind(),
            "InconsistentEscrow"
        );
        let also_bad = EscrowedInverse::restore(id, None, None, InverseStatus::Available);
        assert_eq!(
            also_bad
                .expect_err("Available with no delta is the other direction")
                .kind(),
            "InconsistentEscrow"
        );
    }
}
