// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R5 — the accident the fifth adversarial audit measured, and the falsifiers this lane owes
//! for its own new code** (`req/227`, `req/38` §165).
//!
//! `req/227` §9's implementation row named four arms before this file existed, and said all four
//! would be red the day it wrote them: red if `POST /candidates` answers `201` after a record in
//! the middle of the journal is overwritten with the bytes of another record from the same file;
//! red if `/healthz` answers `200` after two adjacent records are swapped; red if a project whose
//! `Committed` record was replaced comes back from a restart with the operator's file **moved**;
//! red if `gx repair` answers `journal_intact: true` about a journal that is not intact. They were.
//!
//! # What was actually wrong, in one sentence
//!
//! R4 gave the journal a detector and the detector compared a **shape**: the same number of bytes
//! came back as the same number of whole records. `req/227` measured three rewrites that satisfy
//! both counts — a record copied from elsewhere in the same file (the audit used `cp`; one commit
//! writes ten records of the same framed lengths every time, so every record has a twin), two
//! adjacent records swapped, and one bit flipped inside a payload — and then measured the
//! consequence, which was not a missing alarm but a **write to somebody's disk**: where the
//! substituted record was a `Committed`, the next start-up's `recover` read the row as an
//! unfinished commit, asked the adapter again, and took the target file from `three` back to `one`.
//!
//! DR-43-9 is the repair: every record carries a chain link over its own bytes and its
//! predecessor's link, so "is this the file I read" is one comparison of 32 bytes and no rewrite of
//! any size survives it. `Engine::recover` is the second half, and it is deliberately a different
//! mechanism (see `s1_`).
//!
//! # The second half, and why it is half the file
//!
//! Five adversarial audits in a row have found their highest-severity item in the **previous
//! lane's repair** (`req/38` §163 ruling 1 made the answer part of a repair lane's definition of
//! done). So the probes below come in two kinds and are labelled as such: `h01_`/`m0…_` are the
//! audit's, and `s1_`..`s5_` are this lane's attacks on what this lane wrote — a journal whose
//! chain has been **recomputed end to end** so that it verifies perfectly; a journal in the old
//! format, which has no chain at all; a record deleted with the file's length made up at the end;
//! a server and a CLI writing in turn, where a new detector fails by being too **sensitive**; and
//! the file this lane refuses to truncate, which is a new way to leave a project unusable.
//!
//! `cfg(unix)` for its predecessors' reasons — `SIGTERM`, `flock`, `chmod` — and with the same
//! declaration: Windows, WSL 9p and a synchronising client are **not measured** (`req/213` §7(d),
//! unchanged by this lane).

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
/// The sixth copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test binary
/// is its own crate.
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

    /// 🔴 The same start, for a probe whose **point** is that the server refuses.
    ///
    /// `Err` carries whatever the process said before it stopped, so a refusal can be asserted on
    /// its words and not only on its absence.
    fn try_start(project: &Path, home: &Path, key_id: &str) -> Result<Self, String> {
        let token = "r5-runtime-token".to_string();
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

/// Flip one bit at `at`, leaving the file's length exactly as it was.
fn flip_bit(path: &Path, at: usize) -> u64 {
    let mut bytes = std::fs::read(path).expect("read the file");
    assert!(at < bytes.len(), "offset {at} is past the end of {path:?}");
    bytes[at] ^= 0x01;
    std::fs::write(path, &bytes).expect("write the file back");
    bytes.len() as u64
}

// ---------------------------------------------------------------------------
// Reading the journal's framing from outside the engine
// ---------------------------------------------------------------------------

/// Every record frame in a journal file, as `(offset, framed_length)`.
///
/// 🔴 Deliberately written **without** the engine's constants, so that the same walk reads a file
/// written by this version (`JOURNAL_MAGIC`, then `[u32 length][payload][32-byte link]`) and one
/// written before DR-43-9 (`[u32 length][payload]`, repeated). The marker is detected the way
/// `replay` detects it and not by name: its first four bytes read as a `u32` past the one-megabyte
/// record ceiling, which no legacy length header can be.
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

/// The record kinds a journal holds, in order — the engine's own reader, used as an index.
fn kinds(bytes: &[u8]) -> Vec<&'static str> {
    gx_engine::replay(bytes)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect()
}

/// The indexes of the `Committed` records, in order.
fn committed_indexes(bytes: &[u8]) -> Vec<usize> {
    kinds(bytes)
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .collect()
}

/// Copy the framed bytes of record `from` over record `onto`, which must be the same length.
///
/// This is `req/227` H-01(a) with nothing added: no codec, no key, no forging. The bytes written
/// are bytes the file already held.
fn copy_record_over(path: &Path, onto: usize, from: usize) -> usize {
    let mut bytes = std::fs::read(path).expect("read the journal");
    let spans = frames(&bytes);
    let (onto_at, onto_len) = spans[onto];
    let (from_at, from_len) = spans[from];
    assert_eq!(
        onto_len, from_len,
        "the substitution keeps every length exactly: that is the whole of the attack"
    );
    let donor = bytes[from_at..from_at + from_len].to_vec();
    bytes[onto_at..onto_at + onto_len].copy_from_slice(&donor);
    std::fs::write(path, &bytes).expect("write the journal back");
    onto_at
}

