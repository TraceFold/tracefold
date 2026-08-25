// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `gx serve`'s runtime (44 §1.2) — the bind, the three stages of shutdown, and the exit.
//!
//! # Why the runtime is here and the verb is in gx-cli
//!
//! Two texts decide it and they agree. 44 §1.1 lists `gx serve` among the CLI's thirteen commands
//! ("`gx serve` | gx-api start-up"; sem: SEM-gx-api-212), and 41 §2 gives `gx-api` the description "axum HTTP+JSONL stream
//! (conformant to 44)" with the verb `serve` in **gx-cli**'s own line. So the **binary** is `gx` and the
//! **runtime** is this crate: gx-cli declares `gx-api` and never `tokio`, and [`serve`] is a blocking
//! function that builds a runtime rather than an `async fn` that needs one. 47 §1(a) is the third
//! text — "a single static binary (`gx-cli` folds `gx-api`'s functions in via `gx serve`)" (sem: SEM-gx-api-213) — and it fixes the
//! direction of the dependency, which cargo would refuse to have both ways.
//!
//! 🔴 **What that costs, measured rather than asserted**: `gx`'s shipping closure now contains axum,
//! tokio, hyper and tower, and `crates/gx-cli/tests/ac_057.rs` says so in a number. The registry
//! package count does not move (axum has been gx-api's shipping dependency since hand 1 and tokio was
//! resolved inside its tree), but "what is in `Cargo.lock`" and "what the auditor installs" (sem: SEM-gx-api-214) are
//! different questions — the distinction hand 5 drew for `chrono`. §47 registered the `gx-verify`
//! split against "when AC-057's E2E finds dependency closure blocking third-party acceptance" (sem: SEM-gx-api-215); this is the hand that makes
//! the closure non-empty, and the report carries the number.
//!
//! # 🔴 Three stages, and the third one may not be spelled "normal termination" (sem: SEM-gx-api-216)
//!
//! req/88 §6.2's DoD: "**graceful shutdown** (refuse new requests during shutdown + wait for in-flight commits +
//! fall through to (b) on a time limit) + 🔴 'don't spell the crash path as normal termination' (44's exit 0 is 'normal
//! termination'; M4H4-2 forbids it)" (sem: SEM-gx-api-217).
//!
//! | stage | what happens | who measures it |
//! |---|---|---|
//! | 1 | [`Shutdown::begin`] — new requests are refused with `503`, and every subscriber's reader loop ends | [`guard`] |
//! | 2 | in-flight requests run to completion; the commit inside 43 T-9's critical section is the one this exists for | [`Shutdown::in_flight`] |
//! | 3 | after `grace`, the wait is abandoned and the outcome says so | [`ServeOutcome::deadline_exceeded`] |
//!
//! Stage 1 is not decoration. A `GET /stream` subscription is an in-flight request **that never
//! ends**, so a graceful shutdown that only waited would wait for ever; the streams have to be told.
//!
//! Stage 3's exit is **1**, never 0. 44 §1.2 gives `gx serve` two codes and glosses 1 as "start-up failure" (sem: SEM-gx-api-218),
//! which is an excerpt in the sense E-M6-16 already settled ("§1.2's column is an excerpt; §1.4's common table is authoritative"; sem: SEM-gx-api-219): 44
//! §1.4's 1 is "error (bad input, **internal error**, adapter error)" and a shutdown that abandoned work
//! inside T-9 is an internal error. Answering 0 would tell an init system that a process which may
//! have left a half-applied commit terminated normally — M4H4-2's prohibition at the deployment
//! layer. Raised as **M6H6-3**: 44 has no exit that means "stopped with work outstanding" (sem: SEM-gx-api-220).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::problem::ApiError;
use crate::state::AppState;

/// How long stage 2 is given before stage 3 takes over. 44 writes no number.
///
/// Ten seconds because the thing being waited for is one commit — 43 T-9's critical section, which
/// is a CAS, an `adapter.apply` and a ledger append — and not a queue. A longer default would make an
/// operator's `systemctl stop` hang on a server whose engine is wedged; a shorter one would abandon a
/// commit that was going to finish.
pub const DEFAULT_GRACE: Duration = Duration::from_secs(10);

/// 44 §1.2 writes no default bind; [`crate::auth::DEFAULT_BIND`] is the policy hand 5 fixed.
pub use crate::auth::DEFAULT_BIND;

