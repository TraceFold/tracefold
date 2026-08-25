// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! An in-memory `SubstrateAdapter` and the [`Fixture`] over it, so that hand 3's harness can be run
//! before hand 4 exists. (sem: SEM-gx-substrate-conformance-121, SEM-gx-substrate-conformance-122,
//! SEM-gx-substrate-conformance-123, SEM-gx-substrate-conformance-124, SEM-gx-substrate-conformance-125,
//! SEM-gx-substrate-conformance-126, SEM-gx-substrate-conformance-127, SEM-gx-substrate-conformance-128,
//! SEM-gx-substrate-conformance-129, SEM-gx-substrate-conformance-130, SEM-gx-substrate-conformance-131,
//! SEM-gx-substrate-conformance-132, SEM-gx-substrate-conformance-133)
//!
//! req/69 §6.2 hand 3: "since this hand has no adapter implementation, red-first with a mock adapter
//! (test-only, does not read fs)".
//! This is that mock. It is a **test double**, not a preview of `gx-adapter-fs`: it keeps a
//! `BTreeMap` in a `Mutex`, opens no file, resolves no path and reads no clock, and every decision
//! in it that an fs adapter would have to think about is marked as the mock's own.
//!
//! # What the mock's payload grammar is, and why it is a sequence
//!
//! `crates/gx-substrate/src/lib.rs`'s `# Composite deltas (normative)` section (**M4-07, adopted
//! (c)**) makes a composite delta a **free monoid** -- "a sequence of single-file operations" with
//! "the concatenation of the sequence is the witness of composition". The
//! mock's payload is therefore a concatenation of framed write operations:
//!
//! ```text
//! op      := u8 locator_len | locator bytes | u32le content_len | content bytes
//! payload := op*
//! ```
//!
//! Concatenating two payloads is composing two changes, associatively, with the empty payload as
//! the unit -- and nothing above the adapter can tell, because the payload is opaque (P-6). No layer
//! outside this file parses it, which is what `tests/opacity.rs` measures of the five crates below
//! the boundary.
//!
//! # Two things the mock had to decide, and both are seams in the canon
//!
//! * **`applied_at`.** M4-17 (a) rules "`applied_at` is injected by the engine", but 41 §4's
//!   `apply(&self, delta)` has no parameter an engine could inject through, and the adapter is the
//!   caller of [`AppliedDelta::new`]. The mock writes `Timestamp(0)`, which is the placeholder
//!   gx-gate already uses for the same reason ("gx-gate writes `Timestamp(0)` into an escalation
//!   ticket as the placeholder an engine overwrites", `gx_core::Error::CreatedAtNegative`'s
//!   documentation). The seam is raised in `req/72` §2 rather than closed here.
//! * **The escrow ceiling.** M4-21 (a) fixes the *form* -- "declare the inverse delta payload's
//!   ceiling in **one constant place**; exceeding it makes `invert` = `Ok(None)`" -- and leaves the
//!   value to hand 5. [`MOCK_INVERSE_CEILING`] is the mock's own and says so; it exists to give
//!   contract 5 a real `Ok(None)`, which req/69 §4 M4-21 calls "AC-048's first real reason for
//!   `None`".

// Every binary that says `mod support;` compiles the whole file, and no binary uses all of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use gx_canon::cid::{self, Domain};
use gx_core::{
    Actor, ChangeContext, Cid, Commutation, DeltaRef, Fingerprint, GoalBytes, Intent, ObjectId,
    ObjectSnapshot, ReprKind, SubstrateKind, Timestamp,
};
use gx_substrate::{AppliedDelta, Error, InvertOutcome, PlannedDelta, Result, SubstrateAdapter};
use gx_substrate_conformance::Fixture;

/// The locator the fixture points the harness at. Already normalised (41 §4's `snapshot` receives
/// one: H-2 / E-M4-12).
pub const SUBJECT: &str = "/mock/x";

/// A second locator, so that 51 §7's commuting case has two changes that touch different things.
pub const OTHER: &str = "/mock/y";

