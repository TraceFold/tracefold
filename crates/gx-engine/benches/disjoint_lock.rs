//! 🔴 **裁定 #20** (`req/38` §56, req/98 §7-2 手 5 / §9 行 d-1) — **per-object lock は実装しない**。
//! D-7 の発火条件を維持したまま、**disjoint object 群での commits/sec を測る**だけ。
//!
//! > **20**: **D-7 発火条件を維持**し lock は実装しない・**disjoint object 群での commits/sec 測定を
//! > 手 5 に追加**
//!
//! 発火条件 (`req/38:930`): 「serve 経由 AC-066 が SLA を割った時」。SLA=33 NFR-003 の 100 commits/s
//! (暫定値)。∴ この file が答えるのは 1 つの問いである: **N 並行 agent が互いに素な object を触っても、
//! 1 本の `Mutex` で 100 commits/s を割るか**。
//!
//! # 三本の arm と、それぞれが何の上界か
//!
//! | arm | 形 | 何を答えるか |
//! |---|---|---|
//! | `single` | 1 thread・1 engine | 直列の基線 (AC-066 の in-process 版と同形) |
//! | `shared` | N thread・**1 つの `Arc<Mutex<Engine>>`** | **M6-06 採(a) が出荷した形**。D-7 の発火条件はこの数字に対して立つ |
//! | `disjoint` | N thread・**N 個の engine** (各々が自分の journal と ledger と lock を持つ) | per-object lock が買いうる物の **上界** |
//!
//! 🔴 **`disjoint` は per-object lock の実装ではなく、その上界である**。本物の per-object lock は
//! 「1 つの engine の中で、触る object が違えば待たない」形であり、**ledger と journal は 1 本のまま**
//! である (append-only log の writer は 1 つ)。この arm はその 2 つも分けてしまうので、per-object lock
//! が買える値は `shared` と `disjoint` の**間**に在る。上界を測るのは、上界が `shared` に近ければ
//! 「lock は問題ではない」が**下界を測らずに**言えるからで、逆は言えない——それも下に書く。
//!
//! # 待ち時間そのものを測る
//!
//! `shared` arm は 1 commit ごとに **lock を取るまでの時間**と**保持していた時間**を別々に記録する。
//! per-object lock が取り除けるのは前者だけである。∴ 「待ちが全体の何 % か」は、上界の arm を信じなく
//! ても読める唯一の直接測定であり、`LOCK_WAIT_SHARE` がそれである。
//!
//! # M3-15 / req/98 §6-8
//!
//! median + 回数 + 分母。100 commits/s は 33 の**暫定**値であり、この file はそれを pass mark として
//! 比較しない——**発火条件の判定として**印字する。それは裁定 #20 が明示的に頼んだ判定だからで、
//! 「割ったか否かが数字で答えられる」(E-M7-6 逐語) の形である。
//!
//! # 分母
//!
//! tmpfs (`/dev/shm`)。fsync がほぼ無料の場所なので、この数字は **CPU と lock についての数字**であって
//! disk についての数字ではない (support の module note と同じ申告)。

mod support;

use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gx_adapter_fs::FsAdapter;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate, intent_for, measuring, report, signing_key, Sandbox, AT};

/// 既定の並行 agent 数。`GLOVREX_BENCH_AGENTS` で動かせる。
const DEFAULT_AGENTS: usize = 4;

/// 1 agent あたりの commit 数。`GLOVREX_BENCH_COMMITS` で動かせる。
const DEFAULT_COMMITS: usize = 150;

/// 33 NFR-003 の暫定 SLA。D-7 の発火条件が参照する値。
const SLA_COMMITS_PER_SEC: f64 = 100.0;

/// 🔴 **§62 R-7**: この bench の判定を **exit code** にする(結線先=`tools/ci.sh` stage 10・既定 off)。
///
/// 🔴 **この stage が赤になる事は D-7 の発火ではない**。発火条件の逐語は「**serve 経由**の AC-066 が
/// SLA を割った時」(`req/38:930`)であり、本計器は in-process で HTTP・router・Bearer・JSON を含ま
/// ない(req/103 §4-1)。裁定 #20 は「lock は実装しない・測るだけ」で、それは動いていない。
///
/// ∴ ここでの閾値の身分は**回帰 gate**である: 実測は shared で 1,903〜2,163 commits/s(req/103 §4-2)
/// =SLA の 19〜21 倍で、100 を割る事は「並行 commit の道が 20 倍遅くなった」以外に説明が付かない。
/// 20 倍の余裕を持つ閾値は noisy な bench に対して安全に赤くできる唯一の種類の閾値であり、それが
/// この値を選んだ理由である(「発火条件の数字だから」ではない)。
///
/// `GLOVREX_DISJOINT_MIN_RATE` で動かせ、使った値と出所が数字の隣に印字される。
fn min_rate() -> (f64, String) {
    match std::env::var("GLOVREX_DISJOINT_MIN_RATE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        Some(overridden) => (overridden, "GLOVREX_DISJOINT_MIN_RATE".into()),
        None => (SLA_COMMITS_PER_SEC, "declared(33 NFR-003)".into()),
    }
}

