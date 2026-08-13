//! The core types of the transformation calculus, and nothing else.
//!
//! Spec: `req/spec/40-architecture/41-architecture.md` §2 for where this crate sits in
//! the workspace, §3 for the type signatures it is required to carry, §6 for what it is
//! forbidden to do -- no I/O, deterministic, no `unsafe`.
//!
//! The dependency between the two implementation crates runs one way. `gx-canon` names
//! `gx-core`; `gx-core` never names `gx-canon`. That is what lets `Cid` be defined here
//! while the BLAKE3 computation that fills one in lives over there (`ASM-16`, and the
//! A-1 ruling written down in `req/38_ERRATA_2026-08-07.md` §1). `DeltaRef` carries a
//! `Cid`, so putting the type in `gx-canon` would have made the two crates cycle.
//!
//! The module list is the one fixed by 41 §2 -- `object`, `transformation`, `context`,
//! `measure`, `commutation`, `delta`, `error` -- and M1 added none of its own. `Cid`
//! sits in this file rather than in an eighth module for that reason (req/31 §1).
//!
//! M2 hand 1 adds four, and says why rather than quietly widening the list. The rulings of
//! `req/38_ERRATA_2026-08-07.md` §8 move type definitions down here that 42 §0 files under
//! gx-witness, gx-log and gx-substrate, because written as 42 has them the crates form a cycle
//! cargo will not build (E-M2-1) or need an M4 type in an M2 struct (E-M2-2):
//!
//! * `dsse` -- `DsseSignature`, which `Checkpoint` carries (E-M2-1)
//! * `ledger` -- `InclusionProof` and `Checkpoint`, which `ReceiptPayload` carries (E-M2-1)
//! * `fingerprint` -- `FingerprintBytes`, the opaque carrier standing in for M4's `Fingerprint`
//!   (E-M2-2)
//! * `proof` -- `TheoremId`, `ProofRef`, `CheckerResultRef`, `Proof` (E-M2-12)
//!
//! One rule holds all four: **the data comes down, the computation stays up**. Nothing added here
//! signs, hashes, verifies or compares -- that is A-1's shape (`Cid` here, BLAKE3 in gx-canon)
//! applied a second time, and it is what makes the cycle absent from the dependency graph instead
//! of forbidden by a rule someone has to remember. 41 §2's module list is therefore extended by
//! this hand; the erratum is raised in req/50 §4.
//!
//! M2 hand 5 adds a twelfth, [`b64`], and takes the same route rather than piling into this file --
//! **E-M2-16** ruled that preference explicitly (「lib.rs 積み増し案は可読性最優先(規律3)で却下」).
//! It is the RFC 4648 §4 table M2H1-4 settles the JSON face of every raw byte string with, and it
//! sits beside [`Cid`]'s base32 for the same reason that one is written out longhand here.
//!
//! M3 hand 1 adds a thirteenth and a fourteenth, and the rule that put the four M2 ones here is the
//! one that puts these two (`req/38_ERRATA_2026-08-07.md` §19):
//!
//! * `planned` -- `PlannedDeltaBytes`, the opaque carrier standing in for M4's `PlannedDelta`
//!   (**E-M3-1**), which is `FingerprintBytes` a second time
//! * `verdict` -- `VerdictKind`, the three-valued discriminant (**E-M3-2**), placed here because
//!   gx-gate names gx-witness (`GateInput.evidence`) and gx-witness needs the discriminant: filed
//!   where 42 §0 files it, the two crates would name each other
//!
//! Neither adds a dependency and neither computes anything, which is the test of whether a type
//! belongs down here.
//!
//! M4 hand 1 adds a fifteenth, and two rulings of `req/38_ERRATA_2026-08-07.md` §28 move one more
//! type down here and complete another:
//!
//! * `intent` -- `Intent` and `GoalBytes` (**E-M4-2**). 42 §0 already files `Intent` under this
//!   crate; what was missing was the type. `goal` is bytes rather than a `serde_json::Value`, which
//!   is the same 「data comes down, computation stays up」 rule applied to an encoding: a crate that
//!   may not know an encoder may not name a document model either.
//! * `fingerprint` -- gains the real `Fingerprint` beside the `FingerprintBytes` carrier
//!   (**E-M4-1**). E-M2-2 said 「M4 が実 `Fingerprint` を定義したら満期」 and this is the maturity;
//!   the carrier stays because it is the digest component a signed receipt has always held, so no
//!   byte gx-witness signs moves.
//!
//! Both follow the rule above: no dependency, no arithmetic. The scope of a fingerprint and the
//! digest under it are computed in gx-substrate, which is where 42 §0's `fingerprint.rs` row is
//! re-read as 「計算の所在」.
//!
//! M5 hand 1 adds a sixteenth on the same rule (**M5-08 採(a)**, `req/38_ERRATA_2026-08-07.md` §37):
//!
//! * `enforcement` -- `EnforcementMode` and `FailPosture`, DR-2's two independent axes. 43 §10 left
//!   their placement open for three milestones (「型自体の配置は41への追記提案として扱う」); the
//!   engine writes them and gx-witness reads one of them off a receipt, so two crates name the type
//!   and the type comes down. Exactly the shape of `VerdictKind` one paragraph up, and it adds no
//!   dependency and computes nothing.

