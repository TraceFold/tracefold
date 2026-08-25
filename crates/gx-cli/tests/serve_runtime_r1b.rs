// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-43-6 / DR-43-7, end to end** — the three things `req/215` measured a running `gx serve`
//! doing, each inverted by running the shipped binaries and reading the files afterwards.
//!
//! `req/213`'s suite closed "two processes writing to one `.gx/` corrupt the ledger". `req/215`
//! opened the door beside it: **the ledger is a second file, and it moves on its own.**
//!
//! * **H-01/H-02** (`req/215` probe (k)): with the ledger cut to 100 bytes under a live server,
//!   `POST /candidates` answered `201`, `POST .../verify` answered `200`, `POST .../commit` answered
//!   `200` **with a signed receipt**, and `GET /ledger/checkpoint` **signed** `tree_size: 3` over a
//!   file holding no leaves. `Engine::catch_up` re-read the ledger only when the *journal* had grown
//!   (`pipeline.rs`'s `if !arrived.is_empty()`), and `AppState::engine_for_write` never asked
//!   `ledger_agrees` at all, although `store.rs`'s own documentation said it did. Only the next
//!   restart refused.
//! * **H-03** (`req/215` probe (a2)): three read verbs each took a 522-byte ledger with a torn tail
//!   to **0 bytes** — `gx log proof --leaf 0`, `gx verdict-checkpoint list`, and `gx serve`'s
//!   start-up gate on its way to refusing to start and recommending `gx replay`, which walked the
//!   same road. **M-05**: the repair was never named in the start-up line either.
//! * **H-05** (`req/215` probe (stale)): five CLI commits under a live server left `GET
//!   /transformations` at one row and `GET /ledger/checkpoint` at a **signed** `tree_size: 1` over
//!   six leaves, unchanged after three seconds, until the server itself wrote something.
//!
//! Three tests and not one, unlike `serve_runtime_e2e.rs`: those eight facts were about a single
//! sequence, and these three are about three different accidents happening to the same file.
//!
//! `cfg(unix)` for `serve_runtime_e2e.rs`'s reason — `SIGTERM` and `flock`, and `req/213` §7(d)'s
//! declaration that Windows and 9p are not measured.

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use support::{pipeline, run, Pipeline};

