//! 🔴 **R-9 の対裁定** — what a `gx` that was told nothing starts from (**M7 hand 4**).
//!
//! `req/38` §60: 「**R-9 採=手 4 の対裁定**。既定 policy set と既定 registry は**対**で決まる(pack を
//! 足しても adapter が register されなければ NotFound)」. req/101 §9-1 measured the state that ruling
//! is about and named the third change点 as the one nobody had decided:
//!
//! > **同時に adapter の register も決めねばならない**——policy set だけ広げても、adapter が居なければ
//! > `Error::NotFound`(「substrate に adapter が register されていない」)で止まる。∴ 裁定は 1 つでなく
//! > **対**である: 「既定の policy set」と「既定の registry」。
//!
//! So this file measures both halves and the seam between them. The policy half is
//! [`gx_cli::session::default_policies`]; the registry half is
//! [`gx_cli::session::register_default_adapters`]; and the seam is that a substrate with a pack and
//! no adapter is indistinguishable, from a `gx` invocation, from a substrate with neither.
//!
//! # The half this hand did **not** make true, and why it is a probe rather than a paragraph
//!
//! 44 §1.2's `--substrate` takes `fs|git|mcp`, and after this hand a `gx` binary registers **two** of
//! the three. `gx-adapter-mcp` cannot be constructed without a `ToolTransport`, which is the
//! deployment's code by a ruling this project has already made twice (the crate's own manifest: 「a
//! linked MCP client library would be a **second road** to the substrate」, and req/101 §9 R-2 files
//! the missing wire as **deliberate**). Registering it behind a transport that refuses everything
//! would buy nothing — the refusal would land in `snapshot`, before a gate, so the mcp pack would
//! still be unreachable — and would cost a made-up `adapter_version` in a signed provenance record.
//!
//! So the answer is 「not yet」, and D-7's rule is that a deferral carries a firing condition.
//! [`gx_cli::session::MCP_IS_NOT_REGISTERED`] is that condition in a place a probe reads, and
//! [`the_deferral_of_the_mcp_adapter_names_its_firing_condition`] is what stops it from decaying
//! into a memory.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gx_cli::session::{
    default_policies, open_engine, register_default_adapters, DEFAULT_SUBSTRATES,
    MCP_IS_NOT_REGISTERED,
};
use gx_core::{Actor, ChangeContext, GoalBytes, Intent, SubstrateKind, Timestamp};
use gx_engine::InjectedEvidence;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn intent(substrate: SubstrateKind, locator: &str) -> Intent {
    Intent::new(
        substrate,
        locator.to_string(),
        GoalBytes(b"a goal this test never applies".to_vec()),
        ChangeContext::Substrate,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

/// A repository on a tmpfs, holding one commit on `refs/heads/main` with a `README.md` in it.
///
/// Written with `gix` rather than by shelling out to `git`: an external process in a test is a second
/// road to the substrate, and this crate is already the one that has to be sure which road it took.
/// The tmpfs requirement is `crates/gx-adapter-git/tests/support/mod.rs`'s and is asserted rather
/// than assumed — a repository on drvfs fails at the reference lock, several layers away from
/// anything this probe is about.
fn tmpfs_repo(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var("GLOVREX_GIT_TEST_ROOT").unwrap_or_else(|_| "/dev/shm".to_string()),
    );
    assert!(
        root.is_dir(),
        "{} is not a directory; set GLOVREX_GIT_TEST_ROOT to a tmpfs",
        root.display()
    );
    let dir = root.join(format!("glovrex-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a tmpfs accepts a directory");

    let repo = gix::init(&dir).expect("gitoxide creates a repository");
    let blob = repo
        .write_blob(b"# a repository this test never pushes\n")
        .expect("a blob is written")
        .detach();
    let tree = repo
        .write_object(&gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: "README.md".into(),
                oid: blob,
            }],
        })
        .expect("a tree is written")
        .detach();
    let signature = gix::actor::Signature {
        name: "gx-fixture".into(),
        email: "fixture@glovrex.invalid".into(),
        time: gix::date::Time::new(1_754_000_000, 0),
    };
    let commit = repo
        .write_object(&gix::objs::Commit {
            tree,
            parents: Vec::new().into(),
            author: signature.clone(),
            committer: signature.clone(),
            encoding: None,
            message: "fixture\n".into(),
            extra_headers: Vec::new(),
        })
        .expect("a commit is written")
        .detach();
    // The reference is written through a transaction carrying an explicit committer, not through
    // `Repository::reference`: the latter takes the identity from the repository's configuration and
    // this repository has none, so it answers `CreateOrUpdateRefLog(MissingCommitter)` — measured
    // 2026-08-12, and it is the same refusal that stopped
    // `crates/gx-adapter-git/tests/h2_normalised_before_the_gate.rs` from creating a remote-tracking
    // reference until this probe found the real cause.
    let edit = gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: "fixture".into(),
            },
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Object(commit),
        },
        name: gix::refs::FullName::try_from("refs/heads/main").expect("a full reference name"),
        deref: false,
    };
    repo.refs
        .transaction()
        .prepare(
            vec![edit],
            gix::lock::acquire::Fail::Immediately,
            gix::lock::acquire::Fail::Immediately,
        )
        .expect("the reference transaction prepares")
        .commit(signature.to_ref(&mut gix::date::parse::TimeBuf::default()))
        .expect("the reference transaction commits");
    dir
}

