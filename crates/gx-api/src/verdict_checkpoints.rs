// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P3 / FR-M04** — the verdict checkpoint over HTTP (`req/119` §4).
//!
//! 32 §D's tail, verbatim: "**the surface currently has zero endpoints** — `Engine::verdict_checkpoint` was only placed as an API" (sem: SEM-gx-api-205). The
//! CLI half is `gx verdict-checkpoint` (`gx_cli::verdict`); this is the other half, and the pair is
//! what addendum 4's **R-SP** (surface parity) asks for (sem: SEM-gx-api-206): two thin projections of one engine call, never
//! two implementations of one decision.
//!
//! | method / path | what it is |
//! |---|---|
//! | `POST /v1/verdict-checkpoints` | close the window, sign the counts, append. **201** + the checkpoint |
//! | `GET /v1/verdict-checkpoints` | the chain, paged in 44 §2.7's shape |
//! | `GET /v1/verdict-checkpoints/{window_end}` | one checkpoint, by the coordinate it closes at |
//!
//! # 🔴 Three things this surface deliberately does not do
//!
//! 1. **It does not issue on a timer.** Ruling ⑥ (`req/38` §71): "checkpoint issuance defaults to manual (an in-serve
//!    timer for automatic issuance needs a Rule 2 single-point design — reserved for v0.2)" (sem: SEM-gx-api-207). A `POST` is somebody deciding.
//! 2. **It does not verify.** `audit_verdict_chain` is a **verifier's** function and a verifier is
//!    not this server — "an operator who wants to under-report holds the key and will sign the
//!    smaller number" (sem: SEM-gx-api-208) (`gx_log::proof`). A `/verify` endpoint served by the party being audited
//!    would be a check marking its own paper. `gx verdict-checkpoint verify` runs where the reader
//!    is, on documents this endpoint hands out.
//! 3. **It signs with the server's key**, which is the same key `GET /ledger/checkpoint` uses and is
//!    45 §1's engine key rather than an actor's.
//!
//! # 🔴 The cursor is the chain's own position, and `window_end` will not do
//!
//! `crate::list` pages on the **journal** position (M6-13 = M6-05, one coordinate). A checkpoint has
//! no journal record, so that coordinate cannot name one. The obvious substitute is `window_end` and
//! it is **wrong**: two calls with no verdict between them produce a second checkpoint whose window
//! is *empty* (the engine's ruling — "a repeat would double every count"; sem: SEM-gx-api-209), and an empty window has
//! `window_start == window_end`. So several checkpoints can share a `window_end`, and a cursor built
//! on it skips or repeats them. This was measured rather than reasoned about: the first run of
//! `tests/verdict_checkpoints.rs` paged past two empty windows and answered an empty page.
//!
//! ∴ `?cursor=<n>` is the **index in the chain** this deployment published — 0-based, opaque to a
//! client, and total by construction. The deviation from `crate::list`'s coordinate is written here
//! rather than left for a reader to discover that two cursors in one API mean two things.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use gx_core::{Timestamp, VerdictCheckpoint};
use gx_engine::store::EngineJournalRecord;
use gx_engine::Engine;

use crate::extract::{Params, Payload, Segment};
use crate::list::{DEFAULT_LIMIT, MAX_LIMIT};
use crate::problem::ApiError;
use crate::state::{AppState, RequestEvidence};

/// The namespace a **verdict** chain is scoped to.
///
/// Spelled twice — here and in `gx_cli::verdict::DEFAULT_VERDICT_ORIGIN` — for the reason
/// [`crate::DEFAULT_ORIGIN`] is: the two crates cannot see each other, and
/// `crates/gx-cli/tests/verdict_checkpoint_surface.rs` compares the spellings the way `ac_055.rs`
/// compares the ledger's. A mismatch would make a chain issued through one surface fail to fold
/// with a chain issued through the other, which is exactly what an origin is for.
pub const DEFAULT_VERDICT_ORIGIN: &str = "glovrex-verdicts/v1";

/// The three routes this module adds, as a declaration something compares with the router.
pub const VERDICT_CHECKPOINT_ENDPOINTS: [&str; 3] = [
    "POST /verdict-checkpoints",
    "GET /verdict-checkpoints",
    "GET /verdict-checkpoints/{window_end}",
];

type Answer = Result<Response, ApiError>;

fn ok(status: StatusCode, body: serde_json::Value) -> Answer {
    Ok((status, axum::Json(body)).into_response())
}

