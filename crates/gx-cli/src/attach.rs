// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1a** (`req/535` §3 R-1) — `gx attach`: put `.gx/` on a tree that is already running, and
//! say what was put there.
//!
//! # The gap this closes
//!
//! `req/535` §1 read the `Command` enum and counted twenty-two top-level verbs with no `init`, no
//! `attach` and no `detach` among them. `.gx/` was never absent from this binary — every verb that
//! writes calls [`crate::layout::Layout::create`] on the way in — but it arrived as the **side
//! effect of whichever verb ran first**, and nothing anywhere reported what that first verb had
//! made. R-1a is the repair, and it is deliberately narrow: one operation, and an enumeration.
//!
//! The `init` reading is absorbed here rather than given a second verb. A directory that has never
//! been a project takes `Layout::create`'s own init road, which is the road `gx submit` has always
//! taken; what is new is that a machine is told the outcome path by path. A separate `gx init`
//! would be a second door onto one road, and `req/242` H-01's finding is what two doors onto one
//! road cost.
//!
//! # 🔴 What this face does **not** answer
//!
//! `req/535` §2 defines an attach as three parts — the placement, a route pointed at the membrane,
//! and a statement of which of the four questions the resulting face can answer. This module is the
//! **first** part only (`req/535` §8's P-1a). The other two are P-1b and P-1c, and rather than
//! leaving them off the answer, [`run`] names them: a reader of `gx attach`'s output is told that
//! nothing here routes an effect and that nothing here says what the face can observe. An answer
//! that omitted them would read as an attach that had done all three.
//!
//! # The classification, and why it is read off the disk twice
//!
//! Three words — created / already-present / not-placed — and each is derived from a pair of
//! observations rather than from what this module intended to do: the row's path is looked for
//! **before** `Layout::create` runs and **after** it returns.
//!
//! | before | after | word |
//! |:--|:--|:--|
//! | absent | present | `created` |
//! | present | present | `already-present` |
//! | absent | absent | `not-placed` |
//!
//! The fourth combination — present before, absent after — is a placement that **removed**
//! something, which no road in this repository builds (DR-43-7 (1)); it is refused as an internal
//! fault rather than filed under a word, because there is no honest word for it.
//!
//! Two rows reach `not-placed` on every run and they are the two `Layout::create` skips by
//! construction: `LOCK`, whose exclusion belongs to an open descriptor and not to a file, and
//! `ledger/*.torn.*`, which is a naming rule rather than a path. They are enumerated rather than
//! omitted, because "this attach did not place it" is the answer R-1a asks for and an absent row is
//! not an answer.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};

use crate::exit::Outcome;
use crate::layout::{GxPath, Layout, Nature, Shape, GX_PATHS};
use crate::{io, Error, Result};

/// The word for a row whose path this run made.
const CREATED: &str = "created";
/// The word for a row whose path was already there when this run started.
const ALREADY_PRESENT: &str = "already-present";
/// The word for a row this run neither made nor found.
const NOT_PLACED: &str = "not-placed";

/// 🔴 The questions this face does not answer, said in the answer rather than left out.
///
/// `req/405` §7's adequacy condition is that a face is judged by whether it can write down what it
/// cannot take, and `req/535` §2's definition makes a placement without that statement *not an
/// attach*. This is the smallest honest form of it for a lane that implements the placement alone:
/// name the two parts that are not here and the document that owns each.
///
/// # 🔴 **P-1b / R-3g** — it was three sentences and it is two, and **only** the middle one went
///
/// `req/544` R-3g: P-1b answers the second of P-1a's three — *what this project can and cannot
/// observe about a change* — and it answers it as a table (`coverage`, [`crate::face`]) rather
/// than as a sentence. The other two are **carried through byte for byte**:
///
/// * the first is R-2's, which is `gx wrap --adopt-config`'s road and a separate lane;
/// * the third is P-1c's, and there is still no inverse.
///
/// Removing all three would have made an attach that answers a placement read as an attach that
/// answers a route and an exit as well, which is the exact over-claim `req/405` §7 judges a face
/// on. The two survivors are pinned verbatim against the specimen frozen **before** this lane in
/// `tests/fixtures/attach_face_frozen/issued_2026_08_22/attach.json`, so "verbatim" is measured
/// against a document this lane cannot edit without the freeze probe seeing it, rather than against
/// a second copy of the same string.
const NOT_CARRIED_BY_THIS_FACE: [&str; 2] = [
    "which effects reach the membrane: this operation points no route at gx. `gx wrap \
     --adopt-config` is the road that does, and it is a separate invocation",
    // 🔴 **P-1c** (`req/551` D-11) — this used to say the operation had no inverse. It has one now,
    // for the route and only for the route, and the half that stays true is the half about `.gx/`.
    // The old sentence is kept below rather than deleted, for the reason the one above it was.
    "how to leave: `gx wrap --detach-config <PATH> --server-name <NAME>` puts the entry back to the \
     command it ran before, and names what did not come back with it. `.gx/` is not removed by any \
     verb of this binary, so what it holds survives whatever happens next — including the leaving",
];

