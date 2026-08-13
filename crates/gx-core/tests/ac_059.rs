//! AC-059 (FR-054) — θ(g∘f) ≤ θ(g) + θ(f), and a property that can actually fail.
//!
//! AC-059 逐語: 「Given: 合成可能な2つのTransformation f, g（`f.target = g.subjectに対応するCid`）を
//! ランダム生成し、任意の`MorphismMeasure`実装Θを与える。When: `θ(compose(g,f))`と`θ(g)+θ(f)`を
//! 計算する。Then: 常に`θ(g∘f) ≤ θ(g) + θ(f)`が成立する。法則に違反する意図的実装（例: θを2倍で
//! 返す壊れた実装）をconformance/に負テストとして用意し、proptestがそれを棄却する（テスト失敗する）
//! ことも確認する。」
//!
//! # Two readings that had to be settled before the first line
//!
//! **「任意の`MorphismMeasure`実装Θ」.** A property test cannot quantify over all implementations
//! of a trait; nothing can. What is testable is a registry -- the implementations in
//! `conformance/` -- plus the negative case that shows the property discriminates. That is the
//! same shape 46 §4.1 gives the law ("Measure法則: θ(g∘f) ≤ θ(g) + θ(f) | gx-core"), and it is
//! why the law is documented on `MorphismMeasure` and enforced here rather than in the type.
//!
//! **`compose(g,f)` vs `compose(f,g)`.** AC-059 writes the composite `compose(g,f)` in the
//! mathematician's order and 46 §2.1 writes the same composite as `compose f g h` with
//! `composable f g := f.dst = g.src`. The Lean file is the normative definition (46 §2.1 is the
//! spec of the function; the AC is prose about its measure), so the Rust argument order follows
//! Lean: `compose(f, g)` is `g ∘ f`, first the arrow that runs first. The composite this file
//! measures is the one AC-059 calls `compose(g,f)`.

mod conformance;

use conformance::{Growth, PerEdgeInflating, World};
use gx_core::{composable, compose, MorphismMeasure};
use proptest::prelude::*;

/// Sizes small enough to read in a shrink report and wide enough to make growth and shrinkage
/// both reachable.
fn sizes() -> impl Strategy<Value = (usize, usize, usize)> {
    (0usize..64, 0usize..64, 0usize..64)
}

proptest! {
    /// The law itself, on the lawful measure.
    #[test]
    fn ac_059_theta_is_subadditive_under_composition((sx, sy, sz) in sizes()) {
        let world = World::new(sx, sy, sz);
        let theta = Growth::new(&world);

        prop_assert!(
            composable(&world.f, &world.g, world.resolve()),
            "the Given of AC-059 is a composable pair; this world does not supply one"
        );

        let gf = compose(
            &world.f,
            &world.g,
            world.resolve(),
            conformance::metadata(99),
            |_provisional| gx_core::TransformationId(conformance::cid(200)),
        )
        .expect("a composable pair composes");

        let composite = theta.measure(&gf);
        let parts = theta.measure(&world.g) + theta.measure(&world.f);
        prop_assert!(
            composite <= parts,
            "θ(g∘f)={composite} exceeded θ(g)+θ(f)={parts} for sizes {sx}/{sy}/{sz}"
        );
    }

    /// The negative side, and the whole reason the positive side means anything.
    ///
    /// `PerEdgeInflating` charges per composition edge, so the composite costs 5 where its two
    /// parts cost 1 each. If this ever stops failing the law, the property above has stopped
    /// being able to fail.
    #[test]
    fn ac_059_the_property_rejects_a_law_breaking_measure((sx, sy, sz) in sizes()) {
        let world = World::new(sx, sy, sz);
        let theta = PerEdgeInflating;

        let gf = compose(
            &world.f,
            &world.g,
            world.resolve(),
            conformance::metadata(99),
            |_provisional| gx_core::TransformationId(conformance::cid(200)),
        )
        .expect("a composable pair composes");

        let composite = theta.measure(&gf);
        let parts = theta.measure(&world.g) + theta.measure(&world.f);
        prop_assert!(
            composite > parts,
            "the law-breaking measure passed the law (θ(g∘f)={composite}, θ(g)+θ(f)={parts}); \
             the negative case has gone vacuous and AC-059's positive case proves nothing"
        );
    }
}

/// Non-vacuity of the lawful case: the numbers are not all zero.
///
/// A subadditivity test over a measure that returns 0 everywhere passes and says nothing. This
/// pins one world in which every term is positive and the inequality is tight.
#[test]
fn ac_059_the_lawful_measure_is_not_the_zero_measure() {
    let world = World::new(1, 5, 20);
    let theta = Growth::new(&world);

    let gf = compose(
        &world.f,
        &world.g,
        world.resolve(),
        conformance::metadata(99),
        |_provisional| gx_core::TransformationId(conformance::cid(200)),
    )
    .expect("composable");

    assert_eq!(theta.measure(&world.f), 4.0, "1 -> 5");
    assert_eq!(theta.measure(&world.g), 15.0, "5 -> 20");
    assert_eq!(theta.measure(&gf), 19.0, "1 -> 20, and 19 <= 4 + 15");
}

/// A pair that is not composable does not get composed.
///
/// AC-059's Given is 「合成可能な2つの Transformation」, so the property says nothing about the
/// other case -- but the other case has to be refused rather than silently composed, or the
/// Given would be unenforceable and the law would be measured over arrows that do not connect.
#[test]
fn ac_059_a_non_composable_pair_is_refused() {
    let world = World::new(3, 4, 5);
    // g runs from y; f' claims to end at z, so f'.target is not what g.subject denotes.
    let mismatched = conformance::arrow(30, &world.x, &world.z, 0);

    assert!(!composable(&mismatched, &world.g, world.resolve()));
    assert!(compose(
        &mismatched,
        &world.g,
        world.resolve(),
        conformance::metadata(99),
        |_provisional| gx_core::TransformationId(conformance::cid(200)),
    )
    .is_err());
}