/// An engine opened exactly as every verb of this binary opens one.
fn engine_in(name: &str) -> gx_engine::Engine<InjectedEvidence> {
    let project = scratch(name);
    let layout = gx_cli::layout::Layout::create(&project).expect("create .gx/");
    open_engine(
        &layout,
        InjectedEvidence::new(Vec::new()),
        None,
        gx_core::FailPosture::FailClosed,
        None,
    )
    .expect("the shipped set parses and the journal opens")
}

// ---------------------------------------------------------------------------
// The policy half
// ---------------------------------------------------------------------------

/// The default policy set is every pack the build ships, not the fs one.
///
/// Derived from `gx_gate::packs::SHIPPED_PACKS` on both sides rather than written out: a pack added
/// in a later hand joins the default set, and a pack that stopped being loaded is a failure here.
#[test]
fn the_default_policy_set_is_every_shipped_pack() {
    let set = default_policies().expect("the shipped set parses");
    let mut declared: Vec<String> = gx_gate::packs::SHIPPED_PACKS
        .iter()
        .flat_map(|p| p.policy_ids.iter().map(|id| (*id).to_string()))
        .collect();
    declared.sort();
    println!(
        "CLI_DEFAULT_POLICY_IDS={:?} PACKS={}",
        set.policy_ids(),
        gx_gate::packs::SHIPPED_PACKS.len()
    );
    assert_eq!(
        set.policy_ids(),
        declared,
        "a `gx` told no `--policy` decides with every shipped pack (req/38 §60's R-9 対裁定)"
    );
}

// ---------------------------------------------------------------------------
// The registry half
// ---------------------------------------------------------------------------

/// The default registry is the declared set, and the declaration is checked against what registers.
///
/// Both directions, as `FS_PACK_POLICY_IDS` is checked against its pack: a constant nobody compares
/// with the artifact is a constant that drifts.
#[test]
fn the_default_registry_is_the_declared_set() {
    let mut engine = engine_in("defaults_registry");
    let registered = register_default_adapters(&mut engine);
    println!("CLI_DEFAULT_SUBSTRATES declared={DEFAULT_SUBSTRATES:?} registered={registered:?}");
    assert_eq!(
        registered, DEFAULT_SUBSTRATES,
        "the substrates that register are the ones declared"
    );
}

/// 🔴 A `--substrate git` intent reaches the **git adapter**, and fails as that adapter fails.
///
/// The discrimination this probe exists for: before this hand the answer was `NotFound` (「substrate
/// に adapter が register されていない」), and 44 §1.2 documented a flag that could therefore never
/// work. M4H5-5's rule — an argument that changes nothing must not look like one that worked — has a
/// twin here: a flag the documentation offers must not be one no invocation can satisfy.
///
/// The repository does not exist, so the adapter refuses; what matters is **which layer** refuses.
#[test]
fn a_git_intent_reaches_the_git_adapter_rather_than_an_empty_registry() {
    let mut engine = engine_in("defaults_git");
    let intent = intent(
        SubstrateKind::Git,
        "/nonexistent/repo#refs/heads/main:README.md",
    );
    let submitted = engine
        .submit(&intent, 0, Timestamp(1_754_000_000_000_000_000))
        .expect("a draft is recorded before any substrate is touched");
    let planned = engine.plan(&intent, Timestamp(1_754_000_000_000_000_001));
    println!("CLI_GIT_PLAN submitted={submitted:?} answer={planned:?}");
    match planned {
        Err(gx_engine::Error::Adapter { action, .. }) => {
            assert_eq!(
                action, "snapshot",
                "the git adapter refused a repository that is not there, which is its own refusal \
                 and not the registry's"
            );
        }
        other => panic!(
            "a git intent must reach the git adapter. `NotFound {{ what: \"adapter\" }}` here is \
             the state req/38 §60 ruled on: {other:?}"
        ),
    }
}

