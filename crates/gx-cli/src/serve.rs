// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `gx serve` (44 §1.1/§1.2) — the verb, its refusals, and the four things a server needs.
//!
//! # Which crate holds the binary, and why the answer is "both, in this order" (sem: SEM-gx-cli-178)
//!
//! The hand-6 brief asks for the judgement in the report: "which crate the `serve` executable
//! binary belongs to is judged by reading directly from 44 §1.1 (`gx serve` is a CLI verb) and 41
//! §2's crate table" (sem: SEM-gx-cli-179). Read directly:
//!
//! * **44 §1.1** lists thirteen commands and the thirteenth row is "`gx serve` | starts gx-api" (sem: SEM-gx-cli-180) — so
//!   `serve` is a **verb of `gx`**, and `gx` is this crate's `[[bin]]`.
//! * **41 §2** describes `gx-api/` as "axum HTTP+JSONL stream (per 44)" (sem: SEM-gx-cli-181) and puts the literal word
//!   `serve` at the end of **gx-cli**'s own line — so the runtime is gx-api's and the entry is here.
//! * **47 §1(a)** settles the direction: "single static binary (`gx-cli` embeds `gx-api`'s
//!   functionality via `gx serve`)" (sem: SEM-gx-cli-182).
//!
//! ∴ the executable is `gx`, built from gx-cli; the router, the runtime and the graceful shutdown
//! are `gx_api::serve`, and **gx-cli declares no `tokio`** (the brief: "gx-cli does not declare
//! tokio" (sem: SEM-gx-cli-183)). `gx_api::serve` blocks and builds its own runtime for exactly that reason. The brief's
//! stop condition — "if the design turns out to need `serve` placed in gx-cli, stop and file a
//! ticket" — did not fire: no
//! part of the runtime needs to be spelled on this side.
//!
//! # What this module does that gx-api structurally cannot
//!
//! gx-api may not name `.gx/` (the cycle: 47 §1(a) makes gx-cli contain gx-api), so the four things a
//! server needs are assembled here:
//!
//! 1. **the engine** over `.gx/ledger/journal`, with the fs adapter registered and DR-2's two axes
//!    set from the flags 44 §1.2 gives this verb;
//! 2. **the bearer token**, read from a **file** — `--token-file`. Hand 5 closed two of the three
//!    roads (environment, `.gx/config.toml`) and left the third with a note: "the shape hand 6
//!    should take when writing `gx serve`'s flag is: **the token file's path** as the argument" (sem: SEM-gx-cli-184). A path in `ps` is not a
//!    secret; the token itself would be;
//! 3. **the two keys of 45 §1** — the server's own signing key and the adjudicators' — out of req/56
//!    §3's `~/.gx/keys/`, which is a directory gx-api has never heard of;
//! 4. **`.gx/config.toml`'s recorded public keyid** (**E-M6-7**). Hand 5 built the check and named
//!    this hand as the reader: "config.toml's **reader** is hand 6, the hand that holds the flag" (sem: SEM-gx-cli-185).
//!
//! # 🔴 Three refusals, and none of them is a silent success
//!
//! * `--bind` outside loopback. v0.1 has **no authorization layer** (M5H6-4 adopted (a); sem: SEM-gx-cli-186), so a socket on a
//!   public interface is a socket anyone can `cancel` through. §47 registered the firing condition
//!   for the authorization work — M5H6-4(b), "when HTTP is bound outside loopback" (sem: SEM-gx-cli-187) — and until
//!   that fires the honest answer is to refuse rather than to offer an override flag that turns a
//!   ruling into a checkbox. Raised as **M6H6-6** so that the reviewer can rule the override in.
//! * `--tls-cert` / `--tls-key`. 44 §1.2's synopsis has them and req/88 §1 N-09 keeps TLS out of
//!   v0.1 (44 §2.5: "v0.2 (announced): mTLS" (sem: SEM-gx-cli-188)). E-M6-8 fixed the shape for a flag with nowhere to go:
//!   refuse, and say what the synopsis promised. An accepted `--tls-cert` would leave an operator
//!   believing the socket is encrypted, which is worse than the flag not existing.
//! * no token. 44 §2.5 is "mandatory" (sem: SEM-gx-cli-189) and hand 5 made a tokenless server answer `INTERNAL` rather than
//!   `401` to every request; refusing to start says the same thing earlier and to the person who can
//!   fix it.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gx_api::auth::Bearer;
use gx_api::state::{AppState, RequestEvidence, ServerKeys};
use gx_api::{ReceiptSlot, ServeConfig};
use gx_core::{EnforcementMode, FailPosture};
use gx_engine::pipeline::RecoveryPath;
use gx_engine::store::ProcessLock;
use gx_witness::KeyPair;

use crate::keys::KeyStore;
use crate::layout::Layout;
use crate::receipt::{ReceiptStore, StoredKind};
use crate::session::LOCK_FILE;
use crate::{Error, Result};

/// What `gx serve`'s flags amount to, after parsing and before anything is opened.
pub struct ServeSpec {
    /// `--bind <ADDR:PORT>`; [`gx_api::serve::DEFAULT_BIND`] when absent.
    pub bind: Option<String>,
    /// `--record-only` — DR-2's `EnforcementMode` axis, for the process.
    pub record_only: bool,
    /// `--fail-posture <closed|open>` — DR-2's other axis (ASM-13).
    pub fail_posture: Option<String>,
    /// 44 §1.2's TLS pair, accepted so that it can be refused by name (N-09).
    pub tls: (Option<PathBuf>, Option<PathBuf>),
    /// The file holding 44 §2.5's bearer token. Not in 44 §1.2's synopsis — see the module header.
    pub token_file: Option<PathBuf>,
    /// The key id this server signs receipts with. Falls back to `.gx/config.toml` (**E-M6-7**).
    pub signing_key: Option<String>,
}

/// 45 §1's two keys, over req/56 §3's directory.
///
/// Every key in the store is loaded **at start-up**, which is a decision with a cost: a key added
/// while the server runs is a key it will not find, and an operator who adds an adjudicator has to
/// restart. The alternative — reading the directory per request — would put a filesystem walk inside
/// 43 T-5's path and would make the set of people who may rule on an escalation change without any
/// record that it did.
struct StoreKeys {
    signing: KeyPair,
    rulers: BTreeMap<String, KeyPair>,
}

impl ServerKeys for StoreKeys {
    fn signing(&self) -> &KeyPair {
        &self.signing
    }

    fn ruler(&self, key_id: &str) -> Option<&KeyPair> {
        // 🔴 No fallback to `signing`, and **E-M6-15**/INV-S6 is why: "there is no default in which the
        // one being judged approves themselves" (sem: SEM-gx-cli-190). A server that signed a human ruling with its own key would be
        // recording that the server allowed the change.
        self.rulers.get(key_id)
    }
}

