// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! A repository of one test's own, on a tmpfs, and the [`Fixture`] the 51 §7 harness runs against.
//!
//! # Why the tests do not write inside this repository
//!
//! 🔴 Two reasons, and only the second is the one `gx-adapter-fs` gives.
//!
//! 1. **`/mnt/c` is 9p over NTFS.** `gix` takes a `.lock` file beside every reference it edits and
//!    renames it into place; DrvFs does not give POSIX rename semantics (`research_bus/glovrex_m4/
//!    SURVIVORS.md` §A-4), so a test measuring a reference transaction there would be measuring the
//!    wrong filesystem while printing the right words.
//! 2. **A test that wrote into the gx repository would be a test that commits.** This crate's subject
//!    is a git repository, and the repository these tests run from is a git repository. Nothing
//!    prevents a mistyped locator from pointing at the second one except the sandbox, so the sandbox
//!    is not a convenience here — it is the only thing between a fixture and this project's history.
//!
//! Every byte goes to a **tmpfs** (`/dev/shm` by default) and [`filesystem_of`] proves it from
//! `/proc/self/mountinfo` rather than from the path's spelling.
//!
//! What tmpfs does **not** give is durability. Nothing in this crate's tests is evidence about crash
//! recovery, and that is said here rather than left for a reader to assume (req/29 §4's rule about a
//! skip and a pass).

// Every binary that says `mod support;` compiles the whole file, and no binary uses all of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gix::bstr::BString;
use gix::objs::tree::{Entry, EntryKind};
use gix::objs::{Commit, Tree};
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
use gix::refs::{FullName, Target};
use gix::ObjectId;

use gx_adapter_git::{GitAdapter, GitDelta, GitOp};
use gx_canon::cid::{self, Domain};
use gx_core::{Actor, ChangeContext, Cid, GoalBytes, Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{PlannedDelta, Result, SubstrateAdapter};
use gx_substrate_conformance::Fixture;

/// The environment variable that moves the sandbox, for a machine whose tmpfs is elsewhere.
pub const ROOT_ENV: &str = "GLOVREX_GIT_TEST_ROOT";

/// Where a sandbox is made unless [`ROOT_ENV`] says otherwise.
pub const DEFAULT_ROOT: &str = "/dev/shm";

/// The branch the harness's subject lives on.
pub const BRANCH: &str = "refs/heads/main";

/// A second branch, for changes that touch different things.
pub const OTHER_BRANCH: &str = "refs/heads/side";

/// A branch with no commits on it: git's "unborn" state, and this adapter's `Ok(None)`. (sem: SEM-gx-adapter-git-143)
pub const UNBORN_BRANCH: &str = "refs/heads/fresh";

/// The entry the harness's subject is.
pub const SUBJECT: &str = "subject.txt";

/// The content a fresh sandbox starts with.
pub const BEFORE: &[u8] = b"before\n";

/// What an intent asks the subject to become.
pub const GOAL: &[u8] = b"after\n";

/// What the second branch's commit holds, so that a reset between the two moves the tree hash.
pub const ELSEWHERE: &[u8] = b"elsewhere\n";

/// The filesystem type mounted over `path`, read from `/proc/self/mountinfo`.
///
/// The line format is `id parent major:minor root mountpoint options... - fstype source superopts`,
/// so the type is the field after the lone `-`. The mount point that answers is the **longest** one
/// that is a prefix of `path`, which is what makes `/dev/shm` win over `/`.
#[must_use]
pub fn filesystem_of(path: &Path) -> String {
    let target = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .expect("/proc/self/mountinfo is readable on Linux");

    let mut best: Option<(usize, String)> = None;
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(dash) = fields.iter().position(|f| *f == "-") else {
            continue;
        };
        let (Some(point), Some(fstype)) = (fields.get(4), fields.get(dash + 1)) else {
            continue;
        };
        let covers = target == *point
            || (target.starts_with(point)
                && (point.ends_with('/') || target[point.len()..].starts_with('/')));
        if covers && best.as_ref().is_none_or(|(len, _)| point.len() > *len) {
            best = Some((point.len(), (*fstype).to_string()));
        }
    }
    best.map_or_else(|| "unknown".to_string(), |(_, fstype)| fstype)
}

