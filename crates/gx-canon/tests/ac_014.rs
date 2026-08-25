// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-014 (FR-014) — nothing outside gx-canon encodes DAG-CBOR, and there is no second road to
//! a `Cid`.
//!
//! AC-014 verbatim: "Given: the source of every crate in the workspace. When: run a custom
//! lint equivalent to `cargo deny check bans` (checking that nothing outside gx-canon directly
//! imports a DAG-CBOR/CBOR encoder). Then: zero violations." (sem: SEM-gx-canon-035)
//!
//! # Why this is a test and not a `deny.toml`
//!
//! `cargo deny check bans` bans a crate from the *graph*, and gx-canon's graph is the workspace's
//! graph -- so a ban strong enough to catch a stray import in gx-core would also fail on
//! gx-canon's legitimate one. The AC says "an equivalent rule … a custom lint" (sem: SEM-gx-canon-036) for that reason. A test
//! also runs where the ban matters: `cargo test` on a clone, with no extra tool installed.
//!
//! # The two questions
//!
//! 1. **Manifest**: does any package other than gx-canon *declare* a CBOR codec dependency?
//! 2. **Source**: does any file outside `gx-canon/src` *name* one?
//!
//! Both are needed. A dependency declared and unused is a door left open, and a path written
//! without a declaration would not compile today but would the moment someone adds the crate.
//!
//! # And the part AC-014 exists for
//!
//! 42 §2.1-6's rule is "all canonical encoding goes through gx-canon" (sem: SEM-gx-canon-037), which is a claim about
//! roads, not about imports. `cbor::scan_strict`'s doc says the rule "is not a property of a byte
//! string and cannot be checked here -- AC-014's static check owns it". So this file also checks
//! that the hash is taken in exactly one place: `blake3` is named on one code line in the whole
//! workspace, in `cid.rs`. req/42 §4-B secured that at the level of the function's type; this
//! secures it at the level of the repository.
//!
//! # 2026-08-08, M2 hand 2: one place, more than one road
//!
//! **E-M2-12** (`req/38_ERRATA_2026-08-07.md` §8) adds the domain-separated mint 42 §3.11 needs
//! (`leaf_hash = BLAKE3(0x00 || canonical_dagcbor(leaf))`), because the alternative was gx-log
//! calling the digest itself -- the very thing this file forbids. So a second road to a `Cid` now
//! exists on purpose, and "the digest is reachable only through `cid::compute`" (sem: SEM-gx-canon-038) stopped being
//! true the moment the ruling landed.
//!
//! A test whose assertion still passes while its name has become false is req/08 N-1's disease,
//! so the check was not left as it was. It is now two claims, both narrower than the sentence they
//! replace: the digest is *taken* in one place (unchanged, and still mechanical), and the set of
//! functions that *reach* it is declared here by name -- a third road fails this file until
//! somebody writes it down. That is strictly more than the old check asserted, because the old one
//! could not have noticed a second road inside `cid.rs` at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// CBOR codecs, by the name they appear under in a manifest and in a `use` path.
///
/// `serde_ipld_dagcbor` and `ipld-core` are the two 42 §2.1-6 admits, in gx-canon alone. The rest
/// are the encoders someone reaching for CBOR would plausibly pull in; banning only the admitted
/// two would leave the interesting failure -- a second encoder -- unchecked.
const CBOR_CODECS: &[&str] = &[
    "serde_ipld_dagcbor",
    "serde-ipld-dagcbor",
    "ipld_core",
    "ipld-core",
    "ciborium",
    "serde_cbor",
    "serde-cbor",
    "minicbor",
    "cbor4ii",
    "cbor-codec",
];

/// The digest. It may be called from one line, and that line is inside `cid.rs`.
const HASHES: &[&str] = &["blake3"];

fn workspace_root() -> PathBuf {
    // <root>/crates/gx-canon -> <root>
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent()
        .and_then(Path::parent)
        .expect("gx-canon sits two levels below the workspace root")
        .to_path_buf()
}

/// Every `Cargo.toml` in the workspace, package name -> path.
///
/// Read from the directory rather than from `cargo metadata`: the question is what the repository
/// holds, and a member that was quietly dropped from the workspace list would vanish from
/// `cargo metadata` while its source stayed on disk.
fn manifests(root: &Path) -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    for group in ["crates", "probes"] {
        let dir = root.join(group);
        assert!(
            dir.is_dir(),
            "{} is missing; a check that cannot find the tree it audits must fail, not pass \
             (req/29 §4, fail-open forbidden) (sem: SEM-gx-canon-039)",
            dir.display()
        );
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("dir entry").path();
            let manifest = path.join("Cargo.toml");
            if manifest.is_file() {
                let name = path
                    .file_name()
                    .expect("directory has a name")
                    .to_string_lossy()
                    .into_owned();
                out.insert(name, manifest);
            }
        }
    }
    assert!(
        out.len() >= 3,
        "found only {} packages ({:?}); the workspace has gx-core, gx-canon and doubt at least, \
         so this scan is looking in the wrong place",
        out.len(),
        out.keys().collect::<Vec<_>>()
    );
    out
}

