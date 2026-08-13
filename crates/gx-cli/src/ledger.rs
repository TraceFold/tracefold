//! `gx log proof` / `gx log consistency` / `gx log checkpoint` — 44 §1.2 and **M6-24 採(b)**.
//!
//! # 🔴 The third verb is not in 44, and AC-057 cannot be set up without it
//!
//! req/88 §4 M6-24 is a hand-2 blocker and §47 adopted (b):
//!
//! > **CLI/API が要求時に発行する**(`gx log checkpoint` 相当 / `GET /ledger/checkpoint` の handler が
//! > `unsigned_checkpoint` → `sign_checkpoint` を呼ぶ)——`Σ` は動かず、checkpoint は「その時の木に
//! > ついての署名済み言明」という 42 §3.11 の定義どおりの物になる
//!
//! The measurement that made it a blocker: `gx_witness::dsse::sign_checkpoint` had **zero callers
//! outside gx-witness**, so no signed checkpoint existed anywhere in the shipping code — and
//! AC-057's Given is a receipt verified 「`--checkpoint`で与えた既知`Checkpoint`に対して」. No
//! producer, no Given.
//!
//! 44 §1.1's table gives `gx log` two verbs and this adds a third, which is a **surface addition to
//! a specification this lane may not edit**. Raised as **M6H2-7**: 44 §2.6 permits 「新規エンドポイ
//! ント」 as backward-compatible on the HTTP side and says nothing about the CLI, and the HTTP twin
//! (`GET /ledger/checkpoint`, hand 5) is already in 44 §2.2 — so the CLI is the asymmetric half.
//!
//! # 🔴 Who can produce one
//!
//! §47 M6-24's clause: 「署名鍵は ledger signing key であり、**CLI が鍵を持っていない環境(第三者の
//! verifier)では checkpoint を作れない**=正しい。作れるのは台帳の持ち主だけ」. `--key` is therefore
//! required and there is no fallback that invents one. A third party running AC-057 receives a
//! checkpoint; they do not mint one.
//!
//! # Nothing is written unless asked
//!
//! The signed head goes to stdout (44 §1.3's single JSON) and to `--out` if given. `.gx/checkpoints/`
//! gets its producer that way — an operator writes `--out .gx/checkpoints/<n>.json` — rather than by
//! this command deciding to store. Who runs it on a schedule is M5-10's shape of question and this
//! hand does not answer it.

use std::path::{Path, PathBuf};

use gx_core::{Checkpoint, Cid, Timestamp, TransformationId};
use gx_log::proof::{prove_consistency, prove_inclusion, unsigned_checkpoint};
use gx_log::LedgerStore;
use gx_witness::dsse::sign_checkpoint;
use gx_witness::KeyPair;

use crate::exit::Outcome;
use crate::{io, layout::Layout, Error, Result};

/// 42 §3.11's example namespace, and this version's default `origin`.
///
/// 「The log's namespace… It is what stops a checkpoint of one log from verifying against another's
/// key」. A default rather than a requirement: `--origin` overrides it, because an operator running
/// two logs needs two namespaces and a constant compiled into the binary would give them one.
pub const DEFAULT_ORIGIN: &str = "glovrex-ledger/v1";

/// Open the ledger a project's `.gx/` holds, **without creating one**.
///
/// `LedgerStore::open` creates the file if it is absent, which is right for the engine and wrong
/// here: every verb in this module reads, and a read that left an empty ledger behind would make
/// 「there is no ledger」 unobservable after the first attempt. So absence is checked first and
/// answered as absence.
///
/// # Errors
/// [`Error::NotFound`] if the project has no ledger file yet; [`Error::Log`] if it cannot be opened
/// or replayed.
pub fn open(layout: &Layout) -> Result<LedgerStore> {
    let path = layout.ledger_path();
    if !path.is_file() {
        return Err(Error::NotFound {
            what: "ledger",
            id: path.display().to_string(),
        });
    }
    Ok(LedgerStore::open(&path)?)
}

