// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R27 item 1 (`req/331` §0-1, from `req/329` M-01, `req/38` §233 ruling 2)** — the re-entrant
//! abort answer drops the roll-back account Σ is still holding.
//!
//! # What broke
//!
//! `gx-cli/src/pipeline.rs` has two roads that answer *what happened to this transformation*. The
//! non-terminal one asks `Engine::rollback` and `Engine::rollback_not_attempted_because` and puts
//! both on the object. [`terminal_answer`] — the road a **second** asking takes — builds `aborted`
//! and `reason` and asks neither. Same verb, same exit code, same `reason`; the first answer
//! carries `detail="NotAttempted" because="NoInverseWasEscrowed"` and the second carries neither.
//!
//! The twenty-sixth audit measured that the value is still in Σ after the writing process exited
//! (`A26_SIGMA_HOLDS aborted_records=1 carries_not_attempted=true`), so nothing here is a fact the
//! engine lost — it is a fact the answer stopped asking for.
//!
//! # The latent false sentence, and why this suite closes it by construction
//!
//! `wrap::apply_failed_clause` keys on `detail`. Handed an object with **no** `detail` member it
//! falls into the arm for *a word this build does not recognise* and says so about `None`, then
//! sends the reader to `detail` — a member that object does not have. Both halves are false: this
//! build knows all three values, and the remedy names something absent. The repair separates *the
//! answer does not carry the account* from *the engine grew a word this build has not been taught*,
//! so the sentence the audit quoted stops being constructible rather than merely stopping being
//! delivered.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gx_mcp_wire::jsonrpc;
use serde_json::{json, Value};

mod support;

const BEFORE: &str = "the note as it stood before any agent touched it\n";
const AFTER: &str = "the note after an agent wrote through gx wrap\n";
const DEMO_SERVER_ARG: &str = "__demo-notes-server";
const ARRIVALS_ENV: &str = "GX_DEMO_LOG";
/// The shipped server's own fault injection (`crates/gx-cli/src/demo.rs`): that one tool answers
/// `-32603` instead of running, so `adapter.apply` returns `Err` and T-10c is entered.
const TOOL_REFUSES_ENV: &str = "GX_DEMO_TOOL_REFUSES";

