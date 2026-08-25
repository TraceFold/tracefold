// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The grammar, the locator, and the two constants — including the escrow ceiling `gx-adapter-git`
//! argued it could not have.
//!
//! Spec: 42 §3.4 for the payload, 42 §2.1 for the canonical form, 42 §2.3 for `≈`. The rulings are
//! **M4-13, adopted (a)** (`MAX_OPS`), **M4-07, adopted (c)** (the free monoid), **M4H5-4, adopted (b)** (the forward bound), (sem: SEM-gx-adapter-mcp-326)
//! **M4-21, adopted (a)** (the escrow bound), **M4H5-5, adopted (b)** ("the argument is not a position" is not "the apply failed"), (sem: SEM-gx-adapter-mcp-327)
//! **M4H4-1** (`ScopeTooLong` at construction) and **M4H4-2** (`Unimplemented` for what v0.1 does not
//! run).

mod support;

use gx_adapter_mcp::{
    normalize, restore_arguments, McpDelta, McpOp, ToolIntent, MAX_FORWARD_PAYLOAD_BYTES,
    MAX_INVERSE_PAYLOAD_BYTES, MAX_OPS,
};
use gx_substrate::SubstrateAdapter;
use support::{
    absent_snapshot, intent_for, locator_on, McpFixture, NOTIFY_TOOL, SERVER, SUBJECT, WRITE_TOOL,
};

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-adapter-mcp")
        .to_path_buf()
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a directory is readable") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The locator
// ---------------------------------------------------------------------------

/// The four refusals of [`gx_adapter_mcp::locator`], each "the argument is not a position" rather than "the apply failed". (sem: SEM-gx-adapter-mcp-328)
///
/// **M4H5-5, adopted (b)** is the ruling and 43 T-11 is the reason: `ApplyFailed` becomes (sem: SEM-gx-adapter-mcp-329)
/// `AbortReason::ApplyFailed`, which would record a change that failed where no change was ever
/// describable.
#[test]
fn a_spelling_that_is_not_a_position_is_refused_as_one() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let mut refusals: Vec<(&str, String)> = Vec::new();
    for (why, locator) in [
        ("no separator", "https://mcp.example/sse".to_string()),
        ("empty resource", format!("{SERVER}#")),
        ("no scheme on the server", format!("mcp-1#{SUBJECT}")),
        (
            "a `#` in the resource",
            format!("{SERVER}#file:///srv/notes.md#section-2"),
        ),
    ] {
        let error = adapter
            .snapshot(&locator)
            .expect_err("this spelling is not a position");
        refusals.push((why, error.kind().to_string()));
    }
    println!("MCP_NOT_A_POSITION {refusals:?}");
    for (why, kind) in &refusals {
        assert_eq!(
            kind, "NotAPosition",
            "\"{why}\" answered {kind} rather than \"that is not a place\" (sem: SEM-gx-adapter-mcp-330)"
        );
    }
    assert_eq!(refusals.len(), 4);
}

/// 🔴 **Reservation 6** (req/98 §3-4): an over-long scope is refused **at construction**, on the same one road (sem: SEM-gx-adapter-mcp-331)
/// the other two adapters take.
///
/// `req/38` §32 M4H4-1: the locator reaches [`gx_core::Fingerprint::new`] through
/// `gx_substrate::elide_scope`, whose whole job is that a scope past [`gx_core::MAX_SCOPE_BYTES`]
/// becomes a digest line **before** the type exists. So this probe asserts a long position still
/// produces a fingerprint (the road is the eliding one) and that the value it produces is inside the
/// bound.
#[test]
fn an_over_long_position_takes_the_one_eliding_road() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();

    let long = format!("file:///srv/{}.md", "a".repeat(gx_core::MAX_SCOPE_BYTES));
    let locator = locator_on(SERVER, &long);
    assert!(locator.len() > gx_core::MAX_SCOPE_BYTES);

    // The server does not hold it, so `precondition` needs a snapshot the fixture builds. What is
    // measured is the road, not the read: a `Fingerprint` exists for a position longer than the bound.
    let snapshot = absent_snapshot(&locator);
    let elided = gx_substrate::elide_scope(normalize(&locator)).expect("the one road elides");
    println!(
        "MCP_SCOPE_ELISION raw={} elided={} max={}",
        locator.len(),
        elided.len(),
        gx_core::MAX_SCOPE_BYTES
    );
    assert!(
        elided.len() <= gx_core::MAX_SCOPE_BYTES,
        "the elided scope is still over the bound, so `Fingerprint::new` would refuse it"
    );
    // And the adapter refuses to *read* it for the ordinary reason (no such resource), not because the
    // scope broke: the two failures are told apart by their kind.
    let error = adapter
        .precondition(&snapshot)
        .expect_err("this fixture's server holds no such resource");
    println!("MCP_SCOPE_ELISION_KIND {}", error.kind());
    assert_eq!(error.kind(), "Unreadable");
}

