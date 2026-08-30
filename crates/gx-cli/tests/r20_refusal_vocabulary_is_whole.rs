// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/291` M-03** (`req/298` §1 item 2) — the third word, over the **whole** family.
//!
//! # What the twentieth adversarial audit measured
//!
//! R19 gave a declaration-caused refusal its own word on the agent's surface
//! (`gx/verdict: "refused-before-verdict"`) and chose which pipeline errors get it by a verbatim
//! `contains` over **three** of `gx-adapter-mcp`'s exported constants. R18 landed the same day and
//! exported **two more** (`OBJECT_UNNAMED_REFUSAL`, `IDENTITY_IGNORES_THE_ANSWER_REFUSAL`). Neither
//! lane saw the other's half.
//!
//! The audit ran five arms that differ only in the read tool's answer and the spelling of
//! `identity`. Three came back `refused-before-verdict` with `denied:1 failed:0`. Two came back
//! `not-reached`, telling the agent *this is **not** a refusal — it is a change gx could not
//! describe*, with `denied:0 failed:1`. The mechanism was right in all five (object unmoved, effect
//! never sent); the **record** was wrong in two, which is the exact defect R19 wrote
//! `declaration_refusal` to close.
//!
//! # What this suite holds, and why the first arm is a source scan
//!
//! The hedge in `declaration_refusal`'s own doc — *a fourth constant added upstream without a
//! constant would fall through to `Failed`, which is the safe direction* — did not hold here,
//! because the two missing ones **do** have constants and **are** exported. Prose cannot count. So
//! the first arm reads `gx-adapter-mcp`'s source for its `pub const … _REFUSAL` declarations and
//! holds that number — and that **set** — equal to the constants `wrap.rs` wires: the lane that
//! exports a sixth constant is red here until it lists it, which is the property `req/298` §1
//! item 2 asked for. A second arm holds `DECLARATION_REFUSALS`'s declared `[&str; N]` to the same
//! number, which is the length-in-the-type half of that item.
//!
//! 🔴 **Both arms read source rather than linking `DECLARATION_REFUSALS`, and that is deliberate.**
//! The constant is this lane's own; a test that names it cannot be compiled against the pre-repair
//! tree, so this suite's red would have been a missing-symbol error — the same error a suite
//! measuring nothing would give. Reading the source, the file builds at `20f0635` and fails on its
//! assertions: three wired against five exported, and two live arms in which gx tells an agent
//! *this is not a refusal* about a call it refused. `req/299` records both reds.
//!
//! # Denominator
//!
//! * The real-road arm drives **one** of the two constants R18 added
//!   (`OBJECT_UNNAMED_REFUSAL`, reached with a read answer that is not JSON). The other is covered
//!   by the classifier arm and by `crates/gx-adapter-mcp/tests/r18_declaration_soundness.rs`, which
//!   holds its wording; it is **not** driven through a live proxy here.
//! * `DECLARATION_UNSOUND_REFUSAL` is not reachable from a `gx wrap` invocation at all — `req/288`
//!   moved the catalogue parse ahead of the spawn, so that shape is a usage error first. The audit
//!   measured the same thing and said so.

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

const BEFORE: &str = "the document as it was\n";
const ENDPOINT: &str = "stdio://r20v";

fn adapter_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-adapter-mcp/src")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

fn cli_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

/// Every `pub const <NAME>_REFUSAL` a source file declares, in the order it declares them.
fn exported_refusal_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub const "))
        .filter_map(|rest| rest.split(':').next())
        .map(str::trim)
        .filter(|name| name.ends_with("_REFUSAL"))
        .map(str::to_string)
        .collect()
}

