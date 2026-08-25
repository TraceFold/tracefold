// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/279` H-01 / M-01 / M-03 / M-04 / L-03 / L-04** (`req/283`, `req/38` §203) — the
//! declarations a catalogue can make that gx used to believe.
//!
//! # What the nineteenth adversarial audit measured, and what each arm here fixes in place
//!
//! R17 closed "**which** object is this the prior of" (DR-46-15). The nineteenth audit went
//! around it, and every finding below needs no adversary — a deployment writing its own catalogue
//! reaches all six by hand:
//!
//! * **H-01** — a `read_by` declaration with **no restore template** falls back to v0.1's
//!   `{contents, uri}` convention, and on that road the "prior contents" gx escrows are the read
//!   tool's whole **answer document**. The identity check passes, because the answer really is
//!   about the right object. Measured: `verdict=true`, and an undo that wrote
//!   `{"id":"doc:d1","text":"…","etag":"w/123"}` into the object where `the document as it was\n`
//!   had been. DR-46-15 asked "which object is this the prior of"; nobody asked "is this a prior".
//! * **M-04** — a catalogue may name, as its **read face**, a tool the same file declares as an
//!   **effect**. gx then calls it through the escrow road, which takes no `Admitted`, twice per
//!   forward call, before `apply`. Measured: the write landed on the server under a verdict that
//!   said the effect was refused.
//! * **L-03** — `"by_tool": ""` parsed, and an empty tool name went to the wire.
//! * **M-03** — `ObjectIdentity::soundness` is syntactic: it asks whether an `answer` part is
//!   *present*. A member the server always answers empty (`{"answer": "/pad"}`) satisfies that and
//!   contributes nothing, so the spelling comes from the forward call alone and the answer is
//!   never checked. Measured: `verdict=true` and an escrow carrying a stranger's text.
//! * **M-01** — the five shapes where the answer cannot be read at all (not JSON, not UTF-8,
//!   empty, nested past the parser's limit, pointer absent) were refused with DR-46-15's sentence,
//!   which says the read *named another object*. It named none. The remedy it printed — "point
//!   the change at the object the read answers for" — cannot be executed when nothing was read.
//! * **L-04** — the spelling the identity produced was interpolated into the refusal unbounded. A
//!   1 MiB `id` produced a 1,049,118-byte refusal, which goes to the agent's tool result and to
//!   the operator's terminal.
//!
//! # The denominator of this suite, stated here rather than found later
//!
//! * **In-process only.** No live MCP server, no `gx wrap`, no HTTP. The wrap-level run that shows
//!   an H-01 catalogue failing to start a session is a command-line measurement recorded in
//!   `req/285`, not a probe here — `crates/gx-cli` is another lane's write scope.
//! * **The `identity` value space is not re-enumerated.** `tests/r17_attested_object_binding.rs`
//!   and the audit's own arms cover the JSON-type and pointer spaces; these arms take one witness
//!   per finding plus its negative control.
//! * **The restore call's own target is still unbound.** Every arm here is about the *escrowed
//!   bytes*; what a restore tool does with the arguments it is handed remains the server's, and
//!   `docs/LIMITS.md` says so.

mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    Admitted, ArgSource, Catalogue, IdentityPart, McpAdapter, McpDelta, ObjectIdentity,
    OnReadFailure, PriorRead, RestoreTemplate, Reversibility, ToolCall, ToolTransport,
    DECLARATION_UNSOUND_REFUSAL, IDENTITY_IGNORES_THE_ANSWER_REFUSAL, OBJECT_IDENTITY_REFUSAL,
    OBJECT_UNNAMED_REFUSAL,
};
use gx_substrate::{Error, PlannedDelta, Result, SubstrateAdapter};

use support::{absent_snapshot, intent_for};

const SERVER: &str = "stdio://r18";
/// The object every change in this suite is about.
const DOC: &str = "doc:d1";
/// What the object held before the change, and what a real prior would carry.
const BEFORE: &str = "the document as it was\n";

fn locator() -> String {
    format!("{SERVER}#{DOC}")
}

fn forward_arguments() -> Vec<u8> {
    br#"{"id":"doc:d1","text":"the document as the agent wants it\n"}"#.to_vec()
}

