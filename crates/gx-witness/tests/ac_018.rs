// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-018 (FR-018, VerdictReceipt) — a stranger with no ledger can check all three verdicts.
//! (sem: SEM-gx-witness-214, SEM-gx-witness-215, SEM-gx-witness-216, SEM-gx-witness-217,
//! SEM-gx-witness-218, SEM-gx-witness-219, SEM-gx-witness-220, SEM-gx-witness-221,
//! SEM-gx-witness-222, SEM-gx-witness-223)
//!
//! AC-018 verbatim: "Given: a `VerdictReceipt` corresponding to each of
//! `Verdict::Admit/Deny/Escalate` (a DSSE envelope carrying verdict, canonical CID, precondition
//! fingerprints; carrying no inclusion proof). When: verify with no ledger access, via an API
//! equivalent to `gx-cli receipt verify <receipt.json> --offline`. Then: signature verification and
//! the canonical CID consistency check succeed for all three verdict kinds (`Ok(true)`,
//! `checks.inclusion` is `"skipped"`)." Judgement method: `integration (3 cases)`, M2.
//!
//! "an equivalent API" is the AC's own words and is what req/49 §1 N-05 records: 51 §8 puts AC-018
//! among the criteria "at M1/M2/M5, ... verified via direct library API calls". So the subject is
//! [`verify_offline`], and the CLI round trip is AC-054/AC-057 in M6.
//!
//! # The three mappings this file makes explicit
//!
//! * `Ok(true)` → `Ok(checks)` with [`Checks::verified`] true. A `Checks` exists only when the
//!   signature verified (AC-019 makes a bad one an `Err`), so the boolean covers what is left.
//! * `checks.inclusion == "skipped"` → [`InclusionCheck::NotApplicable`]. A `VerdictReceipt` has no
//!   proof by ASM-14, so nothing was skipped for want of an anchor -- there was nothing to check.
//!   The distinction is [`InclusionCheck::Unanchored`]'s, which is a `CommitReceipt`'s case and is
//!   **not** a pass.
//! * "with no ledger access" → `anchor: None` and no `gx_log` value anywhere in the call.

mod support;

use gx_canon::cbor;
use gx_core::VerdictKind;
use gx_witness::receipt::{verify_offline, InclusionCheck, ReceiptKind, ReceiptPayload};
use gx_witness::Error;
use support::{issue, keypair, verdict_payload};

// ---------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------

/// AC-018 verbatim: all three verdicts, verified with no ledger.
#[test]
fn ac_018_all_three_verdicts_verify_offline() {
    let key = keypair(1);
    for (n, kind) in VerdictKind::ALL.iter().enumerate() {
        let payload = verdict_payload(*kind, &key, n as u64);
        let receipt = issue(&payload, &key);

        let checks = verify_offline(&receipt, &key.verifying(), None)
            .unwrap_or_else(|e| panic!("a {kind} receipt did not verify: {e}"));

        assert!(
            checks.verified(),
            "{kind}: Ok(true) expected, got {checks:?}"
        );
        assert!(
            checks.canonical_cid,
            "{kind}: the canonical CID check failed"
        );
        assert_eq!(
            checks.inclusion,
            InclusionCheck::NotApplicable,
            "{kind}: `checks.inclusion` is not AC-018's `skipped`"
        );
        assert_eq!(checks.key_id, *key.key_id());
    }
}

