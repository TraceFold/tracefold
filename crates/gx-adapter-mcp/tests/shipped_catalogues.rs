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
//! **Not** asserted here: that either server answers the way its declaration expects. For github
//! the behavioural evidence is `crates/gx-cli/tests/rmcp1_github_p1.rs`, which drives that
//! declaration through `gx wrap` against a scripted face; for notion it is
//! `crates/gx-adapter-mcp/tests/notion_page_catalogue.rs`, which drives its one pair's escrow and
//! completion against a real captured server response but has not driven `gx wrap` end to end
//! (`catalogues/notion-mcp-server/README.md` names the structural reason). Each pack's own "the
//! shipped pack and the fixture ... are one declaration" test is what binds these bytes to that
//! evidence so the two cannot drift apart silently.

use std::path::{Path, PathBuf};

use gx_adapter_mcp::{ArgSource, Catalogue};

/// The repository root, reached from this package's manifest directory.
///
/// Read at run time rather than through `include_str!`: this crate is a publish target, and a
/// macro road that reaches outside the package directory is the shape `req/1032` §3 measured as a
/// tarball with none of the files in it.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The first shipped pack, by its repository-relative path.
const GITHUB_PACK: &str = "catalogues/github-mcp-server/v1.9.0.json";

/// The declaration `crates/gx-cli/tests/rmcp1_github_p1.rs` drives end to end against a scripted
/// GitHub face. The shipped pack is the same declaration; the last test is what keeps that true.
const FIXTURE: &str = include_str!("fixtures/github16-p1-catalogue.json");

/// The second shipped pack, by its repository-relative path.
const NOTION_PACK: &str = "catalogues/notion-mcp-server/2.5.1.json";

/// The declaration `notion_page_catalogue.rs` drives against the real captured server response
/// (`fixtures/notion-post-page-observation.json`). The shipped pack is the same declaration; the
/// last test in the notion section is what keeps that true.
const NOTION_FIXTURE: &str = include_str!("fixtures/notion-page-catalogue.json");

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

/// The one pair `catalogues/notion-mcp-server/README.md` names.
const NOTION_PAIRS: [(&str, &str); 1] = [("API-post-page", "API-delete-a-block")];

