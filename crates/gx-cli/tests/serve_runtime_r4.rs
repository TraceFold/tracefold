// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R4 — the three accidents the fourth adversarial audit measured, plus the falsifiers this
//! lane owes for its own new code** (`req/225`, `req/38` §163 ruling 2).
//!
//! `req/225` §9's implementation row is the reason this file exists, and it named the arms before
//! they did: red if the byte count of a ledger moves across `gx repair` without `--yes`; red if
//! `gx undo` is not `0` on a project whose engine key is not its actor key; red if
//! `POST /candidates` answers `201` after the journal was rewritten at the same length. All three
//! were red the day it wrote that. (The audit's own sentences are in `req/225` and stay there:
//! this crate's public face carries no Japanese, `req/38` §121.)
//!
//! # The second half, and why it is half the file
//!
//! Four adversarial audits in a row found their highest-severity items in the **previous lane's
//! repair** — R1b's ledger detector became `req/222` H-05, and R3's `gx repair`, witness key and
//! one-sided detector became `req/225` H-01, H-02 and H-03. `req/38` §163 ruling 1 makes the
//! answer a standing part of a repair lane's definition of done: attack your own new code, from a
//! door the repair's own probes do not use, and show the red.
//!
//! So the probes below come in two kinds and are labelled as such. `h01_`/`h02_`/`h03_` are the
//! audit's three. `s1_`..`s5_` are this lane's attacks on what this lane wrote: the whole `.gx/`
//! tree rather than two files; a project with no journal rather than a damaged one; a journal that
//! **shrank** rather than one rewritten at the same length; a key that is **absent** rather than
//! wrong; and a server and a CLI writing side by side, which is the one door where a new detector
//! fails by being *too* sensitive rather than not sensitive enough.
//!
//! `cfg(unix)` for its predecessors' reasons — `SIGTERM`, `flock` — and with the same declaration:
//! Windows, WSL 9p and a synchronising client are **not measured** (`req/213` §7(d), unchanged by
//! this lane).

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
/// The fifth copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test binary
/// is its own crate.
struct Serving {
    child: Child,
    addr: String,
    token: String,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Serving {
    fn start(project: &Path, home: &Path, key_id: &str) -> Self {
        let token = "r4-runtime-token".to_string();
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

    /// `POST /candidates`, answering the id it minted.
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

fn ledger_path(fixture: &Pipeline) -> PathBuf {
    layout(fixture).ledger_path()
}

fn journal_path(fixture: &Pipeline) -> PathBuf {
    layout(fixture).journal_path()
}

/// The commit receipt `.gx/receipts/` holds for one transformation.
fn receipt_path(fixture: &Pipeline, id: &str) -> PathBuf {
    layout(fixture)
        .join("receipts")
        .join(format!("{}.commit.json", id.replace(':', "_")))
}

/// A second key in the same store — the **engine's**, distinct from the fixture's actor key.
fn second_key(fixture: &Pipeline) -> String {
    let generated = support::run(fixture.gx().args(["key", "gen", "--json"]));
    assert_eq!(generated.code, 0, "a second key: {}", generated.stderr);
    generated.json()["key_id"]
        .as_str()
        .expect("44 §1.2's `gx key gen` prints a key_id")
        .to_string()
}

/// Flip one bit at `at`, leaving the file's length exactly as it was.
fn flip_bit(path: &Path, at: usize) -> u64 {
    let mut bytes = std::fs::read(path).expect("read the file");
    assert!(at < bytes.len(), "offset {at} is past the end of {path:?}");
    bytes[at] ^= 0x01;
    std::fs::write(path, &bytes).expect("write the file back");
    bytes.len() as u64
}

/// Every file under `.gx/`, by path, with its bytes — **except the lock**.
///
/// 🔴 The exception is declared rather than quiet. `.gx/LOCK` is req/56 §2's `Nature::Transient`
/// row and `ProcessLock::take` writes a pid and a verb into it for a human reading it later; a
/// verb that took the lock and left the note unchanged would be a verb whose lock nobody can
/// attribute. So "writes nothing" in this file means "writes nothing that is a record", and the
/// one thing it does write is the note saying it was here.
fn tree_of(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name().is_some_and(|n| n == "LOCK") {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            out.push((relative, std::fs::read(&path).unwrap_or_default()));
        }
    }
    out.sort();
    out
}

/// The names in a `.gx/` tree, for a message that fits on a line.
fn names(tree: &[(String, Vec<u8>)]) -> Vec<String> {
    tree.iter()
        .map(|(name, bytes)| format!("{name}:{}", bytes.len()))
        .collect()
}

/// A project whose ledger has one bit flipped in the middle, and whose journal is untouched.
///
/// The middle rather than the tail on purpose: a tail tear is what R3's probes already fire, and
/// this file needs the shape that reaches `gx repair` — a ledger the writer's door will quarantine
/// **and truncate to nothing**, because the damage is in front of every leaf.
fn a_project_whose_ledger_is_damaged(name: &str) -> Pipeline {
    let fixture = pipeline(name, "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    for round in 0..3 {
        server.commit_over_http(&locator, &format!("leaf-{round}\n"), &fixture.key_id);
    }
    shut_down(server);
    let ledger = ledger_path(&fixture);
    let len = flip_bit(&ledger, 40);
    println!("LEDGER_DAMAGED at=40 len={len}");
    fixture
}

// ---------------------------------------------------------------------------
// H-01 — the door that broke what it was called to describe
// ---------------------------------------------------------------------------

/// 🔴 **`req/225` H-01, reproduced before the repair** — `gx repair` without `--yes` took a
/// 522-byte ledger to **0 bytes**.
///
/// 44 §1.2 says that without `--yes` this verb writes not one byte, `gx repair --help` said it
/// found", and `repair.rs`'s module documentation said "`--yes` is required before anything is
/// written". The code opened the writer's door — which quarantines a tail that will not replay and
/// then cuts it (DR-43-7) — **before** it looked at the flag. On the one class of project that
/// ever reaches this verb, the diagnosis was an amputation: `gx repair` cannot rebuild a lost leaf
/// and says so two paragraphs later in its own report.
///
/// The assertion is on **bytes**, not on an exit status: a report is allowed to be any shape it
/// likes as long as the project it reports on is the project it found.
#[test]
fn h01_a_repair_report_does_not_move_a_byte() {
    let fixture = a_project_whose_ledger_is_damaged("r4_repair_readonly");
    let ledger = ledger_path(&fixture);
    let journal = journal_path(&fixture);
    let ledger_before = std::fs::read(&ledger).expect("read the ledger");
    let journal_before = std::fs::read(&journal).expect("read the journal");
    println!(
        "BEFORE ledger={} journal={}",
        ledger_before.len(),
        journal_before.len()
    );

    let reported = support::run(fixture.gx().arg("repair"));
    println!(
        "REPAIR_REPORT exit={} stdout={}",
        reported.code,
        reported.stdout.trim()
    );

    let ledger_after = std::fs::read(&ledger).expect("read the ledger");
    let journal_after = std::fs::read(&journal).expect("read the journal");
    println!(
        "AFTER  ledger={} journal={}",
        ledger_after.len(),
        journal_after.len()
    );
    assert_eq!(
        ledger_before,
        ledger_after,
        "🔴 `req/225` H-01: this report took the ledger from {} bytes to {}. The verb that exists \
         so a damaged project has a way out made it worse by being asked what was wrong",
        ledger_before.len(),
        ledger_after.len()
    );
    assert_eq!(
        journal_before, journal_after,
        "and the journal is the other half of the same promise"
    );
    let torn: Vec<_> = std::fs::read_dir(layout(&fixture).join("ledger"))
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.contains(".torn."))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        torn.is_empty(),
        "and a quarantine copy is a write too — DR-43-7 makes one immediately before a `set_len`, \
         so its presence is proof the truncation happened: {torn:?}"
    );
}

