// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R6 — monotonicity, the downgrade, and the artefact that has to leave the machine**
//! (`req/229`, `req/38` §167).
//!
//! # What the sixth adversarial audit measured, in one sentence
//!
//! R5 gave the journal an **identity** and did not give the pair a **monotonicity**. A chain is
//! prefix-closed — the first `i` records of a chained journal are themselves a perfectly chained
//! journal — and a ledger is a sequence of framed leaves, so its prefixes are perfect ledgers.
//! ∴ an attacker who can write to both files needs no hash, no codec and no key: `truncate(2)` at
//! two frame boundaries produces a pair that agrees with itself and with every gate. `gx repair`
//! answered exit 0 with `journal_intact: true` and `remedy: null`, `gx serve` started, `/healthz`
//! answered `200`, `GET /ledger/checkpoint` answered **signed** — and where the cut fell between a
//! `ledger.append` and its `Committed` record, 43 §7-3b's recovery re-applied the surviving
//! commit's delta and took the operator's file from `three` back to `two`, with
//! `recover.refused: 0` on the start-up line.
//!
//! The second high finding is the mirror image: the chain can be taken off **from the attacker's
//! side**. Strip the marker and the links, and `gx repair` says `legacy`, `journal_intact: true`,
//! exit 0; `gx serve` starts without one word of warning; and `req/227` H-01(a) — the rewrite the
//! chain exists to catch — works again on the downgraded file, live, with a signed checkpoint over
//! it.
//!
//! # The repairs, and why there are three
//!
//! **(a) DR-43-11, the persisted head.** Every commit writes a signed `Checkpoint` plus the
//! journal's length and chain head to `.gx/checkpoints/head.json`, atomically. Open, catch-up and
//! `gx repair` compare the project in front of them with that floor and refuse a shorter or
//! divergent one — through `ledger_agrees`, so every existing gate inherits it, with no new
//! `gx_code` and no new exit status.
//!
//! **(b) The declaration.** `.gx/VERSION` records the journal's framing, and a project that
//! declares `chained` and presents a file with no marker is refused exactly as a chain break is:
//! not cut, not quarantined, not appended to.
//!
//! **(c) DR-43-10 minimal, `gx checkpoint export`.** (a) and (b) both live inside the attacker's
//! write scope. `req/229` §7-4 is the sentence this lane cannot argue with: *the artefact an
//! auditor should hold is not in `.gx/` at all.* So the signed head can be copied out with no key,
//! and `gx repair --against <FILE>` reads it back. The probes below assert both what that catches
//! **and what it does not** (`s1_`, `s2_`) — a lane that measured only its own successes would be
//! repeating the failure five audits in a row have found in the previous lane's repair.
//!
//! `cfg(unix)` for its predecessors' reasons — `SIGTERM`, `flock`, `chmod` — and with the same
//! declaration: Windows, WSL 9p and a synchronising client are **not measured** (`req/213` §7(d),
//! carried unchanged for a tenth lane).

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
/// The seventh copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test
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
        let token = "r6-runtime-token".to_string();
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

    /// The status of a `POST /candidates` that is expected to be refused.
    fn create_status(&self, locator: &str, goal: &str, actor_key: &str) -> (u16, String) {
        let intent = serde_json::json!({
            "substrate": "fs",
            "locator": locator,
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": actor_key } },
        });
        self.request("POST", "/v1/candidates", Some(&intent))
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

// ---------------------------------------------------------------------------
// Reading both files' framing from outside the engine
// ---------------------------------------------------------------------------

/// Every record frame in a journal file, as `(offset, framed_length)`.
///
/// Written without the engine's constants for `serve_runtime_r5.rs`'s reason: the same walk has to
/// read a chained file and a legacy one.
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
///
/// 🔴 The whole of `req/229` H-01's toolkit. There is nothing else in it.
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

/// Copy the framed bytes of record `from` over record `onto`, which must be the same length.
fn copy_record_over(path: &Path, onto: usize, from: usize) -> usize {
    let mut bytes = std::fs::read(path).expect("read the journal");
    let spans = frames(&bytes);
    let (onto_at, onto_len) = spans[onto];
    let (from_at, from_len) = spans[from];
    assert_eq!(onto_len, from_len, "the substitution keeps every length");
    let donor = bytes[from_at..from_at + from_len].to_vec();
    bytes[onto_at..onto_at + onto_len].copy_from_slice(&donor);
    std::fs::write(path, &bytes).expect("write the journal back");
    onto_at
}

/// Take the marker and every link off a chained journal, leaving the old framing.
///
/// `req/229` H-02's toolkit, and no more of one than H-01's: every byte written is a byte the file
/// already held.
///
/// 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the entry check asks the engine which
/// framing the file is in instead of spelling `JOURNAL_MAGIC`. A journal this build creates carries
/// `GXJRNL02`, so the v1 spelling had become false of every fixture handed to this helper and the
/// helper panicked before it downgraded anything. What did **not** change: the file still has to
/// carry a chained marker **at offset zero** — the check is `starts_with` against whichever marker
/// the sniffed format names — so a legacy file and an empty one are refused here exactly as before
/// and no probe can believe it downgraded a journal that was already legacy. The contract is the one
/// H-02 wrote — a legacy journal out of a chained one, every payload untouched. The body needs no
/// version: both markers are eight bytes and share the four bytes `frames` sniffs on.
fn strip_the_chain(path: &Path) -> (u64, u64) {
    let bytes = std::fs::read(path).expect("read the journal");
    assert!(
        gx_engine::replay(&bytes)
            .format()
            .marker()
            .is_some_and(|marker| bytes.starts_with(marker)),
        "the fixture is a chained journal to begin with"
    );
    let mut legacy = Vec::new();
    for (at, len) in frames(&bytes) {
        legacy.extend_from_slice(&bytes[at..at + len - 32]);
    }
    std::fs::write(path, &legacy).expect("write the legacy journal");
    println!("DOWNGRADED {} -> {}", bytes.len(), legacy.len());
    (bytes.len() as u64, legacy.len() as u64)
}

/// Every `<file>.torn.<n>-<m>` beside the ledger directory's files.
fn torn_copies(fixture: &Pipeline) -> Vec<String> {
    let dir = journal_path(fixture)
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

/// `gx checkpoint export <FILE>` — the copy that leaves the machine.
fn export_head(fixture: &Pipeline, to: &Path) -> (i32, serde_json::Value) {
    let run = support::run(fixture.gx().args(["checkpoint", "export"]).arg(to));
    println!("EXPORT exit={} stdout={}", run.code, run.stdout.trim());
    if !run.stderr.trim().is_empty() {
        println!("EXPORT_STDERR={}", run.stderr.trim());
    }
    let json = serde_json::from_str(run.stdout.trim()).unwrap_or(serde_json::Value::Null);
    (run.code, json)
}

/// The commit receipts a project has written, by path.
fn commit_receipts(fixture: &Pipeline) -> Vec<PathBuf> {
    let dir = fixture.project.join(".gx").join("receipts");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read the receipt archive")
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".commit.json"))
        })
        .collect();
    out.sort();
    out
}

