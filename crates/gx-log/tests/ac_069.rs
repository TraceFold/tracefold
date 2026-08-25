// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-069 (NFR-009) — an append that answered "ok" (sem: SEM-gx-log-134) is on the disk.
//!
//! AC-069 verbatim (34 §K): "Given: a call to `ledger.append`. When: a power cut / process kill is
//! simulated immediately after a success response is returned. Then: after restart, the entry is
//! necessarily present (fsync-on-append)." NFR-009 verbatim (33 §2): "`ledger.append` completes an
//! fsync to the underlying storage before returning a success response (fsync-on-append)". (sem: SEM-gx-log-135)
//!
//! # What "power cut / process kill" (sem: SEM-gx-log-136) can and cannot be simulated here
//!
//! Dropping the store and opening the file again is a **process kill**, not a power cut, and the
//! difference is the whole of what makes this file weaker than it reads. A killed process loses
//! nothing that reached the kernel: the page cache belongs to the machine, so a second reader on
//! the same machine sees every byte `write` accepted whether or not anybody called `fsync`. A
//! power cut loses exactly what `fsync` had not yet pushed to the device.
//!
//! So the behavioural half of this file (`..._after_a_reopen`, the torn-tail cases) proves the
//! **framing and the recovery** and would pass on a build with no `fsync` in it at all. The
//! barrier itself is held by the static half at the bottom -- one call site, on the append path,
//! before the entry becomes visible -- and by `tools/verify_m2h3.sh`, which runs this binary under
//! `strace` and counts the syscall, then removes the barrier from the source and shows the count
//! fall to zero while this file goes RED. Saying which half proves which is the point: a
//! crash-consistency suite that cannot fail on a missing `fsync`, presented as though it could, is
//! the fail-open req/29 §4 forbids.
//!
//! Genuine power-loss injection (device-level, `dm-flakey` or equivalent) is not done anywhere in
//! this repository and is recorded as not done in req/52 §5.

mod support;

use gx_core::Timestamp;
use gx_log::proof::{prove_inclusion, verify_inclusion};
use gx_log::{AppendOutcome, LedgerStore};
use std::fs::{self, OpenOptions};
use std::io::Write;
use support::{cid, code_hits, code_lines, scratch, source, tid};

/// Append `n` entries to a fresh store at `path` and return what each successful call returned.
///
/// The store is dropped before this returns, which is the "process kill" (sem: SEM-gx-log-137) of AC-069 as far as a
/// single-process test can stage it.
fn fill(path: &std::path::Path, n: u64) -> Vec<gx_log::LedgerEntry> {
    let mut store = LedgerStore::open(path).expect("open the ledger");
    let mut returned = Vec::new();
    for i in 0..n {
        let outcome = store
            .append(tid(i), cid(1_000 + i), Timestamp(i as i64))
            .expect("append");
        match outcome {
            AppendOutcome::Appended(entry) => returned.push(entry),
            AppendOutcome::AlreadyPresent(entry) => {
                panic!(
                    "entry {} was already present in a fresh store: {entry:?}",
                    i
                )
            }
        }
    }
    returned
}

/// The AC itself: everything a successful call returned is there when the file is opened again.
#[test]
fn ac_069_every_entry_a_successful_append_returned_is_there_after_a_reopen() {
    let path = scratch("ac_069_reopen").join("ledger.log");
    let returned = fill(&path, 8);

    let reopened = LedgerStore::open(&path).expect("reopen the ledger");
    assert_eq!(
        reopened.recovery().torn_tail_bytes,
        0,
        "a cleanly closed ledger has no torn tail"
    );
    assert_eq!(reopened.recovery().records, 8);
    assert_eq!(
        reopened.log().entries(),
        returned.as_slice(),
        "the replayed entries are not the ones the appends returned"
    );
}

