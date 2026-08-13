//! The identity face: what a value *is*, as opposed to how it was recorded (A-4 面2).
//!
//! Spec: 42 §1.3 for the projection, 42 §1.1 for the digest, 42 §1.2 for the text form.
//! `req/38_ERRATA_2026-08-07.md` §1 A-4 is the ruling that splits this face from the wire face
//! in [`crate::cbor`], and A-1 is the ruling that leaves the `Cid` *type* in gx-core while every
//! line that computes one lives here.
//!
//! # Why the trait is declared in this crate
//!
//! [`IdentityView`] is gx-canon's, and so are its impls for gx-core's types. That is not a
//! stylistic choice: the orphan rule admits a foreign type only with a local trait, so declaring
//! the trait here is what lets `Transformation` -- a gx-core type -- have a projection without
//! gx-core learning that canonical encoding, BLAKE3, or this crate exist. The dependency edge
//! stays one-way and `ASM-16`'s cycle never forms.
//!
//! # What a projection is for
//!
//! 42 §1.3: a CID is computed over a subset of a value's fields, and the fields left out
//! contribute nothing to it. Two of the general rules bite in M1. A self-referential field
//! (`Transformation::id`, `ObjectSnapshot::id`) cannot be in its own input. And metadata is out
//! (ASM-4): `created_at` records when a change was *written down*, which is not part of what the
//! change is -- so replaying the same transformation tomorrow yields the same identity, and
//! `req/26 §1`'s rule that a signed payload carries no clock read is satisfied structurally
//! rather than by remembering to satisfy it.

use crate::{Error, Result};
use gx_core::{
    Actor, ChangeContext, Cid, DeltaRef, Fingerprint, GoalBytes, Intent, IntentId, ObjectSnapshot,
    ReprKind, Subject, SubstrateKind, Transformation, TransformationId,
};
use serde::Serialize;

/// The subset of a value's fields that its CID is computed over (42 §1.3).
///
/// One method, and the type it returns borrows from `self` rather than cloning: a projection is
/// a view of a value, and copying `parents` to hash it would be a copy per hash.
pub trait IdentityView {
    /// The projection itself. `Serialize` is the whole of the requirement -- everything else the
    /// identity face does is encoding, and encoding is [`crate::cbor`]'s job.
    type View<'a>: Serialize
    where
        Self: 'a;

    /// Project. Deterministic, total, and free of anything the value does not already hold.
    fn identity_view(&self) -> Self::View<'_>;
}

/// `Transformation` minus `id` and `created_at` (42 §1.3, row 3).
///
/// The field names are the ones the full struct uses, so the map keys of a projection are the
/// map keys a reader already knows from the wire form. The eight here and the ten of 41 §3
/// differ by exactly the two excluded fields, which is the A-3 reading: `intent_id` is *in*,
/// because the Draft a Candidate came from is part of what the Candidate is (ASM-11).
#[derive(Debug, Serialize)]
pub struct TransformationView<'a> {
    pub order: u8,
    pub intent_id: IntentId,
    pub subject: Subject,
    pub target: Option<Cid>,
    pub delta: &'a DeltaRef,
    pub context: &'a ChangeContext,
    pub actor: &'a Actor,
    pub parents: &'a [TransformationId],
}

impl IdentityView for Transformation {
    type View<'a> = TransformationView<'a>;

    fn identity_view(&self) -> TransformationView<'_> {
        TransformationView {
            order: self.order(),
            intent_id: self.intent_id,
            subject: self.subject,
            target: self.target,
            delta: &self.delta,
            context: &self.context,
            actor: &self.actor,
            parents: &self.parents,
        }
    }
}

/// `ObjectSnapshot` minus `id` (42 §1.3, row 1).
///
/// `id` is the CID of this very projection, so including it would ask the value to contain its
/// own digest.
#[derive(Debug, Serialize)]
pub struct ObjectSnapshotView<'a> {
    pub substrate: &'a SubstrateKind,
    pub locator: &'a str,
    pub digest: Cid,
    pub representation: ReprKind,
}

impl IdentityView for ObjectSnapshot {
    type View<'a> = ObjectSnapshotView<'a>;

    fn identity_view(&self) -> ObjectSnapshotView<'_> {
        ObjectSnapshotView {
            substrate: self.substrate(),
            locator: self.locator(),
            digest: *self.digest(),
            representation: *self.representation(),
        }
    }
}

