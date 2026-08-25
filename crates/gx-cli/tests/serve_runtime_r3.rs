// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R3 — the six accidents the third adversarial audit measured, each one a probe that was red
//! before the repair** (`req/222`, `req/38` §160 ruling 2).
//!
//! `req/222` §9's implementation row asked for exactly this file and named its four hardest arms
//! before they existed — "put the falsifiers down first": red if an undo answers 200 after its
//! commit receipt is deleted; red if the journal's length moves after a refused write; red if a
//! sweep after a restart expires zero rows whose deadline has passed; red if a commit answers 200
//! over a ledger that was rewritten at the same length. All four were red the day it wrote that,
//! which is why it asked for them first. (The audit's own sentence is in `req/222` §9 and stays
//! there: this crate's public face carries no Japanese, `req/38` §121.)
//!
//! Those are `a_deleted_commit_receipt_refuses_the_undo_instead_of_firing_it`,
//! `a_refused_rebuild_leaves_the_journal_where_it_was`, `a_deadline_survives_a_restart` and
//! `a_same_length_rewrite_of_the_ledger_stops_the_writing`. Two more sit beside them for the two
//! remaining highs: `a_receipt_from_another_transformation_is_not_this_ones_evidence` (H-02) and
//! `a_project_whose_two_files_disagree_has_a_door` (H-06).
//!
//! Every one of them is a **loss** condition stated as an assertion, which is why they are worth
//! more than the repairs they guard: a future hand that widens `undo_witness`, drops the tail
//! detector, or re-seats `since` at `now` will find out here rather than in the fourth audit.
//!
//! `cfg(unix)` for its three predecessors' reasons — `SIGTERM`, `flock`, and now `chmod` — and with
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
const REAPER_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve`, its address and its start-up line.
///
/// The fourth copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test binary
/// is its own crate.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
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
        let token = "r3-runtime-token".to_string();
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

    /// `POST /candidates`, answering the id **and the state the creating request itself reported**.
    ///
    /// 🔴 **`req/222` M-14** — the second value is why this differs from `serve_runtime_r2.rs`'s
    /// copy. A probe that creates a row and then `GET`s it to assert `Candidate` is racing the
    /// reaper over a TTL it deliberately made short: `req/222` §6 caught the clone-basis floor
    /// **red** on exactly that, `left: {"Aborted": "Expired"} right: "Candidate"`, under the I/O of
    /// a full-workspace run. The `201` body carries `state` read inside the same
    /// `engine_for_write` hold that planned the row, so it cannot say anything else — the
    /// assertion becomes a fact about the response rather than a bet on a timer.
    fn create_over_http(
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
        let (id, _) = self.create_over_http(locator, goal, key_id);
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

fn layout(fixture: &Pipeline) -> gx_cli::layout::Layout {
    gx_cli::layout::Layout::open(&fixture.project).expect("the project is open")
}

/// This project's ledger file, as `Engine::open` derives it.
fn ledger_path(fixture: &Pipeline) -> std::path::PathBuf {
    layout(fixture).ledger_path()
}

/// The commit receipt `.gx/receipts/` holds for one transformation.
///
/// The name is `ReceiptStore::path_of`'s: the `gx1:` text with the colon replaced, then the slot's
/// tag. Written out here rather than reached for through the store, because this probe is about
/// somebody who is **not** gx removing or swapping the file — an operator, a script, an attacker —
/// and they act on the path.
fn receipt_path(fixture: &Pipeline, id: &str) -> std::path::PathBuf {
    layout(fixture)
        .join("receipts")
        .join(format!("{}.commit.json", id.replace(':', "_")))
}

// ---------------------------------------------------------------------------
// H-01 — the witness that was not there
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-01, measured 3/3 and reproduced here before the repair** — deleting one file
/// under `.gx/receipts/` used to turn DR-43-1's refusal into an undo that destroyed a third party's
/// write.
///
/// The audit's own sequence, in one probe: commit over HTTP, let somebody who is not gx write to
/// the target, and undo. With the receipt in place that is `409 PRECONDITION_CHANGED` and the third
/// party's bytes stay — `serve_runtime_r2.rs` already holds that arm. Then **`rm` the receipt** and
/// send the identical request. Before R3 the answer was `200`, the file read `before`, and neither
/// the response nor the server's stderr said that a check had been skipped.
///
/// The assertion is not "it refuses"; it is "**it refuses with the same status as a CAS that ran
/// and failed**", because `req/38` §160 ruling 2 chose to spend no new number on the difference and
/// a client that branched on the number must see no change.
#[test]
fn a_deleted_commit_receipt_refuses_the_undo_instead_of_firing_it() {
    let fixture = pipeline("r3_receipt_deleted", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let id = server.commit_over_http(&locator, "after\n", &fixture.key_id);
    assert_eq!(fixture.target_contents(), "after\n");

    // The control arm: with evidence, and with a world nobody moved, the undo goes through and
    // **says so**. Without this the refusal below would pass on a build whose undo never works.
    let (ok_status, ok_body) = server.request(
        "POST",
        &format!("/v1/transformations/{id}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("CONTROL_UNDO status={ok_status} body={ok_body}");
    assert_eq!(ok_status, 200, "the control: {ok_body}");
    let answered: serde_json::Value = serde_json::from_str(&ok_body).expect("json");
    // 🔴 `req/222` M-12: the answer now says whether the compare-and-set ran.
    assert_eq!(
        answered["witness"],
        serde_json::json!("attested"),
        "M-12: a `200` from `/undo` used to mean 'the inverse was applied' and nothing about \
         whether anything had been compared first. body: {ok_body}"
    );
    assert_eq!(fixture.target_contents(), "before\n");

    // Now the measured accident, on a second commit.
    let second = server.commit_over_http(&locator, "second\n", &fixture.key_id);
    assert_eq!(fixture.target_contents(), "second\n");
    let third_party = "a third party wrote this\n";
    std::fs::write(&fixture.target, third_party).expect("a third party writes");

    let receipt = receipt_path(&fixture, &second);
    assert!(
        receipt.exists(),
        "the fixture depends on `.gx/receipts/{second}.commit.json` existing: {}",
        receipt.display()
    );
    std::fs::remove_file(&receipt).expect("remove the commit receipt");
    println!("RECEIPT_REMOVED={}", receipt.display());

    let (status, body) = server.request(
        "POST",
        &format!("/v1/transformations/{second}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("UNDO_WITHOUT_A_RECEIPT status={status} body={body}");
    assert_eq!(
        status, 409,
        "🔴 `req/222` H-01: this answered **200** and overwrote the third party's file. One `rm` \
         disabled DR-43-1's whole gate, and `archive_commit_receipt`'s `let _ =` reached the same \
         state with no attacker at all. body: {body}"
    );
    assert!(
        body.contains("\"gx_code\":\"PRECONDITION_CHANGED\""),
        "§160 ruling 2 mints no new code: a CAS that could not run answers as a CAS that did not \
         pass. body: {body}"
    );
    assert!(
        body.contains("commit receipt"),
        "and the detail names what is missing, so an operator knows what to restore: {body}"
    );
    assert_eq!(
        fixture.target_contents(),
        third_party,
        "🔴 the fact the status is about: the third party's bytes are still there"
    );

    // The CLI face answers the same way, which is `req/215` M-10's rule (one refusal, one name).
    // The server is stopped first: while it runs it holds `.gx/LOCK` and a CLI verb would be
    // refused with `BUSY` before it reached the question.
    shut_down(server);
    let refused = support::run(
        fixture
            .gx()
            .arg("undo")
            .arg(&second)
            .args(["--settle", "0"]),
    );
    println!(
        "CLI_UNDO_WITHOUT_A_RECEIPT exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(
        refused.code, 3,
        "44 §1.4's 3 — the number `gx undo` already had for a CAS that did not pass. Before R3 \
         this was **0** and the file was overwritten: {}",
        refused.stderr
    );
    assert!(
        refused
            .stderr
            .contains("\"gx_code\":\"PRECONDITION_CHANGED\""),
        "the same word on the terminal as on the socket: {}",
        refused.stderr
    );
    assert_eq!(fixture.target_contents(), third_party);
}

/// 🔴 **`req/222` H-01(a)** — a commit whose receipt the archive will not take is a **failed**
/// commit, and it says which half happened.
///
/// The half of H-01 that needs no attacker. `archive_commit_receipt` filed the receipt with
/// `let _ =`, so a read-only `.gx/receipts/`, a full disk or a tidy-up script produced a committed
/// row with no evidence — and before the repair above, a row with no evidence was a row whose undo
/// fired blind. Two silent failures composing into a loud one.
///
/// The directory is made unwritable with `chmod`, which is why this file is `cfg(unix)`.
#[test]
fn a_commit_whose_receipt_cannot_be_filed_is_not_reported_as_a_plain_success() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = pipeline("r3_receipt_unwritable", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    // One good commit first, so that the directory exists and the fixture is measuring the write
    // rather than the layout.
    server.commit_over_http(&locator, "one\n", &fixture.key_id);

    let receipts = layout(&fixture).join("receipts");
    let original = std::fs::metadata(&receipts)
        .expect("the receipts directory exists")
        .permissions();
    std::fs::set_permissions(&receipts, std::fs::Permissions::from_mode(0o555))
        .expect("make the receipts directory read-only");

    let (id, _) = server.create_over_http(&locator, "two\n", &fixture.key_id);
    let (verify_status, _) = server.request("POST", &format!("/v1/candidates/{id}/verify"), None);
    assert_eq!(verify_status, 200);
    let (status, body) = server.request("POST", &format!("/v1/candidates/{id}/commit"), None);
    println!("COMMIT_WITH_AN_UNWRITABLE_ARCHIVE status={status} body={body}");

    std::fs::set_permissions(&receipts, original).expect("put the permissions back");

    assert_ne!(
        status, 200,
        "🔴 `req/222` H-01(a): this answered **200** and the row it left behind had no evidence \
         for DR-43-1 to check. body: {body}"
    );
    assert!(
        body.contains("receipt"),
        "and the answer names what did not happen, rather than a bare 500: {body}"
    );
    assert!(
        body.contains("undone"),
        "and names the consequence — this row cannot be undone until the receipt is filed: {body}"
    );
}

// ---------------------------------------------------------------------------
// H-02 — the witness that was somebody else's
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-02** — a receipt does not get to choose its own file name.
///
/// `Receipt::payload()` decodes the signed bytes and verifies nothing, and before R3 nobody above
/// it checked either: not the DSSE signature, and not `payload.transformation`. So copying `T2`'s
/// commit receipt over `T1`'s file name made `T2`'s postcondition `T1`'s evidence — and since `T2`
/// is the last thing that touched the file, the CAS passed and the undo of `T1` overwrote a commit
/// that had happened.
///
/// `ReceiptStore::put`'s own doc had already written the rule down ("the payload's `transformation`
/// is what the receipt says about itself... it would let a receipt choose its own file name"). What
/// was missing was a reader that believed it.
#[test]
fn a_receipt_from_another_transformation_is_not_this_ones_evidence() {
    let fixture = pipeline("r3_receipt_swapped", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let one = server.commit_over_http(&locator, "one\n", &fixture.key_id);
    let two = server.commit_over_http(&locator, "two\n", &fixture.key_id);
    assert_eq!(fixture.target_contents(), "two\n");

    // The control: the world moved (T2 moved it), so T1's undo is refused on the evidence T1 has.
    let (before_status, before_body) = server.request(
        "POST",
        &format!("/v1/transformations/{one}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("CONTROL_UNDO_OF_T1 status={before_status} body={before_body}");
    assert_eq!(before_status, 409, "the control: {before_body}");

    std::fs::copy(receipt_path(&fixture, &two), receipt_path(&fixture, &one))
        .expect("copy T2's commit receipt over T1's file name");
    println!("RECEIPT_SWAPPED {two} -> {one}");

    let (status, body) = server.request(
        "POST",
        &format!("/v1/transformations/{one}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("UNDO_WITH_A_SWAPPED_RECEIPT status={status} body={body}");
    assert_eq!(
        status, 409,
        "🔴 `req/222` H-02: this answered **200**, and what it undid was T2's committed change. \
         body: {body}"
    );
    assert!(
        body.contains("another transformation") || body.contains("does not verify"),
        "the detail names which of the four trusts is missing: {body}"
    );
    assert_eq!(
        fixture.target_contents(),
        "two\n",
        "🔴 T2's commit is still there"
    );
}

// ---------------------------------------------------------------------------
// H-03 — the refusal that wrote something
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-03** — "Nothing was written to either row" is now true.
///
/// The rebuild road (`with_a_body` → `rebuilt`) re-plans a row this process did not plan. When
/// `Fingerprint₀` has gone stale 43 §8 forces the re-plan to name a **different** transformation,
/// and the handler refuses — but it learned that by calling `Engine::plan`, which had already
/// appended a `Planned` record for the new id. `req/222` measured all of it: `409` with the
/// sentence above, `journal_rows` 1 → 2, and the row that grew answering `GET` 200, `verify` 200
/// and `commit` 200 — a committable claim on a file a third party owned, created by a request the
/// caller was told had failed. No CAS stands anywhere on that road; DR-43-1's is on `undo` alone.
///
/// The assertion is on the **count**, because that is the fact the refusal's own sentence claims.
#[test]
fn a_refused_rebuild_leaves_the_journal_where_it_was() {
    let fixture = pipeline("r3_refused_rebuild", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let first = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (id, created_state) = first.create_over_http(&locator, "http-one\n", &fixture.key_id);
    assert_eq!(created_state, serde_json::json!("Candidate"));
    let before = first.json("GET", "/v1/healthz");
    println!("HEALTHZ_BEFORE_RESTART={before}");
    shut_down(first);

    // The world moves while nobody is serving, so the row's `Fingerprint₀` is stale and the rebuild
    // will name a different transformation.
    std::fs::write(&fixture.target, "a third party wrote this\n").expect("a third party writes");

    let second = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let rows_before = second.json("GET", "/v1/healthz")["journal_rows"]
        .as_u64()
        .expect("journal_rows");
    let (status, body) = second.request("POST", &format!("/v1/candidates/{id}/verify"), None);
    println!("STALE_VERIFY status={status} body={body}");
    let rows_after = second.json("GET", "/v1/healthz")["journal_rows"]
        .as_u64()
        .expect("journal_rows");
    println!("JOURNAL_ROWS before={rows_before} after={rows_after}");

    assert_eq!(status, 409, "the refusal itself is unchanged: {body}");
    assert!(
        body.contains("Nothing was written to either row"),
        "the sentence this probe is about: {body}"
    );
    assert_eq!(
        rows_after, rows_before,
        "🔴 `req/222` H-03: the journal went from {rows_before} row(s) to {rows_after} while the \
         caller was told nothing had been written. The row that grew was `GET`-able, verifiable \
         and committable, over a file a third party owned"
    );
    assert_eq!(
        fixture.target_contents(),
        "a third party wrote this\n",
        "and nothing reached the substrate"
    );
}

// ---------------------------------------------------------------------------
// H-04 — the deadline that did not survive a restart
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-04** — 43 T-6 is a property of the project, not of one process's memory.
///
/// `Engine::reap` walked the live table, `Engine::open` leaves the live table empty (M5H3-5), and
/// the rebuild re-seated `since` at *now* — so a restart deleted every deadline in the project.
/// The audit measured both halves: a sweep expiring **0** of 200 rows whose TTL was 1 ms, and a row
/// long past its deadline answering `verify` 200 and `commit` 200 after a restart. 43 §9.1.1's
/// "INV-L1/L2 are enforced" was true inside one process lifetime and false across the most ordinary
/// thing an operator does.
///
/// The row is created by one server and expired by another, which is the whole point: nobody calls
/// about it, and the second process has no body for it — only the name, the state and, now, the
/// moment the journal says it entered that state.
#[test]
fn a_deadline_survives_a_restart() {
    let fixture = pipeline("r3_restart_ttl", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let ttl = [
        ("GX_SERVE_TTL_MS", "200"),
        ("GX_SERVE_REAP_INTERVAL_MS", "100"),
    ];
    let first = Serving::start_with(&fixture.project, &fixture.home, &fixture.key_id, &ttl);
    let (id, created_state) = first.create_over_http(&locator, "never-verified\n", &fixture.key_id);
    println!("CREATED id={id} state={created_state}");
    // 🔴 `req/222` M-14 — read off the creating response, not off a second round trip. See
    // `Serving::create_over_http`.
    assert_eq!(
        created_state,
        serde_json::json!("Candidate"),
        "the row starts where 43 T-2 leaves it"
    );
    shut_down(first);

    // Past the deadline while nothing is running. A row whose TTL expired with no process alive is
    // exactly the row `req/222` H-04 found nobody would ever expire.
    std::thread::sleep(Duration::from_millis(600));

    let second = Serving::start_with(&fixture.project, &fixture.home, &fixture.key_id, &ttl);
    let deadline = Instant::now() + REAPER_WAIT;
    let mut last = second.json("GET", &format!("/v1/candidates/{id}"));
    while Instant::now() < deadline && last["state"] == serde_json::json!("Candidate") {
        std::thread::sleep(Duration::from_millis(100));
        last = second.json("GET", &format!("/v1/candidates/{id}"));
    }
    println!("AFTER_RESTART_SWEEP={last}");
    // 🔴 **R37 / `req/496` L-02** — 44 §2.2 L344's `state: <43's state name>` on the
    // workflow-control face. The reason is read off the permanent-record face, which still carries
    // it, so this test keeps asserting `Expired` and not merely `Aborted`.
    let permanent = second.json("GET", &format!("/v1/transformations/{id}"));
    println!("PERMANENT_RECORD_FACE={permanent}");
    assert_eq!(
        permanent["state"],
        serde_json::json!({ "Aborted": "Expired" }),
        "🔴 `req/222` H-04's row, on the face that still carries the reason"
    );
    assert_eq!(
        last["state"],
        serde_json::json!("Aborted"),
        "🔴 `req/222` H-04: this stayed `Candidate` forever. The sweep walked a table the restart \
         had emptied, and 43 §7.4 (h)'s fallback did not fire either — the rebuild re-seated the \
         clock at now, so the row came back looking newly planned"
    );

    // And the write road agrees with the sweep, which is the half an operator actually meets.
    let (verify_status, verify_body) =
        second.request("POST", &format!("/v1/candidates/{id}/verify"), None);
    println!("EXPIRED_VERIFY status={verify_status} body={verify_body}");
    assert_ne!(
        verify_status, 200,
        "🔴 an expired row answered `verify` 200 and then `commit` 200 (`req/222` H-04, 2/2): \
         {verify_body}"
    );
    let (commit_status, commit_body) =
        second.request("POST", &format!("/v1/candidates/{id}/commit"), None);
    println!("EXPIRED_COMMIT status={commit_status} body={commit_body}");
    assert_ne!(commit_status, 200, "and neither does commit: {commit_body}");
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "nothing reached the substrate"
    );
}

