// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The boundary trait: the seven things gx asks a substrate adapter to promise. (sem:
//! SEM-gx-substrate-031, SEM-gx-substrate-032, SEM-gx-substrate-033, SEM-gx-substrate-034,
//! SEM-gx-substrate-035, SEM-gx-substrate-036, SEM-gx-substrate-037, SEM-gx-substrate-038,
//! SEM-gx-substrate-039, SEM-gx-substrate-040, SEM-gx-substrate-041, SEM-gx-substrate-042,
//! SEM-gx-substrate-043, SEM-gx-substrate-044, SEM-gx-substrate-045, SEM-gx-substrate-046,
//! SEM-gx-substrate-047, SEM-gx-substrate-048, SEM-gx-substrate-049, SEM-gx-substrate-050,
//! SEM-gx-substrate-051, SEM-gx-substrate-052)
//!
//! Spec: 41 §4 "the boundary trait (P-6's linchpin)" for the signatures, 51 §7 for the shared
//! contract suite every
//! adapter has to pass, 42 §3.3/§3.4/§3.5 for the values that cross. The amendments this module
//! implements are **E-M4-3**, **E-M4-4** and **E-M4-27** in `req/38_ERRATA_2026-08-07.md` §28/§29.
//!
//! # Seven, and why the count is a requirement
//!
//! **N-08** (req/69 §1): "do not grow `SubstrateAdapter` any methods beyond 41 §4's 7", which is 52
//! contract 2
//! at the size of one trait. Two rulings in this milestone wanted an eighth and neither got it:
//! M4-05 asked for a `can_invert` so that a gate could learn `invert_available` without doing the
//! work, and **E-M4-5** put that in the engine's verify step instead (it calls `invert` and folds
//! `Some`/`None`); M4-07 asked for a `compose_delta` so that composition would have a value, and
//! **M4-07, adopted (c)** put the composite in the payload as a free monoid -- "not one line of the
//! trait changes (P-6 untouched, F1 unfired)". `crates/gx-substrate/tests/adapter_spec.rs` is where
//! that count is measured
//! against 41 §4 rather than remembered.
//!
//! # What the seven do not add up to
//!
//! No arithmetic. req/69 §3.4 rules out `Add`/`Sub`/`Mul` for three reasons -- no group or ring
//! structure exists in the canon (subtraction was already given up in ASM-2), every one of these
//! methods is partial and returns `Result` (an operator cannot be partial), and an operator suggests
//! a total function over payloads that P-6 makes opaque. So the algebra **is** the trait, and its
//! content is the law list L1-L8 (req/69 §3.4) that hand 3's `gx-substrate-conformance` will run
//! against real adapters. What this module owes those laws is their quantifiers, and that is what
//! the contract table below is for.
//!
//! # Which `Result` this is (**E-M4-28**)
//!
//! 41 §4 writes a bare `Result<..>` in every signature, and each crate in this workspace declares
//! its own, so the bare name in the canon resolves to "the crate's own" by precedent. Hand 2
//! imported [`gx_core::Result`] as the smallest thing that compiled, and said in the same breath why
//! it was probably wrong (req/71 §2 M4H2-2): `gx_core::Error` is a closed enum of ten variants whose
//! own documentation reads "The crate does no I/O (41 §6), so there is no error here that comes from
//! the outside world -- every variant is a rejected argument", so an fs adapter's `snapshot` could
//! not report a file it failed to read with any of them.
//!
//! `req/38_ERRATA_2026-08-07.md` §30 M4H2-2, adopted (a), settled it as **E-M4-28** and moved it
//! forward a hand: "declare `gx_substrate::Error`+`Result` ... putting it **at the start of hand 3
//! rather than hand 4** is to avoid the rework of building the conformance harness on the wrong
//! `Result` type and then swapping it out". So these seven return [`crate::Result`], and the rule
//! that decides which vocabulary a failure belongs to is the layer split of 41 §6: "a failure of the
//! outside world ('could not be read') is the adapter layer's vocabulary; a rejected argument is
//! gx-core's vocabulary".
//! [`crate::Error::Core`] carries the second kind outward without relabelling it.
//!
//! None of the seven signatures below changed a character when that happened, which is the reason
//! `crates/gx-substrate/tests/adapter_spec.rs` measures the **import** as well as the text.
//!
//! # No clock, no randomness
//!
//! 41 §6, verbatim: "randomness and time are injected at the engine boundary (for deterministic
//! replay)". Nothing in these signatures hands
//! an adapter a moment or a seed, and [`crate::AppliedDelta::new`] takes the `Timestamp` as an
//! argument for the same reason (**M4-17**). `crates/gx-substrate/tests/substrate_contract.rs`
//! asserts that this crate names no clock at all.

