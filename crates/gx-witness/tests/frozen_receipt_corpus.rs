// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R38 / `req/38` §294-2 (b)** — a receipt this product issued in the past, frozen, read on
//! every floor, and **the limit it currently fails to meet, shown to be real on every run**.
//!
//! # The structural blindness this closes
//!
//! Every other receipt in this tree is **minted by the binary under test**. That makes the suite
//! blind in one direction by construction: a change that alters what the encoder writes and what the
//! decoder requires, *in the same commit*, is invisible — the fixtures move with the code. The third
//! pillar's claim is the opposite of that. It is that a receipt verifies **without the issuer**,
//! which means without this year's binary either.
//!
//! # What is frozen here, and why it may never be re-minted
//!
//! `tests/fixtures/frozen_receipts/issued_2026_08_18/` holds the artefacts of `req/280`, byte for
//! byte. Their digests are in `req/519` §9 rather than here — NFR-012's secret scanner reads a name
//! followed by a long hex run as a keyed token, and it is right to.
//!
//! **Regenerating these files defeats the entire probe.** If a change makes this suite red, the
//! answer is a decoder that still reads the old shape, never a fresher specimen. That instruction is
//! here rather than in a report because the report is not what the next author reads.
//!
//! # 🔴 The limit, stated as a limit
//!
//! This specimen **does not verify today**, and R38 could not make it. The suite is written the way
//! `req/38` §15's `truncationSurvives` practice asks for a floor that cannot yet be reached: the
//! limit is declared (`docs/LIMITS.md`), and a test shows **the limit is real** on every run, so the
//! declaration cannot quietly rot. The day the leaf derivation is fixed, `the_2026_08_specimen_still_does_not_decode`
//! turns red and forces the declaration to be updated. That is the point of it.
//!
//! ## Why, measured
//!
//! Eleven payload members (`0xab`, a CBOR map of eleven). Eight members the current `ReceiptPayload`
//! names are absent, in two families:
//!
//! 🔴 **`req/901` (2026-08-26): the number in the sentence above was `Five` and is now `Six`.**
//! Nothing about this document changed — the *current* payload gained a member and the difference
//! was never recounted. `tools/receipt_generation_gate.mjs` now derives both halves (17 named here,
//! 11 carried there, read from byte 0 of the specimen's DSSE payload) and refuses a stale one, so
//! this sentence cannot drift again without a red run. The table below lists five rows and is left
//! as it stands: which member the sixth is, and whether it belongs in a third family, is a reading
//! of this corpus that lane did not take (it holds no `cargo`).
//!
//! 🔴 **`req/919` W5 (2026-08-29): `Six` is now `Seven` — the sixth is `payload_version`, and this
//! lane names it because it is the one that caused the move.** F7 (`req/868` R-868-6) landed:
//! `Option<u32>` with `#[serde(default)]`, so its absence here is `None` -- "this document predates
//! the field", exactly the family `reversibility`/`verdict_digest`/`read_set` are already in. Which
//! member the *original* sixth was (added between the two `req/901` counts) remains unidentified;
//! this lane's cargo access answers only for the member it itself added.
//!
//! | absent member | how it was added | this document |
//! |---|---|---|
//! | `reversibility` (DR-46-26) | `Option`, written as an explicit null | decodes |
//! | `verdict_digest` (DR-46-31) | `Option` | decodes |
//! | `read_set` (DR-46-34) | `Option`, four spellings of absence | decodes |
//! | `payload_version` (F7, `req/868` R-868-6) | `Option`, `#[serde(default)]` | decodes |
//! | `fingerprint_scope` (P2, `req/350` §7-4) | **required**, no `serde` default | **refuses** |
//! | `determinism_boundary` (DR-46-28) | **required**, no `serde` default | **refuses** |
//!
//! The two refusals are **one mistake made twice** — different errata, different subjects, the same
//! shape: a member added to a signed, archived document type as a non-`Option` with no default, in a
//! product whose third pillar is that the document outlives the issuer. Either alone refuses the
//! file.
//!
//! ## 🔴 And why widening them is **not** the repair, which R38 learned by doing it
//!
//! R38 shipped `#[serde(default)]` for both, and it was wrong: `ReceiptPayload::ledger_digest`
//! re-encodes the **struct**, so a default puts members into the value that the signed bytes never
//! carried, the leaf moves, and inclusion comes back `refuted` — the word for tampering — about a
//! document nobody touched. Worse than the decode error it replaced.
//!
//! R38 then shipped `Option` + `skip_serializing_if`, and that was wrong too, in a more interesting
//! way. It removed those two members from the re-encoding (measured: the two keys were gone) and the
//! re-encoding was **still longer than the bytes that were signed** — R38 recorded a byte count for
//! it, and 🔴 **R39 records that the count is not re-derivable in this tree**: the build it was
//! taken on was withdrawn, and the re-encoding begins with a decode that does not happen. The claim
//! that survives is the direction, not the size. Because `read_set: null` and `reversibility: null` are
//! `Option` *without* skip — deliberately, since DR-46-34 made an explicit null the fourth spelling
//! of an absent read set. It also broke `boundary_attest::dr_46_28_the_payload_declares_a_boundary_field_that_is_not_optional`,
//! which encodes `req/459` ruling 3's argument that `unknown` is a first-class *value* and an
//! `Option` around it would be a second shape for one fact. That guard is right on its own terms.
//!
//! So the two requirements are in genuine conflict at the field level, and the conflict is a symptom:
//! **a signed, archived document's ledger leaf is re-derived from a struct whose canonical form moves
//! with the schema.** Every member ever added has already moved every historical leaf. The repair is
//! to derive the leaf from the bytes that were signed (or to record it), which touches 42 §3.10 and
//! the DR-46 series and is above this lane. `req/519` §7-5 and §11 carry it for ruling.

