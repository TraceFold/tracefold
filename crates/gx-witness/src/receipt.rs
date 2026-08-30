// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The receipt: what was decided, over what, signed, and checkable by a stranger with no network. (sem:
//! SEM-gx-witness-060, SEM-gx-witness-061, SEM-gx-witness-062, SEM-gx-witness-063, SEM-gx-witness-064,
//! SEM-gx-witness-065, SEM-gx-witness-066, SEM-gx-witness-067, SEM-gx-witness-068, SEM-gx-witness-069,
//! SEM-gx-witness-070, SEM-gx-witness-071, SEM-gx-witness-072, SEM-gx-witness-073, SEM-gx-witness-074,
//! SEM-gx-witness-075, SEM-gx-witness-076, SEM-gx-witness-077, SEM-gx-witness-078, SEM-gx-witness-079,
//! SEM-gx-witness-080, SEM-gx-witness-081, SEM-gx-witness-082, SEM-gx-witness-083, SEM-gx-witness-084,
//! SEM-gx-witness-085, SEM-gx-witness-086, SEM-gx-witness-087, SEM-gx-witness-088, SEM-gx-witness-089,
//! SEM-gx-witness-090, SEM-gx-witness-091, SEM-gx-witness-092, SEM-gx-witness-093, SEM-gx-witness-094,
//! SEM-gx-witness-095, SEM-gx-witness-096, SEM-gx-witness-097, SEM-gx-witness-098, SEM-gx-witness-099,
//! SEM-gx-witness-100, SEM-gx-witness-101, SEM-gx-witness-102, SEM-gx-witness-103, SEM-gx-witness-104,
//! SEM-gx-witness-105, SEM-gx-witness-106, SEM-gx-witness-107, SEM-gx-witness-108, SEM-gx-witness-109,
//! SEM-gx-witness-110, SEM-gx-witness-111)
//!
//! Spec: 42 §3.10 for the field tables and for ASM-14's two kinds, 42 §1.3 for what the payload's
//! identity covers, 43 T-4a/b/c and T-11 for when each kind is issued, 32 FR-018 for the
//! requirement, 34 AC-018 and AC-070 for how the two kinds are judged.
//!
//! # Four rulings shape this file, and each is implemented rather than remembered
//!
//! * **E-M2-6** (`req/38_ERRATA_2026-08-07.md` §8) -- `issued_at` leaves the signed core. 42 §3.10
//!   lists it among the payload's fields and 43 T-11 lists what a receipt's signature covers with
//!   no clock in it; the erratum rules 43 correct and CM-5 the principle ("exclude the clock read
//!   from the signed payload"). So the timestamp is on [`Receipt`], beside the envelope, and out of
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
//! 42 §3.10: "`VerdictReceipt` (issued for every `Verdict` = Admit/Deny/Escalate, 43 T-4a/T-4b/T-4c) and
//! `CommitReceipt` (issued only on commit success, 43 T-11). Both share the same `DsseEnvelope`/`ReceiptPayload`
//! schema and are told apart by `ReceiptPayload.receipt_kind`". One schema, one discriminant, and
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
//! 42 §3.10 writes "fixed to `true` because it carries no meaning on `VerdictReceipt`" while 35
//! ASM-13 and 43 T-4e require a verdict-stage receipt to carry `enforced=false` together with
//! `fail_posture_engaged= true`. req/49 §3 M2-9 raised the collision and its default proposal is
//! "add one field, and reread `VerdictReceipt`'s fixed `true` **conditionally**"; E-M2-7 (§8)
//! adopted the first half. The second half is
//! what this file does by *not* asserting a value: both booleans are legal on both kinds, and
//! `tests/receipt_kind_branch.rs` pins that a verdict receipt with `enforced=false` verifies. A
//! schema check that enforced 42's fixed value would refuse the receipt ASM-13 requires.

use gx_canon::cid::IdentityView;
use gx_canon::{cbor, cid};
use gx_core::{
    BoundaryStage, Checkpoint, Cid, DeterminismBoundary, DsseSignature, FingerprintBytes,
    InclusionProof, KeyId, Reversibility, Timestamp, TransformationId, VerdictKind,
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
/// at verification time, with the debt written into H5-8: "once `VerdictKind` grows in M3, fold it
/// into the typed form".
///
/// M3 hand 1 is when that falls due. The type could not simply move to gx-gate -- this crate would
/// then have to name the crate that names it (`GateInput.evidence: &[Evidence]`, 41 §4), which is
/// M2-1's cycle again -- so **E-M3-2** puts `VerdictKind` in gx-core:
///
/// > "M3-13 (adopted): `VerdictKind` (the 3-value enum) goes to **gx-core**, `Verdict` (with its
/// > payload) is gx-gate's. gx-witness's `VERDICT_KINDS` string check is replaced by a type check
/// > (H5-8 matured). Zero cycles is the machine condition."
///
/// The check moved rather than vanishing, and it moved **earlier**: a payload whose `kind` reads
/// `"Admitted"` now fails to decode, where before it decoded and `check_schema` refused it two
/// calls later. Serialising a fieldless variant writes its name, so the bytes did not move --
/// `tests/pae_golden.rs` pins them and stayed green across the change.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerdictSummary {
    /// 42 §3.10's `"Admit"|"Deny"|"Escalate"`, as [`gx_core::VerdictKind`] (E-M3-2).
    pub kind: VerdictKind,
    /// 42 §3.10: "embed its CID'd digest, not the whole `Verdict`".
    ///
    /// # Which value, for a verdict that is not `Admit` (req/49 §3 M2-10)
    ///
    /// 41 §3's `Verdict` is `Admit(AdmitProof) | Deny(Vec<Reason>) | Escalate(EscalationTicket)`,
    /// and 42 §3.10 says only "`proof_digest`" -- which names something only the `Admit` arm has.
    /// req/49 §3 M2-10's default proposal is "spell out the rule for taking the CID of
    /// Deny=`Vec<Reason>`/Escalate=`EscalationTicket` (the type itself is M3's)" and no ruling has landed, so this hand writes the rule down and
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

// ---------------------------------------------------------------------------
// DR-46-24(A): the read-set, and the granularity tag that is the condition of its existence
// ---------------------------------------------------------------------------

// 🔴 **DR-46-26** — `pub struct ReadEntry { digest: Cid, locator: String }` stood here, declared by
// D24 in the window when a receipt was the only thing that carried one. It is now
// [`gx_core::ReadEntry`], and the paragraph above is kept rather than deleted (no-delete) because it
// records why the type was ever in this crate.
//
// The relocation is **forced by the producer**. DR-46-26 widens `SubstrateAdapter::invert` to return
// what the escrow read, and `gx-substrate` does not depend on `gx-witness` -- adding that edge would
// put a receipt crate (and `gx-log` behind it) into the boundary crate's transitive dependencies.
// `ReadEntry` is `{Cid, String}` and holds nothing this crate owns, so it goes down to `gx-core`;
// `ReadSet` below, which is paired with the spill threshold and with `gx-log`'s proof arithmetic,
// stays. The re-export keeps `gx_witness::receipt::ReadEntry` naming the same type it always named.
pub use gx_core::ReadEntry;

/// Beyond this many distinct objects, a read-set spills from G3 to G4 (`req/38` §236 ruling 2).
///
/// 🔴 **The measurement disagrees with the constant, and the constant is what was ruled.**
/// `req/350` §0 set five by arithmetic over member encodings and put the two-times crossover
/// between five and six; `tests/d24_read_set_cost.rs` encodes the members instead and finds about
/// **102 bytes an entry rather than 89**, which moves that crossover to between **four and five**
/// (`n=4` → 1.878, `n=5` → 2.097). The constant stays at the ruled value because `req/350` §4-5
/// had already withdrawn the reasoning the number came from — the two-times line is not the
/// falsifier any more (§7-5) — and changing a ruled constant on the strength of a line nobody is
/// steering by would be worse than recording that the two do not agree. `req/441` carries it to
/// the Fable ruling.
pub const READ_SET_SPILL_THRESHOLD: usize = 5;

/// What the escrow read, at a granularity the reader can tell apart (**DR-46-24(A)**).
///
/// # Why the tag is not optional, and not a second field
///
/// `req/350` §4-1 made the tag the condition of the whole design:
///
/// > The guarantee changes silently at a threshold. The same words "read-set attested" mean
/// > receipt-alone up to five and path-required from six. A product whose reader cannot tell the
/// > two apart is worse than a uniformly weaker guarantee. → carrying a granularity tag (`G3`/`G4`)
/// > as a **required field** on the receipt, and writing the difference into `docs/LIMITS.md`, is
/// > the condition. Drop the tag and the proposal may be refused.
///
/// `req/440` §0-3 then ruled the shape: **one** field, with the tag inside the structure rather
/// than beside it. That is what an enum is — the tag is the map's single key, so a reader reaches
/// one member of the payload and the granularity is the first thing in it — and it is why the
/// alternative (`entries` plus a `granularity` string, two fields) was refused: two fields can
/// disagree, and this one cannot be built disagreeing.
///
/// # What each granularity decides, from the receipt alone
///
/// `req/350` §3's table, as [`ReadSet::names`] implements it: **G3 decides**, because the entries
/// are in the signed bytes. **G4 does not** — the root is a digest, and a digest with no preimage
/// decides nothing on its own. G4 buys back the same decision at G2's price *given the entries from
/// somewhere else*, and the "somewhere else" is measured in [`ReadSet::PerEffectRoot`]'s own
/// documentation.
/// # 🔴 **DR-46-34** — and the same argument, applied to the absence
///
/// `req/38` §268 ruling 5 raised it and `req/472` §6 drafted it: this type used to have exactly the
/// two members below, and every way of **not** having a read-set was spelled `Option::None` at the
/// field. `req/498` located the producers and found the collapse total —
///
/// | the road | what it means | the remedy it calls for |
/// |---|---|---|
/// | ~~`gx_substrate::InvertOutcome::from_option` fixes `Vec::new()` on **both** arms, and T-11 hands the empty list to [`ReadSet::from_reads`]~~ 🔴 **retracted, DEFECT-892-1 (`req/895` §1)**: those adapters do read, and `from_option` is gone. The road that remains is an adapter answering `InvertOutcome::inverted(_, Vec::new())` in its own source | the escrow ran and touched no object | none; this is a property of the change |
/// | `gx-engine`'s `rebuilt_attest` ends its `find_map` in `unwrap_or_default()` | a rebuild found **no** `InverseEscrowed` record for this transformation | the record is gone — `gx repair`, and the journal's own retention |
/// | `gx-engine`'s `InverseEscrowed.reads` is `#[serde(default)]` (E-M5-13's shape) | the record is there and **predates** 42 §3.13's `reads` | nothing is wrong; this project is older than DR-46-26 |
/// | `issue_verdict_receipt` writes `None` by ASM-14 | the escrow is 43 T-10b and had not run | none; the question was not asked yet |
///
/// The first three are `CommitReceipt` facts with three different remedies and they were one
/// spelling. **The same ruling that put the granularity tag inside the structure puts these there
/// too** — `req/440` §0-3's "one field, and it cannot be built disagreeing" is an argument about
/// where a discriminator lives, and it does not stop applying because the thing being discriminated
/// is an absence. So [`ReadSet::Nothing`], [`ReadSet::NoEscrowRecord`] and
/// [`ReadSet::ReadsNotJournalled`] are members here rather than a second field beside `read_set`,
/// and `Option::None` at the field narrows to the fourth row: **a receipt nobody asked**.
///
/// # What `None` still means, and why the rule is not tightened to forbid it
///
/// A `CommitReceipt` this binary issues always carries `Some`. A `CommitReceipt` **issued before
/// this lane** carries `null`, and [`ReceiptPayload::check_schema`] deliberately does *not* refuse
/// it: the payload of an old receipt is bytes that were signed, [`verify_offline`] is a statement
/// about those bytes, and a schema rule that refused them would break every receipt already in the
/// field to buy a distinction only new receipts can carry anyway. So `null` on a `CommitReceipt`
/// reads as "issued before DR-46-34" — a fifth, historical spelling this crate decodes and no
/// longer writes. The narrowing is enforced on the **producer** side, and
/// `gx-engine/tests/dr4634_read_set_absence.rs` is what enforces it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadSet {
    /// **G3** — one entry per distinct object, in the signed bytes.
    ///
    /// About 102 bytes an entry (`tests/d24_read_set_cost.rs`), so a receipt carrying six of them
    /// is roughly 2.3× the 466-byte payload `req/350` §2-3 took as its baseline.
    PerRead(Vec<ReadEntry>),
    /// **G4** — the root of an RFC 6962 tree over those entries, and nothing else.
    ///
    /// 52–53 bytes whatever `leaf_count` is, which is the whole reason the spill exists — measured,
    /// and within a byte of the 1.116 ratio `req/350` §3 carried over from a per-effect digest
    /// without measuring it.
    ///
    /// # Where the path lives: **beside the receipt, and derived rather than stored**
    ///
    /// `req/350` §4-4 named this the third thing it never measured. `tests/d24_read_set_cost.rs`
    /// costs the three placements and the arithmetic decides:
    ///
    /// | placement | what the side artifact holds | bytes at `n=32` |
    /// |---|---|---|
    /// | A — every path in the receipt | — | 5,472 |
    /// | B — the entries beside it, path derived on demand | entries | **3,266** |
    /// | C — the paths beside it | paths (**and** the entries, to have a leaf at all) | 5,472 + 3,266 |
    ///
    /// A is dominated by G3 itself from `n=8` upward (2,192 bytes of paths against 1,633 bytes of
    /// entries at `n=16`): an issuer who is willing to pay for every path has already paid more
    /// than carrying the read-set outright, and would then still need the entries. C is B plus a
    /// derivable quantity, because a path without its leaf verifies nothing. So the path is
    /// **never stored** — it is a function of the entries, and [`read_set_path`] is that function.
    /// Deriving one at `n=32` measured 86 µs (72.6 µs of leaf hashes, 13.4 µs of path), against the
    /// ~103 ms an ext4 wrap costs (`req/350` §2-2): 0.08%.
    PerEffectRoot {
        /// The root of the tree over the entries' leaf hashes.
        root: Cid,
        /// How many entries it folds. Without it a verifier cannot tell how long a path should be,
        /// and `gx_log::proof`'s refusal of a mis-sized path is the same arithmetic one tree down.
        leaf_count: u64,
    },
    /// 🔴 **DR-46-34** — the escrow ran, and read nothing.
    ///
    /// `ReadSet::from_reads` answers this for an empty entry list, which is where the fact used to
    /// become `Ok(None)`.
    ///
    /// 🔴🔴 **RETRACTED by DEFECT-892-1 (`req/895` §1).** The sentence that stood here —
    /// "it is the ordinary answer on the fs, git and postgres adapters, whose `invert` builds its
    /// inverse out of the snapshot already in hand" — was **the defect stated as a feature**.
    /// Those adapters read their substrate (`std::fs::read`, `repo::tip`, a `SELECT`), and
    /// `InvertOutcome::from_option` was discarding the reads before they reached here. So the
    /// ordinary fs commit was minting a **signed** receipt that carried this member, and this
    /// member decides — see [`ReadSet::names`] — that **no** locator in the universe was read.
    /// `from_option` is deleted; those four adapters mint their entries where the read answers, and
    /// their ordinary answer is now [`ReadSet::PerRead`].
    ///
    /// What is left for this member: an escrow that genuinely touched no object. A fixture that
    /// holds its answer in memory is one; so is any future adapter whose inverse is a pure function
    /// of the delta.
    ///
    /// **A positive statement, not an absence.** It says the escrow was asked and answered — which
    /// is exactly what the two members below cannot say, and is why they are not this one. 🔴 And
    /// because it is positive, a road that reaches it without having earned it is not silent: it is
    /// **wrong**, under a signature. That is what DEFECT-892-1 was.
    Nothing,
    /// 🔴 **DR-46-34** — a rebuild that found no `InverseEscrowed` record for this transformation.
    ///
    /// 43 §7-3b's road re-derives the payload from the journal because the prior it digests stopped
    /// existing when `apply` fired (42 §3.13's DR-46-26 note is the argument). Where the record is
    /// not in the journal at all, `rebuilt_attest`'s `find_map` ends in `unwrap_or_default()` and
    /// the road holds **nothing** about what was read — which is a different fact from
    /// [`ReadSet::Nothing`] and calls for a different remedy: a trimmed or damaged journal (42 §5),
    /// not a change with no reads.
    ///
    /// A receipt carrying this does not reproduce the digest of the receipt the ledger witnessed,
    /// and that is the intended behaviour rather than a regression: `Engine::resume` answers
    /// `payload_matched: Some(false)` and refuses, where before it produced a receipt that
    /// **attested** an escrow with no reads on the strength of a record it never found.
    NoEscrowRecord,
    /// 🔴 **DR-46-34** — a rebuild over a journal written before 42 §3.13 carried `reads`.
    ///
    /// `InverseEscrowed.reads` is `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
    /// (E-M5-13's shape), so a pre-DR-46-26 record decodes with an empty list and is
    /// indistinguishable from one that recorded an empty list — *unless something in the record
    /// says which*. `InverseEscrowed.reads_attested` is that something, and it is a `bool` in
    /// `undetermined`'s exact shape for `undetermined`'s exact reason.
    ///
    /// The distinction is worth a member because the two call for opposite conclusions. "The
    /// escrow read nothing" is a statement about the change. "This journal does not record what
    /// the escrow read" is a statement about the **journal**, and a reader who takes the second for
    /// the first has been told a fact about the world by a gap in a file.
    ReadsNotJournalled,
}

