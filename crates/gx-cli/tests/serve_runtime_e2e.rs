// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-43-2, end to end** — a server and a CLI writing to one project, and a server restarted
//! over a journal it did not write (`req/38` §148, GO condition 1 of §122).
//!
//! # What this file is the reversal of
//!
//! Three measurements from `req/182`'s adversarial audit, each taken by running the shipped binaries
//! and reading the files afterwards:
//!
//! * **H-01** (`~/.sg/audit182_probe4/probe.log`): `gx serve` committed, a `gx commit` ran beside it,
//!   the server committed again — and afterwards `gx log proof --leaf 2` answered `found: false`
//!   while `gx replay` reported `ledger_index: null`. Two writers, one ledger, and the second
//!   writer's leaf was truncated away by the next open.
//! * **H-02** (`~/.sg/audit182_probe/probe.log` P11): the server was restarted over its own journal
//!   and answered `404` for a transformation that journal held; `GET /transformations` returned a
//!   page whose every field was `null`; the undo endpoint answered `404`.
//! * **H-03 / M-12**: `Engine::recover` was called from production code **zero** times, so 43 §7's
//!   "run the recovery at start-up" had no implementation behind it, and `ledger_agrees` — which is
//!   only meaningful once the journal-witnessed frontier exists — was false on every restart.
//!
//! Every assertion below is one of those three, inverted. The suite is deliberately **one test**:
//! the facts are about a sequence (write, coexist, write, restart, read), and three tests would each
//! have to rebuild the sequence and would then be measuring their own setup three times.
//!
//! # Timing, and the retry rule
//!
//! `crates/gx-cli/tests/ac_056.rs`'s rule, restated so a reader of this file need not find the other
//! one: **zero retries**. Every wait is a bounded socket read or a bounded poll for the start-up
//! line, and an expiry **fails**. Nothing here re-runs a step and nothing is `#[ignore]`d.
//!
//! # `cfg(unix)`
//!
//! `SIGTERM` and `flock` are the two things this file needs that Windows spells differently, and
//! `req/190` §3-2 declares the Windows and 9p lock behaviour **not measured**. A suite that ran
//! there would be claiming a measurement this lane did not take.

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use support::{pipeline, run};

