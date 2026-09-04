// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **43 §7.9 Model A — the accidents, in one suite** (`req/38` §171 ruling 2(j), `req/232`).
//!
//! # Why this file exists as a bundle rather than as five probes in five suites
//!
//! 43 §7.9 splits the threat model in two. **Model B** — an adversary who can write to `.gx/` —
//! cannot be closed by anything inside `.gx/`, and the eleven suites before this one measure that
//! boundary from every door the audits found. **Model A** is the half where H = 0 is the target:
//! a crash, a power cut, a partial write, a second process, a restart, a mis-edit, an older tool,
//! and any third party who cannot write to the project directory.
//!
//! The ruling asks for that half to be **one suite**, so that the eighth adversarial audit has a
//! named surface to attack rather than a claim spread across a dozen files. Every probe here is a
//! Model A event, and each one asserts both halves of what Model A promises:
//!
//! * the detectors **do not fire** on an accident that left the project sound — a monotonicity
//!   check that takes a healthy project offline after a power cut is worse than the finding it was
//!   built for;
//! * the detectors **do fire**, and the recovery does not write, when the accident left the project
//!   in a state gx cannot vouch for.
//!
//! # What is *not* in here, stated rather than implied
//!
//! Model B (a head deleted, replaced, or rolled back to an older genuine copy) lives in
//! `serve_runtime_r6.rs` and `serve_runtime_r7.rs`, where the arms that measure an open hole assert
//! that gx **passes** the project and say what that costs. Windows, WSL 9p and synchronising
//! clients are not measured anywhere (`req/213` §7(d), carried unchanged for an eleventh lane);
//! every byte here is ext4 through the real binary.

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use support::{pipeline, Pipeline};

