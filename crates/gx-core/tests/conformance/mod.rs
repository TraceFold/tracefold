//! The measure implementations the law tests are run against, and the little world they measure.
//!
//! AC-059 asks for the law-breaking implementation to live in `conformance/`
//! (「法則に違反する意図的実装（例: θを2倍で返す壊れた実装）を conformance/ に負テストとして用意し」).
//! 46 §3.1's `conformance/` is the JSONL vector tree of the Lean differential harness, which is
//! M8 work and has no Rust crate yet; this directory is the M1-sized reading of the same word --
//! a named place where the deliberately wrong implementations live, apart from the tests that
//! reject them. `tests/support/` in gx-canon is the same convention.
//!
//! # Why the measures are defined over a world rather than over a value
//!
//! `MorphismMeasure::measure` sees a `Transformation` and nothing else (41 §3), and a
//! `Transformation` carries an `ObjectId` and a digest, not the snapshots behind them. A cost
//! that is to be compared against `ObjectMeasure` values therefore needs the same resolver the
//! composability check needs, so [`World`] holds it and the measures borrow it.

#![allow(dead_code)] // each test binary uses a different subset of this module

use gx_core::{
    Actor, ChangeContext, Cid, DeltaRef, IntentId, Lyapunov, MorphismMeasure, ObjectId,
    ObjectMeasure, ObjectSnapshot, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId,
};

/// A digest that depends on the seed and on nothing else, so two runs build the same world.
#[must_use]
pub fn cid(seed: u8) -> Cid {
    let mut raw = [0u8; 32];
    for (i, slot) in raw.iter_mut().enumerate() {
        *slot = seed
            .wrapping_mul(31)
            .wrapping_add(u8::try_from(i).expect("i < 32"));
    }
    Cid(raw)
}

/// An object snapshot whose measured size is `size` and whose identity is `seed`.
///
/// The locator carries the size because [`Size`] measures the locator: a snapshot holds no
/// content (ASM-9), so the only thing in it that can stand in for "how big is this object" is a
/// field the test controls.
#[must_use]
pub fn snapshot(seed: u8, size: usize) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(seed)),
        SubstrateKind::Fs,
        "x".repeat(size),
        cid(seed.wrapping_add(128)),
        ReprKind::Bytes,
    )
}

/// The metadata `compose` and `identity` cannot invent (A-7 erratum, `req/38` §1).
#[must_use]
pub fn metadata(seed: u8) -> gx_core::CompositionMetadata {
    gx_core::CompositionMetadata {
        intent_id: IntentId(cid(seed.wrapping_add(7))),
        delta: DeltaRef {
            substrate: SubstrateKind::Fs,
            cid: cid(seed.wrapping_add(11)),
        },
        context: ChangeContext::Evidence,
        actor: Actor::Human {
            key: "conformance".to_string(),
        },
        created_at: Timestamp(0),
    }
}

/// A transformation from `from` to `to`, of the given order, with no parents.
///
/// Through `Transformation::new`, which is the only public door since F-2
/// (`req/46D_AUDIT_RULING_2026-08-07.md` §1) made `order` private. An order above the ceiling is
/// a setup failure here and panics; the tests that need such a value ask for
/// [`arrow_above_the_ceiling`] by name, so a malformed fixture is never built by accident.
#[must_use]
pub fn arrow(seed: u8, from: &ObjectSnapshot, to: &ObjectSnapshot, order: u8) -> Transformation {
    Transformation::new(
        TransformationId(cid(seed.wrapping_add(64))),
        order,
        Subject::Object(*from.id()),
        Some(*to.digest()),
        Vec::new(),
        metadata(seed),
    )
    .unwrap_or_else(|e| panic!("arrow(order={order}) is not a value this crate admits: {e}"))
}

/// An arrow whose order is above [`gx_core::MAX_ORDER`] -- which no constructor will build.
///
/// F-2 closed the struct-literal route, so the remaining way such a value reaches a program is
/// from outside it: `serde`'s derive fills the field from whatever the bytes said and never calls
/// `with_order`. That is why `compose` keeps a check of its own rather than trusting its
/// arguments, and this is how a test can still hand it one. The value is round-tripped through
/// JSON (gx-core's dev-dependency; DAG-CBOR is gx-canon's and unnameable here, A-1).
#[must_use]
pub fn arrow_above_the_ceiling(
    seed: u8,
    from: &ObjectSnapshot,
    to: &ObjectSnapshot,
    order: u8,
) -> Transformation {
    assert!(
        order > gx_core::MAX_ORDER,
        "use `arrow` for an order the crate admits"
    );
    let well_formed = arrow(seed, from, to, 0);
    let mut document =
        serde_json::to_value(&well_formed).expect("a Transformation has a JSON form");
    document["order"] = serde_json::Value::from(order);
    serde_json::from_value(document).expect("serde fills the field without consulting with_order")
}

