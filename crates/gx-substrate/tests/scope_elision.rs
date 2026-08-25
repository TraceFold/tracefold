// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **M4H1-2**, the half that needs a digest: `elide_scope`. (sem: SEM-gx-substrate-107,
//! SEM-gx-substrate-108)
//!
//! 42 §3.8's "long text becomes a digest" is the shape M3-21 gave gx-gate for a `Reason.message`,
//! and M4H1-2
//! asks for the same thing one type over -- a scope past [`gx_core::MAX_SCOPE_BYTES`] becomes a line
//! naming how many bytes were replaced and the CID of what they were. gx-core cannot do it (A-1
//! keeps the hash in gx-canon), so it is here, in the module 41 §2 gave this crate for exactly this:
//! `gx-substrate/src/fingerprint.rs`, which **E-M4-1** re-read as "where the computation lives".
//!
//! # What an elided scope still is
//!
//! A scope is compared, never parsed: 42 §3.5's equality asks whether two fingerprints name the same
//! state, and `cas_eq` refuses when the scopes differ. So an elision has to be a **function of the
//! text** -- the same long path has to elide to the same line every time, or a CAS check across two
//! reads of one file would refuse itself. That is the property this file measures, and it is why the
//! digest goes through gx-canon rather than through anything faster.

use gx_canon::cid::{self, IdentityView};
use gx_core::MAX_SCOPE_BYTES;
use gx_substrate::elide_scope;

fn scope_of(bytes: usize) -> String {
    "s".repeat(bytes)
}

/// Under the bound, the scope is itself: the elision is not a formatter.
#[test]
fn a_scope_inside_the_bound_is_unchanged() {
    for len in [0, 1, 64, MAX_SCOPE_BYTES] {
        let scope = scope_of(len);
        assert_eq!(
            elide_scope(scope.clone()).expect("a string always has a canonical form"),
            scope,
            "a {len}-byte scope was rewritten"
        );
    }
}

/// Past the bound it becomes a line that names what it replaced, and fits.
#[test]
fn a_scope_past_the_bound_becomes_a_line_that_fits() {
    let scope = scope_of(MAX_SCOPE_BYTES + 1);
    let elided = elide_scope(scope.clone()).expect("gx-canon names a string");
    println!("ELIDED_SCOPE={elided}");

    assert!(
        elided.len() <= MAX_SCOPE_BYTES,
        "the elision is itself too long"
    );
    assert!(elided.starts_with("<elided:"));
    assert!(
        elided.contains(&(MAX_SCOPE_BYTES + 1).to_string()),
        "the line does not say how many bytes it replaced: {elided}"
    );
    assert!(
        elided.contains("gx1:"),
        "the line carries no digest: {elided}"
    );
}

/// The digest is gx-canon's, over the text, so an operator holding the original can recompute it.
///
/// The same claim M3-21 makes about an elided message, and the same reason it is worth measuring: a
/// digest nobody can reproduce is a decoration. The road is [`cid::compute`] over the string's own
/// projection -- one encoder, 41 §6 -- and this recomputes it from outside the crate.
#[test]
fn the_digest_is_recomputable_from_the_original() {
    struct Text<'t>(&'t str);
    impl IdentityView for Text<'_> {
        type View<'a>
            = &'a str
        where
            Self: 'a;
        fn identity_view(&self) -> Self::View<'_> {
            self.0
        }
    }

    let scope = format!("/very/long/{}", scope_of(MAX_SCOPE_BYTES));
    let elided = elide_scope(scope.clone()).expect("an elision");
    let expected = cid::to_text(&cid::compute(&Text(&scope)).expect("a string encodes"));
    assert!(
        elided.contains(&expected),
        "the line's digest is not `cid::compute` of the scope: {elided} vs {expected}"
    );
}

/// The elision is a function: the same scope gives the same line, twice and from two callers.
///
/// This is what keeps `cas_eq` meaningful for a file whose path is past the bound -- two reads of
/// one file produce two fingerprints, and if their scopes disagreed the comparison would refuse
/// instead of answering.
#[test]
fn the_elision_is_a_function_of_the_text() {
    let scope = scope_of(MAX_SCOPE_BYTES * 2);
    let once = elide_scope(scope.clone()).expect("an elision");
    let twice = elide_scope(scope).expect("an elision");
    assert_eq!(once, twice);

    let other = elide_scope(scope_of(MAX_SCOPE_BYTES * 2 + 1)).expect("an elision");
    assert_ne!(once, other, "two different scopes elided to one line");
}

/// An elided scope is short enough for `Fingerprint::new` to accept, which is the point of it.
#[test]
fn what_comes_out_is_a_scope_gx_core_accepts() {
    let scope = scope_of(MAX_SCOPE_BYTES * 4);
    let elided = elide_scope(scope).expect("an elision");
    let fingerprint = gx_core::Fingerprint::new(
        gx_core::SubstrateKind::Fs,
        elided.clone(),
        gx_core::Cid([1u8; 32]),
    )
    .expect("the elision is inside the bound M4H1-2 set");
    assert_eq!(fingerprint.scope(), elided);
}
