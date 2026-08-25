// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx wrap` — the proxy an agent connects to, and the one road from a `tools/call` to a commit.
//!
//! Spec: `req/119` §2.4 (the intercept pipeline), §2.5 (registration), §5 (the E2E this verb is the
//! middle of). Ruling: `req/38` §71 ① (the wire is its own crate), ③ (a Deny comes back as a tool (sem: SEM-gx-cli-086)
//! result), and `req/114` §1's P5 row is the **UX** of this verb rather than a second implementation
//! of it.
//!
//! # 🔴 Rule 2 in a proxy, and why this module is thin (sem: SEM-gx-cli-087)
//!
//! `gx-mcp-wire` frames and classifies; `gx-engine` decides and applies; this module is the seam and
//! nothing else. In particular it drives [`crate::pipeline`]'s four functions rather than the engine
//! directly, so that "what a `tools/call` does" and "what `gx submit … && gx plan && gx verify &&
//! gx commit` does" are the **same** four calls in the same order (sem: SEM-gx-cli-088). A second spelling here would be a
//! second pipeline waiting to disagree with the one 44 documents.
//!
//! # What the agent is told
//!
//! * admitted → the server's own tool result, with `_meta` carrying the transformation id, the
//!   verdict and where the receipt was filed;
//! * denied / escalated / failed → ruling ③'s shape (sem: SEM-gx-cli-089): a tool result with `isError: true` whose text is
//!   the reason. An agent that reads it can choose another move; an agent that does not still made
//!   no effect, because nothing reached the server.
//!
//! # 🔴 stdout belongs to MCP
//!
//! The transport specification is explicit — "The server **MUST NOT** write anything to its stdout
//! that is not a valid MCP message" — so this verb's own reporting goes to **stderr**, which is the
//! one place 44 §1.3's "a command that returns a single object puts a single newline-terminated
//! JSON on stdout" has to yield. (sem: SEM-gx-cli-090)
//! The exception is stated here rather than discovered by whoever debugs the first garbled session.

use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use gx_adapter_mcp::{Catalogue, ToolIntent};
use gx_core::{Actor, ChangeContext, EnforcementMode, SubstrateKind};
use gx_mcp_wire::{EffectGate, GateOutcome, GatedCall, Proxy, StdioClient, WireTransport};
use serde_json::{json, Value};

use crate::exit::{Outcome, DENIED, ESCALATED, OK};
use crate::session::{McpWiring, Session};
use crate::{clock, rng, Error, Result};

/// What `gx wrap` was given.
pub struct WrapSpec {
    /// The server command, and its arguments, as they stood after `--`.
    pub command: String,
    /// The server's arguments.
    pub args: Vec<String>,
    /// `--server-env NAME=VALUE`, the `env` member an agent's configuration carries for this server.
    pub env: Vec<String>,
    /// The endpoint half of every locator this session mints. Defaults to
    /// [`gx_mcp_wire::stdio_endpoint`] of the command.
    pub endpoint: Option<String>,
    /// Which argument of a `tools/call` names the resource the change is **about**.
    pub resource_arg: String,
    /// `--restore <TOOL>=<RESTORE_TOOL>`, the declaration only the server's operator can make.
    pub restores: Vec<String>,
    /// `--restore-catalogue <FILE>`: the same declaration as a JSON file, template form included
    /// (A2, req/38 §92 ruling 1; sem: SEM-gx-cli-091). Entries from `restores` apply on top of it.
    pub restore_catalogue: Option<std::path::PathBuf>,
    /// The agent's key id (42 §3.2).
    pub actor_key: String,
    /// The agent's model. 42 §3.2 requires it for an `Agent` and this verb's actor is always one.
    pub actor_model: String,
    /// 42 §3.2's `ChangeContext`.
    pub context: String,
    /// Decide with this pack instead of the shipped set (**E-M6-12**'s flag, one verb along).
    pub policy: Option<std::path::PathBuf>,
    /// DR-2's record-only, for every call of this session.
    pub record_only: bool,
    /// 🔴 **P3.1** — `--fail-posture <closed|open>` (ASM-13, DR-2's other axis). `None` behaves as
    /// `closed`, the same default `gx serve` has (`req/38` §74 item③, gotcha66).
    pub fail_posture: Option<String>,
    /// `--otel-file <PATH>`: where NFR-017's spans go. `None` is **off**, which is the default.
    pub otel_file: Option<String>,
    /// `--otel-locators`: carry raw locators in spans. Off, because a resource URI names a
    /// customer's file (`crate::otel::NOT_EXPORTED`).
    pub otel_locators: bool,
    /// 🔴 **DR-V4B-3** (`req/38` §123 ruling 3, `req/189`): `--forward-resource-arg` — send the
    /// `--resource-arg` member to the server even when it is a `gx_*`-named member. Off by
    /// default; see [`stripped_member`] for the rule the default applies.
    pub forward_resource_arg: bool,
}

/// 🔴 **DR-V4B-3**: which argument member, if any, this session removes from the wire form of every
/// `tools/call` before it reaches the server (`gx_mcp_wire::WireTransport::stripping`).
///
/// The rule: the `--resource-arg` member is stripped **when its name begins with `gx_`** and
/// `--forward-resource-arg` was not given. `gx_resource_uri` (every `tools/a*_e2e.sh`) is gx's own
/// protocol — the position this proxy reads — and not an argument the tool declares; a
/// strict-body server (openapi-mcp-server / Notion, `req/183` §6.2 R7: `400 body.gx_resource_uri
/// should be not present`) refuses the whole call over it, after the gate has admitted it. A
/// `--resource-arg` that names a real argument of the tool (`uri` — the default — for the notes
/// servers, `path` for a file tool) is **never** stripped: taking it off would break the call the
/// other way. Nothing the agent sent beyond that one member is ever touched (44's "carry the
/// agent's arguments as sent"). The rule is one function so a test can hold it without a server.
#[must_use]
pub fn stripped_member(resource_arg: &str, forward_resource_arg: bool) -> Option<String> {
    if forward_resource_arg || !resource_arg.starts_with("gx_") {
        return None;
    }
    Some(resource_arg.to_string())
}

/// 🔴 The engine, behind the wire's seam.
///
/// It holds the [`Session`] the four pipeline functions take and the endpoint every locator is
/// built from. What it does **not** hold is any judgement: every branch below reads an exit code the
/// pipeline returned (44 §1.4's table), which is the same reading `main` does.
struct EngineGate<'a> {
    session: &'a mut Session,
    client: Arc<StdioClient>,
    endpoint: String,
    resource_arg: String,
    actor: Actor,
    context: ChangeContext,
    mode: Option<EnforcementMode>,
    otel: crate::otel::Exporter,
    /// 🔴 **R19 / `req/279` M-07** — the `--mcp-*` flags a single-shot `gx undo` needs to reach
    /// **this** session's server with **this** session's restore declaration, already spelled.
    undo_flags: String,
    /// The `env` names this session started its server with, if any. They travel beside the remedy
    /// and never inside it: this module's `environment` doc fixes that an `env` member is where a
    /// server's credential lives and that a proxy which printed one would put it in the operator's
    /// terminal and in whatever collects it.
    server_env_names: Vec<String>,
}

