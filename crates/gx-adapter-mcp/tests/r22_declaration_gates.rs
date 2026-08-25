// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/303` H-01, M-03 and L-05, on the parse road** (`req/309` §1 items 1, 4 and 8).
//!
//! # What the twenty-first adversarial audit measured, verbatim
//!
//! ```text
//! A21_PARSE a6_gitblob_residual = ACCEPTED (declared=1)
//! A21_RESIDUAL_GITBLOB verdict=Admit after_write="what the agent wrote through gx wrap\n"
//!                      undo_rc=0 after_undo=""
//! A21_FACE blank restored_by => the read declaration of entry "notes.write" is not sound: ...
//! A21_PARSE a1_i_forward_only = REFUSED (the read declaration of entry "notes.write" is not sound: ...)
//! ```
//!
//! Three findings, one file, because all three are answered by `Catalogue::from_json` before a
//! session exists.
//!
//! * **H-01.** R20 widened DR-46-19's gate to "not a function of the forward call alone" and spelled
//!   it `!matches!(source, ArgSource::Forward(_))`. `git_blob_sha1_of_forward` is not `Forward`, and
//!   its own doc comment says the value is *"computable before [the forward call], from the forward
//!   arguments alone"* — so a template of forward members plus a hash of one of them passed a gate
//!   whose printed sentence is the exact claim it violates. Measured end to end: `Admit`, a signed
//!   commit, and the undo gx printed emptied the object with `rc=0`.
//! * **M-03.** Every entry fault was wrapped in *"the **read declaration** of entry X is not
//!   sound"*, including faults about a `restored_by` and about an `arguments` template, in
//!   catalogues that declare no read face anywhere.
//! * **L-05.** `écrire` (U+00E9) and `e` + U+0301 in one file gave `declared=2` — two declarations
//!   a reader cannot tell apart.
//!
//! # Red-first, and why the arms name no symbol this lane invented
//!
//! `req/299` §0 made "a compile error is not a measurement" a standing rule: a suite naming a symbol
//! R22 created cannot be built against `97052f8` at all, and its red would be *cannot find value*,
//! which is equally true of a file measuring nothing. Every arm below therefore drives
//! `Catalogue::from_json` and reads the **sentence**, so the whole file compiles at the branch point
//! and goes red where the defect is. The one exception is
//! [`the_gate_is_a_positive_set_with_a_fail_closed_default`], which reads the shipped source; it is a
//! **seam** rather than a measurement and says so.

mod support;

use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    Admitted, ArgSource, Catalogue, McpAdapter, PriorRead, RestoreTemplate, ToolCall, ToolTransport,
};
use gx_substrate::{Error, Result as SubstrateResult, SubstrateAdapter};
use serde_json::{json, Value};

use support::{absent_snapshot, intent_for};

/// Parse a catalogue from a JSON value the way `gx wrap --restore-catalogue` parses a file.
fn parse(body: &Value) -> Result<Catalogue, String> {
    Catalogue::from_json(&serde_json::to_vec(body).expect("json"))
}

/// The audit's own pair, with `notes.write`'s template supplied by the arm.
fn body(write_arguments: Value) -> Value {
    json!({
        "notes.write": { "restored_by": "notes.restore", "arguments": write_arguments },
        "notes.restore": {
            "restored_by": "notes.write",
            "arguments": { "uri": { "forward": "uri" }, "contents": "prior_contents_utf8" }
        }
    })
}

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// H-01 — the gate is a positive set
// ---------------------------------------------------------------------------