/// 🔴 **`req/225` H-01, probe F4e** — the same diagnosis, beside a live `gx serve`.
///
/// The audit ran `gx repair --json` next to a healthy running server and watched `/healthz` go
/// from `200` to `500` and every write with it: the report had cut the ledger the server was
/// answering from. 44 §1.2's other sentence — "if a live `gx serve` is there, `BUSY`, stop it
/// first" — did not save it, because DR-43-2 makes `.gx/LOCK` per-operation so that a server and a
/// CLI can share a project, and an idle server holds nothing.
///
/// The damage here is in the **middle** of the ledger, which 43 §7.5 (j) declares a read cannot
/// see. That is what makes this probe about `gx repair` rather than about the detector: the server
/// is answering `200` before and must still be answering `200` after, because nothing that
/// happened in between was a write.
#[test]
fn h01_a_repair_report_beside_a_live_server_leaves_it_healthy() {
    let fixture = pipeline("r4_repair_live", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    server.commit_over_http(&locator, "two\n", &fixture.key_id);

    let ledger = ledger_path(&fixture);
    let len = flip_bit(&ledger, 40);
    println!("LEDGER_DAMAGED at=40 len={len}");

    let (before_status, before_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_BEFORE status={before_status} body={before_body}");
    assert_eq!(
        before_status, 200,
        "the fixture depends on a read not seeing a mid-file rewrite (43 §7.5 (j)): {before_body}"
    );

    let ledger_before = std::fs::read(&ledger).expect("read the ledger");
    let reported = support::run(fixture.gx().arg("repair"));
    println!(
        "REPAIR_BESIDE_A_SERVER exit={} stdout={}",
        reported.code,
        reported.stdout.trim()
    );
    let ledger_after = std::fs::read(&ledger).expect("read the ledger");

    let (after_status, after_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER status={after_status} body={after_body}");
    assert_eq!(
        ledger_before.len(),
        ledger_after.len(),
        "🔴 `req/225` H-01 probe F4e: the diagnosis truncated the ledger the live server is \
         serving from"
    );
    assert_eq!(
        after_status, 200,
        "and the server that was healthy before somebody asked what was wrong is still healthy \
         after: {after_body}"
    );
    shut_down(server);
}

/// The other half of the same promise: `--yes` **does** repair, so the door still works.
///
/// Without this arm, `h01_a_repair_report_does_not_move_a_byte` would pass on a build where
/// `gx repair` had been reduced to a no-op — which is the shape of the wrong fix, and exactly the
/// sort of thing `req/38` §163's DoD is for.
#[test]
fn h01_only_yes_moves_the_files() {
    let fixture = a_project_whose_ledger_is_damaged("r4_repair_yes");
    let ledger = ledger_path(&fixture);
    let before = std::fs::read(&ledger).expect("read the ledger");

    let ran = support::run(
        fixture
            .gx()
            .arg("repair")
            .arg("--yes")
            .args(["--signing-key", &fixture.key_id]),
    );
    println!("REPAIR_YES exit={} stdout={}", ran.code, ran.stdout.trim());
    let after = std::fs::read(&ledger).expect("read the ledger");
    println!("YES ledger before={} after={}", before.len(), after.len());
    assert_ne!(
        before.len(),
        after.len(),
        "`--yes` takes the writer's door, and the writer's door removes a tail that will not \
         replay (DR-43-7). If this is equal, the repair has become a report"
    );
    let torn: Vec<_> = std::fs::read_dir(layout(&fixture).join("ledger"))
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.contains(".torn."))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !torn.is_empty(),
        "and it copies before it cuts, so the bytes are still somewhere: {torn:?}"
    );
}

// ---------------------------------------------------------------------------
// H-02 — the receipt verified against a key that never signed it
// ---------------------------------------------------------------------------

/// 🔴 **`req/225` H-02, reproduced before the repair** — on a project whose engine key is not its
/// actor key, **every** `gx undo` was refused, permanently.
///
/// `Session::signing_key` answers with the row's **actor**'s key and `settle_preflight` verified
/// the archived commit receipt's DSSE signature against it. A receipt signed by `gx serve
/// --signing-key <ENGINE>` — the deployment 44 §1.2 and E-M6-7 describe as ordinary, and the one
/// a GUI runs — is signed by the engine's key, so the check failed on an authentic document and
/// said "does not verify under this project's key … so it is not evidence of anything". The undo
/// button could not be drawn, not because gx fired without checking but because it never fired.
///
/// The single-key fixtures every other suite uses cannot see this: they pass because the two keys
/// are the same key.
#[test]
fn h02_an_undo_verifies_the_receipt_under_the_key_that_signed_it() {
    let fixture = pipeline("r4_two_keys", "before\n");
    let locator = fixture.target.display().to_string();
    let engine_key = second_key(&fixture);
    println!("ACTOR_KEY={} ENGINE_KEY={engine_key}", fixture.key_id);
    assert_ne!(fixture.key_id, engine_key, "the fixture needs two keys");
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &engine_key);
    let id = server.commit_over_http(&locator, "after\n", &fixture.key_id);
    assert_eq!(fixture.target_contents(), "after\n");
    shut_down(server);

    assert!(
        receipt_path(&fixture, &id).is_file(),
        "the fixture depends on the receipt being archived: {:?}",
        receipt_path(&fixture, &id)
    );
    let shown = support::run(
        fixture
            .gx()
            .args(["receipt", "show"])
            .arg(&id)
            .args(["--level", "4"]),
    );
    let named = shown.json()["key_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    println!("RECEIPT_KEY_ID={named}");
    assert_eq!(
        named, engine_key,
        "the fixture depends on the server signing with the engine's key, not the actor's"
    );

    let undone = support::run(fixture.gx().arg("undo").arg(&id).args(["--settle", "0"]));
    println!(
        "CLI_UNDO_UNDER_TWO_KEYS exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert_eq!(
        undone.code, 0,
        "🔴 `req/225` H-02: this was **3** for every committed row on this project, for ever. The \
         CAS was reading the actor's key for a document the engine signed: {}",
        undone.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "and the world really is back"
    );
}

/// 🔴 **`req/225` H-02, the same root from the other side** — rotating the engine key used to kill
/// the undo of every commit already made.
///
/// `undo_witness` on the HTTP face verified against `ServerKeys::signing()`, which is the key this
/// **process** signs with. A receipt issued yesterday names yesterday's key, so one
/// `gx key rotate` turned the whole history into bad signatures. The key now comes from the
/// receipt (42 §3.10 puts `key_id` inside the signed payload for exactly this), resolved against
/// req/56 §3's store.
#[test]
fn h02_a_rotated_engine_key_does_not_invalidate_an_old_receipt() {
    let fixture = pipeline("r4_rotate", "before\n");
    let locator = fixture.target.display().to_string();
    let first_engine_key = second_key(&fixture);
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &first_engine_key);
    let id = server.commit_over_http(&locator, "after\n", &fixture.key_id);
    shut_down(server);

    // The rotation: a third key, and a server that signs with it from now on.
    let next_engine_key = second_key(&fixture);
    assert_ne!(first_engine_key, next_engine_key);
    println!("ROTATED from={first_engine_key} to={next_engine_key}");
    let server = Serving::start(&fixture.project, &fixture.home, &next_engine_key);
    let (status, body) = server.request(
        "POST",
        &format!("/v1/transformations/{id}/undo"),
        Some(&serde_json::json!({})),
    );
    println!("UNDO_AFTER_ROTATION status={status} body={body}");
    assert_eq!(
        status, 200,
        "🔴 `req/225` H-02: a key rotation used to refuse the undo of every commit made before it, \
         because the receipt was checked against the key the process holds **today**: {body}"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).expect("json")["witness"],
        serde_json::json!("attested"),
        "and it is attested, not waved through: {body}"
    );
    assert_eq!(fixture.target_contents(), "before\n");
    shut_down(server);
}

