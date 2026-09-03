// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-43-2 lane R2, end to end** — the body a restarted server rebuilds, the timer that
//! enforces 43 T-6, and the health probe that stops lying about a broken project.
//!
//! R1 gave a restarted `gx serve` every row its journal witnesses and gave it **without a body**,
//! and said so in a number: `crates/gx-cli/tests/serve_runtime_e2e.rs` measured
//! `AFTER_RESTART_UNDO status=409` with `"holds no body"` in the detail, and `req/213` §7(a)
//! declared the `200` a later lane owed. `req/216` §3 pinned the same absence on the CLI face —
//! `an_undo_of_an_undo_is_refused_by_name` asserts exit **6** and says in as many words that "the
//! day DR-43-2 lane R2 lands a draft archive this becomes a different number". Two more absences
//! sat beside it: `Engine::reap` had **zero** production callers (`req/182` H-03, and the start-up
//! line said `reaper: "none: …"` about itself), and `/healthz` answered `{"status":"ok"}` about a
//! project the same server was refusing every write to (`req/219` §5(a)).
//!
//! Four tests, one per accident, for `serve_runtime_r1b.rs`'s reason rather than
//! `serve_runtime_e2e.rs`'s: these are four independent facts about one server, not one sequence.
//!
//! `cfg(unix)` for the same two reasons as its two predecessors — `SIGTERM` and `flock` — and with
//! the same declaration: Windows, WSL 9p and a synchronising client are **not measured**
//! (`req/213` §7(d), unchanged by this lane).

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use support::{pipeline, Pipeline};

