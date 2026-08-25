// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AUDIT LANE (AC-P3-14) — A-3: two agents' transformations on one MCP server's footprint.**
//!
//! `req/120` §5.1: A-3 "has never been driven, even once" (sem: SEM-gx-cli-1601). `req/119` §5 asks for "2 agents simultaneously on one server",
//! and `gx-adapter-mcp`'s own crate root states the mechanism plainly: "footprint = **server**" —
//! "two changes to two different resources on one MCP server conflict, and one of them waits …
//! **The cost is not softened**". This file drives that claim through the real engine and finds it
//! **does not hold** for the case req/119 §5 actually describes (two different resources); it holds
//! only when the two transformations share the same resource. §2 of this suite is the evidence for
//! that finding, kept beside §1's control so the discrepancy is measured against a working case
//! rather than asserted from a single red test.
//!
//! # Why this is not two real OS processes racing on one `.gx/`
//!
//! `gx-cli`'s journal has **no cross-process lock** (grepped: no `flock`/`fs2`/advisory lock anywhere
//! in `gx-cli` or `gx-engine`). Two real `gx wrap` processes against one `.gx/` would therefore
//! measure a file-I/O race, not 43 §8's `Conflicts` semantics. Driving the two logical
//! transformations inside one process against a real server is the safe, faithful form of "two
//! agents, one server" this workspace's own concurrency model supports.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

#[path = "support/audit_p3_support.rs"]
mod audit_p3_support;

use std::sync::Arc;

use audit_p3_support::{intent_for, journal_path, key, spawn_probe};
use gx_core::{FailPosture, Timestamp};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use gx_mcp_wire::WireTransport;

const AT: Timestamp = Timestamp(1_755_100_000_000_000_000);

fn engine_with_catalogue(
    name: &str,
    probe: &audit_p3_support::AuditProbe,
) -> Engine<InjectedEvidence> {
    let gate = gx_gate::Gate::with_policies(
        gx_gate::packs::mcp_pack().expect("the shipped mcp pack parses"),
    );
    let mut engine = Engine::open(journal_path(name), gate, InjectedEvidence::none())
        .expect("the engine opens")
        .with_posture(FailPosture::FailClosed);
    // E-M3-4: a delta whose `invert` answers `None` is escalated rather than admitted, true of the
    // *forward* call as well as of an undo. Without a declared restore every verify below comes back
    // `Escalated` -- measured directly in this file's first draft -- which would test E-M3-4 instead
    // of A-3.
    let catalogue = gx_adapter_mcp::Catalogue::new().with_restore("notes.write", "notes.restore");
    engine.register_adapter(
        Arc::new(
            gx_adapter_mcp::McpAdapter::new(Arc::new(WireTransport::new(
                probe.client.clone(),
                probe.endpoint.clone(),
            )))
            .with_catalogue(catalogue),
        ),
        name,
    );
    engine
}

/// §1 (control): **same resource**, two agents. The ordinary case every adapter (fs, git, mcp alike)
/// serialises -- T2 is held at `Candidate` behind T1 until T1 reaches a terminal state.
#[test]
fn a3_1_control_same_resource_two_agents_the_second_waits() {
    let probe = spawn_probe("a3_control_same_resource");
    let (uri, path) = probe.resource("shared.md");
    std::fs::write(&path, "before").expect("seed");
    let locator = format!("{}#{uri}", probe.endpoint);
    let mut engine = engine_with_catalogue("a3_control_engine", &probe);

    let intent_1 = intent_for(
        &locator,
        "notes.write",
        &serde_json::json!({ "uri": uri, "contents": "agent-one's change" }),
    );
    engine.submit(&intent_1, 1, AT).expect("T1 submit");
    let t1 = engine.plan(&intent_1, AT).expect("T1 plan");
    let state_1 = engine.verify(&t1, AT, &key(), None).expect("T1 verify");
    assert_eq!(state_1, Lifecycle::Admitted);

    let intent_2 = intent_for(
        &locator,
        "notes.write",
        &serde_json::json!({ "uri": uri, "contents": "agent-two's change" }),
    );
    engine.submit(&intent_2, 2, AT).expect("T2 submit");
    let t2 = engine.plan(&intent_2, AT).expect("T2 plan");
    let state_2 = engine.verify(&t2, AT, &key(), None).expect("T2 verify");
    println!(
        "AUDIT_A3_CONTROL same_resource t2_state={state_2:?} blocked_by={:?}",
        engine.blocked_by(&t2)
    );
    assert_eq!(
        state_2,
        Lifecycle::Candidate,
        "same resource, one server: T2 is held behind T1 -- the ordinary case, and the baseline the \
         cross-resource test below is measured against"
    );
    assert_eq!(engine.blocked_by(&t2), Some(t1));
}

