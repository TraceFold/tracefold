// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The subject the shared harness runs against, and the schedule it runs in.
//!
//! 51 §7: "each adapter crate only calls this harness from one `#[test]` to inherit the contract
//! tests", so what an adapter's author writes is this file and nothing else.
//!
//! The sandbox is a directory under `CARGO_TARGET_TMPDIR`, which cargo gives an integration test for
//! exactly this. It is a **schedule with nothing to run it**: no cron, no timer, no process. That is
//! the honest bound on everything measured here -- the contracts hold about the entries, and an
//! entry that fires is simulated by writing what a runner would write (`mark_fired`), because a real
//! runner is not this crate's dependency and never becomes one.

#![allow(dead_code)] // each test binary uses a subset; the fixture is shared by all of them

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use gx_adapter_time::{Entry, TimeAdapter, MAX_FORWARD_PAYLOAD_BYTES};
use gx_canon::cid::{self, Domain};
use gx_core::{Actor, ChangeContext, Cid, GoalBytes, Intent, ObjectSnapshot};
use gx_substrate::{PlannedDelta, Result, SubstrateAdapter};
use gx_substrate_conformance::Fixture;

/// The entry every obligation is about.
pub const SUBJECT: &str = "nightly-report.entry";

/// A second entry, for changes that touch different positions.
pub const OTHER: &str = "beside.entry";

/// An entry whose inverse would not fit in the escrow (**M4-21**, AC-048's `Ok(None)`).
pub const HEAVY: &str = "heavy.entry";

/// An entry at a schedule that keeps no record of whether a job ran.
pub const SILENT: &str = "silent.entry";

/// A moment, carried and never compared -- this crate reads no clock, so the value only has to be
/// stable. It is 2023-11-14T22:13:20Z in 42's `Timestamp` unit (nanoseconds since the epoch).
pub const WHEN: i64 = 1_700_000_000_000_000_000;

/// What a fresh schedule starts with at [`SUBJECT`].
#[must_use]
pub fn before() -> Entry {
    Entry {
        action: "report --weekly".to_string(),
        fire_at: WHEN,
        fired: Some(false),
    }
}

/// What an intent asks [`SUBJECT`] to become.
#[must_use]
pub fn goal() -> Entry {
    Entry {
        action: "report --nightly".to_string(),
        fire_at: WHEN + 86_400_000_000_000,
        fired: Some(false),
    }
}

/// An entry at a schedule that does not record firedness at all -- the `Unknown` row.
#[must_use]
pub fn silent() -> Entry {
    Entry {
        action: "report --nightly".to_string(),
        fire_at: WHEN,
        fired: None,
    }
}

/// The canonical bytes of an entry, which is what stands at a position.
///
/// # Panics
/// If the entry has no canonical form, which is a broken encoder rather than a failing claim.
#[must_use]
pub fn bytes(entry: &Entry) -> Vec<u8> {
    entry.encode().expect("an entry has a canonical form")
}

/// A schedule on disk, with nothing that runs it.
pub struct Sandbox {
    dir: PathBuf,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Sandbox {
    /// Make one, with the subject and its neighbour already in it.
    ///
    /// # Panics
    /// If the temporary directory cannot be written to.
    #[must_use]
    pub fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "glovrex-time-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the temporary directory accepts a directory");
        let sandbox = Self { dir };
        sandbox.populate();
        sandbox
    }

    fn populate(&self) {
        self.put(SUBJECT, &before());
        self.put(OTHER, &before());
    }

    /// The absolute position of a named entry.
    #[must_use]
    pub fn locator(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    /// Put an entry in the schedule, the way this crate's `apply` would.
    ///
    /// # Panics
    /// If the directory will not take it.
    pub fn put(&self, name: &str, entry: &Entry) {
        std::fs::write(self.dir.join(name), bytes(entry)).expect("the schedule accepts an entry");
    }

    /// Write raw bytes at a position -- for the cases where what stands there is *not* an entry.
    ///
    /// # Panics
    /// If the directory will not take it.
    pub fn put_raw(&self, name: &str, content: &[u8]) {
        std::fs::write(self.dir.join(name), content).expect("the schedule accepts bytes");
    }

    /// What the runner does when it has run a job: mark the entry fired, in place.
    ///
    /// 🔴 This is deliberately **not** a method of the adapter. gx does not write this assertion
    /// (INV-WM4a-1, `req/1038` §1b); the party that ran the action does, and in this sandbox that
    /// party is the test.
    ///
    /// # Panics
    /// If the entry is not there to mark.
    pub fn mark_fired(&self, name: &str) {
        let path = self.dir.join(name);
        let standing = Entry::decode(&std::fs::read(&path).expect("the entry is there"))
            .expect("the entry is an entry");
        std::fs::write(
            &path,
            bytes(&Entry {
                fired: Some(true),
                ..standing
            }),
        )
        .expect("the schedule accepts the marked entry");
    }

    /// Read the raw bytes at a position.
    ///
    /// # Panics
    /// If nothing is there.
    #[must_use]
    pub fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).expect("the schedule holds this entry")
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

