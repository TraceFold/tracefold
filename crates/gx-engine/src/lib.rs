// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The Transformation lifecycle: the engine's write-ahead journal, the escrow of inverses, and the
//! replay that rebuilds engine state from what was written.
//!
//! Spec: `req/spec/40-architecture/41-architecture.md` §2 for where this crate sits and §5 for the
//! commit protocol it will run, 42 §3.12 for `EscrowedInverse` and 42 §3.13 for the journal, 43 for
//! the eleven states and nineteen transitions, 32 FR-030..FR-040 for what it must do, 34
//! AC-030..AC-045 for how that is judged.
//!
//! # What this crate is for
//!
//! 42 §1.3-3 is the sentence the whole crate exists to serve:
//!
//! > **State is never encoded**: a `Transformation` holds no lifecycle state at all (Draft/
//! > Candidate/…/Committed, see 43). State is managed by an external table on the engine's side,
//! > keyed by `TransformationId` (the engine store) (sem: SEM-gx-engine-072)
//!
//! So the *object* is stateless and the *engine* holds the state. And 43 §7 says where that state
//! lives: every transition is written to the journal **before** any side effect, which makes the
//! journal the truth and the in-memory table a cache. That ordering is the reason a crash cannot
//! leave a change applied with nothing recording it -- the property gx exists to sell.
//!
//! # What is here, and what is not
//!
//! 41 §2 fixes the module list as `src/{lib,pipeline,store,replay}.rs`, and **M5H1-5, adopted (a)**
//! (sem: SEM-gx-engine-073)
//! (req/38 §38) confirms those four are canon — req/78 §2.1's seven-module proposal is recorded as
//! withdrawn. All four files now exist:
//!
//! | module | types (42 §0) | hand | acceptance |
//! |---|---|---|---|
//! | `store.rs` | `EngineJournalRecord`, `EscrowedInverse`, `InverseStatus`, the journal itself | 1 | — |
//! | `replay.rs` | reading a journal back, torn tail included | 1 | AC-039 is hand 3 |
//! | `pipeline.rs` | the eight entry points (**five so far**) and 43 §7's recovery | 2, 4, **5** (this one), 6 | **AC-030..035, AC-038, AC-043** |
//!
//! Hand 2's four are `submit` (T-1), `plan` (T-2), `verify` (T-3 and T-4a..T-4e) and `canonicalize`
//! (T-8/T-8r). Hand 4 adds `commit` — T-9, T-10a, T-10b, T-10c and T-11, which is the whole of the
//! critical section 43 §1 calls `Committing`. `undo`/`cancel`/`escalation` are hand 6's and are
//! **absent rather than stubbed** — `tests/engine_shape.rs` fails on a sixth entry point as readily
//! as on a missing fifth, so the boundary between hands is a measurement instead of an intention.
//!
//! Hand 5 adds [`pipeline::Engine::recover`], which is **not** a ninth entry point: 43 §7 is a
//! procedure over the journal rather than a transition, and it writes the records T-11 and T-10a/c
//! own from a road §7 describes on its own. What it consumes is the record hand 4 wrote —
//! `ApplyStarted` — and what that record buys is the whole of req/78 §3.2 Λ4. See
//! [`pipeline::Engine::recover`] for the two questions the exactly-once judgement asks, and
//! `tests/crash_recovery.rs` for the same crashed bytes recovered two ways.
//!
//! # What hand 4 makes true that was not true before
//!
//! This is the milestone's first hand that **changes the world**. Everything until now read the
//! substrate and wrote the engine's own files; `commit` calls `SubstrateAdapter::apply`, appends to
//! gx-log's ledger and issues a signed receipt. Three properties become measurable at once, and all
//! three are absences:
//!
//! * **Rule 2** (req/78 §3.3; sem: SEM-gx-engine-074): `adapter.apply` is called in **one place** in this crate, and
//!   `tests/ac_035.rs` measures it twice — a scan of the source and a counting adapter.
//! * **INV-S7**: "when `Fingerprint₁≠Fingerprint₀`, `adapter.apply` is never called, under any
//!   circumstance" (sem: SEM-gx-engine-074),
//!   which is AC-034 with a mutation injected between `plan` and `commit`.
//! * **E-M5-1**: every call to `apply` is preceded by an `ApplyStarted` record, so a crash inside
//!   the call leaves "the adapter was asked" written down (sem: SEM-gx-engine-075). Hand 5 is the consumer; hand 4 is what
//!   makes the record true.
//!
//! # `replay` here is the journal's, not FR-039's
//!
//! Two different things are called replay and this hand implements the smaller one.
//! [`replay::replay`] turns the bytes of a journal file back into the records it holds, which is
//! what [`store::EngineJournal::open`] needs in order to append after a crash (43 §7-1). FR-039's
//! *deterministic replay* -- rebuilding engine state from a seed and a clock so that the result is
//! bit-equal -- is hand 3, and **E-M5-2** (`req/38_ERRATA_2026-08-07.md` §37, ruling M5-02 (a))
//! already settled what it may touch:
//!
//! > replay is **a read-only operation that reconstructs Σ only** -- AC-039's "resulting state" is
//! > read as Σ (state table + ledger root + escrow index). It never calls the adapter
//! > (sem: SEM-gx-engine-076)
//!
//! This module is inside that ruling by construction: it takes a `&[u8]` and returns values. It
//! cannot reach a substrate because it is not given one.
//!
//! # The two errata this hand implements
//!
//! `req/38_ERRATA_2026-08-07.md` §37 rules two changes to 42 §3.13, and both are in
//! [`store::EngineJournalRecord`] rather than in a comment about it:
//!
//! * **E-M5-1** adds `ApplyStarted { transformation, delta_cid, at }`. 43 T-10b escrows the inverse
//!   and T-11 records the commit, and between them the adapter is asked to change the world with
//!   **no journal record naming the attempt** -- 51 §8.1 says so itself ("43 defines no individual
//!   journal record name for this interval") (sem: SEM-gx-engine-077). req/78 §3.2 Λ4 shows what that costs in three lines: a crash
//!   after `apply` and before `ledger.append` sends recovery down 43 §7-3c, which recomputes
//!   `Fingerprint₁`, finds it changed *because of its own write*, and aborts with
//!   `PreconditionChanged` -- leaving the substrate modified and the ledger empty. Write-ahead is
//!   the standard answer and this record is it: recovery that finds an `ApplyStarted` knows the
//!   attempt was made and does not re-run the CAS.
//! * **E-M5-3** keys `DraftCreated` on `IntentId`. 42 §3.13 writes `DraftCreated { transformation,
//!   .. }` while 43 T-1 writes "`TransformationId` is not yet settled (delta/target undetermined)"
//!   and puts only `intent_id` in its journal cell. 51 §8.1's precedence clause -- "the canonical
//!   journal record name is 43 §3's transition table; 42 §3.13 is...the old wording, and 43 wins
//!   when they conflict" (sem: SEM-gx-engine-078) -- is applied here for the
//!   first time.
//!
//! `tests/journal_vocabulary.rs` is where those two sentences stop being prose: it parses 42 §3.13
//! and 43 §3 out of the canon and compares them with this crate's source, so the difference between
//! them is measured rather than remembered.
//!
//! # Journal and ledger are different files
//!
//! 42 §3.13: "**a different thing from the Ledger (§3.11)**: the Ledger is the public witness
//! record after a commit is settled; the Journal is the engine's internal record of the pipeline
//! in progress (every step between Draft and Committing), and it is never published"
//! (sem: SEM-gx-engine-079). They are
//! written by different crates, hold different records and have different audiences. What they
//! share is a failure mode -- an append-only file can be cut in half by a crash -- and this crate
//! reuses [`gx_log::Recovery`] for it rather than declaring a second struct with the same two
//! fields. A second spelling of one idea is a second thing that can drift (M5H1-6 records the
//! choice, so an audit that prefers a separate type can force one).

