// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **A2 (`req/910` A., `req/38` SS830, `req/919` W8, 2026-08-30)** — the engine-version seat on
//! the receipt, machine-checked.
//!
//! Mirrors `r868_payload_version_attest.rs`, which mirrored `confinement_attest.rs`: a structural
//! probe (the field exists and is the declared type), a decode-compatibility probe (bytes written
//! before the field existed still decode, to `None` and not to a fabricated value), and a producer
//! probe. The engine-side seats — where the value comes from on each of the four roads — are
//! measured in `gx-engine`'s own suite, because that is where an engine exists to run.
//!
//! # 🔴 What this suite refuses to claim
//!
//! `req/910` A2 says `engine_version` is never captured and never rendered. **Half of that was stale before
//! this lane started**: `Engine::derive_provenance` has written 42 §3.9's
//! `Environment.engine_version` into the journal since M5-25, and `GET /healthz` has rendered it
//! since M6H5-12. What was true is that the *receipt* could not say it, which is the half this
//! closes.
//!
//! And the half it does **not** close is asserted here as a fact rather than left in prose: the
//! value is the workspace version, it reads the same string for every build this project has
//! produced, and `#435`'s question — which *build* answered — therefore remains open. A test that
//! asserted "the receipt names the build" would be this workspace shipping the exact class of green
//! lie it audits for, so the last test below pins the **limit** instead.

mod support;

use gx_canon::cbor;
use gx_witness::receipt::ReceiptPayload;
use gx_core::VerdictKind;
use support::{commit_payload, keypair, verdict_payload, FIXTURE_ENGINE_VERSION};

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

/// The field exists, is `Option<String>`, and carries `#[serde(default)]`.
///
/// The attribute is asserted rather than assumed, and that is not pedantry: W5 landed
/// `payload_version` while its own doc comment and its own test's failure message both named a
/// `#[serde(default)]` that was not on the field. The test stayed green because serde's derive
/// routes a missing field through `missing_field`, which succeeds for anything that deserialises
/// from `None` — so an `Option` decodes from absent bytes either way, and the compatibility claim
/// was true while its stated mechanism was absent. Asserting the attribute is what makes the
/// sentence and the code the same fact.
#[test]
fn a2_the_payload_declares_an_optional_engine_version_field_with_a_default() {
    let src = include_str!("../src/receipt.rs");
    let body = src
        .split("pub struct ReceiptPayload {")
        .nth(1)
        .expect("receipt.rs declares ReceiptPayload")
        .split("\n}")
        .next()
        .expect("split always yields one");
    let lines: Vec<&str> = body.lines().map(str::trim).collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("pub engine_version"))
        .expect("A2 seats the engine-version field on the payload");
    println!("RECEIPT_ENGINE_VERSION_FIELD={:?}", lines[at]);
    assert_eq!(
        lines[at], "pub engine_version: Option<String>,",
        "an Option, for confinement's and catalogue_hash's reason: absence has exactly one honest \
         reading here (\"these bytes predate the erratum\"), not a first-class unknown"
    );
    assert_eq!(
        lines[at - 1],
        "#[serde(default)]",
        "🔴 the attribute the doc comment promises. See this test's own doc for why it is asserted \
         and not assumed"
    );

    // The same assertion turned on the field this one learned it from.
    let pv = lines
        .iter()
        .position(|l| l.starts_with("pub payload_version"))
        .expect("F7's field is still seated");
    assert_eq!(
        lines[pv - 1],
        "#[serde(default)]",
        "🔴 `req/919` W8 added this attribute to F7's field, which had been promised by two texts \
         and present in neither. If this line fails, that repair was reverted"
    );
}

// ---------------------------------------------------------------------------
// The wire: decode compatibility
// ---------------------------------------------------------------------------

/// The pre-A2 shape: every key this build writes except the one this erratum adds.
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
    payload_version: Option<u32>,
}

impl PreErratumPayload {
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
            payload_version: now.payload_version,
        }
    }
}

