// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R12 / `req/242` H-01 + H-02** — the one type that writes a project's own declarations.
//!
//! # What this module is for
//!
//! Three audits in a row (`req/238` H-01, `req/240` H-01, `req/242` H-01/H-02) found the same
//! thing wearing three faces: **something other than the repair road wrote `.gx/VERSION`**. R10
//! closed the branch it found (`Layout::create`'s defaults). R11 closed the branch it found (the
//! order inside `gx repair --yes`). `req/242` then measured a **third** road —
//! `session::anchor_accepting` → `Layout::declare_journal_format` — writing the file on every
//! single-shot writer verb, without a word in `meta_repaired`, without a `.pre-repair` copy, and
//! re-arming R7's declaration digest one `gx submit` after a detector had fired.
//!
//! `req/38` §181 ruling 3 draws the lesson: a repair instruction written as a **place** ("move the
//! write below the lock", "make the reader one reader") closes a branch and leaves the shape that
//! grew it. So this module is written as a **type and a count**:
//!
//! * the bytes of `.gx/VERSION` and `.gx/config.toml` are composed by [`declared_text`] and
//!   [`default_contents`], and both are private to this file;
//! * every `std::fs` call that creates, replaces or renames one of those two files, or that
//!   creates `.gx/ledger/journal`, is a **private** method of [`DeclarationWriter`];
//! * [`DeclarationWriter`] has exactly **two** constructors, and each one demands the evidence its
//!   road is supposed to hold before it writes;
//! * everything else in this binary holds a [`crate::layout::Layout`], which has no method that
//!   writes any of the three. Not "does not call one" — **has none**, so a road back is a compile
//!   error rather than an audit finding.
//!
//! `probes/doubt/tests/declaration_writer_doubt.rs` counts all of that from the source text, so
//! that the sentence above is a measurement rather than a claim, and so that the count is red the
//! moment a fourth road appears.
//!
//! # The two roads, and why they are the two
//!
//! * [`DeclarationWriter::for_init`] — a directory that is **not a project yet**
//!   ([`crate::layout::Layout::create`], which 44 gives no `gx init` for and which `gx submit`
//!   reaches on a fresh directory). Nothing can be lost here: there is no journal, no ledger and
//!   no recorded head, so there is no detector to disarm and no operator bytes to overwrite.
//! * [`DeclarationWriter::for_repair`] — `gx repair --yes`, which by the type's own signature
//!   cannot be built without the writer lock this project's `.gx/LOCK` grants and the signing key
//!   the run resolved. `req/240` H-01's finding was that those two came *after* the write; since
//!   R11 they come first, and since R12 the compiler is what says so.
//!
//! # What is deliberately **not** here
//!
//! The stamping road. Before R12, the first writer verb to touch a project that had never declared
//! a journal framing wrote one — `Layout::declare_journal_format`, called from
//! `session::anchor_accepting`. Its own doc comment claimed "a one-shot window per project, it
//! closes the first time the project is written to", and `req/242` §6 measured the window
//! **reopening every time the line is deleted**. It is gone. A project that has never declared a
//! framing keeps the treatment it had before R6 — `declared_format: None`, no downgrade detector,
//! and `gx repair` says so in `journal_format_declared` — and a project this binary creates
//! declares `chained` in the same write that creates it, from [`DeclarationWriter::initialise`].
//! `docs/LIMITS.md` v0.4-y carries what that costs.

use std::path::{Path, PathBuf};

use crate::layout::{Layout, JOURNAL_FORMAT_KEY, LAYOUT_VERSION, PRE_REPAIR_LIMIT};
use crate::{io, Error, Result};

/// 🔴 **R10 / `req/238` H-01** — what a repair did to a `Nature::Meta` file, as a value the report
/// prints.
///
/// Three answers rather than a `bool`, because "it was fine", "it was not there and now it is" and
/// "it was there, it did not read, your bytes are beside it" are three different things to tell an
/// operator, and the third one names a file they may want.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaRepair {
    /// The file was there and read. Nothing was written.
    Intact,
    /// The file was not there and was written from the project's own facts.
    Created,
    /// The file was there and did not read; the old bytes are at `kept`.
    Rewritten {
        /// Where the bytes that were there have been moved to. Never deleted (no-delete).
        kept: PathBuf,
    },
}

impl MetaRepair {
    /// The word a report carries, or `None` for a file nothing happened to.
    #[must_use]
    pub fn as_str(&self) -> Option<&'static str> {
        match self {
            MetaRepair::Intact => None,
            MetaRepair::Created => Some("created"),
            MetaRepair::Rewritten { .. } => Some("rewritten"),
        }
    }
}

