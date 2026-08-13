//! 🔴 **FR-M7-2 の測定計器** — `prove_inclusion` の cost が `n` とともにどう伸びるか、を **2 arm の
//! 対照実験**として測る (req/98 §3-2 / §6-6, 追加裁定 a = `req/38` §56)。
//!
//! # この file が先に在る理由 (RED-first の bench 版)
//!
//! req/98 §6-6 は 対照実験を req/95 §1 の template で要求する——「同一機械・同一計器 file・連続実行・
//! **各 arm の commit を記録**・median+回数+分母」。実装を先に書いてから計器を書けば、計器は答えを
//! 知った後で条件を選べる。∴ **判定条件を先に書き、arm A(実装前)で走らせて赤を見る**のが本 file の
//! 順序であり、その赤は req/103 §2 に実測として載っている。
//!
//! ## 判定条件 (numbers の前に書かれた物)
//!
//! ```text
//! SUBLINEAR := median(n = 64_000) / median(n = 8_000) < 2.0
//! ```
//!
//! 8 倍の `n` に対する倍率である。**線形なら 8 前後**(req/97 §3.1 の実測: 1,000→64,000 で 64 倍の
//! `n` に対し 68 倍の時間)、**案 A なら 1 に近い**——tile cache は完成した 256-leaf block の root を
//! 持つので、1 proof の hash 数は `O(n/256 + 256·log(n/256))` になる。閾値 2.0 は「線形の 8」と
//! 「案 A の見積り 1.2」の**間**に置いた値であり、どちらの側に落ちるかだけを決める。M5H7-6 の
//! 「gate に使うなら median か p90」に従い、**判定は median**で行う。p99 は記録のみ。
//!
//! ## 🔴 wire 不変を、2 build にまたがって **byte で**測る
//!
//! 追加裁定 a は 「`InclusionProof` の wire 形は変えない」 を要求する。同一 build 内の probe
//! (`tests/incremental_inclusion.rs`) は独立 oracle との一致を測るが、それは 1 つの build の中の話
//! である。この計器は生成した **全 proof の canonical DAG-CBOR を順に BLAKE3 へ通した fingerprint**
//! を印字する: arm A と arm B が同じ fingerprint を印字したなら、2 つの build が出した proof は
//! **1 byte も違わない**。2 build を跨ぐ比較で言える最も強い形であり、`verify_inclusion_of` や第三者
//! 検証器を 1 行も変えずに通る、の実測でもある。
//!
//! # 分母
//!
//! `cargo bench` の `--bench` が付いた時だけ測る (support の `measuring()` と同型)。`cargo test` は
//! 「この file がまだ build する」だけを確かめる——AC-064 の file が記録している 23 倍の
//! unoptimised profile を、測定として読ませないため。

use std::hint::black_box;
use std::time::{Duration, Instant};

use gx_core::{Cid, InclusionProof, Timestamp, TransformationId};
use gx_log::proof::prove_inclusion_at;
use gx_log::tile::TileLog;

/// req/97 §3.1 の表と同じ刻み。arm A の数字がその表と比較できるようにするため。
const SIZES: [u64; 7] = [1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 64_000];

/// 1 つの size につき何本の proof を取るか。中央値の分母であり、req/97 の 5 本より多い。
const PROOFS_PER_SIZE: usize = 21;

/// 判定に使う 2 点。`n` が 8 倍。
const SMALL: u64 = 8_000;
const LARGE: u64 = 64_000;

/// 「sublinear である」の閾値。線形なら 8 前後、案 A なら 1 前後。
const SUBLINEAR_RATIO: f64 = 2.0;

/// 🔴 **§62 R-7**: 判定を **exit code** にする(結線先=`tools/ci.sh` stage 10・既定 off)。
///
/// R-7 逐語(req/103 §9): 「**案 A の「速さ」を守っている test は 1 本も無い**…`audit_path_at` を旧
/// O(n) 実装へ**書き戻す**変異は全部緑になる(答えが同じだから)。速さを守っているのは
/// `benches/inclusion_proof.rs` の判定条件だけで、それは `tools/ci.sh` にも `tools/e2e.sh` にも
/// **結線されていない**」。裁定は「stage 10 の `GLOVREX_CI_BENCH=1` 側へ閾値付きで結線・既定 on に
/// はしない」であり、`stage` は command の RC を読む——∴ 判定は印字ではなく **RC** でなければ
/// 結線しても何も止まらない。
///
/// budget は env で動かせる。`GLOVREX_BENCH_SECONDS` が stage 10c で在る理由と同じで、**動かした
/// 値は数字の隣に印字される**(`BUDGET_SOURCE`)——緩めた走行と宣言どおりの走行が同じに読めては
/// ならない(req/29 §4)。end-to-end の結線確認(不可能な budget を渡して非 0 を観測する)は
/// `tools/verify_m7h6.sh 4` が 1 度だけ測る。
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
/// req/97 §3.1 measured 先頭/中央/末尾 and found no difference, which was itself the finding (the
/// walk is over the whole tree rather than over the path). Keeping the spread means arm B can be
/// read against that: a cache that helped only the leftmost leaf would show up as a spread.
fn indices(n: u64) -> Vec<u64> {
    (0..PROOFS_PER_SIZE)
        .map(|k| (n - 1).saturating_mul(k as u64) / (PROOFS_PER_SIZE as u64 - 1))
        .collect()
}

/// Nearest-rank percentiles with the sample count beside them (M3-15 の 「median+回数+分母」).
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
         judgement=「median({LARGE})/median({SMALL}) < {SUBLINEAR_RATIO}」 \
         (M5H7-6: 判定は median・p99 は記録のみ)"
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
         — 線形なら 8 前後 (n が 8 倍), 案 A なら 1 前後"
    );
    // 🔴 Minted through gx-canon, because 41 §6 admits one road to a canonical encoding and a bench
    // that reached for a hasher directly would be the second. The `Leaf` domain is a namespace
    // choice with exactly one consumer — a human comparing two logs — and the value is **not** a
    // ledger leaf; what it is, is 「the canonical form of every proof this run produced, in order」.
    let fingerprint =
        gx_canon::cid::mint_leaf(&produced).expect("a vector of proofs has a canonical form");
    println!(
        "INCLUSION_FINGERPRINT={} proofs={}  (全 proof の canonical DAG-CBOR を 1 本に。\
         2 arm が同値なら wire は 1 byte も動いていない)",
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
             O(n) が戻った形。守っているのはこの 1 行だけである(req/103 §9 R-7)"
        );
        std::process::exit(1);
    }
}
