// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 Extractors whose refusals are 44 §2.3's, because 44 §2.3 says **every** refusal is.
//!
//! > every error response takes the following shape, with `Content-Type: application/problem+json` (sem: SEM-gx-api-052)
//!
//! axum's own `Json` and `Query` rejections are plain text with no `gx_code`, and they are the
//! refusals a client meets **first** — a mistyped field name never reaches a handler. A surface
//! whose thirteen handlers all answered in RFC 9457 and whose extractors answered in prose would
//! satisfy the letter of §2.3 for every error a well-formed request can cause, which is the set of
//! errors that matter least.
//!
//! This is also 44 §2.3's `VALIDATION_ERROR` meaning what it says: "a malformed request" (sem: SEM-gx-api-053) is exactly what
//! a body that will not deserialise is, and 422 is exactly the status the table pairs with it — so
//! the wrapper changes the **shape** of the answer and not its meaning.
//!
//! Found by `crates/gx-api/tests/router.rs`, which sent `{}` to every route and measured "answered
//! with no body at all" (sem: SEM-gx-api-054) — the shape an unrouted path has.
//!
//! # 🔴 v0.4-l (`req/189`): the four writers 44 §2.3 did not reach, and one silent drop
//!
//! `req/182` H-11 counted four response writers outside this module's reach — an unrouted path
//! (404, empty), a wrong method (405, empty), a `Path<u64>` that did not parse (400 text) and a
//! `POST /verdict-checkpoints` body that used axum's own `Json` (422/415 text) — and H-10 found
//! that a body arriving with **no** `Content-Type` on an optional-body endpoint was read as "no
//! body" (`replay {from,to}` became "the whole journal"). This module now owns all of them:
//! [`Segment`] is the path extractor, [`Payload`]'s optional form reads the body before deciding
//! it is absent, and `crate::router` mounts the two fallbacks. `tests/wire_census.rs` measures
//! problem+json on seven roads instead of three.

use axum::extract::rejection::{
    BytesRejection, FailedToBufferBody, JsonRejection, PathRejection, QueryRejection,
};
use axum::extract::{FromRequest, FromRequestParts, OptionalFromRequest, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::problem::ApiError;

/// A JSON request body, refused in 44 §2.3's shape.
///
/// Used everywhere `Json<T>` would be. `Option<Payload<T>>` works too, for the two endpoints 44 §2.2
/// marks "(optional)" (sem: SEM-gx-api-055) (`verify` and `replay`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Payload<T>(pub T);

impl<S, T> FromRequest<S> for Payload<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = <axum::Json<T> as FromRequest<S>>::from_request(req, state)
            .await
            .map_err(refused_body)?;
        Ok(Payload(value))
    }
}

impl<S, T> OptionalFromRequest<S> for Payload<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        // 🔴 A **missing** body is `None` and a **malformed** one is a refusal. 44 §2.2 marks two
        // bodies "(optional)" (sem: SEM-gx-api-056), which is a statement about absence and not about garbage: folding the
        // two would let `{"record_only": "yes"}` be silently read as "no body was sent" and
        // therefore as "the default posture" — a typo turning into an enforcement decision.
        //
        // 🔴 **H-10** (`req/182` §1-1, `req/189`): axum's `OptionalFromRequest for Json` decides
        // "absent" from the **header** — no `Content-Type`, `Ok(None)`, body unread. So a body
        // that arrived with no header was the exact silent drop the paragraph above forbids, one
        // door over (measured: `replay -H 'Content-Type:' -d '{"from":0,"to":1}'` replayed the
        // whole journal). Absence is decided from the **body** here: no header **and** no bytes is
        // `None`; no header and bytes is a refusal (415, the same answer a wrong `Content-Type`
        // gets — one condition, one code, see `gx_code::UNSUPPORTED_MEDIA_TYPE`).
        if req
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .is_none()
        {
            let bytes = <axum::body::Bytes as FromRequest<S>>::from_request(req, state)
                .await
                .map_err(refused_bytes)?;
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(ApiError::unsupported_media_type(format!(
                "a request body of {} bytes arrived with no `Content-Type` header; this endpoint \
                 reads `application/json` bodies only, and a body it cannot read is refused rather \
                 than treated as absent (44 §2.2 optional is about absence, not about garbage; \
                 req/189 H-10)",
                bytes.len()
            )));
        }
        let optional = <axum::Json<T> as OptionalFromRequest<S>>::from_request(req, state)
            .await
            .map_err(refused_body)?;
        Ok(optional.map(|axum::Json(value)| Payload(value)))
    }
}

