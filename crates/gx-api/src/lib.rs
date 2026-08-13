#![forbid(unsafe_code)]
//! The Glovrex HTTP surface (44 §2) — **thirteen synchronous endpoints**.
//!
//! # 🔴 則 1 — this crate holds no semantic authority
//!
//! The same three absences gx-cli carries, checked by the same instrument
//! (`crates/gx-canon/tests/authority_boundary.rs`): no canonical encode (41 §6), no `Verdict`
//! construction (41 §4), no `Lifecycle` write (42 §1.3-3). req/88 §3 Λ1 explains why this is a
//! design claim and not hygiene — 48 §4 wrote it about the Console in TypeScript (「TS自体が検証
//! ロジックを持つことは決してない」) and Λ1 is the same sentence one language in.
//!
//! For this crate the rule has a sharper edge than for the CLI, because an HTTP handler is where
//! 「just compute the answer here, it is faster」 is most tempting: `GET /candidates/{id}` returns
//! `{transformation, state, verdict, fingerprint}` and every one of those four has an engine
//! accessor already (req/88 §2.2). The surface's job is to call them.
//!
//! # What hand 5 builds
//!
//! Every endpoint of 44 §2.1 except `GET /stream`, plus the four decisions §47 put in front of them:
//!
//! * [`gx_code`] — **M6-09**'s one map, thirty-three refusal kinds onto twelve codes, with the folds
//!   listed rather than implied (req/88 §3 Λ4: 「規律は『畳むな』ではなく『畳んだ事を書け』」);
//! * [`auth`] — **M6-10**'s static Bearer, the loopback default, and 「検査の不在」 as a string;
//! * [`idempotency`] — **M6-11**'s hand-written `Idempotency-Key`, persisted, with the one line that
//!   says what it does **not** protect;
//! * [`state`] — **M6-06 採(a)**'s single lock, and 45 §1's two keys as two methods (**E-M6-7** /
//!   **E-M6-15**).
//!
//! # 🔴 The count, and a disagreement with the requirement document
//!
//! req/88 §6.2 gives this hand 「44 §2 の同期面 **11** endpoint(`/stream` と一覧系を除く全部)」 and
//! hand 1's `SPECIFIED_ENDPOINTS` array — read off 44 §2.1 — has **fourteen** rows. Fourteen minus
//! `/stream` is **thirteen**, and 44 has no list endpoints at all (§2.7: 「本書指定のエンドポイント
//! 群には一覧（list）系エンドポイントは含まれない」), so M6-05's three are hand 6's *additions* and
//! cannot be subtracted from a count of 44's own. The parenthetical is the operative text and the
//! numeral is an enumeration error; thirteen are implemented and the discrepancy is raised as
//! **M6H5-1** under 規律49's lane-side reconciliation.
//!
//! # What hand 6 takes
//!
//! `GET /stream` (M6-12's event map, M6-13's resume cursor), `gx serve`'s runtime and graceful
//! shutdown, and M6-05's three list endpoints. This crate declares no `tokio` for that reason: the
//! hand that writes `#[tokio::main]` declares it (req/38 §38 採(a)).

pub mod auth;
pub mod extract;
pub mod gx_code;
pub mod handlers;
pub mod idempotency;
pub mod list;
pub mod problem;
pub mod rfc3339;
pub mod serve;
pub mod state;
pub mod stream;

use axum::routing::{get, post};

pub use problem::ApiError;
pub use serve::{serve, ServeConfig, ServeError, ServeOutcome, Shutdown};
pub use state::{AppState, RequestEvidence, ServerKeys};

/// 42 §3.11's example namespace, and this version's default `origin` for `GET /ledger/checkpoint`.
///
/// The same constant gx-cli's `gx log checkpoint` defaults to, spelled twice because the two crates
/// cannot see each other (see the manifest). A mismatch would make a checkpoint signed by one
/// surface fail to verify against the other, which is exactly what an origin is for — so
/// `crates/gx-cli/tests/ac_055.rs` compares the two spellings.
pub const DEFAULT_ORIGIN: &str = "glovrex-ledger/v1";

