//! 44 §2.3's RFC 9457 `application/problem+json`, as the one refusal type this crate returns.
//!
//! Five fields, all of them 44 §2.3's: `type`, `title`, `status`, `detail`, `gx_code`. The `gx_code`
//! never comes from a literal at a call site — [`crate::gx_code`] is the one map (M6-09) and this
//! type is what carries its answer onto the wire.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::gx_code;

/// One refusal, in 44 §2.3's shape.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// 44 §2.3's `gx_code`, from [`crate::gx_code`] and never from a literal here.
    pub code: &'static str,
    /// 44 §2.3's `title`: 「短い人間可読要約」.
    ///
    /// 🔴 45 §4's scan reaches this field. req/88 M6-30 names it: 「`title` は 44 §2.3 が『人間可読
    /// 要約』と定義した場所=**45 §4 の禁止文を書ける唯一の wire 上の場所**」. So the titles below are
    /// short, factual, and claim nothing about what the system guarantees.
    pub title: &'static str,
    /// 44 §2.3's `detail`: 「詳細説明」.
    pub detail: String,
    /// 🔴 The one status that does **not** come from [`gx_code::status_of`] — see
    /// [`ApiError::unavailable`]. `None` everywhere else, which is what keeps 44 §2.3's
    /// code-to-status pairing a table rather than a suggestion.
    status: Option<u16>,
}

impl ApiError {
    /// A refusal with a code the map produced.
    #[must_use]
    pub fn new(code: &'static str, title: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            title,
            detail: detail.into(),
            status: None,
        }
    }

    /// 44 §2.3's `VALIDATION_ERROR` (422).
    #[must_use]
    pub fn validation(detail: impl Into<String>) -> Self {
        Self::new(
            "VALIDATION_ERROR",
            "the request is not one this server can attempt",
            detail,
        )
    }

    /// 44 §2.3's `NOT_FOUND` (404).
    #[must_use]
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", "the named object is not here", detail)
    }

    /// 44 §2.3's `INTERNAL` (500), used as the honest 「unclassified」 rather than as a bucket.
    #[must_use]
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new("INTERNAL", "the operation could not be completed", detail)
    }

    /// 44 §2.3's `UNAUTHORIZED` (401).
    ///
    /// The detail says what is required and not what was wrong with what arrived — see
    /// [`crate::auth::guard`] for why the three failure modes share one answer.
    #[must_use]
    pub fn unauthorized() -> Self {
        Self::new(
            "UNAUTHORIZED",
            "this endpoint requires a bearer token",
            "44 §2.5: 「全エンドポイント（`/healthz`除く）に`Authorization: Bearer <token>`必須」",
        )
    }

    /// 44 §2.2's `INVALID_STATE` (409) — see [`gx_code::INVALID_STATE`] for why it is not in §2.3.
    #[must_use]
    pub fn invalid_state(detail: impl Into<String>) -> Self {
        Self::new(
            gx_code::INVALID_STATE,
            "43 §3 offers no such transition from this state",
            detail,
        )
    }

    /// 🔴 `503` — the server is shutting down (44 §1.2's `gx serve`, graceful shutdown stage 1).
    ///
    /// **A fold, and here is what it loses.** 44 §2.3's twelve codes are all about the *request*;
    /// none of them is about the *server*. Hand 5 met the same wall folding `KeyPermissions`
    /// (「**運用の状態**であって request の性質でない。44 §2.3 に運用 code が無い(503 も無い)」) and
    /// folded to `INTERNAL`. This does the same with the code and **keeps the status honest**: a
    /// client's retry policy reads the status, and a shutdown answered `500` would tell an operator
    /// that a rolling restart is a server fault.
    ///
    /// So this is the one place where 44 §2.3's pairing (`INTERNAL` → 500) does not hold on the wire,
    /// and the reason is that 44 has no row for the fact. Raised as **M6H6-4**, with the candidate
    /// row (`UNAVAILABLE`, 503, CLI 1) for §2.6's 「後方互換な追加」. `crates/gx-api/tests/shutdown.rs`
    /// asserts both halves — the status **and** the fold — so neither can drift unnoticed.
    #[must_use]
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            // 🔴 **E-M6-22** (§53, M6H6-4 採(a)). Hand 6 wrote `"INTERNAL"` here and raised the
            // fold: 44 §2.3 has twelve words about requests and none about servers. See
            // [`gx_code::UNAVAILABLE`] for what the fold cost a client's retry policy.
            code: gx_code::UNAVAILABLE,
            title: "this server is not accepting new requests",
            detail: detail.into(),
            status: Some(gx_code::UNAVAILABLE_STATUS),
        }
    }

    /// 🔴 The map, applied: any refusal from one of the four crates, as a problem object.
    ///
    /// One function per origin rather than one generic, because the `kind()` call is what ties a
    /// value to its crate's declared vocabulary and a generic over `impl Kinded` would be a fifth
    /// name for a thing 44 §2.3 has twelve of.
    #[must_use]
    pub fn from_engine(e: &gx_engine::Error) -> Self {
        Self::mapped(gx_code::Origin::Engine, e.kind(), e.to_string())
    }

    /// As [`ApiError::from_engine`], for gx-gate.
    #[must_use]
    pub fn from_gate(e: &gx_gate::Error) -> Self {
        Self::mapped(gx_code::Origin::Gate, e.kind(), e.to_string())
    }

    /// As [`ApiError::from_engine`], for gx-witness.
    #[must_use]
    pub fn from_witness(e: &gx_witness::Error) -> Self {
        Self::mapped(gx_code::Origin::Witness, e.kind(), e.to_string())
    }

    /// As [`ApiError::from_engine`], for gx-log.
    #[must_use]
    pub fn from_log(e: &gx_log::Error) -> Self {
        Self::mapped(gx_code::Origin::Log, e.kind(), e.to_string())
    }

    fn mapped(origin: gx_code::Origin, kind: &str, detail: String) -> Self {
        let Some(row) = gx_code::of_kind(origin, kind) else {
            // Unreachable while `gx_code::DENOMINATOR` compiles, and answered rather than panicked
            // because 41 §6 counts a panic as a bug and a surface that fell over on an unmapped
            // refusal would turn a missing table row into an outage.
            return Self::internal(format!(
                "{origin:?}::{kind} has no row in the gx_code map (M6-09): {detail}"
            ));
        };
        Self {
            code: row.code,
            title: "the operation was refused",
            detail,
            status: None,
        }
    }

    /// The HTTP status 44 §2.3 (or §2.2, for `INVALID_STATE`) pairs with this code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status.unwrap_or_else(|| gx_code::status_of(self.code)))
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// The object 44 §2.3 fixes.
    #[must_use]
    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "type": gx_code::type_uri(self.code),
            "title": self.title,
            "status": self.status().as_u16(),
            "detail": self.detail,
            "gx_code": self.code,
        })
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = self.body();
        // 44 §2.3: 「全エラー応答は`Content-Type: application/problem+json`」. Set explicitly rather
        // than through `Json`, whose content type is `application/json` — the two differ by exactly
        // the thing RFC 9457 defines, and a client content-negotiating on it would never see one.
        (
            status,
            [(axum::http::header::CONTENT_TYPE, "application/problem+json")],
            serde_json::to_string(&body).unwrap_or_default(),
        )
            .into_response()
    }
}
