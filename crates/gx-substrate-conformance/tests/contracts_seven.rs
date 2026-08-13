//! 51 §7's seven contracts, run against the mock -- and run again against a fixture that supplies
//! nothing, so that 「無い」 is shown to be a distinct answer.
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

/// All seven hold for an adapter that meets them, and the report says so with no 「無い」.
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
        "51 §7 逐語: 「各adapterは上記7契約すべてに合格しない限りM4/M7完了条件を満たさない」; these did \
         not: {failures:?}"
    );
    assert_eq!(report.checks.len(), 7);
    assert!(report.is_conformant());
}

/// The report labels every check with 51 §7's own cell, and with the 契約 origin.
///
/// §30 M4H2-1 (a): 「出自(契約 or 法則)を印字で区別(1:1 対応の自己証明を汚さない)」. A reader of the
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
    assert_eq!(Origin::Contract.label(), "契約");
}

/// A fixture that supplies no optional subject leaves two contracts 「無い」, and is not conformant.
///
/// The whole of 「対応の無い契約は「無い」と印字=黙って有界禁」. Contract 5 needs a delta whose inverse
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
    // **§31 M4H3-4 (b)**, implemented in M4 hand 4: 「`is_conformant`(失敗 0)と `is_complete`(未測定 0)
    // を分離。総合判定=両方 true」. Nothing here contradicted 51 §7, and four of the seven were not
    // measured -- two different facts, which hand 3 could only report as one. 「無い」 is still not a
    // pass: it is `is_complete` that says so now, and the completion condition needs both.
    assert!(
        report.is_conformant(),
        "nothing this run could measure failed, so the adapter is not the thing at fault"
    );
    assert!(
        !report.is_complete(),
        "「無い」 counted as measured: an unmeasured contract cannot be reported as met (req/29 §4: a \
         skip and a pass must not look alike)"
    );
    assert_eq!(report.passed() + report.not_supplied(), report.checks.len());
}