// ---------------------------------------------------------------------------
// The fixture: one server, one read face, and an answer the arm chooses
// ---------------------------------------------------------------------------

/// A server whose read face answers exactly what the arm put there, byte for byte.
///
/// Deliberately dumber than `r17_attested_object_binding.rs`'s: these arms are about what gx
/// *believes* a declaration says, so the server has to be able to answer things a real one would
/// answer badly — not JSON, not UTF-8, nothing at all.
#[derive(Debug)]
struct R18Server {
    /// What `read_prior_by_tool` returns, whatever it is asked.
    answer: Mutex<Vec<u8>>,
    /// What `resources/read` holds, for the road that takes it.
    resources: Mutex<BTreeMap<String, Vec<u8>>>,
    /// Every arrival, so that "gx called nothing" is measured rather than assumed.
    arrivals: Mutex<Vec<String>>,
}

impl R18Server {
    fn answering(answer: impl Into<Vec<u8>>) -> Arc<Self> {
        let mut resources = BTreeMap::new();
        resources.insert(DOC.to_string(), BEFORE.as_bytes().to_vec());
        Arc::new(Self {
            answer: Mutex::new(answer.into()),
            resources: Mutex::new(resources),
            arrivals: Mutex::new(Vec::new()),
        })
    }

    fn arrivals(&self) -> Vec<String> {
        self.arrivals.lock().expect("not poisoned").clone()
    }

    fn note(&self, what: impl Into<String>) {
        self.arrivals
            .lock()
            .expect("not poisoned")
            .push(what.into());
    }
}

impl ToolTransport for R18Server {
    fn read(&self, _server: &str, resource: &str) -> Result<Vec<u8>> {
        self.note(format!("read {resource}"));
        self.resources
            .lock()
            .expect("not poisoned")
            .get(resource)
            .cloned()
            .ok_or_else(|| Error::Unreadable {
                locator: format!("{SERVER}#{resource}"),
                detail: "this server publishes no resource at that URI".to_string(),
            })
    }

    fn read_prior_by_tool(&self, _server: &str, tool: &str, arguments: &[u8]) -> Result<Vec<u8>> {
        self.note(format!(
            "read_by_tool {tool} {}",
            String::from_utf8_lossy(arguments)
        ));
        Ok(self.answer.lock().expect("not poisoned").clone())
    }

    fn call(&self, call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        self.note(format!("call {}", call.tool()));
        Ok(b"{}".to_vec())
    }
}

