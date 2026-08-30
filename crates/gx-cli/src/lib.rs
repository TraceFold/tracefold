#![forbid(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// 🔴 **R10 / `req/238` H-01** — `gx repair`'s report object is one `serde_json::json!` literal, and
// the three keys this lane added to it (`declaration_absent`, `head_behind_by`,
// `journal_intact_basis`, and the rest) took the macro's expansion past rustc's default recursion
// limit of 128. The alternative was to build the object and then mutate it key by key, which would
// have made the one place that declares what this verb publishes stop being one place.
#![recursion_limit = "512"]
//! The `gx` command line, as a library. 44 §1's surface and req/56's directory.
//!
//! # 🔴 Rule 1 — this crate holds no semantic authority (sem: SEM-gx-cli-139)
//!
//! req/88 §3 Λ1 is M6's central claim and it is a claim about **absence**:
//!
//! > M6 does not extend `Σ`. gx-cli / gx-api hold only **observation** of `Σ = (L, J, E, Λ)` and
//! > **mapping** onto the engine's 8 entry points. Therefore, if M6 adds semantics, that is an
//! > implementation defect and not a design choice (sem: SEM-gx-cli-140)
//!
//! Three things follow, and all three are mechanically checked by
//! `crates/gx-canon/tests/authority_boundary.rs` rather than promised here:
//!
//! * **no canonical encoding.** 41 §6: "every canonical encode goes through gx-canon only,
//!   bypass forbidden" (sem: SEM-gx-cli-141). This crate
//!   parses `gx1:` text with [`gx_core::Cid::from_text`] and never computes one. Parsing a name is
//!   not minting a name, and the parser living in gx-core rather than gx-canon is what makes the
//!   rule satisfiable instead of something the CLI has to break to do its job.
//! * **no `Verdict`.** 41 §4 puts the one judgement in `Gate::verify`.
//! * **no `Lifecycle` write.** 42 §1.3-3: "state is managed in an external table on the engine
//!   side, keyed by `TransformationId`" (sem: SEM-gx-cli-142). This crate reads states; it keeps none.
//!
//! # 🔴 The one asymmetry, stated rather than hidden (req/88 §3 Λ2)
//!
//! Λ2 says N single-shot CLI runs and one long-lived engine are observationally equal on Σ, and
//! names the single place where that breaks: "the moment the CLI holds state that does not
//! enter `Σ`, equality breaks -- M6-01(a)'s `.gx/drafts/` is exactly that" (sem: SEM-gx-cli-143). The draft body has to live somewhere between
//! `gx submit` and `gx plan`, those are two processes, and the journal records an `IntentId` rather
//! than a body (E-M5-3 + ASM-9). So [`draft`] is CLI-side state that the HTTP surface does not have.
//!
//! 44 §0 already permits the asymmetry — "HTTP `POST /candidates` executes submit+plan together,
//! atomically, so it never exposes the lone Draft state, and falls outside this rule" (sem: SEM-gx-cli-144) — so this is not a breach of the
//! specification. What it does mean is that **AC-055's "identical" has to be read as "identical
//! from `Candidate` onward"**, and that reading is written here because a reading nobody wrote down is a
//! reading the next hand invents differently.
//!
//! # 🔴 What this crate reads back, and what it cannot (req/88 §5 row 13, M4H3-7) (sem: SEM-gx-cli-145)
//!
//! M4H3-7 sent "have the M6 wire hand consider `Deserialize` for the three public `View` types" (sem: SEM-gx-cli-146) to this milestone, and the
//! judgement is **no**, for a reason that is structural rather than a preference: a `View` is a
//! **borrow** type. `gx_canon::TransformationView<'a>` holds `&'a DeltaRef`, `&'a ChangeContext`,
//! `&'a Actor`, `&'a [TransformationId]`; `PayloadView<'a>` is a newtype over `&'a [u8]`.
//! `Deserialize` produces an owned value out of bytes it does not keep, so a borrowing struct can
//! only implement it by borrowing from the input buffer — which would make the decoded value's
//! lifetime the buffer's, and would put a second, weaker meaning on a type whose whole purpose is to
//! be the *projection of a value that exists* (42 §1.3's identity views).
//!
//! Measured rather than asserted: **nine** `…View` types are public in this workspace — four in
//! gx-canon, three in gx-gate, two in gx-substrate — and **eight of the nine carry a `<'a>`**. The
//! ninth, `gx_gate::RequestView`, is owned; it is a Cedar request builder rather than an identity
//! projection, and nothing on a wire refers to it.
//!
//! So this crate reads back the **owned** types instead, and every one it needs already has
//! `Deserialize`: `Receipt`, `ReceiptPayload`, `Checkpoint`, `InclusionProof`, `ConsistencyProof`.
//! M6 adds no `Deserialize` to anything (M5's hands got there first), and M4H3-7 closes with a
//! judgement rather than with an implementation.
//!
//! # What this hand builds and what it does not
//!
//! req/88 §6.2 hand 1: "not a single subcommand is implemented" (sem: SEM-gx-cli-147). There are none. What is here is [`layout`]
//! (req/56's directory, seven paths), [`draft`] (M6-01 adopted (a)), [`index`] (M6-02 adopted (b)) and
//! [`consumers`] (the two accessor decisions M5 left for M6 to settle).

/// 🔴 **R12 / `req/242` H-01** — the one type that writes a project's own declarations.
///
/// Private: nothing outside this crate may build a `DeclarationWriter`, and `MetaRepair` is
/// re-exported from [`layout`] where 44 and `gx repair`'s report already spell it.
mod declaration;

/// 🔴 **P-1a** (`req/535` §3 R-1) — `gx attach`: put `.gx/` on a tree that is already running and
/// enumerate what was put there. The placement road is [`layout::Layout::create`]'s and is not
/// re-implemented; what this module adds is the report that road never had.
///
/// 🔴 **`cfg(feature = "mcp")`** (`req/817`): this module reads an agent's MCP configuration through
/// `gx_mcp_wire::config`, and `gx-mcp-wire` is one of the four crates `req/789` §3 holds private.
/// The public distribution builds without it. See this crate's `Cargo.toml` `[features]`.
#[cfg(feature = "mcp")]
pub mod attach;
pub mod clock;
/// 🔴 **S③** (`req/493`) — `gx confine`, the verb that takes a kernel ruleset and becomes the
/// command. The derivation and the Landlock call are in `gx-confine`; this module is the argument
/// road, the report, and the `exec`.
///
/// 🔴 **`cfg(feature = "confine")`** (`req/817`) — `gx-confine` is private; see `attach` above.
#[cfg(feature = "confine")]
pub mod confine;
pub mod consumers;
/// 🔴 **`cfg(feature = "mcp")`** (`req/817`) — the demo speaks JSON-RPC frames through
/// `gx_mcp_wire::jsonrpc`; see `attach` above.
#[cfg(feature = "mcp")]
pub mod demo;
/// 🔴 **`cfg(feature = "mcp")`** (`req/817`) — undoes an `attach` through `gx_mcp_wire::config`;
/// see `attach` above.
#[cfg(feature = "mcp")]
pub mod detach;
pub mod draft;
pub mod emit;
pub mod exit;
// 🔴 **P-1b** (`req/544`) — the face level of a coverage declaration: what a route claims it could
// observe, in words that are deliberately not a receipt's.
pub mod face;
pub mod index;
pub mod keys;
pub mod layout;
pub mod ledger;
pub mod lifecycle;
pub mod limits;
/// 🔴 **R-922-F2 phase 1** — the `.gx` object file's two verbs. The format is
/// `gx_witness::gxfile`; this module is only the surface over it.
pub mod object;
pub mod otel;
pub mod pipeline;
pub mod policy;
pub mod receipt;
/// 🔴 **`req/470` H-01** — the sentence 43 §7's recovery owes an operator, shared by every verb
/// that sets the recovery off rather than owned by `gx serve`.
pub mod recovery;
pub mod repair;
pub mod replay;
pub mod rng;
pub mod serve;
pub mod session;
/// 🔴 **`cfg(feature = "tui")`** (`req/942` §10-2) — the terminal face. It reads the HTTP surface
/// and nothing else: no engine is opened here, no project directory, no verdict constructed.
#[cfg(feature = "tui")]
pub mod tui;
pub mod verdict;
/// 🔴 **`cfg(feature = "mcp")`** (`req/817`) — `gx wrap` is the road from an agent's `tools/call` to
/// `Engine::commit`, and that road is `gx_mcp_wire`'s transport; see `attach` above.
#[cfg(feature = "mcp")]
pub mod wrap;

