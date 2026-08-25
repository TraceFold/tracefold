// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R19** (`req/284`, from audit 19's H-02 / M-05 / M-06 / M-07) — the road that completes an
//! escalated MCP change, measured end to end through the binary.
//!
//! Audit 19 (`req/279`) measured four separate ways in which the sentence "gx asks a person first"
//! (E-M3-4) stopped being true after the person answered:
//!
//! * **H-02** — `--mcp-server`/`--mcp-server-arg`/`--mcp-restore-catalogue` are `global = true`, so
//!   every verb *accepts* them, and only five verbs (`submit`/`plan`/`verify`/`commit`/`undo`) ever
//!   read them. `gx escalation approve` and `gx serve` took the flags and dropped them, and then
//!   refused the ruling with a sentence naming the very flag they had just discarded.
//! * **M-05** — `GET /v1/escalations` read the **live** ticket table, so an escalation raised by a
//!   `gx wrap` process was invisible to the `gx serve` a person actually looks at.
//! * **M-06** — the three declaration-derived refusals came back to the agent as
//!   `this is not a refusal`, and were counted as `failed` rather than as something gx refused.
//! * **M-07** — the undo remedy `gx wrap` prints on success (`gx undo <TID> --mcp-server <the same
//!   server>`) could not be run as printed: it named no restore catalogue, so the undo escalated,
//!   and after that the state machine had no `undo` left to give.
//!
//! Every test here drives real child processes — a real `gx wrap`, a real `gx __demo-notes-server`
//! behind it, real single-shot verbs afterwards — and asserts on the **object on the substrate**
//! and on the **server's own arrival log**, never on this process's own say-so.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

mod support;

const BEFORE: &str = "the note as it stood before any agent touched it\n";
const AFTER: &str = "the note after an agent wrote through gx wrap\n";

/// The hidden subcommand of this same binary that serves MCP notes (`gx_cli::demo`).
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
/// The `env` name that server writes one line per **arrival** to.
const ARRIVALS_ENV: &str = "GX_DEMO_LOG";

// ---------------------------------------------------------------------------
// A `gx wrap` session, from the agent's side
// ---------------------------------------------------------------------------

