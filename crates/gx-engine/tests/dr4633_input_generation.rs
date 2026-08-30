// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-33** — the input-generation stage a deployment declares, joined with the actor at
//! plan time, journalled as its result, and reproduced by a rebuild.
//!
//! `req/38` §413 rules the wiring: a **separate optional trait** (`InputStageDeclaration`, N-08
//! untouched) carries a deployment's declaration into the engine; the engine joins it with the
//! transformation's `Actor` — an `Actor::Agent` is an LLM origin whatever a static file declared —
//! and journals the **join's result** on the `Planned` record. `attested_boundary` then reads that
//! one value back on the live road and on both rebuild roads, so they agree without the actor
//! (which is not in Σ) or the catalogue (which `gx-engine` does not depend on).
//!
//! | form | what it measures |
//! |---|---|
//! | ① declaration reaches the receipt | a human-actor commit under a `deterministic_replay` declaration attests it |
//! | ② no declaration is v0, unchanged | an unregistered substrate attests `unknown`, and the rebuild reproduces it |
//! | ③ the value is load-bearing (**the negative control**) | flipping the journalled stage by one value refuses the rebuild; keeping it files it |
//! | ④ the actor overrides the declaration | an `Actor::Agent` attests `llm_originated` whatever the declaration said |
//!
//! Form ③ is the one the DR rests on: the *only* difference between the two beds is
//! `Planned.input_generation`, and one reissue is filed while the other is refused — the proof that
//! a rebuild reproducing a different input stage would not reproduce the leaf (43 §7-3b), which is
//! the same shape `dr4634_read_set_absence.rs` uses one erratum over.
//!
//! N-13 keeps adapters out of this crate, so the fixture implements `gx-substrate`'s contracts
//! directly, the way `dr4634_read_set_absence.rs` does.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gx_core::{
    Actor, BoundaryStage, ChangeContext, Fingerprint, GoalBytes, Intent, SubstrateKind, Timestamp,
    TransformationId,
};
use gx_engine::pipeline::Reissued;
use gx_engine::{Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle};
use gx_substrate::{InputStageDeclaration, InvertOutcome, PlannedDelta, SubstrateAdapter};
use gx_witness::receipt::ReceiptPayload;

