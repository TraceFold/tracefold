//! AC-019 (FR-019) — one flipped bit, and the receipt stops verifying. Always.
//!
//! AC-019 逐語: 「Given: Ed25519鍵で署名済みReceipt r。When: rのバイト列表現からランダムな1bitを
//! 反転したr'を検証。Then: `Err(SignatureInvalid)`。未改変rは`Ok`。」判定方法 `property（ランダム
//! bit位置×複数回）`, M2.
//!
//! # Three faces, and then every bit
//!
//! The hand's brief asks the three faces the AC's 「バイト列表現」 decomposes into -- the payload,
//! the signature, and the `keyid` -- and each has its own test below. They are the faces because
//! they fail for three different reasons: the first changes the signed message, the second changes
//! the signature over it, and the third changes *which* signature is asked for. A verifier could
//! plausibly get any one of them right and the others wrong.
//!
//! Then `every_single_bit_...` sweeps the whole serialised envelope **exhaustively** rather than at
//! random. The AC asks for random positions and a property test provides them (`proptest`, at
//! `PROPTEST_CASES` from `ci.sh` stage 4e); the exhaustive sweep is what makes the claim total. It
//! reports its own denominator -- flips attempted, flips refused as bad signatures, flips refused
//! by the decoder -- because a receipt's serialisation contains framing bytes as well as signed
//! ones, and a flip in the CBOR framing damages the container before any signature is consulted.
//! **`Ok` is what must never happen**, and the count of it is asserted at zero.
//!
//! # Why every flip in the signed material is a signature failure and not a parse failure
//!
//! `DsseEnvelope.payload` is bytes and stays bytes until after the signature is checked
//! (`receipt.rs`'s note on the order of checks). So a bit flipped inside the payload -- which is a
//! canonical DAG-CBOR document, and would very often stop being one -- is caught as a bad
//! signature, which is exactly what AC-019 asks for and would not be true of a verifier that
//! decoded first.

mod support;

use gx_canon::cbor;
use gx_core::{DsseSignature, VerdictKind};
use gx_witness::dsse::DsseEnvelope;
use gx_witness::receipt::{verify_offline, Receipt};
use gx_witness::Error;
use proptest::prelude::*;
use support::{issue, issued_at, keypair, verdict_payload};

/// Flip bit `position` of `bytes`, counting from bit 0 of byte 0.
fn flip(bytes: &[u8], position: usize) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out[position / 8] ^= 1 << (position % 8);
    out
}

fn a_receipt() -> (gx_witness::KeyPair, Receipt) {
    let key = keypair(1);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 0), &key);
    (key, receipt)
}

// ---------------------------------------------------------------------------
// 「未改変rは`Ok`」
// ---------------------------------------------------------------------------

/// The other half of AC-019, and the half that makes the rest mean anything: an untouched receipt
/// verifies. A verifier that refused everything would pass every test below.
#[test]
fn ac_019_an_untouched_receipt_verifies() {
    let (key, receipt) = a_receipt();
    let checks = verify_offline(&receipt, &key.verifying(), None).expect("untouched: Ok");
    assert!(checks.verified());
}

// ---------------------------------------------------------------------------
// Face 1: the payload
// ---------------------------------------------------------------------------

/// Every bit of the payload, one at a time. The payload is the signed material, so this is total
/// and not a sample: `SignatureInvalid` for all of them, with no exceptions and no `Ok`.
#[test]
fn ac_019_every_bit_of_the_payload_is_caught() {
    let (key, receipt) = a_receipt();
    let bits = receipt.envelope.payload.len() * 8;
    assert!(bits > 0);

    for position in 0..bits {
        let mut tampered = receipt.clone();
        tampered.envelope.payload = flip(&receipt.envelope.payload, position);
        match verify_offline(&tampered, &key.verifying(), None) {
            Err(Error::SignatureInvalid { .. }) => {}
            other => panic!("payload bit {position} gave {other:?}"),
        }
    }
    println!("AC019_PAYLOAD_BITS={bits}");
}

