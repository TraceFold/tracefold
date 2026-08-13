//! 🔴 `gx serve` (44 §1.1/§1.2) — the verb, its refusals, and the four things a server needs.
//!
//! # Which crate holds the binary, and why the answer is 「both, in this order」
//!
//! The hand-6 brief asks for the judgement in the report: 「serve の実行 binary がどちらの crate かは
//! 44 §1.1(gx serve は CLI verb)と 41 §2 の crate 表を直読して判定」. Read directly:
//!
//! * **44 §1.1** lists thirteen commands and the thirteenth row is 「`gx serve` | gx-api起動」 — so
//!   `serve` is a **verb of `gx`**, and `gx` is this crate's `[[bin]]`.
//! * **41 §2** describes `gx-api/` as 「axum HTTP+JSONL stream（44準拠）」 and puts the literal word
//!   `serve` at the end of **gx-cli**'s own line — so the runtime is gx-api's and the entry is here.
//! * **47 §1(a)** settles the direction: 「単一静的バイナリ(`gx-cli` が `gx serve` で `gx-api` 機能を
//!   内包)」.
//!
//! ∴ the executable is `gx`, built from gx-cli; the router, the runtime and the graceful shutdown
//! are `gx_api::serve`, and **gx-cli declares no `tokio`** (the brief: 「gx-cli に tokio は declare
//! しない」). `gx_api::serve` blocks and builds its own runtime for exactly that reason. The brief's
//! stop condition — 「もし設計上 gx-cli に serve を置く必要が出たら止まって起票」 — did not fire: no
//! part of the runtime needs to be spelled on this side.
//!
//! # What this module does that gx-api structurally cannot
//!
//! gx-api may not name `.gx/` (the cycle: 47 §1(a) makes gx-cli contain gx-api), so the four things a
//! server needs are assembled here:
//!
//! 1. **the engine** over `.gx/ledger/journal`, with the fs adapter registered and DR-2's two axes
//!    set from the flags 44 §1.2 gives this verb;
//! 2. **the bearer token**, read from a **file** — `--token-file`. Hand 5 closed two of the three
//!    roads (environment, `.gx/config.toml`) and left the third with a note: 「token **file の path**
//!    を引数にする形が手 6 が `gx serve` の flag を書く時に採るべき形」. A path in `ps` is not a
//!    secret; the token itself would be;
//! 3. **the two keys of 45 §1** — the server's own signing key and the adjudicators' — out of req/56
//!    §3's `~/.gx/keys/`, which is a directory gx-api has never heard of;
//! 4. **`.gx/config.toml`'s recorded public keyid** (**E-M6-7**). Hand 5 built the check and named
//!    this hand as the reader: 「config.toml の**読み手**は手 6(flag を持つ手)」.
//!
//! # 🔴 Three refusals, and none of them is a silent success
//!
//! * `--bind` outside loopback. v0.1 has **no authorization layer** (M5H6-4 採(a)), so a socket on a
//!   public interface is a socket anyone can `cancel` through. §47 registered the firing condition
//!   for the authorization work — M5H6-4(b), 「HTTP が loopback 以外に bind される時」 — and until
//!   that fires the honest answer is to refuse rather than to offer an override flag that turns a
//!   ruling into a checkbox. Raised as **M6H6-6** so that the reviewer can rule the override in.
//! * `--tls-cert` / `--tls-key`. 44 §1.2's synopsis has them and req/88 §1 N-09 keeps TLS out of
//!   v0.1 (44 §2.5: 「v0.2（予告）: mTLS」). E-M6-8 fixed the shape for a flag with nowhere to go:
//!   refuse, and say what the synopsis promised. An accepted `--tls-cert` would leave an operator
//!   believing the socket is encrypted, which is worse than the flag not existing.
//! * no token. 44 §2.5 is 「必須」 and hand 5 made a tokenless server answer `INTERNAL` rather than
//!   `401` to every request; refusing to start says the same thing earlier and to the person who can
//!   fix it.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gx_api::auth::Bearer;
use gx_api::state::{AppState, RequestEvidence, ServerKeys};
use gx_api::{ReceiptSlot, ServeConfig};
use gx_core::{EnforcementMode, FailPosture};
use gx_witness::KeyPair;

use crate::keys::KeyStore;
use crate::layout::Layout;
use crate::receipt::{ReceiptStore, StoredKind};
use crate::{Error, Result};

