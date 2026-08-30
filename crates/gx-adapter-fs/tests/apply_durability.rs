// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What `apply` does around the `rename`, what it leaves behind, and what this hand may claim.
//!
//! # The three steps, and where they come from
//!
//! This hand fetched the primaries in full over `curl` rather than reading a summary of them
//! (`Desktop/GitRepo/REFERENCES.md`, 2026-08-09; hand 4's entry for the same three was a WebSearch
//! snippet, which is enough to write a doc paragraph and not enough to write a `sync_all`):
//!
//! * **LWN 457667** gives the sequence, verbatim: "The following steps are required to perform this
//!   type of update: create a new temp file (on the same file system!) / write data to the temp file
//!   / fsync() the temp file / rename the temp file to the appropriate name / fsync() the containing
//!   directory". `SURVIVORS.md` §A-2 calls it "3 moves", which are the last three of those five.
//! * **POSIX `rename`** gives the atomicity, and gives it in one direction: "a directory entry named
//!   new shall remain visible to other threads throughout the renaming operation and refer either to
//!   the file referred to by new or old before the operation began", with "the action of the function
//!   be atomic" in the RATIONALE. The subject of the sentence is **other threads**.
//! * **`std::fs::rename`** documents "replacing the original file if `to` already exists" and never (sem: SEM-gx-adapter-fs-175)
//!   uses the word atomic, because the guarantee belongs to the filesystem underneath.
//!
//! # 🔴 One correction this hand owes its own sources
//!
//! `SURVIVORS.md` §A-2 and `req/73` both write that **skipping the parent-directory fsync loses the
//! rename**. That sentence is **not in LWN 457667**: the article gives the ordered list and says
//! nothing about omitting a step (the words "directory entry" do not occur in its body at all). The (sem: SEM-gx-adapter-fs-176)
//! failure modes are a **derivation** -- a directory entry is data like any other, so a rename that
//! reached no stable storage is a rename that a crash can lose -- and this file says derivation where
//! the earlier text said source. Marking it is the whole of the fix; the three steps stay.
//!
//! # What these tests can and cannot say
//!
//! They run on a tmpfs, where `fsync` is effectively a no-op. So they are evidence that the **order**
//! is in the code and that the temporary file does not survive the operation, and they are **not**
//! evidence about crash recovery. Measuring that needs a filesystem with a device under it and a
//! power failure to inject; saying so is req/29 §4's rule about a skip and a pass, applied to a claim
//! rather than to a test.

mod support;

use gx_adapter_fs::FsAdapter;
use gx_substrate::SubstrateAdapter;
use support::{content_at, planned, removal, snapshot_of, Sandbox, BEFORE, GOAL, SUBJECT};

fn crate_root_source() -> String {
    std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("the crate root is readable")
}

fn apply_source() -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/apply.rs"),
    )
    .expect("the apply module is readable")
}

/// The three steps are in the source, in the order the primary gives them.
///
/// A behavioural test on a tmpfs cannot tell a missing `fsync` from a present one, so the order is
/// asserted where it is decidable: the positions of the three calls **inside the one function that
/// writes**.
///
/// 🔴 The function bound was earned rather than chosen. Scanning the whole module, this probe
/// **survived** `tools/verify_m4h5.sh` mutation (a) -- which deletes the directory `fsync` from the
/// write path -- because `remove_whole_file` sits below `write_whole_file` and its own directory
/// `fsync` answered the search. That is §30 M4H2-6's rule ("somewhere in the file" is not "written
/// at that spot") in its fourth costume, and the fifth entry the ledger §31 M4H3-5 opened: an (sem: SEM-gx-adapter-fs-177)
/// instrument that finds the right token in the wrong scope reports on a claim nobody made.
///
/// 🔴 **`req/868` R-868-5 / `req/919` W4 (2026-08-29): the literal token this probe searches for at
/// the directory-sync position changed from `sync_all()` to `sync_parent_directory(`.** The
/// un-`cfg`-gated `std::fs::File::open(parent)?.sync_all()` this test used to find directly is the
/// defect R-868-5 named (native Windows: `File::open` on a directory handle fails, so `apply`
/// reported failure for a rename that had already landed). The repair extracts the step into a
/// `#[cfg(unix)]`/`#[cfg(not(unix))]` function so the directory-durability guarantee is `cfg`-gated
/// and typed (`NameDurability`/`NAME_DURABILITY`, mirroring `gx_engine::NAME_DURABILITY`/G9) rather
/// than called unconditionally. The **order** this test asserts (temp fsync, then rename, then the
/// directory-durability step) is unchanged and still the whole of what it means to test; only the
/// name of the third call moved from an inline `sync_all()` to a named, cfg-gated call that performs
/// `sync_all()` internally on unix.
#[test]
fn the_write_syncs_the_temporary_file_then_renames_then_syncs_the_directory() {
    let src = apply_source();
    let start = src
        .find("fn write_whole_file(")
        .expect("the module has the function that writes");
    let end = src[start..]
        .find("\nfn ")
        .map_or(src.len(), |at| at + start);
    let code: String = src[start..end]
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let temp_sync = code
        .find("sync_all()")
        .expect("the temporary file is fsynced (LWN step 3)");
    let rename = code
        .find("fs::rename(")
        .expect("the change lands by rename (M4-13, adopted (a))"); // (sem: SEM-gx-adapter-fs-178)
    let dir_sync = code[rename..]
        .find("sync_parent_directory(")
        .map(|at| at + rename)
        .expect(
            "the containing directory's durability is handled by the write path itself (LWN step \
             5, R-868-5 cfg-gated form)",
        );

    println!("APPLY_STEPS temp_fsync@{temp_sync} rename@{rename} dir_fsync@{dir_sync}");
    assert!(
        temp_sync < rename && rename < dir_sync,
        "the three steps are out of order, and the order is the whole of what makes the sequence \
         durable rather than merely present"
    );
}

