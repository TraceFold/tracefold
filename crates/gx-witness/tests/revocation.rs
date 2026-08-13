//! **FR-M7-3** — key revocation, and what an offline verifier can say about a receipt signed by a
//! key that has been revoked.
//!
//! req/98 §3-2's AC: 「失効済み鍵で署名された receipt が、失効時刻より後の verify で **無効**と判定
//! される(**遡及範囲は政策設定であり、設定後の一貫性のみを機械が検査する**)」. The two halves are
//! measured separately, because they are two different kinds of statement:
//!
//! | half | measured by |
//! |---|---|
//! | the invariant | `revocation_status` over the four inputs, every combination of them named |
//! | 「設定後の一貫性」 | the same receipt under both settings, and the same setting over a table of receipts |
//!
//! # 🔴 What this suite also measures, on purpose: the limit
//!
//! `Receipt.issued_at` is **unsigned** (**E-M2-6**, CM-5: 「signed payload から clock read 排除」), so
//! `Retroaction::FromRevocation` — which is ASM-45-2's DEFAULT, 「失効前に発行済みのreceiptは遡及無効化
//! しない（『発行時点の鍵状態』で有効性判定）」 — rests on a timestamp nobody signed. Whoever holds the
//! compromised secret can re-issue the same payload with an earlier `issued_at` and stay valid under
//! that setting. 45 §3's own residual register says as much (TH-5: 「TSA連携なしのv0.1では失効時刻の
//! 第三者証明が弱い」, resolved in v0.2 with ASM-4's TSA), and
//! `the_default_setting_cannot_see_a_backdated_receipt` is that sentence as a running test rather
//! than as a paragraph. A limit with a test is a limit somebody can find; a limit in prose is one
//! that gets quoted after it has already been relied on.

mod support;

use gx_canon::cbor;
use gx_core::Timestamp;
use gx_witness::dsse::{DsseEnvelope, REVOCATION_PAYLOAD_TYPE};
use gx_witness::keys::{Retroaction, RevocationEntry, RevocationLedger};
use gx_witness::receipt::{
    verify_offline, verify_offline_consulting, RevocationCheck, RevocationPolicy,
};
use gx_witness::{Error, KeyPair};
use support::{issued_at, keypair, verdict_payload};

use gx_core::VerdictKind;

/// A moment, in the same units `Timestamp` carries (nanoseconds since the epoch).
const SECOND: i64 = 1_000_000_000;

/// 🔴 A revocation envelope built **without this crate's cooperation** — an attacker's, in other
/// words.
///
/// `RevocationEntry::signed_by` refuses to sign a revocation of somebody else's key, which is the
/// producer-side half of the rule. A test that forged through it would be measuring that refusal
/// twice and the verifier's side not at all, so the forgery is assembled from the parts an attacker
/// actually has: the entry's canonical bytes, the payload type, and their own signature.
fn forged(entry: &RevocationEntry, signer: &KeyPair) -> DsseEnvelope {
    let mut envelope = DsseEnvelope {
        payload_type: REVOCATION_PAYLOAD_TYPE.to_string(),
        payload: cbor::encode(entry).expect("an entry has a canonical form"),
        signatures: Vec::new(),
    };
    envelope.sign(signer.signing_key(), signer.key_id());
    envelope
}

// ---------------------------------------------------------------------------
// The entry, and who may sign one
// ---------------------------------------------------------------------------