/// 🔴 **R11 / `req/240` M-04** — `.gx/VERSION` and `.gx/config.toml` as gx-api's `ProjectMeta`.
///
/// `gx serve` asks [`Layout::require_config`] and `Layout::open` once, on its way up. `req/240`
/// M-04 measured what "once" means for a process that then runs for days: with the server up,
/// `rm .gx/VERSION` left `/healthz` answering `{"status":"ok"}` and `POST /v1/candidates` →
/// `verify` → `commit` all succeeding, adding a leaf to a project every CLI verb was refusing.
/// This handle is the same question, asked at each write and each health probe. Two `stat` calls,
/// on the roads that already take the engine's lock or rebuild the health snapshot.
struct LayoutMeta {
    layout: Layout,
}

impl gx_api::state::ProjectMeta for LayoutMeta {
    fn absent(&self) -> Option<gx_api::state::MetaAbsent> {
        let declaration = self.layout.join("VERSION");
        if !declaration.exists() {
            return Some(gx_api::state::MetaAbsent {
                declaration: true,
                path: declaration.display().to_string(),
            });
        }
        let config = self.layout.config_path();
        if !config.exists() {
            return Some(gx_api::state::MetaAbsent {
                declaration: false,
                path: config.display().to_string(),
            });
        }
        None
    }

    /// 🔴 **R11 / `req/240` M-01** — the journal and the ledger, `stat`ed and not read.
    ///
    /// Length and modification time of each, in one string. `GET /v1/healthz` reuses its snapshot
    /// only while this is unchanged, so every rewrite, truncation and append the `serve_runtime`
    /// suites perform under a running server forces the next probe to rebuild — which is what
    /// 43 §7.12 (c) meant by refusing to cache the answer of an endpoint whose job is to notice a
    /// disk that changed. The two files are the only ones the answer depends on
    /// (`Engine::catch_up_unlocked` folds the journal and re-opens a moved ledger).
    ///
    /// A file that cannot be `stat`ed contributes `-` rather than dropping out: "the journal is
    /// gone" and "the journal is 0 bytes" have to be different witnesses.
    fn witness(&self) -> Option<String> {
        fn mark(path: &std::path::Path) -> String {
            match std::fs::metadata(path) {
                Ok(meta) => {
                    let stamp = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or_else(|| "-".to_string(), |d| d.as_nanos().to_string());
                    format!("{}:{stamp}", meta.len())
                }
                Err(_) => "-".to_string(),
            }
        }
        Some(format!(
            "{}|{}",
            mark(&self.layout.journal_path()),
            mark(&self.layout.ledger_path())
        ))
    }
}

/// `.gx/receipts/` as gx-api's archive (M6H5-9's road back for a restarted server).
struct StoreArchive {
    store: ReceiptStore,
}

impl gx_api::ReceiptArchive for StoreArchive {
    fn store(
        &self,
        id: &gx_core::TransformationId,
        slot: ReceiptSlot,
        receipt: &gx_witness::Receipt,
    ) -> std::result::Result<(), String> {
        let kind = match slot {
            ReceiptSlot::Verdict => StoredKind::Verdict,
            ReceiptSlot::Ruling => StoredKind::Ruling,
            ReceiptSlot::Commit => StoredKind::Commit,
        };
        self.store
            .put(id, kind, receipt)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 🔴 **R3 / `req/222` H-02** — the commit slot, and nothing else.
    ///
    /// This was `first_available`, which walks `DISCLOSURE_ORDER` (commit, ruling, verdict). The
    /// caller is DR-43-1's CAS and a verdict receipt has no `postcondition_fingerprint`, so the
    /// fallback turned "there is no evidence" into "there is evidence that says nothing" and the
    /// gate was skipped. `gx undo`'s own pre-flight has always read `StoredKind::Commit` alone;
    /// this is the server's face agreeing with it.
    fn load_commit(&self, id: &gx_core::TransformationId) -> Option<gx_witness::Receipt> {
        self.store.get(id, StoredKind::Commit).ok().flatten()
    }
}

/// 🔴 `.gx/drafts/` as gx-api's draft archive (**T6 condition ① L2**, `req/38` §148 ruling
/// 1(iii), lane R2).
///
/// [`StoreArchive`]'s twin, and deliberately over the **same directory the CLI already uses**
/// rather than a server-only one. req/56 §2 files `.gx/drafts/` as the place an intent body lives
/// between two processes, and that is exactly what a server and a CLI sharing one project are —
/// `crates/gx-cli/tests/serve_runtime_e2e.rs` already measures the two committing side by side. A
/// second directory would mean a row `gx plan` can rebuild and `gx serve` cannot, on the same
/// journal.
///
/// Its cost is req/56 §2's own cell, unchanged: the directory is `Nature::Source`, nothing
/// regenerates a draft, and `gx draft discard` removing one now costs a rebuild on **both**
/// surfaces rather than one.
struct StoreDrafts {
    store: crate::draft::DraftStore,
}

impl gx_api::DraftArchive for StoreDrafts {
    fn store(
        &self,
        id: &gx_core::IntentId,
        intent: &gx_core::Intent,
    ) -> std::result::Result<(), String> {
        self.store
            .put(id, intent)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn load(&self, id: &gx_core::IntentId) -> Option<gx_core::Intent> {
        // 🔴 `Err` and `Ok(None)` are folded to `None` **here and not in the store** (E-M4-35 is
        // the rule the store keeps: "cannot be read" is not "does not exist"). What the trait can
        // carry is a body or nothing, and the caller's answer to nothing is R1's refusal, which
        // names the state and the absence. The unreadable case is therefore reported as "no body",
        // which is true, and loses the reason — declared in `req/221` §7 rather than hidden.
        self.store.get(id).ok().flatten()
    }
}

/// 🔴 **E-M6-7**'s reader — `.gx/config.toml`'s recorded public key id.
///
/// A four-line parser rather than a TOML crate. req/56 §2 gives this file one job on this path
/// ("only a reference to the public keyid" (sem: SEM-gx-cli-191)), the grammar of that job is `key = "value"`, and a dependency whose
/// generality is unused is a dependency whose failure modes are not. Raised as **M6H6-7** if a second
/// setting ever needs a type this cannot express.
///
/// # Errors
/// [`Error::Io`] if the file exists and cannot be read.
pub fn recorded_signing_keyid(layout: &Layout) -> Result<Option<String>> {
    let path = layout.join("config.toml");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(crate::io("read", &path))?;
    Ok(text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let (name, value) = line.split_once('=')?;
        if name.trim() != "engine_signing_keyid" {
            return None;
        }
        Some(value.trim().trim_matches('"').to_string())
    }))
}

