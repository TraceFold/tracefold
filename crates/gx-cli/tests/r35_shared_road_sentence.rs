// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/470` H-01, red-first** (`req/38` §264 ruling 3 item 1; reqdef `req/471` §0-1/§0-3).
//!
//! # The claim this suite exists to make true
//!
//! `docs/LIMITS.md` v0.5-t says, verbatim: "**What v0.5-t changes is therefore the telling, and it
//! changes all of it.** Every row that walks that road now prints a sentence saying the delta was
//! applied and that whether anything had written to the object since the crash was **not**
//! checked".
//!
//! Audit 34 measured the denominator: **five** shipped verbs walk 43 §7-3c's road — `gx verify`,
//! `gx commit`, `gx undo`, `gx repair --yes` and `gx serve` — and exactly **one** of them printed
//! the sentence. The other four answered `rc 0` / `rc 1` / `rc 3` with a third party's bytes
//! replaced and **0 bytes on stderr**. A sixth road, `gx wrap`, inherits two of them
//! (`wrap.rs:547` -> `pipeline::verify`, `wrap.rs:644` -> `pipeline::commit`) and had not been
//! driven by three consecutive audits.
//!
//! # Red-first (`req/38` §226)
//!
//! **No symbol this lane creates is named anywhere in this file.** Every arm drives the shipped
//! `gx` binary and reads bytes that binary produced, so the suite compiles at the commit that
//! precedes the repair and fails on its assertions. That is the point: the red run is the evidence
//! that the bed can fail, and a bed that cannot fail is not evidence of anything.
//!
//! # Why every arm carries a read-only snapshot on each side
//!
//! Audit 34 §7-3 is the reason, and it is worth repeating because it nearly inverted that audit's
//! conclusion. A verb that leaves the world alone cannot be told from a verb that was never put on
//! the road at all. `gx repair --json` **without** `--yes` recovers nothing (`repair.rs:778`
//! filters the call on `writing`), so it is the instrument that says whether the crashed row was
//! still open when the verb ran. Without it, `rc=1` reads as "this road is safe" when it means
//! "this bed missed the road".
//!
//! `cfg(unix)` for the `chmod` on the `gx wrap` launcher script, as every sibling suite says.

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
use support::{pipeline, run, Pipeline, Run};

/// A fragment of the closed-row sentence: the clause that says the delta was written.
const NOTE_MARK: &str = "applying its delta";
/// A fragment of the aborted-row sentence.
const ABORT_MARK: &str = "possibly changed and certainly";
/// The clause that carries the half a counter cannot carry: what was not compared.
const NOT_CHECKED_MARK: &str = "was **not** checked";
/// The clause that carries the other half: that this run may have destroyed somebody's bytes.
const OVERWROTE_MARK: &str = "written over it and cannot tell you so";

// ---------------------------------------------------------------------------
// Byte surgery - copied in shape from `a34_silent_roads.rs`, which took it from
// `a33_shipping_verbs.rs`, so all three lanes cut the same journal in the same place.
// ---------------------------------------------------------------------------

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

fn ledger_frames(bytes: &[u8]) -> Vec<(usize, usize)> {
    const CEILING: u32 = 1 << 20;
    let mut at = 0usize;
    let mut out = Vec::new();
    while at + 4 <= bytes.len() {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > CEILING as usize || at + 4 + length > bytes.len() {
            break;
        }
        out.push((at, 4 + length));
        at += 4 + length;
    }
    out
}

fn truncate_at(path: &Path, at: u64) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
}

fn layout_of(fixture: &Pipeline) -> gx_cli::layout::Layout {
    gx_cli::layout::Layout::open(&fixture.project).expect("the project is open")
}

