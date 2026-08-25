// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-16** (`req/38` §218 ruling 1) — the compare-and-set half on the read-by-tool road.
//!
//! # The invariant this suite holds, in one sentence
//!
//! For every locator `L`: if `L` matches a `$cas_read` prefix, then `snapshot(L)`,
//! `precondition(snapshot(L))` and the post-apply observation all go through that declaration's
//! read tool, and if it matches none, all three go through `resources/read` exactly as they always
//! did — and on **either** road the shape of what comes back (`{digest, Fingerprint}`) and the
//! third-party refusal DR-43-1 (a) builds on it are the same function of the same bytes. A locator
//! with neither face is refused `Unreadable` at plan time, as before: zero regression, and
//! declaration is the only thing that unlocks anything.
//!
//! # What was red before this lane, and where
//!
//! `crates/gx-adapter-mcp/src/adapter.rs` `snapshot`/`precondition` and `apply.rs`'s `observe`
//! called `ToolTransport::read` unconditionally and never consulted `self.catalogue` — which the
//! catalogue's own module doc named as `req/38` §123 ruling 1 (b)'s open ground. Against that
//! source every arm below whose name begins `the_declared_road` fails on an assertion (not on a
//! missing symbol: this file names only symbols that existed before the lane, plus the three the
//! lane adds, and the pre-lane red is recorded in `req/308`).
//!
//! # 🔴 What this suite does **not** measure
//!
//! It does not measure that the read's answer is **about** this object. That predicate is
//! DR-46-15's on the escrow road and **DR-46-21** (`req/305`) on this one, by `req/38` §218 ruling
//! 2's split — one lane, one invariant. The server below is honest by construction, so nothing
//! here would notice a dishonest one, and saying so is the point of saying it.

mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    Admitted, CasArgSource, CasRead, CasTemplate, Catalogue, McpAdapter, ToolCall, ToolTransport,
};
use gx_core::{ReprKind, SubstrateKind};
use gx_substrate::{Error, Result, SubstrateAdapter};

use support::{intent_for, planned};

/// The endpoint every arm points at. A scheme is required (`ASM-69-3`).
const SERVER: &str = "https://mcp.example/dr4616";
/// The prefix a deployment declares, and the object under it.
const PREFIX: &str = "notion://page/";
const PAGE: &str = "notion://page/abc-123";
/// A second object under the same prefix, for the longest-match arm.
const DEEP_PREFIX: &str = "notion://page/deep/";
const DEEP_PAGE: &str = "notion://page/deep/x-9";
/// An object under **no** declared prefix.
const ELSEWHERE: &str = "file:///srv/notes.md";

const BEFORE: &[u8] = b"the page as it was\n";
const AFTER: &[u8] = b"the page as it is now\n";

const READ_TOOL: &str = "API-retrieve-a-page";
/// A **second** read tool, which answers a JSON document about the page rather than its body.
/// The escrow half's `read_by` needs one of these — `ObjectIdentity` reads the answer's `/id` to
/// say which object it was about (DR-46-15) — and the CAS half deliberately does not, because a
/// digest is of the object and not of a document about it.
const ESCROW_READ_TOOL: &str = "API-retrieve-a-page-as-json";
const WRITE_TOOL: &str = "API-patch-page";

fn locator(resource: &str) -> String {
    format!("{SERVER}#{resource}")
}

/// 42 §3.5's equality, which is the one DR-43-1 (a) uses: `Fingerprint` has no `PartialEq` on
/// purpose (**E-M4-15** — "the state moved" and "that comparison has no meaning" are different
/// answers), so every arm below asks `cas_eq` and the `Err` arm is a wiring bug rather than a
/// world that moved.
fn same_state(a: &gx_core::Fingerprint, b: &gx_core::Fingerprint) -> bool {
    a.cas_eq(b)
        .expect("two fingerprints from one adapter over one scope are comparable")
}

// ---------------------------------------------------------------------------
// A server with two faces a deployment can turn on and off independently
// ---------------------------------------------------------------------------

/// An in-process MCP server whose **resource face** and **tool-read face** are separately
/// switchable, which is the only way an arm can tell "the CAS took the declared road" from "the
/// CAS worked".
///
/// Both faces read the same store, so when both are on the two roads answer the same bytes — which
/// is what the bit-identity arm needs — and when one is off the other is the only road there is.
#[derive(Debug)]
struct TwoFacedServer {
    store: Mutex<BTreeMap<String, Vec<u8>>>,
    /// Whether `resources/read` answers at all. `false` is a tools-only server (`req/38` §123:
    /// notion-mcp-server answers `initialize` with `{"tools":{}}`).
    resource_face: bool,
    /// Every `resources/read` that arrived, in order.
    reads: Mutex<Vec<String>>,
    /// Every read-by-tool that arrived, as `(tool, arguments)`.
    tool_reads: Mutex<Vec<(String, String)>>,
    /// Every `tools/call` that arrived.
    calls: Mutex<Vec<String>>,
}