#![forbid(unsafe_code)]

pub mod pipeline;
pub mod replay;
pub mod store;

pub use pipeline::{
    CanonEncoder, Canonicalizer, Door, Engine, EvidenceSource, HeadAuthenticity, HeadKeys,
    HumanRuling, InjectedEvidence, JournalDeparture, Lifecycle, ProjectAnchor, RecoverPartial,
    Recovered, RecoveryPath, UndoRefusal, UndoRefusalRow, UndoWitness, Unobservable,
    UnreachableEvidence, WitnessMissing, DEFAULT_ESCALATION_TTL_NANOS, DEFAULT_VERIFY_TTL_NANOS,
    HEAD_WITNESS_PAYLOAD_TYPE, LIFECYCLE_STATES, RECOVERY_PATHS, UNDO_REFUSALS,
};
pub use replay::{
    reconstruct, replay, ChainBreak, CommittedRow, DraftRow, EscrowRow, JournalCreation,
    JournalFormat, Replay, Sigma, StateRow, JOURNAL_MAGIC,
};
pub use store::{
    BlobStore, EngineJournal, EngineJournalRecord, EscrowedInverse, InverseStatus,
    NotAttemptedBecause, ObservationStore, PutOutcome, Rollback, SupersedeIndex, MAX_BLOB_BYTES,
    MAX_OBSERVATION_BYTES, MAX_RECORD_BYTES,
};