const READ_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve` — the ninth copy of `ac_056.rs`'s shape (a test binary is its own crate).
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Serving {
    fn start(project: &Path, home: &Path, key_id: &str) -> Self {
        Self::try_start(project, home, key_id)
            .unwrap_or_else(|why| panic!("gx serve was expected to serve and did not: {why}"))
    }

    fn try_start(project: &Path, home: &Path, key_id: &str) -> Result<Self, String> {
        Self::try_start_with_stderr(project, home, key_id, Stdio::piped())
    }

    /// 🔴 **R16 / `req/262` H-01** — the same start-up, with the error stream pointed where the
    /// caller says.
    ///
    /// The one thing `req/262` H-01 needed and no probe in this suite could do: a server whose
    /// standard error refuses every write. The start-up line is on standard **output**, so nothing
    /// about the handshake changes; what changes is that a `note` inside a handler now has a
    /// destination that says no.
    fn try_start_with_stderr(
        project: &Path,
        home: &Path,
        key_id: &str,
        stderr: Stdio,
    ) -> Result<Self, String> {
        let token = "model-a-token".to_string();
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
            .stderr(stderr);
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
                return Err(why);
            }
        }
        let start: serde_json::Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("one structured start-up line; got {line:?} ({e})"));
        println!("SERVE_START_RUNTIME={}", start["runtime"]);
        let addr = start["bind"].as_str().expect("bound").to_string();
        Ok(Self {
            child,
            addr,
            token,
            stdout,
        })
    }

    fn request(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> (u16, String) {
        let mut socket = TcpStream::connect(&self.addr).expect("connect");
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

    /// 🔴 **R10 / `req/238` §3** — a request that is answered `503 BUSY` is **sent again**, after
    /// the wait the answer itself names.
    ///
    /// `req/38` §176 ruling 2 sent "this probe is load-sensitive" to the tenth audit, and `req/238`
    /// §3 found the cause: not the engine and not a timeout, but this helper. DR-43-2 makes the
    /// project lock per-operation and **refuses rather than queues**, so `503` with
    /// `{"gx_code":"BUSY","retry_after_ms":50}` is the correct answer whenever a CLI writer holds
    /// the lock — and `three_processes_writing_at_once…` runs two CLI writers on purpose. The
    /// helper asserted `201`/`200`/`200` unconditionally, so the probe was red exactly when the
    /// exclusion it is testing did its job (measured: alone 5/5 green, under CPU load 4/5, on 9p
    /// 1/1 red, every red the same `assert_eq!(status, 200)` against a 503).
    ///
    /// The fix is not to weaken the assertion. `retry_after_ms` is on the wire because `req/38`
    /// §156 ruling 2 put it there for machines to read; this is a machine reading it. What is
    /// asserted afterwards is stronger than before, not weaker: every request ends in its success
    /// status, every refusal on the way was an **explicit** `BUSY` and not a hang or a 500, and the
    /// count of them is returned so the caller can assert that the exclusion actually fired.
    fn commit_over_http(&self, locator: &str, goal: &str, actor_key: &str) -> (String, usize) {
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": actor_key } },
        });
        let mut busy = 0usize;
        let (body, n) = self.persisting("POST", "/v1/candidates", Some(&intent), 201, "create");
        busy += n;
        let created: serde_json::Value = serde_json::from_str(&body).expect("json");
        let id = created["id"].as_str().expect("an id").to_string();
        let (_, n) = self.persisting(
            "POST",
            &format!("/v1/candidates/{id}/verify"),
            None,
            200,
            "verify",
        );
        busy += n;
        let (_, n) = self.persisting(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            None,
            200,
            "commit",
        );
        busy += n;
        (id, busy)
    }

    /// 🔴 **R10 / `req/238` §3** — one request, resent for as long as the answer says `BUSY`.
    ///
    /// Returns the successful body and **how many `BUSY` answers came first**, which is the
    /// denominator `req/236` §3 (iv) asked for: a probe that retries silently could pass on a
    /// build where the lock never engaged, and that is a different system.
    ///
    /// Every non-`BUSY` refusal fails immediately — the retry is for the one refusal whose correct
    /// response is a retry (44 §2.3's `BUSY` row) and for no other. The deadline is a bound on the
    /// whole exchange so that a lock nobody releases is a red probe rather than a hung one.
    fn persisting(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        expect: u16,
        what: &str,
    ) -> (String, usize) {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut busy = 0usize;
        loop {
            let (status, answer) = self.request(method, path, body);
            if status == expect {
                return (answer, busy);
            }
            assert_eq!(
                status, 503,
                "{what}: the only status this probe waits on is DR-43-2's 503 BUSY, and this is \
                 not one: {answer}"
            );
            let problem: serde_json::Value = serde_json::from_str(&answer).unwrap_or_else(|e| {
                panic!("{what}: 503 carries a problem object; got {answer} ({e})")
            });
            assert_eq!(
                problem["gx_code"], "BUSY",
                "{what}: a 503 that is not BUSY is not a refusal to retry: {answer}"
            );
            let wait = problem["retry_after_ms"]
                .as_u64()
                .expect("DR-43-2's BUSY carries retry_after_ms (req/38 §156 ruling 2)");
            busy += 1;
            assert!(
                Instant::now() < deadline,
                "{what}: still BUSY after 60 s and {busy} refusals — the lock is per-operation \
                 (DR-43-2) and something is holding it"
            );
            std::thread::sleep(Duration::from_millis(wait.max(1)));
        }
    }

    /// 🔴 The power cut. `SIGKILL` — no handler runs, no file is closed, nothing is flushed that
    /// was not already flushed by the write that returned.
    fn power_cut(mut self) {
        let pid = self.child.id().to_string();
        let killed = Command::new("kill")
            .args(["-KILL", &pid])
            .status()
            .expect("kill(1) is available on this platform");
        assert!(killed.success(), "SIGKILL was not delivered to {pid}");
        let status = self.child.wait().expect("the server dies");
        println!("POWER_CUT pid={pid} status={status:?}");
        // `Drop` still runs and finds the child already reaped, which is the same no-op every other
        // suite's shut-down leaves behind.
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

fn shut_down(mut server: Serving) {
    let pid = server.child.id().to_string();
    let _ = Command::new("kill").args(["-TERM", &pid]).status();
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

fn head_path(fixture: &Pipeline) -> PathBuf {
    layout(fixture).head_path()
}

fn version_path(fixture: &Pipeline) -> PathBuf {
    fixture.project.join(".gx").join("VERSION")
}

/// Everything in `.gx/checkpoints/`, sorted — the residue check.
fn checkpoint_dir(fixture: &Pipeline) -> Vec<String> {
    let dir = fixture.project.join(".gx").join("checkpoints");
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Every `<file>.torn.<n>-<m>` beside the ledger directory's files.
fn torn_copies(fixture: &Pipeline) -> Vec<String> {
    let dir = layout(fixture)
        .journal_path()
        .parent()
        .expect("the ledger directory")
        .to_path_buf();
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .expect("read the ledger directory")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains(".torn."))
        .collect();
    out.sort();
    out
}

fn repair_report(fixture: &Pipeline, yes: bool) -> (i32, serde_json::Value) {
    let mut command = fixture.gx();
    command.arg("repair");
    if yes {
        command
            .args(["--signing-key", &fixture.key_id])
            .arg("--yes");
    }
    let run = support::run(&mut command);
    println!(
        "REPAIR yes={yes} exit={} stdout={}",
        run.code,
        run.stdout.trim()
    );
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json)
}

/// The tree size the recorded head states, read off the file rather than through the engine.
fn head_tree_size(fixture: &Pipeline) -> u64 {
    let raw = std::fs::read(head_path(fixture)).expect("read the head");
    let head: serde_json::Value = serde_json::from_slice(&raw).expect("the head is JSON");
    head["checkpoint"]["tree_size"]
        .as_u64()
        .expect("a tree size")
}

// ---------------------------------------------------------------------------
// Model A — the power cut
// ---------------------------------------------------------------------------

/// 🔴 **Model A: power loss, three times, with the detector being rewritten on every commit.**
///
/// R6 put a signed head beside the two append-only files and R7 gave it a second signature and a
/// digest of `.gx/VERSION`. All three are written on the commit road, which means the crash window
/// this suite is about now has **four** files in it rather than two. The failure this probe exists
/// to catch is a detector that is found half-written after a crash — a fail-**open** detector on
/// exactly the event it was built to survive — so the assertions are: no `head.json.tmp` residue,
/// no quarantined tail, a head that never goes backwards across the rounds, and a project that
/// opens, serves and reports itself healthy after every cut.
#[test]
fn a_power_cut_during_commits_leaves_a_project_that_opens_and_a_head_that_never_goes_back() {
    let fixture = pipeline("model_a_power_cut", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let mut previous_head = 0u64;
    for round in 0..3 {
        let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
        server.commit_over_http(&locator, &format!("round-{round}-a\n"), &fixture.key_id);
        server.commit_over_http(&locator, &format!("round-{round}-b\n"), &fixture.key_id);
        server.power_cut();

        assert_eq!(
            checkpoint_dir(&fixture),
            vec!["head.json".to_string()],
            "round {round}: a head that can be found half-written is a detector that fails open on \
             the crash it exists for — the write is tmp + fsync + rename + directory fsync"
        );
        let now = head_tree_size(&fixture);
        assert!(
            now > previous_head,
            "round {round}: the recorded head moved forward ({previous_head} -> {now})"
        );
        previous_head = now;

        let (code, report) = repair_report(&fixture, false);
        assert_eq!(
            code, 0,
            "round {round}: a power cut is not damage: {report}"
        );
        assert_eq!(report["rolled_back"], serde_json::Value::Null, "{report}");
        assert_eq!(
            report["head_authenticity"], "verified",
            "round {round}: the head survived the cut and still verifies: {report}"
        );
        assert!(
            torn_copies(&fixture).is_empty(),
            "round {round}: no torn tail"
        );

        let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
        let (health, body) = server.request("GET", "/v1/healthz", None);
        assert_eq!(health, 200, "round {round}: {body}");
        let (checkpoint, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
        assert_eq!(checkpoint, 200, "round {round}: {checkpoint_body}");
        shut_down(server);
    }
    assert_eq!(
        fixture.target_contents(),
        "round-2-b\n",
        "and the substrate holds the last change that was committed"
    );
}

/// 🔴 **Model A: the crash inside 43 §7-3b's window, reconstructed byte for byte.**
///
/// The window is the gap between `ledger.append` and the `Committed` record that closes it: the
/// leaf is durable, the record is not, and the head — written *after* both — still states the
/// previous commit. A machine that loses power there comes back with a ledger one leaf ahead of its
/// journal and a head one commit behind, and 43 §7's recovery is what closes it.
///
/// This is the state R6 and R7 spend most of their refusals near, so it is the one where a
/// false positive would hurt most: a monotonicity check that read "the journal is shorter than the
/// head expects" out of an ordinary crash would refuse to start a project that has nothing wrong
/// with it. Built deterministically — the head is copied while the project is at two commits and
/// restored after the third commit's `Committed` record is cut away, which is exactly what the
/// crash leaves behind.
#[test]
fn a_crash_between_a_leaf_and_its_record_is_closed_by_the_recovery_and_not_refused() {
    let fixture = pipeline("model_a_crash_window", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    server.commit_over_http(&locator, "two\n", &fixture.key_id);
    shut_down(server);
    // The head as a crashing machine would have left it: written after the *second* commit.
    let head_at_two = fixture.home.join("head-at-two.json");
    std::fs::copy(head_path(&fixture), &head_at_two).expect("copy the head at two commits");

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "three\n", &fixture.key_id);
    shut_down(server);

    // Cut the third commit's `Committed` record away: the leaf is on the ledger, the record is not.
    let journal_path = layout(&fixture).journal_path();
    let raw = std::fs::read(&journal_path).expect("read the journal");
    let records = gx_engine::replay(&raw);
    let last_committed = records
        .records()
        .iter()
        .rposition(|r| r.kind() == "Committed")
        .expect("three commits leave three `Committed` records");
    // Walk the frames to find where that record starts.
    let mut at = 8usize; // the `GXJRNL01` marker
    let mut cut = None;
    for index in 0..=last_committed {
        let mut header = [0u8; 4];
        header.copy_from_slice(&raw[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if index == last_committed {
            cut = Some(at as u64);
            break;
        }
        at += 4 + length + 32;
    }
    let cut = cut.expect("the last `Committed` record has an offset");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&journal_path)
        .expect("open the journal");
    file.set_len(cut).expect("cut the record away");
    std::fs::copy(&head_at_two, head_path(&fixture)).expect("restore the head the crash left");
    println!("CRASH_WINDOW journal_cut_to={cut}");

    // 🔴 The recovery closes it, and none of R6's or R7's gates fire on the way.
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (health, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "an ordinary crash is not a refusal: {body}");
    shut_down(server);

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["rolled_back"], serde_json::Value::Null, "{report}");
    assert_eq!(
        report["journal_commits"], report["ledger_leaves"],
        "{report}"
    );
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "and the change that reached the ledger is the change on the disk"
    );
    assert_eq!(
        head_tree_size(&fixture),
        3,
        "the head caught up with the tree once the commit was finished"
    );
}

// ---------------------------------------------------------------------------
// Model A — co-tenancy and restarts
// ---------------------------------------------------------------------------

/// 🔴 **Model A: a second process writing while a server holds the project.**
///
/// DR-43-2 made `.gx/LOCK` per-operation precisely so that a server and a CLI can share a project,
/// and every release since has had to show that its new detector does not turn that into a
/// refusal. R7 adds two: the digest of `.gx/VERSION` (which the writer's door **stamps** on first
/// open) and the signing road's file lock. Both are exercised here from the other process's side.
#[test]
fn co_tenancy_a_cli_and_a_server_write_the_same_project_without_tripping_a_gate() {
    let fixture = pipeline("model_a_co_tenancy", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "server-one\n", &fixture.key_id);

    // The CLI writes while the server is up and idle.
    let committed = fixture.commit_one("cli-one\n");
    assert!(!committed.is_empty());
    let (health, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "{body}");
    // The server signs after the CLI wrote: under the lock, so the answer is current rather than
    // the tree this process last saw (`req/232` M-08).
    let (status, signed_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    assert_eq!(status, 200, "{signed_body}");
    let signed: serde_json::Value = serde_json::from_str(&signed_body).expect("json");
    assert_eq!(
        signed["tree_size"], 2,
        "the signature covers the tree as it is, not as this process last read it: {signed_body}"
    );
    server.commit_over_http(&locator, "server-two\n", &fixture.key_id);
    shut_down(server);

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["ledger_leaves"], 3, "{report}");
    assert_eq!(report["head_authenticity"], "verified", "{report}");
    assert_eq!(fixture.target_contents(), "server-two\n");
}

/// 🔴 **Model A: a restart reads back exactly what the last process wrote.**
///
/// The cheapest probe in the suite and the one whose absence would be hardest to notice: every
/// refusal R6 and R7 added is evaluated at **open**, so an ordinary stop and start is the event
/// that would surface a detector comparing a project against itself incorrectly. Three restarts,
/// no writes between them, and the answers have to be identical every time.
#[test]
fn a_restart_with_no_writes_between_reports_the_same_tree_every_time() {
    let fixture = pipeline("model_a_restart", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    shut_down(server);

    let mut answers = Vec::new();
    for _ in 0..3 {
        let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
        let (health, body) = server.request("GET", "/v1/healthz", None);
        assert_eq!(health, 200, "{body}");
        answers.push(body);
        shut_down(server);
        let (code, report) = repair_report(&fixture, false);
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["head_authenticity"], "verified", "{report}");
    }
    assert_eq!(
        answers[0], answers[1],
        "a restart that changes the answer is a detector reading the project differently each time"
    );
    assert_eq!(answers[1], answers[2]);
    assert_eq!(head_tree_size(&fixture), 1);
}

// ---------------------------------------------------------------------------
// Model A — the mis-edit
// ---------------------------------------------------------------------------

/// 🔴 **Model A: `.gx/VERSION` edited by hand — and the same file rewritten with the same bytes.**
///
/// `req/232` M-02 is the true positive: `journal_format=chained` overwritten with `legacy` took
/// R6's downgrade refusal off with one `write(2)`. Under Model A the same event is an operator with
/// an editor, a configuration-management tool, or an older binary — and the digest in the recorded
/// head catches all three.
///
/// The false positive is the other half and it is asserted in the same arm: a file **rewritten with
/// the same content** (`cat VERSION > VERSION`, a backup restored, a checkout that touches mtime)
/// digests the same and must not be refused. A detector that fired on that would make the check
/// useless in exactly the environments that most need it.
#[test]
fn a_hand_edited_declaration_is_refused_and_an_identical_rewrite_is_not() {
    let fixture = pipeline("model_a_declaration", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    shut_down(server);

    // (a) The same bytes, written again. Nothing has changed and nothing may fire.
    let original = std::fs::read(version_path(&fixture)).expect("read VERSION");
    std::fs::write(version_path(&fixture), &original).expect("rewrite VERSION with itself");
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        code, 0,
        "a file rewritten with its own bytes is not a changed file: {report}"
    );
    assert_eq!(report["rolled_back"], serde_json::Value::Null, "{report}");

    // (b) One word changed by hand.
    std::fs::write(version_path(&fixture), "1\njournal_format=legacy\n").expect("edit VERSION");
    let (code, report) = repair_report(&fixture, false);
    assert_ne!(
        code, 0,
        "the declaration this project's head was written beside changed"
    );
    assert!(
        report["rolled_back"]
            .as_str()
            .is_some_and(|why| why.contains("`.gx/VERSION`")),
        "and the refusal names the file, so an operator knows which one to restore: {report}"
    );
    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("the server does not start over it");
    assert!(refusal.contains("rolled_back"), "{refusal}");

    // (c) Put the original bytes back: the digest matches again and the project opens.
    std::fs::write(version_path(&fixture), &original).expect("restore VERSION");
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        code, 0,
        "the refusal is about the bytes, so restoring them ends it: {report}"
    );
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (health, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "{body}");
    shut_down(server);
}

// ---------------------------------------------------------------------------
// 🔴 R8 — the four arms `req/234` §7 measured this suite as not having
// ---------------------------------------------------------------------------
//
// The eighth audit read this file first and then attacked what it does not assert. Two of the five
// probes above do not measure the window their own name states: the power-cut probe kills the
// server **after** `commit_over_http` has returned `200`, so `SIGKILL` lands at a quiescent point
// and no in-flight write is ever cut; and the declaration probe's false-positive arm rewrites the
// file with **the same bytes**, which is the weakest form of "nothing changed".
//
// The four arms below are the audit's own list (`req/234` §7, §11-7), and each of them is red on
// the binary that lane measured:
//
//   (i)   `SIGKILL` **during** a commit, swept over the phase boundaries;
//   (ii)  `ledger_leaves == the number of commit receipts` after a crash — the one line that closes
//         H-01's whole class;
//   (iii) the declaration's false positive in **byte-different, meaning-identical** forms;
//   (iv)  co-tenancy with **three** processes writing at once.
//
// They are self-adversarial in the sense `req/188` §8 asks for: each one is written to fail on the
// code this same lane wrote, and (i)+(ii) fail on every binary before it.

/// Kill a `gx commit` `after_ms` into its run, and answer the exit status.
///
/// A child process rather than the server, because the CLI road is where a phase boundary can be
/// aimed at: `gx commit` is one transformation from `Canonicalized` to `Committed` and nothing else
/// is happening in the process.
fn commit_cut_at(fixture: &Pipeline, tid: &str, after_ms: u64) -> Option<i32> {
    let mut command = fixture.gx();
    let mut child = command
        .args(["commit", tid])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("gx commit starts");
    std::thread::sleep(Duration::from_millis(after_ms));
    let _ = Command::new("kill")
        .args(["-KILL", &child.id().to_string()])
        .status();
    child.wait().expect("the child dies").code()
}

/// Commit one row in `fixture` uncut, and answer how long the commit took, spawn to exit, in
/// milliseconds.
///
/// A cut aimed at a phase boundary has to be a fraction of a commit rather than a fixed count of
/// milliseconds, because a fixed count only aims at a boundary on the machine and the load it was
/// tuned on: the sweeps below were written where a commit took about 110 ms, and on a runner where
/// it took 65 ms every offset past the second landed on a process that had already exited. The
/// measurement is taken in the same project moments before the cut rather than once for the whole
/// sweep, because these suites run beside forty others and a span measured while the machine was
/// busy aims every later cut past the end of a commit made while it was not.
fn timed_commit_ms(fixture: &Pipeline, goal: &str) -> u64 {
    let tid = fixture.planned_one(goal);
    assert_eq!(
        run_verify(fixture, &tid),
        0,
        "the measured row is Admitted before it is committed"
    );
    let started = Instant::now();
    let committed = support::run(fixture.gx().args(["commit", &tid]));
    let elapsed = started.elapsed();
    assert_eq!(
        committed.code, 0,
        "the measured commit succeeds or the cut below is aimed at nothing: {}",
        committed.stderr
    );
    elapsed.as_millis() as u64
}

/// Where in one commit the cuts land, as a percentage of that commit.
///
/// The proportions the fixed sweep stood for on the machine it was written on: the first is before
/// the journal's first write and the last is after the head, so a sweep that keeps them keeps the
/// phase boundaries these probes name.
const CUT_PERCENTS: [u64; 7] = [36, 55, 68, 77, 86, 95, 109];

/// Every commit receipt filed under `.gx/receipts/`.
fn commit_receipts(fixture: &Pipeline) -> Vec<String> {
    let dir = fixture.project.join(".gx").join("receipts");
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".commit.json"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// `gx verify` as its own process, so the cut below can aim at `commit` alone.
fn run_verify(fixture: &Pipeline, tid: &str) -> i32 {
    support::run(fixture.gx().args(["verify", tid])).code
}

/// 🔴 **R8 / `req/234` H-01 + §7 (i)(ii) — a real mid-commit power cut, swept, and the one
/// assertion that closes the receipt window.**
///
/// # What this measures that the probe above it does not
///
/// `a_power_cut_during_commits_leaves_a_project_that_opens_and_a_head_that_never_goes_back` kills
/// the server after its `200`. This kills a `gx commit` **while it is inside T-11**, at seven
/// offsets that walk the critical section: before the journal's first write, inside it, around
/// `ledger.append`, around the receipt archive, around the `Committed` record, and after the head.
/// Three runs at each offset, because a single run at a fixed millisecond is a measurement of this
/// machine's scheduler.
///
/// # The assertion
///
/// After the cut and after 43 §7's recovery has run (`gx repair --yes`), **every leaf on the ledger
/// has a commit receipt**. `req/234` H-01 is exactly the failure of that sentence: the commit was
/// durable in both files and its receipt was never written, so `gx undo` refused the row forever,
/// `GET /v1/receipts/{tid}` answered 404 and `gx repair` answered `remedy: null`. The audit's own
/// words for this line are "the one line that closes the class" — it is red on every binary before
/// R8 and it does not care *how* the repair was made.
#[test]
fn a_power_cut_inside_a_commit_never_leaves_a_leaf_without_its_receipt() {
    // The offsets walk one commit, each one a percentage of a commit this fixture just made. The
    // sweep is deliberately wider than that commit at both ends so that "before anything" and
    // "after everything" are included rather than assumed.
    let mut cut_landed = 0usize;
    let mut spans: Vec<u64> = Vec::new();
    for (step, percent) in CUT_PERCENTS.into_iter().enumerate() {
        for run in 0..3 {
            let fixture = pipeline(&format!("model_a_midcut_{step}_{run}"), "before\n");
            fixture.commit_one("warm\n");
            let span_ms = timed_commit_ms(&fixture, &format!("span-{step}-{run}\n"));
            spans.push(span_ms);
            let offset = span_ms * percent / 100;
            let tid = fixture.planned_one(&format!("cut-{offset}-{run}\n"));
            assert_eq!(
                run_verify(&fixture, &tid),
                0,
                "the row is Admitted before the cut"
            );
            let code = commit_cut_at(&fixture, &tid, offset);
            if code.is_none() {
                cut_landed += 1;
            }

            // 43 §7's recovery, through the verb an operator would reach for.
            let (repair_code, report) = repair_report(&fixture, true);
            let receipts = commit_receipts(&fixture);
            let leaves = report["ledger_leaves"].as_u64().unwrap_or(u64::MAX);

            // 🔴 The line. Everything else in this probe is scaffolding for it.
            assert_eq!(
                leaves,
                receipts.len() as u64,
                "offset {offset} run {run}: the ledger holds {leaves} leaf/leaves and \
                 `.gx/receipts/` holds {} commit receipt(s). A committed row with no receipt \
                 cannot be undone and cannot be proved to anybody (req/234 H-01). repair exit \
                 {repair_code}, report {report}",
                receipts.len()
            );
            assert_eq!(
                report["receipts_missing"], 0,
                "offset {offset} run {run}: and gx says so itself: {report}"
            );
            assert!(
                torn_copies(&fixture).is_empty(),
                "offset {offset} run {run}: no torn tail"
            );
        }
    }
    // 🔴 The denominator, asserted rather than hoped for: if no `SIGKILL` ever landed on a running
    // commit, the whole sweep above measured nothing and would pass on any binary. `code.is_none()`
    // is "the child died of a signal", which is what a power cut looks like from here.
    println!("MODEL_A_MIDCUT cuts={cut_landed} spans_ms={spans:?}");
    assert!(
        cut_landed >= CUT_PERCENTS.len(),
        "the sweep has to actually cut running commits; only {cut_landed} of {} runs were killed \
         mid-flight, so this probe was measuring completed commits. The commits measured beside \
         them ran {spans:?} ms",
        CUT_PERCENTS.len() * 3
    );
}

/// 🔴 **R8 / `req/234` H-02 + §7 (iii) — the declaration's false positive, in the three forms an
/// editor actually produces.**
///
/// The probe above asserts that rewriting `.gx/VERSION` with **the same bytes** does not fire. That
/// is the weakest possible false positive and it is the reason H-02 went through this suite green:
/// the three forms below are byte-different and mean exactly the same thing, and on the binary the
/// eighth audit measured **all three took a provably intact project offline** — with a diagnosis
/// that said its ledger or its journal was shorter and told the operator to restore from a backup.
///
/// The fourth form is the control: one **word** changed is still refused, which is `req/232` M-02's
/// true positive and the entire reason this digest exists.
#[test]
fn a_declaration_that_means_the_same_thing_is_not_a_changed_declaration() {
    let fixture = pipeline("model_a_declaration_meaning", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    shut_down(server);

    let original = std::fs::read(version_path(&fixture)).expect("read VERSION");
    let text = String::from_utf8(original.clone()).expect("`.gx/VERSION` is text");

    // Byte-different, meaning-identical. Each is one ordinary editor or one round trip.
    //
    // 🔴 **R9 / `req/236` H-04 (self-adversarial 3 of 3)** — five more, and they are the ones that
    // hurt. R8 shipped three forms here and its own `normalise_declaration` doc comment said it
    // handled "a byte-order mark — an editor's, not an operator's". It did not: the digest was
    // behind a parse that read the raw first line, so a BOM, a leading blank line, bare-CR endings,
    // a UTF-16 save and two swapped lines each took `gx repair` (report **and** `--yes`),
    // `gx log proof`, `gx replay` and `gx serve` down together — `VALIDATION_ERROR`, no report, no
    // remedy, no way out. Two of the five (BOM, UTF-16) are the signature artifacts of the Windows
    // and OneDrive surface 43 §7.9 (h) (C1) still declares as **unmeasured**, reproduced here on
    // ext4. All five are red before R9.
    let utf16le = {
        let mut bytes = vec![0xff, 0xfe];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    };
    let reordered = {
        let mut lines: Vec<&str> = text.trim_end().split('\n').collect();
        lines.reverse();
        format!("{}\n", lines.join("\n")).into_bytes()
    };
    let same_meaning: [(&str, Vec<u8>); 8] = [
        ("a trailing newline", format!("{text}\n").into_bytes()),
        ("CRLF line endings", text.replace('\n', "\r\n").into_bytes()),
        (
            "a trailing space on a line",
            text.replace('\n', " \n").into_bytes(),
        ),
        ("a UTF-8 byte-order mark", {
            let mut bytes = vec![0xef, 0xbb, 0xbf];
            bytes.extend_from_slice(&original);
            bytes
        }),
        ("a leading blank line", format!("\n{text}").into_bytes()),
        (
            "bare-CR line endings",
            text.replace('\n', "\r").into_bytes(),
        ),
        ("a UTF-16 LE save", utf16le),
        ("the two lines in the other order", reordered),
    ];
    for (what, bytes) in same_meaning {
        std::fs::write(version_path(&fixture), &bytes).expect("write VERSION");
        let (code, report) = repair_report(&fixture, false);
        assert_eq!(
            code, 0,
            "{what}: the declaration says exactly what it said before, and the journal, the \
             ledger, the chain and the head are untouched. A detector that fires here is the \
             finding (req/234 H-02): {report}"
        );
        assert_eq!(
            report["rolled_back"],
            serde_json::Value::Null,
            "{what}: {report}"
        );
        assert_eq!(
            report["files_agree"], true,
            "{what}: and the two files agree, which is the fact `ledger_agrees` used to deny \
             while printing equal counts (req/234 M-03): {report}"
        );
        let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
        let (health, body) = server.request("GET", "/v1/healthz", None);
        assert_eq!(health, 200, "{what}: and the project still serves: {body}");
        shut_down(server);
    }

    // The control: one word, and the refusal is back — naming `.gx/VERSION` and nothing else.
    std::fs::write(version_path(&fixture), "1\njournal_format=legacy\n").expect("edit VERSION");
    let (code, report) = repair_report(&fixture, false);
    assert_ne!(
        code, 0,
        "a changed *value* is still a changed declaration: {report}"
    );
    let remedy = report["remedy"].as_str().unwrap_or_default().to_string();
    assert!(
        remedy.contains("`.gx/VERSION`"),
        "and the remedy names the file that moved: {remedy}"
    );
    assert!(
        !remedy.contains("shorter"),
        "🔴 and it does not say the ledger or the journal is shorter, because they are not — \
         `req/234` M-03 measured that sentence beside `journal_commits: 2 / ledger_leaves: 2`: \
         {remedy}"
    );
    assert_eq!(
        report["files_agree"], true,
        "the two files agree even while the project is refused: {report}"
    );
    std::fs::write(version_path(&fixture), &original).expect("restore VERSION");
}

/// 🔴 **R8 / `req/234` §7 (iv) — three processes writing the same project at once.**
///
/// The co-tenancy probe above writes from the CLI *while the server is idle*, so there is no
/// concurrency in it at all and neither `BUSY` nor `gx wrap` ever appears. This one runs a server
/// and two CLI writers at the same time and asserts what T6 condition ② actually promises: whatever
/// each writer's own answer was, the project ends with one tree, no torn file, and a head that
/// verifies.
#[test]
fn three_processes_writing_at_once_leave_one_tree_and_no_torn_file() {
    let fixture = pipeline("model_a_three_writers", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    let project = fixture.project.clone();
    let home = fixture.home.clone();
    let target = fixture.target.clone();
    let key = fixture.key_id.clone();
    let writers: Vec<_> = (0..2)
        .map(|n| {
            let (project, home, target, key) =
                (project.clone(), home.clone(), target.clone(), key.clone());
            std::thread::spawn(move || {
                let mut codes = Vec::new();
                for round in 0..2 {
                    let goal = project.join(format!("goal-cli-{n}-{round}.txt"));
                    std::fs::write(&goal, format!("cli-{n}-{round}\n")).expect("write the goal");
                    let mut command = Command::new(env!("CARGO_BIN_EXE_gx"));
                    command
                        .env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .arg("--project")
                        .arg(&project)
                        .args(["submit", "--substrate", "fs"])
                        .arg("--locator")
                        .arg(&target)
                        .arg("--intent")
                        .arg(&goal)
                        .args(["--context", "Evidence"])
                        .args(["--actor-key", &key]);
                    // 🔴 **R11 / `req/240` §3-3** — the exit **and** the word on stderr.
                    //
                    // The audit's finding is about the denominator: `cli_refusals` counted every
                    // non-zero exit as "the exclusion fired", and `BUSY`, `LEDGER_DISAGREES`,
                    // `DECLARATION_ABSENT` and `INTERNAL` all exit **1**. So an engine race that
                    // produced an `INTERNAL` would have satisfied the assertion that the lock
                    // worked. The probe is not weakened — it is made to count the thing it names.
                    let out = command.output().expect("gx submit runs");
                    codes.push((
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr).to_string(),
                    ));
                }
                codes
            })
        })
        .collect();
    let mut busy_answers = 0usize;
    for round in 0..2 {
        let (_, busy) =
            server.commit_over_http(&locator, &format!("server-{round}\n"), &fixture.key_id);
        busy_answers += busy;
    }
    let answers: Vec<_> = writers
        .into_iter()
        .map(|h| h.join().expect("the writer thread finishes"))
        .collect();
    println!("THREE_WRITERS cli_exit_codes={answers:?} http_busy_answers={busy_answers}");
    shut_down(server);
    // 🔴 **R10 / `req/238` §3 + `req/236` §3 (iv)** — the exclusion is asserted as a **fact of this
    // run**, from either side.
    //
    // The HTTP side now retries on `BUSY` instead of failing on it, so a build where the lock never
    // engaged would sail through the three assertions below without ever having tested anything.
    // The denominator is that at least one of the five writers (two CLI processes doing two rounds
    // each, one server doing two commits) was refused: a non-zero CLI exit **is** DR-43-2's refusal
    // — `req/238` §3 measured `[[1,1],[1,1]]` on ten runs of eleven and `[[0,1],[1,1]]` on the
    // eleventh — and `busy_answers` counts the same refusal arriving over the wire.
    let refusals: Vec<&(Option<i32>, String)> = answers
        .iter()
        .flatten()
        .filter(|(code, _)| *code != Some(0))
        .collect();
    let cli_refusals = refusals.len();
    // 🔴 **R11 / `req/240` §3-3** — every refusal is `BUSY` or this run measured something else.
    for (code, stderr) in &refusals {
        let problem: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|e| {
            panic!("44 §1.3 puts a problem object on stderr; got {stderr:?} ({e})")
        });
        assert_eq!(
            problem["gx_code"], "BUSY",
            "🔴 a writer exited {code:?} for a reason that is **not** DR-43-2's per-operation \
             lock. `BUSY`, `LEDGER_DISAGREES`, `DECLARATION_ABSENT` and `INTERNAL` all exit 1, so \
             counting non-zero exits as 'the exclusion fired' would let an engine race pass as \
             proof that the exclusion works (req/240 §3-3): {stderr}"
        );
    }
    assert!(
        cli_refusals + busy_answers > 0,
        "three writers raced and not one of them was excluded: cli={answers:?} \
         http_busy={busy_answers}. DR-43-2's lock is per-operation and refuses rather than queues, \
         so a run where nobody was refused is a run where nothing was tested"
    );

    // 🔴 What is asserted is **not** that every writer succeeded — `BUSY` is a correct answer under
    // DR-43-2's per-operation lock, and a probe that demanded success would be demanding that the
    // exclusion not work. What is asserted is that the project is one project afterwards.
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 0, "three writers are not damage: {report}");
    assert_eq!(report["rolled_back"], serde_json::Value::Null, "{report}");
    assert_eq!(report["head_authenticity"], "verified", "{report}");
    assert_eq!(report["files_agree"], true, "{report}");
    assert_eq!(report["receipts_missing"], 0, "{report}");
    assert_eq!(
        report["ledger_leaves"], 2,
        "the two HTTP commits are the two leaves; the CLI writers only submitted: {report}"
    );
    assert!(torn_copies(&fixture).is_empty(), "no torn tail: {report}");
    assert_eq!(
        checkpoint_dir(&fixture),
        vec!["head.json".to_string()],
        "and no half-written head beside it"
    );
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (health, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "{body}");
    shut_down(server);
}

// ---------------------------------------------------------------------------
// R9 — `req/236` H-01, H-02, H-03, M-04 (`req/38` §175 ruling 2)
// ---------------------------------------------------------------------------

/// The escrowed inverse's blob path for a committed row, read out of the receipt on the disk.
///
/// Through the archived document rather than through an engine, for `Pipeline::journal`'s reason:
/// a helper that asked a live `Engine` would be measuring one process's memory, and every finding
/// this section is about lives in the gap between that memory and the files.
fn inverse_blob_path(fixture: &Pipeline, tid: &str) -> PathBuf {
    let receipt = fixture
        .project
        .join(".gx")
        .join("receipts")
        .join(format!("{}.commit.json", tid.replace(':', "_")));
    let raw = std::fs::read(&receipt).expect("the commit receipt is filed");
    let doc: serde_json::Value = serde_json::from_slice(&raw).expect("a receipt is JSON");
    let payload = doc["envelope"]["payload"].as_str().expect("a payload");
    let bytes = base64_decode(payload);
    // The payload is canonical DAG-CBOR: the text key `inverse_delta` is followed by a 32-byte
    // byte string (major 2, length 32 = `0x58 0x20`). Read positionally rather than with a CBOR
    // decoder so that this helper depends on nothing this crate could get wrong in the same way.
    let key = b"minverse_delta";
    let at = bytes
        .windows(key.len())
        .position(|w| w == key)
        .expect("the payload names an inverse");
    let start = at + key.len();
    assert_eq!(
        &bytes[start..start + 2],
        &[0x58, 0x20],
        "`inverse_delta` is a 32-byte byte string"
    );
    let mut name = String::with_capacity(69);
    for byte in &bytes[start + 2..start + 34] {
        name.push_str(&format!("{byte:02x}"));
    }
    name.push_str(".blob");
    layout(fixture)
        .journal_path()
        .parent()
        .expect("the ledger directory")
        .join("journal.blobs")
        .join(name)
}

/// Standard base64, so that this suite adds no dependency for one field.
fn base64_decode(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for byte in text
        .bytes()
        .filter(|b| *b != b'=' && !b.is_ascii_whitespace())
    {
        let value = u32::try_from(
            ALPHABET
                .iter()
                .position(|c| *c == byte)
                .expect("base64 alphabet"),
        )
        .expect("64 fits");
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).expect("one byte"));
        }
    }
    out
}

/// Every `.tmp` file in the three directories a write stages through.
fn staging_residue(fixture: &Pipeline) -> Vec<String> {
    let gx = fixture.project.join(".gx");
    let mut out = Vec::new();
    for dir in [
        gx.join("checkpoints"),
        gx.join("receipts"),
        gx.join("ledger").join("journal.blobs"),
    ] {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(".tmp") {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out
}

/// 🔴 **R9 / `req/236` H-01 (self-adversarial 1 of 3) — a body that is not the body it is filed as.**
///
/// # The finding this closes
///
/// `BlobStore::put` had no temporary file, no rename and no cleanup, and its first line was
/// `if path.exists() { AlreadyPresent }`. The ninth audit filled a tmpfs and measured the result:
/// **204,800 bytes of a 400,096-byte inverse left at its own content address**, permanent (the
/// early return never rewrites), and then adopted — a *completely successful* later commit from
/// the same pre-state escrows the same inverse CID, so it took the fragment as its own undo,
/// answered `rc=0` with a signed receipt that `gx receipt verify` accepted, reported
/// `inverse_status: "Available"` over HTTP and `escrow_bodies_missing: 0` from `gx repair` — and
/// `gx undo` failed for ever with `INTERNAL` "input ends 195,262 byte(s) early".
///
/// # Why this probe does not fill a disk
///
/// A floor probe that balloons `/dev/shm` is a probe that fails for reasons that are not about gx.
/// What the full disk *produced* is a fragment at a content address, and that is written here
/// directly — which is also the state a **pre-R9 binary** leaves behind and this release still has
/// to answer for. The two assertions are the two halves of the audit's chain:
///
/// 1. the fragment is not called `Available` (`inverse_status` reads the body, not the name), and
///    `gx undo` refuses it by 44 §2.3's name rather than with `INTERNAL`;
/// 2. a later commit that escrows the same CID **replaces** it, and the undo of that commit works.
///
/// Red on every binary before R9, in both halves.
#[test]
fn a_fragment_at_a_content_address_is_neither_available_nor_adopted() {
    let fixture = pipeline("model_a_r9_fragment", "before\n");
    let first = fixture.commit_one("one\n");
    let blob = inverse_blob_path(&fixture, &first);
    let whole = std::fs::read(&blob).expect("the escrowed body is filed");
    assert!(whole.len() > 8, "the body is big enough to halve");

    // The accident, reproduced by its result: half a body at the address the whole one belongs at.
    let half = whole.len() / 2;
    std::fs::write(&blob, &whole[..half]).expect("write the fragment");

    // (1) The status is about the body.
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        report["escrow_bodies_missing"], 1,
        "a row whose body does not read back is a row with no body, and the census is what says \
         so (req/236 H-01/H-02). exit {code}, report {report}"
    );
    assert_eq!(
        report["damaged_bodies"], 1,
        "and the directory walk names the file itself: {report}"
    );
    let undone = support::run(fixture.gx().args(["undo", &first, "--settle", "0"]));
    println!(
        "R9_FRAGMENT_UNDO exit={} err={}",
        undone.code, undone.stderr
    );
    assert!(
        undone.stderr.contains("INVERSE_UNAVAILABLE"),
        "🔴 and the refusal is 44 §2.3's word for it on both faces, not `INTERNAL` (req/236 M-01): \
         {}",
        undone.stderr
    );

    // (2) The next commit from the same pre-state escrows the same CID and must not adopt it.
    std::fs::write(&fixture.target, "before\n").expect("rewind the world");
    let second = fixture.commit_one("two\n");
    assert_eq!(
        inverse_blob_path(&fixture, &second),
        blob,
        "the inverse of any change from this pre-state is the same body, which is what made the \
         adoption possible in the first place"
    );
    assert_eq!(
        std::fs::read(&blob).expect("read the body"),
        whole,
        "🔴 the fragment was replaced rather than accepted: `put` compares the bytes it holds \
         against the bytes it was offered, and only equality is `AlreadyPresent` (req/236 H-01)"
    );
    let undone = support::run(fixture.gx().args(["undo", &second, "--settle", "0"]));
    println!(
        "R9_ADOPTION_UNDO exit={} err={}",
        undone.code, undone.stderr
    );
    assert_eq!(
        undone.code, 0,
        "and the promise the receipt makes is one this project can keep: {}",
        undone.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "the undo put the world back"
    );
}

/// 🔴 **R9 / `req/236` H-02 — the census fires, from a process that did not do the commit.**
///
/// `gx repair`'s `escrow_bodies_missing` filtered `Engine::sigma()`'s escrow component, which was
/// built from a **live map** that `Engine::open` leaves empty — so in every reading process the
/// filter ran over an empty list and the count was **structurally 0**. The audit measured the two
/// answers coming out of one binary at one moment: `GET /v1/transformations` said `BodyMissing`
/// (it falls through to the Σ-shadow) while `gx repair` on the same project said `0`.
///
/// This is that measurement as a line. `gx repair` is a fresh process by construction, so a count
/// above zero here is only reachable if the census reads the journal's own fold.
#[test]
fn the_escrow_census_counts_a_body_a_second_process_cannot_find() {
    let fixture = pipeline("model_a_r9_census", "before\n");
    let tid = fixture.commit_one("one\n");
    let blob = inverse_blob_path(&fixture, &tid);
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 0, "a healthy project first: {report}");
    assert_eq!(report["escrow_bodies_missing"], 0, "{report}");

    std::fs::remove_file(&blob).expect("delete the escrowed body");
    let (_, report) = repair_report(&fixture, false);
    assert_eq!(
        report["escrow_bodies_missing"], 1,
        "🔴 the count that four documents cite has to be able to be non-zero (req/236 H-02): \
         {report}"
    );
    assert_eq!(
        report["escrow_bodies_missing_ids"].as_array().map(Vec::len),
        Some(1),
        "and it names which row: {report}"
    );
    let remedy = report["remedy"].as_str().unwrap_or_default();
    assert!(
        remedy.contains("journal.blobs"),
        "and the remedy names the directory: {remedy}"
    );
}

/// Cut the journal's last `Committed` record away, leaving 43 §7-3b's window exactly.
///
/// The deterministic twin of `commit_cut_at`: the leaf is on the ledger, the record that closes it
/// is not, and no timer had to guess where a commit was. `a_crash_between_a_leaf_and_its_record_...`
/// above builds the same state and is where the frame walk is explained.
fn cut_the_last_committed_record(fixture: &Pipeline) -> u64 {
    let journal_path = layout(fixture).journal_path();
    let raw = std::fs::read(&journal_path).expect("read the journal");
    let records = gx_engine::replay(&raw);
    let last = records
        .records()
        .iter()
        .rposition(|r| r.kind() == "Committed")
        .expect("there is a committed row to cut");
    let mut at = 8usize; // the `GXJRNL01` marker
    let mut cut = None;
    for index in 0..=last {
        let mut header = [0u8; 4];
        header.copy_from_slice(&raw[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if index == last {
            cut = Some(at as u64);
            break;
        }
        at += 4 + length + 32;
    }
    let cut = cut.expect("the last `Committed` record has an offset");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&journal_path)
        .expect("open the journal")
        .set_len(cut)
        .expect("cut the record away");
    cut
}

/// 🔴 **R9 / `req/236` H-03 (self-adversarial 2 of 3) — a recovery under the wrong key does not
/// take the project's way out with it.**
///
/// # The finding
///
/// 43 §7-3b rebuilds a `ReceiptPayload` and compares its digest against the leaf the ledger holds.
/// `key_id` is one of that payload's fields, so a recovery run under any key other than the one
/// that signed the commit **cannot** reproduce the leaf — and the mismatch arm wrote
/// `Aborted(InternalError)`, which is a terminal record, which is what 43 §7-2 makes the recovery
/// stop at. One run under the wrong key therefore removed the row's only way out permanently.
///
/// The ninth audit's paired control: ACTOR key 7 runs, 0 bricked; OTHER key 8 runs, **8** bricked;
/// `gx serve --signing-key <other>` 7 runs, **7** bricked — over a world that had already moved,
/// with `gx serve` then refusing to start at all (`LEDGER_DISAGREES`) and `gx repair` reporting the
/// row it had just aborted as `resumed: 1`. And `gx serve --signing-key <ENGINE>` is the deployment
/// 44 §1.2 and E-M6-7 write as **ordinary**.
///
/// # Two arms, because the window has two halves
///
/// R8 moved the receipt write inside the critical section and in front of the `Committed` record,
/// so most of §7-3b's window is now "the leaf is here and so is the receipt". That half is arm 1:
/// the recovery reads the key out of the document the project already holds and finishes, under
/// **either** key. The narrow half — the leaf landed and the receipt had not been filed — is arm 2:
/// there is nothing to read the key out of, so the recovery refuses **without writing a terminal
/// record** and the correct key still closes it afterwards.
///
/// Built by cutting the journal rather than by timing a kill, so the window is where it is by
/// construction rather than by luck. Both arms are red before R9.
#[test]
fn a_recovery_under_the_wrong_key_does_not_take_the_way_out_with_it() {
    for (arm, keep_receipt) in [("the receipt was filed", true), ("no receipt yet", false)] {
        let fixture = pipeline(
            &format!("model_a_r9_key_{}", usize::from(keep_receipt)),
            "before\n",
        );
        fixture.commit_one("warm\n");
        let other = fixture.another_key();
        let head_before = fixture
            .home
            .join(format!("head-{}.json", usize::from(keep_receipt)));
        std::fs::copy(head_path(&fixture), &head_before).expect("copy the head");

        let tid = fixture.commit_one("two\n");
        let cut = cut_the_last_committed_record(&fixture);
        std::fs::copy(&head_before, head_path(&fixture)).expect("restore the head the crash left");
        if !keep_receipt {
            std::fs::remove_file(
                fixture
                    .project
                    .join(".gx")
                    .join("receipts")
                    .join(format!("{}.commit.json", tid.replace(':', "_"))),
            )
            .expect("remove the receipt the crash had not filed yet");
        }
        let (_, before) = repair_report(&fixture, false);
        println!("R9_KEY arm={arm} cut={cut} before={before}");
        assert_eq!(
            before["ledger_agrees_after"], false,
            "{arm}: the fixture has to actually be in 43 §7-3b's window: {before}"
        );

        // The wrong key, through the verb an operator reaches for.
        let wrong = support::run(
            fixture
                .gx()
                .args(["repair", "--yes", "--signing-key", &other]),
        );
        let wrong_report: serde_json::Value =
            serde_json::from_str(wrong.stdout.trim()).unwrap_or(serde_json::Value::Null);
        println!("R9_KEY arm={arm} wrong_exit={} {wrong_report}", wrong.code);
        if keep_receipt {
            assert_eq!(
                wrong_report["ledger_agrees_after"], true,
                "🔴 {arm}: the key that signed the commit is written down in the receipt this \
                 project already holds, so a recovery under any key can rebuild the payload the \
                 leaf witnessed (req/236 H-03): {wrong_report}"
            );
        } else {
            assert_eq!(
                wrong_report["recover"]["payload_mismatch"], 1,
                "🔴 {arm}: the refusal is counted as itself: {wrong_report}"
            );
            assert_eq!(
                wrong_report["recover"]["resumed"], 0,
                "🔴 {arm}: an aborted row was reported as `resumed: 1` before R9: {wrong_report}"
            );
        }

        // 🔴 The line: the right key still works afterwards, in both arms.
        let (code, after) = repair_report(&fixture, true);
        assert_eq!(
            after["ledger_agrees_after"], true,
            "🔴 {arm}: no run under the wrong key may remove this project's way out (req/236 \
             H-03). exit {code}, report {after}"
        );
        assert_eq!(
            after["receipts_missing"], 0,
            "{arm}: and the receipt the leaf needs is filed: {after}"
        );
        assert_eq!(
            fixture.target_contents(),
            "two\n",
            "{arm}: the change that reached the ledger is the change on the disk"
        );
    }
}

/// 🔴 **R9 / `req/236` M-04 — a crash leaves no staging file behind that `gx repair --yes` will not
/// sweep.**
///
/// The audit's own sweep found `*.commit.json.tmp` in 5 of 33 mid-commit kills and `head.json.tmp`
/// in 2, with `gx repair` saying nothing about either and `--yes` removing neither. They break
/// nothing; what they break is the belief that this verb reports everything it can see.
#[test]
fn a_crash_leaves_no_staging_file_a_repair_will_not_sweep() {
    // The walk of the probe above, from its second offset on: this arm is about what a cut leaves
    // behind rather than about which phase it landed in.
    let mut cut_landed = 0usize;
    let mut seen_before_repair = 0usize;
    let mut spans: Vec<u64> = Vec::new();
    for (step, percent) in CUT_PERCENTS.into_iter().enumerate().skip(1) {
        let fixture = pipeline(&format!("model_a_r9_staging_{step}"), "before\n");
        fixture.commit_one("warm\n");
        let span_ms = timed_commit_ms(&fixture, &format!("span-{step}\n"));
        spans.push(span_ms);
        let offset = span_ms * percent / 100;
        let tid = fixture.planned_one(&format!("cut-{offset}\n"));
        assert_eq!(run_verify(&fixture, &tid), 0);
        if commit_cut_at(&fixture, &tid, offset).is_none() {
            cut_landed += 1;
        }
        let residue = staging_residue(&fixture);
        if !residue.is_empty() {
            seen_before_repair += 1;
        }
        let (_, report) = repair_report(&fixture, false);
        assert_eq!(
            report["staging_files"].as_array().map(Vec::len),
            Some(residue.len()),
            "the report names exactly what is there (req/236 M-04): {report}"
        );
        let (_, report) = repair_report(&fixture, true);
        assert!(
            staging_residue(&fixture).is_empty(),
            "🔴 and `--yes` removes them: {report}, still on disk: {:?}",
            staging_residue(&fixture)
        );
    }
    println!(
        "R9_STAGING cuts={cut_landed} runs_with_residue={seen_before_repair} spans_ms={spans:?}"
    );
    assert!(
        cut_landed >= 3,
        "the sweep has to actually cut running commits; only {cut_landed} landed. The commits \
         measured beside them ran {spans:?} ms"
    );
}

// ---------------------------------------------------------------------------
// R10 — `req/238` H-01 (`req/38` §177 ruling 2)
// ---------------------------------------------------------------------------

/// `.gx/config.toml`, spelled once for the three probes below.
fn config_path(fixture: &Pipeline) -> PathBuf {
    fixture.project.join(".gx").join("config.toml")
}

/// The `gx_code` on stderr, for a verb that refused.
fn refusal_code(run: &support::Run) -> String {
    let problem: serde_json::Value = serde_json::from_str(run.stderr.trim()).unwrap_or_else(|e| {
        panic!(
            "44 §1.3 asks for a problem object on stderr; got {:?} ({e})",
            run.stderr
        )
    });
    problem["gx_code"]
        .as_str()
        .expect("44 §1.3's gx_code")
        .to_string()
}

/// 🔴 **R10 / `req/238` H-01 (self-adversarial 1 of 3)** — a project that **lost** its declaration
/// is diagnosed, is not written to, and is not quietly put back together by the next writer.
///
/// The finding this is the gate for, in the order the audit walked it: `rm .gx/VERSION` →
/// `gx repair` exit **6** `NOT_FOUND` with **zero** report lines (against `docs/LIMITS.md` v0.4-v's
/// "`gx repair` opens anyway and reports everything else it can see") → `gx submit` exit **0**,
/// which re-created the file through `Layout::create`'s defaults and `declare_journal_format`'s
/// byte reader → `gx repair` exit 0, `ledger_agrees_before` back to `true`, `head_authenticity`
/// back to `"verified"`, `remedy` back to `null`. R7 bound this file's digest under the head's
/// signature so that a **rewritten** declaration is caught; one writer verb took the detector off
/// and said nothing. Deletion was the stronger attack, not the weaker one.
///
/// Four assertions, and each of them is red on the R9 binary: the report opens, the writer refuses
/// with a code of its own, the repair happens **only** when asked for, and the repair says so.
#[test]
fn a_declaration_that_is_gone_is_reported_and_is_not_written_back_in_silence() {
    let fixture = pipeline("model_a_declaration_absent", "before\n");
    fixture.commit_one("one\n");
    let original = std::fs::read(version_path(&fixture)).expect("read VERSION");
    std::fs::remove_file(version_path(&fixture)).expect("remove the declaration");

    // ① The diagnosis opens. Before R10 this was exit 6 and an empty stdout.
    let (code, report) = repair_report(&fixture, false);
    println!("R10_ABSENT report_exit={code} {report}");
    assert_eq!(
        code, 1,
        "🔴 a project whose declaration is gone is not healthy, and it is not a 6 either: a 6 is \
         `NOT_FOUND`, which is 44 §1.4's answer for a directory that has no `.gx/` at all. This \
         one has a journal, a ledger, receipts and a signed head: {report}"
    );
    assert_eq!(
        report["declaration_absent"], true,
        "the report has to name the fact rather than leave it to be inferred: {report}"
    );
    assert_eq!(
        report["ledger_leaves"], 1,
        "🔴 `req/227` M-03: everything else was measured anyway — that is what \"opens anyway\" \
         means, and before R10 nothing was: {report}"
    );
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("gx repair --yes")),
        "a state you can see needs a way out of it (`req/222` H-06), and the way out is named: \
         {report}"
    );
    assert!(
        !version_path(&fixture).exists(),
        "🔴 the *report* mode writes nothing. That is the module header's promise and it is what \
         makes a diagnosis safe to run on evidence"
    );

    // ② The writer refuses instead of re-creating. `gx submit` is the verb that used to do it.
    let submitted = fixture.submit("two\n");
    println!(
        "R10_ABSENT submit_exit={} stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(
        submitted.code, 1,
        "🔴 `req/238` H-01: this exited **0** and wrote the declaration back. A writer that \
         re-creates the file whose digest is bound into the signed head is a writer that disarms \
         the detector: {}",
        submitted.stderr
    );
    assert_eq!(refusal_code(&submitted), "DECLARATION_ABSENT");
    assert!(
        !version_path(&fixture).exists(),
        "🔴 and it wrote nothing on its way out"
    );

    // ③ The one road that writes, writes — and says what it did.
    let (yes_code, yes) = repair_report(&fixture, true);
    println!("R10_ABSENT yes_exit={yes_code} {yes}");
    assert_eq!(
        yes["meta_repaired"][0]["file"], ".gx/VERSION",
        "the report names the file it created: {yes}"
    );
    assert_eq!(yes["meta_repaired"][0]["what"], "created", "{yes}");
    let restored = std::fs::read(version_path(&fixture)).expect("the declaration is back");
    assert_eq!(
        restored, original,
        "🔴 what is written is the project's own facts (the layout version, and the framing \
         sniffed off the journal) — so the head's recorded declaration digest matches again \
         because the file **is** the file it was, and not because a writer painted over it"
    );
    let (after_code, after) = repair_report(&fixture, false);
    assert_eq!(after_code, 0, "and the project is whole again: {after}");
    assert_eq!(after["declaration_absent"], false, "{after}");
    assert_eq!(after["head_authenticity"], "verified", "{after}");
    assert_eq!(fixture.submit("three\n").code, 0, "and writable again");
}

/// 🔴 **R10 / `req/238` H-01 (self-adversarial 2 of 3)** — a project that lost `.gx/config.toml`
/// does not get the shipping default back under it.
///
/// 43 §7.9 (b)'s R9 row calls this file "the one that decides the recovery key". The audit set
/// `engine_signing_keyid`, deleted the file, ran `gx submit` — rc **0** — and found the two shipped
/// comments back in its place, which is the project silently on no key at all; the next
/// `gx repair --yes` then asked for a key the project used to name.
///
/// The write verbs refuse with a code of their own; the **read** verbs and `gx repair`'s report
/// mode do not, because `req/227` M-03's rule is that a reader's door must not be narrower than a
/// writer's, and a diagnosis is the one thing a project in this state has left.
#[test]
fn settings_that_are_gone_do_not_come_back_as_the_shipping_default() {
    let fixture = pipeline("model_a_config_absent", "before\n");
    let tid = fixture.commit_one("one\n");
    let chosen = format!("engine_signing_keyid = \"{}\"\n", fixture.key_id);
    let mut settings = std::fs::read_to_string(config_path(&fixture)).expect("read config.toml");
    settings.push_str(&chosen);
    std::fs::write(config_path(&fixture), &settings).expect("record the recovery key");
    std::fs::remove_file(config_path(&fixture)).expect("lose the settings");

    // The reader's door is unchanged: a diagnosis runs, and names it.
    let (code, report) = repair_report(&fixture, false);
    println!("R10_CONFIG report_exit={code} {report}");
    assert_eq!(report["config_absent"], true, "{report}");
    assert_eq!(code, 1, "and that is not a healthy project: {report}");
    assert!(
        !config_path(&fixture).exists(),
        "🔴 the report mode writes nothing at all"
    );
    let proof = support::run(fixture.gx().args(["log", "proof", "--leaf", "0"]));
    assert_eq!(
        proof.code, 0,
        "🔴 `req/227` M-03 — a **read** does not need the settings file and is not refused for \
         its absence: {}",
        proof.stderr
    );

    // Every writer's door refuses, with the word rather than with a default.
    // Real ids, so that the refusal being measured is the one at the writer's door and not clap's.
    for verb in [vec!["undo", tid.as_str()], vec!["commit", tid.as_str()]] {
        let refused = support::run(fixture.gx().args(&verb));
        println!(
            "R10_CONFIG {verb:?} exit={} stderr={}",
            refused.code,
            refused.stderr.trim()
        );
        assert_eq!(refusal_code(&refused), "CONFIG_ABSENT", "{verb:?}");
    }
    let submitted = fixture.submit("two\n");
    assert_eq!(refusal_code(&submitted), "CONFIG_ABSENT");
    assert!(
        !config_path(&fixture).exists(),
        "🔴 `req/238` H-01: `gx submit` exited 0 here and left the two shipped comments behind, \
         which is `engine_signing_keyid` gone with nobody told"
    );

    // And the road that writes, writes.
    let (_, yes) = repair_report(&fixture, true);
    println!("R10_CONFIG yes={yes}");
    assert!(
        yes["meta_repaired"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|r| r["file"] == ".gx/config.toml")),
        "the repair names the file it made: {yes}"
    );
    assert!(config_path(&fixture).exists());
    assert!(
        !std::fs::read_to_string(config_path(&fixture))
            .expect("read it back")
            .contains("engine_signing_keyid"),
        "🔴 and it does **not** invent the operator's setting: what gx can put back is the file, \
         and the line that names a key is the operator's to write. The remedy says so"
    );
}

/// 🔴 **R10 / `req/238` H-01 (self-adversarial 3 of 3)** — a declaration that is not text keeps its
/// bytes.
///
/// The audit's third arm: `1\njournal_format=chained\n\xff\n` on the disk, `gx submit` rc **0**,
/// and the file afterwards `1\njournal_format=chained\n` — the operator's byte thrown away by
/// `declare_journal_format`'s `read_to_string(..).unwrap_or_else(|_| "1\n")`, which was the one
/// reader of this file in the workspace that did not go through
/// `gx_log::head::declaration_lines`. Whatever that `\xff` was — a half-finished edit, a bad
/// transfer, a file somebody's tool wrote — gx is not the thing that gets to decide it was noise.
///
/// So: the writer refuses and touches nothing, and the repair that does rewrite it keeps the old
/// bytes at `VERSION.pre-repair.<n>` (no-delete, DR-43-7 (1)'s rule one file along).
#[test]
fn a_declaration_that_is_not_text_keeps_its_bytes() {
    let fixture = pipeline("model_a_declaration_bytes", "before\n");
    fixture.commit_one("one\n");
    let mut bytes = std::fs::read(version_path(&fixture)).expect("read VERSION");
    bytes.extend_from_slice(&[0xff, b'\n']);
    std::fs::write(version_path(&fixture), &bytes).expect("write the operator's bytes");

    let submitted = fixture.submit("two\n");
    println!(
        "R10_BYTES submit_exit={} stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(
        refusal_code(&submitted),
        "DECLARATION_UNREADABLE",
        "the writer refuses instead of guessing: {}",
        submitted.stderr
    );
    assert_eq!(
        std::fs::read(version_path(&fixture)).expect("read it back"),
        bytes,
        "🔴 `req/238` H-01: this used to come back as `1\\njournal_format=chained\\n` — the \
         operator's bytes destroyed, at rc 0, in silence"
    );

    let (_, yes) = repair_report(&fixture, true);
    println!("R10_BYTES yes={yes}");
    assert_eq!(yes["meta_repaired"][0]["what"], "rewritten", "{yes}");
    let kept = yes["meta_repaired"][0]["kept"]
        .as_str()
        .expect("the rewrite names where the old bytes went")
        .to_string();
    assert_eq!(
        std::fs::read(&kept).expect("the old bytes are still on the disk"),
        bytes,
        "🔴 no-delete: a repair that destroyed what it repaired leaves an operator unable to tell \
         `gx fixed it` from `gx overwrote the evidence`"
    );
    let (after_code, after) = repair_report(&fixture, false);
    assert_eq!(after_code, 0, "{after}");
    assert_eq!(after["declaration_readable"], true, "{after}");
    assert_eq!(
        after["head_authenticity"], "verified",
        "and the digest the head recorded matches the file again: {after}"
    );
}

// ---------------------------------------------------------------------------
// R11 — `req/240` H-01, H-02, M-02, M-03, M-04 (`req/38` §179 ruling 2)
// ---------------------------------------------------------------------------

/// `gx repair` with `--yes` and **no key anywhere** — the buyer's road, spelled once.
fn repair_yes_without_a_key(fixture: &Pipeline) -> (i32, String, serde_json::Value) {
    let run = support::run(fixture.gx().args(["repair", "--yes"]));
    println!(
        "R11_KEYLESS exit={} stdout_len={} stderr={}",
        run.code,
        run.stdout.trim().len(),
        run.stderr.trim()
    );
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, run.stdout.trim().to_string(), json)
}

/// 🔴 **R11 / `req/240` H-01 (self-adversarial 1 of 5)** — the one road that writes a
/// `Nature::Meta` file writes **after** the lock and the key, or it does not write at all.
///
/// The finding, in the order an operator walks it: a project created by `gx submit` carries the
/// shipping `config.toml`, which records no `engine_signing_keyid`. Delete `.gx/VERSION`; `gx
/// submit` refuses `DECLARATION_ABSENT` and its remedy names `gx repair --yes`; that command
/// reaches `signing()`, exits **1** `VALIDATION_ERROR` with an **empty stdout** — and R10's
/// `repair_declaration`/`repair_config` had already run at the top of the function, so both files
/// were on the disk again. gx wrote, said nothing, and said "refused". 43 §7.12 (a) 4 and
/// `docs/LIMITS.md` v0.4-w both promise the opposite in as many words ("it tells you it did, by
/// name").
///
/// Four assertions, and every one of them is red on the R10 binary (`3feb35e`): the declaration is
/// still gone, the settings file is byte-identical, the run says what it could not do on **stdout**,
/// and the same command with a key does the repair and names it.
#[test]
fn a_repair_that_cannot_sign_writes_nothing_and_says_so() {
    let fixture = pipeline("model_a_r11_keyless_repair", "before\n");
    fixture.commit_one("one\n");
    let config_before = std::fs::read(config_path(&fixture)).expect("read config.toml");
    assert!(
        !String::from_utf8_lossy(&config_before).contains("engine_signing_keyid"),
        "🔴 the fixture has to be the **shipping** project — the whole finding is that the road \
         `DECLARATION_ABSENT` names always reaches the key check on a project gx itself created"
    );
    std::fs::remove_file(version_path(&fixture)).expect("remove the declaration");

    let (code, stdout, report) = repair_yes_without_a_key(&fixture);
    assert_eq!(code, 1, "a repair that could not run is not a 0: {report}");
    assert!(
        !version_path(&fixture).exists(),
        "🔴 `req/240` H-01: `.gx/VERSION` was written back here — by a run that exited 1 with an \
         empty stdout, on a project whose operator was told the repair was refused. A write that \
         happens before the run knows it can report is a write nobody is told about"
    );
    assert_eq!(
        std::fs::read(config_path(&fixture)).expect("read config.toml back"),
        config_before,
        "🔴 and `.gx/config.toml` was rewritten on the same road"
    );
    assert!(
        !stdout.is_empty(),
        "🔴 44 §1.3 puts the answer on stdout, and audit 10 M-03 measured this verb discarding the \
         whole diagnosis on every early refusal: `gx repair --yes` printed nothing at all while \
         `gx repair` on the same project printed forty-seven keys"
    );
    assert_eq!(
        report["ledger_leaves"], 1,
        "the report is measured, not a placeholder: {report}"
    );
    assert_eq!(report["declaration_absent"], true, "{report}");
    assert_eq!(report["meta_repaired"], serde_json::json!([]), "{report}");
    let refused = report["meta_repair_refused"]
        .as_str()
        .unwrap_or_else(|| panic!("the run says why it wrote nothing: {report}"));
    assert!(
        refused.contains("--signing-key") && refused.contains("gx key"),
        "🔴 and it names the way out for the shipping project, which has no recorded key: \
         {refused}"
    );

    // The same command, with a key: the repair happens and is named. R10's promise, now on the
    // road R10 put it on.
    let (yes_code, yes) = repair_report(&fixture, true);
    println!("R11_KEYED exit={yes_code} {yes}");
    assert_eq!(yes["meta_repaired"][0]["file"], ".gx/VERSION", "{yes}");
    assert_eq!(yes["meta_repaired"][0]["what"], "created", "{yes}");
    assert!(version_path(&fixture).exists(), "and the file is back");
    assert_eq!(yes["meta_repair_refused"], serde_json::Value::Null, "{yes}");
}

