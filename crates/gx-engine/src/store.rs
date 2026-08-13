//! The engine's own records: the write-ahead journal (42 §3.13) and the escrowed inverse
//! (42 §3.12).
//!
//! Spec: 42 §3.12 and §3.13 for the field tables, 43 §3 for the journal vocabulary and §7 for the
//! ordering that makes it a *write-ahead* log, 41 §6 for 「全canonical encodeはgx-canon経由のみ」.
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
//! decides a next one -- there is no state table in this hand at all, which is **M5-17 採(b)**'s
//! shape from the other side: 「Draft 相は journal だけが持ち状態表は Candidate 以降」. The journal
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
    AbortReason, Actor, Cid, Fingerprint, IntentId, SubstrateKind, Timestamp, TransformationId,
    VerdictKind,
};
use gx_log::Recovery;
use gx_substrate::PlannedDelta;
use gx_witness::Provenance;

use crate::replay::{replay, EscrowRow, LENGTH_BYTES};
use crate::{io_error, Error, Result};

/// The largest journal record this crate will write or allocate for.
///
/// A journal record is a handful of ids, a timestamp and -- in `Planned` -- one [`Fingerprint`],
/// whose scope is already bounded by `gx_core::MAX_SCOPE_BYTES` (1 KiB). So this is three orders of
/// magnitude of headroom rather than a tuning parameter. It exists because the length header is
/// read from a file that may have been damaged: without a ceiling, four corrupted bytes ask for a
/// four-gigabyte allocation before anything has had a chance to refuse them.
///
/// **This is the first of M5-20's ceilings.** §37's ruling is 「engine 受取口の decode 前 byte 上限
/// 1 箇所+契約行 1:1 probe」; the journal is one receiving mouth and the blob store that hand 3
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
/// ([`FingerprintRecord::into_fingerprint`]) -- which is E-6's rule (「読み戻しは checked
/// constructor 必須」) applied one type over.
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
/// AC-038 逐語: 「自動巻き戻し試行が発生し、結果は`Aborted(ApplyFailed)`として記録され、**journal/
/// Receiptに巻き戻し試行の有無（成功/失敗）が記録される**」. Neither seat exists. 42 §3.10's
/// `ReceiptPayload` has no field for it, and ASM-14 issues no receipt at all for an `Aborted`
/// transformation -- there are two kinds and both belong to a verdict or to a commit that succeeded.
/// That leaves the journal, and 42 §3.13's `Aborted` row is three fields none of which is this.
///
/// So the fact is carried where 43 T-10c itself puts the record (「journal: `Aborted{id,
/// ApplyFailed}`」) and the row gains a field, which is exactly the shape **M5H2-1 / E-M5-7** took
/// for `Verdict`: an implementation cannot satisfy an acceptance criterion out of a table that has
/// no room for it, so it writes the true thing and raises the divergence. Raised as **M5H4-2**.
///
/// # Four states, not three
///
/// `Option<Rollback>` and not `Rollback`: an abort that is not T-10c has no rollback question to
/// answer, and `None` says so. [`Rollback::NotAttempted`] is the different fact that the question
/// arose and the answer was no -- 43 T-10c's guard is 「escrow済みinverseが**あれば**」 -- which is
/// req/29 §4's 「skip と pass を同じ顔にしない」 at the one place a v0.1 would blur it.
///
/// 🔴 `NotAttempted` is **unreachable in v0.1** and is named anyway. `SubstrateAdapter::invert`
/// returning `None` makes a transformation `Escalated` at T-3 (**E-M3-4**), so nothing without an
/// escrowed inverse reaches `Committing` until hand 6 resolves an escalation. The same shape as
/// `InverseStatus::Expired`, which 42 §3.12 lists and v0.1 never writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rollback {
    /// No inverse was escrowed, so 43 T-10c's guard did not open. Unreachable in v0.1.
    NotAttempted,
    /// The escrowed inverse was handed to the adapter and the adapter accepted it.
    Succeeded,
    /// The escrowed inverse was handed to the adapter and the adapter refused. 43 T-10c is
    /// 「ベストエフォート、結果に関わらず次へ」, so this does not change the abort reason.
    Failed,
}

