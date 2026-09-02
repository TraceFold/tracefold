// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 44 §2.2's endpoints — the thirteen that are not `GET /stream`.
//!
//! # 🔴 Every handler is the same shape, and that is Rule 1 (req/88 §3 Λ1; sem: SEM-gx-api-090)
//!
//! Parse the request, take the engine's lock, call one of 43's eight entry points, read accessors,
//! serialise. No canonical encode (41 §6), no `Verdict` constructed (41 §4), no `Lifecycle` written
//! (42 §1.3-3). `crates/gx-canon/tests/authority_boundary.rs` scans this directory for all three,
//! and req/88 §3 Λ1 says why the temptation is sharper here than in the CLI: "an HTTP handler is
//! where 'just compute the answer here, it is faster' is most tempting" (sem: SEM-gx-api-091) — `GET /candidates/{id}`
//! returns four fields and every one of them has an engine accessor already.
//!
//! # 🔴 The three asymmetries with 44 §1, written down rather than discovered
//!
//! 1. **`POST /candidates` is `submit` + `plan` in one call.** 44 §2.1 says so — "internally it transitions Draft→
//!    Candidate atomically; the Draft-alone state is not observable at the HTTP layer" (sem: SEM-gx-api-092) — and §0 exempts it from the
//!    id-resolution rule for that reason. This is why the server needs no `.gx/drafts/`: req/88 §3
//!    Λ2's counter-example is the CLI's alone.
//! 2. **Receipts are signed by the server's key, not the actor's.** See [`crate::state`]'s header
//!    (E-M6-7). AC-055 does not measure the key and the difference is deliberate.
//! 3. **`goal` is bytes here as it is in the CLI, and 44 §2.2 types it as an object.** See
//!    [`CreateCandidate`].
//!
//! # 🔴 What 44 §2.2 asks for that this hand answers differently, and why
//!
//! `POST /candidates/{id}/verify` is specified as `202 Accepted` with "asynchronous. The result is … polled for,
//! or through `GET /stream`", and ASM-44-2 permits the other reading in as many words: "an implementation that gets
//! synchronous completion in a short time may return `200` + the final `state`/`verdict` immediately (the client
//! has to be able to handle both response codes)" (sem: SEM-gx-api-093). This surface is synchronous because M6-06, adopted (a), made every request
//! hold one lock — there is no second thread for a 202 to be answered *by*. Returning 202 and then
//! doing the work anyway would be a status describing a concurrency this build does not have.

use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use gx_core::{
    Actor, ChangeContext, GoalBytes, Intent, SubstrateKind, Timestamp, TransformationId,
    VerdictKind,
};
use gx_engine::store::InverseStatus;
use gx_engine::{reconstruct, HumanRuling, Lifecycle};
use serde::Deserialize;

use crate::extract::{Params, Payload, Segment};
use crate::problem::{ApiError, RollbackFacts};
use crate::state::{AppState, RequestEvidence};
use crate::{rfc3339, ReceiptSlot};

/// The answer type every handler returns.
type Answer = Result<Response, ApiError>;

// ---------------------------------------------------------------------------
// Shared readings
// ---------------------------------------------------------------------------

/// 42 §1.2's `gx1:<base32>`, parsed and never minted (Rule 1 (i); sem: SEM-gx-api-094).
fn transformation_id(text: &str) -> Result<TransformationId, ApiError> {
    gx_core::Cid::from_text(text)
        .map(TransformationId)
        .map_err(|e| ApiError::validation(format!("`{text}` is not a `gx1:` id: {e}")))
}

/// 42 §3.1's substrates, in 44 §2.2's spelling (`"fs"|"git"|"mcp"`).
fn substrate_kind(text: &str) -> Result<SubstrateKind, ApiError> {
    match text {
        "fs" => Ok(SubstrateKind::Fs),
        "git" => Ok(SubstrateKind::Git),
        "mcp" => Ok(SubstrateKind::Mcp),
        other => other
            .strip_prefix("custom:")
            .map(|name| SubstrateKind::Custom(name.to_string()))
            .ok_or_else(|| {
                ApiError::validation(format!(
                    "`substrate` takes fs|git|mcp (44 §2.2) or `custom:<NAME>` (42 §3.1); got \
                     {other:?}"
                ))
            }),
    }
}

/// 43 T-9's critical section, as a named predicate.
///
/// A function rather than a `matches!` at the call site for the reason
/// `gx_cli::pipeline::is_committing` records: Rule 1 (iii)'s scanner reads `Lifecycle::` to the right
/// of an `=` as "the surface mints a state" (sem: SEM-gx-api-095), and it is right to be coarse.
fn is_committed(state: Lifecycle) -> bool {
    matches!(state, Lifecycle::Committed)
}

/// Whether a state is one the gate has answered about.
fn is_admitted(state: Lifecycle) -> bool {
    matches!(state, Lifecycle::Admitted)
}

/// The one refusal DR-2's record-only road is open for (43 T-8r), as a named predicate.
///
/// Named for the reason [`is_committed`] gives, and **narrow** for a second one: T-8r opens
/// `Denied` and nothing else. An `Escalated` inverse is waiting on a person and a record-only
/// posture does not answer for them (INV-S6: "`Escalated` does not auto-transition to `Admitted`/`Denied`
/// without going through T-5/T-5b's signed human-ruling receipt"; sem: SEM-gx-api-096), so widening this to "anything that is not
/// `Admitted`" would turn a pending human decision into an applied change.
fn is_denied(state: Lifecycle) -> bool {
    matches!(state, Lifecycle::Denied)
}

/// 🔴 **DR-43-6 (`req/38` §153) / `req/215` M-01 and H-04** -- the honest refusal for a row this
/// process can *see* and holds no *body* for.
///
/// The Σ-shadow knows every row the journal holds; the table holds bodies only for rows this process
/// planned itself (`Engine::open` leaves it empty, and 42 §3.13 records names and digests rather
/// than bodies). T6-① told app layers they may depend on `GET` answering non-null after a restart --
/// and then `req/215` M-01 measured what happens when they act on one of those rows: five of the six
/// write handlers answered a bare `404 NOT_FOUND` reading "no transformation named
/// `TransformationId(Cid(opaque))`", about a row `GET /v1/candidates/{id}` had just answered `200`
/// for. Two answers, one row, and the `404` is the one the E2E suite already calls a false answer.
///
/// `undo` was the one handler that did not do this (`pipeline.rs`'s `undo` falls through to
/// `shadow.row`), and this is that refusal generalised to its four siblings: `409 INVALID_STATE`,
/// the state named, and the reason named as *the missing body* rather than as a missing row. The id
/// is spelled with `to_text` and not `{:?}` -- the engine-side refusals still print
/// `TransformationId(Cid(opaque))` (`req/215` M-01's second half), which is a `pipeline.rs` change
/// and `pipeline.rs`'s undo path belongs to the DR-43-1 lane this week.
///
/// `None` means "not this condition": either the process holds the body (proceed) or nothing in the
/// journal has ever named the id (the engine's own `NOT_FOUND` is then the true answer).
fn without_a_body(
    holds_body: bool,
    // The shadow row's state *name* (`Lifecycle::name`), never the enum: gx-canon's
    // authority_boundary Rule 1(iii) scanner reads a `Lifecycle`-typed binding here as a second
    // state table (req/38 §158). The handler only prints it.
    shadow_state_name: Option<&'static str>,
    id: &TransformationId,
) -> Option<ApiError> {
    if holds_body {
        return None;
    }
    let state_name = shadow_state_name?;
    Some(ApiError::new(
        "INVALID_STATE",
        "the operation was refused",
        format!(
            "{} is {} in this project's journal, this process holds no body for it, and the draft \
             archive this server was given holds no intent it can be rebuilt from, so it cannot be \
             written to. A body is rebuilt from `.gx/drafts/` (DR-43-2 lane R2, req/190 §4-1 L2, \
             `gx_api::DraftArchive`); a deployment running `NoDrafts`, or one whose draft was \
             discarded, keeps R1's answer — the row another `gx` process planned is readable here \
             and not writable here (req/215 M-01, req/38 §153)",
            id.0.to_text(),
            state_name,
        ),
    ))
}

/// 🔴 **T6 condition ① L2 — put the body back before a write, or refuse by name** (`req/38` §148
/// ruling 1(iii), lane R2).
///
/// The one road every write handler takes into a row it may not hold. `Some(error)` means stop.
///
/// # The three cases, and none of them is new judgement
///
/// * **the process holds the body** — nothing happens, at the cost of one `BTreeMap` lookup. This
///   is the overwhelmingly common case and it is why the rebuild is not attempted eagerly.
/// * **the journal names the row and the archive holds its intent** — the body is rebuilt, by the
///   two roads gx-cli has had since M6 and for gx-cli's reasons: a row before the commit is
///   re-`plan`ned (43 T-2's idempotency column — "re-running against the same snapshot yields the
///   same `PlannedDelta` and the same `TransformationId`"), and a `Committed`/`Superseded` row goes
///   through [`gx_engine::Engine::rehydrate_committed`], because a re-plan reads a substrate the
///   commit has already moved. `gx_cli::session::Session`'s `resume` / `rehydrate_committed` pair
///   is the same split; what is new is only that the intent arrives through a trait rather than
///   from a directory this crate may not name.
/// * **neither** — [`without_a_body`]'s refusal, unchanged.
///
/// # 🔴 What it deliberately does not do
///
/// It does not rebuild a row in a **terminal** state that is not `Committed`/`Superseded`
/// (`Aborted`, `Denied`): 43 offers no transition out of those, so a rebuilt body would be a body
/// built in order to be refused one line later, and the refusal naming the state is the truer
/// answer. It does not touch a row this journal has never named either — the engine's own
/// `NOT_FOUND` is correct there, and a rebuild would hide it.
fn with_a_body(
    state: &AppState,
    engine: &mut gx_engine::Engine<RequestEvidence>,
    id: &TransformationId,
    at: Timestamp,
) -> Option<ApiError> {
    if engine.transformation(id).is_some() {
        return None;
    }
    // The state **name** and never the value, for `without_a_body`'s own reason (`req/38` §158):
    // gx-canon's authority_boundary Rule 1(iii) scanner reads a `Lifecycle`-typed binding in this
    // crate as a second state table. Read here rather than after the rebuild, because a successful
    // rebuild is exactly the case where this is not needed.
    let shadow_state_name = engine.state(id).map(|st| st.name());
    match rebuilt(state, engine, id, at) {
        Ok(true) => None,
        Ok(false) => without_a_body(false, shadow_state_name, id),
        Err(e) => Some(e),
    }
}

/// [`with_a_body`]'s middle case, as a result rather than as a refusal.
///
/// `Ok(false)` means "there was nothing to rebuild it from" and is deliberately not an error: the
/// caller turns it into the refusal that names the state, which says more than any message this
/// function could.
///
/// # Errors
/// Whatever the engine refuses the rebuild with, and `409 INVALID_STATE` when a re-plan names
/// another transformation.
fn rebuilt(
    state: &AppState,
    engine: &mut gx_engine::Engine<RequestEvidence>,
    id: &TransformationId,
    at: Timestamp,
) -> Result<bool, ApiError> {
    let Some(lifecycle) = engine.state(id) else {
        return Ok(false);
    };
    let Some(intent_id) = engine.intent_of(id) else {
        return Ok(false);
    };
    let Some(intent) = state.drafts().load(&intent_id) else {
        return Ok(false);
    };
    match lifecycle {
        Lifecycle::Committed | Lifecycle::Superseded => engine
            .rehydrate_committed(id, &intent)
            .map_err(|e| ApiError::from_engine(&e)),
        terminal if terminal.is_terminal() => Ok(false),
        _ => {
            // 🔴 A re-plan that names another transformation is **refused** rather than followed.
            // `Session::resume` reaches the same wall and answers the same way: 43 §8 forces a
            // re-plan once `Fingerprint₀` has gone stale, and a surface that quietly wrote to the
            // transformation the re-plan named would be acting on an id the caller never sent.
            //
            // 🔴 **R3 / `req/222` H-03** — the question is asked **before** the engine is allowed
            // to write, and the last sentence of the refusal is now true.
            //
            // This called `engine.plan(&intent, at)` and compared afterwards. On the stale road
            // that is one journal record too late: `plan` mints a *different* `TransformationId`,
            // finds `rehydrating` false, and appends a `Planned` for it. `req/222` H-03 measured
            // the whole consequence — the caller got `409` reading "Nothing was written to either
            // row" while the journal went from 1 row to 2, and the row that had grown answered
            // `GET` 200, `verify` 200 and `commit` 200, overwriting a third party's file. No CAS
            // stands anywhere on that road: DR-43-1's is on `undo` alone. A refused request that
            // leaves behind a fresh, committable claim on somebody else's substrate is worse than
            // the refusal it was pretending to be.
            //
            // [`gx_engine::Engine::planned_id`] answers the same question out of 41 §4's read-only
            // three, and it is the same code `plan` runs. Cost: those three run twice on the
            // rebuild road. That is the price of an honest refusal.
            let planned = engine
                .planned_id(&intent, at)
                .map_err(|e| ApiError::from_engine(&e))?;
            if planned != *id {
                return Err(ApiError::invalid_state(format!(
                    "{} was planned against a state of the substrate that no longer holds; \
                     planning it now names {} instead (43 §8: `Fingerprint₀` has gone stale, so a \
                     re-plan is forced). Nothing was written to either row",
                    id.0.to_text(),
                    planned.0.to_text(),
                )));
            }
            // The identity holds, so the re-plan is 43 T-2's idempotent one: `plan` finds
            // `recorded == Some(id)` with the row absent from the table, takes its rehydrating
            // branch, and seats the body **without** appending a second `Planned`.
            let replanned = engine
                .plan(&intent, at)
                .map_err(|e| ApiError::from_engine(&e))?;
            if replanned != *id {
                // Unreachable by construction: the two answers come from one function. Answered
                // rather than asserted because 41 §6 counts a panic as a bug, and a surface that
                // reached this line would be one where the engine had two definitions of identity.
                return Err(ApiError::internal(format!(
                    "rebuilding {} named {} after {} was measured read-only; the engine has two \
                     answers for one identity (R3, req/222 H-03)",
                    id.0.to_text(),
                    replanned.0.to_text(),
                    planned.0.to_text(),
                )));
            }
            Ok(true)
        }
    }
}

/// 42 §3.5's fingerprint, as the JSON `FingerprintRecord` already produces.
fn fingerprint_json(state: &AppState, id: &TransformationId) -> serde_json::Value {
    let engine = state.engine();
    engine
        .precondition_fingerprint(id)
        .map(gx_engine::store::FingerprintRecord::of)
        .map_or(serde_json::Value::Null, |f| {
            serde_json::to_value(f).unwrap_or(serde_json::Value::Null)
        })
}

/// 🔴 42 §3.8's `AdmitProof` through its accessors (**M6H3-2**), for a problem `detail` and a body.
///
/// The type has no `Serialize` on purpose (gx-gate: "Deriving `Deserialize` in particular would open
/// a second door into the struct that skips `AdmitProof::new`"; sem: SEM-gx-api-097), so the five public accessors are
/// read. Field names are 42 §3.8's.
fn admit_proof_json(proof: Option<&gx_gate::AdmitProof>) -> serde_json::Value {
    let Some(proof) = proof else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "policy_decisions": proof.policy_decisions(),
        "invariant_results": proof.invariant_results(),
        "evidence_digests": proof.evidence_digests().iter().map(gx_core::Cid::to_text).collect::<Vec<_>>(),
        "composed_from": proof.composed_from().iter().map(|t| t.0.to_text()).collect::<Vec<_>>(),
        "proof_ref": proof.proof_ref(),
    })
}

/// A JSON body and a status, as a response.
fn ok(status: StatusCode, body: serde_json::Value) -> Answer {
    Ok((status, axum::Json(body)).into_response())
}

