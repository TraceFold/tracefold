// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/824` A4** — the attach-source registry: who may present observations, and what they
//! declare they cover.
//!
//! | method / path | what it is |
//! |---|---|
//! | `POST /v1/attach-sources` | register a source. **201** + the row, `Idempotency-Key` honoured |
//! | `GET /v1/attach-sources` | the registry, paged, zero-inclusive (F-B) |
//! | `GET /v1/attach-sources/{id}` | one source, or `404 SOURCE_UNKNOWN` |
//!
//! # 🔴 An attach-source is NOT a `SubstrateAdapter`, and this module is where that refusal lives
//!
//! An adapter is a substrate this product can read **and write**; an attach-source is an external
//! executor we can only receive reports from, and SS273 forbids the write half permanently.
//! `req/824` §2 kills `gx-adapter-vercel` by name: typing a non-writable observation source as an
//! adapter would make every conformance contract (escrow-before-apply, AC-050's bit-equal
//! round-trip) structurally inapplicable — an adapter that fails its own suite by construction, or
//! a suite weakened until it passes. So the registry holds a **second member kind** beside
//! adapters (`req/812` §3-R1), and `req/805` P-18's page renders the distinction rather than
//! flattening it. This registry is `req/805` Phase B (a)'s enumeration endpoint, extended — not a
//! rival beside it.
//!
//! # 🔴 Rule 1 — why this map is membrane state and not an engine object
//!
//! The registry holds no semantic authority: it decides who may POST and echoes what they declared,
//! and nothing in it touches a `Verdict`, a `Lifecycle` or canonical bytes. By Rule 1
//! (SEM-gx-api-174) it therefore must **not** live in the engine — it is one map behind a lock in
//! [`crate::state::AppState`], M6-06 adopted (a)'s serialisation discipline one field over
//! (the health snapshot's own shape).
//!
//! # 🔴 What `coverage_verified: false` is doing on every response
//!
//! The registry records what a source **declares** it covers. Glovrex does not verify the
//! declaration; `declared_coverage` is the source's own claim and is rendered as such. The field is
//! constantly `false` in this phase and is present anyway, because omitting it would let a reader
//! take the declaration for a measurement — the single most likely misreading of this surface
//! (`req/824` A4 LIMITS; `docs/LIMITS.md` carries the Half-B row).
//!
//! # 🔴 Declared deltas (in `req/824` §0's protocol: the source is not edited)
//!
//! 1. **Registrations do not survive a restart in this atom.** `req/824` A4's "state persistence
//!    behind the existing single lock" is implemented as exactly that — state, behind the lock.
//!    A disk home would have to be either `.gx/index/` (declared *safe to delete* by req/56 §2,
//!    which a non-regenerable registry is not) or a new `.gx/` row (a layout decision that is
//!    gx-cli's to declare, not this crate's — the `ReceiptArchive`/`DraftArchive` precedent).
//!    Deferred to the atom that needs it (A5's observations reference sources by id and land in
//!    the journal, which *does* survive), and declared here rather than silently decided.
//! 2. The idempotency replay for `POST /attach-sources` lives in this registry rather than in
//!    [`crate::idempotency::IdempotencyStore`]: that store is keyed on a `TransformationId` and a
//!    registration has none. The conflict answer reuses [`crate::idempotency::replay_or_conflict`]
//!    verbatim, so 44 §2.4's semantics are inherited, not re-implemented.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::extract::{Params, Payload, Segment};
use crate::gx_code;
use crate::idempotency::{replay_or_conflict, Entry};
use crate::list::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::problem::ApiError;
use crate::state::AppState;

/// The three routes this module adds, as a declaration something compares with the router
/// (the [`crate::verdict_checkpoints::VERDICT_CHECKPOINT_ENDPOINTS`] shape).
pub const ATTACH_SOURCE_ENDPOINTS: [&str; 3] = [
    "POST /attach-sources",
    "GET /attach-sources",
    "GET /attach-sources/{id}",
];

/// The source families this phase admits (`req/wire/schema/attach_source.schema.json`).
///
/// A value outside the enum is refused at decode rather than defaulted — a mis-filed source would
/// mis-attribute every observation it later presents. The refusal arrives through
/// [`crate::extract::Payload`] as `422 VALIDATION_ERROR`, which is 44 §2.3's word for a body this
/// server cannot read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    /// A Vercel project reporting deploys/envsets it performed.
    #[serde(rename = "vercel")]
    Vercel,
    /// A GitHub Actions workflow reporting its own runs.
    #[serde(rename = "github-actions")]
    GithubActions,
    /// Any CI job speaking the generic shape.
    #[serde(rename = "generic-ci")]
    GenericCi,
}

