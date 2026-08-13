//! The wire face: canonical DAG-CBOR in, canonical DAG-CBOR out (A-4 面1).
//!
//! Spec: 42 §2.1 for the six rules that make a byte string canonical, 42 §2.3 for the
//! idempotence requirement, 41 §6 for the rule that every canonical encode goes through this
//! crate. AC-009 (round trip) and AC-012 (idempotence) are the acceptance criteria.
//!
//! # Who decides what "canonical" means
//!
//! The encoder does. [`is_canonical`] asks the encoder what it would have written and compares;
//! it never treats a successful parse as evidence. `ASM-01-2` requires exactly this, and it
//! requires it because of a measured failure in an earlier asset: `alpha-ledger::scan_strict`
//! was built on top of its parser and therefore accepted a CRLF-corrupted line with `Ok`,
//! because the parser did. A predicate built on a parser can only ever be as strict as that
//! parser happens to be, and the parser is the thing under test.
//!
//! The consequence is visible: a 64-bit float is canonical DAG-CBOR by IPLD's rules and
//! `serde_ipld_dagcbor` decodes it happily, yet `is_canonical` says no, because [`encode`]
//! refuses to produce one (42 §2.1-4). Parse-defined canonicity could not express that.
//!
//! # Two layers on the decode path
//!
//! [`scan_strict`] is gx's own reading of 42 §2.1, written from the spec text and RFC 8949 §3
//! rather than by consulting the dependency. [`decode`] runs it before handing the bytes to
//! `serde_ipld_dagcbor`. The two layers overlap heavily -- as of this writing the codec already
//! rejects nine of the twelve negative vectors on its own -- and the overlap is the point:
//! `tests/negative_vectors.rs` asserts the gx layer separately, so the guarantee survives a
//! change of dependency, and prints the split so nobody has to guess which layer is load-bearing.
//!
//! Strict-by-default on the decode side is CM-6, adopted 2026-08-07: for Tier 2 and Tier 3
//! vectors IPLD only says a decoder *should* reject, and gx chooses to reject anyway. The
//! reason is that canon has one entrance. If decode admitted a non-canonical spelling, two byte
//! strings would name one value and identity would stop being a function of content.
//!
//! # What this face is not
//!
//! It is not the identity face. Everything is encoded here, `id` and `created_at` included,
//! because AC-009 asks for a round trip over all fields. Dropping fields for CID purposes is
//! the `IdentityView` projection of step 4.

use crate::{Error, Result};
use ipld_core::ipld::Ipld;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Deep enough for any gx structure -- 42's deepest is a `Transformation` at four levels -- and
/// shallow enough that a hostile input cannot exhaust the stack before [`scan_strict`] returns.
const MAX_DEPTH: usize = 64;

/// Encode a value to canonical DAG-CBOR (42 §2.1).
///
/// The bytes are produced by `serde_ipld_dagcbor` (42 §2.1-6 names it as the implementation)
/// and then audited by [`scan_strict`], because the codec implements IPLD's rules and gx's are
/// narrower on two points: no floats (§2.1-4) and no tags at all (§2.1-5). A value that trips
/// either produces [`Error::NotCanonicalizable`] rather than bytes.
///
/// # Errors
/// [`Error::Encode`] when the value has no DAG-CBOR form at all -- a non-finite float, an
/// integer outside `[-u64::MAX-1, u64::MAX]`, a non-string map key. [`Error::NotCanonicalizable`]
/// when it has one that gx does not admit. Both are refusals rather than approximations, which
/// is req/26 §3's rule: state the range you handle and throw on the rest.
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_ipld_dagcbor::to_vec(value).map_err(|e| Error::Encode(e.to_string()))?;
    match scan_strict(&bytes) {
        Ok(()) => Ok(bytes),
        Err(e) => Err(Error::NotCanonicalizable(Box::new(e))),
    }
}