/// 🔴 **FR-M7-4** — E-M6-7's **writer**, beside its reader.
///
/// req/95 §4 ③ (M6H7-8): "`.gx/config.toml`'s `engine_signing_keyid` has a reader but no
/// writer. Since a `scratch` image has no shell either, there is no road to record this value on a
/// fresh volume" (sem: SEM-gx-cli-192). The compose file
/// worked around it with `--signing-key ${GX_SIGNING_KEY_ID:?…}`, which is an operator copying a key
/// id by hand from one command's output into another's environment. `gx key gen --record` is the
/// road, and this is where it ends.
///
/// It lives here rather than in `keys.rs` because [`recorded_signing_keyid`] lives here: the file's
/// grammar is four lines of parser, and a second spelling of it — a writer that emitted what the
/// reader cannot read — is the failure a shared function makes impossible rather than unlikely.
///
/// # What it does to a file that already exists
///
/// Rewrites the `engine_signing_keyid` line and **leaves every other line alone**, including
/// comments. A rotation is the ordinary case (`gx key rotate --record`), and a writer that truncated
/// the file would take an operator's other settings with it the first time they rotated. A file that
/// does not exist yet is created with a comment naming what wrote it, because a mystery file in a
/// project directory is a thing people delete.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read or written.
pub fn record_signing_keyid(layout: &Layout, key_id: &str) -> Result<PathBuf> {
    let path = layout.join("config.toml");
    let line = format!("engine_signing_keyid = \"{key_id}\"");

    let text = if path.exists() {
        std::fs::read_to_string(&path).map_err(crate::io("read", &path))?
    } else {
        String::from(
            "# Written by `gx key gen --record` (FR-M7-4). req/56 §2: this file holds the public\n\
             # key id only -- \"only a reference to the public keyid\" (sem: SEM-gx-cli-193). Secrets live in ~/.gx/keys/ (req/56 §3).\n",
        )
    };

    let mut replaced = false;
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    for existing in text.lines() {
        let trimmed = existing.trim();
        let names_the_slot = !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .is_some_and(|(name, _)| name.trim() == "engine_signing_keyid");
        if names_the_slot && !replaced {
            out.push_str(&line);
            replaced = true;
        } else if names_the_slot {
            // A second assignment of the same key would make the reader's `find_map` answer with
            // the first one. Dropping the duplicate is the only outcome in which the file says one
            // thing; keeping it would leave a line that looks in force and is not.
            continue;
        } else {
            out.push_str(existing);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str(&line);
        out.push('\n');
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(crate::io("create", parent))?;
    }
    std::fs::write(&path, out).map_err(crate::io("write", &path))?;
    Ok(path)
}

/// Parse `--bind`, and refuse anything hand 5's policy refuses.
///
/// # Errors
/// [`Error::Usage`] for an unparseable address and for one [`gx_api::auth::bind_refusal`] rejects.
pub fn resolve_bind(bind: Option<&str>) -> Result<SocketAddr> {
    let text = bind.unwrap_or(gx_api::serve::DEFAULT_BIND);
    if let Some(reason) = gx_api::auth::bind_refusal(text) {
        return Err(Error::Usage {
            detail: format!(
                "{reason} v0.1 has no authorization layer (M5H6-4 adopted (a); sem: SEM-gx-cli-194), so a non-loopback bind is \
                 an unauthenticated surface on a shared network. §47 registered the authorization \
                 work against the firing condition \"when HTTP is bound outside loopback\" (sem: SEM-gx-cli-195) — this \
                 refusal is what makes that condition observable instead of theoretical"
            ),
        });
    }
    text.parse().map_err(|e| Error::Usage {
        detail: format!("`{text}` is not an <ADDR:PORT> (44 §1.2's `gx serve --bind`): {e}"),
    })
}

/// 🔴 **R34 / `req/449` H-02** — what a start-up's recovery did, at `gx repair`'s granularity.
///
/// # Why this is a type and not four `let mut`s inside [`build`]
///
/// `gx repair --json` has separated [`RecoveryPath::LedgerHeldTheCommit`] from
/// [`RecoveryPath::ApplyWasAnnounced`] since `req/329` M-01 / R27, and
/// `crates/gx-cli/tests/r27_reentrant_abort.rs:614` pins the separation with a sentence about why
/// it matters: *"an operator deciding whether to trust a recovered project cannot tell the two
/// apart"*. `gx serve`'s start-up line folded all four roads into `resumed` and printed nothing
/// per row, so the thirty-third audit measured a row that ended
/// `Aborted(ApplyFailed)` — with `Rollback::NotAttempted`, over a world the adapter had already
/// moved — being announced as one of the "resumed 1 transformation(s)". **The same repair was in
/// one of the two verbs.** Naming the fold makes it one thing that both a server and a test can
/// hold, which is what `req/38` §227's sibling-sweep rule asks for.
///
/// `resumed` is unchanged and is still the sum of the four sub-counts, so a monitor that read the
/// old field reads the same number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecoverFold {
    /// 43 §7-2: the last record was terminal and nothing was re-run.
    pub terminal: usize,
    /// The sum of the four roads below. Unchanged in meaning since R5.
    pub resumed: usize,
    /// 43 §7-3c with no `ApplyStarted`: the world did not move and the journal does not hold what
    /// re-running T-10a needs.
    pub nothing_applied: usize,
    /// 🔴 **R5 / `req/227` H-01** — 43 §7 declined to close these rows.
    pub refused: usize,
    /// 43 §7-3b, closed from the commit receipt the critical section had already filed.
    pub closed_from_receipt: usize,
    /// 43 §7-3b, closed from the ledger's leaf alone, with no receipt issued.
    pub closed_from_leaf: usize,
    /// 43 §7-3b, closed after **reading** the substrate and rebuilding the payload.
    pub ledger_held_the_commit: usize,
    /// 🔴 43 §7-3c with an `ApplyStarted` — **the one road of the four that writes**.
    pub apply_was_announced: usize,
    /// 🔴 **R34** — of [`RecoverFold::apply_was_announced`], how many ended in a terminal
    /// `Aborted`. Not a fifth road: a subset of the fourth, and the subset the audit measured
    /// being announced as a resume.
    pub announced_and_aborted: usize,
}

impl RecoverFold {
    /// Fold a recovery's rows. No `_` arm, for [`RecoveryPath::kind`]'s reason: a road added
    /// without a row here should stop the compiler rather than land silently in a total.
    #[must_use]
    pub fn of(rows: &[gx_engine::pipeline::Recovered]) -> Self {
        let mut fold = Self::default();
        for row in rows {
            match row.path {
                RecoveryPath::Terminal => fold.terminal += 1,
                RecoveryPath::NothingWasApplied => fold.nothing_applied += 1,
                // 🔴 **R13 / `req/244` H-03** — 43 §7-3b closed from the receipt the critical
                // section had already filed. A resume, and one that asked no adapter anything.
                RecoveryPath::ClosedFromFiledReceipt => {
                    fold.closed_from_receipt += 1;
                    fold.resumed += 1;
                }
                RecoveryPath::ClosedFromLedgerLeaf => {
                    fold.closed_from_leaf += 1;
                    fold.resumed += 1;
                }
                RecoveryPath::LedgerHeldTheCommit => {
                    fold.ledger_held_the_commit += 1;
                    fold.resumed += 1;
                }
                RecoveryPath::ApplyWasAnnounced => {
                    fold.apply_was_announced += 1;
                    fold.resumed += 1;
                    // 🔴 The row answers this, not the CLI: `authority_boundary.rs` Rule 1 (iii)
                    // keeps the state table on the engine side (42 §1.3-3), and it fired on the
                    // first draft of this line.
                    if row.ended_aborted() {
                        fold.announced_and_aborted += 1;
                    }
                }
                RecoveryPath::NotResumed => fold.refused += 1,
            }
        }
        fold
    }
}

