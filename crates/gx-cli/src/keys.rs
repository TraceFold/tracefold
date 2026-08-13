//! `gx key gen` / `gx key list` — 44 §1.2, req/56 §3, and **M6-29**.
//!
//! # 🔴 The secret never reaches stdout, and that is a check rather than a habit
//!
//! 44 §1.2: 「`gen`: `Actor`署名鍵を生成（ed25519-dalek, 41既定）。stdout: `{ "key_id": KEY_ID,
//! "public_key": <base64> }`（秘密鍵はファイル/OSキーストアへ、**標準出力に出さない**）」. req/88 §4
//! M6-29 turned the last clause into this hand's DoD: 「`gx key gen --json` の出力に秘密鍵 byte が
//! 現れない probe」. `crates/gx-cli/tests/key_surface.rs` generates from a **known seed**, so it can
//! look for the actual thirty-two bytes rather than for the word 「secret」 — a probe that grepped
//! for a field name would pass on a leak with a different label.
//!
//! # Where the keys are
//!
//! req/56 §3: 「秘密鍵=`~/.gx/keys/`(user home・0600)。project 側は**公開 keyid の参照のみ**」. The
//! project's `.gx/` therefore holds no key at all, and [`KeyStore`] is rooted at the user's home
//! rather than at [`crate::layout::Layout`] — the one store in this binary that is not under the
//! project. `KeyPair::save` sets `0o600` at creation on unix (a `create` then `chmod` leaves a
//! window) and `KeyPair::load` refuses a file group or other can read.
//!
//! # 🔴 Key ids are derived, and there is no `--key-id`
//!
//! 44 §1.2's synopsis is `gx key gen [--alg ed25519] [--out <PATH>] [--json]` — no name. So the id
//! is derived from the public key ([`derive_key_id`]) and the file is named after it. Two
//! consequences worth stating: a key file's name is a claim its contents can be checked against,
//! and no operator-supplied string ever reaches a path, so this store has no filename escaping
//! problem to get wrong.

use std::path::{Path, PathBuf};

use gx_core::Timestamp;
use gx_witness::dsse::DsseEnvelope;
use gx_witness::keys::RevocationEntry;
use gx_witness::{KeyPair, PublicKey};

use crate::exit::Outcome;
use crate::layout::Layout;
use crate::{io, Error, Result};

/// The only algorithm 41 names, and the only value `--alg` takes.
pub const ALGORITHM: &str = gx_witness::keys::KEY_ALGORITHM;

