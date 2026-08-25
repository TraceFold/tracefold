// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-074 (FR-028, git half) and **G-4** — the shipped git pack, and the one road a pack is checked by.
//!
//! AC-074 verbatim: "Given: the shipped git/MCP-substrate policy packs (`policies/{git,mcp}/`). When:
//! each pack's conformance test is run. Then: each pack has at least 1 Admit case and 1 Deny case and
//! both pass." Judgment method: "integration (per-pack x2)". **Both halves are here**: hand 2 wrote
//! the git one, hand 4 the mcp one, and "per-pack x2" is why they are two case tables checked separately rather than one table (sem: SEM-gx-gate-219)
//! with a substrate column.
//!
//! 🔴 What this file does **not** measure, now that there is more than one pack in the artifact:
//! what the packs do **together**. AC-074 is a statement about each pack alone (`check_pack` builds
//! its gate from one pack), and the composed set a deployment actually starts from is
//! `gx_gate::packs::shipped_pack_set` -- whose obligation is `crates/gx-gate/tests/shipped_set.rs`.
//! req/101 §9-1 named that gap before this hand opened it: "AC-074 does not measure the 'combined
//! set'. If they are combined, **the composed set's conformance** is required as a new obligation". (sem: SEM-gx-gate-220)
//!
//! # G-4, and why it decides the shape of this file
//!
//! `req/38_ERRATA_2026-08-07.md` §19 reserved G-4 as "a required ticket for M7's reqdef" and req/98
//! §3-4 states it: "when a third-party pack is dropped in, conformance checking runs down **the same
//! single road** (no branching between ours and a third party's)". (sem: SEM-gx-gate-221)
//! So the cases below are not run by a loop written here. They are run by
//! [`gx_gate::packs::check_pack`], which is library code a third party can call, and the third-party
//! pack in this file is run by **the same call**.
//!
//! A claim of that shape is empty without a way to be false, so three measurements hold it:
//!
//! | measurement | what its failure would mean |
//! |---|---|
//! | origin-blindness | the same bytes, loaded as a shipped pack and as a file a stranger wrote, produce the **same** report — a runner that treated the two differently would differ here |
//! | the runner names no pack | a source scan of the runner's own code lines for `FS_PACK`, `GIT_PACK`, `shipped`, `third_party`: a branch on origin has to be spelled somewhere |
//! | a third-party pack decides for itself | a stranger's pack with its own rules gets its own answers, so the first row is not passing because the runner ignores the pack |
//!
//! `tools/verify_m7h2.sh` §2 mutates the runner to take a second road for shipped packs and shows
//! this suite go red; req/60 §7.2's rule about absence checks ("checking for 'absence' needs a mutation") is (sem: SEM-gx-gate-222)
//! why that mutation exists rather than this comment standing alone.

use gx_core::SubstrateKind;
use gx_gate::packs::{
    self, PackCase, PackExpectation, GIT_PACK_POLICY_IDS, MCP_PACK_POLICY_IDS, SHIPPED_PACKS,
};
use gx_gate::{Gate, PolicyEngine};

/// A gate holding the shipped git pack and nothing else (**D-9**: no ready-made invariant ships).
fn shipped_git_gate() -> Gate {
    Gate::with_policies(packs::git_pack().expect("the shipped git pack parses"))
}

/// A gate holding the shipped mcp pack and nothing else.
fn shipped_mcp_gate() -> Gate {
    Gate::with_policies(packs::mcp_pack().expect("the shipped mcp pack parses"))
}

// ---------------------------------------------------------------------------
// The git pack's conformance table
// ---------------------------------------------------------------------------

/// A locator in the spelling `gx-adapter-git` writes: `<repository>#<reference>:<entry>`.
///
/// Written out rather than borrowed from that crate, because gx-gate does not depend on an adapter
/// and must not start doing so to test a policy. The spelling is pinned by
/// `crates/gx-adapter-git/src/locator.rs` (`Position::locator`), and a pack written against a
/// different one would be a pack whose rules never match a real change.
fn git_locator(reference: &str, entry: &str) -> String {
    format!("/srv/repo#{reference}:{entry}")
}

