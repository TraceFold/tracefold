// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-039 (FR-039) — deterministic replay: **Σ, rebuilt from the journal, is the Σ the engine held**.
//!
//! 34 AC-039, verbatim: "Given: a Committed Transformation series on the ledger (generated with
//! seed=42, a fixed clock value T0). When: `gx replay` is run with the same seed/clock. Then: **the
//! reconstructed result state is bit-equal to the original result state**. Also confirm with a
//! **control experiment** that agreement is not guaranteed under a different seed." (sem:
//! SEM-gx-engine-481)
//!
//! 32 FR-039: "gx-engine MUST implement deterministic replay (reconstructing the same result state
//! from the same input) from the ledger's recorded Transformation series. Because randomness and
//! time are injected at the engine boundary, it must be testable that a replay's result under the
//! same seed/clock is bit-equal." (sem: SEM-gx-engine-481)
//!
//! # "result state" = Σ, and Σ is these four components (**E-M5-2**)
//!
//! req/38 §37 rules what the criterion is about, because 34 does not say whether "result state" is (sem: SEM-gx-engine-481)
//! the engine's state or the substrate's:
//!
//! > **M5-02, adopted (a)** = **E-M5-2**: replay is **a read-only operation that reconstructs Σ (sem: SEM-gx-engine-482)
//! > only**; AC-039's "result state" is read as Σ (state table + ledger root + escrow index). It
//! > never calls the adapter (mechanically checked). (sem: SEM-gx-engine-481)
//!
//! So Σ = `(drafts, state table, escrow index, ledger)`, and the `S` reading is refused for the
//! reason req/78 §3.2 Λ7 gives: a replay that rebuilt the *substrate* would write to it, and FR-035
//! forbids the engine to write to a substrate at all.
//!
//! # Why "same seed/clock" needs no re-injection
//!
//! The criterion says "run with the same seed/clock", and under E-M5-2 a replay does not re-run (sem: SEM-gx-engine-483)
//! anything — so where do the seed and the clock come in? **The seed is in the records**:
//! `DraftCreated` carries it (42 §3.13: "determinism is secured by re-running under the same seed (sem: SEM-gx-engine-483)
//! at replay time", sem: SEM-gx-engine-483), a replay reads what was injected instead of
//! injecting it again, and "same seed" is therefore a property of the journal rather than a promise
//! about the caller. That is what makes the **control experiment** real rather than staged: change
//! the seed, and the reconstructed bytes move.
//!
//! 🔴 **The clock is a different story, and it was found by a probe failing.**
//! [`ac_039_the_clock_does_not_reach_sigma_because_42_1_3_excludes_created_at`] was written as the
//! seed control's twin and failed on its own claim: 42 §1.3's exclusion table puts `created_at`
//! outside the `TransformationId`'s preimage ("excluded: `id`, `created_at` / reason: self-reference
//! / ASM-4", sem: SEM-gx-engine-484), and Σ holds state rather than the `at` of the records that
//! produced it. So the clock reaches Σ **nowhere** in v0.1, replay is clock-independent — stronger (sem: SEM-gx-engine-484)
//! than the criterion asks — and "same clock" constrains nothing here. The probe now measures that
//! fact, and the report raises it.
//!
//! # The comparison is not journal-against-journal
//!
//! `Engine::sigma` builds Σ from the engine's own tables and `replay(..).sigma()` builds it from the
//! bytes on disk. If the first one replayed the journal, this whole file would be comparing a value
//! with itself; `tests/store_shape.rs::the_engine_builds_sigma_from_its_tables_and_not_from_its_journal`
//! is the probe that keeps that from happening, and `tools/verify_m5h3.sh` §4 mutates `sigma` into a
//! journal read to show that it notices.

mod support;

use std::sync::Arc;

use gx_core::{FailPosture, Timestamp};
use gx_engine::{replay, Engine, EvidenceSource, InjectedEvidence, Lifecycle, UnreachableEvidence};
use proptest::prelude::*;
use support::{
    gate, intent, scratch, signing_key, CountingAdapter, StubAdapter, FORBID_ETC, PERMIT_ALL,
};