/// 🔴 **M6H5-12, adopted (a)** — this crate's version, as a value the HTTP surface can ask for.
///
/// > **M6H5-12, adopted (c)+(a), hand 7 window** (sem: SEM-gx-engine-080): `engine_version` is
/// > recorded; the version accessor is the distributable's hand's job.
///
/// 44 §2.2 gives `GET /healthz` the field `engine_version`, and hand 5 answered it with
/// `env!("CARGO_PKG_VERSION")` **inside gx-api** — which is gx-api's version, not the engine's. 41
/// §2 puts both crates in one workspace at one version, so the two strings are equal today and the
/// borrow was invisible; §52 put the fix in the hand that builds the distributables, because 47 §4's
/// upgrade runbook ("the journal schema's pre-upgrade verification condition is that `gx replay`'s
/// deterministic replay agrees between the old and new binaries" (sem: SEM-gx-engine-081)) is
/// a procedure an operator runs against a **version number**,
/// and a number reported by the wrong crate is a procedure run against the wrong thing.
///
/// Also reachable as [`Engine::version`], which is the spelling a caller holding an engine uses.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What opening an append-only file found, borrowed from gx-log (42 §3.13, AC-069's shape).
///
/// Re-exported so that a caller of this crate does not have to name gx-log to read the result of
/// [`store::EngineJournal::open`]. See the crate documentation for why it is borrowed rather than
/// redeclared.
pub use gx_log::Recovery;