fn agents() -> usize {
    std::env::var("GLOVREX_BENCH_AGENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_AGENTS)
}

fn commits() -> usize {
    std::env::var("GLOVREX_BENCH_COMMITS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_COMMITS)
}

/// One agent's subject: its own directory, so the object sets are **disjoint by construction** and
/// not by luck. 裁定 #20 asks for 「disjoint object 群」 and a shared directory would make the
/// filesystem the thing under measurement (M5H7-3 (b)'s finding, one substrate over).
fn subject(agent: usize, n: usize) -> String {
    format!("agent-{agent:02}/subject-{n}")
}

/// What one commit attempt answers.
struct Attempt {
    took: Duration,
    /// `shared` arm only: how long the thread waited before it held the lock.
    waited: Duration,
    committed: bool,
}

/// An engine over a sandbox, with the fs adapter registered (AC-065's shape).
fn engine(sandbox: &Sandbox, name: &str) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        sandbox.dir().join(format!("{name}.journal")),
        gate(),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens on the tmpfs");
    engine.register_adapter(Arc::new(FsAdapter::new()), "gx-adapter-fs 0.1.0");
    engine
}

/// One lifecycle, with the engine already held.
fn one(engine: &mut Engine<InjectedEvidence>, sandbox: &Sandbox, agent: usize, n: usize) -> bool {
    let name = subject(agent, n);
    sandbox.write(&name, b"before");
    let locator = sandbox.locator(&name);
    let intent = intent_for(&locator, b"after");
    let seed = (agent as u64) << 32 | n as u64;
    if engine.submit(&intent, seed, AT).is_err() {
        return false;
    }
    let Ok(id) = engine.plan(&intent, AT) else {
        return false;
    };
    if engine.verify(&id, AT, &signing_key(), None).is_err() {
        return false;
    }
    if engine.canonicalize(&id, AT, None).is_err() {
        return false;
    }
    matches!(
        engine.commit(&id, AT, &signing_key()),
        Ok(Lifecycle::Committed)
    )
}

/// The shipped shape: N threads, one engine, one lock (**M6-06 採(a)**).
fn shared_arm(sandbox: &Sandbox, agents: usize, commits: usize) -> (Vec<Attempt>, Duration) {
    let engine = Arc::new(Mutex::new(engine(sandbox, "shared")));
    let started = Instant::now();
    let attempts = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..agents)
            .map(|agent| {
                let engine = Arc::clone(&engine);
                scope.spawn(move || {
                    let mut mine = Vec::with_capacity(commits);
                    for n in 0..commits {
                        let began = Instant::now();
                        // The wait and the hold are two facts. A per-object lock removes the first
                        // and leaves the second, so a number that mixed them would answer a
                        // different question than 裁定 #20's.
                        let mut held = engine.lock().expect("not poisoned");
                        let waited = began.elapsed();
                        let committed = one(&mut held, sandbox, agent, n);
                        drop(held);
                        mine.push(Attempt {
                            took: began.elapsed(),
                            waited,
                            committed,
                        });
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("an agent thread does not panic"))
            .collect::<Vec<_>>()
    });
    (attempts, started.elapsed())
}

/// The ceiling: N threads, N engines, nothing shared at all.
fn disjoint_arm(sandbox: &Sandbox, agents: usize, commits: usize) -> (Vec<Attempt>, Duration) {
    let started = Instant::now();
    let attempts = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..agents)
            .map(|agent| {
                scope.spawn(move || {
                    let mut mine_engine = engine(sandbox, &format!("disjoint-{agent:02}"));
                    let mut mine = Vec::with_capacity(commits);
                    for n in 0..commits {
                        let began = Instant::now();
                        let committed = one(&mut mine_engine, sandbox, agent, n);
                        mine.push(Attempt {
                            took: began.elapsed(),
                            waited: Duration::ZERO,
                            committed,
                        });
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("an agent thread does not panic"))
            .collect::<Vec<_>>()
    });
    (attempts, started.elapsed())
}

/// One thread, one engine: the serial baseline the other two are read against.
fn single_arm(sandbox: &Sandbox, commits: usize) -> (Vec<Attempt>, Duration) {
    let mut engine = engine(sandbox, "single");
    let started = Instant::now();
    let attempts = (0..commits)
        .map(|n| {
            let began = Instant::now();
            let committed = one(&mut engine, sandbox, 0, n);
            Attempt {
                took: began.elapsed(),
                waited: Duration::ZERO,
                committed,
            }
        })
        .collect();
    (attempts, started.elapsed())
}

fn summarise(tag: &str, attempts: &[Attempt], elapsed: Duration) -> f64 {
    let errors = attempts.iter().filter(|a| !a.committed).count();
    let rate = (attempts.len() - errors) as f64 / elapsed.as_secs_f64();
    println!(
        "{tag}_TOTAL attempts={} committed={} errors={errors} elapsed={elapsed:.3?} \
         throughput={rate:.2} commits/s",
        attempts.len(),
        attempts.len() - errors,
    );
    let mut took: Vec<Duration> = attempts.iter().map(|a| a.took).collect();
    report(tag, "per-commit", &mut took);
    rate
}

fn main() {
    if !measuring() {
        // The build check: both arms are reached, on a tree small enough to cost nothing.
        let sandbox = Sandbox::new("disjoint-build-check");
        let (single, _) = single_arm(&sandbox, 1);
        let (shared, _) = shared_arm(&sandbox, 2, 1);
        let (disjoint, _) = disjoint_arm(&sandbox, 2, 1);
        assert_eq!((single.len(), shared.len(), disjoint.len()), (1, 2, 2));
        println!("DISJOINT_BENCH_BUILD_ONLY arms=3");
        return;
    }

    let agents = agents();
    let commits = commits();
    let sandbox = Sandbox::new("disjoint");
    println!(
        "DISJOINT_GENERATOR agents={agents} commits_per_agent={commits} total={} fs={} \
         cpus={} (裁定 #20: lock は実装しない・測るだけ)",
        agents * commits,
        support::filesystem_of(sandbox.dir()),
        std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get),
    );

    let (single, single_elapsed) = single_arm(&sandbox, commits);
    let single_rate = summarise("DISJOINT_SINGLE", &single, single_elapsed);

    let (shared, shared_elapsed) = shared_arm(&sandbox, agents, commits);
    let shared_rate = summarise("DISJOINT_SHARED", &shared, shared_elapsed);
    let mut waits: Vec<Duration> = shared.iter().map(|a| a.waited).collect();
    report("DISJOINT_SHARED_WAIT", "lock acquisition", &mut waits);
    let total_wait: Duration = shared.iter().map(|a| a.waited).sum();
    let total_took: Duration = shared.iter().map(|a| a.took).sum();
    // 🔴 二つの share を印字する。**和の比は 1 本の外れ値で壊れる** ——最初の測定で、600 本のうち
    // 1 本が 301 ms 待ち、p50 が 50 ns のまま和の比が 0.69 になった。和だけを出せば「待ちが 7 割」と
    // 読まれ、それは分布についての嘘である。中央値の比を隣に置き、最大待ちを名指しする。
    let mut sorted_waits: Vec<Duration> = shared.iter().map(|a| a.waited).collect();
    let mut sorted_took: Vec<Duration> = shared.iter().map(|a| a.took).collect();
    sorted_waits.sort_unstable();
    sorted_took.sort_unstable();
    let mid = sorted_waits.len() / 2;
    println!(
        "DISJOINT_LOCK_WAIT_SHARE sum={:.4} median={:.6} total_wait={total_wait:.3?} \
         total_wall={total_took:.3?} max_wait={:.3?}  \
         (per-object lock が取り除けるのは wait だけ。sum は外れ値 1 本で壊れるので median を隣に置く)",
        total_wait.as_secs_f64() / total_took.as_secs_f64().max(f64::MIN_POSITIVE),
        sorted_waits[mid].as_secs_f64() / sorted_took[mid].as_secs_f64().max(f64::MIN_POSITIVE),
        sorted_waits[sorted_waits.len() - 1],
    );

    let (disjoint, disjoint_elapsed) = disjoint_arm(&sandbox, agents, commits);
    let disjoint_rate = summarise("DISJOINT_CEILING", &disjoint, disjoint_elapsed);

    black_box(&single);
    println!(
        "DISJOINT_COMPARISON single={single_rate:.2} shared={shared_rate:.2} \
         ceiling={disjoint_rate:.2} commits/s  headroom_x={:.2}",
        disjoint_rate / shared_rate.max(f64::MIN_POSITIVE),
    );
    let (floor, budget_source) = min_rate();
    let below = shared_rate < floor;
    println!(
        "DISJOINT_D7 sla={SLA_COMMITS_PER_SEC} commits/s  shared_below_sla={below}  \
         floor={floor} BUDGET_SOURCE={budget_source}  \
         (発火条件=`req/38:930`「serve 経由 AC-066 が SLA を割った時」。本計器は **in-process** であり \
         serve ではない——HTTP・router・Bearer・JSON は含まれない。∴ この行は発火条件そのものの判定では \
         なく、その手前の数字である)"
    );

    // 🔴 §62 R-7: the judgement leaves the process. See `min_rate`'s documentation for why a red
    // stage here is a **regression**, not D-7 firing — the two are one sentence apart and the next
    // reader of a red CI log is the person who needs that sentence.
    if below {
        eprintln!(
            "DISJOINT_FAIL shared={shared_rate:.2} < floor={floor} commits/s ({budget_source}) — \
             回帰であって D-7 の発火ではない(発火条件は serve 経由・裁定 #20 は不変)"
        );
        std::process::exit(1);
    }
}