/// Nothing of the mechanism is left in the directory afterwards.
///
/// The temporary file is the one artefact an atomic replacement creates, and a reader of the
/// substrate must never see it as a file of theirs. This is also the check that catches a rename
/// that silently became a copy.
#[test]
fn no_temporary_file_survives_an_apply() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let before: Vec<String> = names_in(sandbox.dir());

    adapter
        .apply(&planned(&adapter, &locator, GOAL))
        .expect("the delta applies");
    let after = names_in(sandbox.dir());

    println!("SANDBOX_ENTRIES_BEFORE={before:?} AFTER={after:?}");
    assert_eq!(
        before, after,
        "an apply left something behind, or took something away"
    );
    assert_eq!(content_at(&locator).as_deref(), Some(GOAL));
}

/// The same, for a removal: `unlink` is itself a directory operation and needs no temporary file.
#[test]
fn a_removal_leaves_the_directory_holding_one_thing_less() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    adapter
        .apply(&removal(&locator))
        .expect("the removal applies");
    let after = names_in(sandbox.dir());

    println!("SANDBOX_ENTRIES_AFTER_REMOVAL={after:?}");
    assert!(!after.contains(&SUBJECT.to_string()));
    assert!(
        after.iter().all(|n| !n.starts_with(".gx-")),
        "a removal has no temporary file to leave: {after:?}"
    );
}

/// **L2 by behaviour**: the retry moves neither the substrate nor what the adapter reports.
///
/// 51 §7 contract 7 and 43 T-10c: a crash between the write and the journal record is recovered by
/// running the same delta again, so the second run has to produce the record the first would have.
/// The harness asserts the same thing as a law; this is the direct statement, and
/// `tools/verify_m4h5.sh` mutation (b) is what shows it is not vacuous -- an `apply` that appended
/// instead of replacing passes every one-shot assertion and fails this one.
#[test]
fn the_second_apply_of_one_delta_moves_nothing() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let delta = planned(&adapter, &locator, GOAL);

    let first = adapter.apply(&delta).expect("the first apply lands");
    let once = content_at(&locator);
    let second = adapter.apply(&delta).expect("the retry is safe");
    let twice = content_at(&locator);

    println!(
        "L2_RETRY digest_equal={} content_equal={}",
        first.resulting_digest() == second.resulting_digest(),
        once == twice
    );
    assert_eq!(once, twice, "the retry moved the substrate");
    assert_eq!(
        first.resulting_digest(),
        second.resulting_digest(),
        "the retry reported a different digest, so a replay would journal a state the first run \
         never saw (E-M4-3, 43 T-10c)"
    );
    match first.postcondition().cas_eq(second.postcondition()) {
        Ok(true) => {}
        Ok(false) => panic!("the retry reported a different postcondition fingerprint"),
        Err(e) => panic!("the two postconditions are not comparable: {e}"),
    }
}

/// A removal is idempotent too, and for the same reason: the retry after a crash is the same call.
#[test]
fn the_second_removal_of_one_delta_is_not_a_failure() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let delta = removal(&locator);

    let first = adapter.apply(&delta).expect("the removal applies");
    let second = adapter
        .apply(&delta)
        .expect("removing what is already gone is the retry, not an error");
    assert_eq!(first.resulting_digest(), second.resulting_digest());
    assert_eq!(content_at(&locator), None);
}

