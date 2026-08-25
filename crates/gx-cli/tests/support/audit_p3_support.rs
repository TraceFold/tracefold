// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AUDIT LANE fixture (AC-P3-14, independent of `req/120`'s implementation lane) for the
//! `gx-cli`-level P3 probes: A-2 (fail posture), A-3 (concurrency/footprint) and A-6 (record-only).
//!
//! Not reached through `tests/support/mod.rs` (an existing file this lane does not edit — surgical
//! change discipline). Each audit file that wants it says so itself:
//!
//! ```ignore
//! #[path = "support/audit_p3_support.rs"]
//! mod audit_p3_support;
//! ```
//!
//! # Why this is a second fixture rather than a reuse of `gx-mcp-wire`'s
//!
//! `gx-mcp-wire/tests/support/mod.rs` is a **different crate's** `tests/` directory, and Rust does
//! not let one crate's integration tests `mod` another crate's. `gx-cli` already depends on
//! `gx-engine`, `gx-gate`, `gx-witness`, `gx-adapter-mcp` and `gx-mcp-wire` as **ordinary**
//! dependencies (`crates/gx-cli/Cargo.toml`), which is what lets this file build the same three types
//! `gx-cli::wrap` composes without adding anything to a manifest.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use gx_mcp_wire::StdioClient;

/// The probe server binary `tools/e2e_p3.sh` already builds
/// (`cargo build -p gx-mcp-wire --features probe-bin --bin mcp_probe_server`).
///
/// 🔴 `CARGO_BIN_EXE_mcp_probe_server` is **not** visible here: Cargo sets that variable only for the
/// crate whose own manifest declares the `[[bin]]` — `gx-mcp-wire`'s self-referential dev-dependency
/// (`gx-mcp-wire = { path = ".", features = ["probe-bin"] }`) is what turns it on for *that* crate's
/// tests, and `gx-cli`'s manifest is not edited to add the same trick (surgical-change discipline).
/// So the path is derived from the workspace layout, and a missing binary panics with the exact
/// command to run rather than failing on a mysterious "no such file".
pub fn probe_server_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // .../crates/gx-cli
    let workspace_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/gx-cli sits two levels under the workspace root")
        .to_path_buf();
    let name = if cfg!(windows) {
        "mcp_probe_server.exe"
    } else {
        "mcp_probe_server"
    };
    let candidate = workspace_root.join("target").join("debug").join(name);
    assert!(
        candidate.exists(),
        "AUDIT fixture: {candidate:?} is missing. Build it first: `cargo build -p gx-mcp-wire \
         --features probe-bin --bin mcp_probe_server` (tools/audit_p3_run.sh's stage 1 does this \
         before any of this crate's audit_p3_* tests run)."
    );
    candidate
}

/// A running probe server, real process, real pipe — the same fixture shape `gx-mcp-wire`'s own
/// `tests/support/mod.rs` uses, reimplemented here rather than imported (see the module header).
pub struct AuditProbe {
    pub client: Arc<StdioClient>,
    pub endpoint: String,
    pub dir: PathBuf,
    pub log: PathBuf,
}

impl Drop for AuditProbe {
    fn drop(&mut self) {
        let _ = self.client.shutdown();
    }
}

impl AuditProbe {
    /// A `file://` resource inside this probe's own scratch directory, and the path behind it.
    pub fn resource(&self, name: &str) -> (String, PathBuf) {
        let path = self.dir.join(name);
        (format!("file://{}", path.display()), path)
    }

    /// The server's **own** count of arrivals (A-7: never this file's opinion about itself).
    pub fn arrivals(&self) -> usize {
        std::fs::read_to_string(&self.log)
            .map(|text| text.lines().filter(|line| !line.is_empty()).count())
            .unwrap_or(0)
    }
}

/// Start the probe server under a clean scratch directory named `name`, under
/// `CARGO_TARGET_TMPDIR` — the same convention `gx-mcp-wire`'s fixture uses, and not `/tmp` directly
/// (discipline 43: idle cleanup on a raw `/tmp` would remove a running fixture's directory mid-suite; sem: SEM-gx-cli-2200).
pub fn spawn_probe(name: &str) -> AuditProbe {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("audit_p3");
    let dir = base.join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    let log = dir.join("arrivals.log");
    let command = probe_server_path();
    let command_str = command.display().to_string();
    let client = Arc::new(
        StdioClient::spawn_with_env(
            &command_str,
            &[],
            &[("GX_PROBE_LOG".to_string(), log.display().to_string())],
        )
        .expect("the probe server starts"),
    );
    client.initialize().expect("the handshake agrees");
    let endpoint = gx_mcp_wire::stdio_endpoint(&command_str);
    AuditProbe {
        client,
        endpoint,
        dir,
        log,
    }
}

/// A **fresh** journal path this engine can open, under the same scratch base as its probe.
///
/// 🔴 Cleared on every call rather than merely created: `gx-engine`'s journal is append-only and
/// `Engine::open` re-reads whatever is already there, so a second run of the same test against a
/// name left over from a previous run would resume a transformation already in a terminal state
/// (`submit`/`plan`'s own idempotency return the *same* `TransformationId` for the same intent+seed)
/// -- measured directly: the first version of this fixture did not clear the directory, and every
/// audit_p3_a2/a3 test failed on a second run with `InvalidState { attempted: "verify" }` against a
/// row `Aborted`/`Escalated`/`Committed` by the run before it.
pub fn journal_path(name: &str) -> PathBuf {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("audit_p3")
        .join(name);
    if base.exists() {
        std::fs::remove_dir_all(&base).expect("clear the engine's scratch directory");
    }
    std::fs::create_dir_all(&base).expect("create the engine's scratch directory");
    base.join("journal.bin")
}

/// A signing key from a fixed seed, so a run is reproducible (the same convention
/// `gx-engine/tests/bin/crash_probe.rs` uses).
pub fn key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("audit-p3-engine-key", &[11u8; 32])
}

/// A `notes.write`-shaped `Intent` at `locator`, from the audit lane's own actor identity — never the
/// implementation lane's `AGENT_KIND` string, so a log mixing the two is never ambiguous about which
/// lane drove a given call.
pub fn intent_for(locator: &str, tool: &str, arguments: &serde_json::Value) -> gx_core::Intent {
    let bytes = serde_json::to_vec(arguments).expect("arguments have a JSON form");
    let goal = gx_adapter_mcp::ToolIntent::new(tool, bytes)
        .encode()
        .expect("a tool call has a canonical form");
    gx_core::Intent::new(
        gx_core::SubstrateKind::Mcp,
        locator.to_string(),
        gx_core::GoalBytes(goal),
        gx_core::ChangeContext::Policy,
        gx_core::Actor::Agent {
            key: "audit-p3-lane-key".to_string(),
            model: "audit-p3-lane (independent of req/120's implementation lane)".to_string(),
        },
    )
}
