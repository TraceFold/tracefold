//! **AC-019, re-confirmed through the CLI** (51 §15 M6 行 / req/88 §7).
//!
//! AC-019 逐語: 「Given: Ed25519鍵で署名済みReceipt r。When: rのバイト列表現からランダムな1bitを反転
//! したr'を検証。Then: `Err(SignatureInvalid)`。未改変rは`Ok`。」判定方法 `property（ランダムbit位置×
//! 複数回）`, M2.
//!
//! # What changes when the criterion moves to a command line
//!
//! `Err(SignatureInvalid)` is a Rust value and a process has an exit status, so the CLI half of
//! AC-019 is 「the refusal is 44 §1.2's `7=無効`, and the object on stdout says the **signature** is
//! what refused」. That second half matters: a flipped bit inside the signed payload could plausibly
//! be reported as a malformed CID or a bad schema, and gx-witness verifies the signature over the
//! raw envelope bytes **before** decoding anything precisely so that it is not.
//!
//! The property here is over bit **positions**, not over random keys: every byte of the signed
//! payload is flipped in turn, which is a superset of 「ランダムな1bit位置×複数回」 for a fixture of
//! this size and is deterministic, so a failure names the byte.

mod support;

use gx_core::VerdictKind;
use gx_witness::receipt::Receipt;
use support::{issue, keypair, run, scratch, verdict_payload, write_json, write_public_key};

/// Flip one bit in every byte of the signed payload; every one of them has to be caught.
#[test]
fn ac_019_cli_every_flipped_bit_in_the_signed_payload_is_refused() {
    let dir = scratch("ac019_cli");
    let key = keypair(1);
    let key_path = write_public_key(&dir, &key);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 300), &key);

    // 「未改変rは`Ok`」 first, so that a fixture which never verified would not read as a hundred
    // successful detections.
    let clean = write_json(
        &dir.join("clean.json"),
        &serde_json::to_value(&receipt).expect("serialises"),
    );
    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&clean)
        .arg("--offline")
        .arg("--key")
        .arg(&key_path));
    println!("AC019_CLI_CLEAN exit={} {}", out.code, out.json());
    assert_eq!(out.code, 0, "AC-019's 「未改変rは`Ok`」");

    let bytes = receipt.envelope.payload.len();
    let mut caught = 0usize;
    let mut wrong_reason = Vec::new();
    for index in 0..bytes {
        let mut tampered: Receipt = receipt.clone();
        tampered.envelope.payload[index] ^= 0b0000_0001;
        let path = write_json(
            &dir.join("tampered.json"),
            &serde_json::to_value(&tampered).expect("serialises"),
        );
        let out = run(support::gx()
            .arg("receipt")
            .arg("verify")
            .arg(&path)
            .arg("--offline")
            .arg("--key")
            .arg(&key_path));
        let json = out.json();
        if out.code == 7 && json["valid"] == serde_json::json!(false) {
            caught += 1;
        }
        if json["checks"]["signature"] != serde_json::json!(false) {
            wrong_reason.push(index);
        }
    }
    println!(
        "AC019_CLI_BYTES={bytes} CAUGHT={caught} REPORTED_AS_SOMETHING_ELSE={}",
        wrong_reason.len()
    );
    assert_eq!(
        caught, bytes,
        "every flipped bit in the signed material has to be refused with 44 §1.2's 7"
    );
    assert!(
        wrong_reason.is_empty(),
        "these bytes were refused for a reason other than the signature: {wrong_reason:?}. \
         gx-witness checks the signature over the raw envelope bytes before decoding so that a \
         flipped bit is never reported as a malformed value"
    );
}

/// A flip in the **signature** is caught too, and is not a different question.
///
/// `DsseEnvelope::verify` asks for the named key's signature and checks that one, so a corrupted
/// `sig` and a corrupted `keyid` both arrive at `SignatureInvalid` — 「a verifier that named the part
/// which failed would be telling a forger which part to fix」.
#[test]
fn ac_019_cli_a_flipped_signature_is_refused_the_same_way() {
    let dir = scratch("ac019_cli_sig");
    let key = keypair(2);
    let key_path = write_public_key(&dir, &key);
    let mut receipt = issue(&verdict_payload(VerdictKind::Deny, &key, 301), &key);
    receipt.envelope.signatures[0].sig[0] ^= 0b1000_0000;
    let path = write_json(
        &dir.join("receipt.json"),
        &serde_json::to_value(&receipt).expect("serialises"),
    );

    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&path)
        .arg("--offline")
        .arg("--key")
        .arg(&key_path));
    let json = out.json();
    println!("AC019_CLI_SIGFLIP exit={} {json}", out.code);
    assert_eq!(out.code, 7);
    assert_eq!(json["checks"]["signature"], serde_json::json!(false));
}
