// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-025 (FR-025) — the two cases, decided by the pack the criterion names by path.
//!
//! AC-025 verbatim: "Given: a Cedar PolicySet (`policies/fs/deny-etc.cedar`: refuses writes to
//! `/etc/**`). When: a write Transformation to `/etc/passwd` is evaluated. Then: the Cedar
//! evaluation result maps to the Deny equivalent. `/tmp/x` maps to the Admit equivalent." Judgment
//! method: "unit (2 cases)". (sem: SEM-gx-gate-190)
//!
//! "The Deny equivalent" is read at both ends. The narrow reading is the one the criterion states --
//! Cedar's decision arrives as `PolicyDecision::Deny`, the vocabulary 42 §3.7 already fixed as "the
//! same vocabulary as `cedar_policy::Decision`" (sem: SEM-gx-gate-191) -- and the wider one is that the same evaluation, taken through
//! `Gate::verify`, comes out of the arm of `Verdict` that matches. Both are asserted, because the
//! first without the second would leave the mapping true and unreachable.
//!
//! What is **not** asserted here is that Cedar decides correctly. req/60 §7.2: "a gx test does not
//! re-implement Cedar's decision. What is measured is ... 'gx's mapping built this request, and its
//! decision mapped onto this branch of the Verdict'" (sem: SEM-gx-gate-192). So the expectations below are about which policy id answered and which arm was taken --
//! facts about the mapping -- and never about Cedar's algorithm, which 46 §1 puts outside the
//! project's scope.

use std::fs;
use std::path::{Path, PathBuf};

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::POLICY_DENIED;
use gx_gate::{Gate, GateInput, PolicyEngine, ReasonSource, Verdict};
use gx_witness::{Evidence, PolicyDecision, TestOutcome};

/// The path AC-025 writes, resolved from this crate rather than from the working directory.
///
/// M3-16's ruling put the directory at the repository root -- "`policies/` is **directly under
/// root** (AC-025's verbatim path takes priority; shipped artifacts are not placed under req/)"
/// (sem: SEM-gx-gate-193) -- so this is `<root>/policies/fs/deny-etc.cedar` and
/// the test fails loudly if the file moves. `tools/e2e.sh` clones the repository and runs the same
/// suite, so a pack that git does not hold cannot pass here either.
fn pack_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-gate")
        .join("policies")
        .join("fs")
        .join("deny-etc.cedar")
}

fn pack() -> PolicyEngine {
    let src = fs::read_to_string(pack_path())
        .unwrap_or_else(|e| panic!("AC-025 names {}: {e}", pack_path().display()));
    PolicyEngine::parse(&src).expect("the pack AC-025 names must parse")
}

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// One change, by an agent, at order 0. The criterion says "a write" and gx has no other kind: (sem: SEM-gx-gate-194)
/// every `Transformation` is a change to the thing it names (P-1).
fn change() -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        0,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: ChangeContext::Substrate,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 0 is below MAX_ORDER")
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

fn evidence() -> Vec<Evidence> {
    vec![Evidence::TestResult {
        case: "the file still parses".to_string(),
        suite: "unit".to_string(),
        outcome: TestOutcome::Pass,
        log_digest: None,
        duration_ms: 4,
    }]
}

// ---------------------------------------------------------------------------
// Case 1: /etc/passwd
// ---------------------------------------------------------------------------

#[test]
fn ac_025_a_change_to_etc_passwd_is_denied() {
    let engine = pack();
    let t = change();
    let pre = object(SubstrateKind::Fs, "/etc/passwd");
    let planned = PlannedDeltaBytes(b"whatever this is, nobody here may read it".to_vec());
    let collected = evidence();
    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
        // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
        // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
        // varying this field alone moves no verdict) about the field and not about this fixture.
        decided_at: Timestamp(0),
    };

    let outcome = engine
        .evaluate(&input)
        .expect("a well-formed request against a well-formed pack is evaluable");

    assert_eq!(
        outcome.decision(),
        PolicyDecision::Deny,
        "Cedar's decision arrives in 42 §3.7's vocabulary"
    );
    let ids: Vec<&str> = outcome
        .determining()
        .iter()
        .map(|record| record.policy_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["fs-deny-etc"],
        "the forbid policy is the one that answered, by the id its @id annotation gives it"
    );
    assert!(
        outcome
            .determining()
            .iter()
            .all(|record| record.decision == PolicyDecision::Deny),
        "a determining policy on a Deny is a forbid policy that was satisfied"
    );

    // The same evaluation through the entry point 41 §4 declares.
    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
        decided_at: Timestamp(0),
    };
    let verdict = Gate::with_policies(pack())
        .verify(input)
        .expect("evaluable, therefore a verdict rather than an error");
    assert_eq!(verdict.kind(), VerdictKind::Deny);
    let Verdict::Deny(reasons) = verdict else {
        panic!("the Deny arm is the one a denial takes")
    };
    assert_eq!(reasons.len(), 1, "one deciding policy, one reason");
    assert_eq!(reasons[0].code(), POLICY_DENIED);
    assert_eq!(
        *reasons[0].source(),
        ReasonSource::Policy {
            policy_id: "fs-deny-etc".to_string()
        },
        "42 §3.8's ReasonSource names the policy that refused"
    );
}

// ---------------------------------------------------------------------------
// Case 2: /tmp/x
// ---------------------------------------------------------------------------

