//! An MCP server that lives in this process, and the 51 §7 fixture over it.
//!
//! 51 §7 逐語: 「各adapterクレートはこのハーネスを`#[test]`から呼び出すのみで契約テストを継承する」, so what an
//! adapter's author writes is [`McpFixture`] and nothing else. Its length is the **third** measurement
//! of whether the eleven-method `Fixture` of M4 hand 3 is a reasonable ask (**M4H3-9**), and
//! `mcp_conformance.rs` prints it beside the other two.
//!
//! # Why the server is here rather than on a socket
//!
//! Because what this crate ships is a boundary, not a client: `Cargo.toml` argues that a linked MCP
//! library would be a **second road** to a substrate whose acceptance criterion is 「バイパス手段が技術的
//! に存在しない」. So a deployment writes the transport, and the smallest honest stand-in for one is a
//! transport written here -- which also makes the fixture able to do what 51 §7 needs and a socket
//! could not: move the substrate **behind the adapter's back** ([`gx_substrate_conformance::Fixture::disturb`])
//! and count what arrived.
//!
//! What that bounds is stated in `mcp_conformance.rs` beside the claim: the contracts hold against
//! **this** server, in one process, single-threaded.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_adapter_mcp::{
    restore_arguments, Admitted, CallLog, Catalogue, McpAdapter, ToolCall, ToolIntent,
    ToolTransport,
};
use gx_core::{
    Actor, ChangeContext, Cid, DeltaRef, GoalBytes, Intent, ObjectId, ObjectSnapshot, ReprKind,
    SubstrateKind,
};
use gx_substrate::{Error, PlannedDelta, Result, SubstrateAdapter};
use gx_substrate_conformance::Fixture;

/// The server this fixture points at.
pub const SERVER: &str = "https://mcp.example/sse";
/// A second server, for 51 §7's 可換 case.
pub const OTHER_SERVER: &str = "stdio://second-server";
/// The resource a change is about.
pub const SUBJECT: &str = "file:///srv/notes.md";
/// A second resource on the **same** server -- the case this adapter's `commutation` decides.
pub const SIBLING: &str = "file:///srv/other.md";

/// The tool that writes a resource, and the tool that puts it back.
pub const WRITE_TOOL: &str = "notes.write";
/// The compensating tool a [`Catalogue`] declares for [`WRITE_TOOL`].
pub const RESTORE_TOOL: &str = "notes.restore";
/// A tool with **no** declared inverse: 51 §7 contract 5's subject for this adapter.
pub const NOTIFY_TOOL: &str = "notify";

/// What the subject holds when the fixture starts.
pub const INITIAL: &[u8] = b"before\n";
/// What the fixture's intent asks for.
pub const GOAL: &[u8] = b"after\n";

/// The locator of the subject on the fixture's server.
#[must_use]
pub fn subject_locator() -> String {
    format!("{SERVER}#{SUBJECT}")
}

/// A locator on this fixture's server.
#[must_use]
pub fn locator_on(server: &str, resource: &str) -> String {
    format!("{server}#{resource}")
}

/// An MCP server, in this process.
///
/// Three tools. [`WRITE_TOOL`] takes the new contents as its whole argument -- a tool whose arguments
/// are opaque to gx, which is the point ([`ToolCall::arguments`] carries whatever the agent asked for).
/// [`RESTORE_TOOL`] takes the **restore convention**'s `{contents, uri}`, which is how the inverse this
/// adapter builds reaches a server that already speaks the protocol. [`NOTIFY_TOOL`] changes nothing a
/// proxy can read, which is the honest shape of most of what an agent calls.
#[derive(Debug, Default)]
pub struct FakeServer {
    resources: Mutex<BTreeMap<String, Vec<u8>>>,
    reads: AtomicUsize,
    calls: AtomicUsize,
    unmatched_admissions: AtomicUsize,
    refuse: Mutex<Option<String>>,
}

impl FakeServer {
    /// A server holding [`INITIAL`] at [`SUBJECT`] and at [`SIBLING`].
    #[must_use]
    pub fn new() -> Self {
        let server = Self::default();
        server.rewind();
        server
    }

