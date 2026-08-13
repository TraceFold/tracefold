//! `.gx/` — req/56's directory, and the seven paths req/88 §6.2 手 1 ④ asks for.
//!
//! req/56 §1 is the whole requirement in one sentence: 「user の project code に**干渉ゼロ**: gx の全
//! state は `<project>/.gx/` に閉じ、既定で VCS に入れない。秘密は project に置かない」. §2 gives six
//! paths, §4 gives the commit boundary, §5 gives the recovery rules.
//!
//! # The seventh path
//!
//! `drafts/`, from **M6-01 採(a)** (req/38 §47): 「`.gx/drafts/` に intent 本体を置く(engine は触ら
//! ない・CLI 層管理)」. req/56 was written in M2 and the ruling is from M6, so the difference between
//! the two documents is the addition and nothing else —
//! `probes/doubt/tests/m6_surface_doubt.rs` parses both and asserts exactly that.
//!
//! # 🔴 What this module does **not** do to the ledger
//!
//! req/56 §5 writes three recovery rules — 「dir 不在=初期化/index 破損=再生成/ledger 破損=tail
//! truncate(手 3 の torn-write 規約流用)」 — and this module implements the first two and
//! **delegates** the third. The truncation of a torn append-only tail is `gx-log`'s and the engine's
//! (`EngineJournal::open`, `LedgerStore::open`, and the five torn shapes M5 hand 1 folded into one
//! answer); a second implementation in the CLI would be a second opinion about where a log ends.
//! [`Recovery::Delegated`] is what this module answers for that path, and answering it is the point:
//! 「何が失われ何が再生成されたかを必ず申告」 (req/56 §5, the skip≠pass lineage of req/29 §4).

use std::path::{Path, PathBuf};

use crate::{io, Error, Result};

/// The layout version this binary writes into `.gx/VERSION`.
///
/// One, and the first thing it will ever have to say is that **E-M5-13 changed the journal record
/// shape** (`Planned` gained `locator` and `parents`). req/38 §47 M6-14 採(a) took that change
/// precisely because M6 is the hand that builds the first distributable and 47 §4 makes journal
/// compatibility an upgrade precondition: before shipping the change costs nothing, after shipping
/// it costs every user's journal. A directory stamped `1` is a directory written after that change.
pub const LAYOUT_VERSION: u32 = 1;

/// Whether a path under `.gx/` is a directory or a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A directory `create` makes and `recover` re-makes.
    Dir,
    /// A file with contents.
    File,
}

/// req/56 §2's third column — 「性質(⑤DB 原則)」 — which is what decides the recovery rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nature {
    /// 「source of truth」 / 「source」. Losing it loses data; nothing can regenerate it.
    Source,
    /// 「derived・消して良いと宣言」. Regenerating it is always correct.
    Derived,
    /// Signed and derived at once (`checkpoints/`): re-derivable, but only by the holder of the
    /// ledger signing key. req/56 §2's cell is 「再署名要」.
    Countersigned,
    /// Settings and metadata. Losing it is a reconfiguration, not a data loss.
    Meta,
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

/// 🔴 req/56 §2's six rows plus **M6-01 採(a)**'s `drafts/`.
///
/// A declared list rather than a `read_dir`, for `SHIPPED_CRATE_ROOTS`'s reason: a list derived from
/// the tree cannot notice a tree that is wrong, and the recovery report below has to be able to say
/// 「this was missing」 about something.
pub const GX_PATHS: [GxPath; 8] = [
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
    // **M6-01 採(a)**, the seventh. The intent body between `gx submit` and `gx plan`, which are two
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
    // 44 §1.2: 「`show`: ローカルストア/gx-apiから`Receipt`を取得し表示」. There was no local store.
    // `Engine::receipt` reads an in-memory table `Engine::open` leaves empty on purpose (M5H3-5);
    // the journal's thirteen record kinds hold no receipt (42 §3.13); and 42 §3.11 keeps the body
    // out of the ledger leaf, which carries a **digest** — 「receipt本体をleafの外に置く」. So a
    // second `gx` process has nowhere to read one from, `gx receipt show` cannot be implemented,
    // and M6-16's staged disclosure (§47 採(a)) — which M6-22 hangs on — has no subject.
    //
    // Its nature is `Source` and that cell is the honest one rather than the flattering one. A
    // receipt is signed, so `Countersigned` looks apt; but 「再署名要」 says a holder of the key can
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
];

