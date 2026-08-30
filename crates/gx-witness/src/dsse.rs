// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The DSSE envelope, its pre-authentication encoding, and the two things gx signs. (sem:
//! SEM-gx-witness-038, SEM-gx-witness-039, SEM-gx-witness-040, SEM-gx-witness-041,
//! SEM-gx-witness-042, SEM-gx-witness-043, SEM-gx-witness-044, SEM-gx-witness-045,
//! SEM-gx-witness-046, SEM-gx-witness-047, SEM-gx-witness-048, SEM-gx-witness-049,
//! SEM-gx-witness-050, SEM-gx-witness-051, SEM-gx-witness-052, SEM-gx-witness-053,
//! SEM-gx-witness-054, SEM-gx-witness-055, SEM-gx-witness-056, SEM-gx-witness-057,
//! SEM-gx-witness-058, SEM-gx-witness-059)
//!
//! Spec: 42 §3.10 for the envelope's three fields and for the sentence that sends the signed bytes
//! to the DSSE standard, 42 §1.3-4 for what a signature is *outside* of, 44 §2.2 for the JSON face,
//! 32 FR-018/FR-019 for the requirement, 34 AC-018/AC-019 for how it is judged.
//!
//! # The pre-authentication encoding is written here, from the standard's own words
//!
//! 42 §3.10, verbatim: "the DSSE signing target (PAE: Pre-Authentication Encoding) uses the default
//! form that concatenates `payload_type` and `payload`, following the DSSE standard (compatible
//! with the in-toto/Sigstore standard implementations, research/02)". That
//! sentence names the algorithm and does not state it, so the byte formula below is **derived from
//! the DSSE specification** (`secure-systems-lab/dsse`, `protocol.md`) rather than transcribed from
//! a gx canonical source -- raised as a ticket in req/54 §4 rather than settled here. It is a
//! specification, in
//! the same standing as RFC 6962 §2.1 in `gx-log/src/tile.rs` and RFC 4648 in [`gx_core::Cid`]: no
//! line of any implementation was read or copied (52 / 05 §4 R-4), and req/49 §3 M2-17 already
//! recorded "the implementation source of DSSE PAE is ... in-house authoring is the default line".
//!
//! ```text
//! PAE(type, body) = "DSSEv1" ‖ SP ‖ LEN(type) ‖ SP ‖ type ‖ SP ‖ LEN(body) ‖ SP ‖ body
//! SP              = one 0x20 byte
//! LEN(s)          = the byte length of s, in ASCII decimal, without leading zeros
//! ```
//!
//! # Why a plain concatenation would not do
//!
//! The lengths are the whole point, and they are why 42 §3.10's own wording -- "the concatenated
//! default form" --
//! is not enough on its own. `payload_type` and `payload` concatenated without them are ambiguous:
//! a type ending in one byte and a payload beginning one byte later produce the same buffer as a
//! type one byte longer, so one signature would authenticate two different (type, payload) pairs.
//! The length prefixes are what make the parse unique, and `tests/pae_golden.rs` pins the bytes.
//!
//! # What gx signs: two payloads, one road (**E-M2-26**)
//!
//! Two things carry a [`gx_core::DsseSignature`] in M2, and since E-M2-26 both are signed the same
//! way -- `PAE(payload_type, body)`, with the type telling the two apart:
//!
//! * a [`crate::receipt::Receipt`]: type [`RECEIPT_PAYLOAD_TYPE`], body the envelope's `payload`
//!   (42 §3.10);
//! * a [`gx_core::Checkpoint`]: type [`CHECKPOINT_PAYLOAD_TYPE`], body
//!   `gx_log::proof::checkpoint_signing_bytes` -- the canonical form of
//!   `{origin, tree_size, root_hash}` that E-M2-19 fixes as the signed core.
//!   [`checkpoint_signing_message`] is the message.
//!
//! Hand 5 signed the checkpoint core **directly**, because 42 §3.11 gives a checkpoint no
//! `payload_type` to put in a PAE, and recorded the asymmetry as H5-4 rather than resolving it. The
//! ruling that resolved it is `req/38_ERRATA_2026-08-07.md` §15, verbatim: "**E-M2-26** (H5-4's
//! settlement, into the fix batch): the checkpoint signature **is given a payload_type and put on
//! the PAE** (a 42 §3.11 erratum). Grounds: ① unify the signing discipline into DSSE alone (the
//! current state, where one key signs two byte formats directly, is 'accidental non-collision', not
//! a design = H5-4)". The type is minted here as that erratum: 42 §3.11's field table has no row for it.
//!
//! What changes is what one key can be talked into signing. Before, the separation between the two
//! roads was that a PAE opens with `DSSEv1` and a canonical CBOR map opens with a major-type-5
//! header -- true of today's two encodings and chosen by nobody. Now it is the length-prefixed
//! `payload_type` inside the signed bytes, which is the property the DSSE encoding exists to give.
//!
//! # What gx does **not** claim: C2SP checkpoint wire compatibility
//!
//! The C2SP `tlog-checkpoint` / `signed-note` form is a **text** format -- an origin line, a decimal
//! tree size, a base64 root, a blank line, then `— <name> <base64 signature>` -- and a signature over
//! that text. gx's checkpoint is canonical DAG-CBOR inside a DSSE pre-authentication encoding.
//! `req/38_ERRATA_2026-08-07.md` §15, verbatim: "② the C2SP text form (newline+signed-note) is
//! confirmed structurally unrelated by primary-source cross-check -- gx does not claim wire
//! compatibility with the C2SP checkpoint (if it did, a conversion layer would be future work), and
//! **states so explicitly in the doc**". So: **a C2SP witness cannot verify a gx checkpoint and gx
//! cannot verify a C2SP one.** The
//! two agree on what a signed tree head is *for* and on nothing about its bytes. Making them
//! interoperate means a conversion layer, and F-5 (`req/38_ERRATA_2026-08-07.md` §17) has since
//! given it an address: **M8, the conformance window**, beside the `tlog-cosignature` v1 clash with
//! CM-5.
//!
//! **Who the incompatible party is, by name.** "not compatible" is worth nothing as an abstraction, so
//! here is the deployment it is about: **Rekor v2 (`sigstore/rekor-tiles`)**, whose `CLIENTS.md`
//! states "The checkpoints the log provides will conform to the C2SP checkpoint spec" and computes
//! its key IDs "per the signed note spec". Hand 7 fetched that file from the repository itself
//! rather than taking it from a transcription (`Desktop/GitRepo/REFERENCES.md`, 2026-08-08). Three
//! measured differences, not one: the note body is text where gx's is canonical DAG-CBOR; the
//! signature is over that text where gx's is over a DSSE pre-authentication encoding; and the tree
//! is hashed with SHA-256 (`tlog-tiles.md`: "The hashing algorithm is defined to be SHA-256")
//! where gx uses BLAKE3 (35 DR-3). The third is already asserted as a declared difference by
//! `gx-log/tests/ac_024.rs`; the first two by
//! `the_checkpoint_message_is_not_a_c2sp_signed_note` in `tests/checkpoint_signature.rs`.
//! `the_checkpoint_message_is_not_a_c2sp_signed_note` in `tests/checkpoint_signature.rs` is the
//! mechanical form of the non-claim, in the shape AC-024 uses for its declared difference: the
//! absence is asserted, so a change that made the two look alike would fail a test rather than pass
//! quietly.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use gx_core::{Checkpoint, DsseSignature, KeyId, VerdictCheckpoint};
use serde::{Deserialize, Serialize};

