//! **I-1** applied to the projection M4 hand 1 creates: `Intent` (42 §1.3, row 2).
//!
//! req/69 §6.0-5 makes this a condition of the hand rather than a nicety: 「**新しい IdentityView
//! 射影には I-1 形の防御を同 turn で置く**…いずれも A-10 形(canonical encode の map key 数 assert)+
//! 「1 field だけ違う 2 値の digest が異なる」を全 field 分。M3 は同じ穴を 3 箇所に開けたまま tag 窓へ
//! 来て、監査が拾った」.
//!
//! # Why the two halves are both needed
//!
//! `crates/gx-gate/tests/verdict_identity.rs` is the worked example and its header explains what
//! the M3 audit found: a projection can name every key and fill one of them with a constant, so a
//! key-count assertion alone declares five fields and identifies four. The count is checked here
//! against the struct's own derived `Debug` -- turning the literal 5 into a statement about
//! `Intent` -- and every one of the five is then shown to move the CID.
//!
//! # Why `Intent`'s mirror is strict
//!
//! 42 §1.3's row has an empty exclusion column and the reason 「Intent自体が独立の意図記述であり除外
//! 規則なし」. `Transformation` may legitimately drop `id` and `created_at`; `Intent` may drop
//! nothing. So the two field sets are not merely related, they are equal, and the assertion below
//! says so with `assert_eq` rather than with a subset check.
//!
//! ASM-11 is what makes this load-bearing rather than tidy: `IntentId` is the CID of this
//! projection, fixed at `submit` (43 T-1) and carried inside `Transformation`'s own IdentityView
//! (42 §1.3, row 3) -- so a field that fails to reach this digest fails to reach every
//! `TransformationId` downstream of it.

mod support;

use std::collections::BTreeMap;

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use gx_core::{Actor, ChangeContext, Cid, GoalBytes, Intent, SubstrateKind};
use ipld_core::ipld::Ipld;
use support::cid_of;

/// The projected bytes. Through [`cbor::encode`], because 42 §1.1 defines a CID as BLAKE3 over the
/// *canonical* form -- a projection encoded some other way would not be the thing the spec hashes.
fn view_bytes(value: &Intent) -> Vec<u8> {
    cbor::encode(&value.identity_view()).expect("the projection of a valid intent must encode")
}

fn view_keys(value: &Intent) -> Vec<String> {
    let map: BTreeMap<String, Ipld> =
        cbor::decode(&view_bytes(value)).expect("the projection is a map of named fields");
    map.into_keys().collect()
}

/// The field names a derived `Debug` prints for the value itself, one indent level down.
///
/// Lifted from `crates/gx-gate/tests/verdict_identity.rs`, where the shape and its soundness note
/// are written out: the reading is of `{:#?}`, a field of the value sits at exactly one indent, and
/// none of the fixtures below carries a newline inside a string.
fn debug_field_names(value: &Intent) -> Vec<String> {
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

fn base_intent() -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        "/tmp/x".to_string(),
        GoalBytes(b"\xa1\x64goal\x01".to_vec()),
        ChangeContext::Evidence,
        Actor::Human {
            key: "operator".to_string(),
        },
    )
}

fn digest_of(intent: &Intent) -> Cid {
    cid::compute(intent).expect("the projection has a canonical form")
}

// ---------------------------------------------------------------------------
// A-10: the key count, and the count as a statement about the struct
// ---------------------------------------------------------------------------

/// The projection is a five-key map, and the five are 42 §1.3's names.
#[test]
fn the_intent_projection_declares_the_five_keys_of_42_1_3() {
    let keys = view_keys(&base_intent());
    assert_eq!(keys.len(), 5, "42 §1.3 row 2 lists five fields: {keys:?}");
    assert_eq!(
        keys,
        vec!["actor", "context", "goal", "locator", "substrate"],
        "the five keys are not the five names of 42 §3.3"
    );
}

/// The projection's key set is the struct's field set, read from the derived `Debug`.
///
/// The count above is a literal and a literal is a claim a hand can keep true by editing it. This
/// is the same claim held against `Intent` itself: a sixth field added to the struct and not to
/// `IntentView` would be a field outside the intent's own name, which 42 §1.3's 「除外規則なし」
/// forbids by construction rather than by review.
#[test]
fn the_intent_projection_has_one_key_per_field_of_the_struct() {
    let intent = base_intent();
    assert_eq!(
        debug_field_names(&intent),
        view_keys(&intent),
        "`Intent` and `IntentView` declare different field sets (42 §1.3: 「除外規則なし」)"
    );
}

