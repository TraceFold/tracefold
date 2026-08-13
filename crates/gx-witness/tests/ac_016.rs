//! AC-016 (FR-016) — an `Evidence` this crate produces is what `GateInput.evidence` takes.
//!
//! AC-016 逐語: 「Given: `GateInput { t, evidence: &[Evidence], invert_available }`。When:
//! gx-witnessが生成した`Evidence`値をそのまま`GateInput.evidence`へ渡す。Then: 型検査が通過する
//! （コンパイル成功）。」判定方法 `unit（型一致コンパイルテスト）`, M2.
//!
//! # Two things in that sentence are not M2's, and one of them is an erratum
//!
//! **E-M2-4** (`req/38_ERRATA_2026-08-07.md` §8): 「GateInput=41 §4 の 5 field が正(AC-016 の 3 項は
//! erratum)」. 41 §4 declares `GateInput { t, pre, planned, evidence, invert_available }`; the AC
//! lists three of the five. The five are what this file checks, and it reads them **out of 41 §4**
//! rather than out of a comment here, so the erratum is machine-held: if the spec's struct ever
//! gains or loses a field, `ac_016_the_stand_in_has_exactly_the_fields_41_4_declares` goes red.
//!
//! **The type itself is M3.** 42 §0 files `GateInput` under gx-gate and `planned: &PlannedDelta`
//! under gx-substrate (M4), and req/49 §1 N-01/N-02 forbid building either here — 52's
//! 「M を跨ぐ先行実装はしない」. So what this file holds is a **stand-in**: the same five fields in
//! the same order, with a local placeholder standing where `PlannedDelta` will be. req/49 §3 M2-5's
//! 既定案 says exactly what that leaves as the subject — 「M2 は『`Evidence` が `&[Evidence]` として
//! 渡せる』だけを型検査対象にする」.
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
//! that as 「ac_016 の 1 箇所に限定・再評価は M3」, and req/38 §20's B-4 re-evaluated it: 「41 §4 の
//! markdown parse を **gx-gate 側へ移す** … 実型が spec と突き合わされない状態は drift を許す=H4-6 の
//! 「1 箇所限定」は**場所の限定であって対象の限定ではない**——対象は実型であるべき」.
//!
//! Both are now in `crates/gx-gate/tests/gate_input_spec.rs`, comparing 41 §4 with the **real**
//! `GateInput` rather than with the stand-in below. Neither was dropped (「erratum 検査 2 本の行き場は
//! 移設先で保持(失わない)」). What stays here is AC-016's own criterion, which is about this
//! crate's `Evidence` and could not be stated from gx-gate: the slot takes those values 「そのまま」.

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

/// 41 §4 逐語:
///
/// ```text
/// pub struct GateInput<'a> {
///     pub t: &'a Transformation,
///     pub pre: &'a ObjectSnapshot,
///     pub planned: &'a PlannedDelta,
///     pub evidence: &'a [Evidence],
///     pub invert_available: bool,
/// }
/// ```
///
/// Field for field, with `PlannedDelta` replaced. `evidence` is the one that matters: its element
/// type is `gx_witness::evidence::Evidence` with no conversion, no wrapper and no `Into` on the way
/// in, which is the whole of FR-016's 「`GateInput.evidence: &[Evidence]`の型と整合することを型検査で
/// 確認できる」.
///
/// It is still a stand-in and not the real thing: gx-gate names this crate (`GateInput.evidence`),
/// so a test here that imported `gx_gate::GateInput` would make the two crates name each other --
/// the cycle E-M3-2 exists to avoid. Whether these five names are still 41 §4's is measured where
/// the real type lives (B-4, `crates/gx-gate/tests/gate_input_spec.rs`).
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

/// AC-016 verbatim: gx-witness's own values are handed to the `evidence` slot 「そのまま」.
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
/// 「そのまま」 means in practice: a caller with either shape needs no adapter.
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
