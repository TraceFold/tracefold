// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! M8-c conformance vector generator — `admissibility` (T1) / `invariant_composition` (T2).
//!
//! Identity: `req/142_M8_LEAN_CONFORMANCE_REQDEF_2026-08-14.md` §1 M8-c item 1, thin generation-side (sem: SEM-gx-gate-256)
//! channel per the C-4 ruling (`req/143_M8A_IMPL_REPORT_2026-08-14.md` §2). Lean side:
//! `lean/GxSpec/Runner.lean` (`checkAdmissibility` / `checkInvariantComposition`), against the
//! frozen `GxSpec.Admissible` / `GxSpec.Invariant` modules (M8-b, `req/38` §86).
//!
//! # Why these two kinds are a *declared class*, not this crate's real Cedar/registry output
//!
//! This crate's own module doc (`crates/gx-gate/src/lib.rs` "T1 is not a property of this crate
//! (**E-M3-5**)") and the adjudicated erratum it quotes (`req/38_ERRATA_2026-08-07.md` §19,
//! ruling **M3-05**) already establish that `A(f) ∧ A(g) → A(g∘f)` is **false** of an arbitrary
//! Cedar policy set (documented counterexample: a policy forbidding `/a` and `/b` together admits
//! `f` on `/a` alone and `g` on `/b` alone but denies `g∘f`) — and that a property test for it
//! "'restricted to the multiplicative class + a declared class precondition' only has meaning in
//! that shape, **into M8's restricted suite**" (sem: SEM-gx-gate-257). This
//! file is that limited suite. The class used is `GxSpec.OrderBounded n` — the exact declared
//! predicate `lean/GxSpec/Admissible.lean::orderBounded_admissible` already uses as T1's own
//! non-vacuity witness (frozen at M8-b) — evaluated over real `order` values (ASM-6's `0..=2`
//! ceiling, the same range `crates/gx-canon/tests/support/mod.rs::any_transformation` draws).
//! This is **not** gx-gate's Cedar/invariant-registry output; `req/145_M8C_IMPL_REPORT_2026-08-14.md`
//! reports this split explicitly rather than silently narrowing what "admissibility" tests.
//!
//! `invariant_composition` is the same shape for the analogous reason: `crates/gx-gate/src/
//! invariant.rs` ships an **empty** registry by default (FR-028's shipped set is the policy pack,
//! not the registry — "The registry ships **empty**"), so there is no real invariant to draw a
//! non-trivial composed value from without inventing a policy-shaped fixture. The declared class
//! here is `digestBelow t`, a real function of the real (generated) `ObjectSnapshot.digest` byte,
//! computed identically and independently on both sides.
//!
//! # v0.2-c: `step_bounded` — a declared class whose closure is NOT structurally guaranteed
//! (`DR-46-3`, `req/150` §C, resolving `req/38` §89 A-2 / `req/147` §3)
//!
//! The M8 adversarial audit proved that both classes above make `law_holds` a mathematical
//! tautology: `OrderBounded` is closed because `compose` takes `max` of orders (`max`-closure
//! identity), and `digestBelow` interval chaining is a hypothetical syllogism — no input, valid
//! or not, can make either law false. `conformance_gen_step_bounded` below adds a third
//! T1-family kind whose declared class `StepBounded k` («the first digest byte moves by at most
//! `k` across the transformation», |src−dst| ≤ k) is **not** closed under composition: `f` moving
//! `a→b` and `g` moving `b→c` each within `k` says nothing bounding `|a−c|` beyond `2k`, so the
//! composed transformation genuinely falls outside the class on real generated inputs.
//!
//! Two consequences, both deliberate:
//!
//! * `law_holds` here is **data, not an assertion**: Rust records the actually-evaluated truth
//!   value (false included), and `Runner.lean` recomputes it independently and compares for
//!   *equality* — a divergence means cross-language disagreement about the valuation, never
//!   "the law failed". This is what makes the law check falsifiable in the `DR-46-3` sense: a
//!   `law_holds:false` vector in the corpus is a live sample proving the check is not vacuous.
//! * **M3-05 consistency** (`req/38` §19: an unrestricted `A(f) ∧ A(g) → A(g∘f)` property test is
//!   meaningful only «restricted to the multiplicative class + a declared class precondition») (sem: SEM-gx-gate-258): `admissibility` above remains
//!   the multiplicative-class-limited suite that *asserts* closure as a truth; `step_bounded`
//!   declares a non-multiplicative class precisely in order to *measure* where closure fails, and
//!   asserts nothing about the law's truth — only bilingual agreement on its value. The batch
//!   assertion below (both `law_holds` branches non-empty, deterministic under the fixed seed) is
//!   the standing machine gate for `AC-V2C-1`'s distribution requirement.

