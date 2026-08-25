// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 51 §14's branch-coverage gate, as two probes over `tests/state_machine_coverage.md` — and the
//! one transition nobody had walked.
//!
//! 51 §14, verbatim: "gx-engine state-machine transitions (43 §3's transition table ...) | branch
//! coverage | 100% (every transition walked by at least one test) | maintain a transition-id ->
//! test-name traceability table at `tests/state_machine_coverage.md` (a CC-generated artefact, the
//! same kind of thing as conformance-report.md) and run a lint script in CI that detects missing
//! transitions" (sem: SEM-gx-engine-867), and "if even one is unwalked, the M's completion
//! condition is not met".
//!
//! # The denominator is 21, and it is read from the file
//!
//! **M5H2-7, adopted (a)** (req/38 §39): "**the denominator for state machine branch coverage is
//! 21**. req/78 §4's M5-14 / §6.2 hand 7's "19" is corrected as stale, and §37's M5-14, adopted
//! (a)'s "19/19 is achievable" is likewise read as **21/21**" (sem: SEM-gx-engine-868).
//! 51 §14's own closing sentence lists nineteen ids and omits `T-4e` and `T-13` — the two that are
//! hardest to reach, which is exactly the pair a hand counting from the sentence would drop. So
//! nothing here counts from a sentence: `lifecycle_states.rs` parses 43 §3's table and this file
//! parses it again beside the artefact, so a table row added to the canon makes the lint red rather
//! than making the report wrong.
//!
//! # Why the lint checks the probe names as well as the ids
//!
//! A traceability table is a document, and a document can name a test that does not exist. "21 rows
//! present" and "21 transitions covered" (sem: SEM-gx-engine-869) are the same sentence only
//! if every name in the fourth
//! column resolves to a `#[test] fn` in the suite it names — so the second probe resolves all of
//! them. Without it the artefact would be a promise, and §30's lesson about absence scans applies to
//! presence scans too: a check that has never seen a miss may be checking nothing.
//!
//! # 🔴 T-13, and what this hand found about it
//!
//! req/84 §5.3 reported "**this hand has not reached T-13 either** (the gate's ⊥ and `cas_eq`'s
//! `Err` -- two roads exist but neither has been walked)" (sem: SEM-gx-engine-870). Measured
//! here, the first half of that is **wrong**: `ac_032.rs`'s
//! `e_m5_5_the_gates_bottom_aborts_whatever_the_enforcement_mode_says` has walked
//! `Verifying → Aborted(InternalError)` since hand 2, which is 43 T-13 from one of its eight
//! from-states. What was genuinely unwalked is the road **M5-24, adopted (a)** named as "the first
//! of the two roads to walk T-13" (sem: SEM-gx-engine-871) — `cas_eq`'s `Err` inside the
//! critical section — and the two probes below walk both of
//! its clauses (**E-M4-27**: substrate first, then scope).
//!
//! That road matters more than the gate's ⊥ because of where it happens. T-13 from `Verifying` is an
//! abort before anything has moved; T-13 from `Committing` is an abort **between** `CommittingStarted`
//! and `apply`, and the thing that has to be true there is that the world did not move and the ledger
//! stayed empty. Both are asserted, because "it returned InternalError" (sem: SEM-gx-engine-872)
//! is the cheap half.
//!
//! The mis-wiring is built in the fixture and not in the engine. "constructing, inside the engine,
//! two fingerprints that disagree on scope or substrate for `cas_eq`'s `Err` is a reproduction of a
//! wiring bug" (sem: SEM-gx-engine-872) — a shipping line that could
//! construct two disagreeing fingerprints would *be* the defect T-13 receives, so the injection sits
//! where hand 5 put its recovery shim: inside the test, in `support::CommitAdapter::miswired` /
//! `::rescoping`.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, Timestamp};
use gx_engine::{Engine, EngineJournalRecord, InjectedEvidence, Lifecycle};
use support::{gate, intent, read_repo, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The artefact 51 §14 names, relative to the repository root.
const TABLE: &str = "crates/gx-engine/tests/state_machine_coverage.md";

// ---------------------------------------------------------------------------
// The lint (51 §14: "a lint script that detects missing transitions in CI", sem: SEM-gx-engine-873)
// ---------------------------------------------------------------------------

/// The transition ids 43 §3's table declares, in the order the table gives them.
///
/// The same parse `lifecycle_states.rs` runs, and deliberately a second copy rather than a shared
/// helper: that file's claim is "the denominator is 21" and this file's is "the artefact covers the
/// denominator" (sem: SEM-gx-engine-874), and a shared reader would let one wrong parse make
/// both probes agree.
fn transitions_declared_by_43_3() -> Vec<String> {
    let text = read_repo("req/spec/40-architecture/43-state-machine.md");
    let section = text
        .split("## 3. ") // (sem: SEM-gx-engine-875)
        .nth(1)
        .expect("43 has a §3")
        .split("\n## ")
        .next()
        .expect("§3 is followed by §4");
    section
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("| T-"))
        .map(|l| {
            l.trim_start_matches("| ")
                .split(' ')
                .next()
                .expect("an id in the first column")
                .to_string()
        })
        .collect()
}

