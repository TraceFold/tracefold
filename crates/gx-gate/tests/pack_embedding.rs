//! The shipped pack reaches a build by one road, and the road ends at the file (FR-028).
//!
//! req/60 §5.2's hand 5 DoD: 「pack の file が build に埋め込まれる経路が 1 本(=第 2 の経路を作らない・
//! **AC-014 と同型の機械検査**)」. AC-014's shape is a source scan asserting that one line does a thing
//! nothing else may do -- there, take a hash; here, embed the pack -- and req/60 §7.2 says what such a
//! claim costs: 「『無い事』の検査には変異が要る … 経路を 1 本足したら RED になる事を変異で実測しないと
//! 空虚になる」. `tools/verify_m3h5.sh` §8 adds a second road and shows this suite go red.
//!
//! Four claims live here, and each one fails differently:
//!
//! | claim | what its failure would mean |
//! |---|---|
//! | one road **per pack** (M7 hand 2: there are two packs now) | two embeddings of one pack, free to diverge the day one of them is edited |
//! | the road ends at the file | the build ships bytes nobody can read in `policies/` |
//! | every shipped `.cedar` is embedded | a policy file in the repository that no build ever loads -- a rule that looks in force and is not |
//! | every statement carries `@id` | positional ids reaching a receipt's CID (ASM-62-1) |
//!
//! Two smaller ones follow them: the module states the range a pack may reason over (M3-10 requires
//! that range be written down, so this checks it is), and no ready-made [`gx_gate::InvariantCheck`]
//! ships (**D-9**, as a source scan -- the same instrument hand 3 used to report 「出荷 invariant 0」).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use gx_gate::packs::{FS_PACK_PATH, GIT_PACK_PATH, SHIPPED_PACKS};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-gate")
        .to_path_buf()
}

/// Every `.rs` file under the directories this workspace keeps source in, as (repo-relative path,
/// text). `target/` is skipped: build output is not source, and a generated file that happened to
/// contain the string would make this scan report on cargo rather than on the tree.
fn rust_sources() -> Vec<(String, String)> {
    let root = repo_root();
    let mut found = Vec::new();
    for top in ["crates", "probes", "fuzz"] {
        collect(&root.join(top), top, &mut found);
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = format!("{prefix}/{name}");
        let path = entry.path();
        if path.is_dir() {
            if name != "target" {
                collect(&path, &rel, out);
            }
        } else if name.ends_with(".rs") {
            if let Ok(text) = fs::read_to_string(&path) {
                out.push((rel, text));
            }
        }
    }
}

/// Every `include_str!`/`include_bytes!` in the workspace's sources, as (file, embedded path).
///
/// Comment lines are skipped, the way `verify_m3h3.sh` counts `order` on code lines only: a module
/// doc that names the macro is describing the road, not being one. That is a real limit and it is
/// written down rather than left for a reader to discover -- a second road hidden inside a macro
/// expansion or built by `concat!` would not be seen here either, which is why the mutation section
/// adds its road the way a person would.
fn embedded_paths() -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (file, text) in rust_sources() {
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for macro_name in ["include_str!(", "include_bytes!("] {
                let mut rest = line;
                while let Some(at) = rest.find(macro_name) {
                    rest = &rest[at + macro_name.len()..];
                    let Some(open) = rest.find('"') else { continue };
                    let after = &rest[open + 1..];
                    let Some(close) = after.find('"') else {
                        continue;
                    };
                    found.push((file.clone(), after[..close].to_string()));
                    rest = &after[close + 1..];
                }
            }
        }
    }
    found
}

/// Every `.cedar` file the repository ships, as repo-relative paths.
fn shipped_cedar_files() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut walk = vec![(repo_root().join("policies"), "policies".to_string())];
    while let Some((dir, prefix)) = walk.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = format!("{prefix}/{name}");
            if entry.path().is_dir() {
                walk.push((entry.path(), rel));
            } else if name.ends_with(".cedar") {
                found.insert(rel);
            }
        }
    }
    found
}

