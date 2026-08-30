#![forbid(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx-tui` — the terminal face, as its own package (`req/942`, extracted by #188/#189).
//!
//! # What the extraction bought, in one sentence
//!
//! The face already named no engine crate in its **source** — `tui/tests/r942_tui.rs`'s
//! g11 derives the forbidden names from `crates/` and has held that line since `req/38` SS965 row
//! (a). What it could not say was anything about the **graph**: the modules shipped inside
//! `gx-cli`, which links `gx-engine`, `gx-gate`, `gx-witness`, `gx-log` and four adapters, so
//! "this face touches no engine internals" was a property of a scan rather than a property of the
//! build. Here it is a property of the build: `Cargo.toml` declares `ratatui` and `serde_json`,
//! and `cargo tree -e normal -p gx-tui` names no `gx-*` crate at all.
//!
//! # The seam this crate ends at
//!
//! Three things the face used to reach into `gx-cli` for, and what replaced each:
//!
//! | was | is | why |
//! |---|---|---|
//! | `crate::exit::Outcome` | [`Answer`] | an exit code is the CLI's vocabulary, not a face's |
//! | `crate::say!` | [`Answer::lines`] | 🔴 **this crate writes to no stream at all** |
//! | `crate::clock::now()` (a `gx_core::Timestamp`) | [`clock::now_nanos`] | a newtype over `i64` is not worth a road into the engine's id crate |
//!
//! 🔴 The middle row is the one that is a rule rather than a rename.
//! `probes/doubt/tests/declaration_writer_doubt.rs` d6 counts the sites in the `gx` binary that
//! hold a standard-output stream, and **the count is the guarantee**. A second package that
//! printed would be a writer outside that denominator — the counter reads `crates/gx-cli/src`, so
//! it would not even be red, it would be *invisible*, which is worse. So `dump` returns its lines
//! and `gx-cli`'s one `say!` site puts them on stdout. The face composes; the CLI emits.
//!
//! # What this crate deliberately does not have
//!
//! No feature flags. In `gx-cli` the face sat behind `tui = ["dep:ratatui"]` so that a build which
//! did not want a terminal library did not carry one; that switch still exists and is still the
//! same switch, one level out — `gx-cli`'s `tui` feature is now `["dep:gx-tui"]`, and a package
//! nobody depends on costs a build nothing. Declaring the feature *again* here would be the same
//! number written in two places, which is the shape `req/88` §6.2's four declaration sites exist
//! to prevent.

pub mod clock;
pub mod tui;

/// What a run of the face produced.
///
/// 🔴 **Lines and not a write.** The face has no destination and does not want one: `--dump`
/// composes the text of one frame, and whoever asked for it decides where text goes. See this
/// crate's header for why that is a rule about stdout ownership rather than a matter of taste.
///
/// `lines` is empty for an interactive run — the terminal already showed the frames, and the
/// object is what is left to say about them.
#[derive(Clone, Debug)]
pub struct Answer {
    /// The text of the frame, one entry per row, in the order it was drawn.
    pub lines: Vec<String>,
    /// The object 44 §1.3 puts a single newline-terminated copy of on stdout.
    pub json: serde_json::Value,
}

/// The two ways this face can fail, and no others.
///
/// 🔴 Both variants keep the names they had while this was a module of `gx-cli`, because the
/// consumer maps them onto `gx_cli::Error` of the same name and a rename would have made the exit
/// code a thing to re-derive rather than a thing to carry. `Usage` lands on `ROW_VALIDATION_ERROR`
/// and `OutputFailed` on `ROW_OUTPUT_FAILED`, exactly as before the move.
#[derive(Debug)]
pub enum Error {
    /// The face was asked for on something that cannot carry it.
    Usage {
        /// What was wrong with the request.
        detail: String,
    },
    /// The terminal could not be measured, drawn on, or read from.
    OutputFailed {
        /// The operating system's own sentence.
        detail: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage { detail } | Self::OutputFailed { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for Error {}

/// This face's own result type.
pub type Result<T> = std::result::Result<T, Error>;