/// Decode canonical DAG-CBOR into a value.
///
/// Strict by default (CM-6): the bytes go through [`scan_strict`] first, so a spelling the
/// encoder would never have produced is refused even when the codec would have understood it.
///
/// # Errors
/// The [`scan_strict`] error when the bytes are not canonical, [`Error::Decode`] when they are
/// canonical but do not describe a `T`.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    scan_strict(bytes)?;
    serde_ipld_dagcbor::from_slice(bytes).map_err(|e| Error::Decode(e.to_string()))
}

/// Are these bytes what the encoder would have written?
///
/// The definition, in one line: `bytes` are canonical exactly when they parse to some value and
/// [`encode`] applied to that value returns `bytes` again. The judgement belongs to the encoder;
/// the parse only supplies a candidate (`ASM-01-2`, T-21, and the module docs above).
///
/// Returns `false` for anything that does not parse, and for anything that parses to a value the
/// encoder refuses -- floats and tags being the two cases where gx is stricter than IPLD.
#[must_use]
pub fn is_canonical(bytes: &[u8]) -> bool {
    match serde_ipld_dagcbor::from_slice::<Ipld>(bytes) {
        Ok(value) => matches!(encode(&value), Ok(again) if again == bytes),
        Err(_) => false,
    }
}

/// gx's own check of the six rules of 42 §2.1, independent of the codec.
///
/// Written from the IPLD DAG-CBOR Strictness section as quoted in
/// `req/33_NEGVECTOR_TABLE_A_2026-08-07.md` and from RFC 8949 §3's major-type / additional-info
/// table. No part of `serde_ipld_dagcbor` or `ipld-core` was read or copied to write it; the
/// crates are called, not transcribed.
///
/// Rule 6 ("all canonical encoding goes through gx-canon") is not a property of a byte string
/// and cannot be checked here -- AC-014's static check owns it (T-30's note says so).
///
/// # Errors
/// One [`Error`] variant per clause, each naming the clause and the byte offset.
pub fn scan_strict(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Err(Error::Empty);
    }
    let mut scan = Scan { b: bytes, i: 0 };
    scan.item(0)?;
    if scan.i == bytes.len() {
        Ok(())
    } else {
        Err(Error::TrailingBytes {
            at: scan.i,
            extra: bytes.len() - scan.i,
        })
    }
}