/// A locator whose content is larger than the mock's escrow ceiling, for contract 5.
pub const BIG: &str = "/mock/big";

/// The mock's own escrow ceiling. **Not** a value any ruling fixes: M4-21 leaves that to hand 5, and
/// this exists so that `Ok(None)` has a real cause here rather than a stubbed one.
pub const MOCK_INVERSE_CEILING: usize = 1_024;

/// The digest the mock takes of a piece of content.
///
/// Through gx-canon, because 41 §6 admits no second place where bytes become a digest. `Domain::Leaf`
/// rather than [`cid::compute`]: content is bytes and not a projected value, so there is no
/// `IdentityView` to go through, and the mint is the road E-M2-12 opened for exactly that case.
fn content_digest(bytes: &[u8]) -> Cid {
    cid::mint(Domain::Leaf, &[bytes])
}

/// One write, framed.
fn frame(locator: &str, content: &[u8]) -> Vec<u8> {
    let l = locator.as_bytes();
    let mut out = Vec::with_capacity(1 + l.len() + 4 + content.len());
    out.push(u8::try_from(l.len()).expect("the mock's locators are short"));
    out.extend_from_slice(l);
    out.extend_from_slice(
        &u32::try_from(content.len())
            .expect("the mock's blobs are small")
            .to_le_bytes(),
    );
    out.extend_from_slice(content);
    out
}

/// Read a payload back into its sequence of writes.
fn unframe(payload: &[u8]) -> std::result::Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut rest = payload;
    while !rest.is_empty() {
        let (&len, tail) = rest
            .split_first()
            .ok_or("a payload that ends inside a header")?;
        let len = usize::from(len);
        if tail.len() < len + 4 {
            return Err("a payload that ends inside a locator".to_string());
        }
        let locator =
            String::from_utf8(tail[..len].to_vec()).map_err(|_| "a locator that is not UTF-8")?;
        let mut size = [0u8; 4];
        size.copy_from_slice(&tail[len..len + 4]);
        let size = usize::try_from(u32::from_le_bytes(size)).expect("usize is at least 32 bits");
        let body = &tail[len + 4..];
        if body.len() < size {
            return Err("a payload that ends inside a body".to_string());
        }
        out.push((locator, body[..size].to_vec()));
        rest = &body[size..];
    }
    Ok(out)
}

/// An adapter over a map, which is the smallest thing that can be asked all seven questions.
pub struct MockAdapter {
    store: Mutex<BTreeMap<String, Vec<u8>>>,
    /// Residuals this adapter has minted, so that a `Conflicts{residual}` CID has a referent
    /// (**M4-14** / **E-M4-8**). A real adapter has no store -- that is the engine's, in M5 -- and
    /// this exists to give hand 3 the shape of the question.
    residuals: Mutex<BTreeMap<Cid, PlannedDelta>>,
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Mutex::new(BTreeMap::new()),
            residuals: Mutex::new(BTreeMap::new()),
        }
    }

    fn read(&self, locator: &str) -> Option<Vec<u8>> {
        self.store
            .lock()
            .expect("the mock's lock")
            .get(locator)
            .cloned()
    }

    fn write(&self, locator: &str, content: Vec<u8>) {
        self.store
            .lock()
            .expect("the mock's lock")
            .insert(locator.to_string(), content);
    }

    /// Build a delta that writes `content` at `locator` -- the mock's one-operation sequence.
    ///
    /// # Errors
    /// Whatever [`PlannedDelta::new`] returns when the projection has no canonical form.
    pub fn write_delta(&self, locator: &str, content: &[u8]) -> Result<PlannedDelta> {
        PlannedDelta::new(self.kind(), frame(locator, content))
    }

    fn ours(&self, delta: &PlannedDelta) -> Result<Vec<(String, Vec<u8>)>> {
        if delta.substrate() != &self.kind() {
            return Err(Error::ForeignDelta {
                expected: self.kind(),
                got: delta.substrate().clone(),
            });
        }
        unframe(delta.payload()).map_err(|detail| Error::PayloadUnreadable { detail })
    }

    /// The snapshot of a locator, with its `id` minted from its own projection.
    fn snapshot_of(&self, locator: &str, content: &[u8]) -> Result<ObjectSnapshot> {
        let mut snapshot = ObjectSnapshot::new(
            ObjectId(Cid([0u8; 32])),
            self.kind(),
            locator.to_string(),
            content_digest(content),
            ReprKind::Bytes,
        );
        // 42 §1.3 row 1 excludes `id` from the projection, so the placeholder is not in the
        // preimage -- the same argument `PlannedDelta::new` makes about `reference`.
        let id = cid::compute(&snapshot).map_err(|e| Error::NotDigestible {
            detail: e.to_string(),
        })?;
        snapshot = ObjectSnapshot::new(
            ObjectId(id),
            self.kind(),
            locator.to_string(),
            content_digest(content),
            ReprKind::Bytes,
        );
        Ok(snapshot)
    }
}

