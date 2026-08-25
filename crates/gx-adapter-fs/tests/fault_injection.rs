// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-M4-35**: a position that will not answer is not a position that holds nothing.
//!
//! req/38 §35 K-4, adopted (a), verbatim: (sem: SEM-gx-adapter-fs-237)
//!
//! > "fs's read/remove reads **only `NotFound`** as 'there is nothing there' -- other failures (permission,
//! > I/O) are `Err`. **Confusing 'cannot read' with 'is absent' is a direct hit on the fail-closed principle**, and the worst arm is the escrow answering 'it was originally
//! > empty' and **turning the undo into a deletion instead of a restoration** (req/76 §2.2). Fix 3 sites (`apply.rs:197`/
//! > `apply.rs:164`/`invert.rs:124`) with **one EACCES-injection test** (red-first; print a skip when running as
//! > root)" (sem: SEM-gx-adapter-fs-238)
//!
//! # What was actually wrong, stated before anything is claimed
//!
//! The three guards **already read `NotFound` and nothing else** -- `git diff` over `crates/*/src` for
//! this hand is 0 lines. What did not exist was any test that made a read or a removal fail in a way
//! that is not `NotFound`, so all three guards could be deleted and the workspace stayed green:
//! `cargo mutants` missed the three (req/76 §2.2) and the audit's hand-written (A-1) survived
//! (req/76 §2.1). The defect was in the checking, exactly as M3's I-1 was, and this file is the fix.
//!
//! # The ruling assumed one injection reaches all three; it reaches one
//!
//! "one test covers all 3 sites" was measured here and is not true, because the three failures need three (sem: SEM-gx-adapter-fs-239)
//! different faults:
//!
//! | place | what it does | the fault that reaches it |
//! |---|---|---|
//! | `invert.rs:124` `read_if_present` | reads the old content the escrow carries | a **file** that cannot be read (`chmod 000`) |
//! | `apply.rs:164` `remove_whole_file` | unlinks | a **directory** that cannot be written (`chmod 555`); a file's own mode does not govern `unlink` |
//! | `apply.rs:197` `observe` | reads back **after** the rename | the file it reads is the one `apply` just created, so no state set up beforehand can make that read fail -- only the **mode the process creates files with** can |
//!
//! So there are three injections and a fourth probe that reads the source. The source probe is the
//! ruling's own option (b) -- "put a probe in the source scan that makes explicit that the 3 guards absorb
//! **only** `NotFound` and treat the rest as `Unreadable`" -- and it is what makes the *rule* falsifiable rather than the (sem: SEM-gx-adapter-fs-240)
//! three instances of it: a fourth call site added tomorrow is caught by the count.
//!
//! # Running as root
//!
//! Permissions do not bind a process with `CAP_DAC_OVERRIDE`, so the two permission injections are
//! meaningless there. [`permissions_bind_this_process`] answers that **behaviourally** -- it makes a
//! `chmod 000` file and tries to read it -- rather than by asking who the user is, because the
//! question is whether the fault can be injected and not what the user is called. When it cannot, the
//! probe prints `SKIPPED` with the reason and asserts nothing (req/29 §4: a skip and a pass must not
//! look alike, so the word is printed).

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use gx_adapter_fs::{FsAdapter, FsDelta};
use gx_substrate::SubstrateAdapter;
use support::{absent_snapshot, creation, removal, Sandbox, SUBJECT};

/// Can this process be denied by a file mode at all?
///
/// Behavioural, not `id -u`: what matters is whether the fault arrives, and `CAP_DAC_OVERRIDE`,
/// a `root` container, an overlay with `nosuid`-style relaxations and a plain user all answer this
/// question differently while giving the same `uid`.
fn permissions_bind_this_process(sandbox: &Sandbox) -> bool {
    let name = "probe-000";
    sandbox.write(name, b"x");
    let path = sandbox.dir().join(name);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
        .expect("a tmpfs accepts a mode");
    let denied = std::fs::read(&path).is_err();
    // Put it back so the sandbox can be removed on drop.
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
    let _ = std::fs::remove_file(&path);
    denied
}

