//! AC-026 (FR-026) — every registered invariant is called, exactly once, by one `verify`.
//!
//! AC-026 逐語: 「Given: モック`InvariantCheck`実装3件をregistryへ登録。When: `Gate::verify(input)`を
//! 呼ぶ。Then: 3件すべての`check`が正確に1回ずつ呼ばれたことがモックの呼び出しカウンタで確認できる。」
//! 判定方法: 「unit（mock検証）」.
//!
//! 「正確に1回ずつ」 has two failure modes and the counter sees both: zero (a registry that was
//! never consulted, or one whose loop skipped an entry) and two (a check asked once per system, or
//! once per arm of a fold). The criterion is stated on a single `verify`, so each test below counts
//! after exactly one call unless it says otherwise.
//!
//! The mocks answer `holds: true` unless a test needs otherwise, because AC-026 is about *being
//! called* and nothing else. What a violation does to the verdict is AC-029's second sentence
//! (`ac_029.rs`) and, in full, hand 4's AC-027.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::verdict::InvariantResult;
use gx_gate::{Gate, GateInput, InvariantCheck, InvariantRegistry, PolicyEngine, Result, Verdict};

// ---------------------------------------------------------------------------
// The mock the criterion asks for: an invariant with a call counter
// ---------------------------------------------------------------------------

/// One mock `InvariantCheck`, counting the calls it receives.
///
/// The counter is an `Arc<AtomicUsize>` rather than a `Cell` because 41 §4 requires
/// `InvariantCheck: Send + Sync` and `check` takes `&self`: a mock that needed a mutable borrow
/// could not be registered at all, which would make the criterion untestable through the real
/// trait. Sequential consistency is not needed for one thread and is what the test asserts about
/// anyway, so it is what is asked for.
struct CountingInvariant {
    id: String,
    calls: Arc<AtomicUsize>,
    holds: bool,
}

impl InvariantCheck for CountingInvariant {
    fn id(&self) -> &str {
        &self.id
    }

    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // The input is read rather than ignored: a mock that never touched its argument would pass
        // even if `run_all` handed every check a different, empty input.
        assert!(
            !input.pre.locator().is_empty(),
            "the check receives the real GateInput (E-M3-7)"
        );
        InvariantResult::new(self.id.clone(), self.holds, None)
    }
}

fn mock(id: &str, holds: bool) -> (Box<dyn InvariantCheck>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let check = CountingInvariant {
        id: id.to_string(),
        calls: Arc::clone(&calls),
        holds,
    };
    (Box::new(check), calls)
}

/// The three mocks AC-026 names, and the counters that watch them.
fn three_mocks() -> (InvariantRegistry, [Arc<AtomicUsize>; 3]) {
    let (first, a) = mock("aaa-first", true);
    let (second, b) = mock("bbb-second", true);
    let (third, c) = mock("ccc-third", true);
    let registry = InvariantRegistry::new()
        .with(first)
        .and_then(|r| r.with(second))
        .and_then(|r| r.with(third))
        .expect("three distinct ids");
    (registry, [a, b, c])
}

fn counts(counters: &[Arc<AtomicUsize>; 3]) -> Vec<usize> {
    counters
        .iter()
        .map(|counter| counter.load(Ordering::SeqCst))
        .collect()
}

// ---------------------------------------------------------------------------
// The gate the criterion calls
// ---------------------------------------------------------------------------

/// A policy set that admits everything, so that nothing about the counts depends on Cedar.
///
/// Written here rather than read from `policies/`: FR-028's pack is hand 5's, and AC-026 is about
/// the registry. `@id` is required by `PolicyEngine::parse`
/// (`req/62_M3_HAND2_REPORT_2026-08-08.md` §3.7).
const PERMIT_EVERYTHING: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

/// A policy set that refuses everything in `/etc/**`, in one statement.
const FORBID_ETC: &str = r#"@id("permit-everything")
permit (principal, action, resource);

@id("forbid-etc")
forbid (principal, action, resource)
when { resource.locator like "/etc/*" };
"#;

fn gate(policies: &str, invariants: InvariantRegistry) -> Gate {
    Gate::with_policies(PolicyEngine::parse(policies).expect("the test policy set parses"))
        .with_invariants(invariants)
}

fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

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

