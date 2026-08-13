//! The registry of invariants, and the one place every one of them is run (FR-026).
//!
//! Spec: 32 FR-026 for the registry and its duty (「全登録invariantに対し実行しなければならない」),
//! 34 AC-026 for the mock counter, 34 AC-029 for the order-2 path, 41 §4 for the trait, 42 §3.8 for
//! `InvariantResult` and `ReasonSource::Invariant`. Rulings: **E-M3-7** (the signature), **E-M3-9**
//! (the canonical order of the results), **E-M3-6** (the code a violation carries).
//!
//! # The signature 41 §4 writes, and the one this file declares
//!
//! 41 §4 逐語: `fn check(&self, pre: &ObjectSnapshot, planned: &PlannedDelta) -> Result<InvariantResult>`.
//! That signature cannot state AC-029, which asks an invariant to refuse an **order-2**
//! transformation -- `order` is a field of `Transformation` (41 §3) and neither argument carries
//! one. req/38 §19 rules the erratum:
//!
//! > 「M3-09(採用=案 a): `InvariantCheck::check(&GateInput)` へ署名変更(41 §4 erratum・監査#10 の
//! > 趣旨と一致)。E-A7-2/E-M2-5 と同型」 → **E-M3-7**
//!
//! `GateInput` is the structure 監査#10 asked for so that 「invariant 実行に必要な入力を含む」, so
//! taking it whole is the reading that makes the struct earn its existence. The two arguments 41 §4
//! names are still reachable, as `input.pre` and `input.planned` -- nothing was taken away.
//!
//! # What runs, and when
//!
//! Every registered check, once per [`crate::Gate::verify`], whatever the policy set answered.
//! FR-026 says 「全登録invariantに対し実行」 without a condition, and a registry that stopped early
//! on a policy `Deny` would make an `AdmitProof`'s record depend on the order the two systems were
//! consulted in. The cost is real -- work is done whose answer may be discarded on the `Deny` arm,
//! since 42 §3.8 gives reasons to a `Deny` and a proof only to an `Admit` -- and it is reported
//! rather than optimised away (`req/63_M3_HAND3_REPORT_2026-08-08.md` §4).
//!
//! # Why the order of the registry cannot reach the answer
//!
//! Three things make it so, and each is a test rather than a sentence
//! (`crates/gx-gate/tests/invariant_registry.rs`):
//!
//! 1. every check is called exactly once, so a permutation changes only *when* each is asked;
//! 2. the results come back in `invariant_id` order (**E-M3-9**), not in registration order;
//! 3. two checks may not claim one id, so 2 is a total order over the results rather than a stable
//!    sort's tie-break -- which is what makes the canonical bytes a function of the *set* of
//!    answers. A duplicate id would put registration order back inside the CID through
//!    `AdmitProof`'s IdentityView (42 §1.3), by the same argument that made hand 2 require `@id` on
//!    every policy.
//!
//! # The A-7 residual: this is not where a composed `created_at` is checked
//!
//! req/38 §7 left 「合成後 `created_at`/`intent_id` の許容値域が未定義のまま `compose()` は無検査で
//! 通す(46B WARN)」 and req/60 §4 routes the placement decision to this hand: 「値域検査は invariant
//! として書けるのか、それとも gx-core の `compose` が持つべきか」. **It cannot be written here.** A
//! check receives one `GateInput`, and a `GateInput` carries the composite (`t`) and the object it
//! is about (`pre`) -- not `f` and `g`. `t.parents` holds their **ids**, and resolving an id needs a
//! store, which 41 §6 does not give this crate. So the parts of the question with content -- 「a
//! composite is not older than what it composes」 and 「its intent is one of theirs」 -- are
//! unaskable from here, and the parts that remain (a timestamp is not negative, an intent id is not
//! the all-zero placeholder) are value-domain checks that belong beside the ceiling check
//! `compose` already performs through `Transformation::with_order`. The placement decision and the
//! erratum it raises are in `req/63_M3_HAND3_REPORT_2026-08-08.md` §3 and §4; **gx-core is not
//! modified by this hand** (52: 裁定禁, and req/60 §5.2 手 3 makes it 起票のみ).
//!
//! # What this file does not build
//!
//! Shipped invariants. FR-028's 「既製invariant集」 is the policy pack, and req/60 §5.2 puts it in
//! hand 5; M3-10's ruling already fixes what such a pack can reach (「v0.1 pack の実効範囲=locator/
//! actor/context/order 級」). The registry ships **empty**, and an empty registry is not the same
//! claim as 「every invariant held」 -- see [`InvariantRegistry::is_empty`].
//!
//! The meet of the two systems over all four quadrants (AC-027, with the Deny-precedence property
//! and the `verdict_meet` suite E-M3-10 names) is hand 4's, and so is E-M3-4's escalation rule and
//! the error-vocabulary window. What this hand owes AC-029 is one quadrant of that meet -- 「gate
//! 自体の policy(order-2 変換に対する invariant)が Deny を返せる」 -- and [`crate::Gate::verify`]
//! is where it lands, once, so that hand 4 widens a fold rather than writing a second one.

