// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The MCP delta grammar: a sequence of tool calls, in canonical DAG-CBOR.
//!
//! Spec: 42 §3.4 for `PlannedDelta.payload` ("an opaque description of the change that only the adapter interprets"), 42 §2.1 for what (sem: SEM-gx-adapter-mcp-141)
//! canonical means. The rulings this module carries over from the fs and git grammars are
//! **M4-13, adopted (a)** (a sequence whose accepted length is one), **M4-07, adopted (c)** (the free monoid) and (sem: SEM-gx-adapter-mcp-142)
//! **N-14** (no parser of an adapter's own -- the payload goes through gx-canon, which 41 §6 makes the
//! only encoder).
//!
//! # One operation, and why there is no discriminant
//!
//! ```text
//! payload := [ op* ]                  -- canonical DAG-CBOR, keys sorted by 42 §2.1-2
//! op      := { "arguments": bytes, "locator": text, "tool": text }
//! ```
//!
//! "call the tool named `tool`, at the server the locator names, with these arguments; the object this (sem: SEM-gx-adapter-mcp-143)
//! change is about is the resource the locator names". `gx-adapter-git` writes two shapes because its (sem: SEM-gx-adapter-mcp-144)
//! forward change (a commit) and its inverse (a reference reset) are different operations on different
//! things. Here they are not: **the inverse of a tool call is another tool call**, the compensating one
//! a [`crate::catalogue::Catalogue`] declares, and a `kind` field distinguishing them would be a
//! a discriminant with no reader -- 52 contract 2's "nobody asked for this" at the size of a field. (sem: SEM-gx-adapter-mcp-145)
//!
//! What tells a forward call from an inverse is therefore **the escrow**, which is where 43 T-10b puts
//! it, and not a word in the payload.
//!
//! ## The restore convention (normative)
//!
//! An inverse has to carry the body: 42 §5, verbatim: "since a digest-only form makes an actual undo physically impossible". This (sem: SEM-gx-adapter-mcp-146)
//! grammar carries it inside `arguments`, as [`restore_arguments`] builds it -- canonical DAG-CBOR of
//! `{ "contents": bytes, "uri": text }`. The two field names are **MCP's own**: a `resources/read`
//! result is a list of contents each carrying its `uri`, so a restore tool that already speaks the
//! protocol is being handed the shape it already reads. Inventing a third spelling would have made the
//! deployment translate between two conventions that mean one thing.
//!
//! 🔴 **Scope narrowed by A2 (req/38 §92, ruling 1), not removed**: this form is right for a restore tool (sem: SEM-gx-adapter-mcp-147)
//! that *reads* the `resources/read` pair (the fs/notes demo, req/136's mock) and demonstrably wrong
//! for a structured-argument API tool -- req/152 §5 measured github-mcp-server refusing exactly these
//! two members with "missing required parameter". It remains the **default** (a catalogue entry with (sem: SEM-gx-adapter-mcp-148)
//! no template), and a [`crate::catalogue::RestoreTemplate`] is the declared alternative: the inverse's
//! `arguments` are then UTF-8 JSON of the restore tool's own members, the same encoding a forward
//! call's arguments already travel in, resolved at escrow time in `crate::invert`. Both shapes are
//! this grammar's `arguments` bytes; nothing about `McpOp` or the payload changes between them.
//!
//! ## What "concatenation is the witness" means when the payload is a CBOR array (sem: SEM-gx-adapter-mcp-149)
//!
//! **M4-07, adopted (c)** makes composition a free monoid over the sequence: **N-14** requires canonical (sem: SEM-gx-adapter-mcp-150)
//! DAG-CBOR and a CBOR array carries its length in its head, so two payloads do not concatenate as
//! bytes. The monoid operation is on the **sequences**: `decode`, concatenate, `encode` -- the same
//! shape the other two adapters have, for the same reason.

use core::fmt;

