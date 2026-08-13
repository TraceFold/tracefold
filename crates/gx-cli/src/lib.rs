#![forbid(unsafe_code)]
//! The `gx` command line, as a library. 44 §1's surface and req/56's directory.
//!
//! # 🔴 則 1 — this crate holds no semantic authority
//!
//! req/88 §3 Λ1 is M6's central claim and it is a claim about **absence**:
//!
//! > M6 は `Σ` を拡張しない。gx-cli / gx-api は `Σ = (L, J, E, Λ)` の**観測**と、engine の 8 入口
//! > への**写像**しか持たない。∴ M6 が意味論を足したらそれは実装欠陥であって設計選択ではない
//!
//! Three things follow, and all three are mechanically checked by
//! `crates/gx-canon/tests/authority_boundary.rs` rather than promised here:
//!
//! * **no canonical encoding.** 41 §6: 「全canonical encodeはgx-canon経由のみ（迂回禁止）」. This crate
//!   parses `gx1:` text with [`gx_core::Cid::from_text`] and never computes one. Parsing a name is
//!   not minting a name, and the parser living in gx-core rather than gx-canon is what makes the
//!   rule satisfiable instead of something the CLI has to break to do its job.
//! * **no `Verdict`.** 41 §4 puts the one judgement in `Gate::verify`.
//! * **no `Lifecycle` write.** 42 §1.3-3: 「状態は`TransformationId`をキーとしたengine側の外部
//!   テーブルで管理される」. This crate reads states; it keeps none.
//!
//! # 🔴 The one asymmetry, stated rather than hidden (req/88 §3 Λ2)
//!
//! Λ2 says N single-shot CLI runs and one long-lived engine are observationally equal on Σ, and
//! names the single place where that breaks: 「`Σ` に入らない state を CLI が持った瞬間に等価が壊れる
//! ——M6-01(a) の `.gx/drafts/` がまさにそれである」. The draft body has to live somewhere between
//! `gx submit` and `gx plan`, those are two processes, and the journal records an `IntentId` rather
//! than a body (E-M5-3 + ASM-9). So [`draft`] is CLI-side state that the HTTP surface does not have.
//!
//! 44 §0 already permits the asymmetry — 「HTTP `POST /candidates`は submit+plan を一括atomically
//! 実行するためDraft単独状態を公開せず、この規則の対象外となる」 — so this is not a breach of the
//! specification. What it does mean is that **AC-055's 「同一」 has to be read as 「同一 from
//! `Candidate` onward」**, and that reading is written here because a reading nobody wrote down is a
//! reading the next hand invents differently.
//!
//! # 🔴 What this crate reads back, and what it cannot (req/88 §5 行 13, M4H3-7)
//!
//! M4H3-7 sent 「3 つの View 公開型の `Deserialize` を M6 wire 手が検討」 to this milestone, and the
//! judgement is **no**, for a reason that is structural rather than a preference: a `View` is a
//! **borrow** type. `gx_canon::TransformationView<'a>` holds `&'a DeltaRef`, `&'a ChangeContext`,
//! `&'a Actor`, `&'a [TransformationId]`; `PayloadView<'a>` is a newtype over `&'a [u8]`.
//! `Deserialize` produces an owned value out of bytes it does not keep, so a borrowing struct can
//! only implement it by borrowing from the input buffer — which would make the decoded value's
//! lifetime the buffer's, and would put a second, weaker meaning on a type whose whole purpose is to
//! be the *projection of a value that exists* (42 §1.3's identity views).
//!
//! Measured rather than asserted: **nine** `…View` types are public in this workspace — four in
//! gx-canon, three in gx-gate, two in gx-substrate — and **eight of the nine carry a `<'a>`**. The
//! ninth, `gx_gate::RequestView`, is owned; it is a Cedar request builder rather than an identity
//! projection, and nothing on a wire refers to it.
//!
//! So this crate reads back the **owned** types instead, and every one it needs already has
//! `Deserialize`: `Receipt`, `ReceiptPayload`, `Checkpoint`, `InclusionProof`, `ConsistencyProof`.
//! M6 adds no `Deserialize` to anything (M5's hands got there first), and M4H3-7 closes with a
//! judgement rather than with an implementation.
//!
//! # What this hand builds and what it does not
//!
//! req/88 §6.2 手 1: 「subcommand は 1 本も実装しない」. There are none. What is here is [`layout`]
//! (req/56's directory, seven paths), [`draft`] (M6-01 採(a)), [`index`] (M6-02 採(b)) and
//! [`consumers`] (the two accessor decisions M5 left for M6 to settle).

pub mod clock;
pub mod consumers;
pub mod draft;
pub mod exit;
pub mod index;
pub mod keys;
pub mod layout;
pub mod ledger;
pub mod lifecycle;
pub mod pipeline;
pub mod policy;
pub mod receipt;
pub mod replay;
pub mod rng;
pub mod serve;
pub mod session;

