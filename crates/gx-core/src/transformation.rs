//! The transformation itself, and the identifiers that address it.
//!
//! Spec: 41 §3 for `Transformation`, `Subject`, `IntentId`, `TransformationId`; 42 §0 for this
//! module's contents; 42 §3.2 for the auxiliary types; 42 §1.3 for what is inside the
//! IdentityView and what is not.

use crate::context::{Actor, ChangeContext};
use crate::delta::DeltaRef;
use crate::error::{Error, Result};
use crate::object::{ObjectId, ObjectSnapshot};
use crate::Cid;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

/// The highest order v0.1 admits (ASM-6, DR-7 DEFAULT: <=2).
///
/// A constant rather than a type-level bound, because the ceiling is a decision record's value
/// and decision records move. FR-003 says so in as many words: an over-high order 「コンパイル時
/// 型検査を通過してもランタイムでErrorを返す」.
pub const MAX_ORDER: u8 = 2;

/// 41 §3: fixed at `submit` time (43 T-1, the Draft transition) and immutable afterwards. Same
/// intent, same `IntentId` -- that determinism is what `Intent`'s canonical form buys (ASM-11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IntentId(pub Cid);

/// 41 §3: the CID of the canonical form including `delta` and `target`, fixed when `plan()`
/// completes (43 T-2, the Candidate transition) and immutable afterwards (ASM-11).
///
/// A `TransformationId` built by hand names nothing in particular; the value that makes it an
/// identity comes from `gx-canon` (A-1). gx-core is the layer that carries ids, not the layer
/// that mints them -- which is also why composition takes its id from an injected function
/// rather than computing one (req/31 §7, 46 §2.1's `idOf`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TransformationId(pub Cid);

/// 42 §3.2: UTC nanoseconds since the Unix epoch.
///
/// Metadata, and only that (ASM-4). 42 §1.3-2 keeps it out of the `Transformation` IdentityView:
/// when a change was *recorded* is not part of what the change *is*, so recording the same
/// transformation twice must not produce two identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

/// What a transformation acts on (41 §3, 42 §3.2).
///
/// `Object` at order 0, `Transformation` at order >= 1. The enum is what makes a transformation
/// of a transformation expressible at all (P-2) without a second struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Subject {
    Object(ObjectId),
    Transformation(TransformationId),
}

/// A transformation as a first-class object (P-1, P-2, 41 §3).
///
/// Ten fields. The AC-002 text lists nine, omitting `intent_id`; A-3
/// (`req/38_ERRATA_2026-08-07.md` §1) rules 41 §3 correct and that omission an erratum, and 42
/// §1.3 already counted `intent_id` among the eight IdentityView fields, so the ten here and the
/// eight there agree: `id` and `created_at` are the two that stay out (self-reference, ASM-4).
///
/// What is *not* here matters as much: no lifecycle state. 42 §1.3-3 forbids encoding
/// Draft/Candidate/Committed into the value, because state is mutable and identity is not; the
/// engine keeps it in a table keyed by `TransformationId`.
///
/// # Nine fields are `pub`; `order` is reached through [`Transformation::order`]
///
/// 41 §3 enumerates ten fields, and all ten are observable -- but enumerating a field is a
/// statement about what the value holds, not about the Rust visibility the field is written with
/// (`req/46D_AUDIT_RULING_2026-08-07.md` §1 F-2). `order` carries the only invariant in the
/// struct, DR-7's ceiling, and while it was `pub` a struct literal could write `order: 250`
/// without ever reaching [`Transformation::with_order`] -- measured in
/// `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §3 W-2. Private is what makes the ceiling a property
/// of the type: the field is read with `order()`, written with `set_order()`, and built with
/// [`Transformation::new`], and all three routes are the one check.
///
/// The cost is that a struct literal no longer builds this type from outside the crate. That is
/// the point -- there is one door -- and the exhaustive field check AC-002 asks for moved to the
/// unit test at the bottom of this file, where the crate can still see all ten.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transformation {
    /// The canonical-form CID. Fixed at `plan()`, not before (ASM-11).
    pub id: TransformationId,
    /// The Draft this came from. Survives into the Candidate so the origin stays traceable.
    pub intent_id: IntentId,
    /// 0 = a transformation of an object, 1 = of a transformation, 2 = of one of those.
    /// v0.1 admits <= 2 (ASM-6, DR-7); the check lives in `Transformation::with_order`, and this
    /// field is private so that nothing can set it without going there (F-2).
    order: u8,
    pub subject: Subject,
    /// The expected post-state digest, fixed by `plan()`. `None` while still a Draft.
    pub target: Option<Cid>,
    pub delta: DeltaRef,
    pub context: ChangeContext,
    pub actor: Actor,
    /// The provenance and composition DAG (C-2, C-6). Traversed by `ancestors`.
    pub parents: Vec<TransformationId>,
    /// Metadata. Outside the IdentityView (ASM-4, 42 §1.3-2).
    pub created_at: Timestamp,
}