/// 🔴 And an `--substrate mcp` intent does **not** — for the reason the constant gives.
///
/// The pair to the probe above. This one is the half hand 4 left open, measured rather than
/// described: the pack ships, the policy set holds it, and no invocation of this binary can reach
/// it, because the wire is the deployment's.
#[test]
fn an_mcp_intent_is_refused_by_the_registry_and_not_by_a_pretend_adapter() {
    let mut engine = engine_in("defaults_mcp");
    let intent = intent(
        SubstrateKind::Mcp,
        "https://mcp.example/sse#file:///srv/notes.md",
    );
    engine
        .submit(&intent, 0, Timestamp(1_754_000_000_000_000_000))
        .expect("a draft is recorded before any substrate is touched");
    let planned = engine.plan(&intent, Timestamp(1_754_000_000_000_000_001));
    println!("CLI_MCP_PLAN answer={planned:?}");
    match planned {
        Err(gx_engine::Error::NotFound { what, .. }) => assert_eq!(
            what, "adapter",
            "the refusal names the registry, so an operator is told the truth: this build has no \
             MCP wire"
        ),
        other => panic!(
            "an mcp intent has no adapter in this binary, and the refusal must say so rather than \
             coming from a transport that pretends: {other:?}"
        ),
    }
}

/// 🔴 **The seam itself**: an engine opened by this binary *decides with* the composed set.
///
/// This probe exists because the battery said the others were not enough. Point (e) of
/// `tools/verify_m7h4.sh` puts `open_engine`'s `None` arm back to `packs::fs_pack()` — the state
/// hand 4 started from — and on the first run **nothing went red**: every probe above measures
/// `default_policies` or `register_default_adapters`, and neither of them measures *the branch that
/// calls them*. A correct function an `open_engine` ignores is the same failure as a wrong function.
///
/// So this drives the real surface: a git transformation on a branch, through submit → plan →
/// verify, against the engine `open_engine` built. The arm is the discrimination — under the
/// composed set `git-permit-default` **admits** it, and under the fs pack alone the same request
/// reaches Cedar's third rule and is **denied** by nothing at all.
///
/// 🔴 The repository is made on a **tmpfs**, and `crates/gx-adapter-git/tests/support/mod.rs` carries
/// the reason at length: `/mnt/c` is 9p over NTFS and does not give the POSIX rename semantics a
/// reference lock is renamed with. `.gx/` itself stays under `CARGO_TARGET_TMPDIR` like every other
/// suite in this crate — a journal is appended to, not renamed under a lock.
#[test]
fn an_engine_this_binary_opened_decides_a_git_change_with_the_git_pack() {
    let repo_dir = tmpfs_repo("defaults_seam");
    let locator = format!("{}#refs/heads/main:README.md", repo_dir.display());
    let mut engine = engine_in("defaults_seam_gx");
    let intent = intent(SubstrateKind::Git, &locator);
    let at = Timestamp(1_754_000_000_000_000_000);

    engine.submit(&intent, 0, at).expect("a draft is recorded");
    let id = engine
        .plan(&intent, Timestamp(at.0 + 1))
        .expect("the git adapter is registered and the branch is readable");
    let key = gx_witness::KeyPair::from_seed("key-agent-1", &[7u8; 32]);
    let state = engine
        .verify(&id, Timestamp(at.0 + 2), &key, None)
        .expect("the gate reached a decision");

    println!("CLI_GIT_VERDICT locator={locator:?} state={state:?}");
    assert_eq!(
        state,
        gx_engine::Lifecycle::Admitted,
        "a change to an entry on a branch is what `git-permit-default` admits, and it is what the \
         wedge exists to mediate. `Denied` here means the engine this binary opened is deciding \
         with a policy set that has no opinion about git — which is exactly the state req/38 §60 \
         ruled on, and which every other probe in this file would have missed"
    );
}

/// The deferral carries a firing condition, and the condition is not 「a later hand」 (**D-7**).
///
/// The shape `crates/gx-cli/src/consumers.rs` uses for M6-21 and M6-22: the decision lives where a
/// reader of the code lands, and a probe keeps it from becoming a sentence nobody re-reads.
#[test]
fn the_deferral_of_the_mcp_adapter_names_its_firing_condition() {
    println!(
        "MCP_IS_NOT_REGISTERED={} chars",
        MCP_IS_NOT_REGISTERED.len()
    );
    for clause in ["ToolTransport", "R-2", "44 §1.2", "firing condition"] {
        assert!(
            MCP_IS_NOT_REGISTERED.contains(clause),
            "the deferral must state {clause:?}: a deferral without a condition is the 「a hand will \
             do it later」 M5H4-8 refused to accept"
        );
    }
    assert!(
        !DEFAULT_SUBSTRATES.contains(&"mcp"),
        "and it must be a deferral rather than a claim: if mcp registers, this constant is stale"
    );
}

