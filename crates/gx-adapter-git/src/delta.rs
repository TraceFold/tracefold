//! The git delta grammar: a sequence of operations, in canonical DAG-CBOR.
//!
//! Spec: 42 §3.4 for `PlannedDelta.payload` (「adapterのみが解釈するopaqueな変更記述」), 42 §2.1 for what
//! canonical means. The rulings this module carries over from the fs grammar are **M4-13 採(a)** (a
//! sequence whose accepted length is one), **M4-07 採(c)** (the free monoid) and **N-14** (no parser
//! of an adapter's own -- the payload goes through gx-canon, which 41 §6 makes the only encoder).
//!
//! # Two operations, and why the second one exists
//!
//! ```text
//! payload := [ op* ]                      -- canonical DAG-CBOR, keys sorted by 42 §2.1-2
//! op      := { "content": bytes | null, "kind": "content", "locator": text }
//!          | { "kind": "reset", "locator": text, "target": text }
//! ```
//!
//! * **`content`** is a change to the file the locator names: 「the entry at this path, on this
//!   branch, becomes these bytes」 (or, with `null`, 「is not in the tree」). Applying it writes a blob,
//!   a tree and a commit, and moves the branch to that commit. This is **AC-050**'s 「commit操作
//!   delta」.
//! * **`reset`** is a change to the branch itself: 「this branch points at this commit」. Applying it
//!   moves one reference and writes no object. This is **AC-050**'s 「branch操作delta」, and it is also
//!   what [`crate::invert`] produces, because AC-050 asks for HEAD to come back **bit-equal** and only
//!   a reset can do that (a revert commit restores the tree and moves HEAD forward).
//!
//! Both carry the **whole** locator, path included, even though a reset acts on the reference alone.
//! That is what keeps an inverse a statement about the same object as the delta it inverts
//! (**E-M4-32**: `invert` refuses a `pre` naming another object, and the comparison is on locators).
//!
//! ## What 「連結が witness」 means when the payload is a CBOR array
//!
//! **M4-07 採(c)** makes composition a free monoid over the sequence: **N-14** requires canonical
//! DAG-CBOR and a CBOR array carries its length in its head, so two payloads do not concatenate as
//! bytes. The monoid operation is on the **sequences**: `decode`, concatenate, `encode`. The same
//! shape `gx-adapter-fs` has, for the same reason, and `tests/git_delta.rs` measures the
//! associativity that makes it one.

use gx_canon::cbor;
use gx_core::b64;
use gx_substrate::{Error, Result};
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use core::fmt;

/// How many operations v0.1 accepts (**M4-13 採(a)**, read for git).
///
/// One, and the reason is the same shape the fs adapter's is: a delta whose application is one
/// reference move is a delta an engine can reason about, and two moves of one branch are two commits
/// that this version will not make silently non-atomic. The bound is enforced in [`GitDelta::decode`]
/// rather than in the constructors -- a longer sequence is a legal *value* of the grammar (that is
/// what makes the monoid free) and an illegal *v0.1 payload*.
pub const MAX_OPS: usize = 1;

/// How deep a tree path this version acts on (crate root, 「What v0.1 does not close」).
///
/// One component. A nested path means rebuilding every tree between the file and the root, which is
/// *n* object writes for one change and *n* more shapes for a partial failure to take; 42's delta is
/// 「一つの変更」 and this version keeps that literal. Enforced in [`crate::locator::parse`], so the
/// refusal is 「that is not a position」 rather than 「the change failed」.
pub const MAX_PATH_DEPTH: usize = 1;

/// How large a **forward** payload this adapter is willing to plan (**M4H5-4 採(b)**, read for git).
///
/// The goal bytes travel through a gate and into a journal -- **E-M4-8**: 「`PlannedDelta.payload` は
/// 保管する(必須)」 -- so an unbounded plan is a cost nobody declared in a place nobody chose. One
/// declaration, as §33 requires, and `tests/git_delta.rs` asserts there is no second.
///
/// **1 MiB**, the same number the fs adapter measured this repository's own population against
/// (largest file under `crates/`, `tools/` and `policies/`: 38,292 bytes, 27 times under it). The
/// population is the same because the substrate a git adapter is pointed at is a source repository.
///
/// 🔴 **There is no matching escrow ceiling, and its absence is a finding.** The crate root argues it
/// in full: an fs inverse carries the whole old file, a git inverse carries an object id, and a
/// constant no input can reach would be a refusal nobody asked for (52 契約 2).
pub const MAX_FORWARD_PAYLOAD_BYTES: usize = 1024 * 1024;