/// The ancestors of `start`, nearest first, each one once (FR-007, C-2, C-6).
///
/// Breadth-first over `parents`, so a node's own parents come before its grandparents and the
/// linear chain of AC-007 yields `[T2.id, T1.id]`. Within one generation the order is the order
/// the `parents` vector was written in, which makes the result a function of the stored value
/// rather than of a hash seed -- the same reason 42 §2.1-2 sorts map keys bytewise and the same
/// reason `alpha-fold` reaches for a `BTreeMap` over a `HashMap` (req/34 §2).
///
/// `resolve` is how this stays a pure function in a crate that may not do I/O (41 §6). It is
/// allowed to fail: an id named in a `parents` list is an ancestor whether or not the caller's
/// store can produce the value behind it, so an unresolvable id is returned and not followed.
/// Cycles are malformed input and are not the caller's fault to survive -- the visited set makes
/// this terminate on one rather than hanging, since 41 §6 counts a hang as the bug it is.
///
/// `start` itself is not in the result unless a cycle leads back to it.
pub fn ancestors<'a, F>(start: &TransformationId, resolve: F) -> Vec<TransformationId>
where
    F: Fn(&TransformationId) -> Option<&'a Transformation>,
{
    let mut out: Vec<TransformationId> = Vec::new();
    let mut seen: BTreeSet<TransformationId> = BTreeSet::new();
    let mut queue: VecDeque<TransformationId> = VecDeque::new();

    if let Some(t) = resolve(start) {
        queue.extend(t.parents.iter().copied());
    }
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        out.push(id);
        if let Some(t) = resolve(&id) {
            queue.extend(t.parents.iter().copied());
        }
    }
    out
}

/// What composition cannot work out for itself (the A-7 erratum, `req/38_ERRATA_2026-08-07.md`
/// §1, and req/31 §7).
///
/// 46 §2.1's Lean `compose` builds four fields -- `id`, `order`, `src`, `dst` -- and says so in as
/// many words: 「provenance/context/actor 等は F0 の定理には非負荷なので opaque metadata として型に
/// 持たせず省略する」. 41 §3's `Transformation` has ten. The five that the composite's own shape
/// does not determine are gathered here and supplied by the caller, because gx-core is not the
/// layer that decides what a composed change *means*; the engine is, in M5.
///
/// Two of the five are not in the A-7 erratum's list of three (`delta`/`context`/`actor`):
/// `intent_id` and `created_at`. They are required fields of 41 §3 all the same, so composition
/// cannot avoid supplying them, and inventing values here would be worse than asking. Reported as
/// an erratum gap in `req/45_M1_HAND5_REPORT_2026-08-07.md`.
///
/// # The range those two are allowed to be in (**E-M3-13**)
///
/// That erratum gap became `req/38_ERRATA_2026-08-07.md` §7's open item -- 「合成後 `created_at` /
/// `intent_id` の許容値域が未定義のまま `compose()` は無検査で通す(46B WARN)」 -- and §22 closes it
/// as **D-6 / E-M3-13**, with two predicates on this struct and a third on composition:
///
/// | # | predicate | checked by | refusal |
/// |---|---|---|---|
/// | ① | `created_at >= 0` | [`Transformation::new`], [`compose`], [`identity`] | [`Error::CreatedAtNegative`] |
/// | ② | `intent_id` is not the all-zero placeholder | [`Transformation::new`], [`compose`], [`identity`] | [`Error::IntentIdUnset`] |
/// | ③ | `created_at >= max(f.created_at, g.created_at)` | [`compose`] only | [`Error::CreatedAtBeforeParts`] |
///
/// Two candidate predicates are **deliberately absent**, and their absence is checked in
/// `crates/gx-core/tests/compose_range.rs` so that it stays a decision rather than an oversight:
///
/// * 「the composite's intent is one of `f`'s and `g`'s」 (④) was **not adopted**. req/38 §22 逐語:
///   「**④(intent ∈ {f,g})は不採用**——`CompositionMetadata` の doc 自身が「両者が別 intent から
///   来うる」と書き、合成物が新しい intent を持つ読みを spec は禁じていない」. The doc it means is the
///   `intent_id` field below, and it says so still.
/// * 「`created_at` is not in the future」 needs a clock, and 41 §6 keeps clocks out of this crate.
///   M5 injects one at the engine boundary, which is where that predicate belongs.
///
/// [`identity`] takes this struct too and, since **E-M3-18** (M4 hand 1), checks ① and ② as well.
/// E-M3-13 had named only the two constructors, so M3 hand 6 implemented those and *measured* the
/// third door instead of widening a ruling on its own authority (req/66 §4); `req/38` §25 H-1 is
/// the widening, and `crates/gx-core/tests/value_range_closure.rs` is where the count of doors that
/// still hand out an unchecked `Transformation` is printed rather than assumed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionMetadata {
    /// Which Draft the composite is accounted to. Composition does not create an intent, and the
    /// two arrows being composed may come from different ones, so there is no defensible way to
    /// derive this from `f` and `g`.
    pub intent_id: IntentId,
    /// The delta the composite is said to apply. Not `f.delta` and not `g.delta`: the composition
    /// of two changes is a third change, and describing it is the caller's business.
    pub delta: DeltaRef,
    pub context: ChangeContext,
    pub actor: Actor,
    /// Metadata (ASM-4), outside every IdentityView (42 §1.3-2). gx-core cannot read a clock
    /// (41 §6 forbids I/O), which is also how `req/26 §1`'s no-clock-in-a-signed-payload rule
    /// stays satisfied by construction rather than by discipline.
    pub created_at: Timestamp,
}

