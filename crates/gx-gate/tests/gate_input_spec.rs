// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **B-4** — 41 §4's `GateInput` is compared against the real type, in the crate that holds it.
//!
//! # What moved, and why it moved here
//!
//! M2 had to name `GateInput`'s five fields before the type existed: AC-016 asks that a
//! gx-witness `Evidence` bind to `GateInput.evidence` "as-is" (sem: SEM-gx-gate-259), and 42 §0 files `GateInput`
//! under gx-gate (M3) while `PlannedDelta` is gx-substrate's (M4). So `gx-witness/tests/ac_016.rs`
//! built a **stand-in** with the same five fields and read 41 §4 out of the spec file to check the
//! list -- E-M2-4's erratum (AC-016's own row lists three of the five), held mechanically.
//!
//! H4-6 allowed that as "restricted to the one place, ac_016; re-evaluation is M3" (sem: SEM-gx-gate-260), and req/38 §20 re-evaluated it:
//!
//! > "**B-4 (loaded onto hand 4, in the direction of option b)**: **move 41 §4's markdown parse to
//! > the gx-gate side** (restructure ac_016 to keep AC-018's compile check + erratum check). Reason:
//! > a state where the real type is never checked against the spec permits drift = H4-6's
//! > 'restricted to one place' **is a restriction of location, not of target** -- the target should
//! > be the real type. The move is implemented in hand 4 (verdict.rs's hand, where `GateInput`
//! > becomes final). The 2 erratum checks keep their destination at the new location (nothing is
//! > lost)" (sem: SEM-gx-gate-261)
//!
//! Both erratum checks are below, against the type rather than against a stand-in of it. What
//! stayed in gx-witness is AC-016's own criterion -- that an `Evidence` value binds to the slice
//! slot with no conversion -- which is a statement about gx-witness's type and cannot be made from
//! here without gx-gate naming... itself. The stand-in stayed with it, because the compile that
//! *is* AC-016's criterion needs something to compile.
//!
//! # Why a source scan rather than a `match`
//!
//! A struct's field names are not available at run time, so the instrument is the source, as in
//! `ac_029.rs`'s third face and gx-log's `ac_069` call-site scan. The compile-time half is
//! [`the_five_fields_are_the_ones_the_type_takes`] below (the name records the count at the time of
//! B-4's move; **E-DR4627-1** made it six): an exhaustive struct literal fails to
//! build if a field is added, removed or renamed, so the two together say "41 §4's names" and
//! "the type takes exactly these". (sem: SEM-gx-gate-262)

use std::path::{Path, PathBuf};

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId,
};
use gx_gate::GateInput;
use gx_witness::Evidence;

/// The names, in the order 41 §4 writes them. Compared against the spec file and against the
/// type below rather than trusted.
///
/// **Five until 2026-08-21, six after (`E-DR4627-1`, DR-46-27).** The five that were here are the
/// first five below, untouched and in their original order; `decided_at` is appended, which is the
/// same shape the erratum takes in 41 §4 itself (the old declaration is not struck through, a sixth
/// line is added under it). The order matters: `fields_of` returns declaration order and
/// [`b_4_the_implementation_declares_the_fields_41_4_declares`] compares the two lists with
/// `assert_eq!`, so a spec that appended the field anywhere but last would fail here.
const DECLARED_FIELDS: [&str; 6] = [
    "t",
    "pre",
    "planned",
    "evidence",
    "invert_available",
    "decided_at",
];

fn workspace_root() -> PathBuf {
    // <root>/crates/gx-gate -> <root>
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

/// The field names of a `pub struct` block, read from a file that declares it.
///
/// One function for both readings -- the spec's markdown and the crate's source -- because the two
/// lists have to be compared and a second parser is a second answer to what a field is.
fn fields_of(text: &str, declaration: &str) -> Vec<String> {
    let mut lines = text.lines().skip_while(|l| !l.starts_with(declaration));
    assert!(
        lines.next().is_some(),
        "no line starts with `{declaration}` -- this test is reading the wrong file"
    );

    let mut fields = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('}') {
            return fields;
        }
        if let Some(rest) = trimmed.strip_prefix("pub ") {
            if let Some(name) = rest.split(':').next() {
                fields.push(name.trim().to_string());
            }
        }
    }
    panic!("the `{declaration}` block is not closed");
}

fn from_41() -> Vec<String> {
    let text = std::fs::read_to_string(
        workspace_root().join("req/spec/40-architecture/41-architecture.md"),
    )
    .expect("41 is readable");
    fields_of(&text, "pub struct GateInput")
}

fn from_source() -> Vec<String> {
    let text = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/lib.rs"))
        .expect("lib.rs is readable");
    fields_of(&text, "pub struct GateInput")
}

// ---------------------------------------------------------------------------
// The comparison B-4 asks for: the spec against the type
// ---------------------------------------------------------------------------