#![forbid(unsafe_code)]

pub mod b64;
pub mod commutation;
pub mod context;
pub mod delta;
pub mod dsse;
pub mod enforcement;
pub mod error;
pub mod fingerprint;
pub mod intent;
pub mod ledger;
pub mod measure;
pub mod object;
pub mod planned;
pub mod proof;
pub mod transformation;
pub mod verdict;

pub use commutation::Commutation;
pub use context::{Actor, ChangeContext, KeyId};
pub use delta::DeltaRef;
pub use dsse::DsseSignature;
pub use enforcement::{EnforcementMode, FailPosture};
pub use error::{AbortReason, Error, Result, ERROR_KINDS};
pub use fingerprint::{Fingerprint, FingerprintBytes, MAX_SCOPE_BYTES};
pub use intent::{GoalBytes, Intent};
pub use ledger::{Checkpoint, InclusionProof, VerdictCheckpoint, VerdictTally};
pub use measure::{Lyapunov, MorphismMeasure, ObjectMeasure};
pub use object::{ObjectId, ObjectSnapshot, ReprKind, SubstrateKind};
pub use planned::PlannedDeltaBytes;
pub use proof::{CheckerResultRef, Proof, ProofRef, TheoremId};
pub use transformation::{
    ancestors, composable, compose, identity, CompositionMetadata, IntentId, Subject, Timestamp,
    Transformation, TransformationId, MAX_ORDER,
};
pub use verdict::VerdictKind;

use core::fmt;

/// Content identifier: the BLAKE3-256 digest of a value's canonical DAG-CBOR form
/// (41 §3, 42 §1.1, DR-3 DEFAULT). Thirty-two raw bytes and nothing else -- no multicodec,
/// no multibase, no self-describing tag (42 §1.1).
///
/// This crate owns the *type* and computes none of it. Producing a `Cid` from a value means
/// projecting through `IdentityView`, encoding on the wire face and hashing, all of which
/// live in `gx-canon` (A-1, `req/38_ERRATA_2026-08-07.md` §1). A `Cid` built here by hand --
/// the field is public, as 41 §3 writes it -- is a 32-byte container, not a claim that any
/// particular value hashes to it.
/// # Why serde is hand-written here
///
/// The derive would emit `[u8; 32]` the way serde emits any fixed array: as a sequence of
/// thirty-two integers. In DAG-CBOR that is a 32-element list -- and above 23 each element grows
/// a header byte, so a digest of large bytes gets longer than one of small bytes. 42 §1.1 says
/// the opposite: 「バイナリ埋め込み（DAG-CBOR内部）では32byte byte-string（major type 2）として直接
/// 格納」. So the binary side of the impls below calls `serialize_bytes`.
///
/// The text side is the E-JCS-1 ruling (`req/38_ERRATA_2026-08-07.md` §5). 42 §1.2 says
/// 「CLI/API/ログの人間可読表示・**JSON埋め込み**はすべてこの形式を正とする」, so a human-readable
/// format gets `gx1:<base32>` and not an array of numbers. That is why `is_human_readable` is
/// consulted here and why [`Cid::to_text`] exists in this crate at all: a serializer that has to
/// mint the spelling cannot be a layer that does not know it. req/31 §1(b) and §11 read the
/// other way, and the erratum supersedes them on this point.
///
/// What survives from that default is the narrower rule that actually prevents a second spelling:
/// there is exactly one implementation of the form in the workspace, it is [`Cid::to_text`], and
/// `gx-canon`'s `cid::to_text` delegates to it rather than repeating it. `Display` is still not
/// implemented (see [`Cid`]'s `Debug`), so no `{}` in a log line mints a `Cid` by accident.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cid(pub [u8; 32]);

/// 42 §1.2's fixed prefix. It is what makes the form self-identifying without a multibase table:
/// gx does not adopt IPLD's self-description (42 §1.1), so a four-character constant does the
/// whole job of saying which format this is.
const CID_TEXT_PREFIX: &str = "gx1:";

/// RFC 4648 base32, lowercase. 42 §1.2 asks for lowercase without padding, so the alphabet is
/// written out here rather than upcased-then-lowered through some other crate's table.
const CID_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// 256 bits at five bits a character, rounded up. 42 §1.2 states the same number as 「32byte→52
/// 文字」.
const CID_BODY_LEN: usize = 52;

/// The four bits at the end of the 52nd character that no digest bit reaches.
const CID_TAIL_BITS: u32 = 4;

