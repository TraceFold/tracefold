// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-016 (FR-016) — an `Evidence` this crate produces is what `GateInput.evidence` takes.
//! (sem: SEM-gx-witness-206, SEM-gx-witness-207, SEM-gx-witness-208, SEM-gx-witness-209,
//! SEM-gx-witness-210, SEM-gx-witness-211, SEM-gx-witness-212, SEM-gx-witness-213)
//!
//! AC-016 verbatim: "Given: `GateInput { t, evidence: &[Evidence], invert_available }`. When: pass
//! an `Evidence` value gx-witness produced directly into `GateInput.evidence`. Then: the type check
//! passes (compiles successfully)." Judgement method: `unit (a type-match compile test)`, M2.
//!
//! # Two things in that sentence are not M2's, and one of them is an erratum
//!
//! **E-M2-4** (`req/38_ERRATA_2026-08-07.md` §8): "GateInput = 41 §4's 5 fields are correct
//! (AC-016's 3 items are the erratum)". 41 §4 declares `GateInput { t, pre, planned, evidence, invert_available }`; the AC
//! lists three of the five. The five are what this file checks, and it reads them **out of 41 §4**
//! rather than out of a comment here, so the erratum is machine-held: if the spec's struct ever
//! gains or loses a field, `ac_016_the_stand_in_has_exactly_the_fields_41_4_declares` goes red.
//!
//! **2026-08-21, E-DR4627-1 (DR-46-27): 41 §4 now declares a sixth,**
//! `GateInput { t, pre, planned, evidence, invert_available, decided_at }`. The paragraph above is
//! left as written because it records what M2 was handed; this note records what changed. Two
//! things follow and both are deliberate:
//!
//! * The stand-in below **keeps its five fields.** It is not `GateInput` and never was — AC-016's
//!   criterion is that a `gx_witness::Evidence` binds to the `evidence` slot with no conversion,
//!   and `decided_at` is a `Timestamp` that has nothing to do with that. Adding a sixth field here
//!   would be tracking a type this crate is forbidden to name (E-M3-2's cycle).
//! * The sentence about `ac_016_the_stand_in_has_exactly_the_fields_41_4_declares` describes a test
//!   that **is no longer in this file** — B-4 moved it to `crates/gx-gate/tests/gate_input_spec.rs`
//!   in M3 hand 4, as the section further down records. That is where 41 §4's field list is
//!   compared against the real type, and it is where the sixth field was accounted for. The
//!   verbatim quotations in this file are therefore **prose that can go stale, held by nothing**;
//!   `req/454` §3-1 names that as a known residual of DR-46-27 rather than pretending a machine
//!   watches it.
//!
//! **The type itself is M3.** 42 §0 files `GateInput` under gx-gate and `planned: &PlannedDelta`
//! under gx-substrate (M4), and req/49 §1 N-01/N-02 forbid building either here — 52's
//! "do not implement ahead across M milestones". So what this file holds is a **stand-in**: the same five fields in
//! the same order, with a local placeholder standing where `PlannedDelta` will be. req/49 §3 M2-5's
//! default proposal says exactly what that leaves as the subject — "M2 makes only 'that `Evidence`
//! can be passed as `&[Evidence]`' the type-check subject".
//!
//! What a compile-time criterion can and cannot show: that this file builds is the criterion. There
//! is no runtime assertion that could add anything, because a mismatched element type is a build
//! failure and never a failing test. The `assert`s below are reads of what went in, so that
//! `-D warnings` does not call the fields dead.
//!
//! # 2026-08-08, M3 hand 4: the spec comparison moved (**B-4**)
//!
//! Two tests stood here that read 41 §4 and 34 out of the spec files: one comparing the stand-in's
//! five fields with 41 §4's, one holding E-M2-4 (AC-016's row lists three of the five). H4-6 allowed
//! that as "confined to one place in ac_016, re-evaluate in M3", and req/38 §20's B-4 re-evaluated
//! it: "**move the 41 §4 markdown parse to gx-gate's side** ... a state where the real type is
//! never checked against the spec permits drift = H4-6's "confined to one place" was **a
//! confinement of place, not of subject** — the subject should be the real type".
//!
//! Both are now in `crates/gx-gate/tests/gate_input_spec.rs`, comparing 41 §4 with the **real**
//! `GateInput` rather than with the stand-in below. Neither was dropped ("the destination of the
//! two erratum checks is kept at the relocation site (not lost)"). What stays here is AC-016's own criterion, which is about this
//! crate's `Evidence` and could not be stated from gx-gate: the slot takes those values "as-is".

mod support;

use gx_core::{ObjectSnapshot, ReprKind, SubstrateKind, Transformation};
use gx_witness::evidence::Evidence;
use support::{cid, oid, one_of_each_evidence, submitted};