/// `Intent` -- all five fields (42 §1.3, row 2).
///
/// The row has an empty exclusion column and the reason 「Intent自体が独立の意図記述であり除外規則
/// なし」, so this is the one projection in the workspace that is required to be the whole struct.
/// That makes the mirror strict: a sixth field added to `Intent` and not here would be a field
/// outside its own name. `crates/gx-canon/tests/intent_identity.rs` compares the two field sets
/// rather than trusting the count, which is the A-10 shape **I-1** asks of every projection.
///
/// The CID of this view is `IntentId` (ASM-11), fixed at `submit` (43 T-1) and immutable after --
/// so 「同一intent→同一IntentId」 (42 §3.3) is a property of this function and of the encoder it
/// feeds, and of nothing else.
#[derive(Debug, Serialize)]
pub struct IntentView<'a> {
    pub substrate: &'a SubstrateKind,
    pub locator: &'a str,
    pub goal: &'a GoalBytes,
    pub context: &'a ChangeContext,
    pub actor: &'a Actor,
}

impl IdentityView for Intent {
    type View<'a> = IntentView<'a>;

    fn identity_view(&self) -> IntentView<'_> {
        IntentView {
            substrate: self.substrate(),
            locator: self.locator(),
            goal: self.goal(),
            context: self.context(),
            actor: self.actor(),
        }
    }
}

/// `Fingerprint` -- all three fields (42 §1.3, row 5).
///
/// The row's exclusion column is empty, so the projection is the whole struct: what a fingerprint
/// *is* is 「which adapter computed it, over which scope, and what the digest of that scope was」, and
/// there is no self-reference or metadata among the three to leave out.
///
/// # Why this impl lives here and the type does not
///
/// **E-M4-1** (req/38 §28) put `Fingerprint` in gx-core -- 「型は下層・計算は上層」 -- because 42 §3.10
/// has a receipt name one, and a receipt (gx-witness) may not depend on an adapter. The projection
/// then has to be here for the reason every other gx-core projection is: [`IdentityView`] is
/// gx-canon's trait, and the orphan rule admits a foreign type only with a local trait. The same
/// arrangement `Transformation`, `ObjectSnapshot` and `Intent` are in.
///
/// `PlannedDelta`'s projection is **not** here, and the asymmetry is the rule working rather than an
/// exception to it: that type lives in gx-substrate, so its own crate can implement a foreign trait
/// for it without gx-canon learning that adapters exist.
///
/// What this projection is *for* is `crates/gx-substrate-conformance`'s L3 and L4 -- a fingerprint's
/// CID is how a harness says 「the state came back」 without reading the substrate. The comparison
/// 42 §3.5 defines is still [`gx_core::Fingerprint::cas_eq`] and never `==` (**E-M4-15**); a digest
/// of the whole projection answers a different question, and the two are not interchangeable: two
/// fingerprints with different scopes have different CIDs and no defined equality at all.
#[derive(Debug, Serialize)]
pub struct FingerprintView<'a> {
    pub substrate: &'a SubstrateKind,
    pub scope: &'a str,
    pub digest: Cid,
}

impl IdentityView for Fingerprint {
    type View<'a> = FingerprintView<'a>;

    fn identity_view(&self) -> FingerprintView<'_> {
        FingerprintView {
            substrate: self.substrate(),
            scope: self.scope(),
            digest: *self.digest(),
        }
    }
}

// ---------------------------------------------------------------------------
// The digest itself (42 §1.1)
// ---------------------------------------------------------------------------

/// The content identifier of a value: BLAKE3-256 over the canonical DAG-CBOR form of its
/// [`IdentityView`] (42 §1.1, 42 §1.3).
///
/// Written as a composition on purpose. The projection is applied, the result goes through the
/// wire face's [`crate::cbor::encode`], and only then is anything hashed -- so there is no
/// argument shape that reaches a `Cid` while skipping either step. That is what AC-014's
/// 「迂回禁止」 asks for at the level of this function: the rule is not that callers should use
/// the projection, it is that the projection is the only input the hash has.
///
/// Nothing here is a second encoder. 42 §2.1-6 admits one, in [`crate::cbor`], and this function
/// borrows it rather than reimplementing the parts it needs.
///
/// # Errors
/// Whatever [`crate::cbor::encode`] returns when the projection has no canonical form. A value
/// that cannot be encoded has no identity, and saying so is better than hashing an approximation
/// of it (`req/26 §3`).
pub fn compute<T: IdentityView + ?Sized>(value: &T) -> Result<Cid> {
    let bytes = crate::cbor::encode(&value.identity_view())?;
    Ok(digest(&[&bytes]))
}

