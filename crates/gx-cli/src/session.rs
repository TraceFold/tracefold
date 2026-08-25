// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! An engine over a project's `.gx/`, and the resume that makes four processes one pipeline.
//!
//! # 🔴 The hole this hand found, and why it is not M6-01
//!
//! req/88 §4 M6-01 is about the **intent body** between `gx submit` and `gx plan`, and hand 1 closed
//! it with [`crate::draft`]. What this hand ran into one step later is the **row**:
//!
//! * `Engine::open` rebuilds the draft phase and 44 §0's resolution index from the journal, and
//!   deliberately leaves the in-flight table empty (M5H3-5);
//! * a row holds a `Transformation`, an `ObjectSnapshot` and a `PlannedDelta`, and the journal holds
//!   names and digests rather than bodies (ASM-9);
//! * so `gx verify <TID>` in a fresh process is a call about a row that is not there, and
//!   `gx commit <TID>` is the same one state later.
//!
//! req/88 §2.1 marked `gx plan` as the blocker and marked `verify` and `commit` "conditionally OK" (sem: SEM-gx-cli-203) on the
//! strength of `--evidence` and `--record-only`. The table was the condition nobody had named.
//! Raised as **M6H3-1**.
//!
//! # The answer: the body is re-planned, the state comes from the journal
//!
//! [`Session::resume`] finds the draft the transformation came from and calls `Engine::plan` again.
//! 43 T-2's idempotency column is the licence — "re-running against the same snapshot produces
//! the same `PlannedDelta` and the same `TransformationId` (safe to retry)" (sem: SEM-gx-cli-204) — and M6 hand 3's change to `Engine::plan` is what makes
//! it *silent*: a re-plan that lands on the recorded id rebuilds the row from the journal's state
//! and appends no second `Planned` record.
//!
//! That silence is the whole point, and it is **req/88 §3 Λ2** rather than tidiness. Λ2 claims that
//! "N CLI invocations" and "N calls into one long-lived engine" (sem: SEM-gx-cli-205) are observationally equal on Σ.
//! A resume that re-drove the pipeline would put a second `VerifyStarted` and a second `Verdict` in
//! the log and issue a second verdict receipt — the single-shot CLI and `gx serve` disagreeing about
//! how many times the gate was asked, which is exactly the equality Λ2 asserts. So the resume writes
//! nothing, and Λ2 now holds from `Candidate` onward rather than only up to it. **The one place it
//! still does not hold is the Draft phase**, which is Λ2's own named counter-example
//! (`.gx/drafts/` is CLI-side state the HTTP surface does not have) and is written in the crate
//! root.
//!
//! # 🔴 What the world moving does
//!
//! A re-plan consults the substrate, so if the file changed between `gx plan` and `gx verify` the
//! recomputed `TransformationId` is a **different one**. That is not a failure of the resume: 43 §8
//! says the same thing about a committed predecessor ("`Fingerprint₀` has gone stale, so a
//! re-`plan()` (re-fingerprint) is forced" (sem: SEM-gx-cli-206)). The engine refuses it without writing anything when the recorded
//! row has moved past `Candidate`, and [`Session::resume`] refuses it here when the ids differ, so
//! an operator is told that the world moved rather than being handed a transformation they did not
//! name.
//!
//! # 🔴 Which key signs, and the separation that is not implemented (M6H3-4)
//!
//! `Engine::verify` and `Engine::commit` both take a `&KeyPair` — T-4's verdict receipt and T-11's
//! commit receipt are signed — and 44 §1.2 gives neither command a key argument. req/56 §3 gives the
//! CLI **one** key directory. So the key this module loads is the one the transformation's own
//! `Actor` names, out of `~/.gx/keys/`.
//!
//! 45 §1 distinguishes a ledger/engine signing key from an actor key, and **that separation is not
//! implemented at this layer**: in v0.1 the receipt over a change is signed by the key of the actor
//! who asked for it. The consequence is worth stating plainly rather than leaving in a diagram — a
//! receipt says "this actor's key attests this change was verified" and not "the engine attests
//! it" (sem: SEM-gx-cli-207), so an operator holding their own key can produce a receipt without the engine's consent
//! being a separate fact. `gx log checkpoint` (hand 2) is the one place the ledger key is already an
//! explicit `--key`, which is why the ledger head is not affected. Raised as **M6H3-4**.

use std::path::Path;
use std::sync::Arc;

use gx_core::{EnforcementMode, Intent, IntentId, Timestamp, TransformationId};
use gx_engine::store::{OwnedLock, ProcessLock};
use gx_engine::{reconstruct, Engine, InjectedEvidence, StateRow};
use gx_gate::Gate;
use gx_witness::KeyPair;

use crate::draft::DraftStore;
use crate::index::ResolutionIndex;
use crate::keys::KeyStore;
use crate::layout::{journal_absent, Layout};
use crate::{Error, Result};

/// The version string a registered adapter is recorded under (42 §3.9's `adapter_version`).
///
/// **M5H4-4**: 41 §4's trait has seven methods and none reports a version, so the registrant says.
/// This binary's registrant is this file, and what it knows is its own package version — which is
/// the honest answer to "which build of the fs adapter was wired in" (sem: SEM-gx-cli-208) for a single static binary
/// (NFR-019), where the adapter and the CLI are one artefact.
pub const ADAPTER_VERSION: &str = concat!("gx-cli ", env!("CARGO_PKG_VERSION"));

/// 🔴 The substrates a `gx` binary can act on when it was told nothing (**M7 hand 4**).
///
/// `req/38` §60's **R-9 paired ruling**: "the default policy set and the default registry are
/// decided **as a pair** (adding a pack without registering an adapter still gives NotFound)" (sem: SEM-gx-cli-209). [`default_policies`] is the first half and
/// [`register_default_adapters`] is the second; this constant is what the second half is checked
/// against, in the shape [`gx_gate::packs::FS_PACK_POLICY_IDS`] is checked against its pack —
/// declared here, derived there, compared by `crates/gx-cli/tests/defaults.rs`.
///
/// The spellings are 44 §1.2's, which is the vocabulary [`crate::substrate_kind`] reads.
///
/// 🔴 **Three since P3** (`req/38` §71 P3 ruling ①, `req/119` §2.5; sem: SEM-gx-cli-210). [`MCP_IS_NOT_REGISTERED`] is kept
/// below with its firing condition, and [`MCP_REGISTRATION_FIRED`] is the record that the condition
/// fired — what registration buys and what it does not is written there rather than implied by a
/// constant that grew by one.
///
/// 🔴 **Four since v0.2.7 Lane A** (`req/38` §81 ruling 1, `req/137` §A1 item 1, gotcha75; sem: SEM-gx-cli-211). Unlike
/// `mcp`, `custom:postgres` needed no firing condition to arrive here: `PostgresAdapter` is a
/// stateless `Copy` struct and every method resolves its own DSN lazily
/// (`gx_adapter_postgres::db::connect`), so nothing about registering it was ever blocked on a
/// deployment supplying a transport — the gap `req/136` §4-1 found was an omission (no crate in
/// this repository ever called `engine.register_adapter` with one), not a deferral with a reason.
/// 🔴 **Three, not four, without the `pg` feature** (`req/817`). `gx-adapter-postgres` is one of
/// the four crates `req/789` §3 holds private, so the public distribution does not register it and
/// must not declare it either — this constant is the declaration `defaults.rs` compares against
/// what actually registers, and a declaration that named an adapter the binary cannot build would
/// turn that probe from a check into a lie. The pair stays honest in both builds.
#[cfg(feature = "pg")]
pub const DEFAULT_SUBSTRATES: [&str; 4] = ["fs", "git", "mcp", "custom:postgres"];
#[cfg(not(feature = "pg"))]
pub const DEFAULT_SUBSTRATES: [&str; 3] = ["fs", "git", "mcp"];

/// 🔴 Why the third of 44 §1.2's three substrates is not in [`DEFAULT_SUBSTRATES`], and what would
/// change that.
///
/// The deferral is the honest half of hand 4's answer to R-9 and it carries a firing condition,
/// because **D-7**'s rule is that a deferral without one is "a hand will do it later" (sem: SEM-gx-cli-212) — which
/// §41 M5H4-8 refused to accept and this project has since armed twice
/// (`crates/gx-cli/src/consumers.rs`; sem: SEM-gx-cli-213, SEM-gx-cli-214).
pub const MCP_IS_NOT_REGISTERED: &str = "\
`gx-adapter-mcp` is **not** in the default registry, and the reason is that it cannot be built \
without one: `McpAdapter::new` takes a `ToolTransport`, which is the deployment's code by a ruling \
this project has made twice — the crate's own manifest (\"a linked MCP client library would be a \
second road to the substrate, with no `Admitted` in its signature, reachable by anyone who can name \
the crate\") and req/101 §9 **R-2**, which files the absent JSON-RPC framing as deliberate rather \
than unfinished. Registering it behind a transport that refused everything would buy nothing — the \
refusal lands in `snapshot`, before a gate, so `policies/mcp/` would still be unreachable from this \
binary — and would cost a made-up `adapter_version` in a signed provenance record, which is the \
mistake M5H4-4 named when it made the version an argument. \
The **firing condition**: the day a wire ships (v0.2's `CallLog`/transport window, req/101 §9 R-1 \
and R-2), `gx serve` and the single-shot verbs register it here and 44 §1.2's `--substrate mcp` \
stops being a flag no invocation of this binary can satisfy. Until then the refusal an operator \
sees is `NotFound { what: \"adapter\" }`, which is the true one: this build has no MCP wire.";