/// 🔴 A receipt signed before this erratum still decodes, and to `None`.
///
/// `req/38` §294 ruling 2 registered what the alternative costs in real receipts that stopped
/// decoding. This is that ruling honoured on the way in, for the eighth key.
#[test]
fn a2_bytes_with_no_engine_version_key_still_decode() {
    let key = keypair(80);
    let now = commit_payload(&key, 8, inclusion());
    let pre = cbor::encode(&PreErratumPayload::of(&now)).expect("the old shape encodes");
    let post = cbor::encode(&now).expect("the new shape encodes");

    println!(
        "A2_WIRE pre_bytes={} post_bytes={} delta={}",
        pre.len(),
        post.len(),
        post.len() - pre.len()
    );
    assert!(
        post.len() > pre.len(),
        "a canonical map with one more key is more bytes; if this is equal the shadow declaration \
         above has drifted and is no longer the pre-erratum shape"
    );
    let holds =
        |bytes: &[u8], needle: &str| bytes.windows(needle.len()).any(|w| w == needle.as_bytes());
    assert!(
        !holds(&pre, "engine_version"),
        "the pre-erratum bytes carry no such key"
    );
    assert!(holds(&post, "engine_version"), "this build's bytes do");

    let decoded: ReceiptPayload = cbor::decode(&pre).expect(
        "🔴 a receipt written before this erratum has to decode. If this line is the failure, \
         `ReceiptPayload::engine_version` has lost its `#[serde(default)]`",
    );
    assert_eq!(
        decoded.engine_version, None,
        "absent bytes read as an absent answer -- \"predates the field\", not a fabricated version \
         string and not the version of whatever build happens to be decoding"
    );
    assert_eq!(
        ReceiptPayload {
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

/// Both kinds carry it, with no kind-dependent rule -- the question is asked of the process that
/// signed, and the process that signs a verdict receipt at T-4a is the one that signs the commit
/// receipt at T-11.
#[test]
fn a2_both_receipt_kinds_carry_the_seat() {
    let key = keypair(81);
    let v = verdict_payload(VerdictKind::Admit, &key, 9);
    let c = commit_payload(&key, 9, inclusion());
    println!(
        "A2_KINDS verdict={:?} commit={:?}",
        v.engine_version, c.engine_version
    );
    assert_eq!(v.engine_version.as_deref(), Some(FIXTURE_ENGINE_VERSION));
    assert_eq!(c.engine_version.as_deref(), Some(FIXTURE_ENGINE_VERSION));
}

// ---------------------------------------------------------------------------
// 🔴 The limit, pinned rather than described
// ---------------------------------------------------------------------------

/// 🔴 **What A2 does not close.** The value is the workspace version, and it does not distinguish
/// two builds of it.
///
/// This test would be strange in most codebases: it asserts that a field is *not yet* as good as it
/// sounds. It is here because `docs/LIMITS.md`, this workspace's own posture, and `req/38` SS854's
/// finding that the sharp edges collect in the interior and the defects collect at the boundary all
/// point the same way — a claim that decays quietly is worse than a gap that is written down. If a
/// later lane mints a real build identity, this test is the one that goes red, and going red is
/// how it asks to be rewritten with the ruling that authorised the change.
///
/// The assertion is deliberately about the **shape** of the source, not about the literal `0.1.0`:
/// pinning the number would make an ordinary version bump look like the defect being described.
#[test]
fn a2_the_value_is_a_crate_version_and_not_a_build_identity() {
    let src = include_str!("../../gx-engine/src/lib.rs");
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with("pub const VERSION"))
        .expect("gx-engine declares the constant the receipt's live seats read");
    println!("A2_SOURCE_OF_TRUTH={}", line.trim());
    assert!(
        line.contains("env!(\"CARGO_PKG_VERSION\")"),
        "🔴 the engine version a receipt carries is the crate version and nothing more. Two builds \
         of the same crate version are indistinguishable in a signed receipt, so `#435`'s \"which \
         implementation answered\" is narrowed and not answered. If this line no longer holds, a \
         build identity has landed: re-read `receipt.rs`'s doc comment on `engine_version` for the \
         three grounds this lane rejected a build script on, and rewrite this test with the ruling \
         that overturned them"
    );

    // And the workspace really does hold no build script, which was ground (1) for that rejection.
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("gx-witness sits two levels under the workspace root");
    let mut scripts: Vec<String> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                // `target/` holds other people's build scripts; this claim is about ours.
                if name != "target" && name != ".git" && name != "node_modules" {
                    stack.push(path);
                }
            } else if name == "build.rs" {
                scripts.push(path.display().to_string());
            }
        }
    }
    println!("A2_BUILD_SCRIPTS={scripts:?}");
    assert!(
        scripts.is_empty(),
        "🔴 a build script exists now, so ground (1) for rejecting a git-hash source is gone. This \
         is a finding rather than a failure: re-open the decision in `receipt.rs`'s doc comment \
         rather than deleting this assertion. Found: {scripts:?}"
    );
}