impl Rollback {
    /// The three, in declaration order.
    pub const ALL_KINDS: [&'static str; 3] = ["NotAttempted", "Succeeded", "Failed"];

    /// Which of [`Rollback::ALL_KINDS`] this is. No `_` arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Rollback::NotAttempted => "NotAttempted",
            Rollback::Succeeded => "Succeeded",
            Rollback::Failed => "Failed",
        }
    }
}

/// One line of the engine's write-ahead log (42 §3.13, 43 §3).
///
/// **Thirteen variants**: 42 §3.13's eleven, plus `ApplyStarted` (**E-M5-1**) and
/// `ProvenanceDerived` (**M5-25 採(a)**, hand 4). Each variant names the transition or transitions
/// of 43 §3 that write it, in the comment beside it, and `tests/journal_vocabulary.rs` checks those
/// comments against the canon rather than trusting them.
///
/// # The two places this is not 42 §3.13's literal text
///
/// * **`DraftCreated` is keyed on `IntentId`** (**E-M5-3**). 42 §3.13 writes a `transformation`
///   field; 43 T-1 writes 「`TransformationId`はまだ確定しない（delta/target未確定）」 and puts only
///   `intent_id` in its journal cell. 51 §8.1 rules the precedence -- 「journal record名は43 §3遷移表
///   を正本とする。42 §3.13は…旧記述であり、矛盾時は43を優先する」 -- and §37 applies it for the
///   first time. Writing 42's version would mean minting a `TransformationId` before `plan` has run,
///   which is ASM-11's two-stage identity broken at the first step.
/// * **`ApplyStarted` exists** (**E-M5-1**). See the crate documentation for the three-line
///   counter-example (req/78 §3.2 Λ4) it closes. Its place in the order is between `InverseEscrowed`
///   (T-10b) and `Committed` (T-11): the record says 「the adapter was asked」, which is the fact
///   recovery needs and the only fact that separates 「the world did not move」 from 「the world moved
///   and nothing recorded it」.
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
    /// # 🔴 Two fields 42 §3.13 does not write — **E-M5-13**, implemented under §47 M6-14 採(a)
    ///
    /// 42 §3.13 gives this record four fields and 43 T-2's cell gives it the second id. Two later
    /// hands each found a third thing missing, and §43 merged their tickets into one erratum —
    /// 「**E-M5-13 は『`Planned` record に locator(M5H5-2)と parents(本件)を足す』1 つの erratum に
    /// 拡張**」:
    ///
    /// * **`locator`** (M5H5-2) — 43 §7-3c resumes an interrupted plan, and a resume needs to know
    ///   *what was being planned against*. Without it v0.1 folded the case into
    ///   `Aborted(InternalError)`, and §42 accepted in writing that this 「配線 bug と嘘をつく」.
    ///   The locator is the adapter's own spelling of the position (42 §3.3) and is already inside
    ///   the `Intent` whose CID is `intent_id`; what the journal lacked was a way to read it back
    ///   without the body, which ASM-9 does not keep.
    /// * **`parents`** (M5H6-6) — 43 T-12's guard is 「`T_u.parents`が`T_o.id`を含み」, and until this
    ///   field the supersede metadata lived only in the in-memory `Transformation`. A crash between
    ///   `Planned` and the commit lost it, so the guard could not be re-checked from the journal
    ///   alone. `Engine::undo` is the one producer of a non-empty list.
    ///
    /// # Why the window was M6 hand 1 and not M7
    ///
    /// 47 §4 makes journal-schema compatibility an upgrade precondition and 33 NFR-024 permits a
    /// breaking change in `0.y.z` with a CHANGELOG entry. **M6 builds the first distributable.**
    /// Before it ships the cost of changing this shape is zero and after it the cost is every user's
    /// journal, so the decision is a *window* rather than a cost comparison (§47 M6-14's 規律51 form).
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
    /// than advisory: 「`enforced=false`と`fail_posture_engaged=true`を必ずreceiptに刻む」. The two
    /// texts disagree, and 51 §8.1 已 settles which wins -- 「journal record名は43 §3遷移表を正本と
    /// する。42 §3.13は…旧記述であり、矛盾時は43を優先する」. **E-M5-3** applied that clause to
    /// `DraftCreated`'s fields and hand 1 applied it to `Planned`'s in the same breath; this is its
    /// third application, and it is **raised as M5H2-1** rather than treated as settled, because
    /// applying a ruled clause to a case nobody has ruled on is still a hand's reading.
    ///
    /// * **`fail_posture_engaged`** is `false` for T-4a/T-4b/T-4c and `true` for T-4e alone. It is
    ///   what makes 「admitted」 and 「admitted because the verifier could not be reached」 different
    ///   records, which is INV-S5's requirement (「`enforced=false`のCommittedは…区別可能な形で」)
    ///   reaching the journal rather than only the receipt.
    /// * **`verdict_digest` is an `Option`**, and `None` belongs to T-4e alone. A digest identifies
    ///   a verdict, and under T-4e **no verdict was computed** -- the gate was never reached. The
    ///   alternative was to mint an empty `AdmitProof` and digest that, which would put a proof in
    ///   the record for an admission no gate made: 「未実装」 and 「失敗」 wearing one face, which
    ///   §32 M4H4-2 refused twice. `None` beside `fail_posture_engaged = true` says the true thing.
    Verdict {
        transformation: TransformationId,
        kind: VerdictKind,
        verdict_digest: Option<Cid>,
        fail_posture_engaged: bool,
        at: Timestamp,
    },
    /// T-5 / T-5b -- a human answered. 42 §3.13: 「kindはAdmit|Denyのみ」, which is a fact about the
    /// two transitions rather than about the type; 43 has no `Escalated → Escalated` edge, and
    /// [`crate::Engine::escalation`] is the guard that says so.
    ///
    /// # 🔴 Two fields 42 §3.13 does not write, and AC-071/072 do (hand 6)
    ///
    /// 42 §3.13 gives this record three fields. AC-071 逐語 asks for more:
    ///
    /// > 発行されるreceipt trail（journal/Receiptメタデータ）に`Evidence(HumanDecision)`
    /// > （decision=Admit, reason, 裁定者actor）が含まれることを確認する
    ///
    /// and AC-072 asks the same of a rejection (「journal記録に`Evidence(HumanDecision)`（decision=
    /// Deny, reason）が含まれる」). **E-M2-3** retired the `Evidence` variant those two name -- 「43
    /// T-5 の「人間裁定receipt（署名済み）」 は receipt」 -- so the three facts have nowhere to go but
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
    HumanDecision {
        transformation: TransformationId,
        kind: VerdictKind,
        reason: String,
        actor: Actor,
        at: Timestamp,
    },
    /// T-8 / T-8r `canonicalize`. `enforced` is `Some(false)` for T-8r only (42 §3.13), which is
    /// why it is an `Option` and not a `bool` -- 「不明」 and 「false」 are different records.
    Canonicalized {
        transformation: TransformationId,
        canonical_cid: Cid,
        enforced: Option<bool>,
        at: Timestamp,
    },
    /// T-9 `commit_start`. 43 §4: 「**side-effect実行前に必ずjournal化**」.
    CommittingStarted {
        transformation: TransformationId,
        at: Timestamp,
    },
    /// 🔴 **M5-25 採(a)** -- the provenance of this transformation, derived by the engine and
    /// written to the journal. **D-7 の 3 回目の決着**, and the second record 42 §3.13 does not
    /// list.
    ///
    /// > **M5-25 採(a)+journal**: **D-7 の 3 回目を決着——engine が `Provenance` の producer になる**
    /// > (器=`ProvenanceInputs` は実在)。出す先は **journal**(ASM-9 の digest-only と整合・receipt
    /// > wire は動かさない)
    ///
    /// gx-witness has carried `Provenance` and `Environment` since M2 with **zero code consumers**
    /// through M3 and M4, and 42 §3.9's `input_objects` -- 「plan/verify中にadapterが読み取った入力
    /// スナップショット群」 -- can only be collected by the thing that watched the adapter read them.
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
    /// 43 T-10b's guard is 「inverse構成可能（`Some`）」 and its journal cell names a CID, so 42
    /// §3.13 typed one. 42 §3.12 nevertheless defines `InverseStatus::Unavailable` as
    /// 「`invert()`がNoneを返した場合（構成不能）」 -- a state the journal had no way to spell.
    /// §40 rules the fix and dates it:
    ///
    /// > **M5H3-2 採(a)方向・実装窓=手 6**=**E-M5-9(予約)**: `InverseEscrowed.inverse_cid` の
    /// > `Option` 化(42 §3.13 erratum・51 §8.1 優先条項の 4 度目)を、**手 6 の escalation 承認が
    /// > 経路を現実にする同 turn で実装**する
    ///
    /// The path is T-5's. **E-M3-4** escalates a transformation whose `invert` answers `None`, so
    /// until a human could approve one nothing without a constructible inverse ever reached
    /// `Committing`; hand 4 wrote no record at all in that arm and said so. Now the arm is live,
    /// and 「we asked and there is none」 has to be distinguishable from 「we never asked」 (§32
    /// M4H4-2) -- which, in a journal, means a record that says the first thing.
    ///
    /// Symmetric with **E-M5-6**, which made the same field optional in the in-memory
    /// [`EscrowedInverse`] and kept it in step with `status` through checked constructors. The two
    /// errata are one decision on both sides of the door: [`BlobStore::escrowed`] is where a row
    /// read back from here goes through [`EscrowedInverse::restore`].
    InverseEscrowed {
        transformation: TransformationId,
        inverse_cid: Option<Cid>,
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
    /// T-12 -- an inverse of this transformation reached `Committed`. 43 §5: 「取消は新規commitで
    /// あり書換ではない」, so this record is written *about* `transformation` and changes none of
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
pub const JOURNAL_RECORD_KINDS: [&str; 13] = [
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
    "Committed",
    "Aborted",
    "Superseded",
];

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
            EngineJournalRecord::Committed { .. } => "Committed",
            EngineJournalRecord::Aborted { .. } => "Aborted",
            EngineJournalRecord::Superseded { .. } => "Superseded",
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
            | EngineJournalRecord::Committed { at, .. }
            | EngineJournalRecord::Aborted { at, .. }
            | EngineJournalRecord::Superseded { at, .. } => *at,
        }
    }

    /// The `TransformationId` this record is about, where there is one.
    ///
    /// `None` for `DraftCreated` alone, and that `None` is **E-M5-3** in the type system: a draft
    /// has no `TransformationId` to be about. A state table keyed on `TransformationId` therefore
    /// cannot hold a draft, which is **M5-17 採(b)** -- 「Draft 相は journal だけが持ち状態表は
    /// Candidate 以降」 -- expressed as a signature rather than as a convention.
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
/// raised and §37 answered with 採(a) -- `Consumed { by }` is written at T-12, in one place,
/// together with the [`SupersedeIndex`] entry. `Available` is T-10b's, and `Unavailable` is
/// **E-M5-9**'s: a T-10b whose `adapter.invert` answered `None`, which is reachable only after a
/// human approves an escalation (T-5), which is why the two errata land in one hand.
///
/// 🔴 `Expired` has **no writer at all**, and that is DR-9 rather than an omission: 「OSS coreの
/// 既定は無期限（期限管理・延長は商用tier機能）」 and req/78 N-06 keeps the enforcement of
/// `retained_until` out of v0.1. `tests/lifecycle_transitions.rs` asserts the absence, so the day
/// a hand adds one it has to say why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InverseStatus {
    /// The inverse is held and has not been used.
    Available,
    /// A transformation applied it and reached `Committed` (43 T-12).
    Consumed { by: TransformationId },
    /// `retained_until` passed. The *器* only: DR-9 puts期限の執行 in the commercial tier and
    /// req/78 N-06 keeps it out of v0.1, so nothing in this crate moves a status to this value.
    Expired,
    /// `SubstrateAdapter::invert` returned `None` -- no inverse could be constructed.
    Unavailable,
}