impl TwoFacedServer {
    fn new(resource_face: bool) -> Self {
        let mut store = BTreeMap::new();
        store.insert(PAGE.to_string(), BEFORE.to_vec());
        store.insert(DEEP_PAGE.to_string(), BEFORE.to_vec());
        store.insert(ELSEWHERE.to_string(), BEFORE.to_vec());
        Self {
            store: Mutex::new(store),
            resource_face,
            reads: Mutex::new(Vec::new()),
            tool_reads: Mutex::new(Vec::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn contents(&self, resource: &str) -> Option<Vec<u8>> {
        self.store.lock().expect("the store").get(resource).cloned()
    }

    /// A write that never passed the adapter — `Fixture::disturb`'s job, and `req/182` H-15's
    /// third party.
    fn write_behind_the_adapter(&self, resource: &str, contents: &[u8]) {
        self.store
            .lock()
            .expect("the store")
            .insert(resource.to_string(), contents.to_vec());
    }

    fn reads(&self) -> Vec<String> {
        self.reads.lock().expect("the reads").clone()
    }

    fn tool_reads(&self) -> Vec<(String, String)> {
        self.tool_reads.lock().expect("the tool reads").clone()
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("the calls").clone()
    }

    fn forget(&self) {
        self.reads.lock().expect("the reads").clear();
        self.tool_reads.lock().expect("the tool reads").clear();
        self.calls.lock().expect("the calls").clear();
    }
}

impl ToolTransport for TwoFacedServer {
    fn read(&self, _server: &str, resource: &str) -> Result<Vec<u8>> {
        self.reads
            .lock()
            .expect("the reads")
            .push(resource.to_string());
        if !self.resource_face {
            return Err(Error::Unreadable {
                locator: resource.to_string(),
                detail: "this server declares no `resources` capability (-32601)".to_string(),
            });
        }
        self.contents(resource).ok_or_else(|| Error::Unreadable {
            locator: resource.to_string(),
            detail: "no such resource".to_string(),
        })
    }

    fn read_prior_by_tool(&self, _server: &str, tool: &str, arguments: &[u8]) -> Result<Vec<u8>> {
        let rendered = String::from_utf8_lossy(arguments).to_string();
        self.tool_reads
            .lock()
            .expect("the tool reads")
            .push((tool.to_string(), rendered.clone()));
        if tool != READ_TOOL && tool != ESCROW_READ_TOOL {
            return Err(Error::Unreadable {
                locator: tool.to_string(),
                detail: format!("this server has no tool {tool:?}"),
            });
        }
        // The declaration hands this tool the page id; the server spells the URI back.
        let asked: serde_json::Value =
            serde_json::from_slice(arguments).map_err(|e| Error::Unreadable {
                locator: tool.to_string(),
                detail: format!("the arguments are not JSON: {e}"),
            })?;
        let id = asked
            .get("page_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::Unreadable {
                locator: tool.to_string(),
                detail: "no `page_id` argument".to_string(),
            })?;
        let resource = format!("{PREFIX}{id}");
        let body = self.contents(&resource).ok_or_else(|| Error::Unreadable {
            locator: resource,
            detail: "no such page".to_string(),
        })?;
        if tool == READ_TOOL {
            // The CAS face answers the object's own bytes, which is what a digest is of.
            return Ok(body);
        }
        // The escrow face answers a document *about* the object, carrying the member
        // `ObjectIdentity` binds on.
        Ok(serde_json::to_vec(&serde_json::json!({
            "id": id,
            "text": String::from_utf8_lossy(&body),
        }))
        .expect("a JSON object has a JSON form"))
    }

    fn call(&self, call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        self.calls
            .lock()
            .expect("the calls")
            .push(call.tool().to_string());
        self.write_behind_the_adapter(call.resource(), AFTER);
        Ok(b"{\"ok\":true}".to_vec())
    }
}

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// The declaration a deployment writes for a tools-only page face.
fn page_cas_read() -> CasRead {
    CasRead::new(
        READ_TOOL,
        CasTemplate::new().with("page_id", CasArgSource::ResourceSuffix),
    )
}

fn declared() -> Catalogue {
    Catalogue::new().with_cas_read(PREFIX, page_cas_read())
}

fn adapter(server: &Arc<TwoFacedServer>, catalogue: Catalogue) -> McpAdapter {
    McpAdapter::new(server.clone()).with_catalogue(catalogue)
}

// ---------------------------------------------------------------------------
// item 1 — the three CAS positions take the declared road
// ---------------------------------------------------------------------------

/// 🔴 **The control, and it runs first.** A tools-only server with **no** declaration is refused,
/// which is what `tests/notion_page_catalogue.rs` has measured since v0.4-b and what this lane
/// must not change. Without this arm, every green below is consistent with an adapter that stopped
/// reading anything.
#[test]
fn a_tools_only_object_with_no_declaration_is_still_refused() {
    let server = Arc::new(TwoFacedServer::new(false));
    let mcp = adapter(&server, Catalogue::new());
    let refused = mcp
        .snapshot(&locator(PAGE))
        .expect_err("a server with no resource face and no declaration has no CAS");
    println!(
        "DR4616_CONTROL_UNDECLARED refused={refused} reads={:?} tool_reads={:?}",
        server.reads(),
        server.tool_reads()
    );
    assert!(
        matches!(refused, Error::Unreadable { .. }),
        "the refusal is the transport's own `Unreadable`, unchanged: {refused:?}"
    );
    assert_eq!(
        server.tool_reads().len(),
        0,
        "🔴 an undeclared locator must not reach the read-by-tool road at all — the set of tools \
         this road touches is the set an operator wrote down, and this operator wrote none"
    );
}

/// 🔴 `snapshot` on a tools-only object, through the declared read tool.
///
/// **Red before the lane**: `adapter.rs`'s `snapshot` called `transport.read` unconditionally, so
/// this returned `Err(Unreadable)` — `req/38` §123 ruling 1 (b)'s open ground, exactly as the
/// catalogue module doc named it.
#[test]
fn the_declared_road_gives_a_tools_only_object_a_snapshot() {
    let server = Arc::new(TwoFacedServer::new(false));
    let mcp = adapter(&server, declared());
    let snap = mcp
        .snapshot(&locator(PAGE))
        .expect("🔴 DR-46-16: a declared CAS read face is what makes a tools-only object snapshot");
    println!(
        "DR4616_SNAPSHOT locator={:?} repr={:?} tool_reads={:?} reads={:?}",
        snap.locator(),
        snap.representation(),
        server.tool_reads(),
        server.reads()
    );
    assert_eq!(
        snap.locator(),
        locator(PAGE),
        "the normalised spelling, H-2"
    );
    assert_eq!(
        snap.representation(),
        &ReprKind::Bytes,
        "the road does not change what the adapter claims to have parsed"
    );
    assert_eq!(
        snap.digest(),
        &gx_adapter_mcp::adapter::content_digest(BEFORE),
        "🔴 the digest is `content_digest` of what the declared read answered — the same function \
         the `resources/read` road applies to the same bytes"
    );
    assert_eq!(
        server.tool_reads(),
        vec![(
            READ_TOOL.to_string(),
            "{\"page_id\":\"abc-123\"}".to_string()
        )],
        "🔴 the behavioural bound: the tool name is the catalogue's and the one argument was built \
         from the locator's own suffix"
    );
    assert_eq!(
        server.reads().len(),
        0,
        "🔴 instead of `resources/read`, never in addition to it (`req/38` §195 clause ⑤)"
    );
}

/// 🔴 `precondition` takes the same road, and DR-43-1 (a)'s fingerprint is a value a tools-only
/// server can now produce.
#[test]
fn the_declared_road_gives_a_tools_only_object_a_precondition() {
    let server = Arc::new(TwoFacedServer::new(false));
    let mcp = adapter(&server, declared());
    let snap = mcp.snapshot(&locator(PAGE)).expect("a snapshot");
    server.forget();
    let before = mcp
        .precondition(&snap)
        .expect("🔴 DR-46-16: the compare-and-set needs a fingerprint on this server too");

    server.write_behind_the_adapter(PAGE, AFTER);
    let after = mcp
        .precondition(&snap)
        .expect("and again, after the world moved");
    println!(
        "DR4616_PRECONDITION moved={} tool_reads={:?} reads={:?}",
        !same_state(&before, &after),
        server.tool_reads(),
        server.reads()
    );
    assert!(
        !same_state(&before, &after),
        "🔴 51 §7 contract 3: the fingerprint changes when the state changes. A precondition that \
         did not would make DR-43-1 (a)'s refusal unreachable on this road"
    );
    assert_eq!(
        server.reads().len(),
        0,
        "the precondition went through the declared tool, not `resources/read`"
    );
    assert_eq!(
        server.tool_reads().len(),
        2,
        "one round trip per precondition — the same one the `resources/read` road costs"
    );
}

/// 🔴 The post-apply observation is the third of the three, and it takes the road the other two
/// took.
///
/// **Red before the lane**: `apply.rs`'s `observe` read `resources/read`, which on this server
/// answers `Unreadable`, which `observe` folds to [`absent_digest`] — so the postcondition of a
/// successful call claimed the object was **empty**. That is not a failure anybody would see; it
/// is a signed receipt carrying a wrong postcondition.
#[test]
fn the_declared_road_carries_the_post_apply_observation_too() {
    let server = Arc::new(TwoFacedServer::new(false));
    let mcp = adapter(&server, declared());
    let delta = planned(&mcp, &locator(PAGE), WRITE_TOOL, b"{\"text\":\"x\"}");
    server.forget();
    let applied = mcp.apply(&delta).expect("the call is made");
    println!(
        "DR4616_OBSERVE digest_is_absent={} calls={:?} tool_reads={:?} reads={:?}",
        applied.resulting_digest() == &gx_adapter_mcp::adapter::absent_digest(),
        server.calls(),
        server.tool_reads(),
        server.reads()
    );
    assert_eq!(
        applied.resulting_digest(),
        &gx_adapter_mcp::adapter::content_digest(AFTER),
        "🔴 the observation is of what the object holds now, read back through the declared face. \
         Before this lane it was `absent_digest` — a signed receipt saying an object a call had \
         just written was empty"
    );
    assert_ne!(
        applied.resulting_digest(),
        &gx_adapter_mcp::adapter::absent_digest(),
        "and it is specifically not the absent digest, which is the value the old road produced"
    );
    assert_eq!(server.calls(), vec![WRITE_TOOL.to_string()], "one call");
    assert_eq!(
        server.reads().len(),
        0,
        "the read-back did not go through the face this server does not have"
    );
}

// ---------------------------------------------------------------------------
// item 1 — the two roads are the same function of the same bytes
// ---------------------------------------------------------------------------

/// 🔴 The bit-identity claim, measured on **one** server that has both faces: the same object, read
/// by the two roads, produces the same `ObjectSnapshot` and the same `Fingerprint`.
///
/// This is the arm that makes "the CAS half moved to the read_by road" a statement about the road
/// and not about the values. If a declaration changed the digest, every receipt written before a
/// deployment declared one would stop comparing to every receipt written after.
#[test]
fn both_roads_produce_the_same_snapshot_and_the_same_fingerprint() {
    let server = Arc::new(TwoFacedServer::new(true));
    let through_resources = adapter(&server, Catalogue::new());
    let through_the_tool = adapter(&server, declared());

    let a = through_resources.snapshot(&locator(PAGE)).expect("road 1");
    let b = through_the_tool.snapshot(&locator(PAGE)).expect("road 2");
    let fa = through_resources.precondition(&a).expect("road 1");
    let fb = through_the_tool.precondition(&b).expect("road 2");
    println!(
        "DR4616_BIT_IDENTITY snapshots_equal={} fingerprints_equal={} reads={} tool_reads={}",
        a == b,
        same_state(&fa, &fb),
        server.reads().len(),
        server.tool_reads().len()
    );
    assert_eq!(
        a, b,
        "🔴 the whole snapshot, id included: the id is `cid::compute` of a projection that carries \
         the digest and the locator, so equality here is equality of both"
    );
    assert!(
        same_state(&fa, &fb),
        "🔴 and the fingerprint DR-43-1 (a) compares — same substrate, same scope, same digest"
    );
    assert_eq!(
        fa.scope(),
        fb.scope(),
        "the scope is the locator on both roads"
    );
    assert_eq!(
        server.reads().len(),
        2,
        "road 1 cost one round trip per position"
    );
    assert_eq!(
        server.tool_reads().len(),
        2,
        "🔴 and road 2 cost the same: `req/38` §195's clause ⑤ window is not widened by this lane"
    );
}

/// 🔴 A locator that matches no declared prefix takes `resources/read`, in a catalogue that
/// declares one for a different prefix. The declaration unlocks what it names and nothing else.
#[test]
fn a_locator_under_no_declared_prefix_still_takes_resources_read() {
    let server = Arc::new(TwoFacedServer::new(true));
    let mcp = adapter(&server, declared());
    let snap = mcp
        .snapshot(&locator(ELSEWHERE))
        .expect("this server has a resource face for that one");
    println!(
        "DR4616_UNMATCHED digest_ok={} reads={:?} tool_reads={:?}",
        snap.digest() == &gx_adapter_mcp::adapter::content_digest(BEFORE),
        server.reads(),
        server.tool_reads()
    );
    assert_eq!(
        server.reads(),
        vec![ELSEWHERE.to_string()],
        "the unmatched locator went down the road it always went down"
    );
    assert_eq!(
        server.tool_reads().len(),
        0,
        "and did not touch the declared tool"
    );
}

/// 🔴 When two declared prefixes match, the **longest** wins — and it is a function of the file
/// rather than of the map's iteration order.
#[test]
fn the_longest_matching_prefix_decides() {
    let deep = CasRead::new(
        "API-retrieve-a-deep-page",
        CasTemplate::new().with("page_id", CasArgSource::ResourceSuffix),
    );
    let catalogue = declared().with_cas_read(DEEP_PREFIX, deep);
    let chosen = catalogue.cas_read_for(DEEP_PAGE).expect("one matches");
    let shallow = catalogue
        .cas_read_for(PAGE)
        .expect("the other matches this");
    println!(
        "DR4616_LONGEST deep=({:?},{:?}) shallow=({:?},{:?})",
        chosen.0,
        chosen.1.by_tool(),
        shallow.0,
        shallow.1.by_tool()
    );
    assert_eq!(
        chosen.0, DEEP_PREFIX,
        "both prefixes match; the longer one decides"
    );
    assert_eq!(chosen.1.by_tool(), "API-retrieve-a-deep-page");
    assert_eq!(shallow.0, PREFIX, "and the shallow object is unaffected");
    assert_eq!(shallow.1.by_tool(), READ_TOOL);
    assert_eq!(
        catalogue.cas_reads_declared(),
        2,
        "a report line that cannot make a file with two declarations look like a file with none"
    );
}

/// 🔴 `resource_suffix` is what is left after the prefix that matched, and `resource`/`server`/the
/// constants are what they say — measured member by member rather than through a whole snapshot.
#[test]
fn the_read_arguments_are_built_from_the_locator_and_the_declaration_alone() {
    let template = CasTemplate::new()
        .with("page_id", CasArgSource::ResourceSuffix)
        .with("uri", CasArgSource::Resource)
        .with("endpoint", CasArgSource::Server)
        .with("note", CasArgSource::Const("gx cas".to_string()))
        .with("depth", CasArgSource::ConstJson(serde_json::json!(2)));
    let built = template
        .resolve(SERVER, PAGE, PREFIX)
        .expect("every word of this vocabulary is total over a position");
    let rendered = String::from_utf8(built).expect("JSON is UTF-8");
    println!("DR4616_ARGUMENTS {rendered}");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("JSON");
    assert_eq!(value["page_id"], serde_json::json!("abc-123"));
    assert_eq!(value["uri"], serde_json::json!(PAGE));
    assert_eq!(value["endpoint"], serde_json::json!(SERVER));
    assert_eq!(value["note"], serde_json::json!("gx cas"));
    assert_eq!(
        value["depth"],
        serde_json::json!(2),
        "🔴 DR-V4B-2's reason, one road over: `{{\"const\": \"2\"}}` would send the string and a \
         strict server would refuse the body"
    );
    assert_eq!(
        rendered, "{\"depth\":2,\"endpoint\":\"https://mcp.example/dr4616\",\"note\":\"gx cas\",\"page_id\":\"abc-123\",\"uri\":\"notion://page/abc-123\"}",
        "key order is the BTreeMap's, so two runs of one declaration produce the same bytes"
    );
}

// ---------------------------------------------------------------------------
// item 1 — declaration soundness, judged before a session
// ---------------------------------------------------------------------------

/// 🔴 A `$cas_read` file parses, and the three faults in it are parse errors rather than a session
/// that starts and refuses its first read.
#[test]
fn the_file_form_parses_and_its_three_faults_are_parse_errors() {
    let sound = br#"{
      "$cas_read": {
        "notion://page/": {
          "by_tool": "API-retrieve-a-page",
          "arguments": { "page_id": "resource_suffix" }
        }
      },
      "API-post-page": { "restored_by": "API-delete-a-block",
                         "arguments": { "block_id": { "do_result": "/id" } } }
    }"#;
    let catalogue = Catalogue::from_json(sound).expect("the shipped shape parses");
    println!(
        "DR4616_PARSE cas_reads={} restores={} chosen={:?}",
        catalogue.cas_reads_declared(),
        catalogue.declared(),
        catalogue.cas_read_for(PAGE).map(|(p, r)| (p, r.by_tool()))
    );
    assert_eq!(catalogue.cas_reads_declared(), 1);
    assert_eq!(
        catalogue.declared(),
        1,
        "the reserved slot is metadata, not a tool declaration — the same property `$server` has"
    );

    for (name, bytes, needle) in [
        (
            "an unnamed read tool",
            br#"{"$cas_read": {"notion://page/": {"by_tool": "  "}}}"#.as_slice(),
            gx_adapter_mcp::CAS_READ_TOOL_UNNAMED,
        ),
        (
            "an empty prefix",
            br#"{"$cas_read": {"": {"by_tool": "API-retrieve-a-page"}}}"#.as_slice(),
            gx_adapter_mcp::CAS_READ_PATTERN_EMPTY,
        ),
        (
            "a read face this file also declares as an effect",
            br#"{"$cas_read": {"notion://page/": {"by_tool": "API-patch-page"}},
                 "API-patch-page": {"restored_by": "API-patch-page",
                                    "arguments": {"in_trash": {"const_json": false}}}}"#
                .as_slice(),
            gx_adapter_mcp::CAS_READ_FACE_IS_AN_EFFECT,
        ),
    ] {
        let refused = Catalogue::from_json(bytes)
            .expect_err("🔴 what can be judged without a call is judged before a session");
        println!("DR4616_PARSE_FAULT {name}: {refused}");
        assert!(
            refused.contains(needle),
            "the parse error carries the constant a probe can hold it to: {refused}"
        );
    }

