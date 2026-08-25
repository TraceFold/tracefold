// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **I-1** (`req/38_ERRATA_2026-08-07.md` §26) — the three `IdentityView` projections this crate
//! takes digests through, and the two questions A-10 asks of each.
//!
//! # What the audit found
//!
//! req/67 §2.1 wrote three mutations of the identity face and none of them was caught:
//!
//! | mutation | what it means | how it was found |
//! |---|---|---|
//! | `AdmitProofView.policy_decisions` → `&[]` | an `Admit` stops being identified by what decided it | hand-written battery B-3, RC=0 |
//! | `DenyReasons::identity_view` → `Default::default()` | a refusal's `proof_digest` stops depending on the refusal | `cargo mutants`, `verdict.rs:182` |
//! | `ElidedText::identity_view` → `Default::default()` | M3-21's "an operator holding the original can recompute the line" becomes false | `cargo mutants`, `verdict.rs:510` | (sem: SEM-gx-gate-357)
//!
//! The audit's one-line reading was "the decision face is dense, the record face (identity) has a hole" (sem: SEM-gx-gate-358), and the ruling adopted
//! the fix as a tag blocker:
//!
//! > "🔴I-1 (adopted = close it immediately in a fix batch, tag blocker): apply **A-10-shaped
//! > defense** to the 3 IdentityView projection sites (AdmitProofView/DenyReasons/ElidedText) =
//! > assert the canonical encode's map key count + 'two values differing in only 1 field have
//! > different digests' x every field" (sem: SEM-gx-gate-359)
//!
//! # Why the suites that already exist did not see it
//!
//! Three separate reasons, and each one is a shape worth recognising again:
//!
//! 1. `proof_digest.rs` computes its expected value with `cid::compute(&proof)` — **through the very
//!    projection under test**. A mutated projection moves both sides of the equation, so the
//!    assertion holds and says nothing. Every test here compares the digest of one value with the
//!    digest of *another value*, which is a question the projection cannot answer tautologically.
//! 2. `verdict_order.rs` asserts the order of the vectors **after construction**, through the
//!    accessors. The accessors are not the projection.
//! 3. `error_vocabulary.rs`'s `the_elision_identifies_the_text_it_replaced` varies the text's
//!    *length* as well as its content, and the elided line names the length separately from the
//!    digest — so it stays discriminating even when the digest stops being. The fixture below holds
//!    the length fixed, which is the difference between measuring the line and measuring the digest
//!    inside it.
//!
//! # The A-10 form (`req/38_ERRATA_2026-08-07.md` §18)
//!
//! > "A-10 (adopted, the required DoD of M3's first hand): one assertion that `Checkpoint`'s encode
//! > map key count = 5 (= 3 covered + 2 declared-out). A mechanical guard against a mirror-struct
//! > structure where an added field silently falls out of the signed set" (sem: SEM-gx-gate-360)
//!
//! `gx-log/tests/checkpoint_core.rs` is that test and this file is its shape one milestone later.
//! `AdmitProofView` is a mirror struct of `AdmitProof` in the strict sense — 42 §1.3 lists the type
//! as "every field", so the two field sets must be equal rather than merely related — which is why (sem: SEM-gx-gate-361)
//! the count assertion here is joined by a comparison against **the field table 42 §3.8 declares**,
//! parsed out of the spec file. That reading was the struct's own derived `Debug` until M5 hand 2;
//! req/68 §4 **J-3** raised the format dependency and §37/§38 settle it here, in the same turn as
//! **I-11** (`gate_shape.rs`, 41 §4) because the two are one design.

use std::collections::BTreeMap;

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use gx_core::{Cid, ProofRef, TheoremId, TransformationId};
use gx_gate::verdict::{elide, DenyReasons, InvariantResult, PolicyDecisionRecord};
use gx_gate::{
    AdmitProof, Reason, ReasonSource, Verdict, INVARIANT_VIOLATED, MAX_MESSAGE_BYTES, POLICY_DENIED,
};
use gx_witness::PolicyDecision;

// ---------------------------------------------------------------------------
// Reading the bytes an identity is taken over
// ---------------------------------------------------------------------------

