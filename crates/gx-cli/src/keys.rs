// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx key gen` / `gx key list` — 44 §1.2, req/56 §3, and **M6-29**.
//!
//! # 🔴 The secret never reaches stdout, and that is a check rather than a habit
//!
//! 44 §1.2: "`gen`: generates an `Actor` signing key (ed25519-dalek, 41's default). stdout:
//! `{ "key_id": KEY_ID, "public_key": <base64> }` (the secret key goes to a file/OS keystore,
//! **never to stdout**)" (sem: SEM-gx-cli-040). req/88 §4
//! M6-29 turned the last clause into this hand's DoD: "a probe that checks `gx key gen --json`'s
//! output contains no secret-key byte". `crates/gx-cli/tests/key_surface.rs` generates from a **known seed**, so it can
//! look for the actual thirty-two bytes rather than for the word "secret" — a probe that grepped
//! for a field name would pass on a leak with a different label.
//!
//! # Where the keys are
//!
//! req/56 §3: "the secret key lives at `~/.gx/keys/` (user home, 0600); the project side holds
//! **only a reference to the public keyid**" (sem: SEM-gx-cli-041). The
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
/// `ed25519-` and the first eight bytes of the public key, in hex. Not a digest: Rule 1 (i) keeps this (sem: SEM-gx-cli-042)
/// crate away from `gx_canon`, and a key id is not a gx identity — 42 §1.2's `gx1:` namespace is for
/// content addresses and a key id belongs to 42 §3.2's "the same namespace as DSSE's `keyid`" (sem: SEM-gx-cli-043), which is a
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
    /// 🔴 **R6 / `req/229` M-06** — the id the key inside the file derives, or `None` if the file
    /// could not be opened or read as a plaintext key.
    ///
    /// `None` covers three cases on purpose and the listing does not pretend to tell them apart:
    /// an unreadable file (the permissions case this verb exists for), an encrypted key (`gen
    /// --passphrase-file`, whose id cannot be derived without the passphrase), and a file that is
    /// not a key at all. What matters for M-06 is the fourth case: a value that is present and
    /// **different** from [`KeyEntry::key_id`].
    pub key_id_inside: Option<String>,
    /// 🔴 **R15 / `req/259` H-01** — the public half, base64, or `None` for the same three cases
    /// [`KeyEntry::key_id_inside`] covers.
    ///
    /// `gx key gen` puts `key_id` and `public_key` on stdout and puts the secret's location on
    /// stderr. The fifteenth audit measured a run whose stdout never arrived — exit 101 from a
    /// panicking `eprintln!`, the secret on the disk, and **no string anywhere naming the key**.
    /// The panic is closed in `emit`; this closes the other half, which is that the two public
    /// fields existed only on a stream. They do not: the file holds the seed, so both are
    /// derivable, and this verb already opens the file to answer `key_id_inside`. Nothing new is
    /// exposed — a public key is the half that is published.
    pub public_key: Option<String>,
}

impl KeyEntry {
    /// 🔴 **R6 / `req/229` M-06** — whether the name on the file and the key inside it agree.
    ///
    /// `true` when the id could not be read: an unreadable file is not a mismatch, and calling it
    /// one would make every encrypted key look tampered with.
    #[must_use]
    pub fn named_correctly(&self) -> bool {
        self.key_id_inside
            .as_ref()
            .is_none_or(|inside| inside == &self.key_id)
    }
}

/// The id the key inside a file derives, without asserting anything about whether it should be
/// there.
///
/// Plaintext keys only: an encrypted key needs its passphrase and this verb has none, so it answers
/// `None` rather than prompting or failing. Permission refusals also land here as `None`, which is
/// what keeps [`KeyStore::list`]'s promise that listing does not need the authority to read.
fn read_key_id(path: &Path) -> Option<String> {
    KeyPair::load(path)
        .ok()
        .map(|pair| pair.key_id().to_string())
}

