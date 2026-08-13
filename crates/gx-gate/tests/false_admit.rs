//! The false-admit suite — 「deny されるべき変換が admit された」 has to be a red CI run.
//!
//! req/60 §5.2 hand 6 逐語: 「**負例 suite**: 「deny されるべき変換が admit された」を検出する vector 群
//! を新設(系統 A/B/C の次の系統・**expected は `MUST_DENY`**)」.
//!
//! # Where it sits among the three系統 req/29 already has
//!
//! req/29 §2 names three vector families: **A** codec deviation, **B** adversarial CJK text, **C**
//! forged judgement. All three are gx-canon's, and all three ask 「is this byte string refused?」.
//! This is the fourth, and it asks a different question of a different layer: **given a request the
//! shipped pack must refuse, does the gate refuse it?** The unit is not a byte string but a
//! `GateInput`, and the answer is not `Err` but a `Verdict`.
//!
//! `expected` is therefore `MUST_DENY` rather than `REJECT`. The two are not synonyms here, and the
//! difference is the whole reason this file exists: an `Err` from [`gx_gate::Gate::verify`] is ⊥
//! (**E-M3-3**: 「評価不能(⊥)は Deny でも Escalate でもない」), so a vector that fell over during
//! evaluation would not have been denied — it would have failed to be judged. A suite that accepted
//! ⊥ as a refusal would go green on a gate that had stopped working, which is the fail-open req/29
//! §4 forbids. So `MUST_DENY` means: the gate reached a decision, and the decision was `Deny`.
//!
//! # What each vector is worth, and why every one carries a control
//!
//! req/29 §2: 「正例の対照を必ず対で持つ」. Without one, a gate that denied everything would pass this
//! file — and 「deny everything」 is exactly what a broken pack looks like from the negative side. The
//! controls are written to differ from their vector in **one** respect (FA-5's differ only in the
//! substrate, FA-7's only in the locator, with the inverse flag held false in both), so a control
//! that goes red names the row of the mapping that moved.
//!
//! Two of the eight controls are worth reading twice:
//!
//! * **FA-5** holds the locator byte-for-byte and changes only `pre.substrate`. If ASM-60-1's
//!   substrate row is dropped, the vector and its control become the same request. 🔴 Its subject
//!   moved in **M7 hand 4** and the vector file says why: the substrate it names has to be one **no
//!   shipped pack speaks for**, and after AC-074 that is no longer `git` (H-9).
//! * **FA-7**'s control expects `MUST_ESCALATE`, not `MUST_ADMIT`. The point of that vector is that
//!   `Deny` outranks `Escalate` (AC-027's first sentence is unconditional), so the control has to
//!   keep `invert_available = false` and move only the locator. A control expecting `Admit` would
//!   have moved two things at once and measured neither.
//!
//! # What this suite does **not** claim
//!
//! It is not a statement that the shipped pack is sufficient. req/29 §1 逐語: 「網羅は主張しない …
//! suite が言えるのは「この一覧は弾いた」まで」. It also cannot see the change itself (P-6 / M3-10), so
//! nothing here is about what a transformation *does* — only about what it is addressed to. And it
//! is not the fold's test: `verdict_meet.rs` measures AC-027 over its four quadrants, while this file
//! measures the artifact a deployment installs.
//!
//! One known false admit is pinned at the bottom of this file rather than left for a reader to
//! discover: the pack judges the locator **as given**, so a locator that walks out of a permitted
//! directory is admitted. It is recorded as behaviour and raised in `req/66` §4, in the shape
//! `gx-canon/tests/negative_vectors.rs` pins its map-key-order discrepancy.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::{packs, Gate, GateInput, ReasonSource, Verdict};
use gx_witness::{Evidence, TestOutcome};
use std::path::{Path, PathBuf};

/// Ten, written down so that adding a vector without accounting for it fails rather than passes
/// unnoticed — the count guard `negative_vectors.rs` carries for its own directory.
///
/// Eight until **M7 hand 4**, which added one vector per pack AC-074 ships
/// ([`every_shipped_pack_is_refused_by_a_vector_of_this_suite`] is why it had to).
const EXPECTED_VECTOR_COUNT: usize = 10;