/// 🔴 **R11 / `req/240` H-01 (self-adversarial 2 of 5)** — a repair that cannot **write** says so
/// too, and the diagnosis survives it.
///
/// The read-only arm, which is `req/227` M-03's rule at its sharpest: a snapshot, a backup or an
/// investigator's copy is the tree somebody looks at *because* something went wrong, and R10
/// answered `gx repair --yes` there with `{"gx_code":"INTERNAL","detail":"write …/.gx/VERSION:
/// Permission denied"}` and no report — 44 §2.3 keeps `INTERNAL` for what cannot be classified,
/// and a read-only directory is entirely classifiable (audit 10 M-02, `req/240` M-06).
#[cfg(unix)]
#[test]
fn a_repair_on_a_read_only_project_reports_instead_of_answering_internal() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = pipeline("model_a_r11_read_only_repair", "before\n");
    fixture.commit_one("one\n");
    // 🔴 **R12 / `req/242` M-06** — measure the filesystem before building a fixture out of
    // it.
    //
    // `req/242` §4 ran this suite with `CARGO_TARGET_TMPDIR` on `/mnt/c` (v9fs) and got 19/20.
    // This was the one red, and the cause was not a regression: on 9p a directory `chmod`ed to
    // `555` reports mode `777` and takes writes, so the read-only project this probe needs cannot
    // be built there and `gx repair --yes` correctly wrote the declaration back. A red that
    // measures the operating system is as useless as a green that measured nothing, so the
    // capability is measured and the skip says why (`chmod_decides_writes`).
    //
    // 🔴 **R13 / `req/244` M-01** — measured on the **project**, which is the directory the arm
    // makes read-only.
    //
    // R12 measured `fixture.home`, and `support::secure_scratch` puts that under
    // `std::env::temp_dir()` — `/tmp`, which is ext4 whatever `CARGO_TARGET_TMPDIR` is. So the
    // guard answered `true` on every filesystem, the skip never fired, and this arm ran on 9p and
    // failed: `req/244` §4 measured 24 passed / 2 failed there, with neither red printing
    // `SKIPPED`, and R12 had wired the same guard into a second arm so the count of reds went from
    // one to two. 43 §7.14 (f) and `docs/LIMITS.md` v0.4-y both said this test "measures the
    // filesystem first and says out loud that it skipped", and it was false of the filesystem that
    // mattered. The subject is `fixture.project/.gx`, so the subject's own filesystem is what
    // decides whether the subject can be built.
    if !chmod_decides_writes(&fixture.project) {
        println!(
            "R11_READONLY SKIPPED: `chmod` does not decide writes under {} (9p/v9fs leaves a 555 \
             directory at 777). The read-only subject of this probe is ext4-only and that is the \
             denominator `docs/LIMITS.md` carries (`req/242` M-06, `req/244` M-01)",
            fixture.project.display()
        );
        return;
    }
    std::fs::remove_file(version_path(&fixture)).expect("remove the declaration");
    let gx_dir = fixture.project.join(".gx");
    let was = std::fs::metadata(&gx_dir).expect("stat .gx").permissions();
    std::fs::set_permissions(&gx_dir, std::fs::Permissions::from_mode(0o555))
        .expect("make .gx read-only");

    let run =
        support::run(
            fixture
                .gx()
                .args(["repair", "--yes", "--signing-key", &fixture.key_id]),
        );
    let report: serde_json::Value =
        serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    println!("R11_READONLY exit={} report={report}", run.code);
    std::fs::set_permissions(&gx_dir, was).expect("restore the permissions");

    assert_eq!(run.code, 1, "still a refusal: {report}");
    assert!(
        !run.stdout.trim().is_empty(),
        "🔴 audit 10 M-03: the whole diagnosis was thrown away here, on the one kind of tree that \
         exists to be diagnosed"
    );
    assert_eq!(
        report["ledger_leaves"], 1,
        "and everything readable was read: {report}"
    );
    let refused = report["meta_repair_refused"]
        .as_str()
        .unwrap_or_else(|| panic!("the run says what it could not do: {report}"));
    assert!(
        refused.contains("Permission denied") && refused.contains("read-only"),
        "🔴 `req/240` M-06: the sentence classifies the fault instead of leaving `INTERNAL` to: \
         {refused}"
    );
}

