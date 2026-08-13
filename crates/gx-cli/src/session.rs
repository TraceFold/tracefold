//! An engine over a project's `.gx/`, and the resume that makes four processes one pipeline.
//!
//! # 🔴 The hole this hand found, and why it is not M6-01
//!
//! req/88 §4 M6-01 is about the **intent body** between `gx submit` and `gx plan`, and hand 1 closed
//! it with [`crate::draft`]. What this hand ran into one step later is the **row**:
//!
//! * `Engine::open` rebuilds the draft phase and 44 §0's resolution index from the journal, and
//!   deliberately leaves the in-flight table empty (M5H3-5);
//! * a row holds a `Transformation`, an `ObjectSnapshot` and a `PlannedDelta`, and the journal holds
//!   names and digests rather than bodies (ASM-9);
//! * so `gx verify <TID>` in a fresh process is a call about a row that is not there, and
//!   `gx commit <TID>` is the same one state later.
//!
//! req/88 §2.1 marked `gx plan` as the blocker and marked `verify` and `commit` 「条件つき可」 on the
//! strength of `--evidence` and `--record-only`. The table was the condition nobody had named.
//! Raised as **M6H3-1**.
//!
//! # The answer: the body is re-planned, the state comes from the journal
//!
//! [`Session::resume`] finds the draft the transformation came from and calls `Engine::plan` again.
//! 43 T-2's idempotency column is the licence — 「同一snapshotに対し再実行しても同一`PlannedDelta`・
//! 同一`TransformationId`（安全に再試行可）」 — and M6 hand 3's change to `Engine::plan` is what makes
//! it *silent*: a re-plan that lands on the recorded id rebuilds the row from the journal's state
//! and appends no second `Planned` record.
//!
//! That silence is the whole point, and it is **req/88 §3 Λ2** rather than tidiness. Λ2 claims that
//! 「N 回の CLI 実行」 and 「1 個の長寿命 engine への N 回の呼び出し」 are observationally equal on Σ.
//! A resume that re-drove the pipeline would put a second `VerifyStarted` and a second `Verdict` in
//! the log and issue a second verdict receipt — the single-shot CLI and `gx serve` disagreeing about
//! how many times the gate was asked, which is exactly the equality Λ2 asserts. So the resume writes
//! nothing, and Λ2 now holds from `Candidate` onward rather than only up to it. **The one place it
//! still does not hold is the Draft phase**, which is Λ2's own named counter-example
//! (`.gx/drafts/` is CLI-side state the HTTP surface does not have) and is written in the crate
//! root.
//!
//! # 🔴 What the world moving does
//!
//! A re-plan consults the substrate, so if the file changed between `gx plan` and `gx verify` the
//! recomputed `TransformationId` is a **different one**. That is not a failure of the resume: 43 §8
//! says the same thing about a committed predecessor (「`Fingerprint₀`は陳腐化しているため再`plan()`
//! （再fingerprint）を強制する」). The engine refuses it without writing anything when the recorded
//! row has moved past `Candidate`, and [`Session::resume`] refuses it here when the ids differ, so
//! an operator is told that the world moved rather than being handed a transformation they did not
//! name.
//!
//! # 🔴 Which key signs, and the separation that is not implemented (M6H3-4)
//!
//! `Engine::verify` and `Engine::commit` both take a `&KeyPair` — T-4's verdict receipt and T-11's
//! commit receipt are signed — and 44 §1.2 gives neither command a key argument. req/56 §3 gives the
//! CLI **one** key directory. So the key this module loads is the one the transformation's own
//! `Actor` names, out of `~/.gx/keys/`.
//!
//! 45 §1 distinguishes a ledger/engine signing key from an actor key, and **that separation is not
//! implemented at this layer**: in v0.1 the receipt over a change is signed by the key of the actor
//! who asked for it. The consequence is worth stating plainly rather than leaving in a diagram — a
//! receipt says 「this actor's key attests this change was verified」 and not 「the engine attests
//! it」, so an operator holding their own key can produce a receipt without the engine's consent
//! being a separate fact. `gx log checkpoint` (hand 2) is the one place the ledger key is already an
//! explicit `--key`, which is why the ledger head is not affected. Raised as **M6H3-4**.

use std::path::Path;

use gx_core::{EnforcementMode, Intent, IntentId, Timestamp, TransformationId};
use gx_engine::{reconstruct, Engine, InjectedEvidence, StateRow};
use gx_gate::Gate;
use gx_witness::KeyPair;