impl Cid {
    /// Write a `Cid` the way a human reads it, and the way JSON embeds it (42 §1.2).
    ///
    /// The single implementation of the spelling. `gx-canon::cid::to_text` calls this one;
    /// nothing else in the workspace builds the string.
    ///
    /// Infallible: every 32-byte array has a spelling.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::with_capacity(CID_TEXT_PREFIX.len() + CID_BODY_LEN);
        out.push_str(CID_TEXT_PREFIX);

        let mut acc: u32 = 0;
        let mut bits: u32 = 0;
        for &byte in &self.0 {
            acc = (acc << 8) | u32::from(byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                out.push(char::from(CID_ALPHABET[((acc >> bits) & 0x1f) as usize]));
            }
        }
        if bits > 0 {
            // The leftover bits are the high end of the last character; the low `5 - bits` are
            // zero, which is what [`Cid::from_text`] insists on seeing.
            out.push(char::from(
                CID_ALPHABET[((acc << (5 - bits)) & 0x1f) as usize],
            ));
        }
        out
    }

    /// Read a `Cid` back from its readable form.
    ///
    /// Strict in the same sense gx-canon's decode path is strict (CM-6): one digest has one
    /// spelling. Uppercase, padding, a wrong length and a final character whose unused bits are
    /// set are all refused rather than repaired, because each of them would make the map from
    /// text to `Cid` many-to-one -- and AC-011 compares two processes by the text they print.
    ///
    /// # Errors
    /// [`Error::CidText`], naming which of those conditions failed.
    pub fn from_text(text: &str) -> Result<Self> {
        let body = text
            .strip_prefix(CID_TEXT_PREFIX)
            .ok_or_else(|| Error::CidText {
                detail: format!("missing the `{CID_TEXT_PREFIX}` prefix"),
            })?;
        if body.len() != CID_BODY_LEN {
            return Err(Error::CidText {
                detail: format!("body is {} characters, not {CID_BODY_LEN}", body.len()),
            });
        }

        let mut raw = [0u8; 32];
        let mut written = 0usize;
        let mut acc: u32 = 0;
        let mut bits: u32 = 0;
        for (position, ch) in body.bytes().enumerate() {
            let value =
                CID_ALPHABET
                    .iter()
                    .position(|&a| a == ch)
                    .ok_or_else(|| Error::CidText {
                        detail: format!(
                        "character {position} is not RFC 4648 base32 in lowercase without padding"
                    ),
                    })?;
            acc = (acc << 5) | u32::try_from(value).expect("alphabet index is below 32");
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                raw[written] = u8::try_from((acc >> bits) & 0xff).expect("masked to eight bits");
                written += 1;
            }
        }

        debug_assert_eq!(written, raw.len());
        if bits != CID_TAIL_BITS || acc & ((1 << CID_TAIL_BITS) - 1) != 0 {
            return Err(Error::CidText {
                detail: "the unused bits of the last character are set, which would give this \
                         digest a second spelling"
                    .to_string(),
            });
        }
        Ok(Cid(raw))
    }
}

impl serde::Serialize for Cid {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_text())
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Cid {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        struct Text;

        impl serde::de::Visitor<'_> for Text {
            type Value = Cid;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a `gx1:<base32>` content identifier")
            }

            fn visit_str<E>(self, v: &str) -> core::result::Result<Cid, E>
            where
                E: serde::de::Error,
            {
                Cid::from_text(v).map_err(E::custom)
            }
        }

        struct Bytes32;

        impl<'v> serde::de::Visitor<'v> for Bytes32 {
            type Value = Cid;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("32 bytes")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> core::result::Result<Cid, E>
            where
                E: serde::de::Error,
            {
                let raw: [u8; 32] = v
                    .try_into()
                    .map_err(|_| E::invalid_length(v.len(), &"32 bytes"))?;
                Ok(Cid(raw))
            }

            fn visit_seq<A>(self, mut seq: A) -> core::result::Result<Cid, A::Error>
            where
                A: serde::de::SeqAccess<'v>,
            {
                let mut raw = [0u8; 32];
                for (i, slot) in raw.iter_mut().enumerate() {
                    *slot = seq.next_element()?.ok_or_else(|| {
                        <A::Error as serde::de::Error>::invalid_length(i, &"32 bytes")
                    })?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(<A::Error as serde::de::Error>::invalid_length(
                        33,
                        &"32 bytes",
                    ));
                }
                Ok(Cid(raw))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Text)
        } else {
            deserializer.deserialize_bytes(Bytes32)
        }
    }
}

/// Deliberately opaque. 42 §1.2 puts the readable form `gx1:<base32>` on the display layer,
/// and req/31 §1 settled which of the two escape routes this crate takes: option (b), where
/// gx-core stays a layer that does not know the display convention at all, so `gx1:` can only
/// be produced by a `gx-canon` function (req/31 §11 records that as the design default).
///
/// `Display` is not implemented for the same reason, and its absence is the stronger form of
/// the same rule: there is no textual rendering of a `Cid` anywhere in this crate, so no log
/// line written against gx-core can accidentally mint a second text format alongside `gx1:`.
impl fmt::Debug for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cid(opaque)")
    }
}