/// The five keys a `request` object may hold. An unknown key is a vector saying something this
/// harness does not read, and silently ignoring it would be the fail-open form of a typo.
const REQUEST_KEYS: [&str; 5] = [
    "substrate",
    "locator",
    "order",
    "invert_available",
    "evidence",
];

// ---------------------------------------------------------------------------
// The vector files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Request {
    substrate: String,
    locator: String,
    order: u8,
    invert_available: bool,
    evidence: usize,
}

#[derive(Debug)]
struct Vector {
    id: String,
    statement: String,
    request: Request,
    /// The source and code the refusal must carry, when the vector names them. `is_err()` was not
    /// enough for gx-canon (H6-4) and 「it was denied」 is not enough here: a `Deny` produced by the
    /// wrong policy is a pack whose reason no operator can act on.
    expected_source: Option<ReasonSource>,
    expected_code: Option<String>,
    control: Request,
    control_expected: VerdictKind,
    /// 🔴 **H-9's 期限**: the substrate whose *absence from the shipped set* is what makes this
    /// vector's expectation true.
    ///
    /// A vector that expects `NoPolicyApplied` is not asserting a rule; it is asserting that
    /// **nothing** in the artifact has an opinion. That is a statement with an expiry date, and
    /// before this hand there was no way to write the date down: req/66 §4 filed the gap as 「vector
    /// に期限を持たせる仕組みは今の schema に無い」 and H-9 reserved the fix for M7.
    ///
    /// [`no_vector_outlives_the_pack_that_makes_it_true`] is the mechanism, and what it does is not
    /// keep the vector true — it makes the vector **fail loudly on the day it stops being true**,
    /// naming the pack that ended it. That day already happened once, in M7 hand 2, and nobody
    /// noticed until this hand's red-first commit.
    expires_when_a_pack_speaks_for: Option<String>,
}

fn vectors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/false_admit")
}

fn request_of(v: &serde_json::Value, where_: &str) -> Request {
    let obj = v
        .as_object()
        .unwrap_or_else(|| panic!("{where_}: request is an object"));
    for key in obj.keys() {
        assert!(
            REQUEST_KEYS.contains(&key.as_str()),
            "{where_}: unknown request key {key:?}; this harness reads {REQUEST_KEYS:?}"
        );
    }
    let s = |k: &str| -> String {
        obj.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{where_}: missing string {k}"))
            .to_string()
    };
    Request {
        substrate: s("substrate"),
        locator: s("locator"),
        order: u8::try_from(
            obj.get("order")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| panic!("{where_}: missing order")),
        )
        .unwrap_or_else(|_| panic!("{where_}: order is 0, 1 or 2")),
        invert_available: obj
            .get("invert_available")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or_else(|| panic!("{where_}: missing invert_available")),
        evidence: usize::try_from(
            obj.get("evidence")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| panic!("{where_}: missing evidence count")),
        )
        .expect("a small count"),
    }
}