/// 🔴 The sentence [`NOT_CARRIED_BY_THIS_FACE`] carried until a detach existed to contradict it.
///
/// `no-delete`, and the same reason as [`ANSWERED_BY_THE_COVERAGE_TABLE`]: a reader who saw this
/// answer before should be able to see that the claim changed, and see what it changed from,
/// without going to find a diff.
const ANSWERED_BY_THE_DETACH_MODE: &str =
    "how to leave: this operation has no inverse yet. `.gx/` is not removed by any verb of this \
     binary, so what it holds survives whatever happens next";

/// 🔴 The sentence [`NOT_CARRIED_BY_THIS_FACE`] no longer holds, kept here because it says what the
/// coverage table replaced.
///
/// `no-delete`: a reader of the answer is told which item became a table, so that "there were three
/// and now there are two" is a statement in the output and not a diff somebody has to find.
const ANSWERED_BY_THE_COVERAGE_TABLE: &str =
    "what this project can and cannot observe about a change: nothing here states it, so nothing \
     here should be read as stating it";

/// One row of [`GX_PATHS`], as this run found and left it.
struct Row {
    path: &'static GxPath,
    before: bool,
    after: bool,
}

impl Row {
    /// Which of the three words this row takes, or the fault that has no word.
    fn class(&self, root: &Path) -> Result<&'static str> {
        match (self.before, self.after) {
            (false, true) => Ok(CREATED),
            (true, true) => Ok(ALREADY_PRESENT),
            (false, false) => Ok(NOT_PLACED),
            // No road builds this. It is refused rather than named because every word above would
            // be false of it, and a placement that reported a removal under "not-placed" would be
            // the shape `req/56` §5's reporting rule exists to forbid.
            (true, false) => Err(Error::Usage {
                detail: format!(
                    "`.gx/{}` was there before this operation and is not there after it. Nothing \
                     in this binary removes a declared path, so this is a fault rather than an \
                     outcome: read {} and do not re-run until what removed it is known",
                    self.path.rel,
                    root.display()
                ),
            }),
        }
    }
}

/// `Shape`, as a word. An exhaustive match rather than a `Debug` print: a variant added to the
/// table later stops this file compiling, which is the seam that keeps the answer complete.
const fn shape_word(shape: Shape) -> &'static str {
    match shape {
        Shape::Dir => "directory",
        Shape::File => "file",
        Shape::Pattern => "name-rule",
    }
}

/// `Nature`, as a word, and the sentence that says what losing the path means.
///
/// 🔴 **R-1c**. `req/535` R-1c asks for three of these to survive into the answer — the source, the
/// derived and the countersigned — and the table declares **five**. All five are printed. Reporting
/// three of five would be a placement report that files `config.toml` and `LOCK` under whichever
/// word was nearest, and the two sentences below are exactly the ones a reader needs to tell "this
/// can be made again" from "this cannot".
const fn nature_word(nature: Nature) -> (&'static str, &'static str) {
    match nature {
        Nature::Source => (
            "source",
            "nothing regenerates what this holds; losing it loses data",
        ),
        Nature::Derived => (
            "derived",
            "rebuilding it is always correct, so losing it costs time and not data",
        ),
        Nature::Countersigned => (
            "countersigned",
            "re-derivable, but only by the holder of the ledger signing key",
        ),
        Nature::Meta => (
            "meta",
            "settings and the declaration; losing it is a reconfiguration",
        ),
        Nature::Transient => (
            "transient",
            "the state of a running process, carrying no project data of its own",
        ),
    }
}