impl SubstrateAdapter for MockAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Custom("mock".to_string())
    }

    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        let content = self.read(locator).ok_or_else(|| Error::Unreadable {
            locator: locator.to_string(),
            detail: "the mock holds nothing at this locator".to_string(),
        })?;
        self.snapshot_of(locator, &content)
    }

    /// Ignores everything about the substrate: the answer is a function of the pair and of nothing
    /// else, which is E-M4-4's determinism and E-M4-29's "zero writes to the substrate" at the same
    /// time.
    fn plan(&self, intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        if intent.locator().is_empty() {
            return Err(Error::NotPlannable {
                detail: "an intent with no locator names nothing to change".to_string(),
            });
        }
        self.write_delta(intent.locator(), &intent.goal().0)
    }

    /// The scope is the locator alone -- the narrow end of 42 §3.5's range, and the mock says so
    /// rather than implying that a wider one was considered.
    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        let content = self.read(snap.locator()).ok_or_else(|| Error::Unreadable {
            locator: snap.locator().to_string(),
            detail: "the mock holds nothing at this locator".to_string(),
        })?;
        // `Fingerprint::new` refuses a scope past `gx_core::MAX_SCOPE_BYTES` (M4H1-2); the mock's
        // locators are short, and the refusal crosses as `Error::Core` if one ever is not.
        Ok(Fingerprint::new(
            self.kind(),
            snap.locator().to_string(),
            content_digest(&content),
        )?)
    }

    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        let ops = self.ours(delta)?;
        let last = ops
            .last()
            .ok_or_else(|| Error::ApplyFailed {
                detail: "an empty sequence changes nothing and names nothing".to_string(),
            })?
            .0
            .clone();
        for (locator, content) in ops {
            self.write(&locator, content);
        }
        let content = self.read(&last).expect("just written");
        Ok(AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(self.kind(), last, content_digest(&content))?,
            content_digest(&content),
            // M4-17: the adapter may not read a clock, and 41 §4 gives it nothing to be handed one
            // through. The placeholder is gx-gate's; the seam is req/72 §2's.
            Timestamp(0),
        ))
    }

    /// Reads the state at call time, which is why 43 T-10b calls this **before** `apply`.
    ///
    /// The mock cannot reconstruct old content from `pre`: an `ObjectSnapshot` carries a digest, not
    /// a body. That is not a limitation of the mock -- it is why 42 §5 escrows the inverse "keeps the
    /// body payload" and why the harness runs the round trip in T-10b's order.
    fn invert(&self, delta: &PlannedDelta, _pre: &ObjectSnapshot) -> Result<InvertOutcome> {
        let ops = self.ours(delta)?;
        let mut payload = Vec::new();
        for (locator, _) in ops {
            let old = self.read(&locator).unwrap_or_default();
            if old.len() > MOCK_INVERSE_CEILING {
                // M4-21 (a): "exceeding the ceiling makes `invert` = `Ok(None)`" -- a real reason,
                // not a stub.
                //
                // 🔴 **DR-46-26** — `Reversibility::False`, and no read attested. The mock's
                // "read" is a map lookup in this process, not a read of a substrate through a
                // transport, so attesting it would put a locator in a receipt about an object
                // nothing outside this test ever held.
                return Ok(InvertOutcome::none(Vec::new()));
            }
            payload.extend_from_slice(&frame(&locator, &old));
        }
        Ok(InvertOutcome::inverted(
            PlannedDelta::new(self.kind(), payload)?,
            Vec::new(),
        ))
    }

    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        let left = self.ours(a)?;
        let right = self.ours(b)?;
        let shared: Vec<(String, Vec<u8>)> = left
            .iter()
            .filter(|(l, _)| right.iter().any(|(r, _)| r == l))
            .cloned()
            .collect();
        if shared.is_empty() {
            return Ok(Commutation::Commutes);
        }
        // The obstruction, as a delta of its own. E-M4-8's storage row is what stops the CID from
        // naming nothing, so the mock keeps the body it just minted.
        let mut payload = Vec::new();
        for (locator, content) in shared {
            payload.extend_from_slice(&frame(&locator, &content));
        }
        let residual = PlannedDelta::new(self.kind(), payload)?;
        let reference = residual.reference().clone();
        self.residuals
            .lock()
            .expect("the mock's lock")
            .insert(reference.cid, residual);
        Ok(Commutation::Conflicts {
            residual: reference,
        })
    }
}

