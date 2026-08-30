// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Every shipped crate root in `crates/` (and, since P4, `sdk/`) forbids `unsafe` — including the
//! one that is a binary.
//!
//! # `sdk/` (P4, req/132 §5 item 1)
//!
//! `sdk/wasm-verify` is the first crate root outside `crates/`: the `wasm-bindgen` projection of
//! `gx_witness::receipt::verify_offline` that the TypeScript SDK's npm package bundles as compiled
//! WASM. It is shipped in exactly the sense this file already cares about -- "a crate root this
//! workspace compiles into something a user receives" (sem: SEM-gx-canon-122) -- so the walker below reads `sdk/` beside
//! `crates/` rather than gaining an exception, the same choice M4 hand 3 made for
//! `gx-substrate-conformance`'s `publish = false`.
//!
//! # Where this came from
//!
//! M2 hand 6 ran `cargo geiger` over the workspace for the first time (51 §10's neighbourhood: the
//! unsafe inventory of what gx depends on). Three of the four gx packages came back `:)` — "no
//! unsafe usage found, declares `#![forbid(unsafe_code)]`" — and `gx-canon` came back `?` — "no
//! unsafe usage found, **missing** `#![forbid(unsafe_code)]`" (sem: SEM-gx-canon-123).
//!
//! `crates/gx-canon/src/lib.rs` has the attribute. What it does not have is the *other* crate root
//! in the same package: `tests/bin/cid_probe.rs`, the separate binary AC-011 requires ("a
//! completely independent child process B (a separate binary)") (sem: SEM-gx-canon-124), pointed at by an explicit `[[bin]]` and compiled under the
//! `probe-bin` feature. An attribute in `lib.rs` says nothing about it, because it is a different
//! crate. So the package's guarantee was one crate root short, and no test could see it — geiger
//! could, on its first run.
//!
//! # Why a source scan rather than a lint
//!
//! `#![forbid(unsafe_code)]` is per crate root by construction: there is no workspace-level form of
//! it, and `[lints]` in a manifest configures a lint level rather than the `forbid` that cannot be
//! locally overridden. The declaration therefore has to be repeated, and a thing that has to be
//! repeated needs something that counts the repetitions. That is this file.
//!
//! Same shape as `gx-witness/tests/evidence_cid.rs`, which reads its own crate's source to prove an
//! absence, and as `ac_018`, which reads gx-log's manifest. §13 H4-6's limit is about parsing the
//! **canonical markdown** (sem: SEM-gx-canon-125) and is not this.
//!
//! # What is deliberately not covered
//!
//! * **Test binaries.** Each file under `tests/` is a crate root too, and none carries the
//!   attribute. A test is not shipped, `unsafe` in one cannot reach a user, and requiring it there
//!   would add nineteen attributes that say nothing about the product. The count below is of the
//!   roots that are *compiled into the deliverable*.
//! * **`probes/doubt`.** It has no `src/` at all -- only `tests/` and `benches/` against the
//!   read-only Alpha tree -- so it has no shipped crate root to carry the attribute.
//! * **`fuzz/`.** Outside the workspace, nightly-only, and `libfuzzer_sys::fuzz_target!` expands to
//!   `unsafe` by construction (`#[no_mangle] pub unsafe extern "C" fn`). Forbidding it there would
//!   forbid the harness.

use std::path::{Path, PathBuf};