fn receipt_files(fixture: &Pipeline) -> Vec<PathBuf> {
    let dir = fixture.project.join(".gx").join("receipts");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Two commits, then the journal cut back to just after the second's `ApplyStarted`, the head put
/// back, and the second commit's receipts removed.
///
/// `drop_the_leaf == true` is 43 s7-3c (the road that applies); `false` is s7-3b (the road R33
/// made read). Returns the **first** commit's id, which is the one `gx undo` names.
///
/// 🔴 Byte-for-byte the construction `a34_silent_roads.rs` used, deliberately: if this lane cut a
/// different world it would be repairing a road the audit did not measure, and the before/after
/// comparison in `req/472` would be between two different beds. The first draft of this function
/// was a paraphrase from memory and it did not even find the journal — `Layout` names these three
/// paths and the paraphrase guessed them.
fn a_project_cut_inside_the_window(fixture: &Pipeline, drop_the_leaf: bool) -> String {
    let first = fixture.commit_one("one\n");
    let l0 = layout_of(fixture);
    let head_before = std::fs::read(l0.head_path()).ok();
    let receipts_before = receipt_files(fixture);
    fixture.commit_one("two\n");
    for p in receipt_files(fixture) {
        if !receipts_before.contains(&p) {
            let _ = std::fs::remove_file(&p);
        }
    }
    match &head_before {
        Some(bytes) => {
            std::fs::write(l0.head_path(), bytes).expect("put the head back");
        }
        None => {
            let _ = std::fs::remove_file(l0.head_path());
        }
    }

    let l = layout_of(fixture);
    let journal_path = l.journal_path();
    let ledger_path = l.ledger_path();
    let journal = std::fs::read(&journal_path).expect("read the journal");
    let kinds: Vec<&'static str> = gx_engine::replay(&journal)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    let spans = frames(&journal);
    assert_eq!(
        spans.len(),
        kinds.len(),
        "instrument: one frame per record ({} vs {})",
        spans.len(),
        kinds.len()
    );
    let last_apply = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "ApplyStarted")
        .map(|(i, _)| i)
        .next_back()
        .expect("instrument: the second commit announced its apply");
    truncate_at(
        &journal_path,
        (spans[last_apply].0 + spans[last_apply].1) as u64,
    );

    if drop_the_leaf {
        let ledger = std::fs::read(&ledger_path).expect("read the ledger");
        let leaves = ledger_frames(&ledger);
        let at = leaves.get(1).map_or(0, |leaf| leaf.0 as u64);
        truncate_at(&ledger_path, at);
    }
    first
}

// ---------------------------------------------------------------------------
// The read-only snapshot that says whether the road is still there
// ---------------------------------------------------------------------------

/// `gx repair --json` **without** `--yes`: reads, recovers nothing. Audit 34 §7-3's instrument.
fn snapshot(fixture: &Pipeline, bed: &str, when: &str) -> Value {
    let r = run(fixture.gx().args(["repair", "--json"]));
    let j: Value = serde_json::from_str(r.stdout.trim()).unwrap_or(Value::Null);
    println!(
        "A35_SNAP bed={bed} when={when} rc={} journal_commits={} ledger_leaves={} world={:?}",
        r.code,
        j["journal_commits"],
        j["ledger_leaves"],
        fixture.target_contents()
    );
    j
}

struct Walk {
    bed: String,
    verb: &'static str,
    code: i32,
    world_before: String,
    world_after: String,
    stdout: String,
    stderr: String,
}

impl Walk {
    fn moved(&self) -> bool {
        self.world_before != self.world_after
    }

    /// Did anything this run printed carry the sentence, on either stream?
    fn note_printed(&self) -> bool {
        let all = format!("{}{}", self.stdout, self.stderr);
        all.contains(NOTE_MARK) || all.contains(ABORT_MARK)
    }

    /// 🔴 The stronger question, and the one `req/471` §0-1 makes the acceptance condition: is the
    /// sentence on **stderr**, where a human reads it? A JSON field is not an answer to this — the
    /// audit's own strongest counter-argument (§1-6 item 3) was that `gx repair` already emits
    /// `apply_was_announced: 1`, and the reply was that a counter says a road was walked while the
    /// sentence says *what was not compared* and *that this run may have destroyed something*.
    fn sentence_on_stderr(&self) -> bool {
        self.stderr.contains(NOTE_MARK) || self.stderr.contains(ABORT_MARK)
    }

    fn report(&self) {
        println!(
            "A35_WALK bed={} verb={} rc={} world_before={:?} world_after={:?} moved={} note_printed={} on_stderr={} stdout_bytes={} stderr_bytes={}",
            self.bed, self.verb, self.code, self.world_before, self.world_after,
            self.moved(), self.note_printed(), self.sentence_on_stderr(),
            self.stdout.len(), self.stderr.len()
        );
        println!(
            "A35_RAW_STDOUT bed={} verb={} <<{}>>",
            self.bed, self.verb, self.stdout
        );
        println!(
            "A35_RAW_STDERR bed={} verb={} <<{}>>",
            self.bed, self.verb, self.stderr
        );
    }
}

