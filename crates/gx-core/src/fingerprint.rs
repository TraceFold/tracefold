//! The precondition fingerprint of CON-2, and the opaque carrier M2 moved without interpreting.
//!
//! Spec: 42 §3.5 for what a `Fingerprint` is, 42 §3.10 for the receipt field that needs one,
//! **E-M2-2** (`req/38_ERRATA_2026-08-07.md` §8) for why the carrier exists, **E-M4-1** (§28) for
//! why the real type is here rather than in gx-substrate.
//!
//! 42 §3.10 types `ReceiptPayload.precondition_fingerprint` as `Fingerprint`, and 42 §0 files
//! `Fingerprint` under `gx-substrate/fingerprint.rs` -- M4. A receipt that cannot be constructed
//! until M4 is a receipt M2 cannot build, which is the same shape A-2 met when the Kani fingerprint
//! row was deferred: the ruling carried 32 opaque bytes then and let M4 supply the meaning.
//!
//! # M4 supplies it here, and the receipt does not move
//!
//! **E-M4-1** 逐語: 「`Fingerprint` 実型(substrate/scope/digest)は **gx-core**・scope 決定と比較の
//! 計算は gx-substrate(E-M2-1「型は下層・計算は上層」・M3-13 先例)。42 §3.10 の
//! `precondition_fingerprint` は `FingerprintBytes`=`Fingerprint.digest` 成分のまま=**M2 receipt
//! wire 形不変**。42 §0 の fingerprint.rs 行は「計算の所在」と読み替える」.
//!
//! So the two types below are not a duplication. [`Fingerprint`] is what an adapter computes and
//! what a CAS check compares; [`FingerprintBytes`] is the digest component of one, which is the
//! thing a signed receipt has always carried. Putting the whole struct on the wire would have
//! changed bytes that M2 pinned and gx-witness signs, for no claim anybody makes about them.
//!
//! Nothing here computes a fingerprint. Deciding what a `scope` covers and hashing the state it
//! names is the adapter's, in gx-substrate and above -- the same split A-1 made for `Cid`.

use serde::de::{Error, SeqAccess, Visitor};
use serde::{Deserializer, Serializer};

use crate::object::SubstrateKind;
use crate::Cid;
use core::fmt;

/// What an adapter says the relevant substrate state is (42 §3.5, **E-M4-1**).
///
/// Three fields and no invariant *between* them -- 42 §3.5 relates none of them -- but one bound on
/// a field: [`Fingerprint::new`] refuses a `scope` past [`MAX_SCOPE_BYTES`] (**M4H1-2**), which is
/// the only thing standing between 42 §3.5's advice to take a scope wide and an unbounded string in
/// a signed receipt. `scope` is the adapter's own identifier for 「対象に干渉しうる周辺状態」 and
/// `digest` is the digest of whatever that scope names.
///
/// # Why there is no `PartialEq` (**E-M4-15**)
///
/// 42 §3.5 defines equality as 「`substrate` と `scope` が一致し、かつ `digest` のバイト列が一致」 and
/// then adds a case that is not equality at all: 「`scope`不一致の比較は意味を持たないため**adapter
/// 実装エラーとして扱う**」. `PartialEq` returns a `bool`, so the third answer would have to be
/// spelled `false` -- and `false` at a CAS check means 「the state moved」, which would abort a commit
/// for a change that never happened while hiding the bug that actually occurred. E-M4-15 rules the
/// derive out and [`Fingerprint::cas_eq`] in, so the misuse is a compile error rather than a
/// silently wrong answer.
///
/// **E-M4-27** then widened the third answer to the other field: 42 §3.5's equality is read as
/// 「**同一 adapter の産物間でのみ定義**」, so two adapters' fingerprints are a refusal rather than an
/// `Ok(false)`. The same argument, one field over -- and the reason it needed a ruling rather than a
/// reading is that 42 §3.5 names only the `scope` case in so many words.
///
/// The knock-on is deliberate: no type holding a `Fingerprint` can derive `PartialEq` either, so
/// every comparison in the workspace has to say which of the two questions it is asking. The
/// absence is a compile error rather than a convention, which is what this shows:
///
/// ```compile_fail
/// use gx_core::{Cid, Fingerprint, SubstrateKind};
/// let a = Fingerprint::new(SubstrateKind::Fs, "/tmp/x".to_string(), Cid([0u8; 32]))
///     .expect("a six-byte scope is inside M4H1-2's bound");
/// let b = a.clone();
/// // 42 §3.5's third answer has no `bool` to be, so there is no `==` here (E-M4-15).
/// let _ = a == b;
/// ```
///
/// # What it is not
///
/// Not a `Cid`-of-itself: 42 §1.3 gives `Fingerprint` an IdentityView of all three fields, and the
/// projection is req/69 §6.2 hand 3's, beside `PlannedDelta`'s. This hand defines the type and the
/// comparison and computes neither a scope nor a digest.
#[derive(Clone, Debug)]
pub struct Fingerprint {
    substrate: SubstrateKind,
    scope: String,
    digest: Cid,
}