/// 42 §3.1's four substrates, from the `--substrate` spelling.
///
/// In the library rather than in `main.rs` because there are now **two** readers of the spelling —
/// the `gx submit` flag and a `gx policy test` scenario — and 44 §1.2 gives them one vocabulary. A
/// second copy in the binary would be a second answer to "what does `fs` mean" (sem: SEM-gx-cli-148) the day one of them
/// grew a case.
///
/// # Errors
/// [`Error::Usage`] for a name that is neither one of 44 §1.2's three nor `custom:<NAME>`.
pub fn substrate_kind(text: &str) -> Result<gx_core::SubstrateKind> {
    match text {
        "fs" => Ok(gx_core::SubstrateKind::Fs),
        "git" => Ok(gx_core::SubstrateKind::Git),
        "mcp" => Ok(gx_core::SubstrateKind::Mcp),
        // 42 §3.1 keeps `Custom` a `String` rather than a registry, and 44 §1.2's synopsis lists
        // only the three that have adapters. A custom kind is accepted here and refused later by
        // `plan` with "no adapter for this substrate" (sem: SEM-gx-cli-149), which is the true refusal: the kind is legal
        // and nothing is registered for it.
        other => other
            .strip_prefix("custom:")
            .map(|name| gx_core::SubstrateKind::Custom(name.to_string()))
            .ok_or_else(|| Error::Usage {
                detail: format!(
                    "--substrate takes fs|git|mcp (44 §1.2) or `custom:<NAME>` (42 §3.1); got \
                     {other:?}"
                ),
            }),
    }
}

/// 42 §3.2's `ChangeContext`, from the `--context` spelling. Shared for [`substrate_kind`]'s reason.
///
/// # Errors
/// [`Error::Usage`] for a name that is neither one of 44 §1.2's six nor `Custom:NAME`.
pub fn change_context(text: &str) -> Result<gx_core::ChangeContext> {
    use gx_core::ChangeContext as C;
    match text {
        "Time" => Ok(C::Time),
        "Evidence" => Ok(C::Evidence),
        "Policy" => Ok(C::Policy),
        "Model" => Ok(C::Model),
        "Representation" => Ok(C::Representation),
        "Substrate" => Ok(C::Substrate),
        other => other
            .strip_prefix("Custom:")
            .map(|name| C::Custom(name.to_string()))
            .ok_or_else(|| Error::Usage {
                detail: format!(
                    "--context takes one of 44 §1.2's six names or `Custom:NAME`; got {other:?}"
                ),
            }),
    }
}

