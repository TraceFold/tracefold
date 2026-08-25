// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-15 + DR-46-14** (`req/38` §199 rulings 2 and 3, from the eighteenth adversarial
//! audit `req/269`) — an escrow is the prior of **the object the compare-and-set attests**, or it
//! is not an escrow.
//!
//! # The measurement this suite exists to make impossible again
//!
//! `req/268` opened the read-by-tool road: a restore declaration may name a **tool** to read its
//! prior with, for a server that publishes no resource for the object. `req/269` H-01 measured
//! what that left open, and the finding is not a corner case — it is the road's **only** real
//! deployment:
//!
//! * `snapshot`, `precondition` and the post-apply observation all read `position.resource()`.
//!   That is the object gx attests, and the object DR-43-1 refuses an undo over when the world
//!   moved.
//! * `invert`'s read-by-tool call carries **no locator**. The tool is the catalogue's and the
//!   arguments are built from the forward call, so the bytes escrowed come from whatever object
//!   that tool answered about.
//!
//! On the target this road was built for those two cannot be the same by accident: a gist has no
//! resource face, so a locator naming one is a locator `snapshot` refuses to plan on (the audit
//! measured that refusal verbatim), and the only deployment that runs at all is one whose locator
//! names a **different** object. The consequence the audit then measured is the accident this
//! product is a wedge in front of: **an undo overwrote a third party's write**, because the
//! compare-and-set was watching a file nobody had touched.
//!
//! `req/269` M-03 measured the same gap from the other side — with no predicate over the read's
//! *answer*, a read tool answering another object's document produced `true` and a restore call
//! carrying a stranger's text.
//!
//! # What closes it, in one predicate
//!
//! A [`gx_adapter_mcp::PriorRead`] now carries a required
//! [`gx_adapter_mcp::ObjectIdentity`]: the deployment declares how the read's own answer spells
//! the object **as this adapter's resource URI**, and `invert` requires that spelling to be the
//! locator. H-01 and M-03 are then the same refusal from two directions, and a declaration that
//! cannot say it does not parse.
//!
//! # What this suite does **not** measure (the denominator, stated here rather than found later)
//!
//! * **Zero live calls to github.** The fixture is in-process, for the reason
//!   `tests/support/mod.rs` gives (this crate ships a boundary, not a client).
//! * **The restore call's own target is still not bound.** This lane binds the escrowed bytes to
//!   the attested object. What a tool does with the arguments it is handed is the server's, and a
//!   declaration whose restore template names one object while its read names another is still a
//!   declaration-soundness burden (`req/38` §102 ruling 2's family). `docs/LIMITS.md` says so.
//! * **The CAS half of the read face is untouched.** `snapshot` and `precondition` still go
//!   through `resources/read`; a server with no resource face at all is still one `gx wrap`
//!   refuses to plan on (`tests/notion_page_catalogue.rs` carries that measurement).
//! * **No HTTP surface is driven here.** These arms are the engine API. The audit declared the
//!   same denominator and it is inherited, not closed.
//! * **The `identity` vocabulary's value space is not enumerated** — three part shapes are
//!   exercised, not every JSON value one could write.

mod support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    Admitted, ArgSource, Catalogue, IdentityPart, McpAdapter, McpDelta, ObjectIdentity,
    OnReadFailure, PointerSegment, PriorPointer, PriorRead, RestoreTemplate, Reversibility,
    ToolCall, ToolTransport, DECLARATION_UNSOUND_REFUSAL, OBJECT_IDENTITY_REFUSAL,
    READ_FAILURE_REFUSAL,
};
use gx_core::{Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, Lifecycle, UndoWitness};
use gx_substrate::{Error, Result, SubstrateAdapter};

use support::{intent_for, RewindableLog};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const UNDO_AT: Timestamp = Timestamp(1_754_000_100_000_000_000);

const SERVER: &str = "stdio://r17";

/// The object the read tool answers for. It has **no** resource face unless the fixture is asked
/// to give it one, which is the real server's shape (`req/265` §1).
const GIST: &str = "gist:g1";
/// A file the contents face does answer for, and which nothing in this suite ever writes. This is
/// the locator an H-01 deployment is forced onto: the only URI `snapshot` will answer for.
const ANCHOR: &str = "repo://o/r/refs/heads/main/contents/notes.txt";

const GIST_FILE: &str = "notes.md";
const GIST_BEFORE: &str = "the gist as it was\n";
const GIST_AFTER: &str = "the gist as the agent wants it\n";
const ANCHOR_BEFORE: &str = "the anchor file, which no arm in this suite writes\n";
const THIRD_PARTY: &str = "a third party's work, written after gx escrowed\n";

const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct Arrival {
    kind: &'static str,
    what: String,
}