/// AC-074's cases for `policies/git/deny-nonbranch-refs.cedar`.
///
/// Every row says why it is in the table. A conformance table is what a deployment reads before it
/// trusts a pack, so a row that cannot say what it is for does not belong in one — the rule
/// `crates/gx-gate/tests/ac_028.rs` set for the fs pack, kept.
fn git_cases() -> Vec<PackCase> {
    vec![
        PackCase::new(
            "a change to an entry on a branch is admitted",
            SubstrateKind::Git,
            git_locator("refs/heads/main", "README.md"),
            PackExpectation::admit_by("git-permit-default"),
        )
        .because(
            "AC-074's Admit case. Cedar is default-deny, so the base permit is what makes a git \
             deployment able to admit anything at all",
        ),
        PackCase::new(
            "moving a tag is refused",
            SubstrateKind::Git,
            git_locator("refs/tags/v1.0.0", "VERSION"),
            PackExpectation::deny_by("git-deny-nonbranch-refs"),
        )
        .because(
            "AC-074's Deny case. A tag is a name a release, a signature or a deployment already \
             refers to, so moving it changes what an existing reference means after the fact",
        ),
        PackCase::new(
            "writing a remote-tracking reference is refused",
            SubstrateKind::Git,
            git_locator("refs/remotes/origin/main", "README.md"),
            PackExpectation::deny_by("git-deny-nonbranch-refs"),
        )
        .because(
            "the second disjunct. `refs/remotes/*` is this repository's record of what another \
             repository said, and writing it locally fabricates a claim about the remote",
        ),
        PackCase::new(
            "a repository whose own directory is called refs/tags is not a tag",
            SubstrateKind::Git,
            "/srv/refs/tags/mirror#refs/heads/main:README.md".to_string(),
            PackExpectation::admit_by("git-permit-default"),
        )
        .because(
            "the patterns anchor on `#`, the separator between the repository and the reference, so \
             a path that merely contains `refs/tags/` cannot make an ordinary branch change match. \
             A pattern written without the anchor turns this row red",
        ),
        PackCase::new(
            "a change on a substrate this pack does not speak for is refused by no policy",
            SubstrateKind::Fs,
            "/tmp/x".to_string(),
            PackExpectation::DenyByNoPolicy,
        )
        .because(
            "both statements are scoped to the git substrate, so an fs change reaches Cedar's third \
             rule rather than being quietly admitted by a git permit. The fs pack's own row 4 is \
             this one in the other direction",
        ),
        PackCase::new(
            "an order-2 change on a branch is admitted by this pack alone",
            SubstrateKind::Git,
            git_locator("refs/heads/main", "policy.cedar"),
            PackExpectation::admit_by("git-permit-default"),
        )
        .at_order(2)
        .in_context(gx_core::ChangeContext::Policy)
        .because(
            "**D-9**, made visible for the second pack: no ready-made invariant ships, so nothing \
             in the shipped artifact refuses a change to the policy layer",
        ),
    ]
}

// ---------------------------------------------------------------------------
// AC-074, git half
// ---------------------------------------------------------------------------

/// Every row of the git table, decided by the pack the build embeds, through the one road.
#[test]
fn ac_074_git_every_conformance_case_holds() {
    let report =
        packs::check_pack(&shipped_git_gate(), &git_cases()).expect("every case is evaluable");
    println!("AC074_GIT {report}");
    assert_eq!(
        report.failures(),
        &[] as &[String],
        "AC-074: every conformance case of the shipped git pack passes"
    );
    assert!(report.holds());
}

/// AC-074's own arithmetic: "at least 1 Admit case and 1 Deny case", counted from the table. (sem: SEM-gx-gate-223)
///
/// Asserted about the suite rather than satisfied by it accidentally: a table that lost its Admit
/// case would otherwise still pass every row it kept.
#[test]
fn ac_074_git_holds_at_least_one_admit_case_and_one_deny_case() {
    let report =
        packs::check_pack(&shipped_git_gate(), &git_cases()).expect("every case is evaluable");
    println!(
        "AC074_GIT_COUNTS admits={} denies={} rows={}",
        report.admits(),
        report.denies(),
        report.rows().len()
    );
    assert!(report.admits() >= 1, "AC-074: \"at least 1 Admit case\""); // (sem: SEM-gx-gate-224)
    assert!(report.denies() >= 1, "AC-074: \"1 Deny case\"");
}

