//! 🔴 `gx serve`'s runtime (44 §1.2) — the bind, the three stages of shutdown, and the exit.
//!
//! # Why the runtime is here and the verb is in gx-cli
//!
//! Two texts decide it and they agree. 44 §1.1 lists `gx serve` among the CLI's thirteen commands
//! (「`gx serve` | gx-api起動」), and 41 §2 gives `gx-api` the description 「axum HTTP+JSONL stream
//! （44準拠）」 with the verb `serve` in **gx-cli**'s own line. So the **binary** is `gx` and the
//! **runtime** is this crate: gx-cli declares `gx-api` and never `tokio`, and [`serve`] is a blocking
//! function that builds a runtime rather than an `async fn` that needs one. 47 §1(a) is the third
//! text — 「単一静的バイナリ(`gx-cli` が `gx serve` で `gx-api` 機能を内包)」 — and it fixes the
//! direction of the dependency, which cargo would refuse to have both ways.
//!
//! 🔴 **What that costs, measured rather than asserted**: `gx`'s shipping closure now contains axum,
//! tokio, hyper and tower, and `crates/gx-cli/tests/ac_057.rs` says so in a number. The registry
//! package count does not move (axum has been gx-api's shipping dependency since hand 1 and tokio was
//! resolved inside its tree), but 「what is in `Cargo.lock`」 and 「what the auditor installs」 are
//! different questions — the distinction hand 5 drew for `chrono`. §47 registered the `gx-verify`
//! split against 「AC-057 の E2E で依存閉包が第三者の受け入れを妨げた時」; this is the hand that makes
//! the closure non-empty, and the report carries the number.
//!
//! # 🔴 Three stages, and the third one may not be spelled 「normal termination」
//!
//! req/88 §6.2's DoD: 「**graceful shutdown**(shutdown 中の新規 request 拒否+in-flight commit 待ち+
//! 時間制限で (b) へ落ちる)+🔴「crash 経路を正常終了と綴らない」(44 の exit 0 は「正常終了」・M4H4-2
//! の禁)」.
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
//! Stage 3's exit is **1**, never 0. 44 §1.2 gives `gx serve` two codes and glosses 1 as 「起動失敗」,
//! which is an excerpt in the sense E-M6-16 already settled (「§1.2 の列は抜粋・§1.4 共通表が正」): 44
//! §1.4's 1 is 「エラー（入力不正・**内部エラー**・adapterエラー）」 and a shutdown that abandoned work
//! inside T-9 is an internal error. Answering 0 would tell an init system that a process which may
//! have left a half-applied commit terminated normally — M4H4-2's prohibition at the deployment
//! layer. Raised as **M6H6-3**: 44 has no exit that means 「stopped with work outstanding」.

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
/// `KeyPermissions` (「44 §2.3 に運用 code が無い(503 も無い)」) — so 「the server is going away」 is
/// carried by the **status** and by the `detail`, and the code is the honest 「分類不能」. Raised as
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
}

impl ServeConfig {
    /// A configuration bound to `addr` with the default grace period.
    #[must_use]
    pub fn new(bind: SocketAddr) -> Self {
        Self {
            bind,
            grace: DEFAULT_GRACE,
        }
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

    /// The structured line 44 §1.2 asks for (「stdout: 起動ログ（構造化JSON行）」).
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
    /// The address would not bind — 44 §1.2's 「1=起動失敗（bindエラー等）」.
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
/// [`ServeError::Bind`] for 44 §1.2's 「bindエラー」, [`ServeError::Runtime`] if the runtime will not
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
        let reason = Arc::new(std::sync::Mutex::new("unknown"));
        let reason_writer = Arc::clone(&reason);

        let server = axum::serve(listener, crate::router(state.clone())).with_graceful_shutdown(
            async move {
                let cause = wait_for_signal().await;
                if let Ok(mut slot) = reason_writer.lock() {
                    *slot = cause;
                }
                // Stage 1. Ordered before axum stops accepting, so that a subscriber's reader loop
                // and the router's refusal begin at the same instant.
                signalled.begin();
            },
        );

        // Stages 2 and 3. `axum::serve`'s future resolves when every connection has finished; the
        // timeout is the deadline that stops that wait being unbounded.
        let cause = || reason.lock().map_or("unknown", |slot| *slot);
        let outcome = match tokio::time::timeout(config.grace, server).await {
            Ok(result) => {
                result.map_err(ServeError::Serving)?;
                ServeOutcome {
                    reason: cause(),
                    deadline_exceeded: false,
                    in_flight_at_end: shutdown.in_flight(),
                }
            }
            Err(_) => ServeOutcome {
                reason: cause(),
                deadline_exceeded: true,
                in_flight_at_end: shutdown.in_flight(),
            },
        };
        Ok(outcome)
    })
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