/// The projected bytes: exactly what [`cid::compute`] hashes (42 §1.1).
///
/// Going through [`cbor::encode`] is the point rather than a convenience — a projection that could
/// be encoded some other way would not be the thing the spec hashes. The same helper opens
/// `gx-canon/tests/identity_view.rs`, which is where this file's questions were first asked of
/// gx-core's two views.
fn view_bytes<T: IdentityView>(value: &T) -> Vec<u8> {
    cbor::encode(&value.identity_view()).expect("the projection of a valid value must encode")
}

/// The keys of a canonical DAG-CBOR map, and the count its head byte declares.
///
/// Two readings of the same bytes on purpose, which is `gx-log/tests/checkpoint_core.rs`'s helper
/// (A-10) in the crate that cannot import it: the head byte is the encoder's own statement of how
/// many pairs follow — canonical form puts a count under 24 in the low five bits of a
/// major-type-5 byte (42 §2.1, RFC 8949 §3) — and the decode is what names them. A test that only
/// counted would pass on a struct that swapped one field for another.
fn map_keys(bytes: &[u8]) -> (u8, Vec<String>) {
    let head = bytes[0];
    assert_eq!(
        head & 0b1110_0000,
        0b1010_0000,
        "the encoding of a projection with named fields is a CBOR map (major type 5); head byte \
         was {head:#04x}"
    );
    let declared = head & 0b0001_1111;
    assert!(
        declared < 24,
        "a map of {declared} or more pairs spells its count in following bytes; this helper reads \
         the short form only, which is all any projection in this crate needs"
    );
    let decoded: BTreeMap<String, serde::de::IgnoredAny> =
        cbor::decode(bytes).expect("a canonical map of text keys");
    (declared, decoded.into_keys().collect())
}

/// The element count a CBOR array's head byte declares, and each element read as a map of keys.
///
/// `DenyReasons` projects to a **slice**, so the outer shape is major type 4 and the A-10 question
/// splits in two: how many reasons are recorded, and how much of each one is.
fn array_of_maps(bytes: &[u8]) -> (u8, Vec<Vec<String>>) {
    let head = bytes[0];
    assert_eq!(
        head & 0b1110_0000,
        0b1000_0000,
        "a slice projects to a CBOR array (major type 4); head byte was {head:#04x}"
    );
    let declared = head & 0b0001_1111;
    assert!(declared < 24, "the short form only");
    let decoded: Vec<BTreeMap<String, serde::de::IgnoredAny>> =
        cbor::decode(bytes).expect("a canonical array of maps");
    (
        declared,
        decoded
            .into_iter()
            .map(|m| m.into_keys().collect())
            .collect(),
    )
}

