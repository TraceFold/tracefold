// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The M2 types that the rulings of `req/38_ERRATA_2026-08-07.md` §8 place in gx-core.
//!
//! No acceptance criterion is claimed here. AC-015..AC-024, AC-069 and AC-070 are M2's set and
//! every one of them needs gx-witness or gx-log to exist first (hands 2..5); what hand 1 owes is
//! the type layer those hands build on, and this file is the red that comes before it.
//!
//! Why the types sit in *this* crate rather than where 42 §0 files them:
//!
//! * **E-M2-1** — 42 §3.10 asks `ReceiptPayload` (gx-witness) to carry an `InclusionProof`
//!   (gx-log), and 42 §3.11 asks `Checkpoint` (gx-log) to carry a `DsseSignature` (gx-witness).
//!   Written that way the two crates name each other and cargo refuses the workspace. The ruling
//!   applies A-1's shape -- "types down, computation up" (sem: SEM-gx-core-162) -- so the *data*
//!   moves down here and the
//!   merkle arithmetic (gx-log) and the signing (gx-witness) stay up there. The cycle is then not
//!   forbidden by a rule anybody has to remember; it is absent from the graph.
//! * **E-M2-2** — `ReceiptPayload.precondition_fingerprint` is typed `Fingerprint`, which 42 §0
//!   files under gx-substrate (M4). M2 carries [`FingerprintBytes`], 32 opaque bytes it never
//!   interprets, the way A-2 deferred the Kani fingerprint row.
//! * **E-M2-12** — the `Proof` family lands here too, so gx-witness (FR-017) and gx-gate's
//!   `ProofRef` (42 §3.8, M3) refer to one set of types instead of two spellings of one idea.
//!
//! What is deliberately NOT here: `ConsistencyProof`. Nothing below gx-log needs it -- it is
//! 42 §3.11 data like the other two, but no gx-witness type carries one, so moving it down would
//! be a move made for symmetry rather than to break a cycle. It stays in gx-log with the
//! `verify_consistency` that takes it (E-M2-8), and hand 2 owns it.

use gx_core::{
    Checkpoint, Cid, DsseSignature, FingerprintBytes, InclusionProof, Proof, ProofRef, TheoremId,
    Timestamp, VerdictCheckpoint, VerdictTally,
};

/// A digest that is easy to tell apart from another one by eye in a failure message.
fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

/// serde_json is the only encoder gx-core may test through: this crate is forbidden to know
/// gx-canon exists (A-1), and DAG-CBOR lives there. So what is checked here is that the values
/// survive a round trip and that the *shape* is the one 42 fixes. The canonical-bytes question --
/// that a signature reaches the wire as a CBOR byte string rather than a 64-element list -- is
/// gx-canon's to answer and is checked in hand 5, where the receipt payload gets encoded for real.
fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let text = serde_json::to_string(value).expect("the value serializes");
    serde_json::from_str(&text).expect("and reads back")
}

// ---------------------------------------------------------------------------
// 42 §3.11 -- the two ledger types the receipt refers to
// ---------------------------------------------------------------------------

/// 42 §3.11, verbatim: `InclusionProof` is `leaf_index: u64`, `tree_size: u64`, `audit_path:
/// Vec<Cid>` (sem: SEM-gx-core-163).
#[test]
fn inclusion_proof_carries_the_three_fields_42_3_11_names() {
    let p = InclusionProof {
        leaf_index: 42,
        tree_size: 100,
        audit_path: vec![cid(1), cid(2), cid(3)],
    };
    let back = round_trip(&p);
    assert_eq!(back.leaf_index, 42);
    assert_eq!(back.tree_size, 100);
    assert_eq!(back.audit_path, p.audit_path);
    assert_eq!(back, p);
}

/// An empty audit path is the one-leaf tree, not a malformed proof: leaf 0 of a tree of size 1 has
/// no sibling. The type must therefore admit it -- refusing it here would push a policy decision
/// into a data type, and whether such a proof *verifies* is gx-log's question (hand 2).
#[test]
fn inclusion_proof_admits_the_single_leaf_tree() {
    let p = InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    };
    assert_eq!(round_trip(&p), p);
}

