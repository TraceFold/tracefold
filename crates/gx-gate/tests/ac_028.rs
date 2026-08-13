//! AC-028 (FR-028) — the shipped fs pack, and the cases a deployment can check it against.
//!
//! AC-028 逐語: 「Given: fs substrate向け出荷policy pack(`policies/fs/`)。When: packのconformance
//! テストを実行。Then: packが最低1 Admitケース・1 Denyケースを持ち両方pass。git/MCP向けpackはadapter
//! 実装に合わせM7で提供され、対応するconformanceテストはAC-074(M7)で検証する。」判定方法:
//! 「integration」.
//!
//! # What a conformance case is here
//!
//! One request, decided end to end: the pack **as the build embeds it** ([`gx_gate::packs::fs_pack`]),
//! a `Gate` built from it, and `Gate::verify` — not `PolicyEngine::evaluate`. A case that stopped at
//! the Cedar decision would leave the half AC-028 is actually about untested, since what a deployment
//! installs a pack to get is a verdict.
//!
//! Every case fixes `invert_available = true`. The third arm of the fold is **E-M3-4**'s, generated
//! from an adapter that could not build an inverse, and it is not a decision the pack makes: a
//! conformance table that let it fire would be measuring the fold rather than the pack
//! (`tests/verdict_meet.rs` measures the fold).
//!
//! No invariants are registered, for the same reason and one more: **D-9** — the shipped pack carries
//! no ready-made [`gx_gate::InvariantCheck`], and `req/65` §3 gives the three reasons. An empty
//! registry here is therefore not a simplification of the shipped state; it *is* the shipped state.
//!
//! # What the cases are expected to say, and what they never assert
//!
//! Expectations name the arm and the policy id that answered. They never assert that Cedar decided
//! correctly — req/60 §7.2: 「Cedar の決定を gx の test が再実装しない」 — so what is measured is that
//! this pack, through this mapping, reaches this arm with this policy recorded as the one that
//! answered.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::packs::{self, FS_PACK_POLICY_IDS};
use gx_gate::{Gate, GateInput, ReasonSource, Verdict, POLICY_DENIED};

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

