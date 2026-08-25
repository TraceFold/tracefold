// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The shared contract harness every `SubstrateAdapter` has to pass (51 §7). (sem:
//! SEM-gx-substrate-conformance-148, SEM-gx-substrate-conformance-149, SEM-gx-substrate-conformance-150,
//! SEM-gx-substrate-conformance-151, SEM-gx-substrate-conformance-152, SEM-gx-substrate-conformance-153,
//! SEM-gx-substrate-conformance-154, SEM-gx-substrate-conformance-155, SEM-gx-substrate-conformance-156,
//! SEM-gx-substrate-conformance-157, SEM-gx-substrate-conformance-158, SEM-gx-substrate-conformance-159,
//! SEM-gx-substrate-conformance-160, SEM-gx-substrate-conformance-161, SEM-gx-substrate-conformance-162,
//! SEM-gx-substrate-conformance-163)
//!
//! 51 §7, verbatim (sem: SEM-gx-substrate-conformance-038): "implement, as an adapter-independent
//! shared test harness `gx-substrate-conformance` (a new test-only crate, filed under 35), the
//! contract every adapter implementing the `SubstrateAdapter` trait (fs, git, mcp, and future
//! additions) must satisfy. Each adapter crate inherits the contract tests merely by calling this
//! harness from `#[test]` (the same shape as cedar-conformance/cedar-spec's method)", and it closes
//! with the completion condition: "no adapter satisfies the M4/M7 completion condition unless it
//! passes all seven of the above contracts".
//!
//! # 身分: 「契約 7」 と 「法則 n」 は別の節である
//!
//! (This heading is kept in Japanese (sem: SEM-gx-substrate-conformance-055):
//! `tests/harness_shape.rs::the_crate_states_its_two_sections_and_their_reason` checks this file's
//! text for the literal substrings "契約 7" and "法則 n".)
//! `req/38_ERRATA_2026-08-07.md` §30 M4H2-1, adopted (a), verbatim (sem:
//! SEM-gx-substrate-conformance-039): "split the harness's identity into **two sections, 'contract 7
//! (1:1 with 51 §7)' + 'law n (from the L-series the rulings produced)'**, and distinguish their
//! origin in print (so as not to muddy the 1:1 correspondence's self-proof)".
//!
//! The reason is the completion condition above. It counts to seven, so the seven have to stay
//! resolvable to 51 §7's own table -- and L6 (commutation symmetry) has no row there, because 51 §7's
//! commutation row is only "the commuting/non-commuting cases are `Commutes`/`Conflicts{residual}`
//! respectively". Filing L6 among the contracts would have made "the above seven contracts" mean
//! eight things, quietly (sem: SEM-gx-substrate-conformance-040). So:
//!
//! * [`contracts`] is **1:1 with 51 §7**, verbatim, and `tests/harness_shape.rs` compares its table
//!   with the canon's on every run.
//! * [`laws`] is everything the rulings added -- L1-L7 of req/69 §3.4 plus K1 of §30 M4H2-4 -- each
//!   row naming the ruling it comes from.
//! * Every [`Check`] carries its [`Origin`], and [`Report::print`] writes "contract" or "law" beside
//!   each (sem: SEM-gx-substrate-conformance-041)
//!   line, so a reader of the output never has to remember which list a name came from.
//!
//! # 対応の無い契約は「無い」と印字
//!
//! (This heading is kept in Japanese (sem: SEM-gx-substrate-conformance-056): the same
//! `tests/harness_shape.rs` test checks this file's text for this literal heading substring.)
//! A contract whose subject a fixture does not supply is reported as [`Outcome::NotSupplied`] and
//! printed as "NOT_SUPPLIED" (sem: SEM-gx-substrate-conformance-042), **not** as a pass and not as a
//! silent skip. Three of the seven need something
//! only the adapter's author can produce -- a delta whose inverse cannot be built (AC-048's
//! `Ok(None)`), a commuting pair and a conflicting pair -- and a harness that returned green for an
//! adapter that supplied none of them would report "all seven contracts pass" about four contracts
//! (sem: SEM-gx-substrate-conformance-043). req/29 §4 is the standing form of that rule: "a skip and
//! a pass must not look alike".
//!
//! **Not supplied is not passed**, and since M4 hand 4 it is not a failure either:
//! [`Report::is_conformant`] answers "zero failures" and [`Report::is_complete`] answers "zero
//! unmeasured" (sem: SEM-gx-substrate-conformance-044)
//! (**§31 M4H3-4 (b)**), with [`Report::meets_51_7`] the completion condition that needs both. An
//! adapter that has not implemented a method says so with [`gx_substrate::Error::Unimplemented`] and
//! is reported "NOT_SUPPLIED" for the obligations that needed it -- see [`unmeasured_or_failed`].
//!
//! # This crate is not `gx-conformance-gen` (**N-12**)
//!
//! req/69 §1 N-12 (sem: SEM-gx-substrate-conformance-045): "do not build a differential-vector
//! generator `gx-conformance-gen` | 51 §4, ASM-51-1 = M8. **it is a different thing from 51 §7's
//! `gx-substrate-conformance` (the adapter contract harness)**, and the doc states plainly that the
//! names are confusingly similar".
//! One word apart and one milestone apart: `gx-conformance-gen` is M8's generator of differential
//! vectors for the *canonical form*; this is M4's fixed suite of obligations for an *adapter*. They
//! share no code, no vocabulary and no schedule.
//!
//! # How an adapter crate uses it
//!
//! Implement [`Fixture`] over the adapter and whatever the adapter needs to be pointed at, then call
//! [`run_all`] from one `#[test]` and assert [`Report::is_conformant`]. That is the whole of the
//! inheritance 51 §7 asks for; `tests/support/mod.rs` in this crate is a worked example over an
//! in-memory mock, and `gx-adapter-fs` (hand 4) is the first real one.

