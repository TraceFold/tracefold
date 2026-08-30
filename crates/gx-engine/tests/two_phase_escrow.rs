// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 Two-phase escrow, engine half (`req/38` §98, ruling 1 / §99, ruling 2; design `req/160` §1-1) (sem: SEM-gx-engine-961).
//!
//! What is measured, over a real journal in a scratch directory:
//!
//! 1. **The mechanism**: a declared do-time member escrows `Pending` at T-10b (pre-state members
//!    resolved — E-M4-30 unmoved), the applied call's answer is journalled (`ApplyObserved`) and
//!    the inverse is completed (`InverseCompleted { Some }`) inside the same Committing critical
//!    section, before T-11: the receipt's `inverse_delta` names the **completed** CID, the escrow
//!    row is `Available`, and a subsequent `undo` runs the completed inverse to `Committed` with
//!    the supersede edge drawn.
//! 2. **The fold** (§99, ruling 2-4) (sem: SEM-gx-engine-962): every completion failure — here, an observation the completion
//!    cannot resolve — journals `InverseCompleted { None }`, folds the row to `Unavailable`, and
//!    the **commit continues**: `Committed`, receipt `Admit` beside `inverse_delta: None` (the
//!    failure's visible fingerprint), and `undo` refused by name.
//! 3. **The crash window** (`req/160` §1-1's named residue): a journal that ends inside the
//!    critical section with a `Pending` escrow and **no** `ApplyObserved` recovers to `Committed`
//!    with the escrow folded honestly to `Unavailable` — the observation died with the process and
//!    is not re-obtainable (`req/160` 1-0, fact 3) (sem: SEM-gx-engine-963); and the same window **with** the observation
//!    journalled recovers by re-resolving it: escrow `Available`, receipt carrying the completed
//!    CID -- "re-resolvable after `ApplyObserved`" (sem: SEM-gx-engine-963), measured.
//! 4. **The registry is optional**: with no completion registered nothing changes — a `Some` from
//!    `invert` escrows `Available` exactly as before (unregistered = the existing behaviour) (sem: SEM-gx-engine-963).
//! 5. **The guard itself** (`req/38` §102, ruling 1, the F1 mutation survivor) (sem: SEM-gx-engine-963): a **live** `Pending`
//!    row — the crash window's trace, seated without `Engine::recover` — reaching `Engine::undo`
//!    is refused **by name** (`Error::InvalidState`, "completion never finished (Pending)") (sem: SEM-gx-engine-963), so
//!    the guard at `pipeline.rs`'s undo entry is pinned by behaviour rather than by spelling.

mod support;

use std::sync::{Arc, Mutex};

use gx_core::{Fingerprint, Reversibility, SubstrateKind, Timestamp};
use gx_engine::store::{FingerprintRecord, ObservationStore};
use gx_engine::{
    BlobStore, Engine, EngineJournal, EngineJournalRecord, Error, InjectedEvidence, InverseStatus,
    Lifecycle,
};
use gx_substrate::{InverseCompletion, PlannedDelta, SubstrateAdapter};