use gx_canon::cbor;
use gx_core::b64;
use gx_substrate::{Error, Result};
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How many operations v0.1 accepts (**M4-13, adopted (a)**, read for tool calls). (sem: SEM-gx-adapter-mcp-151)
///
/// One. Two calls in one delta are two **effects**, and an adapter that ran them under one delta would
/// be one where a failure between them leaves a change half made with no vocabulary to say so. The
/// bound is enforced in [`McpDelta::decode`] rather than in the constructors -- a longer sequence is a
/// legal *value* of the grammar (that is what makes the monoid free) and an illegal *v0.1 payload*.
pub const MAX_OPS: usize = 1;

/// How large a **forward** payload this adapter is willing to plan (**M4H5-4, adopted (b)**, read for MCP). (sem: SEM-gx-adapter-mcp-152)
///
/// The payload travels through a gate and into a journal -- **E-M4-8**: "`PlannedDelta.payload` is stored (sem: SEM-gx-adapter-mcp-153)
/// (mandatory)" -- so an unbounded plan is a cost nobody declared in a place nobody chose. One (sem: SEM-gx-adapter-mcp-154)
/// declaration, as §33 requires, and `tests/mcp_delta.rs` asserts there is no second.
///
/// **1 MiB**, the number the other two adapters measured against this repository's own population. The
/// arguments of a tool call are a JSON object; a megabyte of them is already far outside what an MCP
/// client sends, and the bound is here to be reachable rather than to be tight.
pub const MAX_FORWARD_PAYLOAD_BYTES: usize = 1024 * 1024;

/// How large an **escrowed inverse** this adapter is willing to carry back (**M4-21, adopted (a)**). (sem: SEM-gx-adapter-mcp-155)
///
/// 🔴 **Declared here and deliberately not declared by `gx-adapter-git`.** req/99 §3 D-4 is the rule:
/// a git inverse carries an object id, because a git object database is content-addressed and the old
/// content is already in it, so a ceiling there could not be reached by any input and would be "a (sem: SEM-gx-adapter-mcp-156)
/// refusal nobody asked for" (52 contract 2). An MCP server offers no such store. The inverse of a tool (sem: SEM-gx-adapter-mcp-157)
/// call carries **the resource's prior contents**, exactly as an fs inverse carries the old file, so
/// the ceiling is reachable, is declared, and `invert` answers `Ok(None)` over it -- which **E-M3-4**
/// turns into an escalation to a person.
///
/// `MAX_FORWARD_PAYLOAD_BYTES <= MAX_INVERSE_PAYLOAD_BYTES`, which is the fs adapter's relation and
/// its reason: were the forward bound the larger, an accepted change could be structurally unundoable
/// for a size reason -- `Ok(None)` by construction on a path this adapter itself opened.
pub const MAX_INVERSE_PAYLOAD_BYTES: usize = 1024 * 1024;

/// A byte string: a tool's arguments, or a resource's contents.
///
/// A byte string on the wire and base64 in a human-readable format, which is the pair M2H1-4 settled
/// for every raw byte string in this workspace: a derived `Serialize` over `Vec<u8>` writes each byte
/// as a CBOR integer, whose encoded length depends on its value, so two argument objects of the same
/// size would not encode to the same length.
#[derive(Clone, PartialEq, Eq)]
pub struct Bytes(pub Vec<u8>);

