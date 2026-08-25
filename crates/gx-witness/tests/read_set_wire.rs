// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! DR-46-24(A) — the read-set on the wire: both granularities fire, and the tag says which.
//!
//! `req/350` §7-7 asks for two things this file is, and says why the second is not implied by the
//! first: "run **both values of the granularity tag** through a path that actually reaches them,
//! once each (**introducing is not functioning**)". A type with two variants and a constructor that
//! only ever builds one of them is a type whose second variant nobody has ever seen signed.
//!
//! It also holds the three machines P2 rests on. `req/440` §0-4 makes `fingerprint_scope` **one**
//! field on the condition that the invariant "pre and post are over the same scope" is *checked*
//! first rather than assumed. It is checked — by machinery that was already here — and the check is
//! that these three coordinates still say what they say.

mod support;

use gx_canon::cbor;
use gx_core::VerdictKind;
use gx_witness::receipt::ReceiptPayload;
use gx_witness::receipt::{
    read_set_fold, read_set_leaves, read_set_path, ReadEntry, ReadSet, ReceiptKind,
    READ_SET_SPILL_THRESHOLD,
};
use gx_witness::{verify_offline, Error, Receipt};
use support::{commit_payload, issue, keypair, tid, verdict_payload};

/// A commit receipt whose payload the caller shaped, appended to a real log and signed over the
/// digest that log holds.
///
/// `support::commit_receipt_in_a_log` builds its own payload, so it cannot carry a read-set; this
/// is the same six lines with the payload as an argument. The order is the protocol's and not a
/// convenience: the payload is digested **without** its proof, appended, and only then completed
/// and signed (`ReceiptPayload::ledger_digest`).
fn signed_over_a_real_log(
    key: &gx_witness::KeyPair,
    seed: u64,
    mut payload: ReceiptPayload,
) -> (Receipt, gx_core::Checkpoint) {
    use gx_log::{proof, TileLog};

    let mut log = TileLog::new();
    for i in 0..4u64 {
        log.append(
            tid(900_000 + i),
            gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[&i.to_be_bytes()]),
            gx_core::Timestamp(i as i64),
        )
        .expect("canonical");
    }
    payload.inclusion_proof = Some(gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 0,
        audit_path: Vec::new(),
    });
    let index = log.len();
    log.append(
        payload.transformation,
        payload.ledger_digest().expect("canonical"),
        gx_core::Timestamp(1),
    )
    .expect("canonical");
    payload.inclusion_proof =
        Some(proof::prove_inclusion(&log, index).expect("the entry is in the log"));
    let head = proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", gx_core::Timestamp(2))
        .expect("a non-empty log has a head");
    let _ = seed;
    (issue(&payload, key), head)
}

/// The proof seat a payload holds while its own ledger digest is being taken.
fn placeholder_proof() -> gx_core::InclusionProof {
    gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 0,
        audit_path: Vec::new(),
    }
}

