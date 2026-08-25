// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/493` §1 AC-6** — the `confinement` context, on the face of a receipt.
//!
//! # What was not landed, in `req/497` §7's own words
//!
//! > `Confinement::to_json()` emits `{kernel_confined, ruleset_hash, …}` as a **third fact
//! > orthogonal** to `enforced` / `record_only`, and it is fixed by tests. **But it is not on a real
//! > receipt.** … `gx confine` is a launcher: it takes the ruleset and becomes another program by
//! > `exec`. What makes a receipt is `gx commit`, afterwards.
//!
//! `req/38` §283 ruling 5 queued that as the remaining item of S③. The seat is
//! [`ReceiptPayload::confinement`] and this file is the half of AC-6 that lives where the schema
//! does: the two impossible pairs are refused, the two legal shapes verify, and the byte-level
//! promise made to receipts that predate the erratum is measured rather than asserted in prose.
//!
//! The other half — that the value on a receipt gx *issues* came from a kernel and not from a
//! literal — is not measurable here, because this crate issues no commit. It is measured in
//! `crates/gx-engine/tests/confinement_receipt.rs` (the producer and the rebuild road) and in
//! `crates/gx-cli/tests/confine_receipt.rs` (the environment road `gx confine` opens).
//!
//! # `req/493` §1 AC-4, applied to a schema rule
//!
//! > a gate that has been introduced and never fired does not close its DoD.
//!
//! Each of the two refusals below is built a bed that makes it false and the refusal is asserted on
//! that bed. A rule with no bed is a comment.

mod support;

use gx_canon::cbor;
use gx_core::{
    Cid, DeterminismBoundary, FingerprintBytes, InclusionProof, KeyId, Reversibility,
    TransformationId, VerdictKind,
};
use gx_witness::receipt::{
    verify_offline, ConfinementContext, ReadSet, ReceiptKind, ReceiptPayload, VerdictSummary,
};
use gx_witness::Error;

use support::{commit_payload, issue, keypair, verdict_payload};

/// A one-leaf proof: 43 T-11's shape for the first commit a log holds. Spelled here because the
/// shared fixture takes one rather than building one.
fn inclusion() -> InclusionProof {
    InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    }
}

/// A ruleset hash of the shape `gx_confine::ConfinePlan::ruleset_hash` mints: `gx-canon`'s text
/// form of a `Leaf`-domain digest. Spelled here rather than imported, because `gx-witness` does not
/// name `gx-confine` and must not start: the payload carries a `String` precisely so that the crate
/// that owns the signature does not depend on the crate that owns the kernel call.
fn ruleset_hash() -> String {
    gx_canon::cid::to_text(&gx_canon::cid::mint(
        gx_canon::cid::Domain::Leaf,
        &[b"face\tdeclared\nwrite\t/srv/workspace\n"],
    ))
}

// ---------------------------------------------------------------------------
// The two pairs that are not states of the world
// ---------------------------------------------------------------------------

/// 🔴 A receipt may not say the kernel held it and decline to say what held it.
///
/// The bed: `kernel_confined: true` with no hash. `ConfinePlan::ruleset_hash` exists so that a
/// reader can re-derive the answer from a pre-image they were shown; a `true` carried without it is
/// the claim with the evidence removed, and the reader has no way to tell it from a `true` somebody
/// typed.
#[test]
fn ac6_a_confinement_that_names_no_ruleset_is_refused() {
    let key = keypair(41);
    let payload = ReceiptPayload {
        confinement: Some(ConfinementContext {
            kernel_confined: true,
            ruleset_hash: None,
        }),
        ..verdict_payload(VerdictKind::Admit, &key, 1)
    };
    let refusal = payload.check_schema();
    println!("AC6_SCHEMA no_ruleset={refusal:?}");
    let Err(Error::Schema { detail }) = refusal else {
        panic!("a confinement with no ruleset named is not a state of the world: {refusal:?}");
    };
    assert!(
        detail.contains("ruleset"),
        "the refusal has to name the missing half: {detail}"
    );
    // 🔴 And the refusal is not only advisory: `Receipt::issue` checks the schema *before* it
    // signs, so this payload cannot be given a valid signature. Without this line the rule would
    // hold for a caller who asked and not for the road that mints.
    assert!(
        gx_witness::receipt::Receipt::issue(&payload, support::issued_at(), &key).is_err(),
        "a payload the schema refuses may not be signed"
    );
}

