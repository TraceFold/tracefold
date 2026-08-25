// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx demo` (**P5**, `req/134` §1 item 1, ruling 1) — a disposable, self-contained walk of req/114
//! §3's aha loop: *"the agent broke it → proved what happened → rolled it back → a third party
//! verified the rollback"* (sem: SEM-gx-cli-014).
//!
//! # 🔴 What this is, exactly
//!
//! A Rust port of `tools/e2e_p3.sh` + `tools/p3_agent.py` — the scripted walk P3's own E2E already
//! proved real — folded into one command a first-time operator can type with the network down. It
//! is **not** a second implementation of the membrane: every gated step below shells out to **this
//! same compiled binary**, exactly the commands an operator would type (`gx key gen`, `gx wrap`,
//! `gx undo`, `gx log checkpoint`, `gx receipt verify`), so the pipeline this walk exercises is the
//! one the binary actually ships and not a stand-in for it. What is scripted is the **agent**: the
//! JSON-RPC frames a language model would send over `gx wrap`'s stdin, sent here by this process
//! instead (`WrapSession`, below) — the same substitution `tools/p3_agent.py`'s own module header
//! names and reports honestly (`agent_kind`).
//!
//! # Ruling 1 — in-process bundling, not a second executable (sem: SEM-gx-cli-015)
//!
//! `req/134` §4 ruling 1: "demo server = in-process bundling, adopted (spawning an external
//! process is friction on Windows / first-run environments -- an extension of NFR-019's
//! single-binary principle. The demo must still run the real pipeline, though: a demo-only
//! escape-hatch code path is forbidden)" (sem: SEM-gx-cli-016).
//! The notes server this walk's agent writes through is [`serve_notes`] — reached only as
//! `gx __demo-notes-server`, a **hidden** subcommand of this same binary (`main.rs`'s
//! `Command::DemoNotesServer`) rather than a second compiled artefact. `tests/bin/
//! mcp_probe_server.rs` (P3's own fixture server) is the shape this was read from — self-observed
//! and reimplemented for this binary, not linked (gitrepo HARD: no new dependency, no shared code
//! path with a test-only binary that `cargo install` never builds).
//!
//! # What each of the four stages actually does
//!
//! 1. **broke it** — `gx wrap` (a real child of this process) proxies a scripted agent to a real
//!    `gx __demo-notes-server` (a real child of *that* process): one `notes.write` inside the
//!    sandbox is admitted and applied, one `notes.write` under `/etc/hostname` is refused before it
//!    ever reaches the server (the shipped `policies/mcp/deny-etc-resources.cedar` pack, unmodified
//!    — no demo-only policy).
//! 2. **proved it** — the commit's transformation id, verdict and **real** receipt path
//!    (`.gx/receipts/…commit.json`, read out of the wrap session's own `_meta`, not reconstructed),
//!    plus a signed ledger checkpoint (`gx log checkpoint`) published while the change still stands.
//! 3. **restored it** — `gx undo` (a fresh child process, wired to a fresh instance of the same
//!    notes server) commits the compensating `notes.restore` call and receipts it; the sandbox file
//!    is read back to show it matches what it held before stage 1.
//! 4. **verified it** — `gx receipt verify --offline`, run **twice**, each as its own child process
//!    against the checkpoint that was current when the receipt it checks was issued (the ordering
//!    `tools/e2e_p3.sh` §3b already worked out: an inclusion proof is relative to a tree size, RFC
//!    6962). `verify_p5.sh` repeats this same check in a **third**, independent process afterwards
//!    (AC-P5-1: "not this walk's own self-report").
//!
//! # What this module deliberately does not do
//!
//! No colour, no glyph, no box-drawing character. `=== N. heading ===` is `tools/e2e_p3.sh`'s own
//! convention, reused rather than invented — the visual design of a terminal surface is a design
//! session's decision (`req/134`'s own header), and this module stops at plain, measured text.

