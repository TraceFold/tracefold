// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/320` M-01, M-03, M-04 and L-01** (`req/321` §1 items 2, 4 and 5; `req/38` §229
//! ruling 2) — the declaration's five positions, the axis the gate declares, and the census that
//! has to catch a spelling nobody has thought of yet.
//!
//! # What the twenty-fourth adversarial audit measured
//!
//! ```text
//! A24_CENSUS mutation="third gate, equivalent spelling" counts=(2,1,3) census_is_red=false
//! A24_WS_PREFIX accepted=true governs_a_matching_locator=false
//! A24_WS_ACCEPTED=["cas prefix / U+0020", …, "restored_by / U+FEFF", "read_by.by_tool / U+FEFF"]
//! A24_ABSENCE consumer_files=3 read_subject_sites=4 asks=1
//! ```
//!
//! * **M-03** — R24's structural gate counts two **strings** (`restores.contains_key` and
//!   `.restored_by() == `). A third gate written `restores.get(t).is_some()` /
//!   `tool == s.restored_by()` asks the identical question and was counted zero times, so
//!   `docs/LIMITS.md`'s *"a `catalogue.rs` in which that question is spelled anywhere else fails the
//!   build"* was false as published.
//! * **M-04** — the whitespace gate walks the four positions a **tool name** is spelled; its sibling
//!   decomposition gate walks **five**. The fifth is the `$cas_read` prefix, and it is the quietest
//!   of the five: a prefix with an invisible edge governs nothing and fails by nothing happening.
//! * **L-01** — the gate's doc declares the axis *an edge a reader cannot see* and the implementation
//!   is `char::is_whitespace`, so U+200B and U+FEFF were accepted in all five positions.
//! * **M-01** — the predicate that separates *the server said nothing is here* from *the server
//!   would not answer* is asked at one of the three sites that consume a declared read.
//!
//! # Red-first
//!
//! No symbol this lane created is named: refusal wording is spelled as needles and the structural
//! arms read `src/*.rs` as text, so this file compiles at `d21821e` and fails on its assertions.

#[path = "support/census_roads.rs"]
mod census_roads;

use std::path::Path;

use gx_adapter_mcp::Catalogue;

const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";

/// R17's wording rule: every refusal in this family carries a remedy.
const REMEDY: &str = "What to fix:";

fn adapter_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

