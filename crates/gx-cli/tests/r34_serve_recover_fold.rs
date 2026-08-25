// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R34 / `req/449` H-02** — `gx serve`'s start-up fold, at `gx repair`'s granularity, and
//! the sentence 43 §7-3c's road owes an operator.
//!
//! # What the thirty-third audit measured, and why it needed its own file
//!
//! `req/449` §3 drove `Engine::recover` over an adapter that performs the change and then answers
//! an error (`CommitAdapter::failing_after_the_effect`, the shape `req/372` M-01 built because it
//! is *"the commonest real failure, where the call worked and the answer was lost coming back"*).
//! The row came back `path=ApplyWasAnnounced state=Aborted(ApplyFailed)`, the world had moved from
//! `"SOMEBODY ELSE WROTE THIS"` to `"two\n"`, no refusal was printed — and `gx serve`'s start-up
//! line counted it under **`resumed`**, beside the rows whose commit had completed.
//!
//! The audit could only measure that by **copying serve's fold into the probe**
//! (`a33_recover_road2.rs::what_serve_calls_it`), because the fold was four `match` arms inside a
//! 400-line function that needs a listening socket to reach. A copy is a second source of truth:
//! it says what serve *did* on the day it was written and nothing about what serve does now. So
//! R34 gave the fold a name — [`gx_cli::serve::RecoverFold`] — and this file drives **that**, over
//! all seven of [`gx_engine::pipeline::RECOVERY_PATHS`], with the two states an
//! `ApplyWasAnnounced` row can end in.
//!
//! # The denominator, stated
//!
//! Seven roads and one of them (`ApplyWasAnnounced`) has two terminal states worth telling apart,
//! so **eight rows** is the whole space this fold sees. All eight are built here. What is *not*
//! covered: whether `Engine::recover` puts a row on the road this file hands it — that is
//! `crash_recovery.rs`'s and `a33_recover_road2.rs`'s business, and `req/449` §3 measured it.

use gx_cli::serve::{announced_road_note, RecoverFold};
use gx_core::{AbortReason, Cid, TransformationId};
use gx_engine::pipeline::{Lifecycle, Recovered, RecoveryPath, RECOVERY_PATHS};

fn row(seed: u8, path: RecoveryPath, state: Lifecycle) -> Recovered {
    Recovered {
        transformation: TransformationId(Cid([seed; 32])),
        path,
        state,
        ledger_seq: None,
        appended: None,
        payload_matched: None,
        receipt: None,
        refusal: None,
    }
}

/// The eight rows: one per road, and `ApplyWasAnnounced` twice — once closed and once aborted.
fn every_road() -> Vec<Recovered> {
    vec![
        row(1, RecoveryPath::Terminal, Lifecycle::Committed),
        row(2, RecoveryPath::LedgerHeldTheCommit, Lifecycle::Committed),
        row(3, RecoveryPath::ApplyWasAnnounced, Lifecycle::Committed),
        row(
            4,
            RecoveryPath::ApplyWasAnnounced,
            Lifecycle::Aborted(AbortReason::ApplyFailed),
        ),
        row(
            5,
            RecoveryPath::NothingWasApplied,
            Lifecycle::Aborted(AbortReason::InternalError),
        ),
        row(
            6,
            RecoveryPath::ClosedFromFiledReceipt,
            Lifecycle::Committed,
        ),
        row(7, RecoveryPath::ClosedFromLedgerLeaf, Lifecycle::Committed),
        row(8, RecoveryPath::NotResumed, Lifecycle::Committing),
    ]
}

