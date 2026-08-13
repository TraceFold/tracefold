//! 🔴 **AC-067 / NFR-004** — what the proxy **adds** to a passthrough tool call, measured against the
//! same effect caused without it.
//!
//! 34 AC-067 逐語: 「Given: `gx-adapter-mcp`プロキシ経由のnon-gated（passthrough）tool-call呼び出しと
//! direct呼び出し。When: 両者のレイテンシを比較ベンチマークする。Then: プロキシが追加するレイテンシが
//! **p99 ≤ 30ms**。」 裁定 #15 (`req/38` §56) fixes the environment: **WSL native worktree を正**, docker は
//! 参考。req/98 §6-8 fixes the form: median + 回数 + 分母, and 単発値を閾値と比べない。
//!
//! # 判定条件 (numbers の前に書かれた物・RED-first の bench 版)
//!
//! ```text
//! ADDED(i)  := proxy(i) - direct(i)        (同一 iteration の対・対応のある差)
//! AC-067    := p99( ADDED ) <= 30 ms
//! ```
//!
//! **対応のある差**を取るのは、2 つの分布の p99 を引き算しても「proxy が追加した物」にはならないから
//! である (p99 同士の差は、どの 1 回についても成り立たない数字になりうる)。同一 iteration で direct と
//! proxy を交互に走らせ、その差の分布を報告する。
//!
//! 🔴 **p99 を gate に使うのは、AC-067 がそう書いてあるから**である。M5H7-6 の「gate に使うなら median
//! か p90・p99 は記録のみ」は 33 の**暫定値** (NFR-003 の 100 commits/s) についての規律であり、34 の AC が
//! 逐語で p99 を書いている本項には効かない。∴ 三者とも印字し、**判定は p99**、median と p90 は隣に置く。
//!
//! # 二つの arm が何をしているか、そして何をしていないか
//!
//! | arm | 何が起きるか |
//! |---|---|
//! | **direct** | fixture server の resource へ、その tool 自身の効果を直接書く。`FakeServer` の `WRITE_TOOL` 分岐の本体そのもの——プロキシを経由せずに server へ届いた client が起こす変化。 |
//! | **proxy** | `Engine` の submit → plan → verify(gate=permit) → canonicalize → commit。gate 判定・canonical encode・ledger append・receipt 署名・journal 書き込みが全部この中に在り、tool call は commit の中で **1 回**だけ出る (AC-051 の D-5)。 |
//!
//! **どちらの arm も wire を持たない。** この crate は JSON-RPC を framing せず、fixture server は同一
//! process に居る (`src/lib.rs`「What v0.1 does not close」)。∴ socket・framing・server の実処理時間は
//! **両 arm に共通で、差では相殺される**。AC-067 が訊いているのは 「プロキシが**追加する**レイテンシ」
//! なので、共通項が両側に無い事は差にとって中立である——が、比率 (「何倍遅い」) は本計器から読めない。
//! 読めるのは**絶対値の追加分**だけであり、AC-067 が閾値を絶対値 (30ms) で書いているのはそのためだと
//! 読める。
//!
//! **fsync は測定環境の性質である。** journal と ledger は書き込みのたびに fsync する (NFR-009)。既定の
//! sandbox は tmpfs (`/dev/shm`) で、そこでの fsync はほぼ無料である。∴ `GLOVREX_BENCH_ROOT` を ext4 の
//! path へ向けた **第 2 の arm** を並べる (`tools/m7h5_bench.sh ac067_disk`)。tmpfs の数字だけを出せば
//! 「30ms を余裕で下回る」が filesystem についての主張になってしまう。
//!
//! # decay
//!
//! `Engine` は見た transformation を表に持ち、43 §8 の conflict 検査がその subject の兄弟を歩く
//! (M6-07 採(b) の subject 索引の後でも、兄弟数は増える)。この adapter の footprint は **server 全体**
//! なので、全 iteration が 1 つの subject に載る=兄弟数が iteration 数まで伸びる最悪形である。∴ bucket
//! ごとに印字し、**判定は全 bucket を含む全体の p99**で行う (最後の bucket が最も遅い側)。

#[path = "../tests/support/mod.rs"]
mod support;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gx_core::{Actor, ChangeContext, GoalBytes, Intent, SubstrateKind, Timestamp};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};

use support::{FakeServer, RewindableLog, SERVER, WRITE_TOOL};

/// 34 AC-067 の閾値。
const BUDGET: Duration = Duration::from_millis(30);

/// 🔴 **§62 R-7**: 判定を **exit code** にする(結線先=`tools/ci.sh` stage 10・既定 off)。
///
/// budget は `GLOVREX_AC067_BUDGET_MS` で動かせ、使った値と**出所**が数字の隣に印字される
/// (`BUDGET_SOURCE`)——緩めた走行と宣言どおりの走行が同じに読めてはならない(req/29 §4)。
fn budget() -> (Duration, String) {
    match std::env::var("GLOVREX_AC067_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        Some(ms) => (Duration::from_millis(ms), "GLOVREX_AC067_BUDGET_MS".into()),
        None => (BUDGET, "declared(34 AC-067)".into()),
    }
}