// ---------------------------------------------------------------------------
// H-05 — the rewrite that kept its length
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-05** — a ledger that was rewritten without changing length is not invisible.
///
/// DR-43-6's change detector was `metadata().len()` against `LedgerStore::read_offset`, and its own
/// doc named what it missed. `req/219` §5(h) answered that `ledger_agrees` would catch it. It did
/// not: `ledger_agrees` compared this process's in-memory tree with this process's in-memory
/// frontier, and neither had been re-read.
///
/// What the audit measured, end to end: one bit flipped in the tail of a live project's ledger →
/// `/healthz` **200 `ledger_agrees:true`** → `POST /candidates` → `verify` → **`commit` 200 with a
/// signed receipt** → `GET /ledger/checkpoint` **200, signed, `tree_size: 2`** → ledger 174 → 348
/// bytes → next start-up: *"opening this project removed **348** byte(s) that would not replay"*,
/// quarantine, ledger 0 bytes, refusal to start. One damaged leaf became every leaf lost, because
/// the server kept building on it.
///
/// Two repairs are asserted here at once, and they are asserted through the endpoints an operator
/// actually has: the tail is now part of the cheap detector (so a **read** sees it), and a writer
/// re-reads the ledger from disk under the lock (so a **write** cannot be laid on top of it).
#[test]
fn a_same_length_rewrite_of_the_ledger_stops_the_writing() {
    let fixture = pipeline("r3_same_length_flip", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "http-one\n", &fixture.key_id);

    let healthy = server.json("GET", "/v1/healthz");
    println!("HEALTHZ_BEFORE={healthy}");
    assert_eq!(healthy["ledger_agrees"], serde_json::json!(true));

    let ledger = ledger_path(&fixture);
    let mut bytes = std::fs::read(&ledger).expect("read the ledger");
    let before_len = bytes.len();
    assert!(
        before_len > 8,
        "the fixture needs a ledger with a record in it"
    );
    // One bit, in the last record, keeping the length exactly. The audit flipped the third byte
    // from the end; any byte inside the record does the same thing and this one is not a framing
    // byte by construction (the frame is the first four bytes of the record).
    let last = bytes.len() - 3;
    bytes[last] ^= 0b0000_0001;
    std::fs::write(&ledger, &bytes).expect("write the flipped ledger");
    let after_len = std::fs::metadata(&ledger).map(|m| m.len()).unwrap_or(0);
    println!("LEDGER_FLIPPED at={last} len_before={before_len} len_after={after_len}");
    assert_eq!(
        after_len as usize, before_len,
        "the whole point of this probe is that the length did not change"
    );

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 `req/222` H-05: this answered **200 `ledger_agrees:true`** about a project whose \
         ledger no longer held the tree the server was signing over. body: {health_body}"
    );
    assert!(
        health_body.contains("\"gx_code\":\"LEDGER_DISAGREES\""),
        "the same word DR-43-6 gave the shorter-ledger case: {health_body}"
    );

    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": locator,
        "goal": "http-two\n",
        "context": "Evidence",
        "actor": { "Human": { "key": fixture.key_id } },
    });
    let (create_status, create_body) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("CREATE_ON_A_FLIPPED_LEDGER status={create_status} body={create_body}");
    assert_ne!(
        create_status, 201,
        "🔴 the writing is what turned one damaged leaf into every leaf lost: {create_body}"
    );

    let (checkpoint_status, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    println!("CHECKPOINT status={checkpoint_status} body={checkpoint_body}");
    assert_ne!(
        checkpoint_status, 200,
        "🔴 and the server does not put its signature on a tree it cannot show: {checkpoint_body}"
    );

    let grown = std::fs::metadata(&ledger).map(|m| m.len()).unwrap_or(0);
    println!("LEDGER_AFTER_THE_ATTEMPTS len={grown}");
    assert!(
        (grown as usize) <= before_len,
        "🔴 the ledger did not grow. In `req/222` it went 174 → 348, because the server kept \
         appending good leaves on top of a damaged one — and the next start-up then found none of \
         the 348 replayable and lost the lot. It is *shorter* here rather than equal, and that is \
         DR-43-7 working: the writer's door replayed the file, found the damaged record, copied \
         the bytes aside and cut them off before refusing"
    );
    let quarantined: Vec<String> = std::fs::read_dir(ledger.parent().expect("a directory"))
        .expect("read the ledger directory")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.contains(".torn."))
        .collect();
    println!("QUARANTINED={quarantined:?}");
    assert!(
        !quarantined.is_empty(),
        "🔴 DR-43-7: the damaged bytes were copied before they were cut, so \"the log lost its \
         tail\" and \"the log never had one\" stay different observations"
    );
}