/// §2 (the claim `req/119` §5 A-3 actually makes): **different resources, one server**. `43 §8`'s
/// conflict search is keyed on `Subject::Object`, which for `gx-adapter-mcp` is the **resource**
/// (`ObjectId` from `position.locator()`, `crates/gx-engine/src/pipeline.rs:1615`,
/// `Subject::Object(*pre.id())`) — not the server `commutation()` actually compares
/// (`crates/gx-adapter-mcp/src/commutation.rs`). `Engine::conflicting_predecessor`'s `by_subject`
/// index (M6-07 adopted (a); sem: SEM-gx-cli-1602), added to bound the search to O(k) rather than O(n)) only ever considers
/// transformations sharing **the same subject**, so `commutation()` — which would answer `Conflicts`
/// for these two, being on one server — is **never even called**.
#[test]
fn a3_2_finding_different_resources_one_server_do_not_conflict_despite_the_documented_footprint() {
    let probe = spawn_probe("a3_cross_resource");
    let (uri_a, path_a) = probe.resource("agent-one.md");
    let (uri_b, path_b) = probe.resource("agent-two.md");
    std::fs::write(&path_a, "before-a").expect("seed a");
    std::fs::write(&path_b, "before-b").expect("seed b");
    let locator_a = format!("{}#{uri_a}", probe.endpoint);
    let locator_b = format!("{}#{uri_b}", probe.endpoint);
    let mut engine = engine_with_catalogue("a3_cross_engine", &probe);

    let intent_1 = intent_for(
        &locator_a,
        "notes.write",
        &serde_json::json!({ "uri": uri_a, "contents": "agent-one's change" }),
    );
    engine.submit(&intent_1, 1, AT).expect("T1 submit");
    let t1 = engine.plan(&intent_1, AT).expect("T1 plan");
    let state_1 = engine.verify(&t1, AT, &key(), None).expect("T1 verify");
    println!("AUDIT_A3_FINDING t1_state={state_1:?}");
    assert_eq!(
        state_1,
        Lifecycle::Admitted,
        "T1 is admitted and left outstanding (not committed)"
    );

    let intent_2 = intent_for(
        &locator_b,
        "notes.write",
        &serde_json::json!({ "uri": uri_b, "contents": "agent-two's change" }),
    );
    engine.submit(&intent_2, 2, AT).expect("T2 submit");
    let t2 = engine.plan(&intent_2, AT).expect("T2 plan");
    let state_2 = engine.verify(&t2, AT, &key(), None).expect("T2 verify");
    println!(
        "AUDIT_A3_FINDING t2_state={state_2:?} blocked_by={:?} arrivals_before_any_commit={}",
        engine.blocked_by(&t2),
        probe.arrivals()
    );
    // 🔴 This is the finding, asserted as observed rather than as what req/119 §5 predicts: T2 is
    // **not** held. If a future fix scopes `conflicting_predecessor`'s search to footprint (or drops
    // the `by_subject` shortcut for substrates whose footprint is broader than their subject), this
    // assertion is exactly what will need to flip, and the comment above explains why it should.
    assert_eq!(
        state_2,
        Lifecycle::Admitted,
        "🔴 FINDING (req/125 §2): two different resources on one MCP server did NOT conflict -- \
         contradicting gx-adapter-mcp's own crate-root claim ('two changes to two different \
         resources on one MCP server conflict, and one of them waits ... The cost is not softened') \
         and req/119 §5 A-3's text. by_subject's resource-level index (M6-07 adopted (a); sem: SEM-gx-cli-1603) never calls \
         commutation() for a pair that does not already share a subject."
    );
    assert_eq!(
        engine.blocked_by(&t2),
        None,
        "no wait was recorded -- T2 was simply admitted"
    );

    // Both proceed independently, with **no serialisation** -- the visible consequence of the finding
    // above: both calls reach the server, back to back, with nothing holding the second for the
    // first.
    engine.canonicalize(&t1, AT, None).expect("T1 canonicalize");
    engine.commit(&t1, AT, &key()).expect("T1 commit");
    engine.canonicalize(&t2, AT, None).expect("T2 canonicalize");
    engine.commit(&t2, AT, &key()).expect("T2 commit");
    let arrivals = probe.arrivals();
    println!("AUDIT_A3_FINDING arrivals_after_both_commits={arrivals}");
    assert_eq!(
        arrivals, 2,
        "both calls landed on the real server; the only thing this test could not show is either \
         one waiting for the other, because nothing did"
    );
    assert_eq!(
        std::fs::read_to_string(&path_a).unwrap(),
        "agent-one's change"
    );
    assert_eq!(
        std::fs::read_to_string(&path_b).unwrap(),
        "agent-two's change"
    );
}