use support::{digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The marker the fixture's partial inverse carries in front of the restore payload.
const PARTIAL: &[u8] = b"PARTIAL:";

/// A world-backed adapter whose inverse is **partial until completed** — the engine-facing shape
/// of `gx-adapter-mcp`'s do-result declaration, without an MCP wire in the picture (N-13 keeps
/// adapters out of this crate; the completion contract is `gx-substrate`'s and this implements
/// it directly).
///
/// * `invert` answers `Some(b"PARTIAL:" ++ <prior world>)` — a recipe, deterministic in the
///   snapshot handed in, constructible before `apply` like every escrow;
/// * `apply` writes the payload and reports the do-time answer on the observation seat
///   (`{"url":".../issues/7"}` — the fixture's stand-in for a server-assigned number);
/// * `complete_inverse` strips the marker **only when** the observation carries a derivable
///   `/url`; anything else is `Ok(None)` — the adapter-side fold this file then watches the
///   engine translate into `Unavailable`.
#[derive(Clone, Debug)]
struct TwoPhaseAdapter {
    world: Arc<Mutex<Vec<u8>>>,
    /// What the substrate answers at apply time. `None` = an answer that resolves; `Some(bytes)`
    /// = answer exactly these bytes instead (the unresolvable-observation injection).
    answer: Arc<Mutex<Vec<u8>>>,
}

impl TwoPhaseAdapter {
    fn new(world: &str) -> Self {
        Self {
            world: Arc::new(Mutex::new(world.as_bytes().to_vec())),
            answer: Arc::new(Mutex::new(
                br#"{"id":"5153620527","url":"https://github.test/o/r/issues/7"}"#.to_vec(),
            )),
        }
    }

    fn answering(self, answer: &[u8]) -> Self {
        *self.answer.lock().expect("not poisoned") = answer.to_vec();
        self
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for TwoPhaseAdapter {
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
        intent: &gx_core::Intent,
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
        // A completed inverse arrives without the marker; a partial one must never be applied.
        assert!(
            !delta.payload().starts_with(PARTIAL),
            "a partial escrow reached apply — the engine let an incomplete inverse run"
        );
        let mut world = self.world.lock().expect("not poisoned");
        world.clone_from(&delta.payload().to_vec());
        let digest = digest_of(&world);
        Ok(gx_substrate::AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(SubstrateKind::Fs, "/tmp/world".to_string(), digest)?,
            digest,
            Timestamp(0),
        )
        .with_observation(self.answer.lock().expect("not poisoned").clone()))
    }

    fn invert(
        &self,
        _delta: &PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        let mut payload = PARTIAL.to_vec();
        payload.extend_from_slice(&self.world());
        Ok(gx_substrate::InvertOutcome::inverted(
            PlannedDelta::new(SubstrateKind::Fs, payload)?,
            Vec::new(),
        ))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

impl InverseCompletion for TwoPhaseAdapter {
    fn needs_completion(&self, inverse: &PlannedDelta) -> gx_substrate::Result<bool> {
        Ok(inverse.payload().starts_with(PARTIAL))
    }

    fn complete_inverse(
        &self,
        partial: &PlannedDelta,
        observation: &[u8],
    ) -> gx_substrate::Result<Option<PlannedDelta>> {
        let Some(rest) = partial.payload().strip_prefix(PARTIAL) else {
            return Ok(Some(partial.clone()));
        };
        // The derivation stands in for do_result_number_from: an observation whose `"url":"..."`
        // member ends in /<digits>. Read with a plain scan rather than a JSON crate — this crate
        // declares no serde_json (N-13's spirit: the fixture is not an adapter), and the shape
        // under test is the engine's fold, not the parse. Anything else folds (§99, ruling 1:
        // derivation failure = fail-safe, the only choice) (sem: SEM-gx-engine-964).
        let text = core::str::from_utf8(observation).unwrap_or("");
        let derivable = text
            .split_once("\"url\":\"")
            .and_then(|(_, rest)| rest.split_once('\"'))
            .and_then(|(url, _)| url.rsplit_once('/'))
            .is_some_and(|(_, tail)| !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()));
        if !derivable {
            return Ok(None);
        }
        PlannedDelta::new(SubstrateKind::Fs, rest.to_vec()).map(Some)
    }
}

fn engine_over(
    dir: &std::path::Path,
    adapter: &TwoPhaseAdapter,
    with_completion: bool,
) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "two-phase-fixture/1");
    if with_completion {
        engine.register_completion(SubstrateKind::Fs, Arc::new(adapter.clone()));
    }
    engine
}

/// Drive one intent to `Committed` and hand back its id.
fn commit_one(engine: &mut Engine<InjectedEvidence>, goal: &str) -> gx_core::TransformationId {
    let one = intent("/srv/world", goal);
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &key, None).expect("verify"),
        Lifecycle::Admitted
    );
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(
        engine.commit(&id, AT, &key).expect("commit"),
        Lifecycle::Committed
    );
    id
}

/// The kinds of every journal record about `id`, in order.
fn kinds_about(
    engine: &Engine<InjectedEvidence>,
    id: gx_core::TransformationId,
) -> Vec<&'static str> {
    engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.transformation() == Some(id))
        .map(gx_engine::EngineJournalRecord::kind)
        .collect()
}

// ---------------------------------------------------------------------------
// 1. The mechanism, green
// ---------------------------------------------------------------------------

