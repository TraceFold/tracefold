// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **WM-5a Phase 1, the join** (`req/1011` §4, ruled by `req/1016`) — `Engine::prediction_outcome`
//! driven by a **shipped adapter** instead of by a fixture written to fill the seat.
//!
//! `req/1010` landed the record and measured it with `PredictingAdapter`, a stub in this crate's
//! `tests/` whose whole purpose was to promise something. That was the honest instrument for the
//! question it asked — does the engine write the record on both arms — but it left the accessor
//! answering `None` for every commit a real user could make, because no production `plan` filled
//! `PlannedDelta::promised_target`. `req/1010` §4c said so in as many words and called the silent
//! road "the road every shipped adapter takes".
//!
//! WM-5a Phase 1 filled the seat in `gx-adapter-fs` and `gx-adapter-git`. This file is what makes
//! that a fact about the engine rather than about the two adapters: a real filesystem, a real
//! `FsAdapter`, the ordinary pipeline, and a prediction that was taken and held.
//!
//! | probe | adapter | expected |
//! |---|---|---|
//! | [`a_commit_through_the_shipped_fs_adapter_records_a_prediction_that_held`] | `gx-adapter-fs` | `Some`, `matched() == true` |
//! | [`the_prediction_is_recorded_at_commit_and_not_at_plan`] | `gx-adapter-fs` | `None` before commit, `Some` after |
//! | [`an_adapter_that_promises_nothing_still_records_nothing`] | the silent stub | `None` — negative control |
//!
//! # 🔴 The dependency, and why it is allowed here
//!
//! N-13 keeps every `SubstrateAdapter` out of this crate's **shipping** graph — an engine that
//! linked a filesystem adapter would ship one substrate's grammar to every user of every
//! substrate. `probes/doubt/tests/workspace_doubt.rs` reads `[dependencies]` for exactly that, and
//! its own note has said since its first hand that "`[dev-dependencies]` is deliberately outside
//! it: a test may drive the engine with a real adapter, and doing so is how later hands will show
//! that the boundary holds". This is that test. Nothing in `src/` names an adapter, which
//! `tests/engine_shape.rs` keeps measuring.
//!
//! # What this cannot catch
//!
//! Promise and observation both end in `cid::mint`, so a bug inside the digest function is
//! invisible here as it is in `ac_049.rs`. And a green run says the prediction *held* on this
//! road; it says nothing about roads where it would not, which is what
//! `wm2a_prediction_outcome_e2e.rs`'s broken arm is for.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use gx_adapter_fs::FsAdapter;
use gx_canon::cid::{self, Domain};
use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence, Lifecycle};

use support::{gate, intent, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const BEFORE: &[u8] = b"before";
const GOAL: &str = "after";

/// A bed on the WSL side of the mount rather than under `CARGO_TARGET_TMPDIR`.
///
/// `support::scratch` puts its directories under the target directory, which on this workspace's
/// usual bench is a Windows drive seen through DrvFs. `gx-adapter-fs`'s own suites deliberately run
/// on a tmpfs and say why (`ac_049.rs`: what tmpfs makes the evidence about), and a rename-and-fsync
/// adapter is the wrong thing to measure across a filesystem translation layer. So this file writes
/// where the platform's temporary directory is, and names the choice rather than inheriting it.
fn bed(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("glovrex-wm5a-{}-{name}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the bed");
    }
    std::fs::create_dir_all(&dir).expect("create the bed");
    dir
}

/// A journal, a real `FsAdapter`, and a subject file holding [`BEFORE`].
fn engine_over_a_real_file(name: &str) -> (Engine<InjectedEvidence>, PathBuf, String) {
    let dir = bed(name);
    let subject = dir.join("subject");
    std::fs::write(&subject, BEFORE).expect("the bed accepts a file");

    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(FsAdapter::new()), "gx-adapter-fs/0.1.0");

    let locator = subject.to_string_lossy().into_owned();
    (engine, subject, locator)
}