/// Swap two adjacent record frames, which may be of different lengths.
///
/// The file's length is unchanged, the number of whole records is unchanged, and every byte in it
/// is a byte gx wrote. Only the order is a lie.
fn swap_adjacent(path: &Path, first: usize) {
    let bytes = std::fs::read(path).expect("read the journal");
    let spans = frames(&bytes);
    let (a_at, a_len) = spans[first];
    let (b_at, b_len) = spans[first + 1];
    assert_eq!(a_at + a_len, b_at, "the two frames are adjacent");
    let mut out = bytes[..a_at].to_vec();
    out.extend_from_slice(&bytes[b_at..b_at + b_len]);
    out.extend_from_slice(&bytes[a_at..a_at + a_len]);
    out.extend_from_slice(&bytes[b_at + b_len..]);
    assert_eq!(out.len(), bytes.len(), "and the file is the same size");
    std::fs::write(path, &out).expect("write the journal back");
}

/// A project with three commits behind it, its server already stopped.
///
/// The target file ends at `three\n`, and the three commits are what makes `req/227` H-01
/// reproducible: from the second commit onward every record has a same-length twin in the file.
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

// ---------------------------------------------------------------------------
// H-01 — the rewrite in the middle that nothing caught, and the write-back
// ---------------------------------------------------------------------------

/// 🔴 **`req/227` H-01(a)** — a record overwritten with the bytes of another record from the same
/// file.
///
/// The attacker's whole toolkit is `cp`: `EngineJournal` lays down the same ten framed lengths per
/// commit, so from the second commit onward the donor is already inside the file. R4's detector
/// compared a byte count and a record count, both of which this preserves exactly. Measured before
/// the repair: `/healthz` `200 ledger_agrees:true`, `POST /candidates` `201`, `GET
/// /ledger/checkpoint` `200` **signed**.
#[test]
fn h01_a_record_copied_from_elsewhere_in_the_journal_stops_the_writing() {
    let fixture = three_commits("r5_copy");
    let locator = fixture.target.display().to_string();
    let journal = journal_path(&fixture);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    let (before_status, before_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_BEFORE status={before_status} body={before_body}");
    assert_eq!(before_status, 200, "the control: {before_body}");

    let bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&bytes);
    let commits = committed_indexes(&bytes);
    println!(
        "JOURNAL records={} commits={commits:?} lengths={:?}",
        spans.len(),
        spans.iter().map(|(_, l)| *l).collect::<Vec<_>>()
    );
    let onto = commits[0];
    let from = commits[2];
    let at = copy_record_over(&journal, onto, from);
    println!(
        "RECORD_COPIED onto={onto} from={from} at={at} len_after={}",
        std::fs::metadata(&journal).expect("the journal").len()
    );
    assert_eq!(
        std::fs::metadata(&journal).expect("the journal").len(),
        bytes.len() as u64,
        "the file is exactly as long as it was"
    );

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 `req/227` H-01(a): a monitor read `ok` from a server whose journal held a record gx \
         did not write there: {health_body}"
    );
    assert!(
        health_body.contains("LEDGER_DISAGREES"),
        "the same word every other face uses (req/38 §156 ruling 2(a)): {health_body}"
    );
    assert!(
        health_body.contains("journal is the file that moved"),
        "and it names which file: {health_body}"
    );

    let (create_status, create_body) = server.create_status(&locator, "four\n", &fixture.key_id);
    println!("CREATE_AFTER_COPY status={create_status} body={create_body}");
    assert_ne!(
        create_status, 201,
        "and the write is refused rather than appended on top of it: {create_body}"
    );

    let (checkpoint_status, checkpoint_body) = server.request("GET", "/v1/ledger/checkpoint", None);
    println!("CHECKPOINT_AFTER_COPY status={checkpoint_status} body={checkpoint_body}");
    assert_eq!(
        checkpoint_status, 500,
        "and nothing is signed over it: {checkpoint_body}"
    );
    shut_down(server);
}

/// 🔴 **`req/227` H-01(b)** — two adjacent records swapped.
///
/// Different lengths, so this is not even the "same-length substitution" R4's note declared as its
/// limit: the byte count and the record count are preserved because the *pair* is, and nothing
/// about a count can see an order. Every byte in the file is a byte gx wrote.
#[test]
fn h01_two_adjacent_records_swapped_are_not_the_journal_this_process_read() {
    let fixture = three_commits("r5_swap");
    let journal = journal_path(&fixture);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    let (before_status, before_body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(before_status, 200, "the control: {before_body}");

    let before = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&before);
    // Two neighbours of different lengths, from the middle of the file.
    let first = spans
        .windows(2)
        .position(|w| w[0].1 != w[1].1)
        .expect("a commit writes records of several lengths");
    println!(
        "SWAPPING first={first} lengths=({},{})",
        spans[first].1,
        spans[first + 1].1
    );
    swap_adjacent(&journal, first);
    let after = std::fs::read(&journal).expect("read the journal");
    assert_eq!(before.len(), after.len(), "same length");
    assert_ne!(before, after, "and different order");

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("HEALTHZ_AFTER_SWAP status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 `req/227` H-01(b): the order of a log is the log: {health_body}"
    );
    shut_down(server);
}

