// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! A server, a project and a client, for the suites of M6 hand 5.
//!
//! # 🔴 The client is 51 §7's, and that is not an implementation detail
//!
//! > an axum test client (equivalent to `tower::ServiceExt`) hits the same pipeline as the CLI and compares the response (sem: SEM-gx-api-292)
//!
//! [`Client::send`] drives the router through `tower::ServiceExt::oneshot`, which is the whole
//! service stack — routing, the Bearer layer, extractors, the handler — without a socket. What it
//! deliberately does **not** exercise is `gx serve`'s runtime (hand 6): binding, graceful shutdown
//! and the streaming response are outside this and are said so rather than implied by a green suite.

#![allow(dead_code)] // Each suite uses a subset; one file, one set of shapes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use gx_api::state::{AppState, RequestEvidence, ServerKeys};
use gx_api::{auth::Bearer, ReceiptSlot};
use gx_core::TransformationId;
use gx_engine::Engine;
use gx_witness::{KeyPair, Receipt};
use tower::ServiceExt;

/// The token every fixture uses. A literal, so that a suite asserting a refusal can be sure the
/// value it sends is not the value the server holds.
pub const TOKEN: &str = "m6h5-test-token";

/// An empty directory under the cargo target directory.
///
/// `CARGO_TARGET_TMPDIR` rather than `std::env::temp_dir`, for the reason gx-cli's fixtures give: on
/// this project's WSL2 setup `/tmp` is cleared while the machine sits idle. Cleared on entry, not on
/// exit, so a failing test leaves its tree to be read.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// 🔴 45 §1's two keys, as a fixture.
///
/// The **ruler is a different key from the server's**, which is the whole point of the separation
/// (INV-S6). A fixture that used one key for both would be unable to tell a correct escalation from
/// one signed by the party it is about — the failure hand 4's battery point (l) measured on the CLI
/// side, arranged here so that this hand's suites can measure the same thing.
pub struct Keys {
    signing: KeyPair,
    rulers: Vec<KeyPair>,
}

impl Keys {
    /// A server key and one ruler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            signing: KeyPair::from_seed("server-key", &[7u8; 32]),
            rulers: vec![KeyPair::from_seed("ruler-key", &[9u8; 32])],
        }
    }

    /// The ruler's key id, for a request body.
    #[must_use]
    pub fn ruler_id(&self) -> String {
        self.rulers[0].key_id().clone()
    }

    /// The server's key id, for `.gx/config.toml`'s recorded value (E-M6-7).
    #[must_use]
    pub fn signing_id(&self) -> String {
        self.signing.key_id().clone()
    }
}

impl ServerKeys for Keys {
    fn signing(&self) -> &KeyPair {
        &self.signing
    }

    fn ruler(&self, key_id: &str) -> Option<&KeyPair> {
        self.rulers.iter().find(|k| k.key_id() == key_id)
    }
}

/// An archive in memory, standing in for `.gx/receipts/`.
///
/// The real one is `gx_cli::receipt::ReceiptStore` and the dependency cannot run that way (see the
/// manifest), so what this measures is the **shape** of the contract: three slots, one per kind, and
/// `load_commit` reading the commit receipt. `crates/gx-cli/tests/ac_055.rs` measures the real
/// one.
#[derive(Default)]
pub struct MemoryArchive {
    slots: std::sync::Mutex<Vec<(TransformationId, ReceiptSlot, Receipt)>>,
}

impl gx_api::ReceiptArchive for MemoryArchive {
    fn store(
        &self,
        id: &TransformationId,
        slot: ReceiptSlot,
        receipt: &Receipt,
    ) -> Result<(), String> {
        let mut slots = self.slots.lock().map_err(|e| e.to_string())?;
        slots.retain(|(i, s, _)| !(i == id && *s == slot));
        slots.push((*id, slot, receipt.clone()));
        Ok(())
    }

    /// 🔴 **R3 / `req/222` H-02** — the commit slot alone, as the trait now asks.
    fn load_commit(&self, id: &TransformationId) -> Option<Receipt> {
        let slots = self.slots.lock().ok()?;
        slots
            .iter()
            .find(|(i, s, _)| i == id && *s == ReceiptSlot::Commit)
            .map(|(_, _, r)| r.clone())
    }
}