impl ReadSet {
    /// The tag, in the spelling `req/350` §4-1 and `docs/LIMITS.md` use.
    #[must_use]
    pub const fn granularity(&self) -> &'static str {
        match self {
            ReadSet::PerRead(_) => "G3",
            ReadSet::PerEffectRoot { .. } => "G4",
            // 🔴 **DR-46-34** — the three absence members answer their own name rather than a `G`
            // number. Inventing one would put a granularity where there is no set to be granular
            // about, and `req/350` §4-1's whole point is that this string tells a reader **which
            // guarantee** they hold. "There is no read-set, and here is which absence" is not a
            // weaker granularity; it is a different sentence.
            ReadSet::Nothing => "nothing",
            ReadSet::NoEscrowRecord => "no_escrow_record",
            ReadSet::ReadsNotJournalled => "reads_not_journalled",
        }
    }

    /// 🔴 **DR-46-34** — does this carry a read-set at all, or is it one of the three absences.
    ///
    /// The predicate a caller wants before reading [`ReadSet::distinct_objects`], which answers `0`
    /// for every absence and would otherwise let "no read-set" be read as "zero objects read" —
    /// the same conflation, one level down from the one this DR closes.
    #[must_use]
    pub const fn is_attested(&self) -> bool {
        matches!(self, ReadSet::PerRead(_) | ReadSet::PerEffectRoot { .. })
    }

    /// How many distinct objects the escrow read, at either granularity.
    #[must_use]
    pub fn distinct_objects(&self) -> u64 {
        match self {
            ReadSet::PerRead(entries) => entries.len() as u64,
            ReadSet::PerEffectRoot { leaf_count, .. } => *leaf_count,
            // 🔴 **DR-46-34** — `0` is true of `Nothing` as a count and is **not** a claim on the
            // other two. Ask [`ReadSet::is_attested`] first where the difference matters.
            ReadSet::Nothing | ReadSet::NoEscrowRecord | ReadSet::ReadsNotJournalled => 0,
        }
    }

    /// Build the read-set the threshold calls for, or [`ReadSet::Nothing`] for an escrow that read
    /// nothing.
    ///
    /// The spill is decided here and nowhere else, so the granularity on a receipt is a function of
    /// the read-set rather than of whichever caller assembled it.
    ///
    /// # 🔴 **DR-46-34** — this used to return `Result<Option<Self>>`
    ///
    /// The `Ok(None)` arm is the coordinate at which "the escrow read nothing" stopped being a
    /// fact and became the same `null` as "nobody recorded it". The signature narrows to
    /// `Result<Self>` so that the caller **cannot** re-open the collapse by assembling an
    /// `Option` of its own: a road that has no entries to hand this function is now a road that
    /// has to name its own absence, and there are exactly two such roads (`gx-engine`'s
    /// `rebuilt_attest`, on the two arms `req/498` measured).
    ///
    /// # Errors
    /// [`Error::Canon`] if an entry has no canonical form, which is what a leaf hash is taken over.
    pub fn from_reads(mut entries: Vec<ReadEntry>) -> Result<Self> {
        entries.sort();
        entries.dedup();
        if entries.is_empty() {
            return Ok(ReadSet::Nothing);
        }
        if entries.len() <= READ_SET_SPILL_THRESHOLD {
            return Ok(ReadSet::PerRead(entries));
        }
        let leaves = read_set_leaves(&entries)?;
        Ok(ReadSet::PerEffectRoot {
            root: read_set_root(&leaves),
            leaf_count: leaves.len() as u64,
        })
    }

    /// Does this receipt, **alone**, say that `locator` was read?
    ///
    /// `req/350` §3's denominator made executable. `Some(true)`/`Some(false)` is a decision the
    /// signed bytes carry; `None` is G4 saying honestly that it cannot answer without the entries,
    /// which is the difference the granularity tag exists to publish.
    ///
    /// 🔴 **DR-46-34** — [`ReadSet::Nothing`] answers `Some(false)` about **every** locator, and
    /// that is the whole value of the member: an escrow that read nothing decides the question for
    /// the entire universe of objects, from the receipt alone, which is a *stronger* answer than
    /// G3 gives. The other two absences answer `None` for G4's reason — they hold nothing to
    /// decide with — and this is the line at which the old `Option<ReadSet>` was silently giving
    /// `Nothing`'s strong answer to roads that had not earned it.
    #[must_use]
    pub fn names(&self, locator: &str) -> Option<bool> {
        match self {
            ReadSet::PerRead(entries) => Some(entries.iter().any(|e| e.locator == locator)),
            ReadSet::PerEffectRoot { .. } => None,
            ReadSet::Nothing => Some(false),
            ReadSet::NoEscrowRecord | ReadSet::ReadsNotJournalled => None,
        }
    }
}

/// The leaf hashes of a read-set: `0x00 || canonical_dagcbor(entry)`, 42 §3.11's rule one tree down.
///
/// # Errors
/// [`Error::Canon`] if an entry has no canonical form.
pub fn read_set_leaves(entries: &[ReadEntry]) -> Result<Vec<Cid>> {
    entries
        .iter()
        .map(|e| cid::mint_leaf(e).map_err(Error::from))
        .collect()
}

/// RFC 6962's MTH over those leaves — 42 §3.11's `node_hash`, applied to a tree that is not the
/// ledger's.
///
/// # Panics
/// On an empty slice. A read-set with no reads is [`Option::None`] on the payload, not a root over
/// nothing, and [`ReadSet::from_reads`] is the only constructor that reaches here.
#[must_use]
pub fn read_set_root(leaves: &[Cid]) -> Cid {
    assert!(!leaves.is_empty(), "a root over no leaves is not a root");
    if leaves.len() == 1 {
        return leaves[0];
    }
    let k = split_point(leaves.len());
    cid::mint_node(&read_set_root(&leaves[..k]), &read_set_root(&leaves[k..]))
}

/// RFC 6962 §2.1's `k`: the largest power of two strictly below `n`.
fn split_point(n: usize) -> usize {
    let mut k = 1;
    while k * 2 < n {
        k *= 2;
    }
    k
}

/// The sibling hashes from `index` to the root, leaf-first — **derived, never stored**.
///
/// See [`ReadSet::PerEffectRoot`] for the measurement that made storing one the losing placement.
pub fn read_set_path(index: usize, leaves: &[Cid], out: &mut Vec<Cid>) {
    if leaves.len() <= 1 {
        return;
    }
    let k = split_point(leaves.len());
    if index < k {
        read_set_path(index, &leaves[..k], out);
        out.push(read_set_root(&leaves[k..]));
    } else {
        read_set_path(index - k, &leaves[k..], out);
        out.push(read_set_root(&leaves[..k]));
    }
}

/// Fold a leaf and its path back to a root — RFC 6962 §2.1.1's walk, the same two counters
/// `gx_log::proof::reconstruct_root` uses one tree up.
///
/// `None` when the path does not fit the tree the caller declares, which is the only way a verifier
/// learns that it was handed a path from somewhere else.
#[must_use]
pub fn read_set_fold(index: u64, leaf: &Cid, siblings: &[Cid], leaf_count: u64) -> Option<Cid> {
    if leaf_count == 0 || index >= leaf_count {
        return None;
    }
    let mut node = index;
    let mut last = leaf_count - 1;
    let mut acc = *leaf;
    for sibling in siblings {
        if last == 0 {
            return None;
        }
        if node % 2 == 1 || node == last {
            acc = cid::mint_node(sibling, &acc);
            while node != 0 && node.is_multiple_of(2) {
                node /= 2;
                last /= 2;
            }
        } else {
            acc = cid::mint_node(&acc, sibling);
        }
        node /= 2;
        last /= 2;
    }
    if last == 0 {
        Some(acc)
    } else {
        None
    }
}

