// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-969-EXT** — adversarial probes for the attach interface against a *collected* response
//! (`req/969`).
//!
//! # Why this file exists beside `attach_interface.rs`
//!
//! That file's fixtures are built from the published API description; this file's are bytes GitHub
//! actually sent (`tests/fixtures/attach/PROVENANCE.md` records which is which, per `req/948c`
//! AC-C6). The distinction earned its keep immediately: **the constructed fixtures all pass against
//! a reader that gets the real response wrong.**
//!
//! # The defect these probes pin
//!
//! A real `attestations` response does not carry its bundles. Each entry has `"bundle": null` and a
//! `bundle_url` pointing at storage. The first reader answered that with `NoDsseEnvelope` — *this
//! document has no signed envelope* — when the truth is *the envelope is elsewhere and we have not
//! fetched it*. One is permanent and one is a fetch away, and a caller told the first stops looking.
//!
//! That is the same collapse [`NotAttested`] refuses one layer in, committed one layer out: the
//! answer vocabulary distinguished `FormatHasNoField` from `NotReadByThisBuild` while the refusal
//! vocabulary had no way to say "somewhere else". Probes ① ② ③ hold the three cases apart.

use gx_witness::attach::{self, AttachedAnswer, NotAttested, Refusal};
use gx_witness::coverage::Question;

/// The digest the collected response is keyed on: `gh_2.98.0_linux_amd64.tar.gz` of `cli/cli`
/// v2.98.0, taken from that release's published checksums file.
const REAL_SUBJECT: &str =
    "sha256:3b8ac6b30336802fc1a858d7c084e11cdf24ac1a761ca90b68022d7d729208de";

/// The response GitHub sent. **Collected**, with only the storage SAS query string redacted.
fn real_response() -> &'static [u8] {
    include_bytes!("fixtures/attach/github_attestations_response.json")
}

/// The bundle behind the first `bundle_url`, fetched and decompressed. **Collected**, derived —
/// see the caveat in `PROVENANCE.md` about the hand-written Snappy decoder.
fn real_resolved_bundle() -> &'static [u8] {
    include_bytes!("fixtures/attach/github_bundle_resolved.json")
}

