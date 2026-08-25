// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/476` L-02, red-first** (`req/38` §271 ruling 5 item 4; reqdef `req/479` §0-4).
//!
//! Audit 35 attacked the catalogue's reserved slots along every boundary it could find and lost:
//! a word outside the vocabulary, a capitalised word, an empty string, `"mixed"`, surrounding
//! whitespace and a non-string value are **all** parse errors, and `dr46_28_boundary_declaration.rs`
//! already holds six of those beds. One survived, and the audit graded it `L` while declaring, in
//! the finding itself, that it had **not been driven**: `req/476` §3-2 says the finding is a
//! reading of the implementation rather than a measurement — an inference from serde_json's map
//! semantics — and that no test in this repository held it.
//!
//! This file drives it. `Catalogue::from_json` deserialises into a `BTreeMap`, and a JSON object
//! carrying the same key twice is last-one-wins in every serde map: the first value is dropped
//! without a word. The module header of `catalogue.rs` declares the opposite rule for this very
//! family of keys — "a misspelling is a parse error, not a silent default" — and the reason it
//! gives is that a deployment which believes it opted in and did not is the worst outcome. A file
//! whose author declared `deterministic_replay` and then, further down, declared something else,
//! is that deployment exactly.
//!
//! # Red-first (`req/38` §226)
//!
//! Nothing this lane creates is named here: the arms hand bytes to the shipped `from_json` and read
//! the shipped error string. The suite compiles at the commit before the repair and fails there.

use gx_adapter_mcp::catalogue::{BoundaryStage, Catalogue, DETERMINISM_BOUNDARY_KEY};

/// 🔴 `req/476` L-02, driven: the second `$determinism_boundary` silently wins.
#[test]
fn r36_a_duplicated_reserved_slot_is_a_parse_error() {
    let bytes = format!(
        "{{\"{DETERMINISM_BOUNDARY_KEY}\": \"deterministic_replay\",\
         \n \"{DETERMINISM_BOUNDARY_KEY}\": \"llm_originated\"}}"
    )
    .into_bytes();
    let answered = Catalogue::from_json(&bytes);
    println!(
        "R36DUP slot answered={:?}",
        answered
            .as_ref()
            .map(|c| c.declared_input_generation().as_str())
            .map_err(std::clone::Clone::clone)
    );
    let err = answered.err().unwrap_or_else(|| {
        panic!(
            "req/476 L-02: this file declares the determinism boundary twice and was accepted. \
             serde's map takes the last one, so a deployment that wrote \
             `deterministic_replay` first is now judged `llm_originated` and was never told"
        )
    });
    assert!(
        err.contains(DETERMINISM_BOUNDARY_KEY),
        "the refusal must name the key that is duplicated: {err}"
    );
}

/// The same defect on an ordinary tool key, which is where it costs a restore rather than a
/// boundary: the second `restored_by` wins and the first is gone.
#[test]
fn r36_a_duplicated_tool_key_is_a_parse_error() {
    let bytes = br#"{"create_file": {"restored_by": "delete_file"},
                     "create_file": {"restored_by": "truncate_file"}}"#;
    let answered = Catalogue::from_json(bytes);
    println!("R36DUP tool answered_ok={}", answered.is_ok());
    let err = answered
        .expect_err("req/476 L-02: two declarations for one tool, and the file was accepted");
    assert!(
        err.contains("create_file"),
        "the refusal must name the tool that is declared twice: {err}"
    );
}

/// 🔴 The negative control. A repair that refused *every* catalogue would pass the two arms above
/// and destroy the format, so the shapes that must keep parsing are driven in the same run.
#[test]
fn r36_control_catalogues_without_duplicates_still_parse() {
    let old = br#"{"create_file": {"restored_by": "delete_file"}}"#;
    Catalogue::from_json(old).expect("the v0.1 form still parses");

    let with_slots = format!(
        "{{\"{DETERMINISM_BOUNDARY_KEY}\": \"llm_originated\",\
         \n \"create_file\": {{\"restored_by\": \"delete_file\"}},\
         \n \"notes.write\": {{\"restored_by\": \"notes.restore\"}}}}"
    )
    .into_bytes();
    let catalogue = Catalogue::from_json(&with_slots).expect("distinct keys parse");
    assert_eq!(
        catalogue.declared_input_generation(),
        BoundaryStage::LlmOriginated,
        "the single declaration is still the one that is read"
    );
}