/// The CBOR key whose value the ledger leaf is taken **without** (42 §3.11's circularity; see
/// [`ReceiptPayload::ledger_digest`] for why this one member is cleared and no other).
const INCLUSION_PROOF_KEY: &str = "inclusion_proof";

/// CBOR `null` — 42 §2.1's `f6`, and what `serde` writes for a `None` **at a key**.
///
/// The distinction is the whole of this module's arithmetic: `None` is not "the key is missing", it
/// is "the key is present and holds `f6`". A canonical map with a key holding `f6` and one without
/// the key are different byte strings and therefore different digests.
const CBOR_NULL: u8 = 0xf6;

/// 🔴 **`req/38` §324 ruling 3 — the ledger leaf of a receipt, derived from the bytes that were
/// signed.**
///
/// # The defect this closes, in the words of the three lanes that met it
///
/// `ReceiptPayload::ledger_digest` re-encodes a **struct**. For a value this build just built that
/// is exact; for bytes that arrived from somewhere, it silently asks a different question — what
/// would *this year's schema* have written — and `req/519` §7-5 measured what that costs:
///
/// > `ReceiptPayload::ledger_digest()` is `cid::compute(&staged)` — **it re-derives the leaf from a
/// > struct whose canonical form moves with the schema**. Therefore every member ever added has
/// > already moved every historical leaf.
///
/// (`req/519` §7-5 is written in Japanese; the block above is its content, and the report is the
/// source a reader should go to for the wording. Rendered rather than quoted verbatim because
/// `probes/doubt/tests/cjk_doubt.rs` counts CJK lines per directory and this file is not one of the
/// places the census admits them — `req/38` §319's lesson, which cost a lane a merge turn.)
///
/// `req/38` §294 met it first and read it as a decode problem; §519 §7-5 measured that `Option` did
/// not fix it and named the real cause one level down; §324 caught the third repeat — a 2026-08-22
/// specimen, signature and anchor intact, verifying as `inclusion: refuted`. **`refuted` is the
/// vocabulary's word for tampering**, and it was being said about a document nobody had touched.
///
/// # What it computes, and why the arithmetic is exactly this
///
/// 43 T-11 appends the leaf **before** the receipt is issued, so what the ledger witnessed is the
/// canonical form of the payload as it stood with no inclusion proof in it. The engine builds that
/// form by encoding the payload with `inclusion_proof: None` — and `serde` writes a `None` **at the
/// key**, as `f6`. So the bytes the ledger committed to are the signed bytes with one value
/// replaced by one byte:
///
/// ```text
/// signed:  {... "inclusion_proof": {leaf_index, tree_size, audit_path} ...}
/// leaf-of: {... "inclusion_proof": f6                                  ...}
/// ```
///
/// **Replaced, not removed.** Removing the key would decrement the map header and produce bytes no
/// build ever digested — a leaf that matches nothing in any ledger. The key stays, the map count
/// stays, the key order stays; one value becomes `f6`. That is what makes this road answer the same
/// number as `ReceiptPayload::ledger_digest` for a receipt this build issued **and** the number the
/// 2026-08 builds wrote for the receipts they issued. It is one function, not a compatibility
/// shim with a version switch, because there is only one rule and every generation obeyed it.
///
/// # What it does **not** do
///
/// It does not decode. Nothing here names a member other than the one key it clears, so a document
/// carrying members this build has never heard of is digested exactly as its own build digested it.
/// That is the property the previous three attempts lacked, and it is why this cannot rot the way
/// they did: a member added to `ReceiptPayload` tomorrow does not appear in this function.
///
/// It also does not verify anything. The signature is the envelope's business and the inclusion
/// proof is [`verify_offline`]'s; this answers one question, which is what number the leaf carries.
///
/// # Errors
/// [`Error::Canon`] when the bytes are not canonical DAG-CBOR (they are audited to the whole of
/// 42 §2.1 on the way past, so a spelling no encoder would have written cannot mint a leaf).
pub fn ledger_digest_of_signed_payload(payload_bytes: &[u8]) -> Result<Cid> {
    let Some(span) = cbor::value_span(payload_bytes, INCLUSION_PROOF_KEY)? else {
        // No such key. Not a state any build in this lineage produces — `Option` members are
        // written at their key — so the honest answer is the digest of the bytes as they stand
        // rather than a guess about what a build that omitted it would have meant.
        return Ok(cid::of_canonical_bytes(payload_bytes)?);
    };
    let mut staged = Vec::with_capacity(payload_bytes.len() - span.len() + 1);
    staged.extend_from_slice(&payload_bytes[..span.start]);
    staged.push(CBOR_NULL);
    staged.extend_from_slice(&payload_bytes[span.end..]);
    Ok(cid::of_canonical_bytes(&staged)?)
}

/// 🔴 **`req/493` §0 / AC-6** — whether the process that produced this receipt was held by the
/// kernel, and which ruleset held it.
///
/// # A third fact, orthogonal to the two that were already here
///
/// `req/493` §0: the confinement context is carried "as a **third fact orthogonal** to the existing
/// two values `enforced` / `record_only`" — it is not a fourth value of either. `enforced` says
/// whether a `Deny` would have stopped this transformation; this says whether the process that
/// carried it out could have written outside what it declared **even if nothing had checked**. A
/// receipt can be `enforced=true` and unconfined (gx checked, the kernel did not), or
/// `enforced=false` and confined (the posture was record-only, the kernel still held the process).
/// Neither implies the other, which is what makes this a seat rather than a flag on an existing one.
///
/// # Two fields and two impossible pairs
///
/// `req/493` §1 AC-6 names exactly two: "the kernel-confined bit + the ruleset hash". Two fields
/// spell four combinations and two of them are not states of the world:
///
/// * confined with no ruleset named — a claim that the kernel held something, with no answer to
///   *what*. `gx_confine::ConfinePlan::ruleset_hash` exists precisely so that a reader can
///   re-derive the answer from the pre-image; a `true` with no hash is the claim without the
///   evidence.
/// * unconfined with a ruleset named — a hash for a ruleset the kernel did not take. The plan may
///   well have been derived (`gx confine --plan-only` derives one and enforces nothing); what a
///   receipt may not do is name it as if it had held.
///
/// [`ReceiptPayload::check_schema`] refuses both, which is `req/493` §1 AC-4's rule applied one
/// layer along: a gate that has never been fired is not a gate, so each of the two has a bed in
/// `tests/confinement_attest.rs` that makes it false and is refused.
///
/// # `kernel_confined: false` is a **statement**, and that is why the seat on the payload is the
/// `Option` and this type is not
///
/// The same shape DR-46-28 settled for `DeterminismBoundary`: a value that means "no" and a value
/// that means "nobody wrote this" must not be one spelling. A process that was not launched under
/// `gx confine` produces `kernel_confined: false` — a true sentence about the run — and every
/// receipt this binary issues carries one. The `None` on [`ReceiptPayload::confinement`] means
/// something else entirely and is documented there.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConfinementContext {
    /// Whether the kernel was actually holding the writing face when this receipt was produced.
    ///
    /// `gx_confine::Confinement::to_json`'s `kernel_confined` is the same bit and is derived the
    /// same way (`fs.is_enforcing()`): "fully-enforced" or "partially-enforced" is the kernel
    /// holding something back, and every other face status is not.
    ///
    /// 🔴 What it does **not** say, written here because the name invites the other reading: it is
    /// the **write** face and no other. `req/497` §6 measured what this build's ruleset covers —
    /// reads are unrestricted, execution is unrestricted, `rename` is outside Landlock ABI 1 — and
    /// `gx_confine::FS_COVERS` is the sentence that says so on every run. A reader who takes this
    /// bit for "the process could do nothing it did not declare" is reading more than it carries.
    pub kernel_confined: bool,
    /// [`gx_confine::ConfinePlan::ruleset_hash`]'s answer: the domain-separated digest of the
    /// ruleset's pre-image, as text.
    ///
    /// A `String` rather than a [`Cid`] because it is not one: `gx-confine` mints it through
    /// `gx_canon::cid` with the `Leaf` domain so that it cannot collide with a transformation id
    /// even over identical bytes, and carrying it in the `Cid` type here would put it in the same
    /// namespace as the ids this payload joins on. `None` exactly when `kernel_confined` is false.
    pub ruleset_hash: Option<String>,
}

impl ConfinementContext {
    /// The true sentence about a process nobody confined.
    ///
    /// Named rather than spelled at each of its callers because it is the **default answer** and a
    /// default written by hand in a dozen places is a default that drifts. It is also the one
    /// combination a reader has to be able to tell from `None`: this says "the kernel was not
    /// holding this process", `None` says "the bytes predate the erratum that asks".
    #[must_use]
    pub fn unconfined() -> Self {
        Self {
            kernel_confined: false,
            ruleset_hash: None,
        }
    }
}

/// 🔴 **DR-46-45 (`req/973` §B-1)** — what an undo compared before it fired, carried in the signed
/// bytes so a third party holding the receipt alone can tell the two apart.
///
/// # The three-valued discipline, and why only two values reach a receipt
///
/// `gx_engine::UndoWitness` has three variants and the third one — `Missing` — is a **refusal**
/// (R3, `req/38` §160 ruling 2). A refused undo mints no `TransformationId`, appends no `Planned`
/// and issues no receipt, so there is no signed payload for the third value to appear in. That is
/// not the third value being folded into one of the other two (which is the defect `req/38`
/// §294/DR-46-26 spent lanes closing); it is the third value living in the refusal surface — exit 3
/// / HTTP 409 `PRECONDITION_CHANGED` — where `req/38` §132 ruling 2 put it.
///
/// # `Unobservable` carries text, not a second copy of the engine's enum
///
/// The vocabulary of *which* nothing it was belongs to `gx_engine::Unobservable`, whose `reason()`
/// is the one place those five sentences are written. Re-declaring that enum here would give one
/// fact two spellings across a crate boundary — DR-46-26's defect in the shape gx-witness is least
/// able to keep in step, since this crate cannot name gx-engine (the dependency runs the other
/// way). So the reason arrives as opaque text, exactly as `catalogue_hash` does: "gx-witness
/// carries the field; it does not know what a `Catalogue` is."
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndoDisposition {
    /// The compare-and-swap ran: the world was read afresh and matched the `postcondition_
    /// fingerprint` the original's own receipt signed. This is "checked, then restored".
    Attested,
    /// No attestation existed to compare against, so the inverse was applied **without** a CAS —
    /// declared rather than refused (DR-46-7, `req/38` §123 ruling 1). This is "fired without
    /// checking", and before this field it wore the same face as `Attested`.
    Unobservable {
        /// `gx_engine::Unobservable::reason()`'s sentence, carried verbatim.
        reason: String,
    },
}

impl UndoDisposition {
    /// The word both surfaces print, and the word this payload answers with.
    ///
    /// One spelling for CLI stdout, HTTP's `witness`, and a reader of the signed bytes — the parity
    /// `req/973` §B-1 asks for is *this function having one caller-visible form*, not three
    /// formatters agreeing by inspection.
    #[must_use]
    pub fn word(&self) -> String {
        match self {
            UndoDisposition::Attested => "attested".to_string(),
            UndoDisposition::Unobservable { reason } => format!("unobservable:{reason}"),
        }
    }
}

/// 🔴 **DR-46-45 (`req/973` §B-2)** — the compensation edge, on the face.
///
/// # One key and not two, and the reason is that the two `Option`s would be the same `Option`
///
/// `req/973` §B-2 names a field `undoes: Option<TransformationId>` and §B-1 names a witness field
/// beside it. The set on which each is `Some` is *identical* — a committed undo — so two `Option`s
/// would need a cross-field agreement rule in [`ReceiptPayload::check_schema`] to forbid the two
/// states that are not states of the world. One key cannot be assembled into a contradictory shape,
/// which is `req/440` §0-3's rule ("two fields can contradict each other; one field cannot be built
/// into a contradictory form") applied one erratum along. The deviation from §B-2's literal field
/// name is declared in `req/973` §7-3 rather than made silently.
///
/// # The edge is already signed; this only makes it readable
///
/// `T_u.parents` includes `T_o.id` (43 T-12) and `parents` is inside the `IdentityView` the
/// `TransformationId` is a CID over, so `canonical_cid` already binds this edge. Publishing it in
/// the payload adds **no new authority**: it moves an edge from "inside a hash nobody can invert"
/// to "readable by a party holding the receipt". The direction is child→parent only — `T_o`'s
/// receipt is signed and immutable, and 43 T-12 forbids touching it — so the DAG stays append-only:
/// only a new node makes a new edge.
///
/// # Why the edge cannot be inferred instead
///
/// Measured, not assumed (`req/973` §1-2): `inverse_delta` is **not** a join key — an undo and an
/// unrelated later transformation held the same `gx1:…` because both deltas say "make it BBB", so a
/// join on it stands up false edges. Nor is the fingerprint chain one: two different acts left the
/// same `postcondition_fingerprint`, so it produces parallel edges. `crates/gx-engine/tests/
/// r973_undo_attestation.rs` keeps the `inverse_delta` join as a **negative control** and asserts
/// the false edge appears, so the gate names what it is protecting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoAttestation {
    /// The transformation this one undoes — `T_u.parents[0]`, which 43 T-12 fixes as `T_o.id`.
    pub undoes: TransformationId,
    /// Whether the CAS ran before the inverse was applied. See [`UndoDisposition`].
    pub witness: UndoDisposition,
}