    let misspelt = Catalogue::from_json(
        br#"{"$cas_read": {"notion://page/": {"by_tools": "API-retrieve-a-page"}}}"#,
    )
    .expect_err("deny_unknown_fields");
    println!("DR4616_PARSE_MISSPELT {misspelt}");
    assert!(
        misspelt.contains("by_tools"),
        "a misspelt member is a read the operator did not declare: {misspelt}"
    );
}

/// 🔴 `req/269` M-05's argument on this road: a catalogue built **in code** never met the parser,
/// so `crate::cas` asks the same question again and refuses in the words a declaration fault
/// already has — `DECLARATION_UNSOUND_REFUSAL`, which `gx-cli`'s classifier already wires
/// (`req/291` M-03), so the agent is not told the wiring is broken when the catalogue is wrong.
#[test]
fn a_catalogue_built_in_code_is_judged_at_the_read_rather_than_shipped_past() {
    let server = Arc::new(TwoFacedServer::new(false));
    let unsound = Catalogue::new().with_cas_read(PREFIX, CasRead::new("   ", CasTemplate::new()));
    let mcp = adapter(&server, unsound);
    let refused = mcp
        .snapshot(&locator(PAGE))
        .expect_err("an unsound declaration is not a read that failed");
    let rendered = refused.to_string();
    println!(
        "DR4616_CODE_ROAD {rendered} tool_reads={:?} reads={:?}",
        server.tool_reads(),
        server.reads()
    );
    assert!(
        rendered.contains(gx_adapter_mcp::CAS_READ_TOOL_UNNAMED),
        "the cause comes first: {rendered}"
    );
    assert!(
        rendered.contains(gx_adapter_mcp::DECLARATION_UNSOUND_REFUSAL),
        "🔴 and it carries the constant `gx-cli`'s `declaration_refusal` matches on, so this is \
         recorded as a refusal rather than as a change gx could not describe: {rendered}"
    );
    assert_eq!(
        server.tool_reads().len() + server.reads().len(),
        0,
        "🔴 `req/269` M-05: the declared face was never called and the server did not fail — a \
         remedy naming the server would be one nobody can execute"
    );
}

