// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **T-12 / E-M5-9 / discipline 48** -- the supersede edge from the inside, and the window it leaves open.
//! (sem: SEM-gx-engine-912)
//!
//! Spec: 43 T-12 for the transition, 43 §5 for what an undo is, 42 §3.12 for the escrow's four
//! statuses, 42 §3.13 for the `InverseEscrowed` and `Superseded` records, ASM-43-2 for the index.
//!
//! `tests/ac_040.rs` measures what a caller sees. This measures what the **journal** holds, which
//! is where two of this hand's obligations live:
//!
//! * 🔴 **E-M5-9** — an `InverseEscrowed` whose CID is `None`. Reachable for the first time in this
//!   hand, because reaching it needs a person to approve an escalation (see below).
//! * 🔴 **discipline 48** -- "on a path where a terminal record re-supplies a mid-record's information,
//!   always place one probe that stops at the intermediate state" (§40 M5H3-6) (sem: SEM-gx-engine-913). T-12's `Superseded` re-supplies both halves of what a replay knows
//!   about the escrow, so [`the_state_between_the_commit_and_the_edge_is_measured`] stops in
//!   between.

mod support;

use std::sync::Arc;

use gx_core::{Timestamp, VerdictKind};
use gx_engine::{
    reconstruct, replay, Engine, EngineJournalRecord, HumanRuling, InjectedEvidence, InverseStatus,
    Lifecycle,
};
use support::{
    gate, intent, record_boundaries, ruler, scratch, signing_key, CommitAdapter, PERMIT_ALL,
};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const UNDO_AT: Timestamp = Timestamp(1_754_000_120_000_000_000);

// ---------------------------------------------------------------------------
// E-M5-9
// ---------------------------------------------------------------------------

/// 🔴 **E-M5-9**: a commit whose inverse could not be built records **that**, rather than nothing.
///
/// # The path, and why it did not exist before this hand
///
/// **E-M3-4** makes "`adapter.invert` answered `None`" (sem: SEM-gx-engine-914) the one condition that produces an
/// `Escalate` in v0.1. So a transformation with no constructible inverse stopped at `Escalated`,
/// and hand 4 could write, truthfully, that its `None` arm at T-10b was "Unreachable in v0.1" (sem: SEM-gx-engine-914).
/// T-5 is what makes it reachable: a person approves the escalation, the transformation walks on to
/// `Committing`, and 43 T-10b's guard, "an inverse can be constructed (`Some`)", does **not** open
/// (sem: SEM-gx-engine-914).
///
/// §40 reserved the erratum for exactly this turn:
///
/// > **M5H3-2, direction adopted (a), implementation window = hand 6** = **E-M5-9 (reserved)**:
/// > making `InverseEscrowed.inverse_cid` an `Option` ..., **implemented in the same turn hand 6's
/// > escalation approval makes the path real** (sem: SEM-gx-engine-915)
///
/// Four things are read: the record exists and carries `None`, the escrow index says `Unavailable`
/// (42 §3.12), the receipt's `inverse_delta` is absent (42 §3.10), and a **replay agrees** — which
/// is the half that says the erratum reached the reconstruction and not only the writer.
#[test]
fn e_m5_9_a_commit_with_no_constructible_inverse_records_the_absence() {
    let dir = scratch("supersede_e_m5_9");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/one-way-only.txt", "after");
    engine.submit(&i, 100, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &signing_key(), None).expect("T-4c"),
        Lifecycle::Escalated,
        "E-M3-4: no inverse, so the gate escalates"
    );
    engine
        .escalation(
            &id,
            &HumanRuling {
                decision: VerdictKind::Admit,
                reason: "accepted without an undo guarantee".to_string(),
                actor: ruler(5),
            },
            UNDO_AT,
            &signing_key(),
        )
        .expect("T-5");
    engine.canonicalize(&id, UNDO_AT, None).expect("T-8");
    let committed = engine.commit(&id, UNDO_AT, &signing_key()).expect("T-11");

    let escrowed: Vec<_> = engine
        .journal()
        .records()
        .iter()
        .filter_map(|r| match r {
            EngineJournalRecord::InverseEscrowed { inverse_cid, .. } => Some(*inverse_cid),
            _ => None,
        })
        .collect();
    let payload = engine
        .receipt(&id)
        .expect("T-11 issued one")
        .payload()
        .expect("decodes");
    let sigma = reconstruct(engine.journal().records());
    let row = sigma
        .escrow()
        .iter()
        .find(|e| e.transformation == id)
        .expect("the reconstruction has the row");

    println!(
        "EM59 committed={committed:?} escrowed_records={escrowed:?} status={:?} \
         receipt_inverse={:?} replayed_cid={:?} replayed_status={:?} escrowed_inverse={:?} \
         applies={} world={:?}",
        engine.inverse_status(&id),
        payload.inverse_delta,
        row.inverse_cid,
        row.status,
        engine.escrowed_inverse(&id),
        counts.totals()[4],
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );

    assert_eq!(committed, Lifecycle::Committed, "the change was applied");
    assert_eq!(
        escrowed,
        vec![None],
        "🔴 E-M5-9: one record, carrying the absence. Hand 4 wrote none at all here, which left \
         \"we asked and there is none\" and \"we never asked\" with one face (§32 M4H4-2) (sem: SEM-gx-engine-916)"
    );
    assert_eq!(
        engine.inverse_status(&id),
        Some(InverseStatus::Unavailable),
        "42 §3.12: \"when `invert()` returned None (unconstructible)\" (sem: SEM-gx-engine-917)"
    );
    assert_eq!(engine.escrowed_inverse(&id), None);
    assert_eq!(payload.inverse_delta, None, "42 §3.10's seat stays empty");
    assert_eq!(row.inverse_cid, None, "and the replay reads it back");
    assert_eq!(row.status, InverseStatus::Unavailable);

    // And there is nothing to undo, said as a refusal rather than as a panic.
    let refused = engine
        .undo(&id, &engine.attested_postcondition(&id), 101, UNDO_AT)
        .expect_err("42 §3.12's `Unavailable` is not an inverse");
    println!("EM59_UNDO refused={:?}", refused.kind());
    assert_eq!(refused.kind(), "NotFound");
    assert_eq!(counts.totals()[4], 1, "and nothing was applied twice");
}

