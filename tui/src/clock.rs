// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 Rule 2, one package over — the one place this face reads a clock.
//!
//! `gx-cli` has `src/clock.rs` for the same reason and with the same shape, and the two are not a
//! number written twice: they are two *processes* worth of the same rule, each holding its own
//! read behind one function so that a counter has one call site to count rather than a habit to
//! police. `crates/gx-cli/src/clock.rs`'s read stamps things the engine will keep (42 §3.11's
//! `timestamp`, on a checkpoint); this one stamps `read_at` on a reading — **a fact about this
//! process, never sent anywhere**, drawn on a screen and dropped when the frame is.
//!
//! # Why not `gx_core::Timestamp`
//!
//! That is what the face used before the extraction: `crate::clock::now()` returned a
//! `gx_core::Timestamp` and `wire.rs` immediately took `.0` off it. `Timestamp` is a newtype over
//! `i64`, so the whole of what crossed that boundary was a nanosecond count — and a dependency on
//! the engine's id crate, taken to read one field, is exactly the "except for one date formatter"
//! shape `r942_tui.rs`'s g11 was written against, one layer out where a source scan cannot see it.
//! `req/942` §13-2's rfc3339 already lives in `tui::wire`; this is its other half.
//!
//! # There is no `--at`
//!
//! Inherited verbatim from `gx-cli`'s clock, and it costs nothing to keep: a test that needs a
//! fixed instant calls [`tui::wire::rfc3339`](crate::tui::wire::rfc3339), which takes the number.
//! `now_nanos` is called by one line of `wire.rs` and by nothing else.

/// The wall clock, as nanoseconds since the Unix epoch.
///
/// # Before 1970
///
/// `duration_since(UNIX_EPOCH)` fails for a clock set before the epoch and this answers `0` there
/// rather than panicking — 41 §6 counts a panic as a bug, and a face that refused to draw because
/// the machine's clock is wrong would be reporting the wrong fault. A `read_at` of
/// `1970-01-01T00:00:00Z` on a screen is visibly wrong, which is the outcome that leads a reader
/// to the clock.
#[must_use]
pub fn now_nanos() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
}
