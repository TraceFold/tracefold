// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-040 / AC-044** — undo is a new gated transformation, and the original is not rewritten.
//!
//! 34 AC-040, verbatim ("the centerpiece of undo=a new gated transformation", sem:
//! SEM-gx-engine-502):
//!
//! > Given: an existing Committed Transformation T_o (target file "before"->"after", inverse
//! > escrowed). When: `gx undo <T_o.id>` is run. Then: (1) a new Candidate T_u independently walks
//! > the normal `submit->verify->canonicalize->commit` road (in a separate case where the
//! > invariant/policy corresponding to T_u is deliberately set to Deny, confirm T_u does not reach
//! > `Committed` and stays `Denied` -- undo is not exempted from gating either). (2) when T_u reaches
//! > `Committed`, `T_o.status` transitions to `Superseded` and `superseded_by=T_u.id` is recorded. (sem: SEM-gx-engine-502)
//! > (3) `T_o`'s canonical record, receipt, and ledger entry stay bit-equal, unchanged, comparing
//! > hashes before and after the commit. (sem: SEM-gx-engine-502)
//!
//! 34 AC-044 asks (3) again as a property of its own (INV-S2), and `tests/ac_044.rs` is where it is
//! generated rather than exercised once.
//!
//! # 🔴 43 §5 is the sentence this whole suite is about
//!
//! > There is **no** transition back from `Committed` to `Candidate` or `Admitted` ... T_u
//! > independently walks its own `Draft->Candidate->Verifying->...->Committed` (undo is not
//! > exempted from verification either -- fail-closed, P-4 keeps applying) (sem:
//! > SEM-gx-engine-503)
//!
//! req/78 §3.2 Λ6 is the same statement as mathematics: "undo is neither a category's inverse
//! element nor a groupoid's inverse morphism; it is the operation of placing two pieces of
//! 'cancellation evidence' side by side over `Σ`" (sem: SEM-gx-engine-503). What the engine
//! produces is **two terminal states**, not one reversal, and every assertion below is about that
//! shape.
//!
//! # Why the denial is an invariant and not a Cedar rule
//!
//! An undo agrees with the change it undoes on every attribute ASM-60-1's request carries — same
//! object, same locator, same substrate, order 0, invertible. The delta is what differs, and 41 §4
//! hands it to an `InvariantCheck`. See `support::DenyPayload`.

mod support;

use std::sync::Arc;

use gx_canon::cid;
use gx_core::{Timestamp, VerdictKind};
use gx_engine::{Engine, InverseStatus, Lifecycle};
use support::{
    gate, gate_refusing, intent, scratch, signing_key, CommitAdapter, MaybeEvidence, PERMIT_ALL,
};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const UNDO_AT: Timestamp = Timestamp(1_754_000_120_000_000_000);

/// The canonical digest of a transformation, through gx-canon and nothing else (41 §6).
///
/// The same function `plan` mints the `TransformationId` with, so "bit-equal" (sem:
/// SEM-gx-engine-504) in AC-040 (3) is
/// compared over the bytes 42 §2.1 defines rather than over a `PartialEq` this suite chose.
fn digest(t: &gx_core::Transformation) -> gx_core::Cid {
    cid::compute(t).expect("a transformation has a canonical form")
}

