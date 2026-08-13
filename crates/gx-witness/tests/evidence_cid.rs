//! `Evidence`: four variants (E-M2-3), and a CID that only gx-canon can mint (41 §6).
//!
//! No AC is claimed here. AC-016 is the type-check and lives in `ac_016.rs`; this file holds the
//! other half of req/49 §5's hand-4 DoD — 「Evidence 全 variant の CID が canonical」 — plus the
//! shape of the enum itself.
//!
//! # Four variants, and the ruling that fixed the number
//!
//! **E-M2-3** (`req/38_ERRATA_2026-08-07.md` §8) 逐語: 「Evidence=42 の 4 variant が正(43/44/34/35 の
//! `HumanDecision` 参照は erratum・DR-03-1 の HumanApprovalToken が対応物)」. req/49 §3 M2-4 raised
//! the count as a conflict and proposed five; the ruling went the other way and the ruling is what
//! binds. So a human decision is **not** an `Evidence` in gx: 43 T-5's 「人間裁定receipt」 is a
//! receipt, and DR-03-1's `HumanApprovalToken` is the type that carries the approval. Reading
//! req/49 §3 M2-4's 「5 variant で実装し」 as the instruction would implement a proposal the Owner
//! declined.
//!
//! # 42 §1.3: every field is in the identity
//!
//! The IdentityView table gives `Evidence`（各variant）「全フィールド」 with 除外なし — an evidence
//! item is an independent piece of evidence, so nothing about it is metadata. The projection is
//! therefore the value itself, and the CID is `gx_canon::cid::compute`'s: projection → canonical
//! DAG-CBOR → BLAKE3, with no second road (AC-014's 迂回禁止 holds for this crate's types as it
//! does for gx-core's).

mod support;

use gx_canon::{cbor, cid as canon_cid, Error};
use gx_core::Subject;
use gx_witness::evidence::{Evidence, InTotoStatementRef, PolicyDecision, TestOutcome};
use support::{cid, oid, one_of_each_evidence, tid};

// ---------------------------------------------------------------------------
// The enum's shape (E-M2-3)
// ---------------------------------------------------------------------------

/// Four, and exactly the four 42 §3.7 names. The `match` is what does the work: adding a fifth
/// variant makes this file stop compiling, so the count cannot drift without somebody editing the
/// ruling into the test.
#[test]
fn evidence_has_the_four_variants_42_3_7_declares() {
    let all = one_of_each_evidence();
    assert_eq!(all.len(), 4);

    for e in &all {
        match e {
            Evidence::TestResult { .. }
            | Evidence::Measurement { .. }
            | Evidence::ExternalAttestation { .. }
            | Evidence::PolicyEvaluation { .. } => {}
        }
    }

    // The four are distinct kinds, not four values of one kind.
    let names: Vec<&str> = all
        .iter()
        .map(|e| match e {
            Evidence::TestResult { .. } => "TestResult",
            Evidence::Measurement { .. } => "Measurement",
            Evidence::ExternalAttestation { .. } => "ExternalAttestation",
            Evidence::PolicyEvaluation { .. } => "PolicyEvaluation",
        })
        .collect();
    assert_eq!(
        names,
        vec![
            "TestResult",
            "Measurement",
            "ExternalAttestation",
            "PolicyEvaluation"
        ]
    );
}

/// 42 §3.7 逐語: `TestOutcome` = `Pass | Fail | Skip | Error`, `PolicyDecision` = `Allow | Deny`
/// （「`cedar_policy::Decision`と同一語彙」）.
#[test]
fn the_two_auxiliary_enumerations_carry_42_3_7s_vocabularies() {
    let outcomes = [
        TestOutcome::Pass,
        TestOutcome::Fail,
        TestOutcome::Skip,
        TestOutcome::Error,
    ];
    assert_eq!(outcomes.len(), 4);
    let mut spellings: Vec<String> = outcomes
        .iter()
        .map(|o| serde_json::to_string(o).expect("serialises"))
        .collect();
    spellings.sort();
    spellings.dedup();
    assert_eq!(spellings.len(), 4, "two outcomes share a spelling");

    let decisions = [PolicyDecision::Allow, PolicyDecision::Deny];
    assert_eq!(
        serde_json::to_string(&decisions[0]).expect("serialises"),
        "\"Allow\""
    );
    assert_eq!(
        serde_json::to_string(&decisions[1]).expect("serialises"),
        "\"Deny\""
    );
}

/// 42 §3.7: 「**測定値自体（`f64`）はEvidence CIDに直接埋め込まない**…ここはそのdigestのみを保持
/// する（P-10）」. The type is what enforces it: `Measurement` has a `value_digest: Cid` and no
/// numeric field at all, so 42 §2.1-4's float ban has nothing to catch here.
#[test]
fn a_measurement_carries_a_digest_and_never_a_number() {
    let m = Evidence::Measurement {
        subject: Subject::Transformation(tid(1)),
        measure_id: "lyapunov/entropy".to_string(),
        value_digest: cid(3),
    };
    let bytes = cbor::encode(&m).expect("a measurement has a canonical form");
    assert!(cbor::is_canonical(&bytes));
}

