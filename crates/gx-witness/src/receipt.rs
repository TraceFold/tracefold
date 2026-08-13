//! The receipt: what was decided, over what, signed, and checkable by a stranger with no network.
//!
//! Spec: 42 §3.10 for the field tables and for ASM-14's two kinds, 42 §1.3 for what the payload's
//! identity covers, 43 T-4a/b/c and T-11 for when each kind is issued, 32 FR-018 for the
//! requirement, 34 AC-018 and AC-070 for how the two kinds are judged.
//!
//! # Four rulings shape this file, and each is implemented rather than remembered
//!
//! * **E-M2-6** (`req/38_ERRATA_2026-08-07.md` §8) -- `issued_at` leaves the signed core. 42 §3.10
//!   lists it among the payload's fields and 43 T-11 lists what a receipt's signature covers with
//!   no clock in it; the erratum rules 43 correct and CM-5 the principle (「signed payload から
//!   clock read 排除」). So the timestamp is on [`Receipt`], beside the envelope, and out of
//!   [`ReceiptPayload`].
//! * **E-M2-7** (§8) -- `fail_posture_engaged: bool` is **added**. 35 ASM-13 and 43 T-4e require a
//!   verdict receipt to record that the fail-closed posture was engaged, and 42 §3.10's table has
//!   nowhere to put it. Deterministic, so it goes *inside* the signed core -- the opposite of
//!   `issued_at`, and by the same test: does the value depend on a clock.
//! * **E-M2-2** (§8) -- `precondition_fingerprint` is a [`gx_core::FingerprintBytes`], carried and
//!   never interpreted. M4 owns `Fingerprint` and the CAS comparison; two receipts whose bytes
//!   compare equal here have **not** passed a CAS check.
//! * 🔴 **E-M5-11** (§41, M5 hand 6) -- `verdict` becomes an `Option`. 43 T-4e admits a
//!   transformation **without asking the gate**, so a receipt for it has no verdict to carry and
//!   an empty digest may not be minted to fill the seat (§32 M4H4-2). The wire form of every
//!   receipt that *does* carry a verdict is unchanged, and `tests/receipt_verdict_wire.rs` pins
//!   the bytes across the change rather than regenerating them afterwards.
//!
//! # The two kinds, and what offline verification does with them (ASM-14)
//!
//! 42 §3.10: 「`VerdictReceipt`（全`Verdict`＝Admit/Deny/Escalateで発行、43 T-4a/T-4b/T-4c）と
//! `CommitReceipt`（commit成功時のみ発行、43 T-11）。両者とも同一の`DsseEnvelope`/`ReceiptPayload`
//! スキーマを共有し、`ReceiptPayload.receipt_kind`で判別する」. One schema, one discriminant, and
//! two different obligations:
//!
//! | | `VerdictReceipt` | `CommitReceipt` |
//! |---|---|---|
//! | `inclusion_proof` | `None`, always (ASM-14: not yet in the ledger) | `Some`, required |
//! | `postcondition_fingerprint` | `None`, always (nothing was applied) | may be `Some` |
//! | `inverse_delta` | `None`, always (escrow is 43 T-10b, during commit) | may be `Some` |
//! | `checks.inclusion` | [`InclusionCheck::NotApplicable`] -- AC-018's `"skipped"` | verified against an anchor -- AC-070's `true` |
//!
//! [`verify_offline`] enforces the left column as a schema check and answers the right one against
//! a caller-supplied [`gx_core::Checkpoint`]. `tests/receipt_kind_branch.rs` is the whole table.
//!
//! # `enforced` is not checked, and 42 says it should be
//!
//! 42 §3.10 writes 「`VerdictReceipt`では意味を持たないため`true`固定」 while 35 ASM-13 and 43 T-4e
//! require a verdict-stage receipt to carry `enforced=false` together with `fail_posture_engaged=
//! true`. req/49 §3 M2-9 raised the collision and its 既定案 is 「field を 1 本足し、`VerdictReceipt`
//! の true 固定を**条件つきに読み替える**」; E-M2-7 (§8) adopted the first half. The second half is
//! what this file does by *not* asserting a value: both booleans are legal on both kinds, and
//! `tests/receipt_kind_branch.rs` pins that a verdict receipt with `enforced=false` verifies. A
//! schema check that enforced 42's 固定 would refuse the receipt ASM-13 requires.

use gx_canon::cid::IdentityView;
use gx_canon::{cbor, cid};
use gx_core::{
    Checkpoint, Cid, DsseSignature, FingerprintBytes, InclusionProof, KeyId, Timestamp,
    TransformationId, VerdictKind,
};
use gx_log::LedgerLeaf;
use serde::{Deserialize, Serialize};

use crate::dsse::{DsseEnvelope, RECEIPT_PAYLOAD_TYPE};
use crate::keys::{KeyPair, Retroaction, RevocationEntry, RevocationLedger, VerifyingKeyRef};
use crate::{Error, Result};

/// Which of ASM-14's two receipts this is (42 §3.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReceiptKind {
    /// Issued for every verdict, before the ledger has seen anything (43 T-4a/T-4b/T-4c).
    VerdictReceipt,
    /// Issued when a commit succeeded, after `ledger.append` (43 T-11).
    CommitReceipt,
}

