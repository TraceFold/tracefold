// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! DR-46-24(A) — what a read-set costs, at both granularities, measured rather than assumed.
//!
//! `req/350` §4-4 filed the reason this file exists as the loudest self-report in that report: the
//! generation cost of an inclusion path, the verification time with the path included, and where to
//! put the path were not measured at a single point, so "G4 spill is a design proposal and not a
//! proposal supported by measurement — **the first job of the implementation lane should be to
//! measure here**". `req/38` §236 ruling 2 repeats it and `req/440` §0-1 makes it an order: G4 is
//! not implemented until this measurement is finished. So this file measures first and the wire
//! follows it.
//!
//! # What is measured, and against which denominator
//!
//! Four quantities, at distinct-object counts 1/5/6/32 (the four `req/350` §7-7 fixes for CI) plus
//! the powers between and beyond them that make the curve readable:
//!
//! 1. **Receipt bytes** — the encoded size of each granularity's member. `req/350` §2-3 took these
//!    by arithmetic over member encodings; this encodes the member, so the arithmetic has something
//!    to be checked against.
//! 2. **Path generation** — the cost of producing one inclusion path from the leaves.
//! 3. **Verification with the path** — folding a leaf and its path back to the root, beside the G3
//!    road's own check (find the locator among the entries).
//! 4. **The attest cost against the strictest denominator available in-process**: the digest of the
//!    prior the escrow already reads. `req/350` §2-2 showed a denominator with a disk in it cannot
//!    fire; the digest of the same bytes is the smallest honest denominator, so a ratio taken
//!    against it is the hardest form of §7-5's falsifier.
//!
//! # The placement question, answered by arithmetic rather than by preference
//!
//! `req/350` §4-4's third unmeasured item is "where to put the path". Three placements are costed
//! here and printed as one table:
//!
//! * **A — every path inside the receipt.** The issuer cannot know which object a verifier will
//!   later ask about, so this means `n` paths, not one.
//! * **B — the root inside the receipt, the entries beside it, the path derived on demand.**
//! * **C — the root inside the receipt, the paths beside it.**
//!
//! B and C carry the same receipt. They differ only in what the side artifact holds, and the
//! arithmetic below is what decides between them.
//!
//! # No hash is named here
//!
//! `crates/gx-canon/tests/ac_014.rs` scans every source line in `crates/` and `probes/` for the
//! string naming the workspace's hash, and `req/350` §9-2 measured a lane turning the floor red on
//! an output label alone. Every digest below is `gx_canon::cid`'s, and every name here says
//! `digest`.

use std::time::Instant;

use gx_canon::{cbor, cid};
use gx_core::Cid;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// The two shapes, as local fixtures
// ---------------------------------------------------------------------------
//
// Local, and deliberately: `req/440` §0-1 forbids implementing G4 before this measurement, so the
// measurement may not depend on the type it is measuring for.

/// G3's member: one entry per distinct object the escrow read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ProbeEntry {
    digest: Cid,
    locator: String,
}

/// G4's member: the root of the tree over those entries, and how many leaves it folds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ProbeRoot {
    leaf_count: u64,
    root: Cid,
}

/// `req/350` §2-3's "full locator (50 characters — the form a receipt actually carries)".
fn locator(i: usize) -> String {
    format!("mcp://server-{i:04}/resource/notes/{i:04}/body.md#frag")
}

