// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **NFR-017** — the OpenTelemetry export, at the size v0.1 can be honest about (`req/119` §6).
//!
//! NFR-017 verbatim: "each transition of `gx-engine`'s state machine (see 43) is emitted as
//! OpenTelemetry traces/metrics" (sem: SEM-gx-cli-060). What ships here is **less than that sentence**, and the difference is written down rather
//! than left for a reader to discover from an empty collector:
//!
//! | asked for | shipped | why |
//! |---|---|---|
//! | traces to a collector | OTLP/JSON **spans appended to a file** | an OTLP client is a network dependency, and NFR-019 is "a single binary with zero dynamic dependencies" (sem: SEM-gx-cli-061). A file sink is a thing an operator can `tail`, feed to a collector, or diff. The HTTP/gRPC exporter is v0.2 and is refused **by name** if an endpoint is given |
//! | every transition of `gx-engine` | the transitions **a surface drives** (`submit`/`plan`/`verify`/`commit`) | the exporter lives in gx-cli, so a transition the engine makes without a surface asking is not seen. In v0.1 there is no such transition, and the day there is (a scheduler, an expiry sweep) this file will be silent about it |
//! | metrics | **none** | `VerdictTally` is already published as a signed artefact (FR-M04). A second, unsigned copy over telemetry would be a number an auditor could not check |
//!
//! # 🔴 Default **off**, and that is R-114-4 rather than caution
//!
//! req/114 §3 R-114-4: "no onboarding instrumentation -- **state zero telemetry explicitly**
//! (honesty, productized)" (sem: SEM-gx-cli-062). An exporter
//! that was on by default would make that sentence false the moment a binary shipped. So the sink is
//! `None` until an invocation names a path, and every start-up line prints [`Exporter::status`] —
//! "otel: disabled" said out loud, because "silently absent" (sem: SEM-gx-cli-063) is the thing this project does not do.
//!
//! # 🔴 What is **not** exported, as a list rather than as an omission
//!
//! [`NOT_EXPORTED`]. The short form: nothing that carries a customer's content or a key. The locator
//! is the one that looks safe and is not — a resource URI names a customer's file, so what travels
//! is its scheme and a byte count unless an operator opts in.
//!
//! # Rule 1: an exporter has no authority (sem: SEM-gx-cli-064)
//!
//! [`Exporter::span`] returns nothing and cannot fail upward. A sink that will not take a span
//! leaves a note on stderr and the pipeline's decision is unchanged — which is AC-P3-15's second
//! half and is a property of this signature rather than of a promise.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// The things this exporter refuses to put in a span, and why (`req/119` §6).
pub const NOT_EXPORTED: [&str; 6] = [
    "delta payload bytes — the change itself, which is the customer's content",
    "intent goal bodies — what an agent asked for, in its own words",
    "evidence bodies — 42 §3.7's payloads",
    "receipt and signature bytes — a receipt is published deliberately, not leaked through telemetry",
    "key material of any kind, including the private half nothing here ever holds",
    "raw locators — a resource URI names a customer's file, so a span carries its digest unless \
     `--otel-locators` is given",
];

/// Where spans go, or nowhere.
#[derive(Clone, Debug, Default)]
pub struct Exporter {
    sink: Option<PathBuf>,
    locators: bool,
}

impl Exporter {
    /// The default: **off**.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Append OTLP/JSON spans to a file.
    #[must_use]
    pub fn to_file(path: &Path, locators: bool) -> Self {
        Self {
            sink: Some(path.to_path_buf()),
            locators,
        }
    }

    /// Build one from `--otel-file`, refusing an endpoint this build does not have.
    ///
    /// # Errors
    /// [`crate::Error::Usage`] for a `http://` or `grpc://` destination: v0.2's exporter, refused by
    /// name rather than silently written to a file the operator did not ask for.
    pub fn from_flag(destination: Option<&str>, locators: bool) -> crate::Result<Self> {
        let Some(destination) = destination else {
            return Ok(Self::disabled());
        };
        if destination.starts_with("http://")
            || destination.starts_with("https://")
            || destination.starts_with("grpc://")
        {
            return Err(crate::Error::Usage {
                detail: format!(
                    "--otel-file takes a path and got {destination:?}. An OTLP exporter over \
                     HTTP/gRPC is v0.2 (req/119 §9): NFR-019 asks for a single binary with no \
                     dynamic dependency, and this build ships no network client for telemetry. \
                     Write the spans to a file and let a collector read them"
                ),
            });
        }
        Ok(Self::to_file(Path::new(destination), locators))
    }

