// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/824` A5** — observation ingest: `POST /v1/attach-sources/{id}/observations`.
//!
//! # 🔴 R-1: the response is an ordinary candidate, and that is the whole design
//!
//! This handler parses the wire JSON, asks the registry who is presenting, and hands everything
//! semantic to two engine accessors ([`gx_engine::Engine::observation_class`] and
//! [`gx_engine::Engine::ingest_observation`] — where the A2 admission, the chain rule, the
//! `observation_id` replay and the intent construction live). The candidate that comes back is
//! then verified through [`gx_engine::Engine::verify`] exactly as `POST /candidates/{id}/verify`
//! would, so `/candidates/{id}/commit`, `/cancel`, `/escalation`, `/stream`, `/receipts/{tid}`
//! and `GET /escalations` all work on it unchanged. No `Verdict` is constructed here, no
//! `Lifecycle` is written, no canonical byte is encoded (Rule 1, SEM-gx-api-174; the scanner in
//! `crates/gx-canon/tests/authority_boundary.rs` reads this directory).
//!
//! # 🔴 The three refusals, and whose they are
//!
//! * `404 SOURCE_UNKNOWN` — the registry's (A4): an unregistered source cannot present evidence.
//! * `422 OBSERVATION_CLASS_UNKNOWN` / `422 PLAINTEXT_SECRET_REFUSED` — the **engine's**
//!   (`gx_engine::Error::ObservationClassUnknown` / `::PlaintextSecret`), carried through
//!   [`ApiError::from_engine`]'s one map. This surface constructs neither.
//! * `422 VALIDATION_ERROR` — a body outside the wire schema. `deny_unknown_fields` on every
//!   payload shape is `req/824` §5-Q2's execution-field fence at the type level: a payload
//!   smuggling `command` / `callback_url` / any other field is refused because *nothing outside
//!   the schema is accepted at all* (fixture w824-observation-00016).
//!
//! # 🔴 The Escalate arm is a 2xx, deliberately
//!
//! A chain gap is admitted into the **third state**: `201` with `verdict: "Escalate"`, the
//! `CHAIN_GAP_ESCALATE` gx_code in the body, and an `escalation_ref` reachable at
//! `GET /escalations` — neither a silent accept (a fabricated chain) nor a Deny (discarded
//! evidence). `req/824` A3's row is the one addition whose status is a success code, and pairing
//! it with a 4xx would be the first step to losing the third state (§5-Q6).
//!
//! # 🔴 `verdict` spelling
//!
//! The wire word for an admitted observation is `"Allow"` (`req/wire/schema/observation.
//! schema.json`'s enum); the engine's verdict record says `Admit`. [`wire_verdict`] is that
//! projection — total over the three kinds, a spelling and not a judgment: the verdict the
//! engine holds is what every other surface still reports.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use gx_core::{
    EnvsetEntry, EnvsetFingerprint, EnvsetScope, ObservationClass, ObservationRecord, VerdictKind,
};
use serde::Deserialize;

use crate::extract::{Payload, Segment};
use crate::gx_code;
use crate::problem::ApiError;
use crate::state::AppState;
use crate::ReceiptSlot;

/// The one route this module adds (the `ATTACH_SOURCE_ENDPOINTS` shape one module over).
pub const OBSERVATION_ENDPOINTS: [&str; 1] = ["POST /attach-sources/{id}/observations"];

/// The body `POST /attach-sources/{id}/observations` reads
/// (`req/wire/schema/observation.schema.json`, `ingestRequest`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestBody {
    /// The class, as a string — decoded by the **engine** so a value outside the enum is the
    /// engine's own `OBSERVATION_CLASS_UNKNOWN`, never a defaulted variant.
    pub class: String,
    /// The source's own id for the operation it reports (`req/824` §2-1, all four classes).
    pub observation_id: String,
    /// The last committed record the source believes precedes this one. `None` claims "first".
    #[serde(default)]
    pub prev_ref: Option<String>,
    /// The class payload, parsed against the class's own shape below.
    pub payload: serde_json::Value,
}

type Answer = Result<Response, ApiError>;

/// The wire spelling of a verdict (module header: a projection, not a judgment).
#[must_use]
fn wire_verdict(kind: VerdictKind) -> &'static str {
    match kind {
        VerdictKind::Admit => "Allow",
        VerdictKind::Deny => "Deny",
        VerdictKind::Escalate => "Escalate",
    }
}

