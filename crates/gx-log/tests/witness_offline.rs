// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **AC-B7** (`req/682` §5) — the equivocation detector opens no network and no file.
//!
//! `req/682` §4 puts the whole of Phase B's in-repo scope behind one line: "everything that makes,
//! compares, or fails-red is self-driving; only what *leaves* the machine is the operator's". The
//! detector is the compare half, and this file measures that it stays there — that
//! `detect_equivocation` and `classify_extension` reach for no socket and no file descriptor, so an
//! auditor can run them in the network-blocked environment AC-057 already builds and get the same
//! answer.
//!
//! Two statements, the same shape AC-008 uses for gx-core:
//!
//! 1. **the structure** — `the_detector_source_reaches_for_no_io` scans `witness_audit.rs`'s own
//!    source for any I/O path. A dependency the detector never names cannot be opened, and that is
//!    the durable statement: it keeps being true when a later hand adds a field, where a single
//!    passing run under `unshare` would only have proved the one input did not need the network.
//! 2. **the run** — `the_detector_opens_no_file_descriptor` counts the process's open descriptors
//!    across a real `detect_equivocation` / `classify_extension` call and asserts the count did not
//!    move. A scan for absence is empty unless something would fire, so
//!    `finds_a_planted_violation` shows the source scan firing, and the fd count is the run that
//!    would catch an I/O the source scan's word list missed.

use gx_core::{Checkpoint, Cid, DsseSignature, Timestamp};
use gx_log::proof::prove_consistency;
use gx_log::tile::TileLog;
use gx_log::{classify_extension, detect_equivocation};

/// The source of the detector, and only the detector. `witness_audit.rs` is the one file `req/682`
/// §6 places the offline scan on; the rest of `gx-log` reads and writes the store by design (that is
/// what a log does), so a crate-wide scan would be measuring the wrong thing.
const DETECTOR_SOURCE: &str = include_str!("../src/witness_audit.rs");

/// Crate names and std paths that would mean the detector had started reaching outside the process.
/// The `std::` entries are the file/socket/process/clock paths a pure arithmetic function has no use
/// for; the crate names are the networking stacks AC-008 rules out crate-wide.
const DENY: &[&str] = &[
    "std::net",
    "std::fs",
    "std::io",
    "std::process",
    "std::env",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "File::open",
    "File::create",
    "tokio",
    "reqwest",
    "hyper",
    "mio",
    "socket2",
    "async-std",
    "async_std",
];

/// Every needle in `DENY` that appears in `source`.
fn violations(source: &str) -> Vec<&'static str> {
    DENY.iter()
        .copied()
        .filter(|needle| source.contains(needle))
        .collect()
}

/// 🔴 **AC-B7, the structure** — the detector's source names no I/O path.
#[test]
fn the_detector_source_reaches_for_no_io() {
    let found = violations(DETECTOR_SOURCE);
    assert!(
        found.is_empty(),
        "the offline equivocation detector must reach for no I/O, but its source names: {found:?}"
    );
}

/// The scan is not vacuous: with an I/O path spliced into a copy of the source, it fires.
#[test]
fn finds_a_planted_violation() {
    let planted = format!("{DETECTOR_SOURCE}\nlet _ = std::net::TcpStream::connect(\"x\");\n");
    let found = violations(&planted);
    assert!(
        found.contains(&"std::net") && found.contains(&"TcpStream"),
        "a planted `std::net::TcpStream` must be caught, or the clean scan proves nothing: {found:?}"
    );
}

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

/// A signed checkpoint whose signature is filler: the detector never reads it (`req/682` §2-2).
fn checkpoint(origin: &str, tree_size: u64, root: Cid) -> Checkpoint {
    Checkpoint {
        origin: origin.to_string(),
        tree_size,
        root_hash: root,
        timestamp: Timestamp(1_700_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "ledger-key-1".to_string(),
            sig: vec![0xAB; 64],
        },
    }
}

fn log_seeded(n: u64, base: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&(base + i).to_be_bytes());
        log.append(
            gx_core::TransformationId(Cid(raw)),
            cid(u8::try_from((base + i) % 251).expect("in range")),
            Timestamp(i as i64),
        )
        .expect("canonical");
    }
    log
}

/// The number of file descriptors this process currently holds open. Linux-only: the count is read
/// from `/proc/self/fd`, which is where an opened socket or file would show up.
#[cfg(target_os = "linux")]
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .expect("this platform has /proc/self/fd")
        .count()
}

/// 🔴 **AC-B7, the run** — a real detector call opens no descriptor.
///
/// The reading of `/proc/self/fd` is done twice with the detector calls between; the reads
/// themselves open and close a directory handle each, so a difference of zero across the *calls* is
/// what a pure function produces, and any socket or file the detector opened and kept would raise
/// the after-count.
#[cfg(target_os = "linux")]
#[test]
fn the_detector_opens_no_file_descriptor() {
    // Inputs built before the first count, so the count brackets only the detector.
    let a = checkpoint("glovrex-ledger/v1", 100, cid(7));
    let b = checkpoint("glovrex-ledger/v1", 100, cid(9));
    let log = log_seeded(10, 0);
    let old = checkpoint("glovrex-ledger/v1", 5, log.root_at(5).expect("root"));
    let new = checkpoint("glovrex-ledger/v1", 10, log.root_at(10).expect("root"));
    let proof = prove_consistency(&log, 5, 10).expect("canonical proof");

    let before = open_fd_count();
    let eq = detect_equivocation(&[a, b]);
    let fork = classify_extension(&old, &new, &proof).expect("well-formed inputs");
    let after = open_fd_count();

    // Keep the results live across the second count so the optimiser cannot elide the calls.
    assert_eq!(
        eq.len(),
        1,
        "the equivocation fixture is a real detection, not a no-op"
    );
    assert!(
        fork.is_none(),
        "the extension fixture is a real, consistent extension"
    );
    assert_eq!(
        before, after,
        "the offline detector opened a descriptor it did not close: {before} -> {after} (AC-B7)"
    );
}