/// 34 AC-039's "fixed clock value T0" (sem: SEM-gx-engine-485).
const T0: Timestamp = Timestamp(1_754_000_000_000_000_000);
/// 34 AC-039's "seed=42" (sem: SEM-gx-engine-485).
const SEED: u64 = 42;

/// An engine with the stub adapter registered and a policy set chosen by the caller.
fn engine<E: EvidenceSource>(name: &str, policies: &str, evidence: E) -> Engine<E> {
    let dir = scratch(name);
    let mut e = Engine::open(dir.join("journal.bin"), gate(policies), evidence)
        .expect("a fresh journal opens");
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    e
}

/// The bytes of Σ as the engine holds it, and as the journal on disk reconstructs it.
///
/// Both sides are canonical DAG-CBOR through gx-canon (41 §6), which is what "bit-equal" (sem:
/// SEM-gx-engine-486) is
/// measured on.
fn both_sides<E: EvidenceSource>(e: &Engine<E>) -> (Vec<u8>, Vec<u8>) {
    let live = e.sigma().canonical_bytes().expect("Σ has a canonical form");
    let bytes = std::fs::read(e.journal().path()).expect("the journal is on disk");
    let replayed = replay(&bytes)
        .sigma()
        .canonical_bytes()
        .expect("the reconstructed Σ has a canonical form");
    (live, replayed)
}

/// Walk one intent as far as the mode allows: submit → plan → verify → canonicalize where 43 lets it.
///
/// A script that names the same intent twice is a script that walks one transformation once: T-1 is
/// create-if-absent and hand 2's T-2 refuses to rewind a row that has moved past `Candidate`
/// (`InvalidState`). That refusal is *returned* rather than swallowed here — the helper stops, and
/// the run carries on — because a repeated intent is a real thing a generated script produces and
/// treating it as a panic would make the property about the fixture.
fn walk<E: EvidenceSource>(e: &mut Engine<E>, locator: &str, goal: &str, at: Timestamp, seed: u64) {
    let i = intent(locator, goal);
    e.submit(&i, seed, at).expect("submit");
    let Ok(id) = e.plan(&i, at) else {
        return;
    };
    let state = e.verify(&id, at, &signing_key(), None).expect("verify");
    // `Denied` under `Enforce` is terminal (43 §1) and `Escalated` waits for hand 6, so only an
    // `Admitted` transformation is canonicalised here. Asking anyway would be asking for
    // `InvalidState`, and swallowing that would make this helper hide a transition that refused.
    if state == Lifecycle::Admitted {
        e.canonicalize(&id, at, None).expect("canonicalize");
    }
}

// ---------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------

/// 🔴 **AC-039**: with seed=42 and clock=T0, the reconstructed Σ is **bit-equal** to the original.
///
/// The script walks three transformations under one gate that denies anything under `/etc`, so the
/// state table holds an `Admitted → Canonicalized` pair and a `Denied` — three rows whose states,
/// verdicts, verdict digests, canonical CIDs and `enforced` flags all differ. A reconstruction that
/// got any one of them from the wrong record would move the bytes.
#[test]
fn ac_039_the_reconstructed_state_is_bit_equal_to_the_original() {
    let mut e = engine("ac039_bit_equal", FORBID_ETC, InjectedEvidence::none());
    walk(&mut e, "/tmp/a", "v1", T0, SEED);
    walk(&mut e, "/tmp/b", "v2", T0, SEED);
    walk(&mut e, "/etc/passwd", "v3", T0, SEED);

    let (live, replayed) = both_sides(&e);
    println!(
        "SIGMA_LIVE_BYTES={} SIGMA_REPLAYED_BYTES={} DRAFTS={} ROWS={} BIT_EQUAL={}",
        live.len(),
        replayed.len(),
        e.sigma().drafts().len(),
        e.sigma().transformations().len(),
        live == replayed
    );
    assert_eq!(
        e.sigma().transformations().len(),
        3,
        "the script is meant to leave three rows; a fixture that left fewer would make the \
         comparison cheaper than the criterion"
    );
    assert_eq!(
        live, replayed,
        "AC-039: the reconstructed result state is bit-equal to the original result state (sem: \
         SEM-gx-engine-487)"
    );
}