use std::path::{Path, PathBuf};

use gx_witness::{PublicKey, Receipt, VerifyingKeyRef};

fn frozen(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("frozen_receipts")
        .join("issued_2026_08_18")
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

/// 🔴 The signature over the frozen bytes still checks out.
///
/// This runs **first** and is asserted separately from the decode below, because the two are
/// different claims and folding them would let a decode failure be read as tampering. The signature
/// is over the raw payload bytes (DSSE PAE), so it is answerable without decoding anything, and it
/// is what makes the decode failure below a defect in **this binary** rather than a fact about the
/// specimen.
#[test]
fn a_receipt_this_product_issued_in_2026_08_still_carries_a_good_signature() {
    let receipt = specimen();
    let key = specimen_key();
    let verifying: VerifyingKeyRef<'_> = key.verifying();
    receipt.envelope.verify(&verifying).unwrap_or_else(|e| {
        panic!(
            "the frozen specimen's signature does not check out ({e}). This is not a compatibility \
             finding: either the fixture was edited (it may not be) or signing changed \
             incompatibly, and both are larger than the decode question below"
        )
    });
}

/// 🔴 **`req/38` §294-2 — the limit, shown to be real.**
///
/// This asserts the state the tree is **actually** in: the specimen does not decode, and the member
/// it stops at is named. It is not a defect pinned as expected behaviour — `docs/LIMITS.md` declares
/// it as an open limit and this is the pair that keeps the declaration honest. A green here means
/// "the limit is still exactly where LIMITS says it is". A **red** here means somebody moved it, and
/// the right response is to update the declaration, not this test.
///
/// The assertion is on the *shape* of the refusal rather than its exact words, because two members
/// are missing and which one `serde` reaches first is not a fact worth freezing.
#[test]
fn the_2026_08_specimen_still_does_not_decode_and_limits_says_so() {
    let receipt = specimen();
    match receipt.payload() {
        Err(e) => {
            let said = e.to_string();
            let named: Vec<&str> = DECLARED_REQUIRED_WITH_NO_DEFAULT
                .iter()
                .copied()
                .filter(|m| said.contains(m))
                .collect();
            println!("FROZEN_2026_08_18 refusal={said} names={named:?}");
            // 🔴 **R39 / `req/533` M-02(a)** — this stays a disjunction, over the set rather than
            // over two literals, because `serde` names **one** absent member per decode and asking
            // for both here would be asking for something no single refusal can carry (`req/540`
            // §1-7). What audit 38 counted is that a disjunction *on its own* lets a one-sided
            // widening through in silence; the two probes that close that are
            // `every_member_of_the_declared_set_is_still_required_with_no_default` and
            // `the_declared_set_is_the_set_limits_names`, and this claim is now one of three
            // rather than the whole alarm.
            assert!(
                !named.is_empty(),
                "the specimen is refused for a reason `docs/LIMITS.md` does not describe: {said}.                  The declared limit is {DECLARED_REQUIRED_WITH_NO_DEFAULT:?}, added as required with                  no `serde` default; anything else is a new finding and not this limit"
            );
        }
        Ok(_) => panic!(
            "🔴 the frozen 2026-08-18 receipt now **decodes**, which `docs/LIMITS.md` says it does              not. That is good news and this test is the alarm for it: update the declaration, and              check whether it also *verifies* — decoding was never the whole of the claim.              `ReceiptPayload::ledger_digest` re-encodes the struct, so a receipt can decode and              still compute a leaf that is not the leaf that was committed (`req/519` §7-5)."
        ),
    }
}

/// 🔴 The frozen bytes carry **none** of the five members added after they were written.
///
/// # 🔴 R39 — this test was called `the_re_encoding_gap_is_not_only_the_two_required_members`
///
/// It never re-encoded anything. Its doc gave the residue a size in bytes and its body searched the
/// signed bytes for four member names, which audit 38 caught (`req/533` §3-3): the string
/// `re_encoded_bytes` was nowhere in the tree, and the byte count `req/519` §7-5 cited as primary
/// evidence had been taken on a build that was then withdrawn. A test whose name promises a
/// measurement it does not perform is worse than no test, because a reader looking for the
/// measurement stops here.
///
/// The name is now what the body does, and it is a claim worth having on its own: the gap is not
/// only the two required members. `read_set`, `reversibility`, `verdict_digest` and (`req/919` W5,
/// 2026-08-29) `payload_version` are absent too — they decode, because they were added as `Option`,
/// which is the same-shaped change made correctly — so this is the evidence that the specimen
/// predates all six and that nobody has edited it since. The re-encoding question moved to
/// `crates/gx-cli/tests/r39_frozen_receipt_verdict.rs`, which attempts it, records that this build
/// cannot perform it, and measures the size the limit does have: eleven members carried against
/// eighteen named.
///
/// 🔴 **`req/901` (2026-08-26): "fifteen" here was stale and is now "seventeen".** Counted
/// mechanically from `crates/gx-witness/src/receipt.rs` by `tools/receipt_generation_gate.mjs`,
/// which fails the run rather than letting the sentence rot. 🔴 **`req/919` W5 (2026-08-29):
/// "seventeen" is now "eighteen"** for the same reason, one member later. The test function's own
/// name still says `five_members` — a test name is a historical record and is not renamed
/// (`INHERITED_PRINCIPLES` §3d); what it measures is the absence of the members added after the
/// specimen, and that set is now six.
#[test]
fn the_frozen_bytes_carry_none_of_the_five_members_added_after_them() {
    let receipt = specimen();
    let signed = &receipt.envelope.payload;
    let holds = |needle: &str| signed.windows(needle.len()).any(|w| w == needle.as_bytes());
    println!(
        "FROZEN_2026_08_18 signed_bytes={} carries_read_set={} carries_reversibility={}          carries_boundary={} carries_scope={}",
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
            "the frozen specimen carries `{absent}`, which every reading of it in `req/519` says              it does not. The fixture was edited, and it may not be"
        );
    }
}

