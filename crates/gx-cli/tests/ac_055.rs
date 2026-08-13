//! 🔴 **AC-055** — the CLI and the HTTP surface, on one intent, answering the same three things.
//!
//! 34's verification method for FR-050, and 51 §7's instrument:
//!
//! > integration（axum test client で CLI と比較）
//! > axum test client（`tower::ServiceExt`相当）でCLIと同一パイプラインを叩き、レスポンスを比較する
//!
//! req/88 §6.2's DoD for this hand states the comparison: 「**CLI と HTTP が同一 intent に対し同一
//! `TransformationId`・同一 `Verdict`・同一 `Committed` を返す**」.
//!
//! # 🔴 「同一」 is 「同一 from `Candidate` onward」 — req/88 §3 Λ2, and it is not a weakening
//!
//! Λ2 proves that N single-shot CLI runs and one long-lived engine are observationally equal on Σ,
//! and names the one place the equality breaks: 「`Σ` に入らない state を CLI が持った瞬間に等価が
//! 壊れる——M6-01(a) の `.gx/drafts/` がまさにそれである」. 44 §0 permits that asymmetry explicitly —
//! 「HTTP `POST /candidates`は submit+plan を一括atomically実行するためDraft単独状態を公開せず、この
//! 規則の対象外となる」 — so a suite that compared the **Draft** phase would be asserting something 44
//! says is not observable, and 「a correct implementation would fail it」 (req/88 §8.2).
//!
//! So the comparison starts at the `TransformationId`, which is the first value both surfaces have.
//!
//! # 🔴 How two surfaces are given **one** intent
//!
//! 42 §3.3 puts the `locator` in the `Intent` and the `IntentId` is the intent's CID, so two
//! projects with a target file each would be two locators and therefore two different
//! transformations — the suite would compare two things and find them different for a reason that
//! says nothing about either surface.
//!
//! The fixture is therefore **one target file, two `.gx/` directories**, run in sequence with the
//! file reset between them. That makes every input to the identity identical: the same substrate,
//! the same locator, the same goal bytes, the same context, the same actor key. What differs is only
//! which surface drove the eight entry points — which is the whole of what AC-055 is asking about.
//!
//! # 🔴 What is deliberately **not** compared, and why each is not a dodge
//!
//! * **the signing key.** The CLI signs with the transformation's own actor key (M6H3-4 recorded
//!   that 45 §1's separation is unimplemented there); the server signs with its own (E-M6-7), because
//!   a server is not the actor and never holds a client's private key. AC-055 names three things and
//!   none of them is a key, and the difference is stated in `gx_api::state`'s header rather than
//!   discovered by a reader diffing two receipts.
//! * **`issued_at` / `at`.** Two runs at two times. 43 T-2's idempotency is what makes the *identity*
//!   independent of the clock, and this suite is a demonstration of that: if `created_at` were in the
//!   identity view, the two ids could not match.
//! * **the response envelope.** The CLI's stdout is 44 §1.2's object and the HTTP body is 44 §2.2's;
//!   they are different contracts on purpose. Comparing them field for field would be asserting that
//!   44 §1 and 44 §2 are one document.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use gx_api::state::{AppState, RequestEvidence, ServerKeys};
use gx_api::{auth::Bearer, ReceiptArchive, ReceiptSlot};
use gx_cli::receipt::{ReceiptStore, StoredKind};
use gx_core::TransformationId;
use gx_witness::{KeyPair, Receipt};
use support::{run, scratch, secure_scratch};
use tower::ServiceExt;

const TOKEN: &str = "ac055-token";

/// 🔴 `.gx/receipts/` behind [`ReceiptArchive`] — the **real** store, not a stand-in.
///
/// `gx_api::ReceiptSlot` and `gx_cli::receipt::StoredKind` are two enums for one vocabulary, which
/// the dependency direction leaves unavoidable (47 §1(a) makes gx-cli contain gx-api, so gx-api
/// cannot name gx-cli's type). This adapter is the one place they meet, and
/// [`the_two_receipt_vocabularies_are_one_vocabulary`] asserts they spell the three tags the same.
struct CliArchive {
    store: ReceiptStore,
}

fn slot_to_kind(slot: ReceiptSlot) -> StoredKind {
    match slot {
        ReceiptSlot::Verdict => StoredKind::Verdict,
        ReceiptSlot::Ruling => StoredKind::Ruling,
        ReceiptSlot::Commit => StoredKind::Commit,
    }
}