impl InverseStatus {
    /// The four, in 42 §3.12's order.
    pub const ALL_KINDS: [&'static str; 4] = ["Available", "Consumed", "Expired", "Unavailable"];

    /// Which of [`InverseStatus::ALL_KINDS`] this is. No `_` arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            InverseStatus::Available => "Available",
            InverseStatus::Consumed { .. } => "Consumed",
            InverseStatus::Expired => "Expired",
            InverseStatus::Unavailable => "Unavailable",
        }
    }
}

/// The undo guarantee's body: an inverse delta held for a committed transformation (42 §3.12,
/// DR-1(a)).
///
/// # The one place 42 §3.12 cannot be taken literally
///
/// The table types `inverse_delta` as a `PlannedDelta` and, two rows down, defines
/// `InverseStatus::Unavailable` as 「`invert()`がNoneを返した場合（構成不能）」. A value cannot both
/// hold a delta and record that none exists. So the field is an `Option` here and the two are kept
/// in step by the constructors: [`EscrowedInverse::held`] takes a delta and cannot be
/// `Unavailable`; [`EscrowedInverse::unavailable`] takes none and can be nothing else; and
/// [`EscrowedInverse::restore`], the door a hand-3 store read-back will come through, refuses the
/// contradiction with [`Error::InconsistentEscrow`]. **Raised as M5H1-3** -- the alternative
/// readings are 「`Unavailable` is recorded somewhere that is not this struct」 and 「the escrow
/// table has no row when no inverse exists」, and both are spec changes rather than implementation
/// choices.
///
/// # Why the payload is here at all
///
/// ASM-9 keeps bodies out of the system and stores digests. 42 §5 makes this the named exception --
/// 「**本体payloadを保持**（digest-onlyでは実際のundoが実行不能なため、§5でASM-9の例外として明記）」
/// -- because an undo that has only the digest of its own inverse cannot run. The exception is one
/// row wide and this is the row.
///
/// # Still no setters, now that T-12 exists
///
/// The status moves at T-12 and nowhere else (M5-16 採(a)), and hand 6 fires it -- **on the escrow
/// index, not on this value**. This struct is what [`BlobStore::escrowed`] rebuilds when a caller
/// wants the row *and* its body together; the index the engine mutates is
/// [`crate::EscrowRow`], which the journal reconstructs. So the promise hand 1 made (「`consumed_by`
/// will be added by the hand that fires the edge」) is kept by **not** adding one: the hand that
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
    /// `retained_until` is `None` for the OSS default -- DR-9: 「OSS coreの既定は無期限（期限管理・
    /// 延長は商用tier機能）」 -- and `Some` is accepted so that the field is a *器* rather than a
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
    /// A row with no delta rather than no row: 「we asked and the answer was no」 and 「we never
    /// asked」 are different facts, and a caller who has to tell them apart is exactly the caller
    /// an undo guarantee is for. The same distinction §32 M4H4-2 made between 「未実装」 and
    /// 「失敗」.
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
    /// The checked constructor E-6 asks for (「読み戻しは checked constructor 必須」), placed now
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
// M5-09 採(a) -- the supersedes index
// ---------------------------------------------------------------------------

