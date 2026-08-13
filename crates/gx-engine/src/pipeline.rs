//! The transitions: `submit` → `plan` → `verify` → `canonicalize` (43 §3, T-1 through T-8r).
//!
//! Spec: 43 §1 for the eleven states and §3 for the transition table this file is a transcription
//! of, 41 §5 for the commit protocol the four entry points here open, 32 FR-030..FR-033 for what
//! they must do, 34 AC-030..AC-033 for how that is judged. 41 §2 names this file, and **M5H1-5
//! 採(a)** (req/38 §38) settles that all eight entry points belong in it rather than in a module
//! split of this hand's invention.
//!
//! # Four of the eight, and the line between them
//!
//! | entry point | transitions | hand |
//! |---|---|---|
//! | [`Engine::submit`] | T-1 | **2** |
//! | [`Engine::plan`] | T-2 | **2** |
//! | [`Engine::verify`] | T-3, T-4a, T-4b, T-4c, T-4d, T-4e | **2** |
//! | [`Engine::canonicalize`] | T-8, T-8r | **2** |
//! | [`Engine::commit`] | T-9, T-10a, T-10b, T-10c, T-11 | **4** |
//! | `undo` | T-12 | 6 |
//! | `cancel` | T-7 | 6 |
//! | `escalation` | T-5, T-5b | 6 |
//!
//! The three that are not here are **absent, not stubbed**. `tests/engine_shape.rs` asserts both
//! halves, so a hand reaching forward into T-12 fails a probe rather than leaving a reviewer to
//! notice.
//!
//! # What the state is, and where it lives
//!
//! 42 §1.3-3: 「状態は`TransformationId`をキーとしたengine側の外部テーブル（エンジンストア）で管理
//! される」. So there is a table, and its key is a `TransformationId`.
//!
//! **A draft has no key.** 43 T-1 writes 「`TransformationId`はまだ確定しない（delta/target未確定）」,
//! and **M5-17 採(b)** settles what follows: 「Draft 相は journal だけが持ち状態表は Candidate 以降」.
//! There is therefore no draft table in this file. [`Engine::submit`] writes a journal record and
//! nothing else; [`Engine::plan`] is handed the same `Intent` again and re-derives its `IntentId`
//! rather than looking a body up. That is not an inconvenience worked around -- it is 42 §1.3-3 and
//! ASM-9 agreeing: the engine holds names and digests, not bodies.
//!
//! The one thing kept about drafts is a **set of the `IntentId`s the journal has seen**, and it is a
//! cache in the strict sense: [`Engine::open`] rebuilds it by replaying, and deleting it would cost
//! speed and no truth. req/78 §3.3's 則 1 is the rule -- 「`L`(状態表)は journal の関数である。
//! メモリ上の表は cache であって state ではない」.
//!
//! # Journal-first, in every transition
//!
//! 43 §7: every transition is journalled **before** its side effect. In this hand the side effects
//! are in-memory (the table), and the ordering is still what the code does: append, then mutate. It
//! matters more here than it looks, because hand 4's side effect is `adapter.apply` and the shape a
//! hand learns in the cheap case is the shape it writes in the expensive one.
//!
//! Two calls read the substrate before any of that: `adapter.snapshot` and `adapter.precondition`
//! in T-2, and `adapter.invert` in T-3. All three are reads. FR-035's 「engine自身がsubstrateへの
//! 変更処理を実行してはならない」 is about writes, and hand 4 adds the **one** write there is:
//! [`Engine::apply_once`], reached from [`Engine::commit`] and from nowhere else. FR-035 is not
//! 「no apply」 — it is 「the engine does not do the changing itself」 — and one call site behind a
//! CAS is the shape that makes the distinction measurable rather than asserted.
//!
//! # Three injection points, and why each one is a trait
//!
//! 41 §6: 「乱数・時刻はengine境界で注入」, which req/78 §3.3 則 3 reads as the shape of the type.
//! The clock and the seed arrive as arguments to every entry point rather than as a `Clock` object,
//! because every transition already carries an `at` into its journal record and a second road to the
//! same value would be a second answer. What are traits are the three things a *test* has to be able
//! to replace:
//!
//! * [`EvidenceSource`] — **M5-03 採(a)**. Its `Err` is the only producer of
//!   `AbortReason::VerifierUnavailable` in the workspace, which is what makes T-4d, T-4e and AC-036
//!   constructible; **E-M5-4** settles that 「到達不能の唯一の source は evidence collector」 because
//!   gx-gate is a library and cannot be unreachable.
//! * [`Canonicalizer`] — AC-033 asks for 「冪等性違反を返す壊れたcanon実装を注入した異常系」, so canon
//!   has to be replaceable. See the type for how 41 §6's 「全canonical encodeはgx-canon経由のみ
//!   （迂回禁止）」 survives that.
//! * `SubstrateAdapter` — **M5-07 採(a)**: the engine holds a registry and a caller registers into
//!   it, so gx-engine ships no adapter (N-13) and 「どの substrate でも同じ engine」 stays true of the
//!   artefact and not only of the prose.
//!
//! # What is deliberately not decided here
//!
//! 43 §8's waiting queue (a `Conflicts` transformation held at `Candidate`) is hand 5's, because it
//! needs the synchronisation hook §35 K-6 reserved for the engine layer. TTL (T-6) is hand 6's.
//! Neither is stubbed and neither is silently skipped: [`Engine::verify`] refuses a state it is not
//! written for rather than falling through it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use gx_core::{
    AbortReason, Actor, Cid, Commutation, CompositionMetadata, EnforcementMode, FailPosture,
    Fingerprint, GoalBytes, Intent, IntentId, ObjectSnapshot, PlannedDeltaBytes, Subject,
    SubstrateKind, Timestamp, Transformation, TransformationId, VerdictKind,
};
use gx_core::{FingerprintBytes, InclusionProof, VerdictCheckpoint, VerdictTally};
use gx_gate::{AdmitProof, EscalationTicket, Gate, GateInput, Reason, TicketId, Verdict};
use gx_log::{proof::prove_inclusion, store::VerdictCheckpointStore, LedgerStore};
use gx_substrate::{AppliedDelta, PlannedDelta, SubstrateAdapter};
use gx_witness::{
    Environment, Evidence, KeyPair, Provenance, ProvenanceInputs, Receipt, ReceiptKind,
    ReceiptPayload, VerdictSummary,
};

use crate::replay::{reconstruct, CommittedRow, DraftRow, EscrowRow, Sigma, StateRow};
use crate::store::{
    BlobStore, EngineJournal, EngineJournalRecord, FingerprintRecord, InverseStatus, Rollback,
    SupersedeIndex,
};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// 43 §1 -- the eleven states
// ---------------------------------------------------------------------------

/// Where a transformation is in 43 §1's lifecycle.
///
/// Eleven values, and the enum is the whole vocabulary rather than the part this hand reaches:
/// `Committing`, `Committed` and `Superseded` are named here and written by hands 4 and 6. Naming
/// them now is what lets [`LIFECYCLE_STATES`] be compared against 43 §1's table today, which is the
/// check that would otherwise arrive after the states it was supposed to constrain.
///
/// `Draft` is in the vocabulary and **never in the table**. 43 §1 lists it and 42 §1.3-3 keys the
/// table on `TransformationId`, which a draft does not have (**M5-17 採(b)**); the two facts sit
/// beside each other rather than being reconciled by dropping one, because dropping `Draft` would
/// make the enum disagree with 43 §1 and adding a draft key would make the table disagree with 42.
///
/// `Aborted` carries its reason, which is 43 §1's 「`AbortReason`を必ず伴う」 in the type: there is no
/// way to spell an abort without saying why.
/// `Serialize` because Σ holds one (**E-M5-2**): AC-039 compares the canonical bytes of the state
/// table, and a state written as a `String` beside a separate `Option<AbortReason>` could spell
/// 「aborted, for no reason」. The enum carries the reason where 43 §1 puts it, so the encoding
/// cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Lifecycle {
    /// T-1 has run. Journal only -- see the type documentation.
    Draft,
    /// T-2 has run: `PlannedDelta`, `Fingerprint₀` and `TransformationId` are fixed.
    Candidate,
    /// T-3 has run: evidence is being collected and the gate is being asked.
    Verifying,
    /// T-4a, T-4e, or hand 6's T-5.
    Admitted,
    /// T-4b, or hand 6's T-5b. Terminal unless `EnforcementMode::RecordOnly` opens T-8r.
    Denied,
    /// T-4c. Hand 6 resolves it.
    Escalated,
    /// T-8 or T-8r has run.
    Canonicalized,
    /// Hand 4's critical section (T-9 onward).
    Committing,
    /// Hand 4's terminal (T-11).
    Committed,
    /// Terminal, with the reason gx-core defines (ASM-15).
    Aborted(AbortReason),
    /// Hand 6's terminal (T-12).
    Superseded,
}

/// The eleven state names, declared once, in 43 §1's order.
///
/// The **E-M2-23 / A-10** shape this workspace uses everywhere: one declared list, one `name()`
/// written without a `_` arm, and `tests/lifecycle_states.rs` reading 43 §1's table out of the spec
/// file to compare against both. A twelfth state added without a row is a compile error at
/// [`Lifecycle::name`] and a failing probe at the table.
pub const LIFECYCLE_STATES: [&str; 11] = [
    "Draft",
    "Candidate",
    "Verifying",
    "Admitted",
    "Denied",
    "Escalated",
    "Canonicalized",
    "Committing",
    "Committed",
    "Aborted",
    "Superseded",
];

impl Lifecycle {
    /// Which of [`LIFECYCLE_STATES`] this is. No `_` arm.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Lifecycle::Draft => "Draft",
            Lifecycle::Candidate => "Candidate",
            Lifecycle::Verifying => "Verifying",
            Lifecycle::Admitted => "Admitted",
            Lifecycle::Denied => "Denied",
            Lifecycle::Escalated => "Escalated",
            Lifecycle::Canonicalized => "Canonicalized",
            Lifecycle::Committing => "Committing",
            Lifecycle::Committed => "Committed",
            Lifecycle::Aborted(_) => "Aborted",
            Lifecycle::Superseded => "Superseded",
        }
    }

    /// Whether 43 §1 marks this state terminal.
    ///
    /// `Denied` is 「**終端**（ただしrecord-onlyモード時のみ§3の例外分岐でCanonicalizedへ進む）」, so
    /// the answer depends on a setting and the caller that knows the setting asks the question.
    /// [`Engine::canonicalize`] is that caller.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Lifecycle::Aborted(_) | Lifecycle::Committed | Lifecycle::Superseded
        )
    }
}

// ---------------------------------------------------------------------------
// 43 T-6 / ASM-12 -- the two deadlines
// ---------------------------------------------------------------------------

/// ASM-12's `verify_ttl`, in nanoseconds: **24 hours** (33 NFR-028).
///
/// [`Timestamp`] is an `i64` of nanoseconds, so the default is written as one rather than as a
/// `Duration` a caller would have to convert. 43 T-6 measures it from the moment a transformation
/// entered `Candidate` or `Verifying`; [`Engine::with_ttl`] is how a test asks for AC-045's 100 ms.
pub const DEFAULT_VERIFY_TTL_NANOS: i64 = 24 * 60 * 60 * 1_000_000_000;

/// ASM-12's `escalation_ttl`, in nanoseconds: **72 hours** (33 NFR-028).
///
/// Longer than [`DEFAULT_VERIFY_TTL_NANOS`] because the thing being waited for is a person. INV-L2
/// is what makes it finite at all: 「任意の`Escalated`は有限時間内に…到達する（無期限保留なし）」.
pub const DEFAULT_ESCALATION_TTL_NANOS: i64 = 72 * 60 * 60 * 1_000_000_000;

// ---------------------------------------------------------------------------
// 43 T-5 / T-5b -- what a person decided
// ---------------------------------------------------------------------------

/// A human ruling on an escalated transformation (43 T-5 / T-5b, DR-11, 44 §1.2's `--reason`).
///
/// # Three fields, because AC-071 asks for three
///
/// > 発行されるreceipt trail（journal/Receiptメタデータ）に`Evidence(HumanDecision)`
/// > （decision=Admit, reason, 裁定者actor）が含まれることを確認する
///
/// **E-M2-3** retired the `Evidence` variant that sentence names — 「43 T-5 の「人間裁定receipt
/// （署名済み）」 は receipt」 — so the three facts live in the journal's `HumanDecision` record and
/// in the [`ReceiptKind::VerdictReceipt`] the transition issues. This struct is what the caller
/// hands in, and what its **digest** is: see [`Engine::escalation`] for why a human ruling needs
/// one at all.
///
/// # It is a value with an identity, and that is what makes the receipt honest
///
/// 42 §3.10's `VerdictSummary.proof_digest` is 「`Verdict`全体ではなくそのCID化digest」, and after a
/// human ruling there is no `Verdict` to digest: the gate answered `Escalate` and a person answered
/// something else. Carrying the *ticket's* digest under an `Admit` would say the gate admitted it;
/// minting an empty one is what §32 M4H4-2 refused twice. So the digest is of **this value** —
/// decision, reason, ruler — and nothing else, which is the one thing that is true.
///
/// `at` is not in it, so the digest is clock-free (CM-5: 「signed payload から clock read 排除」)
/// and two identical rulings made a day apart summarise identically. Raised as **M5H6-3**: 42
/// §3.10 gives no rule for this digest and req/49 §3 M2-10 left the `Deny`/`Escalate` rules open in
/// the same way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HumanRuling {
    /// 43 T-5 is `Admit` and T-5b is `Deny`. 42 §3.13: 「kindはAdmit|Denyのみ」.
    pub decision: VerdictKind,
    /// 44 §1.2's `--reason <text>`. Non-empty; see [`Engine::escalation`].
    pub reason: String,
    /// Who ruled — **not** the submitter. `Transformation.actor` is who asked.
    pub actor: Actor,
}

impl IdentityView for HumanRuling {
    type View<'a> = &'a HumanRuling;

    fn identity_view(&self) -> &HumanRuling {
        self
    }
}

// ---------------------------------------------------------------------------
// M5-03 採(a) -- the evidence entry point
// ---------------------------------------------------------------------------

/// Where the evidence a gate decides on comes from (**M5-03 採(a)**).
///
/// 32 FR-032 writes 「evidence収集後 `Gate::verify` を呼び出し」 and 43 T-3 writes 「evidence collector
/// 起動」, and neither 41 §2's crate table nor 42 §3.7 says what a collector *is* -- 42 defines the
/// `Evidence` type and no producer of it. req/78 §4's M5-03 measured the consequence (the
/// constructors of `Evidence` have zero shipping callers) and req/38 §37 rules the shape:
///
/// > **M5-03 採(a)**: `EvidenceSource` trait 1 本を gx-engine に置く(41 §2 へ A-6 追記)。`Err` が
/// > `VerifierUnavailable` の唯一の producer。44 の `--evidence` 注入形(外部収集物の投入)と整合する
/// > 事を確認済。
///
/// # The `Err` is the point
///
/// 43 T-4d and T-4e both fire on 「verifier/evidence collector到達不能」, and until this trait existed
/// **there was no way to be unreachable**: `Gate::verify` is a function call in the same process.
/// AC-036 asks for 「gx-gateプロセスを`kill -9`」, which names a process that does not exist, and
/// **E-M5-4** (M5-19 採(a)) reads it as 「evidence collector が到達不能」 instead. This trait is what
/// makes that reading implementable, so its failure is the workspace's **only** road to
/// `AbortReason::VerifierUnavailable` -- measured by
/// `tests/engine_shape.rs::verifier_unavailable_has_exactly_one_producer` and, from the behaviour
/// side, by `tests/ac_032.rs`.
///
/// Every `Err` is unreachability, not only [`Error::EvidenceUnavailable`]: a collector that failed
/// did not collect, and the engine has no way to tell 「I could not reach it」 from 「it could not
/// answer」 that would not be the collector's own claim about itself.
///
/// # Two implementations, and why those two
///
/// [`InjectedEvidence`] is 44 §1.2's `--evidence` in library form -- 「事前収集済み`Evidence`（42
/// §3.7）をJSONLで追加投入（テスト結果等の外部収集ツール連携用）」 -- and its empty case is 44's
/// 「省略時はgx-gate組込のInvariantCheck/Cedar評価のみ」. [`UnreachableEvidence`] is the other side
/// of the `Result`, and a v0.1 that shipped only the first would be a v0.1 in which T-4d is
/// unreachable code.
pub trait EvidenceSource {
    /// Collect what the gate should decide on.
    ///
    /// # Errors
    /// Anything the collector cannot do. Every one of them reaches
    /// `AbortReason::VerifierUnavailable` under `FailPosture::FailClosed`, or T-4e's degraded
    /// admission under `FailPosture::FailOpen`.
    fn collect(&self, t: &Transformation, pre: &ObjectSnapshot) -> Result<Vec<Evidence>>;
}

/// Evidence handed in from outside, which is 44 §1.2's `--evidence` (**M5-03 採(a)**).
#[derive(Clone, Debug, Default)]
pub struct InjectedEvidence {
    evidence: Vec<Evidence>,
}

impl InjectedEvidence {
    /// A source that answers with these items.
    #[must_use]
    pub fn new(evidence: Vec<Evidence>) -> Self {
        Self { evidence }
    }

    /// A source that answers with nothing, successfully.
    ///
    /// 44 §1.2's 「省略時はgx-gate組込のInvariantCheck/Cedar評価のみ」. **Not the same as a source
    /// that could not be reached** -- this one succeeds and the answer is the empty list, which is
    /// req/29 §4's rule (「skip と pass を同じ顔にしない」) at the one place a v0.1 would be most
    /// tempted to blur it.
    #[must_use]
    pub fn none() -> Self {
        Self {
            evidence: Vec::new(),
        }
    }
}

impl EvidenceSource for InjectedEvidence {
    fn collect(&self, _t: &Transformation, _pre: &ObjectSnapshot) -> Result<Vec<Evidence>> {
        Ok(self.evidence.clone())
    }
}

/// A source that cannot be reached — 43 T-4d and T-4e's precondition, as a value.
///
/// It carries what a deployment would have said, because 「the collector is down」 and 「the
/// collector rejected our credentials」 are different operational facts and the engine records the
/// distinction it was told rather than inventing one.
#[derive(Clone, Debug)]
pub struct UnreachableEvidence {
    detail: String,
}