fn pack_text(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// The road
// ---------------------------------------------------------------------------

/// Exactly one embedding **per shipped pack**, and every one of them is in `packs.rs`.
///
/// 🔴 The claim moved with M7 hand 2 and did not weaken. Until the git pack there was one pack, so
/// 「one road」 and 「one road under `policies/`」 were the same sentence; with two packs the second
/// reading would forbid the second pack rather than forbid a second copy of one. What FR-028 is
/// against is a pack embedded twice -- two constants free to diverge the day one of them is edited --
/// so the count is taken per path, and the total is still asserted against the declared set so that
/// an embedding of a file **no** pack declares is caught as well.
#[test]
fn each_pack_reaches_a_build_by_exactly_one_road() {
    let all = embedded_paths();
    let pack_roads: Vec<&(String, String)> = all
        .iter()
        .filter(|(_, embedded)| embedded.contains("policies/"))
        .collect();
    println!(
        "pack_embedding: {} embedded file(s) in the workspace, {} of them under policies/, {} packs declared",
        all.len(),
        pack_roads.len(),
        SHIPPED_PACKS.len()
    );
    assert_eq!(
        pack_roads.len(),
        SHIPPED_PACKS.len(),
        "FR-028 asks for one road per pack; found {pack_roads:?}"
    );
    for (file, _) in &pack_roads {
        assert_eq!(
            file, "crates/gx-gate/src/packs.rs",
            "the roads belong in the module 41 §2 names for packs"
        );
    }
    for pack in SHIPPED_PACKS {
        let roads = pack_roads
            .iter()
            .filter(|(_, embedded)| embedded.ends_with(pack.path))
            .count();
        assert_eq!(roads, 1, "{}: exactly one road embeds it", pack.path);
    }
}

/// The bytes at the far end of each road are the file on disk.
///
/// A road that ended at a copy would let the repository and the build disagree about what is in
/// force, and every suite that reads a pack file at run time (`ac_025`, `policy_mapping`,
/// `policy_determinism`) would go on measuring the copy that is not shipped.
#[test]
fn the_embedded_bytes_are_the_files_the_criteria_name() {
    for pack in SHIPPED_PACKS {
        assert!(
            !pack.source.is_empty(),
            "{}: an empty pack admits nothing and denies nothing by name",
            pack.path
        );
        assert_eq!(
            pack.source,
            pack_text(pack.path),
            "the embedded pack and {} must be the same bytes",
            pack.path
        );
    }
}

/// Every `.cedar` file the repository ships is the one that is embedded.
///
/// The fail-open this closes: a second pack file added to `policies/` and embedded by nothing looks,
/// to anybody reading the repository, like a rule in force. FR-028 puts the git and MCP packs in M7,
/// so when they arrive this test is the one that says the road has to arrive with them.
#[test]
fn nothing_ships_in_policies_that_no_build_loads() {
    let shipped = shipped_cedar_files();
    let embedded: BTreeSet<String> = embedded_paths()
        .into_iter()
        .filter(|(_, path)| path.contains("policies/"))
        .map(|(_, path)| {
            let at = path.find("policies/").expect("filtered on it");
            path[at..].to_string()
        })
        .collect();
    println!("pack_embedding: shipped={shipped:?} embedded={embedded:?}");
    assert_eq!(
        shipped, embedded,
        "every .cedar under policies/ must be embedded, and nothing else may be"
    );
    assert!(
        shipped.contains(FS_PACK_PATH),
        "AC-025 names {FS_PACK_PATH}"
    );
    assert!(
        shipped.contains(GIT_PACK_PATH),
        "AC-074 asks for a pack under policies/git/, and this build embeds {GIT_PACK_PATH}"
    );
    let declared: BTreeSet<String> = SHIPPED_PACKS
        .iter()
        .map(|pack| pack.path.to_string())
        .collect();
    assert_eq!(
        shipped, declared,
        "SHIPPED_PACKS is the declared set and the tree is the actual one; a pack in one and not \
         the other is a pack no test walks"
    );
}

// ---------------------------------------------------------------------------
// The pack's own format requirements
// ---------------------------------------------------------------------------

/// Every statement in the pack carries `@id`, and the ids are the declared ones (**ASM-62-1**, C-4).
///
/// Counted off the file rather than off the parsed set, because the parsed set cannot tell 「every
/// statement was annotated」 from 「the statements that were annotated parsed」 -- `PolicyEngine::parse`
/// refuses the whole set on a missing annotation, so by the time an engine exists the question is
/// already answered. The count is line-shaped (`@id(` and `permit (` / `forbid (` at the start of a
/// line), which is how this pack is written and is a requirement on the pack rather than on Cedar:
/// a statement folded onto one line would evade the count, and the parse-time refusal is what
/// actually holds the property.
#[test]
fn every_statement_in_every_pack_is_named_by_an_annotation() {
    for pack in SHIPPED_PACKS {
        let text = pack_text(pack.path);
        let statements = text
            .lines()
            .filter(|l| l.starts_with("permit (") || l.starts_with("forbid ("))
            .count();
        let annotations = text.lines().filter(|l| l.starts_with("@id(")).count();
        println!(
            "pack_embedding: {} holds {statements} statement(s), {annotations} @id annotation(s)",
            pack.path
        );
        assert_eq!(
            statements,
            pack.policy_ids.len(),
            "{}: the pack holds the statements packs.rs declares",
            pack.path
        );
        assert_eq!(
            annotations, statements,
            "{}: every statement carries @id -- a policy without one is refused at load with \
             Error::PolicySetUnreadable, and that refusal is the pack format requirement C-4 asks \
             to be written down",
            pack.path
        );
        for id in pack.policy_ids {
            assert!(
                text.contains(&format!("@id(\"{id}\")")),
                "{id} is declared in packs.rs and must be the annotation in {}",
                pack.path
            );
        }
    }
}

/// 🔴 The substrate a pack **declares** is the one every statement in it **scopes on** (M7 hand 4).
///
/// [`gx_gate::packs::ShippedPack::substrate`] is a declaration, and two things now read it: the
/// locality obligation of `shipped_set.rs` (「each pack decides only its own substrate」, which is what
/// makes composing the packs safe) and the vector-expiry gate of `false_admit.rs` (**H-9**). A
/// declaration those two trust and nobody compares with the artifact is the failure this whole module
/// is built against — so the clauses are read out of the pack's own text.
///
/// The scan is line-shaped, like the `@id` count above and with the same limit: a statement folded
/// onto one line, or a scope written through some other spelling, would evade it. What actually holds
/// the property is that a statement without the clause **admits another substrate**, which
/// `shipped_set.rs` measures by running requests; this is the cheaper gate that fires first, at the
/// place a pack author is looking.
#[test]
fn the_declared_substrate_is_the_one_every_statement_scopes_on() {
    for pack in SHIPPED_PACKS {
        let text = pack_text(pack.path);
        let scopes: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with("//"))
            .filter_map(|l| l.split_once("resource.substrate == \""))
            .filter_map(|(_, rest)| rest.split_once('"').map(|(tag, _)| tag.to_string()))
            .collect();
        println!(
            "PACK_SUBSTRATE_SCOPES {} declared={:?} scopes={scopes:?} statements={}",
            pack.path,
            pack.substrate,
            pack.policy_ids.len()
        );
        assert_eq!(
            scopes.len(),
            pack.policy_ids.len(),
            "{}: every statement scopes on a substrate, and a statement that does not is one that \
             decides another pack's requests",
            pack.path
        );
        for tag in &scopes {
            assert_eq!(
                tag, pack.substrate,
                "{}: declares the substrate {:?} and a statement scopes on {tag:?}",
                pack.path, pack.substrate
            );
        }
    }
}