/// Source with its comment lines dropped — the record of *why* a spelling was removed must not be
/// counted as the spelling (`req/316` §5 self-admission 1).
fn code_of(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// M-03 — a census that survives a spelling nobody has thought of
// ---------------------------------------------------------------------------

/// 🔴 **`req/320` M-03** — the census predicate, as a pure function of source text.
///
/// This is the shape R24's arm has after the repair, re-implemented here so it can be **fired at
/// text that is not the shipped file**. R24's arm can only ever say "the file as it stands is
/// fine"; the audit's finding was about a file that does not stand yet, and the only way to hold
/// that is to mutate the text and require the answer to move. The duplication is deliberate and is
/// declared: `r24_predicate_unification.rs` holds the shipped file, and this holds the *discriminat-
/// ing power* of the thing holding it.
/// 🔴 **Widened by `req/324` M-05 (`req/38` §231 ruling 2)**: the private field is **one** road to
/// the question and the four `pub` accessors are four more.
///
/// R25's version of this counted `self.restores` alone and argued the rest was closed by the
/// language — *the map is private, so Rust already refuses a third gate*. The field is private; the
/// question is not. `self.spec_for(tool).is_some()` is a third gate on this file's own accessor, it
/// asks the key half only (the R22/R23/R24 defect family verbatim), and it moved this count `11 →
/// 11`. Counting every road is what makes the number mean *a gate was added* rather than *a gate
/// was added in one of the spellings I listed*.
///
/// `crates/gx-adapter-mcp/tests/r26_reach_census.rs` fires the two audit mutations at this shape
/// and holds the shipped predicate to naming every road.
/// 🔴 **Derived rather than enumerated by `req/329` M-03 (`req/38` §233 ruling 4)**: the list
/// above was five roads and the file had **seven**, and the two it missed included `entry_fault` —
/// the accessor `catalogue.rs`'s own doc calls *"the question"*. Every release in this family
/// replaced a short list with a longer one; the list is what had to go, not its length. See
/// `tests/support/census_roads.rs`.
fn asks_about_the_map(code: &str) -> usize {
    let roads = census_roads::roads_to_the_question(&census_roads::catalogue_source());
    census_roads::reaches_the_question(code, &roads)
}

/// A third gate, in the spelling R24's arm knew.
const THIRD_GATE_OLD_SPELLING: &str = r#"
    pub fn a_third_gate(&self, tool: &str) -> bool {
        self.restores.contains_key(tool) || self.restores.iter().any(|(_, s)| s.restored_by() == tool)
    }
"#;

/// 🔴 The same question, in a spelling R24's arm did not know: `get(..).is_some()` for the key half
/// and the comparison's operands the other way round for the value half.
const THIRD_GATE_EQUIVALENT_SPELLING: &str = r#"
    pub fn a_third_gate(&self, tool: &str) -> bool {
        self.restores.get(tool).is_some() || self.restores.values().any(|s| tool == s.restored_by())
    }
"#;

#[test]
fn the_census_catches_a_third_gate_in_a_spelling_it_was_not_told_about() {
    let src = adapter_src("catalogue.rs");
    let code = code_of(&src);
    let baseline = asks_about_the_map(&code);
    println!("R25_CENSUS baseline_field_reads={baseline}");
    assert!(
        baseline > 0,
        "the scan is looking at the file it thinks it is: an arm that measures an empty file gives \
         the right answer for the wrong reason (`req/316` §5 self-admission 2)"
    );
    for (what, mutation) in [
        ("third gate, old spelling", THIRD_GATE_OLD_SPELLING),
        (
            "third gate, equivalent spelling",
            THIRD_GATE_EQUIVALENT_SPELLING,
        ),
    ] {
        let mutated = format!("{code}\n{mutation}");
        let after = asks_about_the_map(&mutated);
        println!(
            "R25_CENSUS mutation={what:?} field_reads={after} moved={}",
            after > baseline
        );
        assert!(
            after > baseline,
            "🔴 `req/320` M-03 (`req/38` §229 ruling 2): a gate asking *does this file say this \
             tool writes* has to reach the `restores` map in **some** spelling, and the census has \
             to be a function of that rather than of the two strings R24 happened to know. This \
             mutation ({what}) left R24's counts at `(2,1,3)` and its arm green: {after} vs \
             {baseline}"
        );
    }
}

/// 🔴 **`req/320` M-03, second half** — and the sibling files, which the audit read rather than
/// measured.
///
/// `restores` is a private field of a type declared in `catalogue.rs`, so Rust already refuses a
/// third gate in `invert.rs` / `cas.rs` / `adapter.rs`. That is a language fact and this is the arm
/// that makes it a **measured** one: the audit's §9 self-admission 9 says the sibling-file half of
/// M-03 was read from the source and never fired.
#[test]
fn no_other_file_in_this_crate_reaches_the_restores_map() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut reached: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the adapter's src is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
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
        let hits = code.matches(".restores").count();
        if hits > 0 {
            reached.push(format!("{name}: {hits}"));
        }
    }
    println!("R25_SIBLING_FILES scanned={scanned} reached={reached:?}");
    assert!(
        scanned >= 8,
        "the scan walked {scanned} files, which is fewer than this crate has: a scan that found \
         nothing would satisfy the assertion below"
    );
    assert!(
        reached.is_empty(),
        "🔴 `req/320` M-03: a gate asking the same question from another file would be invisible to \
         a census fixed on `catalogue.rs`. The map is private to that module, and this is the arm \
         that measures it rather than reading it: {reached:?}"
    );
}

// ---------------------------------------------------------------------------
// M-04 + L-01 — five positions, and the axis the doc declares
// ---------------------------------------------------------------------------

