//! Σ's components that **no transition in this hand can write** — reconstructed from journals
//! written by hand, and honest about what that costs.
//!
//! `tests/ac_039.rs` compares the engine's Σ with the journal's for every script hand 2's four entry
//! points can produce. That leaves four records untested, because the transitions that write them
//! are hands 4 and 6: `CommittingStarted` (T-9), `InverseEscrowed` (T-10b), `ApplyStarted`
//! (**E-M5-1**), `Committed` (T-11) and `Superseded` (T-12).
//!
//! **This is a weaker instrument and it is used deliberately.** A journal a test wrote is not a
//! journal an execution wrote, so what is measured here is the *reconstruction* and not the
//! agreement between two independent paths. The alternative was to leave Σ's escrow and ledger
//! components with no probe at all until hand 4, which would mean shipping a reconstruction whose
//! two most consequential components had never been run — and 「skip と pass を同じ顔にしない」
//! (req/29 §4) cuts the other way here: an untested component that *looks* tested because AC-039 is
//! green is the failure to avoid. The report says which half is which.

mod support;

use gx_core::{AbortReason, SubstrateKind, Timestamp, VerdictKind};
use gx_engine::{replay, BlobStore, EngineJournal, EngineJournalRecord, InverseStatus, Lifecycle};
use gx_substrate::PlannedDelta;
use support::{cid, iid, scratch, tid};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// A journal in a fresh directory, with these records appended in order.
fn journal_of(
    name: &str,
    records: Vec<EngineJournalRecord>,
) -> (EngineJournal, std::path::PathBuf) {
    let dir = scratch(name);
    let path = dir.join("journal.bin");
    let mut journal = EngineJournal::open(&path).expect("a fresh journal opens");
    for record in records {
        journal.append(record).expect("append");
    }
    (journal, path)
}

/// Σ from the bytes on disk.
fn sigma_of(path: &std::path::Path) -> gx_engine::Sigma {
    let bytes = std::fs::read(path).expect("the journal is on disk");
    replay(&bytes).sigma()
}

/// A commit that ran to the end: T-2 through T-11, with **E-M5-1**'s record in the critical section.
fn a_committed_run() -> Vec<EngineJournalRecord> {
    let t = tid(1);
    vec![
        EngineJournalRecord::DraftCreated {
            intent_id: iid(1),
            rng_seed: 42,
            at: AT,
        },
        EngineJournalRecord::Planned {
            transformation: t,
            intent_id: iid(1),
            locator: "/tmp/one".to_string(),
            delta_cid: cid(10),
            fp0: support::fp(1),
            parents: Vec::new(),
            at: AT,
        },
        EngineJournalRecord::VerifyStarted {
            transformation: t,
            at: AT,
        },
        EngineJournalRecord::Verdict {
            transformation: t,
            kind: VerdictKind::Admit,
            verdict_digest: Some(cid(11)),
            fail_posture_engaged: false,
            at: AT,
        },
        EngineJournalRecord::Canonicalized {
            transformation: t,
            canonical_cid: cid(12),
            enforced: None,
            at: AT,
        },
        EngineJournalRecord::CommittingStarted {
            transformation: t,
            at: AT,
        },
        EngineJournalRecord::InverseEscrowed {
            transformation: t,
            inverse_cid: Some(cid(13)),
            at: AT,
        },
        EngineJournalRecord::ApplyStarted {
            transformation: t,
            delta_cid: cid(10),
            at: AT,
        },
        EngineJournalRecord::Committed {
            transformation: t,
            ledger_seq: 7,
            at: AT,
        },
    ]
}

/// Every component of Σ, from a journal that reaches T-11.
#[test]
fn a_committed_run_reconstructs_into_all_four_components() {
    let (_journal, path) = journal_of("sigma_committed", a_committed_run());
    let sigma = sigma_of(&path);
    let t = tid(1);
    let row = sigma.state_of(&t).expect("the state table holds it");

    println!(
        "DRAFTS={} ROWS={} ESCROW={} LEDGER={:?} STATE={:?} APPLY_STARTED={:?}",
        sigma.drafts().len(),
        sigma.transformations().len(),
        sigma.escrow().len(),
        sigma.ledger(),
        row.state,
        row.apply_started
    );
    assert_eq!(sigma.drafts().len(), 1);
    assert_eq!(row.state, Some(Lifecycle::Committed));
    assert_eq!(row.intent_id, Some(iid(1)));
    assert_eq!(row.delta_cid, Some(cid(10)));
    assert_eq!(row.canonical_cid, Some(cid(12)));
    assert_eq!(row.verdict, Some(VerdictKind::Admit));
    assert!(row.enforced);

    // 🔴 E-M5-1: the record that separates 「the world did not move」 from 「the world moved and
    // nothing recorded it」 has to survive the round trip, or hand 5's recovery reads a journal that
    // has forgotten the one fact it was added for.
    assert_eq!(row.apply_started, Some(cid(10)));

    assert_eq!(sigma.escrow().len(), 1);
    assert_eq!(sigma.escrow()[0].inverse_cid, Some(cid(13)));
    assert_eq!(sigma.escrow()[0].status, InverseStatus::Available);
    assert_eq!(sigma.ledger().len(), 1);
    assert_eq!(sigma.ledger()[0].ledger_seq, 7);
}