/// 42 §3.11, verbatim: `Checkpoint` (signed tree head) is `origin: String`, `tree_size: u64`,
/// `root_hash: Cid`, `timestamp: Timestamp`, `signature: DsseSignature` (sem: SEM-gx-core-164).
///
/// `timestamp` is kept as 42 writes it. E-M2-6 took `issued_at` out of the *receipt's* signed
/// core (CM-5, clock-free signed payload), and 43 T-11 is the list that ruling read; neither says
/// anything about a checkpoint, whose timestamp is a property of the tree head rather than of the
/// receipt. Changing it here would be a second erratum nobody ruled on -- so the field stays and
/// the question is raised in the hand-1 report instead.
#[test]
fn checkpoint_carries_the_five_fields_42_3_11_names() {
    let c = Checkpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tree_size: 100,
        root_hash: cid(7),
        timestamp: Timestamp(1_754_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "key-1".to_string(),
            sig: vec![0xab; 64],
        },
    };
    let back = round_trip(&c);
    assert_eq!(back.origin, "glovrex-ledger/v1");
    assert_eq!(back.tree_size, 100);
    assert_eq!(back.root_hash, cid(7));
    assert_eq!(back.timestamp, Timestamp(1_754_000_000_000_000_000));
    assert_eq!(back.signature.keyid, "key-1");
    assert_eq!(back.signature.sig.len(), 64);
    assert_eq!(back, c);
}

/// The edge that used to close the cycle. `Checkpoint.signature` is `gx_core::DsseSignature`, so
/// a value can be built without gx-witness on the path -- which is the whole of E-M2-1 stated as
/// something a compiler checks rather than something a reviewer remembers.
#[test]
fn a_checkpoint_can_be_built_without_naming_gx_witness() {
    let sig: DsseSignature = DsseSignature {
        keyid: "ledger".to_string(),
        sig: vec![1, 2, 3],
    };
    let c = Checkpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tree_size: 1,
        root_hash: cid(0),
        timestamp: Timestamp(0),
        signature: sig,
    };
    assert_eq!(c.signature.sig, vec![1, 2, 3]);
}

/// Ed25519 signatures are 64 bytes, but the type is not the place to say so: the field is
/// `Vec<u8>` in 42 §3.10, an empty signature is what a malformed envelope carries, and rejecting
/// it belongs to the verifier (AC-019, hand 5) rather than to the container. A round trip that
/// quietly dropped the bytes would hide exactly that case.
#[test]
fn dsse_signature_round_trips_raw_bytes_of_any_length() {
    for len in [0usize, 1, 63, 64, 65] {
        let s = DsseSignature {
            keyid: "k".to_string(),
            sig: vec![0x5a; len],
        };
        let back = round_trip(&s);
        assert_eq!(back.sig.len(), len, "length {len} did not survive");
        assert_eq!(back, s);
    }
}

