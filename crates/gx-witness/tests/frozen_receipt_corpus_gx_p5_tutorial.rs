// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/568` §4 / `req/38` §338** — second frozen specimen, `gx_p5_tutorial` family.
//!
//! `req/519` §7-2 found that the three families `req/294` asked to be frozen
//! (`req/280`/`gx_p5_tutorial`/`tfcore_live`) are **the same one defect, hit three times**: all
//! eleven-member CBOR maps missing `fingerprint_scope` (P2, `req/350` §7-4) and
//! `determinism_boundary` (DR-46-28), both added to `ReceiptPayload` as required with no `serde`
//! default. Only `req/280`'s specimen (`issued_2026_08_18/`) was ever actually frozen — this file
//! closes that gap for the `gx_p5_tutorial` family, per `req/568`'s diagnosis and `req/38` §338's
//! acceptance. `tfcore_live` stays unfrozen: its public key was never exported standalone (§3-2 of
//! `req/568`) and is out of this file's scope.
//!
//! # Why a new file and not a shared helper
//!
//! `frozen_receipt_corpus.rs` is not touched here, deliberately — its own header warns that a
//! frozen specimen must not be edited, and merging the two suites into one parameterised file would
//! mean editing the file that already carries the `issued_2026_08_18` claims. `req/568` §4-2 chose
//! new-file-over-edit for the same reason `frozen_receipt_corpus.rs` chose new-fixture-over-mutation.
//!
//! # Which of the sibling file's assertions are repeated here, and which are not
//!
//! Four of `frozen_receipt_corpus.rs`'s six tests are about **this specific fixture's bytes** and
//! are repeated below, pointed at `issued_2026_08_14_gx_p5_tutorial/` instead:
//! signature-still-checks-out, decode-still-refuses (naming a member of the declared set),
//! carries-none-of-the-five-members-added-after, and carries-no-member-of-the-declared-set. The
//! other two (`every_member_of_the_declared_set_is_still_required_with_no_default`,
//! `the_declared_set_is_the_set_limits_names`) assert facts about **today's schema and
//! `docs/LIMITS.md`** — nothing about which fixture is on disk — so `frozen_receipt_corpus.rs`
//! already asserts them once and running the identical schema/docs check a second time here would
//! be a duplicate assertion with a second place to go stale, not a second finding.
//!
//! # Provenance
//!
//! Copied byte-for-byte, read-only, from WSL `~/gx_p5_tutorial/project/.gx/receipts/gx1_6lpurzo…
//! .commit.json` and `~/gx_p5_tutorial/pub.json` (`req/568` §2, §4-1). Digests are not repeated
//! here for the same NFR-012 reason `frozen_receipt_corpus.rs` states for the sibling specimen.

use std::path::{Path, PathBuf};

use gx_witness::{PublicKey, Receipt, VerifyingKeyRef};

fn frozen(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("frozen_receipts")
        .join("issued_2026_08_14_gx_p5_tutorial")
        .join(name)
}

fn specimen() -> Receipt {
    let text = std::fs::read_to_string(frozen("receipt.json")).expect("the frozen receipt is here");
    serde_json::from_str(&text).expect("the frozen receipt is a `Receipt` document")
}

fn specimen_key() -> PublicKey {
    #[derive(serde::Deserialize)]
    struct Pub {
        key_id: String,
        public_key: String,
    }
    let text = std::fs::read_to_string(frozen("key.pub.json")).expect("the frozen key is here");
    let parsed: Pub = serde_json::from_str(&text).expect("`gx key gen`'s two members");
    let bytes = gx_core::b64::decode(&parsed.public_key).expect("the public key is base64");
    PublicKey::from_bytes(parsed.key_id, &bytes).expect("the public key is on the curve")
}

/// The members `docs/LIMITS.md` declares required with no `serde` default — the same set
/// `frozen_receipt_corpus.rs` names, restated locally because integration test binaries do not
/// share a crate and this file must not import a `const` out of a sibling one.
const DECLARED_REQUIRED_WITH_NO_DEFAULT: [&str; 2] = ["determinism_boundary", "fingerprint_scope"];

/// 🔴 The signature over the frozen `gx_p5_tutorial` bytes still checks out.
///
/// Run first and asserted separately from the decode below, same reasoning as the sibling file: a
/// decode failure is only a defect in *this binary* once tampering is ruled out by a signature that
/// holds.
#[test]
fn a_receipt_gx_p5_tutorial_issued_in_2026_08_still_carries_a_good_signature() {
    let receipt = specimen();
    let key = specimen_key();
    let verifying: VerifyingKeyRef<'_> = key.verifying();
    receipt.envelope.verify(&verifying).unwrap_or_else(|e| {
        panic!(
            "the frozen gx_p5_tutorial specimen's signature does not check out ({e}). This is not a \
             compatibility finding: either the fixture was edited (it may not be) or signing changed \
             incompatibly, and both are larger than the decode question below"
        )
    });
}

