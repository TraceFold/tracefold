// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **M9 cross-cutting adversarial audit lane** — an independent re-attack on P2, key-at-rest
//! encryption (`req/130` §1 item2 / `req/131` AC-P2-3). (sem: SEM-gx-witness-160)
//!
//! The implementation lane's own `ac_p2_3_key_encryption.rs` (existing, untouched) measures
//! "refusal of a tampered ciphertext" at only **one point** (one tamper inside the ciphertext
//! region). This lane measures, at the **raw-byte level** across the whole encrypted key file (the
//! DAG-CBOR file `KeyPair::save_encrypted` writes), tampering independently at several different
//! offsets (the header, the region corresponding to salt/nonce/KDF parameters, the ciphertext
//! region, the tail), and measures **that tampering is detected at every offset** (fail-open = 0)
//! — attacking with a model equivalent to a "real attacker" that corrupts the actual byte stream
//! written to disk directly, without going through the implementation lane's own private struct
//! (`EncryptedStoredKey`).

use std::io::Write;
use std::path::PathBuf;

use gx_witness::keys::KeyPair;

const PASSPHRASE: &str = "audit-m9-correct-passphrase-not-a-real-secret";

fn temp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "gx_audit_m9_p2_{}_{}_{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

fn write_bytes(path: &std::path::Path, bytes: &[u8]) {
    let mut f = std::fs::File::create(path).expect("create");
    f.write_all(bytes).expect("write");
    // `check_permissions` (keys.rs) refuses a file group/other can read on unix; match what
    // `write_key_file` itself sets so tamper cases are refused for the *content* reason under
    // test, not a permission reason unrelated to it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    }
}

/// Flip every bit of one byte (`b ^= 0xFF`) at `offset`, leaving every other byte untouched.
fn flip_byte_at(mut bytes: Vec<u8>, offset: usize) -> Vec<u8> {
    if offset < bytes.len() {
        bytes[offset] ^= 0xFF;
    }
    bytes
}

/// The core attack: an encrypted key file, tampered one byte at a time across a spread of
/// offsets covering the whole file (not just the ciphertext), each attempt independent (starting
/// from the same untampered original each time -- one flip per attempt, not accumulating).
#[test]
fn tampering_any_single_byte_of_an_encrypted_key_file_is_always_caught() {
    let key = KeyPair::generate("audit-m9-p2-tamper").expect("a fresh key pair");
    let original_public = key.public();
    let path = temp_path("tamper.key");
    key.save_encrypted(&path, PASSPHRASE)
        .expect("save_encrypted must succeed with a non-empty passphrase");
    let original_bytes = std::fs::read(&path).expect("read back the file this lane just wrote");
    assert!(
        original_bytes.len() > 32,
        "an encrypted key file shorter than this is not the shape under test"
    );

    // The untampered file must still load and match, before any attack -- the control.
    let loaded = KeyPair::load_encrypted(&path, PASSPHRASE).expect("untampered file must load");
    assert_eq!(
        loaded.public().to_bytes(),
        original_public.to_bytes(),
        "control: the untampered round trip must reproduce the same public key"
    );

    // A spread of offsets across the whole file: first byte, several deciles, and the last byte.
    // Not exhaustive (every byte would be `len` attempts and this is an audit lane, not a fuzzer),
    // but wide enough to cross the CBOR map header, every declared field (format tag, key_id,
    // algorithm, three KDF cost integers, salt, nonce, ciphertext) and the AEAD tag at the end.
    let len = original_bytes.len();
    let mut offsets: Vec<usize> = (0..10).map(|decile| (len * decile) / 10).collect();
    offsets.push(len - 1);
    offsets.sort_unstable();
    offsets.dedup();

    let mut fail_open = 0usize;
    let mut kinds: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for offset in &offsets {
        let tampered = flip_byte_at(original_bytes.clone(), *offset);
        let tpath = temp_path(&format!("tamper_at_{offset}.key"));
        write_bytes(&tpath, &tampered);
        match KeyPair::load_encrypted(&tpath, PASSPHRASE) {
            Ok(loaded_after_tamper) => {
                // Even an `Ok` here is only safe if it is not silently a *different, valid-looking*
                // key -- AEAD authentication should make this branch unreachable, and reaching it
                // at all is the fail-open condition regardless of what key comes out.
                fail_open += 1;
                eprintln!(
                    "FAIL_OPEN offset={offset} tampered file LOADED (public key changed: {})",
                    loaded_after_tamper.public().to_bytes() != original_public.to_bytes()
                );
            }
            Err(e) => {
                *kinds.entry(e.kind()).or_insert(0) += 1;
            }
        }
        let _ = std::fs::remove_file(&tpath);
    }
    let _ = std::fs::remove_file(&path);

    println!(
        "AUDIT_M9_P2_KEY_TAMPER_OFFSETS={} FAIL_OPEN={fail_open} ERROR_KINDS={kinds:?}",
        offsets.len()
    );
    assert_eq!(
        fail_open,
        0,
        "every single-byte tamper across the whole file's offset spread must be caught -- {} \
         offsets tried, {} succeeded silently",
        offsets.len(),
        fail_open
    );
}