impl UnreachableEvidence {
    /// A source that always refuses, saying this.
    #[must_use]
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl EvidenceSource for UnreachableEvidence {
    fn collect(&self, _t: &Transformation, _pre: &ObjectSnapshot) -> Result<Vec<Evidence>> {
        Err(Error::EvidenceUnavailable {
            detail: self.detail.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// AC-033 -- canon, and the one thing that may be replaced about it
// ---------------------------------------------------------------------------

/// `canon(T)`: the bytes 43 T-8 checks T3 over.
///
/// # Why this is a trait, and how 41 §6 survives it
///
/// 41 §6 is unambiguous: 「全canonical encodeはgx-canon経由のみ（迂回禁止）」. A replaceable encoder
/// looks exactly like the second road that sentence forbids. AC-033 is equally unambiguous the other
/// way: 「冪等性違反を返す壊れたcanon実装を注入した異常系ではエラーを返しCanonicalizedへ遷移しない」,
/// which cannot be written unless something about canon can be broken on purpose.
///
/// Both hold because the two roads carry different things. **The identity is never injected**:
/// [`Engine::canonicalize`] takes the `canonical_cid` it journals from `gx_canon::cid::compute`, in
/// one place, whatever this trait says. What is injected is the *evidence for T-8's guard* -- the
/// bytes the idempotence check runs over. A broken canonicalizer can therefore make the engine
/// refuse to canonicalize; it cannot make the engine mint a CID that gx-canon did not compute.
///
/// The check itself is `gx_canon::cbor::is_canonical`, which is 42 §2.3's criterion
/// (`encode_canonical(decode(encode_canonical(x))) == encode_canonical(x)`) in the form gx-canon
/// already publishes and `gx-canon/tests/ac_012.rs` already measures. Asking gx-canon what it would
/// have written is a stronger question than re-running an encoder against itself, and it is the one
/// T-21 says is worth asking: an encoder that normalises everything satisfies idempotence
/// vacuously, and `is_canonical` is the function that refuses the spellings it would not have
/// written.
///
/// Raised as **M5H2-4**: the tension is real even though it resolves, and a later hand tempted to
/// widen this trait into 「the engine's encoder」 should meet the sentence rather than the shape.
pub trait Canonicalizer {
    /// The canonical bytes of the transformation's `IdentityView` (42 §1.1, §2.1).
    ///
    /// # Errors
    /// Whatever the encoder refuses. gx-canon refuses a value with no canonical form.
    fn canonical_form(&self, t: &Transformation) -> Result<Vec<u8>>;
}

/// The only shipping [`Canonicalizer`]: gx-canon, and nothing else (41 §6).
#[derive(Clone, Copy, Debug, Default)]
pub struct CanonEncoder;

impl Canonicalizer for CanonEncoder {
    fn canonical_form(&self, t: &Transformation) -> Result<Vec<u8>> {
        Ok(cbor::encode(&t.identity_view())?)
    }
}

// ---------------------------------------------------------------------------
// The table's rows
// ---------------------------------------------------------------------------

/// One row of the engine store: everything T-2 fixed, plus what has happened since.
///
/// The bodies (`transformation`, `delta`, `pre`) are here because the transitions need them; the
/// **names** are what Σ is made of ([`Engine::sigma`]). `verdict_digest` is held rather than
/// recomputed for the reason hand 4 will need it: 42 §3.10's `ReceiptPayload` carries it, and a
/// digest recomputed at commit time from a verdict rebuilt at commit time would be a second answer
/// to a question the journal already recorded.
#[derive(Clone, Debug)]
struct Entry {
    intent_id: IntentId,
    transformation: Transformation,
    state: Lifecycle,
    /// 🔴 **T-6**: when this row entered `state`, which is what the two TTLs are measured from.
    ///
    /// Not in Σ, and that is deliberate rather than an omission: it is a **function of the
    /// journal** — the `at` of the record that fixed the current state — so a reconstruction can
    /// recompute it and AC-039's comparison would be measuring the same bytes twice. What Σ holds
    /// is the state; what this holds is when the clock 41 §6 injected said it was reached.
    since: Timestamp,
    /// 43 §8's 「`blocked_by: TransformationId`という内部注釈のみ」 (hand 6).
    ///
    /// Not in Σ either, and for a *different* reason: no journal record carries it. 43 §8 is
    /// explicit that waiting adds 「新たな状態は追加しない」, so a blocked transformation is a
    /// `Candidate` that has not been allowed to start verifying, and the annotation is a fact
    /// about the live in-flight set rather than about the log. A restart loses it, which is
    /// correct — the in-flight table is empty after one (M5H3-5).
    blocked_by: Option<TransformationId>,
    delta: PlannedDelta,
    fp0: Fingerprint,
    pre: ObjectSnapshot,
    verdict: Option<VerdictKind>,
    verdict_digest: Option<Cid>,
    enforced: bool,
    fail_posture_engaged: bool,
    canonical_cid: Option<Cid>,
    /// The ticket T-4c raised, with 41 §6's clock in it (**E-5**) and its id re-minted (**E-6**).
    ticket: Option<EscalationTicket>,
    /// 🔴 **M6H3-2 採(a)** — T-4a's `AdmitProof`, kept so that something can be *asked* for it.
    ///
    /// The third seat beside [`Entry::ticket`], and it exists for the reason req/38 §50 gives:
    /// 44 §1.2's stdout for `gx verify` is `{"kind":"Admit","proof":AdmitProof}` and 44 §2.3's
    /// problem `detail` is 「詳細説明」, and until this field existed an operator asking 「why」 got a
    /// **digest** — a value that proves the proof was the one hashed and says nothing about what it
    /// contained. M6H3-2's ruling is 「表の読み・Σ 影響なし」 and that is exactly the shape: the
    /// journal records `verdict_digest` and not this, so nothing about Σ moves and a row rebuilt
    /// from the journal answers `None` here, the way it already answers `None` for
    /// [`Entry::verdict_receipts`].
    admit_proof: Option<AdmitProof>,
    /// 🔴 **M6H3-2 採(a)** — T-4b's reasons, for [`Entry::admit_proof`]'s reason one verdict along.
    ///
    /// 44 §1.2: `{"kind":"Deny","reasons":[Reason]}`. Separate from the proof rather than one
    /// `Option<Verdict>` because [`Entry::ticket`] already took the third variant's seat in M5, and
    /// a field that held a whole `Verdict` would carry the ticket twice.
    deny_reasons: Option<Vec<Reason>>,
    /// ASM-14's first kind, in order of issue: T-4a/b/c or T-4e, then T-5/T-5b (**M5H4-6**).
    ///
    /// A `Vec` because 43 T-5's side effect is 「人間裁定receipt（署名済み）を **provenance鎖に追記**」
    /// — an escalated transformation ends with two, signed by two different keys, and a field that
    /// held one would have to choose which of them to forget.
    verdict_receipts: Vec<Receipt>,
    /// T-12's edge, from `T_o`'s side. Written once, by the commit of the transformation that
    /// carried this one's escrowed inverse.
    superseded_by: Option<TransformationId>,
    // Hand 4. Everything below is written inside the commit critical section, and every one of them
    // is also a component of Σ -- the row and the reconstruction have to agree (AC-039), so a field
    // added here without an arm in `crate::replay::reconstruct` breaks a probe rather than drifting.
    apply_started: Option<Cid>,
    rollback: Option<Rollback>,
    provenance: Option<Provenance>,
    /// The escrowed inverse's CID (T-10b), which 42 §3.10 puts on the receipt.
    inverse_cid: Option<Cid>,
    /// 🔴 **E-M4-31 / M5-18 採(a)**: the moment the engine says the apply happened, not the one the
    /// adapter returned. `Timestamp(0)` reaching this field is the bug the ruling names.
    applied_at: Option<Timestamp>,
    /// The receipt T-11 issued. Held in memory: 44's `gx receipt show` reads a store, and that
    /// store is M6's (req/78 N-01).
    receipt: Option<Receipt>,
}

/// An adapter and the version the deployment registered it under (**M5-07 採(a)**).
///
/// 42 §3.9's `Environment.adapter_version` is a `String` and 41 §4's trait has **seven methods,
/// none of which reports a version**. N-07 forbids an eighth, so the value comes from the only
/// party that has it: whoever calls [`Engine::register_adapter`]. Raised as **M5H4-4**.
struct Registered {
    adapter: Arc<dyn SubstrateAdapter>,
    version: String,
}

// ---------------------------------------------------------------------------
// 43 §7 -- what a recovery did, per transformation
// ---------------------------------------------------------------------------

/// Which of 43 §7's roads [`Engine::recover`] walked for one transformation.
///
/// Four values for a section 43 writes as three steps, and the fourth is **E-M5-1**'s: §7-3 splits
/// on 「ledgerに該当entryが存在する」 alone, and the ruling adds a second question — 「was the
/// adapter asked」 — whose answer decides whether the CAS may be re-run at all.
///
/// | value | 43 | what recovery found |
/// |---|---|---|
/// | [`RecoveryPath::Terminal`] | §7-2 | the last record is terminal: rebuild, re-run nothing |
/// | [`RecoveryPath::LedgerHeldTheCommit`] | §7-3b | a ledger entry exists: the commit completed |
/// | [`RecoveryPath::ApplyWasAnnounced`] | §7-3c + **E-M5-1** | an `ApplyStarted` exists and no ledger entry: re-apply, **do not re-run the CAS** |
/// | [`RecoveryPath::NothingWasApplied`] | §7-3c, refused | no `ApplyStarted` and no ledger entry: the world did not move, and the journal does not carry what re-running T-10a needs (see [`Engine::recover`]) |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RecoveryPath {
    /// 43 §7-2: 「その状態を正としてメモリ上に再構築するのみ（副作用の再実行なし）」.
    Terminal,
    /// 43 §7-3b: 「ledgerに該当entryが存在する場合 → commitはクラッシュ前に完了していた」.
    LedgerHeldTheCommit,
    /// 43 §7-3c as **E-M5-1** rewrites it: the apply was announced, so the CAS is not re-run.
    ApplyWasAnnounced,
    /// 43 §7-3c's re-run, refused because its inputs are not in the journal.
    NothingWasApplied,
}

/// The names of [`RecoveryPath`], declared once (**E-M2-23 / A-10**).
pub const RECOVERY_PATHS: [&str; 4] = [
    "Terminal",
    "LedgerHeldTheCommit",
    "ApplyWasAnnounced",
    "NothingWasApplied",
];

impl RecoveryPath {
    /// Which of [`RECOVERY_PATHS`] this is. No `_` arm, for [`crate::Error::kind`]'s reason.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            RecoveryPath::Terminal => "Terminal",
            RecoveryPath::LedgerHeldTheCommit => "LedgerHeldTheCommit",
            RecoveryPath::ApplyWasAnnounced => "ApplyWasAnnounced",
            RecoveryPath::NothingWasApplied => "NothingWasApplied",
        }
    }
}

/// What [`Engine::recover`] did about one transformation.
///
/// A value rather than a log line: AC-043 asks for 「同一TransformationIdについて`ledger`entryが
/// 高々1件」 over thirty runs, and a probe that had to parse prose to count them would be measuring
/// the prose.
///
/// `receipt` is returned rather than filed in the engine's table because recovery does not rebuild
/// that table — see [`Engine::recover`] for what it does and does not need.
#[derive(Clone, Debug)]
pub struct Recovered {
    /// The transformation this is about.
    pub transformation: TransformationId,
    /// Which road 43 §7 sent the recovery down.
    pub path: RecoveryPath,
    /// Where it left the transformation.
    pub state: Lifecycle,
    /// The ledger sequence number, where the transformation reached one.
    pub ledger_seq: Option<u64>,
    /// `Some("Appended")` or `Some("AlreadyPresent")` when this recovery appended; `None` when it
    /// found the entry already there (§7-3b) or never reached the ledger.
    pub appended: Option<&'static str>,
    /// 🔴 **M5H4-7**, mechanically: `Some(true)` when the payload rebuilt from the journal digests
    /// to exactly what the ledger already holds for this transformation. `None` when there was
    /// nothing to compare against.
    ///
    /// This is what makes 「冪等な再構成であって二重commitではない」 (43 §7-3b) a measurement: a
    /// re-issued receipt whose payload had drifted would hash differently, and the ledger — whose
    /// key idempotency refuses a second digest under one key (ASM-43-1) — would be the one refusing.
    pub payload_matched: Option<bool>,
    /// The receipt this recovery issued, where it issued one (43 §7-3b's 「未発行なら再発行」).
    pub receipt: Option<Receipt>,
}

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// 🔴 **FR-M04**: rebuild the verdict counter from the journal, for [`Engine::open`].
///
/// The rule is 43's transition table and not this crate's call graph: three rows issue a
/// `VerdictReceipt`, and this fold names all three.
///
/// * T-4a / T-4b / T-4c — `Verdict { verdict_digest: Some(_) }`, one per gate answer.
/// * T-4e — `Verdict { kind: Admit, verdict_digest: None }`. No gate ran, so the fourth bucket.
/// * T-5 / T-5b — `HumanDecision`, where a person answered.
///
/// 🔴 **This function is why the recount in `tests/ac_vc.rs` is only independent inside a session,
/// and the test file says so in its own header.** Within a process the counter is incremented at
/// the receipt and the test folds the journal, which are two roads; across a restart both start
/// here. Stating it beats a claim of independence that a restart quietly retires.
fn tally_from_the_journal(records: &[EngineJournalRecord]) -> VerdictTally {
    let mut tally = VerdictTally::default();
    for record in records {
        let kind = match record {
            EngineJournalRecord::Verdict {
                kind,
                verdict_digest: Some(_),
                ..
            }
            | EngineJournalRecord::HumanDecision { kind, .. } => Some(*kind),
            EngineJournalRecord::Verdict {
                verdict_digest: None,
                ..
            } => None,
            _ => continue,
        };
        match kind {
            Some(VerdictKind::Admit) => tally.admit += 1,
            Some(VerdictKind::Deny) => tally.deny += 1,
            Some(VerdictKind::Escalate) => tally.escalate += 1,
            None => tally.unverdicted += 1,
        }
    }
    tally
}

/// 43's state machine, running.
///
/// Generic over the two things a test replaces and holding a registry of the third. See the module
/// documentation for why the clock and the seed are arguments instead of type parameters.
pub struct Engine<E: EvidenceSource, C: Canonicalizer = CanonEncoder> {
    journal: EngineJournal,
    blobs: BlobStore,
    /// 🔴 The public witness ledger of 42 §3.11, wired in hand 4. A **different file** from the
    /// journal and with a different audience (42 §3.13: 「Ledgerはcommit確定後の公開witness台帳、
    /// Journalはengine内部の進行中パイプライン記録」).
    ledger: LedgerStore,
    /// 🔴 **FR-M04** (M7 hand 6): the parallel append-only log of signed verdict counts, a third
    /// file beside the journal and the ledger. `gx_log::store::VerdictCheckpointStore` carries
    /// why it is a file of its own.
    verdict_log: VerdictCheckpointStore,
    /// 🔴 **FR-M04**: how many verdicts of each kind this deployment has issued **since the log
    /// began**, and how far the last published checkpoint reached.
    ///
    /// Incremented where the receipt is *issued*, and rebuilt at `open` from the journal. Those
    /// are not the same road, and the difference is the whole reason `ac_vc.rs` can recount from
    /// the journal and call the comparison independent inside a session — see that file's header
    /// for where the independence stops.
    verdicts: VerdictTally,
    /// The window boundary: the counts the published chain has already spoken for. A window's
    /// tally is this subtracted from [`Engine::verdicts`], which is why it is a tally rather than
    /// a single number — a checkpoint states four counts, not one.
    published: VerdictTally,
    adapters: BTreeMap<SubstrateKind, Registered>,
    gate: Gate,
    evidence: E,
    canon: C,
    mode: EnforcementMode,
    posture: FailPosture,
    /// The drafts, **with their seeds** (42 §3.13: 「replay時に同一シードで再実行」). A set until hand
    /// 3, when Σ made the seed part of what a replay has to reproduce.
    drafted: BTreeMap<IntentId, u64>,
    table: BTreeMap<TransformationId, Entry>,
    /// 🔴 **M6-02 採(a)**: 44 §0's id-resolution, inverted. See [`Engine::resolved`].
    ///
    /// Derived from the journal's `Planned` records and from nothing else, which is why it is a
    /// cache rather than a fifth component of Σ (req/88 Λ1): [`Engine::open`] rebuilds it by
    /// replaying them in order, so a restart cannot make it disagree with the journal, and
    /// [`Engine::sigma`] does not report it.
    resolved: BTreeMap<IntentId, TransformationId>,
    /// 🔴 **M6-07 採(b)** — [`Engine::table`](Engine) keyed the other way: which rows are about which
    /// subject.
    ///
    /// # Why it exists, with the measurement rather than the reasoning
    ///
    /// 43 §8's conflict check asks 「is there an in-flight transformation of **this** subject that
    /// does not commute with mine」, and until this hand [`Engine::conflicting_predecessor`] answered
    /// it by walking every row in `table` and discarding the ones whose subject did not match. The
    /// discard is cheap and the walk is `O(n)`, and `n` is 「every transformation this process has
    /// ever seen」 — which for a single-shot CLI is a handful and for `gx serve` is the whole day.
    ///
    /// M5 hand 8 identified that as the shape of AC-066's decay (n-ratio 4.95x against a measured
    /// verify-cost ratio of 5.08x, a 3% agreement), §45 M5H8-16 registered the index against the
    /// firing condition 「長寿命 engine が表を実際に育てる時」, and §47 M6-07 採(b) fixed the order:
    /// measure through `gx serve` **first**, then index, then measure again. `req/95` carries both
    /// halves; hand 6 deliberately left the engine unindexed so that this hand had a control.
    ///
    /// # What it is not
    ///
    /// **Not part of Σ.** Same standing as `resolved` above: a second reading of the state table,
    /// derived from it, living and dying with it. [`Engine::open`] leaves the table empty (M5H3-5)
    /// and therefore leaves this empty, [`Engine::sigma`] does not report it, and AC-039's
    /// live-vs-replayed comparison is unmoved.
    ///
    /// 🔴 A `BTreeSet` per subject rather than one id: the case the index exists for is **two**
    /// transformations of one object, so a map that held one id per subject would be wrong exactly
    /// where it matters and right everywhere a benchmark looks.
    /// `crates/gx-engine/tests/subject_index.rs` compares this against a full scan.
    by_subject: BTreeMap<Subject, BTreeSet<TransformationId>>,
    /// 43 T-6's two deadlines, in nanoseconds. See [`Engine::with_ttl`].
    verify_ttl: i64,
    escalation_ttl: i64,
    /// Σ's escrow component, live (42 §3.12). Written at T-10b; T-12 moves the status.
    escrow: BTreeMap<TransformationId, EscrowRow>,
    /// 🔴 **M5-09 採(a)**: ASM-43-2's `superseded_by`, in the type `store.rs` declares for it.
    supersedes: SupersedeIndex,
    /// Σ's ledger component, live: which transformation reached `Committed` at which leaf.
    ///
    /// 🔴 **M5H3-4**: this is 「journal-witnessed frontier」 and *not* the ledger's own root, and
    /// hand 4 is where the difference stops being a definition. [`Engine::ledger_agrees`] is the
    /// probe-facing form of the agreement.
    committed: BTreeMap<TransformationId, u64>,
}

/// Written by hand rather than derived, because `Arc<dyn SubstrateAdapter>` has no `Debug` (41 §4
/// asks the trait for seven methods and no formatting) and because a derived one would print two
/// collaborators' internals in place of the thing an operator wants: what is registered, how many
/// transformations are in flight, and which of DR-2's two axes this deployment set.
impl<E: EvidenceSource, C: Canonicalizer> std::fmt::Debug for Engine<E, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("journal", &self.journal.path())
            .field("records", &self.journal.len())
            .field("blobs", &self.blobs.root())
            .field("ledger", &self.ledger.path())
            .field("leaves", &self.ledger.log().len())
            .field("adapters", &self.adapters.keys().collect::<Vec<_>>())
            .field("mode", &self.mode)
            .field("posture", &self.posture)
            .field("verify_ttl", &self.verify_ttl)
            .field("escalation_ttl", &self.escalation_ttl)
            .field("drafts", &self.drafted.len())
            .field("transformations", &self.table.len())
            .field("superseded", &self.supersedes.len())
            .finish()
    }
}

impl<E: EvidenceSource> Engine<E, CanonEncoder> {
    /// Open the journal at `path` and rebuild what it holds.
    ///
    /// 43 §7-1's 「起動時 replay」. This rebuilds the **draft phase** from the journal -- every
    /// `IntentId` a `DraftCreated` names, with the seed it was submitted under -- and opens the blob
    /// store beside it.
    ///
    /// It does **not** rebuild the in-flight table, and hand 3 does not change that: a row holds a
    /// `Transformation` and an `ObjectSnapshot`, and the journal holds names and digests rather than
    /// bodies (ASM-9). What hand 3 adds is that the *state* those rows would carry is recoverable
    /// without them -- [`Engine::sigma`] and [`crate::replay::reconstruct`] agree on Σ, which is what
    /// **E-M5-2** defines AC-039's 「結果状態」 to be. Resuming an in-flight commit after a restart is
    /// hand 5's, and 43 §7-3c's inputs are exactly the two things that survive: `Fingerprint₀` from
    /// the journal and the delta body from the blob store.
    ///
    /// # Where the blobs and the ledger live
    ///
    /// `<journal>.blobs` and `<journal>.ledger`, beside the journal. Derived rather than taken as
    /// arguments because the three files are one engine's state: a caller who could point them at
    /// different directories could open a journal against another engine's bodies, and every
    /// `delta_cid` in it would resolve to something that was never planned. The ledger is a
    /// separate **file** and not a separate idea of the same thing (42 §3.13).
    ///
    /// 🔴 Hand 4 wires the ledger and **does not** rebuild the in-flight table from it. The ledger
    /// replays itself (`gx_log::LedgerStore::open`), so the public log survives a restart; Σ's view
    /// of it does not, because that view lives in a table `open` still leaves empty (M5H3-5, hand
    /// 5's window). Reporting a rebuilt ledger component beside an empty state table would be a
    /// partly-rebuilt Σ presented as a whole one.
    ///
    /// # Errors
    /// [`Error::Io`] if the journal cannot be opened, read, truncated or synced, or if the blob
    /// directory cannot be created. [`Error::Ledger`] if the ledger cannot be opened or replayed.
    pub fn open(path: impl AsRef<std::path::Path>, gate: Gate, evidence: E) -> Result<Self> {
        let path = path.as_ref();
        let mut blobs_root = path.as_os_str().to_os_string();
        blobs_root.push(".blobs");
        let blobs = BlobStore::open(std::path::PathBuf::from(blobs_root))?;
        let mut ledger_path = path.as_os_str().to_os_string();
        ledger_path.push(".ledger");
        let ledger = LedgerStore::open(std::path::PathBuf::from(ledger_path)).map_err(|e| {
            Error::Ledger {
                action: "open",
                detail: e.to_string(),
            }
        })?;
        let mut verdict_path = path.as_os_str().to_os_string();
        verdict_path.push(".verdicts");
        let verdict_log = VerdictCheckpointStore::open(std::path::PathBuf::from(verdict_path))
            .map_err(|e| Error::Ledger {
                action: "open the verdict checkpoint log",
                detail: e.to_string(),
            })?;
        let journal = EngineJournal::open(path)?;
        // 🔴 **FR-M04**: the counter is rebuilt from the journal, the same way `drafted` and
        // `resolved` are, and for the same reason — `open` leaves the table empty, so a counter
        // that lived only in the table would reopen window zero after every restart and publish a
        // chain full of holes. What the rebuild costs is written in `ac_vc.rs`'s header: across a
        // restart the producer and the recount share a source.
        let verdicts = tally_from_the_journal(journal.records());
        // Folded from the chain rather than read off its last entry: the last entry carries its
        // own window, not the sum of the ones before it.
        let published = verdict_log
            .checkpoints()
            .iter()
            .fold(VerdictTally::default(), |acc, c| VerdictTally {
                deny: acc.deny + c.tally.deny,
                admit: acc.admit + c.tally.admit,
                escalate: acc.escalate + c.tally.escalate,
                unverdicted: acc.unverdicted + c.tally.unverdicted,
            });
        let drafted = journal
            .records()
            .iter()
            .filter_map(|r| match r {
                EngineJournalRecord::DraftCreated {
                    intent_id,
                    rng_seed,
                    ..
                } => Some((*intent_id, *rng_seed)),
                _ => None,
            })
            .collect();
        // 🔴 **M6-02 採(a)**, rebuilt the same way `drafted` is: from the journal, in append order,
        // last write winning. That order is the rule req/88 Λ3(ii) asks for — see
        // [`Engine::resolved`] — and taking it from the journal rather than from the table is what
        // makes the answer survive a restart, since `open` deliberately leaves the table empty.
        let resolved = journal
            .records()
            .iter()
            .filter_map(|r| match r {
                EngineJournalRecord::Planned {
                    intent_id,
                    transformation,
                    ..
                } => Some((*intent_id, *transformation)),
                _ => None,
            })
            .collect();
        Ok(Self {
            journal,
            blobs,
            ledger,
            verdict_log,
            verdicts,
            published,
            adapters: BTreeMap::new(),
            gate,
            evidence,
            canon: CanonEncoder,
            mode: EnforcementMode::default(),
            posture: FailPosture::default(),
            verify_ttl: DEFAULT_VERIFY_TTL_NANOS,
            escalation_ttl: DEFAULT_ESCALATION_TTL_NANOS,
            drafted,
            resolved,
            table: BTreeMap::new(),
            by_subject: BTreeMap::new(),
            escrow: BTreeMap::new(),
            supersedes: SupersedeIndex::new(),
            committed: BTreeMap::new(),
        })
    }
}

impl<E: EvidenceSource, C: Canonicalizer> Engine<E, C> {
    /// Replace the canonicalizer (AC-033's abnormal case; see [`Canonicalizer`]).
    #[must_use]
    pub fn with_canonicalizer<C2: Canonicalizer>(self, canon: C2) -> Engine<E, C2> {
        Engine {
            journal: self.journal,
            blobs: self.blobs,
            ledger: self.ledger,
            verdict_log: self.verdict_log,
            verdicts: self.verdicts,
            published: self.published,
            adapters: self.adapters,
            gate: self.gate,
            evidence: self.evidence,
            canon,
            mode: self.mode,
            posture: self.posture,
            verify_ttl: self.verify_ttl,
            escalation_ttl: self.escalation_ttl,
            drafted: self.drafted,
            resolved: self.resolved,
            table: self.table,
            by_subject: self.by_subject,
            escrow: self.escrow,
            supersedes: self.supersedes,
            committed: self.committed,
        }
    }