/// 🔴 **`req/312` H-01(b) (R23)** — the clause every outcome of `gx wrap` uses to say what this
/// proxy put on the wire, when it did not put the **effect** there.
///
/// # The sentence that was false, and what made it false
///
/// Four outcomes of one call ended in the words *"nothing was sent to the server"*. The
/// twenty-second adversarial audit drove a road where gx **refused before a verdict** — zero
/// receipts, the agent's own `notes.write` never framed, the server's own arrival log confirming
/// it — and that same log showed **two** `tools/call` frames gx had sent itself, as compare-and-set
/// reads, before it stopped. The document was empty afterwards. The agent was handed *"nothing was
/// sent to the server"*. `req/38` §225 ruling 1 called that the deciding fact of the finding: a
/// declaration `docs/LIMITS.md` can call a burden the deployment carries is one thing, and a
/// **runtime claim that is false** is another, which no section of that page permits.
///
/// # What the number is, and what it is not
///
/// It is the wire's `reads` counter across this call: every `resources/read` and every `tools/call`
/// gx sent **as a read**. The escrow's `read_by` road (DR-46-9 A-3) and DR-46-16's `$cas_read` road
/// both increment it, which is why counting `reads` rather than either road on its own is the
/// honest total. It is **not** `calls`, which is AC-051's number and counts effects — if an effect
/// had been sent, none of these sentences would be printed.
///
/// # Why a count and not a boolean
///
/// A read is a frame on the wire whatever gx's own vocabulary calls it. `gx-adapter-mcp` now
/// refuses the two shapes of "a read face that writes" that a catalogue file states about itself
/// (`req/312` H-01), and `docs/LIMITS.md` still names the third — a writing tool this file says
/// nothing about — as a burden the deployment carries. The number is the thing that lets an agent
/// notice that burden landing: a refusal that says two reads were sent is a refusal an agent can
/// check against the object.
fn what_reached_the_server(reads: u64) -> String {
    match reads {
        0 => "**nothing was sent to the server**".to_string(),
        1 => {
            "**the effect was not sent**, and 1 read was sent to the server to establish what the \
              object holds"
                .to_string()
        }
        n => format!(
            "**the effect was not sent**, and {n} reads were sent to the server to establish what \
             the object holds"
        ),
    }
}

/// 🔴 **`req/316` L-03, raised to M by `req/38` §227 ruling 4 (R24)** — what an `ApplyFailed`
/// abort does **not** say on its own, said.
///
/// # The disagreement between the record and the world
///
/// 43 T-10c is "`adapter.apply` failed; roll back from the escrowed inverse on a best-effort basis,
/// and journal `Aborted{ApplyFailed}` whatever that attempt did". Best-effort means the attempt can
/// fail too, and the twenty-third audit drove exactly that: the forward call **arrived**, the
/// compensating restore **arrived and was refused**, the object was left holding the effect, the
/// journal held two `ApplyStarted` records, and the ledger said `Aborted{ApplyFailed}` — a phrase
/// whose plain reading is that the change did not land. No transformation committed, so there is no
/// `gx undo` road out of it either.
///
/// # Why this is a sentence and not a new ledger state
///
/// The record `Aborted{ApplyFailed}` is `gx-engine`'s and 43 §3's, and neither is this crate's to
/// change; a second opinion about a transformation's state, written by a proxy, is worse than the
/// gap. What this crate owns is **the sentence the agent is handed**, and the material is already
/// in the object it is handed with: `detail` is `Engine::rollback`, the compensation's own outcome.
/// So the sentence names the three things a reader has to know and points at the two places that
/// hold them, and `docs/LIMITS.md` carries the standing version of the same paragraph. `req/38`
/// §227 ruling 4 asked for the cheaper of the two designs with the material for both reported;
/// this is the cheaper one.
///
/// # 🔴 **`req/320` H-01 and L-03 (`req/38` §229 ruling 1)** — the clause is a function of `detail`
///
/// R24 wrote *"the compensating inverse **was attempted** on a best-effort basis whose own outcome
/// is the `detail` above"* and appended it to every `ApplyFailed`. `Engine::rollback` has **three**
/// values, and `gx-engine`'s own `store.rs` documents that the third one — `NotAttempted` — is
/// reachable in v0.1 by two paths downstream of `Committing`, the first of which is *an escrowed
/// inverse still `Pending`, whose do-time member an apply failure leaves nothing to resolve from*.
/// The twenty-fourth audit drove that road on a real server: the forward call arrived, the object
/// took the agent's bytes, T-10c's guard did not open, **no compensating call was ever framed** —
/// the server's own arrival log held one line — and gx told the agent the compensation *was*
/// attempted. `req/38` §225 ruling 1's deciding fact is a runtime claim that the server's own
/// record contradicts, and this was that with the sign reversed.
///
/// The second half is L-03, and it is the same discipline one word over. `Rollback::Failed` is
/// `apply_once(inverse)` returning `Err`, and this adapter's `apply` is the call **and** the
/// read-back of it — so a compensation whose bytes landed and whose post-read died is `Failed`,
/// with the object back where it started. The audit measured exactly that. `Failed` therefore may
/// not be handed over as *the compensation did not work*; what it licenses is *the compensating
/// call was sent and its own apply came back an error*, which is what this arm says.
///
/// # What is deliberately the same on all three roads
///
/// The two facts that do not depend on `detail`: the tool call **was** sent, and no transformation
/// committed, so there is no `gx undo`. Those stay in one place rather than being repeated per arm,
/// because a lane editing one arm must not be able to move them for one road only.
/// 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — the half of the *not attempted* sentence that
/// is a function of the **cause** rather than of the value.
///
/// One arm per word `gx_engine::NotAttemptedBecause` declares, and one for a word this build does
/// not know. The last one is not decoration: `ALL_CAUSES` is the engine's, and a cause added there
/// would otherwise fall into whichever arm was written last — which is the failure this whole
/// repair is about, one level down from the failure R25 repaired.
///
/// The absent case is the same arm as the unknown one, and it is reached honestly: the cause is not
/// a component of Σ, so a row read back after a restart has none to give.
fn not_attempted_cause_clause(commit: &Value) -> &'static str {
    match commit.get("not_attempted_because").and_then(Value::as_str) {
        Some("EscrowStillPartial") => {
            ": 43 T-10c's guard asks for an inverse this build can execute, and the escrowed one \
             was still partial — a member of it is filled from what the call answers, and the \
             failed apply left no answer to fill it from — so nothing was attempted and nothing \
             was taken back"
        }
        Some("NoInverseWasEscrowed") => {
            ": **no inverse was escrowed at all** for this call, so there was never a compensation \
             to attempt. This is not the partial-escrow road: the adapter answered that it could \
             build no inverse for this transformation, a person approved it anyway, and the \
             consequence of that approval is that a failed apply leaves nothing to undo with"
        }
        Some("RecoveredWithoutRebuilding") => {
            ": this row was closed by **`gx repair`**, not by the call itself. The apply had been \
             announced in the journal and nothing was rebuilt in that run, so no inverse existed \
             in the recovering process to attempt. What the server holds is a question `gx repair`'s \
             report and `gx log` answer together"
        }
        // 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2)** — the two roads on which the
        // compensation was **refused rather than skipped**. The distinction matters to whoever
        // reads this: on the three arms above there was nothing to send, and on these two there
        // was something to send and this engine declined to send it.
        Some("WorldNeverMoved") => {
            ": the object was **read the instant the apply failed** and it was exactly where this \
             transformation found it — the call moved nothing — so there was no effect to \
             compensate and no inverse was sent. This is not a compensation that failed; it is a \
             compensation that had no work to do. The escrowed inverses this build mints restore \
             from any world, so sending one here would have written over whatever the object holds \
             now without taking anything back (`req/372` M-01)"
        }
        Some("WorldMovedBeneath") => {
            ": the apply **did** move the object, and when the object was read again immediately \
             before the compensation it had moved **once more** — so somebody else wrote in \
             between and the escrowed inverse was **not** sent. The inverses this build escrows \
             are absolute: they restore from any world, so sending one here would have overwritten \
             that writer's work and reported success over it (`req/372` M-01 measured exactly that \
             on a branch). What this does **not** say is *who* wrote — your own writer, another \
             agent and a CI job are one observation here. The object still holds whatever this \
             call left plus whatever they wrote; read it before writing to it again"
        }
        Some("WorldCouldNotBeRead") => {
            ": the read that decides whether the compensation may run **could not be taken** — the \
             substrate would not answer, or the two fingerprints were not comparable — so the \
             escrowed inverse was **not** sent. This is not \"the object had moved\": it is that \
             nobody looked, and an absolute inverse is not sent into a substrate that will not say \
             where it is. The change may stand; read the object and `gx log`"
        }
        other => {
            // The cause is absent after a restart and unknown if the engine grew a word this
            // build has not been taught. Both are the same honest answer: say what the value
            // carries and stop.
            let _ = other;
            ". Why it was not attempted is not a fact this answer carries — the engine reaches \
             this outcome by more than one road and this build will not guess which: read \
             `detail`, the `gx repair` report and `gx log`"
        }
    }
}