/// 🔴 **R11 / `req/240` H-02 (self-adversarial 3 of 5)** — a project that lost `.gx/ledger/journal`
/// is not "healthy", and every number in the report is measured.
///
/// R4's early return printed `ledger_agrees_before: true`, `journal_commits: 0`, `ledger_leaves: 0`,
/// `remedy: null` and exit **0** — as **constants**, over a project holding two committed leaves,
/// two commit receipts and a signed head. The next `gx submit` on that project refuses
/// `LEDGER_DISAGREES`. The two arms below are the shape R4's comment was actually right about (a
/// directory nothing has written to, which still exits 0) and the shape it was wrong about.
#[test]
fn a_project_that_lost_its_journal_is_measured_and_not_called_healthy() {
    let fixture = pipeline("model_a_r11_journal_absent", "before\n");
    fixture.commit_one("one\n");
    fixture.commit_one("two\n");
    let healthy = repair_report(&fixture, false).1;
    let healthy_keys: Vec<&String> = healthy
        .as_object()
        .expect("a report object")
        .keys()
        .collect();
    let journal = layout(&fixture).journal_path();
    std::fs::remove_file(&journal).expect("remove the journal");

    let (code, report) = repair_report(&fixture, false);
    println!("R11_JOURNAL_ABSENT exit={code} {report}");
    assert_eq!(
        code, 1,
        "🔴 `req/240` H-02: this exited **0** with `remedy: null` about a project the next writer \
         refuses: {report}"
    );
    assert_eq!(report["journal_absent"], true, "{report}");
    assert_eq!(
        report["ledger_leaves"], 2,
        "🔴 the ledger is on the disk and holds two leaves; the constant said 0: {report}"
    );
    assert_eq!(report["commit_receipts"], 2, "{report}");
    assert_eq!(report["head_recorded"], true, "{report}");
    assert_eq!(
        report["ledger_agrees_before"],
        serde_json::Value::Null,
        "🔴 measured or `null`, never a constant: one of the two files a comparison needs is gone, \
         so `true` was an answer to a question nobody could ask: {report}"
    );
    let keys: Vec<&String> = report
        .as_object()
        .expect("a report object")
        .keys()
        .collect();
    assert_eq!(
        keys, healthy_keys,
        "🔴 the same key set as every other report (the constant printed thirteen of forty-seven, \
         so a monitor reading `head_recorded` got `undefined` rather than an answer)"
    );
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("restore `.gx/ledger/journal`")),
        "a state you can see needs a way out (`req/222` H-06): {report}"
    );
    assert!(
        !journal.exists(),
        "🔴 and the diagnosis did not create the file it is diagnosing"
    );

    // `--yes` on the same project writes nothing either: what it would put in the declaration is
    // the framing sniffed off the journal's first eight bytes, and there is no journal to sniff.
    let before = std::fs::read(version_path(&fixture)).expect("read VERSION");
    let (yes_code, yes) = repair_report(&fixture, true);
    println!("R11_JOURNAL_ABSENT yes={yes_code} {yes}");
    assert_eq!(yes_code, 1, "{yes}");
    assert!(!journal.exists(), "🔴 `--yes` did not create a journal");
    assert_eq!(
        std::fs::read(version_path(&fixture)).expect("read VERSION back"),
        before,
        "and it did not rewrite the declaration on a guess"
    );

    // The arm R4's sentence was right about: a `.gx/` that has a declaration and has never held a
    // commit — no journal, no ledger, no receipts, no head. `Layout::established` says "this is a
    // project"; nothing in it says anything was lost.
    let fresh = pipeline("model_a_r11_journal_never", "before\n");
    let fresh_gx = fresh.project.join(".gx");
    std::fs::create_dir_all(&fresh_gx).expect("make .gx");
    std::fs::write(fresh_gx.join("VERSION"), "1\njournal_format=chained\n")
        .expect("a declaration and nothing else");
    std::fs::write(fresh_gx.join("config.toml"), "# settings\n").expect("settings");
    let (fresh_code, fresh_report) = repair_report(&fresh, false);
    println!("R11_JOURNAL_NEVER exit={fresh_code} {fresh_report}");
    assert_eq!(
        fresh_code, 0,
        "🔴 a project nothing has committed to has lost nothing, and telling that operator their \
         project is damaged would be the mirror of the finding this closes: {fresh_report}"
    );
    assert!(
        fresh_report["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("nothing has been written to")),
        "🔴 **R12 / `req/242` L-04** — this was `remedy: null`, which left a monitor reading          `journal_absent: true` beside exit **0** with nothing between them. The judgement is          unchanged (a `gx key gen` in a fresh directory is not a damaged project); what is new is          that the report says which of the two readings this run made: {fresh_report}"
    );
}

/// 🔴 **R11 / `req/240` M-02 + M-03 (self-adversarial 4 of 5)** — the two files R10 left undeclared.
///
/// M-02: `Layout::create` writes three files and R10's gate covers two of them. `rm
/// .gx/.gitignore` → `gx submit` rc **0**, stderr empty, and req/56 §4's file — the one §4 asks the
/// operator to edit — back in its shipping form with their `!config.toml` line gone.
///
/// M-03: `VERSION.pre-repair.<n>` is a family with no row in `GX_PATHS`, no row in req/56 §2, no
/// row in 43 §7.9 (b), no verb that lists it, and no ceiling short of a thousand — at which point
/// `gx repair --yes` itself becomes a `Usage` refusal. Nothing here deletes one (they are
/// evidence); what changes is that the report names them and that the ceiling is a number an
/// operator can meet.
#[test]
fn the_two_files_a_repair_leaves_beside_the_declaration_are_declared() {
    let fixture = pipeline("model_a_r11_undeclared", "before\n");
    fixture.commit_one("one\n");

    // M-02 — the operator's own edit to req/56 §4's file.
    let ignore = fixture.project.join(".gx").join(".gitignore");
    let mine = "*\n!config.toml\n";
    std::fs::write(&ignore, mine).expect("the operator's edit");
    std::fs::remove_file(&ignore).expect("and then it is gone");
    assert_eq!(
        fixture.submit("two\n").code,
        0,
        "the writer's door does not shut on this file — it decides what git sees and nothing the \
         ledger depends on"
    );
    assert!(
        !ignore.exists(),
        "🔴 `req/240` M-02: `Layout::create` wrote the shipping default back over the operator's \
         edit, at rc 0, in silence"
    );
    let (_, report) = repair_report(&fixture, false);
    assert_eq!(
        report["gitignore_absent"], true,
        "and the absence is a fact the diagnosis carries: {report}"
    );

    // M-03 — three unreadable declarations, three repairs, three copies kept and **named**.
    for round in 0..3 {
        let mut bytes = std::fs::read(version_path(&fixture)).expect("read VERSION");
        bytes.extend_from_slice(&[0xff, b'\n']);
        std::fs::write(version_path(&fixture), &bytes).expect("break the declaration");
        let (_, yes) = repair_report(&fixture, true);
        assert_eq!(
            yes["meta_repaired"][0]["what"], "rewritten",
            "{round}: {yes}"
        );
    }
    let (_, report) = repair_report(&fixture, false);
    let kept = report["kept_aside"]
        .as_array()
        .expect("the report names them")
        .clone();
    println!("R11_KEPT_ASIDE {kept:?}");
    assert_eq!(
        kept.len(),
        3,
        "🔴 `req/240` M-03: three copies were on the disk and the whole report did not contain the \
         substring `pre-repair`: {report}"
    );
    assert_eq!(kept[0], ".gx/VERSION.pre-repair.0", "{report}");

    // The ceiling, met rather than discovered at a thousand. Nothing is removed to make room.
    for _ in 3..gx_cli::layout::PRE_REPAIR_LIMIT {
        let mut bytes = std::fs::read(version_path(&fixture)).expect("read VERSION");
        bytes.extend_from_slice(&[0xff, b'\n']);
        std::fs::write(version_path(&fixture), &bytes).expect("break the declaration");
        let (_, repaired) = repair_report(&fixture, true);
        assert_eq!(
            repaired["meta_repaired"][0]["what"], "rewritten",
            "still repairing: {repaired}"
        );
    }
    let mut bytes = std::fs::read(version_path(&fixture)).expect("read VERSION");
    bytes.extend_from_slice(&[0xff, b'\n']);
    std::fs::write(version_path(&fixture), &bytes).expect("break it once more");
    let run =
        support::run(
            fixture
                .gx()
                .args(["repair", "--yes", "--signing-key", &fixture.key_id]),
        );
    let refusal: serde_json::Value =
        serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    println!("R11_LIMIT exit={} report={refusal}", run.code);
    assert_eq!(run.code, 1, "the ceiling refuses");
    // The refusal arrives as `meta_repair_refused` on **stdout** and not as an empty-stdout
    // `Usage` on stderr, which is the same rule H-01 put on every other road a repair cannot
    // finish: the diagnosis is the deliverable (44 §1.3), and a verb that has just refused is the
    // one an operator most needs it from.
    let why = refusal["meta_repair_refused"]
        .as_str()
        .unwrap_or_else(|| panic!("the ceiling says why: {refusal}"));
    assert!(
        why.contains("pre-repair") && why.contains("VERSION.pre-repair.0"),
        "🔴 and the refusal names the oldest copy rather than removing one (no-delete): {why}"
    );
    assert_eq!(
        refusal["kept_aside"]
            .as_array()
            .expect("the copies are named")
            .len(),
        gx_cli::layout::PRE_REPAIR_LIMIT as usize,
        "and every one of them is still on the disk: {refusal}"
    );
    assert_eq!(
        std::fs::read(version_path(&fixture)).expect("read VERSION"),
        bytes,
        "🔴 a run that refused wrote nothing"
    );
    assert!(
        std::fs::read(fixture.project.join(".gx").join("VERSION.pre-repair.0")).is_ok(),
        "and the oldest copy is still there"
    );
}

