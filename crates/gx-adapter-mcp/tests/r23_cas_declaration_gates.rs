// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/312` H-01(a), M-01, M-02, L-01 and L-02** (`req/313` §1 items 1, 3, 4 and 5) — the
//! declaration space of `$cas_read`, and the fold that signed an absence nobody measured.
//!
//! # What the twenty-second adversarial audit measured, verbatim
//!
//! ```text
//! A22_CASFACE effect  accepted=false  why=… declares that tool as an effect
//! A22_CASFACE restore accepted=true   why=
//! A22_NFD_PLACES_ACCEPTED=["restored_by", "read_by.by_tool", "$cas_read key", "$cas_read by_tool"]
//! A22_NFD_CASKEYS accepted=true cas_reads_declared=Ok(2)
//! A22_INERT pattern="Doc://Host/Page/" resource="doc://host/Page/1" matched=false
//! A22_PREFIX_SIBLING doc://pageant/secret -> Some(("doc://page", "read.page"))
//! A22_OBSERVE_FOLD outcome=Ok(resulting_digest=… equals_absent=true equals_after=false)
//!                  object_now="what the call put there\n"
//! ```
//!
//! Five findings share one file because they are five questions about one declaration slot and the
//! one road that reads it:
//!
//! * **H-01(a)** — the soundness gate asked whether a CAS read face is a **key** of `restores`. A
//!   catalogue names the tools it writes with in **two** places, and the second (`restored_by`, the
//!   inverse) went through. On the real road that emptied the document from `snapshot`, on the
//!   admit road **and** on a road where gx refused before a verdict.
//! * **M-01** — a read that **failed** after the call was folded to the digest of no content, so a
//!   receipt was signed saying an object holding 24 bytes was empty, and every later undo of it was
//!   refused `PRECONDITION_CHANGED` over a world that had not moved.
//! * **M-02** — `docs/LIMITS.md` v0.5-i says of the combining-mark gate *"the width is exactly
//!   this"*. It ran in **one** of the five positions a catalogue file spells a name.
//! * **L-01** — a `$cas_read` prefix was compared, byte for byte as written, against resource URIs
//!   that are always normalised. Four spellings parsed, looked live and unlocked nothing.
//! * **L-02** — the prefix was a **byte** prefix, so `doc://page` governed `doc://pageant/secret`
//!   — a different name space, read through this declaration's tool.
//!
//! # Red-first
//!
//! No symbol this lane created is named. The four sentences this lane adds are spelled as needles,
//! the absence token is spelled as a literal, and every arm reads a value the pre-repair tree can
//! also produce — so the file compiles at `7261321` and fails on its assertions.

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    Admitted, CasRead, CasTemplate, Catalogue, McpAdapter, ToolCall, ToolTransport,
};
use gx_substrate::{Error, Result, SubstrateAdapter};

use support::{absent_snapshot, intent_for};

const SERVER: &str = "stdio://r23";
const DOC: &str = "doc://page/1";
const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";

/// The sentence [`gx_adapter_mcp::CAS_READ_FACE_IS_AN_INVERSE`] carries, spelled rather than
/// imported so this file builds at the branch point (`req/299` §0's rule).
const INVERSE_NEEDLE: &str = "the inverse of one";

/// The token a transport writes into `Unreadable`'s `detail` to mean "the server answered, and its
/// answer is that there is nothing here". Spelled for the same reason.
///
/// 🔴 **R24** — the mock below writes it **first**, because `gx-mcp-wire` writes it first: `req/316`
/// M-01 measured a server forging an absence by spelling this token inside its own error message,
/// and the repair gave the token a position rather than a wider search. A double that composed it
/// the old way would be a double that is no longer a model of the wire, which is `req/316` §5
/// self-admission 3 pointing the other way.
const ANSWERED_ABSENT: &str =
    "[gx: the server answered, and its answer is that this locator holds nothing]";

