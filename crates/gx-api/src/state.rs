//! What a handler is handed: one engine behind one lock, two directories, a token and two keys.
//!
//! # 🔴 **M6-06 採(a)** — one `Mutex`, every request serialised
//!
//! `Engine`'s eight entry points take `&mut self` and axum wants `Send + Sync` state. req/38 §47
//! adopted (a):
//!
//! > `Arc<Mutex<Engine>>` 1 本=**全 request を直列化**(M5H5-6 の「呼び出し側の責務」を最も素直に
//! > 果たす形。代償=NFR-003 の 100 commits/s と手 7/手 8 で同定済の decay が正面から効く)
//!
//! and registered the alternative — a per-object lock, M5H5-6(b) — in the D-7 ledger against the
//! firing condition 「serve 経由の AC-066 実測が SLA を割った時」. §47's note on why not to reach for
//! it first is the measurement argument: 「**(b) を先に入れると「入れたから速い」の対照が取れない**」.
//!
//! The consequence is worth naming rather than leaving in a ruling: **this server has no
//! concurrency**. Two clients committing two unrelated transformations wait for each other. That is
//! a correct v0.1 and a bad v0.2, and the number that turns one into the other is hand 7's.
//!
//! # 🔴 What is not here, and why the resume machinery is absent
//!
//! req/38 §50 and §51 both fixed the line: 「**resume/rehydrate は CLI 固有の層**(serve は表を持つ)」.
//! A single-shot `gx verify` runs in a process whose in-flight table is empty (M5H3-5), so gx-cli
//! has [`Session::resume`](../../gx_cli/session/struct.Session.html#method.resume) to re-plan a row
//! back into existence. A long-lived engine never lost it. That is also why `.gx/drafts/` has no
//! reader here: 44 §0 is explicit that 「HTTP `POST /candidates`は submit+plan を一括atomically実行
//! するためDraft単独状態を公開せず」, so the intent body never has to cross a process boundary and
//! req/88 §3 Λ2's one counter-example is a cost the CLI pays alone (req/56 §2's drafts row says so
//! since M6H4-5).
//!
//! # 🔴 45 §1's two keys, in the type (**E-M6-7** / **E-M6-15**)
//!
//! 45 §1 separates an **Actor signing key** from a **Ledger signing key**, and req/38 §50 M6H3-4
//! recorded that gx-cli does not implement the separation: `gx verify` and `gx commit` sign with the
//! key the transformation's own `Actor` names, so a receipt says 「this actor's key attests this
//! change was verified」 rather than 「the engine attests it」.
//!
//! A server cannot copy that. It is not the actor; the actor is a client on the other side of a
//! socket that never sends a private key. So [`ServerKeys`] has two methods and they are the two
//! keys:
//!
//! * [`ServerKeys::signing`] — **the server's own key**, used for every `VerdictReceipt` and
//!   `CommitReceipt` this surface causes. E-M6-7's design: 「`.gx/config.toml` に engine 署名 keyid
//!   参照(req/56 §2 の「公開 keyid の参照のみ」枠)」, which is why [`AppState::new`] takes an
//!   `expected_signing_keyid` and refuses to start when it disagrees. The **reader** of that file is
//!   `gx serve` (hand 6, the hand with the flag); the check is here, where a mismatch can stop the
//!   surface from existing rather than be noticed in a log.
//! * [`ServerKeys::ruler`] — the **adjudicator's** key, looked up by the id the request carries.
//!   **E-M6-15** made `--actor-key` required on `gx escalation` and INV-S6 is why: 「裁かれる側が
//!   自分を承認する既定値は存在しない」. So there is no default and no fallback to
//!   [`ServerKeys::signing`]: a server that signed a human ruling with its own key would be
//!   recording that **the server** allowed the change.
//!
//! 🔴 **This is a divergence from the CLI and AC-055 does not measure it.** AC-055 compares the
//! transformation id, the verdict and the committed state, and none of the three depends on which
//! key signed. The receipts differ in `key_id`, deliberately, and that is written here rather than
//! discovered by a reader comparing two receipts.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use gx_core::Timestamp;
use gx_engine::{Engine, EvidenceSource};
use gx_witness::{Evidence, KeyPair};