fn walk(fixture: &Pipeline, bed: &str, verb: &'static str, r: Run, before: String) -> Walk {
    let w = Walk {
        bed: bed.to_string(),
        verb,
        code: r.code,
        world_before: before,
        world_after: fixture.target_contents(),
        stdout: r.stdout,
        stderr: r.stderr,
    };
    w.report();
    w
}

fn third_party_then<F>(
    fixture: &Pipeline,
    bed: &str,
    verb: &'static str,
    third_party: bool,
    f: F,
) -> Walk
where
    F: FnOnce(&Pipeline) -> Run,
{
    snapshot(fixture, bed, "before");
    if third_party {
        std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");
    }
    let before = fixture.target_contents();
    let w = walk(fixture, bed, verb, f(fixture), before);
    snapshot(fixture, bed, "after");
    w
}

/// The four beds audit 34 drove, rebuilt here so the repair is measured on the same world.
///
/// Returns `(verb label, expected verb prefix, the walk)`.
fn the_four_roads() -> Vec<(&'static str, &'static str, Walk)> {
    let mut out: Vec<(&'static str, &'static str, Walk)> = Vec::new();

    // `gx repair --yes --json` - repair.rs:778.
    let r = pipeline("r35_repair_3c", "before\n");
    a_project_cut_inside_the_window(&r, true);
    let repair = third_party_then(
        &r,
        "repair_3c_third_party",
        "gx repair --yes --json",
        true,
        |f| {
            run(f
                .gx()
                .args(["repair", "--json", "--yes"])
                .args(["--signing-key", &f.key_id]))
        },
    );
    out.push(("gx repair", "gx repair:", repair));

    // `gx undo` - lifecycle.rs:197. A verb that refuses can still have moved the world first.
    let u = pipeline("r35_undo_3c", "before\n");
    let first = a_project_cut_inside_the_window(&u, true);
    let undo = third_party_then(&u, "undo_3c_third_party", "gx undo", true, |f| {
        run(f.gx().args(["undo", &first, "--settle", "1"]))
    });
    out.push(("gx undo", "gx undo:", undo));

    // `gx verify` - pipeline.rs:253. Planned **after** the third party writes, because otherwise
    // the stale `Fingerprint0` refuses before `recover` is reached (audit 34 §7-3).
    let v = pipeline("r35_verify_3c_replanned", "before\n");
    a_project_cut_inside_the_window(&v, true);
    std::fs::write(&v.target, "THIRD PARTY\n").expect("a third party writes");
    let vid = v.planned_one("three\n");
    let verify = third_party_then(&v, "verify_3c_replanned", "gx verify", false, |f| {
        run(f.gx().args(["verify", &vid]))
    });
    out.push(("gx verify", "gx verify:", verify));

    // `gx commit` - pipeline.rs:373.
    let c = pipeline("r35_commit_3c_replanned", "before\n");
    a_project_cut_inside_the_window(&c, true);
    std::fs::write(&c.target, "THIRD PARTY\n").expect("a third party writes");
    let cid = c.planned_one("three\n");
    let commit = third_party_then(&c, "commit_3c_replanned", "gx commit", false, |f| {
        run(f.gx().args(["commit", &cid]))
    });
    out.push(("gx commit", "gx commit:", commit));

    out
}

// ---------------------------------------------------------------------------
// 1. The four silent verbs
// ---------------------------------------------------------------------------