/// The directory sandboxes are made in, refusing anything that is not a tmpfs.
///
/// Fail-closed on purpose, for the two reasons the module documentation gives.
#[must_use]
pub fn tmpfs_root() -> PathBuf {
    let root = PathBuf::from(std::env::var(ROOT_ENV).unwrap_or_else(|_| DEFAULT_ROOT.to_string()));
    assert!(
        root.is_dir(),
        "{} is not a directory; set {ROOT_ENV} to a tmpfs",
        root.display()
    );
    let fstype = filesystem_of(&root);
    assert_eq!(
        fstype,
        "tmpfs",
        "{} is a {fstype}, and these tests have to write to a tmpfs (/mnt/c is 9p over NTFS and \
         does not give POSIX rename semantics, which is what a reference lock is renamed with). \
         Set {ROOT_ENV}.",
        root.display()
    );
    root
}

/// The signature every commit these tests write carries.
///
/// The adapter's own ([`gx_adapter_git::repo::gx_signature`]) is not reachable from here as a value
/// this file can compare against — it is, and `git_conformance.rs` asserts the two agree — so this is
/// written out rather than imported, which is what makes the assertion a comparison of two routes
/// rather than of one value with itself.
#[must_use]
pub fn fixture_signature() -> gix::actor::Signature {
    gix::actor::Signature {
        name: BString::from("gx-fixture"),
        email: BString::from("fixture@glovrex.invalid"),
        time: gix::date::Time {
            seconds: 0,
            offset: 0,
        },
    }
}

