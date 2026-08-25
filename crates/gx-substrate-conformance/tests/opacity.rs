// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **L8** — the five crates below the boundary read no delta grammar. (sem:
//! SEM-gx-substrate-conformance-108, SEM-gx-substrate-conformance-109, SEM-gx-substrate-conformance-110,
//! SEM-gx-substrate-conformance-111, SEM-gx-substrate-conformance-112, SEM-gx-substrate-conformance-113)
//!
//! req/69 §3.4, law L8: "zero code in the src of gx-core/canon/gate/witness/log reads fs delta
//! grammar", grounded in 11 §3's P-6 and 42 §3.4's "an opaque change description that only the
//! adapter interprets; core/gate/witness handle it only as a byte string". req/69 §6.2 makes the
//! scan a DoD of hand 3.
//!
//! # Why L8 is not in the law table next door
//!
//! Because it is not a property of an adapter. L1-L7 are questions a fixture can be asked; L8 is a
//! question about the rest of the workspace, and the subject of the sentence is
//! "gx-core/canon/gate/witness/log". Running it through the harness's fixture interface would put an
//! adapter in the
//! position of vouching for the crates it is supposed to be opaque to. `src/laws.rs` says where it
//! went; this file is where it is.
//!
//! # "the absence" needs a mutation, and there is one
//!
//! req/69 §8.2: "**checking 'the absence' needs a mutation** ... it becomes empty unless you measure
//! that **adding one path turns it RED** (the precedent of AC-029/AC-028)". `tools/verify_m4h3.sh`
//! §5 adds one line to gx-gate that decodes
//! `GateInput.planned` and prints what falls; without that measurement the three probes below would
//! be three ways of saying that a `grep` found nothing.

use std::path::{Path, PathBuf};

/// The five crates P-6 puts on the other side of the boundary from an adapter.
const BELOW_THE_BOUNDARY: [&str; 5] = ["gx-core", "gx-canon", "gx-witness", "gx-log", "gx-gate"];

/// What a delta looks like when it is being carried rather than read.
///
/// These are the names a lower crate legitimately touches: `PlannedDeltaBytes` is E-M3-1's carrier,
/// `DeltaRef` is ASM-16's reference, `planned` is `GateInput`'s field and `inverse_delta` is the
/// escrow field of 42 §3.10. Each is a handle; none of them is a grammar.
const DELTA_CARRIERS: [&str; 4] = ["PlannedDeltaBytes", "DeltaRef", ".planned", "inverse_delta"];

/// What reading one would look like.
///
/// The codec's own name is **not** in this list, and its absence was found by a test rather than
/// chosen: the first run of this file failed `ac_014_no_source_but_gx_canons_names_a_cbor_codec`,
/// because naming `serde_ipld_dagcbor` anywhere outside gx-canon is what AC-014 bans (42 §2.1-6).
/// The ban is the stronger guard -- it already makes a direct codec call below the boundary
/// impossible -- so L8 measures the road that is left, which is gx-canon's own `decode` re-exported
/// to whoever holds bytes. The two checks compose: AC-014 keeps the codec in one crate, and this
/// keeps that crate's decoder away from a delta.
const DECODERS: [&str; 3] = ["cbor::decode", "from_slice", "from_reader"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

/// Every `.rs` file under `<crate>/src`, as `(path, source)`.
fn sources_of(crate_name: &str) -> Vec<(String, String)> {
    let dir = repo_root().join("crates").join(crate_name).join("src");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("a crate's src is readable") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&path).expect("readable");
                out.push((path.display().to_string(), text));
            }
        }
    }
    out
}

/// The lines of a source that are code: doc comments and ordinary comments dropped.
///
/// The distinction is the whole test. Every one of the five crates *talks* about `PlannedDelta` and
/// about gx-substrate at length -- that is how P-6 is documented -- and none of them may act on one.
fn code_lines(source: &str) -> Vec<(usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            !t.starts_with("//") && !t.starts_with("*")
        })
        .map(|(n, l)| (n + 1, l))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. no edge
// ---------------------------------------------------------------------------

/// None of the five depends on gx-substrate.
///
/// The coarsest statement of L8 and the one that cannot be argued with: a crate that does not have
/// the boundary crate in its graph cannot name a delta type, whatever its source says. It is also
/// what keeps `cargo tree` acyclic, since gx-substrate depends on gx-core and gx-canon.
#[test]
fn no_crate_below_the_boundary_depends_on_gx_substrate() {
    let mut offenders: Vec<String> = Vec::new();
    for name in BELOW_THE_BOUNDARY {
        let manifest =
            std::fs::read_to_string(repo_root().join("crates").join(name).join("Cargo.toml"))
                .expect("a crate's manifest is readable");
        for line in manifest.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("gx-substrate") {
                offenders.push(format!("{name}: {}", line.trim()));
            }
        }
    }
    println!("L8_DEPENDENCY_EDGES={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "a crate below the boundary depends on gx-substrate: {offenders:?}. P-6 (11 §3) is a \
         statement about the dependency graph before it is one about any source line"
    );
}

// ---------------------------------------------------------------------------
// 2. no mention in code
// ---------------------------------------------------------------------------

/// None of the five names gx-substrate anywhere but in prose.
#[test]
fn no_crate_below_the_boundary_names_the_boundary_crate_in_code() {
    let mut offenders: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for name in BELOW_THE_BOUNDARY {
        for (path, source) in sources_of(name) {
            scanned += 1;
            for (n, line) in code_lines(&source) {
                if line.contains("gx_substrate") {
                    offenders.push(format!("{path}:{n}"));
                }
            }
        }
    }
    println!(
        "L8_FILES_SCANNED={scanned} L8_CODE_MENTIONS={}",
        offenders.len()
    );
    assert!(
        offenders.is_empty(),
        "gx-substrate is named in code below the boundary: {offenders:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. no decode of a carrier
// ---------------------------------------------------------------------------

/// No line below the boundary both holds a delta and decodes something.
///
/// The precise form of "code that reads fs delta grammar" that a scan can decide. A lower crate may
/// hold a
/// `PlannedDeltaBytes`, hash it, sign it, put it in a receipt and compare it -- 42 §3.4 asks for
/// exactly that -- and the one thing it may not do is turn those bytes into structure. The pairing
/// is what makes the check specific: `cbor::decode` on its own is gx-canon's job and gx-witness's
/// receipt path, and a carrier on its own is E-M3-1 working as designed.
#[test]
fn no_crate_below_the_boundary_decodes_a_delta_carrier() {
    let mut offenders: Vec<String> = Vec::new();
    for name in BELOW_THE_BOUNDARY {
        for (path, source) in sources_of(name) {
            for (n, line) in code_lines(&source) {
                let carries = DELTA_CARRIERS.iter().any(|c| line.contains(c));
                let decodes = DECODERS.iter().any(|d| line.contains(d));
                if carries && decodes {
                    offenders.push(format!("{path}:{n}: {}", line.trim()));
                }
            }
        }
    }
    println!("L8_CARRIER_DECODES={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "a crate below the boundary reads a delta rather than carrying it: {offenders:?}. 42 §3.4, \
         verbatim: \"an opaque change description that only the adapter interprets; core/gate/\
         witness handle it only as a byte string (P-6)\""
    );
}
