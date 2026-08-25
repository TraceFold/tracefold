// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Ed25519 key pairs, and the file one is kept in. (sem:
//! SEM-gx-witness-112, SEM-gx-witness-113, SEM-gx-witness-114, SEM-gx-witness-115, SEM-gx-witness-116,
//! SEM-gx-witness-117, SEM-gx-witness-118, SEM-gx-witness-119, SEM-gx-witness-120, SEM-gx-witness-121,
//! SEM-gx-witness-122, SEM-gx-witness-123, SEM-gx-witness-124, SEM-gx-witness-125, SEM-gx-witness-126,
//! SEM-gx-witness-127, SEM-gx-witness-128, SEM-gx-witness-129, SEM-gx-witness-130, SEM-gx-witness-131,
//! SEM-gx-witness-132, SEM-gx-witness-133, SEM-gx-witness-134, SEM-gx-witness-135, SEM-gx-witness-136,
//! SEM-gx-witness-137, SEM-gx-witness-138, SEM-gx-witness-139, SEM-gx-witness-140, SEM-gx-witness-141,
//! SEM-gx-witness-142, SEM-gx-witness-143, SEM-gx-witness-144, SEM-gx-witness-145, SEM-gx-witness-146,
//! SEM-gx-witness-147)
//!
//! Spec: 32 FR-020 for the requirement, 34 AC-020 for how it is judged, 45 ASM-9 for the storage
//! scope, 42 §3.2 for `KeyId`.
//!
//! # Everything in this file is derived, and that is the ticket
//!
//! 42 §0's gx-witness row names four modules and none of them is this one; 41 §2 lists
//! `src/{lib,provenance,receipt,evidence,dsse,keys}.rs` and gives `keys.rs` no types at all. So
//! there is no field table to transcribe: the key record's shape, the file's layout, and the
//! permission rule below are **derived** from FR-020's one sentence -- "key generation and local key
//! storage (v0.1 scope: file-based storage)" -- in the standing E-M2-17 gave `CheckerResultRef` and req/52
//! §1.1 gave `LedgerStore`. req/49 §3 M2-14 raised it and no ruling has landed; req/54 §4 raises it
//! again with an implementation attached, which is a better thing to rule on than a proposal.
//!
//! # FR-020 says CLI and AC-020 says otherwise, in the spec's own words
//!
//! FR-020 verbatim ends "the round trip can be tested **via the CLI**" while AC-020 reads itself down
//! to "at M2, before the CLI is wired up, so it is verified at the library level" and hands the CLI round trip to AC-054/AC-057 in
//! M6. This hand implements the library API. The erratum is req/49 §3 M2-14's and is not closed
//! here.
//!
//! # What this module does not do
//!
//! No rotation, no revocation list, no passphrase, no hardware token, no second tier. 45 ASM-45-1
//! keeps v0.1 to "a single tier (signed only)" and ASM-45-2 puts revocation out of scope, so a key
//! file that leaks is contained by the filesystem and by nothing else. That is a real property of
//! v0.1 and it is written here rather than left for a reader to infer from the absence of code.
//!
//! 🔴 **Superseded in part by M7 hand 2 (FR-M7-3), and kept rather than rewritten (no-delete).**
//! The paragraph above records what was true from M5 to here and why. What has changed since:
//!
//! * **rotation and a revocation list exist** — [`RevocationEntry`], [`RevocationLedger`],
//!   [`Retroaction`], and `gx key rotate|revoke` above them. Ruling #6 (req/98 §3-2) is the ruling;
//!   45 §2's TH-5 mitigation column had named the control all along and M5H8-3 read it as a roadmap
//!   entry precisely because no code answered to it.
//! * **still absent**: passphrase, hardware token, second tier. A key file that leaks is still
//!   contained by the filesystem and by nothing else *until somebody revokes it*, and a revocation
//!   is a statement to verifiers rather than a lock on the file.
//! * 🔴 **a citation in the paragraph above is wrong and is corrected here rather than edited into
//!   silence**: 45 §5's ASM-45-1 is about **evidence collectors** ("trust in an Evidence collector is
//!   a single tier in v0.1 (signed only is adopted)"), not about keys, and ASM-45-2 does not put revocation out of
//!   scope — it fixes its **default**: "on key revocation, a receipt issued before the revocation is
//!   not retroactively invalidated ... consulting the revocation list is optional, at the verifier's
//!   discretion". The sentence "a single tier" belongs to the evidence
//!   row. `req/100` §5 raises the mis-citation; the reading that matters for this module is the
//!   primary text, quoted on [`RevocationEntry`].
//!
//! # 🔴 45 TH-5's mitigation column, and how it is to be read (**M5H8-3**)
//!
//! 45 §2's TH-5 row lists "key rotation (a generation-numbered key id)" among the controls, in the present
//! tense, next to the paragraph above that says there is none. req/86 §5.2 raised the contradiction
//! and `req/38_ERRATA_2026-08-07.md` §45 ruled it, verbatim:
//!
//! > **M5H8-3, adopted (a)**: an erratum that reads 45 TH-5's mitigation column as "key rotation is
//! > v0.2 (ASM-45-2)". §3's residual register ("medium") is already correct.
//!
//! So the row is a **roadmap entry read as a control**, and the erratum is that the rotation it
//! names belongs to v0.2 under ASM-45-2. 45 §3's residual register already grades TH-5 "medium" and
//! is right as written; 45 itself is not edited (`req/spec/` is 1 byte unchanged), because the
//! erratum ledger is req/38 and a spec that is quietly corrected is a spec nobody can date. What
//! v0.1 has against a compromised key is what this module implements and no more.