// ---------------------------------------------------------------------------
// H-01 — the pair truncated at two frame boundaries
// ---------------------------------------------------------------------------

/// 🔴 **`req/229` H-01 — the write-back.**
///
/// The cut is placed on purpose: the journal loses everything from the last commit's `Committed`
/// record onward, and the ledger keeps that commit's leaf as its **last** leaf. That is exactly 43
/// §7-3b's crash window, so `recover`'s second gate — "is this row's commit still the last thing
/// the ledger saw" — is **satisfied**, and R5's protection does not apply. Measured on the commit
/// before this lane: `gx serve` started, `recover={"terminal":1,"resumed":1,"nothing_applied":0,
/// "refused":0}`, `/healthz 200`, a signed checkpoint at `tree_size: 2`, and the target file back
/// at `two\n`.
#[test]
fn h01_truncating_both_files_between_a_leaf_and_its_record_does_not_write_back() {
    let fixture = three_commits("r6_h01_writeback");
    let journal = journal_path(&fixture);
    let ledger = ledger_path(&fixture);

    let journal_bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&journal_bytes);
    let record_kinds = kinds(&journal_bytes);
    // The record before the last commit's `Committed`: `ApplyStarted`. Cutting after it leaves the
    // journal describing a commit the ledger has already witnessed.
    let last_committed = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .nth(1)
        .expect("three commits leave three `Committed` records");
    let cut = spans[last_committed - 1].0 + spans[last_committed - 1].1;
    println!(
        "CUTTING_AT record={} kind={} offset={cut}",
        last_committed - 1,
        record_kinds[last_committed - 1]
    );
    truncate_at(&journal, cut as u64);

    let ledger_bytes = std::fs::read(&ledger).expect("read the ledger");
    let leaves = ledger_frames(&ledger_bytes);
    let ledger_cut = leaves[1].0 + leaves[1].1;
    truncate_at(&ledger, ledger_cut as u64);

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(code, 0, "a project that went backwards is not healthy");
    let why = report["rolled_back"]
        .as_str()
        .expect("`gx repair` names the condition rather than folding it into a count");
    assert!(
        why.contains("rolled_back"),
        "the one word every face spells: {why}"
    );
    assert!(
        report["remedy"].is_string(),
        "a state you can see and cannot leave is the trap req/222 H-06 named: {report}"
    );

    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("a rolled-back project is not one to serve from");
    println!("RESTART_REFUSED={refusal}");
    assert!(
        refusal.contains("rolled_back"),
        "the refusal says which condition it is: {refusal}"
    );
    assert!(
        refusal.contains("Nothing was applied"),
        "and says the substrate was not touched: {refusal}"
    );
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "the whole finding: the operator's file does not go back to `two`"
    );
}

/// 🔴 **`req/229` H-01, the other cut** — the history is erased and no second root is signed for a
/// size this project already published.
///
/// Placing the cut **outside** a commit removes a commit whole. Before this lane the project came
/// back healthy and the next commit signed `tree_size: 3` with a **different** root under the same
/// key — the audit's raw is `EQUIVOCATION same_size=True same_root=False same_keyid=True`. Two
/// signed statements about one tree size is the failure a transparency log exists to make
/// impossible.
#[test]
fn h01_a_project_cut_outside_a_commit_cannot_sign_a_second_root_for_a_size_it_published() {
    let fixture = three_commits("r6_h01_equivocation");
    let journal = journal_path(&fixture);
    let ledger = ledger_path(&fixture);

    let journal_bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&journal_bytes);
    let record_kinds = kinds(&journal_bytes);
    let second_committed = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .nth(1)
        .expect("three `Committed` records");
    let cut = spans[second_committed].0 + spans[second_committed].1;
    truncate_at(&journal, cut as u64);
    let ledger_bytes = std::fs::read(&ledger).expect("read the ledger");
    let leaves = ledger_frames(&ledger_bytes);
    truncate_at(&ledger, (leaves[1].0 + leaves[1].1) as u64);

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(code, 0, "exit 1: the next write is going to be refused");
    assert!(
        report["rolled_back"].is_string(),
        "the two files agree with each other and with nothing else: {report}"
    );
    assert_eq!(
        report["journal_commits"], report["ledger_leaves"],
        "the point of this arm: the pair is internally perfect"
    );

    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("no fourth commit is going to be signed over a forged history");
    println!("RESTART_REFUSED={refusal}");
    assert!(refusal.contains("rolled_back"), "{refusal}");
}

