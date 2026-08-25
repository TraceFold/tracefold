// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! T-24 — the text form `gx1:<base32>` (42 §1.2).
//!
//! 42 §1.2 verbatim: "a fixed `gx1:` prefix + 32 raw bytes encoded in RFC 4648 Base32
//! (lowercase, no padding). Example: `gx1:abc123...` (32 bytes → 52 characters)" (sem: SEM-gx-canon-075). Human-readable output for CLI,
//! API and logs is this form and no other; the binary embedding stays a 32-byte byte string
//! (42 §1.1, already fixed by the golden vector G-1 in step 3).
//!
//! # Where the conversion is allowed to live
//!
//! req/31 §1 weighed two escape routes -- put a base32 codec in gx-core so it can spell a `Cid`,
//! or keep gx-core ignorant of the display convention entirely -- and req/31 §11 adopted the
//! second. E-JCS-1 (`req/38_ERRATA_2026-08-07.md` §5) overturned that on 42 §1.2's word: JSON
//! embedding takes the `gx1:` form as well, `Serialize for Cid` lives in gx-core, and a
//! serializer that mints the spelling cannot be a layer that does not know it. So the alphabet
//! moved to `gx_core::Cid::to_text` and this module's [`to_text`]/[`from_text`] delegate to it.
//!
//! The rule that 42 §1.2's "takes as canonical" (sem: SEM-gx-canon-076) actually asks for is unaffected and is what the check at
//! the bottom of this file enforces: **one** implementation of the spelling in the workspace. A
//! copy kept here "for layering" would be the second text format. `gx-core::Cid` still has an
//! opaque `Debug` and no `Display`, so no `{}` in a log line mints one by accident.
//!
//! # Why a decoder as well as an encoder
//!
//! AC-011's two-process check compares what the two processes *print*, so the printed form is
//! load-bearing evidence and has to be readable back. And a text form with no parser cannot be
//! shown to be injective: without [`gx_canon::cid::from_text`] the round-trip property below
//! could not be stated, and a collision in the encoder would go unseen.
//!
//! # Where the expected strings come from
//!
//! Python's `base64.b32encode`, lowercased with padding stripped -- an implementation of RFC
//! 4648 that is not this one. Taking them from gx's own encoder would only prove the encoder has
//! not changed (the same reasoning the golden vectors of step 3 are written under).

mod support;

use gx_canon::cid::{from_text, to_text};
use gx_core::Cid;
use proptest::prelude::*;
use support::any_cid;

/// RFC 4648 base32, lowercase: the only characters a `gx1:` body may contain.
const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz234567";

#[test]
fn t_024_thirty_two_bytes_become_fifty_two_characters() {
    let text = to_text(&Cid([0u8; 32]));
    let body = text
        .strip_prefix("gx1:")
        .expect("the prefix is fixed (42 §1.2)");
    assert_eq!(
        body.len(),
        52,
        "42 §1.2: 32 bytes -> 52 characters (sem: SEM-gx-canon-077)"
    );
    assert!(!text.contains('='), "no padding (sem: SEM-gx-canon-078)");
    assert!(
        body.chars().all(|c| ALPHABET.contains(c)),
        "RFC 4648 base32, lowercase only: {body}"
    );
}