/// 44 §2.7's query, with this module's own cursor.
#[derive(Debug, Default, Deserialize)]
pub struct Page {
    /// "default 50, maximum 200" (sem: SEM-gx-api-210).
    pub limit: Option<usize>,
    /// A position in the chain this server published. Exclusive: paging resumes **after** it.
    pub cursor: Option<usize>,
}

/// The body `POST /verdict-checkpoints` accepts. Every member is optional.
#[derive(Debug, Default, Deserialize)]
pub struct IssueBody {
    /// 42 §3.11's namespace. Defaults to [`DEFAULT_VERDICT_ORIGIN`].
    pub origin: Option<String>,
}

/// 🔴 **DR-44-9** (`req/38` §168 ruling 1, `req/187` §5's fourth filing) — the clock reading for a
/// window boundary, resolved from the journal.
///
/// # The coordinate, and the misreading it caused
///
/// `window_start` / `window_end` are **verdict sequence numbers** — 42 §3.14: "the first verdict
/// sequence number this checkpoint speaks for, inclusive" and "one past the last, exclusive". They
/// are counts, not clock readings and not ledger indices. `req/187` §5 recorded a GUI session
/// reading `window_end` as a time ("14:35:39" in its own sample data), deriving a range from it and
/// getting a **confidently wrong answer** — and then refusing to derive anything until this was
/// settled. §168 ruled the third option of the three it offered: put the time on the wire, keep the
/// index.
///
/// # The resolution, and why it is one predicate rather than two
///
/// A verdict's sequence number is its position among the journal records that *count as* a verdict,
/// and `gx_engine`'s `tally_from_the_journal` defines that set: a `Verdict` record (all three kinds
/// and T-4e's digest-less one) or a `HumanDecision` record (T-5 / T-5b). This function walks the
/// same set in the same order and collects each record's `at`. It is a **second spelling of one
/// definition**, which is the risk this design takes, and `tests/dr44_9_views.rs` measures the two
/// against each other **through the wire** (this function is private, so the suite cannot call it):
/// a window whose `tally` counts one verdict must resolve both of its boundaries to a non-`null`
/// time, and a second window must resolve to the **next** verdict's time and not to some other
/// record's. Under-counting here answers `null` where a time exists; over-counting lands the second
/// window on the wrong record. Either way the disagreement shows up in our own writer before it
/// shows up on somebody's screen.
///
/// # What it is not
///
/// Not signed, and not part of the document. The checkpoint's signed core is everything the
/// engine minted (42 §3.14, `timestamp` excepted for CM-5's reason); these two keys are the API
/// layer resolving a coordinate on the reader's behalf, exactly as 44 §0 makes the RFC 3339
/// conversion "the API layer's responsibility". A verifier folding the chain reconstructs the
/// signing bytes from the typed fields and cannot see them.
fn verdict_times(engine: &Engine<RequestEvidence>) -> Vec<Timestamp> {
    engine
        .journal()
        .records()
        .iter()
        .filter_map(|record| match record {
            EngineJournalRecord::Verdict { at, .. }
            | EngineJournalRecord::HumanDecision { at, .. } => Some(*at),
            _ => None,
        })
        .collect()
}

/// 🔴 **DR-44-9** — one checkpoint on the wire: the document, plus the two resolved boundaries.
///
/// `window_start_at` is the time of the **first** verdict the window speaks for (`window_start`,
/// inclusive) and `window_end_at` the time of the **last** one (`window_end - 1`, because the upper
/// bound is exclusive — 42 §3.14). Both are `null` for an **empty window**
/// (`window_start == window_end`, the shape a second `POST` with nothing between produces), and
/// `null` for a boundary this journal cannot resolve: a chain read back beside a journal that no
/// longer reaches that far names a sequence number with no record, and a `null` says so rather than
/// nominating the nearest record's clock.
fn checkpoint_json(times: &[Timestamp], checkpoint: &VerdictCheckpoint) -> serde_json::Value {
    let at_of = |seq: u64| -> serde_json::Value {
        usize::try_from(seq)
            .ok()
            .and_then(|seq| times.get(seq))
            .map_or(serde_json::Value::Null, |at| crate::rfc3339::of(*at).into())
    };
    let mut value = serde_json::to_value(checkpoint).unwrap_or(serde_json::Value::Null);
    if let Some(map) = value.as_object_mut() {
        let empty = checkpoint.window_end <= checkpoint.window_start;
        map.insert(
            "window_start_at".to_string(),
            if empty {
                serde_json::Value::Null
            } else {
                at_of(checkpoint.window_start)
            },
        );
        map.insert(
            "window_end_at".to_string(),
            if empty {
                serde_json::Value::Null
            } else {
                at_of(checkpoint.window_end - 1)
            },
        );
    }
    value
}