    /// Put the resources back the way the fixture found them.
    pub fn rewind(&self) {
        let mut resources = self.resources.lock().expect("not poisoned");
        resources.clear();
        resources.insert(SUBJECT.to_string(), INITIAL.to_vec());
        resources.insert(SIBLING.to_string(), INITIAL.to_vec());
    }

    /// Write a resource **without** going through the adapter: `Fixture::disturb`'s whole job.
    pub fn write_behind_the_adapter(&self, resource: &str, contents: &[u8]) {
        self.resources
            .lock()
            .expect("not poisoned")
            .insert(resource.to_string(), contents.to_vec());
    }

    /// What a resource holds now.
    #[must_use]
    pub fn contents(&self, resource: &str) -> Option<Vec<u8>> {
        self.resources
            .lock()
            .expect("not poisoned")
            .get(resource)
            .cloned()
    }

    /// How many tool calls have arrived. **AC-051's counter.**
    #[must_use]
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// How many reads have arrived. Reading is not calling (`transport.rs`), so the two are counted
    /// apart and `mcp_plan_purity.rs` asserts both are zero after a plan.
    #[must_use]
    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    /// How many calls arrived with an admission naming a **different** delta. Zero, always: the
    /// counter exists so that 「the token matches」 is measured rather than assumed.
    #[must_use]
    pub fn unmatched_admissions(&self) -> usize {
        self.unmatched_admissions.load(Ordering::SeqCst)
    }

    /// Make the next call fail, for 43 T-11's `AbortReason::ApplyFailed`.
    pub fn refuse_with(&self, detail: &str) {
        *self.refuse.lock().expect("not poisoned") = Some(detail.to_string());
    }

    /// Stop refusing.
    pub fn stop_refusing(&self) {
        *self.refuse.lock().expect("not poisoned") = None;
    }
}

/// The `{contents, uri}` of the restore convention, read back.
///
/// The field types are the adapter's own (`gx_adapter_mcp::delta::Bytes` carries a byte string rather
/// than a sequence of integers, for M2H1-4's reason), so this struct is the server's reader for the
/// convention `delta.rs` documents rather than a second spelling of it.
#[derive(serde::Deserialize)]
struct RestorePayload {
    contents: gx_adapter_mcp::delta::Bytes,
    uri: String,
}

impl ToolTransport for FakeServer {
    fn read(&self, server: &str, resource: &str) -> Result<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if server != SERVER {
            return Err(Error::Unreadable {
                locator: locator_on(server, resource),
                detail: format!("this fixture is one server ({SERVER}) and that is another"),
            });
        }
        self.contents(resource).ok_or_else(|| Error::Unreadable {
            locator: locator_on(server, resource),
            detail: "this server holds no resource at that URI".to_string(),
        })
    }

    fn call(&self, call: &ToolCall, admitted: &Admitted) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if call.delta() != admitted.delta() {
            self.unmatched_admissions.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(detail) = self.refuse.lock().expect("not poisoned").clone() {
            return Err(Error::ApplyFailed { detail });
        }
        if call.server() != SERVER {
            return Err(Error::ApplyFailed {
                detail: format!("this fixture is one server ({SERVER}) and that is another"),
            });
        }
        match call.tool() {
            WRITE_TOOL => {
                self.write_behind_the_adapter(call.resource(), call.arguments());
                Ok(())
            }
            RESTORE_TOOL => {
                let payload = decode_restore(call.arguments())?;
                self.write_behind_the_adapter(&payload.uri, &payload.contents.0);
                Ok(())
            }
            NOTIFY_TOOL => Ok(()),
            other => Err(Error::ApplyFailed {
                detail: format!("this server publishes no tool called {other:?}"),
            }),
        }
    }
}

/// The restore convention, decoded through the one encoder 41 §6 admits.
fn decode_restore(bytes: &[u8]) -> Result<RestorePayload> {
    gx_canon::cbor::decode(bytes).map_err(|e| Error::ApplyFailed {
        detail: format!("the restore arguments are not `{{contents, uri}}`: {e}"),
    })
}