/// The same criterion where the gate was never asked (43 T-4e), which is the row most easily lost.
///
/// T-4e writes a `Verdict` record for a verdict that does not exist: `verdict_digest = None`,
/// `fail_posture_engaged = true`, and the transformation carries on as `Admitted` with
/// `enforced = false` (E-M5-7, INV-S5). A reconstruction that read that record as an ordinary
/// admission would produce a row that is `Admitted` with `enforced = true` — the difference INV-S5
/// exists to keep visible, and one this probe fails on.
#[test]
fn ac_039_a_degraded_admission_reconstructs_as_degraded() {
    let dir = scratch("ac039_t4e");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        UnreachableEvidence::new("the collector is down"),
    )
    .expect("a fresh journal opens")
    .with_posture(FailPosture::FailOpen);
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    walk(&mut e, "/tmp/degraded", "v1", T0, SEED);
    let id = e.transformation_ids()[0];
    assert_eq!(e.state(&id), Some(Lifecycle::Canonicalized));
    assert_eq!(e.enforced(&id), Some(false));
    assert_eq!(e.fail_posture_engaged(&id), Some(true));

    let (live, replayed) = both_sides(&e);
    let sigma = replay(&std::fs::read(e.journal().path()).expect("on disk")).sigma();
    let row = sigma.state_of(&id).expect("the row is reconstructed");
    println!(
        "T4E_ROW state={:?} verdict={:?} digest={:?} enforced={} fpe={}",
        row.state, row.verdict, row.verdict_digest, row.enforced, row.fail_posture_engaged
    );
    assert_eq!(row.verdict, None, "no gate ran, so no verdict exists");
    assert_eq!(row.verdict_digest, None, "and no proof to digest (E-M5-7)");
    assert!(
        !row.enforced,
        "43 §4: \"downgrade to record-only-equivalent mode\" (sem: SEM-gx-engine-488)"
    );
    assert!(
        row.fail_posture_engaged,
        "INV-S5 keeps the difference visible"
    );
    assert_eq!(live, replayed);
}

/// 🔴 The same row **before** it is canonicalised — found by a mutation that nothing caught.
///
/// [`ac_039_a_degraded_admission_reconstructs_as_degraded`] walks T-4e all the way to T-8, and that
/// is exactly why it could not see the `Verdict` arm's `enforced = false`: `Canonicalized` carries
/// `enforced: Some(false)` as well, so the later record repaired what a broken earlier one would
/// have got wrong. `tools/verify_m5h3.sh` §4 (g) flips that assignment to `true` and the whole suite
/// stayed green — a **masked claim**, which is the same shape as an absence scan with no presence
/// (§30) and is the reason a mutation battery is run before a hand is called done rather than after.
///
/// So this probe stops at `verify`. The degraded admission sits in `Admitted` with no
/// canonicalisation behind it, and the `Verdict` record is then the **only** thing that can say
/// `enforced = false`. That is also the state a crash between T-4e and T-8 leaves behind, which is
/// hand 5's window and one more reason the row has to survive replay on its own.
#[test]
fn ac_039_a_degraded_admission_is_unenforced_before_it_is_canonicalised() {
    let dir = scratch("ac039_t4e_uncanonicalised");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        UnreachableEvidence::new("the collector is down"),
    )
    .expect("a fresh journal opens")
    .with_posture(FailPosture::FailOpen);
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let i = intent("/tmp/degraded", "v1");
    e.submit(&i, SEED, T0).expect("submit");
    let id = e.plan(&i, T0).expect("plan");
    let state = e.verify(&id, T0, &signing_key(), None).expect("verify");
    assert_eq!(state, Lifecycle::Admitted, "T-4e admits, degraded");
    assert_eq!(
        e.canonical_cid(&id),
        None,
        "the fixture must not have canonicalised, or the later record masks the claim again"
    );

    let sigma = replay(&std::fs::read(e.journal().path()).expect("on disk")).sigma();
    let row = sigma.state_of(&id).expect("reconstructed");
    println!(
        "T4E_BEFORE_T8 state={:?} enforced={} fpe={} canonical={:?}",
        row.state, row.enforced, row.fail_posture_engaged, row.canonical_cid
    );
    assert!(
        !row.enforced,
        "the `Verdict` record's `fail_posture_engaged` is the only source of `enforced=false` here, \
         and the reconstruction lost it"
    );
    assert!(row.fail_posture_engaged);
    assert_eq!(row.canonical_cid, None);

    let (live, replayed) = both_sides(&e);
    assert_eq!(live, replayed);
}

