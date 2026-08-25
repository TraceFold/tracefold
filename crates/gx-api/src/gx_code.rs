// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-09** — 44 §2.3's twelve `gx_code`s, and the one place every refusal is mapped onto them.
//!
//! > **(a)** put the mapping table in **one place**+fold an unmapped refusal into `INTERNAL`(500), **one line in
//! > the doc for the folded fact** … impose on the mapping table **one line per refusal, no `_` arm** (the shape of E-M2-23 / H-3), and shape it so that
//! > a compile error results the moment the engine gains a variant (sem: SEM-gx-api-058)
//!
//! # 🔴 The quotient, with its denominator (req/88 §3 Λ4)
//!
//! Λ4's claim is that this map "is a quotient map of the refusal space, neither surjective nor injective; information
//! is always lost" (sem: SEM-gx-api-059), and that the discipline is therefore not "don't fold" but "write down what was folded". So the
//! denominator is counted rather than described:
//!
//! | source | refusal kinds |
//! |---|---|
//! | `gx_engine::ERROR_KINDS` | 16 |
//! | `gx_gate::ERROR_KINDS` | 6 |
//! | `gx_witness::ERROR_KINDS` | 10 |
//! | `gx_log::ERROR_KINDS` | 5 |
//! | **total** | **37** |
//!
//! 🔴 **P2 item2** (`req/130` §1, key-at-rest encryption, NFR-010) added two of gx-witness's ten:
//! `KeyEncrypted` and `WrongPassphrase`. Neither is reachable through the HTTP surface today (no
//! endpoint loads a key with a passphrase — `req/131` §3 names that a residual rather than a silent
//! gap), so both fold to `INTERNAL` for the same reason [`Origin::Witness`]'s other operational rows
//! already do.
//!
//! onto 44 §2.3's **12**. [`REFUSALS`] carries one row per refusal kind, all thirty-seven, and
//! [`folds()`] is the list Λ4 asks to be written down: the rows whose meaning does not survive the
//! code they land on.
//!
//! # 🔴 The `_` arm, and the honest form of "compile error" (sem: SEM-gx-api-060)
//!
//! §47's ruling asks that "the engine gaining a variant becomes a compile error" (sem: SEM-gx-api-061). Three of the four enums are
//! `#[non_exhaustive]` (gx-engine, gx-witness, gx-log), which means **a `match` written in this
//! crate is required by the language to have a wildcard arm** — the exact shape the ruling forbids.
//! There is no way to have both from outside those crates.
//!
//! So the map does not `match` at all. Each crate declares its own vocabulary as an array
//! (E-M2-23's "declare each crate's Error vocabulary in one place"; sem: SEM-gx-api-062) and answers `Error::kind()`, whose `match` lives
//! **inside** the defining crate where `#[non_exhaustive]` does not bind and where no `_` arm is
//! written. This file maps `&'static str` to code, and the compile error the ruling asks for is a
//! [`const` assertion](DENOMINATOR) against each array's length: a new variant obliges its own crate
//! to grow its array (each crate's own probe enforces that), and the moment the array grows, this
//! file stops compiling.
//!
//! 🔴 Two of those arrays did not exist before this hand — gx-witness's and gx-log's — and their
//! absence is why nothing had ever counted the denominator. Raised as **M6H5-2**.
//!
//! # What is *not* here
//!
//! `IDEMPOTENCY_CONFLICT` and `UNAUTHORIZED` are 44 §2.3 codes with no refusal kind behind them:
//! they are produced by [`crate::idempotency`] and [`crate::auth`], which are HTTP-layer facts and
//! not any crate's `Error`. They are in [`GX_CODES`] and absent from [`REFUSALS`], and the two
//! tables' disagreement on exactly those two is asserted rather than tolerated.

/// One row of 44 §2.3's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GxCode {
    /// The machine-readable code (44 §2.3: "a machine-readable code (shares its vocabulary
    /// with the CLI exit code and `Reason.code`)"; sem: SEM-gx-api-063).
    pub code: &'static str,
    /// The HTTP status 44 §2.3 pairs it with.
    pub status: u16,
    /// The CLI exit status 44 §2.3's third column gives it.
    pub cli_exit: u8,
    /// 44 §2.3's own fourth column, verbatim.
    pub meaning: &'static str,
}

/// 🔴 44 §2.3's twelve, transcribed **once**.
///
/// `probes/doubt/tests/m6_gx_code.rs` parses the same table out of
/// `req/spec/40-architecture/44-api-spec.md` and compares all four columns, which is the I-11 shape
/// (`adapter_spec.rs`: "parse the canonical markdown and cross-check it against the real type"; sem: SEM-gx-api-064). A table that lived only here
/// would be a second copy of 44 §2.3, and two copies of a table are two things that drift — which is
/// precisely what M6-25 found between 44 §1.2's per-command lists and §1.4's common table.
pub const GX_CODES: [GxCode; 12] = [
    GxCode {
        code: "VALIDATION_ERROR",
        status: 422,
        cli_exit: 1,
        meaning: "リクエスト不正",
    },
    GxCode {
        code: "NOT_FOUND",
        status: 404,
        cli_exit: 6,
        meaning: "対象ID不在",
    },
    GxCode {
        code: "NOT_ADMITTED",
        status: 403,
        cli_exit: 2,
        meaning: "Verdict≠Admitでcommit不可（非record-only）",
    },
    GxCode {
        code: "PRECONDITION_CHANGED",
        status: 409,
        cli_exit: 3,
        meaning: "CAS失敗（CON-2）",
    },
    GxCode {
        code: "APPLY_FAILED",
        status: 422,
        cli_exit: 5,
        meaning: "adapter.apply失敗",
    },
    GxCode {
        code: "ESCALATION_PENDING",
        status: 409,
        cli_exit: 4,
        meaning: "Escalated状態のまま操作不可",
    },
    GxCode {
        code: "INVERSE_UNAVAILABLE",
        status: 409,
        cli_exit: 1,
        meaning: "undo対象の逆deltaが利用不可",
    },
    GxCode {
        code: "IDEMPOTENCY_CONFLICT",
        status: 409,
        cli_exit: 1,
        meaning: "同一Idempotency-Keyで異なるリクエスト本文",
    },
    GxCode {
        code: "ADAPTER_ERROR",
        status: 502,
        cli_exit: 1,
        meaning: "substrate adapter内部エラー",
    },
    GxCode {
        code: "POLICY_ERROR",
        status: 500,
        cli_exit: 1,
        meaning: "Cedar評価自体の失敗（policyバグ等）",
    },
    GxCode {
        code: "UNAUTHORIZED",
        status: 401,
        cli_exit: 1,
        meaning: "認証失敗（§2.6）",
    },
    GxCode {
        code: "INTERNAL",
        status: 500,
        cli_exit: 1,
        meaning: "分類不能な内部エラー",
    },
];

