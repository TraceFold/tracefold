// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-28**: the determinism/LLM boundary reaches the **signed bytes**, and every value it
//! can carry has a bed that makes it false.
//!
//! Spec: 42 §3.10 for the payload table (fifteen rows now), `req/38` §255 ruling 4 for the raising,
//! `req/459` for the design. The raising, verbatim:
//!
//! > The boundary attest is new and correct -- DR-46-28 raised: *this far is deterministic
//! > (replayable), from here on it is LLM-originated*, **put on the face of the receipt** (the
//! > sibling of the cannot-be-established contract). 42 as it stands has **zero**
//! > determinism-boundary fields, confirmed by grep.
//!
//! (The ruling is written in Japanese; the block above is its content, and `req/38` §255 is where a
//! reader should go for the wording. The same applies to the acceptance test quoted below.)
//!
//! # What this file is for, which is not "the field exists"
//!
//! `req/459` ruling 4 sets the acceptance test, and it is deliberately not a test that the field is
//! present or that it round-trips:
//!
//! > KA = *is the boundary claim itself checkable*. Acceptance is: for **each value of the
//! > taxonomy**, a bed that makes that value false is **refused or corrected**, by a machine, and
//! > in a shape that does not prop a self-claim up with an assertion -- the instrument stays
//! > separate from the thing it measures.
//!
//! An attest whose claim cannot be made false is decoration with a signature on it. So each of the
//! four values below is put in a bed built to make it untrue, and the assertion is on what the
//! **production** road does with that bed — `ReceiptPayload::check_schema` for the three a decoder
//! can hand in, `DeterminismBoundary::of_stages` for the one that is arithmetic. Nowhere in this
//! file is a payload built and then asserted to have the boundary the same line just gave it; that
//! shape would restate the claim rather than test it.
//!
//! The four beds, and where each is refused:
//!
//! | value | the bed that makes it false | refused by |
//! |---|---|---|
//! | `deterministic_replay` | a receipt with **no verdict** (43 T-4e called no gate) | `check_schema` |
//! | `llm_originated` | any receipt at all — gx derives verdicts, and DR-46-27 holds that derivation to "same input, same verdict" | `check_schema` |
//! | `mixed` | two stages that are **equal** (nothing is mixed) | `check_schema` |
//! | `unknown` | two stages that **were** established | `of_stages`, which will not mint it |
//!
//! # The fifth instrument: the attest must not reach the thing it attests
//!
//! A field that certified determinism while feeding the derivation it certifies would be the
//! self-reference `req/444` §1 warns about, under its ban on over-claiming. `req/454`'s DR-46-27 answered the same
//! question for `decided_at` with a structural scan and a declared-field count, and
//! [`the_boundary_does_not_reach_the_gate`] is that instrument pointed one erratum later.

mod support;

use gx_canon::cbor;
use gx_core::{BoundaryStage, DeterminismBoundary, VerdictKind};
use gx_witness::receipt::{ReceiptKind, ReceiptPayload};
use gx_witness::Error;
use support::{commit_payload, keypair, verdict_payload};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The one key DR-46-28 adds: `text(20) "determinism_boundary"`.
///
/// `74` is major type 3 with a length of twenty. Written as a derivation rather than as a magic
/// string, so the arithmetic is checkable without a CBOR decoder — the same courtesy
/// `inverse_status_wire.rs` extends for its own key.
fn boundary_key() -> String {
    format!("74{}", hex(b"determinism_boundary"))
}

