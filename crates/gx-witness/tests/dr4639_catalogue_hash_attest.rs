// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! DR-46-39 — `catalogue_hash`, the frozen-payload 3-point discipline measured rather than
//! assumed (`req/777` §3, ruling `req/38` §5689).
//!
//! Same shape `confinement_attest.rs`'s AC-6 half already established for its own field, applied
//! one field along: (AC-1) the seat is an `Option`, (AC-2) `#[serde(default)]` is what keeps a
//! receipt this build issued *before* the erratum decodable, and (AC-3) that claim is a decode
//! probe, not a comment near an attribute.

mod support;

use gx_canon::cbor;
use gx_core::{
    Cid, DeterminismBoundary, FingerprintBytes, InclusionProof, KeyId, Reversibility,
    TransformationId,
};
use gx_witness::receipt::{
    ConfinementContext, ReadSet, ReceiptKind, ReceiptPayload, VerdictSummary,
};

use support::{commit_payload, keypair};

/// A one-leaf proof: 43 T-11's shape for the first commit a log holds. Spelled here (rather than
/// imported from `support`, which does not export it) for the same reason
/// `confinement_attest.rs::inclusion` is local: the shared fixture takes one rather than building
/// one.
fn inclusion() -> InclusionProof {
    InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    }
}

/// The wire shape one erratum before this one: every current field except `catalogue_hash`.
/// Deliberately a second, independent type rather than a clone-and-strip of `ReceiptPayload` --
/// the same discipline `confinement_attest.rs`'s `PreErratumPayload` uses, so a decode probe
/// against it cannot pass by accident from the two types secretly sharing a derive.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PreErratumPayload {
    key_id: KeyId,
    verdict: Option<VerdictSummary>,
    enforced: bool,
    confinement: Option<ConfinementContext>,
    read_set: Option<ReadSet>,
    reversibility: Option<Reversibility>,
    determinism_boundary: DeterminismBoundary,
    receipt_kind: ReceiptKind,
    canonical_cid: Cid,
    inverse_delta: Option<Cid>,
    transformation: TransformationId,
    inclusion_proof: Option<InclusionProof>,
    fingerprint_scope: String,
    fail_posture_engaged: bool,
    precondition_fingerprint: FingerprintBytes,
    postcondition_fingerprint: Option<FingerprintBytes>,
}

