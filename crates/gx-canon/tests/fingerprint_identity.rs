// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **I-1** applied to the second of the two projections M4 hand 3 creates: `Fingerprint`
//! (42 §1.3, row 5).
//!
//! req/69 §6.0-5: "the projections M4 creates are **two**: `PlannedDelta` (= `{substrate,
//! payload}`, 42 §1.3) and `Fingerprint` (= all fields), both in A-10 shape (an assert on the
//! canonical encode's map key count) + "two values differing in only one field have different
//! digests", for every field" (sem: SEM-gx-canon-082). The delta half is
//! `crates/gx-substrate/tests/planned_delta_identity.rs`; this is the fingerprint half, and it lives
//! in gx-canon because the projection does -- **E-M4-1** put the type in gx-core, and the orphan
//! rule puts every gx-core projection here.
//!
//! # Why `Fingerprint`'s mirror is strict
//!
//! 42 §1.3 row 5 has an empty exclusion column: `substrate`, `scope` and `digest` are all of what a
//! fingerprint is. So the two field sets are not merely related, they are equal, and the assertion
//! below says so with `assert_eq` rather than with a subset check -- the same shape `Intent`'s row
//! forces one row up.
//!
//! # What a CID of a fingerprint is **not**
//!
//! It is not 42 §3.5's equality. That relation has three answers -- `Ok(true)`, `Ok(false)` and the
//! two refusals **E-M4-15** and **E-M4-27** typed -- and a digest has one. Two fingerprints with
//! different scopes have different CIDs *and* no defined comparison at all, so a caller that reached
//! for `compute(a) == compute(b)` to decide a CAS check would have replaced a refusal with a `false`,
//! which is precisely the substitution E-M4-15 rejected. What the projection is for is the
//! conformance harness: L3 and L4 need to say "the state came back" and "the state moved" (sem: SEM-gx-canon-083) about
//! recorded values, and a digest is how a report carries one without holding the substrate.

mod support;

use std::collections::BTreeMap;

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use gx_core::{Cid, Fingerprint, SubstrateKind};
use ipld_core::ipld::Ipld;
use support::cid_of;

fn base_fingerprint() -> Fingerprint {
    Fingerprint::new(SubstrateKind::Fs, "/tmp/x".to_string(), cid_of(0x11))
        .expect("a short scope is inside M4H1-2's bound")
}

fn view_bytes(value: &Fingerprint) -> Vec<u8> {
    cbor::encode(&value.identity_view()).expect("the projection of a fingerprint must encode")
}

fn view_keys(value: &Fingerprint) -> Vec<String> {
    let map: BTreeMap<String, Ipld> =
        cbor::decode(&view_bytes(value)).expect("the projection is a map of named fields");
    map.into_keys().collect()
}

fn digest_of(value: &Fingerprint) -> Cid {
    cid::compute(value).expect("the projection has a canonical form")
}

/// The field names a derived `Debug` prints for the value itself, one indent level down.
///
/// Lifted from `intent_identity.rs`, where the shape and its soundness note are written out.
fn debug_field_names(value: &Fingerprint) -> Vec<String> {
    let text = format!("{value:#?}");
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("    ")?;
            if rest.starts_with(' ') {
                return None;
            }
            let (name, _) = rest.split_once(": ")?;
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c == '_')
                .then(|| name.to_string())
        })
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// A-10: the key count, and the count as a statement about the struct
// ---------------------------------------------------------------------------

/// The projection is a three-key map, and the three are 42 §3.5's names.
#[test]
fn the_fingerprint_projection_declares_the_three_keys_of_42_1_3() {
    let keys = view_keys(&base_fingerprint());
    println!("FINGERPRINT_VIEW_KEYS={} ({keys:?})", keys.len());
    assert_eq!(keys.len(), 3, "42 §1.3 row 5 lists three fields: {keys:?}");
    assert_eq!(
        keys,
        vec!["digest", "scope", "substrate"],
        "the three keys are not the three names of 42 §3.5"
    );
}

/// The projection's key set is the struct's field set. No exclusion, so no difference.
#[test]
fn the_fingerprint_projection_has_one_key_per_field_of_the_struct() {
    let fingerprint = base_fingerprint();
    assert_eq!(
        debug_field_names(&fingerprint),
        view_keys(&fingerprint),
        "`Fingerprint` and `FingerprintView` declare different field sets (42 §1.3 row 5 excludes \
         nothing)"
    );
}

/// The projection lands on the wire face rather than beside it (AC-014, 42 §2.1-6).
#[test]
fn the_fingerprint_projection_lands_on_the_wire_face() {
    assert!(cbor::is_canonical(&view_bytes(&base_fingerprint())));
}

// ---------------------------------------------------------------------------
// I-1: every field reaches the digest
// ---------------------------------------------------------------------------

/// Each of the three fields moves the fingerprint's CID.
///
/// `scope` is the one that matters most and the one a lazy projection would drop: 42 §3.5 lets an
/// adapter widen a scope past the object itself, so two fingerprints over the same digest and
/// different scopes are two different statements about the world. A projection that carried only
/// `digest` would identify them -- and a harness comparing recorded fingerprints would then report
/// "the state did not move" (sem: SEM-gx-canon-084) across a scope change nobody agreed to.
#[test]
fn every_field_of_a_fingerprint_reaches_its_digest() {
    let baseline = digest_of(&base_fingerprint());

    let mutants: Vec<(&str, Fingerprint)> = vec![
        (
            "substrate",
            Fingerprint::new(SubstrateKind::Git, "/tmp/x".to_string(), cid_of(0x11))
                .expect("a short scope is inside M4H1-2's bound"),
        ),
        (
            "scope",
            Fingerprint::new(SubstrateKind::Fs, "/tmp/y".to_string(), cid_of(0x11))
                .expect("a short scope is inside M4H1-2's bound"),
        ),
        (
            "digest",
            Fingerprint::new(SubstrateKind::Fs, "/tmp/x".to_string(), cid_of(0x12))
                .expect("a short scope is inside M4H1-2's bound"),
        ),
    ];

    assert_eq!(mutants.len(), 3, "one mutant per projected field");
    for (name, mutant) in mutants {
        assert_ne!(
            digest_of(&mutant),
            baseline,
            "changing `{name}` left the fingerprint's CID unchanged, so it is not really projected"
        );
    }
}

/// Two scopes that differ by one character are two fingerprints.
///
/// Separated because `scope` is a `String` an adapter composes, and a projection that hashed its
/// length -- or that lower-cased it on the way -- would pass the loop above while making
/// `/tmp/A` and `/tmp/B` one name. E-M4-12 already leaves the case question to each adapter; nothing
/// in the identity face may decide it for them.
#[test]
fn two_scopes_of_the_same_length_are_two_fingerprints() {
    let one = Fingerprint::new(SubstrateKind::Fs, "/tmp/A".to_string(), cid_of(0x11))
        .expect("a short scope is inside M4H1-2's bound");
    let other = Fingerprint::new(SubstrateKind::Fs, "/tmp/B".to_string(), cid_of(0x11))
        .expect("a short scope is inside M4H1-2's bound");
    assert_ne!(digest_of(&one), digest_of(&other));
}

/// The same fingerprint has the same CID, and the CID is not a constant this file could have
/// written down.
#[test]
fn the_same_fingerprint_has_the_same_digest() {
    assert_eq!(
        digest_of(&base_fingerprint()),
        digest_of(&base_fingerprint())
    );
    assert_ne!(
        digest_of(&base_fingerprint()),
        cid_of(0x00),
        "non-vacuity: the digest is not a constant"
    );
}