/// 🔴 **R34 / `req/449` H-01 + H-02** — the sentence 43 §7-3c's road owes an operator, or `None`
/// for every other road.
///
/// # The two findings this answers, and the one thing it does not claim
///
/// **H-01.** A row the ledger holds no leaf for is finished by applying the delta, and the delta
/// is written over whatever the substrate holds **at that moment**. The audit reconstructed a
/// crash in that window, let a third party write to the file afterwards, and watched
/// `gx serve` replace their bytes, answer `refused: 0`, answer `/healthz` `200 ok`, and say
/// nothing at all. The world moving is not the finding — the silence is.
///
/// **H-02.** When the adapter refuses the re-apply, the row ends `Aborted(ApplyFailed)` with
/// `Rollback::NotAttempted` (`pipeline.rs`'s `RecoveredWithoutRebuilding`), which means the
/// compensation did not run. `req/372` M-01 built the fixture for the commonest real shape of that
/// failure — *the call worked and the answer was lost coming back* — so "the adapter refused" and
/// "the world did not move" are **not** the same statement, and this sentence does not pretend
/// they are.
///
/// **What is not claimed:** that gx knows whether anybody else wrote. It does not, and R34 did not
/// give it a way to find out. The seat for the comparison does not exist — `ApplyStarted` is
/// `{transformation, delta_cid, at}` and carries no fingerprint, the journal's only fingerprint is
/// `Planned.fp0`, and the value that would separate *our own applied footprint* from *somebody
/// else's write* is a post-apply fingerprint that this vocabulary has no record for (the engine's
/// own doc raises it as **M5H5-3**). Adding one is a change to 42 §3.13 and a DR of its own, and
/// `req/78` §3.2 Λ4 is the proof that the comparison cannot simply be re-run: with `ApplyStarted`
/// present, a fingerprint that differs from `fp0` is what a **successful** apply looks like.
/// So the honest verb is to say what was done and what was not compared.
/// 🔴 **R35 / `req/470` H-01 — this function moved, and this name stayed.**
///
/// The body is now [`crate::recovery::announced_road_note_for`], because audit 34 counted the
/// roads into 43 §7's recovery and found **five** shipped verbs walking the one this sentence is
/// about while exactly one of them said so. The sentence belongs to the road, not to `gx serve`.
///
/// The name stays here, with `gx serve`'s voice bound in, for two reasons that are not
/// convenience: `crates/gx-cli/tests/r34_serve_recover_fold.rs` measures **these** bytes and must
/// go on measuring the same ones across the move, and `no-delete` asks a relocated thing to leave
/// a pointer rather than a hole. New callers should name the verb they are.
#[must_use]
pub fn announced_road_note(row: &gx_engine::pipeline::Recovered) -> Option<String> {
    crate::recovery::announced_road_note_for("gx serve", row)
}

