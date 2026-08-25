// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Fixtures shared by hand 3's suites.
//!
//! Not a test target: cargo builds one integration binary per `.rs` file directly under `tests/`,
//! and a file in a subdirectory is only ever compiled as a module of one that declares it. So a
//! helper here raises no `test result:` line of its own, which is what keeps the e2e floor of
//! `tools/e2e.sh` counting suites rather than support code.
//!
//! `#![allow(dead_code)]` because each suite declares the whole module and uses part of it; without
//! it, `-D warnings` (51 §11.1 stage 2) turns "this suite does not need `tid`" (sem: SEM-gx-log-170) into a build
//! failure.

#![allow(dead_code)]

use gx_core::{Cid, Timestamp, TransformationId};
use gx_log::tile::TileLog;
use std::fs;
use std::path::{Path, PathBuf};

/// A distinguishable digest. Not a real hash of anything -- these suites are about the tree and
/// the file, not about what a receipt is.
pub fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// A distinguishable transformation id, disjoint from the [`cid`] range used for receipts so a
/// swapped argument shows up as a wrong value rather than as a coincidence.
pub fn tid(seed: u64) -> TransformationId {
    TransformationId(cid(9_000_000 + seed))
}

/// An in-memory log of `n` entries, appended in order.
pub fn log_of(n: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        log.append(tid(i), cid(1_000 + i), Timestamp(i as i64))
            .expect("canonical");
    }
    log
}

/// An empty directory to put a ledger file in, under the cargo target directory.
///
/// `CARGO_TARGET_TMPDIR` rather than [`std::env::temp_dir`]: on this project's WSL2 setup the
/// system `/tmp` is cleared while the machine sits idle, and a suite whose fixtures evaporate
/// between two runs reports a filesystem's housekeeping as a durability failure.
///
/// Which filesystem that lands on follows `CARGO_TARGET_DIR` and is therefore **not fixed**: a
/// working-tree run puts it under `target/` beside the repository, which is drvfs (9p, ASM-01-1),
/// while `tools/e2e.sh` exports `$HOME/.sg/target` and puts it on ext4. Both were measured and
/// both are green (req/52 §2). Saying so matters because `fsync` on a 9p mount is not the syscall
/// NFR-009 is written about, so a green working-tree run is the weaker of the two.
///
/// Cleared on entry, not on exit. A test that fails leaves its ledger file behind to be read.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// Code lines of a source file, paired with their 1-based numbers.
///
/// Doc comments are dropped for the reason `ac_014.rs` and `ac_021.rs` drop them: this crate's
/// documentation quotes the rules it implements, and a scan that read the prose would report the
/// documentation of a rule as a breach of it.
pub fn code_lines(path: &Path) -> Vec<(usize, String)> {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let t = line.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .map(|(n, line)| (n + 1, line.to_string()))
        .collect()
}

/// A file of this crate's `src/`.
pub fn source(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name)
}

/// The 1-based line numbers on which `needle` occurs as a substring of a code line.
pub fn code_hits(path: &Path, needle: &str) -> Vec<usize> {
    code_lines(path)
        .into_iter()
        .filter(|(_, line)| line.contains(needle))
        .map(|(n, _)| n)
        .collect()
}
