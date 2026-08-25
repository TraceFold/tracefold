// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The boundary a deployment implements, and the two values it cannot build.
//!
//! 34 **AC-051** asks that "no bypass route technically exists" and ruling #10 (`req/38` §56) asks that the (sem: SEM-gx-adapter-mcp-208)
//! check be a **derivation** rather than a declared list. This module is where the derivation gets its
//! first and largest premise, and the premise is enforced by the compiler:
//!
//! > [`ToolCall`] and [`Admitted`] have private fields and `pub(crate)` constructors, and
//! > [`ToolTransport::call`] takes a reference to each. **No code outside this crate can build
//! > either.**
//!
//! The population that closes is every crate that is not this one -- including crates that do not
//! exist yet, which is the difference between a compiler and a scan. `tests/ac_051.rs` carries the two
//! `compile_fail` doctests that measure it rather than assert it, in the pair form **M4-20, adopted (b)** used (sem: SEM-gx-adapter-mcp-209)
//! for AC-046: a control that compiles beside each refusal, so that "it failed to compile" can only (sem: SEM-gx-adapter-mcp-210)
//! mean the thing under test.
//!
//! # 🔴 Reading is not calling
//!
//! [`ToolTransport::read`] takes no token. That is not an oversight and it is not a weakening: **E-M4-29**
//! (§30 M4H2-3, adopted (b)) reads 41 §4's "a pure function" for the whole boundary as "zero **writes** to the substrate" and (sem: SEM-gx-adapter-mcp-211)
//! adds "reads are not forbidden", and req/98 §6-7 spells the M7 version of the same rule -- "Rule 1's M7 version... (sem: SEM-gx-adapter-mcp-212)
//! is not 'zero I/O'". An adapter that could not read could not answer `snapshot` or `precondition`, and (sem: SEM-gx-adapter-mcp-213)
//! 51 §7 contract 1 and contract 3 both require it to.
//!
//! So the gate is on the **effect** and not on the observation, which is what 41 §4 asks for in as many
//! words ("`apply` is only called after commit approval" -- and no such sentence exists for `snapshot`). AC-051's (sem: SEM-gx-adapter-mcp-214)
//! subject is "tool-call", and a `resources/read` is not one. (sem: SEM-gx-adapter-mcp-215)
//!
//! # What this module is not
//!
//! It is not an MCP client. Nothing here frames JSON-RPC, opens a connection or knows a method name;
//! the manifest argues why (a linked client library would be a **second road** to the substrate, with
//! no `Admitted` in its signature, reachable by anyone who can name the crate). What ships is the shape
//! of the question, and a deployment answers it.

use gx_core::DeltaRef;
use gx_substrate::Result;

/// 🔴 **`req/312` M-01 (R23)** — the token a transport writes into
/// [`gx_substrate::Error::Unreadable`]'s `detail` when the **server answered** and its answer is
/// that this locator holds nothing.
///
/// # Two facts under one variant, and why the discrimination lives in a string
///
/// `Unreadable` is what a transport says for "there is nothing here" **and** for "I could not tell
/// you": a wire that died, a JSON-RPC error, a tool result flagged `isError`, an answer carrying no
/// content. `crate::apply`'s post-call observation folded all of them to the digest of no content,
/// so a read that merely **failed** after a successful call signed a postcondition asserting the
/// object was empty — measured with 24 bytes in the object — and the undo that postcondition
/// blocks is then refused for ever with *the world moved after the transformation being undone
/// committed*, over a world that did not move.
///
/// The honest fix is a second variant, or a field on this one. `gx_substrate::Error` is the frozen
/// face **N-08** pins (`gx-substrate/tests/adapter_spec.rs`), so neither is available to this lane,
/// and inventing a parallel error type in this crate would put two vocabularies where the engine
/// reads one. What is left is the one field the variant carries that the transport writes: this
/// token, in `detail`.
///
/// # The default is the fail-closed one, and that is the whole point
///
/// A transport that says nothing has **not** said "there is nothing there". An unmarked
/// `Unreadable` is therefore a read that did not answer, and the post-call observation refuses
/// rather than signs. A transport that can tell the two apart — [`ToolTransport::read`] against a
/// server whose protocol has a "resource not found" answer — marks the absence and gets the old
/// behaviour, which is the road a call that **removed** a resource travels.
pub const READ_ANSWERED_ABSENT: &str =
    "[gx: the server answered, and its answer is that this locator holds nothing]";