/// 41 §4's field names are the ones the implementation declares, in that order.
///
/// Five when B-4 moved here, six since **E-DR4627-1**. The assertion did not change shape: it is
/// still "whatever 41 §4 writes is whatever gx-gate writes", which is why a field addition had to be
/// written into the canon rather than filed as a canon-unchanged erratum the way E-M4-4 and
/// E-DR4626-1 were. Those two changed a *signature* nothing here parses; this one changes the list
/// this test *is*.
///
/// This is the assertion that moved. In gx-witness it compared the spec with a stand-in, which
/// could agree with 41 §4 while the real type drifted; here a drift on either side is a failure.
#[test]
fn b_4_the_implementation_declares_the_fields_41_4_declares() {
    let spec = from_41();
    let implemented = from_source();
    assert_eq!(
        implemented, spec,
        "41 §4's GateInput and gx-gate's no longer agree"
    );
    assert_eq!(spec, DECLARED_FIELDS.to_vec());
    assert_eq!(
        spec.len(),
        6,
        "E-M2-4: five fields, not three; E-DR4627-1: six, not five"
    );
}

/// The type takes exactly those, checked by the compiler rather than by a string.
///
/// An exhaustive struct literal is the instrument: adding a field makes this file fail to build
/// (missing field), removing or renaming one likewise. Together with the scan above, that is "the
/// names are 41 §4's" and "there are no others". (sem: SEM-gx-gate-263)
#[test]
fn the_five_fields_are_the_ones_the_type_takes() {
    // Renamed in spirit, not in fact: the name says "five" and the list is six since E-DR4627-1.
    // The name is kept because `req/64` §7's table records this test by name as one of the two
    // erratum checks B-4 preserved when it moved here, and a renamed test is a test that report can
    // no longer be checked against. What the name counts is the population at the time of the move.

    let t = transformation();
    let pre = snapshot();
    let planned = PlannedDeltaBytes(vec![0xde, 0xad]);
    let evidence: [Evidence; 0] = [];

    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &evidence,
        invert_available: true,
        // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
        // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
        // varying this field alone moves no verdict) about the field and not about this fixture.
        decided_at: Timestamp(0),
    };

    // Read back, so that `-D warnings` does not call any of them dead and so that the substitution
    // E-M3-1 made is visible: `planned` is `PlannedDeltaBytes`, the opaque carrier gx-core holds
    // until M4 gives it a meaning (42 §3.4's P-6).
    assert_eq!(input.t.id, t.id);
    assert_eq!(input.pre.locator(), "workspace/a.txt");
    assert_eq!(input.planned.0, vec![0xde, 0xad]);
    assert!(input.evidence.is_empty());
    assert!(input.invert_available);
    // E-DR4627-1's field, read back for the same reason as the other five: `-D warnings` would
    // otherwise call it dead, and a field nothing reads is a field nothing proves is there.
    assert_eq!(input.decided_at, Timestamp(0));
}

/// AC-016's row still lists three of the five, which is why **E-M2-4** exists.
///
/// The second of the two erratum checks B-4 required to survive the move. If some future edit made
/// 34 and 41 agree, this says so -- an erratum that has been fixed should stop being carried as one
/// (req/08 N-1's shape).
#[test]
fn e_m2_4_the_ac_lists_three_of_the_five() {
    let text =
        std::fs::read_to_string(workspace_root().join("req/spec/30-requirements/34-acceptance.md"))
            .expect("34 is readable");
    let row = text
        .lines()
        .find(|l| l.starts_with("| AC-016 "))
        .expect("34 still has an AC-016 row");

    let open = row
        .find("GateInput {")
        .expect("the row still writes a GateInput literal")
        + 11;
    let close = row[open..].find('}').expect("the literal is closed") + open;
    let listed: Vec<String> = row[open..close]
        .split(',')
        .filter_map(|f| f.split(':').next())
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();

    assert_eq!(
        listed,
        vec!["t", "evidence", "invert_available"],
        "AC-016's row no longer lists exactly the three fields E-M2-4 calls an erratum"
    );
    for missing in ["pre", "planned"] {
        assert!(
            !listed.iter().any(|f| f == missing),
            "AC-016 now names `{missing}`; E-M2-4 can be closed"
        );
    }
}

/// 41 §4's `InvariantCheck::check` still takes two arguments, and the implementation takes one.
///
/// **E-M3-7** is the erratum that made it so ("change the signature to
/// `InvariantCheck::check(&GateInput)` (a 41 §4 erratum)") (sem: SEM-gx-gate-264), and an erratum nobody measures is a sentence. The same shape as the AC-016 check
/// above: when the spec changes, this fails and the erratum can be closed.
#[test]
fn e_m3_7_the_spec_still_writes_the_two_argument_signature() {
    let text = std::fs::read_to_string(
        workspace_root().join("req/spec/40-architecture/41-architecture.md"),
    )
    .expect("41 is readable");
    assert!(
        text.contains("fn check(&self, pre: &ObjectSnapshot, planned: &PlannedDelta)"),
        "41 §4 no longer writes check(pre, planned); E-M3-7 can be closed"
    );

    let src = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/invariant.rs"))
        .expect("invariant.rs is readable");
    assert!(
        src.contains("fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult>;"),
        "the implemented signature is E-M3-7's"
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn transformation() -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        0,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: ChangeContext::Substrate,
            actor: Actor::Human {
                key: "key-human-1".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 0")
}

fn snapshot() -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        SubstrateKind::Fs,
        "workspace/a.txt".to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}