use gx_core::{
    Commutation, Fingerprint, Intent, ObjectSnapshot, ReadEntry, Reversibility, SubstrateKind,
};

use crate::delta::{AppliedDelta, PlannedDelta};
use crate::error::Result;

/// 🔴 **DR-46-26** -- what [`SubstrateAdapter::invert`] answers: the inverse, what was read to
/// build it, and C-25's three-valued verdict.
///
/// # Why the `Option` is kept **inside** rather than collapsed
///
/// `inverse.is_some()` and `verdict == Reversibility::True` are one fact in `gx-adapter-mcp`'s own
/// table (`invert.rs`), and a struct with two independent public fields can be built holding them
/// apart -- a `True` with no inverse claims an inverse was constructed and then does not carry it.
/// `req/441` §4 settled the same question for `ReadSet` in the same words: **the constructor
/// decides**. So the fields are private and the four constructors below are the only way in, which
/// makes the correspondence a property of the type rather than an assertion somebody has to
/// remember to write.
///
/// Collapsing the `Option` into the enum was the alternative (`Yes(delta) | No | Unknown`), and it
/// was not taken because the `Option` is not redundant from the *consumer's* side: 43 T-10b
/// branches on whether there is a body to escrow, and the engine's verify step folds exactly
/// `is_some()` into `invert_available` (**E-M4-5**). Both would have to re-derive the `Option` from
/// the enum at every call. Keeping it costs one field and removes a derivation.
///
/// # `reads` is `Vec<ReadEntry>` and not `ReadSet`
///
/// `req/441` §4, verbatim: "**spill is the constructor's decision** (`ReadSet::from_reads`). A form
/// in which the caller picks the variant makes the granularity tag a function of *the caller's
/// mood* rather than of the number of reads." An adapter reports objects; the engine's
/// `ReadSet::from_reads` decides G3 or G4. `crates/gx-substrate/tests/adapter_spec.rs` asserts that
/// no adapter in this workspace names a `ReadSet` variant at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvertOutcome {
    inverse: Option<PlannedDelta>,
    verdict: Reversibility,
    reads: Vec<ReadEntry>,
}

impl InvertOutcome {
    /// An inverse was constructed: [`Reversibility::True`], and the body is here.
    #[must_use]
    pub fn inverted(inverse: PlannedDelta, reads: Vec<ReadEntry>) -> Self {
        Self {
            inverse: Some(inverse),
            verdict: Reversibility::True,
            reads,
        }
    }

    /// No inverse exists for this call: [`Reversibility::False`].
    ///
    /// The legitimate reasons an adapter has for this are `gx-adapter-mcp`'s three: no tool is
    /// declared to undo this one, the declaration cannot be resolved from this call's material, or
    /// the body is over **M4-21**'s ceiling.
    #[must_use]
    pub fn none(reads: Vec<ReadEntry>) -> Self {
        Self {
            inverse: None,
            verdict: Reversibility::False,
            reads,
        }
    }

    /// 🔴 Nobody found out: [`Reversibility::Unknown`] (**DR-46-9 A-4**).
    ///
    /// The prior could not be read and this deployment declared `OnReadFailure::Unknown`, so
    /// whether an inverse exists was never established. Under the default posture the effect is
    /// refused instead and this value is never built.
    #[must_use]
    pub fn undetermined(reads: Vec<ReadEntry>) -> Self {
        Self {
            inverse: None,
            verdict: Reversibility::Unknown,
            reads,
        }
    }