// ---------------------------------------------------------------------------
// H-02 — the downgrade
// ---------------------------------------------------------------------------

/// 🔴 **`req/229` H-02** — a journal whose marker and links were removed is refused, and is not
/// cut.
///
/// Before this lane: `gx repair` exit 0, `journal_format: "legacy"`, `journal_intact: true`,
/// `gx serve` started with no warning, a fourth commit landed in the old framing, and the file
/// stayed legacy afterwards.
#[test]
fn h02_a_journal_stripped_of_its_chain_is_refused_rather_than_accepted_as_legacy() {
    let fixture = three_commits("r6_h02_downgrade");
    let journal = journal_path(&fixture);
    let (was, now) = strip_the_chain(&journal);
    assert!(now < was, "the links and the marker are gone");

    let (code, report) = repair_report(&fixture, false);
    assert_ne!(code, 0, "a downgraded project is not healthy");
    assert_eq!(
        report["journal_intact"], false,
        "the chain was removed after this project was written: {report}"
    );
    assert_eq!(report["journal_format"], "legacy", "what the file is now");
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — this fixture is a project this build
    // made, so what it declares is `chained-v2`. The claim did **not** change: the declaration is
    // still the fact that separates a downgrade from an old project, and it is still compared as an
    // exact value rather than as "contains `chained`", which `chained-v2` would satisfy by accident.
    assert_eq!(
        report["journal_format_declared"],
        support::CREATED_JOURNAL_FORMAT.kind(),
        "and what the project says it is — the fact that separates a downgrade from an old project"
    );
    assert_eq!(
        report["downgraded"], true,
        "the two facts above, compared, so that a machine does not have to: {report}"
    );
    assert!(
        report["remedy"]
            .as_str()
            .is_some_and(|r| r.contains("format marker")),
        "the remedy names the missing marker rather than a chain break: {report}"
    );
    assert_eq!(
        report["journal_chain_break_at"],
        serde_json::Value::Null,
        "and it is not a chain break — there is no chain left to break: {report}"
    );

    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("a project whose detector was removed is not one to serve from");
    println!("RESTART_REFUSED={refusal}");
    assert_eq!(
        std::fs::metadata(&journal).expect("stat").len(),
        now,
        "and the file is not cut: DR-43-9 (c-3) applies to a removed chain as much as a broken one"
    );
    assert!(
        torn_copies(&fixture).is_empty(),
        "nothing was quarantined: {:?}",
        torn_copies(&fixture)
    );
    assert_eq!(fixture.target_contents(), "three\n");
}