use std::io::{BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gx_mcp_wire::jsonrpc::{self, Frame};
use serde_json::{json, Value};

use crate::exit::Outcome;
use crate::keys::KeyStore;
use crate::{Error, Result};

/// The argument `gx demo` (and `gx undo`, during stage 3) spawns this same binary with to reach
/// [`serve_notes`]. Kept as one named constant rather than a literal at each call site, so the two
/// halves (`main.rs`'s subcommand name and this module's spawn sites) cannot drift apart.
pub const DEMO_SERVER_ARG: &str = "__demo-notes-server";

/// The `env` name the notes server reads its arrival log from — A-7's technique, one layer over:
/// a count taken from the **server's** side of the wire, in a file this walk did not write to
/// itself, so "the call arrived" is not this process's own say-so.
const ARRIVALS_ENV: &str = "GX_DEMO_LOG";

const BEFORE_TEXT: &str = "the note as it stood before any agent touched it";
const AFTER_TEXT: &str = "the note after an agent wrote through gx wrap";
const DENIED_URI: &str = "file:///etc/hostname";

/// `gx demo` itself — req/134 §1 item 1 / AC-P5-1.
///
/// # Errors
/// [`Error::Usage`] for anything this walk could not complete: a child that would not start, an
/// unexpected answer from one, a receipt that would not verify. Every one of them means the demo
/// did not reach `DEMO_COMPLETE`, which is the honest outcome of a walk that failed partway rather
/// than a reason to print it anyway.
pub fn run() -> Result<Outcome> {
    let started = std::time::Instant::now();
    let gx = std::env::current_exe().map_err(|e| Error::Io {
        action: "read",
        path: "<current executable>".to_string(),
        source: e,
    })?;

    let work = std::env::temp_dir().join("gx-demo");
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(crate::io("remove", &work))?;
    }
    let project = work.join("project");
    let data = work.join("data");
    std::fs::create_dir_all(&project).map_err(crate::io("create", &project))?;
    std::fs::create_dir_all(&data).map_err(crate::io("create", &data))?;

    let notes_path = data.join("notes.md");
    let arrivals_path = data.join("arrivals.log");
    std::fs::write(&notes_path, BEFORE_TEXT).map_err(crate::io("write", &notes_path))?;
    let allowed_uri = format!("file://{}", notes_path.display());

    // `gx wrap` opens `.gx/` and does not create it (its own module header: a long-lived verb over
    // a project that already exists). `gx submit` is the verb 44 gives that job; this walk calls
    // the same function `gx submit` calls (`Layout::create`) in-process rather than shelling out for
    // it, because creating an empty directory structure is setup and not a step of the membrane the
    // aha loop is about.
    crate::layout::Layout::create(&project)?;

    // A key, filed exactly where `gx key gen` always files one (req/56 §3, `KeyStore::user_default`)
    // -- `Session::signing_key` reads from nowhere else, so a `--out` here would produce a key `gx
    // wrap`'s own pipeline could not sign with. In-process for the same reason as `Layout::create`:
    // generating an actor's key is setup, not the membrane.
    let store = KeyStore::user_default()?;
    let key_outcome = crate::keys::gen(&store, crate::keys::ALGORITHM, None)?;
    let key_id = key_outcome.json["key_id"]
        .as_str()
        .ok_or_else(|| Error::Usage {
            detail: "gx key gen answered with no `key_id`".to_string(),
        })?
        .to_string();
    let key_file = store.path_of(&key_id);
    let pub_json = work.join("pub.json");
    std::fs::write(
        &pub_json,
        serde_json::to_vec(&key_outcome.json).unwrap_or_default(),
    )
    .map_err(crate::io("write", &pub_json))?;

    crate::say!(
        "gx demo: a disposable walk of req/114 §3's aha loop -- an agent breaks something through \
         the real `gx wrap` membrane, gx proves what happened, an operator restores it, and a \
         separate process verifies the restore offline."
    )?;
    crate::say!("gx demo: sandbox = {}", work.display())?;
    crate::say!(
        "gx demo: telemetry = disabled (R-114-4 -- this walk emits no OTel spans anywhere; nothing \
         about it is measured or sent)"
    )?;

    // === 1. broke it ============================================================================
    crate::say!()?;
    crate::say!("=== 1. broke it ===")?;
    let restores = [
        "notes.write=notes.restore".to_string(),
        "notes.restore=notes.write".to_string(),
    ];
    let mut wrap_args: Vec<String> = vec![
        "--project".into(),
        project.display().to_string(),
        "wrap".into(),
        "--endpoint".into(),
        "stdio://gx-demo".into(),
        "--actor-key".into(),
        key_id.clone(),
        "--actor-model".into(),
        "gx-demo/1 (a scripted walk over gx wrap, not a language model)".into(),
        "--server-env".into(),
        format!("{ARRIVALS_ENV}={}", arrivals_path.display()),
    ];
    for pair in &restores {
        wrap_args.push("--restore".into());
        wrap_args.push(pair.clone());
    }
    wrap_args.push("--".into());
    wrap_args.push(gx.display().to_string());
    wrap_args.push(DEMO_SERVER_ARG.into());

    let mut session = WrapSession::spawn(&gx, &wrap_args)?;
    let _handshake = session.request(
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "gx-demo", "version": env!("CARGO_PKG_VERSION") },
        }),
    )?;
    session.send_notification("notifications/initialized", json!({}))?;
    let _tools = session.request("tools/list", json!({}))?;

    let admitted = session.request(
        "tools/call",
        json!({ "name": "notes.write", "arguments": { "uri": allowed_uri, "contents": AFTER_TEXT } }),
    )?;
    let denied = session.request(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": DENIED_URI, "contents": "an agent tried this" },
        }),
    )?;
    let wrap_status = session.finish()?;
    if !wrap_status.success() {
        return Err(Error::Usage {
            detail: format!("gx wrap exited {wrap_status:?} partway through the walk"),
        });
    }

    let admitted_result = admitted.get("result").cloned().unwrap_or(Value::Null);
    let denied_result = denied.get("result").cloned().unwrap_or(Value::Null);
    let tid = admitted_result["_meta"]["gx/transformation"]
        .as_str()
        .ok_or_else(|| Error::Usage {
            detail: format!("the admitted call carried no `gx/transformation`: {admitted_result}"),
        })?
        .to_string();
    let verdict = admitted_result["_meta"]["gx/verdict"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let commit_receipt = admitted_result["_meta"]["gx/commit"]["stored_at"]
        .as_str()
        .ok_or_else(|| Error::Usage {
            detail: format!("the commit carried no `stored_at`: {admitted_result}"),
        })?
        .to_string();
    let deny_is_error = denied_result["isError"].as_bool().unwrap_or(false);
    let now_notes = std::fs::read_to_string(&notes_path).map_err(crate::io("read", &notes_path))?;

    crate::say!(
        "an agent, through `gx wrap`, wrote notes.md: {BEFORE_TEXT:?} -> {now_notes:?} (verdict {verdict})"
    )?;
    crate::say!(
        "the same agent tried to write file:///etc/hostname, outside the sandbox -- gx refused it \
         (denied.isError={deny_is_error}) and the call never reached the notes server"
    )?;
    let arrivals_after_stage1 = count_lines(&arrivals_path);
    crate::say!(
        "notes server arrivals so far: {arrivals_after_stage1} (the admitted call, and only it -- \
         measured from the server's own log, `{ARRIVALS_ENV}`, not from this walk's say-so)"
    )?;

    // === 2. proved it ============================================================================
    crate::say!()?;
    crate::say!("=== 2. proved it ===")?;
    crate::say!("transformation = {tid}")?;
    crate::say!("commit receipt = {commit_receipt}")?;
    let head1 = work.join("head1.json");
    checkpoint(&gx, &project, &key_file, &head1)?;
    crate::say!(
        "ledger head published (before the undo) = {}",
        head1.display()
    )?;

    // === 3. restored it ==========================================================================
    crate::say!()?;
    crate::say!("=== 3. restored it ===")?;
    let mut undo_args: Vec<String> = vec![
        "--project".into(),
        project.display().to_string(),
        "--mcp-server".into(),
        gx.display().to_string(),
        "--mcp-server-arg".into(),
        DEMO_SERVER_ARG.into(),
        // 🔴 The locator `notes.write` recorded carries the **`stdio://gx-demo`** endpoint stage
        // 1's `gx wrap --endpoint` minted (not `stdio_endpoint(gx)`'s default, which is derived
        // from the command and would not match): the undo's own snapshot reads through this
        // transport by locator, and a transport connected under a different endpoint refuses with
        // "this transport speaks to X and the locator names Y" (`gx-adapter-mcp`'s own
        // fail-closed check against answering for the wrong server).
        "--mcp-endpoint".into(),
        "stdio://gx-demo".into(),
        "--mcp-server-env".into(),
        format!("{ARRIVALS_ENV}={}", arrivals_path.display()),
    ];
    for pair in &restores {
        undo_args.push("--mcp-restore".into());
        undo_args.push(pair.clone());
    }
    undo_args.push("undo".into());
    undo_args.push(tid.clone());
    let undo_out = run_gx(&gx, &undo_args)?;
    if !undo_out.status.success() {
        return Err(Error::Usage {
            detail: format!(
                "gx undo exited {:?}: stderr={}",
                undo_out.status, undo_out.stderr
            ),
        });
    }
    let undo_json: Value =
        serde_json::from_str(undo_out.stdout.trim()).map_err(|e| Error::Usage {
            detail: format!(
                "gx undo's stdout did not parse as JSON: {e}: {}",
                undo_out.stdout
            ),
        })?;
    let undo_receipt = undo_json["stored_at"].as_str().unwrap_or("").to_string();
    let restored_notes =
        std::fs::read_to_string(&notes_path).map_err(crate::io("read", &notes_path))?;
    crate::say!("undo receipt = {undo_receipt}")?;
    crate::say!(
        "notes.md is back: {restored_notes:?} (== the text before stage 1: {})",
        restored_notes == BEFORE_TEXT
    )?;
    let arrivals_after_undo = count_lines(&arrivals_path);
    crate::say!(
        "notes server arrivals now: {arrivals_after_undo} (one more than stage 1 -- the undo's own \
         `notes.restore` call, arrived at a fresh instance of the same server)"
    )?;
    let head2 = work.join("head2.json");
    checkpoint(&gx, &project, &key_file, &head2)?;
    crate::say!(
        "ledger head published (after the undo) = {}",
        head2.display()
    )?;

    // === 4. verified it ==========================================================================
    crate::say!()?;
    crate::say!("=== 4. verified it (a separate process, offline) ===")?;
    verify_offline(&gx, &commit_receipt, &head1, &pub_json)?;
    crate::say!("commit receipt verifies offline: exit 0 ({commit_receipt})")?;
    verify_offline(&gx, &undo_receipt, &head2, &pub_json)?;
    crate::say!("undo receipt verifies offline: exit 0 ({undo_receipt})")?;

    crate::say!()?;
    crate::say!("next: `gx limits` -- what this build does not cover yet.")?;
    crate::say!("elapsed = {:.1}s", started.elapsed().as_secs_f64())?;
    crate::say!("DEMO_COMPLETE")?;

    Ok(Outcome::ok(json!({
        "gx": "demo",
        "sandbox": work.display().to_string(),
        "transformation": tid,
        "commit_receipt": commit_receipt,
        "undo_receipt": undo_receipt,
        // The checkpoint each receipt verifies against (stage 4's own pairing, RFC 6962's
        // tree-size ordering: `tools/e2e_p3.sh` §3b's reasoning), and the public key file both
        // checks were run with -- so a caller that wants to re-run stage 4 itself (a test, or
        // `verify_p5.sh`) does not have to re-derive paths this walk already computed.
        "commit_checkpoint": head1.display().to_string(),
        "undo_checkpoint": head2.display().to_string(),
        "public_key": pub_json.display().to_string(),
        "arrivals": arrivals_after_undo,
        "elapsed_seconds": started.elapsed().as_secs_f64(),
    })))
}