/// One row of the artefact: its transition id and the `suite::fn` references in its probe column.
struct Row {
    id: String,
    probes: Vec<String>,
}

/// The artefact's rows, parsed out of its one table.
///
/// A row is a line whose first cell is a transition id. The probe column is the **fourth** cell and
/// every backticked token in it is taken, so a transition walked by two probes names two and both
/// are resolved.
fn rows() -> Vec<Row> {
    read_repo(TABLE)
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("| T-"))
        .map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let probes = cells
                .get(3)
                .expect("a row has a probe column")
                .split('`')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect();
            Row {
                id: (*cells.get(1).expect("a row has an id column")).to_string(),
                probes,
            }
        })
        .collect()
}

/// 🔴 **51 §14 / M5H2-7**: the artefact names every transition 43 §3 declares, and no other.
///
/// Both directions and the order, for `floor_doubt::f1`'s reason: a table shorter than the canon is
/// the uncovered transition 51 §14 refuses ("if even one is unwalked, the M's completion condition
/// is not met", sem: SEM-gx-engine-876), and a
/// table longer than it is a row about a transition that no longer exists — a covered id nobody can
/// find in the spec is as much a drift as a missing one.
#[test]
fn the_coverage_table_names_every_transition_43_3_declares() {
    let canon = transitions_declared_by_43_3();
    let rows = rows();
    let covered: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();

    let missing: Vec<&String> = canon.iter().filter(|id| !covered.contains(id)).collect();
    let extra: Vec<&String> = covered.iter().filter(|id| !canon.contains(id)).collect();

    println!(
        "BRANCH_COVERAGE={}/{} MISSING={missing:?} EXTRA={extra:?}",
        covered.len(),
        canon.len()
    );
    assert_eq!(
        canon.len(),
        21,
        "the denominator 51 §14 gates on is 43 §3's table, which is 21 rows (M5H2-7)"
    );
    assert!(
        missing.is_empty(),
        "51 §14: \"if even one is unwalked, the M's completion condition is not met\" (sem: \
         SEM-gx-engine-877) -- {TABLE} names no probe for \
         {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "{TABLE} covers {extra:?}, which 43 §3 no longer declares"
    );
    assert_eq!(
        covered, canon,
        "{TABLE}'s rows are not 43 §3's rows in 43 §3's order"
    );
}