use crate::draft::DraftStore;
use crate::index::ResolutionIndex;
use crate::keys::KeyStore;
use crate::layout::Layout;
use crate::{Error, Result};

/// The version string a registered adapter is recorded under (42 §3.9's `adapter_version`).
///
/// **M5H4-4**: 41 §4's trait has seven methods and none reports a version, so the registrant says.
/// This binary's registrant is this file, and what it knows is its own package version — which is
/// the honest answer to 「which build of the fs adapter was wired in」 for a single static binary
/// (NFR-019), where the adapter and the CLI are one artefact.
pub const ADAPTER_VERSION: &str = concat!("gx-cli ", env!("CARGO_PKG_VERSION"));

/// 🔴 The substrates a `gx` binary can act on when it was told nothing (**M7 hand 4**).
///
/// `req/38` §60's **R-9 対裁定**: 「既定 policy set と既定 registry は**対**で決まる(pack を足しても
/// adapter が register されなければ NotFound)」. [`default_policies`] is the first half and
/// [`register_default_adapters`] is the second; this constant is what the second half is checked
/// against, in the shape [`gx_gate::packs::FS_PACK_POLICY_IDS`] is checked against its pack —
/// declared here, derived there, compared by `crates/gx-cli/tests/defaults.rs`.
///
/// The spellings are 44 §1.2's, which is the vocabulary [`crate::substrate_kind`] reads.
pub const DEFAULT_SUBSTRATES: [&str; 2] = ["fs", "git"];

/// 🔴 Why the third of 44 §1.2's three substrates is not in [`DEFAULT_SUBSTRATES`], and what would
/// change that.
///
/// The deferral is the honest half of hand 4's answer to R-9 and it carries a firing condition,
/// because **D-7**'s rule is that a deferral without one is 「a hand will do it later」 — which
/// §41 M5H4-8 refused to accept and this project has since armed twice
/// (`crates/gx-cli/src/consumers.rs`).
pub const MCP_IS_NOT_REGISTERED: &str = "\
`gx-adapter-mcp` is **not** in the default registry, and the reason is that it cannot be built \
without one: `McpAdapter::new` takes a `ToolTransport`, which is the deployment's code by a ruling \
this project has made twice — the crate's own manifest (「a linked MCP client library would be a \
second road to the substrate, with no `Admitted` in its signature, reachable by anyone who can name \
the crate」) and req/101 §9 **R-2**, which files the absent JSON-RPC framing as deliberate rather \
than unfinished. Registering it behind a transport that refused everything would buy nothing — the \
refusal lands in `snapshot`, before a gate, so `policies/mcp/` would still be unreachable from this \
binary — and would cost a made-up `adapter_version` in a signed provenance record, which is the \
mistake M5H4-4 named when it made the version an argument. \
The **firing condition**: the day a wire ships (v0.2's `CallLog`/transport window, req/101 §9 R-1 \
and R-2), `gx serve` and the single-shot verbs register it here and 44 §1.2's `--substrate mcp` \
stops being a flag no invocation of this binary can satisfy. Until then the refusal an operator \
sees is `NotFound { what: \"adapter\" }`, which is the true one: this build has no MCP wire.";

/// 🔴 The policy set a `gx` decides with when no `--policy` named one (**M7 hand 4**).
///
/// Until this hand it was `packs::fs_pack()`, and `crates/gx-gate/src/packs.rs` carried the note
/// saying why that had to change the day a second adapter was registered. req/38 §60 ruled the day,
/// req/101 §9-1 is the material it took, and this is its second change点: 「`gx_cli::session::
/// open_engine` の `None` 分岐を `fs_pack()` から `shipped_pack_set()` へ」.
///
/// # 則 1 (ii) is untouched by the widening
///
/// What this crate does is **choose** which set the engine judges with; it does not evaluate one,
/// read a decision or name a `Verdict`. A wider set is a wider choice and not a new capability, and
/// the bytes still come from gx-gate's `include_str!` rather than from a file this binary opens
/// (FR-028's 「pack の file が build に埋め込まれる経路が 1 本」).
///
/// # Errors
/// [`Error::Gate`] if the composition does not parse — which for compile-time-fixed inputs means a
/// broken shipped artifact, and includes the case of two packs claiming one statement id.
pub fn default_policies() -> Result<gx_gate::PolicyEngine> {
    Ok(gx_gate::packs::shipped_pack_set()?)
}