/// What `gx serve`'s flags amount to, after parsing and before anything is opened.
pub struct ServeSpec {
    /// `--bind <ADDR:PORT>`; [`gx_api::serve::DEFAULT_BIND`] when absent.
    pub bind: Option<String>,
    /// `--record-only` — DR-2's `EnforcementMode` axis, for the process.
    pub record_only: bool,
    /// `--fail-posture <closed|open>` — DR-2's other axis (ASM-13).
    pub fail_posture: Option<String>,
    /// 44 §1.2's TLS pair, accepted so that it can be refused by name (N-09).
    pub tls: (Option<PathBuf>, Option<PathBuf>),
    /// The file holding 44 §2.5's bearer token. Not in 44 §1.2's synopsis — see the module header.
    pub token_file: Option<PathBuf>,
    /// The key id this server signs receipts with. Falls back to `.gx/config.toml` (**E-M6-7**).
    pub signing_key: Option<String>,
}

/// 45 §1's two keys, over req/56 §3's directory.
///
/// Every key in the store is loaded **at start-up**, which is a decision with a cost: a key added
/// while the server runs is a key it will not find, and an operator who adds an adjudicator has to
/// restart. The alternative — reading the directory per request — would put a filesystem walk inside
/// 43 T-5's path and would make the set of people who may rule on an escalation change without any
/// record that it did.
struct StoreKeys {
    signing: KeyPair,
    rulers: BTreeMap<String, KeyPair>,
}

impl ServerKeys for StoreKeys {
    fn signing(&self) -> &KeyPair {
        &self.signing
    }

    fn ruler(&self, key_id: &str) -> Option<&KeyPair> {
        // 🔴 No fallback to `signing`, and **E-M6-15**/INV-S6 is why: 「裁かれる側が自分を承認する
        // 既定値は存在しない」. A server that signed a human ruling with its own key would be
        // recording that the server allowed the change.
        self.rulers.get(key_id)
    }
}

/// `.gx/receipts/` as gx-api's archive (M6H5-9's road back for a restarted server).
struct StoreArchive {
    store: ReceiptStore,
}

impl gx_api::ReceiptArchive for StoreArchive {
    fn store(
        &self,
        id: &gx_core::TransformationId,
        slot: ReceiptSlot,
        receipt: &gx_witness::Receipt,
    ) -> std::result::Result<(), String> {
        let kind = match slot {
            ReceiptSlot::Verdict => StoredKind::Verdict,
            ReceiptSlot::Ruling => StoredKind::Ruling,
            ReceiptSlot::Commit => StoredKind::Commit,
        };
        self.store
            .put(id, kind, receipt)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn load(&self, id: &gx_core::TransformationId) -> Option<gx_witness::Receipt> {
        self.store
            .first_available(id)
            .ok()
            .flatten()
            .map(|(_, receipt)| receipt)
    }
}

/// 🔴 **E-M6-7**'s reader — `.gx/config.toml`'s recorded public key id.
///
/// A four-line parser rather than a TOML crate. req/56 §2 gives this file one job on this path
/// (「公開 keyid の参照のみ」), the grammar of that job is `key = "value"`, and a dependency whose
/// generality is unused is a dependency whose failure modes are not. Raised as **M6H6-7** if a second
/// setting ever needs a type this cannot express.
///
/// # Errors
/// [`Error::Io`] if the file exists and cannot be read.
pub fn recorded_signing_keyid(layout: &Layout) -> Result<Option<String>> {
    let path = layout.join("config.toml");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(crate::io("read", &path))?;
    Ok(text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let (name, value) = line.split_once('=')?;
        if name.trim() != "engine_signing_keyid" {
            return None;
        }
        Some(value.trim().trim_matches('"').to_string())
    }))
}

