// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R30-1 (`req/374` §0 item 1), the **measurement** half of M-01 — the width of the window the
//! pre-`apply` compare-and-set leaves behind, on the filesystem.
//!
//! # The defect this is the residue of
//!
//! 43 T-10c's automatic roll-back applied the escrowed inverse with **nothing** in front of it. The
//! twenty-ninth audit measured what that costs on a real substrate (`req/372` M-01, on the shipped
//! git adapter): `A29_GIT_THIRD_PARTY word=Succeeded their_commit_is_still_the_tip=false
//! cas_before_the_inverse=none` — a third party's legitimate commit was written over by an absolute
//! inverse and the transformation printed `Succeeded`. `req/38` §240 ruling 2 kept it at M, on the
//! grounds that the word is true about *its own* definition, and sent the repair to R30-1: a
//! `precondition` + `fp0.cas_eq` in front of the inverse, `NotAttemptedBecause::WorldMovedBeneath`
//! when the world has moved, **and the residual window declared with a measured width on fs and on
//! mcp, n >= 100**. This file is the fs half of that number; `crates/gx-adapter-mcp/tests/`
//! carries the mcp half, and mcp is the one that matters, because there the call is remote.
//!
//! # What is measured, exactly
//!
//! The repair is a compare-and-set spelled as three adapter calls with a comparison between them:
//!
//! ```text
//! let guard = adapter.snapshot(locator)?;     // the read
//! let fp    = adapter.precondition(&guard)?;  // the read
//! //  <---------------- the window ---------------->
//! match fp0.cas_eq(&fp) { .. }                // the comparison
//! adapter.apply(inverse)?;                    // the write
//! ```
//!
//! The interval clocked below is `[the instant precondition returns, the instant apply is entered]`
//! and nothing else — the gap between the read finishing and the write starting, which is where a
//! third party has to land to be overwritten by a roll-back that has already checked. It is not the
//! twenty-ninth audit's interval: `A29_ROLLBACK_WINDOW` clocked `[apply returns, the read after it
//! returns]`, because before this repair the only read was **after** the write. The two numbers are
//! printed in the same shape so a reader can put them side by side, but they are two different
//! intervals, and the whole point of moving the read in front of the write is that this one is the
//! smaller.
//!
//! # 🔴 The number is a lower bound, and here is everything it leaves out
//!
//! 1. **Engine-side work between the read and the `apply`.** The shipped pipeline does more in that
//!    gap than compare two fingerprints — it appends an `ApplyStarted` journal record before the
//!    inverse goes out. This bed appends none, so the product's gap is wider than this one.
//! 2. **The `apply`'s own run-up.** A substrate write does not become visible at the instant `apply`
//!    is entered; a third party who writes after the CAS and before the `rename` lands is overwritten
//!    just the same. That is why the inverse `apply`'s own median is printed beside the window
//!    rather than left for a reader to look up — the honest bracket on the interval a third party
//!    can be lost in is `[window, window + apply]`.
//! 3. **Cold caches.** The sandbox is written, read and re-read inside one loop; nothing here is
//!    cold.
//! 4. **Slower substrates.** This is a tmpfs, which is the fastest filesystem this adapter will ever
//!    see; the type is read out of `/proc/self/mountinfo` and printed rather than assumed. A spinning
//!    disk, an NFS mount, a loaded host or a `fsync`-honouring filesystem all widen it.
//!
//! A fifth honesty, of a different kind: this is a **reconstruction**, not the engine's own
//! instrumented timing. The T-10c branch and the helper that reads the world in front of the
//! compensation are private to `crates/gx-engine/src/pipeline.rs`. What makes the reconstruction
//! faithful is that it issues the same three calls, in the same order, against the same **shipped**
//! adapter — the twenty-ninth audit said the same about its own bed, and §3-3 of `req/372` filed it
//! as a limit rather than hiding it.
//!
//! # Why nothing here asserts a duration
//!
//! A test that fails when a number is too large is a test that fails when the machine is busy, and
//! fitting a threshold to an observation is the thing this repository does not do. The deliverable
//! is the printed line; the assertions are structural only — that the sample count is the one
//! claimed, that the samples are non-empty and sorted, and that the median does not exceed the
//! maximum.

mod support;

use std::time::{Duration, Instant};

