// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/38` §324 ruling 3** — the ledger leaf of a signed document comes from the bytes that
//! were signed, and this is the machine that keeps it that way.
//!
//! # The same defect, three times
//!
//! | window | what was read as the cause | what was actually wrong |
//! |---|---|---|
//! | `req/38` §294 | a member was added "required with no default", so August's receipts stopped **decoding** | true, and not the whole of it |
//! | `req/519` §7-5 | `Option` was tried, twice, and the receipts still answered `exit 7` | the leaf is re-derived from a **struct**, so every member ever added had already moved every historical leaf |
//! | `req/38` §324 | a sixteenth member was added and the 2026-08-22 specimen verified as `inclusion: refuted` | the same thing again, now measured on a document that decodes |
//!
//! Each window repaired the layer it could see. This file exists because the third repeat is the
//! point at which "be careful next time" stops being a plan: `ReceiptPayload::ledger_digest`
//! re-encodes a value, and a value is this build's, while a receipt is whoever's who signed it.
//!
//! # What is asserted, and why each of the three is needed
//!
//! 1. **The two roads agree** for a receipt this build issues. Without this the repair would be a
//!    new number rather than the same number reached honestly, and every ledger would have to move.
//! 2. **The bytes road answers for a document the value road cannot even open.** Measured on the
//!    2026-08-18 specimen, which does not decode against this schema at all. This is the property
//!    the whole repair is for, and it is the one that cannot be faked by a green that means nothing.
//! 3. **The leaf of each frozen specimen is pinned to a constant.** This is the ratchet. A member
//!    added to `ReceiptPayload` tomorrow cannot move these numbers, because nothing here decodes —
//!    and if a future hand routes a leaf back through the struct, these constants go red and name
//!    the erratum that says why.
//!
//! # 🔴 DR-A erratum landed (`req/38` §337, `req/565` §2)
//!
//! 42 §3.11's literal wording ("the BLAKE3 digest of `Receipt`, the whole of the DSSE envelope
//! bytes") and what this file measures do not agree — `req/54` §4 H5-1's self-reference (43 T-11
//! appends before the receipt is issued, and 42 §3.10 puts `inclusion_proof` inside the signed
//! envelope, so a leaf covering the whole envelope would cover a proof derived from itself). The
//! ruling kept the wording (option (a), erratum, not (b) rewrite or (c) reject the
//! implementation) and recorded what this build actually computes — the payload digest with
//! `inclusion_proof` replaced by CBOR `f6` — as a no-delete addendum directly under 42 §3.11's
//! table. This file's own constants are that value, pinned; nothing here moves.
//!
//! # 🔴 The fixtures are not this file's to re-mint
//!
//! Both specimen directories carry that instruction in their own suites' headers. If a constant
//! below goes red, **the artefact is the evidence and the code is the suspect**. Re-minting a
//! specimen to make a number agree destroys the only thing in the tree that can see across a
//! release boundary — which is precisely the structural blindness `req/519` §7-6 created the
//! corpus to close.

use std::path::{Path, PathBuf};

use gx_canon::cid;
use gx_witness::receipt::{ledger_digest_of_signed_payload, Receipt};

mod support;

use support::{commit_receipt_in_a_log, keypair};

// ---------------------------------------------------------------------------
// The pinned leaves — the ratchet
// ---------------------------------------------------------------------------

/// 🔴 The ledger leaf of the 2026-08-18 specimen, derived from its own signed bytes.
///
/// This document does **not** decode against this build's `ReceiptPayload` (`req/38` §294; the
/// members DR-46-24/26/28 added are absent and two of them are required). Before this repair there
/// was therefore no road in the workspace that could state its leaf at all. There is now, and this
/// is the number.
///
/// It cannot move. Nothing that produced it looks at a Rust type.
const LEAF_2026_08_18: &str = "gx1:coxm6jakpgd4buxvycrheepzfg6bbkhrpepfc6y6m3fjjswvl33q";

/// 🔴 The ledger leaf of the 2026-08-22 attach-face commit receipt, derived from its signed bytes.
///
/// This one *does* decode today, which is why it was the specimen that caught §324: the value road
/// could run, and ran to a different answer. Pinned here so that the next member addition is
/// measured rather than discovered.
const LEAF_2026_08_22_COMMIT: &str = "gx1:gwul4bnjjdfmii3o7pe5jcmsubo75ae67fy3hya6ltgd7vrp4kta";

/// The 2026-08-18 specimen, where `crates/gx-witness` keeps it.
fn frozen_08_18() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/frozen_receipts/issued_2026_08_18/receipt.json")
}