/// 🔴 **FR-M7-4** — E-M6-7's **writer**, beside its reader.
///
/// req/95 §4 ③ (M6H7-8): 「`.gx/config.toml` の `engine_signing_keyid` に読み手はあり、書き手が無い。
/// `scratch` image には shell も無いので、fresh volume ではこの値を記録する道が無い」. The compose file
/// worked around it with `--signing-key ${GX_SIGNING_KEY_ID:?…}`, which is an operator copying a key
/// id by hand from one command's output into another's environment. `gx key gen --record` is the
/// road, and this is where it ends.
///
/// It lives here rather than in `keys.rs` because [`recorded_signing_keyid`] lives here: the file's
/// grammar is four lines of parser, and a second spelling of it — a writer that emitted what the
/// reader cannot read — is the failure a shared function makes impossible rather than unlikely.
///
/// # What it does to a file that already exists
///
/// Rewrites the `engine_signing_keyid` line and **leaves every other line alone**, including
/// comments. A rotation is the ordinary case (`gx key rotate --record`), and a writer that truncated
/// the file would take an operator's other settings with it the first time they rotated. A file that
/// does not exist yet is created with a comment naming what wrote it, because a mystery file in a
/// project directory is a thing people delete.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read or written.
pub fn record_signing_keyid(layout: &Layout, key_id: &str) -> Result<PathBuf> {
    let path = layout.join("config.toml");
    let line = format!("engine_signing_keyid = \"{key_id}\"");

    let text = if path.exists() {
        std::fs::read_to_string(&path).map_err(crate::io("read", &path))?
    } else {
        String::from(
            "# Written by `gx key gen --record` (FR-M7-4). req/56 §2: this file holds the public\n\
             # key id only -- 「公開 keyid の参照のみ」. Secrets live in ~/.gx/keys/ (req/56 §3).\n",
        )
    };

    let mut replaced = false;
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    for existing in text.lines() {
        let trimmed = existing.trim();
        let names_the_slot = !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .is_some_and(|(name, _)| name.trim() == "engine_signing_keyid");
        if names_the_slot && !replaced {
            out.push_str(&line);
            replaced = true;
        } else if names_the_slot {
            // A second assignment of the same key would make the reader's `find_map` answer with
            // the first one. Dropping the duplicate is the only outcome in which the file says one
            // thing; keeping it would leave a line that looks in force and is not.
            continue;
        } else {
            out.push_str(existing);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str(&line);
        out.push('\n');
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(crate::io("create", parent))?;
    }
    std::fs::write(&path, out).map_err(crate::io("write", &path))?;
    Ok(path)
}

/// Parse `--bind`, and refuse anything hand 5's policy refuses.
///
/// # Errors
/// [`Error::Usage`] for an unparseable address and for one [`gx_api::auth::bind_refusal`] rejects.
pub fn resolve_bind(bind: Option<&str>) -> Result<SocketAddr> {
    let text = bind.unwrap_or(gx_api::serve::DEFAULT_BIND);
    if let Some(reason) = gx_api::auth::bind_refusal(text) {
        return Err(Error::Usage {
            detail: format!(
                "{reason} v0.1 has no authorization layer (M5H6-4 採(a)), so a non-loopback bind is \
                 an unauthenticated surface on a shared network. §47 registered the authorization \
                 work against the firing condition 「HTTP が loopback 以外に bind される時」 — this \
                 refusal is what makes that condition observable instead of theoretical"
            ),
        });
    }
    text.parse().map_err(|e| Error::Usage {
        detail: format!("`{text}` is not an <ADDR:PORT> (44 §1.2's `gx serve --bind`): {e}"),
    })
}

/// Everything a running server needs, built from a project and a set of flags.
///
/// # Errors
/// [`Error::Usage`] for the three refusals in the module header, [`Error::NotFound`] for a signing
/// key the store does not hold, [`Error::Io`]/[`Error::Layout`] from `.gx/`, [`Error::Engine`] if the
/// journal will not open and [`Error::Gate`] if the shipped pack will not parse.
pub fn build(project: &Path, store: &KeyStore, spec: &ServeSpec) -> Result<(AppState, SocketAddr)> {
    if spec.tls.0.is_some() || spec.tls.1.is_some() {
        return Err(Error::Usage {
            detail: "--tls-cert/--tls-key are in 44 §1.2's synopsis and v0.1 serves plain HTTP: 44 \
                     §2.5 puts mTLS in 「v0.2（予告）」 and req/88 §1 N-09 keeps it out of this \
                     milestone. Accepting the flags and serving an unencrypted socket would tell an \
                     operator the connection is protected when it is not (M6H3-5's rule: a flag with \
                     nowhere to go is refused, never dropped). Terminate TLS in front of the \
                     loopback bind until v0.2"
                .to_string(),
        });
    }
    let bind = resolve_bind(spec.bind.as_deref())?;

    let token_file = spec.token_file.as_ref().ok_or_else(|| Error::Usage {
        detail: "--token-file <PATH> is required: 44 §2.5 makes `Authorization: Bearer <token>` \
                 mandatory on every endpoint except `/healthz`, and a server with no token would \
                 answer every request with an internal error rather than a 401 (hand 5's reading of \
                 M4H4-2: 「あなたの token が違う」 is not something to say to a caller when the \
                 server has no token at all). The **path** is the argument and the token is not, so \
                 that `ps` shows where the secret is and not what it is"
            .to_string(),
    })?;
    let token = std::fs::read_to_string(token_file)
        .map_err(crate::io("read", token_file))?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(Error::Usage {
            detail: format!(
                "{} holds no token. An empty bearer would make every request carrying an empty \
                 header succeed, which is 「authentication is off」 wearing the shape of \
                 「authentication passed」",
                token_file.display()
            ),
        });
    }

    let layout = Layout::open(project)?;
    let recorded = recorded_signing_keyid(&layout)?;
    let signing_id = spec
        .signing_key
        .clone()
        .or_else(|| recorded.clone())
        .ok_or_else(|| Error::Usage {
            detail:
                "this server has no signing key: pass --signing-key <KEY_ID>, or record one in \
                     `.gx/config.toml` as `engine_signing_keyid = \"…\"` (req/56 §2's 「公開 keyid \
                     の参照のみ」 slot, E-M6-7). Every `VerdictReceipt` and `CommitReceipt` this \
                     surface issues is signed with it, and 45 §1 keeps it distinct from the \
                     adjudicator's key"
                    .to_string(),
        })?;

    let signing = store.load(&signing_id)?;
    let mut rulers = BTreeMap::new();
    for entry in store.list()? {
        if let Ok(key) = store.load(&entry.key_id) {
            rulers.insert(entry.key_id.clone(), key);
        }
    }
    let keys = Arc::new(StoreKeys { signing, rulers });

    let evidence = RequestEvidence::new();
    let mode = spec.record_only.then_some(EnforcementMode::RecordOnly);
    let posture = match spec.fail_posture.as_deref() {
        None | Some("closed") => FailPosture::FailClosed,
        Some("open") => FailPosture::FailOpen,
        Some(other) => {
            return Err(Error::Usage {
                detail: format!(
                    "--fail-posture takes closed|open (44 §1.2); got {other:?}. The two axes are \
                     independent (43 §4): --record-only decides whether a Deny still applies, \
                     --fail-posture decides what happens when the verifier cannot be reached"
                ),
            })
        }
    };
    let engine = crate::session::open_engine(&layout, evidence.clone(), mode, posture, None)?;

    let archive = Arc::new(StoreArchive {
        store: ReceiptStore::in_layout(&layout),
    });
    let state = AppState::new(
        engine,
        evidence,
        keys,
        Bearer::new(token),
        layout.join("index"),
        recorded.as_deref(),
    )
    .map_err(|e| Error::Usage {
        detail: format!("{}: {}", e.title, e.detail),
    })?
    .with_archive(archive);

    Ok((state, bind))
}

/// 44 §1.2's start-up log line: 「stdout: 起動ログ（構造化JSON行）」.
///
/// 🔴 It carries the absence. `gx_api::auth::ABSENCE_NOTICE` is in `--help`, which an operator reads
/// once; this line is in the log, which is what a second operator finds six months later when they
/// are asking what this process is. 44 §1.2 asks for a structured line and does not say what it holds,
/// so what it holds is the four facts that decide whether the socket is safe: where it is, which
/// policy set is behind it, which key signs, and that the only check is a token.
#[must_use]
pub fn start_line(addr: SocketAddr, spec: &ServeSpec, signing_key_id: &str) -> serde_json::Value {
    serde_json::json!({
        "event": "gx.serve.started",
        "bind": addr.to_string(),
        "enforcement": if spec.record_only { "RecordOnly" } else { "Enforce" },
        "fail_posture": spec.fail_posture.as_deref().unwrap_or("closed"),
        "signing_key_id": signing_key_id,
        "authorization": gx_api::auth::ABSENCE_NOTICE,
    })
}

/// The configuration `gx_api::serve` is handed.
#[must_use]
pub fn config(bind: SocketAddr) -> ServeConfig {
    ServeConfig::new(bind)
}