// ---------------------------------------------------------------------------
// DR-43-1 (a) — the third party, on the declared road
// ---------------------------------------------------------------------------

/// 🔴 The negative control the DoD asks for, at the adapter's own boundary: a change somebody else
/// made behind the adapter moves the precondition, so the fingerprint an undo would be judged
/// against no longer matches — which is the fact `gx undo`'s exit 3 and the HTTP 409
/// `PRECONDITION_CHANGED` row are built on (`crates/gx-cli/tests/undo_cas_e2e.rs` drives the
/// four-adapter symmetry through the CLI; that file is outside this lane's write scope, and
/// `req/308` says so rather than leaving the gap unremarked).
///
/// The arm is a **discrimination**: the same code path answers "unmoved" and "moved" on two
/// worlds, so it cannot pass on an adapter with no CAS at all.
#[test]
fn a_third_party_write_moves_the_precondition_on_the_declared_road() {
    let server = Arc::new(TwoFacedServer::new(false));
    let mcp = adapter(&server, declared());
    let snap = mcp.snapshot(&locator(PAGE)).expect("a snapshot");
    let attested = mcp.precondition(&snap).expect("what the commit attested");

    let unmoved = mcp
        .precondition(&snap)
        .expect("nothing happened in between");
    server.write_behind_the_adapter(PAGE, b"a third party wrote this\n");
    let moved = mcp.precondition(&snap).expect("and now it has");
    println!(
        "DR4616_THIRD_PARTY unmoved_matches={} moved_matches={} contents={:?}",
        same_state(&unmoved, &attested),
        same_state(&moved, &attested),
        server
            .contents(PAGE)
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
    );
    assert!(
        same_state(&unmoved, &attested),
        "the discrimination's other half: on an unmoved world the two agree, so a failure below is \
         the third party and not an adapter that answers a fresh value every time"
    );
    assert!(
        !same_state(&moved, &attested),
        "🔴 DR-43-1 (a): a resource somebody else wrote is not silently overwritten by an undo. \
         Before this lane there was no fingerprint here at all to compare"
    );
}