/// The three kinds 42 §3.10 admits are exactly the three AC-018 names, and a fourth cannot be
/// spelled at all (**E-M3-2**, H5-8 matured).
///
/// # What changed, and what the test now measures
///
/// Until M3 hand 1 this read `receipt.rs`'s `pub const VERDICT_KINDS: [&str; 3]` and asserted that
/// `check_schema` refused `"Admitted"`, `"admit"`, `"Allow"` and `""`. The field is
/// [`gx_core::VerdictKind`] now, so three of those four **do not compile** -- which is a stronger
/// statement than the one this test used to make, and an untestable one. What remains testable is
/// the case a type cannot reach: bytes arriving from somewhere else. So the refusal is measured at
/// the decoder, on a payload encoded with a fourth spelling in the `kind` slot.
///
/// The list is still read rather than restated: `VerdictKind::ALL` is the single declaration, and
/// `as_str` is checked against serde's own output below so the two faces of the enum cannot drift.
#[test]
fn ac_018_a_verdict_kind_42_does_not_admit_is_refused() {
    assert_eq!(
        VerdictKind::ALL.map(VerdictKind::as_str),
        ["Admit", "Deny", "Escalate"],
        "42 §3.10's three spellings"
    );

    let key = keypair(2);
    let good = cbor::encode(&verdict_payload(VerdictKind::Admit, &key, 0)).expect("canonical");
    assert!(
        cbor::decode::<ReceiptPayload>(&good).is_ok(),
        "the control case has to decode, or the negatives below prove nothing"
    );

    for bad in ["Admitted", "admit", "Allow", ""] {
        // 42 §3.10 spells the kind as a text string, so a fourth one is a legal CBOR document and
        // an illegal receipt -- exactly the input a decoder is the right place to refuse.
        let tampered = replace_text(&good, "Admit", bad);
        assert!(
            cbor::decode::<ReceiptPayload>(&tampered).is_err(),
            "{bad:?} was accepted as a verdict kind by the decoder"
        );
    }
}

/// Replace one canonical text string inside encoded bytes with another, header included.
///
/// A text string under 24 bytes is `0x60 | len` followed by the bytes (RFC 8949 §3), so swapping
/// one for another of a different length means rewriting the head byte too. Done by hand rather
/// than by re-encoding a struct, because the whole point is to produce a payload no `VerdictKind`
/// could have written.
fn replace_text(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let needle: Vec<u8> = std::iter::once(0x60 | u8::try_from(from.len()).expect("short"))
        .chain(from.bytes())
        .collect();
    let replacement: Vec<u8> = std::iter::once(0x60 | u8::try_from(to.len()).expect("short"))
        .chain(to.bytes())
        .collect();
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle.as_slice())
        .unwrap_or_else(|| panic!("{from:?} is not in the encoding"));

    let mut out = bytes[..at].to_vec();
    out.extend_from_slice(&replacement);
    out.extend_from_slice(&bytes[at + needle.len()..]);
    out
}

/// "with no ledger access", mechanically: the payload of a `VerdictReceipt` contains no inclusion
/// proof, and the verification is handed no anchor. Both halves, because a receipt that carried a
/// proof and a verifier that ignored it would satisfy neither.
#[test]
fn ac_018_a_verdict_receipt_carries_no_ledger_claim_at_all() {
    let key = keypair(3);
    let receipt = issue(&verdict_payload(VerdictKind::Deny, &key, 0), &key);
    let payload = receipt.payload().expect("the payload decodes");

    assert_eq!(payload.receipt_kind, ReceiptKind::VerdictReceipt);
    assert!(payload.inclusion_proof.is_none(), "ASM-14: always `None`");
    assert!(payload.postcondition_fingerprint.is_none());
    assert!(payload.inverse_delta.is_none());
}

/// What AC-018 lists as the receipt's contents -- "verdict, canonical CID, precondition
/// fingerprints" -- survives the round trip through the bytes that were signed.
#[test]
fn ac_018_the_three_things_the_ac_names_survive_the_envelope() {
    let key = keypair(4);
    let payload = verdict_payload(VerdictKind::Escalate, &key, 9);
    let receipt = issue(&payload, &key);

    let back = receipt.payload().expect("the payload decodes");
    assert_eq!(
        back, payload,
        "the payload did not survive its own encoding"
    );
    assert_eq!(
        back.verdict.as_ref().map(|v| v.kind),
        Some(VerdictKind::Escalate),
        "E-M5-11 made the seat optional; a receipt issued for a verdict still fills it"
    );
    assert_eq!(back.canonical_cid, payload.transformation.0);
    assert_eq!(
        back.precondition_fingerprint,
        payload.precondition_fingerprint
    );
}