pub fn apply_failed_clause(commit: &Value) -> String {
    if commit.get("reason").and_then(Value::as_str) != Some("ApplyFailed") {
        return String::new();
    }
    // 🔴 One arm per value of `Engine::rollback`, plus the arm for a word this build does not know.
    // The last one is not decoration: `Rollback::ALL_KINDS` is `gx-engine`'s and a value added
    // there would otherwise fall into whichever arm was written last, which is the failure this
    // whole repair is about.
    let compensation = match commit.get("detail").and_then(Value::as_str) {
        // 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — one arm per **cause**, inside the arm
        // per value.
        //
        // R25's arm named the value and then asserted a cause: *the escrowed one was still partial*.
        // 🔴 **R30 / `req/372` M-01** — five places since this lane, not three, and the two new
        // ones are the reason the shape had to be an arm-per-cause rather than a longer sentence:
        // they are refusals, and a sentence tuned to the three "there was nothing to send" roads
        // would have been false on both of them.
        // The engine constructs this value at three places with three different facts, so a
        // sentence keyed on the value alone is right about at most one of them — and on the other
        // two it hands the agent a confident account of an escrow row that does not exist. The
        // cause now travels beside the value (`gx_engine::NotAttemptedBecause`), and the shape
        // here is the shape R25 built one level up: an arm for each word the engine declares, and
        // an arm for a word this build does not know.
        //
        // 🔴 The two facts that do not depend on the cause stay in the shared tail below, and the
        // half that does not depend on it stays here: **no compensating inverse was sent** is true
        // on all three roads and is what the value itself carries.
        Some("NotAttempted") => {
            return format!(
                " 43 T-10c: the tool call was sent, the change may stand on the server, and **no \
                 compensating inverse was sent**{}. No transformation committed, so there is no \
                 `gx undo` for this one; the object's state is what the server says it is, and \
                 `gx log` carries the journal records this call wrote.",
                not_attempted_cause_clause(commit)
            );
        }
        Some("Succeeded") => {
            "the compensating inverse was sent, on 43 T-10c's best-effort basis, the adapter \
             accepted it, **and the object was read again afterwards and found at the fingerprint \
             it started from** — so the change was taken back. The read is one read taken after \
             the call, not a lock: `docs/LIMITS.md` says what a writer inside that window can \
             still do to this answer"
        }
        // 🔴 **R29 / `req/361` H-01** — the third world. Until this lane it was printed as the
        // first: the twenty-eighth audit drove an adapter whose apply fails halfway, watched the
        // escrowed inverse run to completion **honestly**, watched the object come to rest further
        // from home than it started (`A B C D` → `A B D` → `A B D C D`), and read `Succeeded` on
        // the wire over it. The compensation running is not the compensation working.
        Some("Diverged") => {
            "the compensating inverse was sent, on 43 T-10c's best-effort basis, and the adapter \
             accepted it — **but the object was read again afterwards and it is not back where it \
             started**. This is the answer `Succeeded` used to give for this world. Do not treat \
             the change as taken back: the compensation ran, and the object is somewhere neither \
             the original nor the restored state. What this word does **not** say is who moved it \
             — a writer of your own inside the window lands here too — so read the object before \
             writing to it again, and `gx log` for the records this call wrote"
        }
        Some("Failed") => {
            "the compensating inverse was sent, on 43 T-10c's best-effort basis, and its own apply \
             came back an error. That is **not** the same as \"the object still holds the change\": \
             this adapter's apply is the call together with the read-back of it, so the error can \
             be either one, and a compensation whose bytes landed and whose read-back died lands \
             here too"
        }
        // 🔴 **`req/329` M-01 (`req/38` §233 ruling 2)** — *this answer carries no account* is not
        // *the engine grew a word this build has not been taught*, and one arm for both told an
        // agent something false.
        //
        // Folded together, an object with no `detail` member reached the unknown-word arm and the
        // proxy said it "does not recognise the word it is carrying (None)" — false about a build
        // that knows all three of `Rollback::ALL_KINDS` — and then sent the reader to `detail`, a
        // member that object does not have. Both halves were wrong at once, and the remedy named
        // the very thing that was missing.
        //
        // Splitting them is what makes the repair a property of this function rather than of the
        // road that fed it: `terminal_answer` no longer produces such an object, but a policy pack,
        // an HTTP route or a third party reading `gx log` still can, and the sentence they compose
        // is now true. `crates/gx-cli/tests/r27_reentrant_abort.rs` drives both halves.
        None => {
            return " 43 T-10c: the tool call was sent and the change may stand on the server. \
                    What the roll-back did is not a fact **this answer** carries — no roll-back \
                    word is on this object at all, which is a statement about the answer and not \
                    about the engine, whose `Aborted` record holds the account: read `gx log` for \
                    this transformation, and the `gx repair` report if a recovery ran. No \
                    transformation committed, so there is no `gx undo` for this one; the object's \
                    state is what the server says it is."
                .to_string();
        }
        other => {
            return format!(
                " 43 T-10c: the tool call was sent and the change may stand on the server. What \
                 the roll-back did is the `detail` above, and this build of the proxy does not \
                 recognise the word it is carrying ({other:?}), so it will not put words in its \
                 mouth: read `detail` and `gx log`. No transformation committed, so there is no \
                 `gx undo` for this one; the object's state is what the server says it is."
            );
        }
    };
    format!(
        " 43 T-10c: the tool call was sent, the change may stand on the server, and {compensation}. \
         No transformation committed, so there is no `gx undo` for this one; the object's state is \
         what the server says it is, and `gx log` carries the journal records this call wrote."
    )
}

