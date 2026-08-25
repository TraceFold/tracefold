// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-010 — the JCS compatibility layer is deterministic.
//!
//! 34 AC-010 verbatim: "Given: a Transformation x of identical logical content. When:
//! `jcs::encode(x)` run three times in a row. Then: all three are exactly identical byte
//! sequences (determinism)" (sem: SEM-gx-canon-025). FR-010 places it as a SHOULD, for
//! interoperability with in-toto / SCITT / Sigstore, and 42 §2.2 fixes what it operates on:
//! "**the same logical value**" (all fields of the struct, not limited to IdentityView but the API-exposed view).
//!
//! # The two faces must not be confused
//!
//! This is the *second* route, not a second identity. 42 §2.2 says so twice over -- the JCS
//! digest would be SHA-256, it is called `JcsDigest` rather than `Cid`, and "`JcsDigest`
//! does not constitute identity" (sem: SEM-gx-canon-026). Accordingly the whole struct goes through here, `id` and `created_at`
//! included, which is the visible difference from `cid::compute`: the identity face drops two
//! fields and this one drops none. A test below pins that difference so the two faces cannot
//! quietly converge.
//!
//! # What is deliberately absent
//!
//! No SHA-256, and no `JcsDigest`. B-08 in req/10 §6 rules it out of M1 -- none of the seventeen
//! M1 acceptance criteria asks for it, and 52 contract 2 forbids implementing what 32-functional
//! does not ask for. The last test in this file is that ruling as a machine check rather than a
//! promise.

mod support;

use gx_canon::{cid, jcs};
use proptest::prelude::*;
use support::{any_transformation, sample_transformation};

/// The keys of the top-level object, in the order the bytes actually spell them.
///
/// Parsing into a `serde_json::Value` would sort them on the way in and prove nothing, so this
/// walks the text. It doubles as the whitespace check: RFC 8785 output has no insignificant
/// whitespace at all, and a scanner that knows where the strings are can say so.
fn top_level_keys_and_whitespace(bytes: &[u8]) -> (Vec<String>, usize) {
    let text = std::str::from_utf8(bytes).expect("JCS output is UTF-8");
    let mut keys = Vec::new();
    let mut whitespace_outside_strings = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    let mut expecting_key = false;

    for ch in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                if expecting_key && depth == 1 {
                    keys.push(std::mem::take(&mut current));
                }
                expecting_key = false;
                continue;
            }
            if expecting_key && depth == 1 {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                current.clear();
            }
            '{' => {
                depth += 1;
                expecting_key = depth == 1;
            }
            '}' => depth -= 1,
            ',' => expecting_key = depth == 1,
            c if c.is_whitespace() => whitespace_outside_strings += 1,
            _ => {}
        }
    }
    (keys, whitespace_outside_strings)
}

#[test]
fn ac_010_three_consecutive_runs_are_byte_identical() {
    let value = sample_transformation();
    let first = jcs::encode(&value).expect("the value has a JCS form");
    let second = jcs::encode(&value).expect("the value has a JCS form");
    let third = jcs::encode(&value).expect("the value has a JCS form");
    assert_eq!(first, second);
    assert_eq!(second, third);
    println!("AC010_RUNS=3 AC010_BYTES={}", first.len());
}

/// 42 §2.2: the API view, not the IdentityView. All ten fields of 41 §3, sorted the way RFC 8785
/// sorts them, with no whitespace between them.
#[test]
fn ac_010_the_jcs_face_carries_every_field_and_sorts_them() {
    let bytes = jcs::encode(&sample_transformation()).expect("JCS form");
    let (keys, whitespace) = top_level_keys_and_whitespace(&bytes);

    let mut expected = vec![
        "id",
        "intent_id",
        "order",
        "subject",
        "target",
        "delta",
        "context",
        "actor",
        "parents",
        "created_at",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected, "42 §2.2 asks for the whole struct, sorted");
    assert_eq!(keys.len(), 10, "41 §3 has ten fields");
    assert_eq!(whitespace, 0, "RFC 8785 admits no insignificant whitespace");
}

/// The difference between the two faces, stated so it cannot erode: JCS keeps `id` and
/// `created_at`, the identity face drops them (42 §1.3 against 42 §2.2).
///
/// The two timestamps are small on purpose. A one-nanosecond difference at a realistic epoch
/// value does *not* show up in JCS output, for the reason the test below this one measures, and
/// using one here would have made this test about number precision instead of about the two
/// faces.
#[test]
fn ac_010_the_jcs_face_is_not_the_identity_face() {
    let mut base = sample_transformation();
    base.created_at = gx_core::Timestamp(1);
    let mut later = base.clone();
    later.created_at = gx_core::Timestamp(2);

    assert_ne!(
        jcs::encode(&base).expect("JCS"),
        jcs::encode(&later).expect("JCS"),
        "`created_at` is inside the API view, so JCS output must move with it"
    );
    assert_eq!(
        cid::compute(&base).expect("CID"),
        cid::compute(&later).expect("CID"),
        "and the identity must not move with it (42 §1.3-2)"
    );
}

