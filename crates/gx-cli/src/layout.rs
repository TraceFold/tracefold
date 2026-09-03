// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `.gx/` — req/56's directory, and the seven paths req/88 §6.2 hand 1 ④ asks for. (sem: SEM-gx-cli-096)
//!
//! req/56 §1 is the whole requirement in one sentence: "**zero interference** with the user's
//! project code: all of gx's state is confined to `<project>/.gx/`, not entered into VCS by
//! default. Secrets are not placed in the project" (sem: SEM-gx-cli-097). §2 gives six
//! paths, §4 gives the commit boundary, §5 gives the recovery rules.
//!
//! # The seventh path
//!
//! `drafts/`, from **M6-01 adopted (a)** (req/38 §47): "put the intent body in `.gx/drafts/`
//! (the engine does not touch it -- managed by the CLI layer)" (sem: SEM-gx-cli-098). req/56 was written in M2 and the ruling is from M6, so the difference between
//! the two documents is the addition and nothing else —
//! `probes/doubt/tests/m6_surface_doubt.rs` parses both and asserts exactly that.
//!
//! # 🔴 What this module does **not** do to the ledger
//!
//! req/56 §5 writes three recovery rules — "dir absent = initialize / index corrupted =
//! regenerate / ledger corrupted = tail truncate (reusing hand 3's torn-write convention)" (sem: SEM-gx-cli-099) — and this module implements the first two and
//! **delegates** the third. The truncation of a torn append-only tail is `gx-log`'s and the engine's
//! (`EngineJournal::open`, `LedgerStore::open`, and the five torn shapes M5 hand 1 folded into one
//! answer); a second implementation in the CLI would be a second opinion about where a log ends.
//! [`Recovery::Delegated`] is what this module answers for that path, and answering it is the point:
//! "always declare what was lost and what was regenerated" (sem: SEM-gx-cli-100) (req/56 §5, the skip≠pass lineage of req/29 §4).

use std::path::{Path, PathBuf};

use crate::{io, Error, Result};

/// The layout version this binary writes into `.gx/VERSION`.
///
/// One, and the first thing it will ever have to say is that **E-M5-13 changed the journal record
/// shape** (`Planned` gained `locator` and `parents`). req/38 §47 M6-14 adopted (a) took that change (sem: SEM-gx-cli-101)
/// precisely because M6 is the hand that builds the first distributable and 47 §4 makes journal
/// compatibility an upgrade precondition: before shipping the change costs nothing, after shipping
/// it costs every user's journal. A directory stamped `1` is a directory written after that change.
pub const LAYOUT_VERSION: u32 = 1;

/// 🔴 **R6 / `req/229` H-02** — the key `.gx/VERSION` records the journal's framing under.
///
/// The file is a first line holding [`LAYOUT_VERSION`] and, from this release, zero or more
/// `key=value` lines after it. The layout **number** is unchanged — a project written by this
/// binary is still layout 1 and every path in [`GX_PATHS`] means what it meant — because what is
/// being recorded is not a new shape of directory but a fact about a file inside it.
///
/// The direction that breaks is the one `CHANGELOG.md` §3 already declares: a binary older than
/// this release parses the whole file as one number and will refuse a project that carries a second
/// line. New reads old; old does not read new.
pub const JOURNAL_FORMAT_KEY: &str = "journal_format";

/// 🔴 **R11 / `req/240` M-03** — how many `*.pre-repair.<n>` copies one file may accumulate.
///
/// R10 wrote `0..1000` and `req/240` M-03 named what that is: an undeclared family with no ceiling,
/// no report and no verb that lists it, whose thousandth member turns `gx repair --yes` itself into
/// a `Usage` refusal. Eight, because the number's job is to stop an accident from filling a
/// directory — an operator who has repaired an unreadable declaration eight times is not being
/// helped by a ninth copy — and because the refusal past it **names the oldest and removes
/// nothing**: no-delete is the rule these files exist to serve.
pub const PRE_REPAIR_LIMIT: u32 = 8;

/// Whether a path under `.gx/` is a directory or a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A directory `create` makes and `recover` re-makes.
    Dir,
    /// A file with contents.
    File,
    /// 🔴 **DR-43-7 (1)** — a *family* of files named by a rule rather than one path.
    ///
    /// One row, `ledger/*.torn.*`, and it exists because req/56 §2's table is a list of things a
    /// project directory may hold and this is one of them: `LedgerStore::open` and
    /// `EngineJournal::open` copy a torn tail to `<file>.torn.<replayed>-<total>` **before**
    /// removing it (`req/219` §2), so the name carries two byte counts and there is no fixed path
    /// to declare. `create` makes none of these and `recover` makes none either — see
    /// [`Recovery::Untouched`] for what it answers about them instead.
    Pattern,
}

/// req/56 §2's third column — "nature (⑤DB principle)" (sem: SEM-gx-cli-102) — which is what decides the recovery rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nature {
    /// "source of truth" / "source" (sem: SEM-gx-cli-103). Losing it loses data; nothing can regenerate it.
    Source,
    /// "derived, declared safe to delete" (sem: SEM-gx-cli-104). Regenerating it is always correct.
    Derived,
    /// Signed and derived at once (`checkpoints/`): re-derivable, but only by the holder of the
    /// ledger signing key. req/56 §2's cell is "re-signing required" (sem: SEM-gx-cli-105).
    Countersigned,
    /// Settings and metadata. Losing it is a reconfiguration, not a data loss.
    Meta,
    /// 🔴 **DR-43-5 (2)** — the state of a *running process*, carrying no data of its own.
    ///
    /// One row, `LOCK`, and this cell is the honest one rather than the convenient one. `Derived`
    /// would say "regenerating it is always correct", which is true of the file and false of what
    /// it is for: the exclusion is the operating system's, held on an open descriptor, and a
    /// second process creating the file does not acquire anything. `Meta` would say losing it is a
    /// reconfiguration; losing it is nothing at all, unless a `gx` is running, in which case the
    /// file is not lost. So the nature is the fact — it is not project state — and
    /// [`Recovery::Untouched`] is what recovery says about it.
    Transient,
}

/// One row of req/56 §2, as the implementation holds it.
#[derive(Clone, Copy, Debug)]
pub struct GxPath {
    /// The name under `.gx/`. Parsed out of this file by `m6_surface_doubt.rs` and compared with
    /// req/56 §2's first column.
    pub rel: &'static str,
    /// Directory or file.
    pub shape: Shape,
    /// What losing it means.
    pub nature: Nature,
}

/// 🔴 req/56 §2's six rows, **M6-01 adopted (a)**'s `drafts/` (sem: SEM-gx-cli-106), M6H2-1's
/// `receipts/`, and the two DR-43-5 (2) / DR-43-7 (1) add (`req/38` §156 ruling 3).
///
/// A declared list rather than a `read_dir`, for `SHIPPED_CRATE_ROOTS`'s reason: a list derived from
/// the tree cannot notice a tree that is wrong, and the recovery report below has to be able to say
/// "this was missing" (sem: SEM-gx-cli-107) about something.
pub const GX_PATHS: [GxPath; 11] = [
    GxPath {
        rel: "ledger",
        shape: Shape::Dir,
        nature: Nature::Source,
    },
    GxPath {
        rel: "checkpoints",
        shape: Shape::Dir,
        nature: Nature::Countersigned,
    },
    GxPath {
        rel: "evidence",
        shape: Shape::Dir,
        nature: Nature::Source,
    },
    GxPath {
        rel: "index",
        shape: Shape::Dir,
        nature: Nature::Derived,
    },
    // **M6-01 adopted (a)**, the seventh (sem: SEM-gx-cli-108). The intent body between `gx submit` and `gx plan`, which are two
    // processes: 42 §1.3-3 keeps the state table on the engine side and E-M5-3 keeps the draft in
    // the journal as an `IntentId` with no body, so there is nowhere else for it to be.
    //
    // Its nature is `Source` and that is the honest cell rather than a comfortable one: nothing
    // regenerates a draft. The consequence — that this directory is CLI-side state the HTTP surface
    // does not have — is req/88 Λ2's one counter-example and is written in the crate root.
    GxPath {
        rel: "drafts",
        shape: Shape::Dir,
        nature: Nature::Source,
    },
    // 🔴 **M6H2-1**, the eighth, and hand 2's own addition rather than a ruled one.
    //
    // 44 §1.2: "`show`: fetch and display a `Receipt` from the local store/gx-api" (sem: SEM-gx-cli-109). There was no local store.
    // `Engine::receipt` reads an in-memory table `Engine::open` leaves empty on purpose (M5H3-5);
    // the journal's thirteen record kinds hold no receipt (42 §3.13); and 42 §3.11 keeps the body
    // out of the ledger leaf, which carries a **digest** — "put the receipt body outside the leaf" (sem: SEM-gx-cli-110). So a
    // second `gx` process has nowhere to read one from, `gx receipt show` cannot be implemented,
    // and M6-16's staged disclosure (§47 adopted (a); sem: SEM-gx-cli-111) — which M6-22 hangs on — has no subject.
    //
    // Its nature is `Source` and that cell is the honest one rather than the flattering one. A
    // receipt is signed, so `Countersigned` looks apt; but "re-signing required" (sem: SEM-gx-cli-112) says a holder of the key can
    // re-derive it, and nothing here can. Re-issuing needs the verdict summary, the proof digest
    // and both fingerprints, and those live in the table `open` does not rebuild. Losing this
    // directory loses receipts.
    GxPath {
        rel: "receipts",
        shape: Shape::Dir,
        nature: Nature::Source,
    },
    GxPath {
        rel: "config.toml",
        shape: Shape::File,
        nature: Nature::Meta,
    },
    GxPath {
        rel: "VERSION",
        shape: Shape::File,
        nature: Nature::Meta,
    },
    // 🔴 **DR-43-5 (2)**, the ninth (`req/38` §156 ruling 3, `req/219` §9).
    //
    // DR-43-2 created this file and did not declare it, and said so where a reader would meet it
    // (`gx_cli::session::LOCK_FILE`: "it is not in `GX_PATHS`, and that is owed rather than
    // hidden"). The reason was that req/56 §2 and `m6_surface_doubt` were outside R1's write
    // scope, so a one-sided addition would have turned a three-way cross-check red for a reason
    // that had nothing to do with locking. All three sides move here, together.
    //
    // What the declaration buys: `Layout::recover` now has a row for it, so `.gx/` can be walked
    // without a file in it being a surprise; and req/56 §2 carries the sentence that says nothing
    // reads it for meaning.
    GxPath {
        rel: "LOCK",
        shape: Shape::File,
        nature: Nature::Transient,
    },
    // 🔴 **R13 / `req/244` H-01**, the eleventh.
    //
    // Where `gx repair --yes` puts a copy of the report it just printed. The audit measured the
    // reason: `println!` panics on a write error, so a run that had written `.gx/VERSION` ended at
    // exit **101** with a Rust panic string and **no report anywhere** — and the next `gx repair`
    // answered `meta_repaired: []` about the file gx had just created. `Outcome::emit` closes the
    // delivery; this closes the other half, which is that a report only ever existed on a stream.
    //
    // Its nature is `Source` and that is the honest cell rather than the comfortable one. `Derived`
    // says "regenerating it is always correct" and nothing regenerates a record of a past run.
    // What it is **not** is project state: it witnesses no commit, so it is absent from
    // [`Layout::logged`], and it does not make a directory a project, so it is absent from
    // [`Layout::established`] — a directory a repair has run in must not start looking like one
    // that has been committed to.
    GxPath {
        rel: "repair",
        shape: Shape::Dir,
        nature: Nature::Source,
    },
    // 🔴 **DR-43-7 (1)**, the tenth (`req/38` §156 ruling 3, `req/219` §9).
    //
    // R1b made a writer copy a torn tail to `<file>.torn.<replayed>-<total>` before removing it,
    // and left the lifetime of those copies to a later lane: "they are created and never removed;
    // `Layout::recover` and `GX_PATHS` know nothing about them". This is the row. It does **not**
    // decide the lifetime — nothing here deletes one, and a `gx` verb that swept them away would
    // be a verb that destroys evidence — it decides that they are *declared*, which is what makes
    // "`.gx/` holds exactly what req/56 §2 says" a statement that survives a crash.
    GxPath {
        rel: "ledger/*.torn.*",
        shape: Shape::Pattern,
        nature: Nature::Source,
    },
];