/// A receipt naming one key and verified against another is refused even when that other key really
/// signed it. 42 §3.10 requires `key_id` to match `DsseSignature.keyid`, and this is the only check
/// that notices a signature moved onto a receipt that names somebody else.
#[test]
fn ac_018_a_receipt_that_names_another_key_is_refused() {
    let signer = keypair(5);
    let other = keypair(6);

    let mut payload = verdict_payload(VerdictKind::Admit, &signer, 0);
    payload.key_id = other.key_id().clone();
    let receipt = issue(&payload, &signer);

    match verify_offline(&receipt, &signer.verifying(), None) {
        Err(Error::Schema { detail }) => assert!(detail.contains("key")),
        other => panic!("expected a schema refusal, got {other:?}"),
    }
}

/// A receipt signed by a key nobody offered does not verify against a key that did not sign it.
/// The trivial case, and the one a verifier that ignored `signature_for` would fail.
#[test]
fn ac_018_a_signature_from_a_different_key_does_not_verify() {
    let signer = keypair(7);
    let stranger = keypair(8);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &signer, 0), &signer);

    assert!(matches!(
        verify_offline(&receipt, &stranger.verifying(), None),
        Err(Error::SignatureInvalid { .. })
    ));
}

// ---------------------------------------------------------------------------
// The field table, counted against the spec rather than against a memory of it
// ---------------------------------------------------------------------------

/// 42 §3.10's `ReceiptPayload` table has **sixteen** rows -- which is the number req/49 counted
/// for a different eleven, and the coincidence is worth naming so nobody reads it as agreement.
///
/// 🔴 **DR-46-24(A)** — eleven until the read-set erratum (`req/38` §236 ruling 2). Two rows were
/// added in one window: `read_set` and `fingerprint_scope`. This test is the reason they had to be
/// added to the spec and the struct in the same commit — it reads the canon file, so a struct that
/// grew without the document says so.
///
/// req/49 §2.1 and §3 M2-9 both say "15 fields" and E-M2-7 was written against that number. The
/// ruling stands either way -- `fail_posture_engaged` is absent from all eleven -- but a number
/// nobody measured is a number that gets carried forward, so this reads the spec file. Raised in
/// req/54 §4; the same shape as M1's A-3.
#[test]
fn ac_018_the_receipt_payload_table_has_the_rows_the_spec_has() {
    let spec = include_str!("../../../req/spec/40-architecture/42-data-model.md");
    // (sem: SEM-gx-witness-222) untranslated: this literal must byte-match a heading inside
    // req/spec/40-architecture/42-data-model.md (untouchable canon), so it stays in the spec's own
    // Japanese rather than being translated.
    let table = spec
        .split("| `ReceiptPayload`フィールド | 型 | 説明 |")
        .nth(1)
        .expect("42 §3.10 has a ReceiptPayload table")
        .split("### 3.11")
        .next()
        .expect("split always yields one");

    let rows: Vec<&str> = table
        .lines()
        .filter(|l| l.starts_with("| `") && l.contains('|'))
        .collect();
    assert_eq!(
        rows.len(),
        17,
        "42 §3.10's table changed shape; the rows found were {rows:#?}"
    );
    // 🔴 **DR-46-39** (`req/38` §5689, `req/777`) — the sixth row an erratum has added to this
    // table, named for the reason the rest are.
    assert!(
        rows.iter().any(|r| r.contains("`catalogue_hash`")),
        "DR-46-39's catalogue-hash row is missing from 42 §3.10"
    );
    // 🔴 **S③ / `req/493` §1 AC-6** — the fifth row an erratum has added to this table, named for
    // the reason the rest are: a count restored by deleting one row and adding another is a count
    // that measured nothing.
    assert!(
        rows.iter().any(|r| r.contains("`confinement`")),
        "`req/493` §0's confinement row is missing from 42 §3.10"
    );
    // 🔴 **DR-46-28** — fourteen until this window, and named for the reason the rest are.
    assert!(
        rows.iter().any(|r| r.contains("`determinism_boundary`")),
        "DR-46-28's boundary row is missing from 42 §3.10"
    );
    // 🔴 **DR-46-26** — thirteen until this window. Asserted by name for the reason the struct
    // assertion below gives: a count restored by deleting one row and adding another is a count
    // that measured nothing.
    assert!(
        rows.iter().any(|r| r.contains("`reversibility`")),
        "DR-46-26's inverse-status row is missing from 42 §3.10"
    );
    println!("SPEC_RECEIPT_PAYLOAD_ROWS={}", rows.len());

    // The two rulings that move the count, asserted against the same text.
    assert!(
        rows.iter().any(|r| r.contains("`issued_at`")),
        "E-M2-6 takes `issued_at` out of a table that has it"
    );
    assert!(
        !rows.iter().any(|r| r.contains("fail_posture_engaged")),
        "E-M2-7 adds a field the table does not have"
    );
}