/// An in-process server whose **tool face and resource face are two faces**.
///
/// 🔴 This is the one thing `req/268`'s fixture could not express and the reason its P-6 measured
/// "a third party's write refuses the undo" while the road was open: there, `set_gist` wrote the
/// tool face and the resource face in the same breath, so a world where the two disagree could not
/// be built. Here `lockstep` is a switch, and it is **off** for the H-01 shape.
#[derive(Debug)]
struct R17Server {
    resources: Mutex<BTreeMap<String, Vec<u8>>>,
    gist_files: Mutex<BTreeMap<String, String>>,
    /// What `get_gist` puts at `/id`. The audit's M-03 arm moves it.
    gist_id: Mutex<String>,
    /// Whether a write through `update_gist` also moves the gist's resource face.
    lockstep: bool,
    log: Mutex<Vec<Arrival>>,
}

impl R17Server {
    /// The H-01 world: the gist has a tool face and **no** resource face, and only `ANCHOR` is
    /// readable through `resources/read`.
    fn split_faces() -> Self {
        let server = Self {
            resources: Mutex::new(BTreeMap::new()),
            gist_files: Mutex::new(BTreeMap::new()),
            gist_id: Mutex::new("g1".to_string()),
            lockstep: false,
            log: Mutex::new(Vec::new()),
        };
        server.put_resource(ANCHOR, ANCHOR_BEFORE.as_bytes());
        server.put_gist(GIST_FILE, GIST_BEFORE);
        server
    }

    /// The world `req/268`'s fixture modelled: the gist has both faces and they move together.
    /// The control every refusal in this suite is a discrimination against.
    fn locked_faces() -> Self {
        let server = Self {
            resources: Mutex::new(BTreeMap::new()),
            gist_files: Mutex::new(BTreeMap::new()),
            gist_id: Mutex::new("g1".to_string()),
            lockstep: true,
            log: Mutex::new(Vec::new()),
        };
        server.put_resource(ANCHOR, ANCHOR_BEFORE.as_bytes());
        server.put_resource(GIST, GIST_BEFORE.as_bytes());
        server.put_gist(GIST_FILE, GIST_BEFORE);
        server
    }

    fn put_resource(&self, uri: &str, bytes: &[u8]) {
        self.resources
            .lock()
            .expect("not poisoned")
            .insert(uri.to_string(), bytes.to_vec());
    }

    fn put_gist(&self, filename: &str, contents: &str) {
        self.gist_files
            .lock()
            .expect("not poisoned")
            .insert(filename.to_string(), contents.to_string());
        if self.lockstep {
            self.put_resource(GIST, contents.as_bytes());
        }
    }

    /// A write nobody told gx about. In the split-faces world this moves the tool face **only**,
    /// which is exactly the state a `resources/read` compare-and-set cannot see.
    fn third_party_writes_gist(&self, contents: &str) {
        self.put_gist(GIST_FILE, contents);
    }

    fn answer_for_another_object(&self) {
        *self.gist_id.lock().expect("not poisoned") = "SOMEONE-ELSES-GIST".to_string();
    }

    fn gist_content(&self) -> String {
        self.gist_files
            .lock()
            .expect("not poisoned")
            .get(GIST_FILE)
            .cloned()
            .unwrap_or_default()
    }

    fn locator(resource: &str) -> String {
        format!("{SERVER}#{resource}")
    }

    fn note(&self, kind: &'static str, what: impl Into<String>) {
        self.log.lock().expect("not poisoned").push(Arrival {
            kind,
            what: what.into(),
        });
    }

    fn arrivals(&self) -> Vec<Arrival> {
        self.log.lock().expect("not poisoned").clone()
    }

    fn count(&self, kind: &str) -> usize {
        self.arrivals().iter().filter(|a| a.kind == kind).count()
    }

    fn clear_log(&self) {
        self.log.lock().expect("not poisoned").clear();
    }

    /// `get_gist`'s answer: the document the real tool answers, `id` included -- which is the
    /// member a bound `identity` reads.
    fn gist_document(&self) -> String {
        let files = self.gist_files.lock().expect("not poisoned");
        let mut members = serde_json::Map::new();
        for (name, contents) in files.iter() {
            members.insert(
                name.clone(),
                serde_json::json!({ "filename": name, "content": contents }),
            );
        }
        serde_json::json!({
            "id": *self.gist_id.lock().expect("not poisoned"),
            "description": "a description no restore call may send as the content",
            "files": serde_json::Value::Object(members),
        })
        .to_string()
    }
}

impl ToolTransport for R17Server {
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>> {
        self.note("read", resource);
        if server != SERVER {
            return Err(Error::Unreadable {
                locator: Self::locator(resource),
                detail: format!("this fixture is one server ({SERVER}) and that is another"),
            });
        }
        self.resources
            .lock()
            .expect("not poisoned")
            .get(resource)
            .cloned()
            .ok_or_else(|| Error::Unreadable {
                locator: Self::locator(resource),
                detail: "this server publishes no resource at that URI".to_string(),
            })
    }