/// 🔴 The day named above, and what it did **not** buy (**P3**, `req/38` §71).
///
/// [`MCP_IS_NOT_REGISTERED`] stays where it is — no-delete, and a deferral that fired is a record
/// rather than a mistake — and this is the other half of the pair: the wire shipped
/// (`gx-mcp-wire`), so `gx-adapter-mcp` registers here, `policies/mcp/` is reachable from this
/// binary, and 44 §1.2's `--substrate mcp` is a flag an invocation can satisfy.
///
/// **What registration is not**: a server. `McpAdapter::new` takes a `ToolTransport` and this
/// binary builds one only when an invocation names a server — `gx wrap -- <cmd>`, or the
/// `--mcp-server` flag a single-shot verb carries. With neither, the adapter is registered behind
/// `UnconfiguredTransport` (private), whose refusal says exactly that. The distinction is worth the
/// sentence: "the substrate has an adapter" and "this process is connected to a server" are two
/// facts, and a deployment that read the first as the second would be told "not found" (sem: SEM-gx-cli-215) by a proxy
/// that is, in fact, right there.
pub const MCP_REGISTRATION_FIRED: &str = "\
`gx-adapter-mcp` **is** in the default registry since P3 (req/38 §71, req/119 §2.5): `gx-mcp-wire` \
ships the JSON-RPC framing whose absence was the firing condition, so the transport is no longer \
only the deployment's. What an invocation still has to name is the **server**: `gx wrap -- <cmd>` \
runs the proxy over a spawned server, and `--mcp-server <CMD>` wires one for a single-shot verb \
(`gx undo` needs it, because a compensating call is a call). Without either, the registered adapter \
holds `UnconfiguredTransport` and every read and every call refuses with that sentence — which is \
the true one: this process is connected to no MCP server. AC-051's D-1 is untouched by all of it: \
`ToolCall` and `Admitted` are still unconstructible outside `gx-adapter-mcp`, and a second crate \
implementing `ToolTransport` is what the trait was published for.";

/// 🔴 The policy set a `gx` decides with when no `--policy` named one (**M7 hand 4**).
///
/// Until this hand it was `packs::fs_pack()`, and `crates/gx-gate/src/packs.rs` carried the note
/// saying why that had to change the day a second adapter was registered. req/38 §60 ruled the day,
/// req/101 §9-1 is the material it took, and this is its second change point (sem: SEM-gx-cli-216): "change
/// `gx_cli::session::open_engine`'s `None` branch from `fs_pack()` to `shipped_pack_set()`".
///
/// # Rule 1 (ii) is untouched by the widening (sem: SEM-gx-cli-217)
///
/// What this crate does is **choose** which set the engine judges with; it does not evaluate one,
/// read a decision or name a `Verdict`. A wider set is a wider choice and not a new capability, and
/// the bytes still come from gx-gate's `include_str!` rather than from a file this binary opens
/// (FR-028's "there is exactly one road by which a pack's file gets embedded into the build"; sem: SEM-gx-cli-218).
///
/// # Errors
/// [`Error::Gate`] if the composition does not parse — which for compile-time-fixed inputs means a
/// broken shipped artifact, and includes the case of two packs claiming one statement id.
pub fn default_policies() -> Result<gx_gate::PolicyEngine> {
    Ok(gx_gate::packs::shipped_pack_set()?)
}

/// 🔴 Put the adapters this binary ships into an engine's registry, and say which arrived.
///
/// The second half of the pair. The return value is **derived** — each adapter is asked its own
/// [`gx_substrate::SubstrateAdapter::kind`] after it is registered — so that
/// [`DEFAULT_SUBSTRATES`] is a declaration something compares with reality rather than a comment
/// with a constant's syntax.
///
/// req/88 §1 N-13 already ruled that a CLI shipping adapters is not a breach of anything: "the CLI
/// is the layer that takes 'which substrate' via `--substrate` and registers it into the registry;
/// without knowing the adapter it can do nothing. What N-13 protects is the engine's neutrality,
/// not the CLI's neutrality" (sem: SEM-gx-cli-219). `gx-engine`'s
/// own manifest still declares no adapter and `ENGINE_SHIPPED_ADAPTERS=0` is unmoved.
pub fn register_default_adapters<E: gx_engine::EvidenceSource>(
    engine: &mut Engine<E>,
) -> Vec<String> {
    register_default_adapters_with(engine, &McpWiring::default())
}

/// 🔴 The same registry, with the MCP half told which server it is talking to (**P3**).
///
/// One function rather than two roads: [`register_default_adapters`] delegates here with
/// [`McpWiring::default`], so "which adapters does a `gx` have" (sem: SEM-gx-cli-220) has one answer and the only
/// difference between an invocation that named a server and one that did not is **the transport
/// inside the third adapter**.
///
/// 🔴 **v0.2.7 Lane A** (`req/38` §81 ruling 1, gotcha75; sem: SEM-gx-cli-221) adds the fourth: `PostgresAdapter::new()`
/// takes nothing (it holds no connection, the same posture `GitAdapter` argues for), so there is no
/// wiring question the way `McpWiring` answers one for the third — it registers unconditionally,
/// and a DSN a deployment never set is refused by name at the first method call
/// (`gx_adapter_postgres::db::dsn_for`), not by this function.
pub fn register_default_adapters_with<E: gx_engine::EvidenceSource>(
    engine: &mut Engine<E>,
    mcp: &McpWiring,
) -> Vec<String> {
    // 🔴 A `Vec` and not a `[_; 4]` since `req/817`: the postgres arm is `cfg(feature = "pg")`
    // because `gx-adapter-postgres` is one of the four crates `req/789` §3 holds private, so the
    // public distribution registers three adapters here and not four. The count is read from
    // `adapters.len()` below rather than written down twice, so the two cannot drift.
    // `mut` is used only by the `pg` push below, so a build without that feature does not need it.
    #[cfg_attr(not(feature = "pg"), allow(unused_mut))]
    let mut adapters: Vec<std::sync::Arc<dyn gx_substrate::SubstrateAdapter>> = vec![
        std::sync::Arc::new(gx_adapter_fs::FsAdapter::new()),
        std::sync::Arc::new(gx_adapter_git::GitAdapter::new()),
        std::sync::Arc::new(mcp.adapter()),
    ];
    #[cfg(feature = "pg")]
    adapters.push(std::sync::Arc::new(
        gx_adapter_postgres::PostgresAdapter::new(),
    ));
    let mut registered = Vec::with_capacity(adapters.len());
    for adapter in adapters {
        registered.push(substrate_tag(&adapter.kind()));
        engine.register_adapter(adapter, ADAPTER_VERSION);
    }
    // 🔴 Two-phase escrow (`req/38` §99 ruling 2-②; sem: SEM-gx-cli-222): the MCP adapter is also the escrow
    // completion for its own substrate, registered beside it — the same wiring decision, made in
    // the same place, so "which adapters does a `gx` have" and "which escrows can it complete" (sem: SEM-gx-cli-223)
    // cannot drift apart. Unconditional for the same reason the adapters are: an invocation that
    // named no server completes nothing because nothing reaches Committing on that road, not
    // because the registry forgot.
    engine.register_completion(
        gx_core::SubstrateKind::Mcp,
        std::sync::Arc::new(mcp.adapter()),
    );
    // 🔴 **DR-46-33 / DR-46-28** (`req/38` §413): the MCP adapter also declares where its inputs
    // come from (its catalogue's `$determinism_boundary` slot), registered beside it for the escrow
    // completion's reason — the same wiring decision in the same place, so "which adapters does a
    // `gx` have" and "which of them declare an input stage" cannot drift. Unconditional too: a
    // catalogue that declared nothing answers `unknown`, which is v0's boundary unchanged.
    engine.register_input_stage_declaration(
        gx_core::SubstrateKind::Mcp,
        std::sync::Arc::new(mcp.adapter()),
    );
    registered
}

/// Which MCP server this process is connected to, and which of its tools can be undone.
///
/// Two fields and both are the deployment's: the transport is "where the server is" and the
/// catalogue is "what undoes what" (sem: SEM-gx-cli-224), which only the party running the server knows
/// (`gx-adapter-mcp`'s `catalogue.rs`). [`McpWiring::default`] is "no server named", and it is the
/// value every verb that was given no `--mcp-server` opens with.
#[derive(Clone, Default)]
pub struct McpWiring {
    transport: Option<std::sync::Arc<dyn gx_adapter_mcp::ToolTransport>>,
    catalogue: gx_adapter_mcp::Catalogue,
    /// 🔴 **`req/291` M-01** — which surface asked, so that a refusal can name a remedy that
    /// exists **on that surface**. See [`McpSurface`].
    surface: McpSurface,
}

/// 🔴 **`req/291` M-01** — which road a refusal for want of a server is being written for.
///
/// # The hole this closes, measured
///
/// One sentence served every caller: *`gx wrap -- <server command>` runs the proxy over one, and
/// `--mcp-server <CMD>` wires one for a single-shot verb.* The twentieth audit read it back on two
/// surfaces and found **both halves false for `gx cancel`**.
///
/// * HTTP: `POST /candidates/{id}/cancel` against a `gx serve` that named no server answers **502**
///   carrying that sentence — and neither remedy applies, because this process's server is fixed at
///   start-up and a `gx wrap` somewhere else does not wire it.
/// * CLI: `gx cancel <TID> --mcp-server …` exits **1** with R19's usage refusal (*this verb opens no
///   road to an MCP server*), and without the flag exits 1 with the sentence above — **naming the
///   flag the other refusal had just refused**. Two refusals pointing at each other is worse than
///   the silent drop R19 removed, because a reader can execute neither.
///
/// The repair `req/298` §1 item 5 ranked first was to give `cancel` a road that needs no snapshot.
/// It is not reachable from the crates this lane may write: `Engine::cancel` refuses any row that
/// is not in the **live table**, the only in-scope way to seat a row raised by another process is
/// `handlers::rebuilt` / `Session::resume`, and both re-plan — which snapshots. So this is item 5's
/// declared fallback ③, and `req/299` says so rather than implying the first was tried and worked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpSurface {
    /// A single-shot verb that reads `--mcp-server` (`submit`, `plan`, `verify`, `commit`, `undo`,
    /// `escalation`). The pre-existing sentence, unchanged.
    #[default]
    SingleShot,
    /// The long-lived HTTP surface, whose server is chosen once at start-up.
    Server,
    /// A verb that opens no road to an MCP server at all, named. `--mcp-server` is a usage error on
    /// it (`req/279` H-02 (c)), so the refusal must not print that flag as its remedy.
    NoRoad(&'static str),
}

impl std::fmt::Debug for McpWiring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpWiring")
            .field("server", &self.transport.is_some())
            .field("restorable_tools", &self.catalogue.declared())
            // 🔴 `req/269` M-01: catalogue-wide and previously printed nowhere.
            .field(
                "on_read_failure",
                &self.catalogue.on_read_failure().as_str(),
            )
            .finish()
    }
}

