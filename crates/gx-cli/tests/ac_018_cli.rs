//! **AC-018, re-confirmed through the CLI** (51 §15 M6 行 / req/88 §7).
//!
//! AC-018 逐語: 「Given: `Verdict::Admit/Deny/Escalate`それぞれに対応する`VerdictReceipt`…When:
//! `gx-cli receipt verify <receipt.json> --offline`相当のAPIで台帳アクセスなしに検証。Then: 3種
//! すべてのverdictで署名検証・canonical CID整合検査が成功する（`Ok(true)`、`checks.inclusion`は
//! `"skipped"`）。」判定方法 `integration（3ケース）`, M2.
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
            "AC-018's 「`Ok(true)`」 is 44 §1.2's `0=valid`"
        );
        assert_eq!(json["valid"], serde_json::json!(true));
        assert_eq!(json["checks"]["signature"], serde_json::json!(true));
        assert_eq!(
            json["checks"]["canonical_cid"],
            serde_json::json!(true),
            "AC-018's 「canonical CID整合検査が成功する」"
        );
        assert_eq!(
            json["checks"]["inclusion"],
            serde_json::json!("not_applicable"),
            "AC-018's 「`checks.inclusion`は`\"skipped\"`」, in this hand's four-value vocabulary \
             (M6H2-3): a VerdictReceipt has nothing in the ledger to be in"
        );
        assert_eq!(json["anchor"], serde_json::json!("none"));
        results.push(out.code);
    }
    println!("AC018_CLI_CASES={} EXITS={results:?}", results.len());
    assert_eq!(results.len(), 3, "34 asks for 「integration（3ケース）」");
}

/// The negative control: a receipt verified against **another key** fails, and says which check.
///
/// Without this, the probe above measures 「the command exits 0」 rather than 「the command checked a
/// signature」. `verify_offline` refuses a `key_id` that disagrees with the signature (42 §3.10), so
/// the refusal here is a schema one and the JSON says so — which is the distinction M4H4-2 asks for
/// between 「未実装」 and 「失敗」, one layer up.
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
    assert_eq!(out.code, 7, "44 §1.2: 「7=無効」");
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(
        json["checks"]["canonical_cid"],
        serde_json::Value::Null,
        "nothing downstream of the signature ran, and `false` would claim it did"
    );
}