/// 🔴 A receipt may not name a ruleset the kernel did not take.
///
/// The bed: a real hash beside `kernel_confined: false`. This is not a hypothetical shape —
/// `gx confine --plan-only` derives a plan and enforces nothing, and `gx_confine::apply` answers
/// `FaceStatus::NotEnforced` on a kernel without Landlock while the plan is still in hand. What is
/// refused is carrying that hash onto a receipt as though it had held.
#[test]
fn ac6_a_ruleset_the_kernel_did_not_take_is_refused() {
    let key = keypair(42);
    let payload = ReceiptPayload {
        confinement: Some(ConfinementContext {
            kernel_confined: false,
            ruleset_hash: Some(ruleset_hash()),
        }),
        ..verdict_payload(VerdictKind::Admit, &key, 2)
    };
    let refusal = payload.check_schema();
    println!("AC6_SCHEMA unheld_ruleset={refusal:?}");
    let Err(Error::Schema { detail }) = refusal else {
        panic!("a ruleset named by an unconfined receipt is not a state of the world: {refusal:?}");
    };
    assert!(
        detail.contains("plan-only") || detail.contains("held"),
        "the refusal has to say what the legal reading of a derived-but-unapplied plan is: {detail}"
    );
    assert!(
        gx_witness::receipt::Receipt::issue(&payload, support::issued_at(), &key).is_err(),
        "a payload the schema refuses may not be signed"
    );
}

/// The other two combinations are legal, on both kinds, and survive a real verification.
///
/// The negative control for the two tests above: without this, a `check_schema` that refused
/// *every* confinement would pass both of them. The pair is the point.
#[test]
fn ac6_the_two_legal_shapes_verify_on_both_kinds() {
    let key = keypair(43);
    let confined = ConfinementContext {
        kernel_confined: true,
        ruleset_hash: Some(ruleset_hash()),
    };
    for context in [ConfinementContext::unconfined(), confined] {
        let verdict = ReceiptPayload {
            confinement: Some(context.clone()),
            ..verdict_payload(VerdictKind::Admit, &key, 3)
        };
        verdict.check_schema().expect("a legal shape");
        let receipt = issue(&verdict, &key);
        verify_offline(&receipt, &key.verifying(), None).expect("it verifies");

        let commit = ReceiptPayload {
            confinement: Some(context.clone()),
            ..commit_payload(&key, 3, inclusion())
        };
        commit
            .check_schema()
            .expect("a legal shape on the other kind too");
        println!(
            "AC6_LEGAL kernel_confined={} hash_present={}",
            context.kernel_confined,
            context.ruleset_hash.is_some()
        );
    }
}

/// 🔴 The seat is **orthogonal**, and that is a claim about pairs rather than about a field.
///
/// `req/493` §0 words it as "a third fact orthogonal to the existing two values `enforced` /
/// `record_only`". A field that were secretly a value of `enforced` would show up as a combination
/// the schema refuses; all four combinations are legal, and this is what says so.
#[test]
fn ac6_confinement_is_orthogonal_to_enforced() {
    let key = keypair(44);
    let mut legal = 0;
    for enforced in [true, false] {
        for confined in [true, false] {
            let payload = ReceiptPayload {
                enforced,
                confinement: Some(ConfinementContext {
                    kernel_confined: confined,
                    ruleset_hash: confined.then(ruleset_hash),
                }),
                ..verdict_payload(VerdictKind::Admit, &key, 4)
            };
            payload
                .check_schema()
                .unwrap_or_else(|e| panic!("enforced={enforced} confined={confined}: {e:?}"));
            legal += 1;
        }
    }
    println!("AC6_ORTHOGONAL legal_pairs={legal}");
    assert_eq!(legal, 4, "all four combinations of the two bits are states of the world: gx can check while the kernel does not, and the kernel can hold a process gx is only recording");
}