/// The id a transformation carries while its real one is being derived from it.
///
/// Handing the derivation callback a whole `Transformation` needs *some* value in the `id` field,
/// and 42 §1.3 excludes `id` from the IdentityView -- so the CID computed over the provisional
/// value is bit-for-bit the CID computed over the final one, and the placeholder cannot change
/// the answer. All zeros rather than, say, `f.id`, so a callback that wrongly reads `.id` returns
/// a constant and is caught by the first test that composes two different pairs.
///
/// Both [`compose`] and [`identity`] hand the callback a value carrying this, which is the F-1
/// ruling of `req/46D_AUDIT_RULING_2026-08-07.md` §1: one shape of callback, so one way to mint
/// the id of an arrow.
const PROVISIONAL_ID: TransformationId = TransformationId(Cid([0u8; 32]));

/// The same thirty-two zero bytes, read as an `IntentId`: 「no Draft named yet」 (**E-M3-13** ②).
///
/// Written beside [`PROVISIONAL_ID`] because it is the same convention seen from the other field,
/// and because a reader asking 「why is *this* value special」 should find both answers in one place.
/// The difference is what each one is allowed to reach: `id` is outside the IdentityView (42 §1.3),
/// so a provisional one changes no CID and is handed to the callback on purpose -- while
/// `intent_id` is inside it, so a placeholder there reaches the digest and two unrelated arrows
/// carrying it agree on the field that says where each came from.
const UNSET_INTENT: IntentId = IntentId(Cid([0u8; 32]));

impl CompositionMetadata {
    /// **E-M3-13** ① and ②, checked where the two fields arrive.
    ///
    /// One function rather than three call sites' worth of `if`s, so that [`Transformation::new`],
    /// [`compose`] and [`identity`] cannot drift into checking different things -- the reason
    /// req/31 §7 routes every order check through [`Transformation::with_order`]. **E-M3-18** made
    /// the third caller, and it made it by adding a call rather than a predicate.
    ///
    /// # Errors
    /// [`Error::CreatedAtNegative`] and [`Error::IntentIdUnset`], in that order. Which one a caller
    /// sees when both hold is fixed here rather than left to the reading order of a struct.
    fn in_range(&self) -> Result<()> {
        if self.created_at.0 < 0 {
            return Err(Error::CreatedAtNegative {
                got: self.created_at.0,
            });
        }
        if self.intent_id == UNSET_INTENT {
            return Err(Error::IntentIdUnset);
        }
        Ok(())
    }
}

/// Do these two arrows meet? 46 §2.1: `composable f g := f.dst = g.src`.
///
/// In 41's vocabulary `f.dst` is `f.target` (the promised post-state digest) and `g.src` is
/// whatever digest `g.subject` denotes -- and resolving a `Subject` to a digest needs a store,
/// which gx-core may not have (41 §6). So the store arrives as a function, exactly as it does for
/// [`ancestors`]. A resolver that cannot answer makes the pair non-composable rather than
/// composable: an unknown is not a match.
pub fn composable<R>(f: &Transformation, g: &Transformation, resolve: R) -> bool
where
    R: Fn(&Subject) -> Option<Cid>,
{
    composability(f, g, resolve).is_ok()
}

