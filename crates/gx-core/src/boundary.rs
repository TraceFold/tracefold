// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-28**: where the replay-deterministic part of a change ends and the LLM-originated
//! part begins, as a value two faces can carry.
//!
//! # The sentence this type exists to make checkable
//!
//! `req/38` §255 ruling 4 raises it in one line: *this far is deterministic (replayable), from here
//! on it is LLM-originated* — a receipt should say so on its face
//! — and `req/459` fixes the core of the taxonomy: `deterministic_replay` is a claim about
//! **verdict derivation** (same input, same verdict), `llm_originated` is a claim about **input
//! generation** and records an origin rather than asserting a truth, `mixed` is the case where the
//! two stages differ *and are enumerated*, and a stage that cannot be established is `unknown` —
//! first class — the sibling of the "cannot-be-established" contract the same ruling names — and
//! never a silent collapse into one of the other three.
//!
//! # Why the claim is narrow, said before the type is read
//!
//! `req/444` §1's own counter-argument refuses the wide version: the engine has non-determinism in it
//! (clocks, key generation, filesystem order), so **"deterministic" here means and only means
//! "replaying the same input yields the same verdict"**. `req/454`'s DR-46-27 already turned that
//! sentence into machinery on the gate's side; this type is where a reader of one receipt gets to
//! see which side of it a change was on.
//!
//! # Why the type is in `gx-core`
//!
//! Two faces name it and they are in different crates. The **declaration** face is
//! `gx_adapter_mcp::catalogue`'s reserved slot (a deployment says where its inputs come from); the
//! **attest** face is `gx_witness::receipt::ReceiptPayload` (one transformation's own answer).
//! `gx-witness` does not depend on `gx-substrate` and `gx-adapter-mcp` does; `gx-core` is the one
//! crate both already name. That is `Reversibility`'s argument one erratum earlier, and the same
//! rule applies here: **the data comes down, the computation stays up**. Nothing in this module
//! reads the world. [`DeterminismBoundary::of_stages`] is arithmetic on two values a caller
//! already holds.

use serde::{Deserialize, Serialize};

/// One stage's class: what is known about how *that* stage produced what it produced.
///
/// Three values rather than two, for [`crate::Reversibility`]'s reason: "this stage is not
/// replay-deterministic" and "nobody established what this stage was" are different facts with
/// different remedies, and folding them loses the one an operator acts on.
///
/// `req/459` names exactly two stages, and they are the two [`DeterminismBoundary::of_stages`]
/// takes: **input generation** (whose class a deployment declares, and whose LLM origin the actor
/// on a transformation can override) and **verdict derivation** (whose class gx observes, because
/// gx is the thing that derives it).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum BoundaryStage {
    /// Replaying this stage on the same input reproduces the same output. On the verdict-derivation
    /// stage this is the claim `req/454`'s KA machinery holds; it is **not** a claim that the stage
    /// has no clock in it, only that the clock does not reach the answer.
    DeterministicReplay,
    /// A model produced this stage's material. **A record of origin, not an assertion of truth**:
    /// the value says where the bytes came from and says nothing about whether they were right.
    LlmOriginated,
    /// 🔴 Not established. The first-class one: a deployment that has not declared where its inputs
    /// come from, or a road on which gx did not derive a verdict at all (43 T-4e calls no gate),
    /// says this rather than minting one of the other two.
    ///
    /// The `Default`, and that is the fail-honest direction rather than a convenience: a catalogue
    /// or a road that says nothing has established nothing, and the alternative default would be a
    /// determinism claim nobody made.
    #[default]
    Unknown,
}

impl BoundaryStage {
    /// The three words, in the order `req/459` states them. A vocabulary a probe can hold a
    /// declaration against.
    pub const ALL: [&'static str; 3] = ["deterministic_replay", "llm_originated", "unknown"];

    /// The word, for a catalogue's reserved slot and for a refusal sentence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            BoundaryStage::DeterministicReplay => "deterministic_replay",
            BoundaryStage::LlmOriginated => "llm_originated",
            BoundaryStage::Unknown => "unknown",
        }
    }
}