/// Plan the one forward call every arm makes, and ask this deployment whether it is reversible.
fn verdict(
    catalogue: Catalogue,
    server: &Arc<R18Server>,
) -> (
    McpAdapter,
    PlannedDelta,
    Result<(Reversibility, Option<PlannedDelta>)>,
) {
    let adapter =
        McpAdapter::new(server.clone() as Arc<dyn ToolTransport>).with_catalogue(catalogue);
    let pre = absent_snapshot(&locator());
    let delta = adapter
        .plan(
            &intent_for(&locator(), "doc.write", &forward_arguments()),
            &pre,
        )
        .expect("a well-formed call plans");
    let reversibility = adapter.reversibility(&delta, &pre);
    // 🔴 **DR-46-26** — this probe pairs `reversibility` with the inverse; the trait road now
    // carries both, and the `Option` this arm compares is the `inverse` projection of it.
    let inverse = adapter
        .invert(&delta, &pre)
        .map(gx_substrate::InvertOutcome::into_inverse);
    let answer = match (reversibility, inverse) {
        (Ok(verdict), Ok(inverse)) => Ok((verdict, inverse)),
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    (adapter, delta, answer)
}

fn refusal_of(answer: &Result<(Reversibility, Option<PlannedDelta>)>) -> String {
    match answer {
        Err(Error::Unreadable { detail, .. }) => detail.clone(),
        other => panic!("this arm refuses; it answered {other:?}"),
    }
}

/// The identity `["doc:", {"answer": "/id"}]` — the sound one every negative control carries.
fn sound_identity() -> ObjectIdentity {
    ObjectIdentity::new(vec![
        IdentityPart::Literal("doc:".to_string()),
        IdentityPart::Answer {
            answer: "/id".to_string(),
        },
    ])
}

/// The restore template a sound declaration carries: put the prior text back at the same id.
fn restore_template() -> RestoreTemplate {
    RestoreTemplate::new()
        .with("id", ArgSource::Forward("id".to_string()))
        .with("text", ArgSource::PriorContentsUtf8)
}

fn sound_read() -> PriorRead {
    PriorRead::new(
        "doc.get",
        RestoreTemplate::new().with("id", ArgSource::Forward("id".to_string())),
        sound_identity(),
    )
}

/// A catalogue in the shape a deployment file takes, so the parse road is the road measured.
fn parse(json: &str) -> core::result::Result<Catalogue, String> {
    Catalogue::from_json(json.as_bytes())
}

// ---------------------------------------------------------------------------
// H-01 — a read declaration that is not a prior declaration
// ---------------------------------------------------------------------------

/// 🔴 **`req/279` H-01** — `read_by` without a restore template does not parse.
///
/// The audit's `n2` catalogue, verbatim in shape. Under v0.5-e it parsed, started a session, and
/// answered `true` while escrowing the read tool's answer **document** as the object's prior.
#[test]
fn h01_a_read_declaration_without_a_restore_template_does_not_parse() {
    let n2 = r#"{
      "doc.write": {
        "restored_by": "doc.restore",
        "read_by": {
          "by_tool": "doc.get",
          "arguments": { "id": { "forward": "id" } },
          "identity": [ "doc:", { "answer": "/id" } ]
        }
      }
    }"#;
    let why = parse(n2).expect_err("`req/279` H-01: this catalogue must not start a session");
    println!("R18_H01_PARSE_REFUSAL={why}");
    assert!(
        why.contains("doc.write") && why.contains("arguments"),
        "the refusal names the entry and the member that is missing: {why}"
    );

    // Negative control 1: the same declaration **with** a template is the shape that works, and
    // this lane did not narrow it.
    let sound = r#"{
      "doc.write": {
        "restored_by": "doc.restore",
        "arguments": { "id": { "forward": "id" }, "text": "prior_contents_utf8" },
        "read_by": {
          "by_tool": "doc.get",
          "arguments": { "id": { "forward": "id" } },
          "identity": [ "doc:", { "answer": "/id" } ]
        }
      }
    }"#;
    parse(sound).expect("a read declaration with a template is unchanged");

    // Negative control 2: the v0.1 `{contents, uri}` form — no template, **no read** — is every
    // catalogue written before this road existed, and it still parses. H-01 is about the pair.
    parse(r#"{ "notes.write": { "restored_by": "notes.restore" } }"#)
        .expect("the v0.1 convention is untouched");
}

/// 🔴 **`req/279` H-01, the code road** — `Catalogue::with_prior_read` on a declaration with no
/// template refuses at the escrow rather than escrowing the answer document.
///
/// `req/269` M-05's argument, applied one finding over: a catalogue built in code never went
/// through a parser, so if the parse check were the only one, this shape would still ship inside
/// a binary. The measurement is that the refusal is a **declaration** refusal and the read face
/// was never called.
#[test]
fn h01_the_code_built_road_refuses_rather_than_escrowing_the_answer_document() {
    // The answer document `doc.get` returns, `id` and all — the bytes v0.5-e escrowed **whole**
    // as the object's "prior contents", and the bytes an undo then wrote into the object.
    let server = R18Server::answering(
        br#"{"id":"d1","text":"the document as it was\n","etag":"w/123"}"#.to_vec(),
    );
    let catalogue = Catalogue::new()
        // `with_restore` is the v0.1 form: no template.
        .with_restore("doc.write", "doc.restore")
        .with_prior_read("doc.write", sound_read());
    let (_adapter, _delta, answer) = verdict(catalogue, &server);
    let refusal = refusal_of(&answer);
    println!("R18_H01_CODE_ROAD_REFUSAL={refusal}");
    assert!(
        refusal.contains(DECLARATION_UNSOUND_REFUSAL),
        "a declaration fault speaks in its own voice: {refusal}"
    );
    assert_eq!(
        server.arrivals(),
        Vec::<String>::new(),
        "`req/269` M-05: nothing is called for a declaration that cannot be right"
    );
}

