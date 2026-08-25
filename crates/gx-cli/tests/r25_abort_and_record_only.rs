// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/320` H-01, M-01, M-02, L-02 and L-03** (`req/321` §1 items 1, 2, 3, 6 and 7;
//! `req/38` §229 rulings 1, 2 and 3) — the five sentences `gx wrap` hands an agent that the
//! twenty-fourth adversarial audit measured against the world, plus the re-measurement §229 ruling
//! 3 asked for.
//!
//! # What the audit measured
//!
//! ```text
//! A24_PA case=forward-refused  reason="ApplyFailed" detail="Succeeded"    clause_says_attempted=true  restore_arrived=true
//! A24_PA case=observation-died reason="ApplyFailed" detail="Failed"       clause_says_attempted=true  restore_arrived=true
//! A24_PA case=pending-escrow   reason="ApplyFailed" detail="NotAttempted" clause_says_attempted=true  restore_arrived=false
//!         arrivals=["call  notes.write  file://…/a24_pa_pending/note.txt"]
//! A24_RO_ABORT verdict="Deny" enforced=true says_admitted=true
//! A24_ROUND create_text="… the substrate would not answer for … [gx: the server answered, …]"
//! A24_RESIDUAL verdict="Admit" undo_rc=4 after_undo="the agent's paragraph\n"
//! ```
//!
//! * **H-01** — `Engine::rollback` has three values and the clause R24 wrote was true of two. On the
//!   third the server's own arrival log held **one** line and gx told the agent the compensating
//!   inverse *was attempted*.
//! * **L-03** — `Rollback::Failed` is `apply_once(inverse)` returning `Err`, and this adapter's
//!   apply is the call **and** the read-back of it, so `Failed` does not mean the object still holds
//!   the change. The audit measured a `Failed` road where the object was back.
//! * **M-02** — a `Deny` that `--record-only` carried through T-8r and whose apply then failed
//!   returned `gx/enforced: true` and a sentence saying *"gx admitted this call"*.
//! * **L-02** — `--record-only` does not reach the escalate road, and nothing said so.
//! * **M-01** — after a removal **gx itself admitted**, the next call to the same locator is
//!   answered with one sentence containing both *the substrate would not answer* and *the server
//!   answered*.
//! * **§229 ruling 3** — `req/312` §2(f)'s accepted residual (`undo rc=0`, `after_undo=""`)
//!   re-measured on this build before the page is allowed to narrow.
//!
//! # Red-first
//!
//! Every arm drives the shipped binary and reads bytes it produced; no symbol this lane created is
//! named, so this file compiles at `d21821e` and fails on its assertions.
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
const ENDPOINT: &str = "stdio://r25";

/// The fragment [`DENY_PACK`] forbids.
const DENIED_NOTE: &str = "gx-denied-note";

