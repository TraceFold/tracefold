// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-MCP1 / P1** (`req/265` §4, `req/38` §347 ruling 1, report `req/602`) — the eleven
//! flag-gated github write tools, driven **through a real wire against a real process**.
//!
//! # Why this suite exists beside P0's, rather than inside it
//!
//! `gx-adapter-mcp/tests/github16_read_by_tool.rs` is P0: the four tools a flag-free
//! github-mcp-server publishes, measured against an **in-process** transport. That is the right
//! fixture for a crate that ships a boundary, and it cannot answer the two questions this lane was
//! given. "The forward call never reached the server" is, in an in-process fixture, the test
//! binary reporting on itself; `req/119` §5 A-7 asks for the **server's own** count, on the other
//! side of a pipe. So P1's probes drive `gx_mcp_wire::WireTransport` over
//! `gx-mcp-wire/tests/bin/mcp_probe_server.rs` in its **github face**
//! (`GX_PROBE_GITHUB=1`), with strict argument validation on, and read the arrival log the server
//! itself appends to.
//!
//! **`GX_PROBE_STRICT_ARGS` is on for every probe here and that is not decoration.** `req/152` §5
//! recorded the A2 filing's finding: the mock's undo "success" was the mock not validating its
//! arguments. [`g11_the_strict_face_refuses_a_member_the_tool_does_not_declare`] is the negative
//! control that the validation is really running, so that every green below is a green against a
//! server that would have refused a wrong call.
//!
//! # 🔴 What P1 found, and it is not what `req/265` §2 predicted
//!
//! `req/265` §2 read the fifteen tools off their input schemas and concluded that once a read face
//! existed, eleven of them would be declarable and the residue would be *server-side* — a missing
//! precondition, a merged pull request, a silent drop. Measured, the residue is mostly **on this
//! side of the wire**, and it has one name:
//!
//! > **A REST API renders an object in a different shape than its setters accept, and RFC 6901
//! > selects — it does not project.**
//!
//! `update_issue_body` is declarable because an issue's `/body` *is* the string
//! `update_issue_body` takes. `update_issue_labels` is not, because the issue renders
//! `[{"id":…,"name":"bug"}]` and the setter takes `["bug"]`: no word in [`ArgSource`] maps a
//! collection of objects onto a collection of their members, and inventing one is a vocabulary
//! change nobody has ruled on. The same shape decides `assignees`. It does **not** decide
//! `milestone` and `type`, because there the wanted value is a single member of a single rendered
//! object (`/milestone/number`, `/type/name`) and a pointer reaches exactly that.
//!
//! So the class table `req/265` §3-3 ships gains a row this lane names **E-proj**, and it is the
//! one that decides most of P1. The whole census, with the mechanism beside each verdict, is
//! [`g10_the_undeclarable_four_are_absent_and_each_absence_has_a_mechanism`] — held against the
//! fixture file, so a later hand that "fills in the missing rows" turns it red.
//!
//! # The verdicts this lane declares (eleven tools; `create_or_update_file` and `update_gist` are P0's)
//!
//! | tool | verdict | mechanism |
//! |---|---|---|
//! | `update_issue_body` | **true** | `/body` is the value the setter takes |
//! | `update_issue_title` | **true** | `/title` likewise |
//! | `update_issue_type` | **true** | `/type/name`; and the setter's `anyOf: [string, null]` can express *removal*, so the round trip is whole in both directions |
//! | `update_issue_milestone` | **true** for a call whose prior had one, `Ok(None)` otherwise | `/milestone/number`; the setter is `integer, minimum: 1` and has **no** spelling for removal — the same operation `update_issue_type` can express and this one cannot. The boundary is enforced by the pointer failing to resolve, not by a promise |
//! | `update_pull_request_body` | **true** | as `update_issue_body` |
//! | `update_pull_request_title` | **true** | as `update_issue_title` |
//! | `update_pull_request_state` | **true** | `/state` is a plain string. A merged pull request refuses to reopen — **loudly**, `422` from the server — and a loud refusal is the failure mode `create_or_update_file` already ships with (`req/265` §2-1 boundary ①) |
//! | `update_issue_labels` | **false** — *E-proj* | rendered as objects, set as strings |
//! | `update_issue_assignees` | **false** — *E-proj* | likewise, and the API additionally drops an un-assignable login in silence |
//! | `update_issue_state` | **unknown** — *S* | the undo **succeeds** and `state_reason` is not back: closing carries a reason, reopening cannot restore one. A silent partial, and `req/38` §347 ruling 1 did **not** adopt partial-inverse declarations (DR-46-36, parked) |
//! | `update_pull_request_draft_state` | **unknown** — *S* | ready→draft clears the requested reviewers and draft→ready does not put them back. Silent partial again |
//!
//! 🔴 **The line between `true` and `unknown` here is a criterion, and it is worth stating because
//! `req/265` §2 did not have it**: a boundary that makes the undo **fail loudly** leaves the
//! verdict `true` (the object is either back or the operator is told it is not); a boundary that
//! lets the undo **succeed while the object is not back** destroys it. `update_pull_request_state`
//! and `update_issue_state` differ on exactly that and on nothing else.
//!
//! # What is **not** measured here (`req/265` §6's discipline, continued)
//!
//! * **Zero live calls against `api.github.com`.** The eleven schemas were transcribed from
//!   upstream's own `__toolsnaps__` at `v1.9.0` (a read of a public repository's file contents,
//!   the same primary source and the same method `req/265` §1 used). The **response** shapes —
//!   that an issue renders `labels` as objects, `milestone` as an object, `type` as an object —
//!   are read off GitHub's published REST schema and are **not** measured. If any of them is
//!   wrong, the verdict for that tool moves, and the fixture is where it would be corrected.
//! * **The feature flags were never toggled on a running server.** `req/196` asked for that to be
//!   probed first; it cannot be, without a live call. What is established is that the eleven tools
//!   exist in upstream's snapshots at `v1.9.0` and that eleven of the fifteen sit behind
//!   `issues_granular` / `pull_requests_granular`, so a default deployment has **four**.
//! * **The `$cas_read` road is not exercised, because for github it is not expressible.** See
//!   [`g6_a_third_party_change_refuses_the_undo`] for the gap and what this suite does instead.
//! * **No rate limit, no 1 MiB escrow ceiling, no secondary limits.**
//! * **`is_suggestion` is not driven.** A call whose effect the API decides is a third thing this
//!   vocabulary has no word for, and guessing at it here would be worse than leaving it named.
//!
//! [`ArgSource`]: gx_adapter_mcp::ArgSource

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

#![allow(clippy::too_many_lines)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gx_adapter_mcp::{CasArgSource, CasTemplate, Catalogue, McpAdapter, Reversibility};
use gx_core::{Actor, SubstrateKind, Timestamp, TransformationId, VerdictKind};
use gx_engine::{Engine, HumanRuling, InjectedEvidence, InverseStatus, Lifecycle, UndoWitness};
use gx_mcp_wire::{StdioClient, WireTransport};
use gx_substrate::SubstrateAdapter;
use serde_json::{json, Value};

const AT: Timestamp = Timestamp(1_756_000_000_000_000_000);
const UNDO_AT: Timestamp = Timestamp(1_756_000_100_000_000_000);

const P1_CATALOGUE: &str =
    include_str!("../../gx-adapter-mcp/tests/fixtures/github16-p1-catalogue.json");

/// The issue this suite drives, as this adapter's resource URI.
const ISSUE: &str = "github://octo/demo/issues/1";
/// The pull request.
const PULL: &str = "github://octo/demo/pulls/7";

const BODY_BEFORE: &str = "the body before";
const TITLE_BEFORE: &str = "the title before";

