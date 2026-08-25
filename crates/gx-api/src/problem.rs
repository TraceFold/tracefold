// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
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
    /// 44 §2.3's `title`: "a short, human-readable summary" (sem: SEM-gx-api-188).
    ///
    /// 🔴 45 §4's scan reaches this field. req/88 M6-30 names it: "`title` is where 44 §2.3 defines
    /// 'human-readable summary' — **the only place on the wire where 45 §4's forbidden words could be written**" (sem: SEM-gx-api-189). So the titles below are
    /// short, factual, and claim nothing about what the system guarantees.
    pub title: &'static str,
    /// 44 §2.3's `detail`: "a detailed explanation" (sem: SEM-gx-api-190).
    pub detail: String,
    /// 🔴 The one status that does **not** come from [`gx_code::status_of`] — see
    /// [`ApiError::unavailable`]. `None` everywhere else, which is what keeps 44 §2.3's
    /// code-to-status pairing a table rather than a suggestion.
    status: Option<u16>,
    /// 🔴 **R28 / `req/334` M-01** — the two roll-back facts, on a refusal that answers about an
    /// abort. `None` on every refusal that does not, which is what keeps the five-key census exact
    /// everywhere else. See [`RollbackFacts`].
    rollback: Option<RollbackFacts>,
}

/// 🔴 **R28 / `req/334` M-01** — what became of 43 T-10c's roll-back, as **members**.
///
/// # The defect this closes
///
/// The twenty-seventh audit drove `POST /transformations/{id}/undo` against an adapter that
/// refuses the inverse's apply and read the delivered body: keyed on the abort taxonomy, and
/// containing the word `rollback` **zero** times. The value was in Σ the whole time
/// (`A27_UNDO_SIGMA aborted_rows=["ApplyFailed rollback=Some(Succeeded)"]`), and when the roll-back
/// itself was made to fail as well the answer was *equally* silent — one `gx_code`, one status and
/// one title for two terminal states an operator has to tell apart: **is my object back where it
/// was, or is it half undone?** That is the question this product is named for.
///
/// # Why members and not prose
///
/// gx-cli's twin road (`gx-cli/src/lifecycle.rs::halted`) already writes `"detail":
/// engine.rollback(undoing)` and `"not_attempted_because": …` as members, and `req/38` §234 ruling
/// 1 fixed the reason: a member carrying `null` is a fact a reader can branch on, where prose — or
/// absence — is not. This surface's `detail` is RFC 9457's own field and is prose by definition, so
/// the twin's member names cannot be reused verbatim; the facts get names of their own.
///
/// # Why this is allowed to widen 44 §2.3's member set
///
/// RFC 9457 §3.2 provides for extension members and this crate has taken that road once before —
/// `retry_after_ms`, on `BUSY` alone (DR-43-5 (3), `req/38` §156 ruling 2(c)). The discipline that
/// came with it is kept exactly: the members appear **only** where the refusal is keyed on the
/// abort taxonomy, so their presence is itself the signal, and `crates/gx-api/tests/wire_census.rs`
/// pins both sides of the asymmetry.
#[derive(Debug, Clone)]
pub struct RollbackFacts {
    /// [`gx_engine::Rollback::kind`], or `null` where no roll-back was in question.
    ///
    /// `null` is honest rather than missing: an abort that never entered T-10c's guard — a CAS that
    /// failed before any apply — has no roll-back to report, and a reader can branch on that.
    pub rollback: Option<&'static str>,
    /// [`gx_engine::NotAttemptedBecause::kind`], or `null`.
    ///
    /// `null` on every row this process did not abort itself: the cause is deliberately **not** a
    /// component of Σ (`gx-engine/src/store.rs` says so in as many words), so a row read back off
    /// the journal after a restart has none to give. Filling it would be asking this repair to
    /// invent a cause, which is the failure R25 and R26 were each a repair of.
    pub not_attempted_because: Option<&'static str>,
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
            rollback: None,
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

