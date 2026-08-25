// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-048 (FR-043) — `Ok(None)` for a reason that exists, and the gate escalating on it.
//!
//! AC-048, verbatim: "Given: a delta whose inverse cannot be constructed (e.g. one whose old content has already been discarded by an overwrite). When:
//! `adapter.invert(delta, pre)` is called. Then: `Ok(None)`. Confirm via an integration test that when gx-gate receives `GateInput.invert_available=false`
//! it requires additional approval (escalating to Escalate, or firing an additional policy condition)."
//! Judgment method: "integration", M4. (sem: SEM-gx-adapter-fs-104)
//!
//! # The criterion's parenthetical is the one the errata replaced
//!
//! "already discarded by an overwrite" describes an adapter asked to invert **after** the change. **E-M4-30** (sem: SEM-gx-adapter-fs-105)
//! ruled that order out for every adapter without a history of its own -- the escrow is built before
//! `apply` (43 T-10b) -- so an fs adapter reached that way would never answer `Ok(None)`; it would
//! answer "the old content is right here". req/69 §4 M4-21 saw the same seam from the other side and
//! **M4-21, adopted (a)** supplied the reason that does exist: (sem: SEM-gx-adapter-fs-106)
//!
//! > "the ceiling on the inverse delta payload is declared **in exactly one constant**; exceeding it is
//! > `invert`=`Ok(None)` (**the 1st reason AC-048's `None` actually occurs**)" (sem: SEM-gx-adapter-fs-107)
//!
//! So the Given here is a file one byte over [`gx_adapter_fs::MAX_INVERSE_PAYLOAD_BYTES`]: 42 §5
//! requires the escrowed inverse to carry the body "because a digest-only inverse makes an actual undo physically impossible", (sem: SEM-gx-adapter-fs-108)
//! and a body over the ceiling is one this adapter declines to escrow. **The refusal is not a
//! refusal to act** -- it is a `false` on `GateInput.invert_available`, and **E-M3-4** makes that the
//! one condition in v0.1 that turns an otherwise admissible change into an `Escalate`.
//!
//! # Why the gate is here rather than the gate's own suite
//!
//! **M4-18, adopted (a)**: "AC-048's integration test goes in gx-adapter-fs's tests, with **gx-gate as a dev-dependency**
//! (E-M3-2 precedent, no cycle)". The gate half was already built and measured in M3 (sem: SEM-gx-adapter-fs-109)
//! (`crates/gx-gate/tests/verdict_meet.rs`, E-M3-4); what had never run is the **join** -- an adapter
//! producing the `Ok(None)` and a gate reading the flag that came from it. A dev-dependency adds no
//! edge to the shipped graph and no package to it either (gx-gate is a workspace member).
//!
//! # The control is what makes the escalation mean something
//!
//! An escalation test with no counterpart passes on a gate that escalates everything. So the same
//! change over a **small** file runs through the same policy set and the same `Gate`, and has to come
//! out `Admit`: the two runs differ in one input, and that input is the adapter's answer.

mod support;

use gx_adapter_fs::{FsAdapter, MAX_INVERSE_PAYLOAD_BYTES};
use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, Subject, Timestamp, Transformation, TransformationId, VerdictKind,
};
use gx_gate::{Gate, GateInput, PolicyEngine, ReasonSource, Verdict, INVERSE_UNAVAILABLE};
use gx_substrate::{PlannedDelta, SubstrateAdapter};
use support::{planned, snapshot_of, Sandbox, GOAL, SUBJECT};

/// Permits everything, so that the only thing deciding between `Admit` and `Escalate` below is the
/// flag the adapter produced. A pack that could refuse would make a green run ambiguous.
const PACK: &str = r#"@id("permit-default")
permit (principal, action, resource);
"#;

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// One change, order 0, by a human. The gate reads `t.order` and the composition metadata; none of
/// it varies between the two runs.
fn change(delta: &PlannedDelta) -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        0,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: delta.reference().clone(),
            context: ChangeContext::Substrate,
            actor: Actor::Human {
                key: "key-human-1".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 0 is under the ceiling")
}

/// A gate holding the permit-everything pack and no invariants.
fn gate() -> Gate {
    Gate::with_policies(PolicyEngine::parse(PACK).expect("the pack parses"))
}