/// Everything a running server needs, built from a project and a set of flags.
///
/// # Errors
/// [`Error::Usage`] for the three refusals in the module header, [`Error::NotFound`] for a signing
/// key the store does not hold, [`Error::Io`]/[`Error::Layout`] from `.gx/`, [`Error::Engine`] if the
/// journal will not open and [`Error::Gate`] if the shipped pack will not parse.
pub fn build(
    project: &Path,
    store: &KeyStore,
    spec: &ServeSpec,
    mcp: &crate::session::McpWiring,
) -> Result<(AppState, SocketAddr)> {
    if spec.tls.0.is_some() || spec.tls.1.is_some() {
        return Err(Error::Usage {
            detail: "--tls-cert/--tls-key are in 44 §1.2's synopsis and v0.1 serves plain HTTP: 44 \
                     §2.5 puts mTLS in \"v0.2 (announced)\" (sem: SEM-gx-cli-196) and req/88 §1 N-09 keeps it out of this \
                     milestone. Accepting the flags and serving an unencrypted socket would tell an \
                     operator the connection is protected when it is not (M6H3-5's rule: a flag with \
                     nowhere to go is refused, never dropped). Terminate TLS in front of the \
                     loopback bind until v0.2"
                .to_string(),
        });
    }
    let bind = resolve_bind(spec.bind.as_deref())?;

    // 🔴 **`req/895` §3** (`req/878` finding 2) — this is a **refusal body**, which a caller's
    // script parses and logs. It used to name `44 §2.5`, `M4H4-2` and a `sem:` anchor, none of
    // which its reader can open: this repository's `req/` tree is not shipped. The reason it gives
    // is unchanged; the words it gives it in are now the reader's, and the provenance stays here in
    // a comment, which is where provenance goes (`req/306` §1 item 2). The requirement is 44 §2.5's
    // (`Authorization: Bearer <token>` on every endpoint except `/healthz`), hand 5's reading of
    // M4H4-2 is why a missing token is a usage error rather than a 401, and the anchor is
    // (sem: SEM-gx-cli-197).
    let token_file = spec.token_file.as_ref().ok_or_else(|| Error::Usage {
        detail: "--token-file <PATH> is required. Every endpoint except /healthz needs an \
                 `Authorization: Bearer <token>` header, so a server started without a token could \
                 only answer an internal error, and \"your token is wrong\" is not an honest thing \
                 to tell a caller when the server holds no token at all. The path is the argument \
                 and the token is not, so that `ps` shows where the secret is rather than what it \
                 is"
        .to_string(),
    })?;
    let token = std::fs::read_to_string(token_file)
        .map_err(crate::io("read", token_file))?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(Error::Usage {
            detail: format!(
                "{} holds no token. An empty bearer would make every request carrying an empty \
                 header succeed, which is \"authentication is off\" (sem: SEM-gx-cli-198) wearing the shape of \
                 \"authentication passed\"",
                token_file.display()
            ),
        });
    }

    let layout = Layout::open(project)?;
    // 🔴 **R10 / `req/238` H-01** — `gx serve` is a writer and asks for the settings file for the
    // reason the single-shot verbs do (`Session::open_wired_with_posture`): the very next line
    // reads `engine_signing_keyid` out of it, and a `config.toml` that is gone would answer "no key
    // recorded" rather than "the file that records the key is missing".
    layout.require_config()?;
    let recorded = recorded_signing_keyid(&layout)?;
    let signing_id = spec
        .signing_key
        .clone()
        .or_else(|| recorded.clone())
        .ok_or_else(|| Error::Usage {
            detail:
                "this server has no signing key: pass --signing-key <KEY_ID>, or record one in \
                     `.gx/config.toml` as `engine_signing_keyid = \"…\"` (req/56 §2's \"only a reference to the \
                     public keyid\" (sem: SEM-gx-cli-199) slot, E-M6-7). Every `VerdictReceipt` and `CommitReceipt` this \
                     surface issues is signed with it, and 45 §1 keeps it distinct from the \
                     adjudicator's key"
                    .to_string(),
        })?;

    let signing = store.load(&signing_id)?;
    let mut rulers = BTreeMap::new();
    for entry in store.list()? {
        if let Ok(key) = store.load(&entry.key_id) {
            rulers.insert(entry.key_id.clone(), key);
        }
    }
    let keys = Arc::new(StoreKeys { signing, rulers });

    let evidence = RequestEvidence::new();
    let mode = spec.record_only.then_some(EnforcementMode::RecordOnly);
    let posture = match spec.fail_posture.as_deref() {
        None | Some("closed") => FailPosture::FailClosed,
        Some("open") => FailPosture::FailOpen,
        Some(other) => {
            return Err(Error::Usage {
                detail: format!(
                    "--fail-posture takes closed|open (44 §1.2); got {other:?}. The two axes are \
                     independent (43 §4): --record-only decides whether a Deny still applies, \
                     --fail-posture decides what happens when the verifier cannot be reached"
                ),
            })
        }
    };
    // 🔴 **R19 / `req/279` H-02 (b)** (`req/284` §1.1) — the server opens wired to the MCP server
    // **this invocation** named, not to nothing.
    //
    // What stood here was `open_engine`, whose entire body is `open_engine_wired(…,
    // &McpWiring::default())`. `--mcp-server`/`--mcp-server-arg`/`--mcp-restore-catalogue` are
    // `clap` globals, so `gx serve` accepted them and this line discarded them — and the GUI, which
    // reaches gx only over HTTP, could then neither allow nor take back an MCP change: audit 19
    // measured `POST /v1/candidates/{id}/escalation` and `POST /v1/transformations/{id}/undo` both
    // answering **502 ADAPTER_ERROR** with `this "gx" is connected to no MCP server`. The wiring
    // value already existed and `open_engine_wired` already took it; the long-lived road was the
    // one caller that never handed it one.
    //
    // One server per process, named at start-up, exactly like `gx wrap`: the alternative — a server
    // that starts a child per request — would put a handshake inside 43 T-5's path and would make
    // "which server did this receipt's effect reach" a question with a different answer each time.
    let mut engine =
        crate::session::open_engine_wired(&layout, evidence.clone(), mode, posture, None, mcp)?;
    // 🔴 **DR-43-4** — 43 T-6's two deadlines, overridable for the one thing that cannot otherwise
    // be measured through the binary.
    //
    // 33 NFR-028 sets them at 24 and 72 hours, which is right for an operator and impossible for a
    // test: an E2E that wanted to watch the reaper expire a row would have to wait a day. The
    // engine has taken `with_ttl` since M5 for exactly this reason (`ac_045` asks it for 100 ms)
    // and no road to it existed through `gx serve`. This is that road, and it is deliberately an
    // **environment variable and not a flag**: 44 §1.2 fixes `gx serve`'s synopsis, a new flag is a
    // surface addition and therefore a DR, and a value that changes how long a person has to answer
    // an escalation must not look like an ordinary option. It is named in the start-up line, so a
    // server running with it says so in the log an operator already reads.
    let ttl = env_millis("GX_SERVE_TTL_MS")?;
    if let Some(ttl) = ttl {
        engine = engine.with_ttl(ttl, ttl);
    }
    let reap_interval = reap_interval()?;

    // 🔴 **T6 conditions ②/③ — the start-up gate** (`req/38` §148 ruling 1(iv), `req/190` §5-1).
    //
    // This is the one place in the process where both of `Engine::recover`'s inputs exist: the
    // adapters are registered (inside `open_engine`) and the signing key is loaded. M5H5-1 named it
    // "the first operation after the adapters are registered" and nothing had ever been that
    // operation — `req/182` H-03 grepped the shipped source and found `recover` called from
    // production code **zero** times, which is why 43 §7's "run at start-up" was a sentence with no
    // implementation behind it.
    //
    // Three steps under the lock, and the order is the argument:
    //
    // 1. **catch up** — read whatever a CLI wrote while no server was running. Without it the
    //    ledger this server is about to append to is the one it read at `open`, and `req/182` H-01
    //    measured what that produces: a leaf staged at an index the file already holds.
    // 2. **`recover`** — 43 §7-3's crash window. A `Committing` row is finished or aborted here or
    //    it is finished nowhere, and `req/190` §9-3 is right that this re-applies: 43 §7-3b needs a
    //    postcondition fingerprint that is recorded nowhere, so the only road to it is an apply the
    //    adapter contracts to be idempotent (M5H5-3). That is a side effect at start-up, so the
    //    count is printed and never merely logged internally — `resumed > 0` also goes to stderr.
    // 3. **`ledger_agrees`** — `req/182` M-12. `false` is a **refusal to start**, not a warning and
    //    not a `--force` flag: it means the journal and the ledger are describing different trees,
    //    and a server that started anyway would sign checkpoints over one of them. A flag here would
    //    be, in `serve.rs`'s own standing phrase, a ruling turned into a checkbox.
    let lock = Arc::new(ProcessLock::open(layout.join(LOCK_FILE))?);
    let startup = {
        let _held = lock.acquire_owned("gx serve")?;
        // 🔴 **DR-43-7 / `req/215` M-05 — the start-up line names what the open repaired.**
        //
        // Read *before* the catch-up, because a catch-up that re-opens the ledger replaces the
        // store and with it the account of what opening it found.
        //
        // `req/182` H-08 asked for a quarantine shape and recorded that `Recovery` had zero
        // production consumers. R1 built the one structured line 44 §1.2 asks for and did not put
        // `Recovery` in it, so `req/215` probe (b) appended 22 bytes of rubbish to a journal, ran
        // `gx serve`, watched the file go from 3140 bytes to 3118 — and found not one word about it
        // in the start-up line or on stderr. Silent repair is indistinguishable from data that was
        // never written. Both files are reported, and both now say where the removed bytes went.
        let recovery = serde_json::json!({
            "journal": {
                "torn_tail_bytes": engine.journal().recovery().torn_tail_bytes,
                "quarantined_to": engine
                    .journal()
                    .quarantined()
                    .map(|p| p.display().to_string()),
            },
            "ledger": {
                "torn_tail_bytes": engine.ledger().recovery().torn_tail_bytes,
                "quarantined_to": engine
                    .ledger()
                    .quarantined()
                    .map(|p| p.display().to_string()),
            },
        });
        let torn_bytes = engine.journal().recovery().torn_tail_bytes
            + engine.ledger().recovery().torn_tail_bytes;
        if torn_bytes > 0 {
            // The same reason `resumed > 0` goes to stderr: a repair is a side effect, and a side
            // effect an operator has to go looking for is one they will not find.
            //
            // 🔴 **R5 / DR-43-9** — and a repair that did **not** happen is a side effect nobody
            // performed, so it must not be announced as one. Where the bytes did not come back
            // because the chain broke, `EngineJournal::open` removes nothing and copies nothing:
            // everything after a break is a whole record, and cutting there would delete what
            // nobody asked to lose. This lane's own probe caught the old sentence telling an
            // operator that 3,947 bytes had been "removed" and "copied" from a file that was
            // untouched.
            let quarantined =
                engine.journal().quarantined().is_some() || engine.ledger().quarantined().is_some();
            if quarantined {
                crate::note!(
                    "gx serve: opening this project removed {torn_bytes} byte(s) that would not \
                     replay; the files were copied to `<file>.torn.<replayed>-<total>` first \
                     (DR-43-7). See `recovery` in the start-up line"
                );
            } else {
                crate::note!(
                    "gx serve: {torn_bytes} byte(s) of this project would not replay and were \
                     **left exactly where they lie** — nothing was removed and nothing was copied \
                     (DR-43-9). Run `gx repair` for the byte the chain stopped verifying at"
                );
            }
        }
        let caught = engine.catch_up()?;
        // 🔴 **R36 / `req/476` H-01** — the `Err` arm, which three consecutive audits left
        // undriven on this verb and which `req/476` §7-1 declares as its own gap. `gx serve` is the
        // road a project comes back on after a power cut, so a start-up that applies a delta and
        // then fails while recording it is the shape most likely to be met in the field — and the
        // one where the operator has nothing else to read.
        let recovered = match engine.recover(crate::clock::now(), keys.signing()) {
            Ok(recovered) => recovered,
            Err(why) => {
                crate::recovery::announce_interrupted_recovery("gx serve", &engine);
                return Err(why.into());
            }
        };
        // 🔴 **R8 / `req/234` H-01 (b)** — the start-up road, which `req/234` named as the one to
        // fix first: a server is what a project comes back on after the power cut, and until R8 the
        // receipts 43 §7-3b re-issued here were dropped on the floor.
        crate::session::file_recovered_receipts(&layout, &recovered);
        // 🔴 **R34 / `req/449` H-02** — the fold, at `gx repair`'s granularity. See
        // [`RecoverFold`].
        let fold = RecoverFold::of(&recovered);
        let RecoverFold {
            terminal,
            resumed,
            nothing_applied,
            refused,
            ..
        } = fold;
        // 🔴 **R5 / `req/227` H-01** — a refusal on this road is the loudest thing a start-up can
        // say: the recovery is the one place gx writes to a substrate without being asked, and it
        // has just declined to. The sentence names the row and the reason, on stderr, before the
        // start-up line an operator's tooling parses.
        for row in &recovered {
            if let Some(why) = row.refusal {
                crate::note!(
                    "gx serve: 43 §7 did not resume {}: {why}",
                    row.transformation.0.to_text()
                );
            }
            // 🔴 **R13 / `req/244` H-03** — a row closed from the leaf alone is a resume that
            // issued no receipt, and a start-up is where an operator finds that out. Said here for
            // the reason the refusal above is said here: the recovery is the one place gx acts
            // without being asked, and the loudest thing it can do is name what it did not check.
            if row.path == RecoveryPath::ClosedFromLedgerLeaf {
                crate::note!(
                    "gx serve: 43 §7-3b closed {} from the ledger's leaf — the commit had \
                     completed before the crash and its receipt was not filed, so the journal \
                     record was written and **no receipt was issued**. `gx repair` counts it under \
                     `receipts_missing`; `gx repair --yes --reissue-receipts` from a process that \
                     can read this row's substrate is what files one (req/244 H-03)",
                    row.transformation.0.to_text()
                );
            }
        }
        // 🔴 **R34 / `req/449` H-01 + H-02**, rewired by **R35 / `req/470` H-01** — 43 §7-3c's
        // road, said out loud, on every row that walked it.
        //
        // This used to be a fourth branch inside the loop above, and being inside *this* loop was
        // the defect: it made the sentence `gx serve`'s property rather than the road's. Audit 34
        // counted five shipped verbs on that road and one of them saying so. The call is now the
        // same call `gx repair` makes and the same one `Session::recover` makes on behalf of
        // `gx verify`, `gx commit` and `gx undo` — see [`crate::recovery`] for why the wiring is
        // arranged so that silence is the thing that takes work.
        crate::recovery::announce_recovery("gx serve", &recovered);
        if resumed > 0 {
            // 🔴 **R33 / `req/397` H-01** — the clause after the semicolon used to read "the
            // adapter was asked again for each (M5H5-3's idempotent re-apply)", and it was already
            // false for two of the four paths this counter folds: `ClosedFromFiledReceipt` and
            // `ClosedFromLedgerLeaf` ask no adapter anything, as R13's own comment above says. R33
            // made it false for a third — 43 §7-3b now *reads* the substrate instead of re-applying
            // to it — so the honest line names the one road that still applies rather than claiming
            // all of them do. The old wording is kept in `req/443` verbatim.
            //
            // 🔴 **R34 / `req/449` H-01** — the R33 sentence above named the road that still
            // applies and stopped there. `req/449` §2-2 measured what it stopped short of: on
            // that road the delta is written over **whatever the substrate holds now**, and a
            // third party who wrote after the crash had their bytes replaced by an `rc=0`
            // start-up whose `refused` was `0` and whose `/healthz` answered `ok`. The word
            // "resumed" is not wrong; it is the whole sentence for a road where it is not the
            // whole story. So the count is now split — the rows that were **read** and the rows
            // that were **written** are two numbers rather than one — and the road that writes
            // says what it did not compare. `req/449` §2-5's second answer is the reason this is
            // a sentence and not a gate: 43 §7-3c has nothing to compare against (see
            // [`announced_road_note`]), and the finding was never "gx cannot tell" but "gx did
            // not say".
            crate::note!(
                "gx serve: 43 §7-3 resumed {resumed} transformation(s) at start-up. {} of them \
                 were closed **without asking an adapter anything** (the ledger already held the \
                 leaf: 43 §7-3b reads the substrate, or the filed receipt, and applies nothing). \
                 {} of them walked 43 §7-3c's road, where the ledger holds no leaf and the delta \
                 **is applied** — see the per-row sentence above for what that road does not \
                 compare (req/397 H-01, req/449 H-01)",
                fold.closed_from_receipt + fold.closed_from_leaf + fold.ledger_held_the_commit,
                fold.apply_was_announced,
            );
        }
        let agrees = engine.ledger_agrees();
        if !agrees {
            // 🔴 **R4 / `req/222` M-02 (still real in `req/225` §1-4)** — the start-up refusal was
            // the one face of this condition that used a different word.
            //
            // `Error::Usage` maps to `VALIDATION_ERROR`, which is 44's word for "the request is not
            // one this binary can attempt" — and nobody made a request: the operator asked to start
            // a server and the *disk* is what refused. Every other face has answered
            // `LEDGER_DISAGREES` since `req/38` §156 ruling 2(a): the CLI write gate
            // (`Session::settle`, through `Error::Malformed { what: "project" }`), `/healthz`, the
            // write path and the checkpoint signer. `req/222` M-02 named the odd one out and
            // `req/225` found it unchanged, so a monitor branching on `gx_code` had to know that
            // one of the five spells it differently. Exit is unchanged at 44 §1.4's **1**.
            // 🔴 **R6 / DR-43-11** — see `gx_api::handlers::healthz` for why the two clauses are
            // joined here rather than inside the outer format's arguments.
            // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated.
            let note =
                gx_api::journal_and_head_note(engine.journal_departure(), engine.rolled_back());
            return Err(Error::Malformed {
                what: "project",
                path: engine.journal().path().display().to_string(),
                detail: format!(
                    "this project's journal witnesses {} commit(s) and its ledger holds {}                      leaf/leaves, and `ledger_agrees` is false after recovery: the two files                      describe different trees (req/182 M-12/H-01).{} A server that started here would                      sign checkpoints over a tree its own journal contradicts, so it refuses.                      `gx repair` reports what is wrong and `gx repair --yes` runs 43 §7's recovery under the project lock (DR-43-8); `gx replay <ID>` names the rows that differ",
                    engine.sigma().ledger().len(),
                    engine.ledger().log().len(),
                    note,
                ),
            });
        }
        serde_json::json!({
            "caught_up_records": caught.records,
            "evicted_rows": caught.evicted.len(),
            "recover": {
                "terminal": terminal,
                "resumed": resumed,
                "nothing_applied": nothing_applied,
                // 🔴 **R5 / `req/227` H-01** — how many rows 43 §7 left alone rather than
                // re-applying.
                //
                // 🔴 **R33 / `req/397` H-01** — what this number means, corrected.
                //
                // It said: "A number that is not zero means the substrate was **not** written to
                // and the project needs `gx repair`." The second half was true. The first half was
                // a claim about a substrate this process had already written to: until R33 the
                // recovery re-applied the delta and *then* compared the rebuilt payload against the
                // ledger's leaf, so a row counted here could be a row whose world had just been
                // moved — `req/397` §2-3 measured `"two\n"` going back to `"one\n"` on a start-up
                // that printed `refused: 1`. An operator reading this field for "the disk is
                // untouched" was reading a sentence gx's own source had contradicted.
                //
                // What it means now, exactly: **43 §7 declined to close these rows.** On 43 §7-3b's
                // road (the ledger already holds the leaf) the recovery reads the substrate and
                // never applies, so a refusal there did not write. On §7-3c's road (no leaf) the
                // delta is applied before anything can be compared, and a refusal there is
                // `ApplyWasAnnounced` — which is counted under `resumed`, not here. The rows that
                // reach this counter are the ones whose refusal sentence is printed on stderr
                // above; that sentence, and not this number, is what says what was touched.
                //
                // 🔴 **R34 / `req/449` H-02** — the last two sentences of the paragraph above are
                // corrected rather than deleted, because the correction is the finding. "A
                // refusal there is `ApplyWasAnnounced` — which is counted under `resumed`" was
                // written as a *definition* by the same lane that fixed this field's meaning, and
                // `req/449` §3-1 read it back as the thing to fix: a row whose apply was
                // announced and whose adapter then refused ends `Aborted(ApplyFailed)` with
                // `Rollback::NotAttempted`, and folding it into `resumed` told an operator that a
                // transformation had been carried forward. Since R34 `apply_was_announced` below
                // is its own number (`gx repair` has counted it apart since `req/329` M-01) and
                // every row on that road prints a sentence. This field still means exactly what
                // the paragraph above says: **43 §7 declined to close these rows**, and it is
                // still the case that a §7-3c row never reaches it.
                "refused": refused,
                // 🔴 **R34 / `req/449` H-02** — the four sub-counts `resumed` sums, at the
                // granularity `gx repair --json` has published since `req/329` M-01 / R27. The
                // audit's sentence was that "the same repair is in one of the two verbs", and
                // `crates/gx-cli/tests/r27_reentrant_abort.rs:614` pins the repair on the other
                // one. `resumed` above is unchanged and is still their sum, so nothing that read
                // this line before reads it differently now.
                //
                // The one an operator has to be able to find is `apply_was_announced`: it is the
                // only road of the four on which this start-up **wrote to a substrate**, and it
                // is the road whose rows may have ended `Aborted`.
                "closed_from_receipt": fold.closed_from_receipt,
                "closed_from_leaf": fold.closed_from_leaf,
                "ledger_held_the_commit": fold.ledger_held_the_commit,
                "apply_was_announced": fold.apply_was_announced,
                // 🔴 **R34 / `req/449` H-02** — of the `apply_was_announced` rows, how many ended
                // in a terminal `Aborted`. `resumed` counts them and `gx repair`'s own report did
                // not separate them either; this is the number that says "a row on the writing
                // road did not come back", and the per-row sentence on stderr says what that
                // means for the world.
                "announced_and_aborted": fold.announced_and_aborted,
            },
            "ledger_agrees": agrees,
            "journal_rows": engine.shadow().len(),
            // 🔴 **DR-43-4 (`req/38` §148 ruling 1(iv))** — what this server does about 43 T-6,
            // in the line an operator already reads.
            //
            // R1 printed `reaper: "none: …"` here and that string was the honest disclosure of an
            // absence: 43 §9's INV-L1/INV-L2 want expiry to happen and no process made it happen,
            // so a `Candidate` whose TTL passed while nobody called was expired by the next write
            // that touched it and not by the clock. The absence is gone; the disclosure is not —
            // it now names the period and the two deadlines, because "there is a timer" and "the
            // timer runs every minute against a twenty-four-hour deadline" are different facts and
            // only the second one lets a reader predict what the server will do.
            "reaper": {
                "interval_ms": u64::try_from(reap_interval.as_millis()).unwrap_or(u64::MAX),
                "verify_ttl_ns": ttl.unwrap_or(gx_engine::pipeline::DEFAULT_VERIFY_TTL_NANOS),
                "escalation_ttl_ns": ttl.unwrap_or(gx_engine::pipeline::DEFAULT_ESCALATION_TTL_NANOS),
                "sweeps": if reap_interval.is_zero() {
                    "none: GX_SERVE_REAP_INTERVAL_MS=0 turned the timer off; TTL expiry is then evaluated only by the next operation that touches a row (DR-43-4)"
                } else {
                    "gx_api::serve's timer calls AppState::reap_once, which takes .gx/LOCK, catches up, and skips on BUSY (DR-43-4)"
                },
            },
            // 🔴 **DR-43-7 / `req/215` M-05** — what opening the two files repaired, and where the
            // removed bytes were put first.
            "recovery": recovery,
        })
    };

    let archive = Arc::new(StoreArchive {
        store: ReceiptStore::in_layout(&layout),
    });
    let drafts = Arc::new(StoreDrafts {
        store: crate::draft::DraftStore::in_layout(&layout),
    });
    let state = AppState::new(
        engine,
        evidence,
        keys,
        Bearer::new(token),
        layout.join("index"),
        recorded.as_deref(),
    )
    .map_err(|e| Error::Usage {
        detail: format!("{}: {}", e.title, e.detail),
    })?
    .with_archive(archive)
    .with_drafts(drafts)
    .with_writer_lock(lock)
    // 🔴 **R11 / `req/240` M-04** — see [`LayoutMeta`].
    .with_project_meta(Arc::new(LayoutMeta {
        layout: layout.clone(),
    }))
    .with_startup(startup);

    Ok((state, bind))
}