/// 🔴 **§62 R-1**: この判定が **gate になるのは journal が tmpfs に在る時だけ**である。
///
/// §62 逐語: 「journal/ledger が tmpfs に在る時、proxy の追加 p99 は budget 30ms の 2.6〜6.5%…
/// ext4(WSL2 VHD)では p99 163.8 ms=5.5 倍割る」「ext4 の数字は proxy の code でなく **fsync 15.1
/// 回 × 単価 5.9 ms** についての数字である事…を判定文の一部とする」。
///
/// ∴ ext4 の上でこの stage を赤にすれば、CI は 「proxy が遅い」 と読める形で **disk の性質**を
/// 報告する事になる。裁定が付けた条件を、それを消費する側にも同じ形で置く: filesystem が tmpfs
/// でなければ**測って記録し、判定はしない**——そして「判定しなかった」を出力に名前つきで出す。
/// 黙って緑になる skip は §30 の病そのものである。
fn gated_on(filesystem: &str) -> bool {
    matches!(filesystem, "tmpfs" | "ramfs")
}

/// 既定の iteration 数。`GLOVREX_BENCH_CALLS` で動かせる。
const DEFAULT_CALLS: usize = 400;

/// bucket 幅 (iteration 数)。decay を 1 本の線ではなく段で見るため。
const BUCKET: usize = 100;

/// A fixed instant: 41 §6 injects time at the engine boundary, and a bench that read a clock the
/// engine is supposed to be given would be measuring `clock_gettime`.
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// 何も拒まない pack。AC-067 の Given が 「non-gated（passthrough）」 だからであって、gate を測ら
/// ないためではない——gate の**判定そのもの**は proxy arm の中に在り、その cost は追加分に含まれる。
const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-mcp-bench", &[9u8; 32])
}

fn measuring() -> bool {
    std::env::args().any(|a| a == "--bench")
}

fn calls() -> usize {
    std::env::var("GLOVREX_BENCH_CALLS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CALLS)
}

/// The filesystem mounted over `path`, read from `/proc/self/mountinfo`.
///
/// Printed with the numbers: a run that fell back to a disk must not be readable as a run that did
/// not, and 裁定 #15's 「正は 1 つ」 is about exactly this kind of ambiguity.
fn filesystem_of(path: &Path) -> String {
    let target = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return "unknown".to_string();
    };
    let mut best = (String::new(), "unknown".to_string());
    for line in mountinfo.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let (Some(point), Some(kind)) = (
            left.split_whitespace().nth(4),
            right.split_whitespace().next(),
        ) else {
            continue;
        };
        if target.starts_with(point) && point.len() >= best.0.len() {
            best = (point.to_string(), kind.to_string());
        }
    }
    best.1
}

/// Where the journal goes. `/dev/shm` unless `GLOVREX_BENCH_ROOT` says otherwise — and the second
/// arm of this measurement is that variable pointed at an ext4 path.
fn sandbox() -> PathBuf {
    let root = PathBuf::from(
        std::env::var("GLOVREX_BENCH_ROOT").unwrap_or_else(|_| "/dev/shm".to_string()),
    );
    let dir = root.join(format!("gx-ac067-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the bench root accepts a directory");
    dir
}

/// The resource one iteration is about. A fresh URI per iteration on **one** server, which is this
/// adapter's worst case for the conflict walk and its only honest one (the fixture is one server).
fn resource(n: usize) -> String {
    format!("file:///srv/bench/notes-{n}.md")
}

fn intent_for(locator: &str, tool: &str, arguments: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Mcp,
        locator.to_string(),
        GoalBytes(
            gx_adapter_mcp::ToolIntent::new(tool, arguments.to_vec())
                .encode()
                .expect("a tool call has a canonical form"),
        ),
        ChangeContext::Policy,
        Actor::Agent {
            key: "key-agent-bench".to_string(),
            model: "bench".to_string(),
        },
    )
}

/// Nearest-rank percentiles with the sample count beside them (req/98 §6-8's 「median+回数+分母」).
fn report(tag: &str, name: &str, samples: &mut [Duration]) -> (Duration, Duration, Duration) {
    assert!(!samples.is_empty(), "a distribution needs samples");
    samples.sort_unstable();
    let n = samples.len();
    let at = |q: f64| -> Duration {
        let rank = ((q * n as f64).ceil() as usize).clamp(1, n);
        samples[rank - 1]
    };
    println!(
        "{tag} {name:<20} n={n} min={:>10.3?} p50={:>10.3?} p90={:>10.3?} p99={:>10.3?} max={:>10.3?}",
        samples[0],
        at(0.50),
        at(0.90),
        at(0.99),
        samples[n - 1],
    );
    (at(0.50), at(0.90), at(0.99))
}

