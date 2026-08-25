// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-11, adopted (b)** (sem: SEM-gx-api-157) — `Idempotency-Key`, written by hand, persisted under `.gx/index/`.
//!
//! 44 §2.4, verbatim:
//!
//! > `POST /candidates/{id}/commit` and `POST /transformations/{id}/undo` accept the `Idempotency-Key`
//! > header. The server caches the response, keyed on `(transformation_id, idempotency_key)`, for a fixed
//! > period (default 24 hours, the value is an operational setting), and **does not re-run the side effect** on a
//! > resend with the same key — it returns the identical response. A different request body with the same key gets `409 IDEMPOTENCY_CONFLICT` (sem: SEM-gx-api-158).
//!
//! # 🔴 The one line M6-11 requires, and it is a correction
//!
//! > the engine's commit is already idempotent, so what the cache protects is **response identity**, not the side effect.
//! > Without writing down this distinction, the false claim "Idempotency-Key prevents double application" arises
//! > (what actually prevents it is T-11's key-idempotency = ASM-43-1) (sem: SEM-gx-api-159)
//!
//! So: **this cache prevents no double application.** 43 T-11's `ledger.append` is keyed on the
//! `TransformationId` and is idempotent (ASM-43-1), `Engine::commit` answers `Committed` for a row
//! that already reached it, and 43 T-9's critical section is what makes a second `apply` impossible.
//! What the cache adds is that the *second response is byte-identical to the first* — which matters
//! because 44 §2.4's stated purpose is "preventing double application when a network break causes
//! 'apply succeeded, but the response was lost'" (sem: SEM-gx-api-160) and a client that retries after losing a response needs the same `Receipt` back rather than
//! a second document describing the same commit differently.
//!
//! Deleting this whole module would therefore not make a double apply possible. That is the sentence
//! M6-11 asks for, and it is worth being uncomfortable about: the feature 44 names after the
//! strongest guarantee in the file is not the thing providing it.
//!
//! # 🔴 Why it is on disk and not in a map
//!
//! §47 adopted (b) over (a) with the reason in the ruling: "44's reason for existing is 'preventing double application
//! when a network break causes apply-succeeded-response-lost', and **that break can involve a process restart**" (sem: SEM-gx-api-161). A
//! process-memory cache is a cache that is empty exactly when the failure it exists for has
//! happened.
//!
//! `.gx/index/` is the right directory and req/56 §2 says why: "derived — **declared safe to delete** —
//! regenerates if lost" (sem: SEM-gx-api-162). A lost record costs a client one duplicated *response*, never a duplicated
//! *effect* — which is only true because of the paragraph above, and is the second reason that
//! paragraph has to be written down.
//!
//! # Not a crate
//!
//! §47 M6-11: "**write it ourselves** (SURVIVORS' dead verdict: the two off-the-shelf crates' existence is unconfirmed, do not use them)" (sem: SEM-gx-api-163).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gx_core::{Timestamp, TransformationId};
use serde::{Deserialize, Serialize};

use crate::problem::ApiError;

/// 44 §2.4's "default 24 hours" (sem: SEM-gx-api-164), in nanoseconds.
///
/// "the value is an operational setting" (sem: SEM-gx-api-165) and there is no operations surface in v0.1, so the default is the value. A
/// constant rather than a field: a knob nothing turns is a knob that misleads a reader about what is
/// configurable (M4H5-5's shape).
pub const TTL_NANOS: i64 = 24 * 60 * 60 * 1_000_000_000;

/// One remembered response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// 🔴 The request body, verbatim, for 44 §2.4's "a different request body with the same key" check (sem: SEM-gx-api-166).
    ///
    /// The **request** and not the response: the conflict 44 defines is about what was asked, and a
    /// server comparing what it answered could not tell a second question from a repeated one.
    ///
    /// Kept whole rather than digested. A digest would be shorter and would answer only "these
    /// differ" (sem: SEM-gx-api-167); the bodies 44 gives these two endpoints are a missing body and `{actor}`, so the
    /// cost is a few dozen bytes and what it buys is that an operator looking at a
    /// `IDEMPOTENCY_CONFLICT` can see **which two requests** disagreed. Nothing secret is in either:
    /// 42 §3.2's `Actor` carries a key **id**, and req/56 §1's "don't put secrets in the project" (sem: SEM-gx-api-168) is about
    /// key material.
    pub request: serde_json::Value,
    /// The response body that was sent, verbatim.
    pub response: serde_json::Value,
    /// The HTTP status that was sent with it.
    pub status: u16,
    /// When it was recorded, for [`TTL_NANOS`].
    pub at_unix_nanos: i64,
}

/// The file for one transformation: every idempotency key seen for it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Wire {
    keys: BTreeMap<String, Entry>,
}