/// A measured limit of the compatibility route, pinned rather than hidden.
///
/// RFC 8785 §3.2.2.3 serialises numbers as ECMAScript doubles, so an integer above 2^53 does not
/// survive the JSON route intact. `Timestamp` is `i64` nanoseconds (42 §3.2), and any realistic
/// epoch value -- 1.7e18 today -- is three orders of magnitude past that ceiling. Two
/// transformations recorded a nanosecond apart therefore produce the same JCS bytes.
///
/// This costs nothing on the identity side: `created_at` is outside the IdentityView (42 §1.3-2),
/// so no `Cid` is affected, and AC-010 asks for determinism, which is unharmed. What it does
/// affect is 42 §2.2's stated purpose -- interoperability with in-toto / SCITT / Sigstore -- since
/// a receipt exported through this route carries a rounded timestamp. Whether the JSON view
/// should spell `Timestamp` as a string is a spec decision and is filed as an erratum candidate
/// rather than decided here; the test exists so the behaviour is a measurement and not a
/// surprise.
#[test]
fn ac_010_rfc_8785_numbers_are_doubles_so_nanosecond_timestamps_round() {
    let mut base = sample_transformation();
    base.created_at = gx_core::Timestamp(1_700_000_000_000_000_000);
    let mut one_nanosecond_later = base.clone();
    one_nanosecond_later.created_at = gx_core::Timestamp(1_700_000_000_000_000_001);

    assert_eq!(
        jcs::encode(&base).expect("JCS"),
        jcs::encode(&one_nanosecond_later).expect("JCS"),
        "if this ever differs, RFC 8785 number handling changed and the erratum candidate is moot"
    );

    // Where the ceiling actually sits, measured rather than quoted.
    let exact = 1i64 << 53;
    let mut at_ceiling = base.clone();
    at_ceiling.created_at = gx_core::Timestamp(exact);
    let mut just_past = base.clone();
    just_past.created_at = gx_core::Timestamp(exact + 1);
    let rounds = jcs::encode(&at_ceiling).expect("JCS") == jcs::encode(&just_past).expect("JCS");
    println!("AC010_JCS_INTEGER_EXACT_UP_TO=2^53 AC010_ROUNDS_AT_2POW53_PLUS_1={rounds}");

    let mut small = base.clone();
    small.created_at = gx_core::Timestamp(1_000_000);
    let mut small_plus = base.clone();
    small_plus.created_at = gx_core::Timestamp(1_000_001);
    assert_ne!(
        jcs::encode(&small).expect("JCS"),
        jcs::encode(&small_plus).expect("JCS"),
        "below the ceiling the field must still be carried faithfully"
    );

    // And the DAG-CBOR route, which is the one identity is built on, keeps every bit.
    assert_ne!(
        gx_canon::cbor::encode(&base).expect("CBOR"),
        gx_canon::cbor::encode(&one_nanosecond_later).expect("CBOR"),
        "the primary encoding must not lose what the compatibility route loses"
    );
}

/// The same logical value written two ways gives one JCS document -- which is what makes this a
/// canonicalisation rather than a serialisation.
#[test]
fn ac_010_a_respelled_input_canonicalises_to_the_same_bytes() {
    let value = sample_transformation();
    let direct = jcs::encode(&value).expect("JCS");

    let document = serde_json::to_value(&value).expect("JSON form");
    let scattered = format!(
        "  {}  ",
        serde_json::to_string_pretty(&document).expect("pretty")
    );
    let reparsed: gx_core::Transformation =
        serde_json::from_str(&scattered).expect("still the same value");

    assert_eq!(jcs::encode(&reparsed).expect("JCS"), direct);
}

/// B-08, as a check rather than a promise: M1 produces no `JcsDigest` and depends on no SHA-256
/// implementation. 52 contract 2 -- "what is not in 32-functional.md is not implemented" (sem: SEM-gx-canon-027) -- is the rule, and
/// a grep is the enforcement.
#[test]
fn ac_010_no_sha256_and_no_jcs_digest_were_added() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    // Dependency *names*, not substrings. The first spelling of this test matched "ring" inside
    // the word "during" in a comment, which is the kind of check that fails loudly today and
    // passes vacuously the day the comment is reworded.
    let declared: Vec<&str> = manifest
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(name, _)| name.trim())
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .collect();
    for banned in ["sha2", "sha-2", "ring", "openssl", "sha1"] {
        assert!(
            !declared.contains(&banned),
            "`{banned}` was declared as a dependency; B-08 keeps SHA-256 out of M1"
        );
    }
    for entry in std::fs::read_dir(root.join("src")).expect("src is readable") {
        let path = entry.expect("dir entry").path();
        let text = std::fs::read_to_string(&path).expect("source is UTF-8");
        // Prose may name the type -- `jcs.rs` quotes 42 §2.2, which is where the name comes
        // from -- and code may not. The distinction is the whole content of B-08.
        let offending: Vec<usize> = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim_start().starts_with("//"))
            .filter(|(_, l)| l.contains("JcsDigest"))
            .map(|(n, _)| n + 1)
            .collect();
        assert!(
            offending.is_empty(),
            "`JcsDigest` appeared in code in {} at line(s) {offending:?}; it belongs to a later \
             milestone",
            path.display()
        );
    }
}

proptest! {
    /// The acceptance criterion over generated values rather than one fixture.
    #[test]
    fn ac_010_jcs_is_deterministic(value in any_transformation()) {
        let a = jcs::encode(&value).expect("JCS");
        let b = jcs::encode(&value).expect("JCS");
        let c = jcs::encode(&value).expect("JCS");
        prop_assert_eq!(&a, &b);
        prop_assert_eq!(&b, &c);
    }

    /// Non-vacuity: a canonicaliser that returned a constant would pass everything above.
    #[test]
    fn ac_010_distinct_values_give_distinct_documents(
        a in any_transformation(),
        b in any_transformation(),
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(jcs::encode(&a).unwrap(), jcs::encode(&b).unwrap());
    }
}
