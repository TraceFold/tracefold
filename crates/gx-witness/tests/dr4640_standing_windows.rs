// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! DR-46-40 — claim-standing retraction windows (`req/730`), AC-1..AC-5 measured, red-first.
//!
//! `StandingEntry`/`StandingLedger` generalize this crate's own `RevocationEntry`/
//! `RevocationLedger` (`keys.rs:652-887`) from one subject (a key) to any witnessed claim, per
//! `req/38` SS589/SS594's ruling and `req/730`'s reqdef. Every AC below is quoted from `req/730`
//! SS3, and every test is run against a real signed envelope, not a hand-built `Standing` value.

mod support;

use gx_canon::cbor;
use gx_core::Timestamp;
use gx_witness::dsse::{DsseEnvelope, STANDING_PAYLOAD_TYPE};
use gx_witness::keys::{Standing, StandingEntry, StandingLedger};
use gx_witness::KeyPair;

use support::keypair;

const SECOND: i64 = 1_000_000_000;

/// A close envelope built without this crate's cooperation -- an attacker's, the same shape
/// `revocation.rs::forged` uses.
fn forged(entry: &StandingEntry, signer: &KeyPair) -> DsseEnvelope {
    let mut envelope = DsseEnvelope {
        payload_type: STANDING_PAYLOAD_TYPE.to_string(),
        payload: cbor::encode(entry).expect("an entry has a canonical form"),
        signatures: Vec::new(),
    };
    envelope.sign(signer.signing_key(), signer.key_id());
    envelope
}

// ---------------------------------------------------------------------------
// AC-1 (red-first, negative control -- the binding condition itself)
// ---------------------------------------------------------------------------

/// **AC-1a, structural** — no hard-purge-shaped method exists on `StandingEntry`/`StandingLedger`:
/// a grep-shaped scan over the module's public API surface for a banned method name. This is the
/// SAME instrument `req/730` SS3 names ("grep-shaped, mirroring `grep -rl "declared gap"
/// crates/`'s style"), run against the live source rather than trusted from memory.
#[test]
fn ac1a_no_hard_purge_shaped_method_name_exists_in_the_source() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/keys.rs"),
    )
    .expect("keys.rs is readable");
    let start = source
        .find("struct StandingEntry")
        .expect("StandingEntry is declared in keys.rs");
    let region = &source[start..];
    let banned = [
        "fn purge",
        "fn delete",
        "fn clear",
        "fn remove",
        "fn reopen",
        "fn unclose",
    ];
    let mut hits = Vec::new();
    for word in banned {
        if region.contains(word) {
            hits.push(word);
        }
    }
    println!("DR4640_AC1A hits={hits:?}");
    assert!(
        hits.is_empty(),
        "a hard-purge-shaped method name exists on the claim-standing type: {hits:?} -- this is \
         exactly what AC-1 refuses"
    );
}

/// **AC-1b, behavioral** — a reopen-shaped attempt (a second, later-signed close for the same
/// claim, from an authority the verifier trusts, that a naive "take the latest" reading would let
/// win) does not move the effective standing backward, and the original entry object is
/// byte-identical before and after the attempt (Rust's ownership model gives this for free once
/// no mutating method exists, but the negative control asserts it rather than assumes it).
#[test]
fn ac1b_a_reopen_shaped_second_statement_does_not_move_the_boundary() {
    let key = keypair(1);
    let first = StandingEntry::new("claim-1", Timestamp(1_000 * SECOND), "closed at t=1000");
    let first_envelope = first.signed_by(&key).expect("signable");

    // The reopen-shaped attempt: a NEW claim, signed by the same trusted authority, with a
    // LATER closed_at -- if `StandingLedger` took the latest, this would look like it "moved"
    // the claim's boundary forward, which is exactly the shape AC-1 refuses to allow to matter.
    let second = StandingEntry::new("claim-1", Timestamp(2_000 * SECOND), "later, same claim");
    let second_envelope = second.signed_by(&key).expect("signable");

    let ledger =
        StandingLedger::from_signed(&[first_envelope.clone(), second_envelope], &key.verifying())
            .expect("both entries authenticate against the same authority");

    let effective = ledger.close_of("claim-1").expect("a close exists");
    assert_eq!(
        effective.closed_at.0,
        1_000 * SECOND,
        "AC-1b: a later-signed statement moved the effective boundary forward -- reopen-shaped \
         behavior leaked through"
    );

    // The original entry is unchanged -- re-decode the first envelope and compare.
    let redecoded: StandingEntry = cbor::decode(&first_envelope.payload).expect("decodes");
    assert_eq!(
        redecoded, first,
        "AC-1b: the original entry's bytes changed -- nothing may mutate a filed close"
    );
}