/// The fixture 51 §7's harness is run against.
pub struct MockFixture {
    adapter: MockAdapter,
}

impl Default for MockFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl MockFixture {
    #[must_use]
    pub fn new() -> Self {
        let fixture = Self {
            adapter: MockAdapter::new(),
        };
        fixture.reset().expect("the mock resets");
        fixture
    }

    fn intent_for(locator: &str, goal: &[u8]) -> Intent {
        Intent::new(
            SubstrateKind::Custom("mock".to_string()),
            locator.to_string(),
            GoalBytes(goal.to_vec()),
            ChangeContext::Evidence,
            Actor::Human {
                key: "conformance".to_string(),
            },
        )
    }

    /// What the intent asks the object to become. Used by [`Fixture::promised_target`].
    pub const GOAL: &'static [u8] = b"after";

    #[must_use]
    pub fn mock(&self) -> &MockAdapter {
        &self.adapter
    }
}

impl Fixture for MockFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        &self.adapter
    }

    fn locator(&self) -> String {
        SUBJECT.to_string()
    }

    fn intent(&self) -> Intent {
        Self::intent_for(SUBJECT, Self::GOAL)
    }

    fn reset(&self) -> Result<()> {
        let mut store = self.adapter.store.lock().expect("the mock's lock");
        store.clear();
        store.insert(SUBJECT.to_string(), b"before".to_vec());
        store.insert(OTHER.to_string(), b"beside".to_vec());
        store.insert(BIG.to_string(), vec![b'z'; MOCK_INVERSE_CEILING + 1]);
        Ok(())
    }

    fn disturb(&self) -> Result<()> {
        self.adapter
            .write(SUBJECT, b"somebody else was here".to_vec());
        Ok(())
    }

    /// A change at [`BIG`], whose inverse would have to carry more than the ceiling.
    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        let pre = self.adapter.snapshot(BIG).ok()?;
        let delta = self
            .adapter
            .plan(&Self::intent_for(BIG, b"small"), &pre)
            .ok()?;
        Some((delta, pre))
    }

    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        Some((
            self.adapter.write_delta(SUBJECT, b"one").ok()?,
            self.adapter.write_delta(OTHER, b"two").ok()?,
        ))
    }

    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        Some((
            self.adapter.write_delta(SUBJECT, b"one").ok()?,
            self.adapter.write_delta(SUBJECT, b"two").ok()?,
        ))
    }

    /// The digest the intent asks for, computed from the goal rather than from the store.
    ///
    /// Two roads to one number: this one goes through the goal bytes, and `apply`'s
    /// `resulting_digest` goes through what was actually written. They agree only if the adapter
    /// wrote what it promised, which is what L5 asks. It is a weak instance of M4-06 -- both roads
    /// are in the same file -- and the strong one is hand 5's, over `gx-adapter-fs`.
    fn promised_target(&self) -> Option<Cid> {
        Some(content_digest(Self::GOAL))
    }

    /// Collapses repeated separators and nothing else.
    ///
    /// Deliberately the smallest normalisation that is one: E-M4-12's five clauses are the fs
    /// adapter's obligation in hand 4, and writing them here would be hand 4's work done in a mock,
    /// where no `≈` documentation obligation (42 §2.3) applies to it.
    fn normalise(&self, locator: &str) -> Option<String> {
        let mut out = String::with_capacity(locator.len());
        let mut last_was_sep = false;
        for c in locator.chars() {
            if c == '/' && last_was_sep {
                continue;
            }
            last_was_sep = c == '/';
            out.push(c);
        }
        Some(out)
    }

    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        vec![
            ("/mock//x".to_string(), SUBJECT.to_string()),
            ("/mock///x".to_string(), "/mock//x".to_string()),
        ]
    }

    fn resolve(&self, reference: &DeltaRef) -> Option<PlannedDelta> {
        self.adapter
            .residuals
            .lock()
            .expect("the mock's lock")
            .get(&reference.cid)
            .cloned()
    }
}