    fn read_prior_by_tool(&self, server: &str, tool: &str, arguments: &[u8]) -> Result<Vec<u8>> {
        let parsed: serde_json::Value =
            serde_json::from_slice(arguments).unwrap_or(serde_json::Value::Null);
        self.note("read_by_tool", format!("{tool} {parsed}"));
        if server != SERVER {
            return Err(Error::Unreadable {
                locator: server.to_string(),
                detail: "another server".to_string(),
            });
        }
        match tool {
            "get_gist" => {
                let object = parsed.as_object().ok_or_else(|| Error::Unreadable {
                    locator: format!("{server}#tool:{tool}"),
                    detail: "the read arguments are not an object".to_string(),
                })?;
                // `req/152` §5's strict posture: an argument the tool does not declare is refused,
                // so an escrow that "worked" cannot be the fixture being lax.
                for member in object.keys() {
                    if member != "gist_id" {
                        return Err(Error::Unreadable {
                            locator: format!("{server}#tool:{tool}"),
                            detail: format!(
                                "body failed validation: body.{member} should be not present"
                            ),
                        });
                    }
                }
                if object.get("gist_id").and_then(serde_json::Value::as_str) != Some("g1") {
                    return Err(Error::Unreadable {
                        locator: format!("{server}#tool:{tool}"),
                        detail: "no such gist".to_string(),
                    });
                }
                Ok(self.gist_document().into_bytes())
            }
            other => Err(Error::Unreadable {
                locator: format!("{server}#tool:{other}"),
                detail: format!("this server publishes no tool called {other:?}"),
            }),
        }
    }