// ---------------------------------------------------------------------------
// AC-2 (earliest-close wins)
// ---------------------------------------------------------------------------

/// **AC-2** — given two signed close-statements for the same claim with different `closed_at`
/// timestamps, the ledger's standing query returns the earlier one. Negative control: a naive
/// "take the latest" implementation must fail this test -- asserted directly by checking the
/// returned value is the earlier, not merely that *a* value returned.
#[test]
fn ac2_earliest_close_wins_not_latest() {
    let key = keypair(2);
    let early = StandingEntry::new("claim-2", Timestamp(500 * SECOND), "first to close");
    let late = StandingEntry::new("claim-2", Timestamp(9_000 * SECOND), "later statement");

    // Both orderings: the result must not depend on which envelope was read first.
    for (a, b) in [(early.clone(), late.clone()), (late.clone(), early.clone())] {
        let envelopes = vec![
            a.signed_by(&key).expect("signable"),
            b.signed_by(&key).expect("signable"),
        ];
        let ledger =
            StandingLedger::from_signed(&envelopes, &key.verifying()).expect("both authenticate");
        let winner = ledger.close_of("claim-2").expect("a close exists");
        assert_eq!(
            winner.closed_at.0,
            500 * SECOND,
            "AC-2: the earliest close did not win (read order: {:?} then {:?})",
            a.closed_at.0,
            b.closed_at.0
        );
    }
}

// ---------------------------------------------------------------------------
// AC-3 (past-instant query is unaffected by a later close)
// ---------------------------------------------------------------------------

/// **AC-3** — a query for standing at instant `t < closed_at` returns the pre-close view (`Open`);
/// only a query for `t >= closed_at` returns `Closed`. Negative control: an implementation that
/// reads only the current standing and applies it retroactively to a historical query must fail
/// this test -- asserted by querying at three distinct instants around the boundary.
#[test]
fn ac3_a_past_instant_query_returns_the_pre_close_view() {
    let key = keypair(3);
    let entry = StandingEntry::new("claim-3", Timestamp(5_000 * SECOND), "closes at t=5000");
    let envelope = entry.signed_by(&key).expect("signable");
    let ledger = StandingLedger::from_signed(&[envelope], &key.verifying()).expect("authenticates");

    assert_eq!(
        ledger.standing_at("claim-3", Timestamp(4_999 * SECOND)),
        Standing::Open,
        "AC-3: an instant strictly before closed_at must read Open"
    );
    assert_eq!(
        ledger.standing_at("claim-3", Timestamp(5_000 * SECOND)),
        Standing::Closed,
        "AC-3: the instant AT closed_at must read Closed (closed_at is the boundary, inclusive)"
    );
    assert_eq!(
        ledger.standing_at("claim-3", Timestamp(5_001 * SECOND)),
        Standing::Closed,
        "AC-3: an instant after closed_at must read Closed"
    );
    assert_eq!(
        ledger.standing_now("claim-3"),
        Standing::Closed,
        "AC-3: 'as of now' must see an already-authenticated close"
    );
}

/// A claim with no close-statement at all reads `Open` at every instant, never `Closed` by
/// omission -- the absence-is-not-a-value discipline `req/730` §1a's own text draws.
#[test]
fn ac3_a_claim_with_no_close_statement_reads_open_at_every_instant() {
    let key = keypair(4);
    let unrelated = StandingEntry::new("claim-other", Timestamp(100 * SECOND), "not claim-4");
    let envelope = unrelated.signed_by(&key).expect("signable");
    let ledger = StandingLedger::from_signed(&[envelope], &key.verifying()).expect("authenticates");
    assert_eq!(ledger.standing_at("claim-4", Timestamp(0)), Standing::Open);
    assert_eq!(ledger.standing_now("claim-4"), Standing::Open);
}

// ---------------------------------------------------------------------------
// AC-4 (fail-closed on an unauthenticated close-statement)
// ---------------------------------------------------------------------------

