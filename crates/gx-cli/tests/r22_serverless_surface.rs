// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/303` M-01** (`req/309` §1 item 3) — the start-up line of a serverless `gx serve` names
//! every verb that surface refuses, and the set is **enumerated from the router** rather than
//! assumed.
//!
//! # What the twenty-first adversarial audit measured, verbatim
//!
//! ```text
//! A21_M02_START mcp={"server":null,"restorable_tools":0,
//!   "note":"no server named: an MCP ruling, undo, cancel, or verify on this surface is refused (...)"}
//! A21_M02 verb=commit  status=502 no_server=true      <- not named by the declaration
//! A21_M02_REFUSED=["verify", "commit", "ruling", "cancel", "undo"]
//! ```
//!
//! # Why R20's arm did not catch it, and what changed here
//!
//! R20 repaired the same sentence one verb narrower and held it with an arm that **asserted the
//! Given**: it put four verbs against the surface, asserted all four were 502, and then required the
//! sentence to name those four. `POST /candidates/{id}/commit` is the fifth `with_a_body` caller and
//! nobody asked it. `req/303` §6-4 named the repair — enumerate the set of handlers that call
//! `with_a_body` from the source and match it against the start-up line, so the day a sixth is
//! added the arm goes red — and `req/38` §221 ruling 4 adopted it.
//!
//! So this file does not put a list of verbs against the surface. It reads
//! `crates/gx-api/src/handlers.rs`, finds every handler that calls `with_a_body`, and requires the
//! sentence to account for each one. A sixth caller added later is red here on the day it is added,
//! with no one having to remember.
//!
//! Read rather than linked for `r20_refusal_vocabulary_is_whole.rs`'s reason: `gx-api` is not this
//! lane's to write, and a probe that named its symbols would turn a defect's red into a symbol
//! table's red.
//!
//! `cfg(unix)` for `SIGTERM`, as every `serve_runtime_r*.rs` says.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

const READ_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_WAIT: Duration = Duration::from_secs(30);
const BEFORE: &str = "the note as it stood before any agent touched it\n";
const AFTER: &str = "what the agent wrote through gx wrap\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ENDPOINT: &str = "stdio://r22s";

/// 🔴 The word each `with_a_body` handler is called by on the surface an operator reads.
///
/// The **set** of handlers is enumerated from `handlers.rs`; this is the translation from a Rust
/// function name to the noun the start-up line uses, and it is declared here because it is a fact
/// about English rather than about the router — `escalate_candidate` is *"an MCP ruling"* on that
/// line and has been since R19. [`the_start_up_line_accounts_for_every_handler_that_needs_a_body`]
/// requires every enumerated handler to have a row, so a sixth caller is red for *this* table too:
/// its verb has to be named before the sentence can be checked.
const HANDLER_WORDS: &[(&str, &str)] = &[
    ("verify_candidate", "verify"),
    ("commit_candidate", "commit"),
    ("escalate_candidate", "ruling"),
    ("cancel_candidate", "cancel"),
    ("undo_transformation", "undo"),
];

/// Every `pub async fn` in `handlers.rs` whose body calls `with_a_body`, in source order.
///
/// The scan is deliberately crude and deliberately loud: it walks the file once, remembers the most
/// recent `pub async fn` header, and records that name the first time a `with_a_body(` call appears
/// beneath it. [`the_scan_of_the_router_found_something`] refuses to let it pass by finding nothing.
fn handlers_that_need_a_body(source: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub async fn ") {
            let name = rest
                .split('(')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            current = Some(name);
            continue;
        }
        if trimmed.contains("with_a_body(") && !trimmed.starts_with("fn ") {
            if let Some(name) = current.take() {
                found.push(name);
            }
        }
    }
    found
}

fn handlers_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-api/src/handlers.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// The instrument's own guard
// ---------------------------------------------------------------------------

/// 🔴 The one way this file passes while measuring nothing: the scan drifts and finds no handler at
/// all, so the loop below has nothing to check. Held first, and by a floor rather than by an exact
/// count — an exact count would have to move for a legitimate sixth caller, which is the event this
/// file exists to notice.
#[test]
fn the_scan_of_the_router_found_something() {
    let found = handlers_that_need_a_body(&handlers_source());
    println!("R22_M01_SCAN n={} {:?}", found.len(), found);
    assert!(
        found.len() >= 5,
        "🔴 the scan of `gx-api/src/handlers.rs` found {} handlers calling `with_a_body`, and this \
         repository is known to have five. The scan has drifted and would now pass by finding \
         nothing — it must be rewritten with whatever rewrote the router: {found:?}",
        found.len()
    );
}

/// 🔴 Every enumerated handler has a word on the surface. A sixth caller with no row here is red
/// **before** anyone starts a server, which is the cheapest place to be told.
#[test]
fn every_handler_that_needs_a_body_has_a_word_an_operator_would_read() {
    let found = handlers_that_need_a_body(&handlers_source());
    let missing: Vec<&String> = found
        .iter()
        .filter(|name| !HANDLER_WORDS.iter().any(|(fn_name, _)| fn_name == name))
        .collect();
    println!("R22_M01_WORDS found={found:?} unmapped={missing:?}");
    assert!(
        missing.is_empty(),
        "🔴 `req/303` M-01: {missing:?} calls `with_a_body`, so it is refused by a `gx serve` that \
         named no MCP server, and this file has no name for it. Give it one here and then check \
         that the start-up line says it"
    );
}