/// 🔴 **The pre-DR-46-28 form**, at the depth this file can pin it: the thirteen-key map D24
/// shipped.
///
/// Copied verbatim from `tests/inverse_status_wire.rs`'s `VERDICT_PAYLOAD_HEX_BEFORE_DR_46_26`,
/// which took it verbatim from `tests/receipt_verdict_wire.rs`'s `VERDICT_PAYLOAD_HEX`. It is
/// duplicated a second time for the reason the first duplication gives: the three files pin the
/// same bytes for three different reasons, and a shared constant would let one erratum's
/// regeneration silently rewrite another's evidence.
///
/// **No new golden is minted here.** DR-46-26 established the form — the old literal stays and the
/// change is measured as a *subtraction* from what the encoder produces now — and this lane adds a
/// layer to that subtraction rather than a literal of its own. One golden, three errata, four
/// states of the wire.
const VERDICT_PAYLOAD_HEX_BEFORE_DR_46_26: &str = "\
ad666b65795f6964656b65792d316776657264696374a2646b696e646541646d69746c70726f6f665f64696765737458\
20000000000007a12500000000000000000000000000000000000000000000000068656e666f72636564f46872656164\
5f736574f66c726563656970745f6b696e646e56657264696374526563656970746d63616e6f6e6963616c5f63696458\
2000000000008954450000000000000000000000000000000000000000000000006d696e76657273655f64656c7461f6\
6e7472616e73666f726d6174696f6e582000000000008954450000000000000000000000000000000000000000000000\
006f696e636c7573696f6e5f70726f6f66f67166696e6765727072696e745f73636f7065781a666978747572653a2f2f\
73636f70652f6f6e652d6f626a656374746661696c5f706f73747572655f656e6761676564f57818707265636f6e6469\
74696f6e5f66696e6765727072696e745820070707070707070707070707070707070707070707070707070707070707\
07077819706f7374636f6e646974696f6e5f66696e6765727072696e74f6";

/// DR-46-26's key, subtracted here only so that this file's subtraction can reach D24's literal.
///
/// Copied from `tests/inverse_status_wire.rs`, where the claim about *this* key is asserted. Here
/// it is a layer and nothing more.
const REVERSIBILITY_KEY_AND_NULL: &str = "6d7265766572736962696c697479f6";

// ---------------------------------------------------------------------------
// The declaration
// ---------------------------------------------------------------------------

/// The field exists, is **not** an `Option`, and the struct has sixteen.
///
/// Structural rather than behavioural for `inverse_status_wire.rs`'s reason: a probe can show a
/// value round-trips, and only a scan can show the *declaration* is the one the erratum names. The
/// `Option` half is the load-bearing one — `req/459` ruling 3 makes `unknown` a first-class value,
/// and an `Option` around it would put two shapes on one fact and rebuild exactly the defect
/// DR-46-26 spent a lane closing.
#[test]
fn dr_46_28_the_payload_declares_a_boundary_field_that_is_not_optional() {
    let src = include_str!("../src/receipt.rs");
    let body = src
        .split("pub struct ReceiptPayload {")
        .nth(1)
        .expect("receipt.rs declares ReceiptPayload")
        .split("\n}")
        .next()
        .expect("split always yields one");
    let field = body
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("pub determinism_boundary"))
        .expect("DR-46-28 seats the boundary on the payload");
    println!("RECEIPT_BOUNDARY_FIELD={field:?}");
    assert_eq!(
        field, "pub determinism_boundary: DeterminismBoundary,",
        "`unknown` is a value here; an Option would be a second shape for the same absence"
    );

    let fields = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub ") && l.ends_with(','))
        .count();
    println!("RECEIPT_PAYLOAD_FIELDS={fields}");
    // 🔴 **G-07 live re-run (`req/801`, 2026-08-25)** — sixteen became seventeen when DR-46-39
    // (`req/777`) seated `catalogue_hash`; this pin was not taught on landing day.
    // 🔴 **F7 / `req/868` R-868-6 / `req/919` W5 (2026-08-29)** — seventeen became eighteen when
    // `payload_version` seated.
    assert_eq!(
        fields, 19,
        "DR-46-26 took the count to fourteen, DR-46-28 added the fifteenth, S③ (`req/493`          §1 AC-6) the sixteenth, DR-46-39 (`req/777` catalogue_hash) the seventeenth, F7 (`req/868` R-868-6, `payload_version`) the eighteenth, and A2 (`req/910`, `engine_version`) the nineteenth"
    );
}