/// The sentence a post-apply observation that did not answer now carries.
const NOT_ANSWERED_NEEDLE: &str = "could not read the object back";

// ---------------------------------------------------------------------------
// H-01(a) — the two places a catalogue file says a tool writes
// ---------------------------------------------------------------------------

/// The audit's own catalogue, verbatim apart from the resource prefix.
fn inverse_as_read_face() -> String {
    format!(
        r#"{{
  "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
  "$cas_read": {{
    "doc://": {{
      "by_tool": "{RESTORE_TOOL}",
      "arguments": {{ "uri": "resource", "contents": {{ "const": "" }} }}
    }}
  }}
}}"#
    )
}

/// 🔴 `req/312` H-01(a): a read face this same file names as an **inverse** is refused.
#[test]
fn a_cas_read_face_this_file_declares_as_an_inverse_is_refused() {
    let parsed = Catalogue::from_json(inverse_as_read_face().as_bytes());
    let why = parsed.as_ref().err().cloned().unwrap_or_default();
    println!("R23_H01A accepted={} why={why}", parsed.is_ok());
    assert!(
        parsed.is_err(),
        "🔴 `req/312` H-01: this catalogue parsed. `snapshot` then called {RESTORE_TOOL:?} six \
         times before the agent's own effect reached the server, the document was empty \
         afterwards, and on the refusing road gx told the agent nothing had been sent. The tool \
         writes, and this file is the thing that says so — `restored_by` is a declaration that \
         {RESTORE_TOOL:?} puts objects back"
    );
    assert!(
        why.contains(INVERSE_NEEDLE),
        "and the sentence names the fact the reader has to act on — that this file declares that \
         tool as an inverse — rather than the one about effects, which is true of a different \
         spelling: {why}"
    );
    assert!(
        why.contains("What to fix:"),
        "R17's wording rule: a refusal carries a remedy: {why}"
    );
}

/// 🔴 The remedy the sibling constant prints, followed **verbatim**, must not reach the harm it
/// names.
///
/// This is the whole shape of the finding: `CAS_READ_FACE_IS_AN_EFFECT` says *"name a read face
/// this catalogue does not declare as an effect"*, and `notes.restore` is not declared as an
/// effect. A reader doing exactly what they were told arrived at an unadmitted write from
/// `snapshot`.
#[test]
fn the_remedy_the_effect_gate_prints_no_longer_leads_to_the_harm_it_names() {
    let effect_face = format!(
        r#"{{
  "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
  "$cas_read": {{ "doc://": {{ "by_tool": "{WRITE_TOOL}" }} }}
}}"#
    );
    let effect = Catalogue::from_json(effect_face.as_bytes());
    let effect_why = effect.as_ref().err().cloned().unwrap_or_default();
    println!(
        "R23_REMEDY effect_refused={} why={effect_why}",
        effect.is_err()
    );
    assert!(
        effect.is_err(),
        "the control: naming the effect tool was refused before this lane and still is"
    );
    assert!(
        effect_why.contains("does not declare as an effect"),
        "and it still prints the remedy whose reading is the subject of this arm: {effect_why}"
    );
    // The declaration a reader reaches by following that remedy.
    assert!(
        Catalogue::from_json(inverse_as_read_face().as_bytes()).is_err(),
        "🔴 following the printed remedy verbatim reaches a declaration gx accepts, and the \
         accepted declaration does the exact thing the remedy exists to prevent"
    );
}