fn deny_mcp_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("deny-mcp-note.cedar")
}

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
    let launcher = pipeline.project.join("r25-server.sh");
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

    fn arrivals_naming(&self, tool: &str) -> usize {
        self.arrivals()
            .iter()
            .filter(|line| line.contains(tool))
            .count()
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
            "r25-probe".to_string(),
        ];
        args.extend(extra.iter().cloned());
        args.push("--".to_string());
        args.push(self.launcher.display().to_string());
        args
    }

    fn undo(&self, tid: &str, catalogue: &Path) -> support::Run {
        let mut command = self.pipeline.gx();
        command.args([
            "--mcp-server",
            &self.launcher.display().to_string(),
            "--mcp-endpoint",
            ENDPOINT,
            "--mcp-restore-catalogue",
            &catalogue.display().to_string(),
            "undo",
            tid,
        ]);
        support::run(&mut command)
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
                "clientInfo": { "name": "r25", "version": "0" },
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

fn meta_of(answer: &Value) -> Value {
    answer["result"]["_meta"].clone()
}

/// The `detail` word `Engine::rollback` left in the commit record this answer carries.
fn rollback_detail(meta: &Value) -> String {
    meta["gx/commit"]["detail"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

// ---------------------------------------------------------------------------
// H-01 / L-03 — one clause per value of `Engine::rollback`
// ---------------------------------------------------------------------------

/// The catalogue whose escrowed inverse is **partial**: one member is filled from what the forward
/// call answers, so `crate::invert` escrows it `Pending` and 43 T-10c's guard reads it as "no
/// inverse this build can execute".
fn pending_escrow_catalogue() -> Value {
    json!({
        "notes.write": {
            "restored_by": "notes.restore",
            "arguments": {
                "uri": { "forward": "uri" },
                "contents": "prior_contents_utf8",
                "note_number": { "do_result_number_from": "/content/0/text" }
            }
        }
    })
}

/// The catalogue whose inverse is complete at plan time.
fn resolved_escrow_catalogue() -> Value {
    json!({
        "notes.write": {
            "restored_by": "notes.restore",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
        }
    })
}

struct Abort {
    text: String,
    detail: String,
    restore_arrivals: usize,
    write_arrivals: usize,
    object: Option<String>,
}

/// One `notes.write` whose apply fails, under whichever catalogue and fault switches the caller
/// wants.
fn aborting_write(name: &str, catalogue_body: Value, env: &[(&str, &str)]) -> Abort {
    let bed = bed(name, "note.txt");
    let catalogue = bed.catalogue("catalogue.json", catalogue_body);
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
        env,
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
    Abort {
        detail: rollback_detail(&meta),
        text,
        restore_arrivals: bed.arrivals_naming("notes.restore"),
        write_arrivals: bed.arrivals_naming("notes.write"),
        object: bed.note_now(),
    }
}

/// 🔴 `req/320` H-01: on the `NotAttempted` road, gx said the compensation **was attempted** and the
/// server's own log held one line.
#[test]
fn the_road_where_no_compensation_was_sent_does_not_claim_one_was() {
    let run = aborting_write(
        "r25_pending_escrow",
        pending_escrow_catalogue(),
        &[("GX_DEMO_READ_FAILS_AFTER_EFFECT", "1")],
    );
    println!(
        "R25_PA case=pending-escrow detail={:?} write_arrivals={} restore_arrivals={} object={:?}\n\
         R25_PA text={}",
        run.detail, run.write_arrivals, run.restore_arrivals, run.object, run.text
    );
    assert_eq!(
        run.detail, "NotAttempted",
        "the premise: 43 T-10c's guard did not open, because the escrowed inverse was still \
         partial. text={}",
        run.text
    );
    assert_eq!(
        run.write_arrivals, 1,
        "the premise: the forward call reached the server exactly once"
    );
    assert_eq!(
        run.restore_arrivals, 0,
        "🔴 the premise this finding turns on, taken from the **server's** arrival log rather than \
         from gx: no compensating call was ever framed"
    );
    assert!(
        !run.text.contains("inverse was attempted"),
        "🔴 `req/320` H-01 (`req/38` §229 ruling 1): `Engine::rollback` has three values and R24's \
         clause was true of two of them. On this road gx told the agent the compensating inverse \
         *was attempted* while the server's own log showed it had never been sent — the same shape \
         `req/38` §225 ruling 1 called the deciding fact of two previous H findings, with the sign \
         reversed: {}",
        run.text
    );
    assert!(
        run.text.contains("no compensating inverse was sent"),
        "🔴 and the sentence says which of the three happened, in words a reader can act on: {}",
        run.text
    );
    // The two facts that hold on every `ApplyFailed` road are still said on this one.
    for needle in [
        "the tool call was sent",
        "transformation committed, so there is no `gx undo`",
    ] {
        assert!(
            run.text.contains(needle),
            "the road-independent half of the clause is unchanged ({needle:?}): {}",
            run.text
        );
    }
}

/// 🔴 `req/320` L-03: `Rollback::Failed` does not license *the object still holds the change*.
///
/// This adapter's `apply` is the call **and** the read-back of it, so an `Err` can be either. The
/// audit measured a `Failed` road on which the compensating bytes had landed and the object was
/// back where it started; the word `Failed` was answering a different question than the one it was
/// offered as the answer to.
#[test]
fn the_failed_road_says_what_failed_rather_than_what_the_object_holds() {
    // 🔴 **R30 / `req/372` M-01** — the first switch used to be `GX_DEMO_READ_FAILS_AFTER_EFFECT`,
    // and it can no longer reach this road. After R30 the engine reads the object before it sends
    // a compensation, so a substrate whose reads are dead answers
    // `NotAttempted(WorldCouldNotBeRead)` — an absolute inverse is not fired into a world nobody
    // can see — and the compensating call is never sent at all. The property this arm is named for
    // is about the word `Failed`, which needs a compensation that **was** sent and whose own apply
    // errored: so the forward call lands and then errors (R30's third demo switch), the reads keep
    // working, and the restore is the call the server refuses.
    let run = aborting_write(
        "r25_compensation_failed",
        resolved_escrow_catalogue(),
        &[
            ("GX_DEMO_TOOL_FAILS_AFTER_EFFECT", "notes.write"),
            ("GX_DEMO_TOOL_REFUSES", "notes.restore"),
        ],
    );
    println!(
        "R25_PA case=observation-died detail={:?} restore_arrivals={} object={:?}\n\
         R25_PA text={}",
        run.detail, run.restore_arrivals, run.object, run.text
    );
    assert_eq!(run.detail, "Failed", "the premise: text={}", run.text);
    assert_eq!(
        run.restore_arrivals, 1,
        "the premise: the compensating call **was** sent, which is what separates this road from \
         `NotAttempted`"
    );
    assert!(
        run.text.contains("its own apply came back an error"),
        "🔴 `req/320` L-03: `detail` is handed over as the answer to *did the compensation work*, \
         and it is the answer to *did the compensating apply return `Ok`* — which includes the \
         read-back of it. The sentence has to say which: {}",
        run.text
    );
    assert!(
        run.text.contains("best-effort"),
        "43 T-10c's own word for the attempt survives on the road where an attempt was made: {}",
        run.text
    );
}

/// 🔴 The third value, so that the branch is a branch: when the compensation is accepted, the
/// sentence says so and says whose word that is.
#[test]
fn the_succeeded_road_names_the_adapter_as_the_one_who_accepted() {
    // 🔴 ~~The road that produces `Succeeded` is the one where the **forward** call is refused: the
    // escrow is complete, the compensating restore is sent, and its own read-back is not
    // sabotaged.~~ — **superseded in R30 (`req/372` M-01, `req/38` §240 ruling 2). The old text is
    // kept because it records what this arm used to drive.**
    //
    // A forward call the server **refuses** never touches the object, and after R30 the engine
    // reads the object the instant an apply fails and declines to send a compensation for an
    // effect that does not exist — `NotAttempted(WorldNeverMoved)`. That is not a regression in
    // this arm, it is this arm having driven the defect: `restore_arrivals=1` on that road meant
    // gx was sending an **absolute** restore over an object it had never changed, which is exactly
    // the write `req/372` M-01 measured erasing a third party's commit.
    //
    // So the `Succeeded` road is now the one where the forward call **lands and then errors** —
    // the commonest real failure, and the road a compensation is actually *for*. It needs the
    // third switch, which R30 added to the shipped demo server for this reason
    // (`GX_DEMO_TOOL_FAILS_AFTER_EFFECT`, `crates/gx-cli/src/demo.rs`).
    // `GX_DEMO_READ_FAILS_AFTER_EFFECT` still cannot produce it — that switch kills every read
    // after any effect arrives, including the compensation's own.
    let run = aborting_write(
        "r25_compensation_ok",
        resolved_escrow_catalogue(),
        &[("GX_DEMO_TOOL_FAILS_AFTER_EFFECT", "notes.write")],
    );
    println!(
        "R25_PA case=compensation-ok detail={:?} restore_arrivals={} object={:?}\n\
         R25_PA text={}",
        run.detail, run.restore_arrivals, run.object, run.text
    );
    assert_eq!(run.detail, "Succeeded", "the premise: text={}", run.text);
    assert_eq!(
        run.object.as_deref(),
        Some(BEFORE),
        "the premise: the object is back"
    );
    assert!(
        run.text.contains("the adapter accepted it"),
        "🔴 the acceptance is the adapter's word about its own call and the sentence says so — the \
         server is what answers for the object: {}",
        run.text
    );
    assert!(
        !run.text.contains("no compensating inverse was sent"),
        "and the three arms are three arms: {}",
        run.text
    );
}

// ---------------------------------------------------------------------------
// M-02 — the abort branch under `--record-only`
// ---------------------------------------------------------------------------

/// 🔴 `req/320` M-02: a `Deny` that record-only carried through T-8r, whose apply then failed,
/// answered `gx/enforced: true` and *"gx admitted this call"*.
#[test]
fn a_record_only_deny_whose_commit_aborts_still_says_policy_did_not_enforce() {
    let bed = bed("r25_ro_abort", &format!("{DENIED_NOTE}.txt"));
    let catalogue = bed.catalogue("catalogue.json", resolved_escrow_catalogue());
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
            "--policy".to_string(),
            deny_mcp_pack().display().to_string(),
            "--record-only".to_string(),
        ]),
        &bed.pipeline.home,
        &bed.arrivals,
        &[("GX_DEMO_READ_FAILS_AFTER_EFFECT", "1")],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "what the agent wrote through gx wrap\n" },
        }),
    );
    let meta = meta_of(&answer);
    let text = text_of(&answer);
    agent.close();
    println!(
        "R25_RO_ABORT verdict={} enforced={} commit={} object={:?}\nR25_RO_ABORT text={}",
        meta["gx/verdict"],
        meta["gx/enforced"],
        meta["gx/commit"],
        bed.note_now(),
        text
    );
    assert_eq!(
        meta["gx/verdict"],
        json!("Deny"),
        "the premise: the pack refused this locator and the verdict does not move"
    );
    assert_eq!(
        meta["gx/commit"]["reason"],
        json!("ApplyFailed"),
        "the premise: record-only carried it past the gate and the apply then failed"
    );
    assert_eq!(
        meta["gx/enforced"],
        json!(false),
        "🔴 `req/320` M-02 (`req/38` §229 ruling 2): 43 §4 says this flag must *always* be `false` \
         on the record-only road. An aborted commit writes no `enforced` member, so R24's repair \
         fell through to `verify`'s value — the one this crate's own comment calls *the value that \
         has not heard about the mode yet* — and the agent was told policy had been enforced over \
         a call policy refused and gx then sent"
    );
    assert!(
        !text.contains("gx admitted this call"),
        "🔴 and the sentence beside it said *gx admitted this call* while `gx/verdict` in the same \
         answer said `Deny`: {text}"
    );
    assert!(
        text.contains("gx denied this call") && text.contains("record-only"),
        "🔴 the sentence names the verdict the gate reached and what carried the call past it: \
         {text}"
    );
}

