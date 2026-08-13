//! The laws the rulings added, run against the mock, and the split between the two sections shown
//! in one report.
//!
//! req/38 §28 逐語: 「**L1〜L8 の生存判定**: 全 8 本採用」, and §30 M4H2-4 added K1. L8 is not here --
//! it is not a property of an adapter and lives in `tests/opacity.rs`, which `src/laws.rs` says in
//! so many words so that the gap in the numbering is accounted for rather than noticed.

mod support;

use gx_substrate_conformance::{laws::LAW_IDS, run_all, run_laws, Fixture, Origin, Outcome};
use support::{BareFixture, MockFixture};

/// Every law holds for the mock, and each is labelled with the id its table row uses.
#[test]
fn the_mock_adapter_obeys_every_law() {
    let fixture = MockFixture::new();
    let report = run_laws(&fixture);
    report.print("mock (laws)");

    let failures: Vec<&gx_substrate_conformance::Check> = report
        .checks
        .iter()
        .filter(|c| c.outcome != Outcome::Pass)
        .collect();
    assert!(failures.is_empty(), "these laws did not hold: {failures:?}");

    let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, LAW_IDS.to_vec());
    assert!(report.checks.iter().all(|c| c.origin == Origin::Law));
    assert_eq!(Origin::Law.label(), "法則");
}

/// L3's Given is a state, and the check is quantified at it.
///
/// **E-M4-3** is the ruling and req/69 §3.2 the proof: reading 41 §4's idempotence and AC-049's round
/// trip as laws about a state map at once forces every delta to be the identity. So the round trip
/// is quantified at 「`invert` に渡した pre の 1 点」 and §28 made writing that state into the Given a
/// condition of the hand. This is that condition, measured: the state is disturbed **after** the
/// inverse was built, and the law is still about the state it was built for.
///
/// A harness that had quantified over 「whatever the substrate is now」 would fail here while the
/// adapter was correct, which is M3-05 one milestone later.
#[test]
fn l3_is_quantified_at_the_state_invert_was_handed() {
    let fixture = MockFixture::new();
    let adapter = fixture.adapter();
    let locator = fixture.locator();

    let pre = adapter.snapshot(&locator).expect("the mock has a subject");
    let given = adapter.precondition(&pre).expect("a scope");
    let delta = adapter
        .plan(&fixture.intent(), &pre)
        .expect("the mock can plan");
    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .expect("the subject is under the mock's ceiling");

    adapter.apply(&delta).expect("apply");
    adapter.apply(&inverse).expect("the inverse applies");

    let after = adapter
        .snapshot(&locator)
        .and_then(|s| adapter.precondition(&s))
        .expect("a scope");
    assert!(
        given.cas_eq(&after).expect("one scope, one adapter"),
        "the round trip did not return to the Given state {:?}",
        given.scope()
    );
}

/// A bare fixture leaves L5, L6 and L7 「無い」, and the run is not conformant.
#[test]
fn the_laws_that_need_a_subject_say_so_when_there_is_none() {
    let fixture = BareFixture::default();
    let report = run_laws(&fixture);
    report.print("mock (laws, bare fixture)");

    let missing: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::NotSupplied(_)))
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        missing,
        vec!["L5", "L6", "L7"],
        "the prophecy, the two commutation cases and the normalisation are what only an adapter's \
         author can supply"
    );
    assert_eq!(report.failed(), 0);
    // §31 M4H3-4 (b), implemented in M4 hand 4: 「失敗 0」 and 「未測定 0」 are two questions.
    assert!(report.is_conformant());
    assert!(!report.is_complete());
}

/// One run, both sections, and the report keeps them apart.
///
/// §30 M4H2-1 (a)'s whole purpose: 「1:1 対応の自己証明を汚さない」. Fifteen checks, of which exactly
/// seven are 51 §7's -- so the completion condition can still be read off the output.
#[test]
fn run_all_keeps_the_two_sections_distinguishable() {
    let fixture = MockFixture::new();
    let report = run_all(&fixture);
    report.print("mock (contracts and laws)");

    assert_eq!(report.checks.len(), 16);
    assert_eq!(report.of(Origin::Contract).len(), 7);
    assert_eq!(report.of(Origin::Law).len(), 9);
    assert!(report.is_conformant());
    assert!(
        report.is_complete(),
        "the full fixture supplies every subject, so 「未測定 0」 holds too (§31 M4H3-4 (b))"
    );

    // No id belongs to both sections, so a line of the report resolves to exactly one source.
    let contract_ids: Vec<&String> = report.of(Origin::Contract).iter().map(|c| &c.id).collect();
    for law in report.of(Origin::Law) {
        assert!(
            !contract_ids.contains(&&law.id),
            "`{}` names both a contract and a law",
            law.id
        );
    }
}