/// `RecordOnly`'s T-8r reconstructs as `enforced=false` too (AC-037's shape, from the replay side).
#[test]
fn ac_039_a_record_only_denial_reconstructs_as_unenforced() {
    let mut e = engine("ac039_t8r", FORBID_ETC, InjectedEvidence::none())
        .with_mode(gx_core::EnforcementMode::RecordOnly);
    walk(&mut e, "/etc/shadow", "v1", T0, SEED);
    let id = e.transformation_ids()[0];
    e.canonicalize(&id, T0, None)
        .expect("T-8r opens for a denial under RecordOnly");

    let (live, replayed) = both_sides(&e);
    let sigma = replay(&std::fs::read(e.journal().path()).expect("on disk")).sigma();
    let row = sigma.state_of(&id).expect("reconstructed");
    println!(
        "T8R_ROW state={:?} verdict={:?} enforced={}",
        row.state, row.verdict, row.enforced
    );
    assert_eq!(row.verdict, Some(gx_core::VerdictKind::Deny));
    assert!(!row.enforced);
    assert_eq!(live, replayed);
}

// ---------------------------------------------------------------------------
// The control experiments (34 AC-039: "agreement is not guaranteed under a different seed", sem:
// SEM-gx-engine-489)
// ---------------------------------------------------------------------------

/// 🔴 **The control**: the same script under a **different seed** does not reconstruct the same Σ.
///
/// This is the probe that makes the criterion above worth stating. A Σ that ignored the injected
/// randomness would be bit-equal to itself under every seed, and "same seed/clock" (sem:
/// SEM-gx-engine-490) would be
/// decoration. So the difference is located as well as asserted: the **state table is identical**
/// (nothing in v0.1 consumes the seed yet) and the **draft rows differ**, which is exactly where 42
/// §3.13 puts the seed.
#[test]
fn ac_039_a_different_seed_does_not_reconstruct_the_same_state() {
    let mut a = engine("ac039_seed42", PERMIT_ALL, InjectedEvidence::none());
    walk(&mut a, "/tmp/a", "v1", T0, SEED);
    let mut b = engine("ac039_seed7", PERMIT_ALL, InjectedEvidence::none());
    walk(&mut b, "/tmp/a", "v1", T0, 7);

    let (_, replayed_a) = both_sides(&a);
    let (_, replayed_b) = both_sides(&b);
    let sigma_a = replay(&std::fs::read(a.journal().path()).expect("on disk")).sigma();
    let sigma_b = replay(&std::fs::read(b.journal().path()).expect("on disk")).sigma();

    // The state table on its own, as bytes: the component that must **not** move when only the seed
    // does. Locating the difference is what turns "the two Σ differ" into "the seed is what
    // differs" (sem: SEM-gx-engine-491), which is the claim 34 AC-039's control experiment is
    // actually about.
    let table_a = gx_canon::cbor::encode(&sigma_a.transformations()).expect("encodes");
    let table_b = gx_canon::cbor::encode(&sigma_b.transformations()).expect("encodes");
    println!(
        "SEED_42_DRAFTS={:?} SEED_7_DRAFTS={:?} EQUAL={}",
        sigma_a.drafts(),
        sigma_b.drafts(),
        replayed_a == replayed_b
    );
    assert_eq!(table_a, table_b, "the two runs walked the same script");
    assert_ne!(
        replayed_a, replayed_b,
        "AC-039's control: under a different seed there is no agreement (sem: \
         SEM-gx-engine-492). A Σ that were equal here would be a Σ the \
         injected randomness never reached"
    );
    assert_eq!(
        sigma_a.drafts()[0].intent_id,
        sigma_b.drafts()[0].intent_id,
        "the same intent has the same id -- the seed is not part of it (ASM-11)"
    );
    assert_ne!(
        sigma_a.drafts()[0].rng_seed,
        sigma_b.drafts()[0].rng_seed,
        "and the seed is what differs"
    );
}