/// 🔴 The control that keeps the repair from being "stamp `false` on every abort": without the
/// flag, an admitted call whose commit aborts is untouched.
#[test]
fn an_admitted_call_whose_commit_aborts_is_not_relabelled() {
    let run = aborting_write(
        "r25_admit_abort_control",
        resolved_escrow_catalogue(),
        &[("GX_DEMO_READ_FAILS_AFTER_EFFECT", "1")],
    );
    println!("R25_ABORT_CONTROL text={}", run.text);
    assert!(
        run.text.contains("gx admitted this call"),
        "the ordinary abort road keeps its sentence: {}",
        run.text
    );
}

// ---------------------------------------------------------------------------
// L-02 — `--record-only` and the escalate road
// ---------------------------------------------------------------------------

/// 🔴 `req/320` L-02: the flag does not reach an escalation, and until this window nothing said so.
///
/// E-M3-4's escalation is what a tool with **no** restore declaration produces, and the audit ran it
/// with and without the flag: same verdict, zero arrivals, object unmoved. The implementation is
/// 43 §4 to the letter; what was missing was any way for a reader to learn it from `gx wrap`.
#[test]
fn record_only_says_that_it_does_not_reach_an_escalation() {
    let mut seen: Vec<(bool, String, Option<String>, usize)> = Vec::new();
    for record_only in [false, true] {
        let bed = bed(
            if record_only {
                "r25_escalate_ro"
            } else {
                "r25_escalate_plain"
            },
            "note.txt",
        );
        // A catalogue that declares a **different** tool, so `notes.write` has no inverse and
        // `SubstrateAdapter::invert` answers `None` — E-M3-4's Given.
        let catalogue = bed.catalogue(
            "catalogue.json",
            json!({
                "notes.delete": {
                    "restored_by": "notes.write",
                    "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
                }
            }),
        );
        let mut extra = vec![
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
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
                "arguments": { "uri": bed.uri, "contents": "an escalating write\n" },
            }),
        );
        let meta = meta_of(&answer);
        let text = text_of(&answer);
        agent.close();
        println!(
            "R25_ESCALATE record_only={record_only} verdict={} object={:?} write_arrivals={}\n\
             R25_ESCALATE text={text}",
            meta["gx/verdict"],
            bed.note_now(),
            bed.arrivals_naming("notes.write")
        );
        assert_eq!(
            meta["gx/verdict"],
            json!("Escalate"),
            "the premise: a tool with no declared inverse escalates at T-3 (E-M3-4): {text}"
        );
        seen.push((
            record_only,
            text,
            bed.note_now(),
            bed.arrivals_naming("notes.write"),
        ));
    }
    let plain = &seen[0];
    let recording = &seen[1];
    assert_eq!(
        recording.2.as_deref(),
        Some(BEFORE),
        "🔴 the measurement first: the flag does **not** carry an escalation through. The object is \
         where it was"
    );
    assert_eq!(
        recording.3, 0,
        "and nothing reached the server on the record-only run either"
    );
    assert!(
        !plain.1.contains("does not reach this road"),
        "the clause is not unconditional: a run without the flag has nothing to say about it: {}",
        plain.1
    );
    assert!(
        recording.1.contains("does not reach this road"),
        "🔴 `req/320` L-02 (`req/38` §229 ruling 2): the flag's name is *record only*, one of the \
         three verdicts still stops, and neither the start-up line nor the answer said which. \
         Measuring it is what the audit had to do; reading it is what this sentence is for: {}",
        recording.1
    );
}