/// An entry with `bundle: null` and whatever `bundle_url` spelling is being tested.
fn entry_with_bundle_url(bundle_url: Option<&str>) -> Vec<u8> {
    let url = bundle_url.map_or(String::new(), |u| format!(r#","bundle_url":"{u}""#));
    format!(r#"{{"attestations":[{{"repository_id":1,"bundle":null{url},"initiator":"github"}}]}}"#)
        .into_bytes()
}

/// 🔴 **Probe ① (AC-E1, AC-E2)** — the collected response says "elsewhere", not "nothing".
///
/// This is the whole lane in one assertion, and it is the probe that was **red before the repair**:
/// the reader returned `NoDsseEnvelope` for both entries. The failure it forbids is the expensive
/// kind — a caller reads "no signed envelope", concludes the artifact is unattested, and stops.
/// It is attested; the envelope is one fetch away, and the refusal now says so and hands over the
/// address to fetch.
#[test]
fn the_real_response_keeps_its_bundles_elsewhere_and_the_refusal_says_where() {
    match attach::read_github_attestations(real_response(), REAL_SUBJECT) {
        Err(Refusal::BundleExternalized { index, url }) => {
            assert_eq!(index, 0, "the first entry is the one that refused");
            assert!(
                !url.is_empty(),
                "an externalised bundle without an address is worse than an absent one: it tells \
                 the caller to go and look, and does not say where"
            );
            assert!(
                url.starts_with("https://"),
                "the address handed back must be fetchable, got {url:?}"
            );
        }
        Err(Refusal::NoDsseEnvelope { .. }) => panic!(
            "the reader said this document has no DSSE envelope. It has one -- GitHub put it \
             behind `bundle_url` rather than inline. Reporting an externalised bundle as an absent \
             one turns `go and fetch it` into `there is nothing to fetch`, which is the same \
             collapse `NotAttested` refuses to make between `FormatHasNoField` and \
             `NotReadByThisBuild` (req/969 INV-E1)"
        ),
        Err(other) => panic!("refused, but for the wrong reason: {other}"),
        Ok(_) => panic!(
            "a response whose bundles are all `null` was read as evidence; nothing in these bytes \
             carries a statement, so any row-set built from them was invented"
        ),
    }
}

/// 🔴 **Probe ② (AC-E3)** — a null bundle with no address is *absent*, not externalised.
///
/// The pair to probe ①, and the reason `BundleExternalized` is a new arm rather than a rename of
/// `NoBundle`. Collapsing these the other way would promise a caller that something is retrievable
/// when the response never named a place to retrieve it from.
#[test]
fn a_null_bundle_with_no_address_is_absent_and_not_externalised() {
    let body = entry_with_bundle_url(None);
    match attach::read_github_attestations(&body, REAL_SUBJECT) {
        Err(Refusal::NoBundle { .. }) => {}
        Err(Refusal::BundleExternalized { url, .. }) => panic!(
            "an entry naming no location was reported as externalised, pointing at {url:?}; the \
             reader invented a place for bytes that this response never located (req/969 INV-E2)"
        ),
        Err(other) => panic!("refused, but for the wrong reason: {other}"),
        Ok(_) => panic!("an entry with no bundle at all was admitted"),
    }
}

/// 🔴 **Probe ③ (AC-E4)** — an empty `bundle_url` is not an address.
///
/// The narrow case of probe ②, kept separate because it is the one a `is_some()` check waves
/// through. A present-but-empty field is the format having *said nothing*, and answering it with a
/// refusal that carries `""` sends the caller to fetch the empty string.
#[test]
fn an_empty_address_is_not_an_address() {
    let body = entry_with_bundle_url(Some(""));
    match attach::read_github_attestations(&body, REAL_SUBJECT) {
        Err(Refusal::NoBundle { .. }) => {}
        Err(Refusal::BundleExternalized { url, .. }) => panic!(
            "an empty `bundle_url` was handed back as the place to look ({url:?}). A field that is \
             present and empty has not named anything (req/969 INV-E2)"
        ),
        Err(other) => panic!("refused, but for the wrong reason: {other}"),
        Ok(_) => panic!("an entry with an empty bundle url was admitted"),
    }
}

/// 🔴 **Probe ④ (AC-E5)** — the resolved bundle projects onto the same four questions.
///
/// The other half of the interface: once the caller has fetched and decompressed what
/// `bundle_url` pointed at, it has somewhere to hand the bytes back. Without this the repair would
/// be a politer dead end — a refusal that names an address and no door to bring the answer to.
///
/// The empirical value of this probe is in the two rows that differ from the constructed fixtures:
/// this statement's `predicate` **has content** (a release predicate, not the build provenance the
/// constructed fixtures assumed), so `When` is this build's gap and not the document's silence; and
/// its first subject carries a `uri` and no `name`, which is the fallback path no constructed
/// fixture had ever taken.
#[test]
fn a_resolved_bundle_answers_the_same_four_questions() {
    let evidence =
        attach::read_resolved_bundle(real_resolved_bundle(), REAL_SUBJECT, Some("github"))
            .expect("the collected bundle is well-formed and names the requested digest");

    assert_eq!(
        evidence.rows.len(),
        Question::ALL.len(),
        "the table is not total"
    );

    // 🔴 Permanent, and still permanent on real bytes: in-toto has nowhere to say what was read.
    match evidence.answer(Question::WhatWasRead) {
        Some(AttachedAnswer::Absent {
            why: NotAttested::FormatHasNoField,
        }) => {}
        other => panic!(
            "the read-set came back as {other:?} on a real document; no in-toto field states what \
             a build read, at any granularity (req/929 L-1)"
        ),
    }

    // The subject was bound: this statement names 23 subjects and one of them is the one asked for.
    match evidence.answer(Question::WhatWasWritten) {
        Some(AttachedAnswer::Declared(d)) => assert!(
            d.claim.contains("gh_2.98.0_linux_amd64.tar.gz"),
            "the matched subject is not named in the claim: {}",
            d.claim
        ),
        other => panic!("a bound subject came back as {other:?}"),
    }

    // 🔴 The predicate has content, so the gap is ours and is releasable -- not the publisher's.
    match evidence.answer(Question::When) {
        Some(AttachedAnswer::Absent {
            why: NotAttested::NotReadByThisBuild,
        }) => {}
        other => panic!(
            "this document's predicate carries content, so declining to open it is this reader's \
             choice; reporting it as the document's silence blames the publisher for our gap. \
             Got {other:?}"
        ),
    }

    // Nothing a foreign document says becomes a measurement. Two arms, so there is no third to
    // reach; adding one stops this file compiling, which is the assertion.
    for (_, answer) in &evidence.rows {
        match answer {
            AttachedAnswer::Declared(_) | AttachedAnswer::Absent { .. } => {}
        }
    }
}

/// 🔴 **Probe ⑤ (AC-E6)** — subject binding survives the seam.
///
/// The resolved path is the one where it would be easiest to relax: the caller has already gone to
/// some trouble to fetch these bytes, so admitting them feels like the cooperative thing to do.
/// It is not. A bundle fetched on behalf of one digest still has to name that digest, or the
/// row-set asserts a link the bytes do not carry (`req/929` INV-A4, `req/969` INV-E5).
#[test]
fn a_resolved_bundle_about_another_artifact_is_still_refused() {
    let other = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    match attach::read_resolved_bundle(real_resolved_bundle(), other, Some("github")) {
        Err(Refusal::SubjectMismatch { .. }) => {}
        Err(e) => panic!("refused, but for the wrong reason: {e}"),
        Ok(_) => panic!(
            "a bundle naming 23 subjects, none of them {other}, was returned as evidence about it; \
             going to the trouble of fetching bytes does not make them about what was asked"
        ),
    }
}

/// 🔴 **Probe ⑥ (AC-E8, AC-E7)** — the refusal vocabulary stayed injective, and the absence
/// vocabulary stayed at three.
///
/// Two invariants that pull against each other, so they are pinned together. Adding
/// `BundleExternalized` had to make the *refusal* set bigger without making the *absence* set
/// bigger: transport grew a case, and what a document can fail to say did not
/// (`req/948c` INV-C3 -- wanting a fourth absence is the signal that the seam is in the wrong
/// place, and here the pressure correctly landed on `Refusal` instead).
#[test]
fn transport_grew_a_case_and_the_absence_vocabulary_did_not() {
    let cases: [(&str, Vec<u8>); 6] = [
        ("not json", b"<html>404</html>".to_vec()),
        (
            "no attestations member",
            br#"{"message":"Not Found"}"#.to_vec(),
        ),
        ("externalised", real_response().to_vec()),
        ("no bundle", entry_with_bundle_url(None)),
        (
            "no dsse envelope",
            br#"{"attestations":[{"bundle":{"mediaType":"x"}}]}"#.to_vec(),
        ),
        (
            "no payload",
            br#"{"attestations":[{"bundle":{"dsseEnvelope":{}}}]}"#.to_vec(),
        ),
    ];

    let mut sentences = Vec::new();
    for (name, bytes) in cases {
        match attach::read_github_attestations(&bytes, REAL_SUBJECT) {
            Err(refusal) => sentences.push((name, refusal.to_string())),
            Ok(_) => panic!("`{name}` was admitted"),
        }
    }
    for i in 0..sentences.len() {
        for j in (i + 1)..sentences.len() {
            assert_ne!(
                sentences[i].1, sentences[j].1,
                "`{}` and `{}` produced the same refusal sentence; six situations that need six \
                 different responses were folded into fewer",
                sentences[i].0, sentences[j].0
            );
        }
    }

    // The absence vocabulary is still exactly three. An added arm stops this matching.
    for why in [
        NotAttested::FormatHasNoField,
        NotAttested::DocumentSilent,
        NotAttested::NotReadByThisBuild,
    ] {
        match why {
            NotAttested::FormatHasNoField
            | NotAttested::DocumentSilent
            | NotAttested::NotReadByThisBuild => {}
        }
    }
}

/// **AC-E11** — the measured cost of one resolved translation, printed rather than asserted.
///
/// A threshold would fail on a slow runner and say nothing about the code; `req/929` §6 principle ②
/// asks for a declared measurement, so the number is produced and the report carries it.
#[test]
fn the_cost_of_one_resolved_translation_is_measured_and_printed() {
    let bundle = real_resolved_bundle();
    let rounds = 1_000;
    let start = std::time::Instant::now();
    for _ in 0..rounds {
        attach::read_resolved_bundle(bundle, REAL_SUBJECT, Some("github")).expect("well-formed");
    }
    let each = start.elapsed() / rounds;
    println!(
        "R-969-EXT bench: {each:?} per resolved translation over {rounds} rounds ({} byte bundle, \
         23 subjects)",
        bundle.len()
    );
}
