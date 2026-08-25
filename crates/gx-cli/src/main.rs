#![forbid(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The `gx` binary. 44 §1's thirteen subcommands — **eight of them**.
//!
//! Hand 1 built the ground (the `.gx/` directory, the draft store, the id-resolution cache) and
//! implemented no verb. Hand 2 implemented 44 §1.2's read side ("`gx receipt show|verify` /
//! `gx log proof|consistency` / `gx key gen|list` / `gx replay` + offline verifier") (sem: SEM-gx-cli-415). **Hand 3**
//! implements the pipeline's front half — "`gx submit` / `gx plan` / `gx verify` / `gx commit`" —
//! which is where the binary stops only reading, and where 44 §1.4's **2** becomes reachable for the
//! first time. `undo`, `cancel`, `escalation`, `policy` and `serve` are hands 4 onwards.
//!
//! # 🔴 discipline 52 — the exit code clap wanted is the one 44 gives to "denied" (sem: SEM-gx-cli-416)
//!
//! req/38 §48 M6H1-1 adopted (a) made it a rule after hand 1 found it (sem: SEM-gx-cli-417):
//!
//! > every subcommand uses `try_parse()` + mapping (usage error → exit **1** "invalid input";
//! > `--help`/`--version` → 0). **44 §1.4's exit 2 is reserved for the state machine's "refused
//! > (denied)"; clap's default 2 is not reused for a CLI argument's usage error** (E-M6-2). clap's
//! > default `parse()` is forbidden (sem: SEM-gx-cli-418).
//!
//! `Parser::parse()` appears nowhere below. [`gx_cli::exit::DENIED`] **is** returned now — by
//! `gx verify` on a `Verdict::Deny` and by `gx commit` on an un-admitted transformation, which are
//! the two things 44 §1.4 says the number means — and that is exactly why discipline 52 exists (sem: SEM-gx-cli-419): a binary
//! that also answered a mistyped flag with 2 would make the status useless to the script that reads
//! it. `crates/gx-cli/tests/exit_map.rs` measures the parser half and
//! `crates/gx-cli/tests/ac_054.rs` measures the state-machine half.
//!
//! # What goes where
//!
//! 44 §1.3 fixes the contract: a single newline-terminated JSON object on stdout, and refusals on
//! **stderr** as `{"type", "title", "gx_code", "detail"}` with stdout left empty. So every command
//! below returns an [`Outcome`] the library built, `main` prints it, and every `Err` becomes
//! `Error::problem()` on stderr. Notes an operator needs and a pipe does not — where a new key was
//! filed, for instance — also go to stderr, which is what keeps `gx key gen --json > key.pub.json`
//! a file containing exactly 44 §1.2's two fields.
//!
//! 🔴 **R14 / `req/246` H-01** — **both** of those stdout sentences and stderr sentences travel
//! through a road that returns a value. The object on stdout goes through
//! [`gx_cli::exit::Outcome::emit`] (R13) and the problem object on stderr goes through
//! [`gx_cli::emit::problem_line`], because `eprint!` panics on a write error exactly as `print!`
//! does — which is what made `gx receipt show gx1:doesnotexist 2>/dev/full` exit **101**. The
//! operator notes are the one thing that stays on the macro, and that is the distinction
//! `probes/doubt/tests/declaration_writer_doubt.rs` D-6 now counts: **an answer goes through the
//! type; a sentence beside an answer does not have to.**

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use gx_cli::exit::{Outcome, ERROR, OK};
use gx_cli::keys::KeyStore;
use gx_cli::layout::Layout;
use gx_cli::receipt::{ReceiptStore, Source};
use gx_cli::session::Session;
use gx_cli::{clock, keys, ledger, receipt, replay, rng, Error, Result};
use gx_core::{Cid, EnforcementMode, TransformationId};

/// `gx` — the Glovrex command line.
#[derive(Debug, Parser)]
#[command(
    name = "gx",
    version,
    about = "Glovrex: transformations as first-class, verified objects.",
    // 🔴 **R21 / `req/304` D1 (`req/306` §1 item 2)** — this is the first thing anyone reads.
    //
    // What was here was the sentence the dogfood lane typed `gx --help` and got back before
    // opening a single file: "M6 hand 2 implements 44 §1.2's read side … The write side (submit,
    // plan, verify, commit, undo, cancel, escalation, policy, serve) is hands 3 onwards." Internal
    // build-phase language in the literal help text of a shipped binary, severity high, and two of
    // its claims were false of the binary printing them — the write side is implemented, and "M6
    // hand 2" is a phase that ended milestones ago. `req/304`'s remedy: "It should say what the
    // tool is and does, or nothing."
    //
    // So: what happens to a change, then the verbs, grouped by what a reader came to do.
    // `crates/gx-cli/tests/r21_help_is_user_facing.rs` reads the verb list off `clap` itself, so a
    // verb added later and left out of this string is red rather than merely missing.
    //
    // 🔴 The doc comments that carry the same words are **not** touched — `consumers.rs`'s "M6
    // hand" notes are the record of which hand settled which decision, and `req/306` §1 item 2
    // rules them out of scope by name. A banner is a user-facing string; a doc comment is
    // provenance. Neither is the other, and the same suite guards that half.
    long_about = "gx is a guard the changes an AI agent makes go through.\n\
                  \n\
                  A change is submitted, planned against the object it names, and judged against \
                  policy. It is then committed with a signed receipt, refused, or escalated to a \
                  person to decide. gx escrows the inverse of a change before it applies anything, \
                  so a commit can be undone; when it cannot build one, it says so and asks rather \
                  than making a change nobody can take back. Every receipt can be checked \
                  afterwards on a machine with no copy of the project and no network.\n\
                  \n\
                  Make a change:        submit, plan, verify, commit\n\
                  Take one back:        undo, cancel, escalation\n\
                  Read what happened:   receipt, log, checkpoint, replay, verdict-checkpoint, \
                  repair\n\
                  Put gx in the path:   attach (place gx's own directory on a project that is \
                  already running, and print what was placed), wrap (an agent's tools), serve \
                  (HTTP), demo\n\
                  Hold it at the kernel: confine (Linux: run a command under a Landlock ruleset \
                  the catalogue decides — `gx limits` says what that does and does not cover)\n\
                  Set up and inspect:   key, policy, draft, limits\n\
                  \n\
                  `gx demo` walks the whole loop in a throwaway directory. `gx <command> --help` \
                  describes one command. `gx limits` prints what this build does not cover yet.",
    subcommand_required = true,
    arg_required_else_help = false
)]
struct Cli {
    /// The project whose `.gx/` directory to use. Defaults to the working directory.
    #[arg(long, global = true, value_name = "DIR")]
    project: Option<PathBuf>,

    /// 44 §1.3: "the CLI offers additional human-readable formatting via `--pretty`" (sem: SEM-gx-cli-420).
    #[arg(long, global = true)]
    pretty: bool,

    /// 🔴 **P3** — the MCP server this invocation is connected to, as a command to start.
    ///
    /// Not in 44 §1.2, and it is what `session::MCP_REGISTRATION_FIRED` calls the second half of
    /// registration: the adapter is in every engine this binary opens, and a *server* is what an
    /// invocation names. `gx undo <TID>` above all needs it — 43 §5's inverse of a tool call is a
    /// call, so a process holding no transport can plan the undo and not perform it.
    #[arg(long, global = true, value_name = "CMD")]
    mcp_server: Option<String>,

    /// An argument for `--mcp-server`, repeatable.
    #[arg(long, global = true, value_name = "ARG")]
    mcp_server_arg: Vec<String>,

    /// The `env` member an agent's configuration carries for that server, repeatable.
    ///
    /// The single-shot twin of `gx wrap --server-env`. Without it a verb would start the server
    /// with only this process's own environment, which is a different server from the one the
    /// operator's configuration describes — and `gx undo` is the verb that most needs to reach the
    /// **same** server the change was made through.
    #[arg(long, global = true, value_name = "NAME=VALUE")]
    mcp_server_env: Vec<String>,

    /// The endpoint half of the locators this invocation reads and writes. Defaults to
    /// `stdio://<command>`, which is what `gx wrap` mints.
    #[arg(long, global = true, value_name = "URI")]
    mcp_endpoint: Option<String>,

    /// "a call to this tool is undone by a call to that one" (sem: SEM-gx-cli-421) — the declaration only the party
    /// running the server can make (`gx-adapter-mcp`'s `catalogue.rs`).
    #[arg(long, global = true, value_name = "TOOL=RESTORE_TOOL")]
    mcp_restore: Vec<String>,