/// 🔴 44 §2.2's own name for a transition asked for from the wrong state, which §2.3's table omits.
///
/// `POST /candidates/{id}/verify`, `/escalation` and `/cancel` all specify "`409` (…,
/// `gx_code=INVALID_STATE`)" (sem: SEM-gx-api-065) in §2.2, and §2.3's twelve-row table does **not** contain it. That is a
/// gap inside one document rather than a choice this hand gets to make, so the constant is named
/// here, used where §2.2 names it, and raised as **M6H5-3**. `GX_CODES` is left at twelve because it
/// is a transcription of §2.3 and a transcription that quietly gained a row would stop being one.
pub const INVALID_STATE: &str = "INVALID_STATE";

/// The status 44 §2.2 gives [`INVALID_STATE`] at every one of its three sites.
pub const INVALID_STATE_STATUS: u16 = 409;

/// 🔴 **E-M6-22** (§53, M6H6-4, adopted (a); sem: SEM-gx-api-066) — the code 44 has no word for: *the server is going away*.
///
/// > **M6H6-4, adopted (a) = E-M6-22**: read `UNAVAILABLE` (503, CLI 1) as a backward-compatible addition to 44 §2.3
/// > (the absent operational code = the same root solution as hand 5's KeyPermissions). Implementation window = the first hand from hand 7 onward that touches it (sem: SEM-gx-api-067).
///
/// 44 §2.3's twelve codes are twelve words about a **request** — "malformed request", "target id absent",
/// "Verdict≠Admit" (sem: SEM-gx-api-068) — and not one of them is a word about a **server**. Hand 5 hit the wall folding
/// `KeyPermissions` and hand 6 hit it again when graceful shutdown's stage 1 had to refuse a request
/// the server was perfectly capable of serving a second earlier. Both folded to `INTERNAL`, both said
/// so, and §53 ruled the addition.
///
/// 🔴 What the fold cost, stated as behaviour rather than as tidiness: a client's retry policy reads
/// `gx_code`. `INTERNAL` means "this server made a mistake" and invites a bug report;
/// `UNAVAILABLE` means "this server is leaving and the replacement will answer" (sem: SEM-gx-api-069) and invites a
/// retry. One of those two instructions was wrong for every shutdown this server has ever performed.
pub const UNAVAILABLE: &str = "UNAVAILABLE";

/// The status [`UNAVAILABLE`] carries. Unchanged by the erratum: hand 6's status was already honest
/// and only the code was a fold.
pub const UNAVAILABLE_STATUS: u16 = 503;

/// 44 §1.4's exit for [`UNAVAILABLE`] — **1**, "error (bad input, internal error, adapter error)" (sem: SEM-gx-api-070).
///
/// 🔴 There is no exit value for "the server went away" (sem: SEM-gx-api-071), which is M6H6-3's other half: 44 gives
/// `gx serve` 0 and 1 and nothing else, so the CLI column of this row is itself a fold. Named here
/// so that the fold is one line of code somebody can find rather than a sentence in one report.
pub const UNAVAILABLE_CLI_EXIT: u8 = 1;

/// 🔴 **M-14** (`req/182` §1-2, repaired in `req/189`) — the code 44 has no word for: *the body is
/// bigger than this server reads*.
///
/// The limit is [`crate::MAX_BODY_BYTES`] (axum's default, now declared rather than inherited) and
/// 44 §2.2 gained a sentence about it. Before this row a 3 MiB `POST /candidates` was answered
/// `422 VALIDATION_ERROR` "length limit exceeded" — problem+json, loud, and the wrong status: 422
/// tells a client its body was read and refused, 413 tells it the body was not read at all and a
/// smaller one might be. RFC 9110 §15.5.14 has the word; 44 §2.3's twelve rows do not.
pub const PAYLOAD_TOO_LARGE: &str = "PAYLOAD_TOO_LARGE";

/// The status [`PAYLOAD_TOO_LARGE`] carries.
pub const PAYLOAD_TOO_LARGE_STATUS: u16 = 413;

/// 🔴 **L-04 / H-10** (`req/182` §1-1/§1-3, repaired in `req/189`) — the code 44 has no word
/// for: *a body arrived that this server cannot read as JSON at all*, because its `Content-Type`
/// is missing or names another media type.
///
/// Two roads meet here and were two different wrong answers before: a `Content-Type: text/plain`
/// body was `422 VALIDATION_ERROR` (L-04, right shape, wrong status — RFC 9110 §15.5.16 is 415),
/// and a body with **no** `Content-Type` on an endpoint whose body is optional was **silently
/// dropped** (H-10: `{"record_only":false}` read as "no body" and therefore as the default posture
/// — the typo-becomes-enforcement-decision `extract.rs` was written to forbid). One condition, one
/// answer: a body that is not declared JSON is refused with this code, on every endpoint.
pub const UNSUPPORTED_MEDIA_TYPE: &str = "UNSUPPORTED_MEDIA_TYPE";

/// The status [`UNSUPPORTED_MEDIA_TYPE`] carries.
pub const UNSUPPORTED_MEDIA_TYPE_STATUS: u16 = 415;

/// 🔴 **DR-43-2 / `req/38` §148** — the code 44 has no word for: *another writer holds this project*.
///
/// 44 §2.3's twelve codes and `UNAVAILABLE` between them describe a bad request, a refused
/// transition and a server that is leaving. None of them describes a server that is **capable, well
/// and momentarily excluded**. `req/182` H-01 measured what the absence cost before there was a lock
/// at all: two `gx` processes writing to one `.gx/` produced a ledger whose second leaf answered
/// `found: false` to its own inclusion proof, and nothing anywhere said "wait".
///
/// The status is `503` and not `409`: `409` is 44's word for a **conflict in the request** (a
/// precondition that changed, an escalation still pending, an idempotency key reused), and the
/// caller's correct response to those is to look at what they sent. The caller's correct response
/// here is to send exactly the same thing again in a moment, which is what `503` plus a
/// `Retry-After` says and what `409` does not. The CLI exit is **1**: `req/38` §148 rules that no
/// new exit number is minted for this, so the fold onto 44 §1.4's "error" is deliberate and the
/// `gx_code` on stderr is where the difference survives (the same shape `UNAVAILABLE` already has).
pub const BUSY: &str = "BUSY";

/// The status [`BUSY`] carries, and the `Retry-After` that goes with it.
pub const BUSY_STATUS: u16 = 503;

/// The `Retry-After` (seconds) a `BUSY` answer carries: one, because the lock is held for the length
/// of one engine operation and a client that waited a minute would be waiting for nothing.
///
/// 🔴 **DR-43-5 (2), `req/215` M-04 — one is the floor of the unit, not a measurement.** R1's own
/// documentation called one second "not a guess about load — an upper bound on what is being waited
/// for", and `req/215` measured the thing being waited for: sampling `.gx/LOCK` every 2 ms across
/// one CLI commit (four processes) found **53 blocked samples ≈ 200 ms for four verbs ≈ 50 ms a
/// verb**, and ten writers retrying every 50 ms all won inside half a second. One second is a true
/// upper bound and a bad instruction: a client obeying it literally takes **ten seconds** to reach
/// the same place. `Retry-After` is RFC 9110 delay-seconds and has no finer unit, so the header
/// stays `1` and the measurement is named in [`BUSY_RETRY_AFTER_MILLIS`] and said out loud in the
/// refusal's `detail`.
pub const BUSY_RETRY_AFTER_SECONDS: u32 = 1;