/// A file's contents.
///
/// A byte string on the wire and base64 in a human-readable format, which is the pair M2H1-4 settled
/// for every raw byte string in this workspace: a derived `Serialize` over `Vec<u8>` writes each byte
/// as a CBOR integer, whose encoded length depends on its value, so two files of the same size would
/// not encode to the same length.
#[derive(Clone, PartialEq, Eq)]
pub struct Blob(pub Vec<u8>);

/// Opaque, like [`gx_core::GoalBytes`]'s: a file's content in a `{:?}` is both unreadable and the one
/// place a log line would quietly stop treating a payload as opaque. The length is printed because
/// the length is not the content.
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

/// One operation, in the two shapes the module documentation gives.
///
/// A tagged struct rather than a serde `enum`: 42 §2.1's canonical form sorts a map's keys, and an
/// externally tagged enum encodes as a one-key map whose value is another map -- two shapes for one
/// grammar. One flat map with a `kind` discriminant has one shape, and the discriminant is a value a
/// reader of the bytes can see without knowing serde's conventions.
///
/// Fields are private with accessors (F-6): one spelling per field, and a payload that cannot be
/// edited after the delta carrying it has been named by its CID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitOp {
    /// The new content of the entry, for a `content` operation. `None` on a `reset`, and `None` on a
    /// `content` operation that removes the entry -- the two are told apart by [`GitOp::kind`],
    /// which is why the discriminant is a field rather than an inference.
    content: Option<Blob>,
    /// `"content"` or `"reset"`.
    kind: String,
    /// The position, normalised (crate root's `≈`).
    locator: String,
    /// The commit a `reset` puts the branch at, in git's own lower-case hexadecimal.
    target: Option<String>,
}

/// The `kind` of a `content` operation.
pub const KIND_CONTENT: &str = "content";
/// The `kind` of a `reset` operation.
pub const KIND_RESET: &str = "reset";

impl GitOp {
    /// The entry at `locator` becomes `content`.
    #[must_use]
    pub fn write(locator: String, content: Vec<u8>) -> Self {
        Self {
            content: Some(Blob(content)),
            kind: KIND_CONTENT.to_string(),
            locator,
            target: None,
        }
    }

    /// The entry at `locator` is not in the tree.
    #[must_use]
    pub fn remove(locator: String) -> Self {
        Self {
            content: None,
            kind: KIND_CONTENT.to_string(),
            locator,
            target: None,
        }
    }

    /// The branch `locator` names points at `target`.
    #[must_use]
    pub fn reset(locator: String, target: String) -> Self {
        Self {
            content: None,
            kind: KIND_RESET.to_string(),
            locator,
            target: Some(target),
        }
    }

