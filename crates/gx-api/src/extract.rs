//! 🔴 Extractors whose refusals are 44 §2.3's, because 44 §2.3 says **every** refusal is.
//!
//! > 全エラー応答は`Content-Type: application/problem+json`で以下の形を取る
//!
//! axum's own `Json` and `Query` rejections are plain text with no `gx_code`, and they are the
//! refusals a client meets **first** — a mistyped field name never reaches a handler. A surface
//! whose thirteen handlers all answered in RFC 9457 and whose extractors answered in prose would
//! satisfy the letter of §2.3 for every error a well-formed request can cause, which is the set of
//! errors that matter least.
//!
//! This is also 44 §2.3's `VALIDATION_ERROR` meaning what it says: 「リクエスト不正」 is exactly what
//! a body that will not deserialise is, and 422 is exactly the status the table pairs with it — so
//! the wrapper changes the **shape** of the answer and not its meaning.
//!
//! Found by `crates/gx-api/tests/router.rs`, which sent `{}` to every route and measured 「answered
//! with no body at all」 — the shape an unrouted path has.

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, OptionalFromRequest, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::problem::ApiError;

/// A JSON request body, refused in 44 §2.3's shape.
///
/// Used everywhere `Json<T>` would be. `Option<Payload<T>>` works too, for the two endpoints 44 §2.2
/// marks 「（任意）」 (`verify` and `replay`).
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
        // bodies 「（任意）」, which is a statement about absence and not about garbage: folding the
        // two would let `{"record_only": "yes"}` be silently read as 「no body was sent」 and
        // therefore as 「the default posture」 — a typo turning into an enforcement decision.
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

/// One rejection, in 44 §2.3's words.
///
/// The message is axum's, kept whole: it names the field and the position, which is the 「詳細説明」
/// §2.3 asks `detail` for, and rewriting it would be this crate having a second opinion about what a
/// deserialiser found.
fn refused_body(e: JsonRejection) -> ApiError {
    ApiError::validation(format!(
        "the request body is not one this endpoint can read (44 §2.2): {e}"
    ))
}
