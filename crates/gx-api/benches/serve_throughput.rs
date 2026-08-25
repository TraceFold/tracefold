// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-07, adopted (b)** (sem: SEM-gx-api-023) — AC-066 measured **again, through the HTTP surface**, so that the decay hand 7
//! is asked to fix is a thing this hand has seen rather than a thing a previous report described.
//!
//! > **recommendation = (b)** — the order is what makes the claim: **hand 7 measures AC-066 via serve first, reproduces the decay,
//! > then adds the index and re-measures** (sem: SEM-gx-api-024). This lets "the index worked" be said with a control.
//!
//! # What is different from `gx-engine/benches/throughput.rs`
//!
//! M5 hand 7's load generator drives `Engine`'s eight entry points **in process, with no lock and no
//! router**. This one drives 44 §2.2's three write endpoints through `gx_api::router` — the same
//! service stack 51 §7 names ("an axum test client, equivalent to `tower::ServiceExt`"; sem: SEM-gx-api-025) — over the
//! `Arc<Mutex<Engine>>` that **M6-06, adopted (a)**, made the v0.1 lifetime model. Three requests per
//! transformation rather than five calls:
//!
//! | request | 44 §2.2 | what the engine does |
//! |---|---|---|
//! | `POST /v1/candidates` | "submit+plan run atomically as one" (sem: SEM-gx-api-026) | T-1 and T-2 under one hold |
//! | `POST /v1/candidates/{id}/verify` | T-3, T-4a | **`conflicting_predecessor` is called here** |
//! | `POST /v1/candidates/{id}/commit` | T-8..T-11 | canonicalise and the critical section |
//!
//! So this measures what the engine bench measures **plus** the lock, the routing, the Bearer layer,
//! the extractors, JSON encode/decode on both sides and the idempotency store's miss path — and it
//! is the number a deployment of 47 §1(a) would actually see, which the in-process one is not.
//!
//! What it still is **not**: a socket. `tests/ac_056.rs` measures over a real socket, deliberately,
//! and repeating that here would put hyper's read path into a number about the engine. The fidelity
//! gap is named rather than left for a reader to assume (the same disclosure
//! `gx-engine/benches/throughput.rs` makes about the CLI it could not drive).
//!
//! # 🔴 The control, and why it is two builds
//!
//! §47 fixed the **order**: "add it only after decay has been reproduced" (sem: SEM-gx-api-027). A subject index is a change to
//! `crates/gx-engine/src/pipeline.rs`, so the two arms cannot come out of one binary the way M5 hand
//! 7's shard experiment did (that arm was a fixture knob, `GLOVREX_BENCH_SHARDS`, and this one is
//! not). Making it a runtime knob would ship a switch whose only consumer is a benchmark, which is a
//! worse answer than naming the limitation: **the two runs are two builds, same machine, same
//! instrument file, back to back, and the commit of each is recorded in req/95.**
//!
//! What keeps that honest is that the *fixture* is identical in both, and the fixture is this file:
//! same subjects, same policy pack, same key, same bucket width, same seconds.
//!
//! # M3-15 / M5H7-6
//!
//! median + count + denominator. Nothing here is compared with 33 NFR-003's 100 commits/s; that
//! number is printed as a budget beside the measurement. p99 is recorded, never a gate.
//!
//! # 🔴 Per-bucket, because "sustained" (sem: SEM-gx-api-028) is the claim (and because the slope *is* the finding)
//!
//! A sixty-second mean hides a line. `Engine::conflicting_predecessor` walks the whole state table
//! (43 §8's conflict check), the table only grows, and M5 hand 8 identified the decay by matching an
//! n-ratio of 4.95x against a measured verify-cost ratio of 5.08x. So the run is bucketed by ten
//! seconds, and **the per-stage p50 of the verify bucket is the number the index is supposed to
//! flatten**. A bench that printed one rate could tell you that something got slower and never which
//! of the three requests it was.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::Request;
use criterion::Criterion;
use gx_api::auth::Bearer;
use gx_api::state::{AppState, RequestEvidence, ServerKeys};
use gx_engine::Engine;
use gx_witness::KeyPair;
use tower::ServiceExt;