struct Wrap {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Wrap {
    /// `home` is `~/.gx/keys/` (req/56 §3). Without it the proxy reaches the gate and then answers
    /// `no key for …`, which is a fixture fault wearing the shape of a finding.
    ///
    /// 🔴 The arrivals log is set in this process's **environment** and not with
    /// `gx wrap --server-env`, deliberately. `gx wrap` never prints an `env` **value** (see
    /// `wrap::environment`: that is where a server's credential lives), so a session that carried
    /// one could not have a fully runnable undo remedy printed for it — and M-07 is about a remedy
    /// a reader runs without adding words. An operator's shell is where server environment lives;
    /// this is that shell.
    fn spawn(args: &[String], home: &Path, arrivals: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env(ARRIVALS_ENV, arrivals)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the gx binary runs");
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let mut session = Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 0,
        };
        session.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "r19", "version": "0" },
            }),
        );
        session.notify("notifications/initialized", json!({}));
        session
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let frame = jsonrpc::request(self.next_id, method, params);
        jsonrpc::write_frame(self.stdin.as_mut().expect("open"), &frame).expect("write");
        match jsonrpc::read_frame(&mut self.stdout).expect("read") {
            Some(line) => serde_json::from_str(&line).expect("gx wrap answers JSON"),
            // A proxy that closed its stdout said why on **its** stderr, and a probe that panicked
            // without reading it would report "no answer" for every cause there is.
            None => panic!(
                "gx wrap closed its stdout without answering {method:?}. its stderr:\n{}",
                self.stderr()
            ),
        }
    }

    fn stderr(&mut self) -> String {
        let mut text = String::new();
        if let Some(mut err) = self.child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut err, &mut text);
        }
        text
    }

    fn notify(&mut self, method: &str, params: Value) {
        let frame = jsonrpc::notification(method, params);
        jsonrpc::write_frame(self.stdin.as_mut().expect("open"), &frame).expect("write");
    }

    /// Close the agent's end and read the session summary `gx wrap` prints to stderr.
    fn finish(mut self) -> String {
        self.stdin = None;
        let out = self.child.wait_with_output().expect("gx wrap exits");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

/// The `{"gx": "wrap", "session": {...}}` object `gx wrap` prints last, out of its stderr.
fn session_summary(stderr: &str) -> Value {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rfind(|v| v.get("session").is_some())
        .unwrap_or_else(|| panic!("gx wrap prints a session summary; stderr was:\n{stderr}"))
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

/// A project, a key store, a ruler's key, a note on the substrate and an arrivals log.
struct Fixture {
    pipeline: support::Pipeline,
    note: PathBuf,
    uri: String,
    arrivals: PathBuf,
    ruler: String,
}

fn fixture(name: &str) -> Fixture {
    // 🔴 `gx submit` is the one verb that **creates** `.gx/` (44 has no `gx init`), and `gx wrap`
    // opens rather than creates. So the fixture submits one fs intent against a throwaway file
    // first: the project exists afterwards, and the note this suite is about has not been touched.
    let pipeline =
        support::pipeline_named(name, "a file this suite does not measure\n", "seed.txt");
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);
    let note = pipeline.project.join("note.txt");
    std::fs::write(&note, BEFORE).expect("write the note");
    let uri = format!("file://{}", note.display().to_string().replace('\\', "/"));
    let arrivals = pipeline.project.join("arrivals.tsv");
    let ruler = pipeline.another_key();
    Fixture {
        pipeline,
        note,
        uri,
        arrivals,
        ruler,
    }
}

impl Fixture {
    fn note_now(&self) -> String {
        std::fs::read_to_string(&self.note).unwrap_or_default()
    }

    fn arrivals_now(&self) -> Vec<String> {
        std::fs::read_to_string(&self.arrivals)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// `gx wrap --project <p> ...`, with the restore declaration the caller wants (or none).
    fn wrap_args(&self, catalogue: Option<&Path>) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "--project".into(),
            self.pipeline.project.display().to_string(),
            "wrap".into(),
            "--endpoint".into(),
            "stdio://r19".into(),
            "--actor-key".into(),
            self.pipeline.key_id.clone(),
            "--actor-model".into(),
            "r19-probe".into(),
        ];
        if let Some(path) = catalogue {
            args.push("--restore-catalogue".into());
            args.push(path.display().to_string());
        }
        args.push("--".into());
        args.push(env!("CARGO_BIN_EXE_gx").to_string());
        args.push(DEMO_SERVER_ARG.into());
        args
    }

    /// The flags a single-shot verb needs to reach the **same** server this fixture's wrap used.
    fn server_flags(&self) -> Vec<String> {
        vec![
            "--mcp-server".into(),
            env!("CARGO_BIN_EXE_gx").to_string(),
            "--mcp-server-arg".into(),
            DEMO_SERVER_ARG.into(),
            "--mcp-endpoint".into(),
            "stdio://r19".into(),
        ]
    }

    /// A `gx` invocation against this project, with the key store and the arrivals log a child
    /// server inherits.
    fn gx(&self) -> Command {
        let mut cmd = self.pipeline.gx();
        cmd.env(ARRIVALS_ENV, &self.arrivals);
        cmd
    }
}

/// One `notes.write` through `gx wrap`, and the `_meta` the agent was answered with.
fn one_write(fixture: &Fixture, catalogue: Option<&Path>) -> (Value, String) {
    let mut wrap = Wrap::spawn(
        &fixture.wrap_args(catalogue),
        &fixture.pipeline.home,
        &fixture.arrivals,
    );
    let answered = wrap.request(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": fixture.uri, "contents": AFTER },
        }),
    );
    let stderr = wrap.finish();
    (answered, stderr)
}

// ---------------------------------------------------------------------------
// H-02 (a) — `gx escalation approve` reaches the server this invocation named
// ---------------------------------------------------------------------------

