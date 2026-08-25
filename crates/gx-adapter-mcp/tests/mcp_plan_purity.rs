// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `plan` reaches nothing, measured two ways — and one of them is stronger than the other two adapters'.
//!
//! **E-M4-29** (§30 M4H2-3 (b)) reads 41 §4's "a pure function" as "determinism over the (intent, pre) pair + zero (sem: SEM-gx-adapter-mcp-285)
//! **writes** to the substrate" and adds "reads are not forbidden", and req/98 §6-7 asks the M7 adapters for that reading: (sem: SEM-gx-adapter-mcp-286)
//! "Rule 1's M7 version... is not 'zero I/O'". (sem: SEM-gx-adapter-mcp-287)
//!
//! 🔴 **This adapter reads nothing either, and here that is checkable in a way it is not elsewhere.**
//! `gx-adapter-fs` compares the tree on disk and `gx-adapter-git` compares `.git` byte for byte; both
//! answer "nothing was written" and neither can answer "nothing was read", because a read leaves no (sem: SEM-gx-adapter-mcp-288)
//! trace on a filesystem. A transport is an object, so a **counter** on it answers both questions
//! exactly: after a plan, `calls` and `reads` are still zero.
//!
//! The text half is kept as well, because the counter measures **this** transport and the scan measures
//! the module. A module that named a write would be one whose next hand could reach a different
//! transport.

mod support;

use gx_substrate::SubstrateAdapter;
use support::{
    absent_snapshot, intent_for, locator_on, McpFixture, GOAL, SERVER, SUBJECT, WRITE_TOOL,
};

/// The counter: six plans, and the transport was never touched.
#[test]
fn planning_reaches_neither_a_call_nor_a_read() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = locator_on(SERVER, SUBJECT);
    let pre = absent_snapshot(&locator);

    for n in 0..6u8 {
        let arguments = vec![b'a' + n];
        adapter
            .plan(&intent_for(&locator, WRITE_TOOL, &arguments), &pre)
            .expect("a well-formed call plans");
    }

    println!(
        "MCP_PLAN_PURITY plans=6 calls={} reads={}",
        fixture.server().calls(),
        fixture.server().reads()
    );
    assert_eq!(fixture.server().calls(), 0, "`plan` made a tool call");
    assert_eq!(
        fixture.server().reads(),
        0,
        "`plan` read the substrate. E-M4-29 permits it and **L1** is what forbids it here: a plan \
         that read the resource would produce a different payload every time the server moved"
    );
}

/// **L1** in this crate's own terms: the same `(intent, pre)` planned either side of a real tool call
/// produces the same delta, byte for byte.
///
/// The harness measures L1 with `Fixture::disturb`; this measures it across an **apply**, which is the
/// move the fixture's disturbance stands in for.
#[test]
fn the_same_pair_plans_the_same_delta_across_a_real_call() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = locator_on(SERVER, SUBJECT);
    let pre = adapter.snapshot(&locator).expect("the server answers");

    let before = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, GOAL), &pre)
        .expect("plan");
    adapter.apply(&before).expect("apply");
    let after = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, GOAL), &pre)
        .expect("plan");

    println!(
        "MCP_PLAN_DETERMINISM equal={} payload_bytes={}",
        before == after,
        before.payload().len()
    );
    assert_eq!(
        before, after,
        "the same (intent, snapshot) planned two different deltas once the server moved, so the \
         answer depends on something that is not an argument (E-M4-4, E-M4-29)"
    );
    assert_eq!(
        before.reference(),
        after.reference(),
        "two deltas that are equal have one CID, which is what makes the call log's key (42 §3.4) \
         a key rather than a coincidence"
    );
}

/// The text half: `src/plan.rs` names no transport operation at all.
///
/// 🔴 Its limit is in the probe, in the **M6H8-1** form: this is a text gate, so a call reached through
/// an alias, a re-export or a macro is outside what it can see. That gap is closed by the counter above,
/// which is why both are here and why neither is presented as the whole measurement.
#[test]
fn the_plan_module_names_no_transport_operation() {
    let plan = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plan.rs"),
    )
    .expect("the module is readable");

    let code: Vec<&str> = plan
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.is_empty() || t.starts_with("//"))
        })
        .collect();
    let mut named: Vec<&str> = Vec::new();
    for word in [
        "ToolTransport",
        "transport",
        ".call(",
        ".read(",
        "Admitted",
        "ToolCall",
        "CallLog",
    ] {
        if code.iter().any(|l| l.contains(word)) {
            named.push(word);
        }
    }
    println!(
        "MCP_PLAN_SCAN code_lines={} named={named:?} \
         LIMIT=text-gate(alias/re-export/macro invisible; the counter above closes it)",
        code.len()
    );
    assert!(
        code.len() > 20,
        "the scan read {} code lines, which is not this module",
        code.len()
    );
    assert!(
        named.is_empty(),
        "`src/plan.rs` names {named:?}: \"calls nothing\" is a claim about source, and the cheapest (sem: SEM-gx-adapter-mcp-289) \
         honest way to keep it is a module that never names one"
    );
}