    fn call(&self, call: &ToolCall, admitted: &Admitted) -> Result<Vec<u8>> {
        assert_eq!(
            call.delta(),
            admitted.delta(),
            "a transport may check the pair, and this one does"
        );
        let arguments: serde_json::Value =
            serde_json::from_slice(call.arguments()).map_err(|e| Error::ApplyFailed {
                detail: format!("the arguments of {:?} are not JSON: {e}", call.tool()),
            })?;
        self.note("call", format!("{} {arguments}", call.tool()));
        let member = |name: &str| {
            arguments
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        match call.tool() {
            "update_gist" => {
                let filename = member("filename").ok_or_else(|| Error::ApplyFailed {
                    detail: "update_gist has no `filename`".to_string(),
                })?;
                let content = member("content").ok_or_else(|| Error::ApplyFailed {
                    detail: "update_gist has no `content`".to_string(),
                })?;
                self.put_gist(&filename, &content);
                Ok(br#"{"id":"g1","url":"https://api.github.com/gists/g1"}"#.to_vec())
            }
            other => Err(Error::ApplyFailed {
                detail: format!("this server publishes no tool called {other:?}"),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------

/// The identity every sound declaration in this suite carries: "the object this read answered
/// about is spelled `gist:` followed by the `id` the answer itself carries".
fn sound_identity() -> ObjectIdentity {
    ObjectIdentity::new(vec![
        IdentityPart::Literal("gist:".to_string()),
        IdentityPart::Answer {
            answer: "/id".to_string(),
        },
    ])
}

fn bound_pointer() -> ArgSource {
    ArgSource::PriorJson(PriorPointer::Bound(vec![
        PointerSegment::Literal("/files/".to_string()),
        PointerSegment::Forward {
            forward: "filename".to_string(),
        },
        PointerSegment::Literal("/content".to_string()),
    ]))
}

fn catalogue_with(identity: ObjectIdentity, posture: OnReadFailure) -> Catalogue {
    Catalogue::new()
        .with_restore_template(
            "update_gist",
            "update_gist",
            RestoreTemplate::new()
                .with("gist_id", ArgSource::Forward("gist_id".to_string()))
                .with("filename", ArgSource::Forward("filename".to_string()))
                .with("content", bound_pointer()),
        )
        .with_prior_read(
            "update_gist",
            PriorRead::new(
                "get_gist",
                RestoreTemplate::new().with("gist_id", ArgSource::Forward("gist_id".to_string())),
                identity,
            ),
        )
        .with_on_read_failure(posture)
}

struct Wired {
    server: Arc<R17Server>,
    adapter: McpAdapter,
    engine: Engine<InjectedEvidence>,
}

fn wire(name: &str, server: R17Server, catalogue: Catalogue) -> Wired {
    let server = Arc::new(server);
    let adapter = McpAdapter::new(server.clone())
        .with_catalogue(catalogue)
        .with_log(Arc::new(RewindableLog::new()));
    let gate = gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(PERMIT_ALL).expect("the fixture policy set parses"),
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    let mut engine = Engine::open(dir.join("journal.bin"), gate, InjectedEvidence::none())
        .expect("a fresh journal");
    engine.register_adapter(Arc::new(adapter.clone()), "gx-adapter-mcp r17");
    Wired {
        server,
        adapter,
        engine,
    }
}

fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-r17", &[23u8; 32])
}

fn gist_arguments(filename: &str, content: &str) -> Vec<u8> {
    serde_json::json!({ "gist_id": "g1", "filename": filename, "content": content })
        .to_string()
        .into_bytes()
}

fn commit(
    wired: &mut Wired,
    resource: &str,
    arguments: &[u8],
) -> std::result::Result<TransformationId, String> {
    let locator = R17Server::locator(resource);
    let intent = intent_for(&locator, "update_gist", arguments);
    wired
        .engine
        .submit(&intent, 42, AT)
        .map_err(|e| format!("submit: {e}"))?;
    let id = wired
        .engine
        .plan(&intent, AT)
        .map_err(|e| format!("plan: {e}"))?;
    let verdict = wired
        .engine
        .verify(&id, AT, &signing_key(), None)
        .map_err(|e| format!("verify: {e}"))?;
    if verdict != Lifecycle::Admitted {
        return Err(format!("verify landed on {verdict:?}"));
    }
    wired
        .engine
        .canonicalize(&id, AT, None)
        .map_err(|e| format!("canonicalize: {e}"))?;
    match wired.engine.commit(&id, AT, &signing_key()) {
        Ok(Lifecycle::Committed) => Ok(id),
        Ok(other) => Err(format!("commit landed on {other:?}")),
        Err(e) => Err(format!("commit: {e}")),
    }
}

fn undo_to_committed(wired: &mut Wired, id: &TransformationId) -> std::result::Result<(), String> {
    let witness = wired.engine.attested_postcondition(id);
    let (_, undoing) = wired
        .engine
        .undo(id, &witness, 43, UNDO_AT)
        .map_err(|e| format!("undo: {e}"))?;
    let state = wired
        .engine
        .verify(&undoing, UNDO_AT, &signing_key(), None)
        .map_err(|e| format!("undo verify: {e}"))?;
    if state != Lifecycle::Admitted {
        return Err(format!("the undo candidate landed on {state:?}"));
    }
    wired
        .engine
        .canonicalize(&undoing, UNDO_AT, None)
        .map_err(|e| format!("undo canonicalize: {e}"))?;
    match wired.engine.commit(&undoing, UNDO_AT, &signing_key()) {
        Ok(Lifecycle::Committed) => Ok(()),
        Ok(other) => Err(format!("the undo commit landed on {other:?}")),
        Err(e) => Err(format!("undo commit: {e}")),
    }
}

// ===========================================================================
// The four the ruling named
// ===========================================================================

/// 🔴 **R-1 (H-01, DR-46-15)** — a declaration whose read answers about an object the locator does
/// **not** name is refused, and the effect never leaves.
///
/// This is the audit's own deployment, rebuilt: the gist has no resource face, so the locator is
/// forced onto `ANCHOR` (the only URI `snapshot` will answer for) while the escrow reads the gist.
/// On the binary this lane replaces, that shape committed, escrowed the gist's bytes, and then let
/// an undo overwrite a third party's write — because the compare-and-set was quantified over
/// `ANCHOR`, which nobody had touched.
///
/// Red condition: a commit goes through, or a `tools/call` arrives, or the refusal is not the
/// constant `docs/LIMITS.md` and `catalogue.rs` both point at.
#[test]
fn r1_a_read_about_another_object_than_the_locator_refuses_the_effect() {
    let mut wired = wire(
        "r17_r1",
        R17Server::split_faces(),
        catalogue_with(sound_identity(), OnReadFailure::Refuse),
    );
    wired.server.clear_log();
    let outcome = commit(&mut wired, ANCHOR, &gist_arguments(GIST_FILE, GIST_AFTER));
    let calls = wired.server.count("call");

    // And the world the old road would have lost: a third party writes the gist, and there is no
    // committed transformation whose undo could reach it.
    wired.server.third_party_writes_gist(THIRD_PARTY);

    println!(
        "R17_R1 outcome={outcome:?} calls={calls} gist={:?} arrivals={:?}",
        wired.server.gist_content(),
        wired.server.arrivals()
    );
    let refusal = outcome.expect_err("an unbound escrow committed");
    assert!(
        refusal.contains(OBJECT_IDENTITY_REFUSAL),
        "the refusal is not the constant DR-46-15 fixed:\n{refusal}"
    );
    assert!(
        refusal.contains("gist:g1") && refusal.contains(ANCHOR),
        "the refusal names neither of the two objects it is about, which is the half an operator \
         debugs with:\n{refusal}"
    );
    assert_eq!(
        calls,
        0,
        "the effect reached the server anyway: {:?}",
        wired.server.arrivals()
    );
    assert_eq!(
        wired.server.gist_content(),
        THIRD_PARTY,
        "the third party's bytes are the only thing that moved, and nothing gx did touched them"
    );
}

/// 🔴 **R-2 (M-03, DR-46-15)** — a read tool that answers with **another object's document** is
/// refused, and its text never becomes an inverse.
///
/// The audit measured the old answer exactly: verdict `true`, and a restore call built as
/// `{"gist_id":"g1","filename":"notes.md","content":"a different object's text"}` — a stranger's
/// body, addressed to this object, reported as reversible. The identity predicate is what makes
/// "the pointer resolved" stop being the same thing as "the read was about this object".
///
/// Run on the **locked-faces** fixture, so the only thing wrong is the answer: this arm cannot be
/// R-1 wearing another name.
#[test]
fn r2_a_read_answering_for_another_document_is_refused_not_escrowed() {
    let server = R17Server::locked_faces();
    server.answer_for_another_object();
    let wired = wire(
        "r17_r2",
        server,
        catalogue_with(sound_identity(), OnReadFailure::Refuse),
    );
    let locator = R17Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let delta = wired
        .adapter
        .plan(
            &intent_for(
                &locator,
                "update_gist",
                &gist_arguments(GIST_FILE, GIST_AFTER),
            ),
            &pre,
        )
        .expect("a well-formed call plans");
    let verdict = wired.adapter.reversibility(&delta, &pre);
    let inverse = wired
        .adapter
        .invert(&delta, &pre)
        .map(gx_substrate::InvertOutcome::into_inverse);
    println!(
        "R17_R2 verdict={:?} inverse={:?}",
        verdict.as_ref().map(|v| v.as_str()),
        inverse.as_ref().map(Option::is_some)
    );
    let refusal = verdict.expect_err("a stranger's document was accepted as this object's prior");
    assert_eq!(refusal.kind(), "Unreadable");
    let text = refusal.to_string();
    assert!(
        text.contains(OBJECT_IDENTITY_REFUSAL),
        "the refusal is not DR-46-15's constant:\n{text}"
    );
    assert!(
        text.contains("SOMEONE-ELSES-GIST"),
        "the refusal does not name the object the read actually answered for:\n{text}"
    );
    assert!(
        inverse.is_err(),
        "an inverse was built from a document about another object"
    );
}

/// 🔴 **R-3 (M-05)** — a failure the **declaration** caused says so, in its own words, and calls
/// nothing.
///
/// The audit's finding, verbatim in shape: a read declaration naming a prior member is a catalogue
/// the operator wrote wrong, and the old road answered it with the *read-failure* sentence — "make
/// the declared read face answer, or add `"$on_read_failure": "unknown"`" — about a face that had
/// never been called (**zero arrivals**), while the second remedy was executable and turned the
/// typo into a permanent `unknown` on every call.
///
/// Red condition: the sentence carries the relaxation, or the read-failure constant, or the true
/// cause is not first, or a read reached the server.
#[test]
fn r3_a_declaration_derived_failure_is_its_own_sentence_and_calls_nothing() {
    let unsound = Catalogue::new()
        .with_restore_template(
            "update_gist",
            "update_gist",
            RestoreTemplate::new()
                .with("gist_id", ArgSource::Forward("gist_id".to_string()))
                .with("filename", ArgSource::Forward("filename".to_string()))
                .with("content", bound_pointer()),
        )
        .with_prior_read(
            "update_gist",
            PriorRead::new(
                "get_gist",
                // The mistake: a read declaring that one of its own arguments comes from the prior
                // it is being called to produce.
                RestoreTemplate::new().with("gist_id", ArgSource::PriorContentsUtf8),
                sound_identity(),
            ),
        );
    let wired = wire("r17_r3", R17Server::locked_faces(), unsound);
    let locator = R17Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let delta = wired
        .adapter
        .plan(
            &intent_for(
                &locator,
                "update_gist",
                &gist_arguments(GIST_FILE, GIST_AFTER),
            ),
            &pre,
        )
        .expect("a well-formed call plans");
    wired.server.clear_log();
    let refused = wired
        .adapter
        .invert(&delta, &pre)
        .expect_err("an unsound declaration was allowed to run");
    let text = refused.to_string();
    let reads = wired.server.count("read_by_tool");
    println!("R17_R3 read_by_tool={reads} refusal={text}");
    assert_eq!(
        reads,
        0,
        "the server was called for a declaration that never got as far as an argument: {:?}",
        wired.server.arrivals()
    );
    assert!(
        text.contains(DECLARATION_UNSOUND_REFUSAL),
        "the refusal is not the declaration constant:\n{text}"
    );
    assert!(
        !text.contains("$on_read_failure"),
        "the refusal offers the relaxation as a remedy for a typo, which is how a typo becomes a \
         permanent `unknown`:\n{text}"
    );
    assert!(
        !text.contains(READ_FAILURE_REFUSAL),
        "a declaration failure is still wearing the read-failure sentence:\n{text}"
    );
    let cause = "the read declaration's member \"gist_id\" draws from the prior contents";
    let cause_at = text.find(cause).expect("the true cause is in the sentence");
    let remedy_at = text
        .find(DECLARATION_UNSOUND_REFUSAL)
        .expect("the remedy is in the sentence");
    assert!(
        cause_at < remedy_at,
        "the true cause is not first, which is the whole of the M-05 complaint:\n{text}"
    );
}

/// 🔴 **R-4 (M-05, the parse half + DR-46-15's required member)** — a declaration that can be
/// judged without a call is judged before `gx wrap` starts.
///
/// `req/269` M-05 (iii): the unsound forms above were caught at *resolution* time, so the session
/// started, the agent ran, and the first guarded write was the moment anyone found out. Four
/// shapes are refused at parse and the shipped catalogue is the control.
#[test]
fn r4_an_unsound_read_declaration_never_reaches_a_running_session() {
    let entry = |read_by: &str| {
        format!(
            r#"{{"update_gist":{{"restored_by":"update_gist","read_by":{read_by},
                "arguments":{{"content":{{"prior_json":"/files/notes.md/content"}}}}}}}}"#
        )
    };
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "a read declaration with no identity at all",
            entry(r#"{"by_tool":"get_gist","arguments":{"gist_id":{"forward":"gist_id"}}}"#),
            "identity",
        ),
        (
            "an identity that names no member of the answer",
            entry(
                r#"{"by_tool":"get_gist","arguments":{"gist_id":{"forward":"gist_id"}},
                    "identity":["gist:",{"forward":"gist_id"}]}"#,
            ),
            "names no `answer` member",
        ),
        (
            "an empty identity",
            entry(
                r#"{"by_tool":"get_gist","arguments":{"gist_id":{"forward":"gist_id"}},
                    "identity":[]}"#,
            ),
            "is empty",
        ),
        (
            "a read whose own argument draws from the prior",
            entry(
                r#"{"by_tool":"get_gist","arguments":{"gist_id":"prior_contents_utf8"},
                    "identity":[{"answer":"/id"}]}"#,
            ),
            "draws from the prior contents",
        ),
    ];
    let mut answers = Vec::new();
    for (what, bytes, needle) in &cases {
        let parsed = Catalogue::from_json(bytes.as_bytes());
        let why = parsed
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| "PARSED".to_string());
        answers.push((*what, why.clone()));
        assert!(
            parsed.is_err(),
            "{what} started a session: {}",
            bytes.replace('\n', " ")
        );
        assert!(
            why.contains(needle),
            "{what} was refused, but not for the reason a reader can act on ({needle:?}): {why}"
        );
    }
    // The control: the shipped P0 catalogue, whose read declaration is sound, still parses.
    let shipped = include_bytes!("fixtures/github16-p0-catalogue.json");
    let ok = Catalogue::from_json(shipped).expect("the shipped catalogue still parses");
    println!(
        "R17_R4 refusals={answers:?} shipped_declared={}",
        ok.declared()
    );
    assert_eq!(
        ok.declared(),
        2,
        "the control moved, so the four refusals above measure a parser that refuses everything"
    );
}

