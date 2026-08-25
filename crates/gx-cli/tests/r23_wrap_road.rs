// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/312` H-01, on the real road** (`req/313` §1 items 1 and 2) — driven through the
//! binary, a real child MCP server and a real journal.
//!
//! # What the twenty-second adversarial audit measured, verbatim
//!
//! ```text
//! A22_CASFACE_ROAD verdict="Admit" doc_after_one_call="" doc_final="" receipts=2
//! A22_CASFACE_ARRIVAL call notes.restore file://…/doc.txt    (× 6, before the agent's notes.write)
//! A22_CASFACE_UNDO rc=4 doc_after_undo="" recovered=false
//!
//! A22_REFUSED_ROAD verdict="refused-before-verdict" doc_now="" receipts=0
//! A22_REFUSED_TEXT gx refused this call and **nothing was sent to the server**: the adapter
//!                  refused to invert: …
//! A22_REFUSED_ARRIVAL call notes.restore file://…/doc.txt   (× 2, notes.write never arrived)
//! ```
//!
//! Two halves of one finding, and `req/38` §225 ruling 1 says the second is the deciding one:
//!
//! * **H-01(a)** — the `$cas_read` soundness gate asked whether a read face is a **key** of
//!   `restores`. A catalogue names the tools it writes with in two places, and the audit used the
//!   other one — `restored_by`, the inverse. gx then called that tool from `snapshot`, before the
//!   transformation had a plan, and the document was empty before the agent's own effect was ever
//!   framed.
//! * **H-01(b)** — the sentence the agent was handed on the refusing road was *"gx refused this
//!   call and **nothing was sent to the server**"*, and the server's own arrival log holds two
//!   frames gx sent. A declaration `docs/LIMITS.md` can call a burden is one thing; a **runtime
//!   claim that is false** is a thing no section of that page permits.
//!
//! # What this suite drives, and what it deliberately does not
//!
//! The repair for (a) makes the audit's catalogue a **start-up error**, so the road that destroyed
//! the object is no longer reachable from a file — which means the sentence in (b) cannot be
//! measured on that road any more. It is measured on the road that is still reachable: a catalogue
//! whose CAS read face is a legitimate read tool, where gx reads the object before it refuses for
//! an unrelated declaration fault. That is the same predicate on a road that survives the first
//! repair, which is what makes the two repairs independent rather than one covering the other.
//!
//! # Red-first
//!
//! Every arm drives the shipped binary and reads bytes it produced. No symbol this lane created is
//! named, so the file compiles at `7261321` and fails on its assertions.
//!
//! `cfg(unix)` for the `chmod` on the launcher script, as every sibling suite says.

#![cfg(unix)]

mod support;

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

const BEFORE: &str = "the note as it stood before any agent touched it\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ENDPOINT: &str = "stdio://r23";

// ---------------------------------------------------------------------------
// The bed
// ---------------------------------------------------------------------------

struct Bed {
    pipeline: support::Pipeline,
    note: PathBuf,
    uri: String,
    launcher: PathBuf,
    arrivals: PathBuf,
}

fn bed(name: &str) -> Bed {
    let pipeline =
        support::pipeline_named(name, "a file this suite does not measure\n", "seed.txt");
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);
    let note = pipeline.project.join("note.txt");
    std::fs::write(&note, BEFORE).expect("write the note");
    let uri = format!("file://{}", note.display());
    let arrivals = pipeline.project.join("arrivals.log");
    let launcher = pipeline.project.join("r23-server.sh");
    std::fs::write(
        &launcher,
        format!(
            "#!/bin/sh\nexec \"{}\" {DEMO_SERVER_ARG}\n",
            env!("CARGO_BIN_EXE_gx")
        ),
    )
    .expect("write the launcher");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    Bed {
        pipeline,
        note,
        uri,
        launcher,
        arrivals,
    }
}

impl Bed {
    fn note_now(&self) -> String {
        std::fs::read_to_string(&self.note).unwrap_or_default()
    }

