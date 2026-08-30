// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/291` H-01 / DR-46-19, on the real road** (`req/298` §1 item 1) — the catalogue whose
//! undo emptied the object, driven through the binary rather than through `invert`.
//!
//! # What the twentieth adversarial audit measured, verbatim
//!
//! ```text
//! A20_U1 verdict=Admit  after_write="the note after an agent wrote through gx wrap\n"
//! A20_U1_UNDO rc=0  ← a signed commit receipt (payload_type=application/vnd.glovrex.receipt+dagcbor)
//! A20_U1_AFTER_UNDO=""   restored=false destroyed=true
//! ```
//!
//! The catalogue's only difference from the one on the line below it was a single `contents`
//! member. gx admitted, signed, printed an undo, and running that undo left the note **empty** with
//! `rc=0`. Every fingerprint gx checks passed: they ask whether the object moved the way the applied
//! delta said, never whether it came back to where the forward call found it.
//!
//! # What this suite holds
//!
//! `req/288` moved the catalogue parse ahead of the spawn, so a declaration this crate can judge is
//! judged before a session exists. The repair therefore shows up here as **`gx wrap` refusing to
//! start**: no start-up JSON line, a usage error naming the entry, and the note untouched. The
//! negative control is the same file with one member added, which still starts, still admits, and
//! whose undo still puts the prior back — without it, "refuse every catalogue" satisfies the arm.
//!
//! `cfg(unix)` for the `chmod` on the launcher script.

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
const AFTER: &str = "the note after an agent wrote through gx wrap\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ENDPOINT: &str = "stdio://r20u";

/// 🔴 Fragments of `gx_adapter_mcp::TEMPLATE_NAMES_NO_PRIOR`, spelled here rather than imported.
///
/// The constant is this lane's; naming it makes this file un-buildable against the pre-repair
/// source, and a suite whose red is *cannot find value in crate `gx_adapter_mcp`* has measured the
/// symbol table rather than the defect. Spelled out, this file runs at `20f0635` — where the
/// catalogue starts a session, the gate admits, the commit is signed, and the undo gx prints
/// leaves the note empty — and the assertions above say so.
/// `crates/gx-adapter-mcp/tests/r20_template_prior_soundness.rs` holds these same fragments to the
/// constant's own source text, so the two cannot drift apart silently.
const NO_PRIOR_FRAGMENTS: &[&str] = &[
    "is a function of the forward call alone",
    "What to fix: draw one member from the prior",
    "`req/291` H-01 / DR-46-19",
];

/// 🔴 The seam: these fragments are the shipped constant's own words, held by reading
/// `gx-adapter-mcp`'s source. Without this, a rewording upstream leaves the arms above asserting
/// against a sentence gx no longer says, and they would go red for the wrong reason with no note
/// saying which.
#[test]
fn the_fragments_this_suite_holds_are_the_adapter_s_own_words() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-adapter-mcp/src/catalogue.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
    // `\`-continued string literals: the compiler drops the newline and the indentation after it.
    let joined = source
        .split("\\\n")
        .map(|part| part.trim_start_matches(' '))
        .collect::<Vec<_>>()
        .join("");
    for fragment in NO_PRIOR_FRAGMENTS {
        assert!(
            joined.contains(fragment),
            "🔴 no constant in `gx-adapter-mcp/src/catalogue.rs` carries {fragment:?} any more"
        );
    }
}

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
                "clientInfo": { "name": "r20u", "version": "0" },
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
    let launcher = pipeline.project.join("r20-server.sh");
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

    fn wrap_args(&self, catalogue: &Path) -> Vec<String> {
        vec![
            "--project".into(),
            self.pipeline.project.display().to_string(),
            "wrap".into(),
            "--endpoint".into(),
            ENDPOINT.into(),
            "--actor-key".into(),
            self.pipeline.key_id.clone(),
            "--actor-model".into(),
            "r20-probe".into(),
            "--restore-catalogue".into(),
            catalogue.display().to_string(),
            "--".into(),
            self.launcher.display().to_string(),
        ]
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

/// The `detail` of the problem+json a refused invocation writes to stderr.
fn detail_of(stderr: &str) -> String {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|v| v["detail"].as_str().map(str::to_string))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The two entries the audit's arm used, with the arm's own `notes.write` template.
fn catalogue_body(write_arguments: Value) -> Value {
    json!({
        "notes.write": { "restored_by": "notes.restore", "arguments": write_arguments },
        // Declared so that the undo's own inverse is constructible and an escalation on the return
        // leg cannot be mistaken for the finding.
        "notes.restore": {
            "restored_by": "notes.write",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
        }
    })
}

// ---------------------------------------------------------------------------
// The finding
// ---------------------------------------------------------------------------

/// 🔴 A catalogue whose restore template is built out of the forward call alone does not start a
/// session — and the note it would have emptied is untouched.
#[test]
fn a_forward_only_template_does_not_start_a_session() {
    let bed = bed("r20_u1_forward_only");
    let catalogue = bed.catalogue(
        "catalogue-forward-only.json",
        catalogue_body(json!({ "uri": { "forward": "uri" } })),
    );
    let run = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &bed.pipeline.home)
            .env("USERPROFILE", &bed.pipeline.home)
            .args(bed.wrap_args(&catalogue))
            .stdin(Stdio::null()),
    );
    println!(
        "R20_U1 rc={} stdout={} stderr={}",
        run.code,
        run.stdout.chars().take(200).collect::<String>(),
        run.stderr.chars().take(420).collect::<String>()
    );
    assert_ne!(
        run.code, 0,
        "🔴 `req/291` H-01: this catalogue admitted, signed a commit, printed an undo, and that \
         undo emptied the note with rc 0. It must not start a session"
    );
    assert!(
        !run.stdout.contains("\"gx\":\"wrap\""),
        "the discriminator is the start-up line: a session that begins is a session that can gate \
         a call under this declaration. stdout: {}",
        run.stdout
    );
    // 44 §1.3's refusal is problem+json, so the sentence arrives JSON-escaped: it is compared
    // after decoding rather than against the escaped bytes, which is a comparison that would pass
    // for the wrong reason on any constant carrying a quote.
    for fragment in NO_PRIOR_FRAGMENTS {
        assert!(
            detail_of(&run.stderr).contains(fragment),
            "and the refusal carries the constant's wording ({fragment:?}), so a reader of this \
             stderr is told what to fix: {}",
            run.stderr
        );
    }
    assert_eq!(
        bed.note_now(),
        BEFORE,
        "nothing was written on the way to this refusal"
    );
}

