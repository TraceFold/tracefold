// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DEFECT-892-1** — an escrow that read the position has to say so.
//!
//! `req/892` measured the consequence on a real lifecycle: a signed `CommitReceipt` for an fs change
//! carried `read_set = Nothing`, and `gx-witness/src/receipt.rs`'s own documentation says that
//! member "answers [`ReadSet::names`] with `Some(false)` about **every** locator … which is a
//! *stronger* answer than G3 gives". So the receipt was not silent about the read; it **denied** it,
//! in the same turn as the read, under a signature.
//!
//! The preimage was `gx_substrate::InvertOutcome::from_option`, which fixed `Vec::new()` on both
//! arms for the fs, git, mysql and postgres adapters, on the stated grounds that "there is no read
//! that could fail separately from the call itself". `crates/gx-adapter-fs/src/invert.rs`'s
//! `read_if_present` calls `std::fs::read` and returns [`gx_substrate::Error::Unreadable`], which is
//! that sentence's counterexample sitting one file away from it. `req/895` §1 is the ledger.
//!
//! # What this file is entitled to claim
//!
//! It measures the **adapter**, not a receipt: `gx-engine` takes `InvertOutcome::reads()` straight
//! into `EngineJournalRecord::InverseEscrowed.reads` and from there into `ReadSet::from_reads`, so
//! a non-empty list here is what makes `PerRead` reachable at all on this substrate. The receipt-level
//! statement belongs to a lane that can drive a whole lifecycle; this one fixes the producer.
//!
//! Both tests are **negative controls in the strict sense**: they fail on the tree that was shipped
//! before `req/895`, and the second one fails for a different reason from the first (an entry that
//! digested the *successor* would satisfy the first test and not the second).

mod support;

use gx_adapter_fs::FsAdapter;
use gx_canon::cid::{self, Domain};
use gx_substrate::SubstrateAdapter;
use support::{planned, snapshot_of, Sandbox, BEFORE, GOAL};

/// The digest an entry is supposed to carry: the same function `snapshot`/`precondition` use.
///
/// Spelled out here rather than imported, because `gx_adapter_fs::adapter::content_digest` is
/// `pub(crate)`: a test that could only compare the adapter's output with the adapter's own helper
/// would be checking that a function equals itself. 41 §6 fixes the road (`gx-canon`), so the road
/// is what this rebuilds.
fn digest_of(content: &[u8]) -> gx_core::Cid {
    cid::mint(Domain::Leaf, &[content])
}

/// 🔴 The escrow read the position, so the outcome names it.
#[test]
fn an_escrow_that_read_the_position_attests_it() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    sandbox.write("target", BEFORE);
    let locator = sandbox.locator("target");

    let pre = snapshot_of(&adapter, &locator);
    let delta = planned(&adapter, &locator, GOAL);
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");

    println!(
        "DR892_FS_READS n={} locators={:?}",
        outcome.reads().len(),
        outcome
            .reads()
            .iter()
            .map(|e| e.locator.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        outcome.reads().len(),
        1,
        "`invert` called `std::fs::read` on the position; an outcome reporting no read is a signed \
         denial of a read that happened (DEFECT-892-1)"
    );
    let entry = &outcome.reads()[0];
    assert_eq!(
        entry.locator,
        gx_adapter_fs::normalize(&locator),
        "the entry names the object in the adapter's own normalised spelling"
    );
    assert_eq!(
        entry.digest,
        digest_of(BEFORE),
        "and digests what the read answered"
    );
}

/// 🔴 The negative control: the attested digest is the **prior**, not the successor.
///
/// An implementation that satisfied the test above by digesting the goal bytes — or the delta's
/// payload, which is the value most easily to hand at that point in the function — would be
/// attesting a state the escrow never read. The two byte strings differ, so this separates them.
#[test]
fn the_attested_digest_is_the_prior_and_not_the_successor() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    sandbox.write("target", BEFORE);
    let locator = sandbox.locator("target");

    let pre = snapshot_of(&adapter, &locator);
    let delta = planned(&adapter, &locator, GOAL);
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");
    let entry = outcome
        .reads()
        .first()
        .expect("the escrow read the position")
        .clone();

    let prior = digest_of(BEFORE);
    let successor = digest_of(GOAL);
    println!(
        "DR892_FS_PRIOR_NOT_SUCCESSOR entry={:?} prior={prior:?} successor={successor:?}",
        entry.digest
    );
    assert_ne!(
        prior, successor,
        "if these agree the assertion below is measuring nothing"
    );
    assert_eq!(entry.digest, prior);
    assert_ne!(
        entry.digest, successor,
        "an escrow attesting the state it is about to write has attested a state it never read"
    );
}

/// 🔴 The entry is the **prior state of the object the CAS watches**, so it agrees with `pre`.
///
/// `pre` is an [`gx_core::ObjectSnapshot`] taken by `snapshot`, whose digest is the same
/// `content_digest` of the same bytes. If the escrow's read and the compare-and-set were quantified
/// over different objects the receipt would attest one and the gate would watch the other — which is
/// `DR-46-15`'s failure one substrate over, and the reason this is a third assertion rather than a
/// clause in the first.
#[test]
fn the_entry_and_the_precondition_are_about_one_object() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    sandbox.write("target", BEFORE);
    let locator = sandbox.locator("target");

    let pre = snapshot_of(&adapter, &locator);
    let fingerprint = adapter.precondition(&pre).expect("precondition answers");
    let delta = planned(&adapter, &locator, GOAL);
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");
    let entry = outcome
        .reads()
        .first()
        .expect("the escrow read the position")
        .clone();

    println!(
        "DR892_FS_ONE_OBJECT entry_locator={} pre_locator={} fp_scope={}",
        entry.locator,
        pre.locator(),
        fingerprint.scope()
    );
    assert_eq!(entry.locator, pre.locator());
    assert_eq!(&entry.digest, pre.digest());
}