/// What a case expects the gate to answer.
#[derive(Debug)]
enum Expect {
    /// Admitted, with this policy recorded as the one that permitted it.
    Admit(&'static str),
    /// Refused, with this policy recorded as the one that forbade it.
    DenyBy(&'static str),
    /// Refused because nothing in the pack was satisfied — Cedar's third rule, reaching
    /// `ReasonSource::NoPolicyApplied` (**E-M3-11**).
    DenyByNoPolicy,
}

struct Case {
    name: &'static str,
    substrate: SubstrateKind,
    locator: &'static str,
    order: u8,
    expect: Expect,
    /// Why the case is in the table. A conformance table is documentation a deployment reads before
    /// it trusts a pack, so a row that cannot say what it is for does not belong in one.
    why: &'static str,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "a change under /tmp is admitted",
            substrate: SubstrateKind::Fs,
            locator: "/tmp/x",
            order: 0,
            expect: Expect::Admit("fs-permit-default"),
            why: "AC-028's Admit case. Cedar is default-deny, so the base permit is what makes an \
                  fs deployment able to admit anything at all",
        },
        Case {
            name: "a change to a file under /etc is refused",
            substrate: SubstrateKind::Fs,
            locator: "/etc/passwd",
            order: 0,
            expect: Expect::DenyBy("fs-deny-etc"),
            why:
                "AC-028's Deny case, and AC-025's first half. 「forbid overrides permit」, so the \
                  base permit does not need an exception carved into it",
        },
        Case {
            name: "a change to the /etc node itself is refused",
            substrate: SubstrateKind::Fs,
            locator: "/etc",
            order: 0,
            expect: Expect::DenyBy("fs-deny-etc"),
            why: "hand 5's decision (req/65 §3): `like \"/etc/*\"` does not match the directory \
                  node, so a pack that only wrote the glob would forbid every file under /etc and \
                  permit replacing /etc",
        },
        Case {
            name: "a change on a substrate this pack does not speak for is refused by no policy",
            substrate: SubstrateKind::Git,
            locator: "refs/heads/main",
            order: 0,
            expect: Expect::DenyByNoPolicy,
            why: "the fs pack scopes both statements to the fs substrate, so a git change reaches \
                  Cedar's third rule rather than being quietly admitted by an fs permit. FR-028 \
                  puts the git pack in M7",
        },
        Case {
            name: "an order-2 change under /tmp is admitted by this pack alone",
            substrate: SubstrateKind::Fs,
            locator: "/tmp/x",
            order: 2,
            expect: Expect::Admit("fs-permit-default"),
            why: "**D-9**, made visible: no ready-made invariant ships, so nothing in the shipped \
                  artifact refuses a change to the policy layer. AC-029 shows a deployment's own \
                  invariant doing it; this row is the honest record that the pack does not",
        },
    ]
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// A change at the given order. At order 2 the subject is another transformation and the context is
/// `Policy` (41 §3, P-3): 「policy自体の変更」.
fn change(order: u8) -> Transformation {
    let subject = if order == 0 {
        Subject::Object(ObjectId(cid(2)))
    } else {
        Subject::Transformation(TransformationId(cid(20 + u64::from(order))))
    };
    Transformation::new(
        TransformationId(cid(1)),
        order,
        subject,
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: if order == 0 {
                ChangeContext::Substrate
            } else {
                ChangeContext::Policy
            },
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("orders 0..=2 are at or below MAX_ORDER")
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

/// A gate holding the shipped pack and nothing else (D-9).
fn shipped_gate() -> Gate {
    Gate::with_policies(packs::fs_pack().expect("the shipped pack parses"))
}

// ---------------------------------------------------------------------------
// The criterion
// ---------------------------------------------------------------------------

/// Every row of the table, decided by the shipped pack through `Gate::verify`.
#[test]
fn ac_028_every_conformance_case_holds() {
    let gate = shipped_gate();
    for case in cases() {
        let t = change(case.order);
        let pre = object(case.substrate.clone(), case.locator);
        let planned = PlannedDeltaBytes(b"opaque to everything below the adapter".to_vec());
        let verdict = gate
            .verify(GateInput {
                t: &t,
                pre: &pre,
                planned: &planned,
                evidence: &[],
                invert_available: true,
            })
            .unwrap_or_else(|e| panic!("{}: the pack must be evaluable: {e}", case.name));

        match (&case.expect, verdict) {
            (Expect::Admit(id), Verdict::Admit(proof)) => {
                let deciding: Vec<&str> = proof
                    .policy_decisions()
                    .iter()
                    .map(|record| record.policy_id.as_str())
                    .collect();
                assert_eq!(
                    deciding,
                    vec![*id],
                    "{}: the proof records which policy permitted it ({})",
                    case.name,
                    case.why
                );
                assert!(
                    proof.invariant_results().is_empty(),
                    "{}: the shipped pack registers no invariant (D-9)",
                    case.name
                );
            }
            (Expect::DenyBy(id), Verdict::Deny(reasons)) => {
                assert_eq!(
                    reasons.len(),
                    1,
                    "{}: one deciding policy, one reason",
                    case.name
                );
                assert_eq!(reasons[0].code(), POLICY_DENIED, "{}", case.name);
                assert_eq!(
                    *reasons[0].source(),
                    ReasonSource::Policy {
                        policy_id: (*id).to_string()
                    },
                    "{}: 42 §3.8's ReasonSource names the policy that refused ({})",
                    case.name,
                    case.why
                );
            }
            (Expect::DenyByNoPolicy, Verdict::Deny(reasons)) => {
                assert_eq!(reasons.len(), 1, "{}", case.name);
                assert_eq!(
                    *reasons[0].source(),
                    ReasonSource::NoPolicyApplied,
                    "{}: nothing in the pack was satisfied ({})",
                    case.name,
                    case.why
                );
            }
            (expected, got) => panic!(
                "{}: expected {expected:?}, got {got:?} -- {}",
                case.name, case.why
            ),
        }
    }
}

/// AC-028's own arithmetic: 「最低1 Admitケース・1 Denyケース」, counted from the table.
///
/// The criterion is a statement about the *pack's* conformance suite, so it is asserted about the
/// suite rather than satisfied by it accidentally. A table that lost its Admit case would otherwise
/// still pass every row it kept.
#[test]
fn ac_028_the_table_holds_at_least_one_admit_case_and_one_deny_case() {
    let cases = cases();
    let admits = cases
        .iter()
        .filter(|c| matches!(c.expect, Expect::Admit(_)))
        .count();
    let denies = cases
        .iter()
        .filter(|c| matches!(c.expect, Expect::DenyBy(_) | Expect::DenyByNoPolicy))
        .count();
    println!(
        "ac_028: {admits} Admit case(s), {denies} Deny case(s), {} rows",
        cases.len()
    );
    assert!(admits >= 1, "AC-028: 「最低1 Admitケース」");
    assert!(denies >= 1, "AC-028: 「1 Denyケース」");
}

/// The cases ran against the artifact that ships, not against a set assembled here.
///
/// [`gx_gate::packs::fs_pack`] parses [`gx_gate::packs::FS_PACK_SOURCE`], which is the one road
/// FR-028 asks for. What this asserts is that the road carries the statements the module declares:
/// a pack that lost its forbid, or gained a statement nobody named, is a different artifact from the
/// one these cases were written against. `tests/pack_embedding.rs` measures the road itself -- how
/// many there are, where it goes, and whether the bytes at the far end are the file on disk.
#[test]
fn the_conformance_runs_against_the_pack_the_build_embeds() {
    let engine = packs::fs_pack().expect("the shipped pack parses");
    assert_eq!(
        engine.policy_ids(),
        FS_PACK_POLICY_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect::<Vec<String>>(),
        "the ids the module declares are the ids the pack answers under (ASM-62-1)"
    );
    assert_eq!(
        engine.len(),
        FS_PACK_POLICY_IDS.len(),
        "two statements, and no third one that no case covers"
    );
}

/// The pack cannot see the change it is judging (P-6 / M3-10), measured rather than described.
///
/// Two requests differing only in the payload get the same verdict. That is the wall behind which
/// 45 TH-1's 「行数保存」 invariant and FR-028's 「既製invariant集」 sit until an adapter hands the gate
/// structured facts (M4), and it is the reason the effective range in `packs.rs` says locator /
/// actor / context / order and stops there.
#[test]
fn the_pack_cannot_see_the_change_it_is_judging() {
    let gate = shipped_gate();
    let t = change(0);
    let pre = object(SubstrateKind::Fs, "/tmp/x");

    let small = PlannedDeltaBytes(b"one line".to_vec());
    let enormous = PlannedDeltaBytes(vec![b'x'; 64 * 1024]);

    let verdict_of = |planned: &PlannedDeltaBytes| {
        gate.verify(GateInput {
            t: &t,
            pre: &pre,
            planned,
            evidence: &[],
            invert_available: true,
        })
        .expect("evaluable")
        .kind()
    };

    assert_eq!(verdict_of(&small), VerdictKind::Admit);
    assert_eq!(
        verdict_of(&small),
        verdict_of(&enormous),
        "a pack that could reason over the payload would be able to tell these apart (P-6)"
    );
}
