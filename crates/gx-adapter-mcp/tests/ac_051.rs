// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-051**: there is no road to a tool call that does not pass a gate, and the road count is
//! **derived**.
//!
//! 34 AC-051, verbatim: "Given: the tool-call boundary under the `gx-adapter-mcp` proxy. When: inspect at the **configuration level** whether a direct call path bypassing (sem: SEM-gx-adapter-mcp-380)
//! the proxy exists, and carry out the call through the proxy. Then: every tool call always passes through the submit->verify (sem: SEM-gx-adapter-mcp-381)
//! route, and no bypass route technically exists (the direct call path is blocked), by an **integration test** (sem: SEM-gx-adapter-mcp-382)
//! is confirmed." (sem: SEM-gx-adapter-mcp-383)
//!
//! Ruling #10 (`req/38` §56) fixes the **form**: "AC-051 inspection form = **upgraded to a derivation** (a declared list is forbidden)", with (sem: SEM-gx-adapter-mcp-384)
//! M6H8-9's lesson behind it -- a hand-written route table missed five routes, and the fix was to derive
//! the table from the router. Nothing below is a list of places somebody remembered.
//!
//! # The five derivations, and what each one's population is
//!
//! | # | what is derived | over what population | by what |
//! |---|---|---|---|
//! | **D-1** | `ToolCall` and `Admitted` cannot be built | **every crate that is not `gx-adapter-mcp`**, including ones that do not exist yet | the compiler (two `compile_fail` doctests in `src/transport.rs`, each with a control that compiles) |
//! | **D-2** | inside the crate, both are minted in **one** place | every `.rs` file under this crate's `src/`, **enumerated by walking it** | a text scan, whose limit is printed beside it (M6H8-1's form) |
//! | **D-3** | of the seven methods, **exactly one** reaches the transport | 41 §4's seven, **read out of the trait's source** rather than typed here | a counting transport |
//! | **D-4** | above the adapter there is one road | every `.rs` file under `crates/*/src` | a scan for `SubstrateAdapter::apply` call sites (Rule 2, req/78 §3.3) | (sem: SEM-gx-adapter-mcp-385)
//! | **D-5** | that road passes the gate | one denying engine, one admitting engine | **driving it**: 0 calls and 1 call |
//!
//! D-1 is the load-bearing one and D-5 is the one AC-051 asks for in as many words. D-2 and D-4 are
//! text gates and say so; D-3 is behavioural and closes the gap D-2 leaves inside the crate (a mint in
//! a file the scanner read but whose call it did not recognise would still have to *reach* the
//! transport to matter, and D-3 counts arrivals).
//!
//! # 🔴 What is **not** derived, and is not hidden
//!
//! * **A different process.** Nothing here stops one from speaking to the same MCP server directly.
//!   AC-051's subject is "the tool-call boundary under the proxy" and making the proxy the only reachable endpoint (sem: SEM-gx-adapter-mcp-386)
//!   is a deployment's job — the same **N-05** disclosure the fs and git adapters carry, and 45 §2.2's
//!   "completeness only via the adapter". (sem: SEM-gx-adapter-mcp-387)
//! * **The transport's own conduct.** While a transport holds the `&ToolCall` and `&Admitted` it was
//!   handed it can pass them on; it cannot build a different call, so the worst it can do is replay
//!   one, and 51 §7 contract 7 makes a replay a no-op.
//!
//! Both are raised in `req/101` rather than left for a reader to notice.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gx_core::{Timestamp, VerdictKind};
use gx_engine::Lifecycle;
use gx_engine::{Engine, InjectedEvidence};
use gx_substrate_conformance::Fixture;
use support::{
    intent_for, subject_locator, FakeServer, McpFixture, RewindableLog, GOAL, SUBJECT, WRITE_TOOL,
};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-adapter-mcp")
        .to_path_buf()
}