/// 42 §3.1's four substrates, from the `--substrate` spelling.
///
/// In the library rather than in `main.rs` because there are now **two** readers of the spelling —
/// the `gx submit` flag and a `gx policy test` scenario — and 44 §1.2 gives them one vocabulary. A
/// second copy in the binary would be a second answer to 「what does `fs` mean」 the day one of them
/// grew a case.
///
/// # Errors
/// [`Error::Usage`] for a name that is neither one of 44 §1.2's three nor `custom:<NAME>`.
pub fn substrate_kind(text: &str) -> Result<gx_core::SubstrateKind> {
    match text {
        "fs" => Ok(gx_core::SubstrateKind::Fs),
        "git" => Ok(gx_core::SubstrateKind::Git),
        "mcp" => Ok(gx_core::SubstrateKind::Mcp),
        // 42 §3.1 keeps `Custom` a `String` rather than a registry, and 44 §1.2's synopsis lists
        // only the three that have adapters. A custom kind is accepted here and refused later by
        // `plan` with 「no adapter for this substrate」, which is the true refusal: the kind is legal
        // and nothing is registered for it.
        other => other
            .strip_prefix("custom:")
            .map(|name| gx_core::SubstrateKind::Custom(name.to_string()))
            .ok_or_else(|| Error::Usage {
                detail: format!(
                    "--substrate takes fs|git|mcp (44 §1.2) or `custom:<NAME>` (42 §3.1); got \
                     {other:?}"
                ),
            }),
    }
}

/// 42 §3.2's `ChangeContext`, from the `--context` spelling. Shared for [`substrate_kind`]'s reason.
///
/// # Errors
/// [`Error::Usage`] for a name that is neither one of 44 §1.2's six nor `Custom:NAME`.
pub fn change_context(text: &str) -> Result<gx_core::ChangeContext> {
    use gx_core::ChangeContext as C;
    match text {
        "Time" => Ok(C::Time),
        "Evidence" => Ok(C::Evidence),
        "Policy" => Ok(C::Policy),
        "Model" => Ok(C::Model),
        "Representation" => Ok(C::Representation),
        "Substrate" => Ok(C::Substrate),
        other => other
            .strip_prefix("Custom:")
            .map(|name| C::Custom(name.to_string()))
            .ok_or_else(|| Error::Usage {
                detail: format!(
                    "--context takes one of 44 §1.2's six names or `Custom:NAME`; got {other:?}"
                ),
            }),
    }
}