    // 🔴🔴 **RETRACTED — DEFECT-892-1 (`req/895` §1).** `InvertOutcome::from_option` stood here.
    //
    // What it did: fixed `Vec::new()` on both arms, for the fs, git, mysql and postgres adapters.
    // What it said, verbatim, as its justification:
    //
    // > "The fs, git and postgres adapters build their inverse out of the snapshot they were
    // > handed, so there is no read that could fail separately from the call itself."
    //
    // 🔴 **That sentence is false, and the counterexample was one file away from it the whole
    // time.** `gx-adapter-fs/src/invert.rs`'s `read_if_present` calls `std::fs::read` on the
    // position and returns [`Error::Unreadable`]; `gx-adapter-git`'s reads the branch tip through
    // `repo::tip`; `gx-adapter-postgres`/`-mysql` open a connection, introspect the catalog and
    // `SELECT` the row. All four perform a read that can fail on its own, and all four were
    // reporting that they had performed none.
    //
    // What that cost, measured on a real lifecycle (`req/892`): a **signed** `CommitReceipt` for an
    // fs change carried `read_set = Nothing`, and `gx-witness/src/receipt.rs` documents that member
    // as answering `ReadSet::names` with `Some(false)` about **every** locator — "a *stronger*
    // answer than G3 gives". So the receipt did not omit the read; it denied it, in the same turn
    // as the read, under a signature. Not a fail-open and not an overclaim: a false positive with a
    // signature over it.
    //
    // The repair is not a smaller `from_option`. A constructor whose only job is to assert a fact
    // about its callers will be reached for by the next adapter too, so the assertion is deleted
    // with the fact. Each adapter now mints its read entry **at the one place in its own `invert`
    // where a read has answered**, which is the shape `gx-adapter-mcp` already had and the reason
    // `gx-adapter-mcp` was the one adapter this defect never reached.
    //
    // Anything that needs the old behaviour deliberately — a fixture standing in for an adapter
    // that genuinely reads nothing — calls [`InvertOutcome::inverted`] or [`InvertOutcome::none`]
    // with an empty list and is thereby saying so in its own source.

    /// The inverse, when one was constructed. `Some` exactly when [`Self::verdict`] is
    /// [`Reversibility::True`].
    #[must_use]
    pub fn inverse(&self) -> Option<&PlannedDelta> {
        self.inverse.as_ref()
    }

    /// C-25's answer for this call.
    #[must_use]
    pub const fn verdict(&self) -> Reversibility {
        self.verdict
    }

    /// The objects the escrow read, in the order the adapter read them.
    #[must_use]
    pub fn reads(&self) -> &[ReadEntry] {
        &self.reads
    }

    /// Take the inverse out, for the caller that escrows the body (43 T-10b).
    #[must_use]
    pub fn into_inverse(self) -> Option<PlannedDelta> {
        self.inverse
    }

    /// Take all three apart at once.
    #[must_use]
    pub fn into_parts(self) -> (Option<PlannedDelta>, Reversibility, Vec<ReadEntry>) {
        (self.inverse, self.verdict, self.reads)
    }
}