/// 🔴 **R15 / `req/259` M-01** — the `Shape::Dir` rows of [`GX_PATHS`], by name.
///
/// The one reading of "which directories does req/56 §2 declare", so that the door that **refuses**
/// a blocked one ([`Layout::create`]'s pre-scan), the verb that **clears** one
/// (`repair::repair_dir_state`) and the report that **counts** what was set aside
/// ([`Layout::kept_aside`]) are three readers of one table rather than three lists that agree until
/// somebody adds a row. R14 generalised the first and left the other two at `"repair"`, which is
/// what `req/259` M-01 measured: six of the seven refused with no way out, and the remedy told the
/// operator about a rename that never happened.
pub fn declared_directories() -> impl Iterator<Item = &'static str> {
    GX_PATHS
        .iter()
        .filter(|p| p.shape == Shape::Dir)
        .map(|p| p.rel)
}

/// What happened to one path when [`Layout::recover`] looked at it.
///
/// req/56 §5's reporting requirement is the reason this is an enum and not a `bool`: "**always
/// declare what was lost and what was regenerated**" (sem: SEM-gx-cli-113). `Intact` and `Regenerated` are both "it is there now" and they are
/// not the same fact, which is req/29 §4's "do not give skip and pass the same face" one directory down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// It was there and was left alone.
    Intact,
    /// It was absent and has been created empty. req/56 §5: "dir absent = initialize" (sem: SEM-gx-cli-114).
    Initialised,
    /// It was absent or unreadable, and rebuilding it is always correct because it is derived.
    /// req/56 §5: "index corrupted = regenerate" (sem: SEM-gx-cli-115).
    Regenerated,
    /// 🔴 It was absent and **nothing here can replace it**. The directory is usable again and the
    /// contents are gone; saying so is the requirement.
    Lost,
    /// 🔴 **DR-43-5 (2) / DR-43-7 (1)** — it is declared and it is **not project state**, so
    /// recovery neither creates it nor calls it lost.
    ///
    /// Two rows answer this and for two different reasons, which is why the variant says what it
    /// does rather than "skipped": `LOCK` is a running process's exclusion (creating one repairs
    /// nothing and an absent one is the ordinary state of a project nobody is writing to), and
    /// `ledger/*.torn.*` is **evidence** — bytes that would not replay, copied out before a writer
    /// removed them. Regenerating evidence is a contradiction; the honest report is that the row
    /// exists, that this function did not touch it, and that what is or is not there is what a
    /// crash left. An operator reading `Untouched` is being told to look, not told it is fine.
    Untouched,
    /// 🔴 It is another layer's to repair. The append-only tail rule is `gx-log`'s and the engine's
    /// (req/56 §5's third rule, "reusing hand 3's torn-write convention" (sem: SEM-gx-cli-116)), and a second implementation of it
    /// here would be a second opinion about where a log ends.
    Delegated,
}

/// The report [`Layout::recover`] returns: one row per declared path, in `GX_PATHS` order.
#[derive(Clone, Debug)]
pub struct RecoveryReport {
    rows: Vec<(&'static str, Recovery)>,
}

impl RecoveryReport {
    /// Every row, in declaration order.
    #[must_use]
    pub fn rows(&self) -> &[(&'static str, Recovery)] {
        &self.rows
    }

    /// What happened to one path.
    #[must_use]
    pub fn of(&self, rel: &str) -> Option<Recovery> {
        self.rows.iter().find(|(r, _)| *r == rel).map(|(_, k)| *k)
    }

    /// The rows whose outcome was not [`Recovery::Intact`].
    ///
    /// The declaration req/56 §5 asks an operator to be shown. An empty list means "nothing was
    /// missing" (sem: SEM-gx-cli-117) and is a different sentence from "nothing was checked", which is why the caller
    /// gets rows rather than a count.
    ///
    /// 🔴 [`Recovery::Untouched`] is excluded as well (`req/38` §156 ruling 3). req/56 §5's
    /// sentence is "always declare **what was lost and what was regenerated**", and about `LOCK`
    /// and `ledger/*.torn.*` this function did neither — a row for something recovery is not
    /// entitled to touch would appear in every report of every healthy project and would train an
    /// operator to skip the list. The rows are still in [`RecoveryReport::rows`] and still
    /// answerable by [`RecoveryReport::of`], which is where a reader asking about them looks.
    #[must_use]
    pub fn changed(&self) -> Vec<(&'static str, Recovery)> {
        self.rows
            .iter()
            .copied()
            .filter(|(_, k)| !matches!(k, Recovery::Intact | Recovery::Untouched))
            .collect()
    }
}

/// An opened `.gx/` directory.
#[derive(Clone, Debug)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    /// Where `.gx/` sits for a project rooted at `project`.
    #[must_use]
    pub fn path_for(project: &Path) -> PathBuf {
        project.join(".gx")
    }