/// Ask the gate about a change whose adapter answered `invert`, with the flag **E-M4-5** describes:
///
/// > "`invert_available` is **built by the engine folding Some/None, executing `invert(δ, pre)`
/// > at verify time against the same snapshot as the precondition**" (sem: SEM-gx-adapter-fs-110)
///
/// Which is what these three lines are: the engine's fold, written where the engine would write it.
fn verdict_for(adapter: &FsAdapter, delta: &PlannedDelta, pre: &ObjectSnapshot) -> (bool, Verdict) {
    let invert_available = adapter
        .invert(delta, pre)
        .expect("invert answers")
        // 🔴 **DR-46-26** — the engine's fold is `InvertOutcome::inverse`'s `is_some` now, and
        // this test is "the engine's fold, written where the engine would write it".
        .inverse()
        .is_some();
    let t = change(delta);
    let planned_bytes = PlannedDeltaBytes(delta.payload().to_vec());
    let verdict = gate()
        .verify(GateInput {
            t: &t,
            pre,
            planned: &planned_bytes,
            evidence: &[],
            invert_available,
            // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
            // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
            // varying this field alone moves no verdict) about the field and not about this fixture.
            decided_at: Timestamp(0),
        })
        .expect("a gate holding a policy set can answer");
    (invert_available, verdict)
}

/// The whole of AC-048: an inverse that will not fit, `Ok(None)`, and a gate that asks a human.
#[test]
fn an_inverse_over_the_escrow_ceiling_is_ok_none_and_the_gate_escalates() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator("over-the-ceiling");
    let given = vec![b'H'; MAX_INVERSE_PAYLOAD_BYTES + 1];
    sandbox.write("over-the-ceiling", &given);

    let pre = snapshot_of(&adapter, &locator);
    let delta = planned(&adapter, &locator, GOAL);

    let answer = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse();
    println!(
        "AC_048_GIVEN_BYTES={} CEILING={} INVERT={}",
        given.len(),
        MAX_INVERSE_PAYLOAD_BYTES,
        if answer.is_none() {
            "Ok(None)"
        } else {
            "Ok(Some)"
        }
    );
    assert!(
        answer.is_none(),
        "the inverse of a whole-file replacement carries the old content (42 §5), and this one is \
         one byte over the declared ceiling -- M4-21's 'the 1st reason it actually occurs' (sem: SEM-gx-adapter-fs-111)"
    );

    let (invert_available, verdict) = verdict_for(&adapter, &delta, &pre);
    assert!(!invert_available, "the fold of Ok(None) is false (E-M4-5)");
    assert_eq!(verdict.kind(), VerdictKind::Escalate);

    let Verdict::Escalate(ticket) = verdict else {
        unreachable!("the kind was checked above")
    };
    assert_eq!(ticket.reasons.len(), 1);
    assert_eq!(ticket.reasons[0].code(), INVERSE_UNAVAILABLE);
    assert!(
        matches!(ticket.reasons[0].source(), ReasonSource::Adapter { .. }),
        "the adapter is who answered `Ok(None)`, so it is the truthful source of the reason"
    );
    println!(
        "AC_048_VERDICT=Escalate REASON={} SOURCE=Adapter",
        ticket.reasons[0].code()
    );
}

/// The control: the same policy set, the same gate, a file the escrow can hold, and an `Admit`.
///
/// Without this the escalation above would be satisfied by a gate that escalates unconditionally,
/// which is the vacuity req/69 §8.2 asks "absence" assertions to be defended against. The two runs (sem: SEM-gx-adapter-fs-112)
/// differ in one input.
#[test]
fn the_same_change_over_a_file_the_escrow_can_hold_is_admitted() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let pre = snapshot_of(&adapter, &locator);
    let delta = planned(&adapter, &locator, GOAL);

    let (invert_available, verdict) = verdict_for(&adapter, &delta, &pre);
    println!(
        "AC_048_CONTROL_INVERT_AVAILABLE={invert_available} VERDICT={:?}",
        verdict.kind()
    );
    assert!(
        invert_available,
        "six bytes are well under the ceiling, so an inverse exists"
    );
    assert_eq!(
        verdict.kind(),
        VerdictKind::Admit,
        "nothing refused and an inverse exists, which is AC-027's Admit arm"
    );
}

/// The `Ok(None)` reaches the gate as a **flag**, and nothing else about the adapter does.
///
/// P-6 and **E-M4-11**: `GateInput.planned` is [`PlannedDeltaBytes`], the opaque carrier, so the gate
/// never learns this adapter's grammar. What crosses the boundary in AC-048 is one boolean, and this
/// states that as a fact about the input rather than as a hope about the implementation.
#[test]
fn the_gate_learns_one_boolean_and_no_grammar() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let delta = planned(&adapter, &locator, GOAL);

    let carrier = PlannedDeltaBytes(delta.payload().to_vec());
    assert_eq!(
        carrier.0,
        delta.payload(),
        "the carrier is the bytes, unread and unconverted (E-M3-1 / E-M4-11)"
    );

    let gate_source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate sits at <root>/crates/gx-adapter-fs")
            .join("crates/gx-gate/src/lib.rs"),
    )
    .expect("gx-gate's root is readable");
    for token in ["FsDelta", "FsOp", "gx_adapter_fs"] {
        assert!(
            !gate_source.contains(token),
            "the gate names {token:?}, which is the opacity L8 measures workspace-wide"
        );
    }
}