use core::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use argon2::{
    Algorithm as Argon2Algorithm, Argon2, Block, Params as Argon2Params, Version as Argon2Version,
};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key as AeadKey, Nonce as AeadNonce};
use ed25519_dalek::{SigningKey, VerifyingKey, SECRET_KEY_LENGTH};
use gx_canon::cbor;
use gx_core::{KeyId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::dsse::{DsseEnvelope, REVOCATION_PAYLOAD_TYPE, STANDING_PAYLOAD_TYPE};
use crate::{Error, Result};

/// The algorithm string written into a key file.
///
/// One value today. It is written down rather than assumed so that a file produced by a future
/// version with a second algorithm is *refused* by this one instead of read as Ed25519 -- the same
/// argument `gx_canon::cid::DIGEST_ALGORITHM` makes for naming the hash.
pub const KEY_ALGORITHM: &str = "ed25519";

// ---------------------------------------------------------------------------
// Key-at-rest encryption (**P2 item2**, `req/130` §1, NFR-010, opt-in — ruling 2)
// ---------------------------------------------------------------------------

/// The tag written into an **encrypted** key file, so a file from a future format (a different KDF
/// or cipher) is refused rather than misread -- [`KEY_ALGORITHM`]'s argument, one level up, applied
/// to the envelope this time (`req/130` §1 item2: "format=versioned header").
pub const ENCRYPTED_KEY_FORMAT: &str = "gx-key-encrypted-argon2id-chacha20poly1305-v1";

/// Argon2id parameters this version writes with when a caller does not choose its own (OWASP's
/// 2024 minimum for Argon2id: 19 MiB, 2 iterations, 1 lane). Read back from the file at load time
/// rather than assumed (`kdf_m_cost_kib`/`kdf_t_cost`/`kdf_p_cost` below), so a future default does
/// not change what an existing file decrypts under.
const KDF_M_COST_KIB: u32 = 19 * 1024;
const KDF_T_COST: u32 = 2;
const KDF_P_COST: u32 = 1;

/// Argon2's own minimum, the recommended width (RFC 9106 §4).
const KDF_SALT_LEN: usize = 16;

/// ChaCha20-Poly1305's key and nonce widths (RFC 8439).
const AEAD_KEY_LEN: usize = 32;
const AEAD_NONCE_LEN: usize = 12;

/// A key pair and the id it signs under.
///
/// The `KeyId` travels with the key because a signature is only checkable against the id it was
/// offered under (42 §3.2: "the same namespace as DSSE's `keyid`"), and a caller holding a bare `SigningKey`
/// would have to remember which id it belongs to -- which is the kind of pairing that drifts.
///
/// `VerifyingKey` is derived once at construction and kept, rather than recomputed per verification:
/// deriving it is a scalar multiplication, and a verifier checking a thousand receipts against one
/// key should pay for it once.
pub struct KeyPair {
    key_id: KeyId,
    signing: SigningKey,
    verifying: VerifyingKey,
}

/// Deliberately opaque, for [`gx_core::Cid`]'s reason turned up a notch: the secret is in this
/// struct, and a `{:?}` in a log line is the classic way one reaches a log aggregator.
impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyPair({}, secret withheld)", self.key_id)
    }
}

/// A public key and the id it verifies under: everything an offline verifier needs (AC-018/AC-070).
///
/// Borrowed rather than owned so that a verifier holding one [`KeyPair`] or one loaded public key
/// can hand it to a verification without a clone per receipt. The fields are public because both of
/// them are public information; there is no invariant to protect.
#[derive(Clone, Copy, Debug)]
pub struct VerifyingKeyRef<'a> {
    /// The id the key verifies under -- the DSSE `keyid` namespace (42 §3.2), matched against
    /// `ReceiptPayload.key_id` before any curve operation is spent.
    pub key_id: &'a str,
    /// The Ed25519 public key itself.
    pub key: &'a VerifyingKey,
}

impl KeyPair {
    /// A fresh key pair, seeded from the operating system (FR-020).
    ///
    /// # Why the entropy is fetched here and injected nowhere
    ///
    /// 41 §6 injects randomness at the engine boundary "for deterministic replay", and that rule is
    /// about the values a replay has to reproduce. A key is not one of them: generating one is an
    /// operator action whose whole purpose is to be unpredictable, and a replay that regenerated
    /// the same key would be reproducing a secret. [`KeyPair::from_seed`] is the injected form, and
    /// it is what the tests use, so this function's only untested part is the syscall.
    ///
    /// # Errors
    /// [`Error::Entropy`] if the operating system will not supply randomness. Not swallowed and not
    /// substituted: a key generated from a weak seed is worse than no key, and every alternative to
    /// failing here is some form of guessing.
    pub fn generate(key_id: impl Into<KeyId>) -> Result<Self> {
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        getrandom::fill(&mut seed).map_err(|e| Error::Entropy {
            detail: e.to_string(),
        })?;
        Ok(Self::from_seed(key_id, &seed))
    }

    /// A key pair from a caller-supplied seed: the deterministic form of [`KeyPair::generate`].
    ///
    /// Every 32-byte value is a valid Ed25519 secret key (RFC 8032 §5.1.5 clamps the scalar), so
    /// this is infallible and there is no seed a caller can offer that this refuses.
    #[must_use]
    pub fn from_seed(key_id: impl Into<KeyId>, seed: &[u8; SECRET_KEY_LENGTH]) -> Self {
        let signing = SigningKey::from_bytes(seed);
        let verifying = signing.verifying_key();
        Self {
            key_id: key_id.into(),
            signing,
            verifying,
        }
    }

    /// The id this pair signs under (42 §3.2's DSSE `keyid` namespace). Read-only: the pairing
    /// of id and key is fixed at construction, which is the drift the struct doc refuses.
    #[must_use]
    pub fn key_id(&self) -> &KeyId {
        &self.key_id
    }

