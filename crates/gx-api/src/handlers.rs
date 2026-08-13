//! 44 §2.2's endpoints — the thirteen that are not `GET /stream`.
//!
//! # 🔴 Every handler is the same shape, and that is 則 1 (req/88 §3 Λ1)
//!
//! Parse the request, take the engine's lock, call one of 43's eight entry points, read accessors,
//! serialise. No canonical encode (41 §6), no `Verdict` constructed (41 §4), no `Lifecycle` written
//! (42 §1.3-3). `crates/gx-canon/tests/authority_boundary.rs` scans this directory for all three,
//! and req/88 §3 Λ1 says why the temptation is sharper here than in the CLI: 「an HTTP handler is
//! where 『just compute the answer here, it is faster』 is most tempting」 — `GET /candidates/{id}`
//! returns four fields and every one of them has an engine accessor already.
//!
//! # 🔴 The three asymmetries with 44 §1, written down rather than discovered
//!
//! 1. **`POST /candidates` is `submit` + `plan` in one call.** 44 §2.1 says so — 「内部的にDraft→
//!    Candidate を atomically遷移し、Draft単独状態はHTTP層では観測不可」 — and §0 exempts it from the
//!    id-resolution rule for that reason. This is why the server needs no `.gx/drafts/`: req/88 §3
//!    Λ2's counter-example is the CLI's alone.
//! 2. **Receipts are signed by the server's key, not the actor's.** See [`crate::state`]'s header
//!    (E-M6-7). AC-055 does not measure the key and the difference is deliberate.
//! 3. **`goal` is bytes here as it is in the CLI, and 44 §2.2 types it as an object.** See
//!    [`CreateCandidate`].
//!
//! # 🔴 What 44 §2.2 asks for that this hand answers differently, and why
//!
//! `POST /candidates/{id}/verify` is specified as `202 Accepted` with 「非同期。結果は…ポーリング
//! または`GET /stream`」, and ASM-44-2 permits the other reading in as many words: 「同期完了が短時間
//! で得られる実装は`200`＋最終`state`・`verdict`を即時返してよい（クライアントは両方のレスポンス
//! コードを扱える必要がある）」. This surface is synchronous because M6-06 採(a) made every request
//! hold one lock — there is no second thread for a 202 to be answered *by*. Returning 202 and then
//! doing the work anyway would be a status describing a concurrency this build does not have.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use gx_core::{
    Actor, ChangeContext, GoalBytes, Intent, SubstrateKind, TransformationId, VerdictKind,
};
use gx_engine::store::InverseStatus;
use gx_engine::{reconstruct, HumanRuling, Lifecycle};
use serde::Deserialize;

use crate::extract::{Params, Payload};
use crate::problem::ApiError;
use crate::state::AppState;
use crate::{rfc3339, ReceiptSlot};

/// The answer type every handler returns.
type Answer = Result<Response, ApiError>;

// ---------------------------------------------------------------------------
// Shared readings
// ---------------------------------------------------------------------------