/// Everything this crate can refuse to do.
///
/// The vocabulary table below is the **E-M2-23 / H-3** form that gx-core, gx-gate and gx-substrate
/// each carry: one declared list of kinds, one `kind()` written without a `_` arm, and a test that
/// reads the variants out of this file and compares. A variant added without a row is a compile
/// error at `kind()` and a failing probe at the table, which is what makes the list a definition
/// rather than a description.
///
/// `Clone` and `PartialEq` on purpose -- a test compares two of these -- which is why the I/O arm
/// carries a kind and a message instead of a [`std::io::Error`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The canonical layer refused. Carried transparently: gx-canon's `Error` already names which
    /// clause of 42 §2.1 caught the value, and restating it as a string here would lose that.
    #[error(transparent)]
    Canon(#[from] gx_canon::Error),

    /// gx-core refused to build a value. Carried transparently for gx-canon's reason: the lower
    /// crate already names which of its invariants caught the value (`OrderExceeded`,
    /// `CreatedAtNegative`, `IntentIdUnset`), and restating it as a string here would lose that.
    /// The shape gx-substrate's `Error::Core` established in M4.
    #[error(transparent)]
    Core(#[from] gx_core::Error),

    /// The filesystem refused. `action` says what was being attempted, so a reader can tell
    /// "could not open the journal" from "could not fsync it" (sem: SEM-gx-engine-082) -- the same `ErrorKind` and
    /// completely different facts about durability (43 §7's write-ahead is only true if the
    /// second one succeeded).
    #[error("{action} ({path}): {detail}")]
    Io {
        action: &'static str,
        path: std::path::PathBuf,
        kind: std::io::ErrorKind,
        detail: String,
    },

    /// Bytes that cannot be written or read as this crate's framing describes them.
    ///
    /// A *malformed* value is this crate's own statement about a size or a shape; a journal record
    /// that will not decode is not an error at all on the read side, because a torn tail is the
    /// ordinary shape of a crash -- see [`replay()`].
    ///
    /// Both of the engine's receiving mouths raise it, which is why the message names neither: hand
    /// 1's journal produces it for a record over [`MAX_RECORD_BYTES`], and hand 3's
    /// [`store::BlobStore`] for a blob over [`MAX_BLOB_BYTES`], for one that will not rebuild into a
    /// delta, and for one that does not hash to the name it was filed under. The message said "the
    /// journal record is malformed" until the blob store began raising it, at which point every
    /// blob refusal was reporting itself as a journal fault -- "do not spell a bad argument as an
    /// apply failure" (§33 M4H5-5; sem: SEM-gx-engine-083) applies to the words as much as to the variant.
    #[error("malformed: {detail}")]
    Malformed { detail: String },

    /// An [`EscrowedInverse`] whose `status` and `inverse_delta` disagree (42 §3.12).
    ///
    /// 42 §3.12 types `inverse_delta` as a `PlannedDelta` and, two rows down, defines
    /// `InverseStatus::Unavailable` as "the case where `invert()` returns None (cannot be
    /// constructed)" (sem: SEM-gx-engine-084). Those two
    /// sentences cannot both hold of one value: there is no delta to hold when none could be
    /// constructed. [`EscrowedInverse`]'s constructors keep the two in step and this is what they
    /// answer with when a caller tries to build the contradiction. Raised as **M5H1-3**.
    #[error("the escrowed inverse is inconsistent: {detail}")]
    InconsistentEscrow { detail: String },

    /// An [`EscalationTicket`](gx_gate::EscalationTicket) whose `id` is not a digest of its
    /// contents (**E-6**, hand 6).
    ///
    /// The checked constructor E-6 asks for -- "reading a ticket back requires a checked
    /// constructor" (sem: SEM-gx-engine-085) -- as a
    /// refusal. 42 §1.3 makes `TicketId` the CID of `{transformation, reasons, required_approval}`
    /// and gx-gate mints one when it escalates; a ticket that arrives at the engine from anywhere
    /// else is *claiming* to hash to its own name until something recomputes it.
    /// [`pipeline::Engine::verify`] does, on the way in, which is the same place **E-5** injects
    /// the clock 41 §6 keeps out of gx-gate.
    ///
    /// Its own variant rather than [`Error::Malformed`] for [`Error::InconsistentEscrow`]'s reason:
    /// "these bytes are not a shape this crate can read" and "this value's name disagrees with its
    /// contents" (sem: SEM-gx-engine-086) are different facts, and only the second one names a collaborator that is wrong.
    #[error("the escalation ticket is inconsistent: {detail}")]
    InconsistentTicket { detail: String },

    /// An [`EvidenceSource`] could not be reached (**M5-03, adopted (a)**, **E-M5-4**).
    ///
    /// The one refusal that becomes `AbortReason::VerifierUnavailable`. It exists as its own
    /// variant because "the collector could not be reached" and "the filesystem refused"
    /// (sem: SEM-gx-engine-087) are
    /// different facts about a deployment, and a `collect` implementation that had to reach for
    /// [`Error::Io`] to say the first one would be saying the second.
    ///
    /// Not every `Err` from `collect` is this variant -- any of them is unreachability, because a
    /// collector that failed did not collect. This is the name for the case where that is *all*
    /// there is to say.
    #[error("the evidence source could not be reached: {detail}")]
    EvidenceUnavailable { detail: String },

    /// `canon(canon(x)) != canon(x)` (43 T-8's guard, 12 F0 T3).
    ///
    /// AC-033's abnormal case, as a value: "in the abnormal case where a broken canon
    /// implementation that returns an idempotence violation is injected, an error is returned and
    /// there is no transition to Canonicalized" (sem: SEM-gx-engine-088). It is an [`Error`] and not an `AbortReason` because
    /// the acceptance criterion asks for exactly that -- an error, and no transition -- which
    /// leaves the transformation in `Admitted` where a caller can look at it.
    #[error("canon is not idempotent for {transformation:?}: {detail}")]
    NotIdempotent {
        transformation: gx_core::TransformationId,
        detail: String,
    },

    /// Something the caller named is not here (44 §2.3's `NOT_FOUND`, CLI exit 6).
    ///
    /// A draft, a transformation, or an adapter for a substrate nobody registered. The third is
    /// **M5-07, adopted (a)**'s consequence and is deliberately not a lifecycle state: 43 §1 has no room
    /// for "the substrate is unknown" (sem: SEM-gx-engine-089), and inventing one would put a deployment mistake into the
    /// state machine the spec fixes.
    #[error("no {what} named {id}")]
    NotFound { what: &'static str, id: String },

    /// A transition was asked for from a state 43 §3 does not offer it from (44 §2.2's
    /// `INVALID_STATE`, 409).
    ///
    /// Named rather than folded into a no-op, because "the transition did not apply" and "the
    /// transition applied and changed nothing" (sem: SEM-gx-engine-090) are the two things req/29 §4 refuses to give one
    /// face.
    #[error("{id} is {state}, and 43 §3 has no `{attempted}` from there")]
    InvalidState {
        id: String,
        state: &'static str,
        attempted: &'static str,
    },

    /// A `SubstrateAdapter` refused (44 §2.3's `ADAPTER_ERROR`, 502).
    ///
    /// `action` names which of the seven methods, because "the object could not be read" and "no
    /// plan could be made for it" (sem: SEM-gx-engine-091) are different facts about the same locator, and an engine that
    /// flattened them would make an adapter bug and a missing file look alike.
    #[error("the adapter refused to {action}: {detail}")]
    Adapter {
        action: &'static str,
        detail: String,
    },

    /// gx-log refused (43 T-11's `ledger.append`, and the inclusion proof taken after it).
    ///
    /// `action` names which, for [`Error::Adapter`]'s reason: "the ledger already holds a different
    /// receipt for this transformation" (`Error::Conflict`, which is INV-S3's guard in the layer
    /// below) and "the ledger could not be fsynced" (sem: SEM-gx-engine-092) are different facts about a deployment, and an
    /// engine that flattened them would make a durability failure and an exactly-once violation
    /// look alike.
    #[error("the ledger refused to {action}: {detail}")]
    Ledger {
        action: &'static str,
        detail: String,
    },

    /// gx-witness refused (43 T-11's receipt issue, and the ledger digest taken before it).
    ///
    /// Separate from [`Error::Ledger`] because the two crates answer different questions and 42
    /// §3.10's schema check (ASM-14's obligations) is the engine's own bug when it fires: the engine
    /// builds the payload, so a `Schema` refusal means this crate filled a field ASM-14 says must be
    /// empty, or left one it says must be filled.
    #[error("the witness refused to {action}: {detail}")]
    Witness {
        action: &'static str,
        detail: String,
    },

    /// 🔴 The canon has no way to write down what happened (hand 4, **M5H4-3**).
    ///
    /// Not a failure of the run and not bad input: the transformation is in a state 43 reaches and
    /// 42 has no shape for. The one instance in v0.1 is a `CommitReceipt` for a T-4e degraded
    /// admission -- 43 T-4e continues the pipeline with **no verdict** (the gate was never asked),
    /// and 42 §3.10 types `ReceiptPayload.verdict` as a `VerdictSummary` with a non-optional
    /// `proof_digest`. Minting an empty digest to fill it would put a proof in a receipt for an
    /// admission no gate made, which is the thing §32 M4H4-2 refused twice.
    ///
    /// So the engine refuses and says which sentence it could not satisfy. "Do not hide the absence
    /// of a check" (sem: SEM-gx-engine-093) (§37's instruction for blocker item 5) applied to a representation gap rather than to a check.
    #[error("{what} cannot be written down: {detail}")]
    Unrepresentable { what: &'static str, detail: String },

    /// 🔴 **DR-43-2 / `req/38` §148** — another `gx` process holds this project's writer lock.
    ///
    /// Its own variant, and the reason is the one Λ4's quotient discipline keeps asking for: every
    /// other refusal in this enum is a statement about the **request** ("that id is not here", "the
    /// gate said no", "the adapter refused"), and this one is a statement about **when** it arrived.
    /// Folding it into [`Error::Io`] would tell a caller that the filesystem broke, and a caller
    /// whose correct response is "wait a moment and send it again" would instead file a bug. That is
    /// the same argument gx-api's `UNAVAILABLE` won in §53, one layer down.
    ///
    /// Raised only by [`store::ProcessLock::acquire`]. `Engine` never takes the lock (M5H5-6,
    /// adopted (a)) and therefore never raises this; the callers that open a project do.
    // 🔴 **R10 / audit 9 L-01** — the note is a **note**, and the message now says so.
    //
    // `ProcessLock::take` writes "<pid> <verb>" into the lock file after the `try_lock` succeeds,
    // and never reads it back as meaning (its own doc comment says as much). The refusal, however,
    // printed it in the position of the answer to "who holds this?". `req/236` §6 measured what
    // that costs: a python process held the flock, `gx repair` answered `BUSY` "another gx process
    // is writing to …/.gx/LOCK (**3498834 gx repair**)", and 3498834 was a gx that had exited — the
    // last one to have taken the lock and written a note. The exclusion is the operating system's
    // and it was correct; only the attribution was invented.
    //
    // No liveness check is added: the holder may be any process that opened the file (audit 9's
    // arm was not a gx at all), so a `/proc` probe would replace one guess with another. The
    // sentence stops guessing instead.
    #[error("another gx process is writing to {path} (last note in that file: {holder:?} — the note is written by whichever process took the lock and is never re-read, so it may name one that has since exited)")]
    Busy {
        /// The lock file that is held.
        path: std::path::PathBuf,
        /// What the holder wrote about itself, for a human. Never read as meaning.
        holder: String,
    },

    /// 🔴 **DR-43-1, adopted (a)** (`req/38` §132 ruling 2, `req/182` H-15) — the world moved after
    /// the transformation being undone committed, so its escrowed inverse was **not** applied.
    ///
    /// Its own variant for [`Error::Busy`]'s reason, one question over. Every other refusal in this
    /// enum is a statement about the request or about this process; this one is a statement about
    /// **the substrate**, and it is the one refusal whose correct response is neither "retry" nor
    /// "file a bug" but "look at what changed and decide". Folding it into [`Error::InvalidState`]
    /// would have said the state machine refused a transition — it did not; the transition was
    /// never asked for, because the precondition an undo exists to preserve was already gone.
    ///
    /// 44 already has the word for it: `PRECONDITION_CHANGED` (409, CLI exit 3), which is what
    /// T-10a's CAS answers with at commit time. This is the same fact, measured before anything is
    /// minted. **No new exit number is created** (`req/38` §132 ruling 2).
    ///
    /// The message names the **scope** and not the digests: `gx_core::FingerprintBytes` has a
    /// deliberately opaque `Debug` because "the canon fixes no readable spelling for a fingerprint",
    /// so a refusal that printed one would be minting a spelling in a log line. The bytes are
    /// carried as values for a caller that wants to compare them, and the sentence names the place.
    #[error(
        "the world at {scope} is not the world {id} attested when it committed, so its escrowed \
         inverse was not applied (DR-43-1: an undo does not overwrite a change it cannot account \
         for)"
    )]
    WorldMoved {
        /// The transformation whose undo was refused.
        id: String,
        /// 42 §3.10's `postcondition_fingerprint`, as the caller read it out of the commit receipt.
        expected: gx_core::FingerprintBytes,
        /// What `adapter.precondition(adapter.snapshot(locator))` answered now.
        found: gx_core::FingerprintBytes,
        /// 42 §3.5's scope — the readable half of a fingerprint, and the only half printed.
        scope: String,
    },
    /// 🔴 **R3 / `req/222` H-01, H-02** — the undo's CAS had no evidence to run against, so it is
    /// refused rather than skipped (`req/38` §160 ruling 2).
    ///
    /// Carries the same `gx_code`, exit status and HTTP status as [`Error::WorldMoved`]
    /// (`PRECONDITION_CHANGED` / 3 / 409) and mints none of its own: the caller's correct response
    /// is identical — look at the target and at `.gx/receipts/`, then decide — and `req/38` §132
    /// ruling 2's "no new exit number" is still standing. What differs is the sentence, because
    /// "the world moved" and "this process cannot tell whether the world moved" are different
    /// facts and an operator who is told the first when the second is true will go looking for a
    /// third party who does not exist.
    ///
    /// 🔴 **R5 / `req/227` M-07** — the second half of the sentence is now the *variant's*, because
    /// one of them pointed at a road that does not exist. A deployment that keeps no receipt
    /// archive at all has no `.gx/receipts/` to restore anything into, and restoring one would
    /// change nothing: the archive handle is `NoArchive` and answers `None` whatever is on the
    /// disk. Measured by an embedder in `req/227` probe D — `409`, and the advice underneath it was
    /// about a directory the project does not have.
    #[error(
        "the undo of {id} was refused because its precondition could not be checked: {reason} \
         (DR-43-1 as repaired by req/38 §160: an undo whose signed postcondition cannot be read is \
         refused, not fired -- {remedy})"
    )]
    WitnessMissing {
        /// The transformation whose undo was refused.
        id: String,
        /// [`crate::WitnessMissing::reason`] -- which trust is missing.
        reason: &'static str,
        /// 🔴 **R5 / `req/227` M-07** — [`crate::WitnessMissing::remedy`]: what this deployment can
        /// actually do about it.
        remedy: &'static str,
    },
}