/// A git repository of one test's own, on a tmpfs, removed when the test ends.
pub struct Sandbox {
    dir: PathBuf,
    /// The commit `main` starts at, so that [`Sandbox::rewind`] can put it back.
    origin: ObjectId,
    /// A commit on another branch, for the reset AC-050 calls "branch-operation delta". (sem: SEM-gx-adapter-git-144)
    elsewhere: ObjectId,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Sandbox {
    /// Make one: a repository with `main` at a commit holding [`SUBJECT`], a `side` branch at a
    /// different commit, and `fresh` unborn.
    ///
    /// # Panics
    /// If the tmpfs or gitoxide will not answer, which is a broken machine rather than a failing
    /// claim.
    #[must_use]
    pub fn new() -> Self {
        let dir = tmpfs_root().join(format!(
            "glovrex-git-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a tmpfs accepts a directory");
        let repo = gix::init(&dir).expect("gitoxide creates a repository");

        let origin = write_commit(&repo, None, BEFORE);
        let elsewhere = write_commit(&repo, None, ELSEWHERE);
        set_reference(&repo, BRANCH, PreviousValue::Any, Target::Object(origin));
        set_reference(
            &repo,
            OTHER_BRANCH,
            PreviousValue::Any,
            Target::Object(elsewhere),
        );
        // HEAD is set explicitly rather than inherited: `gix::init` picks a default branch name from
        // configuration, and a fixture whose HEAD depended on whoever ran it would make AC-050's
        // "HEAD is bit-equal" a statement about that machine. (sem: SEM-gx-adapter-git-145)
        set_reference(
            &repo,
            "HEAD",
            PreviousValue::Any,
            Target::Symbolic(FullName::try_from(BRANCH).expect("a full reference name")),
        );

        Self {
            dir,
            origin,
            elsewhere,
        }
    }

    /// The repository root.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The commit `main` starts at.
    #[must_use]
    pub fn origin(&self) -> ObjectId {
        self.origin
    }

    /// A commit on another branch, holding different content at the same path.
    #[must_use]
    pub fn elsewhere(&self) -> ObjectId {
        self.elsewhere
    }

    /// The repository, opened afresh (nothing here holds one either).
    #[must_use]
    pub fn repository(&self) -> gix::Repository {
        gix::open(&self.dir).expect("the sandbox is a repository")
    }

    /// A locator for a branch and the subject entry on it.
    #[must_use]
    pub fn locator_on(&self, branch: &str) -> String {
        format!("{}#{branch}:{SUBJECT}", self.dir.display())
    }

    /// Put every branch back where [`Sandbox::new`] left it.
    pub fn rewind(&self) {
        let repo = self.repository();
        set_reference(
            &repo,
            BRANCH,
            PreviousValue::Any,
            Target::Object(self.origin),
        );
        set_reference(
            &repo,
            OTHER_BRANCH,
            PreviousValue::Any,
            Target::Object(self.elsewhere),
        );
        delete_reference(&repo, UNBORN_BRANCH);
    }

    /// Move `main` to a new commit, behind the adapter's back.
    ///
    /// This is [`Fixture::disturb`]'s work and it stands for "somebody else pushed", which is the (sem: SEM-gx-adapter-git-146)
    /// situation CON-2's CAS check exists for. It moves both things the harness watches: the object's
    /// digest (the entry's bytes change) and the scope's (the branch tip changes).
    pub fn commit_over(&self, content: &[u8]) -> ObjectId {
        let repo = self.repository();
        let tip = self.tip_of(BRANCH);
        let new = write_commit(&repo, tip, content);
        set_reference(&repo, BRANCH, PreviousValue::Any, Target::Object(new));
        new
    }

    /// Where a branch points, or `None` when it has no commits.
    #[must_use]
    pub fn tip_of(&self, branch: &str) -> Option<ObjectId> {
        self.repository()
            .try_find_reference(branch)
            .expect("the reference store answers")
            .map(|mut r| r.peel_to_id().expect("the reference resolves").detach())
    }

    /// The tree hash **AC-050** compares, read through gitoxide rather than through the adapter.
    ///
    /// A second route on purpose: the adapter's own digest is a `gx-canon` blake3 over an entry's
    /// bytes, and AC-050 asks about git's own SHA-1 of a tree. An assertion that used the adapter's
    /// number would be asking the adapter whether it was right.
    #[must_use]
    pub fn head_and_tree(&self) -> (ObjectId, ObjectId) {
        let repo = self.repository();
        let head = repo.head_id().expect("HEAD resolves").detach();
        let tree = repo
            .find_object(head)
            .expect("HEAD is an object")
            .into_commit()
            .tree_id()
            .expect("a commit has a tree")
            .detach();
        (head, tree)
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Write a blob, a tree holding it at [`SUBJECT`], and a commit — the fixture's own route into a
/// repository, which is not the adapter's.
fn write_commit(repo: &gix::Repository, parent: Option<ObjectId>, content: &[u8]) -> ObjectId {
    let blob = repo
        .write_blob(content)
        .expect("a blob is written")
        .detach();
    let tree = repo
        .write_object(&Tree {
            entries: vec![Entry {
                mode: EntryKind::Blob.into(),
                filename: BString::from(SUBJECT),
                oid: blob,
            }],
        })
        .expect("a tree is written")
        .detach();
    let signature = fixture_signature();
    repo.write_object(&Commit {
        tree,
        parents: parent.into_iter().collect(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: BString::from("fixture\n"),
        extra_headers: Vec::new(),
    })
    .expect("a commit is written")
    .detach()
}

/// Set a reference, with no expectation about where it was.
///
/// `PreviousValue::Any` because this is the fixture and not the adapter: the adapter's own
/// [`gx_adapter_git::repo`] takes the tip it just read as the expected value, and a fixture that did
/// the same would be re-implementing the thing under test.
fn set_reference(repo: &gix::Repository, name: &str, expected: PreviousValue, new: Target) {
    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: BString::from("fixture"),
            },
            expected,
            new,
        },
        name: FullName::try_from(name).expect("a full reference name"),
        deref: false,
    };
    let signature = fixture_signature();
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
}

/// Remove a reference if it is there, so that [`UNBORN_BRANCH`] is unborn again after a test made it.
fn delete_reference(repo: &gix::Repository, name: &str) {
    let full = FullName::try_from(name).expect("a full reference name");
    if repo
        .try_find_reference(name)
        .expect("the reference store answers")
        .is_none()
    {
        return;
    }
    let edit = RefEdit {
        change: Change::Delete {
            expected: PreviousValue::Any,
            log: RefLog::AndReference,
        },
        name: full,
        deref: false,
    };
    let signature = fixture_signature();
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
}

/// An intent this adapter can plan.
#[must_use]
pub fn intent_for(locator: &str, goal: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Git,
        locator.to_string(),
        GoalBytes(goal.to_vec()),
        ChangeContext::Evidence,
        Actor::Human {
            key: "conformance".to_string(),
        },
    )
}

/// A delta this adapter planned, for tests that need one.
///
/// # Panics
/// If the adapter refuses to plan, which for a well-formed locator it does not.
#[must_use]
pub fn planned(adapter: &GitAdapter, locator: &str, goal: &[u8]) -> PlannedDelta {
    let pre = adapter
        .snapshot(locator)
        .expect("the sandbox holds this entry");
    adapter
        .plan(&intent_for(locator, goal), &pre)
        .expect("the adapter plans an entry change")
}

/// A reset, written in the grammar rather than planned -- **AC-050**'s "branch-operation delta". (sem: SEM-gx-adapter-git-147)
///
/// 42 §3.3 gives an [`Intent`] a `goal` and no spelling for "put this branch there", so `plan`
/// produces entry changes only and a branch operation has to be constructed here. §32 M4H4-5 confirmed
/// fixed exactly that division for the fs adapter's removal: "the grammar seat is the AC's **premise**
/// and not an implementation-ahead". (sem: SEM-gx-adapter-git-148)
///
/// # Panics
/// If the sequence has no canonical form, which a one-element array of four fields always has.
#[must_use]
pub fn reset_to(locator: &str, target: ObjectId) -> PlannedDelta {
    let payload = GitDelta::one(GitOp::reset(
        gx_adapter_git::normalize(locator),
        target.to_string(),
    ))
    .encode()
    .expect("a one-operation sequence has a canonical DAG-CBOR form");
    PlannedDelta::new(SubstrateKind::Git, payload).expect("the projection is encodable")
}

/// An entry change, written in the grammar, for a position `plan` would refuse to snapshot.
///
/// # Panics
/// As [`reset_to`].
#[must_use]
pub fn spelled(locator: &str, content: &[u8]) -> PlannedDelta {
    let payload = GitDelta::one(GitOp::write(locator.to_string(), content.to_vec()))
        .encode()
        .expect("a one-operation sequence has a canonical DAG-CBOR form");
    PlannedDelta::new(SubstrateKind::Git, payload).expect("the projection is encodable")
}

/// The `pre` of a change to a position that holds nothing.
///
/// 🔴 **Constructed here because the adapter will not produce one.** `GitAdapter::snapshot` reads the
/// branch and answers `Unreadable` when it has no commits, so v0.1 has no way to describe "this
/// branch is unborn" as a snapshot -- and the `Ok(None)` of 51 §7 contract 5 is quantified at exactly (sem: SEM-gx-adapter-git-149)
/// that state. The digest is the digest of no content, which is the same value an **empty** entry
/// would have; `gx_adapter_git::repo::absent_digest` discloses the collision.
///
/// # Panics
/// Never: the projection of a snapshot always has a canonical form.
#[must_use]
pub fn absent_snapshot(locator: &str) -> ObjectSnapshot {
    let normalised = gx_adapter_git::normalize(locator);
    let digest = cid::mint(Domain::Leaf, &[&[][..]]);
    let placeholder = ObjectSnapshot::new(
        gx_core::ObjectId(Cid([0u8; 32])),
        SubstrateKind::Git,
        normalised.clone(),
        digest,
        gx_core::ReprKind::Bytes,
    );
    let id = cid::compute(&placeholder).expect("a snapshot projects to a canonical form");
    ObjectSnapshot::new(
        gx_core::ObjectId(id),
        SubstrateKind::Git,
        normalised,
        digest,
        gx_core::ReprKind::Bytes,
    )
}

/// The 51 §7 harness's view of this adapter.
///
/// 51 §7, verbatim: "each adapter crate only calls this harness from one `#[test]` to inherit the contract tests", so what (sem: SEM-gx-adapter-git-150)
/// an adapter author writes is this type and nothing else. Its length is the **second** measurement of
/// whether the eleven-method `Fixture` of M4 hand 3 is a reasonable ask (**M4H3-9**: "whether M7's git/mcp can
/// inherit the same shape is **unverified** (because there are 0 real adapters)"), and `git_conformance.rs` prints it (sem: SEM-gx-adapter-git-151)
/// beside the fs adapter's number.
pub struct GitFixture {
    sandbox: Sandbox,
    adapter: GitAdapter,
}

impl GitFixture {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sandbox: Sandbox::new(),
            adapter: GitAdapter::new(),
        }
    }

    #[must_use]
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    #[must_use]
    pub fn git(&self) -> &GitAdapter {
        &self.adapter
    }
}

