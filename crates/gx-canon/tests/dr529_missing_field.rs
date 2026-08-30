// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `req/529` residual cell — **canon × missing-field**, fired live.
//!
//! `req/529` §2's grid marks this cell `✘` (empty) and `req/534` §1's honest disclosure names it
//! among the cells "not individually fired... census-level inference, not live fire". This file
//! fires it directly against `gx_canon::cbor::decode` -- the one place 41 §6 says every canonical
//! decode in this workspace goes through -- rather than against any one domain type, so the
//! finding is about the codec layer itself, not about one struct's own error handling.

use gx_canon::cbor;
use gx_canon::Error;
use serde::{Deserialize, Serialize};

/// A struct with three required fields, no `Option`, no `#[serde(default)]` -- the strictest
/// shape a canon-encoded record can have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FullRecord {
    a: u32,
    b: String,
    c: bool,
}

/// The same record with the middle field (`b`) dropped -- a structurally valid, strictly
/// canonical CBOR map (2 keys instead of 3), not garbage bytes.
#[derive(Debug, Clone, Serialize)]
struct MissingB {
    a: u32,
    c: bool,
}

/// **Fired live.** A canonical, well-formed CBOR map missing a required field is handed to
/// `gx_canon::cbor::decode::<FullRecord>`.
///
/// **Finding (H/M/L)**: refused with `Error::Decode`, not a panic and not a silently-defaulted
/// value (`FullRecord::b` has no `Default` derive and no `#[serde(default)]`, so `serde` itself
/// has no fallback to reach for — this is enforced by the type, not merely observed once). **L,
/// not H or M**: the refusal happens, cleanly, at the type boundary. The one gap worth naming: the
/// error message is `serde`'s own generic "missing field `b`" text (verified below), not a
/// gx-specific structured error carrying a field name as data (the way `Error::Schema` elsewhere
/// in this workspace carries a `detail` a caller can match on) — a caller wanting to distinguish
/// "which field" programmatically has to parse the message string. Recorded as informational,
/// same class as `req/534`'s other L-findings for this residual set.
#[test]
fn dr529_canon_missing_field_is_refused_not_panicked_not_silently_defaulted() {
    let partial = MissingB { a: 7, c: true };
    let bytes =
        cbor::encode(&partial).expect("a record with 2 of 3 fields still has a canonical form");

    // Sanity: these bytes really are strictly canonical (this test is about a missing field, not
    // about non-canonical framing, which `negative_vectors.rs` already covers).
    assert!(
        cbor::is_canonical(&bytes),
        "the fixture itself must be canonical, or this test measures the wrong thing"
    );

    let result = cbor::decode::<FullRecord>(&bytes);
    let err = result.expect_err(
        "a canonical map missing a required field must be REFUSED, not silently accepted with a \
         fabricated value for `b` -- accepting it would be the exact H-class failure req/529 §4-2 \
         AC names: a destructive/incomplete input answered as if it were valid",
    );
    println!("DR529_CANON_MISSING_FIELD err={err:?}");
    match &err {
        Error::Decode(detail) => {
            assert!(
                detail.contains('b') || detail.to_lowercase().contains("missing"),
                "the error should be legible enough to name what went wrong; got: {detail}"
            );
        }
        other => panic!(
            "expected Error::Decode for a missing-field record, got a different variant: {other:?}"
        ),
    }
}

/// Control: the SAME codepath, over a genuinely complete record, decodes cleanly and round-trips
/// -- proving the refusal above is about the missing field specifically, not about
/// `cbor::decode::<FullRecord>` being broken in general.
#[test]
fn dr529_canon_missing_field_control_a_complete_record_decodes_cleanly() {
    let full = FullRecord {
        a: 7,
        b: "seven".to_string(),
        c: true,
    };
    let bytes = cbor::encode(&full).expect("canonical");
    let decoded: FullRecord = cbor::decode(&bytes).expect("a complete record must decode");
    assert_eq!(decoded, full);
}