use crate::keys::VerifyingKeyRef;
use crate::{Error, Result};

/// 42 §3.10, fixed value: "`application/vnd.glovrex.receipt+dagcbor`".
///
/// A constant rather than a literal at each use, because it is inside the signed bytes: a caller
/// that spelled it differently would produce a signature no verifier here accepts, and the failure
/// would look like a bad key.
pub const RECEIPT_PAYLOAD_TYPE: &str = "application/vnd.glovrex.receipt+dagcbor";

/// The payload type of a checkpoint's signed core (**E-M2-26**).
///
/// 42 §3.11's `Checkpoint` table has no `payload_type` row -- the field does not exist there and no
/// canonical source names this string. It is minted by the E-M2-26 erratum
/// (`req/38_ERRATA_2026-08-07.md` §15),
/// spelled after 42 §3.10's receipt type so that the two differ in one word and agree everywhere
/// else, and it is load-bearing rather than decorative: it is inside the signed bytes, so it is what
/// stops a signature over a checkpoint from being read as a signature over a receipt.
///
/// A checkpoint from a producer that spelled it differently produces a signature no verifier here
/// accepts -- the same failure mode, and the same reason for the constant, as
/// [`RECEIPT_PAYLOAD_TYPE`].
pub const CHECKPOINT_PAYLOAD_TYPE: &str = "application/vnd.glovrex.checkpoint+dagcbor";