/// 🔴 **R15 / `req/259` H-01** — the public half of the key inside a file, base64.
///
/// The same read as [`read_key_id`] and the same three `None`s (unreadable, encrypted, not a key).
/// Separate rather than folded into one call because these are two facts and `KeyEntry` reports
/// them as two.
fn read_public_key(path: &Path) -> Option<String> {
    KeyPair::load(path)
        .ok()
        .map(|pair| gx_core::b64::encode(&pair.public().to_bytes()))
}

/// 🔴 **R7 / `req/232` H-01** — the key store, in the shape a door asks it for a public key.
///
/// `gx_engine::HeadKeys` is asked one question — *do you hold the key this document names?* — and
/// the answer is a key or nothing. **Nothing is never a failure here**: an encrypted key, a key
/// this operator does not have, a directory that is not readable and a deployment with no store at
/// all are all "this environment cannot check the head", which is reported as
/// `head_authenticity: "unverified"` and is not a pass. The audit's finding was exactly that a
/// missing check was being reported as a passed one.
#[derive(Debug)]
pub struct StoreHeadKeys {
    store: KeyStore,
}

impl StoreHeadKeys {
    /// The verifier over a key store.
    #[must_use]
    pub fn new(store: KeyStore) -> Self {
        Self { store }
    }
}

impl gx_engine::HeadKeys for StoreHeadKeys {
    fn verifying(&self, key_id: &str) -> Option<gx_witness::PublicKey> {
        self.store.load(key_id).ok().map(|pair| pair.public())
    }
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
    /// happened to be in is the failure req/56 §1's "do not put secrets in the project" (sem: SEM-gx-cli-044) forbids.
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
    /// # 🔴 **R5 / `req/227` M-06** — the file name and the key inside it have to agree
    ///
    /// This function opened `<key_id>.key` and answered with whatever key was inside, without ever
    /// comparing the two. `req/227` M-06 put an actor key's bytes into the engine key's file and
    /// watched `gx undo` refuse with `… does not verify under the key it names,
    /// "ed25519-833a84909f1f9dfe"` — while the receipt in question named `ed25519-ad0f…`, the id
    /// this store had been asked for. The sentence was false about the document it was about, and
    /// the operator was sent to look for tampering in the wrong place. `gx key list` reads file
    /// names alone (deliberately: listing must not need the authority to read), so it answered
    /// `permissions_ok: true` for both and nothing anywhere said the two disagreed.
    ///
    /// A store is a map from an id to a key, and a map whose key and value disagree is not a map.
    /// So the mismatch is its own refusal, and it names **both** ids: the one that was asked for,
    /// which is the one every receipt and every config file spells, and the one that is actually in
    /// the file.
    ///
    /// # Errors
    /// [`Error::NotFound`] if there is no such file, [`Error::Witness`] if it is not a key this
    /// version reads or if anyone but its owner can read it, [`Error::Malformed`] if the file's
    /// name and the key's own id disagree.
    ///
    /// 🔴 **R41 / `req/561`** — supplement to the first sentence above: "there is no such file" is
    /// established by [`crate::layout::presence_of`] answering `Absent` (the operating system said
    /// `NotFound`), and by nothing else. A path whose `stat` fails for any other reason, or that
    /// holds something other than a regular file, is **not** answered as absence: it falls through
    /// to [`KeyPair::load`], whose own `# Errors` words above already cover it. Before R41 this
    /// door spelled the question `!path.is_file()`, which folds every `stat` failure into "no such
    /// file" — `req/559` measured that fold turning a transient `stat` failure under parallel load
    /// into `NOT_FOUND` about a key that was sitting on disk.
    pub fn load(&self, key_id: &str) -> Result<KeyPair> {
        let path = self.path_of(key_id);
        if crate::layout::presence_of(&path).is_absent() {
            return Err(Error::NotFound {
                what: "key",
                id: key_id.to_string(),
            });
        }
        let pair = KeyPair::load(&path)?;
        self.named_as(key_id, pair, &path)
    }