/// Parse the class payload into its typed record (gx-core's, so canonical encode stays
/// engine-side). Every shape here is `deny_unknown_fields` (module header).
fn typed_record(
    class: ObservationClass,
    prev_ref: Option<&str>,
    payload: &serde_json::Value,
) -> Result<ObservationRecord, ApiError> {
    let refusal = |e: serde_json::Error| {
        ApiError::validation(format!(
            "the {} payload is not of req/wire/schema/observation.schema.json's shape: {e}. \
             Fields outside the schema are refused entirely -- this server observes operations \
             other systems performed and never accepts one to perform (SS273, req/824 §5-Q2)",
            class.as_wire_str()
        ))
    };
    Ok(match class {
        ObservationClass::Envset => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct ScopeWire {
                project: String,
                environment: String,
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct EntryWire {
                name: String,
                value_digest: String,
                #[serde(default)]
                scope_tag: Option<String>,
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct EnvsetWire {
                scope: ScopeWire,
                entries: Vec<EntryWire>,
            }
            let wire: EnvsetWire = serde_json::from_value(payload.clone()).map_err(refusal)?;
            ObservationRecord::Envset(EnvsetFingerprint::new(
                EnvsetScope {
                    project: wire.scope.project,
                    environment: wire.scope.environment,
                },
                wire.entries
                    .into_iter()
                    .map(|e| EnvsetEntry::new(e.name, e.value_digest, e.scope_tag))
                    .collect(),
                prev_ref.map(str::to_string),
            ))
        }
        ObservationClass::Deploy => {
            ObservationRecord::Deploy(serde_json::from_value(payload.clone()).map_err(refusal)?)
        }
        ObservationClass::Config => {
            ObservationRecord::Config(serde_json::from_value(payload.clone()).map_err(refusal)?)
        }
        ObservationClass::LogWindow => {
            ObservationRecord::LogWindow(serde_json::from_value(payload.clone()).map_err(refusal)?)
        }
    })
}

/// 🔴 Ingest one observation (**`req/824` A5**; module header for the whole design).
///
/// # Errors
/// [`ApiError`] — `SOURCE_UNKNOWN` (404) for an unregistered source, `VALIDATION_ERROR` (422)
/// for a body outside the schema, and the engine's own refusals (`PLAINTEXT_SECRET_REFUSED`,
/// `OBSERVATION_CLASS_UNKNOWN`, …) through the one map.
pub async fn ingest(
    State(state): State<AppState>,
    Segment(source_id): Segment<String>,
    Payload(body): Payload<IngestBody>,
) -> Answer {
    {
        let registry = state.attach_sources();
        if !registry.holds(&source_id) {
            return Err(ApiError::new(
                gx_code::SOURCE_UNKNOWN,
                "no attach-source is registered under this id",
                format!(
                    "`{source_id}` names no registered attach-source, and an unregistered source \
                     cannot present observations; register it with `POST /v1/attach-sources` \
                     first (req/824 A4/A5)"
                ),
            ));
        }
    }
    if body.observation_id.is_empty() {
        return Err(ApiError::validation(
            "`observation_id` must be a non-empty string: it is the source's own id for the \
             operation, and without it a retry is indistinguishable from a second operation \
             (req/824 §2-1)",
        ));
    }

    let at = state.now();
    let seed = state.seed();
    let (outcome, class) = {
        let mut engine = state.engine_for_write()?;
        let class = engine
            .observation_class(&body.class)
            .map_err(|e| ApiError::from_engine(&e))?;
        let record = typed_record(class, body.prev_ref.as_deref(), &body.payload)?;
        let outcome = engine
            .ingest_observation(
                &source_id,
                &body.observation_id,
                body.prev_ref.as_deref(),
                record,
                seed,
                at,
            )
            .map_err(|e| ApiError::from_engine(&e))?;
        // 🔴 The candidate is verified in the same hold, so the response can answer with a
        // verdict rather than a "come back later" — the same synchronous ruling
        // `POST /candidates/{id}/verify` records (ASM-44-2; there is no second thread for a 202
        // to be answered by). A replayed ingest is NOT re-verified: one operation, one gate ask
        // (a second `VerifyStarted` for the same report would make the journal claim the gate
        // was asked twice).
        if !outcome.replayed {
            // No injected evidence on this road (the observation IS the evidence, and it rides
            // the delta payload); the cell is emptied on both sides so no other request's
            // evidence can be read here and nothing lingers after (`verify_candidate`'s rule).
            state.evidence().load(Vec::new());
            let result = engine.verify(&outcome.id, at, state.keys().signing(), None);
            state.evidence().clear();
            result.map_err(|e| ApiError::from_engine(&e))?;
        }
        (outcome, class)
    };
    // The verdict receipt ASM-14 issued, filed where a restart can find it (the
    // `verify_candidate` road's own line).
    crate::handlers::archive_last_verdict_receipt(&state, &outcome.id, ReceiptSlot::Verdict);

    let (verdict, state_name, escalation_ref) = {
        let engine = state.engine();
        let verdict = engine.verdict(&outcome.id);
        let state_name = engine.state(&outcome.id).map(|s| s.name());
        let escalation_ref = engine
            .ticket(&outcome.id)
            .map(|ticket| ticket.id.0.to_text());
        (verdict, state_name, escalation_ref)
    };

    let mut answer = serde_json::json!({
        // An ORDINARY candidate id — the whole design (R-1). Every candidate route answers
        // about it unchanged.
        "candidate_id": outcome.id.0.to_text(),
        "class": class.as_wire_str(),
        "verdict": verdict.map(wire_verdict),
        "state": state_name,
        // What the source must quote as `prev_ref` for its next record on this scope, once this
        // candidate commits.
        "chain_ref": outcome.chain_ref,
        "replayed": outcome.replayed,
        // P-12: the surface ships its limits before its handler ships an answer.
        "limits": [
            "An observation is evidence that a record was presented, never evidence that the \
             operation occurred; operations never reported are unobserved and are not counted \
             as absent-because-clean (req/824 A5)",
            "Ingest is client-push only: platform-initiated webhooks do not exist (req/805 \
             Phase X U2)",
        ],
    });
    if verdict == Some(VerdictKind::Escalate) {
        if let Some(map) = answer.as_object_mut() {
            // The third state's wire marks: the A3 row whose status is deliberately a 2xx, and
            // the road to the human (`GET /escalations` renders the same ticket).
            map.insert("gx_code".into(), gx_code::CHAIN_GAP_ESCALATE.into());
            map.insert(
                "escalation_ref".into(),
                escalation_ref
                    .clone()
                    .map_or(serde_json::Value::Null, Into::into),
            );
        }
    }
    Ok((StatusCode::CREATED, axum::Json(answer)).into_response())
}
