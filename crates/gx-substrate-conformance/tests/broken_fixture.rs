// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 The negative control this harness never had: an adapter that **fails**. (sem:
//! SEM-gx-substrate-conformance-090, SEM-gx-substrate-conformance-091, SEM-gx-substrate-conformance-092,
//! SEM-gx-substrate-conformance-093, SEM-gx-substrate-conformance-094, SEM-gx-substrate-conformance-095,
//! SEM-gx-substrate-conformance-096, SEM-gx-substrate-conformance-097, SEM-gx-substrate-conformance-098,
//! SEM-gx-substrate-conformance-099, SEM-gx-substrate-conformance-100, SEM-gx-substrate-conformance-101,
//! SEM-gx-substrate-conformance-102, SEM-gx-substrate-conformance-103, SEM-gx-substrate-conformance-104,
//! SEM-gx-substrate-conformance-105, SEM-gx-substrate-conformance-106, SEM-gx-substrate-conformance-107)
//!
//! req/38 §35 K-3, adopted (blocked), verbatim: "**recognized as a tag-blocker**. In the fix batch,
//! **one deliberately contract-breaking fixture** (passing through the three states
//! `is_conformant=false` / `is_complete=false` / L5 returns a fail) -> **print
//! `gx-substrate-conformance` coverage's recovery to ≥80 as a number**", and K-10, adopted (a):
//! "piggyback on K-3's one fixture (observe the `Fixture` trait's default implementations over the
//! same subject)".
//!
//! # Why an instrument needs a subject that fails
//!
//! Until this file, every subject the harness had ever been pointed at passed. req/76 measured what
//! that costs from two directions and both said the same thing:
//!
//! * **coverage 69.51%** (51 §14 asks ≥80), with the missing lines concentrated in `contracts.rs`
//!   63.43 and `laws.rs` 67.80 -- "the failing side" and "the not-supplied side" branches never ran;
//! * **7 of 15 `cargo mutants` survivors in the judgement functions themselves** --
//!   `is_conformant → true`, `meets_51_7 → true`, `Report::failed → 0`, and `law_5`'s
//!   `resulting_digest == target` guard → `true`.
//!
//! The second is the sharper statement. 51 §7's completion condition -- "no adapter satisfies the
//! M4/M7 completion condition unless it passes all seven of the above contracts" -- is read off
//! `meets_51_7`, and a `meets_51_7` that
//! returned `true` unconditionally would have been reported as correct by the suite that exists to
//! check it. **An instrument that has only ever been shown one answer is not known to have two.**
//!
//! # One fixture, eighteen lies
//!
//! [`Flaw`] is one broken obligation each, and [`BrokenFixture`] is the single fixture asked in turn
//! to tell each lie. The point is not eighteen fixtures -- it is one negative control per obligation,
//! because a harness that can report "failed" about contract 3 and not about L6 is a harness with a
//! hole in exactly the place nobody looked.
//!
//! # "failed" and "NOT_SUPPLIED" stay two answers
//!
//! [`Flaw::NotImplemented`] is here to keep §31 M4H3-4 (b) honest in the negative direction too: an
//! adapter that answers [`gx_substrate::Error::Unimplemented`] is **conformant and incomplete**, and
//! this file asserts that this one flaw -- alone among eighteen -- produces no failure at all. If
//! that ever changes, every partially built M7 adapter starts looking wrong instead of unfinished.

mod support;

use gx_substrate_conformance::{run_all, Fixture, Outcome};
use support::{BareFixture, BrokenFixture, Flaw, MockFixture, FLAWS};

/// Every deliberate flaw is reported, and none of them passes.
///
/// The table this prints is the evidence: one row per obligation broken, with what the harness said
/// about it. A flaw whose row shows `FAIL=0 NOT_SUPPLIED=0` would be a lie the harness cannot see.
#[test]
fn every_deliberate_flaw_is_reported_rather_than_passed() {
    let mut rows: Vec<String> = Vec::new();
    let mut any_failed = false;
    let mut any_incomplete = false;

    for flaw in FLAWS {
        let fixture = BrokenFixture::new(flaw);
        let report = run_all(&fixture);
        report.print(&format!("broken ({flaw:?})"));

        assert!(
            !report.meets_51_7(),
            "{flaw:?}: the harness reports 51 §7's completion condition as met for an adapter that \
             breaks one of its obligations on purpose"
        );
        any_failed |= report.failed() > 0;
        any_incomplete |= !report.is_complete();
        rows.push(format!(
            "{flaw:?}: FAIL={} NOT_SUPPLIED={} PASS={} conformant={} complete={}",
            report.failed(),
            report.not_supplied(),
            report.passed(),
            report.is_conformant(),
            report.is_complete()
        ));
    }

    println!("BROKEN_FIXTURE_FLAWS={}", FLAWS.len());
    for row in &rows {
        println!("  {row}");
    }
    // The three states K-3 asks the fixture to pass through. `is_conformant=false` and
    // `is_complete=false` are asserted here over the set; L5's own failure has a probe below.
    assert!(
        any_failed,
        "not one of the eighteen flaws produced a failure: `Report::failed` is not counting"
    );
    assert!(
        any_incomplete,
        "not one of the eighteen flaws produced an unmeasured obligation"
    );
}