use std::collections::BTreeSet;

use crate::verdict::{InvariantResult, Reason, ReasonSource};
use crate::{Error, GateInput, Result};

// `INVARIANT_VIOLATED` was declared here until hand 4 and is now `crate::INVARIANT_VIOLATED`, in
// the crate root with the other three (**E-M3-6**). The window's requirement is 「crate 毎 Error
// 語彙表を 1 箇所宣言」, and a vocabulary spread across the modules that happen to use it is a
// vocabulary nobody can read in one place. The re-export in `lib.rs` keeps every caller's path.

/// One invariant (41 §4, with **E-M3-7**'s argument).
///
/// `Send + Sync` is 41 §4's, and it is what lets a `Gate` be shared: the engine that will call this
/// in M5 holds one gate and serves many transformations.
pub trait InvariantCheck: Send + Sync {
    /// The name this invariant answers under. 42 §3.8 makes it the `invariant_id` of every
    /// [`InvariantResult`] it returns, and [`InvariantRegistry::run_all`] refuses a result that
    /// says otherwise -- a record that misnames who answered is worse than no record.
    fn id(&self) -> &str;

    /// Decide, or say that deciding was impossible.
    ///
    /// # Errors
    /// Whatever the implementation cannot do. An `Err` here is the ⊥ of **E-M3-3** and not a
    /// refusal: [`crate::Gate::verify`] turns it into [`Error::Unevaluable`] rather than into a
    /// `Deny`, because 「『拒否』と『壊れている』を同じ verdict で語ると判定の意味論が濁る」.
    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult>;
}

/// The registry 41 §4 writes as `Vec<Box<dyn InvariantCheck>>`.
///
/// Registration order is kept, because it is the order the checks are *called* in and a caller
/// reading a trace should see what it asked for. It is not the order the answers come back in
/// (**E-M3-9**), and [`InvariantRegistry::run_all`] is where those two part company.
#[derive(Default)]
pub struct InvariantRegistry {
    checks: Vec<Box<dyn InvariantCheck>>,
}

impl core::fmt::Debug for InvariantRegistry {
    /// By the ids it holds, in registration order. `dyn InvariantCheck` has nothing else to show,
    /// and a `Debug` that printed nothing would make a panic message about a gate say nothing about
    /// which invariants it was holding.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InvariantRegistry")
            .field("ids", &self.ids())
            .finish()
    }
}