/// Every `suite::fn` the artefact names is a `#[test] fn` that exists.
///
/// The half that makes the table a measurement rather than a claim. A name is resolved by reading
/// the suite file it names and looking for the function's definition, which is what a reader would
/// do — and, unlike a reader, it is done for all of them on every run.
#[test]
fn every_probe_the_coverage_table_names_is_a_test_that_exists() {
    let mut checked = 0_usize;
    let mut unresolved: Vec<String> = Vec::new();
    for row in rows() {
        assert!(
            !row.probes.is_empty(),
            "{} names no probe, and an empty cell is not coverage (req/29 §4)",
            row.id
        );
        for reference in row.probes {
            checked += 1;
            let (suite, function) = reference
                .split_once("::")
                .unwrap_or_else(|| panic!("{reference} is not written as `suite::fn`"));
            let path = format!("crates/gx-engine/tests/{suite}.rs");
            let source = std::fs::read_to_string(support::repo_root().join(&path))
                .unwrap_or_else(|e| panic!("{path} (named by {}) cannot be read: {e}", row.id));
            if !source.contains(&format!("fn {function}(")) {
                unresolved.push(format!("{reference} (no `fn {function}` in {path})"));
            }
        }
    }
    println!(
        "COVERAGE_TABLE_PROBES={checked} UNRESOLVED={}",
        unresolved.len()
    );
    assert!(
        unresolved.is_empty(),
        "{TABLE} names probes that do not exist: {unresolved:?}. A traceability table that can \
         name a fiction is a document, not a gate"
    );
}

// ---------------------------------------------------------------------------
// 43 T-13, injected
// ---------------------------------------------------------------------------

/// A canonicalized transformation over `adapter`'s world, ready for the critical section.
fn canonicalised(
    name: &str,
    adapter: CommitAdapter,
    counts: Arc<support::Counts>,
) -> (
    Engine<InjectedEvidence>,
    gx_core::TransformationId,
    Arc<support::Counts>,
) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(engine.state(&id), Some(Lifecycle::Canonicalized));
    (engine, id, counts)
}

/// What a T-13 abort inside the critical section has to leave behind, whatever caused it.
fn assert_nothing_moved(
    engine: &Engine<InjectedEvidence>,
    id: &gx_core::TransformationId,
    counts: &support::Counts,
    world: &Arc<std::sync::Mutex<Vec<u8>>>,
) {
    assert_eq!(
        counts.totals()[4],
        0,
        "the CAS refused, so 43 INV-S7's \"in no case is `adapter.apply` called\" holds here too \
         (sem: SEM-gx-engine-878)"
    );
    assert_eq!(
        &*world.lock().expect("the world is not poisoned"),
        b"before",
        "an InternalError inside `Committing` must not leave the substrate changed"
    );
    assert_eq!(
        engine.ledger().log().len(),
        0,
        "INV-S4: an Aborted transformation does not appear in the ledger"
    );
    assert!(
        engine.receipt(id).is_none(),
        "ASM-14 issues a CommitReceipt for a commit, and this one did not happen"
    );
    assert!(
        engine.journal().records().iter().any(|r| matches!(
            r,
            EngineJournalRecord::Aborted {
                reason: AbortReason::InternalError,
                ..
            }
        )),
        "43 T-13's side effect is \"journal: `Aborted{{id, InternalError}}`\" (sem: SEM-gx-engine-879)"
    );
}

/// 🔴 **43 T-13, first clause**: an adapter that fingerprints under the wrong substrate.
///
/// **E-M4-27** answers the substrate mismatch before the scope one, and **M5-24, adopted (a)** is
/// the ruling that says where the answer goes: "`cas_eq`'s `Err` is `Aborted(InternalError)` (43
/// T-13, verbatim match -- the first of the two roads to walk T-13)" (sem: SEM-gx-engine-880).
/// The alternative the ruling names and rejects is
/// `PreconditionChanged`, and that is the assertion with teeth here — `PreconditionChanged` says
/// "someone else moved the world" (sem: SEM-gx-engine-880) and this deployment's world was
/// never touched, so folding a
/// wiring fault into it would file a bug as a business condition.
#[test]
fn t_13_a_miswired_adapter_is_an_internal_error_and_not_a_precondition_change() {
    let (adapter, counts, world) = CommitAdapter::new("before");
    let (mut engine, id, counts) =
        canonicalised("t13_miswired", adapter.miswired(), Arc::clone(&counts));

    let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");

    println!("T13_MISWIRED state={state:?} counts={:?}", counts.totals());
    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::InternalError),
        "M5-24, adopted (a): a `cas_eq` refusal is 43 T-13's \"a bug-grade failure\", not T-10a's \
         (sem: SEM-gx-engine-881)"
    );
    assert_ne!(
        state,
        Lifecycle::Aborted(AbortReason::PreconditionChanged),
        "E-M4-32's reasoning (sem: SEM-gx-engine-882): a wiring fault must not wear a business \
         condition's face"
    );
    assert_nothing_moved(&engine, &id, &counts, &world);
}