/// A revocation is a statement **signed by the key it revokes**, and nothing else is one.
///
/// v0.1 has no trust root above an actor's key: 45 §1 keeps the engine's signing key distinct from
/// the adjudicator's and names no authority over either, so the only signature a verifier can check
/// a revocation against is the revoked key's own. That makes a revocation self-authenticating (the
/// shape OpenPGP's revocation certificate has) and it makes forging one require the very secret the
/// revocation is about — an attacker who has it gains the ability to deny themselves.
///
/// The cost is stated rather than hidden: a key whose secret was **lost** rather than leaked cannot
/// be revoked at all. `req/100` §5 routes an operator-signed revocation to the window that has a
/// trust root to hang it on.
#[test]
fn a_revocation_is_signed_by_the_key_it_revokes() {
    let key = keypair(1);
    let other = keypair(2);
    let entry = RevocationEntry::new(key.key_id().clone(), Timestamp(10 * SECOND), "compromised");

    let envelope = entry
        .signed_by(&key)
        .expect("the entry has a canonical form");
    let read = RevocationEntry::from_signed(&envelope, &key.verifying())
        .expect("a self-signed revocation verifies under the key it names");
    assert_eq!(read, entry, "the entry survives the round trip unchanged");

    // Signed by somebody else's key: refused, and refused as a signature failure rather than as a
    // schema one. A verifier that accepted it would let any key holder revoke any other key.
    let refusal = RevocationEntry::from_signed(&forged(&entry, &other), &key.verifying())
        .expect_err("a revocation signed by another key is not a revocation");
    println!("REVOCATION_FORGED_REFUSAL {refusal}");
    assert!(matches!(refusal, Error::SignatureInvalid { .. }));
}

/// The producer refuses too, at the moment of signing.
///
/// Two defences for one rule, and the reason is timing rather than doubt: an operator whose command
/// produced a record no verifier will accept should learn it from their own command, not from a
/// stranger months later (M6H3-5's shape: 「a flag with nowhere to go is refused, never dropped」).
/// The **load-bearing** half is the verifier's, since an attacker does not call this function —
/// which is why the test above forges without it.
#[test]
fn signing_somebody_elses_revocation_is_refused_at_the_producer() {
    let key = keypair(1);
    let entry = RevocationEntry::new("key-not-mine".to_string(), Timestamp(10 * SECOND), "no");
    let refusal = entry
        .signed_by(&key)
        .expect_err("this key does not speak for that one");
    println!("REVOCATION_PRODUCER_REFUSAL {refusal}");
    assert!(matches!(refusal, Error::Schema { .. }));
}

/// An entry whose `key_id` is not the key that signed it is refused (the same defect, inside out).
#[test]
fn an_entry_naming_another_key_is_refused() {
    let key = keypair(1);
    let entry = RevocationEntry::new(
        "key-somebody-else".to_string(),
        Timestamp(10 * SECOND),
        "not mine to revoke",
    );
    let refusal = RevocationEntry::from_signed(&forged(&entry, &key), &key.verifying())
        .expect_err("the entry names a key the signature does not");
    println!("REVOCATION_MISMATCH_REFUSAL {refusal}");
    assert!(matches!(refusal, Error::Schema { .. }));
}

// ---------------------------------------------------------------------------
// The ledger a verifier consults
// ---------------------------------------------------------------------------

/// The ledger keeps the **earliest** revocation of a key, and entries about other keys are ignored
/// rather than refused.
///
/// Earliest, because revocation is monotone: a second statement cannot un-revoke a key, and taking
/// the latest would let a key holder who kept signing after a compromise push the boundary forward.
/// Ignored, because a verifier holding one public key cannot authenticate a statement about another
/// key, and refusing the whole file would make one operator's ledger unusable by everyone else.
#[test]
fn the_ledger_takes_the_earliest_revocation_and_ignores_other_keys() {
    let key = keypair(1);
    let other = keypair(2);

    let late = RevocationEntry::new(key.key_id().clone(), Timestamp(90 * SECOND), "late")
        .signed_by(&key)
        .expect("encodable");
    let early = RevocationEntry::new(key.key_id().clone(), Timestamp(10 * SECOND), "early")
        .signed_by(&key)
        .expect("encodable");
    let theirs = RevocationEntry::new(other.key_id().clone(), Timestamp(SECOND), "theirs")
        .signed_by(&other)
        .expect("encodable");

    let (ledger, ignored) = RevocationLedger::from_signed(&[late, early, theirs], &key.verifying())
        .expect("every entry about this key authenticates");
    println!(
        "REVOCATION_LEDGER entries={} ignored={ignored}",
        ledger.len()
    );
    assert_eq!(
        ignored, 1,
        "the entry about another key is not this verifier's to check"
    );
    assert_eq!(ledger.len(), 2);
    assert_eq!(
        ledger
            .revocation_of(key.key_id())
            .expect("this key is revoked")
            .revoked_at,
        Timestamp(10 * SECOND),
        "the earliest statement wins: revocation is monotone"
    );
    assert!(ledger.revocation_of(other.key_id()).is_none());
}