/// What happened to one path when [`Layout::recover`] looked at it.
///
/// req/56 §5's reporting requirement is the reason this is an enum and not a `bool`: 「**何が失われ何
/// が再生成されたかを必ず申告**」. `Intact` and `Regenerated` are both 「it is there now」 and they are
/// not the same fact, which is req/29 §4's 「skip と pass を同じ顔にしない」 one directory down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// It was there and was left alone.
    Intact,
    /// It was absent and has been created empty. req/56 §5: 「dir 不在=初期化」.
    Initialised,
    /// It was absent or unreadable, and rebuilding it is always correct because it is derived.
    /// req/56 §5: 「index 破損=再生成」.
    Regenerated,
    /// 🔴 It was absent and **nothing here can replace it**. The directory is usable again and the
    /// contents are gone; saying so is the requirement.
    Lost,
    /// 🔴 It is another layer's to repair. The append-only tail rule is `gx-log`'s and the engine's
    /// (req/56 §5's third rule, 「手 3 の torn-write 規約流用」), and a second implementation of it
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
    /// The declaration req/56 §5 asks an operator to be shown. An empty list means 「nothing was
    /// missing」 and is a different sentence from 「nothing was checked」, which is why the caller
    /// gets rows rather than a count.
    #[must_use]
    pub fn changed(&self) -> Vec<(&'static str, Recovery)> {
        self.rows
            .iter()
            .copied()
            .filter(|(_, k)| *k != Recovery::Intact)
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
    /// command call it rather than making 「did you run `gx init`」 a thing a user can get wrong.
    ///
    /// # Errors
    /// [`Error::Io`] if any directory or file cannot be created.
    pub fn create(project: &Path) -> Result<Self> {
        let root = Self::path_for(project);
        std::fs::create_dir_all(&root).map_err(io("create", &root))?;
        for path in GX_PATHS {
            let full = root.join(path.rel);
            match path.shape {
                Shape::Dir => std::fs::create_dir_all(&full).map_err(io("create", &full))?,
                Shape::File => {
                    if !full.exists() {
                        std::fs::write(&full, default_contents(path.rel))
                            .map_err(io("write", &full))?;
                    }
                }
            }
        }
        // req/56 §4: 「既定=`.gx/` 全体を gitignore(干渉ゼロ原則)」. A `.gitignore` **inside** `.gx/`
        // holding `*` ignores the whole directory including itself, so the user's own `.gitignore`
        // is not edited by us — which is the difference between honouring 干渉ゼロ and talking about
        // it. §4's opt-in (「共有したい物だけ `!.gx/config.toml` 型で明示 un-ignore」) is then the
        // user's edit to this file, in the place a reader looks for it.
        let ignore = root.join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, "# req/56 §4: gx keeps its state out of your history.\n# Un-ignore what you want to share, e.g. `!config.toml`.\n*\n")
                .map_err(io("write", &ignore))?;
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
        let version_path = root.join("VERSION");
        let raw = std::fs::read_to_string(&version_path).map_err(io("read", &version_path))?;
        let found = raw.trim();
        let parsed: u32 = found.parse().map_err(|_| Error::Malformed {
            what: "layout version",
            path: version_path.display().to_string(),
            detail: format!("{found:?} is not a number"),
        })?;
        if parsed > LAYOUT_VERSION {
            return Err(Error::Layout {
                path: version_path.display().to_string(),
                found: found.to_string(),
                expected: LAYOUT_VERSION,
            });
        }
        Ok(Self { root })
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
    /// says why in as many words: 「a caller who could point them at different directories could
    /// open a journal against another engine's bodies」. req/56 §2 gives `.gx/ledger/` as a
    /// directory of 「append-only store segments」 and gives the journal no row at all. req/38 §47
    /// adopted M6-23 as 材料 rather than as a ruling, so the binding is still open.
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

    /// 🔴 req/56 §5, per subdirectory, **with the declaration**.
    ///
    /// Walks `GX_PATHS`, repairs what can be repaired, and returns one row per path saying which of
    /// [`Recovery`]'s five things happened. The rule per row is [`GxPath::nature`]:
    ///
    /// * `Derived` — recreate and call it [`Recovery::Regenerated`]. Nothing is lost.
    /// * `Meta` — recreate with defaults; a `VERSION` that was missing is a fresh directory.
    /// * `Source` — recreate the container and call it [`Recovery::Lost`]. **The directory is usable
    ///   and the contents are gone**, and those are two different facts that a `bool` would merge.
    /// * `Countersigned` — recreate and call it [`Recovery::Lost`] too: req/56 §2's cell is 「再署名
    ///   要」, and re-signing needs the ledger key, which a third party running a verifier does not
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
                (false, _, Nature::Meta) => {
                    std::fs::write(&full, default_contents(path.rel))
                        .map_err(io("write", &full))?;
                    Recovery::Initialised
                }
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

/// What a freshly created file holds.
fn default_contents(rel: &str) -> String {
    match rel {
        "VERSION" => format!("{LAYOUT_VERSION}\n"),
        // req/56 §2: 「project-local 設定(secrets 禁)」, and §3 is why the file is empty rather than
        // pre-filled with a key path: 「秘密鍵=`~/.gx/keys/`(user home・0600)。project 側は**公開
        // keyid の参照のみ**」. A template with a `key = ` line in it is an invitation to put one
        // here, and the four tools req/11 批10 observed all keep credentials out of the project.
        "config.toml" => "# gx project-local settings (req/56 §2). No secrets: keys live in\n# ~/.gx/keys/ (req/56 §3), and this file may hold public key ids only.\n".to_string(),
        _ => String::new(),
    }
}
