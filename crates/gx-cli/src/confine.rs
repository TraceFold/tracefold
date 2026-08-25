// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx confine -- <cmd>`: derive a kernel ruleset from the catalogue, take it, then become the
//! command (`req/493` §0).
//!
//! # Why stdout is not this verb's to write on
//!
//! After [`gx_confine::apply`] succeeds this process **`exec`s** the confined command, which
//! replaces the process image — there is no "after" in which to print a summary. So the
//! confinement report goes to **stderr, before the `exec`**, and stdout is left to the command.
//! `gx wrap`'s arm in `main.rs` gives the same reason for the same shape: a verb whose job is to
//! *become* another program does not own that program's stdout.
//!
//! `--plan-only` is the road for a caller that wants the report as an answer rather than as a
//! diagnostic: nothing is applied, nothing is `exec`ed, and the plan is an ordinary [`Outcome`] on
//! stdout.
//!
//! # What is enforced is narrower than what `req/493` §0 describes, and the difference is printed
//!
//! §0 asks for a ruleset derived from "the declared write-target set" that
//! `Catalogue::writes_per_this_file` returns. That function returns tool names, not paths — see the
//! `gx-confine` crate root, which sets out the mismatch and the bridge. The consequence for *this*
//! module is one field: `write_targets_are_declared` is `false` in every report this build emits,
//! because the paths came from the invocation and not from the catalogue. A report that omitted it
//! would let a reader believe a file had said something no file said.

use std::path::{Path, PathBuf};

use gx_adapter_mcp::catalogue::Catalogue;
use gx_confine::{ConfinePlan, Confinement};
use gx_witness::receipt::ConfinementContext;

use crate::exit::Outcome;
use crate::{Error, Result};

/// What the invocation asked for.
#[derive(Clone, Debug, Default)]
pub struct ConfineSpec {
    /// The tool whose declaration decides whether any write is permitted at all.
    pub tool: Option<String>,
    /// Paths the confined command may write beneath, subject to the catalogue's answer.
    pub allow_write: Vec<PathBuf>,
    /// Derive and print, apply nothing, run nothing.
    pub plan_only: bool,
    /// The command, from after `--`.
    pub cmd: Vec<String>,
}

/// 🔴 **`req/493` §1 AC-6** — the variable this verb sets on the process it becomes.
///
/// # Why a variable and not a file, a flag or a socket
///
/// `req/497` §7 named the shape and it is forced by the mechanism: `gx confine` **`exec`s**. The
/// process that will later run `gx commit` is this process, with a new image, and the only channel
/// that survives `execve` unchanged is the environment. A file would have to be found (by a path
/// this verb cannot know, since it does not know where the agent will run), a flag would have to be
/// threaded through a command line this verb does not own, and a socket would be a daemon.
///
/// It is also the channel that reaches the **children** of the confined command, which is the right
/// scope: Landlock's domain is inherited by every descendant, so every descendant's receipts are
/// entitled to say so.
pub const CONFINEMENT_ENV: &str = "GX_CONFINEMENT";

/// The value [`CONFINEMENT_ENV`] carries: `kernel_confined=<0|1>` and, when it is `1`,
/// `;ruleset_hash=<text>`.
///
/// Two fields, spelled rather than serialised. JSON would need a parser on the reading side whose
/// failure modes are a second vocabulary, and this value crosses a boundary where the reader has no
/// way to ask the writer what it meant — so the grammar is the smallest one that carries the two
/// facts `req/493` §1 AC-6 names and refuses everything else.
///
/// 🔴 It is **not** a credential and this build does not treat it as one: anything that can set a
/// variable in gx's environment can write this. See [`Engine::with_confinement`]'s note — the same
/// trust boundary `docs/LIMITS.md` already states for the rest of the build.
///
/// [`Engine::with_confinement`]: gx_engine::Engine::with_confinement
#[must_use]
pub fn declaration(confinement: &Confinement) -> String {
    // The bit is the fs face's, exactly as `Confinement::to_json` derives it: this build
    // constructs a ruleset for one face, and "confined" means that face is being held.
    if confinement.fs.is_enforcing() {
        format!(
            "kernel_confined=1;ruleset_hash={}",
            confinement.ruleset_hash
        )
    } else {
        "kernel_confined=0".to_string()
    }
}