/// The payload type of a key revocation's signed core (**FR-M7-3**, M7 hand 2).
///
/// The third string of this shape and it is minted here for the same reason the second one was: 42
/// names no revocation record at all (ASM-45-2 puts the list on the verifier's side and describes
/// nothing about its form), and the type has to be **inside the signed bytes** so that a signature
/// over a revocation cannot be replayed as a signature over a receipt or a checkpoint. Spelled after
/// the other two so the three differ in one word.
///
/// A revocation whose producer spelled it differently produces a signature no verifier here accepts,
/// which is the failure mode this constant exists to make impossible for gx's own producer.
pub const REVOCATION_PAYLOAD_TYPE: &str = "application/vnd.glovrex.revocation+dagcbor";

/// The payload type of a claim-standing close-statement's signed core (DR-46-40, `req/730`).
///
/// Same reasoning as [`REVOCATION_PAYLOAD_TYPE`], one type over: a close-statement has to be
/// inside the signed bytes and distinct from every other payload type this workspace signs, so a
/// signature over one cannot be replayed as a signature over a receipt, a checkpoint, or a
/// revocation.
pub const STANDING_PAYLOAD_TYPE: &str = "application/vnd.glovrex.standing+dagcbor";

/// The payload type of an aggregate verdict checkpoint's signed core (**FR-M04**, M7 hand 6).
///
/// The fourth string of this shape, minted for the reason the second and third were: 42 names no
/// aggregate count at all, and the type has to be **inside the signed bytes** so that a signature
/// over a count of refusals cannot be replayed as a signature over a tree head, a receipt or a
/// revocation. Spelled after the other three so the four differ in one word.
///
/// Why it matters more here than for the other three: this one is signed by a party who has an
/// interest in its contents being smaller. Everything that keeps the four loads apart is doing
/// work in the presence of that interest, and `crates/gx-engine/tests/ac_vc.rs`'s AC-VC-4 measures
/// it in both directions rather than in the one that is easy to write.
pub const VERDICT_CHECKPOINT_PAYLOAD_TYPE: &str =
    "application/vnd.glovrex.verdict-checkpoint+dagcbor";

/// The six bytes that open every gx pre-authentication encoding.
const PAE_PREFIX: &[u8] = b"DSSEv1";

/// The DSSE pre-authentication encoding (42 §3.10; formula in this module's header).
///
/// `payload_type` is taken as `&str` because the encoding covers its UTF-8 bytes and a `String`
/// is the only thing 42 §3.10 puts in the field; `payload` is `&[u8]` because it is the canonical
/// DAG-CBOR of a [`crate::receipt::ReceiptPayload`] and is never decoded on this road. That is the
/// property AC-019 rests on: a bit flipped anywhere inside the payload changes the signed bytes
/// without changing whether they parse, so it is caught as a bad signature rather than as a
/// malformed value.
///
/// Infallible: any string and any byte string have an encoding.
#[must_use]
pub fn pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let type_bytes = payload_type.as_bytes();
    let type_len = type_bytes.len().to_string();
    let body_len = payload.len().to_string();

    let mut out = Vec::with_capacity(
        PAE_PREFIX.len() + type_len.len() + type_bytes.len() + body_len.len() + payload.len() + 4,
    );
    out.extend_from_slice(PAE_PREFIX);
    out.push(b' ');
    out.extend_from_slice(type_len.as_bytes());
    out.push(b' ');
    out.extend_from_slice(type_bytes);
    out.push(b' ');
    out.extend_from_slice(body_len.as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload);
    out
}