/// Every `.rs` file under a directory, **found** rather than listed.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a directory is readable") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Source lines that are not comments. The same filter M6's Rule 1 counters use. (sem: SEM-gx-adapter-mcp-388)
fn code_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            !(t.is_empty() || t.starts_with("//"))
        })
        .map(|(i, l)| (i + 1, l.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// D-1 — the compiler's half
// ---------------------------------------------------------------------------

/// The two `compile_fail` doctests exist, each beside a control that compiles.
///
/// **M4-20, adopted (b)**'s pair form, and the reason it is a pair: a `compile_fail` block on its own proves (sem: SEM-gx-adapter-mcp-389)
/// only that *something* is wrong with it — a typo would satisfy it just as well as a private field.
/// The control is the same crate, the same trait and the same types, and it compiles.
///
/// The blocks themselves are **run by `cargo test`** as doc-tests; what this probe adds is that they
/// have not been deleted or quietly turned into ```ignore.
#[test]
fn the_two_values_a_deployment_cannot_build_are_measured_by_the_compiler() {
    let transport =
        std::fs::read_to_string(repo_root().join("crates/gx-adapter-mcp/src/transport.rs"))
            .expect("the boundary module is readable");

    let refusals = transport.matches("```compile_fail").count();
    let ignored = transport.matches("```ignore").count();
    let controls = transport.matches("impl ToolTransport for Wire").count();
    println!(
        "AC051_D1 compile_fail={refusals} controls={controls} ignore={ignored} \
         private_fields=[Admitted.delta, ToolCall.{{server,resource,tool,arguments,delta}}]"
    );
    assert_eq!(
        refusals, 2,
        "one refusal for `Admitted` and one for `ToolCall`: both are arguments of \
         `ToolTransport::call`, and a deployment that could build either would have a road that \
         never passed an `apply`"
    );
    assert!(
        controls >= 1,
        "a `compile_fail` with no control proves only that something is wrong with it"
    );
    assert_eq!(
        ignored, 0,
        "an ```ignore block is a doctest that is not compiled: the refusal would still be in the \
         documentation and would no longer be measured"
    );

    // The fields are private, which is the fact the doctests are about. Derived from the struct
    // definitions rather than asserted about the file as a whole.
    for (kind, marker) in [
        ("Admitted", "pub struct Admitted {"),
        ("ToolCall", "pub struct ToolCall {"),
    ] {
        let start = transport.find(marker).expect("the struct is declared") + marker.len();
        let body =
            &transport[start..start + transport[start..].find('}').expect("a closing brace")];
        assert!(
            !body.contains("pub "),
            "`{kind}` has a public field, so any crate can build one and D-1 is vacuous"
        );
    }
}

// ---------------------------------------------------------------------------
// D-2 — inside the crate, one mint
// ---------------------------------------------------------------------------

/// The mints are derived from this crate's own `src/`, by walking it.
///
/// 🔴 **The limit of this measurement is printed with it** (M6H8-1's form): it is a text gate, so a mint
/// reached through an alias, a macro or a re-export is outside what it can see. What closes that gap is
/// not a stronger scan but **D-3**, which counts arrivals at the transport instead of spellings in the
/// source: a mint the scanner missed still has to reach a `call` to matter.
#[test]
fn inside_this_crate_the_two_values_are_minted_in_one_place() {
    let src = repo_root().join("crates/gx-adapter-mcp/src");
    let files = walk(&src);
    assert!(
        files.len() >= 5,
        "the scan found {} files under src/, which is fewer than this crate has: it is looking at \
         the wrong directory and would report 0 mints about a crate full of them (§30's disease)",
        files.len()
    );

    let mut mints: Vec<String> = Vec::new();
    let mut calls: Vec<String> = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("a source file is readable");
        let name = file
            .strip_prefix(&src)
            .expect("under src/")
            .display()
            .to_string();
        for (line, text) in code_lines(&source) {
            if text.contains("Admitted::for_delta") || text.contains("ToolCall::new") {
                mints.push(format!("{name}:{line}"));
            }
            if text.contains(".call(&call") || text.contains("transport.call(") {
                calls.push(format!("{name}:{line}"));
            }
        }
    }
    println!(
        "AC051_D2 files={} mints={mints:?} transport_calls={calls:?} \
         LIMIT=text-gate(alias/macro/re-export invisible; D-3 counts arrivals instead)",
        files.len()
    );

    let sites: Vec<&str> = mints.iter().map(|m| m.split(':').next().unwrap()).collect();
    assert_eq!(
        mints.len(),
        2,
        "one mint of each value is what makes the road count one; these are what exist: {mints:?}"
    );
    assert!(
        sites.iter().all(|f| *f == "apply.rs"),
        "the mints are not all in `apply.rs`, which is the module 41 §4 says is only reached after a \
         gate admitted the delta: {mints:?}"
    );
    assert_eq!(
        calls.len(),
        1,
        "`ToolTransport::call` is reached from more than one place: {calls:?}"
    );
    assert!(
        calls[0].starts_with("apply.rs"),
        "the one call is not in `apply.rs`: {calls:?}"
    );
}

// ---------------------------------------------------------------------------
// D-3 — of the seven, one
// ---------------------------------------------------------------------------

/// **The seven methods are read out of the trait**, and exactly one of them reaches the transport.
///
/// 🔴 This is where ruling #10's "a declared list is forbidden" bites hardest. A probe that typed the six read-only method (sem: SEM-gx-adapter-mcp-390)
/// names into a `vec![]` would be asserting about the six somebody remembered; if 41 §4 grew an eighth,
/// the list would go on passing while the new method did whatever it liked. So the names come from
/// `crates/gx-substrate/src/adapter.rs` — the trait's own source — and the probe asserts it found seven,
/// which is **N-08**'s count and the number 51 §7's completion condition is stated in.
#[test]
fn exactly_one_of_the_sevens_methods_reaches_a_tool_call() {
    let trait_source =
        std::fs::read_to_string(repo_root().join("crates/gx-substrate/src/adapter.rs"))
            .expect("the trait is readable");
    let body_start = trait_source
        .find("pub trait SubstrateAdapter: Send + Sync {")
        .expect("the trait is declared");
    let derived: Vec<String> = code_lines(&trait_source[body_start..])
        .iter()
        .filter_map(|(_, l)| {
            let t = l.trim_start();
            t.strip_prefix("fn ")
                .and_then(|rest| rest.split(['(', '<']).next())
                .map(str::to_string)
        })
        .collect();
    println!("AC051_D3_METHODS derived={derived:?}");
    assert_eq!(
        derived.len(),
        7,
        "41 §4 has seven methods and N-08 forbids an eighth; the scan derived {derived:?}"
    );

    let fixture = McpFixture::new();
    let adapter = fixture.adapter();
    let locator = fixture.locator();

    // Every method except `apply`, in an order that gives each one its argument.
    let mut exercised: Vec<&str> = Vec::new();
    let _ = adapter.kind();
    exercised.push("kind");
    let pre = adapter.snapshot(&locator).expect("snapshot");
    exercised.push("snapshot");
    let delta = adapter.plan(&fixture_intent(&locator), &pre).expect("plan");
    exercised.push("plan");
    let _ = adapter.precondition(&pre).expect("precondition");
    exercised.push("precondition");
    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse();
    exercised.push("invert");
    let _ = adapter.commutation(&delta, &delta).expect("commutation");
    exercised.push("commutation");

    let before = fixture.server().calls();
    println!(
        "AC051_D3_BEFORE_APPLY exercised={exercised:?} calls={before} reads={} \
         inverse_available={}",
        fixture.server().reads(),
        inverse.is_some()
    );
    assert_eq!(
        before, 0,
        "six of the seven methods ran and a tool call reached the server: {exercised:?}"
    );

    adapter.apply(&delta).expect("apply");
    exercised.push("apply");
    let after = fixture.server().calls();
    println!("AC051_D3_AFTER_APPLY exercised={exercised:?} calls={after}");
    assert_eq!(
        after, 1,
        "`apply` made {after} calls where it should make one"
    );

    let mut sorted = exercised.clone();
    sorted.sort_unstable();
    let mut expected = derived.clone();
    expected.sort();
    assert_eq!(
        sorted, expected,
        "the probe exercised {sorted:?} and the trait declares {expected:?}: a method nobody drove \
         is a method this measurement says nothing about"
    );
}

fn fixture_intent(locator: &str) -> gx_core::Intent {
    intent_for(locator, WRITE_TOOL, GOAL)
}

/// 🔴 **A delta this adapter did not plan never reaches a server** — and the counter says so, not the
/// error.
///
/// This probe exists because the battery found its absence. Mutation (f) of `tools/verify_m7h3.sh`
/// removes `apply`'s `ForeignDelta` check, and the whole suite stayed green: `mcp_commutation.rs`
/// measures the refusal on **`commutation`**, which has its own check, and nothing drove a foreign
/// delta into `apply` at all. A payload another adapter minted with these bytes would then have been
/// decoded and **sent**.
///
/// So the assertion is on the counter as well as on the word. An `apply` that refused with the right
/// error *after* calling would satisfy the error half and is the failure this is about.
#[test]
fn a_delta_from_another_substrate_never_reaches_a_server() {
    use gx_core::SubstrateKind;
    use gx_substrate::PlannedDelta;

    let fixture = McpFixture::new();
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let pre = adapter.snapshot(&locator).expect("the server answers");
    let mine = adapter.plan(&fixture_intent(&locator), &pre).expect("plan");

    // The **same bytes** under another substrate: a payload this grammar can read, which is what
    // makes the refusal a fact about the substrate rather than about the decoder.
    let theirs = PlannedDelta::new(SubstrateKind::Fs, mine.payload().to_vec())
        .expect("the projection is encodable");

    let before = fixture.server().calls();
    let refusal = adapter
        .apply(&theirs)
        .expect_err("a delta of another substrate is not this adapter's to apply");
    println!(
        "AC051_FOREIGN kind={} calls_before={before} calls_after={}",
        refusal.kind(),
        fixture.server().calls()
    );
    assert_eq!(refusal.kind(), "ForeignDelta");
    assert_eq!(
        fixture.server().calls(),
        before,
        "the refusal came back and the call went out: the check has to be **before** the transport, \
         not beside it"
    );
}

// ---------------------------------------------------------------------------
// D-4 -- above the adapter, one road (Rule 2) (sem: SEM-gx-adapter-mcp-391)
// ---------------------------------------------------------------------------

/// The only call of `SubstrateAdapter::apply` in shipping code is the engine's, derived again from
/// this side.
///
/// req/78 §3.3, verbatim: "**Rule 2 (there is one road to `S`)**: the call site of `adapter.apply` must be **exactly one** (sem: SEM-gx-adapter-mcp-392)
/// place across the whole engine", and `gx-engine/tests/ac_035.rs` measures it inside that crate. This derives the (sem: SEM-gx-adapter-mcp-393)
/// same fact over **every** shipping crate, because Rule 2's own subject is the engine and what AC-051 (sem: SEM-gx-adapter-mcp-394)
/// needs is that nothing else calls one either.
#[test]
fn the_workspace_has_one_road_from_a_surface_to_a_substrate() {
    let crates = repo_root().join("crates");
    let mut sites: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("crates/ is readable") {
        let dir = entry.expect("an entry").path();
        let src = dir.join("src");
        if !src.is_dir() {
            continue;
        }
        // 🔴 **Rule 2 is about shipping code**, and "which crates ship" is a fact the manifests carry: (sem: SEM-gx-adapter-mcp-395)
        // `publish = false` is what `gx-substrate-conformance` declares about itself ("conformance is (sem: SEM-gx-adapter-mcp-396)
        // not for publishing", §29 M4H1-6). It calls `apply` eleven times, because it is the harness that (sem: SEM-gx-adapter-mcp-397)
        // runs the contracts -- and a probe that excluded it **by name** would be a declared list, which
        // is the thing ruling #10 forbids. So the predicate is read from the manifest. (sem: SEM-gx-adapter-mcp-398)
        //
        // 🔴 **`publish = false` alone stopped meaning "not shipping" once req/636's blanket guard
        // landed** ("no crate in this workspace is a designated publish target yet") and req/1032's
        // registry-rename decision left more crates private for reasons that have nothing to do with
        // whether they ship: `gx-adapter-mysql`/`gx-adapter-postgres`/`gx-confine`/`gx-mcp-wire` are
        // withheld per `req/1019`; `gx-cli`'s own manifest is the private, full-feature shape and a
        // separate public-safe variant lives at `public/crates/gx-cli/Cargo.toml`; `gx-gate` is
        // blocked from publishing only by `req/1032`'s Blocker B (an out-of-tree `include_str!`) and
        // its own comment says its `publish = false` "is therefore accurate, not a stale leftover" --
        // none of that makes any of the five **test-only**. Excluding them from this scan would hide
        // a real `adapter.apply(` call in real shipping code, which is the opposite of what Rule 2
        // is for. What actually singles `gx-substrate-conformance` out, in its own words, is that it
        // is a **"test instrument: it holds no product code"** -- a phrase deliberately written into
        // its manifest as a self-description (the same paragraph names `probes/doubt` as carrying
        // the identical line, which is why that directory is excluded separately, by the `crates`
        // walk root above never reaching it at all). Both conditions are read from the manifest
        // text, so this stays a derivation and not a declared list: a crate has to say **both** "I am
        // not published" and "I hold no product code" to be excluded, and today only one does.
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("a manifest");
        let publish_false = manifest.lines().any(|l| {
            let code = l.split('#').next().unwrap_or("").trim();
            code == "publish = false"
        });
        let test_instrument = manifest.contains("test instrument");
        if publish_false && test_instrument {
            skipped.push(
                dir.file_name()
                    .expect("named")
                    .to_string_lossy()
                    .into_owned(),
            );
            continue;
        }
        for file in walk(&src) {
            scanned += 1;
            let source = std::fs::read_to_string(&file).expect("a source file is readable");
            for (line, text) in code_lines(&source) {
                if text.contains("adapter.apply(") {
                    sites.push(format!(
                        "{}:{line}",
                        file.strip_prefix(&crates).expect("under crates/").display()
                    ));
                }
            }
        }
    }
    skipped.sort();
    println!("AC051_D4 files={scanned} apply_sites={sites:?} not_shipped={skipped:?}");
    assert_eq!(
        skipped.len(),
        1,
        "exactly one member declares `publish = false` today (the 51 §7 harness); if that changes,          the population this derivation is over changes with it: {skipped:?}"
    );
    assert!(
        scanned > 50,
        "the scan walked {scanned} files, which is not this workspace"
    );
    assert_eq!(
        sites.len(),
        1,
        "Rule 2 puts one road to a substrate in shipping code and these are what exist: {sites:?} (sem: SEM-gx-adapter-mcp-399)"
    );
    assert!(
        sites[0].starts_with("gx-engine/src/pipeline.rs"),
        "the one road is not the engine's: {sites:?}"
    );
}

// ---------------------------------------------------------------------------
// D-5 -- and that road passes the gate (the **integration test** AC-051 asks for) (sem: SEM-gx-adapter-mcp-400)
// ---------------------------------------------------------------------------

/// A Cedar pack that admits everything, and one that refuses this locator.
const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

const FORBID_THE_SUBJECT: &str = r#"@id("permit-everything")
permit (principal, action, resource);

@id("forbid-the-notes")
forbid (principal, action, resource)
when { resource.locator like "*#file:///srv/notes.md" };
"#;

struct Wired {
    server: Arc<FakeServer>,
    engine: Engine<InjectedEvidence>,
}

fn wire(name: &str, policies: &str) -> Wired {
    let server = Arc::new(FakeServer::new());
    let adapter = gx_adapter_mcp::McpAdapter::new(server.clone())
        .with_catalogue(support::catalogue())
        .with_log(Arc::new(RewindableLog::new()));
    let gate = gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(policies).expect("the fixture policy set parses"),
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    let mut engine = Engine::open(dir.join("journal.bin"), gate, InjectedEvidence::none())
        .expect("a fresh journal");
    engine.register_adapter(Arc::new(adapter), "gx-adapter-mcp ac_051");
    Wired { server, engine }
}

fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-mcp-1", &[9u8; 32])
}