    /// Register an adapter for a substrate (**M5-07 採(a)**).
    ///
    /// The registry is the whole of N-13 as a design rather than as a rule: gx-engine declares no
    /// adapter dependency, so the only way a substrate reaches this engine is a caller putting one
    /// here. 43 §1 gives no home for 「the substrate is unknown」, so a `plan` for an unregistered
    /// substrate is [`Error::NotFound`] and never a state.
    ///
    /// `SubstrateKind::Custom` dispatch is **not** interpreted (req/78 N-10, ASM-1): a custom kind
    /// registers and resolves by its string like any other, and no rule is attached to it.
    ///
    /// # 🔴 The version, and why it is an argument (hand 4, **M5H4-4**)
    ///
    /// 42 §3.9's `Environment.adapter_version` is required and 41 §4's trait cannot answer it: the
    /// seven methods report a kind, a snapshot, a plan, a precondition, an application, an inverse
    /// and a commutation, and N-07 forbids an eighth. The registrant knows which build it wired in,
    /// so the registrant says. A default of `"unknown"` was the alternative and is the same mistake
    /// as an empty verdict digest — a made-up value in a signed provenance record.
    pub fn register_adapter(
        &mut self,
        adapter: Arc<dyn SubstrateAdapter>,
        version: impl Into<String>,
    ) {
        self.adapters.insert(
            adapter.kind(),
            Registered {
                adapter,
                version: version.into(),
            },
        );
    }

    /// Set `EnforcementMode` (DR-2). `RecordOnly` is what opens T-8r.
    ///
    /// Global rather than per substrate. 43 §4 allows either (「substrate単位または全体設定」) and
    /// v0.1 takes the whole-deployment reading, because the per-substrate one needs a place to store
    /// a setting per `SubstrateKind` and nothing in 42 declares one.
    #[must_use]
    pub fn with_mode(mut self, mode: EnforcementMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set `FailPosture` (DR-2). The default is `FailClosed`, for every substrate.
    ///
    /// `FailOpen` is 43 T-4e's 「substrate明示的opt-inがある場合のみ有効」 and ASM-13's, and this is
    /// that opt-in. It is a builder rather than a default because a fail-open default is the one
    /// misconfiguration that looks like a working system.
    #[must_use]
    pub fn with_posture(mut self, posture: FailPosture) -> Self {
        self.posture = posture;
        self
    }

    /// 🔴 Set 43 T-6's two deadlines, in nanoseconds (ASM-12, 33 NFR-028).
    ///
    /// The defaults are 24 h and 72 h and AC-045 asks for 「短時間（例: 100ms）に設定したテスト構成」,
    /// which is this. A builder rather than a constant because the two values are a *deployment's*
    /// answer to 「how long may a change wait」 and 33 NFR-028 gives them as defaults rather than as
    /// fixed points.
    ///
    /// # No wall clock is involved, in the engine or in the test
    ///
    /// 41 §6 injects the clock, so 「TTL 経過」 is `now - since >= ttl` for the `now` a caller hands
    /// in, and a test reaches AC-045's condition by passing a later timestamp rather than by
    /// sleeping. That is the same property AC-039 rests on and it is worth stating out loud: the
    /// liveness criterion is measured deterministically, and a suite that slept would be measuring
    /// the scheduler.
    #[must_use]
    pub fn with_ttl(mut self, verify_ttl_nanos: i64, escalation_ttl_nanos: i64) -> Self {
        self.verify_ttl = verify_ttl_nanos;
        self.escalation_ttl = escalation_ttl_nanos;
        self
    }

    /// The journal, for a caller that wants to read what was written.
    #[must_use]
    pub fn journal(&self) -> &EngineJournal {
        &self.journal
    }

    /// The witness ledger of 42 §3.11, for a caller that wants the root, a proof or a leaf.
    ///
    /// Read-only on purpose: 43 T-11 is the one place a leaf is appended, and an accessor handing
    /// out `&mut` would be a second road to the exactly-once property INV-S3 asks for.
    #[must_use]
    pub fn ledger(&self) -> &LedgerStore {
        &self.ledger
    }

    /// 🔴 **FR-M04**: how many verdicts of each kind this deployment has issued in total.
    ///
    /// Cumulative since the log began — not since the last checkpoint. A caller that wants a
    /// window subtracts, which is what [`Engine::verdict_checkpoint`] does.
    #[must_use]
    pub fn verdict_tally(&self) -> VerdictTally {
        self.verdicts
    }

    /// 🔴 **FR-M04**: the chain of aggregate verdict checkpoints this deployment has published.
    #[must_use]
    pub fn verdict_checkpoints(&self) -> &[VerdictCheckpoint] {
        self.verdict_log.checkpoints()
    }

    /// 🔴 **FR-M04**: close the current window, sign the counts, and append them (SHOULD).
    ///
    /// # What this buys
    ///
    /// A `VerdictReceipt` is signed and then lives nowhere an outsider can reach — ASM-14 fixes its
    /// `inclusion_proof` to `None`, so an operator who does not export the refusals can show an
    /// auditor a hundred-percent-Admit record and the ledger will not contradict it, because the
    /// ledger only ever held the commits. This publishes the **count**, so that withholding the
    /// receipts stops being free.
    ///
    /// # What it does not buy
    ///
    /// Two things, and they are on `gx_core::VerdictCheckpoint` in full: a gate widened until
    /// nothing is refused publishes `deny = 0` honestly (裁定 #3), and one key can sign two
    /// internally consistent chains for two verifiers (裁定 #14, v0.2.1's consistency-proof
    /// window). What is closed is **non-disclosure**, and the AC says exactly that much.
    ///
    /// # The window closes even when it is empty
    ///
    /// Two calls with no verdict between them produce a second checkpoint whose window is empty
    /// rather than a repeat of the first, because a verifier folds the chain and a repeat would
    /// double every count in it. An empty window is a true statement about a quiet period.
    ///
    /// # Errors
    /// [`Error::Witness`] if the core cannot be signed, [`Error::Ledger`] if the checkpoint cannot
    /// be appended.
    pub fn verdict_checkpoint(
        &mut self,
        origin: &str,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<VerdictCheckpoint> {
        let window = VerdictTally {
            deny: self.verdicts.deny - self.published.deny,
            admit: self.verdicts.admit - self.published.admit,
            escalate: self.verdicts.escalate - self.published.escalate,
            unverdicted: self.verdicts.unverdicted - self.published.unverdicted,
        };
        let unsigned = gx_log::proof::unsigned_verdict_checkpoint(
            self.ledger.log(),
            origin,
            (self.published.total(), self.verdicts.total()),
            window,
            at,
        );
        let signed =
            gx_witness::dsse::sign_verdict_checkpoint(&unsigned, key.signing_key(), key.key_id())
                .map_err(|e| Error::Witness {
                action: "sign the verdict checkpoint",
                detail: e.to_string(),
            })?;
        // Append **before** the boundary moves: a window that was declared closed by a call which
        // then failed to write it is a hole in the chain that this process would go on to deny
        // ever making.
        self.verdict_log
            .append(signed.clone())
            .map_err(|e| Error::Ledger {
                action: "append the verdict checkpoint",
                detail: e.to_string(),
            })?;
        self.published = self.verdicts;
        Ok(signed)
    }

    /// The blob store this engine's delta bodies are in (**M5-05 採(a)**).
    ///
    /// Every `delta_cid` in the journal is a name this store resolves, which is what makes a
    /// `Planned` record enough to plan again from -- and what hand 4's T-10b will escrow an inverse
    /// into, through the same door.
    #[must_use]
    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }

    /// The enforcement mode this engine runs under.
    #[must_use]
    pub fn mode(&self) -> EnforcementMode {
        self.mode
    }

    /// The fail posture this engine runs under.
    #[must_use]
    pub fn posture(&self) -> FailPosture {
        self.posture
    }

    /// Whether an `IntentId` has a `DraftCreated` record.
    ///
    /// The draft phase's whole observable surface (**M5-17 採(b)**): a draft is a journal record and
    /// a membership question, not a row.
    #[must_use]
    pub fn is_drafted(&self, intent_id: &IntentId) -> bool {
        self.drafted.contains_key(intent_id)
    }

    /// Where a transformation is (43 §1).
    #[must_use]
    pub fn state(&self, id: &TransformationId) -> Option<Lifecycle> {
        self.table.get(id).map(|e| e.state)
    }

    /// The `Fingerprint₀` T-2 recorded (**AC-031**).
    ///
    /// AC-031 逐語: 「ストアから`Fingerprint₀`を後段commit時に再取得できる」. This accessor is the
    /// 「再取得」, and hand 4's `commit` is the 「後段commit時」 that will call it before T-10a's CAS.
    #[must_use]
    pub fn precondition_fingerprint(&self, id: &TransformationId) -> Option<&Fingerprint> {
        self.table.get(id).map(|e| &e.fp0)
    }

    /// The `PlannedDelta` T-2 fixed.
    ///
    /// Held in the row for now. **E-M4-8**'s durable CID-keyed blob store is **M5-05 採(a)** and
    /// hand 3's; what is here is the in-memory Σ, and a restart loses it. `EngineJournal` already
    /// holds the delta's *CID* in every `Planned` record, so hand 3's store is what turns that name
    /// back into a body.
    #[must_use]
    pub fn planned_delta(&self, id: &TransformationId) -> Option<&PlannedDelta> {
        self.table.get(id).map(|e| &e.delta)
    }

    /// The transformation itself.
    #[must_use]
    pub fn transformation(&self, id: &TransformationId) -> Option<&Transformation> {
        self.table.get(id).map(|e| &e.transformation)
    }

    /// The snapshot T-2 read.
    #[must_use]
    pub fn precondition_snapshot(&self, id: &TransformationId) -> Option<&ObjectSnapshot> {
        self.table.get(id).map(|e| &e.pre)
    }

    /// The verdict the gate reached, if one was reached.
    ///
    /// `None` after T-4e, where the gate was never asked. See [`Engine::fail_posture_engaged`].
    #[must_use]
    pub fn verdict(&self, id: &TransformationId) -> Option<VerdictKind> {
        self.table.get(id).and_then(|e| e.verdict)
    }

    /// Whether this transformation is being enforced (DR-2's `enforced`, default `true`).
    ///
    /// `false` after T-8r (a `Denied` carried through under `RecordOnly`) and after T-4e (a
    /// degraded admission). Both are 43 §4's 「record-onlyモード相当」, and INV-S5 requires the
    /// difference to be visible.
    #[must_use]
    pub fn enforced(&self, id: &TransformationId) -> Option<bool> {
        self.table.get(id).map(|e| e.enforced)
    }

    /// Whether `FailPosture::FailOpen` was exercised for this transformation (43 T-4e).
    ///
    /// The receipt seat for this already exists -- **E-M2-7** put `fail_posture_engaged` in
    /// `ReceiptPayload` in M2, which is why req/78's M5-12 was ruled 不採 as a 誤起票 (req/38 §37).
    /// Issuing the receipt is hand 4's; recording the fact is this hand's.
    #[must_use]
    pub fn fail_posture_engaged(&self, id: &TransformationId) -> Option<bool> {
        self.table.get(id).map(|e| e.fail_posture_engaged)
    }

    /// The canonical CID T-8 fixed.
    #[must_use]
    pub fn canonical_cid(&self, id: &TransformationId) -> Option<Cid> {
        self.table.get(id).and_then(|e| e.canonical_cid)
    }

    /// The receipt T-11 issued, if this transformation committed (42 §3.10, ASM-14's
    /// `CommitReceipt`).
    #[must_use]
    pub fn receipt(&self, id: &TransformationId) -> Option<&Receipt> {
        self.table.get(id).and_then(|e| e.receipt.as_ref())
    }

    /// What became of 43 T-10c's rollback, where one was in question (**AC-038**).
    #[must_use]
    pub fn rollback(&self, id: &TransformationId) -> Option<Rollback> {
        self.table.get(id).and_then(|e| e.rollback)
    }

    /// The moment the **engine** says the apply happened (**E-M4-31**, **M5-18 採(a)**).
    ///
    /// Not the adapter's: 41 §4's `apply` returns an `AppliedDelta` carrying an `applied_at` the
    /// adapter had no clock to fill, and gx-adapter-fs answers `Timestamp(0)` for exactly that
    /// reason. The engine rebuilds the value with the moment it was given.
    #[must_use]
    pub fn applied_at(&self, id: &TransformationId) -> Option<Timestamp> {
        self.table.get(id).and_then(|e| e.applied_at)
    }

    /// The CID of the inverse T-10b escrowed, if one could be constructed.
    #[must_use]
    pub fn escrowed_inverse(&self, id: &TransformationId) -> Option<Cid> {
        self.table.get(id).and_then(|e| e.inverse_cid)
    }

    /// The provenance the engine derived (42 §3.9, **M5-25 採(a)**).
    #[must_use]
    pub fn provenance(&self, id: &TransformationId) -> Option<&Provenance> {
        self.table.get(id).and_then(|e| e.provenance.as_ref())
    }

    /// The escalation ticket T-4c raised, with the clock **E-5** injected into it.
    #[must_use]
    pub fn ticket(&self, id: &TransformationId) -> Option<&EscalationTicket> {
        self.table.get(id).and_then(|e| e.ticket.as_ref())
    }

    /// 🔴 **M6H3-2 採(a)** — T-4a's `AdmitProof`, for the surface that has to say **why**.
    ///
    /// > engine に `admit_proof(&TID)`/`deny_reasons(&TID)` を足す(表の読み・Σ 影響なし)。手5 の
    /// > problem+json `detail` と同窓
    ///
    /// Two consumers, and they are the two 44 gives the word to: `gx verify`'s stdout
    /// (44 §1.2: `{"kind":"Admit","proof":AdmitProof}`) and the HTTP problem object's `detail`
    /// (44 §2.3: 「詳細説明」). Before this accessor both had [`Engine::verdict`]'s three-valued
    /// discriminant and a digest, which is enough to prove that a proof was hashed and not enough
    /// to tell anyone what it said.
    ///
    /// # 🔴 `None` means three different things, and only one of them is 「it was an Admit and the
    /// proof is missing」
    ///
    /// A row that has not been verified, a row whose verdict was `Deny` or `Escalate`, and a row
    /// **rebuilt from the journal** all answer `None`. The third is the one worth naming: 42 §3.13
    /// records `verdict_digest` and never the proof, so a second process reading Σ has the digest
    /// and not the value — the same limit [`Engine::verdict_receipts`] already carries. A caller
    /// that needs to tell the three apart reads [`Engine::verdict`] first, which Σ does restore.
    #[must_use]
    pub fn admit_proof(&self, id: &TransformationId) -> Option<&AdmitProof> {
        self.table.get(id).and_then(|e| e.admit_proof.as_ref())
    }

    /// 🔴 **M6H3-2 採(a)** — T-4b's reasons, for [`Engine::admit_proof`]'s reason one verdict along.
    ///
    /// 44 §1.2: `{"kind":"Deny","reasons":[Reason]}`. Each [`Reason`] carries a `code` from
    /// gx-gate's declared vocabulary, a bounded `message` and a `ReasonSource`, which is what makes
    /// 44 §2.3's `detail` for a `NOT_ADMITTED` refusal a sentence about the policy that refused
    /// rather than about the request that arrived.
    ///
    /// The same three-way `None` as `admit_proof`, and 42 §3.13 is the same reason.
    #[must_use]
    pub fn deny_reasons(&self, id: &TransformationId) -> Option<&[Reason]> {
        self.table.get(id).and_then(|e| e.deny_reasons.as_deref())
    }

    /// The `VerdictReceipt`s issued for this transformation, in the order they were issued
    /// (**M5H4-6**, ASM-14's first kind).
    ///
    /// One after T-4a/b/c or T-4e; a second after T-5/T-5b, signed by the ruler's key. Empty
    /// before a verdict and after a `plan` that has not been verified.
    #[must_use]
    pub fn verdict_receipts(&self, id: &TransformationId) -> &[Receipt] {
        self.table
            .get(id)
            .map_or(&[], |e| e.verdict_receipts.as_slice())
    }

    /// 43 T-12: which transformation's commit superseded this one (ASM-43-2, **M5-09 採(a)**).
    #[must_use]
    pub fn superseded_by(&self, id: &TransformationId) -> Option<TransformationId> {
        self.supersedes.superseded_by(id)
    }

    /// How many supersede edges have been drawn.
    #[must_use]
    pub fn supersede_count(&self) -> usize {
        self.supersedes.len()
    }

    /// 42 §3.12's status of the inverse escrowed for this transformation, if one was escrowed.
    #[must_use]
    pub fn inverse_status(&self, id: &TransformationId) -> Option<InverseStatus> {
        self.escrow.get(id).map(|row| row.status)
    }

    /// 43 §8's 「`blocked_by: TransformationId`という内部注釈」, if this transformation is waiting.
    #[must_use]
    pub fn blocked_by(&self, id: &TransformationId) -> Option<TransformationId> {
        self.table.get(id).and_then(|e| e.blocked_by)
    }

    /// When 43 T-6 will expire this transformation, if T-6 applies to the state it is in.
    ///
    /// `None` for every state outside `{Candidate, Verifying, Escalated}` — 43 T-6's from-column is
    /// those three and no others, so a `Canonicalized` transformation waiting for a `commit` call
    /// has no deadline. That is 43's design and not an oversight of this hand: the states T-6
    /// covers are the ones where the engine is waiting on somebody else.
    #[must_use]
    pub fn deadline(&self, id: &TransformationId) -> Option<Timestamp> {
        self.table.get(id).and_then(|e| self.deadline_of(e))
    }

    /// 43 T-6's deadline for one row, or `None` where T-6 does not reach.
    fn deadline_of(&self, entry: &Entry) -> Option<Timestamp> {
        let ttl = match entry.state {
            Lifecycle::Candidate | Lifecycle::Verifying => self.verify_ttl,
            Lifecycle::Escalated => self.escalation_ttl,
            _ => return None,
        };
        Some(Timestamp(entry.since.0.saturating_add(ttl)))
    }

    /// 🔴 **M5H3-4**: whether Σ's ledger component and the ledger's own log say the same thing.
    ///
    /// §40 rules 「手 4 が LedgerStore を結線した同 turn で『frontier と本物の root の一致』を
    /// probe 化する」, and this is the function a probe calls. Σ's `ledger` component is what the
    /// **journal** witnessed — a `(transformation, ledger_seq)` for every `Committed` record — and
    /// the log is the append-only tree gx-log holds. Three things are compared, because they can
    /// disagree in three ways:
    ///
    /// 1. **the count** — a `Committed` record with no leaf is a journal claiming a commit the
    ///    ledger never took, and a leaf with no `Committed` record is 43 §7-3b's crash window (the
    ///    append landed and the record did not);
    /// 2. **each row** — the leaf at `ledger_seq` names that transformation, so a sequence number
    ///    copied from the wrong place is visible;
    /// 3. **the root** — the tree's root at the frontier's size equals its current root, which is
    ///    what makes 「the frontier」 and 「the log」 the same tree rather than two lists of the same
    ///    length.
    ///
    /// Turning that check into a *repair* is 43 §7-3b's recovery and hand 5's. What hand 4 owes is
    /// the observation, running all the time, so that the recovery has something to be a repair of.
    #[must_use]
    pub fn ledger_agrees(&self) -> bool {
        let frontier = self.committed.len() as u64;
        if frontier != self.ledger.log().len() {
            return false;
        }
        for (transformation, seq) in &self.committed {
            match self.ledger.log().entry(*seq) {
                Some(entry) if entry.transformation == *transformation => {}
                _ => return false,
            }
        }
        self.ledger.log().root_at(frontier) == self.ledger.log().root()
    }

    /// 🔴 **M6H5-12 採(a)** — the engine's version, from the engine.
    ///
    /// An associated function rather than a method: the answer does not depend on which engine is
    /// asked, and a `&self` would suggest it might. See [`crate::VERSION`] for why the borrowed
    /// constant hand 5 wrote in gx-api had to move here even though the two strings are equal today.
    #[must_use]
    pub fn version() -> &'static str {
        crate::VERSION
    }

    /// Every transformation the table holds, in `TransformationId` order.
    #[must_use]
    pub fn transformation_ids(&self) -> Vec<TransformationId> {
        self.table.keys().copied().collect()
    }