use support::{copy_tree, digest_of, gate, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";

// ---------------------------------------------------------------------------
// The fixture: an fs-shaped adapter that also declares an input-generation stage
// ---------------------------------------------------------------------------

/// A read-free shape (🔴 **DEFECT-892-1**, `req/895` §1: this used to say "the fs/git/postgres
/// shape (`InvertOutcome::from_option` fixes `Vec::new()`)" — those adapters do read, and
/// `from_option` is gone; this fixture holds its world in memory and reads no substrate), plus the optional
/// `InputStageDeclaration` DR-46-33 wires. `declared` is the deployment's own statement, held so
/// the test can register a `deterministic_replay` deployment and an `unknown` one apart.
#[derive(Clone, Debug)]
struct DeclaringAdapter {
    world: Arc<Mutex<Vec<u8>>>,
    declared: BoundaryStage,
}

impl DeclaringAdapter {
    fn new(world: &str, declared: BoundaryStage) -> Self {
        Self {
            world: Arc::new(Mutex::new(world.as_bytes().to_vec())),
            declared,
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for DeclaringAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
        let world = self.world();
        Ok(gx_core::ObjectSnapshot::new(
            gx_core::ObjectId(digest_of(locator.as_bytes())),
            SubstrateKind::Fs,
            locator.to_string(),
            digest_of(&world),
            gx_core::ReprKind::Bytes,
        ))
    }

    fn plan(
        &self,
        intent: &Intent,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<PlannedDelta> {
        PlannedDelta::new(SubstrateKind::Fs, intent.goal().0.clone())
    }

    fn precondition(&self, snap: &gx_core::ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        Ok(Fingerprint::new(
            SubstrateKind::Fs,
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
        let mut world = self.world.lock().expect("not poisoned");
        world.clone_from(&delta.payload().to_vec());
        let digest = digest_of(&world);
        Ok(gx_substrate::AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(SubstrateKind::Fs, SUBJECT.to_string(), digest)?,
            digest,
            Timestamp(0),
        ))
    }

    /// An inverse and no read behind it — form ① of DR-46-34, the ordinary road.
    fn invert(
        &self,
        _delta: &PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<InvertOutcome> {
        let prior = PlannedDelta::new(SubstrateKind::Fs, self.world())?;
        Ok(InvertOutcome::inverted(prior, Vec::new()))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

impl InputStageDeclaration for DeclaringAdapter {
    fn declared_input_stage(&self) -> BoundaryStage {
        self.declared
    }
}

/// An intent for `SUBJECT`, with the actor the test chooses — the one field the join reads.
fn intent_by(goal: &str, actor: Actor) -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        SUBJECT.to_string(),
        GoalBytes(goal.as_bytes().to_vec()),
        ChangeContext::Policy,
        actor,
    )
}

fn human() -> Actor {
    Actor::Human {
        key: "key-human-1".to_string(),
    }
}

fn agent() -> Actor {
    Actor::Agent {
        key: "key-agent-1".to_string(),
        model: "claude-fable-5".to_string(),
    }
}

/// An engine with the adapter registered, and its declaration registered only when `declare` is
/// true — the difference between a deployment that declared its input stage and one that did not.
fn engine_over(dir: &Path, adapter: &DeclaringAdapter, declare: bool) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "dr4633-declaring-fixture/1");
    if declare {
        engine.register_input_stage_declaration(SubstrateKind::Fs, Arc::new(adapter.clone()));
    }
    engine
}

/// Drive one intent to `Committed` and hand back its id.
fn commit_one(engine: &mut Engine<InjectedEvidence>, goal: &str, actor: Actor) -> TransformationId {
    let one = intent_by(goal, actor);
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &key, None).expect("verify"),
        Lifecycle::Admitted,
        "an inverse was constructed, so C-25 answered `True` and T-4a is the door"
    );
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(
        engine.commit(&id, AT, &key).expect("commit"),
        Lifecycle::Committed
    );
    id
}

fn payload_of(engine: &Engine<InjectedEvidence>, id: &TransformationId) -> ReceiptPayload {
    engine
        .receipt(id)
        .expect("T-11 issued a commit receipt")
        .payload()
        .expect("the receipt payload decodes")
}

/// Copy a committed project and rewrite its journal record for record, the way
/// `dr4634_read_set_absence.rs` does — a real R30 chain, not an in-place byte edit.
fn project_with_journal(
    from: &Path,
    name: &str,
    edit: impl Fn(&EngineJournalRecord) -> Option<EngineJournalRecord>,
) -> PathBuf {
    let to = scratch(name);
    copy_tree(from, &to);
    let path = to.join("journal.bin");
    let rewritten: Vec<EngineJournalRecord> = {
        let journal = EngineJournal::open(&path).expect("the copied journal opens");
        journal.records().iter().filter_map(&edit).collect()
    };
    std::fs::remove_file(&path).expect("the old journal is removed");
    let mut journal = EngineJournal::open(&path).expect("a fresh journal is created");
    for record in rewritten {
        journal.append(record).expect("the record appends");
    }
    to
}

/// Rewrite the `Planned` record's `input_generation` and nothing else.
fn with_input_generation(
    record: &EngineJournalRecord,
    stage: BoundaryStage,
) -> EngineJournalRecord {
    match record {
        EngineJournalRecord::Planned {
            transformation,
            intent_id,
            locator,
            delta_cid,
            fp0,
            parents,
            at,
            ..
        } => EngineJournalRecord::Planned {
            transformation: *transformation,
            intent_id: *intent_id,
            locator: locator.clone(),
            delta_cid: *delta_cid,
            fp0: fp0.clone(),
            parents: parents.clone(),
            input_generation: stage,
            // DR-46-45 (`req/973` B-1): not an undo's plan, so no witness was compared.
            undo_witness: None,
            at: *at,
        },
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// ① the declaration reaches the receipt
// ---------------------------------------------------------------------------

/// 🔴 **Form ①** — a human-actor commit under a `deterministic_replay` declaration attests it.
///
/// The actor is `Human`, so the join does **not** override, and the input-generation stage on the
/// signed receipt is the deployment's own declaration. Both stages are then `deterministic_replay`
/// (the verdict was derived), so `of_stages` collapses the whole boundary to `DeterministicReplay`.
#[test]
fn form_one_a_declared_input_stage_reaches_the_signed_boundary() {
    let dir = scratch("dr4633_form_one");
    let adapter = DeclaringAdapter::new("before", BoundaryStage::DeterministicReplay);
    let mut engine = engine_over(&dir, &adapter, true);
    let id = commit_one(&mut engine, "after", human());

    let payload = payload_of(&engine, &id);
    println!(
        "DR4633_FORM_ONE boundary={} input_generation={}",
        payload.determinism_boundary.as_str(),
        payload.determinism_boundary.input_generation().as_str()
    );
    assert_eq!(
        payload.determinism_boundary.input_generation(),
        BoundaryStage::DeterministicReplay,
        "a human-actor commit attests the deployment's own declaration"
    );
    assert_eq!(
        payload.determinism_boundary,
        gx_core::DeterminismBoundary::DeterministicReplay,
        "both stages deterministic_replay collapse to the whole-transformation value"
    );
}

// ---------------------------------------------------------------------------
// ② no declaration is v0, and a legacy journal reproduces it
// ---------------------------------------------------------------------------

/// 🔴 **Form ②** — an unregistered substrate attests `unknown`, and a journal without the field
/// (its E-M5-13 `serde(default)` shape) reproduces the same boundary on the rebuild road.
#[test]
fn form_two_no_declaration_is_unknown_and_the_rebuild_reproduces_it() {
    let dir = scratch("dr4633_form_two_source");
    let adapter = DeclaringAdapter::new("before", BoundaryStage::DeterministicReplay);
    // `declare = false`: the declaration is not registered, so the join answers `Unknown` for the
    // human actor exactly as v0 did before this lane.
    let mut engine = engine_over(&dir, &adapter, false);
    let id = commit_one(&mut engine, "after", human());

    let issued = payload_of(&engine, &id);
    assert_eq!(
        issued.determinism_boundary.input_generation(),
        BoundaryStage::Unknown,
        "a deployment that declared nothing attests unknown, v0's value"
    );
    drop(engine);

    // A journal written before DR-46-33 carries no `input_generation`; `serde(default)` decodes it
    // to `Unknown`, which is the value the commit already attested — so the rebuild reproduces it.
    let legacy = project_with_journal(&dir, "dr4633_form_two_legacy", |record| {
        Some(with_input_generation(record, BoundaryStage::Unknown))
    });
    let mut reopened = engine_over(&legacy, &adapter, false);
    let outcome = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("DR4633_FORM_TWO reissue={outcome:?}");
    assert!(
        matches!(outcome, Reissued::Filed(_)),
        "a journal without the field reproduces the unknown boundary it attested"
    );
}

// ---------------------------------------------------------------------------
// ③ the value is load-bearing — the negative control
// ---------------------------------------------------------------------------

/// 🔴 **Form ③ and its control** — the one-value difference, and the two answers it decides.
///
/// Both beds are the same committed project, re-journalled record for record. The **only**
/// difference is `Planned.input_generation`:
///
/// * kept at `deterministic_replay` — the value the commit journalled — the rebuild reproduces the
///   boundary, the payload digests to the leaf the ledger holds, and the receipt is **filed**. This
///   is the control; without it the refusal below would be evidence of nothing but the fixture's
///   own doctoring.
/// * flipped to `unknown` — the rebuild attests a different boundary, the digest no longer matches,
///   and the road is **refused**.
///
/// The second arm is the point: it is the negative control the DR rests on — a rebuild that read a
/// different input stage could not reproduce the leaf, which is why the join's result is journalled
/// rather than re-derived from an actor that Σ does not carry (43 §7-3b).
#[test]
fn form_three_the_journalled_input_stage_is_load_bearing_in_the_digest() {
    let dir = scratch("dr4633_form_three_source");
    let adapter = DeclaringAdapter::new("before", BoundaryStage::DeterministicReplay);
    let mut engine = engine_over(&dir, &adapter, true);
    let id = commit_one(&mut engine, "after", human());
    let issued = payload_of(&engine, &id);
    assert_eq!(
        issued.determinism_boundary.input_generation(),
        BoundaryStage::DeterministicReplay
    );
    drop(engine);

    // The control: the journalled stage is left where the commit put it.
    let kept = project_with_journal(&dir, "dr4633_form_three_kept", |record| {
        Some(with_input_generation(
            record,
            BoundaryStage::DeterministicReplay,
        ))
    });
    let mut reopened = engine_over(&kept, &adapter, true);
    let control = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("DR4633_FORM_THREE_CONTROL reissue={control:?}");
    let Reissued::Filed(receipt) = control else {
        panic!("the control kept every fact the issue road had, so it must reproduce the payload");
    };
    let rebuilt = receipt.payload().expect("the rebuilt payload decodes");
    assert_eq!(
        rebuilt.determinism_boundary, issued.determinism_boundary,
        "the rebuild reproduces the boundary the ledger witnessed"
    );

    // The defect: flip the journalled stage by one value and nothing else.
    let flipped = project_with_journal(&dir, "dr4633_form_three_flipped", |record| {
        Some(with_input_generation(record, BoundaryStage::Unknown))
    });
    let mut reopened = engine_over(&flipped, &adapter, true);
    let outcome = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("DR4633_FORM_THREE_FLIPPED reissue={outcome:?}");
    assert!(
        !matches!(outcome, Reissued::Filed(_)),
        "a journal whose input stage was changed must not reproduce the leaf the ledger holds"
    );
}

// ---------------------------------------------------------------------------
// ④ the actor overrides the declaration
// ---------------------------------------------------------------------------

/// 🔴 **Form ④** — an `Actor::Agent` attests `llm_originated` whatever the declaration said, and a
/// human under the same declaration attests the declaration. The one bit of difference is the
/// actor, and it decides the input-generation stage — the join `boundary.rs` names.
#[test]
fn form_four_an_agent_actor_overrides_the_declaration() {
    let dir = scratch("dr4633_form_four_agent");
    let adapter = DeclaringAdapter::new("before", BoundaryStage::DeterministicReplay);
    let mut engine = engine_over(&dir, &adapter, true);
    let agent_id = commit_one(&mut engine, "after", agent());
    let by_agent = payload_of(&engine, &agent_id);
    println!(
        "DR4633_FORM_FOUR_AGENT input_generation={}",
        by_agent.determinism_boundary.input_generation().as_str()
    );
    assert_eq!(
        by_agent.determinism_boundary.input_generation(),
        BoundaryStage::LlmOriginated,
        "an Actor::Agent is an LLM origin whatever a static file declared"
    );

    // The control one actor over: the same declaration, a human, attests the declaration itself.
    let dir_h = scratch("dr4633_form_four_human");
    let adapter_h = DeclaringAdapter::new("before", BoundaryStage::DeterministicReplay);
    let mut engine_h = engine_over(&dir_h, &adapter_h, true);
    let human_id = commit_one(&mut engine_h, "after", human());
    let by_human = payload_of(&engine_h, &human_id);
    println!(
        "DR4633_FORM_FOUR_HUMAN input_generation={}",
        by_human.determinism_boundary.input_generation().as_str()
    );
    assert_eq!(
        by_human.determinism_boundary.input_generation(),
        BoundaryStage::DeterministicReplay,
        "the same declaration under a human actor attests the declaration"
    );
    assert_ne!(
        by_agent.determinism_boundary.input_generation(),
        by_human.determinism_boundary.input_generation(),
        "the actor is the one field of difference, and it decides the stage"
    );
}