// ===========================================================================
// The rest of the ruling, and the controls that make the refusals mean something
// ===========================================================================

/// 🔴 **R-5 (DR-46-14)** — a bound pointer's substituted segment is **escaped**, so a member name
/// carrying RFC 6901's two special characters cannot widen the pointer it lands in.
///
/// `~` and `/` are the pointer grammar's own characters. A filename of `a/b` substituted raw would
/// address `…/files/a/b/content` — two levels down a document that has one member called `a/b` —
/// and a filename of `~0` would address a member spelled `~`. Both are the same class of mistake
/// as an unescaped path separator anywhere else, and this arm is the one that fails when the
/// escaping is dropped.
#[test]
fn r5_a_bound_pointer_escapes_the_member_it_substitutes() {
    let odd = "a/b~c";
    let server = R17Server::locked_faces();
    server.put_gist(odd, "the odd member's own text\n");
    let wired = wire(
        "r17_r5",
        server,
        catalogue_with(sound_identity(), OnReadFailure::Refuse),
    );
    let locator = R17Server::locator(GIST);
    let pre = support::absent_snapshot(&locator);
    let delta = wired
        .adapter
        .plan(
            &intent_for(&locator, "update_gist", &gist_arguments(odd, "x")),
            &pre,
        )
        .expect("a well-formed call plans");
    let inverse = wired
        .adapter
        .invert(&delta, &pre)
        .expect("the read face answers")
        .into_inverse()
        .expect("the escaped pointer resolved");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter wrote it");
    let op = decoded.ops().first().expect("one op");
    let restore: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("the template resolved to JSON");
    println!(
        "R17_R5 member={odd:?} escaped={:?} restore={restore}",
        gx_adapter_mcp::escape_pointer_token(odd)
    );
    assert_eq!(
        gx_adapter_mcp::escape_pointer_token(odd),
        "a~1b~0c",
        "RFC 6901 §3: `~` becomes `~0` first, then `/` becomes `~1`"
    );
    assert_eq!(
        restore.get("content").and_then(serde_json::Value::as_str),
        Some("the odd member's own text\n"),
        "the pointer addressed something other than the member the call named"
    );
}