/// One lie an adapter (or the fixture over it) can tell, named by the obligation it breaks.
///
/// 🔴 **K-3 / K-10** (`req/38` §35). Every check in this harness had, until now, exactly one kind of
/// subject: an adapter that meets its obligations. req/76 measured what that costs from two
/// directions at once -- line coverage **69.51%**, under 51 §14's ≥80, with `contracts.rs` at 63.43
/// and `laws.rs` at 67.80; and **7 of 15** `cargo mutants` survivors sitting in the judgement
/// functions themselves (`is_conformant → true`, `meets_51_7 → true`, `law_5`'s
/// `resulting_digest == target` guard → `true`). Both numbers say the same thing: **the code that
/// decides whether an adapter passed had no negative control**, so a harness that answered "pass" to
/// everything would have been reported as green by its own suite.
///
/// The subject that closes it is one deliberately broken fixture, asked in turn to break each
/// obligation. A single flaw would move the coverage number and leave most arms unmeasured; a flaw
/// per obligation is what makes "failed" a thing this harness is known to be able to say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flaw {
    /// Nothing can be read, so every obligation fails at its first step.
    RefusesEverything,
    /// The products name a substrate the adapter does not speak for (K1, contract 1).
    NamesAnotherSubstrate,
    /// The snapshot answers about a position nobody asked about (contract 1).
    AnswersAboutAnotherLocator,
    /// The substrate moves and the adapter reports the same state (contract 1, contract 3, L4).
    Deaf,
    /// Two plans of one `(intent, snapshot)` differ (contract 2, L1).
    PlansDifferentlyEachTime,
    /// `plan` writes to the substrate (contract 2's "no side effects", E-M4-29).
    WritesWhilePlanning,
    /// An inverse is produced for the delta the fixture calls uninvertible (contract 5).
    InventsAnInverse,
    /// The inverse applies and the object does not come back (contract 4, L3).
    UndoesNothing,
    /// Every pair is independent, including the conflicting one (contract 6, L6's reflexive case).
    AlwaysCommutes,
    /// No pair is independent, including the commuting one (contract 6).
    NeverCommutes,
    /// The retry moves the object (contract 7, L2).
    AppliesTwiceDifferently,
    /// `apply` reaches a state other than the one `plan` promised (**L5**).
    BreaksThePromise,
    /// Two fingerprints of one object are not comparable (**E-M4-15** / **E-M4-27**'s `Err`).
    ScopeDrifts,
    /// The adapter says "not yet", which is "NOT_SUPPLIED" and not "failed" (§31 M4H3-4 (b)).
    NotImplemented,
    /// The fixture cannot put its substrate back before a check.
    ResetFails,
    /// The fixture cannot move its substrate, so contract 1 and L4 lose their falsifier.
    DisturbFails,
    /// Normalising twice is not normalising once (L7's first half).
    NormalisesInconsistently,
    /// Two spellings the fixture calls equal normalise apart (L7's second half).
    CallsEquivalentWhatIsNot,
}