/// Known answers from an independent RFC 4648 implementation.
#[test]
fn t_024_known_answers_match_an_independent_base32() {
    let cases: [([u8; 32], &str); 3] = [
        (
            [0u8; 32],
            "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        (
            [0xffu8; 32],
            "gx1:777777777777777777777777777777777777777777777777777q",
        ),
        (
            [0x11u8; 32],
            "gx1:ceirceirceirceirceirceirceirceirceirceirceirceirceiq",
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(to_text(&Cid(bytes)), expected);
        assert_eq!(from_text(expected).expect("valid text"), Cid(bytes));
    }

    let counting: [u8; 32] = core::array::from_fn(|i| u8::try_from(i).expect("i < 32"));
    assert_eq!(
        to_text(&Cid(counting)),
        "gx1:aaaqeayeaudaocajbifqydiob4ibceqtcqkrmfyydenbwha5dypq"
    );
}

#[test]
fn t_024_a_bad_spelling_is_refused_rather_than_guessed() {
    let good = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let body = &good[4..];

    let refused: [(&str, String); 8] = [
        ("no prefix", body.to_string()),
        ("wrong prefix", format!("gx2:{body}")),
        ("uppercase body", format!("gx1:{}", body.to_uppercase())),
        ("padded", format!("gx1:{}======", &body[..46])),
        ("too short", format!("gx1:{}", &body[..51])),
        ("too long", format!("gx1:{body}a")),
        (
            "character outside the alphabet",
            format!("gx1:{}1", &body[..51]),
        ),
        // The final character carries four bits that are not part of the digest. A spelling
        // that sets them decodes to the same 32 bytes as one that does not, so admitting it
        // would give one `Cid` two texts -- the same second-spelling problem CM-6 refuses on
        // the binary side.
        (
            "non-zero trailing bits",
            format!("{}b", &good[..good.len() - 1]),
        ),
    ];

    for (why, text) in refused {
        assert!(
            from_text(&text).is_err(),
            "`{text}` should have been refused ({why})"
        );
    }

    assert!(from_text(good).is_ok(), "the control must still parse");
}

proptest! {
    #[test]
    fn t_024_every_cid_survives_the_text_round_trip(cid in any_cid()) {
        let text = to_text(&cid);
        prop_assert_eq!(from_text(&text).expect("own output parses"), cid);
        prop_assert_eq!(text.len(), 56);
    }

    /// Injective: two digests never print the same. A text form that collided would make the
    /// two-process comparison of AC-011 vacuous.
    #[test]
    fn t_024_distinct_cids_print_distinctly(a in any_cid(), b in any_cid()) {
        prop_assert_eq!(to_text(&a) == to_text(&b), a == b);
    }
}

/// One spelling, one implementation of it -- checked instead of asserted.
///
/// This test used to say the opposite (`t_024_gx_core_does_not_know_the_text_form`): under
/// req/31 §11's default (b), gx-core was the layer that did not know the display convention, and
/// the alphabet lived in `gx-canon/src/cid.rs`. E-JCS-1 (`req/38_ERRATA_2026-08-07.md` §5) ruled
/// that JSON embedding uses `gx1:` too, and gx-core owns `Serialize for Cid`, so the spelling had
/// to move there. The *property that mattered* survives the move and is what this test now
/// checks: the base32 table exists exactly once in the workspace, so no two code paths can
/// disagree about how a digest is written.
#[test]
fn t_024_the_spelling_has_exactly_one_implementation() {
    fn alphabet_sites(dir: &std::path::Path) -> Vec<String> {
        let mut hits = Vec::new();
        for entry in std::fs::read_dir(dir).expect("source directory is readable") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&path).expect("source is UTF-8");
                for (n, line) in text.lines().enumerate() {
                    if line.contains("abcdefghijklmnopqrstuvwxyz234567") {
                        hits.push(format!("{}:{}", path.display(), n + 1));
                    }
                }
            }
        }
        hits
    }

    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core = alphabet_sites(&here.join("../gx-core/src"));
    let canon = alphabet_sites(&here.join("src"));

    assert_eq!(
        core.len(),
        1,
        "gx-core must hold the one RFC 4648 table (42 §1.2, E-JCS-1); found {core:?}"
    );
    assert!(
        canon.is_empty(),
        "gx-canon must delegate to it, not repeat it -- a second table is a second spelling: {canon:?}"
    );

    // And the delegation is real, not a copy that happens to agree today.
    let cid = Cid([0xa5; 32]);
    assert_eq!(to_text(&cid), cid.to_text());
}
