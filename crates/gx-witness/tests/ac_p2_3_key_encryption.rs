// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-P2-3 (`req/130` §1 item2 / §3, NFR-010) — key-at-rest encryption: generate → save encrypted
//! → load back, a wrong passphrase named apart from a corrupted file, and the plaintext road
//! (`KeyPair::save`/`load`, unchanged) still working exactly as before P2. (sem: SEM-gx-witness-173,
//! SEM-gx-witness-174, SEM-gx-witness-175, SEM-gx-witness-176, SEM-gx-witness-177, SEM-gx-witness-178)
//!
//! `req/130` §3 AC-P2-3 verbatim: "an encrypted key file's gen → load round trip is green; a wrong
//! passphrase = a named refusal; backward compatibility loading old plaintext keys (an existing key
//! becoming unreadable = a breaking change is forbidden)".
//!
//! 🔴 **NFR-010's default-encryption reading is not implemented here, and `req/131` §3 says why**:
//! 33's NFR-010 literal text ("v0.1 is file-based encrypted storage (at rest)" + judgement method
//! "a test confirming the secret key does not exist on disk in plaintext") reads as a **default**-
//! encryption requirement, and ruling 2's condition ("if the literal wording requires default
//! encryption, do not implement it and send it back") fires on that reading. What
//! this file tests is the **capability** ruling 1/ruling 2 both ask for regardless of the default question
//! — opt-in encryption, correctly implemented — not a claim that NFR-010 is satisfied by default.

mod support;

use std::path::PathBuf;