/// The recovered tree is the tree that was written, not merely the same entries.
///
/// Roots rather than entries, because the entries could agree while the fold over them did not --
/// and the root is what a checkpoint publishes.
#[test]
fn ac_069_the_recovered_log_has_the_root_it_had() {
    let path = scratch("ac_069_root").join("ledger.log");
    fill(&path, 13);
    let before = support::log_of(13);

    let reopened = LedgerStore::open(&path).expect("reopen");
    assert_eq!(reopened.log().root(), before.root());
    for size in 1..=13u64 {
        assert_eq!(
            reopened.log().root_at(size),
            before.root_at(size),
            "prefix root of {size}"
        );
    }
}

/// A proof issued by the reopened store verifies against the recovered root.
#[test]
fn ac_069_a_reopened_ledger_still_proves_inclusion() {
    let path = scratch("ac_069_proof").join("ledger.log");
    fill(&path, 20);

    let reopened = LedgerStore::open(&path).expect("reopen");
    let root = reopened.log().root().expect("a non-empty log has a root");
    for index in 0..20u64 {
        let proof = prove_inclusion(reopened.log(), index).expect("proof");
        let entry = reopened.log().entry(index).expect("entry");
        assert_eq!(
            verify_inclusion(&proof, &root, entry),
            Ok(true),
            "leaf {index}"
        );
    }
}

/// A write that was cut in half is discarded, and the ledger opens.
///
/// This is batch 8's "a torn write recovers by truncating the mismatched tail" (sem: SEM-gx-log-138) as a case: a header that promises
/// 40 bytes followed by 10 is exactly what a crash between two `write` calls leaves.
#[test]
fn ac_069_a_torn_tail_is_discarded_and_the_ledger_still_opens() {
    let path = scratch("ac_069_torn").join("ledger.log");
    fill(&path, 3);
    let good = fs::metadata(&path).expect("metadata").len();

    let mut raw = OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open the file directly");
    raw.write_all(&40u32.to_be_bytes())
        .expect("a length header");
    raw.write_all(&[0xAA; 10]).expect("a truncated payload");
    raw.sync_all().expect("fsync the damage");
    drop(raw);

    let reopened = LedgerStore::open(&path).expect("reopen over a torn tail");
    assert_eq!(reopened.recovery().records, 3);
    assert_eq!(reopened.recovery().torn_tail_bytes, 14);
    assert_eq!(
        fs::metadata(&path).expect("metadata").len(),
        good,
        "the torn tail is removed from the file, not merely skipped in memory"
    );
}

/// After recovery the ledger keeps appending from the entry it actually holds.
///
/// The index is assigned from the recovered length, so a torn record must not leave a hole: entry
/// 3 has to be the next one written, not entry 4.
#[test]
fn ac_069_appending_resumes_after_a_torn_tail() {
    let path = scratch("ac_069_resume").join("ledger.log");
    fill(&path, 3);

    let mut raw = OpenOptions::new().append(true).open(&path).expect("open");
    raw.write_all(&[0x00, 0x00, 0x00]).expect("half a header");
    raw.sync_all().expect("fsync");
    drop(raw);

    let mut reopened = LedgerStore::open(&path).expect("reopen");
    assert_eq!(reopened.recovery().torn_tail_bytes, 3);
    let outcome = reopened
        .append(tid(3), cid(1_003), Timestamp(3))
        .expect("append after recovery");
    assert_eq!(outcome.entry().index, 3);

    let again = LedgerStore::open(&path).expect("reopen once more");
    assert_eq!(again.recovery().records, 4);
    assert_eq!(again.recovery().torn_tail_bytes, 0);
    assert_eq!(again.log().root(), reopened.log().root());
}

