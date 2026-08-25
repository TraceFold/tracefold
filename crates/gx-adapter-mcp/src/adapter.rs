// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The seven methods of 41 §4, all seven in one hand.
//!
//! There is no [`Error::Unimplemented`] in this file, which is the fact 51 §7's completion condition
//! turns on: the harness reports "none" for that variant and for nothing else, so an adapter with none (sem: SEM-gx-adapter-mcp-098)
//! of them is an adapter every obligation was **run** against (**§31 M4H3-4 (b)**). The word is still in
//! the vocabulary and this crate still writes it -- [`crate::delta::McpDelta::decode`] answers it for a
//! sequence v0.1 does not run -- which is §32 M4H4-2, confirmed's point: "not implemented" and "failed" are permanently (sem: SEM-gx-adapter-mcp-099)
//! different facts.

use std::sync::Arc;

use gx_canon::cid::{self, Domain};
use gx_core::{
    Cid, Commutation, Fingerprint, Intent, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind,
};
use gx_substrate::{
    elide_scope, AppliedDelta, Error, InputStageDeclaration, InverseCompletion, InvertOutcome,
    PlannedDelta, Result, SubstrateAdapter,
};

use crate::apply::OBSERVATION_NOT_ANSWERED;
use crate::catalogue::{Catalogue, Reversibility};
use crate::delta::{McpDelta, McpOp, MAX_INVERSE_PAYLOAD_BYTES};
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

/// The digest of "there is nothing here". (sem: SEM-gx-adapter-mcp-100)
///
/// 🔴 The same value as the digest of an **empty** resource, and the same residue the other two
/// adapters record: "digest = content only" leaves an adapter nothing with which to distinguish "no resource at (sem: SEM-gx-adapter-mcp-101)
/// this URI" from "a resource with no bytes". Inventing a marker would not help -- any byte string a (sem: SEM-gx-adapter-mcp-102)
/// marker used is also a possible content -- so the fix belongs with a wider `Fingerprint` (v0.2).
#[must_use]
pub fn absent_digest() -> Cid {
    content_digest(&[])
}

/// 🔴 **`req/320` M-01 (`req/38` §229 ruling 2)** — the sentence a read that **answered**, with
/// *there is nothing at this locator*, carries out of `snapshot` and `precondition`.
///
/// # The two sentences that were in the same string
///
/// `req/312` M-01 split `Unreadable` into its two preimages — *the server answered that this
/// locator holds nothing* and *the server would not tell me* — and R23/R24 built the discriminator:
/// [`crate::transport::READ_ANSWERED_ABSENT`], written at position 0 of the detail by the wire and
/// asked for by [`crate::transport::read_answered_absent`]. Exactly **one** of the three sites that
/// consume a declared read asked it: `crate::apply`'s post-apply observation. The other two are
/// here.
///
/// So the twenty-fourth audit ran a session where gx itself admitted a `notes.delete`, watched the
/// object disappear, and then handed the agent one sentence containing both
///
/// > the substrate would not answer for "stdio://a24#tool:notes.fetch": \[gx: the server answered,
/// > and its answer is that this locator holds nothing\]
///
/// — the outer clause being `gx_substrate::Error::Unreadable`'s `Display`, which is an **N-08
/// frozen face** this crate may not edit, and the inner one being gx's own token. A reader cannot
/// act on a sentence that denies its own evidence.
///
/// # Why this is still a refusal
///
/// Not because the read failed — it did not — but because there is no prior state for a
/// compare-and-set to be conditional on. Folding absence to [`absent_digest`] here (which is what
/// `crate::apply` does *after* an admitted call) would let gx mediate a call against an object that
/// does not exist, and DR-43-1 (a)'s undo compares a fingerprint against a state this build would
/// then have invented. That is a capability decision, not a sentence, and it is not this lane's:
/// what changes here is that the refusal says which of the two facts it is.
///
/// # Why the correction is inside `detail` and not instead of it
///
/// The enclosing words are frozen and they are this crate's caller's, so the only place a proxy can
/// tell a reader which reading is the true one is the field it does own. `crate::apply`'s
/// [`crate::OBSERVATION_NOT_ANSWERED`] is the same shape one road over.
pub const READ_ANSWERED_THIS_LOCATOR_IS_ABSENT: &str =
    "gx: read this as \"the object is not there\", not as \"the server would not \
     answer\" -- the server answered this read, and its answer is that the locator holds nothing. \
     The clause around this one is `gx-substrate`'s single word for both facts and this build may \
     not reword it, which is why this sentence is here. gx mediates a change to an object that \
     already exists: the compare-and-set has to name a prior state and there is none, so no verdict \
     was reached and no effect was sent. What to fix: create the object outside gx and make the \
     call again, or name a locator that exists -- and if this locator held something a moment ago, \
     `gx log` is where this session's own admitted removals are recorded (`req/320` M-01)";