/// Every flaw, so that a suite can loop and no arm is added without a subject.
pub const FLAWS: [Flaw; 18] = [
    Flaw::RefusesEverything,
    Flaw::NamesAnotherSubstrate,
    Flaw::AnswersAboutAnotherLocator,
    Flaw::Deaf,
    Flaw::PlansDifferentlyEachTime,
    Flaw::WritesWhilePlanning,
    Flaw::InventsAnInverse,
    Flaw::UndoesNothing,
    Flaw::AlwaysCommutes,
    Flaw::NeverCommutes,
    Flaw::AppliesTwiceDifferently,
    Flaw::BreaksThePromise,
    Flaw::ScopeDrifts,
    Flaw::NotImplemented,
    Flaw::ResetFails,
    Flaw::DisturbFails,
    Flaw::NormalisesInconsistently,
    Flaw::CallsEquivalentWhatIsNot,
];

/// The mock, with one obligation deliberately broken.
///
/// Delegation rather than a second implementation: everything the flaw does not touch is the
/// [`MockAdapter`]'s own behaviour, so a check that fails here fails **because of the flaw** and not
/// because a hastily written double was wrong in some other way too.
pub struct BrokenAdapter {
    inner: MockAdapter,
    flaw: Flaw,
    calls: std::sync::atomic::AtomicUsize,
}