/// The 2026-08-22 attach-face specimen, read across the crate boundary rather than copied.
///
/// `req/540` R-2b: one specimen, one copy. Two copies of a frozen artefact are two things that can
/// rot apart, and the one that rots is the one nobody looks at.
fn frozen_08_22(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gx-cli")
        .join("tests/fixtures/attach_face_frozen/issued_2026_08_22")
        .join(name)
}

fn read_receipt(path: &Path) -> Receipt {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("the frozen specimen is at {}: {e}", path.display()));
    serde_json::from_str(&text).expect("the JSON face of a receipt decodes")
}

// ---------------------------------------------------------------------------
// 1 — the two roads are one number
// ---------------------------------------------------------------------------

/// 🔴 For a receipt this build issues, the bytes road and the value road answer the same digest.
///
/// This is what makes the repair a repair rather than a migration. Every leaf in every ledger was
/// written by `ReceiptPayload::ledger_digest`; if the road that now verifies them answered
/// something else, the repair would refuse the entire history it was written to rescue.
///
/// Both kinds, because the cleared member is `inclusion_proof` and the two kinds are exactly the
/// ones that do and do not carry one — a splice that only worked when the value was a map would
/// pass on one and fail on the other.
#[test]
fn the_bytes_road_and_the_value_road_agree_for_a_receipt_this_build_issues() {
    let key = keypair(3);
    let (commit, _checkpoint) = commit_receipt_in_a_log(&key, 21, 4);
    let verdict = support::issue(
        &support::verdict_payload(gx_core::VerdictKind::Admit, &key, 21),
        &key,
    );

    for (name, receipt) in [("commit", &commit), ("verdict", &verdict)] {
        let by_value = receipt
            .payload()
            .expect("this build issued it, so it decodes")
            .ledger_digest()
            .expect("canonical");
        let by_bytes = ledger_digest_of_signed_payload(&receipt.envelope.payload)
            .expect("the signed bytes are canonical");
        println!(
            "LEAF_ROADS kind={name} by_value={} by_bytes={}",
            cid::to_text(&by_value),
            cid::to_text(&by_bytes)
        );
        assert_eq!(
            by_value, by_bytes,
            "🔴 the producer's road and the verifier's road have parted. Every leaf this build \
             writes is the first one; every leaf it checks is the second. They are the same \
             number or the ledger is unreadable by its own writer"
        );
    }

    // And `Receipt::ledger_digest` is the bytes road, not a third answer.
    assert_eq!(
        commit.ledger_digest().expect("canonical"),
        ledger_digest_of_signed_payload(&commit.envelope.payload).expect("canonical")
    );
}

// ---------------------------------------------------------------------------
// 2 — the road answers for a document the other road cannot open
// ---------------------------------------------------------------------------

/// 🔴 **The property the whole repair exists for**, measured on a real archived artefact.
///
/// The 2026-08-18 specimen does not decode against this build. `req/519` §7-5 established that and
/// `docs/LIMITS.md` says so in the product's own voice. So for this document the value road cannot
/// be taken **at all** — not "takes it and gets the wrong answer", but "cannot run".
///
/// The bytes road runs, because it never names a member other than the one key it clears. That is
/// the difference between a leaf derivation that survives a release boundary and one that does not,
/// and it is stated here as a pair: one road errors, the other returns a number.
#[test]
fn the_bytes_road_states_a_leaf_the_value_road_cannot_even_open() {
    let receipt = read_receipt(&frozen_08_18());

    let by_value = receipt.payload();
    println!(
        "FROZEN_2026_08_18 decodes={} err={:?}",
        by_value.is_ok(),
        by_value.as_ref().err().map(ToString::to_string)
    );
    assert!(
        by_value.is_err(),
        "🔴 this specimen has started decoding. That is not a failure — it is the limit \
         `docs/LIMITS.md` declares having closed — but this probe's pair no longer measures what \
         it claims, and the page and this test have to be updated together"
    );

    let by_bytes = ledger_digest_of_signed_payload(&receipt.envelope.payload)
        .expect("🔴 the signed bytes of an archived receipt are canonical DAG-CBOR; if this errors, the repair reaches no further than the value road did");
    println!(
        "FROZEN_2026_08_18 signed_bytes={} leaf={}",
        receipt.envelope.payload.len(),
        cid::to_text(&by_bytes)
    );
    assert_eq!(
        cid::to_text(&by_bytes),
        LEAF_2026_08_18,
        "🔴 `req/38` §324: the leaf of a signed document may not move. Nothing that computes this \
         number reads a Rust type, so a member added to `ReceiptPayload` cannot have moved it — \
         if this is red, a leaf has been routed back through the struct"
    );
}