#![forbid(unsafe_code)]

pub mod contracts;
pub mod laws;

use gx_core::{Cid, Intent, ObjectSnapshot};
use gx_substrate::{PlannedDelta, Result, SubstrateAdapter};

/// Where a check came from: 51 §7's table, or a ruling.
///
/// Printed rather than inferred (§30 M4H2-1 (a)), because the two have different force. A contract
/// failing means the adapter does not meet 51 §7 and M4/M7 are not complete; a law failing means it
/// contradicts a decision recorded in `req/38`, which may equally mean the decision needs revisiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// 51 §7's table, 1:1.
    Contract,
    /// req/69 §3.4's L-list and the rulings that survived §28/§30.
    Law,
}

impl Origin {
    /// The word that appears in a report line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            // Translated from "契約"/"法則" (sem: SEM-gx-substrate-conformance-046): the two callers
            // that pin this literal, `tests/contracts_seven.rs` and `tests/laws.rs`, were updated in
            // the same migration so the two stay in lockstep.
            Origin::Contract => "contract",
            Origin::Law => "law",
        }
    }
}

/// What running one check said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The obligation held.
    Pass,
    /// It did not, and this is what was seen.
    Fail(String),
    /// The fixture supplies no subject for this obligation, and says which one.
    ///
    /// Printed as "NOT_SUPPLIED". Never a pass: see the crate documentation.
    NotSupplied(String),
}

/// One obligation, run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    /// 51 §7's own cell for a contract, or `L1`..`L7` / `K1` for a law.
    pub id: String,
    pub origin: Origin,
    pub outcome: Outcome,
}

impl Check {
    pub(crate) fn new(id: &str, origin: Origin, outcome: Outcome) -> Self {
        Self {
            id: id.to_string(),
            origin,
            outcome,
        }
    }
}