/// 🔴 The other half of "same seed/clock" (sem: SEM-gx-engine-493), measured: **the clock
/// reaches Σ nowhere**.
///
/// This probe was written to be the seed control's twin — "a different clock moves the ids, so it
/// moves Σ" (sem: SEM-gx-engine-493) — and it **failed on its own claim**, which is how the fact
/// below was found rather than
/// assumed. 42 §1.3's exclusion table is explicit:
///
/// > \| `Transformation` \| `order`, `intent_id`, `subject`, `target`, `delta`, `context`, `actor`,
/// > `parents` \| `id`, **`created_at`** \| self-reference / ASM-4 (sem: SEM-gx-engine-494) \|
///
/// So `created_at` is outside the `TransformationId`'s preimage, and Σ holds **state**, not the
/// `at` of the records that produced it. Two runs of one script under different clocks reconstruct
/// to the same bytes.
///
/// That is **stronger** than AC-039 asks ("the replay result under the same seed/clock is
/// bit-equal" (sem: SEM-gx-engine-495) holds a
/// fortiori when the clock does not participate at all) and it is **narrower** than 32 FR-039's
/// sentence reads, because there is no clock in v0.1 whose change Σ would notice. The report raises
/// it: hand 4's `Committed` rows carry a `ledger_seq` rather than a time, and hand 4 is where a
/// receipt's `issued_at` first becomes engine state that a replay would have to reproduce.
#[test]
fn ac_039_the_clock_does_not_reach_sigma_because_42_1_3_excludes_created_at() {
    let mut a = engine("ac039_clock0", PERMIT_ALL, InjectedEvidence::none());
    walk(&mut a, "/tmp/a", "v1", T0, SEED);
    let mut b = engine("ac039_clock1", PERMIT_ALL, InjectedEvidence::none());
    walk(&mut b, "/tmp/a", "v1", Timestamp(T0.0 + 1), SEED);

    let (_, replayed_a) = both_sides(&a);
    let (_, replayed_b) = both_sides(&b);
    println!(
        "SAME_ID_UNDER_DIFFERENT_CLOCKS={} SAME_SIGMA={}",
        a.transformation_ids()[0] == b.transformation_ids()[0],
        replayed_a == replayed_b
    );
    assert_eq!(
        a.transformation_ids()[0],
        b.transformation_ids()[0],
        "42 §1.3 excludes `created_at` from the identity (ASM-4), so the clock does not name the \
         transformation"
    );
    assert_eq!(
        replayed_a, replayed_b,
        "and nothing else in Σ carries a timestamp either -- if this fails, something began to \
         record time as state and \"same clock\" has become a real condition (sem: \
         SEM-gx-engine-496)"
    );

    // The journals themselves **do** differ: every record carries its `at`. So the clock is
    // recorded and simply not part of the state -- which is the difference between "the engine did
    // not write the time down" and "the time is not what the engine is" (sem: SEM-gx-engine-497).
    let bytes_a = std::fs::read(a.journal().path()).expect("on disk");
    let bytes_b = std::fs::read(b.journal().path()).expect("on disk");
    assert_ne!(
        bytes_a, bytes_b,
        "the two journals are byte-identical, so this probe is not measuring the clock at all"
    );
}

// ---------------------------------------------------------------------------
// E-M5-2's second instrument: the replay touches no adapter
// ---------------------------------------------------------------------------

