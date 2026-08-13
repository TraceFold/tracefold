//! The boundary trait: the seven things gx asks a substrate adapter to promise.
//!
//! Spec: 41 §4 「境界trait（P-6の要）」 for the signatures, 51 §7 for the shared contract suite every
//! adapter has to pass, 42 §3.3/§3.4/§3.5 for the values that cross. The amendments this module
//! implements are **E-M4-3**, **E-M4-4** and **E-M4-27** in `req/38_ERRATA_2026-08-07.md` §28/§29.
//!
//! # Seven, and why the count is a requirement
//!
//! **N-08** (req/69 §1): 「`SubstrateAdapter` に 41 §4 の 7 method 以外を生やさない」, which is 52 契約 2
//! at the size of one trait. Two rulings in this milestone wanted an eighth and neither got it:
//! M4-05 asked for a `can_invert` so that a gate could learn `invert_available` without doing the
//! work, and **E-M4-5** put that in the engine's verify step instead (it calls `invert` and folds
//! `Some`/`None`); M4-07 asked for a `compose_delta` so that composition would have a value, and
//! **M4-07 採(c)** put the composite in the payload as a free monoid -- 「trait は 1 行も変えず(P-6
//! 無傷・F1 不発)」. `crates/gx-substrate/tests/adapter_spec.rs` is where that count is measured
//! against 41 §4 rather than remembered.
//!
//! # What the seven do not add up to
//!
//! No arithmetic. req/69 §3.4 rules out `Add`/`Sub`/`Mul` for three reasons -- no group or ring
//! structure exists in the canon (subtraction was already given up in ASM-2), every one of these
//! methods is partial and returns `Result` (an operator cannot be partial), and an operator suggests
//! a total function over payloads that P-6 makes opaque. So the algebra **is** the trait, and its
//! content is the law list L1〜L8 (req/69 §3.4) that hand 3's `gx-substrate-conformance` will run
//! against real adapters. What this module owes those laws is their quantifiers, and that is what
//! the contract table below is for.
//!
//! # Which `Result` this is (**E-M4-28**)
//!
//! 41 §4 writes a bare `Result<..>` in every signature, and each crate in this workspace declares
//! its own, so the bare name in the canon resolves to 「the crate's own」 by precedent. Hand 2
//! imported [`gx_core::Result`] as the smallest thing that compiled, and said in the same breath why
//! it was probably wrong (req/71 §2 M4H2-2): `gx_core::Error` is a closed enum of ten variants whose
//! own documentation reads 「The crate does no I/O (41 §6), so there is no error here that comes from
//! the outside world -- every variant is a rejected argument」, so an fs adapter's `snapshot` could
//! not report a file it failed to read with any of them.
//!
//! `req/38_ERRATA_2026-08-07.md` §30 M4H2-2 採(a) settled it as **E-M4-28** and moved it forward a
//! hand: 「`gx_substrate::Error`+`Result` を宣言…**手4 でなく手3 冒頭**にするのは、conformance harness を
//! 誤った Result 型の上に建ててから差し替える手戻りを避けるため」. So these seven return
//! [`crate::Result`], and the rule that decides which vocabulary a failure belongs to is the layer
//! split of 41 §6: 「外界の失敗(「読めなかった」)は adapter 層の語彙・引数の拒否は gx-core の語彙」.
//! [`crate::Error::Core`] carries the second kind outward without relabelling it.
//!
//! None of the seven signatures below changed a character when that happened, which is the reason
//! `crates/gx-substrate/tests/adapter_spec.rs` measures the **import** as well as the text.
//!
//! # No clock, no randomness
//!
//! 41 §6 逐語: 「乱数・時刻はengine境界で注入（決定的リプレイのため）」. Nothing in these signatures hands
//! an adapter a moment or a seed, and [`crate::AppliedDelta::new`] takes the `Timestamp` as an
//! argument for the same reason (**M4-17**). `crates/gx-substrate/tests/substrate_contract.rs`
//! asserts that this crate names no clock at all.

use gx_core::{Commutation, Fingerprint, Intent, ObjectSnapshot, SubstrateKind};

use crate::delta::{AppliedDelta, PlannedDelta};
use crate::error::Result;

