//! AC-060 (FR-055) — the opt-in Lyapunov law `m(Y) ≤ m(X) + θ(f)`.
//!
//! AC-060 逐語: 「Given: opt-inで`m(Y) ≤ m(X)+θ(f)`則を有効化した`ObjectMeasure`実装と、
//! 無効化（opt-out）した実装。When: proptestを実行。Then: 有効化した実装のみ当該proptestの対象に
//! なり法則を満たす。無効化した実装ではテストがスキップされる（設定フラグで切替を確認）。」
//!
//! # What "opt-in" is, mechanically
//!
//! An implementation opts in by implementing [`gx_core::Lyapunov`]. Nothing else changes: the
//! measure is the same function, and `ObjectMeasure` still has the single method 41 §3 gives it.
//! The alternative -- a `bool` in this file's registry -- was rejected because a boolean written
//! by the test can disagree with the implementation it labels, and then the skip proves nothing.
//! `conformance::lyapunov_registry` therefore holds two opted-out rows, one of which measures
//! exactly what the opted-in row measures: the only thing separating them is the trait.
//!
//! # Why a skip is printed rather than silently not run
//!
//! req/29 §4's fail-open rule: a suite must say what it did not look at. A skipped row is
//! reported by name on stdout (`AC060_SKIPPED=`), so "the law was never checked" and "the law
//! held" cannot look the same from outside.

mod conformance;

use conformance::{lyapunov_registry, Entry, Growth, Size, SizeOptedOut, World};
use gx_core::{identity, MorphismMeasure, ObjectMeasure};
use proptest::prelude::*;

proptest! {
    /// The law, over every row that opted in.
    #[test]
    fn ac_060_the_opted_in_measure_obeys_the_lyapunov_law(sx in 0usize..64, sy in 0usize..64) {
        let world = World::new(sx, sy, sy);
        let mut checked = 0usize;

        for entry in lyapunov_registry(&world) {
            let Entry::OptedIn { object, morphism, .. } = &entry else {
                continue;
            };
            let m_x = object.measure(&world.x);
            let m_y = object.measure(&world.y);
            let theta = morphism.measure(&world.f);
            prop_assert!(
                m_y <= m_x + theta,
                "m(Y)={m_y} exceeded m(X)+θ(f)={m_x}+{theta} for sizes {sx}/{sy}"
            );
            checked += 1;
        }

        prop_assert!(checked > 0, "no row opted in, so the property tested nothing");
    }

    /// The identity arrow is the law's boundary: `m(x) <= m(x) + θ(identity x)`, which holds
    /// exactly when θ of an identity is not negative. A measure that paid a negative cost for
    /// doing nothing would break the law at its easiest point.
    #[test]
    fn ac_060_the_law_holds_at_the_identity_arrow(sx in 0usize..64) {
        let world = World::new(sx, sx, sx);
        let m = Size;
        let theta = Growth::new(&world);

        // The id is injected (46 §2.1's `idOf`) and nothing this property measures reads it, so
        // a constant is the honest choice here; the canonical derivation is gx-canon's (F-1,
        // req/46D §1, `gx-canon/tests/identity_id.rs`).
        let id = identity(&world.x, conformance::metadata(5), |_| {
            gx_core::TransformationId(conformance::cid(5))
        })
        .expect("the fixture metadata is inside E-M3-13's range");

        let m_x = m.measure(&world.x);
        let cost = theta.measure(&id);
        prop_assert!(cost >= 0.0, "an identity cost {cost}, which is less than nothing");
        prop_assert!(m_x <= m_x + cost);
    }
}

/// The switch: flipping opt-in changes which rows are subject to the law, and the skipped ones
/// are named.
#[test]
fn ac_060_opting_out_removes_a_row_from_the_property_and_says_so() {
    let world = World::new(1, 40, 40);
    let registry = lyapunov_registry(&world);

    let mut checked: Vec<&'static str> = Vec::new();
    let mut skipped: Vec<&'static str> = Vec::new();
    for entry in &registry {
        match entry {
            Entry::OptedIn { .. } => checked.push(entry.name()),
            Entry::OptedOut { .. } => skipped.push(entry.name()),
        }
    }

    println!("AC060_ROWS={}", registry.len());
    println!("AC060_CHECKED={}", checked.join(","));
    println!("AC060_SKIPPED={}", skipped.join(","));

    assert_eq!(checked, vec!["size+growth"]);
    assert_eq!(skipped, vec!["size+free", "size+growth-not-declared"]);
}

/// The skip is not a favour: one opted-out row would fail the law if it were subject to it.
///
/// Without this, "the opted-in rows all pass" is consistent with a law so weak that every
/// implementation passes, and the opt-in switch would be decoration.
#[test]
fn ac_060_an_opted_out_measure_would_have_failed_the_law() {
    let world = World::new(1, 40, 40);
    let m = SizeOptedOut;
    let theta = conformance::Free;

    let m_x = m.measure(&world.x);
    let m_y = m.measure(&world.y);
    let cost = theta.measure(&world.f);

    println!(
        "AC060_OPTED_OUT_WOULD_FAIL=m(Y)={m_y} > m(X)+theta={}",
        m_x + cost
    );
    assert!(
        m_y > m_x + cost,
        "the opted-out row satisfies the law anyway, so opting out costs nothing and the \
         switch is untested"
    );
}

/// `Lyapunov` is a claim about an implementation, not about a value: the opted-in and opted-out
/// measures here return the same numbers, so nothing but the trait separates them.
#[test]
fn ac_060_the_two_measures_differ_only_in_the_opt_in() {
    let world = World::new(7, 9, 11);
    for snapshot in [&world.x, &world.y, &world.z] {
        assert_eq!(Size.measure(snapshot), SizeOptedOut.measure(snapshot));
    }
}
