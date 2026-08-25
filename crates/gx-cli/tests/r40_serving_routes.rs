// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R40 / `req/553` L-02 + L-01 (`req/38` §322-2 (11-1)/(11-3), §328) — the serving routes have
//! no degraded context, and this is the probe that makes the limit a measurement.**
//!
//! # The fact a buyer receives
//!
//! Cut a project's last committed frame and its journal and ledger describe different trees. Ask
//! that server: `GET /healthz` says **500**, and so do `GET /ledger/proof`, `GET /ledger/consistency`
//! and `GET /ledger/checkpoint` — four mouths refusing to say anything about which tree this is.
//! Ask it `GET /receipts/{tid}` in the same second and it answers **200**, byte for byte what a
//! healthy project answers, with nothing in the response saying the project it came from is
//! refusing every other question about itself.
//!
//! # Why that is the right behaviour, and why it still needed writing down
//!
//! `layout.rs`'s `journal_absent` remedy promises it in as many words: *"`gx repair` reads the
//! ledger, the commit receipts and the recorded head out of their own files"*, and *"`gx receipt
//! verify --offline` **still proves what was committed**"*. A receipt is a signed statement about
//! one transformation; it does not become false because a later frame was cut, and the third-party
//! verifier who holds it is the caller the whole `--offline` road exists for. Putting
//! `ledger_agrees` in front of these two routes would kill that promise to close nothing —
//! **issuing is not serving** (`req/38` §322-2 (11-1)).
//!
//! So `req/38` §328 ruling 2 (b) took option (iv) of `req/556` R-3a: **declare the limit and drive
//! it**, rather than add a `degraded` key to the wire (44 §2.2's four-key contract, `wire_census`,
//! DR-44-9's byte-for-byte views) or a header (`problem.rs`: *"`BUSY` carries `Retry-After`, and no
//! other code does"*). A limit that is only prose is a limit that rots; this file is the half that
//! cannot.
//!
//! # 🔴 The falsifier, pre-committed
//!
//! `req/556` R-3c: option (iv) rests on "a consumer who wants to know can ask `/healthz`". **If any
//! consumer is observed rendering `/receipts` without consulting `/healthz` — a GUI, an SDK sample,
//! a buyer's integration — the limit is false and the design escalates to option (iii), the header.**
//! That is written here rather than only in the report so that the next lane to touch this file
//! reads the condition under which it has to change.
//!
//! # What this suite does not do
//!
//! It does not change CLI or HTTP behaviour by one byte, and `a3` measures that: the served bytes
//! either side of the cut are identical. `req/38` §322-2 (11-3) left the CLI/HTTP asymmetry at L,
//! so `a4` records it as a driven fact rather than repairing it.
//!
//! # 🔴 **The falsifier above fired, and `a3` is now the control on its repair** (`req/38` §350 item 4, §369 item 1)
//!
//! Everything above stands as written — it is the record of a decision that was correct on the
//! evidence it had, and no-delete keeps it legible. What has changed is the world: the consumer the
//! condition named was **observed**, and it is this repository's own. `req/566` G-2 and `req/578` §5
//! independently measured `sdk/typescript/src/client.ts`'s `getReceipt` going straight to
//! `/receipts/{tid}` and calling `healthz()` nowhere, and the suite that exercises it
//! (`sdk/typescript/test/audit_m9_p4_tamper_and_errors.test.mjs`) renders a served receipt without
//! asking. `req/38` §350 item 4 ruled the falsifier fired; §369 item 1 ruled the repair.
//!
//! 🔴 **The repair is not the header this file predicted.** The escalation sentence above names
//! option (iii); §369 item 1 took option (ii) instead — a fourth **body** key, `server_health`
//! (`{status, status_reason}`), always present, on `GET /receipts/{tid}` alone
//! (`crates/gx-api/src/handlers.rs::server_health`, 44 §2.2's v0.5-x note). A header would have been
//! invisible to exactly the reader this is for: the SDK hands `response.json()` to its caller, and a
//! consumer who does not know to ask `/healthz` is a consumer who does not know to read a header
//! either. The prediction is left standing above rather than rewritten, because *which* option a
//! pre-committed falsifier ends up buying is itself worth being able to read back.
//!
//! ∴ `a3` no longer asserts that the whole body is unchanged — the whole point of the repair is
//! that one member of it changes. It asserts the **sharper** claim, which is the one the old
//! assertion was a proxy for: `envelope` (the signed document, PAE = `payload_type` + `payload`,
//! 42 §3.10) and `issued_at` and `receipt_view` are byte-for-byte identical either side of the cut,
//! and `server_health` is the **only** member that moved. That is a stronger control on
//! "the view stands beside the document and does not alter it" than equality of the whole body was,
//! because equality of the whole body was also satisfiable by a build that answered nothing at all.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[path = "support/mod.rs"]
mod support;