/// The one place in the workspace where the hash is taken.
///
/// Private, and both public roads to a digest -- [`compute`] and [`mint`] -- end here. That is
/// what keeps `ac_014.rs`'s single-call-site assertion meaningful now that E-M2-12 has given the
/// ledger its own road: one line decides *how* bytes become a digest, and the functions above it
/// decide only *which* bytes. A second call site would be a second answer to the first question.
///
/// Streaming rather than concatenating: `parts` is hashed as though it were one buffer, so
/// `digest(&[a, b])` and `digest(&[ab])` agree and no caller pays for a copy to make them.
fn digest(parts: &[&[u8]]) -> Cid {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part);
    }
    Cid(*hasher.finalize().as_bytes())
}

// ---------------------------------------------------------------------------
// The domain-separated mint (42 §3.11, E-M2-12)
// ---------------------------------------------------------------------------

/// The digest gx computes, by name (35 DR-3 DEFAULT: BLAKE3 + DAG-CBOR).
///
/// It is here because this is the file that calls the hash: a name declared anywhere else could
/// drift from the function it names and nothing would catch it. gx-log's AC-024 compares this
/// string against a Rekor v2 reference vector, where it is expected to *differ* -- E-M2-9 rules
/// that the mismatch is asserted as a declared difference rather than papered over, since Rekor v2
/// and C2SP tlog-tiles are SHA-256 and no reading of AC-024 can make two hash functions agree.
pub const DIGEST_ALGORITHM: &str = "BLAKE3-256";

/// Which position in a Merkle tree a digest is being taken for (42 §3.11).
///
/// 42 §3.11 逐語: 「プレフィクスバイト（`0x00`=leaf, `0x01`=internal node）は second-preimage
/// 攻撃防止のための標準的ドメイン分離であり、Certificate Transparency（RFC 6962）由来の設計を
/// そのまま再利用する」. Without them a leaf whose content happens to look like two concatenated
/// child hashes could be presented as an internal node, and a proof could be built for a leaf the
/// log never held.
///
/// An enum rather than a `u8` argument, for the reason E-M2-18 typed `theorem_ids`: a byte
/// parameter admits `0x02`, which is not a domain 42 defines, and the wire form is unaffected
/// because what reaches the hash is [`Domain::byte`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    /// `0x00` -- the hash of a leaf's canonical form.
    Leaf,
    /// `0x01` -- the hash of an internal node's two children.
    Node,
}

impl Domain {
    /// The prefix byte itself (42 §3.11).
    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            Domain::Leaf => 0x00,
            Domain::Node => 0x01,
        }
    }
}

/// `BLAKE3(domain || parts…)` -- the general form of 42 §3.11's two hash rules.
///
/// The parts are concatenated without framing, because 42 §3.11 writes the node rule as
/// `0x01 || left_hash || right_hash` and a length prefix between the children would be a third
/// thing in the preimage that no verifier elsewhere would insert. Splitting one buffer across
/// several parts therefore changes nothing, which `tests/mint_domain.rs` checks.
///
/// # What this is not
///
/// Not an identity. [`compute`] answers 「what is this value」 by projecting through
/// [`IdentityView`] first (42 §1.3); this answers 「what is this position in a tree」 and takes
/// whatever bytes it is handed. The two never collide -- the domain byte is what separates the
/// tree's hashes from every unprefixed `Cid` in the system -- but the `Cid` type is shared,
/// because 42 §3.11 types `leaf_cid`, `audit_path` and `root_hash` as `Cid` and req/49 §3 M2-6
/// records the tension. E-M2-12 rules the mint into gx-canon rather than a second type into
/// gx-log, so the field names stay as 42 writes them and the *function* carries the distinction.
///
/// Infallible: any bytes have a digest. Whether they were the right bytes is the caller's claim.
#[must_use]
pub fn mint(domain: Domain, parts: &[&[u8]]) -> Cid {
    let tag = [domain.byte()];
    let mut all: Vec<&[u8]> = Vec::with_capacity(parts.len() + 1);
    all.push(&tag);
    all.extend_from_slice(parts);
    digest(&all)
}

/// `leaf_hash = BLAKE3(0x00 || canonical_dagcbor(leaf))` (42 §3.11).
///
/// The canonical form comes from [`crate::cbor::encode`], so 41 §6's 「全 canonical encode は
/// gx-canon 経由のみ」 holds for the ledger as it does for everything else: gx-log hands over a
/// value and never a byte string it produced itself.
///
/// # Errors
/// Whatever [`crate::cbor::encode`] returns for a value with no canonical form. A leaf that
/// cannot be encoded cannot be appended, and saying so is better than hashing an approximation
/// of it (`req/26` §3).
pub fn mint_leaf<T: serde::Serialize + ?Sized>(leaf: &T) -> Result<Cid> {
    let bytes = crate::cbor::encode(leaf)?;
    Ok(mint(Domain::Leaf, &[&bytes]))
}

