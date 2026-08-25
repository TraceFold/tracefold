// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The fs delta grammar: a sequence of whole-file operations, in canonical DAG-CBOR.
//!
//! Spec: 42 §3.4 for `PlannedDelta.payload` ("an opaque description of the change that only the adapter interprets"), 42 §2.1 for
//! what canonical means. The rulings are **M4-13, adopted (a)** (the shape and the length), **M4-07, adopted (c)** (sem: SEM-gx-adapter-fs-029)
//! (the free monoid), and **N-14** (no parser of this adapter's own).
//!
//! # The shape, and the two rulings that fix it
//!
//! **M4-13, adopted (a)**: "v0.1's fs delta is a **single whole-file replacement** -- atomicity is from `rename`...**v0.1's `apply` accepts
//! only a sequence of `len==1`**; `len>1` is Err (unimplemented, stated explicitly -- it does not run non-atomically in silence, i.e. fail-closed)". The reason
//! the accepted length is one is 45 §3: TH-3's residual condition is "`adapter.apply` is not a single syscall
//! (e.g. multiple file writes)", and one `rename` is one syscall. (sem: SEM-gx-adapter-fs-030)
//!
//! **M4-07, adopted (c)** is why the shape is a *sequence* even so: "a composite delta is a **free monoid** -- the fs
//! payload is a 'sequence of single-file operations', and concatenating the sequence is the witness of composition". A grammar that admitted one (sem: SEM-gx-adapter-fs-031)
//! operation and had no room for two would have nothing for composition to be.
//!
//! ## What "concatenation is the witness" means when the payload is a CBOR array (sem: SEM-gx-adapter-fs-032)
//!
//! **N-14** requires the payload to be canonical DAG-CBOR -- "it would be a different matter if the adapter wrote its own
//! parser" -- and a CBOR array carries its length in its head, so two payloads do not concatenate as (sem: SEM-gx-adapter-fs-033)
//! bytes. The monoid operation is therefore on the **sequences**: `decode`, concatenate, `encode`.
//! `tests/fs_delta.rs` measures the associativity that makes it one, and the crate root of
//! `gx-substrate` is where gx refuses the general law about composite arrows that this does **not**
//! establish.
//!
//! # The wire shape, in one line
//!
//! ```text
//! payload := [ op* ]              -- canonical DAG-CBOR, keys sorted by 42 §2.1-2
//! op      := { "content": bytes | null, "locator": text }
//! ```
//!
//! `null` content is a removal. AC-049 (hand 5) asks for "creation / change / deletion, the three kinds", so all three have a
//! spelling here; hand 4 plans only the replacement, because an [`gx_core::Intent`] carries a goal
//! and 42 §3.3 gives "remove this" no spelling of its own. (sem: SEM-gx-adapter-fs-034)

use gx_canon::cbor;
use gx_core::b64;
use gx_substrate::{Error, Result};
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use core::fmt;

/// How many operations v0.1 accepts (**M4-13, adopted (a)**). (sem: SEM-gx-adapter-fs-035)
///
/// One constant, so that hand 5's `apply` and this decoder cannot disagree about the bound, and so
/// that the day v0.2 raises it is one edit. The bound is enforced in [`FsDelta::decode`] rather than
/// in the constructors: a longer sequence is a legal *value* of the grammar (that is what makes the
/// monoid free) and an illegal *v0.1 payload*.
pub const MAX_OPS: usize = 1;

