// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The `makenotion/notion-mcp-server` target's do/undo catalogue and its example policy pack
//! (v0.4-b, `req/170` §3-B, `req/169` §8's recon, `req/183`'s report) — the second real SaaS
//! target after github, and the first declaration that rides on the **fifth** vocabulary word
//! alone (`{"do_result": "/id"}`, `ArgSource::DoResult`): the value the compensating call needs
//! is the created page's own UUID, which the server's do-time answer carries verbatim as `id`,
//! so no derivation (the sixth word's `/(\d+)$` minting) is involved. `req/38` §107, ruling 7, read (sem: SEM-gx-adapter-mcp-372)
//! this off the API reference; this file holds it against the **real** observed result
//! (`fixtures/notion-post-page-observation.json`, captured by `tools/a2b_notion_raw_pair.py`
//! from the real server on 2026-08-15, id/url/in_trash/object members only).
//!
//! # The pair, and why the compensating tool is `API-delete-a-block` and not `API-patch-page`
//!
//! | forward tool | undone by | measured on the real server (req/183 §R) |
//! |---|---|---|
//! | `API-post-page` (create a page) | `API-delete-a-block` with `block_id` = the created `id` | a page is a block; `DELETE /v1/blocks/{id}` sets `in_trash`/`archived` true, read back `in_trash == true` |
//!
//! `req/169` §8 wrote the undo as "`PATCH /v1/pages/{id}` with `in_trash: true`". That is the (sem: SEM-gx-adapter-mcp-373)
//! same trash operation, but as a **tool argument** it is a JSON boolean, and the template
//! vocabulary's constant word (`ArgSource::Const`) is string-only — `{"const": "true"}` would send
//! `"in_trash": "true"`, which the Notion API's strict body validation refuses. The DELETE tool
//! reaches the same server state with the one argument the fifth word can supply, so the pair is
//! declarable **today** with no vocabulary change (gotcha92: the brief's example pair is checked
//! against the real tool list before it is declared — the same move `github_target_catalogue.rs`
//! made when `create_branch ↔ delete_branch` turned out not to exist). A typed constant is named
//! in req/183 as DR-V4B-2 rather than quietly added here.
//!
//! # 🔴 What this file does **not** claim
//!
//! It measures the **declaration** (parse, split, completion against a real observation) and the
//! **policy** (the fixture pack) — the two things an operator writes by hand. It does not claim
//! `gx undo` green against the real server. "lands" (this file) and "undo green" (the E2E, (sem: SEM-gx-adapter-mcp-374)
//! `tools/a2b_notion_undo_e2e.sh`) are two claims with two evidence lines —
//! `req/38` §107, ruling 7's split, kept. (sem: SEM-gx-adapter-mcp-375)
//!
//! # 🔴 **DR-46-16 moved one of the two reasons this file used to give, and the old sentence is
//! kept beside the correction rather than deleted**
//!
//! Until `req/38` §218 this paragraph read: *"notion-mcp-server declares no `resources` capability
//! (`initialize` → `{"tools":{}}`, `resources/read` → -32601, measured), so `McpAdapter::snapshot`
//! cannot read the subject and `gx wrap` refuses to plan (`req/152`'s gotcha93 family, now for a
//! whole server rather than one resource kind)"* — and the E2E was 🔴 **at plan**.
//!
//! The measurement of the server is unchanged and still true: it has no resource face. What
//! changed is that a resource face is no longer the only road to a compare-and-set. A deployment
//! declares `"$cas_read"` naming the read tool for a locator prefix
//! ([`gx_adapter_mcp::CAS_READ_KEY`]) and `snapshot`, `precondition` and the post-apply
//! observation take it. The section "the tools-only road" below holds **both** halves of that as
//! facts rather than as prose: without the declaration the plan is still refused, and with it the
//! whole road runs on a server that answers `-32601` to every `resources/read`.
//!
//! What is still not claimed: this is the **shipped demo server's shape**, in process. No call
//! reached notion.so in this file, and `req/308` says which arms are fixtures and which are not.
//!
//! Notion-Version pin: 2025-09-03, sent per operation by the server itself (commit 1d38420 /
//! package 2.5.1); the catalogue JSON has no metadata slot, so the pin is stated in the pack's
//! header and asserted by the E2E preflight (`req/183` DR-V4B-2b names the missing slot).