impl McpWiring {
    /// A wiring that names a server, and the restores that server's operator declared.
    #[must_use]
    pub fn wired(
        transport: std::sync::Arc<dyn gx_adapter_mcp::ToolTransport>,
        catalogue: gx_adapter_mcp::Catalogue,
    ) -> Self {
        Self {
            transport: Some(transport),
            catalogue,
            surface: McpSurface::SingleShot,
        }
    }

    /// 🔴 **`req/291` M-01** — the same wiring, told which surface is about to read its refusals.
    #[must_use]
    pub fn on_surface(mut self, surface: McpSurface) -> Self {
        self.surface = surface;
        self
    }

    /// Whether an invocation named a server.
    #[must_use]
    pub fn is_wired(&self) -> bool {
        self.transport.is_some()
    }

    /// 🔴 **R19** (`req/284` §1.1 (b)) — this wiring, for a start-up line an operator reads.
    ///
    /// `gx serve` is now a road that can be connected to an MCP server (audit 19 H-02 (b)), and
    /// P-3's discipline — the one `gx wrap`'s `"otel": "disabled"` and `"on_read_failure"` fields
    /// follow — is that a **zero is stated** rather than inferred from a missing field. A reader of
    /// two start-up lines must be able to tell the server that can carry an MCP ruling from the
    /// server that will refuse one, without running a request to find out.
    #[must_use]
    pub fn summary(&self) -> serde_json::Value {
        if self.transport.is_none() {
            return serde_json::json!({
                "server": serde_json::Value::Null,
                "restorable_tools": 0,
                // 🔴 **`req/291` M-02** — the width is the measured one. R19 wrote "an MCP ruling
                // or undo", and the twentieth audit put four verbs against a `gx serve` that named
                // no server: the ruling **502**, the undo **502**, and `cancel` and `verify` — which
                // are neither a ruling nor an undo — **502** as well. All four take
                // `handlers::with_a_body`, whose rebuild re-plans, and a re-plan snapshots. A
                // declaration narrower than the behaviour is the same defect as one that is wider:
                // it is a sentence a reader cannot use to predict the surface.
                // 🔴 **`req/303` M-01 (R22)** — five, not four. R20's arm put the four verbs it had
                // been handed against the surface and asked whether the sentence named them; it
                // never asked the **router** which verbs a serverless surface refuses, so
                // `POST /candidates/{id}/commit` — the fifth `with_a_body` caller — was refused by
                // the surface and unnamed by the line. The arm is now the other way round
                // (`r22_serverless_surface.rs`): enumerate `handlers.rs`'s `with_a_body` callers
                // from the source, then require this sentence to contain every one of them. A
                // sixth caller added later makes that arm red without anyone remembering.
                "note": "no server named: an MCP ruling, undo, cancel, commit, or verify on this \
                         surface is refused (`--mcp-server <CMD>` at start-up wires one for this \
                         process)",
            });
        }
        serde_json::json!({
            "server": "named by this invocation (`--mcp-server`); the connection line is on stderr",
            "restorable_tools": self.catalogue.declared(),
            "on_read_failure": self.catalogue.on_read_failure().as_str(),
            // 🔴 **`req/308` §4(g) / `req/38` §222 ruling (g)** — DR-46-16's `"$cas_read"` changes
            // the **reading road** of every locator under a declared prefix, and until this line it
            // was printed nowhere. That is exactly the defect `req/269` M-01 closed for
            // `on_read_failure`: a posture a single line of the file sets, invisible to the
            // operator who has to approve the file. Stated as a **zero** rather than omitted, which
            // is P-3's discipline and the reason `"otel": "disabled"` is on the sibling line.
            "cas_reads": self.catalogue.cas_reads_declared(),
        })
    }

    /// The adapter this wiring produces.
    #[must_use]
    fn adapter(&self) -> gx_adapter_mcp::McpAdapter {
        let surface = self.surface;
        let transport = self.transport.clone().unwrap_or_else(|| {
            std::sync::Arc::new(UnconfiguredTransport { surface })
                as std::sync::Arc<dyn gx_adapter_mcp::ToolTransport>
        });
        gx_adapter_mcp::McpAdapter::new(transport).with_catalogue(self.catalogue.clone())
    }
}

/// 🔴 The transport a `gx` holds when no invocation named a server.
///
/// It refuses, and the refusal is the true sentence rather than a stand-in for one: "this process is
/// connected to no MCP server" (sem: SEM-gx-cli-225). What it is **not** is the thing `MCP_IS_NOT_REGISTERED` refused —
/// "a transport that refused everything would buy nothing" was written when there was no wire at
/// all, so registering behind it would have left `policies/mcp/` unreachable while claiming
/// otherwise. Since P3 the same binary can reach that pack the moment an invocation names a server,
/// and this value is what stands in the seat until one does.
#[derive(Debug)]
struct UnconfiguredTransport {
    surface: McpSurface,
}

/// The clause every surface shares: **what** is missing. Held apart from the remedy because the
/// fact is one and the executable answers are three (`req/291` M-01).
const NO_SERVER_FACT: &str = "this `gx` is connected to no MCP server";

/// The tail every surface shares: what is *not* missing, so that "registered" stays checkable.
const NO_SERVER_TAIL: &str =
    "The adapter is registered (req/38 §71) and the server is what an invocation names";

/// 🔴 **`req/291` M-01** — the whole sentence, with the remedy that can be executed **here**.
fn no_server(surface: McpSurface) -> String {
    let remedy = match surface {
        // Unchanged from before this lane, verbatim: on a single-shot verb both halves are true.
        McpSurface::SingleShot => {
            "`gx wrap -- <server command>` runs the proxy over one, and `--mcp-server <CMD>` wires \
             one for a single-shot verb"
                .to_string()
        }
        // A running server does not grow a connection because a request wants one: the flag is read
        // once, at start-up, and `gx wrap` elsewhere wires a different process.
        McpSurface::Server => {
            "this server was started without one, and a running server does not acquire one -- \
             `--mcp-server <CMD>` is read once, at start-up, and holds for the process's lifetime. \
             What to fix: start `gx serve --mcp-server <CMD> [--mcp-server-arg …] [--mcp-endpoint \
             …] [--mcp-restore-catalogue …]` again against this project"
                .to_string()
        }
        // The case the audit found pointing at itself: a verb that refuses the very flag the old
        // sentence named as its remedy.
        McpSurface::NoRoad(verb) => format!(
            "and `gx {verb}` opens no road to one -- it refuses `--mcp-server` as a usage error \
             rather than accepting and dropping it (`req/279` H-02 (c)), so that flag is not a \
             remedy this sentence may name. What to fix: reach this row from the surface that does \
             hold a server -- `gx serve --mcp-server <CMD>` over the same project, then `POST \
             /v1/candidates/<id>/cancel`"
        ),
    };
    // `NoRoad`'s remedy continues the fact as one clause; the other two answer it after a colon.
    let joint = if matches!(surface, McpSurface::NoRoad(_)) {
        " "
    } else {
        ": "
    };
    format!("{NO_SERVER_FACT}{joint}{remedy}. {NO_SERVER_TAIL}")
}

impl gx_adapter_mcp::ToolTransport for UnconfiguredTransport {
    fn read(&self, server: &str, resource: &str) -> gx_substrate::Result<Vec<u8>> {
        Err(gx_substrate::Error::Unreadable {
            locator: format!("{server}#{resource}"),
            detail: no_server(self.surface),
        })
    }

    fn call(
        &self,
        _call: &gx_adapter_mcp::transport::ToolCall,
        _admitted: &gx_adapter_mcp::transport::Admitted,
    ) -> gx_substrate::Result<Vec<u8>> {
        Err(gx_substrate::Error::ApplyFailed {
            detail: no_server(self.surface),
        })
    }
}

/// The spelling 44 §1.2 gives a substrate, from the value 42 §3.1 gives it.
///
/// The inverse of [`crate::substrate_kind`], which is the parser for the same vocabulary. Written
/// here rather than borrowed from `gx-gate`'s `policy` module (which has the same table for Cedar's
/// benefit) because that one is private and because the two answer different questions: this is
/// "what did the operator type" and that one is "what does a policy compare against" (sem: SEM-gx-cli-226). They agree,
/// and `crates/gx-cli/tests/defaults.rs` is where the agreement is checked against a real registry.
fn substrate_tag(kind: &gx_core::SubstrateKind) -> String {
    match kind {
        gx_core::SubstrateKind::Fs => "fs".to_string(),
        gx_core::SubstrateKind::Git => "git".to_string(),
        gx_core::SubstrateKind::Mcp => "mcp".to_string(),
        gx_core::SubstrateKind::Custom(name) => format!("custom:{name}"),
    }
}

/// 🔴 How **every** engine in this binary is opened — one spelling, two callers.
///
/// 🔴 **M7 hand 4** widened what "the same decision" means here (sem: SEM-gx-cli-227): the policy set is now every shipped
/// pack ([`default_policies`]) and the registry is every adapter this binary can build
/// ([`register_default_adapters`]). Both are functions rather than lines in this body so that the
/// two halves of req/38 §60's pair can be measured separately from the engine they end up in.
///
/// [`Session`] is the single-shot road (44 §1's twelve verbs) and `gx serve` is the long-lived one,
/// and the difference between them is one type parameter: a CLI process injects the evidence its
/// `--evidence` flags carried (`InjectedEvidence`), a server injects the evidence the current
/// request carried (`gx_api::RequestEvidence`). Everything else — which journal path, which policy
/// pack, which adapter, which two DR-2 axes — is the same decision, and a second spelling of it
/// would be a second answer to "how is a project's engine opened" (sem: SEM-gx-cli-228) waiting to drift.
///
/// `posture` is here and not on [`Session::open`] because 44 §1.2 gives `--fail-posture` to
/// `gx serve` alone; a single-shot verb passes `FailClosed`, which is DR-2's default for every
/// substrate.
///
/// # Errors
/// [`Error::Io`] if the journal's directory cannot be made, [`Error::Engine`] if the journal, blob
/// store or ledger will not open, [`Error::Gate`] if the pack will not parse.
pub fn open_engine<E: gx_engine::EvidenceSource>(
    layout: &Layout,
    evidence: E,
    mode: Option<EnforcementMode>,
    posture: gx_core::FailPosture,
    policy: Option<&Path>,
) -> Result<Engine<E>> {
    open_engine_wired(
        layout,
        evidence,
        mode,
        posture,
        policy,
        &McpWiring::default(),
    )
}