use gx_witness::keys::{KeyPair, ENCRYPTED_KEY_FORMAT, KEY_ALGORITHM};
use gx_witness::Error;
use support::keypair;

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("gx-acp23-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        Self(dir)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// AC-P2-3's core clause: gen → save_encrypted → load_encrypted round trips.
#[test]
fn ac_p2_3_the_encrypted_round_trip() {
    let scratch = Scratch::new("round-trip");
    let path = scratch.path("signing.key");
    let key = KeyPair::generate("key-p2-1").expect("the OS supplies entropy");

    key.save_encrypted(&path, "correct horse battery staple")
        .expect("the encrypted key is written");
    let loaded = KeyPair::load_encrypted(&path, "correct horse battery staple")
        .expect("and read back under the same passphrase");

    assert_eq!(loaded.key_id(), key.key_id());
    assert_eq!(
        loaded.signing_key().to_bytes(),
        key.signing_key().to_bytes(),
        "the round trip has to return the same seed, not merely a key with the same id"
    );
}

/// 🔴 **ruling 1's condition**: a wrong passphrase is a **named** refusal
/// ([`gx_witness::Error::WrongPassphrase`]), distinct from a structurally corrupted file.
#[test]
fn ac_p2_3_a_wrong_passphrase_is_named_apart_from_a_corrupted_file() {
    let scratch = Scratch::new("wrong-passphrase");
    let path = scratch.path("signing.key");
    keypair(11)
        .save_encrypted(&path, "the real passphrase")
        .expect("written");

    match KeyPair::load_encrypted(&path, "not the real passphrase") {
        Err(Error::WrongPassphrase { path: named }) => assert_eq!(named, path),
        other => panic!("expected WrongPassphrase, got {other:?}"),
    }

    // And the right passphrase, on the same file, still works — the refusal above is about the
    // passphrase and not about the file having gone bad in the meantime.
    assert!(KeyPair::load_encrypted(&path, "the real passphrase").is_ok());
}

/// A file `KeyPair::save` wrote (plaintext) is not `ENCRYPTED_KEY_FORMAT`'s shape, and
/// `load_encrypted` says so by name rather than treating the passphrase as wrong.
#[test]
fn ac_p2_3_a_plaintext_file_is_refused_by_load_encrypted_as_the_wrong_shape() {
    let scratch = Scratch::new("plain-via-encrypted");
    let path = scratch.path("signing.key");
    keypair(12).save(&path).expect("written plaintext");

    match KeyPair::load_encrypted(&path, "anything") {
        Err(Error::KeyFormat { detail }) => {
            assert!(
                detail.contains("load"),
                "the refusal names the road: {detail}"
            );
        }
        other => panic!("expected KeyFormat (wrong shape), got {other:?}"),
    }
}

/// 🔴 The reverse of the case above: `load` (plaintext-only) refuses an encrypted file **by name**
/// ([`gx_witness::Error::KeyEncrypted`]) rather than misreading it as a corrupt plain key.
#[test]
fn ac_p2_3_load_names_an_encrypted_file_rather_than_misreading_it() {
    let scratch = Scratch::new("encrypted-via-plain");
    let path = scratch.path("signing.key");
    let key = keypair(13);
    key.save_encrypted(&path, "a passphrase")
        .expect("written encrypted");

    match KeyPair::load(&path) {
        Err(Error::KeyEncrypted {
            path: named,
            key_id,
        }) => {
            assert_eq!(named, path);
            assert_eq!(key_id, key.key_id().to_string());
        }
        other => panic!("expected KeyEncrypted, got {other:?}"),
    }
}

/// 🔴 **AC-P2-3's backward-compatibility clause**: a key `save` wrote before P2 (or after —
/// the function is unchanged) still `load`s exactly as it always has. No breaking change.
#[test]
fn ac_p2_3_a_plaintext_key_still_loads_exactly_as_before() {
    let scratch = Scratch::new("backward-compat");
    let path = scratch.path("signing.key");
    let key = keypair(14);
    key.save(&path).expect("written");

    let loaded = KeyPair::load(&path).expect("a plain key file still loads with no passphrase");
    assert_eq!(loaded.key_id(), key.key_id());
    assert_eq!(
        loaded.signing_key().to_bytes(),
        key.signing_key().to_bytes()
    );
}

/// An empty passphrase is refused at `save_encrypted` — encrypting under no secret would be a
/// plaintext file wearing an encrypted one's shape, and 44 §2.5's "empty token" reasoning applies
/// here for the same reason (`gx_api::auth::Bearer::is_unset`).
#[test]
fn ac_p2_3_an_empty_passphrase_is_refused() {
    let scratch = Scratch::new("empty-passphrase");
    let path = scratch.path("signing.key");
    match keypair(15).save_encrypted(&path, "") {
        Err(Error::KeyFormat { detail }) => assert!(detail.contains("passphrase")),
        other => panic!("expected KeyFormat, got {other:?}"),
    }
    assert!(!path.exists(), "a refused write leaves no file behind");
}

/// 🔴 The encrypted file carries neither the raw seed nor a base64/hex spelling of it — the whole
/// point of encrypting it. Same shape as `key_surface.rs`'s stdout leak check, over the file
/// instead of stdout.
#[test]
fn ac_p2_3_the_encrypted_file_does_not_carry_the_seed() {
    let scratch = Scratch::new("no-leak");
    let path = scratch.path("signing.key");
    let key = keypair(16);
    key.save_encrypted(&path, "a passphrase").expect("written");

    let bytes = std::fs::read(&path).expect("readable");
    let text = String::from_utf8_lossy(&bytes);
    let seed = key.signing_key().to_bytes();
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();

    assert!(
        !bytes.windows(seed.len()).any(|w| w == seed),
        "the raw seed bytes appear in the encrypted file"
    );
    assert!(
        !text.contains(&hex),
        "a hex spelling of the seed appears in the file"
    );
    assert!(
        !text.contains(&gx_core::b64::encode(&seed)),
        "a base64 spelling of the seed appears in the file"
    );

    // Positive control: the *plaintext* file of the same key does carry it, so the assertions
    // above are checking encryption and not merely checking that CBOR does not spell bytes as hex.
    let plain_path = scratch.path("plain.key");
    key.save(&plain_path).expect("written plaintext");
    let plain_bytes = std::fs::read(&plain_path).expect("readable");
    assert!(
        plain_bytes.windows(seed.len()).any(|w| w == seed),
        "the positive control failed: the plaintext file does not carry the seed either, which \
         means this test's method of looking is broken"
    );
}

/// 🔴 The versioned header (`req/130`'s "format=versioned header"): [`ENCRYPTED_KEY_FORMAT`] and
/// [`KEY_ALGORITHM`] are both present in the file as plain text a future reader can find.
#[test]
fn ac_p2_3_the_format_and_algorithm_tags_are_in_the_file() {
    let scratch = Scratch::new("header");
    let path = scratch.path("signing.key");
    keypair(17)
        .save_encrypted(&path, "a passphrase")
        .expect("written");

    let bytes = std::fs::read(&path).expect("readable");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains(ENCRYPTED_KEY_FORMAT),
        "the file does not carry its own format tag"
    );
    assert!(
        text.contains(KEY_ALGORITHM),
        "the file does not carry its own algorithm tag"
    );
}