    /// The half that signs. Borrowed, never cloned out: a caller that wants to sign asks
    /// [`crate::dsse::DsseEnvelope::sign`] to do it.
    #[must_use]
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing
    }

    /// The public half, owned (**M6 hand 2**).
    ///
    /// [`KeyPair::verifying`] borrows and is what a verification takes; this is what a *distributor*
    /// needs. `gx key gen` prints `{key_id, public_key}` (44 §1.2) and `gx receipt verify` has to
    /// read one back, and between those two the value has to exist without the secret beside it.
    #[must_use]
    pub fn public(&self) -> PublicKey {
        PublicKey {
            key_id: self.key_id.clone(),
            key: self.verifying,
        }
    }

    /// The half that verifies, paired with its id (AC-020's "call the verification function with the loaded key").
    #[must_use]
    pub fn verifying(&self) -> VerifyingKeyRef<'_> {
        VerifyingKeyRef {
            key_id: &self.key_id,
            key: &self.verifying,
        }
    }

    /// Write the key pair to `path` (FR-020: "v0.1 scope: file-based storage").
    ///
    /// # The file
    ///
    /// Canonical DAG-CBOR of `StoredKey` (private), through gx-canon -- 41 §6's "every canonical
    /// encode goes through gx-canon alone" holds here as everywhere, so this crate names no codec. Nothing hashes a
    /// key file, so canonicity buys no identity; what it buys is that saving the same key twice
    /// produces the same bytes, which is a property a test can check and a corrupted file cannot
    /// fake.
    ///
    /// # The permissions
    ///
    /// `0o600` on unix, set at creation rather than after it: a `create` followed by a `chmod`
    /// leaves the secret world-readable for the width of that window. There is no equivalent on
    /// Windows and this does not pretend otherwise -- the mode is not set there, [`KeyPair::load`]
    /// does not check it there, and req/54 §5 records that the guarantee is unix-only.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be created or written, [`Error::Canon`] if the record has
    /// no canonical form (it always does -- two strings and thirty-two bytes).
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let record = StoredKey {
            algorithm: KEY_ALGORITHM.to_string(),
            key_id: self.key_id.clone(),
            secret_key: self.signing.to_bytes().to_vec(),
        };
        let bytes = cbor::encode(&record)?;
        write_key_file(path.as_ref(), &bytes)
    }

    /// 🔴 **P2 item2** (`req/130` §1) — write the key pair **encrypted**, under a passphrase.
    ///
    /// Opt-in (ruling 2): [`KeyPair::save`] above is unchanged and stays the default a caller reaches
    /// for without deciding anything, and this is the second road a caller takes on purpose. The
    /// seed is sealed with ChaCha20-Poly1305 under a key Argon2id derives from the passphrase and a
    /// fresh salt; the salt, the nonce and the KDF's own cost parameters travel in the file
    /// (`req/130`'s "format=versioned header") so a future default cost does not change what this
    /// file decrypts under.
    ///
    /// # Errors
    /// `Error::Usage`-shaped (an upper-layer notion, so no link) as [`Error::KeyFormat`] for an empty passphrase (ruling 1's KDF cannot
    /// derive anything meaningful from no secret, and an empty passphrase silently accepted would be
    /// a plaintext file wearing an encrypted one's shape), [`Error::Entropy`] if the operating
    /// system will not supply the salt or the nonce, [`Error::Io`] for the write.
    pub fn save_encrypted(&self, path: impl AsRef<Path>, passphrase: &str) -> Result<()> {
        if passphrase.is_empty() {
            return Err(Error::KeyFormat {
                detail: "an empty passphrase was offered to save_encrypted: encrypting under no \
                         secret would be a plaintext file wearing an encrypted one's shape"
                    .to_string(),
            });
        }
        let mut salt = [0u8; KDF_SALT_LEN];
        getrandom::fill(&mut salt).map_err(|e| Error::Entropy {
            detail: e.to_string(),
        })?;
        let mut nonce_bytes = [0u8; AEAD_NONCE_LEN];
        getrandom::fill(&mut nonce_bytes).map_err(|e| Error::Entropy {
            detail: e.to_string(),
        })?;

        let derived = derive_key(passphrase, &salt, KDF_M_COST_KIB, KDF_T_COST, KDF_P_COST)?;
        let key: AeadKey = derived.into();
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce: AeadNonce = nonce_bytes.into();
        let ciphertext = cipher
            .encrypt(&nonce, self.signing.to_bytes().as_slice())
            .map_err(|_| Error::KeyFormat {
                detail: "the seed could not be sealed under the derived key (RFC 8439 AEAD \
                         encryption refused)"
                    .to_string(),
            })?;

        let record = EncryptedStoredKey {
            format: ENCRYPTED_KEY_FORMAT.to_string(),
            key_id: self.key_id.clone(),
            algorithm: KEY_ALGORITHM.to_string(),
            kdf_m_cost_kib: KDF_M_COST_KIB,
            kdf_t_cost: KDF_T_COST,
            kdf_p_cost: KDF_P_COST,
            kdf_salt: salt.to_vec(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        };
        let bytes = cbor::encode(&record)?;
        write_key_file(path.as_ref(), &bytes)
    }

    /// Read a key pair back (AC-020).
    ///
    /// # 🔴 An encrypted file is refused **by name** here, not misread
    ///
    /// `req/130` §6 ruling 1's condition: "decrypt failure = a named error (do not make key corruption and a wrong passphrase wear the same face)".
    /// The plaintext shape is tried first (unchanged from before P2 — a plaintext file loads exactly
    /// as it always has, which is the whole of AC-P2-3's backward-compatibility half); a file this
    /// crate recognises as `EncryptedStoredKey`'s shape (private) is refused with [`Error::KeyEncrypted`]
    /// rather than being forced through `StoredKey`'s field table, which would otherwise report
    /// "the secret is N bytes; Ed25519 keys are 32" about a file whose secret is exactly 32 bytes
    /// and simply not readable without the passphrase [`KeyPair::load_encrypted`] takes.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be read, [`Error::Canon`] if it is neither shape's canonical
    /// DAG-CBOR, [`Error::KeyFormat`] for a record that decodes but is not an Ed25519 key of the
    /// right length, [`Error::KeyEncrypted`] for a file [`KeyPair::save_encrypted`] wrote,
    /// [`Error::KeyPermissions`] on unix if anyone but the owner can read it.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        check_permissions(path)?;
        let bytes = fs::read(path).map_err(|e| Error::io("read the key", path, &e))?;
        match cbor::decode::<StoredKey>(&bytes) {
            Ok(record) => Self::from_stored(record),
            Err(plain_err) => {
                if let Ok(enc) = cbor::decode::<EncryptedStoredKey>(&bytes) {
                    return Err(Error::KeyEncrypted {
                        path: path.to_path_buf(),
                        key_id: enc.key_id.to_string(),
                    });
                }
                Err(Error::Canon(plain_err))
            }
        }
    }

    /// 🔴 **P2 item2** — read a key [`KeyPair::save_encrypted`] wrote, under its passphrase.
    ///
    /// The KDF's cost parameters and salt are read back from the file rather than assumed
    /// (`req/130`'s versioned-header requirement), so a file this binary wrote last year still
    /// decrypts under this year's `KDF_M_COST_KIB` default changing.
    ///
    /// # 🔴 Wrong passphrase and a corrupted file wear the same face on purpose, and that face is
    /// distinct from every other refusal
    ///
    /// AEAD authentication cannot distinguish "the key was wrong" from "the ciphertext was
    /// tampered with" — that is the property, not a gap in it, and pretending otherwise would be
    /// exactly the oracle a MAC exists to deny an attacker. [`Error::WrongPassphrase`] names *that*
    /// one face; it is still a different face from [`Error::KeyFormat`] (a file whose *structure* —
    /// the format tag, the algorithm, a length — this version cannot read at all, before any
    /// decryption is attempted), which is the distinction ruling 1 asks for: key corruption (structural, caught
    /// before the passphrase is even used) and a wrong passphrase (cryptographic, the one face AEAD can
    /// give) must not wear the same face.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be read, [`Error::KeyFormat`] if it is not
    /// [`ENCRYPTED_KEY_FORMAT`]'s shape (including a plain, unencrypted key — [`KeyPair::load`] is
    /// the road for that one), [`Error::WrongPassphrase`] if the passphrase is wrong or the
    /// ciphertext is corrupted, [`Error::KeyPermissions`] on unix if anyone but the owner can read
    /// it.
    pub fn load_encrypted(path: impl AsRef<Path>, passphrase: &str) -> Result<Self> {
        let path = path.as_ref();
        check_permissions(path)?;
        let bytes = fs::read(path).map_err(|e| Error::io("read the key", path, &e))?;
        let record: EncryptedStoredKey = cbor::decode(&bytes).map_err(|_| Error::KeyFormat {
            detail: format!(
                "{} is not {ENCRYPTED_KEY_FORMAT:?}'s shape; a plain key file loads with `load` \
                 instead",
                path.display()
            ),
        })?;
        if record.format != ENCRYPTED_KEY_FORMAT {
            return Err(Error::KeyFormat {
                detail: format!(
                    "the file declares format {:?}; this version reads {ENCRYPTED_KEY_FORMAT:?} \
                     only",
                    record.format
                ),
            });
        }
        if record.algorithm != KEY_ALGORITHM {
            return Err(Error::KeyFormat {
                detail: format!(
                    "the file declares algorithm {:?}; this version reads {KEY_ALGORITHM} only",
                    record.algorithm
                ),
            });
        }
        let derived = derive_key(
            passphrase,
            &record.kdf_salt,
            record.kdf_m_cost_kib,
            record.kdf_t_cost,
            record.kdf_p_cost,
        )?;
        let nonce: [u8; AEAD_NONCE_LEN] =
            record
                .nonce
                .as_slice()
                .try_into()
                .map_err(|_| Error::KeyFormat {
                    detail: format!(
                        "the nonce is {} bytes; ChaCha20-Poly1305's is {AEAD_NONCE_LEN}",
                        record.nonce.len()
                    ),
                })?;
        let key: AeadKey = derived.into();
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce: AeadNonce = nonce.into();
        let plaintext = cipher
            .decrypt(&nonce, record.ciphertext.as_slice())
            .map_err(|_| Error::WrongPassphrase {
                path: path.to_path_buf(),
            })?;
        let seed: [u8; SECRET_KEY_LENGTH] =
            plaintext
                .as_slice()
                .try_into()
                .map_err(|_| Error::KeyFormat {
                    detail: format!(
                    "the decrypted secret is {} bytes; Ed25519 keys are {SECRET_KEY_LENGTH} (the \
                     passphrase authenticated correctly, so the file itself carries the wrong \
                     length)",
                    plaintext.len()
                ),
                })?;
        Ok(Self::from_seed(record.key_id, &seed))
    }

    /// The shared half of [`KeyPair::load`] and the plaintext branch other callers construct from —
    /// a [`StoredKey`] the canonical layer has already decoded, turned into a live key pair.
    fn from_stored(record: StoredKey) -> Result<Self> {
        if record.algorithm != KEY_ALGORITHM {
            return Err(Error::KeyFormat {
                detail: format!(
                    "the file declares algorithm {:?}; this version reads {KEY_ALGORITHM} only",
                    record.algorithm
                ),
            });
        }
        let seed: [u8; SECRET_KEY_LENGTH] =
            record
                .secret_key
                .as_slice()
                .try_into()
                .map_err(|_| Error::KeyFormat {
                    detail: format!(
                        "the secret is {} bytes; Ed25519 keys are {SECRET_KEY_LENGTH}",
                        record.secret_key.len()
                    ),
                })?;
        Ok(Self::from_seed(record.key_id, &seed))
    }
}

