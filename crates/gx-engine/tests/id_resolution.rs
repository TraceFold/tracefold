//! 🔴 **M6-02 採(a)** — 44 §0's id-resolution, from the engine's side.
//!
//! 44 §0 逐語: 「`gx plan`等、Draft/Candidateをまたいで対象を指定するコマンド・エンドポイントは
//! `IntentId`と`TransformationId`のいずれの`gx1:...`値も受理し（id-resolution規則）、`plan()`完了後は
//! 正準の`TransformationId`へ解決する」. req/88 §4 M6-02 measured that the engine had only the forward
//! map and called the hole a **hand 1 blocker**; req/38 §47 adopted (a)+(b) — the inverse is the
//! engine's and the `.gx/index/` copy is the CLI's cache.
//!
//! # The three claims, and why the third one needed a ruling
//!
//! 1. after `plan`, an `IntentId` resolves to the `TransformationId` it was planned into;
//! 2. before `plan` — while the intent is a **draft** — it resolves to nothing, which is E-M5-3 in
//!    the answer rather than in the type (43 T-1: 「`TransformationId`はまだ確定しない」);
//! 3. when one intent has been planned **more than once**, the answer is the most recently planned
//!    one, in **journal** order.
//!
//! The third is req/88 §3 Λ3(ii): 「43 §8 が…再`plan()`（再fingerprint）を強制する=同一 IntentId から
//! 2 つ目の TID が生じうるので r は多価になりうる」, with the note that the rule 「規則として書かれて
//! いない」. §6.2 手 1 ⑥ asks this hand to write it — 「再 plan で多価になる場合の規則を doc に 1 行」 —
//! and [`gx_engine::Engine::resolved`]'s documentation is that line. This file is the measurement of
//! it, including the part a doc line cannot state: that the order is the journal's and not the
//! table's, so it is the same before and after a restart.

mod support;

use std::sync::Arc;

