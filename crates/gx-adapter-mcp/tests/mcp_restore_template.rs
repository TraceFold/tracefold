// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The tool-aware restore template (A2, req/38 §92, ruling 1): what it resolves, what it refuses, and (sem: SEM-gx-adapter-mcp-298)
//! the two artefacts a github-mcp-server deployment hands `gx` (`fixtures/github-restore-catalogue.json`,
//! `fixtures/github-target-a2.cedar`).
//!
//! # What A2 is, in one paragraph
//!
//! A1-1 (req/152 §5) measured that the v0.1 restore convention -- every inverse carries canonical
//! DAG-CBOR `{contents, uri}` -- is refused by every structured-argument tool of the first real
//! target ("missing required parameter: owner"), and that req/136's mock undo "success" was the mock (sem: SEM-gx-adapter-mcp-299)
//! not validating arguments. The fix `catalogue.rs` now carries is a **declared template**: the
//! deployment says, per tool, how the restore call's own arguments are built from the forward call
//! and from the escrowed prior contents. This file measures the declaration machinery against the
//! in-process fixture server; the real-server measurement (undo green against github-mcp-server
//! v1.9.0) is `tools/a2_invert_e2e.sh`, reported in req/153.
//!
//! # The one derivation, and why it is honest
//!
//! github's `create_or_update_file` demands the **blob sha of the file being replaced** and checks it
//! against the live file. At undo time the live file holds what the forward call wrote -- and a git
//! blob id is content-addressed, so that value is `sha1("blob <len>\0" ++ forward.content)`,
//! computable **before** the forward call runs, which is exactly when **E-M4-30** builds the escrow.
//! The vectors below are `git hash-object`'s own (measured 2026-08-14 against git's CLI), so the
//! derivation is held to git's definition rather than to this crate's opinion of it.
//!
//! # Denominator (named rather than smoothed over) (sem: SEM-gx-adapter-mcp-300)
//!
//! * **issue/PR pairs are declarable, not template-able**: their compensating call needs the created
//!   issue/PR *number*, which exists only in the server's answer **after** `apply` -- material an
//!   escrow built before `apply` (43 T-10b) cannot carry, and no `ArgSource` pretends otherwise.
//! * **`delete_file`'s undo template resolves** (the omit-`sha` case below) **but `gx undo` cannot
//!   run it**: `Engine::undo` snapshots the locator at undo time (T-2) and a deleted file's
//!   `resources/read` is `Unreadable` -- gotcha93's family, raised for v0.2 in req/153 rather than
//!   silently declared working.

mod support;

use std::sync::Arc;

use gx_adapter_mcp::{
    git_blob_sha1_hex, ArgSource, Catalogue, McpAdapter, McpDelta, RestoreTemplate,
};
use gx_core::SubstrateKind;
use gx_gate::packs::{self, PackCase, PackExpectation};
use gx_gate::{Gate, PolicyEngine};
use gx_substrate::SubstrateAdapter;
use support::{locator_on, FakeServer, INITIAL, SERVER, SUBJECT};

/// The deployment file `tools/a2_invert_e2e.sh` hands to `gx wrap --restore-catalogue` and
/// `gx undo --mcp-restore-catalogue` -- one reader (`Catalogue::from_json`), one fixture, both
/// measured here.
const GITHUB_RESTORE_CATALOGUE: &str = include_str!("fixtures/github-restore-catalogue.json");

/// The A2 pack `tools/a2_invert_e2e.sh` names with `--policy`.
const GITHUB_TARGET_A2_PACK: &str = include_str!("fixtures/github-target-a2.cedar");

/// The github-shaped template, spelled with the builder API -- the same declaration the JSON
/// fixture carries, so the two spellings are held against each other below.
fn github_file_template() -> RestoreTemplate {
    RestoreTemplate::new()
        .with("owner", ArgSource::Forward("owner".into()))
        .with("repo", ArgSource::Forward("repo".into()))
        .with("path", ArgSource::Forward("path".into()))
        .with("branch", ArgSource::Forward("branch".into()))
        .with(
            "message",
            ArgSource::Const(
                "gx undo: restore the prior contents (A2 tool-aware invert, req/153)".into(),
            ),
        )
        .with("content", ArgSource::PriorContentsUtf8)
        .with("sha", ArgSource::GitBlobSha1OfForward("content".into()))
}