/// 🔴 The join: a prediction made by shipped code, taken by the engine, and held.
///
/// Every value asserted below is reached by a road the engine did not choose — the promise from the
/// goal bytes, the observation from the bytes on the disk — so a green run is two independent
/// answers agreeing rather than one value compared with itself.
#[test]
fn a_commit_through_the_shipped_fs_adapter_records_a_prediction_that_held() {
    let (mut engine, subject, locator) = engine_over_a_real_file("kept");
    let one = intent(&locator, GOAL);
    let key = signing_key();

    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &key, None).expect("verify"),
        Lifecycle::Admitted,
        "the fs adapter constructs an inverse for a six-byte file, so C-25 answers `True`"
    );
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &key).expect("commit answers");

    let outcome = engine.prediction_outcome(&id);
    let on_disk = std::fs::read(&subject).expect("the subject is readable");
    println!(
        "WM5A_JOIN state={state:?} outcome={outcome:?} on_disk={:?}",
        String::from_utf8_lossy(&on_disk)
    );

    assert_eq!(
        state,
        Lifecycle::Committed,
        "a kept promise is the unchanged road; an abort here means this probe is reading a road it \
         did not take"
    );
    assert_eq!(
        on_disk,
        GOAL.as_bytes(),
        "the apply moved the real world, so the digests below are about a file and not a buffer"
    );

    let outcome = outcome.expect(
        "🔴 WM-5a Phase 1: a commit through `gx-adapter-fs` took no prediction. Either the \
         production `plan` stopped filling `promised_target` or the engine stopped carrying it \
         into 41 §3's `target` — before this lane this `expect` was the correct behaviour, which \
         is why the assertion is written as the lane's own claim",
    );
    assert!(
        outcome.matched(),
        "the shipped adapter promised a post-state its own apply did not reach: {outcome:?}"
    );
    assert_eq!(
        outcome.predicted,
        cid::mint(Domain::Leaf, &[GOAL.as_bytes()]),
        "the prophecy recorded is the goal's digest, computed here without an engine or an adapter \
         in the road"
    );
    assert_eq!(
        outcome.observed,
        cid::mint(Domain::Leaf, &[on_disk.as_slice()]),
        "and the measurement recorded is the digest of the bytes actually on the disk"
    );
    assert_eq!(
        outcome.observed_at, AT,
        "the moment is the engine's injected `at`, not a clock read"
    );
}

/// The record is written by the commit, not by the plan.
///
/// Without this, a `Some` in the probe above would be consistent with the engine filling the map at
/// `plan` time from the promise alone — which would make `observed` a copy of `predicted` and
/// `matched()` a tautology that can never be false.
#[test]
fn the_prediction_is_recorded_at_commit_and_not_at_plan() {
    let (mut engine, _subject, locator) = engine_over_a_real_file("at_commit");
    let one = intent(&locator, GOAL);
    let key = signing_key();

    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    let after_plan = engine.prediction_outcome(&id);

    engine.verify(&id, AT, &key, None).expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    engine.commit(&id, AT, &key).expect("commit");
    let after_commit = engine.prediction_outcome(&id);

    println!("WM5A_WHEN after_plan={after_plan:?} after_commit={after_commit:?}");
    assert_eq!(
        after_plan, None,
        "🔴 a comparison was reported before there was anything to compare against. `plan` has a \
         promise and no observation; recording an outcome there would be reporting a measurement \
         nobody took"
    );
    assert!(
        after_commit.is_some(),
        "and the commit is where the two values meet"
    );
}

/// 🔴 The negative control: an adapter that promises nothing still records nothing.
///
/// `req/1010` §4c held this end with a fixture, and it stays held here for the reason it was
/// written: `None` means no comparison was taken, and folding it into `matched: false` would report
/// a measurement nobody made. What this lane changed is only *which* adapters are on this road —
/// `gx-adapter-mcp` and the two SQL adapters' partial `UPDATE` (`req/1016` §1), not fs and git.
#[test]
fn an_adapter_that_promises_nothing_still_records_nothing() {
    let dir = bed("silent");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "wm5a-silent-stub/1");

    let one = intent("/srv/world", GOAL);
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    engine.verify(&id, AT, &key, None).expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &key).expect("commit answers");
    let outcome = engine.prediction_outcome(&id);

    println!("WM5A_SILENT state={state:?} outcome={outcome:?}");
    assert_eq!(
        state,
        Lifecycle::Committed,
        "an adapter that promises nothing commits exactly as it did before this lane"
    );
    assert_eq!(
        outcome, None,
        "🔴 no comparison was taken, so there is no outcome to report — the third value, kept \
         apart from `matched() == false` by this assertion"
    );
}