/// The cases ran against the artifact that ships, not against a set assembled here.
#[test]
fn the_git_conformance_runs_against_the_pack_the_build_embeds() {
    let pack = packs::git_pack().expect("the shipped git pack parses");
    assert_eq!(
        pack.policy_ids(),
        GIT_PACK_POLICY_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect::<Vec<String>>(),
        "the ids the module declares are the ids the pack answers under (ASM-62-1)"
    );
    assert_eq!(
        pack.len(),
        GIT_PACK_POLICY_IDS.len(),
        "two statements, and no third one that no case covers"
    );
}

/// 🔴 Every rule in the shipped git pack can fire on a locator the git adapter can produce.
///
/// **D-4**'s shape, one layer along (req/99 §3): "do not declare an unreachable constant" (sem: SEM-gx-gate-225). `MAX_PATH_DEPTH` is 1
/// in `gx-adapter-git`, so a locator whose tree path has two components is refused by the adapter
/// before a gate ever sees it — a pack rule matching `*/.github/workflows/*` would look in force and
/// could never fire. Every locator in the table above is therefore a **single** entry under a
/// reference, and this test is what keeps that true when a row is added.
#[test]
fn every_case_names_a_locator_the_git_adapter_could_produce() {
    for case in git_cases() {
        if case.substrate() != &SubstrateKind::Git {
            continue;
        }
        let locator = case.locator();
        let (repository, rest) = locator.split_once('#').unwrap_or_else(|| {
            panic!("{locator}: a git locator names a repository and a reference")
        });
        let (reference, entry) = rest
            .split_once(':')
            .unwrap_or_else(|| panic!("{locator}: a git locator names an entry after a `:`"));
        assert!(
            repository.starts_with('/'),
            "{locator}: ASM-69-3 names positions absolutely"
        );
        assert!(
            reference.starts_with("refs/"),
            "{locator}: the adapter spells a reference in full"
        );
        assert_eq!(
            entry.split('/').count(),
            1,
            "{locator}: MAX_PATH_DEPTH is 1, so a rule about a nested path could never fire (D-4)"
        );
    }
}

// ---------------------------------------------------------------------------
// The mcp pack's conformance table (**M7 hand 4**)
// ---------------------------------------------------------------------------

/// A locator in the spelling `gx-adapter-mcp` writes: `<server endpoint>#<resource URI>`.
///
/// Written out rather than borrowed from that crate, for the reason `git_locator` gives one
/// substrate over: gx-gate does not depend on an adapter and must not start doing so to test a
/// policy. The spelling is pinned by `crates/gx-adapter-mcp/src/locator.rs` (`Position::locator`).
fn mcp_locator(server: &str, resource: &str) -> String {
    format!("{server}#{resource}")
}

