// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `.gx/drafts/` — **M6-01 adopted (a)**: where the intent body lives between `gx submit` and `gx plan`. (sem: SEM-gx-cli-018)
//!
//! # The hole this closes
//!
//! req/88 §4 M6-01 is the reqdef's own "the batch's most important finding" (sem: SEM-gx-cli-019) and a declared hand-1 blocker:
//!
//! > there is nowhere the intent body that `gx plan <ID>` requires is stored -- not in the
//! > canon, not in the implementation, not in the `.gx/` definition. `Engine::plan` takes
//! > `&Intent`, and the journal's `DraftCreated` holds only `{intent_id, rng_seed, at}`.
//! > **Because a single-shot-process CLI runs `gx submit` and `gx plan` as separate processes**,
//! > the intent body cannot cross a process boundary. (sem: SEM-gx-cli-020)
//!
//! The engine's own documentation had written the assumption down: "44 §1.2's `gx plan <ID>`
//! resolves an id to a session **the CLI is holding**" (sem: SEM-gx-cli-021) — and a single-shot process holds nothing.
//! req/38 §47 adopted (a): the body is the CLI's, in `.gx/drafts/`, and the engine is not touched.
//!
//! # 🔴 Why the file is JSON and not the `.cbor` the ticket wrote
//!
//! M6-01(a)'s text says `.gx/drafts/<IntentId>.cbor`. This implements `<IntentId>.json`, and the
//! reason is **Rule 1** (sem: SEM-gx-cli-022):
//!
//! * a canonical CBOR encode outside gx-canon is exactly what 41 §6 forbids ("every canonical
//!   encode goes through gx-canon only, bypass forbidden" (sem: SEM-gx-cli-023)), and Rule 1 (i) turns that into a mechanical check on this crate;
//! * a *non*-canonical CBOR encode would need a second CBOR crate in the graph, i.e. a second
//!   spelling of a format this project has exactly one spelling of;
//! * 44 §1.3 makes JSON the CLI's format anyway ("both CLI and HTTP take JSON as the primary
//!   format" (sem: SEM-gx-cli-024)).
//!
//! Nothing depends on the draft file's bytes: the `IntentId` is minted by `Engine::submit` from the
//! canonical form of the `Intent`, not from this file, and this file is keyed **by** that id. So the
//! encoding is a storage detail rather than an identity, which is the test — and `.cbor` would have
//! made a storage detail look like one. Raised as **M6H1-2**.
//!
//! # 🔴 Why this holds five fields and not an `Intent`
//!
//! `gx_core::Intent` has no `Deserialize`, deliberately, and gx-core says why in a comment that
//! names this milestone:
//!
//! > Nothing reads an `Intent` back yet. […] the reader of an `Intent` is the `submit` API of 44,
//! > which is M6. A `Deserialize` published now would fix the spelling a submit body is parsed from
//! > **before the hand that has to live with it has looked at 44**.
//!
//! Hand 3 is the hand that implements `gx submit` and therefore the hand that has to live with it.
//! So this hand does not publish that wire face; it records the **five arguments the CLI was given**
//! (42 §3.3's five fields) and rebuilds the `Intent` with [`gx_core::Intent::new`]. The round trip is
//! then a CLI-layer fact rather than a core-layer commitment, and hand 3 remains free to choose the
//! submit body's spelling. `goal` is base64 through [`gx_core::b64`] — the same encoding
//! `GoalBytes`'s own human-readable `Serialize` uses, so the two spellings agree if hand 3 later
//! makes them one.

use std::path::{Path, PathBuf};

use gx_core::{b64, Actor, ChangeContext, GoalBytes, Intent, IntentId, SubstrateKind};
use serde::{Deserialize, Serialize};

use crate::{io, layout::Layout, Error, Result};

/// The five arguments `gx submit` was given, as they are stored.
///
/// 42 §3.3's row, one field at a time. The names are the specification's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftRecord {
    /// Which substrate the intent is against.
    pub substrate: SubstrateKind,
    /// The position inside it, in the adapter's own spelling.
    pub locator: String,
    /// The adapter-defined description of what is wanted, base64 (see the module documentation).
    pub goal_b64: String,
    /// Which kind of change this belongs to.
    pub context: ChangeContext,
    /// Who is asking.
    pub actor: Actor,
}

