// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/316` M-02, M-03 and L-03** (`req/317` §1 items 3, 4 and 5; `req/38` §227 rulings 3 and
//! 4) — three roads of `gx wrap` that a real server, a real proxy and real processes drive.
//!
//! # What the twenty-third adversarial audit measured
//!
//! ```text
//! A23_RO record_only=false start_up_says=false verdict=Deny enforced=true object="the note as it stood\n" arrivals=[]
//! A23_RO record_only=true  start_up_says=true  verdict=Deny enforced=true object="the note as it stood\n" arrivals=[]
//! A23_DECLARED absent=Marked tools_only=true ok=false object_now=None tool_reads=1 not_answered=true
//! A23_PA claims: not_reached=false effect_not_sent=false nothing_sent=false apply_failed=true
//! ```
//!
//! * **M-03** — `gx wrap --record-only` printed `"record_only":true` in its start-up line and then
//!   answered a `Deny` exactly as a run without the flag did: `gx/enforced: true`, zero arrivals,
//!   the object untouched. 43 §4 says the opposite in as many words, and `Deny` is the only verdict
//!   on which the flag means anything at all.
//! * **M-02** — a call that **removed** a resource could not be observed on the declared read road,
//!   so the substrate class DR-46-16 exists for could not record a deletion under any wiring.
//! * **L-03 (raised to M)** — when the post-apply observation fails *and* the compensating inverse
//!   fails too, the effect stands on the server, the ledger says `Aborted{ApplyFailed}`, and there
//!   is no committed transformation for `gx undo` to name. The sentence the agent got said nothing
//!   about any of that.
//!
//! # Red-first
//!
//! Every arm drives the shipped binary and reads bytes it produced. No symbol this lane created is
//! named, so this file compiles at `e944d74` and fails on its assertions.
//!
//! `cfg(unix)` for the `chmod` on the launcher script, as every sibling suite says.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]
#![cfg(unix)]

mod support;

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

const BEFORE: &str = "the note as it stood before any agent touched it\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ENDPOINT: &str = "stdio://r24";

/// The pack that permits `mcp` and forbids one locator inside it, so that a `Deny` on this surface
/// is a `forbid` somebody wrote rather than the absence of a permit.
fn deny_mcp_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("deny-mcp-note.cedar")
}

/// The fragment [`deny_mcp_pack`]'s `forbid` matches on.
const DENIED_NOTE: &str = "gx-denied-note";

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

fn bed(name: &str, note_name: &str) -> Bed {
    let pipeline =
        support::pipeline_named(name, "a file this suite does not measure\n", "seed.txt");
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);
    let note = pipeline.project.join(note_name);
    std::fs::write(&note, BEFORE).expect("write the note");
    let uri = format!("file://{}", note.display());
    let arrivals = pipeline.project.join("arrivals.log");
    let launcher = pipeline.project.join("r24-server.sh");
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
    fn note_now(&self) -> Option<String> {
        std::fs::read_to_string(&self.note).ok()
    }

    /// 🔴 The **server's own** record of what reached it, not this suite's account of it.
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
            "r24-probe".to_string(),
        ];
        args.extend(extra.iter().cloned());
        args.push("--".to_string());
        args.push(self.launcher.display().to_string());
        args
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
    fn open(args: &[String], home: &Path, arrivals: &Path, env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gx"));
        command
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("GX_DEMO_LOG", arrivals);
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command
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
                "clientInfo": { "name": "r24", "version": "0" },
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

/// The `_meta` object `gx wrap` attaches to the answer it relays.
fn meta_of(answer: &Value) -> Value {
    answer["result"]["_meta"].clone()
}

/// The catalogue every arm here starts from: a writing tool, its inverse, and the templates that
/// make an escrow buildable.
fn notes_catalogue() -> Value {
    json!({
        "notes.write": {
            "restored_by": "notes.restore",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
        }
    })
}

// ---------------------------------------------------------------------------
// M-03 — `--record-only` on the one road where it means anything
// ---------------------------------------------------------------------------

/// One `notes.write` through `gx wrap`, under the deny pack, with or without `--record-only`.
struct Run {
    verdict: String,
    enforced: Value,
    object: Option<String>,
    arrivals: Vec<String>,
    start_up_says: bool,
}

fn denied_write(name: &str, record_only: bool) -> Run {
    let bed = bed(name, &format!("{DENIED_NOTE}.txt"));
    let catalogue = bed.catalogue("catalogue.json", notes_catalogue());
    let mut extra = vec![
        "--restore-catalogue".to_string(),
        catalogue.display().to_string(),
        "--policy".to_string(),
        deny_mcp_pack().display().to_string(),
    ];
    if record_only {
        extra.push("--record-only".to_string());
    }
    let mut agent = Agent::open(
        &bed.wrap_args(&extra),
        &bed.pipeline.home,
        &bed.arrivals,
        &[],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "what the agent wrote through gx wrap\n" },
        }),
    );
    let meta = meta_of(&answer);
    let stderr = agent.close();
    Run {
        verdict: meta["gx/verdict"].as_str().unwrap_or_default().to_string(),
        enforced: meta["gx/enforced"].clone(),
        object: bed.note_now(),
        arrivals: bed.arrivals(),
        start_up_says: stderr.contains("\"record_only\":true"),
    }
}