/// Write tools the notion pack's README names as refused, either measured (the boolean-argument
/// gap) or simply never driven against the real server (the README says which is which; this test
/// does not distinguish the two, only that neither is declared).
const NOTION_REFUSED: [&str; 5] = [
    "API-patch-page",
    "API-update-a-block",
    "API-patch-block-children",
    "API-move-page",
    "API-create-a-comment",
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

fn shipped_notion() -> Catalogue {
    Catalogue::from_json(&pack_bytes(NOTION_PACK))
        .unwrap_or_else(|why| panic!("{NOTION_PACK} parses through the format's one reader: {why}"))
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

// ---------------------------------------------------------------------------
// The second pack: notion-mcp-server
// ---------------------------------------------------------------------------

/// The bytes reach a running session by the same road as the github pack, and `from_json` runs
/// `soundness()` on them, so a pass here is the file's own format requirement holding.
#[test]
fn the_shipped_notion_pack_parses_and_is_sound() {
    let catalogue = shipped_notion();
    catalogue
        .soundness()
        .expect("every declaration in the shipped notion pack is sound");
    assert_eq!(
        catalogue.declared(),
        NOTION_PAIRS.len(),
        "the pack declares the one pair its README names, and no others"
    );
}

/// The one pair the README names is declared, its restore argument comes from the forward call's
/// own result (not a prior read -- the created page does not exist until the forward call
/// returns), and it carries no `read_by`: a read face beside a template that draws no prior is a
/// declaration-soundness fault (`RestoreSpec::soundness`, DR-46-19), not an oversight to add.
#[test]
fn the_shipped_notion_pack_declares_its_one_pair_with_a_do_result_template_and_no_read_face() {
    let catalogue = shipped_notion();
    for (forward, restored_by) in NOTION_PAIRS {
        assert_eq!(
            catalogue.restore_for(forward),
            Some(restored_by),
            "{forward} is declared as undone by {restored_by}"
        );
        let spec = catalogue
            .spec_for(forward)
            .unwrap_or_else(|| panic!("{forward} has a declaration"));
        assert!(
            spec.read_by().is_none(),
            "{forward}: the restore argument is the forward call's own result -- there is nothing \
             to read before the object exists"
        );
        let template = spec.template().unwrap_or_else(|| {
            panic!("{forward}: the restore call's argument is built by a template")
        });
        assert!(
            template.arguments().values().all(ArgSource::is_do_result),
            "{forward}: every template member resolves from the forward call's own result, not a \
             prior read or escrow-time material"
        );
    }
}

/// The tools the README refuses to declare answer `None`, which is `invert_available = false`,
/// which escalates to a person rather than admitting a wrong undo.
#[test]
fn the_tools_the_notion_pack_refuses_to_declare_answer_none() {
    let catalogue = shipped_notion();
    for tool in NOTION_REFUSED {
        assert_eq!(
            catalogue.restore_for(tool),
            None,
            "{tool}: no pair is declared, and the pack's README says why"
        );
    }
}

/// The pack pins the server its declaration was written against -- the commit rather than a tag,
/// because no tag names it (the README explains why).
#[test]
fn the_shipped_notion_pack_pins_the_server_it_was_written_against() {
    let catalogue = shipped_notion();
    let pin = catalogue
        .server()
        .expect("a shipped pack names the server it was written against")
        .to_string();
    for needle in ["notion-mcp-server", "2.5.1", "1d38420", "2025-09-03"] {
        assert!(pin.contains(needle), "the pin names {needle}: {pin}");
    }
}

/// The reader this file runs refuses a broken declaration -- the positive control beside the
/// negative one, so a failure here is the mutation's doing and not the file's.
#[test]
fn the_reader_this_file_runs_refuses_a_notion_declaration_with_a_blank_restore() {
    let text = String::from_utf8(pack_bytes(NOTION_PACK)).expect("the pack is UTF-8");
    Catalogue::from_json(text.as_bytes()).expect("the control: the unmutated pack parses");

    let blanked = text.replacen(
        "\"restored_by\": \"API-delete-a-block\"",
        "\"restored_by\": \"\"",
        1,
    );
    assert_ne!(blanked, text, "the mutation reached the bytes");
    assert!(
        Catalogue::from_json(blanked.as_bytes()).is_err(),
        "a restore that names no tool is a parse error, so it never reaches a running session"
    );
}

/// The shipped pack and the declaration `notion_page_catalogue.rs` drives are one declaration.
///
/// Compared as parsed catalogues rather than as bytes, and guarded against a vacuous comparison
/// the same way the github pair of this test is.
#[test]
fn the_shipped_notion_pack_and_the_fixture_the_conformance_tests_drive_are_one_declaration() {
    let fixture = Catalogue::from_json(NOTION_FIXTURE.as_bytes()).expect("the fixture parses");
    assert_eq!(
        fixture.declared(),
        NOTION_PAIRS.len(),
        "guard against a vacuous comparison: two empty catalogues are also equal"
    );
    assert_eq!(
        shipped_notion(),
        fixture,
        "the shipped pack is the declaration `notion_page_catalogue.rs` drives"
    );
}

/// Every pack in the tree is structurally sound -- discovered, not named.
///
/// # Why this replaced a name-listing test (`req/985` Task A, 2026-09-02)
///
/// The prior version of this test (`git log -p` on this file carries it) asserted the *exact* set
/// of entries under `catalogues/` -- `README.md`, `github-mcp-server`, `notion-mcp-server` -- with
/// `assert_eq!` against a hardcoded `Vec`. Its own doc comment said "extend this file when a third
/// pack arrives", which was an admission that shipping pack #3 required editing this Rust file and
/// running it through cargo before the edit could be trusted, even when pack #3 carried no new Rust
/// code at all (a catalogue is a JSON declaration plus a README; `Catalogue::from_json` is already
/// generic over its contents). `req/985` X.a1-a7/X.a9/X.l/X.m/X.n and `req/1109` §4 both name this
/// as a structural block on shipping catalogue-only packs in parallel with whatever lane holds the
/// cargo slot that day.
///
/// # What this version checks, and what it deliberately does not
///
/// It discovers pack directories from the filesystem and enforces the shape every pack must have
/// -- one `README.md`, one `*.json` declaration, and nothing else -- plus the two universal
/// structural facts `from_json`/`soundness`/`server()` already check per-pack: it parses through
/// the format's one reader, it is sound, and it pins the server it was written against. A pack
/// satisfying this shape ships without touching this file or running cargo for the *structural*
/// half of the gate.
///
/// It does **not** replace the behavioural tests above (`PAIRS`/`REFUSED`/pin/negative-control for
/// github, `NOTION_PAIRS`/`NOTION_REFUSED`/pin/negative-control for notion) -- those assert what a
/// *specific* server's declaration says and are the part of shipping a pack that legitimately still
/// needs new Rust written and run through cargo, same as it always did. This test only removes the
/// gate that used to fire before a new pack's own behavioural tests got a chance to run.
#[test]
fn every_pack_in_the_tree_is_structurally_sound() {
    let catalogues_dir = repo_root().join("catalogues");
    let top_entries: Vec<std::fs::DirEntry> = std::fs::read_dir(&catalogues_dir)
        .unwrap_or_else(|why| panic!("UNTESTABLE {}: {why}", catalogues_dir.display()))
        .map(|entry| entry.expect("a readable directory entry"))
        .collect();
    assert!(
        !top_entries.is_empty(),
        "UNTESTABLE {}: the directory read as empty. This is not evidence every pack is sound; it \
         is evidence nothing was measured.",
        catalogues_dir.display()
    );

    let has_own_readme = top_entries.iter().any(|entry| {
        entry.file_name() == "README.md"
            && entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
    });
    assert!(has_own_readme, "catalogues/ must carry its own README.md");

    let pack_dirs: Vec<PathBuf> = top_entries
        .iter()
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect();
    assert!(
        !pack_dirs.is_empty(),
        "catalogues/ has README.md but no pack directories -- nothing to declare"
    );

    for entry in &top_entries {
        let name = entry.file_name();
        let is_readme =
            name == "README.md" && entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);
        let is_pack_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        assert!(
            is_readme || is_pack_dir,
            "catalogues/{}: not the tree's own README.md and not a pack directory -- stray entry",
            name.to_string_lossy()
        );
    }

    for pack_dir in &pack_dirs {
        let pack_name = pack_dir.file_name().unwrap().to_string_lossy().into_owned();
        let pack_entries: Vec<std::fs::DirEntry> = std::fs::read_dir(pack_dir)
            .unwrap_or_else(|why| panic!("UNTESTABLE {}: {why}", pack_dir.display()))
            .map(|entry| entry.expect("a readable directory entry"))
            .collect();

        let readme_count = pack_entries
            .iter()
            .filter(|entry| entry.file_name() == "README.md")
            .count();
        assert_eq!(
            readme_count, 1,
            "catalogues/{pack_name}: a pack is a README and one version-named declaration -- found \
             {readme_count} README.md"
        );

        let json_entries: Vec<&std::fs::DirEntry> = pack_entries
            .iter()
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect();
        assert_eq!(
            json_entries.len(),
            1,
            "catalogues/{pack_name}: a pack is a README and one version-named declaration -- found \
             {} *.json files",
            json_entries.len()
        );

        assert_eq!(
            pack_entries.len(),
            2,
            "catalogues/{pack_name}: a pack directory holds exactly its README.md and its one \
             *.json declaration, nothing else -- found {} entries",
            pack_entries.len()
        );

        let json_path = json_entries[0].path();
        let relative = json_path
            .strip_prefix(repo_root())
            .expect("pack json is under the repo root")
            .to_str()
            .expect("pack path is valid UTF-8")
            .replace('\\', "/");
        let catalogue = Catalogue::from_json(&pack_bytes(&relative)).unwrap_or_else(|why| {
            panic!("catalogues/{pack_name}: parses through the format's one reader: {why}")
        });
        catalogue
            .soundness()
            .unwrap_or_else(|why| panic!("catalogues/{pack_name}: soundness: {why}"));
        assert!(
            catalogue.server().is_some(),
            "catalogues/{pack_name}: a shipped pack names the server it was written against"
        );
    }
}