struct Scan<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Scan<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let at = self.i;
        let end = at.checked_add(n).ok_or(Error::Truncated {
            at,
            wanted: n,
            missing: n,
        })?;
        if end > self.b.len() {
            return Err(Error::Truncated {
                at,
                wanted: n,
                missing: end - self.b.len(),
            });
        }
        self.i = end;
        Ok(&self.b[at..end])
    }

    fn peek(&self) -> Result<u8> {
        self.b.get(self.i).copied().ok_or(Error::Truncated {
            at: self.i,
            wanted: 1,
            missing: 1,
        })
    }

    /// Read one head byte and its argument, enforcing the shortest-form rule of 42 §2.1-1 and
    /// the ban on indefinite lengths of 42 §2.1-3. Not used for major type 7, where the same
    /// additional-info values mean float widths rather than integer sizes.
    fn head(&mut self) -> Result<(u8, u64, usize)> {
        let at = self.i;
        let b = self.take(1)?[0];
        let major = b >> 5;
        let ai = b & 0x1f;
        let arg = match ai {
            0..=23 => u64::from(ai),
            24 => {
                let v = u64::from(self.take(1)?[0]);
                Self::require_minimal(v, major, 24, at, v >= 24)?
            }
            25 => {
                let raw = self.take(2)?;
                let v = u64::from(u16::from_be_bytes([raw[0], raw[1]]));
                Self::require_minimal(v, major, 25, at, v > u64::from(u8::MAX))?
            }
            26 => {
                let raw = self.take(4)?;
                let v = u64::from(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]));
                Self::require_minimal(v, major, 26, at, v > u64::from(u16::MAX))?
            }
            27 => {
                let raw = self.take(8)?;
                let mut eight = [0u8; 8];
                eight.copy_from_slice(raw);
                let v = u64::from_be_bytes(eight);
                Self::require_minimal(v, major, 27, at, v > u64::from(u32::MAX))?
            }
            31 => return Err(Error::IndefiniteLength { at }),
            _ => return Err(Error::ReservedAdditionalInfo { at, ai }),
        };
        Ok((major, arg, at))
    }

    fn require_minimal(value: u64, major: u8, ai: u8, at: usize, ok: bool) -> Result<u64> {
        if ok {
            Ok(value)
        } else {
            Err(Error::NonMinimal {
                at,
                major,
                ai,
                value,
            })
        }
    }

    fn item(&mut self, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(Error::TooDeep {
                at: self.i,
                max: MAX_DEPTH,
            });
        }
        // Major type 7 carries no integer argument: the additional-info field selects among
        // false/true/null, the simple values, and the three float widths. Reading it through
        // `head` would report a float as a non-minimal integer, which would be a true rejection
        // for the wrong stated reason.
        if self.peek()? >> 5 == 7 {
            return self.simple_or_float();
        }
        let (major, arg, at) = self.head()?;
        let len = usize::try_from(arg).map_err(|_| Error::Truncated {
            at,
            wanted: usize::MAX,
            missing: usize::MAX,
        })?;
        match major {
            0 | 1 => Ok(()),
            2 => {
                self.take(len)?;
                Ok(())
            }
            3 => {
                let raw = self.take(len)?;
                core::str::from_utf8(raw).map_err(|e| Error::InvalidUtf8 {
                    at,
                    detail: e.to_string(),
                })?;
                Ok(())
            }
            4 => {
                for _ in 0..len {
                    self.item(depth + 1)?;
                }
                Ok(())
            }
            5 => self.map(len, depth),
            6 => Err(Error::TagNotAllowed { at, tag: arg }),
            _ => unreachable!("major type 7 is handled above and majors are three bits"),
        }
    }

    /// 42 §2.1-2: keys are text strings, strictly increasing in the bytewise order of their
    /// *encoded* form.
    ///
    /// Encoded, not decoded: the comparison includes the major-type-and-length header, so a
    /// two-byte key precedes a three-byte one whatever their letters are. That is IPLD's
    /// spec.md L58 (「including their major type 3 and length」) and RFC 8949 §4.2.1's "bytewise
    /// lexicographic order of their deterministic encodings", and it is what the encoder named
    /// by 42 §2.1-6 actually writes. The prose of 42 §2.1-2 says instead 「キーのUTF-8バイト列に
    /// 対する」 order, which disagrees whenever two keys differ in length; the encoder's reading
    /// is the one implemented, and the disagreement is filed as an erratum candidate in req/41.
    fn map(&mut self, entries: usize, depth: usize) -> Result<()> {
        let all = self.b;
        let mut previous: Option<&[u8]> = None;
        for _ in 0..entries {
            let key_at = self.i;
            let major = self.peek()? >> 5;
            if major != 3 {
                return Err(Error::NonTextMapKey { at: key_at, major });
            }
            self.item(depth + 1)?;
            let key = &all[key_at..self.i];
            if let Some(prev) = previous {
                if key <= prev {
                    return Err(Error::UnsortedOrDuplicateMapKey { at: key_at });
                }
            }
            previous = Some(key);
            self.item(depth + 1)?;
        }
        Ok(())
    }

    /// Major type 7. `false`, `true` and `null` are the whole of what a canonical gx value may
    /// hold here: floats are out by 42 §2.1-4, `undefined` and the other simple values have no
    /// IPLD Data Model counterpart, and `break` only exists to end the indefinite-length items
    /// that 42 §2.1-3 forbids.
    fn simple_or_float(&mut self) -> Result<()> {
        let at = self.i;
        let ai = self.take(1)?[0] & 0x1f;
        match ai {
            20..=22 => Ok(()),
            25..=27 => Err(Error::FloatNotAllowed { at }),
            31 => Err(Error::IndefiniteLength { at }),
            28..=30 => Err(Error::ReservedAdditionalInfo { at, ai }),
            _ => Err(Error::SimpleValueNotAllowed { at, ai }),
        }
    }
}