/// 44 §2.4's header.
fn idempotency_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// The two fallbacks — 44 §2.3 reaching the requests no handler was asked about (H-11 ①, req/189)
// ---------------------------------------------------------------------------

/// 🔴 An unrouted path, in 44 §2.3's shape (`404 NOT_FOUND`).
///
/// axum's own fallback is an empty `404`; `req/182` H-11 counted it as one of four response
/// writers outside the problem+json contract, and a client whose error display keys on `gx_code`
/// (48's console, the SDK's `GxApiError`) met prose. Mounted by [`crate::router`] on the outermost
/// router, **outside** the Bearer guard — a path that matched nothing has no route for the guard
/// to sit on, and the status was already `404` for an anonymous caller before this handler; only
/// the body changed. It names the path (what the caller sent) and not the routing table.
pub async fn not_found(method: Method, uri: Uri) -> ApiError {
    ApiError::not_found(format!(
        "{method} {} is not a route of this server (44 §2.1 lists them; base path {})",
        uri.path(),
        crate::BASE_PATH
    ))
}

/// 🔴 A routed path asked with a method it has no handler for, in 44 §2.3's shape.
///
/// axum answers `405` with an empty body; this answers the same status with a problem body.
/// The `gx_code` is `VALIDATION_ERROR` — 44 §2.3's "malformed request" is the closest word it has,
/// and adding a thirteenth code for a mistake no client makes on purpose would be a row nobody
/// asked for; the **status** stays 405 because RFC 9110 §15.5.6 makes it a statement about the
/// method, and [`ApiError::with_status`] is how a code carries a status its table row does not.
pub async fn method_not_allowed(method: Method, uri: Uri) -> ApiError {
    ApiError::validation(format!(
        "{method} is not a method {} answers (44 §2.1 gives each route its methods)",
        uri.path()
    ))
    .with_status(StatusCode::METHOD_NOT_ALLOWED.as_u16())
}

// ---------------------------------------------------------------------------
// `GET /healthz` — 44 §2.2, the one endpoint outside the Bearer guard
// ---------------------------------------------------------------------------

/// `GET /healthz` → `{ "status": "ok", "engine_version": "<string>" }`, unauthenticated (44 §2.6).
///
/// 🔴 **M6H5-12, adopted (a)**, implemented in hand 7 (§52: "the version accessor belongs to the hand that ships the artefact"; sem: SEM-gx-api-098).
///
/// Hand 5 answered this with `env!("CARGO_PKG_VERSION")` — **this crate's** version — and raised the
/// borrow. 41 §2 keeps gx-engine and gx-api at one version in one workspace, so the two strings are
/// equal today, which is exactly why the borrow was invisible. The field is named `engine_version`,
/// so the engine answers it: [`gx_engine::Engine::version`].
///
/// Why it was worth a hand rather than left as a note: 47 §4's upgrade runbook makes "the journal schema
/// must replay deterministically and identically between the old and new binary via `gx replay`" (sem: SEM-gx-api-099) an operator's **pre-upgrade
/// check**, and an operator performs that check against a version number read off a running server.
/// A number reported by the wrong crate is a check performed against the wrong thing — the day the
/// two crates version apart, and not before.
/// # 🔴 **DR-44-6 (`req/38` §156 ruling 2(b))** — it answers about the **project**, not only about
/// the process
///
/// Until this ruling this handler took no state at all, and `req/219` §5(a) declared what that
/// cost in one sentence: *a health probe cannot tell a server that would refuse to start from a
/// server whose disk broke while it ran*. R1b made both writes and `GET /ledger/checkpoint` refuse
/// on `ledger_agrees == false`, so the server no longer signs over a tree it contradicts — and it
/// went on answering `{"status":"ok"}` to every monitor while doing it. A project that cannot be
/// written to is not healthy, and an orchestrator's whole job is to notice.
///
/// So the shape grows by one member and gains a second status:
///
/// * `200 { status: "ok", engine_version, ledger_agrees: true, journal_rows }` — the ordinary case;
/// * `500 LEDGER_DISAGREES` (problem+json, the same code and the same sentence the write path
///   answers) when the two files describe different trees.
///
/// # 🔴 What it does not become
///
/// **Not a deep check.** It calls [`AppState::engine_refreshed`], which is the lockless catch-up:
/// a `metadata()` on two files and a fold of whatever bytes arrived. It does not take `.gx/LOCK`
/// (a health probe that answered `503` while a `gx commit` ran would report a healthy project as
/// sick — R1's judgement, unchanged), it does not `recover`, and it does not sweep. A monitor
/// hitting this every second costs two `stat` calls and the tail of a journal it has usually
/// already read.
///
/// **Not authenticated, and therefore deliberately thin.** 44 §2.5 keeps `/healthz` outside the
/// Bearer guard, so every member here is readable by anyone who can reach the socket. `ledger_agrees`
/// is a boolean about consistency and `journal_rows` is a count; neither names a transformation, an
/// actor, a locator or a key. The `detail` of the `500` names two counts for the same reason the
/// write path's does — an operator reading it has to be able to tell which file to look at — and
/// that is the one place this endpoint says more than a boolean.
/// 🔴 **R11 / `req/240` M-04, shared since L-02 (`req/38` §369 item 1)** — the one sentence a
/// project whose writer's door is shut owes whoever is reading it.
///
/// # Why this is a function and not two `format!` calls
///
/// It was written once, inside [`healthz`]. L-02 puts the same fact on `GET /receipts/{tid}`
/// ([`server_health`]), and a second copy of these words is the shape `req/38` §227 keeps naming:
/// one question answered at two sites, drifting apart at one of them. The bytes are unchanged from
/// what R11 left, so every test that reads `/healthz`'s `status_reason` reads the same characters
/// it did before this lane.
pub(crate) fn degraded_reason(state: &AppState) -> Option<String> {
    state
        .meta_missing()
        .map(|path| format!(
            "`{path}` is not there, so this server refuses every write until it is back              (`DECLARATION_ABSENT`/`CONFIG_ABSENT`, the same refusal the CLI gives). Reads are              unaffected and the ledger and the journal still describe one tree. `gx repair`              reports the project; `gx repair --yes` writes the file back and says that it did              (req/240 M-04)"
        ))
}

pub async fn healthz(State(state): State<AppState>) -> Answer {
    // 🔴 **R11 / `req/240` M-01** — the four facts below, taken under the engine's `Mutex` at most
    // once per [`crate::state::HEALTH_SNAPSHOT_MAX_AGE`] and read outside it in between. See
    // `AppState::health_snapshot` for the measurement that moved them and for what the window is.
    let snapshot = state.health_snapshot()?;
    let agrees = snapshot.agrees;
    let rows = snapshot.rows;
    // 🔴 **R10 / `req/238` M-06** — the frontier's **length**, not a rebuilt Σ.
    //
    // This line read `engine.sigma().ledger().len()`, and `sigma()` reconstructs the whole of Σ —
    // four vectors allocated, every state row copied, the escrow view's two maps merged, all four
    // sorted — to answer one `usize`. `req/238` M-06 measured what that costs on the one endpoint
    // 44 §2.5 leaves outside the bearer guard: 1.39 ms at 5 commits, 3.67 ms at 100, 10.29 ms at
    // 400, linear, unauthenticated. `Engine::committed_len` is the same number off the same map
    // (`Sigma`'s ledger component is built from it) and is O(1). Nothing else in this handler
    // builds Σ: `shadow().len()` and `ledger().log().len()` were already `len()` calls.
    //
    // 🔴 **What this does *not* make O(1), measured rather than assumed.** After the change the
    // median is 1.58 ms at 5 commits and **8.93 ms** at 400 (was 11.92) — still growing. R10
    // attributed the rest by building two throwaway binaries: skipping `ledger_agrees()` moves 400
    // commits from 8.93 to 8.50 ms (≈0.4 ms), and skipping `AppState::engine_refreshed`'s
    // `Engine::catch_up_unlocked` moves it to **1.66 ms** — flat against the project. ∴ the
    // remaining cost is the lockless catch-up, which is R4 (`req/225` H-03)'s **detector**: it
    // re-reads to notice a journal or ledger that was rewritten under a running server, and that
    // is the read whose absence the audit found. Removing it is not a repair, and caching its
    // answer on an endpoint whose whole job is to notice a disk that changed would be a monitor
    // reporting the last time it looked. Declared here and in 43 §7.12 (e) rather than traded away.
    let (journal, ledger) = (snapshot.journal, snapshot.ledger);
    // 🔴 **R4 / `req/225` H-03** — a lockless read asks the journal's tail record as well as its
    // length, so a same-length rewrite of the last record reaches a monitor rather than waiting
    // for the next start-up to lose the file. What a read still cannot see is a rewrite in the
    // **middle** of the journal: that one is caught by `Engine::catch_up`'s full-prefix replay,
    // under the lock, on the next write. Same shape and same declared limit as 43 §7.5 (j) gives
    // for the ledger.
    let journal_departure = snapshot.journal_departure;
    // 🔴 **R6 / DR-43-11** — and the third condition, read before the guard is dropped.
    // 🔴 **R7 / `req/232` H-01** — and the fourth: a recorded head this binary refused to read
    // numbers off. One sentence carries whichever of the two applies, because both mean "this
    // project's statement about its own past is not usable" and a monitor should read one word.
    let rolled_back = snapshot.rolled_back.clone();
    if !agrees {
        // 🔴 **R6** — the two clauses are joined before the outer `format!`, not inside its
        // arguments: `clippy::format_in_format_args` is right that a nested one is a second
        // allocation nobody asked for, and the join has to happen somewhere.
        // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated, and one sentence per
        // condition rather than one paragraph for seven. See `crate::journal_and_head_note`.
        let note = crate::journal_and_head_note(journal_departure, rolled_back.as_deref());
        return Err(ApiError::ledger_disagrees(format!(
            "this project's journal witnesses {journal} commit(s) and its ledger holds {ledger} \
             leaf/leaves, and `ledger_agrees` is false: the two files are describing different \
             trees.{} This server refuses every write and signs no checkpoint while that holds, so \
             it reports itself unhealthy rather than let a monitor read `ok` (req/38 §156 ruling \
             2(b), DR-44-6). `gx repair` reports what is wrong and `gx repair --yes` runs 43 §7's recovery under the project lock (DR-43-8); `gx replay <ID>` names the rows that differ",
            note
        )));
    }
    let reason = degraded_reason(&state);
    ok(
        StatusCode::OK,
        serde_json::json!({
            "status": if reason.is_some() { "degraded" } else { "ok" },
            "engine_version": gx_engine::Engine::<crate::state::RequestEvidence>::version(),
            // 🔴 DR-44-6's two additions. `ledger_agrees` is the gate every writer passes, said
            // out loud; `journal_rows` is the Σ-shadow's row count, which is what makes "this
            // server has caught up with the disk" observable to a monitor that never writes.
            "ledger_agrees": agrees,
            "journal_rows": rows,
            // 🔴 **R11 / `req/240` M-04** — `"ok"` or `"degraded"`, and why.
            //
            // The audit deleted `.gx/VERSION` under a running server and watched this endpoint go
            // on answering `{"status":"ok"}` word for word while every CLI verb on the same
            // project refused `DECLARATION_ABSENT`. The status is **200**: the server is up, the
            // two files still describe one tree, and every read this surface offers still works —
            // what has changed is that the writer's door is shut, which is what `reason` says.
            // `LEDGER_DISAGREES` keeps its 500 above, because there the ledger itself is the
            // thing that cannot be trusted.
            "status_reason": reason,
        }),
    )
}

// ---------------------------------------------------------------------------
// `POST /candidates` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's request body for `POST /candidates`, seven fields.
#[derive(Debug, Deserialize)]
pub struct CreateCandidate {
    /// `"fs"|"git"|"mcp"` → `Intent.substrate`.
    pub substrate: String,
    /// → `Intent.locator`.
    pub locator: String,
    /// 🔴 44 §2.2 types this "an object (adapter-defined)" and **E-M6-11** rules the CLI's `--intent` body
    /// "a byte sequence the adapter interprets" (sem: SEM-gx-api-100) (P-6: interpretation belongs to the adapter).
    ///
    /// The two cannot both be satisfied by an object. 42 §3.3's `GoalBytes` is opaque and is **in
    /// the identity** — `IntentId` is the CID of the intent — so a surface that re-serialised a JSON
    /// object into bytes would give the same logical intent a different id depending on which
    /// surface received it, and the fs adapter, whose goal bytes *are* the file's new content, could
    /// not be driven over HTTP at all.
    ///
    /// So a JSON **string** is taken as its UTF-8 bytes and anything else is compact JSON. A string
    /// is therefore the spelling that makes `POST /candidates` and `gx submit --intent <FILE>` name
    /// the same transformation, which is what AC-055 measures. Raised as **M6H5-7**.
    pub goal: serde_json::Value,
    /// 42 §3.2's `ChangeContext`.
    pub context: ChangeContext,
    /// 42 §3.2's `Actor`.
    pub actor: Actor,
    /// 44 §2.2: "default 0" (sem: SEM-gx-api-101). v0.1 produces only 0 — see [`create_candidate`].
    #[serde(default)]
    pub order: u8,
    /// 44 §2.2: "default `[]`" (sem: SEM-gx-api-102).
    #[serde(default)]
    pub parents: Vec<String>,
}

impl CreateCandidate {
    /// The bytes 42 §3.3's `GoalBytes` carries. See the field's own note.
    fn goal_bytes(&self) -> Vec<u8> {
        match &self.goal {
            serde_json::Value::String(text) => text.as_bytes().to_vec(),
            other => serde_json::to_vec(other).unwrap_or_default(),
        }
    }
}