/// Read [`CONFINEMENT_ENV`] back into the context a receipt carries.
///
/// `Ok(None)` when the variable is not set at all — the ordinary case, and the one in which the
/// caller states the unconfined default rather than an absence.
///
/// # 🔴 A malformed value is a refusal and not a default
///
/// The tempting reading of an unparseable value is "assume unconfined", and it is the wrong one in
/// the same way `req/38` §287-2's `generated_at` was: a field nobody checks is a field, not a claim.
/// A value this function cannot read means something set it, and the two candidates are a newer
/// `gx confine` writing a grammar this build does not know, and something else entirely. Under
/// "assume unconfined" the first produces receipts that quietly under-state a real confinement for
/// as long as the mismatch lasts. So the run stops, which is the posture the rest of this build
/// takes for a fact it cannot establish.
///
/// # Errors
/// [`Error::Usage`] naming the value, when it is set and cannot be read.
pub fn read_declaration(raw: Option<&str>) -> Result<Option<ConfinementContext>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let refuse = || Error::Usage {
        detail: format!(
            "`{CONFINEMENT_ENV}` is set to {raw:?}, which this build cannot read. The grammar is \
             `kernel_confined=0` or `kernel_confined=1;ruleset_hash=<text>`, and `gx confine` is \
             what writes it. gx refuses rather than assuming the process is unconfined: a value it \
             cannot read was set by something, and reading it as \"no confinement\" would put that \
             assumption inside a signature (`req/493` §1 AC-6)"
        ),
    };
    let mut parts = raw.split(';');
    let confined = match parts.next() {
        Some("kernel_confined=1") => true,
        Some("kernel_confined=0") => false,
        _ => return Err(refuse()),
    };
    let hash = match (confined, parts.next()) {
        (true, Some(rest)) => Some(rest.strip_prefix("ruleset_hash=").ok_or_else(refuse)?),
        // Both halves of the refusal: a `1` with nothing after it, and a `0` with something after
        // it. `ReceiptPayload::check_schema` refuses both shapes as well, and would catch these at
        // signing time; caught here they name `GX_CONFINEMENT` instead of naming the receipt, which
        // is where the operator can act.
        (true, None) | (false, Some(_)) => return Err(refuse()),
        (false, None) => None,
    };
    if hash.is_some_and(str::is_empty) || parts.next().is_some() {
        return Err(refuse());
    }
    Ok(Some(ConfinementContext {
        kernel_confined: confined,
        ruleset_hash: hash.map(str::to_string),
    }))
}

/// What this process's environment says confines it, as an engine wants it.
///
/// The unconfined statement when the variable is absent — never an absence. A `None` on a receipt
/// means "written before the erratum", and a build that carries the seat may not write one.
///
/// # Errors
/// [`Error::Usage`] when the variable is set and unreadable; see [`read_declaration`].
pub fn from_environment() -> Result<ConfinementContext> {
    Ok(
        read_declaration(std::env::var(CONFINEMENT_ENV).ok().as_deref())?
            .unwrap_or_else(ConfinementContext::unconfined),
    )
}

/// The `.gx/` store, relative to the project root.
fn store_of(project: &Path) -> PathBuf {
    project.join(".gx")
}

/// 🔴 **The fail-open this build can see and cannot close.**
///
/// A Landlock rule grants access *beneath* a path and ABI 1 has no way to punch a hole in one. So
/// if the project's `.gx/` sits under a granted writable path, the confined command can write the
/// store — which is `req/493` §2's Model B, explicitly out of scope here. It is reported rather
/// than fixed, because a confinement that printed "confined" while permitting exactly the write
/// the whole product exists to make un-forgeable would be worse than no confinement.
fn store_exposed(project: &Path, plan: &ConfinePlan) -> Option<String> {
    let store = store_of(project);
    gx_confine::is_inside(&store, plan.writable()).then(|| {
        format!(
            "{} is beneath a granted writable path, so the confined command can write this \
             project's own store. A Landlock rule grants access beneath a path and ABI 1 cannot \
             carve a hole in one, so this build cannot refuse it here. This is the adversary \
             `req/493` §2 puts out of scope (Model B); what closes it is a writable root that does \
             not contain `.gx/`",
            store.display()
        )
    })
}

/// The report both roads carry.
fn report(
    project: &Path,
    plan: &ConfinePlan,
    applied: Option<&Confinement>,
    cmd: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "plan": plan.to_json(),
        // 🔴 See this module's header. The paths are the invocation's, not the catalogue's, and
        // this field is the one place that fact is not left to be inferred.
        "write_targets_are_declared": false,
        "gx_store_exposed": store_exposed(project, plan),
        "command": cmd,
        "confinement": applied.map(Confinement::to_json),
    })
}