/// What the source **claims** it reports. Every member optional; the object itself is required —
/// a source that declares nothing cannot be rendered numerator/denominator, so the registration
/// is refused rather than stored with an implicit "everything".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredCoverage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envset: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deploys: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<bool>,
}

/// The body `POST /attach-sources` reads.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterBody {
    pub kind: SourceKind,
    pub name: String,
    pub declared_coverage: DeclaredCoverage,
    /// Optional. Absent means observations from this source are authenticated by the membrane's
    /// Bearer token only — and the response says so in its `limits` array, so a keyless source is
    /// not silently presented as a signed one.
    pub pubkey: Option<String>,
}

/// One registered source.
#[derive(Debug, Clone)]
pub struct SourceRow {
    id: String,
    kind: SourceKind,
    name: String,
    declared_coverage: DeclaredCoverage,
    pubkey: Option<String>,
    registered_at: String,
}

/// 🔴 The registry: one map behind one lock (see the module header for both words).
///
/// `sources` is ordered by registration, and that order is the paging coordinate — the
/// chain-index precedent from `verdict_checkpoints.rs`: total by construction, opaque to a
/// client, and not derivable from any field a later row could share with an earlier one.
#[derive(Debug, Default)]
pub struct Registry {
    sources: Vec<SourceRow>,
    /// `Idempotency-Key` → the remembered answer, in the store's own [`Entry`] shape so that
    /// [`replay_or_conflict`] applies verbatim (declared delta 2, module header).
    replays: BTreeMap<String, Entry>,
}

impl Registry {
    /// Whether a source is registered under this id — `req/824` A5's gate: the registry decides
    /// who may present observations, and an unregistered source cannot.
    pub(crate) fn holds(&self, id: &str) -> bool {
        self.sources.iter().any(|row| row.id == id)
    }
}

type Answer = Result<Response, ApiError>;

fn ok(status: StatusCode, body: serde_json::Value) -> Answer {
    Ok((status, axum::Json(body)).into_response())
}

/// The wire form of one row. `pubkey` appears only when one was registered; `limits` is never
/// empty (P-12: absence of a limit statement renders as read-refused-class honesty, never blank).
fn row_json(row: &SourceRow) -> serde_json::Value {
    let mut limits = vec![
        "The registry records what a source declares it covers; `declared_coverage` is the \
         source's own claim and is rendered as such, never as a measured fact (req/824 A4)"
            .to_string(),
        "Operations this source does not report are unobserved, and their absence is \
         indistinguishable from there being none"
            .to_string(),
    ];
    if row.pubkey.is_none() {
        limits.push(
            "This source registered no public key: its observations are authenticated by the \
             membrane's Bearer token only, not by a source-held key"
                .to_string(),
        );
    }
    let mut value = serde_json::json!({
        "id": row.id,
        "kind": row.kind,
        "name": row.name,
        "registered_at": row.registered_at,
        "declared_coverage": row.declared_coverage,
        // Always false in this phase, and present ANYWAY — see the module header.
        "coverage_verified": false,
        "limits": limits,
    });
    if let Some(pubkey) = &row.pubkey {
        if let Some(map) = value.as_object_mut() {
            map.insert("pubkey".into(), pubkey.clone().into());
        }
    }
    value
}

/// The declared digest form of a source public key: 64 lowercase-or-uppercase hex characters
/// carrying an ed25519 public key gx-witness will accept.
///
/// Validation-only: the parsed key is not held beyond the check in this atom (nothing signs or
/// verifies against it until an ingest route exists to present signed observations — A5+).
fn pubkey_refusal(pubkey: &str) -> Option<ApiError> {
    let refusal = |why: String| {
        Some(ApiError::new(
            gx_code::SOURCE_KEY_INVALID,
            "the presented source key is not one this server can accept",
            format!(
                "{why}. This is the registered source's own key failing, which is a different \
                 fact from the membrane Bearer failing (`UNAUTHORIZED`); folding the two would \
                 make an attacker's probe indistinguishable from a misconfigured token \
                 (req/824 A3)"
            ),
        ))
    };
    if pubkey.len() != 64 || !pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
        return refusal(format!(
            "`pubkey` must be 64 hex characters (an ed25519 public key); {} characters arrived",
            pubkey.len()
        ));
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in pubkey.as_bytes().chunks_exact(2).enumerate() {
        let hex = std::str::from_utf8(chunk).unwrap_or("00");
        bytes[i] = u8::from_str_radix(hex, 16).unwrap_or(0);
    }
    match gx_witness::PublicKey::from_bytes("attach-source-pubkey", &bytes) {
        Ok(_) => None,
        Err(e) => refusal(format!("the 32 bytes decode but are not a usable key: {e}")),
    }
}

