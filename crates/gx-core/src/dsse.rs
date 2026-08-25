// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The DSSE signature, as data.
//!
//! Spec: 42 §3.10 for the field table (`DsseSignature { keyid: KeyId, sig: Vec<u8> }`), 41 §2 for
//! the crate that signs.
//!
//! # Why a gx-witness type lives in gx-core
//!
//! 42 §3.11 gives `Checkpoint` a `signature: DsseSignature` and 42 §0 files `DsseSignature` under
//! `gx-witness/dsse.rs`, while 42 §3.10 gives `ReceiptPayload` (gx-witness) an `InclusionProof`
//! from gx-log. Written literally the two crates name each other, and cargo does not build a
//! cycle. **E-M2-1** (`req/38_ERRATA_2026-08-07.md` §8) rules it the way A-1 ruled `Cid`: the type
//! comes down to the layer both sides already depend on, and the work stays up.
//!
//! So this file holds the *shape* of a signature and produces none. Making one means a PAE
//! encoding and an Ed25519 key, both of which are `gx-witness::dsse` (hand 5); `DsseEnvelope`
//! itself does not come down here, because nothing below gx-witness carries an envelope.
//! A `DsseSignature` built by hand is 64-odd bytes in a struct, not a claim that anything verifies.

use crate::context::KeyId;
use serde::{Deserialize, Serialize};

/// One signature over a DSSE pre-authentication encoding (42 §3.10).
///
/// `keyid` matches `ReceiptPayload.key_id`, which is how a verifier picks the key to try before
/// spending a curve operation. `sig` is the raw signature; its length is not checked here --
/// Ed25519 makes it 64 bytes, and a value that is not is exactly the malformed envelope AC-019
/// asks the verifier to reject. A container that refused it would move that refusal to the point
/// where the bytes are parsed, where it cannot be reported as a signature failure.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DsseSignature {
    /// Which key signed (42 §3.10). The same namespace as `ReceiptPayload.key_id` and
    /// [`crate::context::Actor`]'s key, so verifier and record join without a translation table.
    pub keyid: KeyId,
    /// Raw signature bytes. See `raw_bytes` below for why this field is not left to the derive.
    #[serde(with = "raw_bytes")]
    pub sig: Vec<u8>,
}

/// Raw bytes as a CBOR byte string, not as a list of small integers.
///
/// The derive would emit `Vec<u8>` the way serde emits any sequence, and in DAG-CBOR that is a
/// 64-element array whose elements above 23 each grow a header byte -- so the encoded length of a
/// signature would depend on how many of its bytes happen to be large. 42 §1.1 states the rule for
/// `Cid` ("stored directly as a 32-byte byte-string, major type 2"; sem: SEM-gx-core-010) and the
/// same reasoning applies to
/// every raw-byte field: a signature is bytes, and bytes have a major type of their own.
/// [`crate::Cid`] hand-writes its impls for this reason; this module is the same fix where a
/// derive is otherwise wanted.
///
/// The human-readable side is base64 (**M2H1-4**, `req/38_ERRATA_2026-08-07.md` §9: "default
/// recommendation = base64"; sem: SEM-gx-core-011). Hand 1 left it as a byte string in both faces,
/// which serde_json writes as a list of
/// sixty-four integers -- a spelling nothing in the canon asks for, next to a `payload` 44 §2.2
/// already spells in base64. Hand 5 settles it the way the ruling recommended; the table is
/// [`crate::b64`] and `gx-core/tests/base64_vectors.rs` checks it against RFC 4648 §10.
///
/// The reading side accepts all three spellings, because two of them are already on disk or in a
/// test fixture somewhere by the time this changes: a byte string (what DAG-CBOR carries), a
/// base64 string (what JSON now carries), and a sequence of integers (what JSON carried before).
/// Accepting the old form is not repair -- each of the three names exactly one byte string, so the
/// map back is still one-to-one, which is the property [`crate::Cid::from_text`]'s strictness
/// exists to protect.
mod raw_bytes {
    use core::fmt;
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_str(&crate::b64::encode(v))
        } else {
            s.serialize_bytes(v)
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        struct Bytes;

        impl<'v> Visitor<'v> for Bytes {
            type Value = Vec<u8>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("raw bytes, base64 (RFC 4648 §4) in a human-readable format")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Vec<u8>, E> {
                crate::b64::decode(v).map_err(E::custom)
            }

            fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Vec<u8>, E> {
                Ok(v.to_vec())
            }

            fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Vec<u8>, E> {
                Ok(v)
            }

            fn visit_seq<A: SeqAccess<'v>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
                let mut out = Vec::with_capacity(seq.size_hint().unwrap_or_default());
                while let Some(b) = seq.next_element::<u8>()? {
                    out.push(b);
                }
                Ok(out)
            }
        }

        if d.is_human_readable() {
            d.deserialize_any(Bytes)
        } else {
            d.deserialize_bytes(Bytes)
        }
    }
}