/// Derive, report, apply, and become the command.
///
/// # Errors
/// [`Error::Usage`] when there is no command to run or no road on this platform, and
/// [`Error::Io`] when the kernel refuses the ruleset. On every error road **nothing has been run**:
/// a `gx confine` that could not confine does not fall back to running the command unconfined.
pub fn run(
    catalogue: &Catalogue,
    spec: &ConfineSpec,
    project: &Path,
    pretty: bool,
) -> Result<Outcome> {
    let plan = ConfinePlan::derive(catalogue, spec.tool.as_deref(), &spec.allow_write);

    if spec.plan_only {
        return Ok(Outcome::ok(report(project, &plan, None, &spec.cmd)));
    }

    if spec.cmd.is_empty() {
        return Err(Error::Usage {
            detail: "`gx confine` needs a command after `--`, as in `gx confine --tool \
                     notes/write --allow-write ./workspace -- my-agent`. Applying a ruleset and \
                     then exiting would confine nothing: the domain belongs to the process that \
                     takes it, and this one is about to end. Use `--plan-only` to see what would \
                     be enforced without running anything"
                .to_string(),
        });
    }

    // 🔴 Applied **before** the report is written, so that what is reported is what the kernel
    // actually took rather than what was asked for. `RulesetStatus::PartiallyEnforced` is a real
    // answer on an older kernel and the difference between it and `FullyEnforced` is exactly the
    // difference `req/493` §1 AC3 forbids collapsing.
    let confinement = gx_confine::apply(&plan).map_err(|e| Error::Usage {
        detail: format!("{e}"),
    })?;

    // 🔴 **Delivered as a note, not by holding the stream.**
    //
    // The first version wrote this object with `Outcome::emit(&mut std::io::stderr().lock(), …)`,
    // and `probes/doubt/tests/declaration_writer_doubt.rs`'s `d6` was red on it: the census there
    // is of **destinations**, exactly three modules in this binary may hold that stream
    // (`emit.rs`, `gx-api`'s `notes.rs`, `gx-mcp-wire`'s `notes.rs`), and this was a fourth. The
    // road that answers for a failed write is `crate::emit::note` — what a note that went nowhere
    // costs is counted once, in `main`'s `settled`, rather than turning `2>/dev/null` and
    // `2>/dev/full` into two different exit statuses for the same run.
    //
    // A note is also the right *kind* of thing. This verb's stdout belongs to the command it is
    // about to become; what it enforced is the sentence `mcp_wiring`'s "gx: connected to …" is —
    // something an operator needs and a pipe does not.
    let object = report(project, &plan, Some(&confinement), &spec.cmd);
    let text = if pretty {
        serde_json::to_string_pretty(&object)
    } else {
        serde_json::to_string(&object)
    }
    .map_err(|why| Error::OutputFailed {
        detail: why.to_string(),
    })?;
    crate::note!("{text}");

    // 🔴 **`req/493` §1 AC-6** — the fact crosses the `exec` here, and only here.
    //
    // Built **after** `apply` answered, so what the new image inherits is what the kernel took
    // rather than what was asked for: `FaceStatus::PartiallyEnforced` and `NotEnforced` are real
    // answers on an older kernel, and a value written before the call would say `1` for all three.
    exec(&spec.cmd, &declaration(&confinement))
}

/// Become the command. Returns only on failure.
///
/// # Errors
/// [`Error::Io`], carrying the `exec` refusal. A `gx confine` whose command does not exist has
/// still applied the ruleset — and has run nothing, which is the safe half of that pair.
#[cfg(unix)]
fn exec(cmd: &[String], confinement: &str) -> Result<Outcome> {
    use std::os::unix::process::CommandExt;
    // 🔴 `Command::env` and not `std::env::set_var`: the latter is `unsafe` (a data race against any
    // other thread reading the environment) and `crates/gx-canon/tests/unsafe_forbidden.rs` refuses
    // an `unsafe` block in this workspace. `CommandExt::exec` applies the whole `Command`
    // configuration — the environment included — to the image it replaces this process with, so
    // this reaches the confined command and every descendant of it by the ordinary road.
    let err = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .env(CONFINEMENT_ENV, confinement)
        .exec();
    Err(Error::Io {
        action: "exec",
        path: cmd[0].clone(),
        source: err,
    })
}

/// The non-unix arm. Unreachable in practice — [`gx_confine::apply`] has already refused on any
/// platform that reaches here — and present so that the crate builds where the tests do not run.
///
/// # Errors
/// Always [`Error::Usage`].
#[cfg(not(unix))]
fn exec(_cmd: &[String], _confinement: &str) -> Result<Outcome> {
    Err(Error::Usage {
        detail: "`gx confine` runs its command by replacing this process, which needs a unix \
                 `exec`. This binary was not built for one (`req/493` §2)"
            .to_string(),
    })
}
