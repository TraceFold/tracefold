// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DEFECT-892-1**, applied to the git adapter — req/910 C5 / req/919 W2.
//!
//! `crates/gx-adapter-fs/tests/dr892_read_set_attestation.rs` and
//! `crates/gx-adapter-mcp/tests/dr892_mcp_still_per_read.rs` measured this for their own
//! substrates when `InvertOutcome::from_option` was deleted (commit `7f8b700c`, req/895 §1). The
//! git adapter's `src/adapter.rs` and `src/invert.rs` were repaired in that **same** commit — the
//! stated grounds ("no read that could fail separately from the call itself") were false for git
//! too, since [`gx_adapter_git::repo::tip`] has its own `Error::Unreadable` road — but no runtime
//! test was written for this substrate. req/910 C5 names exactly that gap; this file closes it,
//! observing the fs suite's shape and writing the git-specific form (not copying it: the entry
//! this adapter attests names the **scope** — the branch — and not the full locator, which is the
//! one place git's read differs from fs's).
//!
//! # What this file is entitled to claim
//!
//! Same boundary the fs suite draws: this measures the **adapter's** `invert`, not a receipt from
//! a driven lifecycle. `gx-engine` takes `InvertOutcome::reads()` into
//! `EngineJournalRecord::InverseEscrowed.reads` and from there into `ReadSet::from_reads`, so a
//! non-empty, correctly-scoped list here is what makes `PerRead` reachable on this substrate at
//! all.

mod support;

use gx_adapter_git::repo;
use gx_substrate::SubstrateAdapter;
use support::{planned, GitFixture, BRANCH, ELSEWHERE, GOAL};

/// 🔴 The escrow read the branch tip, so the outcome names it.
#[test]
fn an_escrow_that_read_the_branch_tip_attests_it() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = gx_adapter_git::normalize(&fixture.sandbox().locator_on(BRANCH));

    let pre = adapter.snapshot(&locator).expect("the sandbox holds this entry");
    let delta = planned(adapter, &locator, GOAL);
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");

    println!(
        "DR892_GIT_READS n={} locators={:?}",
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
        "`invert` called `repo::tip` on the branch; an outcome reporting no read is a signed \
         denial of a read that happened (DEFECT-892-1, one substrate over from the fs adapter)"
    );
    let entry = &outcome.reads()[0];
    let position = gx_adapter_git::locator::parse(&locator).expect("a planned delta's locator parses");
    assert_eq!(
        entry.locator,
        position.scope(),
        "the entry names the branch (42 §3.5's scope), not the full <repo>#<ref>:<path> locator: \
         the path is never read on this road"
    );
}

/// 🔴 The negative control: the attested digest is the tip's content **before** the change, not
/// after and not the goal bytes — the same separation the fs suite makes, one substrate over.
///
/// An implementation that satisfied the test above by digesting the goal bytes, or the tip
/// *after* a hypothetical write, would be attesting a state the escrow read did not observe. The
/// three byte strings differ (`BEFORE`/`GOAL`/`ELSEWHERE`), so this separates them.
#[test]
fn the_attested_digest_is_the_prior_tip_and_not_the_goal() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = gx_adapter_git::normalize(&fixture.sandbox().locator_on(BRANCH));

    let pre = adapter.snapshot(&locator).expect("the sandbox holds this entry");
    let delta = planned(adapter, &locator, GOAL);
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");
    let entry = outcome
        .reads()
        .first()
        .expect("the escrow read the branch tip")
        .clone();

    let position = gx_adapter_git::locator::parse(&locator).expect("parses");
    let repository = repo::open(&position).expect("the sandbox repository opens");
    let tip = repo::tip(&repository, &position)
        .expect("tip answers")
        .expect("the sandbox's main branch is not unborn");
    let prior = repo::content_digest(repo::object_text(tip).as_bytes());
    let goal_digest = repo::content_digest(GOAL);
    let elsewhere_digest = repo::content_digest(ELSEWHERE);

    println!(
        "DR892_GIT_PRIOR_NOT_GOAL entry={:?} prior={prior:?} goal={goal_digest:?} \
         elsewhere={elsewhere_digest:?}",
        entry.digest
    );
    assert_ne!(
        prior, goal_digest,
        "if these agree the assertion below is measuring nothing"
    );
    assert_ne!(
        prior, elsewhere_digest,
        "the sandbox's two branches must actually differ, or a wrong-branch read would go \
         undetected"
    );
    assert_eq!(entry.digest, prior);
    assert_ne!(
        entry.digest, goal_digest,
        "an escrow attesting the goal it is about to write has attested a state it never read"
    );
}

