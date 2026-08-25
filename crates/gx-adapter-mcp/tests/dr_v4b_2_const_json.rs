// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-V4B-2** (`req/38` §123 ruling 2, landed v0.4-l `req/189`) — a typed constant
//! (`const_json`) beside the string `const`, and the `$server` metadata slot beside the tool
//! declarations. Both backward compatible; both measured here rather than asserted in a doc.
//!
//! What was missing (`req/183` §7 DR-V4B-2): `Const(String)` could not declare a boolean argument,
//! so the second half of Notion's trash round trip (`patch-page {in_trash: false}`) had no spelling
//! and the lane declared the DELETE tool instead. And a catalogue file had no place to say which
//! server it was written for.

use gx_adapter_mcp::{ArgSource, Catalogue, RestoreTemplate, SERVER_METADATA_KEY};

/// The Notion trash round trip's second half, now declarable: `in_trash: false` is a boolean.
#[test]
fn const_json_declares_a_boolean_and_const_stays_a_string() {
    let template = RestoreTemplate::new()
        .with("page_id", ArgSource::Forward("page_id".to_string()))
        .with("in_trash", ArgSource::ConstJson(serde_json::json!(false)))
        .with("note", ArgSource::Const("gx undo".to_string()))
        .with("depth", ArgSource::ConstJson(serde_json::json!(3)))
        .with(
            "properties",
            ArgSource::ConstJson(serde_json::json!({ "archived": false })),
        );
    let resolved = template
        .resolve(br#"{"page_id": "abc", "in_trash": true}"#, b"")
        .expect("every member resolves before apply");
    let value: serde_json::Value = serde_json::from_slice(&resolved).expect("JSON");
    println!("DR_V4B_2 resolved={value}");
    assert_eq!(value["page_id"], serde_json::json!("abc"));
    assert_eq!(
        value["in_trash"],
        serde_json::json!(false),
        "a boolean constant is a JSON boolean on the wire, not the string \"false\""
    );
    assert_eq!(
        value["note"],
        serde_json::json!("gx undo"),
        "`const` is unchanged: a string"
    );
    assert_eq!(value["depth"], serde_json::json!(3));
    assert_eq!(
        value["properties"],
        serde_json::json!({ "archived": false })
    );
}

/// The JSON spelling of a catalogue file: `{"const_json": <any>}` beside `{"const": "..."}`, and
/// the two are distinct tags — a `const` is always a string, a `const_json` is whatever was written.
#[test]
fn a_catalogue_file_reads_const_json_and_keeps_const_as_before() {
    let file = br#"{
      "$server": { "name": "notion-mcp-server", "version": "1.x", "note": "trash round trip" },
      "notion:post-page": {
        "restored_by": "notion:patch-page",
        "arguments": {
          "page_id": { "do_result": "/id" },
          "in_trash": { "const_json": true }
        }
      },
      "notion:patch-page": {
        "restored_by": "notion:patch-page",
        "arguments": {
          "page_id": { "forward": "page_id" },
          "in_trash": { "const_json": false },
          "reason": { "const": "gx undo of a trash" }
        }
      }
    }"#;
    let catalogue = Catalogue::from_json(file).expect("the format reads");
    println!(
        "DR_V4B_2 declared={} server={:?}",
        catalogue.declared(),
        catalogue.server()
    );
    assert_eq!(
        catalogue.declared(),
        2,
        "`$server` is metadata, not a third tool"
    );
    assert_eq!(
        catalogue.server().and_then(|s| s["name"].as_str()),
        Some("notion-mcp-server"),
        "DR-V4B-2b: the server pin is carried verbatim"
    );
    let spec = catalogue.spec_for("notion:patch-page").expect("declared");
    let template = spec.template().expect("templated");
    assert_eq!(
        template.arguments().get("in_trash"),
        Some(&ArgSource::ConstJson(serde_json::json!(false)))
    );
    assert_eq!(
        template.arguments().get("reason"),
        Some(&ArgSource::Const("gx undo of a trash".to_string()))
    );
    // Round trip: the value serialises back under the same tag.
    let json = serde_json::to_value(template).expect("serialises");
    assert_eq!(json["in_trash"], serde_json::json!({ "const_json": false }));
    assert_eq!(
        json["reason"],
        serde_json::json!({ "const": "gx undo of a trash" })
    );
}

/// `deny_unknown_fields` on a declaration is intact: `$server` is a top-level *key*, and a misspelt
/// member inside a declaration is still refused at parse time — the two do not trade off.
#[test]
fn the_server_slot_does_not_loosen_the_declarations() {
    let misspelt = br#"{
      "$server": { "name": "x" },
      "tool": { "restored_by": "y", "argument": {} }
    }"#;
    let refusal = Catalogue::from_json(misspelt).expect_err("`argument` is not `arguments`");
    println!("DR_V4B_2 refused={refusal}");
    assert!(
        refusal.contains("tool"),
        "the refusal names the entry: {refusal}"
    );

    // And a file without the slot reads exactly as before (every shipped catalogue).
    let old = br#"{ "notes.write": { "restored_by": "notes.restore" } }"#;
    let catalogue = Catalogue::from_json(old).expect("the v0.1 form still reads");
    assert_eq!(catalogue.declared(), 1);
    assert_eq!(catalogue.server(), None);
    assert_eq!(SERVER_METADATA_KEY, "$server");
    // Built in code, the slot is a builder.
    let pinned = Catalogue::new().with_server(serde_json::json!({ "name": "probe" }));
    assert_eq!(
        pinned.server().and_then(|s| s["name"].as_str()),
        Some("probe")
    );
}
