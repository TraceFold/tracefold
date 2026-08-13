//! **AC-037** — `mode=RecordOnly ∧ verdict=Deny ∧ status=Committed ⇒ receipt.enforced=false`.
//!
//! 34 AC-037 逐語 (「record-only enforced=false横断の目玉」):
//!
//! > Given: `EnforcementMode::RecordOnly`が有効な対象substrate。When: Cedar/invariant評価がDenyを
//! > 返すTransformationをsubmit→verify→canonicalize(T-8r)→commitまで通す。Then: Tが`Committed`へ
//! > 到達し、発行Receiptの`enforced`フィールドが必ず`false`。プロパティ
//! > `mode=RecordOnly ∧ verdict=Deny ∧ status=Committed ⇒ receipt.enforced=false`を全生成ケースで
//! > 検証（NFR-006と同一命題）。 | property（proptest, 全mode×全verdict組合せ）
//!
//! # 🔴 Why this criterion could not be written until **E-M5-11**
//!
//! 「全mode×全verdict組合せ」 has **eight** cells, and one of them is 43 T-4e's degraded admission:
//! a transformation that reaches `Committed` with `enforced=false` and **no verdict at all**,
//! because the gate was never asked. Until §41 made `ReceiptPayload.verdict` an `Option`, hand 4's
//! `commit` refused that cell with `Error::Unrepresentable` — so a property over 「every generated
//! case」 either skipped a row or failed. §41 says so in as many words: 「実装窓=**手 6**(AC-037 が
//! この経路を正面から踏む)」. [`the_grid`] walks all eight.
//!
//! # Two instruments, because the criterion has two halves
//!
//! * [`ac_037_the_whole_grid_of_modes_and_verdicts`] is **exhaustive**: 2 modes × 4 verdict drivers,
//!   every cell run end to end and its receipt read. A grid is stronger than sampling where the
//!   space is finite, and this one is.
//! * [`ac_037_the_property_holds_for_every_generated_case`] is the proptest 34 asks for, over
//!   randomly chosen cells, seeds and locators. What it adds is that the implication does not
//!   depend on the fixtures the grid happens to use.
//!
//! **M5-15 採(b)** is why the second one is plain `proptest` and not `proptest-state-machine`:
//! 「素の proptest で `Vec<Event>` 生成の自前 model(**external 238 不変**)」.

mod support;

use std::sync::Arc;

use gx_core::{EnforcementMode, FailPosture, Timestamp, VerdictKind};
use gx_engine::{Engine, Lifecycle};
use proptest::prelude::*;
use support::{
    gate, intent, scratch, signing_key, CommitAdapter, MaybeEvidence, FORBID_ETC, PERMIT_ALL,
};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The four roads to a verdict-stage outcome, which is what 「全verdict」 enumerates.
///
/// Three are 43 T-4a/T-4b/T-4c and the fourth is T-4e — 43 §4 calls it 「record-onlyモード相当へ
/// 降格」, so it belongs on the verdict axis of a criterion about `enforced` even though no gate
/// answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Driver {
    /// T-4a: the policy set permits.
    Admit,
    /// T-4b: an invariant refuses this exact change.
    Deny,
    /// T-4c: **E-M3-4** — the adapter cannot build an inverse.
    Escalate,
    /// T-4e: the collector cannot be reached and the posture is `FailOpen`.
    Degraded,
}

impl Driver {
    const ALL: [Driver; 4] = [
        Driver::Admit,
        Driver::Deny,
        Driver::Escalate,
        Driver::Degraded,
    ];
}

/// What one cell of the grid produced.
#[derive(Debug)]
struct Outcome {
    verdict: Option<VerdictKind>,
    state: Lifecycle,
    receipt_enforced: Option<bool>,
    receipt_verdict_present: Option<bool>,
    fail_posture_engaged: bool,
    leaves: u64,
    applies: usize,
}