// ---------------------------------------------------------------------------
// The journal's view of T-12
// ---------------------------------------------------------------------------

/// Run an undo to completion and answer with the engine and the two ids.
fn undone(
    name: &str,
) -> (
    Engine<InjectedEvidence>,
    gx_core::TransformationId,
    gx_core::TransformationId,
) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/superseded.txt", "after");
    engine.submit(&i, 110, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    engine.commit(&t_o, AT, &signing_key()).expect("T-11");

    let (_, t_u) = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 111, UNDO_AT)
        .expect("the candidate");
    engine
        .verify(&t_u, UNDO_AT, &signing_key(), None)
        .expect("T-4a");
    engine.canonicalize(&t_u, UNDO_AT, None).expect("T-8");
    engine.commit(&t_u, UNDO_AT, &signing_key()).expect("T-11");
    (engine, t_o, t_u)
}

/// 🔴 T-12 writes one record, in the right place, about the right transformation.
///
/// 43 T-12's journal cell is `Superseded{T_o.id, by: T_u.id}` and its side effect is "appends
/// `superseded_by = T_u.id` to `T_o`'s metadata". The record is **about** `T_o` and comes **after**
/// `T_u`'s `Committed`, which is the order 43 T-12's trigger requires ("a different transformation
/// `T_u` reaches `Committed`") and the reason discipline 48's window exists. (sem: SEM-gx-engine-918)
#[test]
fn t_12_writes_one_record_about_the_original_after_the_inverse_commits() {
    let (engine, t_o, t_u) = undone("supersede_record");
    let kinds: Vec<&str> = engine
        .journal()
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    let superseded: Vec<_> = engine
        .journal()
        .records()
        .iter()
        .enumerate()
        .filter_map(|(n, r)| match r {
            EngineJournalRecord::Superseded {
                transformation, by, ..
            } => Some((n, *transformation, *by)),
            _ => None,
        })
        .collect();
    let last_committed = kinds
        .iter()
        .rposition(|k| *k == "Committed")
        .expect("two commits happened");

    println!(
        "T12_RECORDS kinds={kinds:?} superseded={superseded:?} last_committed={last_committed}"
    );
    assert_eq!(superseded.len(), 1, "one edge, one record");
    let (at, about, by) = superseded[0];
    assert_eq!(about, t_o, "the record is about the original");
    assert_eq!(by, t_u, "and names the transformation that undid it");
    assert!(
        at > last_committed,
        "43 T-12 fires after `T_u` reaches `Committed`, not before"
    );

    // 42 §3.13's `Superseded` is the **only** record written about `T_o` after its own `Committed`.
    let after: Vec<&str> = engine
        .journal()
        .records()
        .iter()
        .skip_while(|r| r.transformation() != Some(t_o) || r.kind() != "Committed")
        .skip(1)
        .filter(|r| r.transformation() == Some(t_o))
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    println!("T12_AFTER_COMMIT_OF_T_O={after:?}");
    assert_eq!(
        after,
        vec!["Superseded"],
        "43 §5-4: \"none of `T_o`'s canonical record, receipt, or ledger entry is ever rewritten\" -- the \
         one thing appended is the edge (sem: SEM-gx-engine-919)"
    );
}