/// 🔴 **NFR-011 close, C2 (JSON face)** — `DsseSignature` serialises to **exactly two fields**,
/// `keyid` and `sig`, and neither of them is an `alg`.
///
/// `req/38` §109 (DR-46-5, option (b); sem: SEM-gx-core-165) rules that the signing algorithm is a
/// property of the *verifier's pinned key* — the key material's own type,
/// `ed25519_dalek::VerifyingKey` — and never a wire field: DSSE's maintainers refused an in-band
/// `alg` as a feature with "a history of security vulnerabilities" and answered "the recommended
/// solution is to make this a property of the public key" (secure-systems-lab/dsse issue #35), and
/// RFC 8725 §3.1 gives the JWT lesson as a norm — "each key MUST be used with exactly one
/// algorithm" (sem: SEM-gx-core-166). So the wire form `{keyid, sig}` is permanent, and using any
/// wire-carried algorithm name for crypto dispatch is permanently forbidden (33 NFR-011 closing
/// note; sem: SEM-gx-core-167).
///
/// What this test is **not**: a check on readers. DSSE's own parsing rule is "Consumers MUST
/// ignore unrecognized fields" (envelope.md; sem: SEM-gx-core-168), so a reader's tolerance of
/// unknown fields is
/// required by the standard and deliberately out of scope here. This test is a gate on **our own
/// writer**: gx must never begin emitting a third field — an `alg` least of all — without a
/// ruling that turns this line RED first. The DAG-CBOR face of the same claim is
/// `gx-canon/tests/golden_vectors.rs` (gx-core may not name that encoder — A-1).
#[test]
fn dsse_signature_serialises_exactly_two_fields_and_neither_is_alg() {
    let signature = DsseSignature {
        keyid: "key-1".to_string(),
        sig: vec![0xde, 0xad, 0xbe, 0xef],
    };
    let value = serde_json::to_value(&signature).expect("a signature serialises");
    let object = value.as_object().expect("a signature is a JSON object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    println!("DSSE_SIGNATURE_JSON_KEYS={keys:?}");
    assert_eq!(
        keys,
        ["keyid", "sig"],
        "42 §3.10 / req/38 §109: the wire form is {{keyid, sig}} and nothing else"
    );
}

/// 🔴 **Field census, `Checkpoint`** (v0.4-e; the residue row carried over from `req/38` §110:
/// "the field census of `DsseEnvelope`/`Checkpoint` as a whole is not pinned (the denominator of
/// self-adversarial item 1)"; quoted in SEM-gx-core-169) — a signed tree head serialises to
/// **exactly the five keys** 42 §3.11 names, and the signature riding inside it stays
/// `{keyid, sig}`.
///
/// # Why the census exists at the *carrier*, not only at the signature
///
/// 33 NFR-011's note 5 (the revising note plus close, `req/38` §109/§110) permanently forbids a
/// wire-side alg-like field used for crypto dispatch — "using an alg-like field on the wire for
/// crypto dispatch is permanently forbidden" (quoted in SEM-gx-core-170). The C2 gates fixed
/// `DsseSignature` alone; but an `alg` smuggled in *beside*
/// the signature — on the structure that carries it — would slip past those gates while doing
/// the exact thing the prohibition is about. Fixing the carrier's whole key set is the
/// constructive form of the prohibition at this face: any silent addition, alg-like least of
/// all, turns the key set and goes RED in **our own writer** before a reader ever sees it.
///
/// # The declared limit (same as C2's, and it is DSSE's own rule)
///
/// Readers are not policed and cannot be: serde's derive ignores unknown fields on decode, and
/// DSSE's norm says so too (envelope.md "Consumers MUST ignore unrecognized fields"; sem:
/// SEM-gx-core-171). A census
/// on the serialize face therefore protects only what gx **emits** — a peer that mails us extra
/// fields is out of this test's reach, and out of its scope. The DAG-CBOR face has no golden
/// vector for this struct (gx-canon's suite covers Cid/ObjectSnapshot/Transformation/DeltaRef
/// plus the `DsseSignature` entry-count pin), so the JSON face is what is pinned here; the two
/// faces share one derive, so a field added to the struct moves both.
#[test]
fn checkpoint_serialises_exactly_the_five_keys_and_carries_no_alg_beside_its_signature() {
    let c = Checkpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tree_size: 100,
        root_hash: cid(7),
        timestamp: Timestamp(1_754_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "key-1".to_string(),
            sig: vec![0xab; 64],
        },
    };
    let value = serde_json::to_value(&c).expect("a checkpoint serialises");
    let object = value.as_object().expect("a checkpoint is a JSON object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    println!("CHECKPOINT_JSON_KEYS={keys:?}");
    assert_eq!(
        keys,
        ["origin", "root_hash", "signature", "timestamp", "tree_size"],
        "42 §3.11: a signed tree head is these five fields and nothing else"
    );
    let signature = object["signature"]
        .as_object()
        .expect("the signature is an object");
    let mut signature_keys: Vec<&str> = signature.keys().map(String::as_str).collect();
    signature_keys.sort_unstable();
    assert_eq!(
        signature_keys,
        ["keyid", "sig"],
        "33 NFR-011 note 5 (sem: SEM-gx-core-172): the signature stays {{keyid, sig}} where it \
         travels, not only in isolation"
    );
}

/// 🔴 **Field census, `VerdictCheckpoint`** (v0.4-e; the same residue row (sem: SEM-gx-core-173)
/// as `Checkpoint`'s census
/// above) — a signed verdict count serialises to **exactly the eight keys** FR-M04 gave it, its
/// tally to exactly the four buckets, and its signature to `{keyid, sig}`.
///
/// This is the structure where the census earns its keep most directly: the signer is the party
/// the count is evidence *against* (this type's own doc), so its wire face is the one a
/// deployment has the most interest in quietly growing. The prohibition being supported and the
/// declared reader-side limit are the `Checkpoint` census's, one test up.
///
/// The second serialisation pins a JSON-face fact worth a line of its own: an **absent value is
/// not an absent key**. `ledger_root_hash: None` — the all-refusals window FR-M04 exists for —
/// writes `null` under its own name, so the key set is stable across the interesting case and a
/// consumer keying on shape cannot tell the two windows apart by key census alone.
#[test]
fn verdict_checkpoint_serialises_exactly_the_eight_keys_fr_m04_names() {
    let vc = VerdictCheckpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tally: VerdictTally {
            deny: 3,
            admit: 5,
            escalate: 1,
            unverdicted: 1,
        },
        window_start: 0,
        window_end: 10,
        ledger_root_hash: Some(cid(9)),
        ledger_tree_size: 5,
        timestamp: Timestamp(1_754_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "key-1".to_string(),
            sig: vec![0xab; 64],
        },
    };
    let value = serde_json::to_value(&vc).expect("a verdict checkpoint serialises");
    let object = value
        .as_object()
        .expect("a verdict checkpoint is a JSON object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    println!("VERDICT_CHECKPOINT_JSON_KEYS={keys:?}");
    assert_eq!(
        keys,
        [
            "ledger_root_hash",
            "ledger_tree_size",
            "origin",
            "signature",
            "tally",
            "timestamp",
            "window_end",
            "window_start"
        ],
        "FR-M04: the signed count is these eight fields and nothing else"
    );
    let mut tally_keys: Vec<&str> = object["tally"]
        .as_object()
        .expect("the tally is an object")
        .keys()
        .map(String::as_str)
        .collect();
    tally_keys.sort_unstable();
    assert_eq!(
        tally_keys,
        ["admit", "deny", "escalate", "unverdicted"],
        "the four buckets, 43 T-4e's included — a fifth would be a count nobody ruled"
    );
    let mut signature_keys: Vec<&str> = object["signature"]
        .as_object()
        .expect("the signature is an object")
        .keys()
        .map(String::as_str)
        .collect();
    signature_keys.sort_unstable();
    assert_eq!(signature_keys, ["keyid", "sig"]);

    // The empty-ledger window: `None` keeps its key and writes `null`.
    let empty_window = VerdictCheckpoint {
        ledger_root_hash: None,
        ledger_tree_size: 0,
        ..vc
    };
    let value = serde_json::to_value(&empty_window).expect("serialises");
    let object = value.as_object().expect("an object");
    assert!(
        object.contains_key("ledger_root_hash") && object["ledger_root_hash"].is_null(),
        "an all-refusals window keeps the key and writes null — the key set does not move"
    );
}

// ---------------------------------------------------------------------------
// E-M2-2 -- the fingerprint carrier
// ---------------------------------------------------------------------------

/// 32 bytes and no meaning. 42 §3.5's `Fingerprint` is a three-field struct
/// (`substrate`/`scope`/`digest`) computed by an adapter; this is the opaque carrier M2 moves
/// through a receipt without ever computing or comparing one, per E-M2-2.
#[test]
fn fingerprint_bytes_is_an_opaque_thirty_two_byte_carrier() {
    let f = FingerprintBytes([9u8; 32]);
    assert_eq!(f.0.len(), 32);
    assert_eq!(round_trip(&f), f);
}

/// Equality is byte equality, which is *not* 42 §3.5's equivalence: that one first requires
/// `substrate` and `scope` to agree and calls a mismatch an adapter bug. Byte equality is the
/// necessary half of it, and the half M4 will build the CAS check on (CON-2). Saying so with a
/// test keeps the difference from being read as "the CAS check is done".
#[test]
fn fingerprint_bytes_discriminates_on_bytes_alone() {
    let a = FingerprintBytes([0u8; 32]);
    let mut raw = [0u8; 32];
    raw[31] = 1;
    let b = FingerprintBytes(raw);
    assert_ne!(a, b, "a one-bit difference must not compare equal");
    assert_eq!(a, FingerprintBytes([0u8; 32]));
}

/// Same reason `Cid` is opaque (42 §1.2 puts the readable form on the display layer): a fingerprint
/// has no spelling in the canon, so no `{:?}` in a log line may invent one.
#[test]
fn fingerprint_bytes_does_not_print_its_bytes() {
    let shown = format!("{:?}", FingerprintBytes([0xab; 32]));
    assert!(
        !shown.contains("ab") && !shown.contains("171"),
        "the debug form leaked the digest: {shown}"
    );
}

// ---------------------------------------------------------------------------
// E-M2-12 / FR-017 -- the proof family
// ---------------------------------------------------------------------------

/// 46 §2.5 fixes the count: "fixed at five (T1 composition preservation, T2 invariant composition,
/// T3 canonicalisation idempotence + representation independence, T4 receipt soundness, T5 witness
/// lax composition)" (quoted in SEM-gx-core-174). FR-017 asks for a type that can hold any one of
/// them.
#[test]
fn f0_has_exactly_five_theorems_and_each_survives_a_round_trip() {
    assert_eq!(TheoremId::ALL.len(), 5);
    for id in TheoremId::ALL {
        assert_eq!(round_trip(&id), id, "{id:?} did not come back as itself");
    }
    // Individually representable means individually *distinguishable*: five ids that all
    // serialize to the same token would round-trip and still be one id wearing five names.
    let mut spellings: Vec<String> = TheoremId::ALL
        .iter()
        .map(|id| serde_json::to_string(id).expect("a theorem id serializes"))
        .collect();
    spellings.sort();
    spellings.dedup();
    assert_eq!(spellings.len(), 5, "two theorem ids share a spelling");
}

/// F0 has five theorems, so `"T6"` names nothing. 42 §3.8 types `theorem_ids` as `Vec<String>`,
/// where it would have been accepted; the typed enum is the E-FR055-1 move -- "a flag can diverge
/// from the implementation" (sem: SEM-gx-core-175) -- applied to an identifier. The wire form is
/// unchanged for every valid value.
#[test]
fn a_theorem_f0_does_not_have_is_refused() {
    assert!(serde_json::from_str::<TheoremId>("\"T6\"").is_err());
    assert!(serde_json::from_str::<TheoremId>("\"T0\"").is_err());
    assert!(serde_json::from_str::<TheoremId>("\"t1\"").is_err());
    assert!(serde_json::from_str::<TheoremId>("\"T1\"").is_ok());
}

/// 42 §3.8, verbatim: `ProofRef { lean_spec_version: String, theorem_ids: Vec<String> }` (sem:
/// SEM-gx-core-176).
#[test]
fn proof_ref_is_42_3_8_with_its_theorem_ids_typed() {
    let r = ProofRef {
        lean_spec_version: "0.1.0".to_string(),
        theorem_ids: vec![TheoremId::T1, TheoremId::T3],
    };
    let back = round_trip(&r);
    assert_eq!(back.lean_spec_version, "0.1.0");
    assert_eq!(back.theorem_ids, vec![TheoremId::T1, TheoremId::T3]);
    assert_eq!(back, r);
}

/// FR-017, verbatim: "`Proof` (a reference structure to a Lean theorem or to a checker's
/// verification result)" (quoted in SEM-gx-core-177) -- two forms, so an
/// enum of two variants. A proof that cites no theorem is admitted by the type: 46 §2.5 records
/// T2/T4/T5 as unproven (`Invariant.lean` and `Receipt.lean` do not exist -- req/38 §7's red flag),
/// so a receipt issued today cites an empty list, and a type that refused one would force the
/// implementation to lie about which theorems back it.
#[test]
fn a_proof_is_a_lean_reference_or_a_checker_result_reference() {
    let lean = Proof::Lean(ProofRef {
        lean_spec_version: "0.1.0".to_string(),
        theorem_ids: vec![TheoremId::T4],
    });
    assert_eq!(round_trip(&lean), lean);

    let checked = Proof::Checked(gx_core::CheckerResultRef {
        checker_id: "gx-gate/invariant".to_string(),
        theorem_ids: vec![TheoremId::T2],
        result_digest: cid(5),
    });
    assert_eq!(round_trip(&checked), checked);

    let uncited = Proof::Lean(ProofRef {
        lean_spec_version: "0.1.0".to_string(),
        theorem_ids: Vec::new(),
    });
    assert_eq!(round_trip(&uncited), uncited);
}

/// Every one of the five, set on a `Proof` and read back -- the shape AC-017 will take in hand 4
/// once gx-witness re-exports the type. Stated here as a property of the type, not as the AC:
/// AC-017's subject is `gx_witness::Proof`, which does not exist yet.
#[test]
fn each_f0_theorem_can_be_set_on_a_proof_and_recovered() {
    for id in TheoremId::ALL {
        let p = Proof::Lean(ProofRef {
            lean_spec_version: "0.1.0".to_string(),
            theorem_ids: vec![id],
        });
        match round_trip(&p) {
            Proof::Lean(r) => assert_eq!(r.theorem_ids, vec![id]),
            Proof::Checked(_) => panic!("a Lean proof came back as a checker result"),
        }
    }
}