use crate::auth::Bearer;
use crate::idempotency::IdempotencyStore;
use crate::problem::ApiError;
use crate::{NoArchive, ReceiptArchive};

/// 🔴 Evidence a **single request** supplies, injected through 41 §6's own trait.
///
/// 44 §2.2's `POST /candidates/{id}/verify` body is `{ evidence: Evidence[], record_only: bool|null }`
/// and `Engine::open` fixes its `EvidenceSource` for the engine's whole life. The second half of
/// that pair — `record_only` — was solved by **M6-08 採(a)**, a per-call argument, precisely because
/// (b) 「serve が request ごとに `&mut self` で mode を差し替える」 was ruled 「採ってはならない」: it
/// leaks one request's posture into another's.
///
/// This cell is (b)'s shape for the field M6-08 did **not** move, and it is safe here for a reason
/// that is stated rather than assumed: **the whole engine is behind one lock** (M6-06 採(a)), and
/// the load and the `Engine::verify` that reads it happen inside one hold of that lock. There is no
/// interleaving to leak through. The moment M5H5-6(b)'s per-object lock fires, this stops being
/// true — so the honest repair is the one M6-08 already made for `mode`: an evidence argument on
/// `Engine::verify`. Raised as **M6H5-6**, with the firing condition tied to the same ruling.
#[derive(Clone, Debug, Default)]
pub struct RequestEvidence {
    cell: Arc<Mutex<Vec<Evidence>>>,
}

impl RequestEvidence {
    /// An empty source: 44 §1.2's 「省略時はgx-gate組込のInvariantCheck/Cedar評価のみ」.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load what this request supplied. Called with the engine's lock already held.
    pub fn load(&self, evidence: Vec<Evidence>) {
        let mut cell = self.cell.lock().unwrap_or_else(PoisonError::into_inner);
        *cell = evidence;
    }

    /// Empty it, so that the next request cannot read this one's.
    ///
    /// Called on **every** exit from a verify handler, including the refusing ones. A clear that
    /// only ran on success would leave a failed request's evidence visible to the next caller, which
    /// is the leak M6-08(b) is named after.
    pub fn clear(&self) {
        self.load(Vec::new());
    }

