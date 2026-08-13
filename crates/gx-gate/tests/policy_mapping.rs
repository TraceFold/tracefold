//! ASM-60-1 — the table in `policy.rs`'s module doc, as assertions.
//!
//! req/60 §5.2 asks hand 2 to 「**写像を doc に 1 表で書く**(何が principal/action/resource/context に
//! なるか)」, and req/38 §19 attaches a condition to the ruling: 「**条件=手 2 着手前に Cedar 一次 doc
//! (req/60 §8-3)を fetch し「発明でなく引用」を確認**(相違があれば写像を一次に合わせて改訂)」. A table
//! nothing reads is a table that drifts, so every row of it is a case below, and the two facts the
//! primary fixes -- 「P, A, and R are entity references, while C is a record」 and that `like` matches
//! strings rather than entity references -- are what decide which side of the request each gx field
//! lands on.
//!
//! This file also holds the two things the mapping deliberately does **not** carry: the planned
//! delta (P-6) and any attribute a policy might wish for. The second one is not a gap but a
//! fail-open, and it is tested as one.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId,
};
use gx_gate::policy::{
    CONTEXT_EVIDENCE_COUNT, CONTEXT_EVIDENCE_KINDS, CONTEXT_INVERT_AVAILABLE, CONTEXT_ORDER,
    RESOURCE_ATTR_LOCATOR, RESOURCE_ATTR_SUBSTRATE,
};
use gx_gate::{ContextValue, Error, GateInput, PolicyEngine, RequestView};
use gx_witness::{Evidence, InTotoStatementRef, PolicyDecision, TestOutcome};

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn change(order: u8, context: ChangeContext, actor: Actor) -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        order,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context,
            actor,
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("orders 0..=2 are below MAX_ORDER")
}

fn agent() -> Actor {
    Actor::Agent {
        key: "key-agent-1".to_string(),
        model: "claude-fable-5".to_string(),
    }
}

fn object(substrate: SubstrateKind, locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        substrate,
        locator.to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

// ---------------------------------------------------------------------------
// The whole table, at once
// ---------------------------------------------------------------------------

#[test]
fn the_request_view_is_the_asm_60_1_table() {
    let t = change(1, ChangeContext::Policy, agent());
    let pre = object(SubstrateKind::Fs, "/etc/passwd");
    let planned = PlannedDeltaBytes(b"opaque".to_vec());
    let collected = vec![Evidence::TestResult {
        case: "c".to_string(),
        suite: "s".to_string(),
        outcome: TestOutcome::Pass,
        log_digest: None,
        duration_ms: 1,
    }];

    let view = RequestView::of(&GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: false,
    });

    assert_eq!(view.principal_id(), "key-agent-1");
    assert_eq!(view.action_id(), "Policy");
    assert_eq!(view.resource_id(), "fs:/etc/passwd");

    assert_eq!(
        view.resource_attrs()
            .get(RESOURCE_ATTR_LOCATOR)
            .map(String::as_str),
        Some("/etc/passwd")
    );
    assert_eq!(
        view.resource_attrs()
            .get(RESOURCE_ATTR_SUBSTRATE)
            .map(String::as_str),
        Some("fs")
    );
    assert_eq!(
        view.resource_attrs().len(),
        2,
        "two attributes, and the table names both"
    );

    assert_eq!(
        view.context().get(CONTEXT_ORDER),
        Some(&ContextValue::Long(1))
    );
    assert_eq!(
        view.context().get(CONTEXT_INVERT_AVAILABLE),
        Some(&ContextValue::Bool(false))
    );
    assert_eq!(
        view.context().get(CONTEXT_EVIDENCE_COUNT),
        Some(&ContextValue::Long(1))
    );
    assert_eq!(
        view.context().get(CONTEXT_EVIDENCE_KINDS),
        Some(&ContextValue::Set(
            ["TestResult".to_string()].into_iter().collect()
        ))
    );
    assert_eq!(
        view.context().len(),
        4,
        "the context holds ASM-60-1's 「残り」 and nothing invented beside it"
    );
}

// ---------------------------------------------------------------------------
// principal
// ---------------------------------------------------------------------------

/// The key, whichever `Actor` variant carries it (FR-006, P-7).
///
/// Putting the variant into the entity type -- `GxHuman::` / `GxAgent::` -- would make every policy
/// scope branch on it, and 42 §3.2 already made `KeyId` the identity that crosses the DSSE
/// boundary. What that costs is that a policy cannot ask 「was this an agent?」, which is raised in
/// `req/62_M3_HAND2_REPORT_2026-08-08.md` §4 rather than fixed by inventing a row.
#[test]
fn the_principal_is_the_actor_key_whichever_variant_it_is() {
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());

    let actors = [
        Actor::Human {
            key: "one-key".to_string(),
        },
        Actor::Agent {
            key: "one-key".to_string(),
            model: "some-model".to_string(),
        },
        Actor::Process {
            key: "one-key".to_string(),
        },
    ];

    for actor in actors {
        let t = change(0, ChangeContext::Substrate, actor);
        let view = RequestView::of(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        });
        assert_eq!(view.principal_id(), "one-key");
    }
}