/// 🔴 And the same object, moved the same way, is refused identically on the two roads: the
/// deployment's choice of read face does not change what a compare-and-set decides.
#[test]
fn the_two_roads_agree_about_a_world_that_moved() {
    let server = Arc::new(TwoFacedServer::new(true));
    let through_resources = adapter(&server, Catalogue::new());
    let through_the_tool = adapter(&server, declared());
    let snap = through_resources
        .snapshot(&locator(PAGE))
        .expect("a snapshot");
    let attested_a = through_resources.precondition(&snap).expect("road 1");
    let attested_b = through_the_tool.precondition(&snap).expect("road 2");

    server.write_behind_the_adapter(PAGE, b"a third party wrote this\n");
    let moved_a = through_resources.precondition(&snap).expect("road 1");
    let moved_b = through_the_tool.precondition(&snap).expect("road 2");
    println!(
        "DR4616_ROADS_AGREE a_moved={} b_moved={} moved_equal={}",
        !same_state(&moved_a, &attested_a),
        !same_state(&moved_b, &attested_b),
        same_state(&moved_a, &moved_b)
    );
    assert!(
        !same_state(&moved_a, &attested_a),
        "road 1 sees the third party"
    );
    assert!(
        !same_state(&moved_b, &attested_b),
        "road 2 sees the third party"
    );
    assert!(
        same_state(&moved_a, &moved_b),
        "🔴 and they see the same thing: the refusal DR-43-1 (a) builds is bit-identical on the \
         two roads, so a deployment that turns a declaration on does not change what its receipts \
         compare to"
    );
}