/// 🔴 `POST /candidates` (44 §2.2) — T-1 and T-2 in one call, which is 44 §2.1's own requirement.
///
/// > intent → candidate (runs the equivalent of submit+plan in one call. Internally it transitions Draft→Candidate
/// > atomically; the Draft-alone state is not observable at the HTTP layer) (sem: SEM-gx-api-103)
///
/// The two calls happen under **one** hold of the engine lock, which is what makes "atomically" (sem: SEM-gx-api-104)
/// true rather than aspirational: a lock released between them would let another request observe a
/// `Draft`, and 44 §2.1 says that state is not observable here.
///
/// `order` and `parents` are **refused** rather than dropped when non-default, for M6H3-5's reason
/// one surface along: 42 §3.3's `Intent` has no room for either and `plan` fixes `order = 0`, so a
/// value passed here would reach nothing. "an argument that changes nothing must not look like an
/// argument that worked" (sem: SEM-gx-api-105) (M4H5-5).
///
/// # Errors
/// `422 VALIDATION_ERROR` for a body 42 §3.3 cannot hold, `502 ADAPTER_ERROR` for a snapshot or plan
/// the adapter refused (44 §2.2 names both).
pub async fn create_candidate(
    State(state): State<AppState>,
    Payload(body): Payload<CreateCandidate>,
) -> Answer {
    if body.order != 0 {
        return Err(ApiError::validation(format!(
            "`order` {} has nowhere to go: 42 §3.3's `Intent` carries no order and `plan` fixes it \
             at 0 for every transformation of an object (M6H3-5). v0.1 admits order <= 2 as a type \
             (ASM-6) and produces only 0",
            body.order
        )));
    }
    if !body.parents.is_empty() {
        return Err(ApiError::validation(
            "`parents` has nowhere to go: 43 T-12's guard is about `undo`, which is the one \
             producer of a non-empty parents list, and `Engine::submit` takes an `Intent` (42 §3.3, \
             five fields) (M6H3-5)",
        ));
    }
    let intent = Intent::new(
        substrate_kind(&body.substrate)?,
        body.locator.clone(),
        GoalBytes(body.goal_bytes()),
        body.context.clone(),
        body.actor.clone(),
    );

    let at = state.now();
    let seed = state.seed();
    let (transformation, transformation_json, fingerprint, lifecycle) = {
        // 🔴 One hold, two transitions. 44 §2.1's "atomically" (sem: SEM-gx-api-106).
        let mut engine = state.engine_for_write()?;
        let intent_id = engine
            .submit(&intent, seed, at)
            .map_err(|e| ApiError::from_engine(&e))?;
        // 🔴 **T6 condition ① L2 — the write that makes a restart survivable** (`req/38` §148
        // ruling 1(iii), lane R2). 42 §3.13's `DraftCreated` record carries `{intent_id, rng_seed,
        // at}` and no body (ASM-9), so this is the only moment at which the five fields 42 §3.3
        // fixes exist in a process that is about to write them down. Filed **before** `plan`,
        // because the failure being closed is a crash or a restart between the two — a
        // `Candidate` in the journal whose intent nothing holds is exactly `req/182` H-02's row.
        //
        // A failure here does **not** fail the request. The archive is a durability aid and not
        // part of 43's transition: a `POST /candidates` that answered `500` because a directory
        // was read-only would refuse a candidate the engine has already drafted, and the journal
        // would then hold a draft the caller was told did not happen. What it costs when it does
        // fail is exactly what a `NoDrafts` deployment costs — the row is readable after a restart
        // and not writable — and `without_a_body` says so by name.
        //
        // 🔴 **R16 / `req/262` H-01** — and the sentence that says so is a **value** now. It was an
        // `eprintln!`, which panics on a write error, so on the composite a full disk produces on
        // its own — a directory that will not take a file **and** a standard error that will not
        // take a line — this request ended with no HTTP status line at all (0 bytes, three runs,
        // against `201` with the same project and `2>/dev/null`). An answer is a write to a socket
        // and may not depend on the other stream. See [`crate::notes`].
        if let Err(why) = state.drafts().store(&intent_id, &intent) {
            crate::api_note!(
                "gx serve: the draft archive would not hold {}: {why}. The candidate stands; a row \
                 this process does not hold the body for cannot be written to after a restart \
                 (req/38 §148 lane R2)",
                intent_id.0.to_text()
            );
        }
        let id = engine
            .plan(&intent, at)
            .map_err(|e| ApiError::from_engine(&e))?;
        let json =
            serde_json::to_value(engine.transformation(&id)).unwrap_or(serde_json::Value::Null);
        let fp = engine
            .precondition_fingerprint(&id)
            .map(gx_engine::store::FingerprintRecord::of)
            .map_or(serde_json::Value::Null, |f| {
                serde_json::to_value(f).unwrap_or(serde_json::Value::Null)
            });
        let state_now = engine.state(&id);
        (id, json, fp, state_now)
    };

    ok(
        StatusCode::CREATED,
        serde_json::json!({
            "transformation": transformation_json,
            "id": transformation.0.to_text(),
            "precondition_fingerprint": fingerprint,
            // 🔴 The state the engine holds, not 44's literal "Candidate" (sem: SEM-gx-api-107). `gx plan` made the same
            // choice for the same reason: printing a word rather than a value would report a state
            // the engine might not be in, and 43 §8's waiting is a `Candidate` that has not started.
            "state": lifecycle,
            "created_at": rfc3339::of(at),
        }),
    )
}

// ---------------------------------------------------------------------------
// `GET /candidates/{id}` and `GET /transformations/{id}` — 44 §2.2
// ---------------------------------------------------------------------------

/// `GET /candidates/{id}` → `{ transformation, state, verdict, fingerprint }` (44 §2.2).
///
/// 44 §2.1's division: "`/candidates` is the workflow-control face … `/transformations` is the permanent-record read face" (sem: SEM-gx-api-108).
/// The same id can name one object through both and the two answer different questions, which is why
/// this is not one handler with a flag.
///
/// # Errors
/// `404 NOT_FOUND` for an id this engine has never planned.
pub async fn get_candidate(State(state): State<AppState>, Segment(id): Segment<String>) -> Answer {
    let id = transformation_id(&id)?;
    let at = state.now();
    let (transformation, lifecycle, verdict) = {
        // 🔴 **DR-43-6 / `req/215` H-05** — read to the end of the log first, without the lock.
        let mut engine = state.engine_refreshed()?;
        // 🔴 **[T-r56] / this lane** — see [`get_transformation`]'s copy of this comment: the
        // same best-effort, lock-free, idempotent rebuild `with_a_body` already gives the write
        // handlers, generalised to this read face rather than left a sibling that still answers
        // `null` for a row `with_a_body` would happily rebuild one call later.
        if engine.transformation(&id).is_none() {
            let _ = rebuilt(&state, &mut engine, &id, at);
        }
        (
            serde_json::to_value(engine.transformation(&id)).unwrap_or(serde_json::Value::Null),
            engine.state(&id),
            engine.verdict(&id),
        )
    };
    if lifecycle.is_none() && transformation.is_null() {
        return Err(ApiError::not_found(format!(
            "{} is not a transformation this engine holds",
            id.0.to_text()
        )));
    }
    // 🔴 **R37 / `req/496` L-02** — 44 §2.2's shape, which is the one this same surface's
    // `cancel_candidate` already answers (`SEM-gx-api-136`): the **name** of the state, with the
    // reason beside it.
    //
    // What was here was `"state": lifecycle`, i.e. `Lifecycle`'s serde derive. For every unit
    // variant that is the same string — `"Committing"`, `"Planned"` — and for the one variant that
    // carries data it is `{"Aborted":"OwnerCancelled"}`. So one row, read through two mouths of one
    // surface at one instant, answered two different shapes, and `cancel`'s own note beside the
    // flat form says why that is not a difference of taste: *"which is not `Lifecycle`'s serialised
    // form ... A wire contract is a contract about the shape"*. A client that branches on
    // `state === "Aborted"` gets `false` from the read face for a row the write face just called
    // `Aborted`, which is `req/187` §5's `undefined === false` again, one endpoint along.
    //
    // 🔴 **And what this repair deliberately does *not* add.** `req/501` §0 proposed carrying
    // `reason` here too, so that this answer and `cancel`'s were the same object. 44 §2.2 L344 is
    // the reason it does not: "`{ transformation: Transformation, state: <43's state name>,
    // verdict: Verdict|null, fingerprint: Fingerprint }`" — four keys, and `wire_census.rs` exists
    // to turn red when a fifth appears, because "a silent addition ... must turn a census RED
    // before a client sees it". Editing that expectation to admit a key this lane invented is the
    // move the census forbids; a `reason` on the read face is a change to 44 §2.2 and belongs to a
    // DR.
    //
    // The cost is real and is not being hidden: before this repair the serde form carried the
    // reason as its payload, so `GET /candidates/{id}` on an aborted row said *why*, and now it
    // does not. That is a loss of information in exchange for a shape the spec specifies and both
    // mouths agree on. `req/502` records it as the open question rather than settling it here.
    ok(
        StatusCode::OK,
        serde_json::json!({
            "transformation": transformation,
            "state": lifecycle.map(|state| state.name()),
            "verdict": verdict,
            "fingerprint": fingerprint_json(&state, &id),
        }),
    )
}

/// `GET /transformations/{id}` → `{ transformation, state, receipt, superseded_by }` (44 §2.2).
///
/// 🔴 The sentence above is 44 §2.2 L614's four members and is left standing; the wire carries
/// **six**. `rollback` was added by R29 (`req/361` §3-1) and `inverse_status` by the ruling below,
/// both under 44 §2.6's "a backward-compatible addition (a new optional field) is allowed within
/// `/v1`". See the two members at the end of the body for what each one costs and does not answer.
///
/// # Errors
/// `404 NOT_FOUND`.
pub async fn get_transformation(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
) -> Answer {
    let id = transformation_id(&id)?;
    let at = state.now();
    let (transformation, lifecycle, superseded_by, rollback, inverse_status) = {
        // 🔴 **DR-43-6 / `req/215` H-05** — read to the end of the log first, without the lock.
        let mut engine = state.engine_refreshed()?;
        // 🔴 **[T-r56] / this lane** — `with_a_body`'s rebuild (DR-43-6 / `req/215` M-01),
        // generalised from the five write handlers it already covers to this read face
        // (`feedback_fix_the_question_not_the_row`: a repair confined to the row it was measured
        // on leaves the sibling that asks the same question — "does this process hold a body for
        // this row" — still answering wrong). `GET /v1/transformations/{id}` and
        // `GET /v1/candidates/{id}` were the two siblings `without_a_body`'s own doc comment names
        // ("`GET /v1/candidates/{id}` had just answered `200`") without ever being changed
        // themselves.
        //
        // Best-effort and silent by construction: a `GET` must never turn into a write-style
        // refusal, so a rebuild this cannot attempt (no draft archive, no adapter, a re-plan that
        // names a different id) is swallowed and this falls through to the pre-existing `null`
        // body — the same answer this endpoint always gave, not a new failure mode. Safe without
        // the cross-process `.gx/LOCK` write lock `engine_for_write` takes: `rehydrate_committed`
        // appends nothing to the journal or the ledger (`store.rs`'s own note on `catch_up_unlocked`
        // says a `GET` "may not do is repair" of *those two files*; this touches neither), it only
        // seats an entry in this process's in-memory table, and it is idempotent when a body is
        // already held (`rehydrate_committed`'s own first line).
        if engine.transformation(&id).is_none() {
            let _ = rebuilt(&state, &mut engine, &id, at);
        }
        (
            serde_json::to_value(engine.transformation(&id)).unwrap_or(serde_json::Value::Null),
            engine.state(&id),
            engine.superseded_by(&id).map(|t| t.0.to_text()),
            // 🔴 **R29 / `req/361` §3-1** — see the `rollback` member below.
            engine.rollback(&id).map(|r| r.kind()),
            // 🔴 **Owner #260 / `req/987`** — see the `inverse_status` member below. Serialised
            // here, inside the read, so the value and its spelling come from one borrow.
            engine
                .inverse_status(&id)
                .and_then(|status| serde_json::to_value(status).ok())
                .unwrap_or(serde_json::Value::Null),
        )
    };
    if lifecycle.is_none() && transformation.is_null() {
        return Err(ApiError::not_found(format!(
            "{} is not a transformation this engine holds",
            id.0.to_text()
        )));
    }
    ok(
        StatusCode::OK,
        serde_json::json!({
            "transformation": transformation,
            "state": lifecycle,
            "receipt": receipt_value(&state, &id),
            "superseded_by": superseded_by,
            // 🔴 **R29 / `req/361` §3-1 + H-01** — what became of 43 T-10c's roll-back, on the read
            // face rather than only on the refusal that answered the request that aborted.
            //
            // The twenty-eighth audit drove this route **on a corrupted world** and read the body:
            // `state` gave `{"Aborted":"ApplyFailed"}`, so the abort *reason* was readable, and the
            // roll-back was not — a client speaking only HTTP could learn that the commit failed
            // and had no road at all to "is my object back where it was, or is it half undone".
            // R28's refusal-face members answered that question only for whoever was holding the
            // request that aborted; anyone arriving afterwards got silence.
            //
            // 44 §2.6 permits this explicitly ("a backward-compatible addition (a new optional
            // field) is allowed within `/v1`"), and DR-44-9's "no additions" predates the member's
            // existence and is silent about it. `null` where no roll-back was in question.
            "rollback": rollback,
            // 🔴 **Owner #260 (relay ①, ruled with the TUI seat) / `req/987` §3-4 + §4-1** — 42
            // §3.12's status of this row's escrowed inverse, on the **read face for one row** and
            // not only on the list.
            //
            // `req/987` §3-4 measured the asymmetry in the source: `list.rs` has carried this key
            // since M6H6-15 and this handler has never carried it, so *is this one still
            // undoable* was answerable about a **page** and not about the **row**, and a client
            // that already held an id had to fetch a list to learn a fact about a transformation
            // it could name. A consent screen is exactly that client.
            //
            // 🔴 **What the comment on `list.rs`'s copy does and does not say.** It reads "Why on
            // the **list** and not only on `GET /transformations/{id}`", which `req/987` §4-1 (b)
            // read as *the absence here was intended*. It does not say that: it argues that the
            // set-shaped question needs the list **as well**, and it presupposes the row face
            // rather than excluding it. The absence was an omission wearing a justification, which
            // is the same shape as R29's `rollback` — the ruling that put a member on two faces
            // and left the third.
            //
            // 44 §2.6 permits the addition in the same words it permitted `rollback`'s, and it is
            // the same value the list already publishes: the spelling here is `list.rs`'s
            // (`serde_json::to_value`), deliberately, because `req/496` L-02 is this endpoint's
            // own record of what it costs when one row read through two mouths of one surface
            // answers two shapes. **`Consumed` therefore arrives as `{"Consumed":{"by":…}}` on
            // both faces**, and not as the bare word `undo`'s refusal `detail` prints from
            // `InverseStatus::kind()` — that is a sentence for a human, not a wire contract.
            //
            // 🔴 **What this member does not answer, stated rather than implied.** It says *whether*
            // an inverse can still be run and never *what would come back* — no locator, no
            // substrate, no digest of the escrowed bytes. `req/987` §4-2 designed that descriptor
            // (`inverse: {substrate, locator, goal_cid}`) and it is **not** in this change: the
            // three members it names are reachable only from the escrow row and the state table,
            // which is a second read this handler does not take, and one of the three (`locator`)
            // is `null` after a restart on the very rows whose status the Σ-shadow can still
            // answer (`req/987` §3-8's asymmetry). A face that needs "what will be restored"
            // before it asks for consent needs that descriptor and is not served by this member;
            // that is a further ruling, and `req/987` is its reqdef.
            //
            // `null` for a transformation with **no escrow row at all** — `list.rs`'s own care,
            // repeated here for its own reason: `Unavailable` means "`invert()` answered `None`"
            // (42 §3.12), so writing it for a candidate that never reached T-10b would answer a
            // question nobody asked. `null` here is `req/987` §4-3's E1 and nothing else.
            "inverse_status": inverse_status,
        }),
    )
}

// ---------------------------------------------------------------------------
// `POST /candidates/{id}/verify` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's optional body: `{ evidence: Evidence[], record_only: bool|null }`.
#[derive(Debug, Default, Deserialize)]
pub struct VerifyRequest {
    /// 42 §3.7's values, the HTTP form of 44 §1.2's `--evidence` JSONL.
    #[serde(default)]
    pub evidence: Vec<gx_witness::Evidence>,
    /// DR-2's per-request posture — **M6-08, adopted (a)**'s argument (sem: SEM-gx-api-109), never a field on shared state.
    #[serde(default)]
    pub record_only: Option<bool>,
}