/// 34 AC-066: "a sixty-second sustained-load test" (sem: SEM-gx-api-029).
const DEFAULT_SECONDS: u64 = 60;

/// The bucket width, the same ten seconds `gx-engine/benches/throughput.rs` uses, so that the two
/// tables can be read side by side.
const BUCKET: Duration = Duration::from_secs(10);

/// The three requests one lifecycle makes, named so a bucket can be read per stage.
const STAGES: [&str; 3] = ["create", "verify", "commit"];

/// The token the fixture server holds. A literal: the load generator is not measuring auth failures.
const TOKEN: &str = "m6h7-bench-token";

/// One attempt: how long each request took, and whether the lifecycle reached `Committed`.
struct Attempt {
    took: Duration,
    stages: [Duration; STAGES.len()],
    committed: bool,
}

/// 45 §1's two keys, as a fixture. The ruler is never used here (no escalation is driven), but
/// `ServerKeys` is one trait and a fixture that implemented half of it would not compile.
struct Keys {
    signing: KeyPair,
}

impl ServerKeys for Keys {
    fn signing(&self) -> &KeyPair {
        &self.signing
    }

    fn ruler(&self, _key_id: &str) -> Option<&KeyPair> {
        None
    }
}

/// A directory on the fastest filesystem this machine offers, cleared on entry.
///
/// `/dev/shm` when it is there — 33 NFR-002's measurement column (sem: SEM-gx-api-030) says tmpfs and AC-065's fixture proves the
/// mount from `/proc/self/mountinfo`; the filesystem actually used is printed with the numbers, so a
/// run that fell back to a disk cannot be read as a run that did not.
fn sandbox(name: &str) -> PathBuf {
    let base = if Path::new("/dev/shm").is_dir() {
        PathBuf::from("/dev/shm/gx-bench")
    } else {
        std::env::temp_dir().join("gx-bench")
    };
    let dir = base.join(name);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    std::fs::create_dir_all(&dir).expect("create the sandbox");
    dir
}

/// The filesystem mounted over `path`, read from `/proc/self/mountinfo` (the longest matching mount
/// point wins, which is what makes `/dev/shm` beat `/`).
fn filesystem_of(path: &Path) -> String {
    let target = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return "unknown".to_string();
    };
    let mut best = ("".to_string(), "unknown".to_string());
    for line in mountinfo.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let Some(point) = left.split_whitespace().nth(4) else {
            continue;
        };
        let Some(kind) = right.split_whitespace().next() else {
            continue;
        };
        if target.starts_with(point) && point.len() >= best.0.len() {
            best = (point.to_string(), kind.to_string());
        }
    }
    best.1
}

/// The server under load: a project, an engine with the fs adapter, and a router over both.
struct Server {
    root: PathBuf,
    state: AppState,
}

impl Server {
    fn new(name: &str) -> Self {
        let root = sandbox(name);
        let project = root.join("project");
        std::fs::create_dir_all(&project).expect("create the project");
        let journal = project.join(".gx").join("ledger").join("journal");
        std::fs::create_dir_all(journal.parent().expect("has a parent"))
            .expect("create .gx/ledger");

        let evidence = RequestEvidence::new();
        let gate =
            gx_gate::Gate::with_policies(gx_gate::packs::fs_pack().expect("the shipped pack"));
        let mut engine = Engine::open(&journal, gate, evidence.clone()).expect("open the engine");
        engine.register_adapter(
            Arc::new(gx_adapter_fs::FsAdapter::new()),
            "gx-api bench fixture",
        );
        let keys: Arc<dyn ServerKeys> = Arc::new(Keys {
            signing: KeyPair::from_seed("bench-engine-key", &[7u8; 32]),
        });
        let state = AppState::new(
            engine,
            evidence,
            keys,
            Bearer::new(TOKEN),
            project.join(".gx").join("index"),
            None,
        )
        .expect("the fixture state builds");
        Self { root, state }
    }