// ---------------------------------------------------------------------------
// H-03 — the detector that was on one of the two files
// ---------------------------------------------------------------------------

/// 🔴 **`req/225` H-03, reproduced before the repair** — one bit in a live journal's last record,
/// and the server went on writing and signing.
///
/// R3 gave the ledger a tail-record check and an unconditional re-open under the lock, and
/// `req/219` §5(h) had already written down that gx's durable state is a **pair**. The pair had
/// one detector. Measured by the audit: `/healthz` `200 ledger_agrees:true`, `POST /candidates`
/// `201`, `GET /ledger/checkpoint` `200` **signed** — and the next start-up refusing to open the
/// project because its journal witnessed two commits its ledger held one leaf for.
///
/// The flip is in the last record and leaves the length exactly, which is the shape `catch_up`'s
/// offset arithmetic is blind to by construction.
#[test]
fn h03_a_same_length_rewrite_of_the_journal_stops_the_writing() {
    let fixture = pipeline("r4_journal_tail", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);

    let (ok_status, ok_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_BEFORE status={ok_status} body={ok_body}");
    assert_eq!(ok_status, 200, "the control: {ok_body}");

    let journal = journal_path(&fixture);
    let len = std::fs::metadata(&journal).expect("the journal").len() as usize;
    let at = len - 3;
    flip_bit(&journal, at);
    println!(
        "JOURNAL_FLIPPED at={at} len_before={len} len_after={}",
        std::fs::metadata(&journal).expect("the journal").len()
    );

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 `req/225` H-03: a monitor read `ok` from a server whose journal had been rewritten \
         under it: {health_body}"
    );
    assert!(
        health_body.contains("LEDGER_DISAGREES"),
        "the same word every other face uses for two files that describe different trees \
         (req/38 §156 ruling 2(a)): {health_body}"
    );
    assert!(
        health_body.contains("journal is the file that moved"),
        "and it says **which** file, because an operator sent to the wrong one loses the hour: \
         {health_body}"
    );

    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": locator,
        "goal": "two\n",
        "context": "Evidence",
        "actor": { "Human": { "key": fixture.key_id } },
    });
    let (create_status, create_body) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("CREATE_AFTER_FLIP status={create_status} body={create_body}");
    assert_ne!(
        create_status, 201,
        "and the write is refused rather than appended on top of a journal nobody can replay: \
         {create_body}"
    );

    let (checkpoint_status, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    println!("CHECKPOINT_AFTER_FLIP status={checkpoint_status} body={checkpoint_body}");
    assert_eq!(
        checkpoint_status, 500,
        "and nothing is signed over a tree this server's own journal no longer describes: \
         {checkpoint_body}"
    );
    shut_down(server);
}

