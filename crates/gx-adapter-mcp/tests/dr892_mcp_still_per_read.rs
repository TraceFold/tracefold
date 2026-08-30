// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DEFECT-892-1's negative control**: the one adapter the defect never reached.
//!
//! `req/892` found that fs, git, mysql and postgres all reported an empty read-set through
//! `InvertOutcome::from_option`, and that the resulting **signed** receipts carried
//! [`ReadSet::Nothing`] — a member that decides "was this locator read?" with `Some(false)` for
//! every locator. `gx-adapter-mcp` was measured with a real demo receipt and carried
//! [`ReadSet::PerRead`], because its `invert` mints the entry at the point the read answers instead
//! of at a shared constructor that asserted no read had happened.
//!
//! `req/895` repairs the other four **by making them the same shape as this one**. That is exactly
//! the kind of change that repairs three things and breaks the fourth, so this file pins the fourth
//! before the repair rather than after it. It asserted the same values on the tree before `req/895`
//! as on the tree after: that is the claim, and it is the only claim a regression pin can make.

mod support;

use gx_adapter_mcp::McpAdapter;
use gx_substrate::SubstrateAdapter;
use gx_substrate_conformance::Fixture;
use gx_witness::receipt::ReadSet;
use support::{McpFixture, SUBJECT};

/// 🔴 The escrow read the resource through the transport, and says so, entry by entry.
#[test]
fn the_mcp_escrow_still_attests_the_read_it_performed() {
    let fixture = McpFixture::new();
    let adapter: &McpAdapter = fixture.mcp();
    let locator = fixture.locator();

    let pre = adapter.snapshot(&locator).expect("snapshot");
    let delta = adapter.plan(&fixture.intent(), &pre).expect("plan");
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");

    println!(
        "DR892_MCP_READS n={} locators={:?} server_reads={}",
        outcome.reads().len(),
        outcome
            .reads()
            .iter()
            .map(|e| e.locator.clone())
            .collect::<Vec<_>>(),
        fixture.server().reads()
    );
    assert!(
        fixture.server().reads() > 0,
        "if the transport never answered, the assertion below is measuring a fixture and not an \
         escrow"
    );
    assert_eq!(
        outcome.reads().len(),
        1,
        "DEFECT-892-1 must not be repaired by making every adapter agree with the broken three"
    );
    // 🔴 The entry names `position.resource()` and **not** the whole locator, which is what this
    // file got wrong on its first run: `src/invert.rs` says so in the sentence beside the mint
    // ("`locator` is `position.resource()` and not whatever the declared tool called the object")
    // and DR-46-15 is why. The locator carries the endpoint; the object is the resource, and the
    // resource is what `snapshot`, `precondition` and the compare-and-set are quantified over.
    assert_eq!(
        outcome.reads()[0].locator,
        SUBJECT,
        "the entry names the resource the compare-and-set watches (DR-46-15)"
    );
    assert_eq!(
        &outcome.reads()[0].digest,
        pre.digest(),
        "and digests the prior, which is what `snapshot` digested one call earlier"
    );
}

/// 🔴 And the granularity a receipt carries for it is **`PerRead`**, not `Nothing`.
///
/// `ReadSet::from_reads` is the only constructor that chooses (`req/441` §4), so this is the whole
/// distance between the adapter's answer and the member in the signed bytes. It is asserted here
/// rather than inferred from the length above, because `Nothing` is what an empty list produces and
/// "one entry" and "not empty" are the same sentence only while nothing else moves.
#[test]
fn one_entry_becomes_per_read_in_the_signed_bytes() {
    let fixture = McpFixture::new();
    let adapter: &McpAdapter = fixture.mcp();
    let locator = fixture.locator();

    let pre = adapter.snapshot(&locator).expect("snapshot");
    let delta = adapter.plan(&fixture.intent(), &pre).expect("plan");
    let outcome = adapter.invert(&delta, &pre).expect("invert answers");

    let read_set = ReadSet::from_reads(outcome.reads().to_vec()).expect("the entries digest");
    println!(
        "DR892_MCP_GRANULARITY={} attested={} names_subject={:?}",
        read_set.granularity(),
        read_set.is_attested(),
        read_set.names(SUBJECT)
    );
    assert!(
        matches!(read_set, ReadSet::PerRead(_)),
        "the mcp road carried `PerRead` before req/895 and must carry it after"
    );
    assert!(read_set.is_attested());
    assert_eq!(read_set.names(SUBJECT), Some(true));
    assert_eq!(
        read_set.names(&locator),
        Some(false),
        "and answers about the endpoint-qualified spelling honestly: that string is not the object"
    );
    assert_ne!(
        read_set,
        ReadSet::Nothing,
        "the member DEFECT-892-1 was about: a positive claim that no object was read"
    );
}