// ---------------------------------------------------------------------------
// The grammar
// ---------------------------------------------------------------------------

/// `MAX_OPS` is unsupported and the empty sequence is not a payload, and the two are spelled differently. (sem: SEM-gx-adapter-mcp-332)
///
/// **M4H4-2**: "not implemented" and "failed" are permanently different facts, and the shared harness reads (sem: SEM-gx-adapter-mcp-333)
/// [`gx_substrate::Error::Unimplemented`] as "none" and nothing else. A grammar that answered (sem: SEM-gx-adapter-mcp-334)
/// `PayloadUnreadable` for a two-operation sequence would tell a harness that the adapter is broken
/// where the truth is that v0.1 does not run it.
#[test]
fn the_two_refusals_of_decode_are_two_words() {
    let op = || {
        McpOp::call(
            locator_on(SERVER, SUBJECT),
            WRITE_TOOL.to_string(),
            b"x".to_vec(),
        )
    };

    let too_many = McpDelta::of(vec![op(), op()]).encode().expect("encodes");
    let none = McpDelta::of(Vec::new()).encode().expect("encodes");

    let long = McpDelta::decode(&too_many).expect_err("v0.1 runs one operation");
    let empty = McpDelta::decode(&none).expect_err("the unit describes no change");
    println!(
        "MCP_DECODE max_ops={MAX_OPS} too_many={} empty={}",
        long.kind(),
        empty.kind()
    );
    assert_eq!(long.kind(), "Unimplemented");
    assert_eq!(empty.kind(), "PayloadUnreadable");

    let nameless = McpDelta::one(McpOp::call(
        locator_on(SERVER, SUBJECT),
        String::new(),
        Vec::new(),
    ))
    .encode()
    .expect("encodes");
    assert_eq!(
        McpDelta::decode(&nameless)
            .expect_err("an operation that names no tool is not this grammar's")
            .kind(),
        "PayloadUnreadable"
    );
}

/// The monoid is over the **sequences**, and concatenation is associative there.
///
/// **M4-07, adopted (c)** with **N-14**: a CBOR array carries its length in its head, so two payloads do not (sem: SEM-gx-adapter-mcp-335)
/// concatenate as bytes. The operation is `decode`, concatenate, `encode`.
#[test]
fn concatenation_is_associative_over_the_sequences() {
    let at = |n: u8| McpOp::call(locator_on(SERVER, SUBJECT), WRITE_TOOL.to_string(), vec![n]);
    let (a, b, c) = (vec![at(1)], vec![at(2)], vec![at(3)]);

    let left = McpDelta::of([a.clone(), b.clone()].concat());
    let left = McpDelta::of([left.ops().to_vec(), c.clone()].concat());
    let right = McpDelta::of([b, c].concat());
    let right = McpDelta::of([a, right.ops().to_vec()].concat());

    assert_eq!(left, right);
    assert_eq!(
        left.encode().expect("encodes"),
        right.encode().expect("encodes"),
        "two spellings of one sequence encode to two byte strings, so the monoid is not free over \
         the canonical form"
    );
}

/// A goal that is not this adapter's grammar is "no delta plans this intent" and not a broken payload. (sem: SEM-gx-adapter-mcp-336)
#[test]
fn a_goal_that_is_not_a_tool_call_is_not_plannable() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = locator_on(SERVER, SUBJECT);
    let pre = absent_snapshot(&locator);

    let nonsense = gx_core::Intent::new(
        gx_core::SubstrateKind::Mcp,
        locator.clone(),
        gx_core::GoalBytes(b"not cbor at all".to_vec()),
        gx_core::ChangeContext::Policy,
        gx_core::Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    );
    let refusal = adapter
        .plan(&nonsense, &pre)
        .expect_err("the goal is not `{arguments, tool}`");
    println!("MCP_GOAL_GRAMMAR {}", refusal.kind());
    assert_eq!(refusal.kind(), "NotPlannable");

    // And an intent for another substrate is the same word for a different reason.
    let elsewhere = gx_core::Intent::new(
        gx_core::SubstrateKind::Fs,
        locator,
        gx_core::GoalBytes(
            ToolIntent::new(WRITE_TOOL, b"x".to_vec())
                .encode()
                .expect("encodes"),
        ),
        gx_core::ChangeContext::Policy,
        gx_core::Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    );
    assert_eq!(
        adapter
            .plan(&elsewhere, &pre)
            .expect_err("another substrate's intent")
            .kind(),
        "NotPlannable"
    );
}

// ---------------------------------------------------------------------------
// The two ceilings
// ---------------------------------------------------------------------------