impl DraftRecord {
    /// Take the five fields off an intent.
    #[must_use]
    pub fn of(intent: &Intent) -> Self {
        Self {
            substrate: intent.substrate().clone(),
            locator: intent.locator().to_string(),
            goal_b64: b64::encode(&intent.goal().0),
            context: intent.context().clone(),
            actor: intent.actor().clone(),
        }
    }

    /// Put them back.
    ///
    /// # Errors
    /// [`Error::Malformed`] if `goal_b64` is not base64. Strict rather than lossy: a goal that
    /// decoded differently would be a **different intent**, and 42 §3.3 makes the goal part of the
    /// `IntentId`, so a silent repair here would produce an id the engine never minted.
    pub fn to_intent(&self, whence: &Path) -> Result<Intent> {
        let goal = b64::decode(&self.goal_b64).map_err(|detail| Error::Malformed {
            what: "draft",
            path: whence.display().to_string(),
            detail: format!("goal is not base64: {detail}"),
        })?;
        Ok(Intent::new(
            self.substrate.clone(),
            self.locator.clone(),
            GoalBytes(goal),
            self.context.clone(),
            self.actor.clone(),
        ))
    }
}

/// `.gx/drafts/`, as a store.
#[derive(Debug, Clone)]
pub struct DraftStore {
    dir: PathBuf,
}

impl DraftStore {
    /// The store inside an opened layout.
    #[must_use]
    pub fn in_layout(layout: &Layout) -> Self {
        Self {
            dir: layout.join("drafts"),
        }
    }

    /// Where one draft is filed.
    ///
    /// The name is the `IntentId`'s own text form (42 §1.2, `gx1:<base32>`) with the `:` replaced,
    /// because a colon is not a filename character on Windows and 33 NFR-021 makes Linux the release
    /// target while leaving development elsewhere. Base32 is already lowercase and `[a-z2-7]`, so
    /// nothing else in the name needs escaping — and the id is fixed-length, so two drafts cannot
    /// collide by truncation.
    #[must_use]
    pub fn path_of(&self, id: &IntentId) -> PathBuf {
        self.dir
            .join(format!("{}.json", id.0.to_text().replace(':', "_")))
    }

    /// File a draft under the id the **engine** minted.
    ///
    /// The id is an argument rather than something computed here, and that is Rule 1 (i) in the (sem: SEM-gx-cli-025)
    /// signature: this crate may not compute a `Cid`, so the only way it can name a draft is with a
    /// name `Engine::submit` handed it.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be written.
    pub fn put(&self, id: &IntentId, intent: &Intent) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir).map_err(io("create", &self.dir))?;
        let path = self.path_of(id);
        let body = serde_json::to_vec_pretty(&DraftRecord::of(intent))
            .expect("a DraftRecord of owned std types always serialises");
        std::fs::write(&path, body).map_err(io("write", &path))?;
        Ok(path)
    }

    /// Read one back, if it is there.
    ///
    /// `Ok(None)` for "no such draft" and `Err` for "there is a file and it is not a draft". The
    /// two are different answers and E-M4-35 is the rule that keeps them apart: do not read
    /// "cannot be read" as "does not exist" (sem: SEM-gx-cli-026).
    ///
    /// # Errors
    /// [`Error::Io`] if the file exists and cannot be read. [`Error::Malformed`] if it is not a
    /// draft.
    pub fn get(&self, id: &IntentId) -> Result<Option<Intent>> {
        let path = self.path_of(id);
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io("read", &path)(e)),
        };
        let record: DraftRecord =
            serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
                what: "draft",
                path: path.display().to_string(),
                detail: detail.to_string(),
            })?;
        record.to_intent(&path).map(Some)
    }

    /// Forget one. `true` if there was something to forget.
    ///
    /// # Errors
    /// [`Error::Io`] if the file exists and cannot be removed.
    pub fn remove(&self, id: &IntentId) -> Result<bool> {
        let path = self.path_of(id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io("remove", &path)(e)),
        }
    }

    /// How many drafts are filed.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory cannot be listed.
    pub fn len(&self) -> Result<usize> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(io("list", &self.dir)(e)),
        };
        Ok(entries
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .count())
    }

    /// Whether the store is empty.
    ///
    /// # Errors
    /// As [`DraftStore::len`].
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}
