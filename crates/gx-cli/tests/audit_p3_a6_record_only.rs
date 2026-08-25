// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AUDIT LANE (AC-P3-14) — A-6: `--record-only` lets a Denied MCP change apply anyway.**
//!
//! `req/120` §5.1: A-6 "never driven, not even once" (sem: SEM-gx-cli-1003). DR-2 (`req/38` §71 ruling, `gx-core::enforcement`):
//! "the application goes through, but `enforced=false`" (sem: SEM-gx-cli-1004). This file drives the actual Deny (the shipped
//! `policies/mcp/deny-etc-resources.cedar` pack, the same pack AC-P3-13's own E2E exercises for its
//! Deny arm) under `EnforcementMode::RecordOnly` and measures, on the real server, that the tool call
//! the ordinary path would have refused **reaches the server anyway**.

#[path = "support/audit_p3_support.rs"]
mod audit_p3_support;

use std::sync::Arc;

use audit_p3_support::{intent_for, journal_path, key, spawn_probe};
use gx_core::{EnforcementMode, FailPosture, Timestamp};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use gx_mcp_wire::WireTransport;

const AT: Timestamp = Timestamp(1_755_200_000_000_000_000);

/// A-6 / DR-2 / T-8r: a Denied MCP write, replayed under `RecordOnly`, still reaches `apply` -- the
/// real server, not a projection of one -- with `enforced=false` recorded.
///
/// # 🔴 Why the locator names the real `/etc/hostname` and the tool's own effect lands elsewhere
///
/// `gx-adapter-mcp`'s `snapshot`/`precondition` (T-2/T-10a) read whatever `intent.locator()` names --
/// this test's probe server maps a `file://` URI straight onto the OS path, with no redirection, so
/// the transformation's *declared position* has to be a file this process can actually read.
/// `testuser2` (this WSL user) cannot write anywhere under the real `/etc` (checked: `touch
/// /etc/x` → `Permission denied`, no passwordless `sudo`), so a locator claiming `/etc/hostname` is
/// used for exactly what the shipped pack judges — the **string** `resource.locator` — and the
/// pack does not read a byte from the file it names (`deny-etc-resources.cedar`'s own header: "the
/// tool's name is not visible to a policy" (sem: SEM-gx-cli-1005), and the same opacity, P-6, covers the tool's
/// *arguments*). The tool call's own `uri` argument — which is what the server actually opens for
/// `notes.write` — names this test's own scratch file instead, exactly the freedom `gx wrap`'s own
/// `EngineGate` closes by construction (it derives the locator *from* `--resource-arg`) and the raw
/// `Engine::submit`/`plan` API used directly here does not. `/etc/hostname` itself is read-only and
/// is asserted unchanged at the end, so this suite never attempts a write the OS would refuse.
#[test]
fn a6_a_denied_write_still_reaches_the_server_under_record_only() {
    let probe = spawn_probe("a6_record_only");
    let (effect_uri, effect_path) = probe.resource("record-only-effect.md");
    std::fs::write(&effect_path, "untouched").expect("seed the effect resource");
    let etc_hostname_before = std::fs::read_to_string("/etc/hostname")
        .expect("this WSL host has a readable /etc/hostname");
    let locator = format!("{}#file:///etc/hostname", probe.endpoint);
    let before = std::fs::read_to_string(&effect_path).expect("seed reads back");

    let gate = gx_gate::Gate::with_policies(
        gx_gate::packs::mcp_pack().expect("the shipped mcp pack parses"),
    );
    let mut engine = Engine::open(journal_path("a6_engine"), gate, InjectedEvidence::none())
        .expect("the engine opens")
        .with_posture(FailPosture::FailClosed);
    engine.register_adapter(
        Arc::new(gx_adapter_mcp::McpAdapter::new(Arc::new(
            WireTransport::new(probe.client.clone(), probe.endpoint.clone()),
        ))),
        "audit-a6",
    );

    let arguments = serde_json::json!({
        "uri": effect_uri,
        "contents": "record-only wrote this despite the Deny",
    });
    let intent = intent_for(&locator, "notes.write", &arguments);

    engine.submit(&intent, 1, AT).expect("submit");
    let id = engine.plan(&intent, AT).expect("plan");
    let record_only = Some(EnforcementMode::RecordOnly);
    let state = engine.verify(&id, AT, &key(), record_only).expect("verify");
    println!(
        "AUDIT_A6 verify_state={state:?} arrivals_before_canonicalize={}",
        probe.arrivals()
    );
    assert_eq!(
        state,
        Lifecycle::Denied,
        "the shipped mcp pack denies a write locatored under /etc -- the ordinary verdict, unchanged \
         by RecordOnly (DR-2: the *verdict* is not softened, only what happens after it)"
    );
    assert_eq!(probe.arrivals(), 0, "a Deny by itself still makes no call");

    let state = engine
        .canonicalize(&id, AT, record_only)
        .expect("T-8r: RecordOnly opens canonicalize to a Denied row");
    assert_eq!(state, Lifecycle::Canonicalized);
    assert_eq!(
        engine.enforced(&id),
        Some(false),
        "T-8r's flag: the record now says the gate's verdict was **not** enforced"
    );

    let state = engine.commit(&id, AT, &key()).expect("commit");
    let after = std::fs::read_to_string(&effect_path).expect("the resource still reads");
    let etc_hostname_after =
        std::fs::read_to_string("/etc/hostname").expect("/etc/hostname still reads");
    println!(
        "AUDIT_A6 commit_state={state:?} arrivals_after_commit={} before={before:?} after={after:?}",
        probe.arrivals()
    );
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(
        probe.arrivals(),
        1,
        "🔴 A-6 / DR-2: under --record-only, a Denied MCP tool call still reaches the real server -- \
         `enforced=false` (asserted above) is the only trace in the receipt that the gate's Deny was \
         overridden"
    );
    assert_eq!(
        after, "record-only wrote this despite the Deny",
        "and the effect is real: the resource moved"
    );
    assert_ne!(before, after);
    assert_eq!(
        etc_hostname_after, etc_hostname_before,
        "the real /etc/hostname -- named only in the locator the pack judges -- was never opened for \
         writing by this suite"
    );
}
