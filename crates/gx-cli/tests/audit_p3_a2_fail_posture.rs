// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AUDIT LANE (AC-P3-14) — A-2: fail posture, driven through the real MCP wire.**
//!
//! `req/120` §5.1: A-2 "never driven, not even once" (sem: SEM-gx-cli-1000). The first three tests build the same three types
//! `gx-cli::wrap`'s `run()` composes (`gx_engine::Engine`, `gx_gate::Gate`,
//! `gx_adapter_mcp::McpAdapter` over `gx_mcp_wire::WireTransport`) by hand, driven directly rather
//! than through `gx wrap` -- originally because `gx wrap` had **no flag** that reached the
//! `FailOpen` arm at all (the fourth test below now reads that gap as closed, **P3.1 repair**,
//! gotcha66, `req/38` §74 item③).
//!
//! `gx_engine::UnreachableEvidence` is "43 T-4d and T-4e's precondition, as a value" (sem: SEM-gx-cli-1001) (the crate's own
//! doc comment) and is the **only** way in this workspace to make a verifier actually unreachable —
//! `tests/engine_shape.rs::verifier_unavailable_has_exactly_one_producer` is gx-engine's own proof of
//! that, re-read here rather than re-proved. It is also, structurally, the reason the fifth test
//! below cannot go all the way to a real `gx wrap` subprocess: `gx wrap`'s production evidence
//! source (`gx_engine::InjectedEvidence`) and `gx serve`'s (`gx_api::state::RequestEvidence`) both
//! always answer `Ok` (`crates/gx-engine/src/pipeline.rs`, `crates/gx-api/src/state.rs`) -- T-4d/T-4e
//! are structurally unreachable through either binary today, a pre-existing fact this repair did not
//! create and does not close. What the fifth test proves instead is that the flag's value reaches
//! the **exact shared function** (`gx_cli::session::open_engine_wired`) both `gx wrap
//! --fail-posture` (since this repair) and `gx serve --fail-posture` (since before it) parse their
//! flag into and call.

#[path = "support/audit_p3_support.rs"]
mod audit_p3_support;

use std::sync::Arc;

use audit_p3_support::{intent_for, journal_path, key, spawn_probe};
use gx_core::{FailPosture, Timestamp};
use gx_engine::{Engine, Lifecycle, UnreachableEvidence};
use gx_mcp_wire::WireTransport;

const AT: Timestamp = Timestamp(1_755_000_000_000_000_000);

fn engine(name: &str, posture: FailPosture) -> Engine<UnreachableEvidence> {
    let gate = gx_gate::Gate::with_policies(
        gx_gate::packs::mcp_pack().expect("the shipped mcp pack parses"),
    );
    let evidence = UnreachableEvidence::new("AUDIT injection: the collector cannot be reached");
    Engine::open(journal_path(name), gate, evidence)
        .expect("the engine opens")
        .with_posture(posture)
}

/// A-2 / T-4d: `FailClosed` — the engine's **default**, and `gx wrap`'s only reachable posture (see
/// `gx_wrap_has_no_flag_to_reach_failopen` below) — aborts, and the real server sees **nothing**.
#[test]
fn a2_failclosed_aborts_before_the_wire_is_touched() {
    let probe = spawn_probe("a2_failclosed");
    let (uri, path) = probe.resource("notes.md");
    std::fs::write(&path, "before").expect("seed the resource");
    let locator = format!("{}#{uri}", probe.endpoint);

    let mut engine = engine("a2_failclosed_engine", FailPosture::FailClosed);
    engine.register_adapter(
        Arc::new(gx_adapter_mcp::McpAdapter::new(Arc::new(
            WireTransport::new(probe.client.clone(), probe.endpoint.clone()),
        ))),
        "audit-a2-failclosed",
    );

    let intent = intent_for(
        &locator,
        "notes.write",
        &serde_json::json!({ "uri": uri, "contents": "should never arrive" }),
    );
    engine.submit(&intent, 1, AT).expect("T-1");
    let id = engine.plan(&intent, AT).expect("T-2");
    let state = engine
        .verify(&id, AT, &key(), None)
        .expect("verify itself does not error -- the abort is a *state*, not an `Err`");

    println!(
        "AUDIT_A2_FAILCLOSED state={state:?} arrivals={}",
        probe.arrivals()
    );
    assert_eq!(
        state,
        Lifecycle::Aborted(gx_core::AbortReason::VerifierUnavailable),
        "43 T-4d: FailClosed aborts on an unreachable collector"
    );
    assert_eq!(
        probe.arrivals(),
        0,
        "🔴 A-2 FailClosed: nothing reached the server -- the abort happens before `canonicalize`/\
         `commit`, so `apply` (and the wire underneath it) was never called"
    );
    assert_eq!(
        engine.enforced(&id),
        Some(true),
        "🔴 measured, not assumed: `Engine::enforced` reads a row field that defaults to `true` and \
         is only ever flipped to `false` on the two roads that actually degrade enforcement (T-8r's \
         RecordOnly-carried-Deny and T-4e's FailOpen) -- an aborted row with neither flip keeps the \
         default. `verdict()`/`deny_reasons()` are the accessors that are genuinely empty here \
         (no gate ran); `enforced` is not one of them, and asserting the wrong expectation would \
         have been exactly the kind of self-deception A-7 warns against"
    );
    assert_eq!(
        engine.verdict(&id),
        None,
        "no gate ran, so there is no verdict to report"
    );
}