/// 44 §1.2's start-up log line: "stdout: startup log (structured JSON line)" (sem: SEM-gx-cli-200).
///
/// 🔴 It carries the absence. `gx_api::auth::ABSENCE_NOTICE` is in `--help`, which an operator reads
/// once; this line is in the log, which is what a second operator finds six months later when they
/// are asking what this process is. 44 §1.2 asks for a structured line and does not say what it holds,
/// so what it holds is the four facts that decide whether the socket is safe: where it is, which
/// policy set is behind it, which key signs, and that the only check is a token.
#[must_use]
pub fn start_line(
    addr: SocketAddr,
    spec: &ServeSpec,
    signing_key_id: &str,
    runtime: &serde_json::Value,
    mcp: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "event": "gx.serve.started",
        "bind": addr.to_string(),
        "enforcement": if spec.record_only { "RecordOnly" } else { "Enforce" },
        "fail_posture": spec.fail_posture.as_deref().unwrap_or("closed"),
        "signing_key_id": signing_key_id,
        "authorization": gx_api::auth::ABSENCE_NOTICE,
        // 🔴 **P3** — two defaults said out loud rather than left to be inferred from an absence.
        //
        // `otel`: NFR-017's exporter is **off** unless an invocation named a sink, and R-114-4's
        // "state zero telemetry explicitly" (sem: SEM-gx-cli-201) is only true if a start-up line says so (`gx_cli::otel`).
        // `verdict_checkpoints`: ruling ⑥ makes issuing manual — this server runs no timer, so a
        // deployment that never calls `POST /v1/verdict-checkpoints` publishes no counts at all,
        // which is exactly the thing FR-M04 exists to make visible.
        "otel": crate::otel::Exporter::disabled().status(),
        // 🔴 **R19 / `req/279` H-02 (b)** — which MCP server this process is connected to, said out
        // loud. Before this lane `gx serve` accepted `--mcp-server` and opened its engine with
        // `McpWiring::default()`, so the flag was a promise the log never contradicted. The zero is
        // printed for the reason `otel` prints "disabled": an operator who never sees the field is
        // reading the log of a binary that could not have been wired at all
        // (`crate::session::McpWiring::summary`).
        "mcp": mcp,
        "verdict_checkpoints": "manual: POST /v1/verdict-checkpoints (ruling ⑥; no timer runs here)", // (sem: SEM-gx-cli-202)
        // 🔴 **DR-43-2** — what the start-up gate found, in the line an operator already reads.
        //
        // 44 §1.2 asks for **one** structured line, so these facts ride in it rather than in a
        // second one. `ledger_agrees` is always `true` here by construction (a `false` refuses to
        // start, see `build`), and printing it anyway is the point: the field's presence is what
        // makes "this server checked" observable, and an operator who never sees the field is
        // reading the log of a binary from before DR-43-2.
        "runtime": runtime,
        // 🔴 **DR-43-4 landed** (`req/38` §148 ruling 1(iv), lane R2). This key used to be a
        // string reading `"none: TTL expiry is evaluated on the next operation that touches a row,
        // not on a timer (DR-43-4)"` (sem: SEM-gx-cli-1001) — P-3's disclosure of an absence. The
        // absence is closed and the disclosure moved **into** `runtime.reaper`, where it names the
        // period and the two deadlines rather than an apology; the key is kept here so that a
        // script reading the start-up line for the word `reaper` still finds it.
        "reaper": runtime.get("reaper").cloned().unwrap_or(serde_json::Value::Null),
    })
}