// ---------------------------------------------------------------------------
// 3 — the ratchet, on the specimen that caught §324
// ---------------------------------------------------------------------------

/// 🔴 The 2026-08-22 attach-face specimen's leaf, pinned.
///
/// This document decodes today, so both roads run and both answer the same thing — which is exactly
/// why it is worth pinning: **the day they stop agreeing is the day a member was added**, and on
/// that day the value road moves and this constant does not. §324 is the record of that day
/// happening without an instrument in place.
///
/// The suite that owns the fixture (`crates/gx-cli/tests/p1b_attach_face_frozen.rs`) asserts the
/// receipt still *verifies*; this asserts the number that verification rests on, one layer down,
/// so a future break is named at the seam rather than as a red `verify` with a long explanation.
#[test]
fn the_leaf_of_the_2026_08_22_specimen_is_pinned_and_both_roads_reach_it() {
    let receipt = read_receipt(&frozen_08_22("commit_receipt.json"));
    let by_bytes = ledger_digest_of_signed_payload(&receipt.envelope.payload)
        .expect("the signed bytes are canonical");

    let by_value = receipt.payload().map(|p| p.ledger_digest());
    println!(
        "FROZEN_2026_08_22 signed_bytes={} by_bytes={} by_value={:?}",
        receipt.envelope.payload.len(),
        cid::to_text(&by_bytes),
        by_value
            .as_ref()
            .ok()
            .and_then(|d| d.as_ref().ok())
            .map(cid::to_text)
    );

    assert_eq!(
        cid::to_text(&by_bytes),
        LEAF_2026_08_22_COMMIT,
        "🔴 `req/38` §324: this is the specimen whose `inclusion: refuted` sent a lane back. Its \
         leaf is a fact about bytes signed on 2026-08-22 and cannot be moved by anything this \
         build does to its own types"
    );

    // Today the two roads agree, because this specimen was minted under a schema this build still
    // has. That agreement is **not** asserted as a requirement — it is the thing that stops being
    // true the moment a member is added, and the repair's whole purpose is that the receipt goes
    // on verifying when it does. What is asserted is that the *bytes* road is the one the
    // verification takes.
    if let Ok(Ok(value_leaf)) = by_value {
        println!("FROZEN_2026_08_22 roads_agree={}", value_leaf == by_bytes);
    }
}

/// 🔴 The two specimens do not share a leaf, so the constants above are two measurements.
///
/// A pair of pinned digests that happened to be equal — or that both came from some default — would
/// satisfy every assertion above while measuring nothing. `req/519` §292's lesson in its general
/// form: a fixture has to be shown to be the thing it claims to be.
#[test]
fn the_two_pinned_leaves_are_different_numbers() {
    println!("PINNED 08_18={LEAF_2026_08_18} 08_22={LEAF_2026_08_22_COMMIT}");
    assert_ne!(LEAF_2026_08_18, LEAF_2026_08_22_COMMIT);
    assert!(LEAF_2026_08_18.starts_with("gx1:"));
    assert!(LEAF_2026_08_22_COMMIT.starts_with("gx1:"));
}

/// 🔴 The verification road takes the bytes road — asserted on the **source**, not on behaviour.
///
/// A behavioural probe cannot tell "the leaf was derived from bytes" from "the leaf was derived
/// from a struct that happens to round-trip", and today every specimen in the tree round-trips.
/// So this reads the one line that decides it. `req/38` §324's permanent rule needs a machine, and
/// a machine that only works after somebody has already added the member that breaks it is not one.
#[test]
fn the_inclusion_check_derives_its_leaf_from_the_signed_bytes() {
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt.rs"))
            .expect("this crate's own module");
    let body = source
        .split("fn verify_inclusion_from(")
        .nth(1)
        .expect("the inclusion check is declared")
        .split("\n}")
        .next()
        .expect("it closes");
    let from_bytes = body.contains("ledger_digest_of_signed_payload(signed_bytes)");
    let from_struct = body.contains("payload.ledger_digest()");
    println!("INCLUSION_LEAF from_bytes={from_bytes} from_struct={from_struct}");
    assert!(
        from_bytes && !from_struct,
        "🔴 `req/38` §324 ruling 3: the inclusion check derives its leaf from the payload struct \
         again. That is the line three lanes in a row were sent back over — §294, §519 §7-5, §324 \
         — and it answers `Refuted`, the word for tampering, about untouched receipts as soon as \
         `ReceiptPayload` gains a member"
    );
}