/// 🔴 **R-6 (DR-46-15 x DR-46-12)** — the identity refusal honours the posture the deployment
/// declared, and the relaxation still never answers `true`.
///
/// An unbound escrow is, from the escrow's side, the same fact a failed read is: the prior this
/// transformation needed did not arrive. So it takes the answer DR-46-12 already ruled — refuse by
/// default, `unknown` in writing — rather than a third posture nobody configured. What it may
/// never do is claim an inverse it does not hold.
#[test]
fn r6_the_identity_refusal_takes_the_declared_posture_and_never_says_true() {
    let mut answers = Vec::new();
    for posture in [OnReadFailure::Refuse, OnReadFailure::Unknown] {
        let wired = wire(
            match posture {
                OnReadFailure::Refuse => "r17_r6_refuse",
                OnReadFailure::Unknown => "r17_r6_unknown",
            },
            R17Server::split_faces(),
            catalogue_with(sound_identity(), posture),
        );
        let locator = R17Server::locator(ANCHOR);
        let pre = support::absent_snapshot(&locator);
        let delta = wired
            .adapter
            .plan(
                &intent_for(
                    &locator,
                    "update_gist",
                    &gist_arguments(GIST_FILE, GIST_AFTER),
                ),
                &pre,
            )
            .expect("a well-formed call plans");
        answers.push(match wired.adapter.reversibility(&delta, &pre) {
            Ok(verdict) => verdict.as_str().to_string(),
            Err(e) => format!("refused:{}", e.kind()),
        });
        assert!(
            wired
                .adapter
                .invert(&delta, &pre)
                .ok()
                .and_then(gx_substrate::InvertOutcome::into_inverse)
                .is_none(),
            "an inverse was escrowed out of a prior that was never this object's"
        );
    }
    println!(
        "R17_R6 answers={answers:?} posture_words={:?}",
        [
            OnReadFailure::Refuse.as_str(),
            OnReadFailure::Unknown.as_str()
        ]
    );
    assert_eq!(answers, vec!["refused:Unreadable", "unknown"]);
    assert_eq!(
        Reversibility::ALL,
        ["true", "false", "unknown"],
        "C-25's three values, in the order 11 §5-2 states them"
    );
}

