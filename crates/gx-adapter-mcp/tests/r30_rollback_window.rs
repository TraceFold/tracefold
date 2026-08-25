// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R30-1 (`req/374` §0 item 1), the **measurement** half of M-01 — the width of the window the
//! pre-`apply` compare-and-set leaves behind, on MCP.
//!
//! # Why this file exists and the filesystem one is not enough
//!
//! 43 T-10c's automatic roll-back applied the escrowed inverse with **nothing** in front of it, and
//! the twenty-ninth audit measured the cost on the shipped git adapter (`req/372` M-01):
//! `A29_GIT_THIRD_PARTY word=Succeeded their_commit_is_still_the_tip=false
//! cas_before_the_inverse=none`. `req/38` §240 ruling 2 sent the repair to R30-1 and asked for the
//! residual window to be **measured on fs and on mcp** at `n >= 100`.
//!
//! 🔴 The audit's own strongest objection to itself is the reason this file is the one that
//! matters. `req/372` §5 item 6 records that the window was measured **on fs only** and that the
//! MCP window -- a remote call -- was not measured at all; and §2 M-01's first rebuttal answers
//! itself by pointing out that the `median 22.6 µs` figure is an fs/tmpfs number, that for MCP the
//! duration of the `apply` *itself* falls inside the window, and that the window is therefore
//! substrate-dependent -- bounded, but not necessarily small. (Paraphrased rather than quoted: the
//! audit reports are written in Japanese and `probes/doubt/tests/cjk_doubt.rs` holds this directory
//! to zero CJK lines, so the citation carries the argument and the report carries the words.)
//! A number taken on a tmpfs does not bound a window whose write is a tool call over a wire.
//!
//! # What is measured, exactly
//!
//! The repair is a compare-and-set spelled as three adapter calls with a comparison between them,
//! and this bed issues the same three in the same order against the same **shipped** adapter:
//!
//! ```text
//! let guard = adapter.snapshot(locator)?;     // the read
//! let fp    = adapter.precondition(&guard)?;  // the read
//! //  <---------------- the window ---------------->
//! match fp0.cas_eq(&fp) { .. }                // the comparison
//! adapter.apply(inverse)?;                    // the write
//! ```
//!
//! The interval clocked is `[the instant precondition returns, the instant apply is entered]` and
//! nothing else. On this substrate both reads reach the transport (`snapshot` and `precondition`
//! each take `cas::read_subject`) and the write is a `tools/call` followed by the post-apply
//! observation, so the read and the apply printed beside the window are both round trips rather than
//! syscalls — which is the comparison a reader of `docs/LIMITS.md` needs in order to see what the
//! window becomes when the trips are real ones.
//!
//! # 🔴 The number is a lower bound, and on this substrate it is a very loose one
//!
//! 1. **The fixture server is in this process.** `tests/support/mod.rs` says so in its own heading —
//!    "An MCP server that lives in this process" — and gives the design reason: this crate ships a
//!    boundary rather than a client, so a deployment writes the transport. **Nothing here crosses a
//!    socket, a loopback interface or a process boundary**, and no JSON-RPC framing is encoded or
//!    parsed. A real MCP deployment adds the framing, the syscalls and the network to every one of
//!    the three calls. The window below is therefore a floor under a floor, and the honest reading
//!    of it is "even with the wire removed entirely, the gap is not zero".
//! 2. **Engine-side work between the read and the `apply`.** The shipped pipeline appends an
//!    `ApplyStarted` journal record in that gap; this bed appends none.
//! 3. **The `apply`'s own run-up.** A tool call does not take effect at the instant `apply` is
//!    entered, and a third party who writes after the CAS but before the call lands on the server is
//!    overwritten just the same. That is why the inverse `apply`'s own median is printed beside the
//!    window: the honest bracket on the interval a third party can be lost in is
//!    `[window, window + apply]`, and on a remote substrate the second term dominates.
//! 4. **One process, single-threaded, warm.** `mcp_conformance.rs` states the same bound about the
//!    contracts it measures against this server, and it is restated here rather than inherited
//!    silently.
//!
//! A fifth honesty, of a different kind: this is a **reconstruction**, not the engine's own
//! instrumented timing — the T-10c branch and the helper that reads the world in front of the
//! compensation are private to `crates/gx-engine/src/pipeline.rs`. `req/372` §3-3 filed the same
//! limit against the twenty-ninth audit's bed rather than hiding it.
//!
//! # Why nothing here asserts a duration
//!
//! A test that fails when a number is too large is a test that fails when the machine is busy, and
//! fitting a threshold to an observation is the thing this repository does not do. The deliverable
//! is the printed line; the assertions are structural only.

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use gx_adapter_mcp::McpAdapter;
use gx_core::Fingerprint;
use gx_substrate::SubstrateAdapter;
use support::{
    absent_snapshot, catalogue, locator_on, planned, FakeServer, RewindableLog, SERVER, WRITE_TOOL,
};

