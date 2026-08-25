// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What an adapter can refuse to do, and why that vocabulary cannot be gx-core's. (sem:
//! SEM-gx-substrate-069, SEM-gx-substrate-070, SEM-gx-substrate-071, SEM-gx-substrate-072,
//! SEM-gx-substrate-073, SEM-gx-substrate-074, SEM-gx-substrate-075, SEM-gx-substrate-076,
//! SEM-gx-substrate-077, SEM-gx-substrate-078, SEM-gx-substrate-079, SEM-gx-substrate-080,
//! SEM-gx-substrate-081, SEM-gx-substrate-082, SEM-gx-substrate-083, SEM-gx-substrate-084,
//! SEM-gx-substrate-085, SEM-gx-substrate-086)
//!
//! Spec: 41 §6 for "errors are typed with thiserror" and for the layer rule these variants exist to
//! keep, 41 §4 for the seven `# Errors` sections each one is the vocabulary of. The ruling is
//! **E-M4-28** (`req/38_ERRATA_2026-08-07.md` §30 M4H2-2, adopted (a)).
//!
//! # The layer split
//!
//! §30, verbatim: "a failure of the outside world ('could not be read') is the adapter layer's
//! vocabulary; a rejected argument is gx-core's vocabulary -- **that layer split is 41 §6's
//! implementation form**". gx-core says the same thing from its own side -- "The crate does no
//! I/O (41 §6), so there is no error here that comes from the outside world -- every variant is a
//! rejected argument" -- and that sentence is why hand 2 could compile the trait against
//! [`gx_core::Result`] and still leave an fs adapter with no way to report a file it could not read
//! (req/71 §2 M4H2-2).
//!
//! So this enum is **the outside world**, plus [`Error::Core`], which carries a lower refusal
//! outward without relabelling it. The two vocabularies are disjoint by construction and
//! `crates/gx-substrate/tests/substrate_error.rs` asserts that they stay so: a name in both would
//! leave a caller unable to tell "the argument was wrong" from "the world would not answer", which
//! is the same confusion E-M4-15 refused to encode in a `bool`.
//!
//! # Why it arrives in hand 3 rather than in hand 4
//!
//! §30 again: "putting it **at the start of hand 3 rather than hand 4** is to avoid the rework of
//! building the conformance harness on the wrong `Result` type and then swapping it out". The
//! harness in `gx-substrate-conformance` calls all seven
//! methods and folds their failures into a report; building that on a `Result` that has to change
//! would mean writing it twice.
//!
//! # Which failure each variant is for
//!
//! One row per variant, naming the method whose documented failure it spells. The table is the
//! reason the vocabulary is ten words and not eleven: 52 contract 2 forbids inventing a refusal
//! nobody asked for, and `every_variant_is_the_vocabulary_of_a_documented_failure` compares this
//! table with the enum so that a variant added without a documented failure behind it is a red test
//! rather than a word that accumulates. Eight of the ten arrived with hands 3-5; **E-M4-32** and
//! **M4H5-5, adopted (b)** (req/38 §33) added the last two, and both of them are refusals of an
//! *argument* that gx-core could not have spelled -- what counts as "a position" and "the same
//! object" is an adapter's convention.
//!
//! | variant | where the failure is documented | the clause it spells |
//! |---|---|---|
//! | `ApplyFailed` | `SubstrateAdapter::apply` | "When the delta cannot be applied" (43 T-11 turns it into `AbortReason::ApplyFailed`) |
//! | `Core` | every method | carries the lower gx-core refusal straight outward (`From<gx_core::Error>`) |
//! | `ForeignDelta` | `apply` / `invert` / `commutation` | "two substrates" -- a delta another adapter wrote |
//! | `LocatorMismatch` | `SubstrateAdapter::invert` | the delta and `pre` name different objects = an engine wiring bug (**E-M4-32**) |
//! | `NotAPosition` | the position argument of `apply` / `invert` / `commutation` | "the argument is not a position" -- not a failure to apply (**M4H5-5, adopted (b)**) |
//! | `NotDigestible` | [`crate::PlannedDelta::new`] | gx-canon cannot give this projection a canonical form (42 §1.1) |
//! | `NotPlannable` | `SubstrateAdapter::plan` | "When no delta can be planned for this intent against this snapshot" |
//! | `PayloadUnreadable` | `invert` / `commutation` | "a payload this adapter did not write" -- the grammar cannot be read within the same substrate |
//! | `Unimplemented` | whichever of 41 §4's 7 methods this adapter does not yet have | says "unimplemented" as a value rather than as a panic or an `Ok` -- the harness maps only this one to "NOT_SUPPLIED" (§31 M4H3-4(b)) |
//! | `Unreadable` | `snapshot` / `precondition` | "Whatever the adapter cannot read or cannot name" / "When the scope cannot be read" |