// ---------------------------------------------------------------------------
// action
// ---------------------------------------------------------------------------

/// Seven `ChangeContext` shapes, seven action ids, all distinct.
///
/// `Custom` keeps the enumeration open (41 §3), so the id is prefixed rather than passed through:
/// a custom context spelled `"Policy"` must not collide with the variant `Policy`.
#[test]
fn the_action_is_the_change_context() {
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());

    let cases = [
        (ChangeContext::Time, "Time"),
        (ChangeContext::Evidence, "Evidence"),
        (ChangeContext::Policy, "Policy"),
        (ChangeContext::Model, "Model"),
        (ChangeContext::Representation, "Representation"),
        (ChangeContext::Substrate, "Substrate"),
        (ChangeContext::Custom("Policy".to_string()), "Custom:Policy"),
    ];

    let mut seen = std::collections::BTreeSet::new();
    for (context, expected) in cases {
        let t = change(0, context, agent());
        let view = RequestView::of(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        });
        assert_eq!(view.action_id(), expected);
        assert!(seen.insert(view.action_id().to_string()), "ids collide");
    }
    assert_eq!(seen.len(), 7);
}

// ---------------------------------------------------------------------------
// resource
// ---------------------------------------------------------------------------

#[test]
fn the_resource_carries_the_substrate_and_the_locator() {
    let planned = PlannedDeltaBytes(Vec::new());
    let t = change(0, ChangeContext::Substrate, agent());

    let cases = [
        (SubstrateKind::Fs, "fs"),
        (SubstrateKind::Git, "git"),
        (SubstrateKind::Mcp, "mcp"),
        (SubstrateKind::Custom("s3".to_string()), "custom:s3"),
    ];

    for (substrate, tag) in cases {
        let pre = object(substrate, "/tmp/x");
        let view = RequestView::of(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        });
        assert_eq!(view.resource_id(), format!("{tag}:/tmp/x"));
        assert_eq!(
            view.resource_attrs()
                .get(RESOURCE_ATTR_SUBSTRATE)
                .map(String::as_str),
            Some(tag)
        );
    }
}

/// A locator that reads like Cedar syntax names itself and nothing else.
///
/// The entity uid is built with `EntityUid::from_type_name_and_id`, so the id is taken as a value.
/// Building it by formatting `GxResource::"<locator>"` and parsing that back would let a locator
/// containing a quote name a different entity -- an injection with a policy engine at the far end.
/// The forbid still fires, because the locator really is under `/etc`.
#[test]
fn a_locator_that_reads_like_cedar_syntax_cannot_name_another_entity() {
    let hostile = "/etc/x\", GxResource::\"/tmp/safe";
    let t = change(0, ChangeContext::Substrate, agent());
    let pre = object(SubstrateKind::Fs, hostile);
    let planned = PlannedDeltaBytes(Vec::new());
    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    };

    let view = RequestView::of(&input);
    assert_eq!(view.resource_id(), format!("fs:{hostile}"));
    assert_eq!(
        view.resource_attrs()
            .get(RESOURCE_ATTR_LOCATOR)
            .map(String::as_str),
        Some(hostile),
        "the attribute carries the locator byte for byte"
    );

    let outcome = pack().evaluate(&input).expect("evaluable");
    assert_eq!(
        outcome.decision(),
        PolicyDecision::Deny,
        "the string is still under /etc, and the pattern still matches it"
    );
}

// ---------------------------------------------------------------------------
// context
// ---------------------------------------------------------------------------

#[test]
fn the_context_carries_the_order_the_inverse_flag_and_the_evidence_summary() {
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());

    for order in [0u8, 1, 2] {
        let t = change(order, ChangeContext::Substrate, agent());
        let view = RequestView::of(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: order == 1,
        });
        assert_eq!(
            view.context().get(CONTEXT_ORDER),
            Some(&ContextValue::Long(i64::from(order))),
            "AC-029's order-2 case is reachable from a policy as context.order == 2"
        );
        assert_eq!(
            view.context().get(CONTEXT_INVERT_AVAILABLE),
            Some(&ContextValue::Bool(order == 1))
        );
    }
}