// ---------------------------------------------------------------------------
// 🔴 **R39 / `req/533` M-02(a)** — the declared set, in one place
// ---------------------------------------------------------------------------

/// The members `docs/LIMITS.md` declares as "added to `ReceiptPayload` as required, with no `serde`
/// default", which is the whole of the limit this file exists to keep honest.
///
/// # Why a constant and not a disjunction
///
/// R38 asserted `said.contains("determinism_boundary") || said.contains("fingerprint_scope")`, and
/// audit 38 counted what that costs: `serde` names **one** absent member per refusal, so a build
/// that widened one of the two would go on being green while the refusal quietly changed which name
/// it printed — and the page would go on declaring both. R38 had already shipped and withdrawn
/// exactly that one-sided widening (`req/519` §7-3), so it is not a hypothetical road.
///
/// The disjunction survives in [`the_2026_08_specimen_still_does_not_decode_and_limits_says_so`]
/// because it is the true statement about a *single* decode. What carries the weight instead is the
/// pair below it: nothing in the frozen bytes names any member of this set, and nothing in the
/// current schema makes any member of this set optional. A one-sided widening moves the second one.
///
/// 🔴 A constant one has to *remember* to extend is a constant that rots, so
/// [`the_declared_set_is_the_set_limits_names`] reads the page and refuses a disagreement.
const DECLARED_REQUIRED_WITH_NO_DEFAULT: [&str; 2] = ["determinism_boundary", "fingerprint_scope"];

