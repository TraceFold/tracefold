//! 🔴 **M6-07 採(b) / M5H8-16** — the subject index, and the oracle that says it answers the same
//! question the full scan answered.
//!
//! > **M5H8-16**: `conflicting_predecessor` の subject 索引は M6 reqdef の発火条件(長寿命 engine=
//! > `gx serve` が表を実際に育てる時)
//!
//! §47 fixed the order — measure the decay through `gx serve` first, then index, then re-measure —
//! and `req/95` carries both halves of that measurement. What this file carries is the part a
//! benchmark cannot say: **an index is a second answer to a question the table already answers, and
//! two answers are two things that drift.**
//!
//! So every probe here is an equality between the index and a full scan, driven through the public
//! surface only. There is no probe asserting 「the index is fast」: speed is `req/95`'s table, and a
//! suite that asserted a duration would be a flaky test about a machine.
//!
//! # What the index is *not*
//!
//! Not part of Σ. Like `Engine::resolved` (M6-02 採(a)) it is derived from the state table and lives
//! and dies with it, so `Engine::open` starts it empty exactly as it starts the table empty (M5H3-5:
//! the in-flight table is empty after a restart). A probe asserts that too — an index that survived a
//! restart the table did not would be an index describing rows that are gone.
//!
//! # The three doors into the table
//!
//! `plan` (T-2), `plan`'s rehydrating branch, and `rehydrate_committed` (M6H4-4). All three are
//! covered here, and the third is the one a reader is most likely to miss because a later hand wrote
//! it: a row that entered through it and not through the index would make a restarted project unable
//! to see its own predecessor.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use gx_core::{Subject, Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, CommitAdapter, StubAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The oracle: what a full scan of the table would say, per subject.
///
/// Deliberately written the slow way — the shape `conflicting_predecessor` had before this hand —
/// because that is the thing the index has to keep agreeing with.
fn by_scan<E: gx_engine::EvidenceSource>(
    engine: &Engine<E>,
) -> BTreeMap<Subject, Vec<TransformationId>> {
    let mut out: BTreeMap<Subject, Vec<TransformationId>> = BTreeMap::new();
    for id in engine.transformation_ids() {
        let Some(t) = engine.transformation(&id) else {
            continue;
        };
        out.entry(t.subject).or_default().push(id);
    }
    out
}

/// 🔴 The index equals the scan, over a table with many subjects **and repeats**.
///
/// Repeats matter: an index that stored one id per subject would pass a fixture where every subject
/// is distinct — which is exactly the fixture `crates/gx-api/benches/serve_throughput.rs` uses — and
/// would silently lose the second transformation of the same object, which is the only case
/// `conflicting_predecessor` exists for. A green benchmark and a lost conflict would look identical.
#[test]
fn the_index_answers_what_a_full_scan_answers() {
    let dir = scratch("subject_index_scan");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let mut planned = 0usize;
    for object in 0..6u64 {
        for goal in 0..2u64 {
            let i = intent(
                &format!("/tmp/subject-index/object-{object}"),
                &format!("after-{object}-{goal}"),
            );
            engine.submit(&i, object * 10 + goal, AT).expect("submit");
            engine.plan(&i, AT).expect("plan");
            planned += 1;
        }
    }

    let scan = by_scan(&engine);
    assert_eq!(scan.len(), 6, "six locators are six subjects for the stub");
    let mut indexed = 0usize;
    for (subject, want) in &scan {
        let mut got = engine.transformations_on(subject);
        let mut want = want.clone();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(
            got, want,
            "the index and the scan disagree about {subject:?}"
        );
        indexed += got.len();
    }
    assert_eq!(
        indexed, planned,
        "the index holds {indexed} rows and the table holds {planned}"
    );
    println!("SUBJECT_INDEX subjects={} rows={indexed}", scan.len());
}