// ---------------------------------------------------------------------------
// The identity (42 §1.3, 41 §6)
// ---------------------------------------------------------------------------

/// Every variant has a CID, and it comes out of gx-canon.
#[test]
fn every_variant_has_a_cid_and_the_bytes_behind_it_are_canonical() {
    for e in one_of_each_evidence() {
        let digest = canon_cid::compute(&e).expect("every variant has an identity");
        assert_eq!(
            digest,
            canon_cid::compute(&e).expect("twice"),
            "the same value hashed to two digests"
        );

        // 42 §1.3 gives this type 全フィールド with 除外なし, so the projection is the value and the
        // bytes hashed are the bytes `encode` writes. Checking them canonical is checking that the
        // projection did not smuggle in a form the encoder would refuse.
        let bytes = cbor::encode(&e).expect("canonical form");
        assert!(
            cbor::is_canonical(&bytes),
            "identity bytes are not canonical"
        );
        assert_eq!(
            cbor::decode::<Evidence>(&bytes).expect("strict decode"),
            e,
            "the round trip lost a field"
        );
    }
}

/// The four digests are four digests.
#[test]
fn the_four_variants_do_not_collide() {
    let mut digests: Vec<_> = one_of_each_evidence()
        .iter()
        .map(|e| canon_cid::compute(e).expect("identity"))
        .collect();
    let before = digests.len();
    digests.sort();
    digests.dedup();
    assert_eq!(digests.len(), before, "two variants share a CID");
}

/// 「全フィールド・除外なし」 stated as something a machine can fail: change any one field of any
/// variant and the identity moves. A field silently left out of the projection would show up here
/// as two values sharing a digest.
#[test]
fn every_field_of_every_variant_reaches_the_identity() {
    let mut family: Vec<Evidence> = Vec::new();

    // TestResult -- five fields, one alternative each.
    let base_test = || Evidence::TestResult {
        case: "a".to_string(),
        suite: "s".to_string(),
        outcome: TestOutcome::Pass,
        log_digest: Some(cid(1)),
        duration_ms: 1,
    };
    family.push(base_test());
    for alt in [
        Evidence::TestResult {
            case: "b".to_string(),
            suite: "s".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: Some(cid(1)),
            duration_ms: 1,
        },
        Evidence::TestResult {
            case: "a".to_string(),
            suite: "t".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: Some(cid(1)),
            duration_ms: 1,
        },
        Evidence::TestResult {
            case: "a".to_string(),
            suite: "s".to_string(),
            outcome: TestOutcome::Fail,
            log_digest: Some(cid(1)),
            duration_ms: 1,
        },
        Evidence::TestResult {
            case: "a".to_string(),
            suite: "s".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: Some(cid(2)),
            duration_ms: 1,
        },
        // `None` and `Some` are different facts about whether a log was kept (42 §5).
        Evidence::TestResult {
            case: "a".to_string(),
            suite: "s".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: None,
            duration_ms: 1,
        },
        Evidence::TestResult {
            case: "a".to_string(),
            suite: "s".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: Some(cid(1)),
            duration_ms: 2,
        },
    ] {
        family.push(alt);
    }

    // Measurement -- three fields.
    family.push(Evidence::Measurement {
        subject: Subject::Object(oid(1)),
        measure_id: "m".to_string(),
        value_digest: cid(1),
    });
    family.push(Evidence::Measurement {
        subject: Subject::Transformation(tid(1)),
        measure_id: "m".to_string(),
        value_digest: cid(1),
    });
    family.push(Evidence::Measurement {
        subject: Subject::Object(oid(1)),
        measure_id: "n".to_string(),
        value_digest: cid(1),
    });
    family.push(Evidence::Measurement {
        subject: Subject::Object(oid(1)),
        measure_id: "m".to_string(),
        value_digest: cid(2),
    });

    // ExternalAttestation -- three fields, and three inside `InTotoStatementRef`.
    let statement =
        |uri: Option<&str>, digest: u64, inline: Option<serde_json::Value>| InTotoStatementRef {
            uri: uri.map(str::to_string),
            digest: cid(digest),
            inline,
        };
    family.push(Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: statement(Some("u"), 1, None),
        predicate_type: "p".to_string(),
    });
    family.push(Evidence::ExternalAttestation {
        signer: "l".to_string(),
        statement: statement(Some("u"), 1, None),
        predicate_type: "p".to_string(),
    });
    family.push(Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: statement(Some("v"), 1, None),
        predicate_type: "p".to_string(),
    });
    family.push(Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: statement(None, 1, None),
        predicate_type: "p".to_string(),
    });
    family.push(Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: statement(Some("u"), 2, None),
        predicate_type: "p".to_string(),
    });
    family.push(Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: statement(Some("u"), 1, Some(serde_json::json!({"_type": "x"}))),
        predicate_type: "p".to_string(),
    });
    family.push(Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: statement(Some("u"), 1, None),
        predicate_type: "q".to_string(),
    });

    // PolicyEvaluation -- three fields.
    family.push(Evidence::PolicyEvaluation {
        decision: PolicyDecision::Allow,
        policy_id: "p".to_string(),
        explanation_digest: Some(cid(1)),
    });
    family.push(Evidence::PolicyEvaluation {
        decision: PolicyDecision::Deny,
        policy_id: "p".to_string(),
        explanation_digest: Some(cid(1)),
    });
    family.push(Evidence::PolicyEvaluation {
        decision: PolicyDecision::Allow,
        policy_id: "q".to_string(),
        explanation_digest: Some(cid(1)),
    });
    family.push(Evidence::PolicyEvaluation {
        decision: PolicyDecision::Allow,
        policy_id: "p".to_string(),
        explanation_digest: None,
    });

    let count = family.len();
    assert_eq!(count, 22, "the family lost a case");
    let mut digests: Vec<_> = family
        .iter()
        .map(|e| canon_cid::compute(e).expect("identity"))
        .collect();
    digests.sort();
    digests.dedup();
    assert_eq!(
        digests.len(),
        count,
        "two values that differ in one field share a CID"
    );
}

