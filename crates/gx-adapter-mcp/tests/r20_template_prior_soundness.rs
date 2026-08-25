// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/291` H-01 (DR-46-19) / M-04** (`req/298` §1 items 1 and 3) — the two declarations R18
//! left standing one step past the gate it built.
//!
//! # What the twentieth adversarial audit measured
//!
//! * **H-01** — R18 made "a `read_by` and **no** `arguments` template" a parse error, on the ground
//!   that *a read face says where a prior comes from; a template says how a restore call is built
//!   out of it, and one without the other is not a declaration*. The audit wrote the neighbour: a
//!   template that **exists** and names no prior. It parsed. `reversibility` answered `true`. The
//!   gate admitted, the commit was signed, and the undo gx itself printed left the object
//!   **empty** — `rc=0`, with a signed commit receipt beside it. Every fingerprint gx checks
//!   passed, because those ask whether the object moved the way the applied delta said and never
//!   whether it came back to where the forward call found it.
//! * **M-04** — `"restored_by": ""` parsed too, `reversibility` answered `true`, and the escrow it
//!   built came back out of **this crate's own decoder** as `<undecodable: … an mcp operation names
//!   the tool it calls, and this one names none>`. R18 closed the same hole on the field beside it
//!   (`by_tool: ""`, L-03); this is the symmetric half.
//!
//! # The denominator of this suite, stated here rather than found later
//!
//! * **In-process, except where an arm says otherwise.** The real-road measurement — a `gx wrap`
//!   session whose undo empties the object — lives in `crates/gx-cli/tests/`, because that binary
//!   is another module's road; this file measures the parser and `invert`.
//! * **Two shipped fixtures are read from disk as negative controls**, not paraphrased into this
//!   file: `fixtures/notion-page-catalogue.json` and `fixtures/github-issue-catalogue.json` are the
//!   two declarations in this repository whose templates carry **no prior member** and are
//!   nonetheless sound (their inverse is a deletion, keyed on the applied call's own result). They
//!   are the reason the gate is "a member the forward call does not carry" rather than
//!   `is_prior()` alone, and a lane that narrows the gate to `is_prior()` is red here.
//! * **The identity space is not re-enumerated** (`r17_attested_object_binding.rs`,
//!   `r18_declaration_soundness.rs`). One witness per finding plus its controls.

mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    Admitted, Catalogue, McpAdapter, McpDelta, Reversibility, ToolCall, ToolTransport,
    DECLARATION_UNSOUND_REFUSAL,
};
use gx_substrate::{Error, PlannedDelta, Result, SubstrateAdapter};

use support::{absent_snapshot, intent_for};

const SERVER: &str = "stdio://r20";
const DOC: &str = "doc:d1";
/// What the object held before the change: the only thing an inverse could put back.
const BEFORE: &str = "the document as it was\n";

/// 🔴 **Why these are literals and not `gx_adapter_mcp::TEMPLATE_NAMES_NO_PRIOR`.**
///
/// A red-first suite has to be able to *run* against the pre-repair source, or its red is a
/// compiler message rather than a measurement. `TEMPLATE_NAMES_NO_PRIOR` does not exist at
/// `20f0635`, so importing it turns this file's red into "cannot find value in crate
/// `gx_adapter_mcp`" — which is true of any file naming any new symbol and therefore proves
/// nothing about the defect. Spelled as fragments, the suite compiles at the base commit and
/// fails where the defect is: the catalogue parses, `refusal()` is `None`, and the `expect`
/// beneath it says so.
///
/// The cost of a literal is drift, and [`the_fragments_are_the_constants_this_crate_ships`] pays
/// it: it reads `src/catalogue.rs` and holds every fragment to the constant's own text, so a lane
/// that reworded the refusal without reading this file is red here.
const NO_PRIOR_FRAGMENTS: &[&str] = &[
    "is a function of the forward call alone",
    "What to fix: draw one member from the prior",
    "`req/291` H-01 / DR-46-19",
];

/// The same, for `req/291` M-04's constant (`RESTORE_TOOL_UNNAMED`).
const UNNAMED_FRAGMENTS: &[&str] = &[
    "which names no tool: an effect that needs undoing names the tool that undoes it",
    "What to fix: name the restore tool",
    "`req/291` M-04",
];