/// 🔴 `POST /candidates/{id}/verify` (44 §2.2) — T-3 → T-4, synchronously (ASM-44-2).
///
/// # 🔴 `record_only` is the argument M6-08 ruled and `evidence` is the field it did not
///
/// 44 §2.2's body carries both. `record_only` reaches `Engine::verify` as a parameter, which is
/// **M6-08, adopted (a)** and exists exactly so that this handler is correct: (b) — "serve swaps the mode via
/// `&mut self` on a per-request basis" (sem: SEM-gx-api-110) — was ruled "must not be adopted" because a posture written onto
/// shared state leaks into the next request, and a leaked `RecordOnly` is a fail-open.
///
/// `evidence` has no such argument, so it goes through [`crate::state::RequestEvidence`] — the shape
/// M6-08(b) forbids, made safe by the lock M6-06(a) adopted, and raised as **M6H5-6** because "safe
/// because of another ruling" (sem: SEM-gx-api-111) is a dependency and not a design. The cell is cleared on **every**
/// exit, including the refusing ones.
///
/// # Errors
/// `404`, `409 INVALID_STATE` (44 §2.2: "already Verifying or a terminal state"; sem: SEM-gx-api-112).
pub async fn verify_candidate(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
    body: Option<Payload<VerifyRequest>>,
) -> Answer {
    let id = transformation_id(&id)?;
    let Payload(body) = body.unwrap_or_default();
    let mode = body
        .record_only
        .and_then(|on| on.then_some(gx_core::EnforcementMode::RecordOnly));
    let at = state.now();

    let outcome = {
        let mut engine = state.engine_for_write()?;
        // 🔴 **DR-43-6 / `req/215` M-01** — see [`without_a_body`].
        if let Some(refusal) = with_a_body(&state, &mut engine, &id, at) {
            return Err(refusal);
        }
        state.evidence().load(body.evidence.clone());
        let result = engine.verify(&id, at, state.keys().signing(), mode);
        // 🔴 Cleared before the lock is released and before the `?`. A clear in a success-only path
        // would leave a refused request's evidence visible to the next caller.
        state.evidence().clear();
        result
    };
    let lifecycle = outcome.map_err(|e| ApiError::from_engine(&e))?;

    // The verdict receipt ASM-14 issued, filed where a restart can find it (M6H4-7's `verdict`).
    archive_last_verdict_receipt(&state, &id, ReceiptSlot::Verdict);

    let body = {
        let engine = state.engine();
        serde_json::json!({
            "transformation": id.0.to_text(),
            "state": lifecycle,
            "verdict": engine.verdict(&id),
            // 🔴 **M6H3-2** — the two fields 44 §1.2 writes for the CLI, on the surface 44 §2.3
            // names as the second consumer ("problem `detail`"; sem: SEM-gx-api-113). A `Deny` whose reasons were
            // dropped is a refusal an operator cannot act on.
            "proof": admit_proof_json(engine.admit_proof(&id)),
            "reasons": engine.deny_reasons(&id),
            "ticket": engine.ticket(&id),
            "enforced": engine.enforced(&id),
            "fail_posture_engaged": engine.fail_posture_engaged(&id),
            "held_by": engine.blocked_by(&id).map(|b| b.0.to_text()),
            "record_only": mode.is_some(),
            "at": rfc3339::of(at),
        })
    };
    ok(StatusCode::OK, body)
}

// ---------------------------------------------------------------------------
// `POST /candidates/{id}/commit` — 44 §2.2, and 44 §2.4's Idempotency-Key
// ---------------------------------------------------------------------------

/// 🔴 **E-M6-20** — 44 §2.2's commit body, one optional field.
#[derive(Debug, Default, Deserialize)]
pub struct CommitRequest {
    /// DR-2's posture **for this call**. `None` keeps the engine's; see [`commit_candidate`].
    #[serde(default)]
    pub record_only: Option<bool>,
}

/// 🔴 `POST /candidates/{id}/commit` (44 §2.2) — T-8 then T-9..T-11, with 44 §2.4's cache.
///
/// The four refusals 44 §2.2 specifies are answered by **state** rather than by an error's text:
/// `403 NOT_ADMITTED` for a row the gate did not admit outside record-only, `409
/// PRECONDITION_CHANGED` for T-9's CAS, `422 APPLY_FAILED` for T-10, `409 ESCALATION_PENDING` for a
/// row still waiting on a person. A message is prose and a state is a value.
///
/// # 🔴 [DR-2 sensitivity] (sem: SEM-gx-api-114) — **E-M6-20**, implemented here
///
/// 44 §2.2: "even in this case, record-only mode returns `200` + a Receipt with `enforced:false`" (sem: SEM-gx-api-115). Hand 5 could
/// not produce that answer from any HTTP request and raised it (M6H5-8); §52 ruled it —
///
/// > **E-M6-20 (M6H5-8, adopted (a))**: read 44 §2.2's commit body as having `record_only: bool` added
/// > (the HTTP version of E-M6-10; making the [DR-2 sensitivity] paragraph executable). Implementation window = the first hand from hand 6 onward that touches it (sem: SEM-gx-api-116).
///
/// — and this is that hand. The flag reaches T-8r as an **argument to `Engine::canonicalize`**, the
/// shape M6-08, adopted (a), already chose for `verify`: the alternative, a mode written onto the shared
/// engine for the length of a request, is the form §47 ruled "must not be adopted" (sem: SEM-gx-api-117) because it leaks into
/// the next caller. The engine's signature moved for it, gx-cli's `gx commit --record-only` now takes
/// the same road, and one spelling drives both surfaces.
///
/// 🔴 **The body is optional and its absence is not `false`.** A commit with no body at all keeps the
/// engine's posture, which is what every client written against 44 before this addition sends;
/// `{"record_only": false}` **overrides** a `RecordOnly` engine for this call. "unspecified" and "no" (sem: SEM-gx-api-118) are
/// different requests and `Option<bool>` is how the difference survives (M6H5-11's shape, one field
/// along).
///
/// # Errors
/// The four above, plus `404` and `409 IDEMPOTENCY_CONFLICT` (44 §2.4).
pub async fn commit_candidate(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
    headers: HeaderMap,
    body: Option<Payload<CommitRequest>>,
) -> Answer {
    let id = transformation_id(&id)?;
    let key = idempotency_key(&headers);
    let Payload(body) = body.unwrap_or_default();
    let mode = body.record_only.map(|on| {
        if on {
            gx_core::EnforcementMode::RecordOnly
        } else {
            gx_core::EnforcementMode::Enforce
        }
    });
    // 🔴 The cached request body carries the posture. 44 §2.4's conflict is "the same key, a different request
    // body" (sem: SEM-gx-api-119), and two commits of one transformation under two postures are two different requests —
    // one records a refusal and the other enforces it.
    let request_body = serde_json::json!({
        "endpoint": "commit",
        "transformation": id.0.to_text(),
        "record_only": body.record_only,
    });
    let at = state.now();

    if let Some(key) = &key {
        if let Some(entry) = state.idempotency().get(&id, key, at)? {
            let (status, body) =
                crate::idempotency::replay_or_conflict(key, &entry, &request_body)?;
            return ok(StatusCode::from_u16(status).unwrap_or(StatusCode::OK), body);
        }
    }

    // 43 T-8 and T-9..T-11 under **one** hold: the critical section is one operation and a lock
    // released between them would let a second request enter it.
    let lifecycle = {
        let mut engine = state.engine_for_write()?;
        // 🔴 **DR-43-6 / `req/215` M-01** — see [`without_a_body`]. Ahead of the `match` below,
        // because `engine.state` falls through to the Σ-shadow: a row another process planned
        // reaches the `Some` arm and is refused by a transition guard naming the wrong thing.
        if let Some(refusal) = with_a_body(&state, &mut engine, &id, at) {
            return Err(refusal);
        }
        match engine.state(&id) {
            None => {
                return Err(ApiError::not_found(format!(
                    "{} is not a transformation this engine holds",
                    id.0.to_text()
                )))
            }
            // 🔴 **43 T-9's idempotency column, at the surface.** A row that already reached
            // `Committed` is answered with what it reached, and no transition is attempted.
            //
            // Without this a retry — the thing 44 §2.4 exists for — would reach `canonicalize`,
            // which refuses from `Committed` with `InvalidState`, and the client would be told
            // `409` about a commit that **succeeded**. 44 §1.2 promises the opposite for the CLI
            // ("a retry of the identical run is naturally idempotent"; sem: SEM-gx-api-120) and the two surfaces answering differently about one
            // engine is precisely what req/88 §3 Λ2 says must not happen.
            //
            // Note what this does *not* depend on: the idempotency key. A retry with no key at all
            // lands here, which is the measurement `tests/idempotency.rs` uses to show that the
            // exactly-once guarantee is ASM-43-1's and not the cache's.
            Some(Lifecycle::Committed) => {
                drop(engine);
                return committed_answer(&state, &id, at, key.as_deref(), &request_body);
            }
            Some(_) => {}
        }
        if let Err(e) = engine.canonicalize(&id, at, mode) {
            let refused = canonicalise_refusal(engine.state(&id), &e);
            return Err(refused);
        }
        engine
            .commit(&id, at, state.keys().signing())
            .map_err(|e| ApiError::from_engine(&e))?
    };

    if !is_committed(lifecycle) {
        return Err(aborted_refusal(&state, &id, lifecycle));
    }
    archive_commit_receipt(&state, &id)?;
    committed_answer(&state, &id, at, key.as_deref(), &request_body)
}

/// 44 §2.2's `200 OK` for a committed transformation — the receipt, and 44 §2.4's record beside it.
///
/// One function for the fresh commit and for the re-entrant one, because 43 T-9 makes them one
/// answer: "the transition applied" and "the transition had already applied" (sem: SEM-gx-api-121) are the same state and
/// the same receipt, and two spellings of the body would let a retry look different from the call it
/// is retrying — the one thing 44 §2.4 exists to prevent.
fn committed_answer(
    state: &AppState,
    id: &TransformationId,
    at: gx_core::Timestamp,
    key: Option<&str>,
    request_body: &serde_json::Value,
) -> Answer {
    // 44 §2.2: "Response `200 OK` (Committed success): `Receipt` (42 §3.10's JSON representation, `payload` is
    // base64)" (sem: SEM-gx-api-122). The receipt's own `Serialize` produces that form; the fields beside it are what a
    // client needs and cannot compute.
    //
    // 🔴 The receipt may be `null` on a **restarted** server: `Engine::open` leaves the in-flight
    // table empty (M5H3-5), so the row Σ rebuilt has a state and no receipt. The archive is the road
    // back and `receipt_value` takes it; where there is neither, `null` beside a true state is a
    // better answer than a 404 about a commit that happened.
    //
    // 🔴 Each read takes the engine lock **and gives it back** before the next. `std::sync::Mutex`
    // is not re-entrant, so a guard held across `receipt_value` — which takes one of its own — is a
    // deadlock rather than a slow request. Measured, not reasoned about: the first spelling of this
    // function held one and the suite stopped answering.
    let mut body = receipt_value(state, id);
    if body.is_null() {
        body = serde_json::json!({});
    }
    let enforced = state.engine().enforced(id);
    if let Some(map) = body.as_object_mut() {
        map.insert("transformation".into(), id.0.to_text().into());
        map.insert("state".into(), "Committed".into());
        map.insert(
            "enforced".into(),
            serde_json::to_value(enforced).unwrap_or(serde_json::Value::Null),
        );
        map.insert("at".into(), rfc3339::of(at).into());
    }

    if let Some(key) = key {
        state
            .idempotency()
            .put(id, key, request_body, 200, &body, at)?;
    }
    ok(StatusCode::OK, body)
}

/// What a refused `canonicalize` means for 44 §2.2's four commit statuses.
///
/// 43 T-8's from-state is `Admitted` and T-8r adds `Denied` under `RecordOnly` only, so a refusal
/// here is the state machine answering. The two 44 gives codes for are read from the **state**.
fn canonicalise_refusal(lifecycle: Option<Lifecycle>, e: &gx_engine::Error) -> ApiError {
    match lifecycle {
        Some(Lifecycle::Denied) => ApiError::new(
            "NOT_ADMITTED",
            "the gate did not admit this transformation",
            format!(
                "44 §2.2: \"`403` (not record-only and Verdict≠Admit)\" (sem: SEM-gx-api-123). 43 T-8's from-state is \
                 `Admitted`; T-8r opens the same door under `EnforcementMode::RecordOnly` and this \
                 server is not in it. {e}"
            ),
        ),
        Some(Lifecycle::Escalated) => ApiError::new(
            "ESCALATION_PENDING",
            "a person has not ruled on this transformation yet",
            format!(
                "44 §2.2: \"`409 Conflict` (the target remains `Escalated`)\" (sem: SEM-gx-api-124). INV-S6: \"`Escalated` does not auto-transition to \
                 `Admitted`/`Denied` without going through T-5/T-5b's signed human-ruling receipt\". {e}"
            ),
        ),
        _ => ApiError::from_engine(e),
    }
}

/// 🔴 **R28 / `req/334` M-01** — the two roll-back facts of one transformation, as members.
///
/// One function for both abort roads (`aborted_refusal`, the commit's; `halted_undo`, the undo's)
/// for the reason `crate::problem::RollbackFacts` gives: the pair is written together or not at
/// all. Both accessors are `Engine`'s and neither derives anything — the value is on a signed
/// `Aborted` record and the cause is a map this process fills when *it* reaches the abort.
fn rollback_facts(state: &AppState, id: &TransformationId) -> RollbackFacts {
    let engine = state.engine();
    RollbackFacts {
        rollback: engine.rollback(id).map(|r| r.kind()),
        not_attempted_because: engine.rollback_not_attempted_because(id).map(|b| b.kind()),
    }
}

/// The status 44 §2.2 gives a commit that reached `Aborted` rather than `Committed`.
///
/// 🔴 **R28 / `req/334` M-01** — the roll-back left `detail` and became a pair of members.
///
/// Until this lane the value reached the wire as `{:?}` inside the prose: `rollback:
/// Some(Succeeded)`, a Rust `Debug` rendering of an engine type, in the one field RFC 9457 defines
/// as prose. A client that wanted to branch on it had to parse English **and** guess that the
/// parenthesised word was stable. The facts are now members and the sentence says what happened
/// without carrying the value twice.
fn aborted_refusal(state: &AppState, id: &TransformationId, lifecycle: Lifecycle) -> ApiError {
    let facts = rollback_facts(state, id);
    let detail = format!(
        "{} is {} after 43 T-9..T-11; what became of 43 T-10c's roll-back is on the `rollback` \
         member of this object, and why it was not attempted (where this process is the one that \
         reached the abort) is on `rollback_not_attempted_because`",
        id.0.to_text(),
        lifecycle.name(),
    );
    let refusal = match lifecycle {
        Lifecycle::Aborted(gx_core::AbortReason::PreconditionChanged) => ApiError::new(
            "PRECONDITION_CHANGED",
            "the object changed between verification and commit",
            detail,
        ),
        Lifecycle::Aborted(gx_core::AbortReason::ApplyFailed) => ApiError::new(
            "APPLY_FAILED",
            "the adapter could not apply the delta",
            detail,
        ),
        _ => ApiError::internal(detail),
    };
    refusal.with_rollback_facts(facts)
}

// ---------------------------------------------------------------------------
// `POST /candidates/{id}/escalation` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's body: `{ decision: "approve"|"reject", reason: string, actor: Actor }`.
#[derive(Debug, Deserialize)]
pub struct EscalationRequest {
    /// 43 T-5 (`approve`) or T-5b (`reject`).
    pub decision: String,
    /// 44 §2.2 makes it required, and `Engine::escalation` refuses an empty one.
    pub reason: String,
    /// 🔴 **The ruler**, and 42 §3.13 is explicit that this is not `Transformation.actor`.
    pub actor: Actor,
}

