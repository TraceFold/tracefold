// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R13 / `req/244` H-01** — the one place in this binary that puts a line on stdout.
//!
//! # Why a module rather than `println!`
//!
//! `println!` is a macro that cannot fail in the type system and does fail on a disk: Rust's
//! `print!` family returns nothing, so a write error inside it **panics**. `req/244` H-01 measured
//! what that costs on the road that matters most — `gx repair --yes` writing `.gx/VERSION` and then
//! dying at exit **101** with a Rust panic string, three destinations, three runs each, and a next
//! `gx repair` answering `meta_repaired: []` over the file gx had just created.
//!
//! [`crate::exit::Outcome::emit`] closes that for the one JSON object 44 §1.3 gives each verb. This
//! module closes it for everything else a verb prints: `gx limits`' eight plain lines, `gx demo`'s
//! walk, `gx serve`'s start-up line. Same discipline, same shape — `write_all`, a newline, a flush,
//! and a `Result` the caller has to answer for.
//!
//! # 🔴 **R14 / `req/246` H-01** — and stderr is the same road, because it carries answers too
//!
//! R13 closed stdout and left the **other** stream on `eprintln!`, whose family also returns
//! nothing. The fourteenth audit measured what that cost with no adversary and no `--yes`:
//! `gx receipt show gx1:doesnotexist 2>/dev/full` ended at exit **101** with a Rust panic string,
//! because 44 §1.3 puts every verb's refusal on stderr and the macro that put it there panicked
//! when the destination said no. Five arms, three runs each, no variation — and the cheapest of
//! them is a read verb whose stdout was healthy and which wrote nothing anywhere.
//!
//! [`problem_line`] is the road those refusals take now. What it does **not** have is a fallback:
//! when the stream that carries the failure has itself failed, there is nothing left to say it on,
//! and the honest answer is the exit status this run had already determined (`main`'s `refuse`).
//!
//! # 🔴 **R15 / `req/259` H-01** — and the count is of **destinations**, not of payloads
//!
//! R14 wrote "every stream that carries an answer is closed" and then defined *every* as the sites
//! carrying a `.problem()` — a payload. The fifteenth audit re-implemented the census and measured
//! what the definition had left out: **forty-three** `eprintln!` sites in `crates/gx-cli/src/`, all
//! of them still panicking on a write error, two of them carrying an answer outright (`gx wrap`'s
//! start-up JSON and the note that says `gx serve`'s start-up line could not be delivered) and the
//! other forty-one being sentences beside an answer that **end the same run at exit 101 anyway**.
//! Three runs each, no variation: `gx key gen --json 2>/dev/full` exited **101** with an empty
//! stdout and a secret key already on the disk — so the one string that names the key an operator
//! had just made was the thing that went missing — and `gx wrap 2>/dev/full`, the product's
//! membrane, exited **101** as well.
//!
//! So the distinction between an answer and a sentence beside one is **gone from this module**: two
//! writes to the same file descriptor fail together, and a predicate that sorts them by what they
//! mean is a predicate that has to be re-argued every time a verb learns a new sentence. [`note`]
//! is the road the forty-three take now. What a failed note costs is stated once, in `main`'s exit
//! path, and it is deliberately **nothing**: `2>/dev/null` and `2>/dev/full` are two ways of
//! throwing stderr away, and a run that ended differently in the two would be a run a script cannot
//! branch on (R14's reasoning for `refuse`, one payload class wider).
//!
//! # What is asserted about it
//!
//! `probes/doubt/tests/declaration_writer_doubt.rs` D-6 counts `println!` and `print!` call sites in
//! `crates/gx-cli/src/` and fixes the number at the ones this module owns. A verb that reaches for
//! the macro again is red there rather than in the next audit — which is `req/38` §181 ruling 3's
//! rule (a repair written as a **count** rather than as a place) applied to the road R12's own
//! repair left open. 🔴 **R14** widened the count to the `eprintln!` sites that carry a **problem
//! object**. 🔴 **R15** widens it to the destination: **no** `print!`/`eprint!` family macro
//! anywhere under `crates/gx-cli/src/`, and `std::io::stderr()` held nowhere but here.
//!
//! # What it is not
//!
//! It is not a buffer. Each line is flushed, because a line an operator is watching for is a line
//! that has to arrive before the next step runs — `gx demo`'s narration is read while the walk is
//! happening — and because a deferred write is a write whose failure is discovered by a destructor
//! that has nowhere to report it.

use std::io::Write;

/// Put one line on stdout, and say whether it landed.
///
/// # Errors
/// [`crate::Error::OutputFailed`] carrying the operating system's own sentence, for a destination
/// that closed first, one with no room, or one that is not open at all.
pub fn line(text: &str) -> crate::Result<()> {
    write_line(&mut std::io::stdout().lock(), text).map_err(|e| crate::Error::OutputFailed {
        detail: e.to_string(),
    })
}