/// 🔴 **DR-43-4** — how often the reaper asks 43 T-6 whether anything has expired. Sixty seconds.
///
/// 43 writes no number, and this one is chosen against what is being waited for rather than
/// against load: the two TTLs are 24 hours and 72 hours (33 NFR-028), so a minute is three orders
/// of magnitude finer than the deadline it enforces and the sweep it performs is a walk over the
/// rows this process holds. A shorter period would buy precision nobody asked for on a
/// twenty-four-hour deadline and would take `.gx/LOCK` more often, which is a cost a **second**
/// `gx` process pays. A longer one would make `req/38` §148's ruling harder to observe in a test
/// than in production, which is the wrong way round.
pub const DEFAULT_REAP_INTERVAL: Duration = Duration::from_secs(60);

/// 🔴 The shutdown state, shared by the router, the reader tasks and the runtime.
///
/// Two numbers and a flag, because the three stages are three questions: **has it begun**, **how
/// many are still inside**, and **did the deadline pass**.
#[derive(Debug, Default)]
pub struct Shutdown {
    begun: AtomicBool,
    in_flight: AtomicUsize,
}

impl Shutdown {
    /// A server that is running.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage 1: refuse what has not started, and let the subscribers go.
    pub fn begin(&self) {
        self.begun.store(true, Ordering::SeqCst);
    }

    /// Whether stage 1 has happened.
    #[must_use]
    pub fn begun(&self) -> bool {
        self.begun.load(Ordering::SeqCst)
    }

    /// How many requests are inside a handler right now.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Count one in, for as long as the returned value lives.
    fn enter(self: &Arc<Self>) -> InFlight {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        InFlight {
            shutdown: Arc::clone(self),
        }
    }
}

/// One request, counted. Decrements on drop, **including** a drop caused by a panic — a counter that
/// only decremented on the success path would make one panicking handler hang every later shutdown.
struct InFlight {
    shutdown: Arc<Shutdown>,
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.shutdown.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 🔴 Stage 1 and stage 2, as one layer.
///
/// On the router rather than in the handlers, for 44 §2.5's reason: a check thirteen handlers each
/// remember is a check twelve of them can be read without noticing the thirteenth forgot.
///
/// The refusal is `503` with `gx_code: INTERNAL`, and the fold is deliberate and recorded. 44 §2.3's
/// twelve codes have no operational status at all — hand 5 hit the same wall folding
/// `KeyPermissions` ("44 §2.3 has no operational code (no 503 either)"; sem: SEM-gx-api-221) — so "the server is going away" is
/// carried by the **status** and by the `detail`, and the code is the honest "unclassifiable". Raised as
/// **M6H6-4** with a candidate code (`UNAVAILABLE`, 503) for 44 §2.6's backward-compatible addition.
pub async fn guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let shutdown = state.shutdown();
    if shutdown.begun() {
        return ApiError::unavailable(
            "this server is shutting down and is not accepting new requests. Work already inside a \
             handler is being finished (44 §1.2's `gx serve`, graceful shutdown stage 2); retry \
             against the replacement",
        )
        .into_response();
    }
    let _counted = shutdown.enter();
    next.run(request).await
}

/// What the runtime was asked to do.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// `--bind <ADDR:PORT>`.
    pub bind: SocketAddr,
    /// How long stage 2 gets. [`DEFAULT_GRACE`] unless a caller says otherwise.
    pub grace: Duration,
    /// 🔴 **DR-43-4** — how often [`AppState::reap_once`] is called. [`DEFAULT_REAP_INTERVAL`]
    /// unless a caller says otherwise; `Duration::ZERO` turns the timer off and the start-up line
    /// says which of the two a server is running.
    pub reap_interval: Duration,
}

impl ServeConfig {
    /// A configuration bound to `addr` with the default grace period and reaper interval.
    #[must_use]
    pub fn new(bind: SocketAddr) -> Self {
        Self {
            bind,
            grace: DEFAULT_GRACE,
            reap_interval: DEFAULT_REAP_INTERVAL,
        }
    }

    /// 🔴 **DR-43-4** — the same configuration with another reaper period.
    ///
    /// A builder rather than a public field assignment at every call site, and the reason is what
    /// the value means: a `Duration::ZERO` is "this deployment runs no timer", which is exactly the
    /// state `req/182` H-03 measured and 43 §9 wants ended, so it should be reachable by naming it.
    #[must_use]
    pub fn with_reap_interval(mut self, interval: Duration) -> Self {
        self.reap_interval = interval;
        self
    }
}

/// How the server stopped. See the module header for why stage 3 is not exit 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeOutcome {
    /// The signal or condition that started stage 1.
    pub reason: &'static str,
    /// `true` when stage 3 took over from stage 2.
    pub deadline_exceeded: bool,
    /// How many requests were still inside a handler when the wait ended.
    pub in_flight_at_end: usize,
}