/// 🔴 **R6 / DR-43-11 + `req/229` H-02** — what this project records about itself, read from
/// `.gx/` and handed to the engine.
///
/// Two facts and one side effect, and the side effect is the reason this is a function rather than
/// two lines in two places:
///
/// * `.gx/checkpoints/head.json` — where [`gx_engine::Engine`] compares the tree in front of it
///   with the furthest this project has published, and where it writes the next one.
/// * `.gx/VERSION`'s `journal_format` — the declaration a downgrade contradicts.
/// * **stamping**: on the writer's road, a project that has never declared a format declares the
///   one it actually has, *sniffed from the journal's first eight bytes before anything opens it*.
///   The sniff creates nothing and truncates nothing, which is what makes it safe to run in front
///   of `Engine::open` — and running in front of it is the point, because `EngineJournal::open` is
///   the door that would otherwise cut a marker-stripped file to 93 bytes (`req/229` M-01).
///
/// A project whose journal does not exist yet is declared `chained`: the writer's door is about to
/// create it and `EngineJournal::open` stamps `GXJRNL01` on an empty file. So **every project this
/// release creates is chained and says so**, and only projects that predate it can be `legacy`.
///
/// # Errors
/// [`Error::Io`] if `.gx/VERSION` cannot be read or written.
pub fn anchor_for(layout: &Layout) -> Result<gx_engine::ProjectAnchor> {
    anchor_accepting(layout, false)
}

/// 🔴 **R7 / `req/232` H-01/M-02 + `req/38` §171 ruling 2(c)** — the same anchor, with the two
/// facts R7 adds and the one flag only `gx repair` passes.
///
/// * **the keys** — `~/.gx/keys/`, in the shape a door asks it for the public key a recorded head
///   names. R6 wrote a signature into `head.json` and never checked one; the audit edited the
///   numbers, left the signature alone, and every gate opened.
/// * **the declaration's digest** — `.gx/VERSION` as it stands, compared with the digest the head
///   was written beside (M-02: the declaration could be *rewritten* rather than deleted).
/// * **`accept`** — the operator has decided, with a checkpoint from outside this project, to take
///   the shorter tree (`gx repair --accept-rollback --against <FILE>`). It is `false` on every
///   other road in this binary, including `gx serve`'s.
///
/// # Errors
/// [`Error::Io`] if `.gx/VERSION` cannot be read or written.
pub fn anchor_accepting(layout: &Layout, accept: bool) -> Result<gx_engine::ProjectAnchor> {
    // 🔴 **R12 / `req/242` H-01** — ~~`if stamp && declared_journal_format().is_none() {
    // declare_journal_format(sniff(..)) }`~~ — **removed**.
    //
    // These two lines were the third road into `.gx/VERSION`, and every single-shot writer verb
    // came down them. What they cost is in `Layout`'s tombstone for the function they called; what
    // replaces them is nothing at all. A project that has never declared a framing is
    // `declared_format: None` here, which is the pre-R6 treatment exactly, and a project this
    // binary creates declares `chained` in the write that creates it
    // (`crate::declaration::DeclarationWriter::initialise`). The struck lines are kept because a
    // reader who remembers stamping needs to find out where it went.
    // A store this environment does not have is `None` rather than an error: `gx repair` on a
    // machine with no key store is the investigator's road (`req/227` M-03), and a diagnosis that
    // refused to run because it could not verify a signature would be the narrower-door failure
    // three audits have now closed.
    let keys: Option<std::sync::Arc<dyn gx_engine::HeadKeys>> =
        crate::keys::KeyStore::user_default().ok().map(
            |store| -> std::sync::Arc<dyn gx_engine::HeadKeys> {
                std::sync::Arc::new(crate::keys::StoreHeadKeys::new(store))
            },
        );
    Ok(gx_engine::ProjectAnchor {
        head: Some(gx_log::HeadStore::at(
            layout.head_path(),
            crate::ledger::DEFAULT_ORIGIN,
        )),
        declared_format: layout.declared_journal_format()?,
        keys,
        version_digest: layout.version_digest(),
        accept_rollback: accept,
        // 🔴 **R12 / `req/242` H-01 (d)** — the CLI never creates a journal through the
        // engine. The one road that may is `DeclarationWriter::initialise`, which runs only for a
        // directory that is not a project yet; a project whose journal is **gone** meets
        // `Error::JournalAbsent` at the writer's door and a full report from `gx repair`.
        journal_creation: gx_engine::JournalCreation::Refused,
    })
}

/// The framing a journal is in, from its first eight bytes and nothing else.
///
/// A file that is not there is [`gx_engine::JournalFormat::Chained`], because the writer's door is
/// about to make one and it makes chained ones. A file that cannot be opened is chained for the
/// same reason a detector that cannot read answers "not verified": guessing `legacy` on an I/O
/// error would hand an attacker a way to disable the declaration by making the file unreadable for
/// one open.
pub(crate) fn sniff_journal_format(path: &Path) -> gx_engine::JournalFormat {
    use std::io::Read;
    // 🔴 **R30 / `req/372` M-02** — "the format this binary is about to write" is now
    // `ChainedV2`, so the two absent-file arms answer that. Answering `Chained` would declare a
    // v1 project and then stamp a v2 marker on it, which is R6's downgrade guard firing on this
    // build's own work.
    let Ok(mut file) = std::fs::File::open(path) else {
        return gx_engine::JournalFormat::ChainedV2;
    };
    let mut head = [0u8; 8];
    match file.read_exact(&mut head) {
        Ok(()) if &head == gx_engine::replay::JOURNAL_MAGIC_V2 => {
            gx_engine::JournalFormat::ChainedV2
        }
        Ok(()) if &head == gx_engine::JOURNAL_MAGIC => gx_engine::JournalFormat::Chained,
        // A file too short to hold the marker has no records either, so it is treated as one this
        // binary is about to write: `EngineJournal::open` stamps an empty file.
        Err(_) => gx_engine::JournalFormat::ChainedV2,
        Ok(()) => gx_engine::JournalFormat::Legacy,
    }
}

/// 🔴 **R4 / `req/225` H-01** — the same opening, through DR-43-7's **reader's** door.
///
/// [`open_engine`] is a writer's door all the way down: `Engine::open` creates the journal, the
/// ledger and the verdict chain if they are absent, and it quarantines and then **cuts** a tail
/// that will not replay. That is right for a caller about to append and wrong for one that has
/// promised not to write. `req/225` H-01 measured the gap where the promise was written down three
/// times — 44 §1.2's clause that without `--yes` not one byte is written, `gx repair --help`'s "only reports what
/// it found", and `repair.rs`'s own module documentation — and the code opened the writer's door
/// before it looked at the flag: a 522-byte ledger became 0 bytes on a diagnosis, and beside a live
/// `gx serve` the same diagnosis took `/healthz` from `200` to `500`.
///
/// The policy pack, the adapters and DR-2's two axes are resolved exactly as [`open_engine`]
/// resolves them, because a report about a project has to be a report about the engine that
/// project runs. What is **not** done here is the `create_dir_all` for the journal's parent:
/// a directory this verb made would be a byte this verb wrote.
///
/// # Errors
/// [`Error::Engine`] if any of the three append-only files is absent or will not read (absent is a
/// refusal on this door and a creation on the other), [`Error::Gate`] if the pack will not parse.
pub fn open_engine_read_only<E: gx_engine::EvidenceSource>(
    layout: &Layout,
    evidence: E,
    posture: gx_core::FailPosture,
) -> Result<Engine<E>> {
    let gate = Gate::with_policies(default_policies()?);
    let anchor = anchor_for(layout)?;
    let mut engine =
        Engine::open_read_only_anchored(layout.journal_path(), gate, evidence, anchor)?;
    let _registered = register_default_adapters_with(&mut engine, &McpWiring::default());
    Ok(engine.with_posture(posture))
}

/// 🔴 The same opening, with a server named (**P3**).
///
/// [`open_engine`] delegates here, so the sentence above — "how **every** engine in this binary is
/// opened, one spelling" (sem: SEM-gx-cli-229) — is still one spelling. The sixth argument is a struct rather than two
/// parameters for M5H5-1's reason: a parameter every caller ignores is a parameter every caller has
/// to read, and `McpWiring::default()` is what the ones that ignore it pass.
///
/// # Errors
/// As [`open_engine`].
pub fn open_engine_wired<E: gx_engine::EvidenceSource>(
    layout: &Layout,
    evidence: E,
    mode: Option<EnforcementMode>,
    posture: gx_core::FailPosture,
    policy: Option<&Path>,
    mcp: &McpWiring,
) -> Result<Engine<E>> {
    open_engine_wired_accepting(layout, evidence, mode, posture, policy, mcp, false)
}

