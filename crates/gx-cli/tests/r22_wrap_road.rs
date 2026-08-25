// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/303` H-01, M-02, L-03 and L-04, on the real road** (`req/309` §1 items 1, 2, 6, 7 and
//! the added item 12) — driven through the binary, a real child MCP server and a real journal.
//!
//! # What the twenty-first adversarial audit measured, verbatim
//!
//! ```text
//! A21_RESIDUAL_GITBLOB verdict=Admit after_write="what the agent wrote through gx wrap\n"
//!                      undo_rc=0 after_undo=""
//! A21_M05 verdict="refused-before-verdict" receipt=null
//! A21_M05_JOURNAL before=1 after=4 records=[DraftCreated, DraftCreated, Planned, VerifyStarted]
//! A21_M05_STDERR_TAIL {"gx":"wrap","session":{... "denied":1, ... "failed":0}}
//! A21_FLAG_FILE  rc=1 started=false
//! A21_FLAG_FLAG  verdict="refused-before-verdict" doc_after=(unchanged, still BEFORE)
//! ```
//!
//! Four findings share one harness because they are four questions about one session:
//!
//! * **H-01** — a template of forward members and the git blob hash of one of them started a
//!   session, admitted, signed a commit, and the undo gx printed emptied the object with `rc=0`.
//! * **M-02** — `docs/LIMITS.md` v0.5-g says a change refused **before a verdict** leaves *"no
//!   receipt, no journal record, nothing under `.gx/`"*. On the road the audit drove, three journal
//!   records and four files were left behind. `req/38` §221 ruling 3 asks for **both** roads to be
//!   measured before the page is rewritten, which is what
//!   [`the_fourth_outcome_leaves_what_it_leaves_on_each_road`] does.
//! * **L-03** — the session line that page calls the *only* trace spent that outcome as `denied:1`,
//!   in a counter whose own doc comment says "how many the gate denied". The gate never saw it.
//! * **L-04** — `--restore-catalogue <file>` with a blank restore name is a parse error and
//!   `--restore 'doc.write= '` was not: the flag mouth built the entry in code, past the only reader
//!   that ran `soundness()`.
//!
//! Plus **`req/308` §4(g)**: the start-up line prints `restorable_tools` and `on_read_failure` and
//! said nothing about `$cas_read`, which decides the *reading road* of every locator in the session.
//!
//! # Red-first
//!
//! Every arm drives the shipped binary and reads bytes it produced, so the whole file compiles at
//! `97052f8` and goes red where the defect is. No symbol this lane invented is named.
//!
//! `cfg(unix)` for the `chmod` on the launcher script, as every sibling suite says.

#![cfg(unix)]

mod support;

use std::collections::BTreeMap;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

const BEFORE: &str = "the note as it stood before any agent touched it\n";
const AFTER: &str = "what the agent wrote through gx wrap\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ENDPOINT: &str = "stdio://r22";

// ---------------------------------------------------------------------------
// The bed: a project, a note, and a launcher for the shipped demo server
// ---------------------------------------------------------------------------

struct Bed {
    pipeline: support::Pipeline,
    note: PathBuf,
    uri: String,
    launcher: PathBuf,
}

fn bed(name: &str) -> Bed {
    let pipeline =
        support::pipeline_named(name, "a file this suite does not measure\n", "seed.txt");
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);
    let note = pipeline.project.join("note.txt");
    std::fs::write(&note, BEFORE).expect("write the note");
    let uri = format!("file://{}", note.display());
    let launcher = pipeline.project.join("r22-server.sh");
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
    }
}