/// How large an **inverse** payload this adapter is willing to escrow (**M4-21, adopted (a)**).
///
/// req/38 §28, verbatim: "the ceiling on the inverse delta payload is declared **in exactly one constant**; exceeding it is
/// `invert` = `Ok(None)` (**the 1st reason AC-048's `None` actually occurs**). The value is decided by hand 5, with the reasoning printed (this ruling fixes only the shape)". This is the one (sem: SEM-gx-adapter-fs-036)
/// declaration; `crates/gx-adapter-fs/tests/invert_ceiling.rs` asserts that there is no second one and
/// that the contract row in `gx-substrate`'s trait documentation names it (**M4H2-8**).
///
/// # Why a ceiling exists at all
///
/// 42 §5 requires the escrowed inverse to carry the **body** and not a digest -- "because a digest-only inverse makes
/// an actual undo physically impossible" -- and an fs inverse of a whole-file replacement carries the whole old (sem: SEM-gx-adapter-fs-037)
/// file. So an undoable transformation costs its file's size twice: once in the forward payload and
/// once in the escrow. Without a bound, one `apply` over a large file would put an unbounded body into
/// the journal an engine keeps (M5), and the alternative to a bound is not "no cost" but "a cost
/// nobody declared". (sem: SEM-gx-adapter-fs-038)
///
/// # Why **1 MiB**, measured rather than asserted
///
/// The number is chosen against this repository, because that is the substrate gx's own use touches
/// and the only population this hand can measure rather than imagine (2026-08-09, `git ls-files`):
///
/// * **486 tracked files, of which 2 are over this ceiling** -- both raw research captures under
///   `req/15_math_rigor/raw/` (4,229,884 and 1,316,035 bytes), neither of them source.
/// * **The largest file under `crates/`, `tools/` and `policies/` is 38,292 bytes**
///   (`crates/gx-core/src/transformation.rs`), which is **27 times** under the ceiling.
///
/// So every source file gx is made of stays invertible with room to spare, and what loses its undo is
/// a bulk capture -- which is the boundary v0.1 chooses on purpose: an `Ok(None)` is not a refusal to
/// act, it is an escalation to a human (**E-M3-4**), and asking a human before overwriting a
/// four-megabyte capture with no way back is the answer this project wants.
///
/// A power of two so that the day v0.2 moves it, the move is legible; one edit, because there is one
/// declaration.
pub const MAX_INVERSE_PAYLOAD_BYTES: usize = 1024 * 1024;

/// How large a **forward** payload this adapter is willing to plan (**M4H5-4, adopted (b)**). (sem: SEM-gx-adapter-fs-039)
///
/// req/38 §33, verbatim: "the forward payload is a **separate constant** (the escrow ceiling asks 'can it be carried', the forward
/// ceiling asks 'will it be accepted' -- gate/journal resource protection makes the two judgements differ). One declaration
/// site + a 1:1 contract-row probe (M4H2-8 shape); the value is decided by hand 6, with the reasoning printed". Hand 5 raised the gap it closes (req/74 §2 M4H5-4): the inverse had a bound and the (sem: SEM-gx-adapter-fs-040)
/// forward direction had none, so a plan of any size travelled through a gate and into a journal.
///
/// # Why it is a second constant rather than the first one reused
///
/// The two bound different bytes for different reasons. [`MAX_INVERSE_PAYLOAD_BYTES`] asks "can this
/// adapter carry the **old** content back?" and its answer is `Ok(None)`, which **E-M3-4** escalates
/// to a human. This asks "will this adapter accept the **new** content at all?" and its answer is a
/// refusal to plan, because a payload is kept (**E-M4-8**: "`PlannedDelta.payload` is stored (mandatory)") (sem: SEM-gx-adapter-fs-041)
/// and an unbounded one is a cost nobody declared in a place nobody chose. Two questions that can be
/// answered differently need two numbers that can move separately, even on a day they agree.
///
/// # Why **1 MiB**, and what its relation to the escrow bound is
///
/// The same measured population hand 5 argued the escrow ceiling against (2026-08-09, `git ls-files`
/// over this repository): the largest file under `crates/`, `tools/` and `policies/` is **38,292
/// bytes** -- 27 times under this number -- and the only two tracked files above it are raw research
/// captures under `req/15_math_rigor/raw/`, neither of them source. So every file gx is made of is
/// plannable with room to spare, and what a v0.1 fs adapter declines to change in one delta is a bulk
/// capture.
///
/// The relation is the reason the two numbers are equal today, and it is one line:
/// **`MAX_FORWARD_PAYLOAD_BYTES <= MAX_INVERSE_PAYLOAD_BYTES`** means every change this adapter
/// accepts is one whose result it could also have escrowed back. Were this the larger, the adapter
/// would be admitting changes it had already decided it could not carry an inverse for -- `Ok(None)`
/// by construction, on a path it opened itself.
///
/// # Where the bound is enforced, and where it is not
///
/// In [`crate::plan`], which is where a goal becomes a payload. It is **not** in
/// [`FsDelta::decode`], so a payload written by hand still applies -- the same split [`MAX_OPS`] has
/// between a legal value and a legal v0.1 payload, except that `MAX_OPS` is enforced at decode and
/// this is not. §33 M4H5-4 (b) put the refusal on the **plan side** and this hand did not widen it; the residue (sem: SEM-gx-adapter-fs-042)
/// is raised in `req/75` §2 rather than closed by a hand that was not asked to.
pub const MAX_FORWARD_PAYLOAD_BYTES: usize = 1024 * 1024;

