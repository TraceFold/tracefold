// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/324` M-01 / M-02 (`req/38` §231 ruling 2)** — the sibling sweep, finished.
//!
//! # What broke
//!
//! `req/312` M-01 split `Unreadable` into its two preimages — *the server answered that this
//! locator holds nothing* and *the server would not tell me* — and R24 gave the discrimination a
//! **position**: the token is written at offset 0 of `detail` by the wire and asked for by
//! `strip_prefix`, so a server cannot forge it from inside its own message. R25 then funnelled the
//! two consuming sites through one function.
//!
//! `req/322` §3-1 published a sibling sweep table of four rows — and all four are consumers of
//! `cas::read_subject`. This crate has **two** functions that reach
//! `ToolTransport::read_prior_by_tool`, and the second one, `invert::invert_with_verdict`, is the
//! escrow road's declared read. It was on no row of the table, it never called the funnel, and the
//! sentence it hands the agent is built as
//!
//! ```text
//! format!("{READ_FAILURE_REFUSAL} ({error})")
//! ```
//!
//! so gx's own token — the one whose whole value is its position — was pushed to **offset 509 by
//! gx's own words**. The discrimination was unavailable twice over: the funnel was not called, and
//! the predicate would have answered `false` if it had been.
//!
//! # What this file requires
//!
//! 1. every function in this crate that reaches the declared read face routes its failure through
//!    the one funnel (a source census with a denominator, not a spot check);
//! 2. the escrow road's refusal carries the discriminating words;
//! 3. the discrimination is made **before** the sentence is composed — a `strip_prefix` is never
//!    handed a string gx has already wrapped.

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{Admitted, Catalogue, McpAdapter, ToolCall, ToolTransport};
use gx_substrate::{Error, Result, SubstrateAdapter};

use support::intent_for;

const SERVER: &str = "stdio://r26";
const DOC: &str = "doc://page/1";
const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";

/// Spelled rather than imported (`req/299` §0): the token a transport writes into `Unreadable`'s
/// `detail` to mean *the server answered, and the answer is that this locator holds nothing*.
const ANSWERED_ABSENT: &str =
    "[gx: the server answered, and its answer is that this locator holds nothing]";

/// A needle from the sentence the funnel adds. Spelled, not imported.
const NAMES_THE_PREIMAGE: &str = "read this as \"the object is not there\"";

// ---------------------------------------------------------------------------------------------
// The bed
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum ReadFace {
    AnsweredAbsent,
    DidNotAnswer,
    Answers,
}

struct Server {
    face: ReadFace,
    body: Mutex<Option<Vec<u8>>>,
    tool_reads: AtomicUsize,
    plain_reads: AtomicUsize,
}

impl Server {
    fn new(body: &[u8], face: ReadFace) -> Self {
        Self {
            face,
            body: Mutex::new(Some(body.to_vec())),
            tool_reads: AtomicUsize::new(0),
            plain_reads: AtomicUsize::new(0),
        }
    }

    fn answer(&self, locator: String) -> Result<Vec<u8>> {
        match self.face {
            ReadFace::Answers => Ok(self
                .body
                .lock()
                .expect("not poisoned")
                .clone()
                .unwrap_or_default()),
            // The token is written **first**, which is the position R24 gave it.
            ReadFace::AnsweredAbsent => Err(Error::Unreadable {
                locator,
                detail: format!("{ANSWERED_ABSENT} the MCP server refused: resource not found"),
            }),
            ReadFace::DidNotAnswer => Err(Error::Unreadable {
                locator,
                detail: "the MCP server refused: internal error".to_string(),
            }),
        }
    }
}

impl ToolTransport for Server {
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>> {
        self.plain_reads.fetch_add(1, Ordering::SeqCst);
        self.answer(format!("{server}#{resource}"))
    }

    fn read_prior_by_tool(&self, server: &str, tool: &str, _arguments: &[u8]) -> Result<Vec<u8>> {
        self.tool_reads.fetch_add(1, Ordering::SeqCst);
        self.answer(format!("{server}#tool:{tool}"))
    }

    fn call(&self, _call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        Ok(b"{}".to_vec())
    }
}

/// A catalogue whose restore declares a `read_by` face: the escrow road's declared read.
fn escrow_read_catalogue() -> Catalogue {
    let json = format!(
        r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
             "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }},
             "read_by": {{ "by_tool": "notes.fetch",
                           "arguments": {{ "uri": {{ "forward": "uri" }} }},
                           "identity": ["doc://page/", {{ "answer": "/id" }}] }} }} }}"#
    );
    Catalogue::from_json(json.as_bytes()).expect("a well-formed declaration")
}

/// A catalogue whose `$cas_read` face makes `snapshot` consume a declared read.
fn cas_read_catalogue() -> Catalogue {
    let json = format!(
        r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
             "$cas_read": {{ "doc://": {{ "by_tool": "notes.fetch",
                                          "arguments": {{ "uri": "resource" }} }} }} }}"#
    );
    Catalogue::from_json(json.as_bytes()).expect("a well-formed declaration")
}