// ===========================================================================
// the fixture: a real server, in a real process, in its github face
// ===========================================================================

/// A running probe server in github mode, the file its world lives in, and its arrival log.
struct Probe {
    client: Arc<StdioClient>,
    endpoint: String,
    dir: PathBuf,
    state: PathBuf,
    log: PathBuf,
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.client.shutdown();
    }
}

/// One line of the server's own arrival log: `read` for the two read tools, `call` for the eleven
/// writes. The split is the server's, written before the tool runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Arrival {
    kind: String,
    tool: String,
    subject: String,
}

impl Probe {
    /// 🔴 **A-7** — the arrivals as the **server** recorded them, never as this file counted them.
    fn arrivals(&self) -> Vec<Arrival> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let mut parts = line.split('\t');
                Arrival {
                    kind: parts.next().unwrap_or_default().to_string(),
                    tool: parts.next().unwrap_or_default().to_string(),
                    subject: parts.next().unwrap_or_default().to_string(),
                }
            })
            .collect()
    }

    fn writes(&self) -> Vec<Arrival> {
        self.arrivals()
            .into_iter()
            .filter(|a| a.kind == "call")
            .collect()
    }

    fn reads(&self) -> Vec<Arrival> {
        self.arrivals()
            .into_iter()
            .filter(|a| a.kind == "read")
            .collect()
    }

    fn clear_log(&self) {
        let _ = std::fs::remove_file(&self.log);
    }

    fn world(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(&self.state).expect("the world file reads"))
            .expect("the world file is JSON")
    }

    /// A member of the issue, as the server holds it right now.
    fn issue(&self, pointer: &str) -> Value {
        self.world()
            .pointer(&format!("/issues/octo~1demo~11{pointer}"))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn pull(&self, pointer: &str) -> Value {
        self.world()
            .pointer(&format!("/pulls/octo~1demo~17{pointer}"))
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// 🔴 **family 2's third party** — a writer that is not this session, editing the world **behind
    /// the server's back**. That it goes to the file rather than through a tool is the point: a
    /// change this session made would be a change this session's compare-and-set already knows
    /// about.
    fn third_party_sets(&self, collection: &str, key: &str, member: &str, value: Value) {
        let mut world = self.world();
        world
            .get_mut(collection)
            .and_then(|it| it.get_mut(key))
            .and_then(Value::as_object_mut)
            .expect("the object is in the world")
            .insert(member.to_string(), value);
        std::fs::write(
            &self.state,
            serde_json::to_string_pretty(&world).expect("serialize"),
        )
        .expect("the third party writes");
    }

    fn locator(&self, resource: &str) -> String {
        format!("{}#{resource}", self.endpoint)
    }
}

fn seed_world() -> Value {
    json!({
        "issues": {
            "octo/demo/1": {
                "id": 1001,
                "number": 1,
                "url": "https://api.github.com/repos/octo/demo/issues/1",
                "title": TITLE_BEFORE,
                "body": BODY_BEFORE,
                "state": "open",
                "state_reason": null,
                // Rendered as objects, which is the whole of the E-proj finding.
                "type": { "id": 42, "name": "Bug" },
                "labels": [{ "id": 100, "name": "bug", "default": false }],
                "assignees": [{ "id": 200, "login": "octocat" }],
                "milestone": { "number": 5, "title": "a milestone" },
            }
        },
        "pulls": {
            "octo/demo/7": {
                "id": 2007,
                "number": 7,
                "url": "https://api.github.com/repos/octo/demo/pulls/7",
                "title": "pr title before",
                "body": "pr body before",
                "state": "open",
                "draft": false,
                "merged": false,
                "requested_reviewers": [{ "login": "reviewer" }],
            }
        }
    })
}

/// Which read face the server should refuse on. family 3's dial, and nothing else uses it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadFailure {
    /// Both faces answer.
    None,
    /// 🔴 Only the declared read **tools** refuse. `resources/read` still answers, so `snapshot`
    /// (41 §4's precondition read) succeeds and the pipeline gets as far as T-10b — which is where
    /// the escrow read lives and the only place D-1 is about.
    ///
    /// This distinction is a correction R-MCP1 made to itself. The first version of this suite
    /// broke both faces at once, and the refusal it recorded came out of `snapshot`, one stage
    /// **before** any escrow was contemplated. The probe was green and was measuring the wrong
    /// denial.
    Tool,
    /// Both faces refuse — a server with no read face at all, which is a real deployment shape and
    /// a different measurement.
    All,
}

impl ReadFailure {
    fn as_env(self) -> Option<&'static str> {
        match self {
            ReadFailure::None => None,
            ReadFailure::Tool => Some("tool"),
            ReadFailure::All => Some("all"),
        }
    }
}

/// Start the server in github mode.
fn spawn(name: &str, read_failure: ReadFailure) -> Probe {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("rmcp1")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");

    let log = dir.join("arrivals.log");
    let state = dir.join("world.json");
    std::fs::write(
        &state,
        serde_json::to_string_pretty(&seed_world()).expect("serialize the seed"),
    )
    .expect("seed the world");

    let command = probe_server_path().display().to_string();
    let mut env = vec![
        ("GX_PROBE_LOG".to_string(), log.display().to_string()),
        ("GX_PROBE_GITHUB".to_string(), "1".to_string()),
        (
            "GX_PROBE_GITHUB_STATE".to_string(),
            state.display().to_string(),
        ),
        // 🔴 Always on. See the module header: a lenient server makes every undo "succeed".
        ("GX_PROBE_STRICT_ARGS".to_string(), "1".to_string()),
    ];
    if let Some(mode) = read_failure.as_env() {
        env.push(("GX_PROBE_READ_FAIL".to_string(), mode.to_string()));
    }
    let client = Arc::new(
        StdioClient::spawn_with_env(&command, &[], &env).expect("the probe server starts"),
    );
    client.initialize().expect("the handshake agrees");
    let endpoint = gx_mcp_wire::stdio_endpoint(&command);
    Probe {
        client,
        endpoint,
        dir,
        state,
        log,
    }
}

/// The probe binary `cargo test` built beside this one.
fn probe_server_path() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_BIN_EXE_gx"));
    dir.pop();
    let exe = if cfg!(windows) {
        "mcp_probe_server.exe"
    } else {
        "mcp_probe_server"
    };
    let direct = dir.join(exe);
    if direct.exists() {
        return direct;
    }
    // `cargo test -p gx-cli` puts gx in `deps/`'s parent; the probe bin is a sibling either way.
    dir.join("deps").join(exe)
}

/// The engine, the adapter and the catalogue, wired the way `gx wrap` wires them.
struct Wired {
    engine: Engine<InjectedEvidence>,
    adapter: McpAdapter,
}

fn wire(probe: &Probe, name: &str, catalogue: &str) -> Wired {
    let transport = Arc::new(WireTransport::new(
        probe.client.clone(),
        probe.endpoint.clone(),
    ));
    let adapter = McpAdapter::new(transport).with_catalogue(
        Catalogue::from_json(catalogue.as_bytes()).expect("the P1 catalogue parses"),
    );
    // 🔴 The **shipped** pack, not a permit-all fixture: part of what this suite establishes is
    // that a `github://` locator is admitted by the policy set a deployment actually gets.
    let gate = gx_gate::Gate::with_policies(
        gx_gate::packs::mcp_pack().expect("the shipped mcp pack parses"),
    );
    let journal = probe.dir.join(format!("{name}-journal.bin"));
    let mut engine =
        Engine::open(journal, gate, InjectedEvidence::none()).expect("a fresh journal");
    engine.register_adapter(Arc::new(adapter.clone()), "gx-adapter-mcp rmcp1 p1");
    Wired { engine, adapter }
}

fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-rmcp1-p1", &[23u8; 32])
}

fn intent_for(locator: &str, tool: &str, arguments: &Value) -> gx_core::Intent {
    let bytes = serde_json::to_vec(arguments).expect("arguments have a JSON form");
    let goal = gx_adapter_mcp::ToolIntent::new(tool, bytes)
        .encode()
        .expect("a tool call has a canonical form");
    gx_core::Intent::new(
        SubstrateKind::Mcp,
        locator.to_string(),
        gx_core::GoalBytes(goal),
        gx_core::ChangeContext::Policy,
        Actor::Agent {
            key: "rmcp1-p1-lane-key".to_string(),
            model: "rmcp1-p1 (R-MCP1, req/602)".to_string(),
        },
    )
}

fn commit(
    wired: &mut Wired,
    locator: &str,
    tool: &str,
    arguments: &Value,
) -> Result<TransformationId, String> {
    let intent = intent_for(locator, tool, arguments);
    wired
        .engine
        .submit(&intent, 42, AT)
        .map_err(|e| format!("submit: {e}"))?;
    let id = wired
        .engine
        .plan(&intent, AT)
        .map_err(|e| format!("plan: {e}"))?;
    let state = wired
        .engine
        .verify(&id, AT, &signing_key(), None)
        .map_err(|e| format!("verify: {e}"))?;
    if state != Lifecycle::Admitted {
        return Err(format!("verify landed on {state:?}"));
    }
    wired
        .engine
        .canonicalize(&id, AT, None)
        .map_err(|e| format!("canonicalize: {e}"))?;
    match wired.engine.commit(&id, AT, &signing_key()) {
        Ok(Lifecycle::Committed) => Ok(id),
        Ok(other) => Err(format!("commit landed on {other:?}")),
        Err(e) => Err(format!("commit: {e}")),
    }
}

fn undo(wired: &mut Wired, id: &TransformationId) -> Result<(), String> {
    let witness = wired.engine.attested_postcondition(id);
    if !matches!(witness, UndoWitness::Attested(_)) {
        return Err(format!("no attested postcondition: {witness:?}"));
    }
    let (_, undoing) = wired
        .engine
        .undo(id, &witness, 43, UNDO_AT)
        .map_err(|e| format!("undo: {e}"))?;
    let state = wired
        .engine
        .verify(&undoing, UNDO_AT, &signing_key(), None)
        .map_err(|e| format!("undo verify: {e}"))?;
    if state != Lifecycle::Admitted {
        return Err(format!("the undo candidate landed on {state:?}"));
    }
    wired
        .engine
        .canonicalize(&undoing, UNDO_AT, None)
        .map_err(|e| format!("undo canonicalize: {e}"))?;
    match wired.engine.commit(&undoing, UNDO_AT, &signing_key()) {
        Ok(Lifecycle::Committed) => Ok(()),
        Ok(other) => Err(format!("the undo commit landed on {other:?}")),
        Err(e) => Err(format!("undo commit: {e}")),
    }
}

fn issue_args(member: &str, value: Value) -> Value {
    json!({ "owner": "octo", "repo": "demo", "issue_number": 1, member: value })
}

fn pull_args(member: &str, value: Value) -> Value {
    json!({ "owner": "octo", "repo": "demo", "pullNumber": 7, member: value })
}

// ===========================================================================
// family 1 — the round trip, byte for byte, across a real wire
// ===========================================================================

/// 🔴 **G-1** (`req/265` §4-1 `github16_escrow_read_before_apply`) — every escrow read **arrives
/// before** the effect, and the server is the one that says so.
///
/// **E-M4-30**'s physics: the escrow is built before `apply` (43 T-10b), so a read after the
/// forward `tools/call` would be a read of a prior that no longer exists. The count is **two**,
/// for the reason P0's P-1 established and this lane re-measures one crate further out:
/// `adapter.invert` runs at T-3 (to fold `invert_available` into the gate, E-M4-5) and again at
/// T-10b. Two reads per guarded forward call is a rate-limit fact `docs/LIMITS.md` v0.5-d carries.
#[test]
fn g1_every_escrow_read_arrives_before_the_effect() {
    let probe = spawn("g1", ReadFailure::None);
    let mut wired = wire(&probe, "g1", P1_CATALOGUE);
    probe.clear_log();

    let id = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("the body after")),
    )
    .expect("the issue body pair commits");

    let arrivals = probe.arrivals();
    let first_write = arrivals
        .iter()
        .position(|a| a.kind == "call")
        .expect("the effect reached the server");
    let reads: Vec<usize> = arrivals
        .iter()
        .enumerate()
        .filter(|(_, a)| a.kind == "read")
        .map(|(i, _)| i)
        .collect();
    println!("RMCP1_G1 id={id:?} arrivals={arrivals:?} first_write={first_write} reads={reads:?}");

    assert_eq!(
        reads.len(),
        2,
        "the number of escrow reads per guarded forward call moved. Two is T-3 (the gate's \
         `invert_available`) plus T-10b (the escrow); three would mean a new `invert` call site \
         and one would mean the gate stopped being told. Arrivals: {arrivals:?}"
    );
    assert!(
        reads.iter().all(|i| *i < first_write),
        "a prior was read after the effect ran, which is a prior that no longer exists \
         (E-M4-30 / 43 T-10b). Arrivals: {arrivals:?}"
    );
    // 🔴 **L-6-a** (`req/621` §5 / §336, `req/38` §394): the assertion that stood here —
    // `arrivals.iter().all(|a| a.tool != "resources/read")` — was unfalsifiable. `resources/read`
    // is a JSON-RPC **method**, not a tool name, and the `tool` column of this log only ever holds
    // tool names (the server writes `issue_read` for a read and a write tool's name for a call), so
    // the inequality was structurally always true and measured nothing. The claim that carries the
    // finding on a **different axis** is the positive one: every escrow read here went through the
    // tool the catalogue declares, so a read escaping to some other tool is caught.
    let escrow_reads = probe.reads();
    assert!(
        !escrow_reads.is_empty() && escrow_reads.iter().all(|a| a.tool == "issue_read"),
        "an escrow read reached the server as a tool other than the declared `issue_read` (or none \
         arrived at all). `resources/read` is a JSON-RPC method, not a tool name, so its absence \
         from this column was never in question (`req/621` §5 L-6-a); the falsifiable claim is that \
         every escrow read went through the tool the catalogue names: {arrivals:?}"
    );
    assert_eq!(
        escrow_reads.first().map(|a| a.tool.clone()),
        Some("issue_read".to_string()),
        "the tool the escrow reached is the one the **catalogue** names, which is the whole \
         behavioural bound AC-051 D-6 rests on: {arrivals:?}"
    );
}

/// 🔴 **G-2** (`req/265` §4-1 `github16_body_roundtrip_bytes`) — forward, undo, and the issue body
/// holds the **same bytes** it held, measured in the file the server writes.
#[test]
fn g2_the_issue_body_round_trip_is_byte_identical() {
    let probe = spawn("g2", ReadFailure::None);
    let mut wired = wire(&probe, "g2", P1_CATALOGUE);

    let before = probe.issue("/body");
    let id = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("the body after\nwith a trailing space \n")),
    )
    .expect("commit");
    let moved = probe.issue("/body");
    undo(&mut wired, &id).expect("undo");
    let after = probe.issue("/body");

    println!("RMCP1_G2 before={before} moved={moved} after={after}");
    assert_ne!(
        before, moved,
        "sanity: the forward call actually changed it"
    );
    assert_eq!(
        before, after,
        "the round trip is not byte-identical, which is the one thing an escrow exists to make true"
    );
}