/// AC-074's cases for `policies/mcp/deny-etc-resources.cedar`.
///
/// Every row says why it is in the table, as the git table's rows do.
fn mcp_cases() -> Vec<PackCase> {
    vec![
        PackCase::new(
            "a tool call on an ordinary resource is admitted",
            SubstrateKind::Mcp,
            mcp_locator("https://mcp.example/sse", "file:///srv/notes.md"),
            PackExpectation::admit_by("mcp-permit-default"),
        )
        .because(
            "AC-074's Admit case. Cedar is default-deny, so the base permit is what makes an MCP \
             deployment able to admit anything at all",
        ),
        PackCase::new(
            "a tool call that writes a file under /etc is refused",
            SubstrateKind::Mcp,
            mcp_locator("https://mcp.example/sse", "file:///etc/passwd"),
            PackExpectation::deny_by("mcp-deny-etc-resources"),
        )
        .because(
            "AC-074's Deny case, and the whole reason this pack exists: `/etc/passwd` is refused on \
             the fs substrate by `policies/fs/deny-etc.cedar`, and reaching the same file by asking \
             a server to write it is a different SubstrateKind, a different locator and therefore a \
             different policy subject",
        ),
        PackCase::new(
            "a tool call on the /etc node itself is refused",
            SubstrateKind::Mcp,
            mcp_locator("stdio://local-server", "file:///etc"),
            PackExpectation::deny_by("mcp-deny-etc-resources"),
        )
        .because(
            "the second disjunct, and M3 hand 5's argument (req/65 §3) read for this substrate: a \
             pack that forbade every file under /etc while permitting a change to /etc would \
             protect the files and leave a door beside them. The server is a `stdio://` one to say \
             in the table that the rule is about the resource and not about the endpoint",
        ),
        PackCase::new(
            "a resource whose own path merely contains etc is not under /etc",
            SubstrateKind::Mcp,
            mcp_locator("https://mcp.example/sse", "file:///srv/etc/passwd"),
            PackExpectation::admit_by("mcp-permit-default"),
        )
        .because(
            "the patterns are anchored at the start of the resource, so a project directory called \
             `etc` is not the system one. A rule written as `*etc*` turns this row red -- which is \
             the over-refusal half of the same care the row below measures for under-refusal",
        ),
        PackCase::new(
            "a server endpoint that spells file:///etc in its own query is not the resource",
            SubstrateKind::Mcp,
            mcp_locator(
                "https://proxy.example/redirect?to=file:///etc/passwd",
                "file:///srv/notes.md",
            ),
            PackExpectation::admit_by("mcp-permit-default"),
        )
        .because(
            "the patterns anchor on `#`, the separator between the endpoint and the resource. A \
             locator this adapter produces holds exactly one `#` (a resource carrying one is \
             refused at parse), so the anchor is decidable -- and a pattern written without it \
             drags every call on this endpoint into the forbid",
        ),
        PackCase::new(
            "a change on a substrate this pack does not speak for is refused by no policy",
            SubstrateKind::Fs,
            "/tmp/x".to_string(),
            PackExpectation::DenyByNoPolicy,
        )
        .because(
            "both statements are scoped to the mcp substrate, so an fs change reaches Cedar's third \
             rule rather than being quietly admitted by an mcp permit. The fs pack's row 4 and the \
             git pack's row 5 are this one in the other two directions -- and `shipped_set.rs` is \
             where the three of them stop being true together, on purpose",
        ),
        PackCase::new(
            "an order-2 tool call is admitted by this pack alone",
            SubstrateKind::Mcp,
            mcp_locator("https://mcp.example/sse", "file:///srv/policy.cedar"),
            PackExpectation::admit_by("mcp-permit-default"),
        )
        .at_order(2)
        .in_context(gx_core::ChangeContext::Policy)
        .because(
            "**D-9**, made visible for the third pack: no ready-made invariant ships, so nothing in \
             the shipped artifact refuses a change to the policy layer",
        ),
    ]
}

/// Every row of the mcp table, decided by the pack the build embeds, through the one road.
#[test]
fn ac_074_mcp_every_conformance_case_holds() {
    let report =
        packs::check_pack(&shipped_mcp_gate(), &mcp_cases()).expect("every case is evaluable");
    println!("AC074_MCP {report}");
    assert_eq!(
        report.failures(),
        &[] as &[String],
        "AC-074: every conformance case of the shipped mcp pack passes"
    );
    assert!(report.holds());
}

/// AC-074's own arithmetic for the second of its two packs: "at least 1 Admit case and 1 Deny case". (sem: SEM-gx-gate-226)
#[test]
fn ac_074_mcp_holds_at_least_one_admit_case_and_one_deny_case() {
    let report =
        packs::check_pack(&shipped_mcp_gate(), &mcp_cases()).expect("every case is evaluable");
    println!(
        "AC074_MCP_COUNTS admits={} denies={} rows={}",
        report.admits(),
        report.denies(),
        report.rows().len()
    );
    assert!(report.admits() >= 1, "AC-074: \"at least 1 Admit case\""); // (sem: SEM-gx-gate-227)
    assert!(report.denies() >= 1, "AC-074: \"1 Deny case\"");
}