// ---------------------------------------------------------------------------
// KA — the four beds
// ---------------------------------------------------------------------------

/// 🔴 **Bed 1 — `deterministic_replay` made false.** No gate ran, so there is no derivation.
///
/// 43 T-4e degrades a transformation to record-only *without calling the gate at all*: that is the
/// road `verdict: None` exists for (E-M5-11). A receipt on that road claiming its verdict
/// derivation is replay-deterministic is describing a property of something that did not happen —
/// and `Receipt::issue` checks the schema **before** it signs, so the claim never reaches a
/// signature either.
#[test]
fn ka_bed_a_no_verdict_refuses_a_replay_determinism_claim() {
    let key = keypair(1);
    let degraded = support::degraded_payload(&key, 5);
    assert!(
        degraded.verdict.is_none(),
        "the bed needs 43 T-4e's road, which is the one without a verdict"
    );
    degraded
        .check_schema()
        .expect("the fixture's own `unknown` is what that road may say");

    for claim in [
        DeterminismBoundary::DeterministicReplay,
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::LlmOriginated,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::Unknown,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
    ] {
        let payload = ReceiptPayload {
            determinism_boundary: claim,
            ..degraded.clone()
        };
        let detail = refusal(&payload);
        println!("KA_BED_A {} -> {detail}", claim.as_str());
        assert!(
            detail.contains("determinism_boundary") && detail.contains("no verdict"),
            "the refusal names the field and the reason"
        );
        // and the same bed cannot get a signature, which is where it would have mattered
        gx_witness::Receipt::issue(&payload, support::issued_at(), &key)
            .expect_err("a receipt is schema-checked before it is signed");
    }
}

/// 🔴 **Bed 2 — `llm_originated` made false.** gx derives verdicts; a model does not.
///
/// The value claims *both* stages, and one of the two stages is gx's own arithmetic. `req/454`'s
/// DR-46-27 is what holds that arithmetic to "same input, same verdict"; this refusal is that
/// ruling read one erratum forward, and it applies on both receipt kinds and whether or not a
/// verdict is present — which is why the loop below runs three fixtures and not one.
#[test]
fn ka_bed_b_a_receipt_may_not_say_gx_derived_a_verdict_from_a_model() {
    let key = keypair(1);
    let proof = gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    };
    let beds = [
        ("verdict", verdict_payload(VerdictKind::Admit, &key, 5)),
        ("degraded", support::degraded_payload(&key, 5)),
        ("commit", commit_payload(&key, 11, proof)),
    ];
    for (name, base) in beds {
        for claim in [
            DeterminismBoundary::LlmOriginated,
            DeterminismBoundary::Mixed {
                input_generation: BoundaryStage::DeterministicReplay,
                verdict_derivation: BoundaryStage::LlmOriginated,
            },
            DeterminismBoundary::Mixed {
                input_generation: BoundaryStage::Unknown,
                verdict_derivation: BoundaryStage::LlmOriginated,
            },
        ] {
            let payload = ReceiptPayload {
                determinism_boundary: claim,
                ..base.clone()
            };
            let detail = refusal(&payload);
            println!("KA_BED_B {name}/{} -> {detail}", claim.as_str());
            assert!(
                detail.contains("determinism_boundary") && detail.contains("from a model"),
                "the refusal names the field and the claim it will not carry"
            );
        }
    }
}