/// 🔴 **H-02 (a)** — the ruling verb reads the MCP flags it accepts.
///
/// The Given is the one E-M3-4 actually produces on the MCP road: a tool with **no** restore
/// declaration, so the adapter cannot build an inverse, so gx asks a person. The person then rules
/// with `gx escalation approve --mcp-server …`, which before this repair answered
/// `this "gx" is connected to no MCP server` — naming, as its remedy, the flag it had just dropped.
///
/// The assertion is not that the command exits 0. It is that **the effect completes and the object
/// on the substrate holds the agent's text**, and that the server's own arrival log says the call
/// arrived. A ruling that returns 0 and leaves the note unchanged is the defect wearing a green
/// face.
#[test]
fn a_person_can_approve_an_escalated_mcp_change_and_the_effect_completes() {
    let fixture = fixture("r19_h02_cli");
    let (answered, _) = one_write(&fixture, None);

    let meta = &answered["result"]["_meta"];
    let tid = meta["gx/transformation"]
        .as_str()
        .unwrap_or_else(|| panic!("the escalated answer carries a transformation: {answered}"))
        .to_string();
    assert_eq!(
        meta["gx/verdict"], "Escalate",
        "the Given of this test is an escalation (E-M3-4: no declared inverse): {answered}"
    );
    assert_eq!(
        fixture.note_now(),
        BEFORE,
        "an escalated call sends nothing to the server"
    );

    let approved = support::run(
        fixture
            .gx()
            .args(fixture.server_flags())
            .args(["escalation", "approve", &tid])
            .args(["--reason", "a person read the change and allowed it"])
            .args(["--actor-key", &fixture.ruler]),
    );
    assert_eq!(
        approved.code, 0,
        "🔴 `req/279` H-02: `gx escalation approve` accepts `--mcp-server` and drops it, then \
         refuses the ruling with a sentence that names `--mcp-server` as the remedy. stderr:\n{}",
        approved.stderr
    );
    assert_eq!(
        approved.json()["state"],
        "Admitted",
        "43 T-5's human ruling = Admit: {}",
        approved.stdout
    );

    let committed = support::run(
        fixture
            .gx()
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    assert_eq!(
        committed.code, 0,
        "the admitted change commits through the same wiring: {}",
        committed.stderr
    );

    println!(
        "R19_H02A approve={} commit={} note={:?}",
        approved.stdout.trim(),
        committed.stdout.trim(),
        fixture.note_now()
    );
    assert_eq!(
        fixture.note_now(),
        AFTER,
        "🔴 `req/284` §2: the point of the road is the **object**. After a person approved the \
         change and it was committed, the note holds what the agent asked for"
    );
    let arrivals = fixture.arrivals_now();
    println!("R19_H02A arrivals={arrivals:?}");
    assert!(
        arrivals.iter().any(|line| line.contains("notes.write")),
        "the server's own log is the witness that the call arrived, not this process: {arrivals:?}"
    );
}

/// 🔴 **H-02 (c)** — the negative control, unchanged: without the flags the refusal stands.
///
/// `req/284` §1.1 (c): "a ruling shot without the flags keeps refusing (fail-closed unchanged)". A repair that opened
/// the road by making a flagless ruling reach a server would be a different, worse defect.
#[test]
fn a_ruling_without_the_flags_is_still_refused_and_the_object_does_not_move() {
    let fixture = fixture("r19_h02_negative");
    let (answered, _) = one_write(&fixture, None);
    let tid = answered["result"]["_meta"]["gx/transformation"]
        .as_str()
        .expect("a transformation")
        .to_string();

    let approved = support::run(
        fixture
            .gx()
            .args(["escalation", "approve", &tid])
            .args(["--reason", "a person ruled from a process wired to nothing"])
            .args(["--actor-key", &fixture.ruler]),
    );
    assert_ne!(
        approved.code, 0,
        "fail-closed: a process holding no transport cannot perform 43 §5's call"
    );
    assert!(
        approved.stderr.contains("connected to no MCP server"),
        "the refusal is the true sentence: {}",
        approved.stderr
    );
    assert_eq!(
        fixture.note_now(),
        BEFORE,
        "nothing reached the server, so nothing moved"
    );
}

// ---------------------------------------------------------------------------
// H-02 (c) — a flag a verb cannot read is refused, never accepted and dropped
// ---------------------------------------------------------------------------

/// 🔴 **H-02 (c)** — `req/284` §1.1: "if a verb still ignores the flag while accepting it, that pair becomes a usage
/// error (accept-and-ignore must not survive)".
///
/// `clap`'s `global = true` puts these six flags on every verb. Eight verbs read them (the five the
/// audit found, plus `escalation approve`/`reject` and `serve`, wired by this lane). The rest used
/// to take them and say nothing, which is the shape audit 19 called out by name: a flag that is
/// accepted and dropped tells an operator that something was wired.
#[test]
fn a_verb_that_cannot_use_the_mcp_flags_refuses_them_rather_than_dropping_them() {
    let fixture = fixture("r19_h02_usage");
    for verb in [
        vec!["key", "list"],
        vec![
            "cancel",
            "gx1:00000000000000000000000000000000000000000000000000000000000000",
        ],
        vec!["limits"],
        vec!["draft", "list"],
    ] {
        let run = support::run(
            fixture
                .gx()
                .args(["--mcp-server", env!("CARGO_BIN_EXE_gx")])
                .args(&verb),
        );
        assert_eq!(
            run.code,
            1,
            "🔴 `req/279` H-02: `gx {}` accepted `--mcp-server` and dropped it. 44 §1.4's 1 is \
             \"invalid input\", and a flag with nowhere to go is refused rather than dropped \
             (M6H3-5's rule, one flag along). stdout={} stderr={}",
            verb.join(" "),
            run.stdout,
            run.stderr
        );
        assert!(
            run.stderr.contains("--mcp-server"),
            "the refusal names the flag it is about: {}",
            run.stderr
        );
    }
}

// ---------------------------------------------------------------------------
// M-06 — a declaration-derived refusal is a refusal
// ---------------------------------------------------------------------------

/// A catalogue whose read declaration names a tool the server does not have.
///
/// Parses (nothing about it is judgeable without a call) and fails at escrow time with
/// `gx_adapter_mcp::READ_FAILURE_REFUSAL` under the fail-closed default — one of audit 19 M-06's
/// three declaration-derived constants.
fn unreachable_read_catalogue(dir: &Path) -> PathBuf {
    support::write_json(
        &dir.join("catalogue-unreachable-read.json"),
        &json!({
            "notes.write": {
                "restored_by": "notes.restore",
                "read_by": {
                    "by_tool": "notes.read",
                    "arguments": { "uri": { "forward": "uri" } },
                    "identity": [{ "answer": "/uri" }],
                },
                "arguments": { "uri": { "forward": "uri" }, "contents": { "prior_json": "/text" } },
            },
        }),
    )
}

/// 🔴 **M-06** — `req/279`: "the three refusals R17 built reach the agent saying <this is not a refusal>".
///
/// The machine was right the whole time (nothing reaches the server, the object does not move); the
/// **record** was the lie. The agent was told `this is not a refusal — it is a change gx could not
/// describe`, `_meta` said `gx/verdict: "not-reached"`, and the session summary counted `failed`.
#[test]
fn a_declaration_derived_refusal_is_recorded_as_a_refusal_and_not_as_a_failure() {
    let fixture = fixture("r19_m06");
    let catalogue = unreachable_read_catalogue(&fixture.pipeline.project);
    let (answered, stderr) = one_write(&fixture, Some(&catalogue));

    let result = &answered["result"];
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        result["isError"].as_bool().unwrap_or(false),
        "ruling ③'s shape is unchanged: {answered}"
    );
    assert!(
        text.contains(gx_adapter_mcp::READ_FAILURE_REFUSAL),
        "the Given is one of the three declaration-derived constants: {text}"
    );
    assert!(
        !text.contains("not a refusal"),
        "🔴 `req/279` M-06: `docs/LIMITS.md` calls this a **refusal**; the product's own surface \
         told the agent it was not one. text was:\n{text}"
    );
    assert_eq!(
        result["_meta"]["gx/verdict"], "refused-before-verdict",
        "🔴 `req/284` §1.3 — the third word. `not-reached` is for a change gx could not describe; \
         this is a change gx read the deployment's own declaration about and refused. _meta={}",
        result["_meta"]
    );

    let summary = session_summary(&stderr);
    println!(
        "R19_M06 meta={} session={}",
        result["_meta"], summary["session"]
    );
    assert_eq!(
        summary["session"]["failed"], 0,
        "🔴 M-06: the counter said `failed:1` for something gx refused. summary={summary}"
    );
    // 🔴 **`req/303` L-03 (R22)** — the arm's meaning is unchanged and its **field** moved. R19
    // wrote `denied: 1` because `GateOutcome::Denied` was the only carriage a refusal-before-a-
    // verdict had, and `req/279` M-06 deferred a fourth counter. The twenty-first audit measured
    // the cost: the session line `docs/LIMITS.md` calls the only trace of this outcome spent it in
    // a field whose own doc comment reads *how many the gate denied*, and the gate never saw this
    // call. The original assertion is kept beside its replacement rather than deleted, so a reader
    // of this file can see which fact moved and which did not.
    assert_eq!(
        summary["session"]["refused_before_verdict"], 1,
        "the call gx refused is counted where a refused call is counted — which since `req/303` \
         L-03 is the fourth bucket rather than `denied`. summary={summary}"
    );
    assert_eq!(
        summary["session"]["denied"], 0,
        "and it is counted there **instead of** in `denied`, which keeps the meaning its doc \
         comment gives it: a verdict the gate reached. summary={summary}"
    );
    assert_eq!(
        fixture.note_now(),
        BEFORE,
        "the mechanism was never in doubt: nothing reached the server"
    );
    assert!(
        !fixture
            .arrivals_now()
            .iter()
            .any(|line| line.contains("notes.write")),
        "zero arrivals — the read face was never called and neither was the effect"
    );
}