/// 🔴 `req/303` H-01: the audit's catalogue, verbatim. A template of forward members and the git
/// blob hash of one of them is a function of the forward call alone, and it must not parse.
#[test]
fn a_template_of_forward_members_and_their_hash_never_reaches_a_session() {
    let catalogue = body(json!({
        "uri": { "forward": "uri" },
        "sha": { "git_blob_sha1_of_forward": "contents" },
    }));
    let answer = parse(&catalogue);
    println!(
        "R22_H01 accepted={} detail={:?}",
        answer.is_ok(),
        answer.as_ref().err()
    );
    let why = answer.err().unwrap_or_else(|| {
        panic!(
            "🔴 `req/303` H-01: this catalogue parsed, answered `true`, signed a commit, and the \
             undo gx printed emptied the object with rc 0. Every member of its template is \
             computable from the forward call — `git_blob_sha1_of_forward`'s own doc comment says \
             so — which is the exact sentence the gate prints when it refuses"
        )
    });
    assert!(
        why.contains("is a function of the forward call alone"),
        "and it is refused by the template gate rather than by something else: {why}"
    );
}

/// 🔴 The classification, one variant at a time, asked through the parser rather than through the
/// predicate — so this arm compiles at the branch point.
///
/// Each row is a template of **exactly one** member beside no others. If the member draws on
/// something the forward call does not carry the catalogue parses; if it does not, the same gate
/// refuses it. `Forward` and `git_blob_sha1_of_forward` are the two that do not.
#[test]
fn each_word_of_the_vocabulary_answers_the_gate_as_its_own_doc_comment_says() {
    let rows: &[(&str, Value, bool)] = &[
        ("forward", json!({ "c": { "forward": "contents" } }), false),
        (
            "git_blob_sha1_of_forward",
            json!({ "c": { "git_blob_sha1_of_forward": "contents" } }),
            false,
        ),
        (
            "prior_contents_utf8",
            json!({ "c": "prior_contents_utf8" }),
            true,
        ),
        (
            "prior_json",
            json!({ "c": { "prior_json": "/body" } }),
            true,
        ),
        ("const", json!({ "c": { "const": "gx undo" } }), true),
        ("const_json", json!({ "c": { "const_json": false } }), true),
        ("do_result", json!({ "c": { "do_result": "/id" } }), true),
        (
            "do_result_number_from",
            json!({ "c": { "do_result_number_from": "/url" } }),
            true,
        ),
    ];
    let mut wrong: Vec<String> = Vec::new();
    for (word, template, draws_from_outside) in rows {
        let accepted = parse(&body(template.clone())).is_ok();
        println!("R22_WORD {word} accepted={accepted} expected={draws_from_outside}");
        if accepted != *draws_from_outside {
            wrong.push(format!("{word}: accepted={accepted}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "🔴 `req/303` H-01 / `req/38` §221 ruling 1: the gate must be the **positive** set — which \
         words draw on something the forward call does not carry — and one arm per word. \
         Disagreeing words: {wrong:?}"
    );
}

/// 🔴 A **seam**, not a measurement (`req/299` §3's distinction, kept): the predicate is spelled as
/// a match with an explicit arm per variant and a `_` arm answering `false`, so a word added later
/// and left unclassified is refused rather than admitted.
///
/// Read out of the shipped source because the alternative — adding a variant in a test to see what
/// happens — is not something a `#[non_exhaustive]`-free public enum lets a test do without
/// changing the crate.
#[test]
fn the_gate_is_a_positive_set_with_a_fail_closed_default() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalogue.rs");
    let source = std::fs::read_to_string(&path).expect("catalogue.rs readable");
    let start = source
        .find("pub fn draws_from_outside_the_forward_call")
        .unwrap_or_else(|| {
            panic!(
                "🔴 `req/38` §221 ruling 1 asked for a method on `ArgSource` that declares, per \
                 variant, whether the member is drawn from outside the forward call. \
                 `catalogue.rs` declares none"
            )
        });
    let end = start
        + source[start..]
            .find("\n    }\n")
            .expect("the method has a body");
    let method = &source[start..end];
    for variant in [
        "ArgSource::PriorContentsUtf8",
        "ArgSource::PriorJson",
        "ArgSource::DoResult",
        "ArgSource::DoResultNumberFrom",
        "ArgSource::Const",
        "ArgSource::ConstJson",
        "ArgSource::Forward",
        "ArgSource::GitBlobSha1OfForward",
    ] {
        assert!(
            method.contains(variant),
            "🔴 {variant} is not classified by name in the predicate, so it takes the fail-closed \
             default silently — which is right for a word nobody thought about and wrong for a \
             word this repository ships"
        );
    }
    assert!(
        method.contains("_ => false"),
        "🔴 the default arm must answer `false`: a word added later and not classified here has to \
         be treated as carried by the forward call, so a template built only out of it is refused. \
         `req/38` §221 ruling 1: an unclassified new variant answers false, which is fail-closed"
    );
    // Code lines only: this repair's own doc comments quote the old spelling, and a scan that
    // counted a quotation as an occurrence would hold the file to never explaining itself.
    let code_carries_the_negative_spelling = source
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with("//"))
        .any(|line| line.contains("!matches!(source, ArgSource::Forward(_))"));
    assert!(
        !code_carries_the_negative_spelling,
        "🔴 the negative spelling is what `req/303` H-01 measured: it says \"not the one variant I \
         thought of\", and `git_blob_sha1_of_forward` is not that variant"
    );
}

// ---------------------------------------------------------------------------
// H-01 — the controls, so that "refuse everything" does not satisfy the arms above
// ---------------------------------------------------------------------------

/// The two shipped fixtures whose inverses need no prior — read off disk, not retyped.
#[test]
fn the_shipped_fixtures_still_parse() {
    for name in [
        "notion-page-catalogue.json",
        "github-issue-catalogue.json",
        "github-restore-catalogue.json",
        "github16-p0-catalogue.json",
    ] {
        let answer = Catalogue::from_json(&fixture(name));
        println!("R22_FIXTURE {name} accepted={}", answer.is_ok());
        assert!(
            answer.is_ok(),
            "🔴 {name} is a sound declaration this repository ships. Narrowing the gate until it \
             refuses one is the failure `req/299` §0 recorded: {:?}",
            answer.err()
        );
    }
}

/// **DR-V4B-2** (`req/38` §123 ruling 2): the trash round trip, whose inverse is a constant. The
/// whole reason `const_json` exists, and the declaration R20's first spelling of this gate broke.
#[test]
fn the_constant_inverse_of_a_flipped_field_still_parses() {
    let answer = parse(&json!({
        "notion:patch-page": {
            "restored_by": "notion:patch-page",
            "arguments": {
                "page_id": { "forward": "page_id" },
                "in_trash": { "const_json": false }
            }
        }
    }));
    println!("R22_DRV4B2 accepted={}", answer.is_ok());
    assert!(
        answer.is_ok(),
        "🔴 DR-V4B-2: `patch-page {{in_trash: false}}` undoes `patch-page {{in_trash: true}}` and \
         needs no prior. {:?}",
        answer.err()
    );
}

/// 🔴 `git_blob_sha1_of_forward` is not banned — it is **insufficient alone**. The shipped
/// `create_or_update_file` declaration carries it beside `prior_contents_utf8` and still parses,
/// which is the discrimination this whole gate rests on.
#[test]
fn the_git_blob_word_still_travels_beside_a_prior_member() {
    let answer = parse(&body(json!({
        "uri": { "forward": "uri" },
        "contents": "prior_contents_utf8",
        "sha": { "git_blob_sha1_of_forward": "contents" },
    })));
    println!("R22_GITBLOB_OK accepted={}", answer.is_ok());
    assert!(
        answer.is_ok(),
        "the repair is about what a template draws on, not about which words may appear in it: {:?}",
        answer.err()
    );
}

// ---------------------------------------------------------------------------
// M-03 — a fault wears its own face
// ---------------------------------------------------------------------------

/// 🔴 `req/303` M-03: a catalogue with **no `read_by` anywhere** whose `restored_by` names no tool.
/// The fault is about the restore face, and the sentence must not call it a read declaration.
#[test]
fn a_fault_about_a_restore_name_is_not_called_a_read_declaration_fault() {
    let why = parse(&json!({ "notes.write": { "restored_by": "  " } }))
        .expect_err("a nameless restore face is a parse error (`req/291` M-04)");
    println!("R22_FACE restore_name why={why:?}");
    assert!(
        !why.contains("read declaration"),
        "🔴 `req/303` M-03: this catalogue declares no read face, and the fault is about \
         `restored_by`. Naming it a read-declaration fault is the species `req/269` M-05 and \
         `req/279` M-01 were both ruled defects for — a fault wearing another fault's face, with a \
         remedy the reader cannot execute: {why}"
    );
    assert!(
        why.contains("the restore declaration of entry"),
        "and the subject it does name is the one that is wrong: {why}"
    );
}

/// 🔴 `req/303` M-03, second half: a template that draws nothing, in a file with no read face.
#[test]
fn a_fault_about_a_template_is_not_called_a_read_declaration_fault() {
    let why = parse(&body(json!({ "uri": { "forward": "uri" } })))
        .expect_err("a forward-only template is a parse error (`req/291` H-01)");
    println!("R22_FACE template why={why:?}");
    assert!(
        !why.contains("read declaration"),
        "🔴 `req/303` M-03: the fault is about the `arguments` template and this file declares no \
         read face: {why}"
    );
    assert!(
        why.contains("the `arguments` template of entry"),
        "and the subject names the template: {why}"
    );
}

/// The control: a fault that **is** about a read face keeps R18's original subject, unchanged.
/// Without this arm, deleting the words "read declaration" everywhere satisfies both arms above.
#[test]
fn a_fault_about_a_read_face_is_still_called_a_read_declaration_fault() {
    let why = parse(&json!({
        "notes.write": {
            "restored_by": "notes.restore",
            "read_by": {
                "by_tool": "notes.get",
                "arguments": { "id": { "forward": "id" } },
                "identity": [ "doc:", { "answer": "/id" } ]
            }
        }
    }))
    .expect_err("a read face with no template is a parse error (`req/279` H-01)");
    println!("R22_FACE read why={why:?}");
    assert!(
        why.contains("the read declaration of entry"),
        "🔴 R18's subject is right where R18 wrote it, and the repair is a narrowing rather than a \
         rewrite: {why}"
    );
}

// ---------------------------------------------------------------------------
// L-05 — two declarations that render the same
// ---------------------------------------------------------------------------

/// 🔴 `req/303` L-05: the audit's pair. Two keys, one visible name, `declared=2`.
#[test]
fn two_spellings_of_one_visible_tool_name_never_reach_a_session() {
    let answer = parse(&json!({
        "\u{e9}crire": { "restored_by": "restaurer" },
        "e\u{301}crire": { "restored_by": "restaurer" },
    }));
    let declared = answer.as_ref().ok().map(Catalogue::declared);
    println!(
        "R22_NFC declared={declared:?} err={:?}",
        answer.as_ref().err()
    );
    let why = answer.err().unwrap_or_else(|| {
        panic!(
            "🔴 `req/303` L-05: `\\u{{e9}}crire` and `e\\u{{301}}crire` render identically and \
             parsed as {declared:?} declarations. \"What undoes what\" is a decision a person makes \
             by reading the file, and two lines a person cannot tell apart are not two decisions"
        )
    });
    assert!(
        why.contains("carries a combining mark"),
        "and the refusal says which spelling is the problem and what to write instead: {why}"
    );
}

/// The same refusal for the decomposed spelling **alone** — which is how the collision is closed
/// without a normalisation table. Stated in `TOOL_NAME_IS_DECOMPOSED` rather than implied.
#[test]
fn a_decomposed_tool_name_alone_is_a_parse_error() {
    let why = parse(&json!({ "e\u{301}crire": { "restored_by": "restaurer" } }))
        .expect_err("a decomposed key has a composed twin whether or not the twin is in the file");
    println!("R22_NFD why={why:?}");
    assert!(
        why.contains("What to fix: spell the tool name in its composed (NFC) form"),
        "and the remedy is one the reader can execute: {why}"
    );
}

// ---------------------------------------------------------------------------
// M-03 on the **code** road — which closing sentence `crate::invert` appends
// ---------------------------------------------------------------------------

const SERVER: &str = "stdio://r22";
const DOC: &str = "doc:d1";

/// A transport that records every arrival, so an arm can say the server was never asked.
#[derive(Debug, Default)]
struct Watcher {
    arrivals: Mutex<Vec<String>>,
}

impl Watcher {
    fn arrivals(&self) -> Vec<String> {
        self.arrivals.lock().expect("not poisoned").clone()
    }
    fn saw(&self, what: impl Into<String>) {
        self.arrivals
            .lock()
            .expect("not poisoned")
            .push(what.into());
    }
}

impl ToolTransport for Watcher {
    fn read(&self, _server: &str, resource: &str) -> SubstrateResult<Vec<u8>> {
        self.saw(format!("read {resource}"));
        Ok(b"the document as it was\n".to_vec())
    }
    fn read_prior_by_tool(
        &self,
        _server: &str,
        tool: &str,
        arguments: &[u8],
    ) -> SubstrateResult<Vec<u8>> {
        self.saw(format!(
            "read_by_tool {tool} {}",
            String::from_utf8_lossy(arguments)
        ));
        Ok(b"{}".to_vec())
    }
    fn call(&self, call: &ToolCall, _admitted: &Admitted) -> SubstrateResult<Vec<u8>> {
        self.saw(format!("call {}", call.tool()));
        Ok(b"{}".to_vec())
    }
}

/// Plan one `doc.write` under a catalogue **built in code** and ask `invert` for the escrow.
///
/// The code road is the only one a fault about a `restored_by` or an `arguments` template can still
/// reach: both are parse errors, and after `req/309` item 7 the `--restore` flag mouth runs the same
/// check. `req/269` M-05's argument is why the check is asked here at all — a value that never met a
/// parser is a value the parser never checked.
fn invert_under(catalogue: Catalogue) -> (Option<String>, Vec<String>) {
    let transport = Arc::new(Watcher::default());
    let adapter =
        McpAdapter::new(transport.clone() as Arc<dyn ToolTransport>).with_catalogue(catalogue);
    let locator = format!("{SERVER}#{DOC}");
    let pre = absent_snapshot(&locator);
    let arguments = br#"{"id":"doc:d1","text":"what the agent wants\n"}"#.to_vec();
    let delta = adapter
        .plan(&intent_for(&locator, "doc.write", &arguments), &pre)
        .expect("a well-formed call plans");
    let refusal = match adapter.invert(&delta, &pre) {
        Err(Error::Unreadable { detail, .. }) => Some(detail),
        _ => None,
    };
    (refusal, transport.arrivals())
}

/// The sentence [`gx_adapter_mcp::DECLARATION_UNSOUND_REFUSAL`] carries, spelled rather than
/// imported so this file runs at the branch point (`req/299` §0's rule).
const READ_FACE_CLOSER: &str = "The declared read face was never called";

/// 🔴 `req/303` M-03 on the road an agent reads: a fault about a `restored_by` must not be closed
/// with a sentence about a read face that does not exist.
#[test]
fn the_closing_sentence_of_a_restore_name_fault_claims_no_read_face() {
    let (refusal, arrivals) = invert_under(Catalogue::new().with_restore("doc.write", "  "));
    let refusal = refusal.expect("a nameless restore face is refused on the code road too");
    println!("R22_CLOSER restore_name arrivals={arrivals:?} refusal={refusal}");
    assert!(
        !refusal.contains(READ_FACE_CLOSER),
        "🔴 `req/303` M-03: this catalogue declares no read face. The measured sentence read \
         `… the declaration's `restored_by` is \" \" …: this restore catalogue's read declaration \
         is not sound … The declared read face was never called`, and its remedy — *correct the \
         read declaration* — names nothing the reader can open: {refusal}"
    );
    assert!(
        refusal.contains("This entry declares no read face"),
        "and the sentence it does carry says what is true of this fault: {refusal}"
    );
    assert!(
        arrivals.is_empty(),
        "the server is never asked about a declaration this crate judges on its own: {arrivals:?}"
    );
}

/// 🔴 The same for a template that draws nothing the forward call does not carry.
#[test]
fn the_closing_sentence_of_a_template_fault_claims_no_read_face() {
    let template = RestoreTemplate::new().with("id", ArgSource::Forward("id".to_string()));
    let catalogue = Catalogue::new().with_restore_template("doc.write", "doc.restore", template);
    let (refusal, arrivals) = invert_under(catalogue);
    let refusal = refusal.expect("a forward-only template is refused on the code road too");
    println!("R22_CLOSER template arrivals={arrivals:?} refusal={refusal}");
    assert!(
        !refusal.contains(READ_FACE_CLOSER),
        "🔴 `req/303` M-03: the fault is about the `arguments` template: {refusal}"
    );
    assert!(
        refusal.contains("This entry declares no read face"),
        "and the closing sentence is the one for a declaration with no read face: {refusal}"
    );
    assert!(arrivals.is_empty(), "nothing was sent: {arrivals:?}");
}

/// The control: a fault that **is** about a read face keeps R18's closing sentence, word for word.
/// Without it, deleting the sentence everywhere satisfies both arms above.
#[test]
fn the_closing_sentence_of_a_read_face_fault_is_unchanged() {
    let template = RestoreTemplate::new()
        .with("id", ArgSource::Forward("id".to_string()))
        .with("text", ArgSource::PriorContentsUtf8);
    let read = PriorRead::new(
        "doc.get",
        // A member this call does not carry: a fault the parser cannot see, which is why it is the
        // one read-face fault still reachable here (`req/303` §2 (a) 4 measured the same shape).
        RestoreTemplate::new().with(
            "id",
            ArgSource::Forward("a_member_this_call_never_had".into()),
        ),
        gx_adapter_mcp::ObjectIdentity::new(vec![gx_adapter_mcp::IdentityPart::Answer {
            answer: "/id".to_string(),
        }]),
    );
    let catalogue = Catalogue::new()
        .with_restore_template("doc.write", "doc.restore", template)
        .with_prior_read("doc.write", read);
    let (refusal, arrivals) = invert_under(catalogue);
    let refusal = refusal.expect("a read declaration naming a member this call does not carry");
    println!("R22_CLOSER read_face arrivals={arrivals:?} refusal={refusal}");
    assert!(
        refusal.contains(READ_FACE_CLOSER),
        "🔴 R18's sentence stands where R18's subject stands, and the repair is a narrowing rather \
         than a rewrite: {refusal}"
    );
    assert!(arrivals.is_empty(), "nothing was read: {arrivals:?}");
}

/// The control: the composed spelling of the same word parses, and so do the CJK and emoji names
/// `req/303` L-05 measured as accepted. The gate is about combining marks, not about non-ASCII.
#[test]
fn composed_and_non_latin_tool_names_still_parse() {
    for name in [
        "\u{e9}crire",
        "\u{66f8}\u{304d}\u{8fbc}\u{307f}",
        "notes.write",
    ] {
        let answer = parse(&json!({ name: { "restored_by": "restaurer" } }));
        println!("R22_NFC_OK name={name:?} accepted={}", answer.is_ok());
        assert!(
            answer.is_ok(),
            "🔴 the gate refuses a decomposed spelling, not a script: {:?}",
            answer.err()
        );
    }
}
