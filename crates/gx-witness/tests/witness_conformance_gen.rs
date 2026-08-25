// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! M8-c conformance vector generator — `receipt_verify` (T4 receipt soundness + T5 witness
//! recovery). (sem: SEM-gx-witness-148, SEM-gx-witness-149)
//!
//! Identity: `req/142_M8_LEAN_CONFORMANCE_REQDEF_2026-08-14.md` §1 M8-c item 1, thin generation-side
//! channel per the C-4 ruling (`req/143_M8A_IMPL_REPORT_2026-08-14.md` §2). Lean side:
//! `lean/GxSpec/Runner.lean` (`checkReceiptVerify`), against the shape of
//! `T4_receipt_soundness`/`T5_witness_recovery` (`lean/GxSpec/Receipt.lean`, frozen at M8-b,
//! `req/38` §85-4/5·§86).
//!
//! # A declared ledger, not `gx-witness`'s real DSSE/Merkle verification
//!
//! `GxSpec.Ledger.contains`/`GxSpec.InclusionProof.verifies` are `Prop`-valued fields with no
//! concrete definition in the frozen Lean module — 46 §1 puts the actual cryptographic
//! primitives (BLAKE3, DSSE, Merkle inclusion) out of Lean's formalisation scope on purpose
//! ("in-Lean verification of crypto..." is an explicit non-goal; §2.5: receipt "soundness" is
//! proved "against an abstract witness in the model"). `gx-witness`'s real `verify_offline` is already exercised by that
//! crate's own suites (`receipt_kind_branch`, `receipt_verdict_wire`, the DSSE/Merkle property
//! tests referenced by 46 §4.2/§4.3) — duplicating that machinery here would require re-modelling
//! BLAKE3/Ed25519/Merkle proofs in Lean, which is the non-goal.
//!
//! What this file generates instead is a small, real, computable ledger:
//! `entries : List (t, v)` is the ground truth; `claimed` is what an inclusion proof asserts
//! (usually — but not always — a subset of `entries`, so `proof_sound` (`claimed ⊆ entries`) is
//! exercised on both its true and false branches rather than being tautological). `receipt` is
//! drawn from `claimed`, mirroring `ValidReceipt`. `A` (the admissibility half of T4's
//! conclusion) is declared as the always-true predicate — documented explicitly, not hidden —
//! isolating the kind's differential-testable content to `ProofSound ∧ ... → contains`, which is
//! T4's non-trivial half. T5's `recover`/witness triple (`t1`,`t2`,`tcomp`) follows
//! `RecoverableChainWitness` (`Receipt.lean`)'s own construction pattern: placed directly in
//! `entries` so the property holds by the same by-construction style the frozen witness uses.

mod support;

use gx_core::VerdictKind;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::{json, Value};
use support::{conformance_dir, git_sha, write_jsonl};

const N: usize = 250;

/// PR-scale default (`N`, unchanged from M8-c) unless overridden by `GX_CONFORMANCE_N` -- the
/// nightly AC-063 run (`req/142` §1 M8-d item 2) sets this to reach >=10^5 vectors in aggregate
/// across all five kinds without touching the PR-scale gen path at all.
fn vectors_per_kind() -> usize {
    std::env::var("GX_CONFORMANCE_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(N)
}

fn new_runner(seed: u8) -> TestRunner {
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]);
    TestRunner::new_with_rng(Config::default(), rng)
}

fn verdict_kind() -> impl Strategy<Value = VerdictKind> {
    proptest::prelude::prop_oneof![
        proptest::prelude::Just(VerdictKind::Admit),
        proptest::prelude::Just(VerdictKind::Deny),
        proptest::prelude::Just(VerdictKind::Escalate),
    ]
}

fn verdict_str(v: VerdictKind) -> &'static str {
    match v {
        VerdictKind::Admit => "admit",
        VerdictKind::Deny => "deny",
        VerdictKind::Escalate => "escalate",
    }
}

fn entry_json(t: u8, v: VerdictKind) -> Value {
    json!({"t": t, "v": verdict_str(v)})
}

fn contains(entries: &[(u8, VerdictKind)], t: u8, v: VerdictKind) -> bool {
    entries.iter().any(|&(et, ev)| et == t && ev == v)
}