/// 🔴 **`req/320` M-01** — [`READ_ANSWERED_THIS_LOCATOR_IS_ABSENT`], applied to a read that failed.
///
/// The one function both consuming sites call, for `crate::cas`'s own reason: two call sites are two
/// places for the next lane to widen one of them. Anything that is not the answered-absent preimage
/// is returned exactly as it arrived — this adds a sentence to one fact and touches no other.
pub(crate) fn name_the_preimage(error: Error) -> Error {
    match error {
        Error::Unreadable { locator, detail }
            if crate::transport::read_answered_absent(&detail) =>
        {
            Error::Unreadable {
                locator,
                detail: format!("{detail}. {READ_ANSWERED_THIS_LOCATOR_IS_ABSENT}"),
            }
        }
        other => other,
    }
}

/// 🔴 **R28 / `req/334` M-03** — the four facts the late-escrow completion road answers with one
/// word, told apart by name.
///
/// # The defect this closes
///
/// `InverseCompletion::complete_inverse` returns `Ok(None)` on four separate facts and the engine
/// folds every one of them to `InverseStatus::Unavailable`. That word is defined by 42 §3.12 as
/// "`invert()` returned `None`" — *gx asked and there was no inverse to build*, a property of the
/// change. Fact ② is not that: `invert()` answered `Some`, the escrow was written, and what failed
/// is that the **observation did not carry a member the declaration named**. The operator's next
/// action differs — the first is give up, the second is fix the read face and undo becomes possible
/// — and until this lane both were one word with no way to tell them apart.
///
/// Worse than the fold: in fact ② `ArgSource::resolve_from_observation` has **already composed** a
/// human sentence naming the pointer and what was wrong with it, and the road dropped it on the
/// floor (`Err(_unresolvable) => return Ok(None)`). This crate's own header states the rule that
/// broke: a refusal has to say which of the facts it is.
///
/// # What this lane did and did not change
///
/// The `Ok(None)` **is kept**, at all four arms. It is a deliberate fail-safe — `req/38` §99 ruling
/// 2 ④ — and nothing on this road may abort a commit whose apply already succeeded. Widening the
/// trait's return type, or minting a seventh `InverseStatus` word beside `Unavailable` (which is
/// what R8 / `req/234` B-5 did for a different fact, and is the precedent), is a **wire ruling** and
/// not a repair lane's. What changed is that each arm now names its fact and composes a sentence,
/// and the sentence reaches a reader through [`crate::CallLog::note`] instead of the floor. The
/// remaining fold is declared in `docs/LIMITS.md` rather than left for the next audit to find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionRefused {
    /// ① The escrow-time arguments are not the JSON object this adapter wrote.
    ArgumentsAreNotTheObjectThisAdapterWrote,
    /// ② The observation did not carry a member the declaration named. **The one that is not
    /// `Unavailable` in 42 §3.12's sense**: `invert()` answered `Some` before this point.
    ObservationDidNotCarryADeclaredMember,
    /// ③ The completed arguments would not serialise back to JSON.
    CompletedArgumentsWouldNotSerialise,
    /// ④ The completed payload is over **M4-21**'s ceiling.
    CompletedPayloadIsOverTheCeiling,
}