use gx_core::SubstrateKind;

/// Everything an adapter can refuse to do (41 §4's seven `# Errors` sections, **E-M4-28**).
///
/// Unlike [`gx_core::Error`], most of these come from outside the process: a file that is not there,
/// a scope that could not be read, a write that did not land. That asymmetry is the whole point of
/// the type existing.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// gx-core refused an argument, and the refusal crosses the boundary unchanged.
    ///
    /// `#[from]`, so an adapter writing `fp.cas_eq(&other)?` inside one of the seven methods carries
    /// [`gx_core::Error::FingerprintScopeMismatch`] outward as itself rather than as a string. What
    /// this variant deliberately does **not** do is re-spell those ten refusals: an adapter bug and
    /// an unreachable file are different facts, and a caller that cannot separate them cannot decide
    /// whether to retry.
    #[error(transparent)]
    Core(#[from] gx_core::Error),

    /// The substrate would not answer (`snapshot`, `precondition`).
    ///
    /// The locator is carried for the reason [`gx_core::Error::OrderExceeded`] carries the order:
    /// the person reading the refusal is the one who has to see which spelling failed. It is the
    /// **normalised** locator, since 41 §4 has `snapshot` receive one already normalised (H-2 /
    /// E-M4-12), so a reader is not left comparing an error against a path the adapter never used.
    #[error("the substrate would not answer for {locator:?}: {detail}")]
    Unreadable { locator: String, detail: String },

    /// No delta realises this intent against this snapshot (`plan`).
    ///
    /// Not the same as a delta that changes nothing: an empty change is a delta and would be
    /// returned as one. This is "the goal cannot be reached from here", which is a fact about the
    /// pair (**E-M4-4**) and not about the intent alone.
    #[error("no delta plans this intent against this snapshot: {detail}")]
    NotPlannable { detail: String },

    /// The delta could not be performed (`apply`).
    ///
    /// 43 T-11 is what an engine does with it: the transformation ends `Aborted(ApplyFailed)`. The
    /// variant says nothing about whether the substrate moved -- 45 §3 records that a non-atomic
    /// `apply` can fail halfway, and M4-13 (a) is the ruling that keeps v0.1's fs delta to the one
    /// shape where `rename` makes that impossible.
    #[error("the delta could not be applied: {detail}")]
    ApplyFailed { detail: String },

    /// A delta from another adapter was handed to this one (`apply`, `invert`, `commutation`).
    ///
    /// The delta-side twin of **E-M4-27**, and refused for the same reason `cas_eq` refuses a
    /// fingerprint across substrates: a `git` payload reaching an `fs` adapter is a mis-wired engine,
    /// and any answer other than a refusal reports the symptom instead of the fault. Both kinds are
    /// carried so that the wiring mistake is readable from the message.
    #[error(
        "this adapter speaks {expected:?} and the delta is {got:?}: a delta is only meaningful to \
         the adapter whose grammar wrote it (P-6, 42 §3.4)"
    )]
    ForeignDelta {
        expected: SubstrateKind,
        got: SubstrateKind,
    },

    /// `invert` was handed a `pre` that names another object (**E-M4-32**).
    ///
    /// 41 §4's `invert(delta, pre)` takes two values that have to be about one object, and nothing in
    /// the type system says so. Hand 5 answered `Ok(None)` here and raised the reading against itself
    /// (req/74 §2 M4H5-1); §33 took the other case:
    ///
    /// > "passing a `pre` unrelated to the delta is an engine wiring bug, and `Ok(None)`->Escalate
    /// > hides that bug by disguising it as the legitimate business condition 'the inverse cannot be
    /// > constructed' (the same argument E-M4-27 used to refuse `Ok(false)`)"
    ///
    /// So the two answers now mean two things nobody can confuse: [`Option::None`] is a *real*
    /// business condition (the escrow ceiling, an old content already gone) that **E-M3-4** escalates
    /// to a human, and this is a defect in whoever assembled the call. Both locators are carried,
    /// because a wiring bug is diagnosed by seeing which two objects were put side by side.
    #[error(
        "this delta is about {expected:?} and the snapshot is of {got:?}: an inverse is relative to \
         the state of the object the delta changes (41 §4, E-M4-32)"
    )]
    LocatorMismatch { expected: String, got: String },

    /// A locator that is not a position this adapter can act on (**M4H5-5, adopted (b)**).
    ///
    /// "the argument is not a position" rather than "the apply failed". Hand 5 spelled the fs
    /// adapter's refusal of a
    /// relative locator [`Error::ApplyFailed`] and raised it (req/74 §2 M4H5-5): 43 T-11 turns that
    /// variant into `AbortReason::ApplyFailed`, so borrowing it would record a change that failed
    /// where no change was ever describable. It is the same three-way argument [`Error::Unimplemented`]
    /// is the answer to -- a panic, a borrowed variant and an invented `Ok` are each a misstatement --
    /// applied to an argument instead of to a method.
    ///
    /// What counts as a position is the adapter's (**ASM-69-3** makes it an absolute path for fs
    /// v0.1), which is why the word lives here and not in gx-core: gx-core refuses arguments it can
    /// judge on its own, and no locator convention is one of them. Both spellings are carried, since a
    /// caller reading "that is not a position" needs to see what the normalisation made of what they
    /// wrote.
    #[error(
        "{locator:?} normalises to {normalised:?}, which is not a position this adapter can act on"
    )]
    NotAPosition { locator: String, normalised: String },

    /// The payload is this adapter's substrate but not its grammar (`invert`, `commutation`).
    ///
    /// Distinct from [`Error::ForeignDelta`]: there the delta belonged to somebody else, here it
    /// claims to belong here and does not parse -- a version the adapter no longer writes, or bytes
    /// that were damaged. 41 §4's `invert` documentation names the difference this preserves:
    /// "Cannot be answered" and "the answer is no inverse" are different, which is why `invert`
    /// returns `Result<Option<_>>` and this is the `Err` half.
    #[error("this adapter cannot read the payload it was handed: {detail}")]
    PayloadUnreadable { detail: String },

    /// gx-canon has no canonical form for a value this crate had to name.
    ///
    /// Reachable from [`crate::PlannedDelta::new`], which mints a delta's own `DeltaRef` from its
    /// projection (**M4H1-3**, adopted (a), req/38 §29). The same variant name gx-gate uses for the
    /// same
    /// fact, and the same reading: a value that cannot be encoded has no identity, which is a
    /// statement about the record rather than about the change.
    #[error("gx-canon cannot give this value an identity: {detail}")]
    NotDigestible { detail: String },

    /// This adapter does not implement the method that was called, and says so out loud.
    ///
    /// An adapter is built one hand at a time -- `gx-adapter-fs` gets `kind`/`snapshot`/`plan`/
    /// `precondition` in M4 hand 4 and `apply`/`invert` in hand 5 (req/69 §6.2) -- and the three
    /// spellings of "not yet" available to the hand in between are all worse than this one. `todo!()`
    /// and `unimplemented!()` panic, and 41 §6 counts a panic as a bug; returning
    /// [`Error::ApplyFailed`] would say the delta could not be applied, which is a claim about the
    /// delta; and answering `Ok` with something invented is the fail-open req/29 §4 forbids.
    ///
    /// It is a permanent word rather than a scaffold. 51 §7's completion condition is "no adapter
    /// satisfies the M4/M7 completion condition unless it passes all seven of the above contracts",
    /// and an adapter that has not implemented a method has not failed a contract -- it has left it
    /// **unmeasured**, which is the distinction §31 M4H3-4 (b) split `is_conformant` and
    /// `is_complete` to carry. The harness reads this variant and reports "NOT_SUPPLIED"; every
    /// other refusal it reports as a failure. That mapping is the reason "unimplemented" needs a
    /// name of its own instead of borrowing one.
    ///
    /// The method is carried so that a report line names which of the seven, and `detail` says which
    /// hand or version owns it.
    #[error("this adapter does not implement `{method}` yet: {detail}")]
    Unimplemented { method: String, detail: String },
}