/// 🔴 The seam between this suite's literals and the shipped constants, held by reading the source
/// rather than by linking it — the same instrument shape `r20_refusal_vocabulary_is_whole.rs` uses
/// to hold `wrap.rs`'s array to `invert.rs`'s exports. Linking would restore the compile-red this
/// file exists to avoid; reading does not.
#[test]
fn the_fragments_are_the_constants_this_crate_ships() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalogue.rs"),
    )
    .expect("this crate's catalogue module is readable from its own test");
    // The constants are written with `\` line continuations, so the source spells them across
    // lines with leading indentation the compiler removes. Undo exactly that, and no more.
    let joined = source
        .split("\\\n")
        .map(|part| part.trim_start_matches(' '))
        .collect::<Vec<_>>()
        .join("");

    for fragment in NO_PRIOR_FRAGMENTS.iter().chain(UNNAMED_FRAGMENTS) {
        assert!(
            joined.contains(fragment),
            "🔴 this suite holds a wording no constant in `src/catalogue.rs` carries any more: \
             {fragment:?}. Either the refusal was reworded without this file, or the fragment was \
             mistyped — in both cases the arms below are asserting against a sentence gx does not \
             say"
        );
    }
    for name in ["TEMPLATE_NAMES_NO_PRIOR", "RESTORE_TOOL_UNNAMED"] {
        assert!(
            joined.contains(&format!("pub const {name}: &str =")),
            "🔴 `req/298` §1 items 1 and 3 asked for a constant a probe can hold, and {name} is \
             not declared in `src/catalogue.rs`"
        );
    }
}

fn locator() -> String {
    format!("{SERVER}#{DOC}")
}

fn forward_arguments() -> Vec<u8> {
    br#"{"id":"doc:d1","text":"the document as the agent wants it\n"}"#.to_vec()
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

/// A server that publishes the object and answers its read face with whatever the arm chose.
#[derive(Debug)]
struct R20Server {
    answer: Mutex<Vec<u8>>,
    resources: Mutex<BTreeMap<String, Vec<u8>>>,
    arrivals: Mutex<Vec<String>>,
}

impl R20Server {
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

    fn saw(&self, what: impl Into<String>) {
        self.arrivals
            .lock()
            .expect("not poisoned")
            .push(what.into());
    }
}

impl ToolTransport for R20Server {
    fn read(&self, _server: &str, resource: &str) -> Result<Vec<u8>> {
        self.saw(format!("read {resource}"));
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
        self.saw(format!(
            "read_by_tool {tool} {}",
            String::from_utf8_lossy(arguments)
        ));
        Ok(self.answer.lock().expect("not poisoned").clone())
    }

    fn call(&self, call: &ToolCall, _admitted: &Admitted) -> Result<Vec<u8>> {
        self.saw(format!("call {}", call.tool()));
        Ok(b"{}".to_vec())
    }
}

/// What one arm got back from `reversibility` and `invert`, spelled for a report line.
struct Answered {
    verdict: Result<(Reversibility, Option<PlannedDelta>)>,
    arrivals: Vec<String>,
}

impl Answered {
    fn escrow(&self) -> String {
        match &self.verdict {
            Ok((_, Some(delta))) => match McpDelta::decode(delta.payload()) {
                Ok(decoded) => decoded
                    .ops()
                    .iter()
                    .map(|op| format!("{} {}", op.tool(), String::from_utf8_lossy(op.arguments())))
                    .collect::<Vec<_>>()
                    .join(" | "),
                Err(e) => format!("<undecodable: {e}>"),
            },
            Ok((_, None)) => "<none>".to_string(),
            Err(e) => format!("<refused: {e}>"),
        }
    }

    fn word(&self) -> String {
        match &self.verdict {
            Ok((v, _)) => format!("{v:?}"),
            Err(Error::Unreadable { detail, .. }) => format!("Err({detail})"),
            Err(e) => format!("Err({e:?})"),
        }
    }

    /// The refusal text, when this arm refused. `None` when it answered.
    fn refusal(&self) -> Option<String> {
        match &self.verdict {
            Err(Error::Unreadable { detail, .. }) => Some(detail.clone()),
            _ => None,
        }
    }
}

/// Plan one `doc.write` under this catalogue and ask both questions gx asks before it applies.
fn ask(catalogue: Catalogue, server: &Arc<R20Server>) -> Answered {
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
    // 🔴 **DR-46-26** — this probe asks both questions and pairs the answers; the trait road
    // now carries the verdict too, and the `Option` it used to carry is the `inverse` projection.
    let inverse = adapter
        .invert(&delta, &pre)
        .map(gx_substrate::InvertOutcome::into_inverse);
    let verdict = match (reversibility, inverse) {
        (Ok(v), Ok(i)) => Ok((v, i)),
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    Answered {
        verdict,
        arrivals: server.arrivals(),
    }
}

fn parse(json: &str) -> core::result::Result<Catalogue, String> {
    Catalogue::from_json(json.as_bytes())
}

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// H-01 — the template that exists and never names the prior
// ---------------------------------------------------------------------------

/// 🔴 The three shapes the audit fired, all refused **at parse time**, in one wording.
#[test]
fn a_template_that_names_no_prior_never_reaches_a_session() {
    // (i) the empty template — `req/298` §1 item 1's explicit second clause: it must be stopped by
    // this same gate rather than left to escalate later on the road.
    // (ii) a template built out of the forward call alone — the shape whose undo emptied the object.
    // (iii) the same, beside a `read_by`: a read gx performs and then discards.
    for (label, json) in [
        (
            "empty",
            r#"{"doc.write":{"restored_by":"doc.restore","arguments":{}}}"#,
        ),
        (
            "forward_only",
            r#"{"doc.write":{"restored_by":"doc.restore","arguments":{"id":{"forward":"id"}}}}"#,
        ),
        (
            "read_by_without_prior",
            r#"{"doc.write":{"restored_by":"doc.restore","arguments":{"id":{"forward":"id"}},
              "read_by":{"by_tool":"doc.get","arguments":{"id":{"forward":"id"}},
              "identity":["doc:",{"answer":"/id"}]}}}"#,
        ),
    ] {
        let parsed = parse(json);
        println!(
            "R20_H01 shape={label} parse={}",
            match &parsed {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("Err({e})"),
            }
        );
        let refusal = parsed.expect_err(
            "🔴 `req/291` H-01: this catalogue must not start a session — a template that draws \
             nothing the forward call does not already carry is not a declaration of an inverse",
        );
        for fragment in NO_PRIOR_FRAGMENTS {
            assert!(
                refusal.contains(fragment),
                "the refusal must carry {fragment:?}, so a probe can hold its wording: {refusal}"
            );
        }
        assert!(
            refusal.contains("doc.write"),
            "the refusal must name the entry it is about: {refusal}"
        );
    }
}