/// The crate roots that are compiled into something a user receives.
///
/// A list rather than a glob, so that adding a crate root is a decision somebody writes down.
/// `every_shipped_crate_root_is_in_the_list` below is the other half: it walks the tree and fails
/// if a root exists that this list does not name, which is what stops the list from being a
/// comfortable fiction.
const SHIPPED_CRATE_ROOTS: [&str; 24] = [
    // M6 hand 1: the thirteenth, fourteenth and fifteenth. `gx-cli` has **two** roots because
    // NFR-019 asks for a single static binary and 44 spells every command `gx <verb>`: the library
    // is what the tests drive and `src/main.rs` is what a user runs, and `#![forbid(unsafe_code)]`
    // is per crate root -- a binary does not inherit its library's.
    //
    // 🔴 req/88 §6.2 hand 1 ① writes the DoD as "`unsafe_forbidden`'s crate root **12**" (sem: SEM-gx-canon-126), which is the
    // value this list already held at member 10. The number was carried from the member count rather
    // than measured, exactly as M5H1-2 recorded of req/78's "8" (sem: SEM-gx-canon-127). Fifteen is the measurement.
    // Raised as **M6H1-3**.
    "crates/gx-api/src/lib.rs",
    // 🔴 S③ (`req/493`). `src/linux.rs` is a module and not a crate root, so it is not
    // here; the walker collects roots, and a module listed as one would be a claim about
    // the tree that the tree does not make.
    "crates/gx-confine/src/lib.rs",
    "crates/gx-cli/src/lib.rs",
    "crates/gx-cli/src/main.rs",
    "crates/gx-adapter-fs/src/lib.rs", // M4 hand 4: the ninth, and the first real adapter
    // 🔴 M7 hand 1: the sixteenth, and the **second** real adapter. It is the first crate root in
    // this list with an external dependency behind it (`gix`, which 41 §2 names on its row and 32
    // FR-045 makes a MUST), so `#![forbid(unsafe_code)]` here is a statement about this crate's own
    // source and not about gitoxide's -- the attribute cannot reach a dependency, and saying so is
    // better than letting a reader of the list infer otherwise.
    "crates/gx-adapter-git/src/lib.rs",
    // 🔴 M7 hand 3: the seventeenth, and the **third** real adapter. Back to no external
    // dependency -- and here that is AC-051's doing rather than thrift: a linked MCP client would be a
    // second road to the substrate, with no `Admitted` in its signature (`gx-adapter-mcp/Cargo.toml`).
    "crates/gx-adapter-mcp/src/lib.rs",
    // 🔴 **P1** (`req/115` §A, `req/38` §76's P3 DoD close): the twentieth, and the **fourth** real
    // adapter. No linked client of its own kind either -- `postgres`/`sqlparser` are ordinary crate
    // dependencies, not a second road into the substrate (42 §3.1's `Custom(String)` escape hatch
    // used unmodified).
    "crates/gx-adapter-postgres/src/lib.rs",
    // 🔴 **req/521**: the nineteenth workspace member and the **fifth** real adapter root. Same
    // shape as the postgres row above -- `mysql`/`sqlparser` are ordinary crate dependencies and
    // not a second road into the substrate, and `SubstrateKind::Custom(String)` is used unmodified
    // for the second time. `#![forbid(unsafe_code)]` is in its crate root for the same reason every
    // other member carries it: an adapter is the only code allowed to touch its substrate, so it is
    // the last place a raw pointer should be reachable.
    "crates/gx-adapter-mysql/src/lib.rs",
    "crates/gx-core/src/lib.rs",
    "crates/gx-canon/src/lib.rs",
    // M5 hand 1: the tenth workspace member and the first crate that names every layer below it.
    // req/78 §6.2 hand 1 ② writes the DoD as "the crate root list becomes **8**" (sem: SEM-gx-canon-128); the list is ten after
    // this line, because it counts crate *roots* and not packages -- nine `src/lib.rs` plus the
    // AC-011 probe binary. The number in the ruling was carried from M4 hand 1's seven and never
    // re-measured (req/78 §10 records that the batch ran no cargo). Raised as M5H1-2.
    "crates/gx-engine/src/lib.rs",
    "crates/gx-gate/src/lib.rs", // M3 hand 1: the sixth workspace member
    "crates/gx-log/src/lib.rs",
    // 🔴 **P3** (`req/119` §2): the wire, and the probe server AC-P3-1/2/4 are measured against.
    // The probe binary is behind `required-features = ["probe-bin"]` and is listed for the reason
    // `crash_probe` and `cid_probe` are: "shipped" here means "a crate root this workspace
    // compiles" (sem: SEM-gx-canon-129), and a root that forbade nothing would be the one place unsafe could enter.
    "crates/gx-mcp-wire/src/lib.rs",
    "crates/gx-mcp-wire/tests/bin/mcp_probe_server.rs",
    "crates/gx-substrate/src/lib.rs", // M4 hand 1: the seventh
    // M4 hand 3: the eighth. `publish = false` (E-M4-19), so "shipped" is not literally true of it (sem: SEM-gx-canon-130)
    // -- but the walker below finds every crate root under `crates/`, and the honest way to keep the
    // two halves equal is to name it rather than to teach the walker an exception. A test harness
    // that reached for `unsafe` would be a test harness whose results are about something else.
    "crates/gx-substrate-conformance/src/lib.rs",
    "crates/gx-witness/src/lib.rs",
    "crates/gx-canon/tests/bin/cid_probe.rs", // the AC-011 probe binary, via [[bin]]
    // M5 hand 2: the eleventh. AC-030 asks the two-stage identity to hold "a completely independent separate process" (sem: SEM-gx-canon-131)
    // as well as twice in one, and 34's AC-011 row names itself as the precedent for how that is
    // spawned. So this is `cid_probe.rs`'s shape one crate over -- a `[[bin]]` under `tests/bin/`,
    // gated behind a feature the package's own dev-dependency turns on, so that `cargo build` and
    // `cargo package` never see it while `cargo test` always does.
    "crates/gx-engine/tests/bin/engine_id_probe.rs",
    // M5 hand 5: the twelfth. 51 §8.1 asks for crash recovery to be measured "with a real binary,
    // a real process" (sem: SEM-gx-canon-132), and `gx-cli` is M6's, so the real binary is a second `[[bin]]` under `tests/bin/` — same
    // gate, same reasoning, and named here for the same reason the one above it is: the walker finds
    // every crate root under `crates/`, and a root the list does not name is a root nobody checked.
    "crates/gx-engine/tests/bin/crash_probe.rs",
    // 🔴 **P4** (`req/132` §5 item 1): the twenty-first, and the first root outside `crates/` --
    // `sdk/wasm-verify`, the WASM half of the TypeScript SDK's offline receipt verification.
    "sdk/wasm-verify/src/lib.rs",
    // 🔴 **#188/#189** (2026-08-31): the twenty-fourth, and the first root that is a *repository
    // root entry* rather than something under a group directory -- `tui/`, the terminal face
    // extracted from `crates/gx-cli/src/tui/`. The walker below found nothing here until it was
    // told to look: it enumerates `crates/` and `sdk/` as groups, and a member with no group above
    // it has no directory to be discovered in. That is why `ROOT_LEVEL_MEMBERS` exists beside it —
    // a root nobody names is a root nobody checks, which is this file's second test in one line.
    "tui/src/lib.rs",
];