impl CompletionRefused {
    /// The four, in declaration order. One arm per entry and no `_`, the shape
    /// `gx_engine::NotAttemptedBecause::ALL_CAUSES` carries for the same reason.
    pub const ALL_FACTS: [&'static str; 4] = [
        "ArgumentsAreNotTheObjectThisAdapterWrote",
        "ObservationDidNotCarryADeclaredMember",
        "CompletedArgumentsWouldNotSerialise",
        "CompletedPayloadIsOverTheCeiling",
    ];

    /// Which of [`CompletionRefused::ALL_FACTS`] this is.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            CompletionRefused::ArgumentsAreNotTheObjectThisAdapterWrote => {
                "ArgumentsAreNotTheObjectThisAdapterWrote"
            }
            CompletionRefused::ObservationDidNotCarryADeclaredMember => {
                "ObservationDidNotCarryADeclaredMember"
            }
            CompletionRefused::CompletedArgumentsWouldNotSerialise => {
                "CompletedArgumentsWouldNotSerialise"
            }
            CompletionRefused::CompletedPayloadIsOverTheCeiling => {
                "CompletedPayloadIsOverTheCeiling"
            }
        }
    }

    /// The sentence for this fact, with whatever the road learned appended.
    ///
    /// `detail` is where fact ②'s already-composed sentence goes — the pointer and the reason
    /// `resolve_from_observation` named — which is the half this road used to discard.
    #[must_use]
    pub fn sentence(&self, detail: &str) -> String {
        let body = match self {
            CompletionRefused::ArgumentsAreNotTheObjectThisAdapterWrote =>
                "the escrowed partial does not carry the resolved JSON object this adapter writes \
                 into a partial, so there is nothing to fill in. What to fix: nothing at the call \
                 site -- this is a partial gx did not write, and `gx log` is where this project's \
                 own escrow records are",
            CompletionRefused::ObservationDidNotCarryADeclaredMember =>
                "the inverse was derivable and was escrowed; what failed is that the applied call's \
                 observation did not carry a member the restore declaration names. This is **not** \
                 42 §3.12's `Unavailable` (\"`invert()` returned `None`\") even though the engine \
                 records that word: `invert()` answered `Some` before this point, so the undo is \
                 not impossible -- it is unfinished",
            CompletionRefused::CompletedArgumentsWouldNotSerialise =>
                "the filled arguments would not serialise back to JSON. What to fix: the value the \
                 observation carried at the declared pointer is not one this adapter can put back \
                 into a call",
            CompletionRefused::CompletedPayloadIsOverTheCeiling =>
                "the completed inverse is larger than M4-21's ceiling for an escrowed payload, so \
                 it was not minted. What to fix: declare a restore whose body is bounded, or undo \
                 this change outside gx",
        };
        format!(
            "gx could not complete the escrowed inverse ({}): {body}. {detail}",
            self.kind()
        )
    }
}