/// 🔴 `req/316` M-03: under `--record-only`, a denied call is recorded and the effect is sent.
///
/// 43 §4, verbatim: *"record-only mode … from `Denied` as well it goes on, by way of T-8r, to
/// `Canonicalized → Committing → Committed`; but the receipt must always carve `enforced=false`
/// into it"* (quoted in SEM-gx-cli-2309). The pair of runs below
/// differ in one flag and nothing else, which is the audit's own shape.
#[test]
fn a_denied_call_under_record_only_is_recorded_and_the_effect_is_sent() {
    let enforcing = denied_write("r24_ro_off", false);
    let recording = denied_write("r24_ro_on", true);
    for (what, run) in [("enforcing", &enforcing), ("recording", &recording)] {
        println!(
            "R24_RO {what:<9} start_up_says={} verdict={} enforced={} object={:?} arrivals={:?}",
            run.start_up_says, run.verdict, run.enforced, run.object, run.arrivals
        );
    }
    assert_eq!(
        enforcing.verdict, "Deny",
        "the premise: this pack denies this locator on this substrate"
    );
    assert!(
        recording.start_up_says,
        "the premise: the flag reached the spec and the start-up line said so"
    );
    assert_eq!(
        recording.verdict, "Deny",
        "🔴 the **verdict** does not move under record-only. What the mode changes is what happens \
         after the gate has spoken, and a receipt that said `Admit` here would be the fact this \
         mode exists to preserve, destroyed"
    );
    assert_eq!(
        recording.enforced,
        Value::Bool(false),
        "🔴 `req/316` M-03: 43 §4 says the receipt *must always* carry `enforced=false` on this \
         road, and `gx wrap` signed `{}` — a receipt asserting policy was enforced over a call \
         policy refused. The audit measured the flag reaching the start-up line and nothing else",
        recording.enforced
    );
    assert_eq!(
        recording.object.as_deref(),
        Some("what the agent wrote through gx wrap\n"),
        "🔴 `req/316` M-03: the point of DR-2's record-only half is that the change **goes \
         through** and the refusal is on the record beside it. `gx wrap` returned before T-8r, so \
         the flag was inert on the one road it has a meaning on: object={:?} arrivals={:?}",
        recording.object,
        recording.arrivals
    );
    assert!(
        recording
            .arrivals
            .iter()
            .any(|line| line.contains("notes.write")),
        "and the server's own arrival log is what says the effect was sent, not this proxy's \
         account of it: {:?}",
        recording.arrivals
    );
}

/// 🔴 The negative control: without the flag, the enforcing half is exactly where it was.
///
/// Without this arm, "commit every Deny" satisfies the arm above and turns the product off.
#[test]
fn without_the_flag_a_denied_call_is_still_refused_and_the_object_does_not_move() {
    let enforcing = denied_write("r24_ro_control", false);
    println!(
        "R24_RO_CONTROL verdict={} enforced={} object={:?} arrivals={:?}",
        enforcing.verdict, enforcing.enforced, enforcing.object, enforcing.arrivals
    );
    assert_eq!(enforcing.verdict, "Deny", "the gate refused");
    assert_eq!(
        enforcing.object.as_deref(),
        Some(BEFORE),
        "and the object did not move: the default is enforcement and it is unchanged"
    );
    assert!(
        !enforcing
            .arrivals
            .iter()
            .any(|line| line.contains("notes.write")),
        "and the effect never reached the server: {:?}",
        enforcing.arrivals
    );
}