impl ReceiptArchive for CliArchive {
    fn store(
        &self,
        id: &TransformationId,
        slot: ReceiptSlot,
        receipt: &Receipt,
    ) -> Result<(), String> {
        self.store
            .put(id, slot_to_kind(slot), receipt)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn load(&self, id: &TransformationId) -> Option<Receipt> {
        self.store
            .first_available(id)
            .ok()
            .flatten()
            .map(|(_, r)| r)
    }
}

/// The server's own key, and no ruler (this suite escalates nothing).
struct OneKey(KeyPair);

impl ServerKeys for OneKey {
    fn signing(&self) -> &KeyPair {
        &self.0
    }

    fn ruler(&self, _key_id: &str) -> Option<&KeyPair> {
        None
    }
}

/// What one surface answered about one transformation — AC-055's three values.
#[derive(Debug, PartialEq, Eq)]
struct Answered {
    /// 「同一 `TransformationId`」.
    transformation: String,
    /// 「同一 `Verdict`」.
    verdict: String,
    /// 「同一 `Committed`」.
    state: String,
}

/// Everything both surfaces are given: one file, two projects, one key.
struct Fixture {
    target: PathBuf,
    home: PathBuf,
    cli_project: PathBuf,
    http_project: PathBuf,
    key_id: String,
    goal_file: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = scratch(name);
        let target = root.join("target.txt");
        std::fs::write(&target, "before\n").expect("write the target");
        // 44 §1.2's `--intent <FILE>` carries the **bytes** (E-M6-11), and `POST /candidates`'s
        // `goal` is a JSON **string** taken as its UTF-8 bytes (M6H5-7). Both therefore see
        // `after\n`, which is what makes the two `IntentId`s one value.
        let goal_file = root.join("goal.txt");
        std::fs::write(&goal_file, "after\n").expect("write the goal");
        let cli_project = root.join("cli");
        let http_project = root.join("http");
        std::fs::create_dir_all(&cli_project).expect("create");
        std::fs::create_dir_all(&http_project).expect("create");
        // Key material on a filesystem with unix permissions (M6H2-10: this repository sits on
        // drvfs, where `KeyPair::save`'s 0600 becomes 0777 and `KeyPair::load` then refuses it).
        let home = secure_scratch(&format!("{name}-home"));

        let mut fixture = Self {
            target,
            home,
            cli_project,
            http_project,
            key_id: String::new(),
            goal_file,
        };
        let generated = run(fixture.gx().args(["key", "gen", "--json"]));
        assert_eq!(generated.code, 0, "a key: {}", generated.stderr);
        fixture.key_id = generated.json()["key_id"]
            .as_str()
            .expect("a key id")
            .to_string();
        fixture
    }

    /// A `gx` invocation against the **CLI** project, with this fixture's key store.
    fn gx(&self) -> std::process::Command {
        let mut cmd = support::gx();
        cmd.env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .arg("--project")
            .arg(&self.cli_project);
        cmd
    }

    fn target_contents(&self) -> String {
        std::fs::read_to_string(&self.target).unwrap_or_default()
    }

    fn reset_target(&self) {
        std::fs::write(&self.target, "before\n").expect("reset the target");
    }