/// A sequence longer than v0.1 accepts is refused, and refused as "unimplemented" rather than as damage.
///
/// M4-13, adopted (a): "**v0.1's `apply` accepts only a sequence of `len==1`**; `len>1` is Err (unimplemented,
/// stated explicitly -- it does not run non-atomically in silence, i.e. fail-closed)". The decoder is where the bound lives (hand 4) and this is the statement that (sem: SEM-gx-adapter-fs-179)
/// `apply` goes through it -- a second write path that skipped the decoder would make two files move
/// under one `rename`, which is 45 §3's TH-3 condition exactly.
#[test]
fn a_two_operation_payload_is_refused_by_apply() {
    use gx_adapter_fs::{FsDelta, FsOp};
    use gx_core::SubstrateKind;
    use gx_substrate::PlannedDelta;

    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let payload = FsDelta::of(vec![
        FsOp::write(sandbox.locator(SUBJECT), b"one".to_vec()),
        FsOp::write(sandbox.locator("beside"), b"two".to_vec()),
    ])
    .encode()
    .expect("a two-element sequence is a legal value of the grammar");
    let delta = PlannedDelta::new(SubstrateKind::Fs, payload).expect("the projection encodes");

    let refusal = adapter
        .apply(&delta)
        .expect_err("v0.1 accepts one operation");
    println!("APPLY_LEN2_REFUSAL={}", refusal.kind());
    assert_eq!(refusal.kind(), "Unimplemented");
    assert_eq!(
        content_at(&sandbox.locator(SUBJECT)).as_deref(),
        Some(BEFORE),
        "a refused apply moved the substrate, which is the fail-open direction M4-13(a) names"
    );
}

/// A delta another adapter wrote is refused before anything is opened (**E-M4-27**'s delta twin).
#[test]
fn a_delta_from_another_substrate_never_reaches_the_filesystem() {
    use gx_core::SubstrateKind;
    use gx_substrate::PlannedDelta;

    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let foreign = PlannedDelta::new(SubstrateKind::Git, b"not this adapter's grammar".to_vec())
        .expect("the projection encodes");

    assert_eq!(
        adapter
            .apply(&foreign)
            .expect_err("an fs adapter does not apply a git delta")
            .kind(),
        "ForeignDelta"
    );
    assert_eq!(
        adapter
            .invert(&foreign, &snapshot_of(&adapter, &sandbox.locator(SUBJECT)))
            .expect_err("nor invert one")
            .kind(),
        "ForeignDelta"
    );
}

/// **M4H4-10**: the crate root's reachability paragraph is written for an adapter that **writes**.
///
/// §32 record+trigger clause: "**hand 5, when writing `apply`, must add to hand 5's DoD the requirement to rewrite the crate root's
/// disclosure from 'readable' to 'readable, writable'**". A disclosure that still described a reader would understate v0.1 by
/// exactly the thing this hand added, and 45 §4 forbids the direction. Measured in the section rather
/// than in the file (§30 M4H2-6: "somewhere in the file" is not "written at that spot"). (sem: SEM-gx-adapter-fs-180)
#[test]
fn the_reachability_disclosure_describes_writing_and_not_only_reading() {
    let source = crate_root_source();
    let heading = "# No sandbox, and what that means now that this adapter writes";
    let body = source
        .split(heading)
        .nth(1)
        .unwrap_or_else(|| panic!("the crate root has no {heading:?} section"))
        .split("\n//! # ")
        .next()
        .expect("the section ends");

    // 🔴 The tokens are phrases and not words, for the reason mutation (i) exposed: with "write" in (sem: SEM-gx-adapter-fs-181)
    // the list, the probe **survived** a rewrite of the sentence that says what `apply` does, because
    // the word was still in the clause above it. A one-word token in a paragraph of prose is
    // satisfied by any sentence at all -- §30 M4H2-6 again, and the reason each entry below is a
    // clause a single edit removes.
    for token in [
        "creates, replaces and removes whole files",
        "any absolute path this process can write",
        "N-05",
        "Landlock",
        "v0.2",
    ] {
        assert!(
            body.contains(token),
            "the reachability disclosure does not name {token:?}"
        );
    }
    println!("DISCLOSURE_SECTION_LINES={}", body.lines().count());
}

/// The durability paragraph attributes the ordered list to its source and the failure modes to
/// nobody but this repository (the correction at the top of this file).
#[test]
fn the_durability_disclosure_separates_the_source_from_the_derivation() {
    let source = crate_root_source();
    let heading = "# Durability: the three steps, and what a tmpfs cannot show";
    let body = source
        .split(heading)
        .nth(1)
        .unwrap_or_else(|| panic!("the crate root has no {heading:?} section"))
        .split("\n//! # ")
        .next()
        .expect("the section ends");

    for token in ["LWN", "derivation", "tmpfs", "crash"] {
        assert!(
            body.contains(token),
            "the durability disclosure does not name {token:?}, and the one it must not lose is \
             the difference between what a primary says and what follows from it"
        );
    }
}

fn names_in(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the sandbox is a directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}
