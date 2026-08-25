// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6H2-4, adopted (a)** (sem: SEM-gx-api-197) — 44 §0's date format, converted where 44 says it is converted.
//!
//! > date-times are RFC 3339 (e.g. `2026-08-06T10:00:00Z`). The conversion from the internal `Timestamp`
//! > (a nanosecond epoch) is **the API layer's responsibility** (sem: SEM-gx-api-198).
//!
//! req/38 §49 M6H2-4, adopted (a): "the RFC 3339 conversion is decided by hand 5's five-point measurement of `time`/`chrono`. Until then
//! `issued_at_unix_nanos` states its unit explicitly" (sem: SEM-gx-api-199). The measurement is in the manifest and in req/93 §4; the
//! short form is that `chrono` costs **zero** packages because cedar-policy-core already carries it,
//! and `time` would have cost twelve.
//!
//! # 🔴 What is converted, and what is deliberately not
//!
//! **This crate's wire output**, which is what 44 §0 says ("the API layer's responsibility"; sem: SEM-gx-api-200). The CLI still prints
//! `*_unix_nanos` field names, and that is a divergence from 44 §0 rather than an oversight: §0's
//! sentence is in the common convention (sem: SEM-gx-api-201) and covers both surfaces. Changing gx-cli's output shape is a change to a
//! contract three hands' suites already assert, and this hand does not own those verbs. Raised as
//! **M6H5-5**, with the reading that the field **name** carrying the unit (`issued_at_unix_nanos`)
//! is the honest interim: a wrong RFC 3339 string is worse than a right integer, and an integer
//! labelled with its unit is not a wrong RFC 3339 string.
//!
//! # 🔴 chrono without a clock
//!
//! `default-features = false` removes the `clock` feature and with it `Utc::now()`. 41 §6 puts time
//! at the engine boundary and M6-28's instrument counts the CLI's wall-clock reads; a second clock
//! arriving inside a date formatter would be invisible to that instrument because it is not spelled
//! `SystemTime::now`. Removing the feature makes the second clock **impossible** rather than
//! **discouraged**, which is the difference between Rule 2 (sem: SEM-gx-api-202) and a comment.

use chrono::{DateTime, SecondsFormat, Utc};
use gx_core::Timestamp;

/// 42 §1.2's `Timestamp` as 44 §0's RFC 3339, in UTC, to nanosecond precision.
///
/// # 🔴 Nanoseconds, and why the precision is not rounded away
///
/// 44 §0's example is second-precision (`2026-08-06T10:00:00Z`) and the internal value is
/// nanoseconds. Rounding to seconds at the wire would make two receipts issued inside one second
/// carry the same `issued_at`, and 43 T-6's TTLs, `Sigma`'s ordering and the journal's `at` are all
/// nanosecond facts. RFC 3339 permits arbitrary fractional digits; `SecondsFormat::Nanos` uses nine
/// of them, so the conversion is **lossless** and a client that wants seconds can truncate.
///
/// The `Z` spelling rather than `+00:00` is 44 §0's own example.
///
/// # 🔴 A timestamp outside chrono's range
///
/// `DateTime::from_timestamp_nanos` takes an `i64` and cannot fail — the whole `i64` nanosecond
/// range lies inside chrono's representable span (±292 years around 1970) — so there is no fallible
/// path and no `Option` to fold. `Timestamp` is an `i64` (42 §1.2) and gx-core already refuses a
/// negative `created_at` at construction (`CreatedAtNegative`), so the values reaching here are the
/// ones the type system already narrowed.
#[must_use]
pub fn of(at: Timestamp) -> String {
    let utc: DateTime<Utc> = DateTime::from_timestamp_nanos(at.0);
    utc.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

/// 🔴 The other direction — 44 §2.2's `?since=<…|RFC3339>`, for [`crate::stream`].
///
/// The **only** input on this surface that arrives as a date: everything else 44 dates is something
/// this crate wrote. `chrono`'s parser is the same one `of` formats with, so a client that echoes a
/// value it was given is parsing exactly what it was given.
///
/// # Errors
/// [`ApiError`](crate::ApiError) `VALIDATION_ERROR` for text RFC 3339 does not accept, and for a
/// date outside the `i64` nanosecond range — that second refusal is not pedantry: folding it to
/// `i64::MAX` would resolve "resume from the year 3000" to "resume from the end" (sem: SEM-gx-api-203), which is a
/// silently different subscription.
pub fn parse(text: &str) -> Result<Timestamp, crate::ApiError> {
    let parsed = DateTime::parse_from_rfc3339(text).map_err(|e| {
        crate::ApiError::validation(format!(
            "`{text}` is neither a ledger index nor an RFC 3339 instant (44 §2.2's \
             `?since=<ledger_index|RFC3339>`): {e}"
        ))
    })?;
    parsed.timestamp_nanos_opt().map(Timestamp).ok_or_else(|| {
        crate::ApiError::validation(format!(
            "`{text}` is a valid RFC 3339 instant outside the nanosecond range 42 §1.2's \
                 `Timestamp` can carry"
        ))
    })
}

/// The same, for an optional timestamp: `null` stays `null`.
///
/// A missing time and the epoch are different facts (M5-18, adopted (a), made exactly this point about
/// `applied_at`: "`Timestamp(0)` reaching this field is the bug the ruling names"; sem: SEM-gx-api-204), so an absent
/// value must not become `1970-01-01T00:00:00Z` on the way out.
#[must_use]
pub fn maybe(at: Option<Timestamp>) -> serde_json::Value {
    at.map_or(serde_json::Value::Null, |t| of(t).into())
}