// ---------------------------------------------------------------------------
// The surface
// ---------------------------------------------------------------------------

struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<ChildStdout>,
    start_line: Value,
}

impl Serving {
    fn start(project: &Path, home: &Path, key_id: &str) -> Self {
        let token = "r22-surface-token".to_string();
        let token_file = project.join("token");
        std::fs::write(&token_file, format!("{token}\n")).expect("write the token file");

        let mut command = Command::new(env!("CARGO_BIN_EXE_gx"));
        command
            .env("HOME", home)
            .env("USERPROFILE", home)
            .arg("--project")
            .arg(project)
            .arg("serve")
            .args(["--bind", "127.0.0.1:0"])
            .arg("--token-file")
            .arg(&token_file)
            .args(["--signing-key", key_id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("gx serve starts");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut line = String::new();
        let deadline = Instant::now() + STARTUP_WAIT;
        while line.trim().is_empty() && Instant::now() < deadline {
            line.clear();
            if stdout.read_line(&mut line).unwrap_or(0) == 0 {
                let mut why = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut why);
                }
                let _ = child.wait();
                panic!("gx serve was expected to serve and did not: {why}");
            }
        }
        let start: Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("44 §1.2 asks for one start-up line; got {line:?} ({e})"));
        let addr = start["bind"].as_str().expect("bound address").to_string();
        Self {
            child,
            addr,
            token,
            stdout,
            start_line: start,
        }
    }

    fn json(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
        let mut socket = TcpStream::connect(&self.addr).expect("connect to the server");
        socket
            .set_read_timeout(Some(READ_TIMEOUT))
            .expect("a read timeout, so an expiry is a failure and not a hang");
        let payload = body.map(|v| serde_json::to_vec(v).expect("serialises"));
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n",
            self.addr, self.token
        );
        if let Some(bytes) = &payload {
            head.push_str("Content-Type: application/json\r\n");
            head.push_str(&format!("Content-Length: {}\r\n", bytes.len()));
        }
        head.push_str("\r\n");
        socket.write_all(head.as_bytes()).expect("write the head");
        if let Some(bytes) = &payload {
            socket.write_all(bytes).expect("write the body");
        }
        socket.flush().expect("flush");
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).expect("read the answer");
        let text = String::from_utf8_lossy(&raw).to_string();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let body = text
            .split_once("\r\n\r\n")
            .map_or(String::new(), |(_, b)| b.to_string());
        (
            status,
            serde_json::from_str(&body).unwrap_or(Value::String(body)),
        )
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status();
        let _ = self.child.wait();
        let mut line = String::new();
        while self.stdout.read_line(&mut line).unwrap_or(0) > 0 {
            line.clear();
        }
    }
}

/// One `notes.write` through a `gx wrap` that declares no restore for it: E-M3-4 escalates it, and
/// the process then exits — so the row is one no later process holds in its live table, which is
/// what makes the HTTP verbs below take the rebuild road that snapshots.
fn escalate_one_mcp_row(pipeline: &support::Pipeline, uri: &str) -> String {
    let args: Vec<String> = vec![
        "--project".into(),
        pipeline.project.display().to_string(),
        "wrap".into(),
        "--endpoint".into(),
        ENDPOINT.into(),
        "--actor-key".into(),
        pipeline.key_id.clone(),
        "--actor-model".into(),
        "r22-probe".into(),
        "--".into(),
        env!("CARGO_BIN_EXE_gx").to_string(),
        DEMO_SERVER_ARG.into(),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
        .env("HOME", &pipeline.home)
        .env("USERPROFILE", &pipeline.home)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the gx binary runs");
    let mut stdin: Option<ChildStdin> = child.stdin.take();
    let mut stdout = BufReader::new(child.stdout.take().expect("piped"));
    let mut n = 0u64;
    let mut ask = |method: &str, params: Value, stdin: &mut Option<ChildStdin>| -> Value {
        n += 1;
        let frame = jsonrpc::request(n, method, params);
        jsonrpc::write_frame(stdin.as_mut().expect("open"), &frame).expect("write");
        let line = jsonrpc::read_frame(&mut stdout)
            .expect("read")
            .expect("gx wrap answers");
        serde_json::from_str(&line).expect("JSON")
    };
    ask(
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "r22s", "version": "0" },
        }),
        &mut stdin,
    );
    let note = jsonrpc::notification("notifications/initialized", json!({}));
    jsonrpc::write_frame(stdin.as_mut().expect("open"), &note).expect("write");
    let answered = ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": uri, "contents": AFTER } }),
        &mut stdin,
    );
    drop(stdin);
    let _ = child.wait_with_output();
    let meta = &answered["result"]["_meta"];
    assert_eq!(
        meta["gx/verdict"], "Escalate",
        "the Given is an MCP row raised by a process that has now exited: {answered}"
    );
    meta["gx/transformation"]
        .as_str()
        .expect("a transformation id")
        .to_string()
}