mod support;

use std::sync::Arc;

use gx_adapter_mcp::delta::{McpDelta, McpOp};
use gx_adapter_mcp::{ArgSource, Catalogue, McpAdapter};
use gx_core::SubstrateKind;
use gx_gate::packs::{self, PackCase, PackExpectation};
use gx_gate::{Gate, PolicyEngine};
use gx_substrate::{InverseCompletion, SubstrateAdapter};

use support::{FakeServer, SERVER, SUBJECT};

/// The catalogue file `tools/a2b_notion_undo_e2e.sh` hands to `gx wrap --restore-catalogue` and
/// `gx undo --mcp-restore-catalogue`.
const NOTION_CATALOGUE_JSON: &[u8] = include_bytes!("fixtures/notion-page-catalogue.json");

/// The pack the same script hands to `--policy`.
const NOTION_TARGET_PACK_SOURCE: &str = include_str!("fixtures/notion-target-a2b.cedar");

/// The real server's answer to `API-post-page` (first content item's `text`, parsed), captured
/// on 2026-08-15 by `tools/a2b_notion_raw_pair.py` — the members the declaration can point at.
const REAL_OBSERVATION: &[u8] = include_bytes!("fixtures/notion-post-page-observation.json");

/// The throwaway parent this lane's E2E scopes its permit to (the pack's header says why).
const THROWAWAY_PARENT: &str = "3bd8cc9a-40ed-81b8-b572-f339da4f57ac";

fn notion_catalogue() -> Catalogue {
    Catalogue::from_json(NOTION_CATALOGUE_JSON).expect("fixtures/notion-page-catalogue.json parses")
}

/// A forward `API-post-page` delta at the fake server's readable subject (the locator is a
/// position; the pair is named by the tool, so any readable position serves the escrow test).
fn forward_post_page_delta() -> gx_substrate::PlannedDelta {
    let arguments = br#"{"parent":{"page_id":"3bd8cc9a-40ed-81b8-b572-f339da4f57ac"},"properties":{"title":[{"text":{"content":"glovrex-throwaway-child"}}]}}"#;
    let payload = McpDelta::one(McpOp::call(
        format!("{SERVER}#{SUBJECT}"),
        "API-post-page".to_string(),
        arguments.to_vec(),
    ))
    .encode()
    .expect("a forward payload encodes");
    gx_substrate::PlannedDelta::new(SubstrateKind::Mcp, payload).expect("a delta mints")
}

fn adapter() -> McpAdapter {
    McpAdapter::new(Arc::new(FakeServer::new())).with_catalogue(notion_catalogue())
}

// ---------------------------------------------------------------------------
// The declaration
// ---------------------------------------------------------------------------

/// The file declares exactly the one pair the module doc names, by the server's **real** tool
/// names (`API-` prefixed — `tools/list` measured, not the OpenAPI operationIds the README shows).
#[test]
fn the_notion_catalogue_declares_the_create_trash_pair_by_real_tool_names() {
    let catalogue = notion_catalogue();
    assert_eq!(
        catalogue.declared(),
        1,
        "one pair: API-post-page → API-delete-a-block"
    );
    assert_eq!(
        catalogue.restore_for("API-post-page"),
        Some("API-delete-a-block")
    );
    for undeclared in [
        "post-page",
        "API-patch-page",
        "API-move-page",
        "API-update-page-markdown",
    ] {
        assert_eq!(
            catalogue.restore_for(undeclared),
            None,
            "{undeclared}: not declared — an unknown tool is irreversible as far as gx knows (E-M3-4)"
        );
    }
}