    /// Create the directory and everything req/56 §2 declares, and stamp the version.
    ///
    /// Idempotent: running it on a complete directory changes nothing, which is what lets every
    /// command call it rather than making "did you run `gx init`" (sem: SEM-gx-cli-118) a thing a user can get wrong.
    ///
    /// # 🔴 **R10 / `req/238` H-01** — idempotent on a *complete* directory, and refusing on an
    /// incomplete one
    ///
    /// The sentence above used to be the whole story and `req/238` H-01 measured what it cost. This
    /// function's `Shape::File` arm was `if !full.exists() { write(default_contents(rel)) }`, and
    /// the two files it names are `VERSION` — whose digest R7 bound into the signed head so that a
    /// *rewritten* declaration is caught — and `config.toml`, which 43 §7.9 (b) calls the file that
    /// decides the recovery key. So a project that **lost** either of them was answered by the next
    /// `gx submit` writing a fresh one, in silence, at rc 0: `ledger_agrees_before` went `false` →
    /// `true`, `head_authenticity` went back to `verified`, `remedy` back to `null`, and
    /// `engine_signing_keyid` back to the shipping default. R7's detector disarmed itself.
    ///
    /// **Deletion is not a weaker attack than rewriting; it is a stronger one** — a rewrite is
    /// caught and stays caught, and a deletion used to be erased by the next writer.
    ///
    /// So this function keeps its `gx init` standing for a directory that is **not yet a project** (no
    /// `VERSION`, no journal, no recorded head: a fresh `gx submit`, `gx demo`, a test fixture) and
    /// refuses to re-create a [`Nature::Meta`] file in one that **is**. The road that writes those
    /// files back into an established project is `gx repair --yes`, which says what it did
    /// ([`Layout::repair_declaration`], [`Layout::repair_config`]).
    ///
    /// # Errors
    /// [`Error::Io`] if any directory or file cannot be created.
    /// [`Error::DeclarationAbsent`]/[`Error::ConfigAbsent`] if this is an established project whose
    /// `VERSION`/`config.toml` is gone — the silent re-creation `req/238` H-01 measured.
    pub fn create(project: &Path) -> Result<Self> {
        let root = Self::path_for(project);
        // 🔴 **R10** — measured **before** anything is created, because `create_dir_all` below and
        // the `Shape::Dir` arm both change the answer to "has this directory ever been a project".
        // 🔴 **R15 / `req/259` M-01** — the **shape** of every declared directory is asked first,
        // before this function asks whether the project exists.
        //
        // 43 §7.16 (d) is "a declared surface is asked its shape before it is asked whether it is
        // there", and R14 put the scan in front of the first `create_dir_all` — which was in front
        // of every *write* and behind every *read*. `req/259` M-01 measured the difference on
        // `.gx/ledger`: one byte there is a path that is not a directory, and what came back was
        // **`HISTORY_LOST`** from the block below, because `root.join("ledger").join("journal")`
        // does not exist when `ledger` is a file. That refusal is about a project whose log is
        // gone; this one is about a path an operator, a backup or a `tar` put a file at. Naming
        // the second as the first sends them to a restore they may not need — `req/244` H-03's
        // standing lesson about asserting causes — and it is the one row of the seven with no exit
        // for exactly that reason. The predicate goes above the existence questions, where it is
        // true of all seven.
        Self::declared_directories_are_directories(&root)?;
        let established = Self::established(&root);
        // 🔴 **R13 / `req/244` L-04** — a project that has recorded commits and lost its journal is
        // refused **here**, before this function makes a single directory.
        //
        // `session::open_engine_wired_accepting` already refuses it (`req/242` H-01 (d)), and
        // `req/244` L-04 measured what stood in front of that refusal: `gx submit` on a project
        // whose `.gx/ledger/` had been deleted answered rc 1 `JOURNAL_ABSENT` and left an empty
        // `.gx/ledger/` behind — absent before the run, present after it. Two writers made it, the
        // session's `create_dir_all` (gone in R13) and this loop, and the census counts neither
        // because `create_dir_all` is not a byte on a disk. It is still a road that refuses and
        // writes, which is the sentence the whole of `req/242` H-01 is about.
        //
        // The predicate is the one the rest of this function uses, in the same words (43 §7.14
        // (a)): `established` says this is a project, `logged` says it has recorded a commit, and a
        // project that has done both and holds no journal has lost it. A directory that has never
        // recorded a commit takes the init road below and gets a journal made for it, which is
        // `Layout::create`'s job and not a loss.
        // 🔴 **R14 / `req/246` L-01** — and so is every **other** refusal this function makes.
        //
        // 43 §7.15 (f) wrote the rule as a sentence — "a road that refuses does not create a
        // directory" — and R13 closed the one road `req/244` L-04 had measured (`JOURNAL_ABSENT`).
        // `req/246` L-01 measured the other three, one run each, on a project missing two declared
        // directories: `DECLARATION_ABSENT`, `DECLARATION_UNREADABLE` and `CONFIG_ABSENT` each left
        // `.gx/evidence/` and `.gx/repair/` behind them, because they were asked **after** the
        // `Shape::Dir` loop. A rule closed at one of its four sites is a rule closed at a place
        // rather than as a count (`req/38` §181 ruling 3), so all four stand here now, in front of
        // the first `create_dir_all` this function reaches.
        //
        // What could not move up with them: `DeclarationWriter::for_init`'s three writes
        // (`initialise`, `ensure_settings`, `ensure_journal`). Those are the roads that **succeed**,
        // they need the directories to exist, and they are below the loop for that reason.
        let logged = established && Self::logged(&root);
        if established {
            let journal = root.join("ledger").join("journal");
            // 🔴 **R40 / `req/38` §328 ruling 2 ①②③** — the door asks [`presence_of`] once and
            // branches on all three answers, where it used to ask `!journal.exists()` and have two.
            //
            // The order is load-bearing. The **shape** question comes first and is not conditioned
            // on `logged`: a directory standing where the journal belongs is a blocked layout
            // whether or not this project has ever recorded a commit, and audit 39 reached it on a
            // project that had. `Undetermined` comes next and refuses rather than falling through,
            // because every answer below this line reads "the journal is not there" and that
            // sentence is exactly what a `stat` this process was not allowed to make cannot
            // support. `Absent` keeps R12's refusal, unchanged and now true whenever it is said.
            match presence_of(&journal) {
                Presence::Present(found) if !found.is_file() => {
                    return Err(journal_blocked(
                        &journal,
                        // The declared row this path lives in (req/56 §2's `ledger`). See `journal_blocked`
                        // for why the row name is passed rather than written there.
                        "ledger",
                        if found.is_symlink() {
                            "a symbolic link"
                        } else if found.is_dir() {
                            "a directory"
                        } else {
                            "not a regular file"
                        },
                    ));
                }
                Presence::Undetermined(source) => {
                    return Err(Error::Io {
                        action: "read the shape of",
                        path: journal.display().to_string(),
                        source,
                    });
                }
                Presence::Absent if logged => return Err(journal_absent(&journal)),
                Presence::Absent | Presence::Present(_) => {}
            }
            let version_path = root.join("VERSION");
            let raw = Self::read_version(&root, &version_path)?;
            Self::read_declaration(&version_path, &raw)?;
            let config = root.join("config.toml");
            if logged && !config.exists() {
                return Err(config_absent(&config));
            }
            // 🔴 **R40** — the same predicate, and the reason it is `is_absent` rather than
            // `!is_present`: `HISTORY_LOST` says "this project holds **no witness** of any commit
            // it recorded", and a journal this process could not `stat` is not a journal anybody
            // has established is missing. `Undetermined` never reaches here — the match above
            // refuses it — so this line is `Absent` only by construction as well as by spelling.
            if presence_of(&journal).is_absent() && !logged {
                if let Some(evidence) = Self::used_without_witness(&root) {
                    return Err(history_lost(&root, &evidence));
                }
            }
        }
        // (moved to the top of this function by R15 — see the block above the `established` read.)
        //
        // 🔴 **R14 / `req/246` M-04** — every declared directory is checked for its **shape**
        // before any of them is made.
        //
        // R13 gave `.gx/repair/` a `Shape::Dir` row and the loop below started asking the operating
        // system for it. `req/246` M-04 put one byte at that path and measured what came back:
        // `gx submit`, `gx log head` and `gx receipt list` all refused **`INTERNAL`** — 44 §2.3's
        // word for "cannot be classified" — with "create …/.gx/repair: File exists (os error 17)",
        // three runs each, and `gx repair` called the project healthy at exit 0. The state is
        // entirely classifiable: a path that is not a directory is sitting where a declared
        // directory belongs. That is the **predicate**, and it is deliberately not the place — the
        // same class was measured at `.gx/ledger/journal.blobs/` two audits earlier (`req/244`
        // M-06, still open inside the engine), and closing it at one path would be closing it at a
        // place again.
        //
        // The scan is a pass of its own so that this refusal, like the four above, creates nothing:
        // a loop that made six directories and then refused at the seventh would be `req/246` L-01
        // one release after it was closed.
        std::fs::create_dir_all(&root).map_err(io("create", &root))?;
        for path in GX_PATHS {
            // 🔴 **DR-43-5 (2) / DR-43-7 (1)** — two of the ten rows are declared and are not
            // created. A `LOCK` written by `gx init` would be a file that looks like an exclusion
            // and holds none (the exclusion is a descriptor's, and `ProcessLock::open` makes the
            // file when a writer actually needs it); a path named `ledger/*.torn.*` is a rule and
            // not a name. `Shape::Pattern` and `Nature::Transient` are what say so.
            if path.shape == Shape::Pattern || path.nature == Nature::Transient {
                continue;
            }
            // 🔴 **R12 / `req/242` H-01** — the file arm is gone from this loop.
            //
            // It used to be `if !full.exists() { write(default_contents(rel)) }` behind an
            // `established` gate, which made this function one of the workspace's four roads into
            // `.gx/VERSION`. The files a *new* project needs are written by
            // [`crate::declaration::DeclarationWriter::initialise`] below, in one place; this loop
            // makes directories.
            if path.shape == Shape::Dir {
                let full = root.join(path.rel);
                std::fs::create_dir_all(&full).map_err(io("create", &full))?;
            }
        }
        if established {
            // 🔴 **R12 / `req/242` H-01 (a)** — 43 §7.14: the door that creates asks the question
            // the door that opens asks, in the same words.
            //
            // R10 taught this function to refuse an established project with a **missing**
            // `Nature::Meta` file and left "present and unreadable" to [`Layout::open`]. But
            // `gx submit` is the one verb that comes through `create` (44 has no `gx init`), so a
            // `.gx/VERSION` an editor had re-saved as UTF-16 LE walked past here and met
            // `declare_journal_format`, which rewrote it — 50 bytes to 74, three runs, no
            // difference (`req/242` H-01 (a)). Both refusals are one predicate now:
            // [`Self::read_version`] for "not there" and [`Self::read_declaration`] for "does not
            // read", which is the pair [`Layout::open`] uses two functions below.
            // 🔴 **R14 / `req/246` L-01** — the two reads above moved to the top of this function,
            // in front of the first directory it makes, and the paragraph above is kept where it
            // was written because it is the reason they exist at all.
            // 🔴 **R13 / `req/244` H-02** — one question, one predicate, both files.
            //
            // R12 asked "is this the init road?" twice inside these four lines and answered it
            // with two different predicates: `config.toml` was judged by `established` (via this
            // branch) and the journal by [`Self::logged`]. `req/244` H-02 measured the gap, and it
            // is a project with **no way out of gx**: delete `.gx/config.toml` and
            // `.gx/ledger/journal` together and `gx submit` refuses `CONFIG_ABSENT` forever, while
            // `gx repair --yes` returns from `run_the_repair`'s journal-absent branch without ever
            // reaching `repair_meta`, so it creates no `config.toml` and its remedy does not carry
            // the word. Two forms, three runs each, no variation. In the form that has never
            // recorded a commit, `gx repair` answered **exit 0** — 44 §1.2's number for "this
            // project can be written to" — about a project no verb could write to.
            //
            // `logged` is the predicate, for both files. A project that **has** recorded a commit
            // and has lost its settings keeps R10's refusal, and keeps it for R10's reason: the
            // file names the key a recovery signs with (43 §7.9 (b)), and writing the shipped
            // default over it puts `engine_signing_keyid` back to nothing under an operator who
            // set it. A project that has **not** is the `gx init` road, where there is no setting
            // to lose — which is the same argument [`crate::declaration::DeclarationWriter::for_init`]
            // already stands on for the journal one line below.
            //
            // The other half of H-02 — the exit for a project that *has* committed — is
            // `gx repair --yes`'s, and it is in `repair::repair_and_report`.
            //
            // 🔴 **R14 / `req/246` L-01** — the `CONFIG_ABSENT` half of this has moved to the top of
            // the function with the other three refusals; `logged` is measured once, up there, and
            // what is left here is the road that **writes**.
            let config = root.join("config.toml");
            if !config.exists() {
                crate::declaration::DeclarationWriter::for_init(&root).ensure_settings()?;
            }
            // 🔴 **R12 (self-kill, this lane)** — a declaration and nothing else is still a
            // directory nothing has been written to.
            //
            // Measured on this lane's own binary before it was written: a `.gx/` holding
            // `VERSION` and `config.toml` and no `ledger/`, no `checkpoints/` and no `receipts/`
            // — the shape a restore leaves when it keeps the two small files and drops the
            // directories, and the shape `req/242` L-04 measured `gx repair` answering **exit 0,
            // "nothing has been written to"** about — was refused `JOURNAL_ABSENT` by
            // `gx submit`. Two doors, two answers, on one project: the exact failure the last
            // three audits have each found once.
            //
            // [`Self::established`] cannot make this call — its three witnesses include `VERSION`
            // itself, which is the file that is there. The question here is the narrower one
            // `report_without_engine` already asks before it decides whether a missing journal is
            // a loss: **has this project ever recorded a commit.** If it has not, this is the
            // `gx init` road and the journal is created with the rest; if it has, the journal is
            // gone and `JOURNAL_ABSENT` is the answer (`req/242` H-01 (d)).
            // 🔴 **R40 / `req/38` §328 ruling 2 ①** — `is_absent`, not `!exists`. Same reason as
            // the `HISTORY_LOST` guard in [`Self::open`]: this branch decides that a journal is
            // **gone**, and only a `stat` that came back `NotFound` establishes that.
            if presence_of(&root.join("ledger").join("journal")).is_absent() && !logged {
                // 🔴 **R13 / `req/244` M-04** — and this project has to look like one nothing has
                // been written to, rather than merely like one with no witness left.
                //
                // `logged` counts what a **commit** leaves: the ledger beside the journal, the
                // recorded head, the commit receipts. A project that has lost all three is
                // byte-for-byte what `gx key gen` leaves in a fresh directory, and R12 treated it
                // as one — so `gx submit` wrote a new journal over a project that had committed
                // twice, and `gx repair` afterwards said `journal_commits: 0`,
                // `head_authenticity: "absent"`, `remedy: null`. Two commits, and nothing anywhere
                // that says they happened.
                //
                // `.gx/index/`, `.gx/evidence/` and `.gx/drafts/` are the difference. None of them
                // witnesses a commit — which is exactly why `logged` does not count them, and that
                // judgement is unchanged — and all of them witness **use**: `index/` holds the
                // resolutions `gx plan` files, `drafts/` holds intent bodies `gx submit` wrote,
                // `evidence/` holds what a verify collected. Entries in one of them with none of
                // the three witnesses is a project whose log has gone, and the honest answer is to
                // say so rather than to start a second history.
                //
                // Refused rather than warned, because the alternative is not recoverable: once the
                // new journal is written, the fact that the old one existed is nowhere. There is no
                // `--yes` road either — a repair that invented a history would be worse than the
                // loss — and the remedy is the backup. A directory that really is fresh has no
                // entries in any of the three and is unaffected.
                //
                // 🔴 **R14 / `req/246` L-01 + M-02** — the refusal moved to the top of this
                // function (a road that refuses creates nothing), and the predicate it asks —
                // [`Self::used_without_witness`] — became `pub(crate)` so that
                // `repair::report_without_engine` asks the **same** one. `req/246` M-02 measured
                // the two doors disagreeing about this exact project: refused here by name, called
                // "a project nothing has been written to" at exit 0 by the report.
                crate::declaration::DeclarationWriter::for_init(&root).ensure_journal()?;
            }
        } else {
            // 🔴 **R12** — the one road that turns a directory into a project, and the only
            // place in this binary that writes any of the files it writes.
            crate::declaration::DeclarationWriter::for_init(&root).initialise()?;
        }
        Ok(Self { root })
    }