/// The shared write path of [`KeyPair::save`] and [`KeyPair::save_encrypted`]: `0o600` on unix, set
/// at creation rather than after it (a `create` followed by a `chmod` leaves the secret
/// world-readable for the width of that window), fsynced before the call returns. Neither the
/// plaintext nor the encrypted shape hashes the file it produces, so this function does not either
/// — it writes exactly the bytes it is given.
fn write_key_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| Error::io("open", path, &e))?;

    use std::io::Write;
    file.write_all(bytes)
        .map_err(|e| Error::io("write the key", path, &e))?;
    file.sync_all()
        .map_err(|e| Error::io("fsync the key", path, &e))?;
    Ok(())
}

/// Argon2id, at the parameters a file names, over a passphrase and a salt: the shared half of
/// [`KeyPair::save_encrypted`] and [`KeyPair::load_encrypted`].
///
/// The memory buffer is built by this function and not by `argon2`'s own `hash_password_into` --
/// that convenience needs the crate's `password-hash` feature, a PHC-string API this module has no
/// other use for, and `default-features = false` in `Cargo.toml` keeps it out of the tree
/// (ruling 1's "minimal dependencies"). [`Argon2::hash_password_into_with_memory`] is the same algorithm with the
/// buffer supplied by the caller, which is `alloc`'s `Vec` here and is unconditional in the crate
/// (no feature gate).
///
/// # Errors
/// [`Error::KeyFormat`] if the parameters a file names are outside Argon2's own bounds (a corrupted
/// or hand-edited file, since [`KeyPair::save_encrypted`] never writes an invalid set).
fn derive_key(
    passphrase: &str,
    salt: &[u8],
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<[u8; AEAD_KEY_LEN]> {
    let params =
        Argon2Params::new(m_cost_kib, t_cost, p_cost, Some(AEAD_KEY_LEN)).map_err(|e| {
            Error::KeyFormat {
                detail: format!("the file's KDF parameters are not valid Argon2id ones: {e}"),
            }
        })?;
    let mut blocks = vec![Block::default(); params.block_count()];
    let argon2 = Argon2::new(Argon2Algorithm::Argon2id, Argon2Version::V0x13, params);
    let mut out = [0u8; AEAD_KEY_LEN];
    argon2
        .hash_password_into_with_memory(passphrase.as_bytes(), salt, &mut out, &mut blocks)
        .map_err(|e| Error::KeyFormat {
            detail: format!("Argon2id could not derive a key: {e}"),
        })?;
    Ok(out)
}

/// An Ed25519 **public** key and the id it verifies under, owned (**M6 hand 2**).
///
/// # Why this exists beside [`VerifyingKeyRef`]
///
/// AC-057 puts a receipt in an environment with no network and no gx server, and asks a verifier to
/// check its signature there. A signature cannot be checked without the public key, so the key has
/// to travel — and 44 §1.2 already fixes the document it travels as, because `gx key gen` prints
/// "`{ "key_id": KEY_ID, "public_key": <base64> }`" (the secret key goes to a file/OS keystore, never
/// printed to stdout). That object *is* the public key document, and this is the type it decodes to.
///
/// A borrow cannot be the answer: `VerifyingKeyRef` borrows a `VerifyingKey` somebody else owns,
/// and a CLI that read a key out of a file has nobody to borrow from. The alternative was for
/// gx-cli to construct `ed25519_dalek::VerifyingKey` itself, which would make "what a public key
/// is" a question with two answers in this workspace — the shape E-M2-12 moved the `Proof` family
/// down to gx-core to avoid.
///
/// There is deliberately **no `Serialize`**: the wire form is 44 §1.2's two-field object, that
/// object is the CLI's output contract rather than this crate's, and a second serialisation here
/// would be a second spelling of one document.
#[derive(Clone, Debug)]
pub struct PublicKey {
    key_id: KeyId,
    key: VerifyingKey,
}

impl PublicKey {
    /// The length of an Ed25519 public key, in bytes.
    pub const LENGTH: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;

    /// Rebuild a public key from the id and bytes 44 §1.2's `gx key gen` printed.
    ///
    /// # Errors
    /// [`Error::KeyFormat`] if the bytes are not [`PublicKey::LENGTH`] long, or are that long and
    /// are not a point on the curve. Both are refusals rather than a repair: a key that does not
    /// decode cannot verify anything, and accepting it would move the failure to a place where it
    /// reads as "the receipt is bad".
    pub fn from_bytes(key_id: impl Into<KeyId>, bytes: &[u8]) -> Result<Self> {
        let raw: [u8; Self::LENGTH] = bytes.try_into().map_err(|_| Error::KeyFormat {
            detail: format!(
                "the public key is {} bytes; Ed25519 keys are {}",
                bytes.len(),
                Self::LENGTH
            ),
        })?;
        let key = VerifyingKey::from_bytes(&raw).map_err(|e| Error::KeyFormat {
            detail: format!("the bytes are not an Ed25519 public key: {e}"),
        })?;
        Ok(Self {
            key_id: key_id.into(),
            key,
        })
    }

    /// The id this key verifies under.
    #[must_use]
    pub fn key_id(&self) -> &KeyId {
        &self.key_id
    }

    /// The bytes 44 §1.2's `public_key` field carries.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        self.key.to_bytes()
    }

    /// The borrowed form [`crate::receipt::verify_offline`] takes.
    #[must_use]
    pub fn verifying(&self) -> VerifyingKeyRef<'_> {
        VerifyingKeyRef {
            key_id: &self.key_id,
            key: &self.key,
        }
    }
}