/// 🔴 **G-3** (`req/265` §4-1 `github16_cjk_body_roundtrip`) — Japanese, an astral emoji and a
/// combining sequence survive the trip through `prior_json`'s UTF-8 road.
///
/// The bytes matter and not the code points: a normalisation anywhere on the road (the JSON
/// encoder, the canonical form the escrow is stored in, the server's own round trip) would show up
/// here as a body that "looks the same" and is not.
#[test]
fn g3_the_round_trip_survives_cjk_an_astral_emoji_and_a_combining_sequence() {
    let probe = spawn("g3", ReadFailure::None);
    let mut wired = wire(&probe, "g3", P1_CATALOGUE);

    // Written as code points so that **this file** measures as zero under the repository's
    // own CJK rule -- `probes/doubt/tests/cjk_doubt.rs` does exactly this to itself, and for
    // the same reason. Reading left to right: six Japanese characters, a space, an
    // astral-plane emoji joined by a ZWJ, a space, then `ka` + a combining voiced mark + a
    // combining acute + a zero-width space + two more Japanese characters. The combining run
    // is the part a normalising road folds into fewer code points, which is the failure this
    // probe is looking for.
    let awkward = concat!(
        "\u{65E5}\u{672C}\u{8A9E}\u{306E}\u{672C}\u{6587} ",
        "\u{1F9D1}\u{200D}\u{1F680} ",
        "\u{304B}\u{3099}\u{0301}\u{200B}\u{672B}\u{5C3E}",
    );
    probe.third_party_sets("issues", "octo/demo/1", "body", json!(awkward));

    let before = probe.issue("/body");
    assert_eq!(before, json!(awkward), "the seed took");

    let id = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("plain ascii replacement")),
    )
    .expect("commit");
    undo(&mut wired, &id).expect("undo");
    let after = probe.issue("/body");

    let after_text = after.as_str().unwrap_or_default();
    println!(
        "RMCP1_G3 before_bytes={} after_bytes={} equal={}",
        awkward.len(),
        after_text.len(),
        after_text == awkward
    );
    assert_eq!(
        after_text.as_bytes(),
        awkward.as_bytes(),
        "the round trip changed the bytes. Equal-looking text is not equal text; a normalisation \
         on this road would make an undo report success over a body nobody wrote"
    );
}

/// 🔴 **G-4** — the pull-request half of the same road, so that "it works" is not a statement
/// about one object class and one read tool.
///
/// `pull_request_read` is a **different** declared read tool with a different required member
/// (`pullNumber`), and the identity it is bound to spells a different URI shape. A road that only
/// ever ran against `issue_read` would have left the declaration vocabulary's generality asserted.
#[test]
fn g4_the_pull_request_title_round_trip_uses_a_second_read_tool() {
    let probe = spawn("g4", ReadFailure::None);
    let mut wired = wire(&probe, "g4", P1_CATALOGUE);
    probe.clear_log();

    let before = probe.pull("/title");
    let id = commit(
        &mut wired,
        &probe.locator(PULL),
        "update_pull_request_title",
        &pull_args("title", json!("pr title after")),
    )
    .expect("commit");
    undo(&mut wired, &id).expect("undo");
    let after = probe.pull("/title");

    let read_tools: Vec<String> = probe.reads().into_iter().map(|a| a.tool).collect();
    println!("RMCP1_G4 before={before} after={after} read_tools={read_tools:?}");
    assert_eq!(
        before, after,
        "the pull-request title round trip is not whole"
    );
    assert!(
        read_tools.iter().all(|t| t == "pull_request_read"),
        "the second declared read tool is not the one that ran: {read_tools:?}"
    );
}

/// 🔴 **G-5** — `prior_json` reaches **one member of a rendered object**, and the two tools that
/// need it part company on whether their setter can express *removal*.
///
/// This is the probe that carries the lane's finding. `update_issue_type` and
/// `update_issue_milestone` are the same kind of operation over the same kind of rendered value
/// (`{"name": …}`, `{"number": …}`), and:
///
/// * `/type/name` restores, and `issue_type`'s `anyOf: [string, null]` means the *removal* is
///   restorable too;
/// * `/milestone/number` restores, and `milestone`'s `integer, minimum: 1` means a prior of `null`
///   has **no spelling** — the pointer does not resolve, `invert` answers `Ok(None)`, and the
///   receipt says so instead of the undo quietly doing something else.
///
/// **Reversibility is decided by whether the setter can say "nothing".** That sentence is
/// `req/265`'s "Undo is a feature, reversibility is a property" with a mechanism under it, and it
/// is the ORG-facing finding of this lane.
#[test]
fn g5_prior_json_reaches_a_rendered_member_and_removal_is_the_dividing_line() {
    let probe = spawn("g5", ReadFailure::None);
    let mut wired = wire(&probe, "g5", P1_CATALOGUE);

    // --- the arm that restores: a prior type exists, and it comes back ---
    let before_type = probe.issue("/type/name");
    let id = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_type",
        &issue_args("issue_type", json!("Task")),
    )
    .expect("commit the type change");
    undo(&mut wired, &id).expect("undo the type change");
    let after_type = probe.issue("/type/name");

    // --- the arm that refuses: the prior milestone is absent, so there is nothing to point at ---
    probe.third_party_sets("issues", "octo/demo/1", "milestone", Value::Null);
    let locator = probe.locator(ISSUE);
    let arguments = issue_args("milestone", json!(9));
    let intent = intent_for(&locator, "update_issue_milestone", &arguments);
    let pre = wired
        .adapter
        .snapshot(&locator)
        .expect("the fixture answers for the issue");
    let delta = wired
        .adapter
        .plan(&intent, &pre)
        .expect("a well-formed call plans");
    let verdict = wired
        .adapter
        .reversibility(&delta, &pre)
        .expect("the declaration is sound; the call is the thing it does not cover");

    println!(
        "RMCP1_G5 before_type={before_type} after_type={after_type} \
         milestone_prior=null verdict={}",
        verdict.as_str()
    );
    assert_eq!(
        before_type, after_type,
        "`/type/name` did not put the rendered member back"
    );
    assert_eq!(
        verdict,
        Reversibility::False,
        "a prior with no milestone has no `milestone` the setter would accept (`integer, \
         minimum: 1` has no spelling for removal), so the honest answer is that no inverse can be \
         built for **this call** -- not that one was built and will quietly do something else"
    );
}

// ===========================================================================
// family 2 — a precondition that moved refuses the undo
// ===========================================================================

