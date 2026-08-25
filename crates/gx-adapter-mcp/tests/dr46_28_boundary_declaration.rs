// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-28**, declaration half: the catalogue's fourth reserved slot, and the one function
//! where the declaration meets the attest.
//!
//! `req/459` ruling 1 splits the erratum in two: **declare** in the catalogue, following the three
//! reserved-slot precedents and staying backward compatible, and **attest** on the receipt with one
//! added field the declaration can be overridden by. (The ruling is written in Japanese; the
//! sentence above is its content, and `req/459` is the source.) `gx-witness`'s
//! `tests/boundary_attest.rs` is the attest half. This is the other one, and it asserts three
//! things the slot has to be true of before it is worth having:
//!
//! 1. **Backward compatible.** Every catalogue written before this window parses unchanged and
//!    means `unknown` — which is what those files meant without a way to say it.
//! 2. **A misspelling is a parse error, not a silent default.** [`ON_READ_FAILURE_KEY`]'s rule,
//!    for its reason: a deployment that believed it had declared where its inputs come from and
//!    quietly had not is worse than one that was told its file is wrong.
//! 3. **The slot declares one stage, not the boundary.** A file may not say anything about gx's own
//!    verdict derivation. That half is observed, not declared — and the join is the only place the
//!    two meet.

use gx_adapter_mcp::catalogue::{
    BoundaryStage, Catalogue, DeterminismBoundary, DETERMINISM_BOUNDARY_KEY,
};

/// A catalogue with no slot means `unknown`, and every pre-existing file is one.
#[test]
fn an_absent_slot_is_unknown_and_the_old_files_still_parse() {
    let old = br#"{"create_file": {"restored_by": "delete_file"}}"#;
    let catalogue = Catalogue::from_json(old).expect("a catalogue from before this window");
    println!(
        "DECLARED_INPUT_GENERATION={}",
        catalogue.declared_input_generation().as_str()
    );
    assert_eq!(
        catalogue.declared_input_generation(),
        BoundaryStage::Unknown
    );
    assert_eq!(
        Catalogue::new().declared_input_generation(),
        BoundaryStage::Unknown,
        "a catalogue built in code has established nothing either"
    );
}

