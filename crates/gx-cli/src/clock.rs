// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 Rule 2 (req/88 §3.1) — the one place this binary reads a clock (sem: SEM-gx-cli-001).
//!
//! > 41 §6 "randomness and time are injected at the engine boundary" … therefore it is
//! > machine-checked that the real clock and the real rng are each called from exactly one place
//! > across the whole CLI (sem: SEM-gx-cli-002)
//!
//! Every engine entry point takes `at: Timestamp` and `submit` takes `rng_seed: u64`, so the layer
//! that reads the real clock is this one. M6-28 puts the full instrument in hand 3's DoD, alongside
//! the rng; this hand introduces the **first** real clock read (`gx log checkpoint` stamps 42
//! §3.11's `timestamp`) and puts it behind one function so that hand 3 has one call site to count
//! rather than a habit to police.
//!
//! # 🔴 There is no `--at`
//!
//! M6-28's own warning: "do not make `--at` a hidden flag -- turning the clock into a CLI
//! argument lets the receipt's `issued_at` lie" (sem: SEM-gx-cli-003). A test that needs a fixed clock calls the library function that takes
//! a `Timestamp`, which is every one of them; `now()` is called by `main` and by nothing else.

use gx_core::Timestamp;

/// The wall clock, as nanoseconds since the Unix epoch (42's `Timestamp`).
///
/// # Before 1970
///
/// `duration_since(UNIX_EPOCH)` fails for a clock set before the epoch and this answers `0` there
/// rather than panicking. 41 §6 counts a panic as a bug, and the alternative — refusing to run —
/// would make a misconfigured clock look like a broken ledger. A checkpoint stamped 0 is visibly
/// wrong, which is the outcome that leads a reader to the clock.
#[must_use]
pub fn now() -> Timestamp {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
    Timestamp(nanos)
}