/// 🔴 **`req/320` L-02 (`req/38` §229 ruling 2)** — what `--record-only` does **not** reach, said
/// on the road where a reader would otherwise have to measure it.
///
/// The twenty-fourth audit ran E-M3-4's escalation with and without the flag and got the same three
/// numbers both times: the same verdict, zero arrivals, the object unmoved. That is the implementation
/// following 43 §4 to the letter — §4 is written about a `Denied` transformation and T-8r is the
/// transition it opens — but nothing a reader can see says so. The flag's name is *record only*, and
/// an operator who turns it on to watch without stopping anything has no way to learn from `gx wrap`
/// that one of the three verdicts still stops.
///
/// # Why the answer is a sentence and not a wider mode
///
/// Carrying an escalation through would send an effect that **no policy admitted and no person
/// ruled on**. Record-only's whole value is that the thing it lets through was refused *by a
/// decision somebody can read afterwards* (43 §4's `enforced=false` is that reading); an escalation
/// is the absence of a decision, so the same mechanism over it would record nothing worth reading
/// and would make the escalation road quieter than the deny road. The asymmetry is deliberate and
/// this is where it is written down.
fn record_only_does_not_reach_an_escalation(record_only: bool) -> &'static str {
    if !record_only {
        return "";
    }
    " `--record-only` does not reach this road and this call was stopped: 43 §4's mode is written \
     about a `Denied` transformation and T-8r is the transition it opens. An escalation is not a \
     refusal a person has made — it is a question nobody has answered yet — and carrying it \
     through would send an effect no policy admitted and no person ruled on, which is not the \
     thing the mode exists to record."
}