/// The four names 42 §3.7 gives evidence, and the count beside them.
#[test]
fn the_evidence_summary_is_the_four_kinds_of_42_3_7() {
    let t = change(0, ChangeContext::Evidence, agent());
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());
    let collected = vec![
        Evidence::TestResult {
            case: "c".to_string(),
            suite: "s".to_string(),
            outcome: TestOutcome::Fail,
            log_digest: None,
            duration_ms: 1,
        },
        Evidence::Measurement {
            subject: Subject::Object(ObjectId(cid(2))),
            measure_id: "byte-count".to_string(),
            value_digest: cid(7),
        },
        Evidence::ExternalAttestation {
            signer: "someone-else".to_string(),
            statement: InTotoStatementRef {
                uri: None,
                digest: cid(8),
                inline: None,
            },
            predicate_type: "https://slsa.dev/provenance/v1".to_string(),
        },
        Evidence::PolicyEvaluation {
            decision: PolicyDecision::Allow,
            policy_id: "some-other-engine".to_string(),
            explanation_digest: None,
        },
    ];

    let view = RequestView::of(&GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
    });

    assert_eq!(
        view.context().get(CONTEXT_EVIDENCE_COUNT),
        Some(&ContextValue::Long(4))
    );
    assert_eq!(
        view.context().get(CONTEXT_EVIDENCE_KINDS),
        Some(&ContextValue::Set(
            [
                "ExternalAttestation".to_string(),
                "Measurement".to_string(),
                "PolicyEvaluation".to_string(),
                "TestResult".to_string(),
            ]
            .into_iter()
            .collect()
        )),
        "the summary is which kinds were shown; the bodies stay in the evidence store (42 §5)"
    );
}

// ---------------------------------------------------------------------------
// What the mapping does not carry
// ---------------------------------------------------------------------------

/// P-6: the payload reaches no part of the request.
///
/// 42 §3.4: 「`payload` ｜ adapter のみが解釈する **opaque な変更記述。core/gate/witness は byte 列と
/// してのみ扱う(P-6)**」, and req/38 §19's M3-10 requires the consequence to be stated rather than
/// discovered: 「v0.1 pack の実効範囲=**locator/actor/context/order 級と明記**(overclaim 禁)」. Two
/// inputs differing only in what the change *does* are one request.
#[test]
fn the_planned_payload_reaches_no_part_of_the_request() {
    let t = change(0, ChangeContext::Substrate, agent());
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let harmless = PlannedDeltaBytes(b"append one line".to_vec());
    let ruinous = PlannedDeltaBytes(b"truncate the file to zero bytes".to_vec());

    let a = RequestView::of(&GateInput {
        t: &t,
        pre: &pre,
        planned: &harmless,
        evidence: &[],
        invert_available: true,
    });
    let b = RequestView::of(&GateInput {
        t: &t,
        pre: &pre,
        planned: &ruinous,
        evidence: &[],
        invert_available: true,
    });

    assert_eq!(a, b, "a policy cannot see what the change does (P-6)");
}

/// A policy that reads an attribute the mapping does not supply is an error, not a decision.
///
/// This is the fail-open Cedar's own default would produce. The primary: 「skip on error: if a
/// policy's evaluation returns error, the policy does not factor into the authorization response;
/// it is skipped」, and in the same paragraph the escape hatch gx takes -- 「applications can always
/// choose to look at the authorization response's diagnostics and take a different decision if an
/// evaluated policy produces errors」. Left alone, a forbid with a typo in an attribute name is
/// silently skipped and the response is an ordinary-looking Allow. **E-M3-3** makes it `Err`.
#[test]
fn a_policy_reading_an_attribute_the_mapping_does_not_supply_is_not_a_decision() {
    let engine = PolicyEngine::parse(
        r#"
        @id("permits-fs")
        permit (principal, action, resource) when { resource.substrate == "fs" };

        @id("reads-something-nobody-supplies")
        forbid (principal, action, resource) when { resource.classification == "secret" };
        "#,
    )
    .expect("both statements parse");

    let t = change(0, ChangeContext::Substrate, agent());
    let pre = object(SubstrateKind::Fs, "/etc/passwd");
    let planned = PlannedDeltaBytes(Vec::new());
    let outcome = engine.evaluate(&GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    });

    match outcome {
        Err(Error::Unevaluable {
            transformation,
            detail,
        }) => {
            assert_eq!(transformation, t.id);
            assert!(
                detail.contains("policy evaluation error"),
                "the failure says what happened, not just that something did: {detail}"
            );
        }
        other => panic!(
            "a skipped forbid must not read as a clean decision, and this one answered {other:?}"
        ),
    }
}

fn pack() -> PolicyEngine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-gate")
        .join("policies")
        .join("fs")
        .join("deny-etc.cedar");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("AC-025 names {}: {e}", path.display()));
    PolicyEngine::parse(&src).expect("the pack parses")
}