/// What `--leaf` was given: 44 §1.2 accepts both (「`--leaf <INDEX|TRANSFORMATION_ID>`」).
pub enum Leaf {
    /// A leaf index.
    Index(u64),
    /// A transformation, to be resolved to an index.
    Transformation(TransformationId),
}

impl Leaf {
    /// Parse 44's `<INDEX|TRANSFORMATION_ID>`.
    ///
    /// A bare integer is an index and a `gx1:` value is an id. The order matters and is the one 44
    /// §0 implies: base32 never renders a decimal-only string, so nothing is ambiguous.
    ///
    /// # Errors
    /// [`Error::Usage`] if the argument is neither.
    pub fn parse(text: &str) -> Result<Self> {
        if let Ok(index) = text.parse::<u64>() {
            return Ok(Leaf::Index(index));
        }
        // `Cid::from_text` and not a mint: 則 1 (i). Parsing a name is not making one, and the
        // parser lives in gx-core precisely so that this line does not have to reach gx-canon.
        Cid::from_text(text)
            .map(|cid| Leaf::Transformation(TransformationId(cid)))
            .map_err(|e| Error::Usage {
                detail: format!("`{text}` is neither a leaf index nor a `gx1:` id: {e}"),
            })
    }
}

/// 🔴 `gx log proof --leaf <INDEX|TID>` (44 §1.2). stdout: an `InclusionProof` (42 §3.11).
///
/// # Errors
/// [`Error::Log`] if the tree refuses the index. A leaf that is not in the log is **not** an error
/// here: it is an answer with an object saying which leaf, for the reason [`crate::receipt::show`]
/// gives.
///
/// 🔴 **E-M6-24** (req/38 §55, M6H8-14 ②): that answer exits **6**, not 1. 44 §1.2's `log` line says
/// 「1=範囲外/未検出」 and §1.4's common table gives 「未検出（not-found）」 the code 6; M6-25 ruled that
/// the common table wins and §1.2's per-command lists are excerpts, and E-M6-13/E-M6-16 applied that
/// to `cancel`, `escalation` and `undo`. This verb was the one place the same principle had not been
/// applied — hand 2 recorded the divergence in `exit::EXIT_DIVERGENCES` and left it standing.
pub fn proof(store: &LedgerStore, leaf: &Leaf) -> Result<Outcome> {
    let log = store.log();
    let index = match leaf {
        Leaf::Index(i) => *i,
        Leaf::Transformation(tid) => {
            // The resolution req/88 §2.1 row 10 names: 「`--leaf` の TID→index 解決は…
            // `CommittedRow{transformation, ledger_seq}` で可」. Taken off the ledger's own entries
            // rather than off Σ, because this command has no engine and the ledger is the durable
            // artefact that answers the question directly.
            let Some(entry) = log.entries().iter().find(|e| e.transformation == *tid) else {
                return Ok(Outcome::refused(
                    serde_json::json!({
                        "leaf": tid.0.to_text(),
                        "found": false,
                        "tree_size": log.len(),
                    }),
                    crate::exit::NOT_FOUND,
                ));
            };
            entry.index
        }
    };
    if index >= log.len() {
        return Ok(Outcome::refused(
            serde_json::json!({
                "leaf": index,
                "found": false,
                "tree_size": log.len(),
            }),
            crate::exit::NOT_FOUND,
        ));
    }
    let proof = prove_inclusion(log, index)?;
    Ok(Outcome::ok(serde_json::to_value(&proof).map_err(|e| {
        Error::Malformed {
            what: "inclusion proof",
            path: String::new(),
            detail: e.to_string(),
        }
    })?))
}