/// Truncation (a partial write / a disk full mid-write) must be a named refusal, not a panic and
/// not a silent partial key.
#[test]
fn a_truncated_encrypted_key_file_is_refused_not_panicked_on() {
    let key = KeyPair::generate("audit-m9-p2-truncate").expect("a fresh key pair");
    let path = temp_path("truncate.key");
    key.save_encrypted(&path, PASSPHRASE).expect("save");
    let original_bytes = std::fs::read(&path).expect("read");
    let mut fail_open = 0usize;
    for cut in [
        1usize,
        original_bytes.len() / 4,
        original_bytes.len() / 2,
        original_bytes.len() - 1,
    ] {
        let truncated = &original_bytes[..cut.max(1)];
        let tpath = temp_path(&format!("truncate_at_{cut}.key"));
        write_bytes(&tpath, truncated);
        if KeyPair::load_encrypted(&tpath, PASSPHRASE).is_ok() {
            fail_open += 1;
            eprintln!("FAIL_OPEN: a file truncated to {cut} bytes still loaded");
        }
        let _ = std::fs::remove_file(&tpath);
    }
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        fail_open, 0,
        "no truncated prefix of an encrypted key file may load"
    );
    println!("AUDIT_M9_P2_TRUNCATION_FAIL_OPEN={fail_open}");
}

/// An equal-length wrong passphrase (differs by one character only, same byte length as the
/// correct one) is refused with the named `WrongPassphrase` error -- the passphrase-side twin of
/// AC-P2-2's equal-length wrong *token* test, applied to the KDF input rather than the Bearer
/// header.
#[test]
fn an_equal_length_wrong_passphrase_is_refused_by_name() {
    let key = KeyPair::generate("audit-m9-p2-passphrase").expect("a fresh key pair");
    let path = temp_path("passphrase.key");
    key.save_encrypted(&path, PASSPHRASE).expect("save");

    // Same length as PASSPHRASE, differs in the last character only.
    let mut wrong: Vec<u8> = PASSPHRASE.as_bytes().to_vec();
    *wrong.last_mut().unwrap() ^= 0x01;
    let wrong =
        String::from_utf8(wrong).expect("still valid UTF-8 after a bit flip on an ASCII byte");
    assert_eq!(
        wrong.len(),
        PASSPHRASE.len(),
        "the attack is equal-length by construction"
    );
    assert_ne!(wrong, PASSPHRASE);

    let err = KeyPair::load_encrypted(&path, &wrong).expect_err("a wrong passphrase must refuse");
    assert_eq!(err.kind(), "WrongPassphrase");
    let _ = std::fs::remove_file(&path);
    println!(
        "AUDIT_M9_P2_EQUAL_LENGTH_WRONG_PASSPHRASE=REFUSED kind={}",
        err.kind()
    );
}

/// Control: a plaintext (unencrypted) key file, fed to `load_encrypted`, must be refused by name
/// (`KeyFormat`) rather than mistaken for a corrupted encrypted one -- the reverse direction of
/// `req/131`'s own `ac_p2_3_an_encrypted_key_fails_load_with_a_named_error` (plain fed to
/// `load_encrypted`, plaintext=false there is `load` given an encrypted file; here it is
/// `load_encrypted` given a plaintext file, the pairing that file's own test does not cover).
#[test]
fn a_plaintext_key_fed_to_load_encrypted_is_refused_by_name() {
    let key = KeyPair::generate("audit-m9-p2-plaintext").expect("a fresh key pair");
    let path = temp_path("plaintext.key");
    key.save(&path).expect("plain save");
    let err = KeyPair::load_encrypted(&path, PASSPHRASE)
        .expect_err("a plaintext key file must not be readable by load_encrypted");
    assert_eq!(err.kind(), "KeyFormat");
    let _ = std::fs::remove_file(&path);
    println!(
        "AUDIT_M9_P2_PLAINTEXT_INTO_LOAD_ENCRYPTED=REFUSED kind={}",
        err.kind()
    );
}