/// 🔴 **`req/229` H-02(b)** — the audit-five rewrite, re-run on a downgraded journal.
///
/// This is the arm that makes H-02 a high finding rather than a cosmetic one: on the commit before
/// this lane, a live server over a downgraded-and-then-rewritten journal answered `/healthz 200`,
/// `POST /candidates 201` and a **signed** `GET /ledger/checkpoint`, exactly as it did before R5
/// existed. The attacker chose how strong the defence was.
#[test]
fn h02_a_rewrite_on_a_downgraded_journal_is_still_seen_by_a_live_server() {
    let fixture = three_commits("r6_h02_live");
    let locator = fixture.target.display().to_string();
    let journal = journal_path(&fixture);
    strip_the_chain(&journal);

    // The server refuses to start at all now, which is a stronger answer than the one this arm was
    // written to demand — so the refusal is asserted here and the live-rewrite half below is run
    // against a project whose declaration is intact.
    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("the downgrade is refused before a request can be made");
    println!("RESTART_REFUSED={refusal}");

    // And the same rewrite on a **chained** file, live, still answers 500 (R5's property, checked
    // here so that this lane cannot have traded it away).
    let second = three_commits("r6_h02_live_chained");
    let second_journal = journal_path(&second);
    let server = Serving::start(&second.project, &second.home, &second.key_id);
    let (before, before_body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(before, 200, "the control: {before_body}");
    let record_kinds = kinds(&std::fs::read(&second_journal).expect("read"));
    let committed: Vec<usize> = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .collect();
    copy_record_over(&second_journal, committed[0], committed[2]);
    let (after, after_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER status={after} body={after_body}");
    assert_eq!(after, 500, "R5's property, unmoved: {after_body}");
    let (created, created_body) = server.create_status(&locator, "x\n", &second.key_id);
    assert_eq!(created, 500, "and the writes stop: {created_body}");
    shut_down(server);
}

// ---------------------------------------------------------------------------
// M group
// ---------------------------------------------------------------------------

/// 🔴 **`req/229` M-01** — removing eight bytes does not make gx cut the other 5,700.
///
/// Stripping the marker leaves a file whose first four bytes read as a legacy length header and
/// whose rest is chained frames, so the legacy walk stops after one record and calls the remaining
/// 98% a torn tail. The audit measured `gx serve` copying 5,714 bytes to `journal.torn.93-5714`
/// and cutting the journal to 93 — while the `gx repair` afterwards reported
/// `torn_tail_bytes: 0, quarantined_to: null`, so neither the cut nor the copy appeared anywhere in
/// the diagnosis.
#[test]
fn m01_stripping_the_marker_does_not_make_the_writer_cut_the_journal() {
    let fixture = three_commits("r6_m01");
    let journal = journal_path(&fixture);
    let bytes = std::fs::read(&journal).expect("read the journal");
    std::fs::write(&journal, &bytes[8..]).expect("write the journal without its marker");
    let expected = (bytes.len() - 8) as u64;
    println!("MARKER_STRIPPED {} -> {expected}", bytes.len());

    let refusal = Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id)
        .err()
        .expect("the project is refused");
    println!("RESTART_REFUSED={refusal}");
    assert!(
        refusal.contains("LEDGER_DISAGREES"),
        "44 §2.3's `INTERNAL` is `not classifiable` and this is classifiable: {refusal}"
    );
    assert_eq!(
        std::fs::metadata(&journal).expect("stat").len(),
        expected,
        "not one byte was removed"
    );
    assert!(
        torn_copies(&fixture).is_empty(),
        "and nothing was copied: {:?}",
        torn_copies(&fixture)
    );
}

/// 🔴 **`req/229` M-02** — the report opens on a project whose ledger file is gone.
///
/// The audit's raw: `{"gx_code":"INTERNAL","detail":"the ledger refused to open: …"}` with **no
/// JSON report at all**, while `gx repair --yes` printed a complete report and grew the file back.
/// `req/227` M-04's principle — the diagnosis must not open on a narrower set of projects than the
/// repair — applied to the ledger itself.
#[test]
fn m02_a_repair_report_opens_a_project_whose_ledger_file_is_absent() {
    let fixture = three_commits("r6_m02");
    let ledger = ledger_path(&fixture);
    std::fs::remove_file(&ledger).expect("remove the ledger");
    println!("LEDGER_REMOVED {}", ledger.display());

    let (code, report) = repair_report(&fixture, false);
    assert!(
        report.is_object(),
        "the diagnosis is the deliverable and it was not produced"
    );
    assert_eq!(
        report["ledger_present"], false,
        "and it says the file is absent: {report}"
    );
    assert_ne!(code, 0, "the project still cannot be written to");
    assert!(
        !ledger.exists(),
        "a report that writes nothing does not grow a ledger back"
    );
}

/// 🔴 **`req/229` M-04** — a chain break does not let `--yes` cut the **ledger**.
///
/// The audit flipped one bit inside a journal record and one inside a ledger leaf, ran
/// `gx repair --yes`, and watched a run that had just reported `journal_intact: false,
/// journal_chain_break_at: 2754` move 348 of the ledger's 522 bytes into
/// `journal.ledger.torn.174-522` — two leaves, one of them undamaged. "I do not trust this file"
/// and "I will trim the other file to agree with it" cannot both be true of one run.
#[test]
fn m04_a_chain_break_does_not_let_a_repair_cut_the_ledger() {
    let fixture = three_commits("r6_m04");
    let journal = journal_path(&fixture);
    let ledger = ledger_path(&fixture);

    let mut journal_bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&journal_bytes);
    let at = spans[15].0 + 8;
    journal_bytes[at] ^= 0x01;
    std::fs::write(&journal, &journal_bytes).expect("write the journal back");
    let mut ledger_bytes = std::fs::read(&ledger).expect("read the ledger");
    let leaves = ledger_frames(&ledger_bytes);
    let leaf_at = leaves[1].0 + 8;
    ledger_bytes[leaf_at] ^= 0x01;
    std::fs::write(&ledger, &ledger_bytes).expect("write the ledger back");
    let ledger_len = ledger_bytes.len() as u64;
    println!("BOTH_FLIPPED journal_at={at} ledger_at={leaf_at} ledger_len={ledger_len}");

    let (code, report) = repair_report(&fixture, true);
    assert_ne!(code, 0, "the project still disagrees");
    assert_eq!(
        report["journal_intact"], false,
        "the run knows the journal is not evidence: {report}"
    );
    assert_eq!(
        std::fs::metadata(&ledger).expect("stat").len(),
        ledger_len,
        "and therefore does not trim the ledger to agree with it"
    );
    assert!(
        torn_copies(&fixture).is_empty(),
        "nothing was quarantined either: {:?}",
        torn_copies(&fixture)
    );
    assert_eq!(fixture.target_contents(), "three\n");
}

/// 🔴 **`req/229` M-05** — `repaired` is what happened, not the flag that was passed.
///
/// The audit ran `gx repair --yes` twice on a project DR-43-9 (c-3) forbids touching, got
/// `repaired: true` both times, and measured the journal's and the ledger's md5 unchanged on both
/// sides of both runs.
#[test]
fn m05_repaired_is_false_when_the_run_wrote_nothing() {
    let fixture = three_commits("r6_m05");
    let journal = journal_path(&fixture);
    let mut bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&bytes);
    bytes[spans[15].0 + 8] ^= 0x01;
    std::fs::write(&journal, &bytes).expect("write the journal back");
    let before = std::fs::read(&journal).expect("read the journal");
    let ledger_before = std::fs::read(ledger_path(&fixture)).expect("read the ledger");

    let (code, report) = repair_report(&fixture, true);
    assert_ne!(code, 0);
    assert_eq!(
        report["repaired"], false,
        "nothing was written, so nothing was repaired: {report}"
    );
    assert_eq!(
        report["mode"], "yes",
        "what was asked for is still reported, under its own key"
    );
    assert_eq!(
        std::fs::read(&journal).expect("read"),
        before,
        "the bytes are the evidence for `repaired: false`"
    );
    assert_eq!(
        std::fs::read(ledger_path(&fixture)).expect("read"),
        ledger_before
    );
}