/// What a receipt says, and the whole of what its signature covers (42 §3.10).
///
/// # Fourteen fields, and the count 42 §3.10 is read as having
///
/// req/49 §2.1 and §3 M2-9 both call this table "15 fields". It had **eleven** rows -- counted
/// mechanically by `tools/verify_m2h5.sh` off `req/spec/40-architecture/42-data-model.md`, not by
/// eye. The miscount changes nothing E-M2-7 ruled (the flag really is absent from all eleven) and
/// is raised in req/54 §4 as the same shape as M1's A-3.
///
/// 🔴 **DR-46-24(A) (`req/38` §236 ruling 2, `req/440` §0-3/§0-4)** — the table gains **two** rows
/// and the struct two fields: `read_set` (what the escrow read, with its granularity tag inside it)
/// and `fingerprint_scope` (P2, riding on the same erratum because `req/350` §7-4 measured it as
/// the cheapest thing to close beside it). Thirteen rows minus `issued_at` (E-M2-6) plus
/// `fail_posture_engaged` (E-M2-7) is thirteen fields here, and `tests/ac_018.rs` asserts the
/// arithmetic against the spec file rather than against this sentence.
///
/// **The wire moved, and it had to.** A canonical DAG-CBOR map with two more keys is different
/// bytes even when both values are absent — `None` is `0xf6` *at a key*, not nothing — so unlike
/// E-M5-11 this erratum is a migration and not a retyping. `tests/receipt_verdict_wire.rs` carries
/// the pre-DR-46-24 golden beside the new one rather than replacing it, so the change is recorded
/// as a difference instead of being regenerated away.
///
/// 🔴 **DR-46-26 (`req/38`'s S1 ruling 5)** — the table gains a **third** row in a second window
/// and the struct a fourteenth field: `reversibility`, C-25's three-valued answer. D24 closed the
/// read-set half of `req/38` §198 ruling (b) and left the other half open by name; this closes it,
/// and it is the same kind of change as the one above — a key added to a canonical map, so the
/// wire moves again. The D24 form is followed exactly rather than invented: the pre-DR-46-26
/// golden is **kept beside** the new one (`tests/inverse_status_wire.rs`), and the subtraction that
/// turns one into the other — remove the single `reversibility` key, get the D24 bytes back — is
/// asserted rather than described, so "one key was added and nothing else moved" is a measurement.
///
/// # Field order
///
/// Encoded-key order -- length first, then bytewise -- as in [`crate::provenance`] and gx-log's
/// `tile.rs`. A convention that makes the canonical form the obvious form, and one no build
/// enforces (`gx-log/tests/map_key_order.rs` measured that the encoder sorts keys itself).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptPayload {
    /// 42 §3.10: the signing key's id, "matches `DsseSignature.keyid`". Checked by
    /// [`verify_offline`], which is the point of carrying it inside the signed bytes: a receipt
    /// states which key ought to have signed it, so a signature moved onto another key's receipt
    /// does not verify even if that key is trusted.
    pub key_id: KeyId,
    /// 🔴 What was decided, **when something decided** (42 §3.10, **E-M5-11**).
    ///
    /// `Option` since M5 hand 6. 42 §3.10 types this as a `VerdictSummary` and 43 T-4e reaches a
    /// state in which no verdict exists: the fail-open posture degrades a transformation to
    /// record-only "for that Transformation alone" **without calling the gate at all**, so there is
    /// nothing to summarise and no proof to digest. `req/38_ERRATA_2026-08-07.md` §41 rules it:
    ///
    /// > **M5H4-3, adopted (a)** = **E-M5-11**: make `ReceiptPayload.verdict` an **`Option`** (a 42 §3.10
    /// > erratum). T-4e calls no gate and no verdict exists -- carried through to the wire in the
    /// > shape that keeps **the ban on minting an empty digest (M4H4-2)**, symmetric with the journal
    /// > side's E-M5-7 (`verdict_digest=Option`).
    ///
    /// The three alternatives were each a way of writing down something untrue: minting an empty
    /// `AdmitProof` and digesting it (a proof for an admission no gate made — §32 M4H4-2 refused
    /// that twice), making `proof_digest` optional inside a present summary (a third state,
    /// "a verdict exists but there is no proof"), or refusing the commit (which is what hand 4 did, and
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
    /// 🔴 **Superseded by `req/38_ERRATA_2026-08-07.md` §43 (M5H6-8 ① adopted (a)), implemented in the M5
    /// fix hand.** The paragraph above is kept rather than rewritten (no-delete): it records what
    /// was true between hand 6 and this hand, and *why* the rule was not written then. The ruling,
    /// verbatim:
    ///
    /// > **M5H6-8 ① adopted (a), implementation window = fix batch**: add the paired rule "`verdict=None` ⇒
    /// > `fail_posture_engaged=true`" to gx-witness's `check_schema` (a fail-closed double defense,
    /// > equivalent to a 42 §3.10 erratum).
    ///
    /// [`ReceiptPayload::check_schema`] now carries it. The producer's refusal stays where it is;
    /// the schema is the second defence, for payloads that reach this crate from a decoder rather
    /// than from gx-engine.
    pub verdict: Option<VerdictSummary>,
    /// 42 §3.10: "`false` is non-enforced application under record-only mode (DR-2)". Not schema-checked; see this
    /// module's header.
    pub enforced: bool,
    /// 🔴 **`req/493` §0 / AC-6** — whether the kernel was holding the process that produced this,
    /// and which ruleset it was holding. See [`ConfinementContext`] for the two fields and for why
    /// this is orthogonal to the field above rather than a value of it.
    ///
    /// # What the `None` means, and why it is not a third value of the bit
    ///
    /// **"Written by a build older than this erratum."** Every receipt this binary issues carries a
    /// `Some` — an unconfined run says `kernel_confined: false`, which is a sentence about the run —
    /// so a `None` reaching a reader has come off bytes that had no such key. That is DR-46-34's
    /// shape one field along, and `tests/confinement_attest.rs` asserts the unconditional `Some` on
    /// the producing side so the two spellings cannot quietly become interchangeable.
    ///
    /// # `Option` **and** `#[serde(default)]`, and the reason is `req/38` §294 ruling 2
    ///
    /// DR-46-28 added `determinism_boundary` as **required with no default** and a receipt this
    /// product issued in August 2026 stopped decoding — signature and anchor still valid, `decode`
    /// alone dead — which §294 registered as a break of the third pillar ("verifiable for ever
    /// without the issuer"). The remedy it named for that field is the shape this one is born in:
    /// a decoder handed bytes with no `confinement` key reads `None` and goes on, so this erratum
    /// adds a **fifth** row to 42 §3.10's table without adding a second document to that limit.
    ///
    /// 🔴 **Which of the two actually carries the compatibility was measured, not assumed, and it
    /// is the `Option`.** The attribute was removed and
    /// `tests/confinement_attest.rs::ac6_bytes_with_no_confinement_key_still_decode` **stayed
    /// green** — `req/519` §7-4 already recorded the same thing from the other direction, which is
    /// why `frozen_receipt_corpus.rs` refuses *both* spellings for the members that must stay
    /// required. So the attribute is not load-bearing here; it is the declaration made explicit,
    /// and it survives a codec that does not treat `Option` the way this one does. What holds it in
    /// place is a source scan (`ac6_the_compatibility_default_is_declared_in_the_source`) rather
    /// than the decode probe — stated here because a comment claiming the attribute is what keeps
    /// August's receipts readable would be a claim this lane measured to be false.
    ///
    /// `docs/LIMITS.md`'s declared set — the members added required with no default — is therefore
    /// **unmoved**, and `crates/gx-witness/tests/frozen_receipt_corpus.rs` is the machine that
    /// refuses a lane which moves the page and the constant apart.
    ///
    /// # The rebuild roads reproduce this from Σ, and that is what put it in `Environment`
    ///
    /// 43 §7-3b compares a rebuilt payload's digest against the leaf the ledger already holds, so a
    /// field the rebuild cannot reproduce would answer `payload_mismatch` — the word for tampering
    /// — for every crash-window recovery of a confined commit. Unlike `determinism_boundary` this
    /// value cannot be derived from `verdict`: it is a fact about the process, and the process that
    /// repairs is not the process that committed. So it is **journalled**, in the record M5-25
    /// already writes before the world moves (`ProvenanceDerived` →
    /// [`crate::provenance::Environment::confinement`]), and both rebuild roads read it out of
    /// `StateRow::provenance` exactly as `read_set` is read out of the escrow row.
    #[serde(default)]
    pub confinement: Option<ConfinementContext>,
    /// 🔴 **DR-46-39** — which catalogue version governed this receipt, third-party-verifiable
    /// (`req/777` §1, ruling `req/38` §5689: "(a)-family receipt-carry", not a self-hash slot).
    ///
    /// # Carry, not compute-here
    ///
    /// `gx-witness` does not depend on `gx-adapter-mcp` (`req/777` §2, checked against the
    /// manifest rather than assumed) and this field does not change that: the digest of a
    /// `Catalogue` value is minted by whichever crate holds both the catalogue and the
    /// receipt-construction site, through `gx_canon::cid` (AC-014's only road to a digest), and
    /// handed here as opaque text. `gx-witness` carries the field; it does not know what a
    /// `Catalogue` is.
    ///
    /// # `String`, not `Cid`, and the reason is `ConfinementContext::ruleset_hash`'s own
    ///
    /// The identical shape already exists on this struct one field up: `ruleset_hash` is a
    /// `String` "because it is not one... carrying it in the `Cid` type here would put it in the
    /// same namespace as the ids this payload joins on" (`ConfinementContext`'s own doc comment).
    /// `catalogue_hash` joins on nothing else in this payload either, so it follows the same rule.
    ///
    /// # `None` is not a third value of a bit
    ///
    /// `None` means "no catalogue was named as governing this receipt" — every receipt this build
    /// issued before a caller starts minting one, and every receipt from a build that predates
    /// this erratum entirely (`#[serde(default)]` below is what keeps those decoding — `req/294`
    /// ruling 2's remedy, applied here as it was for `determinism_boundary` and `confinement`).
    #[serde(default)]
    pub catalogue_hash: Option<String>,
    /// 🔴 **DR-46-24(A)** — what the escrow read, at a granularity the reader can tell apart.
    ///
    /// `None` on a `VerdictReceipt` always, and for the same reason `inverse_delta` is: the escrow
    /// runs at 43 T-10b, during commit, so at verdict time there is nothing read to attest.
    /// [`ReceiptPayload::check_schema`] holds that.
    ///
    /// # What this does **not** cover, said here because the field's name invites the other reading
    ///
    /// This is the set of objects **gx itself** read to build an inverse — one object, on the road
    /// `req/350` §1 measured. It is **not** the agent's read traffic: `gx_mcp_wire`'s method table
    /// classes `resources/read` and its siblings as `Passthrough` and keeps nothing, and `req/38`
    /// §236 ruling 2 split that off as DR-46-25 with its cost unmeasured. So the cross-object
    /// question — did B read what A wrote — is **not** decided by this field, and the claim that a
    /// read-set attest makes selective undo well-defined does not follow from it. `docs/LIMITS.md`
    /// says so in the same words.
    pub read_set: Option<ReadSet>,
    /// 🔴 **DR-46-26 / DR-46-13** — C-25's three-valued answer, in the **signed bytes**.
    ///
    /// # The defect this closes, in `req/38` §198 ruling (b)'s own words
    ///
    /// > **A-4 is judged half done**: `unknown` reaches the adapter's return value, the refusal
    /// > sentence and the probe — **the receipt payload is still the same shape as `false`** →
    /// > **DR-46-13 raised** (a seventh `InverseStatus` word, or a field added to 42 §3.10 — a
    /// > change to a frozen face, Lean-side confirmation included).
    ///
    /// D24 seated the seventh word (`InverseStatus::Undetermined`) and DR-46-26 gives it a writer,
    /// which closes the escrow row, the API and the CLI. It does **not** close this: a reader who
    /// holds only the receipt still sees `inverse_delta: null` for both "no inverse exists" and
    /// "nobody found out". The remedies differ — the first is a property of the change, the second
    /// is a read that did not answer under a posture the deployment chose and can unchoose — so
    /// the two facts are given two shapes here rather than one.
    ///
    /// # `None` on a `VerdictReceipt`, for `inverse_delta`'s and `read_set`'s reason
    ///
    /// The question is answered by the escrow, the escrow is 43 T-10b, and T-10b is inside commit:
    /// at verdict time nothing has asked. [`ReceiptPayload::check_schema`] holds that, which makes
    /// this the third field with the same kind-dependent rule and the same one-line justification.
    ///
    /// # `Option` on a `CommitReceipt` too, and what the option means there
    ///
    /// `None` is "this commit's receipt was written by a road that had no answer to carry" — a
    /// rebuild for a transformation whose escrow row the journal no longer holds. It is not a
    /// fourth value of C-25: `Some(Reversibility::Unknown)` is the "nobody found out" answer, and
    /// the absence is the absence of an answer at all. `tests/inverse_status_wire.rs` pins the two
    /// apart on the wire.
    ///
    /// 🔴 **The rebuild roads reproduce this rather than reading it back**, and that is not a
    /// nicety: 43 §7-3b compares a rebuilt payload's digest against the leaf the ledger already
    /// holds, so a road that could not reproduce one of fourteen fields would refuse every
    /// crash-window recovery of a commit that had one. `gx-engine` derives the value from the
    /// escrow row (journalled) and re-derives `read_set` from `InverseEscrowed.reads` (journalled
    /// by the same erratum). `crates/gx-cli/tests/model_a_probes.rs` measured the first shape of
    /// this — both seats taken from the filed receipt — answering `payload_mismatch`, because R13
    /// closes a row from a filed receipt *before* the rebuild is attempted and the rebuild road is
    /// therefore the road on which no filed receipt exists.
    pub reversibility: Option<Reversibility>,
    /// 🔴 **DR-46-45 (`req/973` §B-1 + §B-2)** — if this receipt is an undo's, what it undoes and
    /// whether the CAS ran. See [`UndoAttestation`].
    ///
    /// # `None` has two readings here and they are the same reading
    ///
    /// `None` means "this receipt is not a committed undo's" — which covers every ordinary commit,
    /// every verdict receipt, and every receipt signed before this erratum. That is deliberately
    /// **not** a third value of the disposition: "we did not check" is `Unobservable`, spelled out,
    /// inside a `Some`. A reader who finds `None` learns that no undo road wrote this payload, and
    /// a reader who finds `Some` learns which of the two things happened. The state `req/973` §B-1
    /// says a third party could not previously distinguish — "checked and restored" versus "fired
    /// without checking" — is exactly the two arms of the `Some`.
    ///
    /// # Kind-dependent, and the rule is `read_set`'s rather than `confinement`'s
    ///
    /// Always `None` on a `VerdictReceipt`, held by [`ReceiptPayload::check_schema`]. The reason is
    /// not `read_set`'s (that the escrow has not run yet) — `parents` is fixed at T-2 and a verdict
    /// receipt for `T_u` could name it. It is that this field's other half is a claim about a
    /// **write**: a CAS that guarded an application. A verdict receipt applied nothing, so half of
    /// the pair would be a sentence about an event that had not happened. Keeping it commit-only is
    /// also what makes the DAG gate exact rather than approximate: the receipt-borne edge set is
    /// then *precisely* the set of undos that committed, which is the set the journal's `Superseded`
    /// records enumerate. `Planned.parents` is a strict superset of both — it is written for undos
    /// that are later denied or aborted, and the `--retry` road (`req/38` §98 ruling 2) writes two
    /// `Planned` records for one committed undo. `req/973` §B-2's AC named `Planned.parents` as an
    /// equality partner; that is corrected to a containment in `req/973` §8.
    ///
    /// # The rebuild roads reproduce this from Σ
    ///
    /// 43 §7-3b digests a rebuilt payload against the leaf the ledger holds, so a field the rebuild
    /// cannot reproduce answers `payload_mismatch` — the word for tampering — on every crash-window
    /// recovery of an undo. The witness is not derivable from Σ (it is a fact about a comparison
    /// this process made against the live world), so it is **journalled**, in the `Planned` record
    /// that already carries `parents` and `input_generation` for this exact reason
    /// (`gx_engine::store::EngineJournalRecord::Planned`). Both roads read it back through
    /// `Engine::journalled_undo`, exactly as `determinism_boundary` reads
    /// `journalled_input_generation`.
    ///
    /// `Option` **and** `#[serde(default)]`, for `confinement`'s reason (`req/38` §294 ruling 2):
    /// a decoder handed bytes with no `undo` key reads `None` and goes on, so `docs/LIMITS.md`'s
    /// declared set of members-added-required-with-no-default does not move.
    #[serde(default)]
    pub undo: Option<UndoAttestation>,
    /// 🔴 **DR-46-28** — where the replay-deterministic part of this change ends and the
    /// LLM-originated part begins, in the **signed bytes**.
    ///
    /// # What was missing, in `req/38` §255 ruling 4's own words
    ///
    /// > *This far is deterministic (replayable), from here on it is LLM-originated* -- **put it on
    /// > the face of the receipt** (the sibling of the cannot-be-established contract). 42 as it
    /// > stands has **zero** determinism-boundary fields, confirmed by grep.
    ///
    /// (The ruling is written in Japanese; the block above is its content, and `req/38` §255 is the
    /// source a reader should go to for the wording.)
    ///
    /// The nearest thing that existed was `Transformation.actor` — `Actor::Agent { key, model }`
    /// says an agent caused the change. It does not say **what the agent's contribution was**, and
    /// that is the whole question: an agent that wrote an input which gx then gated is a different
    /// object from an agent whose output was applied unexamined, and until this field a receipt
    /// spelled the two the same way. The actor is also not on the receipt at all — a receipt
    /// carries `transformation: TransformationId`, a join key, so a reader holding one receipt
    /// could not reach the actor even to under-read it.
    ///
    /// # Not an `Option`, and the reason is one erratum old
    ///
    /// `unknown` is a **first-class value** here (`req/459` ruling 3), so an `Option` would rebuild
    /// the defect DR-46-26 spent a lane closing: two shapes, `null` and `Unknown`, for one fact, and
    /// a reader who cannot tell "nobody established it" from "the field was not written". One
    /// shape, four values, and [`DeterminismBoundary::Unknown`] is what a road with no answer says.
    ///
    /// # The three rules [`ReceiptPayload::check_schema`] holds, and why each is not a convention
    ///
    /// `req/459` ruling 4 makes the acceptance test "each value of the taxonomy has a bed that
    /// makes it false, and the bed is refused". Three of those beds are payload-local, so they are
    /// refused here rather than described here:
    ///
    /// 1. **A receipt may not claim its verdict derivation was LLM-originated.** gx derives
    ///    verdicts; `req/454`'s DR-46-27 machinery is what holds that derivation to "same input,
    ///    same verdict". So [`DeterminismBoundary::LlmOriginated`] — which claims *both* stages —
    ///    is a statement about a stage gx performed and did not perform that way. The value stays in
    ///    the vocabulary for the **declaration** face, where a tool class gx never gates can carry
    ///    it honestly.
    /// 2. **No verdict, no determinism claim.** 43 T-4e reaches a commit with `verdict: None` by
    ///    calling no gate at all. A payload that claims a replay-deterministic verdict derivation
    ///    there is claiming a property of a derivation that did not happen — the same shape as
    ///    M5H6-8①'s pairing rule two fields up, and refused for the same reason.
    /// 3. **`Mixed` must actually mix.** `req/459` ruling 3 words `mixed` as *enumerated by stage*; a `Mixed`
    ///    whose two stages are equal enumerates one class twice and is the collapsed value wearing
    ///    a different name. [`DeterminismBoundary::of_stages`] never produces one, and a decoder
    ///    that hands one in is refused.
    ///
    /// The fourth bed — `unknown` minted over stages that *were* established — is not payload-local
    /// (the collapsed `Unknown` carries no stages to inspect) and is refused by the arithmetic
    /// instead: `of_stages` cannot return `Unknown` unless both stages are `Unknown`.
    ///
    /// # 🔴 What this field is **not**, said here because the name invites the other reading
    ///
    /// It is not an input to anything. The boundary is written **after** the verdict exists and is
    /// never read back into a decision — a field that attested determinism while participating in
    /// the derivation it attests would be the self-reference `req/444` §1 warns about. That is not
    /// a promise made in this comment: `tests/boundary_attest.rs` scans `gx-gate` for the name and
    /// counts the gate's declared inputs, which is the same instrument `req/454` used one erratum
    /// earlier for `decided_at`.
    ///
    /// And, `req/444` §1's counter-argument forward: **`deterministic_replay` means "replaying the same
    /// input yields the same verdict" and nothing wider.** The engine has clocks, key generation
    /// and filesystem order in it. None of them reach the answer; all of them are still there.
    pub determinism_boundary: DeterminismBoundary,
    /// Which of ASM-14's two shapes this payload claims to be; [`ReceiptPayload::check_schema`]
    /// holds the kind-dependent field rules to it.
    pub receipt_kind: ReceiptKind,
    /// 42 §3.10: `Transformation.id`. Checked against [`ReceiptPayload::transformation`] by
    /// [`verify_offline`] -- see [`Checks::canonical_cid`] for what that check can and cannot say.
    pub canonical_cid: Cid,
    /// 42 §3.10: the escrowed inverse delta's CID. `None` on a `VerdictReceipt` always, since
    /// escrow happens in 43 T-10b, during commit.
    pub inverse_delta: Option<Cid>,
    /// The transformation the receipt witnesses (42 §3.10) -- the join key every other record
    /// of the same act shares.
    pub transformation: TransformationId,
    /// 42 §3.10 / ASM-14: required on a `CommitReceipt`, absent on a `VerdictReceipt`.
    pub inclusion_proof: Option<InclusionProof>,
    /// 🔴 **P2 / DR-46-24(A)** — what the two fingerprints below were taken **over** (42 §3.5's
    /// `scope`).
    ///
    /// # The hole this closes, in the engine's own words
    ///
    /// `req/350` §3's predicate table has six rows and two of them were answered before this field.
    /// P2 — *what did that precondition fingerprint cover* — was not, and the consequence is
    /// written down at `crates/gx-engine/src/pipeline.rs:5347`, at the undo road's compare-and-set:
    ///
    /// > The comparison is on the **digest** alone rather than through `Fingerprint::cas_eq`, and
    /// > that is forced rather than chosen: 42 §3.10 stores a `postcondition_fingerprint` as
    /// > `FingerprintBytes` — 32 bytes with no substrate and no scope — so the two other components
    /// > `cas_eq` insists on are simply not in the receipt.
    ///
    /// That is `req/337` §3's three conditions met by one field: the value is **observed** (every
    /// `Fingerprint` carries it), **unattested** (`grep scope crates/gx-witness/src/receipt.rs`
    /// answered zero before this hand), and it **changes a decision** — a CAS run through
    /// `cas_eq` refuses a comparison across scopes, and a CAS run on digests alone cannot.
    ///
    /// # Why one field and not two, established rather than assumed
    ///
    /// `req/440` §0-4 makes one field the default and asks for the invariant to be checked before
    /// relying on it: that `precondition_fingerprint` and `postcondition_fingerprint` are always
    /// over the same scope. Three machines already hold it, and none of them was added here —
    /// `tests/read_set_wire.rs` pins all three by name:
    ///
    /// 1. `gx_core::Fingerprint::cas_eq` returns `Err(FingerprintScopeMismatch)` rather than a
    ///    boolean when two scopes differ, so no scope-crossing pair is ever *compared*, only
    ///    refused.
    /// 2. `pipeline.rs`'s T-10a runs `fp0.cas_eq(&fp1)` and turns that `Err` into
    ///    `Aborted(InternalError)` (M5-24 adopted (a)), so a transformation whose scope moved does
    ///    not reach the commit receipt at all.
    /// 3. `gx_substrate_conformance::laws` compares `applied.postcondition()` against a fresh
    ///    `precondition` through `cas_eq` too, so an adapter that moved the scope across its own
    ///    `apply` fails 51 §7 rather than shipping.
    ///
    /// The type is a `String` and not an `Option`: `precondition_fingerprint` is not optional
    /// either, and every `Fingerprint` has a scope. **`FingerprintBytes` is untouched** — it is
    /// still the opaque 32 bytes E-M2-2 made it, because widening it would put a scope inside a
    /// type whose whole contract is that it is carried and never interpreted.
    pub fingerprint_scope: String,
    /// **E-M2-7**, added: 35 ASM-13 and 43 T-4e require it and 42 §3.10's table has no room.
    /// Deterministic, so it is inside the signed core.
    pub fail_posture_engaged: bool,
    /// Fingerprint₀ (42 §3.10), as opaque bytes until M4 (**E-M2-2**).
    pub precondition_fingerprint: FingerprintBytes,
    /// 42 §3.10: set only after something was applied, so `None` on a `VerdictReceipt`.
    pub postcondition_fingerprint: Option<FingerprintBytes>,
    /// 🔴 **F7 (`req/871` §1.7, registered `req/868` **R-868-6**) — the receipt-format version field
    /// that did not exist.** `RECEIPT_PAYLOAD_TYPE` carries no version component and the payload
    /// schema had already moved (at least) four times under one type name before this field existed
    /// -- `tools/receipt_generation_gate.mjs`'s derived generation identity (SS858 §⑤, sha256 over
    /// the sorted member set) is the machine-checked stopgap this workspace built for *itself*
    /// while this field did not exist, and that gate's own header says plainly what it does not do:
    /// "it does not put a version on the wire... a third party holding a receipt still cannot tell
    /// which generation wrote it". This field is the remedy that closes that sentence.
    ///
    /// # `Option<u32>`, and why this is not `DeterminismBoundary`'s question again
    ///
    /// `DR-46-28` (this file, `determinism_boundary`) is deliberately **not** an `Option`, because
    /// `unknown` there is a first-class fact about the world -- a receipt can truthfully say "nobody
    /// established which side of the boundary this crossed" -- and folding that fact into `null`
    /// would give one fact two spellings (`null` and `Unknown`), which is exactly `DR-46-26`'s
    /// defect reproduced. `payload_version` asks a different question: not "what happened", but
    /// "which shape of this very struct wrote these bytes". There is no meaningful third state
    /// between "a generation number was written" and "this predates the field" -- absence has
    /// exactly **one** honest reading, the same shape `confinement`'s and `catalogue_hash`'s
    /// `None` already have on this struct (`None` = "the bytes predate the erratum that asks"),
    /// not `determinism_boundary`'s. `#[serde(default)]` keeps every receipt this workspace ever
    /// signed decodable: a DAG-CBOR map missing this key is not malformed, it is simply older.
    ///
    /// # What this build emits, and what it does not retroactively claim
    ///
    /// This binary always writes `Some(`[`CURRENT_PAYLOAD_VERSION`]`)`. That constant starts at
    /// `1` and is **not** a claim about the four historical generations `tools/receipt_generation_gate.mjs`'s
    /// own comment names -- those were never numbered and this field cannot reach backward to
    /// number them; it can only make every generation **from here forward** self-identifying. The
    /// gate's own sha256-derived identity remains the mechanism that tells *this workspace* whether
    /// the struct's shape moved since it was last registered; `payload_version` is the orthogonal,
    /// hand-assigned integer that tells a **third party holding one signed receipt, with no access
    /// to this repository's history,** which shape it is reading. The two are deliberately not tied
    /// together (one is derived and cannot go stale by construction, the other is a small integer a
    /// human bumps on a breaking change and can) -- `check_schema` does not enforce agreement
    /// between them because nothing about a decoded receipt lets a reader ask the gate anything.
    ///
    /// # Kind-independent, and why
    ///
    /// The struct's *shape* does not change between a `VerdictReceipt` and a `CommitReceipt` --
    /// only which fields are populated does -- so this is carried on both, unconditionally, the same
    /// way `receipt_kind` itself is: it answers a question about the payload, not about the
    /// transformation's outcome.
    ///
    /// # 🔴 Recorded, not silently decided: a primary source called the root remedy Owner-gated
    ///
    /// `req/38` SS858 §⑤ (2026-08-26), in the same breath that built the sha256 stopgap above,
    /// wrote: "F7の根本remedy(wireにversionを載せる)はDRでOwner gate" -- read narrowly, as a claim
    /// that *landing this field* needed the Owner's own hand, that sentence conflicts with
    /// `req/919` §3 W5, which lists this exact field addition among the batch's non-Owner-gate
    /// items. This crate's CC recommendation, recorded here rather than decided silently: every
    /// other wire-additive field this struct carries (`read_set`, `fingerprint_scope`,
    /// `reversibility`, `determinism_boundary`, `confinement`, `catalogue_hash`) was landed by a
    /// lane/Fable ruling (`DR-46-2x`, `req/38 §NNN裁定`) without literal Owner sign-off, and this
    /// workspace's own definition of the Owner-gate ceiling (`~/.claude` doctrine's "推奨自走":
    /// publish / push / DOI / financial / legal / key rotation / destructive KILL) does not name
    /// wire-additive schema design among it -- push and publish of anything built on this field
    /// remain Owner-gated exactly as `req/38:2819` fixes unconditionally ("publish/push=Owner
    /// gate(不変)"), and this lane neither commits nor pushes. The SS858 sentence most plausibly
    /// reflects that lane's own time-boxed choice not to spend its remaining cargo budget on the
    /// real field, phrased as a hedge, rather than a canon ruling that a lane may never land this
    /// field -- but it is quoted here **verbatim** rather than paraphrased away, so a reviewer who
    /// reads it the stricter way has everything needed to override this decision without re-deriving
    /// it.
    ///
    /// 🔴 **`req/919` W8 (2026-08-30), additive correction — the attribute this field's own doc and
    /// its own test both named was never on it.** The paragraph above says "`#[serde(default)]`
    /// keeps every receipt this workspace ever signed decodable", and
    /// `tests/r868_payload_version_attest.rs`'s failure message says "if this line is the failure,
    /// `ReceiptPayload::payload_version` has lost its `#[serde(default)]`" -- and W5 landed the
    /// field without it. The test was green anyway, and for a reason neither text names: serde's
    /// derive routes a missing field through `missing_field`, which **succeeds for any type that
    /// deserialises from `None`**, so an `Option` decodes from absent bytes with or without the
    /// attribute. The compatibility claim was therefore true and its stated mechanism was not, which
    /// is the shape this workspace calls a green that lies. Adding the attribute here makes the two
    /// texts true rather than editing them to describe the accident: it is what `confinement` and
    /// `catalogue_hash` already carry, and it is what keeps the promise if this field is ever
    /// wrapped in a type that is not an `Option`.
    #[serde(default)]
    pub payload_version: Option<u32>,
    /// 🔴 **A2 (`req/910` A. / `req/38` SS830, `req/919` W8, 2026-08-30) — which engine build
    /// signed this.** The north star's (`#435`) core question, seated on the one artefact a third
    /// party actually holds.
    ///
    /// # What was already true, and what was not — the ledger row this corrects
    ///
    /// `req/910` A2 reads, in the ledger's own Japanese, that this field "is never captured and
    /// never rendered" (the original wording is in `req/910` section A. and is not restated here:
    /// this crate's comments are English by the workspace's third principle, and the ledger row is
    /// one grep away), inherited from `req/38` SS830. **Half of that is stale and this field is the
    /// half that was not.** The
    /// engine has captured a version since M5-25: `Engine::derive_provenance` writes
    /// [`crate::provenance::Environment::engine_version`] into the `ProvenanceDerived` record
    /// **before the world moves**, and `GET /healthz` has rendered it since M6H5-12 (44 §2.2,
    /// pinned by `gx-api/tests/m6h7_api.rs`). What was missing is the binding SS830 actually asked
    /// for: **the receipt could not say it**. Provenance lives in Σ (`StateRow::provenance`) and Σ
    /// is this repository's; a reader holding one signed document offline — the only reader the
    /// four pillars promise anything to — had no key to read for it. So the honest statement of
    /// the defect is not "never captured" but "captured everywhere except on the wire", which is
    /// the same shape F7 had and is closed the same way.
    ///
    /// # 🔴 Read out of Σ, never out of this process — the constraint that picks the value
    ///
    /// This is [`ReceiptPayload::confinement`]'s rule and it is not stylistic. 43 §7-3b compares a
    /// **rebuilt** payload's digest against the leaf the ledger already holds, and the process that
    /// repairs is not the process that committed. A rebuild road that answered this from its own
    /// `gx_engine::VERSION` would report `payload_mismatch` — the word for tampering — on every
    /// crash-window recovery performed by a build other than the one that committed, which is the
    /// ordinary case for the upgrade 47 §4 describes. The value therefore comes from
    /// `row.provenance.environment.engine_version` on both rebuild roads, exactly as `read_set`
    /// comes out of the escrow row, and from the live constant only where `confinement` also takes
    /// it live (T-4a and T-11, where the signing process *is* the deriving process).
    ///
    /// # `Option<String>`, and what `None` means
    ///
    /// One honest reading, as with `confinement`, `catalogue_hash` and `payload_version`: **"these
    /// bytes predate the erratum that asks"**. `#[serde(default)]` keeps every receipt this
    /// workspace has ever signed decodable (`req/38` §294 ruling 2 registered the cost of the
    /// alternative in real receipts that stopped decoding). There is a second, rarer `None` on the
    /// rebuild roads — a journal written before M5-25 carries no `ProvenanceDerived` — and it reads
    /// the same way for the same reason: reproducing an absence rather than inventing a version
    /// nobody recorded. A `String` rather than a structured type because 42 §3.9 already types the
    /// value it mirrors as one, and a receipt that spelled the same fact differently from the
    /// provenance it is rebuilt from would fail the digest comparison above.
    ///
    /// # 🔴 What this does **not** close, stated here rather than discovered later
    ///
    /// The seat is closed; the **question** is not. Today the value is `gx_engine::VERSION`, which
    /// is `env!("CARGO_PKG_VERSION")` = the workspace version, and it has read `0.1.0` for every
    /// build this project has ever produced. So a receipt now names *a* version and still cannot
    /// distinguish two builds of it — `#435`'s "which implementation answered" is **narrowed, not
    /// answered**. Naming that here is the point: a field that looks like the answer while not
    /// being it is worse than the gap, and this workspace's own rule is that the disclosure lives
    /// where the mechanism lives.
    ///
    /// A build script minting a git hash was considered and **rejected for now**, on three grounds
    /// a later lane can overturn with evidence rather than taste. (1) There is no `build.rs`
    /// anywhere in this workspace; adding the first one is a new mechanism, not a mirror of an
    /// existing one, and `req/38` SS856's cost split puts that in the other category. (2) A git
    /// hash is not available when the crate is built from a published tarball, which has no `.git`
    /// — so the value would differ between a CI build and a third party's rebuild of the same
    /// source, and a *verifier* reproducing a receipt is precisely who this field is for. (3) The
    /// seat is source-agnostic: whenever a build identity does land, it lands in
    /// `Engine::derive_provenance`'s one line, and this field, the spec row, the fixtures and the
    /// wire shape do not move. Landing the seat first is therefore the cheap half done first, and
    /// the residual is registered rather than absorbed.
    ///
    /// # No cross-field rule in `check_schema`
    ///
    /// Deliberately, and for W5's reason: a rule pairing this with `payload_version` would reject
    /// hand-built combinations nobody has observed while buying no fail-closed guarantee, since a
    /// forger who can set one key can set both. The producer is the defence.
    #[serde(default)]
    pub engine_version: Option<String>,
}

