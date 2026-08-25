// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-017 (FR-017) — `Proof` holds any one of F0's five theorem identifiers, and survives serde.
//! (sem: SEM-gx-witness-161, SEM-gx-witness-162, SEM-gx-witness-163, SEM-gx-witness-164,
//! SEM-gx-witness-165, SEM-gx-witness-166, SEM-gx-witness-167)
//!
//! AC-017 verbatim: "Given: set one of T1-T5's identifiers on the `Proof` type. When: a serde round
//! trip. Then: all five theorem ids can be individually represented and restored." Judgement
//! method: `unit`, M2.
//!
//! FR-017 verbatim: "gx-witness MUST implement `Proof` (a reference structure to a Lean theorem
//! reference or a verifier's verification result). It must be testable that it is an enum or id
//! type able to hold one of F0's T1-T5 identifiers."
//!
//! # Where the type lives, and why the subject of this file is `gx_witness::Proof`
//!
//! **E-M2-12** (`req/38_ERRATA_2026-08-07.md` §8): "the `Proof` family of types goes to gx-core, same as E-M2-1".
//! req/49 §3 M2-3 raised the collision — FR-017 asks gx-witness for a `Proof` while 42 §0 files the
//! nearly identical `ProofRef` under gx-gate/verdict.rs (M3) — and the ruling put one set of types
//! below both, so the two crates refer to one idea rather than to two spellings that can drift.
//!
//! FR-017's MUST is then satisfied by gx-witness **publishing** the type, not by defining a second
//! one: `gx_witness::Proof` is `gx_core::Proof`, which this file asserts rather than assumes
//! (`ac_017_the_two_paths_name_one_type`). A re-export is the only reading of "gx-witness MUST
//! implement" that does not contradict E-M2-12, and it is what makes AC-017's subject —
//! `Proof` reached through gx-witness — exist at all. req/50 §4's `m2_types.rs` said this hand
//! would take that shape; this is it.
//!
//! # Two round trips, not one
//!
//! AC-017 says "a serde round trip" without naming a format. JSON is the human-readable face (42 §1.2), and
//! canonical DAG-CBOR is the face an identity is computed over (42 §2.1) — a type that survives one
//! and not the other survives the wrong half. Both are here, and the CBOR side goes through
//! gx-canon rather than through a codec named in this crate (41 §6).
//!
//! # What a `TheoremId` is not
//!
//! `req/38_ERRATA_2026-08-07.md` §7: T2, T4 and T5 are **unproven** (`Invariant.lean` and
//! `Receipt.lean` do not exist, and Lean's `T2_hoare_seq` is a different model from 46 §2.3's Hoare
//! rule). Setting `TheoremId::T4` on a value records that something *claims* T4's shape. req/49 §1
//! N-10 marks reading it as a guarantee the overclaim 45 §4.1 forbids.

use gx_canon::cbor;
use gx_core::Cid;
use gx_witness::{CheckerResultRef, Proof, ProofRef, TheoremId};

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

// ---------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------

/// AC-017 verbatim, over JSON: every one of the five set on a `Proof`, round-tripped, recovered.
#[test]
fn ac_017_every_f0_theorem_survives_a_json_round_trip_on_a_proof() {
    assert_eq!(TheoremId::ALL.len(), 5, "46 §2.5 fixes F0 at five theorems");

    for id in TheoremId::ALL {
        let p = Proof::Lean(ProofRef {
            lean_spec_version: "0.1.0".to_string(),
            theorem_ids: vec![id],
        });
        let text = serde_json::to_string(&p).expect("a proof serialises");
        let back: Proof = serde_json::from_str(&text).expect("and reads back");
        match back {
            Proof::Lean(r) => assert_eq!(
                r.theorem_ids,
                vec![id],
                "{id:?} came back as something else"
            ),
            Proof::Checked(_) => panic!("a Lean proof came back as a checker result"),
        }
    }
}

/// The same five over canonical DAG-CBOR, through gx-canon (41 §6: this crate names no codec).
#[test]
fn ac_017_every_f0_theorem_survives_a_canonical_dagcbor_round_trip() {
    for id in TheoremId::ALL {
        let p = Proof::Lean(ProofRef {
            lean_spec_version: "0.1.0".to_string(),
            theorem_ids: vec![id],
        });
        let bytes = cbor::encode(&p).expect("a proof has a canonical form");
        assert!(
            cbor::is_canonical(&bytes),
            "{id:?} encoded to bytes the encoder would not have written"
        );
        let back: Proof = cbor::decode(&bytes).expect("strict decode");
        assert_eq!(back, p, "{id:?} did not survive the binary face");
    }
}

