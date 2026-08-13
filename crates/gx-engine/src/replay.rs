//! Reading a journal back: the records it holds, and how much of the tail did not survive.
//!
//! Spec: 43 §7-1 (「起動時 replay」), 42 §3.13 for what a record is, 41 §6 for the canonical form.
//!
//! # A pure function over bytes
//!
//! [`replay`] takes a `&[u8]` and returns values. It opens nothing, calls no adapter, and cannot
//! reach a substrate — which is **E-M5-2**'s ruling made structural rather than promised:
//!
//! > replay は **Σ のみを再構成する read-only 操作**…adapter は呼ばない
//!
//! `store.rs` reads the file and hands the bytes over. Splitting it this way is also what makes a
//! torn tail testable without a crash: the case is 「these bytes」, not 「this machine lost power」.
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
//! What counts as 「did not survive」 is deliberately wide: a header shorter than four bytes, a
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

/// What a replay produced.
///
/// The counts are in a [`Recovery`], gx-log's struct, because 「how many records came back and how
/// many bytes after them did not」 is one question about append-only files and not two (see the
/// crate documentation).
#[derive(Clone, Debug)]
pub struct Replay {
    records: Vec<EngineJournalRecord>,
    recovery: Recovery,
    good_bytes: u64,
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

/// Read a journal's bytes back into the records they hold.
///
/// Infallible by construction: a torn tail is the ordinary shape of a crash, not a failure of this
/// function, so it is *reported* in the [`Recovery`] rather than raised. The only thing that could
/// be an error here — an unreadable file — belongs to the caller that opened it.
#[must_use]
pub fn replay(bytes: &[u8]) -> Replay {
    let total = bytes.len() as u64;
    let mut records = Vec::new();
    let mut good: u64 = 0;
    let mut at = 0usize;

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
        if end > bytes.len() {
            break;
        }
        let Ok(record) = cbor::decode::<EngineJournalRecord>(&bytes[start..end]) else {
            break;
        };
        records.push(record);
        good += (LENGTH_BYTES + length as usize) as u64;
        at = end;
    }

    Replay {
        recovery: Recovery {
            records: records.len() as u64,
            torn_tail_bytes: total - good,
        },
        records,
        good_bytes: good,
    }
}

// ---------------------------------------------------------------------------
// E-M5-2 -- Σ, and the read-only operation that rebuilds it
// ---------------------------------------------------------------------------