/// The same question, with the reason attached. [`composable`] is this with the reason discarded,
/// so the predicate and the error can never drift apart.
fn composability<R>(f: &Transformation, g: &Transformation, resolve: R) -> Result<()>
where
    R: Fn(&Subject) -> Option<Cid>,
{
    let dst = f.target.ok_or(Error::TargetMissing)?;
    let src = resolve(&g.subject).ok_or(Error::SubjectUnresolved)?;
    if dst == src {
        Ok(())
    } else {
        Err(Error::NotComposable)
    }
}

/// `g ∘ f`: the composite that runs `f` and then `g` (46 §2.1, req/31 §7).
///
/// The argument order is 46 §2.1's -- `compose f g` where `composable f g` means `f.dst = g.src`
/// -- so the first argument is the arrow that runs first. AC-059 writes the same composite as
/// `compose(g,f)` in the mathematician's order; the function is the one the Lean definition
/// names, and the AC is prose about its measure.
///
/// The four fields the Lean model builds, written in 41's vocabulary:
///
/// | 46 §2.1 | here |
/// |---|---|
/// | `src := f.src` | `subject := f.subject` |
/// | `dst := g.dst` | `target := g.target` |
/// | `order := max f.order g.order` | the same, through [`Transformation::with_order`] |
/// | `id := composeId f.id g.id` | `id_of` applied to the composite, see below |
///
/// and `parents := [f.id, g.id]`, which is 41's way of saying what Lean's `composeId` says: the
/// composite remembers what it was made of (C-2, C-6), so [`ancestors`] walks back through it.
///
/// # Why the id arrives as a callback
///
/// A `TransformationId` is the CID of the canonical form (41 §3, ASM-11), and canonical forms are
/// gx-canon's, which gx-core may not name (A-1). 46 §2.1 has the same shape on the Lean side --
/// `composeId` is an `axiom`, and `identity` takes `idOf` as a parameter -- so the injected
/// function is not a Rust workaround but the same seam in both models. The callback receives the
/// composite carrying an all-zero provisional `id`, which is sound because 42 §1.3 keeps `id` out
/// of the IdentityView: the digest of the provisional value and of the final value are the same
/// bytes. So a callback that computes the CID of what it was handed has computed the CID of what
/// it will be given back -- which is what makes 42 §1.3 evaluable from inside the seam.
///
/// Because `composeId` is an axiom (`ASM-03-2`), associativity is *not* proved on either side.
/// Nothing here asserts `compose(compose(f,g),h) == compose(f,compose(g,h))`, and no test in this
/// crate should be written as though it did.
///
/// # The metadata's range (**E-M3-13**, and why ③ is only here)
///
/// The three predicates are tabled on [`CompositionMetadata`]. ③ -- 「a composite is not dated
/// before the arrows it is made of」 -- is checked in this function and in no other, because this is
/// the only place that holds `f` and `g`. [`Transformation::new`] receives a `parents` list of
/// *ids*, and resolving an id to a value needs a store that 41 §6 forbids this crate; so `new`
/// cannot evaluate ③ even in principle, which is what makes 「compose のみ」 a consequence rather
/// than a choice.
///
/// The order of the checks is: composability first, then ① and ②, then ③. A pair that does not
/// meet has no composite to have metadata about, so its refusal is the earlier fact.
///
/// # Errors
/// - [`Error::TargetMissing`] when `f` is still a Draft.
/// - [`Error::SubjectUnresolved`] when the resolver cannot place `g`'s subject.
/// - [`Error::NotComposable`] when the two arrows do not meet.
/// - [`Error::CreatedAtNegative`] / [`Error::IntentIdUnset`] when the metadata is outside ① or ②.
/// - [`Error::CreatedAtBeforeParts`] when the composite would predate `f` or `g` (③).
/// - [`Error::OrderExceeded`] when `max(f.order, g.order)` is above [`MAX_ORDER`]. The check is
///   [`Transformation::with_order`]'s, not a second copy of it (req/31 §7), so a decision record
///   that moves the ceiling moves it here too.
pub fn compose<R, I>(
    f: &Transformation,
    g: &Transformation,
    resolve: R,
    meta: CompositionMetadata,
    id_of: I,
) -> Result<Transformation>
where
    R: Fn(&Subject) -> Option<Cid>,
    I: FnOnce(&Transformation) -> TransformationId,
{
    composability(f, g, resolve)?;
    meta.in_range()?;
    // ③. `max` and not `f.created_at`: either arrow may be the later one, and a check that read
    // only the first would admit a composite dated before the second.
    let at_least = f.created_at.0.max(g.created_at.0);
    if meta.created_at.0 < at_least {
        return Err(Error::CreatedAtBeforeParts {
            got: meta.created_at.0,
            at_least,
        });
    }

    let provisional = Transformation {
        id: PROVISIONAL_ID,
        intent_id: meta.intent_id,
        order: Transformation::with_order(f.order.max(g.order))?,
        subject: f.subject,
        target: g.target,
        delta: meta.delta,
        context: meta.context,
        actor: meta.actor,
        parents: vec![f.id, g.id],
        created_at: meta.created_at,
    };

    let id = id_of(&provisional);
    Ok(Transformation { id, ..provisional })
}

