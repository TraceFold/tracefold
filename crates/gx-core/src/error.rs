// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Errors, and the canonical reasons a transformation stops short of Committed.
//!
//! Spec: 41 §3 for `AbortReason`, 41 §6 for the rule that errors are typed with `thiserror` and
//! that a panic is a bug, 42 §0 for this module's contents.

use serde::{Deserialize, Serialize};

use crate::object::SubstrateKind;

/// Everything gx-core can refuse to do.
///
/// The crate does no I/O (41 §6), so there is no error here that comes from the outside world --
/// every variant is a rejected argument. `panic` is not an alternative: 41 §6 makes panics bugs,
/// which is why the order check returns rather than asserts.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// AC-003 names this variant. v0.1 admits orders up to 2 (ASM-6, DR-7 DEFAULT: <=2); `max` is
    /// carried in the value rather than left to the message so a caller can report the ceiling it
    /// actually hit if a later DR moves it.
    #[error("order {got} exceeds the v0.1 maximum of {max} (ASM-6, DR-7)")]
    OrderExceeded {
        /// The order the caller asked for.
        got: u8,
        /// The ceiling that refused it (2 in v0.1, DR-7).
        max: u8,
    },

    /// 42 §1.2. The readable form has exactly one spelling per digest, so anything else is a
    /// refusal rather than a best guess (req/26 §3).
    ///
    /// This variant arrived with E-JCS-1 (`req/38_ERRATA_2026-08-07.md` §5), which made the
    /// `gx1:` form the JSON embedding as well as the display form -- and therefore made reading
    /// one back this crate's business, since `Deserialize` for `Cid` lives here.
    #[error("not a `gx1:<base32>` content identifier: {detail} (42 §1.2)")]
    CidText {
        /// What was wrong with the text, in words -- the one field whose reader is a person.
        detail: String,
    },

    /// 46 §2.1's `composable f g := f.dst = g.src`, written in 41's vocabulary: the digest `f`
    /// promises to leave behind is not the digest `g`'s subject denotes. There is no composite
    /// arrow to name, so `compose` returns rather than inventing one.
    #[error(
        "f.target is not the digest g.subject denotes, so the two do not compose (46 §2.1 composable)"
    )]
    NotComposable,

    /// The caller's resolver could not say which digest a subject denotes. Distinct from
    /// [`Error::NotComposable`]: there the answer was no, here there was no answer, and a
    /// composability check that treats "unknown" as "yes" would compose arrows that do not meet.
    #[error("the resolver produced no digest for g.subject, so composability cannot be decided")]
    SubjectUnresolved,

    /// `target` is `None` while the transformation is still a Draft (41 §3: fixed by `plan()`).
    /// A Draft has no promised post-state, so nothing can be composed onto it yet.
    #[error("f has no target: a Draft has no promised post-state to hand on (41 §3, 43 T-2)")]
    TargetMissing,

    /// **E-M3-13** ①: `created_at` is before the epoch.
    ///
    /// 42 §3.2 types a `Timestamp` as "UTC nanoseconds since the Unix epoch" and `i64` can hold
    /// values below it, so "negative" is representable (sem: SEM-gx-core-025) and means a moment
    /// that is not one of the
    /// moments this system records. `got` is carried rather than left to the message because a
    /// caller reporting the refusal has the same reason to name the value that
    /// [`Error::OrderExceeded`] does.
    ///
    /// The bound is `>= 0` and not `> 0`: the epoch itself is a value the crate hands out (gx-gate
    /// writes `Timestamp(0)` into an escalation ticket as the placeholder an engine overwrites), so
    /// refusing it would refuse a value gx builds on purpose.
    #[error("created_at {got} is before the Unix epoch (42 §3.2, E-M3-13 ①)")]
    CreatedAtNegative {
        /// The refused timestamp, in 42 §3.2's unit (UTC nanoseconds since the epoch).
        got: i64,
    },

    /// **E-M3-13** ②: `intent_id` is the all-zero placeholder.
    ///
    /// The all-zero id is this crate's own spelling for "no value yet" (sem: SEM-gx-core-026) --
    /// `PROVISIONAL_ID` in `transformation.rs` is those thirty-two bytes -- and 42 §1.3 puts
    /// `intent_id` **inside** the
    /// `Transformation` IdentityView. An arrow carrying it therefore has an identity computed over
    /// a placeholder, and two unrelated arrows built that way collide on the field that is supposed
    /// to say which Draft each came from.
    ///
    /// No value is carried: there is exactly one placeholder and naming it in the message would
    /// print thirty-two zeroes to say what the variant already says.
    #[error("intent_id is the all-zero placeholder, which names no Draft (42 §1.3, E-M3-13 ②)")]
    IntentIdUnset,

    /// **E-M3-13** ③: a composite dated before the arrows it is made of.
    ///
    /// `compose`'s only, because it is the only place that holds `f` and `g`.
    /// [`crate::Transformation::new`] cannot evaluate it -- the ids in a `parents` list name values
    /// this crate may not resolve (41 §6 forbids the I/O) -- which is why the predicate is stated
    /// for composition alone in req/38 §22 rather than for every constructor.
    ///
    /// `at_least` is `max(f.created_at, g.created_at)`, and the comparison is `>=`, so two arrows
    /// and their composite recorded in the same nanosecond are admitted.
    #[error(
        "created_at {got} is before max(f, g) = {at_least}: a composite cannot predate the arrows \
         it is made of (E-M3-13 ③)"
    )]
    CreatedAtBeforeParts {
        /// The composite's refused `created_at`.
        got: i64,
        /// The floor it had to reach: `max(f.created_at, g.created_at)`.
        at_least: i64,
    },

    /// **E-M4-15**: two fingerprints whose `scope` differs were compared.
    ///
    /// 42 §3.5, verbatim: "equality between `Fingerprint`s is defined as `substrate` and `scope`
    /// agreeing and the `digest` byte strings agreeing (a comparison across differing `scope`s has
    /// no meaning and is **treated as an adapter implementation error**)" (quoted in
    /// SEM-gx-core-027). A `bool` cannot carry that sentence -- `false` would read as "the state
    /// moved" and the CAS check of CON-2 would abort a commit for a reason that never happened -- so
    /// [`crate::Fingerprint::cas_eq`] returns this instead, which is E-M4-15's "type shape".
    ///
    /// Both scopes are carried for the reason [`Error::OrderExceeded`] carries the order: the
    /// adapter author reading the refusal is the one who has to see which two spellings disagreed.
    #[error(
        "fingerprint scopes differ ({left:?} vs {right:?}), so comparing them has no meaning \
         (42 §3.5, E-M4-15)"
    )]
    FingerprintScopeMismatch {
        /// The scope of the left-hand fingerprint of the refused comparison.
        left: String,
        /// The scope of the right-hand one.
        right: String,
    },

    /// **E-M4-27**: two fingerprints computed by different adapters were compared.
    ///
    /// Hand 1 implemented 42 §3.5 to the letter -- the sentence names only the `scope` mismatch as
    /// "adapter implementation error" -- and raised the asymmetry (req/70 §2 M4H1-1). req/38 §29
    /// ruled case (b) (quoted in SEM-gx-core-028):
    ///
    /// "`cas_eq` returns **`Err` on a substrate mismatch too** (the same 'implementation error'
    /// class as a scope mismatch -- comparing another adapter's fingerprint is an engine wiring bug,
    /// and if it turned into an `Ok(false)` -> 'the state moved' Abort the bug would be hidden; the
    /// same argument by which E-M4-15 refused PartialEq). 42 §3.5's equality is read as '**defined
    /// only between the products of one adapter**'."
    ///
    /// The failure this refuses to hide is a mis-wired engine: an `fs` fingerprint reaching a `git`
    /// adapter's CAS check answers "the state moved" every time, so every commit through that path
    /// aborts with [`AbortReason::PreconditionChanged`] -- a reason that names a change nobody made,
    /// while the wiring stays wrong. AC-034 (M5) injects a `Fingerprint₁ != Fingerprint₀` **within**
    /// one substrate, so nothing it measures moves.
    ///
    /// Both kinds are carried, for the reason [`Error::FingerprintScopeMismatch`] carries both
    /// scopes.
    #[error(
        "fingerprints from different substrates ({left:?} vs {right:?}) are not comparable: 42 §3.5 \
         equality is defined between products of one adapter (E-M4-27)"
    )]
    FingerprintSubstrateMismatch {
        /// The substrate of the left-hand fingerprint of the refused comparison.
        left: SubstrateKind,
        /// The substrate of the right-hand one.
        right: SubstrateKind,
    },

    /// **M4H1-2, adopted (a)**: a `scope` longer than [`crate::MAX_SCOPE_BYTES`] reached
    /// [`crate::Fingerprint::new`].
    ///
    /// req/38 §29, verbatim: "gx-core too gets a bound on the scope string plus digesting when it is
    /// exceeded (the same shape as gx-gate's M3-21/ASM-60-4, 1024 bytes). It goes in **in the same
    /// window as hand 4, where the scope generation rule (ASM-69-1) materialises** (generation and
    /// bound are not decided in separate hands)" (quoted in SEM-gx-core-029).
    ///
    /// The bound is here rather than in an adapter because 42 §3.5 lets a `scope` reach as wide as
    /// the adapter likes, and every fingerprint is hashed into a receipt gx-witness signs: an
    /// unbounded string in a signed record is 42 §3.8's "long text is digested" problem one type
    /// over. What the ruling calls "digesting when exceeded" cannot happen *here* -- A-1 keeps every
    /// digest in gx-canon and this crate depends on no encoder -- so the two halves are split the way
    /// E-M2-1 splits everything else: the type and the bound are the lower layer (this refusal), the
    /// digesting is the upper layer (sem: SEM-gx-core-030)
    /// (`gx_substrate::fingerprint::elide_scope`, which an adapter calls before it gets here --
    /// a crate above this one, so the name is not a link).
    /// The bytes and the bound both travel in the value, for the reason [`Error::OrderExceeded`]
    /// carries its ceiling. The message's closing clause is E-M4-1's rule in English (sem:
    /// SEM-gx-core-031).
    #[error(
        "a fingerprint scope of {bytes} bytes exceeds the {max}-byte bound (M4H1-2); an adapter \
         elides it to a digest before constructing (E-M4-1: computation stays in the upper layer)"
    )]
    ScopeTooLong {
        /// How long the refused scope was, in bytes (the unit ASM-60-4 words the bound in).
        bytes: usize,
        /// The bound it exceeded: [`crate::MAX_SCOPE_BYTES`].
        max: usize,
    },
}

