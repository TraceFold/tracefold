// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-922-F2 phase 1** — `gx object export` and `gx object verify`.
//!
//! The format itself is `gx_witness::gxfile`; this file is the surface over it. Spec: `req/922`
//! §3 (F5, F6) and the proposed chapter `req/922_artifacts/spec42_fileformat_chapter_draft.md`
//! §3.16.4/§3.16.5 — **a proposal, not spec canon**.
//!
//! # Why `gx object <verb>` and not `gx export` / `gx verify`
//!
//! `req/922` F5 spells the pair `gx export <id>` / `gx import <file>` and §8 spells the check
//! `gx verify <file.gx>`. **`gx verify <TRANSFORMATION>` already exists** (44 §1.2, T-3 → T-4), so
//! the reqdef's spelling for the check cannot be taken without changing what an existing verb does
//! with an existing argument — which this phase forbids itself. Splitting the pair across two
//! levels (`gx export` at the top, the check underneath something else) would leave a reader
//! unable to guess the second verb from the first, so both live under one group instead, which is
//! this binary's dominant idiom (`gx receipt verify`, `gx checkpoint export`, `gx log checkpoint`).
//! The deviation is reported in the R-922-F2 report rather than settled here; renaming a verb that
//! has never shipped is one `mv`.
//!
//! # What this phase does **not** do
//!
//! No `import`, no `inspect`, no `--kind` (one kind is shipped, so a flag with one legal value is
//! noise), and no `--checkpoint`: `verify` here is the offline pair "the file is what it says it
//! is" + "its signature holds", and the inclusion answer is whatever the offline verifier says
//! with no anchor — `unanchored` for a commit receipt, which is not a pass and says so.
//!
//! 🔴 **R-930-B1 makes half of that stale and the other half still true.** A second kind ships
//! (`DesignToken`, `req/939`), so "one legal value" no longer holds — but `export` still reads from
//! the receipt store and so still has one kind to write, and `verify` still takes the kind off the
//! file's own header rather than from a flag. What did change is the answer for a kind that carries
//! no signature, which is [`unsigned`] below. `--kind` becomes a real question the day something
//! other than a receipt can be exported, and `req/939` §5-2 files it rather than answering it here.

use std::path::Path;

use gx_core::TransformationId;
use gx_witness::gxfile;

use crate::exit::{Outcome, VERIFY_FAILED};
use crate::keys::KeyStore;
use crate::receipt::{judge, ReceiptStore};
use crate::{io, keys, Error, Result};

/// What the CLI calls a refusal from the format layer.
///
/// The fold `gxfile`'s own header names: a [`gxfile::Refusal`] becomes this crate's existing
/// `Malformed`, whose sentence is "{path} is not a readable {what}: {detail}" — the same road
/// `receipt::read_receipt` already takes for a `serde_json` error, and the same `gx_code`
/// (`VALIDATION_ERROR`, exit 1). [`gxfile::Refusal::IdentityMismatch`] does **not** come here: a
/// file that parses and whose identity claim is false has been checked and failed, which is 44
/// §1.4's 7, and `verify` answers it as a verdict rather than as bad input.
const WHAT: &str = "gx object file";

fn malformed(path: &Path, refusal: &gxfile::Refusal) -> Error {
    Error::Malformed {
        what: WHAT,
        path: path.display().to_string(),
        detail: refusal.to_string(),
    }
}

/// `gx object export <ID> --out <FILE>` — write the receipt this project filed for `ID` as a
/// `.gx` file.
///
/// The receipt is carried out byte for byte inside its envelope: nothing is re-encoded, so the
/// identity written into the header is the identity the body already had.
///
/// # Errors
/// [`Error::NotFound`] when this project has filed no receipt for `ID`; [`Error::Malformed`] when
/// the filed document cannot be wrapped (a body that is not canonical DAG-CBOR); [`Error::Io`]
/// when the destination cannot be written.
pub fn export(store: &ReceiptStore, id: &TransformationId, out: &Path) -> Result<Outcome> {
    let Some((stored_kind, receipt)) = store.first_available(id)? else {
        return Err(Error::NotFound {
            what: "receipt (this project has filed none for that id)",
            id: id.0.to_text(),
        });
    };
    let bytes = gxfile::write_receipt(&receipt).map_err(|refusal| Error::Malformed {
        what: "filed receipt",
        path: store.path_of(id, stored_kind).display().to_string(),
        detail: refusal.to_string(),
    })?;
    std::fs::write(out, &bytes).map_err(io("write", out))?;

    // The identity is recomputed from what was written rather than remembered from what was
    // wrapped: the number this answer prints is then a fact about the file on the disk.
    let written = gxfile::read(&bytes).map_err(|refusal| malformed(out, &refusal))?;
    Ok(Outcome::ok(serde_json::json!({
        "path": out.display().to_string(),
        "format_version": written.format_version,
        "kind": written.kind.name(),
        // Which of the three documents a transformation ends with this is (`gx receipt show`'s
        // `stored_kind`): the envelope's kind is `Receipt` for all three, and a reader who could
        // not tell them apart would read "this change was applied" out of "this change was judged".
        "stored_kind": stored_kind.tag(),
        "cid": written.cid.to_text(),
        "bytes": bytes.len(),
    })))
}

