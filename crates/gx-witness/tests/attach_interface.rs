// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-928-ATT** — adversarial probes for the attach interface (`req/929`).
//!
//! The claim under attack is `req/929` §1-3: a foreign attestation cannot be translated without
//! loss, so the only honest translation is one that **says which loss each row suffered**. Every
//! probe here fails if a loss is silently rendered as a fact, as a `false`, or as nothing at all.
//!
//! # The fixtures are constructed, not collected
//!
//! `req/929` §8: this workspace holds **no real GitHub attestation**. These bodies are built from
//! the shape the published OpenAPI description fixes (`attestations[].bundle.dsseEnvelope`, with
//! the envelope's interior left unconstrained by that document) and from the in-toto statement
//! layout the payload carries. They are therefore a **specification-derived** floor, not an
//! empirical one, and `req/929` §8 records that difference rather than letting a green run imply
//! it was tested against a real document.

use gx_witness::attach::{self, AttachedAnswer, NotAttested, Refusal};
use gx_witness::coverage::Question;
use gx_witness::gxfile::GxKind;

/// The digest every fixture below is about, in the spelling the endpoint's path uses.
const SUBJECT: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// A statement with the given subject array, base64'd into a bundle the reader accepts.
fn body_with_subject(subject: &str, initiator: Option<&str>) -> Vec<u8> {
    let statement = format!(
        r#"{{"_type":"https://in-toto.io/Statement/v1","subject":{subject},"predicateType":"https://slsa.dev/provenance/v1","predicate":{{}}}}"#
    );
    let payload = gx_core::b64::encode(statement.as_bytes());
    let initiator = initiator.map_or(String::new(), |who| format!(r#","initiator":"{who}""#));
    format!(
        r#"{{"attestations":[{{"bundle":{{"mediaType":"application/vnd.dev.sigstore.bundle.v0.3+json","dsseEnvelope":{{"payloadType":"application/vnd.in-toto+json","payload":"{payload}","signatures":[{{"sig":"MEUCIQ"}}]}}}}{initiator}}}]}}"#
    )
    .into_bytes()
}

/// The ordinary document: one subject, matching the digest asked for.
fn good_body() -> Vec<u8> {
    body_with_subject(
        &format!(
            r#"[{{"name":"pkg.tar.gz","digest":{{"sha256":"{}"}}}}]"#,
            SUBJECT.trim_start_matches("sha256:")
        ),
        Some("octocat"),
    )
}

/// Goes through the crate's own accessor rather than reaching into `rows`, so the public read
/// surface is exercised by every probe below instead of being shipped untested beside them.
fn answer_for(evidence: &attach::AttachedEvidence, question: Question) -> &AttachedAnswer {
    evidence.answer(question).expect("every question has a row")
}

/// 🔴 **Probe ① (AC-A3)** — a document this reader cannot make sense of is a stated refusal, and
/// four different breakages are four different sentences.
///
/// The failure this forbids is the one that costs most later: returning an **empty but successful**
/// evidence set, which a caller cannot tell from "this artifact has no attestations".
#[test]
fn a_broken_document_is_refused_and_never_answers_with_an_empty_success() {
    let cases: [(&str, Vec<u8>); 4] = [
        ("empty", Vec::new()),
        ("not json", b"<html>404</html>".to_vec()),
        (
            "no attestations member",
            br#"{"message":"Not Found"}"#.to_vec(),
        ),
        (
            "no dsse envelope",
            br#"{"attestations":[{"bundle":{"mediaType":"x"}}]}"#.to_vec(),
        ),
    ];

    let mut refusals = Vec::new();
    for (name, bytes) in cases {
        match attach::read_github_attestations(&bytes, SUBJECT) {
            Ok(evidence) => panic!(
                "`{name}` was admitted with {} evidence row-set(s); a document this reader cannot \
                 read must refuse, because an empty success is indistinguishable from an artifact \
                 that genuinely carries no attestation (req/929 AC-A3)",
                evidence.len()
            ),
            Err(refusal) => refusals.push((name, refusal)),
        }
    }

    // Four breakages, four sentences: folding them would send an operator to the wrong place.
    for i in 0..refusals.len() {
        for j in (i + 1)..refusals.len() {
            assert_ne!(
                refusals[i].1.to_string(),
                refusals[j].1.to_string(),
                "`{}` and `{}` produced the same refusal sentence",
                refusals[i].0,
                refusals[j].0
            );
        }
    }
}

/// 🔴 **Probe ② (AC-A4)** — the read-set question comes back as a *format* gap.
///
/// This is the whole lane in one assertion. in-toto has no place to say what a build read, so the
/// only true answer is "the format has no field for this". Rendering it as `false`, as an empty
/// set, or as a `Declared` claim would each be a fabrication — and the last is the tempting one,
/// because `resolvedDependencies` looks like a read-set and is a declaration of intent.
#[test]
fn the_read_set_is_absent_because_the_format_has_no_field_for_it() {
    let evidence = attach::read_github_attestations(&good_body(), SUBJECT).expect("well-formed");
    let answer = answer_for(&evidence[0], Question::WhatWasRead);

    match answer {
        AttachedAnswer::Absent {
            why: NotAttested::FormatHasNoField,
        } => {}
        AttachedAnswer::Absent { why } => panic!(
            "the read-set was absent for the wrong reason ({why:?}). `FormatHasNoField` is \
             permanent and `NotReadByThisBuild` is releasable; reporting the second where the \
             first is true promises a capability that no later build can deliver (req/929 INV-A3)"
        ),
        AttachedAnswer::Declared(d) => panic!(
            "the read-set was rendered as a claim ({d:?}); no in-toto field states what a build \
             read, so this value was invented (req/929 L-1)"
        ),
    }
}

/// 🔴 **Probe ③ (AC-A5)** — a document about a different artifact is refused.
///
/// The stored subject is a *claim*, and a reader that accepts it without comparing it against what
/// was asked for would attach one artifact's provenance to another. Same shape as
/// `gxfile::Refusal::IdentityMismatch`, aimed one layer out.
#[test]
fn a_document_about_a_different_subject_is_refused() {
    let tampered = body_with_subject(
        r#"[{"name":"pkg.tar.gz","digest":{"sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}]"#,
        Some("octocat"),
    );
    match attach::read_github_attestations(&tampered, SUBJECT) {
        Err(Refusal::SubjectMismatch { .. }) => {}
        Err(other) => panic!("refused, but for the wrong reason: {other}"),
        Ok(_) => panic!(
            "a statement about a different digest was admitted; the subject a document names is a \
             claim and must be compared with the digest that was asked for (req/929 INV-A4)"
        ),
    }
}

/// 🔴 **Probe ⑦** — an explicitly empty list is a *measured zero*, not a refusal.
///
/// The pair this pins is the one probe ① is the other half of: `{"attestations":[]}` means "this
/// artifact has none, and we looked", while a body with no `attestations` member at all means "this
/// is not an attestations response". **Zero is not absence.** Folding them would either refuse a
/// perfectly good answer or manufacture one from an error page.
#[test]
fn an_explicitly_empty_list_is_a_measured_zero_and_not_a_refusal() {
    let none_but_looked = br#"{"attestations":[]}"#;
    let evidence = attach::read_github_attestations(none_but_looked, SUBJECT)
        .expect("an empty list is a valid answer: the artifact has no attestations");
    assert!(
        evidence.is_empty(),
        "an empty list must yield no evidence, not invented evidence"
    );

    // ...and the shape that is NOT this one still refuses.
    assert!(
        attach::read_github_attestations(br#"{"message":"Not Found"}"#, SUBJECT).is_err(),
        "a body with no `attestations` member is not an empty answer, it is not an answer"
    );
}

/// 🔴 **Probe ⑤** — the same hex under a different algorithm is a different subject.
///
/// This probe exists because the first version of `attach.rs` compared the hex alone, after
/// splitting `sha256:` off and discarding it. No attacker needs to find a collision when the
/// implementation has already thrown away the label that distinguishes the two.
#[test]
fn the_same_hex_under_another_algorithm_is_not_the_same_subject() {
    let confused = body_with_subject(
        &format!(
            r#"[{{"name":"pkg.tar.gz","digest":{{"sha512":"{}"}}}}]"#,
            SUBJECT.trim_start_matches("sha256:")
        ),
        Some("octocat"),
    );
    match attach::read_github_attestations(&confused, SUBJECT) {
        Err(Refusal::SubjectMismatch { .. }) => {}
        Err(other) => panic!("refused, but for the wrong reason: {other}"),
        Ok(_) => panic!(
            "a sha512 digest satisfied a request for a sha256 digest; the algorithm is part of the \
             comparison, not decoration on the front of it"
        ),
    }
}

/// 🔴 **Probe ⑧** — a document naming no subject is refused, not returned with a blank row.
///
/// The caller asked what is attested about one digest. A statement with no subject cannot be tied
/// to it, so handing back a row-set *about* that digest would assert a link the bytes do not carry.
/// An honest-looking absence inside a dishonest association is the worst of the available answers.
#[test]
fn a_document_that_cannot_be_bound_to_the_subject_is_refused() {
    for empty in ["[]", "null"] {
        let unbindable = body_with_subject(empty, Some("octocat"));
        match attach::read_github_attestations(&unbindable, SUBJECT) {
            Err(Refusal::SubjectAbsent { .. }) => {}
            Err(other) => panic!("refused, but for the wrong reason: {other}"),
            Ok(_) => panic!(
                "a statement naming no subject (`{empty}`) was returned as evidence about \
                 {SUBJECT}; nothing in the document ties it to that digest (req/929 INV-A4)"
            ),
        }
    }
}

/// 🔴 **Probe ⑨** — a predicate with content is *this build's* gap, not the document's.
///
/// The pair to probe ④. Same four questions, same row, two different absences, and the caller can
/// tell "ask a better publisher" from "wait for a better build".
#[test]
fn a_predicate_with_content_is_this_builds_gap_and_not_the_documents() {
    // The payload is base64 of the statement, so the statement is built with content and encoded.
    let statement = format!(
        r#"{{"_type":"https://in-toto.io/Statement/v1","subject":[{{"name":"pkg.tar.gz","digest":{{"sha256":"{}"}}}}],"predicateType":"https://slsa.dev/provenance/v1","predicate":{{"buildDefinition":{{"x":1}}}}}}"#,
        SUBJECT.trim_start_matches("sha256:")
    );
    let payload = gx_core::b64::encode(statement.as_bytes());
    let body = format!(
        r#"{{"attestations":[{{"bundle":{{"dsseEnvelope":{{"payload":"{payload}"}}}},"initiator":"octocat"}}]}}"#
    );
    let evidence = attach::read_github_attestations(body.as_bytes(), SUBJECT).expect("well-formed");
    match answer_for(&evidence[0], Question::When) {
        AttachedAnswer::Absent {
            why: NotAttested::NotReadByThisBuild,
        } => {}
        other => panic!(
            "the predicate carries content, so the gap is this reader's and is releasable; \
             reporting it as the document's silence blames the publisher for our own choice. \
             Got {other:?}"
        ),
    }
}

/// 🔴 **Probe ⑥** — a subject carrying several algorithms matches on any one of them.
///
/// The other half of the same bug: reading only the first digest a subject lists would refuse a
/// document that *does* name the requested digest, under a key that happened to sort second.
#[test]
fn a_subject_with_several_digests_matches_on_any_one_of_them() {
    let many = body_with_subject(
        &format!(
            r#"[{{"name":"pkg.tar.gz","digest":{{"sha1":"aa","sha256":"{}"}}}}]"#,
            SUBJECT.trim_start_matches("sha256:")
        ),
        Some("octocat"),
    );
    let evidence = attach::read_github_attestations(&many, SUBJECT)
        .expect("the requested digest is present under sha256 and must be found");
    match answer_for(&evidence[0], Question::WhatWasWritten) {
        AttachedAnswer::Declared(d) => assert!(d.claim.contains("sha256")),
        other => panic!("a matching subject came back as {other:?}"),
    }
}

/// 🔴 **Probe ④** — a document that *could* have said and did not is a different absence from a
/// format that *cannot* say.
///
/// `req/929` INV-A3: collapsing these two is how a releasable gap becomes invisible.
#[test]
fn an_empty_predicate_is_the_documents_silence_not_this_builds_refusal() {
    // `good_body()` writes `"predicate":{}` -- the document carries no build detail at all, so the
    // silence is the document's and a better publisher could fix it.
    let evidence = attach::read_github_attestations(&good_body(), SUBJECT).expect("well-formed");
    match answer_for(&evidence[0], Question::When) {
        AttachedAnswer::Absent {
            why: NotAttested::DocumentSilent,
        } => {}
        other => panic!(
            "an empty subject list produced {other:?}; in-toto *has* a subject field, so its \
             emptiness is this document's silence and not the format's gap (req/929 INV-A3)"
        ),
    }
}

/// **AC-A1** — the table is total. A question never simply fails to appear.
#[test]
fn every_question_is_answered_for_every_attestation() {
    let evidence = attach::read_github_attestations(&good_body(), SUBJECT).expect("well-formed");
    for one in &evidence {
        assert_eq!(
            one.rows.len(),
            Question::ALL.len(),
            "the table is not total"
        );
        for question in Question::ALL {
            answer_for(one, question);
        }
    }
}

/// 🔴 **AC-A2** — nothing a foreign document says can become a measurement.
///
/// The assertion is the `match` itself: [`AttachedAnswer`] has two arms, so there is no
/// `Measured` for this road to reach. If a later hand adds one, this test stops compiling — which
/// is the point. A runtime check could be satisfied by a build that simply never took the branch.
#[test]
fn a_foreign_document_can_never_produce_a_measurement() {
    let evidence = attach::read_github_attestations(&good_body(), SUBJECT).expect("well-formed");
    for one in &evidence {
        for (_, answer) in &one.rows {
            match answer {
                AttachedAnswer::Declared(_) | AttachedAnswer::Absent { .. } => {}
            }
        }
    }
}

/// 🔴 The gap list is *derived*, so a change that hides a gap has to change the table itself.
///
/// The parallel to `ReceiptCoverage::unmet`. A hand-written list of known gaps can drift away from
/// the rows it claims to summarise; this one cannot, and the probe pins that it agrees row for row.
#[test]
fn the_list_of_unanswered_questions_is_derived_from_the_rows() {
    let evidence = attach::read_github_attestations(&good_body(), SUBJECT).expect("well-formed");
    let one = &evidence[0];

    let unanswered = one.unanswered();
    let counted_by_hand = one
        .rows
        .iter()
        .filter(|(_, a)| matches!(a, AttachedAnswer::Absent { .. }))
        .count();
    assert_eq!(unanswered.len(), counted_by_hand);

    // 🔴 The lane's headline number, pinned so it cannot drift silently: of the four questions this
    // workspace judges a face on, a GitHub attestation carrying an initiator answers **two**, and
    // both only as claims. The two it cannot answer are the read-set (permanently, L-1) and the
    // clock (this document is silent).
    assert_eq!(
        unanswered
            .iter()
            .map(|(q, why)| (*q, *why))
            .collect::<Vec<_>>(),
        vec![
            (Question::WhatWasRead, NotAttested::FormatHasNoField),
            (Question::When, NotAttested::DocumentSilent),
        ],
        "the surviving/lost split changed"
    );
}

/// **AC-A6** — the three absences are three sentences, and `because()` is injective over them.
#[test]
fn the_three_absences_give_three_different_reasons() {
    let all = [
        NotAttested::FormatHasNoField,
        NotAttested::DocumentSilent,
        NotAttested::NotReadByThisBuild,
    ];
    for i in 0..all.len() {
        assert!(!all[i].because().is_empty());
        for j in (i + 1)..all.len() {
            assert_ne!(
                all[i].because(),
                all[j].because(),
                "{:?} and {:?} say the same thing",
                all[i],
                all[j]
            );
        }
    }
}

/// **AC-A7** — the `.gx` connection is a name, and naming is not shipping.
///
/// This pins that the lane stayed additive: kind 9 already existed and still does not ship, so no
/// `.gx` file changes meaning because of this module.
#[test]
fn the_kind_is_attach_source_and_naming_it_did_not_ship_it() {
    assert_eq!(attach::AttachedEvidence::GX_KIND, GxKind::AttachSource);
    assert!(
        !attach::AttachedEvidence::GX_KIND.is_shipped(),
        "this lane must not flip a registry kind to shipped (req/929 §3 item 2)"
    );
}

/// The authority row: present when the document names an initiator, and honest about the
/// certificate this build does not open when it does not.
#[test]
fn authority_is_declared_when_named_and_unread_when_only_in_the_certificate() {
    let named = attach::read_github_attestations(&good_body(), SUBJECT).expect("well-formed");
    match answer_for(&named[0], Question::ByWhoseAuthority) {
        AttachedAnswer::Declared(d) => assert!(d.claim.contains("octocat")),
        other => panic!("an initiator was present and came back as {other:?}"),
    }

    let anonymous = body_with_subject(
        &format!(
            r#"[{{"name":"pkg.tar.gz","digest":{{"sha256":"{}"}}}}]"#,
            SUBJECT.trim_start_matches("sha256:")
        ),
        None,
    );
    let anonymous = attach::read_github_attestations(&anonymous, SUBJECT).expect("well-formed");
    match answer_for(&anonymous[0], Question::ByWhoseAuthority) {
        AttachedAnswer::Absent {
            why: NotAttested::NotReadByThisBuild,
        } => {}
        other => panic!(
            "with no initiator the identity lives in the Fulcio certificate, which this build does \
             not parse; that is `NotReadByThisBuild` and not a format gap. Got {other:?}"
        ),
    }
}

/// **AC-A9** — the measured cost of one translation, printed rather than asserted.
///
/// A threshold here would be a machine-speed assertion that fails on a slow runner and says
/// nothing about the code. `req/929` §6 principle ② asks for a *declared measurement*, so the
/// number is produced and printed; the report carries it.
#[test]
fn the_cost_of_one_translation_is_measured_and_printed() {
    let body = good_body();
    let rounds = 1_000;
    let start = std::time::Instant::now();
    for _ in 0..rounds {
        let evidence = attach::read_github_attestations(&body, SUBJECT).expect("well-formed");
        assert_eq!(evidence.len(), 1);
    }
    let each = start.elapsed() / rounds;
    println!(
        "R-928-ATT bench: {each:?} per translation over {rounds} rounds ({} byte document)",
        body.len()
    );
}