/// The three verdicts 42 §3.10 spells as string literals, now typed (**E-M3-2**, H5-8 matured).
///
/// # It was a `String` for two hands, and why it stopped being one
///
/// Every other identifier this workspace could type, it typed: `TheoremId` over `Vec<String>`
/// (E-M2-18), `Domain` over a `u8`. The name for this one is `VerdictKind`, and 42 §0 files it
/// under **gx-engine** while req/49 §1 N-03 forbade M2 from creating it -- so hand 5's typed form
/// would have minted a reserved name early. What it did instead was declare the three literals once
/// in a `pub const VERDICT_KINDS: [&str; 3]` and have `VerdictSummary::check` refuse anything else
/// at verification time, with the debt written into H5-8: 「M3 で `VerdictKind` が生えたら型化へ
/// 寄せる」.
///
/// M3 hand 1 is when that falls due. The type could not simply move to gx-gate -- this crate would
/// then have to name the crate that names it (`GateInput.evidence: &[Evidence]`, 41 §4), which is
/// M2-1's cycle again -- so **E-M3-2** puts `VerdictKind` in gx-core:
///
/// > 「M3-13(採用): `VerdictKind`(3 値 enum)を **gx-core** へ、`Verdict`(payload つき)は gx-gate。
/// > gx-witness の `VERDICT_KINDS` 文字列検査は型検査へ置換(H5-8 満期)。循環 0 が機械条件」
///
/// The check moved rather than vanishing, and it moved **earlier**: a payload whose `kind` reads
/// `"Admitted"` now fails to decode, where before it decoded and `check_schema` refused it two
/// calls later. Serialising a fieldless variant writes its name, so the bytes did not move --
/// `tests/pae_golden.rs` pins them and stayed green across the change.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerdictSummary {
    /// 42 §3.10's `"Admit"|"Deny"|"Escalate"`, as [`gx_core::VerdictKind`] (E-M3-2).
    pub kind: VerdictKind,
    /// 42 §3.10: 「`Verdict`全体ではなくそのCID化digestを埋め込む」.
    ///
    /// # Which value, for a verdict that is not `Admit` (req/49 §3 M2-10)
    ///
    /// 41 §3's `Verdict` is `Admit(AdmitProof) | Deny(Vec<Reason>) | Escalate(EscalationTicket)`,
    /// and 42 §3.10 says only 「proof_digest」 -- which names something only the `Admit` arm has.
    /// req/49 §3 M2-10's 既定案 is 「Deny=`Vec<Reason>`・Escalate=`EscalationTicket` の CID を取る
    /// 規則を明記(型自体は M3)」 and no ruling has landed, so this hand writes the rule down and
    /// implements none of it: all three types are gx-gate (M3) and req/49 §1 N-01 forbids defining
    /// them here. What M2 can do is carry the digest and refuse a receipt that omits it, which is
    /// what the field's non-optionality does. req/54 §4 keeps the ticket open.
    pub proof_digest: Cid,
}

// `pub const VERDICT_KINDS: [&str; 3]` and `VerdictSummary::check` stood here until M3 hand 1.
// Both are now `gx_core::VerdictKind` (E-M3-2): the single declaration is that enum's `ALL`, and
// the refusal of a fourth spelling is serde's, which is the point of the change -- a check that
// runs at decode cannot be forgotten by a caller who builds a payload by hand. The history is kept
// in `VerdictSummary`'s own documentation above rather than deleted, because H5-8 recorded the
// string form as a *dated* compromise and the date is what makes the type readable.

