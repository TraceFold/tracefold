// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R27 item 3 (`req/331` §0-3, from `req/329` M-03, `req/38` §233 ruling 4)** — the reach
//! census stops being an enumeration.
//!
//! # The family, three generations of it
//!
//! R22 counted two spellings and a third walked past. R24 repaired that and counted two strings.
//! R25 repaired that and counted the private field, arguing the language closed the siblings. R26
//! repaired that and counted five roads — and wrote on `docs/LIMITS.md` that the census *"counts
//! every road to the question, the field and the four accessors alike"*. The twenty-sixth audit
//! derived the roads from the source rather than from the sentence: there are **seven**, and the
//! three uncounted ones include `entry_fault`, which `catalogue.rs`'s own doc calls **"the
//! question"**. A gate written `catalogue.entry_fault(tool).is_err()` in a sibling file moved the
//! census `0 → 0`.
//!
//! Every repair in that chain was a longer list, and a longer list fails the same way. This file
//! holds the shipped censuses to a road set **derived from `catalogue.rs` on every run**, so the
//! eighth accessor is a road the day it is written.

#[path = "support/census_roads.rs"]
mod census_roads;

use std::path::{Path, PathBuf};

fn record(line: &str) {
    println!("{line}");
    if let Ok(path) = std::env::var("R27_MEASUREMENTS") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// The two files that carry a census of this question on every clone.
const SHIPPED_CENSUSES: [&str; 2] = ["r25_declaration_axes.rs", "r26_reach_census.rs"];

/// 🔴 **Bed control** — the derivation finds the accessors, so a zero below is not an empty scan.
///
/// The number is not frozen; that is the point of the file. What is asserted is that the derivation
/// reaches the map at all and finds the accessor the audit named, because an arm that measured an
/// empty file would give the right answer for the wrong reason.
#[test]
fn a_bed_control_the_derivation_finds_the_reader_accessors() {
    let src = census_roads::catalogue_source();
    let accessors = census_roads::reader_accessors(&src);
    let roads = census_roads::roads_to_the_question(&src);
    record(&format!(
        "R27_CENSUS_DERIVED accessors={accessors:?} roads={}",
        roads.len()
    ));
    assert!(
        accessors.len() >= 5,
        "🔴 the derivation found {} reader accessors, which is fewer than the five the previous \
         release already knew about: it is measuring the wrong thing: {accessors:?}",
        accessors.len()
    );
    assert!(
        accessors.iter().any(|a| a == "entry_fault"),
        "🔴 `req/329` M-03 named `entry_fault` — the accessor this file's own doc calls **the** \
         question — as the uncounted road that matters: {accessors:?}"
    );
    assert_eq!(
        roads.len(),
        accessors.len() + 1,
        "one road per accessor, plus the private field itself: {roads:?}"
    );
}

/// 🔴 **`req/329` M-03** — a sibling gate written on **any** derived road moves the census.
///
/// Fired rather than described, at every road the derivation returns. R26's own arm fired two
/// spellings and showed they moved; the audit fired a third and showed it did not. This fires all
/// of them, so the assertion is about the census rather than about the spellings a lane chose.
#[test]
fn b_a_gate_written_on_any_derived_road_moves_the_census() {
    let src = census_roads::catalogue_source();
    let roads = census_roads::roads_to_the_question(&src);
    let mut immovable: Vec<String> = Vec::new();
    for accessor in census_roads::reader_accessors(&src) {
        let gate = format!(
            "fn a_third_gate(c: &Catalogue, t: &str) -> bool {{ c.{accessor}(t).is_some() }}"
        );
        if census_roads::reaches_the_question(&gate, &roads) == 0 {
            immovable.push(accessor);
        }
    }
    record(&format!("R27_CENSUS_IMMOVABLE {immovable:?}"));
    assert!(
        immovable.is_empty(),
        "🔴 `req/329` M-03: a gate in a sibling file written on {immovable:?} asks the same \
         question and moves the census 0 -> 0. That is the defect family R24, R25 and R26 were each \
         a repair of, one generation on."
    );
}

/// 🔴 The mutation the audit fired, kept as its own arm so the repair cannot regress quietly.
#[test]
fn c_the_gate_written_on_the_files_own_the_question_moves_the_census() {
    let src = census_roads::catalogue_source();
    let roads = census_roads::roads_to_the_question(&src);
    let counted = "fn a_third_gate(c: &Catalogue, t: &str) -> bool { c.restore_for(t).is_some() }";
    let audited = "fn a_third_gate(c: &Catalogue, t: &str) -> bool { c.entry_fault(t).is_err() }";
    let (a, b) = (
        census_roads::reaches_the_question(counted, &roads),
        census_roads::reaches_the_question(audited, &roads),
    );
    record(&format!(
        "R27_CENSUS_MUTATION counted_spelling={a} entry_fault_spelling={b}"
    ));
    assert!(a > 0, "the control spelling has to move it: {a}");
    assert!(
        b > 0,
        "🔴 `req/329` M-03: `catalogue.entry_fault(tool).is_err()` is the third gate written on the \
         accessor `catalogue.rs` names **the** question, and the census scores it {b}."
    );
}

/// 🔴 **The repair, held structurally** — neither shipped census carries a hand-written road list.
///
/// This is the arm that stops the next generation of the family. Every previous repair replaced a
/// short list with a longer one, and each longer list was true on the day it was written. A census
/// that names its roads in an array literal will be wrong again the next time an accessor is added;
/// one that computes them cannot be.
#[test]
fn d_no_shipped_census_enumerates_the_roads_by_hand() {
    let mut enumerating: Vec<String> = Vec::new();
    let mut using_the_derivation: Vec<String> = Vec::new();
    for file in SHIPPED_CENSUSES {
        let text = std::fs::read_to_string(tests_dir().join(file))
            .unwrap_or_else(|_| panic!("{file} is readable"));
        let code = census_roads::code_of(&text);
        // A hand-written list is one that names an accessor in a string literal beside the field.
        let names_an_accessor_literally = [
            "\"spec_for(\"",
            "\"restore_for(\"",
            "\"declared_reversibility(\"",
            "\"writes_per_this_file(\"",
        ]
        .iter()
        .filter(|needle| code.contains(**needle))
        .count();
        if names_an_accessor_literally > 0 {
            enumerating.push(format!(
                "{file} ({names_an_accessor_literally} literal roads)"
            ));
        }
        if code.contains("census_roads::") {
            using_the_derivation.push(file.to_string());
        }
    }
    record(&format!(
        "R27_CENSUS_SHIPPED enumerating={enumerating:?} deriving={using_the_derivation:?}"
    ));
    assert!(
        enumerating.is_empty(),
        "🔴 `req/329` M-03: {enumerating:?} still write the roads out by hand. Every release in \
         this family replaced a short list with a longer one and each was true the day it was \
         written; the list is what has to go, not its length."
    );
    assert_eq!(
        using_the_derivation.len(),
        SHIPPED_CENSUSES.len(),
        "🔴 and both censuses have to reach the derivation, or one of them is holding a question \
         narrower than the one it is named for: {using_the_derivation:?}"
    );
}

/// 🔴 The derivation is a function of the source, so it answers differently for a different source.
///
/// Without this, `reader_accessors` could return a constant and every arm above would pass. The
/// mutation adds an eighth accessor to a **copy** of the text and requires it to appear — which is
/// the property the whole repair rests on: the day someone writes one, it is a road.
#[test]
fn e_an_eighth_accessor_added_to_the_source_becomes_a_road_without_anyone_saying_so() {
    let src = census_roads::catalogue_source();
    let before = census_roads::reader_accessors(&src);
    let eighth = "
impl Catalogue {
    pub fn a_ninth_question(&self, tool: &str) -> bool {
        self.restores.contains_key(tool)
    }
}
";
    let after = census_roads::reader_accessors(&format!("{src}{eighth}"));
    record(&format!(
        "R27_CENSUS_GROWTH before={} after={} added={:?}",
        before.len(),
        after.len(),
        after
            .iter()
            .filter(|a| !before.contains(a))
            .collect::<Vec<_>>()
    ));
    assert_eq!(
        after.len(),
        before.len() + 1,
        "🔴 an accessor added to the source is not a road the derivation returns, so the census is \
         still a list wearing a function's clothes: {before:?} -> {after:?}"
    );
    assert!(
        after.iter().any(|a| a == "a_ninth_question"),
        "and it is the one that was added: {after:?}"
    );
}

/// 🔴 A **builder** is not a road, and the derivation says so.
///
/// `req/329` §9-3 is the reason this arm exists: the audit's own first version counted
/// `with_restore` / `with_prior_read` / `with_restore_template` and reported six roads, then cut it
/// to three because a gate cannot be written on a constructor. Inflating a census is the shape of
/// overclaim this family of repairs must not adopt while fixing the opposite error.
#[test]
fn f_builders_are_not_roads() {
    let src = census_roads::catalogue_source();
    let accessors = census_roads::reader_accessors(&src);
    let builders: Vec<&String> = accessors
        .iter()
        .filter(|a| a.starts_with("with_"))
        .collect();
    record(&format!(
        "R27_CENSUS_BUILDERS counted_as_roads={builders:?}"
    ));
    assert!(
        builders.is_empty(),
        "🔴 a builder constructs a catalogue rather than answering a question about a tool, so a \
         gate cannot be written on one and counting it inflates the census: {builders:?}"
    );
}
