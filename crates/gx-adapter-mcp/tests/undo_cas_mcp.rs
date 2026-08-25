// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-43-1, adopted (a)** on the MCP substrate — the CAS holds where the world is observable,
//! and where it is not, the absence is **declared and not refused** (DR-46-7).
//!
//! `req/38` §132 ruling 2 asks for the undo CAS on three adapters, and this is the third face of the
//! same fact. What makes it worth its own suite rather than a line in `crates/gx-cli/tests/` is that
//! MCP is the substrate where the two halves of the ruling can actually be told apart:
//!
//! * a server that answers `resources/read` has a position, and an undo of a resource somebody else
//!   moved is refused with the same `world-moved` row the fs and git adapters produce;
//! * a **tools-only** server has none (`req/38` §123: notion-mcp-server answers `initialize` with
//!   `{"tools":{}}`, so `snapshot` cannot run at all), and `req/38` §123 ruling 1 settled that this
//!   is written down honestly rather than turned into a refusal. `UndoWitness::Unobservable` is that
//!   sentence in the type system, and the arm below measures that it lets the undo through.
//!
//! The MCP world is moved with `FakeServer::write_behind_the_adapter`, which is `Fixture::disturb`'s
//! job: a write that never passed the adapter is exactly the third party `req/182` H-15 measured on
//! the filesystem.

mod support;

use std::path::Path;
use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence, Lifecycle, UndoWitness, Unobservable};
use support::{intent_for, subject_locator, FakeServer, RewindableLog, GOAL, SUBJECT, WRITE_TOOL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const UNDO_AT: Timestamp = Timestamp(1_754_000_100_000_000_000);

/// A pack that admits everything: this suite is about the CAS and not about the gate.
const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

const MOVED: &[u8] = b"a third party wrote this\n";

struct Wired {
    server: Arc<FakeServer>,
    engine: Engine<InjectedEvidence>,
}

fn wire(name: &str) -> Wired {
    let server = Arc::new(FakeServer::new());
    let adapter = gx_adapter_mcp::McpAdapter::new(server.clone())
        .with_catalogue(support::catalogue())
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
    engine.register_adapter(Arc::new(adapter), "gx-adapter-mcp undo_cas");
    Wired { server, engine }
}

fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-mcp-cas", &[11u8; 32])
}

/// Drive one change all the way to `Committed` and hand back its id.
fn commit_one(wired: &mut Wired) -> gx_core::TransformationId {
    let intent = intent_for(&subject_locator(), WRITE_TOOL, GOAL);
    wired.engine.submit(&intent, 42, AT).expect("submit");
    let id = wired.engine.plan(&intent, AT).expect("plan");
    assert_eq!(
        wired
            .engine
            .verify(&id, AT, &signing_key(), None)
            .expect("verify"),
        Lifecycle::Admitted
    );
    wired.engine.canonicalize(&id, AT, None).expect("T-8");
    assert_eq!(
        wired.engine.commit(&id, AT, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );
    id
}

/// 🔴 A resource somebody else wrote is not silently overwritten by the undo.
#[test]
fn a_resource_moved_behind_the_adapter_refuses_the_undo() {
    let mut wired = wire("undo_cas_mcp_moved");
    let id = commit_one(&mut wired);
    assert_eq!(
        wired.server.contents(SUBJECT).as_deref(),
        Some(GOAL),
        "the fixture has to have moved the world before this test means anything"
    );

    wired.server.write_behind_the_adapter(SUBJECT, MOVED);
    let witness = wired.engine.attested_postcondition(&id);
    assert!(
        matches!(witness, UndoWitness::Attested(_)),
        "the commit seated a receipt carrying 42 §3.10's postcondition, so there is something to \
         judge against: {witness:?}"
    );

    let refused = wired
        .engine
        .undo(&id, &witness, 43, UNDO_AT)
        .expect_err("DR-43-1(a): the world moved");
    let calls_before = wired.server.calls();
    println!(
        "MCP_UNDO_MOVED kind={} calls={calls_before} contents={:?}",
        refused.kind(),
        wired.server.contents(SUBJECT)
    );
    assert_eq!(
        refused.kind(),
        "WorldMoved",
        "43 §5.2's `world-moved` row, on the substrate `req/38` §123 measured the observability of: \
         {refused}"
    );
    assert_eq!(
        wired.server.contents(SUBJECT).as_deref(),
        Some(MOVED),
        "🔴 the third party's write is still there. `req/182` H-15's measurement on fs was that it \
         was not"
    );
    assert_eq!(
        calls_before, 1,
        "a refused undo makes no tool call — the one call is the forward commit's"
    );
    assert_eq!(
        wired.engine.state(&id),
        Some(Lifecycle::Committed),
        "and the original is untouched: no supersede edge was drawn"
    );
}

/// 🔴 **DR-46-7** — an unobservable face is *declared*, and the undo still runs.
///
/// The point of the ruling, measured: a tools-only server has no `resources/read`, so there is no
/// digest for anyone to attest and the fail-closed posture would refuse every undo on every such
/// server. `req/38` §123 ruling 1 chose the other side — write down that the comparison did not
/// happen — and this arm is what stops that from quietly becoming "refuse" the next time somebody
/// tightens the CAS. The world is moved first, so an implementation that judged anyway would fail
/// here rather than pass by accident.
#[test]
fn an_unobservable_face_is_declared_rather_than_refused() {
    let mut wired = wire("undo_cas_mcp_unobservable");
    let id = commit_one(&mut wired);
    wired.server.write_behind_the_adapter(SUBJECT, MOVED);

    // The discrimination, on one world: the *same* call refuses with a witness and proceeds
    // without one. Anything weaker would pass on an implementation that had no CAS at all.
    let attested = wired.engine.attested_postcondition(&id);
    let refused = wired
        .engine
        .undo(&id, &attested, 44, UNDO_AT)
        .expect_err("with a witness, this world is refused");

    let witness = UndoWitness::Unobservable(Unobservable::NoPostcondition);
    let (_, undoing) = wired
        .engine
        .undo(&id, &witness, 44, UNDO_AT)
        .expect("an unobservable face does not refuse");
    let state = wired
        .engine
        .verify(&undoing, UNDO_AT, &signing_key(), None)
        .expect("43 §5-2: an undo is not exempt from verification, unobservable or not");
    println!(
        "MCP_UNDO_UNOBSERVABLE refused={} candidate={undoing:?} verify_state={state:?} \
         contents={:?} reason={}",
        refused.kind(),
        wired.server.contents(SUBJECT),
        Unobservable::NoPostcondition.reason()
    );
    assert_eq!(
        refused.kind(),
        "WorldMoved",
        "the arm that establishes the discrimination: with a witness, this world refuses"
    );
    assert!(
        !matches!(state, Lifecycle::Denied),
        "the candidate the unobservable witness built walks its own pipeline; where it lands is the \
         gate's business and not this ruling's. 🔴 It lands on `Escalated` here rather than \
         `Admitted`, and that is E-M3-4 rather than DR-43-1: the undo of a resource somebody else \
         rewrote has no constructible inverse of its own, so `invert` answers `None` and a human is \
         asked. Measured, not assumed: {state:?}"
    );
    assert!(
        !Unobservable::NoPostcondition.reason().is_empty(),
        "every absence carries a sentence a caller can print — silence is what the type exists to \
         prevent"
    );
}
