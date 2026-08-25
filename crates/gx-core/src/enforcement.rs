// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The two settings DR-2 keeps independent: whether a `Deny` still applies, and what happens when
//! nobody can be asked (**M5-08, adopted (a)**, `req/38_ERRATA_2026-08-07.md` §37; sem: SEM-gx-core-012).
//!
//! Spec: 43 §4 for the semantics of both axes and for the sentence that says they are independent,
//! 43 §3 T-4d/T-4e/T-8r for the three transitions they guard, 42 §3.10 for the `enforced` field a
//! receipt carries, ASM-12 / ASM-13 (35) for the meanings themselves.
//!
//! # Why here and not in gx-engine
//!
//! 43 §10 left the placement open -- "the placement and addition of the setting types for these two
//! axes (`FailPosture`, `EnforcementMode`) are not written in 41 §3/§4, so ... a file-scoped ASM is
//! filed in §10" (quoted in SEM-gx-core-013) -- and it stayed open for three milestones. The ruling
//! that closed it is the same one that closed [`crate::VerdictKind`]:
//!
//! > **M5-08, adopted (a)**: `EnforcementMode`/`FailPosture` go to **gx-core** (the precedent of
//! > E-M2-1/E-M3-2: a type both witness and engine name belongs to the lower layer)
//! > (quoted in SEM-gx-core-014)
//!
//! The engine *writes* the setting, gx-witness *reads* it (a receipt records `enforced`), and
//! gx-gate never sees it at all -- 43 §4's record-only mode "adds no state" (sem: SEM-gx-core-015),
//! so no verdict
//! computation changes. Two crates naming one type is the exact condition E-M2-1 fixed for
//! `InclusionProof` and E-M3-2 for `VerdictKind`: **the data comes down, the computation stays up**.
//! Filed in gx-engine as 42 §0 might suggest, gx-witness would have to name gx-engine to type a
//! field of its own payload, which is the cycle 45 §1's trust boundary forbids.
//!
//! # Why two enums and not two `bool`s
//!
//! 43 §4's last paragraph is the whole of this module's design:
//!
//! > `FailPosture` (the posture when the verifier is unreachable) and `EnforcementMode` (whether to
//! > apply even on Deny) are independent configuration axes. (quoted in SEM-gx-core-016)
//!
//! Two `bool`s carry the same information and lose the sentence. `true` is not a claim about which
//! axis it belongs to, the pair `(true, false)` has no reading without a convention held somewhere
//! else, and the defaults -- which are *not* the same word on the two axes -- become two literals
//! rather than two named states. M5-08 (c) was "make do with two `bool`s" (sem: SEM-gx-core-017)
//! and was not taken for this
//! reason. What the enums buy mechanically is that `tests/enforcement_axes.rs` can enumerate the
//! four combinations and name each one, and that a hand which later adds a third posture adds it to
//! one list instead of to every call site that spelt it `true`.
//!
//! # The defaults are the fail-closed corner
//!
//! DR-2's default is `FailPosture::FailClosed` **for every substrate**, with `RecordOnly` available
//! as an opt-in. Both [`Default`] impls point at the strict end, so a value that arrives without a
//! setting is the safe one -- and a deployment that wants the other has to say so. `FailOpen` in
//! particular is only valid "when the substrate configuration opts in explicitly" (43 §4; sem:
//! SEM-gx-core-018), which is a fact
//! about configuration rather than about this type; what this type guarantees is that nobody
//! reaches it by forgetting.

use serde::{Deserialize, Serialize};

use core::fmt;

/// Whether a `Deny` stops the transformation (43 §4, DR-2).
///
/// This is the axis 43 T-8r turns on: in [`EnforcementMode::Enforce`] the `Denied` state is
/// terminal, and in [`EnforcementMode::RecordOnly`] a denied transformation still walks
/// `Canonicalized → Committing → Committed` with `enforced=false` stamped on its receipt. 43 §4:
/// "this leaves the fact that 'the application went through but policy had refused it' in a form a
/// third party can verify" (quoted in SEM-gx-core-019).
///
/// **What it is not**: it is not a lifecycle state. 43 §4's first line is "record-only mode
/// **adds no state**" (sem: SEM-gx-core-020) -- the mode is a parallel flag, and the eleven states
/// of 43 §1 are the
/// same eleven whichever mode is set. A hand that reached for a twelfth state here would be
/// re-deciding a question 43 already answered.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum EnforcementMode {
    /// `Denied` is terminal. The default (DR-2).
    #[default]
    Enforce,
    /// A `Deny` is recorded and the transformation proceeds, with `enforced=false` on the receipt.
    RecordOnly,
}