// ---------------------------------------------------------------------------
// M-04 — a read face the same file declares as an effect
// ---------------------------------------------------------------------------

/// 🔴 **`req/279` M-04** — `by_tool` may not be an entry key of the same catalogue.
///
/// The audit's `s2`: `{"doc.write": {…, "read_by": {"by_tool": "doc.write"}}}` parsed, and gx
/// then called `doc.write` twice per forward call through a road that takes no `Admitted` — so
/// the server was written by the escrow of a call whose verdict said the effect was refused.
#[test]
fn m04_a_read_face_that_is_a_declared_effect_does_not_parse() {
    let s2 = r#"{
      "doc.write": {
        "restored_by": "doc.restore",
        "arguments": { "id": { "forward": "id" }, "text": "prior_contents_utf8" },
        "read_by": {
          "by_tool": "doc.write",
          "arguments": { "id": { "forward": "id" } },
          "identity": [ "doc:", { "answer": "/id" } ]
        }
      }
    }"#;
    let why = parse(s2).expect_err("`req/279` M-04: an effect is not a read face");
    println!("R18_M04_PARSE_REFUSAL={why}");
    assert!(
        why.contains("doc.write"),
        "the refusal names the tool that is on both sides: {why}"
    );

    // The same file, with the read face pointed at a tool it does **not** declare as an effect.
    let sound = s2.replace("\"by_tool\": \"doc.write\"", "\"by_tool\": \"doc.get\"");
    parse(&sound).expect("a read face that is not a declared effect is unchanged");

    // And the cross shape: a second entry's key is just as much an effect as this entry's own.
    let cross = r#"{
      "doc.write": {
        "restored_by": "doc.restore",
        "arguments": { "id": { "forward": "id" }, "text": "prior_contents_utf8" },
        "read_by": {
          "by_tool": "doc.append",
          "arguments": { "id": { "forward": "id" } },
          "identity": [ "doc:", { "answer": "/id" } ]
        }
      },
      "doc.append": { "restored_by": "doc.restore" }
    }"#;
    let why = parse(cross).expect_err("`req/279` M-04: any entry key is a declared effect");
    println!("R18_M04_CROSS_REFUSAL={why}");
    assert!(why.contains("doc.append"), "{why}");
}

// ---------------------------------------------------------------------------
// L-03 — a read face with no name
// ---------------------------------------------------------------------------

/// 🔴 **`req/279` L-03** — `"by_tool": ""` does not parse, and neither does whitespace.
///
/// The audit measured `arrivals=["read_by_tool  {}"]` — an empty tool name on the wire. The harm
/// is small (the server answers `-32602`), and the point is where the check belongs: R17 put
/// "what can be known before a call is known before the call" at parse time, and this is one line
/// in that same place.
#[test]
fn l03_a_read_face_with_no_name_does_not_parse() {
    for spelling in ["", "   ", "\t"] {
        let json = format!(
            r#"{{
              "doc.write": {{
                "restored_by": "doc.restore",
                "arguments": {{ "id": {{ "forward": "id" }}, "text": "prior_contents_utf8" }},
                "read_by": {{
                  "by_tool": {},
                  "arguments": {{ "id": {{ "forward": "id" }} }},
                  "identity": [ "doc:", {{ "answer": "/id" }} ]
                }}
              }}
            }}"#,
            serde_json::Value::String(spelling.to_string())
        );
        let why = parse(&json).map(|c| c.declared()).expect_err(&format!(
            "`req/279` L-03: {spelling:?} is not a tool name and must not parse"
        ));
        println!("R18_L03 spelling={spelling:?} refusal={why}");
        assert!(
            why.contains("doc.write"),
            "the refusal names the entry it is about: {why}"
        );
    }
}

// ---------------------------------------------------------------------------
// M-03 — an identity that names the answer without reading it
// ---------------------------------------------------------------------------

