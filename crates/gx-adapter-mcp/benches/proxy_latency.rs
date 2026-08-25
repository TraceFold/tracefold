// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **AC-067 / NFR-004** — what the proxy **adds** to a passthrough tool call, measured against the
//! same effect caused without it.
//!
//! 34 AC-067, verbatim: "Given: a non-gated (passthrough) tool-call invocation via the `gx-adapter-mcp` proxy, and (sem: SEM-gx-adapter-mcp-226)
//! a direct invocation. When: benchmark-compare the latency of both. Then: the latency the proxy adds is (sem: SEM-gx-adapter-mcp-227)
//! **p99 <= 30ms**." Ruling #15 (`req/38` §56) fixes the environment: **the WSL native worktree is canonical**, docker is (sem: SEM-gx-adapter-mcp-228)
//! reference only. req/98 §6-8 fixes the form: median + count + denominator, and a single value is never compared against the threshold. (sem: SEM-gx-adapter-mcp-229)
//!
//! # The judgment condition (written before the numbers -- the bench version of RED-first) (sem: SEM-gx-adapter-mcp-230)
//!
//! ```text
//! ADDED(i)  := proxy(i) - direct(i)        (a paired difference within the same iteration) (sem: SEM-gx-adapter-mcp-231)
//! AC-067    := p99( ADDED ) <= 30 ms
//! ```
//!
//! Taking the **paired difference** is because subtracting the p99 of two distributions does not give "what the proxy added" -- (sem: SEM-gx-adapter-mcp-232)
//! (a difference between two p99 values can be a number that holds for no single run). direct and (sem: SEM-gx-adapter-mcp-233)
//! proxy are run alternately within one iteration, and the distribution of their difference is reported. (sem: SEM-gx-adapter-mcp-234)
//!
//! 🔴 **p99 is used as the gate because AC-067 says so, in as many words.** M5H7-6's "if used as a gate, median (sem: SEM-gx-adapter-mcp-235)
//! or p90; p99 is record-only" is a discipline about 33's **provisional number** (NFR-003's 100 commits/s), and does not apply to (sem: SEM-gx-adapter-mcp-236)
//! this section, where 34's AC writes p99 verbatim. ∴ all three are printed, **the judgment is p99**, and median and p90 are placed beside it. (sem: SEM-gx-adapter-mcp-237)
//!
//! # What the two arms do, and what they do not (sem: SEM-gx-adapter-mcp-238)
//!
//! | arm | what happens | (sem: SEM-gx-adapter-mcp-239)
//! |---|---|
//! | **direct** | writes the tool's own effect directly to the fixture server's resource. The body of `FakeServer`'s `WRITE_TOOL` branch itself -- the change a client reaching the server without going through the proxy would cause. | (sem: SEM-gx-adapter-mcp-240)
//! | **proxy** | `Engine`'s submit -> plan -> verify(gate=permit) -> canonicalize -> commit. The gate verdict, canonical encode, ledger append, receipt signing and journal write are all inside this, and the tool call comes out **exactly once**, inside the commit (AC-051's D-5). | (sem: SEM-gx-adapter-mcp-241)
//!
//! **Neither arm has a wire.** This crate frames no JSON-RPC, and the fixture server is in the same (sem: SEM-gx-adapter-mcp-242)
//! process (`src/lib.rs`, "What v0.1 does not close"). ∴ the socket, framing and the server's own processing time are (sem: SEM-gx-adapter-mcp-243)
//! **common to both arms and cancel out in the difference**. What AC-067 asks is "the latency the proxy **adds**" (sem: SEM-gx-adapter-mcp-244)
//! so a common term absent from both sides is neutral to the difference -- but a ratio ("how many times slower") cannot be read from this instrument. (sem: SEM-gx-adapter-mcp-245)
//! What can be read is only **the absolute added amount**, which is presumably why AC-067 writes its threshold as an absolute value (30ms) -- (sem: SEM-gx-adapter-mcp-246)
//! that is the reading. (sem: SEM-gx-adapter-mcp-247)
//!
//! **fsync is a property of the measurement environment.** The journal and ledger fsync on every write (NFR-009). The default (sem: SEM-gx-adapter-mcp-248)
//! sandbox is tmpfs (`/dev/shm`), where fsync is nearly free. ∴ a **second arm** pointing `GLOVREX_BENCH_ROOT` at ext4 is placed (sem: SEM-gx-adapter-mcp-249)
//! alongside it (`tools/m7h5_bench.sh ac067_disk`). Reporting only the tmpfs number would turn (sem: SEM-gx-adapter-mcp-250)
//! "comfortably under 30ms" into a claim about the filesystem. (sem: SEM-gx-adapter-mcp-251)
//!
//! # decay
//!
//! `Engine` keeps a table of the transformations it has seen, and 43 §8's conflict check walks that subject's siblings (sem: SEM-gx-adapter-mcp-252)
//! (even after M6-07, adopted (b)'s subject index, the sibling count still grows). This adapter's footprint is the **whole server** (sem: SEM-gx-adapter-mcp-253)
//! so every iteration lands on one subject -- the worst shape, where the sibling count grows to the iteration count. ∴ (sem: SEM-gx-adapter-mcp-254)
//! print per bucket, and **judge on the overall p99 across every bucket** (the last bucket is the slower side). (sem: SEM-gx-adapter-mcp-255)