/// 🔴 The same three, refused on the **code** road too (`req/269` M-05's argument): a catalogue
/// built through the builder never met a parser, and until this lane `invert` asked
/// `entry_soundness` only when a read face was declared — so the `resources/read` road, which is
/// where the audit's forward-only template actually destroyed an object, was checked by the parser
/// alone.
#[test]
fn the_code_road_refuses_the_same_template_and_calls_no_server() {
    let template = gx_adapter_mcp::RestoreTemplate::new()
        .with("id", gx_adapter_mcp::ArgSource::Forward("id".to_string()));
    let catalogue = Catalogue::new().with_restore_template("doc.write", "doc.restore", template);
    let server = R20Server::answering(b"{}".to_vec());
    let answered = ask(catalogue, &server);
    println!(
        "R20_H01_CODE verdict={} escrow={} arrivals={:?}",
        answered.word(),
        answered.escrow(),
        answered.arrivals
    );
    let refusal = answered.refusal().expect(
        "🔴 `req/291` H-01: a catalogue built in code takes the same answer as one that was parsed",
    );
    for fragment in NO_PRIOR_FRAGMENTS {
        assert!(
            refusal.contains(fragment),
            "one wording on both roads: {refusal}"
        );
    }
    // 🔴 **`req/303` M-03 (R22)** — the claim is unchanged and the **constant** moved. R20 wrote
    // `DECLARATION_UNSOUND_REFUSAL` here because it was the only closing sentence there was; that
    // sentence says *"the declared read face was never called"*, and this catalogue declares no
    // read face at all. The fault is about the `arguments` template, so it now closes with the
    // sentence written for a declaration that has no read face. What this arm exists to hold —
    // that `gx wrap` records it as a **refusal** and not as a failure — is unchanged, and is held
    // here by requiring one of the two rather than by naming which.
    let closes_as_a_declaration_refusal = refusal.contains(DECLARATION_UNSOUND_REFUSAL)
        || refusal.contains("This entry declares no read face, nothing was read");
    assert!(
        closes_as_a_declaration_refusal,
        "and it is a **declaration** refusal, so `gx wrap` records it as one (`req/291` M-03): \
         {refusal}"
    );
    assert!(
        !refusal.contains("The declared read face was never called"),
        "🔴 `req/303` M-03: this catalogue declares no read face, so a sentence saying one was \
         never called is a fault wearing another fault's face: {refusal}"
    );
    assert!(
        answered.arrivals.is_empty(),
        "the server is never asked about a declaration this crate can judge on its own: {:?}",
        answered.arrivals
    );
}