/// Everything one run said about one adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// How many obligations held.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.outcome == Outcome::Pass)
            .count()
    }

    /// How many did not.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| matches!(c.outcome, Outcome::Fail(_)))
            .count()
    }

    /// How many had no subject to be run against.
    #[must_use]
    pub fn not_supplied(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| matches!(c.outcome, Outcome::NotSupplied(_)))
            .count()
    }

    /// Only the checks from one side of the split.
    #[must_use]
    pub fn of(&self, origin: Origin) -> Vec<&Check> {
        self.checks.iter().filter(|c| c.origin == origin).collect()
    }

    /// Nothing the harness could measure contradicted 51 §7 or a ruling: **zero failures**.
    ///
    /// **§31 M4H3-4, adopted (b)** split this from [`Report::is_complete`] (sem:
    /// SEM-gx-substrate-conformance-047):
    ///
    /// > "separate `is_conformant` (zero failures) and **`is_complete` (zero unmeasured)**. The
    /// > overall judgement = both true. A vocabulary that can distinguish 'failed' from 'not
    /// > measured' is needed for when M7's git/mcp supply a different subject (an exact
    /// > implementation of req/29 §4's spirit)"
    ///
    /// Hand 3 answered `passed == checks.len()`, which said "not supplied" and "failed" with one
    /// word (sem: SEM-gx-substrate-conformance-048). M4
    /// hand 4 is the hand that needed two: `gx-adapter-fs` implements four of the seven methods and
    /// refuses the rest with [`gx_substrate::Error::Unimplemented`], so eight obligations have no
    /// subject and **none of them is a defect in the adapter**. Reporting that as non-conformance
    /// would have made every partially built adapter look wrong instead of unfinished.
    ///
    /// Nothing is weaker than before: "NOT_SUPPLIED" is still not a pass, and the completion
    /// condition of 51 §7 -- "no adapter satisfies the M4/M7 completion condition unless it passes
    /// all seven of the above contracts" (sem: SEM-gx-substrate-conformance-049) -- is
    /// [`Report::meets_51_7`], which needs both.
    #[must_use]
    pub fn is_conformant(&self) -> bool {
        !self.checks.is_empty() && self.failed() == 0
    }

    /// Every obligation had a subject to be run against: **zero unmeasured** (sem:
    /// SEM-gx-substrate-conformance-050) (**§31 M4H3-4 (b)**).
    ///
    /// False whenever a fixture supplies no subject (hand 3's three optional pairs) **or** the
    /// adapter answers [`gx_substrate::Error::Unimplemented`]. The two are the same fact from two
    /// sides -- nobody has measured this obligation -- and neither is an accusation.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.checks.is_empty() && self.not_supplied() == 0
    }

    /// 51 §7's completion condition: both questions answered the right way.
    ///
    /// "all seven of the above contracts pass" (sem: SEM-gx-substrate-conformance-051) needs the
    /// contracts to have been **run** as well as not to have
    /// failed, which is exactly [`Report::is_conformant`] and [`Report::is_complete`] together.
    #[must_use]
    pub fn meets_51_7(&self) -> bool {
        self.is_conformant() && self.is_complete()
    }

    /// One line per check, origin first.
    ///
    /// The counts go on their own line so that a report read from a CI log answers "how many, and of
    /// which kind" without the reader counting lines.
    ///
    /// # 🔴 **L-5 / 裁定 #17 — judged in M7 hand 4, and the answer is that it lives**
    ///
    /// ("裁定 #17" above, sem: SEM-gx-substrate-conformance-152, and "退役印" below, sem:
    /// SEM-gx-substrate-conformance-052, are kept in Japanese:
    /// `tests/print_consumers.rs::the_decision_is_recorded_beside_the_function` checks this file's
    /// text for both literal substrings.)
    ///
    /// This function carried a kill condition through four windows. req/77 §L-5 found that the
    /// mutation `Report::print → ()` survives (nobody asserts stdout); req/78 §5 row 23 sent the
    /// consumer question to M6; req/88 §5 row 35 **measured** that no consumer had grown and raised
    /// two options — a retirement mark now, or "M7 (when two adapters actually run the conformance
    /// harness) is the last window" — and req/38 §46 took the second: "killing it in M6 is too
    /// early". req/98 §3-4 row 8 states what M7 owes (sem: SEM-gx-substrate-conformance-052): "judge,
    /// **exactly once**, whether a consumer grew, and if none did, strike **退役印** (a retirement
    /// mark, E-7's form)".
    ///
    /// **The consumer grew.** The candidate M6 named was "when two of M7's adapters actually run the
    /// conformance harness" and all three adapters now do: `gx-adapter-fs` (M4 hand 6),
    /// `gx-adapter-git` (M7
    /// hand 1) and `gx-adapter-mcp` (M7 hand 3) each call this, and the conformance numbers quoted in
    /// req/99, req/100 and req/101 are the lines it printed. So **no retirement mark**, and the
    /// judgement is armed rather than remembered:
    /// `crates/gx-substrate-conformance/tests/print_consumers.rs` derives the call sites from the
    /// tree and requires every adapter crate to be among them, so the day the printing stops the
    /// question re-opens in red.
    ///
    /// What is **not** resolved and is deliberately left where §36 put it: the survivor. Asserting
    /// stdout needs a capture that collides with libtest's, so "`Report::print`'s survivor = printing
    /// is out of scope for assertion (treated as an equivalent mutant)" (sem:
    /// SEM-gx-substrate-conformance-053) stands. req/77's option (b) — a `to_lines()` for the suites to
    /// assert on — would move the measurable content one function along and leave `print` exactly as
    /// unmeasurable, so it buys a published API and no property.
    pub fn print(&self, subject: &str) {
        println!(
            "CONFORMANCE {subject}: CHECKS={} PASS={} FAIL={} NOT_SUPPLIED={} CONTRACT={} LAW={} \
             conformant={} complete={}",
            self.checks.len(),
            self.passed(),
            self.failed(),
            self.not_supplied(),
            self.of(Origin::Contract).len(),
            self.of(Origin::Law).len(),
            self.is_conformant(),
            self.is_complete(),
        );
        for check in &self.checks {
            let verdict = match &check.outcome {
                Outcome::Pass => "pass".to_string(),
                Outcome::Fail(why) => format!("FAIL  {why}"),
                Outcome::NotSupplied(what) => format!("NOT_SUPPLIED  {what}"),
            };
            println!("  [{}] {:<24} {verdict}", check.origin.label(), check.id);
        }
    }
}

