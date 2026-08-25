// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The `github/github-mcp-server` target's do/undo catalogue and its example policy pack (req/150
//! §A1-1, req/151's target pick, req/152's implementation report).
//!
//! # Why these four pairs, out of 20+ write tools
//!
//! req/151 §5 picked github-mcp-server for its write density and its absent restore surface (census
//! v2: write-shaped tools with a restore path are 13.8% of the population, `req/38` §78). req/152's own (sem: SEM-gx-adapter-mcp-353)
//! recon (gh api + `pkg/github/repositories.go` source read, `GitRepo/REFERENCES.md` 2026-08-14)
//! confirms both halves for v1.9.0's **default toolset** (`context`, `repos`, `issues`,
//! `pull_requests`, `users` -- the set a deployment runs without opting into anything extra): most of
//! its write tools have no compensating call at all, and the four that do are the four below. YAGNI
//! per the task brief: 3-5 pairs, not the full 20+.
//!
//! | forward tool | undone by | why the pair is well-formed |
//! |---|---|---|
//! | `create_or_update_file` | `create_or_update_file` (called again with the prior blob's content and `sha`) | the same tool both directions, the same relation `gx-adapter-fs`'s own invert has to a write |
//! | `delete_file` | `create_or_update_file` (recreate with the prior content and `sha`) | asymmetric: deleting and recreating are different API calls |
//! | `issue_write` (`method: "create"`) | `issue_write` (`method: "update", state: "closed"`) | GitHub's REST API has **no issue-deletion endpoint at all** -- closing is the nearest compensating call there is, which is the same "close/delete-shaped" shape the task brief named | (sem: SEM-gx-adapter-mcp-354)
//! | `create_pull_request` | `update_pull_request` (`state: "closed"`) | same shape as the issue pair, one tool over |
//!
//! # Outside the catalogue: pairs that do not exist, named rather than silently omitted (sem: SEM-gx-adapter-mcp-355)
//!
//! * `create_branch` -- v1.9.0 (`GitRepo/REFERENCES.md` 2026-08-14) ships **no `delete_branch` tool
//!   at all**: the `git` toolset holds only `get_repository_tree` (read-only), and `repos` has
//!   `create_branch` with no counterpart. The task brief's own example pair (`create_branch` ↔
//!   `delete_branch`) does not exist in the version this lane built and tested (`v1.9.0`,
//!   commit `cdfa34e`) -- named here rather than quietly substituted.
//! * `create_repository` -- no `delete_repository` tool in any toolset (grepped the whole README).
//! * `fork_repository` -- no un-fork tool.
//! * `merge_pull_request` -- a merge is not reachable in reverse through the REST API a tool could
//!   wrap; there is no "unmerge".
//! * `actions_run_trigger` (`method: "run_workflow"`) -- running a workflow has no compensating call;
//!   a cancel is not an undo of what already ran.
//! * `label_write`, `sub_issue_write` -- real create/update/delete triples that likely *could* pair
//!   (`label_write` create ↔ delete, in particular), left out of the four for the same YAGNI reason
//!   the brief asked for a small set: `label_write` is **outside the default toolset** (`labels` is
//!   not one of `context`/`repos`/`issues`/`pull_requests`/`users`), so it needs an operator's extra
//!   `--toolsets` flag this lane's four do not.
//!
//! # 🔴 What "being catalogued" does **not** get you for free: the automatic invert is fs-shaped (sem: SEM-gx-adapter-mcp-356)
//!
//! `crates/gx-adapter-mcp/src/delta.rs`'s restore convention builds an inverse's arguments as
//! canonical DAG-CBOR of exactly `{ contents: bytes, uri: text }` -- MCP's own `resources/read` shape
//! -- and `gx-mcp-wire`'s `WireTransport::arguments_of` (`crates/gx-mcp-wire/src/client.rs`) sends
//! **exactly that pair, and nothing else**, to whatever tool a catalogue names as the restore. That
//! is correct for a restore tool shaped like the fs/notes demo's (`notes.restore(uri, contents)`).
//! None of github-mcp-server's tools are shaped that way -- every one of them wants its own named,
//! structured parameters (`owner`, `repo`, `path`, `branch`, `message`, `sha`, `method`,
//! `issue_number`, ...) and none of them reads `uri` or `contents` at all. req/152's own E2E against
//! the real server (not this file -- `tools/a11_github_target_e2e.sh`) measured this directly:
//!
//! * `create_or_update_file` called with `{uri, contents}` (exactly what `WireTransport` would send
//!   for an undo of this pair) answers `isError: true`, `"missing required parameter: owner"`.
//! * `issue_write` called the same way answers `isError: true`, `"missing required parameter:
//!   method"` (a different first-missing-field, because the handler's own required-parameter order
//!   differs -- both are the same fact from two tools).
//!
//! So the four pairs below are declarable -- `Catalogue::declared()` counts them, and a deployment
//! that reads this file knows which calls a person is not asked about (**E-M3-4**, `invert_available`)
//! -- but `gx undo` against the real server fails, by design of this adapter's forward-only-shaped
//! escrow, for every one of them. That failure, its exact text, and the receipt/checkpoint machinery
//! around the one pair req/152 drove all the way through `gx wrap` (`create_or_update_file`) are the
//! subject of req/152's own report, not of this test file: what this file measures is the
//! **declaration** (`Catalogue`) and the **policy** (the fixture pack below), the two things a
//! deployment's operator actually writes by hand.

