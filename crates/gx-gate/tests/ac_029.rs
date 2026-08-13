//! AC-029 (FR-029) — an order-2 change takes the same road as every other change, and an invariant
//! can stop it there.
//!
//! AC-029 逐語: 「Given: order=2のTransformation（既存policy P_beforeをP_afterへ緩める変換）。When:
//! `Gate::verify`へ通常のorder-0/1と**同一の`verify`エントリポイント**から渡す。Then: order=2専用の
//! バイパス経路（別関数呼び出し）が存在せず、Cedar評価+invariant registryを経由した通常のVerdictが
//! 返る（呼び出しトレースで確認）。P_afterが全許可へ緩めるような極端なケースでも、gate自体のpolicy
//! （order-2変換に対するinvariant）がDenyを返せる。」判定方法: 「integration + property」.
//!
//! # Measuring something that is absent
//!
//! 「バイパス経路が存在せず」 is a claim about what is *not* there, and req/60 §7.2 says what such a
//! claim costs: 「経路を 1 本足したら RED になる事を変異で実測しないと空虚になる」. So the criterion
//! is measured on three faces, and `tools/verify_m3h3.sh` §7 adds the missing path and shows that
//! all three go red:
//!
//! | face | what it measures | what a bypass would do to it |
//! |---|---|---|
//! | behaviour | an order-2 change reaches the same `Verdict` machinery | an early return would produce a verdict nothing recorded |
//! | call trace | both systems ran, once, with the real input | an early return would leave the counters at zero |
//! | source | `lib.rs` -- the file `verify` lives in -- never mentions `order` | a branch on it would put the word there |
//!
//! The third is the one that catches a bypass which happens to produce the same answer today. It is
//! the idiom `ac_069_the_durability_barrier_has_exactly_one_call_site` established in gx-log: when
//! a property is about the shape of the code rather than about a value, the source is the
//! instrument, and it is written down as such rather than dressed up as a behavioural test.
//!
//! # What 「P_afterが全許可へ緩める」 is, here
//!
//! The extreme case the criterion names is a policy set that permits everything -- the state a
//! successful attack on the policy layer would leave behind. [`PERMIT_EVERYTHING`] below *is* that
//! state, and the invariant that refuses order-2 changes is 「gate自体のpolicy」. The refusal is
//! demonstrated rather than shipped: which invariants a deployment runs is FR-028's pack (hand 5),
//! and M3-10 already fixes what such a pack may reach.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::verdict::{InvariantResult, ReasonSource};
use gx_gate::{
    Gate, GateInput, InvariantCheck, InvariantRegistry, PolicyEngine, Result, Verdict,
    INVARIANT_VIOLATED,
};
use proptest::prelude::*;

/// The policy set of the extreme case: every request is permitted.
const PERMIT_EVERYTHING: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

/// The same, with one refusal, so that the property below samples both outcomes.
///
/// A property that only ever saw `Admit` would hold for a gate that admitted everything, which is
/// the fail-open req/29 §4 keeps naming.
const PERMIT_EVERYTHING_BUT_ETC: &str = r#"@id("permit-everything")
permit (principal, action, resource);

@id("forbid-etc")
forbid (principal, action, resource)
when { resource.locator like "/etc/*" };
"#;

// ---------------------------------------------------------------------------
// The two invariants this file needs
// ---------------------------------------------------------------------------

/// Records every call it receives: how many, and the order of the transformation it was handed.
///
/// The order is what makes this a *trace* rather than a counter. A registry that called every check
/// with a fabricated input -- or with the wrong one of several -- would keep the counts right and
/// the trace wrong.
struct Tracing {
    id: String,
    trace: Arc<Mutex<Vec<u8>>>,
}

impl InvariantCheck for Tracing {
    fn id(&self) -> &str {
        &self.id
    }

    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult> {
        self.trace
            .lock()
            .expect("no test in this file panics while holding the lock")
            .push(input.t.order());
        InvariantResult::new(self.id.clone(), true, None)
    }
}

/// 「gate自体のpolicy（order-2変換に対するinvariant）」: a change to a policy is refused.
///
/// This is the invariant E-M3-7 exists for. Under 41 §4's original signature -- `check(pre,
/// planned)` -- it could not be written at all: `order` is a field of `Transformation` and neither
/// argument carried one.
struct RefusesOrderTwo {
    calls: Arc<AtomicUsize>,
}

impl InvariantCheck for RefusesOrderTwo {
    fn id(&self) -> &str {
        "order-2-changes-are-not-self-approved"
    }

    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let order = input.t.order();
        InvariantResult::new(
            self.id().to_string(),
            order < 2,
            (order >= 2).then(|| format!("order {order} changes the policy layer itself")),
        )
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// A change at the given order. At order 2 it is 「既存policy P_beforeをP_afterへ緩める変換」: a
/// change whose subject is another transformation and whose context is `Policy` (41 §3, P-3).
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
            context: ChangeContext::Policy,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("orders 0..=2 are at or below MAX_ORDER")
}