/// How many samples. Odd, so the median is an observed value, and above `req/374` §0 item 1's floor.
const N: usize = 101;

/// What this substrate is called on the printed line.
///
/// Spelled with the fixture's nature in it rather than as a bare `mcp`, because the number is going
/// into a public limits document and "mcp" would let a reader assume a wire that is not there.
const SUBSTRATE: &str = "mcp(in-process-fixture-server)";

/// Microseconds, to three places (the fs probe says why three).
fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000_000.0
}

/// The median of a sorted, non-empty sample.
fn median(sorted: &[Duration]) -> Duration {
    sorted[sorted.len() / 2]
}

/// The 95th percentile, by the same nearest-rank rule `a29_rollback_window.rs` used.
fn p95(sorted: &[Duration]) -> Duration {
    sorted[(sorted.len() as f64 * 0.95) as usize]
}

/// The instrument's own floor: how far apart two back-to-back readings of the clock are.
fn clock_floor() -> Vec<Duration> {
    let mut samples = Vec::with_capacity(N);
    for _ in 0..N {
        let a = Instant::now();
        let b = Instant::now();
        samples.push(b.duration_since(a));
    }
    samples.sort_unstable();
    samples
}

/// The resource round `round` works on.
///
/// 🔴 A distinct resource per round, and it is not tidiness. `crate::apply` asks its
/// [`gx_adapter_mcp::CallLog`] whether a delta was already issued and returns without calling the
/// server when it was, so a loop that replayed one delta would time a short circuit and print it as
/// an `apply`. Distinct resources make every forward delta and every inverse delta distinct by
/// construction, which is the only way to be sure no sample took the retry road.
fn resource_for(round: usize) -> String {
    format!("file:///srv/notes-{round}.md")
}

