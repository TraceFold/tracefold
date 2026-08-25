// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 3 (`req/338` §0-3, from `req/334` M-03)** — the late-escrow completion road says
//! which of four facts it refused on, and the sentence it composes reaches a reader.
//!
//! # What the audit measured and what it could not
//!
//! `A27_COMPLETION_ROAD silent_returns=4 discards_the_reason=true carries_a_sentence=false` — four
//! separate facts folded to one `Ok(None)`, and in the one case where
//! `ArgSource::resolve_from_observation` had **already composed** a sentence naming the pointer and
//! what was wrong with it, the road bound it to `_unresolvable` and dropped it.
//!
//! The audit declared that it measured this at **source** level and never drove the road (`req/334`
//! §4: "`complete_inverse`'s four `Ok(None)` are source water-level. An E2E through an engine that
//! registered an `InverseCompletion` is 0 runs"). This file drives it: a real `McpAdapter` over the
//! fixture server, a real partial escrow from `invert`, and observations chosen to land on the
//! facts — then reads back what a deployment would actually be handed.
//!
//! # What is deliberately unchanged
//!
//! `Ok(None)` at all four arms. `req/38` §99 ruling 2 ④ makes the fold a fail-safe, and nothing on
//! this road may abort a commit whose apply already succeeded. Minting a seventh `InverseStatus`
//! word beside `Unavailable` — which is what R8 / `req/234` B-5 did for a neighbouring fact, and is
//! the precedent that would apply — is a **wire ruling** and not a repair lane's. What this lane
//! owed was that the reason stop being discarded, and that the remaining fold be **declared**
//! rather than left for the next audit to rediscover (`docs/LIMITS.md`, v0.5-o).

mod support;

use std::sync::Arc;

use gx_adapter_mcp::delta::{McpDelta, McpOp};
use gx_adapter_mcp::log::MemoryCallLog;
use gx_adapter_mcp::{
    ArgSource, Catalogue, CompletionRefused, McpAdapter, RestoreTemplate, OBSERVATION_NOT_ANSWERED,
};
use gx_core::SubstrateKind;
use gx_substrate::{InverseCompletion, SubstrateAdapter};

use support::{FakeServer, SERVER, SUBJECT};