/// 🔴 Every declaration refusal `wrap.rs` actually **wires**, read out of its source in the order
/// it names them.
///
/// Read rather than linked, and the reason is red-first. `DECLARATION_REFUSALS` is a symbol this
/// lane created: a test naming it cannot be built against the pre-repair source at all, so the
/// whole file's red would be *cannot find value in module `wrap`* — a message that is equally true
/// of a file measuring nothing. Read out of the source, this suite compiles at `20f0635` and goes
/// red where the defect is: three wired against five exported, and two live arms telling an agent
/// `not-reached` about a call gx refused.
///
/// It is not the weaker instrument for being a scan. `.len()` would count an array; this counts
/// the constants a human can see in the file, in either shape the file has had — the inline array
/// inside `declaration_refusal` (pre-repair) or the named `DECLARATION_REFUSALS` (post) — and
/// [`the_wired_list_is_a_fixed_length_array`] holds the declared length to it besides.
fn wired_refusal_names(source: &str) -> Vec<String> {
    source
        .match_indices("gx_adapter_mcp::")
        .filter_map(|(at, marker)| {
            let rest = &source[at + marker.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_uppercase() || c == '_'))
                .unwrap_or(rest.len());
            Some(rest[..end].to_string()).filter(|name| name.ends_with("_REFUSAL"))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The mechanical gate
// ---------------------------------------------------------------------------

/// 🔴 The count `req/298` §1 item 2 asked for: the array is as long as the adapter's export list.
#[test]
fn the_classifier_names_every_declaration_refusal_the_adapter_exports() {
    let names = exported_refusal_names(&adapter_src("invert.rs"));
    let wired = wired_refusal_names(&cli_src("wrap.rs"));
    println!(
        "R20_M03_EXPORTED n={} names={:?} wired={} {:?}",
        names.len(),
        names,
        wired.len(),
        wired
    );
    assert!(
        names.len() >= 5,
        "the scan found {} constants, which is fewer than the five this repository is known to \
         export — the scan itself has drifted and would pass by finding nothing: {names:?}",
        names.len()
    );
    assert!(
        !wired.is_empty(),
        "the scan of `wrap.rs` found no `gx_adapter_mcp::…_REFUSAL` at all, which is the one way \
         this gate passes by measuring nothing: the classifier was rewritten into a shape this \
         probe cannot read, and the probe must be rewritten with it"
    );
    let mut missing: Vec<&String> = names.iter().filter(|n| !wired.contains(n)).collect();
    missing.sort();
    assert_eq!(
        wired.len(),
        names.len(),
        "🔴 `req/291` M-03: `gx-adapter-mcp` exports {} declaration refusals and \
         `declaration_refusal` matches on {}. The ones it does not match are told to the agent as \
         `not-reached` — \"this is not a refusal\" — about a call gx refused before the gate. \
         Exported: {names:?}. Not wired: {missing:?}",
        names.len(),
        wired.len()
    );
    assert!(
        missing.is_empty(),
        "🔴 the counts are equal and the sets are not, so one constant was swapped for another \
         rather than added: {missing:?} is exported and unwired"
    );
}

/// 🔴 The length `req/298` §1 item 2 asked to be part of the type, held on the source because
/// naming the constant would put this file's red back into the compiler (see
/// [`wired_refusal_names`]). A `Vec` that happens to be short today answers "how many are there"
/// with today's contents; a `[&str; N]` answers it with `N`, and this is where `N` is checked.
#[test]
fn the_wired_list_is_a_fixed_length_array() {
    let source = cli_src("wrap.rs");
    let wired = wired_refusal_names(&source);
    let declared = source
        .split_once("pub const DECLARATION_REFUSALS: [&str; ")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(len, _)| len.trim().to_string());
    println!("R20_M03_ARRAY declared={declared:?} wired={}", wired.len());
    let declared = declared.expect(
        "🔴 `req/291` M-03: `wrap.rs` declares no `pub const DECLARATION_REFUSALS: [&str; N]`. \
         Pre-repair the list was an anonymous array inside `declaration_refusal` with no length to \
         hold and no name to count, which is the shape that let three stand for five",
    );
    assert_eq!(
        declared,
        wired.len().to_string(),
        "the array's declared length and the constants it lists disagree"
    );
}

/// 🔴 The other direction: the count above cannot be satisfied by moving a constant to a file the
/// scan does not read. `invert.rs` is where a declaration refusal lives, and this holds it there.
#[test]
fn no_other_module_of_the_adapter_declares_a_declaration_refusal() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-adapter-mcp/src");
    let mut elsewhere: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the adapter's src is readable") {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("invert.rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("readable");
        for name in exported_refusal_names(&source) {
            elsewhere.push(format!("{}: {name}", path.display()));
        }
    }
    println!("R20_M03_ELSEWHERE n={} {:?}", elsewhere.len(), elsewhere);
    assert!(
        elsewhere.is_empty(),
        "a `pub const … _REFUSAL` outside `invert.rs` is a constant the count above cannot see, \
         which is the one way that gate can be satisfied while the defect it measures is open: \
         {elsewhere:?}"
    );
}

