// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `ReceiptPayload` JSON round-trip -- the test req/38 SS858 §④ named and left unwritten
//! (the `ReceiptPayload` body test needs cargo, so SS858 named it and left it open rather than
//! writing it unrun), and
//! req/910 C7 / req/919 W1 ask to be identified and actually run.
//!
//! # Why this is a different claim than `tests/receipt_verdict_wire.rs` and `ac_018.rs`
//!
//! Those pin `ReceiptPayload`'s **CBOR** bytes (the real wire: SS858 §④ measured that the tree
//! has zero JSON-deserialisation call sites for a receipt -- the one input path is DSSE-signed
//! DAG-CBOR). This file asks the narrower, still-open question: is `serde`'s **JSON**
//! `Serialize`/`Deserialize` pair for `ReceiptPayload` itself representation-independent, the way
//! `gx-canon/tests/ac_013.rs` measured it for `Transformation`/`ObjectSnapshot` and SS858 §④
//! measured it for the *wrapper* `Receipt` (three receipts, keys reversed, whitespace moved,
//! answers byte-identical)? `ReceiptPayload` was never put through that same probe -- this closes
//! that gap, one type down from the wrapper.
//!
//! `respelled` below is a local re-derivation of `ac_013.rs`'s helper (same repo, same author, not
//! an external import: `gx-canon`'s copy lives in that crate's `tests/` and integration-test
//! modules do not link across crates). Not a CID/canon-form claim -- `ReceiptPayload` has no
//! `IdentityView`-through-`gx-canon` road on the JSON face, only on CBOR (`receipt.rs`'s own doc
//! comment on `IdentityView for ReceiptPayload`) -- so this measures `serde`'s parser, not
//! `gx_canon::cid`.

mod support;

use gx_core::VerdictKind;
use gx_witness::receipt::ReceiptPayload;
use serde_json::Value;
use support::{commit_receipt_in_a_log, degraded_payload, keypair, verdict_payload};

/// Write a JSON value out again with every object's keys in the opposite order and the
/// whitespace scattered through it -- a genuine second *representation*, not the same string
/// twice. Mirrors `gx-canon/tests/ac_013.rs::respelled`.
fn respelled(value: &Value, depth: usize) -> String {
    match value {
        Value::Object(map) => {
            let pad = "  ".repeat(depth + 1);
            let inner: Vec<String> = map
                .iter()
                .rev()
                .map(|(k, v)| {
                    format!(
                        "\n{pad}{} :  {}",
                        Value::String(k.clone()),
                        respelled(v, depth + 1)
                    )
                })
                .collect();
            format!("{{{}\n{}}}", inner.join(" ,"), "  ".repeat(depth))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(|v| respelled(v, depth + 1)).collect();
            format!("[ {} ]", inner.join(" , "))
        }
        scalar => scalar.to_string(),
    }
}

/// The whole claim, for one payload: plain JSON and re-spelled JSON both decode back to a value
/// equal (`ReceiptPayload: PartialEq, Eq`) to the original, the two decodes agree with each
/// other, and re-encoding the round-tripped value reproduces the original plain bytes exactly.
fn assert_json_round_trip(payload: &ReceiptPayload, label: &str) {
    let document = serde_json::to_value(payload).expect("a payload has a JSON form");
    let plain = serde_json::to_string(payload).expect("a payload serialises to a JSON string");
    let respelled_text = respelled(&document, 0);
    assert_ne!(
        plain, respelled_text,
        "{label}: the two JSON spellings must actually differ, or this test compares a string \
         with itself"
    );

    let from_plain: ReceiptPayload =
        serde_json::from_str(&plain).expect("{label}: the plain JSON decodes");
    let from_respelled: ReceiptPayload =
        serde_json::from_str(&respelled_text).expect("{label}: the respelled JSON decodes");

    assert_eq!(
        payload, &from_plain,
        "{label}: the plain JSON round trip changed the payload"
    );
    assert_eq!(
        payload, &from_respelled,
        "{label}: key order / whitespace changed the decoded payload"
    );
    assert_eq!(
        from_plain, from_respelled,
        "{label}: the two spellings decoded to different values"
    );

    // The byte-identity half of SS858 §④'s `Receipt` claim, moved one type down: re-encoding the
    // respelled-then-decoded value must reproduce the same plain-JSON bytes as the original.
    let replain = serde_json::to_string(&from_respelled)
        .expect("{label}: the round-tripped payload re-serialises");
    assert_eq!(
        plain, replain,
        "{label}: re-encoding after the round trip drifted from the original bytes"
    );
    println!(
        "RECEIPT_PAYLOAD_JSON_ROUND_TRIP label={label} plain_bytes={} respelled_bytes={}",
        plain.len(),
        respelled_text.len()
    );
}

/// A `VerdictReceipt` payload: `verdict: Some(..)`, `read_set`/`reversibility`/`inclusion_proof`
/// all `None` -- the branch of every `Option` field that the other two fixtures below do not
/// cover.
#[test]
fn receipt_payload_json_round_trip_verdict() {
    let key = keypair(21);
    let payload = verdict_payload(VerdictKind::Admit, &key, 21);
    assert_json_round_trip(&payload, "verdict");
}

/// 43 T-4e's degraded admission: `verdict: None`, `fail_posture_engaged: true` -- the
/// `Option<VerdictSummary>` branch `receipt_verdict_wire.rs` pins as `0xf6` on the CBOR face;
/// this is the same value asked of the JSON face instead.
#[test]
fn receipt_payload_json_round_trip_degraded() {
    let key = keypair(22);
    let payload = degraded_payload(&key, 22);
    assert_json_round_trip(&payload, "degraded");
}

/// A `CommitReceipt` payload taken back out of a *signed* receipt that a real `TileLog` issued an
/// inclusion proof for (`support::commit_receipt_in_a_log`) -- `read_set: Some(..)`,
/// `reversibility: Some(..)`, `determinism_boundary: Mixed { .. }`, `inclusion_proof: Some(..)`
/// all populated, so this is the densest of the three payloads and the one most likely to expose
/// a `serde` field that does not round-trip through JSON text.
#[test]
fn receipt_payload_json_round_trip_commit() {
    let key = keypair(23);
    let (receipt, _checkpoint) = commit_receipt_in_a_log(&key, 23, 3);
    let payload = receipt
        .payload()
        .expect("the signed receipt's payload decodes");
    assert_json_round_trip(&payload, "commit");
}