use gx_adapter_mcp::Catalogue;
use gx_core::SubstrateKind;
use gx_gate::packs::{self, PackCase, PackExpectation};
use gx_gate::{Gate, PolicyEngine};

/// The pack `tools/a11_github_target_e2e.sh` names with `gx wrap --policy` / `gx undo --policy`.
const GITHUB_TARGET_PACK_SOURCE: &str = include_str!("fixtures/github-target.cedar");

/// The four pairs this module's doc table names, built the way `crates/gx-cli/src/wrap.rs::catalogue`
/// builds one from repeated `--restore TOOL=RESTORE_TOOL` flags -- so a reader can see the CLI
/// invocation `tools/a11_github_target_e2e.sh` actually uses is this same map, spelled as flags.
fn github_target_catalogue() -> Catalogue {
    Catalogue::new()
        .with_restore("create_or_update_file", "create_or_update_file")
        .with_restore("delete_file", "create_or_update_file")
        .with_restore("issue_write", "issue_write")
        .with_restore("create_pull_request", "update_pull_request")
}

/// The catalogue declares exactly the four pairs the module doc names, and nothing named in the
/// "outside the catalogue" list above. (sem: SEM-gx-adapter-mcp-357)
#[test]
fn the_github_target_catalogue_declares_exactly_four_pairs() {
    let catalogue = github_target_catalogue();
    assert_eq!(
        catalogue.declared(),
        4,
        "four pairs (module doc table): create_or_update_file, delete_file, issue_write, \
         create_pull_request"
    );
    assert_eq!(
        catalogue.restore_for("create_or_update_file"),
        Some("create_or_update_file")
    );
    assert_eq!(
        catalogue.restore_for("delete_file"),
        Some("create_or_update_file")
    );
    assert_eq!(catalogue.restore_for("issue_write"), Some("issue_write"));
    assert_eq!(
        catalogue.restore_for("create_pull_request"),
        Some("update_pull_request")
    );
}

/// The tools outside the catalogue this module's doc names answer `None` -- an undeclared tool is (sem: SEM-gx-adapter-mcp-358)
/// `invert_available = false`, which **E-M3-4** turns into an escalation rather than a silent admit.
#[test]
fn the_named_catalogue_external_tools_declare_no_restore() {
    let catalogue = github_target_catalogue();
    for tool in [
        "create_branch",
        "create_repository",
        "fork_repository",
        "merge_pull_request",
        "actions_run_trigger",
        "label_write",
    ] {
        assert_eq!(
            catalogue.restore_for(tool),
            None,
            "{tool}: no restore tool is declared (this module's doc explains why each one is absent)"
        );
    }
}