/// A DSSE envelope: a typed payload and the signatures over it (42 §3.10).
///
/// Three fields, as 42 §3.10's table has them. `payload` is the canonical DAG-CBOR of a
/// [`crate::receipt::ReceiptPayload`] and is kept as bytes rather than as the value: a verifier has
/// to sign and check *the bytes that travelled*, and a struct that held the decoded value would
/// re-encode it to get them -- which works only for as long as this workspace's encoder and the
/// sender's agree byte for byte. Keeping the bytes makes that agreement unnecessary.
///
/// # The JSON face
///
/// 44 §2.2: "`Receipt` (the JSON representation of 42 §3.10; `payload` is base64)". `payload`
/// therefore serialises as
/// a base64 string in a human-readable format and as a byte string in DAG-CBOR, which is the same
/// pair `DsseSignature.sig` takes under M2H1-4 (`req/38_ERRATA_2026-08-07.md` §9) and the same
/// table, [`gx_core`]'s.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DsseEnvelope {
    /// 42 §3.10 fixed value [`RECEIPT_PAYLOAD_TYPE`] for a receipt. Carried rather than assumed, because
    /// it is inside the signed bytes and a verifier that assumed it could not detect a change to it.
    pub payload_type: String,
    /// `canonical_dagcbor(ReceiptPayload)` (42 §3.10).
    #[serde(with = "raw_bytes")]
    pub payload: Vec<u8>,
    /// The signatures over `pae(payload_type, payload)`. A `Vec` because 42 §3.10 types it so:
    /// one envelope may be signed under several keys, and [`DsseEnvelope::signature_for`] is how
    /// a verifier picks the one offered under its key.
    pub signatures: Vec<DsseSignature>,
}

impl DsseEnvelope {
    /// The bytes a signature over this envelope covers.
    #[must_use]
    pub fn signing_bytes(&self) -> Vec<u8> {
        pae(&self.payload_type, &self.payload)
    }

    /// The signature offered under `key_id`, if there is one.
    ///
    /// 42 §3.10 types `signatures` as a `Vec`, so an envelope may carry several; a verifier holds
    /// one key and asks for that key's signature rather than trying every signature against it.
    /// The difference matters for AC-019: a flipped bit in a `keyid` makes this return `None`,
    /// which is a *failure to verify under the key the caller named* and not a different question.
    #[must_use]
    pub fn signature_for(&self, key_id: &str) -> Option<&DsseSignature> {
        self.signatures.iter().find(|s| s.keyid == key_id)
    }

    /// Sign these bytes with `key` and add the signature (FR-019).
    ///
    /// Additive: an envelope already carrying another key's signature keeps it, which is what a
    /// `Vec<DsseSignature>` is for. A second signature under the *same* key id replaces nothing and
    /// is not prevented here -- [`DsseEnvelope::verify`] takes the first match, and a producer that signs twice
    /// under one id has a bug this type cannot see.
    pub fn sign(&mut self, key: &SigningKey, key_id: &KeyId) {
        let signature: Signature = key.sign(&self.signing_bytes());
        self.signatures.push(DsseSignature {
            keyid: key_id.clone(),
            sig: signature.to_bytes().to_vec(),
        });
    }

    /// Check the signature offered under `key`'s id (FR-019, AC-019).
    ///
    /// # Every refusal is [`Error::SignatureInvalid`]
    ///
    /// AC-019, verbatim, asks that a receipt with one bit flipped verify to `Err(SignatureInvalid)`, and
    /// a bit lands in one of four places on this road: the payload, the payload type, the signature
    /// bytes, or the `keyid`. The first two change the pre-authentication encoding, the third
    /// changes the signature, and the fourth changes which signature is asked for -- and all four
    /// are "this key did not sign this". Reporting them apart would tell a forger which part of the
    /// forgery to fix, which is the same argument `gx_log::proof::verify_inclusion` makes for
    /// answering a bad proof with one `false`. A signature of the wrong length is refused here too,
    /// for the reason [`gx_core::DsseSignature`]'s own documentation gives.
    ///
    /// # Errors
    /// [`Error::SignatureInvalid`], always -- see above.
    pub fn verify(&self, key: &VerifyingKeyRef<'_>) -> Result<()> {
        let offered = self
            .signature_for(key.key_id)
            .ok_or_else(|| Error::SignatureInvalid {
                key_id: key.key_id.to_string(),
            })?;
        verify_raw(key, &self.signing_bytes(), offered)
    }
}