/// The same, against a destination the caller names — what the unit tests below measure, and what a
/// future caller with something other than stdout in hand would use.
///
/// # Errors
/// [`std::io::Error`] from either write or from the flush.
pub fn write_line(out: &mut dyn Write, text: &str) -> std::io::Result<()> {
    out.write_all(text.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

/// 🔴 **R14 / `req/246` H-01** — put 44 §1.3's problem object on stderr, and say whether it landed.
///
/// # Why this is not `eprintln!`
///
/// Rust's `eprint!` family returns nothing, exactly as `print!` does, so a write error inside it
/// panics. R13 closed that for stdout and left it open here — and 44 §1.3 puts **every verb's
/// refusal** on stderr, so the open half was the larger one: `req/246` H-01 measured
/// `gx receipt show gx1:doesnotexist 2>/dev/full` at exit **101** with a Rust panic string, on a
/// read verb, with healthy stdout and not one byte written into `.gx/`.
///
/// The serialisation is a value here too, which is where `req/244` L-03's `unwrap_or_default()`
/// went. R13 removed it from stdout and left two copies on stderr (`req/246` L-02), where an empty
/// line at an unchanged exit status was the failure mode.
///
/// # Errors
/// [`std::io::Error`] from the serialiser (folded into [`std::io::ErrorKind::InvalidData`]), from
/// either write, or from the flush. The caller answers for it; there is no second stream to say it
/// on, so what `main` answers with is the exit status this run had already determined.
pub fn problem_line(problem: &serde_json::Value) -> std::io::Result<()> {
    let text = serde_json::to_string(problem)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_line(&mut std::io::stderr().lock(), &text)
}

/// 🔴 **R15 / `req/259` H-01** — how many sentences this run could not put on stderr.
///
/// A process-wide count rather than a per-call value, because the per-call value has nowhere to go:
/// the forty-three sites this replaces sit inside functions whose `Result` is the verb's answer, and
/// a `?` on a failed note would have made the run end at a **different** status depending on how
/// stderr was thrown away. See [`note`].
static NOTES_UNDELIVERED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Put one sentence on stderr, and say whether it landed.
///
/// The typed road. [`note`] is the one caller inside this crate; this is separate so the write and
/// the flush are answered for in a value that the unit tests below can hold.
///
/// # Errors
/// [`std::io::Error`] from either write or from the flush.
pub fn note_line(text: &str) -> std::io::Result<()> {
    write_line(&mut std::io::stderr().lock(), text)
}

/// 🔴 **R15 / `req/259` H-01** — `eprintln!`, with the failure kept and delivered to the one place
/// that can act on it.
///
/// # What the fifteenth audit measured
///
/// R14 closed the `eprintln!` sites carrying 44 §1.3's problem object and left the rest, on the
/// stated ground that "the sentence beside an answer stays a macro". `req/259` H-01 measured the
/// forty-three that stayed: every one of them panics when the destination refuses, which ends the
/// **whole run** at exit 101 — outside 44 §1.4's table — no matter how small the sentence was.
/// Two were not sentences at all: `gx wrap`'s start-up JSON is the membrane's own answer, and
/// `gx key gen --json 2>/dev/full` died at 101 with **stdout empty and the secret key already
/// written**, so an operator held a key they could not name.
///
/// # Why this returns nothing, and what the failure buys
///
/// A note that could not be delivered has exactly one honest consequence, and it is the one R14
/// argued for `refuse`: the run ends with **the status it had already determined**. `2>/dev/null`
/// and `2>/dev/full` are two spellings of "throw stderr away"; a run whose status depended on which
/// one was used is a run no script can branch on. Propagating the error instead would have done
/// precisely that — the forty-three sites sit inside verbs whose `Result` *is* the exit status.
///
/// So the value is not dropped at the call site (forty-three silent discards is the class R13
/// closed); it is counted here and read once, in `main`, where the decision is written down and a
/// probe can hold it. [`notes_undelivered`] is that reading.
pub fn note(text: &str) {
    if note_line(text).is_err() {
        NOTES_UNDELIVERED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 🔴 **R15 / `req/259` H-01** — how many sentences went nowhere, for `main`'s exit path.
#[must_use]
pub fn notes_undelivered() -> u64 {
    NOTES_UNDELIVERED.load(std::sync::atomic::Ordering::Relaxed)
}

/// `println!`, with the failure kept.
///
/// The argument list is `format!`'s, so a call site changes by one word and reads the same. The
/// value is a [`crate::Result`], so a caller that ignores it does not compile without saying so.
#[macro_export]
macro_rules! say {
    () => { $crate::emit::line("") };
    ($($arg:tt)*) => { $crate::emit::line(&format!($($arg)*)) };
}

/// 🔴 **R15 / `req/259` H-01** — `eprintln!`, spelled so that a call site changes by one word.
///
/// The forty-three sites this replaces read the same as they did; what changed is that the write is
/// answered for. See [`crate::emit::note`] for what a failed one costs, and why it is not the call
/// site that decides.
#[macro_export]
macro_rules! note {
    () => { $crate::emit::note("") };
    ($($arg:tt)*) => { $crate::emit::note(&format!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A destination that refuses every write, which is `/dev/full` and a closed pipe in one type.
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

    /// A destination that takes the bytes and refuses the flush — the shape a buffered writer over
    /// a full filesystem has, and the one a `write_all`-only check would miss.
    struct RefusesFlush(Vec<u8>);

    impl Write for RefusesFlush {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the reader is gone",
            ))
        }
    }

    #[test]
    fn a_line_that_lands_is_the_text_and_one_newline() {
        let mut out = Vec::new();
        write_line(&mut out, "gx limits").expect("a `Vec` takes everything");
        assert_eq!(out, b"gx limits\n");
    }

    #[test]
    fn a_write_that_refuses_is_a_value() {
        assert!(write_line(&mut Refuses, "gx limits").is_err());
    }

    /// 🔴 `req/244` H-01's `> /dev/full` arm, in one assertion: the bytes were accepted and the
    /// **flush** is where the destination said no. A check that stopped at `write_all` would call
    /// this line delivered.
    #[test]
    fn a_flush_that_refuses_is_a_value_too() {
        let mut out = RefusesFlush(Vec::new());
        let answer = write_line(&mut out, "gx limits");
        assert_eq!(out.0, b"gx limits\n", "the bytes were taken");
        assert!(answer.is_err(), "and the destination still refused them");
    }
}