use support::Pipeline;

/// How long a socket read may block before the probe fails.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server has to print its start-up line.
const STARTUP_WAIT: Duration = Duration::from_secs(20);

/// A running `gx serve` and its address.
///
/// The ninth copy of `ac_056.rs`'s shape, copied for the reason the second one gives: a test binary
/// is its own crate. 🔴 The port is read out of the server's own start-up line rather than guessed
/// or slept for — `req/556` §4 AC-13 forbids a fixed sleep, because a probe that waits a constant
/// is a probe that passes on a fast machine and hangs on a loaded one.
struct Serving {
    child: Child,
    addr: String,
    token: String,
}

impl Serving {
    fn start(project: &Path, home: &Path, key_id: &str) -> Self {
        let token = "r40-serving-token".to_string();
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
                panic!("gx serve was expected to serve and did not: {why}");
            }
        }
        let start: serde_json::Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("44 §1.2 asks for one start-up line; got {line:?} ({e})"));
        println!("R40_SERVE_START={start}");
        let addr = start["bind"]
            .as_str()
            .expect("the bound address")
            .to_string();
        Self { child, addr, token }
    }

    /// One HTTP/1.1 GET on its own connection, read to the end. Returns the status and the body.
    fn get(&self, path: &str) -> (u16, String) {
        let mut socket = TcpStream::connect(&self.addr).expect("connect to the server");
        socket
            .set_read_timeout(Some(READ_TIMEOUT))
            .expect("a read timeout, so an expiry is a failure and not a hang");
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
            self.addr, self.token
        );
        socket.write_all(request.as_bytes()).expect("write");
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).expect("read the response");
        let text = String::from_utf8_lossy(&raw).to_string();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or_else(|| panic!("no status line in {text:?}"));
        let body = text
            .split_once("\r\n\r\n")
            .map(|(_, b)| b)
            .unwrap_or("")
            .to_string();
        (status, body)
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(self.child.id().to_string())
            .status();
        let _ = self.child.wait();
    }
}

fn journal_of(p: &Pipeline) -> PathBuf {
    p.project.join(".gx").join("ledger").join("journal")
}

/// The engine journal's frame boundaries — copied verbatim from `r38_ledger_face_width.rs`.
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

fn cut_last_frame(p: &Pipeline) {
    let journal = journal_of(p);
    let bytes = std::fs::read(&journal).expect("read the journal");
    let all = frames(&bytes);
    let idx = all.len().checked_sub(1).expect("the journal holds a frame");
    let at = if idx == 0 { all[0].0 } else { all[idx].0 } as u64;
    let before = bytes.len() as u64;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&journal)
        .expect("open for truncation")
        .set_len(at)
        .expect("truncate");
    let after = std::fs::metadata(&journal).expect("stat").len();
    assert!(
        after < before,
        "the cut removed nothing: {before} -> {after}"
    );
}

