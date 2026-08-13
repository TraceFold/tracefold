//! **AC-036** — fail-closed, across many candidates at once (FR-036, NFR-005, 43 T-4d/T-4e).
//!
//! 34 AC-036 逐語:
//!
//! > Given: `FailPosture::FailClosed`（既定）。When: gx-gateプロセスを`kill -9`した状態で複数の
//! > Candidateを`verify_start`させる。Then: 全て`Aborted(VerifierUnavailable)`へ帰結し、`ledger`に
//! > Committed entryが1件も生成されない。substrate単位で`FailPosture::FailOpen`を設定した場合は
//! > 挙動が異なることも構成APIで確認する。
//!
//! # 🔴 「gx-gateプロセスを`kill -9`」 names a process that does not exist (**E-M5-4**)
//!
//! 41 §2 makes gx-gate a **library**: `Gate::verify` is a function call in this process, and a
//! function call cannot become unreachable. §37 rules the reading rather than leaving the criterion
//! unconstructible:
//!
//! > **M5-19 採(a)**=**E-M5-4**: 到達不能の唯一の source は evidence collector。AC-036 の
//! > 「gx-gate プロセスを kill -9」 は「evidence collector が到達不能」と読む erratum(gate は
//! > library=到達不能になりえない・NFR-005 の逐語「verifier/evidence collector到達不能」が既に
//! > この読みを許す)
//!
//! So the unreachable thing here is [`gx_engine::UnreachableEvidence`], and
//! `tests/engine_shape.rs::verifier_unavailable_has_exactly_one_producer` is what keeps that the
//! **only** road to the reason — without which this suite would be measuring one of several.
//!
//! # 「複数の」 is the part a single-transformation probe would miss
//!
//! Hand 2 measured one candidate reaching T-4d. What this suite adds is that the posture is a
//! property of the **deployment** and not of a call: five candidates over three objects all abort,
//! the ledger stays empty, and `adapter.apply` is never reached. A fail-closed engine that leaked
//! one commit out of five would pass every single-transformation probe ever written.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, FailPosture, Timestamp};
use gx_engine::{Engine, EvidenceSource, InjectedEvidence, Lifecycle, UnreachableEvidence};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// Plan `n` candidates over `n` objects in one engine, then verify all of them.
///
/// Returns `(states, apply_calls, ledger_leaves, journal_records)`.
fn chaos<E: EvidenceSource>(
    name: &str,
    evidence: E,
    posture: FailPosture,
    n: usize,
) -> (Vec<Lifecycle>, usize, u64, usize) {
    let dir = scratch(name);
    let mut engine = Engine::open(dir.join("journal.bin"), gate(PERMIT_ALL), evidence)
        .expect("a fresh journal opens")
        .with_posture(posture);
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let mut ids = Vec::new();
    for k in 0..n {
        let i = intent(&format!("/tmp/target-{k}.txt"), "after");
        engine.submit(&i, 40 + k as u64, AT).expect("submit");
        ids.push(engine.plan(&i, AT).expect("plan"));
    }
    let states: Vec<Lifecycle> = ids
        .iter()
        .map(|id| {
            engine
                .verify(id, AT, &signing_key(), None)
                .expect("verify answers with a state")
        })
        .collect();
    (
        states,
        counts.totals()[4],
        engine.ledger().log().len(),
        engine.journal().len(),
    )
}

/// 🔴 AC-036: every candidate aborts, and the ledger stays empty.
#[test]
fn ac_036_an_unreachable_collector_aborts_every_candidate_and_commits_none() {
    let (states, applies, leaves, records) = chaos(
        "ac036_closed",
        UnreachableEvidence::new("the collector was killed"),
        FailPosture::FailClosed,
        5,
    );
    println!(
        "AC036_CLOSED STATES={states:?} APPLY_CALLS={applies} LEDGER_LEAVES={leaves} \
         JOURNAL_RECORDS={records}"
    );
    assert_eq!(states.len(), 5, "「複数の Candidate」 is more than one");
    for state in &states {
        assert_eq!(
            *state,
            Lifecycle::Aborted(AbortReason::VerifierUnavailable),
            "43 T-4d: 「`FailPosture = FailClosed`（DR-2既定・全substrate）」"
        );
    }
    assert_eq!(
        leaves, 0,
        "AC-036: 「`ledger`にCommitted entryが1件も生成されない」"
    );
    assert_eq!(applies, 0, "nothing was applied, so P-4 was never at risk");
}

/// 🔴 AC-036's control: with the posture flipped, the same run behaves differently.
///
/// 「substrate単位で`FailPosture::FailOpen`を設定した場合は挙動が異なることも構成APIで確認する」.
/// The configuration API is [`Engine::with_posture`], and 43 §4 admits either scope
/// (「substrate単位または全体設定」) — v0.1 takes the whole-deployment reading, for the reason
/// `with_mode` gives: nothing in 42 declares a place to store a setting per `SubstrateKind`.
///
/// **Without this control the criterion is not measured.** A fail-closed engine that aborted
/// everything unconditionally would satisfy the probe above and fail the one property DR-2 is
/// about; the difference between the two runs is what says the posture is being read.
#[test]
fn ac_036_the_same_run_under_fail_open_is_admitted_instead() {
    let (states, applies, leaves, _records) = chaos(
        "ac036_open",
        UnreachableEvidence::new("the collector was killed"),
        FailPosture::FailOpen,
        5,
    );
    println!("AC036_OPEN STATES={states:?} APPLY_CALLS={applies} LEDGER_LEAVES={leaves}");
    for state in &states {
        assert_eq!(
            *state,
            Lifecycle::Admitted,
            "43 T-4e: 「当該Transformationに限りrecord-onlyモード相当へ降格して続行」"
        );
    }
    assert_eq!(leaves, 0, "T-4e admits; it does not commit");
    assert_eq!(applies, 0);
}

/// And the reachable collector is the third leg: the same five candidates get verdicts.
///
/// The two probes above differ in the posture; this one differs in the **collector**, which is
/// what shows that neither of them is measuring 「this engine aborts」. 「skip と pass を同じ顔に
/// しない」 (req/29 §4) applied to a suite rather than to a value.
#[test]
fn ac_036_a_reachable_collector_leaves_the_same_candidates_admitted() {
    let (states, applies, leaves, _records) = chaos(
        "ac036_reachable",
        InjectedEvidence::none(),
        FailPosture::FailClosed,
        5,
    );
    println!("AC036_REACHABLE STATES={states:?} APPLY_CALLS={applies} LEDGER_LEAVES={leaves}");
    for state in &states {
        assert_eq!(*state, Lifecycle::Admitted, "PERMIT_ALL admits");
    }
    assert_eq!(applies, 0, "verify does not apply");
}
