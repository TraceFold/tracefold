// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 6 (`req/338` §0-6, from `req/334` L-03)** — the control that can be made to fail
//! by the thing it controls for.
//!
//! `r27_limits_probe_counts.rs` holds a registry of the probe counts `docs/LIMITS.md` states, and
//! above it a **bed control** whose stated purpose is that the registry must not hold the page to
//! numbers that are themselves wrong. The twenty-seventh audit found the registry's own row wrong
//! and the bed control green, and named why: *the same way of counting, run twice, is not two
//! layers.*
//!
//! This file is the second way. It fires both derivations
//! (`support/probe_counts.rs`, shared rather than copied) at texts whose right answer is known by
//! construction, and requires them to **disagree** where the defect lived. A control that no input
//! can make fail is a decoration.
//!
//! It lives in its own file for a reason worth writing down: an arm added to
//! `r27_limits_probe_counts.rs` would change that file's own probe count, which is the exact number
//! the L-03 repair corrects. An instrument that alters the quantity it measures is the shape this
//! whole arc keeps repairing.

#[path = "support/probe_counts.rs"]
mod probe_counts;

use probe_counts::{probe_count, probe_names, substring_count};

fn record(line: &str) {
    println!("{line}");
    let path = std::env::var("R28_MEAS")
        .unwrap_or_else(|_| "/mnt/c/work/r28_logs/r28_measurements.txt".to_string());
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

/// 🔴 The bug, reproduced on a text small enough to have no other explanation.
#[test]
fn a_a_quoted_needle_counts_as_a_probe_under_the_old_method_and_not_under_the_new() {
    let quoted = "fn counts(text: &str) -> usize {\n    text.matches(\"#[test]\").count()\n}\n";
    record(&format!(
        "R28_COUNTER_QUOTED substring={} attributes={} names={:?}",
        substring_count(quoted),
        probe_count(quoted),
        probe_names(quoted)
    ));
    assert_eq!(
        substring_count(quoted),
        1,
        "🔴 the old method has to count the quoted needle, or this arm is not measuring the bug \
         that put a false number on a public page"
    );
    assert_eq!(
        probe_count(quoted),
        0,
        "🔴 a quoted needle is not a probe. This is the whole of `req/334` L-03: the counter \
         counted the string it was written with, the registry believed it, and the page printed it"
    );
    assert!(
        probe_names(quoted).is_empty(),
        "🔴 and the second method has to reach the same answer by a different route: a quoted \
         needle introduces no `fn` item: {:?}",
        probe_names(quoted)
    );
}

/// 🔴 And the counter still sees a real probe — a repair that closes a defect by counting nothing
/// has not repaired anything.
#[test]
fn b_a_real_attribute_is_still_a_probe_under_both_methods() {
    let real = "#[test]\nfn a_real_probe() {\n    assert!(true);\n}\n";
    let asyncy = "#[tokio::test]\nasync fn an_async_probe() {\n    assert!(true);\n}\n";
    record(&format!(
        "R28_COUNTER_REAL attributes={} names={:?} async_attributes={} async_names={:?}",
        probe_count(real),
        probe_names(real),
        probe_count(asyncy),
        probe_names(asyncy)
    ));
    assert_eq!(probe_count(real), 1, "🔴 a real attribute is a probe");
    assert_eq!(probe_names(real), vec!["a_real_probe".to_string()]);
    assert_eq!(
        probe_count(asyncy),
        1,
        "🔴 `#[tokio::test]` is a probe too. No registered suite uses it today, so this changes no \
         number — but a suite converted to async tests must fail this census rather than have its \
         probes quietly leave it"
    );
    assert_eq!(probe_names(asyncy), vec!["an_async_probe".to_string()]);
}

/// 🔴 The two methods part company on shipped files, not only on texts this arm made up — and the
/// registry file no longer inflates itself.
///
/// Two shipped files, two opposite requirements:
///
/// * `support/probe_counts.rs` **quotes** the needle (it is where both counters are written) and
///   declares no probe at all. The old method must be the higher number there, or this arm is
///   measuring nothing.
/// * `r27_limits_probe_counts.rs` is the file the defect was **about**. Moving the counter out of it
///   is what stops it counting itself, so the two methods must now agree there — and that agreement
///   is the mechanical statement of the L-03 repair.
///
/// `req/332` §10-2 recorded this row being "corrected" from four to five. The correction moved the
/// number away from the truth because it trusted the counter. Both numbers are recorded here so the
/// evidence stays in the log rather than being tidied away.
#[test]
fn c_the_two_methods_part_company_where_the_needle_is_quoted_and_agree_where_it_is_not() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let derivation = std::fs::read_to_string(tests.join("support/probe_counts.rs"))
        .expect("the shared derivation is readable");
    let registry = std::fs::read_to_string(tests.join("r27_limits_probe_counts.rs"))
        .expect("the registry suite is readable");
    record(&format!(
        "R28_COUNTER_ON_THE_DERIVATION substring={} attributes={} names={:?}",
        substring_count(&derivation),
        probe_count(&derivation),
        probe_names(&derivation)
    ));
    record(&format!(
        "R28_COUNTER_ON_THE_REGISTRY substring={} attributes={} names={:?}",
        substring_count(&registry),
        probe_count(&registry),
        probe_names(&registry)
    ));
    assert!(
        substring_count(&derivation) > probe_count(&derivation),
        "🔴 the module that writes both counters quotes the needle and declares no probe, so the \
         old method has to be the higher number there. If it is not, the quotation moved and this \
         arm is measuring nothing: substring={} attributes={}",
        substring_count(&derivation),
        probe_count(&derivation)
    );
    assert_eq!(
        probe_count(&derivation),
        0,
        "🔴 the shared derivation declares no probes of its own"
    );
    assert_eq!(
        substring_count(&registry),
        probe_count(&registry),
        "🔴 `req/334` L-03's repair, stated mechanically: the registry file no longer contains the \
         needle it counts with, so it can no longer count itself. A divergence reappearing here \
         means a counter has been written back into the file it measures"
    );
    assert_eq!(
        probe_count(&registry),
        probe_names(&registry).len(),
        "🔴 the two methods disagree about the registry file, so neither is trusted"
    );
}