/// 🔴 **`req/225` H-03, from the middle of the file** — the half a read cannot have, and the half
/// a write must.
///
/// The audit's probe E2b flipped offset 40 of a 1,636-byte journal: the live server carried on,
/// and the next start-up quarantined **the whole file**. The tail check cannot see this (it looks
/// at the last record), so the writer's road re-replays the whole consumed prefix under the lock —
/// the same trade 43 §7.5 already accepted for the ledger.
///
/// ~~What this probe **also** asserts is the limit, out loud: `/healthz` still answers `200`, exactly
/// as 43 §7.5 (j) declares for the ledger's twin of this case. A probe that quietly skipped that
/// would be hiding the denominator.~~
///
/// 🔴 **R5 / `req/227` H-01 — the limit is closed and this assertion is its opposite now.**
///
/// The struck paragraph is kept because it was true and because a reader who remembers it has to
/// be able to find out when it stopped being true. `req/227` fired through the door it declared:
/// a live server answering `200` about a journal it can no longer replay is a server that goes on
/// signing checkpoints over rewritten bytes, and where the rewritten record is a `Committed` the
/// next start-up re-applies its delta to the substrate. DR-43-9's per-record chain makes the check
/// a hash walk with no CBOR decode, which is cheap enough for the read road, so the read road asks
/// it. `serve_runtime_r5.rs` carries the whole class; this line is the one assertion in this file
/// that had to turn over.
#[test]
fn h03_a_rewrite_in_the_middle_of_the_journal_stops_the_next_write() {
    let fixture = pipeline("r4_journal_middle", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);

    let journal = journal_path(&fixture);
    flip_bit(&journal, 40);
    println!("JOURNAL_FLIPPED at=40");

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("MIDFLIP_HEALTHZ status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 **R5**: this line asserted `200` until DR-43-9 — the declared limit that a lockless \
         read looks at the tail record and at the length, and that a middle byte moves neither \
         (43 §7.5 (j)). `req/227` H-01 measured the accident on the other side of that limit, so \
         the read road now verifies the chain: {health_body}"
    );

    let intent = serde_json::json!({
        "substrate": "fs",
        "locator": locator,
        "goal": "two\n",
        "context": "Evidence",
        "actor": { "Human": { "key": fixture.key_id } },
    });
    let (create_status, create_body) = server.request("POST", "/v1/candidates", Some(&intent));
    println!("MIDFLIP_CREATE status={create_status} body={create_body}");
    assert_ne!(
        create_status, 201,
        "🔴 the half that is a guarantee: under `.gx/LOCK` the whole consumed prefix is replayed \
         again, so a write is refused instead of landing on top of bytes that will not come back: \
         {create_body}"
    );
    shut_down(server);
}