/// The `payload_version` this build writes into every `ReceiptPayload` it constructs. See the
/// field's own doc comment for what incrementing this does and does not claim.
pub const CURRENT_PAYLOAD_VERSION: u32 = 1;

/// 42 §1.3: "`ReceiptPayload` | all fields | — | Receipt is a meta-witness and has no exclusion rule
/// (`Receipt` itself is what is signed)".
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
    /// # 🔴 This is a derivation, and the reason is a circularity in the canonical source
    ///
    /// 42 §3.11 defines the field as "the BLAKE3 digest of `Receipt` (the whole of the DSSE envelope bytes)". For a
    /// `CommitReceipt` that value cannot exist. Three statements are each clear and are together
    /// unsatisfiable:
    ///
    /// 1. 43 T-11 orders the commit as "`ledger.append(...)` → `InclusionProof`; Receipt issued" --
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
    /// `ReceiptPayload` ("all fields") applied to the value as it stood before the ledger answered.
    /// Two things are therefore outside the ledger's commitment, and each has a precedent in a
    /// ruling already made:
    ///
    /// * **the signatures**, which 42 §1.3-4 already keeps out of a `ReceiptPayload`'s identity
    ///   ("signatures are excluded ... what is signed is the payload bytes"). A leaf that covered them could not be written
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
    /// **Not ruled** — was true through R41; superseded by the line below.
    ///
    /// 🔴 **Ruled (`req/38` §337, `req/565` §2) — option (a), erratum.** 42 §3.11's literal
    /// wording is unchanged (42 §3.10 and 43 T-11 are untouched too), and the value this function
    /// computes is recorded directly under 42 §3.11's table as a no-delete addendum
    /// (`req/spec/40-architecture/42-data-model.md`, right after the `LedgerLeaf`/`Tile` rows).
    /// `req/54` §4 H5-1 is closed by that addendum in the sense of "recorded", not in the sense
    /// of "the self-reference is resolved" — 42 §3.10 and 43 T-11 still have to move for the
    /// three-statement circularity itself to go away, and this erratum deliberately does not move
    /// either.
    ///
    /// # Errors
    /// [`Error::Canon`] if the payload has no canonical form.
    /// 🔴 **`req/38` §324 ruling 3 — this is the producer's road, and only the producer's.**
    ///
    /// A value this build constructed has one canonical form and this computes it. A value that
    /// arrived as **bytes** does not: it decodes into this year's type, whose canonical form is not
    /// the form those bytes were written in, and taking this road on one of those is the defect
    /// §324 sent a lane back for. [`ledger_digest_of_signed_payload`] is the road for those, and
    /// every consumer in this workspace takes it.
    ///
    /// The two agree, by construction, for every receipt this build issues —
    /// `tests/leaf_from_signed_bytes.rs` asserts the agreement rather than assuming it.
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
    /// req/49 §4 predicted would "first take real form here, as a schema check" -- the conditional-
    /// requirement shape gx already meets in a fabrication-status YAML gate, now in a type.
    ///
    /// # The one rule that is not ASM-14's, and why it is here (**M5H6-8①**)
    ///
    /// `verdict = None` ⇒ `fail_posture_engaged = true`. 42 §3.10 states no such pairing and
    /// `req/38_ERRATA_2026-08-07.md` §43 rules it in anyway, as "a fail-closed double defense, equivalent
    /// to a 42 §3.10 erratum". It is kind-independent, so it is checked before the kind is looked at: a
    /// receipt of either kind that carries no verdict is claiming 43 T-4e's degraded admission, and
    /// T-4e's own cell requires "always carve `fail_posture_engaged=true` into the receipt". The absence of
    /// both is not a milder receipt; it is a receipt that names no reason for having skipped the
    /// gate.
    ///
    /// One direction only. `fail_posture_engaged = true` **with** a verdict is legal and must stay
    /// legal — an operator running a degraded posture whose gate did answer produces exactly that,
    /// and `tests/receipt_kind_branch.rs::both_postures_are_legal` has held that half since M2.
    ///
    /// # Errors
    /// [`Error::Schema`], naming the field and the kind. AC-070 asks for exactly this on a
    /// `CommitReceipt` with no inclusion proof: "a schema violation or `Ok(false)`".
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
                 requires \"always carve `fail_posture_engaged=true` into the receipt\" (§43 M5H6-8①)",
            ));
        }

        // 🔴 **DR-46-28** — the three payload-local beds of `req/459` ruling 4, before the kind,
        // because none of them depends on it. Each one is a way of writing down something the
        // boundary cannot be true of; the fourth bed (`unknown` minted over stages that were
        // established) is arithmetic and lives on `DeterminismBoundary::of_stages`.
        //
        // The order is load-bearing and is well-formedness first: a `Mixed` whose two stages are
        // equal is not a claim that happens to be wrong, it is a value that does not parse as the
        // thing its name says. Asking the two semantic questions of it first would answer about
        // half of a degenerate value and name the wrong defect in the sentence.
        //
        // (1) `Mixed` must mix: `req/459` ruling 3 words it as an enumeration by stage.
        if let DeterminismBoundary::Mixed {
            input_generation,
            verdict_derivation,
        } = self.determinism_boundary
        {
            if input_generation == verdict_derivation {
                let stage = input_generation.as_str();
                return Err(Error::Schema {
                    detail: format!(
                        "the receipt's determinism_boundary is mixed({stage}/{stage}), which enumerates one class twice: `DeterminismBoundary::of_stages` answers {stage} for those two stages and never `Mixed` (`req/459` ruling 3)"
                    ),
                });
            }
        }
        // (2) A receipt may not say gx's own derivation came from a model.
        if self.determinism_boundary.verdict_derivation() == BoundaryStage::LlmOriginated {
            return Err(Error::Schema {
                detail: format!(
                    "the receipt's determinism_boundary is {}, which says this verdict was derived from a model: gx derives verdicts, and DR-46-27's machinery holds that derivation to \"same input, same verdict\". The value belongs on the declaration face (`req/459` ruling 3), not on a receipt",
                    self.determinism_boundary.as_str()
                ),
            });
        }
        // (3) No verdict, no determinism claim: 43 T-4e calls no gate at all.
        if self.verdict.is_none()
            && self.determinism_boundary.verdict_derivation() == BoundaryStage::DeterministicReplay
        {
            return Err(Error::Schema {
                detail: format!(
                    "the receipt's determinism_boundary is {} and it carries no verdict: 43 T-4e is the only road to a receipt without one and it calls no gate, so there is no derivation for the claim to be about (`req/459` ruling 4)",
                    self.determinism_boundary.as_str()
                ),
            });
        }

        // 🔴 **`req/493` §0 / AC-6** — the two pairs [`ConfinementContext`] names as not being
        // states of the world, before the kind, because neither depends on it: a verdict receipt
        // and a commit receipt are produced by the same process and the kernel is holding it or is
        // not. `req/493` §1 AC-4's rule is what makes them refusals rather than remarks — a gate
        // that has never been fired is not a gate, and `tests/confinement_attest.rs` fires each.
        if let Some(confinement) = &self.confinement {
            if confinement.kernel_confined && confinement.ruleset_hash.is_none() {
                return Err(Error::Schema {
                    detail: "the receipt says the kernel confined the process that produced it and names no ruleset: `gx_confine::ConfinePlan::ruleset_hash` is what lets a reader re-derive what was enforced from the pre-image, and a claim carried without it is the claim without the evidence (`req/493` §1 AC-6)".to_string(),
                });
            }
            if !confinement.kernel_confined && confinement.ruleset_hash.is_some() {
                return Err(Error::Schema {
                    detail: "the receipt names a confinement ruleset and says the kernel confined nothing: a plan can be derived without being applied (`gx confine --plan-only` does exactly that), and what a receipt may not do is name one as though it had held (`req/493` §1 AC-6)".to_string(),
                });
            }
        }

        match self.receipt_kind {
            ReceiptKind::VerdictReceipt => {
                if self.inclusion_proof.is_some() {
                    return Err(refuse("an inclusion proof", "always absent: always `None`"));
                }
                if self.postcondition_fingerprint.is_some() {
                    return Err(refuse(
                        "a postcondition fingerprint",
                        "always absent: always None on `VerdictReceipt`",
                    ));
                }
                if self.inverse_delta.is_some() {
                    return Err(refuse(
                        "an inverse delta",
                        "always absent: escrow is 43 T-10b, during commit",
                    ));
                }
                // 🔴 **DR-46-24(A)** — `req/350` §7-3 asked for the kind-dependent rule by name.
                // The read-set is what the escrow read, the escrow is T-10b, and T-10b is inside
                // commit: a verdict receipt claiming one is claiming a read that had not happened
                // when it was signed.
                if self.read_set.is_some() {
                    return Err(refuse(
                        "a read-set",
                        "always absent: the escrow that reads is 43 T-10b, during commit",
                    ));
                }
                // 🔴 **DR-46-26** — the third field with this rule, and it is the same rule.
                // C-25's answer is what the escrow found out; the escrow is T-10b; T-10b is inside
                // commit. A verdict receipt carrying one would be reporting an answer to a question
                // nothing had asked when it was signed.
                if self.reversibility.is_some() {
                    return Err(refuse(
                        "an inverse-status",
                        "always absent: C-25 is answered by the escrow at 43 T-10b, during commit",
                    ));
                }
                // 🔴 **DR-46-45 (`req/973` §B-2)** — the fourth field with a kind-dependent rule,
                // and the reason is *not* the other three's. `parents` is fixed at T-2, so a
                // verdict receipt could name what an undo undoes; what it cannot carry is the
                // other half of the pair, which is a claim about a compare-and-swap that guarded
                // an **application**. A verdict receipt applied nothing. Keeping the pair
                // commit-only is also what makes the receipt-borne edge set exactly the set of
                // undos that committed — the set `Superseded` enumerates — which is the equality
                // `crates/gx-engine/tests/r973_undo_attestation.rs` asserts.
                if self.undo.is_some() {
                    return Err(refuse(
                        "an undo attestation",
                        "always absent: its witness is a claim about a CAS that guarded an apply, and 42 §3.10's `VerdictReceipt` applies nothing",
                    ));
                }
            }
            ReceiptKind::CommitReceipt => {
                if self.inclusion_proof.is_none() {
                    return Err(refuse(
                        "no inclusion proof",
                        "required: mandatory (`Some`) on `CommitReceipt`",
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
/// It was, until E-M2-6 took `issued_at` out of the signed core and sent it "to unsigned envelope
/// metadata". A DSSE envelope has exactly three fields -- 42 §3.10's own table says so, and the
/// standard 42 §4 compares gx against says so -- and a fourth would make the wire form something no
/// DSSE reader parses. So the timestamp rides *beside* the envelope: [`Receipt::envelope`] is
/// exactly 42 §3.10's three fields and is what a DSSE verifier sees, and this struct is the pair.
/// The wire shape of that pair is in no canonical source and is raised in req/54 §4.
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

    /// `LedgerEntry.receipt_digest` (42 §3.11), derived from **the bytes that were signed**.
    ///
    /// Read [`ledger_digest_of_signed_payload`] before using this one, twice over: the value is
    /// **not** 42 §3.11's literal "the whole of the DSSE envelope bytes" (a circularity in the
    /// canonical source, set out at [`ReceiptPayload::ledger_digest`]), and it is not reached by
    /// decoding this receipt either.
    ///
    /// 🔴 **`req/38` §324 ruling 3** — this used to be `self.payload()?.ledger_digest()`, and that
    /// is the line three lanes in a row were sent back over. A receipt is a thing that arrives; the
    /// struct it decodes into is this build's, and re-encoding it asks what this build's schema
    /// would have written rather than what was signed.
    ///
    /// # Errors
    /// [`Error::Canon`] if the payload is not canonical DAG-CBOR.
    pub fn ledger_digest(&self) -> Result<Cid> {
        ledger_digest_of_signed_payload(&self.envelope.payload)
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
    /// AC-018's "canonical CID consistency check": `canonical_cid == transformation`.
    ///
    /// # What this check is worth, honestly
    ///
    /// Both fields are inside the signed payload, so a tamperer cannot move one without breaking
    /// the signature -- which means this catches a **producer's** bug and no attack. 42 §3.10 asks
    /// for both fields all the same (`transformation` "the target transformation", `canonical_cid` "`Transformation.id`"),
    /// and an engine that filled them from two different places could disagree. Raised in req/54 §4:
    /// the AC names a check whose adversarial value is zero, and saying so is better than reporting
    /// it as though it were a signature.
    pub canonical_cid: bool,
    /// What the inclusion-proof walk found (AC-070's "checks.inclusion"): checked against a
    /// known checkpoint, or honestly reported as not checkable for this receipt kind.
    pub inclusion: InclusionCheck,
    /// What a consulted revocation list said about the key (**FR-M7-3**).
    ///
    /// [`RevocationCheck::NotConsulted`] unless the caller took [`verify_offline_consulting`], which
    /// is the road that has a list to consult. Present on every answer rather than only when a list
    /// was given: a field that appeared only when it was interesting is a field a reader misses on
    /// exactly the runs where it matters (M6H8-11 adopted (a)).
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
    /// that as `true` is the fail-open req/29 §4 forbids -- "a skip and a pass must not look the same".
    ///
    /// 🔴 **H-09**: [`InclusionCheck::Unbridged`] is not a pass either, and for the same sentence.
    /// An anchor of a different `tree_size` with nothing bridging it leaves the ledger claim
    /// unchecked; that it is also not *refuted* changes what the operator should do about it, and
    /// changes nothing about whether the receipt has been verified.
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
/// Five values where AC-018 writes `"skipped"` and AC-070 writes `true`, because those two are the
/// two *good* outcomes and a boolean has no room for the three bad ones. The AC vocabulary is a JSON
/// field of the M6 CLI (`gx receipt verify --offline`); this is the library form it will be
/// rendered from, and the mapping is written in each variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InclusionCheck {
    /// A `VerdictReceipt`: ASM-14 says there is nothing in the ledger yet. AC-018's `"skipped"`.
    NotApplicable,
    /// A `CommitReceipt` whose proof reached the anchor's root — directly when the anchor is the
    /// tree the proof names, or through a consistency proof when it is a later one. AC-070's `true`.
    Verified,
    /// A `CommitReceipt` whose proof did not reach the anchor's root -- a forged proof, a proof
    /// against a different tree, or an anchor from a different log.
    ///
    /// 🔴 **H-09 narrowed this word.** Before the repair it also covered every receipt older than
    /// the anchor, because an inclusion proof is relative to the `tree_size` it names and a later
    /// head has a different root by construction. That is [`InclusionCheck::Unbridged`] now:
    /// `Refuted` is reserved for evidence **against** the receipt, and growth is not evidence.
    Refuted,
    /// A `CommitReceipt` verified with no anchor to check against. Not a pass: see
    /// [`Checks::verified`].
    Unanchored,
    /// 🔴 **H-09** — the anchor and the proof are about **different trees**, and nothing bridged
    /// them.
    ///
    /// The anchor commits to a `tree_size` the receipt's `inclusion_proof` does not name, and either
    /// no consistency proof was offered or the one offered was about some other pair of sizes. The
    /// ledger claim is then neither confirmed nor contradicted: the verifier holds two true
    /// statements about two trees and no link between them (RFC 6962 §2.1.2 is that link).
    ///
    /// **Not a pass** ([`Checks::verified`]) and **not a refutation**. Reporting it as `Refuted`
    /// was the false negative `req/222` measured — three commits, and the two older receipts came
    /// back as evidence of tampering — and reporting it as `Verified` would be the fail-open on the
    /// other side. It is the third thing, and it has its own word for the reason req/29 §4 gives.
    Unbridged,
}

/// What happened to the key half of a verification (**FR-M7-3**).
///
/// Five values, for [`InclusionCheck`]'s reason: "the key is fine" has three different meanings
/// here and folding them into a boolean would lose the one that matters most — that nothing was
/// consulted.
///
/// 🔴 **Why `NotConsulted` is a pass and `Unanchored` is not.** A `CommitReceipt` *claims* inclusion
/// in its own signed payload, so verifying one without an anchor leaves a claim the receipt made
/// unchecked (H5-9). A receipt makes no claim about revocation at all: the list is the **verifier's**
/// own input, and 45's ASM-45-2 makes consulting it optional ("consulting the revocation list is
/// optional, at the verifier's discretion"). So not consulting is a verifier declining an option, not a check that was skipped — and
/// the word is on the wire either way, which is M6H8-11 adopted (a)'s rule applied to the second thing a
/// verification can leave out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RevocationCheck {
    /// No list was consulted (the default, and what [`verify_offline`] always answers).
    NotConsulted,
    /// A list was consulted and holds no revocation of this key.
    NotRevoked,
    /// A revocation exists and takes effect **after** the moment of this verification, so there is
    /// nothing to apply yet. The other half of req/98 §3-2's "a verify after the revocation time".
    NotYetInForce,
    /// A revocation is in force, the setting is [`Retroaction::FromRevocation`], and the receipt is
    /// dated before it: "the key's state at the moment of issue" (ASM-45-2's DEFAULT). Valid.
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
/// injected rather than read here for 41 §6's reason ("randomness and clock reads are injected at
/// the engine boundary") and for a
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
/// > "was the key revoked at or before the receipt timestamp" (req/98 §3-2)
///
/// Four inputs and no hidden fifth, so that every answer can be reproduced from the values printed
/// beside it. The order of the arms is the order the questions have to be asked in:
///
/// 1. **no entry** — nothing to apply;
/// 2. **not yet in force** — the revocation is dated after this verification, and applying it early
///    would answer "invalid" about a receipt that is valid at the time of asking;
/// 3. **[`Retroaction::All`]** — every receipt of a revoked key is refused, and no clock is read;
/// 4. **before the revocation** — ASM-45-2's DEFAULT keeps it valid ("the key's state at the moment of issue");
/// 5. **at or after** — the key was already revoked when this receipt says it was issued.
///
/// Arm 4 believes `issued_at`, which **E-M2-6** keeps out of the signed core. That is the limit 45
/// §3 grades as TH-5's residual ("in v0.1, without TSA integration, third-party proof of the
/// revocation time is weak") and it is why
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
/// 3. **The schema**, per ASM-14's kind (AC-070's "schema violation").
/// 4. **`key_id` agrees** with the signature that was checked. 42 §3.10 requires it and nothing
///    else would notice a receipt naming one key and signed by another that a caller happens to
///    trust.
/// 5. **The canonical CID**, and the **inclusion proof** against `anchor`.
///
/// # `anchor`
///
/// AC-070 says "match against a known checkpoint" -- the verifier has to already believe a checkpoint, from a
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
    checks_of(receipt, key, anchor.map(Anchorage::at).as_ref(), None)
}

/// The same verification, **consulting a revocation list** (**FR-M7-3**).
///
/// A second entry point rather than a fifth argument on the first, because ASM-45-2 makes consulting
/// optional ("consulting the revocation list is optional, at the verifier's discretion") and every caller written before this hand
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
    checks_of(
        receipt,
        key,
        anchor.map(Anchorage::at).as_ref(),
        Some(revocation),
    )
}

/// 🔴 **H-09** — an anchor, and (when the log has moved on) what ties the receipt's tree to it.
///
/// # Why the bridge is a second field and not a second checkpoint
///
/// An `inclusion_proof` is relative to exactly one `tree_size` (`gx_log::proof::prove_inclusion_at`),
/// so the head a verifier believes today is, for every receipt but the newest, a root the proof
/// cannot reach. `req/222` measured what that cost: a project with three commits answered
/// `inclusion: "refuted"` for the two older receipts — a **false negative**, and the worst-shaped
/// one, because "refuted" is the word for tampering.
///
/// RFC 6962 §2.1.2 is the standard answer and it is already implemented
/// (`gx_log::proof::verify_consistency`, `gx log consistency`). What was missing is the *carriage*:
/// a place for the caller to put the proof that the tree the receipt names grew into the tree the
/// anchor names. This is that place.
///
/// # The chain has no unchecked link
///
/// 1. `root_at_proof_size` is **computed** from the receipt's own leaf and audit path
///    (`gx_log::proof::root_of_inclusion`) — nobody hands it in, so nobody can forge it;
/// 2. the consistency proof carries that root to `checkpoint.root_hash`;
/// 3. `checkpoint.root_hash` is what the verifier already believed (and, with
///    `--checkpoint-key`, what somebody signed).
///
/// A bridge about other sizes than `(proof.tree_size, checkpoint.tree_size)` is not weaker
/// evidence, it is evidence about something else, and it is answered as
/// [`InclusionCheck::Unbridged`] rather than folded in.
#[derive(Clone, Copy, Debug)]
pub struct Anchorage<'a> {
    /// The head the verifier believes (AC-070's "known checkpoint").
    pub checkpoint: &'a Checkpoint,
    /// RFC 6962 §2.1.2, from the receipt's `tree_size` to the checkpoint's. `None` is honest and
    /// common: a third party holding one file pair has no way to make one.
    pub bridge: Option<&'a gx_log::proof::ConsistencyProof>,
}

impl<'a> Anchorage<'a> {
    /// An anchor with nothing bridging it — what [`verify_offline`] has always passed.
    #[must_use]
    pub fn at(checkpoint: &'a Checkpoint) -> Self {
        Self {
            checkpoint,
            bridge: None,
        }
    }
}

/// The same verification, told **how the log grew** since the receipt was issued (**H-09**).
///
/// The one entry point that can answer `Verified` for a receipt older than the anchor. The other
/// two are this one with `bridge: None`, which is why they still report [`InclusionCheck::Unbridged`]
/// in that case: an answer is not improved by a caller who has no evidence.
///
/// # Errors
/// Everything [`verify_offline`] refuses.
pub fn verify_offline_against(
    receipt: &Receipt,
    key: &VerifyingKeyRef<'_>,
    anchorage: Option<&Anchorage<'_>>,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Result<Checks> {
    checks_of(receipt, key, anchorage, revocation)
}

fn checks_of(
    receipt: &Receipt,
    key: &VerifyingKeyRef<'_>,
    anchor: Option<&Anchorage<'_>>,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Result<Checks> {
    receipt.envelope.verify(key)?;
    let payload = receipt.payload()?;
    payload.check_schema()?;

    if payload.key_id != key.key_id {
        return Err(Error::Schema {
            detail: format!(
                "the receipt names key {:?} and was verified against {:?} (42 §3.10: \
                 matches `DsseSignature.keyid`)",
                payload.key_id, key.key_id
            ),
        });
    }

    let canonical_cid = payload.canonical_cid == payload.transformation.0;
    let inclusion = match (payload.receipt_kind, &payload.inclusion_proof, anchor) {
        (ReceiptKind::VerdictReceipt, _, _) => InclusionCheck::NotApplicable,
        (ReceiptKind::CommitReceipt, Some(proof), Some(anchorage)) => {
            verify_inclusion_from(&payload, &receipt.envelope.payload, proof, anchorage)?
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
/// The arithmetic is gx-log's (`verify_inclusion_of`, `root_of_inclusion`, `verify_consistency`)
/// and is not repeated here. That is why this crate names gx-log at all -- see the dependency note
/// in the crate root.
///
/// # 🔴 Three sizes, and what each one means (**H-09**, RFC 6962 §2.1.2)
///
/// Call `m` the `tree_size` the receipt's proof names and `n` the anchor's.
///
/// * **`m == n`** -- the original question, unchanged: does the path reach this root? Yes is
///   [`InclusionCheck::Verified`], no is [`InclusionCheck::Refuted`], and no still means what it
///   always meant, because the two statements are about the same tree.
/// * **`m < n`** -- the log grew after the receipt was issued, which is the ordinary case for every
///   receipt but the newest. The roots differ *by construction*, so asking the `m == n` question
///   here answers "refuted" about a receipt nothing is wrong with. With a consistency proof the
///   chain closes: reconstruct the root at `m` from the receipt itself, carry it to `n`, compare
///   with the anchor. Without one there is no link and the answer is
///   [`InclusionCheck::Unbridged`].
/// * **`m > n`** -- the anchor is *older* than the commit. A tree of `n` leaves cannot contain a
///   leaf appended at `m`, and its silence is not a statement about the receipt.
///   [`InclusionCheck::Unbridged`] again: the honest answer is "this head is too old to say", and
///   the operator's move is to fetch a later one.
///
/// The bridge is refused before it is used unless it is about exactly `(m, n)`. A proof between two
/// other sizes is not partial evidence; used anyway it would let a holder of any two consistent
/// sizes launder a receipt from a third tree.
fn verify_inclusion_from(
    payload: &ReceiptPayload,
    signed_bytes: &[u8],
    proof: &InclusionProof,
    anchorage: &Anchorage<'_>,
) -> Result<InclusionCheck> {
    let anchor = anchorage.checkpoint;
    let leaf = LedgerLeaf {
        index: proof.leaf_index,
        // 🔴 **`req/38` §324 ruling 3** — from the bytes, not from the struct they decoded into.
        //
        // This is the line the ruling is about. A verifier is by definition holding a document
        // somebody else wrote, possibly under a schema this build has never seen; re-encoding the
        // decoded value asks what *this* build would have written and answers `Refuted` — the word
        // for tampering — about an untouched receipt whenever a member has been added since.
        receipt_digest: ledger_digest_of_signed_payload(signed_bytes)?,
        transformation: payload.transformation,
    };
    if anchor.tree_size == proof.tree_size {
        return Ok(
            if gx_log::proof::verify_inclusion_of(proof, &anchor.root_hash, &leaf)? {
                InclusionCheck::Verified
            } else {
                InclusionCheck::Refuted
            },
        );
    }
    if anchor.tree_size < proof.tree_size {
        return Ok(InclusionCheck::Unbridged);
    }
    // The root the receipt *itself* computes to. `None` is a proof that does not fit the tree it
    // declares -- a statement about no tree at all, and the one case here that is a refutation
    // rather than a gap.
    let Some(root_at_proof_size) = gx_log::proof::root_of_inclusion(proof, &leaf)? else {
        return Ok(InclusionCheck::Refuted);
    };
    let Some(bridge) = anchorage.bridge else {
        return Ok(InclusionCheck::Unbridged);
    };
    if bridge.old_size != proof.tree_size || bridge.new_size != anchor.tree_size {
        return Ok(InclusionCheck::Unbridged);
    }
    Ok(
        if gx_log::proof::verify_consistency(bridge, &root_at_proof_size, &anchor.root_hash)? {
            InclusionCheck::Verified
        } else {
            // Both sizes are the ones asked about and the walk did not land: the receipt's tree is
            // not a prefix of the anchor's, or the bridge is forged. Either way this is evidence
            // against, which is what the word is for.
            InclusionCheck::Refuted
        },
    )
}