/// The crate's error vocabulary, declared once (**E-M2-23**, **H-3**).
///
/// req/38 §25, verbatim: "**H-3 (M4 window)**: gx-core's Error vocabulary table takes the E-M2-23
/// form in the next hand that touches gx-core (the start of M4 = the same window as the H-1
/// implementation)" (quoted in SEM-gx-core-032). `gx-gate`'s `ERROR_KINDS` is the precedent
/// (req/64 §2), and E-3 (req/38 §23) is the ruling that the table lives beside the enum rather than
/// in 41 §6: "E-M2-23's 'vocabulary table in 41 §6' **cannot be written** under the spec freeze (52)
/// -- the canonical home of an erratum is the req/38 ledger, and on the code side `ERROR_KINDS`
/// plus the reconciliation test are the effective form" (quoted in SEM-gx-core-033).
///
/// # What being outside this table would mean, and why it cannot happen
///
/// gx-gate's vocabulary is a set of strings a caller supplies, so "a code outside the declaration is
/// refused at construction" (sem: SEM-gx-core-034) is a
/// check inside `Reason::new`. This one is not: the vocabulary **is** the enum above, and an enum is
/// closed. A kind outside the table is therefore not refused at construction, it is unconstructible
/// -- and [`Error::kind`] below is exhaustive without a `_` arm, so adding a variant without adding
/// its name here stops the crate compiling. `crates/gx-core/tests/core_error_vocabulary.rs` holds
/// the other half: the table and the enum are read out of this file and compared, because a `match`
/// written by the same hand that adds a variant would never notice one.
///
/// Sorted, so that adding a name is an insertion a reviewer sees rather than an append.
pub const ERROR_KINDS: [&str; 11] = [
    "CidText",
    "CreatedAtBeforeParts",
    "CreatedAtNegative",
    "FingerprintScopeMismatch",
    "FingerprintSubstrateMismatch",
    "IntentIdUnset",
    "NotComposable",
    "OrderExceeded",
    "ScopeTooLong",
    "SubjectUnresolved",
    "TargetMissing",
];

