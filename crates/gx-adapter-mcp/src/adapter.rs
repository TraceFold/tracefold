//! The seven methods of 41 §4, all seven in one hand.
//!
//! There is no [`Error::Unimplemented`] in this file, which is the fact 51 §7's completion condition
//! turns on: the harness reports 「無い」 for that variant and for nothing else, so an adapter with none
//! of them is an adapter every obligation was **run** against (**§31 M4H3-4 (b)**). The word is still in
//! the vocabulary and this crate still writes it -- [`crate::delta::McpDelta::decode`] answers it for a
//! sequence v0.1 does not run -- which is §32 M4H4-2 追認's point: 「未実装」 and 「失敗」 are permanently
//! different facts.

use std::sync::Arc;

use gx_canon::cid::{self, Domain};
use gx_core::{
    Cid, Commutation, Fingerprint, Intent, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind,
};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result, SubstrateAdapter};

use crate::catalogue::Catalogue;
use crate::locator;
use crate::log::{CallLog, MemoryCallLog};
use crate::transport::ToolTransport;

/// The digest of a resource's contents.
///
/// `Domain::Leaf` for the reason the other two adapters give -- content is bytes and not a projected
/// value, so there is no `IdentityView` to go through. It is the **same function**, so a resource
/// holding the bytes of a file has the object digest that file has, whichever substrate holds it.
#[must_use]
pub fn content_digest(contents: &[u8]) -> Cid {
    cid::mint(Domain::Leaf, &[contents])
}

/// The digest of 「there is nothing here」.
///
/// 🔴 The same value as the digest of an **empty** resource, and the same residue the other two
/// adapters record: 「digest=内容のみ」 leaves an adapter nothing with which to distinguish 「no resource at
/// this URI」 from 「a resource with no bytes」. Inventing a marker would not help -- any byte string a
/// marker used is also a possible content -- so the fix belongs with a wider `Fingerprint` (v0.2).
#[must_use]
pub fn absent_digest() -> Cid {
    content_digest(&[])
}

/// The MCP adapter (41 §2, 41 §4, FR-046).
///
/// Three fields, and each of them is a seam a deployment fills:
///
/// * the **transport** is the wire (`transport.rs` argues why it is not linked in here);
/// * the **catalogue** is 「which tools can be undone, and by what」, which only the party running the
///   server knows (`catalogue.rs`);
/// * the **log** is what makes a retry a retry on a substrate whose deltas declare no state (`log.rs`).
///
/// `Arc` rather than a generic parameter: an engine holds adapters behind `Box<dyn SubstrateAdapter>`
/// (**AC-046**), so a `McpAdapter<T, L>` would have to be monomorphised by whoever boxes it and every
/// deployment would carry two type parameters through its wiring for no property gained.
#[derive(Clone)]
pub struct McpAdapter {
    transport: Arc<dyn ToolTransport>,
    catalogue: Catalogue,
    log: Arc<dyn CallLog>,
}

/// Written by hand rather than derived: `Arc<dyn ToolTransport>` has no `Debug` (a transport is a
/// deployment's type and 41 §4 puts no bound on it), and a derived one would name the fields it could
/// print while silently dropping the one that matters. What a reader wants here is what is wired, not
/// what is inside it.
impl core::fmt::Debug for McpAdapter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("McpAdapter")
            .field("restorable_tools", &self.catalogue.declared())
            .finish_non_exhaustive()
    }
}

impl McpAdapter {
    /// An adapter over a transport, with an empty catalogue and an in-memory log.
    ///
    /// The defaults are the conservative ones: nothing is declared undoable (so every change is
    /// escalated by **E-M3-4**), and the log is [`MemoryCallLog`], whose bound is stated where it is
    /// defined.
    #[must_use]
    pub fn new(transport: Arc<dyn ToolTransport>) -> Self {
        Self {
            transport,
            catalogue: Catalogue::new(),
            log: Arc::new(MemoryCallLog::new()),
        }
    }

    /// The deployment's declaration of which tools can be undone.
    #[must_use]
    pub fn with_catalogue(mut self, catalogue: Catalogue) -> Self {
        self.catalogue = catalogue;
        self
    }