/// 🔴 `a1`/`a2`/`a3` — one project, one instant, two answers, and the served bytes do not move.
#[test]
fn a_receipt_is_served_at_200_while_the_same_server_reports_500_about_its_own_tree() {
    let p = support::pipeline("r40_serving", "before\n");
    let tid = p.commit_one("first");
    let route = format!("/v1/receipts/{}", tid.replace(':', "%3A"));

    // 🔴 **The bed is one running server, and the cut happens underneath it.**
    //
    // R40's first draft of this probe started a second server on the cut project and it would not
    // start: `gx serve` refuses `LEDGER_DISAGREES` at start-up, saying *"a server that started here
    // would sign checkpoints over a tree its own journal contradicts, so it refuses"*. That refusal
    // is R16's and it is correct — and it means the state `req/553` L-02 measured is only reachable
    // the way L-02 reached it, by moving the file under a server that is already up. Which is also
    // the shape a buyer meets: nobody starts a server on a broken project on purpose; a project
    // breaks while something is serving it.
    //
    // 🔴 So this arm carries a second fact for free, and asserts it below: **`gx serve` will not
    // start on a disputed project**, which is why the limit this file drives is about a server that
    // was already running rather than about a door left open.
    let serving = Serving::start(&p.project, &p.home, &p.key_id);
    let (health_before, health_body_before) = serving.get("/v1/healthz");
    let (receipt_before, receipt_body_before) = serving.get(&route);
    println!("R40_SERVING healthy healthz={health_before} receipt={receipt_before}");
    assert_eq!(health_before, 200, "the healthy project is well");
    assert_eq!(receipt_before, 200, "and serves its receipt");
    assert!(
        health_body_before.contains("\"ok\""),
        "the healthy body says so: {health_body_before}"
    );

    cut_last_frame(&p);

    // 🔴 `a2` — the measurement `req/553` L-02 asked for, on **one server** at one instant.
    let (health_after, health_body_after) = serving.get("/v1/healthz");
    let (proof, _) = serving.get("/v1/ledger/proof?leaf=0");
    let (consistency, _) = serving.get("/v1/ledger/consistency?from=1&to=1");
    let (receipt_after, receipt_body_after) = serving.get(&route);
    println!(
        "R40_SERVING degraded healthz={health_after} proof={proof} consistency={consistency} \
         receipt={receipt_after}"
    );
    assert_eq!(
        health_after, 500,
        "the same server refuses to say this project is well: {health_body_after}"
    );
    assert_eq!(proof, 500, "and refuses to prove anything about the tree");
    assert_eq!(consistency, 500, "and refuses to compare two of them");
    assert_eq!(
        receipt_after, 200,
        "🔴 and hands over the receipt anyway — `req/553` L-02, and the promise `layout.rs`'s \
         `journal_absent` remedy makes to the offline verifier"
    );

    // 🔴 `a3` — **`req/38` §369 item 1 (L-02)**. This read, until this lane:
    //
    //     assert_eq!(receipt_body_before, receipt_body_after,
    //         "the served receipt is byte-for-byte what it was");
    //
    // and its comment said that option (ii)/(iii) of `req/556` R-3a "would both make this assertion
    // fail, which is what makes it a control on the decision rather than a restatement of it". The
    // falsifier fired, §369 item 1 took option (ii), and so this assertion is doing exactly what it
    // was built to do: it goes red when the decision moves. It is replaced by the claim it was a
    // proxy for, which is the one that still has to hold.
    let before: serde_json::Value =
        serde_json::from_str(&receipt_body_before).expect("the served receipt is JSON");
    let after: serde_json::Value =
        serde_json::from_str(&receipt_body_after).expect("the served receipt is JSON");
    println!(
        "R40_SERVING health_in_band before={} after={}",
        before["server_health"], after["server_health"]
    );

    // 🔴 The signed document, and the two things derived from it, are untouched. `envelope` is
    // `{payload_type, payload, signatures}` — DSSE's PAE is `payload_type` + `payload` (42 §3.10) —
    // so this equality **is** the measurement that L-02 did not reach the signature. A `server_health`
    // folded into the signed bytes, or a re-mint on the way out, moves it.
    for untouched in ["envelope", "issued_at", "receipt_view"] {
        assert_eq!(
            before[untouched], after[untouched],
            "`{untouched}` is byte-for-byte what it was across the cut (DR-44-9: a view stands \
             **beside** the document and does not alter it; L-02 adds a member beside it and does \
             not reach into it either)"
        );
    }

    // 🔴 And `server_health` is the **one** member that moved — asserted as a set difference and
    // not as "the two I happen to name", so that a future key added to this endpoint without a
    // census update lands here as well.
    let moved: Vec<String> = before
        .as_object()
        .expect("an object")
        .keys()
        .chain(after.as_object().expect("an object").keys())
        .filter(|k| before[k.as_str()] != after[k.as_str()])
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        moved,
        vec!["server_health".to_string()],
        "exactly one member of the served receipt distinguishes a disputed server from a well one"
    );

    // 🔴 And it says the two words the reader needs, without the reader knowing to ask. This is the
    // whole of what `req/38` §350 item 4 bought: `/healthz` above is a **500** at this instant, and
    // an SDK that never calls it now learns the same fact off the document it did ask for.
    assert_eq!(
        before["server_health"]["status"], "ok",
        "the well server said so in band"
    );
    assert_eq!(
        after["server_health"]["status"], "unhealthy",
        "and the disputed one says so in band rather than only at `/healthz`"
    );
    assert!(
        after["server_health"]["status_reason"]
            .as_str()
            .is_some_and(|why| why.contains("ledger_agrees") && why.contains("gx repair")),
        "and names the condition and the way out: {}",
        after["server_health"]["status_reason"]
    );
    drop(serving);

    // 🔴 `a5` — and a server asked to **start** on this project refuses, which is the door R16
    // closed and the reason the arms above had to cut underneath a running one.
    let refused = std::process::Command::new(env!("CARGO_BIN_EXE_gx"))
        .env("HOME", &p.home)
        .env("USERPROFILE", &p.home)
        .arg("--project")
        .arg(&p.project)
        .arg("serve")
        .args(["--bind", "127.0.0.1:0"])
        // 🔴 The token file is passed even though this run is expected to fail. R40's first draft
        // omitted it and the run refused `VALIDATION_ERROR` — "`--token-file <PATH> is required`" —
        // so the control was measuring a missing argument and calling it a refused start-up. The
        // assertion below names the condition rather than only the exit status for that reason: a
        // control that only checks "it failed" passes for every wrong reason there is.
        .arg("--token-file")
        .arg(p.project.join("token"))
        .args(["--signing-key", &p.key_id])
        .output()
        .expect("gx serve runs");
    let said = String::from_utf8_lossy(&refused.stderr);
    println!("R40_SERVING start_on_cut rc={:?}", refused.status.code());
    assert_ne!(
        refused.status.code(),
        Some(0),
        "a server does not start here"
    );
    assert!(
        said.contains("LEDGER_DISAGREES"),
        "and it says which condition refused it: {said}"
    );
}