/// 🔴 **`req/229` M-06** — `gx key list` reads the key inside the file, not only the file's name.
///
/// `req/227` M-06 closed `KeyStore::load`; the audit measured the half that was left, where a store
/// holding key `B`'s bytes under key `A`'s name answered `{"key_id":"…de56e8db…",
/// "permissions_ok":true}` for a key that exists nowhere. `gx key list` is the one verb an operator
/// runs to see what they hold.
#[test]
fn m06_key_list_reads_the_key_inside_the_file() {
    let fixture = three_commits("r6_m06");
    let second = fixture.another_key();
    let keys = fixture.home.join(".gx").join("keys");
    let mine = keys.join(format!("{}.key", fixture.key_id));
    let theirs = keys.join(format!("{second}.key"));
    let donor = std::fs::read(&theirs).expect("read the second key");
    std::fs::write(&mine, &donor).expect("put the second key under the first key's name");
    println!("KEY_SWAPPED name={} contents={second}", fixture.key_id);

    let run = support::run(fixture.gx().args(["key", "list", "--json"]));
    println!("KEY_LIST exit={} stdout={}", run.code, run.stdout.trim());
    let listing: serde_json::Value = serde_json::from_str(run.stdout.trim()).expect("json");
    assert_eq!(
        listing["misnamed"], 1,
        "one file is not the key it is named for: {listing}"
    );
    let swapped = listing["keys"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|k| k["key_id"] == serde_json::Value::String(fixture.key_id.clone()))
        .expect("the entry for the file we swapped");
    assert_eq!(
        swapped["named_correctly"], false,
        "and the listing says so: {swapped}"
    );
    assert_eq!(
        swapped["key_id_inside"],
        serde_json::Value::String(second.clone()),
        "naming the key that is actually there: {swapped}"
    );
    assert!(
        run.stderr.contains(&second),
        "and saying it out loud, because an operator reads this verb to know what they have: {}",
        run.stderr
    );
}

// ---------------------------------------------------------------------------
// DR-43-10 — the copy that leaves the machine
// ---------------------------------------------------------------------------

/// 🔴 **DR-43-10 / `req/229` §7-4** — the head an auditor keeps refuses a project that went back,
/// and the receipts it covers still verify against it.
///
/// This is the arm that does not depend on anything inside `.gx/` surviving. The export is taken
/// **before** the attack, the attack removes a commit, and afterwards: the project refuses itself
/// against the export, the removed commit's receipt is `refuted` (exit 7) against the project's own
/// ledger, and the same receipt is `verified` (exit 0) against the export. That pair is what makes
/// the lie provable rather than merely suspected.
#[test]
fn dr4310_an_exported_head_refuses_a_project_that_went_backwards() {
    let fixture = three_commits("r6_export");
    let outside = fixture.home.join("auditor-checkpoint.json");
    let (code, exported) = export_head(&fixture, &outside);
    assert_eq!(code, 0, "the export needs no key: {exported}");
    assert_eq!(exported["tree_size"], 3, "three commits: {exported}");
    assert!(outside.is_file(), "and it is outside the project");

    // The receipt of the commit that is about to be removed, copied out beside the checkpoint —
    // the second half of what §7-4 says an auditor holds.
    let receipts = commit_receipts(&fixture);
    assert_eq!(receipts.len(), 3, "one per commit: {receipts:?}");
    let kept: Vec<PathBuf> = receipts
        .iter()
        .map(|p| {
            let to = fixture.home.join(p.file_name().expect("a file name"));
            std::fs::copy(p, &to).expect("copy the receipt out");
            to
        })
        .collect();

    let journal = journal_path(&fixture);
    let ledger = ledger_path(&fixture);
    let journal_bytes = std::fs::read(&journal).expect("read");
    let spans = frames(&journal_bytes);
    let record_kinds = kinds(&journal_bytes);
    let second_committed = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .nth(1)
        .expect("three `Committed` records");
    truncate_at(
        &journal,
        (spans[second_committed].0 + spans[second_committed].1) as u64,
    );
    let ledger_bytes = std::fs::read(&ledger).expect("read");
    let leaves = ledger_frames(&ledger_bytes);
    truncate_at(&ledger, (leaves[1].0 + leaves[1].1) as u64);

    let (against_code, against) = repair_against(&fixture, &outside);
    assert_ne!(against_code, 0, "the export refuses this project");
    assert_eq!(
        against["against"]["rolled_back"], true,
        "and says so in the one place a machine reads: {against}"
    );
    assert_eq!(against["against"]["tree_size"], 3);
    assert_eq!(against["against"]["project_tree_size"], 2);

    // 🔴 The pair that makes the lie provable: **one** receipt is `refuted` by the project's own
    // ledger and `verified` against the head that left the machine, and it is the **same** receipt.
    //
    // The other two answer `unbridged` offline, and that is a **declared** limit rather than a
    // failure of this lane: each receipt's inclusion proof names the tree size it was issued at, so
    // an anchor two commits later needs the consistency proof between them (`req/222` H-09,
    // `--consistency <FILE>`). What §7-4 needs is one receipt that survives both questions, and the
    // removed commit's is exactly the one whose proof names the exported size.
    let mut refuted_and_kept = 0;
    let mut unbridged = 0;
    for receipt in &kept {
        let local = support::run(fixture.gx().args(["receipt", "verify"]).arg(receipt));
        let offline = support::run(
            fixture
                .gx()
                .args(["receipt", "verify"])
                .arg(receipt)
                .arg("--offline")
                .arg("--checkpoint")
                .arg(&outside),
        );
        println!(
            "RECEIPT {} local_exit={} offline_exit={} offline={}",
            receipt.file_name().expect("name").to_string_lossy(),
            local.code,
            offline.code,
            offline.stdout.trim()
        );
        if local.code == 7 && offline.code == 0 {
            refuted_and_kept += 1;
        }
        if offline.stdout.contains("unbridged") {
            unbridged += 1;
        }
    }
    assert_eq!(
        refuted_and_kept, 1,
        "one receipt names a commit the project has removed and the auditor's head still proves"
    );
    assert_eq!(
        unbridged, 2,
        "🔴 the declared limit, asserted rather than hidden: a receipt issued at an earlier tree \
         size needs the consistency proof to reach a later anchor (req/222 H-09)"
    );
}