// ---------------------------------------------------------------------------
// M-04 — the restore face with no name
// ---------------------------------------------------------------------------

/// 🔴 Three spellings of "no tool", each a parse error, symmetric with R18's L-03.
#[test]
fn a_restore_face_with_no_name_never_reaches_a_session() {
    for (label, json) in [
        ("empty", r#"{"doc.write":{"restored_by":""}}"#),
        ("spaces", r#"{"doc.write":{"restored_by":"   "}}"#),
        ("tab", "{\"doc.write\":{\"restored_by\":\"\\t\"}}"),
    ] {
        let parsed = parse(json);
        println!(
            "R20_M04 spelling={label} parse={}",
            match &parsed {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("Err({e})"),
            }
        );
        let refusal = parsed.expect_err(
            "🔴 `req/291` M-04: a declaration that names no restore tool builds an escrow this \
             crate's own decoder refuses to read back",
        );
        for fragment in UNNAMED_FRAGMENTS {
            assert!(
                refusal.contains(fragment),
                "the refusal must carry {fragment:?}: {refusal}"
            );
        }
    }
}

/// 🔴 The code road, and the fact that made this M rather than L: the escrow it used to build was
/// **undecodable by this crate**, so `reversibility` answered `true` about a payload gx cannot read.
#[test]
fn the_code_road_refuses_a_nameless_restore_face_before_it_builds_an_unreadable_escrow() {
    let catalogue = Catalogue::new().with_restore("doc.write", "");
    let server = R20Server::answering(b"{}".to_vec());
    let answered = ask(catalogue, &server);
    println!(
        "R20_M04_CODE verdict={} escrow={} arrivals={:?}",
        answered.word(),
        answered.escrow(),
        answered.arrivals
    );
    let refusal = answered
        .refusal()
        .expect("🔴 `req/291` M-04: the code road takes the same answer as the parsed one");
    for fragment in UNNAMED_FRAGMENTS {
        assert!(
            refusal.contains(fragment),
            "one wording on both roads: {refusal}"
        );
    }
    assert!(
        !answered.escrow().contains("<undecodable"),
        "the point of the repair: no escrow is built at all, so none can be undecodable"
    );
}

// ---------------------------------------------------------------------------
// The negative controls — without these, "refuse everything" satisfies the arms above
// ---------------------------------------------------------------------------

/// 🔴 A template that draws the prior still parses, still answers `true`, and still escrows the
/// object's own bytes.
#[test]
fn a_template_that_draws_the_prior_is_untouched() {
    let json = r#"{"doc.write":{"restored_by":"doc.restore","arguments":{
        "id":{"forward":"id"},"text":"prior_contents_utf8"}}}"#;
    let catalogue = parse(json).expect("the sound form parses");
    let server = R20Server::answering(b"{}".to_vec());
    let answered = ask(catalogue, &server);
    println!(
        "R20_CONTROL verdict={} escrow={} arrivals={:?}",
        answered.word(),
        answered.escrow(),
        answered.arrivals
    );
    assert_eq!(
        answered.word(),
        "True",
        "the sound declaration is reversible"
    );
    assert!(
        answered.escrow().contains(BEFORE.trim_end()),
        "and what it escrows is the object's own prior, not a document about it: {}",
        answered.escrow()
    );
}

