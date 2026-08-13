//! The two ledger proofs a receipt refers to, as data.
//!
//! Spec: 42 §3.11 for both field tables, 41 §2 for the crate that computes them.
//!
//! # What is here and what is not
//!
//! `InclusionProof` is here because 42 §3.10 puts one inside `ReceiptPayload`, and `Checkpoint` is
//! here because it carries the [`crate::DsseSignature`] that came down with it -- **E-M2-1**
//! (`req/38_ERRATA_2026-08-07.md` §8) moving the data below both crates so that gx-witness and
//! gx-log stop naming each other. `LedgerEntry`, `LedgerLeaf`, `Tile` and `ConsistencyProof` do
//! **not** come down: no type below gx-log carries one, so moving them would be symmetry rather
//! than a cycle broken, and each of them is bound to arithmetic that lives up there anyway --
//! leaf hashing (42 §3.11's `BLAKE3(0x00 || ...)`), tile widths, and the `verify_consistency` that
//! E-M2-8 rules must take its `ConsistencyProof` by argument. Hand 2 owns them.
//!
//! Nothing in this file verifies anything. A proof here is a claim somebody recorded; whether the
//! audit path actually reaches the root is `gx-log`'s question (AC-022, AC-023).

use crate::dsse::DsseSignature;
use crate::transformation::Timestamp;
use crate::Cid;
use serde::{Deserialize, Serialize};

/// A Merkle inclusion proof (42 §3.11).
///
/// `audit_path` is the sibling hashes from the leaf up to the root, and an empty one is a
/// legitimate value rather than a malformed proof: leaf 0 of a tree of size 1 has no sibling.
///
/// The `Cid` type carries these hashes because 42 §3.11 writes them that way, and 42 §1.1 defines
/// `Cid` as a BLAKE3 digest of canonical DAG-CBOR with no prefix byte -- while a node hash is
/// `BLAKE3(0x01 || left || right)`. The two are both 32 bytes of BLAKE3 and are not the same
/// function. That tension is req/49 §3 M2-6, ruled by **E-M2-12**: gx-canon gains a mint API that
/// takes the domain byte, so the value in this field is produced by one function instead of by
/// whichever caller reaches for `blake3` first. Hand 2 adds it; the field name stays as 42 writes
/// it, and the distinction is documented rather than encoded in a second type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InclusionProof {
    pub leaf_index: u64,
    /// The tree size the proof was generated against. A proof is only meaningful against the root
    /// of a tree of exactly this size, which is why it travels with the path rather than beside it.
    pub tree_size: u64,
    pub audit_path: Vec<Cid>,
}

/// A signed tree head (42 §3.11).
///
/// # The timestamp is kept
///
/// E-M2-6 took `issued_at` out of the receipt's *signed core* -- CM-5, 「signed payload から clock
/// read 排除」 -- because 43 T-11 lists what a receipt signature covers and no clock is in it.
/// That list is about receipts. A checkpoint's timestamp is a property of the tree head itself,
/// nothing in 43 or 35 says otherwise, and removing it here would be a second erratum on a
/// question nobody put. So the field stands as 42 §3.11 writes it and the question -- whether a
/// checkpoint signature should cover its own clock read -- is raised in req/50 §4 for the owner
/// rather than settled by the crate that happens to hold the struct.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The log's namespace, e.g. `glovrex-ledger/v1` (42 §3.11). It is what stops a checkpoint of
    /// one log from verifying against another's key.
    pub origin: String,
    pub tree_size: u64,
    pub root_hash: Cid,
    pub timestamp: Timestamp,
    /// Signed by the ledger key, which is not necessarily the receipt key (45 ASM-45-1 keeps v0.1
    /// to a single tier, so hand 3 may well use one key for both -- the type does not decide it).
    pub signature: DsseSignature,
}

// ---------------------------------------------------------------------------
// FR-M04 -- the aggregate verdict checkpoint (M7 hand 6, 案 A)
// ---------------------------------------------------------------------------

/// How many verdicts of each kind a window held (**FR-M04**).
///
/// # Why there is a fourth number
///
/// [`crate::VerdictKind`] has three variants and this struct has four fields. The fourth is 43
/// T-4e: the evidence collector could not be reached, the fail posture admitted the
/// transformation, and **no gate ran**. The journal records that row as `Verdict { kind: Admit,
/// verdict_digest: None, fail_posture_engaged: true }` -- an admission with no verdict behind it --
/// and folding it into `admit` would report a decision nobody made. M4H4-2 refused that shape
/// twice ("「未実装」と「失敗」 wearing one face"), and a count published to an auditor is the last
/// place to start.
///
/// So a reader of `admit` is reading gate answers, and a reader of `unverdicted` is reading the
/// number of times the deployment ran without one.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct VerdictTally {
    pub deny: u64,
    pub admit: u64,
    pub escalate: u64,
    /// 43 T-4e: admitted **because** the verifier was unreachable, by a gate that never ran.
    pub unverdicted: u64,
}