/// `identity x`: the arrow from a snapshot to itself (46 §2.1).
///
/// 46 §2.1 verbatim: `identity (x : ObjectSnapshot) (idOf : ObjectSnapshot → TransformationId) :=
/// { id := idOf x, order := 0, src := x, dst := x }`. In 41's vocabulary `src := x` is
/// `subject := Object(x.id)` and `dst := x` is `target := Some(x.digest)`, which is the reading
/// req/31 §7 wrote down as the interim and 46 §2's own text confirms.
///
/// Order 0 is below [`MAX_ORDER`] under every decision record that has ever set it, so the ceiling
/// cannot refuse an identity arrow. No parents: an identity is made of nothing.
///
/// # Why it returns a `Result` anyway (**E-M3-18**)
///
/// Until M4 this function was infallible, on the reading that 「returning a `Result` whose error arm
/// is unreachable would be a lie about the API」. **E-M3-13** (M3 hand 6) made that reading false:
/// the other two constructors began refusing a `created_at` below the epoch (①) and an all-zero
/// `intent_id` (②), both of which arrive in the same [`CompositionMetadata`] this function takes, so
/// the error arm stopped being unreachable and this became the one door of the three that admitted
/// what the other two refused. M3 hand 6 measured the gap rather than closing it, because E-M3-13
/// named two constructors and widening a ruling was not that hand's to do; req/66 §4 raised it and
/// `req/38_ERRATA_2026-08-07.md` §25 ruled:
///
/// > 「🔴**H-1(採用=erratum E-M3-18・実装は M4 冒頭必須 DoD)**: `identity` も `Result` 化し**全構成子
/// > で値域を閉じる**。infallible の根拠(error 腕到達不能)は D-6 で偽になった——unchecked door が 1 つ
/// > 残る限り「gx-core の Transformation は値域内」という不変条件は型で言えない」
///
/// The check is [`CompositionMetadata::in_range`], the same function the other two call, so a later
/// decision record that moves ① or ② moves all three doors at once (req/31 §7's rule for the order
/// ceiling, applied to the range). `crates/gx-core/tests/value_range_closure.rs` counts the doors
/// out of this crate's source and prints `UNCHECKED_DOORS`, and `compose_range.rs`'s pin -- which
/// held the old behaviour so that changing it would be a deliberate act -- is rewritten there
/// rather than deleted.
///
/// # Errors
/// [`Error::CreatedAtNegative`] and [`Error::IntentIdUnset`], for **E-M3-13**'s ① and ②. Not ③: it
/// compares a composite with the arrows it is made of, and an identity arrow is made of nothing.
///
/// Nothing here claims `compose(identity(x), f) == f`. The unit laws are Lean's to prove and they
/// cannot be proved while `composeId` is an axiom (`ASM-03-2`, 46 §2.1); M8 owns them.
///
/// # Why the id arrives the same way it does for [`compose`]
///
/// 46 §2.1 types `idOf` as `ObjectSnapshot → TransformationId`, and reading that literally is
/// what the first implementation did. `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §2 B-1 measured the
/// consequence: what is being identified is the *arrow*, not the snapshot, and 42 §1.3 defines a
/// `TransformationId` as the BLAKE3 of the `Transformation`'s own IdentityView -- which is not a
/// function of `x` alone, because it also carries `intent_id`, `delta`, `context` and `actor` out
/// of `meta`. A callback holding only `x` therefore cannot evaluate 42 §1.3 and has to invent a
/// substitute; two plausible substitutes produced two different ids for one identity arrow.
///
/// So the callback here is [`compose`]'s, exactly: it receives the arrow carrying an all-zero
/// provisional `id`, and since 42 §1.3 excludes `id` from the IdentityView, the CID of what it is
/// handed is the CID of what it returns into. `req/46D_AUDIT_RULING_2026-08-07.md` §1 F-1 is the
/// ruling; the Lean side is unaffected, because `idOf` is a parameter there too and a parameter's
/// domain is a modelling choice rather than a theorem (`ASM-03-2`).
///
/// The projection the callback should compute over lives in `gx-canon`, which this crate may not
/// name (A-1); `gx-canon/tests/identity_id.rs` is the worked example.
///
/// The snapshot-shortcut that reading cost is now a type error rather than a footgun:
///
/// ```compile_fail
/// use gx_core::{identity, ObjectSnapshot, TransformationId};
/// # fn go(x: &ObjectSnapshot, meta: gx_core::CompositionMetadata) {
/// // `id_of` is handed the arrow, not the snapshot, so `s.digest` does not resolve.
/// let _ = identity(x, meta, |s: &ObjectSnapshot| TransformationId(s.digest));
/// # }
/// ```
pub fn identity<I>(
    x: &ObjectSnapshot,
    meta: CompositionMetadata,
    id_of: I,
) -> Result<Transformation>
where
    I: FnOnce(&Transformation) -> TransformationId,
{
    meta.in_range()?;

    let provisional = Transformation {
        id: PROVISIONAL_ID,
        intent_id: meta.intent_id,
        order: 0,
        subject: Subject::Object(*x.id()),
        target: Some(*x.digest()),
        delta: meta.delta,
        context: meta.context,
        actor: meta.actor,
        parents: Vec::new(),
        created_at: meta.created_at,
    };

    let id = id_of(&provisional);
    Ok(Transformation { id, ..provisional })
}