/// Does this row's path exist right now?
///
/// `Shape::Pattern` is answered `false` without asking the filesystem, and that is the honest
/// answer rather than a shortcut: `ledger/*.torn.*` is a rule for naming a copy of a torn tail, so
/// there is no path here to test, and `symlink_metadata` on the literal string would answer a
/// question nobody asked. The row still appears in the enumeration under `not-placed`, with its
/// reason beside it.
fn present(root: &Path, path: &GxPath) -> bool {
    if path.shape == Shape::Pattern {
        return false;
    }
    // `symlink_metadata` for [`Layout::declared_directories_are_directories`]'s reason: a symbolic
    // link where a declared path belongs is something that is **there**, whatever it points at, and
    // an attach that called it absent would go on to report having created a path it did not.
    std::fs::symlink_metadata(root.join(path.rel)).is_ok()
}

/// Every entry under `root`, project-relative, with `/` separators, sorted.
///
/// Only under `.gx/`, and that bound is the point rather than an economy: this operation writes
/// nowhere else, so a walk of the whole project would be reading somebody's `node_modules` in order
/// to answer a question about a directory gx owns. What the tree gained **outside** `.gx/` is
/// answered by [`root_entries`] one level up, where the only entry an attach can add is `.gx`
/// itself.
///
/// # 🔴 R44 lane B, item 2 (`req/591` §4, `req/38` §369) — `unreadable` is not an error path
///
/// A `read_dir` that fails here used to return silently: the directory's own name was already in
/// `out` (inserted by the caller before it recursed), so `placed_entries` did not lose the row, but
/// everything **inside** the directory was gone from both the "before" and the "after" walk, and a
/// diff of two things missing the same member finds nothing — `docs/LIMITS.md`'s standing
/// disclosure names this as a walk that "cannot `stat` a child" and "does not descend into it",
/// silent rather than false. `unreadable` is where the silence now goes: every directory this call
/// could not list is pushed onto it, project-relative, in the same form `out` already carries its
/// names in. The set is not deduplicated here because [`BTreeSet`] already refuses a repeat.
fn walk(project: &Path, dir: &Path, out: &mut BTreeSet<String>, unreadable: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        unreadable.insert(
            dir.strip_prefix(project)
                .unwrap_or(dir)
                .to_string_lossy()
                .replace('\\', "/"),
        );
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(project)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.insert(rel);
        // `symlink_metadata`, so that a symbolic link inside `.gx/` is listed and not followed: a
        // walk that descended through one would report entries that live somewhere else as entries
        // this operation added.
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            walk(project, &path, out, unreadable);
        }
    }
}

/// The names directly under the project directory. Not recursive, and see [`walk`].
fn root_entries(project: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(project) else {
        return out;
    };
    for entry in entries.flatten() {
        out.insert(entry.file_name().to_string_lossy().into_owned());
    }
    out
}

/// Everything under `.gx/`, plus `.gx` itself when it is there, and — R44 lane B item 2 — every
/// directory beneath it this call could not list.
fn placed_entries(project: &Path, root: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut out = BTreeSet::new();
    let mut unreadable = BTreeSet::new();
    if std::fs::symlink_metadata(root).is_err() {
        return (out, unreadable);
    }
    out.insert(
        root.strip_prefix(project)
            .unwrap_or(root)
            .to_string_lossy()
            .replace('\\', "/"),
    );
    walk(project, root, &mut out, &mut unreadable);
    (out, unreadable)
}