/// The template is the fifth word alone: one member, `block_id`, from the observed result's `/id`
/// — no escrow-time material, no derivation. `resolve_split` therefore resolves **nothing** now and
/// leaves exactly that member pending (the partial escrow `req/38` §98, ruling 1 admits). (sem: SEM-gx-adapter-mcp-376)
#[test]
fn the_template_is_the_fifth_word_alone_and_splits_into_one_pending_member() {
    let catalogue = notion_catalogue();
    let spec = catalogue.spec_for("API-post-page").expect("declared");
    let template = spec.template().expect("the pair carries a template");
    assert_eq!(template.arguments().len(), 1, "one member");
    assert_eq!(
        template.arguments().get("block_id"),
        Some(&ArgSource::DoResult("/id".to_string())),
        "the member is the fifth word, pointer `/id`"
    );
    assert!(
        template.arguments().values().all(ArgSource::is_do_result),
        "no member resolves before apply: the created page does not exist yet (E-M4-30's physics)"
    );

    let (resolved, pending) = template
        .resolve_split(br#"{"parent":{"page_id":"x"},"properties":{}}"#, b"")
        .expect("the escrow-time half resolves");
    let resolved: serde_json::Value = serde_json::from_slice(&resolved).expect("JSON");
    assert_eq!(
        resolved,
        serde_json::json!({}),
        "nothing is resolvable before apply"
    );
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending.get("block_id"),
        Some(&ArgSource::DoResult("/id".to_string()))
    );

    let one_phase = template.resolve(br#"{}"#, b"");
    assert!(
        one_phase.is_err(),
        "the one-phase `resolve` refuses a do-result member by name (it is not escrow-time material)"
    );
}

/// Why the compensating tool is the DELETE endpoint and not `API-patch-page {in_trash: true}`:
/// the constant word is **string-only**, and `"true"` is not `true` to a strictly validated body.
/// Held here so the substitution stays a measured reason rather than a preference.
#[test]
fn the_constant_word_is_string_only_which_is_why_the_pair_avoids_a_boolean_argument() {
    let json = serde_json::to_value(ArgSource::Const("true".to_string())).expect("serialises");
    assert_eq!(json, serde_json::json!({"const": "true"}));
    let resolved = gx_adapter_mcp::RestoreTemplate::new()
        .with("in_trash", ArgSource::Const("true".to_string()))
        .resolve(b"{}", b"")
        .expect("resolves");
    let resolved: serde_json::Value = serde_json::from_slice(&resolved).expect("JSON");
    assert_eq!(
        resolved["in_trash"],
        serde_json::Value::String("true".to_string()),
        "a JSON string, not a boolean — the Notion API refuses `\"in_trash\": \"true\"`"
    );
    // And the fixture indeed carries no such member (a reviewer "fixing" it to the README's (sem: SEM-gx-adapter-mcp-377)
    // PATCH form would reintroduce the string/boolean mismatch silently).
    let text = std::str::from_utf8(NOTION_CATALOGUE_JSON).expect("UTF-8");
    assert!(
        !text.contains("in_trash"),
        "the declared pair carries no `in_trash` argument (the DELETE tool needs none)"
    );
}

// ---------------------------------------------------------------------------
// The escrow and the completion, against the real observed result
// ---------------------------------------------------------------------------

/// `invert` escrows a **partial** (`Some`, one pending member) for the declared pair — so
/// E-M4-5's fold answers `invert_available = true` and the gate is not asked to escalate.
#[test]
fn invert_escrows_a_partial_for_the_notion_pair() {
    let adapter = adapter();
    let delta = forward_post_page_delta();
    let pre = adapter
        .snapshot(&format!("{SERVER}#{SUBJECT}"))
        .expect("the fake server's subject is readable");
    let partial = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("a declared do-result pair escrows a partial, not None");
    let decoded = McpDelta::decode(partial.payload()).expect("decodes");
    let op = decoded.ops().first().expect("one op");
    assert_eq!(op.tool(), "API-delete-a-block");
    assert_eq!(op.pending().len(), 1);
    assert!(adapter
        .needs_completion(&partial)
        .expect("this adapter's grammar"));
}

/// The **real** observed result completes the inverse: `block_id` is the created page's UUID,
/// verbatim, a JSON string -- the fifth word's plain form (`req/38` §107, ruling 7's "bare form"). (sem: SEM-gx-adapter-mcp-378)
#[test]
fn the_real_observed_result_completes_the_inverse_with_the_created_pages_id() {
    let observed: serde_json::Value = serde_json::from_slice(REAL_OBSERVATION).expect("JSON");
    assert_eq!(observed["object"], "page");
    assert_eq!(observed["in_trash"], false, "created, not yet trashed");
    let id = observed["id"]
        .as_str()
        .expect("the page object carries `id`");
    assert_eq!(id.len(), 36, "a hyphenated UUID, carried as a string");

    let adapter = adapter();
    let delta = forward_post_page_delta();
    let pre = adapter
        .snapshot(&format!("{SERVER}#{SUBJECT}"))
        .expect("readable");
    let partial = adapter
        .invert(&delta, &pre)
        .expect("answers")
        .into_inverse()
        .expect("partial");
    let full = adapter
        .complete_inverse(&partial, REAL_OBSERVATION)
        .expect("this adapter's grammar")
        .expect("the observation carries `/id`");
    let decoded = McpDelta::decode(full.payload()).expect("decodes");
    let op = decoded.ops().first().expect("one op");
    assert!(
        op.pending().is_empty(),
        "a completed inverse owes nothing further"
    );
    let arguments: serde_json::Value = serde_json::from_slice(op.arguments()).expect("JSON");
    assert_eq!(
        arguments,
        serde_json::json!({ "block_id": id }),
        "exactly the one argument `API-delete-a-block` takes, the created page's own id"
    );
    assert_ne!(full.reference(), partial.reference());
}

/// The folds: an observation with no `id` (an `isError` text, say) or a non-JSON one folds to
/// `None` — undo refused by name, never a wrong `delete-a-block` on some other block.
#[test]
fn an_observation_without_the_id_folds_to_none() {
    let adapter = adapter();
    let delta = forward_post_page_delta();
    let pre = adapter
        .snapshot(&format!("{SERVER}#{SUBJECT}"))
        .expect("readable");
    let partial = adapter
        .invert(&delta, &pre)
        .expect("answers")
        .into_inverse()
        .expect("partial");
    for observation in [
        br#"{"object":"error","status":400,"code":"validation_error","message":"body failed validation"}"#.as_slice(),
        b"Request failed with status code 400",
        b"",
    ] {
        assert!(
            adapter
                .complete_inverse(&partial, observation)
                .expect("this adapter's grammar")
                .is_none(),
            "{:?} carries no `/id` and must fold, not mint",
            String::from_utf8_lossy(observation)
        );
    }
}

// ---------------------------------------------------------------------------
// The example pack
// ---------------------------------------------------------------------------

fn notion_locator(endpoint: &str, page: &str) -> String {
    format!("{endpoint}#notion://pages/{page}")
}

fn notion_target_cases() -> Vec<PackCase> {
    vec![
        PackCase::new(
            "a call whose subject is the designated throwaway parent page is admitted",
            SubstrateKind::Mcp,
            notion_locator("stdio://notion-mcp-server", THROWAWAY_PARENT),
            PackExpectation::admit_by("notion-target-a2b-permit-throwaway-parent"),
        )
        .because(
            "the one statement this pack carries: a permit scoped to the one throwaway parent's \
             synthetic URI — the analogue of the github packs' repository prefix",
        ),
        PackCase::new(
            "a call whose subject is any other page is refused by no policy",
            SubstrateKind::Mcp,
            notion_locator(
                "stdio://notion-mcp-server",
                "4648cc9a-40ed-82e4-905b-01c1de640362",
            ),
            PackExpectation::DenyByNoPolicy,
        )
        .because(
            "Cedar is default-deny and the pack names one parent — an operator's existing page \
             (this id is the workspace's own \"My first project\" row, the one page this lane (sem: SEM-gx-adapter-mcp-379) \
             must never write) reaches Cedar's third rule rather than being admitted",
        ),
        PackCase::new(
            "a change on a substrate this pack does not speak for is refused by no policy",
            SubstrateKind::Fs,
            "/tmp/x".to_string(),
            PackExpectation::DenyByNoPolicy,
        )
        .because("the one statement is scoped to the mcp substrate"),
    ]
}

#[test]
fn the_notion_target_pack_parses_and_carries_named_statements() {
    let engine = PolicyEngine::parse(NOTION_TARGET_PACK_SOURCE)
        .expect("fixtures/notion-target-a2b.cedar parses");
    assert_eq!(
        engine.policy_ids(),
        vec!["notion-target-a2b-permit-throwaway-parent".to_string()]
    );
}

#[test]
fn the_notion_target_pack_holds_at_least_one_admit_case_and_one_deny_case() {
    let gate = Gate::with_policies(
        PolicyEngine::parse(NOTION_TARGET_PACK_SOURCE)
            .expect("fixtures/notion-target-a2b.cedar parses"),
    );
    let report = packs::check_pack(&gate, &notion_target_cases()).expect("every case is evaluable");
    println!("A2B_NOTION_TARGET_PACK {report}");
    assert_eq!(report.failures(), &[] as &[String]);
    assert!(report.holds());
    assert!(report.admits() >= 1);
    assert!(report.denies() >= 1);
}

// ---------------------------------------------------------------------------
// 🔴 The tools-only road (DR-46-16, `req/38` §218 ruling 1)
// ---------------------------------------------------------------------------

/// The same catalogue, plus the `$cas_read` slot a tools-only deployment writes.
const NOTION_CATALOGUE_CAS_JSON: &[u8] = include_bytes!("fixtures/notion-page-catalogue-cas.json");

/// The page a tools-only deployment names, and the prefix the declaration is keyed by.
const NOTION_PAGE: &str = "notion://page/3bd8cc9a-40ed-81b8-b572-f339da4f57ac";
const NOTION_ENDPOINT: &str = "https://mcp.notion.com/mcp";

/// A stand-in for notion-mcp-server's measured shape: `resources/read` answers `-32601` for
/// **everything**, and the page is reachable only through `API-retrieve-a-page`.
#[derive(Debug)]
struct ToolsOnlyNotion {
    body: std::sync::Mutex<Vec<u8>>,
    reads: std::sync::atomic::AtomicUsize,
    tool_reads: std::sync::Mutex<Vec<(String, String)>>,
}

impl ToolsOnlyNotion {
    fn new() -> Self {
        Self {
            body: std::sync::Mutex::new(br#"{"title":"glovrex-throwaway-child"}"#.to_vec()),
            reads: std::sync::atomic::AtomicUsize::new(0),
            tool_reads: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl gx_adapter_mcp::ToolTransport for ToolsOnlyNotion {
    fn read(&self, _server: &str, resource: &str) -> gx_substrate::Result<Vec<u8>> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(gx_substrate::Error::Unreadable {
            locator: resource.to_string(),
            detail: "notion-mcp-server: initialize -> tools only; resources/read -> -32601"
                .to_string(),
        })
    }

    fn read_prior_by_tool(
        &self,
        _server: &str,
        tool: &str,
        arguments: &[u8],
    ) -> gx_substrate::Result<Vec<u8>> {
        self.tool_reads.lock().expect("not poisoned").push((
            tool.to_string(),
            String::from_utf8_lossy(arguments).into_owned(),
        ));
        if tool != "API-retrieve-a-page" {
            return Err(gx_substrate::Error::Unreadable {
                locator: tool.to_string(),
                detail: format!("no tool {tool:?}"),
            });
        }
        Ok(self.body.lock().expect("not poisoned").clone())
    }

    fn call(
        &self,
        call: &gx_adapter_mcp::ToolCall,
        _admitted: &gx_adapter_mcp::Admitted,
    ) -> gx_substrate::Result<Vec<u8>> {
        let _ = call;
        *self.body.lock().expect("not poisoned") = br#"{"in_trash":true}"#.to_vec();
        Ok(REAL_OBSERVATION.to_vec())
    }
}

fn tools_only_locator() -> String {
    format!("{NOTION_ENDPOINT}#{NOTION_PAGE}")
}

/// 🔴 **The half that has not changed**, held as a fact rather than as a comment: on a server with
/// no resource face and **no** `$cas_read` declaration, `snapshot` still refuses, so `gx wrap`
/// still refuses to plan. This is the sentence this file's module doc carried from v0.4-b to
/// `req/38` §218, and it is what stops every arm below from being satisfiable by an adapter that
/// simply stopped reading.
#[test]
fn without_a_cas_read_declaration_a_tools_only_page_still_refuses_to_plan() {
    let server = Arc::new(ToolsOnlyNotion::new());
    let mcp = McpAdapter::new(server.clone()).with_catalogue(notion_catalogue());
    let refused = mcp
        .snapshot(&tools_only_locator())
        .expect_err("the measured shape of notion-mcp-server has no resource face");
    println!(
        "NOTION_TOOLS_ONLY_UNDECLARED refused={refused} reads={} tool_reads={:?}",
        server.reads.load(std::sync::atomic::Ordering::SeqCst),
        server.tool_reads.lock().expect("not poisoned")
    );
    assert!(matches!(refused, gx_substrate::Error::Unreadable { .. }));
    assert_eq!(
        server.tool_reads.lock().expect("not poisoned").len(),
        0,
        "🔴 an undeclared locator does not reach the read-by-tool road: declaration is the only \
         thing that unlocks it"
    );
}

/// 🔴 **DR-46-16, end to end on the shape that motivated it**: the same server, the same page, and
/// a catalogue that adds `$cas_read`. `snapshot` lands, `precondition` lands, the escrow is built
/// and completed from the **real** observed result, and the post-apply observation is read back
/// through the same declared face.
///
/// **Red before this lane** at the first line: `mcp.snapshot(..)` was `Err(Unreadable)`, so nothing
/// after it ran. That is `req/38` §123 ruling 1 (b)'s open ground, closed for a declared locator.
#[test]
fn a_cas_read_declaration_gives_the_tools_only_page_a_compare_and_set() {
    let server = Arc::new(ToolsOnlyNotion::new());
    let catalogue = Catalogue::from_json(NOTION_CATALOGUE_CAS_JSON)
        .expect("fixtures/notion-page-catalogue-cas.json parses");
    assert_eq!(
        catalogue.declared(),
        1,
        "the reserved slot is metadata: the pair is still the one pair"
    );
    assert_eq!(catalogue.cas_reads_declared(), 1);
    let mcp = McpAdapter::new(server.clone()).with_catalogue(catalogue);

    let locator = tools_only_locator();
    let pre = mcp
        .snapshot(&locator)
        .expect("🔴 DR-46-16: a tools-only page now has a compare-and-set");
    let before = mcp.precondition(&pre).expect("and a fingerprint");

    let arguments = br#"{"parent":{"page_id":"3bd8cc9a-40ed-81b8-b572-f339da4f57ac"},"properties":{"title":[{"text":{"content":"glovrex-throwaway-child"}}]}}"#;
    let payload = McpDelta::one(McpOp::call(
        locator.clone(),
        "API-post-page".to_string(),
        arguments.to_vec(),
    ))
    .encode()
    .expect("a forward payload encodes");
    let delta =
        gx_substrate::PlannedDelta::new(SubstrateKind::Mcp, payload).expect("a delta mints");

    // 🔴 **The escrow half is a separate declaration, and on this pair it is still blocked.**
    //
    // `crate::invert` performs its prior read **unconditionally**, before it looks at whether the
    // restore template draws on a prior at all. The notion pair's template draws on none — the
    // inverse is a deletion keyed on the created page's own id (`{"do_result": "/id"}`) — so the
    // read is one this declaration has no use for, and on a server with no resource face it is the
    // one thing standing between this pair and an escrow. Declaring a `read_by` does not fix it
    // either: `RestoreSpec::soundness` (DR-46-19, `req/299` item 1) refuses a read face beside a
    // template that draws no prior, correctly — such a read is one gx performs and discards.
    //
    // This lane does not touch it. The invariant DR-46-16 was given is the compare-and-set half,
    // and eliding a read on the escrow road is a different invariant on a road `req/298` has just
    // re-measured (`req/38` §218's one-lane-one-invariant rule). It is registered in `req/308` as
    // a finding for Fable rather than fixed here, and it is held **as a fact** below so the next
    // lane finds it red the moment it changes.
    let escrow_still_blocked = mcp
        .invert(&delta, &pre)
        .expect_err("the escrow half reads a prior this declaration never uses");
    println!("NOTION_TOOLS_ONLY_ESCROW_BLOCKED {escrow_still_blocked}");
    assert!(
        matches!(escrow_still_blocked, gx_substrate::Error::Unreadable { .. }),
        "🔴 and it is blocked fail-closed, in the transport's own words: the CAS half          landing does not quietly let an unescrowed effect through: {escrow_still_blocked:?}"
    );

    let applied = mcp.apply(&delta).expect("the call is made");
    let after = mcp.precondition(&pre).expect("and the world moved");

    let tool_reads = server.tool_reads.lock().expect("not poisoned").clone();
    println!(
        "NOTION_TOOLS_ONLY_ROUND_TRIP pre_digest={:?} post_digest={:?} moved={} \
         resource_reads={} tool_reads={tool_reads:?}",
        pre.digest(),
        applied.resulting_digest(),
        !before
            .cas_eq(&after)
            .expect("one adapter, one scope: comparable"),
        server.reads.load(std::sync::atomic::Ordering::SeqCst)
    );
    assert_eq!(
        server.reads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "🔴 exactly one `resources/read` was attempted in this whole arm, and it is the escrow          half's -- the one this lane did not move. Every compare-and-set read went through the          declared tool, which is the invariant DR-46-16 establishes"
    );
    assert_eq!(
        tool_reads.len(),
        4,
        "🔴 four compare-and-set positions, four round trips, one each: `snapshot`, the first          `precondition`, the post-apply observation, and the second `precondition`. The lane did          not widen `req/38` §195 clause 5's window -- it moved which face each existing read          speaks to, and added none: {tool_reads:?}"
    );
    for (tool, arguments) in &tool_reads {
        assert_eq!(
            tool, "API-retrieve-a-page",
            "the catalogue's tool, every time"
        );
        assert_eq!(
            arguments, r#"{"page_id":"3bd8cc9a-40ed-81b8-b572-f339da4f57ac"}"#,
            "built from the locator's own suffix under the declared prefix"
        );
    }
    assert!(
        !before
            .cas_eq(&after)
            .expect("one adapter, one scope: comparable"),
        "🔴 51 §7 contract 3 and DR-43-1 (a): the fingerprint moved when the page did, which is \
         what makes an undo refusable on this server at all"
    );
}