/// 🔴 **`req/449` H-02, the counter** — `ApplyWasAnnounced` is its own number, and the number it
/// used to be folded into is unchanged.
///
/// `gx repair --json` has published `apply_was_announced` apart since `req/329` M-01 / R27, and
/// `r27_reentrant_abort.rs:614` pins it with the reason: *"an operator deciding whether to trust a
/// recovered project cannot tell the two apart"*. Before R34 `gx serve` had no such field. The
/// assertion is both halves at once: the sub-counts exist **and** `resumed` still sums them, so
/// nothing that read the old start-up line reads it differently.
#[test]
fn r34_the_start_up_fold_separates_the_road_that_writes() {
    let fold = RecoverFold::of(&every_road());
    println!("R34_FOLD {fold:?}");

    assert_eq!(
        fold.apply_was_announced, 2,
        "🔴 `req/449` H-02: 43 §7-3c's road — the one road of the four `resumed` sums on which \
         this start-up wrote to a substrate — must be a number an operator can find. It was not \
         one before R34"
    );
    assert_eq!(
        fold.announced_and_aborted, 1,
        "🔴 `req/449` H-02: the row that ended `Aborted(ApplyFailed)` with `Rollback::NotAttempted` \
         was announced as one of the `resumed`. It is still counted there — it walked that road — \
         and it is now also counted as what it is"
    );
    assert_eq!(fold.ledger_held_the_commit, 1);
    assert_eq!(fold.closed_from_receipt, 1);
    assert_eq!(fold.closed_from_leaf, 1);
    assert_eq!(
        fold.resumed,
        fold.apply_was_announced
            + fold.ledger_held_the_commit
            + fold.closed_from_receipt
            + fold.closed_from_leaf,
        "`resumed` is unchanged: still the sum of the four roads, so a monitor reading the old \
         field reads the same number"
    );
    assert_eq!(fold.resumed, 5);
    assert_eq!(fold.terminal, 1);
    assert_eq!(fold.nothing_applied, 1);
    assert_eq!(fold.refused, 1);
    assert_eq!(
        fold.terminal + fold.resumed + fold.nothing_applied + fold.refused,
        8,
        "every row lands in exactly one of the four totals"
    );
}

/// The denominator is the type's, not this file's opinion of it: seven roads are declared and
/// eight rows are driven, so a road added without a row here fails **this** assertion before it
/// can land silently in a total.
#[test]
fn r34_the_fold_is_driven_over_every_declared_road() {
    let rows = every_road();
    assert_eq!(
        RECOVERY_PATHS.len(),
        7,
        "instrument: `RecoveryPath` declares seven roads"
    );
    let mut seen: Vec<&'static str> = rows.iter().map(|r| r.path.kind()).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen.len(),
        RECOVERY_PATHS.len(),
        "R34: {} of the {} declared roads are driven here; the un-driven ones are {:?}",
        seen.len(),
        RECOVERY_PATHS.len(),
        RECOVERY_PATHS
            .iter()
            .filter(|p| !seen.contains(p))
            .collect::<Vec<_>>()
    );
    println!(
        "R34_DENOM roads={} rows={}",
        RECOVERY_PATHS.len(),
        rows.len()
    );
}