/// 🔴 **DR-43-5 (2) / `req/215` M-04** — how long the lock is actually held, measured, in
/// milliseconds.
///
/// The number a client should schedule its retry on, and the reason [`BUSY_RETRY_AFTER_SECONDS`] is
/// twenty times larger: the header's unit cannot say it. Carried in the refusal's `detail` rather
/// than as a sixth key of the object, because 44 §2.3 fixes `ProblemDetail`'s member set and
/// `crates/gx-api/tests/wire_census.rs` pins it — a new extension member is a wire-shape change and
/// therefore a DR of its own, raised in the DR-43-5 filing rather than taken here.
pub const BUSY_RETRY_AFTER_MILLIS: u32 = 50;

/// 🔴 **DR-43-5 (2) / `req/215` M-10** — the title `BUSY` carries, on both faces.
///
/// The CLI answered `"another gx process is writing to this project"` and this crate answered
/// `"the operation was refused"` for the same refusal, so a Tauri proxy holding both had two names
/// for one thing. The CLI's is the one that says something, so it is the one that stayed.
pub const BUSY_TITLE: &str = "another gx process is writing to this project";

/// 🔴 **DR-43-6 (`req/38` §156 ruling 2(a))** — the code 44 has no word for: *this project's two
/// files describe different trees*.
///
/// R1b closed the hole `req/215` H-01 measured — a server whose ledger had been cut under it
/// answered `201`, `200` and `200 with a signed receipt`, and signed a checkpoint claiming three
/// leaves over a file that held none — by making `ledger_agrees` a gate both writers pass. What it
/// could not do was **name** the refusal: 44 §2.3's twelve words are all about a request, and the
/// three that could have been borrowed are each a lie of a different kind. `BUSY` says "send the
/// same thing again in a moment", and there is no moment at which this becomes true without a
/// repair. `VALIDATION_ERROR` is where `gx_engine::Error::Malformed` maps, and blames a caller for
/// the state of the server's disk. `UNAVAILABLE` says "this server is going away and a replacement
/// will answer"; the replacement would refuse to start.
///
/// So R1b rode on `INTERNAL` and said the whole of it in `detail`, and filed the shape of the word
/// it wanted. This is that word. `500` and not `503`: `503` invites a retry, and the correct
/// response here is `gx replay <ID>` and an operator — the answer will not change on its own.
///
/// # 🔴 What it does **not** mean
///
/// Not "the journal is corrupt" and not "the ledger is corrupt". It means the two disagree, and
/// which of them is right is a question this server does not answer: the refusal names both counts
/// (frontier and leaves) so that the person reading it can. A code that claimed to know would be
/// the surface deciding, on no evidence, which file to trust.
pub const LEDGER_DISAGREES: &str = "LEDGER_DISAGREES";

/// The status [`LEDGER_DISAGREES`] carries.
pub const LEDGER_DISAGREES_STATUS: u16 = 500;

/// The title [`LEDGER_DISAGREES`] carries, on both faces.
///
/// [`BUSY_TITLE`]'s rule: a proxy speaking both must not hold two names for one refusal. The CLI's
/// own refusal for this condition is `gx_cli::session`'s `settle`, whose `Error::Malformed`
/// message begins with the same fact.
pub const LEDGER_DISAGREES_TITLE: &str =
    "this project's journal and ledger describe different trees";

/// 🔴 **R9 / `req/236` H-04** — this project's `.gx/VERSION` is there and is not a declaration.
///
/// The file req/56 §2 calls `Nature::Meta` is also the file that says which framing this project's
/// journal is in and whose digest is bound into the recorded head — so a binary that cannot read it
/// cannot open the project. Until R9 that refusal came out as `VALIDATION_ERROR`, 44 §2.3's word
/// for "the request is not one this binary can attempt", on requests that were perfectly ordinary.
/// `req/236` H-04 measured five byte shapes an editor produces (a byte-order mark, a leading blank
/// line, bare-CR endings, a UTF-16 save, two swapped lines) taking `gx repair`, `gx log proof`,
/// `gx replay` and `gx serve` down together with **no report, no remedy and no way out**.
///
/// Four of the five are read now (`gx_log::head::declaration_lines`). This code is what is left:
/// bytes that are not text, and a file with no layout-version line at all. `500` rather than `422`
/// for `LEDGER_DISAGREES`' reason — nothing about the request was wrong, and a retry will not
/// change the answer; what has to change is a file on the server's disk.
pub const DECLARATION_UNREADABLE: &str = "DECLARATION_UNREADABLE";

/// The status [`DECLARATION_UNREADABLE`] carries.
pub const DECLARATION_UNREADABLE_STATUS: u16 = 500;

/// The title [`DECLARATION_UNREADABLE`] carries, on both faces.
pub const DECLARATION_UNREADABLE_TITLE: &str =
    "this project's `.gx/VERSION` does not read as a declaration";

/// 🔴 **R10 / `req/238` H-01** — this project's `.gx/VERSION` is **not there**.
///
/// [`DECLARATION_UNREADABLE`]'s row is "present and does not read"; this is the other half, and
/// until R10 it had no word at all. It came out as 44 §1.4's **6** with `NOT_FOUND` — the code for
/// "the object you named is not here" — on a request that named no file, which is why `gx repair`
/// on a project that had lost its declaration exited 6 with an empty report while the ledger, the
/// journal, the receipts and the head were all perfectly readable.
///
/// `500` for [`LEDGER_DISAGREES`]' reason: nothing about the request was wrong and a retry will not
/// change the answer; what has to change is a file on the server's disk.
pub const DECLARATION_ABSENT: &str = "DECLARATION_ABSENT";

/// The status [`DECLARATION_ABSENT`] carries.
pub const DECLARATION_ABSENT_STATUS: u16 = 500;

/// The title [`DECLARATION_ABSENT`] carries, on both faces.
///
/// 🔴 **R45 / `req/621` L-3, ruling `req/38` §394** — widened with `gx_cli::ROW_DECLARATION_ABSENT`
/// so the two faces keep saying the same sentence (F-2's rule, `req/38` §337): a dangling symbolic
/// link at `.gx/VERSION` reaches this code too, and "is not there" alone was false of it.
pub const DECLARATION_ABSENT_TITLE: &str =
    "this project's `.gx/VERSION` is not there, or is a link that does not resolve";

/// 🔴 **R10 / `req/238` H-01** — this project's `.gx/config.toml` is **not there**.
///
/// 43 §7.9 (b)'s R9 row names this file as the one that decides which key a recovery signs with.
/// `req/238` H-01 measured the writer's door answering its absence by writing the shipped default,
/// at rc 0 and in silence, which is `engine_signing_keyid` going back to nothing under an operator
/// who had set it. The refusal is the writer's only — a read verb and a diagnosis both run without
/// the file (`req/227` M-03).
pub const CONFIG_ABSENT: &str = "CONFIG_ABSENT";