/// The payload type is signed too, and 42 §3.10 fixes its value -- so a receipt whose type was
/// edited is caught by the signature and not only by a constant comparison somebody might forget.
#[test]
fn ac_019_every_bit_of_the_payload_type_is_caught() {
    let (key, receipt) = a_receipt();
    let bits = receipt.envelope.payload_type.len() * 8;

    let mut checked = 0usize;
    for position in 0..bits {
        let raw = flip(receipt.envelope.payload_type.as_bytes(), position);
        // A flipped bit can leave the string invalid UTF-8, which is not a receipt anybody can
        // build; those positions are skipped and counted rather than silently dropped.
        let Ok(text) = String::from_utf8(raw) else {
            continue;
        };
        let mut tampered = receipt.clone();
        tampered.envelope.payload_type = text;
        match verify_offline(&tampered, &key.verifying(), None) {
            Err(Error::SignatureInvalid { .. }) => checked += 1,
            other => panic!("payload_type bit {position} gave {other:?}"),
        }
    }
    println!(
        "AC019_PAYLOAD_TYPE_BITS={bits} CHECKED={checked} SKIPPED_NON_UTF8={}",
        bits - checked
    );
    assert!(
        checked > bits / 2,
        "too many positions were skipped to mean anything"
    );
}

// ---------------------------------------------------------------------------
// Face 2: the signature
// ---------------------------------------------------------------------------

/// Every bit of the 64-byte signature. Ed25519 signatures are malleable in some encodings and
/// `verify_strict` is what refuses those; this is the sweep that would notice if the plain `verify`
/// were used instead.
#[test]
fn ac_019_every_bit_of_the_signature_is_caught() {
    let (key, receipt) = a_receipt();
    let original = receipt.envelope.signatures[0].sig.clone();
    assert_eq!(original.len(), 64, "Ed25519 signatures are 64 bytes");

    for position in 0..original.len() * 8 {
        let mut tampered = receipt.clone();
        tampered.envelope.signatures[0].sig = flip(&original, position);
        match verify_offline(&tampered, &key.verifying(), None) {
            Err(Error::SignatureInvalid { .. }) => {}
            other => panic!("signature bit {position} gave {other:?}"),
        }
    }
    println!("AC019_SIGNATURE_BITS={}", original.len() * 8);
}

/// A signature of the wrong length, which is not a bit flip but is the malformed envelope the same
/// AC asks a verifier to reject. `gx_core::DsseSignature` deliberately does not constrain the
/// length; this is where the refusal lands instead.
#[test]
fn ac_019_a_signature_of_the_wrong_length_is_refused() {
    let (key, receipt) = a_receipt();
    for length in [0usize, 1, 63, 65, 128] {
        let mut tampered = receipt.clone();
        tampered.envelope.signatures[0].sig = vec![0u8; length];
        assert!(
            matches!(
                verify_offline(&tampered, &key.verifying(), None),
                Err(Error::SignatureInvalid { .. })
            ),
            "a {length}-byte signature was not refused"
        );
    }
}

/// An envelope with no signature at all.
#[test]
fn ac_019_an_envelope_with_no_signature_is_refused() {
    let (key, receipt) = a_receipt();
    let mut tampered = receipt.clone();
    tampered.envelope.signatures.clear();
    assert!(matches!(
        verify_offline(&tampered, &key.verifying(), None),
        Err(Error::SignatureInvalid { .. })
    ));
}

// ---------------------------------------------------------------------------
// Face 3: the keyid
// ---------------------------------------------------------------------------

/// Every bit of the `keyid`. The failure here has a different mechanism -- the signature under the
/// caller's key id is no longer present -- and the AC's vocabulary is the same, which is the point:
/// a verifier that reported 「no such key」 as a distinct outcome would let a forger separate 「wrong
/// key」 from 「wrong bytes」.
#[test]
fn ac_019_every_bit_of_the_keyid_is_caught() {
    let (key, receipt) = a_receipt();
    let original = receipt.envelope.signatures[0].keyid.clone();
    assert!(!original.is_empty());

    let mut checked = 0usize;
    let bits = original.len() * 8;
    for position in 0..bits {
        let Ok(text) = String::from_utf8(flip(original.as_bytes(), position)) else {
            continue;
        };
        let mut tampered = receipt.clone();
        tampered.envelope.signatures[0].keyid = text;
        match verify_offline(&tampered, &key.verifying(), None) {
            Err(Error::SignatureInvalid { .. }) => checked += 1,
            other => panic!("keyid bit {position} gave {other:?}"),
        }
    }
    println!("AC019_KEYID_BITS={bits} CHECKED={checked}");
    assert!(checked > bits / 2);
}