/// Run one cell: submit → plan → verify → canonicalize → commit, refusing nothing on the way.
fn run(name: &str, mode: EnforcementMode, driver: Driver, seed: u64) -> Outcome {
    let dir = scratch(name);
    let (evidence, posture) = match driver {
        Driver::Degraded => (MaybeEvidence::down(), FailPosture::FailOpen),
        _ => (MaybeEvidence::none(), FailPosture::FailClosed),
    };
    // The `Deny` road is an invariant rather than a Cedar rule, because AC-040 needs the same
    // fixture to deny an undo and permit its original -- see `support::DenyPayload`. Here the
    // simpler road is enough, and FORBID_ETC is the locator-based one hand 2 already uses.
    let (gate, locator) = match driver {
        Driver::Deny => (gate(FORBID_ETC), format!("/etc/target-{seed}.txt")),
        _ => (gate(PERMIT_ALL), format!("/tmp/target-{seed}.txt")),
    };
    let mut engine = Engine::open(dir.join("journal.bin"), gate, evidence)
        .expect("a fresh journal opens")
        .with_mode(mode)
        .with_posture(posture);
    let (adapter, counts, _world) = CommitAdapter::new("before");
    let adapter = match driver {
        Driver::Escalate => adapter.without_inverse(),
        _ => adapter,
    };
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent(&locator, "after");
    engine.submit(&i, seed, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let mut state = engine
        .verify(&id, AT, &signing_key(), None)
        .expect("T-3..T-4e");
    // 43 §1: `Denied` is terminal 「ただしrecord-onlyモード時のみ」, and `Escalated` waits for a
    // person. Both refusals are states rather than errors, so the pipeline simply stops.
    if engine.canonicalize(&id, AT, None).is_ok() {
        state = engine
            .commit(&id, AT, &signing_key())
            .expect("T-9..T-11 or a journalled abort");
    }
    let payload = engine.receipt(&id).map(|r| r.payload().expect("decodes"));
    Outcome {
        verdict: engine.verdict(&id),
        state,
        receipt_enforced: payload.as_ref().map(|p| p.enforced),
        receipt_verdict_present: payload.as_ref().map(|p| p.verdict.is_some()),
        fail_posture_engaged: engine.fail_posture_engaged(&id).unwrap_or(false),
        leaves: engine.ledger().log().len(),
        applies: counts.totals()[4],
    }
}

/// The implication AC-037 and NFR-006 state, as a function of one cell's outcome.
fn implication_holds(mode: EnforcementMode, out: &Outcome) -> bool {
    if mode == EnforcementMode::RecordOnly
        && out.verdict == Some(VerdictKind::Deny)
        && out.state == Lifecycle::Committed
    {
        return out.receipt_enforced == Some(false);
    }
    true
}

// ---------------------------------------------------------------------------
// The grid
// ---------------------------------------------------------------------------

/// 🔴 AC-037, exhaustively: 2 modes × 4 verdict drivers, every receipt read.
#[test]
fn ac_037_the_whole_grid_of_modes_and_verdicts() {
    let mut cells = 0usize;
    let mut committed_denials = 0usize;
    for mode in EnforcementMode::ALL {
        for driver in Driver::ALL {
            let out = run(
                &format!("ac037_{}_{driver:?}", mode.as_str()),
                mode,
                driver,
                100 + cells as u64,
            );
            println!(
                "AC037 mode={} driver={driver:?} verdict={:?} state={:?} \
                 receipt_enforced={:?} receipt_has_verdict={:?} fpe={} leaves={} applies={}",
                mode.as_str(),
                out.verdict,
                out.state,
                out.receipt_enforced,
                out.receipt_verdict_present,
                out.fail_posture_engaged,
                out.leaves,
                out.applies
            );
            assert!(
                implication_holds(mode, &out),
                "AC-037 fails at ({}, {driver:?}): {out:?}",
                mode.as_str()
            );

            match (mode, driver) {
                // T-4b under `Enforce`: 43 §1 makes `Denied` terminal, which is AC-041's half.
                (EnforcementMode::Enforce, Driver::Deny) => {
                    assert_eq!(out.state, Lifecycle::Denied);
                    assert_eq!(out.leaves, 0);
                    assert_eq!(out.applies, 0);
                    assert!(
                        out.receipt_enforced.is_none(),
                        "no commit, no CommitReceipt"
                    );
                }
                // 🔴 T-8r: the目玉. Denied, carried through, and the receipt says it was not enforced.
                (EnforcementMode::RecordOnly, Driver::Deny) => {
                    committed_denials += 1;
                    assert_eq!(out.state, Lifecycle::Committed);
                    assert_eq!(out.receipt_enforced, Some(false));
                    assert_eq!(
                        out.receipt_verdict_present,
                        Some(true),
                        "a gate did answer; the verdict is Deny and it is on the receipt"
                    );
                    assert_eq!(out.leaves, 1);
                    assert_eq!(
                        out.applies, 1,
                        "「適用は通ったが、ポリシー上は拒否されていた」"
                    );
                }
                // T-4c: a person has not answered, so nothing proceeds in either mode (INV-S6).
                (_, Driver::Escalate) => {
                    assert_eq!(out.state, Lifecycle::Escalated);
                    assert_eq!(out.leaves, 0);
                    assert_eq!(out.applies, 0);
                }
                // 🔴 T-4e in both modes: **the cell E-M5-11 opened**.
                (_, Driver::Degraded) => {
                    assert_eq!(out.state, Lifecycle::Committed);
                    assert_eq!(out.verdict, None, "no gate ran");
                    assert_eq!(out.receipt_enforced, Some(false), "43 T-4e");
                    assert_eq!(
                        out.receipt_verdict_present,
                        Some(false),
                        "E-M5-11: the seat is empty rather than filled with a minted digest"
                    );
                    assert!(out.fail_posture_engaged);
                }
                (_, Driver::Admit) => {
                    assert_eq!(out.state, Lifecycle::Committed);
                    assert_eq!(out.receipt_enforced, Some(true));
                    assert_eq!(out.receipt_verdict_present, Some(true));
                }
            }
            cells += 1;
        }
    }
    println!("AC037_CELLS={cells} COMMITTED_DENIALS={committed_denials}");
    assert_eq!(cells, 8, "2 modes × 4 verdict drivers");
    assert_eq!(
        committed_denials, 1,
        "exactly one cell is 「Deny が Committed に到達した」, and it is RecordOnly's"
    );
}

// ---------------------------------------------------------------------------
// The property
// ---------------------------------------------------------------------------

/// 🔴 The proptest 34 names, over randomly chosen cells (**M5-15 採(b)**: plain `proptest`).
///
/// The grid above says the implication holds for the eight fixtures this suite wrote. This says it
/// holds for cells chosen by the generator, with seeds and locators the fixtures did not pick —
/// which is the difference between 「these eight pass」 and 「the implication is true」.
#[test]
fn ac_037_the_property_holds_for_every_generated_case() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: 24,
        ..ProptestConfig::default()
    });
    let strategy = (0usize..2, 0usize..4, 1u64..10_000);
    let mut checked = 0usize;
    runner
        .run(&strategy, |(m, d, seed)| {
            let mode = EnforcementMode::ALL[m];
            let driver = Driver::ALL[d];
            let out = run(&format!("ac037_prop_{m}_{d}_{seed}"), mode, driver, seed);
            prop_assert!(
                implication_holds(mode, &out),
                "AC-037 fails at ({}, {driver:?}, seed {seed}): {out:?}",
                mode.as_str()
            );
            // The other half of NFR-006, stated positively: a receipt that says `enforced=true`
            // was issued for a transformation nothing degraded.
            if out.receipt_enforced == Some(true) {
                prop_assert_eq!(out.verdict, Some(VerdictKind::Admit));
                prop_assert!(!out.fail_posture_engaged);
            }
            Ok(())
        })
        .expect("the property holds");
    checked += 24;
    println!("AC037_PROPERTY_CASES={checked}");
}