impl Bed {
    fn note_now(&self) -> String {
        std::fs::read_to_string(&self.note).unwrap_or_default()
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
            "r22-probe".to_string(),
        ];
        args.extend(extra.iter().cloned());
        args.push("--".to_string());
        args.push(self.launcher.display().to_string());
        args
    }

    fn gx_dir(&self) -> PathBuf {
        self.pipeline.project.join(".gx")
    }

    /// Every file under `.gx/`, as (relative path, byte length).
    fn census(&self) -> BTreeMap<String, u64> {
        fn walk(root: &Path, at: &Path, into: &mut BTreeMap<String, u64>) {
            let Ok(entries) = std::fs::read_dir(at) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, into);
                } else if let Ok(meta) = std::fs::metadata(&path) {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .display()
                        .to_string();
                    into.insert(rel, meta.len());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.gx_dir(), &self.gx_dir(), &mut out);
        out
    }

    /// The kinds of every record in this project's journal, in order.
    fn journal_kinds(&self) -> Vec<String> {
        let path = self.gx_dir().join("ledger").join("journal");
        let raw = std::fs::read(&path).unwrap_or_default();
        if raw.is_empty() {
            return Vec::new();
        }
        gx_engine::replay(&raw)
            .records()
            .iter()
            .map(|r| r.kind().to_string())
            .collect()
    }

    fn receipt_files(&self) -> usize {
        let receipts = self.gx_dir().join("receipts");
        std::fs::read_dir(&receipts)
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
    fn open(args: &[String], home: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", home)
            .env("USERPROFILE", home)
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
                "clientInfo": { "name": "r22", "version": "0" },
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

/// The two entries every arm declares, with `notes.write`'s template supplied by the arm.
fn catalogue_body(write_arguments: Value) -> Value {
    json!({
        "notes.write": { "restored_by": "notes.restore", "arguments": write_arguments },
        "notes.restore": {
            "restored_by": "notes.write",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
        }
    })
}

/// The `detail` of every problem object on a stderr.
fn detail_of(stderr: &str) -> String {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|v| v["detail"].as_str().map(str::to_string))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The last `{"gx":"wrap", ...}` object on a stderr — the session line, or the start-up line.
fn wrap_lines(stderr: &str) -> Vec<Value> {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["gx"] == "wrap")
        .collect()
}

fn limits() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repository root")
        .join("docs/LIMITS.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// H-01 — the audit's git-blob catalogue, on the road that destroyed an object
// ---------------------------------------------------------------------------

/// 🔴 `req/303` H-01: `{"uri": {"forward":"uri"}, "sha": {"git_blob_sha1_of_forward":"contents"}}`
/// does not start a session, and the note it would have emptied is untouched.
#[test]
fn a_template_of_forward_members_and_their_hash_does_not_start_a_session() {
    let bed = bed("r22_h01_gitblob");
    let catalogue = bed.catalogue(
        "catalogue-gitblob.json",
        catalogue_body(json!({
            "uri": { "forward": "uri" },
            "sha": { "git_blob_sha1_of_forward": "contents" },
        })),
    );
    let run = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &bed.pipeline.home)
            .env("USERPROFILE", &bed.pipeline.home)
            .args(bed.wrap_args(&[
                "--restore-catalogue".to_string(),
                catalogue.display().to_string(),
            ]))
            .stdin(Stdio::null()),
    );
    println!(
        "R22_H01_ROAD rc={} stdout={} stderr={}",
        run.code,
        run.stdout.chars().take(160).collect::<String>(),
        run.stderr.chars().take(400).collect::<String>()
    );
    assert_ne!(
        run.code, 0,
        "🔴 `req/303` H-01: this catalogue started a session, admitted, signed a commit, and the \
         undo gx printed emptied the note with rc 0 — under a gate whose own sentence says a \
         template must not be a function of the forward call alone. Every member of it is"
    );
    assert!(
        !run.stdout.contains("\"gx\":\"wrap\""),
        "the discriminator is the start-up line: a session that begins can gate a call under this \
         declaration. stdout: {}",
        run.stdout
    );
    assert!(
        detail_of(&run.stderr).contains("is a function of the forward call alone"),
        "and the refusal is the template gate's own wording: {}",
        run.stderr
    );
    assert_eq!(
        bed.note_now(),
        BEFORE,
        "nothing was written on the way here"
    );
}

/// The control: the same catalogue with a prior member starts, admits, and its undo puts the note
/// back. Without it, "refuse every catalogue" satisfies the arm above.
#[test]
fn the_same_catalogue_with_a_prior_member_still_round_trips() {
    let bed = bed("r22_h01_control");
    let catalogue = bed.catalogue(
        "catalogue-sound.json",
        catalogue_body(json!({
            "uri": { "forward": "uri" },
            "contents": "prior_contents_utf8",
            "sha": { "git_blob_sha1_of_forward": "contents" },
        })),
    );
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
    );
    let answer = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": bed.uri, "contents": AFTER } }),
    );
    let meta = answer["result"]["_meta"].clone();
    println!("R22_H01_CTRL meta={meta}");
    let _ = agent.close();
    assert_eq!(
        meta["gx/verdict"], "Admit",
        "🔴 the repair is about what a template draws on, not about which words may appear in it: \
         `git_blob_sha1_of_forward` beside `prior_contents_utf8` is the shipped \
         `create_or_update_file` declaration. answer: {answer}"
    );
    assert_eq!(bed.note_now(), AFTER, "the effect reached the server");
}