/// 🔴 **A gate that refuses leaves the server untouched.**
///
/// This is the half of AC-051 that a scan cannot state: not "there is one road" but "the road stops (sem: SEM-gx-adapter-mcp-401)
/// where the gate says no". The transformation reaches `Denied` (43 T-4b) and the counter is zero -- (sem: SEM-gx-adapter-mcp-402)
/// and the counter is on the **server**, so it counts arrivals rather than intentions.
#[test]
fn a_denied_change_makes_no_tool_call() {
    let mut wired = wire("ac051_denied", FORBID_THE_SUBJECT);
    let intent = intent_for(&subject_locator(), WRITE_TOOL, GOAL);

    wired.engine.submit(&intent, 42, AT).expect("submit");
    let id = wired.engine.plan(&intent, AT).expect("plan");
    let state = wired
        .engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify answers with a state");

    println!(
        "AC051_D5_DENY state={state:?} verdict={:?} calls={} reads={}",
        wired.engine.verdict(&id),
        wired.server.calls(),
        wired.server.reads()
    );
    assert_eq!(state, Lifecycle::Denied);
    assert_eq!(wired.engine.verdict(&id), Some(VerdictKind::Deny));
    assert_eq!(
        wired.server.calls(),
        0,
        "the gate said no and a tool call still reached the server"
    );
    assert!(
        wired.server.reads() > 0,
        "the pipeline read the object to decide, which is what makes the zero above a fact about \
         calls rather than about a proxy nobody ran"
    );
    assert_eq!(
        wired.server.contents(SUBJECT).as_deref(),
        Some(support::INITIAL),
        "the resource moved under a denied change"
    );
}

