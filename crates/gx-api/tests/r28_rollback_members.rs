// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R28 item 1 (`req/338` §0-1, from `req/334` M-01)** — the wire shape of the two roll-back
//! facts, and the asymmetry that makes their presence a signal.
//!
//! The twenty-seventh audit drove `POST /transformations/{id}/undo` against an adapter that refuses
//! the inverse's apply and read the delivered body: five members, keyed on the abort taxonomy, and
//! the word `rollback` in none of them. It then drove the harder variant — the inverse refused *and*
//! its roll-back refused too — and got a body identical in every member. Two terminal states, one
//! answer: **is the object back where it was, or is it half undone?**
//!
//! `crates/gx-cli/tests/r28_abort_answer_sweep.rs` holds that every road on every surface now asks Σ.
//! This file holds what the asking puts on the wire.
//!
//! # Why this is a unit test over `ApiError` and not a second HTTP bed
//!
//! `crates/gx-api/tests/wire_census.rs` is where this crate pins member sets, and it drives the
//! router. What is new here is not a route's behaviour but a **type's** wire shape, and the property
//! that matters — the members appear on the abort roads and on nothing else — is a property of
//! `ApiError::body`. Driving a whole server to read one object back would measure the same thing
//! through more machinery. The live HTTP evidence exists and is recorded in `req/349` §2: the audit's
//! own suite, re-driven against this tree, reads `"rollback":"Succeeded"` on the first variant and
//! `"rollback":"Failed"` on the second.

use gx_api::{ApiError, RollbackFacts};

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

/// The member names, **sorted**: this file is about which members exist, and the order a JSON
/// object serialises in is not a property 44 §2.3 fixes.
fn members(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

/// 🔴 The five keys 44 §2.3 fixes, on a refusal that is not about an abort.
///
/// The asymmetry is the signal, so it is asserted from the side that must **not** carry the
/// members — the same discipline `retry_after_ms` was landed under (DR-43-5 (3)).
#[test]
fn a_a_refusal_that_is_not_about_an_abort_carries_the_five_keys_and_no_more() {
    let plain = ApiError::not_found("no such transformation").body();
    record(&format!("R28_MEMBERS_PLAIN {:?}", members(&plain)));
    assert_eq!(
        members(&plain),
        vec!["detail", "gx_code", "status", "title", "type"],
        "🔴 the roll-back members must not appear on a refusal that is not about an abort, or \
         their presence stops being the signal that it is: {plain}"
    );
}

/// 🔴 **`req/334` M-01** — the two facts, as members, with the values Σ holds.
#[test]
fn b_an_abort_refusal_carries_both_facts_as_machine_members() {
    let succeeded = ApiError::new("APPLY_FAILED", "t", "d")
        .with_rollback_facts(RollbackFacts {
            rollback: Some("Succeeded"),
            not_attempted_because: None,
        })
        .body();
    let failed = ApiError::new("APPLY_FAILED", "t", "d")
        .with_rollback_facts(RollbackFacts {
            rollback: Some("Failed"),
            not_attempted_because: None,
        })
        .body();
    record(&format!(
        "R28_MEMBERS_ABORT members={:?} succeeded={} failed={}",
        members(&succeeded),
        succeeded["rollback"],
        failed["rollback"]
    ));
    assert_eq!(
        members(&succeeded),
        vec![
            "detail",
            "gx_code",
            "rollback",
            "rollback_not_attempted_because",
            "status",
            "title",
            "type"
        ],
        "🔴 both members are written together or not at all: {succeeded}"
    );
    assert_eq!(succeeded["rollback"], "Succeeded");
    assert_ne!(
        succeeded["rollback"], failed["rollback"],
        "🔴 this is the whole finding: the object being back where it was and the object being \
         half undone have to be two answers. They were one."
    );
}

/// 🔴 `null` is written, not omitted.
///
/// `req/38` §234 ruling 1 settled this one level down, on gx-cli: an **absent** member is
/// indistinguishable from a road that never had the fact, while a member carrying `null` is a fact a
/// reader can branch on. The cause is deliberately not a component of Σ, so `null` is the honest
/// answer for a row this process did not abort itself — and honest is not the same as missing.
#[test]
fn c_an_absent_cause_is_written_as_null_rather_than_left_out() {
    let body = ApiError::new("APPLY_FAILED", "t", "d")
        .with_rollback_facts(RollbackFacts {
            rollback: Some("NotAttempted"),
            not_attempted_because: None,
        })
        .body();
    let with_cause = ApiError::new("APPLY_FAILED", "t", "d")
        .with_rollback_facts(RollbackFacts {
            rollback: Some("NotAttempted"),
            not_attempted_because: Some("RecoveredWithoutRebuilding"),
        })
        .body();
    let nothing_in_question = ApiError::new("PRECONDITION_CHANGED", "t", "d")
        .with_rollback_facts(RollbackFacts {
            rollback: None,
            not_attempted_because: None,
        })
        .body();
    record(&format!(
        "R28_MEMBERS_NULLS absent_cause={} present_cause={} no_rollback_in_question={}",
        body["rollback_not_attempted_because"],
        with_cause["rollback_not_attempted_because"],
        nothing_in_question["rollback"]
    ));
    assert!(
        body.get("rollback_not_attempted_because").is_some(),
        "🔴 the member itself has to be on the object: {body}"
    );
    assert_eq!(
        body["rollback_not_attempted_because"],
        serde_json::Value::Null
    );
    assert_eq!(
        with_cause["rollback_not_attempted_because"], "RecoveredWithoutRebuilding",
        "🔴 and when this process did reach the abort, the cause it recorded is carried: \
         {with_cause}"
    );
    assert!(
        nothing_in_question.get("rollback").is_some()
            && nothing_in_question["rollback"] == serde_json::Value::Null,
        "🔴 an abort that never entered T-10c's guard has no roll-back to report, and says so with \
         a member rather than with silence: {nothing_in_question}"
    );
}