// ---------------------------------------------------------------------------
// M2-13 -- the inline in-toto statement and the float ban
// ---------------------------------------------------------------------------

/// req/49 §3 M2-13: `InTotoStatementRef.inline` is `Option<serde_json::Value>` (42 §3.7) and every
/// field is in the identity (42 §1.3), while 42 §2.1-4 keeps floats out of canonical values. A
/// genuine in-toto Statement containing a number written with a decimal point therefore **has no
/// CID**.
///
/// This hand does not rule on it — the two 既定案 (fold `inline` to digest-only, or state the
/// admitted numeric range) both change 42 §3.7's field table, which is not an implementation's to
/// change (52 契約). What it does is make the refusal loud and measured rather than latent:
/// `Error::NotCanonicalizable` naming `FloatNotAllowed`, which is req/26 §3's 「部分実装は範囲明示 +
/// throw で正直に落ちる」. Raised as H4-4 in req/53 §4.
#[test]
fn an_inline_statement_holding_a_float_has_no_identity_and_says_which_rule_refused_it() {
    let e = Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: InTotoStatementRef {
            uri: None,
            digest: cid(1),
            inline: Some(serde_json::json!({"predicate": {"score": 0.5}})),
        },
        predicate_type: "https://slsa.dev/provenance/v1".to_string(),
    };

    let refused = canon_cid::compute(&e).expect_err("a float has no canonical DAG-CBOR form");
    match &refused {
        Error::NotCanonicalizable(inner) => assert!(
            matches!(**inner, Error::FloatNotAllowed { .. }),
            "the refusal was {inner:?}, not the float clause of 42 §2.1-4"
        ),
        other => panic!("expected NotCanonicalizable, got {other:?}"),
    }
}

/// The admitted half of the same range, so the refusal above is a boundary and not a blanket ban on
/// inline statements. Integers, strings, arrays, objects and null all reach a CID.
#[test]
fn an_inline_statement_of_integers_and_strings_does_have_an_identity() {
    let e = Evidence::ExternalAttestation {
        signer: "k".to_string(),
        statement: InTotoStatementRef {
            uri: None,
            digest: cid(1),
            inline: Some(serde_json::json!({
                "_type": "https://in-toto.io/Statement/v1",
                "subject": [{"name": "a", "digest": {"gx1": "00"}}],
                "predicate": {"builder": {"id": "gx"}, "runs": 3, "flaky": false, "note": null}
            })),
        },
        predicate_type: "https://slsa.dev/provenance/v1".to_string(),
    };
    assert!(canon_cid::compute(&e).is_ok());
}

// ---------------------------------------------------------------------------
// 41 §6 -- one road to a canonical form, and where that is checked
// ---------------------------------------------------------------------------
//
// 41 §6 逐語: 「全 canonical encode は gx-canon 経由のみ」. There is **no test for it in this file**,
// and the omission is deliberate rather than a gap: `gx-canon/tests/ac_014.rs` already scans every
// `.rs` file under `crates/` and `probes/` outside gx-canon for a codec or a hash name, so a copy
// here would be a second answer to one question, and the copy is weaker (this crate only, and only
// `src/`).
//
// It is also, measurably, a *live* check over this crate. A first draft of this file carried that
// duplicate test, and its own list of banned names made `ac_014` fail with
// `crates/gx-witness/tests/evidence_cid.rs:394` in the violation list — which is the proof that
// AC-014's scan reaches gx-witness. `tools/verify_m2h4.sh` §5 records the grep as a number for the
// report; the gate is AC-014.