/// This crate's error vocabulary, declared once (**E-M2-23**, **E-M4-28**).
///
/// The form is gx-core's, which took it from gx-gate: one declaration, [`Error::kind`] exhaustive
/// with no `_` arm, and a test that reads the enum out of the source rather than matching on it.
/// The refusal "code outside the declaration is refused at construction time" is by type here as it
/// is in gx-core -- the vocabulary *is*
/// the enum, an enum is closed, so a kind outside this table is not refused, it is unconstructible.
///
/// Sorted, so that adding a name is an insertion a reviewer sees rather than an append.
pub const ERROR_KINDS: [&str; 10] = [
    "ApplyFailed",
    "Core",
    "ForeignDelta",
    "LocatorMismatch",
    "NotAPosition",
    "NotDigestible",
    "NotPlannable",
    "PayloadUnreadable",
    "Unimplemented",
    "Unreadable",
];

impl Error {
    /// Which of [`ERROR_KINDS`] this refusal is.
    ///
    /// The name and nothing else, for the reason [`gx_core::Error::kind`] gives: the message is
    /// [`std::fmt::Display`]'s and carries the values, while this is the word a caller matches on or
    /// writes into a log field.
    ///
    /// # Exhaustive on purpose
    ///
    /// No `_` arm. An eleventh variant added later does not compile until it is named here and in
    /// [`ERROR_KINDS`], which is what makes the table a definition rather than a description. The
    /// eighth ([`Error::Unimplemented`]) arrived that way in M4 hand 4, and the ninth and tenth
    /// ([`Error::LocatorMismatch`], [`Error::NotAPosition`]) in hand 6.
    ///
    /// [`Error::Core`] answers `"Core"` rather than the inner variant's name. The inner name belongs
    /// to gx-core's vocabulary and this table has to stay disjoint from it (`Display` is
    /// `transparent`, so nothing is hidden from a reader); `e.kind()` answers "whose refusal is
    /// this" and `e.to_string()` answers "which one".
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Core(_) => "Core",
            Error::Unreadable { .. } => "Unreadable",
            Error::NotPlannable { .. } => "NotPlannable",
            Error::ApplyFailed { .. } => "ApplyFailed",
            Error::ForeignDelta { .. } => "ForeignDelta",
            Error::LocatorMismatch { .. } => "LocatorMismatch",
            Error::NotAPosition { .. } => "NotAPosition",
            Error::PayloadUnreadable { .. } => "PayloadUnreadable",
            Error::NotDigestible { .. } => "NotDigestible",
            Error::Unimplemented { .. } => "Unimplemented",
        }
    }
}

