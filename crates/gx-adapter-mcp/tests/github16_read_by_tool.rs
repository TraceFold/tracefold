// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-9 A-3 / A-4, DR-46-10, DR-46-12** (`req/38` §196, requirement `req/265`) — the escrow
//! reaches a prior that lives behind a **tool**, and every way that can fail is a named answer.
//!
//! # The scope, which is four tools and not sixteen
//!
//! `req/265` §1 re-counted the target from upstream's own `__toolsnaps__` (v1.9.0 and `main`, 116
//! tools each): the write tools whose name carries `update` are **fifteen**, not sixteen, and
//! **eleven of the fifteen live behind the `issues_granular` / `pull_requests_granular` feature
//! flags** — a deployment running the default toolsets does not have them. `req/38` §196 fixed P0
//! at the four that exist on a flag-free server, and this suite is written against those four and
//! nothing else:
//!
//! | tool | verdict | why, in the name of a mechanism |
//! |---|---|---|
//! | `create_or_update_file` | **true** | the contents face answers `resources/read`, and the server validates a blob sha, so the CAS is real. Declared since `req/153`; **unchanged by this lane**, and one probe below exists only to measure that it is unchanged |
//! | `update_gist` | **true**, for the overwrite of a file that already exists | no resource face at all; the prior is `get_gist`'s document, which is what `read_by` reaches and what `prior_json` points into |
//! | `update_pull_request` | **false** — *compound* | one call carries fields that reverse (title, body) beside `reviewers`, whose semantics are **add**, not replace. Re-calling the same tool does not remove a reviewer it added; the removal is a different tool. A declaration that claimed otherwise would undo three quarters of a call and report a whole one |
//! | `update_pull_request_branch` | **false** — *no inverse on the surface* | the tool merges base into head, which **creates a commit**. The inverse is a force-push of the old head sha, and github-mcp-server v1.9.0 publishes no force-push tool among its 116. Nobody can declare what the server does not offer |
//!
//! The last two are **absent from `fixtures/github16-p0-catalogue.json` on purpose**, and
//! [`p8_update_pull_request_and_branch_are_false_by_mechanism`] holds the file to that absence. A
//! catalogue is a claim; declaring those two would be a false one, and "we filled two more rows"
//! is not a result worth a lie (11 §5-2 C-25: the verdict *is* the output).
//!
//! # What this fixture is, and where it diverges from the real server
//!
//! [`Github16Server`] is an in-process transport, for `tests/support/mod.rs`'s reason (this crate
//! ships a boundary, not a client). It models three faces and the divergence is declared here
//! rather than found later:
//!
//! * `resources/read` answers for both objects. **The real server has no gist resource**
//!   (`req/265` §1: `AllResources` registers five templates, all `repo://…/contents…`). The
//!   fixture publishes one because `snapshot` and `precondition` still go through
//!   `resources/read` — this lane moved the **escrow** half of the read face and not the CAS half,
//!   and a fixture with no resource at all would measure `gx wrap` refusing to plan, which
//!   `tests/notion_page_catalogue.rs` already measures.
//! * `get_gist` answers a **document** (`{id, description, files: {…}}`), which is the shape the
//!   real tool answers and the reason `prior_json` had to exist: the value a restore call wants is
//!   one member of it, not the whole thing.
//! * the log records every arrival in order, with its kind, which is what makes "the escrow read
//!   happened **before** `apply`" a measurement rather than a reading of the source (`req/119` §5
//!   A-7's rule, one crate over: count on the server's side of the wire).
//!
//! # What is **not** measured here (`req/265` §6's discipline, kept)
//!
//! * **Zero live calls to github.** Every sentence in the table above is read off upstream's Go
//!   source and its published API, and none of it was executed against `api.github.com`.
//! * **No rate limit is exercised.** The `LIMITS.md` v0.5-d line about the escrow read costing a
//!   guarded call about two thirds of its throughput is arithmetic over the request count P-1
//!   measures (two reads per guarded forward call), not a measurement against a real limit.
//! * **The eleven flag-gated tools are untouched** — P1, and this lane does not guess at them.
//! * **The 1 MiB escrow ceiling is not reached.** `MAX_INVERSE_PAYLOAD_BYTES` has an instance on
//!   this road (a gist file can exceed it) and no probe here builds one.
//! * **No `$on_read_failure` value other than the two words is exercised beyond the parse.**

mod support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gx_adapter_mcp::{
    Admitted, ArgSource, Catalogue, IdentityPart, McpAdapter, McpDelta, ObjectIdentity,
    OnReadFailure, PointerSegment, PriorPointer, PriorRead, RestoreTemplate, Reversibility,
    ToolCall, ToolTransport, READ_FAILURE_REFUSAL,
};
use gx_core::{Actor, SubstrateKind, Timestamp, TransformationId, VerdictKind};
use gx_engine::{Engine, HumanRuling, InjectedEvidence, InverseStatus, Lifecycle, UndoWitness};
use gx_substrate::{Error, Result, SubstrateAdapter};

use support::{intent_for, RewindableLog};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const UNDO_AT: Timestamp = Timestamp(1_754_000_100_000_000_000);

/// The endpoint this fixture answers for.
const SERVER: &str = "stdio://github16";
/// The gist object. `gist:` rather than `repo://…` because the real server publishes **no**
/// resource template for a gist; the locator names the object gx's change is about, and what the
/// server will answer for is a separate question this suite keeps separate.
const GIST: &str = "gist:g1";
/// The file object, which the real contents face does answer for.
const FILE: &str = "repo://o/r/refs/heads/main/contents/notes.txt";
/// The one file inside the fixture's gist. The `prior_json` pointer names it, because RFC 6901
/// pointers are literal — see [`p9_a_literal_pointer_guards_one_member_and_the_gap_is_named`].
const GIST_FILE: &str = "notes.md";