    /// Open an existing directory and check its version.
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/VERSION` cannot be read. [`Error::Malformed`] if it is not a number.
    /// [`Error::Layout`] if it names a version newer than [`LAYOUT_VERSION`] — fail-closed, because
    /// a newer directory may hold a journal shape this binary would misread (47 §4).
    pub fn open(project: &Path) -> Result<Self> {
        let root = Self::path_for(project);
        // 🔴 **R16 / `req/262` M-01** — the reading door asks the same question the writing door
        // asks, and asks it first.
        //
        // `Layout::create` has carried the shape scan since R14 and moved it above the existence
        // questions in R15. **This** function had none, and it is the one `gx serve` opens a
        // project with (`crates/gx-cli/src/serve.rs`). The sixteenth audit measured the gap on a
        // project whose `.gx/receipts` was a one-byte file: every CLI verb refused it with exit 1
        // `LAYOUT_BLOCKED`, and `gx serve` **started** on the same project. Driving the HTTP commit
        // road through that server then gave `201` → `200` → `500 INTERNAL`, with the target file
        // already rewritten, one leaf on the ledger, zero commits in the journal, the commit
        // receipt missing and `gx undo` refusing — the effect applied and the receipt that proves
        // it lost, which is the one composition 43 §7-3b exists to keep out of a project.
        //
        // Refusing to start rather than starting degraded, and the comparison is written down
        // because the other reading is defensible: a server that answered `LAYOUT_BLOCKED` to every
        // **write** endpoint and went on serving reads would keep a deployment's read traffic
        // alive. It is refused because there is no read to protect that the same project's CLI is
        // not already refusing — `gx log`, `gx receipt` and `gx replay` all take this door too —
        // and because a server that starts is a server an operator believes in. The one project
        // this makes stricter is one no verb of this binary would have written to anyway, and the
        // way out is the same one the refusal prints. 44 §2 gains the sentence rather than the
        // status: this is a start-up refusal, before there is a socket to answer on.
        Self::declared_directories_are_directories(&root)?;
        // 🔴 **R40 / `req/553` M-01, `req/38` §328 ruling 2 ①②** — and the declared **file** whose
        // shape is knowable is asked about here too, for R16's reason two paragraphs up.
        //
        // R16 put the directory scan on this door because `gx serve` started on a project every CLI
        // verb refused. R40 measured the same asymmetry one path over and the other way round:
        // replace `.gx/ledger/journal` with a **directory** and `gx submit` refused
        // `LAYOUT_BLOCKED` from `Layout::create` while `gx log proof`, `gx log consistency` and `gx
        // log checkpoint` walked straight past this door — nothing here asked about the journal at
        // all — and answered from the ledger, `checkpoint` with a signature. One project, two
        // doors, two answers, which is the sentence `req/242` H-01 (d) and R16 are both about.
        //
        // What is asked here is the **shape** and the **stat**, and deliberately not the presence.
        // A project with no journal is the third-party verifier `ledger::refuse_if_the_two_files_
        // disagree` exists to keep answering (`req/540` R-1b, and R40's AC-3): asking
        // `JOURNAL_ABSENT` on the reading door would refuse the one caller the escape hatch is for.
        // A journal that is a directory, and a journal this process cannot `stat`, are neither of
        // them that caller.
        let journal = root.join("ledger").join("journal");
        match presence_of(&journal) {
            Presence::Present(found) if !found.is_file() => {
                return Err(journal_blocked(
                    &journal,
                    // The declared row this path lives in (req/56 §2's `ledger`). See `journal_blocked`
                    // for why the row name is passed rather than written there.
                    "ledger",
                    if found.is_symlink() {
                        "a symbolic link"
                    } else if found.is_dir() {
                        "a directory"
                    } else {
                        "not a regular file"
                    },
                ));
            }
            Presence::Undetermined(source) => {
                return Err(Error::Io {
                    action: "read the shape of",
                    path: journal.display().to_string(),
                    source,
                });
            }
            Presence::Absent | Presence::Present(_) => {}
        }
        let version_path = root.join("VERSION");
        let raw = Self::read_version(&root, &version_path)?;
        // 🔴 **R6 / `req/229` H-02** — the **first line** is the version and the rest are
        // `key=value`. Before this release the whole file was the number, and a project had nowhere
        // to record what it is — which is precisely what let a chained journal be downgraded to
        // legacy with nothing anywhere contradicting it.
        //
        // 🔴 **R9 / `req/236` H-04** — and the read goes through the **declaration**, not through
        // the bytes. See [`Layout::read_declaration`].
        let parsed = Self::read_declaration(&version_path, &raw)?;
        if parsed > LAYOUT_VERSION {
            return Err(Error::Layout {
                path: version_path.display().to_string(),
                found: parsed.to_string(),
                expected: LAYOUT_VERSION,
            });
        }
        Ok(Self { root })
    }

    /// 🔴 **R15 / `req/259` M-01 + R16 / `req/262` M-01** — every directory req/56 §2 declares is a
    /// directory, asked before anything else and asked by every door.
    ///
    /// The predicate R14 wrote and R15 moved above the existence questions, lifted out of
    /// [`Layout::create`] so that [`Layout::open`] asks it too. One table
    /// ([`declared_directories`]), one question, and now three readers of the refusal side rather
    /// than one: the writing door, the reading door, and `repair::repair_dir_state`'s exit.
    ///
    /// `symlink_metadata` rather than `metadata`, so that the **final component** is not followed:
    /// a symbolic link where a directory is declared is a different shape even when it points at a
    /// real directory somewhere else, and `repair --yes` sets it aside rather than writing through
    /// it (`req/262` L-01). A `.gx/` that is *itself* a symlink is unaffected, because only the
    /// last component of each declared path is examined.
    ///
    /// 🔴 **R43 / `req/578` §6, ruling `req/38` §350 item 5 (addendum S-9)** — and it asks
    /// [`presence_of`], not `if let Ok`.
    ///
    /// `if let Ok(found)` had one arm and three cases behind it: absent (the ordinary state — the
    /// next writer makes the directory), a shape that is not a directory (the refusal this
    /// function exists for), and a `stat` this process could not make. The third fell through the
    /// `Ok` guard with the first, so **the door that is asked first** — R16 put it here so `gx
    /// serve` would stop starting on projects every CLI verb refused — passed rows it had not
    /// looked at.
    ///
    /// No word is minted for the third case. It answers the one this file already gives one
    /// function down for the journal's shape ([`Layout::open`]'s `Presence::Undetermined` arm):
    /// `Error::Io { action: "read the shape of", .. }`, naming the declared path. Measured
    /// (`crates/gx-cli/tests/r43_presence_and_head.rs` bed-N): under an unreadable `.gx/` this
    /// check passed all seven rows silently and the refusal came from the journal check further
    /// in, about a different path.
    ///
    /// # Errors
    /// [`Error::LayoutBlocked`] naming the path, what is there and the way out. [`Error::Io`] for a
    /// declared path whose shape this process could not read.
    fn declared_directories_are_directories(root: &Path) -> Result<()> {
        for rel in declared_directories() {
            let full = root.join(rel);
            match presence_of(&full) {
                Presence::Present(found) if !found.is_dir() => {
                    return Err(layout_blocked(&full, rel));
                }
                Presence::Undetermined(source) => {
                    return Err(Error::Io {
                        action: "read the shape of",
                        path: full.display().to_string(),
                        source,
                    });
                }
                Presence::Absent | Presence::Present(_) => {}
            }
        }
        Ok(())
    }

    /// 🔴 **R10 / `req/238` H-01** — has this directory ever been a gx project?
    ///
    /// The question [`Layout::create`] and [`Layout::open_reporting`] both have to answer before
    /// they can decide what an absent [`Nature::Meta`] file **means**. Three witnesses, any one of
    /// which is enough, and every one of them is written by gx and by nothing else:
    ///
    /// * `VERSION` — the declaration itself;
    /// * `ledger/journal` — the append-only log the writer's door creates;
    /// * `checkpoints/head.json` — the signed statement about the past.
    ///
    /// `false` for a directory with no `.gx/` and for an empty `.gx/` an operator made by hand:
    /// both are `gx submit`'s ordinary create road and neither has anything to lose. `true` the
    /// moment any of the three exists, which is the state `req/238` H-01 is about — a project with
    /// a journal, a ledger, receipts and a signed head, and no declaration.
    ///
    /// 🔴 **R40 / `req/38` §328 ruling 2 ①** — a witness this process cannot `stat` **counts**.
    ///
    /// Both of these predicates answer "is there something here to lose", and the two ways of being
    /// wrong are not symmetric. Counting an unreadable witness as absent says "this is a fresh
    /// directory" about a project that may hold a history, and every road downstream of that answer
    /// is a road that writes. Counting it as present says "this is a project" about a directory
    /// that may hold nothing, and the road downstream of *that* is a refusal naming the path. So
    /// the predicate is `!is_absent()`: only a `stat` that came back `NotFound` subtracts a witness.
    #[must_use]
    fn established(root: &Path) -> bool {
        !presence_of(&root.join("VERSION")).is_absent()
            || !presence_of(&root.join("ledger").join("journal")).is_absent()
            || !presence_of(&root.join("checkpoints").join(gx_log::HEAD_FILE)).is_absent()
    }