/// 🔴 **J-3**: the field names 42 §3.8 gives `AdmitProof`, read out of the spec file.
///
/// This helper used to read the field names off `format!("{value:#?}")` -- the derived `Debug` --
/// and req/68 §4 raised that as **J-3**: "`debug_field_names` reads a derived `Debug`'s format ...
/// it breaks if `{:#?}`'s format changes" (sem: SEM-gx-gate-362). Its option (b) is what is written here:
///
/// > **move toward I-11's shape** -- parse 42 §3.8's field table and check it against the real type
/// > (the 42 version of what `gate_input_spec.rs` did for 41 §4). (b) is the design I-11 reserved
/// > for M4/M5's 'hand that gives the gate a third field', so **it is natural to decide it once, in
/// > the same window** (sem: SEM-gx-gate-363)
///
/// §37/§38 make this that hand (E-7's retirement is the gx-gate diff), so J-3 and I-11 are settled
/// in one turn as the ruling asked.
///
/// # What changed about the claim, not just about the reader
///
/// The old comparison was *implementation against implementation*: `AdmitProof`'s derived `Debug`
/// against `AdmitProofView`'s map keys. Both sides move when a hand edits the crate, so the two
/// could be grown together and the probe would keep passing while drifting away from 42 §1.3's
/// "every field". Reading the left-hand side out of 42 §3.8 makes the comparison (sem: SEM-gx-gate-364)
/// *spec against implementation*, which is the whole content of I-11 -- and it costs nothing here,
/// because the mirror-struct guard the file exists for (a field added to one declaration and not
/// the other) is still exactly what fails.
///
/// The table's shape is 42 §3.8's: a first row whose first cell names the type, then continuation
/// rows whose first cell is empty. Each row's second cell is a fenced field name.
fn fields_of_admit_proof_in_42_3_8() -> Vec<String> {
    let path = workspace_root().join("req/spec/40-architecture/42-data-model.md");
    let whole = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    // Scoped to §3.8 before anything is parsed. 42 names `AdmitProof` in **two** tables: §1.3's
    // IdentityView register, where the second cell is the exclusion rule ("every field") (sem: SEM-gx-gate-365), and
    // §3.8's field table, where it is a field name. The first draft of this helper read the whole
    // file, matched §1.3 first, and compared `["every field"]` against five keys -- a scan pointed // (sem: SEM-gx-gate-366)
    // at the wrong table, which is §30's lesson again and the reason the section bound is here
    // rather than in a comment asking a reader to be careful.
    let section = whole
        .split("### 3.8")
        .nth(1)
        .expect("42 still has a §3.8")
        .split("### 3.9")
        .next()
        .expect("§3.8 is followed by §3.9");
    let text = section;

    let mut names: Vec<String> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            if inside {
                break; // the table ended
            }
            continue;
        }
        let mut cells = line.trim_matches('|').split('|');
        let first = cells.next().unwrap_or_default().trim();
        let second = cells.next().unwrap_or_default().trim();

        if first == "`AdmitProof`" {
            inside = true;
        } else if inside && !first.is_empty() {
            break; // a new type's first row
        } else if !inside {
            continue;
        }

        let name = second.trim_matches('`');
        assert!(
            !name.is_empty(),
            "42 §3.8 gives every `AdmitProof` row a field name; this one is blank: {line}"
        );
        names.push(name.to_string());
    }

    assert!(
        inside,
        "42 §3.8 no longer has a row whose first cell is `AdmitProof`"
    );
    names.sort();
    names
}

/// `<root>` from this crate's manifest directory.
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

/// The compile-time half of the mirror guard (`gate_input_spec.rs`'s second test, one type over).
///
/// The spec parse above says "these are the names 42 §3.8 declares" and the map-key comparison says
/// "the encoded projection carries exactly those" (sem: SEM-gx-gate-367). This third leg pins the **projection struct
/// itself** at compile time: an exhaustive destructuring of [`AdmitProofView`] stops compiling the
/// moment a field is added to it, renamed, or removed, which is a failure no string scan can be
/// written around.
///
/// # What the old `Debug` read was guarding, and who guards it now
///
/// `AdmitProof`'s own fields are **private**, which is why J-3's helper reached for `{:#?}` at all
/// -- an integration test has no other view of them. Dropping that read therefore has to answer
/// where "a sixth field added to `AdmitProof` and projected nowhere" is caught (sem: SEM-gx-gate-368). It is caught by the
/// type's single door: [`AdmitProof::new`] takes one argument per field and builds the struct with a
/// literal, so a sixth field is a compile error at the constructor and at every call site,
/// including the fixtures in this file. The `Debug` read was a second guard on something the
/// constructor already made impossible -- and it was the *weaker* of the two, since it only ever ran
/// against values a test had built.
///
/// What it was **not** guarding, and what the spec parse now adds, is drift between the crate and
/// 42 §3.8. That direction had no probe at all before this hand.
fn view_field_names_pinned_at_compile_time(proof: &AdmitProof) -> Vec<String> {
    let gx_gate::verdict::AdmitProofView {
        policy_decisions: _,
        invariant_results: _,
        evidence_digests: _,
        composed_from: _,
        proof_ref: _,
    } = proof.identity_view();

    let mut names = vec![
        "policy_decisions".to_string(),
        "invariant_results".to_string(),
        "evidence_digests".to_string(),
        "composed_from".to_string(),
        "proof_ref".to_string(),
    ];
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn cid_of(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn policy(id: &str) -> PolicyDecisionRecord {
    PolicyDecisionRecord {
        policy_id: id.to_string(),
        decision: PolicyDecision::Deny,
        diagnostics_digest: None,
    }
}

fn invariant(id: &str) -> InvariantResult {
    InvariantResult::new(id.to_string(), true, None).expect("a short detail")
}

/// One `AdmitProof` with every field occupied, so that a projection dropping any of them is
/// visible. All five differences below are taken against this value.
fn base_proof() -> AdmitProof {
    AdmitProof::new(
        vec![policy("alpha")],
        vec![invariant("a-check")],
        vec![cid_of(9)],
        vec![TransformationId(cid_of(1))],
        None,
    )
}

fn digest_of_admit(proof: AdmitProof) -> Cid {
    Verdict::Admit(proof)
        .proof_digest()
        .expect("an AdmitProof has a canonical form")
}

fn reason(code: &str, message: &str, id: &str) -> Reason {
    Reason::new(
        code,
        message.to_string(),
        ReasonSource::Policy {
            policy_id: id.to_string(),
        },
    )
    .expect("the codes used here are in REASON_CODES")
}

fn digest_of_deny(reasons: Vec<Reason>) -> Cid {
    Verdict::deny(reasons)
        .expect("at least one reason")
        .proof_digest()
        .expect("a Vec<Reason> has a canonical form")
}

/// The identity of a string, re-derived on the test side.
///
/// `ElidedText` is private to `verdict.rs` — properly so, since nothing outside the crate elides —
/// and the value it names can still be checked from here, because 42 §1.1 says what the digest of a
/// text *is*: BLAKE3-256 over its canonical DAG-CBOR. This type states that rule independently, so
/// the assertion below compares the crate's answer with the spec's answer rather than with itself.
struct TextIdentity<'t>(&'t str);

impl IdentityView for TextIdentity<'_> {
    type View<'a>
        = &'a str
    where
        Self: 'a;

    fn identity_view(&self) -> Self::View<'_> {
        self.0
    }
}