// ---------------------------------------------------------------------------
// This lane's attacks on this lane's own code (`req/38` §163 ruling 1)
// ---------------------------------------------------------------------------

/// 🔴 **S-1 — the whole `.gx/` tree, not the two files H-01 looks at.**
///
/// `h01_a_repair_report_does_not_move_a_byte` compares the journal and the ledger, which is what
/// the audit measured. The obvious way for a fix to pass that and still be wrong is to move the
/// writing somewhere else: a quarantine copy beside the ledger, a `.verdicts` chain created on the
/// way past, a blob directory, an index. So this arm reads **every file under `.gx/`** before and
/// after, by name and by bytes, and asserts the two lists are equal.
///
/// The one exclusion is `.gx/LOCK`, and it is in `tree_of`'s documentation rather than here.
#[test]
fn s1_a_repair_report_changes_nothing_anywhere_under_dotgx() {
    let fixture = a_project_whose_ledger_is_damaged("r4_s1_tree");
    let root = layout(&fixture).root().to_path_buf();
    let before = tree_of(&root);
    println!("TREE_BEFORE={:?}", names(&before));

    let reported = support::run(fixture.gx().arg("repair").arg("--json"));
    println!(
        "S1_REPAIR exit={} stdout={}",
        reported.code,
        reported.stdout.trim()
    );

    let after = tree_of(&root);
    println!("TREE_AFTER={:?}", names(&after));
    assert_eq!(
        names(&before),
        names(&after),
        "🔴 a verb that reports must leave the same files with the same lengths. Anything new here \
         is a write the report performed and did not mention"
    );
    assert_eq!(
        before, after,
        "and the same bytes: equal names and equal lengths is not equal content"
    );
}