/// 🔴 The v0.1 `{contents, uri}` form — no template at all — is unchanged. `req/279` H-01 was about
/// the **pair**, and neither lane retired the convention.
#[test]
fn the_v0_1_convention_with_no_template_is_untouched() {
    let catalogue = parse(r#"{"notes.write":{"restored_by":"notes.restore"}}"#)
        .expect("the v0.1 form still parses");
    assert!(catalogue.spec_for("notes.write").is_some());
    println!("R20_V01 declared={}", catalogue.declared());
}

/// 🔴 The two shipped declarations whose inverse is a **deletion**: no prior member, and sound.
///
/// This is the arm that makes the gate's shape falsifiable. `req/298` §1 item 1 spelled the rule as
/// "≥1 member `is_prior()`"; read literally it refuses both of these files, which are the
/// repository's own fixtures and are driven by three `tools/*_e2e.sh` scripts. A change whose
/// inverse deletes what it created has no prior to carry — the identity of the created thing comes
/// from the applied call's result — so the gate asks for a member the **forward call** does not
/// carry, and these two answer with a do-result member.
#[test]
fn a_declaration_whose_inverse_is_a_deletion_needs_no_prior() {
    for (name, tool, member) in [
        ("notion-page-catalogue.json", "API-post-page", "block_id"),
        ("github-issue-catalogue.json", "issue_write", "issue_number"),
    ] {
        let bytes = fixture(name);
        let catalogue = Catalogue::from_json(&bytes).unwrap_or_else(|e| {
            panic!(
                "🔴 `req/291` H-01's gate must not refuse {name}: its inverse is a deletion keyed \
                 on the applied call's own result, which is material the forward call does not \
                 carry. Refused with: {e}"
            )
        });
        let spec = catalogue
            .spec_for(tool)
            .unwrap_or_else(|| panic!("{name} declares {tool}"));
        let template = spec.template().expect("this fixture declares a template");
        assert!(
            template
                .arguments()
                .values()
                .any(gx_adapter_mcp::ArgSource::is_do_result),
            "{name}'s inverse is keyed on the applied result"
        );
        assert!(
            !template
                .arguments()
                .values()
                .any(gx_adapter_mcp::ArgSource::is_prior),
            "{name} carries no prior member — which is the whole reason this arm exists"
        );
        println!("R20_DELETION_INVERSE fixture={name} tool={tool} member={member} parse=ok");
    }
}

/// 🔴 **The third family the gate must not refuse, and the one that was found by the floor rather
/// than by this suite** — an inverse that is a **constant**.
///
/// `notion:patch-page {in_trash: true}` is undone by `notion:patch-page {in_trash: false}`. No
/// prior is read and no result is keyed on: the other value of a flipped field is known from the
/// declaration itself. That is the entire reason [`gx_adapter_mcp::ArgSource::ConstJson`] exists
/// (**DR-V4B-2**, `req/38` §123 ruling 2, `req/189`).
///
/// 🔴 It is written here because the first spelling of this lane's gate — `is_prior()` or
/// `is_do_result()` — **refused it**, and nothing in this suite said so. `dr_v4b_2_const_json.rs`
/// said so, on the full workspace floor, which this lane had not run when the gate was written.
/// The audit did not catch it either: `req/291` §3 lists `const_json` under *families not tried*.
/// The arm is here so the third family has a gate of its own in the suite that owns the rule.
#[test]
fn a_declaration_whose_inverse_is_a_constant_needs_no_prior_either() {
    let file = br#"{
      "notion:patch-page": {
        "restored_by": "notion:patch-page",
        "arguments": {
          "page_id": { "forward": "page_id" },
          "in_trash": { "const_json": false },
          "reason": { "const": "gx undo of a trash" }
        }
      }
    }"#;
    let catalogue = Catalogue::from_json(file).unwrap_or_else(|e| {
        panic!(
            "🔴 `req/291` H-01's gate must not refuse the trash round trip: its inverse is a \
             constant the declaration supplies, which is material the forward call does not \
             carry. Refused with: {e}"
        )
    });
    let template = catalogue
        .spec_for("notion:patch-page")
        .expect("declared")
        .template()
        .expect("templated");
    let values: Vec<&gx_adapter_mcp::ArgSource> = template.arguments().values().collect();
    assert!(
        !values.iter().any(|s| s.is_prior() || s.is_do_result()),
        "the arm is vacuous unless this declaration carries neither a prior nor a do-result: \
         {values:?}"
    );
    println!(
        "R20_CONST_INVERSE tool=notion:patch-page parse=ok members={}",
        values.len()
    );
}

/// 🔴 And the two shipped declarations that **do** carry a prior still parse, so the arm above
/// cannot be satisfied by a gate that accepts everything.
#[test]
fn the_shipped_prior_carrying_fixtures_still_parse() {
    for name in [
        "github-restore-catalogue.json",
        "github16-p0-catalogue.json",
        "github16-p0-catalogue-unknown.json",
    ] {
        let catalogue = Catalogue::from_json(&fixture(name))
            .unwrap_or_else(|e| panic!("{name} must still parse: {e}"));
        println!(
            "R20_SHIPPED fixture={name} declared={}",
            catalogue.declared()
        );
    }
}