impl Default for GitFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture for GitFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        &self.adapter
    }

    fn locator(&self) -> String {
        gx_adapter_git::normalize(&self.sandbox.locator_on(BRANCH))
    }

    fn intent(&self) -> Intent {
        intent_for(&self.locator(), GOAL)
    }

    fn reset(&self) -> Result<()> {
        self.sandbox.rewind();
        Ok(())
    }

    fn disturb(&self) -> Result<()> {
        self.sandbox.commit_over(b"somebody else pushed\n");
        Ok(())
    }

    /// The adapter's own normalisation, which is the subject of L7 (**E-M4-12**).
    fn normalise(&self, locator: &str) -> Option<String> {
        Some(gx_adapter_git::normalize(locator))
    }

    /// **L5's first route**: the digest a plan of this intent promises, derived from the goal and
    /// from nothing else.
    ///
    /// The two routes are the fs adapter's two, one substrate over:
    ///
    /// 1. **here** — the intent's [`GOAL`] bytes, digested without a repository being opened at all.
    ///    This is what `Transformation.target` (41 §3) would carry, which is why 51 §7's harness takes
    ///    it from the fixture: the prophecy is the engine's value and not the adapter's.
    /// 2. **`apply`** — the bytes read back out of the object database after the reference moved,
    ///    which is why `AppliedDelta.resulting_digest` is an observation (req/69 §3.1).
    ///
    /// What the comparison cannot catch is the same residue: both routes end in the one digest
    /// function 41 §6 admits, so a bug **inside** `cid::mint` is invisible to it. What it does catch is
    /// the whole space between the goal and the object database — a tree written without the entry, a
    /// commit that never became the branch's tip, a payload carrying something other than the goal.
    fn promised_target(&self) -> Option<Cid> {
        Some(cid::mint(Domain::Leaf, &[GOAL]))
    }

    /// A delta whose inverse this adapter cannot construct: a change to a file on an **unborn**
    /// branch.
    ///
    /// 51 §7 contract 5 needs a subject only the adapter's author can produce, and the crate root
    /// argues why git's is not the fs adapter's escrow ceiling: a git inverse carries an object id, so
    /// no input can make it too large. What it cannot carry is "the branch was not there" -- undoing (sem: SEM-gx-adapter-git-152)
    /// the first commit on a branch means **deleting** the branch, and v0.1's grammar spells no
    /// deletion. `invert` answers `Ok(None)` and **E-M3-4** asks a person.
    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        let locator = self.sandbox.locator_on(UNBORN_BRANCH);
        let pre = absent_snapshot(&locator);
        let delta = self.adapter.plan(&intent_for(&locator, GOAL), &pre).ok()?;
        Some((delta, pre))
    }

    /// 51 §7's commuting case: two changes on **two branches**. (sem: SEM-gx-adapter-git-153)
    ///
    /// Not "two files" -- the crate root argues at length why a git change's footprint is its branch, (sem: SEM-gx-adapter-git-154)
    /// and `git_commutation.rs` measures the case that difference decides.
    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        Some((
            planned(&self.adapter, &self.locator(), b"one\n"),
            planned(
                &self.adapter,
                &gx_adapter_git::normalize(&self.sandbox.locator_on(OTHER_BRANCH)),
                b"two\n",
            ),
        ))
    }

    /// 51 §7's non-commuting case: two changes on one branch. (sem: SEM-gx-adapter-git-155)
    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        let subject = self.locator();
        Some((
            planned(&self.adapter, &subject, b"one\n"),
            planned(&self.adapter, &subject, b"two\n"),
        ))
    }

    /// Pairs this adapter's `≈` calls equal, one per clause of the crate root that produces two
    /// spellings of one position.
    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        let root = self.sandbox.dir().display().to_string();
        let canonical = self.locator();
        vec![
            // clause 1: a dot segment in the repository path.
            (format!("{root}/.#{BRANCH}:{SUBJECT}"), canonical.clone()),
            // clause 1: a `..` that cancels.
            (
                format!("{root}/nowhere/..#{BRANCH}:{SUBJECT}"),
                canonical.clone(),
            ),
            // clause 2: an unqualified reference is a branch.
            (format!("{root}#main:{SUBJECT}"), canonical.clone()),
            // clause 3: a leading separator on a tree path, which has no root.
            (format!("{root}#heads/main:/{SUBJECT}"), canonical),
        ]
    }
}