/// 🔴 T-12, read back: the original is `Superseded` **and** its escrowed inverse is `Consumed`.
///
/// **M5-09 採(a)** and **M5-16 採(a)** put both writes at T-12 「1 箇所」. Firing that transition is
/// hand 6's; this is the reconstruction, and it fails if either half is dropped — a `Superseded`
/// state with an inverse still `Available` would offer an undo that has already been used.
#[test]
fn a_supersede_consumes_the_escrowed_inverse() {
    let undoer = tid(2);
    let mut records = a_committed_run();
    records.push(EngineJournalRecord::Superseded {
        transformation: tid(1),
        by: undoer,
        at: AT,
    });
    let (_journal, path) = journal_of("sigma_superseded", records);
    let sigma = sigma_of(&path);
    let row = sigma.state_of(&tid(1)).expect("the row");

    println!(
        "SUPERSEDED_STATE={:?} BY={:?} ESCROW_STATUS={:?}",
        row.state,
        row.superseded_by,
        sigma.escrow()[0].status
    );
    assert_eq!(row.state, Some(Lifecycle::Superseded));
    assert_eq!(row.superseded_by, Some(undoer));
    assert_eq!(
        sigma.escrow()[0].status,
        InverseStatus::Consumed { by: undoer },
        "M5-16 採(a): `Consumed{{by}}` is written at T-12, in the same place as the supersedes edge"
    );
}

/// A **trimmed** journal names no state it cannot support (42 §5: 「commit確定後トリム可」).
///
/// A surviving prefix that begins in the middle is a legitimate journal, not a damaged one. The row
/// it produces says what the records say and no more: an `Aborted` with no `Planned` before it has
/// a state and **no** `intent_id`, and a transformation named only by `ApplyStarted` has no state at
/// all. Inventing either would be the engine telling a story its records do not support.
#[test]
fn a_trimmed_journal_names_no_state_it_cannot_support() {
    let (_journal, path) = journal_of(
        "sigma_trimmed",
        vec![
            EngineJournalRecord::Aborted {
                transformation: tid(3),
                reason: AbortReason::Expired,
                rollback: None,
                at: AT,
            },
            EngineJournalRecord::ApplyStarted {
                transformation: tid(4),
                delta_cid: cid(20),
                at: AT,
            },
        ],
    );
    let sigma = sigma_of(&path);
    let aborted = sigma.state_of(&tid(3)).expect("the aborted row");
    let mid_commit = sigma
        .state_of(&tid(4))
        .expect("the row named by ApplyStarted alone");

    println!(
        "TRIMMED_ABORTED state={:?} intent={:?} | TRIMMED_APPLY state={:?} apply_started={:?}",
        aborted.state, aborted.intent_id, mid_commit.state, mid_commit.apply_started
    );
    assert_eq!(
        aborted.state,
        Some(Lifecycle::Aborted(AbortReason::Expired))
    );
    assert_eq!(
        aborted.intent_id, None,
        "the `Planned` record is gone, so the intent is unknown and says so"
    );
    assert_eq!(
        mid_commit.state, None,
        "43 §1 has no value meaning 「unknown」 to borrow, and borrowing one would be a guess"
    );
    assert_eq!(mid_commit.apply_started, Some(cid(20)));
}

/// The ledger component is in **ledger order**, whatever order the journal recorded it in.
///
/// A ledger has an order a set does not, and sorting Σ's ledger component by `TransformationId`
/// would throw that away before AC-039 compared it. The records here are appended with their
/// sequence numbers descending, so a reconstruction that kept journal order or id order fails.
#[test]
fn the_ledger_component_is_in_ledger_order() {
    let (_journal, path) = journal_of(
        "sigma_ledger_order",
        vec![
            EngineJournalRecord::Committed {
                transformation: tid(9),
                ledger_seq: 3,
                at: AT,
            },
            EngineJournalRecord::Committed {
                transformation: tid(1),
                ledger_seq: 1,
                at: AT,
            },
            EngineJournalRecord::Committed {
                transformation: tid(5),
                ledger_seq: 2,
                at: AT,
            },
        ],
    );
    let sigma = sigma_of(&path);
    let seqs: Vec<u64> = sigma.ledger().iter().map(|c| c.ledger_seq).collect();
    println!("LEDGER_ORDER={seqs:?}");
    assert_eq!(seqs, vec![1, 2, 3]);
}