/// 🔴 **G-6** (`req/265` §4-2 `github16_third_party_change_refuses`) — **the probe this lane must
/// not fail.**
///
/// Between the escrow and the undo, somebody who is not this session writes the issue. The undo
/// must **refuse**, because an undo that ran here would overwrite a change gx never saw — the
/// single worst thing this product can do, and the one `req/265` §4-2 names as such.
///
/// # 🔴 The road this probe does **not** take, and why that is a finding rather than a shortcut
///
/// The faithful github shape has **no** resource face for an issue, so the compare-and-set half
/// would have to go through a declared `$cas_read` (DR-46-16). It cannot: `CasArgSource`'s four
/// words (`const`, `const_json`, `resource`, `resource_suffix`, `server`) produce a JSON **string**
/// for anything that varies per locator, and `issue_read` requires `issue_number` to be a
/// **number** (`minimum: 1`). A per-locator numeric argument has no word in the shipped
/// vocabulary. `const_json` can carry a number but is fixed at declaration time, so it would pin
/// one issue per declaration.
///
/// So this fixture publishes a `github://…` resource face and the CAS goes through
/// `resources/read` — the same declared divergence P0 made for the gist, for the same reason and
/// named in the same place (`gx-mcp-wire/tests/bin/github_face.rs`'s header). **What is measured
/// is the refusal; what is not measured is the refusal on a server shaped exactly like github's.**
/// Closing that needs a `cas_read` word that can carry a per-locator number, which is a DR
/// candidate `req/602` raises and this lane does not decide.
#[test]
fn g6_a_third_party_change_refuses_the_undo() {
    let probe = spawn("g6", ReadFailure::None);
    let mut wired = wire(&probe, "g6", P1_CATALOGUE);

    let id = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("this session wrote this")),
    )
    .expect("commit");

    // A third party, through the file, behind the server's back.
    probe.third_party_sets(
        "issues",
        "octo/demo/1",
        "body",
        json!("somebody else wrote this afterwards"),
    );
    let trespass = probe.issue("/body");

    let refused = undo(&mut wired, &id);
    let after = probe.issue("/body");

    println!("RMCP1_G6 trespass={trespass} refused={refused:?} after={after}");
    let detail = refused.as_ref().err().cloned().unwrap_or_default();
    assert!(
        refused.is_err(),
        "🔴 the undo ran over a change gx never saw. This is the failure `req/265` §4-2 calls the \
         one that must not happen: a compensating call assembled from a prior that stopped being \
         the prior"
    );
    assert!(
        detail.contains("DR-43-1"),
        "the undo was refused, but not by the mechanism this probe is about. `DR-43-1` is the \
         name of \"an undo does not overwrite a change it cannot account for\"; a refusal from \
         anywhere else would make this probe green for a reason nobody chose: {detail}"
    );
    assert_eq!(
        after, trespass,
        "the third party's write is gone, so the undo did land -- the refusal above was reported \
         and not enforced"
    );
}

/// 🔴 **G-7** (`req/265` §4-2 `github16_no_change_undo_succeeds`) — the other side of G-6, so that
/// the refusal is a discrimination and not a posture.
///
/// A fail-closed road that refuses **everything** passes G-6 and is worthless. This is the
/// negative control for it.
#[test]
fn g7_an_undisturbed_world_lets_the_undo_through() {
    let probe = spawn("g7", ReadFailure::None);
    let mut wired = wire(&probe, "g7", P1_CATALOGUE);

    let before = probe.issue("/body");
    let id = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("this session wrote this")),
    )
    .expect("commit");
    let allowed = undo(&mut wired, &id);
    let after = probe.issue("/body");

    println!("RMCP1_G7 allowed={allowed:?} before={before} after={after}");
    assert!(
        allowed.is_ok(),
        "an undo with nothing in its way was refused, which makes G-6 a posture rather than a \
         discrimination: {allowed:?}"
    );
    assert_eq!(before, after, "and it put the prior back");
}

/// 🔴 **G-8** — a merged pull request refuses to reopen, **loudly**, and that is why
/// `update_pull_request_state` keeps the verdict `true`.
///
/// # 🔴 Why this probe stopped going through the engine, and what the first version was measuring
///
/// The first version committed a close, had a third party set `merged`, and asserted that the undo
/// refused. It was green — and it was green for the **wrong mechanism**: setting `merged` moves
/// the object, so `DR-43-1`'s witness check refused before the compensating call was ever sent.
/// That is G-6's mechanism wearing G-8's name, and it would have left "the server refuses to
/// reopen a merged pull request" asserted rather than measured. There is no way to reach the
/// second fact through the engine, because a pull request that merged after the escrow *has*
/// moved; the engine is right and the probe was wrong.
///
/// So the claim is measured where it lives: at the tool. The compensating call
/// `update_pull_request_state{state: "open"}` — exactly the call the declaration in
/// `github16-p1-catalogue.json` would assemble — is sent to a merged pull request, and the server
/// answers an error rather than a success. The verdict `true` for this tool rests on that: the
/// undo either restores the state or the operator is **told** it did not, which is the same
/// failure mode `create_or_update_file` already ships with (`req/265` §2-1 boundary ①). Contrast
/// [`g11_the_undeclarable_four_are_absent_and_each_absence_has_a_mechanism`]'s two `S` rows, where
/// the undo returns success over an object that did not come back.
#[test]
fn g8_a_merged_pull_request_refuses_its_own_inverse_out_loud() {
    let probe = spawn("g8", ReadFailure::None);
    probe.third_party_sets("pulls", "octo/demo/7", "merged", json!(true));
    probe.third_party_sets("pulls", "octo/demo/7", "state", json!("closed"));

    // The reopen -- the exact call the declaration assembles for an undo of a close.
    let reopen = probe.client.request(
        "tools/call",
        json!({
            "name": "update_pull_request_state",
            "arguments": { "owner": "octo", "repo": "demo", "pullNumber": 7, "state": "open" },
        }),
    );
    // The same call on a pull request that is not merged, so the refusal is a discrimination.
    probe.third_party_sets("pulls", "octo/demo/7", "merged", json!(false));
    let allowed = probe.client.request(
        "tools/call",
        json!({
            "name": "update_pull_request_state",
            "arguments": { "owner": "octo", "repo": "demo", "pullNumber": 7, "state": "open" },
        }),
    );

    println!(
        "RMCP1_G8 merged_reopen={:?} unmerged_reopen_ok={} state={}",
        reopen.as_ref().err(),
        allowed.is_ok(),
        probe.pull("/state")
    );
    let message = reopen
        .expect_err("reopening a merged pull request is refused")
        .to_string();
    assert!(
        message.contains("422"),
        "the refusal is not the 422 the real API answers, so the fixture is being kinder than the \
         world and the `true` verdict for this tool rests on nothing: {message}"
    );
    assert!(
        allowed.is_ok(),
        "an unmerged pull request could not be reopened either, which makes the refusal above a \
         posture rather than a discrimination: {allowed:?}"
    );
    assert_eq!(
        probe.pull("/state"),
        json!("open"),
        "and the discriminating call actually did the thing"
    );
}

// ===========================================================================
// family 3 — a read that fails, and what the deployment said should happen then
// ===========================================================================

/// 🔴 **G-9** (`req/265` §4-3 `github16_read_unavailable_denies_effect`) — **D-1, fail-closed**:
/// the prior cannot be read, so the effect does not happen, and the proof is that the server's own
/// log holds **zero** write arrivals.
///
/// The server is **up** for this probe — its write tools work — and only its read face refuses.
/// That asymmetry is what makes fail-closed a *choice*: against a server that is simply down, no
/// call reaches it whatever gx decides, and the probe would prove nothing.
#[test]
fn g9_a_read_that_fails_denies_the_effect_and_no_write_arrives() {
    let probe = spawn("g9", ReadFailure::Tool);
    let mut wired = wire(&probe, "g9", P1_CATALOGUE);
    probe.clear_log();

    let refused = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("this must never arrive")),
    );

    let writes = probe.writes();
    let body = probe.issue("/body");
    println!(
        "RMCP1_G9 refused={refused:?} writes={} arrivals={:?} body={body}",
        writes.len(),
        probe.arrivals()
    );
    assert!(
        refused.is_err(),
        "the effect was taken with no prior escrowed"
    );
    assert!(
        writes.is_empty(),
        "🔴 a forward call reached the server after its prior could not be read. D-1 is \
         fail-closed and the count that measures it is the server's own: {writes:?}"
    );
    assert_eq!(
        body,
        json!(BODY_BEFORE),
        "and the object is untouched, which is the fact the arrival count is evidence for"
    );
}