    /// 44 §1.2's four processes, and the three values AC-055 compares.
    fn drive_cli(&self) -> Answered {
        let submitted = run(self
            .gx()
            .arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&self.target)
            .arg("--intent")
            .arg(&self.goal_file)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &self.key_id]));
        assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
        let intent_id = submitted.json()["intent_id"]
            .as_str()
            .expect("an intent id")
            .to_string();

        let planned = run(self.gx().args(["plan", &intent_id]));
        assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
        let tid = planned.json()["transformation"]["id"]
            .as_str()
            .expect("a transformation id")
            .to_string();

        let verified = run(self.gx().args(["verify", &tid]));
        assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
        let committed = run(self.gx().args(["commit", &tid]));
        assert_eq!(committed.code, 0, "commit: {}", committed.stderr);

        println!(
            "CLI intent={intent_id} tid={tid} verdict={} state={}",
            verified.json()["kind"],
            committed.json()["state"]
        );
        Answered {
            transformation: tid,
            verdict: verified.json()["kind"].as_str().unwrap_or("").to_string(),
            state: committed.json()["state"].as_str().unwrap_or("").to_string(),
        }
    }

    /// The server's key: the **same** `KeyPair` the CLI signs with, loaded off disk.
    ///
    /// 🔴 Not because AC-055 needs it — the ids and the verdict do not depend on a key — but because
    /// the fixture would otherwise be quietly asserting that they do not, which is a claim rather
    /// than a setup. The key is the same, the receipts are comparable, and the divergence this hand
    /// **does** ship (a server signs with its own key, E-M6-7) is documented where it lives.
    fn key(&self) -> KeyPair {
        let path = self
            .home
            .join(".gx")
            .join("keys")
            .join(format!("{}.key", self.key_id));
        KeyPair::load(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()))
    }

    /// 44 §2.2's three calls, over 51 §7's test client.
    async fn drive_http(&self) -> Answered {
        let evidence = RequestEvidence::new();
        let journal = self.http_project.join(".gx").join("ledger").join("journal");
        std::fs::create_dir_all(journal.parent().expect("a parent")).expect("create .gx/ledger");
        let gate =
            gx_gate::Gate::with_policies(gx_gate::packs::fs_pack().expect("the shipped pack"));
        let mut engine =
            gx_engine::Engine::open(&journal, gate, evidence.clone()).expect("open the engine");
        engine.register_adapter(
            Arc::new(gx_adapter_fs::FsAdapter::new()),
            "gx-cli ac_055 fixture",
        );
        let layout = gx_cli::layout::Layout::create(&self.http_project).expect("create .gx/");
        let archive = CliArchive {
            store: ReceiptStore::in_layout(&layout),
        };
        let state = AppState::new(
            engine,
            evidence,
            Arc::new(OneKey(self.key())),
            Bearer::new(TOKEN),
            layout.join("index"),
            None,
        )
        .expect("no recorded keyid to disagree with")
        .with_archive(Arc::new(archive));
        let router = gx_api::router(state);

        let created = send(
            &router,
            "POST",
            "/v1/candidates",
            Some(serde_json::json!({
                "substrate": "fs",
                "locator": self.target.display().to_string(),
                "goal": "after\n",
                "context": "Evidence",
                "actor": { "Human": { "key": self.key_id } },
            })),
        )
        .await;
        assert_eq!(created.0, 201, "POST /candidates: {}", created.1);
        let tid = created.1["id"].as_str().expect("an id").to_string();

        let verified = send(
            &router,
            "POST",
            &format!("/v1/candidates/{tid}/verify"),
            None,
        )
        .await;
        assert_eq!(verified.0, 200, "verify: {}", verified.1);
        let committed = send(
            &router,
            "POST",
            &format!("/v1/candidates/{tid}/commit"),
            None,
        )
        .await;
        assert_eq!(committed.0, 200, "commit: {}", committed.1);

        println!(
            "HTTP tid={tid} verdict={} state={}",
            verified.1["verdict"], committed.1["state"]
        );
        Answered {
            transformation: tid,
            verdict: verified.1["verdict"].as_str().unwrap_or("").to_string(),
            state: committed.1["state"].as_str().unwrap_or("").to_string(),
        }
    }
}

