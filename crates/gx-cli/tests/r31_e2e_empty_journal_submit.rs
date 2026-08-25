// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R31 / `req/378` H-02, end to end** — the shipped binary, on the population R30's guard was
//! built for.
//!
//! # Why this file exists
//!
//! The thirtieth adversarial audit measured H-02 at the engine boundary and said so in its own
//! report rather than leaving it to be found: it did not drive `gx submit` end to end. It
//! measured at the engine boundary, with the arguments the CLI's writer door passes, and said
//! the end-to-end reproduction was R31's to take first (`req/378` §4). `req/38` §242 ruling 2 made that this lane's first job, and named what the
//! reproduction decides: the frequency was never measured, and the end-to-end reproduction is
//! what settles it — the grade turns on which roads actually reach the door, not on whether
//! the engine misbehaves once it is reached.
//!
//! `req/38` §242 ruling 7 is why it has to be driven by name: a merge review that runs the floor
//! and the acceptance suite does not reach a road like this one. The standing rule it set: a
//! review that merges a new guard or version branch drives every arm of that branch once,
//! itself, rather than accepting a green acceptance suite in their place.
//!
//! # 🔴 The finding this file records first: two of the three roads are already closed
//!
//! Three roads were driven to the writer's door with a journal of zero bytes. Two of them never
//! arrive, and they are recorded here as arms rather than as prose, because "the defect is
//! unreachable that way" is a claim that rots unless something drives it:
//!
//! | road | what stops it |
//! |---|---|
//! | a project that has committed, journal truncated to zero | **DR-43-11.** The signed head records the journal's length; a journal that shrank on a frame boundary is not a torn tail, and `gx submit` refuses with `LEDGER_DISAGREES` before the framing is ever consulted |
//! | a half-made project with **no** journal file | `DeclarationWriter::create_journal` writes the declared marker with `std::fs::write`, so the engine's door is handed eight bytes and never sees an empty buffer |
//! | a half-made project **with** a zero-byte journal | 🔴 **arrives, and this is the end-to-end reproduction** |
//!
//! # 🔴 What the third road turned out to be, which is not what was expected
//!
//! The reproduction was written expecting the audit's engine-level shape to appear at the far end:
//! a `Diverged` record in a v1-framed file, a chain broken at byte 8, `gx submit` answering 0 over
//! a journal nobody can read. That is **not** what the road does, and the difference is recorded
//! here because it changes what H-02 costs a buyer.
//!
//! Driven on the unrepaired build, `gx submit` answered **1**, with `LEDGER_DISAGREES` — *"the
//! journal is the file that moved: bytes this process had already read back no longer read the
//! same"*. The marker had been stamped (`GXJRNL01`, eight bytes) while the engine held itself to
//! be `ChainedV2`, and the disagreement was caught downstream and reported as a corrupt project.
//! No record was appended, so nothing on the disk was damaged. Driven on the repaired build the
//! same road answers **0**, `records=1`, `chain_intact=true`.
//!
//! So H-02's end-to-end consequence is not corruption but a **false refusal**: a half-made project
//! declaring `chained` whose journal is zero bytes was locked out of the product and told its
//! ledger disagreed with its journal, which was untrue — the two files agreed with each other, and
//! the thing that had moved was the engine's own stamp. `req/38` §242 ruling 2 put the grade on
//! this measurement — the frequency was unmeasured and this reproduction settles it — so both
//! readings are published and the report argues neither up nor down.
//!
//! A zero-byte journal beside a declaration and no head is what a crash between `File::create` and
//! the marker write leaves, and what a restore that recreated the path without its contents
//! leaves. The half-made project itself is not invented here — it is the population
//! `model_a_probes.rs`'s `a_legacy_project_and_a_half_made_one_are_not_locked_out` already
//! carries, built from the same bytes.
//!
//! # What is asserted
//!
//! One sentence, and deliberately not "the submit succeeds": *whatever the binary decides, the
//! durable record must be readable afterwards.* A refusal that leaves the bytes alone is a correct
//! outcome. A success whose journal reads back as zero records over a broken chain is not — and
//! DR-43-9 forbids repairing that by truncation, so a project that reaches it has no road back.
//! The predicate is taken from the disk, through `EngineJournal::open_read_only`, after the
//! process that wrote it has exited.

