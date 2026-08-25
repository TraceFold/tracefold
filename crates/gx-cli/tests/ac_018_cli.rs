// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-018, re-confirmed through the CLI** (51 §15 M6 row; sem: SEM-gx-cli-1015 / req/88 §7).
//!
//! AC-018 verbatim: "Given: a `VerdictReceipt` corresponding to each of `Verdict::Admit/Deny/Escalate`... When:
//! verified with no ledger access via an API equivalent to `gx-cli receipt verify <receipt.json> --offline`. Then: for all 3
//! kinds of verdict, signature verification and canonical CID-consistency checking both succeed (`Ok(true)`, `checks.inclusion` is
//! `"skipped"`)." (sem: SEM-gx-cli-1016) Judgment method `integration (3 cases)`, M2.
//!
//! M2 met it at the library API — `gx_witness::verify_offline` — because the CLI did not exist. The
//! AC's own text names the command it was waiting for, and this is that re-confirmation: the same
//! three cases, through `gx receipt verify --offline`, on a receipt read from a **file**.
//!
//! # 🔴 `checks.inclusion` reads `not_applicable` and not `"skipped"`
//!
//! AC-018 writes 44 §1.2's two-value vocabulary. This hand emits four (H5-9, M6H2-3), and
//! `not_applicable` is the value 44 spells `"skipped"` — `crates/gx-cli/src/receipt.rs`'s
//! `INCLUSION_JSON` carries the mapping so the rename is a documented translation rather than a
//! silent one. The assertion below states both halves so a reader of the AC finds the correspondence
//! here rather than reconstructing it.

mod support;

use gx_core::VerdictKind;
use support::{issue, keypair, run, scratch, verdict_payload, write_json, write_public_key};

/// AC-018's three cases, through the binary, with no ledger anywhere.
#[test]
fn ac_018_cli_all_three_verdict_receipts_verify_offline() {
    let dir = scratch("ac018_cli");
    let key = keypair(1);
    let key_path = write_public_key(&dir, &key);

    let mut results = Vec::new();
    for (n, kind) in [VerdictKind::Admit, VerdictKind::Deny, VerdictKind::Escalate]
        .into_iter()
        .enumerate()
    {
        let receipt = issue(&verdict_payload(kind, &key, 100 + n as u64), &key);
        let path = write_json(
            &dir.join(format!("receipt_{n}.json")),
            &serde_json::to_value(&receipt).expect("serialises"),
        );

        let out = run(support::gx()
            .arg("receipt")
            .arg("verify")
            .arg(&path)
            .arg("--offline")
            .arg("--key")
            .arg(&key_path));
        let json = out.json();
        println!("AC018_CLI {kind:?} exit={} {json}", out.code);

        assert_eq!(
            out.code, 0,
            "AC-018's \"`Ok(true)`\" is 44 §1.2's `0=valid` (sem: SEM-gx-cli-1017)"
        );
        assert_eq!(json["valid"], serde_json::json!(true));
        assert_eq!(json["checks"]["signature"], serde_json::json!(true));
        assert_eq!(
            json["checks"]["canonical_cid"],
            serde_json::json!(true),
            "AC-018's \"canonical CID-consistency checking succeeds\" (sem: SEM-gx-cli-1018)"
        );
        assert_eq!(
            json["checks"]["inclusion"],
            serde_json::json!("not_applicable"),
            "AC-018's \"`checks.inclusion` is `\"skipped\"`\" (sem: SEM-gx-cli-1019), in this hand's four-value vocabulary \
             (M6H2-3): a VerdictReceipt has nothing in the ledger to be in"
        );
        assert_eq!(json["anchor"], serde_json::json!("none"));
        results.push(out.code);
    }
    println!("AC018_CLI_CASES={} EXITS={results:?}", results.len());
    assert_eq!(
        results.len(),
        3,
        "34 asks for \"integration (3 cases)\" (sem: SEM-gx-cli-1020)"
    );
}

/// The negative control: a receipt verified against **another key** fails, and says which check.
///
/// Without this, the probe above measures "the command exits 0" rather than "the command checked a
/// signature" (sem: SEM-gx-cli-1021). `verify_offline` refuses a `key_id` that disagrees with the signature (42 §3.10), so
/// the refusal here is a schema one and the JSON says so — which is the distinction M4H4-2 asks for
/// between "not implemented" and "failure" (sem: SEM-gx-cli-1022), one layer up.
#[test]
fn ac_018_cli_a_receipt_does_not_verify_under_a_stranger() {
    let dir = scratch("ac018_cli_stranger");
    let key = keypair(2);
    let stranger = keypair(3);
    let key_path = write_public_key(&dir, &stranger);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 200), &key);
    let path = write_json(
        &dir.join("receipt.json"),
        &serde_json::to_value(&receipt).expect("serialises"),
    );

    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&path)
        .arg("--offline")
        .arg("--key")
        .arg(&key_path));
    let json = out.json();
    println!("AC018_CLI_STRANGER exit={} {json}", out.code);
    assert_eq!(out.code, 7, "44 §1.2: \"7=invalid\" (sem: SEM-gx-cli-1023)");
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(
        json["checks"]["canonical_cid"],
        serde_json::Value::Null,
        "nothing downstream of the signature ran, and `false` would claim it did"
    );
}