/// **AC-4** — a close-statement not signed by the authority the verifier trusts is rejected, not
/// silently ignored, not silently applied. Negative control: an implementation that applies an
/// unsigned or wrongly-signed close-statement must fail this test.
#[test]
fn ac4_a_close_statement_signed_by_the_wrong_authority_is_rejected() {
    let trusted = keypair(5);
    let attacker = keypair(6);
    let entry = StandingEntry::new("claim-5", Timestamp(SECOND), "forged close");

    // `forged`: an envelope carrying the right entry and payload type, signed by a key the
    // verifier does NOT trust for this claim.
    let bogus = forged(&entry, &attacker);
    let err = StandingEntry::from_signed(&bogus, &trusted.verifying())
        .expect_err("AC-4: a wrongly-signed close-statement must be rejected, not accepted");
    println!("DR4640_AC4_SINGLE err={err:?}");

    // The ledger-level API must propagate the same rejection rather than silently dropping the
    // bad entry from an otherwise-successful ledger (the RevocationLedger precedent silently
    // ignores an entry about a DIFFERENT subject, which is not this case: this entry claims to be
    // about a claim this ledger's caller cares about and fails authentication).
    let good = StandingEntry::new("claim-6", Timestamp(2 * SECOND), "real close")
        .signed_by(&trusted)
        .expect("signable");
    let ledger_result = StandingLedger::from_signed(&[good, bogus], &trusted.verifying());
    assert!(
        ledger_result.is_err(),
        "AC-4: a ledger containing one forged entry must fail closed, not silently keep the good \
         one and drop the bad one"
    );
}

/// An entry with the wrong payload type (a receipt's or a revocation's bytes offered as a
/// standing close) is refused at the type-tag check, before a signature is even verified against
/// the wrong shape.
#[test]
fn ac4_wrong_payload_type_is_refused() {
    let key = keypair(7);
    let mut envelope = DsseEnvelope {
        payload_type: "application/vnd.glovrex.revocation+dagcbor".to_string(),
        payload: cbor::encode(&StandingEntry::new("claim-7", Timestamp(0), "x"))
            .expect("canonical"),
        signatures: Vec::new(),
    };
    envelope.sign(key.signing_key(), key.key_id());
    let err = StandingEntry::from_signed(&envelope, &key.verifying())
        .expect_err("a non-standing payload type must be refused");
    println!("DR4640_AC4_PAYLOAD_TYPE err={err:?}");
}

// ---------------------------------------------------------------------------
// AC-5 (no ReceiptPayload / DSSE-envelope-field contact)
// ---------------------------------------------------------------------------

/// **AC-5** — the claim-standing type is never a member of `ReceiptPayload`'s field list and never
/// a fourth field on `DsseEnvelope`, checked structurally against the live source (`req/730` SS2's
/// declaration, made machine-checked here rather than left as prose that could rot).
#[test]
fn ac5_no_contact_with_receipt_payload_or_the_dsse_envelope_fixed_fields() {
    let receipt_src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt.rs"),
    )
    .expect("receipt.rs is readable");
    assert!(
        !receipt_src.contains("StandingEntry") && !receipt_src.contains("StandingLedger"),
        "AC-5: `ReceiptPayload`'s module names the claim-standing type -- this is the frozen-face \
         contact `req/730` SS2 verified does not exist"
    );

    let dsse_src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dsse.rs"),
    )
    .expect("dsse.rs is readable");
    let decl = dsse_src
        .find("pub struct DsseEnvelope {")
        .expect("DsseEnvelope is declared in dsse.rs");
    // Start counting AFTER the declaration line's own opening brace, so "pub struct" itself is
    // not mistaken for a field.
    let start = decl + "pub struct DsseEnvelope {".len();
    let body_end = dsse_src[start..].find("\n}").expect("the struct body ends");
    let body = &dsse_src[start..start + body_end];
    let field_count = body.matches("pub ").count();
    println!("DR4640_AC5 dsse_envelope_pub_field_count={field_count}");
    assert_eq!(
        field_count, 3,
        "AC-5: `DsseEnvelope` no longer has exactly 3 fields -- either a field was added (and it \
         had better not be a claim-standing one) or this probe's parse drifted"
    );
    assert!(
        !body.contains("StandingEntry") && !body.contains("standing"),
        "AC-5: `DsseEnvelope`'s own field list names the claim-standing type"
    );
}