/// What this crate refuses with.
///
/// 41 §6 asks for `thiserror`. The variants are the CLI layer's own — a directory that will not
/// open, a draft that will not parse — and they are deliberately **not** a mirror of the engine's
/// fourteen: 44 §1.4's exit codes and §2.3's `gx_code` are the mapping surface, req/88 M6-09 is the
/// ticket that designs it, and hand 5 is the hand that owns it. A hand-1 enum that guessed at the
/// mapping would be a table two hands would then have to disagree with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The `.gx/` directory (or something inside it) could not be read or written.
    #[error("{action} {path}: {source}")]
    Io {
        /// What was being attempted, for a message that says where it broke.
        action: &'static str,
        /// The path it broke on.
        path: String,
        /// The underlying refusal.
        source: std::io::Error,
    },
    /// A file inside `.gx/` exists and does not hold what its name says it holds.
    ///
    /// Kept separate from [`Error::Io`] on purpose: 「読めない」 and 「無い」 are different answers
    /// (E-M4-35), and so are 「無い」 and 「在るが壊れている」. req/56 §5's recovery rules branch on
    /// exactly that difference.
    #[error("{path} is not a readable {what}: {detail}")]
    Malformed {
        /// The kind of file it was supposed to be.
        what: &'static str,
        /// Where it is.
        path: String,
        /// What went wrong reading it.
        detail: String,
    },
    /// The directory was written by a layout version this binary does not understand.
    ///
    /// Fail-closed, which is the whole reason `.gx/VERSION` exists (req/56 §2, 「layout version
    /// (migration 用)」). 47 §4 makes journal-schema compatibility an upgrade precondition; a binary
    /// that opened a newer directory 「best effort」 would be the failure that condition is about.
    #[error("{path} declares layout version {found}; this binary writes {expected}")]
    Layout {
        /// Where the version file is.
        path: String,
        /// What it says.
        found: String,
        /// What this binary understands.
        expected: u32,
    },
    /// The arguments do not describe an operation this binary can attempt.
    ///
    /// 44 §1.4's 1 「入力不正」, and **not** its 2: 規律52 (req/38 §48 M6H1-1) reserves 2 for the state
    /// machine's 「拒否」. Everything clap refuses lands here too, through
    /// [`crate::exit::ERROR`].
    #[error("{detail}")]
    Usage {
        /// What was wrong with the request.
        detail: String,
    },
    /// The named thing is not there. 44 §1.4's 6.
    #[error("no {what} for {id}")]
    NotFound {
        /// The kind of thing that was looked for.
        what: &'static str,
        /// The name it was looked for under.
        id: String,
    },
    /// gx-witness refused: a bad signature, a receipt that is not a legal one, a key file.
    #[error(transparent)]
    Witness(#[from] gx_witness::Error),
    /// gx-log refused: a malformed proof shape, a range outside the tree, a torn ledger.
    #[error(transparent)]
    Log(#[from] gx_log::Error),
    /// gx-engine refused: the journal would not open or replay.
    #[error(transparent)]
    Engine(#[from] gx_engine::Error),
    /// gx-gate refused: the shipped policy pack would not parse.
    ///
    /// 🔴 Reached only at [`crate::session::Session::open`], where FR-028's embedded pack is turned
    /// into a `PolicyEngine`. A failure here is a **broken shipped artefact** rather than anything
    /// the operator did — `packs::fs_pack` parses bytes `include_str!` put in the binary — which is
    /// why it is its own variant and not folded into [`Error::Malformed`]: 「the file you gave me is
    /// wrong」 and 「this build is wrong」 are different sentences and E-M3-3 is the standing rule
    /// against giving them one face.
    #[error(transparent)]
    Gate(#[from] gx_gate::Error),
}

impl Error {
    /// 44 §1.4's status for this refusal.
    ///
    /// # 🔴 The mapping is deliberately coarse, and hand 5 owns the fine one
    ///
    /// req/88 §3 Λ4: exit codes and `gx_code` are a **quotient** of the refusal space and 「情報は
    /// 必ず落ちる」, so the discipline is not 「畳むな」 but 「畳んだ事を書け」. What this hand folds:
    /// every refusal from gx-witness, gx-log and gx-engine that is not a missing object arrives at
    /// 1, because 44 §1.2 gives the read side no other code for them — `gx log` has exactly `0` and
    /// `1`. The refusals that deserve a code of their own are listed in the report and are M6-09's
    /// material, which hand 5 turns into the `gx_code` table.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::NotFound { .. } => exit::NOT_FOUND,
            // 🔴 A file that is not there is 44 §1.4's 6 and not its 1. The case that made this
            // explicit is 「this directory has no `.gx/`」: `Layout::open` fails with an `ErrorKind::
            // NotFound` on `VERSION`, and reporting that as an internal error told an operator that
            // gx had broken when what had happened was that they were in the wrong directory.
            // 「読めない」 and 「無い」 are different answers (E-M4-35) and this is the second of them.
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                exit::NOT_FOUND
            }
            _ => exit::ERROR,
        }
    }

    /// 44 §1.3's stderr object: 「`{"type":..., "title":..., "gx_code":..., "detail":...}`（§2.5の
    /// problem+json語彙を流用、HTTP的な`status`フィールドはCLIでは省略可）」.
    ///
    /// `gx_code` is drawn from 44 §2.3's twelve and from nowhere else —
    /// `crates/gx-cli/tests/exit_map.rs` parses that table out of the specification and refuses a
    /// code this file invented. Two of the twelve are all this hand can honestly reach:
    /// `VALIDATION_ERROR` and `NOT_FOUND`. Everything else the read side can fail with is an
    /// internal or adapter refusal, and 44 §2.3 has `INTERNAL` 「分類不能な内部エラー」 for exactly
    /// that — used as the honest 「unclassified」 rather than as a bucket, with the list of what
    /// lands in it written in the report.
    #[must_use]
    pub fn problem(&self) -> serde_json::Value {
        let (gx_code, title) = match self {
            Error::Usage { .. } | Error::Malformed { .. } | Error::Layout { .. } => (
                "VALIDATION_ERROR",
                "the request is not one this binary can attempt",
            ),
            Error::NotFound { .. } => ("NOT_FOUND", "the named object is not here"),
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                ("NOT_FOUND", "the named object is not here")
            }
            Error::Io { .. }
            | Error::Witness(_)
            | Error::Log(_)
            | Error::Engine(_)
            | Error::Gate(_) => ("INTERNAL", "the operation could not be completed"),
        };
        serde_json::json!({
            "type": format!("https://glovrex.dev/errors/{}", gx_code.to_lowercase().replace('_', "-")),
            "title": title,
            "gx_code": gx_code,
            "detail": self.to_string(),
        })
    }
}

/// The CLI's own result type.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(action: &'static str, path: &std::path::Path) -> impl Fn(std::io::Error) -> Error {
    let path = path.display().to_string();
    move |source| Error::Io {
        action,
        path: path.clone(),
        source,
    }
}