fn chmod(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .expect("a tmpfs accepts a mode");
}

/// 🔴 `invert.rs:124` -- the worst arm: an escrow that answers "it was originally empty" about a file it could (sem: SEM-gx-adapter-fs-241)
/// not read.
///
/// The inverse of a whole-file replacement is "put back what is here", and "what is here" is read
/// through [`read_if_present`](../src/invert.rs). Its two arms are "bytes" and "nothing", and
/// "nothing" becomes `FsOp::remove` -- so a read failure counted as an absence produces an inverse (sem: SEM-gx-adapter-fs-242)
/// that **deletes the file the undo was supposed to restore**. That is not a degraded undo, it is the
/// opposite of one, and 43 T-10b escrows this delta before `apply` runs, so nothing downstream ever
/// looks at the position again to notice.
///
/// The assertion is written over the *shape* of a wrong answer rather than as `is_err()`, so that a
/// failure of this probe reports the harm rather than the type.
#[test]
fn an_unreadable_position_is_not_an_empty_one_when_the_escrow_is_built() {
    let sandbox = Sandbox::new();
    if !permissions_bind_this_process(&sandbox) {
        println!(
            "FAULT_EACCES=SKIPPED reason=this process is not bound by file modes (root or \
             CAP_DAC_OVERRIDE), so `chmod 000` injects nothing"
        );
        return;
    }
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    chmod(&sandbox.dir().join(SUBJECT), 0o000);

    // Both are built without reading: `snapshot` would refuse first, which is a different (and
    // already tested) refusal -- the question here is what `invert` does once it is called.
    let delta = creation(&locator, b"after");
    let pre = absent_snapshot(&locator);

    let answer = adapter
        .invert(&delta, &pre)
        .map(gx_substrate::InvertOutcome::into_inverse);
    println!(
        "FAULT_EACCES_INVERT KIND={} ",
        match &answer {
            Ok(None) => "Ok(None)".to_string(),
            Ok(Some(_)) => "Ok(Some(..))".to_string(),
            Err(e) => e.kind().to_string(),
        }
    );

    chmod(&sandbox.dir().join(SUBJECT), 0o644);
    match answer {
        Err(e) => {
            assert_eq!(
                e.kind(),
                "Unreadable",
                "a read that failed for a reason other than absence is 'the question cannot be
                 answered' (41 §4), and this refusal calls it something else: {e} (sem: SEM-gx-adapter-fs-243)"
            );
            assert!(
                e.to_string().contains(&locator),
                "the refusal does not name the position that would not answer: {e}"
            );
        }
        Ok(Some(inverse)) => {
            let decoded = FsDelta::decode(inverse.payload()).expect("this crate's own grammar");
            let restores_nothing = decoded
                .ops()
                .first()
                .is_some_and(|op| op.content().is_none());
            panic!(
                "the escrow was built from a read that failed: E-M4-35's worst arm. The inverse \
                 {} -- an undo that deletes the file it was escrowed to restore (42 §5 keeps the \
                 body precisely so that the undo is physically possible)",
                if restores_nothing {
                    "is a REMOVAL"
                } else {
                    "carries content this process never read"
                }
            );
        }
        Ok(None) => panic!(
            "`invert` answered `Ok(None)` for an unreadable position. E-M4-32 narrowed that answer \
             to 'a legitimate construction of the same object is not possible' and E-M3-4 escalates it to a human as a business \
             condition; a permission error is neither (sem: SEM-gx-adapter-fs-244)"
        ),
    }
}