/// Workspace members that live at the repository root rather than under `crates/` or `sdk/`.
///
/// Named rather than derived, for the same reason `SHIPPED_CRATE_ROOTS` is: the walk below needs
/// something to walk, and "everything at the root that has a Cargo.toml" would make this list
/// unable to disagree with the tree.
const ROOT_LEVEL_MEMBERS: [&str; 1] = ["tui"];

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<repo>/crates/gx-canon`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

/// The workspace member a listed crate root belongs to: `crates/<name>/…` → `crates/<name>`,
/// `sdk/<name>/…` → `sdk/<name>`, and `tui/…` → `tui`.
///
/// 🔴 The third case is not cosmetic. A member at the repository root has **one** path segment, so
/// the two-segment rule would answer `tui/src` — which no manifest declares, so `workspace_declares`
/// would answer `false` and the root would be *skipped as a crate this tree does not carry*. The
/// guard written to keep the published tree honest would have quietly stopped checking a crate the
/// private tree does ship. Group prefixes are named rather than inferred so that a fourth group
/// added tomorrow is a decision here rather than a silent reclassification.
fn member_of(rel: &str) -> String {
    let first = rel.split('/').next().unwrap_or(rel);
    if ["crates", "sdk", "probes"].contains(&first) {
        rel.splitn(3, '/').take(2).collect::<Vec<_>>().join("/")
    } else {
        first.to_string()
    }
}

/// Whether this tree's workspace declares `member` — read from the root `Cargo.toml`'s
/// `members` array, inside which nothing but member paths may be written (the public root's own
/// rule, `public/Cargo.toml`).
///
/// `req/833`: the published tree (`req/817`) carries its own 14-member root; `req/789` §3 holds
/// four crates private, so five of the roots `SHIPPED_CRATE_ROOTS` names have no file there. The
/// guard is keyed on the **declaration**, not on bare absence, so req/29 §4 stays intact: a root
/// whose crate the manifest declares and whose file is gone still fails; only roots of crates
/// this tree deliberately does not carry are set aside, loudly.
fn workspace_declares(member: &str) -> bool {
    let manifest = repo_root().join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", manifest.display()));
    let Some(start) = text.find("members = [") else {
        return false;
    };
    let Some(end) = text[start..].find(']') else {
        return false;
    };
    text[start..start + end]
        .lines()
        .any(|l| l.trim().trim_end_matches(',').trim_matches('"') == member)
}

/// The criterion.
#[test]
fn every_shipped_crate_root_forbids_unsafe() {
    let root = repo_root();
    let mut missing = Vec::new();
    let mut not_carried = 0usize;

    for rel in SHIPPED_CRATE_ROOTS {
        if !workspace_declares(&member_of(rel)) {
            eprintln!(
                "SKIP {rel}: this tree's workspace does not declare {} (req/789 §3 private \
                 crate; published tree, req/817). Its forbid(unsafe_code) is measured on the \
                 private tree (req/833).",
                member_of(rel)
            );
            not_carried += 1;
            continue;
        }
        let path = root.join(rel);
        let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{} is named in the list but unreadable: {e}",
                path.display()
            )
        });
        if !source.contains("#![forbid(unsafe_code)]") {
            missing.push(rel);
        }
    }

    println!(
        "SHIPPED_CRATE_ROOTS={} FORBID_UNSAFE_MISSING={} NOT_CARRIED_BY_THIS_TREE={}",
        SHIPPED_CRATE_ROOTS.len(),
        missing.len(),
        not_carried
    );
    assert!(
        missing.is_empty(),
        "these shipped crate roots do not forbid unsafe: {missing:?}. \
         `#![forbid(unsafe_code)]` is per crate root; a binary does not inherit its library's."
    );
}