// ---------------------------------------------------------------------------
// M-01 — the sentence after a removal gx itself admitted
// ---------------------------------------------------------------------------

/// 🔴 `req/320` M-01: *the substrate would not answer* and *the server answered* in one sentence.
///
/// The road is one session: gx admits a `notes.delete`, the object goes, and the next call naming
/// the same locator is stopped at `snapshot`. That stop is right — there is no prior state for a
/// compare-and-set — but the words handed over denied their own evidence.
#[test]
fn the_call_after_an_admitted_removal_is_told_which_fact_stopped_it() {
    let bed = bed("r25_absent_after_removal", "note.txt");
    let catalogue = bed.catalogue(
        "catalogue.json",
        json!({
            "notes.delete": {
                "restored_by": "notes.write",
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
        &[],
    );
    let removed = agent.ask(
        "tools/call",
        json!({ "name": "notes.delete", "arguments": { "uri": bed.uri } }),
    );
    let remove_verdict = meta_of(&removed)["gx/verdict"].clone();
    let after = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "put it back by hand\n" },
        }),
    );
    let after_meta = meta_of(&after);
    let after_text = text_of(&after);
    agent.close();
    println!(
        "R25_ROUND remove_verdict={remove_verdict} object={:?} next_verdict={}\n\
         R25_ROUND next_text={after_text}",
        bed.note_now(),
        after_meta["gx/verdict"]
    );
    assert_eq!(
        remove_verdict,
        json!("Admit"),
        "the premise: gx itself admitted the removal"
    );
    assert_eq!(bed.note_now(), None, "the premise: the object is gone");
    assert!(
        after_text.contains("the server answered"),
        "the premise: the discriminating token reached this sentence: {after_text}"
    );
    assert!(
        after_text.contains("the object is not there"),
        "🔴 `req/320` M-01 (`req/38` §229 ruling 2): the predicate `req/312` M-01 built was asked \
         at **one** of the three sites that consume a declared read, and the other two are \
         `snapshot` and `precondition`. So the agent was handed one sentence saying both *the \
         substrate would not answer* and *the server answered, and its answer is that this locator \
         holds nothing* — and could not act on either: {after_text}"
    );
    assert!(
        after_text.contains("What to fix:"),
        "and it carries a remedy, which is R17's rule for every refusal in this crate: \
         {after_text}"
    );
    assert_eq!(
        bed.note_now(),
        None,
        "and the behaviour is unchanged: this is still fail-closed, and nothing was written on the \
         way to the sentence"
    );
}