fn object(locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        SubstrateKind::Fs,
        locator.to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

// ---------------------------------------------------------------------------
// AC-026
// ---------------------------------------------------------------------------

#[test]
fn ac_026_three_registered_invariants_are_each_called_exactly_once() {
    let (registry, counters) = three_mocks();
    assert_eq!(
        counts(&counters),
        vec![0, 0, 0],
        "nobody has been asked yet"
    );

    let gate = gate(PERMIT_EVERYTHING, registry);
    let t = change();
    let pre = object("/tmp/x");
    let planned = PlannedDeltaBytes(b"opaque".to_vec());

    let verdict = gate
        .verify(GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: true,
        })
        .expect("a permissive set and three holding invariants reach a verdict");

    assert_eq!(
        counts(&counters),
        vec![1, 1, 1],
        "AC-026: 「3件すべての`check`が正確に1回ずつ呼ばれた」"
    );
    assert_eq!(verdict.kind(), VerdictKind::Admit);

    let Verdict::Admit(proof) = verdict else {
        panic!("everything held and the policy permitted")
    };
    let recorded: Vec<&str> = proof
        .invariant_results()
        .iter()
        .map(InvariantResult::invariant_id)
        .collect();
    assert_eq!(
        recorded,
        vec!["aaa-first", "bbb-second", "ccc-third"],
        "every answer is recorded, in invariant_id order (E-M3-9)"
    );
    assert!(
        proof.invariant_results().iter().all(InvariantResult::holds),
        "three holding invariants are three holding records"
    );
}

/// The count does not depend on what the policy said (FR-026 is unconditional).
///
/// FR-026 逐語: 「`check(pre, planned) -> Result<InvariantResult>` を**全登録invariant**に対し実行
/// しなければならない」. A registry that stopped once a policy had already refused would still pass
/// AC-026's own Given -- the criterion admits any input -- and would make an `AdmitProof`'s record
/// depend on which of the two systems was consulted first. So the refusing case is measured too.
#[test]
fn ac_026_the_count_is_the_same_when_the_policy_refuses() {
    let (registry, counters) = three_mocks();
    let gate = gate(FORBID_ETC, registry);
    let t = change();
    let pre = object("/etc/passwd");
    let planned = PlannedDeltaBytes(Vec::new());

    let verdict = gate
        .verify(GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &[],
            invert_available: false,
        })
        .expect("a refusal is a verdict, not an error");

    assert_eq!(verdict.kind(), VerdictKind::Deny);
    assert_eq!(
        counts(&counters),
        vec![1, 1, 1],
        "every registered invariant ran even though the policy had already refused"
    );
}

/// A second `verify` asks again. Nothing here is memoised, and an answer that was cached would be
/// an answer to an older input.
#[test]
fn ac_026_a_second_verify_asks_every_invariant_again() {
    let (registry, counters) = three_mocks();
    let gate = gate(PERMIT_EVERYTHING, registry);
    let t = change();
    let pre = object("/tmp/x");
    let planned = PlannedDeltaBytes(Vec::new());
    let input = || GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    };

    gate.verify(input()).expect("first");
    assert_eq!(counts(&counters), vec![1, 1, 1]);
    gate.verify(input()).expect("second");
    assert_eq!(
        counts(&counters),
        vec![2, 2, 2],
        "once per verify, not once per gate"
    );
}

/// The registry reports what it holds, in the order it was given.
///
/// Registration order is the call order, and the call order is what a trace shows (AC-029). It is
/// deliberately *not* the order the answers come back in -- see `invariant_registry.rs`.
#[test]
fn the_registry_reports_its_three_invariants_in_registration_order() {
    let (registry, _counters) = three_mocks();
    assert_eq!(registry.len(), 3);
    assert!(!registry.is_empty());
    assert_eq!(registry.ids(), vec!["aaa-first", "bbb-second", "ccc-third"]);
}

/// A gate that was given no invariants records none, and that is not the same as 「all held」.
#[test]
fn a_gate_with_an_empty_registry_records_no_invariant_results() {
    let gate = gate(PERMIT_EVERYTHING, InvariantRegistry::new());
    assert!(gate.invariants().is_empty());

    let t = change();
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
        .expect("evaluable");

    let Verdict::Admit(proof) = verdict else {
        panic!("a permissive set admits")
    };
    assert!(
        proof.invariant_results().is_empty(),
        "nobody was asked, and the record says so by holding nothing"
    );
}