// ---------------------------------------------------------------------------
// Revocation (**FR-M7-3**, M7 hand 2)
// ---------------------------------------------------------------------------

/// A statement that a key is no longer to be trusted, from the moment it names (**FR-M7-3**).
///
/// # What canon fixes, and what it leaves to a setting
///
/// 45 §2's TH-5 names the control — "key rotation (a generation-numbered key id). **After
/// revocation, a receipt issued before it has its validity judged by "the key's state at the moment
/// of issue" and is not retroactively invalidated (the default)**. Consulting the revocation list is
/// at the verifier's discretion" — and 45 §5's ASM-45-2 states it again as an assumption with that DEFAULT. What 45 §3 then
/// records is that the **range** is open: "the revocation's retroactive range is undefined", medium, resolved in v0.2 with
/// ASM-4's TSA. So the range is a policy setting ([`Retroaction`]) and what a machine checks is
/// consistency after the setting, which is exactly what req/98 §3-2's AC asks for.
///
/// M5H8-3 (`req/38_ERRATA_2026-08-07.md` §45) read TH-5's mitigation column as a **roadmap entry**
/// because this module had no rotation at all. That erratum stands as the record of what was true
/// between M5 and here; what this hand changes is the code, not the reading of 45.
///
/// # 🔴 Who may sign one
///
/// The key it revokes, and nobody else ([`RevocationEntry::from_signed`]). v0.1 has no authority
/// above an actor's key — 45 §1 keeps the engine's signing key distinct from the adjudicator's and
/// names no root over either — so the only signature a verifier can check a revocation against is
/// the revoked key's own. Two consequences, and both are properties rather than bugs:
///
/// * a revocation cannot be forged without the very secret it is about, so the worst an attacker
///   with the secret can do with this record is deny themselves;
/// * a key whose secret was **lost** cannot be revoked at all. An operator-signed revocation needs a
///   trust root, which is a design this milestone does not have; `req/100` §5 routes it.
///
/// # 🔴 What a revocation does **not** do
///
/// It does not reach the tile log. 42 §3.11's `LedgerLeaf` is `{transformation, receipt_digest,
/// index}` and making it an enum is a change to a canonical type across three crates — the argument
/// req/98 §3-3 makes against option B for FR-M04, one requirement along. ASM-45-2 puts the list on the
/// verifier's side ("consulting the revocation list is at the verifier's discretion"), which is where this one is, and
/// anchoring revocations in the log is raised rather than assumed (`req/100` §5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevocationEntry {
    /// The key this statement is about. Checked against the signature's key id at
    /// [`RevocationEntry::from_signed`].
    pub key_id: KeyId,
    /// Why, in the operator's words. Carried because a list a verifier consults is also a document
    /// a person reads: "compromised" and "scheduled rotation" lead to different questions.
    pub reason: String,
    /// The moment the revocation takes effect, in 42's `Timestamp` (nanoseconds since the epoch).
    ///
    /// Chosen by whoever signs, which is the same limit `issued_at` has and for the same missing
    /// piece (ASM-4's TSA is v0.2). Under [`Retroaction::All`] the value does not matter; under the
    /// default it is the boundary, and it is the signer's own statement about their own key.
    pub revoked_at: Timestamp,
    /// The key that took over, when this is a rotation rather than a plain revocation.
    ///
    /// A reader of the list can follow the chain without a second document. `None` for a key that
    /// was retired with no successor, which is a different fact and is spelled as one.
    pub superseded_by: Option<KeyId>,
}