impl ServeOutcome {
    /// 44 §1.4's status for this outcome — 0 only for a shutdown that finished its work.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        if self.deadline_exceeded {
            1
        } else {
            0
        }
    }

    /// The structured line 44 §1.2 asks for ("stdout: a start-up log, one structured JSON line"; sem: SEM-gx-api-222).
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "event": "gx.serve.stopped",
            "reason": self.reason,
            "deadline_exceeded": self.deadline_exceeded,
            "in_flight_at_end": self.in_flight_at_end,
            "exit": self.exit_code(),
        })
    }
}

/// What went wrong before there was a server.
#[derive(Debug)]
pub enum ServeError {
    /// The runtime would not build.
    Runtime(std::io::Error),
    /// The address would not bind — 44 §1.2's "1 = start-up failure (a bind error, etc.)" (sem: SEM-gx-api-223).
    Bind {
        /// What was asked for.
        addr: SocketAddr,
        /// What the operating system said.
        source: std::io::Error,
    },
    /// The server stopped answering for a reason that is not a shutdown.
    Serving(std::io::Error),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::Runtime(e) => write!(f, "the async runtime would not start: {e}"),
            ServeError::Bind { addr, source } => write!(f, "cannot bind {addr}: {source}"),
            ServeError::Serving(e) => write!(f, "the server stopped: {e}"),
        }
    }
}

impl std::error::Error for ServeError {}

/// 🔴 Run the surface until a signal, then shut it down in three stages.
///
/// Blocking, and it owns its runtime: see the module header for why the `#[tokio::main]` this
/// replaces would have had to live in gx-cli.
///
/// `started` is called with the bound address once the listener exists and before the first request
/// — the caller prints 44 §1.2's start-up log from it, which this crate cannot do because 44 §1.3's
/// stdout contract is gx-cli's.
///
/// # Errors
/// [`ServeError::Bind`] for 44 §1.2's "a bind error" (sem: SEM-gx-api-224), [`ServeError::Runtime`] if the runtime will not
/// build, [`ServeError::Serving`] for a failure that is not a shutdown.
pub fn serve(
    state: AppState,
    config: &ServeConfig,
    started: impl FnOnce(SocketAddr),
) -> Result<ServeOutcome, ServeError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(ServeError::Runtime)?;

    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(config.bind)
            .await
            .map_err(|source| ServeError::Bind {
                addr: config.bind,
                source,
            })?;
        let local = listener.local_addr().unwrap_or(config.bind);
        started(local);

        let shutdown = state.shutdown();
        let signalled = Arc::clone(&shutdown);
        let state_for_reaper = state.clone();

        // 🔴 **gotcha65 repair** (`req/38` §74 item②, `req/125` §4-2): `grace` is stage 2's
        // deadline and stage 2 does not start until a signal arrives, so the timeout may not wrap
        // this task's whole lifetime -- only the wait *after* the signal (`with_graceful_shutdown`'s
        // own wait for the in-flight count to reach zero) may be bounded by it. The server therefore
        // runs on its own task, woken by a one-shot channel this scope holds the sending half of,
        // rather than being raced against `grace` from the moment it starts.
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server_task = tokio::spawn(async move {
            axum::serve(listener, crate::router(state.clone()))
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        // 🔴 **DR-43-4 (`req/38` §148 ruling 1(iv), lane R2) — the process that has a clock.**
        //
        // Spawned after the listener exists and before the signal wait, so that the first sweep
        // happens on a server that is already answering. It ends with the runtime: `block_on`
        // returns when this scope does, and a task nobody joins is dropped with the runtime — which
        // is the shape wanted here, because a sweep interrupted mid-`reap` has written whatever it
        // journalled and nothing more (`Engine::reap` stops at the first refusal rather than
        // continuing, so there is no half-swept table to inherit).
        let reaper = spawn_reaper(state_for_reaper, config.reap_interval);

        // Stage 0: wait for the signal itself. Unbounded -- nothing has begun yet, and an idle,
        // unsignalled `gx serve` is the ordinary case this repair exists to stop killing.
        let reason = wait_for_signal().await;
        // Stage 1 begins below; the reaper is stopped **first** so that no sweep starts after the
        // router has begun refusing. A sweep already inside `reap` finishes: it holds `.gx/LOCK`,
        // and abandoning a writer mid-journal is the one thing graceful shutdown exists to avoid.
        if let Some(reaper) = reaper {
            reaper.abort();
        }

        // Stage 1. Ordered before the graceful-shutdown future can resolve inside the spawned task
        // (the channel send below is what lets it resolve), so that a subscriber's reader loop and
        // the router's refusal begin at the same instant the module header already documented.
        signalled.begin();
        let _ = shutdown_tx.send(());

        // Stages 2 and 3: `grace` bounds only the drain from here -- the wait for
        // `with_graceful_shutdown`'s future (every connection finishing) to resolve. The abort
        // handle is taken before the join is raced against `grace`, because `timeout` consumes the
        // future it is given and stage 3 still needs a way to cut the task off.
        let abort = server_task.abort_handle();
        let outcome = match tokio::time::timeout(config.grace, server_task).await {
            Ok(joined) => {
                let served = joined.map_err(|e| ServeError::Serving(std::io::Error::other(e)))?;
                served.map_err(ServeError::Serving)?;
                ServeOutcome {
                    reason,
                    deadline_exceeded: false,
                    in_flight_at_end: shutdown.in_flight(),
                }
            }
            Err(_) => {
                // The drain did not finish inside `grace`: abandon the wait rather than leave it
                // polling forever after this function has already returned an outcome that says so.
                abort.abort();
                ServeOutcome {
                    reason,
                    deadline_exceeded: true,
                    in_flight_at_end: shutdown.in_flight(),
                }
            }
        };
        Ok(outcome)
    })
}