/// 🔴 **Every pack this build ships that no invocation of this binary can reach** — as a *derived
/// difference* rather than as a name (**M7 fix批 ②, 起票 A-2**: `req/38` §64, `req/105` §5-2).
///
/// # What was already true, and why it was not enough
///
/// The probe above pins `mcp` **by name**, and today that is correct: one pack ships that nothing
/// registers, and `MCP_IS_NOT_REGISTERED` carries its firing condition. req/105 §5-2 measured what
/// that costs and named the file it costs it in:
///
/// > **問題は 4 本目である**: register されない substrate 向けの pack が将来 1 本出荷された日、それを
/// > 名指す probe は誰も書いていないので、**「出荷されているが 1 度も評価されない pack」が黙って 2 本に
/// > なる**。
///
/// A pack in `SHIPPED_PACKS` whose substrate has no adapter is loaded into every default policy set
/// ([`the_default_policy_set_is_every_shipped_pack`]) and then decides nothing, ever, because
/// `plan` refuses with `NotFound { what: "adapter" }` before a gate is reached. That is a shipped
/// artefact with no consumer — §30's disease with the arrow reversed — and until this probe the
/// only thing standing between it and silence was that today's instance happened to be spelled out.
///
/// # The shape, which is A6's
///
/// `crates/gx-canon/tests/authority_boundary.rs::the_declared_surfaces_are_the_surfaces_this_
/// workspace_has` is the precedent: a hand-written list stays (a decision somebody writes down is
/// still what it is) and is **compared against a derivation**, so that a member appearing without
/// being written down is red rather than invisible. Here both sides are derived — the shipped set
/// from `SHIPPED_PACKS`, the reachable set from what [`register_default_adapters`] actually
/// returns after registering — and the *difference* is the value asserted. A fourth pack for an
/// unregistered substrate lands in that difference on the day it ships, with no edit here.
///
/// # Both readings of the difference, because they are different claims
///
/// req/38 §60's 対裁定 is about the **pair** 「既定 policy set と既定 registry」, and the registry has
/// a declaration ([`DEFAULT_SUBSTRATES`]) and an actual ([`register_default_adapters`]'s derived
/// return). Reachability is a fact about the actual; the declaration is what a reader of the source
/// believes. They agree today — [`the_default_registry_is_the_declared_set`] is the assertion that
/// says so — and this probe takes the difference **twice**, once against each, and requires the two
/// answers to match. If the constant and the code ever part, the neighbouring probe goes red for
/// the drift and this one stays honest about which set it measured.
#[test]
fn the_shipped_packs_no_default_binary_can_reach_are_a_derived_difference() {
    let shipped: BTreeSet<String> = gx_gate::packs::SHIPPED_PACKS
        .iter()
        .map(|pack| pack.substrate.to_string())
        .collect();

    let mut engine = engine_in("defaults_unreachable");
    let registered: BTreeSet<String> = register_default_adapters(&mut engine).into_iter().collect();
    let declared: BTreeSet<String> = DEFAULT_SUBSTRATES
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    let unreachable: BTreeSet<String> = shipped.difference(&registered).cloned().collect();
    let unreachable_by_declaration: BTreeSet<String> =
        shipped.difference(&declared).cloned().collect();

    println!(
        "CLI_SHIPPED_PACK_SUBSTRATES={shipped:?} REGISTERED={registered:?} DECLARED={declared:?}"
    );
    println!(
        "CLI_UNREACHABLE_PACK_SUBSTRATES={unreachable:?} COUNT={} DERIVED_FROM=difference(SHIPPED_PACKS.substrate, register_default_adapters)",
        unreachable.len()
    );

    assert_eq!(
        unreachable,
        BTreeSet::from(["mcp".to_string()]),
        "the packs this build ships and cannot reach are {unreachable:?}. One is expected and it is \
         `mcp`, for the reason `MCP_IS_NOT_REGISTERED` gives. A **second** entry here is a pack \
         that ships, joins every default policy set, and is never evaluated by anything — decide \
         whether its adapter registers or whether the pack ships, and if the answer is 「not yet」 \
         then it needs a firing condition of its own (D-7). An **empty** set here means an adapter \
         started registering without this file being told, and `MCP_IS_NOT_REGISTERED` is now a \
         paragraph describing a state the binary left"
    );
    assert_eq!(
        unreachable, unreachable_by_declaration,
        "the difference taken against what registers and the difference taken against \
         DEFAULT_SUBSTRATES disagree, so the constant and the code have parted and the sentence \
         「出荷したのに 1 度も評価されない pack」 no longer has one meaning"
    );
}