use gx_adapter_fs::FsAdapter;
use gx_core::Fingerprint;
use gx_substrate::SubstrateAdapter;
use support::{filesystem_of, planned, Sandbox, SUBJECT};

/// How many samples. Odd, so the median is an observed value rather than the mean of two, and above
/// `req/374` §0 item 1's floor of a hundred.
const N: usize = 101;

/// Microseconds, to three places.
///
/// The twenty-ninth audit printed one place, which was right for an interval of tens of
/// microseconds. This interval is a comparison between two digests rather than a pair of syscalls,
/// so one place would print `0.0` and call it a measurement.
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

/// The instrument's own floor: how long two back-to-back readings of the clock are apart.
///
/// Printed because the window below is small, and a clock that could not resolve it would print a
/// number anyway. A reader comparing the two lines can see whether the window is a measurement or a
/// quantisation artefact.
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

/// The gap between the compare-and-set's read and the inverse's write, on a filesystem.
#[test]
fn the_window_between_the_cas_read_and_the_inverse_write() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let fstype = filesystem_of(sandbox.dir());
    let substrate = format!("fs({fstype})");

    let mut windows: Vec<Duration> = Vec::with_capacity(N);
    let mut applies: Vec<Duration> = Vec::with_capacity(N);
    let mut reads: Vec<Duration> = Vec::with_capacity(N);

    for round in 0..N {
        // A distinct goal each round, so no sample is a repeat of the delta before it. The fs
        // adapter keeps no call log, so this buys nothing from a short circuit -- it buys that each
        // round's inverse carries different bytes, which is the case the escrow is for.
        let goal = format!("after-{round}\n");
        let pre = adapter
            .snapshot(&locator)
            .expect("the sandbox holds the subject");
        let delta = planned(&adapter, &locator, goal.as_bytes());
        let inverse = adapter
            .invert(&delta, &pre)
            .expect("invert answers")
            .into_inverse()
            .expect("a file that exists has an inverse");

        // The forward apply the engine's roll-back is compensating for. Outside the window.
        adapter.apply(&delta).expect("the forward change applies");

        // `fp0` is state the engine already holds when it reaches T-10c, so acquiring it is not part
        // of the window and is done here rather than inside the clocked region.
        let held = adapter.snapshot(&locator).expect("the read-back answers");
        let fp0: Fingerprint = adapter
            .precondition(&held)
            .expect("the precondition answers");

        // ---- the repaired sequence, in the engine's order ----
        let read_started = Instant::now();
        let guard = adapter.snapshot(&locator).expect("the read-back answers");
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

    windows.sort_unstable();
    applies.sort_unstable();
    reads.sort_unstable();
    let clock = clock_floor();

    println!(
        "R30_ROLLBACK_WINDOW n={N} substrate={substrate} min_us={:.3} median_us={:.3} p95_us={:.3} max_us={:.3}",
        micros(windows[0]),
        micros(median(&windows)),
        micros(p95(&windows)),
        micros(windows[N - 1])
    );
    println!(
        "R30_ROLLBACK_WINDOW_APPLY n={N} substrate={substrate} median_us={:.3} p95_us={:.3} (the inverse's own apply, for scale)",
        micros(median(&applies)),
        micros(p95(&applies))
    );
    println!(
        "R30_ROLLBACK_WINDOW_READ n={N} substrate={substrate} median_us={:.3} p95_us={:.3} (snapshot+precondition, the CAS's own read)",
        micros(median(&reads)),
        micros(p95(&reads))
    );
    println!(
        "R30_ROLLBACK_WINDOW_CLOCK n={N} substrate={substrate} median_us={:.3} max_us={:.3} (back-to-back Instant::now, the instrument's floor)",
        micros(median(&clock)),
        micros(clock[N - 1])
    );
    println!(
        "R30_ROLLBACK_WINDOW_NOTE substrate={substrate} lower_bound=true \
         excludes=engine_journal_ApplyStarted_in_the_gap,the_apply_run_up,cold_caches,slower_filesystems \
         reconstruction=true reason=T-10c_branch_is_private req=372_M-01,38_S240,374_item1"
    );

    // Structural only. No duration is asserted anywhere in this file: a threshold on a clock is a
    // flake on a busy machine, and the number is the deliverable rather than the verdict.
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
}