/// A forged entry **about the key being verified** is a refusal, not an ignore.
#[test]
fn a_forged_entry_about_this_key_stops_the_ledger() {
    let key = keypair(1);
    let attacker = keypair(3);
    let entry = RevocationEntry::new(key.key_id().clone(), Timestamp(10 * SECOND), "denial");

    let refusal = RevocationLedger::from_signed(&[forged(&entry, &attacker)], &key.verifying())
        .expect_err("a statement about this key that this key did not sign");
    println!("REVOCATION_FORGED_LEDGER {refusal}");
    assert!(matches!(refusal, Error::SignatureInvalid { .. }));
}

// ---------------------------------------------------------------------------
// The invariant
// ---------------------------------------------------------------------------

/// 🔴 The AC, at the library API: a receipt signed by a revoked key is **invalid** when it is
/// verified after the revocation.
#[test]
fn fr_m7_3_a_receipt_signed_after_revocation_is_invalid() {
    let key = keypair(1);
    let payload = verdict_payload(VerdictKind::Admit, &key, 1);
    // The receipt claims to have been issued a minute **after** the revocation.
    let receipt = gx_witness::Receipt::issue(&payload, Timestamp(70 * SECOND), &key)
        .expect("a legal receipt");
    let ledger = ledger_revoking(&key, Timestamp(10 * SECOND));

    let checks = verify_offline_consulting(
        &receipt,
        &key.verifying(),
        None,
        &RevocationPolicy {
            ledger: &ledger,
            retroaction: Retroaction::FromRevocation,
            verified_at: Timestamp(100 * SECOND),
        },
    )
    .expect("the signature is still a valid signature");

    println!("FRM73_AFTER {checks:?}");
    assert_eq!(checks.revocation, RevocationCheck::Revoked);
    assert!(
        !checks.verified(),
        "「失効済み鍵で署名された receipt が、失効時刻より後の verify で無効」"
    );
    // 🔴 The signature itself still verifies. A verifier that reported this as a bad signature
    // would be telling an operator to look for tampering that did not happen (E-M3-3's shape, one
    // crate along: 「could not be evaluated」 and 「said something else」 are different facts).
    assert!(checks.canonical_cid);
}

/// The default setting, on a receipt issued **before** the revocation: still valid (ASM-45-2).
#[test]
fn the_default_setting_keeps_receipts_issued_before_the_revocation() {
    let key = keypair(1);
    let receipt = receipt_issued_at(&key, Timestamp(5 * SECOND));
    let ledger = ledger_revoking(&key, Timestamp(10 * SECOND));

    let checks = consulted(
        &receipt,
        &key,
        &ledger,
        Retroaction::FromRevocation,
        Timestamp(100 * SECOND),
    );
    println!("FRM73_BEFORE_DEFAULT {checks:?}");
    assert_eq!(checks.revocation, RevocationCheck::ValidAtIssue);
    assert!(
        checks.verified(),
        "ASM-45-2 の DEFAULT: 「失効前に発行済みのreceiptは遡及無効化しない」"
    );
}

/// The other setting, on the same receipt: invalid. **The machine checks consistency, not the
/// choice** — this pair is what 「遡及範囲は政策設定」 means as a measurement.
#[test]
fn the_retroactive_setting_invalidates_the_same_receipt() {
    let key = keypair(1);
    let receipt = receipt_issued_at(&key, Timestamp(5 * SECOND));
    let ledger = ledger_revoking(&key, Timestamp(10 * SECOND));

    let default = consulted(
        &receipt,
        &key,
        &ledger,
        Retroaction::FromRevocation,
        Timestamp(100 * SECOND),
    );
    let retroactive = consulted(
        &receipt,
        &key,
        &ledger,
        Retroaction::All,
        Timestamp(100 * SECOND),
    );
    println!(
        "FRM73_SETTINGS default={:?} all={:?}",
        default.revocation, retroactive.revocation
    );
    assert_eq!(default.revocation, RevocationCheck::ValidAtIssue);
    assert_eq!(retroactive.revocation, RevocationCheck::Revoked);
    assert!(default.verified() && !retroactive.verified());
}