/// 🔴 **R7 / `req/38` §171 ruling 2(c)** — the same opening, for the one verb that may proceed over
/// a rollback.
///
/// `accept` reaches exactly one caller: `gx repair --yes --accept-rollback --against <FILE>`, which
/// has already checked that the operator holds a checkpoint from outside this project and that the
/// project is not behind **it**. Every other road in this binary passes `false`, and `gx serve`'s
/// start-up cannot pass anything else — a server that could be told to accept a rollback would be a
/// server an attacker could tell.
///
/// # Errors
/// As [`open_engine_wired`].
pub fn open_engine_wired_accepting<E: gx_engine::EvidenceSource>(
    layout: &Layout,
    evidence: E,
    mode: Option<EnforcementMode>,
    posture: gx_core::FailPosture,
    policy: Option<&Path>,
    mcp: &McpWiring,
    accept: bool,
) -> Result<Engine<E>> {
    let policies = match policy {
        Some(path) => crate::policy::load(path)?,
        None => default_policies()?,
    };
    let gate = Gate::with_policies(policies);
    let journal = layout.journal_path();
    // 🔴 **R12 / `req/242` H-01 (d)** — the writer's door refuses a journal that is not
    // there instead of making one.
    //
    // `req/242` measured the pair: `gx repair --yes` refuses to compose a lost journal and says
    // why, and one `gx submit` then created an empty one through `EngineJournal::open`'s
    // `create(true)` — after which `gx repair` answered `journal_absent: false`,
    // `journal_commits: 0`, `ledger_leaves: 2` and told a rollback story instead of the loss. Two
    // barriers, deliberately: this refusal, which carries the word and the remedy, and
    // `JournalCreation::Refused` in the anchor, which is what the engine would do if this line
    // were ever deleted. Every caller that reaches here has come through `Layout::create` (which
    // creates the journal for a directory that is not a project yet) or `Layout::open` (which
    // requires a declaration, so the project is established).
    // 🔴 **R13 / `req/244` L-04** — and the check comes **before** the directory is made.
    //
    // `create_dir_all(journal.parent())` used to sit in front of this refusal, so `gx submit` on a
    // project whose `.gx/ledger/` had been deleted answered rc 1 `JOURNAL_ABSENT` and left an empty
    // `.gx/ledger/` behind it: absent before the run, present after it. A road that refuses is a
    // road that writes nothing, which is the sentence the whole of `req/242` H-01 is about, and the
    // census does not count `create_dir_all` as a write primitive so nothing saw it.
    //
    // The directory is not needed here. Every caller has come through `Layout::create`, whose
    // `Shape::Dir` loop makes `.gx/ledger/` and whose init road creates the journal inside it, or
    // through `Layout::open`, which requires a declaration and therefore an established project.
    // A journal that is not there is the refusal below in both cases.
    // 🔴 **R40 / `req/38` §328 ruling 2 ①** — the writer's door asks the one predicate too.
    //
    // `!journal.exists()` folded "not there" and "this process may not look" into the same refusal,
    // and the refusal it produced says **"is not there"** in its title. Audit 39's own measurement
    // of the write road turned out to be a `cp -a` artefact rather than this fold (`req/558` §2),
    // but the fold was real one directory up: with `.gx/ledger/` unreadable, this door refused
    // `JOURNAL_ABSENT` about a journal holding 1,798 bytes. Now only `Absent` wears that word;
    // anything else this door cannot resolve is a refusal that names the operating system's reason.
    match crate::layout::presence_of(&journal) {
        crate::layout::Presence::Absent => return Err(journal_absent(&journal)),
        crate::layout::Presence::Undetermined(source) => {
            return Err(Error::Io {
                action: "read the shape of",
                path: journal.display().to_string(),
                source,
            })
        }
        crate::layout::Presence::Present(_) => {}
    }
    let anchor = anchor_accepting(layout, accept)?;
    // 🔴 **`req/493` §1 AC-6** — what the kernel is holding this process to, declared at the
    // writer's door.
    //
    // Here for the reason the receipt archive two paragraphs down is here: **every** engine in this
    // binary that can reach T-11 comes through this function, so this is the one line that makes
    // "the commit says what confined it" true of every road rather than of the roads someone
    // remembered. `gx-engine` reads no environment variable of its own — the fact is a caller's to
    // state, and this is the caller.
    //
    // A `GX_CONFINEMENT` this build cannot read stops the run here, before a journal is touched.
    // See `crate::confine::read_declaration` for why the alternative (assume unconfined) is the
    // reading that puts an assumption inside a signature.
    #[cfg(feature = "confine")]
    let mut engine = Engine::open_anchored(&journal, gate, evidence, anchor)?
        .with_confinement(crate::confine::from_environment()?);
    // 🔴 **`cfg(not(feature = "confine"))`** (`req/817`) — the public distribution is built without
    // `gx-confine` (`req/789` §3 holds it private), so this binary has no `gx confine` and no reader
    // for `GX_CONFINEMENT`.
    //
    // It refuses instead of falling through to the engine's `unconfined()` default, for exactly the
    // reason `crate::confine::read_declaration` refuses an unreadable value: something set that
    // variable, and recording "no confinement" because this build cannot read it would put that
    // assumption inside a signature (`req/493` §1 AC-6). A build that cannot honour the declaration
    // says so; it does not quietly issue receipts that claim the process was unconfined.
    //
    // With the variable unset there is nothing to honour, and the engine's own default
    // (`ConfinementContext::unconfined()`) is the same value the confined build would compute.
    #[cfg(not(feature = "confine"))]
    let mut engine = {
        if let Some(raw) = std::env::var_os("GX_CONFINEMENT") {
            return Err(Error::Usage {
                detail: format!(
                    "`GX_CONFINEMENT` is set to {raw:?}, but this build carries no confinement \
                     reader: it was compiled without the `confine` feature, so `gx confine` and \
                     the `GX_CONFINEMENT` grammar are both absent. gx refuses rather than \
                     assuming the process is unconfined -- a value it cannot read was set by \
                     something, and reading it as \"no confinement\" would put that assumption \
                     inside a signature (`req/493` §1 AC-6). Build with `--features confine` to \
                     honour it"
                ),
            });
        }
        Engine::open_anchored(&journal, gate, evidence, anchor)?
    };
    // 🔴 The pair, in two lines: the set above and the registry here (req/38 §60's R-9 paired ruling; sem: SEM-gx-cli-230). The
    // engine still declares no adapter — N-13 is about `gx-engine`'s manifest and is unmoved; this
    // is the caller putting adapters in its registry, which req/88 §1 ruled is the CLI's job.
    let _registered = register_default_adapters_with(&mut engine, mcp);
    // 🔴 **R8 / `req/234` H-01** — the receipt archive, registered at the **writer's** door.
    //
    // Every engine in this binary that can reach T-11 comes through this function
    // ([`open_engine`], [`open_engine_wired`], `gx serve`, `gx repair --yes`, `Session::open`), and
    // registering here is what moves `.gx/receipts/` from "the caller writes it afterwards" to
    // "the commit is not finished until it is filed". [`open_engine_read_only`] deliberately does
    // **not** register one: a reader that filed a receipt would be a read that writes, which is
    // the shape `req/190` §5-2 refuses for the reaper.
    engine.register_receipt_sink(std::sync::Arc::new(crate::receipt::ArchiveSink::in_layout(
        layout,
    )));
    if let Some(mode) = mode {
        engine = engine.with_mode(mode);
    }
    Ok(engine.with_posture(posture))
}

/// 🔴 **R8 / `req/234` H-01 (b)** — file every receipt a recovery re-issued.
///
/// One function so that the three roads that call `Engine::recover` outside a [`Session`]
/// (`gx serve`'s start-up, `gx repair --yes`) write the same thing in the same words. See
/// [`Session::recover`] for why a failure here is a sentence and not a refusal.
pub fn file_recovered_receipts(layout: &Layout, recovered: &[gx_engine::pipeline::Recovered]) {
    let store = crate::receipt::ReceiptStore::in_layout(layout);
    for row in recovered {
        let Some(receipt) = &row.receipt else {
            continue;
        };
        if let Err(e) = store.put(
            &row.transformation,
            crate::receipt::StoredKind::Commit,
            receipt,
        ) {
            crate::note!(
                "gx: 43 §7-3b re-issued the commit receipt for {} and `.gx/receipts/` would not \
                 take it: {e}. The commit stands — the journal and the ledger both witness it — \
                 and until the receipt is filed this row cannot be undone and cannot be proved to \
                 a third party. `gx repair` counts it as `receipts_missing` (req/234 H-01)",
                row.transformation.0.to_text()
            );
        }
    }
}

/// 🔴 **DR-43-2 / `req/38` §148** — the writer lock's name inside `.gx/`.
///
/// # 🔴 ~~It is not in `GX_PATHS`, and that is owed rather than hidden~~ — paid (`req/38` §156
/// ruling 3, DR-43-5 (2))
///
/// req/56 §2's table, [`crate::layout::GX_PATHS`] and
/// `probes/doubt/tests/m6_surface_doubt.rs::the_dotgx_layout_is_req56_exactly` are one list checked
/// from three sides, and the probe is red the moment any one of them gains a row the other two do
/// not have. ~~Two of the three are the specification and a probe outside this lane's write scope, so
/// the row is raised as part of **DR-43-5** and the file is created without being declared. What
/// that costs, stated: `gx doctor`'s layout recovery does not know about `LOCK`, so it neither
/// reports it nor repairs it.~~ All three moved together in lane R2: `LOCK` is req/56 §2's ninth
/// row, `Nature::Transient`, and `Layout::recover` answers `Recovery::Untouched` about it. The
/// struck words are kept because a reader who trusted them would go looking for a gap that is
/// closed.
///
/// What is unchanged: nothing reads the file for meaning — it carries a pid and a verb for a
/// human, and the exclusion is the operating system's. `Layout::create` therefore still does **not**
/// make one, which is what lets an operator read the file's presence as "a `gx` is writing".
pub const LOCK_FILE: &str = "LOCK";

/// 🔴 **R4 / `req/222` M-04** — say what opening the project repaired, if it repaired anything.
///
/// The same sentence `gx serve`'s start-up prints, at the door every single-shot verb comes
/// through. One function rather than a line in `open_wired_with_posture` so that the two faces
/// cannot drift into two accounts of the same event, and because the byte counts and the
/// quarantine paths are read off the same two `Recovery` values in both places.
fn announce_quarantine<E: gx_engine::EvidenceSource>(engine: &Engine<E>) {
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the diagnosis that must come **first**,
    // because everything below it is a sentence about a crash and this is not one.
    //
    // The twenty-ninth audit pointed a pre-R29 binary at a journal a post-R29 binary had written.
    // The older binary could not decode the newer record, reported the rest of the file as a torn
    // tail, cut it, and printed the sentence below — which names `gx repair` as the remedy, and
    // `gx repair`'s own documentation says that what it cannot do is put those records back. The
    // operator was given a confident diagnosis of the wrong illness and a treatment that does not
    // touch the right one.
    //
    // This build cannot repair the binaries that behaved that way; nothing written here reaches
    // them, and `CHANGELOG.md` §3 says so rather than implying otherwise. What it can do is not be
    // one of them the next time the vocabulary grows — so when *this* build is the older one, it
    // says which illness it has.
    if engine.journal().from_a_newer_gx() {
        crate::note!(
            "gx: this project's journal begins with a format marker this build does not know, so \
             its records were written by a **newer gx** — the bytes decode as records of a newer \
             vocabulary, not as a crash. **Nothing was removed and nothing was copied**: the file \
             is exactly as the newer binary left it. `gx repair` is **not** the remedy here and \
             would not help; use the newer gx, or upgrade this one"
        );
        return;
    }
    let torn =
        engine.journal().recovery().torn_tail_bytes + engine.ledger().recovery().torn_tail_bytes;
    if torn == 0 {
        return;
    }
    let mut where_to = Vec::new();
    if let Some(path) = engine.journal().quarantined() {
        where_to.push(path.display().to_string());
    }
    if let Some(path) = engine.ledger().quarantined() {
        where_to.push(path.display().to_string());
    }
    // 🔴 **R5 / `req/227` H-01** — say what happened, and not what usually happens.
    //
    // This sentence used to name a quarantine path unconditionally, so a project whose journal had
    // been **rewritten** — where DR-43-9 deliberately removes nothing, because everything after a
    // chain break is whole — was told that `gx` had "removed" bytes and "copied them" somewhere.
    // Measured on this lane's own probe. A verb that reports a repair it did not perform is the
    // same failure as a report that performs one.
    if where_to.is_empty() {
        crate::note!(
            "gx: {torn} byte(s) of this project would not replay and were **left exactly where \
             they lie** — nothing was removed and nothing was copied (DR-43-9: bytes after a \
             chain break are whole records, and cutting there would delete what nobody asked to \
             lose). Run `gx repair` before anything else"
        );
    } else {
        crate::note!(
            "gx: opening this project removed {torn} byte(s) that would not replay; they were \
             copied to {} first (DR-43-7, req/222 M-04). The verb below ran on the log that \
             replayed",
            where_to.join(", ")
        );
    }
}