/// A vector that cannot be read is a setup failure and not a test result (req/29 §4), so every step
/// here panics rather than skipping, and an empty directory cannot pass.
fn load() -> Vec<Vector> {
    let dir = vectors_dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("vector directory {} unreadable: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    let mut out = Vec::new();
    for p in paths {
        let name = p.display().to_string();
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        let v: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let s = |k: &str| -> String {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("{name}: missing string field {k}"))
                .to_string()
        };
        assert_eq!(
            s("expected"),
            "MUST_DENY",
            "{name}: this family's expectation is MUST_DENY (req/60 §5.2 hand 6)"
        );
        assert_eq!(s("kind"), "false-admit", "{name}");
        for field in ["id", "statement", "why", "spec_anchor", "origin"] {
            assert!(!s(field).is_empty(), "{name}: {field} is empty");
        }

        let (expected_source, expected_code) = match v.get("expected_reason") {
            None => (None, None),
            Some(r) => {
                let source = match r
                    .get("source")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_else(|| panic!("{name}: expected_reason.source"))
                {
                    "Policy" => ReasonSource::Policy {
                        policy_id: r
                            .get("policy_id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_else(|| panic!("{name}: Policy names a policy_id"))
                            .to_string(),
                    },
                    "NoPolicyApplied" => ReasonSource::NoPolicyApplied,
                    "Invariant" => ReasonSource::Invariant {
                        invariant_id: r
                            .get("invariant_id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_else(|| panic!("{name}: Invariant names an invariant_id"))
                            .to_string(),
                    },
                    other => panic!("{name}: {other:?} is not a ReasonSource this harness builds"),
                };
                (
                    Some(source),
                    r.get("code")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string),
                )
            }
        };

        let control = v
            .get("control")
            .unwrap_or_else(|| panic!("{name}: every vector carries a control (req/29 §2)"));
        let control_expected = match control
            .get("expected")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{name}: control.expected"))
        {
            "MUST_ADMIT" => VerdictKind::Admit,
            "MUST_ESCALATE" => VerdictKind::Escalate,
            other => panic!(
                "{name}: a control expecting {other:?} would not be a control — this family's \
                 negative answer is Deny, so a control has to be one of the two verdicts that are \
                 not it"
            ),
        };
        assert!(
            control
                .get("note")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|n| !n.is_empty()),
            "{name}: a control says what it isolates"
        );

        // 🔴 The field is required exactly when the vector expects `NoPolicyApplied`: that is the
        // one expectation whose truth depends on what the artifact does **not** contain, and a
        // vector making it without naming its expiry is the gap H-9 was raised about.
        let expires = v
            .get("expires_when_a_pack_speaks_for")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        if expected_source == Some(ReasonSource::NoPolicyApplied) {
            assert!(
                expires.is_some(),
                "{name}: a vector expecting NoPolicyApplied depends on no pack speaking for its \
                 substrate, so it names the substrate whose pack would end it \
                 (`expires_when_a_pack_speaks_for`) — H-9 / req/66 §4"
            );
        }

        out.push(Vector {
            id: s("id"),
            statement: s("statement"),
            request: request_of(
                v.get("request")
                    .unwrap_or_else(|| panic!("{name}: missing request")),
                &name,
            ),
            expected_source,
            expected_code,
            control: request_of(
                control
                    .get("request")
                    .unwrap_or_else(|| panic!("{name}: control.request")),
                &name,
            ),
            control_expected,
            expires_when_a_pack_speaks_for: expires,
        });
    }
    assert_eq!(
        out.len(),
        EXPECTED_VECTOR_COUNT,
        "vector count changed; update EXPECTED_VECTOR_COUNT and req/66"
    );
    out
}

// ---------------------------------------------------------------------------
// Building the request the vector describes
// ---------------------------------------------------------------------------

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn substrate_of(tag: &str) -> SubstrateKind {
    match tag {
        "fs" => SubstrateKind::Fs,
        "git" => SubstrateKind::Git,
        "mcp" => SubstrateKind::Mcp,
        other => SubstrateKind::Custom(other.to_string()),
    }
}

/// The change, at the order the vector names. At order >= 1 the subject is another transformation
/// and the context is `Policy` (41 §3, P-3), which is the shape `ac_028.rs` uses for the same reason.
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
    .expect("orders 0..=2 are admitted, and the metadata is inside E-M3-13's range")
}

/// `n` pieces of evidence, all of one kind. What reaches a policy is the summary — ASM-60-1 maps the
/// slice to `context.evidence_count` and `context.evidence_kinds` — so what a vector needs to control
/// is how many and of what kind, not what they say.
fn evidence(n: usize) -> Vec<Evidence> {
    (0..n)
        .map(|i| Evidence::TestResult {
            case: format!("case-{i}"),
            suite: "false-admit".to_string(),
            outcome: TestOutcome::Pass,
            log_digest: None,
            duration_ms: 1,
        })
        .collect()
}

/// The verdict the shipped pack reaches for a request, or a panic naming the ⊥ it hit.
///
/// ⊥ is not a refusal (**E-M3-3**), so it may not be read as one here: a vector whose evaluation
/// failed has not been denied, and reporting that as a pass is the fail-open this suite exists to
/// close.
fn verdict_for(gate: &Gate, r: &Request, who: &str) -> Verdict {
    let t = change(r.order);
    let pre = ObjectSnapshot::new(
        ObjectId(cid(2)),
        substrate_of(&r.substrate),
        r.locator.clone(),
        cid(6),
        ReprKind::Bytes,
    );
    let planned = PlannedDeltaBytes(b"opaque to everything below the adapter (P-6)".to_vec());
    let ev = evidence(r.evidence);
    gate.verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &ev,
        invert_available: r.invert_available,
    })
    .unwrap_or_else(|e| {
        panic!("{who}: the gate could not evaluate this request at all ({e}) — ⊥ is not a Deny")
    })
}