impl EngineGate<'_> {
    /// 🔴 **R19 / `req/279` M-07** — the undo an operator can run, with no word left for them to
    /// invent (`req/263` §6 G6).
    fn undo_remedy(&self, tid: &str) -> String {
        format!("gx undo {tid} {}", self.undo_flags)
    }

    /// 🔴 **`req/312` H-01(b)** — [`what_reached_the_server`] for **this** call alone.
    ///
    /// `before` is the wire's read counter as it stood at the top of [`Self::run`]; the difference
    /// is the reads this call caused, which is the only number a sentence about this call may
    /// quote. A cumulative count would make the second call in a session claim the first one's
    /// reads.
    fn sent_clause(&self, before: u64) -> String {
        what_reached_the_server(self.client.reads().saturating_sub(before))
    }

    /// escrow → gate → forward → observe → receipt, for one call (`req/119` §2.4).
    fn run(&mut self, call: &GatedCall<'_>) -> Result<GateOutcome> {
        // 🔴 **`req/312` H-01(b)** — the wire's read counter before this call touched it. Every
        // sentence below that tells the agent what reached the server is a function of the
        // difference between this and the counter when the sentence is written.
        let reads_before = self.client.reads();
        let Some(resource) = call
            .arguments
            .get(&self.resource_arg)
            .and_then(Value::as_str)
        else {
            return Ok(GateOutcome::Failed {
                reason: format!(
                    "gx: this call names no resource. `gx wrap --resource-arg {}` is the argument \
                     this proxy reads a position from, and a tool call with none is a change gx \
                     cannot describe: the CAS has to read an object before and after, and there is \
                     no object here. On this road, {}.",
                    self.resource_arg,
                    self.sent_clause(reads_before)
                ),
                meta: json!({ "gx/verdict": "not-planned", "gx/resource_arg": self.resource_arg }),
            });
        };
        let locator = format!("{}#{resource}", self.endpoint);
        let goal = ToolIntent::new(
            call.tool,
            serde_json::to_vec(call.arguments).map_err(|e| Error::Usage {
                detail: format!("the arguments of {:?} have no JSON form: {e}", call.tool),
            })?,
        )
        .encode()
        .map_err(|e| Error::Usage {
            detail: format!("the goal of this call has no canonical form: {e}"),
        })?;

        let spec = crate::pipeline::SubmitSpec {
            substrate: SubstrateKind::Mcp,
            locator: locator.clone(),
            goal,
            context: self.context.clone(),
            actor: self.actor.clone(),
            order: 0,
            parents: Vec::new(),
        };
        // 🔴 The writer lock, for the length of this tool call and no longer (`req/38` §148).
        //
        // Everything below — submit, plan, verify, commit, and the adapter's apply inside it — is
        // one operation as far as another `gx` process is concerned, and it is exactly the span that
        // must not interleave with a `gx serve` commit. The catch-up inside it is why this is not
        // merely an exclusion: minutes can pass between two tool calls, and whatever the GUI's
        // server committed in them is read into this session before it writes.
        //
        // The lock is released at the end of `gate`, including on every early return: `Session`
        // holds it in a value whose `Drop` unlocks, and `release_writer_lock` drops it.
        self.session.hold_writer_lock()?;
        let started = clock::now();
        let submitted = crate::pipeline::submit(self.session, &spec, rng::seed(), clock::now())?;
        // 🔴 NFR-017's spans, emitted where the surface drives a transition (`crate::otel` says what
        // that is less than). The exporter is off unless an invocation named a sink, and it cannot
        // fail upward — AC-P3-15's second half is a property of `span`'s signature.
        self.otel.span(
            "submit",
            started.0,
            clock::now().0,
            &json!({
                "substrate": "mcp",
                "tool": call.tool,
                "locator": self.otel.locator_attribute(&locator),
            }),
        );
        let intent_text = submitted.json["intent_id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let intent_id =
            gx_core::IntentId(
                gx_core::Cid::from_text(&intent_text).map_err(|e| Error::Usage {
                    detail: format!("the engine's own intent id did not parse: {e}"),
                })?,
            );
        let planning = clock::now();
        crate::pipeline::plan(self.session, &intent_text, clock::now())?;
        self.otel.span(
            "plan",
            planning.0,
            clock::now().0,
            &json!({ "substrate": "mcp", "intent": intent_text }),
        );
        let Some(transformation) = self.session.read().resolved(&intent_id) else {
            return Err(Error::NotFound {
                what: "transformation",
                id: intent_text,
            });
        };
        let tid = transformation.0.to_text();

        let verifying = clock::now();
        let verified =
            crate::pipeline::verify(self.session, &transformation, self.mode, clock::now())?;
        let verdict = verified.json["kind"].clone();
        // 🔴 **`req/320` M-02 (`req/38` §229 ruling 2)** — the same word, kept past the move into
        // `meta`. Two sentences below the commit need to know **which verdict** this call carried
        // into T-8r, and reading it back out of a `Value` that has been moved is how the abort road
        // came to say "gx admitted this call" about a `Deny`.
        let verdict_kind = verified.json["kind"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        self.otel.span(
            "verify",
            verifying.0,
            clock::now().0,
            &json!({
                "substrate": "mcp",
                "transformation": tid,
                "verdict_kind": verdict,
                "state": verified.json["state"].clone(),
                "enforced": verified.json["enforced"].clone(),
                "fail_posture_engaged": verified.json["fail_posture_engaged"].clone(),
                "adapter_version": crate::session::ADAPTER_VERSION,
            }),
        );
        let meta = json!({
            "gx/transformation": tid,
            "gx/locator": locator,
            "gx/verdict": verdict,
            "gx/enforced": verified.json["enforced"].clone(),
            "gx/receipt": verified.json["receipt_stored_at"].clone(),
        });
        // 🔴 **`req/316` M-03 (R24)** — whether this session records rather than enforces.
        //
        // 43 §4 is the primary text and it is unambiguous: under `EnforcementMode::RecordOnly`,
        // *"from `Denied` as well it goes on, by way of T-8r, to `Canonicalized → Committing →
        // Committed`; but the receipt must always carve `enforced=false` into it"* (quoted in
        // SEM-gx-cli-2308). The point of the mode is the sentence after
        // that one — "the effect went through and policy had refused it" is a fact a third party
        // can check — and `Deny` is the only verdict on which the flag means anything at all.
        //
        // Until this window `gx wrap --record-only` printed `"record_only":true` in its start-up
        // line, passed the mode to `verify` and `commit`, and then returned three lines below,
        // before either could act on it. Measured (`req/316` M-03): two runs differing only in the
        // flag produced the same verdict, the same `gx/enforced: true`, zero arrivals, and an
        // unchanged object. The flag was inert on the one road where it has a meaning.
        //
        // `M6H3-9` — filed as "a record-only E2E needs a policy pack over a writable path, and
        // `gx verify` has no `--policy`" — is **false for this surface**: `gx wrap` has `--policy`,
        // and `crates/gx-cli/tests/support` already ships a deny pack over a writable path. That is
        // recorded in `docs/LIMITS.md` rather than left standing.
        let record_only = matches!(self.mode, Some(EnforcementMode::RecordOnly));
        match verified.code {
            DENIED if !record_only => {
                return Ok(GateOutcome::Denied {
                    reason: format!(
                        "gx denied this call and {}. \
                         transformation {tid}, position {locator}. reasons: {}. The receipt for the \
                         refusal is at {}; `gx receipt show {tid}` reads it.",
                        self.sent_clause(reads_before),
                        verified.json["reasons"],
                        verified.json["receipt_stored_at"]
                    ),
                    meta,
                })
            }
            // T-8r's road. The verdict stays `Deny` in every record this call writes — the mode
            // changes what happens next, not what the gate said — and the receipt carries
            // `enforced=false`, which is the whole of what a reader is being offered here.
            DENIED => {}
            ESCALATED => {
                return Ok(GateOutcome::Escalated {
                    reason: format!(
                        "gx escalated this call to a person and {}. \
                         transformation {tid}, ticket {}. `gx escalation approve|reject` is the \
                         ruling, and the call has to be made again afterwards: v0.1 does not hold a \
                         tool call open while a human decides (req/119 §2.4).{}",
                        self.sent_clause(reads_before),
                        verified.json["ticket"],
                        record_only_does_not_reach_an_escalation(record_only)
                    ),
                    meta,
                })
            }
            OK => {}
            _ => {
                return Ok(GateOutcome::Failed {
                    reason: format!(
                        "gx could not reach a verdict for this call: {}. On this road, {}.",
                        verified.json,
                        self.sent_clause(reads_before)
                    ),
                    meta,
                })
            }
        }

        let committing = clock::now();
        let committed =
            crate::pipeline::commit(self.session, &transformation, None, self.mode, clock::now())?;
        // 🔴 One span, not two. 43 puts `canonicalize` (T-8) and `commit` (T-9..T-11) in one call of
        // `Engine::commit`, and this exporter sees calls rather than transitions — so a
        // `canonicalize` span would be a boundary this crate invented. `crate::otel`'s table names
        // the gap rather than filling it with a guess.
        self.otel.span(
            "commit",
            committing.0,
            clock::now().0,
            &json!({
                "substrate": "mcp",
                "transformation": tid,
                "state": committed.json["state"].clone(),
                "enforced": committed.json["enforced"].clone(),
                "adapter_version": crate::session::ADAPTER_VERSION,
            }),
        );
        let mut meta = meta;
        if let Value::Object(map) = &mut meta {
            // 🔴 **`req/316` M-03 (R24)** — `enforced` as the **commit** left it.
            //
            // The value copied into `meta` above is `verify`'s, and T-8r is what sets the flag, so
            // on a record-only Deny the verify-time value is the one that has not heard about the
            // mode yet. A reader of `gx/enforced` is asking about the transformation that was
            // committed, and this is the answer to that question.
            // 🔴 **`req/320` M-02 (`req/38` §229 ruling 2)** — and the branch where the commit
            // **aborted**, which R24's repair did not reach.
            //
            // An aborted transformation writes no `enforced` member at all: 43 §3's `Aborted` row
            // has three fields and this is not one of them, and `gx-engine` is not this lane's to
            // change. So the `if` above fell through and `meta` kept `verify`'s value — the one
            // this crate's own comment calls *the value that has not heard about the mode yet* —
            // and the twenty-fourth audit measured `gx/enforced: true` on a call that record-only
            // had carried past a `Deny` and whose apply then failed. That is the flag 43 §4 says
            // must always be `false` here, reading `true`.
            //
            // What this build knows without inventing a state: `record_only` is this session's own
            // flag, and `Deny` is the verdict this call carried into the commit. A commit was
            // reached from a `Deny` **only** through T-8r, and T-8r is exactly the transition the
            // mode opens — so on this road the answer to *was policy enforced against this call*
            // is `false`, whatever the apply then did. Every other road is left alone: an abort on
            // the admit road keeps `verify`'s value, which is that road's own answer.
            if let Some(enforced) = committed.json.get("enforced") {
                map.insert("gx/enforced".to_string(), enforced.clone());
            } else if record_only && verdict_kind == "Deny" {
                map.insert("gx/enforced".to_string(), json!(false));
            }
            map.insert("gx/commit".to_string(), committed.json.clone());
            // 🔴 **R19 / `req/279` M-07** — a remedy a reader can run **without adding a word**
            // (`req/263` §6 G6).
            //
            // What stood here was `gx undo {tid} --mcp-server <the same server>`, and audit 19 ran
            // it as printed: `--mcp-restore-catalogue` was missing, so `gx undo` opened with an
            // empty catalogue (`0 restorable tool(s) declared`), could build no inverse for the
            // undo's own call, and E-M3-4 escalated it — `rc=4`, `state=Escalated`, object
            // unchanged. Adding the catalogue *afterwards* then met `is Escalated, and 43 §3 has no
            // "undo" from there`, and the road out of that is `gx escalation approve`, which was
            // H-02. One placeholder in a printed sentence closed the whole exit.
            //
            // The proxy knows every word: the command it started, its arguments, the endpoint it
            // mints locators from, and the restore declaration it was given. So the sentence is
            // built from those and not from a placeholder ([`undo_remedy`]).
            map.insert("gx/undo".to_string(), json!(self.undo_remedy(&tid)));
            // The one thing the sentence above cannot carry, said beside it rather than left out.
            if !self.server_env_names.is_empty() {
                map.insert(
                    "gx/undo_server_env".to_string(),
                    json!({
                        "names": self.server_env_names.clone(),
                        "note": "this session started its server with these `env` members \
                                 (`gx wrap --server-env`). Their **values** are the deployment's \
                                 and are never printed, so the undo above needs them in its own \
                                 environment or repeated as `--mcp-server-env NAME=VALUE`",
                    }),
                );
            }
        }
        if committed.code != OK {
            // 🔴 **`req/320` M-02** — the opening phrase is a function of the verdict, not a
            // constant. A `Deny` that `--record-only` carried through T-8r reached this line, and
            // the sentence handed to the agent said *"gx admitted this call"* while `gx/verdict`
            // in the same answer said `Deny`. The gate spoke; what the mode changed is what
            // happened after it spoke, and the sentence now says which of those two it is.
            let opening = if record_only && verdict_kind == "Deny" {
                "gx denied this call, `--record-only` carried it through 43 T-8r anyway, and the \
                 commit did not complete"
            } else {
                "gx admitted this call and the commit did not complete"
            };
            return Ok(GateOutcome::Failed {
                reason: format!(
                    "{opening}: {}.{} Whether the tool ran is answered by the receipt and by the \
                     server, not by this sentence.",
                    committed.json,
                    apply_failed_clause(&committed.json),
                ),
                meta,
            });
        }
        // The server's own answer, kept by the transport when `apply` made the call. `None` is the
        // record-only case (DR-2: the apply still happens) and the belt-and-braces case of a
        // transport that made no call, and it is reported as what it is rather than as an empty
        // success.
        let result = self.client.take_last_result().unwrap_or_else(|| {
            json!({
                "content": [{
                    "type": "text",
                    "text": "gx admitted and committed this call; the server returned no result this \
                             proxy could relay.",
                }],
            })
        });
        Ok(GateOutcome::Admitted { result, meta })
    }
}

impl EffectGate for EngineGate<'_> {
    fn gated_call(&mut self, call: &GatedCall<'_>) -> GateOutcome {
        // 🔴 **`req/312` H-01(b)** — the same baseline `run` takes, read one frame earlier because
        // the refusal below is written *after* `run` has returned and cannot see its locals. No
        // read happens between these two lines, so the two baselines are the same number; taking it
        // here is what makes that a property of the code rather than of the reader's memory.
        let reads_before = self.client.reads();
        let outcome = self.run(call);
        // 🔴 Released on **both** roads, and that is why it is here rather than at the end of `run`
        // (`req/38` §148). A lock kept after a refused call would be a `gx wrap` that took the
        // project on its first failure and never gave it back — an agent's one unplannable call
        // would lock out the GUI's server for the rest of the session.
        self.session.release_writer_lock();
        match outcome {
            Ok(outcome) => outcome,
            // 🔴 An error of the pipeline is **not** a verdict, and it does not become one here. The
            // agent is told that the call did not happen and why, in the shape ruling ③ fixed (sem: SEM-gx-cli-092), and the
            // proxy stays up: one unplannable call is not a reason to take a session down.
            //
            // 🔴 **R19 / `req/279` M-06** — but "not a verdict" is not "not a refusal", and until
            // this lane the two were spelled the same way.
            //
            // R17 built three refusals out of what a **deployment declared** — DR-46-12's
            // unreadable prior, `req/269` M-05's unsound read declaration, DR-46-15's answer about
            // another object. `docs/LIMITS.md` calls all three a *refusal*; every one of them
            // leaves `gx-adapter-mcp` as an `Err`, and every `Err` arrived here and was told to the
            // agent as `this is not a refusal — it is a change gx could not describe`, with
            // `gx/verdict: "not-reached"` and a session counter reading `failed`. The machine was
            // right the whole time (nothing reaches the server, the object does not move); the
            // **record** was the thing that lied, on the one surface an agent reads. For a product
            // whose claim is that its account of itself can be checked, that is the defect.
            //
            // So the three are separated from the rest here, by the constants themselves
            // ([`declaration_refusal`]), and answered as what they are.
            Err(e) => classify_pipeline_error(&e, self.sent_clause(reads_before)),
        }
    }
}