impl EnforcementMode {
    /// Both, in 43 §4's order.
    ///
    /// Declared once, like [`crate::VerdictKind::ALL`] and [`crate::TheoremId::ALL`], so that a
    /// test enumerating the modes reads the implementation instead of restating it.
    pub const ALL: [EnforcementMode; 2] = [EnforcementMode::Enforce, EnforcementMode::RecordOnly];

    /// 43 §4's spelling, for the places a string is what the format holds.
    ///
    /// The same text serde writes, and the two are not allowed to disagree:
    /// `gx-core/tests/enforcement_axes.rs` round-trips every variant through the canonical encoder
    /// and compares.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            EnforcementMode::Enforce => "Enforce",
            EnforcementMode::RecordOnly => "RecordOnly",
        }
    }

    /// Whether a receipt issued under this mode carries `enforced = true` (42 §3.10).
    ///
    /// One function rather than a comparison at each call site, because 43 §4 states the rule once
    /// -- "but the receipt must always be stamped `enforced=false`" (sem: SEM-gx-core-021) -- and a
    /// rule stated once should be
    /// implemented once. The engine's commit path (M5 hand 4) is the caller; `FailOpen` engaging is
    /// the *other* road to `enforced=false` (43 T-4e), and that one is [`FailPosture`]'s, which is
    /// why this function does not take both axes.
    #[must_use]
    pub const fn enforced(self) -> bool {
        match self {
            EnforcementMode::Enforce => true,
            EnforcementMode::RecordOnly => false,
        }
    }
}

impl fmt::Display for EnforcementMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What to do when the verifier or the evidence collector cannot be reached (43 §4, DR-2, ASM-13).
///
/// [`FailPosture::FailClosed`] is 43 T-4d: the transformation aborts with
/// [`crate::AbortReason::VerifierUnavailable`]. [`FailPosture::FailOpen`] is T-4e: **that one
/// transformation** is degraded to record-only and continues, and the receipt must carry both
/// `enforced=false` and `fail_posture_engaged=true` so that the degradation is visible to a third
/// party rather than invisible.
///
/// # The reachability this axis is about is the collector's, not the gate's
///
/// **E-M5-4** (`req/38_ERRATA_2026-08-07.md` §37, ruling M5-19 (a)) reads AC-036's "`kill -9` the
/// gx-gate process" as "the evidence collector is unreachable" (sem: SEM-gx-core-022), because
/// 41 §2 makes gx-gate a library: a
/// function call in the same process cannot become unreachable, and a condition that cannot be
/// constructed is a guard nothing tests. So the only producer of an unreachable verifier is the
/// evidence source, which M5 hand 2 introduces. This type is the posture; the producer is not here.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum FailPosture {
    /// Unreachable verifier aborts the transformation. The default, for every substrate (DR-2).
    #[default]
    FailClosed,
    /// Unreachable verifier degrades this transformation to record-only (ASM-13). Valid only where
    /// a substrate has opted in explicitly.
    FailOpen,
}

impl FailPosture {
    /// Both, in 43 §4's order.
    pub const ALL: [FailPosture; 2] = [FailPosture::FailClosed, FailPosture::FailOpen];

    /// 43 §4's spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            FailPosture::FailClosed => "FailClosed",
            FailPosture::FailOpen => "FailOpen",
        }
    }

    /// Whether reaching this posture is itself a fact the receipt has to record (43 T-4e).
    ///
    /// 43 T-4e: "always stamp `enforced=false` and `fail_posture_engaged=true` on the receipt"
    /// (sem: SEM-gx-core-023). The receipt
    /// field already exists -- **E-M2-7** put `fail_posture_engaged` on `ReceiptPayload` in M2, and
    /// the M5 batch's M5-12 raising it as missing was a misreading of 42 §3.10's stale table that
    /// §37's acceptance caught ("M5-12 was filed in error"; sem: SEM-gx-core-024). So this predicate
    /// has a place to be written to
    /// on the day M5 hand 6 wires T-4e, and nothing about the wire format moves.
    #[must_use]
    pub const fn engaged(self) -> bool {
        match self {
            FailPosture::FailClosed => false,
            FailPosture::FailOpen => true,
        }
    }
}

impl fmt::Display for FailPosture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the strict corner of both axes (DR-2).
    #[test]
    fn both_defaults_are_the_fail_closed_corner() {
        assert_eq!(EnforcementMode::default(), EnforcementMode::Enforce);
        assert_eq!(FailPosture::default(), FailPosture::FailClosed);
    }

    /// The two axes do not constrain each other: all four combinations exist (43 §4).
    #[test]
    fn the_two_axes_are_independent() {
        let mut seen = std::collections::BTreeSet::new();
        for mode in EnforcementMode::ALL {
            for posture in FailPosture::ALL {
                seen.insert((mode.as_str(), posture.as_str()));
            }
        }
        assert_eq!(seen.len(), 4, "2 × 2 settings, none excluded by the types");
    }
}