/// Opaque, like [`gx_core::GoalBytes`]'s. Arguments are an adapter's grammar, and a tool call's
/// arguments are also the most likely place in this whole system for a credential to be sitting; a
/// `{:?}` that printed them would put it in every log line that ever debug-formatted a delta. The
/// length is printed because the length is not the content.
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bytes(opaque, {} bytes)", self.0.len())
    }
}

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&b64::encode(&self.0))
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> core::result::Result<Self, D::Error> {
        struct Raw;

        impl<'v> Visitor<'v> for Raw {
            type Value = Bytes;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a byte string, or base64 (RFC 4648 §4) in a human-readable format")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> core::result::Result<Bytes, E> {
                b64::decode(v).map(Bytes).map_err(E::custom)
            }

            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> core::result::Result<Bytes, E> {
                Ok(Bytes(v.to_vec()))
            }

            fn visit_byte_buf<E: serde::de::Error>(
                self,
                v: Vec<u8>,
            ) -> core::result::Result<Bytes, E> {
                Ok(Bytes(v))
            }

            fn visit_seq<A: SeqAccess<'v>>(
                self,
                mut seq: A,
            ) -> core::result::Result<Bytes, A::Error> {
                let mut out = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    out.push(byte);
                }
                Ok(Bytes(out))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_any(Raw)
        } else {
            deserializer.deserialize_bytes(Raw)
        }
    }
}

/// What an intent's `goal` says, in this adapter's grammar (42 §3.3, P-6).
///
/// 42 §3.3 gives an [`gx_core::Intent`] a `locator` and a `goal` and says nothing about what the goal
/// *is*: the bytes are carried opaquely by [`gx_core::GoalBytes`] (**E-M4-2**) and "what this adapter's (sem: SEM-gx-adapter-mcp-158)
/// grammar says about those bytes" is the whole of the interpretation P-6 reserves to an adapter. For (sem: SEM-gx-adapter-mcp-159)
/// `gx-adapter-fs` and `gx-adapter-git` a goal is the object's new content. **Here it is the tool
/// call**, because FR-046's subject is "tool-call intent" -- what an agent asked to do is "call this (sem: SEM-gx-adapter-mcp-160)
/// tool with these arguments", and a proxy that decided which tool realises a desired file content (sem: SEM-gx-adapter-mcp-161)
/// would be deciding, not gating.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolIntent {
    arguments: Bytes,
    tool: String,
}

impl ToolIntent {
    /// Build one.
    #[must_use]
    pub fn new(tool: impl Into<String>, arguments: Vec<u8>) -> Self {
        Self {
            arguments: Bytes(arguments),
            tool: tool.into(),
        }
    }

    /// The tool's name, as the server publishes it.
    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// The arguments, opaque to everything above the adapter.
    #[must_use]
    pub fn arguments(&self) -> &[u8] {
        &self.arguments.0
    }

    /// The goal bytes of an intent that asks for this call.
    ///
    /// # Errors
    /// [`Error::NotDigestible`] when gx-canon has no canonical form for the pair.
    pub fn encode(&self) -> Result<Vec<u8>> {
        cbor::encode(self).map_err(|e| Error::NotDigestible {
            detail: format!("this tool call has no canonical DAG-CBOR form: {e}"),
        })
    }

    /// Read a goal back.
    ///
    /// # Errors
    /// [`Error::NotPlannable`] when the bytes are not this adapter's goal grammar. `NotPlannable` and
    /// not `PayloadUnreadable`: a goal is an **argument to `plan`**, and "no delta plans this intent (sem: SEM-gx-adapter-mcp-162)
    /// against this snapshot" is exactly what 41 §4 documents for a `plan` that cannot proceed. A (sem: SEM-gx-adapter-mcp-163)
    /// payload is a different value with a different reader.
    pub fn decode(goal: &[u8]) -> Result<Self> {
        let decoded: Self = cbor::decode(goal).map_err(|e| Error::NotPlannable {
            detail: format!(
                "the goal of an mcp intent is a canonical DAG-CBOR `{{arguments, tool}}` (this \
                 adapter's grammar for \"tool-call intent\", FR-046): {e} (sem: SEM-gx-adapter-mcp-164)"
            ),
        })?;
        if decoded.tool.is_empty() {
            return Err(Error::NotPlannable {
                detail: "the goal names no tool, and \"call the empty tool\" is not a change this (sem: SEM-gx-adapter-mcp-165) \
                         adapter can plan"
                    .to_string(),
            });
        }
        Ok(decoded)
    }
}