/// 🔴 **G-10** (`req/265` §4-3 `github16_opt_in_escalates_not_silent`) — the **opt-in**
/// (`$on_read_failure: "unknown"`, DR-46-12) takes the effect, and pays for it in the receipt.
///
/// Three things have to be true together, and a deployment that got two of them would be the
/// silent fail-open the ruling refused:
///
/// 1. the effect **happens** (otherwise the opt-in is decoration);
/// 2. the gate **escalates** rather than admitting (E-M3-4: an effect with no inverse is a
///    person's decision, and the relaxation must not go around that);
/// 3. the receipt records [`InverseStatus::Undetermined`] — *nobody established whether an inverse
///    exists* — and **not** `Unavailable`, which asserts that one was sought and found not to be.
///
/// 🔴 Point 3 corrects the brief this lane was fired with, which asked for `Unavailable`. That was
/// the pre-**DR-46-26** fold: `req/38` §198 ruling (b) found the third value was being flattened
/// into the second everywhere, and `Undetermined` is the shipped, audited word. `req/602` records
/// the correction rather than quietly satisfying the older sentence.
#[test]
fn g10_the_opt_in_takes_the_effect_and_the_receipt_says_undetermined() {
    let probe = spawn("g10", ReadFailure::Tool);
    let opt_in = with_read_failure_opt_in(P1_CATALOGUE);
    let mut wired = wire(&probe, "g10", &opt_in);
    probe.clear_log();

    let locator = probe.locator(ISSUE);
    let arguments = issue_args("body", json!("the opt-in accepted this"));
    let intent = intent_for(&locator, "update_issue_body", &arguments);

    wired.engine.submit(&intent, 42, AT).expect("submit");
    let id = wired.engine.plan(&intent, AT).expect("plan");
    let after_verify = wired
        .engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    let ruling = HumanRuling {
        decision: VerdictKind::Admit,
        reason: "the operator accepts an effect whose reversibility is unknown".to_string(),
        actor: Actor::Human {
            key: "key-operator-rmcp1".to_string(),
        },
    };
    let after_ruling = wired
        .engine
        .escalation(&id, &ruling, AT, &signing_key())
        .expect("43 T-5");
    wired.engine.canonicalize(&id, AT, None).expect("T-8");
    let after_commit = wired.engine.commit(&id, AT, &signing_key()).expect("T-11");

    let status = wired.engine.inverse_status(&id);
    let body = probe.issue("/body");
    println!(
        "RMCP1_G10 verify={after_verify:?} ruling={after_ruling:?} commit={after_commit:?} \
         status={status:?} body={body} writes={}",
        probe.writes().len()
    );

    assert_eq!(
        after_verify,
        Lifecycle::Escalated,
        "the opt-in went around E-M3-4, which is the silent fail-open DR-46-12 refused"
    );
    assert_eq!(after_ruling, Lifecycle::Admitted);
    assert_eq!(after_commit, Lifecycle::Committed);
    assert_eq!(
        body,
        json!("the opt-in accepted this"),
        "the opt-in is only meaningful if the effect actually happened"
    );
    assert_eq!(
        status,
        Some(InverseStatus::Undetermined),
        "the receipt has to say **nobody found out**, not `Unavailable` (\"we asked and there is \
         none\"). DR-46-26 is the ruling that keeps those two apart"
    );
}

/// 🔴 **G-14** (`req/265` §4-3 `github16_read_unavailable_message_verbatim`) — the refusal a
/// deployment reads names **both** remedies, and it survives the crate boundary and the wire
/// **verbatim**.
///
/// `req/38` §195's verbatim-guidance discipline (7/7): a refusal that degrades to "an error occurred"
/// somewhere between `gx-adapter-mcp` and the process that reads it is a refusal nobody can act
/// on. The constant is `gx-adapter-mcp`'s; what this measures is that the sentence which reaches a
/// caller **through a real transport, after a real server refused** is still that sentence and not
/// a paraphrase of it, and that both roads out are in it: make the read answer, or opt in.
#[test]
fn g14_the_refusal_names_both_remedies_and_survives_the_wire_verbatim() {
    let probe = spawn("g14", ReadFailure::Tool);
    let mut wired = wire(&probe, "g14", P1_CATALOGUE);

    let refused = commit(
        &mut wired,
        &probe.locator(ISSUE),
        "update_issue_body",
        &issue_args("body", json!("never arrives")),
    )
    .expect_err("the read face refuses, so the effect is refused");

    println!("RMCP1_G14 refusal={refused}");
    assert!(
        refused.contains(gx_adapter_mcp::READ_FAILURE_REFUSAL),
        "the refusal that reached a caller is not the sentence `gx-adapter-mcp` wrote. Something \
         on the road between the adapter and here paraphrased it, and a paraphrase is a remedy \
         nobody can follow: {refused}"
    );
    for remedy in ["$on_read_failure", "make the declared read face answer"] {
        assert!(
            refused.contains(remedy),
            "the refusal does not name the remedy {remedy:?}, so a reader is told what failed and \
             not what to do: {refused}"
        );
    }
    assert!(
        refused.contains("issue_read"),
        "and it does not say **which** read face would not answer, which is the one fact that \
         makes the first remedy actionable: {refused}"
    );
}

/// 🔴 **G-15** (`req/265` §4-3 `github16_no_resource_for_issue`) — a server with **no read face at
/// all** is one `gx wrap` cannot plan on, **and the opt-in does not rescue it**.
///
/// # 🔴 This probe was written expecting `unknown` and measured `no plan at all`. The measurement
/// is the finding, and it is the sharpest one in the lane.
///
/// The shape here is a real github deployment for an issue: `AllResources` publishes five
/// `repo://…/contents…` templates and nothing else. Both postures were driven on the expectation
/// that `$on_read_failure: "unknown"` would let the effect through with its reversibility
/// unrecorded. It does not, and the reason is structural:
///
/// * `$on_read_failure` governs the **escrow** read, in `invert`, at T-3 and T-10b;
/// * `snapshot` — 41 §4's precondition read — runs **before** any of that, and it has no such
///   relaxation. `catalogue.rs`'s own header says it: "a locator matching no `$cas_read` pattern
///   on a server with no resource face is still one `gx wrap` refuses to plan on — declaration
///   unlocks it, and nothing else does".
///
/// So for a **faithful** github issue, `$cas_read` is the only road, and R-MCP1 measured that
/// `$cas_read` cannot be written for one: [`CasArgSource`]'s per-locator words all produce JSON
/// **strings**, and `issue_read` requires `issue_number` to be a **number**. One missing word
/// stands between the shipped mechanism and a github issue deployment, and the opt-in is not a way
/// round it. `req/602` raises that as a DR candidate; this probe is what it rests on.
///
/// [`CasArgSource`]: gx_adapter_mcp::CasArgSource
#[test]
fn g15_a_server_with_no_read_face_cannot_be_planned_on_and_the_opt_in_does_not_rescue_it() {
    for (name, catalogue, expected) in [
        ("g15_refuse", P1_CATALOGUE.to_string(), None),
        ("g15_optin", with_read_failure_opt_in(P1_CATALOGUE), None),
    ] {
        let probe = spawn(name, ReadFailure::All);
        let wired = wire(&probe, name, &catalogue);
        let locator = probe.locator(ISSUE);
        // `snapshot` is refused too on this server, so the pre-state is the one a plan carries for
        // an object nothing will answer for -- which is exactly the deployment being modelled.
        let snapshot = wired.adapter.snapshot(&locator);
        let verdict = snapshot.as_ref().ok().and_then(|pre| {
            let intent = intent_for(
                &locator,
                "update_issue_body",
                &issue_args("body", json!("after")),
            );
            wired
                .adapter
                .plan(&intent, pre)
                .ok()
                .and_then(|delta| wired.adapter.reversibility(&delta, pre).ok())
        });
        println!(
            "RMCP1_G15 {name} snapshot_ok={} verdict={:?}",
            snapshot.is_ok(),
            verdict.map(|v| v.as_str())
        );
        assert_ne!(
            verdict,
            Some(Reversibility::True),
            "🔴 a server that answered for nothing was told an inverse is available. Whatever \
             else is true of a deployment with no read face, `true` is the one answer it must \
             never get"
        );
        assert!(
            snapshot.is_err(),
            "`snapshot` answered for an object no face on this server will read. If that became \
             possible, the whole shape of this probe changed and the finding it carries has to be \
             re-derived rather than re-asserted"
        );
        assert_eq!(
            verdict, expected,
            "🔴 the posture's answer moved. **Both** postures reach no verdict, because the \
             refusal happens at `snapshot` and `$on_read_failure` governs only the escrow read one \
             stage later. If the opt-in ever does rescue this shape, it means the relaxation grew \
             a second site -- and a relaxation with two sites is the silent fail-open DR-46-12 \
             refused, arriving by the back door"
        );
    }
}