#[test]
fn a_pending_escrow_is_completed_inside_the_critical_section_and_the_undo_runs_it() {
    let dir = scratch("two_phase_green");
    let adapter = TwoPhaseAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter, true);
    let id = commit_one(&mut engine, "after");

    // The record sequence: escrow Pending → apply announced → observed → completed → committed.
    let kinds = kinds_about(&engine, id);
    println!("TWO_PHASE_RECORDS={kinds:?}");
    assert_eq!(
        kinds,
        vec![
            "Planned",
            "VerifyStarted",
            "Verdict",
            "Canonicalized",
            "CommittingStarted",
            "ProvenanceDerived",
            "InverseEscrowed",
            "ApplyStarted",
            "ApplyObserved",
            "InverseCompleted",
            "Committed",
        ]
    );
    let pending_flag = engine.journal().records().iter().find_map(|r| match r {
        EngineJournalRecord::InverseEscrowed { pending, .. } => Some(*pending),
        _ => None,
    });
    assert_eq!(
        pending_flag,
        Some(true),
        "the escrow was journalled partial"
    );

    // The escrow settled Available, on the **completed** CID, and the receipt names it.
    assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Available));
    let completed_cid = engine.escrowed_inverse(&id).expect("a completed escrow");
    let payload = engine
        .receipt(&id)
        .expect("T-11 issued")
        .payload()
        .expect("the receipt payload decodes");
    assert_eq!(
        payload.inverse_delta,
        Some(completed_cid),
        "the receipt seals the completed inverse (req/160 §1-1 step 4)"
    );
    // The completed body is executable: no marker, the prior world.
    let completed = engine
        .blobs()
        .get(&completed_cid)
        .expect("the body is filed");
    assert_eq!(completed.payload(), b"before");

    // Σ agrees with its own reconstruction across the new records (AC-039's road).
    let live = engine.sigma().canonical_bytes().expect("live Σ encodes");
    let replayed = gx_engine::reconstruct(engine.journal().records())
        .canonical_bytes()
        .expect("replayed Σ encodes");
    assert_eq!(live, replayed, "the new records reconstruct to the same Σ");

    // And the undo runs the completed inverse: world restored, edge drawn.
    let key = signing_key();
    let (_, undoing) = engine
        .undo(&id, &engine.attested_postcondition(&id), 8, AT)
        .expect("undo builds the candidate");
    assert_eq!(
        engine.verify(&undoing, AT, &key, None).expect("verify"),
        Lifecycle::Admitted
    );
    engine
        .canonicalize(&undoing, AT, None)
        .expect("canonicalize");
    assert_eq!(
        engine.commit(&undoing, AT, &key).expect("commit"),
        Lifecycle::Committed
    );
    assert_eq!(
        adapter.world(),
        b"before".to_vec(),
        "the undo restored the prior world"
    );
    assert_eq!(engine.state(&id), Some(Lifecycle::Superseded));
    assert_eq!(
        engine.inverse_status(&id),
        Some(InverseStatus::Consumed { by: undoing })
    );
}

// ---------------------------------------------------------------------------
// 2. The fold (§99, ruling 2-4) (sem: SEM-gx-engine-965)
// ---------------------------------------------------------------------------

#[test]
fn a_completion_failure_folds_to_unavailable_and_the_commit_continues() {
    let dir = scratch("two_phase_fold");
    // The server's answer carries no derivable URL — the completion cannot resolve it.
    let adapter = TwoPhaseAdapter::new("before").answering(br#"{"id":"1","url":"not-a-path"}"#);
    let mut engine = engine_over(&dir, &adapter, true);
    let id = commit_one(&mut engine, "after");

    // The commit continued (the world moved; an abort here would be the lie §99, ruling 2-4, names) (sem: SEM-gx-engine-966).
    assert_eq!(engine.state(&id), Some(Lifecycle::Committed));
    assert_eq!(adapter.world(), b"after".to_vec());

    // The fold is journalled and visible: InverseCompleted{None} → Unavailable → receipt None.
    assert!(kinds_about(&engine, id).contains(&"InverseCompleted"));
    let completed_cid = engine.journal().records().iter().find_map(|r| match r {
        EngineJournalRecord::InverseCompleted { inverse_cid, .. } => Some(*inverse_cid),
        _ => None,
    });
    assert_eq!(
        completed_cid,
        Some(None),
        "the outcome record says the fold"
    );
    assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Unavailable));
    let payload = engine
        .receipt(&id)
        .expect("issued")
        .payload()
        .expect("decodes");
    assert!(
        payload.inverse_delta.is_none(),
        "Admit beside inverse_delta: None is the failure's fingerprint"
    );

    // And the undo is refused by name, not by surprise.
    let refused = engine
        .undo(&id, &engine.attested_postcondition(&id), 8, AT)
        .expect_err("no inverse to consume");
    assert_eq!(refused.kind(), "NotFound");
}