/// 🔴 The empty template — `req/298` §1 item 1's second clause. The audit's `A20_U2` arm reached an
/// **escalation** on the return leg, which is a later gate and a weaker one: it depends on the
/// escrow being unresolvable rather than on the declaration being wrong. It is stopped here now.
#[test]
fn an_empty_template_does_not_start_a_session_either() {
    let bed = bed("r20_u2_empty");
    let catalogue = bed.catalogue("catalogue-empty.json", catalogue_body(json!({})));
    let run = support::run(
        Command::new(env!("CARGO_BIN_EXE_gx"))
            .env("HOME", &bed.pipeline.home)
            .env("USERPROFILE", &bed.pipeline.home)
            .args(bed.wrap_args(&catalogue))
            .stdin(Stdio::null()),
    );
    println!(
        "R20_U2 rc={} stderr={}",
        run.code,
        run.stderr.chars().take(360).collect::<String>()
    );
    assert_ne!(run.code, 0, "an empty template names no prior either");
    assert!(
        detail_of(&run.stderr).contains("no members at all"),
        "and it is refused for being empty rather than for something else: {}",
        run.stderr
    );
    assert_eq!(bed.note_now(), BEFORE);
}

// ---------------------------------------------------------------------------
// The negative control — the same file with one member added
// ---------------------------------------------------------------------------

/// 🔴 The declaration that draws the prior still starts, still admits, and its undo still restores.
///
/// This is the whole of the difference the audit measured — one `contents` member — held from the
/// other side, so that the two arms above cannot be satisfied by a gx that refuses everything.
#[test]
fn the_same_declaration_with_the_prior_named_still_does_the_round_trip() {
    let bed = bed("r20_u0_control");
    let catalogue = bed.catalogue(
        "catalogue-sound.json",
        catalogue_body(json!({ "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" })),
    );
    let mut agent = Agent::open(&bed.wrap_args(&catalogue), &bed.pipeline.home);
    let answered = agent.ask(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": bed.uri, "contents": AFTER } }),
    );
    let _stderr = agent.close();
    let meta = &answered["result"]["_meta"];
    println!(
        "R20_U0 verdict={} after_write={:?}",
        meta["gx/verdict"],
        bed.note_now()
    );
    assert_eq!(
        meta["gx/verdict"], "Admit",
        "the sound declaration admits: {answered}"
    );
    assert_eq!(bed.note_now(), AFTER, "and the effect reaches the note");

    let tid = meta["gx/transformation"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    let undone = bed.undo(&tid, &catalogue);
    println!(
        "R20_U0_UNDO rc={} after_undo={:?}",
        undone.code,
        bed.note_now()
    );
    assert_eq!(undone.code, 0, "the undo runs: {}", undone.stderr);
    assert_eq!(
        bed.note_now(),
        BEFORE,
        "🔴 and it **restores** — which is the fact the forward-only template answered `true` about \
         and did not deliver"
    );
}