/// 🔴 **E-M5-2, behavioural half**: reconstructing Σ calls **no** adapter method.
///
/// The structural half is `tests/store_shape.rs` (no adapter named in `replay.rs`, and none in
/// `reconstruct`'s signature). This one registers a counting adapter, runs a script so that the
/// counters are non-zero — an instrument that only ever reads zero is measuring nothing — and then
/// compares the totals across a replay and across `Engine::sigma`.
///
/// Both are checked because they are the two roads to Σ, and "read-only" has to hold on both: an
/// engine that re-read a snapshot to answer "what is my state" (sem: SEM-gx-engine-498) would
/// make FR-035's boundary depend
/// on who asked.
#[test]
fn ac_039_reconstructing_sigma_calls_no_adapter() {
    let dir = scratch("ac039_no_adapter");
    let (adapter, counts) = CountingAdapter::new();
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    e.register_adapter(Arc::new(adapter), "stub-1");
    walk(&mut e, "/tmp/a", "v1", T0, SEED);

    let during = counts.totals();
    assert!(
        during.iter().sum::<usize>() > 0,
        "the counters never moved, so this probe would read zero whatever a replay did"
    );

    let bytes = std::fs::read(e.journal().path()).expect("on disk");
    let replayed = replay(&bytes).sigma();
    let after_replay = counts.totals();
    let live = e.sigma();
    let after_sigma = counts.totals();

    println!(
        "ADAPTER_CALLS_DURING_RUN={during:?} AFTER_REPLAY={after_replay:?} AFTER_SIGMA={after_sigma:?}"
    );
    assert_eq!(
        during, after_replay,
        "E-M5-2: \"never calls the adapter\" -- reconstructing Σ from the journal reached the \
         substrate (sem: SEM-gx-engine-499)"
    );
    assert_eq!(
        during, after_sigma,
        "`Engine::sigma` reached the substrate to answer a question about the engine"
    );
    assert_eq!(
        live.canonical_bytes().expect("canonical"),
        replayed.canonical_bytes().expect("canonical")
    );
}

// ---------------------------------------------------------------------------
// The property (34 AC-039: "integration + property", sem: SEM-gx-engine-500)
// ---------------------------------------------------------------------------

proptest! {
    // Sixty-four cases rather than proptest's default 256: each one opens a journal, writes to it
    // and fsyncs several times, so the cost is dominated by durability rather than by the property.
    // The number is stated here so that a reader knows the denominator (H-5).
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// 🔴 **The property**: for any script this hand can run, live Σ and reconstructed Σ agree.
    ///
    /// The generated part is what an operator varies: which objects, what they are changed to, how
    /// many, and — through the `/etc` prefix — whether the gate admits or denies. The seed and the
    /// clock are generated as well, so "same seed/clock" (sem: SEM-gx-engine-501) is exercised
    /// over a range rather than at one
    /// point.
    ///
    /// What this property does **not** cover is written down rather than implied: `Committed`,
    /// `InverseEscrowed`, `ApplyStarted` and `Superseded` records cannot appear, because the
    /// transitions that write them are hands 4 and 6. `tests/sigma_replay.rs` reaches those records
    /// by writing them into a journal directly, which is a weaker instrument (it measures the
    /// reconstruction against a journal a test wrote) and the honest one available now.
    #[test]
    fn ac_039_live_and_reconstructed_sigma_agree_for_any_script(
        script in prop::collection::vec((0u8..6, 0u8..4, any::<bool>()), 1..6),
        seed in any::<u64>(),
        clock in 1_000_000u64..2_000_000_000,
    ) {
        let dir = scratch("ac039_property");
        let mut e = Engine::open(
            dir.join("journal.bin"),
            gate(FORBID_ETC),
            InjectedEvidence::none(),
        ).expect("a fresh journal opens");
        e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

        for (object, goal, under_etc) in script {
            let locator = if under_etc {
                format!("/etc/o{object}")
            } else {
                format!("/tmp/o{object}")
            };
            walk(&mut e, &locator, &format!("v{goal}"), Timestamp(clock as i64), seed);
        }

        let (live, replayed) = both_sides(&e);
        prop_assert_eq!(live, replayed);
    }
}
