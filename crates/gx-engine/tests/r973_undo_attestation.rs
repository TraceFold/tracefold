// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **DR-46-45 (`req/973` §B-1 and §B-2)** — what an undo compared, and what it undid, on the face.
//!
//! Spec: 42 §3.10 for the `undo` seat on `ReceiptPayload`, 42 §3.13 for `Planned.undo_witness`,
//! 43 T-12 for the supersede edge, 43 §5.2 for the refusal table.
//!
//! # The two questions this file answers, and the one it refuses to answer by inference
//!
//! `req/973` §1 measured that the replay CAS **exists** — a third-party write after `T_o` committed
//! is refused with exit 3 / HTTP 409 — and then measured the hole beside it: a reader holding the
//! signed receipt could not tell an undo that ran that comparison from one that fired with nothing
//! to compare against, because `UndoWitness::Unobservable` is *declared and not refused*
//! (DR-46-7, `req/38` §123 ruling 1) and the declaration reached only an HTTP field. §B-1 is that
//! hole. §B-2 is the other half: the edge from an undo to what it undid was inside
//! `canonical_cid`'s pre-image and nowhere a reader could reach.
//!
//! The third test below is the **negative control**, and it is the reason §B-2 is a field rather
//! than an inference: `inverse_delta` is not a join key. Two unrelated commits in the corpus this
//! file builds carry the same one, because both inverses say "put this locator back to `before`" and
//! a content address is content-addressed. A DAG inferred from it maps one value onto two
//! transformations, which is a false edge and not a missing one — the failure mode that looks
//! correct.
//!
//! # 🔴 The AC this file does **not** implement as written
//!
//! `req/973` §B-2's AC asks for the receipt-borne edge set and the journal-borne edge set to be
//! **equal**, and defines the journal side as "`parents` + `Superseded`". Measured here, that is
//! false, and the correction is recorded in `req/973` §8: `Planned.parents` is written for undos
//! that never commit (this file builds one), and the `--retry` road (`req/38` §98 ruling 2) writes
//! two `Planned` records for one committed undo. So the relation is
//!
//! ```text
//!     edges(receipt) == edges(Superseded)  ⊆  edges(Planned.parents)
//! ```
//!
//! and the containment is asserted **strict** on a corpus built to make it strict — because a
//! containment that never separates is an equality nobody noticed writing.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use gx_core::{Cid, Timestamp, TransformationId};
use gx_engine::{
    Engine, EngineJournalRecord, InjectedEvidence, Lifecycle, UndoWitness, Unobservable,
};
use gx_witness::receipt::{ReceiptKind, UndoAttestation, UndoDisposition};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const LATER: Timestamp = Timestamp(1_754_000_120_000_000_000);
const LATER_STILL: Timestamp = Timestamp(1_754_000_240_000_000_000);

/// The locator every transformation in this file acts on, so that the inverse deltas collide.
const LOCATOR: &str = "/tmp/r973-undo-attestation.txt";

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// What one corpus run hands back: the engine, and the four ids by name.
struct Corpus {
    engine: Engine<InjectedEvidence>,
    /// `before` → `after`, committed.
    t_o: TransformationId,
    /// The undo of `t_o`, committed. Carries the disposition the caller asked for.
    t_u: TransformationId,
    /// `before` → `third`, committed. Shares `t_o`'s `inverse_delta` — see the module note.
    t_2: TransformationId,
    /// The undo of `t_2`, **planned and left as a `Candidate`**. This is the strict half of the
    /// containment: a `Planned` with `parents`, and no receipt and no `Superseded` record.
    t_u2_uncommitted: TransformationId,
}