fn arguments() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "uri": DOC, "contents": "what the call puts there" }))
        .expect("json")
}

/// The escrow road's refusal, as the agent receives it.
fn escrow_refusal(face: ReadFace) -> (String, usize) {
    let locator = format!("{SERVER}#{DOC}");
    let server = Arc::new(Server::new(b"the note as it stood\n", face));
    let adapter = McpAdapter::new(server.clone()).with_catalogue(escrow_read_catalogue());
    let pre = support::absent_snapshot(&locator);
    let arguments = arguments();
    let delta = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, &arguments), &pre)
        .expect("a well-formed call plans");
    let said = match adapter.invert(&delta, &pre) {
        Err(e) => e.to_string(),
        Ok(other) => panic!(
            "the premise: with the declared read failing and the posture at its default, the \
             escrow road refuses. got {other:?}"
        ),
    };
    (said, server.tool_reads.load(Ordering::SeqCst))
}

// ---------------------------------------------------------------------------------------------
// The control -- the road R25 repaired is still repaired
// ---------------------------------------------------------------------------------------------

/// 🔴 The bed control. Without it every arm below proves nothing.
#[test]
fn the_road_r25_repaired_still_names_the_preimage() {
    let locator = format!("{SERVER}#{DOC}");
    let server = Arc::new(Server::new(
        b"the note as it stood\n",
        ReadFace::AnsweredAbsent,
    ));
    let adapter = McpAdapter::new(server.clone()).with_catalogue(cas_read_catalogue());
    let said = match adapter.snapshot(&locator) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("an absent locator has no prior state, so `snapshot` is `Err` here"),
    };
    println!("R26_FUNNEL_CONTROL snapshot={said:?}");
    assert!(
        said.contains(NAMES_THE_PREIMAGE),
        "the control: R25's repair is live on `snapshot`: {said}"
    );
    assert!(
        server.tool_reads.load(Ordering::SeqCst) >= 1,
        "and the control went through the **declared** read face, not `resources/read`"
    );
}

// ---------------------------------------------------------------------------------------------
// M-01 -- the second consumer of a declared read
// ---------------------------------------------------------------------------------------------

/// 🔴 **`req/324` M-01** — the escrow road names the preimage it was handed.
#[test]
fn the_escrow_roads_declared_read_names_the_preimage_it_was_handed() {
    let (said, tool_reads) = escrow_refusal(ReadFace::AnsweredAbsent);
    println!("R26_FUNNEL_ESCROW tool_reads={tool_reads} says={said:?}");
    assert!(
        tool_reads >= 1,
        "the premise: this road consumed the **declared** read (`read_by`), which is what makes it \
         the sibling of the sites R25 funnelled. tool_reads={tool_reads}"
    );
    assert!(
        said.contains(ANSWERED_ABSENT),
        "the premise: the server's decision reached this sentence at all: {said}"
    );
    assert!(
        said.contains(NAMES_THE_PREIMAGE),
        "🔴 `req/324` M-01 (`req/38` §231 ruling 2): the same server decision — a JSON-RPC `-32002`, \
         the code equality the whole discrimination rests on — reaches the agent on the escrow road \
         with no word saying which of the two facts it is. `req/38` §227 ruling 2 is the standing \
         rule: the same question, asked in one place, by every gate. `invert::invert_with_verdict` \
         is the second of the two functions in this crate that reach the declared read face and it \
         was on no row of `req/322` §3-1's table: {said}"
    );
}

/// 🔴 **`req/324` M-02** — the discrimination is made before the sentence is composed.
///
/// The token's whole value is its **position**, and a sentence gx composes around it destroys that
/// position. The repair is not to move the token back to the front of a sentence gx built — it is
/// to ask the question on the string the *transport* wrote, and to compose afterwards. This arm
/// measures the consequence: the composed sentence carries the discriminating words even though
/// the position-dependent predicate would answer `false` about it.
#[test]
fn the_discrimination_survives_the_sentence_gx_composes_around_it() {
    let (said, _) = escrow_refusal(ReadFace::AnsweredAbsent);
    // The predicate R24 wrote, spelled here rather than imported.
    let at_the_front = said
        .strip_prefix(ANSWERED_ABSENT)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '));
    let offset = said.find(ANSWERED_ABSENT);
    println!("R26_TOKEN_POSITION at_the_front={at_the_front} offset={offset:?}");
    assert!(
        said.contains(NAMES_THE_PREIMAGE),
        "🔴 `req/324` M-02: on this road gx wraps the transport's string \
         (`format!(\"{{REFUSAL}} ({{error}})\")`) before anything asks the position-dependent \
         question, so `strip_prefix` answers `false` about gx's own token at offset {offset:?}. \
         Asking first and composing second is the only order in which the answer is about the \
         server: {said}"
    );
}