/// 🔴 `POST /candidates/{id}/escalation` (44 §2.2) — T-5 / T-5b, signed by the **ruler's** key.
///
/// **E-M6-15** made `--actor-key` required on the CLI verb and INV-S6 is the reason: "there is no default
/// under which the party being ruled on approves themselves" (sem: SEM-gx-api-125). The HTTP form of that ruling is that the request **carries** the
/// ruler's key id and the server looks it up ([`crate::state::ServerKeys::ruler`]) with no fallback:
/// a ruling signed with the server's own key would record that the server allowed the change, and a
/// ruling signed with the submitter's key would record that the party being ruled on approved
/// themselves.
///
/// 44 §0's id-resolution is honoured one verb further out, as **M6-04, adopted (c)** (sem: SEM-gx-api-126): a `TicketId` is
/// accepted where 44 §2.2 writes `{id}`, which removes the asymmetry between the CLI's `<TICKET_ID>`
/// and this path parameter.
///
/// # Errors
/// `404`, `409 INVALID_STATE` ("the target is not `Escalated`"; sem: SEM-gx-api-127), `422 VALIDATION_ERROR` ("`reason` missing, etc."),
/// and an `actor` this server holds no key for).
pub async fn escalate_candidate(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
    Payload(body): Payload<EscalationRequest>,
) -> Answer {
    let decision = match body.decision.as_str() {
        "approve" => VerdictKind::Admit,
        "reject" => VerdictKind::Deny,
        other => {
            return Err(ApiError::validation(format!(
                "`decision` is \"approve|reject\" (44 §2.2; sem: SEM-gx-api-128); got {other:?}"
            )))
        }
    };
    if body.reason.trim().is_empty() {
        return Err(ApiError::validation(
            "`reason` is required and cannot be blank (44 §2.2). AC-071/072 both ask the reason to \
             reach the trail, and a ruling that says nothing is a ruling nobody can audit",
        ));
    }
    let cid = gx_core::Cid::from_text(&id)
        .map_err(|e| ApiError::validation(format!("`{id}` is not a `gx1:` id: {e}")))?;

    let key = state.keys().ruler(body.actor.key()).ok_or_else(|| {
        ApiError::validation(format!(
            "this server holds no signing key for {:?}, and INV-S6 leaves no default: 43 T-5's \
             guard is \"the adjudicator holds a valid signing key\" and E-M6-15 ruled that \"there is no default \
             under which the party being ruled on approves themselves\" (sem: SEM-gx-api-129). Signing with the server's own key would record that the \
             server allowed this change",
            body.actor.key()
        ))
    })?;

    let ruling = HumanRuling {
        decision,
        reason: body.reason.clone(),
        actor: body.actor.clone(),
    };
    let at = state.now();
    let (transformation, lifecycle) = {
        let mut engine = state.engine_for_write()?;
        // 44 §0's id-resolution, M6-04, adopted (c)'s extension (sem: SEM-gx-api-130): a ticket id first, because 43 T-4c makes
        // the two 1:1 and the CLI's synopsis names the ticket.
        let named = gx_gate::TicketId(cid);
        let transformation = match engine
            .transformation_of_ticket(&named)
            .map_err(|e| ApiError::from_engine(&e))?
        {
            Some(found) => found,
            None => TransformationId(cid),
        };
        // 🔴 **DR-43-6 / `req/215` M-01** — see [`without_a_body`]. After the ticket is resolved,
        // because the id in the path may be a ticket's and the row is the transformation's.
        if let Some(refusal) = with_a_body(&state, &mut engine, &transformation, at) {
            return Err(refusal);
        }
        let lifecycle = engine
            .escalation(&transformation, &ruling, at, key)
            .map_err(|e| ApiError::from_engine(&e))?;
        (transformation, lifecycle)
    };

    // 43 T-5's receipt, under M6H4-7's third kind: signed by a **different key** from the verdict
    // receipt beside it, which is why the two need two files.
    archive_last_verdict_receipt(&state, &transformation, ReceiptSlot::Ruling);

    let signed_by = {
        let engine = state.engine();
        engine
            .verdict_receipts(&transformation)
            .last()
            .and_then(|r| r.payload().ok())
            .map(|p| p.key_id)
    };
    ok(
        StatusCode::OK,
        serde_json::json!({
            "transformation": transformation.0.to_text(),
            "state": lifecycle,
            "decision": ruling.decision,
            "reason": ruling.reason,
            "ruled_by": ruling.actor,
            // 🔴 Which key signed. Without it a ruling signed with the submitter's key would be
            // indistinguishable from a correct one at this surface — the fact hand 4's battery
            // point (l) measured on the CLI side.
            "signed_by": signed_by,
            "at": rfc3339::of(at),
        }),
    )
}

// ---------------------------------------------------------------------------
// `POST /candidates/{id}/cancel` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's body: `{ actor: Actor }`.
#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    /// 44 §2.2: "an explicit cancel by an actor holding owner privilege" (sem: SEM-gx-api-131). See [`cancel_candidate`] for what is
    /// and is not checked about that sentence.
    pub actor: Actor,
}

/// 🔴 `POST /candidates/{id}/cancel` (44 §2.2) — T-7, with **E-M6-1**'s from-set and no owner check.
///
/// 44 L101 lists `Draft` first and req/38 §47 M6-03, adopted (c) (sem: SEM-gx-api-132), removed it; on this surface the point is
/// moot for a reason 44 §2.1 states itself — a `Draft` is not observable here.
///
/// # 🔴 The `actor` in the body is recorded nowhere and checked against nothing
///
/// 43 T-7's guard is "the actor holds owner privilege (equivalent to `Actor::Human{key}`)" (sem: SEM-gx-api-133) and v0.1 has no
/// authorization layer (M5H6-4, adopted (a)): `Engine::cancel` takes no actor, 43 T-7's `Aborted` record
/// has no field for one, and this surface's only check is the Bearer token
/// ([`crate::auth::ABSENCE_NOTICE`]). 44 §2.2 requires the field, so it is required and its fate is
/// said out loud rather than implied — the feedback ledger §1.2: "authorization is 0 (anybody can press cancel)" (sem: SEM-gx-api-134).
///
/// # Errors
/// `404`, `409 INVALID_STATE` (44 §2.2: "already `Committing` or beyond, or a terminal state"; sem: SEM-gx-api-135).
pub async fn cancel_candidate(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
    Payload(body): Payload<CancelRequest>,
) -> Answer {
    let id = transformation_id(&id)?;
    let _ = &body.actor;
    let at = state.now();
    let lifecycle = {
        let mut engine = state.engine_for_write()?;
        // 🔴 **DR-43-6 / `req/215` M-01** — see [`without_a_body`].
        if let Some(refusal) = with_a_body(&state, &mut engine, &id, at) {
            return Err(refusal);
        }
        engine
            .cancel(&id, at)
            .map_err(|e| ApiError::from_engine(&e))?
    };
    let reason = match lifecycle {
        Lifecycle::Aborted(reason) => Some(reason),
        _ => None,
    };
    ok(
        StatusCode::OK,
        serde_json::json!({
            "transformation": id.0.to_text(),
            // 44 §2.2's own shape: "`{ transformation, state: "Aborted", reason: "OwnerCancelled" }`" (sem: SEM-gx-api-136)
            // — the **name** of the state with the reason beside it, which is not `Lifecycle`'s
            // serialised form (`{"Aborted":"OwnerCancelled"}`). A wire contract is a contract about
            // the shape.
            "state": lifecycle.name(),
            "reason": reason,
            // 🔴 44 §2.2 requires the field and nothing checks it. Echoed so that a client can see
            // that what they sent was recorded in the response and **not** in the ledger.
            "actor_unchecked": body.actor,
            "at": rfc3339::of(at),
        }),
    )
}

// ---------------------------------------------------------------------------
// `POST /transformations/{id}/undo` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's body: `{ actor: Actor }`.
#[derive(Debug, Default, Deserialize)]
pub struct UndoRequest {
    /// 44 §2.2's field. `Engine::undo` mints T_u's intent with **T_o's** actor (P-5), so this
    /// selects nothing — the CLI refuses the equivalent flag outright (M6H4-3) and this surface
    /// cannot, because 44 §2.2 makes the body required. Echoed, unchecked, like `cancel`'s.
    #[serde(default)]
    pub actor: Option<Actor>,
}

/// 🔴 `POST /transformations/{id}/undo` (44 §2.2) — T-12's first half, then the whole pipeline.
///
/// 43 §5-2: "even an undo is not exempted from verification" (sem: SEM-gx-api-137), so the inverse walks `Draft→…→Committed` on its own
/// feet and this drives the same four steps `gx undo` drives.
///
/// # Errors
/// `409 INVERSE_UNAVAILABLE` when 42 §3.12's status is not `Available` — checked **before** the
/// pipeline is entered, because 44 §2.2 names that status and the engine's own refusal for it is a
/// `NotFound` that would answer 404 about a transformation that exists. `404` for an unknown id.
pub async fn undo_transformation(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
    headers: HeaderMap,
    body: Option<Payload<UndoRequest>>,
) -> Answer {
    let id = transformation_id(&id)?;
    let Payload(body) = body.unwrap_or_default();
    let key = idempotency_key(&headers);
    let request_body = serde_json::json!({
        "endpoint": "undo",
        "transformation": id.0.to_text(),
        "actor": body.actor,
    });
    let at = state.now();

    if let Some(key) = &key {
        if let Some(entry) = state.idempotency().get(&id, key, at)? {
            let (status, body) =
                crate::idempotency::replay_or_conflict(key, &entry, &request_body)?;
            return ok(StatusCode::from_u16(status).unwrap_or(StatusCode::OK), body);
        }
    }

    let seed = state.seed();
    let (undoing, witness_said) = {
        let mut engine = state.engine_for_write()?;
        if engine.state(&id).is_none() {
            return Err(ApiError::not_found(format!(
                "{} is not a transformation this engine holds",
                id.0.to_text()
            )));
        }
        // 🔴 **T6 condition ① L2** (`req/38` §148 ruling 1(iii), lane R2). R1 left this the one
        // write handler that refused a body-less row from inside the engine (`UndoRefusal::NoBody`,
        // measured by `serve_runtime_e2e` as the `409` a restarted server gave); the refusal stays
        // exactly where it was and is now reached only when there is genuinely nothing to rebuild
        // from. `undo` is also the handler for which this matters most: 43 §5 makes it an operation
        // **on a committed transformation**, which is precisely the row a long-lived server is
        // least likely to still hold.
        if let Some(refusal) = with_a_body(&state, &mut engine, &id, at) {
            return Err(refusal);
        }
        match engine.inverse_status(&id) {
            Some(InverseStatus::Available) => {}
            other => {
                return Err(ApiError::new(
                    "INVERSE_UNAVAILABLE",
                    "there is no escrowed inverse to commit",
                    format!(
                    "44 §2.2: \"`409` (`InverseStatus` is `Unavailable`/`Expired`/`Consumed`)\" (sem: SEM-gx-api-138). \
                         42 §3.12 says this one is {}",
                    other.map_or("absent".to_string(), |s| s.kind().to_string())
                ),
                ))
            }
        }
        // 🔴 **DR-43-1, adopted (a)** (`req/38` §132 ruling 2) — the HTTP face of the same gate the
        // CLI puts in front of `Engine::undo` — the ruling says the HTTP surface carries it too, so
        // a world that moved answers `409 PRECONDITION_CHANGED` here exactly as it answers exit 3
        // there.
        //
        // The witness comes from the receipt **archive** rather than from the engine's table,
        // because `Engine::open` leaves the table empty after a restart (M5H3-5) and a server that
        // read only its own memory would silently downgrade to `Unobservable` for every row it did
        // not commit itself. A deployment with `NoArchive` says so by name instead.
        let witness = undo_witness(&state, &id);
        // 🔴 **R3 / `req/222` M-12** — read here, answered below. The engine takes the witness by
        // reference and consumes nothing, but the word is taken now so that the answer cannot drift
        // from the value the judgement was made on.
        let witness_said = witness_word(&witness);
        // 🔴 **`req/182` H-16 closed** (lane R2). `Engine::undo` mints `T_u`'s intent in memory
        // and 42 §3.13 records only its id, so before this line the undo of an undo had no body to
        // rebuild from and answered 44 §1.4's 6 (`req/216` §3, `undo_cas_e2e`). Read **before** the
        // call, because the escrow row it is computed from is `Consumed` by the time the undo
        // commits; filed **after** it, because a refused undo must leave no trace and a draft filed
        // ahead of a `PRECONDITION_CHANGED` would be one.
        let undo_draft = engine.undo_intent(&id).ok().flatten();
        let (undo_intent_id, undoing) = engine
            .undo(&id, &witness, seed, at)
            .map_err(|e| ApiError::from_engine(&e))?;
        if let Some(intent) = &undo_draft {
            // 🔴 **R16 / `req/262` H-01** — a value rather than a panic; see [`crate::notes`].
            if let Err(why) = state.drafts().store(&undo_intent_id, intent) {
                crate::api_note!(
                    "gx serve: the draft archive would not hold the undo's own intent {}: {why}. \
                     The undo stands; undoing it after a restart will not find a body \
                     (req/182 H-16)",
                    undo_intent_id.0.to_text()
                );
            }
        }
        let verified = engine
            .verify(&undoing, at, state.keys().signing(), None)
            .map_err(|e| ApiError::from_engine(&e))?;
        // 🔴 **FR-M7-1** (ruling #4, `req/98` §3-2, confirmed `req/38` §57; sem: SEM-gx-api-139) — DR-2's record-only road,
        // on the row 44 §2.1 gives the same `[DR-2 sensitivity]` ✓ as `/commit`.
        //
        // 44 §2.2 says of a commit the gate refused: "even in this case, record-only mode returns `200` +
        // a Receipt with `enforced:false`" (sem: SEM-gx-api-140), and until this hand `/undo` had no such road at all —
        // it required `Admitted` and answered `403` in every posture. The fix batch measured that
        // and refused to patch it, because "changing it would give `undo` a new semantics, which is
        // a ruling" (req/97 §6 M6FIX-2). Ruling #4 is the ruling (sem: SEM-gx-api-141): "a `/undo` in record-only posture
        // records the undo request and answers with `enforced:false` (symmetric with commit)".
        //
        // Nothing new is *decided* here, which is why the condition is this small: the road already
        // exists one layer down. `Engine::canonicalize` admits `Denied` when the effective mode is
        // `RecordOnly` (T-8r) and stamps `enforced=false`, so all this branch does is stop refusing
        // before the engine is asked. The two narrowings are deliberate:
        //
        // * **`Denied` only** ([`is_denied`]). `Escalated` is a person's queue, not a posture's.
        // * **the engine's posture only.** `/commit` also takes a per-call `record_only` in its
        //   body (E-M6-20); FR-M7-1's AC is written about "a server whose posture=`RecordOnly`" (sem: SEM-gx-api-142) and this
        //   hand did not widen it, because a per-call axis on `/undo` is a **wire change** to 44
        //   §2.2's undo body and belongs to whoever rules on it. Raised in `req/99` §residue.
        let recording = engine.mode() == gx_core::EnforcementMode::RecordOnly;
        if !is_admitted(verified) && !(recording && is_denied(verified)) {
            // 🔴 The guard is dropped **before** the error is built. `halted_undo` takes a lock of
            // its own (deliberately: it reads the state back rather than carrying a `Lifecycle`
            // across a parameter list, which Rule 1 (iii) reads as a state table; sem: SEM-gx-api-143), and
            // `std::sync::Mutex` is not re-entrant — so calling it with this guard alive is a
            // deadlock, and under M6-06, adopted (a) (sem: SEM-gx-api-144)'s single `Arc<Mutex<Engine>>` it is a deadlock that
            // takes the whole server with it, not just the request. Measured by
            // `tests/dr2.rs::a_record_only_undo_says_so`, which hung for 60 s before this line
            // existed: the path needs a gate that **denies the inverse**, and no suite had one.
            // The same shape `committed_answer` documents one screen up ("a guard held across
            // `receipt_value` is a deadlock rather than a slow request"), on the branch nobody took.
            drop(engine);
            return Err(halted_undo(&state, &id, &undoing));
        }
        engine
            .canonicalize(&undoing, at, None)
            .map_err(|e| ApiError::from_engine(&e))?;
        let committed = engine
            .commit(&undoing, at, state.keys().signing())
            .map_err(|e| ApiError::from_engine(&e))?;
        if !is_committed(committed) {
            drop(engine);
            return Err(halted_undo(&state, &id, &undoing));
        }
        (undoing, witness_said)
    };

    archive_last_verdict_receipt(&state, &undoing, ReceiptSlot::Verdict);
    archive_commit_receipt(&state, &undoing)?;

    let body = {
        let engine = state.engine();
        let mut json =
            serde_json::to_value(engine.receipt(&undoing)).unwrap_or(serde_json::Value::Null);
        if let Some(map) = json.as_object_mut() {
            map.insert("transformation".into(), undoing.0.to_text().into());
            map.insert("undone".into(), id.0.to_text().into());
            map.insert(
                "superseded_state".into(),
                serde_json::to_value(engine.state(&id)).unwrap_or(serde_json::Value::Null),
            );
            // 🔴 **M6H8-14 ④, adopted (a)** (req/38 §55; sem: SEM-gx-api-145). 44 §2.1 puts the same DR-2 mark (✓) on `/undo`
            // as on `/commit`, and until this batch only `/commit` said which axis it ran on:
            // `committed_answer` inserts `enforced` and this handler did not. An undo drives
            // canonicalize→commit with `mode: None`, so a server in RecordOnly writes an undo
            // receipt with `enforced=false` — and a client could learn that only by base64-decoding
            // the DSSE payload itself. Two endpoints wearing one mark must expose one fact.
            map.insert(
                "enforced".into(),
                serde_json::to_value(engine.enforced(&undoing)).unwrap_or(serde_json::Value::Null),
            );
            map.insert("at".into(), rfc3339::of(at).into());
            // 🔴 **R3 / `req/222` M-12** — whether DR-43-1's compare-and-set ran, on the answer.
            //
            // `attested` means the escrowed inverse was applied over a world whose digest equalled
            // the one `T_o`'s commit receipt signed. `unobservable:<reason>` means the substrate
            // has no position to compare (43 §5.2, `req/38` §123 ruling 1) and the undo went ahead
            // saying so. A GUI that draws "verified undo" may draw it on the first and must not on
            // the second; before this member it could not tell them apart, and `req/188` §9-2's
            // whole tip is that a third party can tell.
            map.insert("witness".into(), witness_said.clone().into());
        }
        json
    };
    if let Some(key) = &key {
        state
            .idempotency()
            .put(&id, key, &request_body, 200, &body, at)?;
    }
    ok(StatusCode::OK, body)
}