/// `node_hash = BLAKE3(0x01 || left_hash || right_hash)` (42 §3.11).
///
/// Order-bearing: `mint_node(l, r)` and `mint_node(r, l)` are different nodes, because left and
/// right are different positions and a verifier that could swap them could move a leaf across the
/// tree.
///
/// Infallible: the children are already digests.
#[must_use]
pub fn mint_node(left: &Cid, right: &Cid) -> Cid {
    mint(Domain::Node, &[&left.0, &right.0])
}

// ---------------------------------------------------------------------------
// The readable form, `gx1:<base32>` (42 §1.2)
// ---------------------------------------------------------------------------

/// Write a `Cid` the way a human reads it, and the way JSON embeds it (42 §1.2).
///
/// Delegates. The RFC 4648 table and the `gx1:` prefix used to live in this file, on the reading
/// of req/31 §1(b) that kept gx-core ignorant of the display convention; E-JCS-1
/// (`req/38_ERRATA_2026-08-07.md` §5) ruled that JSON embedding takes the same form, and a serde
/// impl that has to mint the spelling has to know it. So the spelling moved to
/// [`gx_core::Cid::to_text`] and this function became the name gx-canon's callers already use.
///
/// What the ruling did not change is that there is **one** implementation of the form. Copying
/// the alphabet back into this file to keep the old layout would create exactly the second
/// spelling 42 §1.2 exists to prevent, which is why this is a delegation and not a reimplementation
/// -- `tests/cid_text.rs` checks that mechanically.
///
/// Infallible: every 32-byte array has a spelling.
#[must_use]
pub fn to_text(cid: &Cid) -> String {
    cid.to_text()
}

/// Read a `Cid` back from its readable form.
///
/// Strict in the same sense the decode path of [`crate::cbor`] is strict (CM-6): one digest has
/// one spelling. Uppercase, padding, a wrong length and a final character whose unused bits are
/// set are all refused rather than repaired, because each of them would make the map from text
/// to `Cid` many-to-one -- and AC-011 compares two processes by the text they print.
///
/// The refusals themselves are [`gx_core::Cid::from_text`]'s; this function restates them as
/// gx-canon's [`Error::CidText`] so a caller of this crate handles one error type. The `detail`
/// string is carried across unchanged, so the reason a spelling was refused survives the hop.
///
/// # Errors
/// [`Error::CidText`], naming which of those conditions failed.
pub fn from_text(text: &str) -> Result<Cid> {
    Cid::from_text(text).map_err(|e| match e {
        gx_core::Error::CidText { detail } => Error::CidText { detail },
        // gx-core's error enum is not `non_exhaustive` and `from_text` returns only the variant
        // above; anything else would be a gx-core change that this arm makes visible rather than
        // silently relabelling.
        other => Error::CidText {
            detail: format!("unexpected gx-core error: {other}"),
        },
    })
}

/// Kani harness 2 of 3 (46 §4.2, row `gx-canon::cid::compute`).
///
/// The row's two claims are 「panic-freedom、固定長出力の保証」. Both are below.
///
/// # Bounds (51 §5)
///
/// One `ObjectSnapshot` whose `digest` and `id` are symbolic over all 2^256 values, whose
/// `locator` is the empty string and whose two enumerations are fixed at one variant each.
/// **Not** checked: `Transformation` (its `parents` vector makes the encoded length symbolic,
/// which is what a bounded check cannot absorb), non-empty locators, and the correctness of
/// `blake3` itself -- the crate is called, and 46 §1's non-goals put the verification of
/// cryptographic primitives outside this project by name (「暗号…のLean内検証」は非目標、実装は
/// audited crate に委譲).
///
/// The unwind value was measured, not guessed; see the same note in [`crate::cbor`]'s harness.
#[cfg(kani)]
mod verification {
    use super::*;
    use gx_core::{ObjectId, ReprKind, SubstrateKind};

    #[kani::proof]
    #[kani::unwind(12)]
    fn compute_is_total_and_returns_thirty_two_bytes() {
        let x = ObjectSnapshot::new(
            ObjectId(Cid(kani::any())),
            SubstrateKind::Fs,
            String::new(),
            Cid(kani::any()),
            ReprKind::Bytes,
        );

        match compute(&x) {
            // `Cid` is `[u8; 32]` by construction, so the length claim is a type-level fact; what
            // this asserts is that the value came back at all, and that nothing on the way from
            // the projection through the encoder to the digest panicked.
            Ok(cid) => assert!(cid.0.len() == 32),
            // A value with no canonical form has no identity, and saying so is a return (41 §6).
            Err(_) => {}
        }
    }
}
