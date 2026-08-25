// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The two types M3 hand 1 moved down here, and the rulings that moved them.
//!
//! **E-M3-1** (`PlannedDeltaBytes`) and **E-M3-2** (`VerdictKind`), both from
//! `req/38_ERRATA_2026-08-07.md` §19. Same file shape as `m2_types.rs`, which holds M2's four for
//! the same reason: a type that exists because a ruling put it here should have a test that names
//! the ruling, or the reason is only in a commit message.
//!
//! No acceptance criterion is claimed. 34 gives neither type an AC — `VerdictKind` appears in no
//! AC at all, and `PlannedDelta` only inside AC-016's `GateInput` literal, which is M2's.

use gx_core::{PlannedDeltaBytes, VerdictKind};

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialise");
    serde_json::from_str(&json).expect("deserialise")
}

// ---------------------------------------------------------------------------
// E-M3-1: the opaque planned-delta carrier
// ---------------------------------------------------------------------------

/// Any byte string is a legal value, including none.
///
/// 42 §3.4 makes `payload` "an opaque change description that only the adapter interprets" (sem:
/// SEM-gx-core-178), so this type has no notion
/// of a malformed value — that judgement belongs to M4's adapter and to nothing below it.
#[test]
fn a_planned_delta_carrier_holds_any_bytes() {
    for bytes in [
        vec![],
        vec![0x00],
        vec![0xff; 4096],
        b"{\"op\":\"write\"}".to_vec(),
    ] {
        let carried = PlannedDeltaBytes(bytes.clone());
        assert_eq!(carried.0, bytes);
        assert_eq!(carried, PlannedDeltaBytes(bytes));
    }
}

/// Its `Debug` says the length and not the content (P-6).
///
/// The same refusal `FingerprintBytes` makes, and here it is the stronger of the two: a payload is
/// an adapter's own encoding, so a `{:?}` in a log line would be the one place where "handled only
/// as a byte string" (sem: SEM-gx-core-179) quietly stopped being true. The length is not the
/// content and a reader debugging
/// an empty delta needs it.
#[test]
fn a_planned_delta_carrier_does_not_print_its_payload() {
    let carried = PlannedDeltaBytes(b"secret-ish".to_vec());
    let shown = format!("{carried:?}");
    assert_eq!(shown, "PlannedDeltaBytes(opaque, 10 bytes)");
    assert!(
        !shown.contains("secret"),
        "the payload reached a debug format: {shown}"
    );
}

/// It is **not** a `PlannedDelta`, and the difference is one a caller can be bitten by.
///
/// 42 §1.3 gives `PlannedDelta` the IdentityView `{substrate, payload}`. This carrier holds one of
/// those two, so two equal values may describe changes to two different substrates. The type says
/// so in its documentation; this says so in a way that fails if the documentation is ever quietly
/// widened into a claim of delta equality.
#[test]
fn equality_is_byte_equality_and_not_delta_equality() {
    let a = PlannedDeltaBytes(b"same-bytes".to_vec());
    let b = PlannedDeltaBytes(b"same-bytes".to_vec());
    assert_eq!(a, b);

    // What is absent is the point: there is no substrate here to disagree about, so nothing in
    // this crate can tell an fs delta from a git delta carrying the same bytes. M4 supplies it.
    assert_eq!(
        std::mem::size_of::<PlannedDeltaBytes>(),
        std::mem::size_of::<Vec<u8>>(),
        "the carrier is exactly one field; a second would be a type M4 has not defined yet"
    );
}

// ---------------------------------------------------------------------------
// E-M3-2: the verdict discriminant
// ---------------------------------------------------------------------------

/// Three, in 42 §3.10's order, declared once.
#[test]
fn verdict_kind_has_the_three_42_3_10_names() {
    assert_eq!(
        VerdictKind::ALL,
        [VerdictKind::Admit, VerdictKind::Deny, VerdictKind::Escalate]
    );
    assert_eq!(
        VerdictKind::ALL.map(VerdictKind::as_str),
        ["Admit", "Deny", "Escalate"]
    );
}

/// The two faces of the enum agree: what serde writes is what `as_str` returns.
///
/// This is the assertion that stops the type from drifting away from 42 §3.10's wire spelling. A
/// `#[serde(rename)]` added later, or a variant renamed in Rust alone, breaks it — and a receipt
/// written by an older build would then decode into a different verdict, which is the one failure
/// mode a signed transparency record cannot have.
#[test]
fn the_wire_spelling_is_the_declared_spelling() {
    for kind in VerdictKind::ALL {
        let json = serde_json::to_string(&kind).expect("serialise");
        assert_eq!(json, format!("\"{}\"", kind.as_str()));
        assert_eq!(round_trip(&kind), kind);
        assert_eq!(kind.to_string(), kind.as_str(), "Display follows as_str");
    }
}

/// A fourth spelling does not decode.
///
/// H5-8's string check refused these at verification; the type refuses them at decode, which is
/// what "replace the string check by a type check" (sem: SEM-gx-core-180) buys. Case matters, and
/// so does an exact match.
#[test]
fn a_fourth_spelling_is_refused_by_the_decoder() {
    for bad in [
        "\"Admitted\"",
        "\"admit\"",
        "\"Allow\"",
        "\"\"",
        "0",
        "null",
    ] {
        assert!(
            serde_json::from_str::<VerdictKind>(bad).is_err(),
            "{bad} decoded as a verdict kind"
        );
    }
}
