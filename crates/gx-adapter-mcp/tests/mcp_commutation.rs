// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-052**'s third pair, and the case this adapter's answer costs something.
//!
//! 34 AC-052, verbatim: "Given: two commuting deltas... and two non-commuting deltas... for each of the fs/git/mcp adapters. When: (sem: SEM-gx-adapter-mcp-281)
//! call `adapter.commutation(a,b)`. Then: `Commutes` for the commuting pair, `Conflicts{residual}` for the non-commuting pair (at least one pair per (sem: SEM-gx-adapter-mcp-282)
//! adapter, **6 cases total**)". `req/38` §35 M4H6-9: "AC-052's row is complete at M7's **6/6**". fs supplied (sem: SEM-gx-adapter-mcp-283)
//! two in M4, `gx-adapter-git` two in M7 hand 1 (req/99 §2-3), and this file is the last two.

mod support;

use gx_core::Commutation;
use gx_substrate::SubstrateAdapter;
use support::{
    locator_on, planned, McpFixture, OTHER_SERVER, SERVER, SIBLING, SUBJECT, WRITE_TOOL,
};

/// AC-052, the mcp third: one commuting pair and one conflicting pair.
#[test]
fn ac_052_the_mcp_pair() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();

    let here = locator_on(SERVER, SUBJECT);
    let elsewhere = locator_on(OTHER_SERVER, SUBJECT);

    let commuting = adapter
        .commutation(
            &planned(adapter, &here, WRITE_TOOL, b"one\n"),
            &planned(adapter, &elsewhere, WRITE_TOOL, b"two\n"),
        )
        .expect("two well-formed deltas compare");
    println!("AC052_MCP_COMMUTES {commuting:?}");
    assert_eq!(commuting, Commutation::Commutes);

    let conflicting = adapter
        .commutation(
            &planned(adapter, &here, WRITE_TOOL, b"one\n"),
            &planned(adapter, &here, WRITE_TOOL, b"two\n"),
        )
        .expect("two well-formed deltas compare");
    println!("AC052_MCP_CONFLICTS {conflicting:?}");
    let Commutation::Conflicts { residual } = conflicting else {
        panic!("two calls to one server are not parallel-independent");
    };

    // **M4-14**: the residual names the change that is held back, and it names it by the CID the
    // second delta actually has. A residual pointing at nothing is what E-M4-8's storage row exists
    // against.
    let second = planned(adapter, &here, WRITE_TOOL, b"two\n");
    assert_eq!(
        &residual,
        second.reference(),
        "the residual names something other than the delta that waits"
    );
}

/// 🔴 **The case the design decides, and the one a copy of the fs adapter would get wrong.**
///
/// Two calls to two **different resources on one server** conflict. The crate root argues it: a tool's
/// effects are the server's semantics, and no part of MCP tells a proxy which resources a tool touches,
/// so `Commutes` here would be trusting a map that does not exist -- and `Commutes` is the **fail-open**
/// direction (43 §8 lets both proceed on it).
///
/// An adapter that compared **resources** instead of servers -- which is exactly what `gx-adapter-fs`
/// does, one substrate over -- would answer `Commutes` and pass every other probe in this crate. This is
/// the only one that would go red.
#[test]
fn two_resources_on_one_server_conflict() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();

    let answer = adapter
        .commutation(
            &planned(adapter, &locator_on(SERVER, SUBJECT), WRITE_TOOL, b"one\n"),
            &planned(adapter, &locator_on(SERVER, SIBLING), WRITE_TOOL, b"two\n"),
        )
        .expect("two well-formed deltas compare");
    println!("MCP_TWO_RESOURCES_ONE_SERVER {answer:?}");
    assert!(
        matches!(answer, Commutation::Conflicts { .. }),
        "two calls on one server answered {answer:?}: a proxy has no map from a tool to the \
         resources it touches, and `Commutes` is the fail-open side"
    );
}

/// **L6** in this crate's own terms: the verdict is symmetric, and `commutation(a, a)` is `Conflicts`.
///
/// The harness measures the same law over the fixture's pairs; this measures it over a pair the harness
/// does not supply (two tools on one server), so the symmetry is not a property of one pair.
#[test]
fn the_verdict_is_symmetric_and_a_call_conflicts_with_itself() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let here = locator_on(SERVER, SUBJECT);

    let a = planned(adapter, &here, WRITE_TOOL, b"one\n");
    let b = planned(
        adapter,
        &locator_on(OTHER_SERVER, SIBLING),
        "other.tool",
        b"{}",
    );

    let forward = adapter.commutation(&a, &b).expect("compare");
    let backward = adapter.commutation(&b, &a).expect("compare");
    println!("MCP_L6 forward={forward:?} backward={backward:?}");
    assert_eq!(forward, Commutation::Commutes);
    assert_eq!(backward, Commutation::Commutes);

    let reflexive = adapter.commutation(&a, &a).expect("compare");
    println!("MCP_L6_REFLEXIVE {reflexive:?}");
    assert!(
        matches!(reflexive, Commutation::Conflicts { .. }),
        "M4-25 fixes the reflexive case at `Conflicts`: two applications of one change to one \
         resource are not parallel-independent"
    );
}

/// Nothing in `commutation` touches a server, which is what makes AC-053's "outside the engine pipeline" easy. (sem: SEM-gx-adapter-mcp-284)
#[test]
fn deciding_independence_reaches_no_server() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let here = locator_on(SERVER, SUBJECT);
    let a = planned(adapter, &here, WRITE_TOOL, b"one\n");
    let b = planned(adapter, &here, WRITE_TOOL, b"two\n");

    let before = (fixture.server().calls(), fixture.server().reads());
    let _ = adapter.commutation(&a, &b).expect("compare");
    let after = (fixture.server().calls(), fixture.server().reads());
    println!("MCP_COMMUTATION_IO before={before:?} after={after:?}");
    assert_eq!(
        before, after,
        "`commutation` reached the transport: the footprint is in the locator, so there is nothing \
         a pipeline could have supplied and nothing a server needs to answer"
    );
}

/// A delta from another adapter is a refusal and not an answer.
#[test]
fn a_foreign_delta_is_refused_rather_than_compared() {
    use gx_core::SubstrateKind;
    use gx_substrate::PlannedDelta;

    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let mine = planned(adapter, &locator_on(SERVER, SUBJECT), WRITE_TOOL, b"one\n");
    let theirs = PlannedDelta::new(SubstrateKind::Fs, mine.payload().to_vec())
        .expect("the projection is encodable");

    let refusal = adapter
        .commutation(&mine, &theirs)
        .expect_err("a delta of another substrate is not comparable");
    println!("MCP_FOREIGN_DELTA {}", refusal.kind());
    assert_eq!(refusal.kind(), "ForeignDelta");
}