/// The cases ran against the artifact that ships, not against a set assembled here.
#[test]
fn the_mcp_conformance_runs_against_the_pack_the_build_embeds() {
    let pack = packs::mcp_pack().expect("the shipped mcp pack parses");
    assert_eq!(
        pack.policy_ids(),
        MCP_PACK_POLICY_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect::<Vec<String>>(),
        "the ids the module declares are the ids the pack answers under (ASM-62-1)"
    );
    assert_eq!(
        pack.len(),
        MCP_PACK_POLICY_IDS.len(),
        "two statements, and no third one that no case covers"
    );
}

/// 🔴 Every rule in the shipped mcp pack can fire on a locator the mcp adapter can produce.
///
/// **D-4** for the third time (req/99 §3: "do not declare an unreachable constant") (sem: SEM-gx-gate-228), and on this substrate it bites
/// hardest: the rule a reader expects from an MCP pack is about the **tool**, and a tool's name
/// lives in the payload, which P-6 keeps opaque to a gate. So what the table may name is what the
/// locator carries, and this test is what keeps a row honest when one is added.
///
/// 🔴 **The bound of this instrument, said rather than implied**: it re-states the grammar and the
/// normal form structurally instead of calling `gx_adapter_mcp::locator::parse`, because gx-gate
/// does not depend on an adapter. So it can be wrong in the direction of being **too permissive** --
/// a locator it accepts that the adapter would refuse. The adapter's own side of the pair is
/// `crates/gx-adapter-mcp/tests/h2_normalised_before_the_gate.rs`, which drives real locators
/// through `snapshot` into this pack and is the half that cannot be too permissive.
#[test]
fn every_case_names_a_locator_the_mcp_adapter_could_produce() {
    for case in mcp_cases() {
        if case.substrate() != &SubstrateKind::Mcp {
            continue;
        }
        let locator = case.locator();
        assert_eq!(
            locator.matches('#').count(),
            1,
            "{locator}: an mcp locator holds exactly one `#` -- `parse` refuses a resource carrying \
             one of its own (RFC 3986 §3.5 spends it on the fragment)"
        );
        let (server, resource) = locator.split_once('#').expect("counted one");
        assert!(
            server.contains("://"),
            "{locator}: the server endpoint carries a scheme (ASM-69-3: a bare name is \"whichever \
             server that name pointed at when this ran\")" // (sem: SEM-gx-gate-229)
        );
        assert!(
            !resource.is_empty() && !server.is_empty(),
            "{locator}: neither part may be empty"
        );
        // The normal form, structurally: RFC 3986 §6.2.2's four clauses have nothing left to do.
        assert!(
            !locator.contains("/./") && !locator.contains("/../"),
            "{locator}: clause 4 (§6.2.2.3) folds dot segments, so a normalised locator holds none"
        );
        assert!(
            !locator.contains('%'),
            "{locator}: clause 3 (§6.2.2.2) upper-cases triplets and decodes unreserved ones. A \
             case that needs a percent triplet has to name the normal form of one, and the table \
             above needs none -- so the strong form is asserted here"
        );
        let authority_end = server.find("://").expect("checked") + 3;
        let authority = &server[authority_end..];
        let authority = authority.split(['/', '?']).next().unwrap_or(authority);
        assert_eq!(
            server[..authority_end].to_ascii_lowercase(),
            server[..authority_end],
            "{locator}: clause 1 (§3.1) folds the scheme"
        );
        assert_eq!(
            authority.to_ascii_lowercase(),
            authority,
            "{locator}: clause 2 (§6.2.2.1) folds the host"
        );
    }
}

// ---------------------------------------------------------------------------
// G-4 — one road, and no branch on where a pack came from
// ---------------------------------------------------------------------------