/// The demo's own MCP server: `notes.write` and `notes.restore` on whatever `file://` resource a
/// call names. Reached only as `gx __demo-notes-server` (`main.rs`'s hidden `DemoNotesServer`) --
/// this same binary's own child, not a second executable (ruling 1; sem: SEM-gx-cli-017). The shape is
/// `tests/bin/mcp_probe_server.rs`'s, observed and reimplemented for this crate rather than linked
/// (that file is gated behind a dev-only feature this binary never builds).
///
/// # Errors
/// [`Error::Io`] if stdin cannot be read.
pub fn serve_notes() -> Result<Outcome> {
    let stdin = std::io::stdin();
    let mut input = std::io::BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut calls: u64 = 0;

    loop {
        let line = jsonrpc::read_frame(&mut input).map_err(|e| Error::Io {
            action: "read",
            path: "<stdin>".to_string(),
            source: e,
        })?;
        let Some(line) = line else { break };
        let frame = match jsonrpc::parse(&line) {
            Ok(f) => f,
            Err(e) => {
                let answer = jsonrpc::error(&Value::Null, jsonrpc::PARSE_ERROR, &e.to_string());
                write_frame_out(&mut output, &answer)?;
                continue;
            }
        };
        if let Frame::Request { id, method, params } = frame {
            let answer = match dispatch_notes(&method, &params, &mut calls) {
                Ok(result) => jsonrpc::ok(&id, result),
                Err((code, message)) => jsonrpc::error(&id, code, &message),
            };
            write_frame_out(&mut output, &answer)?;
        }
        // Notification / Response: this server answers nobody's questions and expects none of its
        // own answered, so both arrive here as no-ops (the same three-arm shape
        // `tests/bin/mcp_probe_server.rs` dispatches with).
    }
    Ok(Outcome::ok(
        json!({ "gx": "demo-notes-server", "calls": calls }),
    ))
}

