//! `.gx/index/` — **M6-02 採(b)**: the CLI's cache of 44 §0's id-resolution.
//!
//! # What this is a cache *of*
//!
//! [`gx_engine::Engine::resolved`] (**M6-02 採(a)**) is the answer. This is a copy of it on disk, and
//! it exists because the two are consulted at different moments: the engine's map is rebuilt when a
//! journal is opened, and a CLI that only wants to turn a `gx1:` argument into a
//! `TransformationId` — `gx receipt show <IntentId>`, say — would otherwise have to open a journal
//! to answer a question about a name.
//!
//! req/56 §2's row for this directory is the licence for it existing at all: 「`.gx/index/` — key
//! index 等の**再生成可能な索引** — derived・消して良いと宣言 — 消えたら再生成」. So it is written where
//! a lost file costs a re-open and never where a lost file costs a fact.
//!
//! # 🔴 The word this module does not use
//!
//! req/38 §47 puts a condition on this design: 「手 1 の `.gx/index/` 設計は **`supersede` を予約語に
//! 使わず**、非 canonical 注記の将来席を塞がない」. 23 §3.1's `ReVerificationRecord` is a different
//! concept from 43 T-12's edge and avoids the word on purpose; an index that spent the token on the
//! T-12 relation would take the seat the annotation layer has not been designed into yet.
//! `probes/doubt/tests/m6_surface_doubt.rs` scans this file for it.
//!
//! # The multi-valued case
//!
//! One intent can name more than one transformation (req/88 §3 Λ3(ii), and see
//! [`gx_engine::Engine::resolved`] for the rule). This file stores **one** entry per intent — the
//! most recently learned — for the same reason the engine answers one: a cache that answered a set
//! would be a second, richer opinion than the thing it caches.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gx_core::{Cid, IntentId, TransformationId};
use serde::{Deserialize, Serialize};

use crate::{io, layout::Layout, Error, Result};

/// The file inside `.gx/index/` this module owns.
const FILE: &str = "intent_to_transformation.json";

/// The on-disk form: text ids, in `gx1:` (42 §1.2), sorted.
///
/// Text rather than bytes because 42 §1.2 makes `gx1:<base32>` the form for 「CLI/API/ログの人間可読
/// 表示・JSON埋め込み」, and because a cache a person can read with `cat` is a cache a person can
/// delete with confidence — which req/56 §2 explicitly invites them to do.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Wire {
    resolutions: BTreeMap<String, String>,
}

/// `.gx/index/`'s id-resolution cache.
#[derive(Debug, Clone, Default)]
pub struct ResolutionIndex {
    entries: BTreeMap<IntentId, TransformationId>,
}

impl ResolutionIndex {
    /// Where the cache file sits inside a layout.
    #[must_use]
    pub fn path_in(layout: &Layout) -> PathBuf {
        layout.join("index").join(FILE)
    }

    /// An empty one.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Learn a resolution. Later learnings win, which is the engine's rule (Λ3(ii)).
    pub fn learn(&mut self, intent: IntentId, transformation: TransformationId) {
        self.entries.insert(intent, transformation);
    }

    /// Look one up.
    #[must_use]
    pub fn get(&self, intent: &IntentId) -> Option<TransformationId> {
        self.entries.get(intent).copied()
    }

    /// 🔴 Forget one — **E-M6-14**'s other half.
    ///
    /// `gx draft discard` removes the body, and a cache still resolving that `IntentId` would make
    /// `gx plan` fail with 「the draft is missing」 for an intent somebody deliberately put down: a
    /// 44 §1.4 **6** that reads like corruption. `true` when something was there.
    ///
    /// It is a *removal from a cache* rather than a retraction of anything: the engine's own
    /// `resolved` map is the authority (M6-02 採(a)) and is rebuilt from the journal, which this
    /// does not touch.
    pub fn forget(&mut self, intent: &IntentId) -> bool {
        self.entries.remove(intent).is_some()
    }

    /// How many resolutions are cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether anything is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read the cache, treating **every** failure as an empty cache.
    ///
    /// 🔴 This is the one place in this crate where 「読めない」 and 「無い」 are deliberately the same
    /// answer, and the reason is req/56 §2's own cell: this directory is 「derived・消して良いと宣言」.
    /// A corrupted cache and an absent cache have the identical correct repair — rebuild from the
    /// journal — so refusing on the first would be a hand-written failure mode on a path the
    /// specification promises is disposable. E-M4-35's rule is about **source** data, and the
    /// distinction between the two is exactly [`crate::layout::Nature`].
    ///
    /// What is *not* the same is the report: the caller is told which it was.
    #[must_use]
    pub fn load(layout: &Layout) -> (Self, Freshness) {
        let path = Self::path_in(layout);
        let Ok(raw) = std::fs::read(&path) else {
            return (Self::new(), Freshness::Absent);
        };
        let Ok(wire) = serde_json::from_slice::<Wire>(&raw) else {
            return (Self::new(), Freshness::Unreadable);
        };
        let mut entries = BTreeMap::new();
        let mut skipped = 0usize;
        for (intent, transformation) in wire.resolutions {
            match (Cid::from_text(&intent), Cid::from_text(&transformation)) {
                (Ok(i), Ok(t)) => {
                    entries.insert(IntentId(i), TransformationId(t));
                }
                _ => skipped += 1,
            }
        }
        let freshness = if skipped > 0 {
            Freshness::PartiallyUnreadable { skipped }
        } else {
            Freshness::Loaded
        };
        (Self { entries }, freshness)
    }