/// 🔴 **H-9** — the gate this suite measures is the **whole shipped artifact**, not one pack of it.
///
/// req/38 §25's H-9 reserved the reversal for M7: 「FA-5 の期待は **M7 で反転する**——M7 reqdef 必須
/// ticket(G-4 と同窓)に「vector 期限/pack 出荷との連動」を含める」, and req/66 §4 wrote why: 「git
/// pack(AC-074/M7)が出荷された瞬間、「git の変換は NoPolicyApplied で拒まれる」は偽になる。vector に
/// 期限を持たせる仕組みは今の schema に無い」.
///
/// Both halves of that ticket are below: [`no_vector_outlives_the_pack_that_makes_it_true`] is the
/// 期限, and [`every_shipped_pack_is_refused_by_a_vector_of_this_suite`] is the 連動.
///
/// The set is [`packs::shipped_pack_set`] because that is what a deployment installs after **M7 hand
/// 4** (`req/38` §60's R-9 対裁定): the module header calls this suite 「the artifact a deployment
/// installs」 and the artifact grew from one pack to three.
fn shipped_gate() -> Gate {
    Gate::with_policies(packs::shipped_pack_set().expect("the shipped packs compose into one set"))
}

/// The tag [`gx_gate::packs::ShippedPack::substrate`] carries, for a vector's substrate spelling.
///
/// The mapping `gx_gate::policy` makes, restated for the three names this harness reads plus the
/// `custom:` form — the same table [`substrate_of`] walks, one value along.
fn substrate_tag(spelling: &str) -> String {
    match spelling {
        "fs" | "git" | "mcp" => spelling.to_string(),
        other => format!("custom:{other}"),
    }
}

// ---------------------------------------------------------------------------
// The criteria
// ---------------------------------------------------------------------------

/// **The suite's whole claim**: not one vector is admitted, and not one is escalated.
///
/// Escalation is checked with the same strictness as admission, because a change that two systems
/// refused and that the gate then sent to a human for approval has been given a route it must not
/// have (AC-027's Deny precedence, and FA-7 is the input that separates the two).
#[test]
fn false_admit_no_vector_reaches_a_verdict_other_than_deny() {
    let gate = shipped_gate();
    let mut let_through = Vec::new();
    for v in load() {
        let verdict = verdict_for(&gate, &v.request, &v.id);
        if verdict.kind() != VerdictKind::Deny {
            let_through.push(format!(
                "{} answered {:?} — {}",
                v.id,
                verdict.kind(),
                v.statement
            ));
        }
    }
    assert!(
        let_through.is_empty(),
        "the shipped pack let a MUST_DENY vector through: {let_through:#?}"
    );
}

/// The refusal is the declared one — which policy said no, and under which code.
///
/// `negative_vectors.rs` learned this from H6-4: 「「it was refused」 did not say *why*」. A pack that
/// refused `/etc/passwd` because nothing matched (rather than because its forbid did) would satisfy
/// the probe above and mean something entirely different — it would mean the permit had vanished.
#[test]
fn false_admit_the_refusal_carries_the_declared_source_and_code() {
    let gate = shipped_gate();
    let mut checked = 0;
    for v in load() {
        let Some(expected) = &v.expected_source else {
            continue;
        };
        checked += 1;
        let Verdict::Deny(reasons) = verdict_for(&gate, &v.request, &v.id) else {
            panic!("{}: the previous probe owns this failure", v.id);
        };
        let sources: Vec<&ReasonSource> = reasons.iter().map(gx_gate::Reason::source).collect();
        assert!(
            sources.contains(&expected),
            "{}: refused by {sources:?}, and the vector declares {expected:?} — the refusal is not \
             the claim, the clause that made it is ({})",
            v.id,
            v.statement
        );
        if let Some(code) = &v.expected_code {
            assert!(
                reasons.iter().any(|r| r.code() == code),
                "{}: codes {:?} do not include the declared {code}",
                v.id,
                reasons
                    .iter()
                    .map(gx_gate::Reason::code)
                    .collect::<Vec<_>>()
            );
        }
    }
    println!("FALSE_ADMIT_VECTORS_WITH_A_DECLARED_REASON={checked} of {EXPECTED_VECTOR_COUNT}");
    assert!(checked >= 8, "only {checked} vectors declare a reason");
}

