// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Strategies shared by the property tests of the wire face.
//!
//! This is a directory module rather than a top-level `tests/*.rs` file on purpose: cargo builds
//! one test binary per top-level file, so `support.rs` would have become a tenth binary with no
//! tests in it. `tests/support/mod.rs` is compiled into whichever binary says `mod support;`.
//!
//! The generators cover `Transformation` (41 §3, ten fields) and everything it reaches. AC-009
//! and AC-012 both name `Transformation` specifically, and it is the widest type in gx-core --
//! it contains every other core type except `ObjectSnapshot`, `Commutation` and the measures.

// Every binary that says `mod support;` compiles the whole file, and no binary uses all of it --
// `negative_vectors.rs` wants `unhex` and none of the strategies. Without this, the unused half
// is a warning in every binary and `clippy -D warnings` fails on code that is used elsewhere.
#![allow(dead_code)]

use gx_core::{
    Actor, ChangeContext, Cid, DeltaRef, IntentId, ObjectId, ObjectSnapshot, ReprKind, Subject,
    SubstrateKind, Timestamp, Transformation, TransformationId,
};
use proptest::prelude::*;

/// Strings that a naive encoder gets wrong, plus the unconstrained case.
///
/// `.*` alone leans heavily on ASCII. The explicit arms are the classes req/23 §3 lists as the
/// ones that broke the older assets: NFC/NFD pairs that look identical, a zero-width joiner
/// sequence, a fullwidth space, and an astral-plane character whose UTF-16 form is a surrogate
/// pair. None of them may change across a round trip, and none may change the byte length the
/// encoder writes in the string header.
pub fn any_text() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => ".*",
        1 => Just(String::new()),
        1 => Just("\u{30D5}\u{30A1}".to_string()),      // NFC: the precomposed katakana "fa" (sem: SEM-gx-canon-118)
        1 => Just("\u{30D5}\u{3099}\u{30A1}".to_string()), // NFD: same glyphs, decomposed
        1 => Just("\u{3000}".to_string()),               // fullwidth space
        1 => Just("\u{200B}a\u{200D}b".to_string()),     // zero-width space / joiner
        1 => Just("\u{1F469}\u{200D}\u{1F4BB}".to_string()), // ZWJ emoji sequence
        1 => Just("\u{202E}reversed".to_string()),       // bidi override
    ]
}

pub fn any_cid() -> impl Strategy<Value = Cid> {
    any::<[u8; 32]>().prop_map(Cid)
}

pub fn any_object_id() -> impl Strategy<Value = ObjectId> {
    any_cid().prop_map(ObjectId)
}

/// Every id except the one **E-M3-13** ② takes out of the domain.
///
/// `any::<[u8; 32]>()` reaches the all-zero value with probability 2^-256 when generating and with
/// probability 1 when *shrinking*, since proptest shrinks integers toward zero. So without this the
/// first failure of any property over a `Transformation` would shrink into a value
/// `Transformation::new` refuses, and the report would be a panic inside the strategy instead of a
/// minimal counterexample. One byte is moved and only for that value, so the space this draws from
/// is the whole of `IntentId` minus the placeholder.
pub fn any_intent_id() -> impl Strategy<Value = IntentId> {
    any_cid().prop_map(|Cid(mut raw)| {
        if raw == [0u8; 32] {
            raw[31] = 1;
        }
        IntentId(Cid(raw))
    })
}

pub fn any_transformation_id() -> impl Strategy<Value = TransformationId> {
    any_cid().prop_map(TransformationId)
}

pub fn any_substrate_kind() -> impl Strategy<Value = SubstrateKind> {
    prop_oneof![
        Just(SubstrateKind::Fs),
        Just(SubstrateKind::Git),
        Just(SubstrateKind::Mcp),
        any_text().prop_map(SubstrateKind::Custom),
    ]
}

pub fn any_repr_kind() -> impl Strategy<Value = ReprKind> {
    prop_oneof![
        Just(ReprKind::Bytes),
        Just(ReprKind::Json),
        Just(ReprKind::Tree),
        Just(ReprKind::External),
    ]
}

pub fn any_change_context() -> impl Strategy<Value = ChangeContext> {
    prop_oneof![
        Just(ChangeContext::Time),
        Just(ChangeContext::Evidence),
        Just(ChangeContext::Policy),
        Just(ChangeContext::Model),
        Just(ChangeContext::Representation),
        Just(ChangeContext::Substrate),
        any_text().prop_map(ChangeContext::Custom),
    ]
}

pub fn any_actor() -> impl Strategy<Value = Actor> {
    prop_oneof![
        any_text().prop_map(|key| Actor::Human { key }),
        (any_text(), any_text()).prop_map(|(key, model)| Actor::Agent { key, model }),
        any_text().prop_map(|key| Actor::Process { key }),
    ]
}

pub fn any_delta_ref() -> impl Strategy<Value = DeltaRef> {
    (any_substrate_kind(), any_cid()).prop_map(|(substrate, cid)| DeltaRef { substrate, cid })
}