/// A project directory, an engine over it, and a router.
pub struct Server {
    /// The project root; `.gx/` is under it.
    pub project: PathBuf,
    /// The file on the substrate every fixture transformation is about.
    pub target: PathBuf,
    /// The two keys.
    pub keys: Arc<Keys>,
    /// What was handed to [`gx_api::router`].
    pub state: AppState,
    /// The archive, for a suite that wants to look in it.
    pub archive: Arc<MemoryArchive>,
}

impl Server {
    /// A server over a fresh project whose target file holds `before`.
    pub fn new(name: &str, before: &str) -> Self {
        Self::with_target(name, before, "target.txt")
    }

    /// The same, with the target file named by the caller and placed **outside** the project.
    ///
    /// 🔴 Outside, because AC-055 needs two servers — one CLI, one HTTP — to plan the **same**
    /// `Intent`, and 42 §3.3 puts the `locator` in the identity. Two projects with a target file
    /// each would be two locators and therefore two `IntentId`s, so the file has to be one file.
    pub fn with_target(name: &str, before: &str, target_name: &str) -> Self {
        let root = scratch(name);
        let project = root.join("project");
        std::fs::create_dir_all(&project).expect("create the project");
        let target = root.join(target_name);
        std::fs::write(&target, before).expect("write the target");
        Self::over(project, target)
    }

    /// 🔴 **E-M6-7** — build a state whose recorded engine signing keyid is `expected`.
    ///
    /// Returns the refusal rather than a `Server`, because that is what the check is: a mismatch
    /// stops the surface from existing. `crates/gx-api/tests/auth.rs` is the consumer.
    ///
    /// # Errors
    /// [`gx_api::ApiError`] `VALIDATION_ERROR` when `expected` names a different key.
    pub fn try_with_expected_keyid(
        name: &str,
        expected: &str,
    ) -> Result<AppState, gx_api::ApiError> {
        let root = scratch(name);
        let project = root.join("project");
        std::fs::create_dir_all(&project).expect("create the project");
        let keys = Arc::new(Keys::new());
        let evidence = RequestEvidence::new();
        let journal = project.join(".gx").join("ledger").join("journal");
        std::fs::create_dir_all(journal.parent().expect("has a parent"))
            .expect("create .gx/ledger");
        let gate =
            gx_gate::Gate::with_policies(gx_gate::packs::fs_pack().expect("the shipped pack"));
        let engine = Engine::open(&journal, gate, evidence.clone()).expect("open the engine");
        AppState::new(
            engine,
            evidence,
            keys,
            Bearer::new(TOKEN),
            project.join(".gx").join("index"),
            Some(expected),
        )
    }

    /// 🔴 A server whose gate decides with a pack read off disk — the DR-2 road (**E-M6-12**).
    ///
    /// The shipped pack's one forbid is `/etc`, so the only way to reach a `Verdict::Deny` through
    /// the shipped rules is to aim a transformation at a real path under `/etc` — and a record-only
    /// commit of a denied change **applies** it, which on that locator is a write to `/etc`. Hand 3
    /// hit the same wall and §50 M6H3-9, adopted (a) (sem: SEM-gx-api-293), answered it with a fixture pack that denies a writable
    /// path.
    ///
    /// The pack is **gx-cli's file**, read across the crate boundary rather than copied. A copy
    /// would be a second spelling of a deny rule, and the two would drift the first time either was
    /// edited (E-M2-23's whole subject, one directory over).
    pub fn denying(name: &str, before: &str) -> Self {
        Self::denying_in(name, before, None)
    }