/// How long a socket read may block before the probe fails.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server has to print its start-up line.
const STARTUP_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve`, its address and its start-up line.
///
/// The same shape `serve_runtime_e2e.rs` copied from `ac_056.rs`, copied again for the same reason:
/// a test binary is its own crate, and what the fixture is worth is that it drives the real binary
/// over a real socket. Zero retries — every wait is bounded and an expiry is a failure.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    start: serde_json::Value,
}

impl Serving {
    fn start(project: &std::path::Path, home: &std::path::Path, key_id: &str) -> Self {
        let token = "dr436-runtime-token".to_string();
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
                let mut why = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut why);
                }
                panic!("gx serve stopped before it served: {why}");
            }
        }
        let start: serde_json::Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("44 §1.2 asks for one structured line; got {line:?} ({e})"));
        println!("SERVE_START={start}");
        let addr = start["bind"]
            .as_str()
            .expect("the bound address")
            .to_string();
        Self {
            child,
            addr,
            token,
            start,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> (u16, String) {
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

    fn commit_over_http(&self, locator: &str, goal: &str, key_id: &str) -> String {
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": key_id } },
        });
        let (created_status, created_body) = self.request("POST", "/v1/candidates", Some(&intent));
        assert_eq!(created_status, 201, "create: {created_body}");
        let created: serde_json::Value = serde_json::from_str(&created_body).expect("json");
        let id = created["id"].as_str().expect("an id").to_string();
        let (verify_status, verify_body) =
            self.request("POST", &format!("/v1/candidates/{id}/verify"), None);
        assert_eq!(verify_status, 200, "verify: {verify_body}");
        let (commit_status, commit_body) =
            self.request("POST", &format!("/v1/candidates/{id}/commit"), None);
        assert_eq!(commit_status, 200, "commit: {commit_body}");
        id
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status();
        let _ = self.child.wait();
    }
}

/// This project's ledger file, as `Engine::open` derives it.
fn ledger_path(fixture: &Pipeline) -> std::path::PathBuf {
    gx_cli::layout::Layout::open(&fixture.project)
        .expect("the project is open")
        .ledger_path()
}

fn size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// 🔴 **H-01/H-02** — a ledger that moved under a live server stops it writing, and stops it signing.
///
/// The server is not restarted and is not told anything: the only event is that the file changed.
/// Before DR-43-6 it went on answering `201`/`200` and issuing signed receipts, because
/// `Engine::catch_up` only re-read the ledger when the *journal* had grown and
/// `AppState::engine_for_write` never asked whether the two agreed.
///
/// Both refusals are asserted, because they are two different repairs: the write gate is the
/// symmetry with `Session::settle` (H-01), and the checkpoint gate is the one that stops a **key**
/// being put to a statement about a tree this server cannot show (H-01's second half).
#[test]
fn a_ledger_that_moves_under_a_live_server_stops_the_writes_and_the_signature() {
    let fixture = pipeline("dr436_ledger_moves", "before\n");
    assert_eq!(fixture.submit("warm\n").code, 0);
    let locator = fixture.target.display().to_string();
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "http-one\n", &fixture.key_id);

    let (ok_status, ok_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    assert_eq!(ok_status, 200, "the healthy checkpoint: {ok_body}");
    let healthy: serde_json::Value = serde_json::from_str(&ok_body).expect("json");
    println!("CHECKPOINT_BEFORE={healthy}");

    // The event: somebody else's repair, another process, or one of the read verbs `req/215` H-03
    // caught truncating. Emptying the file is the cleanest form of it — no torn tail, no ambiguity,
    // just a ledger that no longer holds the leaf the journal witnesses.
    let ledger = ledger_path(&fixture);
    let before = size(&ledger);
    std::fs::write(&ledger, b"").expect("empty the ledger under the running server");
    println!("LEDGER_CUT from={before} to={}", size(&ledger));

    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": locator,
        "goal": "after-the-cut\n",
        "context": "Evidence",
        "actor": { "Human": { "key": fixture.key_id } },
    });
    let (create_status, create_body) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("CREATE_AFTER_CUT status={create_status} body={create_body}");
    assert_eq!(
        create_status, 500,
        "H-01: a write into a project whose two files disagree is refused, on the same question \
         `gx`'s CLI has asked since R1 (`Session::settle`)"
    );
    assert!(
        create_body.contains("ledger_agrees"),
        "the refusal names the question it failed: {create_body}"
    );

    let (checkpoint_status, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    println!("CHECKPOINT_AFTER_CUT status={checkpoint_status} body={checkpoint_body}");
    assert_eq!(
        checkpoint_status, 500,
        "H-01: and no signature is put to a tree this server cannot show. Before DR-43-6 this \
         answered 200 with a **signed** checkpoint over leaves that were not on the disk"
    );
    assert!(
        !checkpoint_body.contains("signatures"),
        "nothing signed came back: {checkpoint_body}"
    );
}

/// 🔴 **H-03 / M-05** — a read verb does not shorten a torn ledger, and the writer that does says so.
///
/// Two halves, in order, because the second is what makes the first affordable: the read verbs stop
/// repairing, and the repair moves to the process that holds the lock — which now copies the file
/// first and names both facts in the one structured start-up line 44 §1.2 asks for.
#[test]
fn read_verbs_leave_a_torn_ledger_alone_and_the_writer_quarantines_it() {
    let fixture = pipeline("dr437_read_is_not_repair", "before\n");
    fixture.commit_one("cli-one\n");
    let ledger = ledger_path(&fixture);

    let whole = size(&ledger);
    let junk = b"not-a-record-at-all";
    {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&ledger)
            .expect("append to the ledger");
        file.write_all(junk).expect("write the torn tail");
    }
    let torn_size = size(&ledger);
    assert_eq!(torn_size, whole + junk.len() as u64);
    println!("LEDGER_TORN whole={whole} torn_size={torn_size}");

    // ---- the read verbs ---------------------------------------------------------------
    for (label, args) in [
        ("gx log proof", vec!["log", "proof", "--leaf", "0"]),
        ("gx replay", vec!["replay"]),
        (
            "gx verdict-checkpoint list",
            vec!["verdict-checkpoint", "list"],
        ),
    ] {
        let out = run(fixture.gx().args(&args));
        println!(
            "READ_VERB {label} exit={} size={} stderr={}",
            out.code,
            size(&ledger),
            out.stderr.trim()
        );
        assert_eq!(
            size(&ledger),
            torn_size,
            "H-03: `{label}` changed the file it was reading. Before DR-43-7 each of these took a \
             ledger with a torn tail to 0 bytes, including the start-up gate's own recommended \
             diagnostic"
        );
    }
    assert!(
        !ledger.with_extension("").with_file_name("nothing").exists(),
        "sanity: the helper above is comparing real paths"
    );

    // ---- the writer ------------------------------------------------------------------
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let recovery = &server.start["runtime"]["recovery"];
    println!("RECOVERY={recovery}");
    assert_eq!(
        recovery["ledger"]["torn_tail_bytes"],
        serde_json::json!(junk.len()),
        "M-05: the start-up line names the repair. `req/215` probe (b) watched a journal go from \
         3140 bytes to 3118 with not one word said about it"
    );
    let quarantined = recovery["ledger"]["quarantined_to"]
        .as_str()
        .expect("M-05: and it names where the removed bytes went");
    assert!(
        std::path::Path::new(quarantined).is_file(),
        "the quarantine copy is a file that exists: {quarantined}"
    );
    assert_eq!(
        size(std::path::Path::new(quarantined)),
        torn_size,
        "and it is the whole file as it stood, so the tail can be replayed by hand"
    );
    assert_eq!(
        size(&ledger),
        whole,
        "the writer did repair — the point is that it copied first and said so"
    );
}

/// 🔴 **H-05** — a `GET` is as new as the disk, without the server having written anything.
///
/// The declared limit used to be "between two writes, a `GET` can be one CLI commit behind", and
/// `req/215` measured that it was neither one commit nor bounded in time: five CLI commits later the
/// list still held one row and the **signed** checkpoint still said `tree_size: 1`, three seconds of
/// waiting changed nothing, and only the server's own next write ended it.
///
/// No `sleep` here and no retry: the point is precisely that nothing has to be waited for.
#[test]
fn a_get_is_as_new_as_the_disk_without_the_server_writing() {
    let fixture = pipeline("dr436_get_freshness", "before\n");
    assert_eq!(fixture.submit("warm\n").code, 0);
    let locator = fixture.target.display().to_string();
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "http-one\n", &fixture.key_id);

    let (_, first) = server.request("GET", "/v1/ledger/checkpoint", None);
    let first: serde_json::Value = serde_json::from_str(&first).expect("json");
    assert_eq!(first["tree_size"], serde_json::json!(1));

    // Three whole CLI processes, writing to the same project, while the server sits still.
    for goal in ["cli-one\n", "cli-two\n", "cli-three\n"] {
        fixture.commit_one(goal);
    }

    let (status, body) = server.request("GET", "/v1/ledger/checkpoint", None);
    println!("CHECKPOINT_AFTER_CLI status={status} body={body}");
    let after: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(
        after["tree_size"],
        serde_json::json!(4),
        "H-05: the signed head is the head that is on the disk. The server has not written since \
         its own commit"
    );

    let (list_status, list_body) = server.request("GET", "/v1/transformations", None);
    let list: serde_json::Value = serde_json::from_str(&list_body).expect("json");
    let rows = list["items"].as_array().map_or(0, Vec::len);
    println!("LIST_AFTER_CLI status={list_status} rows={rows}");
    assert_eq!(list_status, 200, "the list still answers: {list_body}");
    assert!(
        rows >= 4,
        "H-05: the audit list names the rows the journal holds, not the rows this process wrote: \
         {list_body}"
    );
}