/// The configuration `gx_api::serve` is handed.
///
/// 🔴 **DR-43-4** — the reaper's period comes from `GX_SERVE_REAP_INTERVAL_MS` when it is set (see
/// [`env_millis`] for why it is an environment variable and not a flag), and from
/// [`gx_api::serve::DEFAULT_REAP_INTERVAL`] otherwise.
///
/// # Errors
/// [`Error::Usage`] if the variable is set to something that is not a number of milliseconds.
pub fn config(bind: SocketAddr) -> Result<ServeConfig> {
    Ok(ServeConfig::new(bind).with_reap_interval(reap_interval()?))
}

/// 🔴 **DR-43-4** — the reaper's period for this process, read once.
///
/// Read in two places that must agree: [`config`], which hands it to the runtime, and [`build`],
/// which puts it in the start-up line. A server whose log said one period and whose timer ran
/// another would be a log that cannot be used to reason about the server, so the reading is one
/// function.
///
/// # Errors
/// [`Error::Usage`] as [`env_millis`].
fn reap_interval() -> Result<std::time::Duration> {
    Ok(match env_millis("GX_SERVE_REAP_INTERVAL_MS")? {
        Some(nanos) => std::time::Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX)),
        None => gx_api::serve::DEFAULT_REAP_INTERVAL,
    })
}