/// 🔴 **Bed 3 — `mixed` made false.** Two stages that are equal are not a mixture.
///
/// `req/459` ruling 3 words the value as *enumerated by stage*, which is why the
/// variant carries its two stages instead of being a marker. The bed is the degenerate enumeration,
/// and it is refused in the same breath in which
/// [`the_arithmetic_will_not_produce_a_degenerate_mixture`] shows the production arithmetic never
/// builds one.
#[test]
fn ka_bed_c_a_mixture_of_one_class_with_itself_is_refused() {
    let key = keypair(1);
    let proof = gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    };
    let base = commit_payload(&key, 11, proof);
    for stage in [
        BoundaryStage::DeterministicReplay,
        BoundaryStage::LlmOriginated,
        BoundaryStage::Unknown,
    ] {
        let payload = ReceiptPayload {
            determinism_boundary: DeterminismBoundary::Mixed {
                input_generation: stage,
                verdict_derivation: stage,
            },
            ..base.clone()
        };
        let detail = refusal(&payload);
        println!("KA_BED_C {} -> {detail}", stage.as_str());
        assert!(
            detail.contains("determinism_boundary") && detail.contains("one class twice"),
            "the refusal names the field and says what is degenerate about the value"
        );
    }
}

/// 🔴 **Bed 4 — `unknown` made false.** It may not be minted over a stage that was established.
///
/// This is the bed the other three cannot be: a collapsed `Unknown` carries no stages, so no
/// payload-local rule can see that something *was* known. The refusal is therefore in the
/// arithmetic, and it is total — every pair of stages that is not two `Unknown`s answers something
/// else. Said the other way round: `unknown` here is the cannot-be-established sibling and not an escape
/// hatch, and this test is the difference between those two readings.
#[test]
fn ka_bed_d_unknown_is_not_an_escape_hatch() {
    let stages = [
        BoundaryStage::DeterministicReplay,
        BoundaryStage::LlmOriginated,
        BoundaryStage::Unknown,
    ];
    let mut minted = 0usize;
    let mut pairs = 0usize;
    for input_generation in stages {
        for verdict_derivation in stages {
            pairs += 1;
            let answer = DeterminismBoundary::of_stages(input_generation, verdict_derivation);
            println!(
                "KA_BED_D of_stages({}, {}) = {}",
                input_generation.as_str(),
                verdict_derivation.as_str(),
                answer.as_str()
            );
            let both_unknown = input_generation == BoundaryStage::Unknown
                && verdict_derivation == BoundaryStage::Unknown;
            if answer == DeterminismBoundary::Unknown {
                minted += 1;
                assert!(
                    both_unknown,
                    "`unknown` was minted over a stage that had a class"
                );
            } else {
                assert!(
                    !both_unknown,
                    "two unestablished stages answered {answer:?}"
                );
            }
        }
    }
    println!("KA_BED_D_PAIRS={pairs} KA_BED_D_UNKNOWN_MINTED={minted}");
    assert_eq!(pairs, 9, "three stages squared is the whole domain");
    assert_eq!(
        minted, 1,
        "exactly one of the nine pairs may answer `unknown`"
    );
}

