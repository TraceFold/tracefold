// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/316` M-01 and M-02** (`req/317` §1 items 2 and 3, `req/38` §227 ruling 3) — how "the
//! server answered, and its answer is that there is nothing here" crosses the adapter boundary, and
//! the road that could not carry it at all.
//!
//! # What the twenty-third adversarial audit measured
//!
//! ```text
//! A23_INJECT ok=true object_bytes=Some(61) detail=""
//! A23_INJECT 🔴 an absence was signed over an object holding 61 bytes
//! A23_BITS_FP absent_answer = LTre3/EbYfFMiG41r6A2c23Nh6dNJ7XBUQIl0PWS4hM=   (-32002)
//! A23_BITS_FP tokenised     = LTre3/EbYfFMiG41r6A2c23Nh6dNJ7XBUQIl0PWS4hM=   (-32603 + token)
//! A23_WIRE_TOKEN total_mentions=2 inside_or_after_read_prior_by_tool=0
//! A23_DECLARED absent=Marked tools_only=true ok=false object_now=None not_answered=true  (×4)
//! ```
//!
//! **M-01.** The decision "the server answered absence" is a JSON-RPC **code equality** — `-32002`
//! and nothing else — and it is made in `gx-mcp-wire`. What was broken is how that decision crossed
//! the boundary: `gx_substrate::Error::Unreadable` is the frozen face N-08 pins, so the only channel
//! is `detail`, and the transport wrote `format!("{e} {TOKEN}")` where `{e}` carries the server's
//! own `message`. The predicate on the far side was `contains`. So a server that spelled those 74
//! characters into the message of **any** error got the absence fold back over a read that had
//! failed, with a postcondition fingerprint bit-identical to a real absence.
//!
//! **M-02.** `read_prior_by_tool` — the road every locator under a declared `$cas_read` prefix is
//! read by, and the only road a *tools-only* server has — never wrote the token at all. So on the
//! substrate class DR-46-16 exists for, a call that **removed** a resource could not be observed
//! under any wiring, and the refusal printed a remedy (*make the read face answer for this
//! locator*) that a reader cannot execute against an object that is gone.
//!
//! # Red-first
//!
//! No symbol this lane created is named; the token is spelled as a literal and the wire is measured
//! by reading its source, which is the arm the audit itself used to establish M-02.

mod support;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{Admitted, Catalogue, McpAdapter, ToolCall, ToolTransport};
use gx_substrate::{Error, Result, SubstrateAdapter};

use support::{absent_snapshot, intent_for};

const SERVER: &str = "stdio://r24";
const DOC: &str = "doc://page/1";
const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";
const AFTER: &[u8] = b"what the call put there\n";

/// The token a transport writes into `Unreadable`'s `detail` to mean "the server answered, and its
/// answer is that this locator holds nothing", spelled rather than imported so this file builds at
/// the branch point (`req/299` §0's rule).
const ANSWERED_ABSENT: &str =
    "[gx: the server answered, and its answer is that this locator holds nothing]";

/// How a read behaves after the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AfterTheCall {
    /// The read answers, and the object holds what the call put there.
    Answers,
    /// The server answered `-32002`, spelled the way `gx-mcp-wire` spells that answer.
    AnsweredAbsent,
    /// The read failed, and the server's own `message` carries the token gx uses for absence —
    /// the forgery `req/316` M-01 measured.
    FailedWithTheTokenInTheServersMessage,
    /// The read failed and said nothing about absence.
    Failed,
}

/// A server whose read face can be told what to do after the effect has landed.
struct Server {
    body: Mutex<Option<Vec<u8>>>,
    after: AfterTheCall,
    called: AtomicUsize,
    /// When set, `read` (the `resources/read` face) refuses outright: a *tools-only* server.
    tools_only: bool,
    tool_reads: AtomicUsize,
}

impl Server {
    fn new(body: &[u8], after: AfterTheCall) -> Self {
        Self {
            body: Mutex::new(Some(body.to_vec())),
            after,
            called: AtomicUsize::new(0),
            tools_only: false,
            tool_reads: AtomicUsize::new(0),
        }
    }

    fn tools_only(body: &[u8], after: AfterTheCall) -> Self {
        Self {
            tools_only: true,
            ..Self::new(body, after)
        }
    }