#[test]
fn conformance_gen_receipt_verify() {
    let sha = git_sha();
    let mut runner = new_runner(0xB1);
    // 4 real ledger entries + 1 optional bogus claim + the T5 triple (t1, t2, tcomp), each
    // (tag: u8, verdict) -- entries.len() stays small so a 250-vector JSONL corpus is cheap to
    // parse on the Lean side.
    let strat = (
        proptest::collection::vec((proptest::prelude::any::<u8>(), verdict_kind()), 2..5), // entries
        0u8..3usize as u8, // claimed_take: how many of entries.len() get claimed (bounded below)
        proptest::prelude::any::<bool>(), // inject a bogus (unsound) claim
        proptest::prelude::any::<u8>(), // bogus tag (only used if injecting)
        verdict_kind(),    // bogus verdict (only used if injecting)
        (proptest::prelude::any::<u8>(), verdict_kind()), // t1, v1 (T5)
        (proptest::prelude::any::<u8>(), verdict_kind()), // t2, v2 (T5)
        (proptest::prelude::any::<u8>(), verdict_kind()), // tcomp, vcomp (T5)
    );
    let count = vectors_per_kind();
    let mut lines = Vec::with_capacity(count);
    for i in 0..count {
        let (
            mut entries,
            claimed_take,
            inject_bogus,
            bogus_t,
            bogus_v,
            (t1, v1),
            (t2, v2),
            (tcomp, vcomp),
        ) = strat
            .new_tree(&mut runner)
            .expect("strategy draws")
            .current();

        // T5's triple, appended to the real ledger first (RecoverableChainWitness's own
        // pattern) -- every "contains" check below reads the *final* entries list, so a bogus
        // claim's not-contained status cannot be invalidated by a later append.
        entries.push((t1, v1));
        entries.push((t2, v2));
        entries.push((tcomp, vcomp));

        // claimed = a real prefix of entries (always sound unless bogus is injected below).
        let take = (usize::from(claimed_take) % entries.len().max(1))
            .max(1)
            .min(entries.len());
        let mut claimed: Vec<(u8, VerdictKind)> = entries[..take].to_vec();
        let mut proof_sound_expected = true;
        if inject_bogus && !contains(&entries, bogus_t, bogus_v) {
            claimed.push((bogus_t, bogus_v));
            proof_sound_expected = false;
        }
        // the receipt under test: the first claimed entry (ValidReceipt by construction).
        let (r_t, r_v) = claimed[0];

        let is_admit = matches!(r_v, VerdictKind::Admit);
        let receipt_contains = contains(&entries, r_t, r_v);
        // proof_sound recomputed structurally (claimed subset entries by real membership), not
        // trusted from the injection flag alone -- catches an off-by-one in the generator itself.
        let proof_sound = claimed.iter().all(|&(ct, cv)| contains(&entries, ct, cv));
        debug_assert_eq!(proof_sound, proof_sound_expected);
        let conclusion_should_hold = !(is_admit && proof_sound) || receipt_contains;

        let t1_contains = contains(&entries, t1, v1);
        let t2_contains = contains(&entries, t2, v2);

        let vector = json!({
            "vector_id": format!("m8c-receipt_verify-{i:05}"),
            "kind": "receipt_verify",
            "input": {
                "entries": entries.iter().map(|&(t, v)| entry_json(t, v)).collect::<Vec<_>>(),
                "claimed": claimed.iter().map(|&(t, v)| entry_json(t, v)).collect::<Vec<_>>(),
                "receipt": entry_json(r_t, r_v),
                "recover": {"tcomp": tcomp, "t1": t1, "t2": t2},
            },
            "expected": {
                "t4": {
                    "valid_receipt": true,
                    "is_admit": is_admit,
                    "proof_sound": proof_sound,
                    "contains": receipt_contains,
                    "conclusion_should_hold": conclusion_should_hold,
                },
                "t5": {
                    "t1_contains": t1_contains,
                    "t2_contains": t2_contains,
                    "recovers": t1_contains && t2_contains,
                },
            },
            "rust_git_sha": sha,
            "lean_toolchain": "leanprover/lean4:v4.33.0",
        });
        lines.push(vector);
    }
    write_jsonl(&conformance_dir(), "receipt_verify.jsonl", &lines);
    println!("CONFORMANCE_GEN receipt_verify={}", lines.len());
}