/// 🔴 **`req/227` H-01(c)** — one bit inside a payload, in the middle of the file.
///
/// This arm is also the **reversal of a limit R4 declared and asserted**:
/// `serve_runtime_r4::h03_a_rewrite_in_the_middle_of_the_journal_stops_the_next_write` pinned
/// `/healthz` at `200` for exactly this damage, on the reasoning that a lockless read looks at the
/// tail record and at the length. `req/227` H-01 measured what that costs — a server that answers
/// `ok` about a file it can no longer replay, and signs checkpoints over it — and DR-43-9 makes the
/// check cheap enough (a hash walk, no CBOR decode) to run on the read road too. The old assertion
/// is corrected in place rather than deleted; this one is its opposite and says so.
#[test]
fn h01_one_bit_inside_a_payload_is_seen_from_the_middle_of_the_file() {
    let fixture = three_commits("r5_bitflip");
    let locator = fixture.target.display().to_string();
    let journal = journal_path(&fixture);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    let (before_status, before_body) = server.request("GET", "/v1/healthz", None);
    assert_eq!(before_status, 200, "the control: {before_body}");

    let len = flip_bit(&journal, 40);
    println!("JOURNAL_FLIPPED at=40 len={len}");

    let (health_status, health_body) = server.request("GET", "/v1/healthz", None);
    println!("MIDFLIP_HEALTHZ status={health_status} body={health_body}");
    assert_eq!(
        health_status, 500,
        "🔴 R4 asserted `200` here as a declared limit; DR-43-9 closes it: {health_body}"
    );

    let (create_status, create_body) = server.create_status(&locator, "four\n", &fixture.key_id);
    println!("MIDFLIP_CREATE status={create_status} body={create_body}");
    assert_ne!(create_status, 201, "and the write stops: {create_body}");
    shut_down(server);

    let (code, report) = repair_report(&fixture, false);
    assert_eq!(
        report["journal_intact"],
        serde_json::json!(false),
        "and the verb an operator is sent to says so rather than reporting a healthy file"
    );
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — this fixture's journal is one this
    // build wrote, so the framing the report names is `chained-v2`. The claim did **not** change:
    // the report still has to name the framing the file is actually in rather than falling back on
    // `legacy` when a bit inside a payload stops the chain verifying.
    assert_eq!(
        report["journal_format"],
        serde_json::json!(support::CREATED_JOURNAL_FORMAT.kind())
    );
    assert!(
        report["journal_chain_break_at"].is_number(),
        "naming the byte the chain stopped verifying at: {report}"
    );
    assert_ne!(
        code, 0,
        "a project in this state is not one to exit 0 about"
    );
}

/// 🔴 **`req/227` H-01, the consequence** — a replaced `Committed` record must not put an old
/// delta back on the substrate.
///
/// This is the arm the audit's §0 is about. The rewrite is the same `cp` as `h01_a…`; what is
/// measured here is the **file on the disk**. Before the repair: the restart succeeded,
/// `runtime.recover={terminal:2,resumed:1}`, the server printed "43 §7-3 resumed 1
/// transformation(s) at start-up; the adapter was asked again", and `target.txt` went from `three`
/// back to `one` — with `/healthz` `200`, `ledger_agrees: true` and `gx repair` reporting a healthy
/// project on both sides of it.
#[test]
fn h01_a_replaced_committed_record_does_not_write_back_to_the_substrate() {
    let fixture = three_commits("r5_writeback");
    let journal = journal_path(&fixture);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let commits = committed_indexes(&bytes);
    assert_eq!(commits.len(), 3, "three commits, three records");
    let at = copy_record_over(&journal, commits[0], commits[2]);
    println!(
        "COMMITTED_REPLACED onto={} from={} at={at}",
        commits[0], commits[2]
    );
    assert_eq!(fixture.target_contents(), "three\n", "before the restart");

    match Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id) {
        Ok(server) => {
            let (status, body) = server.request("GET", "/v1/healthz", None);
            println!("RESTARTED_ANYWAY healthz={status} body={body}");
            assert_eq!(
                status, 500,
                "a server that starts on this project must at least refuse to serve from it: \
                 {body}"
            );
            shut_down(server);
        }
        Err(why) => {
            println!("RESTART_REFUSED stderr={}", why.trim());
            assert!(
                why.contains("LEDGER_DISAGREES") || why.contains("journal"),
                "and the refusal names the journal: {why}"
            );
        }
    }

    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "🔴 `req/227` H-01: the operator's file went from `three` to `one` here, because 43 §7's \
         recovery re-applied the delta of a commit whose record had been replaced. A restart is \
         allowed to refuse; it is not allowed to write"
    );

    let (_, report) = repair_report(&fixture, false);
    assert_eq!(report["journal_intact"], serde_json::json!(false));
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "and neither is the diagnosis"
    );
}