/// A [`CallLog`] the fixture can rewind, which is what makes `Fixture::reset` honest.
///
/// 🔴 **Rewinding the substrate without rewinding the record would be rewinding half a world.** The log
/// is keyed by the delta's CID, and two obligations planning the same intent produce the **same**
/// delta; a fixture that put the resources back and left the record would make the second obligation's
/// `apply` a no-op against a substrate that had been undone. That is not the retry 51 §7 contract 7 is
/// about -- it is the fixture lying about which world it is in.
///
/// It also exists to show the seam is real: [`CallLog`] is a trait because a deployment supplies its
/// own, and this file is a second implementation of it.
#[derive(Debug, Default)]
pub struct RewindableLog {
    applied: Mutex<HashSet<DeltaRef>>,
}

impl RewindableLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget everything, because the world has been put back.
    pub fn rewind(&self) {
        self.applied.lock().expect("not poisoned").clear();
    }

    /// How many distinct deltas have been applied through this log.
    #[must_use]
    pub fn len(&self) -> usize {
        self.applied.lock().expect("not poisoned").len()
    }
}

impl CallLog for RewindableLog {
    fn applied(&self, delta: &DeltaRef) -> bool {
        self.applied.lock().expect("not poisoned").contains(delta)
    }

    fn record(&self, delta: &DeltaRef) {
        self.applied
            .lock()
            .expect("not poisoned")
            .insert(delta.clone());
    }
}

/// The catalogue this fixture's deployment declares: one tool can be undone, one cannot.
#[must_use]
pub fn catalogue() -> Catalogue {
    Catalogue::new().with_restore(WRITE_TOOL, RESTORE_TOOL)
}

