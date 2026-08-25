// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **AC-056** over a real socket, and `gx serve`'s three shutdown stages under a real `SIGTERM`.
//!
//! `crates/gx-api/tests/stream.rs` measures the same acceptance criterion through the router, which
//! answers "is the projection right" (sem: SEM-gx-cli-1920). This file answers the other half — "does the *server* work" (sem: SEM-gx-cli-1920) —
//! and there is no way to ask it without a process, a port and a signal:
//!
//! * the **runtime** (`tokio` inside gx-api, no `#[tokio::main]` in this binary);
//! * the **bind**, including the fact that `--bind 127.0.0.1:0` reports the port it actually got;
//! * **chunked transfer encoding**, which is what hyper does with a body that has no length — the
//!   thing an NDJSON audit stream *is*, and something a `tower::ServiceExt::oneshot` never exercises;
//! * **graceful shutdown**, stage by stage, on the signal an init system sends.
//!
//! # 🔴 Timing, and the retry rule (req/88 §8.2, 51 §13-4)
//!
//! Same rule as the in-process suite, restated because a reader of this file should not have to find
//! the other one: **zero retries** (sem: SEM-gx-cli-1921). Every wait is a socket read timeout of [`READ_TIMEOUT`] or a bounded
//! poll of [`STARTUP_WAIT`] for the process to print its first line, and an expiry **fails**. Nothing
//! here re-runs a step, and no probe is `#[ignore]`d. If one of these expires it is a real defect —
//! "if it still fails, that's a real bug, not flaky" (sem: SEM-gx-cli-1922).
//!
//! # Why the HTTP client is thirty lines of `std::net`
//!
//! A client crate would be a shipping-adjacent dependency added for a test, and this file needs
//! exactly three things: send a request, read a status line, read chunks. M6-27's discipline is about
//! shipping graphs, but a dev-dependency is still a package an auditor sees in `Cargo.lock`, and the
//! hand that just measured the closure growing by 41 is not the hand to add a forty-second.

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use support::pipeline;