/// A revocation dated in the verifier's future is **not yet in force**, under either setting.
///
/// This is the other half of the AC's 「失効時刻より後の verify」: before that moment there is nothing
/// to apply. A verifier that applied it early would answer 「invalid」 about a receipt that is valid
/// at the time the question is being asked.
#[test]
fn a_revocation_dated_later_than_the_verification_is_not_yet_in_force() {
    let key = keypair(1);
    let receipt = receipt_issued_at(&key, Timestamp(5 * SECOND));
    let ledger = ledger_revoking(&key, Timestamp(80 * SECOND));

    for setting in [Retroaction::FromRevocation, Retroaction::All] {
        let checks = consulted(&receipt, &key, &ledger, setting, Timestamp(50 * SECOND));
        println!("FRM73_NOT_YET {setting:?} {:?}", checks.revocation);
        assert_eq!(checks.revocation, RevocationCheck::NotYetInForce);
        assert!(checks.verified());
    }
}

/// A key with no entry in a consulted ledger is `NotRevoked`, which is not the same word as
/// `NotConsulted`.
///
/// req/29 §4: 「skip と pass を同じ顔にしない」. `verify_offline` — the road every caller took before
/// this hand — consults nothing and says so, and ASM-45-2 is why that is a pass rather than a
/// failure: 「revocation list参照はverifier側任意とする」. A verifier that consulted a ledger and
/// found nothing has made a stronger statement, and the two are different words on the wire.
#[test]
fn consulting_nothing_and_finding_nothing_are_different_words() {
    let key = keypair(1);
    let receipt = receipt_issued_at(&key, Timestamp(5 * SECOND));

    let unconsulted = verify_offline(&receipt, &key.verifying(), None).expect("verifies");
    assert_eq!(unconsulted.revocation, RevocationCheck::NotConsulted);
    assert!(
        unconsulted.verified(),
        "ASM-45-2 makes consulting the list the verifier's option, so not consulting is not a \
         failure — and the word on the answer is what keeps it from reading as 「checked, clean」"
    );

    let empty = RevocationLedger::empty();
    let consulted_empty = consulted(
        &receipt,
        &key,
        &empty,
        Retroaction::FromRevocation,
        Timestamp(100 * SECOND),
    );
    assert_eq!(consulted_empty.revocation, RevocationCheck::NotRevoked);
    println!(
        "FRM73_WORDS not_consulted={:?} not_revoked={:?}",
        unconsulted.revocation, consulted_empty.revocation
    );
    assert_ne!(unconsulted.revocation, consulted_empty.revocation);
}

/// 🔴 **The limit, as a test**: the default setting cannot see a backdated receipt.
///
/// `issued_at` is outside the signed core (**E-M2-6**), so the holder of a compromised secret can
/// re-issue the same signed payload with an earlier timestamp. Under `FromRevocation` the result is
/// indistinguishable from a receipt that really was issued early; under `All` it is not. That is the
/// whole reason the setting exists, and 45 §3's TH-5 residual (「TSA連携なしのv0.1では失効時刻の第三者
/// 証明が弱い」) is the same fact in the threat model's words.
#[test]
fn the_default_setting_cannot_see_a_backdated_receipt() {
    let key = keypair(1);
    let payload = verdict_payload(VerdictKind::Admit, &key, 1);
    let honest = gx_witness::Receipt::issue(&payload, Timestamp(70 * SECOND), &key).expect("legal");
    // The same signed bytes, with the unsigned timestamp moved back before the revocation.
    let backdated = gx_witness::Receipt {
        envelope: honest.envelope.clone(),
        issued_at: Timestamp(5 * SECOND),
    };
    assert_eq!(
        honest.envelope, backdated.envelope,
        "nothing that is signed has changed; that is the point"
    );

    let ledger = ledger_revoking(&key, Timestamp(10 * SECOND));
    let by_default = consulted(
        &backdated,
        &key,
        &ledger,
        Retroaction::FromRevocation,
        Timestamp(100 * SECOND),
    );
    let retroactive = consulted(
        &backdated,
        &key,
        &ledger,
        Retroaction::All,
        Timestamp(100 * SECOND),
    );
    println!(
        "FRM73_BACKDATED default={:?} all={:?}",
        by_default.revocation, retroactive.revocation
    );
    assert_eq!(
        by_default.revocation,
        RevocationCheck::ValidAtIssue,
        "the default setting believes an unsigned timestamp, and this is that sentence measured"
    );
    assert_eq!(
        retroactive.revocation,
        RevocationCheck::Revoked,
        "the retroactive setting is what a compromise is answered with, because it reads no clock"
    );
}