/// 🔴 **R-7 (the control)** — a deployment whose read **is** about the attested object still
/// commits, still undoes, and still comes back byte for byte.
///
/// Without this arm every refusal above could be a road that refuses everything. With it, the
/// difference between R-1 and this one is a single fact: whether the locator names the object the
/// read answered for.
#[test]
fn r7_a_bound_deployment_still_round_trips_byte_for_byte() {
    let mut wired = wire(
        "r17_r7",
        R17Server::locked_faces(),
        catalogue_with(sound_identity(), OnReadFailure::Refuse),
    );
    let before = wired.server.gist_content();
    let id = commit(&mut wired, GIST, &gist_arguments(GIST_FILE, GIST_AFTER))
        .expect("a bound declaration commits");
    let moved = wired.server.gist_content();
    undo_to_committed(&mut wired, &id).expect("the escrowed inverse runs");
    let after = wired.server.gist_content();
    println!("R17_R7 before={before:?} moved={moved:?} after={after:?}");
    assert_eq!(moved, GIST_AFTER, "the forward call has to move the world");
    assert_eq!(
        before.as_bytes(),
        after.as_bytes(),
        "the undo did not put the prior bytes back, byte for byte"
    );
}

/// 🔴 **R-8 (the control's other half)** — with both faces moving together, a third party's write
/// still refuses the undo rather than being overwritten.
///
/// `req/268` P-6 measured this and it was true; what the audit showed is that it was true *of the
/// fixture*, whose one write moved both faces. Here it is measured beside R-1, so the pair says
/// what P-6 alone could not: the compare-and-set works exactly when the escrow and the attested
/// object are the same object, and R-1 is the arm that now refuses when they are not.
#[test]
fn r8_a_third_party_write_still_refuses_the_undo_when_the_faces_agree() {
    let mut wired = wire(
        "r17_r8",
        R17Server::locked_faces(),
        catalogue_with(sound_identity(), OnReadFailure::Refuse),
    );
    let id = commit(&mut wired, GIST, &gist_arguments(GIST_FILE, GIST_AFTER))
        .expect("a bound declaration commits");
    wired.server.third_party_writes_gist(THIRD_PARTY);
    let witness = wired.engine.attested_postcondition(&id);
    assert!(
        matches!(witness, UndoWitness::Attested(_)),
        "the locked-faces fixture has a postcondition to attest: {witness:?}"
    );
    let refused = wired
        .engine
        .undo(&id, &witness, 44, UNDO_AT)
        .expect_err("DR-43-1(a): the world moved");
    println!(
        "R17_R8 kind={} gist={:?}",
        refused.kind(),
        wired.server.gist_content()
    );
    assert_eq!(refused.kind(), "WorldMoved");
    assert_eq!(
        wired.server.gist_content(),
        THIRD_PARTY,
        "the third party's bytes were disturbed by a refused undo"
    );
}

