// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The shipped catalogue tree (`catalogues/`), held to the rule `policies/` is already held to.
//!
//! # Why a tree at all
//!
//! A catalogue is the deployment's input, the way a policy pack is: it is what decides whether a
//! tool call can be undone, and an empty one escalates every change to a person. `policies/` ships
//! its packs beside the scenarios that prove them; catalogues shipped **nowhere** until this file,
//! so every deployment wrapping a real server had to write the declaration by hand from nothing.
//! That is the gap this pair of artefacts closes, and this file is the half that makes the other
//! half checkable.
//!
//! # What is asserted, and what is not
//!
//! Asserted here: the shipped bytes go through the format's one reader (`Catalogue::from_json`,
//! which runs `soundness()` on what it parses), they declare exactly the pairs the pack's README
//! names, every declaration carries both halves of a read-by-tool restore, the tools the README
//! refuses to declare answer `None`, and the pack pins the server it was written against.
//!
//! **Not** asserted here: that GitHub answers the way the declaration expects. The behavioural
//! evidence is `crates/gx-cli/tests/rmcp1_github_p1.rs`, which drives this same declaration through
//! `gx wrap` against a scripted face, and the last test below is what binds these bytes to that
//! declaration so the two cannot drift apart silently.

use std::path::{Path, PathBuf};

use gx_adapter_mcp::Catalogue;

/// The repository root, reached from this package's manifest directory.
///
/// Read at run time rather than through `include_str!`: this crate is a publish target, and a
/// macro road that reaches outside the package directory is the shape `req/1032` §3 measured as a
/// tarball with none of the files in it.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The one shipped pack, by its repository-relative path.
const GITHUB_PACK: &str = "catalogues/github-mcp-server/v1.9.0.json";

/// The declaration `crates/gx-cli/tests/rmcp1_github_p1.rs` drives end to end against a scripted
/// GitHub face. The shipped pack is the same declaration; the last test is what keeps that true.
const FIXTURE: &str = include_str!("fixtures/github16-p1-catalogue.json");

/// The seven pairs `catalogues/github-mcp-server/README.md` names, in its table's order.
///
/// Declared here rather than derived from the file, so a pair added to the JSON without a word in
/// the README is a failing test rather than a declaration nobody named -- the same shape
/// `gx-gate`'s `FS_PACK_POLICY_IDS` has against its pack.
const PAIRS: [(&str, &str); 7] = [
    ("update_issue_body", "update_issue_body"),
    ("update_issue_title", "update_issue_title"),
    ("update_issue_type", "update_issue_type"),
    ("update_issue_milestone", "update_issue_milestone"),
    ("update_pull_request_body", "update_pull_request_body"),
    ("update_pull_request_title", "update_pull_request_title"),
    ("update_pull_request_state", "update_pull_request_state"),
];

/// The write tools the pack's README refuses to declare, each with a reason in that table. The
/// negative control: a pack that quietly grew a pair for one of these would pass every assertion
/// above and be wrong.
const REFUSED: [&str; 7] = [
    "update_issue_labels",
    "update_issue_assignees",
    "update_issue_state",
    "update_pull_request",
    "update_pull_request_draft_state",
    "update_pull_request_branch",
    "update_gist",
];

/// Read a repository-relative file, refusing to report an empty read as a measurement.
fn pack_bytes(relative: &str) -> Vec<u8> {
    let path = repo_root().join(relative);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|why| panic!("UNTESTABLE {}: {why}", path.display()));
    assert!(
        !bytes.is_empty(),
        "UNTESTABLE {}: the file read as empty. This is not evidence that the pack is wrong; it \
         is evidence that nothing was measured.",
        path.display()
    );
    bytes
}

fn shipped_github() -> Catalogue {
    Catalogue::from_json(&pack_bytes(GITHUB_PACK))
        .unwrap_or_else(|why| panic!("{GITHUB_PACK} parses through the format's one reader: {why}"))
}

/// The bytes reach a running session by the road `gx wrap --restore-catalogue` takes, and
/// `from_json` runs `soundness()` on them, so a pass here is the file's own format requirement
/// holding rather than a claim about it.
#[test]
fn the_shipped_github_pack_parses_and_is_sound() {
    let catalogue = shipped_github();
    catalogue
        .soundness()
        .expect("every declaration in the shipped github pack is sound");
    assert_eq!(
        catalogue.declared(),
        PAIRS.len(),
        "the pack declares the pairs its README's table names, and no others"
    );
}