/// The repository root, two steps up from this crate.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-witness")
        .to_path_buf()
}

/// `ReceiptPayload`'s declaration, as source text: from `pub struct ReceiptPayload {` to the closing
/// brace at column zero.
///
/// Read rather than derived: the question is what the **schema** says, and a reflection over a
/// constructed value would answer a question about serde's output for one instance instead.
fn receipt_payload_source() -> String {
    let text = std::fs::read_to_string(repo_root().join("crates/gx-witness/src/receipt.rs"))
        .expect("the payload's own module is here");
    let at = text
        .find("pub struct ReceiptPayload {")
        .expect("`ReceiptPayload` is declared in receipt.rs");
    let rest = &text[at..];
    let end = rest.find("\n}").expect("the declaration closes") + 2;
    rest[..end].to_string()
}

/// The `name: Type` pairs of a struct declaration, with the attribute lines that sit above each.
///
/// A hand-rolled reader and not a parser: it takes lines that look like `name: Type,` at one level
/// of indentation, which is the whole of what this file needs and is what `rustfmt` guarantees for
/// this declaration. A member spelled some other way would be **absent** from the map rather than
/// mis-read, and every assertion below treats absence as a failure.
fn members(source: &str) -> Vec<(String, String, Vec<String>)> {
    let mut out = Vec::new();
    let mut attributes: Vec<String> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[") {
            attributes.push(trimmed.to_string());
            continue;
        }
        if trimmed.starts_with("///") || trimmed.starts_with("//") || trimmed.is_empty() {
            continue;
        }
        let declaration = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        if let Some((name, ty)) = declaration.split_once(": ") {
            if name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                && !name.is_empty()
            {
                out.push((
                    name.to_string(),
                    ty.trim_end_matches(',').to_string(),
                    std::mem::take(&mut attributes),
                ));
                continue;
            }
        }
        attributes.clear();
    }
    out
}

/// 🔴 **R39 / `req/533` M-02(a) — the frozen bytes carry no member of the declared set.**
///
/// The suite already asserted this for four names written out by hand. It is asserted here over the
/// **set**, so that a lane which adds a third required member to the page and to the constant gets
/// this claim about it for free, and a lane which adds one to only one of the two gets a red from
/// [`the_declared_set_is_the_set_limits_names`] instead of silence.
#[test]
fn the_frozen_bytes_carry_no_member_of_the_declared_set() {
    let receipt = specimen();
    let signed = &receipt.envelope.payload;
    let holds = |needle: &str| signed.windows(needle.len()).any(|w| w == needle.as_bytes());
    let carried: Vec<&str> = DECLARED_REQUIRED_WITH_NO_DEFAULT
        .iter()
        .copied()
        .filter(|m| holds(m))
        .collect();
    println!(
        "FROZEN_2026_08_18 declared_set={DECLARED_REQUIRED_WITH_NO_DEFAULT:?} \
         signed_bytes={} carried={carried:?}",
        signed.len()
    );
    assert!(
        carried.is_empty(),
        "the frozen specimen carries {carried:?}, which every reading of it in `req/519` says it \
         does not. The fixture was edited, and it may not be"
    );
}