/// 🔴 `req/470` H-01. Four verbs walked the road that writes and said nothing.
///
/// The assertion is deliberately in three parts, because three different repairs would each
/// satisfy a weaker one: that the road **was** walked (else the bed proves nothing), that the
/// sentence reached **stderr** (a JSON counter is not a sentence), and that the sentence names
/// **this** verb (the shipped string was `"gx serve: "`-prefixed at `serve.rs:495`/`:503`, so a
/// naive move would have `gx verify` announce itself as `gx serve`).
#[test]
fn r35_the_four_shipping_verbs_that_walk_the_announced_road_each_say_so() {
    let roads = the_four_roads();

    let mut silent: Vec<String> = Vec::new();
    let mut mislabelled: Vec<String> = Vec::new();
    let mut never_on_the_road: Vec<String> = Vec::new();

    for (verb, prefix, w) in &roads {
        if !w.moved() {
            never_on_the_road.push((*verb).to_string());
        }
        if !w.sentence_on_stderr() {
            silent.push((*verb).to_string());
        } else if !w.stderr.contains(prefix) {
            mislabelled.push(format!("{verb} (stderr names another verb)"));
        }
    }

    println!(
        "A35_FOUR_ROADS silent={silent:?} mislabelled={mislabelled:?} never_on_the_road={never_on_the_road:?}"
    );

    assert!(
        never_on_the_road.is_empty(),
        "the bed failed before the product did: {never_on_the_road:?} did not move the world, so \
         this run measures the bed's limit and not the verb's behaviour (audit 34 §7-3)"
    );
    assert!(
        silent.is_empty(),
        "req/470 H-01: {silent:?} walked 43 s7-3c's road, wrote over whatever the substrate held, \
         and printed no sentence on stderr. docs/LIMITS.md v0.5-t says every row that walks that \
         road prints one"
    );
    assert!(
        mislabelled.is_empty(),
        "req/471 §0-1: {mislabelled:?} printed the sentence under another verb's name. The \
         shipped prefix was fixed to \"gx serve: \" at serve.rs:495/:503 and must become each \
         verb's own name"
    );

    // The two halves a counter cannot carry (audit 34 §1-6 item 3).
    for (verb, _, w) in &roads {
        if w.stderr.contains(NOTE_MARK) {
            assert!(
                w.stderr.contains(NOT_CHECKED_MARK),
                "{verb}: the sentence must say what was **not** compared, not only that a road was walked"
            );
            assert!(
                w.stderr.contains(OVERWROTE_MARK),
                "{verb}: the sentence must say that this run may have written over somebody else's bytes"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2. The negative controls - the sentence must not be unconditional
// ---------------------------------------------------------------------------

/// 🔴 A sentence printed on every run would satisfy the test above and tell an operator nothing.
///
/// These are the two roads that must stay silent: a project that never crashed, and 43 s7-3b —
/// the road where the ledger **does** hold the leaf, which R33 made read the substrate instead of
/// writing to it. Green before the repair and after it; a repair that turns either of these red
/// has replaced a false silence with a false alarm.
#[test]
fn r35_the_roads_that_did_not_write_stay_silent() {
    // No crash at all.
    let h = pipeline("r35_repair_healthy", "before\n");
    h.commit_one("one\n");
    let healthy = third_party_then(&h, "repair_healthy", "gx repair --yes --json", true, |f| {
        run(f
            .gx()
            .args(["repair", "--json", "--yes"])
            .args(["--signing-key", &f.key_id]))
    });

    // 43 s7-3b: the ledger holds the leaf, so the road reads instead of applying.
    let b = pipeline("r35_repair_3b", "before\n");
    a_project_cut_inside_the_window(&b, false);
    let leaf = third_party_then(
        &b,
        "repair_3b_third_party",
        "gx repair --yes --json",
        true,
        |f| {
            run(f
                .gx()
                .args(["repair", "--json", "--yes"])
                .args(["--signing-key", &f.key_id]))
        },
    );

    println!(
        "A35_SILENT_CONTROLS healthy_note={} healthy_moved={} leaf_note={} leaf_moved={} leaf_rc={}",
        healthy.note_printed(), healthy.moved(), leaf.note_printed(), leaf.moved(), leaf.code
    );

    assert!(
        !healthy.note_printed(),
        "a project that never crashed announced a recovery it did not perform"
    );
    assert!(
        !healthy.moved(),
        "the negative control moved the world, so it is not a control"
    );
    assert!(
        !leaf.note_printed(),
        "43 s7-3b reads the substrate and refuses a third party's bytes (req/397 H-01); it must \
         not print the road-that-writes sentence"
    );
    assert!(
        !leaf.moved(),
        "43 s7-3b let a third party's bytes be replaced - that is a regression of req/397 H-01, \
         not this lane's finding"
    );
}

// ---------------------------------------------------------------------------
// 3. `gx wrap` - the membrane, driven at last
// ---------------------------------------------------------------------------

const DEMO_SERVER_ARG: &str = "__demo-notes-server";

struct Agent {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    n: u64,
    frames_seen: Vec<String>,
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
            frames_seen: Vec::new(),
        };
        me.ask(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "r35", "version": "0" },
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
            Some(line) => {
                self.frames_seen.push(line.clone());
                serde_json::from_str(&line).expect("every stdout frame is valid JSON")
            }
            None => {
                let mut text = String::new();
                if let Some(mut err) = self.child.stderr.take() {
                    let _ = err.read_to_string(&mut text);
                }
                panic!("gx wrap closed stdout answering {method:?}: {text}")
            }
        }
    }

    fn close(mut self) -> (String, Vec<String>) {
        self.stdin = None;
        let frames = std::mem::take(&mut self.frames_seen);
        let out = self.child.wait_with_output().expect("gx wrap exits");
        (String::from_utf8_lossy(&out.stderr).to_string(), frames)
    }
}

/// 🔴 `req/471` §0-3 — the membrane, driven. Three audits in a row declared `gx wrap` un-driven
/// and reasoned about it from source; this closes that by running it.
///
/// The bed is the same crash: a project cut inside 43 s7-3c's window, then an agent asks the
/// wrapped server for a write. `wrap.rs:547`/`:644` drive `pipeline::verify` and
/// `pipeline::commit`, which is where the recovery runs, so the sentence has to come out of the
/// membrane's **stderr** — and `wrap.rs`'s own module header is the reason it must be stderr and
/// not stdout: "the transport specification is explicit - the server MUST NOT write anything to
/// its stdout that is not a valid MCP message".
///
/// So this arm asserts **both** halves, and the second is the one that would catch a repair that
/// fixed the silence by breaking the protocol.
#[test]
fn r35_gx_wrap_carries_the_sentence_without_breaking_the_mcp_contract() {
    let fixture = support::pipeline_named("r35_wrap_3c", "before\n", "target.txt");
    let first = a_project_cut_inside_the_window(&fixture, true);
    println!("A35_WRAP_BED crashed_first_commit={first}");

    let note = fixture.project.join("note.txt");
    std::fs::write(&note, "the note as it stood before any agent touched it\n").expect("note");
    let arrivals = fixture.project.join("arrivals.log");
    let launcher = fixture.project.join("r35-server.sh");
    std::fs::write(
        &launcher,
        format!(
            "#!/bin/sh\nexec \"{}\" {DEMO_SERVER_ARG}\n",
            env!("CARGO_BIN_EXE_gx")
        ),
    )
    .expect("launcher");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let snap_before = snapshot(&fixture, "wrap_3c", "before");
    std::fs::write(&fixture.target, "THIRD PARTY\n").expect("a third party writes");

    // The sound catalogue r22's control arm uses, so the agent's call is **admitted** and reaches
    // `pipeline::verify`/`commit`. A refused call would never get to the recovery, and this arm
    // would measure the bed rather than the membrane.
    let catalogue = fixture.project.join("r35-catalogue.json");
    std::fs::write(
        &catalogue,
        serde_json::to_vec_pretty(&json!({
            "notes.write": {
                "restored_by": "notes.restore",
                "arguments": {
                    "uri": { "forward": "uri" },
                    "contents": "prior_contents_utf8",
                }
            },
            "notes.restore": {
                "restored_by": "notes.write",
                "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
            }
        }))
        .expect("json"),
    )
    .expect("catalogue");

    let args: Vec<String> = vec![
        "--project".to_string(),
        fixture.project.display().to_string(),
        "wrap".to_string(),
        "--endpoint".to_string(),
        "stdio://r35".to_string(),
        "--actor-key".to_string(),
        fixture.key_id.clone(),
        "--actor-model".to_string(),
        "r35-probe".to_string(),
        "--restore-catalogue".to_string(),
        catalogue.display().to_string(),
        "--".to_string(),
        launcher.display().to_string(),
    ];

    let mut agent = Agent::open(&args, &fixture.home, &arrivals);
    let answer = agent.ask(
        "tools/call",
        json!({
            "name": "notes.write",
            "arguments": { "uri": format!("file://{}", note.display()), "contents": "the agent's edit\n" },
        }),
    );
    println!(
        "A35_WRAP_ANSWER {}",
        serde_json::to_string(&answer).unwrap_or_default()
    );
    let (stderr, frames) = agent.close();

    println!("A35_WRAP_STDERR <<{stderr}>>");
    println!("A35_WRAP_FRAMES count={}", frames.len());
    for f in &frames {
        println!("A35_WRAP_FRAME <<{f}>>");
    }
    let snap_after = snapshot(&fixture, "wrap_3c", "after");
    println!(
        "A35_WRAP_SNAP before_commits={} after_commits={} world_now={:?}",
        snap_before["journal_commits"],
        snap_after["journal_commits"],
        fixture.target_contents()
    );

    // Half one: stdout stayed a valid MCP stream. Asserted first, because a repair that printed
    // the sentence on stdout would satisfy half two and destroy the product.
    assert!(
        !frames.is_empty(),
        "the membrane answered nothing, so this arm measured nothing"
    );
    for f in &frames {
        let parsed: std::result::Result<Value, _> = serde_json::from_str(f);
        assert!(
            parsed.is_ok(),
            "gx wrap put a non-JSON line on stdout: <<{f}>>. wrap.rs's header: the server MUST \
             NOT write anything to its stdout that is not a valid MCP message"
        );
    }

    // Half two: the sentence came out of the membrane, on stderr.
    assert!(
        stderr.contains(NOTE_MARK) || stderr.contains(ABORT_MARK),
        "req/471 §0-3: `gx wrap` drove pipeline::verify/commit over a project cut inside 43 \
         s7-3c's window and the membrane said nothing about it. stderr was: <<{stderr}>>"
    );
}

// ---------------------------------------------------------------------------
// 4. The census - so that a road added later is counted rather than remembered
// ---------------------------------------------------------------------------

/// 🔴 The shape of `req/470` H-01 was not "one function was wrong". It was "a road existed and
/// nobody had wired the sentence to it". A repair that only wires today's five roads leaves the
/// defect's shape intact for the sixth.
///
/// This is audit 34's own census (`a34_the_denominator_of_the_roads_and_of_the_sentence`,
/// `A34_DENOM recover_call_sites=6 note_call_sites=1`) turned from a **measurement** into a
/// **gate**: every shipped call site that reaches the recovery must be on a road that announces.
///
/// The predicate is deliberately coarse — a site is either a call to the session's own wrapper
/// (which announces by construction, and the four behavioural arms above are what pin that) or a
/// direct call on an engine, in which case its own file must name the announcer. A finer predicate
/// would be a parser, and a parser in a test is a second implementation waiting to disagree.
#[test]
fn r35_every_shipped_recover_call_site_is_on_a_road_that_announces() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .to_path_buf();

    let mut sites: Vec<(String, String)> = Vec::new();
    let mut files_naming_the_announcer: Vec<String> = Vec::new();

    for crate_dir in std::fs::read_dir(&root).expect("crates/").flatten() {
        let src = crate_dir.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)
                .expect("a source directory")
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let mut names_announcer = false;
                for (n, line) in text.lines().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    // The announcer is named by its call, never by its definition.
                    if line.contains("announce_recovery(") && !line.contains("pub fn ") {
                        names_announcer = true;
                    }
                    if line.contains(".recover(") {
                        sites.push((
                            format!("{}:{}", path.display(), n + 1),
                            line.trim().to_string(),
                        ));
                    }
                }
                if names_announcer {
                    files_naming_the_announcer.push(path.display().to_string());
                }
            }
        }
    }

    sites.sort();
    files_naming_the_announcer.sort();
    println!("A35_DENOM recover_call_sites={}", sites.len());
    for (where_, what) in &sites {
        println!("A35_RECOVER_SITE {where_} :: {what}");
    }
    println!("A35_ANNOUNCERS count={}", files_naming_the_announcer.len());
    for f in &files_naming_the_announcer {
        println!("A35_ANNOUNCER {f}");
    }

    let mut unwired: Vec<String> = Vec::new();
    for (where_, what) in &sites {
        // A call through the session's wrapper announces inside the wrapper.
        if what.contains("session.recover(") || what.contains("sess.recover(") {
            continue;
        }
        // `Layout::recover` is a different function on a different type: it repairs `.gx/`'s
        // directory shape and never touches 43 §7's road.
        if what.contains("layout.recover(") {
            continue;
        }
        let file = where_
            .rsplit_once(':')
            .map(|(f, _)| f.to_string())
            .unwrap_or_default();
        if !files_naming_the_announcer.contains(&file) {
            unwired.push(where_.clone());
        }
    }

    assert!(
        unwired.is_empty(),
        "req/470 H-01: these shipped call sites reach 43 §7's recovery on a road that announces \
         nothing: {unwired:?}. Audit 34 measured recover_call_sites=6 against note_call_sites=1"
    );
}