/// 🔴 **R12 / `req/242` H-01** — the only type in this workspace whose functions write
/// `.gx/VERSION`, `.gx/config.toml` or a new `.gx/ledger/journal`.
///
/// Built by [`DeclarationWriter::for_init`] or [`DeclarationWriter::for_repair`] and by nothing
/// else. The field is private and the constructors are `pub(crate)`, so a caller outside this crate
/// cannot make one at all and a caller inside it cannot make one without the evidence the
/// constructor asks for.
#[derive(Debug)]
pub(crate) struct DeclarationWriter {
    /// `.gx/` — the directory whose declarations this writer owns.
    root: PathBuf,
}

impl DeclarationWriter {
    /// 🔴 Road one: a directory that is not a project yet.
    ///
    /// The caller is [`crate::layout::Layout::create`], which has already measured
    /// `Layout::established` and found it `false`. That measurement is the whole of the argument
    /// that this road is safe: with no `VERSION`, no `ledger/journal` and no recorded head there is
    /// nothing here that a write can take away.
    pub(crate) fn for_init(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// 🔴 Road two: `gx repair --yes`, holding the two things `req/240` H-01 found it writing
    /// without.
    ///
    /// The parameters are not used. They are the point: R11 moved the write below
    /// `ProcessLock::open` and below the key, and `req/242` measured that the move had held — but
    /// what held it was the **order of two statements in one function**, which is exactly the kind
    /// of guarantee the last three audits kept walking around. Here the order is the signature: a
    /// run that has not taken the lock has no [`gx_engine::store::OwnedLock`] to lend, and a run that
    /// could not resolve a key has no [`gx_witness::KeyPair`], so neither can build the value that
    /// writes.
    ///
    /// What the signature deliberately does **not** promise is that the report has been printed —
    /// `req/242` H-02's other half. That one is not expressible as a parameter (the report is
    /// composed out of facts this writer's own work changes), and it is carried instead by
    /// `repair::repair_and_report`, whose return type is `Outcome` and not `Result<Outcome>`: after
    /// the lock and the key are in hand, the compiler will not accept a `?` that could leave
    /// without printing.
    pub(crate) fn for_repair(
        root: &Path,
        _lock: &gx_engine::store::OwnedLock,
        _key: &gx_witness::KeyPair,
    ) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// 🔴 **R12** — everything req/56 §2 declares for a directory that is not a project yet.
    ///
    /// The `Nature::Meta` files, req/56 §4's `.gitignore`, and the journal — all four in one place,
    /// because all four are the same fact: *this directory becomes a project here and nowhere
    /// else*.
    ///
    /// # The declaration is complete on the first write
    ///
    /// Before R12 this wrote `1\n` and left `journal_format` to the first writer verb to stamp
    /// (`Layout::declare_journal_format`). The stamp is gone (`req/242` H-01), so the framing is
    /// declared **here**, where it is not a guess: the journal is created by the line below and a
    /// journal this binary creates is chained. A project written by this release therefore says
    /// what it is from its first byte, which is the property R6 wanted and got by a road that
    /// could be walked twice.
    ///
    /// # Errors
    /// [`Error::Io`] if any of the four cannot be written.
    pub(crate) fn initialise(&self) -> Result<()> {
        let version = self.root.join("VERSION");
        if !version.exists() {
            self.write_declaration(
                &version,
                &declared_text(
                    &[LAYOUT_VERSION.to_string()],
                    &[],
                    // 🔴 **R30 / `req/372` M-02** — a project this build creates declares the
                    // **v2** framing, because that is the framing `EngineJournal` will stamp on
                    // its journal. The declaration and the marker have to be minted by the same
                    // release or R6's downgrade guard fires on a project that was never attacked.
                    Some(gx_engine::JournalFormat::ChainedV2),
                ),
            )?;
        }
        let config = self.root.join("config.toml");
        if !config.exists() {
            self.write_config(&config, &default_contents("config.toml"))?;
        }
        // req/56 §4: "default = gitignore the whole `.gx/` (zero-interference principle)". A
        // `.gitignore` **inside** `.gx/` holding `*` ignores the whole directory including itself,
        // so the user's own `.gitignore` is not edited by us.
        //
        // 🔴 **R11 / `req/240` M-02** — and it is written only into a directory that is not a
        // project yet, which is what this whole function is. Absence in an established project is a
        // fact to report ([`crate::layout::Layout::gitignore_absent`]) and not a file to invent.
        let ignore = self.root.join(".gitignore");
        if !ignore.exists() {
            std::fs::write(
                &ignore,
                "# req/56 §4: gx keeps its state out of your history.\n# Un-ignore what you want to share, e.g. `!config.toml`.\n*\n",
            )
            .map_err(io("write", &ignore))?;
        }
        self.create_journal()
    }

    /// 🔴 **R10 / `req/238` H-01, re-cut by R12 / `req/242` H-01 + L-03** — put `.gx/VERSION` back,
    /// under the same predicate that refuses it.
    ///
    /// Three shapes, three answers:
    ///
    /// * the file is there and **reads as a declaration** — nothing is written, [`MetaRepair::Intact`];
    /// * the file is **not there** — the declaration is written from `format`, [`MetaRepair::Created`];
    /// * the file is there and does **not** read — the operator's bytes are moved to
    ///   `VERSION.pre-repair.<n>` *first* (no-delete) and the declaration is written,
    ///   [`MetaRepair::Rewritten`].
    ///
    /// # 🔴 One predicate, R12
    ///
    /// The middle sentence used to be "`gx_log::head::declaration_lines` returns a non-empty bare
    /// line", and [`Layout::open`]'s was "the first bare line parses as a number". `req/242` L-03
    /// measured the gap: a `.gx/VERSION` saved as UTF-16 LE, or holding `x`, or `1.0`, or `-1`, was
    /// `DECLARATION_UNREADABLE` at every door **and** `Intact` here, so `gx repair --yes` answered
    /// it with `meta_repaired: []`, `meta_repair_refused: null` and no explanation at all. The
    /// predicate is now [`Layout::read_declaration`] itself — the reader's — which is 43 §7.14's
    /// sentence: *the judgement that rewrites a file is the judgement that refuses it*.
    ///
    /// # Errors
    /// [`Error::Io`] if the old file cannot be moved aside or the new one cannot be written, and
    /// [`Error::Usage`] if `PRE_REPAIR_LIMIT` copies are already beside it.
    pub(crate) fn repair_declaration(
        &self,
        format: gx_engine::JournalFormat,
    ) -> Result<MetaRepair> {
        let path = self.root.join("VERSION");
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.write_declaration(
                    &path,
                    &declared_text(&[LAYOUT_VERSION.to_string()], &[], Some(format)),
                )?;
                return Ok(MetaRepair::Created);
            }
            Err(e) => return Err(io("read", &path)(e)),
        };
        if Layout::read_declaration(&path, &raw).is_ok() {
            return Ok(MetaRepair::Intact);
        }
        let kept = self.aside(&path)?;
        self.write_declaration(
            &path,
            &declared_text(&[LAYOUT_VERSION.to_string()], &[], Some(format)),
        )?;
        Ok(MetaRepair::Rewritten { kept })
    }

    /// 🔴 **R10 / `req/238` H-01** — the same road for `.gx/config.toml`.
    ///
    /// Only the **absent** shape is repaired: a `config.toml` that is there is the operator's file
    /// and gx has no business normalising it.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be written.
    pub(crate) fn repair_config(&self) -> Result<MetaRepair> {
        let path = self.root.join("config.toml");
        if path.exists() {
            return Ok(MetaRepair::Intact);
        }
        self.write_config(&path, &default_contents("config.toml"))?;
        Ok(MetaRepair::Created)
    }

    /// 🔴 **R12 (self-kill, this lane)** — the journal for a directory that has a declaration
    /// and has never recorded a commit.
    ///
    /// [`crate::layout::Layout::create`]'s road for the shape a restore leaves when it keeps the
    /// two small `Nature::Meta` files and drops the directories: `gx repair` answers that shape
    /// **exit 0, "nothing has been written to"** (`req/242` L-04), so a writer's door that refused
    /// it would be the two-doors-one-project failure again. The caller has already measured
    /// `Layout::logged`; this is the write.
    ///
    /// 🔴 **R13 / `req/244` H-02** — the settings for that same shape.
    ///
    /// [`crate::layout::Layout::create`]'s established branch used to refuse an absent
    /// `.gx/config.toml` unconditionally, while the journal one line below was decided by
    /// `Layout::logged`. `req/244` H-02 measured the two predicates disagreeing on one project:
    /// with both files gone, `gx submit` refused `CONFIG_ABSENT` and `gx repair --yes` never
    /// reached `repair_meta`, so nothing in gx could put either back. This is the init half —
    /// a directory that has never recorded a commit has no settings to lose — and it writes the
    /// same bytes [`Self::initialise`] does, from the same private function.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be written.
    pub(crate) fn ensure_settings(&self) -> Result<()> {
        let config = self.root.join("config.toml");
        if config.exists() {
            return Ok(());
        }
        self.write_config(&config, &default_contents("config.toml"))
    }

    /// # 🔴 **R13 / `req/244` M-03** — and it reads the declaration first.
    ///
    /// [`Self::create_journal`] writes `GXJRNL01` unconditionally, which is correct where the
    /// declaration is written in the same call ([`Self::initialise`]) and a guess everywhere else.
    /// `req/244` M-03 measured the guess: a project whose `.gx/VERSION` says
    /// `journal_format=legacy` — a shape audit 12 §6 confirmed pre-R12 binaries actually stamp, so
    /// a working tree can hold one — got a **chained** journal from one `gx submit`, at rc 0, after
    /// which `gx repair` answered `journal_format_declared: "legacy"`, `journal_intact: true`,
    /// `downgraded: false` and `remedy: null`. gx made a project whose declaration and whose file
    /// disagree, and called it healthy.
    ///
    /// So this road reads what the project declares and creates a journal in **that** framing. The
    /// doc comment on [`Self::initialise`] claims the framing "is declared here, where it is not a
    /// guess"; until R13 that sentence was true of `initialise` and false of this function, which
    /// is the same sentence R12's own §8 (2) is about — one project, two doors, two answers.
    ///
    /// # Why a legacy declaration is **refused** rather than obeyed
    ///
    /// A legacy journal is `[u32 length][payload]` with no marker, so an empty one is a file of
    /// zero bytes — and `gx_engine::replay` reads a file of zero bytes as **chained**, because "an
    /// empty project and a project with one record answer the same question the same way". There is
    /// therefore no byte sequence this function could write that a legacy declaration would be true
    /// of. Obeying the declaration is not expressible; guessing past it is what the audit measured.
    /// So the door refuses, and it refuses with a word it already has: `JOURNAL_ABSENT` says
    /// exactly this — the journal is not there, and a door that appends declined to invent one.
    ///
    /// # Errors
    /// [`Error::Io`] if the file cannot be created or the declaration cannot be read, and
    /// [`Error::JournalAbsent`] for a project that declares `journal_format=legacy` and has no
    /// journal.
    pub(crate) fn ensure_journal(&self) -> Result<()> {
        let journal = self.root.join("ledger").join("journal");
        // 🔴 **R40 / `req/38` §328 ruling 2 ①** — `is_present()` is the fold-free `exists()`, and
        // the `Undetermined` case falls through to `create_journal`, which asks the same question
        // and answers it the same way. Both are guards in front of a **write**, so the safe side is
        // "something may be there": a `stat` this process was not allowed to make must not be read
        // as permission to put eight fresh bytes over whatever is at that path.
        if !crate::layout::presence_of(&journal).is_absent() {
            return Ok(());
        }
        if Layout::declared_journal_format_at(&self.root)? == Some(gx_engine::JournalFormat::Legacy)
        {
            return Err(crate::layout::journal_absent_declared_legacy(
                &journal,
                &self.root.join("VERSION"),
            ));
        }
        self.create_journal()
    }

    // ---------------------------------------------------------------------------------------
    // The four private functions. Everything above is a decision; everything below is a write,
    // and `probes/doubt/tests/declaration_writer_doubt.rs` asserts that there are no others in
    // the workspace.
    // ---------------------------------------------------------------------------------------

    /// The only `std::fs` write of `.gx/VERSION` in this workspace.
    fn write_declaration(&self, path: &Path, text: &str) -> Result<()> {
        debug_assert!(path.starts_with(&self.root));
        std::fs::write(path, text).map_err(io("write", path))
    }

    /// The only `std::fs` write of `.gx/config.toml` in this workspace.
    fn write_config(&self, path: &Path, text: &str) -> Result<()> {
        debug_assert!(path.starts_with(&self.root));
        std::fs::write(path, text).map_err(io("write", path))
    }

    /// 🔴 **R12 / `req/242` H-01 (d)** — the only road that creates `.gx/ledger/journal`.
    ///
    /// `req/242` measured a project whose journal had been deleted: `gx repair --yes` refused to
    /// compose one (correctly — the ledger's leaves were built *from* those records), and then a
    /// single `gx submit` created an eight-byte `GXJRNL01` and the diagnosis about the loss
    /// disappeared. The engine's writer door is where that happened, so since R12 the CLI hands it
    /// [`gx_engine::JournalCreation::Refused`] on every road but this one, and the eight bytes are
    /// written here — in the same call that makes a directory into a project, beside the
    /// declaration that says `journal_format=chained` about them.
    fn create_journal(&self) -> Result<()> {
        let journal = self.root.join("ledger").join("journal");
        // 🔴 **R40** — see [`Self::ensure_journal`]: only an established absence licenses a write.
        if !crate::layout::presence_of(&journal).is_absent() {
            return Ok(());
        }
        if let Some(parent) = journal.parent() {
            std::fs::create_dir_all(parent).map_err(io("create", parent))?;
        }
        // 🔴 **R30 / `req/372` M-02** — the marker is the one **this project declares**, not the
        // one this build happens to write for new projects.
        //
        // The first draft of this repair stamped `GXJRNL02` unconditionally, and
        // `model_a_probes.rs`'s half-made-project arm measured what that costs: a `.gx/` declaring
        // `journal_format=chained` with no journal got a `GXJRNL02` journal from one `gx submit`,
        // so the project's declaration **under-stated its own vocabulary** from birth. That is
        // verbatim the shape `req/244` M-03 closed with `legacy` in place of `chained`, and unlike
        // the `legacy` case there **is** an obedient byte sequence available here.
        //
        // 🔴 It is not cosmetic. `EngineJournal`'s downgrade guard fires when the declared framing
        // outranks the file's, so a project that declares less than it holds can never trip it: a
        // later rewrite of that v2 file into a valid v1 chained file is a vocabulary downgrade
        // that a correctly-declared project catches and this one would not.
        //
        // `ensure_journal` above has already refused the one declaration this cannot obey
        // (`legacy`, which has no marker at all), so anything reaching here either names a chained
        // framing or names nothing — and naming nothing is a project this call is making now,
        // which gets this build's format.
        let declared = Layout::declared_journal_format_at(&self.root)?;
        let marker = declared
            .and_then(gx_engine::JournalFormat::marker)
            .unwrap_or(gx_engine::replay::JOURNAL_MAGIC_V2);
        // 🔴 **R33 / `req/442` §0-2 (a)** — the same eight bytes the engine writes, made durable
        // the same way.
        //
        // This was one `std::fs::write`, and it returned as soon as the call did. The engine's
        // writer door stamps the identical marker (`gx-engine/src/store.rs`, the
        // `write_all(marker)` beside `bytes.extend_from_slice(marker)`) and follows it with
        // `barrier(...)` — a `sync_all` — before it will treat those bytes as the file's framing;
        // R31 fixed that order there (write, barrier, then extend the in-memory copy) precisely so
        // a crash cannot leave the two disagreeing. The CLI's copy of the same act had no barrier
        // at all, so the one state the marker exists to rule out — a journal whose framing is not
        // on the device — was reachable from this line and not from the engine's.
        //
        // 🔴 What this does **not** fix, measured rather than assumed. `req/442` §0-2 (b) asked
        // whether the `journal.exists()` guard above freezes a zero-byte journal for good. This
        // lane drove the state through the shipped surface on the **unmodified** binary
        // (`tests/r33_zero_byte_journal_stamp.rs`, and `req/443` carries the numbers): a project
        // declaring `chained` whose journal holds no bytes is finished by the **engine's** writer
        // door on the next writer verb and answers `journal_intact: true`,
        // `journal_intact_basis: "chain"` afterwards. The guard declines a job one layer down
        // already does, so widening it was reverted — a second stamping road through
        // `Layout::create` for no observable change is the shape `req/242` H-01 spent three audits
        // removing.
        let write = || -> std::io::Result<()> {
            {
                use std::io::Write;
                // `truncate(false)` and a write at offset zero rather than `File::create`: this
                // road is reached only for a file the guard above found absent, and re-creating it
                // would be a truncation of a file another process may have written between the two
                // lines. Eight bytes over an empty file is the whole write either way.
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&journal)?;
                file.write_all(marker)?;
                file.sync_all()?;
            }
            // The directory entry of a file that has just been created. `gx-engine`'s
            // `sync_parent_directory` is the twin of this and carries the same declared gap:
            // elsewhere a directory has no handle to sync, so the *name* is as durable as the
            // platform makes it and no more (req/52 §5, A-5 — v0.1 CI is x86_64 Linux).
            #[cfg(unix)]
            {
                if let Some(parent) = journal.parent() {
                    std::fs::File::open(parent)?.sync_all()?;
                }
            }
            Ok(())
        };
        write().map_err(io("write", &journal))
    }

    /// 🔴 **R10** — move a file out of the way without deleting it, at the first free number.
    ///
    /// `<name>.pre-repair.<n>`, beside DR-43-7's `<file>.torn.<n>-<m>` and for its reason: a repair
    /// that destroyed what it repaired would leave an operator unable to tell "gx fixed it" from
    /// "gx overwrote the evidence".
    ///
    /// 🔴 **R12 / `req/242` L-01** — the refusal past the limit no longer calls
    /// `VERSION.pre-repair.0` "the oldest". `req/242` measured an operator moving `.0` out of `.gx/`
    /// by hand, exactly as the sentence advises, after which the next repair reuses the free `.0`
    /// and the file named "oldest" is the newest one there. The sentence now names the family and
    /// says where to look, which is the part that was true.
    fn aside(&self, path: &Path) -> Result<PathBuf> {
        let name = path
            .file_name()
            .map_or_else(|| "file".to_string(), |n| n.to_string_lossy().to_string());
        for n in 0u32..PRE_REPAIR_LIMIT {
            let candidate = self.root.join(format!("{name}.pre-repair.{n}"));
            if !candidate.exists() {
                std::fs::rename(path, &candidate).map_err(io("rename", path))?;
                return Ok(candidate);
            }
        }
        Err(Error::Usage {
            detail: format!(
                "{} already has {PRE_REPAIR_LIMIT} `.pre-repair.` copies beside it \
                 (`{name}.pre-repair.0` … `{name}.pre-repair.{last}`). gx does not remove one to \
                 make room — they are the bytes that were in this file before a repair replaced \
                 them (DR-43-7 (1)'s rule for `<file>.torn.<n>-<m>`, applied to this family) — so \
                 this run stopped and wrote nothing. Move them somewhere outside `.gx/` and run \
                 the repair again. The numbers are the order the copies were **taken in only until \
                 one of them is moved away**: the next repair reuses the free number, so read the \
                 modification times rather than the suffix if you need the sequence (`req/240` \
                 M-03, `req/242` L-01)",
                path.display(),
                last = PRE_REPAIR_LIMIT - 1,
            ),
        })
    }
}