/// The bound on a fingerprint scope, in bytes (**M4H1-2**, ASM-60-4's value one type over).
///
/// req/38 §29 逐語: 「gx-core にも scope 文字列の上限+超過時 digest 化(gx-gate M3-21/ASM-60-4 と同形・
/// 1024 byte)。**scope の生成規則(ASM-69-1)が実体化する手4 と同窓**で入れる(生成と上限を別の手で
/// 決めない)」.
///
/// One constant, in one place, for every adapter: 42 §3.5 recommends taking a scope 「対象に干渉しうる
/// 周辺状態」 wide, and a receipt carries what it produces into bytes gx-witness signs. The number is
/// gx-gate's [`MAX_MESSAGE_BYTES`](../../gx_gate/constant.MAX_MESSAGE_BYTES.html) rather than a
/// second opinion about the same question, and it is an assumption an Owner may move.
pub const MAX_SCOPE_BYTES: usize = 1024;

impl Fingerprint {
    /// Build one, refusing a scope past [`MAX_SCOPE_BYTES`] (**M4H1-2 採(a)**).
    ///
    /// The fields are private and read through the accessors below, which is F-6's rule
    /// (`req/46D_AUDIT_RULING_2026-08-07.md` §1) -- one spelling per field -- and, here, also what
    /// keeps [`Fingerprint::cas_eq`] the road a comparison takes: `a.digest() == b.digest()` is
    /// still writable, but it is visibly a comparison of one component rather than of two values.
    ///
    /// # The bound is at the constructor, and that is the whole of its value
    ///
    /// An adapter that wanted an unbounded scope cannot get one by not calling a helper: every
    /// `Fingerprint` in the workspace goes through here, so 「1024 byte を超えない」 is a property of
    /// the type rather than a discipline. What an adapter does with a longer one is
    /// [`gx_substrate::fingerprint::elide_scope`] -- 42 §3.8's 「長文は digest 化」, in the crate that
    /// can reach a hash (A-1 keeps every digest in gx-canon, and 41 §2 keeps this crate's
    /// dependencies at 「serde, thiserror程度」). E-M2-1's 「型は下層・計算は上層」, one more time.
    ///
    /// # Errors
    /// [`crate::Error::ScopeTooLong`] when `scope` is longer than [`MAX_SCOPE_BYTES`] bytes. The
    /// count is bytes, as ASM-60-4 words the same bound for a message, and a multi-byte character is
    /// never split: the whole scope is refused or none of it is.
    pub fn new(substrate: SubstrateKind, scope: String, digest: Cid) -> crate::Result<Self> {
        if scope.len() > MAX_SCOPE_BYTES {
            return Err(crate::Error::ScopeTooLong {
                bytes: scope.len(),
                max: MAX_SCOPE_BYTES,
            });
        }
        Ok(Self {
            substrate,
            scope,
            digest,
        })
    }