// ---------------------------------------------------------------------------
// The audit's middle band
// ---------------------------------------------------------------------------

/// 🔴 **`req/227` M-01** — `gx repair`'s `journal_intact` could not be `false`.
///
/// The field was initialised to `true` at `Engine::open` and re-derived by comparing the file
/// against **this process's own** read offset, so a verb that opens, catches up once and prints the
/// answer was asking a tautology. The audit's probe B4 destroyed the framing in the middle of a
/// journal and got `{"recovery":{"journal":{"torn_tail_bytes":2315,…}},"journal_intact":true,…}`
/// out of the same JSON object, with a remedy sentence sending the operator to look at
/// `.gx/ledger/` — the file that had not moved.
#[test]
fn m01_a_repair_report_on_a_damaged_journal_says_the_journal_is_not_intact() {
    let fixture = three_commits("r5_m01");
    let journal = journal_path(&fixture);
    // Destroy the framing in the middle: a length header that promises more than is there.
    let mut bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&bytes);
    let (at, _) = spans[spans.len() / 2];
    bytes[at..at + 4].copy_from_slice(&[0x00, 0x0f, 0xff, 0xff]);
    std::fs::write(&journal, &bytes).expect("write the journal back");
    println!("FRAMING_DESTROYED at={at}");

    let (code, report) = repair_report(&fixture, false);
    println!("M01_REPORT={report}");
    assert_eq!(
        report["journal_intact"],
        serde_json::json!(false),
        "🔴 `req/227` M-01: the one key 44 §1.2 v0.4-q added so that a machine could tell which \
         file to look at, answering `true` about the file that moved"
    );
    assert!(
        report["recovery"]["journal"]["torn_tail_bytes"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "the two answers are about the same file and agree: {report}"
    );
    let remedy = report["remedy"].as_str().unwrap_or_default();
    println!("M01_REMEDY={remedy}");
    assert!(
        remedy.contains("journal"),
        "and the remedy names the journal rather than sending the operator to `.gx/ledger/`: \
         {remedy}"
    );
    assert_ne!(code, 0);
}

/// 🔴 **`req/227` M-02** — the reader's door made two directories.
///
/// 44 §1.2 v0.4-q declared that a report writes exactly one thing, the lock's holder note. The
/// audit deleted `<journal>.blobs` and `<journal>.observations` from a project, ran the report, and
/// watched both grow back: the declaration was a count of one and the truth was three.
#[test]
fn m02_a_repair_report_does_not_create_the_blob_and_observation_directories() {
    let fixture = three_commits("r5_m02");
    let layout = layout(&fixture);
    let blobs = PathBuf::from(format!("{}.blobs", layout.journal_path().display()));
    let observations = PathBuf::from(format!("{}.observations", layout.journal_path().display()));
    assert!(
        blobs.is_dir() && observations.is_dir(),
        "the fixture has both"
    );
    std::fs::remove_dir_all(&blobs).expect("remove the blob directory");
    std::fs::remove_dir_all(&observations).expect("remove the observation directory");

    let (code, report) = repair_report(&fixture, false);
    println!(
        "M02 blobs={} observations={} exit={code}",
        blobs.exists(),
        observations.exists()
    );
    assert!(
        !blobs.exists(),
        "🔴 `req/227` M-02: a report that promises to write nothing created {blobs:?}"
    );
    assert!(!observations.exists(), "🔴 and {observations:?}: {report}");
}

/// 🔴 **`req/227` M-03** — the only door out of a damaged project did not open on a read-only
/// filesystem.
///
/// The report took `.gx/LOCK` before it read `--yes`, and `ProcessLock::open` creates the file, so
/// a snapshot or a backup — exactly what an investigator has — answered
/// `{"gx_code":"INTERNAL","detail":"cannot open the writer lock … Permission denied"}`. `INTERNAL`
/// is 44 §2.3's word for "not classifiable" and this is entirely classifiable.
#[test]
fn m03_a_repair_report_opens_on_a_read_only_filesystem() {
    if unsafe { libc_geteuid() } == 0 {
        println!("M03_SKIPPED: running as root, where `chmod a-w` does not stop a write");
        return;
    }
    let fixture = three_commits("r5_m03");
    let root = layout(&fixture).root().to_path_buf();
    let chmod = Command::new("chmod")
        .args(["-R", "a-w"])
        .arg(&root)
        .status()
        .expect("chmod is available");
    assert!(chmod.success(), "the fixture needs a read-only .gx/");

    let (code, report) = repair_report(&fixture, false);
    println!("M03_REPORT exit={code} json={report}");
    let restore = Command::new("chmod")
        .args(["-R", "u+w"])
        .arg(&root)
        .status()
        .expect("chmod is available");
    assert!(restore.success(), "and the scratch tree is writable again");

    assert_ne!(
        report["gx_code"],
        serde_json::json!("INTERNAL"),
        "🔴 `req/227` M-03: the diagnosis refused to run on the copy an investigator holds"
    );
    assert!(
        report["project"].is_string(),
        "a full report came out: {report}"
    );
    assert_eq!(
        report["lock_held"],
        serde_json::json!(false),
        "and it says it was produced without the lock: {report}"
    );
}