/// Which transformation's inverse superseded which (43 T-12, ASM-43-2).
///
/// # Why it is a type here rather than a map in the engine
///
/// ASM-43-2 asks for a `superseded_by` field 「Transformation本体のcanonical構造には含めず、engine側
/// の索引情報として扱い、receiptの不変性(INV-S2)を壊さない」, and §37 fixes where it lives:
///
/// > **M5-09 採(a)**: supersedes 索引は store.rs・T-12 の照合は escrow の `inverse_delta` CID と
/// > `T_u.delta` CID の一致
///
/// A `BTreeMap` in `pipeline.rs` beside the state table would satisfy every behaviour and none of
/// that: the point of ASM-43-2 is that the edge is **not** part of the transformation, and a field
/// on the engine's own row is one refactor away from being written into one. Here it is a separate
/// structure with one writer and no way to unset an entry.
///
/// # 43 T-12's idempotency column, as a return value
///
/// 「同一`T_u`による重複supersede適用は無視（`superseded_by`が既に設定済みなら再設定しない）」.
/// [`SupersedeIndex::record`] answers whether it wrote, so the caller can tell 「the edge was drawn
/// now」 from 「the edge was already there」 -- which is what keeps T-12 from journalling a second
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
    /// supersede is recorded at all, so a second writer would be a second answer to 「which change
    /// undid this one」.
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
}