/// 🔴 **R-1a** — place `.gx/` for `project`, and answer with what was placed.
///
/// # What it does, in order
///
/// 1. reads the state of all eleven declared paths, and of the directory around them;
/// 2. runs [`Layout::create`], which is the one road in this binary that turns a directory into a
///    project — no second implementation of the placement is written here, for the reason
///    `req/242` H-01 gives about two doors onto one road;
/// 3. reads the same state again;
/// 4. classifies each row from the pair, and enumerates every one of them.
///
/// # 🔴 What it does not do
///
/// It writes nothing outside `.gx/`. It edits no `.gitignore` of the project, no CI configuration
/// and no hook — `req/535` R-1b puts those on the far side of a different operation with an inverse,
/// and this one has no inverse — and it opens no socket. The two facts a reader can check the first
/// of against the answer are `project_root_entries_added`, which is the whole of what this
/// operation put at the top of the tree, and `created_entries`, which is the whole of what it put
/// inside `.gx/`.
///
/// # Errors
/// Whatever [`Layout::create`] refuses with: a project that has recorded commits and lost its
/// journal, a declaration that will not read, a declared directory whose path holds a file. Those
/// refusals are the door's and are not re-spelled here — a second sentence about a project's state,
/// written by a placement report, is a second opinion about it.
/// 🔴 **P-1b** — what an attach was told about the route and about what somebody declares.
///
/// Both halves are optional and both are **inputs**, never outputs: the route half is a path to an
/// agent's own configuration file, which `gx_mcp_wire::config::check` reads, and the declared half
/// is a path to a file whose contents a person wrote. An attach given neither still prints a
/// coverage table — the honest one, in which this operation observes nothing.
#[derive(Debug, Default, Clone)]
pub struct CoverageInput {
    /// The agent configuration to read the route out of, and the entry in it this face is about.
    pub route: Option<(std::path::PathBuf, String)>,
    /// The declarations somebody wrote down.
    pub declared: Option<std::path::PathBuf>,
}

/// Build the face-level declaration for this attach.
///
/// 🔴 The route is **read**, never asked for: `gx_mcp_wire::config::check` is what
/// `gx wrap --check-config` already runs, and a second implementation of "is this route in place"
/// would be a second opinion (`req/538` §3-1 is the measurement that says where this lives).
fn face_declaration(input: &CoverageInput) -> Result<crate::face::FaceDeclaration> {
    let report = match &input.route {
        Some((path, name)) => {
            let raw = std::fs::read(path).map_err(io("read", path))?;
            let document: Value =
                serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
                    what: "agent configuration",
                    path: path.display().to_string(),
                    detail: detail.to_string(),
                })?;
            Some(gx_mcp_wire::config::check(&document, name))
        }
        None => None,
    };
    let declared = match &input.declared {
        Some(path) => crate::face::read_declared(path)?,
        None => std::collections::BTreeMap::new(),
    };
    Ok(crate::face::FaceDeclaration {
        face: report
            .as_ref()
            .map_or_else(|| "unrouted".to_string(), |r| r.name.clone()),
        route: report.as_ref().map(gx_mcp_wire::config::Report::to_json),
        posture: crate::face::posture_from_route(report.as_ref()),
        declared,
    })
}