impl RevocationEntry {
    /// A revocation of `key_id`, effective at `revoked_at`, with no successor.
    #[must_use]
    pub fn new(key_id: impl Into<KeyId>, revoked_at: Timestamp, reason: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
            reason: reason.into(),
            revoked_at,
            superseded_by: None,
        }
    }

    /// The same entry, naming the key that took over.
    #[must_use]
    pub fn superseded_by(mut self, key_id: impl Into<KeyId>) -> Self {
        self.superseded_by = Some(key_id.into());
        self
    }

    /// Sign this entry with the key it revokes.
    ///
    /// The envelope is the same shape a receipt travels in — canonical DAG-CBOR under a typed PAE —
    /// so a verifier reads one with the code it already has, and 41 §6's "every canonical encode
    /// goes through gx-canon alone" holds here as everywhere.
    ///
    /// # Errors
    /// [`Error::Canon`] if the entry has no canonical form (two strings, an integer and an option
    /// always do), and [`Error::Schema`] if `key` is not the key this entry names — a producer that
    /// signed somebody else's revocation would be minting a record no verifier can accept, and
    /// learning that at the moment of signing is better than learning it from a stranger.
    pub fn signed_by(&self, key: &KeyPair) -> Result<DsseEnvelope> {
        if self.key_id != *key.key_id() {
            return Err(Error::Schema {
                detail: format!(
                    "this entry revokes {:?} and would be signed by {:?}; a revocation is signed by \
                     the key it revokes (v0.1 has no authority above an actor's key)",
                    self.key_id,
                    key.key_id()
                ),
            });
        }
        let mut envelope = DsseEnvelope {
            payload_type: REVOCATION_PAYLOAD_TYPE.to_string(),
            payload: cbor::encode(self)?,
            signatures: Vec::new(),
        };
        envelope.sign(key.signing_key(), key.key_id());
        Ok(envelope)
    }

    /// The entry an envelope carries, after checking that the key it names is the key that signed it.
    ///
    /// # Errors
    /// [`Error::Schema`] if the payload type is not [`REVOCATION_PAYLOAD_TYPE`] or the entry names a
    /// key other than `key`, [`Error::Canon`] if the payload is not canonical DAG-CBOR of an entry,
    /// [`Error::SignatureInvalid`] if `key` did not sign these bytes.
    ///
    /// The order matters: the type and the name are read from bytes nobody has vouched for yet, and
    /// the signature is what makes them believable. Both refusals are kept distinct because "this is
    /// about another key" and "this key did not sign it" lead an operator to different places.
    pub fn from_signed(envelope: &DsseEnvelope, key: &VerifyingKeyRef<'_>) -> Result<Self> {
        if envelope.payload_type != REVOCATION_PAYLOAD_TYPE {
            return Err(Error::Schema {
                detail: format!(
                    "the envelope carries {:?}; a revocation is {REVOCATION_PAYLOAD_TYPE:?}",
                    envelope.payload_type
                ),
            });
        }
        let entry: Self = cbor::decode(&envelope.payload)?;
        if entry.key_id != key.key_id {
            return Err(Error::Schema {
                detail: format!(
                    "the entry revokes {:?} and was offered against {:?}: a revocation is signed by \
                     the key it revokes",
                    entry.key_id, key.key_id
                ),
            });
        }
        envelope.verify(key)?;
        Ok(entry)
    }
}

/// How far back a revocation reaches — **the policy setting** req/98 §3-2 keeps out of the machine.
///
/// > "a receipt signed by a revoked key is judged invalid on a verify after the revocation time
/// > (**the retroactive range is a policy setting, and the machine checks only consistency after the
/// > setting**)"
///
/// Two values and no third, because 45 gives exactly two positions: ASM-45-2's DEFAULT, and the one
/// a compromise forces. A range expressed as a duration ("up to 30 minutes before the revoke") would be a third
/// setting nobody has ruled and whose boundary rests on the same unsigned clock the default already
/// rests on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Retroaction {
    /// ASM-45-2's DEFAULT: "on revocation, a receipt issued before it is not retroactively invalidated
    /// (validity is judged by "the key's state at the moment of issue")". A receipt dated before the revocation stays valid.
    ///
    /// 🔴 It rests on `Receipt.issued_at`, which **E-M2-6** keeps outside the signed core, so a
    /// holder of the compromised secret can re-date a receipt and pass. 45 §3 grades exactly that
    /// residual (TH-5: "in v0.1, without TSA integration, third-party proof of the revocation time is weak") and
    /// `crates/gx-witness/tests/revocation.rs::the_default_setting_cannot_see_a_backdated_receipt`
    /// is the measurement of it.
    #[default]
    FromRevocation,
    /// Every receipt the key ever signed is invalid.
    ///
    /// The setting a compromise is answered with, and the one that reads no clock: it is the only
    /// position that is not weakened by an unsigned timestamp.
    All,
}

impl Retroaction {
    /// The two settings, for a caller that offers them (a CLI flag, a table in a test).
    pub const ALL: [Self; 2] = [Self::FromRevocation, Self::All];

    /// The word this setting is spelled with on a command line and in an answer.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FromRevocation => "from-revocation",
            Self::All => "all",
        }
    }
}

/// The revocations a verifier has authenticated, and is willing to apply.
///
/// Built rather than read: every entry in it has been checked against the key it names, so a
/// consumer cannot accidentally apply a statement nobody signed. That is the difference between this
/// type and the file it comes from.
#[derive(Clone, Debug, Default)]
pub struct RevocationLedger {
    entries: Vec<RevocationEntry>,
}

impl RevocationLedger {
    /// A verifier that has consulted nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Read a list of signed entries, keeping the ones this key can authenticate.
    ///
    /// The count answered beside the ledger is how many entries were **ignored**: a verifier holds
    /// one public key and a shared list names many, so an entry about another key is not this
    /// verifier's to check. Ignoring is not silence — the caller is handed the number and
    /// `gx receipt verify` prints it.
    ///
    /// # Errors
    /// [`Error::SignatureInvalid`] if an entry **about this key** was not signed by it, and
    /// [`Error::Canon`] for one that does not decode. Fail-closed on the half that matters: an
    /// unauthenticated statement about the key under verification is either a forgery or a corrupt
    /// file, and applying it would let anyone deny anyone (while ignoring it would let a real
    /// revocation be hidden by breaking its signature).
    pub fn from_signed(
        envelopes: &[DsseEnvelope],
        key: &VerifyingKeyRef<'_>,
    ) -> Result<(Self, usize)> {
        let mut entries = Vec::new();
        let mut ignored = 0usize;
        for envelope in envelopes {
            // Read before believing: the entry names its subject in bytes nobody has vouched for,
            // and that name is what decides whether this verifier is the one who can vouch for them.
            let named = match cbor::decode::<RevocationEntry>(&envelope.payload) {
                Ok(entry) => entry.key_id,
                Err(_) => {
                    ignored += 1;
                    continue;
                }
            };
            if named != key.key_id {
                ignored += 1;
                continue;
            }
            entries.push(RevocationEntry::from_signed(envelope, key)?);
        }
        Ok((Self { entries }, ignored))
    }