impl EngineJournal {
    /// Open the journal at `path`, creating it if it is not there, and replay what it holds.
    ///
    /// A tail that cannot be replayed is removed from the file before this returns, so the next
    /// append lands where the record sequence actually reached. See [`Recovery`] for what is
    /// reported about it, and [`crate::replay`] for why the refusal stops at the first bad record
    /// instead of skipping it. This is 43 §7-1's 「起動時 replay」 with nothing decided on top: the
    /// records come back, and what they *mean* for the state table is hands 2 through 6.
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

        let replayed = replay(&bytes);
        if replayed.recovery().torn_tail_bytes > 0 {
            file.set_len(replayed.good_bytes())
                .map_err(|e| io_error("cannot remove the torn tail of the journal", &path, &e))?;
            barrier(&file, &path, "cannot fsync the truncated journal")?;
        }
        if !existed {
            sync_parent_directory(&path)?;
        }

        let recovery = replayed.recovery();
        Ok(Self {
            file,
            path,
            records: replayed.into_records(),
            recovery,
        })
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
        let payload = cbor::encode(&record)?;
        self.write_and_sync(&payload)?;
        self.records.push(record);
        Ok(self.records.len() as u64 - 1)
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
    fn write_and_sync(&mut self, payload: &[u8]) -> Result<()> {
        let length = u32::try_from(payload.len())
            .ok()
            .filter(|n| *n <= MAX_RECORD_BYTES)
            .ok_or_else(|| Error::Malformed {
                detail: format!(
                    "a record of {} bytes is over the {MAX_RECORD_BYTES}-byte ceiling",
                    payload.len()
                ),
            })?;

        let mut record = Vec::with_capacity(LENGTH_BYTES + payload.len());
        record.extend_from_slice(&length.to_be_bytes());
        record.extend_from_slice(payload);

        self.file
            .write_all(&record)
            .map_err(|e| io_error("cannot write to the journal", &self.path, &e))?;
        barrier(&self.file, &self.path, "cannot fsync the journal")
    }
}