// ---------------------------------------------------------------------------
// Self-adversarial — this lane attacking what this lane wrote
// ---------------------------------------------------------------------------

/// 🔴 **S-1 — the head is inside the attacker's write scope, and this says so with a number.**
///
/// The detector DR-43-11 adds lives in the project. An attacker who truncates the two files can
/// also replace `.gx/checkpoints/head.json` with an **older, genuinely signed** head — this project
/// signed it, so no forgery is involved and no signature check would catch it. The probe asserts
/// the uncomfortable answer: **the project opens**. It then asserts the answer that is not
/// uncomfortable — the copy outside the machine still refuses it — because that is the whole reason
/// `gx checkpoint export` exists.
///
/// A lane that measured only its own successes would be repeating the failure five audits in a row
/// have found in the previous lane's repair. `docs/LIMITS.md`'s v0.4-s paragraph carries this
/// sentence for buyers.
#[test]
fn s1_an_older_signed_head_put_back_in_place_is_not_detected_inside_the_project() {
    let fixture = three_commits("r6_s1");
    let head = head_path(&fixture);
    let journal = journal_path(&fixture);
    let ledger = ledger_path(&fixture);

    // The head as it stood after two commits, taken the way an attacker would: by keeping a copy.
    let outside = fixture.home.join("head-at-three.json");
    let (export_code, _) = export_head(&fixture, &outside);
    assert_eq!(export_code, 0);

    let journal_bytes = std::fs::read(&journal).expect("read");
    let spans = frames(&journal_bytes);
    let record_kinds = kinds(&journal_bytes);
    let second_committed = record_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .nth(1)
        .expect("three `Committed` records");
    let journal_cut = (spans[second_committed].0 + spans[second_committed].1) as u64;
    let ledger_bytes = std::fs::read(&ledger).expect("read");
    let leaves = ledger_frames(&ledger_bytes);
    let ledger_cut = (leaves[1].0 + leaves[1].1) as u64;

    // 🔴 The attacker deletes the detector as well. Nothing signs a replacement, so `read` answers
    // "this project never recorded a head" — which is the honest answer and is also useless.
    std::fs::remove_file(&head).expect("remove the recorded head");
    truncate_at(&journal, journal_cut);
    truncate_at(&ledger, ledger_cut);

    let (code, report) = repair_report(&fixture, false);
    println!(
        "S1_REPAIR exit={code} head_recorded={}",
        report["head_recorded"]
    );
    assert_eq!(
        report["head_recorded"], false,
        "the detector is gone, and the report says that rather than saying `healthy`"
    );
    assert_eq!(
        report["rolled_back"],
        serde_json::Value::Null,
        "🔴 the honest denominator: with no head to compare against, the rollback is invisible \
         from inside the project. This assertion is the measurement, not the goal"
    );
    assert_eq!(code, 0, "and the project reports itself healthy: {report}");

    // 🔴 And the answer to that: the copy that left the machine.
    let (against_code, against) = repair_against(&fixture, &outside);
    assert_ne!(
        against_code, 0,
        "the artefact outside the box is what cannot be deleted from inside it"
    );
    assert_eq!(against["against"]["rolled_back"], true, "{against}");
}