/// 🔴 The `gx_p5_tutorial` specimen still does not decode, refused for a member `docs/LIMITS.md`
/// names.
///
/// Same shape as `frozen_receipt_corpus.rs`'s `the_2026_08_specimen_still_does_not_decode_and_limits_says_so`:
/// a green here means the limit `docs/LIMITS.md` declares is still real for this specimen too; a red
/// means either the limit moved (checked against the `issued_2026_08_18` specimen already, in the
/// sibling file) or this second specimen alone stopped being refused, which would itself be worth
/// knowing.
#[test]
fn the_gx_p5_tutorial_specimen_still_does_not_decode_and_limits_says_so() {
    let receipt = specimen();
    match receipt.payload() {
        Err(e) => {
            let said = e.to_string();
            let named: Vec<&str> = DECLARED_REQUIRED_WITH_NO_DEFAULT
                .iter()
                .copied()
                .filter(|m| said.contains(m))
                .collect();
            println!("FROZEN_GX_P5_TUTORIAL refusal={said} names={named:?}");
            assert!(
                !named.is_empty(),
                "the gx_p5_tutorial specimen is refused for a reason `docs/LIMITS.md` does not \
                 describe: {said}. The declared limit is {DECLARED_REQUIRED_WITH_NO_DEFAULT:?}, \
                 added as required with no `serde` default; anything else is a new finding and not \
                 this limit"
            );
        }
        Ok(_) => panic!(
            "🔴 the frozen gx_p5_tutorial receipt now **decodes**, which `docs/LIMITS.md` says it \
             does not for this defect family. That is good news and this test is the alarm for it: \
             update the declaration, and check whether it also *verifies* — decoding was never the \
             whole of the claim."
        ),
    }
}

/// 🔴 The frozen `gx_p5_tutorial` bytes carry **none** of the five members added after they were
/// written — same claim as the sibling file's `the_frozen_bytes_carry_none_of_the_five_members_added_after_them`,
/// re-derived from this specimen's own signed bytes.
#[test]
fn the_gx_p5_tutorial_frozen_bytes_carry_none_of_the_five_members_added_after_them() {
    let receipt = specimen();
    let signed = &receipt.envelope.payload;
    let holds = |needle: &str| signed.windows(needle.len()).any(|w| w == needle.as_bytes());
    println!(
        "FROZEN_GX_P5_TUTORIAL signed_bytes={} carries_read_set={} carries_reversibility={}          carries_boundary={} carries_scope={}",
        signed.len(),
        holds("read_set"),
        holds("reversibility"),
        holds("determinism_boundary"),
        holds("fingerprint_scope"),
    );
    for absent in [
        "read_set",
        "reversibility",
        "determinism_boundary",
        "fingerprint_scope",
    ] {
        assert!(
            !holds(absent),
            "the frozen gx_p5_tutorial specimen carries `{absent}`, which every reading of it in \
             `req/568` says it does not. The fixture was edited, and it may not be"
        );
    }
}

/// 🔴 The frozen `gx_p5_tutorial` bytes carry no member of the declared set — same claim as the
/// sibling file's `the_frozen_bytes_carry_no_member_of_the_declared_set`, re-derived from this
/// specimen's own signed bytes rather than assumed from the family diagnosis.
#[test]
fn the_gx_p5_tutorial_frozen_bytes_carry_no_member_of_the_declared_set() {
    let receipt = specimen();
    let signed = &receipt.envelope.payload;
    let holds = |needle: &str| signed.windows(needle.len()).any(|w| w == needle.as_bytes());
    let carried: Vec<&str> = DECLARED_REQUIRED_WITH_NO_DEFAULT
        .iter()
        .copied()
        .filter(|m| holds(m))
        .collect();
    println!(
        "FROZEN_GX_P5_TUTORIAL declared_set={DECLARED_REQUIRED_WITH_NO_DEFAULT:?} \
         signed_bytes={} carried={carried:?}",
        signed.len()
    );
    assert!(
        carried.is_empty(),
        "the frozen gx_p5_tutorial specimen carries {carried:?}, which every reading of it in \
         `req/568` says it does not. The fixture was edited, and it may not be"
    );
}