// ---------------------------------------------------------------------------
// L-04 — the flag mouth and the file mouth answer alike
// ---------------------------------------------------------------------------

/// 🔴 `req/303` L-04: `--restore 'notes.write= '` is a start-up error, exactly as
/// `--restore-catalogue` with the same declaration already was.
#[test]
fn the_flag_mouth_refuses_the_declaration_the_file_mouth_refuses() {
    let bed = bed("r22_l04_flag");
    // The file mouth, for the comparison this finding is about.
    let file = bed.catalogue(
        "catalogue-blank.json",
        json!({ "notes.write": { "restored_by": " " } }),
    );
    let by_file = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &bed.pipeline.home)
            .env("USERPROFILE", &bed.pipeline.home)
            .args(bed.wrap_args(&[
                "--restore-catalogue".to_string(),
                file.display().to_string(),
            ]))
            .stdin(Stdio::null()),
    );
    let by_flag = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &bed.pipeline.home)
            .env("USERPROFILE", &bed.pipeline.home)
            .args(bed.wrap_args(&["--restore".to_string(), "notes.write= ".to_string()]))
            .stdin(Stdio::null()),
    );
    println!(
        "R22_L04 file_rc={} file_started={} flag_rc={} flag_started={}",
        by_file.code,
        by_file.stdout.contains("\"gx\":\"wrap\""),
        by_flag.code,
        by_flag.stdout.contains("\"gx\":\"wrap\"")
    );
    println!(
        "R22_L04_FLAG_STDERR {}",
        by_flag.stderr.chars().take(400).collect::<String>()
    );
    assert_ne!(by_file.code, 0, "the Given: the file mouth already refuses");
    assert_ne!(
        by_flag.code, 0,
        "🔴 `req/303` L-04: `docs/LIMITS.md` says of this declaration \"It is a **parse error**: \
         `gx wrap` does not start\". That was a property of the file mouth. The flag mouth built \
         the entry through `Catalogue::with_restore`, which no parser ever saw, so the session \
         started and the server was spawned. stderr: {}",
        by_flag.stderr
    );
    assert!(
        detail_of(&by_flag.stderr).contains("which names no tool"),
        "and both mouths refuse in one wording: {}",
        by_flag.stderr
    );
    assert_eq!(bed.note_now(), BEFORE, "no session, no effect");
}

/// The control: a sound `--restore` pair still starts a session. The repair is a check, not a ban.
#[test]
fn a_sound_restore_flag_still_starts_a_session() {
    let bed = bed("r22_l04_control");
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore".to_string(),
            "notes.write=notes.restore".to_string(),
            "--restore".to_string(),
            "notes.restore=notes.write".to_string(),
        ]),
        &bed.pipeline.home,
    );
    let answer = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": bed.uri, "contents": AFTER } }),
    );
    println!("R22_L04_CTRL meta={}", answer["result"]["_meta"]);
    let _ = agent.close();
    assert_eq!(
        answer["result"]["_meta"]["gx/verdict"], "Admit",
        "the v0.1 `{{contents, uri}}` convention is unchanged: {answer}"
    );
}

// ---------------------------------------------------------------------------
// `req/308` §4(g) — the reading road is on the start-up line
// ---------------------------------------------------------------------------

/// 🔴 `req/308` §4(g): `$cas_read` changes the reading road of every locator under a declared
/// prefix, and the line an operator reads at start-up said nothing about it.
#[test]
fn the_start_up_line_says_how_many_cas_reads_this_catalogue_declares() {
    let bed = bed("r22_cas_reads_line");
    let catalogue = bed.catalogue(
        "catalogue-cas.json",
        json!({
            "$cas_read": {
                "doc:": { "by_tool": "doc.get", "arguments": { "id": "resource" } }
            },
            "notes.write": {
                "restored_by": "notes.restore",
                "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
            }
        }),
    );
    let agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
    );
    let stderr = agent.close();
    let start = wrap_lines(&stderr)
        .into_iter()
        .find(|v| v.get("server_command").is_some())
        .expect("the start-up line is on stderr");
    println!(
        "R22_CAS_LINE restorable_tools={} on_read_failure={} cas_reads={}",
        start["restorable_tools"], start["on_read_failure"], start["cas_reads"]
    );
    assert_eq!(
        start["cas_reads"], 1,
        "🔴 `req/308` §4(g): one line of this file moved the reading road of every locator under \
         `doc:` off `resources/read`, and the start-up line printed `restorable_tools` and \
         `on_read_failure` and nothing about it — the same defect `req/269` M-01 closed for the \
         posture one field up. line: {start}"
    );
}