/// 🔴 **S-1b — an *older, genuinely signed* head put back in place.**
///
/// S-1 deletes the detector, which leaves a visible hole (`head_recorded: false`). This arm is the
/// sharper version: keep a copy of `head.json` from when the tree held two leaves, roll the project
/// back to two leaves, and put the old copy back. Nothing is forged — **this project signed that
/// document** — so no signature check can help, and the numbers agree exactly. The probe asserts
/// that gx passes it, because that is what happens, and then asserts the answer: a copy taken at
/// the *later* size, from outside the machine, still refuses.
#[test]
fn s1b_an_older_head_this_project_itself_signed_is_accepted_when_put_back() {
    let fixture = pipeline("r6_s1b", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let head = head_path(&fixture);

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "one\n", &fixture.key_id);
    server.commit_over_http(&locator, "two\n", &fixture.key_id);
    shut_down(server);
    // The attacker's copy, taken while the project is honest.
    let stolen = fixture.home.join("head-at-two.json");
    std::fs::copy(&head, &stolen).expect("keep a copy of the head at two leaves");
    let journal_at_two = std::fs::metadata(journal_path(&fixture))
        .expect("stat")
        .len();
    let ledger_at_two = std::fs::metadata(ledger_path(&fixture))
        .expect("stat")
        .len();

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "three\n", &fixture.key_id);
    shut_down(server);
    // The auditor's copy, taken at the later size and kept off the machine.
    let auditors = fixture.home.join("auditor-at-three.json");
    let (export_code, _) = export_head(&fixture, &auditors);
    assert_eq!(export_code, 0);
    assert_eq!(fixture.target_contents(), "three\n");

    truncate_at(&journal_path(&fixture), journal_at_two);
    truncate_at(&ledger_path(&fixture), ledger_at_two);
    std::fs::copy(&stolen, &head).expect("put the older signed head back");
    println!("OLD_HEAD_RESTORED {}", head.display());

    let (code, report) = repair_report(&fixture, false);
    println!("S1B_REPAIR exit={code}");
    assert_eq!(
        report["head_recorded"], true,
        "the detector is present and signed by this project: {report}"
    );
    assert_eq!(
        report["rolled_back"],
        serde_json::Value::Null,
        "🔴 the honest denominator: a head this project genuinely signed, restored in place, is \
         indistinguishable from the current one. No signature check reaches this — the document is \
         authentic. This assertion is the measurement, not the goal"
    );
    assert_eq!(code, 0, "and the project reports itself healthy: {report}");

    let (against_code, against) = repair_against(&fixture, &auditors);
    assert_ne!(
        against_code, 0,
        "the copy taken at the later size is what the attacker could not reach"
    );
    assert_eq!(against["against"]["tree_size"], 3);
    assert_eq!(against["against"]["project_tree_size"], 2);
    assert_eq!(against["against"]["rolled_back"], true, "{against}");
}

/// 🔴 **S-2 — the declaration is inside the attacker's write scope too.**
///
/// `.gx/VERSION` is where H-02's repair lives, and it is a file in the project. Removing the
/// `journal_format` line turns a declared-chained project back into one that has never said what it
/// is, and a downgrade is then indistinguishable from an old project. Measured rather than argued,
/// and the same answer applies: the tree statement outside the machine is what survives.
#[test]
fn s2_deleting_the_declaration_makes_a_downgrade_look_like_an_old_project() {
    let fixture = three_commits("r6_s2");
    let version = fixture.project.join(".gx").join("VERSION");
    let recorded = std::fs::read_to_string(&version).expect("read VERSION");
    println!("VERSION_BEFORE={recorded:?}");
    assert!(
        recorded.contains("journal_format=chained"),
        "the fixture declares itself: {recorded:?}"
    );
    let first = recorded.lines().next().expect("a version line").to_string();
    std::fs::write(&version, format!("{first}\n")).expect("write VERSION without the declaration");

    strip_the_chain(&journal_path(&fixture));
    let (code, report) = repair_report(&fixture, false);
    println!("S2_REPAIR exit={code}");
    assert_eq!(
        report["journal_format_declared"],
        serde_json::Value::Null,
        "🔴 the honest denominator: a project that has never declared a format cannot notice a \
         downgrade, and this is the measurement of that"
    );
    assert_eq!(
        report["journal_format"], "legacy",
        "what is left is the same report an old project gets: {report}"
    );
    // The rollback detector still catches this one, because stripping the links makes the journal
    // shorter than its recorded head. That is a second mechanism doing the first one's work, and it
    // is asserted so that a reader knows which of the two is answering.
    assert!(
        report["rolled_back"].is_string(),
        "DR-43-11 catches what the declaration no longer can: {report}"
    );
}

/// 🔴 **S-3 — a tampered export is refused by the verifier that checks its signature.**
///
/// `gx checkpoint export` writes a signed document and `gx repair --against` deliberately does
/// **not** check that signature (a third party may hold no key). So the check has to exist
/// somewhere, and it does: `gx receipt verify --checkpoint-key` is the one caller of
/// `verify_checkpoint`. This arm edits the export's `tree_size` and asserts the refusal, so that
/// "keep a copy" cannot quietly become "trust any file called a checkpoint".
#[test]
fn s3_an_edited_export_does_not_verify_under_the_key_that_signed_it() {
    let fixture = three_commits("r6_s3");
    let outside = fixture.home.join("checkpoint.json");
    let (code, _) = export_head(&fixture, &outside);
    assert_eq!(code, 0);

    let receipts = commit_receipts(&fixture);
    let receipt = receipts.first().expect("a commit receipt");
    let public = fixture
        .home
        .join(".gx")
        .join("keys")
        .join(format!("{}.key", fixture.key_id));

    let honest = support::run(
        fixture
            .gx()
            .args(["receipt", "verify"])
            .arg(receipt)
            .arg("--offline")
            .arg("--checkpoint")
            .arg(&outside)
            .arg("--checkpoint-key")
            .arg(&public),
    );
    println!(
        "S3_HONEST exit={} stdout={}",
        honest.code,
        honest.stdout.trim()
    );
    // The control is `anchor_authenticated`, not the exit status: this receipt was issued at an
    // earlier tree size than the exported head, so its **inclusion** is `unbridged` without a
    // consistency proof (`req/222` H-09, unchanged by this lane). What is being measured here is
    // whether the anchor's own signature was checked at all.
    let honest_json: serde_json::Value =
        serde_json::from_str(honest.stdout.trim()).expect("json on stdout");
    assert_eq!(
        honest_json["anchor_authenticated"], true,
        "the control: an unedited export verifies under the key that signed it: {honest_json}"
    );

    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&outside).expect("read the export")).expect("json");
    doc["tree_size"] = serde_json::json!(99);
    std::fs::write(
        &outside,
        serde_json::to_vec_pretty(&doc).expect("serialise"),
    )
    .expect("write the edited export");

    let edited = support::run(
        fixture
            .gx()
            .args(["receipt", "verify"])
            .arg(receipt)
            .arg("--offline")
            .arg("--checkpoint")
            .arg(&outside)
            .arg("--checkpoint-key")
            .arg(&public),
    );
    println!(
        "S3_EDITED exit={} stdout={} stderr={}",
        edited.code,
        edited.stdout.trim(),
        edited.stderr.trim()
    );
    assert_ne!(
        edited.code, 0,
        "an anchor that does not verify under the key it names is not an anchor"
    );
    let edited_json: serde_json::Value =
        serde_json::from_str(edited.stdout.trim()).unwrap_or(serde_json::Value::Null);
    assert_ne!(
        edited_json["anchor_authenticated"],
        serde_json::Value::Bool(true),
        "and the edit is what the refusal is about: {edited_json} / {}",
        edited.stderr
    );
}