/// Where an undo stopped short of `Committed`, and the code 44 gives that stop.
fn halted_undo(
    state: &AppState,
    original: &TransformationId,
    undoing: &TransformationId,
) -> ApiError {
    // 🔴 The state is **read from the engine** rather than passed in, and the reason is one Rule 1
    // (iii) found rather than one this hand chose: `authority_boundary.rs` reads "a declaration
    // whose type mentions `Lifecycle` and ends in a comma" as "the surface keeps a state table" (sem: SEM-gx-api-146),
    // and a parameter list is exactly that shape once rustfmt puts one argument per line. The probe
    // is right to be coarse, so the code moved — and reading the state back is what every other
    // handler does anyway. gx-cli's `lifecycle::halted` hit the identical scanner and drew the
    // identical conclusion.
    let lifecycle = state.engine().state(undoing);
    // 🔴 **R28 / `req/334` M-01** — the roll-back of the **inverse**, which is the question an
    // operator asks next and which this road answered with silence until this lane.
    //
    // The id is `undoing` and not `original`: T_u is the row that aborted, and T_u's own T-10c is
    // what says whether the object is back where it was or half undone. The twenty-seventh audit
    // drove both terminal states — the inverse refused with its roll-back succeeding
    // (`Some(Succeeded)`) and the inverse refused with its roll-back refused too (`Some(Failed)`)
    // — and read one identical body for both. gx-cli's twin (`lifecycle::halted`) has asked this
    // since M6; this is the same question on the surface the declared consumption model
    // (R-GUI-on-SDK) actually speaks.
    //
    // The strongest argument against asking it is written into the audit that found it: *T_u is
    // itself a roll-back, so asking what became of its roll-back is not a question*. It is: the
    // engine escrows an inverse for T_u exactly as for any other transformation, T-10c fires on it,
    // and Σ records the outcome under a value this build already has three words for. R27's own
    // narrowing was "do not ask about a row that did not abort" — not "do not ask about a row that
    // did".
    let facts = rollback_facts(state, undoing);
    let detail = {
        let engine = state.engine();
        format!(
            "the inverse {} of {} reached {:?} rather than Committed; 43 §5-2: \"even an undo is not \
             exempted from verification\" (sem: SEM-gx-api-147). verdict: {:?}. What became of the \
             inverse's own roll-back is on the `rollback` member of this object — the difference \
             between an object that is back where it was and one that is half undone",
            undoing.0.to_text(),
            original.0.to_text(),
            lifecycle,
            engine.verdict(undoing)
        )
    };
    let refusal = match lifecycle {
        Some(Lifecycle::Denied) => {
            ApiError::new("NOT_ADMITTED", "the gate did not admit the inverse", detail)
        }
        Some(Lifecycle::Escalated) => ApiError::new(
            "ESCALATION_PENDING",
            "the inverse is waiting on a person",
            detail,
        ),
        Some(Lifecycle::Aborted(gx_core::AbortReason::PreconditionChanged)) => ApiError::new(
            "PRECONDITION_CHANGED",
            "the object changed while the inverse was being committed",
            detail,
        ),
        Some(Lifecycle::Aborted(gx_core::AbortReason::ApplyFailed)) => ApiError::new(
            "APPLY_FAILED",
            "the adapter could not apply the inverse",
            detail,
        ),
        _ => ApiError::internal(detail),
    };
    refusal.with_rollback_facts(facts)
}

// ---------------------------------------------------------------------------
// `POST /transformations/{id}/replay` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's optional body: `{ from: u64|null, to: u64|null, dry_run: bool|null }`.
#[derive(Debug, Default, Deserialize)]
pub struct ReplayRequest {
    /// First journal record index, inclusive (**E-M6-6**: of the **journal**, not the ledger).
    #[serde(default)]
    pub from: Option<usize>,
    /// Last journal record index, exclusive.
    #[serde(default)]
    pub to: Option<usize>,
    /// Accepted for 44's synopsis. Replay writes nothing (E-M5-2); reported rather than ignored.
    #[serde(default)]
    pub dry_run: Option<bool>,
}

/// 🔴 `POST /transformations/{id}/replay` (44 §2.2) — `{ matches, diffs }` over Σ, read-only.
///
/// **E-M5-2** fixed replay as "a read-only operation that reconstructs only `Σ`" and **M6-26, adopted (a)** fixed what
/// `diffs` can be: "`matches` answers with byte equality, `diffs` is the sequence of 'the first component name that
/// disagreed, if any'" (sem: SEM-gx-api-148). Three of Σ's four components have no second copy anywhere, so the comparison that means
/// something is the ledger's — and `unchecked` is the denominator that keeps `matches: true` from
/// reading as "everything agreed" (sem: SEM-gx-api-149).
///
/// # Errors
/// `404`, `422 VALIDATION_ERROR` (44 §2.2: "cannot replay due to a missing journal, etc."; sem: SEM-gx-api-150).
pub async fn replay_transformation(
    State(state): State<AppState>,
    Segment(id): Segment<String>,
    body: Option<Payload<ReplayRequest>>,
) -> Answer {
    let id = transformation_id(&id)?;
    let Payload(body) = body.unwrap_or_default();

    let engine = state.engine();
    let records = engine.journal().records();
    let selected: Vec<gx_engine::EngineJournalRecord> = match (body.from, body.to) {
        (Some(from), Some(to)) => {
            if from > to || to > records.len() {
                return Err(ApiError::validation(format!(
                    "from {from} to {to} is not a range of this journal's {} records (indices are \
                     of the journal, not the ledger — E-M6-6)",
                    records.len()
                )));
            }
            records[from..to].to_vec()
        }
        (None, None) => records
            .iter()
            .filter(|r| {
                r.transformation().as_ref() == Some(&id)
                    || matches!(r, gx_engine::EngineJournalRecord::Superseded { by, .. } if by == &id)
            })
            .cloned()
            .collect(),
        _ => {
            return Err(ApiError::validation(
                "`from` and `to` come as a pair (44 §2.2)",
            ))
        }
    };
    if selected.is_empty() {
        return Err(ApiError::not_found(format!(
            "the journal holds no record of {}",
            id.0.to_text()
        )));
    }

    let sigma = reconstruct(&selected);
    let log = engine.ledger().log();
    let mut diffs: Vec<serde_json::Value> = Vec::new();
    for row in sigma.ledger() {
        match log
            .entries()
            .iter()
            .find(|e| e.transformation == row.transformation)
        {
            Some(entry) if entry.index == row.ledger_seq => {}
            Some(entry) => diffs.push(serde_json::json!({
                "component": "ledger",
                "transformation": row.transformation.0.to_text(),
                "journal_ledger_seq": row.ledger_seq,
                "ledger_index": entry.index,
            })),
            None => diffs.push(serde_json::json!({
                "component": "ledger",
                "transformation": row.transformation.0.to_text(),
                "journal_ledger_seq": row.ledger_seq,
                "ledger_index": serde_json::Value::Null,
            })),
        }
    }

    ok(
        StatusCode::OK,
        serde_json::json!({
            "matches": diffs.is_empty(),
            "diffs": diffs,
            // The denominator. `matches: true` means "the one component with a second copy agreed" (sem: SEM-gx-api-151).
            "unchecked": ["drafts", "transformations", "escrow"],
            "records_replayed": selected.len(),
            "dry_run": body.dry_run.unwrap_or(false),
        }),
    )
}

// ---------------------------------------------------------------------------
// `GET /receipts/{tid}` — 44 §2.2
// ---------------------------------------------------------------------------

/// The receipt this engine (or the archive beside it) holds for a transformation.
fn receipt_value(state: &AppState, id: &TransformationId) -> serde_json::Value {
    {
        let engine = state.engine();
        if let Some(receipt) = engine.receipt(id) {
            return serde_json::to_value(receipt).unwrap_or(serde_json::Value::Null);
        }
    }
    // 🔴 **R3** — the commit receipt, following [`crate::ReceiptArchive::load_commit`]'s narrowing.
    // 44 §2.2's `GET /receipts/{tid}` is "the receipt for a **committed** transformation" and
    // answers `404` for one that has not committed, so the disclosure-order fallback this used to
    // inherit could only ever answer a verdict receipt to a question about a commit.
    state
        .archive()
        .load_commit(id)
        .map_or(serde_json::Value::Null, |r| {
            serde_json::to_value(r).unwrap_or(serde_json::Value::Null)
        })
}

/// 🔴 **DR-44-9** (`req/38` §168 ruling 1, `req/187` §5) — the same receipt, typed, for the one
/// handler that needs to look **inside** it.
///
/// [`receipt_value`] answers a `serde_json::Value` because every composite that carries a receipt
/// mounts it beside other keys; this reads the same two sources in the same order and hands back
/// the value itself, so that `receipt_view` below is derived from the receipt this endpoint is
/// about to answer with — and not from a second lookup that could disagree with it.
fn receipt_typed(state: &AppState, id: &TransformationId) -> Option<gx_witness::Receipt> {
    {
        let engine = state.engine();
        if let Some(receipt) = engine.receipt(id) {
            return Some(receipt.clone());
        }
    }
    state.archive().load_commit(id)
}

/// 🔴 **DR-44-9** — the signed payload's readable half, decoded **here** so that a reader does not
/// have to carry a DAG-CBOR decoder (`req/38` §168 ruling 1; `req/187` §5's first filing).
///
/// # What this is, and the one thing it is not
///
/// `Receipt` is `{envelope, issued_at}` (42 §3.10, E-M2-6): everything a receipt *says* is
/// canonical DAG-CBOR inside `envelope.payload`, base64 on this wire. `req/187` §5 measured the
/// consequence from the other side — a window that renders receipts reads `digest`/`n`/`at`/`alg`
/// off a document that carries none of them — and asked for one of two things: a decoder in the
/// window, or a decoded view here. §168 ruled the view, and this is it.
///
/// **It carries no judgement.** No `verified`, no `refuted`, no `inclusion`. Deciding whether a
/// signature holds and whether a leaf is in a tree is `gx_witness::verify_offline`'s, run where the
/// reader is (`gx receipt verify`, 44 §1.2, DR-44-4's one command; a desktop shell proxies to the
/// CLI). **HTTP carries the proof and does not grade it** — a server that graded its own receipts
/// would be marking its own paper, which is the same sentence
/// `crates/gx-api/src/verdict_checkpoints.rs` already writes about `/verify`.
///
/// # Derived from the payload alone, and why that is the whole design
///
/// Every member below is read off [`gx_witness::ReceiptPayload`] — the bytes the signature covers —
/// or off `Receipt.issued_at`, which travels beside them. **Nothing is read from the engine's
/// table, the ledger, or the archive.** A view assembled from the server's *current* state could
/// disagree with the document printed next to it, and then two answers would exist to "what does
/// this receipt say"; `tests/dr44_9_views.rs` is the negative control that a rewritten world does
/// not move a single member of this object.
///
/// | member | source | `null` when |
/// |---|---|---|
/// | `subject` | `payload.transformation` | never |
/// | `tree_size` | `payload.inclusion_proof.tree_size` | no inclusion proof (ASM-14: every `VerdictReceipt`) |
/// | `leaf_index` | `payload.inclusion_proof.leaf_index` | as above |
/// | `root` | `gx_log::proof::root_of_inclusion` over that proof and this receipt's own leaf | as above, or a path that does not land on a root |
/// | `key_id` | `payload.key_id` | never |
/// | `postcondition_fingerprint` | `payload.postcondition_fingerprint` | nothing was applied (42 §3.10) |
/// | `issued_at` | `Receipt.issued_at`, as 44 §0's RFC 3339 | never |
///
/// # 🔴 `root` is a restatement of the proof, not an attestation
///
/// The value is what **this receipt's own audit path reconstructs** from its own leaf — the leaf
/// being `{index, receipt_digest, transformation}` (42 §3.11), each member of which the payload
/// determines (`ReceiptPayload::ledger_digest`). It is not "the root this server holds now". A
/// reader who wants to know whether the two agree fetches `GET /ledger/checkpoint` and compares —
/// which is why the spelling here is `Cid`'s own `gx1:` text and not a second, hex spelling of the
/// same 32 bytes: 42 §1.2 gives a `Cid` one readable form, and a value that could not be compared
/// with `Checkpoint.root_hash` by string equality would be a view that made verification harder.
///
/// # 🔴 There is no `alg`, permanently
///
/// `req/187` §5's list of fields the receipt face reads includes one, and it is the one member of
/// that list this endpoint may never grow. 33 NFR-011's closing note (`req/38` §109, DR-46-5 (b))
/// rules that the algorithm is a property of the **key** and forbids a wire-side alg-like field
/// driving crypto dispatch; `req/38` §113 extended the census from `DsseSignature` to the carriers
/// so that nothing alg-like can ride *beside* a signature, and this object is such a carrier. The
/// reader's answer to "which algorithm" is `key_id`, resolved against the key they pinned.
///
/// # A payload that will not decode
///
/// `null`, and the document is still served. Refusing to hand over a receipt because *this server*
/// could not read it would withhold the very artefact a stranger needs in order to establish that
/// it is malformed.
fn receipt_view(receipt: &gx_witness::Receipt) -> serde_json::Value {
    let Ok(payload) = receipt.payload() else {
        return serde_json::Value::Null;
    };
    // 🔴 **`req/38` §324 ruling 3** — from the signed bytes. `get_receipt` serves documents out of
    // the archive a caller supplied, which is exactly the population that predates this build's
    // schema; the decoded payload's re-encoding would answer for a receipt nobody issued.
    let leaf_digest = receipt.ledger_digest().ok();
    let (tree_size, leaf_index, root) = match (payload.inclusion_proof.as_ref(), leaf_digest) {
        (Some(proof), Some(receipt_digest)) => {
            let leaf = gx_log::LedgerLeaf {
                index: proof.leaf_index,
                receipt_digest,
                transformation: payload.transformation,
            };
            let root = gx_log::proof::root_of_inclusion(proof, &leaf)
                .ok()
                .flatten()
                .map(|root| root.to_text());
            (
                Some(proof.tree_size),
                Some(proof.leaf_index),
                root.map_or(serde_json::Value::Null, Into::into),
            )
        }
        (Some(proof), None) => (
            Some(proof.tree_size),
            Some(proof.leaf_index),
            serde_json::Value::Null,
        ),
        (None, _) => (None, None, serde_json::Value::Null),
    };
    serde_json::json!({
        "subject": payload.transformation.0.to_text(),
        "tree_size": tree_size,
        "leaf_index": leaf_index,
        "root": root,
        "key_id": payload.key_id,
        "postcondition_fingerprint": payload.postcondition_fingerprint,
        // 🔴 The same instant as the top-level `issued_at`, in the other spelling: that one is the
        // raw nanosecond `Timestamp` 42 §1.2 stores, this one is 44 §0's RFC 3339, which §0 makes
        // "the API layer's responsibility". Two spellings, one value, and **neither is signed**
        // (E-M2-6, CM-5) — `tests/dr44_9_views.rs` pins that they name the same instant so the
        // pair cannot drift into two facts.
        "issued_at": rfc3339::of(receipt.issued_at),
    })
}