    /// Whether anything is exported at all.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.sink.is_some()
    }

    /// The word a start-up line prints. **Never empty**: "disabled" is said, not implied. (sem: SEM-gx-cli-065)
    #[must_use]
    pub fn status(&self) -> String {
        match &self.sink {
            None => "disabled".to_string(),
            Some(path) => format!("file:{}", path.display()),
        }
    }

    /// A locator, as this exporter is willing to carry it.
    ///
    /// 🔴 The elided form is the **scheme and a length**, and deliberately not a digest. A digest
    /// would be the more useful value and this crate may not compute one: 41 §6 gives the canonical
    /// encode one door and gx-cli's own manifest refuses `gx-canon` for that reason — "a CLI that
    /// could mint a `Cid` could name a thing without the engine" (sem: SEM-gx-cli-066) (Rule 1 (i)). A telemetry exporter is
    /// exactly the place that argument would be quietly abandoned, so it is not.
    ///
    /// What a reader gets instead is enough to correlate spans of one session (the scheme names the
    /// server) and nothing that identifies a customer's file. The identity a reader needs for real
    /// work is the `transformation` attribute, which the engine minted and this only reads.
    #[must_use]
    pub fn locator_attribute(&self, locator: &str) -> Value {
        if self.locators {
            return json!(locator);
        }
        let scheme = locator
            .split_once("://")
            .map_or("<no scheme>", |(scheme, _)| scheme);
        json!(format!("<elided: {scheme}, {} bytes>", locator.len()))
    }

    /// One span, appended.
    ///
    /// 🔴 Returns `()`. A sink that will not take a span is a note on stderr and nothing else: the
    /// pipeline's judgement does not depend on whether anybody is watching (Rule 1; sem: SEM-gx-cli-067), and AC-P3-15
    /// drives an unwritable sink to measure exactly that.
    pub fn span(&self, name: &str, start_unix_nanos: i64, end_unix_nanos: i64, attrs: &Value) {
        let Some(path) = &self.sink else {
            return;
        };
        let document = json!({
            // OTLP/JSON's shape (opentelemetry-proto's `ExportTraceServiceRequest`), one span per
            // line so that a file is appendable and readable by `tail -f`.
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        { "key": "service.name", "value": { "stringValue": "gx" } },
                        { "key": "service.version", "value": { "stringValue": env!("CARGO_PKG_VERSION") } },
                    ],
                },
                "scopeSpans": [{
                    "scope": { "name": "gx-cli" },
                    "spans": [{
                        "name": name,
                        "startTimeUnixNano": start_unix_nanos.to_string(),
                        "endTimeUnixNano": end_unix_nanos.to_string(),
                        "attributes": attributes(attrs),
                    }],
                }],
            }],
        });
        let line = match serde_json::to_string(&document) {
            Ok(line) => line,
            Err(e) => {
                crate::note!("gx: a span had no JSON form and was dropped: {e}");
                return;
            }
        };
        let opened = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path);
        match opened {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "{line}") {
                    crate::note!("gx: a span could not be written to {}: {e}", path.display());
                }
            }
            Err(e) => crate::note!(
                "gx: the otel sink {} could not be opened: {e} — the run continues, because an \
                 exporter has no say in what this binary decides",
                path.display()
            ),
        }
    }
}

/// A flat JSON object as OTLP's key/value list.
fn attributes(attrs: &Value) -> Vec<Value> {
    let Some(object) = attrs.as_object() else {
        return Vec::new();
    };
    object
        .iter()
        .map(|(key, value)| {
            let typed = match value {
                Value::String(s) => json!({ "stringValue": s }),
                Value::Bool(b) => json!({ "boolValue": b }),
                Value::Number(n) if n.is_i64() => json!({ "intValue": n.as_i64() }),
                other => json!({ "stringValue": other.to_string() }),
            };
            json!({ "key": key, "value": typed })
        })
        .collect()
}