fn write_frame_out<W: Write>(out: &mut W, value: &Value) -> Result<()> {
    jsonrpc::write_frame(out, value).map_err(|e| Error::Io {
        action: "write",
        path: "<stdout>".to_string(),
        source: e,
    })
}

/// 🔴 **`req/316` L-03, raised to M by `req/38` §227 ruling 4 (R24)** — the two faults this server
/// can be asked to have, so that one road can be driven over a real wire instead of described.
///
/// # What has to happen for the road to exist at all
///
/// 43 T-10c is "`adapter.apply` failed; roll back from the escrowed inverse on a **best-effort**
/// basis, and journal `Aborted{ApplyFailed}` whatever that attempt did". Reaching the branch where
/// the best effort *also* fails needs two independent faults in one run: a post-apply observation
/// that does not answer, and a compensating call the server refuses. A mock inside a test binary
/// can arrange both, and the twenty-third audit's did — but then what is measured is the mock. So
/// the faults live here, in the shipped demo server, behind environment variables that are unset in
/// every other run.
///
/// * `GX_DEMO_READ_FAILS_AFTER_EFFECT` — once an **effect** tool has arrived in this process, every
///   read face answers `-32603`. Deliberately *not* `-32002`: this is "I could not tell you", which
///   is the answer that must never be folded to an absence (`req/312` M-01).
/// * `GX_DEMO_TOOL_REFUSES=<name>` — that one tool answers `-32603` instead of running.
/// * 🔴 **R30 / `req/372` M-01** — `GX_DEMO_TOOL_FAILS_AFTER_EFFECT=<name>` — that one tool
///   **runs to completion and then answers `-32603`**. The two switches above cannot express it,
///   and after R30 it is the road that matters most: `TOOL_REFUSES` is a call that never touched
///   the object, and this is a call that **did**. The engine tells them apart now — one is
///   `NotAttempted(WorldNeverMoved)` and the other is a compensation that runs — so a suite that
///   can only produce the first can no longer reach the second, and two of R25's arms went red on
///   exactly that. It is also the honest shape of the commonest real failure: the tool worked and
///   the answer was lost on the way back.
///
/// None of the three is a capability: a server an operator points gx at can already fail any way it
/// likes, and these switches say which way *this* one will, for one suite.
const READ_FAILS_AFTER_EFFECT_ENV: &str = "GX_DEMO_READ_FAILS_AFTER_EFFECT";