/// How long a socket read may block before the probe fails.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server has to print its start-up line.
const STARTUP_WAIT: Duration = Duration::from_secs(20);
/// How long a probe waits for the reaper, before calling its absence a failure.
///
/// Twenty times the interval the fixture asks for, so that a slow machine is not a red suite and a
/// timer that never fires still is. Every wait in this file is bounded and an expiry is a failure —
/// there are no retries and no unbounded sleeps.
const REAPER_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve`, its address and its start-up line.
///
/// The third copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test binary
/// is its own crate. What is new here is `env`, which is how the two DR-43-4 overrides reach the
/// process — 33 NFR-028's deadlines are 24 and 72 hours, so a suite that wanted to watch a reaper
/// would otherwise have to wait a day.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
    start: serde_json::Value,
}

impl Serving {
    fn start(project: &std::path::Path, home: &std::path::Path, key_id: &str) -> Self {
        Self::start_with(project, home, key_id, &[])
    }

    fn start_with(
        project: &std::path::Path,
        home: &std::path::Path,
        key_id: &str,
        env: &[(&str, &str)],
    ) -> Self {
        let token = "r2-runtime-token".to_string();
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
        for (name, value) in env {
            command.env(name, value);
        }
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
                panic!("gx serve stopped before it served: {why}");
            }
        }
        let start: serde_json::Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| {
            panic!("44 §1.2 asks for one structured start-up line; got {line:?} ({e})")
        });
        println!("SERVE_START={start}");
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

    /// One HTTP/1.1 request on its own connection, read to the end.
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

    fn json(&self, method: &str, path: &str) -> serde_json::Value {
        let (status, text) = self.request(method, path, None);
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{method} {path} answered {status} with {text:?} ({e})"))
    }

    fn create_over_http(&self, locator: &str, goal: &str, key_id: &str) -> String {
        self.created_over_http(locator, goal, key_id).0
    }

    /// 🔴 **`req/222` M-14** — the id **and the state the creating request itself reported**.
    ///
    /// The `201` body carries `state`, read inside the same `engine_for_write` hold that planned
    /// the row, so it is the state at creation by construction. A probe that instead creates a row
    /// and then `GET`s it to assert `Candidate` is racing the reaper over a TTL it deliberately
    /// made short — and `req/222` §6 caught the clone-basis floor **red** on exactly that
    /// (`left: {"Aborted": "Expired"} right: "Candidate"`) under the I/O of a full-workspace run,
    /// while the same suite was 8/8 alone and 5/5 under 36 CPU burners. The race is I/O scheduling
    /// and cannot be tuned away with a bigger TTL; it can only be removed by not reading the state
    /// through a second round trip.
    fn created_over_http(
        &self,
        locator: &str,
        goal: &str,
        key_id: &str,
    ) -> (String, serde_json::Value) {
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": key_id } },
        });
        let (status, body) = self.request("POST", "/v1/candidates", Some(&intent));
        assert_eq!(status, 201, "create: {body}");
        let created: serde_json::Value = serde_json::from_str(&body).expect("json");
        (
            created["id"].as_str().expect("an id").to_string(),
            created["state"].clone(),
        )
    }

    fn commit_over_http(&self, locator: &str, goal: &str, key_id: &str) -> String {
        let id = self.create_over_http(locator, goal, key_id);
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

/// `SIGTERM`, wait for the process to go, and free the project's `.gx/LOCK` with it.
fn shut_down(mut server: Serving) {
    let pid = server.child.id().to_string();
    let killed = Command::new("kill")
        .args(["-TERM", &pid])
        .status()
        .expect("kill(1) is available on this platform");
    assert!(killed.success(), "SIGTERM was not delivered to {pid}");
    let status = server.child.wait().expect("the server exits");
    let mut line = String::new();
    while server.stdout.read_line(&mut line).unwrap_or(0) > 0 {
        line.clear();
    }
    println!("SERVE_EXIT={status:?}");
}

/// This project's ledger file, as `Engine::open` derives it.
fn ledger_path(fixture: &Pipeline) -> std::path::PathBuf {
    gx_cli::layout::Layout::open(&fixture.project)
        .expect("the project is open")
        .ledger_path()
}

/// 🔴 **The `409` R1 left, turned into a `200`** — a restarted server undoes a row it never planned.
///
/// The one fact `req/213` §7(a) owed. Before this lane the sequence below ended in
/// `409 INVALID_STATE` with `"holds no body"` in the detail: the Σ-shadow knew the row was
/// `Committed` and 42 §3.13 records names rather than bodies (ASM-9), so there was nothing to undo
/// *with*. The draft archive is the road back — `POST /candidates` files the five fields 42 §3.3
/// fixes, and after the restart the write handler asks for them, rebuilds the row through
/// `Engine::rehydrate_committed`, and the undo proceeds as if the same process had planned it.
///
/// Three assertions and the third is the one that matters: **the file goes back**. A `200` over a
/// row whose escrowed inverse was not applied would be a status, not an undo.
#[test]
fn a_restarted_server_undoes_a_row_it_never_planned() {
    let fixture = pipeline("r2_restart_undo", "before\n");
    let locator = fixture.target.display().to_string();
    // `.gx/` is created by `gx submit` alone (44 has no `gx init`), and `gx serve` opens rather
    // than creates -- a mistyped directory must not start a second, empty ledger. So the project is
    // warmed by the CLI, exactly as `serve_runtime_r1b.rs` does.
    assert_eq!(
        fixture
            .submit(
                "warm
"
            )
            .code,
        0
    );

    let first = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let id = first.commit_over_http(&locator, "after\n", &fixture.key_id);
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "the forward commit applied"
    );
    // The draft the rebuild will need, on disk, under the id the engine minted.
    let drafts = gx_cli::layout::Layout::open(&fixture.project)
        .expect("open")
        .join("drafts");
    let filed = std::fs::read_dir(&drafts).map(|d| d.count()).unwrap_or(0);
    println!("DRAFTS_AFTER_HTTP_COMMIT={filed} id={id}");
    assert!(
        filed >= 1,
        "T6 (1) L2: `POST /candidates` files the intent in `.gx/drafts/` (req/38 §148 ruling \
         1(iii)). Nothing was filed, so nothing can be rebuilt"
    );

    shut_down(first);

    let second = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let before_undo = second.json("GET", &format!("/v1/transformations/{id}"));
    println!("AFTER_RESTART_GET={before_undo}");
    // 🔴 **Stale since `c9a4056e` (2026-09-02, "fix(gx-api): rebuild transformation body on GET,
    // not just on write")** — this assertion used to hold because `GET /v1/transformations/{id}`
    // was `null`-only after a restart (the write handlers' `with_a_body`/`rebuilt` rebuild had
    // never been wired to the read face). That commit wired it, deliberately and by name
    // (`gx-api/src/handlers.rs`'s `[T-r56]`-tagged comment on `get_transformation`, citing
    // `feedback_fix_the_question_not_the_row`: "a repair confined to the row it was measured on
    // leaves the sibling that asks the same question ... still answering wrong" — the sibling
    // being this exact GET), and its own commit message names all 116 `gx-api` tests passing, but
    // did not check this crate's own `serve_runtime_r2.rs`, which is why this went unnoticed until
    // now. The row is no longer a null-bodied Σ-shadow at this point; it is already rebuilt, same
    // as a write handler would rebuild it one call later. The state name is still the fact worth
    // asserting here: the row reads as `Committed` before undo touches it, which is what makes the
    // undo below a real state transition and not a no-op.
    assert_eq!(
        before_undo["state"],
        serde_json::json!("Committed"),
        "the restarted process rebuilt the row (GET now shares the write handlers' rebuild since \
         c9a4056e) and reads it as Committed before undo: {before_undo}"
    );
    assert_ne!(
        before_undo["transformation"],
        serde_json::Value::Null,
        "GET rebuilds the body eagerly since c9a4056e; a null body here would mean the rebuild \
         regressed, not that R1's old Σ-shadow-only answer came back: {before_undo}"
    );

    let (undo_status, undo_body) = second.request(
        "POST",
        &format!("/v1/transformations/{id}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("AFTER_RESTART_UNDO status={undo_status} body={undo_body}");
    assert_eq!(
        undo_status, 200,
        "🔴 lane R2's whole point. `serve_runtime_e2e.rs` measured 409 here and named the draft \
         archive as the road to 200 (req/213 §7(a)); this is that road, walked by two real \
         processes. body: {undo_body}"
    );
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "and the undo undid something: the escrowed inverse reached the substrate"
    );

    let after = second.json("GET", &format!("/v1/transformations/{id}"));
    println!("AFTER_UNDO_GET={after}");
    assert_eq!(
        after["state"],
        serde_json::json!("Superseded"),
        "43 T-12's edge is drawn: {after}"
    );
}

/// 🔴 **DR-43-1's CAS still refuses after a restart** — the rebuild does not weaken the gate.
///
/// The adversarial half of the probe above. A rebuilt row is a row this process did not plan, so
/// the question "does the world still look the way `T_o` attested" has to be answered from the
/// **receipt** (`.gx/receipts/`, through `ReceiptArchive`) rather than from a table this process
/// never had. `req/182` H-15 is what happens when it is not asked: a third party's change silently
/// overwritten, `RC 0`, and nothing saying so.
///
/// So: commit over HTTP, restart, let a third party write, and undo. The answer must be
/// `409 PRECONDITION_CHANGED` — 44's existing word, no new number (DR-43-1 adopted (a)) — and the
/// third party's bytes must still be there afterwards.
#[test]
fn a_rebuilt_row_still_refuses_an_undo_over_a_world_that_moved() {
    let fixture = pipeline("r2_restart_cas", "before\n");
    let locator = fixture.target.display().to_string();
    // `.gx/` is created by `gx submit` alone (44 has no `gx init`), and `gx serve` opens rather
    // than creates -- a mistyped directory must not start a second, empty ledger. So the project is
    // warmed by the CLI, exactly as `serve_runtime_r1b.rs` does.
    assert_eq!(
        fixture
            .submit(
                "warm
"
            )
            .code,
        0
    );

    let first = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let id = first.commit_over_http(&locator, "after\n", &fixture.key_id);
    shut_down(first);

    // The third party. Not `gx`: a person, an editor, another tool.
    std::fs::write(&fixture.target, "a third party wrote this\n").expect("the third party writes");

    let second = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (status, body) = second.request(
        "POST",
        &format!("/v1/transformations/{id}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("RESTART_CAS status={status} body={body}");
    assert_eq!(
        status, 409,
        "DR-43-1 (a): the CAS is against `T_o`'s signed postcondition and the archive is where a \
         restarted server reads it. A rebuild that lost the witness would answer 200 here. body: \
         {body}"
    );
    assert!(
        body.contains("PRECONDITION_CHANGED"),
        "44's own word, not a new one: {body}"
    );
    assert_eq!(
        fixture.target_contents(),
        "a third party wrote this\n",
        "🔴 and nothing moved: a refused undo writes nothing (req/216 §0)"
    );
}

/// 🔴 **DR-43-4** — a `Candidate` nobody ever calls about reaches a terminal state, on the clock.
///
/// 43 §9's INV-L1 says every `Candidate` and `Verifying` reaches a terminal state in finite time,
/// and `req/182` H-03 measured that the sentence had no implementation: `Engine::reap` was called
/// by tests and by nothing that ships, expiry happened only when `verify`/`escalation`/`cancel`
/// was called **on the row itself**, and the start-up line said `reaper: "none: …"` about itself.
/// A transformation nobody touched waited for ever.
///
/// The fixture asks for a 200 ms TTL and a 100 ms sweep through the two operational overrides
/// (`GX_SERVE_TTL_MS`, `GX_SERVE_REAP_INTERVAL_MS`), because 33 NFR-028's real deadlines are 24 and
/// 72 hours and nothing driven through the binary could otherwise observe this at all. Both are
/// named in the start-up line, which is asserted here as well: a server running with shortened
/// deadlines has to **say so**, or the log stops being usable to reason about the server.
///
/// Nothing is called about the row between its creation and its expiry — the `GET` at the end is a
/// read, and a read that expired things would be the shape `req/190` §5-2 refused.
#[test]
fn the_reaper_expires_a_candidate_nobody_ever_touched() {
    let fixture = pipeline("r2_reaper", "before\n");
    let locator = fixture.target.display().to_string();
    // `.gx/` is created by `gx submit` alone (44 has no `gx init`), and `gx serve` opens rather
    // than creates -- a mistyped directory must not start a second, empty ledger. So the project is
    // warmed by the CLI, exactly as `serve_runtime_r1b.rs` does.
    assert_eq!(
        fixture
            .submit(
                "warm
"
            )
            .code,
        0
    );

    let server = Serving::start_with(
        &fixture.project,
        &fixture.home,
        &fixture.key_id,
        &[
            ("GX_SERVE_TTL_MS", "200"),
            ("GX_SERVE_REAP_INTERVAL_MS", "100"),
        ],
    );
    let reaper = &server.start["runtime"]["reaper"];
    println!("REAPER_LINE={reaper}");
    assert_eq!(
        reaper["interval_ms"],
        serde_json::json!(100),
        "the start-up line names the period it is actually running (44 §1.2's one structured line)"
    );
    assert_eq!(
        reaper["verify_ttl_ns"],
        serde_json::json!(200_000_000_i64),
        "and the deadline it is enforcing, in nanoseconds"
    );
    assert_ne!(
        server.start["reaper"],
        serde_json::json!(
            "none: TTL expiry is evaluated on the next operation that touches a row, not on a timer (DR-43-4)"
        ),
        "🔴 R1's start-up line said exactly this string about itself. DR-43-4 is the sentence being \
         removed"
    );

    // 🔴 **`req/222` M-14** — the state at creation comes off the creating response. See
    // [`Serving::created_over_http`] for the failure this removes.
    let (id, created_state) =
        server.created_over_http(&locator, "never-verified\n", &fixture.key_id);
    println!("CANDIDATE_AT_CREATION={created_state}");
    assert_eq!(created_state, serde_json::json!("Candidate"));

    let deadline = Instant::now() + REAPER_WAIT;
    let mut last = server.json("GET", &format!("/v1/candidates/{id}"));
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        last = server.json("GET", &format!("/v1/candidates/{id}"));
        if last["state"] != serde_json::json!("Candidate") {
            break;
        }
    }
    println!("CANDIDATE_AFTER_THE_SWEEP={last}");
    // 🔴 **R37 / `req/496` L-02** — the workflow-control face now publishes 44 §2.2 L344's
    // shape, `state: <43's state name>`, which is what `POST /candidates/{id}/cancel` has always
    // answered on this same surface. The flat name carries no reason, so the half of this
    // assertion that says **which** terminal this is moved to the permanent-record face below
    // rather than being dropped: `Aborted(Expired)` and `Aborted(OwnerCancelled)` are different
    // facts and this test is about the first one.
    let permanent = server.json("GET", &format!("/v1/transformations/{id}"));
    println!("PERMANENT_RECORD_FACE={permanent}");
    assert_eq!(
        permanent["state"],
        serde_json::json!({ "Aborted": "Expired" }),
        "🔴 43 T-6's row, on the face that still carries the reason. `req/502` records that          the two faces now spell one state differently, measured rather than assumed"
    );
    assert_eq!(
        last["state"],
        serde_json::json!("Aborted"),
        "🔴 43 T-6's row: `Aborted(Expired)`, reached by a timer and not by anybody calling about \
         it. Before DR-43-4 this loop ran to its deadline with the row still `Candidate`"
    );

    // The control: the sweep is the reaper's and not the `GET`'s. A read that expired rows would
    // make the assertion above pass on a build with no timer at all.
    let (another, immediately) =
        server.created_over_http(&locator, "also-never-verified\n", &fixture.key_id);
    println!("CONTROL_IMMEDIATELY_AFTER_CREATION id={another} state={immediately}");
    assert_eq!(
        immediately,
        serde_json::json!("Candidate"),
        "a read does not expire anything; the row is inside its TTL and stays where T-2 left it. \
         🔴 `req/222` M-14: read off the `201` rather than through a second round trip, so a slow \
         machine cannot turn this control into a red suite"
    );
}