/// Every `.rs` file under `dir`, recursively.
fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return out;
    }
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(sources(&path));
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    out
}

/// The lines of `path` that are code rather than comment, with 1-based numbers.
///
/// Doc comments cite these crate names on purpose -- gx-canon's own module docs name
/// `serde_ipld_dagcbor` in a sentence -- so a scan that counted comments would report the
/// documentation of the rule as a violation of it.
fn code_lines(path: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(path).expect("source is UTF-8");
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let t = line.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .map(|(n, line)| (n + 1, line.to_string()))
        .collect()
}

#[test]
fn ac_014_no_package_but_gx_canon_declares_a_cbor_codec() {
    let root = workspace_root();
    let mut violations: Vec<String> = Vec::new();

    for (package, manifest) in manifests(&root) {
        if package == "gx-canon" {
            continue;
        }
        for (n, line) in code_lines(&manifest) {
            // A dependency line is `name = ...` or `name.workspace = true`; the crate name is the
            // first token. Matching the whole line would flag a `description` that mentions CBOR.
            let name = line
                .split(['=', '.', ' '])
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches('"');
            if CBOR_CODECS.contains(&name) {
                violations.push(format!("{}:{n}: {}", manifest.display(), line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "only gx-canon may depend on a CBOR codec (42 §2.1-6, AC-014):\n{}",
        violations.join("\n")
    );
}

/// The one package the ban does not apply to, tests included.
///
/// AC-014 says "not from outside gx-canon" (sem: SEM-gx-canon-040), and gx-canon's own test suite is inside gx-canon. It is also
/// where the codec *has* to be named for a second time on purpose: `ac_012` decodes with the
/// codec directly to check gx's idempotence against something other than gx, and `ac_011` calls
/// `blake3` directly to check that the digest gx produced is the digest of the bytes gx encoded.
/// A ban that covered those would forbid the independent cross-checks that make the acceptance
/// tests worth running.
fn gx_canon_dir(root: &Path) -> PathBuf {
    root.join("crates").join("gx-canon")
}

#[test]
fn ac_014_no_source_but_gx_canons_names_a_cbor_codec() {
    let root = workspace_root();
    let allowed = gx_canon_dir(&root);
    let mut violations: Vec<String> = Vec::new();
    let mut scanned = 0usize;

    for group in ["crates", "probes"] {
        for path in sources(&root.join(group)) {
            if path.starts_with(&allowed) {
                continue;
            }
            scanned += 1;
            for (n, line) in code_lines(&path) {
                for codec in CBOR_CODECS {
                    let underscored = codec.replace('-', "_");
                    if line.contains(&underscored) {
                        violations.push(format!("{}:{n}: {}", path.display(), line.trim()));
                    }
                }
            }
        }
    }

    assert!(
        scanned > 10,
        "only {scanned} files were scanned; a check that reads nothing reports nothing"
    );
    assert!(
        violations.is_empty(),
        "canonical encoding goes through gx-canon and nowhere else (42 §2.1-6, AC-014); \
         {scanned} files scanned:\n{}",
        violations.join("\n")
    );
}

#[test]
fn ac_014_the_digest_is_taken_in_exactly_one_place() {
    let root = workspace_root();
    let canon_src = root.join("crates").join("gx-canon").join("src");
    let canon_any = gx_canon_dir(&root);
    let mut outside: Vec<String> = Vec::new();
    let mut inside: Vec<String> = Vec::new();
    let mut in_canon_tests = 0usize;

    for group in ["crates", "probes"] {
        for path in sources(&root.join(group)) {
            for (n, line) in code_lines(&path) {
                for hash in HASHES {
                    if line.contains(hash) {
                        let hit = format!("{}:{n}: {}", path.display(), line.trim());
                        if path.starts_with(&canon_src) {
                            inside.push(hit);
                        } else if path.starts_with(&canon_any) {
                            in_canon_tests += 1;
                        } else {
                            outside.push(hit);
                        }
                    }
                }
            }
        }
    }

    println!("AC014_HASH_LINES_IN_GX_CANON_TESTS={in_canon_tests}");
    assert!(
        outside.is_empty(),
        "the digest may only be taken inside gx-canon (AC-014):\n{}",
        outside.join("\n")
    );
    // One call site. Not one *caller*: E-M2-12 gives the mint its own road (see the module docs
    // and the test below), and both roads end at the same private line. Two call sites would mean
    // two places where the shape of the input to the hash is decided, which is the bypass AC-014 (sem: SEM-gx-canon-041)
    // forbids however few functions are on top of it.
    assert_eq!(
        inside.len(),
        1,
        "the hash is called from {} places; AC-014 admits one:\n{}",
        inside.len(),
        inside.join("\n")
    );
    assert!(
        inside[0].contains("cid.rs"),
        "the single call site is not in cid.rs: {}",
        inside[0]
    );
}

/// Every public function of gx-canon that yields a `Cid`, by name.
///
/// Four of them take the digest -- [`compute`] through an `IdentityView` (42 §1.1, §1.3), the two
/// mints through a domain byte and raw bytes (42 §3.11, E-M2-12), and `of_canonical_bytes` over
/// bytes the caller already holds (`req/38` §324 ruling 3) -- and `from_text` takes none: it parses
/// a spelling somebody else minted (42 §1.2). The distinction is written into the list rather than
/// inferred from a naming convention, because a convention is what a fifth road would be added
/// under.
///
/// 🔴 **`req/38` §324 ruling 3 added the fifth entry, and the reason is worth the line.** The
/// question [`compute`] answers is "what does this **value** digest to", and for a document signed
/// under an earlier release that is the wrong question — the value is reached by decoding into this
/// year's type, whose canonical form is not the one the bytes were written in. `req/519` §7-5
/// measured the cost (every member ever added had already moved every historical ledger leaf) and
/// §324 caught the third repeat. `of_canonical_bytes` answers "what do these **bytes** digest to",
/// which is the question a holder of a signed document can ask. Same private call site; a road is
/// added here whenever the *question* differs, which is exactly what E-M2-12 established.
const CID_PRODUCING_FUNCTIONS: &[&str] = &[
    "compute",            // digest, through the projection
    "from_text",          // parse, no digest
    "mint",               // digest, domain byte + raw parts
    "mint_leaf",          // digest, domain byte + canonical form
    "mint_node",          // digest, domain byte + two children
    "of_canonical_bytes", // digest, over bytes the caller holds (req/38 §324)
];

/// The roads to a `Cid` are the declared ones (AC-014, as E-M2-12 leaves it).
///
/// The scan reads signatures, not call graphs: a `pub fn` in `cid.rs` whose return type mentions
/// `Cid` is a road, and this test fails until its name is in the list above. That is the weaker
/// half of what the old single-call-site assertion claimed and the stronger half of what it
/// actually checked -- it never could have seen a second function inside `cid.rs`.
#[test]
fn ac_014_the_roads_to_a_cid_are_the_declared_ones() {
    let root = workspace_root();
    let cid_rs = root
        .join("crates")
        .join("gx-canon")
        .join("src")
        .join("cid.rs");
    let mut found: Vec<String> = Vec::new();

    for (_, line) in code_lines(&cid_rs) {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("pub fn ")
            .or_else(|| trimmed.strip_prefix("pub const fn "))
        else {
            continue;
        };
        // The return type is on the signature line for every function in this file; a signature
        // wrapped across lines would be missed, so the count is asserted below as well.
        if !line.contains("-> Cid") && !line.contains("-> Result<Cid>") {
            continue;
        }
        let name = rest
            .split(['(', '<', ' '])
            .next()
            .unwrap_or_default()
            .to_string();
        found.push(name);
    }

    found.sort();
    found.dedup();
    let expected: Vec<String> = CID_PRODUCING_FUNCTIONS
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        found, expected,
        "the set of gx-canon functions that yield a `Cid` changed; declare the new road in \
         CID_PRODUCING_FUNCTIONS and say in its doc whether it takes the digest (AC-014, E-M2-12)"
    );
    println!("AC014_CID_ROADS={}", found.join(","));
}

/// The scan is not vacuous: it finds the legitimate uses it is supposed to leave alone.
///
/// Without this, deleting `CBOR_CODECS` would make every test above pass. req/08 N-1's disease --
/// the count holds while the question changes -- applied to a static check.
#[test]
fn ac_014_the_scan_finds_the_permitted_uses() {
    let root = workspace_root();
    let canon_src = root.join("crates").join("gx-canon").join("src");
    let mut codec_hits = 0usize;

    for path in sources(&canon_src) {
        for (_, line) in code_lines(&path) {
            for codec in CBOR_CODECS {
                if line.contains(&codec.replace('-', "_")) {
                    codec_hits += 1;
                }
            }
        }
    }

    assert!(
        codec_hits > 0,
        "gx-canon names no CBOR codec, so the ban above is checking for something that does not \
         exist anywhere and would pass on an empty repository"
    );
    println!("AC014_PERMITTED_CODEC_LINES_IN_GX_CANON={codec_hits}");
}