#[test]
fn ac_025_a_change_to_tmp_x_is_admitted() {
    let engine = pack();
    let t = change();
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(b"opaque".to_vec());
    let collected = evidence();
    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
        decided_at: Timestamp(0),
    };

    let outcome = engine.evaluate(&input).expect("evaluable");
    assert_eq!(outcome.decision(), PolicyDecision::Allow);
    let ids: Vec<&str> = outcome
        .determining()
        .iter()
        .map(|record| record.policy_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["fs-permit-default"],
        "on an Allow the determining policies are the satisfied permits"
    );

    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &collected,
        invert_available: true,
        decided_at: Timestamp(0),
    };
    let verdict = Gate::with_policies(pack())
        .verify(input)
        .expect("evaluable");
    assert_eq!(verdict.kind(), VerdictKind::Admit);
    let Verdict::Admit(proof) = verdict else {
        panic!("the Admit arm")
    };
    assert_eq!(
        proof
            .policy_decisions()
            .iter()
            .map(|record| record.policy_id.as_str())
            .collect::<Vec<&str>>(),
        vec!["fs-permit-default"],
        "the proof records which policy admitted it (42 §3.8)"
    );
    assert!(
        proof.invariant_results().is_empty(),
        "this gate was built with no invariants, and an empty vector is the honest record of that"
    );
    assert!(
        proof.evidence_digests().is_empty(),
        "minting a Cid goes through gx-canon, which this crate does not name yet"
    );
    assert!(
        proof.composed_from().is_empty(),
        "nothing was composed (E-M3-5: a composed verdict is inherited, never re-derived)"
    );
}

// ---------------------------------------------------------------------------
// The pack itself
// ---------------------------------------------------------------------------

/// The two statements are named, and named by `@id` rather than by position.
///
/// `PolicySet::from_str` would call them `policy0` and `policy1` (docs.rs, cedar-policy 4.12.0), and
/// those ids reach a receipt's CID through `AdmitProof`'s IdentityView (42 §1.3). Swapping the two
/// statements in the file must not change what a receipt says.
#[test]
fn the_pack_names_its_policies_by_annotation() {
    assert_eq!(
        pack().policy_ids(),
        vec!["fs-deny-etc".to_string(), "fs-permit-default".to_string()]
    );
    assert_eq!(pack().len(), 2);
}

/// A substrate this pack does not speak for reaches Cedar's third rule.
///
/// "Otherwise (i.e., no policy is satisfied), the final result is Deny" and "the list of
/// determining policies is empty" (docs.cedarpolicy.com/auth/authorization.html) (sem: SEM-gx-gate-195). The fs pack scopes
/// both of its statements to `resource.substrate == "fs"`, so a git transformation evaluated against
/// it alone is refused by nothing in particular -- which is the safe answer, and the case 42 §3.8's
/// four `ReasonSource` variants could not name. **E-M3-11** gave it one, and hand 2's sentinel
/// policy id is gone: this is now the same kind of statement as every other reason rather than a
/// string a pack could collide with.
#[test]
fn a_substrate_the_pack_does_not_speak_for_is_denied_by_no_policy() {
    let t = change();
    let pre = object(SubstrateKind::Git, "refs/heads/main");
    let planned = PlannedDeltaBytes(Vec::new());
    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: false,
        decided_at: Timestamp(0),
    };

    let outcome = pack().evaluate(&input).expect("evaluable");
    assert_eq!(outcome.decision(), PolicyDecision::Deny);
    assert!(
        outcome.determining().is_empty(),
        "rule 3 leaves the determining list empty"
    );

    let reasons = outcome
        .deny_reasons()
        .expect("POLICY_DENIED is in the vocabulary");
    assert_eq!(reasons.len(), 1);
    assert_eq!(
        *reasons[0].source(),
        ReasonSource::NoPolicyApplied,
        "E-M3-11: the absence of a deciding policy is a variant, not a policy id"
    );
    assert_eq!(reasons[0].code(), POLICY_DENIED);
}

/// A policy may now be called `cedar:default-deny`, and it still reports as itself (**E-M3-11**).
///
/// Hand 2 refused that name at parse time, because it was the string it used to mean "no policy
/// decided" and a pack claiming it could have put a false `Policy{..}` row in a receipt. The
/// variant removed the collision, so the refusal was removed with it (req/38 §21 C-3: "the sentinel
/// spelling is deleted at the same time") (sem: SEM-gx-gate-196). What must stay true is that the two cases are still told apart, and that is
/// what this measures: the once-forbidden name decides a request as an ordinary policy, and its
/// reason names it -- while the request no policy answers gets `NoPolicyApplied` above.
#[test]
fn the_name_hand_2_reserved_is_now_an_ordinary_policy_id() {
    let engine = PolicyEngine::parse(
        r#"@id("cedar:default-deny")
        forbid (principal, action, resource);
        "#,
    )
    .expect("nothing reserves this name any more");

    let t = change();
    let pre = object(SubstrateKind::Fs, "/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());
    let outcome = engine
        .evaluate(&GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
            decided_at: Timestamp(0),
        })
        .expect("evaluable");

    assert_eq!(outcome.decision(), PolicyDecision::Deny);
    let reasons = outcome.deny_reasons().expect("in the vocabulary");
    assert_eq!(
        *reasons[0].source(),
        ReasonSource::Policy {
            policy_id: "cedar:default-deny".to_string()
        },
        "a policy that decided is recorded as a policy, whatever it is called"
    );
}