/// The vocabulary of [`Error`], declared once (**E-M2-23**).
///
/// In declaration order, which is also the order of the `match` in [`Error::kind`]:
/// `tests/engine_shape.rs` reads the variant names out of `src/lib.rs` and compares them with this
/// array, so the two lists cannot drift apart in either direction.
/// Four of these carry a 44 §2.3 `gx_code` that M6's API surface will map them onto (sem: SEM-gx-engine-094), and four are
/// `INTERNAL`: `NotFound` → `NOT_FOUND`, `InvalidState` → `INVALID_STATE` (44 §2.2), `Adapter` →
/// `ADAPTER_ERROR`, `EvidenceUnavailable` → `VerifierUnavailable`'s abort rather than an API code.
/// The mapping is written here and checked nowhere: 44's surface is M6 (req/78 N-01), and a probe
/// asserting a correspondence nothing yet consumes would be a probe about a table.
pub const ERROR_KINDS: [&str; 17] = [
    "Canon",
    "Core",
    "Io",
    "Malformed",
    "InconsistentEscrow",
    "InconsistentTicket",
    "EvidenceUnavailable",
    "NotIdempotent",
    "NotFound",
    "InvalidState",
    "Adapter",
    "Ledger",
    "Witness",
    "Unrepresentable",
    "Busy",
    "WorldMoved",
    "WitnessMissing",
];