/// 🔴 **`req/871` F1 — a signed receipt asserting a reversibility it does not have.**
/// **(2026-08-26, seat=Opus, 暫定 — 再審査可. `req/868` R-868-4. MEASURED RED, NOT YET FIXED.)**
///
/// The test above proves `inverse_delta` folds to `None`. It never looks at `reversibility`, and
/// nothing else in the workspace does over this road — `dr4626_invert_seam.rs` asserts
/// `Reversibility::False`/`Unknown` but reaches them from `invert` answering directly, which is
/// captured *before* the fold. So the fold's effect on the receipt's other reversibility field has
/// never been measured. It is measured here.
///
/// `verdict_c25` is taken from `adapter.invert(..).verdict()` at `pipeline.rs:6744`, **before** the
/// settle. `inverse_delta: final_inverse` (`:7189`) reflects the fold at `:7166`;
/// `reversibility: Some(verdict_c25)` (`:7214`) ships the pre-settle answer unchanged. So one
/// signed payload carries `reversibility: Some(True)` beside `inverse_delta: None` — two fields of
/// **one signed artifact** giving opposite answers to "can this be undone". A holder who reads the
/// field we added precisely so that `inverse_delta: null` would stop being ambiguous
/// (`req/38` §198 ruling (b)) is told **yes** by the very field that was supposed to disambiguate.
///
/// **Why this probe is `#[ignore]`d rather than green or red.** It is not ignored because it is
/// doubtful — it is ignored because it is *correct and the code is wrong*, and this lane could not
/// responsibly choose the repair. `Reversibility` is `Serialize`/`Deserialize`, on the wire, with a
/// frozen receipt corpus over it; and neither surviving variant is obviously right. `False` says
/// "no inverse exists for this call", but one *was* constructed and then lost. `Unknown` is
/// documented as "the prior could not be read", which is a different fact. The honest repair may
/// need a fourth word, and minting a word in a signed vocabulary at the end of an overrun box is
/// how a lane replaces one lie with another. Landing it un-ignored would make `main` red for every
/// other lane, which is a worse tax than a declared, runnable falsifier.
///
/// **Run it with `cargo test -p gx-engine --test two_phase_escrow -- --ignored`.** Whoever takes
/// R-868-4 deletes the `#[ignore]` in the same commit that decides the vocabulary.
#[test]
#[ignore = "req/871 F1 / req/868 R-868-4: measured red; the repair is a signed-vocabulary decision"]
fn the_receipt_does_not_claim_reversible_beside_an_inverse_that_folded_away() {
    let dir = scratch("two_phase_f1");
    let adapter = TwoPhaseAdapter::new("before").answering(br#"{"id":"1","url":"not-a-path"}"#);
    let mut engine = engine_over(&dir, &adapter, true);
    let id = commit_one(&mut engine, "after");

    assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Unavailable));
    let payload = engine
        .receipt(&id)
        .expect("issued")
        .payload()
        .expect("decodes");

    println!(
        "F1_INVERSE_DELTA={:?} F1_REVERSIBILITY={:?}",
        payload.inverse_delta, payload.reversibility
    );

    assert!(
        payload.inverse_delta.is_none(),
        "the bed: this road is the fold, so the receipt names no inverse"
    );
    assert_ne!(
        payload.reversibility,
        Some(Reversibility::True),
        "req/871 F1: one signed payload must not answer \"can this be undone\" both ways. \
         `inverse_delta` is None because the completion folded to Unavailable, so `reversibility` \
         cannot still be the pre-settle True captured at pipeline.rs:6744. Fixing this means \
         deriving reversibility from the settled state, and deciding which word the settled state \
         deserves -- see req/868 R-868-4 before changing the enum"
    );
}