impl Transformation {
    /// Build one. The only way in from outside this crate, and it goes through
    /// [`Transformation::with_order`].
    ///
    /// The five arguments the ten fields do not fit into are [`CompositionMetadata`]'s, which is
    /// the same grouping [`compose`] and [`identity`] already take -- so a caller who has the
    /// metadata for a composition has it for a hand-built arrow too, and there is one name for
    /// that set of fields rather than two.
    ///
    /// # Errors
    /// [`Error::OrderExceeded`] when `order` is above [`MAX_ORDER`]. Refusing at construction is
    /// what makes 「every place that sets an order comes through `with_order`」 a fact about the
    /// type instead of a convention (F-2, `req/46D_AUDIT_RULING_2026-08-07.md` §1).
    ///
    /// [`Error::CreatedAtNegative`] and [`Error::IntentIdUnset`] when the metadata is outside
    /// **E-M3-13**'s ① or ②. Not ③: see [`compose`], which is the only constructor that can
    /// evaluate it. The range is checked before the ceiling because it is a fact about the
    /// arguments and the ceiling is a fact about a decision record; a caller reading two refusals
    /// should get the one it can fix without consulting a DR first.
    pub fn new(
        id: TransformationId,
        order: u8,
        subject: Subject,
        target: Option<Cid>,
        parents: Vec<TransformationId>,
        meta: CompositionMetadata,
    ) -> Result<Self> {
        meta.in_range()?;
        Ok(Self {
            id,
            intent_id: meta.intent_id,
            order: Self::with_order(order)?,
            subject,
            target,
            delta: meta.delta,
            context: meta.context,
            actor: meta.actor,
            parents,
            created_at: meta.created_at,
        })
    }

    /// The order this transformation carries. 41 §3's field, read through the one accessor that
    /// exists for it.
    #[must_use]
    pub fn order(&self) -> u8 {
        self.order
    }

    /// Move the order, through the same check the constructor uses.
    ///
    /// A checked constructor beside an unchecked field write would be the F-2 hole one call
    /// later, so this is the only mutation path and it refuses rather than clamps. A refused
    /// write leaves the previous value in place.
    ///
    /// # Errors
    /// [`Error::OrderExceeded`] when `n` is above [`MAX_ORDER`].
    pub fn set_order(&mut self, n: u8) -> Result<()> {
        self.order = Self::with_order(n)?;
        Ok(())
    }

    /// The order-producing API of AC-003 and FR-003: admit `n` as an order, or refuse it.
    ///
    /// Returns the value rather than a wrapper because 41 §3 declares the field as a plain `u8`
    /// and this crate does not get to change that. Every place that sets an order is required to
    /// come through here -- req/31 §7 routes composition's `max(f.order, g.order)` through this
    /// same check -- which is what makes the ceiling a property of the crate rather than of one
    /// call site.
    ///
    /// # Errors
    /// [`Error::OrderExceeded`] when `n > MAX_ORDER`.
    ///
    /// # The claim above, as a check rather than as a sentence
    ///
    /// `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §3 W-2 built a `Transformation` holding
    /// `order = 250` with no compile error and no panic, because `order` was a plain `pub` field
    /// and a struct literal never reaches this function. 「every place that sets an order is
    /// required to come through here」 was therefore discipline, not a fact about the type. F-2
    /// (`req/46D_AUDIT_RULING_2026-08-07.md` §1) closes it; this is that reproduction, and it
    /// must not compile:
    ///
    /// ```compile_fail
    /// use gx_core::{
    ///     Actor, ChangeContext, Cid, DeltaRef, IntentId, ObjectId, Subject, SubstrateKind,
    ///     Timestamp, Transformation, TransformationId,
    /// };
    /// let bypass = Transformation {
    ///     id: TransformationId(Cid([0u8; 32])),
    ///     intent_id: IntentId(Cid([1u8; 32])),
    ///     order: 250, // MAX_ORDER is 2
    ///     subject: Subject::Object(ObjectId(Cid([2u8; 32]))),
    ///     target: None,
    ///     delta: DeltaRef { substrate: SubstrateKind::Fs, cid: Cid([3u8; 32]) },
    ///     context: ChangeContext::Time,
    ///     actor: Actor::Human { key: String::new() },
    ///     parents: Vec::new(),
    ///     created_at: Timestamp(0),
    /// };
    /// ```
    pub fn with_order(n: u8) -> Result<u8> {
        if n <= MAX_ORDER {
            Ok(n)
        } else {
            Err(Error::OrderExceeded {
                got: n,
                max: MAX_ORDER,
            })
        }
    }
}

