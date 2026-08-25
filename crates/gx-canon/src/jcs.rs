// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The JSON compatibility route: RFC 8785 canonicalisation (42 §2.2).
//!
//! Spec: FR-010 (a SHOULD, for in-toto / SCITT / Sigstore interoperability), 42 §2.2 for the
//! contract, 34 AC-010 for the acceptance criterion, T-25 for the implementation crate.
//!
//! # This is a second route, not a second identity
//!
//! 42 §2.2 is explicit: the digest of this route would be SHA-256, would be typed `JcsDigest`
//! rather than `Cid`, and "`JcsDigest` does not constitute identity" (sem: SEM-gx-canon-020). DR-3 keeps BLAKE3 + DAG-CBOR as
//! the primary and this as a compatibility layer. Two consequences follow, and both are visible
//! in the code: everything the struct has goes through here -- 42 §2.2 asks for "all fields
//! of the struct, not limited to `IdentityView`" (sem: SEM-gx-canon-021), which is the opposite of what [`crate::cid::compute`]
//! does -- and no digest is taken at all. B-08 (req/10 §6) keeps `JcsDigest` out of M1 because
//! none of the seventeen M1 criteria asks for it, and 52 contract 2 forbids the convenience.
//!
//! # Why the value passes through `serde_json::Value` first
//!
//! `Value` is a normalised tree, so what the canonicaliser sorts is a document rather than a
//! serialisation order. Routing through it also fixes *one* JSON reading of every type: whatever
//! `serde_json` would have produced is what gets canonicalised, and no second serialiser gets a
//! say.
//!
//! # 42 §1.2 and this route: settled by E-JCS-1
//!
//! 42 §1.2 says the readable form "CLI/API/log human-readable display and JSON embedding all
//! take this form as canonical" (sem: SEM-gx-canon-022) -- including JSON embedding. Until E-JCS-1 this module could not honour that: `Cid`
//! serialised unconditionally with `serialize_bytes` (42 §1.1 wants a byte string in the binary
//! form), JSON has no byte type, and `serde_json` therefore wrote an array of numbers. req/31 §11
//! had settled that only gx-canon may mint `gx1:`, so gx-core could not spell it and the gap
//! stood open; step 4 filed it as an erratum candidate rather than papering over it.
//!
//! E-JCS-1 (`req/38_ERRATA_2026-08-07.md` §5) ruled it the first way: `gx_core::Cid`'s `Serialize`
//! now branches on `is_human_readable()`, writing `gx1:<base32>` for a human-readable format and
//! the 32 raw bytes for a binary one. `serde_json` is human-readable, so what comes out of
//! [`encode`] spells a `Cid` the way 42 §1.2 asks -- `{"digest":"gx1:...","id":"gx1:...", ...}`
//! -- while [`crate::cbor`] is unaffected. The spelling itself lives in
//! [`gx_core::Cid::to_text`], which keeps it a single implementation (`tests/cid_text.rs` checks
//! that mechanically), and `tests/ac_010.rs` still measures determinism, which held either way.
//!
//! `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §2 B-3 found this section still describing the
//! pre-erratum behaviour, with the `gx1:` strings measurable at the time it was read: a doc older
//! than its implementation, which is how a reader gets sent to fix a problem that is not there.
//! F-3 (`req/46D_AUDIT_RULING_2026-08-07.md` §1) is the correction above.

use crate::{Error, Result};
use serde::Serialize;

/// Canonicalise a value to RFC 8785 JSON bytes.
///
/// Deterministic: object keys sorted, no insignificant whitespace, one spelling per number. The
/// same logical value gives the same bytes however it was spelled on the way in, which is what
/// AC-010 measures by running it three times and what `tests/ac_010.rs` also measures by feeding
/// it a differently formatted document.
///
/// # Errors
/// [`Error::Jcs`] when the value has no JSON form (a non-finite float, a map with non-string
/// keys) or when canonicalisation refuses it. As on the CBOR side, the answer to an input outside
/// the supported range is a refusal and not a guess (`req/26 §3`).
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let document = serde_json::to_value(value).map_err(|e| Error::Jcs {
        detail: e.to_string(),
    })?;
    serde_json_canonicalizer::to_vec(&document).map_err(|e| Error::Jcs {
        detail: e.to_string(),
    })
}