/// A query string, refused in 44 §2.3's shape.
#[derive(Debug, Clone, Copy, Default)]
pub struct Params<T>(pub T);

impl<S, T> FromRequestParts<S> for Params<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(value)| Params(value))
            .map_err(|e: QueryRejection| {
                ApiError::validation(format!(
                    "the query string is not one this endpoint can read (44 §2.2): {e}"
                ))
            })
    }
}

/// 🔴 A path segment (`{id}`, `{tid}`, `{window_end}`), refused in 44 §2.3's shape (**H-11 ②**).
///
/// axum's `Path` rejection is `400 text/plain`; `GET /verdict-checkpoints/abc` answered in prose
/// (`req/182` probe1 P3). Every handler takes its segment through this wrapper now, so a segment
/// that will not parse into its type is `422 VALIDATION_ERROR` like every other unreadable input.
/// (`Path<String>` cannot fail to parse but can fail percent-decoding — H-11 ④ — and that road is
/// the same rejection type, so it is covered without a second wrapper.)
#[derive(Debug, Clone, Copy, Default)]
pub struct Segment<T>(pub T);

impl<S, T> FromRequestParts<S> for Segment<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(value)| Segment(value))
            .map_err(|e: PathRejection| {
                ApiError::validation(format!(
                    "the path segment is not one this endpoint can read (44 §2.1): {e}"
                ))
            })
    }
}

/// One rejection, in 44 §2.3's words.
///
/// The message is axum's, kept whole: it names the field and the position, which is the "detailed explanation" (sem: SEM-gx-api-057)
/// §2.3 asks `detail` for, and rewriting it would be this crate having a second opinion about what a
/// deserialiser found.
///
/// 🔴 v0.4-l (`req/189`): three of axum's four rejection kinds keep the 422 they always had; the
/// other two are what the audit called "right shape, wrong status" (L-04) and are answered with
/// the status RFC 9110 gives them — 415 for a body not declared JSON, 413 for a body over
/// [`crate::MAX_BODY_BYTES`] (M-14). The `_` arm exists because `JsonRejection` is
/// `#[non_exhaustive]` upstream; it folds to 422 rather than 500, which is what a new axum
/// rejection kind (an unread body) would mean here.
fn refused_body(e: JsonRejection) -> ApiError {
    match e {
        JsonRejection::MissingJsonContentType(_) => ApiError::unsupported_media_type(format!(
            "this endpoint reads `application/json` bodies only (44 §2.2): {e}"
        )),
        JsonRejection::BytesRejection(inner) => refused_bytes(inner),
        JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
            ApiError::validation(format!(
                "the request body is not one this endpoint can read (44 §2.2): {e}"
            ))
        }
        _ => ApiError::validation(format!(
            "the request body is not one this endpoint can read (44 §2.2): {e}"
        )),
    }
}

/// The body could not be buffered — 413 when it was the length limit (M-14), 422 otherwise.
fn refused_bytes(e: BytesRejection) -> ApiError {
    match e {
        BytesRejection::FailedToBufferBody(FailedToBufferBody::LengthLimitError(_)) => {
            ApiError::payload_too_large(format!(
                "the request body is larger than the {} bytes this server reads (44 §2.2, \
                 `gx_api::MAX_BODY_BYTES`; req/189 M-14): {e}",
                crate::MAX_BODY_BYTES
            ))
        }
        _ => ApiError::validation(format!("the request body could not be read (44 §2.2): {e}")),
    }
}