/// 44 §2's base path.
pub const BASE_PATH: &str = "/v1";

/// 🔴 **M6H4-7**'s three kinds, as this crate's name for them.
///
/// `.gx/receipts/<TID>.<kind>.json`, `kind ∈ {verdict, ruling, commit}`. The **naming** lives in
/// gx-cli (`gx_cli::receipt::StoredKind`), which is the crate that owns req/56 §2's declaration; this
/// is the vocabulary an archive implementation is asked in, and `crates/gx-cli/tests/ac_055.rs`
/// asserts the two spell the three tags identically. Two enums are one more than ideal and are what
/// the dependency direction (47 §1(a): gx-cli contains gx-api) leaves available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptSlot {
    /// 43 T-4a/b/c's `VerdictReceipt`, signed by this server's key.
    Verdict,
    /// 43 T-5 / T-5b's human ruling, signed by the **ruler's** key (INV-S6).
    Ruling,
    /// 43 T-11's `CommitReceipt`.
    Commit,
}

impl ReceiptSlot {
    /// The infix in `<TID>.<kind>.json`.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            ReceiptSlot::Verdict => "verdict",
            ReceiptSlot::Ruling => "ruling",
            ReceiptSlot::Commit => "commit",
        }
    }
}

/// Where receipts are kept between processes, as a question the caller answers.
///
/// 🔴 `Engine::open` leaves the in-flight table empty (M5H3-5), so a server restarted after a commit
/// holds no receipt for it and `GET /receipts/{tid}` would answer 404 about a document on disk. The
/// archive is the road back — and it is a **trait** rather than a directory because
/// `.gx/receipts/`'s layout is req/56 §2's, declared once in gx-cli and compared against the
/// requirement document by `m6_surface_doubt::the_dotgx_layout_is_req56_exactly`. A second spelling
/// here would be a second list.
pub trait ReceiptArchive: Send + Sync {
    /// File one receipt. `Err` carries a message; the caller decides whether it is fatal.
    ///
    /// # Errors
    /// Whatever the archive could not do, as a sentence.
    fn store(
        &self,
        id: &gx_core::TransformationId,
        slot: ReceiptSlot,
        receipt: &gx_witness::Receipt,
    ) -> Result<(), String>;

    /// The most specific receipt held for a transformation, if any.
    fn load(&self, id: &gx_core::TransformationId) -> Option<gx_witness::Receipt>;
}

/// An archive that holds nothing, for a deployment that has none.
///
/// Named rather than an `Option<Arc<dyn ReceiptArchive>>`, because a `None` at every call site is a
/// question every call site has to answer and this is the answer once. It is also the honest default
/// for a server whose receipts live only in its own table: `load` says so by answering `None`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoArchive;

impl ReceiptArchive for NoArchive {
    fn store(
        &self,
        _id: &gx_core::TransformationId,
        _slot: ReceiptSlot,
        _receipt: &gx_witness::Receipt,
    ) -> Result<(), String> {
        Ok(())
    }

    fn load(&self, _id: &gx_core::TransformationId) -> Option<gx_witness::Receipt> {
        None
    }
}