/// The same catalogue with the deployment's opt-in on. Built by editing the JSON rather than
/// keeping a second fixture in step by hand — a copy that drifted would make G-10 a test of the
/// copy.
fn with_read_failure_opt_in(catalogue: &str) -> String {
    let mut parsed: Value = serde_json::from_str(catalogue).expect("the P1 catalogue is JSON");
    parsed
        .as_object_mut()
        .expect("a catalogue is an object")
        .insert("$on_read_failure".to_string(), json!("unknown"));
    serde_json::to_string(&parsed).expect("serialize")
}

/// 🔴 **G-15's positive control** (`req/621` §5-0-2, `req/38` §394 M-4) — the discrimination the
/// two arms of [`g15_a_server_with_no_read_face_cannot_be_planned_on_and_the_opt_in_does_not_rescue_it`]
/// lack on their own.
///
/// `g15_refuse` and `g15_optin` both derive their `Err` from the server's `ReadFailure::All`
/// setting, so a locator this adapter could not parse **at all** would also refuse at `snapshot`,
/// for an unrelated reason, and read identically green. Without a positive control that shape is
/// indistinguishable from the finding. This probe drives the **same `ISSUE` locator** twice — once
/// on a server whose read face is off (refuses, as above) and once on a server whose read face
/// answers (snapshots) — and both halves live in one body on purpose: the discrimination *is* the
/// probe, not a fact split across two greens that a later hand could delete one of.
#[test]
fn g15_control_the_same_locator_snapshots_when_the_read_face_answers() {
    // The negative half: the refusal the two arms measure, re-established beside its control so this
    // probe cannot pass on a locator that simply never parses.
    let off = spawn("g15_control_off", ReadFailure::All);
    let refused = wire(&off, "g15_control_off", P1_CATALOGUE)
        .adapter
        .snapshot(&off.locator(ISSUE));

    // The positive half: the identical locator, on a server that answers, does snapshot.
    let on = spawn("g15_control_on", ReadFailure::None);
    let snapped = wire(&on, "g15_control_on", P1_CATALOGUE)
        .adapter
        .snapshot(&on.locator(ISSUE));

    println!(
        "RMCP1_G15_CONTROL locator={} off_ok={} on_ok={}",
        on.locator(ISSUE),
        refused.is_ok(),
        snapped.is_ok()
    );
    assert!(
        refused.is_err(),
        "the control's negative half: with the read face off, the same locator refuses at \
         `snapshot`, which is the shape `g15_refuse`/`g15_optin` measure"
    );
    assert!(
        snapped.is_ok(),
        "🔴 the same `ISSUE` locator snapshots when the read face answers, so the refusal the two \
         G-15 arms record is the **read face** and not a locator this adapter cannot parse. Without \
         this half every green in G-15 is consistent with a snapshot that never worked at all"
    );
}

/// 🔴 **G-15's mechanism assertion** (`req/621` §5-0, `req/38` §378-3 / §394 M-4) — the claim the
/// original G-15 doc-comment made but never asserted, made falsifiable.
///
/// G-15's three assertions all reduced to `snapshot.is_err()`; the mechanism its header names — that
/// for a faithful github issue `$cas_read` is the only road, and it cannot be written because
/// [`CasArgSource`]'s per-locator words are all JSON **strings** while `issue_read` wants
/// `issue_number` as a **number** — was asserted **nowhere**, and `req/38` §378-3 filed DR-46-38 on
/// that unmeasured mechanism. This probe measures the two halves and the seam between them:
///
/// 1. every per-locator `CasArgSource` word (`Resource`, `ResourceSuffix`, `Server`) resolves to a
///    JSON string — so none can supply a numeric `issue_number` from the locator;
/// 2. the github face publishes `issue_read` with `issue_number` typed `number`;
/// 3. the seam: the string such a word would produce is exactly what a real `issue_read` call
///    refuses, so the gap is not a modelling choice but the tool's own type discipline.
///
/// It does **not** decide DR-46-38's own merit (whether a new numeric word should be added) — only
/// that the finding the DR rests on is now measured rather than narrated. `ConstJson` can carry a
/// number, but it is a **constant**: it cannot vary with the locator, which is the whole of why a
/// per-locator numeric argument still has no word.
#[test]
fn g15_the_mechanism_no_per_locator_cas_word_supplies_issue_read_its_number() {
    // Half 1 — every per-locator word is a string. A fixed github issue locator and the prefix a
    // `$cas_read` for it would match, resolved member by member the way `dr46_16_cas_read_by_tool`
    // does, so this reads the vocabulary and not a whole snapshot.
    const SERVER: &str = "https://mcp.example/gh";
    const ISSUE_URI: &str = "github://octo/demo/issues/1";
    const PREFIX: &str = "github://octo/demo/issues/";
    for word in [
        CasArgSource::Resource,
        CasArgSource::ResourceSuffix,
        CasArgSource::Server,
    ] {
        let built = CasTemplate::new()
            .with("issue_number", word.clone())
            .resolve(SERVER, ISSUE_URI, PREFIX)
            .expect("every per-locator word is total over a parsed position");
        let value: Value = serde_json::from_slice(&built).expect("resolved arguments are JSON");
        println!(
            "RMCP1_G15_MECH word={word:?} issue_number={} is_string={}",
            value["issue_number"],
            value["issue_number"].is_string()
        );
        assert!(
            value["issue_number"].is_string(),
            "🔴 the per-locator word {word:?} produced {:?} for `issue_number`, not a string. \
             G-15's finding rests on every per-locator word being a string; if one now yields a \
             number the gap DR-46-38 addresses has closed and the finding must be re-derived",
            value["issue_number"]
        );
    }

    // Half 2 — `issue_read` publishes `issue_number` as a number, read from the server's own
    // `tools/list` rather than from this file's belief about the schema.
    let probe = spawn("g15_mechanism", ReadFailure::None);
    let listed = probe
        .client
        .request("tools/list", json!({}))
        .expect("tools/list answers");
    let issue_read = listed
        .get("tools")
        .and_then(Value::as_array)
        .expect("a tools array")
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some("issue_read"))
        .expect("the github face publishes issue_read");
    let declared_type = issue_read
        .pointer("/inputSchema/properties/issue_number/type")
        .and_then(Value::as_str);
    println!("RMCP1_G15_MECH issue_read.issue_number.type={declared_type:?}");
    assert_eq!(
        declared_type,
        Some("number"),
        "🔴 `issue_read` no longer types `issue_number` as a number. The other half of the gap is \
         that the tool wants a number; if it took a string the per-locator words above would suffice \
         and G-15's finding would be gone"
    );

    // Half 3 — the seam. The string a per-locator word yields is exactly what a live `issue_read`
    // call refuses, so the gap is the tool's own type discipline and not a fixture convenience.
    let refused = probe
        .client
        .request(
            "tools/call",
            json!({
                "name": "issue_read",
                "arguments": {
                    "method": "get", "owner": "octo", "repo": "demo", "issue_number": "1",
                },
            }),
        )
        .expect_err("a string `issue_number` is refused");
    let message = refused.to_string();
    println!("RMCP1_G15_MECH string_issue_number_refused={message:?}");
    assert!(
        message.contains("must be a number"),
        "`issue_read` accepted a string `issue_number`; the type requirement the finding rests on \
         is not enforced by the server, so the seam between the string words and the numeric tool \
         is not real: {message}"
    );
}