impl BrokenAdapter {
    #[must_use]
    pub fn new(flaw: Flaw) -> Self {
        Self {
            inner: MockAdapter::new(),
            flaw,
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn tick(&self) -> usize {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    fn elsewhere() -> SubstrateKind {
        SubstrateKind::Custom("elsewhere".to_string())
    }
}

impl SubstrateAdapter for BrokenAdapter {
    fn kind(&self) -> SubstrateKind {
        self.inner.kind()
    }

    fn snapshot(&self, locator: &str) -> Result<ObjectSnapshot> {
        match self.flaw {
            Flaw::RefusesEverything => Err(Error::Unreadable {
                locator: locator.to_string(),
                detail: "this adapter refuses everything, on purpose".to_string(),
            }),
            // A constant state: the substrate moves and this answers the same thing.
            Flaw::Deaf => self.inner.snapshot_of(locator, b"always the same"),
            Flaw::AnswersAboutAnotherLocator => self.inner.snapshot_of(OTHER, b"before"),
            Flaw::NamesAnotherSubstrate => {
                let real = self.inner.snapshot(locator)?;
                Ok(ObjectSnapshot::new(
                    ObjectId(Cid([0u8; 32])),
                    Self::elsewhere(),
                    real.locator().to_string(),
                    *real.digest(),
                    ReprKind::Bytes,
                ))
            }
            _ => self.inner.snapshot(locator),
        }
    }

    fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        match self.flaw {
            Flaw::PlansDifferentlyEachTime => {
                let n = self.tick();
                self.inner
                    .write_delta(intent.locator(), format!("{n}").as_bytes())
            }
            Flaw::WritesWhilePlanning => {
                self.inner
                    .write(intent.locator(), b"planning wrote this".to_vec());
                self.inner.plan(intent, pre)
            }
            Flaw::NamesAnotherSubstrate => {
                PlannedDelta::new(Self::elsewhere(), b"a foreign payload".to_vec())
            }
            _ => self.inner.plan(intent, pre),
        }
    }

    fn precondition(&self, snap: &ObjectSnapshot) -> Result<Fingerprint> {
        match self.flaw {
            // A scope that changes between two reads of one object: 42 §3.5's third answer, which
            // `cas_eq` reports as `Err` rather than as "moved" (E-M4-15).
            Flaw::ScopeDrifts => Ok(Fingerprint::new(
                self.kind(),
                format!("{}#{}", snap.locator(), self.tick()),
                *snap.digest(),
            )?),
            Flaw::Deaf => Ok(Fingerprint::new(
                self.kind(),
                snap.locator().to_string(),
                content_digest(b"always the same"),
            )?),
            Flaw::NamesAnotherSubstrate => Ok(Fingerprint::new(
                Self::elsewhere(),
                snap.locator().to_string(),
                *snap.digest(),
            )?),
            _ => self.inner.precondition(snap),
        }
    }

    fn apply(&self, delta: &PlannedDelta) -> Result<AppliedDelta> {
        match self.flaw {
            Flaw::NotImplemented => Err(Error::Unimplemented {
                method: "apply".to_string(),
                detail: "this adapter is half built, on purpose".to_string(),
            }),
            // The retry writes something else, so the second run of one delta leaves another state.
            Flaw::AppliesTwiceDifferently => {
                let n = self.tick();
                let ops = self.inner.ours(delta)?;
                let (locator, content) = ops.last().cloned().unwrap_or_default();
                let mut content = content;
                content.extend_from_slice(format!("{n}").as_bytes());
                self.inner.write(&locator, content.clone());
                Ok(AppliedDelta::new(
                    delta.reference().clone(),
                    Fingerprint::new(self.kind(), locator, content_digest(&content))?,
                    content_digest(&content),
                    Timestamp(0),
                ))
            }
            // The state is reached and the report about it is not: L5 compares the fixture's
            // promise with `resulting_digest`, so a wrong observation is enough.
            Flaw::BreaksThePromise => {
                let applied = self.inner.apply(delta)?;
                Ok(AppliedDelta::new(
                    delta.reference().clone(),
                    applied.postcondition().clone(),
                    content_digest(b"a digest of something else"),
                    Timestamp(0),
                ))
            }
            _ => self.inner.apply(delta),
        }
    }

    fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
        match self.flaw {
            // An inverse for the delta the fixture itself calls uninvertible: one of the two is
            // wrong and contract 5 is what says the harness cannot tell which.
            Flaw::InventsAnInverse => {
                let ops = self.inner.ours(delta)?;
                let (locator, _) = ops.first().cloned().unwrap_or_default();
                self.inner
                    .write_delta(&locator, b"invented")
                    .map(|d| InvertOutcome::inverted(d, Vec::new()))
            }
            Flaw::UndoesNothing => {
                let ops = self.inner.ours(delta)?;
                let (locator, _) = ops.first().cloned().unwrap_or_default();
                self.inner
                    .write_delta(&locator, b"not what was there before")
                    .map(|d| InvertOutcome::inverted(d, Vec::new()))
            }
            _ => self.inner.invert(delta, pre),
        }
    }

    fn commutation(&self, a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
        match self.flaw {
            // 43 §8 stops on `Conflicts`, so a false `Commutes` is the fail-open direction -- and
            // the reflexive case M4-25 fixes at `Conflicts` goes with it.
            Flaw::AlwaysCommutes => Ok(Commutation::Commutes),
            Flaw::NeverCommutes => {
                let residual = self.inner.write_delta(SUBJECT, b"everything conflicts")?;
                Ok(Commutation::Conflicts {
                    residual: residual.reference().clone(),
                })
            }
            _ => self.inner.commutation(a, b),
        }
    }
}

/// The fixture over [`BrokenAdapter`], and the one place a flaw of the *fixture* lives.
///
/// 🔴 It deliberately does **not** override [`Fixture::normalise`] or
/// [`Fixture::equivalent_spellings`] except for the two flaws that are about them. That is **K-10**
/// (`req/38` §35): 7 of the 15 conformance survivors are the `Fixture` trait's own default bodies,
/// unobserved because `FsFixture` overrides every one of them, and "**the default is only ever used
/// the day M7's git/mcp stand up partially implemented**". Running the harness over a fixture that
/// takes the defaults is
/// what puts a subject under them a milestone before M7 does.
pub struct BrokenFixture {
    adapter: BrokenAdapter,
    flaw: Flaw,
}

impl BrokenFixture {
    #[must_use]
    pub fn new(flaw: Flaw) -> Self {
        let fixture = Self {
            adapter: BrokenAdapter::new(flaw),
            flaw,
        };
        // Not through `Fixture::reset`: `ResetFails` is a flaw, and a fixture that could not be
        // built because of its own flaw would report the failure in the wrong place.
        fixture.populate();
        fixture
    }