/// 🔴 **L-02 / `req/38` §369 item 1** (`req/553` L-02, `req/556` R-3c, `req/566` G-2, `req/578` §5,
/// `req/603` §2 ruling (a)) — the health a receipt reader receives **in band**, rather than has to
/// know to ask for.
///
/// # This is a falsifier being honoured, not a new decision
///
/// `docs/LIMITS.md` declares that one server at one instant can answer `500` to `/healthz` and
/// `200` to `GET /receipts/{tid}`, and R40 deliberately did **not** add a member to this response —
/// resting the limit on one sentence: *"a caller who wants to know asks `/healthz`"*. The same
/// paragraph wrote its own falsifier: **if any consumer is observed rendering a served receipt
/// without consulting `/healthz`, this limit becomes a wire change rather than a paragraph.**
/// `req/566` G-2 and `req/578` §5 independently measured that consumer, and it is this
/// repository's own — `sdk/typescript/src/client.ts`'s `getReceipt` goes straight here and calls
/// `healthz()` nowhere. `req/38` §350 item 4 ruled the falsifier fired and §369 item 1 ruled the
/// shape: a fourth additive key, always present.
///
/// # 🔴 Always present, and why the alternative was refused
///
/// The cheaper design is an `Option` mounted only when the server is unwell. `req/603` §2-3 (KA-6)
/// is why it was not taken: an absent key would mean both *"this server is well"* and *"this build
/// does not carry the member"*, which is one key with two preimages — the exact defect DR-46-28
/// closed by refusing `Option<DeterminismBoundary>` beside a first-class `unknown`, and the same
/// shape as the failure being repaired here (a reader who does not know to ask cannot tell silence
/// from health).
///
/// # 🔴 It is outside the signature, by construction and not by care
///
/// The receipt's signature covers DSSE's PAE over `payload_type` and `payload` (42 §3.10), and
/// both are minted in `gx-witness`/`gx-engine`. This crate holds no signing key and contains no
/// PAE, no signer and no `sign` — this value is inserted into a `serde_json::Value` **produced
/// from** an already-signed `Receipt`, so `envelope.payload` is not reachable from here even by
/// mistake. `crates/gx-witness/tests/frozen_receipt_corpus*.rs` read fixtures off disk and never
/// enter this crate; `crates/gx-cli/tests/r40_serving_routes.rs` is the measurement that
/// `envelope` and `issued_at` are byte-for-byte identical either side of a cut while this object
/// is the one member that moves.
///
/// # The four words, and the one that is not a synonym for the others
///
/// | `status` | when | `status_reason` |
/// |---|---|---|
/// | `"ok"` | the two files describe one tree and the writer's door is open | `null` |
/// | `"degraded"` | a `Nature::Meta` file is gone, so writes are refused (`/healthz` = 200) | [`degraded_reason`]'s sentence, shared with `/healthz` |
/// | `"unhealthy"` | `ledger_agrees` is false (`/healthz` = 500 `LEDGER_DISAGREES`) | the counts and the clause that names which difference it is |
/// | `"unknown"` | this server could not read its own journal or ledger to answer | the engine's own refusal |
///
/// 🔴 `"unknown"` is **not** folded into `"unhealthy"`. "The two files disagree" is a finding;
/// "I could not look" is the absence of one, and spelling the second as the first would be a
/// definite claim about a thing nobody observed — which is the sentence this whole product keeps
/// writing (`Presence::Undetermined`, DR-46-34's `ReadsNotJournalled`, DR-46-28's `unknown`).
///
/// # 🔴 What this does **not** do: grade the receipt
///
/// No `verified`, no `refuted`, no `inclusion` — for DR-44-9's reason, unchanged: a server that
/// graded its own receipts would be marking its own paper. This object says what the **server**
/// is, and a reader who has both can tell "a good document from a deployment that is currently in
/// dispute" from "a good document from a well deployment", which is precisely the distinction the
/// SDK could not draw.
///
/// # Staleness, bounded
///
/// Read off [`AppState::health_snapshot`], the same cache `/healthz` reads, whose ceiling is
/// [`crate::state::HEALTH_SNAPSHOT_MAX_AGE`] = **250 ms** — and which is additionally invalidated by
/// the witness (a journal or ledger whose length or mtime moved rebuilds it on the very next
/// probe) and dropped by any write through this server. A fresh read per receipt was considered
/// and refused: it would take the engine's `Mutex` and run `catch_up` on a **read** path that
/// `req/240` M-01 measured at 8.8 ms per lock acquisition on a 400-commit project, to close a
/// window a quarter of a second wide.
fn server_health(state: &AppState) -> serde_json::Value {
    let (status, reason) = match state.health_snapshot() {
        // 🔴 The condition `/healthz` answers **500** to. It is a `200` here, beside a document the
        // server is right to keep serving (`req/553` L-02): the receipt is a signed statement about
        // the past and this server's present dispute does not alter it. What changes is that the
        // reader is now told.
        Ok(snapshot) if !snapshot.agrees => {
            // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated; the same clause `/healthz`
            // and `ledger_disagrees_refusal` print, from the same function.
            let note = crate::journal_and_head_note(
                snapshot.journal_departure,
                snapshot.rolled_back.as_deref(),
            );
            let (journal, ledger) = (snapshot.journal, snapshot.ledger);
            (
                "unhealthy",
                Some(format!(
                    "this project's journal witnesses {journal} commit(s) and its ledger holds \
                     {ledger} leaf/leaves, and `ledger_agrees` is false: the two files are \
                     describing different trees.{note} The receipt beside this object is served \
                     unchanged and is still worth verifying offline — it is a signed statement \
                     about the past, and this server's present dispute does not reach into it — but \
                     this server refuses every write and signs no checkpoint while that holds, so \
                     an inclusion proof compared against the head it is publishing now proves \
                     nothing. `GET /v1/healthz` answers 500 `LEDGER_DISAGREES` for the same \
                     condition (req/38 §156 ruling 2(b), DR-44-6). `gx repair` reports what is \
                     wrong and `gx repair --yes` runs 43 §7's recovery under the project lock \
                     (DR-43-8); `gx replay <ID>` names the rows that differ"
                )),
            )
        }
        Ok(_) => match degraded_reason(state) {
            Some(why) => ("degraded", Some(why)),
            None => ("ok", None),
        },
        // 🔴 The engine could not be read at all. This is deliberately **not** a `500` for the
        // receipt: refusing to hand over a document because *this server* cannot describe its own
        // health would withhold the artefact from the one reader who needs it most, which is the
        // same sentence `receipt_view` writes about a payload that will not decode.
        Err(refusal) => (
            "unknown",
            Some(format!(
                "this server could not read its own journal or ledger to answer, so it does not \
                 say that it is well: {}. The receipt beside this object is unaffected and \
                 verifies offline against the key you pinned; what is unavailable is this \
                 deployment's statement about itself. `GET /v1/healthz` answers the same refusal",
                refusal.detail
            )),
        ),
    };
    serde_json::json!({
        "status": status,
        "status_reason": reason,
    })
}

/// `GET /receipts/{tid}` → the `Receipt` (44 §2.2). `404` for "not yet committed, or does not exist" (sem: SEM-gx-api-152).
///
/// 🔴 Two sources, in order: the engine's live table, then the archive the caller supplied. The
/// second is what makes this endpoint survive a restart — `Engine::open` leaves the in-flight table
/// empty (M5H3-5), so a server restarted after a commit holds no receipt for it — and it is a
/// **caller-supplied** archive because `.gx/receipts/` and its `<TID>.<kind>.json` naming (M6H4-7)
/// are gx-cli's single declaration.
///
/// # Errors
/// `404 NOT_FOUND`.
pub async fn get_receipt(State(state): State<AppState>, Segment(tid): Segment<String>) -> Answer {
    let id = transformation_id(&tid)?;
    let Some(receipt) = receipt_typed(&state, &id) else {
        return Err(ApiError::not_found(format!(
            "no receipt for {}: it has not been committed, or this server holds neither its row \
             nor its archive",
            id.0.to_text()
        )));
    };
    let mut value = serde_json::to_value(&receipt).unwrap_or(serde_json::Value::Null);
    // 🔴 **DR-44-9** — the decoded view, mounted **beside** the document rather than replacing any
    // part of it. `envelope` and `issued_at` are byte-for-byte what they were before this key
    // existed (44 §2.6's "a new optional field"), so an offline verifier reading this answer reads
    // the same two members it always did and ignores the third (DSSE: "Consumers MUST ignore
    // unrecognized fields") — measured in `tests/dr44_9_views.rs` rather than assumed.
    // 🔴 **L-02 / `req/38` §369 item 1** — and the fourth key, on the same terms: mounted beside
    // the document, never folded into it. See [`server_health`] for why it is always present and
    // why it cannot reach the signed bytes.
    if let Some(map) = value.as_object_mut() {
        map.insert("receipt_view".to_string(), receipt_view(&receipt));
        map.insert("server_health".to_string(), server_health(&state));
    }
    ok(StatusCode::OK, value)
}

// ---------------------------------------------------------------------------
// `GET /ledger/proof?leaf=` and `GET /ledger/checkpoint` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's query: "`leaf` accepts a `u64` (leaf index) or `gx1:...` (`TransformationId`, resolved internally
/// to an index)" (sem: SEM-gx-api-153).
#[derive(Debug, Deserialize)]
pub struct ProofQuery {
    /// The leaf, in either spelling.
    pub leaf: String,
}

/// `GET /ledger/proof?leaf=` → an `InclusionProof` (44 §2.2). `404` for "an unknown leaf, or not yet committed" (sem: SEM-gx-api-154).
///
/// # Errors
/// `404 NOT_FOUND`, `422 VALIDATION_ERROR` for a `leaf` that is neither an index nor a `gx1:` id.
pub async fn ledger_proof(State(state): State<AppState>, Params(q): Params<ProofQuery>) -> Answer {
    // 🔴 **DR-43-6 / `req/215` H-05** — a proof about a tree this process last read minutes ago is
    // a proof about the wrong tree. Caught up first, without the project lock.
    let engine = state.engine_refreshed()?;
    let log = engine.ledger().log();
    let index = if let Ok(index) = q.leaf.parse::<u64>() {
        index
    } else {
        let id = transformation_id(&q.leaf)?;
        let Some(entry) = log.entries().iter().find(|e| e.transformation == id) else {
            return Err(ApiError::not_found(format!(
                "{} is not a leaf of this ledger ({} leaves)",
                id.0.to_text(),
                log.len()
            )));
        };
        entry.index
    };
    if index >= log.len() {
        return Err(ApiError::not_found(format!(
            "leaf {index} is outside a tree of {} leaves",
            log.len()
        )));
    }
    // 🔴 **R37 / `req/496` M-04** — the gate `GET /ledger/checkpoint` has had since `req/215` H-01,
    // on the route a buyer uses when they do **not** trust this deployment.
    //
    // Audit 36 cut this project's last journal frame, left the ledger whole, and got the same 46
    // bytes here that a sound project returns, in the same process and the same instant that
    // `checkpoint` was answering `500 LEDGER_DISAGREES` and `GET /candidates/{id}` was answering
    // `state: "Committing"` for this very leaf. The doc-string above transcribes 44 §2.2 with no
    // degradation (`SEM-gx-api-154`) — `404` for an unknown leaf **or one not yet committed** — and
    // an inclusion proof over a leaf whose commit this project's own journal does not witness is
    // exactly the second case.
    //
    // **The position is load-bearing.** It is below the two questions about the caller's argument
    // and above the proof. An unknown leaf and an out-of-range index are facts about the ledger
    // file's own size, which a journal in any state does not change, and they keep their `404` on
    // both sides of a cut — `req/501` §0 declares that as a negative control precisely so that this
    // gate cannot be written as "refuse everything and call it safe".
    //
    // Why this refusal and not the `404`: the two are different sentences. A `404` says this tree
    // has no such leaf, which is false — the leaf is there. This says the two files do not describe
    // one tree, so no answer about *which* tree this is can be honest, which is what happened.
    if !engine.ledger_agrees() {
        return Err(crate::ledger_disagrees_refusal(&engine));
    }
    let proof = gx_log::proof::prove_inclusion(log, index).map_err(|e| ApiError::from_log(&e))?;
    ok(
        StatusCode::OK,
        serde_json::to_value(&proof).unwrap_or(serde_json::Value::Null),
    )
}

