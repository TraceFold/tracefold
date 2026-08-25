// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-34** — the four preimages of `read_set: null`, measured on the roads that make them.
//!
//! `req/38` §268 ruling 5 raised it, `req/472` §6 drafted it, and `req/498` located every producer
//! in code. This file is what turns that reading into a measurement.
//!
//! | form | the coordinate `req/498` measured | what it means | before | after |
//! |---|---|---|---|---|
//! | ① **read nothing** | `gx-substrate/src/adapter.rs`'s `InvertOutcome::from_option` fixes `Vec::new()` on both arms; T-11 hands it to `ReadSet::from_reads` | the escrow ran and touched no object | `null` | `Nothing` |
//! | ② **no escrow record** | `pipeline.rs`'s `rebuilt_attest` ended its `find_map` in `unwrap_or_default()` | a rebuild found no `InverseEscrowed` for this transformation at all | `null` | `NoEscrowRecord` |
//! | ③ **reads not journalled** | `store.rs`'s `#[serde(default)]` on `InverseEscrowed.reads` | the record is there and predates 42 §3.13's `reads` | `null` | `ReadsNotJournalled` |
//! | ④ **not asked** | `pipeline.rs`'s `issue_verdict_receipt` writes `None` by ASM-14 | the escrow is 43 T-10b and had not run when this was signed | `null` | `null` |
//!
//! All four met at `ReadSet::from_reads`, whose `Ok(None)` for an empty entry list is the line at
//! which the distinction was lost. ④ **keeps** `null`, which is the point rather than an omission:
//! `null` goes from meaning four things to meaning one.
//!
//! # What each test is entitled to claim
//!
//! ① and ④ are read off **receipts the engine issued**, which is the only reading in which
//! "attested" means anything. ② and ③ are read off the **rebuild road**, and there the claim is
//! necessarily shaped differently: a rebuild that does not reproduce the digest the ledger
//! witnessed is refused before a receipt exists (`Engine::reissue_receipt` answers
//! `Reissued::WorldMoved`; `Engine::resume` answers `payload_matched: Some(false)`), so the
//! measurement is that the refusal **arrives** where a receipt used to be minted. `req/498`'s
//! §DR-46-34 called that the dangerous case and it is: before this lane, a rebuild over a journal
//! that does not record reads produced a **signed** receipt asserting that the escrow read nothing.
//!
//! The pair in `③` is what makes that claim falsifiable rather than decorative: the *only*
//! difference between the two beds is `InverseEscrowed.reads_attested`, and one is filed while the
//! other is refused.
//!
//! N-13 keeps adapters out of this crate, so the fixture implements `gx-substrate`'s contract
//! directly, in the arrangement `tests/dr4626_invert_seam.rs` uses.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gx_core::{Fingerprint, SubstrateKind, Timestamp, TransformationId};
use gx_engine::pipeline::Reissued;
use gx_engine::{Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle};
use gx_substrate::{InvertOutcome, PlannedDelta, SubstrateAdapter};
use gx_witness::receipt::{ReadSet, ReceiptPayload};