/// The gap between the compare-and-set's read and the inverse's write, on MCP.
#[test]
fn the_window_between_the_cas_read_and_the_inverse_write() {
    let server = Arc::new(FakeServer::new());
    for round in 0..N {
        server
            .write_behind_the_adapter(&resource_for(round), format!("before-{round}\n").as_bytes());
    }
    let adapter = McpAdapter::new(server.clone())
        .with_catalogue(catalogue())
        .with_log(Arc::new(RewindableLog::new()));

    let mut windows: Vec<Duration> = Vec::with_capacity(N);
    let mut applies: Vec<Duration> = Vec::with_capacity(N);
    let mut reads: Vec<Duration> = Vec::with_capacity(N);

    for round in 0..N {
        let locator = locator_on(SERVER, &resource_for(round));
        let goal = format!("after-{round}\n");
        let delta = planned(&adapter, &locator, WRITE_TOOL, goal.as_bytes());

        // T-10b: the inverse is escrowed **before** the forward apply, because it carries the
        // resource's prior contents and after the call there are none to read.
        let inverse = adapter
            .invert(&delta, &absent_snapshot(&locator))
            .expect("invert answers")
            .into_inverse()
            .expect("a tool with a declared restore has an inverse");

        // The forward apply the roll-back is compensating for. Outside the window.
        adapter.apply(&delta).expect("the forward call applies");

        // `fp0` is state the engine already holds when it reaches T-10c, so acquiring it is not part
        // of the window and is done outside the clocked region.
        let held = adapter.snapshot(&locator).expect("the server answers");
        let fp0: Fingerprint = adapter
            .precondition(&held)
            .expect("the precondition answers");

        // ---- the repaired sequence, in the engine's order ----
        let read_started = Instant::now();
        let guard = adapter.snapshot(&locator).expect("the server answers");
        let fp = adapter
            .precondition(&guard)
            .expect("the precondition answers");
        let read_returned = Instant::now();
        let unmoved = fp0
            .cas_eq(&fp)
            .expect("two fingerprints of one scope compare");
        let apply_entered = Instant::now();
        adapter.apply(&inverse).expect("the inverse applies");
        let apply_returned = Instant::now();
        // ---- end of the sequence ----

        assert!(
            unmoved,
            "nobody writes inside this bed, so the compare-and-set has to hold; a false here would \
             mean the window was measured across a state change rather than across a gap"
        );
        reads.push(read_returned.duration_since(read_started));
        windows.push(apply_entered.duration_since(read_returned));
        applies.push(apply_returned.duration_since(apply_entered));
    }

    // The roll-back landed: every round's resource is back at the bytes it started with. Measured
    // rather than assumed, because a probe whose `apply` quietly did nothing would print a very
    // small number and a very wrong one.
    for round in 0..N {
        assert_eq!(
            server.contents(&resource_for(round)).as_deref(),
            Some(format!("before-{round}\n").as_bytes()),
            "round {round}'s inverse put the resource back"
        );
    }

    windows.sort_unstable();
    applies.sort_unstable();
    reads.sort_unstable();
    let clock = clock_floor();

    println!(
        "R30_ROLLBACK_WINDOW n={N} substrate={SUBSTRATE} min_us={:.3} median_us={:.3} p95_us={:.3} max_us={:.3}",
        micros(windows[0]),
        micros(median(&windows)),
        micros(p95(&windows)),
        micros(windows[N - 1])
    );
    println!(
        "R30_ROLLBACK_WINDOW_APPLY n={N} substrate={SUBSTRATE} median_us={:.3} p95_us={:.3} (the inverse's own apply, for scale)",
        micros(median(&applies)),
        micros(p95(&applies))
    );
    println!(
        "R30_ROLLBACK_WINDOW_READ n={N} substrate={SUBSTRATE} median_us={:.3} p95_us={:.3} (snapshot+precondition, the CAS's own read)",
        micros(median(&reads)),
        micros(p95(&reads))
    );
    println!(
        "R30_ROLLBACK_WINDOW_CLOCK n={N} substrate={SUBSTRATE} median_us={:.3} max_us={:.3} (back-to-back Instant::now, the instrument's floor)",
        micros(median(&clock)),
        micros(clock[N - 1])
    );
    println!(
        "R30_ROLLBACK_WINDOW_CALLS n={N} substrate={SUBSTRATE} tool_calls={} transport_reads={} (nothing took the call log's retry road: {} calls is two per round)",
        server.calls(),
        server.reads(),
        server.calls()
    );
    println!(
        "R30_ROLLBACK_WINDOW_NOTE substrate={SUBSTRATE} lower_bound=true transport=in_process_no_socket_no_jsonrpc_framing \
         excludes=the_wire,engine_journal_ApplyStarted_in_the_gap,the_apply_run_up,cold_caches,real_network \
         reconstruction=true reason=T-10c_branch_is_private req=372_M-01,38_S240,374_item1"
    );

    // Structural only. No duration is asserted anywhere in this file.
    assert!(
        windows.len() >= 100,
        "`req/374` §0 item 1 asks for at least a hundred samples, and {} were collected",
        windows.len()
    );
    assert_eq!(windows.len(), N, "every window sample was recorded");
    assert_eq!(applies.len(), N, "every apply sample was recorded");
    assert_eq!(reads.len(), N, "every read sample was recorded");
    assert!(
        windows.windows(2).all(|pair| pair[0] <= pair[1]),
        "the samples are sorted, which is what makes the median and the p95 mean what they say"
    );
    assert!(
        median(&windows) <= windows[N - 1],
        "a median above the maximum would mean the summary was computed off a different sample"
    );
    assert!(
        median(&applies) <= applies[N - 1],
        "the same, for the apply beside it"
    );
    assert_eq!(
        server.calls(),
        2 * N,
        "two tool calls per round, forward and inverse: any fewer would mean a delta was short \
         circuited by the call log and its `apply` was timed as a no-op"
    );
}