/// The controls. A gate that denied everything would pass the two probes above and fail this one,
/// which is the whole of req/29 §2's 「正例の対照を必ず対で持つ」.
#[test]
fn false_admit_every_control_is_not_denied() {
    let gate = shipped_gate();
    let mut wrong = Vec::new();
    let mut admits = 0;
    let mut escalates = 0;
    for v in load() {
        let got = verdict_for(&gate, &v.control, &format!("{} control", v.id)).kind();
        match v.control_expected {
            VerdictKind::Admit => admits += 1,
            VerdictKind::Escalate => escalates += 1,
            VerdictKind::Deny => unreachable!("the loader refuses a control expecting Deny"),
        }
        if got != v.control_expected {
            wrong.push(format!(
                "{} control answered {got:?}, declared {:?}",
                v.id, v.control_expected
            ));
        }
    }
    println!("FALSE_ADMIT_CONTROLS={admits} MUST_ADMIT + {escalates} MUST_ESCALATE");
    assert!(
        wrong.is_empty(),
        "controls behaved unexpectedly: {wrong:#?}"
    );
}

// ---------------------------------------------------------------------------
// 🔴 H-9 — vector expiry, and the linkage to what ships
// ---------------------------------------------------------------------------

/// 🔴 **H-9's first half (期限)**: no vector outlives the absence that made it true.
///
/// A vector expecting `NoPolicyApplied` says 「nothing in the shipped artifact has an opinion about
/// this substrate」. The day a pack for that substrate ships, the sentence is false — and the vector
/// does not become *wrong* so much as it becomes **about something else**, which is worse, because a
/// suite whose rows have quietly changed subject still prints green.
///
/// So the expiry is read off [`SHIPPED_PACKS`], which is the same declaration `packs.rs` embeds and
/// `pack_embedding.rs` compares with the tree. Nothing here is remembered: a pack added in some
/// later hand ends every vector that depended on its absence, in the run that adds it.
///
/// It has already fired once. M7 hand 2 shipped `policies/git/deny-nonbranch-refs.cedar` while FA-5
/// still read 「fs pack が語らない substrate の変換を…admit してはならない」 with a **git** request in
/// it, and the suite stayed green for two hands because it was still being run against the fs pack
/// alone. Hand 4's red-first commit is that reversal, measured.
#[test]
fn no_vector_outlives_the_pack_that_makes_it_true() {
    let mut expired: Vec<String> = Vec::new();
    let mut dated = 0usize;
    for v in load() {
        let Some(spelling) = &v.expires_when_a_pack_speaks_for else {
            continue;
        };
        dated += 1;
        let tag = substrate_tag(spelling);
        if let Some(pack) = packs::SHIPPED_PACKS.iter().find(|p| p.substrate == tag) {
            expired.push(format!(
                "{} expects nothing to speak for {spelling:?}, and {} does — restate the vector \
                 (it is now a request the shipped set decides) or move it to a substrate no pack \
                 speaks for",
                v.id, pack.path
            ));
        }
    }
    println!(
        "FALSE_ADMIT_DATED_VECTORS={dated} EXPIRED={} SHIPPED_PACKS={}",
        expired.len(),
        packs::SHIPPED_PACKS.len()
    );
    assert!(
        expired.is_empty(),
        "H-9: a vector's expectation stopped being true when a pack shipped: {expired:#?}"
    );
}