/// 42 §1.2's `gx1:<base32>`, parsed and never minted (則 1 (i)).
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
/// `gx_cli::pipeline::is_committing` records: 則 1 (iii)'s scanner reads `Lifecycle::` to the right
/// of an `=` as 「the surface mints a state」, and it is right to be coarse.
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
/// posture does not answer for them (INV-S6: 「`Escalated`はT-5/T-5bの署名済み人間裁定receiptを経由
/// せずに`Admitted`/`Denied`へ自動遷移しない」), so widening this to 「anything that is not
/// `Admitted`」 would turn a pending human decision into an applied change.
fn is_denied(state: Lifecycle) -> bool {
    matches!(state, Lifecycle::Denied)
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
/// The type has no `Serialize` on purpose (gx-gate: 「Deriving `Deserialize` in particular would open
/// a second door into the struct that skips `AdmitProof::new`」), so the five public accessors are
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
// `GET /healthz` — 44 §2.2, the one endpoint outside the Bearer guard
// ---------------------------------------------------------------------------

/// `GET /healthz` → `{ "status": "ok", "engine_version": "<string>" }`, unauthenticated (44 §2.6).
///
/// 🔴 **M6H5-12 採(a)**, implemented in hand 7 (§52: 「版 accessor は配布物の手で」).
///
/// Hand 5 answered this with `env!("CARGO_PKG_VERSION")` — **this crate's** version — and raised the
/// borrow. 41 §2 keeps gx-engine and gx-api at one version in one workspace, so the two strings are
/// equal today, which is exactly why the borrow was invisible. The field is named `engine_version`,
/// so the engine answers it: [`gx_engine::Engine::version`].
///
/// Why it was worth a hand rather than left as a note: 47 §4's upgrade runbook makes 「journal schema
/// は`gx replay`による決定的リプレイが新旧バイナリ間で一致すること」 an operator's **pre-upgrade
/// check**, and an operator performs that check against a version number read off a running server.
/// A number reported by the wrong crate is a check performed against the wrong thing — the day the
/// two crates version apart, and not before.
pub async fn healthz() -> Answer {
    ok(
        StatusCode::OK,
        serde_json::json!({
            "status": "ok",
            "engine_version": gx_engine::Engine::<crate::state::RequestEvidence>::version(),
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
    /// 🔴 44 §2.2 types this 「object（adapter定義）」 and **E-M6-11** rules the CLI's `--intent` body
    /// 「adapter が解釈する byte 列」 (P-6: interpretation belongs to the adapter).
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
    /// 44 §2.2: 「既定0」. v0.1 produces only 0 — see [`create_candidate`].
    #[serde(default)]
    pub order: u8,
    /// 44 §2.2: 「既定`[]`」.
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
/// > intent → candidate（submit+plan相当を1呼び出しで実行。内部的にDraft→Candidateを atomically遷移し、
/// > Draft単独状態はHTTP層では観測不可）
///
/// The two calls happen under **one** hold of the engine lock, which is what makes 「atomically」
/// true rather than aspirational: a lock released between them would let another request observe a
/// `Draft`, and 44 §2.1 says that state is not observable here.
///
/// `order` and `parents` are **refused** rather than dropped when non-default, for M6H3-5's reason
/// one surface along: 42 §3.3's `Intent` has no room for either and `plan` fixes `order = 0`, so a
/// value passed here would reach nothing. 「an argument that changes nothing must not look like an
/// argument that worked」 (M4H5-5).
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
        // 🔴 One hold, two transitions. 44 §2.1's 「atomically」.
        let mut engine = state.engine();
        engine
            .submit(&intent, seed, at)
            .map_err(|e| ApiError::from_engine(&e))?;
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
            // 🔴 The state the engine holds, not 44's literal 「Candidate」. `gx plan` made the same
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
/// 44 §2.1's division: 「`/candidates`はワークフロー制御面…`/transformations`は恒久記録の読み取り面」.
/// The same id can name one object through both and the two answer different questions, which is why
/// this is not one handler with a flag.
///
/// # Errors
/// `404 NOT_FOUND` for an id this engine has never planned.
pub async fn get_candidate(State(state): State<AppState>, Path(id): Path<String>) -> Answer {
    let id = transformation_id(&id)?;
    let (transformation, lifecycle, verdict) = {
        let engine = state.engine();
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
    ok(
        StatusCode::OK,
        serde_json::json!({
            "transformation": transformation,
            "state": lifecycle,
            "verdict": verdict,
            "fingerprint": fingerprint_json(&state, &id),
        }),
    )
}

/// `GET /transformations/{id}` → `{ transformation, state, receipt, superseded_by }` (44 §2.2).
///
/// # Errors
/// `404 NOT_FOUND`.
pub async fn get_transformation(State(state): State<AppState>, Path(id): Path<String>) -> Answer {
    let id = transformation_id(&id)?;
    let (transformation, lifecycle, superseded_by) = {
        let engine = state.engine();
        (
            serde_json::to_value(engine.transformation(&id)).unwrap_or(serde_json::Value::Null),
            engine.state(&id),
            engine.superseded_by(&id).map(|t| t.0.to_text()),
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
    /// DR-2's per-request posture — **M6-08 採(a)**'s argument, never a field on shared state.
    #[serde(default)]
    pub record_only: Option<bool>,
}

/// 🔴 `POST /candidates/{id}/verify` (44 §2.2) — T-3 → T-4, synchronously (ASM-44-2).
///
/// # 🔴 `record_only` is the argument M6-08 ruled and `evidence` is the field it did not
///
/// 44 §2.2's body carries both. `record_only` reaches `Engine::verify` as a parameter, which is
/// **M6-08 採(a)** and exists exactly so that this handler is correct: (b) — 「serve が request ごと
/// に `&mut self` で mode を差し替える」 — was ruled 「採ってはならない」 because a posture written onto
/// shared state leaks into the next request, and a leaked `RecordOnly` is a fail-open.
///
/// `evidence` has no such argument, so it goes through [`crate::state::RequestEvidence`] — the shape
/// M6-08(b) forbids, made safe by the lock M6-06(a) adopted, and raised as **M6H5-6** because 「safe
/// because of another ruling」 is a dependency and not a design. The cell is cleared on **every**
/// exit, including the refusing ones.
///
/// # Errors
/// `404`, `409 INVALID_STATE` (44 §2.2: 「既にVerifying/終端状態」).
pub async fn verify_candidate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Payload<VerifyRequest>>,
) -> Answer {
    let id = transformation_id(&id)?;
    let Payload(body) = body.unwrap_or_default();
    let mode = body
        .record_only
        .and_then(|on| on.then_some(gx_core::EnforcementMode::RecordOnly));
    let at = state.now();

    let outcome = {
        let mut engine = state.engine();
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
            // names as the second consumer (「problem `detail`」). A `Deny` whose reasons were
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
/// # 🔴 [DR-2感度] — **E-M6-20**, implemented here
///
/// 44 §2.2: 「record-onlyモードではこのケースでも`200`＋`enforced:false`のReceiptを返す」. Hand 5 could
/// not produce that answer from any HTTP request and raised it (M6H5-8); §52 ruled it —
///
/// > **E-M6-20(M6H5-8 採(a))**: 44 §2.2 の commit body に `record_only: bool` を足して読む
/// > (E-M6-10 の HTTP 版・[DR-2感度] 段落の実行可能化)。実装窓=手6 以降の最初に触る手
///
/// — and this is that hand. The flag reaches T-8r as an **argument to `Engine::canonicalize`**, the
/// shape M6-08 採(a) already chose for `verify`: the alternative, a mode written onto the shared
/// engine for the length of a request, is the form §47 ruled 「採ってはならない」 because it leaks into
/// the next caller. The engine's signature moved for it, gx-cli's `gx commit --record-only` now takes
/// the same road, and one spelling drives both surfaces.
///
/// 🔴 **The body is optional and its absence is not `false`.** A commit with no body at all keeps the
/// engine's posture, which is what every client written against 44 before this addition sends;
/// `{"record_only": false}` **overrides** a `RecordOnly` engine for this call. 「未指定」 and 「no」 are
/// different requests and `Option<bool>` is how the difference survives (M6H5-11's shape, one field
/// along).
///
/// # Errors
/// The four above, plus `404` and `409 IDEMPOTENCY_CONFLICT` (44 §2.4).
pub async fn commit_candidate(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
    // 🔴 The cached request body carries the posture. 44 §2.4's conflict is 「同一キーで異なるリクエスト
    // 本文」, and two commits of one transformation under two postures are two different requests —
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
        let mut engine = state.engine();
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
            // (「同一実行の再試行は自然に冪等」) and the two surfaces answering differently about one
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
    archive_commit_receipt(&state, &id);
    committed_answer(&state, &id, at, key.as_deref(), &request_body)
}

/// 44 §2.2's `200 OK` for a committed transformation — the receipt, and 44 §2.4's record beside it.
///
/// One function for the fresh commit and for the re-entrant one, because 43 T-9 makes them one
/// answer: 「the transition applied」 and 「the transition had already applied」 are the same state and
/// the same receipt, and two spellings of the body would let a retry look different from the call it
/// is retrying — the one thing 44 §2.4 exists to prevent.
fn committed_answer(
    state: &AppState,
    id: &TransformationId,
    at: gx_core::Timestamp,
    key: Option<&str>,
    request_body: &serde_json::Value,
) -> Answer {
    // 44 §2.2: 「Response `200 OK`（Committed成功）: `Receipt`（42 §3.10のJSON表現、`payload`は
    // base64）」. The receipt's own `Serialize` produces that form; the fields beside it are what a
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
                "44 §2.2: 「`403`（非record-onlyかつVerdict≠Admit）」. 43 T-8's from-state is \
                 `Admitted`; T-8r opens the same door under `EnforcementMode::RecordOnly` and this \
                 server is not in it. {e}"
            ),
        ),
        Some(Lifecycle::Escalated) => ApiError::new(
            "ESCALATION_PENDING",
            "a person has not ruled on this transformation yet",
            format!(
                "44 §2.2: 「`409 Conflict`（対象が`Escalated`のまま）」. INV-S6: 「`Escalated`はT-5/\
                 T-5bの署名済み人間裁定receiptを経由せずに`Admitted`/`Denied`へ自動遷移しない」. {e}"
            ),
        ),
        _ => ApiError::from_engine(e),
    }
}

/// The status 44 §2.2 gives a commit that reached `Aborted` rather than `Committed`.
fn aborted_refusal(state: &AppState, id: &TransformationId, lifecycle: Lifecycle) -> ApiError {
    let detail = {
        let engine = state.engine();
        format!(
            "{} is {} after 43 T-9..T-11; rollback: {:?}",
            id.0.to_text(),
            lifecycle.name(),
            engine.rollback(id)
        )
    };
    match lifecycle {
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
    }
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
/// **E-M6-15** made `--actor-key` required on the CLI verb and INV-S6 is the reason: 「裁かれる側が
/// 自分を承認する既定値は存在しない」. The HTTP form of that ruling is that the request **carries** the
/// ruler's key id and the server looks it up ([`crate::state::ServerKeys::ruler`]) with no fallback:
/// a ruling signed with the server's own key would record that the server allowed the change, and a
/// ruling signed with the submitter's key would record that the party being ruled on approved
/// themselves.
///
/// 44 §0's id-resolution is honoured one verb further out, as **M6-04 採(c)**: a `TicketId` is
/// accepted where 44 §2.2 writes `{id}`, which removes the asymmetry between the CLI's `<TICKET_ID>`
/// and this path parameter.
///
/// # Errors
/// `404`, `409 INVALID_STATE` (「対象が`Escalated`でない」), `422 VALIDATION_ERROR` (「`reason`欠落等」,
/// and an `actor` this server holds no key for).
pub async fn escalate_candidate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Payload(body): Payload<EscalationRequest>,
) -> Answer {
    let decision = match body.decision.as_str() {
        "approve" => VerdictKind::Admit,
        "reject" => VerdictKind::Deny,
        other => {
            return Err(ApiError::validation(format!(
                "`decision` is 「approve|reject」 (44 §2.2); got {other:?}"
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
             guard is 「裁定者が有効な署名鍵を保持」 and E-M6-15 ruled that 「裁かれる側が自分を承認 \
             する既定値は存在しない」. Signing with the server's own key would record that the \
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
        let mut engine = state.engine();
        // 44 §0's id-resolution, M6-04 採(c)'s extension: a ticket id first, because 43 T-4c makes
        // the two 1:1 and the CLI's synopsis names the ticket.
        let named = gx_gate::TicketId(cid);
        let transformation = match engine
            .transformation_of_ticket(&named)
            .map_err(|e| ApiError::from_engine(&e))?
        {
            Some(found) => found,
            None => TransformationId(cid),
        };
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
    /// 44 §2.2: 「オーナー権限を持つactorによる明示的キャンセル」. See [`cancel_candidate`] for what is
    /// and is not checked about that sentence.
    pub actor: Actor,
}

/// 🔴 `POST /candidates/{id}/cancel` (44 §2.2) — T-7, with **E-M6-1**'s from-set and no owner check.
///
/// 44 L101 lists `Draft` first and req/38 §47 M6-03 採(c) removed it; on this surface the point is
/// moot for a reason 44 §2.1 states itself — a `Draft` is not observable here.
///
/// # 🔴 The `actor` in the body is recorded nowhere and checked against nothing
///
/// 43 T-7's guard is 「actorがオーナー権限（`Actor::Human{key}`相当）を保持」 and v0.1 has no
/// authorization layer (M5H6-4 採(a)): `Engine::cancel` takes no actor, 43 T-7's `Aborted` record
/// has no field for one, and this surface's only check is the Bearer token
/// ([`crate::auth::ABSENCE_NOTICE`]). 44 §2.2 requires the field, so it is required and its fate is
/// said out loud rather than implied — 還流台帳 §1.2: 「authorization 0 (cancel は誰でも押せる)」.
///
/// # Errors
/// `404`, `409 INVALID_STATE` (44 §2.2: 「既に`Committing`以降または終端状態」).
pub async fn cancel_candidate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Payload(body): Payload<CancelRequest>,
) -> Answer {
    let id = transformation_id(&id)?;
    let _ = &body.actor;
    let at = state.now();
    let lifecycle = {
        let mut engine = state.engine();
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
            // 44 §2.2's own shape: 「`{ transformation, state: "Aborted", reason: "OwnerCancelled" }`」
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
/// 43 §5-2: 「undoであっても検証を免除されない」, so the inverse walks `Draft→…→Committed` on its own
/// feet and this drives the same four steps `gx undo` drives.
///
/// # Errors
/// `409 INVERSE_UNAVAILABLE` when 42 §3.12's status is not `Available` — checked **before** the
/// pipeline is entered, because 44 §2.2 names that status and the engine's own refusal for it is a
/// `NotFound` that would answer 404 about a transformation that exists. `404` for an unknown id.
pub async fn undo_transformation(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
    let undoing = {
        let mut engine = state.engine();
        if engine.state(&id).is_none() {
            return Err(ApiError::not_found(format!(
                "{} is not a transformation this engine holds",
                id.0.to_text()
            )));
        }
        match engine.inverse_status(&id) {
            Some(InverseStatus::Available) => {}
            other => {
                return Err(ApiError::new(
                    "INVERSE_UNAVAILABLE",
                    "there is no escrowed inverse to commit",
                    format!(
                    "44 §2.2: 「`409`（`InverseStatus`が`Unavailable`/`Expired`/`Consumed`）」. \
                         42 §3.12 says this one is {}",
                    other.map_or("absent".to_string(), |s| s.kind().to_string())
                ),
                ))
            }
        }
        let (_, undoing) = engine
            .undo(&id, seed, at)
            .map_err(|e| ApiError::from_engine(&e))?;
        let verified = engine
            .verify(&undoing, at, state.keys().signing(), None)
            .map_err(|e| ApiError::from_engine(&e))?;
        // 🔴 **FR-M7-1** (裁定 #4, `req/98` §3-2, confirmed `req/38` §57) — DR-2's record-only road,
        // on the row 44 §2.1 gives the same `[DR-2感度]` ✓ as `/commit`.
        //
        // 44 §2.2 says of a commit the gate refused: 「record-onlyモードではこのケースでも`200`＋
        // `enforced:false`のReceiptを返す」, and until this hand `/undo` had no such road at all —
        // it required `Admitted` and answered `403` in every posture. The fix batch measured that
        // and refused to patch it, because 「changing it would give `undo` a new semantics, which is
        // a ruling」 (req/97 §6 M6FIX-2). 裁定 #4 is the ruling: 「record-only posture の `/undo` は、
        // undo 要求を record し `enforced:false` で応答する(commit と対称)」.
        //
        // Nothing new is *decided* here, which is why the condition is this small: the road already
        // exists one layer down. `Engine::canonicalize` admits `Denied` when the effective mode is
        // `RecordOnly` (T-8r) and stamps `enforced=false`, so all this branch does is stop refusing
        // before the engine is asked. The two narrowings are deliberate:
        //
        // * **`Denied` only** ([`is_denied`]). `Escalated` is a person's queue, not a posture's.
        // * **the engine's posture only.** `/commit` also takes a per-call `record_only` in its
        //   body (E-M6-20); FR-M7-1's AC is written about 「posture=`RecordOnly` の server」 and this
        //   hand did not widen it, because a per-call axis on `/undo` is a **wire change** to 44
        //   §2.2's undo body and belongs to whoever rules on it. Raised in `req/99` §residue.
        let recording = engine.mode() == gx_core::EnforcementMode::RecordOnly;
        if !is_admitted(verified) && !(recording && is_denied(verified)) {
            // 🔴 The guard is dropped **before** the error is built. `halted_undo` takes a lock of
            // its own (deliberately: it reads the state back rather than carrying a `Lifecycle`
            // across a parameter list, which 則 1 (iii) reads as a state table), and
            // `std::sync::Mutex` is not re-entrant — so calling it with this guard alive is a
            // deadlock, and under M6-06 採(a)'s single `Arc<Mutex<Engine>>` it is a deadlock that
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
        undoing
    };

    archive_last_verdict_receipt(&state, &undoing, ReceiptSlot::Verdict);
    archive_commit_receipt(&state, &undoing);

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
            // 🔴 **M6H8-14 ④ 採(a)** (req/38 §55). 44 §2.1 puts the same DR-2 mark (✓) on `/undo`
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
    // 🔴 The state is **read from the engine** rather than passed in, and the reason is one 則 1
    // (iii) found rather than one this hand chose: `authority_boundary.rs` reads 「a declaration
    // whose type mentions `Lifecycle` and ends in a comma」 as 「the surface keeps a state table」,
    // and a parameter list is exactly that shape once rustfmt puts one argument per line. The probe
    // is right to be coarse, so the code moved — and reading the state back is what every other
    // handler does anyway. gx-cli's `lifecycle::halted` hit the identical scanner and drew the
    // identical conclusion.
    let lifecycle = state.engine().state(undoing);
    let detail = {
        let engine = state.engine();
        format!(
            "the inverse {} of {} reached {:?} rather than Committed; 43 §5-2: 「undoであっても \
             検証を免除されない」. verdict: {:?}",
            undoing.0.to_text(),
            original.0.to_text(),
            lifecycle,
            engine.verdict(undoing)
        )
    };
    match lifecycle {
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
    }
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
/// **E-M5-2** fixed replay as 「`Σ` のみを再構成する read-only 操作」 and **M6-26 採(a)** fixed what
/// `diffs` can be: 「`matches` は byte 一致で答え、`diffs` は『不一致の時に最初に食い違った成分名』の
/// 列」. Three of Σ's four components have no second copy anywhere, so the comparison that means
/// something is the ledger's — and `unchecked` is the denominator that keeps `matches: true` from
/// reading as 「everything agreed」.
///
/// # Errors
/// `404`, `422 VALIDATION_ERROR` (44 §2.2: 「journal欠落等でリプレイ不能」).
pub async fn replay_transformation(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
            // The denominator. `matches: true` means 「the one component with a second copy agreed」.
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
    state
        .archive()
        .load(id)
        .map_or(serde_json::Value::Null, |r| {
            serde_json::to_value(r).unwrap_or(serde_json::Value::Null)
        })
}

/// `GET /receipts/{tid}` → the `Receipt` (44 §2.2). `404` for 「未commitまたは存在しない」.
///
/// 🔴 Two sources, in order: the engine's live table, then the archive the caller supplied. The
/// second is what makes this endpoint survive a restart — `Engine::open` leaves the in-flight table
/// empty (M5H3-5), so a server restarted after a commit holds no receipt for it — and it is a
/// **caller-supplied** archive because `.gx/receipts/` and its `<TID>.<kind>.json` naming (M6H4-7)
/// are gx-cli's single declaration.
///
/// # Errors
/// `404 NOT_FOUND`.
pub async fn get_receipt(State(state): State<AppState>, Path(tid): Path<String>) -> Answer {
    let id = transformation_id(&tid)?;
    let value = receipt_value(&state, &id);
    if value.is_null() {
        return Err(ApiError::not_found(format!(
            "no receipt for {}: it has not been committed, or this server holds neither its row \
             nor its archive",
            id.0.to_text()
        )));
    }
    ok(StatusCode::OK, value)
}

// ---------------------------------------------------------------------------
// `GET /ledger/proof?leaf=` and `GET /ledger/checkpoint` — 44 §2.2
// ---------------------------------------------------------------------------

/// 44 §2.2's query: 「`leaf`は`u64`（leaf index）または`gx1:...`（`TransformationId`、内部でindexへ
/// 解決）を受理する」.
#[derive(Debug, Deserialize)]
pub struct ProofQuery {
    /// The leaf, in either spelling.
    pub leaf: String,
}

/// `GET /ledger/proof?leaf=` → an `InclusionProof` (44 §2.2). `404` for 「leaf不明・未commit」.
///
/// # Errors
/// `404 NOT_FOUND`, `422 VALIDATION_ERROR` for a `leaf` that is neither an index nor a `gx1:` id.
pub async fn ledger_proof(State(state): State<AppState>, Params(q): Params<ProofQuery>) -> Answer {
    let engine = state.engine();
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
    let proof = gx_log::proof::prove_inclusion(log, index).map_err(|e| ApiError::from_log(&e))?;
    ok(
        StatusCode::OK,
        serde_json::to_value(&proof).unwrap_or(serde_json::Value::Null),
    )
}

/// 🔴 `GET /ledger/checkpoint` → a **signed** `Checkpoint` (44 §2.2: 「42 §3.11、**DSSE署名込み**」).
///
/// **M6-24 採(b)**: 「`GET /ledger/checkpoint` の handler が `unsigned_checkpoint` → `sign_checkpoint`
/// を呼ぶ」. The key is the server's ([`crate::state::ServerKeys::signing`]) and §47's note is why
/// that is right rather than restrictive: 「作れるのは台帳の持ち主だけ、が 42 §3.11 の意図」.
///
/// # Errors
/// `404 NOT_FOUND` for an empty log — a tree with no leaves has no head, and answering with a
/// checkpoint over nothing would be a signed statement about a tree that does not exist.
pub async fn ledger_checkpoint(State(state): State<AppState>) -> Answer {
    let at = state.now();
    let engine = state.engine();
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

/// File the last verdict receipt this transformation issued, under `slot`.
///
/// 🔴 Best effort, and the asymmetry with [`crate::idempotency::IdempotencyStore::put`] is
/// deliberate: a receipt the archive could not take is still in the engine's table and still in the
/// response, so failing the request would refuse a commit that happened. The idempotency record is
/// the opposite — losing it changes what a **retry** does.
fn archive_last_verdict_receipt(state: &AppState, id: &TransformationId, slot: ReceiptSlot) {
    let receipt = {
        let engine = state.engine();
        engine.verdict_receipts(id).last().cloned()
    };
    if let Some(receipt) = receipt {
        let _ = state.archive().store(id, slot, &receipt);
    }
}

/// The same for T-11's commit receipt.
fn archive_commit_receipt(state: &AppState, id: &TransformationId) {
    let receipt = {
        let engine = state.engine();
        engine.receipt(id).cloned()
    };
    if let Some(receipt) = receipt {
        let _ = state.archive().store(id, ReceiptSlot::Commit, &receipt);
    }
}
