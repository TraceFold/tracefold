//! A sandbox on a real tmpfs, the measurement M4-24 fixes, and the [`Fixture`] the 51 §7 harness is
//! run against.
//!
//! # Why the tests do not write inside the repository
//!
//! 🔴 The repository lives on `/mnt/c`, which WSL mounts as **9p/DrvFs over NTFS**. A rename there
//! goes through `MoveFileEx`, and POSIX rename atomicity -- the property M4-13 (a) buys v0.1's
//! `apply` -- is **not** guaranteed by it. `research_bus/glovrex_m4/SURVIVORS.md` §A-4 raised that as
//! a design gotcha, and hand 5 fetched the primaries in full (`Desktop/GitRepo/REFERENCES.md`,
//! 2026-08-09) -- hand 4 had confirmed the same three through search snippets, and its paraphrase of
//! the Open Group's sentence is corrected here to the words the page actually carries: 「a directory
//! entry named new shall remain visible to other threads throughout the renaming operation and refer
//! either to the file referred to by new or old before the operation began」, **within one
//! filesystem** (across one, `EXDEV`). Rust's `std::fs::rename` documents replacement without
//! documenting atomicity at all, because the guarantee is the filesystem's.
//!
//! So every byte these tests write goes to a **tmpfs** (`/dev/shm` by default), and
//! [`filesystem_of`] proves it from `/proc/self/mountinfo` rather than from the path's spelling. A
//! test that measured atomicity on DrvFs would be measuring the wrong filesystem while printing the
//! right words. AC-049 (hand 5) asks for tmpfs 逐語; hand 4 puts the requirement in place one hand
//! early because AC-047's 「substrate 状態が変化しない」 is a claim about a filesystem too.
//!
//! What tmpfs does **not** give is durability: `fsync` there is effectively a no-op, so nothing in
//! this crate's tests is evidence about crash recovery. That is said here rather than left for a
//! reader to assume (req/29 §4's rule about a skip and a pass).

// Every binary that says `mod support;` compiles the whole file, and no binary uses all of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gx_adapter_fs::{FsAdapter, FsDelta, FsOp, MAX_INVERSE_PAYLOAD_BYTES};
use gx_canon::cid::{self, Domain};
use gx_core::{Actor, ChangeContext, Cid, GoalBytes, Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result, SubstrateAdapter};
use gx_substrate_conformance::Fixture;

/// The environment variable that moves the sandbox, for a machine whose tmpfs is elsewhere.
pub const ROOT_ENV: &str = "GLOVREX_FS_TEST_ROOT";

/// Where a sandbox is made unless [`ROOT_ENV`] says otherwise.
pub const DEFAULT_ROOT: &str = "/dev/shm";

/// What the harness's subject is called inside a sandbox.
pub const SUBJECT: &str = "subject";

/// A second file, for changes that touch different things.
pub const OTHER: &str = "beside";

/// A file whose inverse would not fit in the escrow (**M4-21**, AC-048's `Ok(None)`).
///
/// Written lazily by [`FsFixture::uninvertible`] rather than by [`Sandbox::populate`], because
/// [`Fixture::reset`] runs before each of the fifteen obligations and a megabyte per obligation would
/// be paid fifteen times to be read once.
pub const HEAVY: &str = "heavy";

/// The content a fresh sandbox starts with.
pub const BEFORE: &[u8] = b"before";

/// What an intent asks the subject to become.
pub const GOAL: &[u8] = b"after";

/// The filesystem type mounted over `path`, read from `/proc/self/mountinfo`.
///
/// The line format is `id parent major:minor root mountpoint options... - fstype source superopts`,
/// so the type is the field after the lone `-`. The mount point that answers is the **longest** one
/// that is a prefix of `path`, which is what makes `/dev/shm` win over `/`. Paths with spaces are
/// escaped as `\040` in that file and none of ours have any; a sandbox root with a space in it would
/// read as an unknown filesystem, which is a refusal rather than a wrong answer.
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
/// Fail-closed on purpose. A machine without a tmpfs cannot produce the evidence AC-047 and AC-049
/// are about, and running the tests on the repository's own 9p mount would produce green output
/// about a filesystem whose rename semantics are not the ones under test.
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
        "{} is a {fstype}, and these tests have to write to a tmpfs (SURVIVORS §A-4: /mnt/c is 9p \
         over NTFS and does not guarantee rename atomicity). Set {ROOT_ENV}.",
        root.display()
    );
    root
}