// ---------------------------------------------------------------------------
// M-07 — the printed undo remedy runs as printed
// ---------------------------------------------------------------------------

/// The ordinary, correct declaration: `notes.write` is undone by `notes.restore`.
fn round_trip_catalogue(dir: &Path) -> PathBuf {
    support::write_json(
        &dir.join("catalogue-round-trip.json"),
        &json!({
            "notes.write": { "restored_by": "notes.restore" },
            "notes.restore": { "restored_by": "notes.write" },
        }),
    )
}

/// 🔴 **M-07** — `req/263` §6 G6: a remedy a reader can run **without adding words**.
///
/// What was printed was `gx undo <TID> --mcp-server <the same server>`: a placeholder in the middle
/// and no restore catalogue, so running it as written escalated the undo (rc 4) and left the state
/// machine with no `undo` from there — the audit's "run it verbatim and the road closes".
///
/// This test takes the printed string, splits it, runs it as a child process, and asserts the note
/// went back. Nothing is added, nothing is substituted.
#[test]
fn the_undo_remedy_gx_wrap_prints_runs_exactly_as_printed() {
    let fixture = fixture("r19_m07");
    let catalogue = round_trip_catalogue(&fixture.pipeline.project);
    let (answered, _) = one_write(&fixture, Some(&catalogue));

    let meta = &answered["result"]["_meta"];
    assert_eq!(
        meta["gx/verdict"], "Admit",
        "the Given is an admitted, committed, escrowed call: {answered}"
    );
    assert_eq!(fixture.note_now(), AFTER, "the effect landed");

    let remedy = meta["gx/undo"]
        .as_str()
        .unwrap_or_else(|| panic!("`gx wrap` prints an undo remedy: {meta}"))
        .to_string();
    assert!(
        !remedy.contains('<'),
        "🔴 `req/279` M-07: the remedy carried the placeholder {remedy:?}, which is a word the \
         reader has to add"
    );
    assert!(
        remedy.contains("--mcp-restore-catalogue"),
        "🔴 `req/284` §1.4: without the catalogue the undo has no inverse to build and escalates. \
         remedy was {remedy:?}"
    );

    println!("R19_M07 remedy={remedy}");
    let words = shell_words(&remedy);
    assert_eq!(words.first().map(String::as_str), Some("gx"));
    let undone = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &fixture.pipeline.home)
            .env("USERPROFILE", &fixture.pipeline.home)
            .env(ARRIVALS_ENV, &fixture.arrivals)
            .arg("--project")
            .arg(&fixture.pipeline.project)
            .args(&words[1..]),
    );
    assert_eq!(
        undone.code, 0,
        "🔴 M-07: the printed remedy, run as printed, exited {}. stdout={} stderr={}",
        undone.code, undone.stdout, undone.stderr
    );
    assert_eq!(
        fixture.note_now(),
        BEFORE,
        "the undo the remedy names is the one that puts the note back"
    );
    assert!(
        fixture
            .arrivals_now()
            .iter()
            .any(|line| line.contains("notes.restore")),
        "the compensating call arrived at the server: {:?}",
        fixture.arrivals_now()
    );
}

/// Split a printed command line the way a shell would, honouring double quotes only.
///
/// The remedy is built by this repository and its only quoted members are paths, so a full POSIX
/// word splitter would be more machinery than the sentence has shapes.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}