/// 🔴 **R-9 (`req/269` L-01)** — the arrival count for a whole do-and-undo round trip, which is
/// the number `docs/LIMITS.md` now carries.
///
/// The published cost line counted the forward half alone: two escrow reads per guarded call,
/// because `invert` runs at T-3 and again at T-10b. An undo is itself a guarded transformation, so
/// it builds its own inverse twice on the same road. The measured total for one do-and-undo is
/// therefore **four** reads and **two** effects, and this arm is what keeps the page honest.
#[test]
fn r9_a_do_and_undo_round_trip_costs_four_escrow_reads_and_two_effects() {
    let mut wired = wire(
        "r17_r9",
        R17Server::locked_faces(),
        catalogue_with(sound_identity(), OnReadFailure::Refuse),
    );
    wired.server.clear_log();
    let id = commit(&mut wired, GIST, &gist_arguments(GIST_FILE, GIST_AFTER))
        .expect("a bound declaration commits");
    let after_forward = wired.server.count("read_by_tool");
    undo_to_committed(&mut wired, &id).expect("the escrowed inverse runs");
    let after_undo = wired.server.count("read_by_tool");
    let effects = wired.server.count("call");
    println!(
        "R17_R9 read_by_tool_after_forward={after_forward} read_by_tool_after_undo={after_undo} \
         effects={effects}"
    );
    assert_eq!(
        after_forward,
        2,
        "the forward half is T-3 plus T-10b: {:?}",
        wired.server.arrivals()
    );
    assert_eq!(
        after_undo,
        4,
        "the undo half is the same two again, and `docs/LIMITS.md` says four: {:?}",
        wired.server.arrivals()
    );
    assert_eq!(effects, 2, "one do and one undo, and nothing else");
}

/// 🔴 **R-10 (`req/269` M-01)** — the catalogue-wide posture is printed where an operator sees it.
///
/// The audit's complaint was not that the slot is catalogue-wide (that is a design choice the
/// ruling kept); it was that **no surface printed it**, so one line could tip a whole file to the
/// fail-open side and nothing would say so. `gx wrap`'s start-up JSON now carries the word.
///
/// 🔴 **LIMIT, declared**: the half below is a **text gate** over `gx-cli`'s source, the shape
/// `tests/ac_051.rs` D-2 uses. It measures that the member is written into the start-up object; it
/// does not run the binary. The value it prints is `OnReadFailure::as_str`, which the first half
/// exercises directly.
#[test]
fn r10_the_catalogue_wide_posture_has_a_word_and_a_surface() {
    assert_eq!(OnReadFailure::Refuse.as_str(), "refuse");
    assert_eq!(OnReadFailure::Unknown.as_str(), "unknown");
    assert_eq!(
        Catalogue::new().on_read_failure().as_str(),
        "refuse",
        "the default is the conservative one and the word says so"
    );
    assert_eq!(
        Catalogue::new()
            .with_on_read_failure(OnReadFailure::Unknown)
            .on_read_failure()
            .as_str(),
        "unknown"
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-adapter-mcp sits at <root>/crates/gx-adapter-mcp")
        .to_path_buf();
    let wrap = std::fs::read_to_string(root.join("crates/gx-cli/src/wrap.rs"))
        .expect("crates/gx-cli/src/wrap.rs is readable");
    println!(
        "R17_R10 wrap_bytes={} names_posture={} LIMIT=text-gate(the binary is not run here)",
        wrap.len(),
        wrap.contains("\"on_read_failure\": catalogue.on_read_failure().as_str()")
    );
    assert!(
        wrap.contains("\"on_read_failure\": catalogue.on_read_failure().as_str()"),
        "`gx wrap`'s start-up line does not print the posture it is running under, which is what \
         made a catalogue-wide relaxation invisible in operation"
    );
}