/// What gx requires of anything that can be changed under a gate (41 §4).
///
/// An implementation is the only code in the system that may read or write its substrate, and the
/// only code that may interpret a [`PlannedDelta`]'s payload (P-6). Everything above this boundary
/// -- gx-core, gx-canon, gx-gate, gx-witness, gx-log -- carries those bytes without looking inside
/// them, which is what makes "the same evidence trail regardless of substrate" a property of the
/// design rather than a
/// promise about implementations.
///
/// `Send + Sync` is 41 §4's own bound and **AC-046**'s subject: an engine holds adapters behind a
/// `Box<dyn SubstrateAdapter>` and may work on several transformations at once, so an implementation
/// that could not cross a thread boundary would be a boundary that only works single-threaded. The
/// bound is on the trait rather than on the box, so the refusal happens at the `impl` -- where the
/// author can see it -- instead of at each call site.
///
/// # The seven contracts
///
/// One row per method, quantifiers included. The quantifiers are the load-bearing part: req/69 §3.2
/// shows that reading "application is idempotent" and AC-049's round trip as laws about a state map
/// at once forces
/// every delta to be the identity, and **E-M4-3** is the ruling that closes it by narrowing both.
/// 51 §7's seven contract rows and the law list L1-L8 (req/69 §3.4) are the same seven obligations
/// seen from the harness side; hand 3 implements them there.
///
/// The full row table, translated (see code below):
///
/// | method | contract | quantified over / ruling | (sem: SEM-gx-substrate-024)
/// |---|---|---|
/// | `kind` | Names which substrate this adapter speaks for. Every product's `substrate` is this same value (`PlannedDelta`, `Fingerprint`), and it returns the same value every time it is called | constant for this one adapter / 41 §4, 42 §3.1 |
/// | `snapshot` | Returns the current state the locator names, as an `ObjectSnapshot`. **The locator arrives already normalised** (normalisation is defined by the crate root's `# Locator normalisation (normative)` section) | over one call / 51 §7 contract 1, **H-2**/**E-M4-12** |
/// | `plan` | Maps an intent to a substrate-specific delta. **No side effects** (does not change the substrate), and determinism is the same `PlannedDelta` "**for the pair (intent, snapshot)**". **Refuses when the payload ceiling is exceeded** (one constant place, `MAX_FORWARD_PAYLOAD_BYTES`, each adapter declares its own; the fs value is 1 MiB = hand 6) | the pair (intent, snapshot) / FR-042, 43 T-2, **E-M4-4**, **M4H5-4(b)**, AC-047 |
/// | `precondition` | Names the state. **A different value before and after a change** is the premise the CAS rests on. The comparison of return values is `cas_eq`, and 42 §3.5's equality is "**defined only between products of the same adapter**" | within one scope / 51 §7 contract 3, CON-2, **E-M4-27** |
/// | `apply` | Called only after a commit is approved, and returns the result of applying it as an observation (`AppliedDelta`). Idempotence is quantified over "**the same delta re-entering** (retry)", not over everything | the same delta re-entering / 41 §4, 51 §7 contract 7, 43 T-10c, **E-M4-3** |
/// | `invert` | Constructs the inverse delta **and reports what it read and what C-25 answered** (**E-DR4626-1**: the return is an [`InvertOutcome`], whose `inverse` is the pre-existing `Option`). **Partial** (an absent inverse is a legitimate answer), and the round-trip law is quantified over "**the one point of `pre` handed to `invert`**". **Exceeding the ceiling makes `Ok(None)`** (one constant place, `MAX_INVERSE_PAYLOAD_BYTES`, each adapter declares its own; the fs value is 1 MiB = hand 5). **`Ok(None)` is limited to a legitimate inability to construct for the same object** -- a `pre` of another object is `Err` | the one point of pre / AC-048, AC-049, **E-M4-3**, **M4-21**, **E-M4-32** |
/// | `commutation` | Decides whether two deltas are independent (not a difference = ASM-2/DPO). **Symmetric** = `commutation(a,b) == commutation(b,a)`; **`commutation(a,a)` is `Conflicts`** (double interference on the same resource = fail-closed) | the pair of two deltas / AC-052, AC-053, **M4-25** |
///
/// The rows above are checked by `crates/gx-substrate/tests/adapter_contract.rs`, clause by clause
/// and row by row, so that a quantifier cannot be dropped or written under the wrong method.
///
/// # AC-046: the boundary crosses threads
///
/// 34 AC-046 asks that a `Box<dyn SubstrateAdapter>` satisfy `Send + Sync` and that a value which
/// does not satisfy the bound fail to compile. **M4-20, adopted (b)** takes the second half with a
/// `compile_fail` doctest rather than `trybuild` -- "zero dependencies, the M1 precedent, 51 §2
/// explicitly allows it" -- and the two examples below are that pair. They differ by one field, so
/// "it failed to compile" can only mean
/// the bound: the control compiles with the same trait, the same seven methods and the same boxing.
///
/// ```
/// use gx_core::{Commutation, Fingerprint, Intent, ObjectSnapshot, SubstrateKind};
/// use gx_substrate::{AppliedDelta, InvertOutcome, PlannedDelta, Result, SubstrateAdapter};
/// struct Shared;
/// impl SubstrateAdapter for Shared {
///     fn kind(&self) -> SubstrateKind { SubstrateKind::Fs }
///     fn snapshot(&self, _l: &str) -> Result<ObjectSnapshot> { unimplemented!() }
///     fn plan(&self, _i: &Intent, _p: &ObjectSnapshot) -> Result<PlannedDelta> { unimplemented!() }
///     fn precondition(&self, _s: &ObjectSnapshot) -> Result<Fingerprint> { unimplemented!() }
///     fn apply(&self, _d: &PlannedDelta) -> Result<AppliedDelta> { unimplemented!() }
///     fn invert(&self, _d: &PlannedDelta, _p: &ObjectSnapshot) -> Result<InvertOutcome> { unimplemented!() }
///     fn commutation(&self, _a: &PlannedDelta, _b: &PlannedDelta) -> Result<Commutation> { unimplemented!() }
/// }
/// fn engine_holds<T: Send + Sync + ?Sized>(_: &T) {}
/// let boxed: Box<dyn SubstrateAdapter> = Box::new(Shared);
/// engine_holds(&*boxed);
/// ```
///
/// The same adapter carrying one `Rc` is not a `SubstrateAdapter`, and the refusal is at the `impl`.
/// Unmasking the block below (turning `compile_fail` into a running example) prints
/// `error[E0277]: Rc<()> cannot be sent between threads safely` and its `shared` twin, which is the
/// evidence that the block fails for the bound and not for a typo -- `tools/verify_m4h2.sh` §3 does
/// exactly that and restores the file:
///
/// ```compile_fail
/// use std::rc::Rc;
/// use gx_core::{Commutation, Fingerprint, Intent, ObjectSnapshot, SubstrateKind};
/// use gx_substrate::{AppliedDelta, InvertOutcome, PlannedDelta, Result, SubstrateAdapter};
/// struct NotShared(Rc<()>);
/// impl SubstrateAdapter for NotShared {
///     fn kind(&self) -> SubstrateKind { SubstrateKind::Fs }
///     fn snapshot(&self, _l: &str) -> Result<ObjectSnapshot> { unimplemented!() }
///     fn plan(&self, _i: &Intent, _p: &ObjectSnapshot) -> Result<PlannedDelta> { unimplemented!() }
///     fn precondition(&self, _s: &ObjectSnapshot) -> Result<Fingerprint> { unimplemented!() }
///     fn apply(&self, _d: &PlannedDelta) -> Result<AppliedDelta> { unimplemented!() }
///     fn invert(&self, _d: &PlannedDelta, _p: &ObjectSnapshot) -> Result<InvertOutcome> { unimplemented!() }
///     fn commutation(&self, _a: &PlannedDelta, _b: &PlannedDelta) -> Result<Commutation> { unimplemented!() }
/// }
/// fn engine_holds<T: Send + Sync + ?Sized>(_: &T) {}
/// let boxed: Box<dyn SubstrateAdapter> = Box::new(NotShared(Rc::new(())));
/// engine_holds(&*boxed);
/// ```
pub trait SubstrateAdapter: Send + Sync {
    /// Which substrate this adapter speaks for (42 §3.1).
    ///
    /// Constant for the life of the value: every `PlannedDelta` it plans and every `Fingerprint` it
    /// computes carries this same [`SubstrateKind`], which is what makes `cas_eq`'s refusal across
    /// substrates (**E-M4-27**) a statement about wiring rather than about state.
    fn kind(&self) -> SubstrateKind;

