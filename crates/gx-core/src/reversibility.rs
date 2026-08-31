// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! C-25's three-valued answer to "can this call be undone?" (relocated by **DR-46-26**).
//!
//! 🔴 **superseded in part → [`C25-QUESTION-RULING-R964`](Reversibility) below** (`req/983`, H-9,
//! 2026-08-31). The line above and the variant doc on [`Reversibility::True`] asked two different
//! questions of one word, and the observation road is the coordinate where the two answers differ.
//! The ruling is on the type; this line is kept unedited because the two spellings of the question
//! are the evidence.
//!
//! # Why the type is down here
//!
//! It was declared in `gx-adapter-mcp` while it was one adapter's private arithmetic. DR-46-26
//! makes it part of `SubstrateAdapter::invert`'s return, and **the relocation is forced rather
//! than preferred**: `gx-adapter-mcp` depends on `gx-substrate`, so a trait in `gx-substrate` that
//! named `gx_adapter_mcp::Reversibility` would cycle.
//!
//! `gx-substrate` would have been far enough for the trait alone. It is **not** far enough for the
//! seat `req/38`'s S1 ruling 5 adds to 42 §3.10: `gx-witness` depends on `gx-core`, `gx-canon` and
//! `gx-log` and **not** on `gx-substrate`, so a receipt payload that names this type can only name
//! it here. One type, two crates that must see it, exactly one crate both of them already depend
//! on — which is the same argument that put `VerdictKind` here in M3 (**E-M3-2**), and the same
//! rule: the data comes down, the computation stays up. Nothing here reads, decides or compares;
//! `gx_adapter_mcp::invert_with_verdict` is still the only thing that works the answer out.

use serde::{Deserialize, Serialize};

/// 🔴 **C-25**'s three values: whether a call can be undone, as a first-class answer.
///
/// 11 §5-2 C-25 puts it as plainly as it can be put — the product's first-class output is not
/// "the change was reversed" but "here is whether it can be, with the reason and the boundary".
/// Two values cannot carry that: "no inverse exists" and "we could not find out" are different
/// facts with different remedies, and folding them loses exactly the information an operator acts
/// on.
///
/// `gx_adapter_mcp::McpAdapter::reversibility` answers it for one planned call, and
/// `gx_substrate::InvertOutcome` is how it crosses the adapter boundary (**DR-46-26**).
///
/// # `C25-QUESTION-RULING-R964` — which of the two questions this word answers
///
/// 🔴 **`req/983` (H-9, `req/954` §3-1, `req/957` §4-3), 2026-08-31.** This type's module line and
/// the `True` variant's line stated two questions and the codebase read them as one:
///
/// - module line, and this heading's first sentence: *"can this call be undone?"* — **operational**
/// - [`Reversibility::True`]: *"an inverse was constructed"* — **mechanistic**
///
/// **The mechanistic reading wins, and the canon decides it rather than this file.** 11 §5-2 C-25
/// asks the operational question in its title and then *defines* its own `true` mechanistically —
/// the parenthesis on `reversible: true` reads, in translation, "an inverse constructed, checked,
/// and escrowed". C-25 assumed the two readings coincide, and for `fs`, `git` and MCP substrates
/// they do.
///
/// **They do not coincide on the observation road.** `gx_engine::is_observation_substrate` refuses
/// that undo *by type*, before the escrow is consulted, "for any observation of any attach-source,
/// ever" (`gx_engine::Error::InverseNotExecutableAtSubstrate`; `gx-core`'s own
/// [`crate::observation`] doc calls it a typed refusal). The inverse really was constructed and
/// escrowed, so `True` is true under C-25's definition — and the engine will still never execute
/// it. One word, answered honestly, that a receipt's reader will read as the other answer.
///
/// **What follows for a caller.** Read `True` as *"an inverse exists and is escrowed"*, never as
/// *"undo will succeed"*. The second question is a **different** question, and C-25's own third
/// clause — the surface we cannot bind must declare that it cannot be bound, machine-readably —
/// requires it to be declared, not merely written down somewhere.
///
/// 🔴 **It is not declared on the receipt today.** `docs/LIMITS.md` discloses the refusal, but a
/// third party holding only the signed bytes does not hold `docs/LIMITS.md`. That residue is filed
/// as **DR-46-48** with an owner and a release condition (`req/983` §4) rather than left as a TODO,
/// and this ruling deliberately does **not** close H-9: it fixes the vocabulary so the additive
/// field can be named from the question instead of from a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Reversibility {
    /// An inverse was constructed. `invert` answered `Some`.
    ///
    /// 🔴 This is the whole of what the word claims — see `C25-QUESTION-RULING-R964` on
    /// [`Reversibility`]. It does **not** claim that `Engine::undo` will execute.
    True,
    /// No inverse exists for this call: no tool is declared to undo it, or the declaration cannot
    /// be resolved from this call's material, or the body is over the escrow ceiling.
    False,
    /// 🔴 **DR-46-9 A-4**: the prior could not be read, so whether an inverse exists was never
    /// established. Reachable only under `OnReadFailure::Unknown` — under the default the effect
    /// is refused and no transformation is committed at all.
    Unknown,
}

impl Reversibility {
    /// The three, in the order C-25 states them. A count a probe can hold a vocabulary against.
    pub const ALL: [&'static str; 3] = ["true", "false", "unknown"];

    /// The word, for a report line and for a receipt reader.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Reversibility::True => "true",
            Reversibility::False => "false",
            Reversibility::Unknown => "unknown",
        }
    }
}