/// A forward `create_or_update_file` call's arguments, as `gx wrap` stores them: UTF-8 JSON.
fn github_forward_arguments() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "owner": "mahirhir",
        "repo": "glovrex-a2-throwaway",
        "path": "README.md",
        "branch": "main",
        "message": "A2 E2E: forward change",
        "content": "written by A2 (forward)\n",
        "sha": "0000000000000000000000000000000000000000",
    }))
    .expect("a JSON object")
}

// ---------------------------------------------------------------------------
// the derivation, held to git's own definition
// ---------------------------------------------------------------------------

/// `git_blob_sha1_hex` agrees with `git hash-object` on four vectors: the empty blob, the
/// canonical `"hello\n"`, Pro Git's own example -- measured against git's CLI on 2026-08-14
/// rather than transcribed from memory -- and a non-ASCII/CRLF vector (req/38 §94, ruling 2, F2 of (sem: SEM-gx-adapter-mcp-301)
/// req/154's audit: the ASCII-only distribution left the byte-length-vs-char-length distinction
/// unexercised by the shipped suite, though the audit had already measured the derivation correct).
/// The fourth expected value is the git-hash-object digest of `U+65E5 U+672C U+8A9E` followed by "CRLF\r\n", measured (sem: SEM-gx-adapter-mcp-302)
/// independently twice (req/154 §2 F2 on 2026-08-14, and re-measured for this vector on
/// 2026-08-15): the UTF-8 encoding is 15 bytes / 9 chars, so a char-length header would diverge.
#[test]
fn the_git_blob_derivation_matches_git_hash_object() {
    for (contents, expected) in [
        (&b""[..], "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"),
        (&b"hello\n"[..], "ce013625030ba8dba906f756967f9e9ca394464a"),
        (
            &b"what is up, doc?"[..],
            "bd9dbf5aae1a3862dd1526723246b20206e5fc37",
        ),
        (
            // "U+65E5 U+672C U+8A9E CRLF\r\n" as UTF-8 bytes: non-ASCII multibyte + CRLF in one vector. (sem: SEM-gx-adapter-mcp-303)
            "\u{65e5}\u{672c}\u{8a9e}CRLF\r\n".as_bytes(),
            "cbeccdb24b9f63444779e29f94932b7588bd3e7d",
        ),
    ] {
        let got = git_blob_sha1_hex(contents);
        println!("A2_BLOB_SHA1 len={} got={got}", contents.len());
        assert_eq!(got, expected, "git hash-object disagrees for {contents:?}");
    }
}

// ---------------------------------------------------------------------------
// resolution
// ---------------------------------------------------------------------------

/// The github template resolves a forward call + prior contents into exactly the restore call's
/// members: coordinates copied from the forward call, the body from the escrow, and `sha` derived
/// from the forward content (the value `git hash-object` prints for it, vector above).
#[test]
fn the_github_template_resolves_to_the_restore_calls_own_members() {
    let resolved = github_file_template()
        .resolve(&github_forward_arguments(), b"the prior contents\n")
        .expect("every member is resolvable");
    let value: serde_json::Value = serde_json::from_slice(&resolved).expect("UTF-8 JSON");
    println!("A2_RESOLVED {value}");
    assert_eq!(
        value,
        serde_json::json!({
            "owner": "mahirhir",
            "repo": "glovrex-a2-throwaway",
            "path": "README.md",
            "branch": "main",
            "message": "gx undo: restore the prior contents (A2 tool-aware invert, req/153)",
            "content": "the prior contents\n",
            "sha": "c27b87a000a1ee20e1aa0903d3864247727aae6c",
        }),
        "the restore call's arguments are the tool's own members, not `{{contents, uri}}`"
    );
    assert_eq!(
        value["sha"],
        serde_json::json!(git_blob_sha1_hex(b"written by A2 (forward)\n")),
        "`sha` is the blob id of what the forward call wrote -- the value the server will hold the \
         undo against"
    );
}