/// The MCP adapter (41 §2, 41 §4, FR-046).
///
/// Three fields, and each of them is a seam a deployment fills:
///
/// * the **transport** is the wire (`transport.rs` argues why it is not linked in here);
/// * the **catalogue** is "which tools can be undone, and by what", which only the party running the (sem: SEM-gx-adapter-mcp-103)
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

    /// What this adapter can undo, for a report line that would otherwise say "conformant" about a run (sem: SEM-gx-adapter-mcp-104)
    /// against an empty catalogue.
    #[must_use]
    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    /// 🔴 **C-25 / DR-46-9 A-4** — "can this call be undone?", in three values, for one planned
    /// call against the world as it is now.
    ///
    /// 11 §5-2 C-25 makes this the product's first-class output rather than the undo itself, and
    /// three values are what the question actually has: an inverse was built
    /// ([`Reversibility::True`]), there is none to build ([`Reversibility::False`]), or the prior
    /// could not be read so nobody found out ([`Reversibility::Unknown`]). Folding the last two
    /// would report "irreversible" about a change nothing established anything about.
    ///
    /// 🔴 **This is not on the commit road.** `SubstrateAdapter::invert` is what T-10b calls and
    /// its signature is 41 §4's; this method exists for the surfaces that report reversibility to
    /// a person, and it performs the same **one** read the escrow performs. Calling it *beside* a
    /// commit would double the reads — so nothing in this workspace does, and
    /// `tests/github16_read_by_tool.rs` counts the arrivals that prove it.
    ///
    /// # Errors
    /// [`crate::invert`]'s — including the `Unreadable` refusal a failed read raises under the
    /// default posture, which is the same answer the commit would get.
    pub fn reversibility(
        &self,
        delta: &PlannedDelta,
        pre: &ObjectSnapshot,
    ) -> Result<Reversibility> {
        crate::invert::invert_with_verdict(self.transport.as_ref(), &self.catalogue, delta, pre)
            .map(|outcome| outcome.verdict())
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
    /// 🔴 **DR-46-16** (`req/38` §218 ruling 1): *which* read this is, is the deployment's to
    /// declare. [`crate::cas::read_subject`] takes the `$cas_read` road when a declared prefix
    /// matches this resource and `resources/read` when none does — one read either way, and the
    /// shape of what comes back (`{digest, ReprKind::Bytes}`) is the same value on both.
    ///
    /// # Errors
    /// [`Error::NotAPosition`] for a locator that is not one, and whatever the transport says about a
    /// resource it will not answer for.
    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        let position = locator::parse(locator)?;
        // 🔴 **`req/320` M-01** — site 1 of the three that consume a declared read. See
        // [`name_the_preimage`].
        let contents =
            crate::cas::read_subject(self.transport.as_ref(), &self.catalogue, &position)
                .map_err(name_the_preimage)?;
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
    /// contract 3 requires this value to **change when the state changes**, so its subject has to be (sem: SEM-gx-adapter-mcp-105)
    /// something a proxy can read; a server has no digest. What the server is instead is the
    /// **footprint** ([`crate::commutation`]), and holding those two apart is this adapter's one real
    /// departure from `gx-adapter-git`, where a branch happens to be both.
    ///
    /// An over-long scope becomes a digest line before it reaches [`Fingerprint::new`] (**M4H1-2**,
    /// [`elide_scope`]); the bound itself is gx-core's and refuses anything that arrives past it. That
    /// is the single road **req/98** §3-4, reservation 6, asks the M7 adapters to take. (sem: SEM-gx-adapter-mcp-106)
    ///
    /// # Errors
    /// [`Error::NotAPosition`], whatever the transport says, and `ScopeTooLong` through [`elide_scope`].
    /// 🔴 **DR-46-16**: the read goes through [`crate::cas::read_subject`], for the reason
    /// [`Self::snapshot`] gives one method up — the fingerprint DR-43-1 (a) compares at undo time
    /// has to be a value the CAS road can produce on a tools-only server, or that server has no
    /// compare-and-set at all.
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        let position = locator::parse(snap.locator())?;
        // 🔴 **`req/320` M-01** — site 2 of three. See [`name_the_preimage`].
        let contents =
            crate::cas::read_subject(self.transport.as_ref(), &self.catalogue, &position)
                .map_err(name_the_preimage)?;
        Ok(Fingerprint::new(
            SubstrateKind::Mcp,
            elide_scope(position.locator())?,
            content_digest(&contents),
        )?)
    }

    /// Delegates to [`crate::apply`], which is the only module in this crate that reaches a transport's
    /// `call` -- the second premise of AC-051.
    /// 🔴 **DR-46-16**: the catalogue is handed down as well, because the post-apply observation is
    /// the third of the CAS half's three reads and has to take the same road the other two took.
    /// A free function's arguments are this crate's own business — `SubstrateAdapter`'s seven
    /// signatures are untouched (**N-08**, `gx-substrate/tests/adapter_spec.rs`), which is the
    /// whole reason `req/38` §218 adopted design fork (b).
    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        crate::apply::apply(
            self.transport.as_ref(),
            &self.catalogue,
            self.log.as_ref(),
            delta,
        )
    }

    /// Delegates to [`crate::invert`]. **E-M4-30**: the escrowed inverse is constructed **before**
    /// `apply` (43 T-10b), because the body it carries is the resource's prior contents.
    /// 🔴 **E-DR4626-1 (DR-46-26)** — the whole outcome crosses now, and the crate-private
    /// `invert_with_verdict` is what it crosses from.
    ///
    /// Until this window the trait method called [`crate::invert::invert`], which threw away two of
    /// the three things `invert_with_verdict` had already worked out in the **same single read**:
    /// C-25's verdict and the `{digest, locator}` of the prior. Both were computed and both were
    /// dropped at this line — `gx_engine::store::InverseStatus::Undetermined`'s documentation named
    /// this exact call as the block it was waiting on. Nothing new is read; the same one server
    /// round trip `req/38` §195 clause ⑤ bounds is the same one round trip.
    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
        crate::invert::invert_with_verdict(self.transport.as_ref(), &self.catalogue, delta, pre)
    }

    /// Delegates to [`crate::commutation`], which compares **servers** and touches no transport
    /// (**M4-25, adopted (a)**, AC-052, AC-053). (sem: SEM-gx-adapter-mcp-107)
    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        crate::commutation::commutation(a, b)
    }
}