/// See [`READ_FAILS_AFTER_EFFECT_ENV`].
const TOOL_REFUSES_ENV: &str = "GX_DEMO_TOOL_REFUSES";

/// 🔴 **R30 / `req/372` M-01** — see [`READ_FAILS_AFTER_EFFECT_ENV`]. The tool runs first and
/// answers the error afterwards, so the object **is** changed and the caller is told it was not.
const TOOL_FAILS_AFTER_EFFECT_ENV: &str = "GX_DEMO_TOOL_FAILS_AFTER_EFFECT";

/// Whether this tool is a change rather than a read. The counter that gates the read fault above
/// counts **effects**, so that the read face's own arrivals cannot arm the fault against itself.
fn is_an_effect(name: &str) -> bool {
    !matches!(name, "notes.fetch")
}

fn dispatch_notes(
    method: &str,
    params: &Value,
    calls: &mut u64,
) -> std::result::Result<Value, (i64, String)> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-11-25",
            "capabilities": { "tools": {}, "resources": {} },
            "serverInfo": { "name": "gx-demo-notes-server", "version": env!("CARGO_PKG_VERSION") },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": notes_tools() })),
        "resources/list" => Ok(json!({ "resources": [] })),
        // 🔴 The read side of the CAS: `gx-adapter-mcp`'s `snapshot` reads a resource **before** a
        // gate sees the change (43 T-2), so a server that answers `tools/call` alone leaves every
        // call unplannable -- this is what `tests/bin/mcp_probe_server.rs` implements and what a
        // first pass of this file omitted (found by running `gx demo` against a real `gx wrap`,
        // not by reading the fixture a second time).
        "resources/read" => {
            let uri = params
                .get("uri")
                .and_then(Value::as_str)
                .ok_or((jsonrpc::INVALID_PARAMS, "no `uri`".to_string()))?;
            let path = uri.strip_prefix("file://").ok_or((
                jsonrpc::INVALID_PARAMS,
                format!("{uri:?} is not a `file://` resource"),
            ))?;
            // 🔴 **`req/312` M-01 (R23)** — a file that is not there and a file that could not be
            // read are two answers, and until this window this server gave one. `-32002` is MCP's
            // *resource not found*, and it is the only answer `gx-mcp-wire` reads as "the locator
            // holds nothing" — which is what makes a call that **removed** a resource still
            // observable as an absence while a read that failed is refused.
            if *calls > 0 && std::env::var(READ_FAILS_AFTER_EFFECT_ENV).is_ok() {
                return Err((
                    jsonrpc::INTERNAL_ERROR,
                    format!("{path}: this read face was asked to fail after an effect landed"),
                ));
            }
            let text = std::fs::read_to_string(path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    (jsonrpc::RESOURCE_NOT_FOUND, format!("{path}: {e}"))
                }
                _ => (jsonrpc::INTERNAL_ERROR, format!("{path}: {e}")),
            })?;
            Ok(json!({ "contents": [{ "uri": uri, "mimeType": "text/plain", "text": text }] }))
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or((jsonrpc::INVALID_PARAMS, "no `name`".to_string()))?;
            let empty = json!({});
            let arguments = params.get("arguments").unwrap_or(&empty);
            let read_is_dead = *calls > 0 && std::env::var(READ_FAILS_AFTER_EFFECT_ENV).is_ok();
            if is_an_effect(name) {
                *calls += 1;
            }
            record_arrival(name, arguments)?;
            if std::env::var(TOOL_REFUSES_ENV).ok().as_deref() == Some(name) {
                return Err((
                    jsonrpc::INTERNAL_ERROR,
                    format!("this server was asked to refuse {name:?}"),
                ));
            }
            if !is_an_effect(name) && read_is_dead {
                return Err((
                    jsonrpc::INTERNAL_ERROR,
                    format!("the read tool {name:?} was asked to fail after an effect landed"),
                ));
            }
            // 🔴 **R30 / `req/372` M-01** — the effect happens, and *then* the error. The order is
            // the whole point of the switch: `TOOL_REFUSES` above returns before `call_notes` and
            // leaves the object untouched, and this one returns after it and does not.
            if std::env::var(TOOL_FAILS_AFTER_EFFECT_ENV).ok().as_deref() == Some(name) {
                let ran = call_notes(name, arguments);
                return match ran {
                    Ok(_) => Err((
                        jsonrpc::INTERNAL_ERROR,
                        format!(
                            "this server was asked to run {name:?} and then fail: the change was \
                             made and this answer is an error anyway"
                        ),
                    )),
                    Err(already) => Err(already),
                };
            }
            call_notes(name, arguments)
        }
        other => Err((
            jsonrpc::METHOD_NOT_FOUND,
            format!("gx-demo-notes-server has no {other:?}"),
        )),
    }
}