// ===========================================================================
// the census, and the negative control under it
// ===========================================================================

/// 🔴 **G-11** — the four tools this lane will **not** declare are absent from the fixture, and
/// each absence has a mechanism beside it.
///
/// A catalogue is a claim. Declaring these four would be a false one, and "we filled four more
/// rows" is not a result worth a lie (11 §5-2 C-25: the verdict *is* the output). This probe holds
/// the file to the absence so that a later hand completing the table turns it red and reads why.
#[test]
fn g11_the_undeclarable_four_are_absent_and_each_absence_has_a_mechanism() {
    let parsed: Value = serde_json::from_str(P1_CATALOGUE).expect("the P1 catalogue is JSON");
    let declared: Vec<&String> = parsed
        .as_object()
        .expect("an object")
        .keys()
        .filter(|key| !key.starts_with('$'))
        .collect();

    // (tool, class, why -- the mechanism, never "hard" or "not supported")
    let refused: [(&str, &str, &str); 4] = [
        (
            "update_issue_labels",
            "E-proj",
            "the issue renders `labels` as objects and the setter takes strings; RFC 6901 selects \
             a member and does not project a collection",
        ),
        (
            "update_issue_assignees",
            "E-proj",
            "the same shape, and the API additionally drops an un-assignable login in silence, so \
             even a projecting word would owe a re-read before claiming the object is back",
        ),
        (
            "update_issue_state",
            "S",
            "the undo succeeds and `state_reason` is not back: closing carries a reason and \
             reopening cannot restore one. A partial inverse is DR-46-36, parked by `req/38` §347 \
             ruling 1 and not available to declare",
        ),
        (
            "update_pull_request_draft_state",
            "S",
            "ready->draft clears the requested reviewers and draft->ready does not put them back; \
             the inverse's reach is narrower than the forward's and the undo does not say so",
        ),
    ];

    println!(
        "RMCP1_G11 declared={declared:?} refused={:?}",
        refused.map(|r| r.0)
    );
    for (tool, class, why) in refused {
        assert!(
            !declared.iter().any(|key| key.as_str() == tool),
            "🔴 `{tool}` is declared in `github16-p1-catalogue.json`. It is class {class}: {why}. \
             If the mechanism changed, change the mechanism first and this row after"
        );
    }
    assert_eq!(
        declared.len(),
        7,
        "the P1 census moved. Seven declared (issue body/title/type/milestone, pull-request \
         body/title/state) and four refused above; the eleventh through fifteenth tools are P0's. \
         A row added without a mechanism is the thing this probe exists to stop: {declared:?}"
    );
}

/// 🔴 **G-12** — the negative control **under every green above**: the fixture server really does
/// validate its arguments.
///
/// `req/152` §5: the A2 filing found that the mock's undo "success" was the mock not validating
/// its arguments. Every round trip in this file would pass against a lenient server that accepted
/// a restore call assembled from a wrong declaration. This probe sends a member no github tool
/// declares and requires the refusal, so that `GX_PROBE_STRICT_ARGS` is measured rather than set.
#[test]
fn g12_the_strict_face_refuses_a_member_the_tool_does_not_declare() {
    let probe = spawn("g12", ReadFailure::None);

    let good = probe.client.request(
        "tools/call",
        json!({
            "name": "update_issue_body",
            "arguments": { "owner": "octo", "repo": "demo", "issue_number": 1, "body": "ok" },
        }),
    );
    let bad = probe.client.request(
        "tools/call",
        json!({
            "name": "update_issue_body",
            "arguments": {
                "owner": "octo", "repo": "demo", "issue_number": 1, "body": "ok",
                "not_a_member_of_this_tool": true,
            },
        }),
    );

    println!(
        "RMCP1_G12 good={:?} bad={:?}",
        good.is_ok(),
        bad.as_ref().err()
    );
    assert!(good.is_ok(), "a well-formed call is accepted: {good:?}");
    let message = bad.expect_err("the strict face refuses").to_string();
    assert!(
        message.contains("should be not present"),
        "the refusal is not the strict-body wording, so `GX_PROBE_STRICT_ARGS` may not be reaching \
         the github face at all -- which would make every green in this file a green against a \
         lenient server: {message}"
    );
}

/// 🔴 **G-13** — the eleven tools are published in upstream's shapes, and the two read tools are
/// among them.
///
/// A fixture whose `tools/list` disagreed with the declarations would make every probe here a test
/// of the fixture's imagination. The member lists are transcribed from `__toolsnaps__` at
/// `v1.9.0`; what this checks is that the transcription is what the server actually publishes and
/// that the count is thirteen — eleven writes plus `issue_read` and `pull_request_read`.
#[test]
fn g13_the_github_face_publishes_the_thirteen_tools_p1_is_about() {
    let probe = spawn("g13", ReadFailure::None);
    let listed = probe
        .client
        .request("tools/list", json!({}))
        .expect("tools/list answers");
    let names: Vec<String> = listed
        .get("tools")
        .and_then(Value::as_array)
        .expect("a tools array")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect();

    println!("RMCP1_G13 names={names:?}");
    for expected in [
        "issue_read",
        "pull_request_read",
        "update_issue_assignees",
        "update_issue_body",
        "update_issue_labels",
        "update_issue_milestone",
        "update_issue_state",
        "update_issue_title",
        "update_issue_type",
        "update_pull_request_body",
        "update_pull_request_draft_state",
        "update_pull_request_state",
        "update_pull_request_title",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "the github face does not publish {expected:?}: {names:?}"
        );
    }
    // 🔴 **L-6-b** (`req/621` §5 / §98, `req/38` §394): the loop above only checks each of the
    // thirteen is *present*, so a fourteenth github tool added to the face — a row the census `g11`
    // exists to refuse — would slip through green. The doc-comment says "the count is thirteen"; this
    // is the cardinality that holds it to that. Counted as the non-`notes.` names, because the github
    // face is additive over the notes face (`notes.write`/`notes.restore`/`notes.stray` stay), so the
    // total is sixteen and the thirteen are exactly the tools whose names are not notes'.
    let github_tools = names
        .iter()
        .filter(|name| !name.starts_with("notes."))
        .count();
    assert_eq!(
        github_tools, 13,
        "🔴 the github face publishes {github_tools} non-notes tools, not the thirteen this suite is \
         about (eleven writes + `issue_read` + `pull_request_read`). A fourteenth is a census row \
         `g11` refuses, and without this count it would pass every arm above green: {names:?}"
    );
    // The notes face is unchanged and still there -- this mode adds a face, it does not replace one.
    assert!(
        names.iter().any(|name| name == "notes.write"),
        "the github face displaced the notes face, which would break every other suite that \
         drives this server: {names:?}"
    );
}

/// Kept so `Path` is used on every platform's build of this file.
#[allow(dead_code)]
fn scratch_root() -> &'static Path {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
}