/// 🔴 The other control: on the **admit** road, `--record-only` changes nothing.
///
/// `enforced` stays true there — 43 §4's flag is about a `Deny` that was carried through, and a run
/// that stamped `false` on every receipt would make the flag useless for telling the two apart.
#[test]
fn on_the_admit_road_record_only_changes_nothing() {
    let bed = bed("r24_ro_admit", "note.txt");
    let catalogue = bed.catalogue("catalogue.json", notes_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
            "--record-only".to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
        &[],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "an admitted write\n" },
        }),
    );
    let meta = meta_of(&answer);
    agent.close();
    println!(
        "R24_RO_ADMIT verdict={} enforced={} object={:?}",
        meta["gx/verdict"],
        meta["gx/enforced"],
        bed.note_now()
    );
    assert_eq!(
        meta["gx/verdict"],
        json!("Admit"),
        "the premise: no pack denies this locator"
    );
    assert_eq!(
        meta["gx/enforced"],
        Value::Bool(true),
        "a gate that admitted was enforced, and record-only does not make that false"
    );
    assert_eq!(
        bed.note_now().as_deref(),
        Some("an admitted write\n"),
        "and the ordinary road is unchanged"
    );
}

// ---------------------------------------------------------------------------
// M-02 — a removal, observed through a declared read face, over a real wire
// ---------------------------------------------------------------------------

/// 🔴 `req/316` M-02: a call that removed a resource is committed rather than refused, on the road
/// a tools-only deployment reads by.
#[test]
fn a_removal_is_observed_through_the_declared_read_face() {
    let bed = bed("r24_removal", "note.txt");
    let catalogue = bed.catalogue(
        "catalogue.json",
        json!({
            "notes.delete": {
                "restored_by": "notes.write",
                "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
            },
            "$cas_read": {
                "file://": { "by_tool": "notes.fetch", "arguments": { "uri": "resource" } }
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
        &[],
    );
    let answer = agent.ask(
        "tools/call",
        json!({ "name": "notes.delete", "arguments": { "uri": bed.uri } }),
    );
    let meta = meta_of(&answer);
    let text = text_of(&answer);
    agent.close();
    println!(
        "R24_REMOVAL verdict={} object={:?} arrivals={:?} text={}",
        meta["gx/verdict"],
        bed.note_now(),
        bed.arrivals(),
        text.chars().take(200).collect::<String>()
    );
    assert!(
        bed.arrivals()
            .iter()
            .any(|line| line.contains("notes.fetch")),
        "the premise: the object was read through the **declared tool face**, which is the road \
         this arm is about: {:?}",
        bed.arrivals()
    );
    assert_eq!(
        bed.note_now(),
        None,
        "the premise: the call removed the resource"
    );
    assert_eq!(
        meta["gx/verdict"],
        json!("Admit"),
        "🔴 `req/316` M-02: `read_prior_by_tool` wrote no absence marker, so a removal on the \
         declared road could not be observed under any wiring — four were measured and all four \
         refused. This is the substrate class DR-46-16 exists for, and it could not record a \
         deletion: {text}"
    );
    assert!(
        meta["gx/commit"]["state"] == json!("Committed"),
        "and it committed: a removal's postcondition is the absent digest, which is what the \
         server answered: {}",
        meta["gx/commit"]
    );
}

// ---------------------------------------------------------------------------
// L-03 (M) — the effect stands, the ledger says the apply failed, and there is no undo
// ---------------------------------------------------------------------------

/// 🔴 `req/316` L-03 (`req/38` §227 ruling 4): when the compensation fails too, the sentence says
/// what is true of the world.
#[test]
fn when_the_compensation_fails_the_sentence_says_the_effect_may_stand() {
    let bed = bed("r24_post_apply", "note.txt");
    let catalogue = bed.catalogue("catalogue.json", notes_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
        // The two faults, in one run: the forward call lands its effect and **then** answers an
        // error, so the commit aborts over a world that really moved; and the compensating call is
        // refused, so 43 T-10c's best effort fails.
        //
        // 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the first fault used to be
        // `GX_DEMO_READ_FAILS_AFTER_EFFECT`. It can no longer reach this road: the engine reads the
        // object before it compensates, so dead reads answer `NotAttempted(WorldCouldNotBeRead)`
        // and no compensating call is sent at all. This arm is about what the sentence says when
        // the best effort **was** made and failed, so it needs a call that moved the world.
        &[
            ("GX_DEMO_TOOL_FAILS_AFTER_EFFECT", "notes.write"),
            ("GX_DEMO_TOOL_REFUSES", "notes.restore"),
        ],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "what the call put there\n" },
        }),
    );
    let text = text_of(&answer);
    let meta = meta_of(&answer);
    agent.close();
    println!(
        "R24_POST_APPLY object={:?} arrivals={:?}\nR24_POST_APPLY text={text}",
        bed.note_now(),
        bed.arrivals()
    );
    assert_eq!(
        bed.note_now().as_deref(),
        Some("what the call put there\n"),
        "the premise: the effect landed and the compensation did not take it back. \
         arrivals={:?} meta={meta}",
        bed.arrivals()
    );
    assert!(
        bed.arrivals()
            .iter()
            .any(|line| line.contains("notes.restore")),
        "the premise: 43 T-10c's best effort was actually attempted: {:?}",
        bed.arrivals()
    );
    assert!(
        text.contains("ApplyFailed"),
        "the premise: the ledger says the apply failed, which is the phrase this arm is about: \
         {text}"
    );
    for needle in [
        "the change may stand on the server",
        "best-effort",
        "transformation committed, so there is no `gx undo`",
    ] {
        assert!(
            text.contains(needle),
            "🔴 `req/316` L-03: the record says `Aborted{{ApplyFailed}}` — whose plain reading is \
             that the change did not land — and the object holds the change. The sentence the \
             agent is handed is the one place gx can say so, and it does not say {needle:?}: {text}"
        );
    }
}

