//! **AC-051**: there is no road to a tool call that does not pass a gate, and the road count is
//! **derived**.
//!
//! 34 AC-051 逐語: 「Given: `gx-adapter-mcp`プロキシ配下のtool-call境界。When: プロキシを経由しない直接呼び
//! 出し経路の有無を**構成レベルで検査**し、プロキシ経由呼び出しを実行する。Then: 全tool-callがsubmit→verify
//! 経路を必ず通過し、バイパス手段が技術的に存在しない（direct呼び出し経路がブロックされる）ことを**結合テスト**
//! で確認する。」
//!
//! 裁定 #10 (`req/38` §56) fixes the **form**: 「AC-051 検査形式=**導出に格上げ**(宣言 list 禁)」, with
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
//! | **D-4** | above the adapter there is one road | every `.rs` file under `crates/*/src` | a scan for `SubstrateAdapter::apply` call sites (則 2, req/78 §3.3) |
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
//!   AC-051's subject is 「プロキシ配下の tool-call 境界」 and making the proxy the only reachable endpoint
//!   is a deployment's job — the same **N-05** disclosure the fs and git adapters carry, and 45 §2.2's
//!   「adapter経由の完全性のみ」.
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

/// Source lines that are not comments. The same filter M6's 則 1 counters use.
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
/// **M4-20 採(b)**'s pair form, and the reason it is a pair: a `compile_fail` block on its own proves
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
/// 🔴 This is where 裁定 #10's 「宣言 list 禁」 bites hardest. A probe that typed the six read-only method
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
    let inverse = adapter.invert(&delta, &pre).expect("invert answers");
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
// D-4 — above the adapter, one road (則 2)
// ---------------------------------------------------------------------------

/// The only call of `SubstrateAdapter::apply` in shipping code is the engine's, derived again from
/// this side.
///
/// req/78 §3.3 逐語: 「**則 2(`S` への道は 1 本)**: `adapter.apply` の呼び出し箇所は engine 全体で **1 箇所**
/// でなければならない」, and `gx-engine/tests/ac_035.rs` measures it inside that crate. This derives the
/// same fact over **every** shipping crate, because 則 2's own subject is the engine and what AC-051
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
        // 🔴 **則 2 is about shipping code**, and 「which crates ship」 is a fact the manifests carry:
        // `publish = false` is what `gx-substrate-conformance` declares about itself (「conformance は
        // publish 対象外」, §29 M4H1-6). It calls `apply` eleven times, because it is the harness that
        // runs the contracts -- and a probe that excluded it **by name** would be a declared list, which
        // is the thing 裁定 #10 forbids. So the predicate is read from the manifest.
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("a manifest");
        if manifest.lines().any(|l| l.trim() == "publish = false") {
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
        "則 2 puts one road to a substrate in shipping code and these are what exist: {sites:?}"
    );
    assert!(
        sites[0].starts_with("gx-engine/src/pipeline.rs"),
        "the one road is not the engine's: {sites:?}"
    );
}

// ---------------------------------------------------------------------------
// D-5 — and that road passes the gate (the 結合テスト AC-051 asks for)
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
/// This is the half of AC-051 that a scan cannot state: not 「there is one road」 but 「the road stops
/// where the gate says no」. The transformation reaches `Denied` (43 T-4b) and the counter is zero —
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