/// 🔴 `GET /ledger/checkpoint` → a **signed** `Checkpoint` (44 §2.2: "42 §3.11, **DSSE-signed**"; sem: SEM-gx-api-155).
///
/// **M6-24, adopted (b)**: "`GET /ledger/checkpoint`'s handler calls `unsigned_checkpoint` → `sign_checkpoint`"
/// (sem: SEM-gx-api-156). The key is the server's ([`crate::state::ServerKeys::signing`]) and §47's note is why
/// that is right rather than restrictive: "only the ledger's owner can make one, is 42 §3.11's intent".
///
/// # Errors
/// `404 NOT_FOUND` for an empty log — a tree with no leaves has no head, and answering with a
/// checkpoint over nothing would be a signed statement about a tree that does not exist.
pub async fn ledger_checkpoint(State(state): State<AppState>) -> Answer {
    let at = state.now();
    // 🔴 **DR-43-6 / `req/215` H-01 and H-05** — the two things that have to be true before this
    // handler signs anything, and neither of them was.
    //
    // *Fresh*: `req/215` H-05 ran five CLI commits under a live server and got a **signed**
    // `tree_size: 1` over a ledger holding six leaves, for as long as the server did not write.
    // A signature is a statement, and a stale statement about a growing tree is a wrong one.
    //
    // *Agreeing*: `req/215` H-01 cut the ledger under a live server and got a **signed**
    // `tree_size: 3` over a file with no leaves at all. Being caught up does not fix that -- the
    // journal and the ledger can be read to the end and still describe different trees -- so the
    // question `Session::settle` and `AppState::engine_for_write` ask is asked here too, before the
    // key is touched. A server that cannot say which tree it has does not sign for either.
    // 🔴 **R7 / `req/232` M-08** — *and under the project lock*, because this handler signs.
    //
    // `engine_refreshed` catches up without `.gx/LOCK`, which is right for a read and wrong for the
    // one read that mints a document. Two servers on one project produced a signed `tree_size: 1`
    // over a two-leaf tree with the current timestamp on it; `AppState::engine_for_signing` carries
    // the whole argument and the cost (`BUSY` while another `gx` writes).
    //
    // **The lockless refresh happens first, and the order is load-bearing.** A journal that shrank
    // under a live server is refused by `catch_up` itself — `Error::Malformed`, which maps to
    // `VALIDATION_ERROR` and would blame the caller for the state of the server's disk
    // (`AppState::engine_for_write`'s own note says why that is the wrong word). The lockless read
    // reaches the same fact through `journal_intact`, so asking it first keeps this endpoint's
    // refusal at `LEDGER_DISAGREES` for exactly the states R4 fixed it at
    // (`serve_runtime_r4::s3_a_journal_that_shrank_stops_being_healthy` measures the pair), and the
    // locked catch-up below is then only ever asked about a project that is sound.
    {
        let engine = state.engine_refreshed()?;
        if !engine.ledger_agrees() {
            // 🔴 **R37 / `req/496` M-04** — the sentence moved to `crate::ledger_disagrees_refusal`
            // **unchanged**, because `GET /ledger/proof` and `GET /ledger/consistency` now owe the
            // same refusal and four copies of one question is the shape `req/38` §227 keeps naming.
            // The bytes are what R4/R6/R7/R32 left, so `probes/doubt`'s census and every test that
            // matches this text read the same words.
            return Err(crate::ledger_disagrees_refusal(&engine));
        }
    }
    let engine = state.engine_for_signing()?;
    if !engine.ledger_agrees() {
        // 🔴 `LEDGER_DISAGREES` (500) since `req/38` §156 ruling 2(a); `INTERNAL` before it.
        // 🔴 **R4 / `req/225` H-03** — and it is now also `false` for a journal that was rewritten
        // underneath this process, which is the condition that used to let this handler sign a
        // checkpoint over a tree its own journal no longer described.
        //
        // 🔴 **R6 / DR-43-11** — and for a project that is behind the head it has already
        // published, where the two files agree with each other and with nothing else. The two
        // clauses are joined **before** the outer `format!` rather than inside its arguments:
        // `clippy::format_in_format_args` is right that a nested one is an allocation nobody asked
        // for, and the join has to happen somewhere.
        // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated.
        // 🔴 **R7 / `req/232` H-01** — or the head this binary would not read numbers off.
        // 🔴 **R37 / `req/496` M-04** — and said in one place. This copy and the lockless one above
        // differed only in where their line breaks fell, and `GET /ledger/proof` and
        // `GET /ledger/consistency` now owe the same refusal: four spellings of one sentence is the
        // shape that drifts apart at three of them.
        return Err(crate::ledger_disagrees_refusal(&engine));
    }
    let log = engine.ledger().log();
    if log.is_empty() {
        return Err(ApiError::not_found(
            "this ledger has no leaves, so it has no head to sign (42 §3.11)",
        ));
    }
    let head = gx_log::proof::unsigned_checkpoint(log, state.origin(), at)
        .map_err(|e| ApiError::from_log(&e))?;
    let key = state.keys().signing();
    let signed = gx_witness::dsse::sign_checkpoint(&head, key.signing_key(), key.key_id())
        .map_err(|e| ApiError::from_witness(&e))?;
    ok(
        StatusCode::OK,
        serde_json::to_value(&signed).unwrap_or(serde_json::Value::Null),
    )
}

// ---------------------------------------------------------------------------
// The archive, and the two receipts a request causes
// ---------------------------------------------------------------------------

/// 🔴 **DR-43-1, adopted (a)** — what this server knows about the world `T_o` left behind.
///
/// The engine cannot fetch this for itself (see [`gx_engine::UndoWitness`]): 42 §3.10's
/// `postcondition_fingerprint` lives in the **commit receipt**, and on this surface the receipt
/// that survives a restart lives in the [`crate::ReceiptArchive`] the deployment supplied. Three
/// answers, all of them named rather than folded into a silent "we did not check":
///
/// * the archive holds a receipt whose payload carries a postcondition → `Attested`, and the
///   engine compares;
/// * ~~the archive holds one that does not (a `VerdictReceipt`, or a payload that will not decode) →
///   `Unobservable`, and the undo proceeds as it did before this ruling;~~
/// * ~~the deployment keeps no archive (`NoArchive`) → `Unobservable::NoArchive`, which is the
///   honest name for "this server cannot see past its own memory".~~
///
/// ~~The third answer is the one worth stating out loud: a server without an archive gets **no** CAS
/// after a restart, and that is a property of the deployment rather than of the ruling.~~
///
/// # 🔴 **R3 (`req/38` §160 ruling 2)** — the struck lines are what `req/222` H-01 and H-02 broke
///
/// The two struck bullets said that a missing, unreadable or postcondition-less receipt let the
/// undo through. `req/222` measured what that meant on a real project, and it is worse than the
/// sentence sounds:
///
/// * **H-01**, 3/3 reproduced: commit, let a third party write to the target, `POST …/undo` →
///   `409 PRECONDITION_CHANGED` (correct). Then `rm .gx/receipts/<TID>.commit.json` — **one
///   command** — and the same request answers **`200`**, the third party's bytes are gone, and
///   nothing on the socket or on stderr says a check was skipped. And no attacker is needed:
///   [`archive_commit_receipt`] filed the receipt with `let _ =`, so a read-only directory or a
///   full disk reached the same state on its own.
/// * **H-02**: `cp <T2>.commit.json <T1>.commit.json` — a receipt for a *different* transformation,
///   under `T_o`'s file name — was accepted as `T_o`'s evidence, because nothing verified the DSSE
///   signature and nothing compared `payload.transformation` with the id being undone.
///
/// So this function now answers four questions before it will attest anything, and every "no" is
/// [`gx_engine::UndoWitness::Missing`] — a **refusal** (43 §5.2's `witness-missing` row), not a
/// skip:
///
/// 1. does this deployment keep receipts at all? (`ReceiptArchive::keeps_receipts`);
/// 2. is there a **commit** receipt for this id? (`load_commit`, narrowed from `load` — H-02's
///    second half was the disclosure-order fallback reaching a verdict receipt);
/// 3. does its DSSE signature verify under ~~**this project's** signing key~~ **the key the
///    receipt names** (🔴 **R4 / `req/225` H-02**: 42 §3.10 puts `key_id` in the signed payload so
///    that a document says which hand signed it, and `verify_offline` refuses a receipt whose
///    `key_id` and verifying key disagree. Using `signing()` meant "the key this process signs
///    with **today**", so one `gx key rotate` turned every receipt in the project's history into
///    a bad signature. The struck words are kept because a reader who trusts them will look for
///    the bug in the archive);
/// 4. does its payload say it is about **this** transformation?
///
/// Only then is `postcondition_fingerprint` read, and its absence is the one answer that is still
/// [`gx_engine::Unobservable`]: an authentic commit receipt this server signed, which says nothing
/// was observed when the change was applied, is `req/38` §123 ruling 1's tools-only face — a fact
/// about the *substrate*, declared rather than refused.
///
/// # What is deliberately not checked
///
/// Inclusion. `verify_offline` is called with `anchor = None`, so [`gx_witness::Checks::inclusion`]
/// is `Unanchored` and this function does not read it. The question here is "did this project sign
/// this document about this transformation", which is what makes the fingerprint evidence; whether
/// the leaf is still in the tree is `ledger_agrees`'s question and is asked on every write by a
/// different gate. Folding the two would make an undo refuse on a torn ledger with a different
/// sentence than every other verb gives for the same fact. (GO condition 6 / `req/222` §4 measured
/// that anchored verification of a non-latest leaf answers `refuted` today; a CAS that consulted it
/// would refuse every undo but the newest.)
fn undo_witness(state: &AppState, id: &TransformationId) -> gx_engine::UndoWitness {
    use gx_engine::{UndoWitness, Unobservable, WitnessMissing};
    if !state.archive().keeps_receipts() {
        return UndoWitness::Missing(WitnessMissing::NoArchive);
    }
    let Some(receipt) = state.archive().load_commit(id) else {
        return UndoWitness::Missing(WitnessMissing::NoReceipt);
    };
    // 🔴 **R4 / `req/225` H-02** — decode first, to learn which key the document names, then check
    // the signature over the raw envelope bytes as before.
    //
    // `verify_offline`'s own header argues for checking the signature before parsing anything, and
    // that ordering is preserved *inside* it: this decode is a second, separate read whose only
    // product is a key id, and every outcome of it that is not a key this deployment holds is a
    // refusal — an undecodable payload is `Unreadable`, an unknown id is `UnknownKey`. So nothing
    // is admitted on the strength of a payload that has not had its signature checked.
    let named = match receipt.payload() {
        Ok(payload) => payload.key_id,
        Err(_) => return UndoWitness::Missing(WitnessMissing::Unreadable),
    };
    let Some(key) = state.keys().verifier(&named) else {
        return UndoWitness::Missing(WitnessMissing::UnknownKey);
    };
    if gx_witness::verify_offline(&receipt, &key.verifying(), None).is_err() {
        return UndoWitness::Missing(WitnessMissing::Unsigned);
    }
    match receipt.payload() {
        Ok(payload) if payload.transformation != *id => {
            UndoWitness::Missing(WitnessMissing::WrongSubject)
        }
        Ok(payload) => match payload.postcondition_fingerprint {
            Some(bytes) => UndoWitness::Attested(bytes),
            None => UndoWitness::Unobservable(Unobservable::NoPostcondition),
        },
        Err(_) => UndoWitness::Missing(WitnessMissing::Unreadable),
    }
}

/// 🔴 **R3 / `req/222` M-12** — what the CAS did, as a word the answer carries.
///
/// `req/222` M-12: "neither the receipt nor the response says whether the CAS ran". A `200` from
/// `POST …/undo` meant "the inverse was applied" and said nothing about whether anything had been
/// compared first, so a GUI could not tell a verified undo from an unverified one — and
/// `req/188` §9-2's tip ("a third party can verify that it was put back") does not survive that.
///
/// Two words, and they are the two the engine judged on: `attested`, or `unobservable:<reason>`.
/// There is no third, because [`gx_engine::UndoWitness::Missing`] does not reach a `200` at all
/// since R3 — it is `409 PRECONDITION_CHANGED`, and the refusal's own detail names the reason.
fn witness_word(witness: &gx_engine::UndoWitness) -> String {
    // 🔴 **DR-46-45 (`req/973` §B-1)** — delegated rather than spelled here. The three arms used to
    // live in this function and an identical three had to be written again for CLI stdout and a
    // third time for the signed payload; `gx_engine::UndoWitness::word` is now the one place the
    // sentence is minted, so "the surfaces agree" is a fact about the call graph rather than
    // something a reader checks by eye. The strings are byte-identical to the ones this function
    // returned before the move — `crates/gx-api/tests/wire_census.rs` is what says so.
    witness.word()
}

/// File the last verdict receipt this transformation issued, under `slot`.
///
/// 🔴 Best effort, and the asymmetry with [`crate::idempotency::IdempotencyStore::put`] is
/// deliberate: a receipt the archive could not take is still in the engine's table and still in the
/// response, so failing the request would refuse a commit that happened. The idempotency record is
/// the opposite — losing it changes what a **retry** does.
pub(crate) fn archive_last_verdict_receipt(
    state: &AppState,
    id: &TransformationId,
    slot: ReceiptSlot,
) {
    let receipt = {
        let engine = state.engine();
        engine.verdict_receipts(id).last().cloned()
    };
    if let Some(receipt) = receipt {
        // 🔴 **R3** — best effort, and no longer silent (`req/222` H-01's second complaint). See
        // [`archive_commit_receipt`] for why this one may fail and that one may not.
        //
        // 🔴 **R16 / `req/262` H-01** — a value rather than a panic ([`crate::notes`]), and 🔴
        // **R16 / `req/262` M-01** — the road it names is one this binary has. It said
        // `gx receipt export`, and `gx receipt` takes `show` and `verify` and nothing else
        // (measured: `error: unrecognized subcommand 'export'`, exit 1). `req/227` M-04's standing
        // rule is that a remedy naming something that does not exist is worse than no remedy.
        if let Err(why) = state.archive().store(id, slot, &receipt) {
            crate::api_note!(
                "gx serve: the receipt archive would not hold the {} receipt for {}: {why}. The \
                 transition stands and nothing gates on this document; \
                 `gx repair --yes --signing-key <KEY_ID> --reissue-receipts` files what it can \
                 once the path will take a file again",
                slot.tag(),
                id.0.to_text(),
            );
        }
    }
}

/// 🔴 **R3 / `req/222` H-01(a)** — T-11's commit receipt, filed as part of the commit.
///
/// # Why this one is not best effort, and its neighbour still is
///
/// The `let _ =` this replaces is the reason `req/222` H-01 says "no attacker is needed". After
/// DR-43-1 the commit receipt is not only a document the caller may want later; it is **the
/// evidence the undo's CAS runs against**, and a commit that quietly did not file one left a row
/// whose undo could not be checked — which, before R3, meant an undo that would fire unchecked over
/// anybody's changes. Two silent failures composing into a loud one.
///
/// So the write is part of the transition's obligations and its failure is the request's failure.
/// The commit itself has already happened — the journal, the ledger and the receipt are all real,
/// and this function cannot and must not undo them — so what the `500` says is exactly that: the
/// change was applied and its receipt could not be filed, which is a state an operator has to know
/// about because [`undo_witness`] will refuse to take it back until the receipt is there.
/// 🔴 **R16 / `req/262` M-01** — and the road the answer names is now one that exists and one this
/// directory's own declaration supports. This paragraph used to read "`.gx/receipts/` is
/// `Nature::Derived` in req/56 §2 — `gx receipt export` can refile it": both halves were false.
/// `crates/gx-cli/src/layout.rs`'s `GX_PATHS` gives `receipts` `Nature::Source`, with the reason
/// written beside it ("re-issuing needs the verdict summary, the proof digest and both
/// fingerprints, and those live in the table `open` does not rebuild… losing this directory loses
/// receipts"), and `gx receipt` has exactly two subcommands, `show` and `verify`. The road that was
/// measured to work on the project this refusal comes from is
/// `gx repair --yes --signing-key <KEY_ID>`, and `--reissue-receipts` beside it files what the
/// live engine still holds.
///
/// [`archive_last_verdict_receipt`] keeps its `let _ =`, and the asymmetry is now a judgement
/// rather than an oversight: a verdict receipt is a disclosure and nothing gates on it. R3 narrowed
/// [`crate::ReceiptArchive::load_commit`] to the commit slot precisely so that a verdict receipt
/// can never stand in as evidence, which is what makes it safe for it to be missing. It is still
/// said out loud on stderr — `req/222` H-01's other complaint was silence.
///
/// # Errors
/// `INTERNAL` (500) naming the transformation and what the archive said.
fn archive_commit_receipt(state: &AppState, id: &TransformationId) -> Result<(), ApiError> {
    let receipt = {
        let engine = state.engine();
        engine.receipt(id).cloned()
    };
    let Some(receipt) = receipt else {
        return Ok(());
    };
    state
        .archive()
        .store(id, ReceiptSlot::Commit, &receipt)
        .map_err(|why| {
            ApiError::new(
                "INTERNAL",
                "the commit was applied and its receipt could not be filed",
                format!(
                    "{} committed — the journal, the ledger and the signed receipt all exist — but \
                     the receipt archive would not take it: {why}. Until it is filed, this \
                     transformation cannot be undone: DR-43-1's compare-and-set reads the archived \
                     commit receipt, and R3 (req/38 §160) refuses an undo whose precondition it \
                     cannot check rather than firing one blind (req/222 H-01). `.gx/receipts/` is \
                     req/56 §2's `Source` — losing it loses receipts, so nothing re-derives them \
                     from nothing. What to fix: read the sentence in brackets above, which is what \
                     the operating system said, and clear whatever it names; then \
                     `gx repair --yes --signing-key <KEY_ID>` (its `--reissue-receipts` files what \
                     this engine still holds). `gx repair` on its own reports the state and \
                     changes nothing",
                    id.0.to_text(),
                ),
            )
        })
}