/// 🔴 **S-4 — the new gates do not fire on a project that is behaving.**
///
/// Every arm above is about a detector that was too weak. This one is about the opposite failure,
/// which is the one a monotonicity check is most likely to have: a floor that is raised wrongly
/// makes an honest project unopenable, and R2's whole co-existence claim dies with it. A server and
/// a CLI write in turn, the project is restarted, and everything answers `200`/`0`.
#[test]
fn s4_a_server_and_a_cli_writing_in_turn_do_not_trip_the_new_gates() {
    let fixture = three_commits("r6_s4");
    let locator = fixture.target.display().to_string();

    for round in 0..3 {
        let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
        server.commit_over_http(&locator, &format!("http-{round}\n"), &fixture.key_id);
        let (status, body) = server.request("GET", "/v1/healthz", None);
        assert_eq!(status, 200, "round {round} after an HTTP commit: {body}");
        let (checkpoint, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
        assert_eq!(checkpoint, 200, "round {round}: {checkpoint_body}");
        shut_down(server);

        let submitted = fixture.submit(&format!("cli-{round}\n"));
        assert_eq!(submitted.code, 0, "round {round}: {}", submitted.stderr);

        let (code, report) = repair_report(&fixture, false);
        assert_eq!(code, 0, "round {round}: {report}");
        assert_eq!(report["rolled_back"], serde_json::Value::Null);
        assert_eq!(report["head_recorded"], true, "and the head is being kept");
    }

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (status, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(status, 200, "and a restart after all of that: {body}");
    shut_down(server);

    // 🔴 The same loss condition for `--against`, which is the check an operator is being told to
    // run on a schedule. One that fires on a healthy project is worse than no check at all: it will
    // be switched off, and then it is not there on the morning it matters. Both the equal case and
    // the grown case have to pass.
    let outside = fixture.home.join("s4-checkpoint.json");
    let (export_code, _) = export_head(&fixture, &outside);
    assert_eq!(export_code, 0);
    let (equal_code, equal) = repair_against(&fixture, &outside);
    assert_eq!(equal_code, 0, "a project at its exported head: {equal}");
    assert_eq!(equal["against"]["rolled_back"], false, "{equal}");
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    server.commit_over_http(&locator, "grown\n", &fixture.key_id);
    shut_down(server);
    let (grown_code, grown) = repair_against(&fixture, &outside);
    assert_eq!(grown_code, 0, "a project that has grown past it: {grown}");
    assert_eq!(grown["against"]["rolled_back"], false, "{grown}");
    assert!(
        grown["against"]["project_tree_size"].as_u64() > grown["against"]["tree_size"].as_u64(),
        "growing is not going backwards: {grown}"
    );
}

/// 🔴 **S-5 — a project that predates this release opens, and is not called a downgrade.**
///
/// 42 §3.13 v0.4-r chose backward compatibility on the ground that "a repair that makes existing
/// projects unopenable is worse than the accident". H-02's repair must not take that back: a
/// journal with no marker and **no declaration** is an old project, and it opens.
#[test]
fn s5_a_project_that_never_declared_a_format_still_opens_as_legacy() {
    let fixture = three_commits("r6_s5");
    let version = fixture.project.join(".gx").join("VERSION");
    let recorded = std::fs::read_to_string(&version).expect("read VERSION");
    let first = recorded.lines().next().expect("a version line").to_string();
    std::fs::write(&version, format!("{first}\n")).expect("write VERSION without the declaration");
    // A journal with no chain and no declaration: an old project, exactly.
    strip_the_chain(&journal_path(&fixture));
    // The recorded head names a longer journal, so it is removed too — an old project has none.
    let head = head_path(&fixture);
    if head.exists() {
        std::fs::remove_file(&head).expect("remove the recorded head");
    }

    let (code, report) = repair_report(&fixture, false);
    println!("S5_REPAIR exit={code} report={report}");
    assert_eq!(
        report["journal_format"], "legacy",
        "an old project reports itself as one: {report}"
    );
    assert_eq!(
        report["journal_intact"], true,
        "and is not refused: 42 §3.13 v0.4-r's backward compatibility is unmoved"
    );
    assert_eq!(code, 0, "{report}");

    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);
    let (status, body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(status, 200, "and it serves: {body}");
    shut_down(server);
}