/// Every pair the README names is declared, and each carries **both** halves of a read-by-tool
/// restore.
///
/// The second half is the one that matters. A declaration naming a read face and no template falls
/// back to the `{contents, uri}` convention, which is right for a `resources/read`-shaped restore
/// tool and wrong for every tool on this server -- the measured failure that made the earlier
/// four-pair github catalogue undeclarable-in-practice (`crates/gx-adapter-mcp/tests/
/// github_target_catalogue.rs`, whose module doc carries the `isError: true` texts).
#[test]
fn the_shipped_github_pack_declares_the_pairs_its_readme_names_with_a_prior_and_a_template() {
    let catalogue = shipped_github();
    for (forward, restored_by) in PAIRS {
        assert_eq!(
            catalogue.restore_for(forward),
            Some(restored_by),
            "{forward} is declared as undone by {restored_by}"
        );
        let spec = catalogue
            .spec_for(forward)
            .unwrap_or_else(|| panic!("{forward} has a declaration"));
        assert!(
            spec.read_by().is_some(),
            "{forward}: the prior comes from a read tool -- v1.9.0 registers no resource for an \
             issue or a pull request, so `resources/read` cannot supply one"
        );
        assert!(
            spec.template().is_some(),
            "{forward}: the restore call's arguments are built by a template -- a read face with \
             no template restores the answer document where the field was"
        );
    }
}

/// The tools the README refuses to declare answer `None`, which is `invert_available = false`,
/// which escalates to a person rather than admitting a wrong undo.
#[test]
fn the_tools_the_pack_refuses_to_declare_answer_none() {
    let catalogue = shipped_github();
    for tool in REFUSED {
        assert_eq!(
            catalogue.restore_for(tool),
            None,
            "{tool}: no pair is declared, and the pack's README says why"
        );
    }
}

/// The pack pins the server its declarations were written against.
///
/// gx carries the pin and does not verify it against the live server, so the pin is a note to the
/// operator rather than a check -- which is exactly why a shipped pack must be unable to omit it.
#[test]
fn the_shipped_github_pack_pins_the_server_it_was_written_against() {
    let catalogue = shipped_github();
    let pin = catalogue
        .server()
        .expect("a shipped pack names the server it was written against")
        .to_string();
    for needle in [
        "github-mcp-server",
        "v1.9.0",
        "issues_granular",
        "pull_requests_granular",
    ] {
        assert!(
            pin.contains(needle),
            "the pin names {needle} -- without the two feature flags the seven tools do not exist \
             on the server at all: {pin}"
        );
    }
}

/// The reader this file runs refuses a broken declaration.
///
/// Without this arm every assertion above could be passing because the reader admits anything. The
/// positive control sits beside the negative one so that a failure here is the mutation's doing and
/// not the file's.
#[test]
fn the_reader_this_file_runs_refuses_a_declaration_with_a_blank_restore() {
    let text = String::from_utf8(pack_bytes(GITHUB_PACK)).expect("the pack is UTF-8");
    Catalogue::from_json(text.as_bytes()).expect("the control: the unmutated pack parses");

    let blanked = text.replacen(
        "\"restored_by\": \"update_issue_body\"",
        "\"restored_by\": \"\"",
        1,
    );
    assert_ne!(blanked, text, "the mutation reached the bytes");
    assert!(
        Catalogue::from_json(blanked.as_bytes()).is_err(),
        "a restore that names no tool is a parse error, so it never reaches a running session"
    );
}

/// The shipped pack and the declaration the end-to-end drives are one declaration.
///
/// Compared as parsed catalogues rather than as bytes: what must not drift is what the two files
/// *declare*, and a line-ending difference is not a divergence.
#[test]
fn the_shipped_pack_and_the_fixture_the_end_to_end_drives_are_one_declaration() {
    let fixture = Catalogue::from_json(FIXTURE.as_bytes()).expect("the fixture parses");
    assert_eq!(
        fixture.declared(),
        PAIRS.len(),
        "guard against a vacuous comparison: two empty catalogues are also equal"
    );
    assert_eq!(
        shipped_github(),
        fixture,
        "the shipped pack is the declaration `crates/gx-cli/tests/rmcp1_github_p1.rs` drives"
    );
}

/// Every pack in the tree is named by this file.
///
/// A pack that arrives without a check is the failure `policies/README.md` guards against with two
/// tests; this is the same guard, one directory over.
#[test]
fn every_pack_in_the_tree_is_named_by_this_file() {
    let entries = |dir: PathBuf| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap_or_else(|why| panic!("UNTESTABLE {}: {why}", dir.display()))
            .map(|entry| {
                entry
                    .expect("a readable directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    };

    assert_eq!(
        entries(repo_root().join("catalogues")),
        vec!["README.md".to_string(), "github-mcp-server".to_string()],
        "one pack and the tree's own README -- extend this file when a second pack arrives"
    );
    assert_eq!(
        entries(repo_root().join("catalogues/github-mcp-server")),
        vec!["README.md".to_string(), "v1.9.0.json".to_string()],
        "a pack is a README and one version-named declaration"
    );
}