/// 🔴 **H-9's second half (pack 出荷との連動)**: every shipped pack is refused by a vector here.
///
/// The expiry above is the defensive direction — a pack arriving must not silently invalidate a
/// row. This is the other one: a pack arriving must **bring** a row. Without it the negative suite
/// would go on measuring the fs pack while the artifact a deployment installs grew two more, which
/// is the same 分母 failure this project keeps finding in absence checks (§30).
///
/// The requirement is deliberately the strong one — a vector whose declared refusal names a
/// statement **of that pack** — because a vector that merely used the substrate could be satisfied
/// by the pack's permit and would then be a positive case wearing a negative's name.
#[test]
fn every_shipped_pack_is_refused_by_a_vector_of_this_suite() {
    let vectors = load();
    let mut missing: Vec<&str> = Vec::new();
    let mut covered: Vec<(&str, String)> = Vec::new();
    for pack in packs::SHIPPED_PACKS {
        let found = vectors.iter().find(|v| {
            substrate_tag(&v.request.substrate) == pack.substrate
                && matches!(
                    &v.expected_source,
                    Some(ReasonSource::Policy { policy_id }) if pack.policy_ids.contains(&policy_id.as_str())
                )
        });
        match found {
            Some(v) => covered.push((pack.substrate, v.id.clone())),
            None => missing.push(pack.path),
        }
    }
    println!("FALSE_ADMIT_PACK_COVERAGE={covered:?} MISSING={missing:?}");
    assert!(
        missing.is_empty(),
        "H-9: a shipped pack has no MUST_DENY vector naming one of its statements: {missing:?}. \
         The negative suite measures the artifact a deployment installs, and the artifact is \
         `SHIPPED_PACKS`"
    );
}

/// What ran, with the denominator (req/29 §4: 「出力に必ず『見ていない範囲』を載せる」).
#[test]
fn false_admit_breakdown() {
    let vectors = load();
    let orders: std::collections::BTreeSet<u8> = vectors.iter().map(|v| v.request.order).collect();
    let substrates: std::collections::BTreeSet<&str> = vectors
        .iter()
        .map(|v| v.request.substrate.as_str())
        .collect();
    println!(
        "FALSE_ADMIT_VECTORS={} CONTROLS={} ORDERS={orders:?} SUBSTRATES={substrates:?}",
        vectors.len(),
        vectors.len()
    );
    for v in &vectors {
        println!(
            "  {:<5} {:<4} {:<28} invert={} evidence={} -> MUST_DENY",
            v.id,
            v.request.substrate,
            v.request.locator,
            v.request.invert_available,
            v.request.evidence
        );
    }
    println!(
        "NOT COVERED: the payload (P-6 keeps it opaque — so on the mcp substrate, **which tool is \
         being called**), any substrate but the four above, any pack but the shipped set, the \
         composition of a deployment's own statements with it, and 「網羅」 — this list is what was \
         tried, not what is safe (req/29 §1)"
    );
    assert_eq!(vectors.len(), EXPECTED_VECTOR_COUNT);
}

// ---------------------------------------------------------------------------
// The false admit this suite found and did not fix
// ---------------------------------------------------------------------------

/// **The pack judges the locator as given, not as resolved** — so `/tmp/../etc/passwd` is admitted.
///
/// This asserts the behaviour rather than the rule, in the shape `negative_vectors.rs` uses for the
/// map-key-order discrepancy: pinning it means a future change to it is deliberate and visible.
///
/// Why it is not fixed here. Normalising a locator is not gx-gate's to do — the locator arrives in
/// `pre` from an adapter (42 §3.1), and inventing a path algebra inside a policy layer would be both
/// an invention the spec does not carry and a second definition of what a path means. Cedar's `like`
/// cannot express it either: `*` matches any run of characters, `..` included. So the honest form is
/// to record it, and to raise the two candidate homes — the adapter contract (M4) and the effective
/// range in `packs.rs`, which today says what a policy may reason over but not that the locator it
/// reasons over is assumed already resolved. `req/66` §4 carries it.
#[test]
fn the_pack_judges_the_locator_as_given_not_as_resolved() {
    let gate = shipped_gate();
    let traversing = Request {
        substrate: "fs".to_string(),
        locator: "/tmp/../etc/passwd".to_string(),
        order: 0,
        invert_available: true,
        evidence: 0,
    };
    let got = verdict_for(&gate, &traversing, "traversal").kind();
    assert_eq!(
        got,
        VerdictKind::Admit,
        "recorded, not endorsed: if this becomes Deny, something normalises locators now and the \
         vector belongs in the table above (req/66 §4)"
    );

    // The other direction is already refused, and for the same reason: the string is what is
    // matched. `/etc/../tmp/x` is under the forbid's glob whether or not it resolves elsewhere.
    let into_etc = Request {
        locator: "/etc/../tmp/x".to_string(),
        ..traversing
    };
    assert_eq!(
        verdict_for(&gate, &into_etc, "traversal-in").kind(),
        VerdictKind::Deny,
        "the asymmetry is the finding: the same unresolved string is over-refused one way and \
         under-refused the other"
    );
}