/// Two encryptions of the same key under the same passphrase produce **different** files — a fresh
/// salt and a fresh nonce each time, which is what keeps two on-disk copies of one key from leaking
/// that they are copies of one key.
#[test]
fn ac_p2_3_two_encryptions_of_the_same_key_are_different_files() {
    let scratch = Scratch::new("nondeterministic");
    let key = keypair(18);
    let (a, b) = (scratch.path("a.key"), scratch.path("b.key"));
    key.save_encrypted(&a, "same passphrase").expect("written");
    key.save_encrypted(&b, "same passphrase").expect("written");

    assert_ne!(
        std::fs::read(&a).expect("readable"),
        std::fs::read(&b).expect("readable"),
        "two encryptions of one key under one passphrase must not be byte-identical"
    );
    // And both still decrypt to the same key, so the difference above is the salt/nonce and not a
    // second key having been generated by accident.
    let from_a = KeyPair::load_encrypted(&a, "same passphrase").expect("a decrypts");
    let from_b = KeyPair::load_encrypted(&b, "same passphrase").expect("b decrypts");
    assert_eq!(
        from_a.signing_key().to_bytes(),
        from_b.signing_key().to_bytes()
    );
}

/// A corrupted encrypted file (a flipped byte **inside the ciphertext**) fails to decrypt even
/// under the right passphrase — the AEAD tag catches tampering, and the refusal is
/// [`gx_witness::Error::WrongPassphrase`] (ruling 1's one face for both wrong-key and tampered-data).
///
/// The ciphertext is 48 bytes (a 32-byte seed plus ChaCha20-Poly1305's 16-byte tag), which is
/// canonical DAG-CBOR's `0x58 0x30` byte-string header (major type 2, one-byte length 48) — the
/// only field this record carries at that length, so the pattern locates it without decoding the
/// record (this crate's own [`EncryptedStoredKey`](gx_witness::keys) is private to `keys.rs` and
/// not reachable from an integration test). If the pattern is ever absent the test says so rather
/// than silently flipping the wrong byte and passing for the wrong reason.
#[test]
fn ac_p2_3_a_tampered_ciphertext_is_refused_even_with_the_right_passphrase() {
    let scratch = Scratch::new("tampered");
    let path = scratch.path("signing.key");
    keypair(19)
        .save_encrypted(&path, "the real passphrase")
        .expect("written");

    let mut bytes = std::fs::read(&path).expect("readable");
    let header = bytes
        .windows(2)
        .position(|w| w == [0x58, 0x30])
        .unwrap_or_else(|| {
            panic!("the 48-byte ciphertext's CBOR header (0x58 0x30) was not found")
        });
    let flip_at = header + 2 + 10; // ten bytes into the 48-byte ciphertext payload
    bytes[flip_at] ^= 0xFF;
    std::fs::write(&path, &bytes).expect("rewritten");

    match KeyPair::load_encrypted(&path, "the real passphrase") {
        Err(Error::WrongPassphrase { .. }) => {}
        other => panic!("expected WrongPassphrase (tampered ciphertext), got {other:?}"),
    }
}
