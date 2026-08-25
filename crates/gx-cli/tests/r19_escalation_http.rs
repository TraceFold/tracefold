// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R19, the HTTP half** (`req/284` §1.1 (b) and §1.2, from audit 19's H-02 and M-05).
//!
//! Two facts about the surface a **person** actually looks at — the one a GUI drives:
//!
//! * **H-02 (b)** — `gx serve` accepted `--mcp-server`/`--mcp-server-arg`/`--mcp-restore-catalogue`
//!   (they are `clap` globals) and opened its engine with `McpWiring::default()` anyway, so every
//!   MCP transformation reaching `POST /v1/candidates/{id}/escalation` or
//!   `POST /v1/transformations/{id}/undo` came back **502 ADAPTER_ERROR**, with a detail naming
//!   `--mcp-server` as the remedy — the flag the process had just discarded.
//! * **M-05** — `GET /v1/escalations` read the **live** ticket table, and the ticket is not written
//!   to the journal. An escalation raised by a `gx wrap` process therefore vanished the moment that
//!   process exited: the server's `/v1/candidates` and `/v1/transformations` both showed the row as
//!   `"state": "Escalated"` while `/v1/escalations` answered `{"items":[]}`. `req/182` H-04 fixed
//!   the badge that never went **down**; this is the same list broken the other way.
//!
//! `cfg(unix)` for the reason every `serve_runtime_r*.rs` gives: `SIGTERM` and `flock`. Windows,
//! WSL 9p and a synchronising client are **not measured**.

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

const READ_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_WAIT: Duration = Duration::from_secs(30);

const BEFORE: &str = "the note as it stood before any agent touched it\n";
const AFTER: &str = "the note after an agent wrote through gx wrap\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ARRIVALS_ENV: &str = "GX_DEMO_LOG";

// ---------------------------------------------------------------------------
// A `gx wrap` session, from the agent's side (the ninth copy of this shape; a
// test binary is its own crate)
// ---------------------------------------------------------------------------

struct Wrap {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Wrap {
    /// `home` is `~/.gx/keys/` (req/56 §3): without it the gate answers `no key for …`, which is a
    /// fixture fault wearing the shape of a finding.
    fn spawn(args: &[String], home: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", home)
            .env("USERPROFILE", home)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the gx binary runs");
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let mut session = Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 0,
        };
        session.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "r19-http", "version": "0" },
            }),
        );
        let frame = jsonrpc::notification("notifications/initialized", json!({}));
        jsonrpc::write_frame(session.stdin.as_mut().expect("open"), &frame).expect("write");
        session
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let frame = jsonrpc::request(self.next_id, method, params);
        jsonrpc::write_frame(self.stdin.as_mut().expect("open"), &frame).expect("write");
        match jsonrpc::read_frame(&mut self.stdout).expect("read") {
            Some(line) => serde_json::from_str(&line).expect("gx wrap answers JSON"),
            None => {
                let mut text = String::new();
                if let Some(mut err) = self.child.stderr.take() {
                    let _ = err.read_to_string(&mut text);
                }
                panic!("gx wrap closed its stdout without answering {method:?}. stderr:\n{text}")
            }
        }
    }

    fn finish(mut self) {
        self.stdin = None;
        let out = self.child.wait_with_output().expect("gx wrap exits");
        println!(
            "R19_WRAP_EXIT={:?} stderr_tail={}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next_back()
                .unwrap_or_default()
        );
    }
}

// ---------------------------------------------------------------------------
// A running `gx serve`
// ---------------------------------------------------------------------------

struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<ChildStdout>,
}