/// A file's contents.
///
/// A byte string on the wire and base64 in a human-readable format, which is the pair M2H1-4 settled
/// for every raw byte string in this workspace: a derived `Serialize` over `Vec<u8>` writes each byte
/// as a CBOR integer, whose encoded length depends on its value, so two files of the same size would
/// not encode to the same length.
#[derive(Clone, PartialEq, Eq)]
pub struct Blob(pub Vec<u8>);

/// Opaque, like [`gx_core::GoalBytes`]'s: a file's content in a `{:?}` is both unreadable and the
/// one place a log line would quietly stop treating a payload as opaque. The length is printed
/// because the length is not the content.
impl fmt::Debug for Blob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blob(opaque, {} bytes)", self.0.len())
    }
}

impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&b64::encode(&self.0))
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> core::result::Result<Self, D::Error> {
        struct Bytes;

        impl<'v> Visitor<'v> for Bytes {
            type Value = Blob;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a byte string, or base64 (RFC 4648 §4) in a human-readable format")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> core::result::Result<Blob, E> {
                b64::decode(v).map(Blob).map_err(E::custom)
            }

            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> core::result::Result<Blob, E> {
                Ok(Blob(v.to_vec()))
            }

            fn visit_byte_buf<E: serde::de::Error>(
                self,
                v: Vec<u8>,
            ) -> core::result::Result<Blob, E> {
                Ok(Blob(v))
            }

            fn visit_seq<A: SeqAccess<'v>>(
                self,
                mut seq: A,
            ) -> core::result::Result<Blob, A::Error> {
                let mut out = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    out.push(byte);
                }
                Ok(Blob(out))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_any(Bytes)
        } else {
            deserializer.deserialize_bytes(Bytes)
        }
    }
}

/// One whole-file operation (**M4-13, adopted (a)**). (sem: SEM-gx-adapter-fs-043)
///
/// Fields are private with accessors (F-6): one spelling per field, and a payload that cannot be
/// edited after the delta that carries it has been named by its CID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsOp {
    content: Option<Blob>,
    locator: String,
}

impl FsOp {
    /// Replace the whole file at `locator` with `content`.
    ///
    /// Whole replacement rather than a patch, which is M4-13, adopted (a) in one word: the atomicity of the (sem: SEM-gx-adapter-fs-044)
    /// change is the atomicity of a `rename`, and a rename replaces a file entire.
    #[must_use]
    pub fn write(locator: String, content: Vec<u8>) -> Self {
        Self {
            content: Some(Blob(content)),
            locator,
        }
    }

    /// Leave nothing at `locator`.
    #[must_use]
    pub fn remove(locator: String) -> Self {
        Self {
            content: None,
            locator,
        }
    }

    /// Where the operation applies -- a normalised absolute locator (**E-M4-12**, **ASM-69-3**).
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// What the file becomes, or `None` for a removal.
    #[must_use]
    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_ref().map(|b| b.0.as_slice())
    }

    /// The position this operation acts on: normalised, and from the root (**ASM-69-3**).
    ///
    /// One place, so that `apply` and `invert` cannot disagree about which file a payload names.
    /// [`crate::plan`] already normalises what it writes, and normalising again is free (L7's
    /// idempotence) and is what stops a hand-written payload from reaching the filesystem with a
    /// spelling no gate ever saw -- M3-10 fixed a policy pack's effective range at the "locator level", so (sem: SEM-gx-adapter-fs-045)
    /// two spellings of one position would be two policy subjects and one file.
    ///
    /// # Errors
    /// [`Error::NotAPosition`] for a relative locator. v0.1 names positions absolutely
    /// (**ASM-69-3**), and the refusal is here rather than in the grammar because a relative locator
    /// is a legal *value* (L7 is defined over every string) and an illegal thing to act on.
    ///
    /// Hand 5 spelled this refusal [`Error::ApplyFailed`] and raised it against itself (req/74 §2
    /// M4H5-5); §33, adopted (b) gave it a word of its own, because 43 T-11 turns `ApplyFailed` into (sem: SEM-gx-adapter-fs-046)
    /// `AbortReason::ApplyFailed` and would have recorded a change that failed where no change was
    /// ever describable.
    pub fn position(&self) -> Result<String> {
        let normalised = crate::locator::normalize(&self.locator);
        if !crate::locator::is_absolute(&normalised) {
            return Err(Error::NotAPosition {
                locator: self.locator.clone(),
                normalised,
            });
        }
        Ok(normalised)
    }
}

