// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 Two-phase escrow's adapter half (`req/38` §98, ruling 1 / §99, ruling 1-2): the do-result (sem: SEM-gx-adapter-mcp-290)
//! vocabulary, the partial escrow, the completion, and the fail-safe folds.
//!
//! The measured need is `req/161` §1-§2: github-mcp-server v1.9.0's write results are
//! `{"id","url"}` with no `number` member, and upstream's own `MinimalResponse` doc comment
//! declares "all other information can be derived from the URL". So the sixth vocabulary word (sem: SEM-gx-adapter-mcp-291)
//! derives the trailing `/(\d+)$` of a pointed text as a JSON number, and every derivation
//! failure is `None` — an undo refused by name, never a wrong call.
//!
//! What this file holds:
//! * the derivation itself, on the two real URL shapes (`/issues/1`, `/pull/2`) and the refusals;
//! * `invert` building a **partial** delta (`Some`, pending members inside the payload) for a
//!   declared do-result pair — E-M4-5's fold still answers `invert_available = true`;
//! * `complete_inverse` minting the executable inverse from the observed result;
//! * the folds: an observation without the member, a non-JSON observation, a non-derivable URL;
//! * backward compatibility: a template with no do-result member resolves exactly as before, and
//!   the legacy payload's canonical bytes are **byte-identical** to the pre-two-phase grammar
//!   (the `pending` seat is absent from the wire when empty, so no existing delta's CID moves).

mod support;

use std::sync::Arc;

use gx_adapter_mcp::catalogue::trailing_number;
use gx_adapter_mcp::delta::{McpDelta, McpOp};
use gx_adapter_mcp::{ArgSource, Catalogue, McpAdapter, RestoreTemplate};
use gx_core::SubstrateKind;
use gx_substrate::{InverseCompletion, SubstrateAdapter};
use serde::Serialize;

use support::{FakeServer, SERVER, SUBJECT};

/// The issue pair's declaration, as the E2E catalogue spells it (`req/38` §99, ruling 1). (sem: SEM-gx-adapter-mcp-292)
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

/// A forward `issue_write(create)` delta at the fixture's subject.
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