/// The arguments an inverse hands to a restore tool (module documentation, "the restore convention"). (sem: SEM-gx-adapter-mcp-166)
///
/// # Errors
/// [`Error::NotDigestible`] when the pair has no canonical form.
pub fn restore_arguments(uri: &str, contents: &[u8]) -> Result<Vec<u8>> {
    #[derive(Serialize)]
    struct Restore<'a> {
        contents: &'a Bytes,
        uri: &'a str,
    }
    let contents = Bytes(contents.to_vec());
    cbor::encode(&Restore {
        contents: &contents,
        uri,
    })
    .map_err(|e| Error::NotDigestible {
        detail: format!("the restore arguments have no canonical DAG-CBOR form: {e}"),
    })
}

/// One operation: a tool call at a position.
///
/// Fields are private with accessors (F-6): one spelling per field, and a payload that cannot be
/// edited after the delta carrying it has been named by its CID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOp {
    /// What the tool is handed. Opaque to everything above this adapter (P-6).
    arguments: Bytes,
    /// The position, normalised (crate root's `≈`).
    locator: String,
    /// 🔴 The members still owed to `arguments`, each with the do-time source that will supply it
    /// (two-phase escrow, `req/38` §98, ruling 1 / §99, ruling 2). (sem: SEM-gx-adapter-mcp-167)
    ///
    /// Empty for every forward call and for every complete inverse — and **absent from the wire
    /// when empty** (`skip_serializing_if`), so the canonical DAG-CBOR of every payload this
    /// grammar wrote before this field existed is byte-identical still (the CID of an existing
    /// delta does not move; `tests/mcp_delta.rs` pins the raw bytes). Non-empty only in a
    /// **partial escrowed inverse**: the completion step (`McpAdapter` as
    /// `gx_substrate::InverseCompletion`) reads these declarations out of the escrowed value
    /// itself and fills `arguments` from the journalled observation — the instructions travel
    /// inside the escrow because a recovery may hold the journal and the blob store and nothing
    /// else.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pending: std::collections::BTreeMap<String, crate::catalogue::ArgSource>,
    /// The tool to call, as the server publishes it.
    tool: String,
}

impl McpOp {
    /// "Call `tool` at the server `locator` names, with `arguments`". (sem: SEM-gx-adapter-mcp-168)
    #[must_use]
    pub fn call(locator: String, tool: String, arguments: Vec<u8>) -> Self {
        Self {
            arguments: Bytes(arguments),
            locator,
            pending: std::collections::BTreeMap::new(),
            tool,
        }
    }

    /// The partial form: the same call, still owed the named do-time members (two-phase escrow).
    #[must_use]
    pub fn call_pending(
        locator: String,
        tool: String,
        arguments: Vec<u8>,
        pending: std::collections::BTreeMap<String, crate::catalogue::ArgSource>,
    ) -> Self {
        Self {
            arguments: Bytes(arguments),
            locator,
            pending,
            tool,
        }
    }

    /// The members still owed, with their do-time sources. Empty for every complete operation.
    #[must_use]
    pub fn pending(&self) -> &std::collections::BTreeMap<String, crate::catalogue::ArgSource> {
        &self.pending
    }

    /// The tool's name.
    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// What the tool is handed.
    #[must_use]
    pub fn arguments(&self) -> &[u8] {
        &self.arguments.0
    }

    /// The locator as it was written.
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// The position this operation acts on: parsed, and therefore normalised.
    ///
    /// One place, so that `apply`, `invert` and `commutation` cannot disagree about which server a
    /// payload names. [`crate::plan`] already normalises what it writes, and normalising again is free
    /// (L7's idempotence) and is what stops a hand-written payload from reaching a server with a
    /// spelling no gate ever saw -- M3-10 fixed a policy pack's effective range at "the locator level", so two (sem: SEM-gx-adapter-mcp-169)
    /// spellings of one position would be two policy subjects and one resource.
    ///
    /// # Errors
    /// [`Error::NotAPosition`] for a locator that does not name a scheme-carrying server and a
    /// resource. [`Error::PayloadUnreadable`] for an operation that names no tool.
    pub fn position(&self) -> Result<crate::locator::Position> {
        self.well_formed()?;
        crate::locator::parse(&self.locator)
    }