/// Take the lock, read to the end of the log, and refuse a journal and a ledger that disagree.
///
/// The three steps `req/38` §148 puts in front of every write, in one place so that `Session::open`
/// and `gx wrap`'s per-call re-entry cannot drift apart.
///
/// # Errors
/// [`crate::Error::Engine`] carrying `gx_engine::Error::Busy` if another `gx` holds the lock, or the
/// engine's own refusal from the catch-up, or [`crate::Error::Malformed`] when the frontier and the
/// tree disagree.
fn lock_and_settle(
    lock: &Arc<ProcessLock>,
    engine: &mut Engine<InjectedEvidence>,
    holder: &str,
) -> Result<Option<OwnedLock>> {
    let held = lock.acquire_owned(holder)?;
    settle(engine)?;
    Ok(Some(held))
}

/// The catch-up and the `ledger_agrees` gate, with the lock already held.
///
/// # Errors
/// As [`lock_and_settle`].
fn settle(engine: &mut Engine<InjectedEvidence>) -> Result<()> {
    let caught = engine.catch_up()?;
    // 🔴 **R4 / `req/225` H-03** — the same gate, now also `false` for a journal somebody rewrote
    // underneath this process. `Engine::ledger_agrees` folds `Engine::journal_intact` in, so no
    // writer has to be told about the second file twice; what the CLI owes is the same thing the
    // server owes, which is saying **which** file moved.
    if !engine.ledger_agrees() {
        // 🔴 **R6 / DR-43-11** — see `gx_api::handlers::healthz` for why the two clauses are joined
        // here rather than inside the outer format's arguments.
        // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated.
        let note = gx_api::journal_and_head_note(engine.journal_departure(), engine.rolled_back());
        return Err(crate::Error::Malformed {
            what: "project",
            path: engine.journal().path().display().to_string(),
            detail: format!(
                "the journal witnesses {} commit(s) and the ledger holds {} leaf/leaves, and                  `ledger_agrees` is false: the two files are describing different trees.{} Writing to                  a disagreement makes it worse, so this verb refuses instead (req/182 M-12/H-01,                  req/38 §148). `gx repair` reports what is wrong and `gx repair --yes` runs 43 §7's                  recovery under the project lock (DR-43-8). {} record(s) had been appended by another                  process since this journal was opened",
                engine.sigma().ledger().len(),
                engine.ledger().log().len(),
                note,
                caught.records,
            ),
        });
    }
    Ok(())
}

/// An engine bound to one project's `.gx/`, with the fs adapter registered.
pub struct Session {
    engine: Engine<InjectedEvidence>,
    layout: Layout,
    /// 🔴 **T6 condition ② (single writer, per operation)** — this project's `.gx/LOCK`
    /// (`req/38` §148 ruling 1(ii)).
    lock: Arc<ProcessLock>,
    /// The lock, held. `None` between [`Session::release_writer_lock`] and
    /// [`Session::hold_writer_lock`] — which is `gx wrap`'s whole shape (see those two).
    held: Option<OwnedLock>,
}

impl Session {
    /// Open the project's `.gx/` and build an engine over it.
    ///
    /// `create` is `true` for `gx submit` alone. Every other verb **opens**, so that a mistyped
    /// directory answers 44 §1.4's 6 rather than quietly starting a second, empty ledger beside the
    /// operator's real one — "cannot be read" and "does not exist" again (E-M4-35; sem: SEM-gx-cli-231), one directory up.
    ///
    /// # Rule 1 (ii) in the signature (sem: SEM-gx-cli-232)
    ///
    /// The [`Gate`] is built from gx-gate's own embedded pack (`packs::fs_pack`) and handed to the
    /// engine. This crate does not evaluate it, does not read its decision and never names a
    /// `Verdict` — 41 §4 keeps judging in one function, and what the CLI does is choose which policy
    /// set the engine judges *with*. FR-028's "there is exactly one road by which a pack's file gets
    /// embedded into the build" (sem: SEM-gx-cli-233) is
    /// why the bytes come from gx-gate rather than from a file this binary reads.
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/` cannot be created or read, [`Error::Layout`] for a newer directory,
    /// [`Error::Engine`] if the journal, blob store or ledger will not open, and
    /// [`Error::Gate`] if the shipped policy pack will not parse.
    pub fn open(
        project: &Path,
        create: bool,
        evidence: Vec<gx_witness::Evidence>,
        mode: Option<EnforcementMode>,
    ) -> Result<Self> {
        Self::open_with_policy(project, create, evidence, mode, None)
    }

    /// The same, deciding with a pack read off disk instead of the shipped one (**E-M6-12**).
    ///
    /// 44 §1.2 gives `gx verify` no `--policy`, and req/38 §50 M6H3-9 adopted (a) adds one: "a
    /// **test-use policy pack fixture** that denies a writable path, plus `gx verify --policy
    /// <PATH>` (an extension of 44)" (sem: SEM-gx-cli-234). The reason
    /// is DR-2's other half. The shipped pack's one forbid is `/etc`, so the only route a `gx`
    /// invocation had to a `Verdict::Deny` ran through a real path under `/etc` — and a **record-only**
    /// commit of a denied change proceeds to `apply`, which on that locator is a write to `/etc`.
    /// Hand 3 measured T-8r inside the engine for that reason and raised the gap; this is the flag
    /// that closes it, and what it buys is that DR-2's "the apply goes through, but
    /// `enforced=false`" (sem: SEM-gx-cli-235) is now
    /// measurable **through the binary** on a file in a temporary directory.
    ///
    /// A separate function rather than a fifth parameter on [`Session::open`]: six call sites pass
    /// nothing, and M5H5-1 refused a five-argument `Engine::open` on the grounds that a parameter
    /// every caller ignores is a parameter every caller has to read.
    ///
    /// # Errors
    /// As [`Session::open`], plus [`crate::Error::Io`] / [`crate::Error::Gate`] if the named pack
    /// cannot be read or does not parse.
    pub fn open_with_policy(
        project: &Path,
        create: bool,
        evidence: Vec<gx_witness::Evidence>,
        mode: Option<EnforcementMode>,
        policy: Option<&Path>,
    ) -> Result<Self> {
        Self::open_wired(
            project,
            create,
            evidence,
            mode,
            policy,
            &McpWiring::default(),
        )
    }

    /// 🔴 The same, with an MCP server named (**P3**, `req/119` §2.5).
    ///
    /// `gx wrap` opens this way, and so does any single-shot verb given `--mcp-server` — `gx undo`
    /// above all, because 43 §5's inverse of a tool call **is a call**, and a process holding no
    /// transport can plan it and not perform it.
    ///
    /// `FailClosed`, same as [`Session::open`]'s reasoning: every one of this function's callers is
    /// a single-shot verb with no `--fail-posture` of its own. [`Session::open_wired_with_posture`]
    /// is the one road that is not (**P3.1**, gotcha66).
    ///
    /// # Errors
    /// As [`Session::open_with_policy`].
    pub fn open_wired(
        project: &Path,
        create: bool,
        evidence: Vec<gx_witness::Evidence>,
        mode: Option<EnforcementMode>,
        policy: Option<&Path>,
        mcp: &McpWiring,
    ) -> Result<Self> {
        Self::open_wired_with_posture(
            project,
            create,
            evidence,
            mode,
            gx_core::FailPosture::FailClosed,
            policy,
            mcp,
        )
    }