// ---------------------------------------------------------------------------
// 3. The crash window, both sides
// ---------------------------------------------------------------------------

/// Write a crashed critical section by hand: everything up to `ApplyStarted`, escrow `Pending`,
/// and — per `observed` — the `ApplyObserved` record and its stored bytes. Returns the id.
fn crashed_window(dir: &std::path::Path, observed: bool) -> gx_core::TransformationId {
    let tid = support::tid(41);
    let partial = {
        let mut payload = PARTIAL.to_vec();
        payload.extend_from_slice(b"before");
        PlannedDelta::new(SubstrateKind::Fs, payload).expect("mints")
    };
    let forward = PlannedDelta::new(SubstrateKind::Fs, b"after".to_vec()).expect("mints");
    let observation = br#"{"id":"5153620527","url":"https://github.test/o/r/issues/7"}"#;

    let journal_path = dir.join("journal.bin");
    let blobs = BlobStore::open(format!("{}.blobs", journal_path.display())).expect("blobs open");
    blobs.put(&forward).expect("the forward body is filed");
    blobs.put(&partial).expect("the partial body is filed");
    if observed {
        let observations =
            ObservationStore::open(format!("{}.observations", journal_path.display()))
                .expect("observations open");
        observations
            .put(observation)
            .expect("the observation is filed");
    }

    // The world the crashed process left: the forward applied ("after"), digest for fp0 taken
    // before it ("before" world) — exactly the CAS-passed critical section.
    let fp0 = FingerprintRecord::of(
        &Fingerprint::new(
            SubstrateKind::Fs,
            "/srv/world".to_string(),
            digest_of(b"before"),
        )
        .expect("in bounds"),
    );
    let verdict_digest = digest_of(b"a verdict");
    let mut journal = EngineJournal::open(&journal_path).expect("journal opens");
    let records = vec![
        EngineJournalRecord::Planned {
            transformation: tid,
            intent_id: support::iid(41),
            locator: "/srv/world".to_string(),
            delta_cid: forward.reference().cid,
            fp0,
            parents: Vec::new(),
            input_generation: gx_core::BoundaryStage::Unknown,
            // DR-46-45 (`req/973` B-1): not an undo's plan, so no witness was compared.
            undo_witness: None,
            at: AT,
        },
        EngineJournalRecord::VerifyStarted {
            transformation: tid,
            at: AT,
        },
        EngineJournalRecord::Verdict {
            transformation: tid,
            kind: gx_core::VerdictKind::Admit,
            verdict_digest: Some(verdict_digest),
            fail_posture_engaged: false,
            at: AT,
        },
        EngineJournalRecord::Canonicalized {
            transformation: tid,
            canonical_cid: digest_of(b"canonical"),
            enforced: None,
            at: AT,
        },
        EngineJournalRecord::CommittingStarted {
            transformation: tid,
            at: AT,
        },
        EngineJournalRecord::InverseEscrowed {
            transformation: tid,
            inverse_cid: Some(partial.reference().cid),
            pending: true,
            reads: Vec::new(),
            undetermined: false,
            // 🔴 **DR-46-34** — the adapter answered, so the list above is a reading.
            reads_attested: true,
            at: AT,
        },
        EngineJournalRecord::ApplyStarted {
            transformation: tid,
            delta_cid: forward.reference().cid,
            at: AT,
        },
    ];
    for record in records {
        journal.append(record).expect("append");
    }
    if observed {
        journal
            .append(EngineJournalRecord::ApplyObserved {
                transformation: tid,
                observation_cid: ObservationStore::address(observation),
                at: AT,
            })
            .expect("append");
    }
    drop(journal);
    tid
}