/// 🔴 The negative control, and the one that decides whether this repair is too wide.
///
/// A read face that is neither an effect nor an inverse is what every shipped fixture declares, and
/// all of them must still parse. Without this arm, refusing every `$cas_read` satisfies the arms
/// above.
#[test]
fn a_read_only_face_and_every_shipped_fixture_still_parse() {
    let read_only = format!(
        r#"{{
  "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
  "$cas_read": {{ "doc://": {{ "by_tool": "doc.read", "arguments": {{ "uri": "resource" }} }} }}
}}"#
    );
    let parsed = Catalogue::from_json(read_only.as_bytes());
    println!("R23_READ_ONLY accepted={}", parsed.is_ok());
    assert!(
        parsed.is_ok(),
        "a read face this file names nowhere as a writer is the shape the slot exists for: {:?}",
        parsed.err()
    );

    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut checked = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&fixtures)
        .expect("the adapter ships fixtures")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        // The directory also holds fixtures that are not catalogues (a recorded `tools/call`
        // observation, for one). Selected by name rather than by "whatever parses", because the
        // second reading would let a catalogue that stopped parsing leave this loop silently.
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("catalogue"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let bytes = std::fs::read(&path).expect("readable");
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let outcome = Catalogue::from_json(&bytes);
        println!("R23_FIXTURE {name} accepted={}", outcome.is_ok());
        assert!(
            outcome.is_ok(),
            "🔴 the widened gate refuses a catalogue this repository ships: {name}: {:?}",
            outcome.err()
        );
        checked.push(name);
    }
    assert!(
        checked.len() >= 4,
        "the scan found {} shipped catalogues, and `req/310` §2 item 1 names four by hand — a scan \
         that found nothing would satisfy the loop above: {checked:?}",
        checked.len()
    );
}

// ---------------------------------------------------------------------------
// M-02 — the five places a catalogue file spells a name
// ---------------------------------------------------------------------------

/// The five positions, each with a decomposed `é` (`e` + U+0301) in exactly one of them.
fn five_places() -> Vec<(&'static str, String)> {
    let nfd = "notes.w\u{0301}rite";
    vec![
        (
            "restores key",
            format!(r#"{{ "{nfd}": {{ "restored_by": "{RESTORE_TOOL}" }} }}"#),
        ),
        (
            "restored_by",
            format!(r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{nfd}" }} }}"#),
        ),
        (
            "read_by.by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
                       "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }},
                       "read_by": {{ "by_tool": "{nfd}",
                                     "arguments": {{ "uri": {{ "forward": "uri" }} }},
                                     "identity": ["doc://", {{ "answer": "/uri" }}] }} }} }}"#
            ),
        ),
        (
            "$cas_read key",
            // The decomposed spelling: `e` followed by U+0301, which renders as `é` and is not
            // the `é` two lines of prose above it. The byte sequence is what the arm is about;
            // `two_cas_read_prefixes_that_render_identically_are_no_longer_two_declarations`
            // holds the pair against each other so this one cannot pass by being the wrong one.
            r#"{ "$cas_read": { "doc://café/": { "by_tool": "doc.read" } } }"#.to_string(),
        ),
        (
            "$cas_read by_tool",
            format!(r#"{{ "$cas_read": {{ "doc://": {{ "by_tool": "{nfd}" }} }} }}"#),
        ),
    ]
}

/// 🔴 `req/312` M-02: the gate is asked in all five places a name is spelled, not one.
#[test]
fn the_combining_mark_gate_is_asked_in_every_place_a_name_is_spelled() {
    let mut accepted: Vec<&str> = Vec::new();
    for (place, bytes) in five_places() {
        let parsed = Catalogue::from_json(bytes.as_bytes());
        let why = parsed.as_ref().err().cloned().unwrap_or_default();
        println!(
            "R23_NFD_PLACE {place:<20} accepted={} why={why}",
            parsed.is_ok()
        );
        if parsed.is_ok() {
            accepted.push(place);
        } else {
            assert!(
                why.contains("combining mark") && why.contains("What to fix:"),
                "the refusal names the fault and a remedy: {place}: {why}"
            );
        }
    }
    println!("R23_NFD_ACCEPTED={accepted:?}");
    assert!(
        accepted.is_empty(),
        "🔴 `req/312` M-02: `docs/LIMITS.md` v0.5-i says of this gate **\"the width is exactly \
         this\"**, and the width it describes is which marks are covered. The audit measured the \
         other axis: which **positions** are asked. Four of the five were not, and the harm v0.5-i \
         closed — two declarations an operator cannot tell apart while approving them — reproduces \
         in every one of them: {accepted:?}"
    );
}