/// 🔴 **G-4**: the same bytes are checked identically whether they ship or arrive from a stranger.
///
/// The shipped pack is reached through `packs::git_pack()` (an `include_str!` inside gx-gate); the
/// third-party road is `PolicyEngine::parse` over text a caller supplies, which is what
/// `gx_cli::policy::load` does with a file an operator names. Both reports are compared field for
/// field, so a runner that gave a shipped pack any allowance a stranger's pack does not get would
/// have to make the two differ.
#[test]
fn g4_a_third_party_copy_of_the_shipped_pack_is_checked_identically() {
    let as_a_stranger_sent_it = Gate::with_policies(
        PolicyEngine::parse(packs::GIT_PACK_SOURCE)
            .expect("the same bytes parse by the other road"),
    );

    let cases = git_cases();
    let by_the_shipped_road = packs::check_pack(&shipped_git_gate(), &cases).expect("evaluable");
    let by_the_third_party_road =
        packs::check_pack(&as_a_stranger_sent_it, &cases).expect("evaluable");

    println!("G4_ORIGIN_BLIND shipped={by_the_shipped_road} third_party={by_the_third_party_road}");
    assert_eq!(
        by_the_shipped_road.rows(),
        by_the_third_party_road.rows(),
        "G-4: conformance does not branch on where a pack came from"
    );
}

/// A stranger's pack decides for itself, which is what makes the row above non-vacuous.
///
/// If `check_pack` ignored the pack it was handed, the test above would pass with two identical
/// wrong answers. This one hands it a pack with a rule the shipped one does not have — every git
/// change refused — and asks for the answers that pack implies.
#[test]
fn g4_a_third_party_pack_gets_its_own_answers() {
    let theirs = Gate::with_policies(
        PolicyEngine::parse(
            r#"
@id("their-deny-all-git")
forbid (principal, action, resource)
when { resource.substrate == "git" };
"#,
        )
        .expect("a third-party pack with one statement"),
    );

    let cases = vec![PackCase::new(
        "their pack refuses the change the shipped pack admits",
        SubstrateKind::Git,
        git_locator("refs/heads/main", "README.md"),
        PackExpectation::deny_by("their-deny-all-git"),
    )
    .because("the same case the shipped pack admits, decided by their rule instead")];
    let report = packs::check_pack(&theirs, &cases).expect("evaluable");
    println!("G4_THIRD_PARTY {report}");
    assert_eq!(
        report.failures(),
        &[] as &[String],
        "a third-party pack is judged by its own statements"
    );
}

/// 🔴 The runner names no pack, and no origin (**G-4**, as a source scan).
///
/// The instrument is the one `req/99` §2-2 used on the shared adapter harness
/// (`the_shared_harness_names_no_adapter`): the words are looked for on **code** lines, because the
/// module documentation names both packs on purpose. A branch on origin has to be spelled, and every
/// spelling of it contains one of these words.
#[test]
fn the_pack_conformance_runner_names_no_pack() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/packs.rs"),
    )
    .expect("the module this test is about");
    let runner = runner_source(&text);
    assert!(
        runner.lines().count() > 20,
        "the runner's source was not found; this scan would report 0 about nothing (§30)"
    );
    let mut named: Vec<String> = Vec::new();
    for line in runner.lines() {
        let code = line.trim_start();
        if code.starts_with("//") || code.starts_with("///") {
            continue;
        }
        for word in [
            "FS_PACK",
            "GIT_PACK",
            "fs_pack",
            "git_pack",
            "shipped",
            "SHIPPED",
            "third_party",
            "policies/",
        ] {
            if code.contains(word) {
                named.push(format!("{word}: {}", code.trim()));
            }
        }
    }
    println!(
        "G4_RUNNER_SCAN lines={} origin_words={}",
        runner.lines().count(),
        named.len()
    );
    assert!(
        named.is_empty(),
        "the conformance runner must not know which pack it is running: {named:?}"
    );
}

