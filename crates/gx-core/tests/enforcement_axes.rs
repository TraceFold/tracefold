// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! DR-2's two axes, as types: `EnforcementMode` and `FailPosture` (**M5-08, adopted (a)**; sem:
//! SEM-gx-core-154).
//!
//! 43 §10 left their placement open for three milestones — "the placement and addition of the
//! setting types for these two axes (`FailPosture`, `EnforcementMode`) are not written in 41 §3/§4,
//! so a file-scoped ASM is filed in §10 as the minimal type addition that makes implementation
//! possible" (quoted in SEM-gx-core-155) — and `req/38_ERRATA_2026-08-07.md` §37 closed it here,
//! on the rule
//! that `VerdictKind` (E-M3-2) and `InclusionProof` (E-M2-1) were closed on: **a type two crates
//! name comes down**.
//!
//! This suite is the M5 half of `m4_types.rs`. It reads 43 §4 out of the canon for the spellings and
//! for the sentence about independence, and it checks the pair of predicates each axis carries —
//! which are the two facts a receipt records (42 §3.10's `enforced`, and `fail_posture_engaged`,
//! which **E-M2-7** put on `ReceiptPayload` back in M2).

use std::path::Path;

use gx_core::{EnforcementMode, FailPosture};

fn canon_43() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf();
    std::fs::read_to_string(root.join("req/spec/40-architecture/43-state-machine.md"))
        .expect("43 is readable")
}

/// The four spellings are 43 §4's, read from 43 §4.
#[test]
fn the_four_values_are_the_spellings_43_4_uses() {
    let canon = canon_43();
    let mut checked = 0;
    for mode in EnforcementMode::ALL {
        let spelling = format!("EnforcementMode::{}", mode.as_str());
        assert!(canon.contains(&spelling), "43 does not write `{spelling}`");
        checked += 1;
    }
    for posture in FailPosture::ALL {
        let spelling = format!("FailPosture::{}", posture.as_str());
        assert!(canon.contains(&spelling), "43 does not write `{spelling}`");
        checked += 1;
    }
    println!("DR2_SPELLINGS_FOUND_IN_43={checked}");
    assert_eq!(checked, 4, "two values on each of two axes");
}

/// The defaults are the fail-closed corner (DR-2), and they are not the same word.
///
/// 43 §4: "DR-2's default is `FailPosture::FailClosed` (every substrate) plus, optionally,
/// `EnforcementMode::RecordOnly` can be enabled" (quoted in SEM-gx-core-156). Two `bool`s would
/// have made this two literals; two enums make it two named states,
/// which is the whole of M5-08 (c)'s rejection.
#[test]
fn the_defaults_are_dr_2s() {
    println!(
        "DEFAULT_ENFORCEMENT={} DEFAULT_POSTURE={}",
        EnforcementMode::default(),
        FailPosture::default()
    );
    assert_eq!(EnforcementMode::default(), EnforcementMode::Enforce);
    assert_eq!(FailPosture::default(), FailPosture::FailClosed);
    assert_ne!(
        EnforcementMode::default().as_str(),
        FailPosture::default().as_str(),
        "the safe end of the two axes has two different names, and that is deliberate"
    );
}