/// `.gx/index/idempotency/`, as a store.
///
/// 🔴 **One file per transformation, keyed inside** rather than one file per `(id, key)` pair. The
/// reason is that 44 §2.4 lets the **client** choose the key, and a client-chosen string in a path
/// component is a traversal waiting for a `../`. Hashing the key would fix that and would put a
/// content-address-shaped thing in a crate Rule 1 (i) forbids to mint one (sem: SEM-gx-api-169); a map key inside a JSON
/// document needs no name at all.
#[derive(Debug, Clone)]
pub struct IdempotencyStore {
    dir: PathBuf,
}

impl IdempotencyStore {
    /// The store under a caller-supplied `.gx/index/`.
    ///
    /// The path is an argument because this crate never spells `.gx/`: req/56 §2's table is
    /// gx-cli's single declaration and a second spelling here would be a second list to keep in
    /// step (see the manifest for why the dependency cannot go the other way).
    #[must_use]
    pub fn at(index_dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: index_dir.into().join("idempotency"),
        }
    }

    /// Where one transformation's keys are filed.
    ///
    /// The transformation's text form with `:` replaced — the shape `DraftStore` and `ReceiptStore`
    /// already use, and for their reason (Windows refuses `:` in a filename).
    #[must_use]
    pub fn path_of(&self, id: &TransformationId) -> PathBuf {
        self.dir
            .join(format!("{}.json", id.0.to_text().replace(':', "_")))
    }

    /// What was answered for this pair, if anything, and if it has not expired.
    ///
    /// # Errors
    /// [`ApiError`] `INTERNAL` if the file exists and cannot be read. **Not** if it does not parse:
    /// req/56 §2 declares this directory disposable, so an unreadable entry is a cache miss and the
    /// caller does the work again. That is the one place this directory's nature makes "unreadable"
    /// and "absent" (sem: SEM-gx-api-170) the same answer, and `crates/gx-cli/src/index.rs` documents the same exception
    /// for the same directory.
    pub fn get(
        &self,
        id: &TransformationId,
        key: &str,
        now: Timestamp,
    ) -> Result<Option<Entry>, ApiError> {
        let Some(wire) = self.read(id)? else {
            return Ok(None);
        };
        let Some(entry) = wire.keys.get(key) else {
            return Ok(None);
        };
        if now.0.saturating_sub(entry.at_unix_nanos) > TTL_NANOS {
            // Expired. Reported as a miss and **not** deleted: deleting on read would make a
            // read mutate a directory, and 44 §2.4's "a fixed period" (sem: SEM-gx-api-171) is about what may be answered from
            // the cache rather than about when bytes leave a disk.
            return Ok(None);
        }
        Ok(Some(entry.clone()))
    }

    /// Remember what was answered.
    ///
    /// # Errors
    /// [`ApiError`] `INTERNAL` if the directory or the file cannot be written. 🔴 A failure here is
    /// **not** silently swallowed, unlike the resolution cache in gx-cli: that one caches a value
    /// the engine can recompute, and this one caches a response nothing can reconstruct once the
    /// side effect has happened. A commit whose record could not be written is a commit whose retry
    /// will re-drive the pipeline, and the caller has to be told.
    pub fn put(
        &self,
        id: &TransformationId,
        key: &str,
        request: &serde_json::Value,
        status: u16,
        response: &serde_json::Value,
        now: Timestamp,
    ) -> Result<(), ApiError> {
        let mut wire = self.read(id)?.unwrap_or_default();
        wire.keys.insert(
            key.to_string(),
            Entry {
                request: request.clone(),
                response: response.clone(),
                status,
                at_unix_nanos: now.0,
            },
        );
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| ApiError::internal(format!("create {}: {e}", self.dir.display())))?;
        let path = self.path_of(id);
        let body = serde_json::to_vec_pretty(&wire)
            .map_err(|e| ApiError::internal(format!("encode idempotency record: {e}")))?;
        std::fs::write(&path, body)
            .map_err(|e| ApiError::internal(format!("write {}: {e}", path.display())))
    }

    /// The directory this store writes to (for a report that has to say where).
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn read(&self, id: &TransformationId) -> Result<Option<Wire>, ApiError> {
        let path = self.path_of(id);
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(ApiError::internal(format!("read {}: {e}", path.display()))),
        };
        Ok(serde_json::from_slice::<Wire>(&raw).ok())
    }
}

/// 44 §2.4's "a different request body with the same key" (sem: SEM-gx-api-172) — the remembered answer, or the conflict.
///
/// # Errors
/// [`ApiError`] `IDEMPOTENCY_CONFLICT` (409) when the key was used for a different body.
pub fn replay_or_conflict(
    key: &str,
    entry: &Entry,
    request: &serde_json::Value,
) -> Result<(u16, serde_json::Value), ApiError> {
    if &entry.request != request {
        return Err(ApiError::new(
            "IDEMPOTENCY_CONFLICT",
            "this idempotency key was used for a different request",
            format!(
                "44 §2.4: \"a different request body with the same key gets `409 IDEMPOTENCY_CONFLICT`\" (sem: SEM-gx-api-173). \
                 The key {key:?} is recorded against {} and this request is {request}",
                entry.request
            ),
        ));
    }
    Ok((entry.status, entry.response.clone()))
}
