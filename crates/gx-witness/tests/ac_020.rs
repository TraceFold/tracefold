// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-020 (FR-020) — generate a key, save it, load it, and verify a receipt with what came back.
//! (sem: SEM-gx-witness-158, SEM-gx-witness-159)
//!
//! AC-020 verbatim: "Given: `gx_witness::keys`'s Ed25519 key-generation library API (at M2, before
//! the CLI is wired up, so it is verified at the library level). When: generate a key pair → save
//! to a file → reload it, and call the verification function with the loaded key against a Receipt
//! gx-witness issued (signing is always internal engine processing; no independent "sign command"
//! exists) via the pipeline. Then: the round trip of generate → save → load → verify succeeds.
//! Reconfirmation at the CLI level via `gx key gen`/`gx receipt verify` happens in M6's E2E AC
//! (AC-054, AC-057)." Judgement method: `unit + integration (direct library API calls)`, M2.
//!
//! FR-020 itself ends "the round trip can be tested **via the CLI**", which the AC reads down to a library
//! round trip for M2 and hands to M6. req/49 §3 M2-14 raised that as an erratum and no ruling has
//! landed; this file implements the AC's reading, which is the only one available before a CLI
//! exists.
//!
//! # The round trip is generated, not seeded
//!
//! `ac_020_the_round_trip` uses [`KeyPair::generate`] -- real entropy -- because that is what the AC
//! names and because it is the one path the seeded fixtures never exercise. Everything else here
//! seeds, so a failure elsewhere is reproducible.
//!
//! # What is not tested, and is a real gap
//!
//! The file permission rule is unix-only (`keys.rs` says why), so on Windows two of these tests
//! assert nothing. They are `#[cfg(unix)]` rather than silently vacuous -- a skipped test that
//! looks like a passing one is the fail-open req/29 §4 forbids -- and req/54 §5 records it.

mod support;

use std::path::PathBuf;

use gx_core::VerdictKind;
use gx_witness::keys::{KeyPair, KEY_ALGORITHM};
use gx_witness::receipt::verify_offline;
use gx_witness::Error;
use support::{issue, keypair, verdict_payload};

/// A path in the process's temporary directory, unique per test, cleaned up by [`Scratch`].
///
/// `std::env::temp_dir` rather than a crate: the ledger suites of hand 3 do the same, and a key
/// file is one file. The name carries the test's own label so a leftover is attributable.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("gx-ac020-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Write raw bytes as a key file would be written, permissions included.
    ///
    /// `std::fs::write` leaves `0o644` under the usual umask, which [`KeyPair::load`] refuses
    /// before it looks at the contents -- so a test about *contents* has to set the mode, or it
    /// measures the permission rule a second time and calls it something else.
    fn write_owner_only(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, bytes).expect("written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        }
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------

/// AC-020 verbatim: generate → save → load → verify.
#[test]
fn ac_020_the_round_trip() {
    let scratch = Scratch::new("round-trip");
    let path = scratch.path("signing.key");

    let generated = KeyPair::generate("key-operator-1").expect("the OS supplies entropy");
    generated.save(&path).expect("the key is written");
    let loaded = KeyPair::load(&path).expect("and read back");

    assert_eq!(loaded.key_id(), generated.key_id());

    // The receipt is issued with the *generated* key and verified with the *loaded* one, which is
    // the direction that makes the round trip mean something: a `load` that returned the key it was
    // handed in memory would pass a test that used one of them twice.
    let receipt = issue(
        &verdict_payload(VerdictKind::Admit, &generated, 0),
        &generated,
    );
    let checks = verify_offline(&receipt, &loaded.verifying(), None)
        .expect("the loaded key verifies the receipt the generated one signed");
    assert!(checks.verified());
}

/// Two generated keys are different keys. `generate` reading a constant would pass every other test
/// in this file.
#[test]
fn ac_020_two_generated_keys_are_not_the_same_key() {
    let a = KeyPair::generate("key-a").expect("entropy");
    let b = KeyPair::generate("key-b").expect("entropy");
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &a, 0), &a);

    assert!(matches!(
        verify_offline(&receipt, &b.verifying(), None),
        Err(Error::SignatureInvalid { .. })
    ));
}

/// A seeded key is the same key every time, which is what makes every other suite reproducible.
#[test]
fn ac_020_the_same_seed_gives_the_same_key() {
    let a = keypair(9);
    let b = keypair(9);
    let receipt = issue(&verdict_payload(VerdictKind::Deny, &a, 0), &a);
    assert!(verify_offline(&receipt, &b.verifying(), None)
        .expect("one key, two constructions")
        .verified());
}

// ---------------------------------------------------------------------------
// The file
// ---------------------------------------------------------------------------

