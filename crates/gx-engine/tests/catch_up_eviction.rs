// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-43-6 (`req/38` §153) — the eviction rule, driven.**
//!
//! DR-43-2's design rests on one sentence, and R1's own self-kill named it as the load-bearing one:
//!
//! > **If a record read by `Engine::catch_up` names a transformation this process holds a row for,
//! > that row leaves the table.**
//!
//! `req/215` H-04 measured how much of that was true and found the rule had **no test at all** —
//! `grep -rn "catch_up\|CaughtUp\|evicted"` outside `src/` returned zero — and, worse, that it
//! looked unreachable from production: a `gx serve` cannot name a row a CLI planned and a CLI
//! cannot name a row the server planned, because both answer `404`/exit 6 for a transformation they
//! hold no body for. Its verdict was that "the rule is a clause, not a property".
//!
//! Two of those three findings are repaired here and the third is corrected. The rule **is**
//! reachable, and the road is the one 44 §1.2 already describes: `gx submit`, `gx plan`, `gx verify`
//! and `gx commit` are four processes, and `Engine::plan`'s rehydrating branch exists precisely so
//! that a second process can hold the body of a transformation the first one planned (see
//! `pipeline.rs`, "the same intent, planned again in another process"). Two engines over one `.gx/`
//! reach that state through the public surface with no fixture surgery, which is what these probes
//! do — and then one of them writes, and the other one's body has to go.
//!
//! What the rule protects is written down where the rule is, and is the reason a *stale body* is
//! worse than no body: an engine holding `Committed`/`Available` for a row another process has just
//! undone would offer a second door onto an inverse that was already consumed.
//!
//! The companion probe is `probes/doubt/tests/catch_up_scan.rs`, which holds the *other* half — that
//! the rule stays one rule — as a source scan, because R1's self-kill wrote its own falsifier
//! ("`pipeline.rs`'s `catch_up` body gaining a `match record` means this design is dead") and
//! `req/215` H-04 pointed out that a falsifier nobody runs is prose.

mod support;

use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent, scratch, signing_key, StubAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// Open one more engine over the same `.gx/`, with the same adapter registered.
///
/// Not a fixture trick: this is what a second `gx` process is. `Engine::open` replays the journal,
/// rebuilds the journal-derived indexes and leaves the in-flight table empty, which is exactly the
/// state a CLI verb starts in.
fn second_process(dir: &std::path::Path) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a second engine opens the same project");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    engine
}

/// 🔴 The rule itself: a record this process did not write, naming a row this process holds, drops
/// the body.
///
/// The order is the whole point. Both engines end up holding a body for one transformation — the
/// first by planning it, the second by re-planning the same intent (43 T-2's idempotency column,
/// "the same `PlannedDelta` and the same `TransformationId`"). Then the **first** one verifies,
/// which appends records naming that transformation, and the second one catches up.
///
/// After that the second engine has to be in the honest state and not a stale one: no body, the
/// state the log says, and the eviction *reported* rather than silent — `CaughtUp.evicted` is what
/// `gx serve` prints in its start-up line so that an operator whose `GET` started answering
/// `transformation: null` can find out why.
#[test]
fn a_record_another_process_appended_drops_the_row_this_process_holds() {
    let dir = scratch("catch_up_eviction_drops");
    let i = intent("/tmp/catch-up-eviction/object-a", "after");

    let mut first = second_process(&dir);
    first.submit(&i, 7, AT).expect("submit");
    let id = first.plan(&i, AT).expect("plan");
    assert!(
        first.transformation(&id).is_some(),
        "the process that planned it holds the body"
    );

    // A second process, opened after the first wrote, holding the same body through the road 44
    // §1.2's four-process flow already uses.
    let mut second = second_process(&dir);
    assert!(
        second.transformation(&id).is_none(),
        "`Engine::open` leaves the in-flight table empty (M5H3-5), so the body is not there yet"
    );
    let replanned = second.plan(&i, AT).expect("the same intent re-plans");
    assert_eq!(
        replanned, id,
        "43 T-2: re-planning the same intent against the same snapshot is the same id"
    );
    assert!(
        second.transformation(&id).is_some(),
        "the rehydrating branch is what puts a body in a second process's table"
    );
    println!("EVICT_SETUP both_hold_the_body=true id={}", id.0.to_text());

    // The first process writes. Nothing about this is unusual: it is `gx verify <TID>`.
    let key = signing_key();
    first
        .verify(&id, AT, &key, None)
        .expect("the first process verifies the row it planned");

    let caught = second.catch_up().expect("the second process catches up");
    println!(
        "EVICT_CAUGHT records={} evicted={:?}",
        caught.records,
        caught
            .evicted
            .iter()
            .map(|t| t.0.to_text())
            .collect::<Vec<_>>()
    );
    assert!(
        caught.records > 0,
        "the verify appended records the second process had not read"
    );
    assert_eq!(
        caught.evicted,
        vec![id],
        "the row another process named is the row that leaves, and it is reported by name"
    );
    assert!(
        second.transformation(&id).is_none(),
        "the body is gone: a stale body is what would authorise a second effect"
    );
    assert_eq!(
        second.state(&id),
        Some(Lifecycle::Admitted),
        "and the Σ-shadow answers the state the log records, which is the first process's, not the \
         Candidate this process was holding (the permit-all gate admits)"
    );
}

/// 🔴 The row does not leave half of itself behind, and the answer that survives is the shadow's.
///
/// The eviction rule says the row "leaves the table" and `Engine::catch_up` removes it from three
/// places (the table, the subject index and the escrow index) — this is the probe that says all
/// three, because an index entry pointing at a row that is gone is the failure mode an index exists
/// to create. `conflicting_predecessor` reads the subject index, so a stale entry there would make a
/// later plan refuse against a body nobody holds.
///
/// The second half is what makes eviction affordable at all: dropping the body is not dropping the
/// row. `state`, `verdict`, `enforced` and the rest fall through to the Σ-shadow, which is what T6-①
/// promises app layers, so what an evicted row costs is precisely the body and nothing else.
#[test]
fn eviction_clears_the_indexes_and_leaves_the_shadow_answering() {
    let dir = scratch("catch_up_eviction_indexes");
    let i = intent("/tmp/catch-up-eviction/object-b", "after");

    let mut first = second_process(&dir);
    first.submit(&i, 11, AT).expect("submit");
    let id = first.plan(&i, AT).expect("plan");
    let subject = first
        .transformation(&id)
        .expect("the planner holds the body")
        .subject;

    let mut second = second_process(&dir);
    second.plan(&i, AT).expect("the same intent re-plans");
    assert_eq!(
        second.transformations_on(&subject),
        vec![id],
        "the subject index is populated in the second process too"
    );

    let key = signing_key();
    first.verify(&id, AT, &key, None).expect("verify");

    let caught = second.catch_up().expect("catch up");
    assert_eq!(caught.evicted, vec![id]);
    assert!(
        second.transformations_on(&subject).is_empty(),
        "the subject index lets go of a row the table let go of: an index entry naming a body \
         nobody holds is a plan refused against nothing"
    );
    assert!(
        !second.transformation_ids().contains(&id),
        "and the body-holding accessor does not name it either"
    );

    // What survives is the shadow, which is the whole reason one rule is enough.
    assert!(
        second.shadow().row(&id).is_some(),
        "T6-①: the journal's row is still readable, bodiless"
    );
    assert_eq!(second.state(&id), Some(Lifecycle::Admitted));
    println!(
        "EVICT_INDEXES subject_rows={} shadow_rows={}",
        second.transformations_on(&subject).len(),
        second.shadow().len()
    );
}