/// A torn tail is not in Σ.
///
/// The last record's bytes are cut in half, which is what a crash mid-append leaves behind. The
/// reconstruction stops at the last whole record, so Σ is a state the execution passed through
/// rather than one assembled across a hole (see `replay`'s module documentation for why the refusal
/// does not skip and carry on).
#[test]
fn a_torn_tail_is_not_in_sigma() {
    let (_journal, path) = journal_of("sigma_torn", a_committed_run());
    let whole = std::fs::read(&path).expect("on disk");
    let complete = replay(&whole).sigma();
    assert_eq!(
        complete.state_of(&tid(1)).expect("row").state,
        Some(Lifecycle::Committed)
    );

    // Cut the file so the final `Committed` record is half-written.
    let torn = &whole[..whole.len() - 12];
    let out = replay(torn);
    let sigma = out.sigma();
    println!(
        "TORN_TAIL_BYTES={} RECORDS={} STATE={:?} LEDGER={}",
        out.recovery().torn_tail_bytes,
        out.recovery().records,
        sigma.state_of(&tid(1)).expect("row").state,
        sigma.ledger().len()
    );
    assert!(out.recovery().torn_tail_bytes > 0);
    assert_eq!(
        sigma.state_of(&tid(1)).expect("row").state,
        Some(Lifecycle::Committing),
        "the state is the last one a whole record fixed"
    );
    assert_eq!(
        sigma.ledger().len(),
        0,
        "and nothing claims a ledger entry that the journal did not finish recording"
    );
}

/// 🔴 **E-M5-6 across a restart**: the index comes from the journal, the body from the blob store.
///
/// This is the whole shape M5-05 採(a) buys. Neither half is an escrow on its own — the journal
/// knows *which* inverse belongs to *which* transformation and nothing about the delta; the store
/// knows the delta and nothing about who it undoes — and `BlobStore::escrowed` is the one place they
/// are put back together, through `EscrowedInverse::restore`, which refuses a row whose status and
/// body disagree.
#[test]
fn an_escrowed_inverse_is_rebuilt_from_the_index_and_the_store() {
    let dir = scratch("sigma_escrow_restart");
    let blobs = BlobStore::open(dir.join("blobs")).expect("a blob store opens");
    let inverse = PlannedDelta::new(SubstrateKind::Fs, b"put the old bytes back".to_vec())
        .expect("digestible");
    let (inverse_cid, _) = blobs.put(&inverse).expect("escrow the body");

    let path = dir.join("journal.bin");
    let mut journal = EngineJournal::open(&path).expect("a fresh journal opens");
    journal
        .append(EngineJournalRecord::InverseEscrowed {
            transformation: tid(1),
            inverse_cid: Some(inverse_cid),
            at: AT,
        })
        .expect("append");
    drop(journal);

    // The restart: nothing is carried over in memory.
    let sigma = sigma_of(&path);
    let row = sigma.escrow()[0];
    let escrowed = blobs
        .escrowed(&row)
        .expect("the two halves rebuild one escrow");
    println!(
        "REBUILT_ESCROW transformation={:?} status={} has_body={}",
        escrowed.transformation(),
        escrowed.status().kind(),
        escrowed.inverse_delta().is_some()
    );
    assert_eq!(escrowed.transformation(), tid(1));
    assert_eq!(escrowed.inverse_delta(), Some(&inverse));
    assert_eq!(escrowed.status(), &InverseStatus::Available);
    assert_eq!(
        row.retained_until, None,
        "42 §3.13's `InverseEscrowed` has no seat for a deadline; DR-9's OSS default is 無期限"
    );

    // The other direction, from a store that lost the body: the index alone is not an escrow, and
    // the refusal names what is missing rather than answering with an inverse that is not there.
    let empty = BlobStore::open(dir.join("empty")).expect("a second store opens");
    let refused = empty
        .escrowed(&row)
        .expect_err("an index row whose body is gone is not an escrow");
    println!("ESCROW_WITHOUT_BODY={}", refused.kind());
    assert_eq!(refused.kind(), "NotFound");
}

/// The whole vocabulary reaches Σ: every one of the twelve records changes something.
///
/// A reconstruction with a silently missing arm is a reconstruction that loses a fact per record.
/// The catalogue in `tests/support` is the same twelve `tests/journal_identity.rs` uses, so a
/// thirteenth variant added tomorrow arrives here as well.
#[test]
fn every_record_in_the_vocabulary_reaches_sigma() {
    let mut unchanged: Vec<&'static str> = Vec::new();
    for record in support::every_variant() {
        let kind = record.kind();
        let before = replay(&[]).sigma().canonical_bytes().expect("canonical");
        let (_journal, path) = journal_of(&format!("sigma_vocab_{kind}"), vec![record]);
        let after = sigma_of(&path).canonical_bytes().expect("canonical");
        if before == after {
            unchanged.push(kind);
        }
    }
    println!("RECORDS_THAT_CHANGE_NOTHING={unchanged:?}");
    assert!(
        unchanged.is_empty(),
        "these records reconstruct into nothing, so a journal holding them replays to a Σ that \
         has forgotten them: {unchanged:?}"
    );
}