/// What this crate refuses with.
///
/// 41 §6 asks for `thiserror`. The variants are the CLI layer's own — a directory that will not
/// open, a draft that will not parse — and they are deliberately **not** a mirror of the engine's
/// fourteen: 44 §1.4's exit codes and §2.3's `gx_code` are the mapping surface, req/88 M6-09 is the
/// ticket that designs it, and hand 5 is the hand that owns it. A hand-1 enum that guessed at the
/// mapping would be a table two hands would then have to disagree with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The `.gx/` directory (or something inside it) could not be read or written.
    #[error("{action} {path}: {source}")]
    Io {
        /// What was being attempted, for a message that says where it broke.
        action: &'static str,
        /// The path it broke on.
        path: String,
        /// The underlying refusal.
        source: std::io::Error,
    },
    /// A file inside `.gx/` exists and does not hold what its name says it holds.
    ///
    /// Kept separate from [`Error::Io`] on purpose: "cannot be read" and "does not exist" are
    /// different answers (E-M4-35), and so are "does not exist" and "exists but is broken" (sem: SEM-gx-cli-150). req/56 §5's recovery rules branch on
    /// exactly that difference.
    #[error("{path} is not a readable {what}: {detail}")]
    Malformed {
        /// The kind of file it was supposed to be.
        what: &'static str,
        /// Where it is.
        path: String,
        /// What went wrong reading it.
        detail: String,
    },
    /// The directory was written by a layout version this binary does not understand.
    ///
    /// Fail-closed, which is the whole reason `.gx/VERSION` exists (req/56 §2, "layout version (for
    /// migration)" (sem: SEM-gx-cli-151)). 47 §4 makes journal-schema compatibility an upgrade precondition; a binary
    /// that opened a newer directory "best effort" would be the failure that condition is about.
    ///
    /// 🔴 **R11 / `req/240` M-07 (audit 10 M-04)** — and the sentence carries the way out.
    ///
    /// The refusal is right and it was mute: a one-character edit (`1` → `2`) took `gx repair`,
    /// `gx repair --yes`, `gx log proof`, `gx replay` and `gx draft list` to exit 1 with an empty
    /// stdout, no remedy and nothing to distinguish "a newer gx wrote this" from "somebody typed a
    /// 2". `req/222` H-06's rule — a state you can see must have a way out — is answered here in
    /// the only place it can be, because the door itself must stay shut (47 §4): in the **words**.
    #[error(
        "{path} declares layout version {found}; this binary writes {expected}. This directory          says it was written by a gx newer than this one, and a binary that opened it anyway could          misread a journal shape it does not know (47 §4), so every verb refuses — including the          ones that only read. What to fix: if a newer gx wrote this project, use that gx; if you          edited the file, put `{expected}` back on the first line of `{path}` (the lines after it          are `key=value` settings and are not the version). gx does not rewrite this number: a          binary that lowered another binary's declaration would be claiming to understand a          directory it has never read (req/240 M-07)"
    )]
    Layout {
        /// Where the version file is.
        path: String,
        /// What it says.
        found: String,
        /// What this binary understands.
        expected: u32,
    },
    /// 🔴 **R9 / `req/236` H-04** — `.gx/VERSION` is there and does not read as a declaration.
    ///
    /// Its own variant rather than [`Error::Malformed`] because the answer an operator needs is not
    /// "a file is broken" but "**this** file is broken, here is the shape it is in, and here is the
    /// text that would fix it". `req/236` H-04 measured what the fold cost: five byte shapes an
    /// ordinary editor produces (a byte-order mark, a leading blank line, bare-CR endings, a UTF-16
    /// save, two lines swapped) each stopped every verb — including the diagnostic one — with
    /// `VALIDATION_ERROR` and "is not a number". Four of those five parse now
    /// (`gx_log::head::declaration_lines`); this variant is for what is left, and it carries the
    /// remedy `VALIDATION_ERROR` had nowhere to put.
    ///
    /// `gx_code` is `DECLARATION_UNREADABLE` (44 §2.3's ruled additions, R9). Exit stays 44 §1.4's
    /// **1**: `req/38` §148's "no new exit number" holds, exactly as it did for `BUSY`.
    #[error("{path} does not read as a declaration: {form}. {remedy}")]
    Declaration {
        /// Where the declaration is.
        path: String,
        /// What shape the bytes are in — the fact, not the advice.
        form: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// 🔴 **R10 / `req/238` H-01** — `.gx/VERSION` is **not there**, in a project that has one.
    ///
    /// Its own variant beside [`Error::Declaration`] because "there and unreadable" and "not there"
    /// need two different remedies and, until R10, only the first of them was an answer at all. The
    /// second was `Error::Io` with an `ErrorKind::NotFound`, which is 44 §1.4's **6** and the
    /// sentence "you are in the wrong directory" — so `gx repair` on a project that had lost its
    /// declaration exited 6 with **zero** report lines, and `docs/LIMITS.md` v0.4-v's promise that
    /// "`gx repair` opens anyway and reports everything else it can see" was false of it.
    ///
    /// A directory with no `.gx/` at all still takes the `NOT_FOUND` road: this variant is only
    /// reached when `gx_cli::layout::Layout::established` says the directory is a project.
    ///
    /// `gx_code` is `DECLARATION_ABSENT`. Exit stays 44 §1.4's **1**, as `req/38` §148 rules for
    /// every code minted since: no new exit number.
    // 🔴 **R45 / `req/621` L-3, ruling `req/38` §394** — "is not there" was false of a `.gx/VERSION`
    // holding a symbolic link that does not resolve: `attach.rs::present`'s rule (repair.rs:1810,
    // R43 S-7) says a declared path holding a link is something that is there, whatever it points
    // at, and this sentence said the opposite. Widened rather than branched — no new field, no new
    // `gx_code`, no spec 44 §2.3 addition — the same way R40 widened `LAYOUT_BLOCKED`'s title from
    // "directory" to "path" rather than adding a second row for the same refusal.
    #[error("{path} is not there, or is a link that does not resolve, so this project declares no layout. {remedy}")]
    DeclarationAbsent {
        /// Where the declaration should be.
        path: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// 🔴 **R10 / `req/238` H-01** — `.gx/config.toml` is **not there**, in a project that has one.
    ///
    /// 43 §7.9 (b)'s R9 row calls this file "the one that decides the recovery key". `req/238`
    /// H-01 measured `gx submit` answering its absence by writing the shipped default at rc 0 —
    /// `engine_signing_keyid` back to nothing, silently, in a project whose operator had set it.
    ///
    /// The **writer's** door only (`Layout::require_config`): a read verb and `gx repair`'s report
    /// mode both run on a project with no `config.toml`, because `req/227` M-03's rule is that a
    /// reader's door must not be narrower than a writer's.
    ///
    /// `gx_code` is `CONFIG_ABSENT`, exit **1**.
    // 🔴 **R45 / `req/621` L-3, ruling `req/38` §394** — same fold as `DeclarationAbsent` above, at
    // `.gx/config.toml`: a dangling symbolic link answered "is not there", which the codebase's own
    // rule (`attach.rs::present`) already calls false. Widened, not branched, for the same reason.
    #[error("{path} is not there, or is a link that does not resolve, so this project's settings cannot be read. {remedy}")]
    ConfigAbsent {
        /// Where the settings file should be.
        path: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// 🔴 **R12 / `req/242` H-01 (d)** — `.gx/ledger/journal` is **not there**, at a door
    /// that was about to append to it.
    ///
    /// R11 taught `gx repair` to measure a project that had lost its journal instead of printing a
    /// constant over it (`req/240` H-02): rc 1, the same forty-seven keys a healthy report carries,
    /// the leaves and the receipts and the head counted off the files that are still there, and a
    /// remedy that names the backup. `req/242` H-01 (d) then measured the answer being **erased**:
    /// one `gx submit`, and `EngineJournal::open`'s `create(true)` had made an eight-byte
    /// `GXJRNL01`, after which the same `gx repair` said `journal_absent: false`,
    /// `journal_commits: 0` and told a rollback story.
    ///
    /// So the writer's door refuses, in the shape `DECLARATION_ABSENT` and `CONFIG_ABSENT` already
    /// have: the diagnosis still opens (`gx repair` reads it out of the ledger, the receipts and
    /// the head), and the verb that would have written over the diagnosis does not run.
    ///
    /// `gx_code` is `JOURNAL_ABSENT`, exit **1** — `req/38` §148's "no new exit number", as for
    /// every code minted since `BUSY`.
    #[error("{path} is not there, so there is no journal to append to. {remedy}")]
    JournalAbsent {
        /// Where the journal should be.
        path: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// 🔴 **R9 / `req/236` M-01** — the escrowed inverse this row names has no readable body.
    ///
    /// 44 §2.3's `INVERSE_UNAVAILABLE` (409 on the wire, **1** on the command line), which is the
    /// answer the HTTP face has always given and the CLI did not. `req/236` M-01 measured the pair
    /// on one project at one moment: `409 INVERSE_UNAVAILABLE` "42 §3.12 says this one is
    /// BodyMissing" over HTTP, and `INTERNAL` "no blob named gx1:…" from `gx undo` — 44 §2.3's word
    /// for "cannot be classified" put on a state with a name.
    #[error("the escrowed inverse of {id} is {status}, so there is nothing to commit")]
    InverseUnavailable {
        /// The transformation whose undo was asked for.
        id: String,
        /// 42 §3.12's status, by name.
        status: &'static str,
    },
    /// 🔴 **R13 / `req/244` H-01** — this run composed an answer and could not put it on stdout.
    ///
    /// The variant exists because the failure had no seat. `Outcome` is R12's guarantee that a
    /// report was *made*; `print_json` was what delivered it, and `println!` panics on a write
    /// error because Rust's `print!` family returns nothing. `req/244` H-01 measured three ways to
    /// reach it without an adversary — a reader that closes first (`| true`), a full destination
    /// (`> /dev/full`), a reader that takes one byte — and all three ended the same way: exit
    /// **101**, a Rust panic string on stderr where 44 §1.3's problem object belongs, and a
    /// `.gx/VERSION` on the disk that the next `gx repair` reported as `meta_repaired: []`.
    ///
    /// So the delivery is a `Result` now, and this is what it fails with. `gx_code` is
    /// `OUTPUT_FAILED`; the exit is 44 §1.4's **1**, because `req/38` §148 mints no new number and
    /// because 101 was never in a table anybody publishes. What a run that reaches this line has
    /// already done is not lost either: `gx repair --yes` files `.gx/repair/last.json` before it
    /// returns, and the next `gx repair` reads it back under `previous_repair`.
    /// 🔴 **R14 / `req/246` M-01 + L-03** — the sentence had a condition it did not state, and a
    /// run of whitespace where a line continuation belonged.
    ///
    /// The text told every reader that "what that run wrote is recorded in `.gx/repair/last.json`",
    /// with no condition on it, and `req/246` M-01 measured the road where that was false: a
    /// journal-less project whose `.gx/config.toml` `--yes` had just written filed no record at all,
    /// so the next `gx repair` answered `previous_repair: null`. R14 files the record on that road
    /// too — and the sentence still names the two things it depends on (the lock and the key),
    /// because `req/227` M-04's rule is that a remedy naming a file that is not there is worse than
    /// no remedy.
    #[error(
        "this run's answer could not be written to stdout ({detail}). The answer itself was \
         produced — what failed is the destination it was being written to (a reader that closed \
         first, a full filesystem, a closed descriptor). Redirect stdout to a file and run the \
         command again. If this was `gx repair --yes` and that run held the project lock and a \
         signing key, what it wrote is filed in `.gx/repair/last.json`; the next `gx repair` prints \
         that file back under `previous_repair` when it is there (req/244 H-01, req/246 M-01)"
    )]
    OutputFailed {
        /// The operating system's own sentence.
        detail: String,
    },
    /// 🔴 **R13 / `req/244` M-04** — this project has used gx and holds no witness of any commit.
    ///
    /// [`crate::layout::Layout::logged`]'s three witnesses (the ledger beside the journal, the
    /// recorded head, the commit receipts) are what say a project has recorded a commit. A project
    /// that has lost all three is indistinguishable from a directory nothing has been written to —
    /// and `req/244` M-04 measured the price of treating it as one: `gx submit` wrote a fresh
    /// eight-byte journal over two committed transformations, after which `gx repair` answered
    /// `journal_commits: 0`, `head_authenticity: "absent"`, `remedy: null`, and the fact that a
    /// history had existed was readable from nowhere.
    ///
    /// `.gx/index/`, `.gx/evidence/` and `.gx/drafts/` are the difference. None of them witnesses a
    /// commit — which is why [`crate::layout::Layout::logged`] does not count them — and all of
    /// them witness **use**. A directory holding entries in one of them and none of the three
    /// witnesses is a project whose log has gone.
    ///
    /// `gx_code` is `HISTORY_LOST`, exit **1**. There is no `--yes` road: a repair that invented a
    /// history would be a worse answer than the loss, and the remedy is the backup.
    #[error("{path} holds no witness of any commit, and {evidence}. {remedy}")]
    HistoryLost {
        /// The `.gx/` this is about.
        path: String,
        /// What says this project has been used.
        evidence: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// 🔴 **R14 / `req/246` M-04** — a declared directory of `.gx/` has something that is not a
    /// directory sitting in its place.
    ///
    /// R13 gave `.gx/repair/` a row in [`crate::layout::GX_PATHS`], and every door that writes asks
    /// the operating system for each `Shape::Dir` row on its way in. `req/246` M-04 put one byte at
    /// `.gx/repair` and measured `gx submit`, `gx log head` and `gx receipt list` all answering
    /// `INTERNAL` "create …/.gx/repair: File exists (os error 17)" — three runs each, for ever —
    /// while `gx repair` called the project healthy at exit **0**. 44 §2.3 keeps `INTERNAL` for what
    /// cannot be classified; this is a path that is there, is not a directory, and is declared to be
    /// one.
    ///
    /// `gx_code` is `LAYOUT_BLOCKED`, exit **1**. `gx repair` reports it under
    /// `repair_dir_blocked` and `gx repair --yes` moves the bytes to `.gx/<name>.pre-repair.<n>` and
    /// makes the directory — **nothing is removed**, for DR-43-7 (1)'s reason. The refusal names the
    /// predicate rather than the path, which is `req/38` §186 ruling 2's instruction and `req/244`
    /// M-06's standing one.
    /// 🔴 **R40 / `req/38` §328 ruling 2 ②** — the sentence gained a slot where it had a constant.
    ///
    /// It used to read "`{path}` is where `.gx/{rel}` has to be **a directory**", because until R40
    /// every path this refusal was raised about was a declared directory. Audit 39 reached the
    /// mirror image — `.gx/ledger/journal`, a declared **file**, holding a directory — and the
    /// hard-coded noun would have made the refusal's own sentence false about it. So `expected`
    /// carries the clause and the row carries the widened title (`ROW_LAYOUT_BLOCKED`).
    ///
    /// 🔴 `rel` stays "which declared row of `.gx/` this is" and is **not** widened to hold
    /// `ledger/journal`. `probes/doubt/tests/m6_surface_doubt.rs` reads every `rel: "…"` in
    /// `layout.rs` and asserts the set equals req/56 §2's eleven rows, and it caught R40 doing
    /// exactly that: the journal is a file **inside** the declared `ledger` row, not a twelfth row,
    /// and inventing one here would have been a layout surface addition smuggled in as a message
    /// field. The exact file travels in `path`, where an exact file belongs.
    #[error("{path} is where {expected}, and it is {found}. {remedy}")]
    LayoutBlocked {
        /// The path that is in the way.
        path: String,
        /// Which declared row of `.gx/` this is.
        rel: String,
        /// What req/56 §2 says should be at that path, as a clause.
        expected: String,
        /// What is actually there, in one noun.
        found: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// 🔴 **DR-B / `req/38` §337 (`req/565` §3, `req/560` §18) — this project's journal is
    /// there, is the regular file `req/56` §2 declares, and this process could not open it.**
    ///
    /// `req/38` §328 ruling 2 ③④ named the condition and did not mint a word for it: not
    /// [`Error::JournalAbsent`] ("is not there" would be false of a file that is), not
    /// [`Error::LayoutBlocked`] ("is not what the declaration says" would be false of a regular
    /// file that is exactly what was declared), and, until this ruling, a fourth `INTERNAL` —
    /// even though the operating system had already classified it (`EACCES`, or any `io::Error`
    /// kind other than `NotFound` reaching an already-present, already-right-shaped journal).
    /// `INTERNAL` is 44 §2.3's word for what *cannot* be classified, and this can.
    ///
    /// `gx_code` is `JOURNAL_UNREADABLE`, exit 44 §1.4's **1** — `req/38` §148's "no new exit
    /// number" holds, exactly as it has for every code minted since `BUSY`. Reachable only from
    /// [`crate::ledger::refuse_if_the_two_files_disagree`], on `gx log proof` / `gx log
    /// consistency` / `gx log checkpoint`, which is where `req/553` M-01's c8 Form A pinned it as
    /// `INTERNAL` before this ruling.
    #[error("{path} is there, is the file req/56 §2 declares, and this process could not open it: {reason}. {remedy}")]
    JournalUnreadable {
        /// Where the journal is.
        path: String,
        /// The operating system's own classification of why the open failed.
        reason: String,
        /// The text that would fix it.
        remedy: String,
    },
    /// The arguments do not describe an operation this binary can attempt.
    ///
    /// 44 §1.4's 1 "invalid input", and **not** its 2: discipline 52 (req/38 §48 M6H1-1) reserves 2 for the state
    /// machine's "refusal" (sem: SEM-gx-cli-152). Everything clap refuses lands here too, through
    /// [`crate::exit::ERROR`].
    #[error("{detail}")]
    Usage {
        /// What was wrong with the request.
        detail: String,
    },
    /// The named thing is not there. 44 §1.4's 6.
    #[error("no {what} for {id}")]
    NotFound {
        /// The kind of thing that was looked for.
        what: &'static str,
        /// The name it was looked for under.
        id: String,
    },
    /// gx-witness refused: a bad signature, a receipt that is not a legal one, a key file.
    #[error(transparent)]
    Witness(#[from] gx_witness::Error),
    /// gx-log refused: a malformed proof shape, a range outside the tree, a torn ledger.
    #[error(transparent)]
    Log(#[from] gx_log::Error),
    /// gx-engine refused: the journal would not open or replay.
    #[error(transparent)]
    Engine(#[from] gx_engine::Error),
    /// gx-gate refused: the shipped policy pack would not parse.
    ///
    /// 🔴 Reached only at [`crate::session::Session::open`], where FR-028's embedded pack is turned
    /// into a `PolicyEngine`. A failure here is a **broken shipped artefact** rather than anything
    /// the operator did — `packs::fs_pack` parses bytes `include_str!` put in the binary — which is
    /// why it is its own variant and not folded into [`Error::Malformed`]: "the file you gave me is
    /// wrong" and "this build is wrong" are different sentences (sem: SEM-gx-cli-153), and E-M3-3 is the standing rule
    /// against giving them one face.
    #[error(transparent)]
    Gate(#[from] gx_gate::Error),
}

// ---------------------------------------------------------------------------
// 🔴 R21 — the map `req/304` D5 measured the absence of
// ---------------------------------------------------------------------------

/// 🔴 **R21 / `req/306` §1 item 1** — which of three things a refusal is.
///
/// `req/304`'s dogfood walk counted three refusals and found all three wearing
/// `gx_code:"INTERNAL"`: a relative `--locator` (the fs adapter reading **ASM-69-3**, its own
/// declaration, and refusing an argument), an `undo` whose inverse `invert()` had already answered
/// `None` for (42 §3.12's `InverseStatus::Unavailable`, a status a *run* produced), and the same
/// undo road a second time. Its finding D5: "a caller or monitor cannot distinguish 'you made a
/// declared mistake' from 'gx broke'".
///
/// That is the sentence this enum answers. 44 §2.3 keeps `INTERNAL` for what **cannot be
/// classified**, and the discipline R12, R13 and R14 each applied one refusal at a time is that a
/// refusal with a citation behind it is classified by definition. What was missing was a place to
/// say which kind of citation, so that the next lane adding an arm has to answer the question
/// rather than reach for the bucket.
///
/// # 🔴 The fourth value, and why `req/306` named three
///
/// `req/306` §1 item 1 asks for three classes — declaration-derived, verification-derived, and a
/// genuine internal fault. Writing the table found **two rows that are none of the three**:
/// [`BUSY`](REFUSAL_MAP) ("this project is momentarily held by another writer") and `OUTPUT_FAILED`
/// ("the answer was produced and the destination would not take it"). Both already have words of
/// their own, both are statements about *this run's circumstances* rather than about the request or
/// about a check, and folding either into one of the three would be a fold nobody wrote down —
/// which is the exact discipline (req/88 §3 Λ4) this table exists to serve. So the residual is a
/// value with a name. `req/307` files it for a ruling rather than deciding it here.
///
/// # Not on the wire
///
/// 44 §2.3 fixes `ProblemDetail`'s member set at five and `crates/gx-api/tests/wire_census.rs`
/// pins it, so a sixth key is a wire-shape change and therefore a DR. This is a fact about the
/// source, readable by a reader and by `crates/gx-cli/tests/r21_refusal_map_is_whole.rs`, and it
/// never leaves the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalClass {
    /// ① A **declaration** refused it: an argument the substrate's own convention does not accept
    /// (**ASM-69-3**'s absolute positions), a declaration file that is absent or will not parse, a
    /// declared directory whose path holds something else, a request this binary cannot attempt.
    /// The operator changes what they declared or what they typed, and it goes away.
    Declared,
    /// ② A **check ran and answered no**: the undo's CAS, 42 §3.12's escrow status, whether the
    /// journal and the ledger describe one tree, whether this project holds any witness of a commit.
    /// The answer will not change on a retry; the operator looks at the world and decides.
    Verified,
    /// 🔴 The residual named above — true of this run rather than of the request or of a check.
    /// Two rows: another writer holds the lock, and stdout would not take the answer.
    Operational,
    /// ③ 44 §2.3's own word, used as the honest "unclassified" and not as a bucket. Everything
    /// that reaches it is something this binary could not name.
    Internal,
}

/// One row of R21's map: a refusal, the word it wears, and which of [`RefusalClass`] it is.
///
/// `Copy`, because [`Error::refusal`] hands one back by value and a row is four words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefusalRow {
    /// The [`Error`] arm (or the guarded half of one) this row answers for, in words.
    pub arm: &'static str,
    /// Which of the three, plus the residual.
    pub class: RefusalClass,
    /// 44 §2.3's `gx_code`, or one of `gx-api`'s `RULED_ADDITIONS`. Never invented here.
    pub code: &'static str,
    /// 44 §1.3's `title` — what a person reads.
    pub title: &'static str,
    /// Why this row wears this word rather than the one next to it.
    pub why: &'static str,
}

/// 🔴 **R21** — the discriminator for "there is no escrowed inverse", as a substring of the
/// subject `gx-engine` builds its `Error::NotFound` with.
///
/// The kind is flattened on the way out: `gx_engine::UndoRefusal::into_error` builds
/// `Error::NotFound { what, id }` with a `&'static str` set at one site each, and **three** of its
/// subjects name an escrowed inverse (`NoEscrow`, `InverseUnavailable`, and the one
/// `Engine::undo`'s intent builder raises). `req/304` printed one of the three. A repair keyed on
/// the sentence it printed would have left the other two in the bucket, which is the failure
/// `feedback_fix_the_question_not_the_row` names, so the classifier is a predicate over the
/// subject and `crates/gx-cli/tests/r21_refusal_semantics.rs` holds it to `gx-engine`'s own set —
/// the shape `r20_refusal_vocabulary_is_whole.rs` established for exactly this problem one crate
/// over.
pub const ESCROWED_INVERSE_SUBJECT: &str = "escrowed inverse";

/// The `LEDGER_DISAGREES` row (DR-43-6).
pub const ROW_LEDGER_DISAGREES: RefusalRow = RefusalRow {
    arm: "Malformed { what: \"project\" }",
    class: RefusalClass::Verified,
    code: "LEDGER_DISAGREES",
    title: "this project's journal and ledger describe different trees",
    why: "`ledger_agrees` is a gate both writers pass; this row is that check answering no.",
};

/// The `INVERSE_UNAVAILABLE` row for an escrow whose **body** is missing (R9, `req/236` M-01).
pub const ROW_INVERSE_UNAVAILABLE: RefusalRow = RefusalRow {
    arm: "InverseUnavailable",
    class: RefusalClass::Verified,
    code: "INVERSE_UNAVAILABLE",
    title: "there is no escrowed inverse to commit",
    why: "42 §3.12's status was read and is not `Available`.",
};

/// 🔴 **R21 / `req/304` §0.8** — the same word for the escrow that was never constructible.
///
/// The road R9 did not repair. `FsAdapter::invert` answering `Ok(None)` above the 1 MiB escrow
/// ceiling is 42 §3.12's `Unavailable`, `gx-gate` escalates it (**E-M3-4**), and the escalation
/// ticket `gx verify` prints publishes the word `INVERSE_UNAVAILABLE` in `reasons[0].code` two
/// verbs before the `undo` that was answered `INTERNAL`. `req/304`'s own remedy: it "already
/// exists as a *reason code* inside the escalation ticket … it just isn't promoted to the
/// top-level `gx_code` on the later `undo` refusal".
///
/// One word **and** one title with [`ROW_INVERSE_UNAVAILABLE`]: `BUSY_TITLE`'s rule is that one
/// refusal does not get two names, and "the escrow's status is not `Available`" is one refusal
/// whichever of the two roads reached it.
pub const ROW_ESCROW_ABSENT: RefusalRow = RefusalRow {
    arm: "Engine(NotFound { what: <an escrowed inverse> })",
    class: RefusalClass::Verified,
    code: "INVERSE_UNAVAILABLE",
    title: "there is no escrowed inverse to commit",
    why:
        "`invert()` was run and answered `None`; 42 §3.12 wrote that down and the ticket published \
          it. Declared cli_exit 1, which is the number this road already returned.",
};

/// 🔴 **R21 / `req/304` §0.5** — the substrate adapter refused, and 44 §2.3 has the word.
///
/// `gx_engine::Error::Adapter`'s own doc comment names the code it is for — "A `SubstrateAdapter`
/// refused (44 §2.3's `ADAPTER_ERROR`, 502)" — and `gx-api`'s `gx_code::REFUSALS` has answered it
/// since M6-09, with `fold: None`. This crate carried it to `INTERNAL` in the same arm as
/// `Error::Io` and `Error::Gate`, so the two faces of one binary held two names for one refusal:
/// the defect DR-43-6 ruled on for `LEDGER_DISAGREES` and DR-43-5 (2) ruled on for `BUSY_TITLE`.
///
/// 🔴 **`Declared` and not `Internal`, and the honest hedge.** `Error::Adapter` covers all seven
/// of 41 §4's methods, so it carries both "the argument is not a position this adapter can act on"
/// (**ASM-69-3**, `req/304`'s case) and "the file could not be read". Those are two classes and the
/// engine flattens them to one string (`e.to_string()`), so this row is the *finest* honest answer
/// this crate can give without an engine change. Splitting them needs `gx_substrate::Error::kind`
/// to survive the hop, which is an engine-side change and outside R21's write scope — filed in
/// `req/307`, together with the case for the `INVALID_LOCATOR` word `req/304`'s remedy asks for
/// (minting one costs a `req/38` ruling, a row in `gx-api`'s `RULED_ADDITIONS` and a word in
/// `sdk/typescript/src/errors.ts`).
pub const ROW_ADAPTER_ERROR: RefusalRow = RefusalRow {
    arm: "Engine(Adapter)",
    class: RefusalClass::Declared,
    code: "ADAPTER_ERROR",
    title: "the substrate adapter refused this operation",
    why: "44 §2.3 row 9, and `gx-api`'s own word for this kind since M6-09. Declared cli_exit 1, \
          which is the number this road already returned.",
};

/// The `DECLARATION_UNREADABLE` row (R9, `req/236` H-04).
pub const ROW_DECLARATION_UNREADABLE: RefusalRow = RefusalRow {
    arm: "Declaration",
    class: RefusalClass::Declared,
    code: "DECLARATION_UNREADABLE",
    title: "this project's `.gx/VERSION` does not read as a declaration",
    why: "the declaration is present and its bytes are not one.",
};

/// The `DECLARATION_ABSENT` row (R10, `req/238` H-01).
///
/// 🔴 **R45 / `req/621` L-3, ruling `req/38` §394** — title widened, same reason `ROW_LAYOUT_BLOCKED`
/// widened from "directory" to "path": a dangling symbolic link at `.gx/VERSION` reached this row
/// and "is not there" was false of it (`attach.rs::present`'s rule). `why` is unchanged — both
/// shapes are still "the declaration this project needs is missing, and no verb writes one back on
/// its own" in every way that matters to the remedy.
pub const ROW_DECLARATION_ABSENT: RefusalRow = RefusalRow {
    arm: "DeclarationAbsent",
    class: RefusalClass::Declared,
    code: "DECLARATION_ABSENT",
    title: "this project's `.gx/VERSION` is not there, or is a link that does not resolve",
    why: "the declaration this project needs is missing, and no verb writes one back on its own.",
};

/// The `CONFIG_ABSENT` row (R10, `req/238` H-01).
///
/// 🔴 **R45 / `req/621` L-3, ruling `req/38` §394** — same widening as `ROW_DECLARATION_ABSENT`,
/// at `.gx/config.toml`.
pub const ROW_CONFIG_ABSENT: RefusalRow = RefusalRow {
    arm: "ConfigAbsent",
    class: RefusalClass::Declared,
    code: "CONFIG_ABSENT",
    title: "this project's `.gx/config.toml` is not there, or is a link that does not resolve",
    why: "the settings that decide the signing key are missing at the writer's door.",
};

/// The `JOURNAL_ABSENT` row (R12, `req/242` H-01 (d)).
pub const ROW_JOURNAL_ABSENT: RefusalRow = RefusalRow {
    arm: "JournalAbsent",
    class: RefusalClass::Declared,
    code: "JOURNAL_ABSENT",
    title: "this project's `.gx/ledger/journal` is not there",
    why: "an append with nothing to append to, refused before it creates an empty history.",
};

/// The `OUTPUT_FAILED` row (R13, `req/244` H-01) — the first of the residual two.
pub const ROW_OUTPUT_FAILED: RefusalRow = RefusalRow {
    arm: "OutputFailed",
    class: RefusalClass::Operational,
    code: "OUTPUT_FAILED",
    title: "this run's answer could not be written to stdout",
    why: "the answer was produced; the destination it was being written to would not take it. \
          Neither the request nor a check — see `RefusalClass::Operational`.",
};

/// The `HISTORY_LOST` row (R13, `req/244` M-04).
pub const ROW_HISTORY_LOST: RefusalRow = RefusalRow {
    arm: "HistoryLost",
    class: RefusalClass::Verified,
    code: "HISTORY_LOST",
    title: "this project has used gx and holds no witness of any commit it recorded",
    why: "`Layout::logged`'s three witnesses were counted and none is there, in a directory that \
          holds entries of use.",
};

/// The `LAYOUT_BLOCKED` row (R14, `req/246` M-04).
///
/// 🔴 **R40 / `req/38` §328 ruling 2 ② — the title widens from "directory" to "path", and the
/// `why` does not move.**
///
/// R14 minted this row for a declared **directory** holding something that is not one. Audit 39
/// reached the mirror image — `.gx/ledger/journal`, which `req/56` §2 declares as a **file**,
/// replaced by a directory of the same name — and nothing in this map fitted it, so it fell to
/// `INTERNAL` and the read road answered exit 0 with a signature. The `why` was already true of it
/// word for word ("the path is there and is not what the declaration says"); only the title's noun
/// was too narrow. Widening the noun is a **declaration correction**, not a new code: `REFUSAL_MAP`
/// stays seventeen rows and the vocabulary stays 44 §2.3's twelve plus `RULED_ADDITIONS`.
///
/// 🔴 What this row deliberately still does **not** cover, said here rather than implied: a journal
/// that **is** a regular file and cannot be opened (mode `0000`, a filesystem that went away). The
/// path there is exactly what the declaration says, so the `why` above would be false of it, and
/// stretching one row over "wrong shape" and "right shape, unreadable" would put two driven
/// meanings on one line — `req/38` §156 ruling 2(a)'s failure, in the form no gate in this tree
/// catches, because `r21_refusal_map_is_whole` reads arms, codes, exits and classes and does not
/// read titles. §328 ruling 2 ③ leaves that condition on `INTERNAL` and ④ files the thirteenth word
/// as a DR against spec 44 §2.3. `docs/LIMITS.md` carries it as a limit rather than a silence.
pub const ROW_LAYOUT_BLOCKED: RefusalRow = RefusalRow {
    arm: "LayoutBlocked",
    class: RefusalClass::Declared,
    code: "LAYOUT_BLOCKED",
    title:
        "a declared path of this project's `.gx/` is occupied by something that is not what the \
            declaration says",
    why: "`GX_PATHS` declares the shape; the path is there and is not what the declaration says.",
};

/// The `JOURNAL_UNREADABLE` row (DR-B, `req/38` §337, `req/565` §3).
///
/// The thirteenth word `req/38` §328 ruling 2 ③④ deliberately did not mint, minted here: the
/// journal is present, is the regular file `req/56` §2 declares, and this process's own attempt
/// to open it failed. Not `JOURNAL_ABSENT` (the file is there) and not `LAYOUT_BLOCKED` (the
/// shape is exactly what was declared) — the operating system named the path and the reason, and
/// `INTERNAL` is 44 §2.3's word for the opposite of that.
pub const ROW_JOURNAL_UNREADABLE: RefusalRow = RefusalRow {
    arm: "JournalUnreadable",
    class: RefusalClass::Declared,
    code: "JOURNAL_UNREADABLE",
    title: "this project's journal is there and this process could not open it",
    why: "`req/56` §2's declared file is present and is a regular file; the operating system \
          refused to open it for a reason other than `NotFound`, which is completely classified.",
};

/// The `VALIDATION_ERROR` row — 44 §2.3's word for a request this binary cannot attempt.
pub const ROW_VALIDATION_ERROR: RefusalRow = RefusalRow {
    arm: "Usage | Malformed | Layout",
    class: RefusalClass::Declared,
    code: "VALIDATION_ERROR",
    title: "the request is not one this binary can attempt",
    why: "the arguments, or a file this binary was told to read as one shape, do not describe an \
          operation.",
};

/// The `NOT_FOUND` row — 44 §1.4's 6, and the one row of this map whose exit is not 1.
pub const ROW_NOT_FOUND: RefusalRow = RefusalRow {
    arm: "NotFound | Io(ErrorKind::NotFound) | Engine(NotFound { what: transformation | draft })",
    class: RefusalClass::Declared,
    code: "NOT_FOUND",
    title: "the named object is not here",
    why: "the caller named something and it is not there. Declared cli_exit 6, and 6 is what \
          `Error::exit_code` answers on all three roads (E-M4-35's \"cannot be read\" vs \"does not \
          exist\"). \u{1f534} `req/38` §225 ruling 5 added the third: `gx-api` has answered \
          `Engine::NotFound` with this word since M6-09 and this binary answered `INTERNAL` / 1, so \
          the two faces of one system held two names for one refusal.",
};

/// The `BUSY` row (DR-43-2) — the second of the residual two.
pub const ROW_BUSY: RefusalRow = RefusalRow {
    arm: "Engine(Busy)",
    class: RefusalClass::Operational,
    code: "BUSY",
    title: "another gx process is writing to this project",
    why: "capable, well and momentarily excluded. The correct response is to send the same thing \
          again, which is neither a declaration to change nor a check to read.",
};

/// The `PRECONDITION_CHANGED` row for a CAS that ran and failed (DR-43-1).
pub const ROW_WORLD_MOVED: RefusalRow = RefusalRow {
    arm: "Engine(WorldMoved)",
    class: RefusalClass::Verified,
    code: "PRECONDITION_CHANGED",
    title: "the world moved after the transformation being undone committed",
    why: "the signed postcondition was compared with the live world and they differ.",
};

/// The `PRECONDITION_CHANGED` row for a CAS that could not run (R3, `req/222` H-01/H-02).
pub const ROW_WITNESS_MISSING: RefusalRow = RefusalRow {
    arm: "Engine(WitnessMissing)",
    class: RefusalClass::Verified,
    code: "PRECONDITION_CHANGED",
    title: "the undo's precondition could not be checked, so it was not fired",
    why: "one code, two titles (R3): the client branches on the code and the person reads the \
          title, and \"the world moved\" would send them after a third party who does not exist.",
};

/// 44 §2.3's honest "unclassified", and what is left in it.
pub const ROW_INTERNAL: RefusalRow = RefusalRow {
    arm: "Io | Witness | Log | Engine | Gate (the rest)",
    class: RefusalClass::Internal,
    code: "INTERNAL",
    title: "the operation could not be completed",
    why: "44 §2.3's word for what cannot be classified. `gx-api`'s `gx_code::folds()` is the list \
          of what lands here on the other face and why; `req/307` §3 carries this lane's own.",
};

/// 🔴 **R21 / `req/306` §1 item 1** — every row [`Error::refusal`] can answer with.
///
/// The length is in the type on purpose, the shape `DECLARATION_REFUSALS` and `HAND2_EXITS` both
/// use: a lane that adds an arm without a row does not compile, and a lane that adds a row nothing
/// answers with is red in `crates/gx-cli/tests/r21_refusal_map_is_whole.rs`, which drives one
/// `Error` value per row and holds the census both ways.
pub const REFUSAL_MAP: [RefusalRow; 18] = [
    ROW_LEDGER_DISAGREES,
    ROW_INVERSE_UNAVAILABLE,
    ROW_ESCROW_ABSENT,
    ROW_ADAPTER_ERROR,
    ROW_DECLARATION_UNREADABLE,
    ROW_DECLARATION_ABSENT,
    ROW_CONFIG_ABSENT,
    ROW_JOURNAL_ABSENT,
    ROW_OUTPUT_FAILED,
    ROW_HISTORY_LOST,
    ROW_LAYOUT_BLOCKED,
    // 🔴 DR-B (`req/38` §337, `req/565` §3) — the thirteenth word.
    ROW_JOURNAL_UNREADABLE,
    ROW_VALIDATION_ERROR,
    ROW_NOT_FOUND,
    ROW_BUSY,
    ROW_WORLD_MOVED,
    ROW_WITNESS_MISSING,
    ROW_INTERNAL,
];

impl Error {
    /// 44 §1.4's status for this refusal.
    ///
    /// # 🔴 The mapping is deliberately coarse, and hand 5 owns the fine one
    ///
    /// req/88 §3 Λ4: exit codes and `gx_code` are a **quotient** of the refusal space and
    /// "information is always lost in the fold" (sem: SEM-gx-cli-154), so the discipline is not "do not fold" but "write down what you folded". What this hand folds:
    /// every refusal from gx-witness, gx-log and gx-engine that is not a missing object arrives at
    /// 1, because 44 §1.2 gives the read side no other code for them — `gx log` has exactly `0` and
    /// `1`. The refusals that deserve a code of their own are listed in the report and are M6-09's
    /// material, which hand 5 turns into the `gx_code` table.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::NotFound { .. } => exit::NOT_FOUND,
            // 🔴 A file that is not there is 44 §1.4's 6 and not its 1. The case that made this
            // explicit is "this directory has no `.gx/`" (sem: SEM-gx-cli-157): `Layout::open` fails with an `ErrorKind::
            // NotFound` on `VERSION`, and reporting that as an internal error told an operator that
            // gx had broken when what had happened was that they were in the wrong directory.
            // "cannot be read" and "does not exist" are different answers (E-M4-35; sem: SEM-gx-cli-158) and this is the second of them.
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                exit::NOT_FOUND
            }
            // 🔴 **DR-43-1, adopted (a)** (`req/38` §132 ruling 2) — the second place this coarse
            // map is deliberately un-folded, and the first one where the un-folding changes the
            // *number* rather than only the `gx_code`.
            //
            // 44 §1.2 already gives `gx undo` a **3** ("commit failure … `Aborted
            // (PreconditionChanged)`"), which is what T-10a answers when the world moves between an
            // undo's own plan and its commit. `Engine::undo`'s pre-flight measures the same fact one
            // window earlier — between `T_o`'s commit and this call — so it takes the same number.
            // A script that already retried on 3 keeps working; nothing new is minted.
            // 🔴 **R3 (`req/38` §160 ruling 2)** — "the CAS could not run" takes the same number as
            // "the CAS failed". `req/222` H-01 measured the alternative: it took **0**, because the
            // undo fired. A script that retries on 3 is a script that will look at the target
            // before deciding, which is exactly the right response to both.
            Error::Engine(
                gx_engine::Error::WorldMoved { .. } | gx_engine::Error::WitnessMissing { .. },
            ) => exit::PRECONDITION_CHANGED,
            // 🔴 **`req/38` §225 ruling 5 (R23)** — the third road to 44 §1.4's **6**, in the same
            // commit as the word (`Error::refusal` below), because `EXIT_AGREEMENT`
            // (`r21_refusal_map_is_whole.rs`) makes a word moved without its number red.
            //
            // The subject is the discriminator and it is an equality rather than a `contains`:
            // `gx-engine` builds `Error::NotFound` with twenty-one subjects, and only two of them
            // name an object **the caller named**. `transformation` and `draft` are those two;
            // `adapter` (nine sites) is "this substrate has no adapter registered", which is a
            // statement about a thing nobody named and stays `INTERNAL` / 1 by `req/38` §224
            // ruling 1 — the same argument that split `DECLARATION_ABSENT` and `JOURNAL_ABSENT`
            // off `NOT_FOUND` (`req/238` H-01, `req/242` H-01 (d)). The six subjects
            // `Engine::commit_receipt`'s `missing(…)` closure builds (a provenance record, an
            // intent id, a planned delta reference, a precondition fingerprint, the `Planned`
            // record, the subject snapshot) are internal invariants of a committed row, not
            // objects a caller can name, and stay where they are for the same reason.
            //
            // Blast radius, measured rather than assumed: `req/312` §2(e) put eleven id-taking
            // verbs against well-formed ids that do not exist and got `NOT_FOUND` / 6 from all of
            // them — every one answered by `gx_cli::Error::NotFound`, this crate's own variant,
            // before the engine was reached. **`INTERNAL` was not produced once.** So this arm
            // moves no exit status any script has ever seen; what it moves is the answer on a road
            // that was reachable only in principle.
            Error::Engine(gx_engine::Error::NotFound { what, .. })
                if matches!(*what, "transformation" | "draft") =>
            {
                exit::NOT_FOUND
            }
            _ => exit::ERROR,
        }
    }

    /// 44 §1.3's stderr object: "`{"type":..., "title":..., "gx_code":..., "detail":...}` (reuses
    /// §2.5's problem+json vocabulary; the HTTP-style `status` field may be omitted in the CLI)" (sem: SEM-gx-cli-155).
    ///
    /// `gx_code` is drawn from 44 §2.3's twelve and from nowhere else —
    /// `crates/gx-cli/tests/exit_map.rs` parses that table out of the specification and refuses a
    /// code this file invented. Two of the twelve are all this hand can honestly reach:
    /// `VALIDATION_ERROR` and `NOT_FOUND`. Everything else the read side can fail with is an
    /// internal or adapter refusal, and 44 §2.3 has `INTERNAL` "an internal error that cannot be
    /// classified" (sem: SEM-gx-cli-156) for
    /// that — used as the honest "unclassified" rather than as a bucket, with the list of what
    /// lands in it written in the report.
    ///
    /// 🔴 **R21 / `req/304` D5 (`req/306` §1 item 1)** — the pair `(gx_code, title)` is no longer
    /// chosen here. It is read off [`Error::refusal`], whose rows are [`REFUSAL_MAP`], so that the
    /// question "which refusals wear which word, and which of them is an unclassified fault"
    /// is answerable from a table rather than from reading a `match`. The four keys on the wire
    /// are unchanged: 44 §2.3 fixes `ProblemDetail`'s member set and `crates/gx-api/tests/
    /// wire_census.rs` pins it, so [`RefusalClass`] is a fact about the source and **never** a
    /// fifth key.
    #[must_use]
    pub fn problem(&self) -> serde_json::Value {
        let row = self.refusal();
        let (gx_code, title) = (row.code, row.title);
        serde_json::json!({
            "type": format!("https://glovrex.dev/errors/{}", gx_code.to_lowercase().replace('_', "-")),
            "title": title,
            "gx_code": gx_code,
            "detail": self.to_string(),
        })
    }

    /// 🔴 **R21 / `req/306` §1 item 1** — which row of [`REFUSAL_MAP`] this refusal is.
    ///
    /// One `match`, one row per arm, **no `_` arm**: the shape M6-09 asked of `gx-api`'s own
    /// map ("one line in the mapping table per refusal, no `_` arm"), applied to the face that
    /// never got one. What [`Error::problem`] used to choose inline is chosen here, so that
    /// "which refusals are unclassified faults" is a question a reader answers from a table
    /// and a probe answers from `REFUSAL_MAP`.
    ///
    /// 🔴 The guarded arms are ordered, and the order is load-bearing: `Malformed { what:
    /// "project" }` before the general `Malformed`, an `Io` whose kind is `NotFound` before
    /// the general `Io`, and the two `Engine` rows R21 adds before the general `Engine(_)`.
    #[must_use]
    pub fn refusal(&self) -> RefusalRow {
        match self {
            // 🔴 **DR-43-6 (`req/38` §156 ruling 2(a))** — the same word on both faces.
            //
            // `Session::settle` raises `Malformed { what: "project" }` when the journal and the
            // ledger describe different trees, and it is the **only** site in this crate that uses
            // that `what` (the discriminator is a `&'static str` set in one place, which is what
            // makes matching on it a reference rather than a guess). Before this ruling the CLI
            // answered `VALIDATION_ERROR` and gx-api answered `INTERNAL` for one condition, so a
            // Tauri proxy holding both had two names and neither was the fact. The exit status is
            // unchanged at 44 §1.4's **1** — `req/38` §148's "no new exit number" holds — so this
            // is the `BUSY` shape again: the fold is undone on stderr and not in the status.
            Error::Malformed {
                what: "project", ..
            } => ROW_LEDGER_DISAGREES,
            // 🔴 **R9 / `req/236` H-04** — the project's declaration, with a word of its own.
            //
            // `VALIDATION_ERROR` is 44 §2.3's "the request is not one this binary can attempt", and
            // the request was `gx repair`. What could not be attempted was reading a two-line text
            // file that nobody had asked about. The fold is undone here for `BUSY`'s reason: the
            // correct response is specific and is not "your command was wrong".
            // 🔴 **R9 / `req/236` M-01** — 44 §2.3's own row, on the face that was not using it.
            Error::InverseUnavailable { .. } => ROW_INVERSE_UNAVAILABLE,
            Error::Declaration { .. } => ROW_DECLARATION_UNREADABLE,
            // 🔴 **R10 / `req/238` H-01** — the two absences, each with a word of its own.
            //
            // Not folded into `DECLARATION_UNREADABLE`: that code's own row says "**present** and
            // does not read", and its remedy is "write these two lines". The remedy for an absent
            // declaration is a different one (`gx repair --yes`, which says what it did), and the
            // remedy for an absent `config.toml` names a key the operator has to put back. Not
            // folded into `NOT_FOUND` either, which is 44 §1.4's 6 for "the object you named is not
            // here" — nobody named this file.
            Error::DeclarationAbsent { .. } => ROW_DECLARATION_ABSENT,
            Error::ConfigAbsent { .. } => ROW_CONFIG_ABSENT,
            // 🔴 **R12 / `req/242` H-01 (d)** — the third absence, with a word of its own.
            //
            // Not `NOT_FOUND`: 44 §1.4's 6 means "the object you named is not here" and nobody
            // named this file. Not `INTERNAL`: an append with nothing to append to is entirely
            // classifiable, and `gx repair` has a report and a remedy for it.
            Error::JournalAbsent { .. } => ROW_JOURNAL_ABSENT,
            // 🔴 **R13 / `req/244` H-01 + M-04** — the two words minted here.
            //
            // Neither is `INTERNAL`. "The answer could not be delivered" is entirely classifiable
            // (the operating system said which way), and so is "this project holds entries and no
            // witness of a commit". 44 §2.3 keeps `INTERNAL` for what cannot be classified, and
            // R12's audit found both of these wearing a number (101) and a word (`INTERNAL`) that
            // told an operator gx had broken.
            Error::OutputFailed { .. } => ROW_OUTPUT_FAILED,
            Error::HistoryLost { .. } => ROW_HISTORY_LOST,
            // 🔴 **R14 / `req/246` M-04** — the word minted here, and the third `INTERNAL` this
            // family has taken back.
            //
            // Same argument, one directory over: "a path that is not a directory is where a
            // declared directory belongs" is completely classified — the operating system named
            // the path and the reason — and `INTERNAL` is 44 §2.3's word for the opposite. The
            // remedy is a move, and gx does the move under `--yes` rather than a delete.
            Error::LayoutBlocked { .. } => ROW_LAYOUT_BLOCKED,
            Error::Usage { .. } | Error::Malformed { .. } | Error::Layout { .. } => {
                ROW_VALIDATION_ERROR
            }
            Error::NotFound { .. } => ROW_NOT_FOUND,
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                ROW_NOT_FOUND
            }
            // 🔴 **DR-43-2 / `req/38` §148** — the one engine refusal with a word of its own.
            //
            // Exit stays 44 §1.4's **1**: §148 rules that no new exit number is minted, so the
            // status a script branches on cannot tell "another gx is writing" from "invalid input".
            // The `gx_code` on stderr can, and this line is where the fold stops being total. That
            // is the standing discipline for this map (req/88 §3 Λ4): fold, and write down what was
            // folded — here the fold is *undone* for the one case whose correct response is a retry
            // rather than a bug report, and gx-api answers the same code with `503` + `Retry-After`.
            Error::Engine(gx_engine::Error::Busy { .. }) => ROW_BUSY,
            // 🔴 **DR-43-1, adopted (a)** — 43 §5.2's `world-moved` row. 44 §2.3 already owns the
            // code and the status (`PRECONDITION_CHANGED`, 409, exit 3), so the fold stops here for
            // the same reason it stops for `BUSY`: a caller's correct response is specific
            // ("something else changed the target — look, then decide"), and `INTERNAL` would have
            // told them this binary was broken.
            Error::Engine(gx_engine::Error::WorldMoved { .. }) => ROW_WORLD_MOVED,
            // 🔴 **R3 (`req/222` H-01/H-02)** — one code, two titles. The code is what a client
            // branches on and 44 §2.3 has one for "the CAS did not pass"; the title is what a
            // person reads, and telling them the world moved when the truth is that the evidence is
            // gone would send them looking for a third party who does not exist.
            Error::Engine(gx_engine::Error::WitnessMissing { .. }) => ROW_WITNESS_MISSING,
            // 🔴 **R21 / `req/304` §0.5 (`req/306` §1 item 1)** — the fourth `INTERNAL` this
            // family takes back, and the first one that was never this crate's to keep.
            //
            // `gx-api`'s `gx_code::REFUSALS` maps `Engine::Adapter` onto `ADAPTER_ERROR`
            // with `fold: None`, and `gx_engine::Error::Adapter`'s own doc comment names
            // the same row. This crate had no map at all — one arm carried `Io`,
            // `Witness`, `Log`, `Engine` and `Gate` together — so a relative `--locator`
            // came back "the operation could not be completed" while the HTTP face said
            // which layer refused. See `ROW_ADAPTER_ERROR` for why the class is
            // `Declared` and what it deliberately does not split.
            Error::Engine(gx_engine::Error::Adapter { .. }) => ROW_ADAPTER_ERROR,
            // 🔴 **R21 / `req/304` §0.8** — the escrow road R9 did not repair.
            //
            // Matched on the **subject** and not on the sentence `req/304` printed:
            // `gx-engine` names an absent escrowed inverse three ways and only one of
            // them was measured. See `ESCROWED_INVERSE_SUBJECT`.
            //
            // Above the general `Engine(_)` arm and below nothing else that could claim
            // it: an `Engine::NotFound` whose subject is **not** an escrowed inverse
            // (a transformation, a draft, a blob, an adapter) is left where it was,
            // because `gx-api` answers it `NOT_FOUND` whose declared `cli_exit` is 6 and
            // this binary exits 1 there — moving the word without the number would put a
            // code and an exit that disagree on one refusal, and moving the number is an
            // exit-status change `req/306` §1 forbids. Filed in `req/307` for a ruling.
            Error::Engine(gx_engine::Error::NotFound { what, .. })
                if what.contains(ESCROWED_INVERSE_SUBJECT) =>
            {
                ROW_ESCROW_ABSENT
            }
            // 🔴 **`req/38` §225 ruling 5 (R23)** — the word this road should always have worn.
            //
            // The comment on the arm above says why R21 left it alone: *"`gx-api` answers it
            // `NOT_FOUND` whose declared `cli_exit` is 6 and this binary exits 1 there — moving the
            // word without the number would put a code and an exit that disagree on one refusal,
            // and moving the number is an exit-status change `req/306` §1 forbids"*. Both halves
            // move here, in one commit, and `EXIT_AGREEMENT` is the gate that makes "one commit" a
            // property rather than a promise. `Error::exit_code` carries the same predicate, and
            // the reason it is an equality over two subjects rather than the whole family is
            // written there.
            //
            // Below the escrow arm and above the general `Engine(_)`: the three escrowed-inverse
            // subjects are also `Engine::NotFound`, they are `INVERSE_UNAVAILABLE` by `req/304`
            // §0.8, and their subjects are sentences rather than the two words this arm equals — so
            // the order is belt and braces rather than load-bearing.
            Error::Engine(gx_engine::Error::NotFound { what, .. })
                if matches!(*what, "transformation" | "draft") =>
            {
                ROW_NOT_FOUND
            }
            // 🔴 **R40 / `req/553` M-02 (`req/38` §322-2 (11-4)) — a document the caller handed
            // this binary, wearing the word the map already declared for it.**
            //
            // Audit 39 asked one question of one system twice. Take a required member out of a
            // **ledger** record and re-encode: `gx log proof` and `gx log checkpoint` answer exit 1
            // `VALIDATION_ERROR`. Take a required member out of a **receipt file** and re-encode
            // canonically: `gx receipt verify` answers exit 1 `INTERNAL`. One condition — "a file
            // this binary was told to read as one shape does not hold that shape" — and two words,
            // which is `req/38` §156 ruling 2(a)'s failure across a pair of roads rather than a
            // pair of faces.
            //
            // Nothing is minted here. `ROW_VALIDATION_ERROR`'s own `why` already claims this
            // condition in as many words — *"the arguments, **or a file this binary was told to
            // read as one shape**, do not describe an operation"* — and `gx_witness::Error::Schema`
            // is documented on its own variant as "well-formed CBOR and not a legal receipt". The
            // row was declared and the arm never arrived; R40 is the arm arriving. Exit does not
            // move (`VALIDATION_ERROR` and `INTERNAL` both declare 44 §1.4's **1**), so `req/38`
            // §148 and `req/306` §1 are untouched, and `REFUSAL_MAP` stays seventeen rows.
            //
            // 🔴 The position is load-bearing, twice over. Above the general `Error::Witness(_)`
            // arm, which still carries everything else gx-witness refuses to `INTERNAL`; and above
            // the `Error::Io` catch-all below, which is a different fact (the file would not read
            // at all, rather than read and did not describe a receipt).
            //
            // 🔴 What is deliberately **not** folded in: `SignatureInvalid`, `KeyFormat` and the
            // permission refusal. A receipt whose signature does not verify is a legal receipt that
            // fails its check — `req/558` AC-9's negative control drives exactly that and expects
            // the answer **not** to move — and calling it a validation error would tell a caller to
            // fix their file when what they have is a proof that something is wrong.
            Error::Witness(gx_witness::Error::Schema { .. } | gx_witness::Error::Canon(_)) => {
                ROW_VALIDATION_ERROR
            }
            // 🔴 **DR-B / `req/38` §337 (`req/565` §3)** — the thirteenth word, taken back from
            // `INTERNAL`. Above the general `Io`/`Witness`/`Log`/`Engine`/`Gate` catch-all and
            // below nothing else that could claim it: this arm exists only for the value
            // `crate::ledger::journal_unreadable` builds, and it builds that value only when
            // `crate::layout::presence_of` has already established the journal is present and a
            // regular file (see that function's own doc comment for the guard).
            Error::JournalUnreadable { .. } => ROW_JOURNAL_UNREADABLE,
            Error::Io { .. }
            | Error::Witness(_)
            | Error::Log(_)
            | Error::Engine(_)
            | Error::Gate(_) => ROW_INTERNAL,
        }
    }
}

/// The CLI's own result type.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(action: &'static str, path: &std::path::Path) -> impl Fn(std::io::Error) -> Error {
    let path = path.display().to_string();
    move |source| Error::Io {
        action,
        path: path.clone(),
        source,
    }
}