    /// Which of the two operations this is.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// What the entry becomes, or `None` for a removal or a reset.
    #[must_use]
    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_ref().map(|b| b.0.as_slice())
    }

    /// The commit a reset puts the branch at.
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// The locator as it was written.
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// The position this operation acts on: parsed, and therefore normalised.
    ///
    /// One place, so that `apply`, `invert` and `commutation` cannot disagree about which branch a
    /// payload names. [`crate::plan`] already normalises what it writes, and normalising again is
    /// free (L7's idempotence) and is what stops a hand-written payload from reaching a repository
    /// with a spelling no gate ever saw -- M3-10 fixed a policy pack's effective range at
    /// 「locator 級」, so two spellings of one position would be two policy subjects and one branch.
    ///
    /// # Errors
    /// [`Error::NotAPosition`] for a locator that names fewer than three parts, a relative
    /// repository, or a nested path. [`Error::PayloadUnreadable`] for an operation whose `kind` this
    /// grammar does not write, or whose fields do not match its `kind`.
    pub fn position(&self) -> Result<crate::locator::Position> {
        self.well_formed()?;
        crate::locator::parse(&self.locator)
    }

    /// The two shapes, checked against the discriminant.
    ///
    /// A `reset` with no target and a `content` with one are both payloads this grammar did not
    /// write, and reading either as the other would let a hand-made payload turn a file change into a
    /// branch move. [`Error::PayloadUnreadable`] is the variant for 「this claims to belong here and
    /// does not parse」, which is exactly what these are.
    ///
    /// # Errors
    /// [`Error::PayloadUnreadable`].
    pub fn well_formed(&self) -> Result<()> {
        let unreadable = |detail: String| Err(Error::PayloadUnreadable { detail });
        match self.kind.as_str() {
            KIND_CONTENT if self.target.is_some() => unreadable(format!(
                "a `{KIND_CONTENT}` operation carries no `target`; this one names {:?}",
                self.target
            )),
            KIND_CONTENT => Ok(()),
            KIND_RESET if self.content.is_some() => unreadable(format!(
                "a `{KIND_RESET}` operation carries no `content`; this one carries {} bytes",
                self.content.as_ref().map_or(0, |b| b.0.len())
            )),
            KIND_RESET if self.target.is_none() => {
                unreadable(format!("a `{KIND_RESET}` operation names the commit it resets to"))
            }
            KIND_RESET => Ok(()),
            other => unreadable(format!(
                "this grammar writes `{KIND_CONTENT}` and `{KIND_RESET}` operations and this one is \
                 {other:?} (42 §3.4: a payload is only meaningful to the adapter whose grammar wrote it)"
            )),
        }
    }
}

/// A sequence of operations: the free monoid of **M4-07 採(c)**.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitDelta {
    ops: Vec<GitOp>,
}

impl GitDelta {
    /// The one-operation sequence v0.1 accepts.
    #[must_use]
    pub fn one(op: GitOp) -> Self {
        Self { ops: vec![op] }
    }

    /// A sequence of any length.
    ///
    /// Longer sequences exist as values so that concatenation has something to be associative over;
    /// only [`MAX_OPS`] survives [`GitDelta::decode`] in v0.1.
    #[must_use]
    pub fn of(ops: Vec<GitOp>) -> Self {
        Self { ops }
    }

    /// The operations, in order.
    #[must_use]
    pub fn ops(&self) -> &[GitOp] {
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
            detail: format!("the git delta has no canonical DAG-CBOR form: {e}"),
        })
    }

    /// Read a payload back, and hold it to v0.1's length and to its own grammar.
    ///
    /// Three refusals, spelled differently on purpose:
    ///
    /// * a sequence longer than [`MAX_OPS`] is **未対応** -- the grammar can say it and this version
    ///   will not run it. That is [`Error::Unimplemented`], the one variant the shared harness reads
    ///   as 「無い」 (**§31 M4H3-4 (b)**), which is what stops a caller from reading the refusal as
    ///   damage;
    /// * the empty sequence is the monoid's unit and describes no change: legal as a value, not as a
    ///   v0.1 payload ([`Error::PayloadUnreadable`]);
    /// * bytes that are not canonical DAG-CBOR, or an operation whose `kind` and fields disagree, are
    ///   payloads this adapter would never have written ([`Error::PayloadUnreadable`]).
    ///
    /// # Errors
    /// The three above.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let ops: Vec<GitOp> = cbor::decode(payload).map_err(|e| Error::PayloadUnreadable {
            detail: format!("not a git delta (42 §2.1 canonical DAG-CBOR): {e}"),
        })?;
        if ops.len() > MAX_OPS {
            return Err(Error::Unimplemented {
                method: "apply".to_string(),
                detail: format!(
                    "v0.1 accepts a {MAX_OPS}-operation sequence and this payload carries {}; two \
                     moves of one branch are two commits, and this version does not make a change \
                     silently non-atomic (M4-13(a))",
                    ops.len()
                ),
            });
        }
        if ops.is_empty() {
            return Err(Error::PayloadUnreadable {
                detail: "the empty sequence is the unit of the monoid and describes no change, so \
                         it is not a v0.1 payload (M4-13(a))"
                    .to_string(),
            });
        }
        for op in &ops {
            op.well_formed()?;
        }
        Ok(Self { ops })
    }
}