/// 🔴 **`req/449` H-01 + H-02, the sentence** — the road that writes says so, and the row that
/// aborted on it says *that*.
///
/// `req/449` §2-2 measured `A33_REPAIR_STDERR bed=noleaf_third_party` **empty** and
/// `A33_SERVE_NOTES count=1` where the single note named the road and not what happened on it.
/// Both halves are asserted from the sentence's own text, not from a flag: what an operator reads
/// is the artifact.
#[test]
fn r34_the_announced_road_says_what_it_did_and_what_it_did_not_compare() {
    let rows = every_road();

    let mut spoken = 0usize;
    for r in &rows {
        if let Some(note) = announced_road_note(r) {
            spoken += 1;
            println!("R34_NOTE path={} note={note}", r.path.kind());
        }
    }
    assert_eq!(
        spoken, 2,
        "🔴 `req/449` H-01/H-02: exactly the two rows on 43 §7-3c's road get a sentence, because \
         that is the road on which a start-up writes. The other six wrote nothing and claim nothing"
    );

    let closed = announced_road_note(&rows[2]).expect("the §7-3c row that closed gets a sentence");
    assert!(
        closed.contains("applying its delta"),
        "the road that writes has to say that it wrote: {closed}"
    );
    assert!(
        closed.contains("was **not** checked"),
        "🔴 `req/449` H-01: the finding is the silence, not the write. The sentence has to name \
         what it did not compare: {closed}"
    );
    assert!(
        closed.contains("written over it and cannot tell you so"),
        "🔴 `req/449` §2-5 answer 2: gx cannot detect the third party on this road, and saying so \
         is the repair R34 is entitled to make: {closed}"
    );

    let aborted =
        announced_road_note(&rows[3]).expect("the §7-3c row that aborted gets a sentence");
    assert!(
        aborted.contains("did **not** finish"),
        "🔴 `req/449` H-02: a row that ended `Aborted` must not be described as carried forward: \
         {aborted}"
    );
    assert!(
        aborted.contains("roll-back **not attempted**"),
        "the compensation did not run (`NotAttemptedBecause::RecoveredWithoutRebuilding`), which \
         is the fact that decides what an operator does next: {aborted}"
    );
    assert!(
        aborted.contains("possibly changed and certainly \\\n             unrecorded")
            || aborted.contains("possibly changed and certainly unrecorded"),
        "🔴 `req/449` §3: 'the adapter refused' and 'the world did not move' are not the same \
         statement — `failing_after_the_effect` is the commonest real shape — so the sentence \
         must not claim the second: {aborted}"
    );

    assert!(
        announced_road_note(&rows[1]).is_none(),
        "43 §7-3b reads and does not apply; it owes no such sentence"
    );
}

/// 🔴 **`req/449` M-01** — the refusal's disjunction is exhaustive over the causes this build can
/// reach, and the fourth member is the one the audit drove.
///
/// R33 turned an assertion ("the difference is the signing key") into a list of three, which was
/// the right move and left the next question standing: **is the list complete?** `req/449` §4-1
/// drove a `StampingAdapter` — one that digests what it was *sent* on write and what the substrate
/// *holds* on read, which is every server that normalises, re-encodes or stamps — on an untouched
/// project, with the key that signed the commit, and reached this refusal with all three listed
/// causes false. The assertion is on the shipped text because the text is the artifact an operator
/// reads.
///
/// It also pins the cost sentence. Re-running reads the same world and rebuilds the same payload,
/// so no verb closes the row; `req/38` §238 / spec 43 §7.11 declared that shape for a **lost key**,
/// and it is reachable here with the key in hand and nothing damaged. Whether to add a verb that
/// overrides the comparison is a DR, and a refusal that does not say the row is stuck sends an
/// operator to re-run a command that cannot succeed.
#[test]
fn r34_the_rebuild_disagreement_lists_a_fourth_cause() {
    let text = gx_engine::pipeline::not_resumed::RECOVERY_REBUILD_DISAGREES;
    for member in ["(1)", "(2)", "(3)", "(4)"] {
        assert!(
            text.contains(member),
            "🔴 `req/449` M-01: the disjunction must carry {member}. A three-member list was one \
             an operator could check to the end and find every item clean: {text}"
        );
    }
    assert!(
        !text.contains("(5)"),
        "instrument: four members are asserted; a fifth means this assertion's denominator moved \
         and was not re-read"
    );
    assert!(
        text.contains("read and its apply are not the same computation"),
        "🔴 `req/449` M-01: the fourth cause is the adapter's two answers being two computations, \
         not the world moving and not the key: {text}"
    );
    assert!(
        text.contains("permanent under re-running"),
        "🔴 `req/449` M-01, the cost: under that cause every verb lands here again, so an operator \
         told only 'the reading and the ledger disagree' re-runs a command that cannot succeed: \
         {text}"
    );
    println!("R34_DISJUNCTION members=4 len={}", text.len());
}