/// 🔴 **S-2 — a project with no journal, rather than one with a damaged journal.**
///
/// H-01's door is "the writer's door repairs what the report was called to describe". The same
/// door has a second mouth that the audit did not fire: `Engine::open` **creates** the three
/// append-only files if they are absent. So `gx repair` on a `.gx/` whose journal has been moved
/// away used to leave three brand-new empty files behind and then report the project healthy —
/// a diagnosis that manufactures the thing it is diagnosing.
///
/// The absence is produced by renaming rather than deleting, so the probe is about `gx repair` and
/// not about a fixture that lost a file.
#[test]
fn s2_a_repair_report_on_a_project_with_no_journal_creates_nothing() {
    let fixture = pipeline("r4_s2_absent", "before\n");
    assert_eq!(fixture.submit("one\n").code, 0);
    let journal = journal_path(&fixture);
    let moved = journal.with_extension("moved");
    std::fs::rename(&journal, &moved).expect("move the journal aside");
    assert!(!journal.exists());

    let reported = support::run(fixture.gx().arg("repair").arg("--json"));
    println!(
        "S2_REPAIR exit={} stdout={} stderr={}",
        reported.code,
        reported.stdout.trim(),
        reported.stderr.trim()
    );
    println!("S2_JOURNAL_EXISTS_AFTER={}", journal.exists());
    assert!(
        !journal.exists(),
        "🔴 a report that creates a journal has invented the project it was asked about. \
         `gx submit` makes one; this verb does not"
    );
}