/// 🔴 **`req/303` M-01** — the sentence accounts for **every** handler the router routes through
/// `with_a_body`, and each named verb is checked against the running surface.
///
/// The order matters and is the repair: the handler set comes from the source, then the sentence is
/// required to contain each one's word, then the surface is asked and its answer recorded. Nothing
/// here asserts "these four are 502" as a Given, which is what let the fifth through.
#[test]
fn the_start_up_line_accounts_for_every_handler_that_needs_a_body() {
    let pipeline = support::pipeline_named(
        "r22_m01_width",
        "a file this suite does not measure\n",
        "seed.txt",
    );
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);
    let note = pipeline.project.join("note.txt");
    std::fs::write(&note, BEFORE).expect("write the note");
    let uri = format!("file://{}", note.display());
    let ruler = pipeline.another_key();
    // The row the HTTP verbs will name: an **MCP** row, raised by a process that has exited. That
    // is what puts each verb on the rebuild road, whose re-plan snapshots, which is where a surface
    // with no server answers 502 instead of doing the work.
    let tid = escalate_one_mcp_row(&pipeline, &uri);

    let found = handlers_that_need_a_body(&handlers_source());
    assert!(found.len() >= 5, "the scan's guard: {found:?}");

    let server = Serving::start(&pipeline.project, &pipeline.home, &pipeline.key_id);
    assert_eq!(
        server.start_line["mcp"]["server"],
        Value::Null,
        "the Given: this process named no MCP server"
    );
    let note_line = server.start_line["mcp"]["note"]
        .as_str()
        .expect("the start-up line carries the MCP zero (`req/279` H-02 (b))")
        .to_string();
    println!("R22_M01_NOTE={note_line}");

    // ---- the sentence is held to the router's set, not to a list this file chose --------------
    let mut unnamed: Vec<String> = Vec::new();
    for handler in &found {
        let word = HANDLER_WORDS
            .iter()
            .find(|(fn_name, _)| fn_name == handler)
            .map(|(_, word)| *word)
            .unwrap_or_else(|| {
                panic!("{handler} has no word here — see the arm above, which says what to do")
            });
        if !note_line.contains(word) {
            unnamed.push(format!("{handler} ({word})"));
        }
    }
    assert!(
        unnamed.is_empty(),
        "🔴 `req/303` M-01: `crates/gx-api/src/handlers.rs` routes {} handlers through \
         `with_a_body`, whose rebuild re-plans and whose re-plan snapshots — so a `gx serve` that \
         named no MCP server refuses every one of them. The start-up line reads {note_line:?} and \
         does not name {unnamed:?}. R20 repaired this same sentence by putting four verbs it had \
         chosen against the surface and asserting all four were refused; the fifth was never \
         asked. A declaration narrower than the behaviour is a sentence a reader cannot use to \
         predict the surface",
        found.len()
    );

    // ---- and the surface really does refuse each named verb ----------------------------------
    let mut answered: Vec<(String, u16)> = Vec::new();
    for (handler, path, body) in [
        (
            "verify_candidate",
            format!("/v1/candidates/{tid}/verify"),
            Some(json!({})),
        ),
        (
            "commit_candidate",
            format!("/v1/candidates/{tid}/commit"),
            Some(json!({})),
        ),
        (
            "escalate_candidate",
            format!("/v1/candidates/{tid}/escalation"),
            Some(
                json!({ "decision": "reject", "actor": { "Human": { "key": ruler } },
                         "reason": "a person read the change and refused it" }),
            ),
        ),
        (
            "cancel_candidate",
            format!("/v1/candidates/{tid}/cancel"),
            Some(json!({ "actor": { "Human": { "key": ruler } } })),
        ),
        (
            "undo_transformation",
            format!("/v1/transformations/{tid}/undo"),
            Some(json!({})),
        ),
    ] {
        let (status, answer) = server.json("POST", &path, body.as_ref());
        println!(
            "R22_M01 handler={handler} status={status} detail={}",
            answer["detail"].as_str().unwrap_or_default()
        );
        answered.push((handler.to_string(), status));
    }
    println!("R22_M01_ANSWERED={answered:?}");
    // Recorded rather than asserted to a shape: what this arm is about is the **sentence**, and the
    // status column is here so a reader of the log can see which road each handler took. A handler
    // that stopped being refused would show up as a status that is not 502, and `req/310` says so.
    let refused: Vec<&String> = answered
        .iter()
        .filter(|(_, status)| *status == 502)
        .map(|(handler, _)| handler)
        .collect();
    println!("R22_M01_REFUSED={refused:?}");
    assert_eq!(
        refused.len(),
        found.len(),
        "🔴 the set the sentence is checked against is the set the surface refuses, and the two          are measured separately so that they can disagree. Router set: {found:?}. Refused with          502: {refused:?}. If a handler stopped being refused, the sentence above is now **wider**          than the behaviour — which `req/291` M-02 rules is the same defect as narrower — and          `req/310` has to say so: {answered:?}"
    );
}