/// What a receipt says, and the whole of what its signature covers (42 §3.10).
///
/// # Eleven fields, and the count 42 §3.10 is read as having
///
/// req/49 §2.1 and §3 M2-9 both call this table 「15 field」. It has **eleven** rows -- counted
/// mechanically by `tools/verify_m2h5.sh` off `req/spec/40-architecture/42-data-model.md`, not by
/// eye. The miscount changes nothing E-M2-7 ruled (the flag really is absent from all eleven) and
/// is raised in req/54 §4 as the same shape as M1's A-3.
///
/// Eleven rows minus `issued_at` (E-M2-6) plus `fail_posture_engaged` (E-M2-7) is eleven fields
/// here, and `tests/ac_018.rs` asserts the arithmetic against the spec file rather than against
/// this sentence.
///
/// # Field order
///
/// Encoded-key order -- length first, then bytewise -- as in [`crate::provenance`] and gx-log's
/// `tile.rs`. A convention that makes the canonical form the obvious form, and one no build
/// enforces (`gx-log/tests/map_key_order.rs` measured that the encoder sorts keys itself).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptPayload {
    /// 42 §3.10: the signing key's id, 「`DsseSignature.keyid`と一致」. Checked by
    /// [`verify_offline`], which is the point of carrying it inside the signed bytes: a receipt
    /// states which key ought to have signed it, so a signature moved onto another key's receipt
    /// does not verify even if that key is trusted.
    pub key_id: KeyId,
    /// 🔴 What was decided, **when something decided** (42 §3.10, **E-M5-11**).
    ///
    /// `Option` since M5 hand 6. 42 §3.10 types this as a `VerdictSummary` and 43 T-4e reaches a
    /// state in which no verdict exists: the fail-open posture degrades a transformation to
    /// record-only 「当該Transformationに限り」 **without calling the gate at all**, so there is
    /// nothing to summarise and no proof to digest. `req/38_ERRATA_2026-08-07.md` §41 rules it:
    ///
    /// > **M5H4-3 採(a)**=**E-M5-11**: `ReceiptPayload.verdict` を **`Option`** にする(42 §3.10
    /// > erratum)。T-4e は gate を呼ばず verdict が存在しない——**空 digest の鋳造禁(M4H4-2)を wire
    /// > まで貫く**形で、journal 側 E-M5-7(`verdict_digest=Option`)と対称。
    ///
    /// The three alternatives were each a way of writing down something untrue: minting an empty
    /// `AdmitProof` and digesting it (a proof for an admission no gate made — §32 M4H4-2 refused
    /// that twice), making `proof_digest` optional inside a present summary (a third state,
    /// 「verdict は在るが proof が無い」), or refusing the commit (which is what hand 4 did, and
    /// what left AC-037 unreachable).
    ///
    /// **The wire did not move**: serde writes `Some(x)` as `x`, so every receipt that has a
    /// verdict encodes to the bytes it encoded to before — pinned across the change in
    /// `tests/receipt_verdict_wire.rs` — and `None` writes the `0xf6` that 42 §2.1's golden vector
    /// G-5 already fixes for an absent value.
    ///
    /// **Not schema-checked against `fail_posture_engaged`.** A receipt with no verdict and no
    /// posture flag would say a commit happened for no stated reason, and the pairing is real; but
    /// §41 ruled the type and not the rule, and 43's own journal record (E-M5-7) carries the same
    /// two fields with the same absence of a cross-check. The producer refuses it instead —
    /// `gx_engine::Error::Unrepresentable` — and the hand's report raises the question rather than
    /// answering it here.
    ///
    /// 🔴 **Superseded by `req/38_ERRATA_2026-08-07.md` §43 (M5H6-8① 採(a)), implemented in the M5
    /// fix hand.** The paragraph above is kept rather than rewritten (no-delete): it records what
    /// was true between hand 6 and this hand, and *why* the rule was not written then. The ruling,
    /// verbatim:
    ///
    /// > **M5H6-8 ①採(a)・実装窓=fix批**: gx-witness `check_schema` に「`verdict=None` ⇒
    /// > `fail_posture_engaged=true`」の対規則を足す(fail-closed の二重防衛・42 §3.10 erratum 相当)。
    ///
    /// [`ReceiptPayload::check_schema`] now carries it. The producer's refusal stays where it is;
    /// the schema is the second defence, for payloads that reach this crate from a decoder rather
    /// than from gx-engine.
    pub verdict: Option<VerdictSummary>,
    /// 42 §3.10: 「`false`はrecord-onlyモードでの非強制適用（DR-2）」. Not schema-checked; see this
    /// module's header.
    pub enforced: bool,
    pub receipt_kind: ReceiptKind,
    /// 42 §3.10: `Transformation.id`. Checked against [`ReceiptPayload::transformation`] by
    /// [`verify_offline`] -- see [`Checks::canonical_cid`] for what that check can and cannot say.
    pub canonical_cid: Cid,
    /// 42 §3.10: the escrowed inverse delta's CID. `None` on a `VerdictReceipt` always, since
    /// escrow happens in 43 T-10b, during commit.
    pub inverse_delta: Option<Cid>,
    pub transformation: TransformationId,
    /// 42 §3.10 / ASM-14: required on a `CommitReceipt`, absent on a `VerdictReceipt`.
    pub inclusion_proof: Option<InclusionProof>,
    /// **E-M2-7**, added: 35 ASM-13 and 43 T-4e require it and 42 §3.10's table has no room.
    /// Deterministic, so it is inside the signed core.
    pub fail_posture_engaged: bool,
    /// Fingerprint₀ (42 §3.10), as opaque bytes until M4 (**E-M2-2**).
    pub precondition_fingerprint: FingerprintBytes,
    /// 42 §3.10: set only after something was applied, so `None` on a `VerdictReceipt`.
    pub postcondition_fingerprint: Option<FingerprintBytes>,
}

/// 42 §1.3: 「`ReceiptPayload` | 全フィールド | — | Receiptはmeta-witnessであり除外規則なし
/// （`Receipt`自体は署名対象）」.
///
/// Borrowed, as `Evidence`'s is, because the projection is the value. Note what the row's own
/// parenthesis says and what §1.3-4 repeats: the *signature* is not in here. The identity of a
/// payload is what was decided; the identity of the envelope around it (`crate::dsse`) is which
/// signed record was appended. Two questions, two digests.
impl IdentityView for ReceiptPayload {
    type View<'a> = &'a ReceiptPayload;