    /// Every entry this ledger holds, in the order they were read.
    #[must_use]
    pub fn entries(&self) -> &[RevocationEntry] {
        &self.entries
    }

    /// How many entries it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The revocation of `key_id` this verifier applies: the **earliest** one.
    ///
    /// Revocation is monotone — a statement cannot un-revoke a key — so a second entry can only
    /// move the boundary, and taking the latest would let a key holder who kept signing after a
    /// compromise push it forward. The earliest is the safe reading and it is the one that makes two
    /// verifiers holding the same list answer alike.
    #[must_use]
    pub fn revocation_of(&self, key_id: &str) -> Option<&RevocationEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.key_id == key_id)
            .min_by_key(|entry| entry.revoked_at.0)
    }
}

// ---------------------------------------------------------------------------
// DR-46-40 -- claim-standing retraction windows (`req/730`, generalizing this
// module's own `RevocationEntry`/`RevocationLedger` pattern from one subject
// (a key) to any witnessed claim's standing).
// ---------------------------------------------------------------------------

/// A signed statement that a claim's standing has moved to closed (`req/730` §0's formal
/// invariant, §6 Option B).
///
/// # The one structural difference from [`RevocationEntry`], and why
///
/// A revocation is self-signed: the key that is revoked is the key that signs the statement
/// (`RevocationEntry::signed_by`'s own check enforces this). A claim is not a keypair, so a
/// close-statement cannot be self-signed the same way — it is signed by whichever authority the
/// verifier already trusts for that claim, supplied at verification time
/// ([`StandingEntry::from_signed`]'s `authority` argument), never inferred from the claim's own
/// identity. This is the AC-4 shape `req/730` §3 asks for: fail-closed on an unauthenticated
/// close-statement, where "authenticated" means "signed by the authority the verifier already
/// decided to trust", not "signed by the subject".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandingEntry {
    /// The claim this statement is about. Opaque and caller-defined (`req/730` §1a generalizes
    /// the invariant to "any witnessed claim", so this type does not couple to one particular
    /// claim-identifier type the way [`RevocationEntry::key_id`] couples to [`KeyId`]).
    pub claim_id: String,
    /// Why, in the closing authority's own words -- the same reason `RevocationEntry::reason`
    /// exists: a list a verifier consults is also a document a person reads.
    pub reason: String,
    /// The instant the claim's standing became closed, in 42's `Timestamp`. `req/730` §0's
    /// invariant: once set for a given `closed_at(c)`, no operation in this type's API may
    /// produce a state where it is unset or moved later than the earliest signed value seen
    /// (`req/730` §3 AC-1's binding condition; enforced by never offering a mutation at all --
    /// see the module doc comment above and `tests/dr4640_standing_windows.rs`'s structural
    /// probe).
    pub closed_at: Timestamp,
    /// The claim that corrects this one, when this closing is a correction rather than a plain
    /// close -- the `req/730` §4 falsifier's escape hatch, the same shape
    /// [`RevocationEntry::superseded_by`] offers for key rotation. `None` for a claim closed with
    /// no correction.
    pub superseded_by: Option<String>,
}

impl StandingEntry {
    /// A close of `claim_id`, effective at `closed_at`, with no correcting claim.
    #[must_use]
    pub fn new(
        claim_id: impl Into<String>,
        closed_at: Timestamp,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            claim_id: claim_id.into(),
            reason: reason.into(),
            closed_at,
            superseded_by: None,
        }
    }

    /// The same entry, naming the claim that corrects it.
    #[must_use]
    pub fn superseded_by(mut self, claim_id: impl Into<String>) -> Self {
        self.superseded_by = Some(claim_id.into());
        self
    }

    /// Sign this entry with the authority closing the claim.
    ///
    /// Unlike [`RevocationEntry::signed_by`], there is no self-referential check here: a claim
    /// carries no key of its own to check the signer against (`req/730` §1's own point -- this is
    /// a generalization to *any* witnessed claim, and most claims are not keys). Who counts as an
    /// authorized closer for a given `claim_id` is a policy this type does not encode; it is the
    /// verifier's, supplied at [`StandingEntry::from_signed`].
    ///
    /// # Errors
    /// [`Error::Canon`] if the entry has no canonical form (two strings, an integer and an option
    /// always do).
    pub fn signed_by(&self, key: &KeyPair) -> Result<DsseEnvelope> {
        let mut envelope = DsseEnvelope {
            payload_type: STANDING_PAYLOAD_TYPE.to_string(),
            payload: cbor::encode(self)?,
            signatures: Vec::new(),
        };
        envelope.sign(key.signing_key(), key.key_id());
        Ok(envelope)
    }

    /// The entry an envelope carries, after checking it was signed by `authority` -- the AC-4
    /// fail-closed check (`req/730` §3): a close-statement not signed by the authority the
    /// verifier trusts for this claim is rejected outright (an `Err`, never a silently-ignored
    /// entry and never a silently-applied one).
    ///
    /// # Errors
    /// [`Error::Schema`] if the payload type is not [`STANDING_PAYLOAD_TYPE`],
    /// [`Error::Canon`] if the payload is not canonical DAG-CBOR of an entry,
    /// [`Error::SignatureInvalid`] if `authority` did not sign these bytes.
    pub fn from_signed(envelope: &DsseEnvelope, authority: &VerifyingKeyRef<'_>) -> Result<Self> {
        if envelope.payload_type != STANDING_PAYLOAD_TYPE {
            return Err(Error::Schema {
                detail: format!(
                    "the envelope carries {:?}; a claim-standing close is {STANDING_PAYLOAD_TYPE:?}",
                    envelope.payload_type
                ),
            });
        }
        let entry: Self = cbor::decode(&envelope.payload)?;
        envelope.verify(authority)?;
        Ok(entry)
    }
}

