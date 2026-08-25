// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-030 (FR-030) — two-stage identity: `IntentId` at submit, `TransformationId` at plan.
//!
//! 34 AC-030, verbatim (sem: SEM-gx-engine-403):
//!
//! > Given: intent I (`{"kind":"fs.write","path":"/tmp/x","content":"v1"}`). When: (1)
//! > `gx_engine::submit(I)` is called as a library function twice in the same process, and once
//! > from a completely independent second process. Then: all three calls return the same
//! > `IntentId`. When: (2) the same plan result for the same I (the same `PlannedDelta`/`target`)
//! > is passed to `gx_engine::plan(...)` the same way -- twice in the same process, once from a
//! > second process. Then: all three calls return the same `TransformationId`. Re-confirmation at
//! > the CLI level (`gx submit`/`gx plan`) happens in M6's E2E AC (AC-054).
//!
//! # What ASM-11 is, and what would break it quietly
//!
//! 42 §3.3 and FR-030 make identity two-stage because a `Transformation` does not know its own
//! delta until an adapter has planned one. The failure this criterion guards against is not "the
//! ids differ" -- that is loud -- but "the ids agree because something outside the value made them
//! agree": a counter, a memoised table, an allocator address, a hash seed. So the third call is in
//! **another process**, started with `env_clear()` in a fresh working directory, which is 34's own
//! AC-011 row read as a recipe ("a separate binary, no shared cache, a separate working directory";
//! sem: SEM-gx-engine-404).
//!
//! `tests/bin/engine_id_probe.rs` is that binary. It builds the intent from two strings rather than
//! deserialising one the parent sent, so the two processes agree on a value and not on a
//! serialisation of one.

