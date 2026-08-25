// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R7 — the threat model split in two, and the detector that had never been checked**
//! (`req/232`, `req/38` §171).
//!
//! # What the seventh adversarial audit measured
//!
//! R6 gave the project a **head**: a signed statement about the furthest it had reached, compared
//! at every door. `req/232` measured two things about it.
//!
//! **H-01 — the detector was never checked.** `HeadStore::read` looked at the version field and the
//! JSON shape and nothing else, so an attacker did not have to *delete* the head to switch it off:
//! writing `{"tree_size": 0, "journal_len": 0, "journal_head": null}` over it left the signature
//! where it was, `compare` found nothing to complain about, and the report went on saying
//! `head_recorded: true` — the one visible signal R6 had given an operator. The pair was then cut
//! in 43 §7-3b's window and the operator's file went from `three` back to `two`, with `gx serve`
//! starting, `/healthz` answering `200`, and the next `gx repair` exiting 0 with `remedy: null`.
//!
//! **H-02 — freshness is not authenticity.** `head.json` is rewritten at every commit, so an
//! attacker who keeps **one copy** of a genuine head has a document that will verify under any
//! check gx can perform. Put it back after a rollback and the head stops being a fence and becomes
//! a **floor**: every rollback that stops above it is invisible. Measured on a four-commit project
//! with a two-commit head restored: `rolled_back: null`, the server started, and the target went
//! from `four` to `three`.
//!
//! # What this lane does about each, and why the two answers are different
//!
//! `req/38` §171 ruling 1 splits the threat model in two (43 §7.9):
//!
//! * **Model A** — accidents, crashes, partial writes, power loss, co-tenancy, restarts,
//!   mis-edits, older tools, and any third party who cannot write to `.gx/`. **H = 0 is the
//!   target**, and H-01 is a Model A hole: a corrupt or forged head is now *refused* (the
//!   checkpoint's signature is verified when a key can be found, and the local numbers — journal
//!   length, chain head, `.gx/VERSION` digest, last leaf — travel in a signed witness rebuilt from
//!   the document's own fields).
//! * **Model B** — an adversary who can write to `.gx/`. H-02 is Model B and **is not closed here
//!   and cannot be**: the restored head is genuine, so no signature check reaches it, and every
//!   witness to freshness lives in the same write scope as the thing it witnesses. The probe for it
//!   below asserts that gx **passes** the project *and* that the world moves — because R6's
//!   `s1b_` asserted only `exit 0` and left a reader free to think "it passes, so no harm done".
//!   The answer is the copy that left the machine, and the same probe asserts that too.
//!
//! Every arm that measures an unclosed hole says so in its own name and asserts the **consequence**
//! as well as the pass (`req/232` §8 ①: a probe that fixes "it is not closed" without fixing "and
//! this is what that costs" is a probe a reader will misread).
//!
//! `cfg(unix)` for its predecessors' reasons — `SIGTERM`, `flock`, `chmod` — and with the same
//! declaration: Windows, WSL 9p and a synchronising client are **not measured** (`req/213` §7(d),
//! carried unchanged for an eleventh lane).

#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use support::{pipeline, Pipeline};

