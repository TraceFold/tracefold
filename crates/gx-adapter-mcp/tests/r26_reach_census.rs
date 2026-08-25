// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/324` M-05 (`req/38` §231 ruling 2)** — private is the field, not the question.
//!
//! # What broke
//!
//! R22 counted two **strings** (`restores.contains_key` and `.restored_by() == `) and a third gate
//! spelled a third way walked past. R25 replaced that with a count of reaches to the **private
//! field** `self.restores`, and argued the sibling files were closed by the language: *"`restores`
//! is a private field, so Rust already refuses a third gate in `invert.rs`"*.
//!
//! The twenty-fifth audit fired the obvious mutation at both halves:
//!
//! ```text
//! pub fn a_third_gate(&self, tool: &str) -> bool { self.spec_for(tool).is_some() }
//! ```
//!
//! `spec_for`, `restore_for`, `declared_reversibility` and `writes_per_this_file` are **`pub`**.
//! The field census moved `11 → 11` and the sibling census moved `0 → 0`. The mutation is not an
//! exotic spelling — it is the shape a lane reaches for when the accessor is already there — and
//! it asks the **key half only**, which is the defect family R22, R23 and R24 were each a repair
//! of.
//!
//! # What this file requires
//!
//! The census counts the **question**, not one road to it: the private field *and* every `pub`
//! accessor that answers the same question. A gate written on any of those roads moves the number,
//! in `catalogue.rs` and in every sibling file of the crate.
//!
//! The sibling half stops being an emptiness assertion and becomes a **declared inventory**: the
//! accessors have legitimate callers (`invert.rs` has one today), so what closes the hole is that
//! the count is declared and moves, not that it is zero.

#[path = "support/census_roads.rs"]
mod census_roads;

use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn adapter_src(file: &str) -> String {
    std::fs::read_to_string(src_dir().join(file)).expect("the adapter's src is readable")
}