/// The status [`CONFIG_ABSENT`] carries.
pub const CONFIG_ABSENT_STATUS: u16 = 500;

/// The title [`CONFIG_ABSENT`] carries, on both faces.
///
/// 🔴 **R45 / `req/621` L-3, ruling `req/38` §394** — same widening as `DECLARATION_ABSENT_TITLE`,
/// at `.gx/config.toml`.
pub const CONFIG_ABSENT_TITLE: &str =
    "this project's `.gx/config.toml` is not there, or is a link that does not resolve";

/// 🔴 **R12 / `req/242` H-01 (d)** — this project's `.gx/ledger/journal` is **not there**.
///
/// The third absence, and the one the last audit measured being *erased*. R11 taught `gx repair` to
/// answer a project that had lost its journal with rc 1, the same forty-seven keys a healthy report
/// carries, and a remedy naming the backup; `req/242` H-01 (d) then measured a single `gx submit`
/// creating an empty eight-byte journal through the engine's writer door, after which the same
/// report said `journal_absent: false` and told a rollback story over the loss.
///
/// `500` for [`DECLARATION_ABSENT`]'s reason: nothing about the request was wrong and a retry will
/// not change the answer; what has to change is a file on the server's disk.
pub const JOURNAL_ABSENT: &str = "JOURNAL_ABSENT";

/// The status [`JOURNAL_ABSENT`] carries.
pub const JOURNAL_ABSENT_STATUS: u16 = 500;

/// The title [`JOURNAL_ABSENT`] carries, on both faces.
pub const JOURNAL_ABSENT_TITLE: &str = "this project's `.gx/ledger/journal` is not there";

/// 🔴 **R13 / `req/244` H-01** — this run produced an answer and could not put it on stdout.
///
/// The word exists because the failure had no word. `Outcome` guarantees that a report was
/// *composed*; `println!` is what put it on the wire, and Rust's `print!` family does not return a
/// `Result` — it panics. `req/244` H-01 measured the consequence three ways (a reader that closed
/// first, a full filesystem, a reader that took one byte): exit **101**, a Rust panic string where
/// 44 §1.3's problem object belongs, and a `.gx/VERSION` twenty-five bytes long on the disk that
/// the *next* `gx repair` reported as `meta_repaired: []`. A machine could not tell "gx answered"
/// from "gx crashed", because 101 is in no table this repository publishes.
///
/// The CLI exit is 44 §1.4's **1** — `req/38` §148's "no new exit number", as for every code minted
/// since `BUSY`. `500` on the wire for [`DECLARATION_ABSENT`]'s reason: nothing about the request
/// was wrong, and what has to change is the destination the answer was being written to.
pub const OUTPUT_FAILED: &str = "OUTPUT_FAILED";

/// The status [`OUTPUT_FAILED`] carries.
pub const OUTPUT_FAILED_STATUS: u16 = 500;

/// The title [`OUTPUT_FAILED`] carries, on both faces.
pub const OUTPUT_FAILED_TITLE: &str = "this run's answer could not be written to stdout";

/// 🔴 **R13 / `req/244` M-04** — this project has lost every witness that it ever recorded a
/// commit, and something else in it says it did.
///
/// `Layout::logged`'s three witnesses are the ledger beside the journal, the recorded head, and the
/// commit receipts. A project that has lost all three looks exactly like a directory `gx key gen`
/// has just been run in — which is what `req/242` L-04 and `req/244` M-04 measured `gx submit`
/// treating it as: a fresh journal, an empty history, and two committed transformations that no
/// verb can read afterwards. `.gx/index/`, `.gx/evidence/` and `.gx/drafts/` are not witnesses of a
/// commit and they are witnesses of *use*, so a directory that holds entries in one of them and
/// none of the three is a project whose log has gone, and the writer's door says so instead of
/// starting a second history over it.
///
/// Refused rather than warned because the alternative is unrecoverable: once a new journal is
/// written, the fact that the old one existed is nowhere. `--yes` has no road here on purpose (a
/// repair that invented a history would be worse than the loss); the remedy is the backup.
pub const HISTORY_LOST: &str = "HISTORY_LOST";

/// The status [`HISTORY_LOST`] carries.
pub const HISTORY_LOST_STATUS: u16 = 500;

/// The title [`HISTORY_LOST`] carries, on both faces.
pub const HISTORY_LOST_TITLE: &str =
    "this project has used gx and holds no witness of any commit it recorded";

/// 🔴 **R14 / `req/246` M-04** — something that is not a directory is sitting where one of `.gx/`'s
/// declared directories belongs.
///
/// # Why a word, and why this word rather than a second `INTERNAL`
///
/// `req/56` §2 and [`GX_PATHS`](../../gx_cli/layout/constant.GX_PATHS.html) declare `.gx/`'s shape,
/// and every door that writes asks the operating system for each declared directory on its way in.
/// `req/246` M-04 put **one byte** at `.gx/repair` — the row R13 added — and measured what came
/// back: `gx submit`, `gx log head` and `gx receipt list` all refused
/// `{"gx_code":"INTERNAL","detail":"create …/.gx/repair: File exists (os error 17)"}`, three runs
/// each, for ever, while `gx repair` reported the project healthy at exit 0. 44 §2.3 keeps
/// `INTERNAL` for what **cannot be classified**, and this is completely classified: the path is
/// there, it is not a directory, the declaration says it has to be one, and the remedy is to move
/// whatever is in the way.
///
/// # The predicate, not the place
///
/// `req/38` §186 ruling 2 asked for M-04 to be closed "by the predicate, not by `journal.blobs`'s
/// place", which is the standing instruction from `req/244` M-06 — the same class one directory
/// down, inside the engine's blob store, still open and still deliberately left to its own lane.
/// So this word is about **any** declared directory of `.gx/` whose path holds something else, and
/// `Layout::create` scans all of them before it makes any of them. What it does not do is reach
/// into `gx-engine`: `journal.blobs/` is created by the engine's own store and answering for it
/// here would be a second opinion about a directory this crate does not own. Written down as the
/// denominator rather than folded in.
///
/// The CLI exit is 44 §1.4's **1** — `req/38` §148's standing "no new exit number".
pub const LAYOUT_BLOCKED: &str = "LAYOUT_BLOCKED";

/// The status [`LAYOUT_BLOCKED`] carries.
pub const LAYOUT_BLOCKED_STATUS: u16 = 500;

/// The title [`LAYOUT_BLOCKED`] carries, on both faces.
///
/// 🔴 **R40 / `req/38` §328 ruling 2 ②** — widened from "a declared **directory** … that is not a
/// directory" to the sentence below, so that a declared **file** whose path holds a directory wears
/// it too (`gx-cli`'s `journal_blocked`). "On both faces" is the reason this constant moves in the
/// same commit as `gx_cli::ROW_LAYOUT_BLOCKED.title`.
///
/// 🔴 **Update (F-2, `req/38` §337, `req/565` §4) — a gate compares the two now.** Through R41
/// there was **no gate in this tree** that compared this constant with `gx_cli::ROW_LAYOUT_BLOCKED
/// .title` (machine scan: zero readers of this constant outside this file), so the equality was a
/// convention a lane kept by hand — audit 40 finding F-2. `crates/gx-cli/tests/
/// r21_refusal_map_is_whole.rs::layout_blocked_title_agrees_across_the_two_faces` is that reader
/// now, and the equality is a measured fact rather than a convention.
pub const LAYOUT_BLOCKED_TITLE: &str = "a declared path of this project's `.gx/` is occupied by \
                                        something that is not what the declaration says";