impl Error {
    /// Which of [`ERROR_KINDS`] this refusal is.
    ///
    /// The name and nothing else: the message is [`std::fmt::Display`]'s and carries the values,
    /// while this is the word a caller matches on or writes into a log field. 44 §2.3's `gx_code`
    /// is a different vocabulary at a different layer -- gx-gate's `REASON_CODES` is where the two
    /// are related (M3-08) -- and nothing here claims a mapping that no ruling has made.
    ///
    /// # Exhaustive on purpose
    ///
    /// No `_` arm. A twelfth variant added later does not compile until it is named here and in
    /// [`ERROR_KINDS`], which is what makes the table a definition rather than a description. The
    /// tenth (**E-M4-27**'s) and the eleventh (**M4H1-2**'s [`Error::ScopeTooLong`]) both arrived
    /// exactly that way: the ruling was written, and three instruments -- this match, the table, and
    /// `compose_range.rs`'s exhaustive one -- refused to build until the name was in all three.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Error::OrderExceeded { .. } => "OrderExceeded",
            Error::CidText { .. } => "CidText",
            Error::NotComposable => "NotComposable",
            Error::SubjectUnresolved => "SubjectUnresolved",
            Error::TargetMissing => "TargetMissing",
            Error::CreatedAtNegative { .. } => "CreatedAtNegative",
            Error::IntentIdUnset => "IntentIdUnset",
            Error::CreatedAtBeforeParts { .. } => "CreatedAtBeforeParts",
            Error::FingerprintScopeMismatch { .. } => "FingerprintScopeMismatch",
            Error::FingerprintSubstrateMismatch { .. } => "FingerprintSubstrateMismatch",
            Error::ScopeTooLong { .. } => "ScopeTooLong",
        }
    }
}

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Why a transformation ended without committing (41 §3, ASM-15).
///
/// This is not an `Error`. It is a recorded outcome that goes into the journal and the receipt
/// (42 §3.13's `Aborted` record), which is why it derives serde and `Error` does not: an abort is
/// something the system reports about itself afterwards, not something a function returns.
/// The enumeration is closed on purpose -- ASM-15 calls it the canonical definition, and an open
/// `Custom` would let each adapter invent its own vocabulary for stopping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AbortReason {
    /// The CAS check failed: `Fingerprint₁ != Fingerprint₀` (41 §5-5b, CON-2).
    PreconditionChanged,
    /// The adapter's `apply` failed after admission (41 §5-6). Fail-safe: nothing was committed,
    /// and 43 T-10c is the transition rule that lands here.
    ApplyFailed,
    /// Fail-closed default when a verifier cannot be reached (DR-2, NFR-005).
    VerifierUnavailable,
    /// The transformation's window ran out before it committed (ASM-15's timeout row) -- an
    /// outcome the engine records, not an error a function returns.
    Expired,
    /// The owner of the intent withdrew it while still uncommitted (ASM-15). Recorded so a
    /// cancelled Draft leaves an accountable trace rather than vanishing.
    OwnerCancelled,
    /// The engine's own fault: an invariant broke inside the pipeline (41 §6 makes a panic a
    /// bug, and this is the recorded shape of the abort that bug causes).
    InternalError,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No AC covers `AbortReason` -- 41 §3 requires the type, and the M1 acceptance set
    /// (AC-001..008, AC-058) does not reach it. Recorded here rather than left untested.
    #[test]
    fn abort_reason_has_the_six_asm_15_variants_and_round_trips() {
        let all = [
            AbortReason::PreconditionChanged,
            AbortReason::ApplyFailed,
            AbortReason::VerifierUnavailable,
            AbortReason::Expired,
            AbortReason::OwnerCancelled,
            AbortReason::InternalError,
        ];
        assert_eq!(all.len(), 6);
        for r in all {
            match r {
                AbortReason::PreconditionChanged
                | AbortReason::ApplyFailed
                | AbortReason::VerifierUnavailable
                | AbortReason::Expired
                | AbortReason::OwnerCancelled
                | AbortReason::InternalError => {}
            }
        }
    }

    #[test]
    fn error_displays_the_offending_order() {
        let e = Error::OrderExceeded { got: 7, max: 2 };
        assert!(e.to_string().contains('7'));
    }
}