/// 🔴 **R10 / `req/238` H-01** — `.gx/VERSION`'s text, composed from what a declaration **says**.
///
/// The one place the bytes of this file are built. `bare` are the lines that carry no `=` (the
/// layout version, first), `pairs` are the settings, and `add` is a `journal_format` to append when
/// the file does not already declare one. Everything that reaches here has been through
/// `gx_log::head::declaration_lines`, which is the same function `normalise_declaration` digests
/// over — so a file this function writes and the digest the head records cannot be two readings of
/// two byte strings.
///
/// 🔴 **R12** — private to this module. `req/242` H-01's third road wrote this file by calling this
/// function from `Layout::declare_journal_format`; a byte-composer that anything may call is a
/// write road that anything may take, and the census probe counts callers of this name.
fn declared_text(
    bare: &[String],
    pairs: &[(String, String)],
    add: Option<gx_engine::JournalFormat>,
) -> String {
    let mut lines: Vec<String> = bare.to_vec();
    lines.extend(pairs.iter().map(|(k, v)| format!("{k}={v}")));
    if let Some(format) = add {
        lines.push(format!("{JOURNAL_FORMAT_KEY}={}", format.kind()));
    }
    format!("{}\n", lines.join("\n"))
}

/// What a freshly created `Nature::Meta` file holds. Private to this module for
/// [`declared_text`]'s reason.
fn default_contents(rel: &str) -> String {
    match rel {
        "VERSION" => format!("{LAYOUT_VERSION}\n"),
        // req/56 §2: "project-local settings (secrets forbidden)", and §3 is why the file is empty
        // rather than pre-filled with a key path: "the secret key lives at `~/.gx/keys/` (user
        // home, 0600); the project side holds only a reference to the public keyid". A template
        // with a `key = ` line in it is an invitation to put one here, and the four tools req/11
        // batch 10 observed all keep credentials out of the project.
        "config.toml" => "# gx project-local settings (req/56 §2). No secrets: keys live in\n# ~/.gx/keys/ (req/56 §3), and this file may hold public key ids only.\n".to_string(),
        _ => String::new(),
    }
}