/// The projection lands on the wire face rather than beside it (AC-014, 42 §2.1-6).
#[test]
fn the_intent_projection_lands_on_the_wire_face() {
    assert!(cbor::is_canonical(&view_bytes(&base_intent())));
}

// ---------------------------------------------------------------------------
// I-1: every field reaches the digest
// ---------------------------------------------------------------------------

/// Each of the five fields moves `IntentId`.
///
/// The half a key count cannot state. Battery B-3 of req/67 was exactly this defect one crate over
/// -- a projection naming every key while filling one with a constant -- and it survived seventeen
/// suites, so the mutants here change **one field at a time** and compare digests of two different
/// values rather than recomputing one value's digest through the projection under test.
#[test]
fn every_field_of_an_intent_reaches_its_digest() {
    let baseline = digest_of(&base_intent());

    let mutants: Vec<(&str, Intent)> = vec![
        (
            "substrate",
            Intent::new(
                SubstrateKind::Git,
                "/tmp/x".to_string(),
                GoalBytes(b"\xa1\x64goal\x01".to_vec()),
                ChangeContext::Evidence,
                Actor::Human {
                    key: "operator".to_string(),
                },
            ),
        ),
        (
            "locator",
            Intent::new(
                SubstrateKind::Fs,
                "/tmp/y".to_string(),
                GoalBytes(b"\xa1\x64goal\x01".to_vec()),
                ChangeContext::Evidence,
                Actor::Human {
                    key: "operator".to_string(),
                },
            ),
        ),
        (
            "goal",
            Intent::new(
                SubstrateKind::Fs,
                "/tmp/x".to_string(),
                GoalBytes(b"\xa1\x64goal\x02".to_vec()),
                ChangeContext::Evidence,
                Actor::Human {
                    key: "operator".to_string(),
                },
            ),
        ),
        (
            "context",
            Intent::new(
                SubstrateKind::Fs,
                "/tmp/x".to_string(),
                GoalBytes(b"\xa1\x64goal\x01".to_vec()),
                ChangeContext::Policy,
                Actor::Human {
                    key: "operator".to_string(),
                },
            ),
        ),
        (
            "actor",
            Intent::new(
                SubstrateKind::Fs,
                "/tmp/x".to_string(),
                GoalBytes(b"\xa1\x64goal\x01".to_vec()),
                ChangeContext::Evidence,
                Actor::Agent {
                    key: "operator".to_string(),
                    model: "some-model".to_string(),
                },
            ),
        ),
    ];

    assert_eq!(mutants.len(), 5, "one mutant per projected field");
    for (name, mutant) in mutants {
        assert_ne!(
            digest_of(&mutant),
            baseline,
            "changing `{name}` left the IntentId unchanged, so it is not really projected"
        );
    }
}

/// A goal that differs only in its bytes is a different intent.
///
/// Separated from the loop above because it is the field most likely to be projected by reference
/// and hashed by length: `GoalBytes` is opaque to every layer, so nothing else in the workspace
/// would notice a projection that carried only its size.
#[test]
fn two_goals_of_the_same_length_are_two_intents() {
    let one = Intent::new(
        SubstrateKind::Fs,
        "/tmp/x".to_string(),
        GoalBytes(vec![0, 1, 2, 3]),
        ChangeContext::Evidence,
        Actor::Human {
            key: "operator".to_string(),
        },
    );
    let other = Intent::new(
        SubstrateKind::Fs,
        "/tmp/x".to_string(),
        GoalBytes(vec![0, 1, 2, 4]),
        ChangeContext::Evidence,
        Actor::Human {
            key: "operator".to_string(),
        },
    );
    assert_ne!(digest_of(&one), digest_of(&other));
}

/// 「同一intent→同一IntentId」 (42 §3.3): the name is a function of the value.
#[test]
fn the_same_intent_has_the_same_id() {
    assert_eq!(digest_of(&base_intent()), digest_of(&base_intent()));
    assert_ne!(
        digest_of(&base_intent()),
        cid_of(0x00),
        "non-vacuity: the digest is not a constant this file could have written down"
    );
}
