//! `plan`, with no filesystem in it (**E-M4-29**).
//!
//! 41 §4 calls `plan` 「純関数・副作用なし」 and §30 M4H2-3 採(b) read that for the trait as 「(intent,
//! pre) の対に対する決定性+substrate への**書き込み** 0」, deliberately leaving reads open so that M7's
//! git adapter may consult an object store. Then it added a clause for this adapter alone:
//!
//! > 「**ただし fs adapter v0.1 の `plan` は I/O 0 が成立する**(単一 file 全置換では target digest は
//! > goal bytes から導出可能)ので、**手4 の DoD に「fs の plan は std::fs を呼ばない」機械検査**を置く
//! > (契約より強い実装を機械で固定)」
//!
//! # Why a module rather than a function in `adapter.rs`
//!
//! Because 「calls no filesystem operation」 is a claim about source, and the cheapest honest way to
//! measure it is a file that never names one. `adapter.rs` opens files -- `snapshot` and
//! `precondition` must -- so a scan of that file could only ever be a scan of a function body.
//! `tests/plan_purity.rs` reads both: this module for filesystem tokens, and the trait method's body
//! for the single line that delegates here.
//!
//! The `pre` argument is unused, and that is the finding rather than an oversight: a whole-file
//! replacement is a function of the goal alone. **E-M4-4** put `pre` in the signature because 43 T-2
//! quantifies determinism over 「同一snapshot」 and a future adapter (or this one, once v0.2 plans a
//! patch instead of a replacement) needs it; L1 in the harness is the law, and it holds a fortiori
//! for an adapter that ignores the argument.

use gx_core::{Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{FsDelta, FsOp, MAX_FORWARD_PAYLOAD_BYTES};
use crate::locator;

/// Work out the change an intent asks for, without making it (41 §4, FR-042).
///
/// The delta is the one-operation sequence of **M4-13 採(a)**: replace the whole file at the
/// intent's normalised locator with the intent's goal bytes. `goal` is carried opaquely by
/// [`gx_core::GoalBytes`] (**E-M4-2**), and what this adapter's grammar says about those bytes is
/// 「this is the file's new content」 -- which is the whole of the interpretation P-6 reserves to an
/// adapter.
///
/// # Errors
/// [`Error::NotPlannable`] when the intent is for another substrate, or names a position this
/// adapter cannot accept (empty, or relative: **ASM-69-3**), or asks for a change larger than
/// [`MAX_FORWARD_PAYLOAD_BYTES`] (**M4H5-4 採(b)**), or when the sequence has no canonical form. All
/// four are 「no delta plans this intent against this snapshot」 rather than failures of the world,
/// which is why none of them is [`Error::Unreadable`].
pub fn plan(intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
    if intent.substrate() != &SubstrateKind::Fs {
        return Err(Error::NotPlannable {
            detail: format!(
                "the intent is for {:?} and this adapter speaks {:?}",
                intent.substrate(),
                SubstrateKind::Fs
            ),
        });
    }

    let target = locator::normalize(intent.locator());
    if !locator::is_absolute(&target) {
        return Err(Error::NotPlannable {
            detail: format!(
                "{:?} normalises to {target:?}, which is not a position from the root; v0.1 names \
                 positions absolutely (ASM-69-3)",
                intent.locator()
            ),
        });
    }

    let payload = FsDelta::one(FsOp::write(target, intent.goal().0.clone())).encode()?;
    // **M4H5-4 採(b)**: the bound is on the payload rather than on the goal, because the payload is
    // what a gate carries and a journal keeps (E-M4-8) -- a bound on the content would leave the
    // encoding's own overhead outside the number that was declared, which is the reading hand 5 gave
    // the escrow ceiling for the same reason.
    if payload.len() > MAX_FORWARD_PAYLOAD_BYTES {
        return Err(Error::NotPlannable {
            detail: format!(
                "the payload of this change would be {} bytes and this adapter plans at most \
                 {MAX_FORWARD_PAYLOAD_BYTES} (M4H5-4(b)); a delta is kept once it is planned (42 §5, \
                 E-M4-8), so the size is a cost the whole pipeline pays",
                payload.len()
            ),
        });
    }
    PlannedDelta::new(SubstrateKind::Fs, payload)
}