// `geteuid(2)`, without a dependency: `libc` is not in this crate's tree and one function does not
// justify adding it, and the symbol is in libc, which every Rust binary on unix links. A plain
// comment rather than a doc comment because rustdoc does not document an extern block, and this
// crate builds with no warnings.
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

/// 🔴 **`req/227` M-04** — the report opened on a narrower set of projects than the repair.
///
/// A project missing `.gx/ledger/journal.verdicts` answered `INTERNAL` to `gx repair` and `0` to
/// `gx repair --yes`, which grew the file back and then reported the project healthy. The verb that
/// writes nothing must not be the one that refuses to look.
#[test]
fn m04_a_repair_report_opens_a_project_whose_verdict_chain_is_absent() {
    let fixture = three_commits("r5_m04");
    let verdicts = PathBuf::from(format!(
        "{}.verdicts",
        layout(&fixture).journal_path().display()
    ));
    assert!(verdicts.is_file(), "the fixture has a verdict chain");
    std::fs::remove_file(&verdicts).expect("remove the verdict chain");

    let (code, report) = repair_report(&fixture, false);
    println!("M04_REPORT exit={code} json={report}");
    assert_ne!(
        report["gx_code"],
        serde_json::json!("INTERNAL"),
        "🔴 `req/227` M-04: the diagnosis refused a project that is missing a file"
    );
    assert_eq!(
        report["verdict_chain_present"],
        serde_json::json!(false),
        "and it says the file is absent rather than being quietly given an empty one: {report}"
    );
    assert!(
        !verdicts.exists(),
        "🔴 and the report did not create it either"
    );
}

/// 🔴 **`req/227` M-06** — a key file whose name and contents disagree.
///
/// The store opened `<key_id>.key` and answered with whatever key was inside. The audit put an
/// actor key's bytes under the engine key's name and watched `gx undo` refuse with `… does not
/// verify under the key it names, "ed25519-833a…"` — while the receipt named the *other* id, the
/// one the store had been asked for. The sentence was false about the document it was about.
#[test]
fn m06_a_key_file_whose_name_and_contents_disagree_is_refused_by_both_ids() {
    let fixture = three_commits("r5_m06");
    let second = fixture.another_key();
    let keys = fixture.home.join(".gx").join("keys");
    let mine = keys.join(format!("{}.key", fixture.key_id));
    let theirs = keys.join(format!("{second}.key"));
    let donor = std::fs::read(&theirs).expect("read the second key");
    std::fs::write(&mine, &donor).expect("put the second key under the first key's name");
    println!("KEY_SWAPPED name={} contents={second}", fixture.key_id);

    let run = support::run(
        fixture
            .gx()
            .arg("repair")
            .args(["--signing-key", &fixture.key_id])
            .arg("--yes"),
    );
    println!("M06 exit={} stderr={}", run.code, run.stderr.trim());
    assert_ne!(
        run.code, 0,
        "a store whose map does not hold is not a store"
    );
    assert!(
        run.stderr.contains(&fixture.key_id),
        "the refusal names the id that was asked for — the one every receipt spells: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains(&second),
        "and the id that is actually in the file: {}",
        run.stderr
    );
}

/// 🔴 **`req/227` M-07** — the refusal on a deployment that keeps no receipt archive pointed at a
/// directory that deployment does not have.
///
/// Measured by an embedder in the audit's probe D: `commit` `200` with a signed receipt, then
/// `undo` `409` — "this deployment keeps no receipt archive (… restore the commit receipt under
/// `.gx/receipts/` …)". There is no `.gx/receipts/` there, and putting one there would change
/// nothing: `NoArchive::load_commit` answers `None` by construction.
///
/// Asserted on the vocabulary rather than through HTTP, because reaching `NoArchive` needs an
/// embedder that does not call `with_archive` and `gx serve` always does. The words are what the
/// audit measured, and they are where the repair is.
#[test]
fn m07_the_no_archive_refusal_does_not_send_an_operator_to_a_directory_that_is_not_there() {
    use gx_engine::WitnessMissing;
    let no_archive = WitnessMissing::NoArchive.remedy();
    println!("M07_NO_ARCHIVE={no_archive}");
    assert!(
        !no_archive.contains(".gx/receipts/"),
        "🔴 `req/227` M-07: {no_archive}"
    );
    assert!(
        no_archive.contains("with_archive"),
        "and it names what a deployment would have to declare instead: {no_archive}"
    );
    for missing in [
        WitnessMissing::NoReceipt,
        WitnessMissing::Unreadable,
        WitnessMissing::Unsigned,
        WitnessMissing::UnknownKey,
        WitnessMissing::WrongSubject,
    ] {
        let remedy = missing.remedy();
        assert!(
            remedy.contains(".gx/receipts/"),
            "the other five are about a document under `.gx/receipts/` and still say so: {remedy}"
        );
    }
}