/// The id a public key is filed and signed under.
///
/// `ed25519-` and the first eight bytes of the public key, in hex. Not a digest: 則 1 (i) keeps this
/// crate away from `gx_canon`, and a key id is not a gx identity — 42 §1.2's `gx1:` namespace is for
/// content addresses and a key id belongs to 42 §3.2's 「DSSE `keyid`と同一名前空間」, which is a
/// string. Hex of the key's own bytes is a **rendering** of the key, so two different keys cannot
/// share an id unless they share sixty-four bits of public key, and the value is reproducible by
/// anybody holding the key rather than by anybody holding this binary.
#[must_use]
pub fn derive_key_id(public: &[u8]) -> String {
    let mut out = String::from("ed25519-");
    for byte in public.iter().take(8) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// `~/.gx/keys/`, as a store.
#[derive(Debug, Clone)]
pub struct KeyStore {
    dir: PathBuf,
}

/// What `gx key list` says about one key file.
#[derive(Debug, Clone)]
pub struct KeyEntry {
    /// The id, taken from the file name.
    pub key_id: String,
    /// Where the file is.
    pub path: PathBuf,
    /// 🔴 unix only: whether group or other can read it. `None` off unix, where there is no
    /// `0o077` to compare against — gx-witness declares that gap (req/54 §5) and a `true` here
    /// would launder it into a guarantee.
    pub permissions_ok: Option<bool>,
}

impl KeyStore {
    /// The store at an explicit directory.
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// req/56 §3's `~/.gx/keys/`.
    ///
    /// # Errors
    /// [`Error::Usage`] if the environment names no home directory. A refusal rather than a
    /// fallback to the working directory: writing a secret into whatever directory the operator
    /// happened to be in is the failure req/56 §1's 「秘密は project に置かない」 forbids.
    pub fn user_default() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| Error::Usage {
                detail: "neither HOME nor USERPROFILE is set, so req/56 §3's `~/.gx/keys/` has no \
                         address; pass --out to name the file"
                    .to_string(),
            })?;
        Ok(Self::at(PathBuf::from(home).join(".gx").join("keys")))
    }

    /// The directory itself.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where a key of this id is filed.
    #[must_use]
    pub fn path_of(&self, key_id: &str) -> PathBuf {
        self.dir.join(format!("{key_id}.key"))
    }

    /// 🔴 Where this store keeps the revocations it has issued (**FR-M7-3**).
    ///
    /// Beside the keys and **not** under a project's `.gx/`: req/56 §3 puts key material in the
    /// user's home and §2's table of `.gx/` paths is a declared list that `m6_surface_doubt.rs`
    /// compares against req/56 itself, so a ninth row would be a change to a document this hand does
    /// not write. The file is also not a secret — every entry in it is a signed public statement —
    /// which is what makes `--out` (an export a verifier is handed) a copy rather than a leak.
    ///
    /// `.json` rather than `.key`, so [`KeyStore::list`] walks past it: that function answers with
    /// key ids taken from file names and a revocation list is not a key.
    #[must_use]
    pub fn revocations_path(&self) -> PathBuf {
        self.dir.join("revocations.json")
    }

    /// Load the key pair filed under `key_id`.
    ///
    /// # Errors
    /// [`Error::NotFound`] if there is no such file, [`Error::Witness`] if it is not a key this
    /// version reads or if anyone but its owner can read it.
    pub fn load(&self, key_id: &str) -> Result<KeyPair> {
        let path = self.path_of(key_id);
        if !path.is_file() {
            return Err(Error::NotFound {
                what: "key",
                id: key_id.to_string(),
            });
        }
        Ok(KeyPair::load(&path)?)
    }

    /// Every key file in the store, by id.
    ///
    /// The ids come from **file names** and no file is opened. Two reasons: listing keys should not
    /// require the authority to read them, and a store holding one unreadable key should still be
    /// listable — an operator debugging a permissions problem is exactly who runs this.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory exists and cannot be listed. A directory that is **not there**
    /// is an empty store rather than a failure: req/56 §3's path is created by `gen`, and 「you have
    /// no keys yet」 is not an error condition.
    pub fn list(&self) -> Result<Vec<KeyEntry>> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io("list", &self.dir)(e)),
        };
        let mut out = Vec::new();
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "key") {
                continue;
            }
            let Some(key_id) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            out.push(KeyEntry {
                key_id,
                permissions_ok: permissions_ok(&path),
                path,
            });
        }
        out.sort_by(|a, b| a.key_id.cmp(&b.key_id));
        Ok(out)
    }
}

/// unix: whether the mode denies group and other. Off unix: `None`, see [`KeyEntry`].
#[cfg(unix)]
fn permissions_ok(path: &Path) -> Option<bool> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
    Some(mode & 0o077 == 0)
}

#[cfg(not(unix))]
fn permissions_ok(_path: &Path) -> Option<bool> {
    None
}

/// 🔴 **M6H2-10** — whether a key file this binary just wrote is one it will be able to read.
///
/// `KeyPair::save` asks for `0o600` at creation and `KeyPair::load` refuses anything group or other
/// can read. On a filesystem with no unix permission model — drvfs and 9p, which is where this
/// repository's own working tree sits — the request is silently ignored and every file reads `0777`,
/// so `gx key gen --out <there>` produces a key `gx receipt verify` will not load.
///
/// The refusal is correct: the secret really is world-readable. What was wrong was its timing, one
/// command later and phrased as a permissions error about a file the operator did not know they had
/// made. So the same judgement is taken here, at the moment the file is written.
///
/// `None` off unix and for a file nobody else can read. A warning rather than a refusal: an operator
/// exporting a key to a shared volume on purpose is a real case, and 44 §1.2 gives `gen` no exit
/// code for 「wrote it, but somewhere unsafe」.
#[must_use]
pub fn permission_warning(path: &Path) -> Option<String> {
    match permissions_ok(path) {
        Some(false) => Some(format!(
            "{} is readable by more than its owner: this filesystem does not honour the 0600              req/56 §3 asks for, and `gx` will refuse to load this key (M6H2-10)",
            path.display()
        )),
        _ => None,
    }
}