#[test]
fn a_crashed_pending_escrow_with_no_observation_folds_to_unavailable() {
    let dir = scratch("two_phase_crash_unobserved");
    let id = crashed_window(&dir, false);

    let adapter = TwoPhaseAdapter::new("after"); // the world as the crashed apply left it
    let mut engine = engine_over(&dir, &adapter, true);
    let recovered = engine.recover(AT, &signing_key()).expect("recovery runs");
    assert_eq!(recovered.len(), 1);
    assert_eq!(
        recovered[0].state,
        Lifecycle::Committed,
        "the commit finishes"
    );

    // The fold: journalled, Σ-visible, receipt-visible.
    let folded = engine.journal().records().iter().find_map(|r| match r {
        EngineJournalRecord::InverseCompleted { inverse_cid, .. } => Some(*inverse_cid),
        _ => None,
    });
    assert_eq!(
        folded,
        Some(None),
        "Pending + no observation folds to Unavailable, journalled (req/160 §1-1's crash window)"
    );
    let row = gx_engine::reconstruct(engine.journal().records());
    let escrow = row
        .escrow()
        .iter()
        .find(|e| e.transformation == id)
        .expect("a row");
    assert_eq!(escrow.status, InverseStatus::Unavailable);
    assert_eq!(escrow.inverse_cid, None);
    let receipt = recovered[0].receipt.as_ref().expect("re-issued");
    assert!(receipt.payload().expect("decodes").inverse_delta.is_none());
}

#[test]
fn a_crashed_pending_escrow_with_the_observation_journalled_is_re_resolved() {
    let dir = scratch("two_phase_crash_observed");
    let id = crashed_window(&dir, true);

    let adapter = TwoPhaseAdapter::new("after");
    let mut engine = engine_over(&dir, &adapter, true);
    let recovered = engine.recover(AT, &signing_key()).expect("recovery runs");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].state, Lifecycle::Committed);

    // Re-resolved from the journalled observation: completed, Available, receipt names it.
    let completed = engine.journal().records().iter().find_map(|r| match r {
        EngineJournalRecord::InverseCompleted { inverse_cid, .. } => Some(*inverse_cid),
        _ => None,
    });
    let completed = completed
        .expect("the outcome is journalled")
        .expect("a crash after ApplyObserved can be re-resolved from the journal's observation (req/160 §1-1) (sem: SEM-gx-engine-967)");
    let body = engine
        .blobs()
        .get(&completed)
        .expect("the completed body is filed");
    assert_eq!(body.payload(), b"before");
    let row = gx_engine::reconstruct(engine.journal().records());
    let escrow = row
        .escrow()
        .iter()
        .find(|e| e.transformation == id)
        .expect("a row");
    assert_eq!(escrow.status, InverseStatus::Available);
    assert_eq!(escrow.inverse_cid, Some(completed));
    let receipt = recovered[0].receipt.as_ref().expect("re-issued");
    assert_eq!(
        receipt.payload().expect("decodes").inverse_delta,
        Some(completed)
    );
}

// ---------------------------------------------------------------------------
// 4. The registry is optional (unregistered = the existing behaviour) (sem: SEM-gx-engine-968)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 5. The guard itself (req/38 §102, ruling 1) (sem: SEM-gx-engine-969): a live Pending row vs `Engine::undo`
// ---------------------------------------------------------------------------