/// The durability barrier 43 §7's write-ahead ordering depends on.
///
/// `sync_all` and not `sync_data`, for `gx-log/src/store.rs`'s reason: a record appended past the
/// old end of file is only reachable if the new length reached the device with it.
fn barrier(file: &File, path: &Path, action: &'static str) -> Result<()> {
    file.sync_all().map_err(|e| io_error(action, path, &e))
}

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

// ---------------------------------------------------------------------------
// M5-05 採(a) -- the content-addressed blob store
// ---------------------------------------------------------------------------

/// The largest blob this store will write, or read back **before decoding it**.
///
/// 🔴 **The second of M5-20's ceilings.** §37's ruling is 「engine 受取口の decode 前 byte 上限 1 箇所
/// +契約行 1:1 probe(M4H2-8 形)」, and the engine has exactly two mouths that take in bytes it did
/// not just write: the journal ([`MAX_RECORD_BYTES`], hand 1) and this store. A separate constant
/// for the same reason hand 1 gave for its own: two files with different contents and different
/// writers must be able to move independently, and sharing one constant would make one ceiling a
/// statement about the other.
///
/// A `u64` rather than hand 1's `u32` because this ceiling is compared against a **file length**
/// (`std::fs::Metadata::len`) before a single byte is read, which is what 「decode 前」 means here.
/// The journal's is compared against a length header inside a buffer that is already in memory.
///
/// The number is the same 1 MiB, and that is not a coincidence to be shared but a coincidence to be
/// noticed: `gx-adapter-fs` bounds a forward payload at 1 MiB (`MAX_FORWARD_PAYLOAD_BYTES`) and an
/// inverse at 1 MiB, so a delta any shipped adapter can build fits, and one that does not is refused
/// at the door rather than after the allocation it asks for.
pub const MAX_BLOB_BYTES: u64 = 1 << 20;

/// What [`BlobStore::put`] did.
///
/// The shape `gx_log::AppendOutcome` established in M2, for the same reason: 「it is stored」 and
/// 「it was already stored and nothing was written」 are different facts about a call, and a caller
/// that cannot tell them apart cannot measure **M4H6-3** -- 「residual CID が既存 delta CID と一致する
/// のは保管の一度性の裏返し=利点」 (req/38 §34), whose implementation form §37 folds into M5-05 採(a)
/// as 「同一 CID なら参照のみ登録」.
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
/// M4H1-3 採(a)). So the file's name is a digest of the file's contents by construction rather than
/// by convention, and [`BlobStore::get`] can check it.
///
/// Reading them back needs a type serde can build, and `PlannedDelta` deliberately has no
/// `Deserialize`: it is minted through a constructor that computes `reference`, and a derived
/// `Deserialize` would be a second door that lets a caller name a delta something it is not. This
/// mirror is therefore the same shape [`FingerprintRecord`] has one module up, for the same reason,
/// and hands its contents back **through `PlannedDelta::new`** (E-6: 「読み戻しは checked
/// constructor 必須」). **M5H1-4** is the ticket that asks whether mirrors or checked `Deserialize`
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