/// Three snapshots and the two arrows between them: `x --f--> y --g--> z`.
///
/// Small on purpose. AC-059 quantifies over composable pairs, and a composable pair is exactly
/// this shape; generating a larger graph would add nothing the law can see.
pub struct World {
    pub x: ObjectSnapshot,
    pub y: ObjectSnapshot,
    pub z: ObjectSnapshot,
    pub f: Transformation,
    pub g: Transformation,
}

impl World {
    /// Sizes drive [`Size`]; seeds keep the three snapshots distinct.
    #[must_use]
    pub fn new(sx: usize, sy: usize, sz: usize) -> Self {
        let x = snapshot(1, sx);
        let y = snapshot(2, sy);
        let z = snapshot(3, sz);
        let f = arrow(10, &x, &y, 0);
        let g = arrow(20, &y, &z, 0);
        Self { x, y, z, f, g }
    }

    /// The resolver `compose` takes: which digest does a subject denote?
    ///
    /// gx-core may not do I/O (41 §6), so composability cannot be decided without one of these.
    /// Here it is a match over three snapshots; in the engine it is a store lookup.
    pub fn resolve(&self) -> impl Fn(&Subject) -> Option<Cid> + '_ {
        move |subject| match subject {
            Subject::Object(id) => [&self.x, &self.y, &self.z]
                .into_iter()
                .find(|s| s.id() == id)
                .map(|s| *s.digest()),
            // Order >= 1 subjects are not part of AC-059's Given, and answering for one we did
            // not build would be an invention rather than a resolution.
            Subject::Transformation(_) => None,
        }
    }

    /// The snapshot a subject names, for the measures below.
    #[must_use]
    pub fn snapshot_of_subject(&self, subject: &Subject) -> Option<&ObjectSnapshot> {
        match subject {
            Subject::Object(id) => [&self.x, &self.y, &self.z]
                .into_iter()
                .find(|s| s.id() == id),
            Subject::Transformation(_) => None,
        }
    }

    /// The snapshot a target digest names.
    #[must_use]
    pub fn snapshot_of_digest(&self, digest: &Cid) -> Option<&ObjectSnapshot> {
        [&self.x, &self.y, &self.z]
            .into_iter()
            .find(|s| s.digest() == digest)
    }
}

// ---------------------------------------------------------------------------
// ObjectMeasure
// ---------------------------------------------------------------------------

/// `m(X)` = how long the locator is. Any total function of a snapshot would do; this one is
/// monotone in a quantity the test controls, so the Lyapunov law of FR-055 has something to bite.
pub struct Size;

impl ObjectMeasure for Size {
    fn measure(&self, x: &ObjectSnapshot) -> f64 {
        // `as` rather than a fallible conversion: 41 §6 forbids panics, and a locator long enough
        // to lose f64 precision is not reachable from this test's strategies.
        x.locator().len() as f64
    }
}

// ---------------------------------------------------------------------------
// MorphismMeasure -- the lawful one
// ---------------------------------------------------------------------------

/// θ(f) = how much the object had to grow, never less than zero.
///
/// Subadditive by construction, and the proof is one line of arithmetic:
/// `max(0, c-a) <= max(0, b-a) + max(0, c-b)` for all reals. It is also the measure that makes
/// FR-055's `m(Y) <= m(X) + θ(f)` hold with equality whenever the object grew, which is what
/// stops AC-060 from being satisfied vacuously by a cost that is simply large.
pub struct Growth<'w> {
    pub world: &'w World,
    pub size: Size,
}

impl<'w> Growth<'w> {
    #[must_use]
    pub fn new(world: &'w World) -> Self {
        Self { world, size: Size }
    }

    /// `m` at both ends of an arrow, when both ends are in the world.
    #[must_use]
    pub fn endpoints(&self, f: &Transformation) -> Option<(f64, f64)> {
        let from = self.world.snapshot_of_subject(&f.subject)?;
        let to = self.world.snapshot_of_digest(&f.target?)?;
        Some((self.size.measure(from), self.size.measure(to)))
    }
}