/// One draft: an `IntentId` and the seed 41 §6 injected with it.
///
/// A draft has no `TransformationId` (43 T-1, **E-M5-3**) and therefore no row in a table keyed on
/// one (**M5-17 採(b)**), so it is its own component of Σ. The seed is here because 42 §3.13 says
/// what it is for -- 「`rng_seed`（`DraftCreated`）はengineが乱数・時刻を境界で注入する際のシードを
/// 記録し、replay時に同一シードで再実行することで決定性を担保する」 -- and a Σ that dropped it would
/// make AC-039's control experiment (「異なるseedでは一致が保証されない」) unwritable: the seed would
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
/// 42 §5: 「`EngineJournalRecord` … commit確定後トリム可」. A trimmed journal legitimately begins in
/// the middle, so a surviving prefix can hold an `Aborted` for a transformation whose `Planned`
/// record is gone. Reconstructing an `intent_id` for it would be the engine telling a story its
/// records do not support, and `None` is the true answer. `state` is an `Option` for the same
/// reason and one further: a journal can name a transformation with a record that fixes no state at
/// all (`ApplyStarted`, `InverseEscrowed`), and 43 §1 has no value meaning 「unknown」 to borrow.
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
    /// The one fact that separates 「the world did not move」 from 「the world moved and nothing
    /// recorded it」 (req/78 §3.2 Λ4). Hand 5's recovery is the consumer; Σ carries it because a
    /// reconstruction that dropped it would leave the recovery blind to exactly the record that was
    /// added for it.
    pub apply_started: Option<Cid>,
    /// 🔴 **AC-038**: what became of 43 T-10c's best-effort rollback, where one was in question.
    ///
    /// `None` on every row that has not aborted under T-10c. Σ carries it because the journal
    /// records it: a reconstruction that dropped the field would make 「the inverse was applied and
    /// the adapter refused it」 invisible to a replay, which is the one fact an operator reading an
    /// `ApplyFailed` needs. See [`crate::store::Rollback`].
    pub rollback: Option<Rollback>,
    /// 🔴 **M5-25 採(a)** -- the provenance the engine derived for this transformation (42 §3.9).
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
/// a deadline, while DR-9 makes 「無期限」 the OSS default and req/78 N-06 keeps the enforcement of
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
/// AC-039's 「結果状態」 as 「Σ(状態表+ledger root+escrow index)」, and in v0.1 the engine reaches the
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
/// > **M5-02 採(a)**=**E-M5-2**: replay は **Σ のみを再構成する read-only 操作**・AC-039 の「結果状態」
/// > =Σ(状態表+ledger root+escrow index)と読む。adapter は呼ばない
///
/// Four components, in the order the ruling names them plus the draft phase that 43 T-1 puts
/// outside the table:
///
/// | component | 43 / 42 | where it comes from |
/// |---|---|---|
/// | `drafts` | 43 T-1, **M5-17 採(b)** | `DraftCreated` |
/// | `transformations` | 42 §1.3-3's 「外部テーブル」 | every record naming a `TransformationId` |
/// | `escrow` | 42 §3.12 | `InverseEscrowed`, and `Superseded` for the status |
/// | `ledger` | 42 §3.11 via T-11 | `Committed` |
///
/// # Bit-equality is byte equality of the canonical form
///
/// AC-039 asks for 「bit-equal」, and this type answers it with [`Sigma::canonical_bytes`] rather
/// than with a derived comparison. Two reasons, and the first is hand 1's: Σ holds a
/// [`FingerprintRecord`], and **E-M4-15** took `==` off `Fingerprint` because 42 §3.5's comparison
/// has three answers. The second is that 「bit-equal」 is a claim about *bytes*, and comparing the
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
/// > **E-M5-2**: replay は Σ のみを再構成する read-only 操作…adapter は呼ばない
///
/// It takes records and returns a value. There is no substrate to reach, no clock to read and no
/// randomness to draw: 「同一seed/clockでのリプレイ結果がbit-equal」 (32 FR-039) holds here for a
/// reason stronger than care, which is that the seed and the clock **are in the records** -- the
/// seed in `DraftCreated`, the clock in every `at` and, through `CompositionMetadata.created_at`, in
/// the `TransformationId` itself. A replay does not re-inject them; it reads what was injected.
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
///   status `Consumed { by }`. **M5-09 採(a)** and **M5-16 採(a)** put both at T-12 「1 箇所」; firing
///   that transition is hand 6's, and reading a record that says it fired is this function's.
#[must_use]
pub fn reconstruct(records: &[EngineJournalRecord]) -> Sigma {
    let mut drafts: BTreeMap<IntentId, u64> = BTreeMap::new();
    let mut rows: BTreeMap<TransformationId, StateRow> = BTreeMap::new();
    let mut escrow: BTreeMap<TransformationId, EscrowRow> = BTreeMap::new();
    let mut ledger: BTreeMap<TransformationId, u64> = BTreeMap::new();

    for record in records {
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
                    // T-4e: 43 §4 degrades this transformation to 「record-onlyモード相当」 and no
                    // gate ran, so there is no verdict to hold.
                    row.verdict = None;
                    row.enforced = false;
                    row.state = Some(Lifecycle::Admitted);
                } else {
                    row.verdict = Some(*kind);
                    row.state = Some(state_of_verdict(*kind));
                }
            }
            EngineJournalRecord::HumanDecision { kind, .. } => {
                let row = row.expect("`HumanDecision` names a transformation");
                row.verdict = Some(*kind);
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
                        // `Unavailable` -- 「`invert()`がNoneを返した場合（構成不能）」. The two are
                        // one record apart and must not be one status apart, or a replay of a
                        // commit that could never be undone would report an undo as available.
                        status: match inverse_cid {
                            Some(_) => InverseStatus::Available,
                            None => InverseStatus::Unavailable,
                        },
                    },
                );
            }
            EngineJournalRecord::ApplyStarted { delta_cid, .. } => {
                let row = row.expect("`ApplyStarted` names a transformation");
                row.apply_started = Some(*delta_cid);
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

    Sigma::new(
        drafts
            .into_iter()
            .map(|(intent_id, rng_seed)| DraftRow {
                intent_id,
                rng_seed,
            })
            .collect(),
        rows.into_values().collect(),
        escrow.into_values().collect(),
        ledger
            .into_iter()
            .map(|(transformation, ledger_seq)| CommittedRow {
                transformation,
                ledger_seq,
            })
            .collect(),
    )
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
