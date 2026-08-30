// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **F7 (`req/871` §1.7, `req/868` R-868-6, `req/919` W5, 2026-08-29)** — the receipt-format
//! version field, machine-checked. Mirrors the shape `confinement_attest.rs` and
//! `dr4639_catalogue_hash_attest.rs` established for the two most recent `ReceiptPayload` additive
//! fields: a structural probe (the field exists, is the declared type), a decode-compatibility
//! probe (bytes written before this field existed still decode, to `None` and not to a fabricated
//! value), and a producer probe (this build always writes `Some`).
//!
//! See `crates/gx-witness/src/receipt.rs`'s doc comment on `payload_version` for the full
//! reasoning, including the recorded tension with `req/38` SS858 §⑤'s "Owner gate" sentence.

mod support;

use gx_canon::cbor;
use gx_witness::receipt::{ReceiptPayload, CURRENT_PAYLOAD_VERSION};
use support::{commit_payload, keypair};

fn inclusion() -> gx_core::InclusionProof {
    gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// The type
// ---------------------------------------------------------------------------

/// The field exists, is `Option<u32>`, and the struct now has eighteen (the same count
/// `ac_018.rs`'s two tests measure against the spec and the struct independently).
#[test]
fn f7_the_payload_declares_an_optional_version_field() {
    let src = include_str!("../src/receipt.rs");
    let body = src
        .split("pub struct ReceiptPayload {")
        .nth(1)
        .expect("receipt.rs declares ReceiptPayload")
        .split("\n}")
        .next()
        .expect("split always yields one");
    let field = body
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("pub payload_version"))
        .expect("F7 seats the version field on the payload");
    println!("RECEIPT_PAYLOAD_VERSION_FIELD={field:?}");
    assert_eq!(
        field, "pub payload_version: Option<u32>,",
        "Option, unlike determinism_boundary: absence has exactly one honest reading here \
         (\"predates the field\"), not a first-class `unknown`"
    );
}

/// `CURRENT_PAYLOAD_VERSION` is `1` -- the first value this field ever takes. Structural, so a
/// future bump is a deliberate one-line diff a reviewer sees, not an accident this probe would
/// paper over (it does not assert the value never changes; it asserts what it starts at).
#[test]
fn f7_current_payload_version_starts_at_one() {
    println!("CURRENT_PAYLOAD_VERSION={CURRENT_PAYLOAD_VERSION}");
    assert_eq!(
        CURRENT_PAYLOAD_VERSION, 1,
        "the first generation to carry its own version number is generation 1 of the *field*, \
         not a retroactive count of the schema's real history (receipt.rs's doc comment is \
         explicit that this does not renumber the four generations that predate it)"
    );
}

// ---------------------------------------------------------------------------
// The wire: decode compatibility
// ---------------------------------------------------------------------------

/// 🔴 **`req/38` §294 ruling 2, honoured on the way in.** Bytes with no `payload_version` key
/// still decode, and they decode to `None` rather than to a fabricated `Some(0)` or a guess at
/// what the writer's version was. Mirrors `confinement_attest.rs`'s
/// `ac6_bytes_with_no_confinement_key_still_decode` and
/// `dr4639_catalogue_hash_attest.rs`'s `dr4639_ac2_ac3_bytes_with_no_catalogue_hash_key_still_decode`
/// exactly -- the same compatibility shape, for the same reason, on the field that closes F7.
#[derive(serde::Serialize)]
struct PreErratumPayload {
    key_id: gx_core::KeyId,
    verdict: Option<gx_witness::receipt::VerdictSummary>,
    enforced: bool,
    confinement: Option<gx_witness::receipt::ConfinementContext>,
    catalogue_hash: Option<String>,
    read_set: Option<gx_witness::receipt::ReadSet>,
    reversibility: Option<gx_core::Reversibility>,
    determinism_boundary: gx_core::DeterminismBoundary,
    receipt_kind: gx_witness::receipt::ReceiptKind,
    canonical_cid: gx_core::Cid,
    inverse_delta: Option<gx_core::Cid>,
    transformation: gx_core::TransformationId,
    inclusion_proof: Option<gx_core::InclusionProof>,
    fingerprint_scope: String,
    fail_posture_engaged: bool,
    precondition_fingerprint: gx_core::FingerprintBytes,
    postcondition_fingerprint: Option<gx_core::FingerprintBytes>,
}