/// **43 T-13, second clause**: the substrates agree and the scopes do not (42 §3.5).
///
/// Unreachable while the first clause is in force — `cas_eq` answers the substrate mismatch and
/// returns — so it needs its own fixture. 42 §3.5 calls this comparison "meaningless" (sem:
/// SEM-gx-engine-883), which is a stronger statement than "unequal": there is no answer to
/// give, and an engine that returned
/// `Ok(false)` here would be inventing one.
#[test]
fn t_13_a_rescoped_fingerprint_reaches_the_same_state_by_the_other_refusal() {
    let (adapter, counts, world) = CommitAdapter::new("before");
    let (mut engine, id, counts) =
        canonicalised("t13_rescoped", adapter.rescoping(), Arc::clone(&counts));

    let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");

    println!("T13_RESCOPED state={state:?} counts={:?}", counts.totals());
    assert_eq!(
        state,
        Lifecycle::Aborted(AbortReason::InternalError),
        "42 §3.5's scope clause is the second of E-M4-27's two refusals and folds the same way"
    );
    assert_nothing_moved(&engine, &id, &counts, &world);
}

/// 🔴 T-13's from-set, measured: two of the eight states 43 T-13 lists are actually entered.
///
/// 43 T-13's from column is "any of {Draft,Candidate,Verifying,Admitted,Denied,Escalated,
/// Canonicalized, Committing}" (sem: SEM-gx-engine-884) — eight states, one transition id.
/// The lint above gates on the id and this probe says which of the eight v0.1 has a road into, so
/// "T-13 covered" is not read as "all eight arms covered". It is `Verifying` (the gate's ⊥,
/// **M5H2-5, adopted (a)**, walked since hand 2 by `ac_032.rs`) and `Committing` (the CAS's `Err`,
/// **M5-24, adopted (a)**, walked above). The other six have
/// no producer in v0.1, and that is a fact about the engine rather than a gap in the table:
/// `InternalError` is written by the fold of a refusal, and the refusals live where the engine calls
/// out.
#[test]
fn t_13_is_entered_from_two_states_and_v0_1_has_no_road_into_the_other_six() {
    // Verifying: the gate answers ⊥ (E-M3-3's `Result<Verdict>`), M5H2-5, adopted (a)
    // (sem: SEM-gx-engine-885).
    let dir = scratch("t13_from_verifying");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gx_gate::Gate::unconfigured(),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let from_verifying = engine.plan(&i, AT).expect("plan");
    let verifying = engine
        .verify(&from_verifying, AT, &signing_key(), None)
        .expect("verify runs");

    // Committing: the CAS refuses (M5-24, adopted (a)) (sem: SEM-gx-engine-886).
    let (adapter, counts, _world) = CommitAdapter::new("before");
    let (mut committing_engine, id, _counts) =
        canonicalised("t13_from_committing", adapter.miswired(), counts);
    let committing = committing_engine
        .commit(&id, AT, &signing_key())
        .expect("commit runs");

    println!(
        "T13_FROM_STATES verifying={verifying:?} committing={committing:?} \
         (43 T-13 lists eight; v0.1 has a road into two)"
    );
    assert_eq!(verifying, Lifecycle::Aborted(AbortReason::InternalError));
    assert_eq!(committing, Lifecycle::Aborted(AbortReason::InternalError));
    // Neither road is the collector's: M5-03, adopted (a), keeps `VerifierUnavailable` to one
    // producer (sem: SEM-gx-engine-887), and
    // an engine that reported ⊥ as unreachability would be saying the collector failed when the
    // policy layer did.
    for engine_records in [
        engine.journal().records(),
        committing_engine.journal().records(),
    ] {
        assert!(
            !engine_records.iter().any(|r| matches!(
                r,
                EngineJournalRecord::Aborted {
                    reason: AbortReason::VerifierUnavailable,
                    ..
                }
            )),
            "⊥ is not unreachability (E-M5-5)"
        );
    }
}