/// The bytes a checkpoint signature is computed over (**E-M2-26**).
///
/// `PAE(`[`CHECKPOINT_PAYLOAD_TYPE`]`, gx_log::proof::checkpoint_signing_bytes(checkpoint))`. The
/// inner byte string is E-M2-19's signed core -- `{origin, tree_size, root_hash}`, with `timestamp`
/// and `signature` outside it (CM-5) -- and this is the message that core travels inside.
///
/// Public because a verifier needs it. A third party checking a published head has the checkpoint
/// and the key and nothing else; asking it to rebuild the message from a formula in a doc comment is
/// how two implementations end up checking two different things.
///
/// # Errors
/// [`Error::Log`] if the core has no canonical form.
pub fn checkpoint_signing_message(checkpoint: &Checkpoint) -> Result<Vec<u8>> {
    let core = gx_log::proof::checkpoint_signing_bytes(checkpoint)?;
    Ok(pae(CHECKPOINT_PAYLOAD_TYPE, &core))
}

/// Sign a checkpoint and return it with the signature attached (**H3-7**, **E-M2-26**).
///
/// `req/38_ERRATA_2026-08-07.md` §12: "H3-7 (`Checkpoint.signature` being non-Option) -> resolved
/// in hand 5: attaching the signature is hand 5's scope", and §9's E-M2-19 fixes the signed core as
/// `{origin, tree_size,
/// root_hash}` with `timestamp` outside it (CM-5). `gx_log::proof::checkpoint_signing_bytes` is
/// that byte string; hand 2 built it and declined to sign it, and this is the caller it named.
///
/// What is signed is [`checkpoint_signing_message`] rather than the core directly. Hand 5 signed the
/// core, and E-M2-26 (§15) ruled that off: one key signing two byte formats with nothing between
/// them saying which is which is a separation that holds by accident of two encodings.
///
/// The input is expected to be a `gx_log::proof::unsigned_checkpoint`, whose `signature` is the
/// empty placeholder that type has to carry because 42 §3.11 makes the field non-optional. What
/// comes back is the same head with a real one, so `Checkpoint.signature` finally means what its
/// name says and H3-7 closes.
///
/// # Errors
/// [`Error::Log`] if the core has no canonical form.
pub fn sign_checkpoint(
    checkpoint: &Checkpoint,
    key: &SigningKey,
    key_id: &KeyId,
) -> Result<Checkpoint> {
    let message = checkpoint_signing_message(checkpoint)?;
    let signature: Signature = key.sign(&message);
    Ok(Checkpoint {
        signature: DsseSignature {
            keyid: key_id.clone(),
            sig: signature.to_bytes().to_vec(),
        },
        ..checkpoint.clone()
    })
}

/// Check a checkpoint's signature over the message E-M2-26 fixes.
///
/// The `timestamp` is **not** covered, and a caller that treats a verified checkpoint as evidence
/// of when the head was published is reading something this function did not check (CM-5). What it
/// does say is that the named key stated this root at this size for this origin.
///
/// A signature taken over the bare core -- the pre-E-M2-26 road -- is refused here, and
/// `tests/checkpoint_signature.rs` measures that rather than leaving it implied.
///
/// # Errors
/// [`Error::SignatureInvalid`] as [`DsseEnvelope::verify`] does, [`Error::Log`] if the core has no
/// canonical form.
pub fn verify_checkpoint(checkpoint: &Checkpoint, key: &VerifyingKeyRef<'_>) -> Result<()> {
    if checkpoint.signature.keyid != key.key_id {
        return Err(Error::SignatureInvalid {
            key_id: key.key_id.to_string(),
        });
    }
    let message = checkpoint_signing_message(checkpoint)?;
    verify_raw(key, &message, &checkpoint.signature)
}