/// The forward ceiling refuses, and the refusal is `plan`'s own word.
#[test]
fn the_forward_ceiling_is_the_number_the_source_declares() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = locator_on(SERVER, SUBJECT);
    let pre = absent_snapshot(&locator);
    let plan_of = |size: usize| {
        adapter
            .plan(&intent_for(&locator, WRITE_TOOL, &vec![b'g'; size]), &pre)
            .map(|d| d.payload().len())
            .map_err(|e| e.kind().to_string())
    };

    let under = plan_of(MAX_FORWARD_PAYLOAD_BYTES - 4096);
    let over = plan_of(MAX_FORWARD_PAYLOAD_BYTES + 1);
    println!("MCP_FORWARD_CEILING={MAX_FORWARD_PAYLOAD_BYTES} UNDER={under:?} OVER={over:?}");
    let accepted = under.expect("a call under the ceiling is plannable");
    assert!(accepted <= MAX_FORWARD_PAYLOAD_BYTES);
    assert_eq!(
        over.expect_err("a call over the ceiling is refused"),
        "NotPlannable"
    );
}

/// 🔴 **The escrow ceiling has an instance here**, which is the thing `gx-adapter-git` recorded it
/// could not have.
///
/// req/99 §3 **D-4**: a git inverse carries an object id, so **M4-21**'s ceiling could not be reached by
/// any input and declaring it would have been "a refusal nobody asked for" (52 contract 2). An MCP server (sem: SEM-gx-adapter-mcp-337)
/// offers no content-addressed store, so the inverse carries the prior contents and the bound is real.
/// Either side of it, measured.
#[test]
fn the_escrow_ceiling_is_reachable_here_and_answers_ok_none() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = fixture_locator();
    let escrow_of = |size: usize| {
        fixture
            .server()
            .write_behind_the_adapter(SUBJECT, &vec![b'z'; size]);
        let pre = adapter.snapshot(&locator).expect("the server answers");
        let delta = adapter
            .plan(&intent_for(&locator, WRITE_TOOL, b"after\n"), &pre)
            .expect("a small forward call");
        adapter
            .invert(&delta, &pre)
            .expect("invert answers")
            .into_inverse()
            .map(|d| d.payload().len())
    };

    let small = escrow_of(1024);
    let large = escrow_of(MAX_INVERSE_PAYLOAD_BYTES + 1);
    println!("MCP_INVERT_CEILING={MAX_INVERSE_PAYLOAD_BYTES} UNDER={small:?} OVER={large:?}");
    let small = small.expect("a kilobyte fits in the escrow");
    assert!(
        small > 1024,
        "the inverse carries the prior contents, its uri and its own framing, so its payload is \
         larger than the contents it restores"
    );
    assert!(
        large.is_none(),
        "a resource over the ceiling still produced an inverse, so the constant is decorative — \
         which is exactly what req/99 §3 D-4 refused to ship for git"
    );
}

fn fixture_locator() -> String {
    normalize(&locator_on(SERVER, SUBJECT))
}