/// The full-locator form `req/350` §2-3 measured the receipt as actually carrying.
fn entries(n: usize) -> Vec<ReadEntry> {
    (0..n)
        .map(|i| ReadEntry {
            digest: gx_canon::cid::mint(
                gx_canon::cid::Domain::Leaf,
                &[format!("prior-{i}").as_bytes()],
            ),
            locator: format!("mcp://server-{i:04}/resource/notes/{i:04}/body.md#frag"),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Both values of the tag, each through a path that reaches it
// ---------------------------------------------------------------------------

/// 🔴 **G3 fires**, is signed, verifies offline, and decides the question from the receipt alone.
#[test]
fn g3_reaches_a_signed_receipt_and_the_receipt_alone_answers() {
    let key = keypair(3);
    let mut payload = commit_payload(&key, 3, placeholder_proof());
    let set = ReadSet::from_reads(entries(READ_SET_SPILL_THRESHOLD)).expect("the entries encode");
    assert_eq!(
        set.granularity(),
        "G3",
        "at the threshold the default holds"
    );
    assert_eq!(set.distinct_objects(), READ_SET_SPILL_THRESHOLD as u64);
    payload.read_set = Some(set);

    let (receipt, checkpoint) = signed_over_a_real_log(&key, 3, payload);
    let checks = verify_offline(&receipt, &key.verifying(), Some(&checkpoint))
        .expect("a legal commit receipt verifies");
    assert!(checks.verified(), "{checks:?}");

    // The whole of `req/350` §3's claim for G3, executed: the receipt **alone** decides.
    let decoded: gx_witness::receipt::ReceiptPayload =
        cbor::decode(&receipt.envelope.payload).expect("the payload round trips");
    let set = decoded.read_set.expect("the read-set survived the wire");
    println!(
        "G3_GRANULARITY={} N={}",
        set.granularity(),
        set.distinct_objects()
    );
    assert_eq!(
        set.names("mcp://server-0003/resource/notes/0003/body.md#frag"),
        Some(true),
        "an object the escrow read"
    );
    assert_eq!(
        set.names("mcp://server-9999/resource/notes/9999/body.md#frag"),
        Some(false),
        "an object it did not"
    );
}

/// 🔴 **G4 fires**, and answers the same question honestly by declining to answer it alone.
///
/// The spill is not asserted by setting the variant; it is produced by handing
/// [`ReadSet::from_reads`] one more object than the threshold, which is the only road either
/// variant is built on.
#[test]
fn g4_reaches_a_signed_receipt_and_says_out_loud_that_it_cannot_decide_alone() {
    let key = keypair(4);
    let mut payload = commit_payload(&key, 4, placeholder_proof());
    let all = entries(READ_SET_SPILL_THRESHOLD + 1);
    let set = ReadSet::from_reads(all.clone()).expect("the entries encode");
    assert_eq!(
        set.granularity(),
        "G4",
        "one past the threshold spills, and the spill is the constructor's and not a caller's"
    );
    payload.read_set = Some(set);

    let (receipt, checkpoint) = signed_over_a_real_log(&key, 4, payload);
    assert!(
        verify_offline(&receipt, &key.verifying(), Some(&checkpoint))
            .expect("a legal commit receipt verifies")
            .verified()
    );

    let decoded: gx_witness::receipt::ReceiptPayload =
        cbor::decode(&receipt.envelope.payload).expect("the payload round trips");
    let set = decoded.read_set.expect("the read-set survived the wire");
    println!(
        "G4_GRANULARITY={} N={}",
        set.granularity(),
        set.distinct_objects()
    );
    assert_eq!(
        set.names("mcp://server-0003/resource/notes/0003/body.md#frag"),
        None,
        "a root is a digest; a digest with no preimage decides nothing, and saying so is the point \
         of the tag"
    );

    // And the decision it *can* support, given the entries from beside the receipt: the path is
    // derived here rather than carried, which is the placement `tests/d24_read_set_cost.rs` costed.
    let ReadSet::PerEffectRoot { root, leaf_count } = set else {
        panic!("the spill built the wrong variant");
    };
    let mut sorted = all;
    sorted.sort();
    sorted.dedup();
    let leaves = read_set_leaves(&sorted).expect("the entries encode");
    for (i, leaf) in leaves.iter().enumerate() {
        let mut path = Vec::new();
        read_set_path(i, &leaves, &mut path);
        assert_eq!(
            read_set_fold(i as u64, leaf, &path, leaf_count),
            Some(root),
            "entry {i} does not fold to the root the receipt signed"
        );
    }
    // A stranger does not, which is the half that makes the fold a check rather than a ritual.
    let stranger = gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[b"never read"]);
    let mut path = Vec::new();
    read_set_path(0, &leaves, &mut path);
    assert_ne!(read_set_fold(0, &stranger, &path, leaf_count), Some(root));
    // And a path of the wrong length is refused rather than folded to something.
    assert_eq!(
        read_set_fold(0, &leaves[0], &path[..1], leaf_count),
        None,
        "a short path has to be refused; folding it would answer about a tree nobody built"
    );
}

/// The empty read-set is an absence, not a root over nothing.
///
/// 🔴 **DR-46-34** — and the absence is now a **named** one. `from_reads` used to answer
/// `Ok(None)` here, which is the exact coordinate `req/472` §6 measured: from that arm onward
/// "the escrow read nothing" was the same bytes as "nobody recorded what it read". It answers
/// `ReadSet::Nothing` instead, and the clause below is the one that makes the difference
/// load-bearing rather than cosmetic — `Nothing` decides `names` for **every** locator, which no
/// absence is entitled to do.
#[test]
fn an_escrow_that_read_nothing_carries_the_named_absence_and_not_a_root() {
    let nothing = ReadSet::from_reads(Vec::new()).expect("nothing encodes");
    assert_eq!(nothing, ReadSet::Nothing);
    assert_eq!(nothing.granularity(), "nothing");
    assert_eq!(nothing.distinct_objects(), 0);
    assert!(!nothing.is_attested());
    assert_eq!(
        nothing.names("mcp://fixture/resource/notes/0000/body.md"),
        Some(false),
        "an escrow that read nothing decides the question for every object, from the receipt alone"
    );
    for absence in [ReadSet::NoEscrowRecord, ReadSet::ReadsNotJournalled] {
        assert_eq!(
            absence.names("mcp://fixture/resource/notes/0000/body.md"),
            None,
            "the two absences that hold nothing decide nothing, which is the half `Ok(None)` used              to give away for free"
        );
    }
}

/// Duplicates are one object, which is what makes this a *set*.
///
/// `req/350` §1's central correction: the escrow road takes **nine** read events per committed call
/// and they are all about **one** object, so "nine entries" was never the quantity. The constructor
/// is where that stops being a remark.
#[test]
fn nine_reads_of_one_object_are_one_entry() {
    let one = entries(1);
    let nine: Vec<ReadEntry> = std::iter::repeat_n(one[0].clone(), 9).collect();
    let set = ReadSet::from_reads(nine).expect("the entry encodes");
    assert_eq!(set.distinct_objects(), 1);
    assert_eq!(set.granularity(), "G3");
}

// ---------------------------------------------------------------------------
// ASM-14: the kind-dependent rule `req/350` §7-3 asked for by name
// ---------------------------------------------------------------------------

/// A `VerdictReceipt` carrying a read-set is refused before it is signed.
#[test]
fn a_verdict_receipt_may_not_claim_a_read_the_escrow_had_not_taken() {
    let key = keypair(5);
    let mut payload = verdict_payload(VerdictKind::Admit, &key, 5);
    assert_eq!(payload.receipt_kind, ReceiptKind::VerdictReceipt);
    payload.read_set = Some(ReadSet::from_reads(entries(1)).expect("the entry encodes"));

    let refused = payload
        .check_schema()
        .expect_err("ASM-14 refuses this shape");
    println!("VERDICT_READ_SET_REFUSAL={refused}");
    assert!(matches!(refused, Error::Schema { .. }));
    assert!(
        gx_witness::Receipt::issue(&payload, support::issued_at(), &key).is_err(),
        "the producer refuses it too: signing it would put a valid signature on a false claim"
    );
}

// ---------------------------------------------------------------------------
// P2: the three machines that make one scope field correct
// ---------------------------------------------------------------------------

/// 🔴 The invariant `req/440` §0-4 requires to be established before one field is enough.
///
/// Nothing here is new machinery — that is the finding. "The precondition and postcondition
/// fingerprints of any receipt that was issued are over the same scope" is already enforced three
/// times over, and this test is what keeps those three from moving without anybody noticing.
#[test]
fn one_scope_field_is_enough_because_three_machines_already_hold_the_pair_together() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    // 1. The comparison itself refuses across scopes rather than answering.
    let core = std::fs::read_to_string(root.join("crates/gx-core/src/fingerprint.rs"))
        .expect("gx-core has fingerprint.rs");
    assert!(
        core.contains("if self.scope != other.scope {")
            && core.contains("Error::FingerprintScopeMismatch"),
        "`cas_eq` no longer refuses a comparison across scopes; a single `fingerprint_scope` on \
         the receipt would then be describing two different scopes"
    );

    // 2. T-10a turns that refusal into an abort, so no scope-crossing pair reaches a receipt.
    let pipeline = std::fs::read_to_string(root.join("crates/gx-engine/src/pipeline.rs"))
        .expect("gx-engine has pipeline.rs");
    assert!(
        pipeline.contains("match fp0.cas_eq(&fp1) {")
            && pipeline
                .contains("Err(_) => return self.abort(id, AbortReason::InternalError, None, at),"),
        "T-10a no longer aborts on `cas_eq`'s refusal (M5-24 adopted (a))"
    );

    // 3. And an adapter that moved the scope across its own `apply` fails 51 §7 rather than
    //    shipping.
    let laws = std::fs::read_to_string(root.join("crates/gx-substrate-conformance/src/laws.rs"))
        .expect("the conformance harness has laws.rs");
    assert!(
        laws.contains("applied.postcondition().cas_eq(&observed)"),
        "the law that compares an adapter's own two fingerprints through `cas_eq` is gone"
    );

    println!("P2_INVARIANT_MACHINES=3 (cas_eq refusal | T-10a abort | 51 §7 law)");
}

/// 🔴 The hole P2 closes, quoted from the place it is written down.
///
/// The undo road's compare-and-set is on digests alone **because the receipt carries no scope**.
/// This does not change that road — `req/440` keeps `pipeline.rs` out of this lane — it pins the
/// sentence so the follow-up has a coordinate that is still true when it arrives.
#[test]
fn the_undo_roads_reason_for_comparing_digests_alone_is_the_missing_scope() {
    let pipeline = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../gx-engine/src/pipeline.rs"),
    )
    .expect("gx-engine has pipeline.rs");
    assert!(
        pipeline.contains("32 bytes with no substrate and no")
            && pipeline.contains("`cas_eq` insists on are simply not in the receipt"),
        "the sentence P2 exists to make false has moved; `req/441` §4's follow-up needs a new \
         coordinate"
    );
    // And the seat it needs now exists.
    let receipt = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt.rs"),
    )
    .expect("this crate has receipt.rs");
    assert!(receipt.contains("pub fingerprint_scope: String,"));
    println!("P2_SEAT=present P2_CONSUMER=pending(req/441 §4)");
}