    /// 🔴 **R5 / `req/227` M-06** — the file-name-versus-contents check, for both loaders.
    ///
    /// # Errors
    /// [`Error::Malformed`] naming both ids when they disagree.
    fn named_as(&self, key_id: &str, pair: KeyPair, path: &Path) -> Result<KeyPair> {
        if pair.key_id() != key_id {
            return Err(Error::Malformed {
                what: "key file",
                path: path.display().to_string(),
                detail: format!(
                    "req/56 §3's key store files a key under its own id, and this file is named \
                     for {key_id:?} while the key inside it is {:?}. Whichever of the two a \
                     receipt names, one of them is not here — so the store answers neither \
                     (req/227 M-06). `gx key list` shows the file names; the id printed here is \
                     the key's own",
                    pair.key_id()
                ),
            });
        }
        Ok(pair)
    }

    /// 🔴 **P2 item2** (`req/130` §1) — load a key [`gen`] wrote **encrypted**
    /// (`--passphrase-file`), under its passphrase.
    ///
    /// # Errors
    /// [`Error::NotFound`] if there is no such file, [`Error::Witness`] wrapping
    /// [`gx_witness::Error::WrongPassphrase`] if the passphrase is wrong or the file is corrupted,
    /// [`gx_witness::Error::KeyFormat`] if the file is not encrypted at all (`load` above is the
    /// road for that one).
    ///
    /// 🔴 **R41 / `req/561`** — the same supplement as [`KeyStore::load`]'s: only
    /// [`crate::layout::presence_of`]'s `Absent` answers `Error::NotFound`; every other `stat`
    /// outcome falls through to [`KeyPair::load_encrypted`] and wears that road's own words.
    pub fn load_encrypted(&self, key_id: &str, passphrase: &str) -> Result<KeyPair> {
        let path = self.path_of(key_id);
        if crate::layout::presence_of(&path).is_absent() {
            return Err(Error::NotFound {
                what: "key",
                id: key_id.to_string(),
            });
        }
        let pair = KeyPair::load_encrypted(&path, passphrase)?;
        // 🔴 **R5 / `req/227` M-06** — the same check on the encrypted road: the passphrase proves
        // who may open the file and says nothing about which key is inside it.
        self.named_as(key_id, pair, &path)
    }

