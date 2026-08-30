// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **I-1** applied to the first of the two projections M4 hand 3 creates: `PlannedDelta`
//! (42 §1.3, row 4).
//!
//! (sem: SEM-gx-substrate-114, SEM-gx-substrate-115, SEM-gx-substrate-116, SEM-gx-substrate-117,
//! SEM-gx-substrate-118, SEM-gx-substrate-119, SEM-gx-substrate-120)
//!
//! req/69 §6.0-5 makes this a condition of the hand rather than a nicety: "**place I-1's defence in
//! the same turn as any new `IdentityView` projection**. The projections M4 builds are **two**:
//! `PlannedDelta` (= `{substrate, payload}`, 42 §1.3) and `Fingerprint` (= all fields), both in the
//! A-10 shape (asserting the canonical encoding's map key count) + 'two values differing in only one
//! field have different digests' for every field. M3 came into the tag window with the same hole
//! open in three places, and the audit caught it."
//!
//! # The two halves, and why one is not enough
//!
//! `crates/gx-gate/tests/verdict_identity.rs` is the worked example and req/67 §2.1's battery B-3 is
//! the defect it was written for: a projection can name every key and fill one of them with a
//! constant, so a key-count assertion alone declares two fields and identifies one. Hand 1 saw the
//! same shape again in its own mutation (f) -- `IntentView.locator` pinned to `""` was caught by
//! exactly one probe out of sixteen suites. So the count is checked against the struct's own
//! exclusion rule, and then every projected field is shown to move the digest.
//!
//! # What this projection has that the others do not
//!
//! A **self-reference**. 42 §1.3 row 4 excludes `reference` with the reason "self-reference", and
//! **M4H1-3**, adopted (a), then made [`PlannedDelta::new`] mint that very field from this
//! projection. The
//! constructor therefore has to start from a placeholder, and the soundness of doing so is exactly
//! the exclusion: if `reference` reached the digest, the minted CID would depend on the placeholder
//! and two identical deltas would get different names. That is not an argument this file trusts --
//! `the_placeholder_the_constructor_starts_from_cannot_reach_the_digest` measures it.

use std::collections::BTreeMap;

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use gx_core::{Cid, SubstrateKind};
use gx_substrate::PlannedDelta;
use serde::de::IgnoredAny;

fn planned(substrate: SubstrateKind, payload: &[u8]) -> PlannedDelta {
    PlannedDelta::new(substrate, payload.to_vec())
        .expect("a delta over a byte payload has a canonical form")
}

fn base() -> PlannedDelta {
    planned(SubstrateKind::Fs, b"replace /tmp/x")
}

/// The projected bytes. Through [`cbor::encode`], because 42 §1.1 defines a CID as BLAKE3 over the
/// *canonical* form -- a projection encoded some other way would not be the thing the spec hashes.
fn view_bytes(value: &PlannedDelta) -> Vec<u8> {
    cbor::encode(&value.identity_view()).expect("the projection of a valid delta must encode")
}

/// The keys of the projection, read back out of the encoded map.
///
/// `IgnoredAny` rather than a value type: this file may not name a CBOR codec's own value model --
/// AC-014 (42 §2.1-6) bans `ipld_core` outside gx-canon, and the first run of hand 3's L8 scan was
/// caught by that same gate. Keys are all this needs.
fn view_keys(value: &PlannedDelta) -> Vec<String> {
    let map: BTreeMap<String, IgnoredAny> =
        cbor::decode(&view_bytes(value)).expect("the projection is a map of named fields");
    map.into_keys().collect()
}

fn digest_of(value: &PlannedDelta) -> Cid {
    cid::compute(value).expect("the projection has a canonical form")
}