/// 🔴 **DR-B (`req/38` §337, `req/565` §3) — the thirteenth word 44 §2.3 does not have: this
/// project's journal is there, is the regular file it is declared to be, and this process could
/// not open it.**
///
/// `req/38` §328 ruling 2 ③④ named this exact condition and deliberately did not mint a word for
/// it, filing it as a DR instead. `req/38` §337 ruled the DR: mint. Not `JOURNAL_ABSENT` (the file
/// is present — "is not there" would be false), not `LAYOUT_BLOCKED` (the shape is exactly what
/// `req/56` §2 declares — "is not what the declaration says" would be false of a regular file that
/// is one), and not a fourth `INTERNAL` — the operating system's own `stat`/`open` refusal is
/// completely classified (a `kind` other than `NotFound`), and `INTERNAL` is 44 §2.3's word for
/// what cannot be.
pub const JOURNAL_UNREADABLE: &str = "JOURNAL_UNREADABLE";

/// The status [`JOURNAL_UNREADABLE`] carries.
pub const JOURNAL_UNREADABLE_STATUS: u16 = 500;

/// The title [`JOURNAL_UNREADABLE`] carries, on both faces.
///
/// [`BUSY_TITLE`]'s rule: a proxy speaking both must not hold two names for one refusal. F-2
/// (`req/38` §337, `req/565` §4) applies its parity gate to this title from the commit that mints
/// it, the same commit as this constant (`req/565` §4-2, AC-7).
pub const JOURNAL_UNREADABLE_TITLE: &str =
    "this project's journal is there and this process could not open it";