/// The five positions a declaration file spells a name in, each with `NAME` substituted.
fn five_positions(name: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "$cas_read prefix",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
                     "$cas_read": {{ "{name}": {{ "by_tool": "notes.fetch" }} }} }}"#
            ),
        ),
        (
            "$cas_read by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
                     "$cas_read": {{ "doc://": {{ "by_tool": "{name}" }} }} }}"#
            ),
        ),
        (
            "restores key",
            format!(r#"{{ "{name}": {{ "restored_by": "{RESTORE_TOOL}" }} }}"#),
        ),
        (
            "restored_by",
            format!(r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{name}" }} }}"#),
        ),
        (
            "read_by.by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
                     "read_by": {{ "by_tool": "{name}",
                                   "arguments": {{ "uri": {{ "forward": "uri" }} }},
                                   "identity": ["doc:", {{ "answer": "/id" }}] }} }} }}"#
            ),
        ),
    ]
}

/// 🔴 `req/320` M-04 and L-01: every invisible edge, in every one of the five positions.
///
/// The five scalars are the two axes together — the whitespace R24 closed in four positions, and the
/// zero-width scalars `char::is_whitespace` answers `false` for. `notes.restore` is the name being
/// padded in the four tool-name positions, and `doc://` in the prefix position, so that each row is
/// a name the file elsewhere spells without the padding.
#[test]
fn an_invisible_edge_is_refused_in_all_five_positions() {
    let scalars = [
        ("U+0020", ' '),
        ("U+00A0", '\u{00A0}'),
        ("U+3000", '\u{3000}'),
        ("U+200B", '\u{200B}'),
        ("U+FEFF", '\u{FEFF}'),
    ];
    let mut accepted: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (label, scalar) in scalars {
        for (place, _) in five_positions("x") {
            let padded = if place == "$cas_read prefix" {
                format!("{scalar}doc://")
            } else {
                format!("{RESTORE_TOOL}{scalar}")
            };
            let json = five_positions(&padded)
                .into_iter()
                .find(|(p, _)| *p == place)
                .map(|(_, body)| body)
                .expect("the position is in the table");
            let parsed = Catalogue::from_json(json.as_bytes());
            checked += 1;
            println!(
                "R25_WS position={place:<18} scalar={label} accepted={}",
                parsed.is_ok()
            );
            match parsed {
                Ok(_) => accepted.push(format!("{place} / {label}")),
                Err(why) => assert!(
                    why.contains("whitespace") && why.contains(REMEDY),
                    "the refusal names the fault and a remedy: {place} / {label}: {why}"
                ),
            }
        }
    }
    println!("R25_WS_ACCEPTED={accepted:?} checked={checked}");
    assert_eq!(checked, 25, "five scalars in five positions");
    assert!(
        accepted.is_empty(),
        "🔴 `req/320` M-04 and L-01 (`req/38` §229 ruling 2): the sets this file draws about itself \
         are compared by bytes, and the gate's own doc declares the axis as *an edge a reader \
         cannot see*. R24 closed `char::is_whitespace` in the four **tool name** positions; the \
         `$cas_read` prefix is the fifth, and U+200B / U+FEFF are invisible in all five: {accepted:?}"
    );
}

/// 🔴 The negative control: names and prefixes with nothing invisible at an edge are unchanged,
/// and an invisible scalar in the **middle** of a name is not this fault.
#[test]
fn a_name_without_an_invisible_edge_is_unchanged() {
    for (what, name) in [
        ("ordinary", "notes.get".to_string()),
        ("interior space", "notes get".to_string()),
        ("interior zero-width", "notes\u{200B}get".to_string()),
    ] {
        let json = format!(
            r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
                 "read_by": {{ "by_tool": "{name}",
                               "arguments": {{ "uri": {{ "forward": "uri" }} }},
                               "identity": ["doc:", {{ "answer": "/id" }}] }},
                 "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }} }} }}"#
        );
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R25_WS_CONTROL {what} accepted={}", parsed.is_ok());
        assert!(
            parsed.is_ok(),
            "the gate is about an **edge** a reader cannot see, not about the character: {what}: \
             {:?}",
            parsed.err()
        );
    }
    for (what, prefix) in [("plain prefix", "doc://"), ("path prefix", "file:///tmp/")] {
        let json = format!(
            r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
                 "$cas_read": {{ "{prefix}": {{ "by_tool": "notes.fetch" }} }} }}"#
        );
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R25_WS_CONTROL_PREFIX {what} accepted={}", parsed.is_ok());
        assert!(
            parsed.is_ok(),
            "the shipped prefix shapes are untouched: {what}: {:?}",
            parsed.err()
        );
    }
}