/// 🔴 Consistency: over a table of receipts, one setting gives one answer per receipt, every time.
///
/// 「設定後の一貫性のみを機械が検査する」 is a statement about a **function**, so it is measured as
/// one: the same inputs answer the same way twice, and the two settings order the receipts the way
/// their definitions say (`All` refuses everything `FromRevocation` refuses, and more).
#[test]
fn the_settings_are_consistent_and_ordered() {
    let key = keypair(1);
    let revoked_at = Timestamp(50 * SECOND);
    let ledger = ledger_revoking(&key, revoked_at);
    let moments = [1, 25, 49, 50, 51, 75, 99].map(|s| Timestamp(s * SECOND));

    let mut refused_by_default = 0;
    for at in moments {
        let receipt = receipt_issued_at(&key, at);
        let first = consulted(
            &receipt,
            &key,
            &ledger,
            Retroaction::FromRevocation,
            Timestamp(100 * SECOND),
        );
        let second = consulted(
            &receipt,
            &key,
            &ledger,
            Retroaction::FromRevocation,
            Timestamp(100 * SECOND),
        );
        assert_eq!(
            first.revocation, second.revocation,
            "one setting, one answer"
        );

        let all = consulted(
            &receipt,
            &key,
            &ledger,
            Retroaction::All,
            Timestamp(100 * SECOND),
        );
        assert_eq!(
            all.revocation,
            RevocationCheck::Revoked,
            "All refuses every receipt of a revoked key"
        );
        if first.revocation == RevocationCheck::Revoked {
            refused_by_default += 1;
            assert!(
                at.0 >= revoked_at.0,
                "the default refuses exactly the receipts dated at or after the revocation"
            );
        } else {
            assert_eq!(first.revocation, RevocationCheck::ValidAtIssue);
            assert!(at.0 < revoked_at.0);
        }
    }
    println!(
        "FRM73_CONSISTENCY moments={} refused_by_default={refused_by_default}",
        moments.len()
    );
    assert_eq!(
        refused_by_default, 4,
        "50, 51, 75, 99 are at or after the revocation"
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn receipt_issued_at(key: &KeyPair, at: Timestamp) -> gx_witness::Receipt {
    let payload = verdict_payload(VerdictKind::Admit, key, 1);
    gx_witness::Receipt::issue(&payload, at, key).expect("a legal receipt")
}

fn ledger_revoking(key: &KeyPair, at: Timestamp) -> RevocationLedger {
    let entry = RevocationEntry::new(key.key_id().clone(), at, "compromised")
        .signed_by(key)
        .expect("encodable");
    let (ledger, ignored) =
        RevocationLedger::from_signed(&[entry], &key.verifying()).expect("authenticates");
    assert_eq!(ignored, 0);
    ledger
}

fn consulted(
    receipt: &gx_witness::Receipt,
    key: &KeyPair,
    ledger: &RevocationLedger,
    retroaction: Retroaction,
    verified_at: Timestamp,
) -> gx_witness::Checks {
    verify_offline_consulting(
        receipt,
        &key.verifying(),
        None,
        &RevocationPolicy {
            ledger,
            retroaction,
            verified_at,
        },
    )
    .expect("the receipt's signature is valid in every case here")
}

/// The fixture clock the rest of the suite does not use, kept so that a reader who follows
/// `support::issued_at` sees why these tests set their own.
#[allow(dead_code)]
fn the_fixture_clock() -> Timestamp {
    issued_at()
}
