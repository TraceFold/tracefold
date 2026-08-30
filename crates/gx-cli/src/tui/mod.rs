// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx tui` — the terminal face (`req/942`).
//!
//! The third renderer to consume the same four routes, after the browser monitor and the offline
//! verify page. It reads and it does not write: the only method this face can put on a socket is
//! `GET` (`super::tui::wire`), it opens no project directory, and it constructs no verdict — when
//! the wire has no verdict for a row it draws the fourth mark rather than rounding to one of three.
//!
//! 🔴 **That last sentence was a claim about code that did not exist** until `req/38` SS965 convert
//! row (d). The three kinds and the fourth mark are now declared
//! ([`wire::VERDICT_KINDS`], [`wire::VerdictMark`]) and gate g15 measures both halves: the fourth
//! mark is not one of the three, and a word this face's vocabulary does not hold is drawn as it
//! arrived rather than rounded into the nearest one. The sentence is kept, in the place it was
//! wrong, because a retraction that deletes the claim leaves nothing for a reader to check.
//!
//! # Three ladders, one shape
//!
//! Placement (`super::tui::layout`), paint (`super::tui::tokens`) and behaviour
//! (`super::tui::acts`) each run `intent -> role -> token -> value`, and each ends in exactly one
//! module that spells the medium. What a reader can do is therefore a **declaration** — eight acts,
//! each with an intent, an effect and the keys that produce it — resolved by one reducer, rather
//! than a `match` in the drawing loop that no gate can read.
//!
//! # The shape of the screen
//!
//! Four declared regions (`super::tui::layout`): the engine's account of itself, the rows it
//! produced, the four facts this process measured while reading, and what was let go of. Which of
//! them survive at a given size is **computed** and handed to the renderer; no screen contains an
//! `if width <`.
//!
//! # Two ways in, one road
//!
//! `gx tui` draws on a terminal. `gx tui --dump` draws the same frame into an off-screen buffer and
//! prints it. Both go through `renderer::draw`, so a capture is a picture of the running program
//! rather than of a second implementation of it.
//!
//! # Environment
//!
//! `GX_BASE_URL` and `GX_TOKEN`, borrowed from the TypeScript SDK and the browser monitor rather
//! than invented here — one fact should not acquire a second name.

pub mod acts;
pub mod layout;
pub mod live;
pub mod renderer;
pub mod tokens;
pub mod wire;

use std::io::IsTerminal;

use serde_json::json;

use crate::exit::Outcome;
use renderer::Tier;
use wire::Screen;

/// Everything `gx tui` was told.
#[derive(Clone, Debug)]
pub struct Options {
    /// The server to read, already resolved from flag, environment or default.
    pub base_url: String,
    /// The bearer token, if this deployment wants one.
    pub token: Option<String>,
    /// Draw one frame into a buffer and print it instead of taking the terminal.
    pub dump: bool,
    /// The buffer's width when dumping.
    pub width: u16,
    /// The buffer's height when dumping.
    pub height: u16,
    /// Spell the disclosure in full even when it costs rows.
    pub wide: bool,
    /// Force a capability tier instead of reading the environment.
    pub tier: Option<Tier>,
}

impl Options {
    /// Resolve the server and the token: flag, then environment, then default.
    #[must_use]
    pub fn resolve(base_url: Option<String>, token_file: Option<&std::path::Path>) -> Self {
        let base_url = base_url
            .or_else(|| std::env::var(wire::BASE_URL_ENV).ok())
            .unwrap_or_else(|| wire::DEFAULT_BASE_URL.to_string());
        let token = token_file
            .and_then(|path| std::fs::read_to_string(path).ok())
            .or_else(|| std::env::var(wire::TOKEN_ENV).ok())
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        Self {
            base_url,
            token,
            dump: false,
            width: 80,
            height: 24,
            wide: false,
            tier: None,
        }
    }
}