/// A directory of one test's own, on a tmpfs, removed when the test ends.
pub struct Sandbox {
    dir: PathBuf,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Sandbox {
    /// Make one, with the subject and its neighbour already in it.
    ///
    /// # Panics
    /// If the tmpfs cannot be written to, which is a broken machine rather than a failing claim.
    #[must_use]
    pub fn new() -> Self {
        let dir = tmpfs_root().join(format!(
            "glovrex-fs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a tmpfs accepts a directory");
        let sandbox = Self { dir };
        sandbox.populate();
        sandbox
    }

    fn populate(&self) {
        self.write(SUBJECT, BEFORE);
        self.write(OTHER, b"beside");
    }

    /// The sandbox root.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The absolute locator of a name inside the sandbox.
    #[must_use]
    pub fn locator(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    /// Write a file, creating parents.
    pub fn write(&self, name: &str, content: &[u8]) {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a tmpfs accepts a directory");
        }
        std::fs::write(&path, content).expect("a tmpfs accepts a file");
    }

    /// Read one back.
    #[must_use]
    pub fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).expect("the sandbox holds this file")
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

/// What **M4-24** fixes as the measurement of 「substrate 状態が変化しない」.
///
/// req/38 §28 逐語: 「AC-047 の「変化しない」の測定=**内容 digest+mtime+size**・atime は測らない(fs 依存
/// で不安定=測れない物を測ったふりにしない)・理由 doc 化」.
///
/// The digest is gx-canon's, over the bytes, which is the same road the adapter's own `precondition`
/// takes -- so a `plan` that rewrote the file would move this whether or not it also moved the
/// timestamps. `mtime` and `size` are metadata the adapter's fingerprint deliberately excludes
/// (ASM-69-1), and they are measured **here** for the opposite reason: this is the control, and a
/// control that could only see what the fingerprint sees would be the fingerprint.
///
/// **atime is not measured, and this is the reason.** A read moves it on some filesystems, is
/// deferred by `relatime`, and is off entirely under `noatime`; the tests would then be measuring
/// the mount options of whoever ran them. What cannot be measured stably is left out rather than
/// asserted loosely.
#[derive(Debug, PartialEq, Eq)]
pub struct StateOf {
    pub digest: Cid,
    pub mtime: std::time::SystemTime,
    pub size: u64,
}

/// Measure a file the way M4-24 says to.
///
/// # Panics
/// If the file cannot be read, which the caller has just created.
#[must_use]
pub fn state_of(path: &Path) -> StateOf {
    let content = std::fs::read(path).expect("the subject is there");
    let meta = std::fs::metadata(path).expect("the subject has metadata");
    StateOf {
        digest: cid::mint(Domain::Leaf, &[&content]),
        mtime: meta.modified().expect("tmpfs keeps mtimes"),
        size: meta.len(),
    }
}

/// An intent this adapter can plan.
#[must_use]
pub fn intent_for(locator: &str, goal: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        locator.to_string(),
        GoalBytes(goal.to_vec()),
        ChangeContext::Evidence,
        Actor::Human {
            key: "conformance".to_string(),
        },
    )
}

/// The 51 §7 harness's view of this adapter.
///
/// 51 §7 逐語: 「各adapterクレートはこのハーネスを`#[test]`から呼び出すのみで契約テストを継承する」, so
/// what an adapter author writes is this type and nothing else. Its length is the first measurement
/// of whether the eleven-method `Fixture` of hand 3 is a reasonable ask (**M4H3-9**), and the number
/// is printed by `tests/conformance.rs`.
pub struct FsFixture {
    sandbox: Sandbox,
    adapter: FsAdapter,
}

impl FsFixture {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sandbox: Sandbox::new(),
            adapter: FsAdapter::new(),
        }
    }

    #[must_use]
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }
}

impl Default for FsFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture for FsFixture {
    fn adapter(&self) -> &dyn SubstrateAdapter {
        &self.adapter
    }

    fn locator(&self) -> String {
        self.sandbox.locator(SUBJECT)
    }

    fn intent(&self) -> Intent {
        intent_for(&self.locator(), GOAL)
    }

    fn reset(&self) -> Result<()> {
        self.sandbox.write(SUBJECT, BEFORE);
        self.sandbox.write(OTHER, b"beside");
        Ok(())
    }

    fn disturb(&self) -> Result<()> {
        self.sandbox.write(SUBJECT, b"somebody else was here");
        Ok(())
    }

    /// The adapter's own normalisation, which is the subject of L7 (**E-M4-12**).
    fn normalise(&self, locator: &str) -> Option<String> {
        Some(gx_adapter_fs::normalize(locator))
    }

