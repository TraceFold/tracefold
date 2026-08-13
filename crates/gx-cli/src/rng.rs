//! 🔴 則 2 (req/88 §3.1) — the one place this binary draws entropy.
//!
//! > 41 §6「乱数・時刻はengine境界で注入」…∴ **実 clock と実 rng を呼ぶ箇所が CLI 全体で各 1 箇所**
//! > である事を機械検査する
//!
//! [`crate::clock`] is the other half. `Engine::submit(intent, rng_seed, at)` is the only entry
//! point of the eight that takes a seed, and 42 §3.13's `DraftCreated{intent_id, rng_seed, at}` is
//! what makes FR-039's replay deterministic: the number this module produces is **recorded**, and a
//! replay re-injects it rather than drawing a new one. So the seed is not a secret and is not a
//! nonce — it is the value that makes an execution reproducible, which is why it has to be drawn
//! once, at the outside edge, and written down.
//!
//! `probes/doubt/tests/m6_surface_doubt.rs`'s `the_clock_and_the_entropy_source_are_each_read_once`
//! counts the call sites, and `tools/verify_m6h3.sh` §4 adds a second one to watch it go red.
//!
//! # Why `RandomState` and not a crate
//!
//! 則 2 is a claim about *how many* places read the outside world, not about the quality of the
//! bits, and this is the only source in the standard library that is seeded by the operating
//! system. `rand` would be an external package added for one `u64` — req/89 §2's whole section is a
//! count of what the two new crates cost, and this hand keeps that number where hand 2 left it
//! (**external 277**).
//!
//! What the choice does cost is worth stating rather than hiding: `RandomState`'s keys are
//! documented as random per instance and are **not** a cryptographic guarantee. Nothing here needs
//! one. A `rng_seed` chosen by an adversary changes no identity — the `IntentId` is the canonical
//! digest of the intent (42 §3.3) and the seed is outside it (`DraftCreated` records it beside the
//! id) — so the seed's only job is to make two runs of the same intent distinguishable in a replay.
//! If a later hand gives the seed a security role, this comment is the line that has to be
//! revisited, and it is here rather than in a report so that the revisiting happens where the code
//! is.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// A seed for `Engine::submit`, drawn from the operating system.
///
/// One call, in `main`, at the moment a draft is created. Every test that wants a fixed seed calls
/// the library function that takes one — which is all of them — so this is never on a path a probe
/// has to work around.
#[must_use]
pub fn seed() -> u64 {
    RandomState::new().build_hasher().finish()
}