/// 🔴 The codes 44 §2.3 does **not** have and a ruling gave this repository.
///
/// ~~Two~~ ~~**Four**~~ (v0.4-l, `req/189`) ~~**Six**~~ (v0.4-n, `req/38` §148/§156) ~~**Seven**~~
/// (v0.4-v, R9) ~~**Nine**~~ (v0.4-w, R10, `req/238` H-01) ~~**Ten**~~ (v0.4-y, R12, `req/242`
/// H-01) ~~**Twelve**~~ (v0.4-z, R13, `req/244` H-01 + M-04) ~~**Thirteen**~~ (v0.5-a, R14,
/// `req/246` M-04) **Fourteen** (v0.5-d, DR-B, `req/38` §337, `req/565` §3),
/// all from 44
/// §2.6's "backward-compatible addition" clause (sem: SEM-gx-api-072) and each ruled in `req/38`:
///
/// | code | status | CLI | ruling | why 44 has no row |
/// |---|---|---|---|---|
/// | `INVALID_STATE` | 409 | 2 | **E-M6-18** (§52, M6H5-3) | §2.2 names it three times; §2.3 omits it |
/// | `UNAVAILABLE` | 503 | 1 | **E-M6-22** (§53, M6H6-4) | §2.3 has no word about the server |
/// | `PAYLOAD_TOO_LARGE` | 413 | 1 | **§122 A M-14** (`req/189`) | §2.3 has no word about a body that was not read |
/// | `UNSUPPORTED_MEDIA_TYPE` | 415 | 1 | **§122 A L-04 + H-10** (`req/189`) | §2.3 has no word about a body that is not JSON |
/// | `BUSY` | 503 | 1 | **DR-43-2** (§148) | §2.3 has no word for a writer that is momentarily excluded |
/// | `LEDGER_DISAGREES` | 500 | 1 | **DR-43-6** (§156 ruling 2(a)) | §2.3 has no word for two files describing different trees |
/// | `DECLARATION_UNREADABLE` | 500 | 1 | **R9** (§175 ruling 2) | §2.3 has no word for a declaration that will not parse |
/// | `DECLARATION_ABSENT` | 500 | 1 | **R10** (§177 ruling 2) | §2.3 has no word for a declaration that is **gone** |
/// | `CONFIG_ABSENT` | 500 | 1 | **R10** (§177 ruling 2) | §2.3 has no word for settings that are **gone** |
/// | `JOURNAL_ABSENT` | 500 | 1 | **R12** (§181 ruling 2) | §2.3 has no word for a project that lost its own log |
/// | `OUTPUT_FAILED` | 500 | 1 | **R13** (§183 ruling 2) | §2.3 has no word for an answer that was composed and could not be delivered |
/// | `HISTORY_LOST` | 500 | 1 | **R13** (§183 ruling 2) | §2.3 has no word for a project that has used gx and holds no witness of any commit |
/// | `LAYOUT_BLOCKED` | 500 | 1 | **R14** (§186 ruling 2) | §2.3 has no word for a declared directory whose path holds something that is not one |
/// | `JOURNAL_UNREADABLE` | 500 | 1 | **DR-B** (§337) | §2.3 has no word for a project's journal that is present, is the right shape, and could not be opened |
///
/// The last four carry CLI exit **1** because 44 §1.4 has no exit for any of them (the same fold
/// `UNAVAILABLE_CLI_EXIT` documents) ~~and because the CLI never meets them: it does not speak HTTP
/// to itself~~.
///
/// 🔴 **R11 / `req/240` L-07** — the struck clause is false, and it is false in the one direction
/// that matters: these two words are ones the **CLI alone** says. `gx submit`, `gx log proof`,
/// `gx replay`, `gx draft list` and `gx log checkpoint` all refuse `DECLARATION_ABSENT` or
/// `CONFIG_ABSENT` off `Layout`, and until R11 the HTTP surface had no road to either (it read
/// `.gx/` once, at start-up — `req/240` M-04). The sentence reads like a copy of
/// `UNAVAILABLE`'s, where it is true. Since R11 the HTTP face answers them too, at the writer's
/// door and on `/healthz`'s `status_reason`, so the exit stays **1** for the reason that stands on
/// its own: 44 §1.4 has no exit for either, and a CLI that invented one would be publishing a
/// number no spec carries.
///
/// `crates/gx-cli/tests/exit_matrix_cli.rs` reads `SPEC_44_EXIT_ADDITIONS`, not this list, so the
/// two lists are compared by a reader and not by a probe — said here so that it is findable. The
/// **SDK's** copy of this vocabulary is compared by a probe since R11
/// (`sdk/typescript/test/gx_code_census.test.mjs`, `req/240` L-06).
///
/// Kept **out** of [`GX_CODES`], which is a transcription of §2.3 and would stop being one the
/// moment it gained a row nobody wrote there. `probes/doubt/tests/m6_gx_code.rs` parses the markdown
/// and compares it with `GX_CODES`; this list is the place a reader looks for the difference between
/// "what 44 says" and "what this server sends" (sem: SEM-gx-api-073), which is a difference that has to be findable.
pub const RULED_ADDITIONS: [GxCode; 14] = [
    GxCode {
        code: INVALID_STATE,
        status: INVALID_STATE_STATUS,
        cli_exit: 2,
        meaning: "a state-machine transition it does not allow (E-M6-18; 44 §2.2 names it in 3 places, §2.3 has no row for it)" /* sem: SEM-gx-api-074 */,
    },
    GxCode {
        code: UNAVAILABLE,
        status: UNAVAILABLE_STATUS,
        cli_exit: UNAVAILABLE_CLI_EXIT,
        meaning: "the server is shutting down and is not accepting new requests (E-M6-22; §2.3 has no operational code)" /* sem: SEM-gx-api-075 */,
    },
    GxCode {
        code: PAYLOAD_TOO_LARGE,
        status: PAYLOAD_TOO_LARGE_STATUS,
        cli_exit: 1,
        meaning: "request body exceeds MAX_BODY_BYTES and was not read (M-14, req/189; 44 §2.3 has no row for an unread body)",
    },
    GxCode {
        code: UNSUPPORTED_MEDIA_TYPE,
        status: UNSUPPORTED_MEDIA_TYPE_STATUS,
        cli_exit: 1,
        meaning: "a request body arrived without `Content-Type: application/json` (L-04 + H-10, req/189; 44 §2.3 has no row for a body that is not JSON)",
    },
    GxCode {
        code: BUSY,
        status: BUSY_STATUS,
        cli_exit: 1,
        meaning: "another gx process holds this project's `.gx/LOCK` and this operation was refused rather than queued (DR-43-2, req/38 §148; 44 §2.3 has no row for a momentarily excluded writer)",
    },
    GxCode {
        code: LEDGER_DISAGREES,
        status: LEDGER_DISAGREES_STATUS,
        cli_exit: 1,
        meaning: "the journal and the ledger of this project describe different trees, so the write was refused and no head was signed (DR-43-6, req/38 §156; 44 §2.3 has no row for two files disagreeing)",
    },
    GxCode {
        code: DECLARATION_UNREADABLE,
        status: DECLARATION_UNREADABLE_STATUS,
        cli_exit: 1,
        meaning: "this project's `.gx/VERSION` is present and does not read as a declaration, so the layout it stamps cannot be established (R9, req/236 H-04; 44 §2.3 has no row for a project whose own declaration will not parse)",
    },
    GxCode {
        code: DECLARATION_ABSENT,
        status: DECLARATION_ABSENT_STATUS,
        cli_exit: 1,
        meaning: "this project has a journal and no `.gx/VERSION`, and no verb writes one back on its own (R10, req/238 H-01; 44 §2.3 has no row for a project that lost its own declaration)",
    },
    GxCode {
        code: CONFIG_ABSENT,
        status: CONFIG_ABSENT_STATUS,
        cli_exit: 1,
        meaning: "this project has a journal and no `.gx/config.toml`, so the key 43 §7.9 (b) says it recovers under cannot be read and will not be silently defaulted (R10, req/238 H-01)",
    },
    GxCode {
        code: JOURNAL_ABSENT,
        status: JOURNAL_ABSENT_STATUS,
        cli_exit: 1,
        meaning: "this project's `.gx/ledger/journal` is not there, so a door that appends refused rather than creating an empty one over the loss (R12, req/242 H-01 (d); 44 §2.3 has no row for a project that lost its own log)",
    },
    GxCode {
        code: OUTPUT_FAILED,
        status: OUTPUT_FAILED_STATUS,
        cli_exit: 1,
        meaning: "this run composed its answer and could not write it to stdout, so it says so here rather than panicking with an exit no table carries (R13, req/244 H-01; 44 §2.3 has no row for a delivery that failed)",
    },
    GxCode {
        code: HISTORY_LOST,
        status: HISTORY_LOST_STATUS,
        cli_exit: 1,
        meaning: "this project holds no witness of any commit it recorded and holds entries that say it recorded some, so the writer's door refused rather than starting a second history over the loss (R13, req/244 M-04; 44 §2.3 has no row for it)",
    },
    GxCode {
        code: LAYOUT_BLOCKED,
        status: LAYOUT_BLOCKED_STATUS,
        cli_exit: 1,
        meaning: "a path that is not a directory is sitting where one of `.gx/`'s declared directories belongs, so every door that writes refused; the state is entirely classifiable and was answering INTERNAL (R14, req/246 M-04; 44 §2.3 has no row for it)",
    },
    GxCode {
        code: JOURNAL_UNREADABLE,
        status: JOURNAL_UNREADABLE_STATUS,
        cli_exit: 1,
        meaning: "this project's journal is present, is the regular file req/56 §2 declares, and this process could not open it; the operating system's own refusal is entirely classifiable and was answering INTERNAL (DR-B, req/38 §337, req/565 §3; 44 §2.3 has no row for it)",
    },
];

/// Where a refusal kind came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `gx_engine::ERROR_KINDS`.
    Engine,
    /// `gx_gate::ERROR_KINDS`.
    Gate,
    /// `gx_witness::ERROR_KINDS`.
    Witness,
    /// `gx_log::ERROR_KINDS`.
    Log,
}

impl Origin {
    /// The crate's declared vocabulary, so that a caller can count what it is mapping.
    #[must_use]
    pub fn vocabulary(self) -> &'static [&'static str] {
        match self {
            Origin::Engine => &gx_engine::ERROR_KINDS,
            Origin::Gate => &gx_gate::ERROR_KINDS,
            Origin::Witness => &gx_witness::ERROR_KINDS,
            Origin::Log => &gx_log::ERROR_KINDS,
        }
    }
}

/// One refusal kind, and what 44 §2.3 calls it.
#[derive(Clone, Copy, Debug)]
pub struct Refusal {
    /// Which crate's vocabulary.
    pub origin: Origin,
    /// The `Error::kind()` string.
    pub kind: &'static str,
    /// The `gx_code` it is answered with.
    pub code: &'static str,
    /// 🔴 Whether meaning is lost on the way (Λ4). `Some(..)` is a row of [`folds()`].
    pub fold: Option<&'static str>,
}