const GIST_BEFORE: &str = "the gist as it was\n";
const GIST_AFTER: &str = "the gist as the agent wants it\n";
/// CJK, an emoji outside the BMP, and a combining mark — the three shapes a UTF-8 round trip loses
/// if anything on the road normalises or re-encodes.
///
/// 🔴 Written as escapes, not as glyphs, and the reason is a machine rather than a preference:
/// `crates/gx-adapter-mcp` is a **migrated** directory in `req/cjk_baseline.json`, so
/// `probes/doubt/tests/cjk_doubt.rs` c3 requires it to hold **zero** lines in
/// `U+3040..U+30FF` / `U+4E00..U+9FFF`. A fixture that spelled its own Japanese out would turn a
/// probe about UTF-8 into a repository that fails its own census — which is exactly what `req/38`
/// §197 recorded happening to an unrelated script one day earlier. The bytes under test are the
/// same either way; only the source is ASCII.
///
/// The escapes spell: three kanji, a hiragana particle, four katakana, a full-width space, a
/// non-BMP emoji, `e` + `U+0301` (a combining acute, which is two code points and one grapheme),
/// and two more kanji.
const GIST_CJK: &str = "\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{30c6}\u{30ad}\u{30b9}\u{30c8}\u{3000}\
                        \u{1f534} e\u{0301} \u{307e}\u{3068}\u{3081}\n";

const FILE_BEFORE: &str = "the file as it was\n";

/// A pack that admits everything: this suite is about the escrow and not about the gate.
const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

const P0_CATALOGUE: &[u8] = include_bytes!("fixtures/github16-p0-catalogue.json");
const P0_CATALOGUE_UNKNOWN: &[u8] = include_bytes!("fixtures/github16-p0-catalogue-unknown.json");

// ---------------------------------------------------------------------------
// The fixture server
// ---------------------------------------------------------------------------

/// What arrived, in order, and by which road.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Arrival {
    kind: &'static str,
    what: String,
}

/// An in-process server shaped like the flag-free github-mcp-server, for the two P0 tools that
/// have an inverse.
#[derive(Debug, Default)]
struct Github16Server {
    /// `resources/read`'s answers, keyed by resource URI.
    resources: Mutex<BTreeMap<String, Vec<u8>>>,
    /// The gist document behind `get_gist`, which no resource template exposes.
    gist_files: Mutex<BTreeMap<String, String>>,
    log: Mutex<Vec<Arrival>>,
    read_tool_fails: AtomicBool,
}

impl Github16Server {
    fn new() -> Self {
        let server = Self::default();
        server
            .resources
            .lock()
            .expect("not poisoned")
            .insert(GIST.to_string(), GIST_BEFORE.as_bytes().to_vec());
        server
            .resources
            .lock()
            .expect("not poisoned")
            .insert(FILE.to_string(), FILE_BEFORE.as_bytes().to_vec());
        server
            .gist_files
            .lock()
            .expect("not poisoned")
            .insert(GIST_FILE.to_string(), GIST_BEFORE.to_string());
        server
    }

    fn locator(resource: &str) -> String {
        format!("{SERVER}#{resource}")
    }

    fn note(&self, kind: &'static str, what: impl Into<String>) {
        self.log.lock().expect("not poisoned").push(Arrival {
            kind,
            what: what.into(),
        });
    }

    fn arrivals(&self) -> Vec<Arrival> {
        self.log.lock().expect("not poisoned").clone()
    }

    fn count(&self, kind: &str) -> usize {
        self.arrivals().iter().filter(|a| a.kind == kind).count()
    }

    fn clear_log(&self) {
        self.log.lock().expect("not poisoned").clear();
    }

    /// Make the declared read face fail, the way a 5xx or a rate limit does.
    fn break_read_tool(&self) {
        self.read_tool_fails.store(true, Ordering::SeqCst);
    }

    fn gist_content(&self) -> String {
        self.gist_files
            .lock()
            .expect("not poisoned")
            .get(GIST_FILE)
            .cloned()
            .unwrap_or_default()
    }

    /// Write the gist behind the adapter's back: `Fixture::disturb`'s job, for the CAS arm.
    fn third_party_writes_gist(&self, contents: &str) {
        self.set_gist(GIST_FILE, contents);
    }

    fn set_gist(&self, filename: &str, contents: &str) {
        self.gist_files
            .lock()
            .expect("not poisoned")
            .insert(filename.to_string(), contents.to_string());
        self.resources
            .lock()
            .expect("not poisoned")
            .insert(GIST.to_string(), contents.as_bytes().to_vec());
    }

    /// `get_gist`'s answer: the whole document, which is **not** the value a restore wants.
    fn gist_document(&self) -> String {
        let files = self.gist_files.lock().expect("not poisoned");
        let mut members = serde_json::Map::new();
        for (name, contents) in files.iter() {
            members.insert(
                name.clone(),
                serde_json::json!({ "filename": name, "content": contents }),
            );
        }
        serde_json::json!({
            "id": "g1",
            "description": "a description the restore call must not send as the content",
            "public": false,
            "files": serde_json::Value::Object(members),
        })
        .to_string()
    }
}

impl ToolTransport for Github16Server {
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>> {
        self.note("read", resource);
        if server != SERVER {
            return Err(Error::Unreadable {
                locator: Self::locator(resource),
                detail: format!("this fixture is one server ({SERVER}) and that is another"),
            });
        }
        self.resources
            .lock()
            .expect("not poisoned")
            .get(resource)
            .cloned()
            .ok_or_else(|| Error::Unreadable {
                locator: Self::locator(resource),
                detail: "this server publishes no resource at that URI".to_string(),
            })
    }

    fn read_prior_by_tool(&self, server: &str, tool: &str, arguments: &[u8]) -> Result<Vec<u8>> {
        let arguments: serde_json::Value =
            serde_json::from_slice(arguments).unwrap_or(serde_json::Value::Null);
        self.note("read_by_tool", format!("{tool} {arguments}"));
        if server != SERVER {
            return Err(Error::Unreadable {
                locator: server.to_string(),
                detail: "another server".to_string(),
            });
        }
        if self.read_tool_fails.load(Ordering::SeqCst) {
            return Err(Error::Unreadable {
                locator: format!("{server}#tool:{tool}"),
                detail: "the server answered 503".to_string(),
            });
        }
        match tool {
            // The strict posture `req/152` §5 asked for: an argument the tool does not declare is
            // refused, so an undo that "succeeds" cannot be the fixture being lax.
            "get_gist" => {
                let object = arguments.as_object().ok_or_else(|| Error::Unreadable {
                    locator: format!("{server}#tool:{tool}"),
                    detail: "the read arguments are not an object".to_string(),
                })?;
                for member in object.keys() {
                    if member != "gist_id" {
                        return Err(Error::Unreadable {
                            locator: format!("{server}#tool:{tool}"),
                            detail: format!(
                                "body failed validation: body.{member} should be not present"
                            ),
                        });
                    }
                }
                if object.get("gist_id").and_then(serde_json::Value::as_str) != Some("g1") {
                    return Err(Error::Unreadable {
                        locator: format!("{server}#tool:{tool}"),
                        detail: "no such gist".to_string(),
                    });
                }
                Ok(self.gist_document().into_bytes())
            }
            other => Err(Error::Unreadable {
                locator: format!("{server}#tool:{other}"),
                detail: format!("this server publishes no tool called {other:?}"),
            }),
        }
    }

