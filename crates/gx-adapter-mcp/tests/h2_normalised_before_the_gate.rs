//! 🔴 **H-2** — the locator this adapter hands a gate is the **normalised** one (**M7 hand 4**).
//!
//! `req/38` §25's H-2 was ruled 採(案 c 両方) and its first half was a ticket: 「M4 adapter 契約の必須
//! ticket「locator は正規化済みで gate に渡す」を予約(M7 の G-4/H-9 と並ぶ M4 reqdef 必須項目)」, with
//! the second half a line in `packs.rs` saying that a gate compares the string it is given. req/98
//! §3-4 row 3 is what M7 owes on it: 「git/mcp adapter が locator を正規化してから gate に渡す事を
//! **adapter 側の probe** で固定(gate 内正規化は不採)」.
//!
//! `gx-substrate`'s `substrate_contract.rs` measures the **contract's words** and `gx-adapter-fs`'s
//! `locator_normalisation.rs` measures **the function**. Neither of them measures the sentence H-2
//! actually makes, which is about a *road*: the locator a policy is evaluated against is the one an
//! adapter put in an `ObjectSnapshot`, so 「normalised」 has to hold at the point of handover and not
//! only inside a helper.
//!
//! # The road, in one line
//!
//! `Engine::plan` calls `adapter.snapshot(intent.locator())` and hands the result to
//! `Gate::verify` as `pre` (`crates/gx-engine/src/pipeline.rs`), and `RequestView` reads
//! `pre.locator()` into Cedar's `resource.locator`. So the two facts below compose into the claim:
//!
//! 1. [`snapshot_reports_the_normal_form_of_whatever_spelling_it_was_asked_about] — the value that
//!    reaches `pre` is `locator::normalize`'s output, for every clause of RFC 3986 §6.2.2;
//! 2. [`the_shipped_pack_refuses_a_spelling_that_would_have_evaded_it`] — a spelling that does not
//!    match the shipped pack's pattern is refused anyway, because by the time a gate sees it, it is
//!    the normal form.
//!
//! # 🔴 And the third fact, which is the one that says whose job this is
//!
//! [`the_gate_normalises_nothing_and_that_is_why_this_probe_is_here`] hands a gate the **raw**
//! spelling directly and watches it be admitted. 「gate 内正規化は不採」 (req/98 §3-4) is not a
//! preference: a path algebra inside a policy layer would be an invention the spec does not carry and
//! a second definition of what a locator means (`packs.rs` argues it at length).
//!
//! 🔴 **That is the same fact `crates/gx-gate/tests/false_admit.rs::
//! the_pack_judges_the_locator_as_given_not_as_resolved` pins on the fs substrate, and it is a fact
//! about the gate rather than about the fs adapter.** The fs adapter folds `..` as well — E-M4-12
//! makes lexical normalisation a contract for *every* adapter — so that probe reaches the gate with
//! an unfolded spelling only because it builds the `ObjectSnapshot` itself instead of asking
//! `snapshot` for one. Reading it as 「fs is broken and mcp is not」 would be reading a probe's
//! construction as a substrate's property, so this file says the opposite out loud: what these three
//! probes measure is the **handover**, and it holds on both M7 adapters for the same reason it holds
//! on the M4 one.
//!
//! What differs per substrate is only what a *lexical* normal form can reach: fs leaves 45 TH-2's
//! symlink open, git resolves an unqualified reference name textually rather than against the
//! reference store, and this adapter leaves RFC 3986 §6.2.3/§6.2.4 undone on purpose.

mod support;

use gx_adapter_mcp::locator;
use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::{packs, Gate, GateInput};
use gx_substrate::SubstrateAdapter;
use support::McpFixture;

/// A resource under `/etc`, reachable through a server: the thing the shipped mcp pack refuses.
const ETC: &str = "file:///etc/passwd";

/// Spellings of one position, one per clause of RFC 3986 §6.2.2, and what each one is for.
///
/// The server is spelled `HTTPS://MCP.Example` in three of the four so that clauses 1 and 2 are
/// exercised on the endpoint as well as clauses 3 and 4 on the resource — the crate root applies the
/// four clauses to **both** parts, and a normaliser that folded only one would pass a table that
/// only varied one.
fn spellings() -> Vec<(&'static str, String, &'static str)> {
    vec![
        (
            "clause 1 (§3.1): the scheme folds",
            format!("HTTPS://mcp.example/sse#{ETC}"),
            "a policy written on `https://` is not in force on `HTTPS://` unless something folds it",
        ),
        (
            "clause 2 (§6.2.2.1): the host folds",
            format!("https://MCP.Example/sse#{ETC}"),
            "the same, one component along",
        ),
        (
            "clause 3 (§6.2.2.2): an unreserved triplet decodes",
            "https://mcp.example/sse#file:///%65tc/passwd".to_string(),
            "`%65` is `e`, and a pattern matching `/etc/` does not match `/%65tc/`",
        ),
        (
            "clause 4 (§6.2.2.3): dot segments fold",
            "https://mcp.example/sse#file:///srv/../etc/passwd".to_string(),
            "the fs pack's known false admit, in this substrate's grammar",
        ),
    ]
}

/// The fixture's server, holding the subject **and** a resource under `/etc`.
fn fixture_with_an_etc_resource() -> McpFixture {
    let fixture = McpFixture::new();
    fixture
        .server()
        .write_behind_the_adapter(ETC, b"root:x:0:0:root:/root:/bin/sh\n");
    fixture
}

