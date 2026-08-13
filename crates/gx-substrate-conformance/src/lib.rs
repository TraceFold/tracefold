//! The shared contract harness every `SubstrateAdapter` has to pass (51 §7).
//!
//! 51 §7 逐語: 「`SubstrateAdapter` traitを実装する全adapter（fs, git, mcp、将来追加分含む）が満たすべき
//! 契約を、adapter非依存の共有テストハーネス`gx-substrate-conformance`（新規test-only crate、35起票対象）
//! として実装する。各adapterクレートはこのハーネスを`#[test]`から呼び出すのみで契約テストを継承する
//! （cedar-conformance/cedar-specの手法と同型）」, and it closes with the completion condition:
//! 「各adapterは上記7契約すべてに合格しない限りM4/M7完了条件を満たさない」.
//!
//! # 身分: 「契約 7」 と 「法則 n」 は別の節である
//!
//! `req/38_ERRATA_2026-08-07.md` §30 M4H2-1 採(a) 逐語: 「harness の身分を **「契約 7(51 §7 と 1:1)+
//! 法則 n(裁定由来 L 系)」の 2 節に分離**し、出自を印字で区別(1:1 対応の自己証明を汚さない)」.
//!
//! The reason is the completion condition above. It counts to seven, so the seven have to stay
//! resolvable to 51 §7's own table -- and L6 (commutation symmetry) has no row there, because 51 §7's
//! commutation row is only 「可換/非可換2ケースがそれぞれ`Commutes`/`Conflicts{residual}`」. Filing L6
//! among the contracts would have made 「上記7契約」 mean eight things, quietly. So:
//!
//! * [`contracts`] is **1:1 with 51 §7**, verbatim, and `tests/harness_shape.rs` compares its table
//!   with the canon's on every run.
//! * [`laws`] is everything the rulings added -- L1〜L7 of req/69 §3.4 plus K1 of §30 M4H2-4 -- each
//!   row naming the ruling it comes from.
//! * Every [`Check`] carries its [`Origin`], and [`Report::print`] writes 「契約」 or 「法則」 beside each
//!   line, so a reader of the output never has to remember which list a name came from.
//!
//! # 対応の無い契約は「無い」と印字
//!
//! A contract whose subject a fixture does not supply is reported as [`Outcome::NotSupplied`] and
//! printed as 「無い」, **not** as a pass and not as a silent skip. Three of the seven need something
//! only the adapter's author can produce -- a delta whose inverse cannot be built (AC-048's
//! `Ok(None)`), a commuting pair and a conflicting pair -- and a harness that returned green for an
//! adapter that supplied none of them would report 「7契約すべてに合格」 about four contracts. req/29 §4
//! is the standing form of that rule: 「a skip and a pass must not look alike」.
//!
//! **Not supplied is not passed**, and since M4 hand 4 it is not a failure either:
//! [`Report::is_conformant`] answers 「失敗 0」 and [`Report::is_complete`] answers 「未測定 0」
//! (**§31 M4H3-4 (b)**), with [`Report::meets_51_7`] the completion condition that needs both. An
//! adapter that has not implemented a method says so with [`gx_substrate::Error::Unimplemented`] and
//! is reported 「無い」 for the obligations that needed it -- see [`unmeasured_or_failed`].
//!
//! # This crate is not `gx-conformance-gen` (**N-12**)
//!
//! req/69 §1 N-12: 「差分ベクタ生成器 `gx-conformance-gen` を作らない | 51 §4・ASM-51-1=M8。**51 §7 の
//! `gx-substrate-conformance`(adapter 契約ハーネス)とは別物**で、名前が紛らわしい事を doc に明記する」.
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
            Origin::Contract => "契約",
            Origin::Law => "法則",
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
    /// Printed as 「無い」. Never a pass: see the crate documentation.
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

    /// Nothing the harness could measure contradicted 51 §7 or a ruling: **失敗 0**.
    ///
    /// **§31 M4H3-4 採(b)** split this from [`Report::is_complete`]:
    ///
    /// > 「`is_conformant`(失敗 0)と **`is_complete`(未測定 0)を分離**。総合判定=両方 true。M7 の
    /// > git/mcp が別 subject を供給する時に「落ちた」と「測っていない」を区別できる語彙が要る(req/29 §4
    /// > の精神の正確な実装)」
    ///
    /// Hand 3 answered `passed == checks.len()`, which said 「無い」 and 「落ちた」 with one word. M4
    /// hand 4 is the hand that needed two: `gx-adapter-fs` implements four of the seven methods and
    /// refuses the rest with [`gx_substrate::Error::Unimplemented`], so eight obligations have no
    /// subject and **none of them is a defect in the adapter**. Reporting that as non-conformance
    /// would have made every partially built adapter look wrong instead of unfinished.
    ///
    /// Nothing is weaker than before: 「無い」 is still not a pass, and the completion condition of
    /// 51 §7 -- 「各adapterは上記7契約すべてに合格しない限りM4/M7完了条件を満たさない」 -- is
    /// [`Report::meets_51_7`], which needs both.
    #[must_use]
    pub fn is_conformant(&self) -> bool {
        !self.checks.is_empty() && self.failed() == 0
    }

    /// Every obligation had a subject to be run against: **未測定 0** (**§31 M4H3-4 (b)**).
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
    /// 「上記7契約すべてに合格」 needs the contracts to have been **run** as well as not to have
    /// failed, which is exactly [`Report::is_conformant`] and [`Report::is_complete`] together.
    #[must_use]
    pub fn meets_51_7(&self) -> bool {
        self.is_conformant() && self.is_complete()
    }

    /// One line per check, origin first.
    ///
    /// The counts go on their own line so that a report read from a CI log answers 「how many, and of
    /// which kind」 without the reader counting lines.
    ///
    /// # 🔴 **L-5 / 裁定 #17 — judged in M7 hand 4, and the answer is that it lives**
    ///
    /// This function carried a kill condition through four windows. req/77 §L-5 found that the
    /// mutation `Report::print → ()` survives (nobody asserts stdout); req/78 §5 row 23 sent the
    /// consumer question to M6; req/88 §5 row 35 **measured** that no consumer had grown and raised
    /// two options — a retirement mark now, or 「M7(adapter 2 本が conformance harness を実走する時)
    /// が最後の窓」 — and req/38 §46 took the second: 「M6 で殺すのは早い」. req/98 §3-4 row 8 states
    /// what M7 owes: 「消費者が生えたか否かを **1 度だけ**判定し、生えなければ**退役印**(E-7 の形)を
    /// 打つ」.
    ///
    /// **The consumer grew.** The candidate M6 named was 「M7 の adapter 2 本が conformance harness を
    /// 実走する時」 and all three adapters now do: `gx-adapter-fs` (M4 hand 6), `gx-adapter-git` (M7
    /// hand 1) and `gx-adapter-mcp` (M7 hand 3) each call this, and the conformance numbers quoted in
    /// req/99, req/100 and req/101 are the lines it printed. So **no retirement mark**, and the
    /// judgement is armed rather than remembered:
    /// `crates/gx-substrate-conformance/tests/print_consumers.rs` derives the call sites from the
    /// tree and requires every adapter crate to be among them, so the day the printing stops the
    /// question re-opens in red.
    ///
    /// What is **not** resolved and is deliberately left where §36 put it: the survivor. Asserting
    /// stdout needs a capture that collides with libtest's, so 「`Report::print` survivor=印字は
    /// assert 対象外(等価変異扱い)」 stands. req/77's option (b) — a `to_lines()` for the suites to
    /// assert on — would move the measurable content one function along and leave `print` exactly as
    /// unmeasurable, so it buys a published API and no property.
    pub fn print(&self, subject: &str) {
        println!(
            "CONFORMANCE {subject}: CHECKS={} PASS={} FAIL={} 無い={} 契約={} 法則={} \
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
                Outcome::NotSupplied(what) => format!("無い  {what}"),
            };
            println!("  [{}] {:<24} {verdict}", check.origin.label(), check.id);
        }
    }
}