/// An intent that asks for a tool call.
#[must_use]
pub fn intent_for(locator: &str, tool: &str, arguments: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Mcp,
        locator.to_string(),
        GoalBytes(
            ToolIntent::new(tool, arguments.to_vec())
                .encode()
                .expect("a tool call has a canonical form"),
        ),
        ChangeContext::Policy,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

/// A delta this adapter planned, for a call at a position.
///
/// # Panics
/// If the adapter refuses to plan, which for a well-formed position it does not.
#[must_use]
pub fn planned(adapter: &McpAdapter, locator: &str, tool: &str, arguments: &[u8]) -> PlannedDelta {
    let pre = absent_snapshot(locator);
    adapter
        .plan(&intent_for(locator, tool, arguments), &pre)
        .expect("this adapter plans a well-formed call")
}

/// The `pre` of a change to a position this fixture's server does not answer for.
///
/// 🔴 Constructed here because the adapter will not produce one: `snapshot` reads, and a server that
/// will not answer produces [`Error::Unreadable`] rather than a snapshot of nothing. `plan` never reads
/// it (L1 is why), so it is a legal argument for planning and for `invert`'s locator check.
///
/// # Panics
/// Never: the projection of a snapshot always has a canonical form.
#[must_use]
pub fn absent_snapshot(locator: &str) -> ObjectSnapshot {
    let normalised = gx_adapter_mcp::normalize(locator);
    let digest = gx_adapter_mcp::adapter::absent_digest();
    let placeholder = ObjectSnapshot::new(
        ObjectId(Cid([0u8; 32])),
        SubstrateKind::Mcp,
        normalised.clone(),
        digest,
        ReprKind::Bytes,
    );
    let id = gx_canon::cid::compute(&placeholder).expect("a snapshot projects to a canonical form");
    ObjectSnapshot::new(
        ObjectId(id),
        SubstrateKind::Mcp,
        normalised,
        digest,
        ReprKind::Bytes,
    )
}

/// The 51 §7 harness's view of this adapter.
pub struct McpFixture {
    server: Arc<FakeServer>,
    log: Arc<RewindableLog>,
    adapter: McpAdapter,
}

impl McpFixture {
    #[must_use]
    pub fn new() -> Self {
        let server = Arc::new(FakeServer::new());
        let log = Arc::new(RewindableLog::new());
        let adapter = McpAdapter::new(server.clone())
            .with_catalogue(catalogue())
            .with_log(log.clone());
        Self {
            server,
            log,
            adapter,
        }
    }

    #[must_use]
    pub fn server(&self) -> &Arc<FakeServer> {
        &self.server
    }

    #[must_use]
    pub fn log(&self) -> &Arc<RewindableLog> {
        &self.log
    }

    #[must_use]
    pub fn mcp(&self) -> &McpAdapter {
        &self.adapter
    }
}

impl Default for McpFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture for McpFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        &self.adapter
    }

    fn locator(&self) -> String {
        gx_adapter_mcp::normalize(&subject_locator())
    }

    fn intent(&self) -> Intent {
        intent_for(&self.locator(), WRITE_TOOL, GOAL)
    }

    /// Put the world back -- **both halves of it**, and [`RewindableLog`] says why the record is one.
    fn reset(&self) -> Result<()> {
        self.server.rewind();
        self.log.rewind();
        Ok(())
    }

    fn disturb(&self) -> Result<()> {
        self.server
            .write_behind_the_adapter(SUBJECT, b"somebody else called a tool\n");
        Ok(())
    }

    /// The adapter's own normalisation, which is the subject of L7 (**E-M4-12**).
    fn normalise(&self, locator: &str) -> Option<String> {
        Some(gx_adapter_mcp::normalize(locator))
    }

    /// **L5**: the digest a plan of this intent promises, derived from the goal and from nothing else.
    ///
    /// The two routes are the other adapters' two, one substrate over: here, the [`GOAL`] bytes
    /// digested without a server being read at all; and in `apply`, the bytes read back **out of the
    /// server** after the call. What the comparison catches is the whole space between what the agent
    /// asked for and what the resource holds -- a tool that wrote somewhere else, a payload carrying
    /// something other than the arguments, a proxy that reported success without calling.
    fn promised_target(&self) -> Option<Cid> {
        Some(gx_adapter_mcp::adapter::content_digest(GOAL))
    }

    /// A delta whose inverse this adapter cannot construct: a call to a tool with **no declared
    /// restore**.
    ///
    /// 51 §7 contract 5 needs a subject only the adapter's author can produce. For a filesystem it is
    /// an escrow ceiling; for a git repository it is an unborn branch; here it is the ordinary case --
    /// **most tools cannot be undone**, and nothing in the protocol says which can. `invert` answers
    /// `Ok(None)` and **E-M3-4** asks a person.
    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        let locator = self.locator();
        let pre = absent_snapshot(&locator);
        let delta = self
            .adapter
            .plan(&intent_for(&locator, NOTIFY_TOOL, b"{}"), &pre)
            .ok()?;
        Some((delta, pre))
    }

    /// 51 §7's 可換 case: two calls on **two servers**.
    ///
    /// Not 「two resources」 -- the crate root argues why a tool call's footprint is its server, and
    /// `mcp_commutation.rs` measures the case that difference decides.
    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        Some((
            planned(&self.adapter, &self.locator(), WRITE_TOOL, b"one\n"),
            planned(
                &self.adapter,
                &locator_on(OTHER_SERVER, SUBJECT),
                WRITE_TOOL,
                b"two\n",
            ),
        ))
    }

    /// 51 §7's 非可換 case: two calls on one server.
    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        let subject = self.locator();
        Some((
            planned(&self.adapter, &subject, WRITE_TOOL, b"one\n"),
            planned(&self.adapter, &subject, WRITE_TOOL, b"two\n"),
        ))
    }

    /// Pairs this adapter's `≈` calls equal, one per clause of the crate root's RFC 3986 §6.2.2.
    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        let canonical = self.locator();
        vec![
            // clause 1 (§3.1 / §6.2.2.1): the scheme is case-insensitive.
            (
                locator_on("HTTPS://mcp.example/sse", SUBJECT),
                canonical.clone(),
            ),
            // clause 2 (§6.2.2.1): so is the host.
            (
                locator_on("https://MCP.Example/sse", SUBJECT),
                canonical.clone(),
            ),
            // clause 3 (§6.2.2.2): a triplet encoding an unreserved character is that character.
            (
                locator_on(SERVER, "file:///srv/%6Eotes.md"),
                canonical.clone(),
            ),
            // clause 4 (§6.2.2.3 / §5.2.4): dot segments are removed.
            (locator_on(SERVER, "file:///srv/./x/../notes.md"), canonical),
        ]
    }
}

/// The restore arguments, for a probe that wants to build one without an `invert`.
///
/// # Panics
/// If the pair has no canonical form, which a two-field map always has.
#[must_use]
pub fn restore_payload(uri: &str, contents: &[u8]) -> Vec<u8> {
    restore_arguments(uri, contents).expect("the restore arguments have a canonical form")
}