/// The three words a file may spell, and that each one lands on the stage it names.
#[test]
fn the_slot_carries_one_of_req_459s_three_stage_words() {
    for (word, expected) in [
        ("deterministic_replay", BoundaryStage::DeterministicReplay),
        ("llm_originated", BoundaryStage::LlmOriginated),
        ("unknown", BoundaryStage::Unknown),
    ] {
        let bytes = format!(r#"{{"{DETERMINISM_BOUNDARY_KEY}": "{word}"}}"#).into_bytes();
        let catalogue = Catalogue::from_json(&bytes).expect("the word is one of the three");
        println!(
            "SLOT {word} -> {}",
            catalogue.declared_input_generation().as_str()
        );
        assert_eq!(catalogue.declared_input_generation(), expected);
        assert_eq!(
            catalogue.declared_input_generation().as_str(),
            word,
            "the word a file spells is the word the value answers"
        );
    }
    assert_eq!(
        BoundaryStage::ALL,
        ["deterministic_replay", "llm_originated", "unknown"],
        "the vocabulary the parser is written against"
    );
}

/// 🔴 A misspelt or wrong-typed slot is refused, and the refusal shows the three words.
///
/// The bed that matters is `"llm-originated"` with a hyphen: a file whose author believed they had
/// declared an LLM origin. Taking it as `unknown` would be that belief left standing.
#[test]
fn a_value_that_is_not_one_of_the_three_is_a_parse_error() {
    for bad in [
        r#""llm-originated""#,
        r#""LlmOriginated""#,
        r#""mixed""#,
        r#""""#,
        "true",
        "[\"llm_originated\"]",
    ] {
        let bytes = format!(r#"{{"{DETERMINISM_BOUNDARY_KEY}": {bad}}}"#).into_bytes();
        let err = Catalogue::from_json(&bytes)
            .expect_err("a slot that is not one of the three words is a parse error");
        println!("SLOT_REFUSED {bad} -> {err}");
        assert!(
            err.contains(DETERMINISM_BOUNDARY_KEY) && err.contains("deterministic_replay"),
            "the refusal names the slot and the vocabulary"
        );
    }
}

/// 🔴 **`mixed` is not a word a file may spell**, and that is the two-face split showing through.
///
/// `mixed` is a property of a *transformation* — one stage differed from the other — and a
/// catalogue declares one stage. A file that could say "mixed" would be declaring something about
/// gx's verdict derivation, which is the self-claim `req/444` §1 refuses. Asserted separately from
/// the parse-error test above because the reason is different: `"mixed"` is not a typo.
#[test]
fn a_file_may_not_declare_anything_about_gxs_own_derivation() {
    let bytes = format!(r#"{{"{DETERMINISM_BOUNDARY_KEY}": "mixed"}}"#).into_bytes();
    Catalogue::from_json(&bytes).expect_err("`mixed` is about a transformation, not a declaration");

    let src = include_str!("../src/catalogue.rs");
    let parsed: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('"') && l.contains("=> BoundaryStage::"))
        .collect();
    println!("SLOT_PARSE_ARMS={parsed:?}");
    assert_eq!(
        parsed.len(),
        3,
        "the parser accepts exactly the three stage words"
    );
}

/// 🔴 **The join** — the one function where the declared half and the observed half meet.
///
/// The whole point of the two faces: a deployment that declares its inputs LLM-originated, on a
/// transformation gx gated, produces exactly the sentence `req/38` §255 ruling 4 asked a receipt to
/// carry — *this far is deterministic, from here on it is LLM-originated* — with both stages named.
#[test]
fn the_declaration_and_the_observation_join_into_req_459s_taxonomy() {
    let declared = Catalogue::new().with_declared_input_generation(BoundaryStage::LlmOriginated);

    let gated = declared.declared_boundary(BoundaryStage::DeterministicReplay);
    println!("JOIN gated -> {}", gated.as_str());
    assert_eq!(
        gated,
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::LlmOriginated,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
        "the boundary DR-46-28 exists to state"
    );

    // 43 T-4e: no gate was asked, so nothing is known about the derivation either.
    let ungated = declared.declared_boundary(BoundaryStage::Unknown);
    println!("JOIN ungated -> {}", ungated.as_str());
    assert_eq!(
        ungated,
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::LlmOriginated,
            verdict_derivation: BoundaryStage::Unknown,
        },
        "the LLM origin survives a road that derived no verdict; only the other stage goes unknown"
    );

    // And a deployment that declared nothing joins to `unknown` on that road, which is the v0
    // engine's actual output: `pipeline::attested_boundary` writes exactly this pair.
    let silent = Catalogue::new();
    assert_eq!(
        silent.declared_boundary(BoundaryStage::Unknown),
        DeterminismBoundary::Unknown,
        "two unestablished stages are the one pair that answers `unknown`"
    );
    assert_eq!(
        silent.declared_boundary(BoundaryStage::DeterministicReplay),
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::Unknown,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
        "this is what every receipt gx issues today carries; see docs/LIMITS.md"
    );
}

/// The fourth slot is a fourth slot: `$`-prefixed, and the three that came before still work.
///
/// A file that carries all four reserved keys and a tool declaration parses, and none of the keys
/// is read as a tool. The bed is the one `SERVER_METADATA_KEY`'s doc names: `RestoreSpec` has
/// `deny_unknown_fields`, so a reserved key read as a declaration would fail loudly — which is why
/// this test is a parse and not an inspection.
#[test]
fn the_four_reserved_slots_coexist() {
    let bytes = br#"{
        "$server": {"name": "fixture", "version": "0"},
        "$on_read_failure": "unknown",
        "$determinism_boundary": "llm_originated",
        "create_file": {"restored_by": "delete_file"}
    }"#;
    let catalogue = Catalogue::from_json(bytes).expect("four reserved slots and one tool");
    println!(
        "FOUR_SLOTS declared={} input_generation={}",
        catalogue.declared(),
        catalogue.declared_input_generation().as_str()
    );
    assert_eq!(
        catalogue.declared(),
        1,
        "the reserved keys are metadata; exactly one tool was declared"
    );
    assert_eq!(
        catalogue.declared_input_generation(),
        BoundaryStage::LlmOriginated
    );
}