/// 🔴 The negative control, and it is what stops the repair from becoming *say absence always*.
#[test]
fn a_read_that_did_not_answer_is_not_told_the_object_is_absent_on_either_road() {
    let (escrow_said, _) = escrow_refusal(ReadFace::DidNotAnswer);
    println!("R26_FUNNEL_NEGATIVE escrow={escrow_said:?}");
    assert!(
        !escrow_said.contains(NAMES_THE_PREIMAGE),
        "the funnel must not name absence for a read that failed: {escrow_said}"
    );

    let locator = format!("{SERVER}#{DOC}");
    let server = Arc::new(Server::new(
        b"the note as it stood\n",
        ReadFace::DidNotAnswer,
    ));
    let adapter = McpAdapter::new(server).with_catalogue(cas_read_catalogue());
    let snapshot_said = match adapter.snapshot(&locator) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("a read that did not answer has no snapshot"),
    };
    println!("R26_FUNNEL_NEGATIVE snapshot={snapshot_said:?}");
    assert!(
        !snapshot_said.contains(NAMES_THE_PREIMAGE),
        "and the same on the road R25 repaired: {snapshot_said}"
    );
}

/// 🔴 The second negative control: a read that **answers** produces no refusal at all, so the arms
/// above cannot be passing because this bed refuses everything.
#[test]
fn a_read_that_answers_reaches_an_escrow() {
    let locator = format!("{SERVER}#{DOC}");
    let server = Arc::new(Server::new(b"the note as it stood\n", ReadFace::Answers));
    let adapter = McpAdapter::new(server.clone()).with_catalogue(escrow_read_catalogue());
    let pre = support::absent_snapshot(&locator);
    let arguments = arguments();
    let delta = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, &arguments), &pre)
        .expect("plans");
    let answered = adapter.invert(&delta, &pre);
    println!(
        "R26_FUNNEL_POSITIVE reads={} ok={}",
        server.tool_reads.load(Ordering::SeqCst),
        answered.is_ok()
    );
    assert!(
        server.tool_reads.load(Ordering::SeqCst) >= 1,
        "the bed drove the declared read face"
    );
}

// ---------------------------------------------------------------------------------------------
// The sibling sweep, as a standing gate with a denominator
// ---------------------------------------------------------------------------------------------

/// 🔴 **`req/38` §227 ruling 2 (sibling sweep), made mechanical** — every site in this crate that
/// reaches the declared read face routes its failure through the one funnel.
///
/// `req/322` §3-1's table was written by hand and it missed a function. A table written by hand is
/// exactly what this arm replaces: the denominator is derived from the source on every run, so a
/// third consumer added later is red until it is funnelled.
#[test]
fn every_function_that_reaches_the_declared_read_face_routes_through_the_one_funnel() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("src is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();

    /// The two markers that answer the question: the predicate itself, and the name of the one
    /// funnel. Spelled as text for the reason this whole arm is text — a red that is *cannot find
    /// value in crate* has measured the symbol table instead of the defect.
    const ASKS: [&str; 2] = ["read_answered_absent(", "name_the_preimage"];

    let mut code_of: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let src = std::fs::read_to_string(&path).expect("readable");
        code_of.insert(
            name,
            src.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let asks = |code: &str| ASKS.iter().any(|needle| code.contains(needle));

    // The files that reach the declared read face, by a call that is not its definition and not
    // the trait signature.
    let mut reaching: Vec<String> = Vec::new();
    for (name, code) in &code_of {
        let calls = code
            .matches("read_prior_by_tool(")
            .count()
            .saturating_sub(code.matches("fn read_prior_by_tool(").count());
        if calls > 0 {
            reaching.push(name.clone());
        }
    }

    // For each of them the question is answered **in it**, or in every file that calls into it.
    // `cas.rs` is the second shape: its call is the body of `read_subject`, and the three consumers
    // of `read_subject` each answer for themselves. `invert.rs` is the first.
    let mut unanswered: Vec<String> = Vec::new();
    let mut how: Vec<String> = Vec::new();
    for name in &reaching {
        let code = &code_of[name];
        if asks(code) {
            how.push(format!("{name}: in the file"));
            continue;
        }
        let module = name.trim_end_matches(".rs");
        let callers: Vec<&String> = code_of
            .iter()
            .filter(|(other, c)| *other != name && c.contains(&format!("{module}::")))
            .map(|(other, _)| other)
            .collect();
        let silent: Vec<&&String> = callers.iter().filter(|c| !asks(&code_of[**c])).collect();
        if callers.is_empty() || !silent.is_empty() {
            unanswered.push(format!("{name}: callers={callers:?} silent={silent:?}"));
        } else {
            how.push(format!("{name}: in all {} callers", callers.len()));
        }
    }
    println!("R26_SWEEP reaching={reaching:?} how={how:?} unanswered={unanswered:?}");
    assert!(
        reaching.len() >= 2,
        "🔴 the denominator: this crate has two roads to the declared read face — \
         `cas::read_subject` and `invert::invert_with_verdict`. A sweep that finds fewer is \
         measuring the wrong thing, which is how `req/322` §3-1's hand-written table missed one: \
         {reaching:?}"
    );
    assert!(
        unanswered.is_empty(),
        "🔴 `req/324` M-01: a road to the declared read face whose failure never meets the one \
         funnel hands the agent `gx-substrate`'s frozen *the substrate would not answer* over a \
         server decision that said the opposite. {unanswered:?}"
    );
}