/// A round trip through the whole seven-method road on a tools-only server: plan, apply, and the
/// escrowed inverse.
///
/// 🔴 Red before the lane at its **first** step — the locator could not be snapshotted, so nothing
/// after it ran. That is `tests/notion_page_catalogue.rs`'s self-admission in one arm.
///
/// # 🔴 The two halves are two declarations, and this lane deliberately did not merge them
///
/// `$cas_read` unlocks the **compare-and-set** half; `read_by` ([`PriorRead`], DR-46-9 A-3)
/// unlocks the **escrow** half. A tools-only deployment writes both, as this fixture does. Making
/// `crate::invert`'s default road fall back to `$cas_read` would let one declaration serve both
/// and was rejected on the spot: `PriorRead` carries a required `ObjectIdentity` (DR-46-15, after
/// `req/269` H-01 measured an undo overwriting a third party's write) and `CasRead` does not until
/// **DR-46-21** lands. A fallback would therefore hand the escrow road a source of bytes that
/// nothing binds to the object — which is DR-46-15's own accident, reintroduced through a
/// convenience. `req/308` records it as a residue; the unification belongs to the lane that gives
/// this road its identity predicate.
#[test]
fn a_tools_only_object_walks_plan_apply_and_invert() {
    let server = Arc::new(TwoFacedServer::new(false));
    let catalogue = declared()
        .with_restore_template(
            WRITE_TOOL,
            WRITE_TOOL,
            gx_adapter_mcp::RestoreTemplate::new()
                .with("uri", gx_adapter_mcp::ArgSource::Forward("uri".to_string()))
                .with(
                    "text",
                    gx_adapter_mcp::ArgSource::PriorJson(gx_adapter_mcp::PriorPointer::Literal(
                        "/text".to_string(),
                    )),
                ),
        )
        // The escrow half's own declaration, with the identity binding DR-46-15 requires of it.
        .with_prior_read(
            WRITE_TOOL,
            gx_adapter_mcp::PriorRead::new(
                ESCROW_READ_TOOL,
                gx_adapter_mcp::RestoreTemplate::new().with(
                    "page_id",
                    gx_adapter_mcp::ArgSource::Const("abc-123".to_string()),
                ),
                gx_adapter_mcp::ObjectIdentity::new(vec![
                    gx_adapter_mcp::IdentityPart::Literal(PREFIX.to_string()),
                    gx_adapter_mcp::IdentityPart::Answer {
                        answer: "/id".to_string(),
                    },
                ]),
            ),
        );
    let mcp = adapter(&server, catalogue);
    let locator = locator(PAGE);
    let pre = mcp.snapshot(&locator).expect("🔴 DR-46-16: the first step");
    let arguments = format!("{{\"uri\":\"{PAGE}\",\"text\":\"x\"}}");
    let delta = mcp
        .plan(
            &intent_for(&locator, WRITE_TOOL, arguments.as_bytes()),
            &pre,
        )
        .expect("plan names no transport call at all");
    let inverse = mcp
        .invert(&delta, &pre)
        .expect("the escrow road")
        .into_inverse()
        .expect("a declared restore with a prior member");
    let applied = mcp.apply(&delta).expect("the call");
    println!(
        "DR4616_ROUND_TRIP pre={:?} post={:?} calls={:?} tool_reads={}",
        pre.digest(),
        applied.resulting_digest(),
        server.calls(),
        server.tool_reads().len()
    );
    assert_eq!(
        pre.digest(),
        &gx_adapter_mcp::adapter::content_digest(BEFORE),
        "the CAS read gave the pre-state, which is the object's own bytes"
    );
    assert_eq!(
        applied.resulting_digest(),
        &gx_adapter_mcp::adapter::content_digest(AFTER),
        "and the observation gave the post-state, through the same face"
    );
    assert_eq!(inverse.substrate(), &SubstrateKind::Mcp);
    let decoded =
        gx_adapter_mcp::McpDelta::decode(inverse.payload()).expect("this crate's own grammar");
    let op = decoded.ops().first().expect("one op");
    let escrowed: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("the restore call's arguments are JSON");
    assert_eq!(
        escrowed["text"],
        serde_json::json!(String::from_utf8_lossy(BEFORE).to_string()),
        "🔴 the escrow carries the prior, and the CAS read established the same object's pre-state          — the two halves are on one server and about one object"
    );
    assert_eq!(
        server.reads().len(),
        0,
        "🔴 neither half touched `resources/read`, which this server does not have"
    );
}