/// The `Result` 41 §4's seven signatures return (**E-M4-28**).
///
/// 41 §4 writes a bare `Result<..>` and every crate in this workspace declares its own, so the bare
/// name resolves to "the crate's own" by precedent (gx-canon, gx-core, gx-gate, gx-log, gx-witness).
/// Until hand 3 this one resolved to gx-core's, which compiled and could not say "could not be
/// read".
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The conversion E-M4-28 asks for, exercised rather than declared.
    #[test]
    fn a_core_refusal_crosses_the_boundary_as_itself() {
        let core = gx_core::Error::OrderExceeded { got: 7, max: 2 };
        let wrapped: Error = core.clone().into();
        assert_eq!(wrapped.kind(), "Core");
        assert_eq!(
            wrapped.to_string(),
            core.to_string(),
            "`transparent` means the lower message survives the hop"
        );
    }

    /// Every kind is one of the declared eight, taken through the values rather than the table.
    #[test]
    fn every_variant_answers_with_a_declared_kind() {
        let all = [
            Error::Core(gx_core::Error::TargetMissing),
            Error::Unreadable {
                locator: "/tmp/x".to_string(),
                detail: "no such file".to_string(),
            },
            Error::NotPlannable {
                detail: "the goal is already met".to_string(),
            },
            Error::ApplyFailed {
                detail: "rename refused".to_string(),
            },
            Error::ForeignDelta {
                expected: SubstrateKind::Fs,
                got: SubstrateKind::Git,
            },
            Error::LocatorMismatch {
                expected: "/tmp/x".to_string(),
                got: "/tmp/y".to_string(),
            },
            Error::NotAPosition {
                locator: "relative/x".to_string(),
                normalised: "relative/x".to_string(),
            },
            Error::PayloadUnreadable {
                detail: "unknown payload version".to_string(),
            },
            Error::NotDigestible {
                detail: "no canonical form".to_string(),
            },
            Error::Unimplemented {
                method: "apply".to_string(),
                detail: "M4 hand 5 supplies it".to_string(),
            },
        ];
        assert_eq!(all.len(), ERROR_KINDS.len());
        let mut kinds: Vec<&str> = all.iter().map(Error::kind).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, ERROR_KINDS.to_vec());
    }
}