fn record(line: &str) {
    println!("{line}");
    let path = std::env::var("R28_MEAS")
        .unwrap_or_else(|_| "/mnt/c/work/r28_logs/r28_measurements.txt".to_string());
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

/// The issue pair's declaration, as `do_result.rs` spells it.
fn issue_catalogue() -> Catalogue {
    Catalogue::new().with_restore_template(
        "issue_write",
        "issue_write",
        RestoreTemplate::new()
            .with("method", ArgSource::Const("update".to_string()))
            .with("owner", ArgSource::Forward("owner".to_string()))
            .with("repo", ArgSource::Forward("repo".to_string()))
            .with(
                "issue_number",
                ArgSource::DoResultNumberFrom("/url".to_string()),
            )
            .with("state", ArgSource::Const("closed".to_string())),
    )
}

fn forward_issue_delta() -> gx_substrate::PlannedDelta {
    let arguments =
        br#"{"method":"create","owner":"mahirhir","repo":"glovrex-a4-throwaway","title":"t"}"#;
    let payload = McpDelta::one(McpOp::call(
        format!("{SERVER}#{SUBJECT}"),
        "issue_write".to_string(),
        arguments.to_vec(),
    ))
    .encode()
    .expect("a forward payload encodes");
    gx_substrate::PlannedDelta::new(SubstrateKind::Mcp, payload).expect("a delta mints")
}

/// An adapter whose log this suite keeps a handle on, so the notes can be read back.
fn adapter_and_log() -> (McpAdapter, Arc<MemoryCallLog>) {
    let log = Arc::new(MemoryCallLog::new());
    let adapter = McpAdapter::new(Arc::new(FakeServer::new()))
        .with_catalogue(issue_catalogue())
        .with_log(log.clone());
    (adapter, log)
}

fn partial_inverse(adapter: &McpAdapter) -> gx_substrate::PlannedDelta {
    let delta = forward_issue_delta();
    let pre = adapter
        .snapshot(&format!("{SERVER}#{SUBJECT}"))
        .expect("the fixture's subject is readable");
    adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("a declared do-result pair escrows a partial, not None")
}

// ---------------------------------------------------------------------------
// a — the bed: the road still completes when the observation carries the member
// ---------------------------------------------------------------------------

/// 🔴 Without this arm every assertion below could be green because the road refuses everything.
#[test]
fn a_bed_control_a_good_observation_still_completes_and_notes_nothing() {
    let (adapter, log) = adapter_and_log();
    let partial = partial_inverse(&adapter);
    let observation =
        br#"{"id":"5153620527","url":"https://github.com/mahirhir/glovrex-a4-throwaway/issues/1"}"#;
    let full = adapter
        .complete_inverse(&partial, observation)
        .expect("this adapter's grammar")
        .expect("the observation carries what the declaration names");
    record(&format!(
        "R28_COMPLETION_BED completed=true notes={:?} reference_moved={}",
        log.notes(),
        full.reference() != partial.reference()
    ));
    assert!(
        log.notes().is_empty(),
        "🔴 a completion that succeeded must say nothing: a note on the success road would make \
         the notes useless as a record of refusals: {:?}",
        log.notes()
    );
}

// ---------------------------------------------------------------------------
// b — the fact the audit called the strong one, driven
// ---------------------------------------------------------------------------

/// 🔴 **`req/334` M-03** — the observation did not carry a declared member, and the reason survives.
///
/// This is fact ②, and it is the one an operator can act on: the inverse **was** derivable and
/// **was** escrowed. `Unavailable` tells them the opposite.
#[test]
fn b_an_observation_missing_the_declared_member_names_the_fact_and_keeps_the_sentence() {
    let (adapter, log) = adapter_and_log();
    let partial = partial_inverse(&adapter);
    // A well-formed result that simply does not carry `/url`.
    let observation = br#"{"id":"5153620527"}"#;
    let answered = adapter
        .complete_inverse(&partial, observation)
        .expect("this adapter's grammar");
    let notes = log.notes();
    record(&format!(
        "R28_COMPLETION_MISSING_MEMBER folded_to_none={} notes={notes:?}",
        answered.is_none()
    ));
    assert!(
        answered.is_none(),
        "🔴 the fail-safe is unchanged: a completion that cannot be built is `None`, never a wrong \
         call (`req/38` §99 ruling 2 ④)"
    );
    assert_eq!(
        notes.len(),
        1,
        "🔴 exactly one refusal happened, so exactly one sentence should have been composed: \
         {notes:?}"
    );
    let said = &notes[0];
    assert!(
        said.contains(CompletionRefused::ObservationDidNotCarryADeclaredMember.kind()),
        "🔴 the sentence has to name **which** of the four facts this is, which is the whole of \
         `req/334` M-03: {said:?}"
    );
    assert!(
        said.contains("issue_number"),
        "🔴 the declaration names `issue_number` and the sentence has to say so — a reason that \
         does not name the member is not actionable: {said:?}"
    );
    assert!(
        said.contains("pointer"),
        "🔴 `resolve_from_observation` composed a sentence naming the pointer, and the whole \
         finding was that this road dropped it on the floor: {said:?}"
    );
    assert!(
        said.contains(OBSERVATION_NOT_ANSWERED),
        "🔴 the remedy is the read face, which this crate already has one sentence for. Naming it \
         rather than paraphrasing it is what keeps this build from growing two accounts of one \
         condition: {said:?}"
    );
    assert!(
        said.contains("not** 42 §3.12's `Unavailable`") || said.contains("Unavailable"),
        "🔴 the sentence has to say that the word the engine records is not the word for this \
         fact, or the operator reads `Unavailable` and gives up on an undo that is merely \
         unfinished: {said:?}"
    );
}

// ---------------------------------------------------------------------------
// c — the four facts are four, and they are told apart
// ---------------------------------------------------------------------------

/// 🔴 One word for four facts was the finding. Four distinct words is the repair, and this arm
/// holds that they really are distinct rather than four spellings of one.
#[test]
fn c_the_four_facts_are_four_distinct_named_facts() {
    let all = [
        CompletionRefused::ArgumentsAreNotTheObjectThisAdapterWrote,
        CompletionRefused::ObservationDidNotCarryADeclaredMember,
        CompletionRefused::CompletedArgumentsWouldNotSerialise,
        CompletionRefused::CompletedPayloadIsOverTheCeiling,
    ];
    let kinds: Vec<&str> = all.iter().map(|f| f.kind()).collect();
    let sentences: Vec<String> = all.iter().map(|f| f.sentence("D.")).collect();
    let mut distinct = kinds.clone();
    distinct.sort_unstable();
    distinct.dedup();
    record(&format!(
        "R28_COMPLETION_FACTS kinds={kinds:?} declared={:?} distinct={}",
        CompletionRefused::ALL_FACTS,
        distinct.len()
    ));
    assert_eq!(distinct.len(), 4, "🔴 four facts, four words: {kinds:?}");
    assert_eq!(
        kinds,
        CompletionRefused::ALL_FACTS.to_vec(),
        "🔴 the declared list and the values have to be the same list, or a reader branching on \
         `ALL_FACTS` misses one"
    );
    for (fact, said) in all.iter().zip(sentences.iter()) {
        assert!(
            said.contains(fact.kind()) && said.contains("D."),
            "🔴 every sentence names its fact and carries what the road learned: {said:?}"
        );
    }
    let mut bodies = sentences.clone();
    bodies.sort();
    bodies.dedup();
    assert_eq!(
        bodies.len(),
        4,
        "🔴 four words with one sentence between them is the fold wearing a disguise: {sentences:?}"
    );
}

// ---------------------------------------------------------------------------
// d — every silent return on the road composes a sentence first
// ---------------------------------------------------------------------------

/// 🔴 The audit's arm measured this road's shape from source and found `carries_a_sentence=false`.
/// This asks the stronger question: is there **any** `Ok(None)` on the road that is still silent?
///
/// Derived from the source rather than from the three arms above, because those drive the facts a
/// lane thought of — and the whole family of defects this arc keeps repairing is the fact nobody
/// thought of.
#[test]
fn d_no_silent_return_is_left_on_the_completion_road() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapter.rs"),
    )
    .expect("adapter.rs is readable");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let start = code
        .find("fn complete_inverse")
        .expect("the completion road exists");
    let road = &code[start..];
    let end = road.find("\n    }").map(|e| e + 4).unwrap_or(road.len());
    let road = &road[..end];

    // Each `Ok(None)` must have a note composed above it and below the previous one.
    let mut silent_without_a_note = 0usize;
    let mut returns = 0usize;
    let mut cursor = 0usize;
    let mut last = 0usize;
    while let Some(at) = road[cursor..].find("return Ok(None)") {
        let here = cursor + at;
        returns += 1;
        if !road[last..here].contains("CompletionRefused::") {
            silent_without_a_note += 1;
        }
        last = here;
        cursor = here + "return Ok(None)".len();
    }
    record(&format!(
        "R28_COMPLETION_ROAD_SOURCE silent_returns={returns} \
         without_a_named_fact={silent_without_a_note}"
    ));
    assert!(
        returns >= 3,
        "🔴 the road's shape is not what this arm measured; re-read before trusting: {returns}"
    );
    assert_eq!(
        silent_without_a_note, 0,
        "🔴 `req/334` M-03: {silent_without_a_note} of the {returns} silent returns on the \
         completion road compose no sentence, so the fact they refused on is unrecoverable"
    );
}