/// The field names a derived `Debug` prints for the value itself, one indent level down.
///
/// Lifted from `crates/gx-canon/tests/intent_identity.rs`, where the shape and its soundness note
/// are written out: the reading is of `{:#?}`, a field of the value sits at exactly one indent, and
/// none of the fixtures below carries a newline inside a string.
fn debug_field_names(value: &PlannedDelta) -> Vec<String> {
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

/// The projection is a two-key map, and the two are 42 §1.3's names.
#[test]
fn the_planned_delta_projection_declares_the_two_keys_of_42_1_3() {
    let keys = view_keys(&base());
    println!("PLANNED_DELTA_VIEW_KEYS={} ({keys:?})", keys.len());
    assert_eq!(keys.len(), 2, "42 §1.3 row 4 lists two fields: {keys:?}");
    assert_eq!(
        keys,
        vec!["payload", "substrate"],
        "the two keys are not 42 §3.4's two names"
    );
}

/// The projection's key set is the struct's field set minus the exclusions 42 §1.3 names.
///
/// The count above is a literal, and a literal is a claim a hand can keep true by editing it. This
/// is the same claim held against `PlannedDelta` itself: a field added to the struct and not to
/// `PlannedDeltaView` would be part of a delta and outside its own name, so the fields 42 §1.3
/// permits to be outside are enumerated here and any other one fails this test.
///
/// 🔴 **The list moved from one to two** (`req/919` A1; **M5H2-2, adopted (b)**). `promised_target`
/// is the second exclusion and 42 §1.3 row 4 carries its reason: a delta *is* the change it
/// describes, so two deltas with the same `{substrate, payload}` are the same delta whatever
/// either predicts about the result — and 43 T-2 already binds the prediction into identity one
/// level up, in the `TransformationId` "including delta/**target**". It moved because a named,
/// dated ruling moved it; the mechanism this test defends is unchanged, and a third field added
/// without a ruling still fails here.
#[test]
fn the_projection_is_the_struct_minus_its_self_reference() {
    let delta = base();
    const EXCLUDED: [&str; 2] = ["reference", "promised_target"];
    let mut expected = debug_field_names(&delta);
    expected.retain(|f| !EXCLUDED.contains(&f.as_str()));
    assert_eq!(
        view_keys(&delta),
        expected,
        "`PlannedDelta` and `PlannedDeltaView` differ by something other than {EXCLUDED:?} \
         (42 §1.3 row 4: excluded = `reference` (self-reference) and `promised_target` (the \
         prediction is bound at 43 T-2's `TransformationId`, not at the delta's))"
    );
}

/// A promise does not move the delta's name, and that is what lets the builder run after `new`.
///
/// 🔴 **M5H2-2, adopted (b)** (`req/919` A1) — the soundness argument of
/// `PlannedDelta::with_promised_target`, measured rather than reasoned. `reference` is minted by
/// the constructor and never re-minted, so if the seat were inside the projection the builder
/// would leave every delta naming what it used to be. It is outside, so the digest, the reference
/// and the projected keys are all the same on both sides of a promise. The negative control is the
/// pair above it: `payload` **does** move the digest, so this file is not simply unable to see a
/// change.
#[test]
fn a_promised_target_is_outside_the_delta_s_own_name() {
    let bare = base();
    let promised = base().with_promised_target(Cid([7u8; 32]));

    assert_eq!(
        digest_of(&bare),
        digest_of(&promised),
        "the prophecy reached the CID, so 42 §1.3 row 4's second exclusion is not implemented"
    );
    assert_eq!(
        bare.reference().cid,
        promised.reference().cid,
        "the builder left `reference` naming a value that no longer exists"
    );
    assert_eq!(view_keys(&bare), view_keys(&promised));
    assert_eq!(
        promised.promised_target(),
        Some(Cid([7u8; 32])),
        "the seat did not carry what the builder put in it"
    );
    assert_eq!(bare.promised_target(), None, "`new` invented a prophecy");
}

/// The projection lands on the wire face rather than beside it (AC-014, 42 §2.1-6).
#[test]
fn the_planned_delta_projection_lands_on_the_wire_face() {
    assert!(cbor::is_canonical(&view_bytes(&base())));
}

// ---------------------------------------------------------------------------
// I-1: every projected field reaches the digest
// ---------------------------------------------------------------------------

/// Both projected fields move the delta's CID.
///
/// The half a key count cannot state. Battery B-3 (req/67 §2.1) and hand 1's mutation (f) were both
/// this defect -- a projection naming every key while filling one with a constant -- so the mutants
/// here change **one field at a time** and compare the digests of two different values rather than
/// recomputing one value's digest through the projection under test.
#[test]
fn every_field_of_a_planned_delta_reaches_its_digest() {
    let baseline = digest_of(&base());

    let mutants: Vec<(&str, PlannedDelta)> = vec![
        ("substrate", planned(SubstrateKind::Git, b"replace /tmp/x")),
        ("payload", planned(SubstrateKind::Fs, b"replace /tmp/y")),
    ];

    assert_eq!(mutants.len(), 2, "one mutant per projected field");
    for (name, mutant) in mutants {
        assert_ne!(
            digest_of(&mutant),
            baseline,
            "changing `{name}` left the delta's CID unchanged, so it is not really projected"
        );
    }
}

/// A payload that differs only in its bytes is a different delta.
///
/// Separated from the loop above because `payload` is the field most likely to be projected by
/// length: it is opaque to every layer (P-6), so nothing else in the workspace would notice a
/// projection that carried only its size. Same probe `intent_identity.rs` keeps for `GoalBytes`.
#[test]
fn two_payloads_of_the_same_length_are_two_deltas() {
    let one = planned(SubstrateKind::Fs, &[0, 1, 2, 3]);
    let other = planned(SubstrateKind::Fs, &[0, 1, 2, 4]);
    assert_ne!(digest_of(&one), digest_of(&other));
}

// ---------------------------------------------------------------------------
// 42 §3.4: the reference is the CID of the projection
// ---------------------------------------------------------------------------

/// `reference.cid` is the digest of this delta's own projection (42 §3.4, **M4H1-3**, adopted (a)).
///
/// The agreement hand 1 could only state. Both sides are computed here from the same value, which is
/// the tautology `proof_digest.rs` was caught in (req/68 §2) unless the two roads differ -- and they
/// do: the constructor mints through `cid::compute` on a value whose `reference` is a placeholder,
/// while this reads the field afterwards. A constructor that stored a constant, or that hashed
/// something other than the projection, fails here.
#[test]
fn the_reference_is_the_cid_of_the_projection() {
    let delta = base();
    assert_eq!(
        delta.reference().cid,
        digest_of(&delta),
        "42 §3.4, verbatim: \"its own canonical reference (matches `DeltaRef.cid`)\""
    );
    assert_eq!(
        &delta.reference().substrate,
        delta.substrate(),
        "the reference names a different adapter's grammar than the delta does"
    );
}

/// The placeholder the constructor starts from cannot reach the digest.
///
/// The soundness argument of [`PlannedDelta::new`], measured rather than reasoned. Two deltas built
/// through the constructor from the same substrate and payload get the same reference; if
/// `reference` were in the projection, the first mint would feed the second and the two would
/// diverge. This is also what makes "the same delta" in E-M4-3's idempotence quantifier a decidable
/// phrase.
#[test]
fn the_placeholder_the_constructor_starts_from_cannot_reach_the_digest() {
    let first = base();
    let second = base();
    assert_eq!(first.reference().cid, second.reference().cid);
    assert_eq!(first, second);

    assert_ne!(
        first.reference().cid,
        Cid([0u8; 32]),
        "non-vacuity: the minted CID is not the placeholder itself"
    );
}