    /// 🔴 **M6-07 採(b)** — the rows about `subject`, in `TransformationId` order.
    ///
    /// The subject index read out loud. Equal, always, to filtering [`Engine::transformation_ids`]
    /// by `transformation(&id).subject`, and `crates/gx-engine/tests/subject_index.rs` asserts that
    /// equality rather than describing it — an index is a second answer to a question the table
    /// already answers, and two answers are two things that drift.
    ///
    /// Empty for a subject the table has never seen. **Not** 「every row」: a miss that fell back to a
    /// full scan would still be correct and would put back exactly the cost the index removed, and no
    /// correctness probe would ever notice.
    #[must_use]
    pub fn transformations_on(&self, subject: &Subject) -> Vec<TransformationId> {
        self.by_subject
            .get(subject)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// The one door into the state table, so that the subject index cannot be forgotten at three of
    /// four call sites.
    ///
    /// Four callers: T-2's `plan`, `plan`'s rehydrating branch, `undo`'s inverse candidate, and
    /// [`Engine::rehydrate_committed`] (M6H4-4). The last is the one a reader misses, because a
    /// later hand wrote it; making the insert private and the index automatic is the structural
    /// answer to 「remember to update both」, which is the shape M6-09's `_`-arm ban takes in a
    /// different corner of this workspace.
    ///
    /// Re-seating an id that is already there moves it between subject buckets rather than leaving a
    /// stale entry behind. A re-plan (43 T-2's idempotency column) replaces the row, and a
    /// re-plan whose `Fingerprint₀` was taken over a different object would otherwise leave this id
    /// in two buckets at once.
    fn seat(&mut self, id: TransformationId, entry: Entry) {
        if let Some(previous) = self.table.get(&id) {
            let previous = previous.transformation.subject;
            if previous != entry.transformation.subject {
                if let Some(bucket) = self.by_subject.get_mut(&previous) {
                    bucket.remove(&id);
                    if bucket.is_empty() {
                        self.by_subject.remove(&previous);
                    }
                }
            }
        }
        self.by_subject
            .entry(entry.transformation.subject)
            .or_default()
            .insert(id);
        self.table.insert(id, entry);
    }

    /// 🔴 **Σ** — this engine's state, from its own tables (**E-M5-2**, AC-039's 「元の結果状態」).
    ///
    /// > **E-M5-2**: AC-039 の「結果状態」=Σ(状態表+ledger root+escrow index)
    ///
    /// # This function must not read the journal, and a probe says so
    ///
    /// AC-039 compares this value with [`crate::replay::reconstruct`]'s. If Σ were built here by
    /// replaying the journal, both sides would come from the same bytes and the criterion would
    /// hold however wrong the reconstruction was. So every field below comes from `self.drafted`
    /// and `self.table` -- the caches the transitions maintain as they run -- and
    /// `tests/store_shape.rs::the_engine_builds_sigma_from_its_tables_and_not_from_its_journal`
    /// reads this body to check it. 則 1 (「メモリ上の表は cache であって state ではない」) is why
    /// the comparison is worth making: a cache that has drifted from the journal is a bug the
    /// journal cannot show on its own.
    ///
    /// # The two components that stopped being empty (hand 4)
    ///
    /// Hand 3 returned empty vectors for `escrow` and `ledger` and said why: no transition it had
    /// implemented wrote them. T-10b and T-11 are this hand's, so both are live now, and the
    /// consequence is what hand 3 asked for -- `tests/ac_039.rs` compares Σ against a journal
    /// **an execution wrote** rather than one a test assembled, which is 「2 経路の一致」 rather
    /// than 「再構成そのもの」.
    ///
    /// `superseded_by` is still `None` on every row: T-12 is hand 6's, and the reconstruction side
    /// already reads the record that will carry it.
    #[must_use]
    pub fn sigma(&self) -> Sigma {
        Sigma::new(
            self.drafted
                .iter()
                .map(|(intent_id, rng_seed)| DraftRow {
                    intent_id: *intent_id,
                    rng_seed: *rng_seed,
                })
                .collect(),
            self.table
                .iter()
                .map(|(id, e)| StateRow {
                    transformation: *id,
                    intent_id: Some(e.intent_id),
                    delta_cid: Some(e.delta.reference().cid),
                    fp0: Some(FingerprintRecord::of(&e.fp0)),
                    state: Some(e.state),
                    verdict: e.verdict,
                    verdict_digest: e.verdict_digest,
                    enforced: e.enforced,
                    fail_posture_engaged: e.fail_posture_engaged,
                    canonical_cid: e.canonical_cid,
                    apply_started: e.apply_started,
                    rollback: e.rollback,
                    provenance: e.provenance.clone(),
                    // 🔴 T-12's edge, live since hand 6. Hand 4 wrote `None` here and said why:
                    // the reconstruction side already read the record that carries it, and the
                    // live side had no transition that wrote one.
                    superseded_by: e.superseded_by,
                })
                .collect(),
            self.escrow.values().copied().collect(),
            self.committed
                .iter()
                .map(|(transformation, ledger_seq)| CommittedRow {
                    transformation: *transformation,
                    ledger_seq: *ledger_seq,
                })
                .collect(),
        )
    }

    // -----------------------------------------------------------------------
    // T-1
    // -----------------------------------------------------------------------

    /// **T-1** `submit(intent)` — mint the `IntentId` and record a draft.
    ///
    /// 43 T-1's row, field by field: the guard is 「intentがschema適合」 (which the `Intent` type
    /// carries, having been built through gx-core's constructor), the side effect is 「canonical
    /// encodeでintent CID確定→`IntentId`確定（ASM-11）；journal: `DraftCreated{intent_id}`」, and the
    /// idempotency rule is 「同一canonical encodeのintent再送は同一`IntentId`を返す（副作用なし、
    /// create-if-absent）」.
    ///
    /// **The idempotency is not a promise, it is the return path.** A resubmitted intent takes the
    /// early return below and never reaches [`EngineJournal::append`], so 「副作用なし」 is a fact
    /// about which statements ran. `tests/ac_030.rs` measures it by counting journal records, which
    /// is the only way to tell 「it returned the same id」 from 「it returned the same id and wrote
    /// another record」.
    ///
    /// `rng_seed` is 41 §6's injected randomness. It reaches the journal and nothing else in this
    /// hand -- FR-039's replay is what consumes it, and that is hand 3.
    ///
    /// # Errors
    /// [`Error::Canon`] if the intent has no canonical form. [`Error::Io`] if the journal cannot be
    /// appended to.
    pub fn submit(&mut self, intent: &Intent, rng_seed: u64, at: Timestamp) -> Result<IntentId> {
        let intent_id = IntentId(cid::compute(intent)?);
        if self.drafted.contains_key(&intent_id) {
            return Ok(intent_id);
        }
        self.journal.append(EngineJournalRecord::DraftCreated {
            intent_id,
            rng_seed,
            at,
        })?;
        self.drafted.insert(intent_id, rng_seed);
        Ok(intent_id)
    }

    // -----------------------------------------------------------------------
    // T-2
    // -----------------------------------------------------------------------

    /// **T-2** `plan()` — snapshot, plan, fix `Fingerprint₀` and the `TransformationId`.
    ///
    /// The `Intent` is handed in again rather than looked up, because there is nowhere to look it
    /// up from: **M5-17 採(b)** keeps the draft phase in the journal and the journal records an
    /// `IntentId`, not a body. 44 §1.2's `gx plan <ID>` resolves an id to a session the CLI is
    /// holding; a library API resolves it to the value the caller still has.
    ///
    /// # Where `target` comes from, and 🔴 why it is `None`
    ///
    /// 43 T-2 says the `TransformationId` is 「delta/target込みcanonical formのCID」, so `target` --
    /// 「The expected post-state digest, fixed by `plan()`」 (41 §3) -- has to be known here. **It is
    /// not knowable.** `SubstrateAdapter` has seven methods and none of them returns a predicted
    /// post-state: `plan` returns a `PlannedDelta` of `{substrate, payload, reference}`, and the only
    /// type carrying a `resulting_digest` is `AppliedDelta`, which exists *after* `apply`. So v0.1
    /// fixes `target = None` and the canonical form includes the absence.
    ///
    /// This is worth stating plainly because of what it does to **M5-11 / 卓-5**. That ticket asks
    /// how the engine should refuse when 「plan の予言と apply の実測」 disagree, and req/38 §37 sent
    /// it to the Owner desk with an instruction for this milestone: 「裁定まで engine 側の拒否検査は
    /// 書かず、**「検査の不在」を doc に 1 行**(隠さない)」. Here is that line, and one more:
    /// **the check is absent because the prediction is absent** -- there is no `target` for an
    /// `AppliedDelta.resulting_digest` to disagree with, so the comparison 卓-5 is about cannot be
    /// written against today's trait at all. Raised as **M5H2-2**.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the intent has no draft or its substrate has no registered adapter.
    /// [`Error::Adapter`] if `snapshot`, `plan` or `precondition` refuses. [`Error::Canon`] if the
    /// transformation has no canonical form. [`Error::Io`] from the journal.
    pub fn plan(&mut self, intent: &Intent, at: Timestamp) -> Result<TransformationId> {
        let intent_id = IntentId(cid::compute(intent)?);
        if !self.drafted.contains_key(&intent_id) {
            return Err(Error::NotFound {
                what: "draft",
                id: format!("{intent_id:?}"),
            });
        }
        // 43 T-2's idempotency column: 「同一snapshotに対し再実行しても同一`PlannedDelta`・同一
        // `TransformationId`（安全に再試行可）」. Safe to retry is not safe to *rewind*: a candidate
        // that has since been verified, denied or canonicalised would have its row replaced by a
        // fresh `Candidate` and its verdict forgotten. So a re-plan is allowed only while the row is
        // still where T-2 left it, and refused otherwise.
        if let Some(existing) = self
            .table
            .values()
            .find(|e| e.intent_id == intent_id && e.state != Lifecycle::Candidate)
        {
            return Err(Error::InvalidState {
                id: format!("{:?}", existing.transformation.id),
                state: existing.state.name(),
                attempted: "plan",
            });
        }

        let adapter = self
            .adapters
            .get(intent.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", intent.substrate()),
            })?
            .adapter
            .clone();

        let pre = adapter
            .snapshot(intent.locator())
            .map_err(|e| Error::Adapter {
                action: "snapshot",
                detail: e.to_string(),
            })?;
        let delta = adapter.plan(intent, &pre).map_err(|e| Error::Adapter {
            action: "plan",
            detail: e.to_string(),
        })?;
        let fp0 = adapter.precondition(&pre).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;

        // `id` is outside the IdentityView (42 §1.3, ASM-4), so the CID computed over the value
        // carrying a placeholder is bit-for-bit the CID computed over the finished one. This is
        // gx-core's own `PROVISIONAL_ID` convention; the constant is private there, so the zero
        // value is written here with the same reasoning rather than reached for.
        let mut transformation = Transformation::new(
            TransformationId(Cid([0u8; 32])),
            0,
            Subject::Object(*pre.id()),
            None,
            Vec::new(),
            CompositionMetadata {
                intent_id,
                delta: delta.reference().clone(),
                context: intent.context().clone(),
                actor: intent.actor().clone(),
                created_at: at,
            },
        )?;
        let id = TransformationId(cid::compute(&transformation)?);
        transformation.id = id;

        // 🔴 **M6 hand 3 — the same intent, planned again in another process.**
        //
        // Nothing above this line has written anything: `snapshot`, `plan` and `precondition` are
        // 41 §4's read-only three, and the identity is a function of what they returned. So this is
        // the first point at which the two cross-process questions can be asked, and the last point
        // at which they can be answered without a side effect.
        //
        // 44 §1.2 runs `gx submit`, `gx plan`, `gx verify` and `gx commit` as **four processes**,
        // and `Engine::open` rebuilds the draft phase and the resolution index from the journal
        // while leaving the in-flight table empty (M5H3-5). A row's *body* — the `Transformation`,
        // the `ObjectSnapshot`, the `PlannedDelta` — is not in the journal (ASM-9), so the only way
        // a later process can hold one is to plan it again; and 43 T-2's idempotency column is what
        // makes that legal: 「同一snapshotに対し再実行しても同一`PlannedDelta`・同一
        // `TransformationId`（安全に再試行可）」.
        //
        // Two things follow, and both are refinements of guards this function already had for the
        // single-process case.
        let recorded = self.resolved.get(&intent_id).copied();
        let rehydrating = recorded == Some(id) && !self.table.contains_key(&id);
        if let Some(recorded) = recorded {
            if !rehydrating && !self.table.contains_key(&recorded) {
                // The guard above this one — 「a re-plan is allowed only while the row is still
                // where T-2 left it」 — reads the **table**, and the table is empty after a restart.
                // Read from the journal instead and the same rule holds across processes: an intent
                // whose transformation has already been verified, denied or committed may not be
                // re-planned into a second one, because doing so would leave the first row's
                // verdict behind and answer a later `gx verify <TID>` about a transformation the
                // operator did not name. The refusal is 43 §8's 「再`plan()`を強制する」 seen from
                // the other side, and it costs no journal record.
                let sigma = reconstruct(self.journal.records());
                if let Some(state) = sigma.state_of(&recorded).and_then(|row| row.state) {
                    if !matches!(state, Lifecycle::Candidate) {
                        return Err(Error::InvalidState {
                            id: format!("{recorded:?}"),
                            state: state.name(),
                            attempted: "plan",
                        });
                    }
                }
            }
        }
        if rehydrating {
            // 🔴 The body comes from the re-plan and the **state comes from the journal**.
            //
            // This is the split M5H3-5 left open, answered where the answer exists rather than in
            // `Engine::open`: `open` cannot rebuild a row because it has no adapter to ask, and by
            // the time this line runs the adapter has answered. What the journal does hold is every
            // transition, which is req/78 Λ1's whole claim about Σ — so a row rebuilt here carries
            // the state, the verdict, the flags and the canonical CID the log recorded, and no
            // second `Planned` record is appended for something that already happened.
            //
            // req/88 §3 Λ2 is the reason for this shape rather than a re-drive: 「N 回の CLI 実行」
            // and 「1 個の長寿命 engine への N 回の呼び出し」 are observationally equal on Σ **only
            // if** the second process writes nothing the first would not have written twice. A
            // resume that re-verified would put a second `VerifyStarted` and a second `Verdict` in
            // the log and issue a second verdict receipt — one long-lived engine's journal and a
            // single-shot CLI's journal disagreeing about how many times the gate was asked.
            //
            // What it does **not** restore is what the journal does not hold: the verdict receipts
            // and the in-flight annotations `blocked_by` and `since`.
            //
            // 🔴 **The escalation ticket is no longer on that list** (M6H3-10, settled by
            // measurement in M6 hand 4). The journal records the verdict and not the ticket, and
            // 42 §3.13's vocabulary does **not** grow, because the ticket did not have to be
            // recorded to be recovered: `gx_gate::escalation_ticket` is the one road E-M3-4 takes
            // and it reads nothing but the `TransformationId`, so a row whose journalled verdict is
            // `Escalate` can rebuild the ticket it raised. See [`Engine::rebuilt_ticket`].
            self.blobs.put(&delta)?;
            let sigma = reconstruct(self.journal.records());
            let row = sigma.state_of(&id);
            let ticket = match row.and_then(|r| r.verdict) {
                Some(VerdictKind::Escalate) => self.rebuilt_ticket(&id)?,
                _ => None,
            };
            self.seat(
                id,
                Entry {
                    intent_id,
                    transformation,
                    state: row.and_then(|r| r.state).unwrap_or(Lifecycle::Candidate),
                    since: at,
                    blocked_by: None,
                    delta,
                    fp0,
                    pre,
                    verdict: row.and_then(|r| r.verdict),
                    verdict_digest: row.and_then(|r| r.verdict_digest),
                    enforced: row.is_none_or(|r| r.enforced),
                    fail_posture_engaged: row.is_some_and(|r| r.fail_posture_engaged),
                    canonical_cid: row.and_then(|r| r.canonical_cid),
                    ticket,
                    // Not in Σ, so not restored: the journal records the verdict's digest and
                    // never the proof (42 §3.13). A rebuilt row therefore answers `None` here, the
                    // same way it answers with an empty `verdict_receipts` below, and M6H3-2's
                    // 「Sigma 影響なし」 is what that costs.
                    admit_proof: None,
                    deny_reasons: None,
                    verdict_receipts: Vec::new(),
                    superseded_by: row.and_then(|r| r.superseded_by),
                    apply_started: row.and_then(|r| r.apply_started),
                    rollback: row.and_then(|r| r.rollback),
                    provenance: row.and_then(|r| r.provenance.clone()),
                    inverse_cid: None,
                    applied_at: None,
                    receipt: None,
                },
            );
            return Ok(id);
        }

        self.journal.append(EngineJournalRecord::Planned {
            transformation: id,
            intent_id,
            // **E-M5-13**, the locator half (M5H5-2): read off the intent the caller still has, so
            // that 43 §7-3c can name what it was planning against without the body ASM-9 discards.
            locator: intent.locator().to_string(),
            delta_cid: delta.reference().cid,
            fp0: FingerprintRecord::of(&fp0),
            // **E-M5-13**, the parents half (M5H6-6). Empty here: a `plan` of order 0 has no
            // predecessor, and the one producer of a non-empty list is `undo`.
            parents: transformation.parents.clone(),
            at,
        })?;
        self.resolved.insert(intent_id, id);

        // 🔴 **E-M4-8 / M5-05 採(a)**, and journal-first in the order of two statements: the name
        // goes into the journal, then the body goes into the store. 42 §5 makes keeping the body
        // mandatory (「保管する（必須）」 for the escrowed inverse, and E-M4-8 extends it to the
        // planned delta so that replay and undo are constructible at all). A re-plan of the same
        // intent lands on the same CID and is answered `AlreadyPresent` without a second write,
        // which is **M4H6-3** on the live path rather than only in a probe.
        self.blobs.put(&delta)?;

        self.seat(
            id,
            Entry {
                intent_id,
                transformation,
                state: Lifecycle::Candidate,
                // 43 T-6 starts counting here. A re-plan (43 T-2's idempotency column) replaces
                // the row, which restarts the clock -- correct, because `Fingerprint₀` was taken
                // again and the transformation is waiting on a fresh precondition.
                since: at,
                blocked_by: None,
                delta,
                fp0,
                pre,
                verdict: None,
                verdict_digest: None,
                enforced: true,
                fail_posture_engaged: false,
                canonical_cid: None,
                ticket: None,
                admit_proof: None,
                deny_reasons: None,
                verdict_receipts: Vec::new(),
                superseded_by: None,
                apply_started: None,
                rollback: None,
                provenance: None,
                inverse_cid: None,
                applied_at: None,
                receipt: None,
            },
        );
        Ok(id)
    }

    // -----------------------------------------------------------------------
    // T-3, T-4a..T-4e
    // -----------------------------------------------------------------------

