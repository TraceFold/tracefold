// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R16 / `req/262` H-01** — this crate's one road to a standard stream.
//!
//! # Why a second module with the same shape as `gx_cli::emit`
//!
//! R15 closed every `eprintln!` under `crates/gx-cli/src/` and wrote the census against the
//! **destination** rather than the payload. The sixteenth audit measured what that definition had
//! left out, and the answer was not a payload and not a place: it was the **window**. The census
//! looked at a *crate*; the thing that is shipped is a **binary**. `crates/gx-cli/Cargo.toml`
//! declares `[[bin]] name = "gx"`, and `gx-api` is one of the fourteen workspace crates that binary
//! links — so six `eprintln!` sites in this crate were inside the artefact and outside the count.
//!
//! What that cost, measured on the previous binary (three runs each, no variation): with
//! `.gx/drafts` at mode `0500` and `gx serve`'s standard error on `/dev/full`, a
//! `POST /v1/candidates` came back **0 bytes — no HTTP status line at all** — where the same
//! request on the same project with standard error on `/dev/null` answered **201 Created**. The
//! server stayed up (the next `GET /v1/transformations` was `200`), so what was lost was not the
//! process: it was **that request's answer**. Neither half does it alone — a dead standard error on
//! a healthy project answered `201` on both destinations — and both halves come from one cause, a
//! filesystem that has stopped taking writes.
//!
//! # Why this is not `gx_cli::emit` itself
//!
//! Two texts forbid the obvious de-duplication and they are both structural rather than stylistic.
//! 47 §1(a) makes the artefact "a single static binary (`gx-cli` folds `gx-api`'s functions in via
//! `gx serve`)", so **gx-cli depends on gx-api** and cargo refuses the edge back. The only crate
//! both of them already carry is `gx-core`, and 41 §6 gives that crate its identity in the negative
//! — "no I/O, deterministic" — which is a promise this module would break. A new leaf crate would
//! contradict 41 §2's crate table, which this lane may not edit.
//!
//! So the road is per crate and the **denominator is written down by name**: three modules hold a
//! standard stream in the `gx` binary — [`crate::notes`], `gx_mcp_wire::notes` and
//! `gx_cli::emit` — and `probes/doubt/tests/declaration_writer_doubt.rs` D-6 requires exactly
//! those three across every crate the binary links. The **reading** is still one place:
//! `gx_cli::main::settled` adds the three counters together, because the decision about what a note
//! that went nowhere costs belongs to the artefact and not to a crate.
//!
//! # What a failed note costs, and why it is not the request's failure
//!
//! Nothing, and that is the point of the repair. An HTTP answer is a write to a **socket**; it has
//! never needed standard error and must not depend on it. Where 44 or a standing ruling already
//! says a write is best effort — `req/38` §148 lane R2 for the draft archive, `req/222` H-01's
//! asymmetry for a verdict receipt — it stays best effort, and the sentence beside it becomes a
//! value instead of a panic. Where a failure **is** the request's failure it was already an
//! [`crate::ApiError`] with a status, and it still is.

use std::io::Write;

/// 🔴 **R16 / `req/262` H-01** — how many sentences this process could not put on standard error.
///
/// Process-wide rather than per call, for `gx_cli::emit`'s reason: the sites this replaces sit
/// inside handlers and inside a background task, and a `?` on a failed note would make the answer
/// depend on where standard error was pointed.
static NOTES_UNDELIVERED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Put one line on standard error, and say whether it landed.
///
/// # Errors
/// [`std::io::Error`] from either write or from the flush. Flushed rather than buffered, because a
/// line an operator is watching a server for is a line that has to arrive before the next request
/// does, and because a deferred write is one whose failure a destructor discovers with nowhere to
/// report it.
pub fn note_line(text: &str) -> std::io::Result<()> {
    let mut out = std::io::stderr().lock();
    out.write_all(text.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

/// `eprintln!`, with the failure kept and counted rather than turned into a panic.
pub fn note(text: &str) {
    if note_line(text).is_err() {
        NOTES_UNDELIVERED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 🔴 **R16 / `req/262` H-01** — how many sentences went nowhere, for the binary's exit path.
///
/// `gx_cli::main::settled` reads this beside its own crate's counter and `gx_mcp_wire`'s.
#[must_use]
pub fn notes_undelivered() -> u64 {
    NOTES_UNDELIVERED.load(std::sync::atomic::Ordering::Relaxed)
}

/// `eprintln!`, spelled so that a call site changes by one word.
#[macro_export]
macro_rules! api_note {
    () => { $crate::notes::note("") };
    ($($arg:tt)*) => { $crate::notes::note(&format!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A destination that refuses every write — `/dev/full` and a closed pipe in one type.
    struct Refuses;

    impl Write for Refuses {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "no space left on device",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The counter moves, and nothing else does. A note is not an answer.
    #[test]
    fn a_note_that_could_not_be_delivered_is_counted_and_costs_nothing_else() {
        let before = notes_undelivered();
        // The real destination in a test process is a terminal or a capture pipe, both of which
        // take the bytes, so the counter is exercised through the same predicate the road uses.
        let refused = {
            let mut out = Refuses;
            out.write_all(b"x").and_then(|()| out.flush())
        };
        assert!(refused.is_err(), "the destination refused");
        assert_eq!(
            notes_undelivered(),
            before,
            "a refusal somewhere else is not this counter's business"
        );
        note("gx serve: a note the unit test delivered");
        assert!(
            notes_undelivered() >= before,
            "the counter never goes backwards"
        );
    }
}
