//! The boundary a deployment implements, and the two values it cannot build.
//!
//! 34 **AC-051** asks that 「バイパス手段が技術的に存在しない」 and 裁定 #10 (`req/38` §56) asks that the
//! check be a **derivation** rather than a declared list. This module is where the derivation gets its
//! first and largest premise, and the premise is enforced by the compiler:
//!
//! > [`ToolCall`] and [`Admitted`] have private fields and `pub(crate)` constructors, and
//! > [`ToolTransport::call`] takes a reference to each. **No code outside this crate can build
//! > either.**
//!
//! The population that closes is every crate that is not this one -- including crates that do not
//! exist yet, which is the difference between a compiler and a scan. `tests/ac_051.rs` carries the two
//! `compile_fail` doctests that measure it rather than assert it, in the pair form **M4-20 採(b)** used
//! for AC-046: a control that compiles beside each refusal, so that 「it failed to compile」 can only
//! mean the thing under test.
//!
//! # 🔴 Reading is not calling
//!
//! [`ToolTransport::read`] takes no token. That is not an oversight and it is not a weakening: **E-M4-29**
//! (§30 M4H2-3 採(b)) reads 41 §4's 「純関数」 for the whole boundary as 「substrate への**書き込み** 0」 and
//! adds 「読み込みは禁じない」, and req/98 §6-7 spells the M7 version of the same rule -- 「則 1 の M7 版…
//! 「I/O 0」ではない」. An adapter that could not read could not answer `snapshot` or `precondition`, and
//! 51 §7 contract 1 and contract 3 both require it to.
//!
//! So the gate is on the **effect** and not on the observation, which is what 41 §4 asks for in as many
//! words (「`apply` は commit承認後にのみ呼ばれる」 -- and no such sentence exists for `snapshot`). AC-051's
//! subject is 「tool-call」, and a `resources/read` is not one.
//!
//! # What this module is not
//!
//! It is not an MCP client. Nothing here frames JSON-RPC, opens a connection or knows a method name;
//! the manifest argues why (a linked client library would be a **second road** to the substrate, with
//! no `Admitted` in its signature, reachable by anyone who can name the crate). What ships is the shape
//! of the question, and a deployment answers it.

use gx_core::DeltaRef;
use gx_substrate::Result;

/// Proof that a gate admitted the delta this call belongs to.
///
/// Not a capability in the security sense -- it grants nothing, and a transport that ignores it still
/// works. What it is, is a **type that cannot be written down outside this crate**: since
/// [`ToolTransport::call`] requires one, the set of places from which a tool call can be made is a set
/// the compiler computes, and the crate root's second premise (「inside this crate, they are minted in
/// one place」) is the only part left for a text gate to say.
///
/// It carries the delta so that a transport can check the call it was handed against the admission it
/// was handed. Nothing here forces it to; the field is there so that a deployment that wants the check
/// has the material for it.
///
/// # The refusal, and its control (**M4-20 採(b)**'s pair form)
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
/// so 「it failed to compile」 above can only mean the private field. A transport *receives* both values
/// and may read them; what it cannot do is make one.
///
/// ```
/// use gx_adapter_mcp::{Admitted, ToolCall, ToolTransport};
/// struct Wire;
/// impl ToolTransport for Wire {
///     fn read(&self, _server: &str, _resource: &str) -> gx_substrate::Result<Vec<u8>> {
///         Ok(Vec::new())
///     }
///     fn call(&self, call: &ToolCall, admitted: &Admitted) -> gx_substrate::Result<()> {
///         assert_eq!(call.delta(), admitted.delta(), "a transport may check the pair");
///         Ok(())
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
    /// 🔴 Not necessarily the resource the tool will touch. gx's claim is 「this change is about this
    /// object」, and what a tool actually does is the server's; the crate root says what the membrane
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
    /// one that is not there. v0.1 has no way to describe 「this position is empty」 as an
    /// `ObjectSnapshot`, which is the same shape `gx-adapter-fs` and `gx-adapter-git` have for an
    /// absent file and an unborn branch.
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>>;

    /// Call a tool (MCP's `tools/call`), for a delta a gate admitted.
    ///
    /// Returns nothing on success, and that is 41 §4's shape rather than a simplification: `apply`
    /// takes no pre-state and returns no post-state, so what comes back to an engine is an
    /// **observation** the adapter makes afterwards (req/69 §3.1: 「post は返り値でなく観測値である」).
    /// A transport that returned the tool's own report of what it did would be handing the adapter a
    /// second, unverifiable source for a value it is about to read for itself.
    ///
    /// # Errors
    /// [`gx_substrate::Error::ApplyFailed`] when the call could not be made or the tool refused. 43
    /// T-11 turns that into `AbortReason::ApplyFailed`.
    fn call(&self, call: &ToolCall, admitted: &Admitted) -> Result<()>;
}
