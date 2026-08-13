//! `invert`: the delta that puts a position back, built while the old bytes are still there.
//!
//! Spec: 41 §4 for the method and for DR-1(a), 42 §5 for why the escrow carries a body, 34 AC-048
//! and AC-049 for what is measured. The rulings are **E-M4-30** (the escrow is constructed **before**
//! `apply`), **E-M4-3** (the round trip is quantified at the one `pre` handed in), **M4-21 採(a)**
//! (the payload ceiling and its `Ok(None)`) and **E-M4-5** (an engine folds `Some`/`None` into
//! `GateInput.invert_available` at verify time).
//!
//! # 🔴 Why this module reads the substrate, and why the order is not negotiable
//!
//! **E-M4-30** (req/38 §31 M4H3-1 採(a)) 逐語:
//!
//! > 「escrow(invert)は apply の前(43 T-10b が state machine の正本)。根拠は物理: 上書き/削除の逆は旧内容
//! > 本体を運ぶ(42 §5 の escrow 必須理由)ので、**invert は pre が観測可能な時点でしか構成できない**——
//! > T-10b 順が唯一 constructible であり、逐語順を満たせるのは自前履歴を持つ adapter だけ」
//!
//! This adapter keeps no history. The inverse of a whole-file replacement is 「put back what is here」,
//! and 「what is here」 exists only until `apply` runs. So [`invert`] reads the position, and the caller
//! is required by 43 T-10b to call it first. That is also why 42 §5 wants the **body**: 「digest-only
//! では実際のundoが物理的に不可能なため」.
//!
//! # The inverse is a function of the state, not of the forward operation's shape
//!
//! Whether the delta writes or removes, the inverse is the same question -- what does this position
//! hold at the escrow point? -- and the answer has two arms:
//!
//! | at the escrow point | inverse | the case it undoes |
//! |---|---|---|
//! | a file with content *c* | write *c* back | 変更 and 削除 |
//! | nothing | remove | 作成 |
//!
//! The second arm also gives the right answer to a removal of something that is already gone: the
//! forward delta is a no-op and its inverse is a removal, which is a no-op too. Nothing special-cases
//! it, because nothing has to.
//!
//! # The `pre` argument, and the two things it is
//!
//! `pre` is an [`ObjectSnapshot`]: 42 §3.3 gives it a digest and not the bytes, so it cannot be the
//! source of the body an inverse carries. What it is instead:
//!
//! 1. **the Given the law is quantified at** -- 「`invert` に渡した `pre` の 1 点」 (**E-M4-3**), which
//!    the harness writes into the property's message and `tests/ac_049.rs` into each case's;
//! 2. **a statement of which object this question is about**. A `pre` naming another position is a
//!    mis-wired call and is refused as one (**E-M4-32**) -- see below.
//!
//! It is deliberately **not** compared with the position's current digest. **E-M4-5** settled that a
//! prediction going stale between verify and commit is folded by the CAS check into an `Abort`
//! (「予測が外れた変換は Fingerprint 不一致で apply に到達しない」), so an adapter that refused here would
//! be turning a state machine transition into an error at a layer that does not own the decision.

use gx_core::{ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::delta::{FsDelta, FsOp, MAX_INVERSE_PAYLOAD_BYTES};
use crate::locator;

/// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
///
/// # The one reason for `Ok(None)` (**E-M4-32** narrowed it to this)
///
/// 41 §4 separates 「the question itself cannot be answered」 (`Err`) from 「the answer is no inverse」
/// (`Ok(None)`), and **E-M3-4** makes the second an escalation to a human rather than a refusal to
/// act. §33 fixed which facts may take the second form: 「**`Ok(None)` は「同一 object の正当な構成
/// 不能」(上限超過・旧内容破棄済み)に限定**」. For this adapter, in v0.1, that leaves exactly one:
///
/// * **The escrow ceiling** (**M4-21 採(a)**, 「AC-048 の None の実在する第 1 の理由」). An inverse
///   carries the whole old file, so an undoable change over a large file costs its size twice. Over
///   [`MAX_INVERSE_PAYLOAD_BYTES`] this adapter declines, and the change is escalated instead of
///   being quietly made unundoable.
///
/// Hand 5 had a second `Ok(None)` here -- a `pre` naming another position -- and raised the reading
/// against itself (req/74 §2 M4H5-1). **E-M4-32** took the other case, and the argument is
/// **E-M4-27**'s: a delta and a snapshot of two different objects is a wiring bug in whoever
/// assembled the call, and answering `Ok(None)` would send it down the escalation path wearing the
/// face of a legitimate business condition. An operator asked 「this change cannot be undone, proceed?」
/// would be answering the wrong question about a call that should never have been made.
///
/// # Errors
/// [`Error::LocatorMismatch`] when `pre` is a snapshot of another object (**E-M4-32**),
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// [`Error::Unimplemented`] for a sequence v0.1 does not run, and [`Error::Unreadable`] when the
/// position exists and will not answer.
pub(crate) fn invert(delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>> {
    if delta.substrate() != &SubstrateKind::Fs {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Fs,
            got: delta.substrate().clone(),
        });
    }
    let decoded = FsDelta::decode(delta.payload())?;
    let op = decoded
        .ops()
        .first()
        .expect("decode refuses the empty sequence");
    let position = op.position()?;

    let named = locator::normalize(pre.locator());
    if named != position {
        return Err(Error::LocatorMismatch {
            expected: position,
            got: named,
        });
    }

    let restore = match read_if_present(&position)? {
        Some(old) => FsOp::write(position, old),
        None => FsOp::remove(position),
    };
    let payload = FsDelta::one(restore).encode()?;
    if payload.len() > MAX_INVERSE_PAYLOAD_BYTES {
        return Ok(None);
    }
    PlannedDelta::new(SubstrateKind::Fs, payload).map(Some)
}

/// The bytes at a position, or `None` when there are none.
///
/// The absence is a state and not a failure: it is what a creation is planned against, and its
/// inverse is a removal.
fn read_if_present(position: &str) -> Result<Option<Vec<u8>>> {
    match std::fs::read(position) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Unreadable {
            locator: position.to_string(),
            detail: e.to_string(),
        }),
    }
}