/// What an adapter's author supplies so that the obligations can be run against it.
///
/// Everything with a default returns "nothing", and "nothing" is reported as "NOT_SUPPLIED" rather
/// than
/// skipped. The three optional pairs exist because no adapter-independent code can invent them: only
/// the author of an fs adapter knows which two changes touch different files, and only they know a
/// change whose inverse would exceed M4-21's escrow ceiling.
///
/// # The substrate has to be movable from outside
///
/// [`Fixture::disturb`] is what makes contract 3 and L4 falsifiable: a `precondition` that never
/// changes would satisfy every assertion that only looks at one moment. It stands for "somebody else
/// wrote to the substrate", which is the situation CON-2's CAS check exists for.
pub trait Fixture {
    /// The adapter under test.
    fn adapter(&self) -> &dyn SubstrateAdapter;

    /// A locator that exists, **already normalised** (41 §4's `snapshot` receives one: H-2 /
    /// E-M4-12).
    fn locator(&self) -> String;

    /// An intent this adapter can plan against [`Fixture::locator`].
    fn intent(&self) -> Intent;

    /// Put the substrate back the way this fixture found it.
    ///
    /// Called before every obligation, so that a check never inherits the state another left behind.
    ///
    /// # Errors
    /// Whatever the fixture's own substrate does.
    fn reset(&self) -> Result<()>;

    /// Change the substrate behind the adapter's back.
    ///
    /// # Errors
    /// Whatever the fixture's own substrate does.
    fn disturb(&self) -> Result<()>;

    /// A delta whose inverse this adapter cannot construct (AC-048's `Ok(None)`).
    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        None
    }

    /// Two deltas the adapter considers independent (51 §7's "commuting" case).
    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        None
    }

    /// Two deltas the adapter considers dependent (51 §7's "non-commuting" case).
    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        None
    }

    /// The digest `plan` promised for this intent, if the adapter promises one.
    ///
    /// L5 is **M4-06, adopted (b)**: "M4 enforces "adapter self-consistency" as a conformance
    /// property" (sem: SEM-gx-substrate-conformance-054). The prophecy
    /// itself lives in `Transformation.target` (41 §3), which is the engine's value and not the
    /// adapter's, so the fixture is what carries it here.
    fn promised_target(&self) -> Option<Cid> {
        None
    }

    /// The adapter's lexical locator normalisation, if it has one (L7 / E-M4-12).
    fn normalise(&self, _locator: &str) -> Option<String> {
        None
    }

    /// Pairs of spellings the adapter's `≈` calls equal (42 §2.3), for L7's second half.
    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Resolve a `DeltaRef` back to the delta it names, if the fixture keeps a store.
    ///
    /// **M4-14**: `Conflicts{residual}` carries a CID, and **E-M4-8**'s storage row is what stops it
    /// from being "a CID that has a reference but no destination to point to". There is no store in
    /// M4 -- that is the engine's, in
    /// M5 -- so what this hand can put in place is the shape of the question.
    fn resolve(&self, _reference: &gx_core::DeltaRef) -> Option<PlannedDelta> {
        None
    }
}

/// Which of the two answers a refusal is: "not yet" or "failed" (**§31 M4H3-4 (b)**).
///
/// [`gx_substrate::Error::Unimplemented`] is the one refusal that is not a failure of the adapter.
/// Its documentation says why it exists as a word of its own: an adapter built one hand at a time
/// has to be able to say "not yet" without panicking, without claiming the delta was bad, and
/// without answering `Ok` to a question it cannot answer. This is the single place the harness turns
/// that word into an outcome, so "unimplemented" cannot mean two things in two contracts.
///
/// Every other refusal is a [`Outcome::Fail`] carrying the message, `context` naming the call.
#[must_use]
pub fn unmeasured_or_failed(error: &gx_substrate::Error, context: &str) -> Outcome {
    match error {
        gx_substrate::Error::Unimplemented { method, detail } => Outcome::NotSupplied(format!(
            "the adapter does not implement `{method}` yet ({detail}), so {context} is unmeasured \
             rather than failed"
        )),
        other => Outcome::Fail(format!("{context}: {other}")),
    }
}

/// Run 51 §7's seven contracts.
#[must_use]
pub fn run_contracts(fixture: &dyn Fixture) -> Report {
    Report {
        checks: contracts::run(fixture),
    }
}

/// Run the laws the rulings added.
#[must_use]
pub fn run_laws(fixture: &dyn Fixture) -> Report {
    Report {
        checks: laws::run(fixture),
    }
}

/// Run both, contracts first.
#[must_use]
pub fn run_all(fixture: &dyn Fixture) -> Report {
    let mut checks = contracts::run(fixture);
    checks.extend(laws::run(fixture));
    Report { checks }
}