// ---------------------------------------------------------------------------
// H-06 — the state you could see and could not leave
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-06 / DR-43-8** — a project whose two files disagree has a door.
///
/// The audit's finding was not that repair was hard; it was that there was **no verb**. Every write
/// refused before it ran, `gx serve` refused to start, `Engine::recover` sat behind both gates, and
/// the refusals pointed at `gx replay`, which diagnoses and repairs nothing (DR-43-7: a read does
/// not repair). Four ways to observe the state and none to leave it.
///
/// Three things are asserted, in the order an operator meets them: the trap is real (a write is
/// refused), the door exists and runs from inside the project lock, and — the part that matters
/// most for honesty — when gx **cannot** put the tree back it says so in a sentence rather than
/// reporting a repair it did not perform. A leaf's `receipt_digest` is not in the journal (42
/// §3.13), so a leaf rebuilt here would be invented, and an invented leaf is a signed lie about a
/// Merkle tree.
#[test]
fn a_project_whose_two_files_disagree_has_a_door() {
    let fixture = pipeline("r3_repair", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "http-one\n", &fixture.key_id);
    shut_down(server);

    let ledger = ledger_path(&fixture);
    let before = std::fs::metadata(&ledger).map(|m| m.len()).unwrap_or(0);
    std::fs::write(&ledger, b"").expect("empty the ledger");
    println!("LEDGER_CUT from={before} to=0");

    // The trap, measured: every write verb refuses, by name.
    let blocked = support::run(
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
        "WRITE_INTO_A_BROKEN_PROJECT exit={} stderr={}",
        blocked.code,
        blocked.stderr.trim()
    );
    assert_eq!(blocked.code, 1);
    assert!(blocked.stderr.contains("\"gx_code\":\"LEDGER_DISAGREES\""));
    assert!(
        blocked.stderr.contains("gx repair"),
        "🔴 H-06's first half: the refusal names the door. Before R3 it named `gx replay`, which \
         diagnoses and repairs nothing: {}",
        blocked.stderr
    );

    // The door, reporting. No `--yes`, so nothing is written and no key is needed.
    let reported = support::run(fixture.gx().arg("repair"));
    println!(
        "REPAIR_REPORT exit={} stdout={}",
        reported.code,
        reported.stdout.trim()
    );
    assert_eq!(
        reported.code, 1,
        "44 §1.4's 1: the project still cannot be written to, so a `&&` chain stops here: {}",
        reported.stderr
    );
    let report = reported.json();
    assert_eq!(report["repaired"], serde_json::json!(false));
    assert_eq!(report["ledger_agrees_after"], serde_json::json!(false));
    assert_eq!(report["journal_commits"], serde_json::json!(1));
    assert_eq!(report["ledger_leaves"], serde_json::json!(0));
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|s| s.contains("cannot rebuild")),
        "🔴 H-06's second half: the one place that says what gx cannot put back, rather than \
         reporting a repair it did not perform: {report}"
    );

    // The door, running. 43 §7's recovery, from inside `.gx/LOCK`, which no other verb reaches on
    // this project.
    let repaired = support::run(
        fixture
            .gx()
            .arg("repair")
            .arg("--yes")
            .args(["--signing-key", &fixture.key_id]),
    );
    println!(
        "REPAIR_RUN exit={} stdout={}",
        repaired.code,
        repaired.stdout.trim()
    );
    let ran = repaired.json();
    // 🔴 **R6 / `req/229` M-05 + H-01 — this assertion moved, and the reason is the finding.**
    //
    // ~~`repaired: true` — "the recovery ran".~~ It never meant that: `repaired` was the `--yes`
    // flag copied into the report, so it was `true` for every `--yes` run including ones that wrote
    // nothing (`req/229` M-05 measured two such runs with both files' md5 unchanged). R6 makes the
    // key report what happened.
    //
    // And what happens here is now `false` for a second, sharper reason. This fixture cuts the
    // ledger from 174 bytes to **0** while the journal witnesses one commit — which is precisely a
    // project that has gone behind the head it already published, and DR-43-11 refuses to run 43
    // §7's recovery on one. `req/229` H-01 is the measurement of what running anyway costs: a
    // recovery that re-applies a delta from before the rollback wrote an operator's file back.
    // ∴ the door still opens, still reports, and still refuses to write — which is what H-06 asked
    // for. The counts are where "what it did" lives.
    assert_eq!(
        ran["repaired"],
        serde_json::json!(false),
        "R6: nothing was written, and `repaired` now says so rather than echoing the flag: {ran}"
    );
    assert_eq!(
        ran["mode"],
        serde_json::json!("yes"),
        "the flag is still reported, under its own key: {ran}"
    );
    assert!(
        ran["recover"].is_object(),
        "and reports what it did, in `gx serve`'s own three counts: {ran}"
    );
    assert!(
        ran["recover"]["refused"].as_u64().is_some_and(|n| n > 0),
        "R6: the recovery declined every row rather than re-applying into a rolled-back \
         project (DR-43-11): {ran}"
    );
}