/// 🔴 The router: `/v1` + thirteen routes, with `/healthz` outside the Bearer guard.
///
/// 44 §2.5's 「全エンドポイント（`/healthz`除く）に`Authorization: Bearer <token>`必須」 is a statement
/// about the router's **shape**, so it is enforced on the shape: everything but `/healthz` sits under
/// one `route_layer`, and `crates/gx-api/tests/auth.rs` walks every route asserting each refuses an
/// unauthenticated request. Thirteen handlers each remembering to call a checker would be thirteen
/// chances to forget, and the one that forgot would not be discoverable by reading the twelve that
/// did not.
///
/// axum 0.8's path syntax is `{id}`, which is also 44 §2.1's own spelling.
// No `#[must_use]`: `axum::Router` already carries one, and clippy refuses the duplicate — the same
// note hand 1 left on the empty router this replaces.
pub fn router(state: AppState) -> axum::Router {
    let guarded = axum::Router::new()
        .route("/candidates", post(handlers::create_candidate))
        .route("/candidates/{id}", get(handlers::get_candidate))
        .route("/candidates/{id}/verify", post(handlers::verify_candidate))
        .route("/candidates/{id}/commit", post(handlers::commit_candidate))
        .route(
            "/candidates/{id}/escalation",
            post(handlers::escalate_candidate),
        )
        .route("/candidates/{id}/cancel", post(handlers::cancel_candidate))
        .route(
            "/transformations/{id}/undo",
            post(handlers::undo_transformation),
        )
        .route(
            "/transformations/{id}/replay",
            post(handlers::replay_transformation),
        )
        .route("/transformations/{id}", get(handlers::get_transformation))
        .route("/receipts/{tid}", get(handlers::get_receipt))
        .route("/ledger/proof", get(handlers::ledger_proof))
        .route("/ledger/checkpoint", get(handlers::ledger_checkpoint))
        // 🔴 Hand 6: 44 §2.2's fourteenth endpoint, and M6-05's four extensions.
        .route("/stream", get(stream::stream))
        .route("/candidates", get(list::candidates))
        .route("/escalations", get(list::escalations))
        .route("/transformations", get(list::transformations))
        .route("/ledger/consistency", get(list::ledger_consistency))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::guard,
        ));

    axum::Router::new().nest(
        BASE_PATH,
        axum::Router::new()
            .route("/healthz", get(handlers::healthz))
            .merge(guarded)
            // 🔴 Hand 6: graceful shutdown's stages 1 and 2, **outside** the Bearer guard and
            // therefore covering `/healthz` too. A load balancer's health check is exactly the
            // request that must stop succeeding first when a server is going away: answering
            // 「ok」 while refusing everything else would keep traffic arriving at a socket that
            // refuses it. See `serve::guard` for the three stages.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                serve::guard,
            ))
            .with_state(state),
    )
}

/// The endpoints 44 §2.2 specifies, by name, so that the count is a declaration and not a memory.
///
/// The `unsafe_forbidden.rs` shape: a list somebody has to edit deliberately. Hand 5 serves twelve of
/// these plus `/healthz`; hand 6 takes `/stream` plus the three list endpoints M6-05 raises. A hand
/// that implemented one and forgot to strike it here would leave the two halves disagreeing, which is
/// what `tests/router.rs` compares.
pub const SPECIFIED_ENDPOINTS: [&str; 14] = [
    "POST /candidates",
    "GET /candidates/{id}",
    "POST /candidates/{id}/verify",
    "POST /candidates/{id}/commit",
    "POST /candidates/{id}/escalation",
    "POST /candidates/{id}/cancel",
    "POST /transformations/{id}/undo",
    "POST /transformations/{id}/replay",
    "GET /transformations/{id}",
    "GET /receipts/{tid}",
    "GET /ledger/proof",
    "GET /ledger/checkpoint",
    "GET /stream",
    "GET /healthz",
];

/// 🔴 The one of [`SPECIFIED_ENDPOINTS`] hand 5 did **not** serve, and hand 6 does.
///
/// A named list rather than a subtraction, so that 「twelve are served」 and 「this is the missing
/// one」 are one statement. `/stream` is hand 6's for four rulings' worth of reasons (M6-12's event
/// map, M6-13's resume cursor, M6-06's runtime, and the fact that a JSONL stream over a mutex is a
/// design and not a route).
pub const HAND6_ENDPOINTS: [&str; 1] = ["GET /stream"];

/// How many of [`SPECIFIED_ENDPOINTS`] this crate serves.
///
/// **Fourteen** since hand 6: thirteen behind the Bearer guard and `/healthz` outside it. See the
/// crate header for why hand 5's thirteen was not req/88 §6.2's eleven (M6H5-1), and
/// [`list::EXTENSION_ENDPOINTS`] for the four this crate serves that 44 does not specify.
pub const IMPLEMENTED_ENDPOINTS: usize = 14;