impl InvariantRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Add one, under the id it answers with.
    ///
    /// # Errors
    /// [`Error::RegistryUnusable`] when the id is empty, or when another registered check already
    /// answers under it. The second one is the load-bearing refusal: 42 §1.3 puts
    /// `AdmitProof.invariant_results` inside the IdentityView, **E-M3-9** canonicalises that vector
    /// by `invariant_id`, and a sort has nothing to order two equal keys by except the order they
    /// arrived in. So a duplicate id would make a receipt's CID depend on the order a deployment
    /// happened to register its invariants -- the same defect hand 2 refused in `@id`-less policies
    /// (`req/62_M3_HAND2_REPORT_2026-08-08.md` §3.7), reached by a different road.
    pub fn register(&mut self, check: Box<dyn InvariantCheck>) -> Result<()> {
        let id = check.id();
        if id.is_empty() {
            return Err(Error::RegistryUnusable {
                detail: "an invariant registered under the empty id names nothing in a receipt"
                    .to_string(),
            });
        }
        if self.checks.iter().any(|held| held.id() == id) {
            return Err(Error::RegistryUnusable {
                detail: format!(
                    "two invariants claim the id {id:?}; the canonical order of \
                     AdmitProof.invariant_results is that id (E-M3-9), so a second one would put \
                     registration order inside a receipt's CID"
                ),
            });
        }
        self.checks.push(check);
        Ok(())
    }

    /// The same, as a builder step.
    ///
    /// # Errors
    /// [`InvariantRegistry::register`]'s.
    pub fn with(mut self, check: Box<dyn InvariantCheck>) -> Result<Self> {
        self.register(check)?;
        Ok(self)
    }

    /// The ids this registry holds, **in registration order**.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.checks.iter().map(|check| check.id()).collect()
    }

    /// How many invariants are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// Whether nothing is registered.
    ///
    /// An empty registry answers every input with an empty result vector, and that is **not** the
    /// same statement as 「every invariant held」: it is 「nobody was asked」. The two look alike in
    /// an `AdmitProof` -- both leave `invariant_results` empty -- which is why FR-028's shipped pack
    /// (hand 5) is what turns a deployment's gate from the first into the second.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// Run every registered invariant, once, and return the answers in **E-M3-9**'s order.
    ///
    /// The two properties AC-026 and AC-029 rest on are here rather than at the call site: exactly
    /// one call each (there is one loop and it has no `continue`), and an order that is a function
    /// of the answers rather than of the registry.
    ///
    /// # Errors
    /// [`Error::Unevaluable`] when any check returned an `Err`, or when a check returned a result
    /// naming an `invariant_id` other than its own (42 §3.8: 「`InvariantCheck::id()` と等しい」).
    /// **Every check still runs first**: stopping at the first failure would make the reported
    /// error depend on registration order, which is the property this file exists to hold. The ids
    /// are sorted into the message for the same reason.
    pub fn run_all(&self, input: &GateInput<'_>) -> Result<Vec<InvariantResult>> {
        let mut results: Vec<InvariantResult> = Vec::with_capacity(self.checks.len());
        let mut failed: BTreeSet<String> = BTreeSet::new();

        for check in &self.checks {
            match check.check(input) {
                Ok(result) if result.invariant_id() == check.id() => results.push(result),
                Ok(result) => {
                    failed.insert(format!(
                        "{:?} answered under the id {:?}",
                        check.id(),
                        result.invariant_id()
                    ));
                }
                Err(error) => {
                    failed.insert(format!("{:?} could not decide: {error}", check.id()));
                }
            }
        }

        if !failed.is_empty() {
            let detail = failed.into_iter().collect::<Vec<String>>().join("; ");
            return Err(Error::Unevaluable {
                transformation: input.t.id,
                detail: format!("the invariant registry did not produce a full answer: {detail}"),
            });
        }

        results.sort_by(|a, b| a.invariant_id().cmp(b.invariant_id()));
        Ok(results)
    }
}

/// The reasons the invariants that did not hold give (42 §3.8).
///
/// One per violated invariant, in the order the results arrive -- which
/// [`InvariantRegistry::run_all`] has already made `invariant_id` order. `ReasonSource::Invariant`
/// is 42 §3.8's own variant, so unlike hand 2's Cedar-rule-3 case there is nothing here that the
/// vocabulary cannot name (`req/62_M3_HAND2_REPORT_2026-08-08.md` §4 C-3).
///
/// The message carries `detail` when there is one. 42 §3.8 asks for 「短い人間可読メッセージ(長文は
/// digest 化)」 and **M3-21** fixes the threshold at [`crate::MAX_MESSAGE_BYTES`]; both ends are
/// held, since a `detail` was bounded when its [`InvariantResult`] was built and the message built
/// from it is bounded again here. The second bound is not redundant: 「invariant {id} does not hold:
/// {detail}」 is longer than `detail`, so a detail just inside the bound produces a message just
/// outside it, and the elision moves to the message.
///
/// # Errors
/// [`Error::UnknownReasonCode`] cannot arise ([`crate::INVARIANT_VIOLATED`] is in the vocabulary),
/// and [`Error::NotDigestible`] only if an over-long message has no canonical form. The `Result` is
/// here because [`Reason::new`] is the only door into a `Reason` (**E-M3-6**).
pub fn violations(results: &[InvariantResult]) -> Result<Vec<Reason>> {
    results
        .iter()
        .filter(|result| !result.holds())
        .map(|result| {
            Reason::new(
                crate::INVARIANT_VIOLATED,
                match result.detail() {
                    Some(detail) => {
                        format!(
                            "invariant {} does not hold: {detail}",
                            result.invariant_id()
                        )
                    }
                    None => format!("invariant {} does not hold", result.invariant_id()),
                },
                ReasonSource::Invariant {
                    invariant_id: result.invariant_id().to_string(),
                },
            )
        })
        .collect()
}
