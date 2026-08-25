// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-31** — an escalated commit's receipt can be re-issued.
//!
//! `req/453` §10 raised it and `req/470` §4-3 confirmed it from the source: `replay.rs`'s
//! `HumanDecision` arm moved `StateRow.verdict` and `StateRow.state` and left
//! `StateRow.verdict_digest` holding whatever T-4c's `Escalate` had put there. Every commit that
//! walked E-M3-4's road — which is **every commit with no constructible inverse** — therefore had
//! a Σ that named the human's `Admit` beside the escalation's proof digest, and
//! [`Engine::reissue_receipt`] rebuilt that pair into the `VerdictSummary` it digests. The rebuilt
//! payload could not reproduce the leaf, so `gx repair --yes --reissue-receipts` answered
//! `world_moved` about a substrate nobody had touched.
//!
//! This file is the bed. It was written **before** the repair and it failed: the assertion below
//! read `world_moved` where it asks for `filed`. What it measures after the repair is the whole of
//! the claim — the re-issue lands, and the digest it lands with is the one the ledger witnessed.

mod support;

use std::path::Path;
use std::sync::Arc;

use gx_core::{Cid, Timestamp, TransformationId, VerdictKind};
use gx_engine::pipeline::Reissued;
use gx_engine::{
    Engine, EngineJournal, EngineJournalRecord, HumanRuling, InjectedEvidence, Lifecycle,
};