impl Error {
    /// Which of [`ERROR_KINDS`] this refusal is.
    ///
    /// No `_` arm: a variant added tomorrow stops this function from compiling, which is the point.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Error::Canon(_) => "Canon",
            Error::Core(_) => "Core",
            Error::Io { .. } => "Io",
            Error::Malformed { .. } => "Malformed",
            Error::InconsistentEscrow { .. } => "InconsistentEscrow",
            Error::InconsistentTicket { .. } => "InconsistentTicket",
            Error::EvidenceUnavailable { .. } => "EvidenceUnavailable",
            Error::NotIdempotent { .. } => "NotIdempotent",
            Error::NotFound { .. } => "NotFound",
            Error::InvalidState { .. } => "InvalidState",
            Error::Adapter { .. } => "Adapter",
            Error::Ledger { .. } => "Ledger",
            Error::Witness { .. } => "Witness",
            Error::Unrepresentable { .. } => "Unrepresentable",
            Error::Busy { .. } => "Busy",
            Error::WorldMoved { .. } => "WorldMoved",
            Error::WitnessMissing { .. } => "WitnessMissing",
        }
    }
}

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Carry a [`std::io::Error`] into this crate's error type without losing what it said.
///
/// Lifted from `gx-log/src/store.rs`, where the reasoning is written out: the kind is kept as a
/// value and the message as a string because [`Error`] is `Clone` and `PartialEq` and
/// `std::io::Error` is neither.
pub(crate) fn io_error(
    action: &'static str,
    path: &std::path::Path,
    source: &std::io::Error,
) -> Error {
    Error::Io {
        action,
        path: path.to_path_buf(),
        kind: source.kind(),
        detail: source.to_string(),
    }
}