/// Resolution is deterministic: one declaration, one forward call, one escrow -- one byte string,
/// twice. (`RestoreTemplate` is a `BTreeMap` and `serde_json`'s map is ordered, so this is a
/// property of the types; the test keeps it a fact when either changes.)
#[test]
fn resolution_is_deterministic() {
    let template = github_file_template();
    let first = template
        .resolve(&github_forward_arguments(), b"prior")
        .expect("resolvable");
    let second = template
        .resolve(&github_forward_arguments(), b"prior")
        .expect("resolvable");
    assert_eq!(first, second);
}

/// A template with no `sha` member (the `delete_file` shape: recreating an absent file needs no
/// optimistic-concurrency token) resolves without one -- omission is declared by not declaring, not
/// by a null.
#[test]
fn a_template_without_a_member_resolves_without_it() {
    let resolved = RestoreTemplate::new()
        .with("path", ArgSource::Forward("path".into()))
        .with("content", ArgSource::PriorContentsUtf8)
        .resolve(&github_forward_arguments(), b"restored body")
        .expect("both members resolve");
    let value: serde_json::Value = serde_json::from_slice(&resolved).expect("UTF-8 JSON");
    assert_eq!(
        value,
        serde_json::json!({ "path": "README.md", "content": "restored body" })
    );
    assert!(
        value.get("sha").is_none(),
        "an undeclared member does not appear"
    );
}

/// A forward member the call does not carry makes the template unresolvable, and the refusal names
/// the member -- the sentence `invert` folds into `Ok(None)` (E-M3-4's escalation), so it had better
/// say which declaration failed.
#[test]
fn a_missing_forward_member_is_named_in_the_refusal() {
    let miss = RestoreTemplate::new()
        .with("issue_number", ArgSource::Forward("issue_number".into()))
        .resolve(&github_forward_arguments(), b"")
        .expect_err("the forward call carries no issue_number");
    println!("A2_MISS {miss}");
    assert!(
        miss.contains("issue_number"),
        "the refusal names the member: {miss}"
    );
}

/// Non-UTF-8 prior contents make `PriorContentsUtf8` unresolvable -- the same v0.2 `blob` residue
/// the wire's read path declares, refused rather than mojibake'd into a text member.
#[test]
fn non_utf8_prior_contents_are_refused_not_transcoded() {
    let miss = RestoreTemplate::new()
        .with("content", ArgSource::PriorContentsUtf8)
        .resolve(&github_forward_arguments(), &[0xff, 0xfe, 0x00])
        .expect_err("not UTF-8");
    assert!(miss.contains("UTF-8"), "the refusal says why: {miss}");
}

// ---------------------------------------------------------------------------
// the catalogue file: one format, one reader
// ---------------------------------------------------------------------------

/// The shipped fixture parses, declares exactly the one end-to-end-proven pair, and its template is
/// **equal** to the builder spelling above -- so the JSON file and the Rust declaration cannot
/// drift apart silently.
#[test]
fn the_github_catalogue_file_declares_the_one_proven_pair() {
    let catalogue = Catalogue::from_json(GITHUB_RESTORE_CATALOGUE.as_bytes())
        .expect("fixtures/github-restore-catalogue.json parses");
    println!("A2_CATALOGUE declared={}", catalogue.declared());
    assert_eq!(
        catalogue.declared(),
        1,
        "one pair: create_or_update_file, the one the real-server E2E proves end to end. \
         delete_file and the issue/PR pairs are the module doc's denominator, absent on purpose (sem: SEM-gx-adapter-mcp-304)"
    );
    assert_eq!(
        catalogue.restore_for("create_or_update_file"),
        Some("create_or_update_file")
    );
    let spec = catalogue
        .spec_for("create_or_update_file")
        .expect("declared");
    assert_eq!(
        spec.template(),
        Some(&github_file_template()),
        "the JSON spelling and the builder spelling are one declaration"
    );
}