    /// Read the current state of `locator` (41 §4, 51 §7 contract 1).
    ///
    /// The locator arrives **already normalised**; the normalisation is lexical and is defined in
    /// this crate's root documentation under `# Locator normalisation (normative)` (**H-2** /
    /// **E-M4-12**). It is stated there rather than performed here because a shared normaliser in
    /// the boundary crate would be a path grammar living above the adapters -- the road M3-10
    /// refused for the gate.
    ///
    /// # Errors
    /// Whatever the adapter cannot read or cannot name.
    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot>;

    /// Work out the change an intent asks for, without making it (41 §4: "pure function, no side
    /// effects").
    ///
    /// **E-M4-4** added the `pre` argument: 32 FR-042 requires "the same `PlannedDelta` is produced
    /// from the same intent" without saying against what, while 43 T-2 says "re-running **against
    /// the same snapshot** produces the same `PlannedDelta`". Reading the pre-state from `&self`
    /// instead would have made two calls either side of an outside write disagree while both were
    /// correct, so the state the plan is relative to is now an argument -- which is req/69 §3.2's
    /// "an arrow has endpoints" in the signature. AC-047 measures the determinism and that the
    /// substrate does not move.
    ///
    /// `req/spec/` is frozen (52), so 41 §4 still writes the one-argument form; the erratum ledger
    /// is the canonical source and `adapter_spec.rs` holds both sides of the difference.
    ///
    /// # Errors
    /// When no delta can be planned for this intent against this snapshot.
    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta>;

