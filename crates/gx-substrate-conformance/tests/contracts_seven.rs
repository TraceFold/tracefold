// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 51 §7's seven contracts, run against the mock -- and run again against a fixture that supplies
//! nothing, so that "NOT_SUPPLIED" is shown to be a distinct answer. (sem:
//! SEM-gx-substrate-conformance-061, SEM-gx-substrate-conformance-062, SEM-gx-substrate-conformance-063,
//! SEM-gx-substrate-conformance-064, SEM-gx-substrate-conformance-065, SEM-gx-substrate-conformance-066,
//! SEM-gx-substrate-conformance-067, SEM-gx-substrate-conformance-068)
//!
//! # What passing here does and does not mean
//!
//! It means the harness can decide all seven, and that an adapter satisfying them exists. It does
//! **not** mean any real adapter passes: `gx-adapter-fs` is hand 4 and `git`/`mcp` are M7. What hand
//! 3 owes 51 §7 is the instrument, and an instrument nobody has pointed at a subject is not an
//! instrument -- which is why the mock is here and why the second fixture is here beside it.

mod support;

use gx_substrate_conformance::{contracts::CONTRACT_IDS, run_contracts, Origin, Outcome};
use support::{BareFixture, MockFixture};

/// All seven hold for an adapter that meets them, and the report says so with no "NOT_SUPPLIED".
#[test]
fn the_mock_adapter_meets_all_seven_contracts() {
    let fixture = MockFixture::new();
    let report = run_contracts(&fixture);
    report.print("mock (full fixture)");

    let failures: Vec<&gx_substrate_conformance::Check> = report
        .checks
        .iter()
        .filter(|c| c.outcome != Outcome::Pass)
        .collect();
    assert!(
        failures.is_empty(),
        "51 §7, verbatim: \"no adapter satisfies the M4/M7 completion condition unless it passes \
         all seven of the above contracts\"; these did \
         not: {failures:?}"
    );
    assert_eq!(report.checks.len(), 7);
    assert!(report.is_conformant());
}

/// The report labels every check with 51 §7's own cell, and with the contract origin.
///
/// §30 M4H2-1 (a): "distinguish the origin (contract or law) in print (so as not to muddy the 1:1
/// correspondence's self-proof)". A reader of the
/// output matches a line against the canon by its name, so the names have to be the canon's.
#[test]
fn every_contract_check_is_labelled_with_51_7s_own_name() {
    let fixture = MockFixture::new();
    let report = run_contracts(&fixture);

    let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        CONTRACT_IDS.to_vec(),
        "the report's labels are not 51 §7's seven cells, in order"
    );
    assert!(
        report.checks.iter().all(|c| c.origin == Origin::Contract),
        "a law is filed among the contracts, which is what §30 M4H2-1 (a) split the two sections to \
         prevent"
    );
    assert_eq!(Origin::Contract.label(), "contract");
}

/// A fixture that supplies no optional subject leaves two contracts "NOT_SUPPLIED", and is not
/// conformant.
///
/// The whole of "an unmatched contract is printed as `NOT_SUPPLIED`; silently treating it as bounded
/// is forbidden". Contract 5 needs a delta whose inverse
/// cannot be built and contract 6 needs both commutation cases; nothing adapter-independent can
/// invent either. A harness that quietly skipped them would report five passes as seven, and the
/// M4/M7 completion condition would be quoting a number that means something else.
#[test]
fn a_fixture_that_supplies_nothing_is_told_so_rather_than_passed() {
    let fixture = BareFixture::default();
    let report = run_contracts(&fixture);
    report.print("mock (bare fixture)");

    let missing: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::NotSupplied(_)))
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        missing,
        vec!["invertのNone契約", "commutation"],
        "these are the two contracts whose subject only an adapter's author can produce"
    );
    assert_eq!(report.failed(), 0, "nothing here is wrong, only unmeasured");
    // **§31 M4H3-4 (b)**, implemented in M4 hand 4: "separate `is_conformant` (zero failures) and
    // `is_complete` (zero unmeasured). The overall judgement = both true". Nothing here contradicted
    // 51 §7, and four of the seven were not measured -- two different facts, which hand 3 could only
    // report as one. "NOT_SUPPLIED" is still not a
    // pass: it is `is_complete` that says so now, and the completion condition needs both.
    assert!(
        report.is_conformant(),
        "nothing this run could measure failed, so the adapter is not the thing at fault"
    );
    assert!(
        !report.is_complete(),
        "\"NOT_SUPPLIED\" counted as measured (sem: SEM-gx-substrate-conformance-147): an \
         unmeasured contract cannot be reported as met \
         (req/29 §4: a \
         skip and a pass must not look alike)"
    );
    assert_eq!(report.passed() + report.not_supplied(), report.checks.len());
}