/// 🔴 And the harm itself, in the position the audit reproduced it in.
#[test]
fn two_cas_read_prefixes_that_render_identically_are_no_longer_two_declarations() {
    let twins = r#"{ "$cas_read": {
             "doc://café/": { "by_tool": "read.one" },
             "doc://café/": { "by_tool": "read.two" }
           } }"#
        .to_string();
    let parsed = Catalogue::from_json(twins.as_bytes());
    let declared = parsed.as_ref().map(Catalogue::cas_reads_declared);
    println!(
        "R23_NFD_TWINS accepted={} declared={declared:?}",
        parsed.is_ok()
    );
    assert!(
        parsed.is_err(),
        "🔴 `req/312` M-02: `doc://café/` and `doc://cafe´/` are one line to a reader and two \
         reading roads to this parser, and which one governs an object is then decided by a byte \
         the page does not show. This file was accepted with `cas_reads_declared` = {declared:?}"
    );
}

/// The control: a **composed** spelling is not the fault, and is still a declaration.
#[test]
fn the_composed_spelling_of_the_same_prefix_still_parses() {
    let composed = r#"{ "$cas_read": { "doc://café/": { "by_tool": "read.one" } } }"#;
    let parsed = Catalogue::from_json(composed.as_bytes());
    println!("R23_NFC accepted={}", parsed.is_ok());
    assert!(
        parsed.is_ok(),
        "the gate is about the decomposed spelling and not about the script: {:?}",
        parsed.err()
    );
    assert_eq!(
        parsed.expect("parsed").cas_reads_declared(),
        1,
        "one declaration, and it survives normalisation"
    );
}

// ---------------------------------------------------------------------------
// L-01 — the prefix is normalised the way every resource URI is
// ---------------------------------------------------------------------------

/// 🔴 `req/312` L-01: four spellings that parsed and unlocked nothing now unlock what they say.
#[test]
fn a_prefix_that_is_not_in_normal_form_governs_the_locators_it_names() {
    let resource = "doc://host/Page/1";
    let mut inert: Vec<&str> = Vec::new();
    for pattern in [
        "Doc://Host/Page/",
        "doc://Host/Page/",
        "DOC://host/Page/",
        "doc://host/Page/./",
    ] {
        let json = format!(r#"{{ "$cas_read": {{ "{pattern}": {{ "by_tool": "doc.read" }} }} }}"#);
        let catalogue = Catalogue::from_json(json.as_bytes())
            .unwrap_or_else(|e| panic!("{pattern:?} is a well-formed declaration: {e}"));
        let matched = catalogue.cas_read_for(resource).is_some();
        println!("R23_INERT pattern={pattern:?} resource={resource:?} matched={matched}");
        if !matched {
            inert.push(pattern);
        }
    }
    assert!(
        inert.is_empty(),
        "🔴 `req/312` L-01: these declarations parse, the start-up line counts them, and they \
         unlock nothing — the resource they are compared against has been through `normalize` and \
         they have not. `req/269` M-01's defect exactly: one line decides the file's behaviour and \
         the operator cannot see which way: {inert:?}"
    );
}

/// 🔴 And the thing normalisation must not do quietly: drop one of two spellings.
#[test]
fn two_prefixes_that_normalise_to_one_are_refused_rather_than_one_being_dropped() {
    let json = r#"{ "$cas_read": {
        "Doc://Host/page/": { "by_tool": "read.upper" },
        "doc://host/page/": { "by_tool": "read.lower" }
    } }"#;
    let parsed = Catalogue::from_json(json.as_bytes());
    let why = parsed.as_ref().err().cloned().unwrap_or_default();
    let declared = parsed.as_ref().map(Catalogue::cas_reads_declared);
    println!(
        "R23_COLLIDE accepted={} declared={declared:?} why={why}",
        parsed.is_ok()
    );
    assert!(
        parsed.is_err(),
        "🔴 two spellings of one normal form are two declarations of one reading road, and \
         exactly one of them can survive the map. Surviving silently is the repair for L-01 \
         reproducing the defect L-01 is: declared={declared:?}"
    );
    assert!(
        why.contains("normalises to") && why.contains("What to fix:"),
        "and the sentence names both spellings and a remedy: {why}"
    );
}