#[path = "../tests/support/mod.rs"]
mod support;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gx_core::{Actor, ChangeContext, GoalBytes, Intent, SubstrateKind, Timestamp};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};

use support::{FakeServer, RewindableLog, SERVER, WRITE_TOOL};

/// 34 AC-067's threshold. (sem: SEM-gx-adapter-mcp-256)
const BUDGET: Duration = Duration::from_millis(30);

/// 🔴 **§62 R-7**: the verdict becomes an **exit code** (wired to `tools/ci.sh` stage 10, default off). (sem: SEM-gx-adapter-mcp-257)
///
/// The budget can be moved by `GLOVREX_AC067_BUDGET_MS`, and the value used and **its source** are printed beside the number (sem: SEM-gx-adapter-mcp-258)
/// (`BUDGET_SOURCE`) -- a loosened run and a run at the declared value must never read the same (req/29 §4). (sem: SEM-gx-adapter-mcp-259)
fn budget() -> (Duration, String) {
    match std::env::var("GLOVREX_AC067_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        Some(ms) => (Duration::from_millis(ms), "GLOVREX_AC067_BUDGET_MS".into()),
        None => (BUDGET, "declared(34 AC-067)".into()),
    }
}

/// 🔴 **§62 R-1**: this verdict **becomes a gate only when the journal lives on tmpfs**. (sem: SEM-gx-adapter-mcp-260)
///
/// §62, verbatim: "when the journal/ledger live on tmpfs, the proxy's added p99 is 2.6-6.5% of the 30ms budget... (sem: SEM-gx-adapter-mcp-261)
/// on ext4 (WSL2 VHD) p99 is 163.8ms, 5.5x over" "the ext4 number is not about the proxy's code but about **15.1 fsync (sem: SEM-gx-adapter-mcp-262)
/// calls at 5.9ms each**... and this fact is made part of the judgment sentence itself". (sem: SEM-gx-adapter-mcp-263)
///
/// ∴ turning this stage red on ext4 would make CI report **a disk property** in a shape that reads as "the proxy is slow". (sem: SEM-gx-adapter-mcp-264)
/// The condition the ruling attached is placed the same way on the consuming side too: when the filesystem is not tmpfs, (sem: SEM-gx-adapter-mcp-265)
/// **measure and record, but do not judge** -- and print "no judgment made" in the output, by name. (sem: SEM-gx-adapter-mcp-266)
/// A skip that silently turns green is precisely §30's disease. (sem: SEM-gx-adapter-mcp-267)
fn gated_on(filesystem: &str) -> bool {
    matches!(filesystem, "tmpfs" | "ramfs")
}

/// The default iteration count. Can be moved by `GLOVREX_BENCH_CALLS`. (sem: SEM-gx-adapter-mcp-268)
const DEFAULT_CALLS: usize = 400;

/// The bucket width (iteration count). So decay is seen in steps rather than as one line. (sem: SEM-gx-adapter-mcp-269)
const BUCKET: usize = 100;

/// A fixed instant: 41 §6 injects time at the engine boundary, and a bench that read a clock the
/// engine is supposed to be given would be measuring `clock_gettime`.
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// A pack that refuses nothing. Because AC-067's Given is "non-gated (passthrough)", not because the gate is not (sem: SEM-gx-adapter-mcp-270)
/// being measured -- the gate's **verdict itself** is inside the proxy arm, and its cost is part of the added amount. (sem: SEM-gx-adapter-mcp-271)
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
/// not, and ruling #15's "there is exactly one canonical value" is about exactly this kind of ambiguity. (sem: SEM-gx-adapter-mcp-272)
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

/// Nearest-rank percentiles with the sample count beside them (req/98 §6-8's "median + count + denominator"). (sem: SEM-gx-adapter-mcp-273)
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

    // Decay seen in steps: the added amount per bucket. Not used for judgment (the judgment is the overall p99). (sem: SEM-gx-adapter-mcp-274)
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
         p90_added={p90:.3?} n={n}  (the distribution of a paired difference; 34 AC-067 writes p99 verbatim) (sem: SEM-gx-adapter-mcp-275)"
    );

    // 🔴 §62 R-1's condition is kept the same shape on the consuming side too. (sem: SEM-gx-adapter-mcp-276)
    if !gated {
        println!(
            "AC067_NOT_GATED journal_fs={filesystem} -- record only. §62 R-1: the ext4 number is not about the proxy's (sem: SEM-gx-adapter-mcp-277) \
             code, but about 15.1 fsync calls at 5.9ms each, and turning CI red on it (sem: SEM-gx-adapter-mcp-278) \
             would report a disk property as a proxy property (sem: SEM-gx-adapter-mcp-279)"
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
             (no wire on either arm: neither has framing or a socket, and they cancel out in the difference) (sem: SEM-gx-adapter-mcp-280)",
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