/// Saving the same key twice writes the same bytes. Canonical encoding buys nothing about identity
/// here -- nothing hashes a key file -- but it does make the file a function of the key, which is
/// a property a corrupted write cannot fake.
#[test]
fn ac_020_the_file_is_a_function_of_the_key() {
    let scratch = Scratch::new("deterministic");
    let key = keypair(3);
    let (a, b) = (scratch.path("a.key"), scratch.path("b.key"));

    key.save(&a).expect("written");
    key.save(&b).expect("written");
    assert_eq!(
        std::fs::read(&a).expect("readable"),
        std::fs::read(&b).expect("readable")
    );
}

/// The file names its algorithm, and a file naming another one is refused rather than read as
/// Ed25519. The mechanism a second algorithm would arrive through, tested before there is one.
#[test]
fn ac_020_a_file_from_another_algorithm_is_refused() {
    let scratch = Scratch::new("algorithm");
    let path = scratch.path("other.key");
    keypair(4).save(&path).expect("written");

    let bytes = std::fs::read(&path).expect("readable");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains(KEY_ALGORITHM),
        "the record does not name its algorithm"
    );

    // Rewrite the record with a different algorithm of the same length, so the CBOR framing is
    // untouched and the refusal comes from the check rather than from the decoder.
    assert_eq!(KEY_ALGORITHM.len(), "ed25519".len());
    let swapped: Vec<u8> = bytes
        .windows(KEY_ALGORITHM.len())
        .enumerate()
        .find(|(_, w)| *w == KEY_ALGORITHM.as_bytes())
        .map(|(at, _)| {
            let mut out = bytes.clone();
            out[at..at + KEY_ALGORITHM.len()].copy_from_slice(b"ed448xx");
            out
        })
        .expect("the algorithm string is in the file");
    let path = scratch.write_owner_only("other.key", &swapped);

    match KeyPair::load(&path) {
        Err(Error::KeyFormat { detail }) => assert!(detail.contains("algorithm")),
        other => panic!("expected a format refusal, got {other:?}"),
    }
}

/// A file that is not a key record at all.
#[test]
fn ac_020_a_file_that_is_not_a_key_is_refused() {
    let scratch = Scratch::new("garbage");
    let path = scratch.write_owner_only("garbage.key", b"not cbor at all");
    assert!(matches!(KeyPair::load(&path), Err(Error::Canon(_))));
}

/// A missing file names the action that failed, not just the errno.
#[test]
fn ac_020_a_missing_file_says_what_it_was_doing() {
    let scratch = Scratch::new("missing");
    match KeyPair::load(scratch.path("absent.key")) {
        Err(Error::Io { action, kind, .. }) => {
            assert_eq!(kind, std::io::ErrorKind::NotFound);
            assert!(!action.is_empty());
        }
        other => panic!("expected an I/O refusal, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The permission rule (unix only -- see this file's header)
// ---------------------------------------------------------------------------

/// A key this crate wrote is readable by its owner and by nobody else.
#[cfg(unix)]
#[test]
fn ac_020_a_saved_key_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new("mode");
    let path = scratch.path("owner-only.key");
    keypair(5).save(&path).expect("written");

    let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the key was written as {mode:o}");
    println!("AC020_KEY_FILE_MODE={mode:o}");
}

/// A key file anyone can read is refused rather than loaded. A secret whose file is `0o644` has
/// already leaked, and reading it silently would make the leak invisible.
#[cfg(unix)]
#[test]
fn ac_020_a_world_readable_key_is_refused() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new("leaky");
    let path = scratch.path("leaky.key");
    keypair(6).save(&path).expect("written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");

    match KeyPair::load(&path) {
        Err(Error::KeyPermissions { mode, .. }) => assert_eq!(mode, 0o644),
        other => panic!("a world-readable key was accepted: {other:?}"),
    }

    // And the same file, made private again, loads. Without this the test above would pass for an
    // implementation that refused every key file.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    assert!(KeyPair::load(&path).is_ok());
}

/// The debug rendering does not carry the secret. `KeyPair` is the one type in this workspace that
/// holds one, and `{:?}` in a log line is how a secret reaches a log aggregator.
#[test]
fn ac_020_the_debug_rendering_withholds_the_secret() {
    let key = keypair(7);
    let rendered = format!("{key:?}");
    let secret = key.signing_key().to_bytes();

    assert!(rendered.contains(key.key_id()));
    // Every spelling the derive would have produced: hex, the decimal list, and the base64 face
    // M2H1-4 gives raw bytes elsewhere.
    let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    let decimal: String = secret
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    for spelling in [hex, decimal, gx_core::b64::encode(&secret)] {
        assert!(
            !rendered.contains(&spelling),
            "the debug output carries the secret: {rendered}"
        );
    }
}