/// 🔴 **R19 / `req/279` M-06** — is this pipeline error one of the three refusals a **declaration**
/// caused?
///
/// Matched on `gx-adapter-mcp`'s own exported constants rather than on an error variant: all three
/// are `gx_substrate::Error::Unreadable` by the time they reach this crate, so the variant cannot
/// tell them apart, and the constants are the thing the adapter's own suites and `docs/LIMITS.md`
/// already pin verbatim (`crates/gx-adapter-mcp/tests/github16_read_by_tool.rs`). A declaration
/// refusal added upstream without a constant would fall through to `Failed` — which is the safe
/// direction.
///
/// 🔴 **`req/291` M-03** — the list was three long and the adapter exported **five**. R18 added
/// `OBJECT_UNNAMED_REFUSAL` and `IDENTITY_IGNORES_THE_ANSWER_REFUSAL` on the same day R19 wrote
/// this function, and neither lane saw the other's half: the twentieth audit ran five arms that
/// differ only in the read tool's answer and the spelling of `identity`, and two of them came back
/// `gx/verdict: "not-reached"` with the sentence *this is **not** a refusal — it is a change gx
/// could not describe*, and a session counter reading `failed:1`. The mechanism was right in all
/// five (object unmoved, effect never sent); the **record** was wrong in two, which is the defect
/// R19 wrote this function to close.
///
/// So the hedge in the paragraph above is no longer the only thing holding the count.
/// [`DECLARATION_REFUSALS`] is a fixed-length array and
/// `crates/gx-cli/tests/r20_refusal_vocabulary_is_whole.rs` reads `gx-adapter-mcp`'s **source** for
/// its `pub const … _REFUSAL` declarations and holds the two counts equal — so the next lane that
/// exports a sixth constant makes this lane red rather than making an agent read "not a refusal"
/// about a refusal.
#[must_use]
pub fn declaration_refusal(rendered: &str) -> bool {
    DECLARATION_REFUSALS
        .iter()
        .any(|constant| rendered.contains(constant))
}

/// 🔴 **`req/291` M-03** — every sentence `gx-adapter-mcp` refuses a **declaration** with.
///
/// The length is part of the type on purpose: the probe that counts the adapter's exported
/// constants compares against `DECLARATION_REFUSALS.len()`, and an array whose length is a literal
/// is the one shape where "how many are there" cannot be answered by a `Vec` that happens to be
/// short today.
/// 🔴 **`req/303` M-03 (R22)** — the sixth. `gx-adapter-mcp` split the declaration refusal in two
/// so that a fault about a `restored_by` or an `arguments` template stops claiming a read face was
/// never called. The seam this doc comment promised — *"the next lane that exports a sixth constant
/// makes this lane red rather than making an agent read 'not a refusal' about a refusal"* — is what
/// caught it: `r20_refusal_vocabulary_is_whole` went red on the count the moment the constant
/// existed, before any road was run.
/// 🔴 **DR-46-21 (`req/38` §218 ruling 2, §417/§421)** — the seventh. The compare-and-set half now
/// binds a **content-addressed** read's answer to the object it is keyed by (digest re-verification,
/// `gx-adapter-mcp/src/cas.rs`), and its refusal is the escrow road's `OBJECT_IDENTITY_REFUSAL`
/// parity for the CAS road: the read answered about a different object, so gx refused before a
/// verdict. It carries its own sentence rather than the escrow one — a CAS read has no `identity`
/// member to correct and escrows no inverse — and `r20`/`r22` held the count and the census the
/// moment the constant existed.
pub const DECLARATION_REFUSALS: [&str; 7] = [
    gx_adapter_mcp::READ_FAILURE_REFUSAL,
    gx_adapter_mcp::DECLARATION_UNSOUND_REFUSAL,
    gx_adapter_mcp::DECLARATION_WITHOUT_A_READ_FACE_REFUSAL,
    gx_adapter_mcp::OBJECT_IDENTITY_REFUSAL,
    gx_adapter_mcp::OBJECT_UNNAMED_REFUSAL,
    gx_adapter_mcp::IDENTITY_IGNORES_THE_ANSWER_REFUSAL,
    gx_adapter_mcp::CAS_OBJECT_DIGEST_REFUSAL,
];

/// 🔴 **`req/284` §1.3's third word.**
///
/// `Admit`/`Deny`/`Escalate` are 43 T-4's three and this is none of them: no verdict was reached,
/// because gx read the deployment's own declaration and stopped before the gate. `not-reached` is
/// the word for a change gx could not *describe*; this is a change gx **refused**, and an agent
/// choosing its next move needs to know which of the two happened — one says "the wiring is
/// broken", the other says "the catalogue is wrong and nothing was sent".
///
/// 🔴 **`req/303` L-03 (R22)** — the literal moved to `gx-mcp-wire`, where
/// `Stats::refused_before_verdict` reads it, and this is now that one value under its original
/// name. The word was in two places the moment the wire needed to count it: the classifier writes
/// it and the counter reads it, and two spellings of one word is the shape `LEDGER_DISAGREES` and
/// `BUSY_TITLE` were both ruled defects for.
pub const REFUSED_BEFORE_VERDICT: &str = gx_mcp_wire::REFUSED_BEFORE_VERDICT;