/// 🔴 A subject the table has never seen answers **empty**, not 「every row」.
///
/// The failure this refuses is the one an index makes possible: a lookup that misses and falls back
/// to scanning the whole table would still be *correct*, would reintroduce exactly the cost the
/// index removed, and no correctness probe would ever see it.
#[test]
fn an_unknown_subject_answers_empty() {
    let dir = scratch("subject_index_unknown");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let i = intent("/tmp/subject-index/known", "after");
    engine.submit(&i, 1, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");

    assert!(
        engine
            .transformations_on(&Subject::Transformation(id))
            .is_empty(),
        "a subject with no rows answered non-empty"
    );
    let known = engine.transformation(&id).expect("the row exists").subject;
    assert_eq!(engine.transformations_on(&known), vec![id]);
}

/// 🔴 43 §8's wait is still entered — the index changed the search, not the answer.
///
/// Two transformations of **one** object over an adapter whose `commutation` answers `Conflicts`:
/// the first passes T-3, the second is asked to verify while the first is in flight, and 43 §8 makes
/// the second wait. A `blocked_by` that came back `None` after the index went in would be an index
/// that lost the only row that mattered.
#[test]
fn a_predecessor_on_the_same_subject_still_blocks() {
    let dir = scratch("subject_index_conflict");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.conflicting()), "commit-adapter-1");

    let first = intent("/tmp/subject-index/shared.txt", "after-one");
    engine.submit(&first, 1, AT).expect("submit");
    let a = engine.plan(&first, AT).expect("plan a");
    let second = intent("/tmp/subject-index/shared.txt", "after-two");
    engine.submit(&second, 2, AT).expect("submit");
    let b = engine.plan(&second, AT).expect("plan b");

    engine
        .verify(&a, AT, &signing_key(), None)
        .expect("verify a");
    let state = engine
        .verify(&b, AT, &signing_key(), None)
        .expect("verify b");

    assert_eq!(
        engine.blocked_by(&b),
        Some(a),
        "43 §8's wait was lost: b should be blocked by a"
    );
    assert_eq!(
        state,
        Lifecycle::Candidate,
        "a blocked row stays a Candidate (43 §8: 新たな状態は追加しない)"
    );
    let subject = engine.transformation(&a).expect("a exists").subject;
    let mut on = engine.transformations_on(&subject);
    let mut both = vec![a, b];
    on.sort_unstable();
    both.sort_unstable();
    assert_eq!(on, both, "both rows are on the same subject");
}

/// 🔴 A restart empties the index exactly as it empties the table (M5H3-5).
#[test]
fn a_restart_leaves_the_index_as_empty_as_the_table() {
    let dir = scratch("subject_index_restart");
    let subject = {
        let mut engine = Engine::open(
            dir.join("journal.bin"),
            gate(PERMIT_ALL),
            InjectedEvidence::none(),
        )
        .expect("a fresh journal opens");
        engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
        let i = intent("/tmp/subject-index/restarted", "after");
        engine.submit(&i, 1, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        engine.transformation(&id).expect("the row exists").subject
    };

    let reopened = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the journal reopens");
    assert!(
        reopened.transformation_ids().is_empty(),
        "M5H3-5: the in-flight table is empty after a restart"
    );
    assert!(
        reopened.transformations_on(&subject).is_empty(),
        "the index outlived the table it indexes"
    );
}

/// 🔴 Rehydration puts the row back **into the index**, not only into the table.
///
/// `Engine::rehydrate_committed` (M6H4-4) is the third of the three doors into the table and the one
/// a reader is most likely to miss, because M6 hand 4 wrote it and M5 hand 2 wrote the other two.
#[test]
fn a_rehydrated_row_is_in_the_index() {
    let dir = scratch("subject_index_rehydrate");
    let i = intent("/tmp/subject-index/rehydrated.txt", "after");
    let id = {
        let mut engine = Engine::open(
            dir.join("journal.bin"),
            gate(PERMIT_ALL),
            InjectedEvidence::none(),
        )
        .expect("a fresh journal opens");
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
        engine.submit(&i, 1, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        engine
            .verify(&id, AT, &signing_key(), None)
            .expect("verify");
        engine.canonicalize(&id, AT, None).expect("canonicalize");
        assert_eq!(
            engine.commit(&id, AT, &signing_key()).expect("commit"),
            Lifecycle::Committed
        );
        id
    };

    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the journal reopens");
    let (adapter, _counts, _world) = CommitAdapter::new("after");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
    assert!(engine.transformation_ids().is_empty(), "a fresh table");
    assert!(
        engine.rehydrate_committed(&id, &i).expect("rehydrate"),
        "the committed row rehydrates"
    );
    let subject = engine.transformation(&id).expect("the row is back").subject;
    assert_eq!(
        engine.transformations_on(&subject),
        vec![id],
        "the rehydrated row reached the table and not the index"
    );
}