    /// 42 §3.5's equality, with the third answer kept (**E-M4-15**).
    ///
    /// `Ok(true)` is 「同じ名前の状態に居る」, `Ok(false)` is 「動いた」, and `Err` is 「その比較は意味を
    /// 持たない」. The CAS check of CON-2 (41 §5-5b) is the caller that has to tell the three apart:
    /// 43 T-10a turns `Ok(false)` into `AbortReason::PreconditionChanged`, and an `Err` is a bug in
    /// an adapter rather than a state the machine has a transition for.
    ///
    /// # Both mismatches refuse, and the substrate is answered first (**E-M4-27**)
    ///
    /// Hand 1 wrote 42 §3.5 to the letter: the sentence calls only the `scope` mismatch an
    /// implementation error, so a `substrate` mismatch answered `Ok(false)`. §29 M4H1-1 (b) ruled
    /// that asymmetry out -- 「42 §3.5 の等価性は「**同一 adapter の産物間でのみ定義**」と読む」 -- on the
    /// argument E-M4-15 already made about `PartialEq`: an answer of 「the state moved」 to a
    /// question that was never a comparison aborts a commit for a change nobody made and leaves the
    /// real defect, a mis-wired adapter, invisible.
    ///
    /// When both disagree the refusal names the substrate, because that is the whole diagnosis: two
    /// adapters do not share a scope vocabulary, so 「scope が違う」 would report a symptom of the
    /// thing that went wrong rather than the thing.
    ///
    /// # Errors
    /// [`crate::Error::FingerprintSubstrateMismatch`] when the two substrates differ, and
    /// [`crate::Error::FingerprintScopeMismatch`] when the substrates agree and the scopes do not.
    pub fn cas_eq(&self, other: &Self) -> crate::Result<bool> {
        if self.substrate != other.substrate {
            return Err(crate::Error::FingerprintSubstrateMismatch {
                left: self.substrate.clone(),
                right: other.substrate.clone(),
            });
        }
        if self.scope != other.scope {
            return Err(crate::Error::FingerprintScopeMismatch {
                left: self.scope.clone(),
                right: other.scope.clone(),
            });
        }
        Ok(self.digest == other.digest)
    }

    /// Which adapter computed this (42 §3.5).
    #[must_use]
    pub fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    /// The adapter-defined scope identifier (42 §3.5).
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// The digest of the state the scope names (42 §3.5).
    #[must_use]
    pub fn digest(&self) -> &Cid {
        &self.digest
    }
}

/// The Kani row A-2 deferred to M4, four milestones and four count-slips later (**E-M4-23**).
///
/// 46 §4.2's 「`gx-core` hot path」 row is `compose`, `identity` and **`Fingerprint` comparison**.
/// A-2 (`req/38` §1) deferred the third to M4 because M1 had no such type; §28 **E-M4-23** recorded
/// that M4's own DoD had lost it again; and req/76 §4 K-7 counted the slip a **fourth** time --
/// 「本手は Kani を 1 度も走らせておらず harness も書いていない…数え落としが M2-15/16・M3-19 に続いて
/// 4 回目」. §35 K-7 採(a) put it in the fix 批's DoD, and this module is it.
///
/// # Why this function and not the whole type
///
/// [`Fingerprint::cas_eq`] is where 42 §3.5's equality is **decided**, and 41 §5-5b's CAS check is
/// the caller. The three answers are not three shades of one: `Ok(false)` aborts a commit
/// (43 T-10a's `AbortReason::PreconditionChanged`), `Ok(true)` lets one through, and `Err` is a bug
/// in an adapter that no state machine has a transition for. **E-M4-15** and **E-M4-27** are the two
/// rulings that put each answer where it is, and both were argued from what happens when one is
/// spelled as another -- which is exactly a claim about a three-branch total function over all
/// inputs, and exactly what a bounded model check is for.
///
/// # The bounds, stated rather than implied (51 §5)
///
/// The digests are symbolic over all 2^256 values, and every `bool` and `u8` below is symbolic over
/// its whole type. The **heap** values are drawn from small fixed sets rather than made symbolic:
/// the substrate is one of four (three unit variants and one `Custom` with a one-character name)
/// and the scope is one of two one-character strings. That is the standard reduction 46 §4.1 asks
/// for, and it is not a weakening here: `cas_eq` compares its `String` and its `SubstrateKind` with
/// `PartialEq` and never reads a character, so a two-element set exhausts the branch structure
/// (equal / different) that the function can observe. What symbolic strings would add is coverage of
/// `String`'s own `==`, which is not this crate's code.
///
/// `#[kani::unwind(40)]` is measured rather than guessed, which is **E-46-4**'s standing
/// instruction (「Kani の unwind/bound は実測で決め harness ごとに明記する」). At 4 -- enough for the
/// one-character scopes -- both harnesses fail with `unwinding assertion loop 0` inside
/// `<builtin-library-memcmp>`: the loop that actually needs the room is the **32-byte digest**
/// comparison, not the strings, and 40 is that with margin.
#[cfg(kani)]
mod verification {
    use super::*;