/// 🔴 **DR-43-4** — a duration in milliseconds read from the environment, as nanoseconds.
///
/// Two values reach `gx serve` this way and neither is in 44 §1.2's synopsis: the reaper's period
/// and 43 T-6's two TTLs. A flag for either would be a surface addition (44 §2.6's
/// "backward-compatible addition" clause, and therefore a DR), and both exist for the same reason:
/// 33 NFR-028's deadlines are 24 and 72 hours, so **nothing driven through the binary can observe
/// the reaper at all** without a way to shorten them — the shape `Engine::with_ttl` has had since
/// M5 and that `gx serve` had no road to.
///
/// Strict rather than lenient: a value that is not a number is a **refusal to start**, not a
/// silent fall back to the default. An operator who mistyped `GX_SERVE_TTL_MS=6O` (letter O) and
/// got a server running 24-hour deadlines would be reading a log line that says 24 hours and
/// believing they asked for a minute; M4H5-5's rule — "an argument that changes nothing must not
/// look like an argument that worked" — reaches an environment variable the same way it reaches a
/// flag.
///
/// # Errors
/// [`Error::Usage`] if the variable is set and is not a non-negative integer.
fn env_millis(name: &str) -> Result<Option<i64>> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(None);
    };
    let text = raw.to_string_lossy().trim().to_string();
    let millis: i64 = text.parse().map_err(|_| Error::Usage {
        detail: format!(
            "{name}={text:?} is not a number of milliseconds. It is an operational override for 43 \
             T-6 (DR-43-4) and is not in 44 §1.2's synopsis; unset it to use the shipped default"
        ),
    })?;
    if millis < 0 {
        return Err(Error::Usage {
            detail: format!("{name}={text:?} is negative; 43 T-6 has no deadline before its state"),
        });
    }
    Ok(Some(millis.saturating_mul(1_000_000)))
}