/// The ten-field guard of AC-002, kept where the crate can still see all ten.
///
/// `crates/gx-core/tests/ac_002.rs` destructures a `Transformation` without `..`, so that an
/// eleventh field cannot be added without a test failing to compile. F-2
/// (`req/46D_AUDIT_RULING_2026-08-07.md` §1) made `order` private, and a test binary is a
/// separate crate, so that binary can no longer name every field. The guard is the same
/// mechanism moved one visibility boundary in; AC-002 keeps its ten assertions over the public
/// surface and points here for the exhaustiveness half.
#[cfg(test)]
mod field_set {
    use super::*;
    use crate::context::Actor;
    use crate::object::{ObjectId, SubstrateKind};

    #[test]
    fn the_field_set_is_exactly_ten() {
        let t = Transformation {
            id: TransformationId(Cid([0u8; 32])),
            intent_id: IntentId(Cid([1u8; 32])),
            order: 2,
            subject: Subject::Object(ObjectId(Cid([2u8; 32]))),
            target: Some(Cid([3u8; 32])),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: Cid([4u8; 32]),
            },
            context: ChangeContext::Policy,
            actor: Actor::Human {
                key: "k".to_string(),
            },
            parents: vec![TransformationId(Cid([5u8; 32]))],
            created_at: Timestamp(7),
        };

        // No `..`: an eleventh field stops this compiling, which is the whole of the check.
        let Transformation {
            id,
            intent_id,
            order,
            subject,
            target,
            delta,
            context,
            actor,
            parents,
            created_at,
        } = t;

        assert_eq!(id, TransformationId(Cid([0u8; 32])));
        assert_eq!(intent_id, IntentId(Cid([1u8; 32])));
        assert_eq!(order, 2);
        assert!(matches!(subject, Subject::Object(_)));
        assert_eq!(target, Some(Cid([3u8; 32])));
        assert_eq!(delta.substrate, SubstrateKind::Fs);
        assert_eq!(context, ChangeContext::Policy);
        assert!(matches!(actor, Actor::Human { .. }));
        assert_eq!(parents.len(), 1);
        assert_eq!(created_at, Timestamp(7));
    }
}

/// Kani harness 3 of 3 (46 §4.2, row 「`gx-core` hot path」).
///
/// 46 §4.2 lists the row as `compose`, `identity` and `Fingerprint` comparison. `Fingerprint` is
/// a gx-substrate type that M1 does not define, and implementing it here to satisfy a Kani row
/// would be the cross-milestone先行実装 `B-04` forbids. A-2 (`req/38_ERRATA_2026-08-07.md` §1)
/// rules the `Fingerprint` third deferred to M4 and M1's Kani scope limited to `compose` and
/// `identity`; **this module is that reduced third harness, and the omission is recorded here
/// rather than only in a report** (ASM-10-2).
///
/// The module lives inside this file rather than in a new one because 41 §2 fixes gx-core's
/// module list at seven (req/31 §2).
///
/// # The bounds, stated rather than implied
///
/// 51 §5 asks a bounded model check to name its bounds. Everything below is bounded by
/// construction: `order` is symbolic over the whole of `u8` (all 256 values, no bound), the
/// digests are symbolic over all 2^256 values, and every heap value is fixed -- one empty string
/// per string field, `parents` of length 0 going in and 2 coming out. Composition performs no
/// arithmetic beyond `u8::max`, so "no integer overflow" is a claim about a program with no
/// integer operations in it: true, and worth pinning, since a later `order + 1` would break it.
#[cfg(kani)]
mod verification {
    use super::*;
    use crate::context::Actor;
    use crate::object::{ObjectId, ObjectSnapshot, ReprKind, SubstrateKind};