fn change() -> Transformation {
    Transformation::new(
        TransformationId(Cid([0u8; 32])),
        0,
        Subject::Object(ObjectId(Cid([1u8; 32]))),
        None,
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(Cid([1u8; 32])),
            delta: DeltaRef {
                substrate: SubstrateKind::Mcp,
                cid: Cid([1u8; 32]),
            },
            context: ChangeContext::Substrate,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("orders 0..=2 are admitted")
}

/// What the shipped mcp pack answers about a snapshot.
fn verdict_for(gate: &Gate, pre: &ObjectSnapshot) -> VerdictKind {
    let t = change();
    let planned = PlannedDeltaBytes(b"opaque to everything below the adapter (P-6)".to_vec());
    gate.verify(GateInput {
        t: &t,
        pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
    })
    .expect("the shipped pack evaluates this request")
    .kind()
}

/// A snapshot carrying a locator **exactly as written** — the value an adapter that did not
/// normalise would hand a gate.
fn snapshot_spelled(locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(Cid([1u8; 32])),
        SubstrateKind::Mcp,
        locator.to_string(),
        Cid([2u8; 32]),
        ReprKind::Bytes,
    )
}

// ---------------------------------------------------------------------------
// 1. The value that reaches `pre`
// ---------------------------------------------------------------------------

/// Whatever spelling `snapshot` is asked about, the snapshot it returns names the **normal form**.
///
/// 41 §4 has `snapshot` receive a locator already normalised (H-2 / E-M4-12), and the adapter
/// normalises again anyway — free, by L7's idempotence, and it is what stops a caller's spelling from
/// reaching a gate through a snapshot. This probe is that second sentence, measured.
#[test]
fn snapshot_reports_the_normal_form_of_whatever_spelling_it_was_asked_about() {
    let fixture = fixture_with_an_etc_resource();
    let normal = format!("https://mcp.example/sse#{ETC}");
    let mut reported = Vec::new();
    for (clause, raw, _) in spellings() {
        let snap = fixture
            .mcp()
            .snapshot(&raw)
            .unwrap_or_else(|e| panic!("{clause}: {raw} — {e}"));
        assert_eq!(
            snap.locator(),
            normal,
            "{clause}: `snapshot` handed on {:?} instead of the normal form",
            snap.locator()
        );
        assert_eq!(
            locator::normalize(&raw),
            normal,
            "{clause}: and the function agrees with the method"
        );
        reported.push(clause);
    }
    println!(
        "H2_MCP_SNAPSHOT_NORMALISES clauses={} normal={normal:?}",
        reported.len()
    );
    assert_eq!(reported.len(), 4, "one spelling per clause of §6.2.2");
}

// ---------------------------------------------------------------------------
// 2. What that buys at the gate
// ---------------------------------------------------------------------------

/// 🔴 The shipped pack refuses every spelling, and two of the four would have evaded its patterns.
///
/// This is H-2's whole point stated as a consequence rather than as a contract: M3-10 fixes a pack's
/// effective range at 「locator 級」, so two spellings of one position are **two policy subjects**, and
/// an adapter that handed on the caller's spelling would be handing a policy author a rule that is
/// not in force on the position they wrote it about.
#[test]
fn the_shipped_pack_refuses_a_spelling_that_would_have_evaded_it() {
    let fixture = fixture_with_an_etc_resource();
    let gate = Gate::with_policies(packs::mcp_pack().expect("the shipped mcp pack parses"));
    let mut evaded_raw = 0usize;
    for (clause, raw, why) in spellings() {
        let snap = fixture.mcp().snapshot(&raw).expect("the fixture holds it");
        assert_eq!(
            verdict_for(&gate, &snap),
            VerdictKind::Deny,
            "{clause}: the pack must refuse this position however it was spelled ({why})"
        );
        // The counterfactual, in the same run: the raw spelling, handed to the same gate.
        if verdict_for(&gate, &snapshot_spelled(&raw)) != VerdictKind::Deny {
            evaded_raw += 1;
        }
    }
    println!("H2_MCP_GATE spellings=4 denied_after_snapshot=4 would_have_evaded_raw={evaded_raw}");
    assert!(
        evaded_raw >= 2,
        "a probe in which no raw spelling evades the pack is a probe measuring nothing: the point \
         of normalising is that clauses 3 and 4 change the answer"
    );
}

/// 🔴 The gate normalises nothing, and that is why the probe above is in **this** crate.
///
/// 「gate 内正規化は不採」 (req/38 §25 H-2, req/98 §3-4): a path algebra inside a policy layer would be
/// an invention 42 §3.1 does not carry, and Cedar's `like` cannot express one either (`*` matches any
/// run of characters, `..` included). So the responsibility is the adapter's, and the way to show a
/// responsibility is somebody's is to show what happens when they do not discharge it.
#[test]
fn the_gate_normalises_nothing_and_that_is_why_this_probe_is_here() {
    let gate = Gate::with_policies(packs::mcp_pack().expect("the shipped mcp pack parses"));
    let evading = "https://mcp.example/sse#file:///srv/../etc/passwd";
    let answer = verdict_for(&gate, &snapshot_spelled(evading));
    println!("H2_MCP_GATE_IS_NOT_A_NORMALISER spelling={evading:?} answer={answer:?}");
    assert_eq!(
        answer,
        VerdictKind::Admit,
        "recorded, not endorsed: if this becomes Deny, something in gx-gate resolves locators now, \
         and req/38 §25's H-2 ruling (案 c: the ticket goes to the adapter contract, and the gate \
         states that it judges the spelling it is given) has been reversed without being re-ruled"
    );
    // And the normal form of that same spelling is refused, by the same gate, in the same run.
    assert_eq!(
        verdict_for(&gate, &snapshot_spelled(&locator::normalize(evading))),
        VerdictKind::Deny,
        "the pack is not indifferent to the position — only to the spelling"
    );
}