/// A record whose bytes changed after they were written is not replayed.
///
/// The last byte of the file sits inside the last entry's `transformation`, so the record still
/// decodes -- what catches it is that the leaf hash recomputed from the decoded fields is not the
/// `leaf_cid` the record carries. That is the same refusal `verify_inclusion` makes (AC-022's
/// `consistent_lie`), applied at the point where the log is read back rather than where a proof is
/// checked, and it is why this file adds no checksum: the record already carries a BLAKE3 digest
/// over three of its five fields.
#[test]
fn ac_069_a_record_whose_digest_no_longer_matches_is_not_replayed() {
    let path = scratch("ac_069_bitrot").join("ledger.log");
    fill(&path, 4);

    let mut bytes = fs::read(&path).expect("read the ledger");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    fs::write(&path, &bytes).expect("write the damage back");

    let reopened = LedgerStore::open(&path).expect("reopen over a damaged record");
    assert_eq!(
        reopened.recovery().records,
        3,
        "the damaged record must not be replayed"
    );
    assert!(reopened.recovery().torn_tail_bytes > 0);
}

/// Damage in the middle takes everything after it, and says so.
///
/// A log that skipped a bad record and carried on would renumber every entry after it: the tree
/// would be a different tree with the same later leaves, and every proof already issued against
/// the old root would be false. Stopping at the first refusal is the only recovery that keeps the
/// prefix meaning what it meant.
#[test]
fn ac_069_damage_in_the_middle_truncates_from_there() {
    let path = scratch("ac_069_middle").join("ledger.log");
    fill(&path, 4);
    let full = fs::metadata(&path).expect("metadata").len();

    let mut bytes = fs::read(&path).expect("read");
    bytes[8] = 0xFF; // inside the first record's payload
    fs::write(&path, &bytes).expect("write back");

    let reopened = LedgerStore::open(&path).expect("reopen");
    assert_eq!(reopened.recovery().records, 0);
    assert_eq!(reopened.recovery().torn_tail_bytes, full);
    assert_eq!(fs::metadata(&path).expect("metadata").len(), 0);
}

/// A length header that asks for more than the reader will ever hold is a torn tail (**A-1**).
///
/// A-1 verbatim (`req/38_ERRATA_2026-08-07.md` §18, adopted as a required DoD of M3's first hand):
/// "one fixture that guards replay's `MAX_RECORD_BYTES` check (a `0xFFFFFFFF` header -> a torn tail
/// cut without allocating). An M4 mutant survived = a concrete hole: zero tests guard store.rs's own claim" (sem: SEM-gx-log-139).
///
/// `store.rs` says the ceiling exists so that "four corrupted bytes ask for a four-gigabyte
/// allocation before anything has had a chance to refuse them" (sem: SEM-gx-log-140). What this half asserts is the
/// behaviour that claim implies and that a caller can see: the three good records survive, the
/// four bytes that make the impossible promise are removed from the file, and the store opens.
/// It does **not** assert that no allocation happened -- see the static half below for why that
/// sentence needs a different instrument.
#[test]
fn ac_069_a_length_header_over_the_ceiling_is_a_torn_tail() {
    let path = scratch("ac_069_ceiling").join("ledger.log");
    let returned = fill(&path, 3);
    let good = fs::metadata(&path).expect("metadata").len();

    let mut raw = OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open the file directly");
    raw.write_all(&u32::MAX.to_be_bytes())
        .expect("a length header promising 4 GiB - 1");
    raw.sync_all().expect("fsync the damage");
    drop(raw);

    let reopened = LedgerStore::open(&path).expect("reopen over an impossible length header");
    assert_eq!(reopened.recovery().records, 3, "the good prefix is kept");
    assert_eq!(
        reopened.recovery().torn_tail_bytes,
        4,
        "the four header bytes are the whole of the tail"
    );
    assert_eq!(
        reopened.log().entries(),
        returned.as_slice(),
        "the replayed entries are not the ones the appends returned"
    );
    assert_eq!(
        fs::metadata(&path).expect("metadata").len(),
        good,
        "the impossible header is removed from the file, not merely skipped in memory"
    );
}