/// An intent for this adapter: `goal` is the entry to stand at `locator`, and an empty goal is a
/// cancellation.
#[must_use]
pub fn intent_for(locator: &str, goal: &[u8]) -> Intent {
    Intent::new(
        gx_adapter_time::adapter::kind(),
        locator.to_string(),
        GoalBytes(goal.to_vec()),
        ChangeContext::Evidence,
        Actor::Human {
            key: "conformance".to_string(),
        },
    )
}

/// The 51 §7 harness's view of this adapter.
pub struct TimeFixture {
    sandbox: Sandbox,
    adapter: TimeAdapter,
}

impl TimeFixture {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sandbox: Sandbox::new(),
            adapter: TimeAdapter::new(),
        }
    }

    #[must_use]
    pub const fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }
}

impl Default for TimeFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture for TimeFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        &self.adapter
    }

    fn locator(&self) -> String {
        self.sandbox.locator(SUBJECT)
    }

    fn intent(&self) -> Intent {
        intent_for(&self.locator(), &bytes(&goal()))
    }

    fn reset(&self) -> Result<()> {
        self.sandbox.put(SUBJECT, &before());
        self.sandbox.put(OTHER, &before());
        Ok(())
    }

    /// Somebody else edited the schedule: the moment moved, so the digest moved.
    fn disturb(&self) -> Result<()> {
        self.sandbox.put(
            SUBJECT,
            &Entry {
                fire_at: WHEN + 1,
                ..before()
            },
        );
        Ok(())
    }

    fn normalise(&self, locator: &str) -> Option<String> {
        Some(gx_adapter_time::normalize(locator))
    }

    /// One pair per rule this adapter's normalisation has (`crate::locator`).
    ///
    /// `..` is deliberately absent: it is **not** folded, because `/a/b/..` is `/a` only when `b` is
    /// a directory and not a symbolic link. A pair claiming those two spellings equal would be this
    /// fixture asserting something the adapter is careful not to say.
    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        let base = self.locator();
        vec![
            (base.replace(SUBJECT, &format!("/{SUBJECT}")), base.clone()),
            (base.replace(SUBJECT, &format!("./{SUBJECT}")), base.clone()),
            (format!("{base}/"), base),
        ]
    }

    /// **L5's first route**: the digest a plan of this intent promises, derived from the goal and
    /// from nothing else -- no position is read on this line or above it.
    fn promised_target(&self) -> Option<Cid> {
        Some(cid::mint(Domain::Leaf, &[&bytes(&goal())]))
    }

    /// A delta whose inverse exceeds [`MAX_FORWARD_PAYLOAD_BYTES`] (**M4-21**, AC-048's `Ok(None)`).
    ///
    /// The escrow carries the *standing* entry, so the subject is a position holding an entry over
    /// the ceiling and an ordinary plan to replace it.
    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        let locator = self.sandbox.locator(HEAVY);
        self.sandbox.put(HEAVY, &heavy_entry());
        let pre = self.adapter.snapshot(&locator).ok()?;
        let delta = self
            .adapter
            .plan(&intent_for(&locator, &bytes(&goal())), &pre)
            .ok()?;
        Some((delta, pre))
    }

    /// 51 §7's commuting case: two entries at different positions of one schedule.
    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        let subject = self.locator();
        let beside = self.sandbox.locator(OTHER);
        Some((
            self.adapter
                .plan(
                    &intent_for(&subject, &bytes(&goal())),
                    &self.adapter.snapshot(&subject).ok()?,
                )
                .ok()?,
            self.adapter
                .plan(
                    &intent_for(&beside, &bytes(&before())),
                    &self.adapter.snapshot(&beside).ok()?,
                )
                .ok()?,
        ))
    }

    /// The non-commuting case: two changes at one position.
    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        let subject = self.locator();
        let pre = self.adapter.snapshot(&subject).ok()?;
        Some((
            self.adapter
                .plan(&intent_for(&subject, &bytes(&goal())), &pre)
                .ok()?,
            self.adapter
                .plan(&intent_for(&subject, &bytes(&before())), &pre)
                .ok()?,
        ))
    }
}

/// An entry whose canonical form is over the escrow ceiling.
#[must_use]
pub fn heavy_entry() -> Entry {
    Entry {
        action: "H".repeat(MAX_FORWARD_PAYLOAD_BYTES + 1),
        fire_at: WHEN,
        fired: Some(false),
    }
}