/// What gx requires of anything that can be changed under a gate (41 §4).
///
/// An implementation is the only code in the system that may read or write its substrate, and the
/// only code that may interpret a [`PlannedDelta`]'s payload (P-6). Everything above this boundary
/// -- gx-core, gx-canon, gx-gate, gx-witness, gx-log -- carries those bytes without looking inside
/// them, which is what makes 「どの substrate でも同じ証跡」 a property of the design rather than a
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
/// shows that reading 「適用は冪等」 and AC-049's round trip as laws about a state map at once forces
/// every delta to be the identity, and **E-M4-3** is the ruling that closes it by narrowing both.
/// 51 §7's seven contract rows and the law list L1〜L8 (req/69 §3.4) are the same seven obligations
/// seen from the harness side; hand 3 implements them there.
///
/// | method | contract | quantified over / ruling |
/// |---|---|---|
/// | `kind` | どの substrate の adapter かを名乗る。産物の `substrate` はすべてこの値(`PlannedDelta`・`Fingerprint`)で、呼ぶたびに同じ値を返す | この adapter 1 個に対して定数 / 41 §4・42 §3.1 |
/// | `snapshot` | locator が指す現在状態を `ObjectSnapshot` として返す。**locator は正規化済みで受ける**(正規化は crate root の `# Locator normalisation (normative)` 節が定める) | 1 回の呼び出しに対して / 51 §7 契約 1・**H-2**/**E-M4-12** |
/// | `plan` | intent を substrate 固有の delta へ写す。**副作用なし**(substrate を変えない)で、決定性は 「**(intent, snapshot) の対に対して**」 同一 `PlannedDelta`。**payload 上限超過は拒否**(定数 1 箇所=各 adapter が宣言する `MAX_FORWARD_PAYLOAD_BYTES`・fs の値は 1 MiB=手 6) | (intent, snapshot) の対 / FR-042・43 T-2・**E-M4-4**・**M4H5-4(b)**・AC-047 |
/// | `precondition` | 状態に名前を与える。**変更前後で異なる値**を返す事が CAS の前提。返り値の比較は `cas_eq` であり、42 §3.5 の等価性は 「**同一 adapter の産物間でのみ定義**」 | 1 つの scope の中で / 51 §7 契約 3・CON-2・**E-M4-27** |
/// | `apply` | commit 承認後にのみ呼ばれ、適用結果を観測値(`AppliedDelta`)として返す。冪等の量化は 「**同一 delta の再入**(retry)」 であって全域ではない | 同一 delta の再入 / 41 §4・51 §7 契約 7・43 T-10c・**E-M4-3** |
/// | `invert` | 逆 delta を構成する。**部分**(`Ok(None)` を許す)で、往復法則の量化は 「**`invert` に渡した `pre` の 1 点**」。**上限超過は `Ok(None)`**(定数 1 箇所=各 adapter が宣言する `MAX_INVERSE_PAYLOAD_BYTES`・fs の値は 1 MiB=手 5)。**`Ok(None)` は同一 object の正当な構成不能に限る**——別 object の `pre` は `Err` | pre の 1 点 / AC-048・AC-049・**E-M4-3**・**M4-21**・**E-M4-32** |
/// | `commutation` | 2 delta の独立性を判定する(差ではない=ASM-2/DPO)。**対称**=`commutation(a,b) == commutation(b,a)`・**commutation(a,a) は `Conflicts`**(同一資源への二重干渉=fail-closed) | 2 delta の対 / AC-052・AC-053・**M4-25** |
///
/// The rows above are checked by `crates/gx-substrate/tests/adapter_contract.rs`, clause by clause
/// and row by row, so that a quantifier cannot be dropped or written under the wrong method.
///
/// # AC-046: the boundary crosses threads
///
/// 34 AC-046 asks that a `Box<dyn SubstrateAdapter>` satisfy `Send + Sync` and that a value which
/// does not satisfy the bound fail to compile. **M4-20 採(b)** takes the second half with a
/// `compile_fail` doctest rather than `trybuild` -- 「依存 0・M1 先例・51 §2 が明示許容」 -- and the two
/// examples below are that pair. They differ by one field, so 「it failed to compile」 can only mean
/// the bound: the control compiles with the same trait, the same seven methods and the same boxing.
///
/// ```
/// use gx_core::{Commutation, Fingerprint, Intent, ObjectSnapshot, SubstrateKind};
/// use gx_substrate::{AppliedDelta, PlannedDelta, Result, SubstrateAdapter};
/// struct Shared;
/// impl SubstrateAdapter for Shared {
///     fn kind(&self) -> SubstrateKind { SubstrateKind::Fs }
///     fn snapshot(&self, _l: &str) -> Result<ObjectSnapshot> { unimplemented!() }
///     fn plan(&self, _i: &Intent, _p: &ObjectSnapshot) -> Result<PlannedDelta> { unimplemented!() }
///     fn precondition(&self, _s: &ObjectSnapshot) -> Result<Fingerprint> { unimplemented!() }
///     fn apply(&self, _d: &PlannedDelta) -> Result<AppliedDelta> { unimplemented!() }
///     fn invert(&self, _d: &PlannedDelta, _p: &ObjectSnapshot) -> Result<Option<PlannedDelta>> { unimplemented!() }
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
/// use gx_substrate::{AppliedDelta, PlannedDelta, Result, SubstrateAdapter};
/// struct NotShared(Rc<()>);
/// impl SubstrateAdapter for NotShared {
///     fn kind(&self) -> SubstrateKind { SubstrateKind::Fs }
///     fn snapshot(&self, _l: &str) -> Result<ObjectSnapshot> { unimplemented!() }
///     fn plan(&self, _i: &Intent, _p: &ObjectSnapshot) -> Result<PlannedDelta> { unimplemented!() }
///     fn precondition(&self, _s: &ObjectSnapshot) -> Result<Fingerprint> { unimplemented!() }
///     fn apply(&self, _d: &PlannedDelta) -> Result<AppliedDelta> { unimplemented!() }
///     fn invert(&self, _d: &PlannedDelta, _p: &ObjectSnapshot) -> Result<Option<PlannedDelta>> { unimplemented!() }
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

    /// Read the current state of `locator` (41 §4, 51 §7 契約 1).
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

    /// Work out the change an intent asks for, without making it (41 §4: 「純関数・副作用なし」).
    ///
    /// **E-M4-4** added the `pre` argument: 32 FR-042 requires 「同一intentから同一`PlannedDelta`が
    /// 生成され」 without saying against what, while 43 T-2 says 「**同一snapshotに対し**再実行しても
    /// 同一`PlannedDelta`」. Reading the pre-state from `&self` instead would have made two calls
    /// either side of an outside write disagree while both were correct, so the state the plan is
    /// relative to is now an argument -- which is req/69 §3.2's 「arrow は端点を持つ」 in the
    /// signature. AC-047 measures the determinism and that the substrate does not move.
    ///
    /// `req/spec/` is frozen (52), so 41 §4 still writes the one-argument form; the erratum ledger
    /// is the正本 and `adapter_spec.rs` holds both sides of the difference.
    ///
    /// # Errors
    /// When no delta can be planned for this intent against this snapshot.
    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta>;

    /// Name the state a commit is conditional on (41 §4, CON-2).
    ///
    /// 42 §3.5 lets the `scope` reach past the object itself to 「対象に干渉しうる周辺状態」, and
    /// 51 §7 契約 3 requires the value to **change when the state changes** -- a fingerprint that
    /// never moves would make the CAS check of 41 §5-5b unfalsifiable. Comparison is
    /// [`Fingerprint::cas_eq`] and never `==`: `Ok(false)` is 「動いた」, and the two `Err`s are
    /// 「その比較は意味を持たない」 (**E-M4-15**, **E-M4-27**).
    ///
    /// # Errors
    /// When the scope cannot be read.
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint>;

    /// Perform a delta that a gate has already admitted (41 §4: 「commit承認後にのみ呼ばれる」).
    ///
    /// Idempotence is quantified over 「**同一 delta の再入**(retry)」 (**E-M4-3**), which is the
    /// reading 51 §7 契約 7 and 43 T-10c already had: a crash between the write and the journal
    /// record must be recoverable by running the same delta again. It is **not** 「⟦δ⟧∘⟦δ⟧ = ⟦δ⟧ for
    /// every state」 -- that reading together with AC-049's round trip makes every delta the
    /// identity (req/69 §3.2).
    ///
    /// What comes back is an observation, not a new world: 41 §4 gives `apply` no pre-state and no
    /// post-state, so [`AppliedDelta`] carries a fingerprint and a digest of what the adapter saw
    /// afterwards. If `plan` promised a `target`, `resulting_digest` has to equal it -- L5, ruled a
    /// conformance property by **M4-06 採(b)** and implemented in hand 5.
    ///
    /// # Errors
    /// When the delta cannot be applied. 43 T-11 turns that into `AbortReason::ApplyFailed`.
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta>;

    /// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
    ///
    /// Partial in two ways. `Ok(None)` is a legitimate answer -- 41 §4: 「構成不能なら None → gateで
    /// 扱いが変わる」, and E-M3-4 is the gate rule that escalates on it -- and **M4-21** names the
    /// first real reason for one: an inverse that would have to carry more than the escrow ceiling
    /// declared in a single constant. The value of that constant is hand 5's.
    ///
    /// The law is quantified at one point. AC-049's round trip holds for 「**`invert` に渡した `pre`
    /// の 1 点**」 (**E-M4-3**), not for every state: a property test whose generator moves the state
    /// between `apply` and the inverse falsifies correct adapters, which is M3-05 one milestone
    /// later. Hand 3's harness writes that state into the property's Given.
    ///
    /// **E-M4-32** fixes which facts may take the `Ok(None)` form: 「**`Ok(None)` は「同一 object の
    /// 正当な構成不能」(上限超過・旧内容破棄済み)に限定**」. A `pre` that is a snapshot of **another**
    /// object is a mis-wired call and belongs in the `Err` half ([`crate::Error::LocatorMismatch`]) --
    /// answering `Ok(None)` would send a defect down E-M3-4's escalation path wearing the face of a
    /// legitimate business condition, which is the argument E-M4-27 made about `cas_eq`.
    ///
    /// # Errors
    /// When the question itself cannot be answered. 「Cannot be answered」 and 「the answer is no
    /// inverse」 are different, which is why this returns `Result<Option<_>>` and not `Option<_>`.
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>>;

    /// Decide whether two deltas are independent (41 §4, C-4 / ASM-2).
    ///
    /// Independence, not difference: ASM-2 gave up the commutator 「`[D_a,D_b]` の引き算による定義」
    /// because it needs Δ to be an abelian group, and what survives is the DPO parallel-independence
    /// question with a `residual` naming the obstruction (42 §3.6). **M4-25 採(a)** adds the
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