/// The ceiling is consulted *before* the buffer is allocated, and it is the only such buffer.
///
/// This is the half that goes RED when `length > MAX_RECORD_BYTES` is deleted from the guard, and
/// it is a source scan for the same reason `ac_069_the_durability_barrier_has_exactly_one_call_site`
/// is one: **no behaviour visible to this process separates the two builds**. A reader without the
/// ceiling allocates `u32::MAX` zeroed bytes, reads the few bytes the file actually holds into
/// them, finds the record short and breaks -- reporting the same `Recovery` the guarded build
/// reports. Measured, not assumed: under that mutation the behavioural half above stays green and
/// this one fails (M3 hand 1, `req/61`). Saying which half proves which is the point (req/29 §4,
/// and the M4 mutation of req/58 §2.13 that survived every existing suite).
///
/// The claim "without allocating" (sem: SEM-gx-log-141) is an ordering between two lines, so an ordering is what is checked --
/// exactly the shape of `ac_069_the_entry_becomes_visible_only_after_the_barrier` one screen down.
#[test]
fn ac_069_the_record_ceiling_is_consulted_before_the_buffer_is_allocated() {
    let store = source("store.rs");
    let lines = code_lines(&store);

    let start = lines
        .iter()
        .position(|(_, l)| l.contains("fn replay("))
        .expect("store.rs has a replay function");
    let body: Vec<&(usize, String)> = lines[start..].iter().collect();

    let allocations: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, (_, l))| l.contains("vec![0u8; length as usize]"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        allocations.len(),
        1,
        "replay must have one buffer sized by the declared length, so that one gate covers it; \
         found {} at {:?}",
        allocations.len(),
        allocations.iter().map(|i| body[*i].0).collect::<Vec<_>>()
    );

    let gate = body
        .iter()
        .position(|(_, l)| l.contains("length > MAX_RECORD_BYTES"))
        .unwrap_or_else(|| {
            panic!(
                "replay does not compare the declared length against MAX_RECORD_BYTES; \
                 store.rs claims the ceiling stops four corrupted bytes asking for four gigabytes, \
                 and nothing else in this crate can make that true (A-1)"
            )
        });

    assert!(
        gate < allocations[0],
        "the ceiling is checked at line {} and the buffer is allocated at line {}; A-1 requires \
         that order -- a ceiling consulted after the allocation refuses nothing",
        body[gate].0,
        body[allocations[0]].0
    );
}

/// An empty file is an empty ledger, not a failure.
#[test]
fn ac_069_an_absent_file_opens_as_an_empty_ledger() {
    let path = scratch("ac_069_fresh").join("ledger.log");
    let store = LedgerStore::open(&path).expect("open a ledger that does not exist yet");
    assert!(store.log().is_empty());
    assert_eq!(store.recovery().records, 0);
    assert_eq!(store.recovery().torn_tail_bytes, 0);
    assert!(path.exists(), "opening creates the file");
}

// ---------------------------------------------------------------------------
// The static half: the barrier itself
// ---------------------------------------------------------------------------

/// There is exactly one durability barrier in `store.rs`, and one road to the disk.
///
/// This is the assertion that goes RED when the `fsync` is removed, and it is a source scan rather
/// than a behaviour because no behaviour visible to this process can tell a build that calls
/// `fsync` from one that does not (see the module docs). `tools/verify_m2h3.sh` performs that
/// removal and checks that this test fails -- the check of the check (req/36's false-PASS mutant table, (sem: SEM-gx-log-142)
/// same discipline).
#[test]
fn ac_069_the_durability_barrier_has_exactly_one_call_site() {
    let store = source("store.rs");
    let syncs = code_hits(&store, ".sync_all()");
    assert_eq!(
        syncs.len(),
        1,
        "NFR-009's fsync must be one line that every write goes through; found {} at {syncs:?}",
        syncs.len()
    );

    let writes = code_hits(&store, ".write_all(");
    assert_eq!(
        writes.len(),
        1,
        "one road to the file, so that no write can bypass the framing or the barrier; found {} \
         at {writes:?}",
        writes.len()
    );
}