/// 🔴 **G-4, counted**: shipping code calls `Gate::verify` from exactly two places.
///
/// The origin-blindness probe above pins that the runner **cannot** tell a shipped pack from a
/// stranger's; this one pins that there is only one runner to tell it in. They are different claims
/// and the second is the one that fails first: a hand that wanted a shipped pack checked differently
/// would not add a branch to `check_pack`, it would write a second loop somewhere else, which is
/// exactly what `gx_cli::policy::test` was until M7 hand 2.
///
/// The two legitimate call sites and what each is:
///
/// | site | what it is |
/// |---|---|
/// | `crates/gx-engine/src/pipeline.rs` | the **real** decision — 43 T-4, the gate a transformation passes through |
/// | `crates/gx-gate/src/packs.rs` | the **hypothetical** decision — the one conformance road (G-4) |
///
/// A third is a second answer to "how is a pack checked" waiting to drift. `tools/verify_m7h2.sh` (sem: SEM-gx-gate-230)
/// §2 adds one back into gx-cli and shows this go red — the mutation req/60 §7.2 requires before an
/// absence check counts for anything.
#[test]
fn g4_shipping_code_holds_one_conformance_road() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-gate")
        .to_path_buf();

    let mut sites: Vec<String> = Vec::new();
    let mut files = 0usize;
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // `src/` only: a test may build a gate and ask it things, and counting those would
                // make this a census of the test suite rather than of the product.
                if path
                    .file_name()
                    .is_some_and(|n| n == "tests" || n == "benches" || n == "target")
                {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                files += 1;
                for line in text.lines() {
                    let code = line.trim_start();
                    if code.starts_with("//") {
                        continue;
                    }
                    if code.contains(".verify(GateInput") {
                        sites.push(
                            path.strip_prefix(&root)
                                .unwrap_or(&path)
                                .display()
                                .to_string()
                                .replace('\\', "/"),
                        );
                    }
                }
            }
        }
    }
    sites.sort();
    println!("G4_GATE_VERIFY_SITES files={files} sites={sites:?}");
    assert_eq!(
        sites,
        vec![
            "crates/gx-engine/src/pipeline.rs".to_string(),
            "crates/gx-gate/src/packs.rs".to_string(),
        ],
        "G-4: one real decision and one conformance road; a third call site is a second road"
    );
}

/// The runner's own source, delimited by the markers `packs.rs` carries around it.
///
/// A region rather than the whole file, because the module above it declares both packs by name and
/// a scan of the file would find those declarations and report on the wrong thing.
fn runner_source(text: &str) -> String {
    let start = text
        .find("// ---8<--- the one conformance road (G-4) ---8<---")
        .expect("the runner's opening marker");
    let end = text
        .find("// ---8<--- end of the one conformance road ---8<---")
        .expect("the runner's closing marker");
    text[start..end].to_string()
}

// ---------------------------------------------------------------------------
// The shipped set
// ---------------------------------------------------------------------------

/// All four packs are declared in one table, and both of AC-074's are in it.
///
/// The table is what `tests/pack_embedding.rs` walks to count roads, so a pack that shipped without
/// a row here would be invisible to the road count — which is the fail-open that test exists to
/// close, one level up.
#[test]
fn the_shipped_set_holds_every_pack() {
    let paths: Vec<&str> = SHIPPED_PACKS.iter().map(|p| p.path).collect();
    println!("SHIPPED_PACKS {paths:?}");
    assert_eq!(
        paths,
        vec![
            "policies/fs/deny-etc.cedar",
            "policies/git/deny-nonbranch-refs.cedar",
            "policies/mcp/deny-etc-resources.cedar",
            "policies/postgres/deny-system-catalogs.cedar"
        ],
        "FR-028 ships the fs pack (M3), the git pack (M7 hand 2) and the mcp pack (M7 hand 4); \
         req/446 V0-C adds the postgres pack, which is the first written against \
         `policies/PACK_FORMAT.md` and the first to declare a deny-default"
    );
    for pack in SHIPPED_PACKS {
        let engine = PolicyEngine::parse(pack.source).expect("a shipped pack parses");
        assert_eq!(
            engine.policy_ids(),
            pack.policy_ids
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<String>>(),
            "{}: the declared ids are the ids the pack answers under",
            pack.path
        );
    }
}