fn main() {
    if !measuring() {
        // `cargo test`'s check that this file still builds and still reaches both arms, on two
        // iterations. A measurement is `cargo bench` (AC-064's file records the 23x unoptimised
        // profile that made this guard necessary).
        let (direct, proxy, _filesystem) = run(2);
        assert_eq!(direct.len(), 2);
        assert_eq!(proxy.len(), 2);
        println!("AC067_BENCH_BUILD_ONLY arms=2 iterations=2");
        return;
    }

    let n = calls();
    let (mut direct, mut proxy, filesystem) = run(n);
    let mut added: Vec<Duration> = proxy
        .iter()
        .zip(&direct)
        .map(|(p, d)| p.saturating_sub(*d))
        .collect();

    report("AC067_DIRECT", "whole run", &mut direct);
    report("AC067_PROXY", "whole run", &mut proxy);
    let (p50, p90, p99) = report("AC067_ADDED", "proxy - direct", &mut added);

    // decay を段で見る: bucket ごとの追加分。判定には使わない (判定は全体の p99)。
    let paired: Vec<Duration> = proxy
        .iter()
        .zip(&direct)
        .map(|(p, d)| p.saturating_sub(*d))
        .collect();
    for (b, chunk) in paired.chunks(BUCKET).enumerate() {
        let mut per = chunk.to_vec();
        report(
            "AC067_ADDED_BUCKET",
            &format!("{}..{}", b * BUCKET, b * BUCKET + chunk.len()),
            &mut per,
        );
    }

    let (budget, budget_source) = budget();
    let pass = p99 <= budget;
    let gated = gated_on(&filesystem);
    println!(
        "AC067_VERDICT p99_added={p99:.3?} budget={budget:?} BUDGET_SOURCE={budget_source} \
         pass={pass} gated={gated} journal_fs={filesystem} p50_added={p50:.3?} \
         p90_added={p90:.3?} n={n}  (対応のある差の分布・34 AC-067 は p99 を逐語で書く)"
    );

    // 🔴 §62 R-1 の条件を、それを消費する側でも同じ形に保つ。
    if !gated {
        println!(
            "AC067_NOT_GATED journal_fs={filesystem} — 記録のみ。§62 R-1: ext4 の数字は proxy の \
             code についてではなく fsync 15.1 回 × 単価 5.9 ms についての数字であり、それで CI を \
             赤にすれば disk の性質を proxy の性質として報告する事になる"
        );
        return;
    }
    if !pass {
        eprintln!(
            "AC067_FAIL p99_added={p99:.3?} > budget={budget:?} ({budget_source}) on \
             journal_fs={filesystem}"
        );
        std::process::exit(1);
    }
}

/// One run of both arms, interleaved, with the order alternating so that neither arm is always the
/// one that pays for a cold cache line.
fn run(n: usize) -> (Vec<Duration>, Vec<Duration>, String) {
    let dir = sandbox();
    let filesystem = filesystem_of(&dir);
    let server = Arc::new(FakeServer::new());
    let adapter = gx_adapter_mcp::McpAdapter::new(server.clone())
        .with_catalogue(support::catalogue())
        .with_log(Arc::new(RewindableLog::new()));
    let gate = gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(PERMIT_ALL).expect("the fixture policy set parses"),
    );
    let mut engine = Engine::open(dir.join("journal.bin"), gate, InjectedEvidence::none())
        .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(adapter), "gx-adapter-mcp ac067 bench");

    if measuring() {
        println!(
            "AC067_GENERATOR calls={n} bucket={BUCKET} journal_fs={} journal={} server=in-process \
             (no wire on either arm: framing と socket は両 arm に無く、差では相殺される)",
            filesystem_of(&dir),
            dir.display()
        );
    }

    let mut direct = Vec::with_capacity(n);
    let mut proxy = Vec::with_capacity(n);
    let key = signing_key();

    for i in 0..n {
        // Two resources per iteration so that the two arms never touch one URI: an arm that read
        // back what the other wrote would be measuring the fixture's map, not the road.
        let direct_uri = resource(2 * i);
        let proxy_uri = resource(2 * i + 1);
        server.write_behind_the_adapter(&direct_uri, b"before\n");
        server.write_behind_the_adapter(&proxy_uri, b"before\n");
        let goal = format!("after-{i}\n").into_bytes();

        let time_direct = || {
            let started = Instant::now();
            // The body of `FakeServer`'s WRITE_TOOL arm: what a client that reached the server
            // without a proxy causes. No counter, no admission — that is the point of the arm.
            server.write_behind_the_adapter(&direct_uri, &goal);
            let took = started.elapsed();
            black_box(server.contents(&direct_uri));
            took
        };

        let mut time_proxy = || {
            let locator = format!("{SERVER}#{proxy_uri}");
            let intent = intent_for(&locator, WRITE_TOOL, &goal);
            let started = Instant::now();
            engine.submit(&intent, i as u64, AT).expect("submit");
            let id = engine.plan(&intent, AT).expect("plan");
            engine.verify(&id, AT, &key, None).expect("verify");
            engine.canonicalize(&id, AT, None).expect("canonicalize");
            let state = engine.commit(&id, AT, &key).expect("commit");
            let took = started.elapsed();
            assert_eq!(state, Lifecycle::Committed, "the pack admits everything");
            took
        };

        if i % 2 == 0 {
            direct.push(time_direct());
            proxy.push(time_proxy());
        } else {
            proxy.push(time_proxy());
            direct.push(time_direct());
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    (direct, proxy, filesystem)
}
