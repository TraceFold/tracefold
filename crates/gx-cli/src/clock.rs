//! 🔴 則 2 (req/88 §3.1) — the one place this binary reads a clock.
//!
//! > 41 §6「乱数・時刻はengine境界で注入」…∴ **実 clock と実 rng を呼ぶ箇所が CLI 全体で各 1 箇所**
//! > である事を機械検査する
//!
//! Every engine entry point takes `at: Timestamp` and `submit` takes `rng_seed: u64`, so the layer
//! that reads the real clock is this one. M6-28 puts the full instrument in hand 3's DoD, alongside
//! the rng; this hand introduces the **first** real clock read (`gx log checkpoint` stamps 42
//! §3.11's `timestamp`) and puts it behind one function so that hand 3 has one call site to count
//! rather than a habit to police.
//!
//! # 🔴 There is no `--at`
//!
//! M6-28's own warning: 「`--at` の隠し flag を作らない——時計を CLI 引数にすると receipt の
//! `issued_at` が嘘をつける」. A test that needs a fixed clock calls the library function that takes
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