    /// The one shape, checked.
    ///
    /// A payload that names no tool is a payload this grammar did not write, and running it would mean
    /// asking a server to do "". [`Error::PayloadUnreadable`] is the variant for "this claims to belong (sem: SEM-gx-adapter-mcp-170)
    /// here and does not parse", which is exactly what it is. (sem: SEM-gx-adapter-mcp-171)
    ///
    /// # Errors
    /// [`Error::PayloadUnreadable`].
    pub fn well_formed(&self) -> Result<()> {
        if self.tool.is_empty() {
            return Err(Error::PayloadUnreadable {
                detail: "an mcp operation names the tool it calls, and this one names none (42 \
                         §3.4: a payload is only meaningful to the adapter whose grammar wrote it)"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// A sequence of operations: the free monoid of **M4-07, adopted (c)**. (sem: SEM-gx-adapter-mcp-172)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpDelta {
    ops: Vec<McpOp>,
}

impl McpDelta {
    /// The one-operation sequence v0.1 accepts.
    #[must_use]
    pub fn one(op: McpOp) -> Self {
        Self { ops: vec![op] }
    }

    /// A sequence of any length.
    ///
    /// Longer sequences exist as values so that concatenation has something to be associative over;
    /// only [`MAX_OPS`] survives [`McpDelta::decode`] in v0.1.
    #[must_use]
    pub fn of(ops: Vec<McpOp>) -> Self {
        Self { ops }
    }

    /// The operations, in order.
    #[must_use]
    pub fn ops(&self) -> &[McpOp] {
        &self.ops
    }

    /// The payload bytes: canonical DAG-CBOR, through gx-canon (**N-14**, 41 §6).
    ///
    /// # Errors
    /// [`Error::NotDigestible`] when gx-canon has no canonical form for the sequence. A value that
    /// cannot be encoded has no identity, which is the same reading `PlannedDelta::new` gives the same
    /// variant.
    pub fn encode(&self) -> Result<Vec<u8>> {
        cbor::encode(&self.ops).map_err(|e| Error::NotDigestible {
            detail: format!("the mcp delta has no canonical DAG-CBOR form: {e}"),
        })
    }

    /// Read a payload back, and hold it to v0.1's length and to its own grammar.
    ///
    /// Three refusals, spelled differently on purpose:
    ///
    /// * a sequence longer than [`MAX_OPS`] is **unsupported** -- the grammar can say it and this version (sem: SEM-gx-adapter-mcp-173)
    ///   will not run it. That is [`Error::Unimplemented`], the one variant the shared harness reads
    ///   as "none" (**§31 M4H3-4 (b)**), which is what stops a caller from reading the refusal as (sem: SEM-gx-adapter-mcp-174)
    ///   damage;
    /// * the empty sequence is the monoid's unit and describes no change: legal as a value, not as a
    ///   v0.1 payload ([`Error::PayloadUnreadable`]);
    /// * bytes that are not canonical DAG-CBOR, or an operation that names no tool, are payloads this
    ///   adapter would never have written ([`Error::PayloadUnreadable`]).
    ///
    /// # Errors
    /// The three above.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let ops: Vec<McpOp> = cbor::decode(payload).map_err(|e| Error::PayloadUnreadable {
            detail: format!("not an mcp delta (42 §2.1 canonical DAG-CBOR): {e}"),
        })?;
        if ops.len() > MAX_OPS {
            return Err(Error::Unimplemented {
                method: "apply".to_string(),
                detail: format!(
                    "v0.1 accepts a {MAX_OPS}-operation sequence and this payload carries {}; two \
                     tool calls in one delta are two effects, and a failure between them would \
                     leave a change half made with no vocabulary to say so (M4-13(a))",
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