/// A-2 / T-4e: `FailOpen` — reachable at the **engine** level (this is what the mechanism does when
/// wired), even though no `gx` invocation can select it (next test). Degraded admission proceeds all
/// the way to a real `apply`, and `enforced=false` / `fail_posture_engaged=true` are the only record
/// that the verifier was never actually asked.
#[test]
fn a2_failopen_admits_degraded_and_the_real_call_still_happens() {
    let probe = spawn_probe("a2_failopen");
    let (uri, path) = probe.resource("notes.md");
    std::fs::write(&path, "before").expect("seed the resource");
    let locator = format!("{}#{uri}", probe.endpoint);

    let mut engine = engine("a2_failopen_engine", FailPosture::FailOpen);
    engine.register_adapter(
        Arc::new(gx_adapter_mcp::McpAdapter::new(Arc::new(
            WireTransport::new(probe.client.clone(), probe.endpoint.clone()),
        ))),
        "audit-a2-failopen",
    );

    let intent = intent_for(
        &locator,
        "notes.write",
        &serde_json::json!({ "uri": uri, "contents": "degraded-admission" }),
    );
    engine.submit(&intent, 1, AT).expect("T-1");
    let id = engine.plan(&intent, AT).expect("T-2");
    let state = engine.verify(&id, AT, &key(), None).expect("verify");
    println!(
        "AUDIT_A2_FAILOPEN verify_state={state:?} enforced={:?} fail_posture_engaged={:?} \
         arrivals_after_verify={}",
        engine.enforced(&id),
        engine.fail_posture_engaged(&id),
        probe.arrivals()
    );
    assert_eq!(
        state,
        Lifecycle::Admitted,
        "43 T-4e: degraded admission, not a refusal"
    );
    assert_eq!(engine.enforced(&id), Some(false), "ASM-13's receipt field");
    assert_eq!(
        engine.fail_posture_engaged(&id),
        Some(true),
        "ASM-13's other field"
    );
    assert_eq!(probe.arrivals(), 0, "verify alone reaches no transport");

    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &key()).expect("commit");
    let after = std::fs::read_to_string(&path).expect("the resource still reads");

    println!(
        "AUDIT_A2_FAILOPEN commit_state={state:?} arrivals_after_commit={} resource={after:?}",
        probe.arrivals()
    );
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(
        probe.arrivals(),
        1,
        "🔴 A-2 FailOpen: a degraded admission still reaches `apply`, and a real call landed on a \
         real server -- with the verifier never actually consulted. `enforced=false` and \
         `fail_posture_engaged=true` (asserted above) are the only trace of that in the receipt."
    );
    assert_eq!(after, "degraded-admission");
}

/// 🔴 **P3.1 repair** (gotcha66, `req/38` §74 item③, `req/125` §4-3): the finding this test used to
/// prove -- "`gx wrap` cannot reach the FailOpen arm at all" (sem: SEM-gx-cli-1002) -- is what it now proves **closed**.
/// The function name is unchanged (**C-1**: a test name is history, not a live claim); what changed
/// is the read, laid out below rather than left to the diff.
///
/// Before the repair: `Wrap`'s clap fields carried no `fail_posture`, `wrap.rs` never named the type
/// `FailPosture`, and `session.rs`'s only road into `gx wrap`'s engine (`Session::open_wired`)
/// hard-coded `FailClosed` with no parameter to override it -- of A-2's two arms, only `FailClosed`
/// was reachable from any `gx` invocation that speaks MCP.
///
/// After it: `Wrap` carries `--fail-posture <closed|open>` (the same flag and vocabulary `Serve`
/// already had), `wrap.rs::run` parses it into a `gx_core::FailPosture` and passes it to the new
/// `Session::open_wired_with_posture`, and `Session::open_wired` -- still hard-coding `FailClosed`,
/// now as *its own* default for the five callers that are not `gx wrap` -- is a caller of that
/// constructor rather than `wrap.rs`'s only road in. The old assertion
/// (`session_rs.contains("gx_core::FailPosture::FailClosed")`) would therefore still read `true` and
/// no longer prove what it once did -- exactly the "a passing assertion that stopped meaning
/// anything" shape A-7 warns against -- which is why this version derives the claim from the new
/// constructor's presence instead of from that substring surviving.
#[test]
fn gx_wrap_has_no_flag_to_reach_failopen_and_this_is_read_from_the_source() {
    let wrap_rs = include_str!("../src/wrap.rs");
    let main_rs = include_str!("../src/main.rs");
    let session_rs = include_str!("../src/session.rs");

    assert!(
        wrap_rs.contains("FailPosture"),
        "gx-cli/src/wrap.rs now names the type `FailPosture`: `run()` parses `spec.fail_posture` \
         into one and passes it to `Session::open_wired_with_posture`. If this goes red the repair \
         regressed and `WrapSpec` no longer carries a value the run function can choose"
    );
    // The `Wrap { ... }` clap variant specifically, in `main.rs` -- extracted between its own opening
    // and the next variant's, so a hit on `Serve`'s real `--fail-posture` (main.rs, a few hundred
    // lines away) cannot be mistaken for one on `Wrap`'s.
    let wrap_variant_start = main_rs
        .find("Wrap {")
        .expect("main.rs declares a Wrap variant");
    let after_wrap = &main_rs[wrap_variant_start..];
    let wrap_variant_end = after_wrap
        .find("VerdictCheckpoint {")
        .expect("VerdictCheckpoint is the next variant declared after Wrap");
    let wrap_variant_text = &after_wrap[..wrap_variant_end];
    assert!(
        wrap_variant_text.contains("gx_binary: String,"),
        "sanity: the slice extracted actually ends at Wrap's last field, not somewhere else \
         (guards this derivation against `main.rs` being reordered out from under it)"
    );
    assert!(
        wrap_variant_text.contains("fail_posture"),
        "🔴 the `Wrap` command's own flags (main.rs) now carry `--fail-posture`, the same flag and \
         vocabulary `Serve`'s already had -- of A-2's two arms, both are now reachable from a `gx \
         wrap` invocation. Wrap variant text was: {wrap_variant_text}"
    );
    assert!(
        session_rs.contains("open_wired_with_posture"),
        "gx-cli/src/session.rs now carries `Session::open_wired_with_posture`, which `wrap.rs::run` \
         calls with the parsed flag instead of going through `Session::open_wired`'s hard-coded \
         `FailClosed` -- the repair's actual road in"
    );
}