/// Bytes that are not the catalogue format are refused with a sentence, not half-read: a file of
/// the wrong shape configuring "what undoes what" silently would be a policy-grade bug. (sem: SEM-gx-adapter-mcp-305)
#[test]
fn a_malformed_catalogue_file_is_refused() {
    for bytes in [
        &b"not json at all"[..],
        br#"["a", "list"]"#,
        br#"{"tool": {"arguments": {}}}"#, // no restored_by
        br#"{"tool": {"restored_by": "x", "arguments": {"a": {"no_such_source": "y"}}}}"#,
    ] {
        let refusal = Catalogue::from_json(bytes).expect_err("not the format");
        println!("A2_REFUSED {refusal}");
    }
}

// ---------------------------------------------------------------------------
// invert, catalogue-driven
// ---------------------------------------------------------------------------

/// Through the adapter's own `invert`: a templated declaration produces an inverse whose op calls
/// the restore tool with **resolved JSON arguments** -- prior contents in `content`, forward-derived
/// blob sha in `sha` -- while a template-less declaration on the same adapter still produces the
/// v0.1 `{contents, uri}` DAG-CBOR (no-delete: the old form is the default, unchanged, and the
/// mock/notes deployments keep reading exactly what they read).
#[test]
fn invert_resolves_a_template_and_leaves_the_legacy_form_alone() {
    let server = Arc::new(FakeServer::new());
    let adapter = McpAdapter::new(server.clone()).with_catalogue(
        Catalogue::new()
            .with_restore_template("notes.write", "notes.put_back", github_file_template())
            .with_restore("notes.append", "notes.restore"),
    );
    let locator = locator_on(SERVER, SUBJECT);
    let pre = adapter.snapshot(&locator).expect("the fixture answers");

    // The templated pair. The fixture's subject holds INITIAL, which is what the escrow carries.
    let forward = support::planned(
        &adapter,
        &locator,
        "notes.write",
        &github_forward_arguments(),
    );
    let inverse = adapter
        .invert(&forward, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("a declared, resolvable template");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter's grammar");
    let op = &decoded.ops()[0];
    assert_eq!(op.tool(), "notes.put_back");
    let arguments: serde_json::Value =
        serde_json::from_slice(op.arguments()).expect("resolved arguments are UTF-8 JSON");
    println!("A2_INVERSE_ARGS {arguments}");
    assert_eq!(
        arguments["content"],
        serde_json::json!(core::str::from_utf8(INITIAL).expect("the fixture's body is text")),
        "the escrowed body is the resource's prior contents, read before apply (43 T-10b)"
    );
    assert_eq!(
        arguments["sha"],
        serde_json::json!(git_blob_sha1_hex(b"written by A2 (forward)\n"))
    );
    assert_eq!(arguments["owner"], serde_json::json!("mahirhir"));

    // The legacy pair, byte-for-byte the shape `delta.rs`'s restore convention documents.
    let forward = support::planned(&adapter, &locator, "notes.append", b"{\"uri\":\"x\"}");
    let inverse = adapter
        .invert(&forward, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("a declared restore");
    let decoded = McpDelta::decode(inverse.payload()).expect("this adapter's grammar");
    let op = &decoded.ops()[0];
    assert_eq!(op.tool(), "notes.restore");
    assert_eq!(
        op.arguments(),
        support::restore_payload(SUBJECT, INITIAL).as_slice(),
        "a declaration without a template still writes canonical DAG-CBOR {{contents, uri}}"
    );
}

/// A template this call does not carry the material for -- `invert`'s third `Ok(None)`: the
/// conservative direction, E-M3-4 asks a person, rather than an escrow that would fail at the
/// server (req/152 §5 measured that failure for the shape-blind form).
///
/// 🔴 **`req/291` H-01 (R20) moved this fixture, and the note is here so the move is not silent.**
/// It used to be `{"issue_number": {"forward": "issue_number"}}` — a template built out of the
/// forward call alone, which is now a *declaration* fault refused before resolution ever runs, so
/// the arm would have been green for a reason that is not this arm's. The prior member restores
/// the separation: the declaration is **sound** (it draws the prior), and `issue_number` is still
/// absent from this call's arguments, so what is measured is exactly what the name says — a sound
/// template whose material this call does not carry answers `Ok(None)` rather than escrowing a
/// call the server would refuse.
#[test]
fn invert_answers_none_when_the_template_cannot_be_resolved() {
    let server = Arc::new(FakeServer::new());
    let adapter = McpAdapter::new(server).with_catalogue(
        Catalogue::new().with_restore_template(
            "notes.write",
            "notes.put_back",
            RestoreTemplate::new()
                .with("content", ArgSource::PriorContentsUtf8)
                .with("issue_number", ArgSource::Forward("issue_number".into())),
        ),
    );
    let locator = locator_on(SERVER, SUBJECT);
    let pre = adapter.snapshot(&locator).expect("the fixture answers");
    let forward = support::planned(
        &adapter,
        &locator,
        "notes.write",
        &github_forward_arguments(),
    );
    let inverse = adapter
        .invert(&forward, &pre)
        .expect("invert answers")
        .into_inverse();
    assert!(
        inverse.is_none(),
        "\"gx cannot construct the undo of this call\" escalates instead of escrowing a call the (sem: SEM-gx-adapter-mcp-306) \
         server would refuse"
    );
}

// ---------------------------------------------------------------------------
// the A2 pack, ac_074-shaped
// ---------------------------------------------------------------------------

/// The A2 pack parses and carries one named statement (**ASM-62-1**: `@id` required), scoped to the
/// A2 throwaway repository and to it alone.
#[test]
fn the_a2_pack_parses_and_carries_one_named_statement() {
    let engine =
        PolicyEngine::parse(GITHUB_TARGET_A2_PACK).expect("fixtures/github-target-a2.cedar parses");
    assert_eq!(
        engine.policy_ids(),
        vec!["github-target-a2-permit-throwaway-repo".to_string()]
    );
}

/// AC-074's arithmetic for the A2 pack: at least one Admit (inside the A2 throwaway repository),
/// and Deny for the repository the **A1-1** lane used -- retired, archived, and deliberately not
/// permitted again -- and for another substrate.
#[test]
fn the_a2_pack_admits_its_own_repository_and_nothing_else() {
    let gate =
        Gate::with_policies(PolicyEngine::parse(GITHUB_TARGET_A2_PACK).expect("the pack parses"));
    let cases = vec![
        PackCase::new(
            "a call scoped to the A2 throwaway repository is admitted",
            SubstrateKind::Mcp,
            "stdio://github-mcp-server#repo://mahirhir/glovrex-a2-throwaway/refs/heads/main/contents/README.md".to_string(),
            PackExpectation::admit_by("github-target-a2-permit-throwaway-repo"),
        ),
        PackCase::new(
            "the A1-1 lane's retired repository is not permitted by the A2 pack",
            SubstrateKind::Mcp,
            "stdio://github-mcp-server#repo://mahirhir/glovrex-a11-throwaway/refs/heads/main/contents/README.md".to_string(),
            PackExpectation::DenyByNoPolicy,
        ),
        PackCase::new(
            "a change on a substrate this pack does not speak for is refused by no policy",
            SubstrateKind::Fs,
            "/tmp/x".to_string(),
            PackExpectation::DenyByNoPolicy,
        ),
    ];
    let report = packs::check_pack(&gate, &cases).expect("every case is evaluable");
    println!("A2_GITHUB_TARGET_PACK {report}");
    assert_eq!(report.failures(), &[] as &[String]);
    assert!(report.holds());
    assert!(report.admits() >= 1 && report.denies() >= 2);
}