// ---------------------------------------------------------------------------
// L-02 — a prefix ends where a segment ends
// ---------------------------------------------------------------------------

/// 🔴 `req/312` L-02: `doc://page` does not govern `doc://pageant/secret`.
#[test]
fn a_prefix_governs_a_name_space_and_not_a_name_that_starts_with_it() {
    let json = r#"{ "$cas_read": {
        "doc://":           { "by_tool": "read.any" },
        "doc://page":       { "by_tool": "read.page" },
        "doc://page/deep/": { "by_tool": "read.deep" }
    } }"#;
    let catalogue = Catalogue::from_json(json.as_bytes()).expect("a well-formed declaration");
    let face = |resource: &str| {
        catalogue
            .cas_read_for(resource)
            .map(|(pattern, read)| (pattern.to_string(), read.by_tool().to_string()))
    };
    for (resource, expected) in [
        ("doc://other/1", "read.any"),
        ("doc://page/1", "read.page"),
        ("doc://page/deep/1", "read.deep"),
        ("doc://page", "read.page"),
    ] {
        let got = face(resource);
        println!("R23_PREFIX resource={resource} -> {got:?}");
        assert_eq!(
            got.as_ref().map(|(_, tool)| tool.as_str()),
            Some(expected),
            "longest-prefix matching is unchanged for every locator inside a declared name space"
        );
    }
    let sibling = face("doc://pageant/secret");
    println!("R23_PREFIX_SIBLING doc://pageant/secret -> {sibling:?}");
    assert_eq!(
        sibling.as_ref().map(|(_, tool)| tool.as_str()),
        Some("read.any"),
        "🔴 `req/312` L-02: `doc://pageant` is a **different name space** from `doc://page`, and a \
         byte prefix cannot tell them apart. Because DR-46-21 has not landed, a read's answer is \
         not yet bound to the object it is about, so the neighbour's bytes become this object's \
         digest. The declaration that should govern it is the scheme's, which is what an operator \
         who wrote `doc://` meant: {sibling:?}"
    );
}

// ---------------------------------------------------------------------------
// M-01 — the fold that signed an absence nobody measured
// ---------------------------------------------------------------------------

/// A server whose read face can be told to die **after** the call, and which can mark a genuine
/// absence the way a transport that knows the difference does.
#[derive(Default)]
struct Dying {
    body: Mutex<Option<Vec<u8>>>,
    /// After the call, `read` fails with the transport's word for "I could not tell you".
    dies_after_the_call: Mutex<bool>,
    /// After the call, `read` answers that the locator holds nothing, marked as an **answer**.
    removed_by_the_call: Mutex<bool>,
    called: AtomicUsize,
}

impl Dying {
    fn new(body: &[u8]) -> Self {
        Self {
            body: Mutex::new(Some(body.to_vec())),
            ..Self::default()
        }
    }

    fn now(&self) -> Option<Vec<u8>> {
        self.body.lock().expect("not poisoned").clone()
    }
}

impl ToolTransport for Dying {
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>> {
        let locator = format!("{server}#{resource}");
        if self.called.load(Ordering::SeqCst) > 0 {
            if *self.dies_after_the_call.lock().expect("not poisoned") {
                return Err(Error::Unreadable {
                    locator,
                    detail: "the pipe to this server closed while `resources/read` was in flight"
                        .to_string(),
                });
            }
            if *self.removed_by_the_call.lock().expect("not poisoned") {
                return Err(Error::Unreadable {
                    locator,
                    detail: format!("{ANSWERED_ABSENT} resources/read: no such resource."),
                });
            }
        }
        self.now().ok_or(Error::Unreadable {
            locator,
            detail: format!("{ANSWERED_ABSENT} this server holds nothing there."),
        })
    }