use gx_core::{SubstrateKind, Timestamp};
use gx_engine::{Engine, EngineJournalRecord, InjectedEvidence};
use support::{gate, intent, scratch, StubAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

fn engine(path: &std::path::Path) -> Engine<InjectedEvidence> {
    let mut engine =
        Engine::open(path, gate(PERMIT_ALL), InjectedEvidence::none()).expect("a journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    engine
}

/// A draft resolves to nothing, and a candidate resolves to itself.
///
/// Both directions in one probe, because the pair is the claim: `intent_of` and `resolved` have to
/// be inverses on the transformations that exist, and `resolved` has to be **partial** on the ones
/// that do not. A total function here would have to invent a `TransformationId` for a draft, which
/// is the two-stage identity of ASM-11 collapsing into one.
#[test]
fn resolution_is_partial_before_plan_and_inverse_after_it() {
    let dir = scratch("id_res_partial");
    let mut e = engine(&dir.join("journal.bin"));
    let i = intent("/tmp/x", "v1");

    let intent_id = e.submit(&i, 42, AT).expect("submit");
    println!(
        "AFTER_SUBMIT resolved={:?} is_drafted={}",
        e.resolved(&intent_id),
        e.is_drafted(&intent_id)
    );
    assert!(
        e.resolved(&intent_id).is_none(),
        "a draft has no `TransformationId` to resolve to (E-M5-3 / 43 T-1); an answer here would \
         be an id nothing minted"
    );

    let tid = e.plan(&i, AT).expect("plan");
    println!(
        "AFTER_PLAN resolved={:?} intent_of={:?}",
        e.resolved(&intent_id),
        e.intent_of(&tid)
    );
    assert_eq!(
        e.resolved(&intent_id),
        Some(tid),
        "44 §0: after `plan()` the intent resolves to the canonical `TransformationId`"
    );
    assert_eq!(
        e.intent_of(&tid),
        Some(intent_id),
        "the forward half still holds, so the two are inverses where both are defined"
    );
}

/// An intent nobody submitted resolves to nothing, and so does a `TransformationId`'s worth of
/// noise. Fail-closed: 「読めない」 is not 「無い」 (E-M4-35), and an unknown id is 「無い」.
#[test]
fn an_unknown_intent_resolves_to_nothing() {
    let dir = scratch("id_res_unknown");
    let e = engine(&dir.join("journal.bin"));
    let unknown = gx_core::IntentId(gx_core::Cid([7u8; 32]));
    println!("UNKNOWN_RESOLVED={:?}", e.resolved(&unknown));
    assert!(e.resolved(&unknown).is_none());
}

/// A re-plan of the **same** intent against the **same** world is idempotent, and resolution is
/// unmoved by it.
///
/// 43 T-2's idempotency column: 「同一snapshotに対し再実行しても同一`PlannedDelta`・同一
/// `TransformationId`（安全に再試行可）」. The stub adapter answers as a function of its arguments, so
/// this is the case the test harness can produce directly — and it is the case where Λ3's
/// multi-valuedness does **not** arise. It is measured first so that the probe below is read as
/// 「the other case」 rather than as the only one.
#[test]
fn an_idempotent_replan_does_not_move_the_answer() {
    let dir = scratch("id_res_replan_same");
    let mut e = engine(&dir.join("journal.bin"));
    let i = intent("/tmp/x", "v1");
    let intent_id = e.submit(&i, 42, AT).expect("submit");

    let first = e.plan(&i, AT).expect("the first plan");
    let second = e.plan(&i, Timestamp(AT.0 + 1)).expect("the re-plan");
    println!(
        "IDEMPOTENT_REPLAN equal={} resolved={:?}",
        first == second,
        e.resolved(&intent_id)
    );
    assert_eq!(first, second, "43 T-2's idempotency column");
    assert_eq!(e.resolved(&intent_id), Some(first));
}

/// 🔴 **Λ3(ii)** — one intent, two transformations, and the rule that picks between them.
///
/// # Why this is measured at the journal and not through `plan`
///
/// The multi-valued case needs a **moved world**: 43 §8 forces a re-plan when a predecessor commits,
/// and the second plan then sees a different snapshot, mints a different delta and therefore a
/// different `TransformationId`. The stub adapter in `tests/support` answers as a pure function of
/// its arguments — which is the property AC-030 needs from it — so it cannot move. Reaching for the
/// real fs adapter would put an adapter in this crate's shipping-adjacent test surface for the sake
/// of one fixture (N-13's neighbourhood), and writing a mutable stub would change the instrument
/// every other suite in this crate shares.
///
/// So the two `Planned` records are written into a journal directly and the engine is opened on it.
/// That is the **weaker instrument** `sigma_replay.rs` is honest about — a journal a test wrote is
/// not a journal an execution wrote — and it is the right one here, because **the rule under test is
/// a rule about journal order**, not about planning. What it cannot show is that the engine's live
/// path produces this order; what it can show, and does, is that `open`'s rebuild and the rule agree.
/// The live half arrives when a hand has an adapter whose world moves (hand 3's `.gx/drafts/`
/// round-trip is the first).
#[test]
fn a_replanned_intent_resolves_to_the_most_recent_transformation() {
    let dir = scratch("id_res_replan");
    let path = dir.join("journal.bin");
    let one = support::tid(1);
    let two = support::tid(2);
    let shared = support::iid(1);
    {
        let mut journal = gx_engine::EngineJournal::open(&path).expect("a fresh journal opens");
        journal
            .append(EngineJournalRecord::DraftCreated {
                intent_id: shared,
                rng_seed: 42,
                at: AT,
            })
            .expect("append");
        for (t, at) in [(one, AT), (two, Timestamp(AT.0 + 1))] {
            journal
                .append(EngineJournalRecord::Planned {
                    transformation: t,
                    intent_id: shared,
                    locator: "/tmp/x".to_string(),
                    delta_cid: support::cid(11),
                    fp0: support::fp(1),
                    parents: Vec::new(),
                    at,
                })
                .expect("append");
        }
    }

    let e = engine(&path);
    println!(
        "REPLAN_JOURNAL first={:?} second={:?} resolved={:?}",
        one.0,
        two.0,
        e.resolved(&shared)
    );
    assert_ne!(one, two, "the two records name different transformations");
    assert_eq!(
        e.resolved(&shared),
        Some(two),
        "Λ3(ii): the rule is 「the most recently planned」, and 「recent」 is the journal's append \
         order -- not the table's, which is CID order and therefore arbitrary with respect to time"
    );
}

/// 🔴 The answer survives a restart, and it survives it **because it is derived from the journal**.
///
/// `Engine::open` deliberately leaves the state table empty (M5H3-5), so an index built from the
/// table would answer `None` after every restart while the journal still held the fact. This is the
/// probe that would catch that implementation, and it is the reason the index is rebuilt in `open`
/// beside `drafted` rather than maintained only at `plan`.
#[test]
fn resolution_is_rebuilt_from_the_journal_after_a_restart() {
    let dir = scratch("id_res_restart");
    let path = dir.join("journal.bin");
    let i = intent("/tmp/x", "v1");

    let (intent_id, tid) = {
        let mut e = engine(&path);
        let intent_id = e.submit(&i, 42, AT).expect("submit");
        let tid = e.plan(&i, AT).expect("plan");
        (intent_id, tid)
    };

    let reopened = engine(&path);
    println!(
        "AFTER_RESTART resolved={:?} table_is_empty={} journal_records={}",
        reopened.resolved(&intent_id),
        reopened.transformation_ids().is_empty(),
        reopened.journal().len()
    );
    assert!(
        reopened.transformation_ids().is_empty(),
        "the premise: `open` does not rebuild the state table (M5H3-5). If this ever changes the \
         probe below stops being about the journal"
    );
    assert_eq!(
        reopened.resolved(&intent_id),
        Some(tid),
        "M6-02: the inverse is journal-derived, so a restart cannot lose it"
    );
}

/// **E-M5-13** on the live path: the two fields carry the values `plan` was given.
///
/// `journal_vocabulary.rs` holds the *shape* against 42 §3.13 and this holds the *content*: the
/// locator is the intent's own (43 §7-3c has to name what the interrupted plan was against) and the
/// parents list is empty for a plan and non-empty for an undo — the case M5H6-6 raised.
#[test]
fn the_planned_record_carries_the_locator_it_was_given() {
    let dir = scratch("id_res_locator");
    let mut e = engine(&dir.join("journal.bin"));
    let i = intent("/tmp/deep/path", "v1");
    e.submit(&i, 42, AT).expect("submit");
    e.plan(&i, AT).expect("plan");

    let (locator, parents) = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Planned {
                locator, parents, ..
            } => Some((locator.clone(), parents.clone())),
            _ => None,
        })
        .expect("a Planned record");
    println!(
        "PLANNED_LOCATOR={locator:?} PLANNED_PARENTS={}",
        parents.len()
    );
    assert_eq!(
        locator, "/tmp/deep/path",
        "E-M5-13 (locator half, M5H5-2): the record names the position the plan was made against"
    );
    assert!(
        parents.is_empty(),
        "an order-0 plan has no predecessor; the non-empty case is `undo`'s (M5H6-6)"
    );
    assert_eq!(
        *i.substrate(),
        SubstrateKind::Fs,
        "the fixture is an fs intent, which is what makes the locator a path"
    );
}