/// The resource URI shape the four catalogued tools' locators use in this target's E2E: a repository
/// file path for the two file tools, and a synthetic (non-MCP-`resources/read`-backed) path for the
/// two tools that name no repository resource of their own (module doc's last paragraph).
fn github_locator(endpoint: &str, resource_path: &str) -> String {
    format!("{endpoint}#repo://mahirhir/glovrex-a11-throwaway/{resource_path}")
}

fn other_repo_locator(endpoint: &str) -> String {
    format!("{endpoint}#repo://mahirhir/some-other-repo/refs/heads/main/contents/x.md")
}

/// `PackCase`s for the fixture pack: one call inside the throwaway repository this lane's E2E used,
/// one outside it (the "repository outside the catalogue" case) and one on a different substrate entirely -- (sem: SEM-gx-adapter-mcp-359)
/// the same three-row minimum shape `crates/gx-gate/tests/ac_074.rs` uses for the shipped packs.
fn github_target_cases() -> Vec<PackCase> {
    vec![
        PackCase::new(
            "a call scoped to the designated throwaway repository is admitted",
            SubstrateKind::Mcp,
            github_locator(
                "stdio://github-mcp-server",
                "refs/heads/main/contents/README.md",
            ),
            PackExpectation::admit_by("github-target-permit-throwaway-repo"),
        )
        .because(
            "the one statement this pack carries: a repository-prefixed permit, which is the \
             nearest a resource-scoped policy gets to \"only a catalogue tool is permitted\" -- the module doc (sem: SEM-gx-adapter-mcp-360) \
             of `fixtures/github-target.cedar` explains why the literal reading is not \
             cedar-expressible (the tool's name is opaque to a policy, 42 §3.4)",
        ),
        PackCase::new(
            "a call on a repository this pack does not name is refused by no policy",
            SubstrateKind::Mcp,
            other_repo_locator("stdio://github-mcp-server"),
            PackExpectation::DenyByNoPolicy,
        )
        .because(
            "Cedar is default-deny and this pack carries exactly one narrow permit, so a resource \
             outside the throwaway repository's prefix reaches Cedar's third rule rather than being \
             admitted -- the deny half AC-074's own table asks every shipped-shaped pack to carry",
        ),
        PackCase::new(
            "a change on a substrate this pack does not speak for is refused by no policy",
            SubstrateKind::Fs,
            "/tmp/x".to_string(),
            PackExpectation::DenyByNoPolicy,
        )
        .because(
            "the one statement is scoped to the mcp substrate, the same isolation \
             `policies/mcp/deny-etc-resources.cedar`'s own table measures for the shipped pack",
        ),
    ]
}

/// The fixture pack parses, and every statement in it carries `@id` -- `PolicyEngine::parse` refuses
/// a set without one (**ASM-62-1**), so a pass here is the pack's own format requirement holding.
#[test]
fn the_github_target_pack_parses_and_carries_named_statements() {
    let engine = PolicyEngine::parse(GITHUB_TARGET_PACK_SOURCE)
        .expect("fixtures/github-target.cedar parses");
    assert_eq!(
        engine.policy_ids(),
        vec!["github-target-permit-throwaway-repo".to_string()],
        "one statement, one id -- the module doc's \"deny-default with nothing but a narrow permit\""
    );
}

/// Every row of the table holds, and AC-074's own arithmetic ("at least 1 Admit case, 1 Deny case") is (sem: SEM-gx-adapter-mcp-361)
/// satisfied by a target-scoped example pack the same way it is by a shipped one.
#[test]
fn the_github_target_pack_holds_at_least_one_admit_case_and_one_deny_case() {
    let gate = Gate::with_policies(
        PolicyEngine::parse(GITHUB_TARGET_PACK_SOURCE)
            .expect("fixtures/github-target.cedar parses"),
    );
    let report = packs::check_pack(&gate, &github_target_cases()).expect("every case is evaluable");
    println!("A11_GITHUB_TARGET_PACK {report}");
    assert_eq!(
        report.failures(),
        &[] as &[String],
        "every conformance case of the github-target example pack passes"
    );
    assert!(report.holds());
    assert!(report.admits() >= 1, "at least one Admit case");
    assert!(report.denies() >= 1, "at least one Deny case");
}