// ---------------------------------------------------------------------------
// The wire, and the promise made to bytes that predate the erratum
// ---------------------------------------------------------------------------

/// `ReceiptPayload` as it stood before this erratum: the same fifteen members, and no
/// `confinement` key.
///
/// A shadow declaration rather than a stored blob, for `req/540` R-2b's reason one layer along: a
/// golden file is a thing that can be regenerated by the hand that broke it, and this cannot —
/// serde writes the field names off *this* declaration, so the pre-erratum bytes are produced by
/// the pre-erratum shape and by nothing else. Encoded through the same canonical encoder, so key
/// order is the encoder's and not this file's.
#[derive(serde::Serialize)]
struct PreErratumPayload {
    key_id: KeyId,
    verdict: Option<VerdictSummary>,
    enforced: bool,
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

/// 🔴 **`req/38` §294 ruling 2, honoured on the way in rather than repaired on the way out.**
///
/// §294 measured what DR-46-28 cost: a member added **required with no default** stopped a receipt
/// this product issued in August 2026 from decoding at all — signature valid, anchor valid, `decode`
/// dead — which is a break of the pillar that says a receipt is checkable for ever without its
/// issuer. The remedy it named is `Option` + a serde default, and this erratum is born in that
/// shape. So the claim is: bytes with **no** `confinement` key still decode, and they decode to
/// `None` rather than to a fabricated `false`.
///
/// Everything else about the value has to survive the round trip too, which is what makes this a
/// compatibility test rather than a test that a `None` is a `None`.
#[test]
fn ac6_bytes_with_no_confinement_key_still_decode() {
    let key = keypair(45);
    let now = commit_payload(&key, 5, inclusion());
    let pre = cbor::encode(&PreErratumPayload::of(&now)).expect("the old shape encodes");
    let post = cbor::encode(&now).expect("the new shape encodes");

    println!(
        "AC6_WIRE pre_bytes={} post_bytes={} delta={}",
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
        !holds(&pre, "confinement"),
        "the pre-erratum bytes carry no such key"
    );
    assert!(holds(&post, "confinement"), "this build's bytes do");

    let decoded: ReceiptPayload = cbor::decode(&pre).expect(
        "🔴 `req/38` §294 ruling 2: a receipt written before this erratum has to decode. If this \
         line is the failure, `ReceiptPayload::confinement` has lost its `#[serde(default)]` and \
         this erratum has just done to August's receipts what DR-46-28 did",
    );
    assert_eq!(
        decoded.confinement, None,
        "absent bytes read as an absent answer, not as `kernel_confined: false` — which would be a \
         claim about a process nobody observed"
    );
    assert_eq!(
        ReceiptPayload {
            confinement: now.confinement.clone(),
            ..decoded
        },
        now,
        "nothing but the one key moved"
    );
}

/// 🔴 The `#[serde(default)]` above cannot be quietly removed.
///
/// The shape `crates/gx-witness/tests/frozen_receipt_corpus.rs` uses for the two members
/// `docs/LIMITS.md` declares, pointed the other way: that file refuses a member that *gains* a
/// default, this refuses the one member whose default is the compatibility promise *losing* it. The
/// source is read rather than the behaviour inferred, because a decode test would go on passing for
/// a while after the attribute went away — serde only complains when a key is missing, and every
/// receipt this build writes carries one.
#[test]
fn ac6_the_compatibility_default_is_declared_in_the_source() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt.rs"),
    )
    .expect("the payload's own module is here");
    let at = source
        .find("pub confinement: Option<ConfinementContext>,")
        .expect("`ReceiptPayload` declares the seat `req/493` §1 AC-6 asks for");
    let above = &source[at.saturating_sub(200)..at];
    println!(
        "AC6_DEFAULT declared={}",
        above.contains("#[serde(default)]")
    );
    assert!(
        above.contains("#[serde(default)]"),
        "🔴 `req/38` §294 ruling 2: the seat's serde default is the whole of what keeps August \
         2026's receipts decodable. Removing it adds a third document to a limit `docs/LIMITS.md` \
         declares as two"
    );
}