/// 🔴 And the classifier answers `true` for each of the five, by value rather than by count.
#[test]
fn each_exported_declaration_refusal_is_classified_as_one() {
    for (name, constant) in [
        ("READ_FAILURE_REFUSAL", gx_adapter_mcp::READ_FAILURE_REFUSAL),
        (
            "DECLARATION_UNSOUND_REFUSAL",
            gx_adapter_mcp::DECLARATION_UNSOUND_REFUSAL,
        ),
        (
            "OBJECT_IDENTITY_REFUSAL",
            gx_adapter_mcp::OBJECT_IDENTITY_REFUSAL,
        ),
        (
            "OBJECT_UNNAMED_REFUSAL",
            gx_adapter_mcp::OBJECT_UNNAMED_REFUSAL,
        ),
        (
            "IDENTITY_IGNORES_THE_ANSWER_REFUSAL",
            gx_adapter_mcp::IDENTITY_IGNORES_THE_ANSWER_REFUSAL,
        ),
        // 🔴 **`req/303` M-03 (R22)** — the sixth. The list above was written when every entry
        // fault was about a read face; this is the sentence a fault about a `restored_by` or an
        // `arguments` template carries instead. Added here rather than only in the count gate
        // because this arm asks the question by **value**, and a constant the classifier does not
        // match is a refusal an agent is told is "not a refusal".
        // `r22_refusal_constant_census.rs` holds this list to the adapter's source, so a seventh
        // cannot be added anywhere in the crate without one of the two going red.
        (
            "DECLARATION_WITHOUT_A_READ_FACE_REFUSAL",
            gx_adapter_mcp::DECLARATION_WITHOUT_A_READ_FACE_REFUSAL,
        ),
    ] {
        let rendered =
            format!("the substrate would not answer for \"stdio://x#doc:d1\": {constant}");
        let classified = gx_cli::wrap::declaration_refusal(&rendered);
        println!("R20_M03_CLASSIFY {name}={classified}");
        assert!(
            classified,
            "🔴 `req/291` M-03: {name} is a refusal caused by a **declaration**, and an agent told \
             `not-reached` about it is told the wiring is broken when the catalogue is wrong"
        );
    }
    // Without this, "classify everything as a refusal" satisfies the loop above.
    let unrelated =
        "the substrate would not answer for \"stdio://x#doc:d1\": connection reset by peer";
    println!(
        "R20_M03_CLASSIFY control={}",
        gx_cli::wrap::declaration_refusal(unrelated)
    );
    assert!(
        !gx_cli::wrap::declaration_refusal(unrelated),
        "a server that dropped the connection is not a declaration this deployment wrote wrong"
    );
}

// ---------------------------------------------------------------------------
// The real road — one of the two constants R18 added, through a live proxy
// ---------------------------------------------------------------------------

/// The fixture server: `doc.write` / `doc.restore` as effects, `doc.get` as the read face, and the
/// read face answers exactly what the environment holds. Written here rather than referenced from a
/// path outside the repository, so this suite runs from a clone.
const SERVER_PY: &str = r#"#!/usr/bin/env python3
import json, os, sys
LOG = os.environ.get("R20_LOG")
ANSWER = os.environ.get("R20_READ_ANSWER", "{}")

def note(line):
    if LOG:
        with open(LOG, "a", encoding="utf-8") as handle:
            handle.write(line + "\n")

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def text_result(text):
    return {"content": [{"type": "text", "text": text}], "isError": False}

def handle(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-11-25"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "r20-server", "version": "0"}}
    if method == "tools/list":
        schema = {"type": "object", "properties": {"uri": {"type": "string"},
                                                   "contents": {"type": "string"},
                                                   "id": {"type": "string"}}}
        return {"tools": [{"name": n, "description": n, "inputSchema": schema}
                          for n in ("doc.write", "doc.restore", "doc.get")]}
    if method == "resources/list":
        return {"resources": []}
    if method == "resources/read":
        uri = params.get("uri", "")
        path = uri[len("file://"):] if uri.startswith("file://") else uri
        with open(path, "r", encoding="utf-8") as handle:
            return {"contents": [{"uri": uri, "mimeType": "text/plain", "text": handle.read()}]}
    if method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}
        if name in ("doc.write", "doc.restore"):
            uri = args.get("uri", "")
            path = uri[len("file://"):] if uri.startswith("file://") else uri
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(args.get("contents", ""))
            return text_result("wrote %d bytes to %s" % (len(args.get("contents", "")), uri))
        if name == "doc.get":
            return text_result(ANSWER)
        raise ValueError("r20-server has no tool %r" % (name,))
    raise ValueError("r20-server has no %r" % (method,))

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    note(line)
    try:
        message = json.loads(line)
    except ValueError:
        continue
    if "id" not in message:
        continue
    try:
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": handle(message.get("method"), message.get("params") or {})})
    except Exception as why:
        send({"jsonrpc": "2.0", "id": message["id"],
              "error": {"code": -32602, "message": str(why)}})
"#;

struct Agent {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    n: u64,
}

