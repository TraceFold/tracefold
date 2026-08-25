// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **FR-M7-2's measuring instrument** — measures how the cost of `prove_inclusion` grows with `n`,
//! as a **2-arm controlled experiment** (req/98 §3-2 / §6-6, additional ruling a = `req/38` §56). (sem: SEM-gx-log-008)
//!
//! # Why this file exists first (the RED-first shape, for a bench) (sem: SEM-gx-log-009)
//!
//! req/98 §6-6 requires the controlled experiment to follow req/95 §1's template -- "same machine,
//! same instrument file, back-to-back runs, **record each arm's commit**, median + count +
//! denominator". Writing the implementation before the instrument lets the instrument choose its
//! threshold after already knowing the answer. ∴ this file's order is **write the judgement condition
//! first, run it on arm A (before the implementation) and watch it go red** -- and that red is
//! recorded as a measurement in req/103 §2. (sem: SEM-gx-log-010)
//!
//! ## The judgement condition (written before the numbers) (sem: SEM-gx-log-011)
//!
//! ```text
//! SUBLINEAR := median(n = 64_000) / median(n = 8_000) < 2.0
//! ```
//!
//! the ratio for an 8x `n`. **Linear lands around 8** (req/97 §3.1's measurement: 1,000→64,000, a 64x
//! `n` for a 68x time), **option A lands near 1** -- the tile cache holds the root of a completed
//! 256-leaf block, so one proof's hash count becomes `O(n/256 + 256·log(n/256))`. The threshold 2.0
//! sits **between** "linear's 8" and "option A's estimate of 1.2", and decides only which side it
//! falls on. Following M5H7-6's "median or p90 if it's used as a gate", **the judgement uses the
//! median**; p99 is recorded only. (sem: SEM-gx-log-012)
//!
//! ## 🔴 Measuring the wire invariant, across 2 builds, **byte for byte** (sem: SEM-gx-log-013)
//!
//! additional ruling a requires that "`InclusionProof`'s wire shape does not change". The same-build
//! probe (`tests/incremental_inclusion.rs`) measures agreement with an independent oracle, but that
//! is a story inside one build. This instrument prints the **fingerprint of every proof it produced,
//! run through BLAKE3 in order, over their canonical DAG-CBOR**: if arm A and arm B print the same
//! fingerprint, the two builds' proofs **do not differ by a single byte**. That is the strongest
//! shape a cross-build comparison can state, and it is also the measurement that `verify_inclusion_of`
//! and a third-party verifier pass without changing a single line. (sem: SEM-gx-log-014)
//!
//! # Denominator (sem: SEM-gx-log-015)
//!
//! Measures only when `cargo bench` carries `--bench` (the same shape as support's `measuring()`).
//! `cargo test` confirms only that "this file still builds" -- so that the 23x unoptimised-profile
//! gap AC-064's file records is never read as a measurement. (sem: SEM-gx-log-016)

use std::hint::black_box;
use std::time::{Duration, Instant};

use gx_core::{Cid, InclusionProof, Timestamp, TransformationId};
use gx_log::proof::prove_inclusion_at;
use gx_log::tile::TileLog;

/// The same steps as req/97 §3.1's table, so arm A's numbers can be compared against it. (sem: SEM-gx-log-017)
const SIZES: [u64; 7] = [1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 64_000];

/// How many proofs are taken per size. This is the median's denominator, and it is more than req/97's 5. (sem: SEM-gx-log-018)
const PROOFS_PER_SIZE: usize = 21;

/// The two points the judgement uses. `n` is 8x. (sem: SEM-gx-log-019)
const SMALL: u64 = 8_000;
const LARGE: u64 = 64_000;

/// The threshold for "is sublinear". Linear lands around 8, option A around 1. (sem: SEM-gx-log-020)
const SUBLINEAR_RATIO: f64 = 2.0;

/// 🔴 **§62 R-7**: turns the judgement into an **exit code** (wired to `tools/ci.sh` stage 10, default off). (sem: SEM-gx-log-021)
///
/// R-7 verbatim (req/103 §9): "**not one test guards option A's 'speed'**… a mutant that writes
/// `audit_path_at` back to the old O(n) implementation turns fully green (the answer is the same).
/// The only thing guarding speed is `benches/inclusion_proof.rs`'s judgement condition, and it is
/// **wired into neither `tools/ci.sh` nor `tools/e2e.sh`**." The ruling is "wire it into stage 10's
/// `GLOVREX_CI_BENCH=1` side, with a threshold -- do not default it on", and `stage` reads a command's
/// RC -- ∴ the judgement has to be an **RC**, not print output, or wiring it in stops nothing. (sem: SEM-gx-log-022)
///
/// The budget can be moved by env, for the same reason `GLOVREX_BENCH_SECONDS` exists at stage 10c:
/// **a moved value prints beside the number** (`BUDGET_SOURCE`) -- a loosened run and a run at the
/// declared value must not read the same (req/29 §4). The end-to-end wiring check (hand it an
/// impossible budget and observe a non-zero exit) is measured once by `tools/verify_m7h6.sh 4`. (sem: SEM-gx-log-023)
fn budget() -> (f64, &'static str) {
    match std::env::var("GLOVREX_INCLUSION_MAX_RATIO")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        Some(overridden) => (overridden, "GLOVREX_INCLUSION_MAX_RATIO"),
        None => (SUBLINEAR_RATIO, "declared"),
    }
}

/// A distinguishable digest. Not a hash of anything — the tree's shape is what is being measured,
/// not the canonical form of a receipt.
fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn log_of(n: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        log.append(
            TransformationId(cid(i)),
            cid(1_000_000 + i),
            Timestamp(i as i64),
        )
        .expect("a leaf of three ids has a canonical form");
    }
    log
}