/// How long a socket read may block before the probe fails. See the module header: 0 retries.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server has to print its start-up line.
const STARTUP_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve`, its address, and its standard output.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Serving {
    /// Start `gx serve` over a project, on a port the operating system chooses.
    ///
    /// 🔴 `--bind 127.0.0.1:0` is loopback, so M6-10's policy admits it, and the **kernel** picks the
    /// port. A test that picked its own would either collide with whatever else is on this machine or
    /// need a bind-then-release dance that is itself a race; 44 §1.2 asks for a structured start-up
    /// line, and the port in that line is the answer to "which one did you get" (sem: SEM-gx-cli-1923).
    fn start(project: &std::path::Path, home: &std::path::Path, key_id: &str) -> Self {
        let token = "m6h6-socket-token".to_string();
        let token_file = project.join("token");
        std::fs::write(&token_file, format!("{token}\n")).expect("write the token file");

        let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
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
            .stderr(Stdio::piped())
            .spawn()
            .expect("gx serve starts");

        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut line = String::new();
        let deadline = Instant::now() + STARTUP_WAIT;
        while line.trim().is_empty() && Instant::now() < deadline {
            line.clear();
            if stdout.read_line(&mut line).unwrap_or(0) == 0 {
                // stdout closed: the process stopped. Its refusal is on stderr (44 §1.3) and is
                // worth more than a timeout — a fixture that reported "no start-up line" for "the (sem: SEM-gx-cli-1924)
                // server said why it would not start" would be M4H4-2's mistake in a test harness.
                let mut why = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut why);
                }
                panic!("gx serve stopped before it served: {why}");
            }
        }
        let start: serde_json::Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| {
            panic!("44 §1.2: \"stdout: startup log (structured JSON line)\" (sem: SEM-gx-cli-1925); got {line:?} ({e})")
        });
        println!("SERVE_START={start}");
        assert_eq!(start["event"], serde_json::json!("gx.serve.started"));
        let addr = start["bind"]
            .as_str()
            .expect("the bound address")
            .to_string();
        assert!(
            addr.starts_with("127.0.0.1:"),
            "M6-10 adopted (b; sem: SEM-gx-cli-1926): the default and the only permitted family is loopback; got {addr}"
        );
        // 🔴 M6H5-13's other half, in the log rather than in `--help`: the absence follows the
        // server into the place a second operator will look six months from now.
        let notice = start["authorization"].as_str().unwrap_or_default();
        assert!(
            notice.contains("no authorization layer") && notice.contains("Bearer"),
            "the start-up line carries `ABSENCE_NOTICE`: {start}"
        );
        Self {
            child,
            addr,
            token,
            stdout,
        }
    }

    /// One HTTP/1.1 request on its own connection, read to the end.
    fn request(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> (u16, String) {
        let mut socket = self.connect();
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
        let body = text.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        (status, body.to_string())
    }

    /// Open `GET /v1/stream` and hand back a reader positioned after the response headers.
    fn subscribe(&self, query: &str) -> BufReader<TcpStream> {
        let mut socket = self.connect();
        socket
            .write_all(
                format!(
                    "GET /v1/stream{query} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
                    self.addr, self.token
                )
                .as_bytes(),
            )
            .expect("write the subscription");
        socket.flush().expect("flush");
        let mut reader = BufReader::new(socket);
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read a header");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            headers.push_str(&line);
        }
        println!("SUBSCRIBE_HEADERS={}", headers.replace("\r\n", " | "));
        assert!(headers.starts_with("HTTP/1.1 200"), "{headers}");
        assert!(
            headers.to_lowercase().contains("application/x-ndjson"),
            "44 §2.2: \"`Content-Type: application/x-ndjson`\" (sem: SEM-gx-cli-1927) — {headers}"
        );
        assert!(
            headers.to_lowercase().contains("transfer-encoding: chunked"),
            "🔴 a stream with no length is chunked, which is the framing the in-process suite cannot \
             exercise: {headers}"
        );
        reader
    }

    fn connect(&self) -> TcpStream {
        let socket = TcpStream::connect(&self.addr).expect("connect to the server");
        socket
            .set_read_timeout(Some(READ_TIMEOUT))
            .expect("a read timeout, so an expiry is a failure and not a hang");
        socket
    }
}

/// Read one chunk of a chunked body, as one NDJSON line.
///
/// Each `send` on the reader task writes one line, so each chunk holds one object. That is an
/// observation about this implementation rather than a rule of the protocol, and it is asserted:
/// a chunk carrying two lines would mean a client cannot process events as they arrive.
fn next_line(reader: &mut BufReader<TcpStream>) -> serde_json::Value {
    let mut size_line = String::new();
    reader.read_line(&mut size_line).expect("a chunk size");
    let size = usize::from_str_radix(size_line.trim(), 16).unwrap_or_else(|e| {
        panic!("chunked framing: {size_line:?} is not a hex length ({e})");
    });
    assert!(size > 0, "the stream ended where a line was expected");
    let mut data = vec![0u8; size];
    reader.read_exact(&mut data).expect("the chunk body");
    let mut crlf = [0u8; 2];
    reader.read_exact(&mut crlf).expect("the chunk terminator");
    let text = String::from_utf8(data).expect("utf-8");
    assert_eq!(
        text.matches('\n').count(),
        1,
        "one chunk, one line — a client reading NDJSON needs each object to arrive whole: {text:?}"
    );
    serde_json::from_str(text.trim()).expect("one JSON object per line")
}

/// 🔴 **AC-056**, end to end: subscribe over a socket, commit from a second connection, read the line.
///
/// 34's text: "while subscribed, another client commits → one line of `event: "committed"` is delivered" (sem: SEM-gx-cli-1928). Every noun is real
/// here — two connections, one server process, one signal at the end.
#[test]
fn ac_056_over_a_socket_a_subscriber_sees_another_clients_commit() {
    let fixture = pipeline("m6h6_ac056_socket", "before\n");
    // One `gx submit` first, because `gx serve` **opens** a project rather than creating one:
    // a server that made a `.gx/` would silently start a second, empty ledger beside an
    // operator's real one if they mistyped `--project` (the reading hand 2 fixed for every
    // other verb).
    assert_eq!(
        fixture
            .submit(
                "warm
"
            )
            .code,
        0
    );
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    // Subscribe from the end.
    let mut stream = server.subscribe("?since=2099-01-01T00:00:00Z");

    // A second connection drives the pipeline.
    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": fixture.target.display().to_string(),
        "goal": "after\n",
        "context": "Evidence",
        "actor": { "Human": { "key": fixture.key_id } },
    });
    let (created_status, created_body) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("SOCKET_CREATE={created_status} {created_body}");
    assert_eq!(created_status, 201, "{created_body}");
    let created: serde_json::Value = serde_json::from_str(&created_body).expect("json");
    let id = created["id"].as_str().expect("an id").to_string();

    let (verify_status, verify_body) =
        server.request("POST", &format!("/v1/candidates/{id}/verify"), None);
    assert_eq!(verify_status, 200, "{verify_body}");
    let (commit_status, commit_body) =
        server.request("POST", &format!("/v1/candidates/{id}/commit"), None);
    assert_eq!(commit_status, 200, "{commit_body}");

    let mut seen = Vec::new();
    let committed = loop {
        let line = next_line(&mut stream);
        let event = line["event"].as_str().unwrap_or_default().to_string();
        seen.push(event.clone());
        if event == "committed" {
            break line;
        }
        assert!(seen.len() < 20, "no `committed` in twenty lines: {seen:?}");
    };
    println!("AC056_SOCKET_EVENTS={seen:?}\nAC056_SOCKET_COMMITTED={committed}");
    assert_eq!(committed["transformation"], serde_json::json!(id));
    assert!(committed["data"]["ledger_index"].is_number());

    // The substrate really moved: AC-056 is about a **commit**, and a stream reporting one that did
    // not happen would be the worst possible pass.
    assert_eq!(fixture.target_contents(), "after\n");

    let stopped = shut_down(server);
    println!("AC056_SOCKET_SHUTDOWN={stopped}");
    assert_eq!(stopped["exit"], serde_json::json!(0));
}

/// 🔴 Graceful shutdown, all three stages, on a real `SIGTERM`.
///
/// Stage 1 is measured by the **subscription ending**: an open `GET /stream` is an in-flight request
/// that never finishes, so a server that did not tell its readers to stop could not shut down at all
/// and this probe would hit `READ_TIMEOUT`. Stage 2 is the exit status — 0 means the wait completed
/// rather than expiring. Stage 3 is what did **not** happen, and the log line says so
/// (`deadline_exceeded: false`, `in_flight_at_end: 0`).
#[test]
fn graceful_shutdown_ends_the_subscriptions_and_exits_zero() {
    let fixture = pipeline("m6h6_shutdown_socket", "before\n");
    // One `gx submit` first, because `gx serve` **opens** a project rather than creating one:
    // a server that made a `.gx/` would silently start a second, empty ledger beside an
    // operator's real one if they mistyped `--project` (the reading hand 2 fixed for every
    // other verb).
    assert_eq!(
        fixture
            .submit(
                "warm
"
            )
            .code,
        0
    );
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let mut stream = server.subscribe("");

    let stopped = shut_down(server);
    println!("SHUTDOWN_LOG={stopped}");
    assert_eq!(stopped["event"], serde_json::json!("gx.serve.stopped"));
    assert_eq!(
        stopped["reason"],
        serde_json::json!("SIGTERM"),
        "the signal an init system sends, named in the log rather than inferred"
    );
    assert_eq!(stopped["deadline_exceeded"], serde_json::json!(false));
    assert_eq!(stopped["in_flight_at_end"], serde_json::json!(0));
    assert_eq!(
        stopped["exit"],
        serde_json::json!(0),
        "44 §1.2: \"0=normal termination (SIGTERM, etc.)\" (sem: SEM-gx-cli-1929) — and 1 when work was abandoned, which is the case this \
         probe shows did not arise"
    );

    // Stage 1, from the client's side: the body ended.
    let mut size_line = String::new();
    let read = std::io::BufRead::read_line(&mut stream, &mut size_line);
    println!("SHUTDOWN_STREAM_TAIL={size_line:?} read={read:?}");
    assert!(
        matches!(size_line.trim(), "" | "0"),
        "🔴 the subscription ended rather than being abandoned mid-line. Without stage 1 the reader \
         task would still be polling and `axum::serve`'s graceful shutdown would have waited for a \
         body with no last frame — every shutdown would reach the deadline and every exit would be 1"
    );
}

/// 🔴 **P3.1 repair, gotcha65** (`req/38` §74 item②, `req/125` §4-2) — idle, no connections, no
/// signal, and the process must still be there past `gx_api::serve::DEFAULT_GRACE` (10s).
///
/// Before the repair, `serve.rs` wrapped `axum::serve(...).with_graceful_shutdown(...)`'s **whole**
/// lifetime in `tokio::time::timeout(config.grace, server)` — a future that does not resolve until a
/// shutdown signal arrives, so the timeout fired on the clock alone and `gx serve` exited
/// `deadline_exceeded: true` after ten seconds regardless of load or signal (the audit's own
/// reproduction: "if nothing connects and 15 seconds pass … it terminates on its own" (sem: SEM-gx-cli-1930)). This probe is that reproduction run
/// against the fix: fifteen real seconds — five past the ten-second grace, the same margin the audit
/// used — with the process touched by nothing, and only then a real `SIGTERM` to prove the grace
/// window is still there in full for the drain once one arrives (`deadline_exceeded: false`).
#[test]
fn idle_with_no_signal_outlives_the_grace_period() {
    let fixture = pipeline("m6h6_shutdown_idle", "before\n");
    assert_eq!(
        fixture
            .submit(
                "warm
"
            )
            .code,
        0
    );
    let mut server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    std::thread::sleep(std::time::Duration::from_secs(15));
    let status = server
        .child
        .try_wait()
        .expect("try_wait does not error on a live child");
    println!(
        "IDLE_15S_STILL_ALIVE={} STATUS={status:?}",
        status.is_none()
    );
    assert!(
        status.is_none(),
        "🔴 gotcha65: `gx serve` must still be running after 15s idle with zero connections and \
         zero signals — DEFAULT_GRACE (10s) bounds stage 2's drain wait *after* a shutdown signal, \
         not the process's total unsignalled lifetime. A `Some` here means the whole-lifetime \
         `tokio::time::timeout` regressed and this process already exited: {status:?}"
    );

    let stopped = shut_down(server);
    println!("IDLE_15S_SHUTDOWN={stopped}");
    assert_eq!(
        stopped["deadline_exceeded"],
        serde_json::json!(false),
        "a signal that arrives after the idle wait still drains inside the full grace window"
    );
    assert_eq!(stopped["exit"], serde_json::json!(0));
}

/// Send `SIGTERM`, wait for the process, and hand back the line it printed on the way out.
fn shut_down(mut server: Serving) -> serde_json::Value {
    let pid = server.child.id().to_string();
    let killed = Command::new("kill")
        .args(["-TERM", &pid])
        .status()
        .expect("kill(1) is available on this platform");
    assert!(killed.success(), "SIGTERM was not delivered to {pid}");

    let status = server.child.wait().expect("the server exits");
    let mut line = String::new();
    let mut last = serde_json::Value::Null;
    while server.stdout.read_line(&mut line).unwrap_or(0) > 0 {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            last = value;
        }
        line.clear();
    }
    println!("SERVE_EXIT_STATUS={status:?}");
    assert_eq!(
        status.code(),
        last["exit"].as_i64().map(|c| c as i32),
        "the status the process exits with is the one its own log line reports"
    );
    last
}