// ---------------------------------------------------------------------------
// §229 ruling 3 — the accepted residual, re-measured before the page moves
// ---------------------------------------------------------------------------

/// 🔴 **`req/38` §229 ruling 3 / `req/321` §1 item 7** — `req/312` §2(f)'s road, on this build.
///
/// `docs/LIMITS.md` declares an **accepted residual**: a restore template whose only member drawing
/// on something the forward call does not carry is a `{"const": …}` satisfies the prior-soundness
/// gate while carrying nothing of the prior. `req/312` §2(f) measured what that reached —
/// `verdict=Admit`, `undo rc=0`, `after_undo=""` — and the twenty-fourth audit, driving a *different*
/// spelling, got `undo rc=4` / `Escalated` / object unmoved instead.
///
/// The page may only be narrowed from a measurement, so this arm **is** the measurement and it
/// asserts the two halves the page's sentence is about rather than a verdict on them: the
/// declaration is still accepted, and what the printed undo then does is printed here.
#[test]
fn the_const_member_residual_is_measured_on_this_build() {
    measure_the_residual(
        "r25_residual",
        "catalogue-const-member.json",
        json!({ "const": "" }),
    );
}

/// 🔴 **`req/324` H-01 (`req/38` §231 ruling 1)** — the same measurement, the other spelling.
///
/// The lesson §230 ruling 1 wrote down is *do not read a measurement of one spelling as a fact
/// about the family*, and the arm above measured one spelling. `ConstJson` sits on the **same
/// arm** of the gate's classifier as `Const`, and the twenty-fifth audit drove it to the same
/// terminal state on a real binary. A residual measured at one spelling is how a page came to be
/// written at one spelling.
#[test]
fn the_const_json_member_residual_is_measured_on_this_build() {
    measure_the_residual(
        "r25_residual_json",
        "catalogue-const-json-member.json",
        json!({ "const_json": "" }),
    );
}

