//! Where a transformation came from: its intent, its inputs, and the environment it ran in.
//!
//! Spec: 42 §3.9 for the two field tables, 32 FR-015 for the requirement, 34 AC-015 for its test,
//! 11 §4 for the word 「由来」 this structure carries the last quarter of.
//!
//! # What a `Provenance` adds to a `Transformation`
//!
//! 42 §3.9 says it plainly: 「`Transformation`自体が`actor`/`parents`/`subject`を保持する（41既定）
//! ため、`Provenance`は**それに加えて**「環境」…とIntentへの遡及可能性を担う補助構造体」. So three
//! of the four things 11 §4's glossary calls provenance — actor, parent transformations, the
//! subject — are already in the arrow, and this type carries the fourth (environment) plus the
//! route back to the Intent.
//!
//! # The second argument (E-M2-5)
//!
//! AC-015 writes `Provenance::derive_from(&T)`. 41 §3's ten fields do not hold `input_objects`
//! (「plan/verify中にadapterが読み取った入力スナップショット群」) or any part of [`Environment`], so
//! a one-argument derivation would have to invent them. req/49 §3 M2-11 raised it and **E-M2-5**
//! (`req/38_ERRATA_2026-08-07.md` §8) ruled: 「Provenance 決定的導出=E-A7-2 と同じく導出不能分は
//! 呼び出し側引数(`ProvenanceInputs`)」 — the same move [`gx_core::CompositionMetadata`] makes for
//! composition, and for the same reason. gx-witness cannot read a clock, a hostname or an adapter's
//! notion of which snapshots it touched; asking is better than guessing.
//!
//! What survives of AC-015 is unchanged and is the whole point: **the same arguments give the same
//! value**. [`Provenance::derive_from`] reads nothing else — no clock, no counter, no environment
//! variable, no hash seed — and it folds the one argument whose spelling a caller could vary
//! (`input_objects`' order) into a canonical one.
//!
//! # This type has no CID
//!
//! 42 §1.3's IdentityView table lists `ObjectSnapshot`, `Intent`, `Transformation`, `PlannedDelta`,
//! `Fingerprint`, `Evidence`, `AdmitProof`, `EscalationTicket`, `ReceiptPayload` and `LedgerLeaf`.
//! `Provenance` is **not** among them, and 42 §3.10's fifteen `ReceiptPayload` fields do not carry
//! one either. So there is no projection to write and nothing here mints a digest; the value
//! serialises and that is all it owes today. Raised as H4-2 in req/53 §4 rather than closed by
//! inventing a row.

use gx_core::{Cid, ObjectId, SubstrateKind, Transformation, TransformationId};
use serde::{Deserialize, Serialize};

/// Where the engine was standing when the transformation happened (42 §3.9).
///
/// Every field comes from the caller: an engine version is the engine's to state, an adapter kind
/// and version are the adapter's, and a host or correlation id is a fact about a machine or a
/// session that gx-witness has no way to observe. ASM-10 (single-node operation) makes `host_id`
/// omissible, which is why it and `correlation_id` are `Option` while the three version strings are
/// not — a receipt that cannot say which engine produced it is missing something, and a receipt
/// that does not name a host is merely running alone.
///
/// # Field order
///
/// Declared in encoded-key order (length first, then bytes), which E-42-3
/// (`req/38_ERRATA_2026-08-07.md` §4) settles as the order the encoder writes. As `map_key_order.rs`
/// measured for gx-log, the encoder sorts a struct's keys itself and a misordered declaration fails
/// nothing — so this is a convention that makes the canonical form the obvious form, not a check.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Environment {
    /// Omissible under ASM-10 (「単一ノード運用」).
    pub host_id: Option<String>,
    pub adapter_kind: SubstrateKind,
    /// 42 §3.9's example: an MCP session id.
    pub correlation_id: Option<String>,
    /// gx-engine's build version.
    pub engine_version: String,
    pub adapter_version: String,
}

