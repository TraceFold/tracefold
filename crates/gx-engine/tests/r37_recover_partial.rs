// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R37 / `req/501` M-02** — the discriminating control for `finished_before_failure`.
//!
//! # Why this control exists here rather than on a CLI bed
//!
//! Audit 36 built `a36_control_the_counter_on_a_project_with_no_earlier_commit` to tell
//! *"the counter counts rows that closed before this process started"* from *"the counter is right
//! and this bed really closed one"*, and declared in `req/496` §7 item 3 that it **did not
//! discriminate**: with no floor, R7 / `req/232` M-01's laundering guard keeps `record_head` from
//! running at all, so the `Err` road is never reached and the bed answers `rc 0`.
//!
//! The bed that would discriminate at CLI level needs **two** `Committing` rows in one project —
//! one this run closes and one that then raises. That project is not constructible with the
//! instrument the beds use: the cut is a `set_len` truncation, a truncation removes a suffix, and a
//! suffix removal leaves exactly one row mid-flight. Removing a *middle* frame instead breaks
//! DR-43-9's chain, `journal_intact` answers `false`, and the whole recovery takes
//! [`RecoveryPath::NotResumed`] before any of this is reached — a different road, not a smaller
//! bed.
//!
//! So the discrimination is driven where the count is computed. Two directions, both needed:
//!
//! * a partial holding only rows that were terminal **before** this run counts **0** — the defect;
//! * a partial holding one such row **and** one this run closed counts **1** — which is what stops
//!   the repair from being `finished = 0`, the fix that would pass the first arm and say nothing.

use gx_core::{Cid, TransformationId};
use gx_engine::pipeline::{Lifecycle, RecoverPartial, Recovered, RecoveryPath};

fn id(byte: u8) -> TransformationId {
    TransformationId(Cid([byte; 32]))
}

fn row(byte: u8, path: RecoveryPath, state: Lifecycle) -> Recovered {
    Recovered {
        transformation: id(byte),
        path,
        state,
        ledger_seq: None,
        appended: None,
        payload_matched: None,
        receipt: None,
        refusal: None,
    }
}

#[test]
fn r37_the_counter_separates_rows_this_run_closed_from_rows_that_were_already_closed() {
    // 1. The shape audit 36 measured: one row that was `Committed` before the process started,
    //    pushed into `out` by `Engine::recover`'s outer loop as `RecoveryPath::Terminal`.
    let only_pre_existing = RecoverPartial {
        finished: vec![row(1, RecoveryPath::Terminal, Lifecycle::Committed)],
        ..RecoverPartial::default()
    };
    println!(
        "R37_COUNTER shape=only_pre_existing finished_len={} closed_by_this_run={}",
        only_pre_existing.finished.len(),
        only_pre_existing.closed_by_this_run()
    );
    assert_eq!(
        only_pre_existing.closed_by_this_run(),
        0,
        "🔴 req/496 M-02: a row that was terminal before this process started is not a row this \
         recovery finished"
    );

    // 2. The control: the same partial with a row this run actually closed. If the repair were
    //    `finished = 0` rather than a predicate, this arm is where it fails.
    let one_of_each = RecoverPartial {
        finished: vec![
            row(1, RecoveryPath::Terminal, Lifecycle::Committed),
            row(2, RecoveryPath::ClosedFromLedgerLeaf, Lifecycle::Committed),
        ],
        ..RecoverPartial::default()
    };
    println!(
        "R37_COUNTER shape=one_of_each finished_len={} closed_by_this_run={}",
        one_of_each.finished.len(),
        one_of_each.closed_by_this_run()
    );
    assert_eq!(
        one_of_each.closed_by_this_run(),
        1,
        "the control: a recovery that closed a row must say 1, or the repair has replaced a wrong \
         number with a constant"
    );

    // 3. And the roads that are neither. A refused row (`NothingWasApplied`) and a row whose apply
    //    was announced and then aborted both walked this run, and neither was **finished**.
    let neither = RecoverPartial {
        finished: vec![
            row(3, RecoveryPath::NothingWasApplied, Lifecycle::Committing),
            row(
                4,
                RecoveryPath::ApplyWasAnnounced,
                Lifecycle::Aborted(gx_core::AbortReason::ApplyFailed),
            ),
        ],
        ..RecoverPartial::default()
    };
    println!(
        "R37_COUNTER shape=walked_but_not_closed finished_len={} closed_by_this_run={}",
        neither.finished.len(),
        neither.closed_by_this_run()
    );
    assert_eq!(
        neither.closed_by_this_run(),
        0,
        "a row this run refused and a row whose apply failed are not rows it finished"
    );

    // 4. `finished` itself must keep every row, because it is what `announce_recovery` reads: the
    //    aborted `ApplyWasAnnounced` row above is exactly the one `req/449` H-02's sentence is
    //    about, and a repair that filtered the vector instead of the count would silence it.
    assert_eq!(
        neither.finished.len(),
        2,
        "🔴 the repair must narrow the **count**, not the vector: `announce_recovery` reads \
         `finished` and `req/449` H-02's sentence is owed to a row that is not `Committed`"
    );
}
