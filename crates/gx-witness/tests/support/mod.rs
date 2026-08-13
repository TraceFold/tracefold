//! Fixtures shared by hand 4's and hand 5's suites.
//!
//! Not a test target: cargo builds one integration binary per `.rs` file directly under `tests/`,
//! and a file in a subdirectory is only compiled as a module of one that declares it. So a helper
//! here raises no `test result:` line of its own, which is what keeps the floor in `tools/e2e.sh`
//! counting suites rather than support code. Same shape as `gx-log/tests/support/mod.rs`.
//!
//! `#![allow(dead_code)]` because each suite declares the whole module and uses part of it;
//! without it, `-D warnings` (51 §11.1 stage 2) turns 「this suite does not need `oid`」 into a
//! build failure.

#![allow(dead_code)]

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, Subject,
    SubstrateKind, Timestamp, Transformation, TransformationId,
};
use gx_witness::evidence::{Evidence, InTotoStatementRef, PolicyDecision, TestOutcome};
use gx_witness::provenance::{Environment, ProvenanceInputs};

/// A digest that is easy to tell apart from another one in a failure message. Not a hash of
/// anything -- these suites are about derivation and identity, not about what a receipt is.
pub fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// Disjoint seed ranges, so a swapped argument shows up as a wrong value rather than as a
/// coincidence that happens to compare equal.
pub fn oid(seed: u64) -> ObjectId {
    ObjectId(cid(4_000_000 + seed))
}

pub fn tid(seed: u64) -> TransformationId {
    TransformationId(cid(9_000_000 + seed))
}

pub fn iid(seed: u64) -> IntentId {
    IntentId(cid(7_000_000 + seed))
}

/// The five fields 41 §3 does not let a caller derive (`CompositionMetadata`, E-A7-2).
pub fn meta(seed: u64) -> CompositionMetadata {
    CompositionMetadata {
        intent_id: iid(seed),
        delta: DeltaRef {
            substrate: SubstrateKind::Fs,
            cid: cid(2_000_000 + seed),
        },
        context: ChangeContext::Evidence,
        actor: Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
        created_at: Timestamp(1_754_000_000_000_000_000),
    }
}

/// A submitted arrow: order 0, no parents. 42 §3.9's 「Draft/Candidate生成時」 case.
pub fn submitted(seed: u64) -> Transformation {
    Transformation::new(
        tid(seed),
        0,
        Subject::Object(oid(seed)),
        Some(cid(seed)),
        Vec::new(),
        meta(seed),
    )
    .expect("order 0 is below MAX_ORDER")
}

/// An arrow that carries parents, which is what `gx_core::compose` writes into one.
pub fn with_parents(seed: u64) -> Transformation {
    Transformation::new(
        tid(seed),
        0,
        Subject::Object(oid(seed)),
        Some(cid(seed)),
        vec![tid(seed + 1), tid(seed + 2)],
        meta(seed),
    )
    .expect("order 0 is below MAX_ORDER")
}

/// 42 §3.9's five environment fields, filled with values a reader can tell apart.
pub fn environment() -> Environment {
    Environment {
        host_id: Some("host-a".to_string()),
        adapter_kind: SubstrateKind::Git,
        correlation_id: Some("mcp-session-7".to_string()),
        engine_version: "gx-engine 0.1.0".to_string(),
        adapter_version: "gx-substrate-git 0.1.0".to_string(),
    }
}

/// The second argument of `derive_from` (E-M2-5): everything 41 §3's ten fields do not hold.
pub fn inputs(objects: Vec<ObjectId>) -> ProvenanceInputs {
    ProvenanceInputs {
        input_objects: objects,
        environment: environment(),
    }
}

/// One value of every `Evidence` variant, in 42 §3.7's order.
pub fn one_of_each_evidence() -> Vec<Evidence> {
    vec![
        Evidence::TestResult {
            case: "canonical_round_trip".to_string(),
            suite: "gx-canon/ac_009".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: Some(cid(11)),
            duration_ms: 42,
        },
        Evidence::Measurement {
            subject: Subject::Object(oid(3)),
            measure_id: "lyapunov/entropy".to_string(),
            value_digest: cid(12),
        },
        Evidence::ExternalAttestation {
            signer: "key-external-1".to_string(),
            statement: InTotoStatementRef {
                uri: Some("https://example.invalid/att/1".to_string()),
                digest: cid(13),
                inline: None,
            },
            predicate_type: "https://slsa.dev/provenance/v1".to_string(),
        },
        Evidence::PolicyEvaluation {
            decision: PolicyDecision::Allow,
            policy_id: "cedar/allow-fs-write".to_string(),
            explanation_digest: Some(cid(14)),
        },
    ]
}

// ---------------------------------------------------------------------------
// Hand 5: keys, receipts, and a ledger to anchor one against
// ---------------------------------------------------------------------------

use gx_core::{Checkpoint, FingerprintBytes, InclusionProof, VerdictKind};
use gx_witness::receipt::{Receipt, ReceiptKind, ReceiptPayload, VerdictSummary};
use gx_witness::KeyPair;

/// A key pair from a fixed seed, so a failure is reproducible and no test depends on entropy.
///
/// `KeyPair::from_seed` is infallible for every 32-byte value (RFC 8032 §5.1.5 clamps the scalar),
/// which is why these suites can seed from a counter without checking anything.
pub fn keypair(seed: u8) -> KeyPair {
    KeyPair::from_seed(format!("key-{seed}"), &[seed; 32])
}