/// 🔴 **discipline 48**: the state **between** `T_u`'s commit and the supersede edge, measured. (sem: SEM-gx-engine-920)
///
/// §40 establishes: "on a path where a terminal record re-supplies a mid-record's information,
/// always place one probe that stops at the intermediate state" (sem: SEM-gx-engine-920). Here the terminal record is `Superseded`, and it re-supplies two facts a replay would
/// otherwise take from the middle of the run: `T_o`'s state, and the escrow's `InverseStatus`
/// (`crate::replay::reconstruct` sets `Consumed { by }` from this one record). So a probe that only
/// ever looked at the finished journal could not tell a correct engine from one that never wrote
/// `InverseEscrowed` at all.
///
/// The stop is built the way hand 5 built 43 §7-3b's window: the journal is **truncated by one
/// record**, which is what a crash between `Committed` and `Superseded` leaves behind. The
/// reconstruction of that prefix must say `T_o` is still `Committed` and its inverse still
/// `Available` — two facts that the full journal overwrites.
#[test]
fn the_state_between_the_commit_and_the_edge_is_measured() {
    let (engine, t_o, t_u) = undone("supersede_mid");
    let bytes = std::fs::read(engine.journal().path()).expect("the journal is readable");
    let boundaries = record_boundaries(&bytes);
    let full = reconstruct(engine.journal().records());
    let full_row = full.state_of(&t_o).expect("the row");

    // One record short: the prefix that ends with `T_u`'s `Committed`.
    let cut = *boundaries.last().expect("at least one record");
    let prefix = replay(&bytes[..cut]);
    let mid = prefix.sigma();
    let mid_row = mid.state_of(&t_o).expect("the row");
    let mid_escrow = mid
        .escrow()
        .iter()
        .find(|e| e.transformation == t_o)
        .expect("the escrow row");
    let full_escrow = full
        .escrow()
        .iter()
        .find(|e| e.transformation == t_o)
        .expect("the escrow row");

    println!(
        "RULE48 records_full={} records_prefix={} cut_at={cut} \
         mid_state={:?} mid_superseded_by={:?} mid_escrow={:?} \
         full_state={:?} full_superseded_by={:?} full_escrow={:?}",
        engine.journal().len(),
        prefix.records().len(),
        mid_row.state,
        mid_row.superseded_by,
        mid_escrow.status,
        full_row.state,
        full_row.superseded_by,
        full_escrow.status
    );

    assert_eq!(
        prefix.records().len(),
        engine.journal().len() - 1,
        "exactly one record short"
    );
    assert_eq!(
        prefix
            .records()
            .last()
            .map(gx_engine::EngineJournalRecord::kind),
        Some("Committed"),
        "the crash window is after `T_u` committed and before the edge was drawn"
    );
    // The intermediate state: the inverse **was applied and committed**, and the original is still
    // an ordinary `Committed` whose inverse is still on offer.
    assert_eq!(mid_row.state, Some(Lifecycle::Committed));
    assert_eq!(mid_row.superseded_by, None);
    assert_eq!(mid_escrow.status, InverseStatus::Available);
    assert_eq!(
        mid.state_of(&t_u).expect("the row").state,
        Some(Lifecycle::Committed),
        "and `T_u` is committed at the moment of the stop, which is what makes the window real"
    );
    // The terminal record supplies both of the two facts, which is why the stop was needed.
    assert_eq!(full_row.state, Some(Lifecycle::Superseded));
    assert_eq!(full_row.superseded_by, Some(t_u));
    assert_eq!(full_escrow.status, InverseStatus::Consumed { by: t_u });
}