impl PreErratumPayload {
    /// The same value as `now`, minus the one key this erratum adds.
    fn of(now: &ReceiptPayload) -> Self {
        Self {
            key_id: now.key_id.clone(),
            verdict: now.verdict.clone(),
            enforced: now.enforced,
            confinement: now.confinement.clone(),
            catalogue_hash: now.catalogue_hash.clone(),
            read_set: now.read_set.clone(),
            reversibility: now.reversibility,
            determinism_boundary: now.determinism_boundary,
            receipt_kind: now.receipt_kind,
            canonical_cid: now.canonical_cid,
            inverse_delta: now.inverse_delta,
            transformation: now.transformation,
            inclusion_proof: now.inclusion_proof.clone(),
            fingerprint_scope: now.fingerprint_scope.clone(),
            fail_posture_engaged: now.fail_posture_engaged,
            precondition_fingerprint: now.precondition_fingerprint,
            postcondition_fingerprint: now.postcondition_fingerprint,
        }
    }
}

#[test]
fn f7_bytes_with_no_payload_version_key_still_decode() {
    let key = keypair(68);
    let now = commit_payload(&key, 6, inclusion());
    let pre = cbor::encode(&PreErratumPayload::of(&now)).expect("the old shape encodes");
    let post = cbor::encode(&now).expect("the new shape encodes");

    println!(
        "F7_WIRE pre_bytes={} post_bytes={} delta={}",
        pre.len(),
        post.len(),
        post.len() - pre.len()
    );
    assert!(
        post.len() > pre.len(),
        "a canonical map with one more key is more bytes; if this is equal the shadow \
         declaration above has drifted and is no longer the pre-erratum shape"
    );
    let holds =
        |bytes: &[u8], needle: &str| bytes.windows(needle.len()).any(|w| w == needle.as_bytes());
    assert!(
        !holds(&pre, "payload_version"),
        "the pre-erratum bytes carry no such key"
    );
    assert!(holds(&post, "payload_version"), "this build's bytes do");

    let decoded: ReceiptPayload = cbor::decode(&pre).expect(
        "🔴 a receipt written before this erratum has to decode. If this line is the failure, \
         `ReceiptPayload::payload_version` has lost its `#[serde(default)]`",
    );
    assert_eq!(
        decoded.payload_version, None,
        "absent bytes read as an absent answer -- \"predates the field\", not a fabricated \
         version number"
    );
    assert_eq!(
        ReceiptPayload {
            payload_version: now.payload_version,
            // 🔴 **A2 (`req/910`, `req/919` W8, 2026-08-30)**: `PreErratumPayload` now also predates
            // `engine_version`, patched back from `now` the same way `confinement_attest.rs` and
            // `dr4639_catalogue_hash_attest.rs` patch back the errata that came after theirs. This
            // test isolates F7's key, so a later key's absence from the shadow is correct rather
            // than a drift -- which is exactly what the `post.len() > pre.len()` assertion above
            // now measures two keys' worth of.
            engine_version: now.engine_version.clone(),
            ..decoded
        },
        now,
        "nothing but the one key moved"
    );
}

// ---------------------------------------------------------------------------
// The producer
// ---------------------------------------------------------------------------

/// Every receipt this crate's own fixtures build carries `Some(CURRENT_PAYLOAD_VERSION)` --
/// checked directly on the shared `support` helpers rather than assumed, so a future edit to
/// `verdict_payload`/`commit_payload` that quietly drops the field is caught here.
#[test]
fn f7_this_builds_fixtures_always_carry_some() {
    let key = keypair(69);
    let payload = commit_payload(&key, 7, inclusion());
    println!("PAYLOAD_VERSION={:?}", payload.payload_version);
    assert_eq!(
        payload.payload_version,
        Some(CURRENT_PAYLOAD_VERSION),
        "this build's own fixtures should carry the version every receipt this build issues does"
    );
}