/// 🔴 The fault-ordering control (`req/38` §228 ruling 4): a name that is **only** invisible
/// characters is the *unnamed* fault and still reaches the three sentences this crate has for it.
///
/// Widening the edge predicate is exactly the move that could put this new sentence in front of
/// them, which is the regression the full-workspace floor caught R24 committing one axis over.
#[test]
fn a_name_that_is_only_invisible_characters_is_still_the_unnamed_fault() {
    for (what, name) in [
        ("spaces", "   ".to_string()),
        ("zero-width", "\u{200B}\u{FEFF}".to_string()),
    ] {
        let json = format!(r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{name}" }} }}"#);
        let why = Catalogue::from_json(json.as_bytes())
            .err()
            .unwrap_or_else(|| panic!("{what}: a name with no characters in it is refused"));
        println!("R25_UNNAMED {what} why={why}");
        assert!(
            !why.contains("starts or ends with"),
            "🔴 a reader whose file says `\"restored_by\": \"   \"` must be told to write a name, \
             not to trim one they never wrote: {what}: {why}"
        );
    }
    // And the empty `$cas_read` prefix still reaches its own sentence rather than the new one.
    let json = format!(
        r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
             "$cas_read": {{ "": {{ "by_tool": "notes.fetch" }} }} }}"#
    );
    let why = Catalogue::from_json(json.as_bytes()).expect_err("the empty prefix is refused");
    println!("R25_UNNAMED empty_prefix why={why}");
    assert!(
        !why.contains("starts or ends with"),
        "the empty prefix has no edge, and the sentence it reaches is the one about being empty: \
         {why}"
    );
}

// ---------------------------------------------------------------------------
// M-01 — the predicate, at every site that consumes a declared read
// ---------------------------------------------------------------------------

/// 🔴 `req/320` M-01: the sibling sweep, as a standing gate.
///
/// The audit's scan: three consuming sites (`adapter.rs::snapshot`, `adapter.rs::precondition`,
/// `apply.rs::observe`), one of which asked the predicate. This arm holds the **number** rather than
/// the names, so a fourth consumer added later is red here before it is measured by an audit.
#[test]
fn every_site_that_consumes_a_declared_read_asks_which_preimage_it_got() {
    // The two markers that answer the question: the predicate itself, and the name of the one
    // funnel `adapter.rs` routes its two sites through. Spelled as text rather than imported, for
    // the reason this whole file is spelled as text — a red that is *cannot find value in crate*
    // has measured the symbol table instead of the defect.
    const ASKS: [&str; 2] = ["read_answered_absent(", "name_the_preimage"];
    let mut sites = 0usize;
    let mut unanswered: Vec<String> = Vec::new();
    // 🔴 **Widened by `req/324` M-01 (`req/38` §231 ruling 2)**: `invert.rs` was not on this list,
    // and the definition of *consuming site* did not reach it either.
    //
    // R25 counted calls to `cas::read_subject(` and scanned three files. The escrow road does not
    // go through that funnel — it calls `ToolTransport::read_prior_by_tool` directly — so it was a
    // consumer of a declared read that this sweep could not see, in a file this sweep did not open.
    // A sibling sweep whose *membership* is hand-written finds the siblings somebody remembered.
    for file in ["adapter.rs", "apply.rs", "cas.rs", "invert.rs"] {
        let code = code_of(&adapter_src(file));
        // A consuming site is a call to the CAS funnel, or a direct call to the declared read face.
        // `cas.rs` holds the funnel's definition: its own call to the transport is the body of
        // `read_subject`, whose three consumers are already counted here, so counting it again
        // would ask the definition to answer a question its callers answer.
        let direct = if file == "cas.rs" {
            0
        } else {
            code.matches("read_prior_by_tool(").count()
        };
        let consumes = code.matches("cas::read_subject(").count() + direct;
        let answers: usize = ASKS.iter().map(|needle| code.matches(needle).count()).sum();
        println!("R25_ABSENCE file={file} consuming_sites={consumes} answers={answers}");
        sites += consumes;
        if consumes > answers {
            unanswered.push(format!("{file}: {consumes} consume, {answers} answer"));
        }
    }
    println!("R25_ABSENCE sites={sites}");
    assert_eq!(
        sites, 4,
        "the premise: four sites consume a declared read -- `snapshot`, `precondition`, the          post-apply observation, and the escrow road's `read_by`. If that number moved, this arm is          measuring a crate it has not been told about"
    );
    assert!(
        unanswered.is_empty(),
        "🔴 `req/320` M-01 (`req/38` §229 ruling 2): `req/312` M-01 built a predicate that separates          *the server answered that this locator holds nothing* from *the server would not answer*,          and it was asked at one site of three. The other two are `snapshot` and `precondition`, and          a reader of their refusal is handed `gx-substrate`'s frozen *the substrate would not          answer* over the top of gx's own token saying the opposite: {unanswered:?}"
    );
    // 🔴 And the funnel is not a hole: whatever `adapter.rs` routes through actually asks the
    // predicate. Without this, a lane could satisfy the arm above by naming a function that does
    // nothing — the shape `req/38` §227 ruling 1 keeps finding.
    let adapter = code_of(&adapter_src("adapter.rs"));
    assert!(
        adapter.contains("read_answered_absent("),
        "🔴 `adapter.rs` names a funnel and the funnel does not ask the predicate: {adapter:?}"
    );
}

