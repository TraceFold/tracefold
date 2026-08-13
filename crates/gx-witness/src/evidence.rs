//! What the gate is shown: test results, measurements, external attestations, policy decisions.
//!
//! Spec: 42 §3.7 for the enum and its two auxiliary vocabularies, 42 §1.3 for what its CID is taken
//! over, 32 FR-016 for the requirement, 34 AC-016 for its test, 41 §4 for the `GateInput` slot these
//! values are handed to.
//!
//! # Four variants (E-M2-3)
//!
//! req/49 §3 M2-4 counted six places in 43/44/34/35 that ask for a fifth `Evidence(HumanDecision)`
//! and proposed implementing five. **E-M2-3** (`req/38_ERRATA_2026-08-07.md` §8) ruled the other
//! way, verbatim: 「Evidence=42 の 4 variant が正(43/44/34/35 の `HumanDecision` 参照は erratum・
//! DR-03-1 の HumanApprovalToken が対応物)」. So a human ruling is not evidence in gx's vocabulary —
//! 43 T-5's 「人間裁定receipt（署名済み）」 is a receipt, and DR-03-1's `HumanApprovalToken` is the
//! type that carries the approval itself. The four below are the whole enum.
//!
//! # A variant is a kind of evidence, not a verdict about it
//!
//! req/26 §11's 「rule=データ / エンジン=ロジックのみ」 lands here: this module names what was
//! observed and holds no rule for reading it. Whether a `TestOutcome::Fail` blocks a transformation
//! is a Cedar policy's business, in gx-gate (M3). Nothing in this file branches on a value.
//!
//! Nor is 42 §3.7's `TestOutcome` (four values) the same scale as the three-valued evidence field of
//! req/26 §11's rubric schema. req/49 §4 records that they are separate systems and must not be
//! merged; this note is the whole of that inheritance.
//!
//! # 🔴 45 TH-8's mitigation column overstates v0.1, and by how much (**M5H8-4**)
//!
//! 45 §2's TH-8 row lists 「無署名evidenceは証拠として不採用」 as a control. There is no such
//! control in this crate and none anywhere else: [`Evidence`] has four variants and **no signature
//! field** (42 §3.7), and neither gx-gate nor gx-engine has a road that refuses an unsigned one —
//! req/86 §5.2 measured both (`pub enum Evidence` and a grep over gate's source that returned
//! nothing). `req/38_ERRATA_2026-08-07.md` §45 rules the reading, verbatim:
//!
//! > **M5H8-4 採(a)+卓**: 45 §2 TH-8 緩和欄を実体に合わせる erratum=「evidence は receipt の DSSE
//! > 署名に**覆われる**(改竄は receipt 検証で顕在化)が、collector 署名の検証経路は v0.1 に無い」。
//! > (b) `Evidence` への署名 field 追加(42 §3.7 型変更)は **M6/M7 reqdef で再訪**(evidence の
//! > producer が増える時が設計窓)。残存等級「中〜高」は不変=文言だけが過大だった。
//!
//! The distinction is worth stating exactly, because the weaker true statement is still useful.
//! An evidence value's digest travels inside a `ReceiptPayload`, and that payload is what the DSSE
//! signature covers — so **evidence that was altered after the receipt was issued is detected**, by
//! whoever verifies the receipt. What v0.1 does not have is the other half: nothing says the
//! collector that produced the evidence signed it, and nothing verifies such a signature, so
//! evidence that was false **when it was collected** is admitted with the same standing as
//! evidence that was true. 45 §3 already grades the residual risk 「中〜高」 and is right as
//! written; the mitigation column is the part that was ahead of the code. `req/spec/` is unchanged
//! — the erratum lives in req/38 and here.
//!
//! # Every field is in the identity
//!
//! 42 §1.3's table: `Evidence`（各variant）→「全フィールド」, 除外なし, because 「Evidence自体は独立の
//! 証拠物であり除外規則なし」. So the projection is the value, and [`Evidence`]'s CID is
//! `gx_canon::cid::compute`'s: project, encode canonically, hash. There is no second road — this
//! crate names neither a codec nor a hash (41 §6), which `tests/evidence_cid.rs` checks by reading
//! the source.

use gx_canon::cid::IdentityView;
use gx_core::{Cid, KeyId, Subject};
use serde::{Deserialize, Serialize};

/// How a test case ended (42 §3.7: `Pass | Fail | Skip | Error`).
///
/// `Skip` and `Error` are not `Fail`. A skipped case was not run and an errored one did not reach a
/// verdict about the subject, so collapsing either into a failure would record a fact the test
/// framework never stated — and a policy that wants to treat them alike can, in gx-gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TestOutcome {
    Pass,
    Fail,
    Skip,
    Error,
}