/// One request through the router, with the token.
async fn send(
    router: &axum::Router,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {TOKEN}"));
    let request = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            builder
                .body(axum::body::Body::from(
                    serde_json::to_vec(&value).expect("serialises"),
                ))
                .expect("a request")
        }
        None => builder.body(axum::body::Body::empty()).expect("a request"),
    };
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("the router answers");
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .expect("a readable body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// 🔴 **AC-055** — one intent, two surfaces, three identical answers.
#[tokio::test]
async fn ac_055_the_cli_and_the_http_surface_answer_the_same_three_things() {
    let fixture = Fixture::new("ac_055");

    let cli = fixture.drive_cli();
    let after_cli = fixture.target_contents();
    assert_eq!(after_cli, "after\n", "the CLI applied the delta");

    // 🔴 The world is put back, because the second surface plans against it. 43 §8 is explicit that a
    // committed predecessor makes `Fingerprint₀` stale — 「再`plan()`（再fingerprint）を強制する」 — so
    // running the two against a moved substrate would compare two transformations of two different
    // worlds and find them different for a reason that says nothing about either surface.
    fixture.reset_target();
    assert_eq!(fixture.target_contents(), "before\n");

    let http = fixture.drive_http().await;
    let after_http = fixture.target_contents();

    println!("AC055_CLI={cli:?}");
    println!("AC055_HTTP={http:?}");
    println!(
        "AC055_TID_SAME={} VERDICT_SAME={} STATE_SAME={} SUBSTRATE_SAME={}",
        u8::from(cli.transformation == http.transformation),
        u8::from(cli.verdict == http.verdict),
        u8::from(cli.state == http.state),
        u8::from(after_cli == after_http),
    );

    assert_eq!(
        cli.transformation, http.transformation,
        "🔴 同一 `TransformationId`. The id is the CID of the canonical transformation (41 §3), so \
         two surfaces naming one change with two names would mean one of them encodes differently — \
         which 41 §6's 「全canonical encodeはgx-canon経由のみ」 exists to make impossible and which \
         則 1 (i) is the mechanical check for"
    );
    assert_eq!(
        cli.verdict, http.verdict,
        "🔴 同一 `Verdict`. 41 §4 keeps the judgement in one function; a difference here would mean \
         a surface decided something"
    );
    assert_eq!(cli.state, "Committed", "the CLI reached 43 T-11");
    assert_eq!(
        cli.state, http.state,
        "🔴 同一 `Committed`. 42 §1.3-3 keeps the state table on the engine side (則 1 (iii))"
    );
    assert_eq!(
        after_cli, after_http,
        "and the substrate ended in the same place, which is the fact the three values are about"
    );
}

/// 🔴 Λ2's asymmetry, measured rather than asserted: the CLI has a `.gx/drafts/` and the server does not.
///
/// req/88 §3 Λ2's counter-example, and the reason AC-055's 「同一」 reads as 「同一 from `Candidate`
/// onward」. 44 §0 permits it (「HTTP `POST /candidates`は…Draft単独状態を公開せず」) and req/56 §2's
/// drafts row records the cost since M6H4-5 (「plan と undo ができない」) — so the right place for it in
/// a suite is a **count**, beside the ids that do match.
#[tokio::test]
async fn the_draft_phase_is_the_one_place_the_two_surfaces_differ() {
    let fixture = Fixture::new("ac_055_drafts");
    fixture.drive_cli();
    fixture.reset_target();
    fixture.drive_http().await;

    let count = |root: &PathBuf| {
        std::fs::read_dir(root.join(".gx").join("drafts"))
            .map(|entries| entries.filter_map(std::result::Result::ok).count())
            .unwrap_or(0)
    };
    let cli_drafts = count(&fixture.cli_project);
    let http_drafts = count(&fixture.http_project);
    println!("CLI_DRAFTS={cli_drafts} HTTP_DRAFTS={http_drafts}");
    assert_eq!(
        cli_drafts, 1,
        "M6-01 採(a): `gx submit` and `gx plan` are two processes and the intent body has nowhere \
         else to live"
    );
    assert_eq!(
        http_drafts, 0,
        "🔴 and the server has none: 44 §2.1 makes `POST /candidates` one call, so no body ever \
         crosses a process boundary. This is Λ2's counter-example measured — the equality holds from \
         `Candidate` onward and the Draft phase is where the CLI carries state Σ does not"
    );
}

/// 🔴 The two receipt vocabularies are one vocabulary (M6H4-7).
///
/// `gx_api::ReceiptSlot` and `gx_cli::receipt::StoredKind` are two enums because the dependency
/// direction leaves no third place for one (47 §1(a): gx-cli contains gx-api). Two spellings of a
/// three-word vocabulary is exactly the drift E-M2-23's 「1 箇所宣言」 is about, so the one place they
/// meet — this suite's archive adapter — is where the equality is asserted.
#[test]
fn the_two_receipt_vocabularies_are_one_vocabulary() {
    let pairs = [
        (ReceiptSlot::Verdict, StoredKind::Verdict),
        (ReceiptSlot::Ruling, StoredKind::Ruling),
        (ReceiptSlot::Commit, StoredKind::Commit),
    ];
    for (slot, kind) in pairs {
        println!("SLOT {} == KIND {}", slot.tag(), kind.tag());
        assert_eq!(
            slot.tag(),
            kind.tag(),
            "M6H4-7's `<TID>.<kind>.json` has one spelling per kind, whichever crate is asked"
        );
        assert_eq!(
            slot_to_kind(slot),
            kind,
            "and the adapter maps them straight"
        );
    }
    // The origin constant, spelled twice for the same reason and asserted here for the same one:
    // a checkpoint signed under one origin does not verify against another (42 §3.11).
    println!(
        "ORIGIN_CLI={} ORIGIN_API={}",
        gx_cli::ledger::DEFAULT_ORIGIN,
        gx_api::DEFAULT_ORIGIN
    );
    assert_eq!(gx_cli::ledger::DEFAULT_ORIGIN, gx_api::DEFAULT_ORIGIN);
}
