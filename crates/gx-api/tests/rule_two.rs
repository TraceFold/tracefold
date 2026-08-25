// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **Rule 2 / M6-28** (sem: SEM-gx-api-385), one surface along: the clock and the entropy source are each read **once**.
//!
//! req/88 §3.1 Rule 2: 41 §6's "randomness and time are injected at the engine boundary" makes the surface the layer that turns an
//! outside-world reading into an injected argument, and M6-28, adopted (a), is the instrument — "measure by source scan
//! that `SystemTime::now()` and the rng seed's acquisition each happen exactly once across the whole CLI" (sem: SEM-gx-api-386).
//!
//! `probes/doubt/tests/m6_surface_doubt.rs` counts gx-cli's. This counts gx-api's, and the reason it
//! is a separate file rather than a line in that one is the dependency direction: the doubt crate
//! reads source paths and either would work, but the claim belongs beside the crate that has to keep
//! it — 44 §2's thirteen endpoints are thirteen chances for a second clock, and a receipt with two
//! answers to "when" (sem: SEM-gx-api-387) is a receipt nobody can order.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate must sit at <root>/crates/gx-api")
        .to_path_buf()
}

/// `//`-comments removed. The same reason `authority_boundary.rs` strips them: these files discuss
/// the words they are scanned for.
fn code_only(source: &str) -> String {
    source
        .lines()
        .map(|l| match l.find("//") {
            Some(at) => &l[..at],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `.rs` file under `crates/gx-api/src`, comment-free.
fn sources() -> Vec<(String, String)> {
    let dir = repo_root().join("crates/gx-api/src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("crates/gx-api/src exists") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let name = path
                .file_name()
                .expect("named")
                .to_string_lossy()
                .to_string();
            let body = std::fs::read_to_string(&path).expect("readable");
            out.push((name, code_only(&body)));
        }
    }
    assert!(
        !out.is_empty(),
        "no source under crates/gx-api/src — every counter below would be 0 because there is \
         nothing to count (§30)"
    );
    out.sort();
    out
}

/// 🔴 One wall clock and one entropy source, both in `state.rs`.
#[test]
fn the_clock_and_the_entropy_source_are_each_read_once() {
    let mut clock: Vec<String> = Vec::new();
    let mut entropy: Vec<String> = Vec::new();
    for (name, code) in sources() {
        for line in code.lines() {
            if line.contains("SystemTime::now(") {
                clock.push(name.clone());
            }
            if line.contains("RandomState::new(") {
                entropy.push(name.clone());
            }
        }
    }
    println!("API_CLOCK_CALL_SITES={clock:?} API_ENTROPY_CALL_SITES={entropy:?}");
    assert_eq!(
        clock,
        vec!["state.rs".to_string()],
        "Rule 2 (sem: SEM-gx-api-388): the real clock is read in exactly one place, and it is the file a reader opens to ask \
         \"where does this server learn the time\""
    );
    assert_eq!(
        entropy,
        vec!["state.rs".to_string()],
        "Rule 2: and the entropy `Engine::submit` is seeded from, for the same reason (sem: SEM-gx-api-389)"
    );
}

/// 🔴 **M6-28's other half** — there is no way to tell this surface what time it is.
///
/// > do not create a hidden `--at` flag — making the clock a CLI argument lets a receipt's `issued_at` lie (sem: SEM-gx-api-390)
///
/// The HTTP form of the same hole is a request **field**: an `at` in a body would let a client set
/// the timestamp on a receipt they are about to be given. The environment is scanned as well, for
/// M5H5-4(b)'s reason — it is the same flag with a longer name.
#[test]
fn no_request_can_set_the_clock() {
    let mut offences: Vec<String> = Vec::new();
    for (name, code) in sources() {
        for line in code.lines() {
            let t = line.trim();
            // 🔴 A **deserialised request** field called `at`/`now`/`timestamp`. Scoped to
            // `handlers.rs` because that is where 44 §2.2's request bodies are declared, and the
            // scoping is the probe being precise rather than lenient: `idempotency::Entry` has an
            // `at_unix_nanos` and it is a **stored** field written from `AppState::now`, so a scan
            // that flagged it would be reporting the clock's one call site as its own violation.
            // Response fields are built with `serde_json::json!` and match nothing here.
            if name == "handlers.rs" {
                for field in ["pub at:", "pub now:", "pub timestamp:", "pub at_unix"] {
                    if t.starts_with(field) {
                        offences.push(format!("{name}: {t}"));
                    }
                }
            }
            if let Some(at) = t.find("env::var") {
                let tail = t[at..].to_ascii_uppercase();
                if ["NOW", "TIME", "EPOCH", "CLOCK", "SEED", "RANDOM"]
                    .iter()
                    .any(|n| tail.contains(n))
                {
                    offences.push(format!("{name}: {t}"));
                }
            }
        }
    }
    println!("API_TIME_INPUTS={offences:?}");
    assert!(
        offences.is_empty(),
        "M6-28: a request that carries its own `at` is a receipt whose `issued_at` the client \
         chose, and an environment variable is the same input with a longer name (M5H5-4(b)): \
         {offences:?}"
    );
}

/// 🔴 **Rule 1's manifest half, for this crate** (sem: SEM-gx-api-391) — `gx-canon` is not a dependency and may not become one.
///
/// `crates/gx-canon/tests/authority_boundary.rs` already scans both secondary surfaces for all three
/// absences. This is the same claim stated where the crate's own reader is, because the manifest is
/// the place the breach would be **written** and 41 §6's monopoly is easiest to lose by adding a line
/// to a table "for later" (sem: SEM-gx-api-392).
#[test]
fn this_crate_cannot_name_the_canonical_layer() {
    let manifest = std::fs::read_to_string(repo_root().join("crates/gx-api/Cargo.toml"))
        .expect("the manifest");
    let declarations: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("gx-canon"))
        .collect();
    println!("GX_CANON_DECLARATIONS={}", declarations.len());
    assert!(
        declarations.is_empty(),
        "Rule 1 (i) (sem: SEM-gx-api-393): 41 §6 gives the canonical encode one door, and a surface that could mint a `Cid` \
         could name a transformation the engine never saw: {declarations:?}"
    );
    // 🔴 And gx-cli is absent for a **different** reason, which is worth its own line: 47 §1(a)
    // makes gx-cli contain gx-api, so a dependency this way would be a cycle cargo refuses.
    let cli: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("gx-cli"))
        .collect();
    println!("GX_CLI_DECLARATIONS={}", cli.len());
    assert!(
        cli.is_empty(),
        "47 §1(a): \"`gx-cli` folds `gx-api`'s functions in via `gx serve`\" (sem: SEM-gx-api-394), so the dependency runs the other \
         way and this one would close the loop: {cli:?}"
    );
}