/// 🔴 `gx key gen [--alg ed25519] [--out <PATH>]` (44 §1.2, FR-020).
///
/// The seed is the operating system's ([`KeyPair::generate`]), the id is derived from the public
/// key it produced, and the pair is re-seated under that id so that the file name, the `key_id`
/// inside the file, and the `keyid` of every signature it will make are one value.
///
/// # Errors
/// [`Error::Usage`] for an algorithm this version does not write, [`Error::Witness`] if the
/// operating system supplies no entropy or the file cannot be written, [`Error::Io`] if the
/// directory cannot be created.
pub fn gen(store: &KeyStore, alg: &str, out: Option<&Path>) -> Result<Outcome> {
    gen_recording(store, alg, out, None)
}

/// 🔴 `gx key gen --record` (**FR-M7-4**): the same generation, with the id written where a server
/// reads it.
///
/// The two fields 44 §1.2 fixes on stdout are unchanged — a third one would break
/// `gx key gen --json > key.pub.json` — so where the id was recorded goes to **stderr**, next to
/// where the secret was filed and for the same reason.
///
/// # Errors
/// Everything [`gen`] refuses, plus [`Error::Io`] if `.gx/config.toml` cannot be written.
pub fn gen_recording(
    store: &KeyStore,
    alg: &str,
    out: Option<&Path>,
    record_into: Option<&Layout>,
) -> Result<Outcome> {
    let outcome = generate(store, alg, out)?;
    if let Some(layout) = record_into {
        let key_id = outcome.json["key_id"].as_str().unwrap_or_default();
        let path = crate::serve::record_signing_keyid(layout, key_id)?;
        eprintln!(
            "gx: recorded engine_signing_keyid = {key_id:?} in {} (FR-M7-4)",
            path.display()
        );
    }
    Ok(outcome)
}

fn generate(store: &KeyStore, alg: &str, out: Option<&Path>) -> Result<Outcome> {
    if alg != ALGORITHM {
        return Err(Error::Usage {
            detail: format!(
                "--alg takes {ALGORITHM:?} (41's default and the only algorithm gx-witness writes); \
                 got {alg:?}"
            ),
        });
    }
    // Generated once, then re-seated: `KeyPair::generate` needs an id before there is a public key
    // to derive one from, and `SigningKey::to_bytes` gives back the very seed it was built with, so
    // the second pair is the same key rather than a second one.
    let drawn = KeyPair::generate(String::new())?;
    let seed = drawn.signing_key().to_bytes();
    let key_id = derive_key_id(&drawn.public().to_bytes());
    let pair = KeyPair::from_seed(key_id.clone(), &seed);

    let path = match out {
        Some(p) => p.to_path_buf(),
        None => {
            std::fs::create_dir_all(store.dir()).map_err(io("create", store.dir()))?;
            store.path_of(&key_id)
        }
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(io("create", parent))?;
        }
    }
    pair.save(&path)?;

    // 🔴 The two fields 44 §1.2 names, and no third. `path` is deliberately **not** here: it would
    // be the location of a secret, printed by the command whose contract is that the secret does not
    // reach stdout. It goes to stderr as a note in `main`, where a redirect to a file does not
    // capture it.
    Ok(Outcome::ok(serde_json::json!({
        "key_id": pair.key_id(),
        "public_key": gx_core::b64::encode(&pair.public().to_bytes()),
    })))
}