    /// The same, with the **process** posture DR-2 calls "per-substrate or whole-configuration" (sem: SEM-gx-api-294) set.
    ///
    /// Needed by one probe and one only: "an explicit `record_only: false` overrides a record-only
    /// server" (sem: SEM-gx-api-295) has nothing to override unless the server is one. `Engine::with_mode` is a builder
    /// consumed at `open`, which is exactly why M6-08 ruled the per-call argument — so the posture is
    /// set **here**, where a fixture is being built, and never on a live `AppState`.
    pub fn denying_in(name: &str, before: &str, mode: Option<gx_core::EnforcementMode>) -> Self {
        let root = scratch(name);
        let project = root.join("project");
        std::fs::create_dir_all(&project).expect("create the project");
        // 🔴 The **name** carries the fragment the pack's `forbid` matches on. Cedar's `like` is a
        // string matcher over the locator attribute (ASM-60-1), and the locator is a temporary
        // directory that does not exist until this line runs — so the denied thing is named rather
        // than pathed, which is gx-cli's `DENIED_FRAGMENT` and is spelled the same way here.
        let target = root.join("gx-denied-target.txt");
        std::fs::write(&target, before).expect("write the target");
        let pack = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../gx-cli/tests/fixtures/deny-writable.cedar"
        );
        let src = std::fs::read_to_string(pack).expect("hand 4's fixture pack is where it was");
        let policies = gx_gate::PolicyEngine::parse(&src).expect("the fixture pack parses");
        Self::build_in(project, target, policies, mode)
    }

    /// A server over an existing project and target (what AC-055 hands it).
    pub fn over(project: PathBuf, target: PathBuf) -> Self {
        let policies = gx_gate::packs::fs_pack().expect("the shipped pack");
        Self::build_in(project, target, policies, None)
    }

    fn build_in(
        project: PathBuf,
        target: PathBuf,
        policies: gx_gate::PolicyEngine,
        mode: Option<gx_core::EnforcementMode>,
    ) -> Self {
        let keys = Arc::new(Keys::new());
        let archive = Arc::new(MemoryArchive::default());
        let evidence = RequestEvidence::new();
        let journal = project.join(".gx").join("ledger").join("journal");
        std::fs::create_dir_all(journal.parent().expect("has a parent"))
            .expect("create .gx/ledger");
        let gate = gx_gate::Gate::with_policies(policies);
        let mut engine = Engine::open(&journal, gate, evidence.clone()).expect("open the engine");
        if let Some(mode) = mode {
            engine = engine.with_mode(mode);
        }
        engine.register_adapter(
            Arc::new(gx_adapter_fs::FsAdapter::new()),
            "gx-api test fixture",
        );
        let state = AppState::new(
            engine,
            evidence,
            keys.clone(),
            Bearer::new(TOKEN),
            project.join(".gx").join("index"),
            Some(&keys.signing_id()),
        )
        .expect("the recorded keyid is this server's")
        .with_archive(archive.clone());
        Self {
            project,
            target,
            keys,
            state,
            archive,
        }
    }

    /// 🔴 Give this server the project's `.gx/LOCK`, and hand back a **second** handle on it.
    ///
    /// The fixture that makes `BUSY` reachable in-process. `req/213` §2-3 built the lock on
    /// `std::fs::File::try_lock`, whose exclusion is per open file description — so a second
    /// `ProcessLock::open` on the same path, in this same process, excludes the server's exactly as
    /// a second `gx` would. That is what lets `crates/gx-api/tests/wire_census.rs` reach the one
    /// refusal whose wire shape has a sixth member (DR-43-5 (3)'s `retry_after_ms`) without the two
    /// real processes `crates/gx-cli/tests/serve_runtime_r2.rs` uses.
    ///
    /// What the caller must do with the returned handle: hold it. Dropping it releases the lock and
    /// the next write succeeds.
    #[must_use]
    pub fn with_writer_lock(&mut self) -> Arc<gx_engine::store::ProcessLock> {
        let path = self.project.join(".gx").join("LOCK");
        let server_side =
            Arc::new(gx_engine::store::ProcessLock::open(&path).expect("open the lock file"));
        self.state = self.state.clone().with_writer_lock(server_side);
        Arc::new(gx_engine::store::ProcessLock::open(&path).expect("open a second handle"))
    }

    /// A client over this server's router.
    #[must_use]
    pub fn client(&self) -> Client {
        Client {
            router: gx_api::router(self.state.clone()),
            token: Some(TOKEN.to_string()),
        }
    }

    /// A client with no bearer token, for the auth suite.
    #[must_use]
    pub fn anonymous(&self) -> Client {
        Client {
            router: gx_api::router(self.state.clone()),
            token: None,
        }
    }

    /// The contents of the file on the substrate.
    pub fn target_contents(&self) -> String {
        std::fs::read_to_string(&self.target).unwrap_or_default()
    }

    /// 44 §2.2's body for `POST /candidates`, against this fixture's target.
    ///
    /// `goal` is a **string** and the reason is in `handlers::CreateCandidate`: the fs adapter reads
    /// goal bytes as the file's new content, and a JSON string is the spelling under which
    /// `POST /candidates` and `gx submit --intent <FILE>` name the same transformation (M6H5-7).
    pub fn intent_body(&self, goal: &str, actor_key: &str) -> serde_json::Value {
        serde_json::json!({
            "substrate": "fs",
            "locator": self.target.display().to_string(),
            "goal": goal,
            "context": "Evidence",
            "actor": { "Human": { "key": actor_key } },
        })
    }
}