/// 🔴 **The self-kill on H-05's repair, measured rather than argued** (`req/223` §8).
///
/// The tail detector catches the tail. The obvious attack on it is to flip a byte **in the middle**
/// of a ledger with more than one leaf, leaving both the length and the last record untouched — and
/// the honest question is whether anything catches that. `req/38` §160 ruling 2 asked for the
/// measurement rather than the argument, so here it is.
///
/// The answer is two-part and this probe asserts the half that is a guarantee:
///
/// * **a write is refused.** Under `.gx/LOCK` the ledger is re-opened from disk unconditionally, so
///   the replay stops at the damaged record and `ledger_agrees` compares the journal's frontier
///   against a tree that was read *during this hold*. That is the property H-05 was actually about:
///   good leaves are never laid on top of a broken one.
/// * **a read may still say `ok`.** `/healthz` runs the cheap detector only (length, then the tail
///   record), so a mid-file rewrite can be invisible to it until the next write. The status is
///   printed rather than asserted: it is a **denominator**, declared in 43 §7.5 (j) and in
///   `req/223` §7, and pinning it here would turn a limitation into a requirement and make the day
///   somebody closes it a red suite.
#[test]
fn a_mid_file_rewrite_is_caught_by_the_next_write_and_not_by_a_read() {
    let fixture = pipeline("r3_mid_file_flip", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "http-one\n", &fixture.key_id);
    server.commit_over_http(&locator, "http-two\n", &fixture.key_id);

    let ledger = ledger_path(&fixture);
    let mut bytes = std::fs::read(&ledger).expect("read the ledger");
    let before_len = bytes.len();
    // Inside the **first** record, well away from the tail: a two-leaf ledger's first leaf sits at
    // the front of the file, and byte 40 is inside its payload rather than in its four-byte frame.
    assert!(before_len > 120, "the fixture needs two records");
    bytes[40] ^= 0b0000_0001;
    std::fs::write(&ledger, &bytes).expect("write the flipped ledger");
    println!("LEDGER_MID_FLIPPED at=40 len={before_len}");

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("MIDFLIP_HEALTHZ status={health_status} body={health_body}");

    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": locator,
        "goal": "http-three\n",
        "context": "Evidence",
        "actor": { "Human": { "key": fixture.key_id } },
    });
    let (create_status, create_body) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("MIDFLIP_CREATE status={create_status} body={create_body}");
    assert_ne!(
        create_status, 201,
        "🔴 the guarantee: a writer re-reads the ledger from disk under the lock, so it cannot lay \
         a good leaf on top of a damaged one — which is the accident `req/222` H-05 measured \
         (174 → 348 bytes, then all 348 lost at the next start-up). body: {create_body}"
    );
}