use support::{copy_tree, gate, intent, ruler, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/tmp/dr4631-one-way-only.txt";

/// The ruling T-5 is driven with. One function rather than a literal per test: `cid::compute` is
/// taken over this value, so two copies that drifted apart would be two digests.
fn ruling() -> HumanRuling {
    HumanRuling {
        decision: VerdictKind::Admit,
        reason: "DR-46-31: admitted without an undo guarantee".to_string(),
        actor: ruler(5),
    }
}

/// An engine over `dir` with an adapter that cannot invert — E-M3-4's escalation road.
fn engine_over(dir: &Path) -> (Engine<InjectedEvidence>, Arc<CommitAdapter>) {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let adapter = Arc::new(adapter.without_inverse());
    engine.register_adapter(adapter.clone(), "commit-adapter-1");
    (engine, adapter)
}

/// Submit, plan, verify into `Escalated`, rule on it, canonicalize, commit.
fn escalated_commit(engine: &mut Engine<InjectedEvidence>) -> TransformationId {
    let one = intent(SUBJECT, "after");
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("T-1");
    let id = engine.plan(&one, AT).expect("T-2");
    assert_eq!(
        engine.verify(&id, AT, &key, None).expect("T-4c"),
        Lifecycle::Escalated,
        "E-M3-4: an inverse that cannot be constructed escalates rather than admits"
    );
    let owner_key = gx_witness::KeyPair::from_seed("dr4631-ruler", &[11u8; 32]);
    assert_eq!(
        engine
            .escalation(&id, &ruling(), AT, &owner_key)
            .expect("T-5"),
        Lifecycle::Admitted
    );
    engine.canonicalize(&id, AT, None).expect("T-8");
    assert_eq!(
        engine.commit(&id, AT, &key).expect("T-11"),
        Lifecycle::Committed
    );
    id
}

/// 🔴 The bed. A commit that a person admitted can have its receipt re-issued.
///
/// Three things are read, and the third is what makes the first two more than a green light:
/// the re-issue is `filed`; the re-issued payload carries the ruling's proof digest; and Σ —
/// rebuilt from the journal alone, which is all `gx repair` has — carries the same digest as the
/// receipt the ledger witnessed. A re-issue that landed while Σ still disagreed with the leaf
/// would be a different bug wearing this one's face.
#[test]
fn dr4631_an_escalated_commit_can_have_its_receipt_reissued() {
    let dir = scratch("dr4631_escalated_reissue");
    let (mut engine, _adapter) = engine_over(&dir);
    let id = escalated_commit(&mut engine);

    // The document the ledger witnessed, read before anything is rebuilt.
    let filed = engine
        .receipt(&id)
        .expect("T-11 filed one")
        .payload()
        .expect("it decodes");
    let witnessed = filed
        .verdict
        .as_ref()
        .map(|v| v.proof_digest)
        .expect("the commit receipt names the verdict it was admitted under");

    let sigma = gx_engine::reconstruct(engine.journal().records());
    let row = sigma.state_of(&id).cloned().expect("Σ holds the row");
    println!(
        "DR4631_SIGMA verdict={:?} sigma_digest_is_the_witnessed_one={}",
        row.verdict,
        row.verdict_digest == Some(witnessed)
    );

    let reissued = engine
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue runs");
    println!("DR4631_REISSUE={}", reissued.kind());
    let rebuilt = match reissued {
        Reissued::Filed(receipt) => receipt.payload().expect("the rebuilt payload decodes"),
        other => panic!(
            "DR-46-31: an escalated commit's re-issue answered `{}` — the human's ruling did not \
             reach Σ's `verdict_digest`, so the rebuilt payload cannot digest to the leaf",
            other.kind()
        ),
    };

    assert_eq!(
        rebuilt.verdict.as_ref().map(|v| v.kind),
        Some(VerdictKind::Admit),
        "the re-issue names the verdict the person gave"
    );
    assert_eq!(
        rebuilt.verdict.as_ref().map(|v| v.proof_digest),
        Some(witnessed),
        "and names it with the ruling's own digest, which is what the leaf was taken over"
    );
    assert_eq!(
        row.verdict_digest,
        Some(witnessed),
        "Σ rebuilt from the journal alone carries the digest the receipt was issued under -- \
         without this the re-issue above landed for some other reason"
    );
}

// ---------------------------------------------------------------------------
// The negative controls (`req/473` §1-6): the repair is a **record**, not a repair of Σ
// ---------------------------------------------------------------------------

/// Copy the committed project and rewrite its journal with `HumanDecision.verdict_digest` replaced.
///
/// The records are re-appended rather than patched in place, so the copy's chain is a chain this
/// build wrote — anything the re-issue then answers is a fact about the digest and not about a
/// damaged file. The blobs, the ledger and the recorded head come across untouched with the tree,
/// which is what makes the leaf the same leaf.
fn journal_rewritten_with(from: &Path, to: &Path, digest: Option<Cid>) {
    copy_tree(from, to);
    let path = to.join("journal.bin");
    let records: Vec<EngineJournalRecord> = EngineJournal::open(&path)
        .expect("the copied journal opens")
        .records()
        .to_vec();
    std::fs::remove_file(&path).expect("the journal is rewritten rather than appended to");
    let mut journal = EngineJournal::open(&path).expect("a fresh journal opens");
    let mut rewritten = 0usize;
    for record in records {
        let record = match record {
            EngineJournalRecord::HumanDecision {
                transformation,
                kind,
                reason,
                actor,
                at,
                ..
            } => {
                rewritten += 1;
                EngineJournalRecord::HumanDecision {
                    transformation,
                    kind,
                    reason,
                    actor,
                    verdict_digest: digest,
                    at,
                }
            }
            other => other,
        };
        journal.append(record).expect("the record is re-appended");
    }
    println!("DR4631_REWROTE human_decisions={rewritten} digest={digest:?}");
    assert_eq!(rewritten, 1, "the fixture has exactly one human ruling");
}

/// Re-open a project and ask for the re-issue, over an adapter holding the same world.
fn reissue_over(dir: &Path, world: &str) -> String {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the project re-opens");
    let (adapter, _counts, _world) = CommitAdapter::new(world);
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");
    let sigma = gx_engine::reconstruct(engine.journal().records());
    let id = sigma
        .transformations()
        .first()
        .expect("one transformation")
        .transformation;
    let reissued = engine
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue runs");
    reissued.kind().to_string()
}

/// 🔴 **Negative control (a)** — a journal written before this field still degrades, honestly.
///
/// The repair must not be a repair of *old* projects: a `HumanDecision` with no digest is a record
/// from a binary that never had one, and the only truthful thing to say about it is that the
/// ruling's proof was never written down. `replay.rs` leaves T-4c's escalation digest in the seat,
/// the rebuild names it, and the re-issue answers `world_moved` exactly as it did before DR-46-31.
///
/// Without this control the positive test above is satisfied by a replay that *invented* a digest
/// for every escalated row — which would make Σ agree with a leaf it had not read.
#[test]
fn dr4631_a_journal_written_before_this_field_still_refuses_the_reissue() {
    let dir = scratch("dr4631_control_old_journal");
    let (mut engine, _adapter) = engine_over(&dir);
    escalated_commit(&mut engine);
    drop(engine);

    let old = scratch("dr4631_control_old_journal_copy");
    journal_rewritten_with(&dir, &old, None);
    let answer = reissue_over(&old, "after");
    println!("DR4631_CONTROL_OLD_JOURNAL={answer}");
    assert_eq!(
        answer, "world_moved",
        "a pre-DR-46-31 record must replay to the old blocked degradation -- if this is `filed`, \
         replay is deriving the digest rather than reading it"
    );
}

/// 🔴 The control's own control — the rewrite road files when the digest is right.
///
/// Both refusals above are `world_moved`, and `world_moved` is also what a project would answer if
/// the rewrite had damaged something that has nothing to do with the verdict. This test runs the
/// **same** rewrite with the digest the receipt was issued under and requires `filed`, so the two
/// refusals are facts about the digest rather than about the road that produced them.
#[test]
fn dr4631_the_rewrite_road_itself_files_when_the_digest_is_the_witnessed_one() {
    let dir = scratch("dr4631_control_rewrite_road");
    let (mut engine, _adapter) = engine_over(&dir);
    let id = escalated_commit(&mut engine);
    let witnessed = engine
        .receipt(&id)
        .expect("T-11 filed one")
        .payload()
        .expect("it decodes")
        .verdict
        .as_ref()
        .map(|v| v.proof_digest)
        .expect("the commit receipt names its verdict");
    drop(engine);

    let same = scratch("dr4631_control_rewrite_road_copy");
    journal_rewritten_with(&dir, &same, Some(witnessed));
    let answer = reissue_over(&same, "after");
    println!("DR4631_CONTROL_REWRITE_ROAD={answer}");
    assert_eq!(
        answer, "filed",
        "the rewrite road refuses even a correct journal, so the two refusals above are not \
         about the digest"
    );
}

/// 🔴 **Negative control (b)** — a record carrying the wrong digest is refused.
///
/// The same rewrite, with a digest that is neither the escalation's nor the ruling's. If the
/// re-issue filed this, the leaf comparison would not be reading the verdict at all and the
/// positive test would be measuring a road that accepts anything.
#[test]
fn dr4631_a_forged_ruling_digest_does_not_reproduce_the_leaf() {
    let dir = scratch("dr4631_control_forged");
    let (mut engine, _adapter) = engine_over(&dir);
    escalated_commit(&mut engine);
    drop(engine);

    let forged = scratch("dr4631_control_forged_copy");
    journal_rewritten_with(&dir, &forged, Some(Cid([0x5au8; 32])));
    let answer = reissue_over(&forged, "after");
    println!("DR4631_CONTROL_FORGED={answer}");
    assert_eq!(
        answer, "world_moved",
        "a forged proof digest reproduced the leaf, so the digest is not reaching the payload"
    );
}