    /// **L5's first route** (**M4H3-3 採(a)**): the digest a plan of this intent promises, derived
    /// from the goal and from nothing else.
    ///
    /// §31 逐語: 「手5 DoD に「fs adapter は goal から導出した target と apply の観測値を**別経路で作り**
    /// 一致を測る」」. The two routes are:
    ///
    /// 1. **here** -- the intent's [`GOAL`] bytes, digested without the filesystem being touched at
    ///    all. This is what `Transformation.target` (41 §3) would carry, which is why 51 §7's harness
    ///    takes it from the fixture: the prophecy is the engine's value and not the adapter's.
    /// 2. **`apply`** -- the bytes the adapter reads back off the substrate after its `rename`, which
    ///    is why `AppliedDelta.resulting_digest` is an observation (req/69 §3.1: 「post は返り値でなく
    ///    観測値である」) rather than the buffer that was written.
    ///
    /// What the comparison cannot catch is said in `tests/ac_049.rs`: both routes end in the one
    /// digest function 41 §6 admits, so a bug **inside** `cid::mint` is invisible to it. What it does
    /// catch is the whole space between the goal and the bytes on disk -- a partial write, a rename
    /// into the wrong position, a stale page, a payload that carried something other than the goal.
    fn promised_target(&self) -> Option<Cid> {
        Some(cid::mint(Domain::Leaf, &[GOAL]))
    }

    /// A delta whose inverse exceeds [`gx_adapter_fs::MAX_INVERSE_PAYLOAD_BYTES`] (**M4-21**).
    ///
    /// 51 §7 contract 5 needs a subject only the adapter's author can produce, and this is the one
    /// **M4-21 採(a)** named: 「超過は `invert`=`Ok(None)`(**AC-048 の None の実在する第 1 の理由**)」.
    /// The old content is what an inverse has to carry (42 §5), so the subject is a file one byte over
    /// the ceiling and a plan that would replace it.
    fn uninvertible(&self) -> Option<(PlannedDelta, ObjectSnapshot)> {
        let locator = self.sandbox.locator(HEAVY);
        self.sandbox
            .write(HEAVY, &vec![b'H'; MAX_INVERSE_PAYLOAD_BYTES + 1]);
        let pre = self.adapter.snapshot(&locator).ok()?;
        let delta = self.adapter.plan(&intent_for(&locator, GOAL), &pre).ok()?;
        Some((delta, pre))
    }

    /// 51 §7's 可換 case, and AC-052's own example: 「別ファイルへの書き込み」.
    ///
    /// Planned rather than hand-written, so that the pair the harness compares is the pair an engine
    /// would carry. Both positions exist after [`Fixture::reset`], which is what lets `plan` answer.
    fn commuting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        let subject = self.locator();
        let beside = self.sandbox.locator(OTHER);
        Some((
            self.adapter
                .plan(
                    &intent_for(&subject, b"one"),
                    &self.adapter.snapshot(&subject).ok()?,
                )
                .ok()?,
            self.adapter
                .plan(
                    &intent_for(&beside, b"two"),
                    &self.adapter.snapshot(&beside).ok()?,
                )
                .ok()?,
        ))
    }

    /// 51 §7's 非可換 case: AC-052's 「同一ファイルへの競合書き込み」.
    ///
    /// Two whole-file replacements of one file. Nothing of the second is independent of the first --
    /// which is why the residual this adapter mints is the whole of it (see `src/commutation.rs`).
    fn conflicting_pair(&self) -> Option<(PlannedDelta, PlannedDelta)> {
        let subject = self.locator();
        let pre = self.adapter.snapshot(&subject).ok()?;
        Some((
            self.adapter
                .plan(&intent_for(&subject, b"one"), &pre)
                .ok()?,
            self.adapter
                .plan(&intent_for(&subject, b"two"), &pre)
                .ok()?,
        ))
    }

    /// Pairs this adapter's `≈` calls equal (42 §2.3), one per clause of E-M4-12.
    fn equivalent_spellings(&self) -> Vec<(String, String)> {
        let subject = self.locator();
        vec![
            (format!("{subject}/."), subject.clone()),
            (
                format!("{}/{}/../{}", self.sandbox.dir().display(), OTHER, SUBJECT),
                subject.clone(),
            ),
            (
                subject.replace(&format!("/{SUBJECT}"), &format!("//{SUBJECT}")),
                subject.clone(),
            ),
            (format!("{subject}/"), subject),
        ]
    }
}

/// The refusal a fixture reports when the sandbox itself will not answer.
#[must_use]
pub fn unreadable(locator: &str, e: &std::io::Error) -> Error {
    Error::Unreadable {
        locator: locator.to_string(),
        detail: e.to_string(),
    }
}

/// A snapshot of a locator, for tests that need one without asserting about it.
///
/// # Panics
/// If the adapter refuses, which the caller's sandbox has just made possible.
#[must_use]
pub fn snapshot_of(adapter: &FsAdapter, locator: &str) -> ObjectSnapshot {
    adapter.snapshot(locator).expect("the sandbox holds this")
}