/// Whether an [`gx_substrate::Error::Unreadable`] detail was written by a transport that
/// **decided** this locator holds nothing.
///
/// # 🔴 `req/316` M-01 (R24) — why this is a position and not a search
///
/// This was `detail.contains(READ_ANSWERED_ABSENT)`, and the token lived in the same string as the
/// server's own error message: `WireTransport::read` wrote `format!("{e} {TOKEN}")`, and `{e}` is
/// `WireError::Refused`'s `Display`, which carries whatever the server put in `message`. So a
/// server that wrote these 74 characters into the message of *any* JSON-RPC error got the absence
/// fold back on a road where its read had **failed** — measured: a signed postcondition whose
/// fingerprint was bit-identical to the fingerprint of a genuinely absent object, over an object
/// still holding its bytes, with `-32603` on the wire.
///
/// The decision itself is, and was, a **code equality**: `gx_mcp_wire::client::answered_absent`
/// asks `code == -32002` ([`gx_mcp_wire::jsonrpc::RESOURCE_NOT_FOUND`]) and nothing else. What was
/// broken is the way that decision crossed the boundary. `gx_substrate::Error` is the frozen face
/// **N-08** pins, so the decision cannot travel as a field or a variant; what is left is `detail`,
/// and the fix is to give the token a **position** the server cannot reach. gx writes it first and
/// this asks for it first, so the server's text can only ever be behind it — and the branch that
/// does not decide absence writes `e.to_string()`, which begins with one of `WireError`'s own
/// prefixes and never with this token.
///
/// The trailing check is what keeps the position honest for a future token that is a prefix of
/// this one: the token is either the whole detail or is followed by a space.
#[must_use]
pub fn read_answered_absent(detail: &str) -> bool {
    detail
        .strip_prefix(READ_ANSWERED_ABSENT)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Proof that a gate admitted the delta this call belongs to.
///
/// Not a capability in the security sense -- it grants nothing, and a transport that ignores it still
/// works. What it is, is a **type that cannot be written down outside this crate**: since
/// [`ToolTransport::call`] requires one, the set of places from which a tool call can be made is a set
/// the compiler computes, and the crate root's second premise ("inside this crate, they are minted in (sem: SEM-gx-adapter-mcp-216)
/// one place") is the only part left for a text gate to say. (sem: SEM-gx-adapter-mcp-217)
///
/// It carries the delta so that a transport can check the call it was handed against the admission it
/// was handed. Nothing here forces it to; the field is there so that a deployment that wants the check
/// has the material for it.
///
/// # The refusal, and its control (**M4-20, adopted (b)**'s pair form) (sem: SEM-gx-adapter-mcp-218)
///
/// A crate that is not this one cannot write the value down. The block below is the whole of that
/// claim, and it fails to compile:
///
/// ```compile_fail
/// use gx_adapter_mcp::Admitted;
/// use gx_core::{Cid, DeltaRef, SubstrateKind};
/// let admitted = Admitted {
///     delta: DeltaRef { substrate: SubstrateKind::Mcp, cid: Cid([0u8; 32]) },
/// };
/// ```
///
/// The control below is the same crate, the same types and the same trait, and it **does** compile --
/// so "it failed to compile" above can only mean the private field. A transport *receives* both values (sem: SEM-gx-adapter-mcp-219)
/// and may read them; what it cannot do is make one.
///
/// ```
/// use gx_adapter_mcp::{Admitted, ToolCall, ToolTransport};
/// struct Wire;
/// impl ToolTransport for Wire {
///     fn read(&self, _server: &str, _resource: &str) -> gx_substrate::Result<Vec<u8>> {
///         Ok(Vec::new())
///     }
///     fn call(&self, call: &ToolCall, admitted: &Admitted) -> gx_substrate::Result<Vec<u8>> {
///         assert_eq!(call.delta(), admitted.delta(), "a transport may check the pair");
///         Ok(Vec::new())
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Admitted {
    delta: DeltaRef,
}

impl Admitted {
    /// Minted in exactly one place, and `tests/ac_051.rs` derives that place from this crate's own
    /// `src/` rather than being told it.
    pub(crate) fn for_delta(delta: &DeltaRef) -> Self {
        Self {
            delta: delta.clone(),
        }
    }

    /// The delta a gate admitted.
    #[must_use]
    pub fn delta(&self) -> &DeltaRef {
        &self.delta
    }
}

/// One call to one tool, as this adapter's `apply` built it.
///
/// Private fields and a `pub(crate)` constructor, for [`Admitted`]'s reason: a transport that could
/// **build** a `ToolCall` could make one this crate never planned, and the token would then only prove
/// that some admitted apply was in progress. Since it cannot, the worst a transport can do with what it
/// holds is send the call it was already asked to send -- and 51 §7 contract 7 makes a second send a
/// no-op.
///
/// The refusal has the same pair as [`Admitted`]'s, and the control there is the control for both:
///
/// ```compile_fail
/// use gx_adapter_mcp::ToolCall;
/// use gx_core::{Cid, DeltaRef, SubstrateKind};
/// let call = ToolCall {
///     server: "https://mcp.example/sse".to_string(),
///     resource: "file:///srv/notes.md".to_string(),
///     tool: "notes.write".to_string(),
///     arguments: Vec::new(),
///     delta: DeltaRef { substrate: SubstrateKind::Mcp, cid: Cid([0u8; 32]) },
/// };
/// ```
#[derive(Debug)]
pub struct ToolCall {
    server: String,
    resource: String,
    tool: String,
    arguments: Vec<u8>,
    delta: DeltaRef,
}

impl ToolCall {
    pub(crate) fn new(
        server: &str,
        resource: &str,
        tool: &str,
        arguments: &[u8],
        delta: &DeltaRef,
    ) -> Self {
        Self {
            server: server.to_string(),
            resource: resource.to_string(),
            tool: tool.to_string(),
            arguments: arguments.to_vec(),
            delta: delta.clone(),
        }
    }

    /// The server endpoint, normalised (the crate root's `≈`).
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    /// The resource this change is **about**, normalised.
    ///
    /// 🔴 Not necessarily the resource the tool will touch. gx's claim is "this change is about this (sem: SEM-gx-adapter-mcp-220)
    /// object", and what a tool actually does is the server's; the crate root says what the membrane (sem: SEM-gx-adapter-mcp-221)
    /// offers against the difference (serialisation, not detection). It is handed over so that a
    /// transport which *can* check has the material, and so that the two facts are visibly two.
    #[must_use]
    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// The tool to call.
    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// What the tool is handed.
    #[must_use]
    pub fn arguments(&self) -> &[u8] {
        &self.arguments
    }

    /// The delta this call performs. The same value [`Admitted::delta`] carries, which is what makes
    /// the two checkable against each other.
    #[must_use]
    pub fn delta(&self) -> &DeltaRef {
        &self.delta
    }
}

/// The wire, which is the deployment's.
///
/// `Send + Sync` because [`crate::McpAdapter`] holds one and 41 §4 bounds the adapter (**AC-046**): an
/// engine may work on several transformations at once, and a transport that could not cross a thread
/// boundary would make the adapter one that only works single-threaded.
pub trait ToolTransport: Send + Sync {
    /// Read the current contents of a resource (MCP's `resources/read`).
    ///
    /// No token: reading is not calling, and the module documentation gives the ruling.
    ///
    /// # Errors
    /// [`gx_substrate::Error::Unreadable`] for a resource this server will not answer for -- including
    /// one that is not there. v0.1 has no way to describe "this position is empty" as an (sem: SEM-gx-adapter-mcp-222)
    /// `ObjectSnapshot`, which is the same shape `gx-adapter-fs` and `gx-adapter-git` have for an
    /// absent file and an unborn branch.
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>>;

    /// 🔴 **DR-46-9 A-3** (`req/38` §196) — read a prior the server publishes **only** behind a
    /// tool, for a declaration that names one
    /// ([`crate::catalogue::PriorRead`]).
    ///
    /// The default implementation refuses, so a deployment that has not written one behaves
    /// exactly as it did before this method existed: a catalogue that declares no `read_by`
    /// never reaches here, and one that does gets [`OnReadFailure`](crate::catalogue::OnReadFailure)'s
    /// answer for a read that failed.
    ///
    /// # 🔴 This sends a `tools/call`, and pretending otherwise would be the weakening
    ///
    /// AC-051 asks that "no bypass route technically exists", and the crate root's derivation is
    /// built on `call` being reachable from `apply` alone. This method is a **second** road to the
    /// wire and it is declared as one rather than hidden behind the word "read":
    ///
    /// * it takes **no [`Admitted`]**, because it is not performing the transformation's effect —
    ///   it is building the escrow the transformation cannot commit without (**E-M4-30**, 43
    ///   T-10b), and 41 §4 puts no gate in front of `snapshot` or `precondition` either
    ///   (**E-M4-29**, "reads are not forbidden");
    /// * the **tool name and every argument come from the deployment's catalogue**, never from the
    ///   agent: the name is a string in the catalogue file and the arguments are built by a
    ///   template out of the forward call's own members and constants. The set of tools an agent
    ///   can reach through this road is therefore the set an operator wrote down, and it is empty
    ///   until an operator writes one;
    /// * **it is called from exactly one place**, `crate::invert`, and `tests/ac_051.rs` D-6
    ///   derives that site by walking this crate's `src/` rather than being told it.
    ///
    /// What the derivation **cannot** rule out is an operator declaring a *writing* tool as a read
    /// face. Nothing in MCP marks a tool read-only that a server cannot also lie about
    /// (`annotations.readOnlyHint` is the server's own word for itself), so this is a declaration
    /// the deployment is answerable for, in the same sense the catalogue's "what undoes what"
    /// already is. `docs/LIMITS.md` v0.5-d says so on the page a buyer reads.
    ///
    /// # Errors
    /// [`gx_substrate::Error::Unreadable`] — the same vocabulary [`ToolTransport::read`] answers
    /// in, because from the escrow's side the two failures are one fact: the prior did not arrive.
    fn read_prior_by_tool(&self, server: &str, tool: &str, arguments: &[u8]) -> Result<Vec<u8>> {
        let _ = arguments;
        Err(gx_substrate::Error::Unreadable {
            locator: server.to_string(),
            detail: format!(
                "this deployment's transport publishes no read-by-tool face, so a prior that lives \
                 behind the tool {tool:?} cannot be read"
            ),
        })
    }

    /// Call a tool (MCP's `tools/call`), for a delta a gate admitted.
    ///
    /// Returns the tool result's **content bytes** on success — the first text content of the
    /// `tools/call` result as UTF-8 bytes when the server sent one, else the serialized result.
    ///
    /// 🔴 **A narrowing correction, not a reversal** (`req/38` §98, ruling 1, two-phase escrow). This (sem: SEM-gx-adapter-mcp-223)
    /// method returned `()` through v0.2, with the doctrine: "a transport that returned the tool's (sem: SEM-gx-adapter-mcp-224)
    /// own report of what it did would be handing the adapter a second, unverifiable source for a
    /// value it is about to read for itself". That doctrine stays true **of the post-state** -- (sem: SEM-gx-adapter-mcp-225)
    /// `apply`'s fingerprint and digest are still the read-back, never these bytes. What changed
    /// is one measured fact (`req/153` §6, `req/161` §1): a do-time server-assigned value a
    /// declared inverse needs (a created issue's number, derivable from the result's URL) exists
    /// **only** in this answer, at exactly this moment, and is not re-obtainable later (the
    /// idempotency log never re-issues the call). So the answer is now carried upward as an
    /// **observation record** — journalled, content-addressed, and read by the escrow completion
    /// step alone (`gx_substrate::InverseCompletion`) — not read as a state.
    ///
    /// # Errors
    /// [`gx_substrate::Error::ApplyFailed`] when the call could not be made or the tool refused. 43
    /// T-11 turns that into `AbortReason::ApplyFailed`.
    fn call(&self, call: &ToolCall, admitted: &Admitted) -> Result<Vec<u8>>;
}