    fn call(&self, call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        self.called.fetch_add(1, Ordering::SeqCst);
        let text: serde_json::Value =
            serde_json::from_slice(call.arguments()).unwrap_or(serde_json::Value::Null);
        let body = text
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .as_bytes()
            .to_vec();
        *self.body.lock().expect("not poisoned") = Some(body);
        Ok(b"{\"ok\":true}".to_vec())
    }
}

fn apply_over(server: &Arc<Dying>) -> Result<gx_substrate::AppliedDelta> {
    let adapter = McpAdapter::new(server.clone() as Arc<dyn ToolTransport>)
        .with_catalogue(Catalogue::new().with_restore(WRITE_TOOL, RESTORE_TOOL));
    let locator = format!("{SERVER}#{DOC}");
    let arguments = br#"{"uri":"doc://page/1","contents":"what the call put there\n"}"#.to_vec();
    let pre = absent_snapshot(&locator);
    let delta = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, &arguments), &pre)
        .expect("a well-formed call plans");
    adapter.apply(&delta)
}

/// 🔴 `req/312` M-01: a read that **failed** after the call is not signed as an absence.
#[test]
fn a_read_that_dies_after_the_call_is_refused_rather_than_signed_as_absence() {
    let server = Arc::new(Dying::new(b"the note as it stood\n"));
    *server.dies_after_the_call.lock().expect("not poisoned") = true;
    let outcome = apply_over(&server);
    let object = server.now();
    let absent = gx_adapter_mcp::adapter::absent_digest();
    let signed_absence = outcome
        .as_ref()
        .ok()
        .map(|applied| applied.resulting_digest() == &absent);
    println!(
        "R23_OBSERVE_FOLD ok={} signed_absence={signed_absence:?} object_now={:?}",
        outcome.is_ok(),
        object.as_deref().map(String::from_utf8_lossy)
    );
    assert_eq!(
        object.as_deref(),
        Some(b"what the call put there\n".as_slice()),
        "the call was made and the object holds its bytes — this arm is about the record, not the \
         effect"
    );
    let why = match &outcome {
        Err(Error::Unreadable { detail, .. }) => detail.clone(),
        other => panic!(
            "🔴 `req/312` M-01: the observation did not answer and this adapter signed a \
             postcondition anyway. The digest it signed is the digest of **no content**, over an \
             object holding 24 bytes, and every later undo of that receipt is refused \
             `PRECONDITION_CHANGED` — *the world moved after the transformation being undone \
             committed* — over a world that did not move. signed_absence={signed_absence:?} \
             outcome={other:?}"
        ),
    };
    assert!(
        why.contains(NOT_ANSWERED_NEEDLE) && why.contains("What to fix:"),
        "and the refusal says which of the two facts happened, and what to do: {why}"
    );
    assert!(
        why.contains("not** signed as absence") || why.contains("not signed as absence"),
        "and it says what it declined to sign, which is the thing a reader of the journal will \
         otherwise go looking for: {why}"
    );
}