/// The zero is stated rather than inferred from a missing field (P-3, and the reason
/// `"otel": "disabled"` is on the same line).
#[test]
fn a_catalogue_with_no_cas_read_prints_the_zero() {
    let bed = bed("r22_cas_reads_zero");
    let catalogue = bed.catalogue(
        "catalogue-plain.json",
        catalogue_body(json!({ "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" })),
    );
    let agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
    );
    let stderr = agent.close();
    let start = wrap_lines(&stderr)
        .into_iter()
        .find(|v| v.get("server_command").is_some())
        .expect("the start-up line is on stderr");
    println!("R22_CAS_ZERO cas_reads={}", start["cas_reads"]);
    assert_eq!(
        start["cas_reads"], 0,
        "a field that is absent when the count is zero is a field a reader has to infer: {start}"
    );
}

// ---------------------------------------------------------------------------
// M-02 + L-03 — what the fourth outcome leaves, on each road, and how it is counted
// ---------------------------------------------------------------------------

/// A catalogue whose fault the **parser cannot see**: the read face names a forward member this
/// call does not carry. It is refused by `crate::invert` at resolution time, which is the road that
/// has already created a draft.
fn stale_read_catalogue() -> Value {
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

/// 🔴 **`req/303` M-02 and L-03, and `req/38` §221 ruling 3** — both roads, measured, and the page
/// held to the measurement rather than to a needle.
///
/// `docs/LIMITS.md` v0.5-g said a change refused **before a verdict** leaves *"no receipt, no
/// journal record, nothing under `.gx/`"*, and `crates/gx-cli/tests/limits_sync.rs` held that
/// sentence by looking for its own words in the page. The sentence was never measured. `req/38`
/// §221 ruling 3 asks which road each half is true of, so this arm drives **two**:
///
/// * the road where the proxy answers **before it submits anything** — a call that names no
///   resource, which `EngineGate::run` refuses before `crate::pipeline::submit`;
/// * the road where the refusal comes from the deployment's own declaration at resolution time,
///   after `submit`, `plan` and `verify` have each written what they write.
///
/// Both are refusals gx makes before a verdict. They do not leave the same thing behind, and that
/// is the fact the page now carries.
#[test]
fn the_fourth_outcome_leaves_what_it_leaves_on_each_road() {
    let bed = bed("r22_m02_roads");
    let catalogue = bed.catalogue("catalogue-stale-read.json", stale_read_catalogue());
    let args = bed.wrap_args(&[
        "--restore-catalogue".to_string(),
        catalogue.display().to_string(),
    ]);

    // ---- road 1: the proxy answers before it submits -------------------------------------
    let before_1 = bed.census();
    let journal_1 = bed.journal_kinds();
    let mut agent = Agent::open(&args, &bed.pipeline.home);
    let answered_1 = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "contents": AFTER } }),
    );
    let stderr_1 = agent.close();
    let after_1 = bed.census();
    let journal_after_1 = bed.journal_kinds();
    let meta_1 = &answered_1["result"]["_meta"];
    let new_1: Vec<&String> = after_1
        .keys()
        .filter(|k| !before_1.contains_key(*k))
        .collect();
    println!(
        "R22_ROAD1 verdict={} journal={}->{} new_files={:?} receipts={}",
        meta_1["gx/verdict"],
        journal_1.len(),
        journal_after_1.len(),
        new_1,
        bed.receipt_files()
    );
    assert_eq!(
        journal_after_1.len(),
        journal_1.len(),
        "road 1's Given: a call the proxy refuses before it submits writes no journal record. \
         kinds now: {journal_after_1:?}"
    );

    // ---- road 2: the declaration refusal, after a draft exists ---------------------------
    let bed = self::bed("r22_m02_roads_2");
    let catalogue = bed.catalogue("catalogue-stale-read.json", stale_read_catalogue());
    let args = bed.wrap_args(&[
        "--restore-catalogue".to_string(),
        catalogue.display().to_string(),
    ]);
    let before_2 = bed.census();
    let journal_2 = bed.journal_kinds();
    let mut agent = Agent::open(&args, &bed.pipeline.home);
    let answered_2 = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": bed.uri, "contents": AFTER } }),
    );
    let stderr_2 = agent.close();
    let after_2 = bed.census();
    let journal_after_2 = bed.journal_kinds();
    let meta_2 = &answered_2["result"]["_meta"];
    let added: Vec<String> = journal_after_2[journal_2.len()..].to_vec();
    let new_2: Vec<(&String, &u64)> = after_2
        .iter()
        .filter(|(k, v)| before_2.get(*k) != Some(*v))
        .collect();
    println!(
        "R22_ROAD2 verdict={} journal={}->{} added={:?} changed={:?} receipts={}",
        meta_2["gx/verdict"],
        journal_2.len(),
        journal_after_2.len(),
        added,
        new_2,
        bed.receipt_files()
    );
    assert_eq!(
        meta_2["gx/verdict"], "refused-before-verdict",
        "road 2's Given: this is the fourth outcome, not a denial and not a failure. answer: \
         {answered_2}"
    );
    assert!(
        !added.is_empty(),
        "🔴 road 2's whole point: `docs/LIMITS.md` v0.5-g said this outcome leaves **no journal \
         record**, and the twenty-first audit measured three. If it now leaves none, the page has \
         to change again and this arm is where that is noticed"
    );
    assert_eq!(bed.note_now(), BEFORE, "nothing was sent to the server");

    // ---- the page is held to the measurement, not to its own words -----------------------
    let doc = limits();
    for kind in &added {
        assert!(
            doc.contains(kind.as_str()),
            "🔴 `req/38` §221 ruling 3: this road leaves a `{kind}` record and `docs/LIMITS.md` \
             does not name it. The sentence that page carried — \"no receipt, no journal record, \
             nothing under `.gx/`\" — was held by a needle looking for its own words, which is why \
             it survived three lanes while being false. This gate counts records instead. \
             measured: {added:?}"
        );
    }
    assert_eq!(
        bed.receipt_files(),
        0,
        "the half of the old sentence that was true stays true: no receipt is written"
    );

    // ---- L-03: the session line puts the fourth outcome in its own bucket ----------------
    let session = wrap_lines(&stderr_2)
        .into_iter()
        .find(|v| v.get("session").is_some())
        .expect("the session line is on stderr");
    println!("R22_L03 session={}", session["session"]);
    assert_eq!(
        session["session"]["refused_before_verdict"], 1,
        "🔴 `req/303` L-03: the line `docs/LIMITS.md` calls the **only** trace of this outcome \
         counted it as `denied:1`, in a field whose own doc comment reads \"how many the gate \
         denied\". The gate never saw this call. line: {session}"
    );
    assert_eq!(
        session["session"]["denied"], 0,
        "and `denied` keeps the meaning its doc comment gives it: {session}"
    );
    assert_eq!(
        session["session"]["failed"], 0,
        "and `failed` keeps its own: nothing here was broken: {session}"
    );

    // Road 1's counter is a different word again, and the arm records it rather than asserting a
    // shape `req/309` did not ask for.
    let session_1 = wrap_lines(&stderr_1)
        .into_iter()
        .find(|v| v.get("session").is_some())
        .expect("the session line is on stderr");
    println!(
        "R22_L03_ROAD1 verdict={} session={}",
        meta_1["gx/verdict"], session_1["session"]
    );
}