/// Whether a claim is in force, as of some instant -- `req/730` §0's two-state answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// No authenticated close-statement governs the instant queried.
    Open,
    /// An authenticated close-statement's `closed_at` is at or before the instant queried.
    Closed,
}

/// The claim-standing close-statements a verifier has authenticated against one trusted
/// authority, and is willing to apply -- the generalization of [`RevocationLedger`] one type over.
#[derive(Clone, Debug, Default)]
pub struct StandingLedger {
    entries: Vec<StandingEntry>,
}

impl StandingLedger {
    /// A verifier that has consulted nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Read a list of signed close-statements against one trusted `authority`.
    ///
    /// # Errors
    /// [`Error::SignatureInvalid`] or [`Error::Schema`] or [`Error::Canon`] on the **first**
    /// envelope that does not authenticate against `authority` -- fail-closed, per AC-4: unlike
    /// [`RevocationLedger::from_signed`] (which silently ignores an entry that is honestly about a
    /// *different* key, because a shared list naming many keys is expected), every entry handed to
    /// this ledger is expected to be about a claim this same `authority` governs, so one that does
    /// not authenticate is treated as a forgery or a corrupt file, not a stranger's business.
    pub fn from_signed(
        envelopes: &[DsseEnvelope],
        authority: &VerifyingKeyRef<'_>,
    ) -> Result<Self> {
        let entries = envelopes
            .iter()
            .map(|envelope| StandingEntry::from_signed(envelope, authority))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { entries })
    }

    /// Every entry this ledger holds, in the order they were read.
    #[must_use]
    pub fn entries(&self) -> &[StandingEntry] {
        &self.entries
    }

    /// How many entries it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The close of `claim_id` this verifier applies: the **earliest** one (AC-2) --
    /// [`RevocationLedger::revocation_of`]'s `min_by_key` reading, for the identical reason: a
    /// later-signing authority cannot push the boundary forward.
    #[must_use]
    pub fn close_of(&self, claim_id: &str) -> Option<&StandingEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.claim_id == claim_id)
            .min_by_key(|entry| entry.closed_at.0)
    }

    /// The standing of `claim_id` **as of** `at` (AC-3): `Closed` only when a close exists whose
    /// `closed_at <= at`; `Open` otherwise, including for every instant before the close. A query
    /// for a past instant is therefore unaffected by a close that happens later -- the same
    /// distinction [`Retroaction`] already draws for revocation, generalized to any claim.
    #[must_use]
    pub fn standing_at(&self, claim_id: &str, at: Timestamp) -> Standing {
        match self.close_of(claim_id) {
            Some(entry) if entry.closed_at.0 <= at.0 => Standing::Closed,
            _ => Standing::Open,
        }
    }

    /// The standing of `claim_id` **as of now** -- the common case, `standing_at` at the largest
    /// representable instant so any authenticated close (whatever its timestamp) governs.
    #[must_use]
    pub fn standing_now(&self, claim_id: &str) -> Standing {
        self.standing_at(claim_id, Timestamp(i64::MAX))
    }
}

/// Refuse a key file anyone but its owner can read.
///
/// A secret whose file is `0o644` has already leaked to every process on the machine, and reading
/// it silently would make the leak invisible. Owner-execute and the sticky bits are not checked --
/// they say nothing about who can read the bytes.
///
/// Not enforced on Windows: the ACL model has no `0o077` to compare against, and a check that
/// returned `Ok` there while pretending to be the same check would be the fail-open req/29 §4
/// forbids. It is a declared gap (req/54 §5), not a silent one.
#[cfg(unix)]
fn check_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::metadata(path).map_err(|e| Error::io("stat the key", path, &e))?;
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(Error::KeyPermissions {
            path: path.to_path_buf(),
            mode,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// The record a key file holds.
///
/// Fields in encoded-key order (length first, then bytes), the convention `map_key_order.rs`
/// measured to be a convention and not a check.
///
/// `secret_key` is a `Vec<u8>` carrying exactly [`SECRET_KEY_LENGTH`] bytes rather than a
/// `[u8; 32]`: the length is validated on the way in with a message that names the real length, and
/// a fixed array would fail inside serde with "invalid length" and no mention of keys at all.
#[derive(Serialize, Deserialize)]
struct StoredKey {
    /// [`KEY_ALGORITHM`]. Present so a file from a version that adds a second one is refused rather
    /// than misread.
    algorithm: String,
    key_id: KeyId,
    /// The Ed25519 seed (RFC 8032 §5.1.5), as a byte string on the wire.
    #[serde(with = "crate::dsse::raw_bytes")]
    secret_key: Vec<u8>,
}

/// 🔴 **P2 item2** (`req/130` §1) — the record an **encrypted** key file holds.
///
/// A distinct struct rather than an optional field on [`StoredKey`]: a plain key file has to keep
/// decoding against exactly the shape it was written with (AC-P2-3's backward-compatibility half),
/// and a `StoredKey` that grew an optional encryption field would still decode an old file today but
/// would make every future plaintext writer responsible for remembering to leave the new field
/// `None` -- one more thing to get right at the write site nobody asked to change.
///
/// declaration order is not load-bearing (`map_key_order.rs`'s finding: `serde_ipld_dagcbor` sorts a
/// struct's keys itself), and is written in encoded order (length, then bytes) as the convention
/// `StoredKey` documents above.
#[derive(Serialize, Deserialize)]
struct EncryptedStoredKey {
    key_id: KeyId,
    /// [`ENCRYPTED_KEY_FORMAT`]. Present so a file from a future format (a different KDF or cipher)
    /// is refused rather than misread.
    format: String,
    #[serde(with = "crate::dsse::raw_bytes")]
    nonce: Vec<u8>,
    algorithm: String,
    #[serde(with = "crate::dsse::raw_bytes")]
    kdf_salt: Vec<u8>,
    kdf_t_cost: u32,
    kdf_p_cost: u32,
    #[serde(with = "crate::dsse::raw_bytes")]
    ciphertext: Vec<u8>,
    kdf_m_cost_kib: u32,
}

impl Error {
    /// One place that turns a `std::io::Error` into this crate's, so every I/O refusal names what
    /// was being attempted. `gx_log::Error::Io` carries the same four fields for the same reason.
    pub(crate) fn io(action: &'static str, path: &Path, e: &std::io::Error) -> Self {
        Error::Io {
            action,
            path: PathBuf::from(path),
            kind: e.kind(),
            detail: e.to_string(),
        }
    }
}