/// 🔴 **R11 / `req/240` M-04 (self-adversarial 5 of 5)** — a running server asks about `.gx/VERSION`
/// at every write, not once at start-up.
///
/// Measured on R10: with the server up, `rm .gx/VERSION` changed nothing on the wire —
/// `GET /v1/healthz` answered `{"status":"ok","ledger_agrees":true,"journal_rows":1}` word for
/// word, `POST /v1/candidates` answered **201**, verify **200**, commit **200**, and the ledger
/// grew a leaf — while every CLI verb on the same project was refusing `DECLARATION_ABSENT`. One
/// project, two answers, chosen by which door you came in.
#[test]
fn a_serving_process_notices_a_declaration_that_goes_missing_under_it() {
    let fixture = pipeline("model_a_r11_serve_meta", "before\n");
    fixture.commit_one("one\n");
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    let (health, body) = server.request("GET", "/v1/healthz", None);
    println!("R11_SERVE health_before={health} {body}");
    assert_eq!(health, 200, "{body}");
    let before: serde_json::Value = serde_json::from_str(&body).expect("healthz is JSON");
    assert_eq!(before["status"], "ok", "{body}");

    std::fs::remove_file(version_path(&fixture)).expect("remove the declaration under the server");

    let (created, body) = server.request(
        "POST",
        "/v1/candidates",
        Some(&serde_json::json!({
            "substrate": "fs",
            "locator": fixture.target.display().to_string(),
            "goal": "after\n",
            "context": "Evidence",
            "actor": { "Human": { "key": fixture.key_id } },
        })),
    );
    println!("R11_SERVE create_after={created} {body}");
    assert_eq!(
        created, 500,
        "🔴 `req/240` M-04: this answered **201** and the project grew a leaf under a head whose \
         declaration digest matches nothing: {body}"
    );
    let problem: serde_json::Value = serde_json::from_str(&body).expect("problem+json");
    assert_eq!(problem["gx_code"], "DECLARATION_ABSENT", "{body}");

    let (health, body) = server.request("GET", "/v1/healthz", None);
    println!("R11_SERVE health_after={health} {body}");
    let after: serde_json::Value = serde_json::from_str(&body).expect("healthz is JSON");
    assert_eq!(
        after["status"], "degraded",
        "🔴 a monitor has to be able to see a writer's door that is shut: {body}"
    );
    assert!(
        after["status_reason"]
            .as_str()
            .is_some_and(|why| why.contains("VERSION")),
        "and the reason names the file: {body}"
    );
    assert_eq!(
        health, 200,
        "the server is up and every read still works, so the status stays 200 and the word does \
         the work: {body}"
    );
    shut_down(server);

    // And the same project, from the other door, says the same thing.
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["declaration_absent"], true, "{report}");

    // 🔴 **R12 / `req/242` L-08** — and the **other** word, against a live server.
    //
    // `state.rs`'s `meta_refusal` implements `DECLARATION_ABSENT` and `CONFIG_ABSENT`
    // symmetrically, and 44 v0.4-x declares that both are observable on the HTTP face since R11.
    // Only the first half had a probe: "the code is symmetric" is a reading of the source, and the
    // thing `req/240` M-04 measured was a door that answered differently from the one beside it.
    // The declaration is still gone from the arm above, and `gx serve` refuses to start without
    // one. Putting it back through the one road that may write it is also this arm's own opening
    // assertion: `gx repair --yes` restores the file and names it.
    let (restored_code, restored) = repair_report(&fixture, true);
    println!("R12_CONFIG_SERVE restore_exit={restored_code} {restored}");
    assert_eq!(
        restored["meta_repaired"][0]["file"], ".gx/VERSION",
        "{restored}"
    );
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let settings = config_path(&fixture);
    let kept = std::fs::read(&settings).expect("read config.toml");
    std::fs::remove_file(&settings).expect("lose the settings under the running server");
    let (health, body) = server.request("GET", "/v1/healthz", None);
    let health_json: serde_json::Value =
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    println!("R12_CONFIG_SERVE health={health} {body}");
    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": fixture.target.display().to_string(),
        "goal": "after config went missing\n",
        "context": "Evidence",
        "actor": { "Human": { "key": "probe" } },
    });
    let (status, refusal) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("R12_CONFIG_SERVE post={status} {refusal}");
    let problem: serde_json::Value =
        serde_json::from_str(&refusal).unwrap_or(serde_json::Value::Null);
    assert_eq!(
        problem["gx_code"], "CONFIG_ABSENT",
        "🔴 `req/242` L-08: 44 v0.4-x says this word is observable on the HTTP face. Until R12 \
         only its twin had a probe: {refusal}"
    );
    assert_eq!(status, 500, "{refusal}");
    assert_eq!(
        health, 200,
        "the server is up and every read still works: {body}"
    );
    assert_eq!(
        health_json["status"], "degraded",
        "and a monitor can see the writer's door is shut: {body}"
    );
    // The file is not written back by the server, and the round trip closes.
    assert!(
        !settings.exists(),
        "🔴 a serving process does not put a `Nature::Meta` file back either"
    );
    std::fs::write(&settings, &kept).expect("restore the settings");
    let (health_after, after_body) = server.request("GET", "/v1/healthz", None);
    println!("R12_CONFIG_SERVE health_after={health_after} {after_body}");
    let after_json: serde_json::Value =
        serde_json::from_str(&after_body).unwrap_or(serde_json::Value::Null);
    assert_eq!(after_json["status"], "ok", "{after_body}");
    let (again, _) = server.request("POST", "/v1/candidates", Some(&intent));
    assert_eq!(again, 201, "and the writer's door opens again");
    shut_down(server);
}

/// 🔴 **R12 / `req/242` M-06** — does `chmod` decide anything on the filesystem under this
/// directory.
///
/// `req/242` §4 ran the suite with `CARGO_TARGET_TMPDIR` pointing at `/mnt/c` (v9fs) and got
/// **19/20**: the one red was the read-only repair probe, and the cause was not a regression. On
/// 9p, `chmod 555` on a directory leaves the mode at `777` and a write into it succeeds, so the
/// *fixture* cannot be built there — the probe was asserting a refusal the operating system was
/// never going to produce.
///
/// The answer is neither to weaken the assertion nor to leave a red that means nothing. This
/// measures the filesystem in front of the probe: make a directory, take the write bit off, try to
/// write into it, put the bit back. `false` means "this arm is not measurable here", and the caller
/// prints why and returns — a skip that says its reason, which is what `req/29` §4 asks for
/// ("do not give skip and pass the same face").
/// 🔴 **R13 / `req/244` M-01** — hand it the directory the arm is about.
///
/// The parameter is a path and not a fixture on purpose, and that is exactly how R12 came to pass
/// it the wrong one. `Pipeline::home` is `support::secure_scratch`'s, which is
/// `std::env::temp_dir()` — ext4 on this machine no matter where `CARGO_TARGET_TMPDIR` points —
/// while `Pipeline::project` is `support::scratch`'s, which is `CARGO_TARGET_TMPDIR` and is the
/// filesystem a 9p run is measuring. Both callers hand over `project` now, because `project/.gx` is
/// the directory whose write bit the arms take off.
fn chmod_decides_writes(under: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let probe = under.join("r12_chmod_probe");
    let _ = std::fs::remove_dir_all(&probe);
    if std::fs::create_dir_all(&probe).is_err() {
        return false;
    }
    let was = match std::fs::metadata(&probe) {
        Ok(meta) => meta.permissions(),
        Err(_) => return false,
    };
    if std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o555)).is_err() {
        return false;
    }
    let wrote = std::fs::write(probe.join("canary"), b"x").is_ok();
    let _ = std::fs::set_permissions(&probe, was);
    let _ = std::fs::remove_dir_all(&probe);
    !wrote
}

/// 🔴 **R12 / `req/242` H-01 (a) + (c) + L-03 (self-adversarial 1 of 4)** — a declaration
/// whose version line is not a number keeps its bytes through every writer verb, and
/// `gx repair --yes` says what it did with them.
///
/// # The two predicates, which is what this arm is really about
///
/// `.gx/VERSION` had two readers with two different questions. `Layout::read_declaration` — the one
/// every door refuses on — asks "does the first bare line parse as a number". `declare_journal_format`
/// — the one `session::anchor_accepting` called on the writer's road — asked "did
/// `gx_log::head::declaration_lines` return a non-empty bare line". A file holding `1.0`, or `x`, or
/// three NUL bytes answers **yes** to the second and **no** to the first, so `gx repair`,
/// `gx log proof`, `gx replay` and `gx serve` all said `DECLARATION_UNREADABLE` about a file that
/// `gx submit` quietly rewrote (`req/242` H-01 (a): 3 bytes to 27, three runs, no `.pre-repair`
/// copy, nothing in `meta_repaired`).
///
/// R11 wrote the lesson for this in 43 §7.13 (b) — "a predicate is a predicate only once it is
/// wired into every branch that needs it" — about a different function, and left this one. R12
/// deletes the second reader instead of aligning it: `declare_journal_format` is gone, and
/// `crate::declaration::DeclarationWriter::repair_declaration` decides `Intact` vs `Rewritten` by
/// calling `Layout::read_declaration` itself.
///
/// # And `req/242` L-03, which is the same fix seen from the report
///
/// On exactly these bytes, `gx repair --yes` answered `meta_repaired: []` **and**
/// `meta_repair_refused: null` — the key that exists to say why a `--yes` wrote nothing was `null`
/// about a `--yes` that wrote nothing, because `repair_declaration` read the file with the writer's
/// predicate and called it `Intact`.
#[test]
fn a_declaration_whose_version_is_not_a_number_is_refused_and_repaired_by_name() {
    for (arm, bytes) in [
        // An operator (or a merge tool) putting something that looks like a version on line one.
        ("a version line that is not a number", b"1.0\n".to_vec()),
        // `req/242` H-01 (a)'s second shape, verbatim: 3 bytes that grew to 27.
        ("three NUL bytes", vec![0u8, 0, 0]),
    ] {
        let fixture = pipeline(
            &format!("model_a_r12_bad_version_{}", bytes.len()),
            "before\n",
        );
        fixture.commit_one("one\n");
        std::fs::write(version_path(&fixture), &bytes).expect("write the operator's bytes");

        // ① Every door says the same thing about the file.
        let (code, report) = repair_report(&fixture, false);
        println!("R12_BADVERSION arm={arm} report_exit={code} {report}");
        assert_eq!(code, 1, "[{arm}] {report}");
        assert_eq!(report["declaration_readable"], false, "[{arm}] {report}");
        let proof = support::run(fixture.gx().args(["log", "proof", "--leaf", "0"]));
        assert_eq!(
            refusal_code(&proof),
            "DECLARATION_UNREADABLE",
            "[{arm}] the reader refuses it too"
        );

        // ② The writer refuses **with the same word**, and leaves the bytes alone. This is the
        //    assertion that is red on the R11 binary, where the file grew and the verb then failed
        //    on `LEDGER_DISAGREES` — a sentence about the wrong file entirely.
        let submitted = fixture.submit("two\n");
        println!(
            "R12_BADVERSION arm={arm} submit_exit={} stderr={}",
            submitted.code,
            submitted.stderr.trim()
        );
        assert_eq!(
            refusal_code(&submitted),
            "DECLARATION_UNREADABLE",
            "🔴 `req/242` H-01 (a) [{arm}]: the write gate and the read gate are one \
             predicate (43 §7.14)"
        );
        assert_eq!(
            std::fs::read(version_path(&fixture)).expect("read it back"),
            bytes,
            "🔴 `req/242` H-01 (a) [{arm}]: `gx submit` appended `journal_format=chained` to \
             these bytes, took no `.pre-repair` copy, and said nothing"
        );

        // ③ And the one road that may write it does, keeps the operator's bytes, and names both.
        //    Before R12 this was `meta_repaired: []` with `meta_repair_refused: null`
        //    (`req/242` L-03).
        let (yes_code, yes) = repair_report(&fixture, true);
        println!("R12_BADVERSION arm={arm} yes_exit={yes_code} {yes}");
        assert_eq!(
            yes["meta_repaired"][0]["file"], ".gx/VERSION",
            "🔴 `req/242` L-03 [{arm}]: a `--yes` that wrote nothing said nothing about why: \
             {yes}"
        );
        assert_eq!(
            yes["meta_repaired"][0]["what"], "rewritten",
            "[{arm}] {yes}"
        );
        let kept = yes["meta_repaired"][0]["kept"]
            .as_str()
            .unwrap_or_else(|| panic!("[{arm}] no-delete: the operator's bytes are named: {yes}"));
        assert_eq!(
            std::fs::read(kept).expect("the copy is on the disk"),
            bytes,
            "[{arm}] 🔴 no-delete: what was in the file is beside it, byte for byte"
        );
        let now = std::fs::read_to_string(version_path(&fixture)).expect("read");
        // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the declaration this repair writes
        // back carries the framing it sniffed off the journal, and this fixture's journal is one
        // this build wrote, so that value is `chained-v2`. The claim did **not** change: the file
        // is still compared byte for byte against a pinned expectation, so a repair that invented a
        // framing the project's own journal is not in still turns this red.
        assert_eq!(
            now,
            format!(
                "1\njournal_format={}\n",
                support::CREATED_JOURNAL_FORMAT.kind()
            ),
            "[{arm}] {now:?}"
        );
        let (after_code, after) = repair_report(&fixture, false);
        assert_eq!(after_code, 0, "[{arm}] {after}");
        assert_eq!(after["declaration_readable"], true, "[{arm}] {after}");
    }
}

/// 🔴 **R12 / `req/242` H-01 (b) (self-adversarial 2 of 4)** — a detector that fired stays fired
/// through a writer verb.
///
/// This is the arm that reproduces `req/238` H-01 word for word, one release after it was closed.
/// A healthy two-commit project (`head_authenticity: "verified"`), the `journal_format` line
/// deleted from `.gx/VERSION` by hand, `gx repair` rc **1** with R7's `rolled_back` sentence — and
/// then **one `gx submit`**, rc 0, stderr empty, after which the file was back to its original
/// bytes and `gx repair` answered rc 0, `head_authenticity: "verified"`, `rolled_back: null`,
/// `meta_repaired: []`, `.pre-repair` copies **0**. Three runs, no difference. Neither the fact
/// that the detector fired nor the fact that gx wrote the file was anywhere.
///
/// What is asserted is not "the submit fails". A project whose declaration is *readable* is a
/// project gx will write to, and refusing it would lock out every project written before R6. What
/// is asserted is that the writer **does not touch the file**, so whatever the detector said before
/// the write it still says after it.
#[test]
fn a_writer_verb_does_not_re_arm_a_declaration_digest_that_fired() {
    let fixture = pipeline("model_a_r12_rearm", "before\n");
    fixture.commit_one("one\n");
    fixture.commit_one("two\n");
    let healthy = repair_report(&fixture, false).1;
    assert_eq!(
        healthy["head_authenticity"], "verified",
        "the fixture starts sound: {healthy}"
    );

    // The Model B edit R7's digest exists to catch, in its most ordinary shape: one line removed.
    let text = std::fs::read_to_string(version_path(&fixture)).expect("read VERSION");
    let first = text.lines().next().expect("a version line").to_string();
    let edited = format!("{first}\n");
    std::fs::write(version_path(&fixture), &edited).expect("drop the journal_format line");

    let (fired_code, fired) = repair_report(&fixture, false);
    println!("R12_REARM fired_exit={fired_code} {fired}");
    assert_eq!(fired_code, 1, "the detector fires: {fired}");
    assert!(
        fired["rolled_back"].is_string(),
        "🔴 R7's declaration digest is what this arm is about: {fired}"
    );

    // One writer verb. Before R12 this is the whole attack.
    let submitted = fixture.submit("three\n");
    println!(
        "R12_REARM submit_exit={} stderr={:?}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(
        std::fs::read_to_string(version_path(&fixture)).expect("read"),
        edited,
        "🔴 `req/242` H-01 (b): `gx submit` put the deleted line back and said nothing. A writer \
         that repairs the file whose digest is bound into the signed head is a writer that \
         disarms the detector — `req/238` H-01's sentence, measured again one release later"
    );

    let (after_code, after) = repair_report(&fixture, false);
    println!("R12_REARM after_exit={after_code} {after}");
    assert_eq!(
        after_code, 1,
        "🔴 and the red is still red after the write: {after}"
    );
    assert!(
        after["rolled_back"].is_string(),
        "🔴 `req/242` H-01 (b): this went back to `null` and `head_authenticity: verified`, with \
         no record anywhere that either had ever been otherwise: {after}"
    );
    assert_eq!(
        after["meta_repaired"].as_array().map(Vec::len),
        Some(0),
        "nothing claims to have repaired anything: {after}"
    );
    // 🔴 And the honest other half: an undeclared project is not locked out, it is *undeclared*.
    assert_eq!(
        after["journal_format_declared"],
        serde_json::Value::Null,
        "the report says the framing is no longer declared rather than inventing one: {after}"
    );
}

/// 🔴 **R12 / `req/242` H-01 (d) (self-adversarial 3 of 4)** — a journal that is **gone** is not
/// re-created by the next writer, and the diagnosis about the loss survives.
///
/// R11 closed the report side (`req/240` H-02): rc 1, forty-seven keys, two leaves measured off the
/// ledger, `ledger_agrees_before: null`, a remedy naming the backup, and `--yes` refusing to
/// compose a journal it cannot sniff. `req/242` H-01 (d) measured the answer being **erased**: one
/// `gx submit` created an eight-byte `GXJRNL01` through `EngineJournal::open`'s `create(true)`, and
/// the next `gx repair` reported `journal_absent: false`, `journal_commits: 0`, `ledger_leaves: 2`
/// and a rollback story instead of the loss.
///
/// Two barriers now, and this asserts the outer one. The inner one is
/// `gx_engine::JournalCreation::Refused`, which the CLI puts in every `ProjectAnchor` it builds.
#[test]
fn a_journal_that_is_gone_is_not_created_by_the_next_writer() {
    let fixture = pipeline("model_a_r12_journal_create", "before\n");
    fixture.commit_one("one\n");
    fixture.commit_one("two\n");
    let journal = layout(&fixture).journal_path();
    std::fs::remove_file(&journal).expect("lose the journal");

    let (code, report) = repair_report(&fixture, false);
    println!("R12_JOURNAL report_exit={code} {report}");
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["journal_absent"], true, "{report}");
    assert_eq!(
        report["ledger_leaves"], 2,
        "measured, not assumed: {report}"
    );

    let submitted = fixture.submit("three\n");
    println!(
        "R12_JOURNAL submit_exit={} stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(
        refusal_code(&submitted),
        "JOURNAL_ABSENT",
        "🔴 `req/242` H-01 (d): this exited 0 and made an empty log where a lost one had been"
    );
    assert!(
        !journal.exists(),
        "🔴 `req/242` H-01 (d): eight bytes of `GXJRNL01` appeared here, and with them the report \
         that a journal was lost stopped being true"
    );

    // The diagnosis is unchanged by the attempt, which is the half that matters to an operator
    // holding a backup.
    let (again_code, again) = repair_report(&fixture, false);
    assert_eq!(again_code, 1, "{again}");
    assert_eq!(again["journal_absent"], true, "{again}");
    assert_eq!(again["ledger_leaves"], 2, "{again}");
    assert!(
        again["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("backup")),
        "and the way out is still named: {again}"
    );
}

/// 🔴 **R12 / `req/242` H-02 (self-adversarial 4 of 4)** — a `gx repair --yes` that wrote something
/// prints its report, whatever the engine then does.
///
/// R11 moved the `Nature::Meta` write below `ProcessLock::open` and below the key and wrote
/// 43 §7.13 (a) about it: "a write happens only after this run is certain it can produce a report".
/// The certainty was placed at the lock. The report is composed *after* the engine opens, catches
/// up and recovers, and three `?` sat in between — so on an ordinary project holding a signing key,
/// four different kinds of damage under `.gx/ledger/` gave rc **1** `INTERNAL`, **zero bytes** on
/// stdout, and a twenty-five byte `.gx/VERSION` on the disk (twelve runs, no difference). The
/// operator read "refused" and gx had written.
///
/// The fix is not a fourth move. `repair::repair_and_report` returns `Outcome` and not
/// `Result<Outcome>`, so after the lock and the key are in hand the compiler will not accept a `?`
/// that leaves without printing. Two arms here, both filesystem-independent (no `chmod`, so this
/// probe measures the same thing on 9p as on ext4).
#[test]
fn a_repair_that_wrote_something_reports_even_when_the_engine_will_not_open() {
    for (arm, damage) in [("ledger-is-a-directory", 0u8), ("blobs-is-a-file", 1u8)] {
        let fixture = pipeline(
            &format!("model_a_r12_report_before_open_{damage}"),
            "before\n",
        );
        fixture.commit_one("one\n");
        // The ordinary project of an operator who has run `gx serve`: the settings name the key.
        let chosen = format!("engine_signing_keyid = \"{}\"\n", fixture.key_id);
        let mut settings = std::fs::read_to_string(config_path(&fixture)).expect("read config");
        settings.push_str(&chosen);
        std::fs::write(config_path(&fixture), &settings).expect("record the recovery key");
        std::fs::remove_file(version_path(&fixture)).expect("lose the declaration");

        // Spelled off the project rather than through `Layout::open`: this fixture has no
        // declaration, and since R12 the reader's door refuses one that is not there — which is
        // the refusal the arm is here to measure the *report* surviving.
        let journal = fixture.project.join(".gx").join("ledger").join("journal");
        let ledger = journal.with_extension("ledger");
        let blobs = journal.with_extension("blobs");
        if damage == 0 {
            std::fs::remove_file(&ledger).expect("remove the ledger file");
            std::fs::create_dir_all(&ledger).expect("a directory where a file belongs");
        } else {
            std::fs::remove_dir_all(&blobs).expect("remove the blob store");
            std::fs::write(&blobs, b"a backup restored a file over a directory")
                .expect("a file where a directory belongs");
        }

        let run = support::run(fixture.gx().args(["repair", "--yes"]));
        println!(
            "R12_REPORT_BEFORE_OPEN arm={arm} exit={} stdout_len={} stderr={}",
            run.code,
            run.stdout.trim().len(),
            run.stderr.trim()
        );
        assert!(
            !run.stdout.trim().is_empty(),
            "🔴 `req/242` H-02 [{arm}]: **zero bytes** here, on a run that had already written \
             `.gx/VERSION`. stderr was `{}`",
            run.stderr.trim()
        );
        let report: serde_json::Value = serde_json::from_str(run.stdout.trim())
            .unwrap_or_else(|e| panic!("[{arm}] 44 §1.3's single JSON object on stdout: {e}"));
        assert_eq!(run.code, 1, "[{arm}] still a refusal: {report}");
        assert_eq!(
            report["engine_open_failed"]["stage"], "open",
            "[{arm}] the report says which of the three steps refused: {report}"
        );
        assert!(
            report["engine_open_failed"]["reason"].is_string(),
            "[{arm}] and what it said: {report}"
        );
        // 🔴 The other half of `req/242` H-02: whatever was written is **named**.
        assert_eq!(
            report["meta_repaired"][0]["file"], ".gx/VERSION",
            "[{arm}] gx wrote this file on the way in, and the run that wrote it says so: {report}"
        );
        assert!(
            version_path(&fixture).exists(),
            "[{arm}] the fixture is the one the audit measured: the file **is** written"
        );
        // And the facts that do not need an engine are still measured.
        assert_eq!(
            report["commit_receipts"], 1,
            "[{arm}] the receipts are read off their own directory: {report}"
        );
        assert!(
            report["remedy"]
                .as_str()
                .is_some_and(|why| why.contains("engine refused at")),
            "[{arm}] and the operator is told what to fix: {report}"
        );
    }
}