pub fn run(project: &Path, coverage: &CoverageInput) -> Result<Outcome> {
    let root = Layout::path_for(project);

    let before_rows: Vec<bool> = GX_PATHS.iter().map(|p| present(&root, p)).collect();
    let (before_placed, _before_unreadable) = placed_entries(project, &root);
    let before_root = root_entries(project);

    Layout::create(project)?;

    // 🔴 **P-1b** — the declaration is built and written here, *between* the two observations, so
    // that a side-car this operation wrote shows up in `created_entries` like everything else it
    // put inside `.gx/`. A file written after the second observation would be a file the answer did
    // not name, which is the shape AC-2' exists to refuse.
    let face = face_declaration(coverage)?;
    let side_car = if coverage.route.is_some() || coverage.declared.is_some() {
        Some(face.write(&root)?)
    } else {
        None
    };

    let mut rows = Vec::with_capacity(GX_PATHS.len());
    let (mut created, mut already, mut not_placed) = (0u64, 0u64, 0u64);
    for (path, before) in GX_PATHS.iter().zip(before_rows) {
        let row = Row {
            path,
            before,
            after: present(&root, path),
        };
        let class = row.class(&root)?;
        match class {
            CREATED => created += 1,
            ALREADY_PRESENT => already += 1,
            _ => not_placed += 1,
        }
        let (nature, losing_it) = nature_word(path.nature);
        let mut entry = json!({
            "rel": path.rel,
            "shape": shape_word(path.shape),
            "nature": nature,
            "losing_it": losing_it,
            "placement": class,
        });
        // The two rows nothing places carry the reason with them. `req/56` §5's rule is that a
        // report declares what was lost and what was regenerated; the same rule read forwards is
        // that a row reported as untouched says why, or an operator reads it as a failure.
        if class == NOT_PLACED {
            if let Value::Object(map) = &mut entry {
                map.insert(
                    "why".to_string(),
                    json!(if path.shape == Shape::Pattern {
                        "a rule for naming a copy of a torn tail, not a path: nothing creates one \
                         and nothing removes one"
                    } else {
                        "the state of a running process. Creating it would make a file that looks \
                         like an exclusion and holds none — the exclusion is an open descriptor's"
                    }),
                );
            }
        }
        rows.push(entry);
    }

    let (after_placed, after_unreadable) = placed_entries(project, &root);
    let after_root = root_entries(project);
    let created_entries: Vec<&String> = after_placed.difference(&before_placed).collect();
    let root_added: Vec<&String> = after_root.difference(&before_root).collect();

    // 🔴 The path is reported as this process resolved it, and `canonicalize` is asked **after** the
    // directory exists so that a `--project` naming a relative path answers with the same string a
    // reader would type. A failure to canonicalize is not a refusal: it means the answer carries
    // the path as given, which is still true, and a placement that refused over a display string
    // would be a verb that failed after it had succeeded.
    let shown = |p: &Path| -> String {
        std::fs::canonicalize(p)
            .unwrap_or_else(|_| p.to_path_buf())
            .display()
            .to_string()
    };
    let cwd = std::env::current_dir().map_err(io("read", Path::new(".")))?;
    let under_cwd = std::fs::canonicalize(&root)
        .ok()
        .zip(std::fs::canonicalize(&cwd).ok())
        .map(|(gx_dir, here)| gx_dir.starts_with(&here));

    Ok(Outcome::ok(json!({
        "gx": "attach",
        "project": shown(project),
        "gx_dir": shown(&root),
        // 🔴 **R-1d**. A `.gx/` outside the tree the operator is standing in is the road for a
        // read-only checkout and for a repository whose own gate refuses an untracked entry
        // (`req/535` §7 KA-1), and which of the two happened is a fact rather than an intention:
        // it is read back off the two canonical paths. `null` when either will not canonicalize.
        "gx_dir_under_working_directory": under_cwd,
        "working_directory": shown(&cwd),
        "placement": rows,
        "counts": {
            "total": GX_PATHS.len(),
            "created": created,
            "already_present": already,
            "not_placed": not_placed,
        },
        // Everything this operation put inside `.gx/`, including the entries that are not rows of
        // the table — `.gx/.gitignore` (req/56 §4, and the reason `git status` says nothing after
        // an attach) and `.gx/ledger/journal`.
        "created_entries": created_entries,
        // 🔴 R44 lane B, item 2 (`req/591` §4, `req/38` §369) — every directory under `.gx/` this
        // walk could not list, project-relative. Always present and empty when there is nothing to
        // name: `created_entries` cannot show a file that lives inside one of these, because the
        // walk never reached it, and this is the field that says why rather than leaving the gap
        // silent (`docs/LIMITS.md`'s standing disclosure on `attach.rs`'s walk).
        "unreadable_entries": after_unreadable,
        // Everything it put at the top of the project, which is `.gx` and nothing else.
        "project_root_entries_added": root_added,
        "network": "none",
        // 🔴 **P-1b / R-3g** — two sentences, not three. The one that left became the table below,
        // and it is named here rather than silently dropped.
        "not_carried_by_this_face": NOT_CARRIED_BY_THIS_FACE,
        "now_answered_by_coverage": ANSWERED_BY_THE_COVERAGE_TABLE,
        // 🔴 **P-1c / `req/551` AC-20** — what the second item said before a detach existed.
        "now_answered_by_detach": ANSWERED_BY_THE_DETACH_MODE,
        // 🔴 **P-1b / R-3** — the face-level coverage declaration: four questions, one word each,
        // and not one of those words is `measured` (`req/38` §313 ruling 2).
        "coverage": face.to_json(),
        // Where the declaration was written, or `null` when there was nothing to write down.
        "coverage_side_car": side_car.map(|p| p.display().to_string()),
    })))
}