/// 🔴 The negative control, and the road the fold was written for: a call that **removed** the
/// resource still observes an absence.
///
/// Without this arm, refusing every unreadable post-state satisfies the arm above and takes a
/// legitimate shape of change with it.
#[test]
fn a_call_that_removed_the_resource_is_still_observed_as_an_absence() {
    let server = Arc::new(Dying::new(b"the note as it stood\n"));
    *server.removed_by_the_call.lock().expect("not poisoned") = true;
    let outcome = apply_over(&server);
    let absent = gx_adapter_mcp::adapter::absent_digest();
    println!(
        "R23_OBSERVE_REMOVED ok={} is_absent={:?}",
        outcome.is_ok(),
        outcome
            .as_ref()
            .ok()
            .map(|a| a.resulting_digest() == &absent)
    );
    let applied = outcome.expect(
        "a server that **answers** that the locator holds nothing has told this adapter a fact \
         about the world, and the postcondition of a removal is the absent digest",
    );
    assert_eq!(
        applied.resulting_digest(),
        &absent,
        "the collision `gx-adapter-fs` and `gx-adapter-git` disclose for an absent file is \
         unchanged — what changed is which preimage reaches it"
    );
}

/// 🔴 The healthy control: nothing about the ordinary road moved.
#[test]
fn a_read_that_answers_after_the_call_signs_what_the_object_holds() {
    let server = Arc::new(Dying::new(b"the note as it stood\n"));
    let applied = apply_over(&server).expect("the ordinary road is unchanged");
    let after = gx_adapter_mcp::adapter::content_digest(b"what the call put there\n");
    println!(
        "R23_OBSERVE_HEALTHY is_after={}",
        applied.resulting_digest() == &after
    );
    assert_eq!(
        applied.resulting_digest(),
        &after,
        "the postcondition is the read-back of what the call put there"
    );
}

/// 🔴 And the fold is asked about the **declared** road too, which is where DR-46-16 widened the
/// preimage of `Unreadable` without widening the fold's argument.
#[test]
fn the_declared_road_gets_the_same_two_answers_as_the_resources_road() {
    let json = format!(
        r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
              "$cas_read": {{ "doc://": {{ "by_tool": "doc.read", "arguments": {{ "uri": "resource" }} }} }} }}"#
    );
    let catalogue = Catalogue::from_json(json.as_bytes()).expect("a read-only face parses");
    // A declared face on a server that has no such tool: `read_prior_by_tool`'s default refuses,
    // and the refusal is unmarked — it is "I could not tell you".
    let server = Arc::new(Dying::new(b"the note as it stood\n"));
    let adapter =
        McpAdapter::new(server.clone() as Arc<dyn ToolTransport>).with_catalogue(catalogue);
    let locator = format!("{SERVER}#{DOC}");
    let arguments = br#"{"uri":"doc://page/1","contents":"what the call put there\n"}"#.to_vec();
    let pre = absent_snapshot(&locator);
    let planned = adapter.plan(&intent_for(&locator, WRITE_TOOL, &arguments), &pre);
    let outcome = planned.and_then(|delta| adapter.apply(&delta));
    println!("R23_DECLARED_ROAD ok={}", outcome.is_ok());
    match outcome {
        Err(Error::Unreadable { detail, .. }) => assert!(
            detail.contains("read-by-tool face") || detail.contains(NOT_ANSWERED_NEEDLE),
            "a declared face this transport does not publish is a read that did not answer: \
             {detail}"
        ),
        other => panic!(
            "🔴 a declared CAS face that cannot be reached is not an absence: this road is where \
             DR-46-16 widened `Unreadable`'s preimage, and the fold has to have moved with it: \
             {other:?}"
        ),
    }
}

/// The instrument's own guard: `CasRead` and `CasTemplate` are the types these declarations parse
/// into, and this arm builds one in code so a lane that changes the format is red here too.
#[test]
fn the_declaration_this_file_writes_in_json_is_the_value_the_crate_builds_in_code() {
    let built =
        Catalogue::new().with_cas_read("doc://", CasRead::new("doc.read", CasTemplate::new()));
    println!("R23_CODE_ROAD declared={}", built.cas_reads_declared());
    assert_eq!(built.cas_reads_declared(), 1);
    assert_eq!(
        built.cas_read_for(DOC).map(|(p, r)| (p, r.by_tool())),
        Some(("doc://", "doc.read")),
        "the code road and the file road resolve the same declaration"
    );
}