/// The spelling both `sdk/wasm-verify` and `crates/gx-cli` embed when they report
/// [`verify_checkpoint`]'s `Err` to a caller (`format!("{CHECKPOINT_REFUSAL_PREFIX}: {e}")`).
///
/// # Why this exists (B1 / DIV-901-1, `req/914` §7, `req/874` R3)
///
/// `web_verify/src/verdict.js::CHECKPOINT_REFUTED` matches this exact prefix, anchored to the start
/// of the string, to read "the checkpoint refused its own signature check" as `deny` — one of R3's
/// four `deny` triggers. Before this constant existed, `sdk/wasm-verify/src/lib.rs` and
/// `crates/gx-cli/src/receipt.rs` each hand-wrote their own English for the same
/// [`Error::SignatureInvalid`] and drifted apart: the CLI's spelling never matched the classifier's
/// regex, so the CLI side fell through to the classifier's default `hold` instead of `deny`
/// (`tools/verify_parity.mjs`'s `DIV-901-1`). A single `pub const`, imported by both callers (both
/// already carry `gx-witness` as a `path` dependency), makes that drift structurally impossible
/// rather than a matter of two authors remembering to copy the same sentence. Additive only —
/// [`verify_checkpoint`]'s behavior is unchanged.
pub const CHECKPOINT_REFUSAL_PREFIX: &str =
    "checkpoint_key: the checkpoint did not verify under it";

// ---------------------------------------------------------------------------
// FR-M04 -- the aggregate verdict checkpoint's three functions (M7 hand 6)
// ---------------------------------------------------------------------------

/// The bytes an aggregate verdict checkpoint's signature is computed over (**FR-M04**).
///
/// `PAE(`[`VERDICT_CHECKPOINT_PAYLOAD_TYPE`]`, gx_log::proof::verdict_checkpoint_signing_bytes(cp))`.
/// The inner byte string is the core `{tally, origin, window_end, window_start, ledger_root_hash,
/// ledger_tree_size}` with `timestamp` and `signature` outside it (CM-5, as for the tree head).
///
/// Public for [`checkpoint_signing_message`]'s reason: a third party holding a published count and
/// a key must be able to rebuild the message from a function rather than from a doc comment.
///
/// # Errors
/// [`Error::Log`] if the core has no canonical form.
pub fn verdict_checkpoint_signing_message(checkpoint: &VerdictCheckpoint) -> Result<Vec<u8>> {
    let core = gx_log::proof::verdict_checkpoint_signing_bytes(checkpoint)?;
    Ok(pae(VERDICT_CHECKPOINT_PAYLOAD_TYPE, &core))
}

/// Sign an aggregate verdict checkpoint and return it with the signature attached (**FR-M04**).
///
/// The input is expected to be a `gx_log::proof::unsigned_verdict_checkpoint`, whose `signature` is
/// the empty placeholder that type carries because the field is not an `Option` — the same shape
/// [`sign_checkpoint`] takes for the tree head.
///
/// 🔴 **What signing this does not establish.** The key that signs is the deployment's own, and the
/// deployment is the party the count is evidence *against*. So a valid signature here says "this
/// key stated these numbers" and nothing about whether the numbers are true; the check that can
/// contradict them is `gx_log::proof::audit_verdict_chain`, which takes what a verifier counted for
/// itself. Kept in two functions on purpose: one road that a key can satisfy, one road that it
/// cannot.
///
/// # Errors
/// [`Error::Log`] if the core has no canonical form.
pub fn sign_verdict_checkpoint(
    checkpoint: &VerdictCheckpoint,
    key: &SigningKey,
    key_id: &KeyId,
) -> Result<VerdictCheckpoint> {
    let message = verdict_checkpoint_signing_message(checkpoint)?;
    let signature: Signature = key.sign(&message);
    Ok(VerdictCheckpoint {
        signature: DsseSignature {
            keyid: key_id.clone(),
            sig: signature.to_bytes().to_vec(),
        },
        ..checkpoint.clone()
    })
}

/// Check an aggregate verdict checkpoint's signature (**FR-M04**).
///
/// The `timestamp` is **not** covered (CM-5), so a caller treating a verified checkpoint as
/// evidence of *when* the counts were published is reading something this function did not check.
/// What it does say is that the named key stated this tally over this window for this origin,
/// beside this ledger head.
///
/// # Errors
/// [`Error::SignatureInvalid`] as [`verify_checkpoint`] does, [`Error::Log`] if the core has no
/// canonical form.
pub fn verify_verdict_checkpoint(
    checkpoint: &VerdictCheckpoint,
    key: &VerifyingKeyRef<'_>,
) -> Result<()> {
    if checkpoint.signature.keyid != key.key_id {
        return Err(Error::SignatureInvalid {
            key_id: key.key_id.to_string(),
        });
    }
    let message = verdict_checkpoint_signing_message(checkpoint)?;
    verify_raw(key, &message, &checkpoint.signature)
}