/// 🔴 Register a source (**`req/824` A4**). `201` + the row; `Idempotency-Key` replays the same
/// body byte-identically (a CI job that retries its setup step must not multiply the registry).
///
/// # Errors
/// [`ApiError`] — `VALIDATION_ERROR` for a body outside the schema, `SOURCE_KEY_INVALID` for a
/// pubkey that does not decode, `IDEMPOTENCY_CONFLICT` for a reused key over a different body.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Payload(body): Payload<RegisterBody>,
) -> Answer {
    if body.name.is_empty() {
        return Err(ApiError::validation(
            "`name` must be a non-empty string (req/wire/schema/attach_source.schema.json)",
        ));
    }
    if let Some(pubkey) = &body.pubkey {
        if let Some(refusal) = pubkey_refusal(pubkey) {
            return Err(refusal);
        }
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let at = state.now();
    // The request body's canonical JSON, for 44 §2.4's "a different request body with the same
    // key" comparison. Serialising the deserialised value rather than keeping raw bytes means
    // field order does not defeat the replay, which is the store's own behaviour one door over.
    let request = serde_json::json!({
        "kind": body.kind,
        "name": body.name,
        "declared_coverage": body.declared_coverage,
        "pubkey": body.pubkey,
    });

    let mut registry = state.attach_sources();
    if let Some(key) = &key {
        if let Some(entry) = registry.replays.get(key) {
            let (status, response) = replay_or_conflict(key, entry, &request)?;
            return ok(
                StatusCode::from_u16(status).unwrap_or(StatusCode::CREATED),
                response,
            );
        }
    }
    let row = SourceRow {
        id: format!("as-{}", registry.sources.len() + 1),
        kind: body.kind,
        name: body.name,
        declared_coverage: body.declared_coverage,
        pubkey: body.pubkey,
        registered_at: crate::rfc3339::of(at),
    };
    let response = row_json(&row);
    registry.sources.push(row);
    if let Some(key) = key {
        registry.replays.insert(
            key,
            Entry {
                request,
                response: response.clone(),
                status: StatusCode::CREATED.as_u16(),
                at_unix_nanos: at.0,
            },
        );
    }
    ok(StatusCode::CREATED, response)
}

/// The registry, paged on the registration index, **zero-inclusive** (F-B): an empty registry
/// answers `total: 0` explicitly — an absent denominator reads as "unknown" and an explicit zero
/// reads as "measured, and it is none".
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for a limit outside 44 §2.7's range — refused rather than
/// clamped, for `crate::list::Page::limit`'s reason.
pub async fn list(
    State(state): State<AppState>,
    Params(page): Params<crate::verdict_checkpoints::Page>,
) -> Answer {
    let limit = match page.limit {
        None => DEFAULT_LIMIT,
        Some(n) if (1..=MAX_LIMIT).contains(&n) => n,
        Some(n) => {
            return Err(ApiError::validation(format!(
                "`limit={n}` is outside 44 §2.7's range (1..={MAX_LIMIT}, default \
                 {DEFAULT_LIMIT}). A clamped limit would be a page that looks complete and is not"
            )))
        }
    };
    let registry = state.attach_sources();
    let mut items = Vec::new();
    let mut next_cursor = None;
    for (position, row) in registry.sources.iter().enumerate() {
        if let Some(after) = page.cursor {
            if position <= after {
                continue;
            }
        }
        if items.len() == limit {
            next_cursor = Some(position - 1);
            break;
        }
        items.push(row_json(row));
    }
    ok(
        StatusCode::OK,
        serde_json::json!({
            "items": items,
            "next_cursor": next_cursor,
            "total": registry.sources.len(),
        }),
    )
}

/// One source, by the id registration minted.
///
/// # Errors
/// [`ApiError`] `SOURCE_UNKNOWN` (404) — not 44 §2.3's `NOT_FOUND`, which names a transformation;
/// an unregistered attach-source is a different absent thing and `req/824` A3 minted the word.
pub async fn get(State(state): State<AppState>, Segment(id): Segment<String>) -> Answer {
    let registry = state.attach_sources();
    match registry.sources.iter().find(|row| row.id == id) {
        Some(row) => ok(StatusCode::OK, row_json(row)),
        None => Err(ApiError::new(
            gx_code::SOURCE_UNKNOWN,
            "no attach-source is registered under this id",
            format!(
                "`{id}` names no registered attach-source; `GET /v1/attach-sources` lists the \
                 sources this deployment knows, with their declared (not verified) coverage \
                 (req/824 A4)"
            ),
        )),
    }
}