impl Agent {
    fn open(args: &[String], home: &Path) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gx"));
        command
            .env("HOME", home)
            .env("USERPROFILE", home)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("the gx binary runs");
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
                "clientInfo": { "name": "r20v", "version": "0" },
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

fn launcher(project: &Path) -> PathBuf {
    let py = project.join("r20-server.py");
    std::fs::write(&py, SERVER_PY).expect("write the fixture server");
    let sh = project.join("r20-server.sh");
    std::fs::write(&sh, format!("#!/bin/sh\nexec python3 {}\n", py.display()))
        .expect("write the launcher");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&sh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    sh
}

/// 🔴 The arm the audit ran as `object_unnamed`: a read face whose answer is not JSON, so nothing
/// could be read out of it and no object was named. gx refuses before the gate; the question is
/// what it **says it did**.
#[test]
fn a_refusal_r18_added_is_recorded_as_a_refusal_on_the_real_road() {
    let pipeline = support::pipeline_named(
        "r20_vocabulary_unnamed",
        "a file this suite does not measure\n",
        "seed.txt",
    );
    let seeded = pipeline.submit("create this project's .gx/ directory");
    assert_eq!(seeded.code, 0, "seed submit: {}", seeded.stderr);

    let doc = pipeline.project.join("doc.txt");
    std::fs::write(&doc, BEFORE).expect("write the document");
    let uri = format!("file://{}", doc.display());
    let log = pipeline.project.join("server.log");
    let server = launcher(&pipeline.project);

    let catalogue = pipeline.project.join("catalogue.json");
    std::fs::write(
        &catalogue,
        serde_json::to_vec_pretty(&json!({
            "doc.write": {
                "restored_by": "doc.restore",
                "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" },
                "read_by": {
                    "by_tool": "doc.get",
                    "arguments": { "id": { "forward": "id" } },
                    "identity": [{ "answer": "/uri" }],
                },
            },
            "doc.restore": {
                "restored_by": "doc.write",
                "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" },
            },
        }))
        .expect("json"),
    )
    .expect("write the catalogue");

    let args: Vec<String> = vec![
        "--project".into(),
        pipeline.project.display().to_string(),
        "wrap".into(),
        "--endpoint".into(),
        ENDPOINT.into(),
        "--actor-key".into(),
        pipeline.key_id.clone(),
        "--actor-model".into(),
        "r20-probe".into(),
        "--restore-catalogue".into(),
        catalogue.display().to_string(),
        "--server-env".into(),
        "R20_READ_ANSWER=this answer is not JSON at all".to_string(),
        "--server-env".into(),
        format!("R20_LOG={}", log.display()),
        "--".into(),
        server.display().to_string(),
    ];
    let mut agent = Agent::open(&args, &pipeline.home);
    let answered = agent.ask(
        "tools/call",
        json!({ "name": "doc.write",
                "arguments": { "uri": uri, "id": "doc:d1",
                               "contents": "the document as the agent wants it\n" } }),
    );
    let stderr = agent.close();

    let meta = &answered["result"]["_meta"];
    let text = answered["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let session = stderr
        .lines()
        .find(|l| l.contains("\"admitted\""))
        .unwrap_or("<no session line>")
        .to_string();
    println!(
        "R20_M03_ROAD verdict={} session={session}",
        meta["gx/verdict"]
    );
    println!(
        "R20_M03_ROAD text={}",
        text.chars().take(320).collect::<String>()
    );

    // The machine was never the defect: the object does not move and the effect is never sent.
    assert_eq!(
        std::fs::read_to_string(&doc).expect("the document is readable"),
        BEFORE,
        "the object must not move — if it did, this is a different finding altogether"
    );
    let calls: Vec<String> = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| {
            v.get("method")
                .and_then(Value::as_str)
                .filter(|m| *m == "tools/call")
                .map(|_| v["params"]["name"].as_str().unwrap_or("?").to_string())
        })
        .collect();
    println!("R20_M03_ROAD tool_calls={calls:?}");
    assert!(
        !calls.iter().any(|c| c == "doc.write"),
        "the effect must never reach the server: {calls:?}"
    );

    // The record is the thing this lane repairs.
    assert_eq!(
        meta["gx/verdict"],
        gx_cli::wrap::REFUSED_BEFORE_VERDICT,
        "🔴 `req/291` M-03: gx stopped at the deployment's own declaration, before the gate. That \
         is a refusal, and R19 gave it a word; `OBJECT_UNNAMED_REFUSAL` was not in the list that \
         earns it"
    );
    assert!(
        !text.contains("not a refusal"),
        "the sentence the agent reads must not deny what happened: {text}"
    );
    assert!(
        session.contains("\"failed\":0"),
        "and the session counter must not read this as a failure of the server: {session}"
    );
}