/// 🔴 **DR-43-4** — 43 T-6's sweep, on a timer, as a task that can be stopped.
///
/// `None` when the interval is zero, which is the deployment that runs no reaper — the state
/// `req/182` H-03 measured across the whole product, now reachable only by asking for it.
///
/// The first tick of a `tokio::time::interval` fires immediately, and that is wanted: a server
/// restarted after a long outage should not wait a period before noticing rows whose deadline
/// passed while it was down. `MissedTickBehavior::Delay` because a sweep that took longer than the
/// period must not be answered with a burst of catch-up sweeps, each of which takes `.gx/LOCK`.
///
/// What a tick prints is deliberately asymmetric: a sweep that expired nothing says nothing (a
/// line a minute, for ever, is a log nobody reads), and a sweep that expired something, or that
/// could not run, says so once. 43 T-6's abort is a state transition an operator did not ask for,
/// which is the definition of a thing that must be visible.
///
/// 🔴 **R16 / `req/262` H-01** — and all three of those lines were `eprintln!`, which panics on a
/// write error. A panic inside this task kills **the task**, not the process, so a server whose
/// standard error had stopped taking writes would go on answering requests with 43 T-6's deadline
/// no longer enforced by anything — and the line that would have said the reaper stopped is on the
/// stream that just refused one. The audit could not put the clock inside that window and recorded
/// it as a reading of the source; the repair is the same one the request road took, [`crate::notes`].
fn spawn_reaper(state: AppState, interval: Duration) -> Option<tokio::task::JoinHandle<()>> {
    if interval.is_zero() {
        return None;
    }
    Some(tokio::spawn(async move {
        let mut ticks = tokio::time::interval(interval);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            // 🔴 `spawn_blocking` because `reap_once` takes a `std::sync::Mutex` and a file lock:
            // a blocking wait held across an `.await` on a runtime worker is the classic way to
            // starve one, and this task is the only one in the process that is not answering a
            // request. `AppState` is `Clone` and every field is an `Arc`, so the clone is the
            // handle and not a copy of the engine.
            let asked = state.clone();
            let swept = tokio::task::spawn_blocking(move || asked.reap_once()).await;
            match swept {
                Ok(Ok(expired)) if expired.is_empty() => {}
                Ok(Ok(expired)) => {
                    let ids: Vec<String> = expired.iter().map(|id| id.0.to_text()).collect();
                    crate::api_note!(
                        "gx serve: 43 T-6 expired {} transformation(s) on the reaper's sweep \
                         (DR-43-4): {}",
                        ids.len(),
                        ids.join(" ")
                    );
                }
                Ok(Err(e)) => crate::api_note!(
                    "gx serve: the reaper's sweep was refused ({}): {}. 43 §9's INV-L1/INV-L2 are \
                     not being enforced while this persists",
                    e.code,
                    e.detail
                ),
                // The task itself panicked or was cancelled. 41 §6 counts a panic as a bug; a
                // reaper that died silently would leave a server enforcing no deadline and saying
                // nothing, which is the state this whole ruling exists to end.
                Err(e) => crate::api_note!("gx serve: the reaper stopped: {e}"),
            }
        }
    }))
}

/// The signals a shutdown may arrive on, and what to call each in the log.
///
/// `SIGTERM` is what an init system sends and is therefore the one that matters; `ctrl_c` is what an
/// operator sends. On a platform with no `SIGTERM` the interrupt is the only road, which is stated
/// rather than silently reduced.
async fn wait_for_signal() -> &'static str {
    #[cfg(unix)]
    {
        let mut term =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(stream) => stream,
                // A process that cannot install the handler still has to be stoppable.
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                    return "interrupt";
                }
            };
        tokio::select! {
            _ = term.recv() => "SIGTERM",
            _ = tokio::signal::ctrl_c() => "interrupt",
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "interrupt"
    }
}