/// 🔴 Two-phase escrow completion (`req/38` §98, ruling 1 / §99, ruling 2-②) -- a **separate optional (sem: SEM-gx-adapter-mcp-108)
/// trait**, not an eighth method: N-08 fixes `SubstrateAdapter` at seven and `adapter_spec.rs`
/// measures it, so this capability rides beside the boundary rather than through it. An engine
/// that registers this value (`Engine::register_completion`) can complete a `Pending` escrow; one
/// that does not behaves exactly as before.
impl InverseCompletion for McpAdapter {
    /// A partial escrow announces itself in the payload: the op's `pending` map is non-empty.
    ///
    /// Pure over the payload (P-6: this adapter is the only reader of its grammar) — no transport,
    /// no catalogue lookup, so the answer a recovery gets from journal + blob store alone is the
    /// answer the live engine got.
    fn needs_completion(&self, inverse: &PlannedDelta) -> Result<bool> {
        if inverse.substrate() != &SubstrateKind::Mcp {
            return Err(Error::ForeignDelta {
                expected: SubstrateKind::Mcp,
                got: inverse.substrate().clone(),
            });
        }
        let decoded = McpDelta::decode(inverse.payload())?;
        let op = decoded
            .ops()
            .first()
            .expect("decode refuses the empty sequence");
        Ok(!op.pending().is_empty())
    }