    /// 🔴 How many items the cell is holding **right now**.
    ///
    /// The property [`RequestEvidence::clear`] exists for, made observable: 「a request's evidence
    /// does not outlive the request」. Without this accessor the clear is invisible to a behavioural
    /// probe — every verify handler loads unconditionally, so a *later* request cannot see an
    /// earlier one's items through the gate, and a mutation that deleted the clear survived a suite
    /// that only compared verdicts (measured: `tools/verify_m6h5.sh` point (n), first run).
    ///
    /// What the clear actually buys is therefore not correctness of the next verdict but the
    /// **lifetime** of a client's evidence in this process's memory, and a lifetime is a fact about
    /// the cell rather than about a response. So it is asserted on the cell.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cell
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether the cell is holding nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl EvidenceSource for RequestEvidence {
    fn collect(
        &self,
        _t: &gx_core::Transformation,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_engine::Result<Vec<Evidence>> {
        Ok(self
            .cell
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }
}

/// 🔴 45 §1's two keys, as the two questions a server has to answer about signing.
///
/// A trait rather than two `KeyPair` fields because **this crate does not know where keys live**:
/// req/56 §3 puts them in `~/.gx/keys/` at 0600 and that is gx-cli's declaration, and the manifest
/// explains why the dependency cannot run that way. The caller — `gx serve` in hand 6, a fixture in
/// this hand's suites — supplies the implementation.
pub trait ServerKeys: Send + Sync {
    /// The key this server signs verdict and commit receipts with (**E-M6-7**).
    fn signing(&self) -> &KeyPair;

    /// The adjudicator's key, by the id the request carries (**E-M6-15**, INV-S6).
    ///
    /// `None` for an id this server holds no key for. There is deliberately no fallback: see the
    /// module header.
    /// Borrowed rather than owned: `KeyPair` is deliberately not `Clone` (a secret that can be
    /// copied is a secret with more places to leak from), so an implementation holds its keys and
    /// hands out references.
    fn ruler(&self, key_id: &str) -> Option<&KeyPair>;
}

/// Everything a handler needs, cloned per request (all of it is `Arc` inside).
#[derive(Clone)]
pub struct AppState {
    engine: Arc<Mutex<Engine<RequestEvidence>>>,
    evidence: RequestEvidence,
    keys: Arc<dyn ServerKeys>,
    bearer: Bearer,
    archive: Arc<dyn ReceiptArchive>,
    idempotency: IdempotencyStore,
    origin: String,
    shutdown: Arc<crate::serve::Shutdown>,
}

impl AppState {
    /// Build the state a router is served with.
    ///
    /// * `engine` — already opened, with an adapter registered. Opening it is the caller's job for
    ///   the reason `.gx/` is: `Engine::open` takes a journal path and this crate does not know the
    ///   layout that produces one.
    /// * `evidence` — the same [`RequestEvidence`] the engine was opened with. Passing it twice is
    ///   ugly and is the honest shape: the engine owns the source and the handler has to be able to
    ///   fill it, and a getter on `Engine` would be a public accessor for a field only this crate
    ///   wants.
    /// * `receipts_dir` / `index_dir` — `.gx/receipts/` and `.gx/index/`, named by the caller.
    /// * `expected_signing_keyid` — **E-M6-7**: `.gx/config.toml`'s 「公開 keyid の参照のみ」. When
    ///   given and disagreeing with the key, the state is not built.
    ///
    /// # Errors
    /// [`ApiError`] `VALIDATION_ERROR` when `expected_signing_keyid` names a different key from the
    /// one supplied. 🔴 A **refusal to start**, not a warning: the value exists so that a project
    /// can record which key its receipts are signed by, and a server that started anyway would make
    /// the recorded value a comment.
    pub fn new(
        engine: Engine<RequestEvidence>,
        evidence: RequestEvidence,
        keys: Arc<dyn ServerKeys>,
        bearer: Bearer,
        index_dir: impl Into<PathBuf>,
        expected_signing_keyid: Option<&str>,
    ) -> Result<Self, ApiError> {
        if let Some(expected) = expected_signing_keyid {
            let actual = keys.signing().key_id();
            if actual != expected {
                return Err(ApiError::validation(format!(
                    "this project records its engine signing key as {expected:?} (E-M6-7, \
                     `.gx/config.toml`'s public keyid reference) and the key this server was \
                     started with is {actual:?}. Every receipt it issued would be unverifiable \
                     against the recorded id"
                )));
            }
        }
        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            evidence,
            keys,
            bearer,
            archive: Arc::new(NoArchive),
            idempotency: IdempotencyStore::at(index_dir),
            origin: crate::DEFAULT_ORIGIN.to_string(),
            shutdown: Arc::new(crate::serve::Shutdown::new()),
        })
    }

    /// The log's namespace for `GET /ledger/checkpoint` (42 §3.11). Defaults to
    /// [`crate::DEFAULT_ORIGIN`].
    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = origin.into();
        self
    }

    /// 🔴 Where receipts are kept between processes — `.gx/receipts/`, through the caller's own
    /// implementation (see [`crate::ReceiptArchive`] for why it is not a path).
    #[must_use]
    pub fn with_archive(mut self, archive: Arc<dyn ReceiptArchive>) -> Self {
        self.archive = archive;
        self
    }