/// 🔴 The axes are independent: all four settings exist, and 43 §4 says they must.
///
/// The sentence is "`FailPosture` (the posture when the verifier is unreachable) and
/// `EnforcementMode` (whether to apply even on Deny) are independent configuration axes" (quoted in
/// SEM-gx-core-157). Independence in a type system means no combination is unrepresentable, and
/// this is that check — plus the check that the canon still says so, because a `bool` pair collapsed
/// into one setting would first show up as this sentence disappearing.
///
/// The canon is Japanese and this file is not, so the sentence is located by the tokens it shares
/// with this file: one line of 43 that names both types in backticks and both parenthesised
/// conditions (`verifier`, `Deny`). That is a weaker probe than the sentence itself (it no longer
/// reads the word for "independent"); SEM-gx-core-158 records the reduction.
#[test]
fn the_two_axes_are_independent() {
    let canon = canon_43();
    let sentence = canon.lines().find(|l| {
        l.contains("`FailPosture`")
            && l.contains("`EnforcementMode`")
            && l.contains("verifier")
            && l.contains("Deny")
    });
    assert!(
        sentence.is_some(),
        "43 §4's independence sentence has moved; M5-08's shape depends on it (sem: SEM-gx-core-158)"
    );
    let mut combinations = Vec::new();
    for mode in EnforcementMode::ALL {
        for posture in FailPosture::ALL {
            combinations.push(format!("{mode}+{posture}"));
        }
    }
    println!("DR2_COMBINATIONS={} ({combinations:?})", combinations.len());
    assert_eq!(combinations.len(), 4);
}

/// The two predicates are the two facts a receipt carries (42 §3.10, 43 T-4e).
///
/// `enforced()` is `EnforcementMode`'s and `engaged()` is `FailPosture`'s, and they are deliberately
/// not one function of two arguments: T-4e reaches `enforced=false` **through the posture**, and
/// T-8r reaches it through the mode, and a receipt that could not say which road it came by would
/// lose the difference between "policy said no and we recorded it" and "nobody could be asked"
/// (sem: SEM-gx-core-159).
#[test]
fn each_axis_carries_the_receipt_fact_it_is_responsible_for() {
    assert!(EnforcementMode::Enforce.enforced());
    assert!(!EnforcementMode::RecordOnly.enforced());
    assert!(!FailPosture::FailClosed.engaged());
    assert!(FailPosture::FailOpen.engaged());
    println!(
        "RECEIPT_FACTS enforce={} record_only={} closed={} open={}",
        EnforcementMode::Enforce.enforced(),
        EnforcementMode::RecordOnly.enforced(),
        FailPosture::FailClosed.engaged(),
        FailPosture::FailOpen.engaged()
    );
}

/// The wire face is the name, and `as_str` is the same text serde writes.
///
/// The same pin `m3_types.rs` puts on `VerdictKind`: two spellings of one value is one spelling too
/// many, and the day they disagree is the day a stored setting reads back as a different one.
#[test]
fn the_text_form_is_the_serialised_form() {
    for mode in EnforcementMode::ALL {
        let json = serde_json::to_string(&mode).expect("serialisable");
        assert_eq!(json, format!("\"{}\"", mode.as_str()));
        let back: EnforcementMode = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, mode);
    }
    for posture in FailPosture::ALL {
        let json = serde_json::to_string(&posture).expect("serialisable");
        assert_eq!(json, format!("\"{}\"", posture.as_str()));
        let back: FailPosture = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, posture);
    }
}

/// Neither type reaches gx-gate, which is what 43 §4's "adds no state" (sem: SEM-gx-core-160)
/// means in practice.
///
/// The mode changes which transitions may fire; it does not change what the gate computes. If a
/// hand ever passes one of these into `Gate::verify`, the verdict stops being a function of the
/// evidence and the policy alone — and `I-11`/`J-3`/`D-11` fire, because gx-gate's shape moved.
/// This is the cheap version of that alarm, one crate down.
#[test]
fn neither_axis_is_named_by_the_gate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root")
        .to_path_buf();
    let mut offenders: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(root.join("crates/gx-gate/src")).expect("gx-gate has a src/") {
        let path = entry.expect("an entry").path();
        let text = std::fs::read_to_string(&path).expect("readable");
        for line in text.lines().map(str::trim) {
            if line.starts_with("//") {
                continue;
            }
            if line.contains("EnforcementMode") || line.contains("FailPosture") {
                offenders.push(format!("{}: {line}", path.display()));
            }
        }
    }
    println!("GATE_MENTIONS_OF_DR2={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "43 §4: record-only mode 'adds no state' (sem: SEM-gx-core-161) -- the gate does not read \
         it: {offenders:?}"
    );
}