    /// The subject file for attempt `n`. One directory, on purpose: **M5H7-3, adopted (b)**'s control arm (sem: SEM-gx-api-031)
    /// found the shard question to be about the instrument, and this hand is asking about the engine,
    /// so the generator is kept in the shape whose behaviour is already known.
    fn subject(&self, n: usize) -> PathBuf {
        let path = self.root.join("subjects").join(format!("subject-{n}"));
        std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create subjects/");
        std::fs::write(&path, b"before").expect("write the subject");
        path
    }

    /// One request through the whole service stack (51 §7's client, without a socket).
    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {TOKEN}"));
        let request = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                builder
                    .body(Body::from(serde_json::to_vec(&value).expect("serialises")))
                    .expect("a request")
            }
            None => builder.body(Body::empty()).expect("a request"),
        };
        let response = gx_api::router(self.state.clone())
            .oneshot(request)
            .await
            .expect("the router answers");
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .expect("the body is read");
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }
}

/// One lifecycle over HTTP. Every refusal is an error rather than a panic: 34 asks for an error
/// **rate**, and a generator that stopped on the first refusal would report zero for a broken
/// deployment (req/29 §4).
async fn one(server: &Server, n: usize) -> Attempt {
    let subject = server.subject(n);
    let body = serde_json::json!({
        "substrate": "fs",
        "locator": subject.display().to_string(),
        "goal": format!("after-{n}"),
        "context": "Evidence",
        "actor": { "Agent": { "key": "bench-actor-key", "model": "bench" } },
    });

    let mut stages = [Duration::ZERO; STAGES.len()];
    let started = Instant::now();
    let mut mark = Instant::now();

    let (status, created) = server.send("POST", "/v1/candidates", Some(body)).await;
    stages[0] = mark.elapsed();
    mark = Instant::now();
    if status != 201 && status != 200 {
        return Attempt {
            took: started.elapsed(),
            stages,
            committed: false,
        };
    }
    let id = created["transformation"]["id"]
        .as_str()
        .or_else(|| created["id"].as_str())
        .unwrap_or_default()
        .to_string();

    let (status, _) = server
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    stages[1] = mark.elapsed();
    mark = Instant::now();
    if status != 200 {
        return Attempt {
            took: started.elapsed(),
            stages,
            committed: false,
        };
    }

    let (status, committed) = server
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    stages[2] = mark.elapsed();
    let ok = status == 200 && committed["state"].as_str() == Some("Committed");
    Attempt {
        took: started.elapsed(),
        stages,
        committed: ok,
    }
}

/// Nearest-rank percentiles with the sample count beside them (M3-15's "median + count + denominator"; sem: SEM-gx-api-032).
fn report(tag: &str, name: &str, samples: &mut [Duration]) {
    assert!(!samples.is_empty(), "a distribution needs samples");
    samples.sort_unstable();
    let n = samples.len();
    let at = |q: f64| -> Duration {
        let rank = ((q * n as f64).ceil() as usize).clamp(1, n);
        samples[rank - 1]
    };
    println!(
        "{tag} {name}: n={n} p50={:?} p90={:?} p99={:?} max={:?}",
        at(0.50),
        at(0.90),
        at(0.99),
        samples[n - 1]
    );
}

/// `--bench` separates a measurement from `cargo test`'s check that this file still builds.
fn measuring() -> bool {
    std::env::args().any(|a| a == "--bench")
}