    /// Name the state a commit is conditional on (41 §4, CON-2).
    ///
    /// 42 §3.5 lets the `scope` reach past the object itself to "surrounding state that can
    /// interfere with the target", and 51 §7 contract 3 requires the value to **change when the
    /// state changes** -- a fingerprint that never moves would make the CAS check of 41 §5-5b
    /// unfalsifiable. Comparison is [`Fingerprint::cas_eq`] and never `==`: `Ok(false)` is "moved",
    /// and the two `Err`s are "that comparison has no meaning" (**E-M4-15**, **E-M4-27**).
    ///
    /// # Errors
    /// When the scope cannot be read.
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint>;

    /// Perform a delta that a gate has already admitted (41 §4: "called only after a commit is
    /// approved").
    ///
    /// Idempotence is quantified over "**the same delta re-entering** (retry)" (**E-M4-3**), which
    /// is the reading 51 §7 contract 7 and 43 T-10c already had: a crash between the write and the
    /// journal record must be recoverable by running the same delta again. It is **not** "⟦δ⟧∘⟦δ⟧ =
    /// ⟦δ⟧ for every state" -- that reading together with AC-049's round trip makes every delta the
    /// identity (req/69 §3.2).
    ///
    /// What comes back is an observation, not a new world: 41 §4 gives `apply` no pre-state and no
    /// post-state, so [`AppliedDelta`] carries a fingerprint and a digest of what the adapter saw
    /// afterwards. If `plan` promised a `target`, `resulting_digest` has to equal it -- L5, ruled a
    /// conformance property by **M4-06, adopted (b)** and implemented in hand 5.
    ///
    /// # Errors
    /// When the delta cannot be applied. 43 T-11 turns that into `AbortReason::ApplyFailed`.
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta>;