/// The one content-addressed store (**M5-05 採(a)**): deltas in, deltas out, keyed by their CID.
///
/// # Why one store holds both kinds of delta
///
/// §37's ruling is 「**CID キーの blob store 1 本**が `PlannedDelta` と `inverse_delta` の**両方**を
/// 持つ」, and the reason is **M4H6-3**: an inverse whose payload happens to equal a forward delta's
/// has the same CID, and 「保管の一度性」 means the second one is a *reference* rather than a copy.
/// Two stores would each hold their own idea of that one blob. There is no separate escrow store:
/// 42 §5's exception (「`EscrowedInverse.inverse_delta`（payload本体）… **保管する（必須）**」) is a
/// statement about a *body*, and a body is what this store keeps. The escrow **index** -- which
/// transformation the inverse belongs to, and what has become of it -- is reconstructed from the
/// journal by [`crate::replay::reconstruct`], and [`BlobStore::escrowed`] is where the two meet.
///
/// **M5H1-6's condition is met**: this store declares no third spelling of 「what opening a file
/// found」. A directory of whole files has no torn tail -- a blob is either there and complete, or
/// absent, or the wrong size, and the last two are refusals rather than recoveries -- so
/// `gx_log::Recovery` stays the one word for the one idea it names.
///
/// # The contract
///
/// | method | refuses when | reason |
/// |---|---|---|
/// | `put` | the encoded delta is over `MAX_BLOB_BYTES` | a blob that could be written and never read back is a trap the store sets for itself |
/// | `get` | the file is over `MAX_BLOB_BYTES` **before it is read**, or its contents do not rebuild into a delta whose CID is the name it was filed under | 「decode 前 byte 上限」 (M5-20 採(a)); and a blob that does not hash to its name is not the blob that was asked for |
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
        if path.exists() {
            // 🔴 M4H6-3: 「同一 CID なら参照のみ登録」. The early return **is** the reference: no file
            // is opened for writing, so a blob that is already here is untouched by a second put
            // even if the bytes offered differ (they cannot, without breaking the digest -- and
            // `get` is where that is caught).
            return Ok((cid, PutOutcome::AlreadyPresent));
        }

        let bytes = cbor::encode(&delta.identity_view())?;
        if bytes.len() as u64 > MAX_BLOB_BYTES {
            return Err(Error::Malformed {
                detail: format!(
                    "a delta of {} bytes is over the {MAX_BLOB_BYTES}-byte blob ceiling",
                    bytes.len()
                ),
            });
        }

        let mut file =
            File::create(&path).map_err(|e| io_error("cannot create the blob", &path, &e))?;
        file.write_all(&bytes)
            .map_err(|e| io_error("cannot write the blob", &path, &e))?;
        barrier(&file, &path, "cannot fsync the blob")?;
        sync_parent_directory(&path)?;
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
        // 🔴 M5-20 採(a), 「decode 前」 in one statement: the length that decides comes from the
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
    #[must_use]
    pub fn contains(&self, cid: &Cid) -> bool {
        self.path_of(cid).exists()
    }

    /// How many blobs are filed.
    ///
    /// Counted from the directory each time rather than cached: the cache would be a second answer
    /// to a question the filesystem already answers, and a store whose count disagreed with its
    /// contents would be the exact failure 「参照のみ」 is measured by.
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
    /// and names none, is [`Error::InconsistentEscrow`] -- so 「持っていないと言った物を保管した
    /// store」 cannot be reconstructed any more than it could be constructed.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cid_of(b: u8) -> Cid {
        Cid([b; 32])
    }

    /// `DraftCreated` is the one record with no `TransformationId` (**E-M5-3**, **M5-17 採(b)**).
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