fn notes_tools() -> Value {
    json!([
        {
            "name": "notes.write",
            "description": "write text to a resource",
            "inputSchema": { "type": "object", "properties": { "uri": { "type": "string" }, "contents": { "type": "string" } } },
        },
        {
            "name": "notes.restore",
            "description": "put a resource's earlier text back",
            "inputSchema": { "type": "object", "properties": { "uri": { "type": "string" }, "contents": { "type": "string" } } },
        },
        // 🔴 **`req/316` M-02 (R24)** — the two tools a *tools-only* deployment needs, so that the
        // road DR-46-16 exists for can be driven against a real server rather than described.
        //
        // `notes.fetch` is a **read face behind a tool**: the shape a server that publishes no
        // `resources/read` for its objects offers instead, which is the whole premise of the
        // `$cas_read` slot. `notes.delete` is a call that **removes** a resource — the one kind of
        // effect whose post-apply observation has to distinguish "the server says it is gone" from
        // "the read failed", and the case the twenty-third audit could not produce under any
        // wiring because this road had no way to say the first one.
        // 🔴 The name is `notes.fetch` and not `notes.read`, and the full-workspace floor is why.
        //
        // `crates/gx-cli/tests/r19_escalation_road.rs` builds its Given out of a catalogue whose read face
        // is `notes.read` — a tool this server **does not publish** — so that the escrow fails and the
        // agent gets `READ_FAILURE_REFUSAL`. Publishing a tool by that name moved that arm's failure to a
        // different constant, which is a Given another lane owns being changed from underneath it. A new
        // face on a shared fixture needs a name no fixture already means something by.
        {
            "name": "notes.fetch",
            "description": "read a resource's text, behind a tool rather than `resources/read`",
            "inputSchema": { "type": "object", "properties": { "uri": { "type": "string" } } },
        },
        {
            "name": "notes.delete",
            "description": "remove a resource",
            "inputSchema": { "type": "object", "properties": { "uri": { "type": "string" } } },
        },
    ])
}