/// 🔴 The other side of the same road: when 43 T-10c's best effort **succeeds**, the object is back
/// and the ledger says the same three words it says when the effort failed.
///
/// This is why the repair is a sentence rather than a new ledger state. `Aborted{ApplyFailed}` is
/// 43 §3's record and `gx-engine`'s to write, and a proxy inventing a second opinion about a
/// transformation's state would be worse than the gap. What the two runs share is the record; what
/// they differ in is the world. So the sentence hedges (*may* stand) and names the two places that
/// answer it, which is the honest width of what this crate knows.
#[test]
fn when_the_compensation_succeeds_the_object_is_back_and_the_clause_is_earned() {
    let bed = bed("r24_post_apply_ok", "note.txt");
    let catalogue = bed.catalogue("catalogue.json", notes_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
        // 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the fault used to be
        // `GX_DEMO_READ_FAILS_AFTER_EFFECT`, and it can no longer reach this road. The engine now
        // reads the object before it sends a compensation, so a substrate whose reads are dead
        // answers `NotAttempted(WorldCouldNotBeRead)` and the compensating call is never sent —
        // an absolute inverse is not fired into a world nobody can see. This arm is about a
        // compensation that **runs**, which needs a forward call that really moved the object, so
        // it uses the fault R30 added to the shipped demo server for exactly this shape.
        &[("GX_DEMO_TOOL_FAILS_AFTER_EFFECT", "notes.write")],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "what the call put there\n" },
        }),
    );
    let text = text_of(&answer);
    agent.close();
    println!(
        "R24_POST_APPLY_OK object={:?} arrivals={:?}",
        bed.note_now(),
        bed.arrivals()
    );
    assert_eq!(
        bed.note_now().as_deref(),
        Some(BEFORE),
        "the compensating inverse ran and the object is back where it was: {text}"
    );
    assert!(
        text.contains("ApplyFailed"),
        "the ledger's word is the same on both roads — which is the disagreement this repair is \
         about, and the reason the clause is a sentence rather than a new state: {text}"
    );
}

/// 🔴 The control that keeps the clause from being unconditional: a call that **committed** carries
/// none of it.
///
/// Without it, appending the paragraph to every answer satisfies both arms above and tells every
/// reader of a clean commit that their object may be in a state nobody recorded.
#[test]
fn a_call_that_committed_carries_no_apply_failed_clause() {
    let bed = bed("r24_post_apply_none", "note.txt");
    let catalogue = bed.catalogue("catalogue.json", notes_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
        &[],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "an ordinary admitted write
        " },
        }),
    );
    let text = text_of(&answer);
    let meta = meta_of(&answer);
    agent.close();
    println!(
        "R24_POST_APPLY_NONE verdict={} state={} text={}",
        meta["gx/verdict"],
        meta["gx/commit"]["state"],
        text.chars().take(160).collect::<String>()
    );
    assert_eq!(
        meta["gx/commit"]["state"],
        json!("Committed"),
        "the premise: this call committed"
    );
    for needle in ["ApplyFailed", "may stand on the server", "best-effort"] {
        assert!(
            !text.contains(needle),
            "a committed call is not told its object may be in a state nobody recorded              ({needle:?}): {text}"
        );
    }
}