/// A delta this adapter planned, for tests that need one.
///
/// # Panics
/// If the adapter refuses to plan.
#[must_use]
pub fn planned(adapter: &FsAdapter, locator: &str, goal: &[u8]) -> PlannedDelta {
    let pre = snapshot_of(adapter, locator);
    adapter
        .plan(&intent_for(locator, goal), &pre)
        .expect("the adapter plans a whole-file replacement")
}

/// A removal, written in the grammar rather than planned (**M4H4-5 採(a)**).
///
/// 42 §3.3 gives an [`Intent`] a `goal` and no spelling for 「remove this」, so `plan` produces
/// replacements only and 「削除」 -- one of AC-049's three -- has to be constructed here. §32 M4H4-5
/// 追認 fixed exactly that division: 「削除の綴り(`content: null`)の文法席は **AC-049(M4 の AC)の前提**
/// であって先行実装でない。produce/consume は手5」.
///
/// # Panics
/// If the sequence has no canonical form, which a one-element array of two fields always has.
#[must_use]
pub fn removal(locator: &str) -> PlannedDelta {
    let payload = FsDelta::one(FsOp::remove(gx_adapter_fs::normalize(locator)))
        .encode()
        .expect("a one-operation sequence has a canonical DAG-CBOR form");
    PlannedDelta::new(SubstrateKind::Fs, payload).expect("the projection is encodable")
}

/// A whole-file write, written in the grammar (the same road [`removal`] takes).
///
/// Used where the position does not exist yet, since `plan` needs a pre-snapshot and this adapter's
/// `snapshot` will not answer for a position that holds nothing (see [`absent_snapshot`]).
///
/// # Panics
/// As [`removal`].
#[must_use]
pub fn creation(locator: &str, content: &[u8]) -> PlannedDelta {
    let payload = FsDelta::one(FsOp::write(
        gx_adapter_fs::normalize(locator),
        content.to_vec(),
    ))
    .encode()
    .expect("a one-operation sequence has a canonical DAG-CBOR form");
    PlannedDelta::new(SubstrateKind::Fs, payload).expect("the projection is encodable")
}

/// A whole-file write whose locator is carried **exactly as spelled**, normalisation and all.
///
/// [`creation`] normalises before it encodes, which is what an adapter's own `plan` does. This one
/// does not, so that a test can hand `commutation` two payloads that name one position in two
/// spellings -- the case E-M4-12 exists for and the one M3-10 calls two policy subjects and one
/// file. Nothing but a test writes a payload like this.
///
/// # Panics
/// As [`removal`].
#[must_use]
pub fn spelled(locator: &str, content: &[u8]) -> PlannedDelta {
    let payload = FsDelta::one(FsOp::write(locator.to_string(), content.to_vec()))
        .encode()
        .expect("a one-operation sequence has a canonical DAG-CBOR form");
    PlannedDelta::new(SubstrateKind::Fs, payload).expect("the projection is encodable")
}

/// The `pre` of a creation: an [`ObjectSnapshot`] naming a position that holds nothing.
///
/// 🔴 **Constructed here because the adapter will not produce one.** `FsAdapter::snapshot` reads the
/// file and answers [`Error::Unreadable`] when there is none, so v0.1 has no way to describe 「this
/// position is empty」 as a snapshot -- and AC-049's 「作成」 case is quantified at exactly that state.
/// The digest is the digest of no content, which is the same value `apply` observes after a removal
/// and the same value an **empty file** would have: ASM-69-1 puts only content under the fingerprint,
/// so 「nothing here」 and 「a file with no bytes」 are one state to this adapter. Both halves are raised
/// as起票 in `req/74` §2 rather than decided here.
///
/// # Panics
/// Never: the projection of a snapshot always has a canonical form, and the caller has just named a
/// position.
#[must_use]
pub fn absent_snapshot(locator: &str) -> ObjectSnapshot {
    let normalised = gx_adapter_fs::normalize(locator);
    let digest = cid::mint(Domain::Leaf, &[&[][..]]);
    let placeholder = ObjectSnapshot::new(
        gx_core::ObjectId(Cid([0u8; 32])),
        SubstrateKind::Fs,
        normalised.clone(),
        digest,
        gx_core::ReprKind::Bytes,
    );
    let id = cid::compute(&placeholder).expect("a snapshot projects to a canonical form");
    ObjectSnapshot::new(
        gx_core::ObjectId(id),
        SubstrateKind::Fs,
        normalised,
        digest,
        gx_core::ReprKind::Bytes,
    )
}

/// What is at a position, or `None` when nothing is.
#[must_use]
pub fn content_at(locator: &str) -> Option<Vec<u8>> {
    match std::fs::read(locator) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("{locator} would not answer: {e}"),
    }
}