/// How long a socket read may block before the probe fails.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server has to print its start-up line.
const STARTUP_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve`, its address and its start-up line.
///
/// The eighth copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test
/// binary is its own crate.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Serving {
    fn start(project: &Path, home: &Path, key_id: &str) -> Self {
        Self::try_start(project, home, key_id).unwrap_or_else(|why| {
            panic!("gx serve was expected to serve and did not: {why}");
        })
    }

    /// The same start, for a probe whose **point** is that the server refuses.
    fn try_start(project: &Path, home: &Path, key_id: &str) -> Result<Self, String> {
        let token = "r7-runtime-token".to_string();
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
                return Err(why);
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
        Ok(Self {
            child,
            addr,
            token,
            stdout,
        })
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

    fn create_over_http(&self, locator: &str, goal: &str, actor_key: &str) -> String {
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": actor_key } },
        });
        let (status, body) = self.request("POST", "/v1/candidates", Some(&intent));
        assert_eq!(status, 201, "create: {body}");
        let created: serde_json::Value = serde_json::from_str(&body).expect("json");
        created["id"].as_str().expect("an id").to_string()
    }

    fn commit_over_http(&self, locator: &str, goal: &str, actor_key: &str) -> String {
        let id = self.create_over_http(locator, goal, actor_key);
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

fn journal_path(fixture: &Pipeline) -> PathBuf {
    layout(fixture).journal_path()
}

fn ledger_path(fixture: &Pipeline) -> PathBuf {
    layout(fixture).ledger_path()
}

fn head_path(fixture: &Pipeline) -> PathBuf {
    layout(fixture).head_path()
}

fn version_path(fixture: &Pipeline) -> PathBuf {
    fixture.project.join(".gx").join("VERSION")
}

// ---------------------------------------------------------------------------
// Reading both files' framing from outside the engine (the R5/R6 walk, unchanged)
// ---------------------------------------------------------------------------

/// Every record frame in a journal file, as `(offset, framed_length)`.
fn frames(bytes: &[u8]) -> Vec<(usize, usize)> {
    const CEILING: u32 = 1 << 20;
    let chained = bytes.len() >= 8 && {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[..4]);
        u32::from_be_bytes(header) > CEILING
    };
    let link = usize::from(chained) * 32;
    let mut at = usize::from(chained) * 8;
    let mut out = Vec::new();
    while at + 4 <= bytes.len() {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > CEILING as usize || at + 4 + length + link > bytes.len() {
            break;
        }
        out.push((at, 4 + length + link));
        at += 4 + length + link;
    }
    out
}

/// The ledger's frames — `[u32 length][payload]`, with no marker and no link.
fn ledger_frames(bytes: &[u8]) -> Vec<(usize, usize)> {
    const CEILING: u32 = 1 << 20;
    let mut at = 0usize;
    let mut out = Vec::new();
    while at + 4 <= bytes.len() {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > CEILING as usize || at + 4 + length > bytes.len() {
            break;
        }
        out.push((at, 4 + length));
        at += 4 + length;
    }
    out
}

/// The record kinds a journal holds, in order.
fn kinds(bytes: &[u8]) -> Vec<&'static str> {
    gx_engine::replay(bytes)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect()
}

/// Cut a file at `at` bytes, which the caller has taken off a frame boundary.
fn truncate_at(path: &Path, at: u64) -> (u64, u64) {
    let before = std::fs::metadata(path).expect("stat").len();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
    println!("TRUNCATED {} {before} -> {at}", path.display());
    (before, at)
}

/// The offset of the `ApplyStarted` record that precedes the `n`th `Committed`, and the ledger
/// offset that keeps `n` leaves — 43 §7-3b's window, which is where a cut moves the world.
fn window_cuts(fixture: &Pipeline, nth_commit: usize) -> (u64, u64) {
    let journal = std::fs::read(journal_path(fixture)).expect("read the journal");
    let spans = frames(&journal);
    let record_kinds = kinds(&journal);
    let committed = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .nth(nth_commit)
        .expect("that many `Committed` records");
    let journal_cut = (spans[committed - 1].0 + spans[committed - 1].1) as u64;
    let ledger = std::fs::read(ledger_path(fixture)).expect("read the ledger");
    let leaves = ledger_frames(&ledger);
    let ledger_cut = (leaves[nth_commit].0 + leaves[nth_commit].1) as u64;
    println!(
        "WINDOW nth={nth_commit} journal_cut={journal_cut} ledger_cut={ledger_cut} \
         kind_before={}",
        record_kinds[committed - 1]
    );
    (journal_cut, ledger_cut)
}

/// A project with three commits behind it, its server already stopped.
fn three_commits(name: &str) -> Pipeline {
    let fixture = pipeline(name, "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    for goal in ["one\n", "two\n", "three\n"] {
        server.commit_over_http(&locator, goal, &fixture.key_id);
    }
    shut_down(server);
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "the fixture's own control: three commits, the last one wins"
    );
    fixture
}

/// The JSON `gx repair` prints, and its exit status.
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
    if !run.stderr.trim().is_empty() {
        println!("REPAIR_STDERR={}", run.stderr.trim());
    }
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json)
}

/// `gx repair --against <FILE>`.
fn repair_against(fixture: &Pipeline, against: &Path) -> (i32, serde_json::Value) {
    let mut command = fixture.gx();
    command.arg("repair").arg("--against").arg(against);
    let run = support::run(&mut command);
    println!(
        "REPAIR_AGAINST exit={} stdout={}",
        run.code,
        run.stdout.trim()
    );
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json)
}

/// `gx repair --yes --accept-rollback --against <FILE>` — R7's one road onto a shorter tree.
fn repair_accepting(fixture: &Pipeline, against: &Path) -> (i32, serde_json::Value, String) {
    let mut command = fixture.gx();
    command
        .arg("repair")
        .args(["--signing-key", &fixture.key_id])
        .arg("--yes")
        .arg("--accept-rollback")
        .arg("--against")
        .arg(against);
    let run = support::run(&mut command);
    println!(
        "REPAIR_ACCEPT exit={} stdout={} stderr={}",
        run.code,
        run.stdout.trim(),
        run.stderr.trim()
    );
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json, run.stderr)
}

/// `gx checkpoint export <FILE>` — the copy that leaves the machine.
fn export_head(fixture: &Pipeline, to: &Path) -> (i32, serde_json::Value, String) {
    let run = support::run(fixture.gx().args(["checkpoint", "export"]).arg(to));
    println!(
        "EXPORT exit={} stdout={} stderr={}",
        run.code,
        run.stdout.trim(),
        run.stderr.trim()
    );
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json, run.stderr)
}

/// `gx log checkpoint --key <FILE>` — the verb that mints a signed head by hand.
fn mint_checkpoint(fixture: &Pipeline) -> (i32, serde_json::Value, String) {
    let key_file = fixture
        .home
        .join(".gx")
        .join("keys")
        .join(format!("{}.key", fixture.key_id));
    let run = support::run(
        fixture
            .gx()
            .args(["log", "checkpoint", "--key"])
            .arg(&key_file),
    );
    println!(
        "MINT exit={} stdout={} stderr={}",
        run.code,
        run.stdout.trim(),
        run.stderr.trim()
    );
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json, run.stderr)
}

/// Rewrite `head.json` through a closure, leaving every field the closure does not touch.
///
/// 🔴 The whole of `req/232` H-01's toolkit: one `write(2)` over a JSON document, with the
/// signature left exactly where gx put it.
fn edit_head(fixture: &Pipeline, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = head_path(fixture);
    let mut head: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("read the head"))
            .expect("the head is JSON");
    edit(&mut head);
    std::fs::write(&path, serde_json::to_vec_pretty(&head).expect("serialise")).expect("write");
    println!("HEAD_EDITED {}", path.display());
}

// ---------------------------------------------------------------------------
// H-01 — the forged head (Model A: closed)
// ---------------------------------------------------------------------------

/// 🔴 **`req/232` H-01** — a head whose numbers were rewritten is refused, and the world stays put.
///
/// Measured on the parent commit: `gx repair` exit 1 with `rolled_back: null` **and**
/// `head_recorded: true`, `gx serve` **started**, `/healthz 200`, a signed checkpoint at
/// `tree_size: 2`, `TARGET_AFTER='two\n'` (it had been `three`), and a final `gx repair` at exit 0
/// with `remedy: null`. The attack is one `write(2)`: `tree_size` to `0`, `journal_len` to `0`,
/// `journal_head` to `null`, signature untouched.
#[test]
fn h01_a_head_whose_numbers_were_rewritten_is_refused_and_the_world_stays_put() {
    let fixture = three_commits("r7_h01_forged");
    edit_head(&fixture, |head| {
        head["checkpoint"]["tree_size"] = serde_json::json!(0);
        head["journal_len"] = serde_json::json!(0);
        head["journal_head"] = serde_json::Value::Null;
    });
    let (journal_cut, ledger_cut) = window_cuts(&fixture, 1);
    truncate_at(&journal_path(&fixture), journal_cut);
    truncate_at(&ledger_path(&fixture), ledger_cut);

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(
        code, 0,
        "a project whose detector was replaced is not healthy"
    );
    assert_eq!(
        report["head_authenticity"], "refuted",
        "the document is there and it is not one this binary will read numbers off: {report}"
    );
    assert!(
        report["head_invalid"]
            .as_str()
            .is_some_and(|why| why.contains("does not verify")),
        "the refusal names the signature: {report}"
    );
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("head.json")),
        "and names the file to move aside — req/222 H-06's trap, one file along: {report}"
    );

    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("a project whose head was forged is not one to serve from");
    println!("RESTART_REFUSED={refusal}");
    assert!(refusal.contains("head_invalid"), "{refusal}");
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "the whole finding: the operator's file does not go back to `two`"
    );
}

/// 🔴 **`req/232` H-01, the second half** — the refused head is **not** overwritten.
///
/// A repair that replaced the document it refused would destroy the only evidence of what was done
/// to the project, and would turn "somebody replaced the detector" into "the detector is fine now".
/// `--yes` is the road that writes, so `--yes` is the road this is asserted on.
#[test]
fn h01_a_repair_does_not_overwrite_the_head_it_refused() {
    let fixture = three_commits("r7_h01_evidence");
    edit_head(&fixture, |head| {
        head["checkpoint"]["tree_size"] = serde_json::json!(0);
        head["journal_len"] = serde_json::json!(0);
        head["journal_head"] = serde_json::Value::Null;
    });
    let forged = std::fs::read(head_path(&fixture)).expect("read the forged head");

    let (code, report) = repair_report(&fixture, true);
    assert_ne!(code, 0, "{report}");
    assert_eq!(report["mode"], "yes");
    assert_eq!(
        report["recover"]["resumed"], 0,
        "the recovery does not run over a head this binary refused: {report}"
    );
    assert_eq!(
        std::fs::read(head_path(&fixture)).expect("read the head again"),
        forged,
        "the refused document is evidence and is left where it lies"
    );
    assert_eq!(fixture.target_contents(), "three\n");
}

// ---------------------------------------------------------------------------
// H-02 — the replayed head (Model B: **not** closed, and the probe says so)
// ---------------------------------------------------------------------------

/// 🔴 **`req/232` H-02 — the arm that asserts a hole, and asserts what the hole costs.**
///
/// An older head **this project itself signed**, put back in place. Nothing is forged, so no
/// signature check reaches it — the document is authentic and the question is *freshness*, which
/// cannot be answered from inside the project because every witness to it is in the same write
/// scope as the thing it witnesses (43 §7.9 Model B).
///
/// R6's `s1b_` had this attack and asserted only `exit 0`. `req/232` §8 ① names that as the failure
/// mode of "assert the hole in green": a reader concludes "it passes, so nothing happened". So this
/// arm asserts the whole of it — the project reports itself healthy, the server starts, **and the
/// operator's file goes from `four` back to `three`** — and then asserts the one thing that does
/// answer: the checkpoint that left the machine.
#[test]
fn h02_an_older_head_this_project_signed_is_accepted_and_the_world_moves() {
    let fixture = pipeline("r7_h02_replay", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    server.commit_over_http(&locator, "two\n", &fixture.key_id);
    shut_down(server);
    // The attacker's copy, taken while the project is honest. One `cp`, no key, no forgery.
    let stolen = fixture.home.join("head-at-two.json");
    std::fs::copy(head_path(&fixture), &stolen).expect("keep a copy of the head at two leaves");

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "three\n", &fixture.key_id);
    server.commit_over_http(&locator, "four\n", &fixture.key_id);
    shut_down(server);
    assert_eq!(fixture.target_contents(), "four\n");
    // The auditor's copy, taken at the later size and kept off the machine.
    let auditors = fixture.home.join("auditor-at-four.json");
    let (export_code, export, _) = export_head(&fixture, &auditors);
    assert_eq!(export_code, 0, "{export}");
    assert_eq!(export["tree_size"], 4);
    assert_eq!(
        export["signature_checked"], true,
        "🔴 R7 / req/232 M-06: the export says whether it checked what it copied: {export}"
    );

    // Roll the project back into 43 §7-3b's window on the third commit, then put the old head back.
    let (journal_cut, ledger_cut) = window_cuts(&fixture, 2);
    truncate_at(&journal_path(&fixture), journal_cut);
    truncate_at(&ledger_path(&fixture), ledger_cut);
    std::fs::copy(&stolen, head_path(&fixture)).expect("put the older signed head back");

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        report["head_authenticity"], "verified",
        "🔴 the measurement: the document is genuine — this project signed it — so every check gx \
         can perform passes: {report}"
    );
    assert_eq!(
        report["rolled_back"],
        serde_json::Value::Null,
        "🔴 and the floor it states is one the project is above, so nothing is refused. This \
         assertion is the measurement, not the goal (43 §7.9 Model B)"
    );
    println!("H02_REPAIR exit={code}");

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (health, health_body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "{health_body}");
    shut_down(server);
    // 🔴 **R13 / `req/244` H-03 — what the hole costs is now smaller, and this is the measurement
    // of the difference.**
    //
    // Until R13 this line asserted `"three\n"`: the start-up recovery walked 43 §7-3b by
    // **re-applying** the third commit's delta, because 42 §3.10's `postcondition_fingerprint` is a
    // reading of the world and no journal record carries one — so an operator's file went from
    // `four` back to `three` on a project that reported itself healthy. That was the cost of the
    // Model B freshness hole, asserted rather than left green (`req/232` §8 ①).
    //
    // `req/244` H-03 measured the same road from the other end: a `gx wrap` commit killed inside
    // that window could not be closed *at all*, because `gx repair` has no MCP server to re-apply
    // through, and the refusal was answered with a terminal `Aborted`. The repair for that is
    // `RecoveryPath::ClosedFromFiledReceipt` — where the commit receipt the critical section
    // already filed digests to the leaf the ledger already witnessed, the `Committed` record is
    // written from **the document** and no adapter is asked anything. The third commit's receipt is
    // on this project's disk, so that is the road this start-up takes, and the world is not
    // touched.
    //
    // What is unchanged, and is what this arm is actually about: the older head **this project
    // signed** was accepted (`head_authenticity: "verified"`, `rolled_back: null`), the server
    // started 200 over it, and nothing inside the project could tell that the document was stale.
    // 43 §7.9's Model B hole is exactly where it was. What has moved is that gx no longer writes to
    // the operator's substrate on the way through it — which is a narrowing of the harm and not a
    // closing of the hole, and the two must not be read as one.
    assert_eq!(
        fixture.target_contents(),
        "four\n",
        "🔴 R13 / `req/244` H-03: the start-up recovery closes 43 §7-3b's window from the filed \
         commit receipt, so it no longer re-applies the third commit's delta over a world that \
         says `four`. Before R13 this was `three` — the hole's cost. The hole itself (an older \
         head this project signed, accepted as fresh) is measured by the three assertions above \
         and is unchanged"
    );

    // 🔴 The answer, and the only one: the document that was not in the attacker's write scope.
    let (against_code, against) = repair_against(&fixture, &auditors);
    assert_ne!(against_code, 0, "{against}");
    assert_eq!(against["against"]["foreign"], false, "{against}");
    assert_eq!(against["against"]["tree_size"], 4);
    assert_eq!(against["against"]["project_tree_size"], 3);
    assert_eq!(
        against["against"]["rolled_back"], true,
        "the copy taken at the later size is what the attacker could not reach: {against}"
    );
}

// ---------------------------------------------------------------------------
// M group
// ---------------------------------------------------------------------------

/// 🔴 **`req/232` M-02** — the declaration can be **rewritten**, and now that is caught.
///
/// R6 refused a project whose `.gx/VERSION` had lost its `journal_format` line. The audit did not
/// delete the line: it wrote `legacy` over `chained`, and `gx repair` answered
/// `journal_format_declared: "legacy"`, `downgraded: false` — R6's whole downgrade refusal lifted
/// by one `write(2)`, with the file still present and still well formed.
///
/// The head now records the digest of `.gx/VERSION`, so the rewrite is the same kind of fact as a
/// shortened journal. **Model A**: this catches an old tool, a mis-edit and a restore from the
/// wrong backup. It does not catch an adversary who can also rewrite the head — 43 §7.9 Model B,
/// and `s2_` below measures exactly that boundary.
#[test]
fn m02_rewriting_the_declaration_is_caught_by_the_recorded_head() {
    let fixture = three_commits("r7_m02_declaration");
    let before = std::fs::read_to_string(version_path(&fixture)).expect("read VERSION");
    assert!(before.contains("journal_format=chained"), "{before:?}");
    std::fs::write(version_path(&fixture), "1\njournal_format=legacy\n").expect("rewrite VERSION");

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(code, 0, "a rewritten declaration is not a healthy project");
    assert!(
        report["rolled_back"]
            .as_str()
            .is_some_and(|why| why.contains("`.gx/VERSION`")),
        "the refusal names the file that changed: {report}"
    );
    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("and the server does not start on it");
    assert!(refusal.contains("rolled_back"), "{refusal}");
}

/// 🔴 **`req/232` M-01** — a repair does not mint a **first** head over a tree it just wrote through.
///
/// The audit's road: delete the detector (R6's `s1_` shape), cut the pair in 43 §7-3b's window, run
/// `gx repair --yes`. The recovery re-applied an old delta and then `record_head` — which writes
/// unconditionally when there is no floor — minted a head over the shortened tree, so the rollback
/// became this project's attested past with nothing saying it had ever been longer.
///
/// The rollback itself is Model B and is not closed. The **laundering** is closed: after this run
/// there is still no head, so the project goes on reporting `head_recorded: false` — "no statement
/// about my past" — instead of attesting the shorter one.
#[test]
fn m01_a_repair_does_not_mint_a_first_head_over_a_tree_it_just_recovered() {
    let fixture = three_commits("r7_m01_laundering");
    std::fs::remove_file(head_path(&fixture)).expect("the attacker deletes the detector");
    let (journal_cut, ledger_cut) = window_cuts(&fixture, 1);
    truncate_at(&journal_path(&fixture), journal_cut);
    truncate_at(&ledger_path(&fixture), ledger_cut);

    let (code, report) = repair_report(&fixture, true);
    println!("M01_REPAIR exit={code}");
    assert_eq!(
        report["head_recorded"], false,
        "🔴 the finding: a run that has just written through a project does not get to mint its \
         first attested floor. `head_recorded: false` is the honest answer — this project has made \
         no statement about its past: {report}"
    );
    assert!(
        !head_path(&fixture).exists(),
        "and no head file appears on disk"
    );
}

/// 🔴 **`req/38` §171 ruling 2(c)** — the way onto a shorter tree, and the three things it demands.
///
/// An operator who has restored from a backup genuinely needs to move the floor. R6 had no way to
/// say so, so `gx repair --yes` did it silently (M-01). This is the explicit road: `--yes`,
/// `--accept-rollback`, and a checkpoint from **outside** the project that the project is not
/// behind. The new head records what it replaced.
#[test]
fn accept_rollback_moves_the_floor_only_with_evidence_from_outside() {
    let fixture = pipeline("r7_accept", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    server.commit_over_http(&locator, "two\n", &fixture.key_id);
    shut_down(server);
    // The backup an operator restores **to**: two commits, exported while healthy.
    let at_two = fixture.home.join("kept-at-two.json");
    assert_eq!(export_head(&fixture, &at_two).0, 0);
    let journal_at_two = std::fs::metadata(journal_path(&fixture))
        .expect("stat")
        .len();
    let ledger_at_two = std::fs::metadata(ledger_path(&fixture))
        .expect("stat")
        .len();

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "three\n", &fixture.key_id);
    shut_down(server);
    let at_three = fixture.home.join("kept-at-three.json");
    assert_eq!(export_head(&fixture, &at_three).0, 0);

    // The restore: the two files go back to where the backup had them, the head does not.
    truncate_at(&journal_path(&fixture), journal_at_two);
    truncate_at(&ledger_path(&fixture), ledger_at_two);
    let (code, report) = repair_report(&fixture, false);
    assert_ne!(code, 0, "the project is behind its own head");
    assert!(report["rolled_back"].is_string(), "{report}");

    // 🔴 The wrong evidence: a checkpoint that says the tree was **longer** does not authorise
    // taking the shorter one.
    let (bad_code, bad, _) = repair_accepting(&fixture, &at_three);
    assert_ne!(bad_code, 0, "{bad}");
    assert_eq!(
        bad["accepted_rollback"],
        serde_json::Value::Null,
        "nothing was accepted: {bad}"
    );
    assert!(
        bad["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("--accept-rollback was not honoured")),
        "{bad}"
    );

    // 🔴 The right evidence: the checkpoint the operator restored **to**.
    let (good_code, good, stderr) = repair_accepting(&fixture, &at_two);
    assert_eq!(good_code, 0, "{good} {stderr}");
    assert_eq!(
        good["accepted_rollback"]["was_tree_size"], 3,
        "the new head records the tree it replaced, so the next reader can see what was given up: \
         {good}"
    );
    let (after_code, after) = repair_report(&fixture, false);
    assert_eq!(after_code, 0, "{after}");
    assert_eq!(after["rolled_back"], serde_json::Value::Null, "{after}");
    assert_eq!(after["head_authenticity"], "verified", "{after}");
}

/// 🔴 **`req/232` M-04** — a checkpoint from another project is named, not believed.
///
/// R6 read `origin` and `key_id`, printed both, and compared neither: a healthy project handed
/// another healthy project's export answered `rolled_back: true` with a remedy that said "it was
/// signed by this project's own key". 42 §3.11 makes `origin` the field that stops a checkpoint of
/// one log verifying against another's; this is the caller that had been ignoring it.
#[test]
fn m04_an_export_from_another_project_is_named_rather_than_believed() {
    let mine = three_commits("r7_m04_mine");
    let theirs = three_commits("r7_m04_theirs");
    let foreign = theirs.home.join("their-head.json");
    assert_eq!(export_head(&theirs, &foreign).0, 0);

    let (code, report) = repair_against(&mine, &foreign);
    assert_ne!(
        code, 0,
        "a question that could not be asked is not a clean bill"
    );
    assert_eq!(report["against"]["foreign"], true, "{report}");
    assert_eq!(
        report["against"]["rolled_back"],
        serde_json::Value::Null,
        "🔴 a checkpoint that is not this project's says nothing about it — in either direction: \
         {report}"
    );
    assert_ne!(
        report["against"]["key_id"], report["against"]["project_key_id"],
        "the two keys are printed side by side, which is what the GUI shows: {report}"
    );
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|why| why.contains("is not this project's")),
        "{report}"
    );
    assert_eq!(
        report["ledger_agrees_after"], true,
        "and the project's own diagnosis is unaffected by the auditor's mistake: {report}"
    );
}

/// 🔴 **`req/232` L-01** — an unreadable `--against` file does not take the diagnosis with it.
#[test]
fn l01_an_unreadable_external_checkpoint_does_not_lose_the_projects_diagnosis() {
    let fixture = three_commits("r7_l01_unreadable");
    let broken = fixture.home.join("truncated.json");
    std::fs::write(&broken, "{\"origin\":\"glovrex-ledger/v1\",\"tre").expect("write half a file");

    let (code, report) = repair_against(&fixture, &broken);
    assert_ne!(code, 0, "{report}");
    assert_eq!(report["against"]["readable"], false, "{report}");
    assert_eq!(
        report["journal_commits"], report["ledger_leaves"],
        "the project's own numbers are still in the report: {report}"
    );
    assert_eq!(report["head_authenticity"], "verified", "{report}");
}

/// 🔴 **`req/232` M-05** — `gx log checkpoint` will not sign a tree the recorded head contradicts.
///
/// The verb that **mints** a signed head had never read the head store that exists to protect one:
/// the audit cut a commit out of a three-leaf project, committed again to reach three leaves, and
/// got a second signed root for `tree_size: 3` under the same key. Two attested histories of one
/// length is the failure a transparency log exists to make impossible.
#[test]
fn m05_log_checkpoint_will_not_sign_a_tree_the_recorded_head_contradicts() {
    let fixture = three_commits("r7_m05_equivocation");
    let (first_code, first, _) = mint_checkpoint(&fixture);
    assert_eq!(first_code, 0, "{first}");
    assert_eq!(first["tree_size"], 3);
    let published_root = first["root_hash"].as_str().expect("a root").to_string();

    // Cut a commit out of the history, outside the commit window, leaving the head in place.
    let journal = std::fs::read(journal_path(&fixture)).expect("read");
    let spans = frames(&journal);
    let record_kinds = kinds(&journal);
    let second_committed = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .nth(1)
        .expect("three `Committed` records");
    truncate_at(
        &journal_path(&fixture),
        (spans[second_committed].0 + spans[second_committed].1) as u64,
    );
    let ledger = std::fs::read(ledger_path(&fixture)).expect("read");
    let leaves = ledger_frames(&ledger);
    truncate_at(&ledger_path(&fixture), (leaves[1].0 + leaves[1].1) as u64);

    let (code, json, stderr) = mint_checkpoint(&fixture);
    assert_ne!(
        code, 0,
        "the verb that mints a signed head refuses a tree the head store contradicts: {json}"
    );
    assert!(
        stderr.contains("rolled_back"),
        "and it refuses in the words every other face uses: {stderr}"
    );
    assert!(
        !stderr.contains(&published_root),
        "no second document is produced at all, so the first root is not restated: {stderr}"
    );
}

/// 🔴 **`req/232` M-06** — `gx checkpoint export` verifies before it copies.
///
/// The one artefact this product tells a buyer to carry out of the box was being copied without
/// anybody looking at it: the audit exported from a project whose head said `tree_size: 0`, got
/// exit 0, and got a document listing `key_id` and `signed_fields` as if it were attested.
#[test]
fn m06_export_refuses_a_head_whose_signature_does_not_check_out() {
    let fixture = three_commits("r7_m06_export");
    edit_head(&fixture, |head| {
        head["checkpoint"]["tree_size"] = serde_json::json!(0);
    });
    let out = fixture.home.join("forged-export.json");
    let (code, json, stderr) = export_head(&fixture, &out);
    assert_ne!(code, 0, "{json}");
    assert!(stderr.contains("does not verify"), "{stderr}");
    assert!(
        !out.exists(),
        "and nothing that is not evidence is written where an auditor keeps evidence"
    );
}

/// 🔴 **`req/232` M-07** — a broken `head.json` still lets the reader's door open.
///
/// `req/227` M-04 and `req/229` M-02 closed this for the ledger and the verdict chain: *the set of
/// projects the reporting mode can open must not be narrower than the set the repairing mode can*.
/// R6 reopened it with the file it added — one byte of rubbish and `gx repair` printed no JSON at
/// all, `gx_code: "INTERNAL"`, which is 44 §2.3's word for "not classifiable" about a state that is
/// entirely classifiable.
#[test]
fn m07_a_broken_head_still_lets_the_reporting_door_open() {
    let fixture = three_commits("r7_m07_broken");
    std::fs::write(head_path(&fixture), "x").expect("one byte of rubbish");

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(
        code, 0,
        "fail-closed: a corrupt detector is not an absent one"
    );
    assert!(
        report.is_object(),
        "🔴 the diagnosis is printed rather than swallowed by an INTERNAL: {report}"
    );
    assert_eq!(report["head_authenticity"], "refuted", "{report}");
    assert_eq!(
        report["journal_commits"], 3,
        "and the rest of the diagnosis — which is independent of this file — is all there: {report}"
    );
    assert_eq!(report["ledger_leaves"], 3, "{report}");

    // The other reading verbs are unaffected: they do not read the head.
    let replayed = support::run(fixture.gx().args(["replay", "--json"]));
    assert_eq!(replayed.code, 0, "{}", replayed.stderr);
}

/// 🔴 **`req/232` M-08** — a second server signs nothing from a tree it has not caught up to.
///
/// Two servers on one project. The second commits; the first is asked for a signed checkpoint. R6
/// answered with a **signed** `tree_size: 1` over a two-leaf tree, stamped with the current time —
/// a document whose numbers and whose clock came from different moments, under one key. The signing
/// road now takes `.gx/LOCK` and catches up inside it, so the answer is either current or `BUSY`.
#[test]
fn m08_a_second_server_does_not_sign_a_tree_it_has_not_caught_up_to() {
    let fixture = pipeline("r7_m08_two_servers", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let first = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    first.commit_over_http(&locator, "via1\n", &fixture.key_id);
    let second = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    second.commit_over_http(&locator, "via2\n", &fixture.key_id);

    let (status, body) = first.request("GET", "/v1/ledger/checkpoint", None);
    println!("STALE_CHECKPOINT status={status} body={body}");
    assert_eq!(status, 200, "{body}");
    let signed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(
        signed["tree_size"], 2,
        "🔴 the finding: the server that had not caught up used to sign the tree it last saw, with \
         a timestamp from now. A stale read is a read; a stale signature is a document: {body}"
    );

    shut_down(second);
    shut_down(first);
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 0, "and the project is healthy afterwards: {report}");
}

// ---------------------------------------------------------------------------
// Self-adversarial — this lane attacking what this lane wrote
// ---------------------------------------------------------------------------

/// 🔴 **S-1 — an environment with no key does not get a pass.**
///
/// The whole of this lane's H-01 repair depends on finding a public key for the id the head names.
/// A third party, an encrypted key, a stripped key store: all of them mean *nothing was checked*,
/// and the failure mode to avoid is reporting that as though it had been. The other failure mode is
/// as bad: refusing to open a perfectly healthy project because a signature could not be checked
/// (`req/227` M-03 — the investigator's copy). Both are asserted here, in one arm.
#[test]
fn s1_a_project_whose_key_is_not_in_this_store_is_unverified_and_still_opens() {
    let fixture = three_commits("r7_s1_no_key");
    let key_file = fixture
        .home
        .join(".gx")
        .join("keys")
        .join(format!("{}.key", fixture.key_id));
    let stashed = fixture.home.join("key.stashed");
    std::fs::rename(&key_file, &stashed).expect("take the key out of the store");

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        code, 0,
        "a healthy project whose signature nobody here can check is still a healthy project: \
         {report}"
    );
    assert_eq!(
        report["head_authenticity"], "unverified",
        "🔴 and it says `unverified` rather than `verified`: the audit's finding was that a missing \
         check was being reported as a passed one: {report}"
    );
    assert_eq!(report["head_recorded"], true, "{report}");

    // 🔴 And the same project, with the key back, says what it means.
    std::fs::rename(&stashed, &key_file).expect("put the key back");
    let (code, report) = repair_report(&fixture, false);
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["head_authenticity"], "verified", "{report}");
}

/// 🔴 **S-2 — the numbers the *checkpoint's* signature does not cover.**
///
/// This lane's own hole, aimed at deliberately. `Checkpoint` signs `{origin, tree_size, root_hash}`
/// and nothing else, so verifying it says nothing about `journal_len`, `journal_head`, the
/// `.gx/VERSION` digest or the last leaf — exactly the numbers the recovery reads. If the witness
/// signature were absent, or were checked against a stored payload rather than a rebuilt one, this
/// edit would pass every check and the head would go on gating a journal it no longer describes.
#[test]
fn s2_editing_a_number_the_checkpoint_signature_does_not_cover_is_still_refused() {
    let fixture = three_commits("r7_s2_witness");
    edit_head(&fixture, |head| {
        // Not covered by `checkpoint.signature`. Covered by the witness, which is rebuilt from
        // this very field before the signature over it is checked.
        head["journal_len"] = serde_json::json!(1);
    });

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(code, 0, "{report}");
    assert_eq!(report["head_authenticity"], "refuted", "{report}");
    assert!(
        report["head_invalid"]
            .as_str()
            .is_some_and(|why| why.contains("witness")),
        "the refusal names which of the two signatures failed: {report}"
    );
}

/// 🔴 **S-3 — a head written before this release still opens.**
///
/// The compatibility arm, and it is not a formality: R7 added two fields and a second signature, and
/// a project written by R6 has neither. Refusing those would make this release unable to open the
/// projects the last one wrote — the failure `req/229` §7-4 warns about in the other direction — so
/// an R6-era head is `unverified` about its local numbers and **not** refused. Built by stripping
/// the R7 fields from a genuine head, which is exactly what an R6 binary would have written.
#[test]
fn s3_a_head_written_before_this_release_still_opens() {
    let fixture = three_commits("r7_s3_r6_head");
    edit_head(&fixture, |head| {
        let object = head.as_object_mut().expect("the head is an object");
        object.remove("witness_signature");
        object.remove("version_digest");
        object.remove("ledger_leaf_hash");
        object.remove("accepted_rollback");
    });

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        code, 0,
        "an R6 project opens on an R7 binary, exactly as it did: {report}"
    );
    assert_eq!(
        report["head_authenticity"], "verified",
        "its checkpoint still verifies — that half is unchanged: {report}"
    );
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (health, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "{body}");
    shut_down(server);
}

/// 🔴 **S-4 — the new gates do not fire on a project nobody attacked.**
///
/// Every other arm attacks. This one attacks the **false positive**, because a monotonicity check
/// that fires by being too sensitive takes a healthy project offline, which is worse than the
/// finding it was built for. Three rounds of server-then-CLI writing in turn, with `gx repair`
/// between each — and the declaration digest, the witness signature and the signing lock all have
/// to survive a `.gx/VERSION` that gets stamped, a journal that grows and a head that is rewritten
/// at every commit.
#[test]
fn s4_a_server_and_a_cli_writing_in_turn_do_not_trip_the_new_gates() {
    let fixture = pipeline("r7_s4_alternating", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    for round in 0..3 {
        let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
        server.commit_over_http(&locator, &format!("http-{round}\n"), &fixture.key_id);
        let (health, body) = server.request("GET", "/v1/healthz", None);
        assert_eq!(health, 200, "round {round}: {body}");
        let (checkpoint, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
        assert_eq!(checkpoint, 200, "round {round}: {checkpoint_body}");
        shut_down(server);

        assert!(
            !fixture.commit_one(&format!("cli-{round}\n")).is_empty(),
            "round {round}: a CLI commit lands beside the server's"
        );
        let (code, report) = repair_report(&fixture, false);
        assert_eq!(code, 0, "round {round}: {report}");
        assert_eq!(report["rolled_back"], serde_json::Value::Null, "{report}");
        assert_eq!(report["head_authenticity"], "verified", "{report}");
    }
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (health, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(health, 200, "{body}");
    shut_down(server);
}

/// 🔴 **S-5 — the escape hatch cannot be opened with somebody else's key.**
///
/// `--accept-rollback` is the one road R7 adds onto a shorter tree, so it is the one an attacker
/// would rather have than any of the ones this lane closed. It demands a checkpoint from outside
/// the project — and a checkpoint from outside is not the same thing as a checkpoint from *another
/// project*, which is `req/232` M-04's finding pointed at this lane's own new verb.
#[test]
fn s5_accept_rollback_refuses_evidence_from_another_project() {
    let fixture = pipeline("r7_s5_accept_foreign", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    server.commit_over_http(&locator, "two\n", &fixture.key_id);
    shut_down(server);
    let journal_at_two = std::fs::metadata(journal_path(&fixture))
        .expect("stat")
        .len();
    let ledger_at_two = std::fs::metadata(ledger_path(&fixture))
        .expect("stat")
        .len();
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "three\n", &fixture.key_id);
    shut_down(server);
    truncate_at(&journal_path(&fixture), journal_at_two);
    truncate_at(&ledger_path(&fixture), ledger_at_two);

    let others = three_commits("r7_s5_other_project");
    let foreign = others.home.join("other-head.json");
    assert_eq!(export_head(&others, &foreign).0, 0);

    let (code, report, stderr) = repair_accepting(&fixture, &foreign);
    assert_ne!(code, 0, "{report} {stderr}");
    assert_eq!(
        report["accepted_rollback"],
        serde_json::Value::Null,
        "no floor is moved on another project's document: {report}"
    );
    assert_eq!(report["against"]["foreign"], true, "{report}");
    let (still, after) = repair_report(&fixture, false);
    assert_ne!(still, 0, "and the project is still refused: {after}");
    assert!(after["rolled_back"].is_string(), "{after}");
}