/// A signature added under a second key id does not make the first one verify. The `Vec` of 42
/// §3.10 admits several signatures, and a verifier that tried them all against one key would accept
/// this.
#[test]
fn ac_019_a_signature_under_another_id_does_not_stand_in() {
    let (key, receipt) = a_receipt();
    let mut tampered = receipt.clone();
    let valid = tampered.envelope.signatures[0].clone();
    tampered.envelope.signatures = vec![DsseSignature {
        keyid: "somebody-else".to_string(),
        sig: valid.sig,
    }];
    assert!(matches!(
        verify_offline(&tampered, &key.verifying(), None),
        Err(Error::SignatureInvalid { .. })
    ));
}

// ---------------------------------------------------------------------------
// 「rのバイト列表現」, exhaustively
// ---------------------------------------------------------------------------

/// Every bit of the serialised envelope, with the outcomes counted.
///
/// This is the AC's own subject -- 「rのバイト列表現からランダムな1bitを反転」 -- taken totally
/// instead of at random. The envelope is encoded canonically, each bit is flipped, and the bytes are
/// decoded and verified. Three outcomes are possible and all three are counted:
///
/// * the decoder refuses the bytes (a flip in the CBOR framing, in a length header, or one that
///   makes a text field invalid UTF-8) -- the container was damaged before any signature was read;
/// * the verification refuses (`SignatureInvalid`) -- a flip in the signed material or the
///   signature;
/// * `Ok` -- **which must not happen**, and is asserted at zero.
///
/// Reporting the split is the point. A test that only asserted 「not Ok」 would be satisfied by an
/// implementation where every flip broke the decoder and none reached the signature, and the
/// numbers are what show that both roads are live.
#[test]
fn ac_019_every_single_bit_of_the_serialised_receipt_is_caught() {
    let (key, receipt) = a_receipt();
    let bytes = cbor::encode(&receipt.envelope).expect("an envelope has a canonical form");

    let (mut invalid, mut undecodable, mut accepted) = (0usize, 0usize, 0usize);
    for position in 0..bytes.len() * 8 {
        let tampered = flip(&bytes, position);
        match cbor::decode::<DsseEnvelope>(&tampered) {
            Err(_) => undecodable += 1,
            Ok(envelope) => {
                let r = Receipt {
                    envelope,
                    issued_at: issued_at(),
                };
                match verify_offline(&r, &key.verifying(), None) {
                    Err(Error::SignatureInvalid { .. }) => invalid += 1,
                    Err(_) => undecodable += 1,
                    Ok(_) => accepted += 1,
                }
            }
        }
    }

    let total = bytes.len() * 8;
    println!(
        "AC019_ENVELOPE_BYTES={} BITS={total} SIGNATURE_INVALID={invalid} \
         REFUSED_BEFORE_VERIFY={undecodable} ACCEPTED={accepted}",
        bytes.len()
    );
    assert_eq!(accepted, 0, "a tampered receipt verified");
    assert_eq!(invalid + undecodable, total);
    assert!(
        invalid > total / 2,
        "only {invalid} of {total} flips reached the signature check; the rest died in the decoder"
    );
}

// ---------------------------------------------------------------------------
// 「property（ランダムbit位置×複数回）」
// ---------------------------------------------------------------------------

proptest! {
    /// AC-019's stated 判定方法, over random positions and random receipts.
    ///
    /// The exhaustive sweep above is stronger for one receipt; this is broader across receipts --
    /// different key seeds, different verdicts, different transformations, so the property is not a
    /// fact about one payload's byte pattern.
    #[test]
    fn ac_019_a_random_bit_of_a_random_receipt_is_caught(
        key_seed in 0u8..=255,
        transformation_seed in 0u64..1000,
        kind_index in 0usize..3,
        offset in 0usize..100_000,
    ) {
        let key = support::keypair(key_seed);
        let kind = gx_core::VerdictKind::ALL[kind_index];
        let receipt = issue(&verdict_payload(kind, &key, transformation_seed), &key);
        let bytes = cbor::encode(&receipt.envelope).expect("canonical");

        let position = offset % (bytes.len() * 8);
        let tampered = flip(&bytes, position);
        let outcome = cbor::decode::<DsseEnvelope>(&tampered).map(|envelope| {
            verify_offline(
                &Receipt { envelope, issued_at: issued_at() },
                &key.verifying(),
                None,
            )
        });
        prop_assert!(
            !matches!(outcome, Ok(Ok(_))),
            "bit {position} of a {kind} receipt was accepted"
        );
    }
}