/// Thirty-two opaque bytes standing in for M4's `Fingerprint` (E-M2-2).
pub fn fingerprint(seed: u8) -> FingerprintBytes {
    FingerprintBytes([seed; 32])
}

/// A `VerdictReceipt` payload for one of the three verdicts (42 §3.10, 43 T-4a/b/c).
///
/// `enforced` is `false` and `fail_posture_engaged` is `true`, which is what 35 ASM-13 and 43 T-4e
/// require of a verdict-stage receipt and what 42 §3.10's 「`true`固定」 would have forbidden --
/// see `receipt.rs`'s module note on why the schema check stays out of it.
pub fn verdict_payload(kind: VerdictKind, key: &KeyPair, seed: u64) -> ReceiptPayload {
    let t = tid(seed);
    ReceiptPayload {
        key_id: key.key_id().clone(),
        // `Some` since **E-M5-11** (§41): the seat is optional and this fixture is the case where
        // something decided. The one without a verdict is `degraded_payload` below.
        verdict: Some(VerdictSummary {
            kind,
            proof_digest: cid(500_000 + seed),
        }),
        enforced: false,
        receipt_kind: ReceiptKind::VerdictReceipt,
        canonical_cid: t.0,
        inverse_delta: None,
        transformation: t,
        inclusion_proof: None,
        fail_posture_engaged: true,
        precondition_fingerprint: fingerprint(7),
        postcondition_fingerprint: None,
    }
}

/// 🔴 **E-M5-11**: a payload for 43 T-4e's degraded admission — no verdict, posture engaged.
///
/// The shape the `Option` exists for. `enforced` is `false` and `fail_posture_engaged` is `true`
/// because 43 T-4e requires both 「必ずreceiptに刻む」, and `verdict` is `None` because the gate was
/// never asked. Used by `tests/receipt_verdict_wire.rs` to show that the absence writes a `null`
/// and changes nothing else about the encoding.
pub fn degraded_payload(key: &KeyPair, seed: u64) -> ReceiptPayload {
    ReceiptPayload {
        verdict: None,
        ..verdict_payload(VerdictKind::Admit, key, seed)
    }
}

/// A `CommitReceipt` payload: everything the verdict one carries, plus what 43 T-11 adds.
pub fn commit_payload(key: &KeyPair, seed: u64, proof: InclusionProof) -> ReceiptPayload {
    ReceiptPayload {
        inclusion_proof: Some(proof),
        inverse_delta: Some(cid(600_000 + seed)),
        postcondition_fingerprint: Some(fingerprint(8)),
        enforced: true,
        fail_posture_engaged: false,
        receipt_kind: ReceiptKind::CommitReceipt,
        ..verdict_payload(VerdictKind::Admit, key, seed)
    }
}

/// The clock a receipt records and no signature covers (E-M2-6).
pub fn issued_at() -> Timestamp {
    Timestamp(1_754_600_000_000_000_000)
}

/// Sign a payload. Panics on a schema violation, which every caller here intends to avoid.
pub fn issue(payload: &ReceiptPayload, key: &KeyPair) -> Receipt {
    Receipt::issue(payload, issued_at(), key).expect("the fixture is a legal receipt")
}

/// A `CommitReceipt` whose inclusion proof really was produced by a ledger, with the checkpoint
/// that ledger would publish.
///
/// # The commit protocol of 43 T-11, in five lines
///
/// 「`ledger.append(...)` → `InclusionProof`；Receipt発行」: the payload is built first *without*
/// its proof, appended under [`ReceiptPayload::ledger_digest`], and only then completed and signed.
/// The digest a verifier recomputes is the one taken here, which is the whole reason
/// `ledger_digest` excludes the proof -- see its documentation for why 42 §3.11's literal reading
/// has no value at this point in the protocol.
///
/// `others` is how many unrelated entries the log holds beside this one, so the audit path is not
/// trivially empty.
pub fn commit_receipt_in_a_log(key: &KeyPair, seed: u64, others: u64) -> (Receipt, Checkpoint) {
    use gx_log::{proof, TileLog};

    let mut log = TileLog::new();
    for i in 0..others {
        log.append(tid(900_000 + i), cid(910_000 + i), Timestamp(i as i64))
            .expect("canonical");
    }

    let staged = commit_payload(key, seed, empty_proof());
    let index = log.len();
    log.append(
        tid(seed),
        staged.ledger_digest().expect("canonical"),
        Timestamp(1),
    )
    .expect("canonical");

    let inclusion = proof::prove_inclusion(&log, index).expect("the entry is in the log");
    let receipt = issue(&commit_payload(key, seed, inclusion), key);
    let head = proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(2))
        .expect("a non-empty log has a head");
    (receipt, head)
}

/// The placeholder a payload carries while its own digest is being taken.
///
/// It never reaches a signature: `ledger_digest` clears the field before hashing, so any value here
/// is invisible to the leaf. Written as an obviously impossible proof (a tree of zero leaves) so
/// that one escaping into a signed receipt would be refused by `verify_inclusion_of` rather than
/// mistaken for a real claim.
fn empty_proof() -> InclusionProof {
    InclusionProof {
        leaf_index: 0,
        tree_size: 0,
        audit_path: Vec::new(),
    }
}