/// 🔴 **DR-44-6** — the health probe stops calling a project healthy that the server refuses to
/// write to.
///
/// `req/219` closed H-01 by making `ledger_agrees` a gate both writers pass, and declared in its
/// own denominator (§5(a)) what it had not closed: `handlers::healthz` took no state, so "a server
/// that would refuse to start" and "a server whose disk broke while it ran" were indistinguishable
/// to the one endpoint an orchestrator polls. The write path refused correctly and the monitor read
/// `ok`.
///
/// The event here is the same one `serve_runtime_r1b.rs` uses — the ledger is emptied under a live
/// server, by nobody the server knows about — and the assertion is on the endpoint that used to be
/// blind to it. `LEDGER_DISAGREES` is asserted by name, because the second half of the ruling
/// (§156 2(a)) is that this condition stopped riding on `INTERNAL`.
#[test]
fn a_ledger_that_moved_makes_healthz_say_so_by_name() {
    let fixture = pipeline("r2_healthz", "before\n");
    let locator = fixture.target.display().to_string();
    // `.gx/` is created by `gx submit` alone (44 has no `gx init`), and `gx serve` opens rather
    // than creates -- a mistyped directory must not start a second, empty ledger. So the project is
    // warmed by the CLI, exactly as `serve_runtime_r1b.rs` does.
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
    server.commit_over_http(&locator, "http-one\n", &fixture.key_id);

    let (healthy_status, healthy_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_BEFORE status={healthy_status} body={healthy_body}");
    assert_eq!(healthy_status, 200);
    let healthy: serde_json::Value = serde_json::from_str(&healthy_body).expect("json");
    assert_eq!(
        healthy["ledger_agrees"],
        serde_json::json!(true),
        "DR-44-6's new member, on a project whose two files describe one tree"
    );
    assert!(
        healthy["journal_rows"].as_u64().unwrap_or(0) >= 1,
        "and the Σ-shadow's count, which is what makes 'this server has caught up' observable: \
         {healthy}"
    );

    let ledger = ledger_path(&fixture);
    let before = std::fs::metadata(&ledger).map(|m| m.len()).unwrap_or(0);
    std::fs::write(&ledger, b"").expect("empty the ledger under the running server");
    println!("LEDGER_CUT from={before} to=0");

    let (sick_status, sick_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER status={sick_status} body={sick_body}");
    assert_ne!(
        sick_status, 200,
        "🔴 req/219 §5(a): before DR-44-6 this answered 200 `{{\"status\":\"ok\"}}` while every \
         write into the same project was being refused. body: {sick_body}"
    );
    assert_eq!(sick_status, 500, "the status DR-43-6 pairs with the code");
    assert!(
        sick_body.contains("\"gx_code\":\"LEDGER_DISAGREES\""),
        "🔴 §156 ruling 2(a): the condition has a word of its own and no longer rides on \
         `INTERNAL`. body: {sick_body}"
    );

    // 🔴 The two faces answer the **same** code. `req/215` M-10's rule — a proxy speaking both must
    // not hold two names for one refusal — applied to the second refusal that has two faces.
    //
    // The server is stopped first, and that is not tidiness: while it runs it holds `.gx/LOCK`, so
    // a CLI verb would be refused with `BUSY` before it ever reached the question this asserts.
    shut_down(server);
    let refused = support::run(
        fixture
            .gx()
            .arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&fixture.target)
            .arg("--intent")
            .arg(&fixture.target)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &fixture.key_id]),
    );
    println!(
        "CLI_AGAINST_A_BROKEN_LEDGER exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(
        refused.code, 1,
        "44 §1.4's 1, unchanged (req/38 §148: no new exit number): {}",
        refused.stderr
    );
    assert!(
        refused.stderr.contains("\"gx_code\":\"LEDGER_DISAGREES\""),
        "the same word on the terminal as on the socket (req/215 M-10's rule, one refusal along): {}",
        refused.stderr
    );
}
