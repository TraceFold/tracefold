//! **I-1** applied to the first of the two projections M4 hand 3 creates: `PlannedDelta`
//! (42 §1.3, row 4).
//!
//! req/69 §6.0-5 makes this a condition of the hand rather than a nicety: 「**新しい IdentityView
//! 射影には I-1 形の防御を同 turn で置く**。M4 が作る射影は `PlannedDelta`(=`{substrate, payload}`・
//! 42 §1.3)と `Fingerprint`(=全 field)の **2 つ**で、いずれも A-10 形(canonical encode の map key 数
//! assert)+「1 field だけ違う 2 値の digest が異なる」を全 field 分。M3 は同じ穴を 3 箇所に開けたまま
//! tag 窓へ来て、監査が拾った」.
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
//! A **self-reference**. 42 §1.3 row 4 excludes `reference` with the reason 「自己参照」, and
//! **M4H1-3** 採(a) then made [`PlannedDelta::new`] mint that very field from this projection. The
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

/// The projection's key set is the struct's field set minus the one exclusion 42 §1.3 names.
///
/// The count above is a literal, and a literal is a claim a hand can keep true by editing it. This
/// is the same claim held against `PlannedDelta` itself: a fourth field added to the struct and not
/// to `PlannedDeltaView` would be part of a delta and outside its own name, and the only field 42
/// §1.3 permits to be outside is `reference`, for the stated reason 「自己参照」.
#[test]
fn the_projection_is_the_struct_minus_its_self_reference() {
    let delta = base();
    let mut expected = debug_field_names(&delta);
    expected.retain(|f| f != "reference");
    assert_eq!(
        view_keys(&delta),
        expected,
        "`PlannedDelta` and `PlannedDeltaView` differ by something other than `reference` \
         (42 §1.3 row 4: 除外=`reference`, 理由=自己参照)"
    );
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

/// `reference.cid` is the digest of this delta's own projection (42 §3.4, **M4H1-3** 採(a)).
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
        "42 §3.4 逐語: 「自身のcanonical参照(`DeltaRef.cid`と一致)」"
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
/// diverge. This is also what makes 「同一 delta」 in E-M4-3's idempotence quantifier a decidable
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