/// Each flaw is reported against the obligation it breaks, and not merely somewhere.
///
/// §30 M4H2-6's rule one layer up: a harness that answered "something failed" would pass the probe
/// above while telling an adapter's author nothing. The pairs below are the check each lie is aimed
/// at, so a report that blames the wrong obligation is RED.
#[test]
fn each_flaw_is_reported_against_the_obligation_it_breaks() {
    // Three of the ten right-hand strings are kept in Japanese (sem: SEM-gx-substrate-conformance-060):
    // they are `contracts::CONTRACT_IDS` values, which stay Japanese for the reason SEM-006/007 give.
    let aimed_at: [(Flaw, &str); 10] = [
        (Flaw::AnswersAboutAnotherLocator, "snapshot"),
        (Flaw::PlansDifferentlyEachTime, "plan"),
        (Flaw::Deaf, "precondition"),
        (Flaw::UndoesNothing, "apply/invert往復"),
        (Flaw::InventsAnInverse, "invertのNone契約"),
        (Flaw::AlwaysCommutes, "commutation"),
        (Flaw::NeverCommutes, "commutation"),
        (Flaw::AppliesTwiceDifferently, "apply冪等性"),
        (Flaw::BreaksThePromise, "L5"),
        (Flaw::CallsEquivalentWhatIsNot, "L7"),
    ];

    for (flaw, id) in aimed_at {
        let report = run_all(&BrokenFixture::new(flaw));
        let check = report
            .checks
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("{flaw:?}: no check is named {id:?}"));
        println!(
            "AIMED {flaw:?} -> [{}] {id} {:?}",
            check.origin.label(),
            check.outcome
        );
        assert!(
            matches!(check.outcome, Outcome::Fail(_)),
            "{flaw:?} breaks {id} and the harness reported {:?} about it",
            check.outcome
        );
    }
}

/// **L5**: the prophecy is compared, and the comparison is what fails.
///
/// The survivor req/76 §2.2 names is `laws.rs:307`'s guard -- `applied.resulting_digest() == &target`
/// replaced by `true`, which makes "adapter self-consistency" (M4-06, adopted (b)) hold for every
/// adapter that
/// returns anything at all. This is the subject that tells the two apart: the substrate reaches the
/// promised state and the adapter **reports** a different digest, so only a harness that reads the
/// report rather than the world can see it.
#[test]
fn a_promise_the_adapter_does_not_keep_is_an_l5_failure() {
    let report = run_all(&BrokenFixture::new(Flaw::BreaksThePromise));
    let l5 = report
        .checks
        .iter()
        .find(|c| c.id == "L5")
        .expect("L5 is in the law table");
    let Outcome::Fail(why) = &l5.outcome else {
        panic!(
            "L5 answered {:?} for an adapter that reported a digest it did not reach",
            l5.outcome
        )
    };
    println!("L5_FAILURE={why}");
    assert!(
        why.contains("promised") && why.contains("observed"),
        "the failure does not say which two values disagreed, so an adapter's author cannot act on \
         it: {why}"
    );
    assert!(
        !report.is_conformant(),
        "a failed law leaves the run non-conformant"
    );
}

