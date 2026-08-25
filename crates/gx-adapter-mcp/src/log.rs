// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The record that makes a retry a retry.
//!
//! 51 §7 contract 7 and **E-M4-3** quantify idempotence over "the same delta re-entering (retry)", and 43 T-10c is (sem: SEM-gx-adapter-mcp-039)
//! the situation it exists for: a crash between the write and the journal record, whose recovery is
//! running the same delta again.
//!
//! The other two adapters answer it by **comparing**. `gx-adapter-fs` asks whether the file already
//! holds these bytes; `gx-adapter-git` asks whether the branch already points at this commit. Both can,
//! because for both of them the delta declares the state it wants. A tool call does not declare a
//! state: what a call does is the server's, and a proxy that decided "this call has already been made" (sem: SEM-gx-adapter-mcp-040)
//! from the resource's contents would be **guessing** -- and guessing wrong in the direction that makes
//! a retry send a second irreversible effect.
//!
//! So this adapter records instead. A delta is applied at most once per record, keyed by the delta's
//! own CID (42 §3.4: "its own canonical reference", which two spellings of one change share and two changes never (sem: SEM-gx-adapter-mcp-041)
//! do).
//!
//! # 🔴 What that makes the idempotence a property of
//!
//! **The proxy's record, and not the substrate.** A retry against a log that has forgotten the delta
//! sends the call again, and [`MemoryCallLog`] forgets when the process ends -- which is precisely the
//! event 43 T-10c is about. The seam is here so that a deployment can put a durable log behind it, and
//! v0.1 ships none.
//!
//! Writing "`apply` is idempotent" without this paragraph would be the comfortable claim and the false one. (sem: SEM-gx-adapter-mcp-042)
//! `req/101` raises the durable log; `gx-adapter-mcp`'s conformance run measures the retry **within one
//! process**, which is what the contract can be measured over today and is not more.

use std::collections::HashSet;
use std::sync::Mutex;

use gx_core::DeltaRef;

/// What `apply` asks before it calls a tool.
///
/// `Send + Sync` for the reason [`crate::ToolTransport`] is: an adapter holds one and 41 §4 bounds the
/// adapter (**AC-046**).
pub trait CallLog: Send + Sync {
    /// Has this delta already been applied?
    fn applied(&self, delta: &DeltaRef) -> bool;

    /// Record that it has.
    ///
    /// Called **after** the call returned successfully. A record written first would turn a failed
    /// call into a change the log claims was made, and 43 T-11's `AbortReason::ApplyFailed` is the
    /// engine's answer to a failure -- not "it happened". (sem: SEM-gx-adapter-mcp-043)
    fn record(&self, delta: &DeltaRef);

    /// 🔴 **R28 / `req/334` M-03** -- a sentence this adapter composed and has no return type to
    /// put it in.
    ///
    /// # Why this method exists
    ///
    /// `InverseCompletion::complete_inverse` answers `Result<Option<PlannedDelta>>`. Four different
    /// facts land on its `Ok(None)` -- the escrowed arguments are not the JSON object this adapter
    /// wrote, the observation did not carry a member the declaration named, the completed arguments
    /// would not serialise, and the completed payload is over M4-21's ceiling -- and the engine
    /// folds all four to one `InverseStatus::Unavailable`. The fold at the engine is a ruling and is
    /// not this lane's to move (see `docs/LIMITS.md`, v0.5-o). What *was* this lane's is that the
    /// **reason was being dropped on the floor**: in one of the four cases
    /// `ArgSource::resolve_from_observation` has already composed a human sentence naming the
    /// pointer and what was wrong with it, and the road discarded it with `Err(_unresolvable)`.
    ///
    /// # Why on the log rather than on a new seam
    ///
    /// The log is already "the seam a deployment fills" for facts this adapter learns while making
    /// a call, and it is already wired through every construction of [`crate::McpAdapter`]. A
    /// default body -- **drop it**, which is exactly what every implementation did before this
    /// method existed -- keeps the trait backward-compatible for anyone who has implemented it.
    fn note(&self, _note: &str) {}
}

/// The log v0.1 ships: one process, one `HashSet`, gone when the process is.
///
/// Named for what it is. A deployment that wants the retry of 43 T-10c -- the one after a crash -- to
/// be a no-op needs a log that survives the crash, and that is the deployment's until v0.2.
#[derive(Debug, Default)]
pub struct MemoryCallLog {
    applied: Mutex<HashSet<DeltaRef>>,
    /// 🔴 **R28 / `req/334` M-03** -- the sentences [`CallLog::note`] was handed, in order.
    notes: Mutex<Vec<String>>,
}

impl MemoryCallLog {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many distinct deltas this log has seen applied. Printed by the conformance fixture, so that
    /// "the retry made no call" can be told from "no call was ever made". (sem: SEM-gx-adapter-mcp-044)
    ///
    /// # Panics
    /// If a previous holder of the lock panicked. There is nothing this type could return instead that
    /// would not be a number about a log whose state is unknown.
    #[must_use]
    pub fn len(&self) -> usize {
        self.applied
            .lock()
            .expect("the call log is not poisoned")
            .len()
    }

    /// Whether nothing has been applied through this log.
    ///
    /// # Panics
    /// As [`MemoryCallLog::len`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 🔴 **R28 / `req/334` M-03** -- the sentences this log was handed, in the order it got them.
    ///
    /// # Panics
    /// As [`MemoryCallLog::len`].
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        self.notes
            .lock()
            .expect("the call log is not poisoned")
            .clone()
    }
}

impl CallLog for MemoryCallLog {
    fn applied(&self, delta: &DeltaRef) -> bool {
        self.applied
            .lock()
            .expect("the call log is not poisoned")
            .contains(delta)
    }

    fn record(&self, delta: &DeltaRef) {
        self.applied
            .lock()
            .expect("the call log is not poisoned")
            .insert(delta.clone());
    }

    /// Kept rather than dropped, so a deployment running the in-memory log can read back the reason
    /// a completion was refused. The bound is [`MemoryCallLog`]'s own: one process.
    fn note(&self, note: &str) {
        self.notes
            .lock()
            .expect("the call log is not poisoned")
            .push(note.to_string());
    }
}