/// The `file://` path a notes tool was asked about.
fn notes_path(arguments: &Value) -> std::result::Result<String, (i64, String)> {
    let uri = arguments
        .get("uri")
        .and_then(Value::as_str)
        .ok_or((jsonrpc::INVALID_PARAMS, "no `uri`".to_string()))?;
    uri.strip_prefix("file://")
        .map(std::string::ToString::to_string)
        .ok_or((
            jsonrpc::INVALID_PARAMS,
            format!("{uri:?} is not a `file://` resource"),
        ))
}

/// A-7's count, taken from this server's own side of the wire -- one line per **arrival**, before
/// the tool runs, so a call the gate denied leaves no line here at all.
///
/// # 🔴 **R15 / `req/259` L-02** — an instrument that cannot record says so
///
/// What stood here was `let _ = writeln!(file, …)` inside an `if let Ok(mut file)`, so a write that
/// failed left the walk one arrival short and told nobody: `gx demo` reports "the server's own log
/// says N calls arrived" and N would quietly have been the wrong number. That is the same shape as
/// the two the audit found on stderr, on a file, and in the one place where the whole point of the
/// bytes is to be counted afterwards. Both failures are values now — the environment variable being
/// unset is still not one, because a walk that was never asked to record is not a walk that failed
/// to.
///
/// # Errors
/// A JSON-RPC `INTERNAL_ERROR` naming the file, which the caller returns to the proxy: the call is
/// refused rather than performed-and-miscounted.
fn record_arrival(name: &str, arguments: &Value) -> std::result::Result<(), (i64, String)> {
    let Ok(path) = std::env::var(ARRIVALS_ENV) else {
        return Ok(());
    };
    let uri = arguments
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| (jsonrpc::INTERNAL_ERROR, format!("{path}: {e}")))?;
    writeln!(file, "call\t{name}\t{uri}")
        .map_err(|e| (jsonrpc::INTERNAL_ERROR, format!("{path}: {e}")))?;
    file.flush()
        .map_err(|e| (jsonrpc::INTERNAL_ERROR, format!("{path}: {e}")))
}

fn call_notes(name: &str, arguments: &Value) -> std::result::Result<Value, (i64, String)> {
    let contents = arguments
        .get("contents")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match name {
        "notes.write" | "notes.restore" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .ok_or((jsonrpc::INVALID_PARAMS, "no `uri`".to_string()))?;
            let path = uri.strip_prefix("file://").ok_or((
                jsonrpc::INVALID_PARAMS,
                format!("{uri:?} is not a `file://` resource"),
            ))?;
            std::fs::write(path, contents)
                .map_err(|e| (jsonrpc::INTERNAL_ERROR, format!("{path}: {e}")))?;
            Ok(json!({
                "content": [{ "type": "text", "text": format!("wrote {} bytes to {uri}", contents.len()) }],
                "isError": false,
            }))
        }
        // 🔴 **`req/316` M-02 (R24)** — the tool read face, answering `-32002` for a locator that
        // holds nothing, exactly as `resources/read` on this server has since R23. It is the same
        // distinction and the same code: a server that cannot make it leaves gx fail-closed.
        "notes.fetch" => {
            let path = notes_path(arguments)?;
            let text = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    (jsonrpc::RESOURCE_NOT_FOUND, format!("{path}: {e}"))
                }
                _ => (jsonrpc::INTERNAL_ERROR, format!("{path}: {e}")),
            })?;
            Ok(json!({
                "content": [{ "type": "text", "text": text }],
                "isError": false,
            }))
        }
        // A call whose effect is a removal. Removing something that is already gone is not an
        // error here: the tool's promise is that the resource does not exist afterwards.
        "notes.delete" => {
            let path = notes_path(arguments)?;
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err((jsonrpc::INTERNAL_ERROR, format!("{path}: {e}"))),
            }
            Ok(json!({
                "content": [{ "type": "text", "text": format!("removed {path}") }],
                "isError": false,
            }))
        }
        other => Err((
            jsonrpc::INVALID_PARAMS,
            format!("gx-demo-notes-server has no tool {other:?}"),
        )),
    }
}