/// "individually" is the load-bearing word: five ids that round-trip but share a spelling are one id
/// wearing five names, and every one of them would pass the two tests above.
#[test]
fn ac_017_the_five_identifiers_are_individually_distinguishable() {
    let mut json: Vec<String> = TheoremId::ALL
        .iter()
        .map(|id| serde_json::to_string(id).expect("a theorem id serialises"))
        .collect();
    json.sort();
    json.dedup();
    assert_eq!(json.len(), 5, "two theorem ids share a JSON spelling");

    let mut binary: Vec<Vec<u8>> = TheoremId::ALL
        .iter()
        .map(|id| cbor::encode(id).expect("a theorem id has a canonical form"))
        .collect();
    binary.sort();
    binary.dedup();
    assert_eq!(binary.len(), 5, "two theorem ids share a canonical form");
}

/// FR-017's second form: "a reference structure to a verifier's verification result". A `Proof` that is a checker result has to
/// round-trip too, or half the requirement is untested.
#[test]
fn ac_017_a_checker_result_reference_is_the_other_form_of_a_proof() {
    for id in TheoremId::ALL {
        let p = Proof::Checked(CheckerResultRef {
            checker_id: "gx-gate/invariant".to_string(),
            theorem_ids: vec![id],
            result_digest: cid(5),
        });
        let bytes = cbor::encode(&p).expect("canonical");
        assert_eq!(cbor::decode::<Proof>(&bytes).expect("strict"), p);
        let text = serde_json::to_string(&p).expect("json");
        assert_eq!(serde_json::from_str::<Proof>(&text).expect("json back"), p);
    }
}

/// A proof citing nothing is the honest value today (`req/38_ERRATA_2026-08-07.md` §7: T4 and T5
/// are unproven and `Receipt.lean` does not exist), so the type has to admit it. A type that
/// required a citation would make every receipt this workspace can issue a false one.
#[test]
fn ac_017_a_proof_may_cite_no_theorem_at_all() {
    let uncited = Proof::Lean(ProofRef {
        lean_spec_version: "0.1.0".to_string(),
        theorem_ids: Vec::new(),
    });
    let bytes = cbor::encode(&uncited).expect("canonical");
    assert_eq!(cbor::decode::<Proof>(&bytes).expect("strict"), uncited);
}

/// F0 has five theorems, so `"T6"` names nothing. 42 §3.8 types `theorem_ids` as `Vec<String>`,
/// where it would have been accepted; **E-M2-18** (`req/38_ERRATA_2026-08-07.md` §9) rules the typed
/// enum, with the wire form unchanged for every valid value. Asserted here as well as in gx-core's
/// `m2_types.rs`, because AC-017's subject is the type as gx-witness publishes it.
#[test]
fn ac_017_an_identifier_f0_does_not_have_is_refused() {
    assert!(serde_json::from_str::<TheoremId>("\"T1\"").is_ok());
    for bad in ["\"T0\"", "\"T6\"", "\"t1\"", "\"\""] {
        assert!(
            serde_json::from_str::<TheoremId>(bad).is_err(),
            "{bad} was accepted as an F0 theorem"
        );
    }
}

// ---------------------------------------------------------------------------
// E-M2-12 — one type, reached by two paths
// ---------------------------------------------------------------------------

/// The re-export is an identity, not a copy. If a second `Proof` were ever defined in gx-witness,
/// these assignments stop compiling — which is the same kind of check as gx-log's absent cycle:
/// the compiler holds it, not a reviewer.
#[test]
fn ac_017_the_two_paths_name_one_type() {
    let from_core: gx_core::Proof = gx_core::Proof::Lean(gx_core::ProofRef {
        lean_spec_version: "0.1.0".to_string(),
        theorem_ids: vec![gx_core::TheoremId::T3],
    });
    let from_witness: Proof = from_core.clone();
    assert_eq!(from_witness, from_core);

    let id_core: gx_core::TheoremId = gx_core::TheoremId::T5;
    let id_witness: TheoremId = id_core;
    assert_eq!(id_witness, id_core);

    let ref_core: gx_core::CheckerResultRef = gx_core::CheckerResultRef {
        checker_id: "c".to_string(),
        theorem_ids: Vec::new(),
        result_digest: cid(1),
    };
    let ref_witness: CheckerResultRef = ref_core.clone();
    assert_eq!(ref_witness, ref_core);
}