fn sustained_load() {
    let seconds: u64 = std::env::var("GLOVREX_BENCH_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SECONDS);
    let server = Server::new("ac066-serve");
    println!(
        "AC066_SERVE_GENERATOR seconds={seconds} bucket={BUCKET:?} fs={} stack=router(oneshot) \
         lock=Arc<Mutex<Engine>> (M6-06, adopted (a); sem: SEM-gx-api-033); the socket is deliberately outside this -- \
         `crates/gx-cli/tests/ac_056.rs` measures that",
        filesystem_of(&server.root)
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime");

    let started = Instant::now();
    let budget = Duration::from_secs(seconds);
    let mut attempts: Vec<Attempt> = Vec::new();
    let mut bucket_of: Vec<usize> = Vec::new();
    let mut n = 0_usize;
    while started.elapsed() < budget {
        let bucket = (started.elapsed().as_secs_f64() / BUCKET.as_secs_f64()) as usize;
        let attempt = runtime.block_on(one(&server, n));
        black_box(attempt.committed);
        attempts.push(attempt);
        bucket_of.push(bucket);
        n += 1;
    }
    let elapsed = started.elapsed();

    let errors = attempts.iter().filter(|a| !a.committed).count();
    let mut all: Vec<Duration> = attempts.iter().map(|a| a.took).collect();
    println!(
        "AC066_SERVE_TOTAL attempts={} committed={} errors={errors} error_rate={:.6} \
         elapsed={:.3?} throughput={:.2} commits/s",
        attempts.len(),
        attempts.len() - errors,
        errors as f64 / attempts.len() as f64,
        elapsed,
        (attempts.len() - errors) as f64 / elapsed.as_secs_f64(),
    );
    report("AC066_SERVE_LATENCY", "whole-run", &mut all);

    let buckets = bucket_of.iter().copied().max().unwrap_or(0) + 1;
    for b in 0..buckets {
        let mut took: Vec<Duration> = attempts
            .iter()
            .zip(&bucket_of)
            .filter(|(_, bucket)| **bucket == b)
            .map(|(a, _)| a.took)
            .collect();
        let failed = attempts
            .iter()
            .zip(&bucket_of)
            .filter(|(a, bucket)| **bucket == b && !a.committed)
            .count();
        if took.is_empty() {
            continue;
        }
        let width = BUCKET
            .as_secs_f64()
            .min(elapsed.as_secs_f64() - b as f64 * BUCKET.as_secs_f64());
        println!(
            "AC066_SERVE_BUCKET_{b} commits={} errors={failed} throughput={:.2} commits/s",
            took.len(),
            (took.len() - failed) as f64 / width,
        );
        report(
            "AC066_SERVE_BUCKET",
            &format!("seconds {}..{}", b * 10, b * 10 + 10),
            &mut took,
        );
        for (i, stage) in STAGES.iter().enumerate() {
            let mut per: Vec<Duration> = attempts
                .iter()
                .zip(&bucket_of)
                .filter(|(_, bucket)| **bucket == b)
                .map(|(a, _)| a.stages[i])
                .collect();
            report("AC066_SERVE_STAGE", &format!("{b}:{stage}"), &mut per);
        }
    }

    println!("AC066_SERVE_FINAL_TABLE transformations={n}");
    println!(
        "AC066_SERVE_BUDGET 100 commits/s is 33's provisional NFR-003 value and is a design budget, \
         not a pass mark (M3-15). Nothing above is compared against it."
    );
}

/// criterion's view of one lifecycle over HTTP, for 51 §9's regression detection.
fn bench_one(c: &mut Criterion) {
    let server = Server::new("ac066-serve-criterion");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime");
    let mut n = 0_usize;
    let mut group = c.benchmark_group("serve_throughput");
    group.bench_function("one_commit_over_http", |b| {
        b.iter(|| {
            n += 1;
            black_box(runtime.block_on(one(&server, n)).committed)
        });
    });
    group.finish();
}

fn main() {
    if measuring() {
        sustained_load();
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_one(&mut criterion);
    criterion.final_summary();
}