/// Which [`GateOutcome`] an error of the pipeline becomes.
fn classify_pipeline_error(e: &Error, sent: String) -> GateOutcome {
    let rendered = e.to_string();
    if declaration_refusal(&rendered) {
        return GateOutcome::Denied {
            reason: format!(
                "gx refused this call and {sent}: {rendered}. No \
                 verdict was reached — gx stopped at the restore catalogue's own declaration, \
                 before the gate — so this is a refusal by gx and not a failure of the server.",
            ),
            meta: json!({ "gx/verdict": REFUSED_BEFORE_VERDICT }),
        };
    }
    GateOutcome::Failed {
        reason: format!(
            "gx could not carry this call through its pipeline: {rendered}. No verdict was reached, \
             so this is not a refusal — it is a change gx could not describe.",
        ),
        meta: json!({ "gx/verdict": "not-reached" }),
    }
}

/// Parse the deployment's restore declaration: an optional JSON catalogue file (template form
/// included -- `gx-adapter-mcp`'s `Catalogue::from_json` is the format's one reader), with
/// `--restore <TOOL>=<RESTORE_TOOL>` entries applied on top of it (later spelling wins, so a flag
/// can override one file entry without rewriting the file).
///
/// # Errors
/// [`Error::Usage`] for a spelling with no `=` (a catalogue entry that named one tool would be a
/// declaration nobody could act on), for a file that cannot be read, and for file bytes that are
/// not the catalogue format.
pub fn catalogue(restores: &[String], file: Option<&Path>) -> Result<Catalogue> {
    let mut catalogue = match file {
        None => Catalogue::new(),
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| Error::Io {
                action: "read",
                path: path.display().to_string(),
                source: e,
            })?;
            Catalogue::from_json(&bytes).map_err(|detail| Error::Usage {
                detail: format!("--restore-catalogue {}: {detail}", path.display()),
            })?
        }
    };
    for entry in restores {
        let (tool, restored_by) = entry.split_once('=').ok_or_else(|| Error::Usage {
            detail: format!(
                "--restore takes <TOOL>=<RESTORE_TOOL> and got {entry:?}: a catalogue says \"a call \
                 to this tool is undone by a call to that one\" (sem: SEM-gx-cli-093), and only the party running the \
                 server knows it (`gx-adapter-mcp`'s catalogue.rs)"
            ),
        })?;
        if tool.is_empty() || restored_by.is_empty() {
            return Err(Error::Usage {
                detail: format!("--restore {entry:?} names an empty tool"),
            });
        }
        catalogue = catalogue.with_restore(tool, restored_by);
    }
    // 🔴 **`req/303` L-04** (`req/38` §221 ruling 5) — the flag mouth is asked the same question as
    // the file mouth, at the same time.
    //
    // `Catalogue::from_json` runs `soundness()` on the bytes it parses, so
    // `--restore-catalogue <file>` with a blank restore name is a **parse error**: `gx wrap` does
    // not start, and `docs/LIMITS.md` said so. `--restore 'doc.write= '` took a different road —
    // `with_restore` builds the entry in code, which no parser ever saw — so the session started,
    // the server was spawned, and the same declaration was refused later by `crate::invert`'s
    // `entry_fault`. The mechanism was right (the object did not move, nothing was sent) and the
    // asymmetry was in the *sentence*: one spelling of one declaration was a start-up error and the
    // other was a per-call refusal.
    //
    // Asked after the flags are applied rather than during, for `from_json`'s own reason: a
    // `read_by` naming an effect is a question about the **whole** file, and a half-built map
    // answers it against half a file. `Error::Usage` because that is what this function's other
    // faults are, and because a start-up flag that cannot be acted on is a usage error.
    catalogue.soundness().map_err(|detail| Error::Usage {
        detail: format!("--restore: {detail}"),
    })?;
    Ok(catalogue)
}

/// 🔴 **R19 / `req/279` M-07** — this session's server and restore declaration, spelled as the
/// `--mcp-*` globals a single-shot verb takes.
///
/// The proxy knows every word of it: the command it started (`spec.command` and `spec.args`), the
/// endpoint every locator it minted carries, and the restore declaration it was handed. What used
/// to be printed was `--mcp-server <the same server>` — a placeholder, and the audit's measurement
/// of what a placeholder costs is in `EngineGate::undo_remedy`'s comment.
///
/// **Not** included: `--mcp-server-env`. See [`environment`] — the values are the deployment's
/// credentials and this module does not print them. Their names travel in `_meta` beside the
/// remedy (`gx/undo_server_env`), so the omission is stated rather than silent.
#[must_use]
pub fn undo_flags(spec: &WrapSpec, endpoint: &str) -> String {
    let mut words: Vec<String> = vec!["--mcp-server".into(), shell_word(&spec.command)];
    for arg in &spec.args {
        words.push("--mcp-server-arg".into());
        words.push(shell_word(arg));
    }
    // Always, and not only when `--endpoint` was given: `gx wrap` mints
    // `stdio://<command>` by default and a single-shot verb would mint its own from **its**
    // spelling of the command, so an undo that did not name the endpoint could look for the
    // escrowed inverse under a locator this session never wrote.
    words.push("--mcp-endpoint".into());
    words.push(shell_word(endpoint));
    if let Some(path) = &spec.restore_catalogue {
        words.push("--mcp-restore-catalogue".into());
        words.push(shell_word(&path.display().to_string()));
    }
    for restore in &spec.restores {
        words.push("--mcp-restore".into());
        words.push(shell_word(restore));
    }
    words.join(" ")
}

/// A word of a printed command line, quoted only when it would otherwise split.
///
/// Double quotes and nothing cleverer: every member above is a command, a path, an endpoint or a
/// `TOOL=RESTORE_TOOL` pair minted by this repository, and a full POSIX quoter would be more
/// machinery than the sentence has shapes. A word already holding a double quote is left alone
/// rather than mangled — it would be a server command nobody could have typed.
fn shell_word(word: &str) -> String {
    if word.is_empty() || word.contains(char::is_whitespace) && !word.contains('"') {
        return format!("\"{word}\"");
    }
    word.to_string()
}

/// Parse `--server-env NAME=VALUE` into the pairs a child is started with.
///
/// The **names** are echoed in the start-up line and the values never are: an `env` member is where
/// a server's credential lives, and a proxy that printed one would put it in the operator's terminal
/// and in whatever collects it.
///
/// # Errors
/// [`Error::Usage`] for a spelling with no `=`.
pub fn environment(entries: &[String]) -> Result<Vec<(String, String)>> {
    entries
        .iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .ok_or_else(|| Error::Usage {
                    detail: format!("--server-env takes NAME=VALUE and got {entry:?}"),
                })
        })
        .collect()
}

