// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Shared helpers for `tests/gate_conformance_gen.rs` (M8-c, `req/142` §1 M8-c item 1).
//!
//! A directory module, not a top-level file, for the reason `gx-canon/tests/support/mod.rs`
//! already states: cargo builds one test binary per top-level `tests/*.rs` file, and a top-level
//! `support.rs` would become a binary of its own with no tests in it.

#![allow(dead_code)]

use serde_json::Value;
use std::io::Write;

/// The real commit under test, read once per test binary. `git` is on PATH inside the same WSL2
/// checkout every other `tools/*.sh` script assumes (req/142 HARD rules).
pub fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// `<repo root>/conformance` — the only crossing between the WSL2 Rust side and the Windows lake
/// side (`req/142` §3: `/mnt/c` and `C:\` name the same bytes). Same two-`ancestors` idiom as
/// `probes/doubt/tests/floor_doubt.rs::repo_root`: this crate sits at `<root>/crates/gx-gate`,
/// two path segments below root.
pub fn conformance_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("gx-gate sits at <root>/crates/gx-gate")
        .join("conformance");
    std::fs::create_dir_all(&dir).expect("conformance/ is creatable");
    dir
}

pub fn write_jsonl(dir: &std::path::Path, name: &str, lines: &[Value]) {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path)
        .unwrap_or_else(|e| panic!("cannot create {}: {e}", path.display()));
    for v in lines {
        writeln!(f, "{v}").expect("a line writes");
    }
}