/// 🔴 **R12 / `req/242` M-03** — `gx repair --yes` on a tree with no `.gx/LOCK` that it cannot
/// create reports instead of answering `INTERNAL`.
///
/// R11's M-06 repair widened the door for a lock that is **held** and for a read-only `.gx/` that
/// already has the file. `req/242` M-03 measured the shape it left: `.gx/LOCK` is
/// `Nature::Transient`, `GX_PATHS` declares that gx does not create it, and it is therefore absent
/// from every backup, every `git archive`, every `rsync --exclude '*LOCK*'` and every project no
/// writer has run in. On such a tree, made read-only, `gx repair --yes` answered `INTERNAL` "cannot
/// open the writer lock … Permission denied" with **zero** bytes on stdout while `gx repair` on the
/// same tree printed 3,825 of report.
///
/// 🔴 **`req/242` M-06** — and the arm is skipped, with its reason printed, on a filesystem where
/// `chmod` decides nothing (9p: `chmod 555` leaves the mode at `777`). A green that was never
/// measured and a red that measures the operating system are the same lie in two directions.
#[test]
fn a_repair_that_cannot_make_the_lock_reports_instead_of_answering_internal() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = pipeline("model_a_r12_lockless_read_only", "before\n");
    fixture.commit_one("one\n");
    // 🔴 **R13 / `req/244` M-01** — the project, not the home. See the sibling arm above: `home`
    // is `std::env::temp_dir()`'s and is ext4 whatever `CARGO_TARGET_TMPDIR` points at, so this
    // guard could not fire on the filesystem it was written for.
    if !chmod_decides_writes(&fixture.project) {
        println!(
            "R12_LOCKLESS SKIPPED: `chmod` does not decide writes on the filesystem under {} \
             (9p/v9fs leaves a 555 directory at 777), so the read-only fixture this arm needs \
             cannot be built here. Measured, not assumed — see `chmod_decides_writes` \
             (`req/242` M-06, `req/244` M-01)",
            fixture.project.display()
        );
        return;
    }
    std::fs::remove_file(version_path(&fixture)).expect("remove the declaration");
    let gx_dir = fixture.project.join(".gx");
    let lock = gx_dir.join("LOCK");
    let _ = std::fs::remove_file(&lock);
    assert!(!lock.exists(), "the shape a backup hands an investigator");
    let was = std::fs::metadata(&gx_dir).expect("stat .gx").permissions();
    std::fs::set_permissions(&gx_dir, std::fs::Permissions::from_mode(0o555))
        .expect("make .gx read-only");

    let run =
        support::run(
            fixture
                .gx()
                .args(["repair", "--yes", "--signing-key", &fixture.key_id]),
        );
    let stdout = run.stdout.trim().to_string();
    println!(
        "R12_LOCKLESS exit={} stdout_len={} stderr={}",
        run.code,
        stdout.len(),
        run.stderr.trim()
    );
    std::fs::set_permissions(&gx_dir, was).expect("restore the permissions");

    assert!(
        !stdout.is_empty(),
        "🔴 `req/242` M-03: `INTERNAL` and an empty stdout, on the one kind of tree this verb \
         exists to be run on. stderr was `{}`",
        run.stderr.trim()
    );
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("44 §1.3's single JSON object");
    assert_eq!(run.code, 1, "{report}");
    assert_eq!(report["lock_held"], false, "{report}");
    assert_eq!(report["ledger_leaves"], 1, "measured anyway: {report}");
    assert!(
        report["meta_repair_refused"]
            .as_str()
            .is_some_and(|why| why.contains("writer lock")),
        "and the run says what it could not do and why: {report}"
    );
    assert!(
        !version_path(&fixture).exists(),
        "🔴 a run that could not take the lock writes nothing at all"
    );
}

/// 🔴 **R12 (self-kill, this lane)** — removing the stamping road does not lock a legacy project
/// out, and the writer's door and the diagnosis agree about a directory that has never held a
/// commit.
///
/// # The two questions this answers
///
/// R12 deleted `Layout::declare_journal_format` (`req/242` H-01) and made `.gx/ledger/journal`
/// creatable on one road only (`req/242` H-01 (d)). Both are removals, and a removal's risk is
/// **who used to come through here**. The brief's own self-kill asks it twice: does gate ② still
/// pass, and what does `gx serve`'s first start-up do. This measures gate ②'s four shapes; the
/// second question is answered by `Layout::open`, which `gx serve` has always used and which has
/// always required a project.
///
/// # The fourth arm is a defect this lane found in itself
///
/// Arm (d) — a `.gx/` holding `VERSION` and `config.toml` and no `ledger/`, `checkpoints/` or
/// `receipts/` — was **red on this lane's own first binary**: `gx submit` answered
/// `JOURNAL_ABSENT` while `gx repair` answered exit **0**, "nothing has been written to"
/// (`req/242` L-04's own arm). Two doors, two answers, on one project — the failure the last three
/// audits have each found once, reintroduced by the repair for the third of them. `Layout::logged`
/// is the narrower predicate that fixes it: `Layout::established` counts `VERSION` among its
/// witnesses and therefore cannot tell "lost its log" from "never had one".
#[test]
fn a_legacy_project_and_a_half_made_one_are_not_locked_out() {
    let fixture = pipeline("model_a_r12_gate_two", "before\n");
    let key = fixture.key_id.clone();
    let home = fixture.home.clone();
    let root = fixture
        .project
        .parent()
        .expect("a scratch root")
        .to_path_buf();

    // A `gx submit` against an arbitrary directory, with this fixture's key store.
    let submit_into = |project: &Path| -> support::Run {
        let goal = project.join("intent.txt");
        std::fs::write(&goal, "a goal\n").expect("write the intent");
        let target = project.join("target.txt");
        std::fs::write(&target, "hello\n").expect("write the target");
        let mut cmd = support::gx();
        cmd.env("HOME", &home)
            .env("USERPROFILE", &home)
            .arg("--project")
            .arg(project)
            .arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&target)
            .arg("--intent")
            .arg(&goal)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &key]);
        support::run(&mut cmd)
    };

    // (a) A project written before this release ever existed: the declaration carries the layout
    //     version and **nothing else**, and there is a journal.
    let legacy = root.join("r12_gate2_legacy");
    let _ = std::fs::remove_dir_all(&legacy);
    std::fs::create_dir_all(legacy.join(".gx").join("ledger")).expect("make a legacy .gx/");
    std::fs::write(legacy.join(".gx").join("VERSION"), "1\n").expect("a bare declaration");
    std::fs::write(legacy.join(".gx").join("config.toml"), "# settings\n").expect("settings");
    std::fs::write(legacy.join(".gx").join("ledger").join("journal"), b"").expect("an empty log");
    let run = submit_into(&legacy);
    println!(
        "R12_GATE2 legacy exit={} stderr={}",
        run.code,
        run.stderr.trim()
    );
    assert_eq!(
        run.code, 0,
        "🔴 a project that never declared a framing is not locked out: {}",
        run.stderr
    );
    assert_eq!(
        std::fs::read_to_string(legacy.join(".gx").join("VERSION")).expect("read"),
        "1\n",
        "🔴 **and nothing stamped it.** Before R12 this verb appended `journal_format=chained` \
         here, which is the road `req/242` H-01 measured rewriting operator bytes and re-arming a \
         detector that had fired. The declaration is the operator's; an undeclared project stays \
         undeclared and `gx repair` says so"
    );

    // (b) An empty `.gx/` an operator made by hand.
    let empty = root.join("r12_gate2_empty");
    let _ = std::fs::remove_dir_all(&empty);
    std::fs::create_dir_all(empty.join(".gx")).expect("make an empty .gx/");
    let run = submit_into(&empty);
    println!(
        "R12_GATE2 empty exit={} stderr={}",
        run.code,
        run.stderr.trim()
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — a project this build creates declares
    // `chained-v2` and its journal carries `GXJRNL02`. Both expectations are derived from the one
    // pinned format so that they cannot drift apart: the declaration and the marker being minted by
    // the same release is the property this pair of assertions is for, and it is unchanged.
    assert_eq!(
        std::fs::read_to_string(empty.join(".gx").join("VERSION")).expect("read"),
        format!(
            "1\njournal_format={}\n",
            support::CREATED_JOURNAL_FORMAT.kind()
        ),
        "🔴 a project this binary creates says what it is from its first byte"
    );
    assert_eq!(
        std::fs::read(empty.join(".gx").join("ledger").join("journal"))
            .expect("read")
            .get(..8)
            .map(<[u8]>::to_vec),
        support::CREATED_JOURNAL_FORMAT.marker().map(|m| m.to_vec()),
        "🔴 and the journal it declares a framing about is one it made, in that framing"
    );

    // (c) No `.gx/` at all — 44 has no `gx init`, so this is it.
    let fresh = root.join("r12_gate2_fresh");
    let _ = std::fs::remove_dir_all(&fresh);
    std::fs::create_dir_all(&fresh).expect("make a directory");
    let run = submit_into(&fresh);
    println!(
        "R12_GATE2 fresh exit={} stderr={}",
        run.code,
        run.stderr.trim()
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — same one-line change as arm (b) above,
    // for the same reason: the value is `chained-v2` now and nothing else about the claim moved.
    assert_eq!(
        std::fs::read_to_string(fresh.join(".gx").join("VERSION")).expect("read"),
        format!(
            "1\njournal_format={}\n",
            support::CREATED_JOURNAL_FORMAT.kind()
        )
    );

    // (d) A declaration, settings, and no evidence that a commit ever happened.
    let half = root.join("r12_gate2_half_made");
    let _ = std::fs::remove_dir_all(&half);
    std::fs::create_dir_all(half.join(".gx")).expect("make .gx/");
    std::fs::write(
        half.join(".gx").join("VERSION"),
        "1\njournal_format=chained\n",
    )
    .expect("write");
    std::fs::write(half.join(".gx").join("config.toml"), "# settings\n").expect("write");
    let (repair_code, repair) = {
        let mut cmd = support::gx();
        cmd.env("HOME", &home)
            .env("USERPROFILE", &home)
            .arg("--project")
            .arg(&half)
            .arg("repair");
        let run = support::run(&mut cmd);
        let json: serde_json::Value =
            serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
        (run.code, json)
    };
    println!("R12_GATE2 half_made repair_exit={repair_code} {repair}");
    assert_eq!(
        repair_code, 0,
        "the diagnosis reads this as a project nothing has been written to: {repair}"
    );
    let run = submit_into(&half);
    println!(
        "R12_GATE2 half_made exit={} stderr={}",
        run.code,
        run.stderr.trim()
    );
    assert_eq!(
        run.code, 0,
        "🔴 **red on this lane's own first binary**: `gx submit` answered `JOURNAL_ABSENT` while \
         `gx repair` answered exit 0 about the same directory. One project, two doors, two \
         answers — the shape the last three audits each found once. `Layout::logged` is the \
         narrower predicate: {}",
        run.stderr
    );
    assert_eq!(
        std::fs::read(half.join(".gx").join("ledger").join("journal"))
            .expect("read")
            .get(..8)
            .map(<[u8]>::to_vec),
        Some(gx_engine::JOURNAL_MAGIC.to_vec())
    );
}

// ---------------------------------------------------------------------------
// 🔴 R14 — the fourteenth audit's findings, as gates (`req/246`, `req/38` §186 ruling 2)
// ---------------------------------------------------------------------------

/// A destination that accepts every byte and has no room for any of them.
///
/// `/dev/full` is the arm `req/246` H-01 measured on, and it is the one a buyer meets as a disk
/// that filled up while a CI job was running. `None` on a system that has no such device, which is
/// the same shape `chmod_decides_writes` uses: a probe that cannot measure its subject says so
/// rather than passing quietly.
fn dev_full() -> Option<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .ok()
}

/// 44 §1.4's table, as a set. Membership is the whole assertion of the H-01 gate.
fn is_in_the_exit_table(code: i32) -> bool {
    (0..=7).contains(&code)
}

/// The record `gx repair --yes` files, as a path.
fn repair_record_path(fixture: &Pipeline) -> PathBuf {
    fixture.project.join(".gx").join("repair").join("last.json")
}

/// 🔴 **R14 / `req/246` H-01** — a stderr that will not take the refusal does not turn the run into
/// a panic.
///
/// # What the audit measured
///
/// R13 closed stdout: `Outcome::emit` returns a value and a failed delivery is `OUTPUT_FAILED` at
/// 44 §1.4's 1. The object that says so was printed by `eprintln!`, and Rust's `eprint!` family
/// returns nothing exactly as `print!` does — so it **panics** on a write error. The same macro
/// carried every verb's every refusal, which is what 44 §1.3 puts on that stream. Five arms, three
/// runs each, exit **101** with a Rust panic string in all fifteen.
///
/// The cheapest arm is the first one below: a **read** verb, healthy stdout, nothing written into
/// `.gx/` — and the one closest to a buyer is a full disk under `gx repair --yes`, or the `2>&1`
/// people write every day.
///
/// # What is asserted, and what is deliberately not
///
/// The assertion is **membership of 44 §1.4's table**, not a particular number, because the ruling
/// is that a run keeps the status it had already determined: `2>/dev/null` and `2>/dev/full` are two
/// ways of throwing stderr away and a script has to get the same answer from both. So each arm is
/// measured **twice** — against a dead stderr and against `/dev/null` — and the two statuses have to
/// agree. That is a stronger claim than "not 101", and it is the one 43 §7.16 (a) makes.
#[test]
fn a_refusal_whose_stderr_will_not_take_it_still_ends_inside_the_exit_table() {
    if dev_full().is_none() {
        println!("SKIPPED: this system has no /dev/full, so the H-01 arms cannot be measured");
        return;
    }
    let reader = pipeline("model_a_r14_stderr_read", "before\n");
    reader.commit_one("a change");

    // The arm that needs `.gx/VERSION` gone and a key in hand: `--yes` writes, and both streams die.
    let broken = pipeline("model_a_r14_stderr_broken", "before\n");
    broken.commit_one("a change");
    std::fs::remove_file(version_path(&broken)).expect("remove the declaration");

    // The writer's refusal: committed, settings deleted, so `gx submit` refuses `CONFIG_ABSENT`.
    let refused = pipeline("model_a_r14_stderr_config", "before\n");
    refused.commit_one("a change");
    std::fs::remove_file(config_path(&refused)).expect("remove the settings");
    let goal = refused.project.join("goal-r14.txt");
    std::fs::write(&goal, "a goal\n").expect("write the goal");

    struct Arm {
        name: &'static str,
        build: Box<dyn Fn() -> Command>,
        /// Whether stdout is killed as well as stderr — `req/246` H-01's arms (c) and (e).
        kill_stdout: bool,
    }
    let arms: Vec<Arm> = vec![
        Arm {
            name: "read verb (`gx receipt show <missing>`)",
            build: {
                let project = reader.project.clone();
                let home = reader.home.clone();
                Box::new(move || {
                    let mut c = support::gx();
                    c.env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .arg("--project")
                        .arg(&project)
                        .args(["receipt", "show", "gx1:doesnotexist"]);
                    c
                })
            },
            kill_stdout: false,
        },
        Arm {
            name: "writer refusal (`gx submit` on CONFIG_ABSENT)",
            build: {
                let project = refused.project.clone();
                let home = refused.home.clone();
                let key = refused.key_id.clone();
                let target = refused.target.clone();
                Box::new(move || {
                    let mut c = support::gx();
                    c.env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .arg("--project")
                        .arg(&project)
                        .arg("submit")
                        .args(["--substrate", "fs"])
                        .arg("--locator")
                        .arg(&target)
                        .arg("--intent")
                        .arg(&goal)
                        .args(["--context", "Evidence"])
                        .args(["--actor-key", &key]);
                    c
                })
            },
            kill_stdout: false,
        },
        Arm {
            name: "`gx repair --yes` with both streams dead",
            build: {
                let project = broken.project.clone();
                let home = broken.home.clone();
                let key = broken.key_id.clone();
                Box::new(move || {
                    let mut c = support::gx();
                    c.env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .arg("--project")
                        .arg(&project)
                        .arg("repair")
                        .args(["--signing-key", &key])
                        .arg("--yes");
                    c
                })
            },
            kill_stdout: true,
        },
        Arm {
            name: "plain lines (`gx limits`) with both streams dead",
            build: Box::new(|| {
                let mut c = support::gx();
                c.arg("limits");
                c
            }),
            kill_stdout: true,
        },
        Arm {
            name: "clap's own usage error (the control)",
            build: Box::new(|| {
                let mut c = support::gx();
                c.args(["repair", "--no-such-flag"]);
                c
            }),
            kill_stdout: false,
        },
    ];

    for arm in &arms {
        let mut statuses = Vec::new();
        for dead in [true, false] {
            let mut command = (arm.build)();
            let stderr = if dead {
                Stdio::from(dev_full().expect("/dev/full opened once already"))
            } else {
                Stdio::null()
            };
            // 🔴 stdout's destination does **not** vary between the two halves, and the reason is
            // the claim being made. The comparison below is "the same run, with stderr thrown away
            // two different ways, answers the same"; letting stdout be healthy in one half and full
            // in the other would compare two different runs — one that delivered its answer and one
            // that lost it, which is exactly the difference `OUTPUT_FAILED` exists to report.
            if arm.kill_stdout {
                command.stdout(Stdio::from(
                    dev_full().expect("/dev/full opened once already"),
                ));
            } else {
                command.stdout(Stdio::null());
            }
            command.stderr(stderr);
            let status = command.status().expect("the gx binary runs");
            let code = status.code().unwrap_or(-1);
            println!(
                "R14_H01 arm={:?} stderr={} exit={code}",
                arm.name,
                if dead { "/dev/full" } else { "/dev/null" },
            );
            assert!(
                is_in_the_exit_table(code),
                "🔴 `req/246` H-01: `{}` left 44 §1.4's table with {code}. `eprint!` panics on a \
                 write error exactly as `print!` does, and 44 §1.3 puts every refusal on stderr — \
                 so the whole vocabulary of this binary's statuses went out of the table on a full \
                 disk. Exit 101 is what the audit measured, fifteen runs out of fifteen",
                arm.name
            );
            statuses.push(code);
        }
        assert_eq!(
            statuses[0], statuses[1],
            "🔴 43 §7.16 (a) clause 3: `2>/dev/full` and `2>/dev/null` are two ways of throwing \
             stderr away, and `{}` answered them differently ({statuses:?}). A run keeps the status \
             it had already determined; `OUTPUT_FAILED` is for a lost **answer** on stdout and does \
             not spread to a refusal the number already names",
            arm.name
        );
    }
}

/// 🔴 **R14 / `req/246` M-01** — the road that lost its journal files a record of what it wrote.
///
/// R13 built this road for `req/244` H-02: a project that lost `.gx/config.toml` and
/// `.gx/ledger/journal` together used to have no exit from gx, and `gx repair --yes` now writes the
/// settings back there. What it did not do was file `.gx/repair/last.json`, so the next
/// `gx repair` answered `previous_repair: null` about a run that had put 139 bytes on the disk —
/// and the `OUTPUT_FAILED` object told that same run, without a condition, to go and read the file
/// nobody had made (`req/227` M-04: a remedy that names the wrong file is worse than none).
#[test]
fn a_repair_that_wrote_settings_without_a_journal_leaves_the_record_behind() {
    let fixture = pipeline("model_a_r14_m01", "before\n");
    fixture.commit_one("a change");
    let journal = layout(&fixture).journal_path();
    std::fs::remove_file(config_path(&fixture)).expect("remove the settings");
    std::fs::remove_file(&journal).expect("remove the journal");

    let (code, report) = repair_report(&fixture, true);
    println!(
        "R14_M01 yes_exit={code} meta_repaired={} repair_record={}",
        report["meta_repaired"], report["repair_record"]
    );
    assert_eq!(
        code, 1,
        "the journal is still gone, so the project still cannot be written to"
    );
    assert!(
        report["meta_repaired"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["file"] == ".gx/config.toml")),
        "🔴 R13's own road: `--yes` writes the settings back here (`req/244` H-02)"
    );
    assert_eq!(
        report["repair_record"]["written"],
        serde_json::Value::Bool(true),
        "🔴 `req/246` M-01: this run put bytes in the project, so the record of it is a record like \
         any other. It used to be `null` on this road, and the next `gx repair` said the run had \
         never happened"
    );
    assert!(
        repair_record_path(&fixture).is_file(),
        "🔴 `req/246` M-01: the file the report names is on the disk"
    );

    let (_, next) = repair_report(&fixture, false);
    assert!(
        !next["previous_repair"].is_null(),
        "🔴 `req/246` M-01: the next `gx repair` reads it back — which is the whole sentence \
         `OUTPUT_FAILED` puts in front of a buyer"
    );
    assert!(
        next["previous_repair"]["meta_repaired"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["file"] == ".gx/config.toml")),
        "🔴 and what it reads back is what that run did"
    );
}