// ---------------------------------------------------------------------------
// This lane's attacks on this lane's own code (`req/38` §163 ruling 1)
// ---------------------------------------------------------------------------

/// 🔴 **S-1 — a journal whose chain has been recomputed end to end.**
///
/// Every other probe here attacks a file whose links no longer fit. The obvious attack on a chain
/// is to **rebuild it**: drop a record and recompute every link from the genesis, so the file that
/// results verifies perfectly and is not the file gx wrote. Nothing inside the journal can catch
/// that — a chain is an argument about a file's own consistency, and this file is consistent.
///
/// So the answer has to come from outside the journal, and this arm is what says whether it does.
/// The record dropped is the first commit's `Committed`: the ledger still holds three leaves, the
/// journal now witnesses two, and 43 §7's recovery is handed a row that looks like an unfinished
/// commit whose leaf has two later leaves behind it. The refusal is `Engine::recover`'s and not the
/// chain's — which is exactly why the two mechanisms are different.
#[test]
fn s1_a_journal_whose_chain_was_recomputed_is_still_refused_by_the_ledger() {
    let fixture = three_commits("r5_s1");
    let journal = journal_path(&fixture);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&bytes);
    let drop = committed_indexes(&bytes)[0];
    println!("FORGING drop={drop} of {} records", spans.len());

    // Rebuild the file: the marker, then every surviving payload, re-linked from the genesis.
    let mut forged = gx_engine::JOURNAL_MAGIC.to_vec();
    let mut chain = gx_engine::replay::genesis_link();
    for (i, (at, len)) in spans.iter().enumerate() {
        if i == drop {
            continue;
        }
        let payload = &bytes[at + 4..at + len - 32];
        forged.extend_from_slice(&bytes[*at..at + 4]);
        forged.extend_from_slice(payload);
        chain = gx_engine::replay::link(&chain, payload);
        forged.extend_from_slice(&chain);
    }
    std::fs::write(&journal, &forged).expect("write the forged journal");
    let replayed = gx_engine::replay(&forged);
    println!(
        "FORGED records={} chain_break={:?} bytes={}",
        replayed.records().len(),
        replayed.chain_break().map(|b| b.at),
        forged.len()
    );
    assert!(
        replayed.chain_break().is_none(),
        "the forgery is a **valid** chain: if this fails the probe is not attacking what it claims"
    );

    match Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id) {
        Ok(server) => {
            let (status, body) = server.request("GET", "/v1/healthz", None);
            println!("S1_RESTARTED healthz={status} body={body}");
            assert_eq!(status, 500, "the pair still disagrees: {body}");
            shut_down(server);
        }
        Err(why) => {
            println!("S1_REFUSED stderr={}", why.trim());
            assert!(
                why.contains("did not resume") || why.contains("LEDGER_DISAGREES"),
                "and the refusal is about the ledger and the recovery: {why}"
            );
        }
    }
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "🔴 S-1: a perfectly-chained journal must not be able to steer the recovery into writing \
         an old delta back. The ledger is the last thing standing here and it is what stands"
    );
}