/// Start a server, hand it a handshake, and put the gate between it and the agent.
///
/// # Errors
/// [`Error::Usage`] for a server that will not start or a handshake that will not agree — including
/// **AC-P3-2**'s case, a protocol revision this build has not read the specification for.
pub fn run(project: &Path, spec: &WrapSpec) -> Result<Outcome> {
    let env = environment(&spec.env)?;
    // 🔴 **`req/38` §207 ruling 4** (`req/285` §2's residue) — the catalogue is read and checked for
    // soundness **before** anything is spawned. It used to be read after `initialize()`, which made
    // the refusal correct and the sentence wrong: `docs/LIMITS.md` tells a buyer that a declaration
    // naming a read face with no restore template means "`gx wrap` does not start", and what
    // actually happened was that the server's child process started, completed a handshake, and
    // was then dropped. No gate opened and no `tools/call` was relayed, so nothing was written —
    // but a process that ran is a process that ran, and the sentence is now literal.
    // `crates/gx-cli/tests/r20_wrap_parse_before_spawn.rs` holds the order from the outside, by the
    // only evidence that does not depend on reading this file: whether the child ran at all.
    let catalogue = catalogue(&spec.restores, spec.restore_catalogue.as_deref())?;
    let client = Arc::new(
        StdioClient::spawn_with_env(&spec.command, &spec.args, &env).map_err(|e| Error::Usage {
            detail: e.to_string(),
        })?,
    );
    let handshake = client.initialize().map_err(|e| Error::Usage {
        detail: e.to_string(),
    })?;
    let endpoint = spec
        .endpoint
        .clone()
        .unwrap_or_else(|| gx_mcp_wire::stdio_endpoint(&spec.command));
    let stripped = stripped_member(&spec.resource_arg, spec.forward_resource_arg);
    let mut transport = WireTransport::new(client.clone(), endpoint.clone());
    if let Some(member) = &stripped {
        transport = transport.stripping(member.clone());
    }
    let transport = Arc::new(transport);
    let mode = spec.record_only.then_some(EnforcementMode::RecordOnly);
    // 🔴 **P3.1** (gotcha66, `req/38` §74 item③) — the same vocabulary `gx serve --fail-posture`
    // parses (`crates/gx-cli/src/serve.rs::build`), so an operator reads one flag twice rather than
    // two flags that happen to agree.
    let posture = match spec.fail_posture.as_deref() {
        None | Some("closed") => gx_core::FailPosture::FailClosed,
        Some("open") => gx_core::FailPosture::FailOpen,
        Some(other) => {
            return Err(Error::Usage {
                detail: format!(
                    "--fail-posture takes closed|open (44 §1.2); got {other:?}. The two axes are \
                     independent (43 §4): --record-only decides whether a Deny still applies, \
                     --fail-posture decides what happens when the verifier cannot be reached"
                ),
            })
        }
    };
    let otel = crate::otel::Exporter::from_flag(spec.otel_file.as_deref(), spec.otel_locators)?;

    // 🔴 The start-up line goes to **stderr**, and it names what is off as well as what is on — the
    // `gx serve` discipline ("otel: disabled" and the rest; sem: SEM-gx-cli-094) one verb along.
    let start = json!({
        "gx": "wrap",
        "server_command": spec.command,
        "endpoint": endpoint,
        "protocol_revision": handshake.revision,
        "server_info": handshake.server_info,
        "restorable_tools": catalogue.declared(),
        // 🔴 `req/269` M-01 (`req/38` §199 ruling 3): the escrow read posture is a property of the
        // **whole catalogue file** -- one line in it moves the fail-closed default on every
        // declaration, including the ones that name no read tool and take the `resources/read`
        // road they always took. Until this window it was printed nowhere, so a relaxation was
        // opt-in on paper and invisible in operation.
        "on_read_failure": catalogue.on_read_failure().as_str(),
        // 🔴 **`req/308` §4(g) / `req/38` §222 ruling (g)** — DR-46-16's `"$cas_read"` slot decides,
        // per resource-URI prefix, whether `snapshot`, `precondition` and the post-apply
        // observation go through a declared read tool or through `resources/read`. That is the
        // reading road of every locator in the session, set by one line of the file, and it was
        // printed nowhere — the same defect `req/269` M-01 closed for `on_read_failure` one field
        // up. A zero is printed rather than the field omitted, so "this catalogue declares none" is
        // a fact an operator reads instead of infers.
        "cas_reads": catalogue.cas_reads_declared(),
        "server_env_names": env.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>(),
        "resource_arg": spec.resource_arg,
        // DR-V4B-3 (`req/189`): what this session takes off the wire form of every call — the
        // member named, or `null` (a real tool argument, or `--forward-resource-arg`).
        "resource_arg_stripped_before_send": stripped,
        "record_only": spec.record_only,
        "fail_posture": spec.fail_posture.as_deref().unwrap_or("closed"),
        // 🔴 R-114-4's "state zero telemetry explicitly" (sem: SEM-gx-cli-095): the word "disabled" is printed rather than left to be
        // inferred from the absence of a field.
        "otel": otel.status(),
        "otel_not_exported": crate::otel::NOT_EXPORTED,
        "policy": spec.policy.as_ref().map(|p| p.display().to_string()),
        "gated_methods": ["tools/call"],
        "unclassified_methods": "refused (req/119 §2.6 B-7/B-8: the default is refusal)",
        "not_closed": "B-2 (an agent that starts a server itself) and B-10 (a server that resolves \
                       one resource name to another) are declared, not closed. `gx wrap \
                       --check-config` is the machine check for B-1.",
    });
    crate::note!("{start}");

    let mut session = Session::open_wired_with_posture(
        project,
        false,
        Vec::new(),
        mode,
        posture,
        spec.policy.as_deref(),
        &McpWiring::wired(transport, catalogue),
    )?;
    // 🔴 **T6 condition ② (per operation, not per process)** — `req/38` §148, `req/190` F-7.
    //
    // `gx wrap` holds this session for the whole life of an MCP proxy, which can be an entire agent
    // run. `Session::open` took the project's `.gx/LOCK`; holding it for that life would mean a
    // `gx serve` beside this process could never write, and "the GUI runs `gx serve`, the agent runs
    // `gx wrap`" is the premise DR-43-2 exists to keep true. So it is released here, and
    // `EngineGate::gate` takes it again around each `tools/call` — which is the one operation this
    // surface performs and therefore the right grain.
    session.release_writer_lock();
    let actor = Actor::Agent {
        key: spec.actor_key.clone(),
        model: spec.actor_model.clone(),
    };
    // Taken before `endpoint` moves into the gate: the undo has to name the **same** endpoint this
    // session minted its locators with (`undo_flags`).
    let endpoint_for_undo = endpoint.clone();
    let mut gate = EngineGate {
        session: &mut session,
        client: client.clone(),
        endpoint,
        resource_arg: spec.resource_arg.clone(),
        actor,
        context: crate::change_context(&spec.context)?,
        mode,
        otel,
        // 🔴 **R19 / `req/279` M-07** — built once, from this session's own invocation.
        undo_flags: undo_flags(spec, &endpoint_for_undo),
        server_env_names: env.iter().map(|(name, _)| name.clone()).collect(),
    };

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stats = Proxy::new(client.clone(), &mut gate)
        .run(BufReader::new(stdin.lock()), stdout.lock())
        .map_err(|e| Error::Io {
            action: "proxy",
            path: "<stdio>".to_string(),
            source: e,
        })?;
    let _ = client.shutdown();

    let summary = json!({
        "gx": "wrap",
        "session": stats.to_json(),
        "client_calls": client.calls(),
        "client_reads": client.reads(),
    });
    // stderr again, and for the same reason: the agent's parser is still reading stdout as MCP.
    //
    // 🔴 **R15 / `req/259` M-02** — and the delivery is a value now.
    //
    // What stood here was `let mut err = std::io::stderr(); let _ = writeln!(err, "{summary}");` —
    // the membrane's own session summary, thrown at a stream with the answer to "did it arrive"
    // discarded on the spot. That is the third instance of the class R13 closed for stdout and R14
    // closed for the problem object, and the audit named it as code rather than as a measurement
    // because `:491` above panicked first and hid it. Both sites take [`crate::emit::note`] now.
    // The summary is **also** this verb's `Outcome`, so a run whose stderr is gone still puts it on
    // stdout, where 44 §1.3 says a verb's single object goes.
    crate::note!("{summary}");
    Ok(Outcome::ok(summary))
}
