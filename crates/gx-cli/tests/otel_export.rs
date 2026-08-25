// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-P3-15** — the exporter, its default, and the one property that matters most about it.
//!
//! `req/119` §6 / NFR-017. Three claims, and the third is the load-bearing one:
//!
//! 1. **off by default**, and the word "disabled" is printed rather than implied (R-114-4; sem: SEM-gx-cli-1305);
//! 2. a sink that was named receives OTLP/JSON spans, and what they carry is [`NOT_EXPORTED`]'s
//!    complement — no locator unless an operator asked for one;
//! 3. 🔴 **an exporter that cannot write changes nothing**. `gx wrap`'s live arm is in
//!    `tools/e2e_p3.sh`, where a real session writes real spans; here the same property is measured
//!    where it is cheapest to make deterministic — the call returns `()` and the run continues.

mod support;

use gx_cli::otel::{Exporter, NOT_EXPORTED};

/// The default is off, and it says so.
#[test]
fn ac_p3_15_the_exporter_is_off_by_default_and_says_the_word() {
    let exporter = Exporter::disabled();
    println!(
        "OTEL_STATUS={} enabled={}",
        exporter.status(),
        exporter.is_enabled()
    );
    assert!(!exporter.is_enabled());
    assert_eq!(
        exporter.status(),
        "disabled",
        "R-114-4's \"telemetry is explicitly noted as 0\" is only true if a start-up line can print the word (sem: SEM-gx-cli-1306)"
    );
    // And the same value is what `gx serve`'s start-up line carries — the two are one function, so
    // a hand that turned the exporter on by default would move both at once.
    let line = gx_cli::serve::start_line(
        "127.0.0.1:8787".parse().expect("an address"),
        &gx_cli::serve::ServeSpec {
            bind: None,
            record_only: false,
            fail_posture: None,
            tls: (None, None),
            token_file: None,
            signing_key: None,
        },
        "ed25519-test",
        // 🔴 **DR-43-2** — the start-up gate's own numbers ride in this line (`req/38` §148). This
        // suite is about the exporter, so it hands the shape a server with nothing to recover would
        // hand it, and asserts below that the field is carried rather than dropped.
        &serde_json::json!({"ledger_agrees": true, "recover": {"resumed": 0}}),
        // 🔴 **R19 / `req/279` H-02 (b)** — `gx serve` can now be wired to an MCP server, and the
        // line says which. This suite hands it the shape a server that named none produces
        // (`gx_cli::session::McpWiring::default().summary()`), and asserts below that the zero is
        // **printed** rather than left to be inferred — the same discipline `otel: "disabled"` is
        // held to two lines up.
        &gx_cli::session::McpWiring::default().summary(),
    );
    println!("SERVE_START={line}");
    assert_eq!(
        line["runtime"]["ledger_agrees"],
        serde_json::json!(true),
        "44 §1.2 asks for one start-up line and DR-43-2's gate reports inside it"
    );
    assert_eq!(line["otel"], "disabled");
    assert_eq!(
        line["mcp"]["server"],
        serde_json::Value::Null,
        "🔴 R19: a server that named no MCP server says so in the line, for the reason the \
         exporter says \"disabled\" — an absence a reader has to infer is an absence a reader gets \
         wrong. line was {line}"
    );
    assert!(
        line["verdict_checkpoints"]
            .as_str()
            .is_some_and(|s| s.contains("manual")),
        "ruling ⑥ (sem: SEM-gx-cli-1307): no timer runs, and a deployment that never posts publishes no counts: {line}"
    );
}

/// A named sink receives OTLP/JSON, and the locator is elided unless asked for.
#[test]
fn a_named_sink_receives_otlp_json_with_the_locator_elided() {
    let dir = support::scratch("otel_sink");
    let path = dir.join("spans.jsonl");
    let exporter = Exporter::to_file(&path, false);
    assert!(exporter.is_enabled());
    assert_eq!(exporter.status(), format!("file:{}", path.display()));

    let locator = "stdio:///usr/local/bin/notes-mcp#file:///srv/private/customer-notes.md";
    exporter.span(
        "verify",
        1_754_000_000_000_000_000,
        1_754_000_000_100_000_000,
        &serde_json::json!({
            "substrate": "mcp",
            "verdict_kind": "Admit",
            "enforced": true,
            "locator": exporter.locator_attribute(locator),
        }),
    );

    let written = std::fs::read_to_string(&path).expect("the sink was written");
    println!("SPAN={}", written.trim());
    let document: serde_json::Value =
        serde_json::from_str(written.trim()).expect("one span per line, and each line is JSON");
    let span = &document["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
    assert_eq!(span["name"], "verify");
    assert_eq!(
        span["startTimeUnixNano"], "1754000000000000000",
        "OTLP/JSON carries nanosecond timestamps as strings"
    );
    let attributes = span["attributes"].as_array().expect("attributes");
    assert!(attributes
        .iter()
        .any(|a| a["key"] == "verdict_kind" && a["value"]["stringValue"] == "Admit"));

    assert!(
        !written.contains("customer-notes.md"),
        "🔴 the resource URI names a customer's file and must not be in a span by default: {written}"
    );
    assert!(
        written.contains("<elided"),
        "and the elision is visible rather than a missing field: {written}"
    );

    // Opting in is a different exporter, and it does carry the locator — so the default is a choice
    // an operator can reverse, which is what makes it a default rather than a limitation.
    let opted_in = Exporter::to_file(&dir.join("spans-locators.jsonl"), true);
    assert_eq!(opted_in.locator_attribute(locator), locator);
}

/// 🔴 The one that matters: a sink that cannot be written changes nothing.
///
/// A directory is used as the sink path — `OpenOptions::append` on one fails on every platform this
/// runs on. The call returns `()`, so there is no branch a pipeline could take on it: Rule 1 (sem: SEM-gx-cli-1308) in a
/// signature rather than in a sentence.
#[test]
fn ac_p3_15_an_exporter_that_cannot_write_changes_nothing() {
    let dir = support::scratch("otel_unwritable");
    let exporter = Exporter::to_file(&dir, false);
    // Returns `()`. If this line panicked or propagated, an operator's telemetry misconfiguration
    // would become a refused change.
    exporter.span("commit", 1, 2, &serde_json::json!({ "substrate": "mcp" }));
    println!("UNWRITABLE_SINK_OK status={}", exporter.status());
    assert!(dir.is_dir(), "and nothing was written where a directory is");
}

/// An OTLP endpoint is refused **by name** rather than written to a file nobody asked for.
#[test]
fn an_http_endpoint_is_refused_with_the_version_that_will_have_one() {
    let refused = Exporter::from_flag(Some("http://127.0.0.1:4318/v1/traces"), false)
        .expect_err("this build ships no network client for telemetry");
    let detail = refused.to_string();
    println!("REFUSED={detail}");
    assert!(detail.contains("v0.2"), "{detail}");
    assert!(detail.contains("NFR-019"), "{detail}");
}

/// The non-export list is a list, and every entry says what it is protecting.
#[test]
fn what_is_not_exported_is_written_down() {
    println!("NOT_EXPORTED={NOT_EXPORTED:#?}");
    assert_eq!(NOT_EXPORTED.len(), 6);
    for entry in NOT_EXPORTED {
        assert!(
            entry.contains('—') || entry.contains(','),
            "each entry names the thing and the reason: {entry}"
        );
    }
}