/// 🔴 The entry and the CAS's precondition are quantified over the **same branch** — DR-46-15's
/// rule, one substrate over from the fs adapter.
///
/// # Why this is a `locator`/`scope` equality and not a digest equality (measured, not assumed)
///
/// `adapter.rs::precondition` digests the tip's **raw object id bytes**
/// (`repo::content_digest(tip.as_bytes())`), while `invert.rs`'s escrow read digests the tip's
/// **pretty-printed object text** (`repo::content_digest(repo::object_text(tip).as_bytes())`) —
/// confirmed live below: the two digests over the same tip disagree. Unlike the fs adapter, where
/// the escrow's read and `precondition` compute the identical `content_digest` over identical
/// bytes (so equality holds at the digest level), git's two call sites take different byte
/// projections of the same object. That is a genuine asymmetry, distinct from DEFECT-892-1 and out
/// of this ticket's scope (req/919 W2) — noted here rather than silently worked around, and the
/// claim this test actually makes is the one DR-46-15 needs: the escrow's read and the CAS's
/// watched state name the **one branch**, not two.
#[test]
fn the_entry_and_the_precondition_are_about_one_branch() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = gx_adapter_git::normalize(&fixture.sandbox().locator_on(BRANCH));

    let pre = adapter.snapshot(&locator).expect("the sandbox holds this entry");
    let fingerprint = adapter.precondition(&pre).expect("precondition answers");
    let delta = planned(adapter, &locator, GOAL);
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");
    let entry = outcome
        .reads()
        .first()
        .expect("the escrow read the branch tip")
        .clone();

    let position = gx_adapter_git::locator::parse(&locator).expect("parses");
    let repository = repo::open(&position).expect("the sandbox repository opens");
    let tip = repo::tip(&repository, &position)
        .expect("tip answers")
        .expect("not unborn");
    let raw_oid_digest = repo::content_digest(tip.as_bytes());
    let object_text_digest = repo::content_digest(repo::object_text(tip).as_bytes());

    println!(
        "DR892_GIT_ONE_BRANCH entry_locator={} fp_scope={} entry_digest={:?} \
         fp_digest={:?} raw_oid_digest={raw_oid_digest:?} object_text_digest={object_text_digest:?}",
        entry.locator,
        fingerprint.scope(),
        entry.digest,
        fingerprint.digest()
    );
    assert_eq!(
        entry.locator,
        fingerprint.scope(),
        "the escrow's read and the CAS's precondition must be quantified over the same branch, \
         or the receipt would attest one object and the gate would watch another (DR-46-15)"
    );
    // Documented as a passing observation rather than a permanently-red assertion (a test that
    // requires a defect is not a floor -- SS850's rule): git's `precondition` and its escrow read
    // are, by measurement, two different byte projections of the same tip. Nothing in 42 §3.5 / 42
    // §3.10 requires `read_set` entries and `precondition_fingerprint` to share one digest
    // function across adapters, so this is an asymmetry to note (raised to seat in the W2 report),
    // not a DEFECT-892-1-shaped bug -- the locator/scope equality above is the invariant that
    // *is* required, and it holds.
    assert_ne!(
        entry.digest, raw_oid_digest,
        "if these agreed, the note above would be describing a coincidence that stopped being true"
    );
    assert_eq!(
        entry.digest, object_text_digest,
        "the escrow entry's digest is exactly the tip's pretty-printed object text, independently \
         re-derived"
    );
    assert_eq!(
        fingerprint.digest(),
        &raw_oid_digest,
        "the CAS fingerprint's digest is exactly the tip's raw object id, independently re-derived"
    );
}