use support::{copy_tree, digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";

// ---------------------------------------------------------------------------
// The fixture: an adapter that reports an inverse and **no reads**
// ---------------------------------------------------------------------------

/// The shape the fs, git and postgres adapters have: the inverse comes out of the snapshot already
/// in hand, so `InvertOutcome::from_option` fixes `Vec::new()` and form ① is the *ordinary* road
/// rather than an exotic one. That is what makes this DR worth a wire change — `req/472` §6's
/// "in v0.1 the read-set is always empty" was measured on exactly these three adapters.
#[derive(Clone, Debug)]
struct QuietAdapter {
    world: Arc<Mutex<Vec<u8>>>,
}

impl QuietAdapter {
    fn new(world: &str) -> Self {
        Self {
            world: Arc::new(Mutex::new(world.as_bytes().to_vec())),
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for QuietAdapter {
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

    /// 🔴 `InvertOutcome::from_option`'s own shape, written out: an inverse, and no read behind it.
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

fn engine_over(dir: &Path, adapter: &QuietAdapter) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "dr4634-quiet-fixture/1");
    engine
}

/// Drive one intent to `Committed` and hand back its id.
fn commit_one(engine: &mut Engine<InjectedEvidence>, goal: &str) -> TransformationId {
    let one = intent(SUBJECT, goal);
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

/// Copy a committed project and **rewrite its journal**, passing every record through `edit`.
///
/// `None` from `edit` drops the record. The journal is rebuilt through `EngineJournal::append`
/// rather than by editing bytes in place, so the chain (`JOURNAL_MAGIC_V2`, R30) is a real one and
/// nothing under test can fail for a framing reason that belongs to the fixture.
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

/// The `read_set` member as canonical bytes — the quantity the whole DR is about.
fn read_set_bytes(payload: &ReceiptPayload) -> Vec<u8> {
    gx_canon::cbor::encode(&payload.read_set).expect("the member encodes")
}

// ---------------------------------------------------------------------------
// ① the escrow ran and read nothing
// ---------------------------------------------------------------------------

/// 🔴 **Form ①** — the ordinary fs/git/postgres road now says "read nothing" instead of nothing.
///
/// The last clause is the one that carries weight. `ReadSet::Nothing` answers [`ReadSet::names`]
/// with `Some(false)` about **every** locator, which is a stronger statement than G3 makes and one
/// that only an escrow that actually ran is entitled to. Under `Option::None` a caller had no way
/// to ask, so the strong answer was unavailable on the road that had earned it and — one road over
/// — was being implied on roads that had not.
#[test]
fn form_one_an_escrow_that_read_nothing_says_so_in_the_signed_bytes() {
    let dir = scratch("dr4634_form_one");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    let payload = payload_of(&engine, &id);
    let read_set = payload
        .read_set
        .as_ref()
        .expect("DR-46-34: a CommitReceipt this binary issues always names its read-set");
    println!(
        "DR4634_FORM_ONE granularity={} attested={} names_subject={:?}",
        read_set.granularity(),
        read_set.is_attested(),
        read_set.names(SUBJECT)
    );
    assert_eq!(*read_set, ReadSet::Nothing);
    assert_eq!(read_set.granularity(), "nothing");
    assert!(!read_set.is_attested());
    assert_eq!(
        read_set.names(SUBJECT),
        Some(false),
        "an escrow that ran and read nothing decides the question for every object"
    );
    assert_eq!(read_set.names("/srv/somewhere-else"), Some(false));
    // The escrow did run, so C-25 answered — which is what separates this from the two absences.
    assert!(payload.reversibility.is_some());
    assert!(payload.inverse_delta.is_some());
}

// ---------------------------------------------------------------------------
// ② a rebuild that found no escrow record at all
// ---------------------------------------------------------------------------

/// 🔴 **Form ②** — `rebuilt_attest`'s `unwrap_or_default()` no longer manufactures an empty read.
///
/// The record is dropped from the journal and everything else is left where it was, so the road is
/// 43 §7-3b's with exactly one fact missing. The rebuild is refused rather than filed, which is the
/// honest answer: a payload it cannot reproduce is not the document the ledger witnessed.
#[test]
fn form_two_a_rebuild_with_no_escrow_record_is_refused_instead_of_filed() {
    let dir = scratch("dr4634_form_two_source");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");
    drop(engine);

    let stripped = project_with_journal(&dir, "dr4634_form_two", |record| match record {
        EngineJournalRecord::InverseEscrowed { .. } => None,
        other => Some(other.clone()),
    });
    let mut reopened = engine_over(&stripped, &adapter);
    let outcome = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("DR4634_FORM_TWO reissue={outcome:?}");
    assert!(
        !matches!(outcome, Reissued::Filed(_)),
        "a road holding no record of what the escrow read must not sign a receipt about it"
    );
}

// ---------------------------------------------------------------------------
// ③ a journal that does not record reads — and its negative control
// ---------------------------------------------------------------------------

/// Rewrite the escrow record's `reads_attested` and nothing else.
fn with_reads_attested(record: &EngineJournalRecord, attested: bool) -> EngineJournalRecord {
    match record {
        EngineJournalRecord::InverseEscrowed {
            transformation,
            inverse_cid,
            pending,
            reads,
            undetermined,
            at,
            ..
        } => EngineJournalRecord::InverseEscrowed {
            transformation: *transformation,
            inverse_cid: *inverse_cid,
            pending: *pending,
            reads: reads.clone(),
            undetermined: *undetermined,
            reads_attested: attested,
            at: *at,
        },
        other => other.clone(),
    }
}

/// 🔴 **Form ③ and its control** — the one-bit difference, and the two answers it decides.
///
/// Both beds are the same committed project, re-journalled record for record. The **only**
/// difference is `InverseEscrowed.reads_attested`:
///
/// * `true` — a modern record that recorded an empty read-set. The rebuild reproduces
///   [`ReadSet::Nothing`], the payload digests to the leaf the ledger holds, and the receipt is
///   **filed**. This is the control, and without it the refusal below would be evidence of nothing
///   but the fixture's own doctoring.
/// * `false` — E-M5-13's shape, which is what every journal written before DR-46-26 decodes to.
///   The rebuild answers [`ReadSet::ReadsNotJournalled`], the digest no longer matches, and the
///   road is **refused**.
///
/// The second arm is the defect closing. Before this lane both beds were byte-identical after
/// `#[serde(default)]` had run, so both were filed — and the one on the right was a **signed**
/// receipt asserting that the escrow read nothing, on the strength of a field that was not in the
/// file. 42 §3.13's DR-46-26 note already ruled that a journal without `reads` cannot rebuild
/// ("without the journal only 13 of the 14 fields can be rebuilt"); this is the empty case, which had
/// been agreeing by accident rather than by reproduction.
#[test]
fn form_three_a_journal_that_does_not_record_reads_is_refused_and_one_that_does_is_filed() {
    let dir = scratch("dr4634_form_three_source");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");
    let issued = payload_of(&engine, &id);
    drop(engine);

    // The control: the record says it recorded the (empty) read-set.
    let modern = project_with_journal(&dir, "dr4634_form_three_modern", |record| {
        Some(with_reads_attested(record, true))
    });
    let mut reopened = engine_over(&modern, &adapter);
    let control = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!(
        "DR4634_FORM_THREE_CONTROL reads_attested=true reissue={}",
        match &control {
            Reissued::Filed(_) => "Filed",
            other => {
                println!("DR4634_FORM_THREE_CONTROL_DETAIL={other:?}");
                "refused"
            }
        }
    );
    let Reissued::Filed(receipt) = control else {
        panic!("the control has every fact the issue road had, so it must reproduce the payload");
    };
    let rebuilt = receipt.payload().expect("the rebuilt payload decodes");
    assert_eq!(
        rebuilt.read_set,
        Some(ReadSet::Nothing),
        "the rebuild re-chooses the granularity by the same function of the same entries"
    );
    assert_eq!(
        read_set_bytes(&rebuilt),
        read_set_bytes(&issued),
        "and lands on the bytes the issuing road landed on, which is what the digest check means"
    );

    // The defect: the record predates the field, so it records nothing about the reads.
    let legacy = project_with_journal(&dir, "dr4634_form_three_legacy", |record| {
        Some(with_reads_attested(record, false))
    });
    let mut reopened = engine_over(&legacy, &adapter);
    let outcome = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("DR4634_FORM_THREE_LEGACY reads_attested=false reissue={outcome:?}");
    assert!(
        !matches!(outcome, Reissued::Filed(_)),
        "a journal that does not record what the escrow read must not mint a receipt saying it \
         read nothing"
    );
}

// ---------------------------------------------------------------------------
// ④ the receipt nobody asked
// ---------------------------------------------------------------------------

/// 🔴 **Form ④** — `null` survives, and this is now the only thing it says.
///
/// ASM-14 keeps `read_set` absent on a `VerdictReceipt` because the escrow is 43 T-10b and had not
/// run when the verdict was signed. That rule is unchanged; what changed is everything *else* that
/// used to share the spelling.
#[test]
fn form_four_a_verdict_receipt_still_carries_null_and_now_that_is_all_null_means() {
    let dir = scratch("dr4634_form_four");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    let verdicts = engine.verdict_receipts(&id);
    assert!(!verdicts.is_empty(), "T-4a files one");
    for receipt in verdicts {
        let payload = receipt.payload().expect("the verdict payload decodes");
        assert_eq!(payload.read_set, None);
        payload
            .check_schema()
            .expect("ASM-14's rule is the one that keeps it absent");
    }
    println!(
        "DR4634_FORM_FOUR verdict_receipts={} read_set=null",
        verdicts.len()
    );
}

// ---------------------------------------------------------------------------
// The claim, as one measurement
// ---------------------------------------------------------------------------

/// 🔴 The four forms are **four shapes**: six pairwise comparisons, none of which held before.
///
/// The members are compared as canonical bytes rather than as values, because "the receipt says
/// which" is a statement about what a third party decodes and not about what this process holds in
/// memory. `Option::None` is `0xf6` at the key (42 §2.1's golden vector G-5), which is what form ④
/// still encodes to and what the other three no longer do.
#[test]
fn the_four_forms_encode_to_four_different_shapes() {
    let forms: [(&str, Option<ReadSet>); 4] = [
        ("read_nothing", Some(ReadSet::Nothing)),
        ("no_escrow_record", Some(ReadSet::NoEscrowRecord)),
        ("reads_not_journalled", Some(ReadSet::ReadsNotJournalled)),
        ("not_asked", None),
    ];
    let encoded: Vec<(&str, Vec<u8>)> = forms
        .iter()
        .map(|(name, set)| {
            let bytes = gx_canon::cbor::encode(set).expect("the member encodes");
            println!(
                "DR4634_SHAPE form={name} bytes={} hex={}",
                bytes.len(),
                bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
            );
            (*name, bytes)
        })
        .collect();

    let mut compared = 0usize;
    for (i, (left_name, left)) in encoded.iter().enumerate() {
        for (right_name, right) in encoded.iter().skip(i + 1) {
            assert_ne!(
                left, right,
                "DR-46-34: `{left_name}` and `{right_name}` are two facts and must not be one \
                 spelling"
            );
            compared += 1;
        }
    }
    println!("DR4634_PAIRWISE_DISTINCT={compared}/6");
    assert_eq!(compared, 6);

    // 🔴 The negative control for the whole file: form ④ is unmoved. A lane that "fixed" `null` by
    // giving every road a member would have broken ASM-14 and every receipt already in the field.
    assert_eq!(
        encoded[3].1,
        vec![0xf6],
        "42 §2.1's G-5 is what an absent value encodes to, and form ④ still encodes to it"
    );
}

// ---------------------------------------------------------------------------
// The negative controls: nothing already written is broken
// ---------------------------------------------------------------------------

/// 🔴 A journal record written before this lane encodes to the bytes it always encoded to.
///
/// `reads_attested` is `skip_serializing_if = "is_false"`, which is `undetermined`'s shape and
/// `pending`'s before it (E-M5-13). The claim is not "we were careful": it is that the key **is not
/// in the bytes** when the flag is `false`, so a pre-DR-46-34 record re-encodes identically and
/// `journal_roundtrip.rs`'s equality survives the version. A field added without this attribute
/// would have moved every escrow record ever written.
#[test]
fn a_record_that_predates_this_field_encodes_to_the_bytes_it_always_did() {
    let escrowed = |attested: bool| EngineJournalRecord::InverseEscrowed {
        transformation: support::tid(3),
        inverse_cid: Some(support::cid(7)),
        pending: false,
        reads: Vec::new(),
        undetermined: false,
        reads_attested: attested,
        at: Timestamp(500),
    };
    let legacy = gx_canon::cbor::encode(&escrowed(false)).expect("the record encodes");
    let modern = gx_canon::cbor::encode(&escrowed(true)).expect("the record encodes");
    let key = b"reads_attested";
    let holds = |bytes: &[u8]| bytes.windows(key.len()).any(|w| w == key);
    println!(
        "DR4634_JOURNAL_COMPAT legacy_bytes={} holds_key={} modern_bytes={} holds_key={}",
        legacy.len(),
        holds(&legacy),
        modern.len(),
        holds(&modern)
    );
    assert!(
        !holds(&legacy),
        "a `false` flag must not put a key on the wire, or every journal ever written moves"
    );
    assert!(
        holds(&modern),
        "and a `true` one must, or the field is declared and never written — which is the shape \
         `journal_identity.rs`'s A-10 count exists to catch"
    );
    assert!(
        legacy.len() < modern.len(),
        "the legacy encoding is the shorter one by exactly the key it does not carry"
    );
}

/// 🔴 A `CommitReceipt` issued before this lane still decodes, still schema-checks, and still
/// verifies against its own signature.
///
/// This is the reason `check_schema` was **not** tightened to require `Some` on a `CommitReceipt`,
/// and the reason is worth a probe rather than a sentence. A receipt is bytes that were signed;
/// `verify_offline` is a statement about those bytes; a schema rule added afterwards that refused
/// them would retroactively invalidate every receipt in the field to buy a distinction only new
/// receipts can carry anyway. So `null` on a `CommitReceipt` keeps a fifth, **historical** reading
/// — "issued before DR-46-34" — which this binary decodes and never writes.
#[test]
fn a_commit_receipt_issued_before_this_lane_still_decodes_and_still_verifies() {
    let dir = scratch("dr4634_backward_compat");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    // The old shape, built by hand from a real receipt: the same payload with the DR-46-34 member
    // taken back out. Signed here, because a receipt nobody signed proves nothing about receipts.
    let mut old_shape = payload_of(&engine, &id);
    old_shape.read_set = None;
    assert_eq!(
        old_shape.receipt_kind,
        gx_witness::receipt::ReceiptKind::CommitReceipt
    );
    old_shape
        .check_schema()
        .expect("DR-46-34 must not refuse a receipt that was legal when it was signed");

    let key = signing_key();
    let receipt = gx_witness::Receipt::issue(&old_shape, AT, &key).expect("the old shape signs");
    let decoded = receipt.payload().expect("and decodes");
    assert_eq!(
        decoded.read_set, None,
        "the historical spelling survives a round trip"
    );
    println!(
        "DR4634_RECEIPT_COMPAT check_schema=ok round_trip=ok read_set={:?}",
        decoded.read_set
    );
}