    /// 44 §2.3's `INTERNAL` (500), used as the honest "unclassified" (sem: SEM-gx-api-191) rather than as a bucket.
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
            "44 §2.5: \"every endpoint (except `/healthz`) requires `Authorization: Bearer <token>`\" (sem: SEM-gx-api-192)",
        )
    }

    /// 🔴 **DR-43-6 (`req/38` §156 ruling 2(a))**: `500 LEDGER_DISAGREES` — the journal and the
    /// ledger of this project describe different trees. See [`gx_code::LEDGER_DISAGREES`].
    ///
    /// R1b answered this condition with [`ApiError::internal`] and said in `detail` that the fold
    /// was a fold: "44 §2.3 has no code for 'this project's two files describe different trees';
    /// minting one is a surface addition and therefore a DR". This is that DR, landed, and the
    /// difference it buys is not cosmetic — `INTERNAL` is 44's word for *unclassified*, and a
    /// client that retried on it would be retrying against a condition that never resolves on its
    /// own, while an operator grepping their logs for real internal faults would find this
    /// alongside them for ever.
    #[must_use]
    pub fn ledger_disagrees(detail: impl Into<String>) -> Self {
        Self::new(
            gx_code::LEDGER_DISAGREES,
            gx_code::LEDGER_DISAGREES_TITLE,
            detail,
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
    /// ("**an operational state**, not a property of the request. 44 §2.3 has no operational code (no 503 either)"; sem: SEM-gx-api-193) and
    /// folded to `INTERNAL`. This does the same with the code and **keeps the status honest**: a
    /// client's retry policy reads the status, and a shutdown answered `500` would tell an operator
    /// that a rolling restart is a server fault.
    ///
    /// So this is the one place where 44 §2.3's pairing (`INTERNAL` → 500) does not hold on the wire,
    /// and the reason is that 44 has no row for the fact. Raised as **M6H6-4**, with the candidate
    /// row (`UNAVAILABLE`, 503, CLI 1) for §2.6's "backward-compatible addition" (sem: SEM-gx-api-194). `crates/gx-api/tests/shutdown.rs`
    /// asserts both halves — the status **and** the fold — so neither can drift unnoticed.
    #[must_use]
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            // 🔴 **E-M6-22** (§53, M6H6-4, adopted (a); sem: SEM-gx-api-195). Hand 6 wrote `"INTERNAL"` here and raised the
            // fold: 44 §2.3 has twelve words about requests and none about servers. See
            // [`gx_code::UNAVAILABLE`] for what the fold cost a client's retry policy.
            code: gx_code::UNAVAILABLE,
            title: "this server is not accepting new requests",
            detail: detail.into(),
            status: Some(gx_code::UNAVAILABLE_STATUS),
            rollback: None,
        }
    }

    /// 🔴 **M-14** (`req/189`): `413 PAYLOAD_TOO_LARGE` — the body was bigger than
    /// [`crate::MAX_BODY_BYTES`] and was not read. See [`gx_code::PAYLOAD_TOO_LARGE`].
    #[must_use]
    pub fn payload_too_large(detail: impl Into<String>) -> Self {
        Self::new(
            gx_code::PAYLOAD_TOO_LARGE,
            "the request body is larger than this server reads",
            detail,
        )
    }

    /// 🔴 **L-04 / H-10** (`req/189`): `415 UNSUPPORTED_MEDIA_TYPE` — a body arrived that is not
    /// declared `application/json`. See [`gx_code::UNSUPPORTED_MEDIA_TYPE`].
    #[must_use]
    pub fn unsupported_media_type(detail: impl Into<String>) -> Self {
        Self::new(
            gx_code::UNSUPPORTED_MEDIA_TYPE,
            "the request body is not declared as JSON",
            detail,
        )
    }

    /// 🔴 The same refusal with an explicit status (`req/189` H-11 ①: `405` under
    /// `VALIDATION_ERROR`). The second place, after [`ApiError::unavailable`], where the wire status
    /// is not the table's — and like the first it is a status 44 §2.3 has no row for, kept honest
    /// on the wire (a client's retry policy reads it) rather than folded to the row's.
    #[must_use]
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
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
        // 🔴 **DR-43-5 (2) / `req/215` M-10** — the one refusal whose wire form is rewritten here
        // rather than carried through from the crate that raised it.
        //
        // `gx_engine::Error::Busy`'s `Display` is `"another gx process is writing to {path}
        // ({holder})"`, which is right on a terminal — the operator owns the machine and the pid is
        // the diagnosis — and wrong over a socket. `req/215` probe (i) read it off the wire:
        // `"another gx process is writing to /tmp/gx_audit215/i/.gx/LOCK (698528 gx serve)"`, an
        // absolute server path and a live pid handed to any client holding a token. The path is the
        // server's own layout and the pid belongs in the server's log.
        //
        // The title is unified with the CLI's in the same breath (M-10's second half), so that a
        // proxy speaking both does not have two names for one refusal.
        if origin == gx_code::Origin::Engine && kind == "Busy" {
            return Self {
                code: row.code,
                title: gx_code::BUSY_TITLE,
                detail: format!(
                    "another gx process holds this project's writer lock, so the operation was \
                     refused rather than queued (DR-43-2). Send the same request again: the lock is \
                     held for one engine operation, measured at about {} ms (req/215 M-04), and \
                     `Retry-After` says {} because RFC 9110 has no finer unit. Which process holds \
                     it, and where the lock file is, are in this server's log and not in this \
                     answer (req/215 M-10)",
                    gx_code::BUSY_RETRY_AFTER_MILLIS,
                    gx_code::BUSY_RETRY_AFTER_SECONDS,
                ),
                status: None,
                rollback: None,
            };
        }
        Self {
            code: row.code,
            title: "the operation was refused",
            detail,
            status: None,
            rollback: None,
        }
    }

    /// 🔴 **R28 / `req/334` M-01** — attach the two roll-back facts to a refusal about an abort.
    ///
    /// Deliberately a builder rather than a parameter on [`ApiError::new`]: the members belong to
    /// the roads keyed on the abort taxonomy and to no others, and a parameter every call site had
    /// to pass `None` to would make their absence a default rather than a statement.
    #[must_use]
    pub fn with_rollback_facts(mut self, facts: RollbackFacts) -> Self {
        self.rollback = Some(facts);
        self
    }

    /// Whether this refusal carries the roll-back members — the presence that is itself the signal.
    #[must_use]
    pub const fn carries_rollback_facts(&self) -> bool {
        self.rollback.is_some()
    }

    /// The HTTP status 44 §2.3 (or §2.2, for `INVALID_STATE`) pairs with this code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status.unwrap_or_else(|| gx_code::status_of(self.code)))
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// The object 44 §2.3 fixes.
    ///
    /// 🔴 **DR-43-5 (3) (`req/38` §156 ruling 2(c))** — five members always, and a **sixth on
    /// `BUSY` alone**: `retry_after_ms`.
    ///
    /// R1b measured that the `Retry-After` header cannot say what it needs to say. Sampling
    /// `.gx/LOCK` every 2 ms across one CLI commit found the lock held about **50 ms per verb**,
    /// and RFC 9110's `delay-seconds` has no finer unit — so the header says `1`, a client obeying
    /// it literally takes twenty times longer than it needs to, and R1b put the real number in
    /// `detail` where only a human can read it. `detail` is prose; a retry schedule is a number a
    /// client computes with. Putting it in prose is asking every client to parse English.
    ///
    /// RFC 9457 §3.2 provides for exactly this — a problem type may define extension members — and
    /// the reason it was not simply added then is the reason it is added carefully now: 44 §2.3
    /// fixes the member set and `crates/gx-api/tests/wire_census.rs` pins it, so the shape moves
    /// only with a ruling. **The member appears on `BUSY` and on nothing else**, which keeps the
    /// five-key census exact for every other refusal and makes the presence of the key itself the
    /// signal that a retry is the correct response.
    #[must_use]
    pub fn body(&self) -> serde_json::Value {
        let mut body = serde_json::json!({
            "type": gx_code::type_uri(self.code),
            "title": self.title,
            "status": self.status().as_u16(),
            "detail": self.detail,
            "gx_code": self.code,
        });
        if self.code == gx_code::BUSY {
            if let Some(map) = body.as_object_mut() {
                map.insert(
                    "retry_after_ms".into(),
                    gx_code::BUSY_RETRY_AFTER_MILLIS.into(),
                );
            }
        }
        // 🔴 **R28 / `req/334` M-01** — the second extension pair, on the abort roads alone.
        //
        // Both members are written together or not at all, and each carries `null` where `null` is
        // the honest answer. Writing one and omitting the other would recreate, one member along,
        // exactly the ambiguity `req/38` §234 ruling 1 closed: an absent member is
        // indistinguishable from a road that never had the fact.
        if let Some(facts) = &self.rollback {
            if let Some(map) = body.as_object_mut() {
                map.insert(
                    "rollback".into(),
                    facts
                        .rollback
                        .map_or(serde_json::Value::Null, serde_json::Value::from),
                );
                map.insert(
                    "rollback_not_attempted_because".into(),
                    facts
                        .not_attempted_because
                        .map_or(serde_json::Value::Null, serde_json::Value::from),
                );
            }
        }
        body
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = self.body();
        // 44 §2.3: "every error response is `Content-Type: application/problem+json`" (sem: SEM-gx-api-196). Set explicitly rather
        // than through `Json`, whose content type is `application/json` — the two differ by exactly
        // the thing RFC 9457 defines, and a client content-negotiating on it would never see one.
        // 🔴 **DR-43-2 / DR-43-5** — `BUSY` carries `Retry-After`, and no other code does.
        //
        // A `503` without it says "not now" and leaves the caller to invent a schedule; the whole
        // reason `req/38` §148 gave this refusal a word of its own is that the correct response is a
        // retry, and a machine-readable "in about a second" is what turns that from advice into an
        // instruction. The lock is held for one engine operation, so one second is not a guess about
        // load — it is an upper bound on what is being waited for.
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        if self.code == gx_code::BUSY {
            headers.insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
        }
        (
            status,
            headers,
            serde_json::to_string(&body).unwrap_or_default(),
        )
            .into_response()
    }
}