    /// Fill the pending members from the journalled observation and mint the complete inverse.
    ///
    /// `Ok(None)` for every legitimate non-construction — the observation is not JSON, the pointer
    /// resolves to nothing, the derived form's text ends in no `/<digits>` (`req/38` §99, ruling 1's (sem: SEM-gx-adapter-mcp-109)
    /// fail-safe: a derivation failure is a fail-safe by design), the completed payload is over **M4-21**'s ceiling, (sem: SEM-gx-adapter-mcp-110)
    /// or the escrow-time arguments are not the JSON object this adapter wrote. The engine folds
    /// `Err` the same way (§99, ruling 2-④), so nothing on this road can abort a commit whose apply (sem: SEM-gx-adapter-mcp-111)
    /// already succeeded.
    fn complete_inverse(
        &self,
        partial: &PlannedDelta,
        observation: &[u8],
    ) -> Result<Option<PlannedDelta>> {
        if partial.substrate() != &SubstrateKind::Mcp {
            return Err(Error::ForeignDelta {
                expected: SubstrateKind::Mcp,
                got: partial.substrate().clone(),
            });
        }
        let decoded = McpDelta::decode(partial.payload())?;
        let op = decoded
            .ops()
            .first()
            .expect("decode refuses the empty sequence");
        if op.pending().is_empty() {
            // Not a partial escrow: the complete delta is already the answer.
            return Ok(Some(partial.clone()));
        }
        let Ok(serde_json::Value::Object(mut arguments)) =
            serde_json::from_slice::<serde_json::Value>(op.arguments())
        else {
            // ① A partial this adapter wrote carries the template's resolved JSON object; anything
            // else is "a legitimate construction is impossible" read one phase later -- fail-safe, not a crash. (sem: SEM-gx-adapter-mcp-112)
            self.log.note(
                &CompletionRefused::ArgumentsAreNotTheObjectThisAdapterWrote
                    .sentence("The escrowed arguments did not decode as a JSON object."),
            );
            return Ok(None);
        };
        for (name, source) in op.pending() {
            match source.resolve_from_observation(observation) {
                Ok(value) => {
                    arguments.insert(name.clone(), value);
                }
                // ② The observation does not carry what the declaration names (a moved URL shape,
                // a missing member): the same family as E-M4-32's `Ok(None)`, one phase later.
                //
                // 🔴 **R28 / `req/334` M-03** — `unresolvable` is the sentence
                // `ArgSource::resolve_from_observation` composed, naming the pointer and what was
                // wrong with it. Until this lane it was bound to `_unresolvable` and dropped. It is
                // the one fact of the four an operator can act on, so it is the one that most had
                // to survive.
                //
                // The remedy is named at the site rather than inside `sentence()` because it is a
                // property of **this** fact and of no other of the four: what an operator acts on
                // is the read face, which is exactly what `crate::OBSERVATION_NOT_ANSWERED` already
                // says for the sibling condition one road over (gx made the call and the read-back
                // did not answer at all). Named rather than paraphrased, so this build cannot grow
                // two accounts of one condition.
                Err(unresolvable) => {
                    self.log.note(
                        &CompletionRefused::ObservationDidNotCarryADeclaredMember.sentence(
                            &format!(
                                "The declaration names {name:?} and resolving it answered: \
                                 {unresolvable}. The read face is the thing to act on: \
                                 {OBSERVATION_NOT_ANSWERED}"
                            ),
                        ),
                    );
                    return Ok(None);
                }
            }
        }
        let Ok(arguments) = serde_json::to_vec(&serde_json::Value::Object(arguments)) else {
            // ③ The filled arguments will not go back to JSON.
            self.log.note(
                &CompletionRefused::CompletedArgumentsWouldNotSerialise
                    .sentence("The filled argument object would not re-serialise."),
            );
            return Ok(None);
        };
        let payload = McpDelta::one(McpOp::call(
            op.locator().to_string(),
            op.tool().to_string(),
            arguments,
        ))
        .encode()?;
        if payload.len() > MAX_INVERSE_PAYLOAD_BYTES {
            // ④ M4-21's ceiling, measured against the *completed* body.
            self.log.note(
                &CompletionRefused::CompletedPayloadIsOverTheCeiling.sentence(&format!(
                    "The completed payload is {} bytes against a ceiling of \
                     {MAX_INVERSE_PAYLOAD_BYTES}.",
                    payload.len()
                )),
            );
            return Ok(None);
        }
        PlannedDelta::new(SubstrateKind::Mcp, payload).map(Some)
    }
}

/// 🔴 **DR-46-33 / DR-46-28** — the input-generation declaration (`req/38` §413), a **separate
/// optional trait** for `InverseCompletion`'s reason: N-08 fixes `SubstrateAdapter` at seven and
/// `adapter_spec.rs` measures it, so where a deployment says its inputs come from rides beside the
/// boundary rather than through it. An engine that registers this value
/// (`Engine::register_input_stage_declaration`) attests the catalogue's declaration; one that does
/// not attests `unknown`, exactly as before this lane.
impl InputStageDeclaration for McpAdapter {
    /// The `$determinism_boundary` reserved slot this catalogue was built with
    /// ([`crate::Catalogue::declared_input_generation`]). A property of the deployment, so it is a
    /// read of the catalogue in hand and touches no transport (P-6): the answer a recovery gets
    /// from the same catalogue is the answer the live engine got.
    fn declared_input_stage(&self) -> gx_core::BoundaryStage {
        self.catalogue.declared_input_generation()
    }
}