mod support;

use std::path::{Path, PathBuf};

use support::{run, scratch, secure_scratch, Run};

/// The first eight bytes on the disk, as a string.
fn marker_on_disk(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes[..bytes.len().min(8)]).to_string()
}

/// A project directory holding a `chained` declaration, settings, and whatever journal the caller
/// asks for — the shape `model_a_probes.rs` calls "half made".
///
/// `journal` is `None` for "no file at all" and `Some(bytes)` for a file holding exactly those
/// bytes. Nothing here is a finding: these are the bytes an R6-R29 binary leaves behind, written
/// directly because this build's own `gx init` declares `chained-v2`.
fn half_made(name: &str, journal: Option<&[u8]>) -> PathBuf {
    let root = scratch(name);
    let gx = root.join(".gx");
    std::fs::create_dir_all(&gx).expect("make .gx/");
    std::fs::write(gx.join("VERSION"), "1\njournal_format=chained\n").expect("the declaration");
    std::fs::write(gx.join("config.toml"), "# settings\n").expect("the settings");
    if let Some(bytes) = journal {
        std::fs::create_dir_all(gx.join("ledger")).expect("make .gx/ledger/");
        std::fs::write(gx.join("ledger").join("journal"), bytes).expect("the journal");
    }
    root
}

fn journal_path(project: &Path) -> PathBuf {
    project.join(".gx").join("ledger").join("journal")
}

/// `gx submit` into a project directory, with a key from `home`.
fn submit_into(project: &Path, home: &Path, key: &str) -> Run {
    let goal = project.join("intent.txt");
    std::fs::write(&goal, "a goal\n").expect("write the intent");
    let target = project.join("target.txt");
    std::fs::write(&target, "hello\n").expect("write the target");
    let mut cmd = support::gx();
    cmd.env("HOME", home)
        .env("USERPROFILE", home)
        .arg("--project")
        .arg(project)
        .arg("submit")
        .args(["--substrate", "fs"])
        .arg("--locator")
        .arg(&target)
        .arg("--intent")
        .arg(&goal)
        .args(["--context", "Evidence"])
        .args(["--actor-key", key]);
    run(&mut cmd)
}

/// A key store, and the id of the key in it.
fn a_key(name: &str) -> (PathBuf, String) {
    let home = secure_scratch(name);
    let mut cmd = support::gx();
    cmd.env("HOME", &home)
        .env("USERPROFILE", &home)
        .args(["key", "gen", "--json"]);
    let generated = run(&mut cmd);
    assert_eq!(generated.code, 0, "a key: {}", generated.stderr);
    let key = generated.json()["key_id"]
        .as_str()
        .expect("`gx key gen` prints a key_id")
        .to_string();
    (home, key)
}

