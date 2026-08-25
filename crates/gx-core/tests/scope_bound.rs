// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **M4H1-2**: the bound on a fingerprint scope, declared once and refused at construction.
//!
//! req/38 §29, verbatim: "**M4H1-2, adopted (a); the window is hand 4**: gx-core too gets a bound on
//! the scope string plus digesting when it is exceeded (the same shape as gx-gate's
//! M3-21/ASM-60-4, 1024 bytes). It goes in **in the same window as hand 4, where the scope
//! generation rule (ASM-69-1) materialises** (generation and bound are not decided in separate
//! hands)" (quoted in SEM-gx-core-193).
//!
//! # The half that is here and the half that is not
//!
//! The ruling names two things and this crate can only hold one of them. A-1 keeps every digest in
//! gx-canon and 41 §2 keeps this crate's dependencies at "serde, thiserror and not much more", so
//! "digesting when exceeded" (sem: SEM-gx-core-194) cannot happen here: there is no hash to reach.
//! What is here is the **bound and the refusal**, and the elision lives one layer up in
//! [`gx_substrate::elide_scope`], where gx-canon is in scope.
//!
//! That is E-M2-1's split ("types down, computation up"; sem: SEM-gx-core-195) applied to the same
//! type E-M4-1 already split for
//! the same reason -- the `Fingerprint` type is here and its scope arithmetic is in gx-substrate. The
//! consequence is the useful one: because the refusal is at the constructor, **no** `Fingerprint`
//! anywhere in the workspace can carry an unbounded scope, whichever adapter built it. An elision
//! helper alone would have left a door (E-M3-18's word for it) open beside it.
//!
//! M4 hand 4 raises the divergence from the ruling's literal "the digesting goes in gx-core" as a
//! filed item in `req/73` §2 rather than deciding it here (sem: SEM-gx-core-196).

use gx_core::{Cid, Error, Fingerprint, SubstrateKind, ERROR_KINDS, MAX_SCOPE_BYTES};

fn scope_of(bytes: usize) -> String {
    "s".repeat(bytes)
}

fn build(scope: String) -> gx_core::Result<Fingerprint> {
    Fingerprint::new(SubstrateKind::Fs, scope, Cid([7u8; 32]))
}

/// The value, and that it is written in one place.
///
/// gx-gate's `MAX_MESSAGE_BYTES` is 1024 for ASM-60-4 and this is 1024 for the same reason: "the
/// same shape" in the ruling (sem: SEM-gx-core-197) means the same number as well as the same
/// shape, and two constants that agree today are two constants that disagree eventually.
#[test]
fn the_bound_is_one_declaration_at_the_value_the_ruling_named() {
    assert_eq!(
        MAX_SCOPE_BYTES, 1024,
        "M4H1-2: '1024 bytes' (sem: SEM-gx-core-198)"
    );

    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/fingerprint.rs"),
    )
    .expect("fingerprint.rs is readable");
    assert_eq!(
        source.matches("pub const MAX_SCOPE_BYTES").count(),
        1,
        "the bound is declared more than once, and two constants agree until they do not"
    );
    println!("MAX_SCOPE_BYTES={MAX_SCOPE_BYTES}");
}

/// At the bound it is accepted; one byte past it, it is refused.
///
/// The boundary is measured on both sides because an off-by-one here is the difference between a
/// bound and a suggestion. The count is **bytes**, as ASM-60-4 words it.
#[test]
fn the_boundary_is_measured_from_both_sides() {
    let inside = build(scope_of(MAX_SCOPE_BYTES)).expect("exactly at the bound is inside it");
    assert_eq!(inside.scope().len(), MAX_SCOPE_BYTES);

    let refusal = build(scope_of(MAX_SCOPE_BYTES + 1)).expect_err("one byte past the bound");
    assert_eq!(refusal.kind(), "ScopeTooLong");
    assert!(ERROR_KINDS.contains(&refusal.kind()));
    assert_eq!(
        refusal,
        Error::ScopeTooLong {
            bytes: MAX_SCOPE_BYTES + 1,
            max: MAX_SCOPE_BYTES
        },
        "the refusal carries both numbers, as `OrderExceeded` carries its ceiling"
    );
}

/// Bytes, not characters: a multi-byte scope is measured the way the bound is worded.
#[test]
fn the_bound_counts_bytes_and_not_characters() {
    // 512 characters, three bytes each.
    let scope = "\u{3042}".repeat(512);
    assert_eq!(scope.chars().count(), 512);
    assert_eq!(scope.len(), 1_536);
    let refusal = build(scope).expect_err("1536 bytes is past a 1024-byte bound");
    assert_eq!(refusal.kind(), "ScopeTooLong");
}

/// The message says what to do about it, because the road out is in another crate.
#[test]
fn the_refusal_names_the_road_out() {
    let refusal = build(scope_of(MAX_SCOPE_BYTES + 1)).expect_err("past the bound");
    let message = refusal.to_string();
    assert!(
        message.contains("elide") && message.contains("M4H1-2"),
        "an adapter author reading this refusal has to be able to find the elision: {message}"
    );
}