/// 🔴 **S-2a — a journal in the old format still opens, and is reported as what it is.**
///
/// DR-43-9 changes the framing of a file that operators already have. A repair that made every
/// existing project unopenable would be a worse accident than the one it fixes, so the format is
/// sniffed and a legacy journal is read and appended to in its own framing. What it does **not**
/// get is a chain, and the report says so rather than letting `journal_intact: true` imply an
/// answer nobody computed.
#[test]
fn s2a_a_journal_written_before_the_chain_still_opens_and_is_reported_as_legacy() {
    let fixture = three_commits("r5_s2a");
    let journal = journal_path(&fixture);
    // 🔴 **R6 / `req/229` H-02** — an old project has **no declaration**, and this is now what makes
    // it one.
    //
    // R5 built this fixture by stripping the chain off a project this binary had just written, and
    // that was the whole of "an old project" because nothing recorded what the project was.
    // `req/229` H-02 measured the cost of that: the same file is also what a **downgrade** produces,
    // and gx could not tell them apart, so a chained project could be turned into a legacy one and
    // accepted with no warning. R6 records the framing in `.gx/VERSION`, so the fixture has to say
    // what it is claiming to be — and what this arm claims is a project written before that record
    // existed. Removing the declaration is therefore part of building the fixture, not a weakening
    // of it: 42 §3.13 v0.4-r's backward compatibility is what this arm defends, and it is unmoved.
    let version = fixture.project.join(".gx").join("VERSION");
    let recorded = std::fs::read_to_string(&version).expect("read VERSION");
    let first = recorded.lines().next().expect("a version line").to_string();
    std::fs::write(&version, format!("{first}\n")).expect("a project from before the declaration");
    strip_the_chain(&journal);
    // The recorded head names a journal longer than the stripped one, and an old project has no
    // recorded head either (`.gx/checkpoints/` was empty in every project before R6 — `req/229`
    // L-02). Removing it is the same fixture-building step as the line above.
    let head = gx_cli::layout::Layout::open(&fixture.project)
        .expect("the project is open")
        .head_path();
    if head.exists() {
        std::fs::remove_file(&head).expect("an old project keeps no head");
    }

    let (code, report) = repair_report(&fixture, false);
    println!("S2A_REPORT exit={code} json={report}");
    assert_eq!(report["journal_format"], serde_json::json!("legacy"));
    assert_eq!(
        report["journal_intact"],
        serde_json::json!(true),
        "a file with no links cannot contradict itself, and saying otherwise would make every \
         existing project look damaged: {report}"
    );
    assert_eq!(code, 0, "and the project is usable: {report}");

    let submitted = fixture.submit("four\n");
    println!(
        "S2A_SUBMIT exit={} stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );
    assert_eq!(submitted.code, 0, "including for writing");
    let after = std::fs::read(&journal).expect("read the journal");
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — "not chained" rather than "not
    // `JOURNAL_MAGIC`". There are two chained markers now, and naming one of them would let an
    // append that rewrote this legacy file into `GXJRNL02` framing pass the check the arm exists
    // for. The claim is unchanged and is now whole.
    assert!(
        !gx_engine::replay(&after).format().is_chained(),
        "and the append stayed in the file's own framing rather than rewriting an append-only file"
    );
}

/// 🔴 **S-2b — the same substitution, on a legacy journal, must still not reach the substrate.**
///
/// This is the honest half of the format decision. A legacy journal has no chain, so H-01's
/// rewrite is **invisible** to the detector — and the arm asserts that it is, rather than pretending
/// otherwise. What must still hold is the consequence: 43 §7's recovery is not allowed to write an
/// old delta back, and the gate that stops it there is the ledger's (`Engine::recover`'s second
/// question), which needs no chain.
#[test]
fn s2b_a_legacy_journal_still_does_not_let_a_recovery_write_back() {
    let fixture = three_commits("r5_s2b");
    let journal = journal_path(&fixture);
    // 🔴 **R6** — see `s2a_…` for why building "an old project" now means removing the declaration
    // and the recorded head as well as the chain. The arm's claim is unchanged: on a journal with
    // no chain the rewrite is invisible, and the write-back is refused anyway.
    let version = fixture.project.join(".gx").join("VERSION");
    let recorded = std::fs::read_to_string(&version).expect("read VERSION");
    let first = recorded.lines().next().expect("a version line").to_string();
    std::fs::write(&version, format!("{first}\n")).expect("a project from before the declaration");
    let head = gx_cli::layout::Layout::open(&fixture.project)
        .expect("the project is open")
        .head_path();
    if head.exists() {
        std::fs::remove_file(&head).expect("an old project keeps no head");
    }
    strip_the_chain(&journal);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let commits = committed_indexes(&bytes);
    copy_record_over(&journal, commits[0], commits[2]);
    println!(
        "LEGACY_COMMITTED_REPLACED onto={} from={}",
        commits[0], commits[2]
    );

    match Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id) {
        Ok(server) => {
            let (status, body) = server.request("GET", "/v1/healthz", None);
            println!("S2B_RESTARTED healthz={status} body={body}");
            shut_down(server);
        }
        Err(why) => println!("S2B_REFUSED stderr={}", why.trim()),
    }
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "🔴 S-2b: on a legacy journal the rewrite is not detectable, and the write-back is still \
         refused — the two halves of the repair are independent on purpose"
    );
}

/// Rewrite a chained journal into the framing that predates DR-43-9.
///
/// The marker goes, every 32-byte link goes, and every payload stays exactly as it was. That is
/// what a journal written by an earlier `gx` looks like, byte for byte.
///
/// 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the entry check asks the engine which
/// framing the file is in instead of spelling `JOURNAL_MAGIC`. M-02 gave a journal this build
/// creates the `GXJRNL02` marker, so "starts with the v1 marker" had become false of every fixture
/// this helper is handed, and the helper panicked on its own precondition. What did **not** change:
/// the file still has to carry a chained marker **at offset zero** — the check is `starts_with`
/// against whichever marker the sniffed format names — so a legacy file and an empty one are
/// refused here exactly as before, and a probe that thought it was downgrading a chained journal
/// cannot quietly be downgrading nothing. The contract is the same too, a legacy journal out of a
/// chained one. The body needs no version: both markers are eight bytes and share the four bytes
/// `frames` sniffs on.
fn strip_the_chain(path: &Path) {
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
    println!("CHAIN_STRIPPED from={} to={}", bytes.len(), legacy.len());
}