    /// The same declaration as a JSON file, template form included (A2, req/38 §92 ruling 1; sem: SEM-gx-cli-422): a map
    /// from forward tool to `{restored_by, arguments?}` — `Catalogue::from_json` is the one reader
    /// of the format. `--mcp-restore` entries are applied on top of it.
    #[arg(long, global = true, value_name = "FILE")]
    mcp_restore_catalogue: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// 44 §1.1's thirteen. Eight of the verbs are here.
// clippy::large_enum_variant: boxing Wrap's gx_binary would break clap::Subcommand's derived
// value-parsing (Box<String> has no clap arg impl); allow is the non-invasive, behavior-preserving fix.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, clap::Subcommand)]
enum Command {
    /// T-1: create a Draft from an intent (44 §1.2).
    Submit {
        /// Which substrate the change is against.
        #[arg(long, value_name = "fs|git|mcp")]
        substrate: String,
        /// The position inside it, in the adapter's own spelling (42 §3.3).
        #[arg(long, value_name = "STR")]
        locator: String,
        /// The intent body, or `-` for stdin. Carried opaquely — see [`intent_bytes`].
        #[arg(long, value_name = "FILE|-")]
        intent: String,
        /// 42 §3.2's `ChangeContext`.
        #[arg(
            long,
            value_name = "Time|Evidence|Policy|Model|Representation|Substrate|Custom:NAME"
        )]
        context: String,
        /// The key id of the actor asking (42 §3.2: the DSSE `keyid` namespace).
        #[arg(long, value_name = "KEY_ID")]
        actor_key: String,
        /// Which of 42 §3.2's three actors this is.
        #[arg(long, value_name = "human|agent|process", default_value = "human")]
        actor_kind: String,
        /// The model, for `--actor-kind agent`.
        #[arg(long, value_name = "STR")]
        actor_model: Option<String>,
        /// 44 §1.2's flag. v0.1 produces order 0 only; see `pipeline::submit`.
        #[arg(long, default_value_t = 0, value_name = "0|1|2")]
        order: u8,
        /// 44 §1.2's flag. v0.1 has one producer of parents and it is `undo`.
        #[arg(long, value_name = "TID")]
        parent: Vec<String>,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// T-2: snapshot and plan, fixing the `TransformationId` (44 §1.2).
    Plan {
        /// An `IntentId` or a `TransformationId` — 44 §0's id-resolution accepts both.
        id: String,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-3 → T-4: collect evidence, ask the gate (44 §1.2).
    Verify {
        /// The `gx1:` transformation id.
        transformation: String,
        /// Pre-collected `Evidence` (42 §3.7) as JSONL, added to what the gate computes.
        #[arg(long, value_name = "FILE")]
        evidence: Vec<PathBuf>,
        /// DR-2 record-only, **for this call** (M6-08 adopted (a); sem: SEM-gx-cli-423). Not a fail posture.
        #[arg(long)]
        record_only: bool,
        /// 🔴 Decide with this pack instead of the shipped one — not in 44 §1.2 (**E-M6-12**).
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-8 → T-11: canonicalize, re-check the precondition, apply, append, issue (44 §1.2).
    Commit {
        /// The `gx1:` transformation id.
        transformation: String,
        /// 44 §1.2: "when unspecified, the CLI deterministically derives it from `transformation_id`" (sem: SEM-gx-cli-424).
        #[arg(long, value_name = "STR")]
        idempotency_key: Option<String>,
        /// 🔴 Not in 44 §1.2's synopsis — see `run`'s note and M6H3-7.
        #[arg(long)]
        record_only: bool,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 **P3** — run the MCP proxy: an agent on stdin, a server as a child, a gate between them.
    ///
    /// `req/119` §2, `req/38` §71. Not in 44 §1.1's thirteen — the surface addition is raised in the
    /// same shape `gx log checkpoint` (M6H2-7) and `gx draft discard` (E-M6-14) were, and req/114's
    /// P5 row owns the **UX** of this verb rather than a second implementation of it.
    ///
    /// 🔴 stdout carries MCP frames and nothing else while this verb runs (the transport
    /// specification forbids anything else there), so its own reporting is on **stderr**.
    Wrap {
        /// The server command and its arguments, after `--`.
        #[arg(last = true, value_name = "CMD [ARGS]...")]
        server: Vec<String>,
        /// The `env` member an agent's configuration carries for this server, repeatable.
        #[arg(long, value_name = "NAME=VALUE")]
        server_env: Vec<String>,
        /// The endpoint half of every locator this session mints. Defaults to `stdio://<command>`.
        #[arg(long, value_name = "URI")]
        endpoint: Option<String>,
        /// Which argument of a `tools/call` names the resource the change is about.
        #[arg(long, default_value = "uri", value_name = "NAME")]
        resource_arg: String,
        /// `--restore <TOOL>=<RESTORE_TOOL>`, repeatable.
        #[arg(long, value_name = "TOOL=RESTORE_TOOL")]
        restore: Vec<String>,
        /// The catalogue as a JSON file, template form included (A2): the same format
        /// `--mcp-restore-catalogue` reads on `gx undo`. `--restore` entries apply on top.
        #[arg(long, value_name = "FILE")]
        restore_catalogue: Option<std::path::PathBuf>,
        /// The agent's key id (42 §3.2).
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// The agent's model. 42 §3.2 requires it for an `Agent`, and every change through this
        /// proxy is an agent's.
        #[arg(long, value_name = "STR")]
        actor_model: Option<String>,
        /// 42 §3.2's `ChangeContext`.
        #[arg(
            long,
            default_value = "Substrate",
            value_name = "Time|Evidence|Policy|Model|Representation|Substrate|Custom:NAME"
        )]
        context: String,
        /// Decide with this pack instead of the shipped set (**E-M6-12**).
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
        /// DR-2's record-only, for every call of this session.
        #[arg(long)]
        record_only: bool,
        /// 🔴 **P3.1** — DR-2's other axis (ASM-13): what happens when the verifier cannot be
        /// reached. Same flag and vocabulary as `Serve`'s (closed|open); default unchanged
        /// (`FailClosed`). `req/38` §74 item③ (gotcha66): before this, `gx wrap` had no road to
        /// `FailOpen` at all.
        #[arg(long, value_name = "closed|open")]
        fail_posture: Option<String>,
        /// 🔴 **NFR-017** — append OTLP/JSON spans to this file. **Off** unless given, and the
        /// start-up line prints "otel: disabled" when it is not (R-114-4: "state zero telemetry
        /// explicitly"; sem: SEM-gx-cli-425).
        #[arg(long, value_name = "PATH")]
        otel_file: Option<String>,
        /// Carry raw locators in those spans. Off, because a resource URI names a customer's file
        /// (`gx_cli::otel::NOT_EXPORTED`).
        #[arg(long)]
        otel_locators: bool,
        /// DR-V4B-3 (`req/189`): send the `--resource-arg` member to the server even when it is a
        /// `gx_*`-named member (by default such a member is gx's own and is stripped from the
        /// wire form of the call, because a strict-body server refuses it; a `--resource-arg`
        /// naming a real tool argument is never stripped, flag or no flag).
        #[arg(long)]
        forward_resource_arg: bool,
        /// 🔴 **B-1** — rewrite an agent's config so its direct entry for the server is gone, and
        /// print the result. Nothing is served; `--server-name` says which entry.
        #[arg(long, value_name = "PATH")]
        adopt_config: Option<PathBuf>,
        /// 🔴 **B-1's machine check** — report whether that entry routes through gx and whether any
        /// entry still starts the server directly. exit 0 = the direct road is gone, 7 = it is not.
        #[arg(long, value_name = "PATH")]
        check_config: Option<PathBuf>,
        /// 🔴 **P-1c** (`req/551` §2) — take gx back out of that entry, putting back the command it
        /// used to run, and print what did **not** come back with it. `.gx/` is not touched.
        ///
        /// This is a mode of `wrap` rather than a verb of its own on purpose: the thing being undone
        /// is `--adopt-config`, which is a mode of `wrap`, and a top-level `gx detach` standing
        /// opposite `gx attach` would read as the undo of the *placement* — which is the one thing
        /// it does not do (`req/551` D-3).
        #[arg(long, value_name = "PATH")]
        detach_config: Option<PathBuf>,
        /// The entry in `mcpServers` that `--adopt-config` / `--check-config` / `--detach-config`
        /// is about.
        #[arg(long, value_name = "NAME")]
        server_name: Option<String>,
        /// Where the `gx` binary is, for the entry `--adopt-config` writes.
        #[arg(long, default_value = "gx", value_name = "PATH")]
        gx_binary: String,
    },
    /// 🔴 **P-1a** (`req/535` §3 R-1) — put `.gx/` on a tree that is already running, and print
    /// what was put there.
    ///
    /// Not one of 44 §1.1's thirteen, and reported as an addition in the shape `gx wrap` and
    /// `gx confine` took. The placement itself is unchanged — [`gx_cli::layout::Layout::create`] is
    /// the road every writing verb has always taken into a directory — so what this verb adds is
    /// the enumeration: all eleven declared paths, each filed as created, already there, or not
    /// placed, with the nature that decides what losing it costs.
    ///
    /// 🔴 It points no route at gx and states nothing about what a route could observe. Those are
    /// `req/535` §8's P-1b and P-1c, and the answer names them rather than leaving them off.
    Attach {
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
        /// 🔴 **P-1b / R-3e** — the agent configuration to read the route out of.
        ///
        /// Read, never written: this is the file `gx wrap --check-config` inspects, and what it
        /// answers about the route is what the coverage table's posture is derived from. Without
        /// it the table still prints, saying that this operation observes nothing — which is the
        /// truth about an attach that pointed no route.
        #[arg(long, value_name = "PATH")]
        route_config: Option<PathBuf>,
        /// The entry in `mcpServers` that `--route-config` is about.
        #[arg(long, value_name = "NAME")]
        server_name: Option<String>,
        /// 🔴 **P-1b** — a file of declarations somebody wrote about this face.
        ///
        /// The **only** place a human sentence enters a coverage table. A file offering a measured
        /// value is refused by name: the receipt is the only address a measurement has.
        #[arg(long, value_name = "PATH")]
        declared: Option<PathBuf>,
    },
    /// 🔴 **P5** (`req/134` §1 item 1, ruling 1; sem: SEM-gx-cli-426) — a disposable, network-free walk of req/114 §3's aha
    /// loop: an agent breaks something through the real `gx wrap` membrane, gx proves what
    /// happened, an operator restores it, and a separate process verifies the restore offline.
    /// Not one of 44 §1.1's thirteen (ruling 2: reported as an addition, the shape `gx wrap` itself (sem: SEM-gx-cli-427)
    /// took at P3).
    Demo {
        /// 44 §1.2's flag. `gx demo` prints its own narrative regardless; this covers the trailing
        /// summary line only.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Hidden — the notes server `gx demo` (and the `gx undo` step inside it) spawn as **this
    /// same binary's own child** (ruling 1: bundled in the one artefact rather than shipped as a (sem: SEM-gx-cli-428)
    /// second executable). Not part of 44 §1.1's thirteen and not meant to be run directly.
    #[command(hide = true, name = "__demo-notes-server")]
    DemoNotesServer,
    /// 🔴 **P5** (`req/134` §1 item 7, ruling 2; sem: SEM-gx-cli-429) — the eight lines 21 §10-4 fixes ("what this build
    /// does not cover yet"), printed for a terminal. `docs/LIMITS.md` prints the same eight for a
    /// browser (AC-P5-5: the two are checked to agree).
    Limits {
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 **S③** (`req/493` §0) — run a command under a kernel ruleset (Landlock) whose write face
    /// the catalogue decides.
    ///
    /// The third of `req/337` §0's three derivations, and the one that happens *during*: the escrow
    /// gate answers before a call and the receipt attests after it, and this asks the kernel to
    /// hold the answer while the call runs. Not in 44 §1.1's thirteen — it is a launcher, not a
    /// transformation verb, and it opens no `.gx/`.
    ///
    /// 🔴 **What it enforces is narrower than what `req/493` §0 describes, and it says so on every
    /// run.** §0 asks for the ruleset to come from the write-target set
    /// `Catalogue::writes_per_this_file` returns; that function returns *tool names*. So the
    /// catalogue supplies whether writing is permitted at all and the invocation supplies where,
    /// and the report carries `write_targets_are_declared: false` so a reader is not left to infer
    /// which half came from a file. See the `gx-confine` crate root.
    Confine {
        /// The tool whose declaration decides whether any write is permitted.
        ///
        /// Without it the catalogue is never asked and every `--allow-write` is granted — which is
        /// a real road (confining a command the catalogue says nothing about) and is reported as
        /// `unasked` rather than as a `yes`.
        #[arg(long, value_name = "TOOL")]
        tool: Option<String>,
        /// A directory the command may write beneath, if the catalogue permits writing at all.
        /// Repeatable.
        #[arg(long, value_name = "DIR")]
        allow_write: Vec<PathBuf>,
        /// Print what would be enforced and stop. Nothing is applied and nothing is run.
        #[arg(long)]
        plan_only: bool,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
        /// The command to run, after `--`.
        ///
        /// 🔴 It is `exec`ed, so this process **becomes** it. That is why the confinement report
        /// goes to stderr before the `exec` and stdout is left alone: there is no "after".
        #[arg(last = true, value_name = "CMD")]
        cmd: Vec<String>,
    },
    /// 🔴 **P3 / FR-M04** — issue and check aggregate verdict checkpoints (`req/119` §4).
    ///
    /// Named `verdict-checkpoint` rather than `checkpoint` by ruling ⑤ (sem: SEM-gx-cli-430): `gx log checkpoint` is the
    /// ledger's signed **tree head** (42 §3.11) and this is a signed **count of verdicts**. One
    /// word for two objects is a trap.
    VerdictCheckpoint {
        #[command(subcommand)]
        cmd: VerdictCheckpointCmd,
    },
    /// Read and check receipts (44 §1.2).
    Receipt {
        #[command(subcommand)]
        cmd: ReceiptCmd,
    },
    /// Read the ledger (44 §1.2), and publish a signed head (M6-24 adopted (b); sem: SEM-gx-cli-431).
    Log {
        #[command(subcommand)]
        cmd: LogCmd,
    },
    /// 🔴 **R6 / DR-43-10** — take the project's signed head **out of the box**.
    ///
    /// Named `checkpoint` beside `verdict-checkpoint` for ruling ⑤'s reason inverted: that verb
    /// carries a signed **count of verdicts** and this one carries the ledger's signed **tree
    /// head**, which is the object 42 §3.11 defines. `gx log checkpoint` mints a new one and needs
    /// the ledger key; this copies the one the project already signed and needs no key at all,
    /// which is what makes it something an operator can run on a schedule.
    Checkpoint {
        #[command(subcommand)]
        cmd: CheckpointCmd,
    },
    /// Generate and list signing keys (44 §1.2, req/56 §3).
    Key {
        #[command(subcommand)]
        cmd: KeyCmd,
    },
    /// T-12: commit the escrowed inverse of a committed transformation (44 §1.2, P-5).
    Undo {
        /// The `gx1:` transformation id to take back.
        transformation: String,
        /// 44 §1.2's flag. v0.1 signs with the original actor's key — see `lifecycle::undo`.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2: "when unspecified, the CLI deterministically derives it from `transformation_id`" (sem: SEM-gx-cli-432).
        #[arg(long, value_name = "STR")]
        idempotency_key: Option<String>,
        /// 🔴 Decide with this pack instead of the shipped one — not in 44 §1.2 (**E-M6-12**).
        ///
        /// An undo is verified like anything else (43 §5-2), so it has a verdict, and the verdict
        /// can be `Deny` — 44 §1.4's 2. Reaching that from a `gx` invocation needs a pack that
        /// refuses a **writable** path, for the same reason `gx verify --policy` needs one.
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
        /// 🔴 Settle pre-flight budget in seconds; 0 disables it (`req/38` §98 ruling 2; sem: SEM-gx-cli-433).
        ///
        /// Before firing, `gx undo` polls the substrate (read-only) until it reports the digest
        /// T_o's own commit receipt attested as its postcondition — real SaaS substrates are not
        /// read-after-write consistent (`req/153` §4.1), and an undo fired into the stale window
        /// lands on `Aborted(ApplyFailed)` (fail-safe) that a bounded wait avoids. On timeout the
        /// undo fires once anyway: the poll judges nothing and the result vocabulary is unchanged.
        /// The default is the E2E-measured harness bound (`req/153` §6: poll 1-2 within 120s);
        /// in-process substrates match on the first poll and never wait.
        #[arg(long, value_name = "SECS", default_value_t = 120)]
        settle: u64,
        /// 🔴 Re-fire the whole undo up to N more times on `Aborted(ApplyFailed)` (§98 ruling 2's (sem: SEM-gx-cli-434)
        /// D-complement — explicit flag, default off).
        ///
        /// Each attempt is an ordinary, independent T_u honestly journalled as its own
        /// transformation (the escrow row stays `Available` until one commits — 42 §3.12's
        /// `Consumed` is written by success alone), and each attempt runs its own settle
        /// pre-flight. Only exit 5 re-fires: a denial, an escalation or a precondition change is
        /// an answer, not a transient.
        #[arg(long, value_name = "N", default_value_t = 0)]
        retry: u32,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// T-7: the owner stops a transformation before the critical section (44 §1.2, DR-11).
    Cancel {
        /// The `gx1:` transformation id.
        transformation: String,
        /// 44 §1.2's flag. v0.1 has no authorization layer — see `lifecycle::cancel`.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-5 / T-5b: a person rules on an escalated transformation (44 §1.2, DR-11).
    Escalation {
        #[command(subcommand)]
        cmd: EscalationCmd,
    },
    /// Check a Cedar policy pack, and run scenarios against it (44 §1.2).
    Policy {
        #[command(subcommand)]
        cmd: PolicyCmd,
    },
    /// 🔴 **DR-43-8** — diagnose, and with `--yes` repair, a project whose journal and ledger
    /// disagree (`req/38` §160 ruling 2, `req/222` H-06).
    ///
    /// The door beside the `LEDGER_DISAGREES` gate. Every write verb refuses on that condition,
    /// `gx serve` refuses to start on it, and the diagnostic those refusals point at (`gx replay`)
    /// repairs nothing — so before this verb the state was observable from four places and exitable
    /// from none. What runs with `--yes` is what `gx serve` runs at start-up: open through the
    /// writer's door (quarantine a torn tail, then remove it — DR-43-7), catch up, and
    /// `Engine::recover` (43 §7-3). What is different is that the answer is a report rather than a
    /// refusal.
    ///
    /// 🔴 **R4 / `req/225` H-01** — without `--yes` the project is opened through DR-43-7's
    /// **reader's** door and not one byte of it moves. Until R4 the writer's door was taken before
    /// the flag was read, so the diagnosis quarantined and then cut the torn tail it had been
    /// called to describe: measured at 522 bytes of ledger going to 0 on a `gx repair --json`.
    ///
    /// Not a `--force`: it writes no transition the engine would not have written by itself, and a
    /// project that still disagrees afterwards is reported as such and exits 1.
    Repair {
        /// Run the repair. Without it this verb reads the project through DR-43-7's read-only door
        /// and writes nothing at all — no repair, no quarantine, no key needed (R4, `req/225`
        /// H-01).
        #[arg(long)]
        yes: bool,
        /// The key `recover` signs with, if it finishes an interrupted commit. Defaults to
        /// `.gx/config.toml`'s `engine_signing_keyid` (E-M6-7), as `gx serve` does.
        #[arg(long)]
        signing_key: Option<String>,
        /// 🔴 **R6 / DR-43-10** — a signed checkpoint kept outside this machine
        /// (`gx checkpoint export`). The project is refused if its tree is behind that document.
        ///
        /// This is the only check in `gx` that cannot be defeated by write access to the project,
        /// because the evidence is not in the project (`req/229` §7-4).
        #[arg(long, value_name = "FILE")]
        against: Option<PathBuf>,
        /// 🔴 **R7 / `req/38` §171 ruling 2(c)** — take the shorter tree, on purpose and on the
        /// record.
        ///
        /// A project that has gone backwards is refused everywhere, and R6 had no way to say "yes,
        /// I restored from a backup, this shorter tree is the one I want". `gx repair --yes` did it
        /// silently instead (`req/232` M-01): it re-applied an old delta and then wrote a fresh
        /// head over the shortened tree, so the rollback became the attested past.
        ///
        /// This makes it a decision. It requires `--yes` and `--against <FILE>`, the file has to be
        /// this project's, and the new head records what it replaced.
        #[arg(long, requires = "against")]
        accept_rollback: bool,
        /// 🔴 **R8 / `req/234` H-01** — file the commit receipts this project's committed leaves
        /// have none of.
        ///
        /// A commit that reached the journal and the ledger and whose receipt was never written is
        /// a row `gx undo` refuses forever and no third party can check. This rebuilds each such
        /// receipt from the world and the journal and files it **only** when it digests to what the
        /// ledger already witnessed — so it can produce the document that was committed, or
        /// nothing. It never asks the substrate to change anything. Requires `--yes`; the count it
        /// acts on is `receipts_missing` in the diagnosis.
        #[arg(long)]
        reissue_receipts: bool,
        /// Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// Deterministic replay of the engine journal (44 §1.2, E-M5-2).
    Replay {
        /// A `gx1:` transformation id. Omit it and use `--from`/`--to`, or neither for everything.
        transformation: Option<String>,
        /// First journal record index, inclusive (M6H2-8: of the **journal**).
        #[arg(long, requires = "to")]
        from: Option<usize>,
        /// Last journal record index, exclusive.
        #[arg(long, requires = "from")]
        to: Option<usize>,
        /// Accepted for 44 §1.2's synopsis. Replay writes nothing (E-M5-2); see M6H2-9.
        #[arg(long)]
        dry_run: bool,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Put down an intent that was never planned (**E-M6-14**; not in 44 §1.1's thirteen).
    ///
    /// The verb exists because **E-M6-1** took `Draft` out of `gx cancel`'s from-set: a draft has no
    /// `TransformationId`, no row in the state table (M5-17 adopted (b)) and no journal record that could
    /// carry an `Aborted`. "discarding a draft is an operation that does not land in the ledger" (sem: SEM-gx-cli-435) — 43 T-7 is a transition and this is not.
    Draft {
        #[command(subcommand)]
        cmd: DraftCmd,
    },
    /// Serve 44 §2's HTTP surface until a signal (44 §1.1: "`gx serve` | starts gx-api"; sem: SEM-gx-cli-436).
    ///
    /// 🔴 **Authorization** — the only check is a single static Bearer token (44 §2.5, v0.1). It
    /// answers whether the caller holds this server's token and nothing about who they are: there is
    /// no authorization layer in v0.1 (M5H6-4), `cancel` and `escalation` accept the actor the
    /// request declares, and 43 T-7's owner guard has no enforcement point. The default bind is
    /// therefore loopback (127.0.0.1:8787); binding anywhere else exposes an unauthorized surface and
    /// is refused without an explicit flag.
    Serve {
        /// 44 §1.2's flag. Loopback only in v0.1 — see the note above (M6-10 adopted (b); sem: SEM-gx-cli-437).
        #[arg(long, value_name = "ADDR:PORT")]
        bind: Option<String>,
        /// DR-2's `EnforcementMode` axis for this process (43 §4). **Not** a fail posture.
        #[arg(long)]
        record_only: bool,
        /// DR-2's other axis (ASM-13): what happens when the verifier cannot be reached.
        #[arg(long, value_name = "closed|open")]
        fail_posture: Option<String>,
        /// 44 §1.2's synopsis. Refused in v0.1 — 44 §2.5 puts mTLS in "v0.2 (announced)" (N-09; sem: SEM-gx-cli-438).
        #[arg(long, value_name = "PATH")]
        tls_cert: Option<PathBuf>,
        /// As `--tls-cert`.
        #[arg(long, value_name = "PATH")]
        tls_key: Option<PathBuf>,
        /// 🔴 The file holding 44 §2.5's bearer token. Required; the **path** is the argument so
        /// that the secret is not in `ps` (not in 44 §1.2's synopsis — M6H6-8).
        #[arg(long, value_name = "PATH")]
        token_file: Option<PathBuf>,
        /// The key this server signs receipts with (45 §1). Defaults to `.gx/config.toml`'s
        /// `engine_signing_keyid` (**E-M6-7**).
        #[arg(long, value_name = "KEY_ID")]
        signing_key: Option<String>,
    },
}

#[derive(Debug, clap::Subcommand)]
enum DraftCmd {
    /// 🔴 **R10 / audit 8 M-06 + L-02** — the drafts this project holds, and the ones whose body
    /// is gone.
    ///
    /// `gx draft discard` has existed since M6 and there has never been a verb that says what
    /// there is to discard: audit 8 raised it, audit 9 and `req/238` §6 measured it still answering
    /// `error: unrecognized subcommand 'list'`. The finding is not "a listing is missing" but "a
    /// **body-less** draft is uncountable": `Engine::submit` records `DraftCreated{intent_id}` in
    /// the journal and the CLI writes the body beside it in `.gx/drafts/`, and the two can come
    /// apart — a `.gx/drafts/` an operator emptied, a project restored without it, a draft filed
    /// by a binary that crashed between the two writes. `gx plan` then answers "the draft is
    /// missing" for an intent the journal witnesses.
    ///
    /// So the list is taken from the **journal** (Σ's `drafts` component) and each row says
    /// whether the body is on the disk, and the rows whose body is gone are counted separately.
    /// A read: no lock, no writer's door (`req/215` M-02's rule for `verdict-checkpoint list`).
    List {
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// 🔴 **E-M6-14** — discard a draft body. Writes nothing to the ledger.
    Discard {
        /// The `gx1:` **intent** id `gx submit` printed.
        intent: String,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum EscalationCmd {
    /// T-5 — "human ruling = Admit" (sem: SEM-gx-cli-439), to `Admitted` (AC-071).
    Approve {
        /// A `TicketId`, or the `TransformationId` 44 §2.2 uses for the same operation (M6-04 adopted (c); sem: SEM-gx-cli-440).
        id: String,
        /// 44 §1.2: "`--reason`: reason for the ruling (required)" (sem: SEM-gx-cli-441).
        #[arg(long, value_name = "TEXT")]
        reason: String,
        /// The ruler's key id. 43 T-5: "the ruler holds a valid signing key" (sem: SEM-gx-cli-442).
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// T-5b — "human ruling = Deny" (sem: SEM-gx-cli-443), to `Denied` (AC-072).
    Reject {
        /// A `TicketId`, or the `TransformationId` 44 §2.2 uses (M6-04 adopted (c); sem: SEM-gx-cli-444).
        id: String,
        /// 44 §1.2: "`--reason`: reason for the ruling (required)" (sem: SEM-gx-cli-445).
        #[arg(long, value_name = "TEXT")]
        reason: String,
        /// The ruler's key id.
        #[arg(long, value_name = "KEY_ID")]
        actor_key: Option<String>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum PolicyCmd {
    /// 44 §1.2: "Cedar policy syntax/schema verification" (sem: SEM-gx-cli-446) — and the invariant half (FR-027, M6-21).
    Lint {
        /// The pack to read.
        path: PathBuf,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 44 §1.2: "run Gate evaluation against the specified scenario … and check it against the
    /// expected value" (sem: SEM-gx-cli-447).
    Test {
        /// The pack to decide with.
        path: PathBuf,
        /// The scenarios: `Intent`/`Evidence`/expected `Verdict`, as JSON.
        #[arg(long, value_name = "FILE")]
        scenario: PathBuf,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum VerdictCheckpointCmd {
    /// Close the current window, sign the counts, append them to the chain.
    Issue {
        /// The signing key. Required for `gx log checkpoint`'s reason (§47 M6-24: "only the ledger's
        /// owner can make one"; sem: SEM-gx-cli-448) — a count published about a deployment is a statement by that deployment.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// The namespace (42 §3.11's `origin`).
        #[arg(long, default_value = gx_cli::verdict::DEFAULT_VERDICT_ORIGIN)]
        origin: String,
        /// Also write it here, byte for byte as stdout carries it.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// Check one checkpoint or a chain of them.
    Verify {
        /// The documents, in chain order. `-` reads one from stdin.
        #[arg(value_name = "FILE|-")]
        files: Vec<String>,
        /// The public key the chain was signed with. Without it the signature check says `skipped`.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// A signed ledger head (`gx log checkpoint`) to bind the chain against.
        #[arg(long, value_name = "FILE")]
        ledger_checkpoint: Option<PathBuf>,
        /// Recount the verdicts from this project's journal and compare (AC-VC-2's half).
        #[arg(long)]
        recount_from_journal: bool,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// The chain this deployment has published.
    List {
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum ReceiptCmd {
    /// Show a stored receipt, at one of 48 §3.1's four disclosure levels (M6-16 adopted (a); sem: SEM-gx-cli-449).
    Show {
        /// The `gx1:` transformation id.
        transformation: String,
        /// 1=verdict badge, 2=summary, 3=full expansion, 4=raw signatures. Default 1.
        #[arg(long, default_value_t = 1, value_name = "1..4")]
        level: u8,
        /// "`--json` is always the full amount" (M6-16 adopted (a); sem: SEM-gx-cli-450): equivalent to `--level 4`.
        #[arg(long)]
        json: bool,
    },
    /// Verify a receipt: DSSE signature, canonical CID, and inclusion against a known checkpoint.
    Verify {
        /// The receipt file, or `-` for stdin (44 §1.2).
        file: String,
        /// Do not consult any ledger. AC-057's mode.
        #[arg(long)]
        offline: bool,
        /// The known `Checkpoint` to check the inclusion proof against.
        #[arg(long, value_name = "FILE")]
        checkpoint: Option<PathBuf>,
        /// 🔴 The key the `--checkpoint` was signed with (**M6H8-11 adopted (b)**, req/38 §55; sem: SEM-gx-cli-451). Without it
        /// the anchor is taken on trust and the answer says `anchor_authenticated: false`.
        #[arg(long, value_name = "FILE")]
        checkpoint_key: Option<PathBuf>,
        /// 🔴 **H-09** — the `ConsistencyProof` (`gx log consistency --from <receipt's tree_size>
        /// --to <checkpoint's tree_size>`) that ties the tree this receipt names to the tree the
        /// anchor names (RFC 6962 §2.1.2). Needed only when the log has grown since the receipt was
        /// issued **and** the anchor came from a file; on the default path the local ledger produces
        /// one. Without it such a receipt answers `inclusion: unbridged` — not a pass, and not a
        /// refutation.
        #[arg(long, value_name = "FILE")]
        consistency: Option<PathBuf>,
        /// The public key. `gx key gen`'s output, or a gx key file. See M6H2-6.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// 🔴 **FR-M7-3**: the revocation list to consult (`gx key revoke`'s output). Without it the
        /// answer says `revocation: not_consulted`, which ASM-45-2 permits — "consulting the
        /// revocation list is optional on the verifier's side" (sem: SEM-gx-cli-452) — and which is a different word from `not_revoked`.
        #[arg(long, value_name = "FILE")]
        revocations: Option<PathBuf>,
        /// 🔴 How far back a revocation reaches. ASM-45-2's DEFAULT is `from-revocation`
        /// ("a receipt issued before revocation is not retroactively invalidated"; sem: SEM-gx-cli-453); `all` refuses every receipt the key ever
        /// signed and is the setting a compromise is answered with, because it reads no clock.
        #[arg(
            long,
            value_name = "from-revocation|all",
            default_value = "from-revocation"
        )]
        retroaction: String,
    },
    /// 🔴 **P-1b** (`req/544` §3 R-3c) — which of the four questions this receipt answers, and
    /// which it does not.
    ///
    /// Derived from the fifteen members the receipt already carries: no field was added to make
    /// this answerable, so a receipt issued before this verb existed has a coverage table too. The
    /// table cannot disagree with the receipt because it is not stored anywhere — it is a function
    /// of the document being read.
    ///
    /// With `--face`, the face's own claim is printed **beside** the receipt's answer, in a
    /// separate vocabulary. A face that claims it can observe reads and a receipt that answers
    /// `unknown` are both correct at once, and the answer says so rather than refusing.
    Coverage {
        /// The receipt file.
        file: String,
        /// A face declaration (`.gx/faces/<face>.json`) to print the claim from.
        #[arg(long, value_name = "FILE")]
        face: Option<PathBuf>,
        /// 44 §1.2's flag. Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
}

/// 🔴 **R6 / DR-43-10** — the head an auditor keeps.
#[derive(Debug, clap::Subcommand)]
enum CheckpointCmd {
    /// Copy this project's recorded signed head to a file outside `.gx/`.
    ///
    /// No key is needed: the document is already signed. What comes out is exactly what
    /// `gx receipt verify --offline --checkpoint <FILE>` reads, so the export is usable by somebody
    /// who holds no key of this project's at all.
    Export {
        /// Where to write it. A path outside the project is the entire point (`req/229` §7-4).
        file: PathBuf,
        /// 🔴 **AC-B5 / `req/682` §2-3** — also write the C2SP tlog-checkpoint **body** here, the
        /// three-line note text a public transparency log ingests. Generated only: this verb never
        /// publishes it (`req/682` §4). Omitted, only the JSON at `<FILE>` is written, as before.
        #[arg(long, value_name = "PATH")]
        note_out: Option<PathBuf>,
        /// Output is JSON either way (44 §1.3).
        #[arg(long)]
        json: bool,
    },
    /// 🔴 **AC-B5 / `req/682` §2-2** — name any contradiction across collected checkpoints, offline.
    ///
    /// Two signed checkpoints of one `origin` at the same `tree_size` with different roots are two
    /// attested histories of one length — the failure a transparency log exists to make impossible,
    /// and one no signature can reconcile. This is how an operator reaches
    /// `gx_log::detect_equivocation` over a set of `gx checkpoint export` files. Exit 7 when a
    /// contradiction is found, 0 with a soundness note when none is. No network.
    ///
    /// 🔴 **B-audit M-1 / N-47** — `--proof` is the second branch of `req/682` §2-2's own detector
    /// pair: two checkpoints of one origin at **different** sizes, bridged by their own consistency
    /// proof (`gx log consistency --from <old> --to <new>`), are classified as a genuine extension or
    /// a `fork` via `gx_log::classify_extension`. Omitted, `audit` answers exactly as before
    /// (equivocation only) — this is additive, not a behaviour change to the default road.
    Audit {
        /// The checkpoint files to audit (the JSON `gx checkpoint export` writes). At least one.
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,
        /// 🔴 The public key to verify each checkpoint's signature under before auditing it. Omitted,
        /// the arithmetic still runs but `signatures_verified` is `false`: a forged checkpoint could
        /// then manufacture a false equivocation, which is why passing it is the stronger audit.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// 🔴 **B-audit M-1 / N-47** — a `ConsistencyProof` (the JSON `gx log consistency` prints)
        /// bridging exactly the two `--files` given, by size. Requires exactly two files; `classify_extension`
        /// needs one pair and one proof, and more than a pair leaves ambiguous which two the proof
        /// bridges. Reuses `gx_cli::receipt::read_consistency` — one document, two commands (H-09's
        /// reasoning applied here).
        #[arg(long, value_name = "FILE")]
        proof: Option<PathBuf>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Subcommand)]
enum LogCmd {
    /// An inclusion proof for one leaf (44 §1.2).
    Proof {
        /// A leaf index or a `gx1:` transformation id.
        #[arg(long, value_name = "INDEX|TID")]
        leaf: String,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// A consistency proof between two tree sizes (44 §1.2).
    Consistency {
        #[arg(long, value_name = "SIZE")]
        from: u64,
        #[arg(long, value_name = "SIZE")]
        to: u64,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Publish a signed checkpoint of the current tree (**M6-24 adopted (b)**; not in 44 §1.1, M6H2-7; sem: SEM-gx-cli-454).
    Checkpoint {
        /// The ledger signing key. Required: only the log's owner can publish its head.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// The log's namespace (42 §3.11).
        #[arg(long, default_value = ledger::DEFAULT_ORIGIN)]
        origin: String,
        /// Also write the checkpoint here, byte for byte as stdout carries it.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, clap::Subcommand)]
enum KeyCmd {
    /// Generate an Ed25519 signing key (44 §1.2, FR-020).
    Gen {
        #[arg(long, default_value = keys::ALGORITHM)]
        alg: String,
        /// Where to write the secret. Defaults to req/56 §3's `~/.gx/keys/<key_id>.key`.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// 🔴 **FR-M7-4**: record the generated id in the project's `.gx/config.toml` as
        /// `engine_signing_keyid`, which is where `gx serve` reads it (E-M6-7). Without this flag
        /// the slot has a reader and no writer (M6H7-8), and a fresh volume has no way to fill it.
        #[arg(long)]
        record: bool,
        /// 🔴 **P2 item2** (`req/130` §1, NFR-010): a **file** holding a passphrase. When given, the
        /// secret is written encrypted (argon2id + ChaCha20-Poly1305) instead of plaintext-0600.
        /// Opt-in (ruling 2; sem: SEM-gx-cli-455): omitted, `gen` writes exactly what it always has. The **path** is the
        /// argument and the passphrase is not, for `--token-file`'s reason (`crates/gx-cli/src/
        /// serve.rs`'s module header).
        #[arg(long, value_name = "PATH")]
        passphrase_file: Option<PathBuf>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// List locally known key ids (44 §1.2).
    List {
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Revoke a key (**FR-M7-3**, ruling #6; not in 44 §1.2 — see `gx_cli::keys::revoke`; sem: SEM-gx-cli-456).
    Revoke {
        /// The key to revoke. Its secret must still be in the store: a revocation is signed by the
        /// key it revokes.
        #[arg(long, value_name = "KEY_ID")]
        key_id: String,
        /// Why, in the operator's words. It travels inside the signed entry.
        #[arg(long, default_value = "revoked by the operator")]
        reason: String,
        /// Write the list here instead of `~/.gx/keys/revocations.json`.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
    /// 🔴 Generate a successor and revoke the predecessor, in one command (**FR-M7-3**).
    Rotate {
        /// The key being rotated out.
        #[arg(long, value_name = "KEY_ID")]
        key_id: String,
        #[arg(long, default_value = keys::ALGORITHM)]
        alg: String,
        #[arg(long, default_value = "rotated by the operator")]
        reason: String,
        /// Record the **successor**'s id in `.gx/config.toml` (FR-M7-4). A rotation whose server
        /// keeps signing with the revoked key is a rotation that did nothing.
        #[arg(long)]
        record: bool,
        /// 44 §1.2's flag.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // `--help` and `--version` arrive here as errors and are not errors: clap models them as
            // a parse that terminated early, and 44 §1.4's 0 is "normal termination" (sem: SEM-gx-cli-457).
            //
            // 🔴 `DisplayHelpOnMissingArgumentOrSubcommand` is **not** in the set, and hand 1's
            // mapping had it there. The two are different events: "the operator asked for help" and
            // "the operator named a verb that needs a sub-verb and gave none" (sem: SEM-gx-cli-458). With one level of
            // subcommands the distinction never fired; with two, `gx receipt` took it — a command
            // that did nothing and exited 0. 44 §1.4's 0 is "reached the intended state" (sem: SEM-gx-cli-459) and no state was
            // reached, so it is 1. Raised as **M6H2-11**.
            let asked_for_output = matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            // 🔴 **R15 / `req/259` H-01** — the one write in this binary that does not go through
            // `gx_cli::emit`, and it is named rather than hidden.
            //
            // clap owns this one: the argument vector did not parse, so there is no `Cli`, no verb
            // and no `Outcome` yet — a usage error is the road that runs **before** anything this
            // repository writes. Its delivery is clap's own (`anstream`), which returns a value and
            // does not panic on a refused write: `req/259`'s control arm and this lane's C4
            // measured `gx repair --no-such-flag` at rc **1** with stderr on `/dev/null` and on
            // `/dev/full` alike, three runs each, on both binaries. So the discard here costs the
            // usage text and nothing else, and D-6 carries this site as the census's stated
            // denominator rather than pretending the count is zero.
            let _clap_delivered_its_own_usage = e.print();
            // 🔴 Not `e.exit()`, which would use clap's 2. discipline 52 (sem: SEM-gx-cli-460).
            return ExitCode::from(if asked_for_output { OK } else { ERROR });
        }
    };

    // 🔴 `gx __demo-notes-server` (ruling 1; sem: SEM-gx-cli-461) is intercepted **before** the generic `run`/`print_json`
    // pipeline, for `gx wrap`'s own reason (this module's `Wrap` arm doc comment): its stdout
    // carries MCP frames and nothing else while it runs. Unlike `wrap`, this verb has no room for
    // a trailing summary line after the loop ends -- by the time `serve_notes`'s read loop sees
    // EOF, the parent `gx wrap` process (`StdioClient::shutdown`) may already have closed its read
    // end of this process's stdout, and a `println!` into a closed pipe panics (Rust's `print!`
    // family does not return a `Result`). Found by running `gx demo` for real, not by reading the
    // MCP transport specification a second time.
    if matches!(cli.command, Command::DemoNotesServer) {
        return match gx_cli::demo::serve_notes() {
            Ok(outcome) => settled(outcome.code),
            Err(e) => refuse(&e),
        };
    }

    let pretty = cli.pretty;
    match run(&cli) {
        // 🔴 **R13 / `req/244` H-01** — the report is delivered, and the delivery is answered for.
        //
        // This arm used to be `print_json(&outcome.json, pretty); ExitCode::from(outcome.code)`,
        // and `print_json` was a `println!`. Rust's `print!` family does not return a `Result`, so
        // a write error inside it **panics**: the audit measured `gx repair --yes` writing
        // `.gx/VERSION` and then ending at exit **101** with a Rust panic string on stderr, three
        // destinations (a reader that closed first, `> /dev/full`, a reader that takes one byte),
        // three runs each, no variation. 101 is in no table this repository publishes, so a script
        // could not tell "gx answered" from "gx crashed"; and the next `gx repair` said
        // `meta_repaired: []` about the file that run had just created.
        //
        // Both halves are closed here. The delivery is [`Outcome::emit`], which returns a value; a
        // failure of it is 44 §1.3's problem object on stderr under `OUTPUT_FAILED` and 44 §1.4's
        // **1**. And what the run had already written is not lost: `gx repair --yes` files
        // `.gx/repair/last.json` before it returns (see `repair::repair_and_report`), and the next
        // `gx repair` reads it back under `previous_repair`.
        //
        // 🔴 **R14 / `req/246` H-01** — and the stderr half of both arms is the same road now. See
        // [`refuse`].
        Ok(outcome) => match outcome.emit(&mut std::io::stdout().lock(), pretty) {
            // 🔴 **R15 / `req/259` H-01** — and the sentences this run could not put beside the
            // answer are read here, once. See [`settled`].
            Ok(()) => settled(outcome.code),
            Err(why) => refuse(&Error::OutputFailed {
                detail: why.to_string(),
            }),
        },
        // 44 §1.3: "errors are printed to stderr … and stdout emits nothing (or emits no partial
        // result)" (sem: SEM-gx-cli-462).
        Err(e) => refuse(&e),
    }
}

/// 🔴 **R14 / `req/246` H-01** — put a refusal on stderr, and answer with the status this run
/// determined.
///
/// # What the fourteenth audit measured
///
/// R13 closed stdout ([`Outcome::emit`]) and made the failure of it 44 §1.3's problem object under
/// `OUTPUT_FAILED`. The object was delivered by `eprintln!`, and Rust's `eprint!` family returns
/// nothing for the same reason `print!` does — so a write error inside it **panics**. The same macro
/// carried **every verb's every refusal**, which is the object 44 §1.3 puts on that stream. Five
/// arms, three runs each, no variation, exit **101** in all fifteen:
/// `gx receipt show gx1:doesnotexist 2>/dev/full` (a read verb, healthy stdout, nothing written),
/// `gx submit` refusing `CONFIG_ABSENT` the same way, `gx repair --yes > /dev/full 2>/dev/full`,
/// `gx repair --yes 2>&1 | true` — which is one token a buyer writes every day — and
/// `gx limits > /dev/full 2>/dev/full`. clap's own usage error stayed at **1** in the same
/// conditions, which is how the finding is known to be about this road and not about the process.
///
/// # Why there is no fallback, and what the status is
///
/// When the stream that carries "this could not be delivered" cannot itself be written, gx has
/// nothing left to say it on. Inventing a third channel would be inventing a place an operator does
/// not read; panicking is what `req/246` H-01 **is**. So the answer is the exit status, and the
/// status is **the one this run had already determined** — `e.exit_code()`, which is inside 44
/// §1.4's table by construction.
///
/// That choice is the one a script can act on. `2>/dev/null` and `2>/dev/full` are two ways of
/// throwing stderr away, and they now give the **same** status for the same run; folding a dead
/// stderr into `OUTPUT_FAILED`'s 1 would have made `gx receipt show <missing>` answer "not found"
/// with 6 in one and "error" with 1 in the other. `OUTPUT_FAILED` keeps the case it was minted for
/// — the **answer** on stdout was lost, so exit 0 would have claimed an answer that never arrived —
/// and does not spread to the case where the answer was a refusal the status already names.
///
/// What a `gx repair --yes` run wrote is still readable afterwards: `.gx/repair/last.json` and the
/// next `gx repair`'s `previous_repair` (R13, and `req/246` M-01 for the road that did not file
/// one).
fn refuse(e: &Error) -> ExitCode {
    // The one deliberate discard in this binary, and it is deliberate: the value says whether the
    // sentence explaining the refusal arrived, and there is no surviving stream to report *that* on.
    // `gx_cli::emit::problem_line` is where the write and the flush are answered for, so the drop
    // here is of a fact with nowhere to go rather than of a fact nobody looked at.
    let _stderr_took_it = gx_cli::emit::problem_line(&e.problem());
    settled(e.exit_code())
}

/// 🔴 **R15 / `req/259` H-01** — the status this run ends with, and the one place a sentence that
/// went nowhere is answered for.
///
/// # What the fifteenth audit measured
///
/// R14 moved 44 §1.3's problem object onto [`gx_cli::emit::problem_line`] and defined "every stream
/// that carries an answer" as the sites carrying a `.problem()`. `req/259` H-01 re-implemented the
/// census against the **destination** instead and found **forty-three** `eprintln!` sites still
/// standing in `crates/gx-cli/src/`, every one of which ends the whole run at exit **101** when the
/// destination refuses. Two of them were answers by any reading — `gx wrap`'s start-up JSON, and
/// this binary's note that `gx serve`'s start-up line could not be delivered — and the cheapest
/// measurement was the first command a buyer runs: `gx key gen --json 2>/dev/full`, exit **101**,
/// stdout **empty**, and the secret key already on the disk under a name nothing had printed.
///
/// # Why the number does not move
///
/// Every one of those sites is now [`gx_cli::emit::note`], whose failures are counted rather than
/// dropped, and this is where the count is read. What it does with it is **nothing**, and that is
/// the decision rather than the omission:
///
/// * a run whose status depended on whether stderr had been thrown away with `2>/dev/null` or with
///   `2>/dev/full` is a run no script can branch on — R14 argued exactly this for a refusal, and a
///   sentence beside an answer has strictly less claim on the status than a refusal does;
/// * there is no third stream to say it on. 44 §1.3 fixes what stdout carries (`gx key gen --json >
///   pub.json` must be those two fields and nothing else), so borrowing it would corrupt the answer
///   in order to report a note;
/// * `OUTPUT_FAILED` keeps the case it was minted for — the **answer** on stdout was lost, so exit 0
///   would have claimed an answer that never arrived. Here the answer did arrive.
///
/// What is left for the operator is written in `docs/LIMITS.md` v0.5-c: keep gx's stderr alive if
/// you want its sentences, and if you did not, the facts are still recoverable from the project and
/// the key store (`gx key list` names both halves of a key `gx key gen` made).
///
/// # 🔴 **R16 / `req/262` H-01** — and the count is of the **binary**, not of this crate
///
/// The sixteenth audit's finding was not that R15's predicate was wrong but that its **window** was:
/// the census looked at `crates/gx-cli/src/` and the thing that ships is `gx`, a binary that links
/// fourteen workspace crates. Six `eprintln!` sites stood in `gx-api` and seven more in
/// `gx-mcp-wire`, all thirteen inside the artefact and outside the count — and the measured cost of
/// the `gx-api` six was an HTTP request that ended with **no status line at all** (0 bytes, three
/// runs) on a project whose `.gx/drafts` was read-only with `gx serve 2>/dev/full`, against `201`
/// for the same request with the same project and `2>/dev/null`.
///
/// Cargo forbids one shared module — 47 §1(a) makes this crate the one that folds the others in, so
/// the edge back does not exist, and the crate all three share (`gx-core`) is "no I/O" by 41 §6. So
/// there are three roads, named in `probes/doubt/tests/declaration_writer_doubt.rs` D-6, and this
/// function is the one **reading**: the sum below is the artefact's count of sentences that went
/// nowhere, which is the unit the decision above is about.
fn settled(code: u8) -> ExitCode {
    // The count is read so that the choice is a choice. The three `note` roads are where the
    // failure stopped being a panic; this line is where it stops being a fact anybody can act on,
    // and the paragraph above is why there is nowhere else for it to go.
    //
    // 🔴 **R16** — three summands, one per crate in this binary that holds a standard stream. A
    // fourth road appearing without a summand here is red in D-6 rather than in the next audit.
    let _sentences_with_nowhere_to_go = gx_cli::emit::notes_undelivered()
        + gx_api::notes::notes_undelivered()
        + gx_mcp_wire::notes::notes_undelivered();
    ExitCode::from(code)
}

// 🔴 **R13 / `req/244` H-01 + L-03** — `print_json` is gone, and its two faults with it.
//
// 44 §1.3's sentence ("a command that returns a single object puts a single newline-terminated
// JSON on stdout"; sem: SEM-gx-cli-463) is now kept by `Outcome::emit`, in the crate that owns the
// type, where the failure of a write is a value the caller cannot drop. What this function did
// wrong twice: `println!` panicked on a write error (`req/244` H-01, exit 101), and
// `text.unwrap_or_default()` printed an **empty line** at exit 0 when the serialiser refused
// (`req/244` L-03 — no reachable input produces it today, which is not the same fact as "there is
// no road"). Both are `io::Result` in `Outcome::emit`.

/// The project root every `.gx/`-bound command resolves against.
fn project(cli: &Cli) -> Result<PathBuf> {
    match &cli.project {
        Some(p) => Ok(p.clone()),
        None => std::env::current_dir().map_err(|e| Error::Io {
            action: "read",
            path: ".".to_string(),
            source: e,
        }),
    }
}

/// Parse 42 §1.2's `gx1:<base32>` into a transformation id.
///
/// [`Cid::from_text`] and never a mint — Rule 1 (i) (sem: SEM-gx-cli-464). The parser is in gx-core exactly so that this
/// line does not have to reach gx-canon (see the crate root).
fn transformation_id(text: &str) -> Result<TransformationId> {
    Cid::from_text(text)
        .map(TransformationId)
        .map_err(|e| Error::Usage {
            detail: format!("`{text}` is not a `gx1:` id: {e}"),
        })
}

/// 42 §3.1's four substrates, from `--substrate`. The spelling is [`gx_cli::substrate_kind`]'s:
/// `gx policy test`'s scenario file reads the same names and 44 §1.2 gives them one vocabulary.
fn substrate(text: &str) -> Result<gx_core::SubstrateKind> {
    gx_cli::substrate_kind(text)
}

/// 42 §3.2's `ChangeContext`, from `--context`. Shared for [`substrate`]'s reason.
fn context(text: &str) -> Result<gx_core::ChangeContext> {
    gx_cli::change_context(text)
}

/// 42 §3.2's `Actor`, from `--actor-kind` / `--actor-key` / `--actor-model`.
///
/// 🔴 `--actor-model` is **required** for an agent and refused for the other two. 42 §3.2: "`Agent` (sem: SEM-gx-cli-465)
/// adds `model` because that is the one fact about an agent a human reviewer needs and cannot
/// recover from the key" (sem: SEM-gx-cli-466) — a default would put a made-up model in a signed provenance record, which
/// is the shape M5H4-4 refused for `adapter_version`.
fn actor(kind: &str, key: &str, model: Option<&str>) -> Result<gx_core::Actor> {
    use gx_core::Actor as A;
    let key = key.to_string();
    match (kind, model) {
        ("human", None) => Ok(A::Human { key }),
        ("process", None) => Ok(A::Process { key }),
        ("agent", Some(model)) => Ok(A::Agent {
            key,
            model: model.to_string(),
        }),
        ("agent", None) => Err(Error::Usage {
            detail:
                "--actor-kind agent needs --actor-model (42 §3.2: the one fact about an agent a \
                     reviewer cannot recover from the key)"
                    .to_string(),
        }),
        ("human" | "process", Some(_)) => Err(Error::Usage {
            detail: "--actor-model belongs to `--actor-kind agent`; 42 §3.2 gives `Human` and \
                     `Process` a key and nothing else"
                .to_string(),
        }),
        (other, _) => Err(Error::Usage {
            detail: format!("--actor-kind takes human|agent|process (44 §1.2); got {other:?}"),
        }),
    }
}

/// 🔴 The bytes `--intent <FILE|->` carried, **unparsed**.
///
/// 44 §1.2 calls it "Intent JSON (a JSON body equivalent to 42 §3.3's `goal` field)" (sem: SEM-gx-cli-467) and this reads it as
/// bytes. The reason is **E-M4-2**: `GoalBytes` is opaque, and "what these bytes mean" is the one
/// interpretation P-6 reserves to an adapter — the fs adapter reads them as a file's new content and
/// says so in its own module. A CLI that parsed the body would be the second place in the system
/// that has an opinion about a goal, and the first place a JSON re-serialisation could change the
/// `IntentId` (42 §3.3 puts the goal in the identity). Raised as **M6H3-8**: 44 says JSON and this
/// stores whatever it was given.
fn intent_bytes(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| Error::Io {
                action: "read",
                path: "<stdin>".to_string(),
                source: e,
            })?;
        return Ok(buf);
    }
    std::fs::read(path).map_err(|e| Error::Io {
        action: "read",
        path: path.to_string(),
        source: e,
    })
}

/// 🔴 **P3** — the MCP server this invocation named, started and handshaken, or "none" (sem: SEM-gx-cli-468).
///
/// Built **once** per process and only when `--mcp-server` was given: a verb that names no server
/// spawns nothing, so `gx key list` is still a command that starts no child. The handshake happens
/// here rather than lazily inside the adapter because a revision this build has not read the
/// specification for is a refusal an operator should get **before** a transformation is drafted
/// (AC-P3-2), not in the middle of a commit.
fn mcp_wiring(cli: &Cli) -> Result<gx_cli::session::McpWiring> {
    let Some(command) = &cli.mcp_server else {
        return Ok(gx_cli::session::McpWiring::default());
    };
    let env = gx_cli::wrap::environment(&cli.mcp_server_env)?;
    let client = std::sync::Arc::new(
        gx_mcp_wire::StdioClient::spawn_with_env(command, &cli.mcp_server_arg, &env).map_err(
            |e| Error::Usage {
                detail: e.to_string(),
            },
        )?,
    );
    let handshake = client.initialize().map_err(|e| Error::Usage {
        detail: e.to_string(),
    })?;
    let endpoint = cli
        .mcp_endpoint
        .clone()
        .unwrap_or_else(|| gx_mcp_wire::stdio_endpoint(command));
    let catalogue =
        gx_cli::wrap::catalogue(&cli.mcp_restore, cli.mcp_restore_catalogue.as_deref())?;
    // stderr, not stdout: 44 §1.3 fixes stdout to the command's single JSON object, and which
    // server a verb connected to is a note an operator needs and a pipe does not. The count is the
    // catalogue's own (`declared()`), so a `--mcp-restore-catalogue` file's entries are in it.
    gx_cli::note!(
        "gx: connected to {command:?} as {endpoint:?} over MCP {} ({} restorable tool(s) declared)",
        handshake.revision,
        catalogue.declared()
    );
    Ok(gx_cli::session::McpWiring::wired(
        std::sync::Arc::new(gx_mcp_wire::WireTransport::new(client, endpoint)),
        catalogue,
    ))
}

/// 🔴 **R19 / `req/279` H-02 (c)** (`req/284` §1.1) — which verbs read the six MCP globals.
///
/// `clap`'s `global = true` puts `--mcp-server` and its five companions on **every** subcommand.
/// Until this lane exactly five verbs called [`mcp_wiring`]; the other sixteen accepted the flags
/// and dropped them, and audit 19 measured what that costs: `gx escalation approve --mcp-server …`
/// exited 1 with a refusal that named `--mcp-server` as the remedy, having thrown that very flag
/// away. Three verbs are wired by this lane (`escalation approve`/`reject` through
/// `escalation_cmd`, and `serve` through `serve_cmd`); the rest are answered here.
///
/// A list rather than a trait or a field on each variant: the set is small, it is the thing the
/// audit counted, and a `match` with no wildcard is a compile error the day a verb is added — which
/// is the property that keeps this honest, because the next `Command` variant cannot be silently
/// filed under "accepts and drops".
fn reads_the_mcp_wiring(command: &Command) -> bool {
    match command {
        // The five `mcp_wiring` has always been called from: everything that opens an engine which
        // may plan, judge, apply or invert an MCP change.
        Command::Submit { .. }
        | Command::Plan { .. }
        | Command::Verify { .. }
        | Command::Commit { .. }
        | Command::Undo { .. }
        // 🔴 R19's three: 43 T-5's ruling verb and the long-lived HTTP surface.
        | Command::Escalation { .. }
        | Command::Serve { .. }
        // 🔴 **S③** (`req/493`) — `gx confine` reads `--mcp-restore` / `--mcp-restore-catalogue`
        // and **only** those two of the six. The catalogue is the whole input to its derivation:
        // whether the named tool may write at all is a question only that file answers. It opens
        // no transport and starts no server, so `--mcp-server` and its three companions are as
        // homeless here as on `gx cancel` — which is why this arm goes through
        // [`confine_catalogue`] rather than [`mcp_wiring`], and why that function refuses the
        // four it does not read instead of accepting and dropping them (audit 19's finding).
        | Command::Confine { .. } => true,
        // 🔴 `gx wrap` names its server **after `--`** and carries its own `--server-env`,
        // `--endpoint`, `--restore` and `--restore-catalogue`. The globals are a second spelling it
        // does not read, and two spellings of "which server" on one command is the ambiguity this
        // refusal exists to prevent.
        Command::Wrap { .. }
        // 🔴 **P-1a** — `gx attach` places a directory and starts nothing. Pointing a route at the
        // membrane is `gx wrap --adopt-config`'s and is a separate invocation (`req/535` §8's
        // P-1b), so the six globals have nowhere to go here for `gx cancel`'s reason exactly.
        | Command::Attach { .. }
        // Read verbs, key management, policy linting, the demo walk and the two local repairs:
        // none of them opens a road to a server, and `gx cancel` in particular does not (43 T-7
        // aborts a row and applies nothing).
        | Command::Cancel { .. }
        | Command::Policy { .. }
        | Command::Draft { .. }
        | Command::Demo { .. }
        | Command::DemoNotesServer
        | Command::Limits { .. }
        | Command::VerdictCheckpoint { .. }
        | Command::Receipt { .. }
        | Command::Log { .. }
        | Command::Key { .. }
        | Command::Checkpoint { .. }
        | Command::Repair { .. }
        | Command::Replay { .. } => false,
    }
}

/// The six globals, and whether this invocation gave each one.
fn mcp_flags_given(cli: &Cli) -> Vec<&'static str> {
    let mut given = Vec::new();
    if cli.mcp_server.is_some() {
        given.push("--mcp-server");
    }
    if !cli.mcp_server_arg.is_empty() {
        given.push("--mcp-server-arg");
    }
    if !cli.mcp_server_env.is_empty() {
        given.push("--mcp-server-env");
    }
    if cli.mcp_endpoint.is_some() {
        given.push("--mcp-endpoint");
    }
    if !cli.mcp_restore.is_empty() {
        given.push("--mcp-restore");
    }
    if cli.mcp_restore_catalogue.is_some() {
        given.push("--mcp-restore-catalogue");
    }
    given
}

/// 🔴 **M6H3-5's rule, one flag family along**: a flag with nowhere to go is refused, not dropped.
///
/// The same shape `gx undo --actor-key` and `gx cancel --actor-key` already take, and for the same
/// reason `req/279` H-02 gives for putting it here: accepting a flag tells an operator that
/// something was wired. `gx serve --mcp-server` used to start a server that could not reach one
/// MCP call, and the only way to find out was to make a ruling and read a 502.
///
/// # Errors
/// [`Error::Usage`] — 44 §1.4's **1**, "invalid input", which is where every other
/// nowhere-to-go flag in this binary lands.
fn refuse_unused_mcp_flags(cli: &Cli) -> Result<()> {
    if reads_the_mcp_wiring(&cli.command) {
        return Ok(());
    }
    let given = mcp_flags_given(cli);
    if given.is_empty() {
        return Ok(());
    }
    let wrap_note = if matches!(cli.command, Command::Wrap { .. }) {
        ". `gx wrap` names its server after `--` and carries its own `--server-env`, \
         `--endpoint`, `--restore` and `--restore-catalogue`; use those"
    } else {
        ". The verbs that read them are `submit`, `plan`, `verify`, `commit`, `undo`, \
         `escalation approve|reject` and `serve`"
    };
    Err(Error::Usage {
        detail: format!(
            "{} has nowhere to go on this command: it is a `clap` global, so every verb parses it, \
             and this one opens no road to an MCP server (`req/279` H-02){wrap_note}. A flag this \
             verb accepted and dropped would tell you a server had been wired when none had",
            given.join(", ")
        ),
    })
}

fn run(cli: &Cli) -> Result<Outcome> {
    // 🔴 **R19 / `req/279` H-02 (c)** — before a project is opened and before a server is started,
    // for the ordering hand 2 fixed for every other "invalid input": a flag that is wrong is wrong
    // wherever it was typed.
    refuse_unused_mcp_flags(cli)?;
    match &cli.command {
        Command::Submit {
            substrate: kind,
            locator,
            intent,
            context: ctx,
            actor_key,
            actor_kind,
            actor_model,
            order,
            parent,
            json: _,
        } => {
            // Arguments first, project second — hand 2's order, for hand 2's reason: a locator that
            // is not a locator is "invalid input" (sem: SEM-gx-cli-469) wherever it was typed.
            let spec = gx_cli::pipeline::SubmitSpec {
                substrate: substrate(kind)?,
                locator: locator.clone(),
                goal: intent_bytes(intent)?,
                context: context(ctx)?,
                actor: actor(actor_kind, actor_key, actor_model.as_deref())?,
                order: *order,
                parents: parent
                    .iter()
                    .map(|p| transformation_id(p))
                    .collect::<Result<Vec<_>>>()?,
            };
            // 🔴 `gx submit` is the one verb that **creates** `.gx/`. 44 has no `gx init`, and the
            // alternative — every verb creating — would answer a mistyped directory by starting a
            // second, empty ledger beside the operator's real one.
            let mut session = Session::open_wired(
                &project(cli)?,
                true,
                Vec::new(),
                None,
                None,
                &mcp_wiring(cli)?,
            )?;
            // Rule 2: the clock and the entropy source, each read in one place, at the outside edge (sem: SEM-gx-cli-470).
            gx_cli::pipeline::submit(&mut session, &spec, rng::seed(), clock::now())
        }
        Command::Plan { id, json: _ } => {
            let mut session = Session::open_wired(
                &project(cli)?,
                false,
                Vec::new(),
                None,
                None,
                &mcp_wiring(cli)?,
            )?;
            gx_cli::pipeline::plan(&mut session, id, clock::now())
        }
        Command::Verify {
            transformation,
            evidence,
            record_only,
            policy,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            let evidence = gx_cli::pipeline::read_evidence(evidence)?;
            // 🔴 **E-M6-12** — `--policy` is not in 44 §1.2's synopsis. req/38 §50 M6H3-9 adopted (a) (sem: SEM-gx-cli-471)
            // adds it, and `Session::open_with_policy` carries the reason: without a pack that
            // denies a **writable** path, DR-2's record-only commit could only be exercised
            // against `/etc`, which is a write to `/etc`.
            let mut session = Session::open_wired(
                &project(cli)?,
                false,
                evidence,
                None,
                policy.as_deref(),
                &mcp_wiring(cli)?,
            )?;
            // 44 §1.2: "force per this command (overriding the global setting)" (sem: SEM-gx-cli-472) — the per-call argument of
            // M6-08 adopted (a), not `with_mode`.
            let mode = record_only.then_some(EnforcementMode::RecordOnly);
            gx_cli::pipeline::verify(&mut session, &id, mode, clock::now())
        }
        Command::Commit {
            transformation,
            idempotency_key,
            record_only,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            // 🔴 **M6H3-7**: `--record-only` is not in 44 §1.2's `gx commit` synopsis, and 44 §1.2's
            // [DR-2 sensitivity] paragraph for this command is normative — "if a target whose
            // Verdict=Deny is `commit`ted under record-only mode, apply goes through but
            // `Receipt.enforced=false` is stamped, and the exit code is 0" (sem: SEM-gx-cli-473).
            // The mode that paragraph is about reaches T-8r through `canonicalize`, which `gx commit`
            // drives, and the only inputs 44 gives a single-shot CLI are per-command flags; without
            // one the paragraph names an outcome no `gx` invocation can produce. This is the shape
            // **E-M6-4** already ruled once (M6H2-6's `--key`, without which FR-052 was
            // unconstructible), and it is raised rather than assumed.
            //
            // 🔴 Hand 3 wrote this as `with_mode` and said why: "`canonicalize` reads the engine's
            // setting, and a second per-call parameter is a ruling this hand does not have" (sem: SEM-gx-cli-474). **This
            // hand has it** — E-M6-20 (req/38 §52) put `record_only` in 44 §2.2's commit body, and a
            // long-lived server has no other road to T-8r (M6-08 ruled the mode-swap form "must not
            // be adopted"; sem: SEM-gx-cli-475). So `canonicalize` now takes the argument `verify` already took, and both
            // surfaces drive one spelling: a per-call override, never a setting on shared state.
            let mode = record_only.then_some(EnforcementMode::RecordOnly);
            let mut session = Session::open_wired(
                &project(cli)?,
                false,
                Vec::new(),
                None,
                None,
                &mcp_wiring(cli)?,
            )?;
            gx_cli::pipeline::commit(
                &mut session,
                &id,
                idempotency_key.as_deref(),
                mode,
                clock::now(),
            )
        }
        Command::Undo {
            transformation,
            actor_key,
            idempotency_key,
            policy,
            settle,
            retry,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            // 🔴 M6H3-5's rule one verb along: a flag with nowhere to go is refused, not dropped.
            // `Engine::undo` mints T_u's `Intent` with **T_o's** context and actor (P-5), so the
            // key that signs is the original actor's and `--actor-key` selects nothing (M6H4-3).
            if let Some(key) = actor_key {
                return Err(Error::Usage {
                    detail: format!(
                        "--actor-key {key:?} has nowhere to go: 43 §5-1 makes T_u's intent carry \
                         T_o's own context and actor, so the receipt is signed with the original \
                         actor's key and this flag would select nothing (M6H4-3)"
                    ),
                });
            }
            // 🔴 **P3** — `--mcp-server` matters most here: 43 §5's inverse of a tool call **is a
            // call**, so an undo in a process holding no transport is one that can be planned and
            // not performed (`session::MCP_REGISTRATION_FIRED`).
            let mut session = Session::open_wired(
                &project(cli)?,
                false,
                Vec::new(),
                None,
                policy.as_deref(),
                &mcp_wiring(cli)?,
            )?;
            // 🔴 `--retry` (§98 ruling 2's D-complement; sem: SEM-gx-cli-476): re-fire on `Aborted(ApplyFailed)` alone.
            // The loop lives here rather than in the library so that each attempt is a whole,
            // ordinary `lifecycle::undo` — fresh timestamp, fresh seed, its own T_u honestly in
            // the journal — and not a partial replay of one.
            let mut attempt: u32 = 0;
            loop {
                let outcome = gx_cli::lifecycle::undo(
                    &mut session,
                    &id,
                    idempotency_key.as_deref(),
                    rng::seed(),
                    clock::now(),
                    *settle,
                )?;
                if outcome.code == gx_cli::exit::APPLY_FAILED && attempt < *retry {
                    attempt += 1;
                    gx_cli::note!(
                        "gx undo --retry: attempt {attempt}/{retry} re-fires after \
                         Aborted(ApplyFailed); the aborted attempt stays journalled as its own \
                         transformation"
                    );
                    continue;
                }
                break Ok(outcome);
            }
        }
        Command::Cancel {
            transformation,
            actor_key,
            json: _,
        } => {
            let id = transformation_id(transformation)?;
            // 43 T-7's owner guard has no enforcement point in v0.1 (M5H6-4 adopted (a); sem: SEM-gx-cli-477): `Engine::cancel`
            // takes no actor and the `Aborted` record has no field for one. Accepting the flag and
            // dropping it would tell an operator a permission was checked.
            if let Some(key) = actor_key {
                return Err(Error::Usage {
                    detail: format!(
                        "--actor-key {key:?} has nowhere to go: v0.1 has no authorization layer \
                         (M5H6-4 adopted (a); sem: SEM-gx-cli-478), `Engine::cancel` takes no actor and 43 T-7's `Aborted` \
                         record has no field for one, so nothing would check the permission the \
                         flag names (M6H4-3)"
                    ),
                });
            }
            // 🔴 **`req/291` M-01** — the wiring is still the empty one (`gx cancel` opens no road
            // to a server and [`reads_the_mcp_wiring`] still answers `false` for it), but it is now
            // told **which** verb is holding it. The resume a cancel needs re-plans, a re-plan
            // snapshots, and on an MCP row the snapshot refuses; until this lane that refusal named
            // `--mcp-server` as its remedy — the flag this same verb refuses as a usage error one
            // line above. A refusal whose remedy is another refusal is not a remedy.
            let mut session = Session::open_wired(
                &project(cli)?,
                false,
                Vec::new(),
                None,
                None,
                &gx_cli::session::McpWiring::default()
                    .on_surface(gx_cli::session::McpSurface::NoRoad("cancel")),
            )?;
            gx_cli::lifecycle::cancel(&mut session, &id, clock::now())
        }
        Command::Escalation { cmd } => escalation_cmd(cli, cmd),
        Command::Policy { cmd } => match cmd {
            PolicyCmd::Lint { path, json: _ } => gx_cli::policy::lint(path),
            PolicyCmd::Test {
                path,
                scenario,
                json: _,
            } => gx_cli::policy::test(path, scenario),
        },
        Command::Draft { cmd } => draft_cmd(cli, cmd),
        Command::Serve {
            bind,
            record_only,
            fail_posture,
            tls_cert,
            tls_key,
            token_file,
            signing_key,
        } => serve_cmd(
            cli,
            &gx_cli::serve::ServeSpec {
                bind: bind.clone(),
                record_only: *record_only,
                fail_posture: fail_posture.clone(),
                tls: (tls_cert.clone(), tls_key.clone()),
                token_file: token_file.clone(),
                signing_key: signing_key.clone(),
            },
        ),
        Command::Attach {
            json: _,
            route_config,
            server_name,
            declared,
        } => {
            // 🔴 A route is a pair — a file and the entry in it — and half a pair is a question
            // nobody can answer, so it is refused rather than guessed at.
            let route = match (route_config, server_name) {
                (Some(path), Some(name)) => Some((path.clone(), name.clone())),
                (None, None) => None,
                (Some(_), None) => {
                    return Err(gx_cli::Error::Usage {
                        detail:
                            "--route-config needs --server-name: a configuration holds several \
                                 servers and a face is about one of them"
                                .to_string(),
                    })
                }
                (None, Some(_)) => {
                    return Err(gx_cli::Error::Usage {
                        detail: "--server-name needs --route-config: without the file there is no \
                                 entry to look the name up in"
                            .to_string(),
                    })
                }
            };
            gx_cli::attach::run(
                &project(cli)?,
                &gx_cli::attach::CoverageInput {
                    route,
                    declared: declared.clone(),
                },
            )
        }
        Command::Wrap { .. } => wrap_cmd(cli),
        Command::Demo { json: _ } => gx_cli::demo::run(),
        // 🔴 Unreachable in practice: `main` intercepts this variant before calling `run` (see
        // `main`'s own comment on that branch, right above its `match Cli::try_parse()`). The arm
        // stays for exhaustiveness -- `Command` is matched elsewhere too, and a variant with no
        // arm here is a compile error the day this function's shape changes -- and it calls the
        // same function `main`'s early branch does, so the two paths cannot answer differently.
        Command::DemoNotesServer => gx_cli::demo::serve_notes(),
        Command::Limits { json } => gx_cli::limits::run(*json),
        Command::Confine {
            tool,
            allow_write,
            plan_only,
            json: _,
            cmd,
        } => confine_cmd(
            cli,
            &gx_cli::confine::ConfineSpec {
                tool: tool.clone(),
                allow_write: allow_write.clone(),
                plan_only: *plan_only,
                cmd: cmd.clone(),
            },
        ),
        Command::VerdictCheckpoint { cmd } => verdict_checkpoint_cmd(cli, cmd),
        Command::Receipt { cmd } => receipt_cmd(cli, cmd),
        Command::Log { cmd } => log_cmd(cli, cmd),
        Command::Key { cmd } => key_cmd(cli, cmd),
        Command::Repair {
            yes,
            signing_key,
            against,
            accept_rollback,
            reissue_receipts,
            json: _,
        } => gx_cli::repair::run_accepting(
            &project(cli)?,
            signing_key.as_deref(),
            *yes,
            against.as_deref(),
            *accept_rollback,
            *reissue_receipts,
        ),
        Command::Checkpoint {
            cmd:
                CheckpointCmd::Export {
                    file,
                    note_out,
                    json: _,
                },
        } => {
            let layout = gx_cli::layout::Layout::open(&project(cli)?)?;
            gx_cli::ledger::export(&layout, file, note_out.as_deref())
        }
        Command::Checkpoint {
            cmd:
                CheckpointCmd::Audit {
                    files,
                    key,
                    proof,
                    json: _,
                },
        } => gx_cli::ledger::audit(files, key.as_deref(), proof.as_deref()),
        Command::Replay {
            transformation,
            from,
            to,
            dry_run,
            json: _,
        } => {
            let layout = Layout::open(&project(cli)?)?;
            let journal = replay::open(&layout)?;
            // The ledger is the independent witness `matches` is about (see the module header). Its
            // absence is reported rather than treated as agreement — `ledger_consulted: false` and
            // exit 1, which is 44 §1.2's "unable to execute".
            //
            // 🔴 **DR-43-7** — and now the *reason* is said out loud. `ledger::open` refuses a torn
            // tail instead of silently repairing it (`req/215` H-03), so `.ok()` would turn the one
            // fact this verb exists to deliver into a `false` in a JSON field. This verb is what
            // `gx serve`'s start-up refusal recommends; a recommended diagnostic that answers
            // "could not check" and does not say why is not one. The note goes to stderr, beside
            // 44 §1.3's single JSON object on stdout, for the reason `gx serve` prints its resumed
            // count there: a side fact an operator has to go looking for is one they will not find.
            let store = match ledger::open(&layout) {
                Ok(store) => Some(store),
                Err(why) => {
                    gx_cli::note!("gx replay: the ledger was not consulted: {why}");
                    None
                }
            };
            let range = match (transformation, from, to) {
                (Some(tid), _, _) => replay::Range::Transformation(transformation_id(tid)?),
                (None, Some(from), Some(to)) => replay::Range::Records {
                    from: *from,
                    to: *to,
                },
                (None, None, None) => replay::Range::All,
                (None, _, _) => {
                    return Err(Error::Usage {
                        detail: "--from and --to come as a pair (44 §1.2)".to_string(),
                    })
                }
            };
            replay::replay(&journal, store.as_ref(), &range, *dry_run)
        }
    }
}

/// 🔴 **E-M6-14** — `gx draft discard <IntentId>`, the verb E-M6-1 made necessary.
///
/// 44 L101's `gx cancel` from-set once contained `Draft`; §47 M6-03 adopted (c) took it out, because a
/// draft has no `TransformationId` to cancel (req/88 Λ3: "id-resolution solves the problem of
/// 'pointing' at something, but not the problem of 'there being no seat'"; sem: SEM-gx-cli-479). What was left was an intent an operator had submitted and could not put
/// down.
///
/// 🔴 **The ledger does not learn about it, and that is the ruling rather than an omission.**
/// M5H6-1 refused a fourteenth journal record (`Aborted{intent_id, OwnerCancelled}`) on the grounds
/// that the vocabulary would grow with nothing to protect, and §51 M6H4-2 adopted (a) fixed the verb with
/// "one line in the doc: 'discarding a draft is an operation that does not land in the ledger'" (sem: SEM-gx-cli-480). Here is that line, and
/// `crates/gx-cli/tests/m6h6_cli.rs` is the count that keeps it true: a discard removes a file and
/// appends no record. What is discarded was never a transformation — 42 §1.3-3's state table starts
/// at `Candidate` — so there is nothing about it for a ledger to be missing (sem: SEM-gx-cli-481).
///
/// A draft that is not there is 44 §1.4's **6**, never 0: answering "done" for a name the project
/// never held is "do not give skip and pass the same face" (sem: SEM-gx-cli-482) (req/29 §4) at the verb level.
/// 🔴 **R10 / audit 8 M-06 + L-02** — `gx draft list`.
///
/// Σ's `drafts` component is the list the **journal** witnesses; `.gx/drafts/` is where the CLI
/// keeps the bodies. A row in the first with no file in the second is the state audit 8 asked for a
/// verb to be able to count, and it is reported as a count rather than left for a reader to derive
/// from two lists.
///
/// # Errors
/// As `Layout::open`, plus [`Error::Io`] if the journal will not open.
fn draft_list(cli: &Cli) -> Result<Outcome> {
    let layout = Layout::open(&project(cli)?)?;
    // The reader's door: no lock and no repair. `gx repair`'s report mode opens the same way, and
    // `req/215` M-02 is the finding that made a *read* taking the writer lock a defect.
    let engine = gx_cli::session::open_engine_read_only(
        &layout,
        gx_engine::InjectedEvidence::none(),
        gx_core::FailPosture::FailClosed,
    )?;
    let store = gx_cli::draft::DraftStore::in_layout(&layout);
    let mut rows = Vec::new();
    let mut bodyless = 0usize;
    for draft in engine.sigma().drafts() {
        let path = store.path_of(&draft.intent_id);
        let held = path.exists();
        if !held {
            bodyless += 1;
        }
        rows.push(serde_json::json!({
            "intent_id": draft.intent_id.0.to_text(),
            "rng_seed": draft.rng_seed,
            "body_present": held,
            "path": path.display().to_string(),
        }));
    }
    let total = rows.len();
    Ok(Outcome::ok(serde_json::json!({
        "drafts": rows,
        "count": total,
        // 🔴 The number audit 8 M-06 asked for. A draft the journal witnesses and `.gx/drafts/` has
        // no body for cannot be planned, and before this verb there was no way to be told so
        // without running `gx plan` on it and reading the refusal.
        "bodyless": bodyless,
        "bodies_dir": layout.join("drafts").display().to_string(),
    })))
}

fn draft_cmd(cli: &Cli, cmd: &DraftCmd) -> Result<Outcome> {
    let intent = match cmd {
        DraftCmd::List { .. } => return draft_list(cli),
        DraftCmd::Discard { intent, .. } => intent,
    };
    let intent_id = gx_core::IntentId(Cid::from_text(intent).map_err(|e| Error::Usage {
        detail: format!("`{intent}` is not a `gx1:` intent id: {e}"),
    })?);
    let layout = Layout::open(&project(cli)?)?;
    let drafts = gx_cli::draft::DraftStore::in_layout(&layout);
    let path = drafts.path_of(&intent_id);
    if !drafts.remove(&intent_id)? {
        return Err(Error::NotFound {
            what: "draft",
            id: intent.clone(),
        });
    }
    // 🔴 The index entry goes too. `.gx/index/` is req/56 §2's "derived, safe to delete" (sem: SEM-gx-cli-483) cache of
    // `IntentId → TransformationId`, and a resolution pointing at a body that is gone would make
    // `gx plan` answer "the draft is missing" (sem: SEM-gx-cli-484) for an intent the operator deliberately discarded —
    // a 6 that reads like corruption. Best effort, because a cache that will not be written is not
    // a reason to fail an operation that succeeded.
    let (mut index, _) = gx_cli::index::ResolutionIndex::load(&layout);
    index.forget(&intent_id);
    let _ = index.store(&layout);
    Ok(Outcome::ok(serde_json::json!({
        "intent_id": intent_id.0.to_text(),
        "discarded": true,
        "path": path.display().to_string(),
        // The sentence the ruling asks for, on the wire as well as in the source: an operator
        // reading stdout is told that nothing was recorded, rather than having to infer it.
        "ledger": "unchanged: discarding a draft is not a transition (E-M6-14; 43 T-7 acts on a                    transformation and a draft is not one yet)",
    })))
}

/// 🔴 `gx serve` (44 §1.1) — build the four things a server needs, then run until a signal.
///
/// The judgement about which crate holds what is in `gx_cli::serve`'s module header, and the runtime
/// is `gx_api::serve`: this function is the part 44 §1.3 makes gx-cli's, which is what reaches
/// **stdout**. Two structured lines, one at each end ("stdout: startup log (structured JSON
/// line)"; sem: SEM-gx-cli-485), and the
/// second one carries the exit — a shutdown that abandoned work says so in the log **and** in the
/// status, because 44's exit 0 is "normal termination" (sem: SEM-gx-cli-486) and M4H4-2 forbids spelling a crash path as one.
fn serve_cmd(cli: &Cli, spec: &gx_cli::serve::ServeSpec) -> Result<Outcome> {
    let store = KeyStore::user_default()?;
    // 🔴 **R19 / `req/279` H-02 (b)** — the long-lived road gets the same wiring the single-shot
    // verbs get, from the same function, so "which server did this invocation name" has one answer
    // in this binary rather than two.
    // 🔴 **`req/291` M-01** — the same wiring, told it is about to serve HTTP. A `gx serve` that
    // named no server refuses `undo`, a ruling, `cancel` and `verify` with 502, and until this lane
    // the remedy those 502s printed was "`--mcp-server <CMD>` wires one for a single-shot verb" —
    // a sentence about a road this surface is not on. The server's server is chosen at start-up.
    let mcp = mcp_wiring(cli)?.on_surface(gx_cli::session::McpSurface::Server);
    let mcp_summary = mcp.summary();
    let (state, bind) = gx_cli::serve::build(&project(cli)?, &store, spec, &mcp)?;
    let signing = state.keys().signing().key_id().clone();
    // 🔴 **DR-43-2** — read off the state before it moves into the server, because 44 §1.2 asks for
    // one start-up line and the facts the gate measured belong in it (`gx_cli::serve::build`).
    let runtime = state.startup().clone();
    let outcome = gx_api::serve(state, &gx_cli::serve::config(bind)?, |addr| {
        // 44 §1.2's start-up log, printed **after** the listener exists: a line that said "serving
        // on" (sem: SEM-gx-cli-487) before the bind succeeded would be a line an operator's script could act on before
        // the socket was there.
        //
        // 🔴 **R13 / `req/244` H-01** — through `emit`, and the refusal goes to stderr rather than
        // panicking. The callback `gx_api::serve` hands us returns `()`, so this is the one site
        // where the value cannot be propagated: a start-up line that will not write is not a reason
        // to refuse to serve (the socket is already open and the server is already the thing an
        // operator asked for), and it is every reason to say so. Never `println!`: this callback
        // runs inside the runtime, and a panic here would take a listening server down over a pipe.
        if let Err(why) = gx_cli::emit::line(
            &gx_cli::serve::start_line(addr, spec, &signing, &runtime, &mcp_summary).to_string(),
        ) {
            gx_cli::note!("gx serve: the start-up line could not be written to stdout ({why}); the server is listening (req/244 H-01)");
        }
    })
    .map_err(|e| Error::Usage {
        detail: e.to_string(),
    })?;
    let code = outcome.exit_code();
    let json = outcome.to_json();
    Ok(if code == OK {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, code)
    })
}

/// 🔴 `gx wrap` (**P3**, `req/119` §2) — three modes, and only one of them serves.
///
/// The destructuring is a `let ... else` rather than thirteen parameters: M5H5-1's rule is that a
/// parameter every caller ignores is a parameter every caller has to read, and this function has one
/// caller. The `else` arm is unreachable by construction — `run` dispatches on the same variant.
/// 🔴 **S③** (`req/493` §0) — derive the ruleset, report it, take it, become the command.
///
/// # Why this refuses four of the six MCP globals by hand
///
/// [`reads_the_mcp_wiring`] is a per-*verb* answer, and `gx confine` is the first verb for which
/// that granularity is too coarse: it reads two of the six (`--mcp-restore`,
/// `--mcp-restore-catalogue`) and opens no transport, so the other four have exactly the nowhere to
/// go that audit 19 measured on `gx escalation approve`. Saying `true` in that function and
/// stopping there would have this verb accept `--mcp-server` and drop it — the failure R19 exists
/// to close, reintroduced by a verb added after it.
fn confine_cmd(cli: &Cli, spec: &gx_cli::confine::ConfineSpec) -> Result<Outcome> {
    let homeless: Vec<&str> = [
        (cli.mcp_server.is_some(), "--mcp-server"),
        (!cli.mcp_server_arg.is_empty(), "--mcp-server-arg"),
        (!cli.mcp_server_env.is_empty(), "--mcp-server-env"),
        (cli.mcp_endpoint.is_some(), "--mcp-endpoint"),
    ]
    .into_iter()
    .filter_map(|(given, name)| given.then_some(name))
    .collect();
    if !homeless.is_empty() {
        return Err(Error::Usage {
            detail: format!(
                "{} has nowhere to go on `gx confine`: this verb reads the catalogue \
                 (`--mcp-restore`, `--mcp-restore-catalogue`) and starts no server. What it does \
                 with the catalogue is decide whether the command it is about to run may write at \
                 all; it never calls a tool, so there is no server for these to name. A flag this \
                 verb accepted and dropped would tell you a server had been wired when none had \
                 (`req/279` H-02)",
                homeless.join(", ")
            ),
        });
    }

    // The same reader `gx wrap` uses, so a catalogue file that parses for one parses for the other.
    // `Catalogue::from_json` runs `soundness()` on the bytes, which is what makes a malformed
    // declaration a refusal here rather than a ruleset derived from half a file.
    let catalogue =
        gx_cli::wrap::catalogue(&cli.mcp_restore, cli.mcp_restore_catalogue.as_deref())?;
    gx_cli::confine::run(&catalogue, spec, &project(cli)?, cli.pretty)
}

fn wrap_cmd(cli: &Cli) -> Result<Outcome> {
    let Command::Wrap {
        server,
        server_env,
        endpoint,
        resource_arg,
        restore,
        restore_catalogue,
        actor_key,
        actor_model,
        context,
        policy,
        record_only,
        fail_posture,
        otel_file,
        otel_locators,
        forward_resource_arg,
        adopt_config,
        check_config,
        detach_config,
        server_name,
        gx_binary,
    } = &cli.command
    else {
        return Err(Error::Usage {
            detail: "wrap_cmd was reached for another verb".to_string(),
        });
    };

    // --- B-1's two config modes, neither of which starts a server -------------------------------
    if let Some(path) = check_config {
        let name = server_name.clone().ok_or_else(|| Error::Usage {
            detail: "--check-config needs --server-name: a config holds several servers and the \
                     check is about one of them"
                .to_string(),
        })?;
        let document = read_json(path)?;
        let report = gx_mcp_wire::config::check(&document, &name);
        let json = report.to_json();
        // The passing state is "routed through gx **and** no entry starts the server directly" (sem: SEM-gx-cli-488).
        // Either half alone is a config that still has the direct road in it.
        return Ok(if report.wrapped && report.direct.is_empty() {
            Outcome::ok(json)
        } else {
            Outcome::refused(json, gx_cli::exit::VERIFY_FAILED)
        });
    }
    if let Some(path) = adopt_config {
        let name = server_name.clone().ok_or_else(|| Error::Usage {
            detail: "--adopt-config needs --server-name".to_string(),
        })?;
        let document = read_json(path)?;
        // 🔴 **`req/551` G-1** — the flag *names* come from `ADOPT_FLAG_NAMES` rather than from
        // literals here, because `--detach-config` reads them back to find where its own flags end
        // and the wrapped command begins. Two hands spelling the same four flags in two files is
        // exactly the drift that would make a reverse operation write a wrong `command` into an
        // operator's document, so there is one spelling and both hands use it.
        let [restore_flag, catalogue_flag, key_flag, model_flag] =
            gx_mcp_wire::config::ADOPT_FLAG_NAMES;
        let mut flags: Vec<String> = Vec::new();
        for entry in restore {
            flags.push(restore_flag.to_string());
            flags.push(entry.clone());
        }
        if let Some(catalogue_file) = restore_catalogue {
            flags.push(catalogue_flag.to_string());
            flags.push(catalogue_file.display().to_string());
        }
        if let Some(key) = actor_key {
            flags.push(key_flag.to_string());
            flags.push(key.clone());
        }
        if let Some(model) = actor_model {
            flags.push(model_flag.to_string());
            flags.push(model.clone());
        }
        let adoption =
            gx_mcp_wire::config::adopt(&document, &name, gx_binary, &flags).map_err(|e| {
                Error::Usage {
                    detail: e.to_string(),
                }
            })?;
        let body = serde_json::to_vec_pretty(&adoption.config).map_err(|e| Error::Malformed {
            what: "agent configuration",
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        std::fs::write(path, body).map_err(|e| Error::Io {
            action: "write",
            path: path.display().to_string(),
            source: e,
        })?;
        let report = gx_mcp_wire::config::check(&adoption.config, &name);
        return Ok(Outcome::ok(serde_json::json!({
            "adopted": name,
            "config": path.display().to_string(),
            "was": { "command": adoption.original.command, "args": adoption.original.args },
            "check": report.to_json(),
        })));
    }

    // 🔴 **P-1c** (`req/551` §2) — B-1's third config mode, and the only one that takes gx back out.
    // It sits beside the other two rather than under a verb of its own because what it undoes is
    // `--adopt-config`, and because `gx detach` opposite `gx attach` would name the wrong inverse.
    if let Some(path) = detach_config {
        let name = server_name.clone().ok_or_else(|| Error::Usage {
            detail: "--detach-config needs --server-name: a config holds several servers and a \
                     detach is about one of them"
                .to_string(),
        })?;
        let document = read_json(path)?;
        let (config, answer) = gx_cli::detach::run(path, &name, &document)?;
        // Written back through the same serialiser the adoption used, which is why the answer says
        // in `not_restored` that this puts the entry back and not the file.
        let body = serde_json::to_vec_pretty(&config).map_err(|e| Error::Malformed {
            what: "agent configuration",
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        std::fs::write(path, body).map_err(|e| Error::Io {
            action: "write",
            path: path.display().to_string(),
            source: e,
        })?;
        return Ok(Outcome::ok(answer));
    }

    // --- the serving mode ------------------------------------------------------------------------
    let mut server = server.iter();
    let command = server.next().cloned().ok_or_else(|| Error::Usage {
        detail: "`gx wrap [OPTIONS] -- <server command> [args...]` — the server this proxy stands \
                 in front of is the one argument it cannot default"
            .to_string(),
    })?;
    // 42 §3.2 gives an `Agent` a key **and** a model, and M5H4-4's rule is that a made-up value in a
    // signed provenance record is worse than a refusal. Every change through this proxy is an
    // agent's, so both are required here rather than defaulted.
    let actor_key = actor_key.clone().ok_or_else(|| Error::Usage {
        detail:
            "--actor-key names the agent whose calls this proxy carries (42 §3.2). `gx key gen` \
                 makes one, and `gx key list` shows the ids this store holds"
                .to_string(),
    })?;
    let actor_model = actor_model.clone().ok_or_else(|| Error::Usage {
        detail: "--actor-model is required for an agent: 42 §3.2 puts `model` on `Actor::Agent` \
                 because it is \"the one fact about an agent a human reviewer needs and cannot \
                 recover from the key\" (sem: SEM-gx-cli-489)"
            .to_string(),
    })?;
    gx_cli::wrap::run(
        &project(cli)?,
        &gx_cli::wrap::WrapSpec {
            command,
            args: server.cloned().collect(),
            env: server_env.clone(),
            endpoint: endpoint.clone(),
            resource_arg: resource_arg.clone(),
            restores: restore.clone(),
            restore_catalogue: restore_catalogue.clone(),
            actor_key,
            actor_model,
            context: context.clone(),
            policy: policy.clone(),
            record_only: *record_only,
            fail_posture: fail_posture.clone(),
            otel_file: otel_file.clone(),
            otel_locators: *otel_locators,
            forward_resource_arg: *forward_resource_arg,
        },
    )
}

/// Read a JSON document a command was pointed at.
fn read_json(path: &std::path::Path) -> Result<serde_json::Value> {
    let raw = std::fs::read(path).map_err(|e| Error::Io {
        action: "read",
        path: path.display().to_string(),
        source: e,
    })?;
    serde_json::from_slice(&raw).map_err(|e| Error::Malformed {
        what: "agent configuration",
        path: path.display().to_string(),
        detail: e.to_string(),
    })
}

/// 🔴 `gx verdict-checkpoint` (**P3 / FR-M04**, `req/119` §4).
fn verdict_checkpoint_cmd(cli: &Cli, cmd: &VerdictCheckpointCmd) -> Result<Outcome> {
    match cmd {
        VerdictCheckpointCmd::Issue {
            key,
            origin,
            out,
            json: _,
        } => {
            let path = key.as_ref().ok_or_else(|| Error::Usage {
                detail: "--key names the key these counts are signed with. §47 M6-24's rule for a \
                         tree head holds for a count as well: a statement a deployment publishes \
                         about itself is signed by that deployment and by nothing standing in for it"
                    .to_string(),
            })?;
            let pair = gx_witness::KeyPair::load(path)?;
            let mut session = Session::open(&project(cli)?, false, Vec::new(), None)?;
            gx_cli::verdict::issue(&mut session, &pair, origin, clock::now(), out.as_deref())
        }
        VerdictCheckpointCmd::Verify {
            files,
            key,
            ledger_checkpoint,
            recount_from_journal,
            json: _,
        } => {
            // 🔴 The project is opened **only** for `--recount-from-journal`. AC-057's environment
            // is a third party with a document and a key and no `.gx/` at all, and a verifier that
            // opened one unasked would fail in the one place it has to work.
            let session = if *recount_from_journal {
                Some(Session::open(&project(cli)?, false, Vec::new(), None)?)
            } else {
                None
            };
            gx_cli::verdict::verify(
                session.as_ref(),
                &gx_cli::verdict::VerifySpec {
                    files: files.clone(),
                    key: key.clone(),
                    ledger_checkpoint: ledger_checkpoint.clone(),
                    recount_from_journal: *recount_from_journal,
                },
            )
        }
        VerdictCheckpointCmd::List { json: _ } => {
            // 🔴 **DR-43-7 / `req/215` M-02** — a read opens the file, not the project. See
            // `gx_cli::verdict::list_from_file`: opening a `Session` here took the writer lock and
            // repaired what it read, and this verb does neither.
            let layout = Layout::open(&project(cli)?)?;
            gx_cli::verdict::list_from_file(&layout)
        }
    }
}

/// 43 T-5 / T-5b, from the command line.
///
/// The ruler is 42 §3.2's `Actor` and is **not** the transformation's own: 42 §3.13's `HumanDecision`
/// carries "the ruler, which is not `Transformation.actor` (the submitter)" (sem: SEM-gx-cli-490), because a record naming
/// only the submitter would say who asked and never who allowed. `--actor-key` therefore *does* have
/// somewhere to go here, unlike on `undo` and `cancel`, and when it is absent the ruling is filed
/// under the process's own key store default rather than being invented.
fn escalation_cmd(cli: &Cli, cmd: &EscalationCmd) -> Result<Outcome> {
    let (id, reason, actor_key, decision) = match cmd {
        EscalationCmd::Approve {
            id,
            reason,
            actor_key,
            ..
        } => (id, reason, actor_key, gx_cli::lifecycle::Decision::Approve),
        EscalationCmd::Reject {
            id,
            reason,
            actor_key,
            ..
        } => (id, reason, actor_key, gx_cli::lifecycle::Decision::Reject),
    };
    // 44 §1.2 makes `--reason` mandatory and `Engine::escalation` refuses an empty one; the refusal
    // is repeated here so that it arrives as "invalid input" (sem: SEM-gx-cli-491) before a project is opened rather than as an
    // engine error after one (the ordering hand 2 fixed for every other verb).
    if reason.trim().is_empty() {
        return Err(Error::Usage {
            detail: "--reason is required and cannot be blank (44 §1.2: \"reason for the ruling (required)\"; sem: SEM-gx-cli-492); \
                     AC-071/072 both ask the reason to reach the trail, and a ruling that says \
                     nothing is a ruling nobody can audit"
                .to_string(),
        });
    }
    // 🔴 **M6H4-6** — 44 §1.2 writes `[--actor-key <KEY_ID>]` as optional and this refuses without
    // it. 42 §3.13's `HumanDecision.actor` is "the ruler, which is **not** `Transformation.actor`
    // (the submitter)" (sem: SEM-gx-cli-493), and INV-S6 exists so that an escalation records **who allowed it**. There is
    // no honest default for who a person is: falling back to the submitter's key would file a
    // ruling under the name of the party the ruling is about, which is the one thing T-5 is for.
    let Some(key_id) = actor_key.clone() else {
        return Err(Error::Usage {
            detail: "--actor-key names the ruler and is required here, although 44 §1.2 writes it \
                     optional (M6H4-6). 42 §3.13's `HumanDecision` carries the **ruler** and not \
                     the submitter, and INV-S6 is why: an escalation records who allowed a change, \
                     so defaulting to the submitter's key would file the ruling under the name of \
                     the party it is about. `gx key list` shows the ids this store holds"
                .to_string(),
        });
    };
    let ruler = gx_core::Actor::Human { key: key_id };
    // 🔴 **R19 / `req/279` H-02 (a)** (`req/284` §1.1) — the ruling verb reads the server flags it
    // has always accepted.
    //
    // E-M3-4 is the sentence this product is sold on: gx asks a person before an effect it cannot
    // take back. On the MCP road the person's answer arrived here, and here opened the engine with
    // `Session::open` — which is `McpWiring::default()`, a process connected to nothing. So T-5's
    // snapshot hit `UnconfiguredTransport` and the ruling failed with `this "gx" is connected to no
    // MCP server: … "--mcp-server <CMD>" wires one for a single-shot verb` — naming, as its remedy,
    // a flag `clap` had put on this very command and this line had thrown away. Audit 19 measured
    // the whole road closed: `approve_rc=1`, object unchanged, and the same 502 on both HTTP
    // routes. `gx escalation reject` opens the same way for the same reason: T-5b writes a receipt
    // about a row whose position is on the server too.
    //
    // Flag-less invocations are unchanged — `mcp_wiring` returns the default when no `--mcp-server`
    // was given, so the refusal an operator sees today still stands (`req/284` §1.1 (c),
    // fail-closed unchanged).
    let mut session = Session::open_wired(
        &project(cli)?,
        false,
        Vec::new(),
        None,
        None,
        &mcp_wiring(cli)?,
    )?;
    gx_cli::lifecycle::escalation(&mut session, id, decision, reason, &ruler, clock::now())
}

fn receipt_cmd(cli: &Cli, cmd: &ReceiptCmd) -> Result<Outcome> {
    match cmd {
        // 🔴 **P-1b** — no `Layout` is opened and no key is needed: the coverage table is a
        // function of the document, so a third party holding one file can read it, which is the
        // same posture `--offline` verification already has.
        ReceiptCmd::Coverage {
            file,
            face,
            json: _,
        } => gx_cli::receipt::coverage(file, face.as_deref()),
        ReceiptCmd::Show {
            transformation,
            level,
            json,
        } => {
            // 🔴 The arguments are parsed **before** the project is opened. An id that is not an id
            // is "invalid input" (sem: SEM-gx-cli-494) wherever it was typed, and a version that answered "no `.gx/` here"
            // first would report the wrong one of two independent faults — and would make the
            // refusal depend on the working directory, which is how "it works on my machine" (sem: SEM-gx-cli-495) gets
            // into an error message.
            let id = transformation_id(transformation)?;
            let level = if *json { receipt::MAX_LEVEL } else { *level };
            let layout = Layout::open(&project(cli)?)?;
            let store = ReceiptStore::in_layout(&layout);
            receipt::show(&store, &id, level)
        }
        ReceiptCmd::Verify {
            file,
            offline,
            checkpoint,
            checkpoint_key,
            consistency,
            key,
            revocations,
            retroaction,
        } => {
            // 🔴 Read the receipt **before** anything is resolved from it. AC-057's environment has
            // no project directory at all, and a verifier that opened `.gx/` on the way in would
            // fail in the one place it has to work.
            let mut stdin_bytes = Vec::new();
            // 🔴 **R10 / `req/222` L-04** — a `gx1:` transformation id is resolved to the receipt
            // this project filed for it, instead of being opened as a filename.
            //
            // `req/222` L-04 raised it, `req/238` §6 measured it still current: `gx receipt verify
            // gx1:…` answered exit 6 `NOT_FOUND` "read gx1:…: No such file or directory", which is
            // the id being handed to `open(2)`. Every other receipt verb in 44 §1.2 takes the id
            // (`gx receipt show <ID>`), and the store that resolves it is three lines away.
            //
            // The **file** road is unchanged and is still first: AC-057's third party has a
            // document and no project, and a path that exists is a path. Only a string that parses
            // as a `gx1:` id and is not a file on the disk takes the new road, and a project that
            // has no receipt for it gets 44 §1.4's 6 with the id named — not a path.
            let resolved: Option<PathBuf> = match Cid::from_text(file).map(TransformationId) {
                Ok(id) if !std::path::Path::new(file).exists() => {
                    let layout = Layout::open(&project(cli)?)?;
                    let store = ReceiptStore::in_layout(&layout);
                    let path = store.path_of(&id, receipt::StoredKind::Commit);
                    if !path.exists() {
                        return Err(Error::NotFound {
                            what: "commit receipt (this project has filed none for that id; \
                                   `gx repair --yes --reissue-receipts` files what it can)",
                            id: file.clone(),
                        });
                    }
                    Some(path)
                }
                _ => None,
            };
            let source = if file == "-" {
                std::io::stdin()
                    .read_to_end(&mut stdin_bytes)
                    .map_err(|e| Error::Io {
                        action: "read",
                        path: "<stdin>".to_string(),
                        source: e,
                    })?;
                Source::Stdin(&stdin_bytes)
            } else if let Some(path) = &resolved {
                Source::File(path.as_path())
            } else {
                Source::File(std::path::Path::new(file))
            };

            // 🔴 **H-09**: read once, here, and answer from *this* copy. The anchor can no longer be
            // chosen without the receipt — a consistency proof runs between the `tree_size` the
            // receipt names and the one the head names, so the first number has to be in hand before
            // the ledger is opened for the second. Reading the path again later would let an
            // operator (or a race) substitute a different document between the two reads.
            let receipt = receipt::read(&source)?;
            let proof_size = receipt::proof_tree_size(&receipt);

            let public = match key {
                Some(path) => keys::read_public(path)?,
                None => {
                    // The owner's convenience path: the key id the receipt declares, in the local
                    // store. Never available to the third party AC-057 is about, which is why the
                    // flag exists at all.
                    let key_id = receipt.payload()?.key_id;
                    KeyStore::user_default()?.load(&key_id)?.public()
                }
            };

            // 🔴 **H-09** — the anchor, and (when the log has moved on) the bridge to it.
            //
            // `req/222` measured the shape this replaces: with the default anchor every receipt but
            // the newest came back `inclusion: "refuted"`, because the head is a checkpoint of a
            // **later** tree and an inclusion proof reaches exactly one root. Three commits, two
            // accusations. The repair is the standard transparency-log move (RFC 6962 §2.1.2) and
            // not a widening: on the default path the local ledger can *prove* that the tree the
            // receipt names grew into the tree the head names, so the chain closes with no
            // unchecked link. `--consistency` is the same proof handed in by a third party who
            // received it out of band; without either, the answer is `unbridged` and still not a
            // pass.
            let bridge_from_file = match consistency {
                Some(path) => Some(receipt::read_consistency(path)?),
                None => None,
            };
            let (anchor, bridge_from_ledger, anchor_source) = if *offline {
                match checkpoint {
                    Some(path) => (
                        Some(receipt::read_checkpoint(path)?),
                        None,
                        "checkpoint-file",
                    ),
                    // 44 §1.2 permits `--offline` alone. With no anchor a `CommitReceipt` reports
                    // `inclusion: unanchored`, which `Checks::verified` refuses to call a pass
                    // (H5-9) — so this is not a quiet downgrade, it is a visible one.
                    None => (None, None, "none"),
                }
            } else {
                match checkpoint {
                    Some(path) => (
                        Some(receipt::read_checkpoint(path)?),
                        None,
                        "checkpoint-file",
                    ),
                    None => {
                        let layout = Layout::open(&project(cli)?)?;
                        let store = ledger::open(&layout)?;
                        // 🔴 **R38 — RETRACTED, and the retraction is the interesting part.**
                        //
                        // R38 first put `ledger::refuse_if_the_two_files_disagree` here, on the
                        // argument that `anchor_source: "local-ledger"` is a claim about which tree
                        // this project is, made out of the same pair of files `gx log proof` reads,
                        // and that audit 37 had counted the family one mouth short.
                        //
                        // `serve_runtime_r6::dr4310_an_exported_head_refuses_a_project_that_went_backwards`
                        // refused it, and was right. That suite is DR-43-10's demonstration: a
                        // project is rolled back, and the point is that a removed commit's receipt
                        // answers `verified` against the checkpoint the auditor carried out of the
                        // machine and **`refuted` (exit 7) against the project's own ledger**. The
                        // gap between those two answers is the evidence. A gate here replaces the
                        // informative `refuted` with `LEDGER_DISAGREES` and deletes it.
                        //
                        // The distinction the first attempt missed: the three `gx log` verbs **mint
                        // a statement about** the tree, and a statement about a tree nobody can
                        // name is not honest. `gx receipt verify` **compares a document against**
                        // the tree, and a mismatch is the answer, not an obstacle to it. Reading
                        // the ledger is what makes this verb work, and it says which anchor it used
                        // (`anchor: "local-ledger"`) rather than implying a signed one.
                        let head = ledger::local_head(&store, clock::now())?;
                        // Only when the log really has grown past the receipt. A failure to prove
                        // it is **not** raised: the ledger is a party to the question, and a
                        // verification that died because its own log could not produce a witness
                        // would report a missing proof as a broken binary. It is left as `None`,
                        // and the answer says `unbridged`.
                        let bridge = match proof_size {
                            Some(size) if size < head.tree_size => {
                                gx_log::proof::prove_consistency(store.log(), size, head.tree_size)
                                    .ok()
                            }
                            _ => None,
                        };
                        (Some(head), bridge, "local-ledger")
                    }
                }
            };
            let bridge = bridge_from_file.as_ref().or(bridge_from_ledger.as_ref());
            // 🔴 **M6H8-11 adopted (b)** (req/38 §55; sem: SEM-gx-cli-496): the anchor's own signature, checked only when a key
            // for it is offered — and reported either way by `anchor_authenticated`. 45 ASM-45-1
            // allows the log's key to differ from the receipt's, so this is a second flag rather
            // than a reuse of `--key`.
            let anchor_authenticated = match (checkpoint_key, anchor.as_ref()) {
                (Some(path), Some(head)) => {
                    let anchor_key = keys::read_public(path)?;
                    match receipt::authenticate_anchor(head, &anchor_key) {
                        Ok(()) => true,
                        // 44 §1.2's `7=invalid` (sem: SEM-gx-cli-497), not an internal error: the checkpoint is part of what
                        // was being verified.
                        Err(e) => {
                            return Ok(receipt::anchor_refused(anchor_source, &e.to_string()))
                        }
                    }
                }
                (Some(_), None) => {
                    return Err(Error::Usage {
                        detail: "--checkpoint-key names the key a --checkpoint was signed with, \
                                 and no checkpoint was given. Authenticating an anchor that does \
                                 not exist is not a weaker check, it is no check"
                            .to_string(),
                    })
                }
                (None, _) => false,
            };
            // 🔴 **FR-M7-3**. The list is authenticated against the key being verified with — the
            // only key a third party holds — and the setting is the operator's, which is what
            // "retroaction scope is a policy setting" (sem: SEM-gx-cli-498) means at a command line.
            let ledger = match revocations {
                Some(path) => Some(receipt::read_revocation_ledger(path, &public)?),
                None => None,
            };
            if let Some((_, ignored)) = &ledger {
                if *ignored > 0 {
                    gx_cli::note!(
                        "gx: {ignored} revocation entr(y/ies) name other keys and were not checked \
                         — this verifier holds one public key (FR-M7-3)"
                    );
                }
            }
            let policy = match &ledger {
                Some((ledger, _)) => Some(gx_witness::RevocationPolicy {
                    ledger,
                    retroaction: retroaction_setting(retroaction)?,
                    // Rule 2: the verifier's own clock, read in the one place that reads one (sem: SEM-gx-cli-499).
                    verified_at: clock::now(),
                }),
                None => None,
            };
            let anchorage = anchor
                .as_ref()
                .map(|checkpoint| gx_witness::Anchorage { checkpoint, bridge });
            let outcome = receipt::judge(
                &receipt,
                &public,
                anchorage.as_ref(),
                anchor_source,
                anchor_authenticated,
                policy.as_ref(),
            );
            // 🔴 gotcha79 (req/38 §81, req/136 §4-5): `inclusion: refuted` reads identically to a
            // forged proof, but the audit lane's own first run hit it by pairing a receipt against
            // the wrong checkpoint (gotcha61/73 -- a proof is relative to a **tree_size**, not just
            // a log identity). One line on stderr, non-blocking, `--offline` only: the JSON on
            // stdout is unchanged.
            //
            // 🔴 **H-09** moved most of that traffic to `unbridged`, and gave it the note that says
            // what to do. The advice differs by word, which is the point of having two: `refuted`
            // now really is "this does not hold against a tree of the same size", while `unbridged`
            // is "you are holding two statements about two trees" and has an actionable answer.
            if outcome.json["checks"]["inclusion"].as_str()
                == Some(receipt::inclusion_json(
                    gx_witness::receipt::InclusionCheck::Unbridged,
                ))
            {
                let sizes = match (proof_size, anchor.as_ref().map(|a| a.tree_size)) {
                    (Some(from), Some(to)) => format!(" --from {from} --to {to}"),
                    _ => String::new(),
                };
                gx_cli::note!(
                    "gx: inclusion unbridged -- the anchor and this receipt name different tree \
                     sizes, so nothing was proved either way. Put the consistency proof between \
                     them in a file (`gx log consistency{sizes}`) and pass it as --consistency \
                     <FILE> (RFC 6962 2.1.2, req/222 H-09)"
                );
            }
            if *offline
                && outcome.json["checks"]["inclusion"].as_str()
                    == Some(receipt::inclusion_json(
                        gx_witness::receipt::InclusionCheck::Refuted,
                    ))
            {
                gx_cli::note!(
                    "gx: inclusion refuted -- check that --checkpoint is not later than the \
                     receipt's own commit (an inclusion proof is relative to the tree_size at \
                     commit time, req/38 gotcha61/73)"
                );
            }
            Ok(outcome)
        }
    }
}

/// `--retroaction`, as the setting `gx_witness` names.
///
/// A refusal rather than a fallback for a word this build does not have: the two settings answer
/// differently about the same receipt, so a misspelling that quietly became the default would give
/// an operator the opposite of what they asked for on exactly the run where it matters.
fn retroaction_setting(word: &str) -> Result<gx_witness::Retroaction> {
    gx_witness::Retroaction::ALL
        .into_iter()
        .find(|setting| setting.as_str() == word)
        .ok_or_else(|| Error::Usage {
            detail: format!(
                "--retroaction takes {:?}; got {word:?}. \"retroaction scope is a policy setting\" (req/98 §3-2; sem: SEM-gx-cli-500) and the \
                 two positions are 45 ASM-45-2's default and the one a compromise forces",
                gx_witness::Retroaction::ALL.map(gx_witness::Retroaction::as_str)
            ),
        })
}

fn log_cmd(cli: &Cli, cmd: &LogCmd) -> Result<Outcome> {
    let layout = Layout::open(&project(cli)?)?;
    match cmd {
        LogCmd::Proof { leaf, json: _ } => {
            // Same order as `receipt show`: the argument is parsed before the ledger is opened, so a
            // `--leaf` that is neither an index nor an id is "invalid input" (sem: SEM-gx-cli-501) rather than "no ledger".
            let leaf = ledger::Leaf::parse(leaf)?;
            let store = ledger::open(&layout)?;
            ledger::proof(&store, &leaf, Some(&layout))
        }
        LogCmd::Consistency { from, to, json: _ } => {
            let store = ledger::open(&layout)?;
            ledger::consistency(&store, *from, *to, Some(&layout))
        }
        LogCmd::Checkpoint { key, origin, out } => {
            let store = ledger::open(&layout)?;
            let path = key.as_ref().ok_or_else(|| Error::Usage {
                detail:
                    "--key names the ledger signing key. §47 M6-24: \"only the ledger's owner can make one\" (sem: SEM-gx-cli-502)\
                         — a checkpoint is a signed statement about this log and nothing else can \
                         stand in for the key"
                        .to_string(),
            })?;
            // 🔴 **R10 / audit 8 L-01** — `--key` takes a **file**, and a key **id** is now
            // answered rather than dropped through as an `INTERNAL`.
            //
            // The audit passed `ed25519-…` (the string every other verb's `--actor-key` and
            // `--signing-key` take) and got `{"gx_code":"INTERNAL","detail":"stat the key
            // (ed25519-…): No such file or directory (os error 2)"}` — 44 §2.3's word for "cannot
            // be classified" on a fact that classifies perfectly. Two roads and a refusal that
            // names both: a path that exists is read as a file, and anything else is looked up in
            // `~/.gx/keys/` under that id.
            let pair = if path.is_file() {
                gx_witness::KeyPair::load(path)?
            } else {
                let id = path.display().to_string();
                KeyStore::user_default()?
                    .load(&id)
                    .map_err(|_| Error::NotFound {
                        what: "ledger signing key (as a file path or as a key id in ~/.gx/keys/)",
                        id,
                    })?
            };
            ledger::checkpoint(
                &store,
                &pair,
                origin,
                clock::now(),
                out.as_deref(),
                Some(&layout),
            )
        }
    }
}

fn key_cmd(cli: &Cli, cmd: &KeyCmd) -> Result<Outcome> {
    let store = KeyStore::user_default()?;
    match cmd {
        KeyCmd::Gen {
            alg,
            out,
            record,
            passphrase_file,
            json: _,
        } => {
            // 🔴 The project is opened **only** when `--record` asks for it. 44 §1.2's `gen` has no
            // project at all, and a command that opened `.gx/` unasked would fail for an operator
            // making a key outside one — which is most of them (M6H2-6's shape: a verifier's
            // environment is not a project).
            let layout = if *record {
                Some(Layout::open(&project(cli)?)?)
            } else {
                None
            };
            // 🔴 **P2 item2**: read before generating, so a bad `--passphrase-file` refuses before a
            // key is drawn from the operating system's entropy rather than after — an operator who
            // mistyped the path gets the refusal without a key having been minted and then discarded.
            let passphrase = passphrase_file
                .as_deref()
                .map(keys::read_passphrase)
                .transpose()?;
            let outcome = keys::gen_recording(
                &store,
                alg,
                out.as_deref(),
                layout.as_ref(),
                passphrase.as_deref(),
            )?;
            // Where the secret was filed, on **stderr**: 44 §1.2 fixes stdout to two fields, and
            // `gx key gen --json > pub.json` has to produce exactly those two.
            let filed = out.clone().unwrap_or_else(|| {
                store.path_of(outcome.json["key_id"].as_str().unwrap_or_default())
            });
            // 🔴 **R15 / `req/259` H-01** — and the sentence says where the two public halves can
            // be read again.
            //
            // The audit's cheapest measurement was this command: `gx key gen --json 2>/dev/full`
            // ended at exit **101** with stdout empty and the secret already on the disk, so the
            // operator held a key and had no string that named it. The panic is gone (this is
            // `emit::note` now, and stdout carries 44 §1.2's two fields whatever stderr does), and
            // the second half of the repair is that neither field was ever only on a stream:
            // `gx key list` derives both from the file this run just wrote. Naming that verb here
            // costs nothing and exposes nothing — the secret's **location** is what this sentence
            // has always carried, and the key id and public key are public by construction.
            gx_cli::note!(
                "gx: secret key written to {} (req/56 §3). Its `key_id` and `public_key` are on \
                 stdout, and `gx key list` reads both back out of that file if this run's stdout \
                 went nowhere (req/259 H-01)",
                filed.display()
            );
            // 🔴 M6H2-10: `KeyPair::save` asks for 0600 and a filesystem with no unix permission
            // model (drvfs, 9p, a Windows share) silently gives 0777 instead — and `KeyPair::load`
            // then refuses the file this command just wrote. The refusal is right and its timing was
            // not: it arrived at the next command. Said here, where the operator can still choose a
            // different `--out`.
            if let Some(warning) = keys::permission_warning(&filed) {
                gx_cli::note!("gx: {warning}");
            }
            Ok(outcome)
        }
        KeyCmd::List { json: _ } => keys::list(&store),
        KeyCmd::Revoke {
            key_id,
            reason,
            out,
            json: _,
        } => {
            // Rule 2: the clock is read in `clock::now` and nowhere else (sem: SEM-gx-cli-503). There is no `--at` here for
            // M6-28's reason — a revocation whose moment is an argument is a boundary an operator
            // can move after the fact, and the boundary is the whole content of the record.
            let outcome = keys::revoke(&store, key_id, reason, clock::now(), None, out.as_deref())?;
            gx_cli::note!(
                "gx: the secret for {key_id:?} is kept — receipts it signed still have to verify \
                 (FR-M7-3)"
            );
            Ok(outcome)
        }
        KeyCmd::Rotate {
            key_id,
            alg,
            reason,
            record,
            json: _,
        } => {
            let layout = if *record {
                Some(Layout::open(&project(cli)?)?)
            } else {
                None
            };
            let outcome = keys::rotate(&store, alg, key_id, reason, clock::now(), layout.as_ref())?;
            if !*record {
                gx_cli::note!(
                    "gx: --record was not given, so `.gx/config.toml` still names the revoked key \
                     if it named one (FR-M7-4)"
                );
            }
            Ok(outcome)
        }
    }
}