    /// Every key file in the store, by id.
    ///
    /// The ids come from **file names** and no file is opened. Two reasons: listing keys should not
    /// require the authority to read them, and a store holding one unreadable key should still be
    /// listable — an operator debugging a permissions problem is exactly who runs this.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory exists and cannot be listed. A directory that is **not there**
    /// is an empty store rather than a failure: req/56 §3's path is created by `gen`, and "you have
    /// no keys yet" is not an error condition. (sem: SEM-gx-cli-045)
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
            // 🔴 **R6 / `req/229` M-06** — the id inside the file, beside the id on the file.
            //
            // `req/227` M-06 was closed for `KeyStore::load`, which refuses a file whose contents
            // are a different key and names both ids. `req/229` measured the half that was left:
            // this function reads **file names only**, so a store where key `A`'s file holds key
            // `B`'s bytes answered `{"key_id":"…de56e8db…","permissions_ok":true}` for a key that
            // does not exist anywhere — while `gx serve --signing-key A` refused. `gx key list` is
            // the one verb an operator runs to see what they have.
            //
            // The reason the ids came from names is kept and is still good: "listing keys should
            // not require the authority to read them". So a file that will not open is not an
            // error and not a silence — `key_id_inside` is `null`, `readable` is `false`, and the
            // permissions judgement is unchanged. Only a file that **does** open and disagrees is
            // called out, and it is called out as a fact rather than as a refusal.
            let inside = read_key_id(&path);
            // 🔴 **R15 / `req/259` H-01** — read from the same file, in the same breath.
            let public_key = read_public_key(&path);
            out.push(KeyEntry {
                key_id,
                permissions_ok: permissions_ok(&path),
                key_id_inside: inside,
                public_key,
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
/// code for "wrote it, but somewhere unsafe" (sem: SEM-gx-cli-046).
#[must_use]
pub fn permission_warning(path: &Path) -> Option<String> {
    match permissions_ok(path) {
        Some(false) => Some(format!(
            "{} is readable by more than its owner: this filesystem does not honour the 0600 req/56 §3 asks for, and `gx` will refuse to load this key (M6H2-10)",
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
    gen_recording(store, alg, out, None, None)
}

/// 🔴 `gx key gen --record` (**FR-M7-4**): the same generation, with the id written where a server
/// reads it.
///
/// The two fields 44 §1.2 fixes on stdout are unchanged — a third one would break
/// `gx key gen --json > key.pub.json` — so where the id was recorded goes to **stderr**, next to
/// where the secret was filed and for the same reason.
///
/// # 🔴 `passphrase` — **P2 item2** (`req/130` §1)
///
/// `Some` writes the secret encrypted ([`KeyPair::save_encrypted`]) instead of plaintext-0600
/// ([`KeyPair::save`]); `None` is unchanged from before P2. Opt-in, per ruling 2 (sem: SEM-gx-cli-047): the caller (`gx key
/// gen --passphrase-file <PATH>`, main.rs) decides, and a caller that decides nothing gets exactly
/// today's behaviour.
///
/// # Errors
/// Everything [`gen`] refuses, plus [`Error::Io`] if `.gx/config.toml` cannot be written.
pub fn gen_recording(
    store: &KeyStore,
    alg: &str,
    out: Option<&Path>,
    record_into: Option<&Layout>,
    passphrase: Option<&str>,
) -> Result<Outcome> {
    let outcome = generate(store, alg, out, passphrase)?;
    if let Some(layout) = record_into {
        let key_id = outcome.json["key_id"].as_str().unwrap_or_default();
        let path = crate::serve::record_signing_keyid(layout, key_id)?;
        crate::note!(
            "gx: recorded engine_signing_keyid = {key_id:?} in {} (FR-M7-4)",
            path.display()
        );
    }
    Ok(outcome)
}

fn generate(
    store: &KeyStore,
    alg: &str,
    out: Option<&Path>,
    passphrase: Option<&str>,
) -> Result<Outcome> {
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
    match passphrase {
        Some(pass) => pair.save_encrypted(&path, pass)?,
        None => pair.save(&path)?,
    }

    // 🔴 The two fields 44 §1.2 names, and no third. `path` is deliberately **not** here: it would
    // be the location of a secret, printed by the command whose contract is that the secret does not
    // reach stdout. It goes to stderr as a note in `main`, where a redirect to a file does not
    // capture it. Whether the write was encrypted is not a third field either, for the same reason —
    // the operator who passed `--passphrase-file` already knows.
    Ok(Outcome::ok(serde_json::json!({
        "key_id": pair.key_id(),
        "public_key": gx_core::b64::encode(&pair.public().to_bytes()),
    })))
}

/// 🔴 **P2 item2** (`req/130` §1) — read a passphrase from a **file**, the shape 44 §2.5's bearer
/// token takes (`--token-file`, `crates/gx-cli/src/serve.rs`) and for the identical reason: the
/// **path** is the argument and the passphrase is not, so `ps` shows where the secret lives and not
/// what it is.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Usage`] if it holds nothing (an empty
/// passphrase silently accepted would be a plaintext key wearing an encrypted one's shape —
/// [`gx_witness::KeyPair::save_encrypted`] also refuses it, and the message is given here first,
/// where the operator can still choose a different file).
pub fn read_passphrase(path: &Path) -> Result<String> {
    let passphrase = std::fs::read_to_string(path)
        .map_err(io("read", path))?
        .trim()
        .to_string();
    if passphrase.is_empty() {
        return Err(Error::Usage {
            detail: format!(
                "{} holds no passphrase; an empty passphrase would make `gen --passphrase-file` a \
                 plaintext key wearing an encrypted one's shape",
                path.display()
            ),
        });
    }
    Ok(passphrase)
}

/// `gx key list` (44 §1.2: "list of locally known key IDs" (sem: SEM-gx-cli-048)).
///
/// # 🔴 The permission warning
///
/// `KeyPair::load` refuses a world-readable key, but a refusal at use time tells an operator only
/// about the key they reached for. req/88 §4 M6-29 adopted (a): "**warn when a file that is not
/// 0600 is found** -- the CLI's version of witness's `KeyPermissions` error" (sem: SEM-gx-cli-049). So the listing carries the mode judgement per key,
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
    // 🔴 **R6 / `req/229` M-06** — how many of these files are not the key they are named for.
    let misnamed = entries.iter().filter(|e| !e.named_correctly()).count();
    for entry in entries.iter().filter(|e| !e.named_correctly()) {
        crate::note!(
            "gx key list: {} is named for {:?} and the key inside it is {:?}. `gx` refuses to load \
             either id from this file (req/227 M-06); neither key is usable until the file is put \
             back under its own name (req/229 M-06)",
            entry.path.display(),
            entry.key_id,
            entry.key_id_inside.as_deref().unwrap_or("")
        );
    }
    Ok(Outcome::ok(serde_json::json!({
        "dir": store.dir().display().to_string(),
        "keys": entries
            .iter()
            .map(|e| serde_json::json!({
                "key_id": e.key_id,
                "permissions_ok": e.permissions_ok,
                // 🔴 **R6 / `req/229` M-06** — the two ids, separately, because they are two facts.
                // The audit's raw is this verb answering `permissions_ok: true` for a key id that
                // exists nowhere. 44 §1.2's "list of locally known key IDs" is unmoved: the file
                // name is still `key_id` and still first.
                "key_id_inside": e.key_id_inside,
                // 🔴 **R15 / `req/259` H-01** — the field `gx key gen` prints beside `key_id`, so
                // a key made by a run whose stdout never arrived can still be named in full.
                "public_key": e.public_key,
                "named_correctly": e.named_correctly(),
            }))
            .collect::<Vec<_>>(),
        "count": entries.len(),
        "insecure": insecure,
        "misnamed": misnamed,
    })))
}

// ---------------------------------------------------------------------------
// `gx key revoke` / `gx key rotate` (**FR-M7-3**, ruling #6) (sem: SEM-gx-cli-050)
// ---------------------------------------------------------------------------

/// The revocations a store holds, as the signed envelopes a verifier is handed.
///
/// A file that is not there is an empty list rather than a failure — [`KeyStore::list`]'s reasoning
/// about a store with no keys yet, one file along.
///
/// # Errors
/// [`Error::Io`] if the file exists and cannot be read, [`Error::Malformed`] if it is not a list of
/// DSSE envelopes. Malformed is a refusal and not an empty list: a revocation list that cannot be
/// read is exactly the file an attacker would corrupt, and answering "no revocations" (sem: SEM-gx-cli-051) about it is
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
/// 44 §1.2's key section is `gen` and `list`. Ruling #6 (req/98 §3-2) put rotation and revocation in
/// M7 — "U-06/13 key rotation = adopted for M7" (sem: SEM-gx-cli-052) — and 45 §2's TH-5 has named "key rotation
/// (generation-numbered key id)" as a
/// control since v0.1. **M6-24 adopted (b)** is the precedent for the shape: `gx log checkpoint` is a verb
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
/// # 🔴 The successor is written plaintext (**P2 item2 residual**)
///
/// `req/130` §1 item2 scopes encryption to `gen`/`load`; `rotate`'s successor has no
/// `--passphrase-file` of its own in this pass, named rather than silently dropped
/// (`req/131` §3).
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

    let generated = gen_recording(store, alg, None, record_into, None)?;
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
/// than with "not a key" (sem: SEM-gx-cli-053) — an operator who passed the receipt by mistake should be told that.
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
