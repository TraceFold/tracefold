//! 🔴 **M6-16 採(a)** — `gx receipt show --level 1..4`, and **M6-22 採(b)** at level 4.
//!
//! req/88 §6.2 手 2's DoD names both: 「段階開示 `--level 1..4`(M6-16)」 and, through §47's ruling on
//! M6-22, 「L4(生署名)出力が `signature_for` の消費者」. What has to be measured is not that four
//! levels exist but that they are **nested and different**: a `--level` that returned the same
//! object four times would satisfy a test that only asked for `Ok`.
//!
//! The fourth probe is `checks.inclusion`'s four values (§5 行 4 / H5-9), which is a claim about the
//! *vocabulary* rather than about a run.

mod support;

use gx_cli::receipt::{self, ReceiptStore, StoredKind, INCLUSION_JSON, MAX_LEVEL};
use gx_core::VerdictKind;
use gx_witness::receipt::InclusionCheck;
use support::{commit_receipt, issue, keypair, project, tid, verdict_payload};

/// 🔴 The four levels are nested: every field of level `n-1` is in level `n`, and each adds.
///
/// Nesting is what makes 「段階開示」 a disclosure rather than four unrelated reports. 48 §3.1's
/// layers are a ladder — 「L1=verdict バッジ / L2=Receipt 要約 / L3=全展開 / L4=独立検証結果」 — and a
/// reader who climbed one rung should not have to re-read what they already had.
#[test]
fn the_four_levels_are_nested_and_each_one_adds_something() {
    let key = keypair(1);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 5), &key);

    let mut previous: Vec<String> = Vec::new();
    let mut widths = Vec::new();
    for level in 1..=MAX_LEVEL {
        let json = receipt::disclose(&receipt, level).expect("the fixture decodes");
        let object = json.as_object().expect("an object");
        let mut keys: Vec<String> = object.keys().cloned().collect();
        keys.sort();
        for held in &previous {
            assert!(
                keys.contains(held),
                "level {level} dropped `{held}`, which level {} showed; a disclosure that takes \
                 fields away as it discloses is not a ladder",
                level - 1
            );
        }
        assert!(
            keys.len() > previous.len(),
            "level {level} added nothing to level {}: {keys:?}",
            level - 1
        );
        widths.push(keys.len());
        previous = keys;
    }
    println!("DISCLOSURE_FIELDS_PER_LEVEL={widths:?}");
    assert_eq!(widths.len(), 4);
}

/// Level 1 is the badge, and it says the two things a badge is for.
///
/// 「verdict バッジ」 answers 「was this admitted, and was it enforced」. `verdict` is an `Option`
/// because of **E-M5-11**: under 43 T-4e the gate was never called, so `null` is the true answer and
/// an empty proof would have been an invention (M4H4-2).
#[test]
fn level_one_is_the_verdict_badge() {
    let key = keypair(2);
    for kind in [VerdictKind::Admit, VerdictKind::Deny, VerdictKind::Escalate] {
        let receipt = issue(&verdict_payload(kind, &key, 6), &key);
        let json = receipt::disclose(&receipt, 1).expect("decodes");
        println!("L1 {kind:?} -> {json}");
        assert_eq!(json["verdict"], serde_json::to_value(kind).expect("value"));
        assert!(json["transformation"]
            .as_str()
            .expect("text")
            .starts_with("gx1:"));
        assert!(json["enforced"].is_boolean());
        assert!(
            json.get("payload").is_none(),
            "level 1 does not expand the payload"
        );
    }
}

/// 🔴 **M6-22 採(b)** — level 4 carries the raw signature, and it is `signature_for`'s answer.
///
/// The accessor §46 M5FIX-3 left as gx-witness's one survivor. What makes this the *consumer* and
/// not merely a place the bytes appear is that the signature is fetched **by the id the payload
/// declares** — 42 §3.10 requires the payload's `key_id` and the envelope's `keyid` to agree, so
/// 「the signature this receipt says signed it」 is a lookup and not a position in a `Vec`.
#[test]
fn level_four_is_the_raw_signature_the_payload_names() {
    let key = keypair(3);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 7), &key);
    let json = receipt::disclose(&receipt, MAX_LEVEL).expect("decodes");

    let signature = &json["signature"];
    println!(
        "L4_KEYID={} L4_SIG_BYTES={} ENVELOPE_SIGNATURES={}",
        signature["keyid"], signature["sig_bytes"], json["signatures_in_envelope"]
    );
    assert_eq!(signature["keyid"], serde_json::json!(key.key_id()));
    assert_eq!(
        signature["sig_bytes"],
        serde_json::json!(64),
        "Ed25519 signatures are 64 bytes"
    );
    assert!(
        signature["sig_b64"].as_str().expect("base64").len() > 80,
        "the raw signature is disclosed, not summarised"
    );

    // The negative half: an envelope holding only *somebody else's* signature answers `null` rather
    // than handing over the first one it has. A `signatures[0]` implementation would pass the
    // assertions above and fail this one.
    let mut foreign = receipt.clone();
    foreign.envelope.signatures[0].keyid = "key-somebody-else".to_string();
    let json = receipt::disclose(&foreign, MAX_LEVEL).expect("decodes");
    println!("L4_FOREIGN_SIGNATURE={}", json["signature"]);
    assert!(
        json["signature"].is_null(),
        "`signature_for` answers about the key the payload names, and this envelope has none"
    );
    assert_eq!(json["signatures_in_envelope"], serde_json::json!(1));
}