/// 🔴 Put the adapters this binary ships into an engine's registry, and say which arrived.
///
/// The second half of the pair. The return value is **derived** — each adapter is asked its own
/// [`gx_substrate::SubstrateAdapter::kind`] after it is registered — so that
/// [`DEFAULT_SUBSTRATES`] is a declaration something compares with reality rather than a comment
/// with a constant's syntax.
///
/// req/88 §1 N-13 already ruled that a CLI shipping adapters is not a breach of anything: 「CLI は
/// 『どの substrate か』を `--substrate` で受けて registry に register する層であり、adapter を知らな
/// ければ何もできない。N-13 が守るのは engine の中立性であって、CLI の中立性ではない」. `gx-engine`'s
/// own manifest still declares no adapter and `ENGINE_SHIPPED_ADAPTERS=0` is unmoved.
pub fn register_default_adapters<E: gx_engine::EvidenceSource>(
    engine: &mut Engine<E>,
) -> Vec<String> {
    let adapters: [std::sync::Arc<dyn gx_substrate::SubstrateAdapter>; 2] = [
        std::sync::Arc::new(gx_adapter_fs::FsAdapter::new()),
        std::sync::Arc::new(gx_adapter_git::GitAdapter::new()),
    ];
    let mut registered = Vec::with_capacity(adapters.len());
    for adapter in adapters {
        registered.push(substrate_tag(&adapter.kind()));
        engine.register_adapter(adapter, ADAPTER_VERSION);
    }
    registered
}

/// The spelling 44 §1.2 gives a substrate, from the value 42 §3.1 gives it.
///
/// The inverse of [`crate::substrate_kind`], which is the parser for the same vocabulary. Written
/// here rather than borrowed from `gx-gate`'s `policy` module (which has the same table for Cedar's
/// benefit) because that one is private and because the two answer different questions: this is
/// 「what did the operator type」 and that one is 「what does a policy compare against」. They agree,
/// and `crates/gx-cli/tests/defaults.rs` is where the agreement is checked against a real registry.
fn substrate_tag(kind: &gx_core::SubstrateKind) -> String {
    match kind {
        gx_core::SubstrateKind::Fs => "fs".to_string(),
        gx_core::SubstrateKind::Git => "git".to_string(),
        gx_core::SubstrateKind::Mcp => "mcp".to_string(),
        gx_core::SubstrateKind::Custom(name) => format!("custom:{name}"),
    }
}

/// 🔴 How **every** engine in this binary is opened — one spelling, two callers.
///
/// 🔴 **M7 hand 4** widened what 「the same decision」 means here: the policy set is now every shipped
/// pack ([`default_policies`]) and the registry is every adapter this binary can build
/// ([`register_default_adapters`]). Both are functions rather than lines in this body so that the
/// two halves of req/38 §60's pair can be measured separately from the engine they end up in.
///
/// [`Session`] is the single-shot road (44 §1's twelve verbs) and `gx serve` is the long-lived one,
/// and the difference between them is one type parameter: a CLI process injects the evidence its
/// `--evidence` flags carried (`InjectedEvidence`), a server injects the evidence the current
/// request carried (`gx_api::RequestEvidence`). Everything else — which journal path, which policy
/// pack, which adapter, which two DR-2 axes — is the same decision, and a second spelling of it
/// would be a second answer to 「how is a project's engine opened」 waiting to drift.
///
/// `posture` is here and not on [`Session::open`] because 44 §1.2 gives `--fail-posture` to
/// `gx serve` alone; a single-shot verb passes `FailClosed`, which is DR-2's default for every
/// substrate.
///
/// # Errors
/// [`Error::Io`] if the journal's directory cannot be made, [`Error::Engine`] if the journal, blob
/// store or ledger will not open, [`Error::Gate`] if the pack will not parse.
pub fn open_engine<E: gx_engine::EvidenceSource>(
    layout: &Layout,
    evidence: E,
    mode: Option<EnforcementMode>,
    posture: gx_core::FailPosture,
    policy: Option<&Path>,
) -> Result<Engine<E>> {
    let policies = match policy {
        Some(path) => crate::policy::load(path)?,
        None => default_policies()?,
    };
    let gate = Gate::with_policies(policies);
    let journal = layout.journal_path();
    if let Some(parent) = journal.parent() {
        std::fs::create_dir_all(parent).map_err(crate::io("create", parent))?;
    }
    let mut engine = Engine::open(&journal, gate, evidence)?;
    // 🔴 The pair, in two lines: the set above and the registry here (req/38 §60's R-9 対裁定). The
    // engine still declares no adapter — N-13 is about `gx-engine`'s manifest and is unmoved; this
    // is the caller putting adapters in its registry, which req/88 §1 ruled is the CLI's job.
    let _registered = register_default_adapters(&mut engine);
    if let Some(mode) = mode {
        engine = engine.with_mode(mode);
    }
    Ok(engine.with_posture(posture))
}