/// **And an admitted change makes exactly one**, through submit → verify → commit.
#[test]
fn an_admitted_change_makes_exactly_one_tool_call_after_the_gate() {
    let mut wired = wire("ac051_admitted", PERMIT_ALL);
    let intent = intent_for(&subject_locator(), WRITE_TOOL, GOAL);

    wired.engine.submit(&intent, 42, AT).expect("submit");
    let id = wired.engine.plan(&intent, AT).expect("plan");

    let verified = wired
        .engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify answers with a state");
    let after_verify = wired.server.calls();
    assert_eq!(verified, Lifecycle::Admitted);
    assert_eq!(
        after_verify, 0,
        "the gate admitted and the call went out during **verify**: submit→verify is a decision and \
         not an effect"
    );

    let canonicalized = wired
        .engine
        .canonicalize(&id, AT, None)
        .expect("canonicalize answers with a state");
    assert_eq!(
        wired.server.calls(),
        0,
        "the canonical form was computed and the call went out during **canonicalize**: 41 §5's          commit protocol puts the effect after the CAS, not before it"
    );

    let committed = wired
        .engine
        .commit(&id, AT, &signing_key())
        .expect("commit answers with a state");
    println!(
        "AC051_D5_ADMIT verify_state={verified:?} canonicalized={canonicalized:?} \
         commit_state={committed:?} \
         calls_after_verify={after_verify} calls_after_commit={} unmatched_admissions={}",
        wired.server.calls(),
        wired.server.unmatched_admissions()
    );
    assert_eq!(committed, Lifecycle::Committed);
    assert_eq!(
        wired.server.calls(),
        1,
        "one admitted change is one tool call"
    );
    assert_eq!(wired.server.unmatched_admissions(), 0);
    assert_eq!(
        wired.server.contents(SUBJECT).as_deref(),
        Some(GOAL),
        "the call went out and the resource did not move"
    );
    assert!(
        wired.engine.receipt(&id).is_some(),
        "an admitted change that reached a server left no receipt"
    );
}