    /// 🔴 [`Session::open_wired`], with the posture **not** hard-coded (**P3.1**, gotcha66;
    /// `req/38` §74 item③, `req/125` §4-3).
    ///
    /// `gx wrap` is a long-lived verb over one MCP server, the same shape `gx serve` is over one
    /// HTTP surface, and 44 §1.2 gives `gx serve` a `--fail-posture` for exactly that reason.
    /// Before this, `wrap.rs::run` had no road to anything but [`Session::open_wired`]'s
    /// `FailClosed`, and the `Wrap` command's own flags carried no `--fail-posture` to put a
    /// different value on. This constructor is `gx wrap`'s road to the same flag `gx serve`
    /// already had, and [`Session::open_wired`] is now a caller of it rather than the only road in.
    ///
    /// # Errors
    /// As [`Session::open_with_policy`].
    pub fn open_wired_with_posture(
        project: &Path,
        create: bool,
        evidence: Vec<gx_witness::Evidence>,
        mode: Option<EnforcementMode>,
        posture: gx_core::FailPosture,
        policy: Option<&Path>,
        mcp: &McpWiring,
    ) -> Result<Self> {
        let layout = if create {
            Layout::create(project)?
        } else {
            Layout::open(project)?
        };
        // 🔴 **R10 / `req/238` H-01** — the writer's door asks for the settings file, and asks for
        // it **here**, which is the one door every single-shot write verb comes through.
        //
        // `req/238` H-01's third arm: `.gx/config.toml` deleted, `gx submit` run, rc **0**, and the
        // file back to the two shipped comments — so `engine_signing_keyid`, which 43 §7.9 (b)
        // calls the setting that decides which key a recovery signs with, was gone and the next
        // `gx repair --yes` asked for a key the project used to name. `Layout::create` no longer
        // writes it back (that is the silence being closed); this is the other half, for the verbs
        // that open rather than create. Read verbs do not come through here and `gx repair`'s
        // report mode does not either — `req/227` M-03.
        layout.require_config()?;
        let lock = Arc::new(ProcessLock::open(layout.join(LOCK_FILE))?);
        let mut engine = match open_engine_wired(
            &layout,
            InjectedEvidence::new(evidence),
            mode,
            posture,
            policy,
            mcp,
        ) {
            Ok(engine) => engine,
            // 🔴 **DR-B / `req/38` §337, `req/565` §3 — one condition, one word, on the write
            // road too.** `req/38` §156 ruling 2(a) is "one condition gets one word", and
            // `crates/gx-cli/tests/r40_journal_presence.rs::c6` measures the write road on the
            // same project the read road (`crate::ledger::refuse_if_the_two_files_disagree`) just
            // answered. `path` is compared against `layout.journal_path()` so this stays scoped to
            // the journal specifically — an `Io` failure on the ledger or the blob store is a
            // different fact and keeps folding to `INTERNAL` unminted, same as it did before this
            // ruling.
            Err(e) => {
                if let Error::Engine(gx_engine::Error::Io {
                    kind, path, detail, ..
                }) = &e
                {
                    if *kind != std::io::ErrorKind::NotFound && *path == layout.journal_path() {
                        return Err(crate::ledger::journal_unreadable(path, *kind, detail));
                    }
                }
                return Err(e);
            }
        };
        // 🔴 **R4 / `req/222` M-04 (still real in `req/225` §1-4)** — a repair the CLI performed is
        // a repair the CLI says out loud.
        //
        // `Engine::open` is the writer's door: it quarantines a tail that will not replay and then
        // cuts it (DR-43-7). `gx serve` has printed a line about that since R1; every single-shot
        // verb came through the same door in silence. `req/222` M-04 measured it and `req/225`
        // found it unchanged — 22 bytes appended to a journal, `gx submit`, **rc 0 and an empty
        // stderr**, and a `journal.torn.1636-1658` nobody was told about. A silent repair is
        // indistinguishable from data that was never written, which is the whole reason DR-43-7
        // copies before it cuts.
        //
        // stderr and not stdout: 44 §1.3 gives stdout to the verb's single JSON object, and this
        // is a fact about the project rather than about the verb's result. The exit status is
        // unchanged — the operation really did succeed, on the log that really is there.
        announce_quarantine(&engine);
        // 🔴 **T6 conditions ② and ③, at the one door every verb comes through** (`req/38` §148).
        //
        // A single-shot `gx` verb is one operation and this session is its whole life, so taking the
        // lock here is taking it "per operation" — the phrase `req/190` §3-1 uses to distinguish the
        // adopted (a') from the rejected (a). `gx wrap` is the exception the distinction exists for:
        // it is a long-lived session over many tool calls and it releases the lock immediately
        // (`wrap.rs`), taking it again around each call.
        //
        // Then two things, in order:
        //
        // 1. **catch up** — read whatever another process appended since this journal was opened.
        //    Nothing arrives in the ordinary single-shot case (we opened the file a microsecond
        //    ago); what this closes is the window between `Engine::open` and the lock, and it is the
        //    same call `gx wrap` makes before every later tool call, when the window is minutes long.
        // 2. **ask `ledger_agrees`** — `req/182` M-12. The journal-witnessed frontier and the
        //    ledger's own tree must be the same tree, and `Engine::open` now rebuilds the frontier
        //    from the journal so the question has an answer before `recover` is called. `false` means
        //    the two files are telling different stories (`req/182` H-01's truncation, or H-08's
        //    mid-file damage), and appending to a disagreement makes it worse — so the verb refuses
        //    instead, with the numbers to say it with.
        let held = lock_and_settle(&lock, &mut engine, "gx")?;
        Ok(Self {
            engine,
            layout,
            lock,
            held,
        })
    }

    /// 🔴 Release the writer lock, keeping the engine (`gx wrap`'s first act).
    ///
    /// `req/190` F-7: `gx wrap` holds an engine for the life of an MCP session, exactly as `gx serve`
    /// holds one for the life of a server. A lock held for that whole life would make the GUI premise
    /// — the GUI runs `gx serve`, the agent runs `gx wrap` — structurally impossible, which is why
    /// `req/38` §148 refused the process-lifetime lock and adopted the per-operation one. This is
    /// the release half of "per operation"; [`Session::hold_writer_lock`] is the other.
    pub fn release_writer_lock(&mut self) {
        self.held = None;
    }

    /// 🔴 Take the writer lock again and catch up (`gx wrap`, once per tool call).
    ///
    /// Idempotent: a session that already holds it catches up and returns. The catch-up is not
    /// optional and not an optimisation — between two tool calls minutes can pass, and everything a
    /// `gx serve` or a second CLI wrote in them is read here, before this session writes anything.
    ///
    /// # Errors
    /// [`crate::Error::Engine`] carrying `Busy` if another process holds the lock, or the engine's
    /// own refusal if the catch-up cannot read the journal or the ledger, or if `ledger_agrees` is
    /// false.
    pub fn hold_writer_lock(&mut self) -> Result<()> {
        if self.held.is_none() {
            self.held = lock_and_settle(&self.lock, &mut self.engine, "gx wrap")?;
        } else {
            settle(&mut self.engine)?;
        }
        Ok(())
    }

    /// 🔴 **DR-43-4, the entry sweep** — 43 T-6, once, at the top of a write verb (`req/38` §148
    /// ruling 1(iv): "the reaper is a `gx serve` timer **and** one sweep at every write entry").
    ///
    /// The half of the ruling a single-shot CLI can carry. `gx serve` has a clock and a timer; a
    /// `gx commit` has neither and lives for one operation — but it *is* a writer, it already holds
    /// `.gx/LOCK` and it has already caught up, so the cheapest honest place for INV-L1/INV-L2 to
    /// be enforced in a CLI-only project is here. A project driven entirely from the terminal now
    /// expires a stale `Candidate` on the next thing anybody does to it, instead of never.
    ///
    /// # 🔴 Why this is not in [`Session::open`]
    ///
    /// `Session::open` is also the road a **read** takes (`gx receipt verify
    /// --recount-from-journal`), and a read that aborted a transformation would be the shape
    /// `req/190` §5-2 refused for the reaper in as many words — "a read that writes breaks
    /// idempotence and ETags". So the sweep is a call, and the callers are the seven verbs of 44
    /// §1.2 that write: `submit`, `plan`, `verify`, `commit`, `undo`, `cancel`, `escalation`.
    ///
    /// Answers what it expired, so that a verb can say so on stderr rather than leave an operator
    /// to discover that something aborted while they were doing something else.
    ///
    /// # Errors
    /// [`crate::Error::Engine`] from the journal, as [`gx_engine::pipeline::Engine::reap`] raises
    /// it. A sweep that cannot write stops the verb: it means the journal is unwritable, which the
    /// verb was about to discover anyway.
    pub fn sweep(&mut self, at: gx_core::Timestamp) -> Result<Vec<TransformationId>> {
        let expired = self.engine.reap(at)?;
        if !expired.is_empty() {
            let ids: Vec<String> = expired.iter().map(|id| id.0.to_text()).collect();
            crate::note!(
                "gx: 43 T-6 expired {} transformation(s) whose deadline had passed before this \
                 command ran (DR-43-4, req/38 §148): {}",
                ids.len(),
                ids.join(" ")
            );
        }
        Ok(expired)
    }

    /// 43 §7's recovery, for a caller that has the signing key (**M5H5-1**).
    ///
    /// The road `req/38` §148 asks every write verb to walk. It is a call and not part of
    /// [`Session::open`] for the reason `Engine::recover` states: the key and the adapters arrive
    /// after the engine exists, and a read-only verb (`gx verdict-checkpoint list`, `gx receipt
    /// verify --recount-from-journal`) opens the same `Session` — a recovery fired unasked there
    /// would make a read write, which is exactly what `req/190` §5-2 refuses for the reaper.
    ///
    /// # Errors
    /// As [`gx_engine::pipeline::Engine::recover`].
    /// 🔴 **R8 / `req/234` H-01 (b)** — and what the recovery re-issued is **filed**.
    ///
    /// 43 §7-3b asks the recovery to "re-issue the receipt from the existing `InclusionProof` (if
    /// not yet issued)"; the engine has done that since M5 and put the document on
    /// `Recovered::receipt`. `req/234` H-01 read the five call sites and found that all five drop
    /// it — they read `row.path` and `row.refusal` and nothing else — so the re-issue existed and
    /// left no trace on the disk.
    ///
    /// Since R8 the **engine** files it through [`crate::receipt::ArchiveSink`], which is the road
    /// that closes the window for every caller at once. This loop is the second belt and is kept
    /// deliberately: an engine constructed without a sink (a test, an embedder, `open_engine_read_only`
    /// promoted by a future caller) still has a writer here, and a `put` of a receipt that is
    /// already on the disk writes the same bytes. Failing to file is **not** fatal on this road —
    /// the commit it belongs to was finished by a previous process and refusing now would take a
    /// project offline over a document that can be re-issued again — so it is reported on stderr,
    /// which is where `gx repair`'s `receipts_missing` picks the fact up.
    /// 🔴 **R35 / `req/470` H-01 — `verb` is not decoration, and this is where the repair lives.**
    ///
    /// Audit 34 read the five call sites of this road and found the same thing `req/234` H-01
    /// found about the receipt: they take the answer and drop it. `gx verify`, `gx commit` and
    /// `gx undo` each wrote `session.recover(at, &key)?;` and went on, so a recovery that had just
    /// written a delta over whatever a third party had put in the file went by in silence — `rc 0`
    /// for `gx verify`, and **0 bytes on stderr** for three of the four.
    ///
    /// The announcement is therefore made **here**, inside the call the write verbs already make,
    /// rather than at each of them. `verb` is what the sentence will call itself, so `gx verify`
    /// does not announce itself as `gx serve` (the old string was prefixed `"gx serve: "` in the
    /// format literal). The cost of the parameter is the point: a new write verb cannot reach 43
    /// §7's recovery without saying which verb it is, and having said so it is loud for free.
    pub fn recover(
        &mut self,
        at: gx_core::Timestamp,
        key: &KeyPair,
        verb: &str,
    ) -> Result<Vec<gx_engine::pipeline::Recovered>> {
        // 🔴 **R36 / `req/476` H-01** — the `?` that used to be on this line took two facts with
        // it: the rows this recovery had already finished, and the row whose delta it had **just
        // written** to somebody's substrate. Audit 35 measured all four verbs that pass through
        // here going silent over a file whose contents they had replaced. The error still
        // propagates exactly as before; what changes is that it no longer leaves without saying.
        let recovered = match self.engine.recover(at, key) {
            Ok(recovered) => recovered,
            Err(why) => {
                crate::recovery::announce_interrupted_recovery(verb, &self.engine);
                return Err(why.into());
            }
        };
        file_recovered_receipts(&self.layout, &recovered);
        crate::recovery::announce_recovery(verb, &recovered);
        Ok(recovered)
    }