/// The indices proved at each size: the first leaf, the middle, the last, and a spread between.
///
/// req/97 §3.1 measured first/middle/last and found no difference, which was itself the finding (the (sem: SEM-gx-log-024)
/// walk is over the whole tree rather than over the path). Keeping the spread means arm B can be
/// read against that: a cache that helped only the leftmost leaf would show up as a spread.
fn indices(n: u64) -> Vec<u64> {
    (0..PROOFS_PER_SIZE)
        .map(|k| (n - 1).saturating_mul(k as u64) / (PROOFS_PER_SIZE as u64 - 1))
        .collect()
}

/// Nearest-rank percentiles with the sample count beside them (M3-15's "median + count + denominator"). (sem: SEM-gx-log-025)
fn report(tag: &str, name: &str, samples: &mut [Duration]) -> Duration {
    assert!(!samples.is_empty(), "a distribution needs samples");
    samples.sort_unstable();
    let n = samples.len();
    let at = |q: f64| -> Duration {
        let rank = ((q * n as f64).ceil() as usize).clamp(1, n);
        samples[rank - 1]
    };
    println!(
        "{tag} {name:<18} n={n} min={:>10.3?} p50={:>10.3?} p90={:>10.3?} p99={:>10.3?} max={:>10.3?}",
        samples[0],
        at(0.50),
        at(0.90),
        at(0.99),
        samples[n - 1],
    );
    at(0.50)
}

fn measuring() -> bool {
    std::env::args().any(|a| a == "--bench")
}

fn main() {
    if !measuring() {
        // `cargo test` builds this file and reaches the arms it claims, on a tree small enough to
        // cost nothing: a build check, not a measurement.
        let log = log_of(300);
        let proof = prove_inclusion_at(&log, 7, 300).expect("the leaf is in the tree");
        assert_eq!(proof.tree_size, 300);
        println!(
            "INCLUSION_BENCH_BUILD_ONLY audit_path={}",
            proof.audit_path.len()
        );
        return;
    }

    println!(
        "INCLUSION_ARM sizes={SIZES:?} proofs_per_size={PROOFS_PER_SIZE} \
         judgement=\"median({LARGE})/median({SMALL}) < {SUBLINEAR_RATIO}\" \
         (M5H7-6: judgement uses the median, p99 is recorded only) (sem: SEM-gx-log-026)"
    );

    let mut medians: Vec<(u64, Duration)> = Vec::new();
    // The fingerprint is taken over **every** proof this run produced, in order. Two builds that
    // print the same value produced byte-identical proofs.
    let mut produced: Vec<InclusionProof> = Vec::new();

    for size in SIZES {
        let log = log_of(size);
        let mut took: Vec<Duration> = Vec::new();
        let mut path_lengths: Vec<usize> = Vec::new();
        for index in indices(size) {
            let started = Instant::now();
            let proof = prove_inclusion_at(&log, index, size).expect("the leaf is in the tree");
            took.push(started.elapsed());
            path_lengths.push(proof.audit_path.len());
            black_box(&proof);
            produced.push(proof);
        }
        let median = report("INCLUSION_PROVE", &format!("n={size}"), &mut took);
        println!(
            "INCLUSION_PATHS n={size} audit_path_len_min={} max={}  (wire: the proof carries \
             log2(n) hashes and always did — the cost was never the proof's size)",
            path_lengths.iter().min().copied().unwrap_or(0),
            path_lengths.iter().max().copied().unwrap_or(0),
        );
        medians.push((size, median));
    }

    let small = medians
        .iter()
        .find(|(n, _)| *n == SMALL)
        .expect("SMALL is one of SIZES")
        .1;
    let large = medians
        .iter()
        .find(|(n, _)| *n == LARGE)
        .expect("LARGE is one of SIZES")
        .1;
    let ratio = large.as_secs_f64() / small.as_secs_f64();
    let (budget, budget_source) = budget();
    let pass = ratio < budget;
    println!(
        "INCLUSION_VERDICT ratio={ratio:.3} (median n={LARGE} / median n={SMALL}) \
         budget={budget} BUDGET_SOURCE={budget_source} pass={pass} \
         — linear lands around 8 (n is 8x), option A lands around 1 (sem: SEM-gx-log-027)"
    );
    // 🔴 Minted through gx-canon, because 41 §6 admits one road to a canonical encoding and a bench
    // that reached for a hasher directly would be the second. The `Leaf` domain is a namespace
    // choice with exactly one consumer — a human comparing two logs — and the value is **not** a
    // ledger leaf; what it is, is "the canonical form of every proof this run produced, in order" (sem: SEM-gx-log-028).
    let fingerprint =
        gx_canon::cid::mint_leaf(&produced).expect("a vector of proofs has a canonical form");
    println!(
        // (sem: SEM-gx-log-029)
        "INCLUSION_FINGERPRINT={} proofs={}  (every proof's canonical DAG-CBOR folded into one. \
         if the 2 arms agree, the wire moved by 0 bytes)",
        gx_canon::cid::to_text(&fingerprint),
        produced.len(),
    );

    // 🔴 §62 R-7: the judgement leaves the process, not just the log. The fingerprint above is
    // printed **first** on purpose — a run that is about to fail still has to hand over everything
    // it measured, because a red stage whose output stops at the verdict makes the next reader
    // re-run the five minutes to find out why.
    if !pass {
        eprintln!(
            "INCLUSION_FAIL ratio={ratio:.3} >= budget={budget} ({budget_source}) — \
             the shape of O(n) come back. This one line is the only thing guarding it (req/103 §9 R-7)" // (sem: SEM-gx-log-030)
        );
        std::process::exit(1);
    }
}
