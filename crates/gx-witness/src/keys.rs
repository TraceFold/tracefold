//! Ed25519 key pairs, and the file one is kept in.
//!
//! Spec: 32 FR-020 for the requirement, 34 AC-020 for how it is judged, 45 ASM-9 for the storage
//! scope, 42 §3.2 for `KeyId`.
//!
//! # Everything in this file is derived, and that is the ticket
//!
//! 42 §0's gx-witness row names four modules and none of them is this one; 41 §2 lists
//! `src/{lib,provenance,receipt,evidence,dsse,keys}.rs` and gives `keys.rs` no types at all. So
//! there is no field table to transcribe: the key record's shape, the file's layout, and the
//! permission rule below are **derived** from FR-020's one sentence -- 「鍵生成とローカル鍵ストレージ
//! （v0.1範囲: ファイルベース保存）」 -- in the standing E-M2-17 gave `CheckerResultRef` and req/52
//! §1.1 gave `LedgerStore`. req/49 §3 M2-14 raised it and no ruling has landed; req/54 §4 raises it
//! again with an implementation attached, which is a better thing to rule on than a proposal.
//!
//! # FR-020 says CLI and AC-020 says otherwise, in the spec's own words
//!
//! FR-020 逐語 ends 「往復が **CLI経由で** テストできる」 while AC-020 reads itself down to 「M2時点
//! ではCLI結線前のためライブラリレベルで検証する」 and hands the CLI round trip to AC-054/AC-057 in
//! M6. This hand implements the library API. The erratum is req/49 §3 M2-14's and is not closed
//! here.
//!
//! # What this module does not do
//!
//! No rotation, no revocation list, no passphrase, no hardware token, no second tier. 45 ASM-45-1
//! keeps v0.1 to 「単一 tier（署名済みのみ）」 and ASM-45-2 puts revocation out of scope, so a key
//! file that leaks is contained by the filesystem and by nothing else. That is a real property of
//! v0.1 and it is written here rather than left for a reader to infer from the absence of code.
//!
//! 🔴 **Superseded in part by M7 hand 2 (FR-M7-3), and kept rather than rewritten (no-delete).**
//! The paragraph above records what was true from M5 to here and why. What has changed since:
//!
//! * **rotation and a revocation list exist** — [`RevocationEntry`], [`RevocationLedger`],
//!   [`Retroaction`], and `gx key rotate|revoke` above them. 裁定 #6 (req/98 §3-2) is the ruling;
//!   45 §2's TH-5 mitigation column had named the control all along and M5H8-3 read it as a roadmap
//!   entry precisely because no code answered to it.
//! * **still absent**: passphrase, hardware token, second tier. A key file that leaks is still
//!   contained by the filesystem and by nothing else *until somebody revokes it*, and a revocation
//!   is a statement to verifiers rather than a lock on the file.
//! * 🔴 **a citation in the paragraph above is wrong and is corrected here rather than edited into
//!   silence**: 45 §5's ASM-45-1 is about **evidence collectors** (「Evidence collectorの信頼はv0.1で
//!   は単一tier（署名済みのみ採用）」), not about keys, and ASM-45-2 does not put revocation out of
//!   scope — it fixes its **default**: 「鍵失効時、失効前に発行済みのreceiptは遡及無効化しない…
//!   revocation list参照はverifier側任意とする」. The sentence 「単一 tier」 belongs to the evidence
//!   row. `req/100` §5 raises the mis-citation; the reading that matters for this module is the
//!   primary text, quoted on [`RevocationEntry`].
//!
//! # 🔴 45 TH-5's mitigation column, and how it is to be read (**M5H8-3**)
//!
//! 45 §2's TH-5 row lists 「鍵ローテーション（世代付き鍵ID）」 among the controls, in the present
//! tense, next to the paragraph above that says there is none. req/86 §5.2 raised the contradiction
//! and `req/38_ERRATA_2026-08-07.md` §45 ruled it, verbatim:
//!
//! > **M5H8-3 採(a)**: 45 TH-5 緩和欄は「鍵ローテーションは v0.2(ASM-45-2)」と読む erratum。
//! > §3 残存表(「中」)が既に正しい。
//!
//! So the row is a **roadmap entry read as a control**, and the erratum is that the rotation it
//! names belongs to v0.2 under ASM-45-2. 45 §3's residual register already grades TH-5 「中」 and
//! is right as written; 45 itself is not edited (`req/spec/` is 1 byte unchanged), because the
//! erratum ledger is req/38 and a spec that is quietly corrected is a spec nobody can date. What
//! v0.1 has against a compromised key is what this module implements and no more.

use core::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use ed25519_dalek::{SigningKey, VerifyingKey, SECRET_KEY_LENGTH};
use gx_canon::cbor;
use gx_core::{KeyId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::dsse::{DsseEnvelope, REVOCATION_PAYLOAD_TYPE};
use crate::{Error, Result};

/// The algorithm string written into a key file.
///
/// One value today. It is written down rather than assumed so that a file produced by a future
/// version with a second algorithm is *refused* by this one instead of read as Ed25519 -- the same
/// argument `gx_canon::cid::DIGEST_ALGORITHM` makes for naming the hash.
pub const KEY_ALGORITHM: &str = "ed25519";

/// A key pair and the id it signs under.
///
/// The `KeyId` travels with the key because a signature is only checkable against the id it was
/// offered under (42 §3.2: 「DSSE `keyid`と同一名前空間」), and a caller holding a bare `SigningKey`
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
    pub key_id: &'a str,
    pub key: &'a VerifyingKey,
}

