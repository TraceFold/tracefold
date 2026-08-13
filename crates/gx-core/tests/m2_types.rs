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
//!   applies A-1's shape -- 「型は下層・計算は上層」 -- so the *data* moves down here and the
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
    Timestamp,
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

/// 42 §3.11 逐語: `InclusionProof` は `leaf_index: u64`・`tree_size: u64`・`audit_path: Vec<Cid>`。
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

/// 42 §3.11 逐語: `Checkpoint`(signed tree head) は `origin: String`・`tree_size: u64`・
/// `root_hash: Cid`・`timestamp: Timestamp`・`signature: DsseSignature`。
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

/// 46 §2.5 fixes the count: 「5本固定（T1 合成保存, T2 不変条件合成, T3 正準化冪等+表現非依存,
/// T4 receipt健全性, T5 witness lax合成）」. FR-017 asks for a type that can hold any one of them.
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
/// where it would have been accepted; the typed enum is the E-FR055-1 move -- 「flag が実装と
/// 乖離できる」 -- applied to an identifier. The wire form is unchanged for every valid value.
#[test]
fn a_theorem_f0_does_not_have_is_refused() {
    assert!(serde_json::from_str::<TheoremId>("\"T6\"").is_err());
    assert!(serde_json::from_str::<TheoremId>("\"T0\"").is_err());
    assert!(serde_json::from_str::<TheoremId>("\"t1\"").is_err());
    assert!(serde_json::from_str::<TheoremId>("\"T1\"").is_ok());
}

/// 42 §3.8 逐語: `ProofRef { lean_spec_version: String, theorem_ids: Vec<String> }`.
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

/// FR-017 逐語: 「`Proof`（Lean定理参照または検証器の検証結果への参照構造）」 -- two forms, so an
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