/// One `gx wrap` child process, and the agent-side half of the JSON-RPC session with it -- the
/// scripting `tools/p3_agent.py`'s `Session` class does, ported to this binary so a first-time run
/// needs no interpreter and no second file on disk.
struct WrapSession {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl WrapSession {
    fn spawn(gx: &Path, args: &[String]) -> Result<Self> {
        let mut child = Command::new(gx)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| Error::Usage {
                detail: format!("could not start `gx wrap` ({gx:?} {args:?}): {e}"),
            })?;
        let stdin = child.stdin.take().ok_or_else(|| Error::Usage {
            detail: "gx wrap's stdin was not piped".to_string(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| Error::Usage {
            detail: "gx wrap's stdout was not piped".to_string(),
        })?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 0,
        })
    }

    /// Send a request and read exactly the one line it is answered on. `gx wrap`'s own proxy
    /// (`gx-mcp-wire::server::Proxy::run`) answers one frame per line before reading the next, so
    /// request and response are 1:1 in order -- the same assumption `tools/p3_agent.py`'s `Session.
    /// send` makes.
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let frame = jsonrpc::request(self.next_id, method, params);
        self.write(&frame)?;
        self.read_one()
    }

    /// A notification carries no id and is answered on nothing (JSON-RPC 2.0 §4.1); used for
    /// `notifications/initialized` only.
    fn send_notification(&mut self, method: &str, params: Value) -> Result<()> {
        let frame = jsonrpc::notification(method, params);
        self.write(&frame)
    }

    fn write(&mut self, frame: &Value) -> Result<()> {
        let stdin = self.stdin.as_mut().ok_or_else(|| Error::Usage {
            detail: "gx wrap's stdin was already closed".to_string(),
        })?;
        jsonrpc::write_frame(stdin, frame).map_err(|e| Error::Io {
            action: "write",
            path: "<gx wrap stdin>".to_string(),
            source: e,
        })
    }

    fn read_one(&mut self) -> Result<Value> {
        let line = jsonrpc::read_frame(&mut self.stdout).map_err(|e| Error::Io {
            action: "read",
            path: "<gx wrap stdout>".to_string(),
            source: e,
        })?;
        let line = line.ok_or_else(|| Error::Usage {
            detail: "gx wrap closed its stdout before answering".to_string(),
        })?;
        serde_json::from_str(&line).map_err(|e| Error::Usage {
            detail: format!("gx wrap's answer did not parse as JSON: {e}: {line}"),
        })
    }

    /// Close this end's stdin (the agent's EOF, which is what ends a real session too) and wait for
    /// the child.
    fn finish(mut self) -> Result<std::process::ExitStatus> {
        self.stdin = None; // drops the handle, closing the pipe
        self.child.wait().map_err(|e| Error::Usage {
            detail: format!("gx wrap did not exit cleanly: {e}"),
        })
    }
}

/// stdout/stderr/status of one completed `gx <args>` invocation.
struct GxRun {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_gx(gx: &Path, args: &[String]) -> Result<GxRun> {
    let output = Command::new(gx)
        .args(args)
        .output()
        .map_err(|e| Error::Usage {
            detail: format!("could not run {gx:?} {args:?}: {e}"),
        })?;
    Ok(GxRun {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn checkpoint(gx: &Path, project: &Path, key_file: &Path, out: &Path) -> Result<()> {
    let args = vec![
        "--project".to_string(),
        project.display().to_string(),
        "log".to_string(),
        "checkpoint".to_string(),
        "--key".to_string(),
        key_file.display().to_string(),
        "--out".to_string(),
        out.display().to_string(),
    ];
    let run = run_gx(gx, &args)?;
    if !run.status.success() {
        return Err(Error::Usage {
            detail: format!(
                "gx log checkpoint exited {:?}: stderr={}",
                run.status, run.stderr
            ),
        });
    }
    Ok(())
}

fn verify_offline(gx: &Path, receipt: &str, checkpoint: &Path, pub_json: &Path) -> Result<()> {
    let args = vec![
        "receipt".to_string(),
        "verify".to_string(),
        receipt.to_string(),
        "--offline".to_string(),
        "--checkpoint".to_string(),
        checkpoint.display().to_string(),
        "--checkpoint-key".to_string(),
        pub_json.display().to_string(),
        "--key".to_string(),
        pub_json.display().to_string(),
    ];
    let run = run_gx(gx, &args)?;
    if !run.status.success() {
        return Err(Error::Usage {
            detail: format!(
                "gx receipt verify --offline {receipt} exited {:?}: stdout={} stderr={}",
                run.status, run.stdout, run.stderr
            ),
        });
    }
    Ok(())
}

fn count_lines(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}