/// Run the face.
///
/// # Errors
/// [`crate::Error::Usage`] when the terminal face is asked for on something that is not a terminal,
/// and [`crate::Error::OutputFailed`] when a dumped line does not land.
pub fn run(options: &Options) -> crate::Result<Outcome> {
    let tier = options.tier.unwrap_or_else(Tier::detect);
    if options.dump {
        return dump(options, tier);
    }
    // 🔴 **stdin, and the reason is a probe's count rather than a preference.**
    //
    // `probes/doubt/tests/declaration_writer_doubt.rs` d6 counts the sites in this binary that hold
    // a standard output stream, and **the count is the guarantee**: four are named with their
    // reasons and a fifth is red even when, as here, it only asks the handle a question rather than
    // writing to it. The probe's implementation is broader than the rule it states
    // (`req/259` H-01 is about *writing* outside `emit`), and the honest response to that is to
    // work inside it and report the gap -- not to respell `std::io::stdout()` until the scanner
    // stops seeing it, which is the quiet door this repository's probes exist to close.
    //
    // stdin is not a substitute chosen to dodge a scan; it is the right question for this verb.
    // `gx tui` reads keys, so a run whose input is not a terminal cannot be driven at all, and
    // `event::read()` would block on a stream that will never carry a keypress.
    //
    // What it does not catch, said plainly: `gx tui > file` from an interactive shell. stdin is
    // still a terminal there, so the refusal does not fire and the escape sequences land in the
    // file. `--dump` is the documented road for that and `--help` says so.
    if !std::io::stdin().is_terminal() {
        return Err(crate::Error::Usage {
            detail: "gx tui reads keys from a terminal and stdin is not one. `gx tui --dump` \
                     renders the same frame into a buffer and prints it, which is what a pipe, a \
                     file and a capture want"
                .to_string(),
        });
    }
    interactive(options, tier)
}

/// One frame, off-screen, printed.
fn dump(options: &Options, tier: Tier) -> crate::Result<Outcome> {
    let screen = Screen::read(&options.base_url, options.token.as_deref());
    let measured = renderer::measured(&screen);
    let plan = layout::resolve(options.width, options.height, &measured, options.wide);
    let buffer =
        renderer::render_to_buffer(&screen, options.width, options.height, tier, options.wide);
    for line in renderer::buffer_text(&buffer).lines() {
        crate::say!("{line}")?;
    }
    Ok(Outcome::ok(json!({
        "gx": "tui",
        "base_url": options.base_url,
        "tier": tier.name(),
        "width": options.width,
        "height": options.height,
        "regions_drawn": plan.rows.iter().map(|(role, rows)| json!({ "role": role.name(), "rows": rows })).collect::<Vec<_>>(),
        "regions_dropped": plan.dropped.iter().map(|role| role.name()).collect::<Vec<_>>(),
        "provenance_folded": plan.provenance_folded,
        "fields_not_drawn": plan.dropped_fields,
        "fields_total": plan.total_fields,
        "truncated": plan.truncated,
        "readings": screen.readings().iter().map(|reading| json!({
            "route": reading.route,
            "status": reading.status,
            "read_at": reading.read_at,
            // A `u128` has no place in a JSON number that has to survive every reader; the value is
            // a millisecond count and fits with room to spare.
            "elapsed_ms": u64::try_from(reading.elapsed_ms).unwrap_or(u64::MAX),
            "error": reading.error,
        })).collect::<Vec<_>>(),
    })))
}

/// Take the terminal, draw, and give it back.
///
/// The whole of the terminal binding lives behind the seam in [`renderer::interactive`] — taking
/// the screen, reading a key, giving the screen back. This module names no medium at all, which is
/// the property gate g5 measures.
fn interactive(options: &Options, tier: Tier) -> crate::Result<Outcome> {
    let (frames, reads) = renderer::interactive(options, tier)?;
    Ok(Outcome::ok(json!({
        "gx": "tui",
        "base_url": options.base_url,
        "tier": tier.name(),
        "frames": frames,
        "reads": reads,
    })))
}