/// 🔴 **R14 / `req/246` M-02** — one question, one predicate, whichever door is asking.
///
/// R13 gave the writer's door `HISTORY_LOST`: a project with entries under `.gx/index/`,
/// `.gx/evidence/` or `.gx/drafts/` and none of `Layout::logged`'s three witnesses is one whose log
/// has gone, and `gx submit` refuses rather than starting a second history. The **reporting** door
/// kept asking the narrower question and answered the same project with exit **0** and "this is
/// what `.gx/` looks like after `gx key gen` in a fresh directory" — three runs, no variation,
/// while `gx submit` was refusing it by name. 43 §7.15 (b)'s rule generalises: the predicate that
/// refuses and the predicate that decides the exit are one predicate.
#[test]
fn a_project_whose_history_is_gone_is_answered_the_same_way_by_both_doors() {
    let fixture = pipeline("model_a_r14_m02", "before\n");
    fixture.commit_one("a change");
    fixture.commit_one("a second change");
    let root = fixture.project.join(".gx");
    for gone in ["ledger", "checkpoints", "receipts"] {
        std::fs::remove_dir_all(root.join(gone)).expect("remove a witness");
    }

    let submitted = fixture.submit("one more");
    println!(
        "R14_M02 submit_exit={} stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(submitted.code, 1, "R13's refusal is unchanged");
    assert_eq!(
        refusal_code(&submitted),
        "HISTORY_LOST",
        "and it is still the same word"
    );

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        code, 1,
        "🔴 `req/246` M-02: 44 §1.2 gives this verb's 0 to \"this project can be written to\", and \
         no verb can write to this one. It answered 0 for a whole release"
    );
    let remedy = report["remedy"].as_str().unwrap_or_default().to_string();
    assert!(
        remedy.contains("HISTORY_LOST"),
        "🔴 the remedy names what the writer's door says, rather than describing a fresh \
         directory: {remedy}"
    );
    assert!(
        !remedy.contains("what `.gx/` looks like after `gx key gen` in a fresh directory"),
        "🔴 `req/246` M-02: that sentence is the one the same release's writer contradicts: {remedy}"
    );
    let (yes_code, _) = repair_report(&fixture, true);
    assert_eq!(
        yes_code, 1,
        "🔴 and there is still no `--yes` road out of a lost history — R13's judgement is \
         unchanged; what moved is the number and the sentence"
    );
}

/// 🔴 **R14 / `req/246` M-03** — the durable record holds no generations, so it survives run 127.
///
/// R13 filed "the same object it printed", and that object carries `previous_repair`: the whole of
/// the report the run before it filed. The file therefore held generation n − 1, which held n − 2.
/// On one healthy project with nothing to repair and no adversary: 1 run 1,718 B, 40 runs
/// 177,764 B, 126 runs **1,318,468 B** — and at **127** `serde_json`'s recursion limit refused the
/// read, `previous_repair` became `None`, and the file was rewritten from an empty history. "No
/// repair has run here" and "126 of them have" became the same answer. The printed report grew with
/// it, past the 64 KiB a pipe holds, which is `req/246` M-03's second half.
///
/// The sweep is **130** runs, so that 127 is inside it rather than beside it.
#[test]
fn a_repair_record_does_not_grow_a_generation_each_run() {
    let fixture = pipeline("model_a_r14_m03", "before\n");
    fixture.commit_one("a change");
    let record = repair_record_path(&fixture);

    let mut largest_report = 0usize;
    let mut largest_record = 0u64;
    for n in 1..=130u32 {
        let run = support::run(
            fixture
                .gx()
                .arg("repair")
                .args(["--signing-key", &fixture.key_id])
                .arg("--yes"),
        );
        largest_report = largest_report.max(run.stdout.len());
        let size = std::fs::metadata(&record).map(|m| m.len()).unwrap_or(0);
        largest_record = largest_record.max(size);
        if matches!(n, 1 | 2 | 40 | 126 | 127 | 130) {
            println!(
                "R14_M03 n={n} record_bytes={size} report_bytes={}",
                run.stdout.len()
            );
        }
        assert!(
            size > 0,
            "🔴 `req/246` M-03: the record vanished at run {n} — which is what a chain of \
             generations does at 127, when the parser refuses the depth and the writer starts \
             again from nothing"
        );
        let report: serde_json::Value = serde_json::from_str(run.stdout.trim())
            .unwrap_or_else(|e| panic!("44 §1.3's single object at run {n}: {e}"));
        assert!(
            n == 1 || !report["previous_repair"].is_null(),
            "🔴 `req/246` M-03: run {n} could not read the record the run before it filed, so \
             \"no repair has run here\" and \"{} of them have\" are the same answer",
            n - 1
        );
    }
    println!("R14_M03 largest_record={largest_record} largest_report={largest_report}");
    assert!(
        largest_record < 64 * 1024,
        "🔴 `req/246` M-03: a durable record that grows without bound is one that stops being \
         readable; 130 runs reached {largest_record} bytes"
    );
    assert!(
        largest_report < 64 * 1024,
        "🔴 `req/246` M-03's second half: a report past a pipe's 64 KiB is a report `| head` cuts, \
         and 44 §1.3 says stdout emits no partial result; 130 runs reached {largest_report} bytes"
    );
}

/// 🔴 **R14 / `req/246` M-04** — a declared directory blocked by something that is not one has a
/// way out, and the refusal is classified.
///
/// One byte at `.gx/repair` — the row R13 added — locked the project out of `gx submit`,
/// `gx log head` and `gx receipt list` with `INTERNAL` "create …/.gx/repair: File exists (os error
/// 17)", three runs each, for ever; and `gx repair` answered exit **0**, `ledger_agrees_after:
/// true`, `remedy: null`, with `repair_record.written: false` as the only trace — a key that moves
/// neither the status nor the remedy.
#[test]
fn a_declared_directory_blocked_by_a_file_is_named_and_has_an_exit() {
    let fixture = pipeline("model_a_r14_m04", "before\n");
    fixture.commit_one("a change");
    let blocked = fixture.project.join(".gx").join("repair");
    std::fs::remove_dir_all(&blocked).expect("remove the directory");
    std::fs::write(&blocked, b"x").expect("put one byte where a directory belongs");

    let submitted = fixture.submit("one more");
    println!(
        "R14_M04 submit_exit={} stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(
        submitted.code, 1,
        "the writer's door is shut, which is right"
    );
    assert_eq!(
        refusal_code(&submitted),
        "LAYOUT_BLOCKED",
        "🔴 `req/246` M-04: 44 §2.3 keeps `INTERNAL` for what cannot be classified, and the \
         operating system had classified this completely: {}",
        submitted.stderr
    );

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        code, 1,
        "🔴 `req/246` M-04: the diagnosis called this project healthy while every writer was \
         refused"
    );
    // 🔴 **R15 / `req/259` M-01** — the key is a **list** now, one row per blocked declared
    // directory, because a project can have more than one of them occupied at once.
    assert_eq!(report["repair_dir_blocked"][0]["path"], ".gx/repair");
    assert_eq!(
        report["repair_dir_blocked"][0]["cleared"],
        serde_json::Value::Bool(false)
    );
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|text| text.contains(".gx/repair")),
        "🔴 the remedy names the path: {}",
        report["remedy"]
    );

    let (yes_code, cleared) = repair_report(&fixture, true);
    println!(
        "R14_M04 yes_exit={yes_code} blocked={}",
        cleared["repair_dir_blocked"]
    );
    assert_eq!(
        cleared["repair_dir_blocked"][0]["cleared"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        cleared["repair_dir_blocked"][0]["kept"],
        ".gx/repair.pre-repair.0"
    );
    assert!(
        fixture
            .project
            .join(".gx")
            .join("repair.pre-repair.0")
            .is_file(),
        "🔴 DR-43-7 (1): the bytes are moved and not removed — gx did not write them"
    );
    assert!(
        blocked.is_dir(),
        "and the declared directory is a directory again"
    );
    assert!(
        cleared["kept_aside"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row == ".gx/repair.pre-repair.0")),
        "🔴 `kept_aside` is the key that says what gx set aside: {}",
        cleared["kept_aside"]
    );

    let after = fixture.submit("one more, now");
    println!("R14_M04 submit_after_exit={}", after.code);
    assert_eq!(
        after.code, 0,
        "🔴 `req/222` H-06's standing rule: a state you can see has a way out of it: {}",
        after.stderr
    );
}

/// 🔴 **R14 / `req/246` L-01** — a road that refuses creates no directory, on all four of them.
///
/// 43 §7.15 (f) wrote the rule and R13 closed the one road `req/244` L-04 had measured
/// (`JOURNAL_ABSENT`). The other three refusals of `Layout::create` stood **after** the
/// `Shape::Dir` loop, so `DECLARATION_ABSENT`, `DECLARATION_UNREADABLE` and `CONFIG_ABSENT` each
/// left `.gx/evidence/` and `.gx/repair/` behind them. A rule closed at one of its four sites is a
/// rule closed at a place rather than as a count (`req/38` §181 ruling 3).
#[test]
fn every_road_that_refuses_creates_no_declared_directory() {
    /// One shape of damage, and the `gx_code` `Layout::create` answers it with.
    type Form = (&'static str, Box<dyn Fn(&Pipeline)>);
    let forms: Vec<Form> = vec![
        (
            "DECLARATION_ABSENT",
            Box::new(|f: &Pipeline| {
                std::fs::remove_file(version_path(f)).expect("remove the declaration");
            }),
        ),
        (
            "DECLARATION_UNREADABLE",
            Box::new(|f: &Pipeline| {
                std::fs::write(version_path(f), [0xff_u8, 0xfe, 0x00, 0x01])
                    .expect("write bytes that are not a declaration");
            }),
        ),
        (
            "CONFIG_ABSENT",
            Box::new(|f: &Pipeline| {
                std::fs::remove_file(config_path(f)).expect("remove the settings");
            }),
        ),
        (
            "JOURNAL_ABSENT",
            Box::new(|f: &Pipeline| {
                std::fs::remove_file(layout(f).journal_path()).expect("remove the journal");
            }),
        ),
    ];
    for (expected, break_it) in forms {
        let fixture = pipeline(
            &format!("model_a_r14_l01_{}", expected.to_lowercase()),
            "before\n",
        );
        fixture.commit_one("a change");
        let root = fixture.project.join(".gx");
        // Two declared directories removed, so that a road which makes them is visible.
        for dir in ["evidence", "repair"] {
            std::fs::remove_dir_all(root.join(dir)).expect("remove a declared directory");
        }
        break_it(&fixture);

        let submitted = fixture.submit("one more");
        let made: Vec<&str> = ["evidence", "repair"]
            .into_iter()
            .filter(|dir| root.join(dir).exists())
            .collect();
        println!(
            "R14_L01 form={expected} exit={} code={} created={made:?}",
            submitted.code,
            refusal_code(&submitted)
        );
        assert_eq!(
            submitted.code, 1,
            "{expected} refuses: {}",
            submitted.stderr
        );
        assert_eq!(refusal_code(&submitted), expected, "{}", submitted.stderr);
        assert!(
            made.is_empty(),
            "🔴 43 §7.15 (f) / `req/246` L-01: `{expected}` refused and made {made:?} on the way \
             out. The rule is about a **count** of refusing roads and not about the one road that \
             was measured"
        );
    }
}

// ---------------------------------------------------------------------------
// 🔴 R15 — the fifteenth audit's findings, as gates (`req/259`, `req/38` §188 ruling 2)
// ---------------------------------------------------------------------------

/// The seven `Shape::Dir` rows of req/56 §2, spelled out here so that a probe and the binary do not
/// read the same list — `crates/gx-cli/src/layout.rs`'s `declared_directories` is the subject being
/// measured, and a test that imported it would agree with itself.
const DECLARED_DIRECTORIES: [&str; 7] = [
    "ledger",
    "checkpoints",
    "evidence",
    "index",
    "drafts",
    "receipts",
    "repair",
];

/// 🔴 **R15 / `req/259` H-01** — the verbs that carry an **answer on stderr** keep the status they
/// had determined, whichever way stderr was thrown away.
///
/// # What the fifteenth audit measured
///
/// R14 moved 44 §1.3's problem object onto `emit::problem_line` and defined "every stream that
/// carries an answer" as the sites carrying a `.problem()` — a **payload**. The audit re-implemented
/// the census against the **destination** and found forty-three `eprintln!` sites still standing in
/// `crates/gx-cli/src/`, all of which panic on a write error. Two of them are answers by any
/// reading, and the arms below are the ones a buyer meets first. Each was three runs, no variation:
///
/// * `gx key gen --json 2>/dev/full` — exit **101**, stdout **0 bytes**, and the secret key already
///   written to `.gx/keys/`. The one string naming the key an operator had just made was the thing
///   that went missing.
/// * `gx wrap … 2>/dev/full` — exit **101** on the product's membrane, whose start-up JSON and
///   session summary are on stderr by design (stdout carries MCP frames).
/// * `gx demo 2>/dev/full` — the walk stopped after step 1 of 3, because the `gx wrap` it starts
///   died at 101.
///
/// The assertion is the one R14 wrote for refusals, applied to these: **membership of 44 §1.4's
/// table, and the same number from both spellings of "throw stderr away"**.
#[test]
fn a_verb_whose_answer_rides_stderr_keeps_its_number_when_stderr_is_full() {
    if dev_full().is_none() {
        println!("SKIPPED: this system has no /dev/full, so the H-01 arms cannot be measured");
        return;
    }
    let membrane = pipeline("model_a_r15_wrap", "before\n");
    membrane.commit_one("a change");
    let walker = pipeline("model_a_r15_demo", "before\n");
    let minting = pipeline("model_a_r15_keygen", "before\n");

    struct Arm {
        name: &'static str,
        build: Box<dyn Fn() -> Command>,
    }
    let arms: Vec<Arm> = vec![
        Arm {
            // The first command a buyer runs, and the one that writes a secret before it speaks.
            name: "`gx key gen --json` (the note comes after the key is on the disk)",
            build: {
                let home = minting.home.clone();
                Box::new(move || {
                    let mut c = support::gx();
                    c.env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .args(["key", "gen", "--json"]);
                    c
                })
            },
        },
        Arm {
            // The membrane. Its start-up line and its session summary are stderr by design.
            name: "`gx wrap` (the membrane's own reporting is stderr)",
            build: {
                let project = membrane.project.clone();
                let home = membrane.home.clone();
                let key = membrane.key_id.clone();
                Box::new(move || {
                    let mut c = support::gx();
                    c.env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .arg("--project")
                        .arg(&project)
                        .arg("wrap")
                        .args(["--endpoint", "stdio://r15"])
                        .args(["--actor-key", &key])
                        .args(["--actor-model", "probe-r15"])
                        .args(["--restore", "notes.write=notes.restore"])
                        .args(["--restore", "notes.restore=notes.write"])
                        .arg("--")
                        .arg(env!("CARGO_BIN_EXE_gx"))
                        .arg("__demo-notes-server");
                    c.stdin(Stdio::null());
                    c
                })
            },
        },
        Arm {
            name: "`gx demo` (the first experience, three steps of walk)",
            build: {
                let project = walker.project.clone();
                let home = walker.home.clone();
                Box::new(move || {
                    let mut c = support::gx();
                    c.env("HOME", &home)
                        .env("USERPROFILE", &home)
                        .arg("--project")
                        .arg(&project)
                        .arg("demo");
                    c
                })
            },
        },
    ];

    for arm in &arms {
        let mut statuses = Vec::new();
        for dead in [true, false] {
            let mut command = (arm.build)();
            let stderr = if dead {
                Stdio::from(dev_full().expect("/dev/full opened once already"))
            } else {
                Stdio::null()
            };
            // stdout's destination does not vary between the halves — R14's reasoning, unchanged:
            // the claim is "the same run, with stderr thrown away two different ways".
            command.stdout(Stdio::null()).stderr(stderr);
            let status = command.status().expect("the gx binary runs");
            let code = status.code().unwrap_or(-1);
            println!(
                "R15_H01 arm={:?} stderr={} exit={code}",
                arm.name,
                if dead { "/dev/full" } else { "/dev/null" },
            );
            assert!(
                is_in_the_exit_table(code),
                "🔴 `req/259` H-01: `{}` left 44 §1.4's table with {code}. Every `eprint!` in this \
                 binary panics on a write error, and R14 counted only the sites carrying a \
                 `.problem()` — so the answer-carrying ones stayed outside the table on a full \
                 disk. Exit 101 is what the audit measured, three runs out of three",
                arm.name
            );
            statuses.push(code);
        }
        assert_eq!(
            statuses[0], statuses[1],
            "🔴 43 §7.17 (a): the count is of destinations, not of payloads — `2>/dev/full` and \
             `2>/dev/null` are two ways of throwing stderr away and `{}` answered them differently \
             ({statuses:?})",
            arm.name
        );
    }
}

/// 🔴 **R15 / `req/259` H-01** — a key an operator made can be named again from the store.
///
/// The harm the audit measured was not the panic on its own: it was that `key_id` and `public_key`
/// existed **only** on the stream that never arrived, while the secret sat on the disk. The panic is
/// closed by [`a_verb_whose_answer_rides_stderr_keeps_its_number_when_stderr_is_full`]; this closes
/// the other half. `gx key list` already opened each file to answer `key_id_inside`, so both public
/// fields are derivable there and nothing new is exposed — a public key is the half that is
/// published.
#[test]
fn a_key_that_was_generated_can_be_named_again_from_the_store() {
    let fixture = pipeline("model_a_r15_key_recall", "before\n");
    let generated = support::run(fixture.gx().args(["key", "gen", "--json"]));
    assert_eq!(generated.code, 0, "{}", generated.stderr);
    let made = generated.json();
    let key_id = made["key_id"]
        .as_str()
        .expect("44 §1.2's key_id")
        .to_string();
    let public_key = made["public_key"]
        .as_str()
        .expect("44 §1.2's public_key")
        .to_string();

    let listed = support::run(fixture.gx().args(["key", "list", "--json"]));
    assert_eq!(listed.code, 0, "{}", listed.stderr);
    let rows = listed.json();
    let found = rows["keys"]
        .as_array()
        .expect("a list of keys")
        .iter()
        .find(|row| row["key_id"] == key_id.as_str())
        .unwrap_or_else(|| panic!("🔴 `req/259` H-01: {key_id} is not in `gx key list`: {rows}"));
    println!("R15_H01 recalled key_id={key_id} row={found}");
    assert_eq!(
        found["public_key"], public_key,
        "🔴 `req/259` H-01: `gx key gen`'s two public fields were only ever on a stream, and the \
         audit measured a run where that stream did not arrive — exit 101, stdout 0 bytes, the \
         secret on the disk. The store holds the seed, so both are derivable: {rows}"
    );
}

/// 🔴 **R15 / `req/259` M-01** — **every** declared directory blocked by something that is not one
/// has the same refusal and the same way out.
///
/// # What the fifteenth audit measured
///
/// R14 generalised the refusal (`Layout::create` pre-scans all seven `Shape::Dir` rows) and left the
/// **exit** at one name: `repair::repair_dir_state` read a `REPAIR_DIR = "repair"` constant. Seven
/// directories, three runs each. `.gx/evidence`, `.gx/index`, `.gx/drafts` and `.gx/receipts`
/// refused `gx submit` for ever while `gx repair` answered exit **0**, `remedy: null`,
/// `repair_dir_blocked: null` — the verb whose job is to say what is wrong called the project
/// healthy — and `--yes` set nothing aside. `.gx/checkpoints` exited 1 with no way out.
/// `.gx/ledger` came out as `HISTORY_LOST`, because the shape was asked **after** the existence
/// questions and `.gx/ledger/journal` cannot exist when `.gx/ledger` is a file.
///
/// # What is asserted, and the one row that is deliberately different
///
/// For every row: `LAYOUT_BLOCKED`, `gx repair` exit 1 naming the path, `--yes` setting the bytes
/// aside under `kept_aside` and making the directory. `.gx/ledger` then refuses again by its own
/// name (`HISTORY_LOST`) and that is right rather than a gap — the shape came back and the log did
/// not, which is what the remedy now says out loud. The other six return to `gx submit` exit 0.
#[test]
fn every_declared_directory_blocked_by_a_file_has_the_same_exit() {
    // The one row whose bytes *are* the project's history: clearing the shape cannot bring it back.
    let history = "ledger";
    for rel in DECLARED_DIRECTORIES {
        let fixture = pipeline(&format!("model_a_r15_m01_{rel}"), "before\n");
        fixture.commit_one("a change");
        let blocked = fixture.project.join(".gx").join(rel);
        std::fs::remove_dir_all(&blocked).expect("remove the declared directory");
        std::fs::write(&blocked, b"x").expect("put one byte where a directory belongs");

        let submitted = fixture.submit("one more");
        assert_eq!(
            refusal_code(&submitted),
            "LAYOUT_BLOCKED",
            "🔴 `req/259` M-01: `.gx/{rel}` is a declared directory and the operating system had \
             classified this completely: {}",
            submitted.stderr
        );

        let (code, report) = repair_report(&fixture, false);
        assert_eq!(
            code, 1,
            "🔴 `req/259` M-01: `gx repair` called `.gx/{rel}` healthy while every writer was \
             refused — four of the seven rows did exactly that: {report}"
        );
        let named = report["repair_dir_blocked"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row["path"] == format!(".gx/{rel}")));
        assert!(
            named,
            "🔴 `req/259` M-01: the report names the blocked row: {}",
            report["repair_dir_blocked"]
        );
        assert!(
            report["remedy"]
                .as_str()
                .is_some_and(|text| text.contains(&format!(".gx/{rel}"))),
            "🔴 the remedy names the path: {}",
            report["remedy"]
        );

        let (_, cleared) = repair_report(&fixture, true);
        assert!(
            blocked.is_dir(),
            "🔴 `req/259` M-01: `.gx/{rel}` is a directory again"
        );
        let aside = serde_json::Value::String(format!(".gx/{rel}.pre-repair.0"));
        assert!(
            cleared["kept_aside"]
                .as_array()
                .is_some_and(|rows| rows.contains(&aside)),
            "🔴 `req/244` L-01 / `req/259` M-01: `kept_aside` counts the names gx itself makes, and \
             R14's list held two of the seven: {}",
            cleared["kept_aside"]
        );
        assert!(
            fixture
                .project
                .join(".gx")
                .join(format!("{rel}.pre-repair.0"))
                .is_file(),
            "🔴 DR-43-7 (1): the bytes are moved and not removed — gx did not write them"
        );

        let after = fixture.submit("one more, now");
        // Printed from the raw stderr rather than through `refusal_code`, because six of the seven
        // rows end at exit 0 and a verb that answered has nothing on that stream to parse.
        println!(
            "R15_M01 rel={rel} submit_after_exit={} stderr={}",
            after.code,
            after.stderr.trim()
        );
        if rel == history {
            // The bytes sitting at `.gx/ledger` were not the log — the log is what the directory
            // held. So the shape comes back and the loss is still a loss, and what the assertion is
            // about is that gx names **that** rather than repeating the shape's word. Which loss it
            // is depends on what the project still has: `JOURNAL_ABSENT` where a receipt and a
            // recorded head survive, `HISTORY_LOST` where none of the three witnesses do. Both are
            // words about contents, which is the distinction R15 moved the shape scan above.
            let named = refusal_code(&after);
            assert!(
                named == "JOURNAL_ABSENT" || named == "HISTORY_LOST",
                "🔴 the shape came back and the log did not, and gx says so with a word about the \
                 contents rather than with `LAYOUT_BLOCKED` again: {named} / {}",
                after.stderr
            );
        } else {
            assert_eq!(
                after.code, 0,
                "🔴 `req/222` H-06's standing rule: a state you can see has a way out of it: {}",
                after.stderr
            );
        }
    }
}

