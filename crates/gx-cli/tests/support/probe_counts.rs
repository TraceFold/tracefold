// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 6 (`req/338` §0-6, from `req/334` L-03)** — how many probes a suite file has,
//! derived **two ways that fail differently**.
//!
//! # The defect this module exists to close
//!
//! R27 built a registry so that a numeric claim on `docs/LIMITS.md` keeps being checked after the
//! page's anchor moves past it, and put a **bed control** above the registry — "without this the arm
//! below would hold the page to numbers that are themselves wrong, which is the failure one level up
//! from the one this file exists for".
//!
//! That one-level-up failure then happened. The count was
//! `text.matches("#[test]").count()` — the *characters* `#[test]`, wherever they appear, including
//! inside a string literal. Exactly one registered file contains that literal in a string: the
//! registry's own file, in the counter itself. So the counter counted itself, the registry
//! published `five` for a file with `four`, `docs/LIMITS.md` printed `five`, and the bed control
//! ratified all of it — because the bed control **was the same computation run a second time**.
//! `req/332` §10-2 even records the row being "corrected" from `four` to `five`: the correction
//! moved the number away from the truth, because it trusted the counter.
//!
//! Three layers agreeing is not three layers when all three stand on one computation.
//!
//! # The shape of the repair
//!
//! Two derivations that cannot fail the same way:
//!
//! * [`probe_count`] reads **attributes** — a test attribute alone at the start of a line. A quoted
//!   needle is not at the start of a line and is not alone on it.
//! * [`probe_names`] reads the **items those attributes introduce**. A quoted needle introduces no
//!   `fn`, so this walk cannot see one however it is spelled.
//!
//! The control requires them to agree, and `r28_probe_counter_discrimination.rs` fires both at a
//! text that quotes the needle to show that they can be made to disagree — a control that cannot be
//! made to fail by the thing it controls for is not a control.
//!
//! Shared rather than written twice, per `req/38` §234 ruling 3: an instrument calls the derivation
//! instead of carrying a copy of its predicate.

/// How many probes a suite has, counted from attributes at the start of a line.
///
/// `#[tokio::test]` counts too. No registered suite uses it today — so this changes no number —
/// and a suite converted to async tests would otherwise have its probes silently leave the census
/// rather than fail it.
#[must_use]
pub fn probe_count(text: &str) -> usize {
    text.lines()
        .map(str::trim_start)
        .filter(|l| *l == "#[test]" || *l == "#[tokio::test]")
        .count()
}

/// The names of the functions those attributes introduce — the second, independent method.
#[must_use]
pub fn probe_names(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut names = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t != "#[test]" && t != "#[tokio::test]" {
            continue;
        }
        // The `fn` item this attribute introduces, past any further attributes on the way.
        for candidate in lines.iter().skip(i + 1).take(6) {
            let c = candidate.trim_start();
            if let Some(rest) = c
                .strip_prefix("fn ")
                .or_else(|| c.strip_prefix("async fn "))
                .or_else(|| c.strip_prefix("pub fn "))
            {
                if let Some(name) = rest.split(['(', '<']).next() {
                    names.push(name.to_string());
                }
                break;
            }
        }
    }
    names
}

/// The **old** count, kept so that the divergence this repair is about stays visible in the record
/// rather than being tidied away.
#[must_use]
pub fn substring_count(text: &str) -> usize {
    text.matches("#[test]").count()
}