fn digest_of_text(text: &str) -> Cid {
    cid::compute(&TextIdentity(text)).expect("a string always has a canonical form")
}

// ---------------------------------------------------------------------------
// 1. AdmitProof — the mirror struct (A-10)
// ---------------------------------------------------------------------------

/// The projection declares five map keys and they are 42 §3.8's five field names (**A-10**).
#[test]
fn the_admit_projection_declares_the_five_keys_of_42_3_8() {
    let (declared, keys) = map_keys(&view_bytes(&base_proof()));

    assert_eq!(
        declared, 5,
        "42 §3.8 gives `AdmitProof` five fields and 42 §1.3 projects \"every field\" (sem: SEM-gx-gate-369); the encoding \
         declares {declared}. A field that leaves the projection leaves the CID, and every other \
         test in this crate stays green (req/67 §2.1, battery B-3)"
    );
    assert_eq!(
        keys,
        [
            "composed_from",
            "evidence_digests",
            "invariant_results",
            "policy_decisions",
            "proof_ref"
        ],
        "the five keys are not the five names of 42 §3.8"
    );
}

/// 🔴 **J-3**: the projection's key set is the field set **42 §3.8 declares**.
///
/// The count above is a literal, and a literal is a claim a hand can keep true by editing it. This
/// is the same claim held against the spec: a sixth field added to `AdmitProof` and not to
/// `AdmitProofView` is a field outside the CID, which is exactly the mirror-struct defect A-10 was
/// written for -- and a field 42 §3.8 declares that the projection never carries is the same defect
/// arriving from the other side.
///
/// Three lists, compared pairwise, and each one has a different way of being wrong:
///
/// | list | comes from | what its being wrong means |
/// |---|---|---|
/// | [`fields_of_admit_proof_in_42_3_8`] | the spec file, parsed | the crate drifted from 42 §3.8 |
/// | `map_keys(view_bytes(..))` | the bytes actually hashed | a field is declared but not encoded |
/// | [`view_field_names_pinned_at_compile_time`] | the compiler | the view was edited |
///
/// Before this hand the first list was `format!("{proof:#?}")` and no list came from the spec at
/// all (req/68 §4 **J-3**; §37/§38 fire it here with **I-11**, which is the same move on 41 §4).
#[test]
fn the_admit_projection_has_one_key_per_field_of_the_struct() {
    let proof = base_proof();
    let (_, keys) = map_keys(&view_bytes(&proof));
    let declared = fields_of_admit_proof_in_42_3_8();

    println!("ADMIT_PROOF_FIELDS_IN_42_3_8={declared:?} PROJECTED_KEYS={keys:?}");
    assert_eq!(
        declared, keys,
        "42 §3.8 and `AdmitProofView` declare different field sets, so something the type is said \
         to hold is not part of what it is (42 §1.3: \"every field\")" // (sem: SEM-gx-gate-370)
    );
    assert_eq!(
        view_field_names_pinned_at_compile_time(&proof),
        keys,
        "the projection encodes a key set the struct does not declare"
    );
}