fn adapter() -> McpAdapter {
    McpAdapter::new(Arc::new(FakeServer::new())).with_catalogue(issue_catalogue())
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
// The derivation (§99, ruling 1's `/(\d+)$`) (sem: SEM-gx-adapter-mcp-293)
// ---------------------------------------------------------------------------

/// The two real result shapes derive, and the refusals refuse — fail-safe, never a guess.
#[test]
fn the_trailing_number_derivation_accepts_the_two_real_shapes_and_refuses_the_rest() {
    // req/161 §1-§2's measured URLs, verbatim shapes.
    let issue = trailing_number("https://github.com/mahirhir/glovrex-a3-throwaway/issues/1");
    assert_eq!(issue.map(|n| n.as_u64()), Some(Some(1)));
    let pull = trailing_number("https://github.com/mahirhir/glovrex-a3-throwaway/pull/2");
    assert_eq!(pull.map(|n| n.as_u64()), Some(Some(2)));

    for (case, text) in [
        ("a non-numeric tail", "https://github.com/o/r/issues/1a"),
        ("a trailing slash", "https://github.com/o/r/issues/1/"),
        ("no digits at all", "https://github.com/o/r/issues/new"),
        ("an empty tail", "https://github.com/o/r/issues/"),
        ("no slash anywhere", "12345"),
        (
            "digits then more path",
            "https://github.com/o/r/issues/1/comments",
        ),
        ("the empty string", ""),
    ] {
        assert!(
            trailing_number(text).is_none(),
            "{case} ({text:?}) must derive nothing -- a derivation failure is a fail-safe by design (§99, ruling 1) (sem: SEM-gx-adapter-mcp-294)"
        );
    }
}

// ---------------------------------------------------------------------------
// The partial escrow (invert stays pure, pending travels in the payload)
// ---------------------------------------------------------------------------

/// A declared do-result pair escrows `Some(partial)`: every escrow-time member resolved, the
/// do-time member pending inside the payload, and the completion trait sees it.
#[test]
fn invert_builds_a_partial_delta_for_a_do_result_pair() {
    let adapter = adapter();
    let partial = partial_inverse(&adapter);

    let decoded = McpDelta::decode(partial.payload()).expect("the partial decodes");
    let op = decoded.ops().first().expect("one op");
    assert_eq!(op.tool(), "issue_write");
    let arguments: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("resolved arguments are JSON");
    assert_eq!(arguments["method"], "update");
    assert_eq!(arguments["owner"], "mahirhir");
    assert_eq!(arguments["repo"], "glovrex-a4-throwaway");
    assert_eq!(arguments["state"], "closed");
    assert!(
        arguments.get("issue_number").is_none(),
        "the do-time member is not resolvable before apply (E-M4-30's physics)"
    );
    assert_eq!(
        op.pending().len(),
        1,
        "exactly the declared do-result member is pending"
    );
    assert!(
        adapter
            .needs_completion(&partial)
            .expect("the partial is this adapter's grammar"),
        "the completion trait reads the pending map out of the escrowed value itself"
    );

    // E-M4-5's fold at verify time answers `true` — the whole point of declaring the pair.
    let complete_free = forward_issue_delta();
    let pre = adapter
        .snapshot(&format!("{SERVER}#{SUBJECT}"))
        .expect("readable");
    assert!(adapter
        .invert(&complete_free, &pre)
        .expect("answers")
        .inverse()
        .is_some());
}

/// `invert` is deterministic in (delta, pre) with a do-result member — the same partial twice,
/// which is what lets T-3's answer predict T-10b's (E-M4-5, the narrowed form §98, ruling 1 keeps). (sem: SEM-gx-adapter-mcp-295)
#[test]
fn the_partial_escrow_is_deterministic() {
    let adapter = adapter();
    let a = partial_inverse(&adapter);
    let b = partial_inverse(&adapter);
    assert_eq!(
        a.reference(),
        b.reference(),
        "two escrows of one (delta, pre) are one value"
    );
}

// ---------------------------------------------------------------------------
// The completion (§98, ruling 1 step 3) (sem: SEM-gx-adapter-mcp-296)
// ---------------------------------------------------------------------------

/// The real observed result (`req/161` §1's verbatim shape) completes the inverse: the number is
/// minted from the URL as a JSON **number**, the pending map is gone, and the delta is executable.
#[test]
fn complete_inverse_mints_the_executable_inverse_from_the_observed_result() {
    let adapter = adapter();
    let partial = partial_inverse(&adapter);
    let observation =
        br#"{"id":"5153620527","url":"https://github.com/mahirhir/glovrex-a4-throwaway/issues/1"}"#;

    let full = adapter
        .complete_inverse(&partial, observation)
        .expect("the partial is this adapter's grammar")
        .expect("the observation carries the URL the declaration names");

    let decoded = McpDelta::decode(full.payload()).expect("the completed delta decodes");
    let op = decoded.ops().first().expect("one op");
    assert!(
        op.pending().is_empty(),
        "a completed inverse owes nothing further"
    );
    let arguments: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("arguments are JSON");
    assert_eq!(
        arguments["issue_number"],
        serde_json::json!(1),
        "the derived member is a JSON number, not a string — `issue_write(update)`'s own type"
    );
    assert_eq!(arguments["state"], "closed");
    assert_ne!(
        full.reference(),
        partial.reference(),
        "completion mints a new delta; the partial stays what the journal escrowed"
    );
}

/// Every completion failure is `Ok(None)` — the fail-safe family, one phase after E-M4-32's.
#[test]
fn every_unresolvable_observation_folds_to_none() {
    let adapter = adapter();
    let partial = partial_inverse(&adapter);
    for (case, observation) in [
        (
            "a result with no url member",
            &br#"{"id":"5153620527"}"#[..],
        ),
        (
            "a url with no trailing number",
            &br#"{"id":"1","url":"https://github.com/o/r/issues/new"}"#[..],
        ),
        ("a non-JSON observation", &b"the server said words"[..]),
        ("a url that is not a string", &br#"{"id":"1","url":7}"#[..]),
    ] {
        assert!(
            adapter
                .complete_inverse(&partial, observation)
                .expect("still this adapter's grammar")
                .is_none(),
            "{case} must fold to None (§99, ruling 2-④), never mint a wrong call (sem: SEM-gx-adapter-mcp-297)"
        );
    }
}

// ---------------------------------------------------------------------------
// Backward compatibility (the wire does not move for anyone undeclared)
// ---------------------------------------------------------------------------

/// A template with no do-result member resolves exactly as before, and the completion trait calls
/// its escrow complete.
#[test]
fn a_template_without_do_result_members_is_complete_at_escrow_time() {
    let template = RestoreTemplate::new()
        .with("owner", ArgSource::Forward("owner".to_string()))
        .with("state", ArgSource::Const("closed".to_string()));
    let (arguments, pending) = template
        .resolve_split(br#"{"owner":"mahirhir"}"#, b"")
        .expect("escrow-time members resolve");
    assert!(pending.is_empty(), "nothing is owed after apply");
    let resolved: serde_json::Value = serde_json::from_slice(&arguments).expect("JSON");
    assert_eq!(resolved["owner"], "mahirhir");
    assert_eq!(resolved["state"], "closed");

    // `resolve` (the pre-two-phase surface) answers the same bytes for the same declaration.
    let one_phase = template
        .resolve(br#"{"owner":"mahirhir"}"#, b"")
        .expect("a do-result-free template resolves in one phase");
    assert_eq!(one_phase, arguments);
}

/// `resolve` refuses a do-result member by name — the one-phase surface cannot silently hand out
/// a partial as if it were complete.
#[test]
fn the_one_phase_resolve_refuses_do_result_members_by_name() {
    let template = RestoreTemplate::new().with(
        "issue_number",
        ArgSource::DoResultNumberFrom("/url".to_string()),
    );
    let refused = template
        .resolve(br#"{}"#, b"")
        .expect_err("a do-result member cannot resolve before apply");
    assert!(
        refused.contains("issue_number"),
        "the refusal names the member: {refused}"
    );
}

/// 🔴 The legacy raw-byte assert: an op with no pending members encodes to **the same canonical
/// DAG-CBOR bytes** as the pre-two-phase grammar (`{arguments, locator, tool}` and nothing else),
/// so no delta that exists today changes its CID. The legacy shape is redeclared here, locally,
/// and encoded through the same gx-canon road — a real byte comparison, not a claim.
#[test]
fn a_pending_free_op_encodes_byte_identically_to_the_legacy_grammar() {
    #[derive(Serialize)]
    struct LegacyOp<'a> {
        arguments: gx_adapter_mcp::delta::Bytes,
        locator: &'a str,
        tool: &'a str,
    }
    let locator = format!("{SERVER}#{SUBJECT}");
    let arguments = br#"{"uri":"file:///srv/notes.md","contents":"before"}"#.to_vec();

    let today = McpDelta::one(McpOp::call(
        locator.clone(),
        "notes.restore".to_string(),
        arguments.clone(),
    ))
    .encode()
    .expect("encodes");
    let legacy = gx_canon::cbor::encode(&vec![LegacyOp {
        arguments: gx_adapter_mcp::delta::Bytes(arguments),
        locator: &locator,
        tool: "notes.restore",
    }])
    .expect("the legacy shape encodes");
    assert_eq!(
        today, legacy,
        "the pending seat must be absent from the wire when empty — a moved byte here would move \
         every existing delta's CID"
    );

    // And a legacy payload decodes into today's type, owing nothing.
    let decoded = McpDelta::decode(&legacy).expect("legacy bytes decode");
    assert!(decoded.ops()[0].pending().is_empty());
}