/// Every measurement is written to stdout **and** appended to a file, so a run whose panic eats the
/// captured stdout still leaves the number behind (`req/324` §9-3's lesson).
fn record(line: &str) {
    println!("{line}");
    if let Ok(path) = std::env::var("R27_MEASUREMENTS") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

// ---------------------------------------------------------------------------
// A `gx wrap` session, from the agent's side (bed: r19_escalation_road.rs)
// ---------------------------------------------------------------------------

struct Wrap {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Wrap {
    fn spawn(args: &[String], home: &Path, arrivals: &Path, env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gx"));
        command
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env(ARRIVALS_ENV, arrivals);
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
                "clientInfo": { "name": "r27", "version": "0" },
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
            None => panic!("gx wrap closed its stdout without answering {method:?}"),
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        let frame = jsonrpc::notification(method, params);
        jsonrpc::write_frame(self.stdin.as_mut().expect("open"), &frame).expect("write");
    }

    fn finish(mut self) -> String {
        self.stdin = None;
        let out = self.child.wait_with_output().expect("gx wrap exits");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

struct Fixture {
    pipeline: support::Pipeline,
    note: PathBuf,
    uri: String,
    arrivals: PathBuf,
    ruler: String,
}

fn fixture(name: &str) -> Fixture {
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

    // Kept though no arm calls it today: it is the reader for the arrivals file this fixture
    // writes, and an arm that asserts on arrival order needs it. Allowed rather than removed
    // (no-delete, req/477 stage 2).
    #[allow(dead_code)]
    fn arrivals_now(&self) -> Vec<String> {
        std::fs::read_to_string(&self.arrivals)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn wrap_args(&self, catalogue: Option<&Path>) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "--project".into(),
            self.pipeline.project.display().to_string(),
            "wrap".into(),
            "--endpoint".into(),
            "stdio://r27".into(),
            "--actor-key".into(),
            self.pipeline.key_id.clone(),
            "--actor-model".into(),
            "r27-probe".into(),
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

    fn server_flags(&self) -> Vec<String> {
        vec![
            "--mcp-server".into(),
            env!("CARGO_BIN_EXE_gx").to_string(),
            "--mcp-server-arg".into(),
            DEMO_SERVER_ARG.into(),
            "--mcp-endpoint".into(),
            "stdio://r27".into(),
        ]
    }

    fn gx(&self) -> Command {
        let mut cmd = self.pipeline.gx();
        cmd.env(ARRIVALS_ENV, &self.arrivals);
        cmd
    }
}

/// One `notes.write` through `gx wrap`, and the answer the agent was handed.
fn one_write(fixture: &Fixture, catalogue: Option<&Path>, env: &[(&str, &str)]) -> (Value, String) {
    let mut wrap = Wrap::spawn(
        &fixture.wrap_args(catalogue),
        &fixture.pipeline.home,
        &fixture.arrivals,
        env,
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

/// The Given both road-(a) probes share: a call with **no** restore declaration, so `invert`
/// answers `None`, so E-M3-4 escalates — then a person approves it (T-5).
///
/// `invert_available == false` is the **one** condition that produces an `Escalate` in v0.1
/// (`gx-engine/src/pipeline.rs`), so this is the only road to
/// `NotAttemptedBecause::NoInverseWasEscrowed`: the arm is unreachable without a human ruling.
fn escalated_then_approved(fixture: &Fixture) -> String {
    let (answered, _) = one_write(fixture, None, &[]);
    let meta = &answered["result"]["_meta"];
    let tid = meta["gx/transformation"]
        .as_str()
        .unwrap_or_else(|| panic!("the escalated answer carries a transformation: {answered}"))
        .to_string();
    assert_eq!(
        meta["gx/verdict"], "Escalate",
        "the Given of this road is E-M3-4's escalation (no declared inverse): {answered}"
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
    assert_eq!(approved.code, 0, "the ruling runs: {}", approved.stderr);
    assert_eq!(
        approved.json()["state"],
        "Admitted",
        "43 T-5's human ruling is an Admit: {}",
        approved.stdout
    );
    tid
}

// ---------------------------------------------------------------------------
// Bed control — the road reaches a commit at all when nothing is injected
// ---------------------------------------------------------------------------

/// 🔴 **Bed soundness, measured before any finding** (`req/326` §6-3's lesson: audit 25's bed could
/// not reach the gate it claimed to measure, and only a negative control revealed it).
///
/// Without the fault injection the approved transformation **commits**. If this arm were red, every
/// `NotAttempted` the probes below report would be a bed that never reached T-10c rather than a
/// road that reached it and answered.
fn reversible_catalogue(dir: &Path) -> PathBuf {
    support::write_json(
        &dir.join("catalogue-reversible.json"),
        &json!({
            "notes.write": {
                "restored_by": "notes.restore",
                "arguments": {
                    "uri": { "forward": "uri" },
                    "contents": "prior_contents_utf8",
                },
            },
        }),
    )
}

/// Every record frame in a journal file, as `(offset, framed_length)` (bed: `serve_runtime_r7.rs`).
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

fn truncate_at(path: &Path, at: u64) -> (u64, u64) {
    let before = std::fs::metadata(path).expect("stat").len();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
    (before, at)
}

// ---------------------------------------------------------------------------
// Bed control — the road reaches T-10c at all, and the *first* asking is right
// ---------------------------------------------------------------------------

/// 🔴 **Bed soundness, measured before any finding** (`req/326` §6-3's lesson).
///
/// If this arm were red, every "the second answer is thinner" below would be a bed that never
/// reached `Rollback::NotAttempted` rather than a road that reached it and then forgot it.
#[test]
fn a_bed_control_the_first_asking_carries_the_rollback_account() {
    let fixture = fixture("r27_bed_first");
    let tid = escalated_then_approved(&fixture);
    let first = support::run(
        fixture
            .gx()
            .env(TOOL_REFUSES_ENV, "notes.write")
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    let a: Value = serde_json::from_str(first.stdout.trim()).unwrap_or(Value::Null);
    record(&format!(
        "R27_BED_FIRST rc={} reason={} detail={} because={}",
        first.code, a["reason"], a["detail"], a["not_attempted_because"]
    ));
    assert_eq!(a["reason"], "ApplyFailed", "the bed reaches T-10c: {a}");
    assert_eq!(
        a["detail"], "NotAttempted",
        "the first asking carries the value: {a}"
    );
    assert_eq!(
        a["not_attempted_because"], "NoInverseWasEscrowed",
        "and the cause R26 wired beside it: {a}"
    );
}

// ---------------------------------------------------------------------------
// The finding — the same question, asked twice
// ---------------------------------------------------------------------------

/// 🔴 **`req/329` M-01** — the re-entrant answer carries the roll-back account the first one did.
///
/// The audit's `A26_REENTRY` line is the red this arm reproduces:
///
/// ```text
/// first  rc=5 reason="ApplyFailed" detail="NotAttempted" because="NoInverseWasEscrowed"
/// second rc=5 reason="ApplyFailed" detail=null           because=null           reentered=true
/// ```
///
/// `reentered: true` is not a defence. 44 §1.2's idempotency contract is that retrying answers the
/// same thing, and a script that branches on `detail` cannot tell *not attempted* from *succeeded*
/// from *failed* the second time it asks.
#[test]
fn b_the_same_question_asked_twice_answers_with_the_rollback_both_times() {
    let fixture = fixture("r27_reentry");
    let tid = escalated_then_approved(&fixture);
    let first = support::run(
        fixture
            .gx()
            .env(TOOL_REFUSES_ENV, "notes.write")
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    let a: Value = serde_json::from_str(first.stdout.trim()).unwrap_or(Value::Null);
    let second = support::run(
        fixture
            .gx()
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    let b: Value = serde_json::from_str(second.stdout.trim()).unwrap_or(Value::Null);
    record(&format!(
        "R27_REENTRY first rc={} reason={} detail={} because={} | second rc={} reason={} detail={} because={} reentered={}",
        first.code, a["reason"], a["detail"], a["not_attempted_because"],
        second.code, b["reason"], b["detail"], b["not_attempted_because"], b["reentered"]
    ));
    assert_eq!(
        second.code, first.code,
        "the contract this arm holds is that the second asking answers the same thing"
    );
    assert_eq!(b["reason"], a["reason"], "same reason: {b}");
    assert_eq!(
        b["reentered"], true,
        "the second asking is the re-entrant road, so the repair is measured on it: {b}"
    );
    assert_eq!(
        b["detail"], a["detail"],
        "🔴 `req/329` M-01: the value is on the journal's `Aborted` record, `Engine::rollback` \
         reads it back, and the non-terminal road on this same file already asks for it. The \
         re-entrant answer drops it: {b}"
    );
    // 🔴 The cause is a **different fact from the value**, and this arm holds the difference rather
    // than flattening it.
    //
    // `req/331` §0-1 asks for "detail/because filled on the second asking too". The value is filled
    // and is asserted above: it is written into the journal's `Aborted` record and Σ hands it back.
    // The **cause** is not a component of Σ — `gx-engine/src/store.rs` declares that in as many
    // words, and `Engine::rollback_not_attempted_because` reads a map this process fills when *it*
    // reaches the abort — so a process that did not abort the row has no cause to give. Asserting
    // that it re-appears would be asking this repair to invent one, which is the failure R25 and
    // R26 were each a repair of, one level up.
    //
    // What was actually broken is asserted instead: the member was **absent**, and an absent member
    // is what drove `apply_failed_clause` into the arm that told an agent this build does not know
    // a word. The member is now written on every road, carrying `null` where `null` is the honest
    // answer — and `null` is a fact a reader can branch on, where absence was not.
    // `req/329` §2's own reading is the authority here: what is dropped is the **value**, and the
    // cause's absence after a restart is the honest half.
    assert!(
        b.get("not_attempted_because").is_some(),
        "🔴 `req/329` M-01: the member itself has to be on the object. Absent, it is indistinguishable \
         from a road that never had a cause, and it is what composed the sentence this suite's arm \
         `c` refuses: {b}"
    );
    assert_eq!(
        b["not_attempted_because"],
        Value::Null,
        "the cause is not a component of Σ (`gx-engine/src/store.rs`), so the re-entrant road \
         answers `null` — honestly, and with the member present: {b}"
    );
}

/// 🔴 The sentence an agent would be handed, on both askings, from the real objects.
///
/// Not a string comparison for its own sake: the audit's finding is that the second object drives
/// `apply_failed_clause` into the arm for *a word this build does not recognise*, which is false
/// about the build and whose remedy names a member the object does not carry.
#[test]
fn c_neither_asking_produces_a_sentence_that_is_false_about_this_build() {
    let fixture = fixture("r27_sentences");
    let tid = escalated_then_approved(&fixture);
    let first = support::run(
        fixture
            .gx()
            .env(TOOL_REFUSES_ENV, "notes.write")
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    let a: Value = serde_json::from_str(first.stdout.trim()).unwrap_or(Value::Null);
    let second = support::run(
        fixture
            .gx()
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    let b: Value = serde_json::from_str(second.stdout.trim()).unwrap_or(Value::Null);
    let (sa, sb) = (
        gx_cli::wrap::apply_failed_clause(&a),
        gx_cli::wrap::apply_failed_clause(&b),
    );
    record(&format!("R27_SENTENCE first={sa:?}"));
    record(&format!("R27_SENTENCE second={sb:?}"));
    for (which, said) in [("first", &sa), ("second", &sb)] {
        assert!(
            !said.contains("does not recognise the word it is carrying (None)"),
            "🔴 `req/329` M-01: the {which} sentence tells an agent this build does not know a \
             word, when what happened is that the answer carried no word at all — and then sends \
             it to a member the object does not have: {said:?}"
        );
    }
}

/// 🔴 The latent sentence, closed **by construction** rather than by the road that fed it.
///
/// Arm `b` repairs the object. This one asks the harder question `req/331` §0-1 sets: can the false
/// sentence still be *composed* by anyone who hands the shipped function an object with no `detail`?
/// A hand-built object stands in for every future road — a policy pack, an HTTP route, a third-party
/// reader of `gx log` — so the repair is a property of the function and not of one caller.
#[test]
fn d_an_object_with_no_detail_member_is_answered_without_claiming_an_unknown_word() {
    let absent = json!({ "reason": "ApplyFailed" });
    let null = json!({ "reason": "ApplyFailed", "detail": Value::Null });
    let unknown = json!({ "reason": "ApplyFailed", "detail": "AWordThisBuildWasNeverTaught" });
    let (sa, sn, su) = (
        gx_cli::wrap::apply_failed_clause(&absent),
        gx_cli::wrap::apply_failed_clause(&null),
        gx_cli::wrap::apply_failed_clause(&unknown),
    );
    record(&format!("R27_CLAUSE_ABSENT {sa:?}"));
    record(&format!("R27_CLAUSE_NULL {sn:?}"));
    record(&format!("R27_CLAUSE_UNKNOWN {su:?}"));
    for (which, said) in [("absent", &sa), ("null", &sn)] {
        assert!(
            !said.contains("does not recognise the word"),
            "🔴 an answer with a {which} `detail` is not an engine word this build was never \
             taught, and saying so is false about the build: {said:?}"
        );
        assert!(
            !said.contains("read `detail`"),
            "🔴 the remedy names the member the object does not carry: {said:?}"
        );
        assert!(
            said.contains("`gx log`"),
            "🔴 a reader still has to be sent somewhere that holds the account: {said:?}"
        );
    }
    assert!(
        su.contains("does not recognise the word"),
        "🔴 and the arm that *is* about an unknown engine word keeps its sentence, or this repair \
         has widened into the case it was meant to leave alone: {su:?}"
    );
}

// ---------------------------------------------------------------------------
// The `gx repair` half of M-01
// ---------------------------------------------------------------------------

/// 🔴 **`req/329` M-01, road (b)** — `gx repair`'s report names the cause, and the two §7-3b arms
/// it used to fold together are told apart.
///
/// The Given is the audit's: an MCP commit whose `ApplyStarted` is on disk, whose ledger leaf is
/// gone, and a `gx repair` that by construction has no MCP server to re-apply against. That is
/// `RecoveryPath::ApplyWasAnnounced`, where the engine sets
/// `NotAttemptedBecause::RecoveredWithoutRebuilding`.
///
/// Two facts an operator cannot get today: the report says `resumed`, which counts
/// `LedgerHeldTheCommit` — *the commit completed* — in the same number as `ApplyWasAnnounced` —
/// *the apply was announced and nothing was rebuilt*; and the cause the recovery itself set appears
/// nowhere in the report (`A26_REPAIR_CAUSE_IN_REPORT present=false`).
#[test]
fn e_the_repair_report_names_the_cause_and_separates_the_two_resumed_arms() {
    let fixture = fixture("r27_repair");
    let catalogue = reversible_catalogue(&fixture.pipeline.project);
    let (answered, _) = one_write(&fixture, Some(&catalogue), &[]);
    let meta = &answered["result"]["_meta"];
    let tid = meta["gx/transformation"]
        .as_str()
        .unwrap_or_else(|| panic!("the committed answer carries a transformation: {answered}"))
        .to_string();

    let layout = gx_cli::layout::Layout::open(&fixture.pipeline.project).expect("project opens");
    let journal_path = layout.journal_path();
    let ledger_path = layout.ledger_path();
    let journal = std::fs::read(&journal_path).expect("read the journal");
    let kinds: Vec<&'static str> = gx_engine::replay(&journal)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    let spans = frames(&journal);
    let last_apply = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "ApplyStarted")
        .map(|(i, _)| i)
        .next_back()
        .expect("the commit announced an apply");
    truncate_at(
        &journal_path,
        (spans[last_apply].0 + spans[last_apply].1) as u64,
    );
    let ledger = std::fs::read(&ledger_path).expect("read the ledger");
    let leaves = ledger_frames(&ledger);
    truncate_at(&ledger_path, leaves.first().map_or(0, |leaf| leaf.0 as u64));
    let head_removed = std::fs::remove_file(layout.head_path()).is_ok();
    record(&format!(
        "R27_REPAIR_CUT leaves={} head_removed={head_removed}",
        leaves.len()
    ));

    let repaired = support::run(
        fixture
            .gx()
            .arg("repair")
            .args(["--signing-key", &fixture.pipeline.key_id])
            .arg("--yes"),
    );
    let report = repaired.json();
    record(&format!(
        "R27_REPAIR_REPORT rc={} repaired={} recover={}",
        repaired.code, report["repaired"], report["recover"]
    ));
    let recover = &report["recover"];
    assert_eq!(
        recover["resumed"], 1,
        "the bed reaches §7-3b's window: {recover}"
    );
    assert_eq!(
        recover["apply_was_announced"], 1,
        "🔴 `req/329` M-01: `ApplyWasAnnounced` — the apply was announced and this run rebuilt \
         nothing — is folded into `resumed` beside `LedgerHeldTheCommit`, which means *the commit \
         completed*. An operator deciding whether to trust a recovered project cannot tell the two \
         apart: {recover}"
    );
    assert_eq!(
        recover["ledger_held_the_commit"], 0,
        "and the other arm is counted on its own so the pair is readable: {recover}"
    );
    let causes = &recover["not_attempted_because"];
    record(&format!("R27_REPAIR_CAUSES {causes}"));
    assert!(
        causes
            .as_array()
            .is_some_and(|a| a.iter().any(|c| c == "RecoveredWithoutRebuilding")),
        "🔴 `req/329` M-01: the recovery set `RecoveredWithoutRebuilding` on the row it closed, and \
         the report an operator reads does not carry it. The cause is the one fact that says which \
         of `gx repair`'s roads left the world where it is: {report}"
    );

    // And what a reader asking about the row afterwards, in a new process, is handed.
    let asked = support::run(
        fixture
            .gx()
            .args(fixture.server_flags())
            .args(["commit", &tid]),
    );
    let json: Value = serde_json::from_str(asked.stdout.trim()).unwrap_or(Value::Null);
    record(&format!(
        "R27_REPAIR_READBACK rc={} reason={} detail={} because={}",
        asked.code, json["reason"], json["detail"], json["not_attempted_because"]
    ));
    assert_eq!(
        json["detail"], "NotAttempted",
        "🔴 the re-entrant road again: the repaired row's roll-back value is in Σ and the answer \
         drops it: {json}"
    );
    let sentence = gx_cli::wrap::apply_failed_clause(&json);
    record(&format!("R27_REPAIR_SENTENCE {sentence:?}"));
    assert!(
        !sentence.contains("does not recognise the word it is carrying (None)"),
        "🔴 and the sentence the audit quoted is composed from exactly this object: {sentence:?}"
    );
}

// ---------------------------------------------------------------------------
// Sibling sweep — derived from the source on every run, not written by hand
// ---------------------------------------------------------------------------

/// 🔴 **`req/331` §0-1's sibling sweep** — every road in `gx-cli/src` that answers *this
/// transformation aborted* asks Σ what became of the roll-back.
///
/// Derived rather than enumerated, for the reason `req/329` M-03 is about: an enumeration holds the
/// roads a lane happened to think of. The denominator is every function in `crates/gx-cli/src`
/// whose body writes the `aborted` member; the requirement is that the same body asks `rollback`.
/// A third road added tomorrow enters this census without anyone remembering to add it.
#[test]
fn f_every_road_that_says_a_transformation_aborted_asks_what_became_of_the_rollback() {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut composing: Vec<String> = Vec::new();
    let mut answering_about_an_abort: Vec<String> = Vec::new();
    let mut without: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&src_dir)
        .expect("gx-cli/src is readable")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // Comment lines dropped: a road named in prose is not a road.
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect();
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("\"aborted\"") {
                continue;
            }
            // Walk up to the nearest `fn` header, then take its body to the closing brace at that
            // indentation.
            let Some(start) = (0..=i).rev().find(|n| {
                let t = lines[*n].trim_start();
                t.starts_with("fn ") || t.starts_with("pub fn ") || t.starts_with("async fn ")
            }) else {
                continue;
            };
            let header = lines[start];
            let name = header
                .split("fn ")
                .nth(1)
                .and_then(|rest| rest.split(['(', '<']).next())
                .unwrap_or_default()
                .to_string();
            let indent = header.len() - header.trim_start().len();
            let mut body = String::new();
            for candidate in lines.iter().skip(start + 1) {
                let c_trim = candidate.trim_start();
                let c_indent = candidate.len() - c_trim.len();
                if c_trim.starts_with('}') && c_indent <= indent {
                    break;
                }
                body.push_str(candidate);
                body.push('\n');
            }
            let road = format!("{file}::{name}");
            if composing.contains(&road) {
                continue;
            }
            composing.push(road.clone());
            // 🔴 The narrowing, declared rather than applied by name.
            //
            // Writing the `aborted` member is not the same as answering **about an abort**:
            // `canonicalise_refusal` writes `"aborted": false` with `reason: "NotCanonicalizable"`
            // and a `detail` that is an error string, for a transformation that did not abort.
            // Asking `Engine::rollback` there would be asking what became of a roll-back that was
            // never in question. The road this arm is about is one keyed on the **abort taxonomy**
            // — `Lifecycle::Aborted` or `AbortReason` — and both numbers are recorded above, so
            // the wider denominator stays in the record rather than being quietly dropped.
            if !body.contains("Lifecycle::Aborted") && !body.contains("AbortReason") {
                continue;
            }
            answering_about_an_abort.push(road.clone());
            // 🔴 **R29 / `req/361` M-01, applied to the question rather than to the measured
            // row.** The twenty-eighth audit filed this predicate where it found it — in R28's
            // `r28_abort_answer_sweep.rs` — and R28's sweep superseded this arm without deleting
            // it (no-delete). So the defect the audit named had **two** live instances and the
            // brief named one. `body.contains("rollback")` is satisfied by the word sitting in a
            // string literal, and R28 shipped exactly such a `detail` sentence on the road this
            // arm walks: the machinery could be deleted and both gates would still answer *this
            // road asks*. The narrowing above is deliberate and stays; what changes is that
            // "asks" now means **calls the machinery** rather than **contains the word**.
            //
            // Found by a census over the repaired tree rather than by the brief, which is the
            // reason the census is run: a repair aimed at the row that was measured leaves its
            // siblings shipping.
            let asks = ["rollback_facts(", "with_rollback_facts", ".rollback("]
                .iter()
                .any(|call| body.contains(call));
            if !asks {
                without.push(road);
            }
        }
    }
    composing.sort();
    answering_about_an_abort.sort();
    without.sort();
    record(&format!(
        "R27_ABORT_ANSWER_SITES composing_the_member={composing:?} \
         answering_about_an_abort={answering_about_an_abort:?} without_the_rollback={without:?}"
    ));
    assert!(
        answering_about_an_abort.len() >= 2,
        "🔴 the denominator has to be found before it can be held: {answering_about_an_abort:?} \
         out of {composing:?}"
    );
    assert!(
        without.is_empty(),
        "🔴 `req/329` M-01: {without:?} answer *this transformation aborted* without asking Σ what \
         became of the roll-back. The value is on a signed record and the accessor is one call \
         away; a road that does not ask hands a script an answer it cannot branch on."
    );
}