impl KeyPair {
    /// A fresh key pair, seeded from the operating system (FR-020).
    ///
    /// # Why the entropy is fetched here and injected nowhere
    ///
    /// 41 §6 injects randomness at the engine boundary 「決定的リプレイのため」, and that rule is
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

    /// The half that verifies, paired with its id (AC-020's 「ロードした鍵で検証関数を呼ぶ」).
    #[must_use]
    pub fn verifying(&self) -> VerifyingKeyRef<'_> {
        VerifyingKeyRef {
            key_id: &self.key_id,
            key: &self.verifying,
        }
    }

    /// Write the key pair to `path` (FR-020: 「v0.1範囲: ファイルベース保存」).
    ///
    /// # The file
    ///
    /// Canonical DAG-CBOR of [`StoredKey`], through gx-canon -- 41 §6's 「全 canonical encode は
    /// gx-canon 経由のみ」 holds here as everywhere, so this crate names no codec. Nothing hashes a
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
        let path = path.as_ref();
        let record = StoredKey {
            algorithm: KEY_ALGORITHM.to_string(),
            key_id: self.key_id.clone(),
            secret_key: self.signing.to_bytes().to_vec(),
        };
        let bytes = cbor::encode(&record)?;

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
        file.write_all(&bytes)
            .map_err(|e| Error::io("write the key", path, &e))?;
        file.sync_all()
            .map_err(|e| Error::io("fsync the key", path, &e))?;
        Ok(())
    }

    /// Read a key pair back (AC-020).
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be read, [`Error::Canon`] if it is not canonical DAG-CBOR
    /// of a [`StoredKey`], [`Error::KeyFormat`] for a record that decodes but is not an Ed25519 key
    /// of the right length, [`Error::KeyPermissions`] on unix if anyone but the owner can read it.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        check_permissions(path)?;
        let bytes = fs::read(path).map_err(|e| Error::io("read the key", path, &e))?;
        let record: StoredKey = cbor::decode(&bytes)?;

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

/// An Ed25519 **public** key and the id it verifies under, owned (**M6 hand 2**).
///
/// # Why this exists beside [`VerifyingKeyRef`]
///
/// AC-057 puts a receipt in an environment with no network and no gx server, and asks a verifier to
/// check its signature there. A signature cannot be checked without the public key, so the key has
/// to travel — and 44 §1.2 already fixes the document it travels as, because `gx key gen` prints
/// 「`{ "key_id": KEY_ID, "public_key": <base64> }`（秘密鍵はファイル/OSキーストアへ、標準出力に
/// 出さない）」. That object *is* the public key document, and this is the type it decodes to.
///
/// A borrow cannot be the answer: `VerifyingKeyRef` borrows a `VerifyingKey` somebody else owns,
/// and a CLI that read a key out of a file has nobody to borrow from. The alternative was for
/// gx-cli to construct `ed25519_dalek::VerifyingKey` itself, which would make 「what a public key
/// is」 a question with two answers in this workspace — the shape E-M2-12 moved the `Proof` family
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
    /// reads as 「the receipt is bad」.
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
/// 45 §2's TH-5 names the control — 「鍵ローテーション（世代付き鍵ID）。**失効後、失効前発行receiptの
/// 有効性は「発行時点の鍵状態」で判定し遡及無効化しない（既定）**。revocation listはverifier側で任意
/// 参照」 — and 45 §5's ASM-45-2 states it again as an assumption with that DEFAULT. What 45 §3 then
/// records is that the **range** is open: 「鍵失効の遡及範囲が未定義」, 中, resolved in v0.2 with
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
/// req/98 §3-3 makes against 案 B for FR-M04, one requirement along. ASM-45-2 puts the list on the
/// verifier's side (「revocation list参照はverifier側任意」), which is where this one is, and
/// anchoring revocations in the log is raised rather than assumed (`req/100` §5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevocationEntry {
    /// The key this statement is about. Checked against the signature's key id at
    /// [`RevocationEntry::from_signed`].
    pub key_id: KeyId,
    /// Why, in the operator's words. Carried because a list a verifier consults is also a document
    /// a person reads: 「compromised」 and 「scheduled rotation」 lead to different questions.
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
    /// so a verifier reads one with the code it already has, and 41 §6's 「全 canonical encode は
    /// gx-canon 経由のみ」 holds here as everywhere.
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
    /// the signature is what makes them believable. Both refusals are kept distinct because 「this is
    /// about another key」 and 「this key did not sign it」 lead an operator to different places.
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
/// > 「失効済み鍵で署名された receipt が、失効時刻より後の verify で無効と判定される(**遡及範囲は政策
/// > 設定であり、設定後の一貫性のみを機械が検査する**)」
///
/// Two values and no third, because 45 gives exactly two positions: ASM-45-2's DEFAULT, and the one
/// a compromise forces. A range expressed as a duration (「revoke より 30 分前まで」) would be a third
/// setting nobody has ruled and whose boundary rests on the same unsigned clock the default already
/// rests on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Retroaction {
    /// ASM-45-2's DEFAULT: 「失効時、失効前に発行済みのreceiptは遡及無効化しない（「発行時点の鍵状態」
    /// で有効性判定）」. A receipt dated before the revocation stays valid.
    ///
    /// 🔴 It rests on `Receipt.issued_at`, which **E-M2-6** keeps outside the signed core, so a
    /// holder of the compromised secret can re-date a receipt and pass. 45 §3 grades exactly that
    /// residual (TH-5: 「TSA連携なしのv0.1では失効時刻の第三者証明が弱い」) and
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
/// a fixed array would fail inside serde with 「invalid length」 and no mention of keys at all.
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