/// 🔴 The non-merge, held as a fact rather than as a comment: a `$cas_read` declaration does
/// **not** unlock the escrow half. A deployment that declares only the CAS read still gets
/// `Unreadable` out of `invert`, in the transport's own words.
///
/// This arm exists so that a later lane that "helpfully" makes `crate::invert` fall back to
/// `$cas_read` makes this red and has to argue with DR-46-15 in the open. Until **DR-46-21** binds
/// a CAS read's answer to its object, escrowed bytes may not come from this road.
#[test]
fn a_cas_read_declaration_does_not_unlock_the_escrow_half() {
    let server = Arc::new(TwoFacedServer::new(false));
    let catalogue = declared().with_restore_template(
        WRITE_TOOL,
        WRITE_TOOL,
        gx_adapter_mcp::RestoreTemplate::new()
            .with("uri", gx_adapter_mcp::ArgSource::Forward("uri".to_string()))
            .with("text", gx_adapter_mcp::ArgSource::PriorContentsUtf8),
    );
    let mcp = adapter(&server, catalogue);
    let locator = locator(PAGE);
    let pre = mcp.snapshot(&locator).expect("the CAS half is declared");
    let arguments = format!("{{\"uri\":\"{PAGE}\",\"text\":\"x\"}}");
    let delta = mcp
        .plan(
            &intent_for(&locator, WRITE_TOOL, arguments.as_bytes()),
            &pre,
        )
        .expect("plan");
    let refused = mcp
        .invert(&delta, &pre)
        .expect_err("🔴 the escrow half needs its own `read_by`, with its own `ObjectIdentity`");
    println!("DR4616_NO_MERGE snapshot_ok=true invert={refused}");
    assert!(
        matches!(refused, Error::Unreadable { .. }),
        "and it is refused fail-closed, which is DR-46-12's default: {refused:?}"
    );
}