// ---------------------------------------------------------------------------
// The scope of the claim, stated where a reader of the type will meet it
// ---------------------------------------------------------------------------

/// 🔴 **The overclaim `req/38` §236 ruling 3 forbids, refused mechanically.**
///
/// "read-set attest = selective undo becomes decidable" does not hold until (B) — the agent's read
/// traffic — is implemented, and (B) is DR-46-25 with its cost unmeasured. The type's own
/// documentation has to say so, and `docs/LIMITS.md` has to say so, because a field named
/// `read_set` invites exactly the reading the ruling forbids.
#[test]
fn the_type_and_the_limits_both_say_which_reads_this_does_not_cover() {
    let receipt = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt.rs"),
    )
    .expect("this crate has receipt.rs");
    assert!(
        receipt.contains("It is **not** the agent's read traffic"),
        "the field's own documentation has to carry the scope of the claim"
    );
    let limits = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/LIMITS.md"),
    )
    .expect("the tree has docs/LIMITS.md");
    assert!(
        limits.contains("DR-46-24(A)") && limits.contains("DR-46-25"),
        "LIMITS has to name both halves: what this covers and what it does not"
    );
    assert!(
        limits.contains("G3") && limits.contains("G4"),
        "req/350 §4-1 makes writing the two granularities' difference into LIMITS the condition of \
         the design"
    );
    println!("LIMITS_CARRIES_SCOPE=true");
}