/// The arithmetic's other half, stated where a reader looks for it: equal stages collapse.
///
/// Bed 3 shows a hand-built degenerate `Mixed` is refused; this shows the production road never
/// hands one in, so the refusal is a second defence rather than the only one — the same two-layer
/// shape `check_schema` and `gx_engine::Error::Unrepresentable` already have for `verdict`.
#[test]
fn the_arithmetic_will_not_produce_a_degenerate_mixture() {
    let stages = [
        BoundaryStage::DeterministicReplay,
        BoundaryStage::LlmOriginated,
        BoundaryStage::Unknown,
    ];
    for stage in stages {
        let answer = DeterminismBoundary::of_stages(stage, stage);
        println!("COLLAPSE {} -> {}", stage.as_str(), answer.as_str());
        assert!(
            !matches!(answer, DeterminismBoundary::Mixed { .. }),
            "equal stages answered a mixture"
        );
    }
    for input_generation in stages {
        for verdict_derivation in stages {
            if input_generation == verdict_derivation {
                continue;
            }
            let answer = DeterminismBoundary::of_stages(input_generation, verdict_derivation);
            assert_eq!(
                answer,
                DeterminismBoundary::Mixed {
                    input_generation,
                    verdict_derivation
                },
                "differing stages are a mixture, and the mixture names them"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The attest must not reach the thing it attests
// ---------------------------------------------------------------------------

// 🔴 `the_boundary_does_not_reach_the_gate` MOVED, req/38 SS991 (2026-08-31), to
// `boundary_gate_reach.rs` in this same directory. Not deleted, not weakened: the test is
// byte-for-byte what it was, and both `include_str!` paths resolve identically because the file
// sits in the same directory.
//
// Why it had to leave this file: it embeds `../../gx-gate/tests/gate_input_spec.rs`, which is on
// the public sync's canon-reading exclusion set (it names `req/spec` outside a comment).
// `include_str!` resolves before any Rust is read, so a shipped file that embeds a withheld file
// does not compile on a public clone — measured, twice (`req/38_ERRATA_2026-08-07.md` §SS773, and
// the staging check that produced this move).
//
// `tools/pub_sync_dryrun.sh` now closes its exclusion set under `include_str!`/`include_bytes!`
// reachability, so whichever file holds this test is withheld automatically. Left here, that
// closure would have withheld all eleven tests in this file to withhold one. The other ten are
// ordinary tests of `DeterminismBoundary` that owe the private tree nothing, and they still ship.

// ---------------------------------------------------------------------------
// The wire
// ---------------------------------------------------------------------------

/// 🔴 **The subtraction, one layer deeper.** Today's bytes, minus this key and DR-46-26's, are
/// D24's.
///
/// `receipt_verdict_wire.rs` established the discipline and `inverse_status_wire.rs` added the
/// second layer to it: the golden from before a change is **kept**, and each erratum is measured as
/// a subtraction from what the encoder produces now. A regenerated golden records what the code
/// does; a golden carried across three errata records what none of them did.
///
/// So this test mints no literal. It removes `determinism_boundary`, winds the map header from
/// fifteen back to fourteen, removes `reversibility`, winds it back to thirteen, and asserts the
/// result is the map D24 shipped — byte for byte, with a field order, a value and a header that no
/// hand in this lane could have adjusted without the comparison failing.
#[test]
fn the_wire_moved_by_exactly_the_one_key_dr_46_28_added() {
    let key = keypair(1);
    let now = hex(&cbor::encode(&verdict_payload(VerdictKind::Admit, &key, 5)).expect("canonical"));
    let added = boundary_key();
    println!("WIRE_ADDED_BOUNDARY_KEY={added}");
    println!("WIRE_NOW_BYTES={}", now.len() / 2);
    assert!(
        now.contains(&added),
        "the boundary key is not on the wire; this subtraction would measure nothing"
    );
    // 🔴 **DR-46-39 (`req/777` `catalogue_hash`, seventeenth key)** — the header check moved with
    // it (`req/801` G-07 live re-run, 2026-08-25).
    // 🔴 **F7 (`req/868` R-868-6, `payload_version`, eighteenth key, `req/919` W5, 2026-08-29)** —
    // the header check moved again.
    // 🔴 **A2 (`req/910` A., `req/919` W8, 2026-08-30)** — and again for the nineteenth.
    assert!(
        now.starts_with("b3"),
        "a canonical map of nineteen opens `b3`; today's opens {}",
        &now[..2]
    );

    // The fixture's value is `unknown`, so the key's whole contribution is itself and `text(7)`.
    let contribution = format!("{added}67{}", hex(b"Unknown"));
    assert!(
        now.contains(&contribution),
        "the fixture's boundary is `unknown`, and its encoding is the key and that word"
    );

    // 🔴 **S③ (`req/493` §1 AC-6) is the outermost layer of the subtraction now.**
    //
    // The discipline this file established does not change when a fourth erratum arrives; it gains
    // a step. `confinement` comes off first and the header winds sixteen → fifteen, after which the
    // two layers below are exactly the ones this test already asserted. The key's contribution is
    // derived from the encoder rather than written as a literal, for `boundary_key`'s reason: an
    // arithmetic a reader can check beats a magic string a hand can regenerate.
    let confinement_contribution = format!(
        "6b{}{}",
        hex(b"confinement"),
        hex(
            &cbor::encode(&Some(gx_witness::receipt::ConfinementContext::unconfined()))
                .expect("canonical")
        )
    );
    println!("WIRE_ADDED_CONFINEMENT_KEY={confinement_contribution}");
    assert!(
        now.contains(&confinement_contribution),
        "the confinement key is not on the wire; this subtraction would measure nothing"
    );
    // 🔴 **DR-46-39 (`req/777`)** — a fifth layer, outermost: `catalogue_hash` comes off first and
    // the header winds seventeen → sixteen. The fixture names no catalogue, so the contribution is
    // the key and a null. Found red by `req/801`'s G-07 live re-run (2026-08-25) — DR-46-39's lane
    // taught its own attest suite and `ac_018` but none of the three subtraction towers.
    let catalogue_contribution = format!("6e{}f6", hex(b"catalogue_hash"));
    assert!(
        now.contains(&catalogue_contribution),
        "DR-46-39's key is not on the wire; this subtraction would measure nothing"
    );
    // 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)** — a sixth layer, outermost:
    // `payload_version` comes off first and the header winds eighteen → seventeen. The fixture
    // carries `Some(CURRENT_PAYLOAD_VERSION)` (every receipt this build issues does), so the
    // contribution is the key and the encoded small uint -- derived from the encoder, not written
    // as a literal, for `boundary_key`'s reason.
    let payload_version_contribution = format!(
        "6f{}{}",
        hex(b"payload_version"),
        hex(&cbor::encode(&Some(gx_witness::receipt::CURRENT_PAYLOAD_VERSION)).expect("canonical"))
    );
    println!("WIRE_ADDED_PAYLOAD_VERSION_KEY={payload_version_contribution}");
    assert!(
        now.contains(&payload_version_contribution),
        "F7's key is not on the wire; this subtraction would measure nothing"
    );
    // 🔴 **A2 (`req/910` A., `req/919` W8, 2026-08-30)** — a seventh layer, outermost:
    // `engine_version` comes off first and the header winds nineteen → eighteen. Derived from the
    // fixture's own constant rather than re-spelled, so a tower cannot keep passing after the
    // string it subtracts has moved.
    let engine_version_contribution = format!(
        "6e{}{}",
        hex(b"engine_version"),
        hex(&cbor::encode(&Some(support::FIXTURE_ENGINE_VERSION.to_string())).expect("canonical"))
    );
    println!("WIRE_ADDED_ENGINE_VERSION_KEY={engine_version_contribution}");
    assert!(
        now.contains(&engine_version_contribution),
        "A2's key is not on the wire; this subtraction would measure nothing"
    );
    let stripped = now
        .replacen(&engine_version_contribution, "", 1)
        .replacen("b3", "b2", 1)
        .replacen(&payload_version_contribution, "", 1)
        .replacen("b2", "b1", 1)
        .replacen(&catalogue_contribution, "", 1)
        .replacen("b1", "b0", 1)
        .replacen(&confinement_contribution, "", 1)
        .replacen("b0", "af", 1)
        .replacen(&contribution, "", 1)
        .replacen("af", "ae", 1)
        .replacen(REVERSIBILITY_KEY_AND_NULL, "", 1)
        .replacen("ae", "ad", 1);
    println!("WIRE_STRIPPED_BYTES={}", stripped.len() / 2);
    assert_eq!(
        stripped,
        VERDICT_PAYLOAD_HEX_BEFORE_DR_46_26.replace(['\n', ' '], ""),
        "an erratum in this chain moved more than the one key it declares"
    );
}

/// The four values are distinguishable **on the wire**, which is the point of putting them there.
///
/// `req/350` §7-7's rule, applied a third time: "run both values of the tag through a path that
/// actually reaches them, once each (**introducing is not functioning**)". Four values here — and
/// `Mixed` twice, in both orders, because a variant whose two stages encoded to the same bytes
/// whichever way round they went would carry an enumeration that enumerated nothing.
#[test]
fn the_four_values_encode_differently_from_each_other() {
    let key = keypair(1);
    let proof = gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    };
    let base = commit_payload(&key, 11, proof);

    let mut seen: Vec<(String, String)> = Vec::new();
    for value in [
        DeterminismBoundary::DeterministicReplay,
        DeterminismBoundary::Unknown,
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::LlmOriginated,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::DeterministicReplay,
            verdict_derivation: BoundaryStage::Unknown,
        },
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::Unknown,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
    ] {
        let payload = ReceiptPayload {
            determinism_boundary: value,
            ..base.clone()
        };
        payload
            .check_schema()
            .expect("a commit receipt that carries a verdict may say any of these");
        let bytes = hex(&cbor::encode(&payload).expect("canonical"));
        println!("BOUNDARY_WIRE {}={} bytes", value.as_str(), bytes.len() / 2);
        for (other, prior) in &seen {
            assert_ne!(
                &bytes,
                prior,
                "`{}` and `{other}` encode to the same bytes",
                value.as_str()
            );
        }
        seen.push((value.as_str(), bytes));
    }
    assert_eq!(seen.len(), 5, "five shapes were compared");
    // `llm_originated` is the sixth value the vocabulary has and the one no receipt may carry:
    // its bed is `ka_bed_b`, and it is named here so this count is not read as "all of them".
    println!("BOUNDARY_WIRE_REFUSED_ON_RECEIPTS=llm_originated");
}

/// The vocabulary, and that `Mixed`'s word carries its two stages into a report line.
#[test]
fn the_four_words_are_req_459s() {
    println!("BOUNDARY_ALL={:?}", DeterminismBoundary::ALL);
    println!("BOUNDARY_STAGE_ALL={:?}", BoundaryStage::ALL);
    assert_eq!(
        DeterminismBoundary::ALL,
        ["deterministic_replay", "llm_originated", "mixed", "unknown"]
    );
    assert_eq!(
        BoundaryStage::ALL,
        ["deterministic_replay", "llm_originated", "unknown"]
    );
    assert_eq!(
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::LlmOriginated,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        }
        .as_str(),
        "mixed(llm_originated/deterministic_replay)",
        "a refusal that said only `mixed` would be about a value the reader cannot see"
    );
}

/// A `VerdictReceipt` carries the field too, and that is deliberate rather than an omission.
///
/// It is the first of the erratum fields with **no** kind-dependent rule. `read_set`,
/// `reversibility` and `inverse_delta` are all absent on a verdict receipt because the escrow that
/// answers them is 43 T-10b, inside commit. The boundary is not answered by the escrow: the
/// question "did a gate derive this, and from what kind of input" is asked at T-4a as much as at
/// T-11, and a verdict receipt that could not say so would leave every Deny outside the attest.
#[test]
fn the_boundary_has_no_kind_dependent_rule() {
    let key = keypair(1);
    let clean = verdict_payload(VerdictKind::Deny, &key, 5);
    assert_eq!(clean.receipt_kind, ReceiptKind::VerdictReceipt);
    for value in [
        DeterminismBoundary::DeterministicReplay,
        DeterminismBoundary::Unknown,
        DeterminismBoundary::Mixed {
            input_generation: BoundaryStage::LlmOriginated,
            verdict_derivation: BoundaryStage::DeterministicReplay,
        },
    ] {
        let payload = ReceiptPayload {
            determinism_boundary: value,
            ..clean.clone()
        };
        payload.check_schema().unwrap_or_else(|e| {
            panic!("ASM-14 has no rule about the boundary: {value:?} -> {e:?}")
        });
        println!("VERDICT_RECEIPT_ALLOWS={}", value.as_str());
    }
}

/// The refusal's sentence, or a panic saying what came instead.
fn refusal(payload: &ReceiptPayload) -> String {
    match payload
        .check_schema()
        .expect_err("the bed is built to be refused")
    {
        Error::Schema { detail } => detail,
        other => panic!("the schema refuses; it answered {other:?}"),
    }
}