    /// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
    ///
    /// Partial in two ways. `Ok(None)` is a legitimate answer -- 41 §4: "if it cannot be constructed,
    /// `None` -> the gate changes how it handles it", and E-M3-4 is the gate rule that escalates on
    /// it -- and **M4-21** names the
    /// first real reason for one: an inverse that would have to carry more than the escrow ceiling
    /// declared in a single constant. The value of that constant is hand 5's.
    ///
    /// The law is quantified at one point. AC-049's round trip holds for "**the one point of `pre`
    /// handed to `invert`**" (**E-M4-3**), not for every state: a property test whose generator
    /// moves the state
    /// between `apply` and the inverse falsifies correct adapters, which is M3-05 one milestone
    /// later. Hand 3's harness writes that state into the property's Given.
    ///
    /// **E-M4-32** fixes which facts may take the `Ok(None)` form: "**`Ok(None)` is limited to a
    /// 'legitimate inability to construct for the same object' (ceiling exceeded, old content
    /// already discarded)**". A `pre` that is a snapshot of **another**
    /// object is a mis-wired call and belongs in the `Err` half ([`crate::Error::LocatorMismatch`]) --
    /// answering `Ok(None)` would send a defect down E-M3-4's escalation path wearing the face of a
    /// legitimate business condition, which is the argument E-M4-27 made about `cas_eq`.
    ///
    /// # Errors
    /// When the question itself cannot be answered. "Cannot be answered" and "the answer is no
    /// inverse" are different, which is why this returns a `Result` around the outcome rather than
    /// a bare outcome -- and `Err` is **not** a fourth value of C-25. `gx-adapter-mcp`'s table
    /// (`invert.rs`) has four rows and the fourth is "the prior would not be read under the default
    /// posture: the effect is refused", which fails the commit closed: no receipt and no escrow row
    /// are written at all, so nothing downstream ever has to tell it apart from the other three.
    /// 🔴 **E-DR4626-1 (DR-46-26)** -- the return is an [`InvertOutcome`] and not a bare `Option`.
    ///
    /// 41 §4 writes `Result<Option<PlannedDelta>>` and `req/spec/` is frozen (52), so the erratum
    /// ledger is the canonical source and `crates/gx-substrate/tests/adapter_spec.rs` holds both
    /// halves of the difference -- the second use of the shape **E-M4-4** established for `plan`.
    /// **The method count does not move**: N-08 fixes `SubstrateAdapter` at seven and it is still
    /// seven, which is why this is a wider return and not the eighth method M4-05 and M4-07 were
    /// both refused (`req/38` §28).
    ///
    /// Two facts were being dropped at this line, and both for the same reason -- the value was
    /// computed and had no seat to travel in.
    ///
    /// 1. **What the escrow read.** `gx-adapter-mcp` holds `{digest, locator}` for the prior it
    ///    read, and `gx-engine` wrote `read_set: None` into every receipt because the value could
    ///    not reach it. `reads` is that value. An adapter returns `Vec<ReadEntry>` and **never** a
    ///    `ReadSet`: the granularity spill is `ReadSet::from_reads`'s decision (`req/441` §4), and
    ///    a caller that could choose the variant would make the tag a function of the caller rather
    ///    than of the number of objects.
    /// 2. **C-25's third value.** `Ok(None)` was the junction at which "no inverse exists" and
    ///    "nobody found out" became one word, and `gx_engine::store::InverseStatus::Undetermined`
    ///    names this exact line as the block it was waiting on. `verdict` is that value.
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome>;

    /// Decide whether two deltas are independent (41 §4, C-4 / ASM-2).
    ///
    /// Independence, not difference: ASM-2 gave up the commutator "the definition via `[D_a,D_b]`'s
    /// subtraction" because it needs Δ to be an abelian group, and what survives is the DPO
    /// parallel-independence question with a `residual` naming the obstruction (42 §3.6). **M4-25,
    /// adopted (a)** adds the
    /// symmetry that DPO independence has and 41 §4 never wrote -- `commutation(a,b)` and
    /// `commutation(b,a)` agree -- and fixes the reflexive case at `Conflicts`, since a delta and
    /// itself touch the same resource and fail-closed is the conservative side.
    ///
    /// AC-053 requires this to be callable with no engine and no gate in the picture, which is why
    /// it is a method on the adapter and takes two plain deltas.
    ///
    /// # Errors
    /// When the two deltas cannot be compared at all -- two substrates, or a payload this adapter
    /// did not write.
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation>;
}