    fn identity_view(&self) -> &ReceiptPayload {
        self
    }
}

impl ReceiptPayload {
    /// The digest a ledger leaf carries for this receipt (42 §3.11's `receipt_digest`).
    ///
    /// # 🔴 This is a derivation, and the reason is a circularity in the 正本
    ///
    /// 42 §3.11 defines the field as 「`Receipt`（DSSE envelope bytes全体）のBLAKE3digest」. For a
    /// `CommitReceipt` that value cannot exist. Three statements are each clear and are together
    /// unsatisfiable:
    ///
    /// 1. 43 T-11 orders the commit as 「`ledger.append(...)` → `InclusionProof`；Receipt発行」 --
    ///    the append happens **before** the receipt is issued;
    /// 2. 42 §3.10 puts the resulting `inclusion_proof` **inside** `ReceiptPayload`, which is inside
    ///    the signed envelope;
    /// 3. 42 §3.11 says the leaf appended in (1) carries the digest of the envelope from (3).
    ///
    /// A leaf that committed to the whole envelope would have to commit to a proof derived from
    /// itself. No ordering of those three operations produces a value, and AC-070 asks a verifier
    /// holding only a receipt and a checkpoint to *recompute* it.
    ///
    /// # What is computed instead, and why these two exclusions
    ///
    /// The CID of this payload with `inclusion_proof` cleared -- 42 §1.3's own row for
    /// `ReceiptPayload` (「全フィールド」) applied to the value as it stood before the ledger answered.
    /// Two things are therefore outside the ledger's commitment, and each has a precedent in a
    /// ruling already made:
    ///
    /// * **the signatures**, which 42 §1.3-4 already keeps out of a `ReceiptPayload`'s identity
    ///   (「署名は除外…署名対象はpayload bytes」). A leaf that covered them could not be written
    ///   before the signing, which is what 43 T-11 requires.
    /// * **the inclusion proof**, which is the self-reference above. Excluding a field because
    ///   including it is impossible is E-M2-6's shape exactly, one layer along.
    ///
    /// Everything else is covered, so the leaf still binds the transformation, the verdict, the
    /// fingerprints, the inverse delta and both posture flags. What is lost against 42 §3.11's
    /// wording is that two receipts differing **only** in their signature share a leaf -- which is
    /// the same property `issued_at`'s exclusion buys, and which makes 43 ASM-43-1's idempotent
    /// re-append work across a retry rather than raising `Error::Conflict`.
    ///
    /// **Not ruled.** req/54 §4 raises the circularity as the blocking defect it is; a ruling that
    /// keeps 42 §3.11's literal wording has to change 42 §3.10 or 43 T-11, and neither is an
    /// implementation's to touch.
    ///
    /// # Errors
    /// [`Error::Canon`] if the payload has no canonical form.
    pub fn ledger_digest(&self) -> Result<Cid> {
        let staged = ReceiptPayload {
            inclusion_proof: None,
            ..self.clone()
        };
        Ok(cid::compute(&staged)?)
    }