/// 🔴 **R39 / `req/533` M-02(a) — and the current schema still requires every one of them.**
///
/// This is the claim that makes the pair an alarm rather than a coincidence. Claim one is about
/// bytes written in August 2026 and will be true for ever; claim two is about the schema **this
/// build** carries, and it is the one a widening moves. Without it, a build that made
/// `fingerprint_scope` optional would leave every other probe in this file green — the specimen
/// would still be refused, for the other member — while `docs/LIMITS.md` went on declaring two.
///
/// Two ways a member stops being required are checked, because R38 shipped and withdrew **both**:
/// `Option<..>` (`req/519` §7-4) and `#[serde(default)]` (`req/519` §7-3).
#[test]
fn every_member_of_the_declared_set_is_still_required_with_no_default() {
    let source = receipt_payload_source();
    let members = members(&source);
    let names: Vec<&str> = members.iter().map(|(n, _, _)| n.as_str()).collect();
    println!(
        "FROZEN_2026_08_18 payload_members={} {names:?}",
        names.len()
    );

    for declared in DECLARED_REQUIRED_WITH_NO_DEFAULT {
        let (_, ty, attributes) = members
            .iter()
            .find(|(name, _, _)| name == declared)
            .unwrap_or_else(|| {
                panic!(
                    "`docs/LIMITS.md` declares `{declared}` a required member of `ReceiptPayload` \
                     and the type does not have a member by that name. Members read: {names:?}"
                )
            });
        println!("FROZEN_2026_08_18 member={declared} type={ty} attributes={attributes:?}");
        assert!(
            !ty.starts_with("Option<"),
            "🔴 `{declared}` is now `{ty}`. The limit `docs/LIMITS.md` declares — two members added \
             as required with no default — has moved, and the page is now wrong rather than \
             pessimistic. Update the declaration; do not update this test"
        );
        assert!(
            !attributes.iter().any(|a| a.contains("serde(default")),
            "🔴 `{declared}` now carries a serde default ({attributes:?}). `req/519` §7-3 measured \
             what that does: the leaf moves and an untouched receipt comes back `refuted`, which is \
             the word for tampering. Update the declaration; do not update this test"
        );
    }
}

/// 🔴 **R39 / `req/540` R-3b — the constant above and the page cannot drift apart.**
///
/// `req/540` KA-5: a constant a person has to remember to extend is a constant that guards nothing.
/// The page names its members in one sentence — "…added to `ReceiptPayload` as required, with no
/// `serde` default — `a` (…) and `b` (…)" — so this reads that sentence, keeps the backticked names
/// that are **actually members of the type** (which drops `ReceiptPayload` and `serde` without a
/// hand-written exclusion list), and refuses a set that is not the constant's.
///
/// 🔴 What this still does not close, stated rather than implied: one author editing the sentence
/// and the constant in one commit satisfies both sides. Nothing here can stop that; what it stops
/// is the two moving apart by accident, which is the shape `req/519` §7-3 actually produced.
#[test]
fn the_declared_set_is_the_set_limits_names() {
    let limits = std::fs::read_to_string(repo_root().join("docs/LIMITS.md"))
        .expect("the page this file is the pair of");
    const MARKER: &str = "as required, with no `serde` default";
    let at = limits
        .find(MARKER)
        .expect("`docs/LIMITS.md` no longer declares the limit this file is the pair of");
    // The sentence, and not the paragraph: whichever boundary is nearer behind the marker — the end
    // of the previous sentence or the end of the previous paragraph — starts it. A span that
    // reached back further would pick up member names from the neighbouring table and this probe
    // would then be asserting that the constant equals "every member the section mentions".
    let start = limits[..at]
        .rfind(". ")
        .map(|cut| cut + 2)
        .max(limits[..at].rfind("\n\n").map(|cut| cut + 2))
        .unwrap_or(0);
    let end = at + limits[at..].find(". ").expect("the sentence ends") + 1;
    let sentence = &limits[start..end];

    let source = receipt_payload_source();
    let member_names: Vec<String> = members(&source).into_iter().map(|(n, _, _)| n).collect();
    let mut named: Vec<&str> = sentence
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|word| member_names.iter().any(|m| m == word))
        .collect();
    named.sort_unstable();
    named.dedup();
    let mut declared: Vec<&str> = DECLARED_REQUIRED_WITH_NO_DEFAULT.to_vec();
    declared.sort_unstable();
    println!("FROZEN_2026_08_18 limits_sentence={sentence:?}");
    println!("FROZEN_2026_08_18 limits_names={named:?} constant={declared:?}");
    assert_eq!(
        named, declared,
        "🔴 `req/540` R-3b: `docs/LIMITS.md` names {named:?} as the members added required with no \
         default and this file's constant holds {declared:?}. One of the two moved without the \
         other, which is the whole failure mode §294's pair exists to prevent"
    );
}