// ---------------------------------------------------------------------------
// D-6 — the second road to the wire, declared and bounded
// ---------------------------------------------------------------------------

/// 🔴 **D-6 (DR-46-9 A-3, `req/38` §196)** — the read-by-tool road is **one** site, it is in
/// `invert.rs`, and what it sends is the deployment's declaration rather than the agent's payload.
///
/// # Why this derivation had to be added rather than assumed
///
/// D-1 through D-5 close "no tool call happens outside `apply`" for `ToolTransport::call`, and
/// `read_prior_by_tool` is a **different method** that also frames a `tools/call`. So the road
/// count went from one to two, and `req/38` §196 made accounting for it an implementation
/// condition of the ruling ("the attack surface for the eighteenth audit is calculated
/// explicitly"). Leaving the old derivation to speak for a road it does not measure is exactly the
/// hand-written route table M6H8-9 was about.
///
/// # What the two halves are
///
/// * a **text gate** with its limit printed, D-2's form: the call sites under `src/`, counted and
///   named. Its blind spots are D-2's blind spots (an alias, a macro, a re-export);
/// * a **behavioural bound**, which is the part that matters and the part a scan cannot do: what
///   goes out is the tool named in the **catalogue** and arguments built from material the
///   **declaration** names, so an agent that supplies a whole payload cannot widen the set of
///   tools this road reaches. The transport below records what arrived and the assertions hold it
///   to the declaration.
///
/// # 🔴 The count moved from one to two, and it moved with an argument (**DR-46-16**)
///
/// `req/38` §218 ruling 1 gave the **compare-and-set** half the same second road the escrow
/// half got from DR-46-9 A-3: `snapshot`, `precondition` and the post-apply observation take a
/// declared read tool when a `$cas_read` prefix matches the resource. That is three positions in
/// two modules, and three call sites would have been three roads to bound. They are funnelled
/// through **one** function in `src/cas.rs`, so the crate's road count is **2** and both are named
/// below — `invert.rs` for the escrow half, `cas.rs` for the CAS half. The number is not the
/// property; the property is that each road is derived rather than declared, and that each is
/// bounded behaviourally by what its declaration may say.
///
/// 🔴 The CAS road's bound is **stronger** than the escrow road's, and for a structural
/// reason rather than a careful one: at `snapshot` there is no agent payload in existence. 41 §4
/// hands the method a locator, so every argument this road can send comes from the locator and
/// from constants the operator wrote. The second half of the behavioural assertion below measures
/// that rather than restating it.
///
/// 🔴 **What is still not derived**: an operator may declare a *writing* tool as a read face.
/// Nothing in MCP marks a tool read-only in a way a server cannot misstate, so this is declaration
/// soundness — the same burden the catalogue's "what undoes what" already carries (`req/38` §102
/// ruling 2) — and `docs/LIMITS.md` v0.5-d says it on the page a buyer reads.
#[test]
fn d6_the_read_by_tool_road_is_one_site_and_carries_only_the_declaration() {
    use std::sync::Mutex;

    use gx_adapter_mcp::{
        ArgSource, Catalogue, IdentityPart, McpAdapter, ObjectIdentity, PriorPointer, PriorRead,
        RestoreTemplate, ToolTransport,
    };
    use gx_substrate::{Result as SubstrateResult, SubstrateAdapter};

    // --- the text gate, D-2's form ---
    let src = repo_root().join("crates/gx-adapter-mcp/src");
    let files = walk(&src);
    let mut sites: Vec<String> = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("a source file is readable");
        let name = file
            .strip_prefix(&src)
            .expect("under src/")
            .display()
            .to_string();
        for (line, text) in code_lines(&source) {
            if text.contains("read_prior_by_tool(") && !text.contains("fn read_prior_by_tool") {
                sites.push(format!("{name}:{line}"));
            }
        }
    }
    sites.sort();
    let modules: Vec<&str> = sites
        .iter()
        .map(|site| site.split(':').next().expect("file:line"))
        .collect();
    println!(
        "AC051_D6 files={} read_by_tool_sites={sites:?}          LIMIT=text-gate(alias/macro/re-export invisible; the behavioural half below bounds what          these sites may send)",
        files.len()
    );
    assert_eq!(
        sites.len(),
        2,
        "🔴 the read-by-tool road is reached from a number of places this derivation does not          account for. Two are accounted for and bounded below: `invert.rs` (the escrow half,          DR-46-9 A-3) and `cas.rs` (the compare-and-set half, DR-46-16). A third is a road nothing          bounds, and raising this number without adding a behavioural bound for the new road is          the hand-written route table M6H8-9 was about: {sites:?}"
    );
    assert_eq!(
        modules,
        vec!["cas.rs", "invert.rs"],
        "🔴 the two sites are not the two modules that own the two halves. `invert.rs` is where          E-M4-30 puts the escrow, and `cas.rs` is the single funnel `snapshot`, `precondition` and          `observe` share — a site anywhere else means one of those three positions grew a road          of its own: {sites:?}"
    );

    // --- the behavioural bound ---
    #[derive(Debug, Default)]
    struct Recording {
        seen: Mutex<Vec<(String, String)>>,
    }
    impl ToolTransport for Recording {
        fn read(&self, _server: &str, _resource: &str) -> SubstrateResult<Vec<u8>> {
            Ok(Vec::new())
        }
        fn read_prior_by_tool(
            &self,
            _server: &str,
            tool: &str,
            arguments: &[u8],
        ) -> SubstrateResult<Vec<u8>> {
            self.seen.lock().expect("not poisoned").push((
                tool.to_string(),
                String::from_utf8_lossy(arguments).into_owned(),
            ));
            // 🔴 DR-46-15: the answer says which object it is about, and the declaration below
            // says how to read that. A read that answered for another `uri` refuses.
            Ok(format!(r#"{{"uri":"{}","body":"the prior"}}"#, support::SUBJECT).into_bytes())
        }
        fn call(
            &self,
            _call: &gx_adapter_mcp::ToolCall,
            _admitted: &gx_adapter_mcp::Admitted,
        ) -> SubstrateResult<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    let transport = Arc::new(Recording::default());
    let catalogue = Catalogue::new()
        .with_restore_template(
            "widget_write",
            "widget_write",
            RestoreTemplate::new()
                .with("id", ArgSource::Forward("id".to_string()))
                .with(
                    "body",
                    ArgSource::PriorJson(PriorPointer::Literal("/body".to_string())),
                ),
        )
        .with_prior_read(
            "widget_write",
            PriorRead::new(
                "widget_read",
                RestoreTemplate::new().with("id", ArgSource::Forward("id".to_string())),
                ObjectIdentity::new(vec![IdentityPart::Answer {
                    answer: "/uri".to_string(),
                }]),
            ),
        );
    let adapter = McpAdapter::new(transport.clone()).with_catalogue(catalogue);

    let locator = subject_locator();
    let pre = support::absent_snapshot(&locator);
    // The agent's payload names a tool of its own and carries a member nobody declared. Neither
    // may reach the wire on this road.
    let arguments = br#"{"id":"w-1","body":"new","tool":"admin_delete_everything"}"#;
    let delta = adapter
        .plan(&intent_for(&locator, "widget_write", arguments), &pre)
        .expect("a well-formed call plans");
    let inverse = adapter
        .invert(&delta, &pre)
        .expect("the recording transport answers")
        .into_inverse()
        .expect("an inverse was constructed");

    let seen = transport.seen.lock().expect("not poisoned").clone();
    println!(
        "AC051_D6_SENT {seen:?} inverse_bytes={}",
        inverse.payload().len()
    );
    assert_eq!(seen.len(), 1, "one escrow, one read: {seen:?}");
    assert_eq!(
        seen[0].0, "widget_read",
        "the tool that went out is not the one the catalogue names: {seen:?}"
    );
    assert_eq!(
        seen[0].1, r#"{"id":"w-1"}"#,
        "the read carried something other than the declared members. The agent's `body` and its \
         `tool` are in the forward payload and neither is declared, so neither may travel: {seen:?}"
    );
    // --- the CAS road's behavioural bound (DR-46-16) ---
    //
    // 🔴 The same question on the other road, asked on a **fresh** transport so that the escrow
    // half's arrival above cannot be mistaken for this one. What is measured is that `snapshot`
    // reaches the tool the `$cas_read` declaration names, with arguments built from the locator:
    // at this position there is no forward payload for anything else to come from.
    let cas_transport = Arc::new(Recording::default());
    let cas_adapter = McpAdapter::new(cas_transport.clone()).with_catalogue(
        Catalogue::new().with_cas_read(
            "widget://",
            gx_adapter_mcp::CasRead::new(
                "widget_cas_read",
                gx_adapter_mcp::CasTemplate::new()
                    .with("id", gx_adapter_mcp::CasArgSource::ResourceSuffix),
            ),
        ),
    );
    let _ = cas_adapter.snapshot("stdio://widgets#widget://w-7");
    let cas_seen = cas_transport.seen.lock().expect("not poisoned").clone();
    println!("AC051_D6_CAS_SENT {cas_seen:?}");
    assert_eq!(
        cas_seen.len(),
        1,
        "🔴 one CAS read, one arrival: `snapshot` takes the declared road instead of          `resources/read`, never in addition to it: {cas_seen:?}"
    );
    assert_eq!(
        cas_seen[0].0, "widget_cas_read",
        "the tool that went out is not the one the catalogue names: {cas_seen:?}"
    );
    assert_eq!(
        cas_seen[0].1, r#"{"id":"w-7"}"#,
        "🔴 the read carried something other than what the locator and the declaration supply.          At `snapshot` there is no agent payload in existence, so this is the whole of what this          road can ever send: {cas_seen:?}"
    );
}