    /// 🔴 The **server's own** record of what reached it, not this suite's account of it.
    ///
    /// `GX_DEMO_LOG` is written by `gx __demo-notes-server` before each tool runs, which is what
    /// let the audit say "gx destroyed this object" rather than "the object is empty".
    fn arrivals(&self) -> Vec<String> {
        std::fs::read_to_string(&self.arrivals)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn catalogue(&self, name: &str, body: Value) -> PathBuf {
        let path = self.pipeline.project.join(name);
        std::fs::write(&path, serde_json::to_vec_pretty(&body).expect("json")).expect("catalogue");
        path
    }

    fn wrap_args(&self, extra: &[String]) -> Vec<String> {
        let mut args = vec![
            "--project".to_string(),
            self.pipeline.project.display().to_string(),
            "wrap".to_string(),
            "--endpoint".to_string(),
            ENDPOINT.to_string(),
            "--actor-key".to_string(),
            self.pipeline.key_id.clone(),
            "--actor-model".to_string(),
            "r23-probe".to_string(),
        ];
        args.extend(extra.iter().cloned());
        args.push("--".to_string());
        args.push(self.launcher.display().to_string());
        args
    }

    fn receipt_files(&self) -> usize {
        std::fs::read_dir(self.pipeline.project.join(".gx").join("receipts"))
            .map(|d| d.flatten().count())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// An agent on the other side of `gx wrap`
// ---------------------------------------------------------------------------

struct Agent {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    n: u64,
}

impl Agent {
    fn open(args: &[String], home: &Path, arrivals: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("GX_DEMO_LOG", arrivals)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the gx binary runs");
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let mut me = Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            n: 0,
        };
        me.ask(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "r23", "version": "0" },
            }),
        );
        let note = jsonrpc::notification("notifications/initialized", json!({}));
        jsonrpc::write_frame(me.stdin.as_mut().expect("open"), &note).expect("write");
        me
    }

    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.n += 1;
        let frame = jsonrpc::request(self.n, method, params);
        jsonrpc::write_frame(self.stdin.as_mut().expect("open"), &frame).expect("write");
        match jsonrpc::read_frame(&mut self.stdout).expect("read") {
            Some(line) => serde_json::from_str(&line).expect("JSON"),
            None => {
                let mut text = String::new();
                if let Some(mut err) = self.child.stderr.take() {
                    let _ = err.read_to_string(&mut text);
                }
                panic!("gx wrap closed stdout answering {method:?}: {text}")
            }
        }
    }

    fn close(mut self) -> String {
        self.stdin = None;
        let out = self.child.wait_with_output().expect("gx wrap exits");
        String::from_utf8_lossy(&out.stderr).to_string()
    }
}

/// Every `text` a `tools/call` answer carries, joined.
fn text_of(answer: &Value) -> String {
    answer["result"]["content"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn detail_of(stderr: &str) -> String {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|v| v["detail"].as_str().map(str::to_string))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// H-01(a) — the audit's catalogue, on the road that destroyed an object
// ---------------------------------------------------------------------------

/// 🔴 `req/312` H-01(a): the audit's declaration does not start a session, and the note it emptied
/// twice is untouched.
#[test]
fn a_cas_read_face_that_is_this_files_restore_tool_does_not_start_a_session() {
    let bed = bed("r23_h01_inverse");
    let catalogue = bed.catalogue(
        "catalogue-inverse-face.json",
        json!({
            "notes.write": { "restored_by": "notes.restore" },
            "$cas_read": {
                "file://": {
                    "by_tool": "notes.restore",
                    "arguments": { "uri": "resource", "contents": { "const": "" } }
                }
            }
        }),
    );
    let run = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &bed.pipeline.home)
            .env("USERPROFILE", &bed.pipeline.home)
            .env("GX_DEMO_LOG", &bed.arrivals)
            .args(bed.wrap_args(&[
                "--restore-catalogue".to_string(),
                catalogue.display().to_string(),
            ]))
            .stdin(Stdio::null()),
    );
    println!(
        "R23_H01_ROAD rc={} stdout={} stderr={}",
        run.code,
        run.stdout.chars().take(160).collect::<String>(),
        run.stderr.chars().take(500).collect::<String>()
    );
    assert_ne!(
        run.code, 0,
        "🔴 `req/312` H-01: this catalogue started a session. gx then called `notes.restore` from \
         `snapshot` — six arrivals before the agent's own `notes.write` — and the document was \
         empty on the admit road and on the refusing road alike"
    );
    assert!(
        !run.stdout.contains("\"gx\":\"wrap\""),
        "the discriminator is the start-up line: a session that begins can read an object under \
         this declaration. stdout: {}",
        run.stdout
    );
    let detail = detail_of(&run.stderr);
    assert!(
        detail.contains("the inverse of one") || detail.contains("inverse of \"notes.write\""),
        "and the refusal names the thing this file says about itself — that `notes.restore` puts \
         objects back — rather than the sentence about effects, which is true of a spelling this \
         file does not use: {}",
        run.stderr
    );
    assert_eq!(
        bed.note_now(),
        BEFORE,
        "nothing was written on the way here"
    );
    assert!(
        bed.arrivals().is_empty(),
        "and the server was never asked anything: parse runs before spawn ({:?})",
        bed.arrivals()
    );
}

