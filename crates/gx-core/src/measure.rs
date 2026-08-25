// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Observables over objects and over transformations.
//!
//! Spec: 41 §3 for both traits and the laws attached to them, 42 §1.3-5 for why their values are
//! not part of anything's identity.

use crate::object::ObjectSnapshot;
use crate::transformation::Transformation;

/// A number read off an object (P-10, 12 F0).
///
/// The signature is 41 §3 verbatim. Being a trait rather than a field is the point: the core does
/// not know what is worth measuring, and a measure is supplied from outside and swapped without
/// touching the value being measured.
pub trait ObjectMeasure {
    /// The number this measure assigns to `x`. Pure by 41 §6 (no I/O in this crate's traits),
    /// and never part of `x`'s identity (42 §1.3-5): measuring twice must not mint two objects.
    fn measure(&self, x: &ObjectSnapshot) -> f64;
}

/// A cost read off a transformation -- Θ in 41 §3.
///
/// Implementations are required to satisfy `θ(g∘f) <= θ(g) + θ(f)` (subadditivity, FR-054); the
/// companion law `m(Y) <= m(X) + θ(f)` is opt-in (FR-055). Neither is enforceable here: both need
/// `compose`, and the property tests that reject a law-breaking implementation are AC-059, in
/// step 5 of req/31 §9. Stating the laws in the trait's documentation and testing them elsewhere
/// is the honest arrangement -- a `f64` cannot be constrained by a Rust type.
///
/// The values never enter a CID (42 §1.3-5). Measuring the same transformation twice, or with a
/// different measure, must not give it a second identity.
pub trait MorphismMeasure {
    /// The cost θ(f) this measure assigns to `f`. Implementations owe FR-054's subadditivity
    /// (see the trait doc); the value stays outside every CID (42 §1.3-5).
    fn measure(&self, f: &Transformation) -> f64;
}

/// The opt-in of FR-055: an `ObjectMeasure` that declares itself a Lyapunov function.
///
/// FR-055's law `m(Y) <= m(X) + θ(f)` is opt-in -- AC-060 says so twice
/// ("an `ObjectMeasure` implementation that has opted in ... and one that has opted out"; sem:
/// SEM-gx-core-072) -- and something has
/// to carry the opt-in. 41 §3 fixes `ObjectMeasure` at one method, so the switch is a separate
/// marker trait rather than a second method on it: implementing this one adds no behaviour and
/// changes no signature, it only records the claim.
///
/// A marker rather than a `fn is_lyapunov() -> bool` because the claim belongs to the
/// implementation and should not be answerable differently on two calls. AC-060's
/// "confirm the switch by a configuration flag" (sem: SEM-gx-core-073) is satisfied by the presence
/// or absence of this impl, which the
/// property test partitions on.
///
/// Implementing this trait is a claim, not a proof. Whether a given measure really is a Lyapunov
/// function for a given cost is what AC-060's property checks, and nothing here enforces it --
/// the same arrangement as [`MorphismMeasure`]'s subadditivity, for the same reason: an `f64`
/// cannot be constrained by a Rust type.
///
/// The spec does not say how the opt-in should be spelled; this trait is gx's answer and is
/// reported as an erratum candidate in `req/45_M1_HAND5_REPORT_2026-08-07.md`.
pub trait Lyapunov: ObjectMeasure {}