    fn call(&self, call: &ToolCall, admitted: &Admitted) -> Result<Vec<u8>> {
        assert_eq!(
            call.delta(),
            admitted.delta(),
            "a transport may check the pair, and this one does"
        );
        let arguments: serde_json::Value =
            serde_json::from_slice(call.arguments()).map_err(|e| Error::ApplyFailed {
                detail: format!("the arguments of {:?} are not JSON: {e}", call.tool()),
            })?;
        self.note("call", format!("{} {arguments}", call.tool()));
        let member = |name: &str| {
            arguments
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        match call.tool() {
            "update_gist" => {
                let filename = member("filename").ok_or_else(|| Error::ApplyFailed {
                    detail: "update_gist has no `filename`".to_string(),
                })?;
                let content = member("content").ok_or_else(|| Error::ApplyFailed {
                    detail: "update_gist has no `content`".to_string(),
                })?;
                self.set_gist(&filename, &content);
                Ok(br#"{"id":"g1","url":"https://api.github.com/gists/g1"}"#.to_vec())
            }
            "create_or_update_file" => {
                let content = member("content").ok_or_else(|| Error::ApplyFailed {
                    detail: "create_or_update_file has no `content`".to_string(),
                })?;
                self.resources
                    .lock()
                    .expect("not poisoned")
                    .insert(FILE.to_string(), content.into_bytes());
                Ok(
                    br#"{"id":"1","url":"https://api.github.com/repos/o/r/contents/notes.txt"}"#
                        .to_vec(),
                )
            }
            other => Err(Error::ApplyFailed {
                detail: format!("this server publishes no tool called {other:?}"),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------

struct Wired {
    server: Arc<Github16Server>,
    adapter: McpAdapter,
    engine: Engine<InjectedEvidence>,
}

fn catalogue_from(bytes: &[u8]) -> Catalogue {
    Catalogue::from_json(bytes).expect("the P0 catalogue fixture parses")
}

fn wire(name: &str, catalogue: Catalogue) -> Wired {
    let server = Arc::new(Github16Server::new());
    let adapter = McpAdapter::new(server.clone())
        .with_catalogue(catalogue)
        .with_log(Arc::new(RewindableLog::new()));
    let gate = gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(PERMIT_ALL).expect("the fixture policy set parses"),
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    let mut engine = Engine::open(dir.join("journal.bin"), gate, InjectedEvidence::none())
        .expect("a fresh journal");
    engine.register_adapter(Arc::new(adapter.clone()), "gx-adapter-mcp github16");
    Wired {
        server,
        adapter,
        engine,
    }
}

fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-github16", &[19u8; 32])
}

fn gist_arguments(content: &str) -> Vec<u8> {
    serde_json::json!({ "gist_id": "g1", "filename": GIST_FILE, "content": content })
        .to_string()
        .into_bytes()
}

fn file_arguments(content: &str) -> Vec<u8> {
    serde_json::json!({
        "owner": "o", "repo": "r", "path": "notes.txt", "branch": "main",
        "message": "the agent's message", "content": content,
    })
    .to_string()
    .into_bytes()
}

/// Drive one change to `Committed`, or hand back what stopped it.
fn commit(
    wired: &mut Wired,
    resource: &str,
    tool: &str,
    arguments: &[u8],
) -> std::result::Result<TransformationId, String> {
    let locator = Github16Server::locator(resource);
    let intent = intent_for(&locator, tool, arguments);
    wired
        .engine
        .submit(&intent, 42, AT)
        .map_err(|e| format!("submit: {e}"))?;
    let id = wired
        .engine
        .plan(&intent, AT)
        .map_err(|e| format!("plan: {e}"))?;
    let verdict = wired
        .engine
        .verify(&id, AT, &signing_key(), None)
        .map_err(|e| format!("verify: {e}"))?;
    if verdict != Lifecycle::Admitted {
        return Err(format!("verify landed on {verdict:?}"));
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

/// Run the undo all the way through its own pipeline: 43 §5-2 exempts nothing.
fn undo_to_committed(wired: &mut Wired, id: &TransformationId) -> std::result::Result<(), String> {
    let witness = wired.engine.attested_postcondition(id);
    assert!(
        matches!(witness, UndoWitness::Attested(_)),
        "the fixture has a resource face, so there is a postcondition to attest: {witness:?}"
    );
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

// ===========================================================================
// Family 1 — the round trip, byte for byte
// ===========================================================================

/// 🔴 **P-1** — every escrow read arrives **before** the effect, and the count is **two**, which is
/// not the number the ruling assumed.
///
/// **E-M4-30**'s physics is the ordering: the escrow is built before `apply` (43 T-10b), so a read
/// after the first `tools/call` would be a read of a prior that no longer exists.
///
/// 🔴 **The count is a finding, and it is written here rather than rounded off.** `req/38` §196's
/// DR-46-12 line says "one extra read per forward call". The measurement says **two**, and the
/// reason is structural rather than accidental: `adapter.invert` is called at **T-3** (verify, to
/// fold `invert_available` into the gate — E-M4-5) and again at **T-10b** (the escrow itself), and
/// each call reads its own prior. That was already true of the `resources/read` road, so the
/// read-by-tool road adds **no new `invert`** — what it does is move those two reads onto a
/// server's tool-call budget. `docs/LIMITS.md` v0.5-d carries the measured number, not the
/// assumed one.
///
/// What must not grow is reads **per invert**: a declaration that read the prior *and* the
/// resource would widen T-10b's window, which is `req/38` §195 clause ⑤'s critical section.
/// [`the_escrow_read_costs_one_round_trip_on_both_roads`] holds that number to one.
///
/// The measurement is the server's own arrival log, not the proxy's counter (`req/119` §5 A-7).
#[test]
fn p1_every_escrow_read_arrives_before_the_effect_and_the_count_is_two() {
    let mut wired = wire("gh16_p1", catalogue_from(P0_CATALOGUE));
    wired.server.clear_log();
    let id = commit(&mut wired, GIST, "update_gist", &gist_arguments(GIST_AFTER))
        .expect("the gist pair commits");

    let arrivals = wired.server.arrivals();
    let first_call = arrivals
        .iter()
        .position(|a| a.kind == "call")
        .expect("the effect reached the server");
    let read_by_tool: Vec<usize> = arrivals
        .iter()
        .enumerate()
        .filter(|(_, a)| a.kind == "read_by_tool")
        .map(|(i, _)| i)
        .collect();
    println!(
        "GH16_P1 id={id:?} arrivals={} first_call={first_call} read_by_tool={read_by_tool:?} \
         log={:?}",
        arrivals.len(),
        arrivals
    );
    assert_eq!(
        read_by_tool.len(),
        2,
        "the number of escrow reads per guarded forward call moved. Two is T-3 (the gate's \
         `invert_available`) plus T-10b (the escrow), and `docs/LIMITS.md` v0.5-d quotes that \
         number; three would mean a new `invert` call site and one would mean the gate stopped \
         being told. Arrivals: {arrivals:?}"
    );
    assert!(
        read_by_tool.iter().all(|i| *i < first_call),
        "a prior was read after the effect ran, which is a prior that no longer exists \
         (E-M4-30 / 43 T-10b). Arrivals: {arrivals:?}"
    );
}

/// 🔴 **P-2** — `update_gist` forward, then undo, and the gist holds the **same bytes** it held.
#[test]
fn p2_the_gist_round_trip_is_byte_identical() {
    let mut wired = wire("gh16_p2", catalogue_from(P0_CATALOGUE));
    let before = wired.server.gist_content();
    let id = commit(&mut wired, GIST, "update_gist", &gist_arguments(GIST_AFTER))
        .expect("the gist pair commits");
    let moved = wired.server.gist_content();
    undo_to_committed(&mut wired, &id).expect("the escrowed inverse runs");
    let after = wired.server.gist_content();
    println!(
        "GH16_P2 before={:?} moved={:?} after={:?} equal={}",
        before,
        moved,
        after,
        before.as_bytes() == after.as_bytes()
    );
    assert_eq!(
        moved, GIST_AFTER,
        "the forward call has to have moved the world before an undo means anything"
    );
    assert_eq!(
        before.as_bytes(),
        after.as_bytes(),
        "the undo did not put the prior bytes back, byte for byte"
    );
}

/// 🔴 **P-3** — the same round trip with CJK, an astral emoji and a combining mark.
///
/// The `prior_json` road parses the prior as JSON and hands one member back as a `serde_json`
/// string, so this is the arm that would catch a re-encoding, a normalisation, or a byte slice
/// taken at a non-boundary anywhere between the read tool and the restore call.
#[test]
fn p3_the_round_trip_survives_cjk_and_an_astral_emoji() {
    let mut wired = wire("gh16_p3", catalogue_from(P0_CATALOGUE));
    wired.server.set_gist(GIST_FILE, GIST_CJK);
    let before = wired.server.gist_content();
    let id = commit(&mut wired, GIST, "update_gist", &gist_arguments(GIST_AFTER))
        .expect("the gist pair commits");
    undo_to_committed(&mut wired, &id).expect("the escrowed inverse runs");
    let after = wired.server.gist_content();
    println!(
        "GH16_P3 before_bytes={} after_bytes={} equal={}",
        before.len(),
        after.len(),
        before.as_bytes() == after.as_bytes()
    );
    assert_eq!(before, GIST_CJK, "the fixture seeded what it meant to seed");
    assert_eq!(
        before.as_bytes(),
        after.as_bytes(),
        "a UTF-8 round trip lost bytes: {before:?} came back as {after:?}"
    );
}

/// 🔴 **P-4 (DR-46-10)** — `prior_json` hands over **one member**, not the document it came in.
///
/// The discrimination this arm exists for: `prior_contents_utf8` over a read tool's answer would
/// resolve to the whole `{id, description, files: …}` document, and a restore call built that way
/// would write the document *into* the gist and report success. The fixture's document carries a
/// `description` that is nowhere in the content, so "the pointer resolved" and "the whole answer
/// travelled" cannot look the same.
#[test]
fn p4_prior_json_resolves_one_member_and_not_the_whole_document() {
    let wired = wire("gh16_p4", catalogue_from(P0_CATALOGUE));
    let locator = Github16Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let delta = wired
        .adapter
        .plan(
            &intent_for(&locator, "update_gist", &gist_arguments(GIST_AFTER)),
            &pre,
        )
        .expect("a well-formed call plans");
    // 🔴 **DR-46-26** — the outcome, not only its inverse. The lane's red phase (`req/38` §226)
    // broke `invert.rs`'s `locator: position.resource()` into a literal naming another object and
    // **nothing went red**: the read-set had no adversarial assertion anywhere on the MCP road,
    // where the value is actually produced. This is it.
    //
    // The two clauses are the two halves of an entry. `locator` has to be the object the escrow is
    // *about* — DR-46-15's binding, which this same function refuses a call over when the declared
    // read answers for something else — and `digest` has to be the digest of what the read
    // answered, taken through the same `content_digest` `snapshot` uses, or a verifier holding the
    // receipt and the object cannot compare them.
    let outcome = wired
        .adapter
        .invert(&delta, &pre)
        .expect("the read face answers");
    let reads = outcome.reads();
    println!(
        "GH16_P4_READS n={} entries={:?}",
        reads.len(),
        reads.iter().map(|e| e.locator.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(
        reads.len(),
        1,
        "`req/350` §1 measured one object per escrow on this road"
    );
    assert_eq!(
        reads[0].locator,
        gx_adapter_mcp::locator::parse(&locator)
            .expect("the fixture's locator parses")
            .resource(),
        "DR-46-26: the attested locator is the object the escrow is about (DR-46-15's binding),         not whatever a declared tool called it"
    );
    // The digest is of what the **prior** read answered, and the two things it must not be are the
    // two ways this could be wrong: the digest of nothing (a read that did not answer, attested
    // anyway) and the digest of the forward content (the post-state attested as the prior, which is
    // exactly the confusion T-10b's ordering exists to prevent).
    assert_ne!(
        reads[0].digest,
        gx_adapter_mcp::adapter::content_digest(&[]),
        "a read that answered nothing was attested as though it had"
    );
    assert_ne!(
        reads[0].digest,
        gx_adapter_mcp::adapter::content_digest(GIST_AFTER.as_bytes()),
        "the post-state was attested as the prior; T-10b runs before `apply` for this reason"
    );
    let inverse = outcome.into_inverse().expect("an inverse was constructed");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter wrote it");
    let op = decoded.ops().first().expect("one op");
    let arguments: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("the template resolved to JSON");
    println!("GH16_P4 restore_tool={} arguments={arguments}", op.tool());
    assert_eq!(op.tool(), "update_gist");
    assert_eq!(
        arguments.get("content").and_then(serde_json::Value::as_str),
        Some(GIST_BEFORE),
        "the restore's `content` is the file member the pointer names"
    );
    let carried = arguments.to_string();
    assert!(
        !carried.contains("a description the restore call must not send as the content"),
        "the whole read document travelled into the restore call: {carried}"
    );
}

/// 🔴 **P-5** — the `resources/read` road is **unchanged**: `create_or_update_file` still takes it,
/// and the read-by-tool face is never touched for a declaration that names none.
///
/// The regression arm. Every catalogue shipped before this window declares no `read_by`, so the
/// number that has to stay zero is the one below; a lane that made the new road the default would
/// break every deployment whose transport has no read-by-tool face at all.
#[test]
fn p5_a_declaration_with_no_read_by_still_takes_resources_read() {
    let mut wired = wire("gh16_p5", catalogue_from(P0_CATALOGUE));
    wired.server.clear_log();
    let id = commit(
        &mut wired,
        FILE,
        "create_or_update_file",
        &file_arguments("the agent's file content\n"),
    )
    .expect("the contents pair commits");
    undo_to_committed(&mut wired, &id).expect("the escrowed inverse runs");

    let by_tool = wired.server.count("read_by_tool");
    let reads = wired.server.count("read");
    println!(
        "GH16_P5 reads={reads} read_by_tool={by_tool} contents={:?}",
        String::from_utf8_lossy(
            &wired
                .server
                .read(SERVER, FILE)
                .expect("the file is readable")
        )
    );
    assert_eq!(
        by_tool, 0,
        "a declaration with no `read_by` reached the read-by-tool face, which is a road it never \
         asked for"
    );
    assert!(reads > 0, "the `resources/read` road was not taken at all");
    assert_eq!(
        wired.server.read(SERVER, FILE).expect("readable"),
        FILE_BEFORE.as_bytes(),
        "the contents pair's own round trip"
    );
}

// ===========================================================================
// Family 2 — the CAS, and the two tools that stay false
// ===========================================================================

/// 🔴 **P-6** — a gist somebody else rewrote between the escrow and the undo is **not** overwritten.
///
/// `req/265` §2-2 named this the most dangerous boundary on the whole target: GitHub's issue and
/// gist update endpoints accept no `If-Match`, so **the server side has no precondition at all**
/// and the only thing standing between an undo and a third party's work is gx's own CAS. This arm
/// is that CAS on the read-by-tool road.
#[test]
fn p6_a_third_party_write_refuses_the_undo_rather_than_overwriting_it() {
    let mut wired = wire("gh16_p6", catalogue_from(P0_CATALOGUE));
    let id = commit(&mut wired, GIST, "update_gist", &gist_arguments(GIST_AFTER))
        .expect("the gist pair commits");

    const THIRD_PARTY: &str = "somebody else was editing this gist\n";
    wired.server.third_party_writes_gist(THIRD_PARTY);

    let witness = wired.engine.attested_postcondition(&id);
    let refused = wired
        .engine
        .undo(&id, &witness, 44, UNDO_AT)
        .expect_err("DR-43-1(a): the world moved");
    println!(
        "GH16_P6 kind={} gist={:?}",
        refused.kind(),
        wired.server.gist_content()
    );
    assert_eq!(
        refused.kind(),
        "WorldMoved",
        "the undo went ahead over a third party's write, which is the one accident this whole \
         lane exists to prevent"
    );
    assert_eq!(
        wired.server.gist_content(),
        THIRD_PARTY,
        "the third party's bytes were disturbed by a refused undo"
    );
}

/// 🔴 **P-7** — an undo of a world nobody moved **succeeds**, so P-6 is a discrimination and not a
/// fail-closed that refuses everything.
#[test]
fn p7_an_undisturbed_world_lets_the_undo_through() {
    let mut wired = wire("gh16_p7", catalogue_from(P0_CATALOGUE));
    let id = commit(&mut wired, GIST, "update_gist", &gist_arguments(GIST_AFTER))
        .expect("the gist pair commits");
    let outcome = undo_to_committed(&mut wired, &id);
    println!(
        "GH16_P7 outcome={outcome:?} gist={:?} status={:?}",
        wired.server.gist_content(),
        wired.engine.inverse_status(&id)
    );
    outcome.expect("an undisturbed world is not a refusal");
    assert_eq!(wired.server.gist_content(), GIST_BEFORE);
    assert!(
        matches!(
            wired.engine.inverse_status(&id),
            Some(InverseStatus::Consumed { .. })
        ),
        "43 T-12 seats the escrow as consumed once its undo commits"
    );
}

/// 🔴 **P-8 (C-25)** — `update_pull_request` and `update_pull_request_branch` are **false**, and the
/// catalogue file is held to that.
///
/// Two mechanisms, and neither of them is "we ran out of time" (`req/265` §3-3's failure classes):
///
/// * **compound** — `update_pull_request` carries `reviewers`, whose semantics are *add*. The same
///   tool called again does not remove what it added, so no declaration written in that tool can
///   be the inverse of a call that used the field.
/// * **no inverse on the surface** — `update_pull_request_branch` creates a merge commit and the
///   inverse is a force-push. github-mcp-server v1.9.0 publishes no force-push tool among its 116
///   (`req/265` §1, upstream `__toolsnaps__`), so there is nothing to name.
///
/// A catalogue *could* declare them; nothing stops an operator writing a lie. What the fixture
/// does is not write one, and what this arm does is make the absence a measured fact rather than
/// an omission a later lane quietly fills.
#[test]
fn p8_update_pull_request_and_branch_are_false_by_mechanism() {
    let catalogue = catalogue_from(P0_CATALOGUE);
    let verdicts: Vec<(&str, &str)> = [
        "create_or_update_file",
        "update_gist",
        "update_pull_request",
        "update_pull_request_branch",
    ]
    .into_iter()
    .map(|tool| (tool, catalogue.declared_reversibility(tool).as_str()))
    .collect();
    println!(
        "GH16_P8 declared={} verdicts={verdicts:?}",
        catalogue.declared()
    );
    assert_eq!(
        catalogue.declared(),
        2,
        "P0 is four tools and exactly two of them have an inverse; a catalogue with more rows \
         than that is claiming something this lane did not establish"
    );
    assert_eq!(
        verdicts,
        vec![
            ("create_or_update_file", "true"),
            ("update_gist", "true"),
            ("update_pull_request", "false"),
            ("update_pull_request_branch", "false"),
        ]
    );
    assert_eq!(
        Reversibility::ALL,
        ["true", "false", "unknown"],
        "C-25's three values, in the order 11 §5-2 states them"
    );
}

/// 🔴 **P-9 (DR-46-14, `req/38` §198 ruling (c) / §199 ruling 2)** — a `prior_json` pointer bound
/// to the forward call follows **the member the call touched**, and answers nothing when that
/// member has no prior.
///
/// # What this probe measured before this window, and why it changed
///
/// `req/268` shipped this arm as a **finding**: RFC 6901 has no variables, so a literal
/// `"/files/notes.md/content"` beside `"filename": {"forward": "filename"}` resolved **anyway**
/// for a forward call against `other.md` — `notes.md` was still in the document — and gx answered
/// `true` while building a restore that carried `other.md`'s coordinate with `notes.md`'s text.
/// That is not an inverse. The finding was handed up rather than fixed, became **DR-46-14**
/// (`req/38` §198 ruling (c)), and `req/38` §199 put it in this lane beside DR-46-15 so that the
/// frozen vocabulary is opened once rather than twice.
///
/// The fix is a pointer whose segments come from the forward call
/// (`["/files/", {"forward": "filename"}, "/content"]`), which is what the fixture now declares.
/// The two arms below are the two answers that replace the one wrong one:
///
/// * a call against a member the document **has** builds that member's own inverse;
/// * a call against a member the document **does not have** resolves to nothing, and the third
///   `Ok(None)` of `catalogue.rs`'s header answers — `false`, and **E-M3-4** asks a person.
///
/// 🔴 What is **not** closed by this, and is the same burden `req/38` §102 ruling 2 left with the
/// deployment: a declaration may still bind the pointer to the *wrong* forward member. gx checks
/// that the pointer is built from this call, not that the deployment picked the right member to
/// build it from.
#[test]
fn p9_a_bound_pointer_follows_the_member_the_call_touched() {
    let wired = wire("gh16_p9", catalogue_from(P0_CATALOGUE));
    wired.server.set_gist(
        "other.md",
        "another file's own text
",
    );
    let locator = Github16Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);

    // Arm 1: a member the document carries.
    let arguments = serde_json::json!({ "gist_id": "g1", "filename": "other.md", "content": "x" })
        .to_string()
        .into_bytes();
    let delta = wired
        .adapter
        .plan(&intent_for(&locator, "update_gist", &arguments), &pre)
        .expect("a well-formed call plans");
    let verdict = wired
        .adapter
        .reversibility(&delta, &pre)
        .expect("the read face answers");
    let inverse = wired
        .adapter
        .invert(&delta, &pre)
        .expect("no read failure")
        .into_inverse()
        .expect("the bound pointer resolved against the member the call touched");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter wrote it");
    let op = decoded.ops().first().expect("one op");
    let restore: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("the template resolved to JSON");

    // Arm 2: a member the document does not carry.
    let absent =
        serde_json::json!({ "gist_id": "g1", "filename": "not-in-the-gist.md", "content": "x" })
            .to_string()
            .into_bytes();
    let absent_delta = wired
        .adapter
        .plan(&intent_for(&locator, "update_gist", &absent), &pre)
        .expect("a well-formed call plans");
    let absent_verdict = wired
        .adapter
        .reversibility(&absent_delta, &pre)
        .expect("the read face answers");
    let absent_inverse = wired
        .adapter
        .invert(&absent_delta, &pre)
        .expect("no read failure")
        .into_inverse();

    println!(
        "GH16_P9 verdict={} restore={restore} absent_verdict={} absent_inverse={:?}",
        verdict.as_str(),
        absent_verdict.as_str(),
        absent_inverse.is_some()
    );
    assert_eq!(
        verdict,
        Reversibility::True,
        "the call names a member the prior document carries, so the inverse exists"
    );
    assert_eq!(
        restore.get("filename").and_then(serde_json::Value::as_str),
        Some("other.md"),
        "the restore lands on the file the forward call named"
    );
    assert_eq!(
        restore.get("content").and_then(serde_json::Value::as_str),
        Some("another file's own text
"),
        "🔴 DR-46-14: the restore has to carry **that member's** prior text. Before this window          it carried `notes.md`'s, with `true` beside it -- a wrong call reported as an inverse"
    );
    assert_eq!(
        absent_verdict,
        Reversibility::False,
        "a member the prior document does not carry has no inverse to build, and `false` with an          escalation is the answer -- never a pointer quietly resolving somewhere else"
    );
    assert!(
        absent_inverse.is_none(),
        "an inverse was built for a member the prior never had"
    );
}

// ===========================================================================
// Family 3 — the read that fails
// ===========================================================================

/// 🔴 **P-10 (DR-46-12)** — a prior that will not be read means **the effect does not happen**.
///
/// The count that carries it is on the server's side of the wire: zero `tools/call` arrivals. A
/// proxy that applied first and discovered the escrow was impossible afterwards would have moved
/// a world it cannot move back, which is the failure this whole product is a wedge in front of.
#[test]
fn p10_a_read_that_fails_denies_the_effect() {
    let mut wired = wire("gh16_p10", catalogue_from(P0_CATALOGUE));
    wired.server.break_read_tool();
    wired.server.clear_log();
    let outcome = commit(&mut wired, GIST, "update_gist", &gist_arguments(GIST_AFTER));
    let calls = wired.server.count("call");
    println!(
        "GH16_P10 outcome={outcome:?} calls={calls} gist={:?}",
        wired.server.gist_content()
    );
    assert!(
        outcome.is_err(),
        "the commit went through without an escrow"
    );
    assert_eq!(
        calls,
        0,
        "the effect reached the server anyway: {:?}",
        wired.server.arrivals()
    );
    assert_eq!(
        wired.server.gist_content(),
        GIST_BEFORE,
        "the world moved under a refused commit"
    );
}

/// 🔴 **P-11** — the refusal says what to do, in the words the page says it in.
///
/// `req/263` §6 G6 measured that a refusal is worth having only when the remedy it names can be
/// executed without a reader adding a word, so the sentence is a constant and this arm holds the
/// refusal to it verbatim. Both remedies have to be in it: a reader who cannot fix the read face
/// still has a documented way forward, and one who does not want the relaxation still knows it
/// exists.
#[test]
fn p11_the_refusal_names_both_remedies_verbatim() {
    let wired = wire("gh16_p11", catalogue_from(P0_CATALOGUE));
    wired.server.break_read_tool();
    let locator = Github16Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let delta = wired
        .adapter
        .plan(
            &intent_for(&locator, "update_gist", &gist_arguments(GIST_AFTER)),
            &pre,
        )
        .expect("a well-formed call plans");
    let refused = wired
        .adapter
        .invert(&delta, &pre)
        .expect_err("fail-closed is the default");
    let text = refused.to_string();
    println!("GH16_P11 refusal={text}");
    assert!(
        text.contains(READ_FAILURE_REFUSAL),
        "the refusal is not the constant docs/LIMITS.md quotes:\n{text}"
    );
    for remedy in [
        "make the declared read face answer",
        "\"$on_read_failure\": \"unknown\"",
    ] {
        assert!(
            text.contains(remedy),
            "the refusal does not carry the remedy {remedy:?}:\n{text}"
        );
    }
    assert!(
        text.contains("the server answered 503"),
        "the refusal drops what the server actually said, which is the half a reader debugs \
         with:\n{text}"
    );
}

/// 🔴 **P-12 (DR-46-9 A-4)** — the opt-in calls the reversibility **unknown** and still puts the
/// change in front of a person; it does not open a quiet road.
///
/// C-25's third value, earned: nothing in this run established that no inverse exists — the prior
/// was simply never read. Reporting `false` would be a claim about the change; reporting `true`
/// would be a lie.
///
/// 🔴 **The measured shape of the opt-in is stronger than the ruling asked for, and that is worth
/// naming**: `$on_read_failure: "unknown"` does **not** make the effect sail through. `invert`
/// answers `None`, so **E-M3-4** escalates and `verify` lands on `Escalated` — a person has to
/// rule (43 T-5) before anything reaches the server. So the relaxation buys "a human may allow
/// this", not "gx stops asking". The arm below drives both halves: the escalation, and then the
/// committed transformation whose escrow row is `Unavailable` and whose undo is refused by name.
#[test]
fn p12_the_opt_in_records_unknown_and_still_asks_a_person() {
    let mut wired = wire("gh16_p12", catalogue_from(P0_CATALOGUE_UNKNOWN));
    wired.server.break_read_tool();
    let locator = Github16Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let delta = wired
        .adapter
        .plan(
            &intent_for(&locator, "update_gist", &gist_arguments(GIST_AFTER)),
            &pre,
        )
        .expect("a well-formed call plans");
    let verdict = wired
        .adapter
        .reversibility(&delta, &pre)
        .expect("the opt-in does not refuse");

    // The pipeline, by hand, because the interesting step is the one `commit()` does not take.
    let intent = intent_for(&locator, "update_gist", &gist_arguments(GIST_AFTER));
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
            key: "key-operator-1".to_string(),
        },
    };
    let after_ruling = wired
        .engine
        .escalation(&id, &ruling, AT, &signing_key())
        .expect("43 T-5");
    wired.engine.canonicalize(&id, AT, None).expect("T-8");
    let after_commit = wired.engine.commit(&id, AT, &signing_key()).expect("T-11");

    let status = wired.engine.inverse_status(&id);
    let witness = wired.engine.attested_postcondition(&id);
    let refused = wired.engine.undo(&id, &witness, 45, UNDO_AT);
    println!(
        "GH16_P12 verdict={} verify={after_verify:?} ruling={after_ruling:?} \
         commit={after_commit:?} status={status:?} undo_err={:?} gist={:?}",
        verdict.as_str(),
        refused.as_ref().err().map(gx_engine::Error::kind),
        wired.server.gist_content()
    );
    assert_eq!(
        verdict,
        Reversibility::Unknown,
        "an unread prior is not evidence that no inverse exists"
    );
    assert_eq!(
        after_verify,
        Lifecycle::Escalated,
        "the opt-in has to keep E-M3-4 in the road: a relaxation that admitted straight through \
         would be the silent fail-open DR-46-12 refused"
    );
    assert_eq!(after_ruling, Lifecycle::Admitted);
    assert_eq!(after_commit, Lifecycle::Committed);
    assert_eq!(
        wired.server.gist_content(),
        GIST_AFTER,
        "the opt-in is only meaningful if the effect actually happened"
    );
    // 🔴 **DR-46-26 turns this assertion over, and the old one was the defect measured from the
    // other side.** Until this lane the row answered `Unavailable` here — "we asked and there is
    // none" — for a commit whose verdict was `Reversibility::Unknown`, i.e. *nobody found out*.
    // That is `req/38` §198 ruling (b) exactly: the third value reached the adapter's return, the
    // refusal sentence and the probe, and was flattened into the second everywhere below. The seat
    // (D24) and the writer (DR-46-26) now carry it through, so the row says which of the two this
    // was, and the receipt does too.
    assert_eq!(
        status,
        Some(InverseStatus::Undetermined),
        "the escrow row has to say an inverse was asked for and **nobody established** whether one         exists -- `Unavailable` here would be the fold DR-46-13 was raised about"
    );
    assert!(
        refused.is_err(),
        "an undo ran against a transformation that never escrowed one"
    );
}

/// 🔴 **P-13** — a transport with **no** read-by-tool face refuses, and never answers `true`.
///
/// The default implementation of `ToolTransport::read_prior_by_tool` is a refusal, so a deployment
/// that upgrades gx without writing one keeps the behaviour it had: its catalogues declare no
/// `read_by` and never reach the road. This arm drives the other case — a catalogue that *does*
/// declare one against a transport that has none — and measures both postures, so the answer is
/// "refused" or "unknown" and never a silent `true`.
#[test]
fn p13_a_transport_with_no_read_face_is_refused_or_unknown_never_true() {
    #[derive(Debug)]
    struct ResourcesOnly;
    impl ToolTransport for ResourcesOnly {
        fn read(&self, _server: &str, _resource: &str) -> Result<Vec<u8>> {
            Ok(GIST_BEFORE.as_bytes().to_vec())
        }
        fn call(&self, _call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
            Ok(b"{}".to_vec())
        }
    }

    let read = PriorRead::new(
        "get_gist",
        RestoreTemplate::new().with("gist_id", ArgSource::Forward("gist_id".to_string())),
        ObjectIdentity::new(vec![
            IdentityPart::Literal("gist:".to_string()),
            IdentityPart::Answer {
                answer: "/id".to_string(),
            },
        ]),
    );
    let template = RestoreTemplate::new()
        .with("gist_id", ArgSource::Forward("gist_id".to_string()))
        .with(
            "content",
            ArgSource::PriorJson(PriorPointer::Bound(vec![
                PointerSegment::Literal("/files/".to_string()),
                PointerSegment::Forward {
                    forward: "filename".to_string(),
                },
                PointerSegment::Literal("/content".to_string()),
            ])),
        );

    let locator = Github16Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let mut answers = Vec::new();
    for posture in [OnReadFailure::Refuse, OnReadFailure::Unknown] {
        let catalogue = Catalogue::new()
            .with_restore_template("update_gist", "update_gist", template.clone())
            .with_prior_read("update_gist", read.clone())
            .with_on_read_failure(posture);
        let adapter = McpAdapter::new(Arc::new(ResourcesOnly)).with_catalogue(catalogue);
        let delta = adapter
            .plan(
                &intent_for(&locator, "update_gist", &gist_arguments(GIST_AFTER)),
                &pre,
            )
            .expect("a well-formed call plans");
        answers.push(match adapter.reversibility(&delta, &pre) {
            Ok(verdict) => verdict.as_str().to_string(),
            Err(e) => format!("refused:{}", e.kind()),
        });
        assert!(
            adapter
                .invert(&delta, &pre)
                .ok()
                .and_then(gx_substrate::InvertOutcome::into_inverse)
                .is_none(),
            "an inverse was built out of a prior nobody read"
        );
    }
    println!("GH16_P13 answers={answers:?}");
    assert_eq!(answers, vec!["refused:Unreadable", "unknown"]);
    assert_eq!(
        SubstrateKind::Mcp,
        gx_adapter_mcp::McpAdapter::new(Arc::new(ResourcesOnly)).kind()
    );
}

// ===========================================================================
// The measurement `req/38` §196 asked this lane to report
// ===========================================================================

/// The cost of the escrow read, in this fixture, with its denominator printed beside it.
///
/// 🔴 **What this number is not**: a network measurement. The transport here is in-process, so what
/// is measured is the *shape* of the road — the read-by-tool path against the `resources/read` path
/// over the same escrow, with the same one round trip each. The claim the lane makes from it is
/// the one arithmetic supports: **one read per forward call, on both roads, so the escrow adds no
/// round trip that `resources/read` did not already add** — and the rate-limit consequence
/// (`docs/LIMITS.md` v0.5-d) follows from the count, not from the microseconds.
#[test]
fn the_escrow_read_costs_one_round_trip_on_both_roads() {
    const RUNS: usize = 200;
    let wired = wire("gh16_latency", catalogue_from(P0_CATALOGUE));
    let gist_locator = Github16Server::locator(GIST);
    let file_locator = Github16Server::locator(FILE);
    let gist_pre = support::absent_snapshot(&gist_locator);
    let file_pre = support::absent_snapshot(&file_locator);
    let gist_delta = wired
        .adapter
        .plan(
            &intent_for(&gist_locator, "update_gist", &gist_arguments(GIST_AFTER)),
            &gist_pre,
        )
        .expect("plans");
    let file_delta = wired
        .adapter
        .plan(
            &intent_for(
                &file_locator,
                "create_or_update_file",
                &file_arguments("x\n"),
            ),
            &file_pre,
        )
        .expect("plans");

    let mut by_tool = Vec::with_capacity(RUNS);
    let mut by_resource = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = wired
            .adapter
            .invert(&gist_delta, &gist_pre)
            .expect("invert");
        by_tool.push(start.elapsed().as_nanos());
        let start = Instant::now();
        let _ = wired
            .adapter
            .invert(&file_delta, &file_pre)
            .expect("invert");
        by_resource.push(start.elapsed().as_nanos());
    }
    by_tool.sort_unstable();
    by_resource.sort_unstable();

    wired.server.clear_log();
    let _ = wired
        .adapter
        .invert(&gist_delta, &gist_pre)
        .expect("invert");
    let one_escrow = wired.server.arrivals();

    println!(
        "GH16_ESCROW_READ_LATENCY runs={RUNS} read_by_tool_median_ns={} resources_read_median_ns={} \
         reads_per_escrow={:?}",
        by_tool[RUNS / 2],
        by_resource[RUNS / 2],
        one_escrow
    );
    assert_eq!(
        one_escrow.len(),
        1,
        "one escrow, one read -- the count is the claim, and it is the one the rate-limit line in \
         docs/LIMITS.md rests on: {one_escrow:?}"
    );
    assert_eq!(one_escrow[0].kind, "read_by_tool");
}