    /// 🔴 The engine, locked. Every handler that touches it holds this for the whole operation.
    ///
    /// The guard is returned rather than a closure taken, because several handlers do two engine
    /// calls that must not be interleaved with another request's — `canonicalize` then `commit` is
    /// 43 T-8 followed by T-9's critical section, and a lock released between them would let a
    /// second request enter the section 44 §2.2 assumes is one operation.
    ///
    /// # Errors
    /// Never. A poisoned mutex is recovered rather than propagated: poisoning means a previous
    /// request panicked, 41 §6 counts a panic as a bug, and refusing every later request would turn
    /// one bug into an outage. The engine's own state is `Σ` plus a table rebuilt from it, so there
    /// is no torn invariant a poisoned lock is protecting.
    pub fn engine(&self) -> std::sync::MutexGuard<'_, Engine<RequestEvidence>> {
        self.engine.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The per-request evidence cell.
    #[must_use]
    pub fn evidence(&self) -> &RequestEvidence {
        &self.evidence
    }

    /// The two keys (see the module header).
    #[must_use]
    pub fn keys(&self) -> &dyn ServerKeys {
        self.keys.as_ref()
    }

    /// The token this server accepts.
    #[must_use]
    pub fn bearer(&self) -> &Bearer {
        &self.bearer
    }

    /// Where receipts survive a restart (see [`crate::ReceiptArchive`]).
    #[must_use]
    pub fn archive(&self) -> &dyn ReceiptArchive {
        self.archive.as_ref()
    }

    /// 44 §2.4's cache.
    #[must_use]
    pub fn idempotency(&self) -> &IdempotencyStore {
        &self.idempotency
    }

    /// The ledger namespace checkpoints are signed under.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// 🔴 The shutdown flag the router, the runtime and every `/stream` reader share.
    ///
    /// On the state rather than passed to [`crate::serve::serve`] alone, because the three stages
    /// have three readers in three places: `serve::guard` refuses on it, the reader task in
    /// [`crate::stream`] ends on it, and the runtime waits on the count beside it. A shutdown one of
    /// the three could not see would be a shutdown that hung (the stream) or that kept accepting
    /// work (the router).
    #[must_use]
    pub fn shutdown(&self) -> Arc<crate::serve::Shutdown> {
        Arc::clone(&self.shutdown)
    }

    /// 🔴 The server's clock, read in **one** place (則 2 / M6-28's shape, one surface along).
    ///
    /// 41 §6: 「乱数・時刻はengine境界で注入」. Every engine entry takes `at: Timestamp` and this is
    /// where the outside world's answer enters this crate. `m6_surface_doubt` counts gx-cli's
    /// `SystemTime::now(` call sites and asserts one; `crates/gx-api/tests/rule_two.rs` asserts the
    /// same about this crate, because 44 §2's fourteen endpoints are fourteen chances for a second
    /// clock and a receipt with two answers to 「when」 is a receipt nobody can order.
    ///
    /// # Panics
    /// Never: a `SystemTime` before the unix epoch is folded to zero rather than unwrapped.
    #[must_use]
    pub fn now(&self) -> Timestamp {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        Timestamp(since)
    }

    /// 🔴 The server's entropy, read in **one** place — 則 2's other half.
    ///
    /// 41 §6: 「乱数・時刻はengine境界で注入」, and `Engine::submit` is the one entry that takes a
    /// `rng_seed: u64`. `RandomState` is the same source `gx_cli::rng` uses and is named here for the
    /// same reason it is named there: whatever the source is, a **second** one is a second name, and
    /// `crates/gx-api/tests/rule_two.rs` counts the names.
    ///
    /// The seed reaches 42 §3.13's `DraftCreated` record, so a replay re-injects it and the
    /// determinism 44 §2.2's replay endpoint depends on is the engine's rather than this line's.
    #[must_use]
    pub fn seed(&self) -> u64 {
        use std::hash::{BuildHasher, Hasher};
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("archive", &"<dyn ReceiptArchive>")
            .field("idempotency", &self.idempotency.dir())
            .field("origin", &self.origin)
            .field("bearer", &self.bearer)
            .finish_non_exhaustive()
    }
}