/// `gx key list` (44 §1.2: 「ローカル既知鍵ID一覧」).
///
/// # 🔴 The permission warning
///
/// `KeyPair::load` refuses a world-readable key, but a refusal at use time tells an operator only
/// about the key they reached for. req/88 §4 M6-29 採(a): 「**0600 でない file を見つけたら警告**=
/// witness の `KeyPermissions` error の CLI 版」. So the listing carries the mode judgement per key,
/// and the exit status stays 0 — a warning that failed the command would make `list` unusable for
/// the exact operator who needs it.
///
/// # Errors
/// [`Error::Io`] if the directory exists and cannot be listed.
pub fn list(store: &KeyStore) -> Result<Outcome> {
    let entries = store.list()?;
    let insecure = entries
        .iter()
        .filter(|e| e.permissions_ok == Some(false))
        .count();
    Ok(Outcome::ok(serde_json::json!({
        "dir": store.dir().display().to_string(),
        "keys": entries
            .iter()
            .map(|e| serde_json::json!({
                "key_id": e.key_id,
                "permissions_ok": e.permissions_ok,
            }))
            .collect::<Vec<_>>(),
        "count": entries.len(),
        "insecure": insecure,
    })))
}

// ---------------------------------------------------------------------------
// `gx key revoke` / `gx key rotate` (**FR-M7-3**, 裁定 #6)
// ---------------------------------------------------------------------------

/// The revocations a store holds, as the signed envelopes a verifier is handed.
///
/// A file that is not there is an empty list rather than a failure — [`KeyStore::list`]'s reasoning
/// about a store with no keys yet, one file along.
///
/// # Errors
/// [`Error::Io`] if the file exists and cannot be read, [`Error::Malformed`] if it is not a list of
/// DSSE envelopes. Malformed is a refusal and not an empty list: a revocation list that cannot be
/// read is exactly the file an attacker would corrupt, and answering 「no revocations」 about it is
/// the fail-open req/29 §4 forbids.
pub fn read_revocations(path: &Path) -> Result<Vec<DsseEnvelope>> {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io("read", path)(e)),
    };
    serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
        what: "revocation list",
        path: path.display().to_string(),
        detail: detail.to_string(),
    })
}

/// Append one signed revocation to a list, keeping everything already in it.
///
/// Append-only in the shape a JSON array allows: the file is read, the entry is pushed, the whole
/// array is written back. That is not the tile log's append-only (nothing here is anchored), and the
/// difference is stated rather than implied — what a verifier gets from this file is a set of signed
/// statements, each of which stands or falls on its own signature, and the file itself vouches for
/// nothing.
fn append_revocation(path: &Path, envelope: DsseEnvelope) -> Result<usize> {
    let mut entries = read_revocations(path)?;
    entries.push(envelope);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io("create", parent))?;
    }
    let text = serde_json::to_string_pretty(&entries).map_err(|detail| Error::Malformed {
        what: "revocation list",
        path: path.display().to_string(),
        detail: detail.to_string(),
    })?;
    std::fs::write(path, format!("{text}\n")).map_err(io("write", path))?;
    Ok(entries.len())
}

/// 🔴 `gx key revoke --key-id <ID> [--reason <TEXT>] [--out <PATH>]` (**FR-M7-3**).
///
/// # This verb is not in 44 §1.2, and that is a ruling rather than an oversight
///
/// 44 §1.2's key section is `gen` and `list`. 裁定 #6 (req/98 §3-2) put rotation and revocation in
/// M7 — 「U-06/13 鍵ローテ=M7 採」 — and 45 §2's TH-5 has named 「鍵ローテーション（世代付き鍵ID）」 as a
/// control since v0.1. **M6-24 採(b)** is the precedent for the shape: `gx log checkpoint` is a verb
/// 44 §1.1 does not list, added because a ruling required the capability.
///
/// # The signature, and why the store has to hold the secret
///
/// A revocation is signed by the key it revokes (`gx_witness::keys::RevocationEntry`), so this
/// command loads the **secret** and the key file has to still be there. A key already deleted cannot
/// be revoked, which is the same limit as a key whose secret was lost and is why `rotate` keeps the
/// predecessor's file rather than removing it.
///
/// # Errors
/// [`Error::NotFound`] if the store holds no such key, [`Error::Witness`] if the file will not load
/// or the entry will not sign, [`Error::Io`]/[`Error::Malformed`] for the list file.
pub fn revoke(
    store: &KeyStore,
    key_id: &str,
    reason: &str,
    at: Timestamp,
    superseded_by: Option<&str>,
    out: Option<&Path>,
) -> Result<Outcome> {
    let pair = store.load(key_id)?;
    let mut entry = RevocationEntry::new(key_id.to_string(), at, reason);
    if let Some(successor) = superseded_by {
        entry = entry.superseded_by(successor.to_string());
    }
    let envelope = entry.signed_by(&pair)?;

    let path = out.map_or_else(|| store.revocations_path(), Path::to_path_buf);
    let entries = append_revocation(&path, envelope)?;

    Ok(Outcome::ok(serde_json::json!({
        "key_id": key_id,
        "revoked_at": at.0,
        "reason": reason,
        "superseded_by": superseded_by,
        "revocations": path.display().to_string().replace('\\', "/"),
        "entries": entries,
    })))
}