pub fn any_subject() -> impl Strategy<Value = Subject> {
    prop_oneof![
        any_object_id().prop_map(Subject::Object),
        any_transformation_id().prop_map(Subject::Transformation),
    ]
}

pub fn any_object_snapshot() -> impl Strategy<Value = ObjectSnapshot> {
    (
        any_object_id(),
        any_substrate_kind(),
        any_text(),
        any_cid(),
        any_repr_kind(),
    )
        .prop_map(|(id, substrate, locator, digest, representation)| {
            ObjectSnapshot::new(id, substrate, locator, digest, representation)
        })
}

/// All ten fields of 41 §3 vary, including the two that stay out of the IdentityView (`id` and
/// `created_at`). The wire face carries everything (A-4 face 1) (sem: SEM-gx-canon-119); leaving them fixed here would hide
/// exactly the fields the identity face is required to drop later.
///
/// # `created_at` draws from `0..` and not from `i64`, since M3 hand 6
///
/// **E-M3-13** ① makes a negative `created_at` a value `Transformation::new` refuses, and this
/// strategy builds through that constructor -- so `any::<i64>()` would have made about half of every
/// sample a panic in the generator rather than a case. What narrowed is the draw, not the claim:
/// `created_at` still varies over 2^63 values and the identity face still has to drop it, which is
/// what the properties reading this strategy assert. The other end is untouched, so
/// `i64::MAX` is still reachable and "not in the future" is still nobody's check here (sem: SEM-gx-canon-120) (that one is M5's).
pub fn any_transformation() -> impl Strategy<Value = Transformation> {
    (
        any_transformation_id(),
        any_intent_id(),
        0u8..=2,
        any_subject(),
        proptest::option::of(any_cid()),
        any_delta_ref(),
        any_change_context(),
        any_actor(),
        proptest::collection::vec(any_transformation_id(), 0..4),
        (0i64..).prop_map(Timestamp),
    )
        .prop_map(
            |(
                id,
                intent_id,
                order,
                subject,
                target,
                delta,
                context,
                actor,
                parents,
                created_at,
            )| {
                Transformation::new(
                    id,
                    order,
                    subject,
                    target,
                    parents,
                    gx_core::CompositionMetadata {
                        intent_id,
                        delta,
                        context,
                        actor,
                        created_at,
                    },
                )
                .expect("the strategy draws order from 0..=2, which is the whole admitted range")
            },
        )
}

/// A `Cid` whose thirty-two bytes are all `b`. Fixtures need distinguishable digests, not
/// plausible ones -- nothing in M1 checks that a digest is the hash of anything.
pub fn cid_of(b: u8) -> Cid {
    Cid([b; 32])
}

/// The snapshot the identity-face tests project. `/tmp/x` is the locator named by AC-011.
pub fn sample_object_snapshot() -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid_of(0x11)),
        SubstrateKind::Fs,
        "/tmp/x".to_string(),
        cid_of(0x22),
        ReprKind::Bytes,
    )
}

/// The transformation the identity-face tests project.
///
/// AC-011 names its input as "a Transformation value x of identical logical content, built
/// from the same intent I (equivalent to `{"kind":"fs.write","path":"/tmp/x","content":"hello"}`)" (sem: SEM-gx-canon-121). `Intent` is not implemented in
/// M1 (42 §3.3 is step 4's explicit non-scope), so what is reproduced here is the shape that
/// intent would produce: an `Fs` substrate, the locator `/tmp/x`, and a delta reference standing
/// in for the write. `model` is a placeholder string rather than any product's name (ASM-05-6).
pub fn sample_transformation() -> Transformation {
    Transformation::new(
        TransformationId(cid_of(0x01)),
        0,
        Subject::Object(ObjectId(cid_of(0x11))),
        Some(cid_of(0x33)),
        vec![TransformationId(cid_of(0x55))],
        sample_metadata(),
    )
    .expect("order 0 is within the ceiling")
}

/// The five fields a `Transformation` does not derive from its own shape (A-7 erratum, req/38 §1).
pub fn sample_metadata() -> gx_core::CompositionMetadata {
    gx_core::CompositionMetadata {
        intent_id: IntentId(cid_of(0x02)),
        delta: DeltaRef {
            substrate: SubstrateKind::Fs,
            cid: cid_of(0x44),
        },
        context: ChangeContext::Substrate,
        actor: Actor::Agent {
            key: "agent-key-1".to_string(),
            model: "model-a".to_string(),
        },
        created_at: Timestamp(1_700_000_000_000_000_000),
    }
}

/// Lowercase hex, for the messages that have to name a byte string.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse the `hex` field of a vector file. Panics on malformed input, which is correct for a
/// fixture loader: a vector that cannot be read is a setup failure, and req/29 §4 forbids
/// reporting setup failures as probe results.
pub fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "hex string of odd length: {s}");
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).unwrap_or_else(|e| panic!("bad hex {s}: {e}"))
        })
        .collect()
}