/// An engine bound to one project's `.gx/`, with the fs adapter registered.
pub struct Session {
    engine: Engine<InjectedEvidence>,
    layout: Layout,
}

impl Session {
    /// Open the project's `.gx/` and build an engine over it.
    ///
    /// `create` is `true` for `gx submit` alone. Every other verb **opens**, so that a mistyped
    /// directory answers 44 §1.4's 6 rather than quietly starting a second, empty ledger beside the
    /// operator's real one — 「読めない」 and 「無い」 again (E-M4-35), one directory up.
    ///
    /// # 則 1 (ii) in the signature
    ///
    /// The [`Gate`] is built from gx-gate's own embedded pack (`packs::fs_pack`) and handed to the
    /// engine. This crate does not evaluate it, does not read its decision and never names a
    /// `Verdict` — 41 §4 keeps judging in one function, and what the CLI does is choose which policy
    /// set the engine judges *with*. FR-028's 「pack の file が build に埋め込まれる経路が 1 本」 is
    /// why the bytes come from gx-gate rather than from a file this binary reads.
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/` cannot be created or read, [`Error::Layout`] for a newer directory,
    /// [`Error::Engine`] if the journal, blob store or ledger will not open, and
    /// [`Error::Gate`] if the shipped policy pack will not parse.
    pub fn open(
        project: &Path,
        create: bool,
        evidence: Vec<gx_witness::Evidence>,
        mode: Option<EnforcementMode>,
    ) -> Result<Self> {
        Self::open_with_policy(project, create, evidence, mode, None)
    }

    /// The same, deciding with a pack read off disk instead of the shipped one (**E-M6-12**).
    ///
    /// 44 §1.2 gives `gx verify` no `--policy`, and req/38 §50 M6H3-9 採(a) adds one: 「書込可能 path
    /// を deny する **test 用 policy pack fixture**+`gx verify --policy <PATH>`(44 拡張)」. The reason
    /// is DR-2's other half. The shipped pack's one forbid is `/etc`, so the only route a `gx`
    /// invocation had to a `Verdict::Deny` ran through a real path under `/etc` — and a **record-only**
    /// commit of a denied change proceeds to `apply`, which on that locator is a write to `/etc`.
    /// Hand 3 measured T-8r inside the engine for that reason and raised the gap; this is the flag
    /// that closes it, and what it buys is that DR-2's 「適用は通すが `enforced=false`」 is now
    /// measurable **through the binary** on a file in a temporary directory.
    ///
    /// A separate function rather than a fifth parameter on [`Session::open`]: six call sites pass
    /// nothing, and M5H5-1 refused a five-argument `Engine::open` on the grounds that a parameter
    /// every caller ignores is a parameter every caller has to read.
    ///
    /// # Errors
    /// As [`Session::open`], plus [`crate::Error::Io`] / [`crate::Error::Gate`] if the named pack
    /// cannot be read or does not parse.
    pub fn open_with_policy(
        project: &Path,
        create: bool,
        evidence: Vec<gx_witness::Evidence>,
        mode: Option<EnforcementMode>,
        policy: Option<&Path>,
    ) -> Result<Self> {
        let layout = if create {
            Layout::create(project)?
        } else {
            Layout::open(project)?
        };
        let engine = open_engine(
            &layout,
            InjectedEvidence::new(evidence),
            mode,
            gx_core::FailPosture::FailClosed,
            policy,
        )?;
        Ok(Self { engine, layout })
    }

    /// The engine, for a caller that is about to drive one of 43's transitions.
    pub fn engine(&mut self) -> &mut Engine<InjectedEvidence> {
        &mut self.engine
    }

    /// The engine, read-only.
    #[must_use]
    pub fn read(&self) -> &Engine<InjectedEvidence> {
        &self.engine
    }

    /// The `.gx/` this session is bound to.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The draft store beside it.
    #[must_use]
    pub fn drafts(&self) -> DraftStore {
        DraftStore::in_layout(&self.layout)
    }

    /// Remember that an intent resolved to a transformation (**M6-02 採(b)**'s cache).
    ///
    /// Best effort by construction: req/56 §2 declares `.gx/index/` 「derived・消して良いと宣言」, so
    /// a cache that could not be written is not a reason to fail a command that succeeded. The
    /// engine's own `resolved` map is the authority and is rebuilt from the journal.
    pub fn remember(&self, intent: IntentId, transformation: TransformationId) {
        let (mut index, _) = ResolutionIndex::load(&self.layout);
        index.learn(intent, transformation);
        let _ = index.store(&self.layout);
    }

    /// The `IntentId` a transformation was planned from, without opening a body.
    ///
    /// Two roads, in this order:
    ///
    /// 1. `.gx/index/`, the cache hand 1 built (**M6-02 採(b)**);
    /// 2. the **file names** of `.gx/drafts/`, each of which *is* an `IntentId` in 42 §1.2's text
    ///    form, checked against `Engine::resolved` (**M6-02 採(a)**).
    ///
    /// The second road is what makes the first disposable, which req/56 §2 promises it is. Note
    /// what neither road does: **compute** anything. A name is parsed (`Cid::from_text`, in gx-core)
    /// and the engine is asked; 則 1 (i) survives a lookup because a lookup is not a mint.
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/drafts/` cannot be listed.
    pub fn intent_of(&self, transformation: &TransformationId) -> Result<Option<IntentId>> {
        let (index, _) = ResolutionIndex::load(&self.layout);
        let dir = self.layout.join("drafts");
        let mut candidates: Vec<IntentId> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(std::result::Result::ok) {
                let path = entry.path();
                if !path.extension().is_some_and(|x| x == "json") {
                    continue;
                }
                let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                    continue;
                };
                if let Ok(cid) = gx_core::Cid::from_text(&stem.replace('_', ":")) {
                    candidates.push(IntentId(cid));
                }
            }
        }
        // The cache first, and only when it agrees with the engine. A cache consulted *instead of*
        // the authority is the shape hand 1's `index.rs` refused to provide a helper for.
        for id in candidates
            .iter()
            .copied()
            .filter(|id| index.get(id).as_ref() == Some(transformation))
            .chain(candidates.iter().copied())
        {
            if self.engine.resolved(&id).as_ref() == Some(transformation) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Read one draft back.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::Malformed`] from the store.
    pub fn draft(&self, intent: &IntentId) -> Result<Option<Intent>> {
        self.drafts().get(intent)
    }

    /// 🔴 What the **journal** says about a transformation, without holding a row.
    ///
    /// `Engine::open` leaves the in-flight table empty (M5H3-5) and the journal holds every
    /// transition, so this is Σ's answer to 「where is this transformation」 in a process that has
    /// planned nothing. It is `reconstruct` — **E-M5-2**'s read-only Σ rebuild — and it opens no
    /// substrate.
    ///
    /// # 🔴 Why every verb asks this **before** [`Session::resume`]
    ///
    /// A resume re-plans, and a re-plan reads the world. After a commit the world **has moved** —
    /// by that commit — so a transformation that reached `Committed` can never be re-planned into
    /// itself again. Resuming first would therefore turn 44 §1.2's 「同一実行の再試行は自然に冪等」
    /// into a refusal, and the refusal would be about the substrate rather than about the request.
    ///
    /// A terminal row does not need a body: 43 T-9's idempotency column answers a re-entrant commit
    /// with the state it already reached, and the receipt for it is in `.gx/receipts/`. So the rule
    /// is: **ask the journal first, and only rebuild a row when there is still work to do on it.**
    pub fn recorded(&self, id: &TransformationId) -> Option<StateRow> {
        reconstruct(self.engine.journal().records())
            .state_of(id)
            .cloned()
    }

    /// 🔴 Put a `Candidate` (or anything past it) back in the engine's table.
    ///
    /// The sequence 44 §0 describes, with the authority in the third step: find the intent, read the
    /// body, ask the engine to plan it again. What comes back is the engine's answer about identity,
    /// and this function's only judgement is to **refuse a different one**.
    ///
    /// # Errors
    /// [`Error::NotFound`] if nothing here has ever planned that transformation, or if the draft it
    /// came from is gone — which is the one durable consequence of `.gx/drafts/` being
    /// `Nature::Source` (req/56 §2: 「失われる」). [`Error::Usage`] if the world has moved and the
    /// plan now names a different transformation. Anything the engine refuses, unchanged.
    pub fn resume(&mut self, id: &TransformationId, at: Timestamp) -> Result<()> {
        if self.engine.transformation(id).is_some() {
            return Ok(());
        }
        let intent_id = self.intent_of(id)?.ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: id.0.to_text(),
        })?;
        let intent = self.draft(&intent_id)?.ok_or_else(|| Error::NotFound {
            what: "draft for transformation",
            id: id.0.to_text(),
        })?;
        let planned = self.engine.plan(&intent, at)?;
        if planned == *id {
            return Ok(());
        }
        Err(Error::Usage {
            detail: format!(
                "{} was planned against a state of the substrate that no longer holds; planning it \
                 now names {} instead. 43 §8: 「`Fingerprint₀`は陳腐化しているため再`plan()`（再\
                 fingerprint）を強制する」 — run `gx plan` again and verify the transformation that \
                 comes back",
                id.0.to_text(),
                planned.0.to_text()
            ),
        })
    }

    /// 🔴 Put a **`Committed`** row back — the state [`Session::resume`] cannot reach.
    ///
    /// `resume` re-plans, a re-plan reads the substrate, and a commit has moved it, so a committed
    /// transformation can never be planned into itself again (the module header says as much, and
    /// [`crate::pipeline::commit`] relies on it). `gx undo <TID>` is nonetheless an operation on a
    /// committed transformation (43 §5), so this is the road in:
    /// [`gx_engine::Engine::rehydrate_committed`] rebuilds the row from Σ, the blob store and the
    /// `Intent` this project is still holding in `.gx/drafts/` — and re-identifies the result, so
    /// a draft that is not the one the transformation was planned from is refused rather than
    /// undone.
    ///
    /// 🔴 **The draft is therefore load-bearing after the commit.** req/56 §2 files `.gx/drafts/`
    /// as `Nature::Source` with 「失われる」 in its recovery column, and this is the second thing
    /// that is lost with it: hand 1 wrote that a missing draft costs a `gx plan`, and it costs a
    /// `gx undo` as well. Raised as **M6H4-5**.
    ///
    /// # Errors
    /// [`Error::NotFound`] if Σ has never heard of the transformation, if it is not committed, or
    /// if the draft it came from is gone. Anything the engine refuses, unchanged.
    pub fn rehydrate_committed(&mut self, id: &TransformationId) -> Result<()> {
        if self.engine.transformation(id).is_some() {
            return Ok(());
        }
        let intent_id = self.intent_of(id)?.ok_or_else(|| Error::NotFound {
            what: "transformation",
            id: id.0.to_text(),
        })?;
        let intent = self.draft(&intent_id)?.ok_or_else(|| Error::NotFound {
            what: "draft for transformation",
            id: id.0.to_text(),
        })?;
        if self.engine.rehydrate_committed(id, &intent)? {
            return Ok(());
        }
        Err(Error::NotFound {
            what: "committed transformation",
            id: id.0.to_text(),
        })
    }

    /// 🔴 The key a receipt is signed with: the one the transformation's actor names.
    ///
    /// See the module documentation for the separation this does not implement (**M6H3-4**). The
    /// refusal when the key is missing names the command that makes one, because an operator who
    /// reaches `gx verify` without a key has done nothing wrong — 44 §1.2 never told them to.
    ///
    /// # Errors
    /// [`Error::NotFound`] if the transformation is not in the table, [`Error::Witness`] if the key
    /// file is missing, unreadable, or has permissions `KeyPair::load` refuses (req/56 §3's 0600).
    pub fn signing_key(&self, id: &TransformationId) -> Result<KeyPair> {
        let actor = self
            .engine
            .transformation(id)
            .map(|t| t.actor.key().clone())
            .ok_or_else(|| Error::NotFound {
                what: "transformation",
                id: id.0.to_text(),
            })?;
        let store = KeyStore::user_default()?;
        store.load(&actor).map_err(|e| match e {
            Error::Witness(_) => Error::Usage {
                detail: format!(
                    "this transformation names actor key {actor:?} and req/56 §3's key store has no \
                     usable key under that id ({e}); `gx key gen` makes one"
                ),
            },
            other => other,
        })
    }
}