    /// One substrate out of four: the three unit variants and one `Custom`.
    ///
    /// `Custom` is in the set because **E-M4-27**'s subject is two *adapters*, and a workspace whose
    /// only custom kind is a test double (`Custom("mock")`) would otherwise leave the variant that
    /// carries a heap value out of the proof.
    fn a_substrate(pick: u8) -> SubstrateKind {
        match pick % 4 {
            0 => SubstrateKind::Fs,
            1 => SubstrateKind::Git,
            2 => SubstrateKind::Mcp,
            _ => SubstrateKind::Custom("c".to_string()),
        }
    }

    /// One scope out of two. Both are inside [`MAX_SCOPE_BYTES`], so the constructor's refusal is
    /// not what this harness is about (`scope_bound.rs` measures that).
    fn a_scope(pick: bool) -> String {
        if pick { "a" } else { "b" }.to_string()
    }

    fn a_fingerprint(substrate: u8, scope: bool, digest: [u8; 32]) -> Fingerprint {
        Fingerprint::new(a_substrate(substrate), a_scope(scope), Cid(digest))
            .expect("a one-character scope is inside the bound")
    }

    /// `cas_eq` is total, and each of its three answers is the one 42 §3.5 gives that input.
    ///
    /// Total in the sense the other three harnesses in this crate use: it returns for every input,
    /// a refusal being a value and not a panic (41 §6). The three claims below are 42 §3.5 read as
    /// **E-M4-15** and **E-M4-27** settled it, and they partition the input space -- so an
    /// implementation that answered `Ok(false)` to a mismatched substrate (which is what hand 1
    /// wrote, and what §29 M4H1-1 (b) overturned) fails this proof rather than a test that happens
    /// to have that pair in it.
    #[kani::proof]
    #[kani::unwind(40)]
    fn cas_eq_is_total_and_answers_42_3_5s_three_cases() {
        let left_digest: [u8; 32] = kani::any();
        let right_digest: [u8; 32] = kani::any();
        let left = a_fingerprint(kani::any(), kani::any(), left_digest);
        let right = a_fingerprint(kani::any(), kani::any(), right_digest);

        let same_substrate = left.substrate() == right.substrate();
        let same_scope = left.scope() == right.scope();

        match left.cas_eq(&right) {
            // 「同じ名前の状態に居る」 / 「動いた」: reachable only when the two name one state.
            Ok(equal) => {
                assert!(same_substrate);
                assert!(same_scope);
                assert!(equal == (left_digest == right_digest));
            }
            // 「その比較は意味を持たない」, and which of the two mismatches it names is fixed:
            // the substrate is answered first, because two adapters do not share a scope
            // vocabulary and 「scope が違う」 would report a symptom instead of the diagnosis.
            // `crate::Error` spelled out: the name `Error` in this module is `serde::de::Error`,
            // which the serde impls below need and which is a trait.
            Err(crate::Error::FingerprintSubstrateMismatch { .. }) => assert!(!same_substrate),
            Err(crate::Error::FingerprintScopeMismatch { .. }) => {
                assert!(same_substrate);
                assert!(!same_scope);
            }
            Err(_) => assert!(
                false,
                "cas_eq answered with a word that is not one of its two"
            ),
        }
    }