mod support;

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence};
use support::{gate, intent, scratch, signing_key, StubAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// An engine with the stub adapter registered and a journal in `name`'s scratch directory.
fn engine(name: &str) -> Engine<InjectedEvidence> {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    engine
}

/// What the second process answered: `(IntentId, TransformationId)` as `gx1:` text.
///
/// The comparison is made on the **text** as well as on the parsed value, which is the shape
/// `gx-canon/tests/ac_011.rs` uses: two CIDs that are equal as values but print differently would
/// still be a defect, because 42 §1.2's text form is what crosses every boundary a user sees.
fn ids_from_another_process(locator: &str, goal: &str) -> (String, String) {
    let workdir = std::env::temp_dir().join(format!(
        "gx-engine-ac030-{}-{}",
        std::process::id(),
        locator.len()
    ));
    std::fs::create_dir_all(&workdir).expect("a working directory for the child");

    let mut child = Command::new(env!("CARGO_BIN_EXE_engine_id_probe"))
        .current_dir(&workdir)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the probe binary is built before the integration tests");

    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(format!("{{\"locator\": \"{locator}\", \"goal\": \"{goal}\"}}").as_bytes())
        .expect("the child takes its input");

    let out = child.wait_with_output().expect("the child finishes");
    assert!(
        out.status.success(),
        "the probe failed ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("the probe prints UTF-8");

    let mut intent_id = None;
    let mut transformation_id = None;
    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("INTENT_ID=") {
            intent_id = Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("TRANSFORMATION_ID=") {
            transformation_id = Some(v.to_string());
        }
    }
    let _ = std::fs::remove_dir_all(&workdir);
    (
        intent_id.unwrap_or_else(|| panic!("no INTENT_ID in {stdout:?}")),
        transformation_id.unwrap_or_else(|| panic!("no TRANSFORMATION_ID in {stdout:?}")),
    )
}

/// AC-030 (1) and (2): three calls each, two of them in this process and one in another.
#[test]
fn ac_030_the_same_intent_gets_the_same_two_ids_in_three_calls() {
    let i = intent("/tmp/x", "v1");

    let mut a = engine("ac030_a");
    let first = a.submit(&i, 42, AT).expect("submit");
    let second = a.submit(&i, 42, AT).expect("resubmit");
    let planned_first = a.plan(&i, AT).expect("plan");
    let planned_second = a.plan(&i, AT).expect("replan");

    // A different engine, a different journal, a different table -- everything except the value.
    let mut b = engine("ac030_b");
    let third = b.submit(&i, 42, AT).expect("submit in a second engine");
    let planned_third = b.plan(&i, AT).expect("plan in a second engine");

    let (child_intent, child_transformation) = ids_from_another_process("/tmp/x", "v1");

    println!(
        "AC030_INTENT_ID={} AC030_TRANSFORMATION_ID={}",
        child_intent, child_transformation
    );

    assert_eq!(first, second, "(1) the same intent, submitted twice");
    assert_eq!(first, third, "(1) the same intent, a second engine");
    assert_eq!(
        gx_canon::cid::to_text(&first.0),
        child_intent,
        "(1) the same intent, a completely independent process"
    );

    assert_eq!(
        planned_first, planned_second,
        "(2) the same plan result, planned twice"
    );
    assert_eq!(
        planned_first, planned_third,
        "(2) the same plan result, a second engine"
    );
    assert_eq!(
        gx_canon::cid::to_text(&planned_first.0),
        child_transformation,
        "(2) the same plan result, a completely independent process"
    );
}

/// 43 T-1's idempotency, measured as "no side effect" rather than as "the same value came back"
/// (sem: SEM-gx-engine-405).
///
/// Verbatim: "resubmitting an intent with the same canonical encoding returns the same
/// `IntentId` (**no side effect**, create-if-absent)" (sem: SEM-gx-engine-405). A `submit` that
/// returned the right id and appended a second `DraftCreated`
/// would satisfy the first half and break the second, and only the journal can tell them apart.
#[test]
fn ac_030_a_resubmitted_intent_writes_no_second_record() {
    let i = intent("/tmp/x", "v1");
    let mut e = engine("ac030_idempotent");

    assert_eq!(e.journal().len(), 0);
    e.submit(&i, 42, AT).expect("submit");
    assert_eq!(e.journal().len(), 1, "T-1 wrote one DraftCreated");
    e.submit(&i, 42, AT).expect("resubmit");
    e.submit(&i, 99, AT)
        .expect("resubmit with a different seed");
    assert_eq!(
        e.journal().len(),
        1,
        "create-if-absent: a resubmission is not an event, and a second seed does not make it one"
    );
}

/// Two intents that differ in one field get different ids, in both stages.
///
/// The other half of the criterion. An implementation that answered with a constant would pass
/// every assertion above, which is B-3's shape (a projection that names its fields and fills one
/// with a constant) applied to identity itself.
#[test]
fn ac_030_a_different_intent_gets_different_ids() {
    let mut e = engine("ac030_distinct");
    let base = intent("/tmp/x", "v1");
    let other_goal = intent("/tmp/x", "v2");
    let other_locator = intent("/tmp/y", "v1");

    let a = e.submit(&base, 42, AT).expect("submit");
    let b = e.submit(&other_goal, 42, AT).expect("submit");
    let c = e.submit(&other_locator, 42, AT).expect("submit");
    assert_ne!(a, b, "the goal is inside the IntentId (42 §1.3 row 2)");
    assert_ne!(a, c, "so is the locator");

    let pa = e.plan(&base, AT).expect("plan");
    let pb = e.plan(&other_goal, AT).expect("plan");
    let pc = e.plan(&other_locator, AT).expect("plan");
    assert_ne!(pa, pb, "a different delta is a different transformation");
    assert_ne!(pa, pc, "so is a different subject");
}

/// A `TransformationId` is the CID of the transformation it names.
///
/// ASM-11's "immutable thereafter" (sem: SEM-gx-engine-406) and 42 §1.3's exclusion of `id`
/// from the projection, together: recomputing
/// the CID of the stored value must give back the id it is stored under. This is what makes the
/// three-way agreement above a property of the *value* rather than of `plan`'s return path.
#[test]
fn ac_030_the_id_is_the_cid_of_the_value_it_names() {
    let i = intent("/tmp/x", "v1");
    let mut e = engine("ac030_selfid");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");

    let stored = e.transformation(&id).expect("the row is there");
    assert_eq!(stored.id, id);
    assert_eq!(
        gx_core::TransformationId(gx_canon::cid::compute(stored).expect("canonical")),
        id,
        "the id is a function of the value, so recomputing it returns it"
    );
}

/// `plan` refuses to rewind a transformation that has moved past `Candidate`.
///
/// 43 T-2 is "safe to retry" (sem: SEM-gx-engine-407), which the tests above rely on. It is not
/// "safe to replay over a verdict": a second `plan` on a verified transformation would replace the row with a fresh
/// `Candidate` and forget what the gate said. The guard is what keeps the retry from being a
/// rewind, and it refuses rather than silently doing nothing (req/29 §4).
#[test]
fn ac_030_replanning_a_transformation_that_has_moved_is_refused() {
    let i = intent("/tmp/x", "v1");
    let mut e = engine("ac030_rewind");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");
    e.verify(&id, AT, &signing_key(), None).expect("verify");

    let refused = e
        .plan(&i, AT)
        .expect_err("a verified candidate may not be replanned");
    assert_eq!(refused.kind(), "InvalidState", "{refused}");
}