/// `gx object verify <FILE> [--key <FILE>]` — the file is what it says it is, and it is signed.
///
/// Two questions, answered in order and reported apart:
///
/// 1. **identity** — the header's claim against `BLAKE3(enc(body))`, recomputed here
///    (`req/922` §7-3). A file that fails this is refused before its signature is looked at,
///    because the two answers would say the same thing and the first one is the cheaper truth.
/// 2. **signature** — delegated whole to [`crate::receipt::judge`], so this verb and
///    `gx receipt verify` cannot drift into two verdicts about one document.
///
/// # Errors
/// [`Error::Io`] when the file cannot be read, [`Error::Malformed`] for every refusal the format
/// layer raises except the identity mismatch, [`Error::KeyFormat`]-family refusals through
/// [`keys::read_public`] when a key is given and cannot be read.
pub fn verify(path: &Path, key: Option<&Path>) -> Result<Outcome> {
    let bytes = std::fs::read(path).map_err(io("read", path))?;
    let file = match gxfile::read(&bytes) {
        Ok(file) => file,
        // 🔴 The one refusal that is a **verdict** and not bad input. `checks.signature` is `null`
        // and not `false` for `judge`'s reason: nothing downstream ran, and `false` would claim a
        // signature was checked and disagreed.
        Err(refusal @ gxfile::Refusal::IdentityMismatch { .. }) => {
            return Ok(Outcome::refused(
                serde_json::json!({
                    "valid": false,
                    "checks": {
                        "identity": false,
                        "signature": serde_json::Value::Null,
                        "canonical_cid": serde_json::Value::Null,
                        "inclusion": serde_json::Value::Null,
                        "revocation": serde_json::Value::Null,
                    },
                    "refusal": refusal.to_string(),
                }),
                VERIFY_FAILED,
            ));
        }
        Err(refusal) => return Err(malformed(path, &refusal)),
    };

    // 🔴 **R-930-B1** — a kind that carries no signature answers the first question and not the
    // second, and the two are reported apart rather than folded (`req/939` §2-F-2).
    let Some(receipt) = file.receipt() else {
        return Ok(unsigned(&file));
    };

    let public = match key {
        Some(path) => keys::read_public(path)?,
        // The owner's convenience path, identical to `gx receipt verify`'s: the key id the
        // document declares, in the local store. A third party has no such store, which is what
        // `--key` is for.
        None => {
            let key_id = receipt.payload()?.key_id;
            KeyStore::user_default()?.load(&key_id)?.public()
        }
    };

    let mut outcome = judge(receipt, &public, None, "none", false, None);
    if let Some(map) = outcome.json.as_object_mut() {
        map.insert("format_version".into(), file.format_version.into());
        map.insert("kind".into(), file.kind.name().into());
        map.insert("cid".into(), file.cid.to_text().into());
        if let Some(checks) = map.get_mut("checks").and_then(|c| c.as_object_mut()) {
            checks.insert("identity".into(), true.into());
        }
    }
    Ok(outcome)
}

/// 🔴 The answer for an object whose kind has no signer (**R-930-B1**, `req/939` §2-F-2).
///
/// `verify` asks two questions. The first — is the file what it says it is — has been answered by
/// the time this is reached, and answered `true`: the identity was recomputed from the body and
/// agreed with the header's claim. The second has **no answer**, because nothing signs a document
/// of this kind, and the three spellings available for that are not interchangeable:
///
/// * `false` would say a signature was checked and disagreed. Nothing was checked.
/// * `true` would say a document nobody signed had been verified. That is the overclaim this
///   project's whole vocabulary exists to prevent.
/// * `null` is what this function already uses one branch above for "nothing downstream ran", and
///   it is reused here rather than a fourth word being minted.
///
/// `valid` is `null` for the same reason: the summary of a question that was not asked is not a
/// boolean. A caller that requires a boolean therefore fails at its own call site instead of
/// reading a guess, which is the direction an unanswered question should break in. The exit code
/// says only that nothing refused the file — the same reading `unanchored` already has in this
/// verb — and the sentence beside it says what was not done.
fn unsigned(file: &gxfile::GxObjectFile) -> Outcome {
    Outcome::ok(serde_json::json!({
        "valid": serde_json::Value::Null,
        "checks": {
            "identity": true,
            "signature": serde_json::Value::Null,
            "canonical_cid": serde_json::Value::Null,
            "inclusion": serde_json::Value::Null,
            "revocation": serde_json::Value::Null,
        },
        "format_version": file.format_version,
        "kind": file.kind.name(),
        "cid": file.cid.to_text(),
        "unsigned_because": "no signer exists for this kind, so the signature question was not \
                             asked; the identity above was recomputed from the body and is the \
                             only thing this answer attests",
    }))
}