/// 🔴 AC-040 ①②③ in one run: the undo succeeds, `T_o` is superseded, and `T_o` is untouched.
#[test]
fn ac_040_an_undo_is_a_new_gated_transformation_that_supersedes_the_original() {
    let dir = scratch("ac040_success");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        MaybeEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    // --- T_o: "before" -> "after" ---------------------------------------------------------
    let i = intent("/tmp/undone.txt", "after");
    engine.submit(&i, 60, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    assert_eq!(
        engine.commit(&t_o, AT, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );
    let world_after = world.lock().expect("the world is not poisoned").clone();

    // ③'s baseline, taken **before** the undo: the three things that must not move.
    let record_before = digest(engine.transformation(&t_o).expect("the row holds it"));
    let receipt_before = engine.receipt(&t_o).expect("T-11 issued one").clone();
    let leaf_before = engine
        .ledger()
        .log()
        .entry(0)
        .expect("one leaf")
        .receipt_digest;
    let escrow_before = engine.inverse_status(&t_o);

    // --- T_u: the undo ----------------------------------------------------------------------
    let (undo_intent, t_u) = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 61, UNDO_AT)
        .expect("T-12's first half");
    assert_ne!(
        t_u, t_o,
        "43 §5: \"a normal Transformation starting from a new `submit(intent)`\" (sem: \
         SEM-gx-engine-505)"
    );

    // ① the normal road, one call at a time, with nothing skipped.
    let verified = engine
        .verify(&t_u, UNDO_AT, &signing_key(), None)
        .expect("T-4a");
    let canonicalized = engine.canonicalize(&t_u, UNDO_AT, None).expect("T-8");
    let committed = engine.commit(&t_u, UNDO_AT, &signing_key()).expect("T-11");

    let world_undone = world.lock().expect("the world is not poisoned").clone();
    println!(
        "AC040 t_o={t_o:?} t_u={t_u:?} undo_intent={undo_intent:?} verified={verified:?} \
         canonicalized={canonicalized:?} committed={committed:?} \
         state_o={:?} superseded_by={:?} escrow_before={escrow_before:?} escrow_after={:?} \
         applies={} leaves={} world_after={:?} world_undone={:?} parents={:?}",
        engine.state(&t_o),
        engine.superseded_by(&t_o),
        engine.inverse_status(&t_o),
        counts.totals()[4],
        engine.ledger().log().len(),
        String::from_utf8_lossy(&world_after),
        String::from_utf8_lossy(&world_undone),
        engine.transformation(&t_u).expect("the row").parents
    );

    // ① the undo was gated like anything else.
    assert_eq!(verified, Lifecycle::Admitted);
    assert_eq!(canonicalized, Lifecycle::Canonicalized);
    assert_eq!(committed, Lifecycle::Committed);
    assert_eq!(
        engine.verdict(&t_u),
        Some(VerdictKind::Admit),
        "\"undo is not exempted from verification either\" (sem: SEM-gx-engine-506) -- a verdict \
         exists because a gate was asked"
    );
    assert_eq!(
        engine.verdict_receipts(&t_u).len(),
        1,
        "and it was signed (ASM-14's first kind)"
    );
    assert_eq!(
        engine.transformation(&t_u).expect("the row").parents,
        vec![t_o],
        "43 T-12's guard: \"`T_u.parents` contains `T_o.id`\" (sem: SEM-gx-engine-507)"
    );
    assert_eq!(
        engine.transformation(&t_u).expect("the row").subject,
        engine.transformation(&t_o).expect("the row").subject,
        "\"`T_u`'s `Subject` matches `T_o`\" (sem: SEM-gx-engine-508)"
    );

    // ② the edge.
    assert_eq!(
        engine.state(&t_o),
        Some(Lifecycle::Superseded),
        "43 T-12: \"`T_o.status` transitions to `Superseded`\" (sem: SEM-gx-engine-509)"
    );
    assert_eq!(
        engine.superseded_by(&t_o),
        Some(t_u),
        "\"`superseded_by=T_u.id` is recorded\" (sem: SEM-gx-engine-510)"
    );
    assert_eq!(escrow_before, Some(InverseStatus::Available));
    assert_eq!(
        engine.inverse_status(&t_o),
        Some(InverseStatus::Consumed { by: t_u }),
        "M5-16, adopted (a): the status moves with the edge (sem: SEM-gx-engine-511)"
    );
    assert_eq!(engine.supersede_count(), 1);

    // ③ and nothing about `T_o` moved.
    assert_eq!(
        digest(engine.transformation(&t_o).expect("still there")),
        record_before,
        "43 §5-4: \"none of `T_o`'s canonical record, receipt, or ledger entry is ever rewritten\" \
         (sem: SEM-gx-engine-512)"
    );
    assert_eq!(engine.receipt(&t_o), Some(&receipt_before));
    assert_eq!(
        engine
            .ledger()
            .log()
            .entry(0)
            .expect("one leaf")
            .receipt_digest,
        leaf_before
    );

    // And the world really did go back, which is the only reason any of this is worth doing.
    assert_eq!(&*world_after, b"after");
    assert_eq!(&*world_undone, b"before");
    assert_eq!(
        counts.totals()[4],
        2,
        "two applies: the change and its inverse"
    );
    assert_eq!(engine.ledger().log().len(), 2, "two commits, two leaves");
}

/// 🔴 AC-040 ①'s second case: an undo the gate denies does **not** supersede anything.
///
/// > In a separate case where the invariant/policy corresponding to T_u is deliberately set to
/// > Deny, confirm T_u does not reach `Committed` and stays `Denied` -- undo is not exempted from
/// > gating either (sem: SEM-gx-engine-513)
///
/// The invariant refuses the payload the escrowed inverse carries ("before") and admits the one the
/// original carries ("after") (sem: SEM-gx-engine-513), so one engine, one policy set, two
/// outcomes — which is what makes
/// this a measurement of the *gating* rather than of two differently-configured runs.
#[test]
fn ac_040_an_undo_the_gate_denies_leaves_the_original_committed() {
    let dir = scratch("ac040_denied");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate_refusing(PERMIT_ALL, "no-rollback", "before"),
        MaybeEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/one-way.txt", "after");
    engine.submit(&i, 62, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    engine.commit(&t_o, AT, &signing_key()).expect("T-11");
    let record_before = digest(engine.transformation(&t_o).expect("the row"));
    let receipt_before = engine.receipt(&t_o).expect("one").clone();

    let (_, t_u) = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 63, UNDO_AT)
        .expect("the candidate is built");
    let verdict_state = engine
        .verify(&t_u, UNDO_AT, &signing_key(), None)
        .expect("T-4b");
    let canonicalize = engine
        .canonicalize(&t_u, UNDO_AT, None)
        .map_err(|e| e.kind().to_string());
    let commit = engine
        .commit(&t_u, UNDO_AT, &signing_key())
        .map_err(|e| e.kind().to_string());

    println!(
        "AC040_DENIED t_u_state={verdict_state:?} canonicalize={canonicalize:?} commit={commit:?} \
         state_o={:?} superseded_by={:?} escrow={:?} applies={} leaves={} world={:?}",
        engine.state(&t_o),
        engine.superseded_by(&t_o),
        engine.inverse_status(&t_o),
        counts.totals()[4],
        engine.ledger().log().len(),
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );

    assert_eq!(
        verdict_state,
        Lifecycle::Denied,
        "\"stays `Denied`\" (sem: SEM-gx-engine-514)"
    );
    assert_eq!(canonicalize.err().as_deref(), Some("InvalidState"));
    assert_eq!(commit.err().as_deref(), Some("InvalidState"));
    assert_eq!(
        engine.state(&t_o),
        Some(Lifecycle::Committed),
        "no `T_u` reached `Committed`, so 43 T-12 never fired"
    );
    assert_eq!(engine.superseded_by(&t_o), None);
    assert_eq!(
        engine.inverse_status(&t_o),
        Some(InverseStatus::Available),
        "the inverse is still there for a later, permitted undo"
    );
    assert_eq!(engine.supersede_count(), 0);
    assert_eq!(
        digest(engine.transformation(&t_o).expect("the row")),
        record_before
    );
    assert_eq!(engine.receipt(&t_o), Some(&receipt_before));
    assert_eq!(counts.totals()[4], 1, "only the original was applied");
    assert_eq!(engine.ledger().log().len(), 1);
    assert_eq!(&*world.lock().expect("the world is not poisoned"), b"after");
}

/// The three refusals `undo` makes before it builds anything.
///
/// A transformation that has not committed has no escrowed inverse to consume (43 T-10b runs inside
/// the critical section); one that has already been superseded has had it consumed (42 §3.12's
/// `Consumed`, which is what makes "only once" (sem: SEM-gx-engine-515) a fact); and an unknown
/// id names nothing. Each is a
/// different sentence and each gets its own refusal.
#[test]
fn ac_040_undo_refuses_what_it_cannot_undo() {
    let dir = scratch("ac040_refusals");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        MaybeEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/refusals.txt", "after");
    engine.submit(&i, 64, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");

    let uncommitted = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 65, UNDO_AT)
        .expect_err("a `Candidate` has escrowed nothing");
    let absent = gx_core::TransformationId(gx_core::Cid([3u8; 32]));
    let unknown = engine
        .undo(
            &absent,
            &engine.attested_postcondition(&absent),
            66,
            UNDO_AT,
        )
        .expect_err("nothing is filed under that id");

    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    engine.commit(&t_o, AT, &signing_key()).expect("T-11");
    let (_, t_u) = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 67, UNDO_AT)
        .expect("the first undo");
    engine
        .verify(&t_u, UNDO_AT, &signing_key(), None)
        .expect("T-4a");
    engine.canonicalize(&t_u, UNDO_AT, None).expect("T-8");
    engine.commit(&t_u, UNDO_AT, &signing_key()).expect("T-11");
    let twice = engine
        .undo(&t_o, &engine.attested_postcondition(&t_o), 68, UNDO_AT)
        .expect_err("its inverse has been consumed");

    println!(
        "AC040_REFUSALS uncommitted={:?} unknown={:?} twice={:?} escrow={:?} state_o={:?}",
        uncommitted.kind(),
        unknown.kind(),
        twice.kind(),
        engine.inverse_status(&t_o),
        engine.state(&t_o)
    );
    assert_eq!(uncommitted.kind(), "InvalidState");
    assert_eq!(unknown.kind(), "NotFound");
    assert_eq!(twice.kind(), "InvalidState");
    assert_eq!(
        engine.inverse_status(&t_o),
        Some(InverseStatus::Consumed { by: t_u })
    );
    assert_eq!(engine.state(&t_o), Some(Lifecycle::Superseded));
}