/// 🔴 Close the window and publish the counts (**FR-M04**, 43-adjacent but not a transition).
///
/// **201**, because a checkpoint is a document this call created — and it is appended to a chain,
/// so a second `POST` with no verdicts between the two produces a second checkpoint whose window is
/// **empty** rather than a repeat. The engine's own doc comment carries that reasoning ( "a verifier
/// folds the chain and a repeat would double every count in it" (sem: SEM-gx-api-211) ), and it is why this endpoint is
/// not idempotent in the way `POST /candidates/{id}/commit` is.
///
/// # Errors
/// [`ApiError`] when the core cannot be signed or the chain cannot be appended.
pub async fn issue(State(state): State<AppState>, body: Option<Payload<IssueBody>>) -> Answer {
    let at = state.now();
    // 🔴 H-11 ③ (`req/189`): `Option<axum::Json<_>>` until v0.4-l — the one body on this surface
    // that answered malformed JSON in axum's `text/plain` instead of 44 §2.3's problem+json.
    let origin = body
        .and_then(|Payload(body)| body.origin)
        .unwrap_or_else(|| DEFAULT_VERDICT_ORIGIN.to_string());
    let (signed, times) = {
        let key = state.keys().signing();
        let mut engine = state.engine();
        let signed = engine
            .verdict_checkpoint(&origin, at, key)
            .map_err(|e| ApiError::from_engine(&e))?;
        // Read under the same lock that minted it: a window resolved against a journal some other
        // request had already extended would name later records than the ones it counted.
        let times = verdict_times(&engine);
        (signed, times)
    };
    ok(StatusCode::CREATED, checkpoint_json(&times, &signed))
}

/// The chain, paged (44 §2.7's envelope).
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for a limit outside 44 §2.7's range — refused rather than
/// clamped, for `crate::list::Page::limit`'s reason.
pub async fn list(State(state): State<AppState>, Params(page): Params<Page>) -> Answer {
    let limit = match page.limit {
        None => DEFAULT_LIMIT,
        Some(n) if (1..=MAX_LIMIT).contains(&n) => n,
        Some(n) => {
            return Err(ApiError::validation(format!(
            "`limit={n}` is outside 44 §2.7's range (1..={MAX_LIMIT}, default {DEFAULT_LIMIT}). \
                 A clamped limit would be a page that looks complete and is not"
        )))
        }
    };
    let engine = state.engine();
    let times = verdict_times(&engine);
    let chain = engine.verdict_checkpoints();
    let mut items = Vec::new();
    let mut next_cursor = None;
    for (position, checkpoint) in chain.iter().enumerate() {
        if let Some(after) = page.cursor {
            if position <= after {
                continue;
            }
        }
        if items.len() == limit {
            // The position of the **last row returned**, so that a client hands back what it has
            // seen rather than what it is about to see.
            next_cursor = Some(position - 1);
            break;
        }
        items.push(checkpoint_json(&times, checkpoint));
    }
    ok(
        StatusCode::OK,
        serde_json::json!({
            "items": items,
            "next_cursor": next_cursor,
            // The count a reader needs in order to know whether the chain they are folding is the
            // whole of what this deployment has published.
            "total": chain.len(),
        }),
    )
}

/// One checkpoint, by the `window_end` it closes at.
///
/// 🔴 The **first** that closes there. Empty windows share a `window_end` with the checkpoint before
/// them (`window_start == window_end`), so this coordinate is not a key — `GET
/// /v1/verdict-checkpoints` is where a reader sees the chain in order, and this endpoint is the
/// convenience for the ordinary case where a window has something in it. Said here rather than left
/// for a client to discover that two documents answer to one path.
///
/// # Errors
/// [`ApiError`] `NOT_FOUND` when no checkpoint closes there. The refusal names the coordinate rather
/// than answering with the nearest one: a chain folded out of near-misses is not a chain.
pub async fn get(State(state): State<AppState>, Segment(window_end): Segment<u64>) -> Answer {
    let engine = state.engine();
    let times = verdict_times(&engine);
    let found = engine
        .verdict_checkpoints()
        .iter()
        .find(|checkpoint| checkpoint.window_end == window_end)
        .cloned();
    match found {
        Some(checkpoint) => ok(StatusCode::OK, checkpoint_json(&times, &checkpoint)),
        None => Err(ApiError::not_found(format!(
            "no verdict checkpoint closes at {window_end}; `GET /v1/verdict-checkpoints` lists the \
             windows this deployment has published"
        ))),
    }
}