/// 🔴 **S-3 — a record deleted, with the file's length made up at the end.**
///
/// The attacks above keep every record. This one removes a whole frame and pads the tail so that
/// the file weighs what it weighed, which is the shape a length-based detector is blindest to and
/// which the chain sees for a different reason: the record after the hole no longer links to the
/// record before it.
#[test]
fn s3_a_record_deleted_and_the_tail_padded_is_refused() {
    let fixture = three_commits("r5_s3");
    let journal = journal_path(&fixture);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let spans = frames(&bytes);
    let (at, len) = spans[spans.len() / 2];
    let mut cut = bytes[..at].to_vec();
    cut.extend_from_slice(&bytes[at + len..]);
    cut.extend(std::iter::repeat_n(0u8, len));
    assert_eq!(cut.len(), bytes.len(), "the file weighs what it weighed");
    std::fs::write(&journal, &cut).expect("write the journal back");
    println!("RECORD_DELETED at={at} len={len} padded={len}");

    match Serving::try_start(&fixture.project, &fixture.home, &fixture.key_id) {
        Ok(server) => {
            let (status, body) = server.request("GET", "/v1/healthz", None);
            println!("S3_RESTARTED healthz={status} body={body}");
            assert_eq!(status, 500, "a hole is a hole: {body}");
            shut_down(server);
        }
        Err(why) => println!("S3_REFUSED stderr={}", why.trim()),
    }
    let (_, report) = repair_report(&fixture, false);
    assert_eq!(
        report["journal_intact"],
        serde_json::json!(false),
        "and the report says the journal is what moved: {report}"
    );
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "and nothing was re-applied on the way through"
    );
}

/// 🔴 **S-4 — the loss condition: a server and a CLI writing in turn must not trip the chain.**
///
/// Every other arm attacks a detector for being blind. This one attacks it for being **loud**: the
/// chain is now verified on the read road as well as the write road, so a false positive would not
/// merely refuse a write — it would make `/healthz` answer `500` for a project that is perfectly
/// well, and R2's whole "a server and a CLI can share a project" claim would die with it.
///
/// Three rounds of (HTTP commit → `/healthz` → CLI submit → `/healthz`), which is exactly the road
/// where one process folds records the other wrote and has to continue the other's chain rather
/// than its own idea of it.
#[test]
fn s4_a_server_and_a_cli_writing_in_turn_do_not_trip_the_chain() {
    let fixture = pipeline("r5_s4", "before\n");
    let locator = fixture.target.display().to_string();
    assert_eq!(fixture.submit("warm\n").code, 0);
    let server = Serving::start(&fixture.project, &fixture.home, &fixture.key_id);

    for round in 0..3 {
        server.commit_over_http(&locator, &format!("http-{round}\n"), &fixture.key_id);
        let (a_status, a_body) = server.request("GET", "/v1/healthz", None);
        println!("S4 round={round} after_http={a_status}");
        assert_eq!(a_status, 200, "after the server's own write: {a_body}");

        let submitted = fixture.submit(&format!("cli-{round}\n"));
        assert_eq!(
            submitted.code, 0,
            "the CLI writes into the same journal: {}",
            submitted.stderr
        );
        let (b_status, b_body) = server.request("GET", "/v1/healthz", None);
        println!("S4 round={round} after_cli={b_status} body={b_body}");
        assert_eq!(
            b_status, 200,
            "🔴 S-4: the server folded records another process wrote and called them damage: \
             {b_body}"
        );
    }
    shut_down(server);
}

/// 🔴 **S-5 — the file this lane refuses to truncate.**
///
/// DR-43-7 quarantines a torn tail and cuts it, because the next append has to land where the
/// record sequence actually reached. A chain break is **not** a torn tail — everything after it is
/// whole — so `EngineJournal::open` leaves it alone, and this arm is the falsifier for the new risk
/// that decision creates: a repair lane that cut here would delete every record written after
/// somebody's edit and call it a repair, which is `req/225` H-01's amputation with a new name.
///
/// So: a project whose chain is broken, and the byte-for-byte file across a report **and** a
/// `--yes` repair.
#[test]
fn s5_a_chain_break_is_never_truncated_by_opening_the_project() {
    let fixture = three_commits("r5_s5");
    let journal = journal_path(&fixture);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let commits = committed_indexes(&bytes);
    copy_record_over(&journal, commits[0], commits[1]);
    let broken = std::fs::read(&journal).expect("read the journal");

    let (_, report) = repair_report(&fixture, false);
    let after_report = std::fs::read(&journal).expect("read the journal");
    assert_eq!(
        broken, after_report,
        "🔴 S-5: the report moved bytes in a file it cannot repair: {report}"
    );

    let (code, yes_report) = repair_report(&fixture, true);
    let after_yes = std::fs::read(&journal).expect("read the journal");
    println!("S5_YES exit={code} json={yes_report}");
    assert_eq!(
        broken, after_yes,
        "🔴 S-5: `--yes` truncated at a chain break. Everything after one is whole, and gx cannot \
         put back what it cuts"
    );
    let torn: Vec<String> = std::fs::read_dir(layout(&fixture).join("ledger"))
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.contains(".torn."))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        torn.is_empty(),
        "and no quarantine copy was made either, because nothing was removed: {torn:?}"
    );
    assert_eq!(
        fixture.target_contents(),
        "three\n",
        "and `--yes` did not re-apply anything on the way through"
    );
}
