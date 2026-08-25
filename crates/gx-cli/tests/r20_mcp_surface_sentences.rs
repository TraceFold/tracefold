// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/291` M-01 / M-02** (`req/298` §1 items 4 and 5) — the two sentences a surface without
//! an MCP server prints, and whether either of them is true.
//!
//! # What the twentieth adversarial audit measured
//!
//! * **M-02** — R19's new `gx serve` start-up line declares `"no server named: an MCP **ruling or
//!   undo** on this surface is refused"`. The audit put four verbs against such a server and got
//!   **502** from all four: the ruling, the undo, `cancel` and `verify`. The last two are neither a
//!   ruling nor an undo. All four take `handlers::with_a_body`, whose rebuild re-plans, and a
//!   re-plan snapshots. A declaration narrower than the behaviour is a sentence a reader cannot use
//!   to predict the surface.
//! * **M-01** — `gx cancel` had no road to an MCP row on **either** surface, and the two refusals
//!   pointed at each other. HTTP answered 502 naming `--mcp-server <CMD>` as the remedy ("wires one
//!   for a single-shot verb"); the CLI verb refuses that very flag as a usage error (R19 (c)), and
//!   without it printed the same sentence. `Aborted` (the owner stopped it) and `Denied` (a person
//!   said no) are different facts on an audit trail, and only the second was reachable.
//!
//! # Which repair this file holds, and the one it does not
//!
//! `req/298` §1 item 5 ranked "give `cancel` a road that needs no snapshot" first (②) and "split
//! the remedy by surface" as the fallback (③). ② is **not reachable from the crates this lane may
//! write**: `Engine::cancel` refuses any row that is not in the live table, the in-scope ways to
//! seat a row raised by another process are `handlers::rebuilt` and `Session::resume`, and both
//! re-plan — which snapshots. `crates/gx-engine` is outside `req/298` §0's write scope. So this
//! file holds ③, and it holds it to R19 M-07's standard: **the remedy the refusal prints is
//! executed, in another process, and the row reaches `Aborted`.**
//!
//! `cfg(unix)` for the reason every `serve_runtime_r*.rs` gives: `SIGTERM` and `flock`.

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
const ENDPOINT: &str = "stdio://r20";

// ---------------------------------------------------------------------------
// A `gx wrap` session, from the agent's side
// ---------------------------------------------------------------------------

struct Wrap {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Wrap {
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
                "clientInfo": { "name": "r20-surface", "version": "0" },
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
        let _ = self.child.wait_with_output().expect("gx wrap exits");
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
    start_line: Value,
}

impl Serving {
    fn start(project: &Path, home: &Path, key_id: &str, mcp: &[String]) -> Self {
        let token = "r20-surface-token".to_string();
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
        let addr = start["bind"].as_str().expect("bound address").to_string();
        Self {
            child,
            addr,
            token,
            stdout,
            start_line: start,
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
    uri: String,
    arrivals: PathBuf,
    ruler: String,
}

fn fixture(name: &str) -> Fixture {
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
        uri,
        arrivals,
        ruler,
    }
}

impl Fixture {
    fn mcp_flags(&self) -> Vec<String> {
        vec![
            "--mcp-server".into(),
            env!("CARGO_BIN_EXE_gx").to_string(),
            "--mcp-server-arg".into(),
            DEMO_SERVER_ARG.into(),
            "--mcp-server-env".into(),
            format!("{ARRIVALS_ENV}={}", self.arrivals.display()),
            "--mcp-endpoint".into(),
            ENDPOINT.into(),
        ]
    }