    /// A fingerprint compares equal with itself, and the comparison does not depend on the side.
    ///
    /// Reflexivity is what the CAS check of CON-2 rests on -- a commit is admitted when the state
    /// read at verify and the state read at commit are 「the same」, and the two reads produce two
    /// values. **M4H1-2**'s elision was argued from this in `gx-substrate`'s `elide_scope`: 「二読の
    /// 一つの長い path は同じ line に elide しなければ、CAS 検査が自分自身と比較した file について
    /// 「その比較は意味を持たない」と答える」. Symmetry is the other half: an engine that compared
    /// `(before, after)` and one that compared `(after, before)` must not disagree, and 42 §3.5
    /// leaves the direction unwritten.
    #[kani::proof]
    #[kani::unwind(40)]
    fn cas_eq_is_reflexive_and_direction_free() {
        let digest: [u8; 32] = kani::any();
        let other: [u8; 32] = kani::any();
        let left = a_fingerprint(kani::any(), kani::any(), digest);
        let right = a_fingerprint(kani::any(), kani::any(), other);

        assert!(left.cas_eq(&left) == Ok(true));

        match (left.cas_eq(&right), right.cas_eq(&left)) {
            (Ok(there), Ok(back)) => assert!(there == back),
            (Err(_), Err(_)) => {}
            _ => assert!(
                false,
                "one direction of a comparison is an answer and the other is a refusal"
            ),
        }
    }
}

/// Thirty-two bytes a receipt carries and M2 never reads (E-M2-2).
///
/// # What this is not
///
/// 42 §3.5's `Fingerprint` is `{ substrate: SubstrateKind, scope: String, digest: Cid }`, computed
/// by an adapter over 「対象に干渉しうる周辺状態」, and its equivalence is defined there as
/// `substrate` and `scope` agreeing *and* the digest bytes matching -- a `scope` mismatch is an
/// adapter bug, not a `false`. This type has one of those three fields and therefore cannot
/// implement that relation. Its `PartialEq` is byte equality: the necessary half, and the half the
/// CAS check of CON-2 will be built on in M4. **A receipt whose two `FingerprintBytes` compare
/// equal has not passed the CAS check** -- M2 has not performed one.
///
/// # Why not `Cid`
///
/// `Cid` means "the BLAKE3 digest of this value's canonical DAG-CBOR form" (42 §1.1). What M4 puts
/// in this field is fixed by 42 §3.5's own `digest` field plus whatever scope discipline the
/// adapter applies, and calling it a `Cid` before M4 says so would assert a provenance for the
/// bytes that no ruling has given them. A distinct newtype also means the compiler refuses to pass
/// one where the other belongs, which is the whole value of carrying it opaquely.
///
/// The bytes are public because the field is a carrier, and hiding them behind an accessor would
/// suggest an invariant this type does not hold -- any 32 bytes are a legal value here.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerprintBytes(pub [u8; 32]);

/// Opaque for the reason [`crate::Cid`]'s is: the canon fixes no readable spelling for a
/// fingerprint (42 §1.2 gives one to `Cid` and to nothing else), so no `{:?}` in a log line may
/// mint one. When M4 defines the real `Fingerprint`, its display -- if it gets one -- is decided
/// there and not by whatever a debug format happened to print in the meantime.
impl fmt::Debug for FingerprintBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FingerprintBytes(opaque)")
    }
}

/// A byte string on the wire and base64 in a human-readable format, which is the same pair
/// [`crate::dsse`]'s `raw_bytes` writes and settled by the same ruling: **M2H1-4** names
/// 「sig/FingerprintBytes」 together and recommends base64, and hand 5 takes it. A derive would
/// write thirty-two integers whose encoded length depends on their values.
impl serde::Serialize for FingerprintBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&crate::b64::encode(&self.0))
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> serde::Deserialize<'de> for FingerprintBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bytes32;

        impl<'v> Visitor<'v> for Bytes32 {
            type Value = FingerprintBytes;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("32 bytes, base64 (RFC 4648 §4) in a human-readable format")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<FingerprintBytes, E> {
                let raw = crate::b64::decode(v).map_err(E::custom)?;
                self.visit_bytes(&raw)
            }

            fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<FingerprintBytes, E> {
                let raw: [u8; 32] = v
                    .try_into()
                    .map_err(|_| E::invalid_length(v.len(), &"32 bytes"))?;
                Ok(FingerprintBytes(raw))
            }

            fn visit_seq<A: SeqAccess<'v>>(self, mut seq: A) -> Result<FingerprintBytes, A::Error> {
                let mut raw = [0u8; 32];
                for (i, slot) in raw.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| <A::Error as Error>::invalid_length(i, &"32 bytes"))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(<A::Error as Error>::invalid_length(33, &"32 bytes"));
                }
                Ok(FingerprintBytes(raw))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_any(Bytes32)
        } else {
            deserializer.deserialize_bytes(Bytes32)
        }
    }
}