/// What an adapter's author supplies so that the obligations can be run against it.
///
/// Everything with a default returns 「nothing」, and 「nothing」 is reported as 「無い」 rather than
/// skipped. The three optional pairs exist because no adapter-independent code can invent them: only
/// the author of an fs adapter knows which two changes touch different files, and only they know a
/// change whose inverse would exceed M4-21's escrow ceiling.
///
/// # The substrate has to be movable from outside
///
/// [`Fixture::disturb`] is what makes contract 3 and L4 falsifiable: a `precondition` that never
/// changes would satisfy every assertion that only looks at one moment. It stands for 「somebody else
/// wrote to the substrate」, which is the situation CON-2's CAS check exists for.
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

    /// Two deltas the adapter considers independent (51 §7's 「可換」 case).
    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        None
    }

    /// Two deltas the adapter considers dependent (51 §7's 「非可換」 case).
    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        None
    }

    /// The digest `plan` promised for this intent, if the adapter promises one.
    ///
    /// L5 is **M4-06 採(b)**: 「M4 は「adapter の自己整合」を conformance property で強制」. The prophecy
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
    /// from being 「参照はあるが指し先の無い CID」. There is no store in M4 -- that is the engine's, in
    /// M5 -- so what this hand can put in place is the shape of the question.
    fn resolve(&self, _reference: &gx_core::DeltaRef) -> Option<PlannedDelta> {
        None
    }
}

/// Which of the two answers a refusal is: 「まだ無い」 or 「落ちた」 (**§31 M4H3-4 (b)**).
///
/// [`gx_substrate::Error::Unimplemented`] is the one refusal that is not a failure of the adapter.
/// Its documentation says why it exists as a word of its own: an adapter built one hand at a time
/// has to be able to say 「not yet」 without panicking, without claiming the delta was bad, and
/// without answering `Ok` to a question it cannot answer. This is the single place the harness turns
/// that word into an outcome, so 「未実装」 cannot mean two things in two contracts.
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