/// 🔴 The F1 mutation survivor, closed by behaviour (`req/38` §102, ruling 1 / `req/164` §2 F1) (sem: SEM-gx-engine-970):
/// `Engine::undo` claims to refuse a `Pending` row **by name**, and until this test no probe ever
/// put a live one in front of it — deleting the guard left every shipped suite green.
///
/// # Why the literal crash window cannot carry this probe, measured against the sources
///
/// `req/164` F1's sketch — the `crashed_window` fixture as-is, `recover` un-called, `undo` fired —
/// refuses **upstream** of the guard: that journal's row replays to `Committing`
/// (`replay.rs`'s `CommittingStarted` arm), `Engine::open` leaves the state table empty, and both
/// roads into `undo` refuse before the escrow row is read (`self.table.get` → `NotFound`;
/// `rehydrate_committed` seats `Committed`/`Superseded` rows only). So a guard-deleted engine
/// passes that sketch too, and the mutation stays a survivor.
///
/// The shape the guard actually names -- "a `Pending` row **outside** the critical section is a
/// crash's trace" (sem: SEM-gx-engine-971) -- is a row whose state is `Committed` while its escrow row is still `Pending`:
/// exactly what this crate's own writer can never journal (the live commit seals the row before
/// `Committed`), which is why the guard is defence-in-depth and why the probe builds the trace the
/// way this file builds every crash window: by hand, into the journal. A real commit is driven
/// first so every other fact (blobs, provenance, `Planned`, the re-identifying CID) is genuine;
/// the crash window's tail — a `Pending` escrow with **no** completion outcome after it — is then
/// appended, and the row is seated the way `gx undo` seats a committed row in a fresh process
/// (`rehydrate_committed`), with `Engine::recover` deliberately **not** run: the guard's own
/// comment names recovery as the road that completes or folds, and this walk stops at the door.
#[test]
fn a_live_pending_escrow_reaching_undo_before_recovery_is_refused_by_name() {
    let dir = scratch("two_phase_pending_undo_guard");
    let adapter = TwoPhaseAdapter::new("before");
    let (id, partial_cid) = {
        let mut engine = engine_over(&dir, &adapter, true);
        let id = commit_one(&mut engine, "after");
        let partial_cid = engine
            .journal()
            .records()
            .iter()
            .find_map(|r| match r {
                EngineJournalRecord::InverseEscrowed {
                    inverse_cid: Some(cid),
                    pending: true,
                    ..
                } => Some(*cid),
                _ => None,
            })
            .expect("the live commit escrowed a partial inverse");
        (id, partial_cid)
    };

    // The crash window's trace: the escrow re-opened `Pending` — the partial body's own CID, the
    // completion's outcome record never written after it. `InverseEscrowed { pending: true }` with
    // no `InverseCompleted` following is the exact vocabulary `crashed_window` writes; here it is
    // the journal's **last word**, so Σ's last-write-wins replay leaves the row `Pending` beside a
    // `Committed` state.
    {
        let mut journal =
            EngineJournal::open(dir.join("journal.bin")).expect("the journal reopens");
        journal
            .append(EngineJournalRecord::InverseEscrowed {
                transformation: id,
                inverse_cid: Some(partial_cid),
                pending: true,
                reads: Vec::new(),
                undetermined: false,
                // 🔴 **DR-46-34** — the adapter answered, so the list above is a reading.
                reads_attested: true,
                at: AT,
            })
            .expect("append");
    }

    // A fresh process. Recovery is NOT run — `Engine::recover` would complete or fold the row,
    // and the claim under test is what `undo` does when it meets the row **before** that road.
    let mut engine = engine_over(&dir, &adapter, true);
    assert!(
        engine
            .rehydrate_committed(&id, &intent("/srv/world", "after"))
            .expect("the committed row rehydrates from Σ and the blob store"),
        "the row is seated the way `gx undo` seats it"
    );
    assert_eq!(
        engine.inverse_status(&id),
        Some(InverseStatus::Pending),
        "the escrow row reaching undo is live and Pending"
    );

    // The claim of req/162 §1, now behavioural: refused by name, not by surprise.
    let refused = engine
        .undo(&id, &engine.attested_postcondition(&id), 8, AT)
        .expect_err("a partial inverse is not an executable one");
    match &refused {
        Error::InvalidState { attempted, .. } => assert!(
            attempted.contains("completion never finished (Pending)"),
            "the refusal is the Pending guard's own sentence, not the state guard's: {attempted:?}"
        ),
        other => panic!("the Pending guard refuses with InvalidState, got {other:?}"),
    }
    // Refused means untouched: the row is still Pending (nothing consumed, nothing folded), and
    // no undo candidate was drafted for it.
    assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Pending));
    let planned = engine
        .journal()
        .records()
        .iter()
        .filter(|r| matches!(r, EngineJournalRecord::Planned { .. }))
        .count();
    assert_eq!(planned, 1, "the refusal drafted no undo transformation");
}

#[test]
fn an_unregistered_substrate_escrows_available_exactly_as_before() {
    let dir = scratch("two_phase_unregistered");
    let adapter = TwoPhaseAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter, false);
    let id = commit_one(&mut engine, "after");

    let kinds = kinds_about(&engine, id);
    assert!(
        !kinds.contains(&"ApplyObserved") && !kinds.contains(&"InverseCompleted"),
        "no completion registered → the two-phase records are never written: {kinds:?}"
    );
    let pending_flag = engine.journal().records().iter().find_map(|r| match r {
        EngineJournalRecord::InverseEscrowed { pending, .. } => Some(*pending),
        _ => None,
    });
    assert_eq!(
        pending_flag,
        Some(false),
        "the escrow is complete, as it always was"
    );
    assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Available));
}