/// 🔴 The road that arrives: a `chained` declaration, no signed head, and a journal of zero bytes,
/// met by the shipped `gx submit`.
#[test]
fn r31_e2e_a_half_made_chained_project_with_a_zero_byte_journal_keeps_a_readable_journal() {
    let (home, key) = a_key("r31_e2e_key");
    let project = half_made("r31_e2e_zero_byte_journal", Some(b""));
    let journal = journal_path(&project);
    assert_eq!(
        std::fs::metadata(&journal).expect("stat").len(),
        0,
        "the bed is a journal file of zero bytes"
    );

    let submitted = submit_into(&project, &home, &key);
    let marker_after = marker_on_disk(&journal);
    let bytes_after = std::fs::metadata(&journal).expect("stat").len();
    println!(
        "R31_E2E_SUBMIT exit={} marker_after={marker_after:?} bytes_after={bytes_after} \
         stderr={}",
        submitted.code,
        submitted.stderr.trim()
    );

    // Read it back the way the next invocation of any verb reads it: off the disk, after the
    // writing process has exited.
    let reopened = gx_engine::EngineJournal::open_read_only(&journal);
    match &reopened {
        Ok(j) => println!(
            "R31_E2E_REOPEN open=Ok records={} chain_intact={} format={:?} torn_tail={}",
            j.records().len(),
            j.chain_intact(),
            j.format(),
            j.recovery().torn_tail_bytes,
        ),
        Err(e) => println!("R31_E2E_REOPEN open=Err {e}"),
    }

    // 🔴 The invariant, whichever way the submit went: the framing the file reports is the framing
    // its own bytes carry.
    if let Ok(j) = &reopened {
        if let Some(marker) = j
            .format()
            .marker()
            .map(|m| String::from_utf8_lossy(m).to_string())
        {
            assert_eq!(
                marker_after,
                marker,
                "🔴 `req/378` H-02 (end to end): the journal reports {:?} and the eight bytes on \
                 the disk are {marker_after:?}",
                j.format()
            );
        }
    }

    // 🔴 The predicate this file exists for, stated so that **either** outcome is judged rather
    // than only the one this lane hoped for. A refusal is sound. What is not sound is answering 0
    // over a journal that cannot be read back.
    let j = reopened.expect("whatever the verb decided, the bytes it left must open");
    if submitted.code == 0 {
        assert!(
            j.chain_intact(),
            "🔴 `req/378` H-02 (end to end): `gx submit` answered 0 and the journal it wrote reads \
             back with a broken chain. The first record's link would have been minted over the v2 \
             genesis under a v1 header, and DR-43-9 forbids repairing that by truncation, so the \
             project could not be appended to again"
        );
        assert!(
            !j.records().is_empty(),
            "🔴 `req/378` H-02 (end to end): `gx submit` answered 0 and the journal holds no \
             records"
        );
    } else {
        // 🔴 The measured outcome on this build, recorded rather than asserted away: the verb
        // refuses with `LEDGER_DISAGREES` before any record is appended, so the divergence stays
        // in memory and never reaches the disk. That is what makes this road **not** an
        // end-to-end reproduction, and it is the fact `req/38` §242 ruling 2 asked this lane to
        // establish. What is still asserted is that the refusal was clean.
        assert!(
            j.chain_intact(),
            "a refusal must leave a journal that still reads as itself"
        );
        assert!(
            j.records().is_empty(),
            "and a refusal must not have appended a record: {} present",
            j.records().len()
        );
    }
}

/// 🔴 Road 2, driven so that "it cannot arrive that way" is measured rather than asserted: the same
/// half-made project with **no** journal file at all.
///
/// `DeclarationWriter::create_journal` writes the marker the project declares, so the engine's
/// door is handed eight bytes and the empty-buffer branch is never taken. If a later change made
/// `create_journal` leave an empty file instead, this arm and the one above would both move.
#[test]
fn r31_e2e_a_half_made_project_with_no_journal_is_given_the_marker_it_declares() {
    let (home, key) = a_key("r31_e2e_absent_key");
    let project = half_made("r31_e2e_absent_journal", None);
    assert!(
        !journal_path(&project).exists(),
        "the bed is a project with no journal file"
    );

    let submitted = submit_into(&project, &home, &key);
    let journal = journal_path(&project);
    println!(
        "R31_E2E_ABSENT exit={} marker={:?} bytes={} stderr={}",
        submitted.code,
        marker_on_disk(&journal),
        std::fs::metadata(&journal).map(|m| m.len()).unwrap_or(0),
        submitted.stderr.trim()
    );
    assert_eq!(submitted.code, 0, "{}", submitted.stderr);
    assert_eq!(
        marker_on_disk(&journal),
        "GXJRNL01",
        "the journal is created carrying the marker the project declares (`req/372` M-02)"
    );
    let j = gx_engine::EngineJournal::open_read_only(&journal).expect("it opens");
    println!(
        "R31_E2E_ABSENT_REOPEN records={} chain_intact={} format={:?}",
        j.records().len(),
        j.chain_intact(),
        j.format()
    );
    assert!(j.chain_intact(), "and it reads back as itself");
}