/// The control: the same file with a read-only face starts, gates, and round-trips.
///
/// Without it, "refuse every `$cas_read`" satisfies the arm above and takes DR-46-16 with it.
#[test]
fn the_same_declaration_with_a_read_only_face_still_starts_a_session() {
    let bed = bed("r23_h01_control");
    let catalogue = bed.catalogue(
        "catalogue-read-only.json",
        json!({
            "notes.write": {
                "restored_by": "notes.restore",
                "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
            }
        }),
    );
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "what the agent wrote through gx wrap\n" },
        }),
    );
    let stderr = agent.close();
    println!(
        "R23_H01_CONTROL note={:?} arrivals={:?}",
        bed.note_now(),
        bed.arrivals()
    );
    assert_eq!(
        bed.note_now(),
        "what the agent wrote through gx wrap\n",
        "the ordinary road is unchanged: {}",
        text_of(&answer)
    );
    assert!(
        stderr.contains("\"gx\":\"wrap\""),
        "and the session started: {stderr}"
    );
}

/// The declaration whose fault appears when it is **resolved**, one call in.
///
/// `read_by` names a forward member this call does not carry, so `Catalogue::from_json` cannot see
/// the fault (the member exists in some call, just not this one) and `crate::invert` refuses when
/// it builds the escrow. This is `req/310` item 2's "road 2", and it is the road that survives
/// H-01(a)'s repair: the `$cas_read` half is sound, so gx reads the object through `resources/read`
/// for `snapshot` and `precondition` before it stops.
fn refusing_catalogue() -> Value {
    json!({
        "notes.write": {
            "restored_by": "notes.restore",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" },
            "read_by": {
                "by_tool": "notes.get",
                "arguments": { "id": { "forward": "a_member_this_call_never_had" } },
                "identity": [ "file://", { "answer": "/id" } ]
            }
        },
        "notes.restore": {
            "restored_by": "notes.write",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
        }
    })
}

// ---------------------------------------------------------------------------
// H-01(b) — the sentence, on a road the first repair leaves reachable
// ---------------------------------------------------------------------------

/// 🔴 `req/312` H-01(b): when gx read the object before it refused, the refusal says so.
///
/// The declaration here is sound in the way H-01(a) now requires — the CAS read face is a read tool
/// — and the fault is elsewhere: `notes.restore`'s own entry declares a template drawing a member
/// this call does not carry, so `invert` refuses when it resolves. By then `snapshot` and
/// `precondition` have each read the object, so two frames are on the wire and the old sentence
/// said none were.
#[test]
fn a_refusal_after_a_read_says_what_was_sent() {
    let bed = bed("r23_h01b_sentence");
    let catalogue = bed.catalogue("catalogue-refusing.json", refusing_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "what the agent wanted\n" },
        }),
    );
    let told = text_of(&answer);
    let stderr = agent.close();
    let arrivals = bed.arrivals();
    println!("R23_H01B_TEXT {told}");
    println!(
        "R23_H01B_ARRIVALS {arrivals:?} receipts={}",
        bed.receipt_files()
    );
    assert!(
        told.contains("gx refused this call"),
        "the road this arm needs is the refusal road: {told}"
    );
    assert_eq!(
        bed.note_now(),
        BEFORE,
        "the effect was not sent — that half of the old sentence was always true"
    );
    assert!(
        !told.contains("nothing was sent to the server"),
        "🔴 `req/312` H-01(b) / `req/38` §225 ruling 1: gx read this object before it stopped, so \
         *nothing was sent to the server* is a **runtime claim that is false**. On the audit's \
         road the reads were writes and the object was destroyed while this sentence was printed. \
         What the agent was told: {told}"
    );
    assert!(
        told.contains("the effect was not sent"),
        "and what it says instead separates the effect from the reads: {told}"
    );
    assert!(
        told.contains("reads were sent to the server")
            || told.contains("read was sent to the server"),
        "and it counts them, so an agent can check the number against the object: {told}"
    );
    assert!(
        stderr.contains("refused_before_verdict"),
        "R19's word is unchanged — this lane moved the sentence, not the classification: {stderr}"
    );
}