    /// One `notes.write` through a `gx wrap` that declares no restore for it: E-M3-4 escalates it,
    /// and the process then exits — so the row is one no later process holds in its live table.
    fn escalate_one(&self) -> String {
        let args: Vec<String> = vec![
            "--project".into(),
            self.pipeline.project.display().to_string(),
            "wrap".into(),
            "--endpoint".into(),
            ENDPOINT.into(),
            "--actor-key".into(),
            self.pipeline.key_id.clone(),
            "--actor-model".into(),
            "r20-probe".into(),
            "--server-env".into(),
            format!("{ARRIVALS_ENV}={}", self.arrivals.display()),
            "--".into(),
            env!("CARGO_BIN_EXE_gx").to_string(),
            DEMO_SERVER_ARG.into(),
        ];
        let mut wrap = Wrap::spawn(&args, &self.pipeline.home);
        let answered = wrap.request(
            "tools/call",
            json!({ "name": "notes.write", "arguments": { "uri": self.uri, "contents": AFTER } }),
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

/// The `detail` of a problem+json body, as a string.
fn detail(body: &Value) -> String {
    body["detail"].as_str().unwrap_or_default().to_string()
}

// ---------------------------------------------------------------------------
// M-02 — the declared width is the measured width
// ---------------------------------------------------------------------------

/// 🔴 **M-02** — the start-up line names every verb the surface actually refuses.
#[test]
fn the_start_up_line_names_every_verb_a_serverless_surface_refuses() {
    let fixture = fixture("r20_m02_width");
    let tid = fixture.escalate_one();
    let server = Serving::start(
        &fixture.pipeline.project,
        &fixture.pipeline.home,
        &fixture.pipeline.key_id,
        &[],
    );
    let note = server.start_line["mcp"]["note"]
        .as_str()
        .expect("the start-up line carries the MCP zero (`req/279` H-02 (b))")
        .to_string();
    println!("R20_M02_NOTE={note}");
    assert_eq!(
        server.start_line["mcp"]["server"],
        Value::Null,
        "the Given: this process named no server"
    );

    // Measured first, then compared with the declaration — never the other way round.
    let mut refused: Vec<&str> = Vec::new();
    for (verb, method, path, body) in [
        (
            "ruling",
            "POST",
            format!("/v1/candidates/{tid}/escalation"),
            Some(
                json!({ "decision": "reject", "actor": { "Human": { "key": fixture.ruler } },
                         "reason": "a person read the change and refused it" }),
            ),
        ),
        (
            "undo",
            "POST",
            format!("/v1/transformations/{tid}/undo"),
            Some(json!({})),
        ),
        (
            "cancel",
            "POST",
            format!("/v1/candidates/{tid}/cancel"),
            Some(json!({ "actor": { "Human": { "key": fixture.ruler } } })),
        ),
        (
            "verify",
            "POST",
            format!("/v1/candidates/{tid}/verify"),
            Some(json!({})),
        ),
    ] {
        let (status, answer) = server.json(method, &path, body.as_ref());
        println!(
            "R20_M02 verb={verb} status={status} detail={}",
            detail(&answer)
        );
        if status == 502 {
            refused.push(verb);
        }
    }
    println!("R20_M02_REFUSED={refused:?}");
    assert_eq!(
        refused,
        vec!["ruling", "undo", "cancel", "verify"],
        "the Given of this arm is that all four are refused; if one of them stopped being refused, \
         the sentence below is the wrong repair and `req/299` must say so"
    );
    for verb in &refused {
        assert!(
            note.contains(*verb),
            "🔴 `req/291` M-02: the start-up line declares {note:?} and this surface also refuses \
             {verb:?}. A declaration narrower than the behaviour is a sentence a reader cannot use \
             to predict the surface"
        );
    }
}

// ---------------------------------------------------------------------------
// M-01 — the two refusals stop pointing at each other
// ---------------------------------------------------------------------------

/// 🔴 **M-01, the HTTP half** — a 502 from a serverless `gx serve` names a remedy for **this**
/// surface, not for a single-shot verb somewhere else.
#[test]
fn the_http_refusal_names_a_remedy_this_surface_can_take() {
    let fixture = fixture("r20_m01_http_sentence");
    let tid = fixture.escalate_one();
    let server = Serving::start(
        &fixture.pipeline.project,
        &fixture.pipeline.home,
        &fixture.pipeline.key_id,
        &[],
    );
    let (status, answer) = server.json(
        "POST",
        &format!("/v1/candidates/{tid}/cancel"),
        Some(&json!({ "actor": { "Human": { "key": fixture.ruler } } })),
    );
    let detail = detail(&answer);
    println!("R20_M01_HTTP status={status} detail={detail}");
    assert_eq!(
        status, 502,
        "the Given: no server, so the rebuild's snapshot refuses"
    );
    assert!(
        detail.contains("connected to no MCP server"),
        "the fact is unchanged and stays checkable: {detail}"
    );
    assert!(
        detail.contains("gx serve --mcp-server"),
        "🔴 `req/291` M-01: the remedy must be one this surface can take. A server's server is \
         chosen at start-up: {detail}"
    );
    assert!(
        !detail.contains("wires one for a single-shot verb"),
        "🔴 `req/291` M-01: this is not a single-shot verb, and `gx cancel` — the single-shot verb \
         the reader would reach for — refuses that flag outright: {detail}"
    );
}

/// 🔴 **M-01, the CLI half** — `gx cancel` no longer prints, as its remedy, the flag it refuses.
#[test]
fn the_cli_refusal_does_not_name_the_flag_the_same_verb_refuses() {
    let fixture = fixture("r20_m01_cli_sentence");
    let tid = fixture.escalate_one();

    let without = support::run(fixture.pipeline.gx().arg("cancel").arg(&tid));
    println!(
        "R20_M01_CLI rc={} stderr={}",
        without.code,
        without.stderr.chars().take(400).collect::<String>()
    );
    assert_eq!(
        without.code, 1,
        "the Given: `gx cancel` opens no road to an MCP server, so the row's snapshot refuses"
    );
    assert!(
        without.stderr.contains("connected to no MCP server"),
        "the fact is unchanged: {}",
        without.stderr
    );
    assert!(
        !without
            .stderr
            .contains("--mcp-server <CMD>` wires one for a single-shot verb"),
        "🔴 `req/291` M-01: this refusal used to name `--mcp-server` as its remedy while the same \
         verb refuses that flag as a usage error — two refusals pointing at each other, and a \
         reader can execute neither: {}",
        without.stderr
    );
    assert!(
        without.stderr.contains("gx serve --mcp-server"),
        "and the remedy it prints instead must be the surface that does hold a server: {}",
        without.stderr
    );

    // The negative control: R19 (c) is not weakened. The flag is still a usage error here.
    let with = support::run(
        fixture
            .pipeline
            .gx()
            .args(["--mcp-server", env!("CARGO_BIN_EXE_gx")])
            .arg("cancel")
            .arg(&tid),
    );
    println!(
        "R20_M01_CLI_FLAG rc={} stderr={}",
        with.code,
        with.stderr.chars().take(240).collect::<String>()
    );
    assert_eq!(
        with.code, 1,
        "R19 (c): a flag with nowhere to go is refused, not dropped"
    );
    assert!(
        with.stderr.contains("nowhere to go on this command"),
        "R19 (c)'s wording is untouched by this lane: {}",
        with.stderr
    );
}

/// 🔴 **M-01, the road that exists** — the remedy the refusals now print is **executed**, and the
/// row reaches `Aborted{OwnerCancelled}`.
///
/// Without this arm the two above are satisfiable by a lane that made the sentences prettier while
/// leaving `Aborted` unreachable for an MCP row. R19's M-07 set this standard: a remedy is worth
/// printing only if running it works.
#[test]
fn the_remedy_runs_and_the_owner_can_abort_an_mcp_row() {
    let fixture = fixture("r20_m01_remedy_runs");
    let tid = fixture.escalate_one();
    let server = Serving::start(
        &fixture.pipeline.project,
        &fixture.pipeline.home,
        &fixture.pipeline.key_id,
        &fixture.mcp_flags(),
    );
    println!("R20_M01_REMEDY start_mcp={}", server.start_line["mcp"]);
    let (status, answer) = server.json(
        "POST",
        &format!("/v1/candidates/{tid}/cancel"),
        Some(&json!({ "actor": { "Human": { "key": fixture.ruler } } })),
    );
    println!("R20_M01_REMEDY status={status} body={answer}");
    assert_eq!(
        status, 200,
        "🔴 `req/291` M-01: `Aborted` (the owner stopped it) and `Denied` (a person said no) are \
         different facts on an audit trail, and until this lane only the second was reachable for \
         an MCP row: {answer}"
    );
    assert_eq!(
        answer["state"], "Aborted",
        "43 T-7's terminal state: {answer}"
    );
    assert_eq!(
        answer["reason"], "OwnerCancelled",
        "and its reason, which is the whole of what distinguishes it from a denial: {answer}"
    );
}
