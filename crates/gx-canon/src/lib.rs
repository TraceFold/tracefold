//! Canonical form and content addressing.
//!
//! Spec: `req/spec/40-architecture/41-architecture.md` §2 for the module list and the
//! dependency line, `req/spec/40-architecture/42-*.md` for the encoding rules themselves.
//!
//! Two faces, kept apart on purpose (A-4, in `req/38_ERRATA_2026-08-07.md` §1):
//!
//! - **wire** -- encode and decode every field, so a value survives the round trip byte
//!   for byte (AC-009, AC-012). Whether bytes are canonical is decided by the encoder,
//!   never by the fact that a decoder accepted them; a decoder that accepts too much
//!   would otherwise define the canon it is supposed to be checked against.
//! - **identity** -- project a value through `IdentityView`, encode that projection on
//!   the wire face, hash it with BLAKE3 (AC-011, AC-013). `id` and `created_at` are not
//!   in the projection, so the same transformation recorded at two times has one CID.
//!
//! The identity face is defined as that composition, which is how the projection stops
//! being skippable: there is no second road to a `Cid` (AC-014).
//!
//! `IdentityView` is declared here and implemented here for the `gx-core` types, so the
//! trait and the impl share a crate and `gx-core` stays unaware of any of it (A-1).
//!
//! Step 3 of the plan in `req/31_M1_CONDITIONAL_PLAN_2026-08-07.md` §9 filled the wire face in
//! ([`cbor`]); step 4 is filling the identity face in ([`cid`], [`jcs`]).

#![forbid(unsafe_code)]

pub mod cbor;
pub mod cid;
pub mod jcs;

/// Everything the canonical layer can refuse to do.
///
/// Two of these are the underlying codec speaking ([`Error::Encode`], [`Error::Decode`]); the
/// rest are gx's own reading of 42 §2.1, produced by [`cbor::scan_strict`], and each names the
/// clause it enforces. Keeping them apart is what lets the negative-vector suite report *which*
/// layer caught a vector instead of only that something did.
///
/// 41 §6 types errors with `thiserror` and calls a panic a bug, which is why every refusal here
/// is a returned value -- including the ones that come from input a caller can control.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The codec would not serialise the value: a non-finite float, an integer outside
    /// `[-u64::MAX-1, u64::MAX]`, a map key that is not a string. req/26 §3's rule -- state the
    /// supported range and refuse the rest rather than guessing -- lands here.
    #[error("the value has no DAG-CBOR form: {0}")]
    Encode(String),

    /// The codec would not deserialise the bytes into the requested type.
    #[error("the bytes do not decode to the requested type: {0}")]
    Decode(String),

    /// The value encoded, but the bytes it produced are not ones gx admits -- a float field, for
    /// instance (42 §2.1-4). The distinction from [`Error::Encode`] matters: the codec was
    /// willing and gx was not.
    #[error("the value encodes to bytes gx does not admit as canonical: {0}")]
    NotCanonicalizable(Box<Error>),

    #[error("input ends {missing} byte(s) early (wanted {wanted} at byte {at})")]
    Truncated {
        at: usize,
        wanted: usize,
        missing: usize,
    },

    #[error("empty input: canonical DAG-CBOR is exactly one top-level item")]
    Empty,

    /// 42 §2.1-1, and the IPLD Strictness clauses at spec.md L55-L57.
    #[error(
        "the argument of major type {major} at byte {at} is written with additional \
         information {ai} (value {value}), which is not the shortest form (42 §2.1-1)"
    )]
    NonMinimal {
        at: usize,
        major: u8,
        ai: u8,
        value: u64,
    },

    /// 42 §2.1-3.
    #[error("indefinite-length item at byte {at} (42 §2.1-3)")]
    IndefiniteLength { at: usize },

    #[error("additional information {ai} at byte {at} is reserved (RFC 8949 §3)")]
    ReservedAdditionalInfo { at: usize, ai: u8 },

    /// 42 §2.1-5. gx admits no tags whatsoever, tag 42 included -- a `Cid` is embedded as a
    /// plain 32-byte string (42 §1.1), so there is nothing left for a tag to mean.
    #[error("CBOR tag {tag} at byte {at}; 42 §2.1-5 admits no tags at all, tag 42 included")]
    TagNotAllowed { at: usize, tag: u64 },

    /// 42 §2.1-4. Wider than IPLD, which admits 64-bit floats: the measures of P-10 are kept out
    /// of identity-bearing structures entirely, so no canonical value has anywhere to put one.
    #[error("floating point at byte {at}; 42 §2.1-4 keeps floats out of canonical values")]
    FloatNotAllowed { at: usize },

    #[error("simple value {ai} at byte {at}; only false, true and null are admitted")]
    SimpleValueNotAllowed { at: usize, ai: u8 },

    /// 42 §2.1-2, first clause.
    #[error("map key at byte {at} has major type {major}, but keys are text strings (42 §2.1-2)")]
    NonTextMapKey { at: usize, major: u8 },

    /// 42 §2.1-2, second and third clauses. One error for both because they are one comparison:
    /// keys must be *strictly* increasing, so a repeat and a swap fail the same test.
    #[error(
        "map key at byte {at} does not come strictly after its predecessor in bytewise order \
         (42 §2.1-2: sorted, and no duplicates)"
    )]
    UnsortedOrDuplicateMapKey { at: usize },

    #[error("text string at byte {at} is not valid UTF-8: {detail}")]
    InvalidUtf8 { at: usize, detail: String },

    /// IPLD Strictness spec.md L66: one top-level object, and every byte of the input belongs
    /// to it.
    #[error("{extra} byte(s) after the top-level item; a DAG-CBOR object is the whole input")]
    TrailingBytes { at: usize, extra: usize },

    /// Not a spec clause. 41 §6 counts a panic as a bug and a stack overflow is a panic that
    /// cannot be caught, so nesting is bounded rather than trusted.
    #[error("nesting deeper than {max} at byte {at}")]
    TooDeep { at: usize, max: usize },

    /// 42 §1.2. The readable form has exactly one spelling per digest, so anything else is a
    /// refusal rather than a best guess (req/26 §3).
    #[error("not a `gx1:<base32>` content identifier: {detail} (42 §1.2)")]
    CidText { detail: String },

    /// 42 §2.2, the compatibility route. Separate from [`Error::Encode`] because the two faces
    /// fail for different reasons and a caller debugging one should not be shown the other.
    #[error("the value has no RFC 8785 JSON form: {detail}")]
    Jcs { detail: String },
}

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, Error>;