/// Sixteen rows, minus `issued_at`, plus `fail_posture_engaged`, is what the struct has.
///
/// Counted off this crate's own source, so the arithmetic in the previous test lands on something
/// rather than being a remark about a document.
#[test]
fn ac_018_the_struct_is_the_table_as_the_errata_corrected_it() {
    let src = include_str!("../src/receipt.rs");
    let body = src
        .split("pub struct ReceiptPayload {")
        .nth(1)
        .expect("receipt.rs declares ReceiptPayload")
        .split("\n}")
        .next()
        .expect("split always yields one");
    let fields: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub ") && l.ends_with(','))
        .collect();

    assert_eq!(fields.len(), 17, "fields found: {fields:#?}");
    // 🔴 **DR-46-39** (`req/38` §5689, `req/777`) — the sixth row an erratum has added to this
    // table.
    assert!(
        fields.iter().any(|f| f.contains("catalogue_hash")),
        "DR-46-39's catalogue-hash seat is missing"
    );
    // 🔴 **S③ / `req/493` §1 AC-6** — the fifth row an erratum has added to this table.
    assert!(
        fields.iter().any(|f| f.contains("confinement")),
        "`req/493` §0's confinement seat is missing"
    );
    // 🔴 **DR-46-28** — the fourth row an erratum has added to this table.
    assert!(
        fields.iter().any(|f| f.contains("determinism_boundary")),
        "DR-46-28's boundary field is missing"
    );
    // 🔴 **DR-46-26** — the third row an erratum has added to this table.
    assert!(
        fields.iter().any(|f| f.contains("reversibility")),
        "DR-46-26's inverse-status field is missing"
    );
    // 🔴 **DR-46-24(A)** — the two rows the erratum added, asserted by name so that a future hand
    // cannot restore the count by deleting one of them and adding something else.
    assert!(
        fields.iter().any(|f| f.contains("read_set")),
        "DR-46-24(A)'s read-set is missing"
    );
    assert!(
        fields.iter().any(|f| f.contains("fingerprint_scope")),
        "P2's scope is missing"
    );
    assert!(
        fields.iter().any(|f| f.contains("fail_posture_engaged")),
        "E-M2-7's field is missing"
    );
    assert!(
        !fields.iter().any(|f| f.contains("issued_at")),
        "E-M2-6 puts `issued_at` outside the signed core"
    );
    println!("RECEIPT_PAYLOAD_FIELDS={}", fields.len());
}

// ---------------------------------------------------------------------------
// E-M2-1: the cycle is still absent
// ---------------------------------------------------------------------------

/// gx-log does not name gx-witness.
///
/// Hand 5 added the edge in the other direction (this crate calls `gx_log::proof`), so the sentence
/// "neither names the other" stopped being true and the half that matters had to become checkable
/// on its own. E-M2-1 forbade the **cycle**; cargo refuses one, and this says out loud which
/// direction is the forbidden one so that a future hand adding a `gx-witness` dependency to gx-log
/// meets a failing test rather than a compiler error it might work around.
#[test]
fn ac_018_gx_log_still_does_not_name_gx_witness() {
    let manifest = include_str!("../../gx-log/Cargo.toml");
    let code: String = manifest
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("gx-witness"),
        "gx-log names gx-witness; E-M2-1's cycle is back"
    );
}