/// Source with its comment lines dropped: the record of *why* a spelling was removed is not the
/// spelling.
fn code_of(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 🔴 Every road to the question *does this file say this tool writes, and what undoes it*.
///
/// The private field is one road. The four `pub` accessors are four more, and they are the roads
/// `req/324` M-05 walked. A census that counts one of five answers a narrower question than the
/// one it is named for.
/// 🔴 **Derived rather than enumerated by `req/329` M-03 (`req/38` §233 ruling 4)** — the list
/// this file used to carry was five roads and `catalogue.rs` had **seven**. The three it missed
/// were `declared`, `soundness` and `entry_fault`, and the last of those is the accessor this
/// file's own subject calls *"the question"*. R22, R24, R25 and R26 each replaced a short list
/// with a longer one and each longer list was true the day it was written. See
/// `tests/support/census_roads.rs` for what counts as a road and why builders do not.
fn roads() -> Vec<String> {
    census_roads::roads_to_the_question(&census_roads::catalogue_source())
}

/// The census, as a pure function of source text, so it can be fired at text that is not the
/// shipped file.
fn reaches_the_question(code: &str) -> usize {
    census_roads::reaches_the_question(code, &roads())
}

/// A third gate asking *does this file say this tool writes*, written on this file's own `pub`
/// accessors instead of on the private field — `req/324` M-05's mutation, verbatim.
const THIRD_GATE_ON_THE_ACCESSORS: &str = r"
    pub fn a_third_gate(&self, tool: &str) -> bool {
        self.spec_for(tool).is_some()
    }
";

/// The same question, from a sibling file, over the `pub` accessors — the second mutation.
const THIRD_GATE_IN_A_SIBLING_FILE: &str = r"
fn a_third_gate(catalogue: &Catalogue, tool: &str) -> bool {
    catalogue.restore_for(tool).is_some() || catalogue.declared_reversibility(tool).is_true()
}
";

/// 🔴 The bed control: the four accessors the mutations stand on are really `pub` on the shipped
/// type, so the mutations are mutations of *this* build and not a fantasy.
#[test]
fn the_public_accessors_the_mutations_stand_on_exist() {
    let src = adapter_src("catalogue.rs");
    let mut public: Vec<&str> = Vec::new();
    for accessor in [
        "pub fn spec_for",
        "pub fn restore_for",
        "pub fn declared_reversibility",
        "pub fn writes_per_this_file",
    ] {
        if src.contains(accessor) {
            public.push(accessor);
        }
    }
    println!("R26_CENSUS public_accessors={public:?}");
    assert_eq!(
        public.len(),
        4,
        "🔴 `req/324` M-05's premise is that the question has `pub` roads. If an accessor was made \
         private the finding changes shape and this file has to be re-derived rather than \
         re-run: {public:?}"
    );
}

/// 🔴 **`req/324` M-05** — a third gate on the accessors moves the census.
#[test]
fn the_census_moves_for_a_third_gate_written_on_this_files_public_accessors() {
    let src = adapter_src("catalogue.rs");
    let code = code_of(&src);
    let baseline = reaches_the_question(&code);
    assert!(
        baseline >= 10 && src.contains("writes_per_this_file") && src.contains("entry_fault"),
        "an arm that measures an empty file gives the right answer for the wrong reason: \
         baseline={baseline}"
    );
    let mutated = code_of(&format!("{src}\n{THIRD_GATE_ON_THE_ACCESSORS}"));
    let after = reaches_the_question(&mutated);
    println!(
        "R26_CENSUS baseline={baseline} after_third_gate_on_accessors={after} moved={}",
        after > baseline
    );
    assert!(
        after > baseline,
        "🔴 `req/324` M-05 (`req/38` §231 ruling 2): a third gate asking the identical question — \
         *does this file say this tool writes* — written `self.spec_for(tool).is_some()` reaches \
         the map through this file's own `pub` accessor and is counted {after} against a baseline \
         of {baseline}. Private is the **field**; the question has four more roads and they are \
         all `pub`."
    );
}

/// 🔴 **`req/324` M-05, the sibling half** — the same mutation in a sibling file moves the sibling
/// census.
///
/// R25's arm asserted the sibling count was **zero** and argued from privacy. The accessors have
/// legitimate callers, so what closes the hole is a declared inventory that moves — not an
/// emptiness that was only ever true of one spelling.
#[test]
fn the_sibling_census_moves_for_a_third_gate_written_on_a_public_accessor() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(src_dir())
        .expect("the adapter's src is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    let mut scanned = 0usize;
    let mut inventory: Vec<(String, usize)> = Vec::new();
    let mut invert_code = String::new();
    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        scanned += 1;
        if name == "catalogue.rs" {
            continue;
        }
        let code = code_of(&std::fs::read_to_string(&path).expect("readable"));
        let hits = reaches_the_question(&code);
        if name == "invert.rs" {
            invert_code = code.clone();
        }
        if hits > 0 {
            inventory.push((name, hits));
        }
    }
    println!("R26_SIBLING scanned={scanned} inventory={inventory:?}");
    assert!(
        scanned >= 8,
        "the scan walked {scanned} files, which is fewer than this crate has: a scan that found \
         nothing would satisfy every assertion below"
    );
    assert!(
        !invert_code.is_empty(),
        "the mutation is fired at `invert.rs`, which the scan must have read"
    );
    let before = reaches_the_question(&invert_code);
    let after = reaches_the_question(&format!("{invert_code}\n{THIRD_GATE_IN_A_SIBLING_FILE}"));
    println!(
        "R26_SIBLING invert_before={before} invert_after={after} moved={}",
        after > before
    );
    assert!(
        after > before,
        "🔴 `req/324` M-05: R25's sibling arm counted `.restores` and argued that the field's \
         privacy closed the crate. A gate in `invert.rs` written \
         `catalogue.restore_for(tool).is_some()` compiles, asks the same question, and moved that \
         count {before} → {before}. Counting the question rather than the field is what makes the \
         sibling scan measure anything: {before} → {after}"
    );
}

/// 🔴 The shipped census — the one that runs on every clone — counts the question and not one road
/// to it.
///
/// Reading a sibling **test** file is the move `r22_refusal_constant_census.rs` and
/// `r25_declaration_axes.rs` already make: an arm here that measured only its own predicate would
/// leave the shipped one free to drift back.
#[test]
fn the_shipped_census_counts_every_road_to_the_question() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("r25_declaration_axes.rs");
    let code = code_of(&std::fs::read_to_string(&path).expect("the shipped census is readable"));
    let mut missing: Vec<String> = Vec::new();
    // 🔴 `req/329` M-03: the roads are derived, so this arm asks the shipped census to reach the
    // derivation rather than to repeat a list. A census that names its roads in a literal is wrong
    // again the next time an accessor is added; one that computes them cannot be.
    if !code.contains("census_roads::") {
        missing.push("the derived road set (tests/support/census_roads.rs)".to_string());
    }
    for road in roads() {
        if road != "restores" && code.contains(&format!("\"{road}\"")) {
            missing.push(format!("{road} is written out as a literal"));
        }
    }
    println!("R26_SHIPPED_CENSUS missing_roads={missing:?}");
    assert!(
        missing.is_empty(),
        "🔴 `req/324` M-05: the census that runs on every clone has to reach the map by every road \
         the question has, because a lane adds a gate on whichever road is already `pub`. Missing \
         from the shipped predicate: {missing:?}"
    );
}