impl PreErratumPayload {
    /// The same value as `now`, minus the one key this erratum adds.
    fn of(now: &ReceiptPayload) -> Self {
        Self {
            key_id: now.key_id.clone(),
            verdict: now.verdict.clone(),
            enforced: now.enforced,
            confinement: now.confinement.clone(),
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

/// **AC-1**: the field is present, `Option`-typed, and `None` on a hand-built fixture that named
/// no catalogue -- the honest starting state, not a placeholder value.
#[test]
fn dr4639_ac1_the_field_is_an_option_and_absent_by_default_on_a_fixture() {
    let key = keypair(90);
    let payload = commit_payload(&key, 1, inclusion());
    assert_eq!(
        payload.catalogue_hash, None,
        "a fixture that never named a governing catalogue must read back as an absent answer, \
         not a fabricated one"
    );
}

/// **AC-2 / AC-3**: bytes a build issued *before* this erratum -- no `catalogue_hash` key at all --
/// still decode, and the decoded value reads `None`. Measured the same way `confinement_attest.rs`
/// measures its own field: byte-length delta, key-presence scan, then an actual decode.
#[test]
fn dr4639_ac2_ac3_bytes_with_no_catalogue_hash_key_still_decode() {
    let key = keypair(91);
    let now = commit_payload(&key, 2, inclusion());
    let pre = cbor::encode(&PreErratumPayload::of(&now)).expect("the old shape encodes");
    let post = cbor::encode(&now).expect("the new shape encodes");

    println!(
        "DR4639_AC23_WIRE pre_bytes={} post_bytes={} delta={}",
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
        !holds(&pre, "catalogue_hash"),
        "the pre-erratum bytes carry no such key"
    );
    assert!(holds(&post, "catalogue_hash"), "this build's bytes do");

    let decoded: ReceiptPayload = cbor::decode(&pre).expect(
        "🔴 `req/38` §294 ruling 2 / `req/777` AC-2: a receipt written before this erratum has \
         to decode. If this line is the failure, `ReceiptPayload::catalogue_hash` has lost its \
         `#[serde(default)]`",
    );
    assert_eq!(
        decoded.catalogue_hash, None,
        "absent bytes read as an absent answer -- no catalogue was named as governing, not a \
         fabricated empty digest"
    );
    // 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)**: `PreErratumPayload` now also
    // predates `payload_version` -- patched back from `now` for the same reason `catalogue_hash`
    // is: this test isolates `catalogue_hash`'s addition specifically, not a later erratum's.
    assert_eq!(
        ReceiptPayload {
            catalogue_hash: now.catalogue_hash.clone(),
            payload_version: now.payload_version,
            // 🔴 **A2 (`req/910`, `req/919` W8, 2026-08-30)**: likewise -- this shadow predates
            // `engine_version` too, and this test isolates `catalogue_hash`.
            engine_version: now.engine_version.clone(),
            ..decoded
        },
        now,
        "every OTHER field round-trips unchanged through the pre-erratum decode -- the erratum \
         added exactly one key and moved nothing else"
    );
}

/// **AC-2, source-level**: the `#[serde(default)]` attribute is where AC-2 says it must be,
/// checked by reading the source rather than inferred from the decode probe passing (the same
/// separation `confinement_attest.rs::ac6_the_compatibility_default_is_declared_in_the_source`
/// draws, since a decode probe and a source scan can each catch a defect the other misses).
#[test]
fn dr4639_ac2_the_compatibility_default_is_declared_in_the_source() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt.rs"),
    )
    .expect("the payload's own module is here");
    let at = source
        .find("pub catalogue_hash: Option<String>,")
        .expect("`ReceiptPayload` declares the seat `req/777` AC-1 asks for");
    let above = &source[at.saturating_sub(200)..at];
    println!(
        "DR4639_AC2_DEFAULT declared={}",
        above.contains("#[serde(default)]")
    );
    assert!(
        above.contains("#[serde(default)]"),
        "🔴 `req/38` §294 ruling 2 / `req/777` AC-2: the seat's serde default is the whole of \
         what keeps pre-erratum receipts decodable"
    );
}

/// **AC-0 / §2**: the field is `String`, not `Cid` -- checked at the type level (this line would
/// not compile if the field's type changed to `Cid`), for the exact reason
/// `ConfinementContext::ruleset_hash` is a `String` and not a `Cid`: it does not join on anything
/// else this payload carries, and a `Cid`-typed field would put it in the same namespace as
/// `canonical_cid`/`transformation`, which it is not part of.
#[test]
fn dr4639_ac0_the_seat_is_string_typed_not_cid_typed() {
    let key = keypair(92);
    let mut payload = commit_payload(&key, 3, inclusion());
    let digest_text: String = "gx1:leaf:deadbeef".to_string();
    payload.catalogue_hash = Some(digest_text.clone());
    assert_eq!(payload.catalogue_hash, Some(digest_text));
}

/// **AC-4**: `catalogue_hash` must not appear in `frozen_receipt_corpus.rs`'s
/// `DECLARED_REQUIRED_WITH_NO_DEFAULT` -- it is optional-with-default, and that constant asserts
/// the opposite contract. Read the live source rather than trusted from memory, so a future edit
/// that mistakenly adds it here fails loudly.
#[test]
fn dr4639_ac4_the_field_is_not_in_the_required_with_no_default_set() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/frozen_receipt_corpus.rs"),
    )
    .expect("the corpus test lives beside this one");
    let at = source
        .find("const DECLARED_REQUIRED_WITH_NO_DEFAULT")
        .expect("the declared-required-set constant exists");
    let line_end = source[at..].find(';').map_or(source.len(), |i| at + i);
    let decl = &source[at..line_end];
    assert!(
        !decl.contains("catalogue_hash"),
        "`catalogue_hash` is optional-with-default (AC-1/AC-2); it must not be declared \
         required-with-no-default, which asserts the opposite compatibility contract: {decl}"
    );
}