/// A sequence of operations: the free monoid of **M4-07, adopted (c)**. (sem: SEM-gx-adapter-fs-047)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsDelta {
    ops: Vec<FsOp>,
}

impl FsDelta {
    /// The one-operation sequence v0.1 accepts.
    #[must_use]
    pub fn one(op: FsOp) -> Self {
        Self { ops: vec![op] }
    }

    /// A sequence of any length.
    ///
    /// Longer sequences exist as values so that concatenation has something to be associative over;
    /// only [`MAX_OPS`] survives [`FsDelta::decode`] in v0.1.
    #[must_use]
    pub fn of(ops: Vec<FsOp>) -> Self {
        Self { ops }
    }

    /// The operations, in order.
    #[must_use]
    pub fn ops(&self) -> &[FsOp] {
        &self.ops
    }

    /// The payload bytes: canonical DAG-CBOR, through gx-canon (**N-14**, 41 §6).
    ///
    /// # Errors
    /// [`Error::NotDigestible`] when gx-canon has no canonical form for the sequence. A value that
    /// cannot be encoded has no identity, which is the same reading `PlannedDelta::new` gives the
    /// same variant.
    pub fn encode(&self) -> Result<Vec<u8>> {
        cbor::encode(&self.ops).map_err(|e| Error::NotDigestible {
            detail: format!("the fs delta has no canonical DAG-CBOR form: {e}"),
        })
    }

    /// Read a payload back, and hold it to v0.1's length (**M4-13, adopted (a)**). (sem: SEM-gx-adapter-fs-048)
    ///
    /// The two refusals are different facts and are spelled differently on purpose:
    ///
    /// * A sequence longer than [`MAX_OPS`] is **unimplemented** -- the grammar can say it and this version
    ///   will not run it, because two file writes are not one syscall (45 §3's TH-3 condition) and
    ///   "it does not run non-atomically in silence, i.e. fail-closed". That is [`Error::Unimplemented`], the same word the (sem: SEM-gx-adapter-fs-049)
    ///   adapter uses for `apply`, and it is what stops a caller from reading the refusal as damage.
    /// * Anything else -- bytes that are not canonical DAG-CBOR, an array of something else, the
    ///   empty sequence -- is a payload this adapter would never have written, which is
    ///   [`Error::PayloadUnreadable`]. The empty sequence is the monoid's unit and describes no file
    ///   operation: legal as a value, not as a v0.1 payload.
    ///
    /// # Errors
    /// The two above.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let ops: Vec<FsOp> = cbor::decode(payload).map_err(|e| Error::PayloadUnreadable {
            detail: format!("not an fs delta (42 §2.1 canonical DAG-CBOR): {e}"),
        })?;
        if ops.len() > MAX_OPS {
            return Err(Error::Unimplemented {
                method: "apply".to_string(),
                detail: format!(
                    "v0.1 accepts a {MAX_OPS}-operation sequence and this payload carries {}; a \
                     multi-file change is not one `rename` (M4-13(a), 45 §3 TH-3)",
                    ops.len()
                ),
            });
        }
        if ops.is_empty() {
            return Err(Error::PayloadUnreadable {
                detail: "the empty sequence is the unit of the monoid and describes no file \
                         operation, so it is not a v0.1 payload (M4-13(a))"
                    .to_string(),
            });
        }
        Ok(Self { ops })
    }
}