fn object(locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        SubstrateKind::Fs,
        locator.to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

fn permissive() -> PolicyEngine {
    PolicyEngine::parse(PERMIT_EVERYTHING).expect("the permissive set parses")
}

// ---------------------------------------------------------------------------
// Face 1 and 2: the same road, and the trace that says so
// ---------------------------------------------------------------------------

/// Orders 0, 1 and 2 produce the same trace: one call to each registered invariant, carrying the
/// order that was actually submitted.
#[test]
fn ac_029_every_order_goes_through_the_same_entry_point_and_the_same_registry() {
    for order in [0u8, 1, 2] {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let registry = InvariantRegistry::new()
            .with(Box::new(Tracing {
                id: "aaa-watcher".to_string(),
                trace: Arc::clone(&trace),
            }))
            .and_then(|r| {
                r.with(Box::new(Tracing {
                    id: "bbb-watcher".to_string(),
                    trace: Arc::clone(&trace),
                }))
            })
            .expect("two distinct ids");

        let gate = Gate::with_policies(permissive()).with_invariants(registry);
        let t = change(order);
        let pre = object("/tmp/x");
        let planned = PlannedDeltaBytes(Vec::new());

        let verdict = gate
            .verify(GateInput {
                t: &t,
                pre: &pre,
                planned: &planned,
                evidence: &[],
                invert_available: true,
            })
            .expect("a permissive set and two holding invariants reach a verdict");

        let seen = trace.lock().expect("uncontended").clone();
        assert_eq!(
            seen,
            vec![order, order],
            "AC-029: order {order} reached both invariants, once each, carrying its own order"
        );
        assert_eq!(
            verdict.kind(),
            VerdictKind::Admit,
            "nothing refused this change at order {order}"
        );

        let Verdict::Admit(proof) = verdict else {
            panic!("a permissive set and holding invariants admit")
        };
        assert_eq!(
            proof.policy_decisions().len(),
            1,
            "the Cedar evaluation happened for order {order} too -- 「Cedar評価+invariant registryを\
             経由した通常のVerdict」"
        );
        assert_eq!(proof.invariant_results().len(), 2);
    }
}

/// 「P_afterが全許可へ緩めるような極端なケースでも、gate自体のpolicyがDenyを返せる」.
///
/// The policy set permits everything, which is the state the criterion asks about, and the verdict
/// is still `Deny` -- reached through the ordinary fold rather than through a check the gate makes
/// about itself somewhere else. The reason names the invariant (42 §3.8's `ReasonSource::Invariant`)
/// and carries E-M3-6's second code.
#[test]
fn ac_029_an_invariant_refuses_an_order_2_change_a_permissive_policy_would_admit() {
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = InvariantRegistry::new()
        .with(Box::new(RefusesOrderTwo {
            calls: Arc::clone(&calls),
        }))
        .expect("one id");
    let gate = Gate::with_policies(permissive()).with_invariants(registry);

    // Order 0 first: the same gate, the same policy set, and no refusal -- so the Deny below is the
    // order and not the fixture.
    let t0 = change(0);
    let pre = object("/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());
    let admitted = gate
        .verify(GateInput {
            t: &t0,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        })
        .expect("evaluable");
    assert_eq!(admitted.kind(), VerdictKind::Admit);

    let t2 = change(2);
    let refused = gate
        .verify(GateInput {
            t: &t2,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        })
        .expect("a refusal is a verdict, not an error (E-M3-3)");

    assert_eq!(refused.kind(), VerdictKind::Deny);
    let Verdict::Deny(reasons) = refused else {
        panic!("the Deny arm")
    };
    assert_eq!(reasons.len(), 1, "one invariant refused, one reason");
    assert_eq!(reasons[0].code(), INVARIANT_VIOLATED);
    assert_eq!(
        *reasons[0].source(),
        ReasonSource::Invariant {
            invariant_id: "order-2-changes-are-not-self-approved".to_string()
        },
        "42 §3.8's ReasonSource names the invariant that refused"
    );
    assert!(
        reasons[0].message().contains("order 2"),
        "the detail the invariant gave survives into the reason: {:?}",
        reasons[0].message()
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the invariant was asked once per verify, at both orders"
    );
}

// ---------------------------------------------------------------------------
// Face 3: the source of the entry point
// ---------------------------------------------------------------------------

fn source(file: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Lines that are not comments. `//`, `///` and `//!` all start with the same two characters, and
/// this crate uses no block comments -- `tools/verify_m3h3.sh` §10 measures that claim so that a
/// bypass hidden inside `/* */` could not walk past this scan.
fn code_lines(src: &str) -> Vec<&str> {
    src.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect()
}

/// There is one `verify`, and the file it lives in does not know what an order is.
///
/// The second half is the sharper of the two. A bypass has to read the order to be a bypass, so a
/// file whose code never names it cannot contain one; and the word is cheap to grep for, which is
/// what makes the mutation in `tools/verify_m3h3.sh` §7 turn this red immediately.
#[test]
fn ac_029_the_entry_point_is_the_only_road_and_it_cannot_see_an_order() {
    let lib = source("lib.rs");
    let policy = source("policy.rs");
    let invariant = source("invariant.rs");
    let verdict = source("verdict.rs");

    let verify_definitions: usize = [&lib, &policy, &invariant, &verdict]
        .iter()
        .map(|src| {
            code_lines(src)
                .iter()
                .filter(|line| line.contains("fn verify("))
                .count()
        })
        .sum();
    assert_eq!(
        verify_definitions, 1,
        "41 §4 declares one `verify`, and AC-029 asks that there be no second one"
    );

    let order_in_lib: Vec<&str> = code_lines(&lib)
        .into_iter()
        .filter(|line| line.contains("order"))
        .collect();
    assert!(
        order_in_lib.is_empty(),
        "the file holding `verify` mentions an order in code, so it can branch on one: {order_in_lib:?}"
    );

    // Crate-wide, `order` is read in exactly one place: the row of ASM-60-1's mapping that publishes
    // it to `context.order`, where a *policy* can compare it. That is the opposite of a bypass -- it
    // is what makes the order visible to the thing that decides.
    let readers: Vec<(&str, usize)> = [
        ("lib.rs", &lib),
        ("policy.rs", &policy),
        ("invariant.rs", &invariant),
        ("verdict.rs", &verdict),
    ]
    .into_iter()
    .map(|(name, src)| {
        (
            name,
            code_lines(src)
                .iter()
                .filter(|line| line.contains(".order()"))
                .count(),
        )
    })
    .collect();
    assert_eq!(
        readers,
        vec![
            ("lib.rs", 0),
            ("policy.rs", 1),
            ("invariant.rs", 0),
            ("verdict.rs", 0)
        ],
        "the only code that reads an order is the mapping row that hands it to a policy"
    );
}

// ---------------------------------------------------------------------------
// The property half (34: 「integration + property」)
// ---------------------------------------------------------------------------

fn any_locator() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("/etc/passwd".to_string()),
        "/tmp/[a-z]{1,8}".prop_map(String::from),
        "[ -~]{0,16}".prop_map(String::from),
    ]
}

proptest! {
    /// Whatever else varies, the road does not: every registered invariant is called exactly once,
    /// and the answer at order 2 is the answer at order 0 when nothing reads the order.
    ///
    /// The second half is the property form of 「order-2専用のバイパス経路が存在せず」. An early
    /// return for order 2 would break it whichever way it decided: admitting would differ from the
    /// order-0 answer wherever the policy refuses, and refusing would differ wherever it permits.
    #[test]
    fn ac_029_the_order_does_not_change_the_path(
        order in 0u8..=2,
        locator in any_locator(),
        invert_available in any::<bool>(),
    ) {
        let answer_at = |order: u8| {
            let trace = Arc::new(Mutex::new(Vec::new()));
            let registry = InvariantRegistry::new()
                .with(Box::new(Tracing { id: "aaa-watcher".to_string(), trace: Arc::clone(&trace) }))
                .and_then(|r| r.with(Box::new(Tracing {
                    id: "bbb-watcher".to_string(),
                    trace: Arc::clone(&trace),
                })))
                .expect("two distinct ids");
            let engine = PolicyEngine::parse(PERMIT_EVERYTHING_BUT_ETC).expect("the set parses");
            let gate = Gate::with_policies(engine).with_invariants(registry);

            let t = change(order);
            let pre = object(&locator);
            let planned = PlannedDeltaBytes(Vec::new());
            let verdict = gate.verify(GateInput {
                t: &t,
                pre: &pre,
                planned: &planned,
                evidence: &[],
                invert_available,
            });
            let seen = trace.lock().expect("uncontended").clone();
            (verdict.map(|v| v.kind()), seen)
        };

        let (kind, seen) = answer_at(order);
        prop_assert_eq!(seen, vec![order, order], "each invariant once, with the real order");

        let (baseline, _) = answer_at(0);
        prop_assert_eq!(
            kind,
            baseline,
            "the order changed the answer, and nothing in this fixture reads it"
        );
    }
}
