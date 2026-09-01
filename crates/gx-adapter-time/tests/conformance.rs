// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The one `#[test]` an adapter crate calls the shared harness from (51 §7).
//!
//! 🔴 What passing means here, and what it does not. The seven contracts and the nine laws are
//! asked of **this fixture**: a schedule that is a directory under `CARGO_TARGET_TMPDIR`, with no
//! runner, in one thread. It is not a claim about cron, about systemd, about a scheduling service's
//! API, or about what happens when an entry actually fires -- that last one has no subject here at
//! all, because nothing in this crate can make an entry fire. `req/1038` §5 is where the bound is
//! written down; this file is where it is measured.

mod support;

use gx_substrate_conformance::{run_all, run_contracts, run_laws, Origin, Outcome};
use support::TimeFixture;

/// All seven of 51 §7's contracts hold, and none of them is "absent".
#[test]
fn the_time_adapter_meets_every_one_of_the_seven_contracts() {
    let fixture = TimeFixture::new();
    let report = run_contracts(&fixture);
    report.print("time (WM-4a)");

    let failures: Vec<&gx_substrate_conformance::Check> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Fail(_)))
        .collect();
    assert!(failures.is_empty(), "these contracts failed: {failures:?}");

    let unmeasured: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::NotSupplied(_)))
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        unmeasured.is_empty(),
        "51 §7 counts to seven and these have no subject: {unmeasured:?}"
    );
    assert_eq!(report.passed(), 7);
    assert!(report.is_conformant());
    assert!(report.is_complete());
}

/// All nine laws hold, L5 included -- which is the one this adapter fills from its first line
/// rather than from a later hand (`req/1038` §2, the `promised_target` seat).
#[test]
fn the_time_adapter_obeys_every_law_the_rulings_added() {
    let fixture = TimeFixture::new();
    let report = run_laws(&fixture);
    report.print("time (WM-4a, laws)");

    let failures: Vec<&gx_substrate_conformance::Check> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Fail(_)))
        .collect();
    assert!(failures.is_empty(), "these laws failed: {failures:?}");

    let measured: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| c.outcome == Outcome::Pass)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        measured,
        vec!["L1", "L2", "L3", "L4", "L5", "L6", "L7", "K1", "K2"],
        "the same nine the fs adapter answers; a new substrate does not get a shorter list"
    );
    assert!(report.is_conformant());
    assert!(report.is_complete());
}

/// One run, both sections, and the completion condition met by the sixth adapter of this workspace.
#[test]
fn one_run_reports_sixteen_obligations_and_meets_the_completion_condition() {
    let fixture = TimeFixture::new();
    let report = run_all(&fixture);
    report.print("time (WM-4a, contracts and laws)");

    assert_eq!(report.checks.len(), 16);
    assert_eq!(report.of(Origin::Contract).len(), 7);
    assert_eq!(report.of(Origin::Law).len(), 9);
    assert_eq!(report.failed(), 0);
    assert_eq!(report.not_supplied(), 0);
    assert_eq!(report.passed(), 16);
    assert!(report.meets_51_7());
    println!(
        "TIME_CONFORMANCE conformant={} complete={} meets_51_7={} passed={} unmeasured={}",
        report.is_conformant(),
        report.is_complete(),
        report.meets_51_7(),
        report.passed(),
        report.not_supplied()
    );
}