/// 🔴 `gx key rotate --key-id <ID> [--record]` (**FR-M7-3**): the successor and the revocation, in
/// one command.
///
/// Two commands would leave a window in which half a rotation has happened, and the half an operator
/// skips is the revocation — which is the half that does anything. The order is generate first: a
/// revocation naming a successor that does not exist would be a statement about nothing, and a
/// failure between the two steps leaves an unused new key rather than a revoked key with no
/// replacement.
///
/// 🔴 **The old secret is kept.** Receipts it signed still have to verify — that is the whole point
/// of `Retroaction::FromRevocation` — and deleting the file would make a revoked key
/// indistinguishable from a lost one at the only place that can tell them apart.
///
/// # Errors
/// Everything [`gen`] and [`revoke`] refuse.
pub fn rotate(
    store: &KeyStore,
    alg: &str,
    key_id: &str,
    reason: &str,
    at: Timestamp,
    record_into: Option<&Layout>,
) -> Result<Outcome> {
    // Fail before generating if the predecessor is not one this store can speak for: a rotation that
    // left a new key behind and could not revoke the old one is the half-done state above.
    let _ = store.load(key_id)?;

    let generated = gen_recording(store, alg, None, record_into)?;
    let successor = generated.json["key_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let revoked = revoke(store, key_id, reason, at, Some(&successor), None)?;

    Ok(Outcome::ok(serde_json::json!({
        "key_id": successor,
        "public_key": generated.json["public_key"],
        "revoked": key_id,
        "revoked_at": revoked.json["revoked_at"],
        "superseded_by": successor,
        "reason": reason,
        "revocations": revoked.json["revocations"],
        "entries": revoked.json["entries"],
    })))
}

/// Read the `{ "key_id", "public_key" }` document `gx key gen` prints, or a `gx` key file.
///
/// Both, in that order, because they are the two things an operator plausibly points `--key` at and
/// telling them apart costs one parse. A file that is neither is refused with the reason, rather
/// than with 「not a key」 — an operator who passed the receipt by mistake should be told that.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read; [`Error::Usage`] if it is neither document;
/// [`Error::Witness`] if it is a public-key document whose bytes are not an Ed25519 key.
pub fn read_public(path: &Path) -> Result<PublicKey> {
    let raw = std::fs::read(path).map_err(io("read", path))?;
    if let Ok(doc) = serde_json::from_slice::<PublicKeyDoc>(&raw) {
        let bytes = gx_core::b64::decode(&doc.public_key).map_err(|detail| Error::Usage {
            detail: format!("{}: `public_key` is not base64 ({detail})", path.display()),
        })?;
        return Ok(PublicKey::from_bytes(doc.key_id, &bytes)?);
    }
    match KeyPair::load(path) {
        Ok(pair) => Ok(pair.public()),
        Err(e) => Err(Error::Usage {
            detail: format!(
                "{} is neither a `{{key_id, public_key}}` document (44 §1.2's `gx key gen` output) \
                 nor a gx key file: {e}",
                path.display()
            ),
        }),
    }
}

/// 44 §1.2's `gx key gen` stdout, as a type to read it back with.
#[derive(serde::Deserialize)]
struct PublicKeyDoc {
    key_id: String,
    public_key: String,
}