/// "not yet" is not "failed", measured from the failing side (**§31 M4H3-4 (b)**).
///
/// The one flaw of the eighteen that is not a defect. `Error::Unimplemented` is §32 M4H4-2's
/// permanent vocabulary -- ""unimplemented" and "failure" are permanently separate facts, and M7's
/// git/mcp also start up partially implemented" -- and this asserts the harness keeps the
/// distinction when the adapter is wrong about
/// nothing and silent about something.
#[test]
fn an_adapter_that_is_only_half_built_is_incomplete_rather_than_wrong() {
    let report = run_all(&BrokenFixture::new(Flaw::NotImplemented));
    report.print("broken (NotImplemented)");

    let unmeasured: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::NotSupplied(_)))
        .map(|c| c.id.as_str())
        .collect();
    println!(
        "HALF_BUILT FAIL={} NOT_SUPPLIED={unmeasured:?} conformant={} complete={} meets={}",
        report.failed(),
        report.is_conformant(),
        report.is_complete(),
        report.meets_51_7()
    );
    assert_eq!(
        report.failed(),
        0,
        "an adapter that says \"not yet\" is being reported as wrong, which would make every \
         partially built M7 adapter look like a defect"
    );
    assert!(!unmeasured.is_empty(), "nothing was reported as unmeasured");
    assert!(report.is_conformant());
    assert!(!report.is_complete());
    assert!(
        !report.meets_51_7(),
        "NOT_SUPPLIED counted toward \"all seven contracts pass\""
    );
}

/// **K-10**: the `Fixture` trait's own default bodies, observed.
///
/// req/76 §2.2 lists seven survivors in them -- `normalise` and `equivalent_spellings` -- with the
/// reason in one line: "since the fs fixture overrides the default, nobody observes the default
/// value". The defaults are what an M7 adapter author gets before writing anything, and "nothing" is
/// the answer the harness turns into "NOT_SUPPLIED" rather than into a pass; a default that quietly
/// returned a value
/// would put an invented subject under a law.
///
/// Both subjects are here because they cover different halves: [`BareFixture`] takes **all seven**
/// defaults, and [`BrokenFixture`] takes the two normalisation ones while supplying the pairs, which
/// is the shape a half-built adapter actually has.
#[test]
fn the_default_bodies_of_the_fixture_trait_answer_nothing() {
    let bare = BareFixture::default();
    let reference = gx_core::DeltaRef {
        substrate: bare.adapter().kind(),
        cid: gx_core::Cid([0u8; 32]),
    };
    println!(
        "FIXTURE_DEFAULTS uninvertible={} commuting={} conflicting={} promised={} normalise={:?} \
         spellings={} resolve={}",
        bare.uninvertible().is_some(),
        bare.commuting_pair().is_some(),
        bare.conflicting_pair().is_some(),
        bare.promised_target().is_some(),
        bare.normalise("/mock//x"),
        bare.equivalent_spellings().len(),
        bare.resolve(&reference).is_some()
    );
    assert!(
        bare.uninvertible().is_none(),
        "the default invented an uninvertible delta"
    );
    assert!(
        bare.commuting_pair().is_none(),
        "the default invented a commuting pair"
    );
    assert!(
        bare.conflicting_pair().is_none(),
        "the default invented a conflicting pair"
    );
    assert!(
        bare.promised_target().is_none(),
        "the default invented a prophecy"
    );
    assert!(
        bare.normalise("/mock//x").is_none(),
        "the default normalisation returned a spelling, so L7 would measure a normalisation no \
         adapter wrote"
    );
    assert!(
        bare.equivalent_spellings().is_empty(),
        "the default named equivalent spellings, so L7's second half would compare pairs nobody \
         declared equal"
    );
    assert!(
        bare.resolve(&reference).is_none(),
        "the default resolved a reference"
    );

    // The same two defaults under the broken fixture, which supplies the pairs: L7 has to be
    // "NOT_SUPPLIED"
    // and not a pass, on a subject where the other obligations were measured.
    let half = BrokenFixture::new(Flaw::RefusesEverything);
    assert!(half.normalise("/mock//x").is_none());
    assert!(half.equivalent_spellings().is_empty());
    let report = run_all(&half);
    let l7 = report
        .checks
        .iter()
        .find(|c| c.id == "L7")
        .expect("L7 is in the law table");
    println!("L7_WITH_DEFAULTS={:?}", l7.outcome);
    assert!(
        matches!(l7.outcome, Outcome::NotSupplied(_)),
        "a fixture with no normalisation of its own was reported as {:?} rather than \"NOT_SUPPLIED\"",
        l7.outcome
    );
}

/// The control: the same run over a fixture that keeps its obligations is still green.
///
/// Without it, this file could be passing because the harness now fails everything -- which is the
/// mirror of the defect it was written for, and just as invisible.
#[test]
fn the_negative_control_did_not_break_the_positive_one() {
    let report = run_all(&MockFixture::new());
    println!(
        "CONTROL_AFTER_BROKEN PASS={} FAIL={} NOT_SUPPLIED={} meets={}",
        report.passed(),
        report.failed(),
        report.not_supplied(),
        report.meets_51_7()
    );
    assert!(
        report.meets_51_7(),
        "the mock stopped meeting 51 §7: {:?}",
        report.checks
    );
}