/// 🔴 `checks.inclusion` has **four** values where 44 §1.2 writes two (§5 行 4 / H5-9, M6H2-3).
///
/// H5-9's clause is 「**Unanchored を pass にしない**」, and folding four into 44's `bool|"skipped"`
/// would put 「the ledger claim was not checked」 under the same face as 「it was checked and held」.
/// Distinctness is asserted, not merely the count: four names that collided would be the fold with
/// extra steps.
#[test]
fn the_inclusion_vocabulary_is_four_distinct_values() {
    let mut spellings: Vec<&str> = INCLUSION_JSON.iter().map(|(s, _)| *s).collect();
    spellings.sort_unstable();
    spellings.dedup();
    println!("INCLUSION_VALUES={INCLUSION_JSON:?}");
    assert_eq!(
        spellings.len(),
        4,
        "four distinct spellings for `InclusionCheck`'s four variants"
    );
    assert_eq!(
        receipt::inclusion_json(InclusionCheck::NotApplicable),
        "not_applicable"
    );
    assert_eq!(
        receipt::inclusion_json(InclusionCheck::Verified),
        "verified"
    );
    assert_eq!(receipt::inclusion_json(InclusionCheck::Refuted), "refuted");
    assert_eq!(
        receipt::inclusion_json(InclusionCheck::Unanchored),
        "unanchored"
    );
    // 🔴 The two 44 §1.2 has no word for. The table carries that fact so M6H2-3 is readable from
    // the source rather than only from the report.
    let unnamed = INCLUSION_JSON
        .iter()
        .filter(|(_, spec)| spec.starts_with("no word"))
        .count();
    println!("INCLUSION_VALUES_44_CANNOT_SPELL={unnamed}");
    assert_eq!(unnamed, 2, "`refuted` and `unanchored`");
}

/// 🔴 The store round-trips, and a missing receipt is 44 §1.2's `6=未検出` rather than a failure.
///
/// `.gx/receipts/` is **M6H2-1**, this hand's own addition, and the two answers it has to keep apart
/// are 「there is no receipt」 and 「there is a file and it is not one」 (E-M4-35).
#[test]
fn the_receipt_store_round_trips_and_a_missing_one_is_not_found() {
    let (_dir, layout) = project("receipt_store");
    let store = ReceiptStore::in_layout(&layout);
    let key = keypair(4);
    let (receipt, _head, _log) = commit_receipt(&key, 8, 3);
    let id = tid(8);

    // 🔴 **M6H4-7** (req/38 §51 採(a)): the key is `(TransformationId, kind)` and no longer the
    // transformation alone. One transformation issues up to three receipts — ASM-14's verdict
    // receipt, 43 T-5's ruling and 43 T-11's commit — and under the old name they shared one slot,
    // so the last writer won and 「who allowed this」 could erase 「what was decided」.
    let path = store.put(&id, StoredKind::Commit, &receipt).expect("put");
    println!("RECEIPT_STORED_AT={}", path.display());
    assert!(
        path.to_string_lossy().ends_with(".commit.json"),
        "M6H4-7: `<TID>.<kind>.json`, kind in {{verdict, ruling, commit}}; got {}",
        path.display()
    );
    let back = store
        .get(&id, StoredKind::Commit)
        .expect("get")
        .expect("it is there");
    assert_eq!(back, receipt, "the JSON face round-trips byte for byte");
    assert!(
        store.get(&id, StoredKind::Verdict).expect("get").is_none(),
        "and the other two kinds are separate slots rather than aliases of this one"
    );
    // `first_available` is what `show` uses: the commit receipt is preferred because it is the only
    // kind carrying an `inclusion_proof`, and the kind it found is reported rather than assumed.
    let (kind, found) = store
        .first_available(&id)
        .expect("first_available")
        .expect("one is there");
    println!("FIRST_AVAILABLE_KIND={}", kind.tag());
    assert_eq!(kind, StoredKind::Commit);
    assert_eq!(found, receipt);

    let outcome = receipt::show(&store, &tid(999), 1).expect("show answers");
    println!("SHOW_MISSING_CODE={} JSON={}", outcome.code, outcome.json);
    assert_eq!(outcome.code, 6, "44 §1.2: 「exit: 0=存在, 6=未検出」");
    assert_eq!(outcome.json["found"], serde_json::json!(false));

    // 「在るが壊れている」 is a third answer.
    std::fs::write(store.path_of(&tid(1234), StoredKind::Commit), b"{not json").expect("write");
    let err = store
        .get(&tid(1234), StoredKind::Commit)
        .expect_err("a broken file is not a missing one");
    println!("SHOW_CORRUPT={err}");
    assert!(matches!(err, gx_cli::Error::Malformed { .. }));
}

/// `--level 0` and `--level 5` are 「入力不正」, and 規律52 sends that to 1 rather than to 2.
#[test]
fn a_level_outside_the_ladder_is_a_usage_error() {
    let (_dir, layout) = project("receipt_level_range");
    let store = ReceiptStore::in_layout(&layout);
    for level in [0u8, 5, 255] {
        let err = receipt::show(&store, &tid(1), level).expect_err("outside 1..=4");
        println!("LEVEL_{level}_EXIT={} {err}", err.exit_code());
        assert!(matches!(err, gx_cli::Error::Usage { .. }));
        assert_eq!(
            err.exit_code(),
            1,
            "規律52: 「入力不正」 is 1; 2 is the state machine's 「拒否」"
        );
    }
}