    fn populate(&self) {
        let mut store = self.adapter.inner.store.lock().expect("the mock's lock");
        store.clear();
        store.insert(SUBJECT.to_string(), b"before".to_vec());
        store.insert(OTHER.to_string(), b"beside".to_vec());
        store.insert(BIG.to_string(), vec![b'z'; MOCK_INVERSE_CEILING + 1]);
    }
}

impl Fixture for BrokenFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        &self.adapter
    }

    fn locator(&self) -> String {
        SUBJECT.to_string()
    }

    fn intent(&self) -> Intent {
        MockFixture::intent_for(SUBJECT, MockFixture::GOAL)
    }

    fn reset(&self) -> Result<()> {
        if self.flaw == Flaw::ResetFails {
            return Err(Error::Unreadable {
                locator: SUBJECT.to_string(),
                detail: "this fixture cannot put its substrate back, on purpose".to_string(),
            });
        }
        self.populate();
        Ok(())
    }

    fn disturb(&self) -> Result<()> {
        if self.flaw == Flaw::DisturbFails {
            return Err(Error::Unreadable {
                locator: SUBJECT.to_string(),
                detail: "this fixture cannot move its substrate, on purpose".to_string(),
            });
        }
        self.adapter
            .inner
            .write(SUBJECT, b"somebody else was here".to_vec());
        Ok(())
    }

    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        let pre = self.adapter.inner.snapshot(BIG).ok()?;
        let delta = self
            .adapter
            .plan(&MockFixture::intent_for(BIG, b"small"), &pre)
            .ok()?;
        Some((delta, pre))
    }

    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        Some((
            self.adapter.inner.write_delta(SUBJECT, b"one").ok()?,
            self.adapter.inner.write_delta(OTHER, b"two").ok()?,
        ))
    }

    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        Some((
            self.adapter.inner.write_delta(SUBJECT, b"one").ok()?,
            self.adapter.inner.write_delta(SUBJECT, b"two").ok()?,
        ))
    }

    /// A promise the adapter is not going to keep, for [`Flaw::BreaksThePromise`]; for every other
    /// flaw the promise is the true one, so L5 is measured rather than broken twice over.
    fn promised_target(&self) -> Option<Cid> {
        Some(content_digest(MockFixture::GOAL))
    }

    /// Overridden **only** by the two flaws that are about normalisation. Every other flaw leaves
    /// the trait's default in place, which is the K-10 half of this fixture.
    fn normalise(&self, locator: &str) -> Option<String> {
        match self.flaw {
            // Not idempotent: each call adds a separator, so `normalize(normalize(l))` differs.
            Flaw::NormalisesInconsistently => Some(format!("{locator}/")),
            Flaw::CallsEquivalentWhatIsNot => Some(locator.to_string()),
            _ => None,
        }
    }

    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        match self.flaw {
            Flaw::CallsEquivalentWhatIsNot => {
                vec![(SUBJECT.to_string(), OTHER.to_string())]
            }
            _ => Vec::new(),
        }
    }
}

/// The same adapter behind a fixture that supplies none of the optional subjects.
///
/// This is what "an unmatched contract is printed as NOT_SUPPLIED" is measured with: an adapter can
/// be perfectly correct
/// and still leave three obligations unmeasured, and the report has to say so rather than counting
/// them as passes. Without this second fixture the rule would be a sentence in a doc comment.
#[derive(Default)]
pub struct BareFixture(pub MockFixture);

impl Fixture for BareFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        self.0.adapter()
    }

    fn locator(&self) -> String {
        self.0.locator()
    }

    fn intent(&self) -> Intent {
        self.0.intent()
    }

    fn reset(&self) -> Result<()> {
        self.0.reset()
    }

    fn disturb(&self) -> Result<()> {
        self.0.disturb()
    }
}