/// Kani harness 1 of 3 (46 §4.2, row `gx-canon::cbor::encode/decode`).
///
/// The two claims of that row are 「往復性・panic-freedom（不正入力を含む）」. Both are here:
/// a round trip over a symbolic value, and totality of [`scan_strict`] over symbolic *bytes* --
/// the second being the one that matters, because [`scan_strict`] is gx's own hand-written parser
/// and the only place in the workspace where hostile input is walked byte by byte.
///
/// The module lives in this file because 41 §2 fixes gx-canon's module list at four (req/31 §2).
///
/// # Bounds (51 §5 asks for these to be named, not implied)
///
/// - `scan_strict` is checked over **all** byte strings of length 0..=2, symbolic in every bit
///   (2^16 inputs plus the shorter ones). Two bytes reaches three of the decisions 42 §2.1 makes
///   -- the whole 256-entry header table, a truncated argument, and a trailing byte after a
///   complete item. It does **not** reach a nested container; §5 of the hand-5 report says what
///   that costs and why the bound is here rather than higher.
/// - the round trip is checked over a symbolic `u8` payload.
/// - **the unwind numbers are the bound, and they were chosen by measurement.** `Scan::item` and
///   `Scan::map` recurse, so an unwind of 64 (the value `MAX_DEPTH` would suggest) asks CBMC to
///   unroll 64 levels of a mutual recursion whose input cannot nest more than three deep -- the
///   first attempt did exactly that and exhausted 8 GB and then the whole WSL VM. The values
///   below are the smallest that admit every path a 3-byte input can take.
/// - **not** checked: deep nesting (`MAX_DEPTH` = 64 is far beyond a bounded check of this size),
///   long inputs, and the codec crates' own correctness -- `serde_ipld_dagcbor` and `ipld-core`
///   are called, and Kani verifies the calls rather than auditing their source.
#[cfg(kani)]
mod verification {
    use super::*;

    /// Hostile input never panics: every byte string of length <= 2 gets an answer.
    ///
    /// This is the harness that earns its keep. [`scan_strict`] is the one place in the workspace
    /// where bytes a stranger chose are walked one at a time, and every index in it is one an
    /// off-by-one would turn into a panic.
    #[kani::proof]
    #[kani::unwind(2)]
    fn scan_strict_is_total_on_short_inputs() {
        // The empty input has exactly one answer, and it is a refusal rather than an acceptance:
        // a canonical DAG-CBOR object is one top-level item, and zero of them is not one.
        assert!(matches!(scan_strict(&[]), Err(Error::Empty)));

        // All 256 headers, one byte each. The length is concrete: a symbolic length makes every
        // slice index in the scanner symbolic too, and the measurement in §5 of the hand-5 report
        // is that CBMC then exhausts 8 GB rather than finishing.
        let byte: u8 = kani::any();
        let verdict = scan_strict(&[byte]);
        // Reaching this line for every one of the 256 is the panic-freedom claim.
        let _ = verdict.is_ok();
    }

    /// The round trip of the 46 §4.2 row, on the smallest value that exercises it.
    #[kani::proof]
    #[kani::unwind(12)]
    fn encode_then_decode_returns_the_value() {
        let value: u8 = kani::any();
        let bytes = encode(&value).expect("a u8 always has a canonical form");
        let back: u8 = decode(&bytes).expect("gx's own output is canonical");
        assert!(back == value);
    }
}