    /// **T-3 → T-4a/T-4b/T-4c/T-4d/T-4e** — collect evidence, ask the gate, record what came back.
    ///
    /// One entry point for six transitions, because 43 gives them one trigger and one from-state:
    /// everything below `Verifying` in the table is a branch on what the two collaborators answered,
    /// and splitting them into six functions would put the branch in the caller.
    ///
    /// | what answered | verdict | to | transition |
    /// |---|---|---|---|
    /// | collector `Ok`, gate `Admit` | `Admit` | `Admitted` | T-4a |
    /// | collector `Ok`, gate `Deny` | `Deny` | `Denied` | T-4b |
    /// | collector `Ok`, gate `Escalate` | `Escalate` | `Escalated` | T-4c |
    /// | collector `Err`, `FailClosed` | — | `Aborted(VerifierUnavailable)` | T-4d |
    /// | collector `Err`, `FailOpen` | `Admit`, degraded | `Admitted`, `enforced=false` | T-4e |
    /// | collector `Ok`, gate `Err` (⊥) | — | `Aborted(InternalError)` | **see below** |
    ///
    /// # 🔴 The gate's ⊥ is not the collector's `Err` (**M5-23 採(a)** = **E-M5-5**)
    ///
    /// **E-M3-3** made `Gate::verify` fallible and said what the failure is not: 「評価不能(⊥)は Deny
    /// でも Escalate でもない」. **M5-23 採(a)** settles what it *is* on this side:
    ///
    /// > `RecordOnly` は `Deny` にのみ効き、⊥(Err)には効かない——⊥は「判定が存在しない」ので常に
    /// > `Aborted`(fail-closed)
    ///
    /// So ⊥ aborts whatever the enforcement mode says, and the code below reads the mode nowhere on
    /// that arm. What 43 does not say is **which** `AbortReason` it carries, and there are only two
    /// candidates. It is **not** `VerifierUnavailable`: **M5-03 採(a)** makes the collector's `Err`
    /// that reason's only producer, and a second producer would delete the property AC-036 is
    /// measured by. It is `InternalError`, which is 43 T-13's 「engine内部の想定外失敗…バグ級の失敗」
    /// -- and the same reading **M5-24 採(a)** already applied to `cas_eq`'s `Err`, where a lower
    /// layer's 「this is a wiring bug」 must not be dressed as an ordinary business condition. A gate
    /// that cannot evaluate is a deployment with an unreadable policy set or a registry that refused
    /// to build; both are 「壊れている」 rather than 「拒否された」, which is the distinction E-M3-3
    /// exists to keep. Raised as **M5H2-5**, because deriving a reason from two rulings is still a
    /// reading.
    ///
    /// # T-4e writes a `Verdict` record for a verdict that does not exist
    ///
    /// 43 T-4e's journal cell is `Verdict{id, Admit, fail_posture_engaged=true}`, and no gate ran.
    /// The record's `verdict_digest` is therefore `None` -- see [`EngineJournalRecord::Verdict`] for
    /// why an empty `AdmitProof` was not minted to fill it.
    ///
    /// # `invert_available` costs a call 43 does not schedule
    ///
    /// 41 §4 gives `GateInput` a field for it and **E-M3-4** makes `false` the one condition that
    /// produces an `Escalate` in v0.1, so the gate cannot be asked without it. 43 schedules
    /// `adapter.invert` at T-10b (escrow, before `apply`), which is hand 4's and far too late. So
    /// this hand calls it here as well: it is a read, it takes `(delta, pre)` and no clock, and 41
    /// §4 asks the adapter for 「逆delta（undo保証, DR-1(a)）。構成不能なら None」 rather than for a
    /// side effect. Two calls to one pure function is the cost of 41 §4 and 43 §3 scheduling the
    /// same question differently; recorded as **M5H2-6** rather than absorbed silently.
    ///
    /// # 🔴 Three things hand 6 adds, and none of them is a new state
    ///
    /// * **T-6, lazily** (M5-10 採(a)). The deadline is evaluated before the guard, so a
    ///   `Candidate` that has sat past `verify_ttl` becomes `Aborted(Expired)` and this call then
    ///   refuses it as an invalid state. INV-L1 without a resident process.
    /// * **43 §8's waiting** (AC-045's second clause). Before `VerifyStarted` is written, the
    ///   engine asks `adapter.commutation` about every in-flight transformation on the same
    ///   `Subject` that has already started verifying, and a `Conflicts` holds this one at
    ///   `Candidate` with `blocked_by` set — 「新たな状態は追加しない。`blocked_by:
    ///   TransformationId`という内部注釈のみ」. **The TTL keeps running while it waits**, which is
    ///   INV-L4.
    /// * **ASM-14's verdict receipt** (M5H4-6). Every road out of this function that reaches a
    ///   verdict issues one, T-4e included — 43 T-4e requires 「`enforced=false`と
    ///   `fail_posture_engaged=true`を必ずreceiptに刻む」 and the only receipt that exists at that
    ///   moment is a verdict-stage one. That receipt is what **E-M5-11** made writable.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the id is not in the table, [`Error::InvalidState`] if it is not a
    /// `Candidate` (which includes a row 43 §8 is holding behind a conflicting commit),
    /// [`Error::NotFound`] if its adapter has been unregistered, [`Error::Adapter`] if `invert` or
    /// `commutation` refuses, [`Error::Io`] from the journal, [`Error::Witness`] if the verdict
    /// receipt cannot be issued, and [`Error::Canon`] if a verdict has no canonical form. A
    /// collector that refuses and a gate that cannot evaluate are **not** errors here: they are
    /// transitions, and they come back in the `Ok`.
    pub fn verify(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
        mode: Option<EnforcementMode>,
    ) -> Result<Lifecycle> {
        // 🔴 **M6-08 採(a)** (req/38 §47) — the per-call override 44 §1.2 asks for in as many words:
        //
        // > `--record-only`: DR-2 record-onlyモードを**本コマンド単位で**強制（グローバル設定の上書き）
        //
        // [`Engine::with_mode`] is a builder that consumes `self` at `open`, which a single-shot CLI
        // can use and a long-lived `gx serve` cannot: 44 §2.2's `POST /candidates/{id}/verify` body
        // carries `record_only: bool|null` **per request**, and a server that answered it by
        // reassigning a field on shared state would leak one request's posture into another's — the
        // fail-open M6-08(b) was written down as the form 「採ってはならない」. So the override is an
        // argument, `None` means 「use the engine's setting」, and no state moves.
        let mode = mode.unwrap_or(self.mode);
        // 43 T-6, before anything else: a deadline that has passed is a transition that already
        // happened, and answering a request about a row without evaluating it would make liveness
        // depend on who called.
        self.expire_if_due(id, at)?;

        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        if entry.state != Lifecycle::Candidate {
            return Err(Error::InvalidState {
                id: format!("{id:?}"),
                state: entry.state.name(),
                attempted: "verify",
            });
        }

        // 43 §8. A row that was waiting is re-evaluated first: 「`T1`が終端…に達した時点で`T2`は
        // 再評価される: `T1`が`Committed`なら`T2`の`Fingerprint₀`は陳腐化しているため再`plan()`
        // （再fingerprint）を強制する」.
        if let Some(blocker) = entry.blocked_by {
            if self.table.get(&blocker).map(|e| e.state) == Some(Lifecycle::Committed) {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: "Candidate (blocked)",
                    attempted: "verify after a conflicting commit; 43 §8 forces a re-plan",
                });
            }
            if let Some(entry) = self.table.get_mut(id) {
                entry.blocked_by = None;
            }
        }
        if let Some(blocker) = self.conflicting_predecessor(id, mode)? {
            if let Some(entry) = self.table.get_mut(id) {
                entry.blocked_by = Some(blocker);
            }
            // Still `Candidate`, still on the clock. No journal record: 43 §8 adds no transition,
            // and a log entry for 「nothing happened」 would make a replay report waiting as an
            // event.
            return Ok(Lifecycle::Candidate);
        }

        // T-3, journal-first.
        self.journal.append(EngineJournalRecord::VerifyStarted {
            transformation: *id,
            at,
        })?;
        self.set_state(id, Lifecycle::Verifying, at);

        let entry = &self.table[id];
        let collected = self.evidence.collect(&entry.transformation, &entry.pre);

        let evidence = match collected {
            Ok(evidence) => evidence,
            Err(_) => return self.unreachable_collector(id, at, key),
        };

        let entry = &self.table[id];
        let adapter = self
            .adapters
            .get(entry.delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", entry.delta.substrate()),
            })?
            .adapter
            .clone();
        let invert_available = adapter
            .invert(&entry.delta, &entry.pre)
            .map_err(|e| Error::Adapter {
                action: "invert",
                detail: e.to_string(),
            })?
            .is_some();

        let planned = PlannedDeltaBytes(entry.delta.payload().to_vec());
        let answered = self.gate.verify(GateInput {
            t: &entry.transformation,
            pre: &entry.pre,
            planned: &planned,
            evidence: &evidence,
            invert_available,
        });

        let verdict = match answered {
            Ok(verdict) => verdict,
            // ⊥ -- see the section above. Not the collector's road, not RecordOnly's business.
            Err(_) => {
                self.journal.append(EngineJournalRecord::Aborted {
                    transformation: *id,
                    reason: AbortReason::InternalError,
                    // No rollback question arises before the critical section: nothing has been
                    // escrowed and nothing has been applied (see `Rollback` for why `None` and
                    // `Some(NotAttempted)` are different facts).
                    rollback: None,
                    at,
                })?;
                return Ok(self.set_state(id, Lifecycle::Aborted(AbortReason::InternalError), at));
            }
        };

        let kind = verdict.kind();
        let digest = verdict.proof_digest().map_err(|e| Error::Malformed {
            detail: format!("the verdict has no canonical form: {e}"),
        })?;
        self.journal.append(EngineJournalRecord::Verdict {
            transformation: *id,
            kind,
            verdict_digest: Some(digest),
            fail_posture_engaged: false,
            at,
        })?;

        let to = match &verdict {
            Verdict::Admit(_) => Lifecycle::Admitted,
            Verdict::Deny(_) => Lifecycle::Denied,
            Verdict::Escalate(_) => Lifecycle::Escalated,
        };
        // T-4c's side effect: 「`EscalationTicket`生成」. The ticket the gate built carries
        // `Timestamp(0)` because 41 §6 keeps clocks out of that layer, and its `id` is a value the
        // gate minted -- **E-5** injects the one and **E-6** checks the other.
        //
        // 🔴 **M6H3-2 採(a)**: the other two arms are kept as well, in the two fields beside
        // `ticket`. Until this hand the `Verdict` was consumed here for its `kind` and its digest
        // and then dropped, so 44 §1.2's `{"kind":"Admit","proof":AdmitProof}` and
        // `{"kind":"Deny","reasons":[Reason]}` had nothing behind them and the HTTP surface's
        // problem `detail` (44 §2.3: 「詳細説明」) had a digest to offer an operator asking 「why」.
        // The `match` moves the value rather than cloning it, so nothing is stored twice.
        let (ticket, admit_proof, deny_reasons) = match verdict {
            Verdict::Escalate(ticket) => (Some(self.checked_ticket(ticket, at)?), None, None),
            Verdict::Admit(proof) => (None, Some(proof), None),
            Verdict::Deny(reasons) => (None, None, Some(reasons)),
        };
        if let Some(entry) = self.table.get_mut(id) {
            entry.verdict = Some(kind);
            entry.verdict_digest = Some(digest);
            entry.ticket = ticket;
            entry.admit_proof = admit_proof;
            entry.deny_reasons = deny_reasons;
        }
        let state = self.set_state(id, to, at);
        // ASM-14's first kind, for all three verdicts (42 §3.10: 「全`Verdict`＝Admit/Deny/Escalate
        // で発行、43 T-4a/T-4b/T-4c」).
        self.issue_verdict_receipt(
            id,
            Some(VerdictSummary {
                kind,
                proof_digest: digest,
            }),
            at,
            key,
        )?;
        Ok(state)
    }

    /// **E-5 / E-6**: the ticket 43 T-4c raised, with the engine's clock in it and its id checked.
    ///
    /// > **E-5**: ticket `created_at` は engine が注入
    /// > **E-6**: ticket 読み戻しは checked constructor 必須
    ///
    /// The two are one function because the ticket crosses one boundary. gx-gate builds it and
    /// says so in as many words -- 「`created_at` is the one field with no honest source. 41 §6
    /// keeps clocks out of this layer…so the value written is `Timestamp(0)` -- the epoch, as a
    /// placeholder the engine overwrites」 -- and this is the overwrite (**E-5**, the same shape
    /// E-M4-31 gave `AppliedDelta.applied_at`).
    ///
    /// **E-6** is the other half. 42 §1.3 makes `TicketId` a digest of the ticket's projection and
    /// gx-gate mints it there; a value arriving from outside is a *claim* that the ticket hashes to
    /// that name until something recomputes it. This does, and refuses a disagreement rather than
    /// filing it — which is the `ReceiptPayload` shape E-6 names, one type over. `created_at` is
    /// outside the projection (ASM-4), so injecting the clock cannot move the id.
    ///
    /// # Errors
    /// [`Error::Canon`] if the ticket has no canonical form, [`Error::InconsistentTicket`] if its
    /// `id` is not the digest of what it holds.
    fn checked_ticket(
        &self,
        mut ticket: EscalationTicket,
        at: Timestamp,
    ) -> Result<EscalationTicket> {
        let minted = TicketId(cid::compute(&ticket)?);
        if minted != ticket.id {
            return Err(Error::InconsistentTicket {
                detail: format!(
                    "the ticket is filed as {} and its contents hash to {}",
                    cid::to_text(&ticket.id.0),
                    cid::to_text(&minted.0)
                ),
            });
        }
        ticket.created_at = at;
        Ok(ticket)
    }

    /// 🔴 **M6H3-10 採(b), the answer** — the ticket a journalled `Escalate` raised, rebuilt.
    ///
    /// req/38 §50 sent hand 4 to measure before ruling: 「`EscalationTicket` が row から再構成できるか
    /// を実測してから journal 語彙増(a)を判定」. The measurement is
    /// `crates/gx-engine/tests/ticket_rehydration.rs` and the answer is that it can, because
    /// [`gx_gate::escalation_ticket`] is a function of the `TransformationId` and of two constants
    /// (E-M3-4's one reason and ASM-60-3's approval requirement). ∴ **42 §3.13 does not grow.**
    ///
    /// The rebuild goes through [`Engine::checked_ticket`], which is the point rather than a
    /// formality: E-6's rule is 「ticket 読み戻しは checked constructor 必須」, and this **is** a read
    /// back — of a value nothing stored. It is also the **second producer** of
    /// [`Error::InconsistentTicket`] that §43 M5H6-8② predicted the CLI/API surface would reach:
    /// the first is a gate handing the engine a ticket whose name disagrees with its contents, and
    /// this is a rebuild that hashed to something other than the name it minted — which can only
    /// happen if the one road and the digest have drifted apart, and is precisely the drift that
    /// would make a resumed `gx escalation` operate on a ticket that never existed.
    ///
    /// `created_at` comes from the journalled `Verdict` record's own `at`, so the rebuilt ticket
    /// carries the moment the gate answered rather than the moment somebody resumed. ASM-4 keeps it
    /// out of the identity, so a missing record cannot move the id — it can only leave the field at
    /// the epoch, and that case is the one where no verdict was journalled at all.
    ///
    /// # Errors
    /// [`Error::Canon`] via gx-gate if the ticket has no canonical form,
    /// [`Error::InconsistentTicket`] if the rebuild does not hash to the id it was minted under.
    fn rebuilt_ticket(&self, id: &TransformationId) -> Result<Option<EscalationTicket>> {
        // gx-gate's refusal has one cause here — the ticket's projection has no canonical form —
        // and it is spelled as this crate's `Malformed` rather than given a `From`, for the reason
        // E-M3-3 gives: a new `From` on the engine's enum would silently widen what a gate failure
        // can look like from every call site at once.
        let ticket = gx_gate::escalation_ticket(*id).map_err(|e| Error::Malformed {
            detail: format!("the escalation ticket for {id:?} has no canonical form: {e}"),
        })?;
        let at = self.journalled_verdict_at(id).unwrap_or(Timestamp(0));
        self.checked_ticket(ticket, at).map(Some)
    }

    /// When the journal says the gate answered about this transformation.
    ///
    /// The last such record, because 43 T-4a's determinism makes a second one a re-evaluation and
    /// the ticket an operator is holding is the one the latest verdict raised.
    fn journalled_verdict_at(&self, id: &TransformationId) -> Option<Timestamp> {
        self.journal
            .records()
            .iter()
            .rev()
            .find_map(|record| match record {
                EngineJournalRecord::Verdict {
                    transformation, at, ..
                } if transformation == id => Some(*at),
                _ => None,
            })
    }

    /// 🔴 **M6-04 採(a)** — 44 §1.2's `<TICKET_ID>`, resolved to the transformation it names.
    ///
    /// > 43 T-4c 逐語「ticket idは`TransformationId`に1:1紐付け」
    ///
    /// The declaration was 1:1 and the mapping was one-directional: [`Engine::ticket`] answers
    /// 「which ticket does this transformation have」 and nothing answered the question 44 §1.2's
    /// command line actually asks. This is the inverse, and it is computed from **Σ** rather than
    /// from the in-flight table, so it works in the single-shot process that is the whole of
    /// req/88 §3 Λ2: a fresh `gx escalation approve <TICKET_ID>` has planned nothing and holds no
    /// row, and it still resolves.
    ///
    /// # 🔴 It is a scan, and the cost is written down rather than hidden
    ///
    /// One rebuild per `Escalated` row, so `O(e)` in the number of open escalations rather than
    /// `O(n)` in the ledger — 43's INV-L2 makes every escalation resolve in finite time, so `e` is
    /// the count of what is genuinely awaiting a person. M5H7-3's disease is the other shape (a
    /// full-table walk per call), and the `.gx/index/` cache M6-04(b) describes is available if
    /// this ever measures badly. It is not built today: a cache in front of a function nobody has
    /// measured is the thing M5H8-15 refused.
    ///
    /// # Errors
    /// Anything [`Engine::rebuilt_ticket`] refuses, unchanged — a rebuild that does not hash to its
    /// own name is a fact about this build, not a 「not found」.
    pub fn transformation_of_ticket(&self, ticket: &TicketId) -> Result<Option<TransformationId>> {
        for row in reconstruct(self.journal.records()).transformations() {
            if row.verdict != Some(VerdictKind::Escalate) {
                continue;
            }
            if let Some(rebuilt) = self.rebuilt_ticket(&row.transformation)? {
                if rebuilt.id == *ticket {
                    return Ok(Some(row.transformation));
                }
            }
        }
        Ok(None)
    }

    /// Issue ASM-14's `VerdictReceipt` and file it on the row (**M5H4-6**).
    ///
    /// 42 §3.10's three obligations for the kind are met by construction rather than by a check:
    /// `inclusion_proof` and `postcondition_fingerprint` and `inverse_delta` are all `None`,
    /// because nothing has been appended, nothing has been applied and nothing has been escrowed.
    /// `Receipt::issue` runs `check_schema` before signing, so a hand that changed one of them
    /// would get an [`Error::Witness`] rather than a signed impossible receipt.
    ///
    /// `verdict` is an `Option` and `None` is 43 T-4e's — the case **E-M5-11** exists for.
    /// `canonical_cid` is `id.0`, which is what 42 §3.10 asks for (「`Transformation.id`」) and what
    /// `verify_offline` compares against `transformation`; T-8 has not run, and the value it will
    /// fix is the same one, because `id` is outside the IdentityView (42 §1.3, ASM-4).
    ///
    /// # Errors
    /// [`Error::Witness`] if the payload violates ASM-14 or the signature cannot be made.
    fn issue_verdict_receipt(
        &mut self,
        id: &TransformationId,
        verdict: Option<VerdictSummary>,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<()> {
        let Some(entry) = self.table.get(id) else {
            return Ok(());
        };
        let payload_kind = verdict.as_ref().map(|v| v.kind);
        let payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict,
            enforced: entry.enforced,
            receipt_kind: ReceiptKind::VerdictReceipt,
            canonical_cid: id.0,
            inverse_delta: None,
            transformation: *id,
            inclusion_proof: None,
            fail_posture_engaged: entry.fail_posture_engaged,
            precondition_fingerprint: FingerprintBytes(entry.fp0.digest().0),
            postcondition_fingerprint: None,
        };
        let receipt = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
            action: "issue the verdict receipt",
            detail: e.to_string(),
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.verdict_receipts.push(receipt);
            // 🔴 **FR-M04**: the counter is incremented **here**, where the receipt is issued,
            // rather than where the journal record is written. The two are one row apart on every
            // road, and keeping them apart is what lets `tests/ac_vc.rs` recount from the journal
            // and have that be a second opinion instead of a restatement.
            //
            // `None` is 43 T-4e, and it gets the fourth bucket rather than being folded into
            // `admit`: no gate ran, so nothing admitted (M4H4-2, the third application).
            match payload_kind {
                Some(VerdictKind::Admit) => self.verdicts.admit += 1,
                Some(VerdictKind::Deny) => self.verdicts.deny += 1,
                Some(VerdictKind::Escalate) => self.verdicts.escalate += 1,
                None => self.verdicts.unverdicted += 1,
            }
        }
        Ok(())
    }

    /// T-4d and T-4e: the collector could not be reached, and the posture decides which.
    ///
    /// The **only** place `AbortReason::VerifierUnavailable` is written in this workspace
    /// (**M5-03 採(a)**, **E-M5-4**).
    fn unreachable_collector(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Lifecycle> {
        match self.posture {
            // T-4d. 43's guard: 「`FailPosture = FailClosed`（DR-2既定・全substrate）」.
            FailPosture::FailClosed => {
                self.journal.append(EngineJournalRecord::Aborted {
                    transformation: *id,
                    reason: AbortReason::VerifierUnavailable,
                    rollback: None,
                    at,
                })?;
                Ok(self.set_state(id, Lifecycle::Aborted(AbortReason::VerifierUnavailable), at))
            }
            // T-4e. 43: 「当該Transformationに限りrecord-onlyモード相当へ降格して続行；`enforced=false`
            // と`fail_posture_engaged=true`を必ずreceiptに刻む」.
            //
            // 🔴 **The receipt is issued here, and hand 6 is the first hand that can issue it.**
            // Until **E-M5-11** the payload had a required `VerdictSummary` and no gate had run, so
            // the only shapes available were a minted empty digest (§32 M4H4-2, refused twice) or
            // no receipt at all -- and 43 T-4e says 「必ず…刻む」. The `None` below is the erratum
            // paying out on the exact transition it was ruled for.
            FailPosture::FailOpen => {
                self.journal.append(EngineJournalRecord::Verdict {
                    transformation: *id,
                    kind: VerdictKind::Admit,
                    verdict_digest: None,
                    fail_posture_engaged: true,
                    at,
                })?;
                if let Some(entry) = self.table.get_mut(id) {
                    entry.verdict = None;
                    entry.verdict_digest = None;
                    entry.enforced = false;
                    entry.fail_posture_engaged = true;
                }
                let state = self.set_state(id, Lifecycle::Admitted, at);
                self.issue_verdict_receipt(id, None, at, key)?;
                Ok(state)
            }
        }
    }

    // -----------------------------------------------------------------------
    // T-6 -- the deadlines, and who evaluates them
    // -----------------------------------------------------------------------

    /// **T-6**, for one transformation: abort it if its deadline has passed.
    ///
    /// > **M5-10 採(a)+(b) 併用**: lazy TTL 評価(liveness)+明示的 `reap(now)` API(掃き)
    ///
    /// 43 T-6's idempotency column is 「reaperは同一idに対し一度のみ発火（journal存在チェックで冪等）」,
    /// and this shape answers it without a check: after the abort the state is `Aborted`, which
    /// [`Engine::deadline_of`] gives no deadline, so a second call finds nothing due. The 「journal
    /// 存在チェック」 is the state table being a function of the journal (則 1) rather than a second
    /// query.
    ///
    /// Answers whether it fired, which is what makes [`Engine::reap`] able to report a count rather
    /// than a promise.
    ///
    /// # Errors
    /// [`Error::Io`] from the journal.
    fn expire_if_due(&mut self, id: &TransformationId, now: Timestamp) -> Result<bool> {
        let due = self
            .table
            .get(id)
            .and_then(|entry| self.deadline_of(entry))
            .is_some_and(|deadline| now.0 >= deadline.0);
        if !due {
            return Ok(false);
        }
        self.abort(id, AbortReason::Expired, None, now)?;
        Ok(true)
    }

    /// 🔴 **T-6** as a sweep: expire everything whose deadline has passed (**M5-10 採(b)**).
    ///
    /// The half of M5-10 that lazy evaluation cannot do. INV-L1 and INV-L2 are about *every*
    /// `Candidate`/`Verifying`/`Escalated` reaching a terminal state in finite time, and a
    /// transformation nobody calls an entry point about would otherwise wait forever with a
    /// deadline nothing evaluated. 43 T-6 names no trigger — 「reaperは同一idに対し一度のみ発火」 is
    /// the whole of its idempotency column and nothing says who runs it — and v0.1 has no resident
    /// process to run one (`gx serve` is M6, req/78 N-01). So the trigger is a call, and M6's
    /// server is one of the callers it will have.
    ///
    /// Answers the transformations it expired, in `TransformationId` order.
    ///
    /// # Errors
    /// [`Error::Io`] from the journal. A sweep that cannot write stops at the first refusal rather
    /// than continuing: a partially journalled sweep would leave the table and the log disagreeing
    /// about which rows had expired.
    pub fn reap(&mut self, now: Timestamp) -> Result<Vec<TransformationId>> {
        let candidates: Vec<TransformationId> = self.table.keys().copied().collect();
        let mut expired = Vec::new();
        for id in candidates {
            if self.expire_if_due(&id, now)? {
                expired.push(id);
            }
        }
        Ok(expired)
    }

    // -----------------------------------------------------------------------
    // 43 §8 -- waiting, and the annotation that is not a state
    // -----------------------------------------------------------------------

    /// 43 §8: is an in-flight transformation on the same `Subject` in conflict with this one?
    ///
    /// > `Commutation::Conflicts{residual}` → engineは`T2`を`Candidate`または`Verifying`のまま
    /// > 待機キューに保持する（新たな状態は追加しない。`blocked_by: TransformationId`という
    /// > 内部注釈のみ）
    ///
    /// # What 「先行」 means here, and why the definition matters
    ///
    /// 43 T-3's guard is 「同一Subjectに対する`Conflicts`中の**先行**Transformationがない」, and a
    /// definition is needed or two `Candidate`s block each other forever — a deadlock that would
    /// satisfy INV-L4 only because the TTL eventually kills both. So 先行 is **「has already passed
    /// T-3」**: a transformation still at `Draft` or `Candidate` has not started verifying and
    /// blocks nobody. That makes the waiting a queue with an order rather than a symmetric refusal.
    ///
    /// A `Denied` blocks under `RecordOnly` and not under `Enforce`, because 43 §1 makes `Denied`
    /// terminal 「ただしrecord-onlyモード時のみ§3の例外分岐でCanonicalizedへ進む」 — under
    /// `RecordOnly` it is still going to apply something. 🔴 `mode` is an **argument** since M6
    /// hand 3 (**M6-08 採(a)**): the caller is [`Engine::verify`], whose own mode may be a per-call
    /// override of the engine's, and a helper that read `self.mode` would answer this question
    /// under a posture the request did not ask for.
    ///
    /// **M4H6-4** is why the answer may be asked of the adapter each time rather than cached:
    /// 「独立性は delta の性質・状態遷移は engine の仕事」, so `commutation` is a function of the two
    /// deltas and calling it again cannot change its mind. req/78 §3.2 Λ8 is the same statement.
    ///
    /// # Errors
    /// [`Error::NotFound`] if no adapter is registered, [`Error::Adapter`] if `commutation` refuses.
    fn conflicting_predecessor(
        &self,
        id: &TransformationId,
        mode: EnforcementMode,
    ) -> Result<Option<TransformationId>> {
        let Some(this) = self.table.get(id) else {
            return Ok(None);
        };
        let adapter = self
            .adapters
            .get(this.delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", this.delta.substrate()),
            })?
            .adapter
            .clone();

        // 🔴 **M6-07 採(b)** — the rows on this subject, not every row this process has seen.
        //
        // What changed is the **search** and not the answer: the loop below was `for (other_id,
        // other) in &self.table` with the subject comparison as its first `continue`, so the set it
        // reaches is identical and the order is identical (`BTreeSet<TransformationId>` iterates in
        // the same order `BTreeMap<TransformationId, _>` does). What is gone is walking `n` rows to
        // find `k`. `tests/subject_index.rs` asserts the equality against a full scan, and `req/95`
        // carries the before/after measurement §47 M6-07 採(b) ordered.
        //
        // `self.by_subject` is read into a `Vec` first because the loop borrows `self.table`
        // immutably while `adapter` is already cloned out — the same reason the old loop could not
        // call anything on `&mut self` either.
        let siblings = self
            .by_subject
            .get(&this.transformation.subject)
            .map(|ids| ids.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for other_id in &siblings {
            if other_id == id {
                continue;
            }
            let Some(other) = self.table.get(other_id) else {
                continue;
            };
            debug_assert_eq!(
                other.transformation.subject, this.transformation.subject,
                "the subject index put a row under a subject that is not its own"
            );
            let in_flight = match other.state {
                Lifecycle::Draft | Lifecycle::Candidate => continue,
                Lifecycle::Denied => mode == EnforcementMode::RecordOnly,
                state => !state.is_terminal(),
            };
            if !in_flight {
                continue;
            }
            let answer = adapter
                .commutation(&other.delta, &this.delta)
                .map_err(|e| Error::Adapter {
                    action: "commutation",
                    detail: e.to_string(),
                })?;
            if matches!(answer, Commutation::Conflicts { .. }) {
                return Ok(Some(*other_id));
            }
        }
        Ok(None)
    }

    // -----------------------------------------------------------------------
    // T-5, T-5b
    // -----------------------------------------------------------------------

    /// **T-5 / T-5b** `escalation` — a person answers, and the answer is signed (AC-071, AC-072).
    ///
    /// 43 T-5's row: from `Escalated`, guard 「裁定者が有効な署名鍵を保持」, side effect 「人間裁定
    /// receipt（署名済み）をprovenance鎖に追記；journal: `HumanDecision{id, Admit}`」, to `Admitted`.
    /// T-5b is the same row with `Deny` and `Denied`. The guard is the `key` argument: an engine
    /// that took no key could not have issued the receipt the row requires, so 「holds a valid
    /// signing key」 is a type rather than a check.
    ///
    /// # INV-S6 is what this function is for
    ///
    /// > `Escalated`はT-5/T-5bの署名済み人間裁定receiptを経由せずに`Admitted`/`Denied`へ自動遷移しない
    ///
    /// There is no other road out of `Escalated` except T-6's expiry and T-7's cancel, and both
    /// land in `Aborted`. `tests/ac_071.rs` measures the absence from the other side.
    ///
    /// # Three refusals, and each one is a fact that would otherwise be invented
    ///
    /// * **not `Escalated`** — [`Error::InvalidState`]. 43 has no `Escalated → Escalated` edge and
    ///   no human ruling on anything else.
    /// * **`decision = Escalate`** — 42 §3.13: 「kindはAdmit|Denyのみ」. A person escalating an
    ///   escalation is a request for a state 43 §1 does not have.
    /// * **an empty reason** — 44 §1.2's trigger is `--reason <text>` and AC-071/072 both require
    ///   the reason to reach the trail. `Verdict::deny` refuses an empty `Vec<Reason>` in gx-gate
    ///   for the same reason: a refusal that says nothing is a refusal nobody can audit.
    ///
    /// # Errors
    /// [`Error::InvalidState`] for the first two above, [`Error::Malformed`] for the third,
    /// [`Error::NotFound`] for an unknown id, [`Error::Canon`] if the ruling has no canonical form,
    /// [`Error::Io`] from the journal, [`Error::Witness`] if the receipt cannot be issued.
    pub fn escalation(
        &mut self,
        id: &TransformationId,
        ruling: &HumanRuling,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Lifecycle> {
        // 43 T-6 first, so that INV-L2's deadline is not overtaken by a late ruling. Which of the
        // two wins is not written in 43; the engine takes the earlier one, because the expiry
        // already happened at the moment the deadline passed and this call is later.
        self.expire_if_due(id, at)?;

        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        if entry.state != Lifecycle::Escalated {
            return Err(Error::InvalidState {
                id: format!("{id:?}"),
                state: entry.state.name(),
                attempted: "escalation",
            });
        }
        let to = match ruling.decision {
            VerdictKind::Admit => Lifecycle::Admitted,
            VerdictKind::Deny => Lifecycle::Denied,
            VerdictKind::Escalate => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: "Escalated",
                    attempted: "escalation(Escalate); 42 §3.13 admits Admit and Deny only",
                })
            }
        };
        if ruling.reason.trim().is_empty() {
            return Err(Error::Malformed {
                detail: "a human ruling with no reason cannot be audited (44 §1.2's `--reason`, \
                         AC-071/072); `Verdict::deny` refuses the same emptiness in gx-gate"
                    .to_string(),
            });
        }

        // The digest of what the person decided -- see [`HumanRuling`] for why it is of this value
        // and not of the ticket the gate raised.
        let proof_digest = cid::compute(ruling)?;

        // Journal-first (43 §7). The receipt is the external side effect and follows.
        self.journal.append(EngineJournalRecord::HumanDecision {
            transformation: *id,
            kind: ruling.decision,
            reason: ruling.reason.clone(),
            actor: ruling.actor.clone(),
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.verdict = Some(ruling.decision);
            entry.verdict_digest = Some(proof_digest);
        }
        let state = self.set_state(id, to, at);
        self.issue_verdict_receipt(
            id,
            Some(VerdictSummary {
                kind: ruling.decision,
                proof_digest,
            }),
            at,
            key,
        )?;
        Ok(state)
    }

    // -----------------------------------------------------------------------
    // T-7
    // -----------------------------------------------------------------------

    /// **T-7** `cancel` — the owner stops it before the critical section (AC-073, DR-11, FR-059).
    ///
    /// 43 T-7's from-set is `{Draft, Candidate, Verifying, Admitted, Canonicalized, Escalated}`,
    /// its guard is 「actorがオーナー権限（Actor::Human{key}相当）を保持；`Committing`到達前」, and its
    /// idempotency column is 「二重キャンセルは無効操作として無視（既にAborted）」.
    ///
    /// # 🔴 `Draft` is in 43's from-set and is not reachable here
    ///
    /// A draft has no `TransformationId` (43 T-1, **E-M5-3**) and `Aborted` is keyed on one, so
    /// there is no record this engine could write about cancelling a draft — and no row to move,
    /// because **M5-17 採(b)** keeps the draft phase in the journal alone. Cancelling a draft is
    /// therefore **unrepresentable in v0.1**, and it is written down rather than quietly dropped:
    /// raised as **M5H6-1**. The cost is bounded — a draft holds no `PlannedDelta`, has read
    /// nothing and will expire from nothing, because 43 T-6 does not reach `Draft` either.
    ///
    /// # 🔴 The owner-permission guard has no enforcement point, and that is stated rather than faked
    ///
    /// 43 T-7 requires the actor to hold owner permission. v0.1 has no authorization layer: 44's
    /// API surface is M6 (req/78 N-01), the `Aborted` record has no actor field, and nothing in the
    /// engine knows who owns a transformation. Taking an `Actor` argument and dropping it would be
    /// worse than not taking one — a value the caller supplied and nothing recorded. So the guard
    /// is **unenforced**, this sentence is the disclosure §37 asks for when a check is absent
    /// (「「検査の不在」を doc に 1 行(隠さない)」), and **M5H6-4** is the ticket.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown id. [`Error::InvalidState`] from `Committing`,
    /// `Committed`, `Superseded` and from `Denied` (which 43 T-7's from-set does not include).
    /// [`Error::Io`] from the journal.
    pub fn cancel(&mut self, id: &TransformationId, at: Timestamp) -> Result<Lifecycle> {
        self.expire_if_due(id, at)?;

        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;
        match entry.state {
            // 43 T-7's idempotency column. No second record: a journal that grew on every repeated
            // cancel would report re-entries as events (T-1 and T-8 take the same early return).
            Lifecycle::Aborted(reason) => return Ok(Lifecycle::Aborted(reason)),
            Lifecycle::Candidate
            | Lifecycle::Verifying
            | Lifecycle::Admitted
            | Lifecycle::Canonicalized
            | Lifecycle::Escalated => {}
            state => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: state.name(),
                    attempted: "cancel",
                })
            }
        }
        // No rollback question: T-7 cannot fire from `Committing`, so nothing has been escrowed and
        // nothing applied (see `Rollback` for why `None` and `Some(NotAttempted)` differ).
        self.abort(id, AbortReason::OwnerCancelled, None, at)
    }

    /// 🔴 **The `Committed` row a second process does not have** (M6 hand 4's finding, M6H4-4).
    ///
    /// M6H3-1 named the hole one state earlier — 「`gx verify`/`gx commit` は別 process で row を
    /// 持たない」 — and answered it with 「body は再 plan・state は journal」. That answer **stops at
    /// `Committed`**, and `gx undo <TID>` is the verb that walks into the wall: a re-plan reads the
    /// substrate, a successful commit has *moved* the substrate, so the recomputed `TransformationId`
    /// is a different one and [`Engine::plan`] refuses (correctly). 43 §5 nonetheless makes `undo` an
    /// operation on a **`Committed`** transformation, so 44 §1.2's `gx undo` is unreachable from a
    /// single-shot CLI without this.
    ///
    /// # 🔴 Nothing is invented and **the journal's vocabulary does not grow**
    ///
    /// Every field of the row comes from Σ, from the blob store, or from the caller's `Intent` — the
    /// draft `.gx/drafts/` is holding, which is where 44 §0's id-resolution already sends a CLI:
    ///
    /// | field | where it comes from |
    /// |---|---|
    /// | `subject` | **`Provenance.input_objects[0]`** — 42 §3.9's list is 「adapterが読み取った入力スナップショット群」 and in v0.1 the engine watches the adapter read exactly one, T-2's, whose `ObjectId` *is* the subject [`Engine::derive_provenance`] |
    /// | `intent_id`, `delta_cid`, `fp0`, `superseded_by` | the `StateRow` |
    /// | `parents`, `locator`, `created_at` | the `Planned` record (**E-M5-13**'s two fields do the work they were added for) |
    /// | `context`, `actor`, `substrate` | the `Intent` the caller passes |
    /// | the delta body | the blob store (**E-M4-8**: keeping it is what makes replay and undo constructible at all) |
    ///
    /// # 🔴 The rebuild proves itself
    ///
    /// A reconstruction that guessed would be worse than none, so the rebuilt `Transformation` is
    /// **re-identified**: its CID is computed and compared with the id that was asked for, and a
    /// disagreement is [`Error::InvalidState`] rather than a row. Content addressing makes that a
    /// proof rather than a check — 42 §1.3 puts every field of the identity view into the digest, so
    /// a rebuild that hashes to the recorded name differs from the original in nothing that matters.
    ///
    /// `pre` is the one field that is **not** the historical value, and it is not fabricated either:
    /// it is a fresh `adapter.snapshot(locator)`, which is what an `ObjectSnapshot` is defined to be
    /// (「The object as it is now」). ASM-9 does not store content, so the snapshot T-2 took is gone
    /// and its *digest* survives in `fp0`. Nothing reads a committed row's `pre` except `undo`, which
    /// reads its locator; a rebuilt row that pretended to hold the old snapshot would be the lie this
    /// avoids.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the transformation has no adapter, no delta body, or (for a committed
    /// row) no provenance — the last is a journal written by something that is not this code.
    /// [`Error::InvalidState`] if the rebuild does not re-identify. [`Error::Io`] from the blobs.
    pub fn rehydrate_committed(&mut self, id: &TransformationId, intent: &Intent) -> Result<bool> {
        if self.table.contains_key(id) {
            return Ok(true);
        }
        let sigma = reconstruct(self.journal.records());
        let Some(row) = sigma.state_of(id).cloned() else {
            return Ok(false);
        };
        if !matches!(
            row.state,
            Some(Lifecycle::Committed | Lifecycle::Superseded)
        ) {
            return Ok(false);
        }

        let missing = |what: &'static str| Error::NotFound {
            what,
            id: format!("{id:?}"),
        };
        let provenance = row.provenance.clone().ok_or_else(|| {
            missing("provenance for a committed transformation (42 §3.9, M5-25 採(a))")
        })?;
        let subject = *provenance
            .input_objects
            .first()
            .ok_or_else(|| missing("the subject snapshot in the provenance record"))?;
        let intent_id = row.intent_id.ok_or_else(|| missing("intent id"))?;
        let delta_cid = row
            .delta_cid
            .ok_or_else(|| missing("planned delta reference"))?;
        let delta = self.blobs.get(&delta_cid)?;
        let fp0 = row
            .fp0
            .clone()
            .ok_or_else(|| missing("precondition fingerprint"))?
            .into_fingerprint()?;
        let (locator, parents, created_at) = self
            .planned_record(id)
            .ok_or_else(|| missing("the Planned record"))?;

        let mut transformation = Transformation::new(
            TransformationId(Cid([0u8; 32])),
            0,
            Subject::Object(subject),
            None,
            parents,
            CompositionMetadata {
                intent_id,
                delta: delta.reference().clone(),
                context: intent.context().clone(),
                actor: intent.actor().clone(),
                created_at,
            },
        )?;
        let rebuilt = TransformationId(cid::compute(&transformation)?);
        if rebuilt != *id {
            return Err(Error::InvalidState {
                id: format!("{id:?}"),
                state: "Committed",
                attempted: "rehydrate: the rebuilt transformation names another id, so the intent \
                            supplied is not the one this transformation was planned from",
            });
        }
        transformation.id = *id;

        let adapter = self
            .adapters
            .get(delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", delta.substrate()),
            })?
            .adapter
            .clone();
        let pre = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;

        self.seat(
            *id,
            Entry {
                intent_id,
                transformation,
                state: row.state.unwrap_or(Lifecycle::Committed),
                since: created_at,
                blocked_by: None,
                delta,
                fp0,
                pre,
                verdict: row.verdict,
                verdict_digest: row.verdict_digest,
                enforced: row.enforced,
                fail_posture_engaged: row.fail_posture_engaged,
                canonical_cid: row.canonical_cid,
                ticket: None,
                admit_proof: None,
                deny_reasons: None,
                verdict_receipts: Vec::new(),
                superseded_by: row.superseded_by,
                apply_started: row.apply_started,
                rollback: row.rollback,
                provenance: Some(provenance),
                inverse_cid: None,
                applied_at: None,
                receipt: None,
            },
        );
        // 42 §3.12's row, verbatim from Σ. `status` matters as much as the CID: a `Consumed` inverse
        // is what stops a second undo of one commit, and a rebuild that reset it to `Available`
        // would make 「一度だけ」 false across a restart.
        if let Some(escrow) = sigma
            .escrow()
            .iter()
            .find(|e| e.transformation == *id)
            .cloned()
        {
            self.escrow.insert(*id, escrow);
        }
        // T-12's idempotency column reads this index, and Σ carries the edge.
        if let Some(by) = row.superseded_by {
            self.supersedes.record(*id, by);
        }
        Ok(true)
    }

    /// The `Planned` record's locator, parents and moment (**E-M5-13**).
    ///
    /// Not in `StateRow`: Σ's reconstruction keeps what a *state* is made of, and these three are
    /// facts about the planning event. The last such record wins, for [`Engine::journalled_verdict_at`]'s
    /// reason.
    fn planned_record(
        &self,
        id: &TransformationId,
    ) -> Option<(String, Vec<TransformationId>, Timestamp)> {
        self.journal
            .records()
            .iter()
            .rev()
            .find_map(|record| match record {
                EngineJournalRecord::Planned {
                    transformation,
                    locator,
                    parents,
                    at,
                    ..
                } if transformation == id => Some((locator.clone(), parents.clone(), *at)),
                _ => None,
            })
    }

    // -----------------------------------------------------------------------
    // T-12 -- undo
    // -----------------------------------------------------------------------

    /// **T-12, first half** `undo` — build the transformation that will undo a committed one.
    ///
    /// 43 §5 is unambiguous about what this is and is not:
    ///
    /// > 1. Undoは新規`submit(intent)`から始まる**通常のTransformation**（T_u）である…
    /// > 2. T_uは自身の`Draft→Candidate→Verifying→...→Committed`を独立に経る（undoであっても検証を
    /// >    免除されない — fail-closed・P-4は適用され続ける）
    ///
    /// So this function **does not undo anything**. It creates a `Candidate`, and the caller then
    /// drives [`Engine::verify`], [`Engine::canonicalize`] and [`Engine::commit`] exactly as for any
    /// other transformation — which is what AC-040's second case measures, where a policy denies
    /// the undo and `T_o` stays `Committed`. The supersede edge is drawn by that commit, not by
    /// this call: see [`Engine::supersede_after_commit`].
    ///
    /// # 🔴 Why it is not literally 「新規`submit(intent)`」
    ///
    /// 43 §5-1 says T_u's intent 「means 『T_oのescrowed inverse（T-10bで既にescrow済み）を適用する』」,
    /// and 41 §4 gives no way to write that intent down: `plan(intent, pre)` is the adapter's
    /// function from a goal to a delta, and there is no goal that an arbitrary adapter is
    /// guaranteed to plan into *this particular* escrowed delta. An engine that called `plan` and
    /// hoped would be undoing something else on the day the two disagreed.
    ///
    /// So the escrowed delta is used **directly** as T_u's delta, and the `Intent` is minted for
    /// its identity alone (`IntentId` is the CID of all five fields, 42 §1.3): same substrate, same
    /// locator, `GoalBytes` = the inverse's payload, and T_o's context and actor. Both journal
    /// records 43 §5-1 implies are written — `DraftCreated` then `Planned` — so a replay sees a
    /// transformation that began normally, because it did. What is skipped is `adapter.plan`, and
    /// the skip is raised as **M5H6-5**.
    ///
    /// `Fingerprint₀` is taken **now**, against a fresh snapshot: the undo's precondition is the
    /// world as `T_o` left it, not the world `T_o` was planned against. Without that, T-10a's CAS
    /// would refuse every undo.
    ///
    /// # Errors
    /// [`Error::NotFound`] if `original` is unknown, if it escrowed no inverse, if the inverse's
    /// body is not in the blob store, or if its substrate has no registered adapter.
    /// [`Error::InvalidState`] if `original` is not `Committed`, or if its inverse has already been
    /// consumed by another undo (42 §3.12's `Consumed`). [`Error::Adapter`] from `snapshot` or
    /// `precondition`. [`Error::Canon`], [`Error::Core`] and [`Error::Io`] as `plan` raises them.
    pub fn undo(
        &mut self,
        original: &TransformationId,
        rng_seed: u64,
        at: Timestamp,
    ) -> Result<(IntentId, TransformationId)> {
        let entry = self.table.get(original).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{original:?}"),
        })?;
        if entry.state != Lifecycle::Committed {
            return Err(Error::InvalidState {
                id: format!("{original:?}"),
                state: entry.state.name(),
                attempted: "undo",
            });
        }
        let subject = entry.transformation.subject;
        let context = entry.transformation.context.clone();
        let actor = entry.transformation.actor.clone();
        let locator = entry.pre.locator().to_string();
        let substrate = entry.delta.substrate().clone();

        let row = self.escrow.get(original).ok_or_else(|| Error::NotFound {
            what: "escrowed inverse",
            id: format!("{original:?}"),
        })?;
        // 42 §3.12's `Consumed` is what makes 「一度だけ」 a fact rather than a hope: a second undo
        // of the same commit would be a second transformation claiming the same inverse.
        if matches!(row.status, InverseStatus::Consumed { .. }) {
            return Err(Error::InvalidState {
                id: format!("{original:?}"),
                state: "Superseded",
                attempted: "undo an inverse another transformation already consumed",
            });
        }
        let inverse_cid = row.inverse_cid.ok_or_else(|| Error::NotFound {
            what: "escrowed inverse (42 §3.12 Unavailable: `invert` answered None)",
            id: format!("{original:?}"),
        })?;
        let inverse = self.blobs.get(&inverse_cid)?;

        let adapter = self
            .adapters
            .get(&substrate)
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{substrate:?}"),
            })?
            .adapter
            .clone();

        // T-1. The intent exists for its identity: 42 §1.3 row 2 puts all five fields in the CID,
        // so two undos of one commit mint one `IntentId` and the second is answered without a
        // second record (43 T-1's create-if-absent, unchanged).
        let intent = Intent::new(
            substrate,
            locator.clone(),
            GoalBytes(inverse.payload().to_vec()),
            context.clone(),
            actor.clone(),
        );
        let intent_id = IntentId(cid::compute(&intent)?);
        if !self.drafted.contains_key(&intent_id) {
            self.journal.append(EngineJournalRecord::DraftCreated {
                intent_id,
                rng_seed,
                at,
            })?;
            self.drafted.insert(intent_id, rng_seed);
        }

        // T-2, with the adapter asked only for what it alone knows: the world as it is now.
        let pre = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        let fp0 = adapter.precondition(&pre).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;

        let mut transformation = Transformation::new(
            TransformationId(Cid([0u8; 32])),
            0,
            subject,
            None,
            // 43 T-12's guard: 「`T_u.parents`が`T_o.id`を含み」. This is where that becomes true,
            // and it is also C-2's provenance edge: the undo names what it undoes.
            vec![*original],
            CompositionMetadata {
                intent_id,
                delta: inverse.reference().clone(),
                context,
                actor,
                created_at: at,
            },
        )?;
        let id = TransformationId(cid::compute(&transformation)?);
        transformation.id = id;

        self.journal.append(EngineJournalRecord::Planned {
            transformation: id,
            intent_id,
            locator: pre.locator().to_string(),
            delta_cid: inverse_cid,
            fp0: FingerprintRecord::of(&fp0),
            // 🔴 **E-M5-13**'s reason, on the one path that has one: T-12's guard is
            // 「`T_u.parents`が`T_o.id`を含み」, and this is where the list stops being in-memory
            // only. A crash between here and `Committed` used to lose the edge (M5H6-6's window,
            // which the `M5H6_6` probe measured); now the journal carries it.
            parents: transformation.parents.clone(),
            at,
        })?;
        self.resolved.insert(intent_id, id);
        // **M4H6-3** on the live path for the second time: the body is already filed under this
        // CID, so the store answers `AlreadyPresent` and writes nothing. 「保管の一度性」 is what
        // makes an undo's delta the *same* blob as the commit's escrowed inverse rather than a copy.
        self.blobs.put(&inverse)?;

        self.seat(
            id,
            Entry {
                intent_id,
                transformation,
                state: Lifecycle::Candidate,
                since: at,
                blocked_by: None,
                delta: inverse,
                fp0,
                pre,
                verdict: None,
                verdict_digest: None,
                enforced: true,
                fail_posture_engaged: false,
                canonical_cid: None,
                ticket: None,
                admit_proof: None,
                deny_reasons: None,
                verdict_receipts: Vec::new(),
                superseded_by: None,
                apply_started: None,
                rollback: None,
                provenance: None,
                inverse_cid: None,
                applied_at: None,
                receipt: None,
            },
        );
        Ok((intent_id, id))
    }

    /// **T-12, second half** — draw the supersede edge, once, when an inverse reaches `Committed`.
    ///
    /// 43 T-12: from `Committed(T_o)`, trigger 「別Transformation`T_u`が`Committed`へ到達し、
    /// `T_u.delta == T_o`のescrowed inverse」, guard 「`T_u.parents`が`T_o.id`を含み、`T_u`の`Subject`
    /// が`T_o`と一致」, side effect 「`T_o`のメタデータに`superseded_by = T_u.id`を追記（journal:
    /// `Superseded{T_o.id, by: T_u.id}`）。**`T_o`のcanonical record・receiptは不変のまま**」.
    ///
    /// Every clause is a line below, and the matching is **M5-09 採(a)**'s: 「T-12 の照合は escrow の
    /// `inverse_delta` CID と `T_u.delta` CID の一致」. Three facts move together (M5-16 採(a): 「1
    /// 箇所」) — `T_o`'s state, the [`SupersedeIndex`] entry, and 42 §3.12's `InverseStatus` — and
    /// this is the only place any of them is written.
    ///
    /// # What is deliberately *not* written
    ///
    /// `T_o`'s canonical record, its receipt and its ledger entry. INV-S2 and P-5 (「取消は新規commit
    /// であり書換ではない」) are the whole of AC-044, and the way to satisfy them is to touch none of
    /// the three — which is why the row's `receipt` and `transformation` fields do not appear here.
    ///
    /// # Errors
    /// [`Error::Io`] from the journal.
    fn supersede_after_commit(
        &mut self,
        t_u: &TransformationId,
        at: Timestamp,
    ) -> Result<Option<TransformationId>> {
        let Some(entry) = self.table.get(t_u) else {
            return Ok(None);
        };
        let delta_cid = entry.delta.reference().cid;
        let subject = entry.transformation.subject;
        let parents = entry.transformation.parents.clone();

        let mut found = None;
        for parent in parents {
            let Some(row) = self.escrow.get(&parent) else {
                continue;
            };
            if row.inverse_cid != Some(delta_cid) || !matches!(row.status, InverseStatus::Available)
            {
                continue;
            }
            let Some(original) = self.table.get(&parent) else {
                continue;
            };
            if original.state != Lifecycle::Committed
                || original.transformation.subject != subject
                // 43 T-12's idempotency column: 「`superseded_by`が既に設定済みなら再設定しない」.
                || self.supersedes.superseded_by(&parent).is_some()
            {
                continue;
            }
            found = Some(parent);
            break;
        }
        let Some(t_o) = found else {
            return Ok(None);
        };

        // Journal-first, then the three facts.
        self.journal.append(EngineJournalRecord::Superseded {
            transformation: t_o,
            by: *t_u,
            at,
        })?;
        self.supersedes.record(t_o, *t_u);
        if let Some(row) = self.escrow.get_mut(&t_o) {
            row.status = InverseStatus::Consumed { by: *t_u };
        }
        if let Some(entry) = self.table.get_mut(&t_o) {
            entry.superseded_by = Some(*t_u);
        }
        self.set_state(&t_o, Lifecycle::Superseded, at);
        Ok(Some(t_o))
    }

    // -----------------------------------------------------------------------
    // T-8, T-8r
    // -----------------------------------------------------------------------

    /// **T-8 / T-8r** `canonicalize` — check T3, fix the canonical CID, record `enforced`.
    ///
    /// 43 T-8's side effects are 「canonical CID確定；`canon(canon(x))=canon(x)`確認（T3）」 and its
    /// from-state is `Admitted`. T-8r adds `Denied` under `EnforcementMode::RecordOnly`, with
    /// 「`enforced=false`フラグをTransformation付随メタデータに刻印」.
    ///
    /// The idempotence check runs **before** anything is written, which is what AC-033's abnormal
    /// case asks for: 「エラーを返しCanonicalizedへ遷移しない」. A refusal leaves the state where it
    /// was and the journal untouched, so a caller can look at the transformation afterwards and a
    /// replay never sees a canonicalisation that did not happen.
    ///
    /// # 🔴 `enforced = Some(false)` is reachable from T-8 as well as T-8r
    ///
    /// 42 §3.13 annotates the record 「T-8rのみenforced=Some(false)」. That is narrower than 43 §4,
    /// which degrades a **T-4e** transformation to 「record-onlyモード相当」 while leaving it
    /// `Admitted` -- so it reaches canonicalisation through T-8, carrying `enforced=false`. Writing
    /// `None` there to satisfy 42's parenthetical would hide exactly the fact INV-S5 requires to be
    /// visible (「`enforced=false`のCommittedは…区別可能な形でreceiptに刻まれる」). The flag follows
    /// the transformation, not the transition. Raised as **M5H2-3**.
    ///
    /// # 🔴 `mode` is **E-M6-20**'s argument, and it is the shape M6-08 already ruled
    ///
    /// `None` means 「whatever this engine was opened in」 and `Some(..)` overrides it **for this
    /// call**. 44 §2.2's commit body grew a `record_only` field under E-M6-20 (req/38 §52, 「E-M6-10
    /// の HTTP 版・[DR-2感度] 段落の実行可能化」), and a long-lived server cannot express it any other
    /// way: [`Engine::with_mode`] is a builder that consumes `self` at `open` time, and the
    /// alternative — 「serve が request ごとに `&mut self` で mode を差し替える」 — is the form §47
    /// M6-08 ruled 「**採ってはならない**」 because a posture written onto shared state leaks into the
    /// next request, and a leaked `RecordOnly` is a fail-open. [`Engine::verify`] took the same
    /// argument for the same reason one hand earlier; this is its other half, because DR-2's
    /// 「Denyでも適用するか」 is decided at **T-8r**, not at T-4.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown id. [`Error::InvalidState`] for any state that is not
    /// `Admitted`, and for `Denied` when the effective mode is `Enforce` (where 43 §1 makes `Denied`
    /// terminal). [`Error::NotIdempotent`] when the canonical form is not a fixed point.
    /// [`Error::Canon`] if the transformation has no canonical form. [`Error::Io`] from the journal.
    pub fn canonicalize(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        mode: Option<EnforcementMode>,
    ) -> Result<Lifecycle> {
        // 43 T-6 reaches `Candidate`/`Verifying`/`Escalated` and not `Admitted`/`Denied`, so this
        // call fires nothing today. It is here because M5-10 採(a) is 「状態を…進める時に TTL を評価」
        // and an entry point that skipped the evaluation would be relying on the from-state list
        // never widening.
        self.expire_if_due(id, at)?;
        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;

        // 43 T-8's idempotency column: 「canonicalizeは冪等（T3）。再計算しても同一canon_cid」. The
        // honest form of 「同一」 is 「the same one, not recomputed」 -- an early return, so a second
        // call writes no second `Canonicalized` record. Same shape as T-1's create-if-absent, and
        // for the same reason: a journal that grew on every re-entry would report re-entries as
        // events, which is right for `VerifyStarted` (a real second attempt) and wrong here (a
        // caller asking again for a value that is already fixed).
        if entry.state == Lifecycle::Canonicalized {
            return Ok(Lifecycle::Canonicalized);
        }

        let record_only = mode.unwrap_or(self.mode) == EnforcementMode::RecordOnly;
        match entry.state {
            Lifecycle::Admitted => {}
            Lifecycle::Denied if record_only => {}
            state => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: state.name(),
                    attempted: "canonicalize",
                })
            }
        }

        let bytes = self.canon.canonical_form(&entry.transformation)?;
        if !cbor::is_canonical(&bytes) {
            return Err(Error::NotIdempotent {
                transformation: *id,
                detail: format!(
                    "canon produced {} bytes that gx-canon would not have written, so \
                     canon(canon(x)) != canon(x) (42 §2.3, 12 F0 T3)",
                    bytes.len()
                ),
            });
        }

        // The identity is gx-canon's, always, whatever the canonicalizer above is (41 §6).
        let canonical_cid = cid::compute(&entry.transformation)?;

        // T-8r's flag, and T-4e's -- see the section above.
        let enforced = if entry.state == Lifecycle::Denied {
            Some(false)
        } else if entry.enforced {
            None
        } else {
            Some(false)
        };

        self.journal.append(EngineJournalRecord::Canonicalized {
            transformation: *id,
            canonical_cid,
            enforced,
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.canonical_cid = Some(canonical_cid);
            if enforced == Some(false) {
                entry.enforced = false;
            }
        }
        Ok(self.set_state(id, Lifecycle::Canonicalized, at))
    }

    // -----------------------------------------------------------------------
    // T-9, T-10a, T-10b, T-10c, T-11 -- the commit critical section
    // -----------------------------------------------------------------------

    /// **T-9 → T-10a/T-10b/T-10c → T-11** `commit` — the critical section 43 §1 calls `Committing`.
    ///
    /// One entry point for five transitions, for the reason [`Engine::verify`] is one for six: 43
    /// gives them one trigger, and the branches are what two collaborators answered. The order of
    /// the statements below **is** 41 §5's protocol, and every one of them is journalled before the
    /// side effect it describes.
    ///
    /// | step | 43 | what it does |
    /// |---|---|---|
    /// | T-9 | `commit_start` | `CommittingStarted`, then the state moves. 「**side-effect実行前に必ずjournal化**」 |
    /// | — | **M5-25 採(a)** | `ProvenanceDerived` — before the world moves, so a crash cannot lose it |
    /// | T-10a | CAS | `Fingerprint₁ := adapter.precondition(now)`, compared with `Fingerprint₀` |
    /// | T-10b | escrow | `adapter.invert`, `InverseEscrowed`, then the body into the blob store |
    /// | — | **E-M5-1** | `ApplyStarted`, then the **one** call to `adapter.apply` |
    /// | T-10c | apply failed | best-effort rollback, then `Aborted(ApplyFailed)` with what happened |
    /// | T-11 | apply succeeded | `ledger.append` → `InclusionProof` → receipt → `Committed` |
    ///
    /// # 🔴 The CAS has three answers and only two of them are transitions (**M5-24 採(a)**)
    ///
    /// `Fingerprint::cas_eq` returns `Result<bool>` because 42 §3.5's comparison has three answers,
    /// and **E-M4-15 / E-M4-27** made the third one an `Err`: two fingerprints from different
    /// adapters, or over different scopes, cannot be compared at all. §37 rules where it goes:
    ///
    /// > **M5-24 採(a)**: `cas_eq` の `Err` は `Aborted(InternalError)`(43 T-13 逐語一致・T-13 を踏む
    /// > 1 本目の経路=M5-14 と同時に閉じる)。doc 1 行義務。
    ///
    /// **Here is that line**: an `Err` from the CAS is `Aborted(InternalError)` and never
    /// `PreconditionChanged`. The difference is the whole value of the `Result`: `PreconditionChanged`
    /// says 「someone else moved the world」 and `InternalError` says 「this deployment is wired
    /// wrong」, and folding the second into the first would retire a bug as a business condition —
    /// the same mistake **E-M4-32** refused for `Ok(None)`. It is also the **first road to T-13**,
    /// which 51 §14's branch coverage needs (M5-14).
    ///
    /// # 🔴 Journal-first has exactly one exception, and 43 §7-3b is the price paid for it
    ///
    /// Every record above is written before its side effect. `Committed` is not, and cannot be: it
    /// carries `ledger_seq`, which does not exist until `ledger.append` has answered. 43 T-11's own
    /// cell writes the journal last for that reason, and §7-3b is the recovery that exists because
    /// of it — 「ledgerに該当entryが存在する場合 → commitはクラッシュ前に完了していた。journalに
    /// `Committed`エントリが欠落しているだけ」. The exception and its compensation are one design,
    /// and naming it here is what keeps a later hand from "fixing" the ordering.
    ///
    /// # What is refused rather than invented
    ///
    /// A T-4e degraded admission has **no verdict** — the gate was never asked — and 42 §3.10's
    /// `ReceiptPayload.verdict` has no way to say so. The engine refuses with
    /// [`Error::Unrepresentable`] **before** T-9 opens, so nothing is journalled for a commit that
    /// cannot be completed. Raised as **M5H4-3**.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown id or an unregistered adapter. [`Error::InvalidState`]
    /// from any state that is not `Canonicalized`. [`Error::Unrepresentable`] for the T-4e case
    /// above. [`Error::Adapter`] if `snapshot`, `precondition` or `invert` refuses — note that a
    /// refusal from **`apply`** is not an error here, it is T-10c and comes back in the `Ok`.
    /// [`Error::Ledger`] and [`Error::Witness`] from T-11's two collaborators. [`Error::Io`] from
    /// the journal or the blob store.
    pub fn commit(
        &mut self,
        id: &TransformationId,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Lifecycle> {
        self.expire_if_due(id, at)?;
        let entry = self.table.get(id).ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: format!("{id:?}"),
        })?;

        // 43 T-9's idempotency column: 「二重commit_start要求は既にCommittingなら無視」, and 43 §1
        // makes `Committed` terminal. Both are answered without writing anything, which is what
        // 「無視」 means in a journal: a second request that appended a second `CommittingStarted`
        // would report a re-entry as an event. Resuming an interrupted critical section is 43
        // §7-3's recovery and hand 5's; in this hand a `Committing` row means the call above is
        // still on the stack.
        match entry.state {
            Lifecycle::Committed => return Ok(Lifecycle::Committed),
            Lifecycle::Committing => return Ok(Lifecycle::Committing),
            Lifecycle::Canonicalized => {}
            state => {
                return Err(Error::InvalidState {
                    id: format!("{id:?}"),
                    state: state.name(),
                    attempted: "commit",
                })
            }
        }

        // Everything the receipt needs, resolved before the section opens (see the note above).
        let canonical_cid = entry.canonical_cid.ok_or_else(|| Error::InvalidState {
            id: format!("{id:?}"),
            state: entry.state.name(),
            attempted: "commit",
        })?;
        // 🔴 **E-M5-11**. Hand 4 refused every degraded admission here, because 42 §3.10 required a
        // `VerdictSummary` and 43 T-4e has none; §41 made the seat an `Option` and the refusal
        // moved rather than vanishing. What is refused now is the shape that would be **untrue**:
        // no verdict and no reason for there being none. `Error::Unrepresentable` keeps a producer,
        // and it is the honest one — a commit with neither a verdict nor an engaged fail-open
        // posture is a receipt that says a change was allowed and cannot say by what.
        let verdict = match (entry.verdict, entry.verdict_digest) {
            (Some(kind), Some(proof_digest)) => Some(VerdictSummary { kind, proof_digest }),
            (None, None) if entry.fail_posture_engaged => None,
            _ => {
                return Err(Error::Unrepresentable {
                    what: "a CommitReceipt with no verdict and no engaged fail-open posture",
                    detail: format!(
                        "{id:?} has verdict={:?} digest={:?} fail_posture_engaged={}; 43 T-4e is \
                         the one road to a commit without a verdict and it sets the flag, so this \
                         row is a half-filled pair rather than a degraded admission",
                        entry.verdict, entry.verdict_digest, entry.fail_posture_engaged
                    ),
                })
            }
        };

        let adapter = self
            .adapters
            .get(entry.delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", entry.delta.substrate()),
            })?
            .adapter
            .clone();
        let delta = entry.delta.clone();
        let pre = entry.pre.clone();
        let fp0 = entry.fp0.clone();
        let locator = pre.locator().to_string();

        // --- T-9, journal-first ------------------------------------------------------------
        self.journal
            .append(EngineJournalRecord::CommittingStarted {
                transformation: *id,
                at,
            })?;
        self.set_state(id, Lifecycle::Committing, at);

        // --- M5-25 採(a): the provenance, before anything can be lost --------------------------
        let provenance = self.derive_provenance(id, &pre);
        self.journal
            .append(EngineJournalRecord::ProvenanceDerived {
                transformation: *id,
                provenance: provenance.clone(),
                at,
            })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.provenance = Some(provenance);
        }

        // --- T-10a: CAS ----------------------------------------------------------------------
        // 43 §7's 「`Fingerprint₁ := adapter.precondition(now)`」. Two calls, because 41 §4's
        // `precondition` takes a snapshot rather than a locator: 「now」 is a **fresh** snapshot,
        // and reusing T-2's would make the comparison a value against itself.
        let fresh = adapter.snapshot(&locator).map_err(|e| Error::Adapter {
            action: "snapshot",
            detail: e.to_string(),
        })?;
        let fp1 = adapter.precondition(&fresh).map_err(|e| Error::Adapter {
            action: "precondition",
            detail: e.to_string(),
        })?;
        match fp0.cas_eq(&fp1) {
            // INV-S7: 「いかなる場合も`adapter.apply`は呼ばれない」. The return is the enforcement --
            // there is no path from here to the call below.
            Ok(false) => return self.abort(id, AbortReason::PreconditionChanged, None, at),
            // M5-24 採(a). See the section above.
            Err(_) => return self.abort(id, AbortReason::InternalError, None, at),
            Ok(true) => {}
        }

        // --- T-10b: escrow the inverse, before the world moves --------------------------------
        let inverse = adapter.invert(&delta, &pre).map_err(|e| Error::Adapter {
            action: "invert",
            detail: e.to_string(),
        })?;
        // 🔴 **E-M5-9**. Both arms journal, and that is the whole erratum: 43 T-10b's guard is
        // 「inverse構成可能（`Some`）」 and hand 4 wrote nothing at all when the answer was `None`,
        // because 42 §3.13 typed the record's CID as required. The `None` arm is **reachable now**
        // — E-M3-4 escalates a transformation whose `invert` answers `None`, and T-5 is what lets a
        // person approve one — so 「we asked and there is none」 has to be a record. Without it a
        // replay would find no escrow row for the commit and report the undo guarantee as
        // 「never asked」, which is §32 M4H4-2's refusal in the log rather than in a type.
        let escrowed = match inverse {
            Some(inverse) => {
                let inverse_cid = inverse.reference().cid;
                self.journal.append(EngineJournalRecord::InverseEscrowed {
                    transformation: *id,
                    inverse_cid: Some(inverse_cid),
                    at,
                })?;
                // 43 T-10b: 「既にescrow済みなら再書込みしない（冪等）」 -- which the store answers
                // by content addressing rather than by a flag (`PutOutcome::AlreadyPresent`).
                self.blobs.put(&inverse)?;
                self.escrow.insert(
                    *id,
                    EscrowRow {
                        transformation: *id,
                        inverse_cid: Some(inverse_cid),
                        // DR-9: the OSS default is 無期限, and the journal has no seat for a
                        // deadline anyway (M5H3-3).
                        retained_until: None,
                        status: InverseStatus::Available,
                    },
                );
                if let Some(entry) = self.table.get_mut(id) {
                    entry.inverse_cid = Some(inverse_cid);
                }
                Some((inverse_cid, inverse))
            }
            None => {
                self.journal.append(EngineJournalRecord::InverseEscrowed {
                    transformation: *id,
                    inverse_cid: None,
                    at,
                })?;
                self.escrow.insert(
                    *id,
                    EscrowRow {
                        transformation: *id,
                        inverse_cid: None,
                        retained_until: None,
                        // 42 §3.12: 「`invert()`がNoneを返した場合（構成不能）」.
                        status: InverseStatus::Unavailable,
                    },
                );
                None
            }
        };

        // --- E-M5-1, then the one call that changes the world ---------------------------------
        self.journal.append(EngineJournalRecord::ApplyStarted {
            transformation: *id,
            delta_cid: delta.reference().cid,
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.apply_started = Some(delta.reference().cid);
        }
        let applied = match self.apply_once(adapter.as_ref(), &delta) {
            Ok(applied) => applied,
            // --- T-10c ---------------------------------------------------------------------
            Err(_) => {
                let rollback = match &escrowed {
                    Some((inverse_cid, inverse)) => {
                        // The rollback **is** an apply, so it gets the record every apply gets: a
                        // crash inside it must not look like a crash before it. This is also what
                        // keeps 則 2 honest -- one call site, reached twice.
                        self.journal.append(EngineJournalRecord::ApplyStarted {
                            transformation: *id,
                            delta_cid: *inverse_cid,
                            at,
                        })?;
                        if let Some(entry) = self.table.get_mut(id) {
                            entry.apply_started = Some(*inverse_cid);
                        }
                        // 43 T-10c: 「ベストエフォート、結果に関わらず次へ」 -- the outcome is
                        // recorded and does not change the reason.
                        match self.apply_once(adapter.as_ref(), inverse) {
                            Ok(_) => Rollback::Succeeded,
                            Err(_) => Rollback::Failed,
                        }
                    }
                    None => Rollback::NotAttempted,
                };
                return self.abort(id, AbortReason::ApplyFailed, Some(rollback), at);
            }
        };

        // 🔴 **E-M4-31 / M5-18 採(a)**: the moment is the engine's. `AppliedDelta` has four
        // accessors and no setter, so the value is **rebuilt** rather than mutated -- which is the
        // ruling's own form (「gx-substrate に 1 行も足さない・engine 側 1 箇所」). req/78 §4 M5-18
        // writes the call as `AppliedDelta::new(*d.delta(), ...)`; `DeltaRef` is not `Copy` (it
        // holds a `SubstrateKind`, which holds a `String`), so it is cloned.
        let applied = AppliedDelta::new(
            applied.delta().clone(),
            applied.postcondition().clone(),
            *applied.resulting_digest(),
            at,
        );

        // --- T-11 ------------------------------------------------------------------------------
        let mut payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict,
            enforced: self.table[id].enforced,
            receipt_kind: ReceiptKind::CommitReceipt,
            canonical_cid,
            inverse_delta: escrowed.as_ref().map(|(cid, _)| *cid),
            transformation: *id,
            // Filled below. `ReceiptPayload::ledger_digest` clears it in any case -- see there for
            // the circularity in 42 §3.11 that makes the leaf a digest of the payload without its
            // own proof.
            inclusion_proof: None,
            fail_posture_engaged: self.table[id].fail_posture_engaged,
            precondition_fingerprint: FingerprintBytes(fp0.digest().0),
            postcondition_fingerprint: Some(FingerprintBytes(applied.postcondition().digest().0)),
        };
        let receipt_digest = payload.ledger_digest().map_err(|e| Error::Witness {
            action: "digest the receipt payload",
            detail: e.to_string(),
        })?;
        let outcome = self
            .ledger
            .append(*id, receipt_digest, at)
            .map_err(|e| Error::Ledger {
                action: "append",
                detail: e.to_string(),
            })?;
        let leaf = outcome.entry().index;
        let proof: InclusionProof =
            prove_inclusion(self.ledger.log(), leaf).map_err(|e| Error::Ledger {
                action: "prove inclusion",
                detail: e.to_string(),
            })?;
        payload.inclusion_proof = Some(proof);
        let receipt = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
            action: "issue the receipt",
            detail: e.to_string(),
        })?;

        self.journal.append(EngineJournalRecord::Committed {
            transformation: *id,
            ledger_seq: leaf,
            at,
        })?;
        self.committed.insert(*id, leaf);
        if let Some(entry) = self.table.get_mut(id) {
            entry.applied_at = Some(applied.applied_at());
            entry.receipt = Some(receipt);
        }
        let state = self.set_state(id, Lifecycle::Committed, at);
        // 🔴 **T-12**, from the other side: this commit may be the inverse of an earlier one. The
        // edge is drawn **after** `Committed` because 43 T-12's trigger is 「別Transformation`T_u`が
        // `Committed`へ到達し」 -- a transformation that has not committed supersedes nothing. A
        // crash in between leaves `T_u` committed and `T_o` still `Committed`, which is the window
        // 規律48's intermediate probe stops in (`tests/supersede.rs`).
        self.supersede_after_commit(id, at)?;
        Ok(state)
    }

    /// 🔴 **43 §7's recovery**, run after a restart (AC-043, 51 §8.1).
    ///
    /// 43 §7 writes it as three numbered steps and this function is those steps:
    ///
    /// 1. **§7-1** — 「`EngineJournal`を…末尾まで順に replay し、各`TransformationId`について最後に
    ///    記録された遷移を復元する」. [`Engine::open`] read the file; [`crate::replay::reconstruct`]
    ///    turns those records into Σ, and Σ's rows *are* 「最後に記録された遷移」.
    /// 2. **§7-2** — a terminal last record is rebuilt and nothing is re-run. For `Committed` that
    ///    means restoring Σ's ledger component (the frontier [`Engine::ledger_agrees`] compares), and
    ///    for the aborts it means nothing at all.
    /// 3. **§7-3** — a `CommittingStarted` with no terminal record after it is a crash **inside the
    ///    critical section**, and the exactly-once judgement runs.
    ///
    /// # 🔴 The judgement has two questions, not one, and the second is E-M5-1's
    ///
    /// 43 §7-3 asks the ledger and branches on the answer. Under **E-M5-1** the engine asks a second
    /// question first — 「was the adapter asked to apply」 — because req/78 §3.2 Λ4 is a three-line
    /// proof that the one-question version breaks:
    ///
    /// > crash が「apply 成功後・`ledger.append` 前」に起きたとする…復旧手順 3c は…
    /// > `Fingerprint₁ := adapter.precondition(now)` を再計算し…ところが **apply は既に成功している
    /// > ので `Fingerprint₁ ≠ Fingerprint₀`**…substrate は変更済み・Transformation は Aborted…
    /// > =「適用されたのに記録が無い」
    ///
    /// The recovery **never re-runs the CAS**. Where an `ApplyStarted` exists the comparison would
    /// be against the engine's own footprint, and where it does not exist there is nothing to
    /// compare *for* — the world did not move, so no apply may follow. 「自分の partial apply を外来
    /// 干渉と誤認しない」 is therefore structural here: `cas_eq` is not called in this function, and
    /// `tests/crash_recovery.rs` measures both halves (a source scan, and the shim of Λ4's mistaken
    /// recovery run against the same bytes).
    ///
    /// # What 43 §7-3c cannot do in v0.1, and it is an input rather than an intention
    ///
    /// §7-3c says 「T-10a以降の手順…を**最初から再実行**」, and T-10a needs
    /// `adapter.precondition(now)` — which needs a **fresh snapshot**, which needs a **locator**.
    /// The journal does not hold one. It holds `Fingerprint₀`, whose `scope` is 42 §3.5's 「adapter
    /// 定義のスコープ識別子」 and is explicitly allowed to be **wider** than the object (「単一
    /// `ObjectSnapshot.digest`より広い範囲をカバーしてよい」), so reading a locator out of it would
    /// be the engine parsing an adapter's grammar. ∴ a `Committing` row with **no** `ApplyStarted`
    /// and **no** ledger entry is folded to `Aborted(InternalError)`: nothing was applied (so P-4
    /// and INV-S4 hold), and INV-L3 forbids leaving it `Committing`. Raised as **M5H5-2**.
    ///
    /// # 🔴 Both roads re-apply, and 43 §7-3b did not expect to
    ///
    /// §7-3b reads as though the ledger entry alone finishes the job: 「既存の`InclusionProof`から
    /// receiptを（未発行なら）再発行し」. Re-issuing needs the **payload**, and 42 §3.10's payload
    /// carries `postcondition_fingerprint` — a value produced by `apply` and recorded **nowhere**.
    /// The journal holds `Fingerprint₀` (in `Planned`) and no fingerprint of the result. So the
    /// recovery obtains it the only way v0.1 can: by applying again, which 41 §4 contracts to be
    /// idempotent and which 43 §7-3c already relies on. The cost is one write to a world that had
    /// already reached that state, and the alternative — a journal record carrying the postcondition
    /// — is a change to 42 §3.13 that this hand raises rather than makes (**M5H5-3**).
    ///
    /// # What recovery needs, measured (**M5H3-5**)
    ///
    /// The four inputs are the journal, the blob store, the ledger and a registered adapter — plus
    /// the signing key, which is why this is a call and not part of [`Engine::open`]: 41 §6 injects
    /// the adapter and the key **after** the engine exists, and an `open` that recovered would have
    /// to recover before either arrived. What it does **not** need is the state table:
    /// `Transformation` bodies, `ObjectSnapshot`s and in-memory `PlannedDelta`s are all absent after
    /// a restart and none of them is read here. That is M5H3-5's measurement, and it is why `open`
    /// still rebuilds only the draft phase. Raised as **M5H5-1**.
    ///
    /// # 🔴 What a recovery still does not do: T-12 (hand 6)
    ///
    /// A crash between `T_u`'s `Committed` record and its `Superseded` record leaves the supersede
    /// edge undrawn, and this function does not draw it. It cannot check 43 T-12's guard: 「`T_u.
    /// parents`が`T_o.id`を含み」 is a fact about the `Transformation` body, and the journal holds
    /// names and digests rather than bodies (ASM-9) -- the state table a recovery works from has no
    /// `parents`. Firing T-12 on the escrow CID match **alone** would be dropping half the guard,
    /// which is the sort of shortcut §32 M4H4-2 keeps refusing. So the window is left open,
    /// measured (`tests/supersede.rs`), and raised as **M5H6-6**.
    ///
    /// # Errors
    /// [`Error::NotFound`] when no adapter is registered for a delta's substrate.
    /// [`Error::Unrepresentable`] for a `Committing` row with no verdict and no engaged fail-open
    /// posture. [`Error::Io`] from the journal or the blob store, [`Error::Ledger`] and
    /// [`Error::Witness`] from T-11's two collaborators.
    pub fn recover(&mut self, at: Timestamp, key: &KeyPair) -> Result<Vec<Recovered>> {
        let sigma = reconstruct(self.journal.records());
        let escrowed: BTreeMap<TransformationId, Cid> = sigma
            .escrow()
            .iter()
            .filter_map(|e| e.inverse_cid.map(|cid| (e.transformation, cid)))
            .collect();
        let committed: BTreeMap<TransformationId, u64> = sigma
            .ledger()
            .iter()
            .map(|c| (c.transformation, c.ledger_seq))
            .collect();
        let rows: Vec<StateRow> = sigma.transformations().to_vec();

        let mut out = Vec::new();
        for row in rows {
            match row.state {
                // §7-2, and the one terminal that leaves a trace in Σ.
                Some(Lifecycle::Committed) => {
                    let seq = committed.get(&row.transformation).copied();
                    if let Some(seq) = seq {
                        self.committed.insert(row.transformation, seq);
                    }
                    out.push(Recovered {
                        transformation: row.transformation,
                        path: RecoveryPath::Terminal,
                        state: Lifecycle::Committed,
                        ledger_seq: seq,
                        appended: None,
                        payload_matched: None,
                        receipt: None,
                    });
                }
                // §7-3.
                Some(Lifecycle::Committing) => out.push(self.resume(
                    &row,
                    escrowed.get(&row.transformation).copied(),
                    at,
                    key,
                )?),
                // Every other last record is either terminal with nothing to restore (§7-2) or an
                // in-flight state outside the critical section, which 43 §7 does not resume.
                _ => {}
            }
        }
        Ok(out)
    }

    /// 43 §7-3 for one transformation. See [`Engine::recover`] for the two questions it asks.
    fn resume(
        &mut self,
        row: &StateRow,
        inverse_cid: Option<Cid>,
        at: Timestamp,
        key: &KeyPair,
    ) -> Result<Recovered> {
        let id = row.transformation;
        let refused = |state: Lifecycle| Recovered {
            transformation: id,
            path: RecoveryPath::NothingWasApplied,
            state,
            ledger_seq: None,
            appended: None,
            payload_matched: None,
            receipt: None,
        };

        // 43 §7-3a: 「`ledger`を`TransformationId`…で照会する」.
        let held = self
            .ledger
            .log()
            .entries()
            .iter()
            .find(|e| e.transformation == id)
            .cloned();

        // The two questions. Neither road below may be walked when both answers are 「no」: the
        // adapter was never asked and the ledger holds nothing, so the world is as `plan` left it.
        if held.is_none() && row.apply_started.is_none() {
            let state = self.abort(&id, AbortReason::InternalError, None, at)?;
            return Ok(refused(state));
        }

        // Everything the receipt needs, from the journal and the blob store alone.
        let Some(delta_cid) = row.delta_cid else {
            // A journal trimmed past its own `Planned` record (42 §5) cannot name the delta.
            let state = self.abort(&id, AbortReason::InternalError, None, at)?;
            return Ok(refused(state));
        };
        let (Some(canonical_cid), Some(fp0)) = (row.canonical_cid, row.fp0.clone()) else {
            let state = self.abort(&id, AbortReason::InternalError, None, at)?;
            return Ok(refused(state));
        };
        // 🔴 **E-M5-11**, on the recovery's side of the door. Hand 4 refused a `Committing` row
        // with no verdict outright; since §41 the degraded admission of 43 T-4e is representable
        // and **reachable** (hand 6 commits one), so a crash inside its critical section has to be
        // recoverable too -- refusing here would make T-4e the one transition a restart could not
        // finish. What is still refused is the half-filled pair, for `commit`'s reason.
        let verdict =
            match (row.verdict, row.verdict_digest) {
                (Some(kind), Some(proof_digest)) => Some(VerdictSummary { kind, proof_digest }),
                (None, None) if row.fail_posture_engaged => None,
                _ => return Err(Error::Unrepresentable {
                    what:
                        "a CommitReceipt rebuilt with no verdict and no engaged fail-open posture",
                    detail: format!(
                        "{id:?} was in `Committing` with verdict={:?} digest={:?} \
                         fail_posture_engaged={}; the recovery has nothing true to put there",
                        row.verdict, row.verdict_digest, row.fail_posture_engaged
                    ),
                }),
            };
        let delta = self.blobs.get(&delta_cid)?;
        let adapter = self
            .adapters
            .get(delta.substrate())
            .ok_or_else(|| Error::NotFound {
                what: "adapter",
                id: format!("{:?}", delta.substrate()),
            })?
            .adapter
            .clone();

        // 43 §7-3c: 「`adapter.apply`はadapter契約上idempotentに設計されるため…再実行は安全である」.
        // Announced again for the reason it was announced the first time (E-M5-1): a crash inside
        // *this* call must be distinguishable from a crash before it.
        self.journal.append(EngineJournalRecord::ApplyStarted {
            transformation: id,
            delta_cid,
            at,
        })?;
        let applied = match self.apply_once(adapter.as_ref(), &delta) {
            Ok(applied) => applied,
            // The adapter refused an application it is contracted to accept twice (41 §4). The
            // rollback of T-10c is not attempted: the escrowed inverse restores the state the
            // snapshot was taken over, and applying it after a *successful* earlier apply would
            // undo a commit the ledger may already hold. Raised as **M5H5-5**.
            Err(_) => {
                let state = self.abort(
                    &id,
                    AbortReason::ApplyFailed,
                    Some(Rollback::NotAttempted),
                    at,
                )?;
                return Ok(Recovered {
                    transformation: id,
                    path: RecoveryPath::ApplyWasAnnounced,
                    state,
                    ledger_seq: None,
                    appended: None,
                    payload_matched: None,
                    receipt: None,
                });
            }
        };
        // E-M4-31 / M5-18 採(a), on the recovery's side of the door as well.
        let applied = AppliedDelta::new(
            applied.delta().clone(),
            applied.postcondition().clone(),
            *applied.resulting_digest(),
            at,
        );

        let mut payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict,
            enforced: row.enforced,
            receipt_kind: ReceiptKind::CommitReceipt,
            canonical_cid,
            inverse_delta: inverse_cid,
            transformation: id,
            inclusion_proof: None,
            fail_posture_engaged: row.fail_posture_engaged,
            precondition_fingerprint: FingerprintBytes(fp0.digest().0),
            postcondition_fingerprint: Some(FingerprintBytes(applied.postcondition().digest().0)),
        };
        let receipt_digest = payload.ledger_digest().map_err(|e| Error::Witness {
            action: "digest the rebuilt receipt payload",
            detail: e.to_string(),
        })?;

        let (leaf, appended, payload_matched, path) = match &held {
            // 43 §7-3b.
            Some(entry) => {
                let matched = entry.receipt_digest == receipt_digest;
                if !matched {
                    // The rebuild is not the thing the ledger witnessed, so re-issuing would put a
                    // second answer to 「what was committed」 into the world. Fail-closed.
                    let state = self.abort(&id, AbortReason::InternalError, None, at)?;
                    return Ok(Recovered {
                        transformation: id,
                        path: RecoveryPath::LedgerHeldTheCommit,
                        state,
                        ledger_seq: Some(entry.index),
                        appended: None,
                        payload_matched: Some(false),
                        receipt: None,
                    });
                }
                (
                    entry.index,
                    None,
                    Some(true),
                    RecoveryPath::LedgerHeldTheCommit,
                )
            }
            // 43 §7-3c. `ledger.append` is key-idempotent (ASM-43-1), so 「万一過去の試行が部分的に
            // ledgerへ到達していても二重entryは生じない」 is the collaborator's guarantee and the
            // outcome is *reported* rather than branched on.
            None => {
                let outcome =
                    self.ledger
                        .append(id, receipt_digest, at)
                        .map_err(|e| Error::Ledger {
                            action: "append",
                            detail: e.to_string(),
                        })?;
                let kind = match outcome {
                    gx_log::AppendOutcome::Appended(_) => "Appended",
                    gx_log::AppendOutcome::AlreadyPresent(_) => "AlreadyPresent",
                };
                (
                    outcome.entry().index,
                    Some(kind),
                    None,
                    RecoveryPath::ApplyWasAnnounced,
                )
            }
        };

        let proof: InclusionProof =
            prove_inclusion(self.ledger.log(), leaf).map_err(|e| Error::Ledger {
                action: "prove inclusion",
                detail: e.to_string(),
            })?;
        payload.inclusion_proof = Some(proof);
        let receipt = Receipt::issue(&payload, at, key).map_err(|e| Error::Witness {
            action: "re-issue the receipt",
            detail: e.to_string(),
        })?;

        self.journal.append(EngineJournalRecord::Committed {
            transformation: id,
            ledger_seq: leaf,
            at,
        })?;
        self.committed.insert(id, leaf);
        Ok(Recovered {
            transformation: id,
            path,
            state: Lifecycle::Committed,
            ledger_seq: Some(leaf),
            appended,
            payload_matched,
            receipt: Some(receipt),
        })
    }

    /// 🔴 **則 2**: the only place in this crate that asks a substrate to change (req/78 §3.3).
    ///
    /// > **則 2(`S` への道は 1 本)**: `adapter.apply` の呼び出し箇所は engine 全体で **1 箇所**で
    /// > なければならない
    ///
    /// Both of T-10c's applications go through it -- the forward delta and, when that fails, the
    /// escrowed inverse -- so the single road is a fact about the source and not about how many
    /// times it is walked. `tests/ac_035.rs` measures both halves: a scan that finds exactly one
    /// invocation line in `src/`, and a counting adapter that says how many times it was reached.
    ///
    /// The function is deliberately thin. Anything it did beyond calling and naming the failure
    /// would be work happening on the far side of the one door.
    fn apply_once(
        &self,
        adapter: &dyn SubstrateAdapter,
        delta: &PlannedDelta,
    ) -> Result<AppliedDelta> {
        adapter.apply(delta).map_err(|e| Error::Adapter {
            action: "apply",
            detail: e.to_string(),
        })
    }

    /// Journal an abort, move the row, and answer with the state (43 T-10a / T-10c / T-13).
    ///
    /// One helper so that 「what does an abort write」 has one answer, and so that `rollback` cannot
    /// be forgotten at a call site: the parameter is required and every caller has to say which of
    /// [`Rollback`]'s facts applies, `None` included.
    fn abort(
        &mut self,
        id: &TransformationId,
        reason: AbortReason,
        rollback: Option<Rollback>,
        at: Timestamp,
    ) -> Result<Lifecycle> {
        self.journal.append(EngineJournalRecord::Aborted {
            transformation: *id,
            reason,
            rollback,
            at,
        })?;
        if let Some(entry) = self.table.get_mut(id) {
            entry.rollback = rollback;
        }
        Ok(self.set_state(id, Lifecycle::Aborted(reason), at))
    }

    /// Derive 42 §3.9's `Provenance` for a transformation (**M5-25 採(a)**, D-7's third window).
    ///
    /// The engine is the producer because it is the only party that saw what the adapter read.
    /// `Provenance::derive_from` does the rest -- including deciding when `intent_digest` is `None`,
    /// which is gx-witness's judgement and not this crate's to re-take.
    ///
    /// # `input_objects` is one object in v0.1, and that is a measurement rather than a stub
    ///
    /// 42 §3.9 asks for 「plan/verify中にadapterが読み取った入力スナップショット群（`subject`以外の
    /// 副次入力を含む）」. In v0.1 the engine watches the adapter read **exactly one**: T-2's
    /// `adapter.snapshot(locator)`. `verify` reads nothing further (T-3's `invert` is handed the
    /// snapshot T-2 took), and 41 §4 gives an adapter no way to report a secondary read. So the
    /// list has one element, and the day an adapter reads two is the day 41 §4 needs a way to say
    /// so -- raised with the version question in **M5H4-4**.
    fn derive_provenance(&self, id: &TransformationId, pre: &ObjectSnapshot) -> Provenance {
        let entry = &self.table[id];
        let version = self
            .adapters
            .get(entry.delta.substrate())
            .map_or_else(String::new, |registered| registered.version.clone());
        Provenance::derive_from(
            &entry.transformation,
            ProvenanceInputs {
                input_objects: vec![*pre.id()],
                environment: Environment {
                    // ASM-10 (単一ノード運用) makes it omissible, and an engine that invented a
                    // hostname would be putting an unverifiable claim into a provenance record.
                    host_id: None,
                    adapter_kind: entry.delta.substrate().clone(),
                    // 42 §3.9's example is an MCP session id, which arrives with an API this
                    // milestone does not build (N-01).
                    correlation_id: None,
                    engine_version: env!("CARGO_PKG_VERSION").to_string(),
                    adapter_version: version,
                },
            },
        )
    }

    // -----------------------------------------------------------------------

    /// Move a row's state. The one mutation, so that a reader looking for 「who changes state」 finds
    /// one answer and every caller of it is a journalled transition above.
    ///
    /// 🔴 Takes the moment as well, since hand 6: 43 T-6 measures its two deadlines from 「the state
    /// was entered」, and a `since` maintained anywhere but here would be a second answer to when
    /// that was. The value is the `at` of the record that was just appended, so the deadline is a
    /// function of the journal even though the field is not in Σ.
    fn set_state(&mut self, id: &TransformationId, to: Lifecycle, at: Timestamp) -> Lifecycle {
        if let Some(entry) = self.table.get_mut(id) {
            entry.state = to;
            entry.since = at;
        }
        to
    }

    /// The `IntentId` a transformation came from.
    #[must_use]
    pub fn intent_of(&self, id: &TransformationId) -> Option<IntentId> {
        self.table.get(id).map(|e| e.intent_id)
    }

    /// 🔴 **M6-02 採(a)** — the `TransformationId` an intent was planned into, if it was.
    ///
    /// The inverse of [`Engine::intent_of`], and the thing 44 §0's id-resolution rule needs in order
    /// to exist at all: 「`gx plan`等、Draft/Candidateをまたいで対象を指定するコマンド・エンドポイント
    /// は`IntentId`と`TransformationId`のいずれの`gx1:...`値も受理し…`plan()`完了後は正準の
    /// `TransformationId`へ解決する」. Before this accessor a caller holding an `IntentId` could only
    /// walk [`Engine::transformation_ids`] comparing `intent_of` — the O(n) shape M5H7-3 identified
    /// as a measured decay rather than a theoretical one.
    ///
    /// # 🔴 The rule when the answer is not unique (req/88 §3 Λ3(ii))
    ///
    /// Resolution is a **partial** map, and re-planning can make one intent name more than one
    /// transformation. 43 §8 forces a re-plan when a predecessor commits, and [`Engine::plan`]
    /// permits it 「while the row is still where T-2 left it」 — so a second `plan` of the same intent
    /// against a moved world mints a second `TransformationId` while the first row is still in the
    /// table. **This accessor answers with the most recently planned one**, which is the last
    /// `Planned` record in journal order.
    ///
    /// Journal order rather than table order, and the difference is not cosmetic: the table is a
    /// `BTreeMap<TransformationId, _>`, so its order is CID order — content-addressed and therefore
    /// arbitrary with respect to time. 「The latest」 has to mean 「the latest thing that happened」,
    /// and the journal is the only structure that records happening. The same order is what
    /// [`Engine::open`] replays, so the answer after a restart is the answer before it.
    ///
    /// It answers `None` for a **draft** and that is E-M5-3 rather than an omission: before `plan`
    /// there is no `TransformationId` to resolve to (43 T-1: 「`TransformationId`はまだ確定しない」),
    /// which is why 44 L101's `gx cancel` from-set could not be satisfied by id-resolution and
    /// **E-M6-1** removed `Draft` from it instead (req/38 §47).
    #[must_use]
    pub fn resolved(&self, intent_id: &IntentId) -> Option<TransformationId> {
        self.resolved.get(intent_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declared list is the variants, in order (**E-M2-23 / A-10**).
    #[test]
    fn the_declared_states_are_the_variants_in_order() {
        let variants = [
            Lifecycle::Draft,
            Lifecycle::Candidate,
            Lifecycle::Verifying,
            Lifecycle::Admitted,
            Lifecycle::Denied,
            Lifecycle::Escalated,
            Lifecycle::Canonicalized,
            Lifecycle::Committing,
            Lifecycle::Committed,
            Lifecycle::Aborted(AbortReason::Expired),
            Lifecycle::Superseded,
        ];
        let names: Vec<&str> = variants.iter().map(Lifecycle::name).collect();
        assert_eq!(names, LIFECYCLE_STATES.to_vec());
    }

    /// 43 §1's terminal column, for the three that do not depend on a setting.
    #[test]
    fn the_terminal_states_are_the_ones_43_1_marks_terminal() {
        assert!(Lifecycle::Committed.is_terminal());
        assert!(Lifecycle::Superseded.is_terminal());
        assert!(Lifecycle::Aborted(AbortReason::Expired).is_terminal());
        // `Denied` is terminal 「ただしrecord-onlyモード時のみ」, so the answer needs a mode and this
        // function does not take one -- `canonicalize` is where the setting is read.
        assert!(!Lifecycle::Denied.is_terminal());
        assert!(!Lifecycle::Candidate.is_terminal());
    }
}