/// Every one of the five fields moves the digest a receipt records.
///
/// This is the half a count cannot state: a projection that names all five keys and fills one of
/// them with a constant declares five and identifies four. Battery B-3 is precisely that mutation —
/// `policy_decisions: &[]` — and it survived seventeen suites.
#[test]
fn every_field_of_an_admit_proof_reaches_its_digest() {
    let baseline = digest_of_admit(base_proof());

    let variants: Vec<(&str, AdmitProof)> = vec![
        (
            "policy_decisions",
            AdmitProof::new(
                vec![policy("bravo")],
                vec![invariant("a-check")],
                vec![cid_of(9)],
                vec![TransformationId(cid_of(1))],
                None,
            ),
        ),
        (
            "invariant_results",
            AdmitProof::new(
                vec![policy("alpha")],
                vec![invariant("b-check")],
                vec![cid_of(9)],
                vec![TransformationId(cid_of(1))],
                None,
            ),
        ),
        (
            "evidence_digests",
            AdmitProof::new(
                vec![policy("alpha")],
                vec![invariant("a-check")],
                vec![cid_of(8)],
                vec![TransformationId(cid_of(1))],
                None,
            ),
        ),
        (
            "composed_from",
            AdmitProof::new(
                vec![policy("alpha")],
                vec![invariant("a-check")],
                vec![cid_of(9)],
                vec![TransformationId(cid_of(2))],
                None,
            ),
        ),
        (
            "proof_ref",
            AdmitProof::new(
                vec![policy("alpha")],
                vec![invariant("a-check")],
                vec![cid_of(9)],
                vec![TransformationId(cid_of(1))],
                Some(ProofRef {
                    lean_spec_version: "GxSpec-0.1".to_string(),
                    theorem_ids: vec![TheoremId::T5],
                }),
            ),
        ),
    ];

    assert_eq!(variants.len(), 5, "one difference per projected field");
    for (field, variant) in variants {
        assert_ne!(
            digest_of_admit(variant),
            baseline,
            "two proofs differing only in `{field}` have the same proof_digest, so `{field}` is \
             declared by the projection and not carried by it"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. DenyReasons — the array, and what each element holds
// ---------------------------------------------------------------------------

/// A `Deny`'s projection is the reasons themselves: one array element per reason, three keys each.
///
/// The A-10 question asked of a slice. `verdict.rs:182`'s survivor replaces the whole projection
/// with `Default::default()` — an empty slice — and the element count is where that is visible
/// before any digest is compared.
#[test]
fn the_deny_projection_is_the_reasons_themselves_three_keys_each() {
    let reasons = vec![
        reason(POLICY_DENIED, "alpha forbids this", "alpha"),
        reason(POLICY_DENIED, "bravo forbids this", "bravo"),
    ];
    let (declared, elements) = array_of_maps(&view_bytes(&DenyReasons(&reasons)));

    assert_eq!(
        declared, 2,
        "M2-10 takes the digest over the vector of reasons, so the number of refusals is part of \
         what a receipt identifies; the encoding declares {declared}"
    );
    assert_eq!(elements.len(), 2);
    for element in &elements {
        assert_eq!(
            element,
            &["code", "message", "source"],
            "42 §3.8 gives `Reason` three fields and all three reach the digest"
        );
    }
}

/// Every field of a reason, and the number of reasons, moves the digest.
///
/// Four differences against one baseline. Each is the smallest change that can be made in isolation:
/// the code with the source held fixed, the message with both held fixed, the source id with the
/// text of the message held fixed (which is I-2's fixture seen from the identity side), and one
/// reason more.
#[test]
fn every_field_of_a_reason_reaches_a_denys_digest() {
    let baseline = digest_of_deny(vec![reason(POLICY_DENIED, "alpha forbids this", "alpha")]);

    let variants: Vec<(&str, Vec<Reason>)> = vec![
        // The second live code. It was `EVIDENCE_INSUFFICIENT` until **E-7** fired (§37 M5-04
        // adopted (b)) and `Reason::new` began refusing the retired word; what the case needs is any (sem: SEM-gx-gate-371)
        // code that differs from the baseline's, and `INVARIANT_VIOLATED` is one with a producer.
        (
            "code",
            vec![reason(INVARIANT_VIOLATED, "alpha forbids this", "alpha")],
        ),
        (
            "message",
            vec![reason(POLICY_DENIED, "alpha forbids that", "alpha")],
        ),
        (
            "source",
            vec![reason(POLICY_DENIED, "alpha forbids this", "bravo")],
        ),
        (
            "the number of reasons",
            vec![
                reason(POLICY_DENIED, "alpha forbids this", "alpha"),
                reason(POLICY_DENIED, "bravo forbids this", "bravo"),
            ],
        ),
    ];

    for (field, reasons) in variants {
        assert_ne!(
            digest_of_deny(reasons),
            baseline,
            "two refusals differing only in `{field}` have the same proof_digest, so the record of \
             why a change was refused is not what the receipt identifies (M2-10)"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. ElidedText — the digest an operator is told to recompute
// ---------------------------------------------------------------------------

/// The elided line names the digest of the text it replaced, and 42 §1.1 says which value that is.
///
/// M3-21 sends an over-long message to a digest rather than into the bytes a receipt is hashed
/// over, and the promise that makes it a record rather than a deletion is "an operator holding the
/// original can recompute the same line" (sem: SEM-gx-gate-372). `verdict.rs:510`'s survivor breaks exactly that promise while leaving the line's shape,
/// its byte count and its `gx1:` prefix intact.
#[test]
fn an_elided_line_names_the_digest_of_the_text_it_replaced() {
    let text = "y".repeat(MAX_MESSAGE_BYTES + 1);
    let line = elide(text.clone()).expect("digestible");

    assert!(
        line.contains(&cid::to_text(&digest_of_text(&text))),
        "the line does not name the CID of what it replaced, so an operator holding the original \
         cannot recompute it (M3-21); line was {line}"
    );
    assert!(
        !line.contains(&cid::to_text(&digest_of_text(""))),
        "the line names the digest of the empty string, which is what a projection that dropped \
         its text would produce"
    );
}

/// Two texts of the same length elide to different lines.
///
/// The length is held fixed on purpose. An elided line carries the byte count beside the digest, so
/// texts of different lengths stay distinguishable even when the digest stops depending on them —
/// which is why `error_vocabulary.rs`'s elision test cannot see the projection fail.
#[test]
fn two_texts_of_the_same_length_do_not_elide_to_the_same_line() {
    let one = "a".repeat(MAX_MESSAGE_BYTES + 1);
    let other = "b".repeat(MAX_MESSAGE_BYTES + 1);
    assert_eq!(one.len(), other.len(), "the fixture holds the length fixed");

    assert_ne!(
        elide(one).expect("digestible"),
        elide(other).expect("digestible"),
        "two different messages of the same length elide to one line, so the elision identifies \
         nothing"
    );
}

// ---------------------------------------------------------------------------
// 4. All three land on the wire face
// ---------------------------------------------------------------------------

/// The projections are not a second encoding (AC-014, 42 §2.1-6).
///
/// The same assertion `gx-canon/tests/identity_view.rs` makes of gx-core's two views, made of this
/// crate's: whatever a projection produces is bytes the wire face would have written, which is what
/// makes `cid::compute` a composition rather than a parallel road.
#[test]
fn the_projections_of_this_crate_land_on_the_wire_face() {
    let reasons = vec![reason(POLICY_DENIED, "alpha forbids this", "alpha")];

    assert!(cbor::is_canonical(&view_bytes(&base_proof())));
    assert!(cbor::is_canonical(&view_bytes(&DenyReasons(&reasons))));
    assert!(
        cbor::is_canonical(&view_bytes(&TextIdentity("a message"))),
        "the text projection the elided line's digest is taken over"
    );
}