/// 🔴 **R-3 of req/99, answered.** The escrow ceiling is declared **per adapter that carries a body**,
/// and by no crate that is not an adapter.
///
/// req/99 §7 read `MAX_FORWARD_PAYLOAD_BYTES`'s "one constant, one place" as **per adapter** when the git adapter (sem: SEM-gx-adapter-mcp-338)
/// declared a second one, and left the inverse bound alone because git declares none — raising R-3:
/// "if hand 3 declares an escrow ceiling for mcp, the same re-reading is needed". It does, so this is that reading, and the (sem: SEM-gx-adapter-mcp-339)
/// gate is **not** weakened: a non-adapter crate declaring one is still red, an adapter declaring two is
/// still red, and "no adapter declares one at all" is red as well. (sem: SEM-gx-adapter-mcp-340)
///
/// The one thing this cannot assert is "every adapter whose inverse carries a body declares one" -- (sem: SEM-gx-adapter-mcp-341)
/// "carries a body" is not a fact in the source. What stands in for it is the pair of probes that name (sem: SEM-gx-adapter-mcp-342)
/// the choice on both sides: git's `this_adapter_declares_no_escrow_ceiling` and the test above.
#[test]
fn the_escrow_ceiling_is_declared_once_per_adapter_that_needs_one() {
    let root = repo_root();
    let mut per_crate: Vec<(String, Vec<String>)> = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("crates/ is readable") {
        let dir = entry.expect("an entry").path();
        let src = dir.join("src");
        if !src.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .expect("a named directory")
            .to_string_lossy()
            .into_owned();
        let mut found = Vec::new();
        for file in walk(&src) {
            let text = std::fs::read_to_string(&file).expect("a source file is readable");
            for line in text.lines() {
                if line
                    .trim_start()
                    .starts_with("pub const MAX_INVERSE_PAYLOAD_BYTES")
                {
                    found.push(format!("{}: {}", file.display(), line.trim()));
                }
            }
        }
        per_crate.push((name, found));
    }
    per_crate.sort();

    let adapters: Vec<&(String, Vec<String>)> = per_crate
        .iter()
        .filter(|(name, _)| name.starts_with("gx-adapter-"))
        .collect();
    let declaring: usize = adapters.iter().filter(|(_, f)| !f.is_empty()).count();
    println!(
        "MAX_INVERSE_PAYLOAD_BYTES_ADAPTERS={} DECLARING={declaring} PER_CRATE={:?}",
        adapters.len(),
        per_crate
            .iter()
            .map(|(name, found)| (name, found.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        adapters.len() >= 3,
        "the scan found {} adapter crates, which is not this workspace (§30's disease)",
        adapters.len()
    );
    assert!(
        declaring >= 2,
        "fewer than two adapters declare an escrow ceiling, so M4-21 is a constant with no reader"
    );
    for (name, found) in &per_crate {
        let limit = usize::from(name.starts_with("gx-adapter-"));
        assert!(
            found.len() <= limit,
            "M4-21 fixes the ceiling at \"one constant, one place\" per adapter and no crate below the boundary (sem: SEM-gx-adapter-mcp-343) \
             declares one; `{name}` has {found:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The inverse
// ---------------------------------------------------------------------------

/// A tool with no declared restore has no inverse, and the answer is `Ok(None)` rather than an error.
///
/// **E-M4-32**: `Ok(None)` is "a legitimate construction of the same object is impossible", and this is the ordinary case for an MCP (sem: SEM-gx-adapter-mcp-344)
/// proxy — most tools cannot be undone and nothing in the protocol says which can. **E-M3-4** escalates.
#[test]
fn a_tool_with_no_declared_restore_has_no_inverse() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = fixture_locator();
    let pre = adapter.snapshot(&locator).expect("the server answers");

    let undoable = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, b"after\n"), &pre)
        .expect("plan");
    let opaque = adapter
        .plan(&intent_for(&locator, NOTIFY_TOOL, b"{}"), &pre)
        .expect("plan");

    let with = adapter
        .invert(&undoable, &pre)
        .expect("invert answers")
        .into_inverse();
    let without = adapter
        .invert(&opaque, &pre)
        .expect("invert answers")
        .into_inverse();
    println!(
        "MCP_INVERT declared={} undoable={} opaque={}",
        adapter.catalogue().declared(),
        with.is_some(),
        without.is_some()
    );
    assert!(
        with.is_some(),
        "the catalogue declares a restore for this tool"
    );
    assert!(
        without.is_none(),
        "a tool with no declared restore produced an inverse, so the catalogue is not what decides"
    );
}

/// A `pre` naming another object is a **wiring mistake** and is refused as one (**E-M4-32**).
#[test]
fn an_inverse_against_another_object_is_an_error_and_not_ok_none() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = fixture_locator();
    let pre = adapter.snapshot(&locator).expect("the server answers");
    let delta = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, b"after\n"), &pre)
        .expect("plan");

    let elsewhere = absent_snapshot(&locator_on(SERVER, "file:///srv/other.md"));
    let refusal = adapter
        .invert(&delta, &elsewhere)
        .expect_err("a `pre` of another object is a mis-wired call");
    println!("MCP_INVERT_MISMATCH {}", refusal.kind());
    assert_eq!(
        refusal.kind(),
        "LocatorMismatch",
        "answering `Ok(None)` would send a wiring bug down E-M3-4's escalation path wearing the \
         face of a legitimate business condition"
    );
}

/// The restore convention is the one the crate documents, and it round-trips through gx-canon.
#[test]
fn the_inverse_carries_the_body_in_the_documented_shape() {
    let fixture = McpFixture::new();
    let adapter = fixture.mcp();
    let locator = fixture_locator();
    let pre = adapter.snapshot(&locator).expect("the server answers");
    let delta = adapter
        .plan(&intent_for(&locator, WRITE_TOOL, b"after\n"), &pre)
        .expect("plan");

    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("the catalogue declares a restore");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter's own grammar");
    let op = decoded.ops().first().expect("one operation");
    println!(
        "MCP_RESTORE tool={:?} argument_bytes={}",
        op.tool(),
        op.arguments().len()
    );
    assert_eq!(op.tool(), support::RESTORE_TOOL);
    assert_eq!(
        op.arguments(),
        restore_arguments(SUBJECT, support::INITIAL).expect("the convention encodes"),
        "the inverse's arguments are not the `{{contents, uri}}` the crate documents, so a restore \
         tool written against the documentation would be handed something else"
    );
}