mod support;

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::json;
use support::{conformance_dir, git_sha, write_jsonl};

const N_PER_KIND: usize = 250;

/// PR-scale default (`N_PER_KIND`, unchanged from M8-c) unless overridden by `GX_CONFORMANCE_N`
/// -- the nightly AC-063 run (`req/142` §1 M8-d item 2) sets this to reach >=10^5 vectors in
/// aggregate across all five kinds without touching the PR-scale gen path at all.
fn vectors_per_kind() -> usize {
    std::env::var("GX_CONFORMANCE_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(N_PER_KIND)
}

fn new_runner(seed: u8) -> TestRunner {
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]);
    TestRunner::new_with_rng(Config::default(), rng)
}

/// order in `0..=2` — ASM-6's admitted ceiling, the range `any_transformation()` already draws.
fn order_strategy() -> impl Strategy<Value = u8> {
    0u8..=2
}

/// A single-byte digest tag. `GxSpec.Core.ObjectSnapshot.digest` is a `ByteArray` in Lean and an
/// unconstrained-length one in Rust (42); a single byte is the smallest faithful instance that
/// still lets `composable` (`f.dst = g.src`) be a real, non-degenerate equality check and keeps
/// the JSONL small across a 250-vector corpus.
fn byte_strategy() -> impl Strategy<Value = u8> {
    proptest::prelude::any::<u8>()
}

#[test]
fn conformance_gen_admissibility() {
    let sha = git_sha();
    let mut runner = new_runner(0xA1);
    let strat = (
        order_strategy(), // n (class bound)
        order_strategy(), // f.order
        order_strategy(), // g.order
        byte_strategy(),  // f.src digest
        byte_strategy(),  // f.dst = g.src digest (composable by construction)
        byte_strategy(),  // g.dst digest
    );
    let count = vectors_per_kind();
    let mut lines = Vec::with_capacity(count);
    for i in 0..count {
        let (n, f_order, g_order, f_src, mid, g_dst) = strat
            .new_tree(&mut runner)
            .expect("strategy draws")
            .current();

        let admit_f = f_order <= n;
        let admit_g = g_order <= n;
        let admit_compose = f_order.max(g_order) <= n;

        let vector = json!({
            "vector_id": format!("m8c-admissibility-{i:05}"),
            "kind": "admissibility",
            "input": {
                "n": n,
                "f": {"order": f_order, "src_digest": f_src, "dst_digest": mid},
                "g": {"order": g_order, "src_digest": mid, "dst_digest": g_dst},
            },
            "expected": {
                "composable": true,
                "admit_f": admit_f,
                "admit_g": admit_g,
                "admit_compose": admit_compose,
            },
            "rust_git_sha": sha,
            "lean_toolchain": "leanprover/lean4:v4.33.0",
        });
        lines.push(vector);
    }
    write_jsonl(&conformance_dir(), "admissibility.jsonl", &lines);
    println!("CONFORMANCE_GEN admissibility={}", lines.len());
}