    fn now(&self) -> Option<Vec<u8>> {
        self.body.lock().expect("not poisoned").clone()
    }

    /// The answer both read faces give, so that the two roads differ only in which method carries
    /// it — which is the whole subject of M-02.
    fn answer(&self, locator: String) -> Result<Vec<u8>> {
        if self.called.load(Ordering::SeqCst) == 0 {
            return self.now().ok_or(Error::Unreadable {
                locator,
                detail: format!("{ANSWERED_ABSENT} this server holds nothing there."),
            });
        }
        match self.after {
            AfterTheCall::Answers => self.now().ok_or(Error::Unreadable {
                locator,
                detail: format!("{ANSWERED_ABSENT} this server holds nothing there."),
            }),
            AfterTheCall::AnsweredAbsent => Err(Error::Unreadable {
                locator,
                detail: format!("{ANSWERED_ABSENT} the MCP server refused: {{\"code\":-32002}}"),
            }),
            // The forgery: a `-32603` — "I could not tell you" — whose `message` happens to carry
            // gx's own token, which is what a transport composing `format!("{e} {TOKEN}")` puts in
            // one string with it.
            AfterTheCall::FailedWithTheTokenInTheServersMessage => Err(Error::Unreadable {
                locator,
                detail: format!(
                    "the MCP server refused: {{\"code\":-32603,\"message\":\"read failed \
                     {ANSWERED_ABSENT}\"}}"
                ),
            }),
            AfterTheCall::Failed => Err(Error::Unreadable {
                locator,
                detail: "the pipe to this server closed while the read was in flight".to_string(),
            }),
        }
    }
}

impl ToolTransport for Server {
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>> {
        let locator = format!("{server}#{resource}");
        if self.tools_only {
            return Err(Error::Unreadable {
                locator,
                detail: "this server publishes no `resources/read` face".to_string(),
            });
        }
        self.answer(locator)
    }

    fn read_prior_by_tool(&self, server: &str, tool: &str, _arguments: &[u8]) -> Result<Vec<u8>> {
        self.tool_reads.fetch_add(1, Ordering::SeqCst);
        self.answer(format!("{server}#tool:{tool}"))
    }

    fn call(&self, call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        self.called.fetch_add(1, Ordering::SeqCst);
        let arguments: serde_json::Value =
            serde_json::from_slice(call.arguments()).unwrap_or(serde_json::Value::Null);
        let body = arguments
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .as_bytes()
            .to_vec();
        *self.body.lock().expect("not poisoned") = Some(body);
        Ok(b"{\"ok\":true}".to_vec())
    }
}

fn apply_over(server: &Arc<Server>, catalogue: Catalogue) -> Result<gx_substrate::AppliedDelta> {
    let adapter =
        McpAdapter::new(server.clone() as Arc<dyn ToolTransport>).with_catalogue(catalogue);
    let locator = format!("{SERVER}#{DOC}");
    let arguments = br#"{"uri":"doc://page/1","contents":"what the call put there\n"}"#.to_vec();
    let pre = absent_snapshot(&locator);
    let delta = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, &arguments), &pre)
        .expect("a well-formed call plans");
    adapter.apply(&delta)
}

fn plain_catalogue() -> Catalogue {
    Catalogue::new().with_restore(WRITE_TOOL, RESTORE_TOOL)
}

fn declared_catalogue() -> Catalogue {
    let json = format!(
        r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
             "$cas_read": {{ "doc://": {{ "by_tool": "doc.read", "arguments": {{ "uri": "resource" }} }} }} }}"#
    );
    Catalogue::from_json(json.as_bytes()).expect("a well-formed declaration")
}

fn signed_absence(outcome: &Result<gx_substrate::AppliedDelta>) -> Option<bool> {
    let absent = gx_adapter_mcp::adapter::absent_digest();
    outcome
        .as_ref()
        .ok()
        .map(|applied| applied.resulting_digest() == &absent)
}

// ---------------------------------------------------------------------------
// M-01 — the token has a position the server cannot reach
// ---------------------------------------------------------------------------