    /// The kind-dependent obligations of ASM-14, as a check rather than as a comment.
    ///
    /// 42 §3.10 states them as prose about which fields are filled for which kind, and prose is
    /// what `req/38_ERRATA_2026-08-07.md` §11's H2-6 called the weak form. This is the schema
    /// req/49 §4 predicted would 「schema 検査としてここで初めて実体を持つ」 -- the conditional-
    /// requirement shape gx already meets in a fabrication-status YAML gate, now in a type.
    ///
    /// # The one rule that is not ASM-14's, and why it is here (**M5H6-8①**)
    ///
    /// `verdict = None` ⇒ `fail_posture_engaged = true`. 42 §3.10 states no such pairing and
    /// `req/38_ERRATA_2026-08-07.md` §43 rules it in anyway, as 「fail-closed の二重防衛・42 §3.10
    /// erratum 相当」. It is kind-independent, so it is checked before the kind is looked at: a
    /// receipt of either kind that carries no verdict is claiming 43 T-4e's degraded admission, and
    /// T-4e's own cell requires 「`fail_posture_engaged=true`を必ずreceiptに刻む」. The absence of
    /// both is not a milder receipt; it is a receipt that names no reason for having skipped the
    /// gate.
    ///
    /// One direction only. `fail_posture_engaged = true` **with** a verdict is legal and must stay
    /// legal — an operator running a degraded posture whose gate did answer produces exactly that,
    /// and `tests/receipt_kind_branch.rs::both_postures_are_legal` has held that half since M2.
    ///
    /// # Errors
    /// [`Error::Schema`], naming the field and the kind. AC-070 asks for exactly this on a
    /// `CommitReceipt` with no inclusion proof: 「スキーマ不正または`Ok(false)`」.
    pub fn check_schema(&self) -> Result<()> {
        // The verdict kind used to be checked here. It is checked by the decoder now (E-M3-2), so
        // a `VerdictSummary` that exists at all carries one of the three -- there is no state left
        // for this function to refuse.
        let refuse = |field: &str, expected: &str| Error::Schema {
            detail: format!(
                "a {:?} carries {field}, which ASM-14 says is {expected} (42 §3.10)",
                self.receipt_kind
            ),
        };

        // M5H6-8① (§43): the pairing rule, before the kind, because it holds for both kinds.
        if self.verdict.is_none() && !self.fail_posture_engaged {
            return Err(refuse(
                "no verdict and no engaged fail posture",
                "a pair: 43 T-4e is the only road to a receipt without a verdict and it \
                 requires 「`fail_posture_engaged=true`を必ずreceiptに刻む」 (§43 M5H6-8①)",
            ));
        }

        match self.receipt_kind {
            ReceiptKind::VerdictReceipt => {
                if self.inclusion_proof.is_some() {
                    return Err(refuse(
                        "an inclusion proof",
                        "always absent: 「常に`None`」",
                    ));
                }
                if self.postcondition_fingerprint.is_some() {
                    return Err(refuse(
                        "a postcondition fingerprint",
                        "always absent: 「`VerdictReceipt`では常にNone」",
                    ));
                }
                if self.inverse_delta.is_some() {
                    return Err(refuse(
                        "an inverse delta",
                        "always absent: escrow is 43 T-10b, during commit",
                    ));
                }
            }
            ReceiptKind::CommitReceipt => {
                if self.inclusion_proof.is_none() {
                    return Err(refuse(
                        "no inclusion proof",
                        "required: 「`CommitReceipt`は必須（`Some`）」",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A signed receipt: the DSSE envelope, and the one field E-M2-6 put outside it.
///
/// # Why this is not literally 42 §3.10's `Receipt = DsseEnvelope`
///
/// It was, until E-M2-6 took `issued_at` out of the signed core and sent it 「unsigned envelope
/// metadata へ」. A DSSE envelope has exactly three fields -- 42 §3.10's own table says so, and the
/// standard 42 §4 compares gx against says so -- and a fourth would make the wire form something no
/// DSSE reader parses. So the timestamp rides *beside* the envelope: [`Receipt::envelope`] is
/// exactly 42 §3.10's three fields and is what a DSSE verifier sees, and this struct is the pair.
/// The wire shape of that pair is in no 正本 and is raised in req/54 §4.
///
/// # The consequence worth stating
///
/// `issued_at` is outside [`Receipt::ledger_digest`] as well, so two receipts differing only in
/// their clock share a ledger leaf. That is CM-5 paying out: `ledger.append` is keyed on the
/// transformation and idempotent on the digest (43 ASM-43-1), and a clock inside the digest would
/// have turned a retry after midnight into an `Error::Conflict`. `tests/ac_070.rs` holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// 42 §3.10's three fields, and what a DSSE verifier is given.
    pub envelope: DsseEnvelope,
    /// **Unsigned** (E-M2-6, CM-5). A verifier that reads this as attested is reading something no
    /// signature covers; [`verify_offline`] does not look at it.
    pub issued_at: Timestamp,
}

impl Receipt {
    /// Encode the payload, wrap it, sign it (FR-018, FR-019, 43 T-4a/b/c and T-11).
    ///
    /// The order is the requirement and not an implementation detail: 42 §3.10 makes `payload` the
    /// canonical DAG-CBOR of the value and the signature a signature over *those bytes*, so the
    /// canonical form is produced first and everything downstream refers to it. req/26 §11's
    /// ①canonical-then-sign is the same shape, one encoding down.
    ///
    /// The schema is checked here as well as at verification. A producer that builds an impossible
    /// receipt should learn it at the moment it built one, not from a stranger months later --
    /// and signing an invalid receipt would put a valid signature on it.
    ///
    /// # Errors
    /// [`Error::Schema`] if the payload violates ASM-14, [`Error::Canon`] if it has no canonical
    /// form (an inline float in an evidence digest cannot reach here, since none is carried).
    pub fn issue(payload: &ReceiptPayload, issued_at: Timestamp, key: &KeyPair) -> Result<Self> {
        payload.check_schema()?;
        let bytes = cbor::encode(payload)?;
        let mut envelope = DsseEnvelope {
            payload_type: RECEIPT_PAYLOAD_TYPE.to_string(),
            payload: bytes,
            signatures: Vec::new(),
        };
        envelope.sign(key.signing_key(), key.key_id());
        Ok(Self {
            envelope,
            issued_at,
        })
    }

    /// The payload, decoded from the bytes that were signed.
    ///
    /// Strict decode, through gx-canon (CM-6): a payload whose bytes are not canonical is refused
    /// rather than accepted-and-re-canonicalised, because the second form would let one receipt
    /// have two byte spellings and only one of them would carry a valid signature.
    ///
    /// # Errors
    /// [`Error::Canon`] if the bytes are not canonical DAG-CBOR of a [`ReceiptPayload`].
    pub fn payload(&self) -> Result<ReceiptPayload> {
        Ok(cbor::decode(&self.envelope.payload)?)
    }

    /// `LedgerEntry.receipt_digest` (42 §3.11), as [`ReceiptPayload::ledger_digest`] derives it.
    ///
    /// Read that function before using this one: the value is **not** 42 §3.11's literal 「DSSE
    /// envelope bytes全体」, and the reason is a circularity in the 正本 rather than a shortcut.
    ///
    /// # Errors
    /// [`Error::Canon`] if the payload is not canonical DAG-CBOR.
    pub fn ledger_digest(&self) -> Result<Cid> {
        self.payload()?.ledger_digest()
    }

    /// The signature offered under `key_id`, if there is one.
    #[must_use]
    pub fn signature_for(&self, key_id: &str) -> Option<&DsseSignature> {
        self.envelope.signature_for(key_id)
    }
}

/// What an offline verification found (AC-018's `checks`, AC-070's `checks.inclusion`).
///
/// # Why there is no `signature: bool`
///
/// AC-019 makes a bad signature an `Err(SignatureInvalid)`, so a `Checks` that exists at all is one
/// whose signature verified. A `signature: true` field could only ever hold `true`, and a field
/// that cannot be `false` is a reader's invitation to check the wrong thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks {
    /// AC-018's 「canonical CID整合検査」: `canonical_cid == transformation`.
    ///
    /// # What this check is worth, honestly
    ///
    /// Both fields are inside the signed payload, so a tamperer cannot move one without breaking
    /// the signature -- which means this catches a **producer's** bug and no attack. 42 §3.10 asks
    /// for both fields all the same (`transformation` 「対象変換」, `canonical_cid` 「`Transformation.id`」),
    /// and an engine that filled them from two different places could disagree. Raised in req/54 §4:
    /// the AC names a check whose adversarial value is zero, and saying so is better than reporting
    /// it as though it were a signature.
    pub canonical_cid: bool,
    pub inclusion: InclusionCheck,
    /// What a consulted revocation list said about the key (**FR-M7-3**).
    ///
    /// [`RevocationCheck::NotConsulted`] unless the caller took [`verify_offline_consulting`], which
    /// is the road that has a list to consult. Present on every answer rather than only when a list
    /// was given: a field that appeared only when it was interesting is a field a reader misses on
    /// exactly the runs where it matters (M6H8-11 採(a)).
    pub revocation: RevocationCheck,
    /// Which key the signature was checked against. Carried so a caller aggregating verifications
    /// does not have to remember which key it passed in.
    pub key_id: KeyId,
}

impl Checks {
    /// AC-018's and AC-070's `Ok(true)`: everything that could be checked was, and passed.
    ///
    /// [`InclusionCheck::Unanchored`] is **not** a pass. A `CommitReceipt` verified without an
    /// anchor has had its signature checked and its ledger claim not checked at all, and reporting
    /// that as `true` is the fail-open req/29 §4 forbids -- 「skip と pass を同じ見た目にしない」.
    #[must_use]
    pub fn verified(&self) -> bool {
        self.canonical_cid
            && matches!(
                self.inclusion,
                InclusionCheck::NotApplicable | InclusionCheck::Verified
            )
            // **FR-M7-3**. `NotConsulted` passes and `Unanchored` does not, and the asymmetry is
            // argued on [`RevocationCheck`] rather than assumed here.
            && self.revocation.passes()
    }
}

/// What happened to the ledger half of a verification.
///
/// Four values where AC-018 writes `"skipped"` and AC-070 writes `true`, because those two are the
/// two *good* outcomes and a boolean has no room for the two bad ones. The AC vocabulary is a JSON
/// field of the M6 CLI (`gx receipt verify --offline`); this is the library form it will be
/// rendered from, and the mapping is written in each variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InclusionCheck {
    /// A `VerdictReceipt`: ASM-14 says there is nothing in the ledger yet. AC-018's `"skipped"`.
    NotApplicable,
    /// A `CommitReceipt` whose proof reached the anchor's root. AC-070's `true`.
    Verified,
    /// A `CommitReceipt` whose proof did not reach the anchor's root -- a forged proof, a proof
    /// against a different tree, or an anchor from a different log.
    Refuted,
    /// A `CommitReceipt` verified with no anchor to check against. Not a pass: see
    /// [`Checks::verified`].
    Unanchored,
}

/// What happened to the key half of a verification (**FR-M7-3**).
///
/// Five values, for [`InclusionCheck`]'s reason: 「the key is fine」 has three different meanings
/// here and folding them into a boolean would lose the one that matters most — that nothing was
/// consulted.
///
/// 🔴 **Why `NotConsulted` is a pass and `Unanchored` is not.** A `CommitReceipt` *claims* inclusion
/// in its own signed payload, so verifying one without an anchor leaves a claim the receipt made
/// unchecked (H5-9). A receipt makes no claim about revocation at all: the list is the **verifier's**
/// own input, and 45's ASM-45-2 makes consulting it optional (「revocation list参照はverifier側任意と
/// する」). So not consulting is a verifier declining an option, not a check that was skipped — and
/// the word is on the wire either way, which is M6H8-11 採(a)'s rule applied to the second thing a
/// verification can leave out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RevocationCheck {
    /// No list was consulted (the default, and what [`verify_offline`] always answers).
    NotConsulted,
    /// A list was consulted and holds no revocation of this key.
    NotRevoked,
    /// A revocation exists and takes effect **after** the moment of this verification, so there is
    /// nothing to apply yet. The other half of req/98 §3-2's 「失効時刻より後の verify」.
    NotYetInForce,
    /// A revocation is in force, the setting is [`Retroaction::FromRevocation`], and the receipt is
    /// dated before it: 「発行時点の鍵状態」 (ASM-45-2's DEFAULT). Valid.
    ValidAtIssue,
    /// The receipt is not valid: the key was already revoked when the receipt says it was issued, or
    /// the setting is [`Retroaction::All`].
    Revoked,
}

impl RevocationCheck {
    /// Whether this answer lets a receipt be valid.
    #[must_use]
    pub const fn passes(self) -> bool {
        !matches!(self, Self::Revoked)
    }
}

/// The revocation posture a verifier brings to a verification: a list, a setting, and a clock read.
///
/// All three are the **verifier's**, which is the whole shape of ASM-45-2: nothing about revocation
/// travels inside a receipt, and a receipt cannot state its own key's standing. `verified_at` is
/// injected rather than read here for 41 §6's reason (「乱数・時刻はengine境界で注入」) and for a
/// sharper one — a library that read a clock could not be tested about what it answers at a chosen
/// moment, which is most of what there is to test.
#[derive(Clone, Copy, Debug)]
pub struct RevocationPolicy<'a> {
    /// The entries this verifier has authenticated ([`RevocationLedger::from_signed`]).
    pub ledger: &'a RevocationLedger,
    /// How far back a revocation reaches. The setting req/98 §3-2 keeps outside the machine.
    pub retroaction: Retroaction,
    /// The moment the verification is happening.
    pub verified_at: Timestamp,
}

/// 🔴 The invariant (**FR-M7-3**): was this key revoked at or before the moment the receipt claims?
///
/// > 「鍵が receipt timestamp 以前に revoke されていないか」 (req/98 §3-2)
///
/// Four inputs and no hidden fifth, so that every answer can be reproduced from the values printed
/// beside it. The order of the arms is the order the questions have to be asked in:
///
/// 1. **no entry** — nothing to apply;
/// 2. **not yet in force** — the revocation is dated after this verification, and applying it early
///    would answer 「invalid」 about a receipt that is valid at the time of asking;
/// 3. **[`Retroaction::All`]** — every receipt of a revoked key is refused, and no clock is read;
/// 4. **before the revocation** — ASM-45-2's DEFAULT keeps it valid (「発行時点の鍵状態」);
/// 5. **at or after** — the key was already revoked when this receipt says it was issued.
///
/// Arm 4 believes `issued_at`, which **E-M2-6** keeps out of the signed core. That is the limit 45
/// §3 grades as TH-5's residual (「TSA連携なしのv0.1では失効時刻の第三者証明が弱い」) and it is why
/// arm 3 exists: a compromise is answered with the setting that reads no clock at all.
#[must_use]
pub fn revocation_status(
    entry: Option<&RevocationEntry>,
    issued_at: Timestamp,
    retroaction: Retroaction,
    verified_at: Timestamp,
) -> RevocationCheck {
    let Some(entry) = entry else {
        return RevocationCheck::NotRevoked;
    };
    if entry.revoked_at.0 > verified_at.0 {
        return RevocationCheck::NotYetInForce;
    }
    match retroaction {
        Retroaction::All => RevocationCheck::Revoked,
        Retroaction::FromRevocation if issued_at.0 < entry.revoked_at.0 => {
            RevocationCheck::ValidAtIssue
        }
        Retroaction::FromRevocation => RevocationCheck::Revoked,
    }
}

/// Verify a receipt with no ledger, no network and no clock (AC-018, AC-070).
///
/// # The order of the checks, and why it is the order
///
/// 1. **The signature**, over the raw envelope bytes. Before anything is decoded, because that is
///    what makes AC-019 hold: a bit flipped anywhere inside `payload` alters the pre-authentication
///    encoding, and a verifier that parsed first would report some of those flips as malformed
///    values instead of as bad signatures.
/// 2. **The payload decodes**, strictly (CM-6).
/// 3. **The schema**, per ASM-14's kind (AC-070's 「スキーマ不正」).
/// 4. **`key_id` agrees** with the signature that was checked. 42 §3.10 requires it and nothing
///    else would notice a receipt naming one key and signed by another that a caller happens to
///    trust.
/// 5. **The canonical CID**, and the **inclusion proof** against `anchor`.
///
/// # `anchor`
///
/// AC-070 says 「既知checkpoint照合」 -- the verifier has to already believe a checkpoint, from a
/// witness, a previous run, or an operator. Passing `None` is legitimate (a `VerdictReceipt` needs
/// none) and is reported as [`InclusionCheck::Unanchored`] for a `CommitReceipt` rather than
/// quietly passing. The checkpoint's own signature is **not** checked here: that is
/// [`crate::dsse::verify_checkpoint`], it may be a different key (45 ASM-45-1), and a verifier that
/// checked it silently would make one `Ok` mean two things.
///
/// # Errors
/// [`Error::SignatureInvalid`] for any flipped bit in the signed material or the signature
/// (AC-019), [`Error::Canon`] for a payload that is not canonical DAG-CBOR, [`Error::Schema`] for
/// an ASM-14 violation or a key id that disagrees with the signature, [`Error::Log`] if the
/// reconstructed leaf has no canonical form.
pub fn verify_offline(
    receipt: &Receipt,
    key: &VerifyingKeyRef<'_>,
    anchor: Option<&Checkpoint>,
) -> Result<Checks> {
    checks_of(receipt, key, anchor, None)
}

/// The same verification, **consulting a revocation list** (**FR-M7-3**).
///
/// A second entry point rather than a fifth argument on the first, because ASM-45-2 makes consulting
/// optional (「revocation list参照はverifier側任意とする」) and every caller written before this hand
/// consults nothing: a signature change would have turned that legitimate posture into a compile
/// error and then into a hurried `None` at each call site. The two functions differ in exactly one
/// input and answer the same type, with [`Checks::revocation`] saying which road was taken.
///
/// # Errors
/// Everything [`verify_offline`] refuses. Note what is **not** an error: a receipt whose key has
/// been revoked verifies as a *signature* and is refused by [`Checks::verified`] — reporting it as a
/// bad signature would send an operator looking for tampering that did not happen.
pub fn verify_offline_consulting(
    receipt: &Receipt,
    key: &VerifyingKeyRef<'_>,
    anchor: Option<&Checkpoint>,
    revocation: &RevocationPolicy<'_>,
) -> Result<Checks> {
    checks_of(receipt, key, anchor, Some(revocation))
}

fn checks_of(
    receipt: &Receipt,
    key: &VerifyingKeyRef<'_>,
    anchor: Option<&Checkpoint>,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Result<Checks> {
    receipt.envelope.verify(key)?;
    let payload = receipt.payload()?;
    payload.check_schema()?;

    if payload.key_id != key.key_id {
        return Err(Error::Schema {
            detail: format!(
                "the receipt names key {:?} and was verified against {:?} (42 §3.10: \
                 「`DsseSignature.keyid`と一致」)",
                payload.key_id, key.key_id
            ),
        });
    }

    let canonical_cid = payload.canonical_cid == payload.transformation.0;
    let inclusion = match (payload.receipt_kind, &payload.inclusion_proof, anchor) {
        (ReceiptKind::VerdictReceipt, _, _) => InclusionCheck::NotApplicable,
        (ReceiptKind::CommitReceipt, Some(proof), Some(head)) => {
            if verify_inclusion_from(&payload, proof, head)? {
                InclusionCheck::Verified
            } else {
                InclusionCheck::Refuted
            }
        }
        (ReceiptKind::CommitReceipt, _, None) => InclusionCheck::Unanchored,
        // Unreachable: `check_schema` refused a CommitReceipt with no proof two statements ago.
        // Written as a refusal rather than as an `expect`, because 41 §6 counts a panic as a bug.
        (ReceiptKind::CommitReceipt, None, Some(_)) => {
            return Err(Error::Schema {
                detail: "a CommitReceipt reached the inclusion check with no proof".to_string(),
            })
        }
    };

    // 🔴 The key half. `issued_at` is the receipt's **unsigned** envelope field (E-M2-6) and this is
    // the one place it is read for a judgement; the limit that carries is on `Retroaction`.
    let revocation = revocation.map_or(RevocationCheck::NotConsulted, |policy| {
        revocation_status(
            policy.ledger.revocation_of(key.key_id),
            receipt.issued_at,
            policy.retroaction,
            policy.verified_at,
        )
    });

    Ok(Checks {
        canonical_cid,
        inclusion,
        revocation,
        key_id: key.key_id.to_string(),
    })
}

/// Rebuild the ledger leaf from the receipt itself, and ask gx-log whether it is in the tree.
///
/// # This is what makes AC-070 possible offline
///
/// 42 §3.11's `LedgerLeaf` is `{transformation, receipt_digest, index}` and a receipt carries all
/// three: the transformation is a payload field, the index is `inclusion_proof.leaf_index`, and the
/// digest is [`ReceiptPayload::ledger_digest`] of the payload in hand. So a verifier with a receipt
/// and a checkpoint needs nothing from the log -- not the entry, not the neighbouring leaves, not
/// the tree.
///
/// The third of those is where 42 §3.11's own wording had to be derived from rather than
/// transcribed; [`ReceiptPayload::ledger_digest`] is the whole argument and req/54 §4 the ticket.
///
/// The arithmetic is gx-log's (`verify_inclusion_of`) and is not repeated here. That is why this
/// crate names gx-log at all -- see the dependency note in the crate root.
fn verify_inclusion_from(
    payload: &ReceiptPayload,
    proof: &InclusionProof,
    anchor: &Checkpoint,
) -> Result<bool> {
    let leaf = LedgerLeaf {
        index: proof.leaf_index,
        receipt_digest: payload.ledger_digest()?,
        transformation: payload.transformation,
    };
    Ok(gx_log::proof::verify_inclusion_of(
        proof,
        &anchor.root_hash,
        &leaf,
    )?)
}