/// 🔴 `gx log consistency --from <SIZE> --to <SIZE>` (44 §1.2). stdout: a `ConsistencyProof`.
///
/// # Errors
/// [`Error::Log`] if the sizes are not a pair this tree can prove between. `gx_log` refuses
/// `old > new` and sizes beyond the tree, and that refusal is carried rather than re-decided — a
/// second opinion about what a tree contains is the drift E-M2-12 exists to prevent.
pub fn consistency(store: &LedgerStore, from: u64, to: u64) -> Result<Outcome> {
    let log = store.log();
    match prove_consistency(log, from, to) {
        Ok(proof) => Ok(Outcome::ok(serde_json::to_value(&proof).map_err(|e| {
            Error::Malformed {
                what: "consistency proof",
                path: String::new(),
                detail: e.to_string(),
            }
        })?)),
        // 44 §1.2 spells the whole of this command's failure as `範囲外/未検出`, so an out-of-range
        // pair is answered rather than raised: the object says what the tree's size actually is,
        // which is the one fact a caller who guessed wrong needs. The status is §1.4's **6**
        // (**E-M6-24**, req/38 §55) rather than §1.2's 1 — see `proof` above for the reading.
        Err(e) => Ok(Outcome::refused(
            serde_json::json!({
                "from": from,
                "to": to,
                "tree_size": log.len(),
                "refusal": e.to_string(),
            }),
            crate::exit::NOT_FOUND,
        )),
    }
}

/// 🔴 `gx log checkpoint --key <FILE> [--origin <STR>] [--out <PATH>]` — **M6-24 採(b)**.
///
/// `unsigned_checkpoint` then `sign_checkpoint`, which is the first call to the latter outside
/// gx-witness in this repository. `at` is an argument and not a clock read: 41 §6 injects time at
/// the engine boundary and [`crate::clock::now`] is the binary's one reader (則 2).
///
/// # Errors
/// [`Error::Log`] if the log is empty — 42 §3.11's head is 「その時の木についての署名済み言明」 and a
/// tree of zero entries has no root, so `unsigned_checkpoint` refuses. [`Error::Witness`] if the
/// signing bytes have no canonical form. [`Error::Io`] if `--out` cannot be written.
pub fn checkpoint(
    store: &LedgerStore,
    key: &KeyPair,
    origin: &str,
    at: Timestamp,
    out: Option<&Path>,
) -> Result<Outcome> {
    let unsigned = unsigned_checkpoint(store.log(), origin, at)?;
    let signed = sign_checkpoint(&unsigned, key.signing_key(), key.key_id())?;
    let json = serde_json::to_value(&signed).map_err(|e| Error::Malformed {
        what: "checkpoint",
        path: String::new(),
        detail: e.to_string(),
    })?;
    if let Some(path) = out {
        write_checkpoint(path, &json)?;
    }
    Ok(Outcome::ok(json))
}

/// The bytes `--out` writes, which are the bytes stdout carries.
///
/// One serialisation for both, so that a checkpoint an operator stored and a checkpoint they piped
/// are the same document. Two writers would be two documents that verify differently the day one of
/// them gains a field.
fn write_checkpoint(path: &Path, json: &serde_json::Value) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(io("create", parent))?;
        }
    }
    let body = serde_json::to_vec_pretty(json).map_err(|e| Error::Malformed {
        what: "checkpoint",
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    std::fs::write(path, body).map_err(io("write", path))?;
    Ok(path.to_path_buf())
}

/// The head of the local ledger, **unsigned**, for `gx receipt verify` without `--offline`.
///
/// 44 §1.2 calls the non-offline path 「台帳照会による`inclusion_proof`検証」. There is no ledger
/// client in v0.1 — `gx-api` serves nothing until hands 5 and 6 — so the enquiry is against the
/// local store, and the caller reports which anchor it used. Unsigned because the anchor's role in
/// `verify_offline` is to supply a root and a size; the signature would be checked by
/// `gx_witness::dsse::verify_checkpoint`, and a local head has nobody to attest it to itself.
///
/// # Errors
/// [`Error::Log`] if the log is empty.
pub fn local_head(store: &LedgerStore, at: Timestamp) -> Result<Checkpoint> {
    Ok(unsigned_checkpoint(store.log(), DEFAULT_ORIGIN, at)?)
}