    /// Write the cache.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory or the file cannot be written.
    pub fn store(&self, layout: &Layout) -> Result<PathBuf> {
        let dir = layout.join("index");
        std::fs::create_dir_all(&dir).map_err(io("create", &dir))?;
        let path = dir.join(FILE);
        let wire = Wire {
            resolutions: self
                .entries
                .iter()
                .map(|(i, t)| (i.0.to_text(), t.0.to_text()))
                .collect(),
        };
        let body =
            serde_json::to_vec_pretty(&wire).expect("a map of two strings always serialises");
        std::fs::write(&path, body).map_err(io("write", &path))?;
        Ok(path)
    }
}

/// What [`ResolutionIndex::load`] found. req/56 §5's 「必ず申告」, one file down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// There was a cache and it was read whole.
    Loaded,
    /// There was no cache.
    Absent,
    /// There was a file and it was not a cache. Rebuilt from nothing.
    Unreadable,
    /// There was a cache and some entries were not ids. The rest were kept.
    PartiallyUnreadable {
        /// How many entries were dropped.
        skipped: usize,
    },
}

impl Freshness {
    /// Whether the caller should rebuild from the journal before trusting the answer.
    #[must_use]
    pub fn needs_rebuild(self) -> bool {
        !matches!(self, Freshness::Loaded)
    }
}

/// The parser 44 §0's id-resolution starts with: a `gx1:` string, without knowing which id it is.
///
/// 🔴 **則 1 (i) lives or dies here.** 44 §0 asks the CLI to accept 「`IntentId`と`TransformationId`の
/// いずれの`gx1:...`値も」, and the naive way to do that is to compute something. This does not: it
/// **parses** with [`gx_core::Cid::from_text`], which is in gx-core precisely because 42 §1.2 makes
/// the text form a display concern rather than an identity one. Which of the two an id is cannot be
/// told from its bytes (42 §1.1: 「どの型を指すかはRust側の型…で判別する・自己記述タグは付与しない」),
/// so the answer is 「a `Cid`, and the caller decides」 — and the caller deciding is
/// [`ResolutionIndex::get`] finding it or not.
///
/// # Errors
/// [`Error::Malformed`] if the text is not a `gx1:` id. Strict, because
/// [`gx_core::Cid::from_text`] is strict: a decoder that accepted two spellings of one id would make
/// the text form ambiguous, and the whole point of the form is that it is not.
pub fn parse_id(text: &str) -> Result<Cid> {
    Cid::from_text(text).map_err(|e| Error::Malformed {
        what: "gx1 identifier",
        path: text.to_string(),
        detail: e.to_string(),
    })
}

/// Given a parsed id, the two ways it might be read.
///
/// Returned as a pair rather than as a guess, because guessing is what 44 §0 does **not** ask for:
/// the rule is 「いずれの値も受理し…`plan()`完了後は正準の`TransformationId`へ解決する」, which is a
/// resolution against a store and not an inspection of the bytes.
#[must_use]
pub fn both_readings(id: Cid) -> (IntentId, TransformationId) {
    (IntentId(id), TransformationId(id))
}

/// 🔴 What this module deliberately does **not** provide: a one-call 「resolve this argument」.
///
/// 44 §0's rule has three steps — parse, look up, fall back to the engine — and the third one is
/// where the authority is (`Engine::resolved`). A helper here that folded all three would put the
/// **order of authority** inside a cache module, and the order is the interesting part: a cache that
/// answered before the engine was consulted would be a second opinion presented as the first.
/// [`parse_id`] and [`both_readings`] are the two halves this layer owns; hand 3 writes the sequence
/// where the sequence belongs, in the subcommand that has an engine to ask.
///
/// The path this module writes, for a caller that wants to name it in a message.
#[must_use]
pub fn file_name() -> &'static str {
    FILE
}

/// A tiny convenience for the recovery report: whether the cache file exists.
#[must_use]
pub fn exists_in(layout: &Layout) -> bool {
    Path::new(&ResolutionIndex::path_in(layout)).is_file()
}
