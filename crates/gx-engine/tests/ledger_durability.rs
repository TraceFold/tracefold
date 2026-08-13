//! **K-9** — the `sync_parent_directory` survivor, killed from the crate that first depends on it.
//!
//! req/38 §35 K-9 and §37 **M5-28 採(a)**: 「K-9 の EACCES 注入は手 4(engine が ledger durability の
//! 初消費者)・前提条件行つき・L-1 の実形(fault 3 種の可能性)を書く」. req/76 §2.2 measured the gap:
//! gx-log's `sync_parent_directory` had two `mutants` survivors because **the durability call was
//! never measured by a behaviour** — every probe in the workspace checked what was written, and none
//! checked that the write reached the device's idea of the directory.
//!
//! # 🔴 前提条件 (規律45), because this probe depends on the machine
//!
//! 1. **Unix.** `sync_parent_directory` is `#[cfg(unix)]` in both crates; on other platforms it is a
//!    declared gap (req/52 §5) and there is nothing to measure. The suite is compiled out there.
//! 2. **A filesystem that enforces mode bits.** The scratch directory of the other suites lives
//!    under `CARGO_TARGET_TMPDIR`, which on this project's WSL2 setup is on `/mnt/c` (drvfs) where
//!    `chmod` is advisory. So this one probe uses `std::env::temp_dir()` — ext4 on the CI image
//!    (A-5: v0.1 CI is x86_64 Linux) — and **asserts that the mode took** before it asserts anything
//!    else. A machine where it does not is a failing probe, not a silent pass (req/29 §4).
//! 3. **Not running as root.** Mode `0o300` is enforced against everyone except a process that may
//!    override it. If this fails on a machine where the mode did take, that is the reason.
//!
//! # Why `0o300` and not `0o000`
//!
//! The directory must be **writable and searchable** so that creating the ledger file succeeds, and
//! **unreadable** so that `File::open(parent)` — which is how fsync reaches a directory on Unix —
//! refuses with `EACCES`. `0o000` would fail earlier, at the create, and would measure the create
//! rather than the sync. The window is one bit wide and it is the whole of the test.
//!
//! # What this does **not** measure (L-1's 実形)
//!
//! req/38 §36 L-1 recorded that a durability claim wants three kinds of fault: a refusal (this), a
//! partial write, and a power loss. Only the first is constructible without a fault-injecting
//! filesystem. The other two remain the declared limit gx-log already wrote down in req/52 §5, and
//! this suite does not narrow it — `sync_all` returning `Ok` still does not prove the platter moved.

#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence};
use gx_log::LedgerStore;
use support::{gate, intent, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// A directory that may be written and entered but not read, on a filesystem that means it.
///
/// Returns `None` when 前提 2 or 3 does not hold, and the callers turn that into a **failure** with
/// the reason printed rather than into a skip.
fn write_only_dir(name: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("gx_m5h4_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the temp directory is writable");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o300)).expect("chmod");
    let mode = fs::metadata(&dir).expect("stat").permissions().mode() & 0o777;
    println!("SCRATCH_DIR={} MODE={mode:o}", dir.display());
    if mode != 0o300 {
        return None;
    }
    Some(dir)
}

/// Restore the mode so that the directory can be removed, then remove it.
fn cleanup(dir: &PathBuf) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    let _ = fs::remove_dir_all(dir);
}

/// 🔴 **K-9**: `LedgerStore::open` refuses when the parent directory cannot be fsynced.
///
/// The survivor this kills is `sync_parent_directory → Ok(())`: a mutant that skips the call
/// returns `Ok` here, and the real function returns `Err`. Nothing in the workspace could tell the
/// two apart until something depended on the answer, which is why §35 put the window in the hand
/// that wires the ledger.
#[test]
fn a_ledger_whose_directory_cannot_be_fsynced_is_refused() {
    let Some(dir) = write_only_dir("ledger") else {
        panic!(
            "前提 2 not met: `chmod 0o300` did not take on this filesystem, so the EACCES this \
             probe injects cannot happen. On drvfs/9p mode bits are advisory -- run on ext4 (A-5)."
        );
    };

    let opened = LedgerStore::open(dir.join("ledger.bin"));
    let refusal = match &opened {
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    };
    println!("LEDGER_OPEN_REFUSAL={refusal:?}");
    cleanup(&dir);

    let refusal = refusal.unwrap_or_else(|| {
        panic!(
            "前提 3 may not hold: the ledger opened in a directory that cannot be fsynced. A \
             process that may override mode bits (root) sees no EACCES."
        )
    });
    assert!(
        refusal.contains("directory"),
        "the refusal names the directory, not the file: {refusal}"
    );
}

/// The same fault, reached through the engine — the consumer §35 was waiting for.
///
/// `Engine::open` creates three things beside each other (blobs, ledger, journal), and every one of
/// them wants the directory entry on the device. The engine refuses, which is what makes 43 §7's
/// write-ahead ordering true rather than nominal: a journal whose *name* did not survive is a
/// journal that recorded nothing.
#[test]
fn an_engine_whose_directory_cannot_be_fsynced_is_refused() {
    let Some(dir) = write_only_dir("engine") else {
        panic!("前提 2 not met -- see the sibling probe for what that means");
    };

    let opened = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    );
    let kind = opened.as_ref().err().map(gx_engine::Error::kind);
    let detail = opened.as_ref().err().map(std::string::ToString::to_string);
    println!("ENGINE_OPEN_REFUSAL={kind:?} {detail:?}");
    cleanup(&dir);

    // Which layer refuses depends on the order `Engine::open` builds its three files in, and that
    // order is not this probe's claim: `Io` is the engine's own journal directory and `Ledger` is
    // gx-log's. What is asserted is that one of them refused and said which.
    let kind = kind.expect("前提 3 may not hold: the engine opened where nothing can be fsynced");
    assert!(
        kind == "Ledger" || kind == "Io",
        "the engine refuses, and says which layer refused: {kind} {detail:?}"
    );
}

/// The control: the same sequence in a directory that can be fsynced commits.
///
/// §30 again — a refusal measured without the success beside it is a measurement of the fixture. The
/// probe runs the whole critical section, so the thing shown to work is the thing the two probes
/// above show to fail.
#[test]
fn the_same_engine_in_an_ordinary_directory_commits() {
    let dir = std::env::temp_dir().join(format!("gx_m5h4_ok_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the temp directory is writable");

    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("an ordinary directory opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &signing_key()).expect("commit");

    println!(
        "CONTROL_STATE={state:?} LEAVES={} APPLY={}",
        engine.ledger().log().len(),
        counts.totals()[4]
    );
    assert_eq!(state, gx_engine::Lifecycle::Committed);
    assert_eq!(engine.ledger().log().len(), 1);
    let _ = fs::remove_dir_all(&dir);
}