#[test]
fn conformance_gen_invariant_composition() {
    let sha = git_sha();
    let mut runner = new_runner(0xA2);
    let strat = (
        byte_strategy(), // t_i threshold
        byte_strategy(), // t_j threshold
        byte_strategy(), // t_k threshold
        byte_strategy(), // f.src digest
        byte_strategy(), // f.dst = g.src digest
        byte_strategy(), // g.dst digest
    );
    let count = vectors_per_kind();
    let mut lines = Vec::with_capacity(count);
    for i in 0..count {
        let (t_i, t_j, t_k, src_f, mid, dst_g) = strat
            .new_tree(&mut runner)
            .expect("strategy draws")
            .current();

        let i_src = src_f < t_i;
        let j_dst_f = mid < t_j;
        let j_src_g = mid < t_j; // same byte as j_dst_f by construction (composable)
        let k_dst_g = dst_g < t_k;
        let triple_f_holds = !i_src || j_dst_f;
        let triple_g_holds = !j_src_g || k_dst_g;
        let composed_holds = !i_src || k_dst_g;

        let vector = json!({
            "vector_id": format!("m8c-invariant_composition-{i:05}"),
            "kind": "invariant_composition",
            "input": {
                "t_i": t_i, "t_j": t_j, "t_k": t_k,
                "f": {"src_digest": src_f, "dst_digest": mid},
                "g": {"src_digest": mid, "dst_digest": dst_g},
            },
            "expected": {
                "composable": true,
                "i_src": i_src,
                "j_dst_f": j_dst_f,
                "j_src_g": j_src_g,
                "k_dst_g": k_dst_g,
                "triple_f_holds": triple_f_holds,
                "triple_g_holds": triple_g_holds,
                "composed_holds": composed_holds,
            },
            "rust_git_sha": sha,
            "lean_toolchain": "leanprover/lean4:v4.33.0",
        });
        lines.push(vector);
    }
    write_jsonl(&conformance_dir(), "invariant_composition.jsonl", &lines);
    println!("CONFORMANCE_GEN invariant_composition={}", lines.len());
}

/// |a − b| over `u8`, as the generator's one-line restatement of the declared class — the Lean
/// side (`GxSpec.Runner.StepBounded`) defines the same distance independently over `Nat`.
fn step(a: u8, b: u8) -> u8 {
    a.max(b) - a.min(b)
}

#[test]
fn conformance_gen_step_bounded() {
    let sha = git_sha();
    let mut runner = new_runner(0xA3);
    let strat = (
        8u8..=128,       // k (class bound; range chosen so both law branches populate well)
        byte_strategy(), // f.src digest
        byte_strategy(), // f.dst = g.src digest (composable by construction)
        byte_strategy(), // g.dst digest
    );
    let count = vectors_per_kind();
    let mut lines = Vec::with_capacity(count);
    let mut law_true = 0usize;
    let mut law_false = 0usize;
    for i in 0..count {
        let (k, f_src, mid, g_dst) = strat
            .new_tree(&mut runner)
            .expect("strategy draws")
            .current();

        let within_f = step(f_src, mid) <= k;
        let within_g = step(mid, g_dst) <= k;
        // The composed transformation's shape per GxSpec.compose: src = f.src, dst = g.dst.
        let within_compose = step(f_src, g_dst) <= k;
        // Evaluated, not asserted (module doc): false is a legitimate, expected sample.
        let law_holds = !(within_f && within_g) || within_compose;
        if law_holds {
            law_true += 1;
        } else {
            law_false += 1;
        }

        let vector = json!({
            "vector_id": format!("v02c-step_bounded-{i:05}"),
            "kind": "step_bounded",
            "input": {
                "k": k,
                "f": {"src_digest": f_src, "dst_digest": mid},
                "g": {"src_digest": mid, "dst_digest": g_dst},
            },
            "expected": {
                "composable": true,
                "within_f": within_f,
                "within_g": within_g,
                "within_compose": within_compose,
                "law_holds": law_holds,
            },
            "rust_git_sha": sha,
            "lean_toolchain": "leanprover/lean4:v4.33.0",
        });
        lines.push(vector);
    }
    // AC-V2C-1's distribution gate (module doc): the corpus must contain real samples of BOTH
    // law_holds branches, or the class has degenerated back into an A-2-style tautology (or into
    // an always-false vacuity). Deterministic under the fixed seed.
    assert!(
        law_false > 0,
        "step_bounded generated no law_holds=false sample -- DR-46-3's non-tautology \
         evidence is gone from the corpus"
    );
    assert!(
        law_true > 0,
        "step_bounded generated no law_holds=true sample -- the law check degenerated to \
         always-false vacuity"
    );
    write_jsonl(&conformance_dir(), "step_bounded.jsonl", &lines);
    println!(
        "CONFORMANCE_GEN step_bounded={} law_true={law_true} law_false={law_false}",
        lines.len()
    );
}