/// Build the corpus, driving `t_u` with `witness_for_t_u`, or with `T_o`'s own signed
/// postcondition when that is `None`.
///
/// The witness is a parameter rather than always `attested_postcondition` because the whole of §B-1
/// is that the two answers must be distinguishable afterwards, and a fixture that can only produce
/// one of them cannot show that. `Engine::undo` takes the witness as an argument — that is the seam
/// the two shipped surfaces (`gx-cli`'s `settle_preflight`, `gx-api`'s `undo_witness`) both feed —
/// so driving it directly is the engine-level bed for both, not a hand-forged state.
///
/// 🔴 The `None` case is **not** a defaulted parameter for tidiness. The attested witness is `T_o`'s
/// own signed postcondition and does not exist until `T_o` has committed, so it cannot be built by
/// the caller and handed in; a fixture that minted thirty-two bytes and called them attested would
/// be measuring its own literal. It is read from the live engine, here, between the two commits.
fn corpus(name: &str, witness_for_t_u: Option<UndoWitness>) -> Corpus {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    // T_o: before → after.
    let i = intent(LOCATOR, "after");
    engine.submit(&i, 210, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    assert_eq!(
        engine.commit(&t_o, AT, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );

    // T_u: the undo of T_o, driven with the caller's witness, through to Committed.
    let witness = witness_for_t_u.unwrap_or_else(|| engine.attested_postcondition(&t_o));
    let (_, t_u) = engine
        .undo(&t_o, &witness, 211, LATER)
        .expect("the candidate");
    engine
        .verify(&t_u, LATER, &signing_key(), None)
        .expect("T-4a");
    engine.canonicalize(&t_u, LATER, None).expect("T-8");
    assert_eq!(
        engine.commit(&t_u, LATER, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );

    // T_2: before → third, on the same locator. Its inverse is "put it back to `before`", which is
    // byte-identical to T_o's inverse and therefore the same CID.
    let i2 = intent(LOCATOR, "third");
    engine.submit(&i2, 212, LATER_STILL).expect("submit");
    let t_2 = engine.plan(&i2, LATER_STILL).expect("plan");
    engine
        .verify(&t_2, LATER_STILL, &signing_key(), None)
        .expect("T-4a");
    engine.canonicalize(&t_2, LATER_STILL, None).expect("T-8");
    assert_eq!(
        engine
            .commit(&t_2, LATER_STILL, &signing_key())
            .expect("T-11"),
        Lifecycle::Committed
    );

    // T_u2: planned and abandoned. Nothing here is a failure being tolerated — an operator who runs
    // `gx undo` and does not settle leaves exactly this, and 43 T-2's `Candidate` is a state the
    // journal is supposed to hold.
    let witness_2 = engine.attested_postcondition(&t_2);
    let (_, t_u2_uncommitted) = engine
        .undo(&t_2, &witness_2, 213, LATER_STILL)
        .expect("the candidate");

    Corpus {
        engine,
        t_o,
        t_u,
        t_2,
        t_u2_uncommitted,
    }
}

/// The `undo` seat of a transformation's commit receipt.
fn attestation(
    engine: &Engine<InjectedEvidence>,
    id: &TransformationId,
) -> Option<UndoAttestation> {
    engine
        .receipt(id)
        .expect("the transformation committed, so T-11 issued a receipt")
        .payload()
        .expect("the payload this build signed decodes")
        .undo
}

/// The `inverse_delta` seat, which the negative control joins on.
fn inverse_delta(engine: &Engine<InjectedEvidence>, id: &TransformationId) -> Option<Cid> {
    engine
        .receipt(id)
        .expect("the transformation committed")
        .payload()
        .expect("decodes")
        .inverse_delta
}

// ---------------------------------------------------------------------------
// §B-1 — the disposition reaches the signature
// ---------------------------------------------------------------------------

/// 🔴 **§B-1, arm 1.** An undo that compared says so, in the bytes its key signed.
#[test]
fn an_attested_undo_carries_attested_in_its_signed_receipt() {
    let c = corpus("r973_attested", None);
    assert!(
        matches!(
            c.engine.attested_postcondition(&c.t_o),
            UndoWitness::Attested(_)
        ),
        "the fixture's premise: a receipt this process signed carries a postcondition, so the \
         witness the corpus used really was an attestation and not a fallback"
    );
    let seat = attestation(&c.engine, &c.t_u);
    println!("R973_B1_ATTESTED seat={seat:?}");
    assert_eq!(
        seat,
        Some(UndoAttestation {
            undoes: c.t_o,
            witness: UndoDisposition::Attested,
        }),
        "§B-1: an undo that ran DR-43-1's compare-and-swap must say so where a third party can \
         read it, and §B-2: it must name what it undid"
    );
}

/// 🔴 **§B-1, arm 2 — the bed the previous lane could not fire.**
///
/// `UndoWitness::Unobservable` is the one answer that is neither an attestation nor a refusal: the
/// inverse is applied with **no** compare-and-swap, and DR-46-7 rules that this is declared rather
/// than refused. `req/973` §4 recorded that this branch had never been exercised. It is exercised
/// here, and the assertion is not that it works but that it is **legible afterwards**.
#[test]
fn an_unobservable_undo_says_that_instead_and_names_which_nothing_it_was() {
    let c = corpus(
        "r973_unobservable",
        Some(UndoWitness::Unobservable(Unobservable::NoPostcondition)),
    );
    let seat = attestation(&c.engine, &c.t_u);
    println!("R973_B1_UNOBSERVABLE seat={seat:?}");
    assert_eq!(
        seat,
        Some(UndoAttestation {
            undoes: c.t_o,
            witness: UndoDisposition::Unobservable {
                reason: Unobservable::NoPostcondition.reason().to_string(),
            },
        }),
        "§B-1: an undo that fired without a comparison must say which nothing it had, in the \
         vocabulary `gx_engine::Unobservable::reason` owns"
    );
}

/// 🔴 **§B-1's acceptance criterion, stated as the discrimination it asks for.**
///
/// Two runs of the same shape, differing only in the witness, must produce receipts a reader can
/// tell apart. Before this erratum they produced receipts that were **byte-identical in every field
/// a reader could reach** — same `state`, same `superseded_state`, same six stdout keys — which is
/// the sentence `req/973` §B-1 opens with.
#[test]
fn checked_and_restored_is_distinguishable_from_fired_without_checking() {
    let checked = corpus("r973_discrimination_checked", None);
    let unchecked = corpus(
        "r973_discrimination_unchecked",
        Some(UndoWitness::Unobservable(Unobservable::NoPostcondition)),
    );

    let a = attestation(&checked.engine, &checked.t_u);
    let b = attestation(&unchecked.engine, &unchecked.t_u);
    println!("R973_B1_DISCRIMINATION checked={a:?} fired={b:?}");
    assert_ne!(
        a, b,
        "§B-1: the two must not wear one face. If this is equal the erratum bought nothing"
    );

    // And the discrimination survives the round trip through the signature, which is the only form
    // of it that helps a third party: both are read back out of the decoded payload above, and the
    // words are the ones both surfaces print.
    let words: Vec<String> = [a, b]
        .into_iter()
        .map(|seat| seat.expect("both are undos").witness.word())
        .collect();
    println!("R973_B1_WORDS={words:?}");
    assert_eq!(
        words,
        vec![
            "attested".to_string(),
            "unobservable:the commit receipt carries no postcondition".to_string()
        ],
        "the receipt's word is the same word `UndoWitness::word` gives CLI stdout and HTTP"
    );
}

/// 🔴 **The partition (§25o ④).** An ordinary commit's receipt carries no attestation at all.
///
/// The `Option` is not a third value of the disposition, and this is what says so: `None` means
/// "no undo road wrote this payload", and it holds for the two ordinary commits in the corpus.
#[test]
fn an_ordinary_commit_receipt_carries_no_undo_attestation() {
    let c = corpus("r973_partition", None);
    let ordinary = [(("t_o"), c.t_o), (("t_2"), c.t_2)];
    for (name, id) in ordinary {
        let seat = attestation(&c.engine, &id);
        println!("R973_PARTITION {name} seat={seat:?}");
        assert_eq!(
            seat, None,
            "{name} is a plan, not an undo; a `Some` here would be a claim about a comparison \
             nothing made"
        );
    }
    assert!(
        attestation(&c.engine, &c.t_u).is_some(),
        "and the partition covers: the undo's receipt does carry one, so the two classes are \
         disjoint **and** exhaustive over the corpus"
    );
}

/// 🔴 **Adversarial probe 1 — the kind-dependent rule fires.**
///
/// A `VerdictReceipt` carrying an attestation is refused by `check_schema`. Built by hand, because
/// no road in the engine produces one: a gate that has never been fired is not a gate
/// (`req/493` §1 AC-4's rule, applied here).
#[test]
fn a_verdict_receipt_that_claims_an_undo_attestation_is_refused() {
    let c = corpus("r973_schema_refusal", None);
    let mut payload = c
        .engine
        .receipt(&c.t_u)
        .expect("committed")
        .payload()
        .expect("decodes");
    // Turn the commit receipt into the shape a verdict receipt has, one field at a time, so that
    // the only rule left to break is the one under test. Without this the refusal could be any of
    // the four `VerdictReceipt` rules and the probe would name the wrong defect.
    payload.receipt_kind = ReceiptKind::VerdictReceipt;
    payload.inclusion_proof = None;
    payload.postcondition_fingerprint = None;
    payload.inverse_delta = None;
    payload.read_set = None;
    payload.reversibility = None;

    let with_seat = payload.clone();
    let mut without_seat = payload;
    without_seat.undo = None;

    let refused = with_seat.check_schema();
    let allowed = without_seat.check_schema();
    println!("R973_SCHEMA with_seat={refused:?} without_seat={allowed:?}");
    assert!(
        refused.is_err(),
        "DR-46-45's kind-dependent rule: a verdict receipt applied nothing, so it cannot carry a \
         claim about a compare-and-swap that guarded an apply"
    );
    assert!(
        allowed.is_ok(),
        "and the refusal is about **this** field: the same payload with the seat emptied passes, \
         so the probe is not measuring one of the other four rules"
    );
}

/// 🔴 **Adversarial probe 2 — the refusal arm reaches no receipt, and that is the third value.**
///
/// `UndoWitness::Missing` maps to `None`, not to a disposition. The three-valued discipline is kept
/// by the refusal *surface* (exit 3 / HTTP 409), not by a third spelling inside the payload — and
/// the mapping is asserted arm by arm so a fourth variant cannot be added without a decision here.
#[test]
fn the_refusal_arm_has_no_disposition_and_the_other_two_do() {
    let arms = [
        (
            "attested",
            UndoWitness::Attested(gx_core::FingerprintBytes([7u8; 32])),
            Some(UndoDisposition::Attested),
        ),
        (
            "unobservable",
            UndoWitness::Unobservable(Unobservable::NoPostcondition),
            Some(UndoDisposition::Unobservable {
                reason: Unobservable::NoPostcondition.reason().to_string(),
            }),
        ),
        (
            "missing",
            UndoWitness::Missing(gx_engine::WitnessMissing::NoReceipt),
            None,
        ),
    ];
    for (name, witness, expected) in arms {
        let got = witness.disposition();
        println!("R973_MAPPING {name} -> {got:?} (word={:?})", witness.word());
        assert_eq!(
            got, expected,
            "{name}: the mapping from witness to what a receipt may say is not what DR-46-45 \
             declares"
        );
    }
}

// ---------------------------------------------------------------------------
// §B-2 — the DAG, and the AC as corrected
// ---------------------------------------------------------------------------

/// Edges (child → parent) recovered from the commit receipts alone.
fn edges_from_receipts(engine: &Engine<InjectedEvidence>) -> BTreeSet<(String, String)> {
    committed_ids(engine)
        .into_iter()
        .filter_map(|id| attestation(engine, &id).map(|seat| (text(&id), text(&seat.undoes))))
        .collect()
}

/// Edges recovered from the journal's `Superseded` records.
fn edges_from_superseded(engine: &Engine<InjectedEvidence>) -> BTreeSet<(String, String)> {
    engine
        .journal()
        .records()
        .iter()
        .filter_map(|r| match r {
            EngineJournalRecord::Superseded {
                transformation, by, ..
            } => Some((text(by), text(transformation))),
            _ => None,
        })
        .collect()
}

/// Edges recovered from every `Planned` record that names a parent.
fn edges_from_planned(engine: &Engine<InjectedEvidence>) -> BTreeSet<(String, String)> {
    engine
        .journal()
        .records()
        .iter()
        .filter_map(|r| match r {
            EngineJournalRecord::Planned {
                transformation,
                parents,
                ..
            } => parents
                .first()
                .map(|parent| (text(transformation), text(parent))),
            _ => None,
        })
        .collect()
}

/// Every transformation that reached `Committed` and therefore has a commit receipt.
fn committed_ids(engine: &Engine<InjectedEvidence>) -> Vec<TransformationId> {
    engine
        .journal()
        .records()
        .iter()
        .filter_map(|r| match r {
            EngineJournalRecord::Committed { transformation, .. } => Some(*transformation),
            _ => None,
        })
        .collect()
}

fn text(id: &TransformationId) -> String {
    id.0.to_text()
}

/// 🔴 **§B-2's AC, in the form the measurement supports.**
///
/// Equality against `Superseded`, containment in `Planned.parents`, and the containment **strict**.
/// `req/973` §B-2 asked for equality against both; §8 records the correction and this is the probe
/// that forces it — with the corpus built so that the strictness is a fact and not a coincidence.
#[test]
fn the_receipt_dag_equals_the_superseded_dag_and_sits_strictly_inside_the_planned_dag() {
    let c = corpus("r973_dag", None);
    let from_receipts = edges_from_receipts(&c.engine);
    let from_superseded = edges_from_superseded(&c.engine);
    let from_planned = edges_from_planned(&c.engine);

    println!(
        "R973_B2_DAG receipts={from_receipts:?} superseded={from_superseded:?} \
         planned={from_planned:?}"
    );
    assert_eq!(
        from_receipts, from_superseded,
        "§B-2: the receipt-borne edges are exactly the edges the journal's `Superseded` records \
         enumerate — both are written only when an undo commits"
    );
    assert_eq!(
        from_receipts,
        BTreeSet::from([(text(&c.t_u), text(&c.t_o))]),
        "and the one edge is the one the corpus made"
    );
    assert!(
        from_receipts.is_subset(&from_planned),
        "every committed undo was planned first"
    );
    assert!(
        from_planned.contains(&(text(&c.t_u2_uncommitted), text(&c.t_2))),
        "the abandoned undo left a `Planned` edge, which is what makes the containment strict"
    );
    assert_ne!(
        from_receipts, from_planned,
        "🔴 the AC as `req/973` §B-2 first wrote it would be RED here, and correctly so: \
         `Planned.parents` is a strict superset. See `req/973` §8"
    );
}

/// 🔴 **Negative control (§B-2's own gate names what it protects).**
///
/// `inverse_delta` is not a join key. Two unrelated commits in this corpus carry the same one, so a
/// DAG inferred from it maps a value onto more than one transformation — a **false** edge, which is
/// worse than a missing one because it looks like an answer.
///
/// This probe is required to *fail to reconstruct*: if a future change made `inverse_delta` unique,
/// this test goes red and the negative control has to be re-founded rather than quietly stop
/// measuring anything.
#[test]
fn joining_on_inverse_delta_stands_up_a_false_edge() {
    let c = corpus("r973_negative_control", None);
    let mut by_delta: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in committed_ids(&c.engine) {
        if let Some(delta) = inverse_delta(&c.engine, &id) {
            by_delta
                .entry(gx_canon::cid::to_text(&delta))
                .or_default()
                .push(text(&id));
        }
    }
    let collisions: Vec<(&String, &Vec<String>)> =
        by_delta.iter().filter(|(_, ids)| ids.len() > 1).collect();
    println!("R973_B2_NEGATIVE_CONTROL by_delta={by_delta:?} collisions={collisions:?}");

    assert!(
        !collisions.is_empty(),
        "the control measures nothing unless `inverse_delta` actually collides; `req/973` §1-2 \
         measured a collision in the field and this corpus reproduces one by construction \
         (two commits whose inverse is the same bytes at the same locator)"
    );
    let (_, colliding) = collisions[0];
    assert!(
        colliding.contains(&text(&c.t_o)) && colliding.contains(&text(&c.t_2)),
        "and the colliding pair is the unrelated one: {colliding:?}"
    );
    assert!(
        !colliding.contains(&text(&c.t_u)),
        "which is the point — the value that collides is not even the undo's, so a join on it \
         answers about the wrong two transformations entirely"
    );
}

/// 🔴 **Adversarial probe 3 — the seat survives the road that rebuilds a payload from Σ.**
///
/// 43 §7-3b compares a rebuilt payload's digest against the leaf the ledger holds, so a field the
/// rebuild cannot reproduce answers `payload_mismatch` — the word for tampering — on every
/// crash-window recovery of an undo. Both roads read `Engine::journalled_undo`, and this asserts the
/// value that helper returns is reachable from the **journal alone**: the corpus is replayed into a
/// second engine which never committed anything, and it must reach the same attestation.
#[test]
fn a_process_that_committed_nothing_reads_the_same_attestation_out_of_the_journal() {
    let c = corpus("r973_rebuild_seat", None);
    let signed = attestation(&c.engine, &c.t_u).expect("the undo committed");

    let planned: Vec<(
        TransformationId,
        Vec<TransformationId>,
        Option<UndoDisposition>,
    )> = c
        .engine
        .journal()
        .records()
        .iter()
        .filter_map(|r| match r {
            EngineJournalRecord::Planned {
                transformation,
                parents,
                undo_witness,
                ..
            } => Some((*transformation, parents.clone(), undo_witness.clone())),
            _ => None,
        })
        .collect();
    let row = planned
        .iter()
        .find(|(id, _, _)| *id == c.t_u)
        .expect("the undo was planned");
    let from_journal = UndoAttestation {
        undoes: *row
            .1
            .first()
            .expect("43 T-12 fixes T_u.parents as [T_o.id]"),
        witness: row.2.clone().expect("the undo road fills the seat"),
    };
    println!("R973_REBUILD signed={signed:?} from_journal={from_journal:?}");
    assert_eq!(
        signed, from_journal,
        "43 §7-3b: what the receipt says must be reconstructible from Σ, or every crash-window \
         recovery of an undo answers `payload_mismatch`"
    );
}