/// The module says what a pack may reason over, and the three things it may not (**M3-10**).
///
/// M3-10's ruling is 「v0.1 pack の実効範囲=locator/actor/context/order 級と明記(overclaim 禁)」 -- an
/// obligation to write something down. A documentation obligation nobody checks is a promise, so the
/// clauses the rulings name by name are asserted to be present: the range itself, C-8's 「読み取りは
/// gate を通らない」, C-1's 「actor は key でのみ識別できる」, the `@id` refusal C-4 asks for, and P-6,
/// which is why the payload is absent from the range.
#[test]
fn the_pack_module_states_its_effective_range() {
    let text = fs::read_to_string(repo_root().join("crates/gx-gate/src/packs.rs"))
        .expect("the module this test is about");
    for clause in [
        "locator/actor/context/order 級",
        "読み取りは gate を通らない",
        "actor は key でのみ識別できる",
        "PolicySetUnreadable",
        "P-6",
        "D-9",
    ] {
        assert!(
            text.contains(clause),
            "packs.rs must state {clause:?} -- M3-10 makes the range a written requirement"
        );
    }
}

/// No ready-made invariant ships (**D-9**), measured on the source.
///
/// The decision and its three reasons are in `packs.rs` and `req/65` §3. What is measurable is the
/// absence: this crate contains no `impl InvariantCheck for`, so a deployment's registry holds only
/// what the deployment put in it. The day one ships, this test is the one that has to be rewritten
/// on purpose rather than the count quietly moving.
#[test]
fn the_shipped_artifact_carries_no_invariant() {
    let implementations: Vec<String> = rust_sources()
        .into_iter()
        .filter(|(file, _)| file.starts_with("crates/gx-gate/src/"))
        .flat_map(|(file, text)| {
            text.lines()
                .filter(|line| {
                    let t = line.trim_start();
                    !t.starts_with("//") && t.contains("impl InvariantCheck for")
                })
                .map(|line| format!("{file}: {}", line.trim()))
                .collect::<Vec<String>>()
        })
        .collect();
    println!(
        "pack_embedding: {} shipped InvariantCheck implementation(s) (D-9: none)",
        implementations.len()
    );
    assert!(
        implementations.is_empty(),
        "D-9 decided no ready-made invariant ships in M3: {implementations:?}"
    );
}