/// The half that stops the list above from being a comfortable fiction.
///
/// Walks `crates/` and `sdk/` (P4, req/132 §5 item 1) for anything that is a crate root by cargo's
/// rules -- `src/lib.rs`, `src/main.rs`, files under `src/bin/`, and the one file an explicit
/// `[[bin]]` points at -- and fails if it finds one the list does not name. Without this, a new
/// binary added tomorrow would be forbidden-free and the test above would still pass, which is
/// req/08 N-1's shape (a name that has become a lie).
#[test]
fn every_shipped_crate_root_is_in_the_list() {
    let root = repo_root();
    let mut found: Vec<String> = Vec::new();

    for group in ["crates", "sdk"] {
        let Ok(entries) = std::fs::read_dir(root.join(group)) else {
            continue; // `crates/` always exists; a `sdk/` with no readable dir yet is not this test's job
        };
        for entry in entries {
            let dir = entry.expect("readable").path();
            if !dir.is_dir() || !dir.join("Cargo.toml").is_file() {
                continue; // `sdk/typescript` has no Cargo.toml: not a crate root, skipped rather than flagged
            }
            for candidate in ["src/lib.rs", "src/main.rs"] {
                if dir.join(candidate).is_file() {
                    found.push(format!(
                        "{group}/{}/{candidate}",
                        dir.file_name().expect("named").to_string_lossy()
                    ));
                }
            }
            if let Ok(bins) = std::fs::read_dir(dir.join("src/bin")) {
                for b in bins.flatten() {
                    found.push(
                        b.path()
                            .strip_prefix(&root)
                            .expect("under the root")
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
            // The explicit `[[bin]]` roots, read from the manifest rather than guessed: cargo only
            // auto-discovers `src/bin/`, and gx-canon's probe deliberately lives elsewhere so that
            // `src/` still holds exactly the modules 41 §2 lists.
            let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
            for line in manifest.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("path = ") {
                    let p = rest.trim().trim_matches('"');
                    if p.ends_with(".rs") && dir.join(p).is_file() {
                        found.push(format!(
                            "{group}/{}/{p}",
                            dir.file_name().expect("named").to_string_lossy()
                        ));
                    }
                }
            }
        }
    }

    // 🔴 **#188/#189** — members that are themselves root entries. The loop above enumerates
    // *groups* and asks each child whether it is a crate; a member with no group above it is
    // never offered to that question, so `tui/src/lib.rs` would have been in the list with
    // nothing walking towards it — a declaration checked in one direction only. Walked directly
    // here, with the same three questions the group loop asks.
    for name in ROOT_LEVEL_MEMBERS {
        let dir = root.join(name);
        if !dir.join("Cargo.toml").is_file() {
            continue;
        }
        for candidate in ["src/lib.rs", "src/main.rs"] {
            if dir.join(candidate).is_file() {
                found.push(format!("{name}/{candidate}"));
            }
        }
        if let Ok(bins) = std::fs::read_dir(dir.join("src/bin")) {
            for b in bins.flatten() {
                found.push(
                    b.path()
                        .strip_prefix(&root)
                        .expect("under the root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
        for line in manifest.lines() {
            if let Some(rest) = line.trim().strip_prefix("path = ") {
                let p = rest.trim().trim_matches('"');
                if p.ends_with(".rs") && dir.join(p).is_file() {
                    found.push(format!("{name}/{p}"));
                }
            }
        }
    }

    found.sort();
    found.dedup();
    // req/833: hold the walk only to the roots whose crates this tree's workspace declares —
    // the published tree (req/817) does not carry the four req/789 §3 private crates. A root
    // set aside here is announced; a declared root the walk does not find still fails below.
    let mut declared: Vec<String> = SHIPPED_CRATE_ROOTS
        .iter()
        .filter(|rel| {
            let carried = workspace_declares(&member_of(rel));
            if !carried {
                eprintln!(
                    "SKIP {rel}: this tree's workspace does not declare {} (req/789 §3 private \
                     crate; published tree, req/817). Checked on the private tree (req/833).",
                    member_of(rel)
                );
            }
            carried
        })
        .map(|s| (*s).to_string())
        .collect();
    declared.sort();

    println!(
        "SHIPPED_CRATE_ROOTS_FOUND={} DECLARED={}",
        found.len(),
        declared.len()
    );
    assert_eq!(
        found, declared,
        "the shipped crate roots on disk are not the ones SHIPPED_CRATE_ROOTS names"
    );
}