/// The part of a `Provenance` that an arrow does not contain (E-M2-5).
///
/// Named as a struct rather than passed as two loose arguments for [`gx_core::CompositionMetadata`]'s
/// reason: a caller who has these facts has them as a set, and a set with a name is one thing to
/// look up instead of an argument order to remember.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceInputs {
    /// Snapshots the adapter read during plan/verify, beyond the `subject` (42 §3.9).
    ///
    /// Handed over in whatever order they were collected. [`Provenance::derive_from`] puts them in
    /// the canonical one, so a caller does not have to know there is a canonical one.
    pub input_objects: Vec<ObjectId>,
    pub environment: Environment,
}

/// The derived record of where a transformation came from (42 §3.9).
///
/// # Field order
///
/// Encoded-key order again: `environment` (11) before the two thirteens, `input_objects` before
/// `intent_digest` on the bytes after the shared `in`, and `transformation` (14) last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub environment: Environment,
    /// Ascending by id, whatever order the caller collected them in — see
    /// [`Provenance::derive_from`].
    pub input_objects: Vec<ObjectId>,
    /// The canonical digest of the Intent submitted for this transformation, when there was one.
    pub intent_digest: Option<Cid>,
    pub transformation: TransformationId,
}

impl Provenance {
    /// Derive the provenance of `t`, given the facts `t` does not contain (AC-015, E-M2-5).
    ///
    /// Deterministic in the strong sense AC-015 asks for: the body reads its two arguments and
    /// nothing else. Two calls with equal arguments return equal values, and so do two calls whose
    /// `input_objects` differ only in order.
    ///
    /// Infallible. Nothing here can fail — there is no I/O, no arithmetic that can overflow and no
    /// invariant relating the fields — so a `Result` whose error arm is unreachable would be a lie
    /// about the API (the same reasoning that makes [`gx_core::identity`] infallible).
    ///
    /// # `input_objects` is canonicalised, not copied
    ///
    /// 42 §3.9 types it `Vec<ObjectId>` and writes no ordering rule, which req/49 §3 M2-19 raised:
    /// two adapters that read the same three snapshots in two orders would produce two different
    /// provenance records of one fact, and any digest taken over one of them would differ. The
    /// 既定案 there is req/26 §2's id-sort-for-determinism — 「収集順でなく id 昇順に正準化する」 —
    /// and that is what the sort below is. Duplicates are **kept**: folding them away would be a
    /// second decision about whether reading one snapshot twice is one fact or two, and 42 says
    /// nothing about it (H4-3).
    ///
    /// # `intent_digest`, and the one place this is a judgement
    ///
    /// 42 §1.3-3: 「`IntentId`は`Intent`（§3.3）のCIDであり`submit`時点…で確定」, so 42 §3.9's
    /// 「`submit`時のIntentのcanonical digest」 is `t.intent_id`'s digest. The question is when it is
    /// `None`, and 42 §3.9 answers with two clauses that do not pick out the same set:
    /// 「Draft/Candidate生成時のみ設定（order≥1の合成変換ではNone）」.
    ///
    /// The parenthetical cannot be the rule. [`gx_core::compose`] sets `order = max(f.order,
    /// g.order)` and not `+ 1`, so composing two order-0 arrows yields an order-0 arrow — reading
    /// 「order≥1」 as the predicate would attach a submitted Intent's digest to a composite, which is
    /// exactly what the clause exists to prevent. What `compose` does write is `parents = [f.id,
    /// g.id]`, while `identity` and a fresh submission write none, so `parents.is_empty()` is the
    /// predicate available on a `Transformation` that separates the two.
    ///
    /// It is a **derivation, not a transcription**, and it errs toward `None`: an arrow that lists
    /// provenance parents without being a composition loses its digest too, because 41 §3 calls
    /// `parents` 「the provenance and composition DAG」 and nothing on the value tells the two apart.
    /// Losing a true `Some` costs a lookup; keeping a false one puts a claim about a submitted
    /// intent on a receipt that has none. req/53 §4 H4-1 asks the Owner to rule.
    #[must_use]
    pub fn derive_from(t: &Transformation, inputs: ProvenanceInputs) -> Self {
        let ProvenanceInputs {
            mut input_objects,
            environment,
        } = inputs;
        input_objects.sort_unstable();

        Self {
            environment,
            input_objects,
            intent_digest: if t.parents.is_empty() {
                Some(t.intent_id.0)
            } else {
                None
            },
            transformation: t.id,
        }
    }
}