/// 🔴 `a4` — the CLI and the HTTP face answer one project differently, driven rather than implied.
///
/// `req/38` §322-2 (11-3) left this at L and R40 does not repair it: `GET /candidates/{id}` calls
/// `state.engine_refreshed()?` and catches up before it answers, and the CLI's read road asks
/// `ledger_agrees` first. On a merely-cut project the catch-up succeeds and the HTTP face answers
/// 200 where the CLI refuses `LEDGER_DISAGREES`. The limit is written in `docs/LIMITS.md`; this arm
/// is what keeps the sentence true of the tree.
#[test]
fn the_cli_and_the_http_face_answer_one_cut_project_differently() {
    let p = support::pipeline("r40_asymmetry", "before\n");
    // 🔴 The order is load-bearing: the candidate is planned **first** so its frames sit behind the
    // commit's, and the cut below takes the committed frame. Cutting a candidate's own frames
    // leaves the two files agreeing (audit 39 arm B measured that), which would make this probe
    // pass for the wrong reason.
    let candidate = p.planned_one("a candidate to read back");
    let tid = p.commit_one("and a commit to cut");

    // The server is up **before** the cut, for the reason `a5` above measures: `gx serve` will not
    // start on a project whose two files are in dispute.
    let serving = Serving::start(&p.project, &p.home, &p.key_id);
    cut_last_frame(&p);

    let cli = support::run(p.gx().args(["log", "proof", "--leaf", &tid]));
    println!("R40_ASYMMETRY cli_rc={} stderr={}", cli.code, cli.stderr);
    assert_ne!(cli.code, 0, "the CLI refuses this project");

    let (candidate_status, _) =
        serving.get(&format!("/v1/candidates/{}", candidate.replace(':', "%3A")));
    let (health, _) = serving.get("/v1/healthz");
    println!("R40_ASYMMETRY http_candidate={candidate_status} healthz={health}");
    assert_eq!(health, 500, "the server knows the tree is in dispute");
    assert_eq!(
        candidate_status, 200,
        "🔴 `req/553` L-01: and answers about a candidate anyway. Left at L by `req/38` §322-2 \
         (11-3) — recorded here so that a lane changing it changes this line too"
    );
}