/// The barrier is crossed *before* the entry becomes visible.
///
/// NFR-009 is an ordering ("completes the fsync before returning a success response") (sem: SEM-gx-log-143), and an ordering is not checked
/// by the presence of a call. `append` stages the entry, writes it, syncs, and only then commits
/// it to the tree; if those two lines were swapped, a caller could read an entry back out of a
/// log whose bytes were still in a buffer.
#[test]
fn ac_069_the_entry_becomes_visible_only_after_the_barrier() {
    let store = source("store.rs");
    let lines = code_lines(&store);

    let start = lines
        .iter()
        .position(|(_, l)| l.contains("pub fn append("))
        .expect("store.rs has a public append");
    let body: Vec<&(usize, String)> = lines[start..].iter().collect();

    let write = body
        .iter()
        .position(|(_, l)| l.contains("self.write_and_sync("))
        .expect("append writes the record through the barrier");
    let commit = body
        .iter()
        .position(|(_, l)| l.contains("self.tree.commit("))
        .expect("append commits the staged entry to the tree");

    assert!(
        write < commit,
        "the record is written and synced at line {} and committed to the tree at line {}; \
         NFR-009 requires that order",
        body[write].0,
        body[commit].0
    );
}

/// The platform gap is **declared**, and on this platform only a source scan can say so.
///
/// 🔴 **M5H8-10** (`req/38_ERRATA_2026-08-07.md` §45) came with two `cargo-mutants` survivors, not
/// one: `sync_parent_directory` has a `#[cfg(unix)]` body and a `#[cfg(not(unix))]` body, and
/// reducing **either** to `Ok(())` left every probe in the crate green (req/86 §3.2, rows 6 and 7).
///
/// The unix one is now behaviour: `store::tests::creating_a_ledger_pushes_its_directory_entry_to_
/// the_device` counts the call. The other one **cannot be**, and saying why is the point of this
/// probe rather than a caveat on it: v0.1 CI is x86_64 Linux (A-5), so that arm is not compiled
/// here, and no test that runs on this machine can distinguish a build in which it is a declared
/// no-op from a build in which it is an accidental one. What is left to check is that it is still
/// *declared* — that the parameter is visibly discarded and the two arms are still two arms — which
/// is the same reasoning, and the same remedy, as
/// `ac_069_the_durability_barrier_has_exactly_one_call_site` two screens up.
///
/// Do not read this as coverage of non-unix durability. gx makes **no** durability claim for a
/// ledger's *name* off unix; req/52 §5 records that no other platform was measured, and this probe
/// asserts that the source still says so out loud.
#[test]
fn ac_069_the_non_unix_directory_sync_is_a_declared_gap_and_still_says_so() {
    let store = source("store.rs");
    let lines = code_lines(&store);

    let definitions = code_hits(&store, "fn sync_parent_directory(");
    assert_eq!(
        definitions.len(),
        2,
        "one body per platform arm; found {} at {definitions:?}",
        definitions.len()
    );

    let gap = lines
        .iter()
        .position(|(n, _)| *n == definitions[1])
        .expect("the second definition is a code line");
    let body: Vec<&(usize, String)> = lines[gap..gap + 4].iter().collect();

    assert!(
        lines.iter().any(|(n, l)| *n < definitions[1]
            && *n + 2 > definitions[1]
            && l.contains("cfg(not(unix))")),
        "the second definition is not the `#[cfg(not(unix))]` arm; the scan is reading the wrong \
         function"
    );
    assert!(
        body.iter().any(|(_, l)| l.contains("let _ = path;")),
        "the non-unix arm must discard its argument in the open, so that a reader sees a gap \
         rather than a call; found {:?}",
        body.iter().map(|(n, l)| (*n, l.trim())).collect::<Vec<_>>()
    );
}