/// Standing in for gx-substrate's `PlannedDelta` (42 §3.4, M4).
///
/// The two fields are 42 §3.4's IdentityView (`substrate`, `payload`) and nothing else — enough to
/// occupy the slot 41 §4 gives it, and deliberately not enough to be mistaken for an early
/// implementation of the real type. N-02 forbids defining `PlannedDelta` in M2, and a struct named
/// something else, living in a test file, is the difference between a placeholder and a violation.
struct PlannedDeltaStandIn {
    substrate: SubstrateKind,
    payload: Vec<u8>,
}

/// 41 §4 verbatim, **as of M2**. `E-DR4627-1` (DR-46-27, 2026-08-21) appended a sixth field,
/// `decided_at: Timestamp`, which this stand-in deliberately does not carry — see the module doc.
///
/// ```text
/// pub struct GateInput<'a> {
///     pub t: &'a Transformation,
///     pub pre: &'a ObjectSnapshot,
///     pub planned: &'a PlannedDelta,
///     pub evidence: &'a [Evidence],
///     pub invert_available: bool,
///     pub decided_at: Timestamp,   // E-DR4627-1; not mirrored below
/// }
/// ```
///
/// Field for field, with `PlannedDelta` replaced. `evidence` is the one that matters: its element
/// type is `gx_witness::evidence::Evidence` with no conversion, no wrapper and no `Into` on the way
/// in, which is the whole of FR-016's "it can be confirmed by type check that
/// `GateInput.evidence: &[Evidence]`'s type is consistent".
///
/// It is still a stand-in and not the real thing: gx-gate names this crate (`GateInput.evidence`),
/// so a test here that imported `gx_gate::GateInput` would make the two crates name each other --
/// the cycle E-M3-2 exists to avoid. Whether 41 §4's names are still the type's is measured where
/// the real type lives (B-4, `crates/gx-gate/tests/gate_input_spec.rs`) — and since E-DR4627-1 that
/// list is six long while this stand-in stays five, on purpose: what AC-016 needs occupied is the
/// `evidence` slot, and a stand-in that chased every later field addition would be an
/// implementation of a type this crate may not name.
struct GateInputStandIn<'a> {
    t: &'a Transformation,
    pre: &'a ObjectSnapshot,
    planned: &'a PlannedDeltaStandIn,
    evidence: &'a [Evidence],
    invert_available: bool,
}

// ---------------------------------------------------------------------------
// The criterion: this file compiling is the test
// ---------------------------------------------------------------------------

/// AC-016 verbatim: gx-witness's own values are handed to the `evidence` slot "as-is".
#[test]
fn ac_016_evidence_from_this_crate_binds_to_the_gate_input_slice() {
    let t = submitted(1);
    let pre = ObjectSnapshot::new(
        oid(1),
        SubstrateKind::Fs,
        "workspace/a.txt".to_string(),
        cid(5),
        ReprKind::Bytes,
    );
    let planned = PlannedDeltaStandIn {
        substrate: SubstrateKind::Fs,
        payload: vec![0xde, 0xad],
    };
    let evidence: Vec<Evidence> = one_of_each_evidence();

    let input = GateInputStandIn {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &evidence,
        invert_available: true,
    };

    // Nothing above converted anything. These reads exist so `-D warnings` does not call the
    // fields dead and so a reader can see that what went in came back out.
    assert_eq!(input.t.id, t.id);
    assert_eq!(input.pre.locator(), "workspace/a.txt");
    assert_eq!(input.planned.substrate, SubstrateKind::Fs);
    assert_eq!(input.planned.payload, vec![0xde, 0xad]);
    assert!(input.invert_available);
    assert_eq!(input.evidence.len(), 4, "one value of each 42 §3.7 variant");
    assert_eq!(input.evidence, evidence.as_slice());
}

/// A borrowed slice of a `Vec` and a borrowed array literal are the same type here, which is what
/// "as-is" means in practice: a caller with either shape needs no adapter.
#[test]
fn ac_016_an_array_of_evidence_binds_to_the_same_slot() {
    let t = submitted(1);
    let pre = ObjectSnapshot::new(
        oid(1),
        SubstrateKind::Fs,
        "l".to_string(),
        cid(5),
        ReprKind::Bytes,
    );
    let planned = PlannedDeltaStandIn {
        substrate: SubstrateKind::Mcp,
        payload: Vec::new(),
    };
    let one = one_of_each_evidence();
    let borrowed: [Evidence; 1] = [one[0].clone()];

    let empty = GateInputStandIn {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: false,
    };
    let single = GateInputStandIn {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &borrowed,
        invert_available: false,
    };

    assert!(empty.evidence.is_empty());
    assert_eq!(single.evidence.len(), 1);
    assert!(!empty.invert_available && !single.invert_available);
    assert_eq!(empty.planned.substrate, SubstrateKind::Mcp);
    assert_eq!(single.t.id, t.id);
    assert_eq!(single.pre.locator(), "l");
}