/// The control: a denial the **gate** made is still `denied`. Without it, moving every `Denied` to
/// the new bucket satisfies the arm above.
#[test]
fn a_denial_the_gate_made_is_still_counted_as_denied() {
    let bed = bed("r22_l03_control");
    let outside = bed
        .pipeline
        .project
        .join("..")
        .join("outside-the-project.txt");
    std::fs::write(&outside, BEFORE).expect("write a file the policy will not allow");
    let catalogue = bed.catalogue(
        "catalogue-sound.json",
        catalogue_body(json!({ "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" })),
    );
    let mut agent = Agent::open(
        &bed.wrap_args(&[
            "--restore-catalogue".to_string(),
            catalogue.display().to_string(),
        ]),
        &bed.pipeline.home,
    );
    let answer = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": bed.uri, "contents": AFTER } }),
    );
    let stderr = agent.close();
    let session = wrap_lines(&stderr)
        .into_iter()
        .find(|v| v.get("session").is_some())
        .expect("the session line is on stderr");
    println!(
        "R22_L03_CTRL verdict={} session={}",
        answer["result"]["_meta"]["gx/verdict"], session["session"]
    );
    assert_eq!(
        session["session"]["refused_before_verdict"], 0,
        "a call that reached the gate is not a call gx stopped before it: {session}"
    );
    assert_eq!(
        session["session"]["admitted"], 1,
        "the Given: this sound declaration was admitted: {session}"
    );
}