impl MorphismMeasure for Growth<'_> {
    fn measure(&self, f: &Transformation) -> f64 {
        match self.endpoints(f) {
            Some((from, to)) => (to - from).max(0.0),
            // An arrow whose ends this world cannot resolve costs nothing to describe. The law
            // still holds for it, and inventing a number would be the sort of guess req/26 §3
            // rules out.
            None => 0.0,
        }
    }
}

/// θ ≡ 0. Subadditive (0 <= 0 + 0), so AC-059 admits it; useless for FR-055, which is why
/// AC-060 has it opted out.
pub struct Free;

impl MorphismMeasure for Free {
    fn measure(&self, _f: &Transformation) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// MorphismMeasure -- the deliberately wrong one (AC-059's negative side)
// ---------------------------------------------------------------------------

/// θ(f) = 1 + 2·|parents|: a cost that charges *extra* for having been composed.
///
/// AC-059's example is 「θを2倍で返す壊れた実装」. Doubling a subadditive measure is still
/// subadditive (`2c <= 2a + 2b` whenever `c <= a + b`), so a literal doubling would pass the
/// property and prove nothing. What breaks the law is charging per composition edge: two atomic
/// arrows cost 1 each, their composite carries two parents and costs 5, and 5 > 2.
///
/// This is the discriminating case. Without it, a property that never fails cannot be told apart
/// from a property that cannot fail.
pub struct PerEdgeInflating;

impl MorphismMeasure for PerEdgeInflating {
    fn measure(&self, f: &Transformation) -> f64 {
        1.0 + 2.0 * f.parents.len() as f64
    }
}

// ---------------------------------------------------------------------------
// AC-060's opt-in flag
// ---------------------------------------------------------------------------

/// `Size` opts in to FR-055 (see [`Lyapunov`]). Whether it is *right* to opt in is what AC-060's
/// property checks; the trait only records the claim.
impl Lyapunov for Size {}

/// The same measurement, without the claim.
///
/// Byte-identical behaviour to [`Size`] and no `impl Lyapunov`. That is the whole difference, and
/// keeping the numbers identical is what makes AC-060's skip observable: the opted-out entry is
/// skipped because it did not opt in, not because it measures something the law happens to admit.
pub struct SizeOptedOut;

impl ObjectMeasure for SizeOptedOut {
    fn measure(&self, x: &ObjectSnapshot) -> f64 {
        x.locator().len() as f64
    }
}

/// One row of AC-060's registry: a measure pair, and the opt-in state carried in the type.
///
/// FR-055 is opt-in (「opt-inで`m(Y) ≤ m(X)+θ(f)`則を有効化した`ObjectMeasure`実装と、
/// 無効化（opt-out）した実装」). The switch is [`Lyapunov`], a gx-core marker trait, rather than a
/// `bool` written here: a hand-set boolean can disagree with the implementation it labels, and a
/// registry that lies about which rows opted in would make the skip meaningless.
pub enum Entry<'w> {
    /// Opted in: subject to the law.
    OptedIn {
        name: &'static str,
        object: Box<dyn Lyapunov + 'w>,
        morphism: Box<dyn MorphismMeasure + 'w>,
    },
    /// Opted out: skipped, and recorded as skipped.
    OptedOut {
        name: &'static str,
        object: Box<dyn ObjectMeasure + 'w>,
        morphism: Box<dyn MorphismMeasure + 'w>,
    },
}

impl Entry<'_> {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Entry::OptedIn { name, .. } | Entry::OptedOut { name, .. } => name,
        }
    }
}

/// The registry AC-060 runs over.
///
/// Three rows on purpose: one that opted in and obeys, one that opted out and would fail if it
/// had not (θ ≡ 0 cannot pay for growth), and one that opted out while measuring exactly what the
/// first row measures -- so the skip is visibly a function of the flag alone.
#[must_use]
pub fn lyapunov_registry(world: &World) -> Vec<Entry<'_>> {
    vec![
        Entry::OptedIn {
            name: "size+growth",
            object: Box::new(Size),
            morphism: Box::new(Growth::new(world)),
        },
        Entry::OptedOut {
            name: "size+free",
            object: Box::new(SizeOptedOut),
            morphism: Box::new(Free),
        },
        Entry::OptedOut {
            name: "size+growth-not-declared",
            object: Box::new(SizeOptedOut),
            morphism: Box::new(Growth::new(world)),
        },
    ]
}