/// `apply.rs:164` — a removal the filesystem refused is not a removal that happened.
///
/// `unlink` is governed by the **directory's** mode and not the file's, so this is the fault that
/// reaches the second guard. The retry reading of idempotence (**E-M4-3**, 43 T-10c) is what makes
/// the guard right in the first place -- running a removal twice must not fail the second time --
/// and it is exactly that reading a swallowed `EACCES` abuses: "it was already gone" and "I was not
/// allowed to take it away" are opposite facts about the substrate, and only one of them means the (sem: SEM-gx-adapter-fs-245)
/// state the delta asked for was reached.
#[test]
fn a_removal_the_filesystem_refused_is_not_a_removal_that_happened() {
    let sandbox = Sandbox::new();
    if !permissions_bind_this_process(&sandbox) {
        println!(
            "FAULT_EACCES_REMOVE=SKIPPED reason=this process is not bound by file modes (root or \
             CAP_DAC_OVERRIDE)"
        );
        return;
    }
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let delta = removal(&locator);

    chmod(sandbox.dir(), 0o555);
    let answer = adapter.apply(&delta);
    let still_there = Path::new(&locator).exists();
    chmod(sandbox.dir(), 0o755);

    println!(
        "FAULT_EACCES_REMOVE KIND={} SUBJECT_STILL_THERE={still_there}",
        match &answer {
            Ok(_) => "Ok".to_string(),
            Err(e) => e.kind().to_string(),
        }
    );
    assert!(
        still_there,
        "the fixture did not inject the fault: the removal succeeded, so this probe measured a \
         writable directory"
    );
    let refusal = answer.expect_err(
        "`apply` reported success for a removal the filesystem refused -- 43 T-11 turns that into a \
         `Committed` record for a change that never happened, which is the fail-open direction of \
         45 §3",
    );
    assert_eq!(
        refusal.kind(),
        "ApplyFailed",
        "a step the filesystem refused is `ApplyFailed` (41 §4's word for 'the filesystem refused a \
         step'): {refusal} (sem: SEM-gx-adapter-fs-246)"
    );
}

