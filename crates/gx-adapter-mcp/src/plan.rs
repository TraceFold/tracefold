//! `plan`, with no server in it, and the reason that is a law rather than a preference.
//!
//! 41 §4 calls `plan` 「純関数・副作用なし」 and §30 M4H2-3 採(b) read that for the trait as 「(intent, pre)
//! の対に対する決定性+substrate への**書き込み** 0」, adding 「読み込みは禁じない」.
//!
//! 🔴 **The road is open and this module does not take it.** The reason is **L1** (req/69 §3.4), which
//! quantifies determinism over the pair and moves the substrate in between: the same `(intent, pre)`
//! planned twice, with a tool call landing between the two, has to produce the **same delta**. A plan
//! that read the resource -- to embed its current contents, or a version, or an etag -- would produce a
//! different payload each time the server moved.
//!
//! What the payload says is therefore exactly what the agent asked for and nothing about the world:
//! 「call this tool, at this position, with these arguments」. [`crate::apply`] is where the world is
//! touched, at the moment the engine's CAS has just declared the pre-state current (41 §5-5b).
//!
//! # Why a module rather than a function in `adapter.rs`
//!
//! Because 「reaches no transport」 is a claim about source, and the cheapest honest way to measure it is
//! a file that never names one. `adapter.rs` reads resources -- `snapshot` and `precondition` must --
//! so a scan of that file could only ever be a scan of a function body. `tests/mcp_plan_purity.rs`
//! reads both, and adds the measurement text cannot make: a counting transport is handed to the
//! adapter and its two counters are **still zero** after a plan.
//!
//! The `pre` argument is unused, and that is the finding rather than an oversight -- the same one the
//! other two adapters record. **E-M4-4** put `pre` in the signature because 43 T-2 quantifies
//! determinism over 「同一snapshot」; an adapter whose payload is the intent restated does not need it,
//! and L1 holds a fortiori for one that ignores it.

use gx_core::{Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{McpDelta, McpOp, ToolIntent, MAX_FORWARD_PAYLOAD_BYTES};
use crate::locator;

/// Work out the change an intent asks for, without making it (41 §4, FR-042, FR-046).
///
/// FR-046's verb is 「candidate 化」: what comes back is a candidate, and the gate is what decides
/// whether it becomes a call.
///
/// # Errors
/// [`Error::NotPlannable`] when the intent is for another substrate, when its goal is not this
/// adapter's `{arguments, tool}` grammar, or when the payload would exceed
/// [`MAX_FORWARD_PAYLOAD_BYTES`] (**M4H5-4 採(b)**). [`Error::NotAPosition`] when the locator does not
/// name a scheme-carrying server and a resource -- 「引数が位置でない」 rather than 「適用に失敗した」
/// (**M4H5-5 採(b)**). [`Error::NotDigestible`] when the sequence has no canonical form.
pub fn plan(intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
    if intent.substrate() != &SubstrateKind::Mcp {
        return Err(Error::NotPlannable {
            detail: format!(
                "the intent is for {:?} and this adapter speaks {:?}",
                intent.substrate(),
                SubstrateKind::Mcp
            ),
        });
    }

    // Parsed rather than merely normalised: a locator that is not a position is refused **here**,
    // before a payload exists, so that no delta this adapter minted can carry a spelling `apply` would
    // have to refuse. The refusal is `NotAPosition` and not `NotPlannable` because the two answer
    // different questions -- 「that is not a place」 and 「no change reaches that goal from here」.
    let position = locator::parse(intent.locator())?;
    let call = ToolIntent::decode(&intent.goal().0)?;

    let payload = McpDelta::one(McpOp::call(
        position.locator(),
        call.tool().to_string(),
        call.arguments().to_vec(),
    ))
    .encode()?;
    // **M4H5-4 採(b)**: the bound is on the payload rather than on the arguments, because the payload
    // is what a gate carries and a journal keeps (E-M4-8) -- a bound on the arguments would leave the
    // encoding's own overhead outside the number that was declared.
    if payload.len() > MAX_FORWARD_PAYLOAD_BYTES {
        return Err(Error::NotPlannable {
            detail: format!(
                "the payload of this call would be {} bytes and this adapter plans at most \
                 {MAX_FORWARD_PAYLOAD_BYTES} (M4H5-4(b)); a delta is kept once it is planned \
                 (42 §5, E-M4-8), so the size is a cost the whole pipeline pays",
                payload.len()
            ),
        });
    }
    PlannedDelta::new(SubstrateKind::Mcp, payload)
}