/// 🔴 `req/316` M-01: a server that writes gx's own token into its error message does not get an
/// absence signed.
#[test]
fn a_failure_carrying_the_token_in_the_servers_message_is_not_signed_as_absence() {
    let server = Arc::new(Server::new(
        b"the note as it stood\n",
        AfterTheCall::FailedWithTheTokenInTheServersMessage,
    ));
    let outcome = apply_over(&server, plain_catalogue());
    let forged = signed_absence(&outcome);
    println!(
        "R24_FORGERY ok={} signed_absence={forged:?} object_now={:?}",
        outcome.is_ok(),
        server.now().as_deref().map(String::from_utf8_lossy)
    );
    assert_eq!(
        server.now().as_deref(),
        Some(AFTER),
        "the call was made and the object holds its bytes — this arm is about the record"
    );
    let why = match &outcome {
        Err(Error::Unreadable { detail, .. }) => detail.clone(),
        other => panic!(
            "🔴 `req/316` M-01: the read **failed** (`-32603` — 'I could not tell you') and gx \
             signed a postcondition of absence over an object holding its bytes, because the \
             token lived in the same string as the server's own message and the predicate was a \
             substring search. The fingerprint that produced was bit-identical to a genuine \
             absence, and the undo it blocks is refused `PRECONDITION_CHANGED` for ever over a \
             world that did not move. signed_absence={forged:?} outcome={other:?}"
        ),
    };
    assert!(
        why.contains("What to fix:"),
        "and the refusal carries a remedy: {why}"
    );
}

/// 🔴 The negative control, and the road the token exists for: the **genuine** answer still signs.
///
/// Without it, "never read the token" satisfies the arm above and takes the observation of a
/// removal with it.
#[test]
fn a_server_that_answered_absence_still_gets_an_absence_signed() {
    let server = Arc::new(Server::new(
        b"the note as it stood\n",
        AfterTheCall::AnsweredAbsent,
    ));
    let outcome = apply_over(&server, plain_catalogue());
    println!(
        "R24_GENUINE ok={} absent={:?}",
        outcome.is_ok(),
        signed_absence(&outcome)
    );
    let applied = outcome.expect(
        "a server that answered `-32002` has told this adapter a fact about the world, and the \
         postcondition of a removal is the absent digest",
    );
    assert_eq!(
        applied.resulting_digest(),
        &gx_adapter_mcp::adapter::absent_digest(),
        "the road R23 opened is unchanged; what moved is which spellings reach it"
    );
}

/// 🔴 The healthy road, unchanged: a read that answers signs what the object holds.
///
/// Without it, an arm that refuses everything and an arm that signs an absence for everything are
/// both satisfied by the three arms around it.
#[test]
fn a_read_that_answers_after_the_call_signs_what_the_object_holds() {
    let server = Arc::new(Server::new(
        b"the note as it stood
",
        AfterTheCall::Answers,
    ));
    let applied = apply_over(&server, plain_catalogue()).expect("the ordinary road is unchanged");
    let after = gx_adapter_mcp::adapter::content_digest(AFTER);
    println!(
        "R24_HEALTHY is_after={}",
        applied.resulting_digest() == &after
    );
    assert_eq!(
        applied.resulting_digest(),
        &after,
        "the postcondition is the read-back of what the call put there"
    );
}

/// 🔴 The plain failure, unchanged: no token, no absence.
#[test]
fn a_failure_that_says_nothing_about_absence_is_still_refused() {
    let server = Arc::new(Server::new(b"the note as it stood\n", AfterTheCall::Failed));
    let outcome = apply_over(&server, plain_catalogue());
    println!("R24_PLAIN_FAILURE ok={}", outcome.is_ok());
    assert!(
        matches!(outcome, Err(Error::Unreadable { .. })),
        "fail-closed is the default and it did not move: {outcome:?}"
    );
}

/// 🔴 The wire writes the token **first**, at every site that writes it.
///
/// # Why this is a source scan
///
/// The composition happens in `gx-mcp-wire`, one crate over, and what makes the predicate above
/// safe is not that it is strict — it is that gx is the only writer of position 0. A transport that
/// went on composing `format!("{e} {TOKEN}")` would be a transport whose genuine absences stop
/// being read, which is fail-closed and therefore silent. This arm is what makes it loud.
#[test]
fn every_site_that_writes_the_token_writes_it_first() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-mcp-wire/src/client.rs");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("client.rs readable: {e}"));
    let leading = src.matches("format!(\"{} {e}\"").count();
    let trailing = src.matches("format!(\"{e} {}\"").count();
    println!("R24_WIRE_POSITION token_first={leading} token_last={trailing}");
    assert!(
        src.contains("READ_ANSWERED_ABSENT"),
        "the scan is looking at the file it thinks it is (`req/316` §5 self-admission 2)"
    );
    assert_eq!(
        trailing, 0,
        "🔴 `req/316` M-01: a site still composes the server's own words **before** gx's token. \
         The server controls the text in front of it, and the predicate on the far side asks for \
         position 0"
    );
    assert!(
        leading >= 2,
        "🔴 and both read roads write it: `resources/read` and the declared tool face. \
         token_first={leading}"
    );
}

