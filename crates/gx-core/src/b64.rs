//! The readable spelling of a raw byte string (**M2H1-4**).
//!
//! `req/38_ERRATA_2026-08-07.md` §9 left one question to hand 5: 「raw bytes(sig/FingerprintBytes)
//! の JSON 綴りは**手 5(receipt の JSON 面)で決める**。既定推奨=base64(42 §1.2 Cid・44 §2.2
//! payload と同系)」. Hand 5 takes the recommendation. 44 §2.2 already spells one raw byte string
//! in base64 -- 「`Receipt`（42 §3.10のJSON表現、`payload`はbase64）」 -- and a workspace where the
//! payload is base64 while the signature over it is a list of sixty-four integers would have two
//! answers to one question. The DAG-CBOR face is untouched and stays a byte string (major type 2),
//! which `req/38_ERRATA_2026-08-07.md` §9 fixes as 「既決として動かさない」.
//!
//! # Why the alphabet is here rather than in a crate
//!
//! [`super::Cid::to_text`] writes RFC 4648 base32 out longhand in `lib.rs`, for the reason recorded
//! there: gx fixes one spelling and a table is what fixes it. The same argument reaches base64 --
//! and 41 §2 allows this crate 「serde, thiserror 程度」, which a codec dependency is not. The
//! `base64` crate hand 1 declared on gx-witness for 44 §2.2 is therefore dropped by hand 5 at its
//! first use, which is where req/50 §5 said the version would be re-opened; the package leaves the
//! tree with it. `tests/base64_vectors.rs` holds RFC 4648 §10's own vectors, so the table is
//! checked against the standard rather than against this implementation's opinion of it.
//!
//! Standard alphabet with padding (RFC 4648 §4), not the URL-safe one: 44 §2.2 puts this string in
//! a JSON body, and the DSSE and in-toto envelopes 42 §4 compares gx's against use the standard
//! table.
//!
//! # Why it is public, and why it is a module
//!
//! `gx-witness` spells `DsseEnvelope.payload` with the same table (44 §2.2), and the one thing this
//! file exists to prevent is a second table. Publishing it is what makes 「one spelling」 checkable.
//!
//! A twelfth module rather than more of `lib.rs`, because **E-M2-16** (`req/38_ERRATA_2026-08-07.md`
//! §9) already ruled on exactly this choice when hand 1 added four: 「lib.rs 積み増し案は可読性最優先
//! (規律3)で却下——lane の 4 module 追加を追認」. 41 §2's module list for gx-core, extended from
//! seven to eleven by that erratum, becomes twelve; req/54 §4 raises it, in the form E-M2-16 took.
//!
//! # What this is not
//!
//! Not an identity, not a canonical encoding, and not a road to a digest. It is a rendering of
//! bytes for formats that have no byte type -- JSON. 42 §2.1's canonical rules are about DAG-CBOR
//! and nothing here touches them.

/// RFC 4648 §4, table 1.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

/// Infallible: every byte string has a spelling.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).map_or(0, |b| u32::from(*b));
        let b2 = chunk.get(2).map_or(0, |b| u32::from(*b));
        let packed = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(ALPHABET[(packed >> 18) as usize & 0x3f]));
        out.push(char::from(ALPHABET[(packed >> 12) as usize & 0x3f]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[(packed >> 6) as usize & 0x3f])
        } else {
            char::from(PAD)
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[packed as usize & 0x3f])
        } else {
            char::from(PAD)
        });
    }
    out
}

/// Read a byte string back, strictly.
///
/// Strict in the sense [`super::Cid::from_text`] is strict, and for the same reason: a decoder
/// that repairs its input makes the map from text to bytes many-to-one, and then a signature
/// has more than one spelling. Refused: a length that is not a multiple of four, padding
/// anywhere but at the end, more than two pad characters, a character outside table 1
/// (whitespace and newlines included), and a final group whose unused bits are set.
///
/// # Errors
/// A short reason. `&'static str` rather than [`super::Error`] because both callers are serde
/// visitors, which need only something that implements `Display` to hand to `E::custom` -- and
/// a codec detail has no business becoming a variant of the calculus's error type.
pub fn decode(text: &str) -> core::result::Result<Vec<u8>, &'static str> {
    let raw = text.as_bytes();
    // `is_multiple_of` rather than `% 4 != 0`: 卓-1 (req/38 §47) raised the declared MSRV from 1.85
    // to 1.89, which turned on clippy's `manual_is_multiple_of` (stable since 1.87). The lint firing
    // is the MSRV declaration doing its job — a rule that was previously suppressed because the
    // manifest claimed to support a compiler that did not have the method.
    if !raw.len().is_multiple_of(4) {
        return Err("base64 length is not a multiple of four (RFC 4648 §4 requires padding)");
    }
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    let pad = raw.iter().rev().take_while(|&&c| c == PAD).count();
    if pad > 2 {
        return Err("more than two padding characters");
    }
    let body = &raw[..raw.len() - pad];
    if body.contains(&PAD) {
        return Err("a padding character before the end of the input");
    }

    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &ch in body {
        let value = ALPHABET
            .iter()
            .position(|&a| a == ch)
            .ok_or("a character outside RFC 4648 §4 table 1")?;
        acc = (acc << 6) | u32::try_from(value).expect("alphabet index is below 64");
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).expect("masked to eight bits"));
        }
    }
    if bits > 0 && acc & ((1 << bits) - 1) != 0 {
        return Err(
            "the unused bits of the last character are set, which would give these \
                    bytes a second spelling",
        );
    }
    Ok(out)
}