/// 🔴 **CLI version of A-2** (P3.1 repair AC, `req/38` §74 item③): the three tests above build the
/// engine's three types by hand; this one drives them through
/// `gx_cli::session::open_engine_wired` -- the production function `Session::open_wired_with_posture`
/// (`gx wrap --fail-posture open`'s road, since this repair) and `crate::serve::build`
/// (`gx serve --fail-posture open`'s, since before it) both parse their flag into and call,
/// `register_default_adapters_with` included, which a hand-built engine (the three tests above)
/// skips. What it substitutes -- `UnreachableEvidence` for the CLI's own `InjectedEvidence` -- and
/// why a real `gx wrap` subprocess cannot do this at all today is the module header's second
/// paragraph.
#[test]
fn a2_cli_wired_through_open_engine_wired_failopen_reaches_a_committed_receipt() {
    let probe = spawn_probe("a2_cli_failopen");
    let (uri, path) = probe.resource("notes.md");
    std::fs::write(&path, "before").expect("seed the resource");
    let locator = format!("{}#{uri}", probe.endpoint);

    let project = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("audit_p3")
        .join("a2_cli_failopen_project");
    if project.exists() {
        std::fs::remove_dir_all(&project).expect("clear the project directory");
    }
    let layout = gx_cli::layout::Layout::create(&project).expect("a fresh project layout");

    let transport: Arc<dyn gx_adapter_mcp::ToolTransport> = Arc::new(WireTransport::new(
        probe.client.clone(),
        probe.endpoint.clone(),
    ));
    let mcp = gx_cli::session::McpWiring::wired(transport, gx_adapter_mcp::Catalogue::new());

    let mut engine = gx_cli::session::open_engine_wired(
        &layout,
        UnreachableEvidence::new(
            "AUDIT injection: the collector cannot be reached (CLI-wired arm)",
        ),
        None,
        FailPosture::FailOpen,
        None,
        &mcp,
    )
    .expect("the same construction `gx wrap --fail-posture open` now reaches");

    let intent = intent_for(
        &locator,
        "notes.write",
        &serde_json::json!({ "uri": uri, "contents": "degraded-admission-cli" }),
    );
    engine.submit(&intent, 1, AT).expect("T-1");
    let id = engine.plan(&intent, AT).expect("T-2");
    let state = engine.verify(&id, AT, &key(), None).expect("verify");
    assert_eq!(
        state,
        Lifecycle::Admitted,
        "43 T-4e via the CLI's own shared constructor, not a hand-built engine"
    );
    assert_eq!(engine.enforced(&id), Some(false), "ASM-13's receipt field");
    assert_eq!(
        engine.fail_posture_engaged(&id),
        Some(true),
        "ASM-13's other field"
    );

    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &key()).expect("commit");
    let after = std::fs::read_to_string(&path).expect("the resource still reads");
    println!(
        "AUDIT_A2_CLI_FAILOPEN state={state:?} enforced={:?} fail_posture_engaged={:?} \
         arrivals={} resource={after:?}",
        engine.enforced(&id),
        engine.fail_posture_engaged(&id),
        probe.arrivals()
    );
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(
        probe.arrivals(),
        1,
        "🔴 the receipt's enforced=false/fail_posture_engaged=true is the only trace of a real call \
         landing with the verifier never actually consulted -- through the CLI's own construction \
         path this time, not a hand-built one"
    );
    assert_eq!(after, "degraded-admission-cli");
}