/// The bed both arms drive, with the member that draws on nothing the forward call carries as
/// its only parameter.
fn measure_the_residual(name: &str, file: &str, member: serde_json::Value) {
    let bed = bed(name, "note.txt");
    let catalogue = bed.catalogue(
        file,
        json!({
            "notes.write": {
                "restored_by": "notes.restore",
                "arguments": { "uri": { "forward": "uri" }, "contents": member }
            },
            "notes.restore": {
                "restored_by": "notes.write",
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
        &[],
    );
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": bed.uri, "contents": "the agent's paragraph\n" },
        }),
    );
    let meta = meta_of(&answer);
    agent.close();
    let verdict = meta["gx/verdict"].clone();
    let tid = meta["gx/transformation"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let after_call = bed.note_now();
    let undo = bed.undo(&tid, &catalogue);
    let after_undo = bed.note_now();
    println!(
        "R25_RESIDUAL bed={name} verdict={verdict} tid={tid} after_call={after_call:?} undo_rc={} \
         after_undo={after_undo:?}\nR25_RESIDUAL undo_stdout={}",
        undo.code,
        undo.stdout.chars().take(300).collect::<String>()
    );
    assert_eq!(
        verdict,
        json!("Admit"),
        "the first half of the page's sentence: the declaration is accepted and reaches an admit. \
         If this ever changes, the page's paragraph moves in the same commit"
    );
    assert_eq!(
        after_call.as_deref(),
        Some("the agent's paragraph\n"),
        "the premise: the effect landed"
    );
    // 🔴 The second half is the **measurement**, not an expectation: `req/312` §2(f) got `rc=0` and
    // an emptied object here, and this asserts the road as it is on this build so that the page can
    // be narrowed to what was measured rather than to what was hoped.
    assert!(
        undo.code == 0 || undo.code == 4,
        "the undo either restores (`rc=0`, `req/312`'s measurement) or is refused for a reason this \
         build has a word for; anything else is a road nobody has described: rc={} stdout={} \
         stderr={}",
        undo.code,
        undo.stdout,
        undo.stderr
    );
    if undo.code == 0 {
        assert_eq!(
            after_undo.as_deref(),
            Some(""),
            "🔴 `req/312` §2(f) reproduced: the printed undo emptied the object. `docs/LIMITS.md`'s \
             accepted-residual paragraph stands exactly as written"
        );
    } else {
        assert_eq!(
            after_undo.as_deref(),
            Some("the agent's paragraph\n"),
            "🔴 the undo did not run and the object did not move, so the residual is *the \
             declaration is accepted*, not *the undo destroys*. That is the narrowing `req/38` §229 \
             ruling 3 asks the page to make — and only from this measurement: undo stdout={} \
             stderr={}",
            undo.stdout,
            undo.stderr
        );
    }
}