/// One response, read into memory.
pub struct Answer {
    /// The status line.
    pub status: StatusCode,
    /// `content-type`, so that a suite can assert 44 §2.3's `application/problem+json`.
    pub content_type: String,
    /// The body, parsed. `Null` when it is not JSON.
    pub json: serde_json::Value,
    /// 🔴 `Retry-After`, so that a suite can assert what DR-43-5 says about `BUSY`: the header is
    /// RFC 9110's `delay-seconds` and the measured hold rides in the body's `retry_after_ms`.
    pub retry_after: Option<String>,
}

impl Answer {
    /// 44 §2.3's `gx_code`, for a refusal.
    pub fn gx_code(&self) -> String {
        self.json["gx_code"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

/// 51 §7's test client.
pub struct Client {
    router: axum::Router,
    token: Option<String>,
}

impl Client {
    /// 🔴 A request whose body is **not** drained — for `GET /stream`, which never ends.
    ///
    /// [`Client::send`] reads the whole body into memory, which is right for thirteen endpoints and
    /// is a hang for the fourteenth. This hands back the response so that a suite can read one frame
    /// at a time (`http_body_util::BodyExt::frame`) and assert on a line that arrived **while the
    /// stream was still open**, which is what AC-056 is about.
    pub async fn open(&self, path: &str, headers: &[(&str, &str)]) -> axum::response::Response {
        let mut builder = Request::builder().method("GET").uri(path);
        if let Some(token) = &self.token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        self.router
            .clone()
            .oneshot(builder.body(Body::empty()).expect("a request"))
            .await
            .expect("the router answers")
    }

    /// One request, with this client's token (if any) and no extra headers.
    pub async fn send(&self, method: &str, path: &str, body: Option<serde_json::Value>) -> Answer {
        self.send_with(method, path, body, &[]).await
    }

    /// 🔴 A request with **raw** bytes and exactly the headers given — no `content-type` added,
    /// no JSON serialisation (v0.4-l, `req/189`: H-10 needs "a body with no `Content-Type`" and
    /// M-14 needs "a body over the limit", neither of which [`Client::send`] can produce because
    /// it always declares JSON — the audit's own note that the test client was outside the
    /// denominator for that reason, `req/182` H-10).
    pub async fn send_raw(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Answer {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = &self.token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let request = builder.body(Body::from(body)).expect("a request");
        self.answer(request).await
    }

    /// The same, with extra headers — 44 §2.4's `Idempotency-Key`, for instance.
    pub async fn send_with(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
        headers: &[(&str, &str)],
    ) -> Answer {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = &self.token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let request = match body {
            Some(value) => builder
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&value).expect("serialises")))
                .expect("a request"),
            None => builder.body(Body::empty()).expect("a request"),
        };
        self.answer(request).await
    }

    /// One request through the whole service stack, read into memory.
    async fn answer(&self, request: Request<Body>) -> Answer {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("the router answers");
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .expect("a readable body");
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        Answer {
            status,
            content_type,
            json,
            retry_after,
        }
    }
}