    /// A log that outlives this process, when a deployment has one.
    #[must_use]
    pub fn with_log(mut self, log: Arc<dyn CallLog>) -> Self {
        self.log = log;
        self
    }

    /// What this adapter can undo, for a report line that would otherwise say 「conformant」 about a run
    /// against an empty catalogue.
    #[must_use]
    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }
}

impl SubstrateAdapter for McpAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Mcp
    }

    /// The state of one resource, named by its own projection.
    ///
    /// The locator is parsed on the way in and the snapshot reports the **normalised** spelling: 41 §4
    /// has `snapshot` receive one already normalised (**H-2**), normalising again is free (L7's
    /// idempotence), and it stops a caller's spelling from reaching a gate through a snapshot.
    /// `representation` is [`ReprKind::Bytes`]: this adapter reads a resource's contents and does not
    /// parse them.
    ///
    /// # Errors
    /// [`Error::NotAPosition`] for a locator that is not one, and whatever the transport says about a
    /// resource it will not answer for.
    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        let position = locator::parse(locator)?;
        let contents = self
            .transport
            .read(position.server(), position.resource())?;
        let digest = content_digest(&contents);

        // 42 §1.3 row 1 excludes `id` from the projection, so the placeholder cannot reach the digest
        // -- the same argument `PlannedDelta::new` makes about `reference`.
        let placeholder = ObjectSnapshot::new(
            ObjectId(Cid([0u8; 32])),
            SubstrateKind::Mcp,
            position.locator(),
            digest,
            ReprKind::Bytes,
        );
        let id = cid::compute(&placeholder).map_err(|e| Error::NotDigestible {
            detail: e.to_string(),
        })?;
        Ok(ObjectSnapshot::new(
            ObjectId(id),
            SubstrateKind::Mcp,
            position.locator(),
            digest,
            ReprKind::Bytes,
        ))
    }

    /// Delegates to [`crate::plan`], which names no transport call at all (**E-M4-29**, L1).
    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        crate::plan::plan(intent, pre)
    }

    /// Name the state a commit is conditional on -- and here that state is the **resource**.
    ///
    /// 🔴 Not the server, and the crate root's table is where the three sets are laid out. 51 §7
    /// 契約 3 requires this value to **change when the state changes**, so its subject has to be
    /// something a proxy can read; a server has no digest. What the server is instead is the
    /// **footprint** ([`crate::commutation`]), and holding those two apart is this adapter's one real
    /// departure from `gx-adapter-git`, where a branch happens to be both.
    ///
    /// An over-long scope becomes a digest line before it reaches [`Fingerprint::new`] (**M4H1-2**,
    /// [`elide_scope`]); the bound itself is gx-core's and refuses anything that arrives past it. That
    /// is the single road **req/98** §3-4 予約 6 asks the M7 adapters to take.
    ///
    /// # Errors
    /// [`Error::NotAPosition`], whatever the transport says, and `ScopeTooLong` through [`elide_scope`].
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        let position = locator::parse(snap.locator())?;
        let contents = self
            .transport
            .read(position.server(), position.resource())?;
        Ok(Fingerprint::new(
            SubstrateKind::Mcp,
            elide_scope(position.locator())?,
            content_digest(&contents),
        )?)
    }

    /// Delegates to [`crate::apply`], which is the only module in this crate that reaches a transport's
    /// `call` -- the second premise of AC-051.
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        crate::apply::apply(self.transport.as_ref(), self.log.as_ref(), delta)
    }

    /// Delegates to [`crate::invert`]. **E-M4-30**: the escrowed inverse is constructed **before**
    /// `apply` (43 T-10b), because the body it carries is the resource's prior contents.
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>> {
        crate::invert::invert(self.transport.as_ref(), &self.catalogue, delta, pre)
    }

    /// Delegates to [`crate::commutation`], which compares **servers** and touches no transport
    /// (**M4-25 採(a)**, AC-052, AC-053).
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        crate::commutation::commutation(a, b)
    }
}