impl Serving {
    /// Start a server, optionally wiring it to the MCP server this project's changes were made
    /// through — the `--mcp-*` globals `gx serve` used to accept and drop.
    fn start(project: &Path, home: &Path, key_id: &str, mcp: &[String]) -> Self {
        let token = "r19-http-token".to_string();
        let token_file = project.join("token");
        std::fs::write(&token_file, format!("{token}\n")).expect("write the token file");

        let mut command = Command::new(env!("CARGO_BIN_EXE_gx"));
        command
            .env("HOME", home)
            .env("USERPROFILE", home)
            .arg("--project")
            .arg(project)
            .args(mcp)
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
        println!("R19_SERVE_START={start}");
        let addr = start["bind"].as_str().expect("bound address").to_string();
        Self {
            child,
            addr,
            token,
            stdout,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, String) {
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
        (status, body)
    }

    fn json(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
        let (status, text) = self.request(method, path, body);
        let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        (status, value)
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

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

struct Fixture {
    pipeline: support::Pipeline,
    note: PathBuf,
    uri: String,
    arrivals: PathBuf,
    ruler: String,
}

fn fixture(name: &str) -> Fixture {
    // 🔴 `gx submit` is the one verb that **creates** `.gx/`, and both `gx wrap` and `gx serve`
    // open rather than create. One throwaway fs intent makes the project exist without touching
    // the note this suite measures.
    let pipeline =
        support::pipeline_named(name, "a file this suite does not measure\n", "seed.txt");
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);
    let note = pipeline.project.join("note.txt");
    std::fs::write(&note, BEFORE).expect("write the note");
    let uri = format!("file://{}", note.display().to_string().replace('\\', "/"));
    let arrivals = pipeline.project.join("arrivals.tsv");
    let ruler = pipeline.another_key();
    Fixture {
        pipeline,
        note,
        uri,
        arrivals,
        ruler,
    }
}

impl Fixture {
    fn note_now(&self) -> String {
        std::fs::read_to_string(&self.note).unwrap_or_default()
    }

    fn mcp_flags(&self) -> Vec<String> {
        vec![
            "--mcp-server".into(),
            env!("CARGO_BIN_EXE_gx").to_string(),
            "--mcp-server-arg".into(),
            DEMO_SERVER_ARG.into(),
            "--mcp-server-env".into(),
            format!("{ARRIVALS_ENV}={}", self.arrivals.display()),
            "--mcp-endpoint".into(),
            "stdio://r19".into(),
        ]
    }

    /// One `notes.write` through a `gx wrap` that declares **no** restore for it, so E-M3-4
    /// escalates it and the `gx wrap` process then exits — which is the whole point of M-05.
    fn escalate_one(&self) -> String {
        let args: Vec<String> = vec![
            "--project".into(),
            self.pipeline.project.display().to_string(),
            "wrap".into(),
            "--endpoint".into(),
            "stdio://r19".into(),
            "--actor-key".into(),
            self.pipeline.key_id.clone(),
            "--actor-model".into(),
            "r19-probe".into(),
            "--server-env".into(),
            format!("{ARRIVALS_ENV}={}", self.arrivals.display()),
            "--".into(),
            env!("CARGO_BIN_EXE_gx").to_string(),
            DEMO_SERVER_ARG.into(),
        ];
        let mut wrap = Wrap::spawn(&args, &self.pipeline.home);
        let answered = wrap.request(
            "tools/call",
            json!({
                "name": "notes.write",
                "arguments": { "uri": self.uri, "contents": AFTER },
            }),
        );
        wrap.finish();
        let meta = &answered["result"]["_meta"];
        assert_eq!(
            meta["gx/verdict"], "Escalate",
            "the Given is an escalation raised by a process that has now exited: {answered}"
        );
        meta["gx/transformation"]
            .as_str()
            .expect("a transformation id")
            .to_string()
    }
}

// ---------------------------------------------------------------------------
// M-05 — the queue a person looks at
// ---------------------------------------------------------------------------

/// 🔴 **M-05** — an escalation raised by another process is in `GET /v1/escalations`.
///
/// The audit's `A19_Q` arm, asserted the other way round: `/v1/candidates` and
/// `/v1/transformations` said `"state": "Escalated"` and `/v1/escalations` said `{"items":[]}` — so
/// the one surface 43 T-4c's "notify a human" actually is showed nothing to notify anyone about.
#[test]
fn an_escalation_raised_by_another_process_is_in_the_list_a_person_reads() {
    let fixture = fixture("r19_m05_queue");
    let tid = fixture.escalate_one();

    let server = Serving::start(
        &fixture.pipeline.project,
        &fixture.pipeline.home,
        &fixture.pipeline.key_id,
        &[],
    );

    let (candidates_status, candidates) = server.json("GET", "/v1/candidates", None);
    assert_eq!(candidates_status, 200, "candidates: {candidates}");
    assert!(
        candidates["items"]
            .as_array()
            .expect("items")
            .iter()
            .any(|row| row["transformation"] == tid.as_str() && row["state"] == "Escalated"),
        "the Given: the row is Escalated on the surfaces that read Σ. {candidates}"
    );

    let (status, escalations) = server.json("GET", "/v1/escalations", None);
    assert_eq!(status, 200, "escalations: {escalations}");
    let items = escalations["items"].as_array().expect("items");
    println!("R19_M05 escalations={escalations}");
    assert_eq!(
        items.len(),
        1,
        "🔴 `req/279` M-05: `list::escalations` asked `Engine::ticket` — the **live** table — and \
         the ticket is not journalled, so an escalation raised by a `gx wrap` that has since \
         exited was invisible here while `/v1/candidates` showed it as `Escalated`. \
         `Engine::ticket_as_raised` rebuilds it from Σ and `/stream` has used it since v0.4-l. \
         got: {escalations}"
    );
    assert_eq!(items[0]["transformation"], tid.as_str());
    assert!(
        items[0]["ticket_id"].is_string(),
        "the ticket T-4c raised, rebuilt from Σ: {}",
        items[0]
    );
    assert!(
        serde_json::to_string(&items[0]["reasons"])
            .unwrap_or_default()
            .contains("INVERSE_UNAVAILABLE"),
        "E-M3-4's own reason travels with it: {}",
        items[0]
    );
}

// ---------------------------------------------------------------------------
// H-02 (b) — the HTTP surface reaches the server its invocation named
// ---------------------------------------------------------------------------

/// 🔴 **H-02 (b)** — `gx serve --mcp-server …` opens its engine wired to that server.
///
/// Before this repair `serve.rs` called `session::open_engine`, whose whole body is
/// `open_engine_wired(…, &McpWiring::default())`. `open_engine_wired` already existed and already
/// took the value; the server was the one road that never handed it one. So the GUI's ruling on an
/// MCP change was **502 ADAPTER_ERROR** — `the adapter refused to snapshot: … this "gx" is
/// connected to no MCP server` — and the object could never move.
#[test]
fn a_person_can_rule_on_an_mcp_change_over_http_and_the_effect_completes() {
    let fixture = fixture("r19_h02_http");
    let tid = fixture.escalate_one();

    let server = Serving::start(
        &fixture.pipeline.project,
        &fixture.pipeline.home,
        &fixture.pipeline.key_id,
        &fixture.mcp_flags(),
    );

    let ruling = json!({
        "decision": "approve",
        "reason": "a person read the change in the GUI and allowed it",
        "actor": { "Human": { "key": fixture.ruler } },
    });
    let (status, ruled) = server.json(
        "POST",
        &format!("/v1/candidates/{tid}/escalation"),
        Some(&ruling),
    );
    println!("R19_H02B ruling_status={status} body={ruled}");
    assert_eq!(
        status, 200,
        "🔴 `req/279` H-02 (c): the ruling came back 502 ADAPTER_ERROR whose detail named \
         `--mcp-server` — a flag `gx serve` accepted and dropped. body: {ruled}"
    );
    assert_eq!(ruled["state"], "Admitted", "43 T-5: {ruled}");

    let (commit_status, committed) =
        server.json("POST", &format!("/v1/candidates/{tid}/commit"), None);
    assert_eq!(
        commit_status, 200,
        "the admitted change commits through the same wiring: {committed}"
    );
    println!(
        "R19_H02B commit={committed} note={:?} arrivals={:?}",
        fixture.note_now(),
        std::fs::read_to_string(&fixture.arrivals).unwrap_or_default()
    );
    assert_eq!(
        fixture.note_now(),
        AFTER,
        "🔴 `req/284` §2: the object is the assertion. A person ruled over HTTP and the effect \
         reached the server this process named"
    );

    let (undo_status, undone) =
        server.json("POST", &format!("/v1/transformations/{tid}/undo"), None);
    println!("R19_H02B undo_status={undo_status} body={undone}");
    assert_ne!(
        undo_status, 502,
        "🔴 `req/279` H-02 (d): `POST /v1/transformations/{{id}}/undo` on an MCP transformation \
         answered 502 ADAPTER_ERROR with the same sentence. Whatever this row's state machine \
         allows, the answer is no longer \"this gx is connected to no MCP server\". body: {undone}"
    );
    assert!(
        !serde_json::to_string(&undone)
            .unwrap_or_default()
            .contains("connected to no MCP server"),
        "the transport is wired: {undone}"
    );
}

/// 🔴 The negative control for (b): a `gx serve` that named no server still refuses, in the same
/// sentence it always did (`req/284` §1.1 (c) — fail-closed is unchanged).
#[test]
fn a_server_that_named_no_mcp_server_still_refuses_the_ruling() {
    let fixture = fixture("r19_h02_http_negative");
    let tid = fixture.escalate_one();

    let server = Serving::start(
        &fixture.pipeline.project,
        &fixture.pipeline.home,
        &fixture.pipeline.key_id,
        &[],
    );
    let ruling = json!({
        "decision": "approve",
        "reason": "a person ruled from a server wired to nothing",
        "actor": { "Human": { "key": fixture.ruler } },
    });
    let (status, ruled) = server.json(
        "POST",
        &format!("/v1/candidates/{tid}/escalation"),
        Some(&ruling),
    );
    println!("R19_H02B_NEG status={status} body={ruled}");
    assert_ne!(status, 200, "fail-closed is unchanged: {ruled}");
    assert!(
        serde_json::to_string(&ruled)
            .unwrap_or_default()
            .contains("connected to no MCP server"),
        "and the refusal is still the true sentence: {ruled}"
    );
    assert_eq!(fixture.note_now(), BEFORE, "nothing moved");
}