/// 🔴 The other side of the same predicate: when gx really did send nothing, it still says so.
///
/// A call naming no resource is refused by the proxy before `submit`, so the wire is untouched.
/// Without this arm, printing "reads were sent" unconditionally satisfies the arm above.
#[test]
fn a_refusal_before_any_read_still_says_nothing_was_sent() {
    let bed = bed("r23_h01b_zero");
    let mut agent = Agent::open(&bed.wrap_args(&[]), &bed.pipeline.home, &bed.arrivals);
    let answer = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "contents": "no uri here\n" } }),
    );
    let told = text_of(&answer);
    agent.close();
    println!("R23_H01B_ZERO {told} arrivals={:?}", bed.arrivals());
    assert!(
        told.contains("this call names no resource"),
        "the road this arm needs is the proxy's own pre-submit refusal: {told}"
    );
    assert!(
        told.contains("nothing was sent to the server"),
        "🔴 a proxy that says reads were sent when none were is the same defect pointing the other \
         way. This road is the audit's own `A22_ROAD1`: zero journal records, zero files, zero \
         arrivals: {told}"
    );
    assert!(
        bed.arrivals().is_empty(),
        "and the server's own log agrees: {:?}",
        bed.arrivals()
    );
}

/// 🔴 And the number is **this call's**, not the session's.
///
/// A cumulative counter would make the second refusal in a session claim the first one's reads,
/// which is a different false sentence in the same seat.
#[test]
fn the_count_a_refusal_prints_is_this_calls_and_not_the_sessions() {
    let bed = bed("r23_h01b_per_call");
    let catalogue = bed.catalogue("catalogue-refusing.json", refusing_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
    );
    let mut counts = Vec::new();
    // 🔴 Two **different** goals, not the same one twice: a repeated goal is the same intent, and
    // the second call meets a transformation the first one left in `Verifying` — a different road,
    // which would make this arm measure the wrong thing. The two calls read the same object the
    // same number of times, which is what the equality below is about.
    for contents in ["what the agent wanted\n", "what the agent wanted next\n"] {
        let answer = agent.ask(
            "tools/call",
            json!({
                "name": "notes.write",
                "arguments": { "uri": bed.uri, "contents": contents },
            }),
        );
        let told = text_of(&answer);
        // The two calls do not take the same road — the second meets a transformation the first
        // left holding this locator — and that is what makes the equality below worth having: the
        // clause is a function of **this call's** reads and not of the road or of the session.
        assert!(
            told.contains("the effect was not sent"),
            "every outcome that did not send the effect carries the clause: {told}"
        );
        // The word before `read`/`reads`, which is where the clause puts the number.
        let words: Vec<&str> = told.split_whitespace().collect();
        let n: u64 = words
            .windows(2)
            .position(|w| (w[0] == "read" || w[0] == "reads") && (w[1] == "was" || w[1] == "were"))
            .and_then(|at| words.get(at.wrapping_sub(1)).copied())
            .and_then(|word| word.parse().ok())
            .unwrap_or_else(|| panic!("the refusal counts what it sent: {told}"));
        counts.push(n);
    }
    agent.close();
    println!("R23_H01B_PER_CALL counts={counts:?}");
    assert_eq!(
        counts[0], counts[1],
        "🔴 two identical calls read the same number of times, so two identical refusals quote the \
         same number. A cumulative counter would print {} and then {}",
        counts[0], counts[1]
    );
    assert!(
        counts[0] >= 1,
        "and it is not zero: this road reads the object before it refuses"
    );
}