/// 🔴 **DR-46-28** — the boundary of one transformation, as a whole-transformation value.
///
/// # The four, and what makes each of them false
///
/// A taxonomy whose values cannot be falsified is decoration, and `req/459`'s KA is exactly that
/// question — *is the boundary claim itself checkable*. So each value is stated here beside the bed that makes
/// it false, and every one of those beds is refused by a machine rather than by this paragraph —
/// [`Self::of_stages`] for the three that are arithmetic, and
/// `gx_witness::receipt::ReceiptPayload::check_schema` for the three that a decoder can hand in.
///
/// | value | what it claims | the bed that makes it false |
/// |---|---|---|
/// | [`Self::DeterministicReplay`] | **both** stages replay-deterministic | either stage is not — `of_stages` answers `Mixed`, and a payload claiming it with no verdict is refused |
/// | [`Self::LlmOriginated`] | **both** stages LLM-originated | verdict derivation is gx's own and is replay-deterministic — a receipt claiming it is refused outright |
/// | [`Self::Mixed`] | the stages **differ**, and here they are | the two stages are equal — `of_stages` answers the collapsed value, and a payload carrying `Mixed` with equal stages is refused |
/// | [`Self::Unknown`] | **neither** stage was established | either stage was — `of_stages` will not mint `Unknown` over a stage that has a class |
///
/// The last row is the one worth naming: `unknown` is first class so that a stage nobody
/// established can be said out loud, and it is **not** an escape hatch, because the arithmetic
/// refuses to produce it wherever something was established.
///
/// # `Mixed` carries its enumeration
///
/// `req/459` ruling 3 words `mixed` as *enumerated by stage*. A bare marker would put
/// the claim beyond a reader's reach: `Mixed` and `Mixed` would be the same bytes for "the model
/// wrote the input and gx derived the verdict" and for anything else. The variant therefore carries
/// the two stages, which is what makes the equal-stages bed above a *machine* refusal rather than a
/// convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeterminismBoundary {
    /// Both stages are replay-deterministic. **Not reachable from gx's own writer**: gx does not
    /// observe how a human or an unattended process generated an input, so the writer says
    /// `Unknown` for that stage rather than minting a determinism claim. The value exists for a
    /// deployment that declares its input generation replay-deterministic — the declaration face.
    DeterministicReplay,
    /// Both stages are LLM-originated. **A receipt may not carry it**: gx's verdict derivation is
    /// gx's own construction and is replay-deterministic, so this value is a claim about a stage gx
    /// performed and did not perform this way. `check_schema` refuses it, and the value is left for
    /// the declaration face, where a tool class gx never gates can honestly carry it.
    LlmOriginated,
    /// The two stages differ, and they are named.
    Mixed {
        /// Where this transformation's input came from. Declared by a deployment
        /// (`gx_adapter_mcp::catalogue`'s reserved slot) and overridden by what the transformation's
        /// `Actor` says: an `Actor::Agent` is an LLM origin whatever a static file declared.
        input_generation: BoundaryStage,
        /// What gx did. [`BoundaryStage::DeterministicReplay`] when a gate derived a verdict;
        /// [`BoundaryStage::Unknown`] when none was asked (43 T-4e).
        verdict_derivation: BoundaryStage,
    },
    /// Neither stage was established.
    Unknown,
}

impl DeterminismBoundary {
    /// The four words, in `req/459` ruling 3's order.
    pub const ALL: [&'static str; 4] =
        ["deterministic_replay", "llm_originated", "mixed", "unknown"];

    /// 🔴 **The arithmetic the KA rests on**: the boundary two stages imply.
    ///
    /// Equal stages collapse to the value that says so; differing stages are `Mixed` and are
    /// enumerated. Total, `const`, and dependent on nothing but its two arguments — which is what
    /// lets a test build the bed that falsifies a value and read the answer off the same function
    /// production uses, rather than off an assertion that restates the claim.
    #[must_use]
    pub const fn of_stages(
        input_generation: BoundaryStage,
        verdict_derivation: BoundaryStage,
    ) -> Self {
        match (input_generation, verdict_derivation) {
            (BoundaryStage::DeterministicReplay, BoundaryStage::DeterministicReplay) => {
                Self::DeterministicReplay
            }
            (BoundaryStage::LlmOriginated, BoundaryStage::LlmOriginated) => Self::LlmOriginated,
            (BoundaryStage::Unknown, BoundaryStage::Unknown) => Self::Unknown,
            (input_generation, verdict_derivation) => Self::Mixed {
                input_generation,
                verdict_derivation,
            },
        }
    }

    /// What this boundary claims about **verdict derivation** — the stage gx performs.
    ///
    /// [`Self::Unknown`] claims nothing about either stage, so it answers
    /// [`BoundaryStage::Unknown`]. This is the projection `check_schema` holds its rules against,
    /// and it is a read of the value rather than of the world.
    #[must_use]
    pub const fn verdict_derivation(self) -> BoundaryStage {
        match self {
            Self::DeterministicReplay => BoundaryStage::DeterministicReplay,
            Self::LlmOriginated => BoundaryStage::LlmOriginated,
            Self::Mixed {
                verdict_derivation, ..
            } => verdict_derivation,
            Self::Unknown => BoundaryStage::Unknown,
        }
    }

    /// What this boundary claims about **input generation** — the stage a deployment declares.
    #[must_use]
    pub const fn input_generation(self) -> BoundaryStage {
        match self {
            Self::DeterministicReplay => BoundaryStage::DeterministicReplay,
            Self::LlmOriginated => BoundaryStage::LlmOriginated,
            Self::Mixed {
                input_generation, ..
            } => input_generation,
            Self::Unknown => BoundaryStage::Unknown,
        }
    }

    /// The word, for a report line and for a refusal sentence. `Mixed` names its two stages,
    /// because a refusal that said only "mixed" would be about a value the reader cannot see.
    #[must_use]
    pub fn as_str(self) -> String {
        match self {
            Self::DeterministicReplay => "deterministic_replay".to_string(),
            Self::LlmOriginated => "llm_originated".to_string(),
            Self::Mixed {
                input_generation,
                verdict_derivation,
            } => format!(
                "mixed({}/{})",
                input_generation.as_str(),
                verdict_derivation.as_str()
            ),
            Self::Unknown => "unknown".to_string(),
        }
    }
}