    fn any_cid() -> Cid {
        Cid(kani::any())
    }

    fn metadata() -> CompositionMetadata {
        CompositionMetadata {
            // Not the all-zero id: **E-M3-13** ② refuses it, and a harness whose metadata is
            // refused would prove its claims about an `Err` arm the input never leaves. The value
            // is fixed rather than symbolic for the reason the bounds note gives -- what is
            // symbolic here is `order` and the digests.
            intent_id: IntentId(Cid([9u8; 32])),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: Cid([1u8; 32]),
            },
            context: ChangeContext::Time,
            actor: Actor::Human { key: String::new() },
            created_at: Timestamp(0),
        }
    }

    fn arrow(order: u8, subject: Subject, target: Option<Cid>) -> Transformation {
        let meta = metadata();
        Transformation {
            id: TransformationId(Cid([2u8; 32])),
            intent_id: meta.intent_id,
            order,
            subject,
            target,
            delta: meta.delta,
            context: meta.context,
            actor: meta.actor,
            parents: Vec::new(),
            created_at: meta.created_at,
        }
    }

    /// `compose` returns for every input it can be given: no panic, no overflow, and never a
    /// composite above the ceiling.
    #[kani::proof]
    #[kani::unwind(64)]
    fn compose_is_total_and_respects_the_ceiling() {
        let f_order: u8 = kani::any();
        let g_order: u8 = kani::any();
        let meeting: Cid = any_cid();
        let f_target: Option<Cid> = if kani::any() { Some(any_cid()) } else { None };

        let subject_of_g = Subject::Object(ObjectId(any_cid()));
        let f = arrow(f_order, Subject::Object(ObjectId(any_cid())), f_target);
        let g = arrow(g_order, subject_of_g, Some(any_cid()));

        let out = compose(
            &f,
            &g,
            |_: &Subject| Some(meeting),
            metadata(),
            |_: &Transformation| TransformationId(Cid([3u8; 32])),
        );

        match out {
            Ok(t) => {
                // The Given held, so the shape is the one 46 §2.1 fixes.
                assert!(t.order <= MAX_ORDER);
                assert!(t.order == f_order || t.order == g_order);
                assert!(t.parents.len() == 2);
                assert!(t.subject == f.subject);
                assert!(t.target == g.target);
            }
            Err(_) => {
                // Refusal is a value, not a panic (41 §6). Reaching this arm is the point.
            }
        }
    }

    /// The composability predicate is total too, and agrees with `compose`'s own decision.
    #[kani::proof]
    #[kani::unwind(64)]
    fn composable_agrees_with_compose_on_every_input() {
        let meeting: Cid = any_cid();
        let f_target: Option<Cid> = if kani::any() { Some(any_cid()) } else { None };
        let f = arrow(0, Subject::Object(ObjectId(any_cid())), f_target);
        let g = arrow(0, Subject::Object(ObjectId(any_cid())), Some(any_cid()));

        let resolve = |_: &Subject| Some(meeting);
        let predicate = composable(&f, &g, resolve);
        let composed = compose(&f, &g, resolve, metadata(), |_: &Transformation| {
            TransformationId(Cid([3u8; 32]))
        });

        // A predicate that says yes while the function says no would let a caller build a Given
        // the function then refuses -- or worse, the other way round.
        assert!(
            predicate == composed.is_ok() || matches!(composed, Err(Error::OrderExceeded { .. }))
        );
    }

    /// `identity` is total: every snapshot has one, and it has the shape 46 §2.1 gives it.
    ///
    /// Total in the sense the other two harnesses use -- it returns for every input, a refusal
    /// being a value and not a panic (41 §6). Since **E-M3-18** the return is a `Result`, and the
    /// `Err` arm is what this harness's fixed in-range `metadata()` is chosen to avoid: what is
    /// proved is that the shape holds wherever the range does.
    #[kani::proof]
    #[kani::unwind(64)]
    fn identity_is_total() {
        let digest = any_cid();
        let x = ObjectSnapshot::new(
            ObjectId(any_cid()),
            SubstrateKind::Fs,
            String::new(),
            digest,
            ReprKind::Bytes,
        );

        match identity(&x, metadata(), |_: &Transformation| {
            TransformationId(Cid([4u8; 32]))
        }) {
            Ok(id) => {
                assert!(id.order == 0);
                assert!(id.target == Some(digest));
                assert!(id.parents.is_empty());
            }
            Err(_) => {
                // Refusal is a value, not a panic (41 §6), as in the two harnesses above.
            }
        }
    }
}