impl VerdictTally {
    /// The four numbers added up.
    ///
    /// It is the width of the window the tally covers, which is why every checkpoint carries both:
    /// `window_end - window_start` and this value are two statements of one quantity, and a
    /// verifier that finds them different has found a checkpoint that does not describe its own
    /// window.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.deny + self.admit + self.escalate + self.unverdicted
    }
}

/// A signed count of the verdicts a deployment issued over one window (**FR-M04**, SHOULD).
///
/// # The defect
///
/// 42 §3.10 fixes `VerdictReceipt.inclusion_proof` to `None` (ASM-14), so a Deny is signed and
/// then lives nowhere an outsider can reach. An operator who simply never exports the Deny
/// receipts can show an auditor a hundred-percent-Admit record, and nothing in the ledger
/// contradicts it -- the ledger only ever held the commits.
///
/// This type is the contradiction. It commits to **how many** there were, so that withholding the
/// receipts stops being free: the count is signed, the windows are contiguous, and the whole chain
/// is bound to the ledger's own head.
///
/// # What it does not do -- two limits, and they are the AC's own words
///
/// * 🔴 **裁定 #3**: it does not close **policy relaxation**. A gate widened until nothing is
///   refused publishes `deny = 0`, honestly. The claim FR-M04 supports is 「**非開示**は検出でき
///   る」 and no more; `crates/gx-engine/tests/ac_vc.rs::policy_relaxation_is_not_detected` is that
///   sentence as a measurement.
/// * 🔴 **裁定 #14**: it does not close a **split view**. What a signature attests is 「this key
///   stated these counts」, never 「these are the only counts this key stated」, so one key can sign
///   two internally consistent chains for two verifiers. Detecting the fork needs the verifiers to
///   compare, which is the consistency-proof window `req/98` §9 行 v-6 sends to v0.2.1.
///
/// # No hash chain, and why
///
/// A `previous` link was weighed and dropped. What it would buy over contiguous windows is
/// tamper-evidence of a checkpoint's *content*, and the signature already covers that; what
/// remains is substituting a differently-signed checkpoint with the same boundaries, which needs
/// the key -- and an attacker with the key can re-sign a chain of any length. It would also need a
/// hash domain 42 §3.11 does not define, which is a spelling the canon does not have (the same
/// argument `store.rs` used to refuse a CRC). Contiguity is what detects a removed checkpoint, and
/// [`VerdictCheckpoint::ledger_tree_size`] is what detects a removed tail.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerdictCheckpoint {
    /// The log's namespace, as 42 §3.11 uses it on [`Checkpoint`]: what stops one deployment's
    /// counts from being read as another's.
    pub origin: String,
    pub tally: VerdictTally,
    /// The first verdict sequence number this checkpoint speaks for, inclusive. Zero for the
    /// first, and every later one opens where its predecessor closed.
    pub window_start: u64,
    /// One past the last, exclusive -- and therefore the number of verdicts the deployment had
    /// issued in total when this checkpoint was minted.
    pub window_end: u64,
    /// The head of the **commit** ledger at the moment this checkpoint was minted.
    ///
    /// `None` is not a degenerate case: a window in which every transformation was refused commits
    /// nothing, so the ledger is empty and has no root -- which is exactly the window FR-M04
    /// exists for. A type that made this mandatory would be unable to describe the interesting
    /// case.
    pub ledger_root_hash: Option<Cid>,
    /// How many leaves that head held.
    ///
    /// This is the load-bearing field of AC-VC-5. Every ledger leaf is a commit, and a commit is
    /// downstream of an Admit, so a chain of verdict checkpoints whose Admits do not reach the
    /// ledger's current size has stopped publishing. The ledger's own size is not something the
    /// operator can shrink quietly -- it is anchored by [`Checkpoint`] -- which is what makes it
    /// worth binding to.
    pub ledger_tree_size: u64,
    /// Outside the signed core, as [`Checkpoint`]'s is and for CM-5's reason: a verified
    /// checkpoint is a statement about counts, not about when they were written down.
    pub timestamp: Timestamp,
    pub signature: DsseSignature,
}