/// 🔴 **R15 / `req/259` M-01** — the remedy gx hands the operator is true of the project it was
/// handed for.
///
/// # Why this is a probe and not a review note
///
/// `req/227` M-04 is the standing rule — a remedy naming the wrong file is worse than none — and
/// R14 cited it as its own reason for repairing `OUTPUT_FAILED`'s sentence. The fifteenth audit
/// found R14's **new** refusal breaking it: the problem object for `.gx/evidence` said, verbatim,
/// that `gx repair --yes` "renames it to `.gx/evidence.pre-repair.<n>`, makes the directory, and
/// names the copy it kept under `kept_aside`". All three clauses were false for six of the seven
/// rows. So the assertion is not that the sentence contains a word: it is that **doing what the
/// sentence says produces what the sentence promises**.
#[test]
fn the_remedy_for_a_blocked_directory_is_true_of_every_declared_directory() {
    for rel in DECLARED_DIRECTORIES {
        let fixture = pipeline(&format!("model_a_r15_remedy_{rel}"), "before\n");
        fixture.commit_one("a change");
        let blocked = fixture.project.join(".gx").join(rel);
        std::fs::remove_dir_all(&blocked).expect("remove the declared directory");
        std::fs::write(&blocked, b"x").expect("put one byte where a directory belongs");

        let submitted = fixture.submit("one more");
        let promised = format!(".gx/{rel}.pre-repair.<n>");
        assert!(
            submitted.stderr.contains(&promised),
            "🔴 the refusal promises the operator a rename to {promised}: {}",
            submitted.stderr
        );
        // 🔴 **R16 / `req/262` M-02** — do exactly what it says, and take the command **out of what
        // it said** rather than out of this file.
        //
        // R15's version of this line was `repair_report(&fixture, true)`, which builds
        // `gx repair --signing-key <ID> --yes`. The sentence said `gx repair --yes`. The sixteenth
        // audit ran the sentence on all seven directories in both shapes and got `cleared: false`
        // fourteen times out of fourteen — so this machine, built to measure whether a remedy is
        // true, had been measuring a command of its own invention. `req/262` M-02's own repair
        // note: a gate that composes its own arm cannot see a wrong sentence.
        let cleared = run_the_remedy_verbatim(&fixture, &submitted.stderr);
        let made = fixture
            .project
            .join(".gx")
            .join(format!("{rel}.pre-repair.0"));
        println!(
            "R15_M01 remedy rel={rel} promised={promised} kept={} exists={}",
            cleared["kept_aside"],
            made.exists()
        );
        assert!(
            made.is_file(),
            "🔴 `req/227` M-04 / `req/259` M-01: gx promised the rename and did not make it. The \
             audit measured this sentence being false for six of the seven declared directories"
        );
        assert!(
            blocked.is_dir(),
            "🔴 and the second clause — \"makes the directory\" — is true too"
        );
        let aside = serde_json::Value::String(format!(".gx/{rel}.pre-repair.0"));
        assert!(
            cleared["kept_aside"]
                .as_array()
                .is_some_and(|rows| rows.contains(&aside)),
            "🔴 and the third — \"names the copy it kept under `kept_aside`\": {}",
            cleared["kept_aside"]
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 🔴 R16 — the sixteenth audit's findings, as gates (`req/262`, `req/38` §192 ruling 2)
// ---------------------------------------------------------------------------------------------

/// The `gx …` command lines a refusal names, taken out of the refusal's own text.
///
/// 🔴 **R16 / `req/262` M-01 + M-02** — 43 §7.17 (b) condition 2 says the truth of a remedy is
/// measured by running what it tells you to run, and the machine R15 built ran a command **it**
/// composed. This is the reader that closes that: backtick-quoted, starting with `gx `, taken from
/// the problem object's own text exactly as an operator would copy it.
/// Source text with its comments removed, so that prose **about** a wrong sentence is not read as
/// the sentence.
///
/// 🔴 **R16** — the same cut `probes/doubt/tests/declaration_writer_doubt.rs` makes, and for the
/// same reason one file up: this repository keeps what a repair corrected (no-delete), so
/// `handlers.rs` and `pipeline.rs` both still *quote* the strings the audit found. A scan that read
/// a doc comment as a live string would make the record of a repair look like the fault.
fn code_of(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let bytes = line.as_bytes();
            let mut in_string = false;
            let mut escaped = false;
            for i in 0..bytes.len() {
                let c = bytes[i];
                if escaped {
                    escaped = false;
                } else if c == b'\\' && in_string {
                    escaped = true;
                } else if c == b'"' {
                    in_string = !in_string;
                } else if !in_string && c == b'/' && bytes.get(i + 1) == Some(&b'/') {
                    return &line[..i];
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn commands_named_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        let quoted = &rest[..close];
        rest = &rest[close + 1..];
        if quoted.starts_with("gx ") && !out.contains(&quoted.to_string()) {
            out.push(quoted.to_string());
        }
    }
    out
}

/// Run the `gx repair …` line a refusal printed, with **nothing added**, and hand back the report.
///
/// The only substitution is the placeholder the sentence itself uses for a value only the operator
/// has (`<KEY_ID>`), which is what an operator does when they read it. Any other divergence between
/// what gx printed and what runs here would put this probe back where `req/262` M-02 found it.
fn run_the_remedy_verbatim(fixture: &Pipeline, refusal: &str) -> serde_json::Value {
    let named = commands_named_in(refusal);
    let line = named
        .iter()
        .find(|c| c.starts_with("gx repair") && c.contains("--yes"))
        .unwrap_or_else(|| {
            panic!("🔴 `req/262` M-02: the refusal names no `gx repair … --yes` to run: {named:?}")
        })
        .clone();
    println!("R16_M02 remedy_verbatim=[{line}]");
    let mut command = fixture.gx();
    for word in line.split_whitespace().skip(1) {
        if word == "<KEY_ID>" {
            command.arg(&fixture.key_id);
        } else {
            command.arg(word);
        }
    }
    let run = support::run(&mut command);
    println!("R16_M02 ran rc={} stdout={}", run.code, run.stdout.trim());
    serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null)
}

/// 🔴 **R16 / `req/262` H-01** — an HTTP answer does not depend on the **other** stream.
///
/// # What the sixteenth audit measured
///
/// R15 closed every `eprintln!` under `crates/gx-cli/src/` and wrote the census against the
/// destination. The window was a crate; what ships is a binary. Six sites stood in `gx-api`, three
/// of them on `gx serve`'s request road, and on a project whose `.gx/drafts` was mode `0500` with
/// the server's standard error on `/dev/full`, `POST /v1/candidates` came back **0 bytes — no HTTP
/// status line at all** (three runs, no variation), where the same request on the same project
/// with `2>/dev/null` answered `201 Created`. The server stayed up, so what vanished was the
/// **request's answer**: a client cannot tell "gx refused" from "the network broke".
///
/// Neither half does it alone. The audit's control — a healthy project, standard error on
/// `/dev/full` — answered `201` on both destinations, so the fault is the **composition**, and both
/// halves come out of one cause: a filesystem that has stopped taking writes.
#[cfg(unix)]
#[test]
fn an_http_answer_does_not_depend_on_the_error_stream() {
    use std::os::unix::fs::PermissionsExt;

    let mut answers = Vec::new();
    for dest in ["/dev/null", "/dev/full"] {
        let fixture = pipeline(
            &format!("model_a_r16_h01_{}", dest.trim_start_matches("/dev/")),
            "before\n",
        );
        fixture.submit("make the layout");
        let drafts = fixture.project.join(".gx").join("drafts");
        std::fs::set_permissions(&drafts, std::fs::Permissions::from_mode(0o500))
            .expect("make the declared directory read-only");
        // 🔴 The fixture is void if this process can write anyway (it is root, or the filesystem
        // ignores the mode), and a void fixture that passes is worse than a red one.
        assert!(
            std::fs::File::create(drafts.join("probe")).is_err(),
            "🔴 `req/262` H-01: this arm needs a directory the process cannot write to, and this \
             one took a file — the measurement below would be about nothing"
        );

        let sink = std::fs::OpenOptions::new()
            .write(true)
            .open(dest)
            .unwrap_or_else(|e| panic!("open {dest}: {e}"));
        let serving = Serving::try_start_with_stderr(
            &fixture.project,
            &fixture.home,
            &fixture.key_id,
            Stdio::from(sink),
        )
        .unwrap_or_else(|why| panic!("gx serve was expected to serve on {dest}: {why}"));
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": fixture.target.display().to_string(),
            "goal": "agent-change\n",
            "context": "Evidence",
            "actor": { "Human": { "key": fixture.key_id } },
        });
        let (status, body) = serving.request("POST", "/v1/candidates", Some(&intent));
        println!("R16_H01 stderr={dest} status={status} bytes={}", body.len());
        assert_ne!(
            status, 0,
            "🔴 `req/262` H-01: the answer had no HTTP status line at all. An answer is a write to \
             a socket and may not depend on standard error: with stderr on {dest} and a read-only \
             `.gx/drafts`, the previous binary closed the connection after 0 bytes"
        );
        // And the server is still there, which is what says this was a lost answer rather than a
        // dead process — the shape the audit measured and the reason it is not a crash report.
        let (alive, _) = serving.request("GET", "/v1/transformations", None);
        assert_eq!(alive, 200, "🔴 the server answers the next request");
        answers.push(status);
        std::fs::set_permissions(&drafts, std::fs::Permissions::from_mode(0o700))
            .expect("put the mode back so the scratch directory can be cleaned");
    }
    assert_eq!(
        answers[0], answers[1],
        "🔴 `req/262` H-01: `2>/dev/null` and `2>/dev/full` are two ways of throwing standard \
         error away, and the HTTP answer is the same in both or it was never an answer: {answers:?}"
    );
    assert_eq!(
        answers[0], 201,
        "🔴 and the answer is 44 §2.1's own: the draft archive is best effort (req/38 §148 lane \
         R2), so a directory that will not take a file does not refuse a candidate the engine has \
         already drafted"
    );
}

/// 🔴 **R16 / `req/262` M-01** — a project every verb of this binary refuses does not get a server.
///
/// # What the sixteenth audit measured
///
/// `Layout::create` has asked the shape of every declared directory since R14 and has asked it
/// first since R15. `Layout::open` — the door `gx serve` uses — asked nothing. So on a project
/// whose `.gx/receipts` was one byte, `gx submit` answered exit 1 `LAYOUT_BLOCKED` and `gx serve`
/// **started**; the HTTP commit road then answered `201` → `200` → `500 INTERNAL` with the target
/// file already rewritten, one leaf on the ledger, zero commits in the journal, no commit receipt
/// and `gx undo` refusing. The effect applied and the document that proves it lost — two doors
/// giving one project opposite answers, and the losing side is the one that writes.
///
/// The refusal is at start-up rather than per endpoint, and the comparison is written down in
/// `layout.rs`: there is no read this keeps from anybody that the same project's CLI is not
/// already refusing, and a server that starts is a server an operator believes in.
#[test]
fn a_project_with_a_blocked_declared_directory_gets_no_server() {
    for rel in DECLARED_DIRECTORIES {
        let fixture = pipeline(&format!("model_a_r16_m01_{rel}"), "before\n");
        fixture.commit_one("a change");
        // What the substrate holds **after** the one commit this fixture makes on purpose. The
        // assertion below is that the refused server moves it no further.
        let settled = std::fs::read_to_string(&fixture.target).expect("read the target");
        let blocked = fixture.project.join(".gx").join(rel);
        std::fs::remove_dir_all(&blocked).expect("remove the declared directory");
        std::fs::write(&blocked, b"x").expect("put one byte where a directory belongs");

        let refused = fixture.submit("one more");
        assert_eq!(
            refused.code, 1,
            "🔴 the CLI refuses this project: {}",
            refused.stderr
        );
        assert!(
            refused.stderr.contains("LAYOUT_BLOCKED"),
            "🔴 by the shape's own name: {}",
            refused.stderr
        );

        let outcome = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id).err();
        let why = outcome.unwrap_or_else(|| {
            panic!(
                "🔴 `req/262` M-01: `gx serve` started on a project where `.gx/{rel}` is a regular \
                 file and every CLI verb refuses. The audit drove the HTTP commit road through \
                 exactly this server and it applied the change and then lost the receipt"
            )
        });
        println!("R16_M01 rel={rel} serve_refused={}", why.trim());
        assert!(
            why.contains("LAYOUT_BLOCKED"),
            "🔴 and it refuses by the same name the CLI uses, so one project has one answer: {why}"
        );
        assert_eq!(
            std::fs::read_to_string(&fixture.target).expect("read the target"),
            settled,
            "🔴 `req/262` M-01: nothing on the substrate moved — the whole point of refusing at \
             start-up is that no effect is applied before the receipt is found to be impossible. \
             The audit's server rewrote this file and then answered `500`"
        );
    }
}

/// 🔴 **R16 / `req/262` M-01** — every `gx …` a refusal names is a command this binary has.
///
/// # What the sixteenth audit measured
///
/// The `500` from the HTTP commit road said, word for word, "`gx receipt export` refiles it", and
/// `gx receipt` takes `show` and `verify`: `error: unrecognized subcommand 'export'`, exit 1. Two
/// sites carried that string. `req/227` M-04 is the standing rule — a remedy that names something
/// that does not exist is worse than no remedy — and this is its general form, run rather than
/// read: collect the backticked `gx …` lines out of real refusals and ask this binary about each
/// one.
///
/// The verb path is what is checked, not the whole line: a flag's *value* belongs to the operator
/// (`<KEY_ID>`), and `--help` on the path answers 0 exactly when clap knows the command.
#[test]
fn every_command_a_refusal_names_is_one_this_binary_has() {
    let fixture = pipeline("model_a_r16_m01_verbs", "before\n");
    fixture.commit_one("a change");

    // Every refusal this lane can reach without an adversary, in one bag.
    let mut refusals: Vec<String> = Vec::new();
    let blocked = fixture.project.join(".gx").join("evidence");
    std::fs::remove_dir_all(&blocked).expect("remove the declared directory");
    std::fs::write(&blocked, b"x").expect("one byte where a directory belongs");
    refusals.push(fixture.submit("one more").stderr);
    refusals.push(support::run(fixture.gx().arg("repair")).stdout);
    std::fs::remove_file(&blocked).expect("put it back");
    std::fs::create_dir(&blocked).expect("put it back");
    refusals.push(support::run(fixture.gx().args(["receipt", "show", "gx1:doesnotexist"])).stderr);
    // And the two sites the audit named, read out of the source, so that a string nothing in this
    // suite happens to reach is measured anyway.
    refusals.push(code_of(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("crates/")
                .join("gx-api/src/handlers.rs"),
        )
        .expect("read the HTTP surface"),
    ));

    let mut checked = 0usize;
    for text in &refusals {
        for line in commands_named_in(text) {
            let path: Vec<&str> = line
                .split_whitespace()
                .skip(1)
                .take_while(|w| !w.starts_with('-') && !w.starts_with('<'))
                .collect();
            if path.is_empty() {
                continue;
            }
            let mut command = fixture.gx();
            command.args(path.iter()).arg("--help");
            let asked = support::run(&mut command);
            checked += 1;
            assert_eq!(
                asked.code,
                0,
                "🔴 `req/227` M-04 / `req/262` M-01: gx printed `{line}` and this binary answers \
                 {} to `gx {} --help`. A remedy naming a verb that does not exist is worse than \
                 no remedy — the audit measured `gx receipt export` in two sites of the HTTP \
                 surface, on the road that has already applied the change: {}",
                asked.code,
                path.join(" "),
                asked.stderr.trim()
            );
        }
    }
    println!("R16_M01 command_lines_checked={checked}");
    assert!(
        checked >= 4,
        "🔴 a probe that found no command lines to check is green because it looked at nothing"
    );
}

/// 🔴 **R16 / `req/262` M-02** — a repair with no key says so, and the sentence that sends an
/// operator there names the flag.
///
/// # What the sixteenth audit measured
///
/// Seven declared directories, two shapes each: a plain `gx repair --yes` answered
/// `cleared: false`, `kept_aside: []` **fourteen times out of fourteen**, and a following
/// `gx submit` refused again — because `repair.rs`'s `writing` is
/// `yes && key.is_some() && _held.is_some()`. That behaviour is kept: everything past that point
/// can end in a signature, and `req/242` M-03 built the one road on which a run that cannot write
/// writes nothing at all. What was wrong was the **sentence**, which said `gx repair --yes` and
/// nothing about a key.
///
/// So this asserts the pair: the keyless run is honest about having changed nothing and names the
/// flag, and the refusal that sends an operator there carries the flag in the command line itself.
#[test]
fn a_repair_with_no_key_names_the_flag_it_needs() {
    let fixture = pipeline("model_a_r16_m02_keyless", "before\n");
    fixture.commit_one("a change");
    let blocked = fixture.project.join(".gx").join("evidence");
    std::fs::remove_dir_all(&blocked).expect("remove the declared directory");
    std::fs::write(&blocked, b"x").expect("one byte where a directory belongs");

    let refusal = fixture.submit("one more").stderr;
    let named = commands_named_in(&refusal);
    println!("R16_M02 commands_in_the_refusal={named:?}");
    assert!(
        named
            .iter()
            .any(|c| c.starts_with("gx repair") && c.contains("--signing-key")),
        "🔴 `req/262` M-02: the command line the refusal prints is the one that works. Without the \
         flag it is 14/14 `cleared: false`: {named:?}"
    );

    let keyless = support::run(fixture.gx().args(["repair", "--yes"]));
    let report: serde_json::Value =
        serde_json::from_str(keyless.stdout.trim()).unwrap_or(serde_json::Value::Null);
    println!("R16_M02 keyless rc={} report={}", keyless.code, report);
    let rows = report["repair_dir_blocked"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !rows.is_empty() && rows.iter().all(|r| r["cleared"] == serde_json::json!(false)),
        "🔴 the keyless run changed nothing, and the report says so rather than claiming a repair: \
         {report}"
    );
    assert!(
        blocked.is_file(),
        "🔴 and the project is exactly as the run found it"
    );
    assert!(
        report["meta_repair_refused"]
            .as_str()
            .is_some_and(|s| s.contains("--signing-key")),
        "🔴 `req/262` M-02: and the report names the flag too, so the one round trip an operator \
         loses is the last one: {}",
        report["meta_repair_refused"]
    );
}

/// 🔴 **R16 / `req/262` M-01** — the reason a refusal gives is the one the operating system gave.
///
/// The `500` on the HTTP commit road ended "What to fix: the write permission on `.gx/receipts/`,
/// or the disk it is on". The audit drove that road on a project where `.gx/receipts` was a
/// one-byte file, and what the operating system said was `File exists (os error 17)` — neither a
/// permission nor a disk. `req/244` H-03's standing lesson is that a refusal that asserts a cause
/// sends an operator to the wrong repair; the string the archive returned is already in the
/// message, so the sentence points at it instead of guessing.
#[test]
fn a_refusal_does_not_assert_a_cause_it_did_not_measure() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/");
    let engine = code_of(
        &std::fs::read_to_string(crates.join("gx-engine/src/pipeline.rs"))
            .expect("read the pipeline"),
    );
    assert!(
        !engine.contains("What to fix: the write permission on"),
        "🔴 `req/262` M-01 / `req/244` H-03: this sentence names two causes and the measured one \
         was a third (`File exists (os error 17)`)"
    );
    let surface = code_of(
        &std::fs::read_to_string(crates.join("gx-api/src/handlers.rs"))
            .expect("read the HTTP surface"),
    );
    assert!(
        !surface.contains("gx receipt export"),
        "🔴 `req/262` M-01: `gx receipt` has `show` and `verify`, and this string was handed to an \
         operator on the road that had already applied their change"
    );
    assert!(
        !surface.contains("req/56 §2's `Derived`"),
        "🔴 `req/262` M-01: `crates/gx-cli/src/layout.rs` gives `receipts` `Nature::Source`, with \
         the reason beside it — losing the directory loses receipts, so nothing re-derives them"
    );
}