fn entries(n: usize) -> Vec<ProbeEntry> {
    (0..n)
        .map(|i| ProbeEntry {
            digest: cid::mint(cid::Domain::Leaf, &[locator(i).as_bytes()]),
            locator: locator(i),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The tree, over the rules 42 §3.11 already fixes
// ---------------------------------------------------------------------------

/// Leaf hashes, one per entry: `0x00 || canonical_dagcbor(entry)`, through gx-canon's mint.
fn leaves(entries: &[ProbeEntry]) -> Vec<Cid> {
    entries
        .iter()
        .map(|e| cid::mint_leaf(e).expect("an entry has a canonical form"))
        .collect()
}

/// RFC 6962's MTH, written as 42 §3.11 writes it. Empty is refused rather than given a value: a
/// read-set with no reads is absent, not a root over nothing.
fn root(leaves: &[Cid]) -> Cid {
    assert!(!leaves.is_empty(), "a root over no leaves is not a root");
    if leaves.len() == 1 {
        return leaves[0];
    }
    let k = split(leaves.len());
    cid::mint_node(&root(&leaves[..k]), &root(&leaves[k..]))
}

/// The largest power of two strictly below `n` — RFC 6962 §2.1's `k`.
fn split(n: usize) -> usize {
    let mut k = 1;
    while k * 2 < n {
        k *= 2;
    }
    k
}

/// The sibling hashes from `index` up to the root, leaf-first (the order `InclusionProof.audit_path`
/// already uses).
fn path(index: usize, leaves: &[Cid], out: &mut Vec<Cid>) {
    if leaves.len() <= 1 {
        return;
    }
    let k = split(leaves.len());
    if index < k {
        path(index, &leaves[..k], out);
        out.push(root(&leaves[k..]));
    } else {
        path(index - k, &leaves[k..], out);
        out.push(root(&leaves[..k]));
    }
}

/// Fold a leaf and its path back to a root — what a verifier does.
///
/// RFC 6962 §2.1.1's walk, in the same two-counter form `gx_log::proof::reconstruct_root` already
/// uses: `node` is the index within the level, `last` the index of that level's last node, their
/// parity says which side the sibling is on, and `node == last` marks the ragged right edge. Written
/// out rather than called because `gx-log`'s is private and keyed to a `LedgerLeaf`; the shipped
/// road is the one this measurement is deciding on.
fn fold(index: usize, leaf: &Cid, siblings: &[Cid], size: usize) -> Cid {
    let mut node = index;
    let mut last = size - 1;
    let mut acc = *leaf;
    for sibling in siblings {
        if node % 2 == 1 || node == last {
            acc = cid::mint_node(sibling, &acc);
            while node != 0 && node.is_multiple_of(2) {
                node /= 2;
                last /= 2;
            }
        } else {
            acc = cid::mint_node(&acc, sibling);
        }
        node /= 2;
        last /= 2;
    }
    acc
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

/// Median of `runs` readings. `req/350` §2's shape: median, never a single reading, and never a
/// `saturating_sub` that can only return the answer somebody wanted (§9-2).
fn median_ns(runs: usize, mut f: impl FnMut()) -> u128 {
    let mut samples: Vec<u128> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        f();
        samples.push(t.elapsed().as_nanos());
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

const RUNS: usize = 200;

/// The counts `req/350` §7-7 fixes, plus the shape of the curve between and beyond them.
const COUNTS: [usize; 8] = [1, 4, 5, 6, 8, 16, 32, 64];

// ---------------------------------------------------------------------------
// 1 — the tree is a tree, before anything is timed
// ---------------------------------------------------------------------------

/// Every leaf of every tree folds to its own root, and a stranger does not.
///
/// Timing a structure that does not hold would measure nothing, so this runs first.
#[test]
fn d24_every_path_folds_back_to_the_root_and_a_stranger_does_not() {
    for n in COUNTS {
        let set = entries(n);
        let ls = leaves(&set);
        let r = root(&ls);
        for (i, leaf) in ls.iter().enumerate() {
            let mut p = Vec::new();
            path(i, &ls, &mut p);
            assert_eq!(fold(i, leaf, &p, n), r, "n={n} index={i}");
            let stranger = cid::mint(cid::Domain::Leaf, &[b"not a member of this read-set"]);
            assert_ne!(
                fold(i, &stranger, &p, n),
                r,
                "n={n} index={i}: a stranger folded to the root"
            );
        }
    }
    println!("D24_TREE_SOUND=true COUNTS={COUNTS:?}");
}

// ---------------------------------------------------------------------------
// 2 — receipt bytes, both granularities
// ---------------------------------------------------------------------------

/// What each granularity adds to a receipt, encoded rather than estimated.
#[test]
fn d24_the_member_bytes_of_both_granularities() {
    println!("D24_BASELINE_PAYLOAD_BYTES=466  # req/350 §2-3, the shipped receipt it measured");
    for n in COUNTS {
        let set = entries(n);
        let g3 = cbor::encode(&set).expect("entries encode");
        let ls = leaves(&set);
        let g4 = cbor::encode(&ProbeRoot {
            leaf_count: n as u64,
            root: root(&ls),
        })
        .expect("a root encodes");
        println!(
            "D24_MEMBER n={n} G3_BYTES={} G4_BYTES={} G3_RATIO={:.3} G4_RATIO={:.3} G3_BYTES_PER_ENTRY={:.1}",
            g3.len(),
            g4.len(),
            1.0 + g3.len() as f64 / 466.0,
            1.0 + g4.len() as f64 / 466.0,
            g3.len() as f64 / n as f64,
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — the three things §4-4 says were never measured
// ---------------------------------------------------------------------------

/// Path generation, path-borne verification, and the placement arithmetic.
#[test]
fn d24_the_g4_costs_req350_did_not_measure() {
    for n in COUNTS {
        let set = entries(n);
        let ls = leaves(&set);
        let r = root(&ls);
        let target = n / 2;

        let build_ns = median_ns(RUNS, || {
            let _ = root(&ls);
        });
        let path_ns = median_ns(RUNS, || {
            let mut p = Vec::new();
            path(target, &ls, &mut p);
        });
        let leaves_ns = median_ns(RUNS, || {
            let _ = leaves(&set);
        });

        let mut p = Vec::new();
        path(target, &ls, &mut p);
        let path_bytes = cbor::encode(&p).expect("a path encodes").len();

        let verify_g4_ns = median_ns(RUNS, || {
            let _ = fold(target, &ls[target], &p, n);
        });
        let needle = locator(target);
        let verify_g3_ns = median_ns(RUNS, || {
            let _ = set.iter().find(|e| e.locator == needle);
        });
        assert_eq!(fold(target, &ls[target], &p, n), r);

        let entries_bytes = cbor::encode(&set).expect("entries encode").len();
        let all_paths_bytes: usize = (0..n)
            .map(|i| {
                let mut q = Vec::new();
                path(i, &ls, &mut q);
                cbor::encode(&q).expect("a path encodes").len()
            })
            .sum();

        println!(
            "D24_G4 n={n} PATH_LEN={} PATH_BYTES={path_bytes} LEAVES_MEDIAN_NS={leaves_ns} \
             ROOT_MEDIAN_NS={build_ns} PATH_MEDIAN_NS={path_ns} \
             VERIFY_G4_MEDIAN_NS={verify_g4_ns} VERIFY_G3_MEDIAN_NS={verify_g3_ns}",
            p.len()
        );
        println!(
            "D24_PLACEMENT n={n} A_ALL_PATHS_IN_RECEIPT={all_paths_bytes} \
             B_SIDE_ENTRIES={entries_bytes} C_SIDE_ALL_PATHS={all_paths_bytes} \
             B_OVER_C={:.3}",
            entries_bytes as f64 / all_paths_bytes.max(1) as f64
        );
    }
}

// ---------------------------------------------------------------------------
// 3b — the four points `req/350` §7-7 fixes for CI, over the **shipped** type
// ---------------------------------------------------------------------------

/// 🔴 The measured shape is the shipped shape, and the four ratios are pinned.
///
/// The fixtures above are local on purpose (`req/440` §0-1 forbids G4 depending on the type it was
/// measuring for). This is the join: the same four counts, encoded through
/// `gx_witness::receipt::ReadSet`, with the ratios asserted rather than printed. A drift in the
/// member's encoding — a renamed key, a variant that stopped being externally tagged — moves these
/// numbers and this fails.
///
/// The bounds are wide enough to survive a locator of a different length and narrow enough to
/// refuse a granularity that stopped spilling: the point of a CI pin is the **shape** of the curve
/// (G3 linear in `n`, G4 flat), not a byte count nobody can reproduce on another machine.
#[test]
fn d24_the_four_counts_req350_fixes_for_ci_over_the_shipped_type() {
    use gx_witness::receipt::{ReadEntry, ReadSet, READ_SET_SPILL_THRESHOLD};

    assert_eq!(
        READ_SET_SPILL_THRESHOLD, 5,
        "req/38 §236 ruling 2's constant"
    );

    let mut flat: Vec<usize> = Vec::new();
    for n in [1usize, 5, 6, 32] {
        let set = ReadSet::from_reads(
            (0..n)
                .map(|i| ReadEntry {
                    digest: cid::mint(cid::Domain::Leaf, &[locator(i).as_bytes()]),
                    locator: locator(i),
                })
                .collect(),
        )
        .expect("the entries encode");
        let bytes = cbor::encode(&set).expect("the member encodes").len();
        let ratio = 1.0 + bytes as f64 / 466.0;
        println!(
            "D24_CI n={n} GRANULARITY={} MEMBER_BYTES={bytes} RATIO={ratio:.3}",
            set.granularity()
        );
        assert_eq!(set.distinct_objects(), n as u64);

        match n {
            1 | 5 => {
                assert_eq!(set.granularity(), "G3", "at or below the threshold");
                assert_eq!(set.names(&locator(0)), Some(true), "G3 decides alone");
            }
            _ => {
                assert_eq!(set.granularity(), "G4", "past the threshold it spills");
                assert_eq!(set.names(&locator(0)), None, "G4 does not decide alone");
                flat.push(bytes);
            }
        }

        // The curve's shape, as two claims a byte count cannot fake.
        match n {
            1 => assert!((1.15..1.35).contains(&ratio), "one entry, ratio {ratio}"),
            5 => assert!((1.9..2.3).contains(&ratio), "five entries, ratio {ratio}"),
            _ => assert!(
                (1.05..1.20).contains(&ratio),
                "a root is flat in n; ratio {ratio} at n={n}"
            ),
        }
    }
    assert!(
        flat[0].abs_diff(flat[1]) <= 2,
        "G4's member is flat in n: {flat:?} at n=6 and n=32"
    );
}

// ---------------------------------------------------------------------------
// 4 — §7-5's falsifier, against the strictest denominator this process holds
// ---------------------------------------------------------------------------

/// "Re-design if the attest cost exceeds 10% of the escrow read cost for the same bytes"
/// (`req/350` §7-5, adopted by `req/38` §236 ruling 1).
///
/// The denominator a shipped escrow read carries is a server round trip; `req/350` §2-2 measured
/// its floor at 0.28–11 ms on tmpfs and about 103 ms on ext4, and showed that a falsifier written
/// against it cannot fire. So this takes the **smallest** denominator that is still honestly part
/// of the read: the digest of the prior bytes the escrow already holds. A ratio under 10% against
/// that is a ratio under 10% against anything larger.
/// # 🔴 And what it found: the replacement falsifier is structurally broken too, in the other
/// direction
///
/// `req/350` §2-2 retired the first falsifier ("wrap latency +5%") because its denominator was a
/// disk and could not fire whatever the design did. The replacement's denominator is the escrow
/// read — and the attest cost has a **floor** that does not shrink with the prior (one canonical
/// encode and one digest per entry, about 2.5 µs at `n=1`), while the denominator does. So the
/// ratio is worst for **small** priors and best for large ones, for any implementation: it fires at
/// a 1 KiB prior and passes at 512 KiB, and neither reading is about the design. The measurement is
/// asserted as that shape rather than as a verdict, and `req/441` §3 takes the denominator back to
/// Fable.
#[test]
fn d24_the_attest_cost_against_the_smallest_honest_denominator() {
    let mut by_prior: Vec<(usize, f64)> = Vec::new();
    for bytes in [1_024usize, 65_536, 524_288] {
        let prior = vec![0xa5u8; bytes];
        let prior_ns = median_ns(RUNS, || {
            let _ = cid::mint(cid::Domain::Leaf, &[&prior]);
        });
        for n in [1usize, 5, 6, 32] {
            let set = entries(n);
            let attest_ns = median_ns(RUNS, || {
                let ls = leaves(&set);
                let _ = root(&ls);
            });
            let pct = attest_ns as f64 * 100.0 / prior_ns.max(1) as f64;
            println!(
                "D24_FALSIFIER prior_bytes={bytes} n={n} PRIOR_DIGEST_MEDIAN_NS={prior_ns} \
                 ATTEST_MEDIAN_NS={attest_ns} PCT_OF_PRIOR_DIGEST={pct:.2} OVER_10PCT={}",
                pct > 10.0
            );
            if n == 1 {
                by_prior.push((bytes, pct));
            }
        }
    }
    // The finding, as a claim about the ratio rather than about the design: it is governed by the
    // denominator's size. A falsifier whose verdict is decided by how large the prior happened to
    // be is not measuring the mechanism, which is exactly the criticism §2-2 made of the one this
    // replaced.
    println!("D24_FALSIFIER_SHAPE={by_prior:?}");
    assert!(
        by_prior[0].1 > by_prior[1].1 && by_prior[1].1 > by_prior[2].1,
        "the attest/prior-digest ratio is expected to fall as the prior grows, because the \
         numerator has a floor and the denominator does not: {by_prior:?}"
    );
}