/// What a policy engine answered (42 §3.7: `Allow | Deny`, 「`cedar_policy::Decision`と同一語彙」).
///
/// The same two words Cedar uses, so a decision crossing the boundary needs no translation table —
/// the reasoning that makes `KeyId` an alias of `String` in 42 §3.2 rather than a newtype.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny,
}

/// A reference to an in-toto Statement (42 §3.7).
///
/// 逐語: 「§5の保管方針に従い全文embed または digest-onlyのいずれか（`InTotoStatementRef { inline:
/// Option<serde_json::Value>, digest: Cid, uri: Option<String> }`）」. `digest` is not optional and
/// the other two are: whatever else is known, the statement is named by its digest, so an evidence
/// item whose body was dropped still says which body it was.
///
/// # `inline` and the float ban (req/49 §3 M2-13)
///
/// Every field of an `Evidence` is in its identity (42 §1.3) and 42 §2.1-4 keeps floats out of
/// canonical values, so an inline Statement carrying a number written with a decimal point has **no
/// CID**. This hand does not change that: both of req/49's 既定案 (fold `inline` to digest-only, or
/// write down an admitted numeric range) edit 42 §3.7's field table, which an implementation may not
/// do (52 契約). What it does instead is req/26 §3's 「範囲明示 + throw で正直に落ちる」 — the value
/// is refused by `gx_canon::Error::NotCanonicalizable(FloatNotAllowed)`, naming the clause, and
/// `tests/evidence_cid.rs` holds both sides of the boundary. Raised as H4-4 in req/53 §4.
///
/// A caller who has a Statement with floats in it has one road that works today, and it is the one
/// 42 §5 already offers: keep the body in the evidence store and carry `digest` alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InTotoStatementRef {
    pub uri: Option<String>,
    pub digest: Cid,
    /// The Statement itself, when 42 §5's retention policy keeps it.
    pub inline: Option<serde_json::Value>,
}

/// Something the gate is shown (42 §3.7).
///
/// # Field order inside each variant
///
/// Encoded-key order, as in [`crate::provenance`] and gx-log's `tile.rs`: shorter names first, then
/// bytewise. 42 §3.7 lists them in reading order; the set is 42's and the order is the encoder's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evidence {
    /// One test case (42 §3.7).
    TestResult {
        case: String,
        suite: String,
        outcome: TestOutcome,
        /// 42 §5 keeps raw logs outside the evidence value; this is set only when one was kept.
        log_digest: Option<Cid>,
        duration_ms: u64,
    },
    /// One measurement (42 §3.7).
    ///
    /// There is no numeric field, and that is the requirement rather than an omission: 逐語
    /// 「**測定値自体（`f64`）はEvidence CIDに直接埋め込まない**…ここはそのdigestのみを保持する
    /// （P-10: 観測量はprimitiveでなく、identity-bearingなCID体系に生のfloatを持ち込まない設計上の
    /// 選択）」. So 42 §2.1-4's float ban has nothing to catch in this variant — the type already
    /// made it unreachable.
    Measurement {
        /// What was measured.
        subject: Subject,
        /// The id of an `ObjectMeasure` / `MorphismMeasure` implementation (41 §3).
        measure_id: String,
        /// The digest of the recorded value, which lives in the evidence store (42 §5).
        value_digest: Cid,
    },
    /// An attestation somebody else signed (42 §3.7).
    ExternalAttestation {
        /// The external signer's key id.
        signer: KeyId,
        statement: InTotoStatementRef,
        /// A copy of the Statement's `predicateType`, carried for filtering.
        predicate_type: String,
    },
    /// One policy evaluation (42 §3.7).
    PolicyEvaluation {
        decision: PolicyDecision,
        /// A Cedar policy id.
        policy_id: String,
        /// The digest of Cedar's diagnostics; the text follows 42 §5.
        explanation_digest: Option<Cid>,
    },
}

/// 42 §1.3: 全フィールド, 除外なし.
///
/// The projection is the value, so the view borrows rather than clones — a `Cid` is taken over
/// these bytes on every gate evaluation, and copying a `String` per hash to say 「all of it」 would
/// be a copy that changes nothing. The trait is gx-canon's and the type is this crate's, so the
/// orphan rule admits the impl and gx-witness never has to know what BLAKE3 is (A-1's shape, applied
/// a third time).
///
/// Written out rather than skipped: without an impl there is no `gx_canon::cid::compute` for an
/// `Evidence` at all, and 42 §1.3 gives the type a row.
impl IdentityView for Evidence {
    type View<'a> = &'a Evidence;

    fn identity_view(&self) -> &Evidence {
        self
    }
}