/// 🔴 **S-3 — a journal that shrank, rather than one rewritten at the same length.**
///
/// H-03 fires the same-length rewrite the audit measured. The neighbouring shape is a journal that
/// got **shorter**, and it has its own history: `req/222` M-01 measured `/healthz` answering `200
/// ledger_agrees:true` and `GET /ledger/checkpoint` answering `200` **signed** over a journal cut
/// in half, and `req/225` §1-4 found it still real. `EngineJournal::catch_up_unlocked` returns
/// quietly for a short file — correctly, because a lockless reader cannot tell damage from a race
/// — and nothing downstream asked again.
///
/// It is the same detector's third question, so it is this lane's code being attacked from a door
/// H-03 does not use.
#[test]
fn s3_a_journal_that_shrank_stops_being_healthy() {
    let fixture = pipeline("r4_s3_shrunk", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);

    let (ok_status, _) = server.request("GET", "/v1/healthz", None);
    assert_eq!(ok_status, 200, "the control");

    let journal = journal_path(&fixture);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let half = bytes.len() / 2;
    std::fs::write(&journal, &bytes[..half]).expect("cut the journal in half");
    println!("JOURNAL_CUT from={} to={half}", bytes.len());

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("S3_HEALTHZ status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 `req/222` M-01, still real at `req/225`: an append-only log cannot shrink, and a \
         monitor was reading `ok` from a server whose log had: {health_body}"
    );

    let (checkpoint_status, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    println!("S3_CHECKPOINT status={checkpoint_status} body={checkpoint_body}");
    assert_eq!(
        checkpoint_status, 500,
        "and it signs nothing over a tree whose journal is not there any more: {checkpoint_body}"
    );
    shut_down(server);
}

/// 🔴 **S-4 — a key that is absent, rather than a key that is wrong.**
///
/// H-02 changes which key a receipt is verified against. The attack on that change is not "use a
/// different key" — that is what it fixes — but "make the lookup fail" and see what the failure is
/// called. A receipt naming a key req/56 §3's store does not hold cannot have its signature
/// checked at all, and the two wrong answers available are both bad: attesting it would be
/// DR-43-1's whole gate off again, and calling it a bad signature would send an operator looking
/// for tampering that did not happen.
///
/// So it is a refusal **with its own name**, and the sentence is what this probe asserts. Before
/// this lane there was no such name — the same situation produced the "does not verify under this
/// project's key" line, which is the accusation.
#[test]
fn s4_a_receipt_naming_a_key_the_store_does_not_hold_is_refused_by_name() {
    let fixture = pipeline("r4_s4_nokey", "before\n");
    let locator = fixture.target.display().to_string();
    let engine_key = second_key(&fixture);
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &engine_key);
    let id = server.commit_over_http(&locator, "after\n", &fixture.key_id);
    shut_down(server);

    let key_file = fixture
        .home
        .join(".gx")
        .join("keys")
        .join(format!("{engine_key}.key"));
    assert!(key_file.is_file(), "the fixture depends on {key_file:?}");
    std::fs::remove_file(&key_file).expect("take the engine key out of the store");
    println!("KEY_REMOVED={}", key_file.display());

    let refused = support::run(fixture.gx().arg("undo").arg(&id).args(["--settle", "0"]));
    println!(
        "S4_UNDO exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(
        refused.code, 3,
        "44 §1.4's 3 — the number a CAS that did not pass already had: {}",
        refused.stderr
    );
    assert!(
        refused.stderr.contains("holds no key of that id"),
        "🔴 and it names the absence rather than accusing the document. Before R4 this said the \
         receipt \"does not verify under this project's key\", which is the sentence for tampering: \
         {}",
        refused.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "and the world is untouched, because the undo was refused"
    );
}

/// 🔴 **S-5 — the door where a new detector fails by being too sensitive.**
///
/// Every other probe in this file attacks the detectors for missing something. This one attacks
/// them for firing when nothing is wrong, which is the failure mode a byte-comparison detector
/// actually has: `EngineJournal`'s tail is recorded at the offset an append is *expected* to land
/// at, and a server and a CLI writing to one project alternate appends between two processes. If
/// the recorded offset and the file ever disagree, `tail_unchanged` answers `false` for a healthy
/// project, every write becomes `LEDGER_DISAGREES`, and R2's whole co-residency claim dies.
///
/// So: a server and a CLI commit to the same project in turn, three rounds, and the assertion is
/// that everything succeeds and `/healthz` never leaves `200`. This one was **green** before the
/// lane as well — it is a loss condition rather than a reproduction, and it is here because it is
/// the arm most likely to catch a wrong version of this repair.
#[test]
fn s5_a_server_and_a_cli_writing_in_turn_do_not_trip_the_new_detector() {
    let fixture = pipeline("r4_s5_together", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    for round in 0..3 {
        let goal = format!("http-{round}\n");
        server.commit_over_http(&locator, &goal, &fixture.key_id);
        let (health, body) = server.request("GET", "/v1/healthz", None);
        println!("S5_HEALTHZ_AFTER_HTTP round={round} status={health} body={body}");
        assert_eq!(health, 200, "after an HTTP commit: {body}");

        let submitted = fixture.submit(&format!("cli-{round}\n"));
        println!(
            "S5_CLI round={round} exit={} stderr={}",
            submitted.code,
            submitted.stderr.trim()
        );
        assert_eq!(
            submitted.code, 0,
            "🔴 a CLI write beside a live server is R2's claim, and a detector that answered \
             `false` for our own appends would end it: {}",
            submitted.stderr
        );
        let (health, body) = server.request("GET", "/v1/healthz", None);
        println!("S5_HEALTHZ_AFTER_CLI round={round} status={health} body={body}");
        assert_eq!(health, 200, "after a CLI commit: {body}");
    }
    shut_down(server);
}