/// 🔴 **M5H6-6**: a recovery does **not** close that window, and the reason is 43 T-12's guard.
///
/// The probe above shows the window; this shows what a restart does with it. `Engine::recover`
/// resumes a `Committing` row and rebuilds Σ's ledger component for a terminal `Committed` one --
/// and draws no supersede edge, because 43 T-12's guard is "`T_u.parents` contains `T_o.id`" (sem: SEM-gx-engine-921) and the
/// journal holds names and digests rather than `Transformation` bodies (ASM-9). Firing on the
/// escrow CID match alone would be half a guard, which is the shortcut §32 M4H4-2 keeps refusing.
///
/// So the gap is measured and raised rather than closed by a shortcut: after a restart over the
/// truncated journal, `T_o` is `Committed` with an `Available` inverse and no edge, for good.
#[test]
fn a_recovery_does_not_draw_the_edge_the_crash_interrupted() {
    let (engine, t_o, t_u) = undone("supersede_recover");
    let path = engine.journal().path().to_path_buf();
    let bytes = std::fs::read(&path).expect("readable");
    let cut = *record_boundaries(&bytes).last().expect("one record");
    drop(engine);
    std::fs::write(&path, &bytes[..cut]).expect("the truncated journal is written");

    let mut restarted = Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("the truncated journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("after");
    restarted.register_adapter(Arc::new(adapter), "commit-adapter-1");
    let recovered = restarted
        .recover(UNDO_AT, &signing_key())
        .expect("43 §7's recovery");
    let sigma = reconstruct(restarted.journal().records());
    let row = sigma.state_of(&t_o).expect("the row");

    println!(
        "M5H6_6 recovered={} paths={:?} state_o={:?} superseded_by={:?} escrow={:?} \
         supersede_records={}",
        recovered.len(),
        recovered.iter().map(|r| r.path).collect::<Vec<_>>(),
        row.state,
        row.superseded_by,
        sigma
            .escrow()
            .iter()
            .find(|e| e.transformation == t_o)
            .map(|e| e.status),
        restarted
            .journal()
            .records()
            .iter()
            .filter(|r| r.kind() == "Superseded")
            .count()
    );
    assert_eq!(
        row.state,
        Some(Lifecycle::Committed),
        "still not superseded"
    );
    assert_eq!(row.superseded_by, None);
    assert_eq!(
        restarted
            .journal()
            .records()
            .iter()
            .filter(|r| r.kind() == "Superseded")
            .count(),
        0,
        "M5H6-6: the recovery cannot check 43 T-12's `parents` guard, so it draws nothing"
    );
    // The world is not damaged by the gap: both commits are in the ledger and the inverse was
    // applied. What is missing is the *metadata* saying which of the two undid the other.
    assert_eq!(restarted.ledger().log().len(), 2);
    assert_ne!(t_o, t_u);
}

/// 🔴 **AC-039 across T-12**: the live Σ and the reconstruction agree after an undo.
///
/// **E-M5-2** defines AC-039's "resulting state" as Σ (sem: SEM-gx-engine-922), and hand 3 measured the agreement over a journal a
/// test assembled; hand 4 measured it over one an execution wrote. Neither could reach T-12, so
/// `superseded_by` was `None` on every row of the live side by construction — hand 4's `sigma()`
/// wrote the literal and said so.
///
/// This is the first run in which the field can be wrong. The live engine sets it in
/// `supersede_after_commit`; the reconstruction reads it from the `Superseded` record; and if the
/// two ever disagreed, Rule 1 ("the table in memory is a cache, not the state") (sem: SEM-gx-engine-923) would be false of the
/// one field T-12 owns. Compared as **bytes**, for [`gx_engine::Sigma`]'s reason.
#[test]
fn sigma_agrees_with_its_reconstruction_across_the_supersede_edge() {
    let (engine, t_o, t_u) = undone("supersede_sigma");
    let live = engine.sigma().canonical_bytes().expect("canonical");
    let replayed = reconstruct(engine.journal().records())
        .canonical_bytes()
        .expect("canonical");
    let live_row = engine
        .sigma()
        .state_of(&t_o)
        .expect("the row")
        .superseded_by;
    println!(
        "SIGMA_T12 live_bytes={} replayed_bytes={} bit_equal={} live_superseded_by={:?} \
         index={:?}",
        live.len(),
        replayed.len(),
        live == replayed,
        live_row,
        engine.superseded_by(&t_o)
    );
    assert_eq!(live_row, Some(t_u), "the live row carries T-12's edge");
    assert_eq!(
        engine.superseded_by(&t_o),
        Some(t_u),
        "and so does the index M5-09, adopted (a), names (sem: SEM-gx-engine-924)"
    );
    assert_eq!(
        live, replayed,
        "AC-039: the state table and the journal disagree about the supersede"
    );
}

/// The index is the only writer of the edge, and it never overwrites (43 T-12's idempotency).
///
/// "a duplicate supersede application by the same `T_u` is ignored (once `superseded_by` is already
/// set, it is not reset)", at the level of the type M5-09, adopted (a), names (sem: SEM-gx-engine-925). Exercised directly because the engine's own path can only
/// fire once: a second `commit` of `T_u` returns early at 43 T-9's idempotency column, so the
/// engine cannot reach the second call this refusal is for.
#[test]
fn the_supersede_index_records_once_and_never_overwrites() {
    let mut index = gx_engine::SupersedeIndex::new();
    let a = gx_core::TransformationId(gx_core::Cid([1u8; 32]));
    let b = gx_core::TransformationId(gx_core::Cid([2u8; 32]));
    let c = gx_core::TransformationId(gx_core::Cid([3u8; 32]));
    let first = index.record(a, b);
    let again = index.record(a, c);
    println!(
        "SUPERSEDE_INDEX first={first} again={again} by={:?} len={}",
        index.superseded_by(&a),
        index.len()
    );
    assert!(first, "the first call draws it");
    assert!(
        !again,
        "\"once already set, it is not reset\" (sem: SEM-gx-engine-926)"
    );
    assert_eq!(index.superseded_by(&a), Some(b), "and not the second claim");
    assert_eq!(index.len(), 1);
    assert!(index.superseded_by(&b).is_none());
    assert!(!index.is_empty());
    assert_eq!(index.iter().count(), 1);
}

/// 🔴 K6 mutant-kill (`undo`'s draft guard, staging pipeline.rs:2912:12 `delete !`, mutants
/// run e, `req/38` §73): an undo **begins with its own journalled draft**.
///
/// 43 §5-1: an undo is a new transformation that begins normally, so both records are written —
/// `DraftCreated` then `Planned` — and 42 §3.13 puts the `rng_seed` in the draft so a replay
/// re-injects the same seed. The guard is T-1's create-if-absent; deleted, a *fresh* undo
/// intent skips the journal (and a repeated one writes twice). Nothing asserted the record
/// existed, so the rewrite survived: the session-visible behaviour is identical and only the
/// journal — the thing a crash reads back — is short one draft.
#[test]
fn an_undo_begins_with_its_own_journalled_draft() {
    let dir = scratch("supersede_undo_draft");
    let i = intent("/tmp/undo-draft.txt", "after");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
    engine.submit(&i, 110, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    engine.commit(&t_o, AT, &signing_key()).expect("T-11");

    let (undo_intent, _t_u) = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 111, UNDO_AT)
        .expect("the candidate");

    let drafts: Vec<_> = engine
        .journal()
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
    println!("UNDO_DRAFTS={drafts:?} undo_intent={:?}", undo_intent.0);
    assert_eq!(
        drafts.len(),
        2,
        "one draft for the forward intent, one for the undo's (43 §5-1: both records are written) (sem: SEM-gx-engine-927)"
    );
    assert!(
        drafts.contains(&(undo_intent, 111)),
        "the undo's draft names its intent and carries the seed 42 §3.13 promises a replay"
    );
}