/// The one curve operation in this crate, and the one place a signature becomes a verdict.
///
/// `verify_strict` rather than `verify`: ed25519-dalek's strict form refuses small-order public
/// keys and non-canonical `s` scalars, which are the shapes under which one signature can verify
/// under two keys. gx uses a signature to say "*this* key attested this", so a check that admits a
/// second answer is the wrong check.
fn verify_raw(key: &VerifyingKeyRef<'_>, message: &[u8], offered: &DsseSignature) -> Result<()> {
    let invalid = || Error::SignatureInvalid {
        key_id: key.key_id.to_string(),
    };
    let signature = Signature::from_slice(&offered.sig).map_err(|_| invalid())?;
    let public: &VerifyingKey = key.key;
    public
        .verify_strict(message, &signature)
        .map_err(|_| invalid())
}

/// The envelope payload's two faces: base64 in JSON (44 §2.2), a byte string in DAG-CBOR.
///
/// The same pair, and the same table, as [`gx_core::DsseSignature`]'s `sig` (M2H1-4). It is written
/// out here rather than derived because serde writes a `Vec<u8>` as a sequence of integers by
/// default, which is neither of the two spellings the canon names.
pub(crate) mod raw_bytes {
    use core::fmt;
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_str(&gx_core::b64::encode(v))
        } else {
            s.serialize_bytes(v)
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        struct Bytes;

        impl<'v> Visitor<'v> for Bytes {
            type Value = Vec<u8>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a payload: base64 (44 §2.2) in JSON, a byte string in DAG-CBOR")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Vec<u8>, E> {
                gx_core::b64::decode(v).map_err(E::custom)
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

// ---------------------------------------------------------------------------
// M5H8-9 — the three roads into `raw_bytes`, each pinned to its own visitor method
// ---------------------------------------------------------------------------

/// 🔴 **M5H8-9, adopted (a)** (`req/38_ERRATA_2026-08-07.md` §45), verbatim:
///
/// > **M5H8-9, adopted (a)**: a probe that directly exercises dsse `raw_bytes`'s Visitor 3 roads
/// > (visit_str/visit_byte_buf/visit_seq) = **the fix batch**.
///
/// # What was measured, and why the probes are here rather than in `tests/`
///
/// req/86 §3.1 ran `cargo-mutants` over all 71 gx-witness mutants and found **11 survivors, 10 of
/// them in this one module**: `expecting` reduced to a no-op, and `visit_str` / `visit_byte_buf` /
/// `visit_seq` each reduced to `Ok(vec![])`, `Ok(vec![0])` and `Ok(vec![1])`. The reason is stated
/// there and is worth repeating: a byte string reaches this visitor by three different roads
/// depending on the format, and every existing probe in the crate travels **one** of them
/// (DAG-CBOR's `visit_bytes`, which is why that method has no survivor). Potholes in the other two
/// are invisible.
///
/// [`raw_bytes`] is `pub(crate)`, so an integration test cannot name it and the road it would have
/// to travel instead — a whole [`DsseEnvelope`] through a real codec — is the road that already
/// exists and already only reaches `visit_bytes`. Unit tests beside the module are the only place
/// the *method* can be chosen, which is what "a decode with the road pinned" means. The pattern is the
/// workspace's: `gx-log/src/proof.rs`, `gx-log/src/tile.rs`, `gx-core/src/error.rs` and four others
/// carry `#[cfg(test)] mod tests` for the same reason.
///
/// The one method with no probe below is `visit_bytes`, deliberately: it is covered by every
/// receipt round trip in `tests/`, and `cargo-mutants` measured that (it is the one method of the
/// four with no survivor). Adding a fourth probe would record coverage that already exists.
#[cfg(test)]
mod tests {
    use super::raw_bytes;
    use serde::de::value::{Error as ValueError, SeqDeserializer};
    use serde::de::Visitor;
    use serde::Deserializer;

    /// The payload every road below carries. Four bytes, none of them `0` or `1` alone, because the
    /// mutants this file exists to kill return `vec![]`, `vec![0]` and `vec![1]` — a fixture that
    /// happened to be one of those would let one of the three through.
    const PAYLOAD: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

    /// Which `Visitor` method the decode is pinned to. There is no dispatch and no format: the
    /// point of this deserializer is that the road is chosen by the test rather than by a codec.
    enum Road {
        Str(String),
        ByteBuf(Vec<u8>),
        Seq(Vec<u8>),
        /// A type the visitor does not accept, so that serde builds its own message out of
        /// `Visitor::expecting`. This is the road that holds the fourth method.
        Unacceptable,
    }

    struct Fixed(Road);

    impl<'de> Deserializer<'de> for Fixed {
        type Error = ValueError;

        fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, ValueError> {
            match self.0 {
                Road::Str(s) => visitor.visit_str(&s),
                Road::ByteBuf(b) => visitor.visit_byte_buf(b),
                Road::Seq(b) => {
                    SeqDeserializer::<_, ValueError>::new(b.into_iter()).deserialize_any(visitor)
                }
                Road::Unacceptable => visitor.visit_bool(true),
            }
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
            option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
            identifier ignored_any
        }
    }

    fn decode(road: Road) -> Result<Vec<u8>, ValueError> {
        raw_bytes::deserialize(Fixed(road))
    }

    /// JSON's road (44 §2.2: "`payload` is base64"). The bytes come back, and they are *these* bytes.
    #[test]
    fn visit_str_decodes_the_base64_it_was_given() {
        let text = gx_core::b64::encode(&PAYLOAD);
        let got = decode(Road::Str(text.clone())).expect("base64 decodes");
        println!("VISIT_STR_IN={text} OUT={got:02x?}");
        assert_eq!(
            got,
            PAYLOAD.to_vec(),
            "the JSON road returned something other than the payload it was handed"
        );

        // The negative control: this road must be able to fail, or `Ok(vec![])` would be
        // indistinguishable from a road that never looked at its input.
        let refused = decode(Road::Str("not base64!!".to_string()));
        assert!(refused.is_err(), "invalid base64 decoded to {refused:?}");
    }

    /// The owned-bytes road. Reached by any decoder that hands the visitor a `Vec<u8>` it already
    /// allocated — no codec in this workspace does, which is exactly why the mutants survived.
    #[test]
    fn visit_byte_buf_returns_the_buffer_unchanged() {
        let got = decode(Road::ByteBuf(PAYLOAD.to_vec())).expect("a byte buffer is a payload");
        println!("VISIT_BYTE_BUF_OUT={got:02x?}");
        assert_eq!(
            got,
            PAYLOAD.to_vec(),
            "the owned-bytes road did not return its buffer"
        );
    }

    /// The sequence road: a payload spelled as a list of integers, which is what serde writes for a
    /// `Vec<u8>` when nothing overrides it (this module's header says so) and what a JSON document
    /// produced by such a writer looks like coming back.
    #[test]
    fn visit_seq_collects_every_element_in_order() {
        let got = decode(Road::Seq(PAYLOAD.to_vec())).expect("a sequence of bytes is a payload");
        println!("VISIT_SEQ_OUT={got:02x?}");
        assert_eq!(
            got,
            PAYLOAD.to_vec(),
            "the sequence road dropped or reordered elements"
        );

        // Length is part of the claim: a `visit_seq` that stopped after one element would still
        // return "real bytes" by the loose reading.
        let long: Vec<u8> = (0u8..=255).collect();
        assert_eq!(
            decode(Road::Seq(long.clone())).expect("long sequence"),
            long
        );
    }

    /// The fourth method. `expecting` is only ever read when a decode fails, so the probe that holds
    /// it is a probe about an error message — and the message is the whole of what a caller handed a
    /// malformed document is told.
    #[test]
    fn expecting_names_both_spellings_the_canon_allows() {
        let refused = decode(Road::Unacceptable).expect_err("a bool is not a payload");
        let text = refused.to_string();
        println!("EXPECTING_MESSAGE={text}");
        assert!(
            text.contains("base64 (44 §2.2)") && text.contains("byte string in DAG-CBOR"),
            "the type error must name the two spellings 42 §3.10 and 44 §2.2 allow; got {text:?}"
        );
    }
}