// ---------------------------------------------------------------------------
// M-02 — the declared road can say "the server answered that it is gone"
// ---------------------------------------------------------------------------

/// 🔴 `req/316` M-02: the road a tools-only deployment reads by carries the absence answer.
///
/// The audit's four wirings all refused, and the reason was structural: the crate-wide scan found
/// the token mentioned twice in `gx-mcp-wire` and **zero** times inside or after
/// `fn read_prior_by_tool`. This is that scan, kept.
#[test]
fn the_declared_read_road_has_a_branch_that_marks_an_absence() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-mcp-wire/src/client.rs");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("client.rs readable: {e}"));
    let total = src.matches("READ_ANSWERED_ABSENT").count();
    let start = src
        .find("fn read_prior_by_tool")
        .expect("`read_prior_by_tool` is in this file");
    let after = src[start..].matches("READ_ANSWERED_ABSENT").count();
    println!("R24_WIRE_TOKEN total_mentions={total} inside_or_after_read_prior_by_tool={after}");
    assert!(
        total >= 2,
        "the scan is looking at the file it thinks it is: total={total}"
    );
    assert!(
        after >= 1,
        "🔴 `req/316` M-02: `read_prior_by_tool` writes no absence marker, so on a server that \
         publishes no `resources/read` — the substrate class DR-46-16 exists for — a call that \
         **removed** a resource cannot be observed under any wiring. Four were measured and all \
         four refused, and the remedy the refusal printed asked the reader to make the read face \
         answer for an object that is gone"
    );
}

/// 🔴 And the behaviour, over the declared road, with a transport faithful to the repaired wire.
#[test]
fn a_removal_observed_through_a_declared_read_face_is_signed_as_an_absence() {
    let server = Arc::new(Server::tools_only(
        b"the note as it stood\n",
        AfterTheCall::AnsweredAbsent,
    ));
    let outcome = apply_over(&server, declared_catalogue());
    println!(
        "R24_DECLARED_ABSENCE ok={} absent={:?} tool_reads={}",
        outcome.is_ok(),
        signed_absence(&outcome),
        server.tool_reads.load(Ordering::SeqCst)
    );
    assert!(
        server.tool_reads.load(Ordering::SeqCst) >= 1,
        "the declared face is the road this arm is about: a run through `resources/read` would \
         satisfy the assertion below for the wrong reason"
    );
    let applied = outcome.expect(
        "a tools-only server that answers `-32002` through its declared read face has said the \
         object is gone, and that is what a removal's postcondition is",
    );
    assert_eq!(
        applied.resulting_digest(),
        &gx_adapter_mcp::adapter::absent_digest(),
        "the same digest the `resources/read` road produces for the same fact"
    );
}

/// 🔴 The control that keeps M-02's repair fail-closed: on the declared road too, a failure that
/// did not answer is refused rather than signed.
#[test]
fn a_declared_read_that_failed_is_still_refused_on_the_declared_road() {
    for (what, after) in [
        ("plain failure", AfterTheCall::Failed),
        (
            "forged token",
            AfterTheCall::FailedWithTheTokenInTheServersMessage,
        ),
    ] {
        let server = Arc::new(Server::tools_only(b"the note as it stood\n", after));
        let outcome = apply_over(&server, declared_catalogue());
        println!(
            "R24_DECLARED_FAILCLOSED {what}: ok={} absent={:?}",
            outcome.is_ok(),
            signed_absence(&outcome)
        );
        assert!(
            matches!(outcome, Err(Error::Unreadable { .. })),
            "🔴 the branch M-02 adds is the **code equality** and nothing wider: {what}: {outcome:?}"
        );
    }
}