/// How long a socket read may block before the probe fails.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server has to print its start-up line.
const STARTUP_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve`, its address, and its standard output.
///
/// The shape `ac_056.rs` uses, copied rather than shared: a test binary is its own crate, and what
/// this fixture is worth is that it drives the **real** binary over a **real** socket.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
    start: serde_json::Value,
}

impl Serving {
    fn start(project: &std::path::Path, home: &std::path::Path, key_id: &str) -> Self {
        let token = "dr432-runtime-token".to_string();
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
        let start: serde_json::Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| {
            panic!("44 §1.2 asks for one structured start-up line; got {line:?} ({e})")
        });
        println!("SERVE_START={start}");
        assert_eq!(start["event"], serde_json::json!("gx.serve.started"));
        let addr = start["bind"]
            .as_str()
            .expect("the bound address")
            .to_string();
        Self {
            child,
            addr,
            token,
            stdout,
            start,
        }
    }

    /// One HTTP/1.1 request on its own connection, read to the end: status, headers, body.
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, String, String) {
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
        let (headers, body) = text
            .split_once("\r\n\r\n")
            .map_or((String::new(), String::new()), |(h, b)| {
                (h.to_string(), b.to_string())
            });
        (status, headers, body)
    }

    fn json(&self, method: &str, path: &str) -> serde_json::Value {
        let (status, _, text) = self.request(method, path, None);
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{method} {path} answered {status} with {text:?} ({e})"))
    }

    /// Drive the create, verify and commit endpoints, and answer with the transformation id.
    fn commit_over_http(&self, locator: &str, goal: &str, key_id: &str) -> String {
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": key_id } },
        });
        let (created_status, _, created_body) =
            self.request("POST", "/v1/candidates", Some(&intent));
        assert_eq!(created_status, 201, "create: {created_body}");
        let created: serde_json::Value = serde_json::from_str(&created_body).expect("json");
        let id = created["id"].as_str().expect("an id").to_string();

        let (verify_status, _, verify_body) =
            self.request("POST", &format!("/v1/candidates/{id}/verify"), None);
        assert_eq!(verify_status, 200, "verify: {verify_body}");
        let (commit_status, _, commit_body) =
            self.request("POST", &format!("/v1/candidates/{id}/commit"), None);
        assert_eq!(commit_status, 200, "commit: {commit_body}");
        id
    }
}

/// `SIGTERM`, wait, and hand back the shutdown line.
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
    println!("SERVE_EXIT={status:?} LAST={last}");
    last
}

/// 🔴 **GO condition 1** (`req/38` §122): one project, two writers, and a restart.
///
/// | # | fact asserted | the measurement it reverses |
/// |---|---|---|
/// | 1 | the start-up line carries `ledger_agrees: true` | H-03/M-12 (`recover` never ran) |
/// | 2 | a `gx commit` beside a live server succeeds and the substrate moves | H-01 |
/// | 3 | the ledger holds **three** leaves | H-01 (a writer's leaf was truncated) |
/// | 4 | `gx log proof --leaf 2` is `found: true` | H-01 (`found: false` in probe4) |
/// | 5 | `gx replay` matches for all three | H-01 (`ledger_index: null` in probe4) |
/// | 6 | after a restart, the read endpoint is `200` and no row's state is null | H-02 |
/// | 7 | after a restart, undo is `200` or a **named** refusal, never a bare `404` | H-02 |
/// | 8 | a blocked writer gets `BUSY` — CLI exit 1 with a `gx_code`, HTTP `503` with `Retry-After` | the refusal DR-43-2 adds |
#[test]
fn a_server_and_a_cli_share_one_project_and_the_server_survives_a_restart() {
    let fixture = pipeline("dr432_serve_runtime", "before\n");
    // `gx serve` **opens** a project rather than creating one, so one CLI verb makes the `.gx/`.
    assert_eq!(fixture.submit("warm\n").code, 0);
    let locator = fixture.target.display().to_string();

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    // ---- 1. the start-up gate reported ------------------------------------------------
    println!("RUNTIME_1={}", server.start["runtime"]);
    assert_eq!(
        server.start["runtime"]["ledger_agrees"],
        serde_json::json!(true),
        "H-03/M-12: `recover` runs at start-up and `ledger_agrees` is asked, both inside the one \
         structured line 44 §1.2 specifies"
    );

    // ---- 2. HTTP commit, CLI commit beside it, HTTP commit again ----------------------
    let http_one = server.commit_over_http(&locator, "http-one\n", &fixture.key_id);
    // A whole second process driving submit, plan, verify and commit against the same `.gx/` while
    // the server holds an open engine over it: `req/182` H-01's exact arrangement.
    let cli_one = fixture.commit_one("cli-one\n");
    let http_two = server.commit_over_http(&locator, "http-two\n", &fixture.key_id);
    println!("IDS http1={http_one} cli={cli_one} http2={http_two}");
    assert_eq!(
        fixture.target_contents(),
        "http-two\n",
        "the substrate really moved three times; a coexistence probe whose writes did not land \
         would be the worst possible pass"
    );

    // ---- 3. three commits, three leaves ------------------------------------------------
    let checkpoint = server.json("GET", "/v1/ledger/checkpoint");
    println!("CHECKPOINT={checkpoint}");
    assert_eq!(
        checkpoint["tree_size"],
        serde_json::json!(3),
        "H-01: the server's in-memory tree had staged the CLI's index and the leaf was lost. \
         Catching up under `.gx/LOCK` before appending is what makes this three"
    );

    // ---- 4/5. the last leaf proves, and every row replays ------------------------------
    let proof = run(fixture.gx().args(["log", "proof", "--leaf", "2"]));
    println!("PROOF exit={} {}", proof.code, proof.stdout.trim());
    assert_eq!(proof.code, 0, "gx log proof --leaf 2: {}", proof.stderr);
    let proof_json = proof.json();
    // 🔴 `found` is the **refusal**'s own field: `ledger::proof` answers it with exit 6 when the
    // leaf is out of range, and answers an `InclusionProof` with exit 0 when it is not. probe4 read
    // that refusal object for a commit that had returned 200, so the reversal is its absence.
    assert!(
        proof_json.get("found").is_none(),
        "H-01's headline measurement: probe4 read the refusal object here for a commit that had \n         answered 200. {proof_json}"
    );
    assert_eq!(
        proof_json["leaf_index"],
        serde_json::json!(2),
        "{proof_json}"
    );
    assert_eq!(
        proof_json["tree_size"],
        serde_json::json!(3),
        "{proof_json}"
    );
    // And by transformation id, which is the road probe4 walked: the server's **second** commit is
    // the leaf that used to be missing from the tree a second process could read.
    let by_id = run(fixture.gx().args(["log", "proof", "--leaf", &http_two]));
    println!("PROOF_BY_ID exit={} {}", by_id.code, by_id.stdout.trim());
    assert_eq!(
        by_id.code, 0,
        "the server's second commit resolves to a leaf of the ledger's own file: {} {}",
        by_id.stdout, by_id.stderr
    );
    for id in [&http_one, &cli_one, &http_two] {
        let replayed = run(fixture.gx().args(["replay", id]));
        println!(
            "REPLAY {id} exit={} {}",
            replayed.code,
            replayed.stdout.trim()
        );
        assert_eq!(
            replayed.json()["matches"],
            serde_json::json!(true),
            "H-01: probe4 read a ledger diff with `ledger_index: null` for the server's second \
             commit. {}",
            replayed.stdout
        );
    }

    // ---- 8. both writers refuse while a third holds the lock ---------------------------
    // The lock is taken by this test process, which is the third writer and therefore exactly the
    // thing being excluded. `flock` is per open file description, so a lock this process holds is
    // one the server and the CLI both see.
    let lock_path = fixture.project.join(".gx").join("LOCK");
    {
        let held = std::sync::Arc::new(
            gx_engine::store::ProcessLock::open(&lock_path).expect("the lock file opens"),
        );
        let _guard = held.acquire_owned("the probe").expect("the lock is free");

        let goal = fixture.project.join("blocked-goal.txt");
        std::fs::write(&goal, "blocked\n").expect("write the goal");
        let busy = run(fixture
            .gx()
            .arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&fixture.target)
            .arg("--intent")
            .arg(&goal)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &fixture.key_id]));
        println!("CLI_BUSY exit={} stderr={}", busy.code, busy.stderr.trim());
        assert_eq!(
            busy.code, 1,
            "44 §1.4's 1: `req/38` §148 mints no new exit number for BUSY. {}",
            busy.stderr
        );
        let problem: serde_json::Value =
            serde_json::from_str(busy.stderr.trim()).expect("44 §1.3's problem object on stderr");
        assert_eq!(
            problem["gx_code"],
            serde_json::json!("BUSY"),
            "the exit status folds and the `gx_code` does not: {problem}"
        );

        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": "blocked\n",
            "context": "Evidence",
            "actor": { "Human": { "key": fixture.key_id } },
        });
        let (status, headers, body) = server.request("POST", "/v1/candidates", Some(&intent));
        println!("HTTP_BUSY status={status} headers={headers:?} body={body}");
        assert_eq!(
            status, 503,
            "44 §2.3 has no word for a busy writer and DR-43-2 adds one: {body}"
        );
        assert!(
            headers.to_lowercase().contains("retry-after: 1"),
            "a 503 without `Retry-After` leaves the caller to invent a schedule: {headers:?}"
        );
        let problem: serde_json::Value = serde_json::from_str(&body).expect("problem+json");
        assert_eq!(problem["gx_code"], serde_json::json!("BUSY"), "{problem}");
    }
    // The guard is dropped here: the project is free again, and the restart below depends on it.

    // ---- restart -----------------------------------------------------------------------
    let stopped = shut_down(server);
    assert_eq!(stopped["event"], serde_json::json!("gx.serve.stopped"));
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    println!("RUNTIME_2={}", server.start["runtime"]);
    assert_eq!(
        server.start["runtime"]["ledger_agrees"],
        serde_json::json!(true),
        "the gate is asked on every start, not only the first"
    );
    assert!(
        server.start["runtime"]["journal_rows"]
            .as_u64()
            .is_some_and(|n| n >= 3),
        "H-02: a restarted engine holds the journal's rows now. {}",
        server.start["runtime"]
    );

    // ---- 6. the restarted server can read its own journal ------------------------------
    for id in [&http_one, &cli_one, &http_two] {
        let (status, _, body) = server.request("GET", &format!("/v1/transformations/{id}"), None);
        println!("AFTER_RESTART GET {id} -> {status} {body}");
        assert_eq!(
            status, 200,
            "H-02: this answered 404 for a row the journal held. {body}"
        );
        let row: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert!(
            !row["state"].is_null(),
            "the state comes from the shadow and the body does not; a real state beside a null \
             `transformation` is the honest form of that (bodies are lane R2): {row}"
        );
    }
    let listed = server.json("GET", "/v1/transformations");
    println!("AFTER_RESTART_LIST={listed}");
    let items = listed["items"].as_array().expect("a page of rows");
    assert!(items.len() >= 3, "three commits are on this page: {listed}");
    for item in items {
        assert!(
            !item["state"].is_null(),
            "H-02: every field of every row was null after a restart. {item}"
        );
    }

    // ---- 7. undo answers, and never with a bare `404` ----------------------------------
    let (undo_status, _, undo_body) =
        server.request("POST", &format!("/v1/transformations/{cli_one}/undo"), None);
    println!("AFTER_RESTART_UNDO status={undo_status} body={undo_body}");
    assert_ne!(
        undo_status, 404,
        "H-02: undo answered 404 for a committed row after a restart. R1 rebuilds states and not \
         bodies, so it may still refuse — but it must refuse **by name**: {undo_body}"
    );
    if undo_status != 200 {
        let problem: serde_json::Value = serde_json::from_str(&undo_body).expect("problem+json");
        assert_ne!(
            problem["gx_code"],
            serde_json::json!("NOT_FOUND"),
            "\"this engine has never heard of it\" was the false answer; the true one names the \
             missing body: {problem}"
        );
        // 🔴 **Lane R2 moved which refusal this is** (`req/38` §148 ruling 1(iii)). R1 answered
        // `409 INVALID_STATE` "holds no body" here, because the Σ-shadow knew the state and
        // nothing held the intent; the draft archive now rebuilds the row, so the undo runs and
        // reaches DR-43-1's CAS — which refuses, correctly, because this suite commits **three**
        // transformations and `cli_one` is not the last. The world moved after it, and 43 §5.2's
        // `world-moved` row is the true answer.
        //
        // The struck expectation is kept as an arm rather than deleted: a deployment with no
        // draft archive (`NoDrafts`) still answers R1's refusal, and that arm has to stay legible.
        // What is asserted either way is what this probe was always about — the refusal **names
        // itself**.
        let detail = problem["detail"].as_str().unwrap_or_default();
        assert!(
            matches!(
                problem["gx_code"].as_str(),
                Some("PRECONDITION_CHANGED" | "INVALID_STATE")
            ),
            "the refusal is one of the two named ones: DR-43-1's CAS (the world moved after this \
             row committed) or R1's missing body (a deployment with no draft archive): {problem}"
        );
        if problem["gx_code"] == serde_json::json!("INVALID_STATE") {
            assert!(
                detail.contains("body") || detail.contains("draft"),
                "the refusal says which half is missing and where the other half comes from \
                 (req/190 §4-1 L2): {problem}"
            );
        } else {
            assert!(
                detail.contains("attested"),
                "DR-43-1's CAS names what it compared: {problem}"
            );
        }
    }

    let stopped = shut_down(server);
    println!("FINAL_SHUTDOWN={stopped}");
    assert_eq!(stopped["exit"], serde_json::json!(0));
}