// ---------------------------------------------------------------------------
// M-03 — the shipped gate, held to the property the two arms above demonstrate
// ---------------------------------------------------------------------------

/// 🔴 **`req/320` M-03, the half that is red rather than demonstrative.**
///
/// The two arms at the top of this file show that counting the **map** discriminates where counting
/// two **strings** does not; both are properties of functions defined here, so both are true of any
/// tree. The claim `docs/LIMITS.md` makes is about the gate this repository actually ships, and that
/// gate lives in `r24_predicate_unification.rs`. So this arm reads that file and requires it to hold
/// the property — which is the only formulation that is red on the tree where the census still
/// counted `restores.contains_key` and `.restored_by() == ` and let the equivalent spelling past.
///
/// Reading a sibling **test** file rather than a `src` one is the same move `r22_refusal_constant_census.rs`
/// makes on `wrap.rs`, one directory over: when the thing being claimed is *a gate exists and has
/// this shape*, the gate's source is the evidence.
#[test]
fn the_shipped_census_counts_the_map_and_not_two_spellings() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("r24_predicate_unification.rs");
    let gate = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
    let code = code_of(&gate);
    println!(
        "R25_SHIPPED_CENSUS field_census={} old_key_census={} old_value_census={}",
        code.contains("self.restores"),
        code.contains(r#"matches("restores.contains_key")"#),
        code.contains(r#"matches(".restored_by() == ")"#)
    );
    assert!(
        code.contains("cas_read_soundness") && code.contains("entry_fault"),
        "the scan is looking at the file it thinks it is"
    );
    assert!(
        code.contains("self.restores"),
        "🔴 `req/320` M-03 (`req/38` §229 ruling 2): the shipped census has to be a function of the \
         `restores` map, because that is the thing every spelling of the question must reach. \
         `docs/LIMITS.md` says *a `catalogue.rs` in which that question is spelled anywhere else \
         fails the build*, and the twenty-fourth audit wrote a third gate that did not fail it"
    );
    for old in [
        r#"matches("restores.contains_key")"#,
        r#"matches(".restored_by() == ")"#,
    ] {
        assert!(
            !code.contains(old),
            "🔴 and the string-counting census is gone rather than sitting beside the new one: a \
             gate that still decides on {old} decides on two spellings somebody happened to know"
        );
    }
}