    /// The engine, for a caller that is about to drive one of 43's transitions.
    pub fn engine(&mut self) -> &mut Engine<InjectedEvidence> {
        &mut self.engine
    }

    /// The engine, read-only.
    #[must_use]
    pub fn read(&self) -> &Engine<InjectedEvidence> {
        &self.engine
    }

    /// The `.gx/` this session is bound to.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The draft store beside it.
    #[must_use]
    pub fn drafts(&self) -> DraftStore {
        DraftStore::in_layout(&self.layout)
    }

    /// Remember that an intent resolved to a transformation (**M6-02 adopted (b)**'s cache; sem: SEM-gx-cli-236).
    ///
    /// Best effort by construction: req/56 §2 declares `.gx/index/` "derived, declared safe to
    /// delete" (sem: SEM-gx-cli-237), so
    /// a cache that could not be written is not a reason to fail a command that succeeded. The
    /// engine's own `resolved` map is the authority and is rebuilt from the journal.
    pub fn remember(&self, intent: IntentId, transformation: TransformationId) {
        let (mut index, _) = ResolutionIndex::load(&self.layout);
        index.learn(intent, transformation);
        let _ = index.store(&self.layout);
    }

    /// The `IntentId` a transformation was planned from, without opening a body.
    ///
    /// Two roads, in this order:
    ///
    /// 1. `.gx/index/`, the cache hand 1 built (**M6-02 adopted (b)**; sem: SEM-gx-cli-238);
    /// 2. the **file names** of `.gx/drafts/`, each of which *is* an `IntentId` in 42 §1.2's text
    ///    form, checked against `Engine::resolved` (**M6-02 adopted (a)**; sem: SEM-gx-cli-239).
    ///
    /// The second road is what makes the first disposable, which req/56 §2 promises it is. Note
    /// what neither road does: **compute** anything. A name is parsed (`Cid::from_text`, in gx-core)
    /// and the engine is asked; Rule 1 (i) survives a lookup because a lookup is not a mint (sem: SEM-gx-cli-240).
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/drafts/` cannot be listed.
    pub fn intent_of(&self, transformation: &TransformationId) -> Result<Option<IntentId>> {
        let (index, _) = ResolutionIndex::load(&self.layout);
        let dir = self.layout.join("drafts");
        let mut candidates: Vec<IntentId> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(std::result::Result::ok) {
                let path = entry.path();
                if !path.extension().is_some_and(|x| x == "json") {
                    continue;
                }
                let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                    continue;
                };
                if let Ok(cid) = gx_core::Cid::from_text(&stem.replace('_', ":")) {
                    candidates.push(IntentId(cid));
                }
            }
        }
        // The cache first, and only when it agrees with the engine. A cache consulted *instead of*
        // the authority is the shape hand 1's `index.rs` refused to provide a helper for.
        for id in candidates
            .iter()
            .copied()
            .filter(|id| index.get(id).as_ref() == Some(transformation))
            .chain(candidates.iter().copied())
        {
            if self.engine.resolved(&id).as_ref() == Some(transformation) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Read one draft back.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::Malformed`] from the store.
    pub fn draft(&self, intent: &IntentId) -> Result<Option<Intent>> {
        self.drafts().get(intent)
    }

    /// 🔴 What the **journal** says about a transformation, without holding a row.
    ///
    /// `Engine::open` leaves the in-flight table empty (M5H3-5) and the journal holds every
    /// transition, so this is Σ's answer to "where is this transformation" (sem: SEM-gx-cli-241) in a process that has
    /// planned nothing. It is `reconstruct` — **E-M5-2**'s read-only Σ rebuild — and it opens no
    /// substrate.
    ///
    /// # 🔴 Why every verb asks this **before** [`Session::resume`]
    ///
    /// A resume re-plans, and a re-plan reads the world. After a commit the world **has moved** —
    /// by that commit — so a transformation that reached `Committed` can never be re-planned into
    /// itself again. Resuming first would therefore turn 44 §1.2's "retrying the same execution is
    /// naturally idempotent" (sem: SEM-gx-cli-242)
    /// into a refusal, and the refusal would be about the substrate rather than about the request.
    ///
    /// A terminal row does not need a body: 43 T-9's idempotency column answers a re-entrant commit
    /// with the state it already reached, and the receipt for it is in `.gx/receipts/`. So the rule
    /// is: **ask the journal first, and only rebuild a row when there is still work to do on it.**
    pub fn recorded(&self, id: &TransformationId) -> Option<StateRow> {
        reconstruct(self.engine.journal().records())
            .state_of(id)
            .cloned()
    }

    /// 🔴 Put a `Candidate` (or anything past it) back in the engine's table.
    ///
    /// The sequence 44 §0 describes, with the authority in the third step: find the intent, read the
    /// body, ask the engine to plan it again. What comes back is the engine's answer about identity,
    /// and this function's only judgement is to **refuse a different one**.
    ///
    /// # Errors
    /// [`Error::NotFound`] if nothing here has ever planned that transformation, or if the draft it
    /// came from is gone — which is the one durable consequence of `.gx/drafts/` being
    /// `Nature::Source` (req/56 §2: "is lost"; sem: SEM-gx-cli-243). [`Error::Usage`] if the world has moved and the
    /// plan now names a different transformation. Anything the engine refuses, unchanged.
    pub fn resume(&mut self, id: &TransformationId, at: Timestamp) -> Result<()> {
        if self.engine.transformation(id).is_some() {
            return Ok(());
        }
        let intent_id = self.intent_of(id)?.ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: id.0.to_text(),
        })?;
        let intent = self.draft(&intent_id)?.ok_or_else(|| Error::NotFound {
            what: "draft for transformation",
            id: id.0.to_text(),
        })?;
        let planned = self.engine.plan(&intent, at)?;
        if planned == *id {
            return Ok(());
        }
        Err(Error::Usage {
            detail: format!(
                "{} was planned against a state of the substrate that no longer holds; planning it \
                 now names {} instead. 43 §8: \"`Fingerprint₀` has gone stale, so a re-`plan()` \
                 (re-fingerprint) is forced\" (sem: SEM-gx-cli-244) — run `gx plan` again and verify the transformation that \
                 comes back",
                id.0.to_text(),
                planned.0.to_text()
            ),
        })
    }

    /// 🔴 Put a **`Committed`** row back — the state [`Session::resume`] cannot reach.
    ///
    /// `resume` re-plans, a re-plan reads the substrate, and a commit has moved it, so a committed
    /// transformation can never be planned into itself again (the module header says as much, and
    /// [`crate::pipeline::commit`] relies on it). `gx undo <TID>` is nonetheless an operation on a
    /// committed transformation (43 §5), so this is the road in:
    /// [`gx_engine::Engine::rehydrate_committed`] rebuilds the row from Σ, the blob store and the
    /// `Intent` this project is still holding in `.gx/drafts/` — and re-identifies the result, so
    /// a draft that is not the one the transformation was planned from is refused rather than
    /// undone.
    ///
    /// 🔴 **The draft is therefore load-bearing after the commit.** req/56 §2 files `.gx/drafts/`
    /// as `Nature::Source` with "is lost" (sem: SEM-gx-cli-245) in its recovery column, and this is the second thing
    /// that is lost with it: hand 1 wrote that a missing draft costs a `gx plan`, and it costs a
    /// `gx undo` as well. Raised as **M6H4-5**.
    ///
    /// # Errors
    /// [`Error::NotFound`] if Σ has never heard of the transformation, if it is not committed, or
    /// if the draft it came from is gone. Anything the engine refuses, unchanged.
    pub fn rehydrate_committed(&mut self, id: &TransformationId) -> Result<()> {
        if self.engine.transformation(id).is_some() {
            return Ok(());
        }
        let intent_id = self.intent_of(id)?.ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: id.0.to_text(),
        })?;
        let intent = self.draft(&intent_id)?.ok_or_else(|| Error::NotFound {
            what: "draft for transformation",
            id: id.0.to_text(),
        })?;
        if self.engine.rehydrate_committed(id, &intent)? {
            return Ok(());
        }
        Err(Error::NotFound {
            what: "committed transformation",
            id: id.0.to_text(),
        })
    }

    /// 🔴 The key a receipt is signed with: the one the transformation's actor names.
    ///
    /// See the module documentation for the separation this does not implement (**M6H3-4**). The
    /// refusal when the key is missing names the command that makes one, because an operator who
    /// reaches `gx verify` without a key has done nothing wrong — 44 §1.2 never told them to.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the transformation is not in the table, [`Error::Witness`] if the key
    /// file is missing, unreadable, or has permissions `KeyPair::load` refuses (req/56 §3's 0600).
    pub fn signing_key(&self, id: &TransformationId) -> Result<KeyPair> {
        let actor = self
            .engine
            .transformation(id)
            .map(|t| t.actor.key().clone())
            .ok_or_else(|| Error::NotFound {
                what: "transformation",
                id: id.0.to_text(),
            })?;
        let store = KeyStore::user_default()?;
        store.load(&actor).map_err(|e| match e {
            Error::Witness(_) => Error::Usage {
                detail: format!(
                    "this transformation names actor key {actor:?} and req/56 §3's key store has no \
                     usable key under that id ({e}); `gx key gen` makes one"
                ),
            },
            other => other,
        })
    }
}