/// `apply.rs:197` — the read-back after the rename, made to fail.
///
/// No state prepared in advance reaches this guard: the file `observe` reads is the one `apply` just
/// renamed into place, so what has to change is the **mode the process creates files with**. `umask`
/// is per-process and there is no `std` call for it (and **A-6** forbids adding `libc` for one test),
/// so the probe re-executes this one test under `sh -c 'umask 0777'`. The sandbox is made by the
/// parent, before the umask changes, because a directory created under that mask cannot be written
/// to at all.
///
/// What the guard protects: `observe`'s value becomes `AppliedDelta::resulting_digest` and the
/// postcondition. A read failure counted as an absence therefore reports "the file is now empty" -- (sem: SEM-gx-adapter-fs-247)
/// L5 compares that against the target a plan promised and, for any non-empty goal, the two disagree;
/// for a **removal** they agree, and the pipeline records a successful change to a state nobody
/// observed. Either way the digest a receipt carries is one the substrate never held.
#[test]
fn a_position_that_will_not_answer_after_the_write_is_not_an_empty_one() {
    const CHILD: &str = "GLOVREX_FS_FAULT_UMASK_CHILD";
    const NAME: &str = "a_position_that_will_not_answer_after_the_write_is_not_an_empty_one";

    if let Ok(dir) = std::env::var(CHILD) {
        // --- the child, running under `umask 0777` ---
        let adapter = FsAdapter::new();
        let locator = format!("{dir}/written");
        let answer = adapter.apply(&creation(&locator, b"after"));
        let mode = std::fs::metadata(&locator).map(|m| m.permissions().mode() & 0o777);
        println!("FAULT_UMASK_CHILD MODE={mode:?} ANSWER={answer:?}");
        let refusal = answer.expect_err(
            "`apply` reported a digest for a position it could not read back: E-M4-35 at \
             `apply.rs:197`. `observe`'s value is the `resulting_digest` a receipt carries, so a \
             read failure read as an absence puts the digest of no content into the record",
        );
        assert_eq!(refusal.kind(), "Unreadable", "{refusal}");
        return;
    }

    let sandbox = Sandbox::new();
    if !permissions_bind_this_process(&sandbox) {
        println!(
            "FAULT_UMASK=SKIPPED reason=this process is not bound by file modes (root or \
             CAP_DAC_OVERRIDE)"
        );
        return;
    }
    let exe = std::env::current_exe().expect("a test binary knows its own path");
    let Ok(child) = std::process::Command::new("sh")
        .arg("-c")
        .arg(r#"umask 0777; exec "$0" --exact "$1" --nocapture --test-threads=1"#)
        .arg(&exe)
        .arg(NAME)
        .env(CHILD, sandbox.dir())
        .output()
    else {
        println!("FAULT_UMASK=SKIPPED reason=no `sh` on this machine to set a umask with");
        return;
    };

    let out = String::from_utf8_lossy(&child.stdout).into_owned();
    print!("{out}");
    println!(
        "FAULT_UMASK_PARENT CHILD_RC={:?} STDERR={}",
        child.status.code(),
        String::from_utf8_lossy(&child.stderr).trim()
    );
    assert!(
        out.contains("FAULT_UMASK_CHILD"),
        "the child did not reach the injection, so this probe measured the re-execution and not \
         the guard: {out}"
    );
    assert!(
        child.status.success(),
        "the child failed: see its output above. It runs this same test under `umask 0777`, so \
         the file `apply` renames into place is unreadable and `observe` has to refuse"
    );
}

/// The **rule**, not the three instances of it: every absence-tolerating arm in this crate tolerates
/// `NotFound` and nothing else (the ruling's option (b)).
///
/// Three behavioural probes hold three call sites. A fourth call site added tomorrow is held by
/// nothing -- and confusing 'cannot read' with 'is absent' is a mistake that is made once per call site. So this (sem: SEM-gx-adapter-fs-248)
/// reads the source: every `Err(e) if ...` arm in `src/` has to be guarded on `ErrorKind::NotFound`,
/// and there have to be exactly the three the ruling names.
///
/// It also happens to be the only probe in this file that a `cargo mutants` run of the guard
/// expression itself cannot pass, since that run rewrites the very text this reads -- which is the
/// point of writing the rule down where the rule lives.
#[test]
fn every_absence_arm_in_this_crate_tolerates_only_not_found() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut guards: Vec<(String, String)> = Vec::new();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&src)
        .expect("src is readable")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();

    for file in &files {
        let text = std::fs::read_to_string(file).expect("a source file is readable");
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || !trimmed.starts_with("Err(") {
                continue;
            }
            if let Some(guard) = trimmed.strip_prefix("Err(e) if ") {
                let name = file
                    .file_name()
                    .expect("a file has a name")
                    .to_string_lossy()
                    .into_owned();
                guards.push((name, guard.trim_end_matches(" => {}").trim().to_string()));
            }
        }
    }

    println!("NOT_FOUND_GUARDS={} {guards:?}", guards.len());
    let stray: Vec<&(String, String)> = guards
        .iter()
        .filter(|(_, g)| !g.contains("e.kind() == std::io::ErrorKind::NotFound"))
        .collect();
    assert!(
        stray.is_empty(),
        "these arms treat something other than `NotFound` as absent: {stray:?}. \
         E-M4-35: fs's read/remove reads only `NotFound` as 'there is nothing there' -- other failures (permission, \
          I/O) are Err (sem: SEM-gx-adapter-fs-249)"
    );

    let mut places: Vec<&String> = guards.iter().map(|(f, _)| f).collect();
    places.sort();
    assert_eq!(
        places,
        vec!["apply.rs", "apply.rs", "invert.rs"],
        "the absence-tolerating arms are not the three E-M4-35 names (apply.rs:164 \
         `remove_whole_file`, apply.rs:197 `observe`, invert.rs:124 `read_if_present`). A new one \
         needs a behavioural probe of its own in this file, not a fourth entry in this list"
    );
}