    /// 🔴 **R12 (self-kill, this lane)** — has this project ever recorded a commit?
    ///
    /// Narrower than [`Self::established`], and deliberately: that one answers "is this a project"
    /// and counts `VERSION` among its witnesses, so it cannot tell a project that lost its log from
    /// a directory that carries a declaration and nothing else. This counts only the things a
    /// **commit** leaves — the ledger beside the journal, the recorded head, and the commit
    /// receipts — which is the same set `repair::report_without_engine` calls `witnessed` before it
    /// decides whether an absent journal is a loss or an empty beginning. The two predicates
    /// answering differently is how one project gets two doors.
    #[must_use]
    fn logged(root: &Path) -> bool {
        // 🔴 **R40** — see [`Self::established`] for why an unreadable witness counts.
        !presence_of(&root.join("ledger").join("journal.ledger")).is_absent()
            || !presence_of(&root.join("checkpoints").join(gx_log::HEAD_FILE)).is_absent()
            || std::fs::read_dir(root.join("receipts"))
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false)
    }

    /// 🔴 **R13 / `req/244` M-04** — what says this project has been used, when nothing says it
    /// has committed.
    ///
    /// `None` for a directory with no entries in any of the three, which is what `gx key gen` in a
    /// fresh directory leaves and is [`Self::create`]'s ordinary init road. `Some(sentence)` naming
    /// which directories hold what, so the refusal can say what it saw rather than asserting a
    /// conclusion — `req/244` H-03's remedy is the standing lesson about asserting causes.
    ///
    /// Deliberately **not** folded into [`Self::logged`]. That predicate answers "has this project
    /// recorded a commit", it is the one [`Self::create`] and `repair::report_without_engine` both
    /// branch on, and widening it would make a directory holding one abandoned draft into a project
    /// with a history. Two questions, two predicates — which is the shape `req/244` H-02 is about
    /// when it goes wrong, and the shape it is about when it goes right.
    ///
    /// 🔴 **R14 / `req/246` M-02** — `pub(crate)`, because `repair::report_without_engine` asks the
    /// same question and used to answer it with a different predicate. That is the shape 43 §7.15
    /// (b) forbids: one question, one predicate, whichever door is asking. The writer's door
    /// (`Self::create`, below) and the reporting door now read this one function.
    #[must_use]
    pub(crate) fn used_without_witness(root: &Path) -> Option<String> {
        let mut seen = Vec::new();
        for dir in ["index", "evidence", "drafts"] {
            let count = std::fs::read_dir(root.join(dir))
                .map(|entries| entries.flatten().count())
                .unwrap_or(0);
            if count > 0 {
                seen.push(format!(
                    "`.gx/{dir}/` holds {count} entr{}",
                    if count == 1 { "y" } else { "ies" }
                ));
            }
        }
        if seen.is_empty() {
            None
        } else {
            Some(seen.join(", "))
        }
    }

    /// 🔴 **R10 / `req/238` H-01** — the declaration's bytes, with "not there" classified.
    ///
    /// A directory with no `.gx/` at all keeps 44 §1.4's **6** and the `NOT_FOUND` word it has
    /// carried since M4: that refusal means "you are in the wrong directory", and folding it into a
    /// fault about a declaration would make a typo look like damage. An **established** project
    /// whose declaration is gone is the other sentence entirely, and it is the one `req/238` H-01
    /// measured nobody saying.
    fn read_version(root: &Path, version_path: &Path) -> Result<Vec<u8>> {
        match std::fs::read(version_path) {
            Ok(raw) => Ok(raw),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && Self::established(root) => {
                Err(declaration_absent(version_path))
            }
            Err(e) => Err(io("read", version_path)(e)),
        }
    }

    /// 🔴 **R9 / `req/236` H-04** — `.gx/VERSION`'s layout number, read out of what the file
    /// **declares**.
    ///
    /// R8 normalised byte-order marks, line endings, surrounding space and blank lines — and said
    /// so in `gx_log::head::normalise_declaration`'s own doc comment — but that normalisation sat
    /// *behind* this parse, which read `raw.lines().next()` off the bytes. `req/236` H-04 measured
    /// the consequence on five shapes an ordinary editor produces (a UTF-8 BOM, a leading blank
    /// line, bare-CR line endings, a UTF-16 LE save, and the two lines swapped): every one of them
    /// stopped `gx repair`, `gx repair --yes`, `gx log proof`, `gx replay` **and** `gx serve` with
    /// `VALIDATION_ERROR`, "`\"\u{feff}1\"` is not a number" — no report, no remedy, no way out.
    ///
    /// The parse now consumes `gx_log::head::declaration_lines`, which is the same function the
    /// digest is taken over. So the two questions a project asks about this file — "what does it
    /// declare" and "does it still declare what it declared" — cannot answer from different
    /// readings of the same bytes.
    ///
    /// The version is the **first line that is not a `key=value`**, which is what makes line order
    /// stop mattering. A file with no such line, and a file that is not text at all, are
    /// [`Error::Declaration`] — a classified refusal with a remedy, and one that
    /// [`Layout::open_reporting`] lets a diagnosis walk past.
    ///
    /// # Errors
    /// [`Error::Declaration`] if the bytes will not decode as text, if there is no version line, or
    /// if the version line is not a number.
    pub(crate) fn read_declaration(path: &Path, raw: &[u8]) -> Result<u32> {
        let Some((bare, _)) = gx_log::head::declaration_lines(raw) else {
            return Err(declaration_not_text(path));
        };
        let Some(found) = bare.first() else {
            return Err(declaration_no_version_line(path));
        };
        found
            .parse::<u32>()
            .map_err(|_| declaration_bad_version_line(path, found))
    }

    /// 🔴 **R9 / `req/236` H-04** — the door a **diagnosis** comes through.
    ///
    /// `req/227` M-03's rule is that a reader's door must not be narrower than a writer's, and
    /// `req/222` H-06's is that a state you can see must have a way out of it. A project whose
    /// `.gx/VERSION` will not parse had neither: the one verb whose whole job is to say what is
    /// wrong refused to open, so the operator's screen carried a sentence about a *number* and
    /// nothing about the ledger, the journal, the receipts or the head.
    ///
    /// This opens the directory and hands the fault back as a value. The **writer's** door is
    /// unchanged — [`Layout::open`] still refuses — and a version newer than [`LAYOUT_VERSION`] is
    /// still fail-closed here too, because that one is not "unreadable" but "written by something
    /// this binary does not understand" (47 §4).
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/VERSION` is not there at all, and [`Error::Layout`] for a directory
    /// stamped with a newer version.
    pub fn open_reporting(project: &Path) -> Result<(Self, Option<Error>)> {
        let root = Self::path_for(project);
        let version_path = root.join("VERSION");
        // 🔴 **R10 / `req/238` H-01** — "the file is not there" is a **form** of the declaration
        // fault, handed back as a value like every other one.
        //
        // `docs/LIMITS.md` v0.4-v told a buyer that "`gx repair` opens anyway and reports
        // everything else it can see". `req/238` H-01 measured the sentence being false of the one
        // shape an ordinary accident produces: a `.gx/VERSION` that a backup restore, a
        // synchronising client or an editor **removed** answered exit 6 `NOT_FOUND` with zero
        // report lines. The ledger, the journal, the receipts and the head are all readable in that
        // project; nothing about them depends on this file.
        let raw = match std::fs::read(&version_path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && Self::established(&root) => {
                return Ok((Self { root }, Some(declaration_absent(&version_path))));
            }
            Err(e) => return Err(io("read", &version_path)(e)),
        };
        match Self::read_declaration(&version_path, &raw) {
            Ok(parsed) if parsed > LAYOUT_VERSION => Err(Error::Layout {
                path: version_path.display().to_string(),
                found: parsed.to_string(),
                expected: LAYOUT_VERSION,
            }),
            Ok(_) => Ok((Self { root }, None)),
            Err(fault) => Ok((Self { root }, Some(fault))),
        }
    }

    /// `.gx/` itself.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// One of the declared paths, resolved.
    #[must_use]
    pub fn join(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// 🔴 Where `Engine::open` is pointed, and therefore where the ledger file is (**M6H2-2**).
    ///
    /// `Engine::open(p)` derives `<p>.blobs` and `<p>.ledger` as **siblings** of the journal, and
    /// says why in as many words: "a caller who could point them at different directories could
    /// open a journal against another engine's bodies" (sem: SEM-gx-cli-120). req/56 §2 gives `.gx/ledger/` as a
    /// directory of "append-only store segments" and gives the journal no row at all. req/38 §47
    /// adopted M6-23 as raw material rather than as a ruling, so the binding is still open.
    ///
    /// This hand binds it the only way that satisfies both: the journal is `.gx/ledger/journal`,
    /// which puts the journal, `journal.blobs` and `journal.ledger` inside req/56 §2's `ledger/`
    /// row — three append-only store segments, in the directory whose row says so. No engine
    /// signature moves. Raised as **M6H2-2** because the next hand to write to it (hand 3) inherits
    /// the choice, and a path invented twice is two directories.
    #[must_use]
    pub fn journal_path(&self) -> PathBuf {
        self.root.join("ledger").join("journal")
    }

    /// 🔴 **R6 / DR-43-11** — `.gx/checkpoints/head.json`, the signed head this project keeps.
    ///
    /// req/56 §2 already declares `checkpoints/` — its cells read "signed checkpoints", "derived
    /// but signed, therefore kept", "re-signing required" — so this is a **file in a declared row**
    /// rather than a new row: [`GX_PATHS`] does not move and
    /// `probes/doubt/tests/m6_surface_doubt.rs`'s three-way comparison is untouched. `req/229` L-02
    /// measured what the row held before today — nothing, in every project the audit built, healthy
    /// or attacked — and named that as the structural cause of H-01: a project that keeps no record
    /// of the furthest it has reached cannot notice that it has gone back.
    #[must_use]
    pub fn head_path(&self) -> PathBuf {
        self.root.join("checkpoints").join(gx_log::HEAD_FILE)
    }

    /// 🔴 **R7 / `req/232` M-02** — the digest of `.gx/VERSION`, as it stands right now.
    ///
    /// R6 recorded the journal's framing in this file and refused a project that **deleted** the
    /// line. The seventh audit did not delete it: it wrote `journal_format=legacy` over the top,
    /// and `gx repair` reported `journal_format_declared: "legacy"`, `downgraded: false` and the
    /// whole of R6's refusal came off with one `write(2)`. Binding the file's digest into the
    /// recorded head makes a rewritten declaration the same kind of fact as a shortened journal —
    /// caught by the comparison at every door, with no new `gx_code` and no new exit.
    ///
    /// `None` for a project with no `.gx/VERSION`, which is not a refusal: a head written without
    /// one records no digest and compares exactly as R6's did.
    #[must_use]
    pub fn version_digest(&self) -> Option<String> {
        std::fs::read(self.root.join("VERSION"))
            .ok()
            .map(|bytes| gx_log::head::declaration_digest(&bytes))
    }

    /// 🔴 **R6 / `req/229` H-02** — the journal framing this project has declared, if it has.
    ///
    /// `None` for a project written before this release, or one whose `.gx/VERSION` has only the
    /// number on it. Absence is **not** a declaration of `legacy`: a project that has never said
    /// what it is gets exactly the treatment it got before, and [`Layout::declare_journal_format`]
    /// is what turns absence into a fact — from a read of the file that cuts nothing.
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/VERSION` cannot be read.
    pub fn declared_journal_format(&self) -> Result<Option<gx_engine::JournalFormat>> {
        Self::declared_journal_format_at(&self.root)
    }

    /// 🔴 **R13 / `req/244` M-03** — the same read, off a `.gx/` nobody has opened yet.
    ///
    /// [`crate::declaration::DeclarationWriter::ensure_journal`] runs inside [`Self::create`],
    /// before a `Layout` exists, and until R13 it created a **chained** journal without looking at
    /// what the project declares. The audit measured the result on a `journal_format=legacy`
    /// declaration — a shape audit 12 §6 confirmed pre-R12 binaries stamp, so a working tree can
    /// hold one: one `gx submit`, rc 0, a `GXJRNL01` on the disk, and `gx repair` afterwards
    /// answering `journal_format_declared: "legacy"`, `journal_intact: true`, `downgraded: false`
    /// and `remedy: null` over a project whose declaration and whose file disagree.
    ///
    /// # Errors
    /// [`Error::Io`] if `.gx/VERSION` cannot be read.
    pub fn declared_journal_format_at(root: &Path) -> Result<Option<gx_engine::JournalFormat>> {
        let path = root.join("VERSION");
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io("read", &path)(e)),
        };
        // 🔴 **R9 / `req/236` H-04** — `declaration_lines`, not `lines().skip(1)`.
        //
        // `skip(1)` was "the version is on the first line", stated as an index into the raw bytes.
        // It read a byte-order mark as part of the number, a leading blank line as the version, and
        // a file whose two lines had been swapped as a version of `journal_format=chained`. The
        // split is now by **shape** — a line with an `=` is a setting, a line without one is the
        // version — which is the same reading the digest is taken over, and duplicate keys still
        // resolve to the first (the normal form's sort is stable).
        let Some((_, pairs)) = gx_log::head::declaration_lines(&raw) else {
            // Bytes that are not text declare nothing this function can read. `None` rather than a
            // refusal, for the reason the unreadable-value arm below gives: this file is
            // `Nature::Meta`, and `Layout::open` is the door that classifies it.
            return Ok(None);
        };
        for (key, value) in pairs {
            if key != JOURNAL_FORMAT_KEY {
                continue;
            }
            return Ok(match value.trim() {
                // 🔴 **R30 / `req/372` M-02** — `chained-v2` is what this build declares.
                // `chained` is still read, and still means exactly what it meant: a project made
                // before the record vocabulary grew. Dropping it would turn every existing
                // project into an undeclared one and switch R6's downgrade guard off for them.
                "chained-v2" => Some(gx_engine::JournalFormat::ChainedV2),
                "chained" => Some(gx_engine::JournalFormat::Chained),
                "legacy" => Some(gx_engine::JournalFormat::Legacy),
                // An unreadable value is treated as no declaration rather than as a refusal: this
                // file is `Nature::Meta` (req/56 §2, "losing it is a reconfiguration") and a
                // project made unopenable by one bad line would be a worse failure than the one
                // being defended against.
                _ => None,
            });
        }
        Ok(None)
    }

    /// 🔴 ~~**R6 / `req/229` H-02** — record the framing this project's journal is in.~~ —
    /// **removed in R12 (`req/242` H-01)**.
    ///
    /// `Layout::declare_journal_format` was the third road into `.gx/VERSION`. It was reached from
    /// `session::anchor_accepting` on the writer's road, which is `gx submit`, `gx plan`,
    /// `gx commit`, `gx undo` and `gx serve`'s start-up. It did not appear in `meta_repaired`, it
    /// took no `.pre-repair` copy, and its gate ("`declaration_lines` returned a non-empty bare
    /// line") was **not** the gate [`Layout::open`] refuses on ("the first bare line is a number"),
    /// so a UTF-16 LE save that every door answered `DECLARATION_UNREADABLE` was rewritten by the
    /// next `gx submit`. Deleting the `journal_format` line from a healthy project and running one
    /// `gx submit` put `head_authenticity: verified` back over a `rolled_back` that R7's detector
    /// had just raised, at rc 0 with an empty stderr.
    ///
    /// Its own doc comment claimed "the only roads that create or overwrite this file are
    /// `gx repair --yes` … and `Layout::create`" and "a one-shot window per project, it closes the
    /// first time the project is written to". `req/242` measured both false, the second one
    /// structurally: the window reopens every time the `journal_format` line is deleted.
    ///
    /// What replaces it: nothing stamps. A project that has never declared a framing is
    /// `declared_format: None` — the pre-R6 treatment exactly, with `gx repair`'s
    /// `journal_format_declared: null` saying so out loud — and a project this binary creates
    /// declares `chained` in the write that creates it
    /// ([`crate::declaration::DeclarationWriter::initialise`]). `docs/LIMITS.md` v0.4-y carries what
    /// the removal costs a project written before this release.
    ///
    /// 🔴 **R10/R11's repairs moved rather than vanished**: `repair_declaration`,
    /// `repair_config` and `aside` are [`crate::declaration::DeclarationWriter`] methods now,
    /// because that is the type that may write these files.
    const _DECLARE_JOURNAL_FORMAT_REMOVED_IN_R12: () = ();

    /// `.gx/config.toml`, spelled once.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// 🔴 **R10 / `req/238` H-01** — the writer's door asks this and the reader's door does not.
    ///
    /// `req/227` M-03's rule is that a reader's door must not be narrower than a writer's, so this
    /// is called from `Session::open_wired_with_posture` and from `gx serve`'s build and from
    /// nowhere that only reads: `gx log proof`, `gx receipt show` and `gx repair`'s report mode all
    /// run on a project whose `config.toml` is gone. Only the verbs that would otherwise have had
    /// it re-created underneath them refuse.
    ///
    /// # Errors
    /// [`Error::ConfigAbsent`] if this is an established project and the file is gone.
    pub fn require_config(&self) -> Result<()> {
        let path = self.config_path();
        if Self::established(&self.root) && !path.exists() {
            return Err(config_absent(&path));
        }
        Ok(())
    }

    /// 🔴 **R11 / `req/240` M-03** — every `*.pre-repair.<n>` this project is holding.
    ///
    /// The family [`Layout::aside`] creates, counted so that `gx repair` can say it is there.
    /// `req/240` M-03 measured three of them accumulating in `.gx/` while the report said
    /// `remedy: null`, `staging_files: []` and did not contain the substring `pre-repair` at all —
    /// a family with no row in [`GX_PATHS`], no row in req/56 §2, no row in 43 §7.9 (b)'s detector
    /// table and no verb that lists it. Nothing here removes one (that is the point of keeping
    /// them); what changes is that an operator is told the number.
    ///
    /// Sorted, project-relative, and read off the directory rather than remembered: these outlive
    /// the run that made them.
    #[must_use]
    pub fn kept_aside(&self) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(&self.root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
                    // 🔴 **R12 / `req/242` L-02** — the names **gx wrote**, not every name
                    // that contains the substring.
                    //
                    // The filter was `name.contains(".pre-repair.")`, so an operator's own
                    // `.gx/notes.pre-repair.txt` came out in `kept_aside` as a file gx claimed to
                    // have set aside. That is the mirror, one report down, of `req/240` L-01 —
                    // which R11 fixed for the *sweep* by asking the same question: is this a name
                    // gx makes. `DeclarationWriter::aside` makes exactly
                    // `<file>.pre-repair.<n>` with `n` a number below `PRE_REPAIR_LIMIT`, so that
                    // is what is counted. `.gx/sub/deep.pre-repair.9` is still not counted: this
                    // walks `.gx/` itself, which is where `aside` writes.
                    // 🔴 **R13 / `req/244` L-01** — and the **stem** is the one file `aside` is
                    // ever called on.
                    //
                    // R12 closed the tail (`.txt` and `.9` stopped counting) and left the stem
                    // open, so `.gx/notes.pre-repair.3` — a name gx cannot produce — was still
                    // reported as a file gx had set aside. `DeclarationWriter::aside` has exactly
                    // one caller, `repair_declaration`, and it is called on `.gx/VERSION` and on
                    // nothing else; a second caller is a change a person makes, and it is a change
                    // that has to move this line with it.
                    // 🔴 **R14 / `req/246` M-04** — and the second stem gx can now produce.
                    //
                    // `repair::aside_of` moves a **non-directory** out of `.gx/repair`'s way so
                    // that a project blocked by one has an exit. Those bytes are somebody's, gx
                    // keeps them, and a file gx set aside is exactly what this key is for. The
                    // list is closed on purpose: an operator's `.gx/notes.pre-repair.3` is still
                    // not counted (`req/244` L-01), and a third writer of this name has to move
                    // this line with it.
                    // 🔴 **R15 / `req/259` M-01** — and the stems are read off the declared table
                    // rather than written out here.
                    //
                    // `repair::aside_of` is called on **every** `Shape::Dir` row now, so a list
                    // holding two names would have under-counted six of the seven — a file gx had
                    // just set aside, missing from the key whose whole job is to name it. The rule
                    // is unchanged (only names gx itself can produce are counted, `req/244` L-01);
                    // what changed is that "the names gx can produce" is derived from the same
                    // table the writer walks. `.gx/VERSION` stays spelled out because
                    // `DeclarationWriter::aside` is the other writer of this shape and its subject
                    // is a `Nature::Meta` **file**, not one of these rows.
                    .filter(|name| {
                        name.rsplit_once(".pre-repair.")
                            .is_some_and(|(stem, tail)| {
                                (stem == "VERSION" || declared_directories().any(|rel| rel == stem))
                                    && tail.parse::<u32>().is_ok_and(|n| n < PRE_REPAIR_LIMIT)
                            })
                    })
                    .map(|name| format!(".gx/{name}"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    /// 🔴 **R11 / `req/240` M-02** — is req/56 §4's `.gx/.gitignore` where the project left it.
    ///
    /// A fact rather than a refusal, for the reason [`Layout::create`] gives where it stopped
    /// writing this file back into an established project.
    #[must_use]
    pub fn gitignore_absent(&self) -> bool {
        Self::established(&self.root) && !self.root.join(".gitignore").exists()
    }

    /// The ledger file beside [`Layout::journal_path`], as `Engine::open` derives it.
    ///
    /// Spelled here rather than reconstructed by each caller: `gx log proof` and `gx receipt
    /// verify` both need it, and a second `push(".ledger")` somewhere else is a second convention.
    #[must_use]
    pub fn ledger_path(&self) -> PathBuf {
        let mut p = self.journal_path().into_os_string();
        p.push(".ledger");
        PathBuf::from(p)
    }

    /// The verdict checkpoint chain beside [`Layout::journal_path`], as `Engine::open` derives it.
    ///
    /// 🔴 **DR-43-7 (`req/38` §153).** Spelled here for [`Layout::ledger_path`]'s stated reason — a
    /// second `push(".verdicts")` somewhere else is a second convention — and needed now because
    /// `gx verdict-checkpoint list` stopped opening a whole `Session` to read a file. `req/215` M-02
    /// measured what that cost: a **read** took the project's writer lock and answered `BUSY` when
    /// another process held it, and truncated a torn tail on the way past (H-03).
    #[must_use]
    pub fn verdict_log_path(&self) -> PathBuf {
        let mut p = self.journal_path().into_os_string();
        p.push(".verdicts");
        PathBuf::from(p)
    }

    /// 🔴 req/56 §5, per subdirectory, **with the declaration**.
    ///
    /// Walks `GX_PATHS`, repairs what can be repaired, and returns one row per path saying which of
    /// [`Recovery`]'s five things happened. The rule per row is [`GxPath::nature`]:
    ///
    /// * `Derived` — recreate and call it [`Recovery::Regenerated`]. Nothing is lost.
    /// * `Meta` — recreate with defaults; a `VERSION` that was missing is a fresh directory.
    /// * `Source` — recreate the container and call it [`Recovery::Lost`]. **The directory is usable
    ///   and the contents are gone**, and those are two different facts that a `bool` would merge.
    /// * `Countersigned` — recreate and call it [`Recovery::Lost`] too: req/56 §2's cell is
    ///   "re-signing required" (sem: SEM-gx-cli-121), and re-signing needs the ledger key, which a third party running a verifier does not
    ///   have. Reporting it as regenerated would be a checkpoint claimed rather than produced.
    /// * `ledger/` — [`Recovery::Delegated`], see the module documentation.
    ///
    /// # Errors
    /// [`Error::Io`] if a repair cannot be made.
    pub fn recover(&self) -> Result<RecoveryReport> {
        let mut rows = Vec::with_capacity(GX_PATHS.len());
        for path in GX_PATHS {
            let full = self.root.join(path.rel);
            let present = full.exists();
            let outcome = match (present, path.rel, path.nature) {
                // 🔴 **DR-43-5 (2) / DR-43-7 (1)** — declared, and not this function's to repair.
                // See [`Recovery::Untouched`]: one is a running process's exclusion and the other
                // is evidence, and creating either would be a lie of a different kind.
                (_, _, Nature::Transient) => Recovery::Untouched,
                _ if path.shape == Shape::Pattern => Recovery::Untouched,
                (true, "ledger", _) => Recovery::Intact,
                (false, "ledger", _) => {
                    std::fs::create_dir_all(&full).map_err(io("create", &full))?;
                    Recovery::Delegated
                }
                (true, _, _) => Recovery::Intact,
                (false, _, Nature::Derived) => {
                    std::fs::create_dir_all(&full).map_err(io("create", &full))?;
                    Recovery::Regenerated
                }
                // 🔴 **R12 / `req/242` H-01** — a `Nature::Meta` file this walk finds missing is
                // **reported** and not written.
                //
                // This arm was the workspace's fourth road into `.gx/VERSION` and
                // `.gx/config.toml`, and it is reachable from no verb: nothing in `main.rs` calls
                // [`Layout::recover`], and the `gx doctor` its neighbours name does not exist. A
                // road no probe forbade and no reader met is the shape the other three had before
                // somebody measured them. `Recovery::Lost` is the honest cell — R7 binds this
                // file's digest into the signed head, so a re-created declaration is a detector
                // taken off, and the only road that may take it off is the one that says it did.
                (false, _, Nature::Meta) => Recovery::Lost,
                (false, _, Nature::Source | Nature::Countersigned) => {
                    std::fs::create_dir_all(&full).map_err(io("create", &full))?;
                    Recovery::Lost
                }
            };
            rows.push((path.rel, outcome));
        }
        Ok(RecoveryReport { rows })
    }
}

/// 🔴 **R9 / `req/236` H-04** — `.gx/VERSION` is there and is not text.
///
/// A free function rather than a closure inside [`Layout::read_declaration`] because R10 gave the
/// declaration a **second** caller that must refuse rather than guess
/// ([`Layout::declare_journal_format`]), and two spellings of one refusal is how "the reader and
/// the writer disagree" gets back in.
fn declaration_not_text(path: &Path) -> Error {
    Error::Declaration {
        path: path.display().to_string(),
        form: "the bytes are neither UTF-8 nor UTF-16 with a byte-order mark".to_string(),
        remedy: "this file is two lines of plain text. Write it again as UTF-8: the layout version on the first line (`1`) and `journal_format=chained` on the second, with a trailing newline — or let `gx repair --yes` do it, which keeps your bytes beside it as `VERSION.pre-repair.<n>` and says so. Nothing else in `.gx/` is touched by rewriting it, and `gx repair` will tell you whether the digest then matches the head this project signed"
            .to_string(),
    }
}

/// 🔴 **R9 / `req/236` H-04** — every line is a setting and none of them is the version.
fn declaration_no_version_line(path: &Path) -> Error {
    Error::Declaration {
        path: path.display().to_string(),
        form: "no line carries a layout version — every line is a `key=value`".to_string(),
        remedy: "add the layout version as its own line: `1`. Order does not matter (R9 reads the declaration rather than the byte order), but the number has to be there, because it is what tells a future binary whether it can read this directory at all"
            .to_string(),
    }
}

/// 🔴 **R9 / `req/236` H-04** — there is a version line and it is not a number.
fn declaration_bad_version_line(path: &Path, found: &str) -> Error {
    Error::Declaration {
        path: path.display().to_string(),
        form: format!("the layout version line reads {found:?}, which is not a number"),
        remedy: "replace that line with the layout version alone — `1` for a directory this binary writes. Settings belong on their own `key=value` lines below it"
            .to_string(),
    }
}

/// 🔴 **R10 / `req/238` H-01** — this project has a journal and no declaration.
///
/// The remedy names `gx repair --yes` and **not** "run any verb and it will come back", which is
/// what the binary used to do without telling anybody.
fn declaration_absent(path: &Path) -> Error {
    Error::DeclarationAbsent {
        path: path.display().to_string(),
        remedy: "this project has a journal, a ledger and a head, so it is a project that has lost its declaration rather than a directory that never had one — a restore from a backup that skipped it, a synchronising client, or an editor. gx will not write one back on its own: R7 binds this file's digest into the signed head so that a *rewritten* declaration is caught, and a writer that silently re-created a *lost* one would take that detector off (`req/238` H-01). Run `gx repair` to see everything else about the project, and `gx repair --yes` to write the declaration back and be told that it did"
            .to_string(),
    }
}

/// 🔴 **R10 / `req/238` H-01** — this project has a journal and no `.gx/config.toml`.
///
/// 43 §7.9 (b)'s R9 row calls this "the file that decides the recovery key". `req/238` H-01
/// measured `gx submit` answering its absence by writing the shipped two-comment default at rc 0,
/// which is `engine_signing_keyid` going back to nothing with nobody told.
fn config_absent(path: &Path) -> Error {
    Error::ConfigAbsent {
        path: path.display().to_string(),
        remedy: "43 §7.9 (b) calls this the file that decides which key a recovery signs with (`engine_signing_keyid`), so re-creating it from the shipping defaults would put this project back on a key it did not choose, silently. gx will not: run `gx repair` to see the rest of the project, `gx repair --yes` to write the default file back and be told that it did, and then put your `engine_signing_keyid` line back — or pass `--signing-key` for one run"
            .to_string(),
    }
}

/// 🔴 **R12 / `req/242` H-01 (d)** — this project's journal is not there and a writer's door
/// was about to append to it.
///
/// The remedy is `journal_absent_report`'s, in one sentence: gx does not rebuild the file from the
/// ledger's leaves, because those leaves were built **from** the journal's records and a journal
/// composed here would be a witness statement gx wrote rather than one it kept.
pub(crate) fn journal_absent(path: &Path) -> Error {
    Error::JournalAbsent {
        path: path.display().to_string(),
        remedy: "restore `.gx/ledger/journal` from a backup, or from whatever removed it, and run \
                 the verb again. gx does not compose one: the ledger's leaves were built from those \
                 records, so a journal written here would be a statement gx made up rather than one \
                 it kept, and the next `gx repair` would report a healthy project over a loss \
                 (`req/242` H-01 (d)). What still holds meanwhile: `gx repair` reads the ledger, the \
                 commit receipts and the recorded head out of their own files and reports all three, \
                 and `gx receipt verify --offline` still proves what was committed"
            .to_string(),
    }
}

/// 🔴 **R13 / `req/244` M-04** — this project has been used and holds no witness of any commit.
///
/// The refusal `req/244` M-04 asked for. `Layout::logged`'s three witnesses are gone and something
/// else in `.gx/` says work happened here, so the writer's door declines to write a fresh journal
/// over it. The sentence names what it saw rather than what it concluded.
fn history_lost(root: &Path, evidence: &str) -> Error {
    Error::HistoryLost {
        path: root.display().to_string(),
        evidence: evidence.to_string(),
        remedy: "the three things a commit leaves — `.gx/ledger/journal.ledger`, \
                 `.gx/checkpoints/head.json` and the commit receipts under `.gx/receipts/` — are \
                 all absent, and the directories above say this project has been worked in. gx \
                 will not start a second history over that: `gx submit` would create a fresh \
                 journal, after which `gx repair` reports `journal_commits: 0` and \
                 `head_authenticity: \"absent\"` and nothing anywhere records that a history \
                 existed (req/244 M-04, req/242 L-04). What to fix: restore `.gx/` from a backup — \
                 the ledger, the checkpoints and the receipts belong to the same backup unit as \
                 the journal (req/56 §5, 47 §4). If this directory really is a fresh start and the \
                 entries above are residue you do not want, move them aside by hand; gx does not \
                 remove them, because a verb that deletes evidence is a verb this repository does \
                 not build (DR-43-7 (1)). `gx repair` reads and reports everything that is still \
                 there meanwhile"
            .to_string(),
    }
}

/// 🔴 **R14 / `req/246` M-04** — a declared directory's path holds something that is not one.
///
/// The noun in the middle is measured rather than assumed: a regular file, a symbolic link and
/// "something else" are three different things an operator has to look for, and a refusal that
/// said "a file" about a dangling symlink would send them to the wrong place. The remedy names the
/// verb that clears it and says what that verb does with the bytes, because `req/244` M-04's
/// standing rule is that gx does not remove what it did not write.
///
/// # 🔴 **R16 / `req/262` M-02** — and the command line it prints is the one that works
///
/// R15 made this refusal true for all seven declared directories and wrote 43 §7.17 (b) condition
/// 2: "the truth of a remedy is measured by running what it tells you to run". The gate it built
/// ran `gx repair --yes --signing-key <ID>`. The **text** said `gx repair --yes`. The sixteenth
/// audit ran the text: seven directories, two shapes each, **fourteen out of fourteen** answered
/// `cleared: false`, `kept_aside: []`, and a following `gx submit` refused again — because
/// `repair.rs`'s `writing` predicate is `yes && key.is_some() && _held.is_some()`, so a repair
/// with no key resolves to a report. The probe had been reading its own idea of the command rather
/// than gx's, which is the failure mode a machine built to check a sentence is supposed to be
/// immune to.
///
/// So the flag is in the sentence, and `model_a_probes`'
/// `the_remedy_for_a_blocked_directory_is_true_of_every_declared_directory` now **extracts the
/// command line out of this string** and runs it with nothing added. The alternative — dropping the
/// key requirement for the part of a repair that only renames and makes a directory — was weighed
/// and not taken: `repair_dir_state` runs inside the same `--yes` road that goes on to write
/// `.gx/VERSION` and `.gx/config.toml` and to file a signed repair record, and splitting `writing`
/// into "needs a key" and "does not" would put a second predicate in front of the one road
/// `req/242` M-03 built so that a run which cannot take the lock writes nothing at all.
/// 🔴 **R40 / `req/553` M-01, `req/38` §328 ruling 2 ① — what `Path::exists()` cannot say.**
///
/// `Path::exists()` is `fs::metadata(..).is_ok()`, so it folds **every** `Err` to `false`: a path
/// that is not there and a path this process may not `stat` come back as one answer. R40 measured
/// what that costs. Make `.gx/ledger/` unreadable and `gx repair --json` says `journal_absent:
/// true` about a journal that is sitting there holding 1,798 bytes, and `gx submit` refuses
/// `JOURNAL_ABSENT` — a refusal whose own title is "this project's `.gx/ledger/journal` **is not
/// there**", said about a file that is.
///
/// So the question is asked once, here, and it has **three** answers rather than two:
///
/// * [`Presence::Absent`] — the operating system said `NotFound`. This is the only answer that
///   licenses the word "absent", and the only one the read road's escape hatch may pass on
///   (`ledger::refuse_if_the_two_files_disagree`).
/// * [`Presence::Present`] — a `stat` succeeded, and the file type it returned comes with it, so a
///   caller that cares whether a declared **file** is a directory does not have to ask twice.
/// * [`Presence::Undetermined`] — something else. **Not** folded into either of the others: a door
///   that cannot tell what is at a path is a door that fails closed, and the sentence it prints
///   names the operating system's own classification rather than guessing at absence.
///
/// `symlink_metadata` rather than `metadata`, for the reason
/// [`Layout::declared_directories_are_directories`] gives one screen up: the **final component** is
/// not followed, so a symbolic link where a journal is declared is its own shape rather than
/// whatever it points at, and a dangling one is `Present` (the link is there) rather than `Absent`.
pub(crate) enum Presence {
    /// `stat` said `NotFound`. The one answer that means "there is no second file".
    Absent,
    /// `stat` succeeded. Carries the type, so the shape question is answered by the same syscall.
    Present(std::fs::FileType),
    /// `stat` failed for any other reason. Fail closed; never call this absent.
    Undetermined(std::io::Error),
}

/// The one spelling of "is this path there", asked by every journal-facing door.
///
/// 🔴 **R40 / `req/38` §156 ruling 2(a), in its predicate form.** Before R40 the same question was
/// spelled `!journal.exists()` at `Layout::open`, at the writer's door in `session.rs` and in
/// `repair.rs`'s report, and each spelling folded `Err` its own way by accident rather than by
/// decision. One condition gets one word; a condition asked three times gets one predicate.
pub(crate) fn presence_of(path: &Path) -> Presence {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => Presence::Present(meta.file_type()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Presence::Absent,
        Err(e) => Presence::Undetermined(e),
    }
}

impl Presence {
    /// `true` only for [`Presence::Absent`] — the predicate `!path.exists()` was standing in for.
    ///
    /// Named rather than pattern-matched at every site so that "absent" cannot quietly re-acquire
    /// its old meaning ("absent, or unreadable, or on a filesystem that went away").
    /// 🔴 There is deliberately **no** `is_present()` beside it. Every site R40 converted needed
    /// either "is this established as absent" or the full three-way `match`, and a boolean spelled
    /// the other way round would be `!exists()`'s fold wearing a new name — the two-answer question
    /// is the defect, so the two-answer helper is not offered.
    pub(crate) fn is_absent(&self) -> bool {
        matches!(self, Presence::Absent)
    }
}

/// 🔴 **R40 / `req/38` §328 ruling 2 ② — the declared **file** whose path holds something else.**
///
/// `.gx/ledger/journal` is declared as a file by `req/56` §2 and `Layout::create` writes one there.
/// Audit 39 replaced it with a **directory** of the same name and every read verb answered exit 0
/// with a signed checkpoint, because the engine's open failed and the read road treated a failed
/// open as "there is no second file". The shape is knowable before any open is attempted, and
/// `LAYOUT_BLOCKED`'s own `why` — "`GX_PATHS` declares the shape; the path is there and is not what
/// the declaration says" — is true of it word for word.
///
/// §328 ruling 2 ② widened that row's **title** from "a declared directory" to "a declared path"
/// so this case can wear it honestly. What the widening deliberately does **not** cover is a
/// journal that is a regular file the process cannot open (mode `0000`): there the path *is* what
/// the declaration says, the `why` above would be false, and §328 ruling 2 ③ leaves that condition
/// on `INTERNAL` and files the vocabulary question as a DR rather than stretching this row over two
/// conditions.
/// 🔴 `rel` is a **parameter** rather than a literal in this function, and the reason is a gate.
///
/// `probes/doubt/tests/m6_surface_doubt.rs::the_dotgx_layout_is_req56_exactly` reads every
/// `rel: "…"` line out of this file and asserts the result **is req/56 §2's row list** — as a list,
/// not as a set. R40 wrote `rel: "ledger/journal"` here first and the probe went red with
/// `ADDED=["ledger/journal"]`, correctly: the journal is a file **inside** the declared `ledger`
/// row, not a twelfth row, and inventing one would have been a layout surface addition smuggled in
/// as a message field. Changing the literal to `"ledger"` left the probe red for the second reason
/// the list-not-set comparison exists to give: twelve entries where req/56 §2 has eleven.
///
/// So the row this refusal is **about** is named by the door that is walking it, where the identity
/// is already established, and this function takes it. Nothing here declares a path.
fn journal_blocked(path: &Path, rel: &'static str, found: &'static str) -> Error {
    Error::LayoutBlocked {
        path: path.display().to_string(),
        rel: rel.to_string(),
        expected: "`.gx/ledger/journal` has to be the regular file this project's history is \
                   appended to"
            .to_string(),
        found: found.to_string(),
        remedy: "req/56 §2 declares `.gx/ledger/journal` as the append-only file this project's \
                 history lives in, and what is at that path is not one. gx does not remove what is \
                 there — it is not gx's file, and the verb that destroys evidence is the verb this \
                 repository does not build (DR-43-7 (1)). Move it yourself and put the journal \
                 back, from a backup or from whatever displaced it. gx will not compose a journal \
                 from the ledger's leaves: those leaves were built **from** the journal's records, \
                 so one written here would be a statement gx made up rather than one it kept \
                 (`req/242` H-01 (d)). What still holds meanwhile: `gx repair` reads the ledger, \
                 the commit receipts and the recorded head out of their own files and reports all \
                 three, and `gx receipt verify --offline` still proves what was committed"
            .to_string(),
    }
}

fn layout_blocked(path: &Path, rel: &'static str) -> Error {
    let found = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => "a symbolic link",
        Ok(meta) if meta.is_file() => "a regular file",
        Ok(_) => "not a directory",
        Err(_) => "a path this process cannot read",
    };
    Error::LayoutBlocked {
        path: path.display().to_string(),
        rel: rel.to_string(),
        expected: format!("`.gx/{rel}` has to be a directory"),
        found: found.to_string(),
        remedy: format!(
            "req/56 §2 declares `.gx/{rel}` as a directory and every verb that writes asks the \
             operating system for it on the way in, so this project refuses `gx submit`, `gx log` \
             and `gx receipt` until the path is one. gx does not remove what is there — it is not \
             gx's file, and the verb that destroys evidence is the verb this repository does not \
             build (DR-43-7 (1)). Two ways out: move it yourself, or run \
             `gx repair --yes --signing-key <KEY_ID>`, which \
             renames it to `.gx/{rel}.pre-repair.<n>`, makes the directory, and names the copy it \
             kept under `kept_aside`. The key id is any one `gx key list` shows (`gx key gen` makes \
             one): a repair that may write resolves a key first, because everything past that \
             point can end in a signature, so a plain `gx repair --yes` reports that it was \
             refused and moves nothing (req/262 M-02). `gx repair` on its own reports the state \
             under \
             `repair_dir_blocked` and exits 1 (req/246 M-04, req/259 M-01 — this holds for every \
             directory req/56 §2 declares, which it did not before). What that restores is the \
             **shape**: if what belonged in this directory was a log or a receipt, it is still \
             gone afterwards and the next refusal will say so by its own name. If what is at this \
             path is a **symbolic link** (the noun above says which), the bytes it points at are \
             neither moved nor touched: gx sets the link aside and makes an empty directory, so a \
             project that was keeping this directory somewhere else comes back as a project with \
             an empty one. `kept_aside` names the link in the report of the run that moved it and \
             nothing says it afterwards — put the target back yourself if that is what you meant \
             (req/262 L-01)"
        ),
    }
}

/// 🔴 **R13 / `req/244` M-03** — this project declares `journal_format=legacy` and has no journal,
/// so the door that would have created one declined.
///
/// The same word as [`journal_absent`] and for the same reason — the journal is not there and a
/// writer refused to invent one — with the sentence that says which fact stopped it. What R12 left
/// open: `DeclarationWriter::create_journal` writes `GXJRNL01` unconditionally, so one `gx submit`
/// over a legacy declaration produced a **chained** file at rc 0, after which `gx repair` reported
/// `journal_format_declared: "legacy"`, `journal_intact: true` and `downgraded: false` about a
/// project whose two halves disagree (`req/244` M-03, three runs, no variation).
pub(crate) fn journal_absent_declared_legacy(journal: &Path, declaration: &Path) -> Error {
    Error::JournalAbsent {
        path: journal.display().to_string(),
        remedy: format!(
            "{} declares `journal_format=legacy` and this journal is not there. gx will not create \
             one here: a legacy journal carries no marker, so an empty one is a file of zero bytes \
             and gx reads a file of zero bytes as **chained** — there is no journal this door could \
             write that the declaration would be true of, and writing a chained one anyway is what \
             `req/244` M-03 measured (rc 0, `GXJRNL01` on the disk, and a `gx repair` afterwards \
             calling the project healthy). What to fix: restore `.gx/ledger/journal` from a backup, \
             which is the only thing that makes the declaration true again; or, if this project \
             has never recorded a commit and the legacy line is the residue of an older binary, \
             change that line to `journal_format=chained` and run the verb again. `gx repair` reads \
             and reports everything else meanwhile",
            declaration.display()
        ),
    }
}

// 🔴 ~~**R10 / `req/238` H-01** — one of the two [`Nature::Meta`] files is gone from a
// project that has one, dispatched to the sentence that names it.~~ — **removed in R12**.
//
// `meta_absent(rel, full)` existed because `Layout::create`'s loop wrote both `Nature::Meta`
// files and had to pick a refusal off a `&str`. R12 took the file arm out of that loop
// (`req/242` H-01), so the two callers ask for the two sentences by name — `declaration_absent`
// through `Layout::read_version`, and `config_absent` directly — and a dispatcher on a string is
// one fewer place where "which file is this about" can be answered wrongly.

/// 🔴 **R12 / `req/242` H-01** — `MetaRepair` lives with the type that produces it.
///
/// Re-exported here because 44 and `gx repair`'s report both spell it as a fact about the layout,
/// and because moving a name is not the same as changing one. The enum, the two byte-composers
/// (`declared_text`, `default_contents`) and every `std::fs` call that touches `.gx/VERSION` or
/// `.gx/config.toml` are in `crate::declaration` — the module
/// `probes/doubt/tests/declaration_writer_doubt.rs` counts.
pub use crate::declaration::MetaRepair;