/// 🔴 **The map** — thirty-eight refusal kinds, one row each, in vocabulary order.
///
/// Read it as Λ4's quotient made explicit. What a reader should be able to do with it is exactly
/// what a `_` arm prevents: look up a refusal they saw in a log and find the sentence explaining why
/// it wears the code it wears.
pub const REFUSALS: [Refusal; 38] = [
    // ---- gx_engine::ERROR_KINDS (17) -------------------------------------------------
    Refusal {
        origin: Origin::Engine,
        kind: "Canon",
        code: "INTERNAL",
        fold: Some(
            "gx-canon refusing means a value the engine built has no canonical form. That is this \
             system's bug and never the request's, so it is not `VALIDATION_ERROR`; 44 §2.3 has no \
             code for \"the server built something unencodable\" and `INTERNAL` \"an unclassifiable internal \
             error\" (sem: SEM-gx-api-076) is the honest bucket.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Core",
        code: "VALIDATION_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Io",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Malformed",
        code: "VALIDATION_ERROR",
        fold: Some(
            "Two producers with two different subjects (gx-engine's own note says so): a journal \
             record over `MAX_RECORD_BYTES` is the server's fault and a blob that does not hash to \
             its own name is a stored artefact's. Neither is \"a malformed request\" (sem: SEM-gx-api-077) in the ordinary \
             sense, and both arrive on the road a request opened. 422 is the closer of 44's two \
             wrong answers because it is the one that does not tell a client to retry.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "InconsistentEscrow",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "InconsistentTicket",
        code: "INTERNAL",
        fold: Some(
            "E-6's checked constructor refusing means a ticket's id disagrees with its contents — a \
             value **claiming** to hash to its own name. That is a collaborator being wrong, which \
             44 §2.3 has no word for at all; `POLICY_ERROR` would blame Cedar for a digest.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "EvidenceUnavailable",
        code: "INTERNAL",
        fold: Some(
            "🔴 The one refusal that normally never reaches this map: 43 T-4d turns it into \
             `AbortReason::VerifierUnavailable` **inside** `Engine::verify`, which is a state and \
             not an error. It arrives here only where a caller sees the `Err` directly. 44 §2.3 has \
             no \"the verifier could not be reached\" (sem: SEM-gx-api-078) and 503 is not one of its statuses.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "NotIdempotent",
        code: "INTERNAL",
        fold: Some(
            "AC-033's `canon(canon(x)) != canon(x)`. A broken canonicaliser is the deepest \
             invariant this system has and `INTERNAL` is the shallowest code 44 offers; the JSON \
             `detail` carries the transformation so the fold is readable.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "NotFound",
        code: "NOT_FOUND",
        // 🔴 **`req/38` §225 ruling 5 (R23)** — the fold this row always was, written down.
        fold: Some(
            "`gx_engine::Error::NotFound` carries twenty subjects and this row answers one word \
             for all of them. Two name an object the caller named (`transformation`, `draft`) and \
             `NOT_FOUND` is exactly right for those; nine name an **adapter** that is not \
             registered for a substrate, which is a statement about something nobody named, and \
             `req/38` §224 ruling 1 keeps that one `INTERNAL` / exit 1 on the CLI face for the \
             reason `DECLARATION_ABSENT` and `JOURNAL_ABSENT` were split off `NOT_FOUND` \
             (`req/238` H-01). Three name an escrowed inverse and the CLI answers those \
             `INVERSE_UNAVAILABLE` (R21, `req/304` §0.8); the remaining six are internal \
             invariants of a committed row. So the two faces of this system agree on the two \
             subjects a caller can name and deliberately disagree on the rest, and this note is \
             where that disagreement is declared rather than discovered.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "InvalidState",
        code: INVALID_STATE,
        fold: Some(
            "🔴 `INVALID_STATE` is 44 **§2.2**'s word and is not in §2.3's twelve-row table. See \
             `INVALID_STATE` above and M6H5-3: the endpoint sections name it three times and the \
             code table does not carry it.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Adapter",
        code: "ADAPTER_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Ledger",
        code: "INTERNAL",
        fold: Some(
            "The engine's own note distinguishes two facts here — \"the ledger already holds a \
             different receipt for this transformation\" (sem: SEM-gx-api-079) (INV-S3's guard, an exactly-once violation) \
             and \"the ledger could not be fsynced\" (a durability failure) — and 44 §2.3 has one \
             code for both. The `action` field survives in `detail`.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Witness",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Unrepresentable",
        code: "INTERNAL",
        fold: Some(
            "🔴 **M5FIX-8's first of two.** \"the canon has no way to write down what happened\" (sem: SEM-gx-api-080) — a \
             T-4e degraded admission whose `CommitReceipt` would need a `proof_digest` for a gate \
             nobody asked. req/88 M6-09 names this exact row: it is not client input, so it is not \
             `VALIDATION_ERROR`, and it is not a failure either, so `INTERNAL` is a fold and not a \
             classification. Adding a code is 44 §2.6's \"backward-compatible addition\" and is M6H5-4.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Busy",
        code: BUSY,
        // 🔴 Not a fold, and that is the point of adding the code rather than reusing one. Every
        // other operational refusal in this table lands on `INTERNAL` and tells an operator that gx
        // broke; this one is answered with the word for what happened, so a client's retry policy
        // has something true to branch on (`req/38` §148, DR-43-5).
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "WorldMoved",
        code: "PRECONDITION_CHANGED",
        // 🔴 Not a fold either, and for once the row was already waiting: 44 §2.3's
        // `PRECONDITION_CHANGED` is "CAS failure (CON-2)", and DR-43-1(a) is that same CAS moved one
        // window earlier -- from "between this undo's plan and its commit" to "between the original
        // commit and this undo". The status a client sees is identical (409) and the exit a script
        // branches on is identical (3), which is what `req/38` §132 ruling 2 asked for when it
        // refused to mint a number.
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "WitnessMissing",
        code: "PRECONDITION_CHANGED",
        // 🔴 **R3 (`req/38` §160 ruling 2, `req/222` H-01/H-02)** — a fold, and a deliberate one.
        //
        // `WorldMoved` one row up says "the world is not what was attested"; this says "nobody can
        // say what was attested". They are different facts and they share a code, because a
        // caller's correct response to both is the same — look at the target and at
        // `.gx/receipts/`, then decide — and §132 ruling 2's ban on minting a number is still
        // standing. What the code loses, the `detail` carries: `Error::WitnessMissing`'s message
        // names which of the four trusts is missing (absent / unreadable / unsigned / about
        // another transformation).
        fold: Some(
            "R3: `PRECONDITION_CHANGED` means \"the CAS did not pass\" and now covers both \"it \
             ran and failed\" and \"it could not run\". A client that branches only on the code \
             cannot tell a third party's write from a deleted receipt; the sentence can.",
        ),
    },
    // ---- gx_gate::ERROR_KINDS (6) ----------------------------------------------------
    Refusal {
        origin: Origin::Gate,
        kind: "EmptyDeny",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "NotDigestible",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "PolicySetUnreadable",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "RegistryUnusable",
        code: "POLICY_ERROR",
        fold: Some(
            "🔴 gx-gate's own source says it: \"**44 has no code for the invariant side**\" (sem: SEM-gx-api-081) \
             (req/64 §4's erratum). 44 §2.3 defines `POLICY_ERROR` as \"Cedar's own evaluation failing (a policy \
             bug, etc.)\" and an invariant registry that cannot name its invariants apart is not Cedar. \
             **E-M3-16** already ruled the repair — widen `POLICY_ERROR`'s description — and named \
             M6's API window as the implementation site, which is this row.",
        ),
    },
    Refusal {
        origin: Origin::Gate,
        kind: "Unevaluable",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "UnknownReasonCode",
        code: "POLICY_ERROR",
        fold: None,
    },
    // ---- gx_witness::ERROR_KINDS (8) -------------------------------------------------
    Refusal {
        origin: Origin::Witness,
        kind: "SignatureInvalid",
        code: "VALIDATION_ERROR",
        fold: Some(
            "A receipt whose signature does not check is a **document the caller supplied** that is \
             not what it says it is, so 422 is right and 500 would be wrong. What is lost is that \
             this is the one refusal an attacker can provoke on purpose, and 44 gives it the same \
             code as a missing field.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Schema",
        code: "VALIDATION_ERROR",
        fold: Some(
            "🔴 **M5FIX-8's second of two**, and req/90 §2.4 measured it: \"`gx_witness::Error::\
             Schema`'s 'a receipt that is not legal' is input-caused, but is also not `VALIDATION_ERROR` (what was passed \
             in is itself broken)\" (sem: SEM-gx-api-082). When the engine raises it the subject is the engine's own payload \
             (ASM-14's obligations) and when a verifier raises it the subject is the caller's file. \
             One code, two subjects.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Canon",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Log",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Io",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "KeyFormat",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "KeyPermissions",
        code: "INTERNAL",
        fold: Some(
            "req/90 §2.4's third: \"`gx_witness::Error::KeyPermissions` is **an operational state**, not \
             a property of the request\" (sem: SEM-gx-api-083). A key file group-readable on the server is a deployment fault a \
             client can neither cause nor repair, and 44 §2.3 has no operational code — 503 is not \
             among its statuses.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Entropy",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "KeyEncrypted",
        code: "INTERNAL",
        fold: Some(
            "🔴 **P2 item2** (`req/130` §1, NFR-010). Not a client's doing: `load` reached a key \
             file this deployment's own operator encrypted, and no HTTP endpoint offers a \
             passphrase to retry with — the fold is the same operational shape as \
             `KeyPermissions`'s, a fact about this server's configuration rather than about a \
             request. `req/131` §3 names the missing passphrase surface a residual rather than a \
             silent gap.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "WrongPassphrase",
        code: "INTERNAL",
        fold: Some(
            "🔴 **P2 item2**. AEAD authentication cannot tell \"wrong passphrase\" from \"tampered \
             ciphertext\" (sem: SEM-gx-api-084) apart — that is the property, not a gap — and 44 §2.3 has no code for \
             either half of that pair. Reached only where a caller supplies a passphrase directly \
             (`gx key`'s CLI-side commands today), never through the HTTP surface, which is the \
             same absence `KeyEncrypted` above names.",
        ),
    },
    // ---- gx_log::ERROR_KINDS (5) -----------------------------------------------------
    Refusal {
        origin: Origin::Log,
        kind: "Canon",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Log,
        kind: "OutOfRange",
        code: "VALIDATION_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Log,
        kind: "Malformed",
        code: "VALIDATION_ERROR",
        fold: Some(
            "req/90 §2.4's first: \"`gx_log::Error::Malformed`'s 'a tree size out of range' is **caused by the client's \
             own input** and is not an internal error (`VALIDATION_ERROR` is correct)\" (sem: SEM-gx-api-085). Hand 2 folded it \
             into `INTERNAL` because 44 §1.2 gives the read side only 0 and 1; this hand has the \
             statuses and pays the debt. What is still lost is that the same variant also carries \
             \"a proof shape this crate cannot read\", which is not the caller's doing.",
        ),
    },
    Refusal {
        origin: Origin::Log,
        kind: "Io",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Log,
        kind: "Conflict",
        code: "INTERNAL",
        fold: Some(
            "ASM-43-1's guard: the ledger holds a **different** receipt under this transformation. \
             That is an exactly-once violation — the strongest thing this system can discover about \
             itself — and it lands on \"unclassifiable\" (sem: SEM-gx-api-086) because 44 §2.3 has no word for it.",
        ),
    },
];

/// 🔴 The denominator, as a compile-time assertion (the honest form of "a variant increase becomes a compile
/// error" (sem: SEM-gx-api-087) — see the module header).
///
/// Each crate's own probe keeps its `ERROR_KINDS` array in step with its enum, so a variant added
/// anywhere obliges an array to grow, and the moment one grows this constant stops evaluating.
pub const DENOMINATOR: usize = {
    let total = gx_engine::ERROR_KINDS.len()
        + gx_gate::ERROR_KINDS.len()
        + gx_witness::ERROR_KINDS.len()
        + gx_log::ERROR_KINDS.len();
    assert!(
        total == REFUSALS.len(),
        "M6-09: every declared refusal kind needs a row in REFUSALS. A crate grew its ERROR_KINDS \
         array and this map did not."
    );
    total
};

/// 🔴 The rows Λ4 asks to be written down: refusals whose meaning does not survive their code.
///
/// Not a defect list. Λ4 proves the map "is … neither surjective nor injective; information is always lost" (sem: SEM-gx-api-088), so a fold is
/// the normal case and the abnormal thing would be a table claiming none. What makes it a discipline
/// rather than a shrug is that each row says **what** was lost, in a sentence somebody can disagree
/// with.
#[must_use]
pub fn folds() -> Vec<&'static Refusal> {
    REFUSALS.iter().filter(|r| r.fold.is_some()).collect()
}

/// The `gx_code` for one refusal kind, or `None` if the vocabulary has no such row.
///
/// `None` is reachable only from a caller that invented a kind string; every value produced by an
/// `Error::kind()` in this workspace is in the table, and [`DENOMINATOR`] is why.
#[must_use]
pub fn of_kind(origin: Origin, kind: &str) -> Option<&'static Refusal> {
    REFUSALS
        .iter()
        .find(|r| r.origin == origin && r.kind == kind)
}

/// 44 §2.3's row for a code, by name.
#[must_use]
pub fn code(name: &str) -> Option<&'static GxCode> {
    GX_CODES.iter().find(|c| c.code == name)
}

/// The HTTP status one `gx_code` carries — 44 §2.3's second column, and [`RULED_ADDITIONS`]'
/// (of which [`INVALID_STATE`]'s 409 was the first; `UNAVAILABLE` keeps answering through
/// [`crate::ApiError::unavailable`]'s explicit status as it always has).
#[must_use]
pub fn status_of(name: &str) -> u16 {
    if name == INVALID_STATE {
        return INVALID_STATE_STATUS;
    }
    if let Some(added) = RULED_ADDITIONS.iter().find(|c| c.code == name) {
        return added.status;
    }
    code(name).map_or(500, |c| c.status)
}

/// 44 §2.3's `type` URI for a code.
///
/// 44 §2.3's example is `https://glovrex.dev/errors/precondition-changed`, which is the code
/// lowercased with `_` becoming `-`. gx-cli's `Error::problem` derives it the same way, and the two
/// derivations agreeing is what makes "shares its vocabulary with the CLI exit code and `Reason.code`" (sem: SEM-gx-api-089) true of the
/// `type` field as well as of the code.
#[must_use]
pub fn type_uri(name: &str) -> String {
    format!(
        "https://glovrex.dev/errors/{}",
        name.to_lowercase().replace('_', "-")
    )
}