/// 🔴 **`req/279` M-03** — an `answer` part that spells nothing does not bind anything.
///
/// The audit's `n3`: `identity = [{"forward":"id"}, {"answer":"/pad"}]` against a server whose
/// `/pad` is always `""`. The parse-time check passes (an `answer` part is present), the spelling
/// equals the locator (it came from the forward call), and gx answered `true` while escrowing
/// `{"id":"doc:d1","text":"a stranger's text"}`.
#[test]
fn m03_an_identity_whose_answer_spells_nothing_is_refused_not_escrowed() {
    let server = R18Server::answering(
        br#"{"id":"doc:SOMEONE-ELSES-DOC","pad":"","text":"a stranger's text"}"#.to_vec(),
    );
    let padded = ObjectIdentity::new(vec![
        IdentityPart::Forward {
            forward: "id".to_string(),
        },
        IdentityPart::Answer {
            answer: "/pad".to_string(),
        },
    ]);
    let catalogue = Catalogue::new()
        .with_restore_template("doc.write", "doc.restore", restore_template())
        .with_prior_read(
            "doc.write",
            PriorRead::new(
                "doc.get",
                RestoreTemplate::new().with("id", ArgSource::Forward("id".to_string())),
                padded,
            ),
        );
    let (_adapter, _delta, answer) = verdict(catalogue, &server);
    let refusal = refusal_of(&answer);
    println!("R18_M03_REFUSAL={refusal}");
    assert!(
        refusal.contains(IDENTITY_IGNORES_THE_ANSWER_REFUSAL),
        "the refusal says the answer was never read, in its own words: {refusal}"
    );
    assert!(
        !refusal.contains(OBJECT_IDENTITY_REFUSAL),
        "this is not \"a different object\" — the identity read no object at all: {refusal}"
    );
}

// ---------------------------------------------------------------------------
// M-01 — an answer that could not be read is not an answer about another object
// ---------------------------------------------------------------------------

/// 🔴 **`req/279` M-01** — the five unreadable shapes stop borrowing DR-46-15's sentence.
///
/// `req/269` M-05 was "a declaration mistake wearing a server failure's face". This is the same
/// species one gate over: an answer nobody could read wearing the face of an answer about someone
/// else, with a remedy ("point the change at the object the read answers for") that cannot be
/// executed because no object was named.
#[test]
fn m01_an_answer_that_could_not_be_read_is_not_told_it_named_another_object() {
    let shapes: Vec<(&str, Vec<u8>)> = vec![
        ("not_json", b"<html>rate limited</html>".to_vec()),
        ("non_utf8", vec![b'{', b'"', b'i', 0xff, b'"', b'}']),
        ("empty", Vec::new()),
        ("deep", {
            let mut deep = vec![b'['; 600];
            deep.extend(vec![b']'; 600]);
            deep
        }),
        (
            "pointer_absent",
            br#"{"identifier":"doc:d1","text":"no `id` member here"}"#.to_vec(),
        ),
    ];
    for (shape, answer) in shapes {
        let server = R18Server::answering(answer);
        let catalogue = Catalogue::new()
            .with_restore_template("doc.write", "doc.restore", restore_template())
            .with_prior_read("doc.write", sound_read());
        let (_adapter, _delta, verdicted) = verdict(catalogue, &server);
        let refusal = refusal_of(&verdicted);
        println!("R18_M01 shape={shape} refusal={refusal}");
        assert!(
            !refusal.contains(OBJECT_IDENTITY_REFUSAL),
            "shape={shape}: nothing here established that the read named another object: {refusal}"
        );
        assert!(
            !refusal.contains("point the change at the object the read answers for"),
            "shape={shape}: a remedy nobody can execute is not a remedy: {refusal}"
        );
        assert!(
            refusal.contains(OBJECT_UNNAMED_REFUSAL),
            "shape={shape}: the cause that did happen has its own sentence: {refusal}"
        );
    }

    // The negative control, and the whole point of splitting the two: a read that **did** name an
    // object, and named the wrong one, still carries DR-46-15's sentence verbatim.
    let server =
        R18Server::answering(br#"{"id":"SOMEONE-ELSE","text":"a stranger's text"}"#.to_vec());
    let catalogue = Catalogue::new()
        .with_restore_template("doc.write", "doc.restore", restore_template())
        .with_prior_read("doc.write", sound_read());
    let (_adapter, _delta, verdicted) = verdict(catalogue, &server);
    let refusal = refusal_of(&verdicted);
    println!("R18_M01 shape=mismatch refusal={refusal}");
    assert!(
        refusal.contains(OBJECT_IDENTITY_REFUSAL),
        "a read that named the wrong object is still DR-46-15's refusal: {refusal}"
    );
}

// ---------------------------------------------------------------------------
// L-04 — a refusal is a sentence, not a payload
// ---------------------------------------------------------------------------

/// 🔴 **`req/279` L-04** — the spelling an identity produced is bounded before it is printed.
///
/// Measured at v0.5-e: a 1 MiB `id` produced a **1,049,118-byte** refusal, which travels to the
/// agent's tool result and to the operator's terminal.
#[test]
fn l04_the_refusal_is_bounded_when_the_answer_spells_a_megabyte() {
    let huge = "a".repeat(1024 * 1024);
    let answer = serde_json::json!({ "id": huge, "text": "a stranger's text" }).to_string();
    let server = R18Server::answering(answer.into_bytes());
    let catalogue = Catalogue::new()
        .with_restore_template("doc.write", "doc.restore", restore_template())
        .with_prior_read("doc.write", sound_read());
    let (_adapter, _delta, verdicted) = verdict(catalogue, &server);
    let refusal = refusal_of(&verdicted);
    println!("R18_L04_REFUSAL_BYTES={}", refusal.len());
    println!(
        "R18_L04_REFUSAL_HEAD={}",
        refusal.chars().take(320).collect::<String>()
    );
    assert!(
        refusal.len() < 2048,
        "a refusal carries a sentence, not the server's payload: {} bytes",
        refusal.len()
    );
    assert!(
        refusal.contains(OBJECT_IDENTITY_REFUSAL),
        "bounding the interpolation does not change the constant: {refusal}"
    );
    assert!(
        refusal.contains("...(1048580 bytes)"),
        "the elision says how much was dropped, so nothing is silently lost: {refusal}"
    );
}

// ---------------------------------------------------------------------------
// The control every arm above is a discrimination against
// ---------------------------------------------------------------------------

/// A sound declaration still escrows, byte for byte, and the posture still moves the answer.
///
/// Without this arm every assertion above is satisfiable by an adapter that refuses everything.
#[test]
fn a_sound_declaration_still_escrows_and_the_posture_still_moves_the_answer() {
    let server = R18Server::answering(
        serde_json::json!({ "id": "d1", "text": BEFORE })
            .to_string()
            .into_bytes(),
    );
    let catalogue = Catalogue::new()
        .with_restore_template("doc.write", "doc.restore", restore_template())
        .with_prior_read("doc.write", sound_read());
    let (_adapter, _delta, verdicted) = verdict(catalogue, &server);
    let (reversibility, inverse) = verdicted.expect("a sound declaration is not refused");
    assert_eq!(reversibility, Reversibility::True);
    let inverse = inverse.expect("a sound declaration escrows");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter wrote it");
    let op = decoded.ops().first().expect("one op");
    println!(
        "R18_CONTROL restore_tool={} arguments={}",
        op.tool(),
        String::from_utf8_lossy(op.arguments())
    );
    assert_eq!(op.tool(), "doc.restore");

    // And the relaxation still reaches the identity gate (`req/269` M-02's window, unchanged by
    // this lane): the same mismatch that refuses above answers `unknown` under `"unknown"`.
    let server = R18Server::answering(br#"{"id":"SOMEONE-ELSE","text":"x"}"#.to_vec());
    let catalogue = Catalogue::new()
        .with_restore_template("doc.write", "doc.restore", restore_template())
        .with_prior_read("doc.write", sound_read())
        .with_on_read_failure(OnReadFailure::Unknown);
    let (_adapter, _delta, verdicted) = verdict(catalogue, &server);
    let (reversibility, inverse) = verdicted.expect("the relaxation takes the effect");
    println!("R18_CONTROL posture=unknown verdict={reversibility:?}");
    assert_eq!(reversibility, Reversibility::Unknown);
    assert!(inverse.is_none());
}
