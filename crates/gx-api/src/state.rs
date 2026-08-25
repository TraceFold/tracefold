// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What a handler is handed: one engine behind one lock, two directories, a token and two keys.
//!
//! # 🔴 **M6-06, adopted (a)** (sem: SEM-gx-api-225) — one `Mutex`, every request serialised
//!
//! `Engine`'s eight entry points take `&mut self` and axum wants `Send + Sync` state. req/38 §47
//! adopted (a):
//!
//! > one `Arc<Mutex<Engine>>` = **serialises every request** (the most straightforward way to fulfil
//! > M5H5-6's "the caller's responsibility". Cost: NFR-003's 100 commits/s and the decay hand 7/hand 8 identified hit it head-on) (sem: SEM-gx-api-226)
//!
//! and registered the alternative — a per-object lock, M5H5-6(b) — in the D-7 ledger against the
//! firing condition "when AC-066 measured via serve breaks the SLA" (sem: SEM-gx-api-227). §47's note on why not to reach for
//! it first is the measurement argument: "**putting (b) in first loses the control for 'it's fast because we added it'**".
//!
//! The consequence is worth naming rather than leaving in a ruling: **this server has no
//! concurrency**. Two clients committing two unrelated transformations wait for each other. That is
//! a correct v0.1 and a bad v0.2, and the number that turns one into the other is hand 7's.
//!
//! # 🔴 What is not here, and why the resume machinery is absent
//!
//! req/38 §50 and §51 both fixed the line: "**resume/rehydrate is a CLI-specific layer** (serve holds the table)" (sem: SEM-gx-api-228).
//! A single-shot `gx verify` runs in a process whose in-flight table is empty (M5H3-5), so gx-cli
//! has [`Session::resume`](../../gx_cli/session/struct.Session.html#method.resume) to re-plan a row
//! back into existence. A long-lived engine never lost it. That is also why `.gx/drafts/` has no
//! reader here: 44 §0 is explicit that "HTTP `POST /candidates` runs submit+plan atomically as one,
//! so it does not expose the Draft-alone state" (sem: SEM-gx-api-229), so the intent body never has to cross a process boundary and
//! req/88 §3 Λ2's one counter-example is a cost the CLI pays alone (req/56 §2's drafts row says so
//! since M6H4-5).
//!
//! # 🔴 45 §1's two keys, in the type (**E-M6-7** / **E-M6-15**)
//!
//! 45 §1 separates an **Actor signing key** from a **Ledger signing key**, and req/38 §50 M6H3-4
//! recorded that gx-cli does not implement the separation: `gx verify` and `gx commit` sign with the
//! key the transformation's own `Actor` names, so a receipt says "this actor's key attests this
//! change was verified" (sem: SEM-gx-api-230) rather than "the engine attests it".
//!
//! A server cannot copy that. It is not the actor; the actor is a client on the other side of a
//! socket that never sends a private key. So [`ServerKeys`] has two methods and they are the two
//! keys:
//!
//! * [`ServerKeys::signing`] — **the server's own key**, used for every `VerdictReceipt` and
//!   `CommitReceipt` this surface causes. E-M6-7's design: "`.gx/config.toml` carries a reference to the engine's signing keyid
//!   (within req/56 §2's 'a reference to the public keyid only' frame)" (sem: SEM-gx-api-231), which is why [`AppState::new`] takes an
//!   `expected_signing_keyid` and refuses to start when it disagrees. The **reader** of that file is
//!   `gx serve` (hand 6, the hand with the flag); the check is here, where a mismatch can stop the
//!   surface from existing rather than be noticed in a log.
//! * [`ServerKeys::ruler`] — the **adjudicator's** key, looked up by the id the request carries.
//!   **E-M6-15** made `--actor-key` required on `gx escalation` and INV-S6 is why: "there is no default
//!   under which the party being ruled on approves themselves" (sem: SEM-gx-api-232). So there is no default and no fallback to
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
use gx_engine::store::ProcessLock;
use gx_engine::{Engine, EvidenceSource};
use gx_witness::{Evidence, KeyPair};

use crate::auth::Bearer;
use crate::idempotency::IdempotencyStore;
use crate::problem::ApiError;
use crate::{DraftArchive, NoArchive, NoDrafts, ReceiptArchive};

/// 🔴 Evidence a **single request** supplies, injected through 41 §6's own trait.
///
/// 44 §2.2's `POST /candidates/{id}/verify` body is `{ evidence: Evidence[], record_only: bool|null }`
/// and `Engine::open` fixes its `EvidenceSource` for the engine's whole life. The second half of
/// that pair — `record_only` — was solved by **M6-08, adopted (a)** (sem: SEM-gx-api-233), a per-call argument, precisely because
/// (b) "serve swaps the mode via `&mut self` on a per-request basis" was ruled "must not be adopted": it
/// leaks one request's posture into another's.
///
/// This cell is (b)'s shape for the field M6-08 did **not** move, and it is safe here for a reason
/// that is stated rather than assumed: **the whole engine is behind one lock** (M6-06, adopted (a); sem: SEM-gx-api-234), and
/// the load and the `Engine::verify` that reads it happen inside one hold of that lock. There is no
/// interleaving to leak through. The moment M5H5-6(b)'s per-object lock fires, this stops being
/// true — so the honest repair is the one M6-08 already made for `mode`: an evidence argument on
/// `Engine::verify`. Raised as **M6H5-6**, with the firing condition tied to the same ruling.
#[derive(Clone, Debug, Default)]
pub struct RequestEvidence {
    cell: Arc<Mutex<Vec<Evidence>>>,
}

impl RequestEvidence {
    /// An empty source: 44 §1.2's "when omitted, only gx-gate's built-in InvariantCheck/Cedar evaluation" (sem: SEM-gx-api-235).
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
    /// The property [`RequestEvidence::clear`] exists for, made observable: "a request's evidence
    /// does not outlive the request" (sem: SEM-gx-api-236). Without this accessor the clear is invisible to a behavioural
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

    /// 🔴 **R4 / `req/225` H-02** — the key a **stored document** names, for checking its
    /// signature.
    ///
    /// A third question, and it is not either of the two above. [`ServerKeys::signing`] answers
    /// "which hand does this server write with", which is about *today*; [`ServerKeys::ruler`]
    /// answers "may this person adjudicate", which is an authorisation. This one answers "whose
    /// signature is on the document in front of me", which is about a receipt that may be older
    /// than this process and older than the key it currently signs with.
    ///
    /// `undo_witness` used `signing()` for it, and `req/225` H-02 measured both halves of what
    /// that costs: a receipt signed before a `gx key rotate` stops verifying — permanently, for
    /// every commit in the project's history — and the CLI face, which was reading the *actor's*
    /// key for the same job, refused every undo on any deployment that passes `--signing-key`.
    ///
    /// The default resolves the server's own key and then the keys it was given for adjudication,
    /// which is every key file `gx serve` found in req/56 §3's store at start-up. An embedder that
    /// keeps its trusted public keys somewhere else overrides this; one that does not gets a
    /// fail-closed answer (`None` means "cannot check", and the caller refuses).
    fn verifier(&self, key_id: &str) -> Option<&KeyPair> {
        let signing = self.signing();
        if signing.key_id() == key_id {
            return Some(signing);
        }
        self.ruler(key_id)
    }
}

/// 🔴 **R11 / `req/240` M-04** — the two `Nature::Meta` files, asked about by a **long-lived**
/// process.
///
/// A handle and not a path, for [`crate::ReceiptArchive`]'s reason: `.gx/` is req/56's layout and
/// this crate does not name it. `gx-cli`'s `serve::build` implements it over `Layout` and hands it
/// over; a server built by an embedder that has no such directory simply has none, and answers
/// exactly as it did before R11.
///
/// # What it is for
///
/// R10 made every CLI writer refuse a project whose `.gx/VERSION` or `.gx/config.toml` is gone.
/// `gx serve` asks the same question — and asked it **once**, at start-up. `req/240` M-04 measured
/// the gap: with a server up, `rm .gx/VERSION` changed nothing at all on the wire (`/healthz`
/// answered `{"status":"ok"}`, a candidate was created, verified and committed, the ledger grew a
/// leaf) while every CLI verb on the same project was refusing `DECLARATION_ABSENT`. One project,
/// two answers, decided by which door you came in.
pub trait ProjectMeta: Send + Sync {
    /// `None` when both files are where the project left them.
    fn absent(&self) -> Option<MetaAbsent>;

    /// 🔴 **R11 / `req/240` M-01** — a cheap witness of the two append-only files, taken **without**
    /// the engine's `Mutex` and without reading a byte of them.
    ///
    /// Length and modification time of the journal and the ledger, in one string. It answers one
    /// question and only one: *could* what this server last read still be current. A caller that
    /// gets the same witness twice may serve its cached answer; anything else — a rewrite, a
    /// truncation, an append, a file that has gone — changes it and forces a rebuild.
    ///
    /// `None` for a server with no project directory (an embedder), which means "cannot tell" and
    /// makes the caller rebuild every time, exactly as it did before R11.
    fn witness(&self) -> Option<String>;
}

/// Which of the two files is gone, and where it should be.
#[derive(Clone, Debug)]
pub struct MetaAbsent {
    /// `true` for `.gx/VERSION`, `false` for `.gx/config.toml` — the declaration is named first
    /// because nothing else is a statement about the file that is actually missing.
    pub declaration: bool,
    /// The path, for the `detail` an operator reads.
    pub path: String,
}

/// 🔴 **R11 / `req/240` M-01** — what `GET /v1/healthz` answers from, taken under the engine's
/// `Mutex` and then read outside it.
#[derive(Clone, Debug)]
pub(crate) struct HealthSnapshot {
    taken: std::time::Instant,
    /// What [`ProjectMeta::witness`] said when this snapshot was taken.
    witness: Option<String>,
    pub(crate) agrees: bool,
    pub(crate) rows: usize,
    pub(crate) journal: usize,
    pub(crate) ledger: u64,
    /// 🔴 **R32 / `req/392` M-02** — which term of `Engine::journal_intact` was false, so
    /// that the sentence `/healthz` prints is the sentence for the file in front of it. `None`
    /// is the old `true`.
    pub(crate) journal_departure: Option<gx_engine::JournalDeparture>,
    pub(crate) rolled_back: Option<String>,
}

/// 🔴 **R11 / `req/240` M-01** — how stale `GET /v1/healthz` may be about a change **another
/// process** made.
///
/// 🔴 **This is a ceiling on how long a snapshot may be reused, not the detector's resolution.**
/// The first version of this lane made it the whole story and the existing suites said no, in five
/// places at once: `serve_runtime_r2`'s `a_ledger_that_moved_makes_healthz_say_so_by_name`,
/// `r3`'s `a_same_length_rewrite_of_the_ledger_stops_the_writing`, `r4`'s
/// `h03_a_same_length_rewrite_of_the_journal_stops_the_writing` and `s3_a_journal_that_shrank…`,
/// and `r5`/`r6`'s head arms — every one of them rewrites a file **under** a running server and
/// asks the very next probe about it. A time-only cache answered `200 {"status":"ok"}` to all of
/// them. §7.12 (c)'s sentence — "caching its answer on an endpoint whose whole job is to notice a
/// disk that changed would be a monitor reporting the last time it looked" — was right, and the
/// probes are where it is written down as a machine.
///
/// So the reuse is **witnessed**: [`ProjectMeta::witness`] stats the journal and the ledger
/// (2 syscalls, no lock, no read) and a snapshot is only reused while that string is unchanged.
/// ∴ every change those suites make forces a rebuild on the next probe, and what the window bounds
/// is the one case the witness cannot see: a change that leaves both files byte-identical in
/// length **and** modification time. Three facts fix the rest:
///
/// * a write **through this server** is never stale: [`WritingEngine`]'s `Drop` drops the snapshot,
///   so the next probe rebuilds it. That is the "invalidate at the point the head moves" half;
/// * a write by another `gx` (a CLI commit, another server) changes the witness and is seen by the
///   next probe;
/// * a flood of identical probes against an unchanged project rebuilds at most four times a second
///   between them, which is the whole of what this buys.
pub const HEALTH_SNAPSHOT_MAX_AGE: std::time::Duration = std::time::Duration::from_millis(250);
/// Everything a handler needs, cloned per request (all of it is `Arc` inside).
#[derive(Clone)]
pub struct AppState {
    engine: Arc<Mutex<Engine<RequestEvidence>>>,
    evidence: RequestEvidence,
    keys: Arc<dyn ServerKeys>,
    bearer: Bearer,
    archive: Arc<dyn ReceiptArchive>,
    /// 🔴 **T6 condition ① L2** — where the intent bodies are, so that a row this process did not
    /// plan can be rebuilt rather than refused (`req/38` §148 ruling 1(iii), lane R2). See
    /// [`crate::DraftArchive`] for why it is a handle and not a path.
    drafts: Arc<dyn DraftArchive>,
    /// 🔴 **T6 condition ② (single writer, per operation)** — the project's `.gx/LOCK`, injected
    /// (`req/38` §148 ruling 1(ii)).
    ///
    /// A handle and not a path, for [`ReceiptArchive`](crate::ReceiptArchive)'s reason: `.gx/` is
    /// req/56's layout and gx-api does not name it. `gx-cli`'s `serve::build` opens the file and
    /// hands it over, so a server built by something else simply has no lock — and answers exactly
    /// as it did before DR-43-2, which is honest for an embedder that is the only writer.
    writer_lock: Option<Arc<ProcessLock>>,
    /// 🔴 What the start-up gate measured, for the one structured line 44 §1.2 asks for.
    ///
    /// `gx-cli`'s `serve::build` runs the gate (`recover`, then `ledger_agrees`) before this state
    /// exists, and `gx-cli`'s `serve::start_line` prints the line after the listener does. The facts
    /// travel between them here rather than as a second log line, because `crates/gx-cli/tests/
    /// ac_056.rs` reads the **first** line and asserts it is `gx.serve.started` — a second line
    /// before it would be a start-up log a client's script could not parse.
    startup: serde_json::Value,
    idempotency: IdempotencyStore,
    origin: String,
    shutdown: Arc<crate::serve::Shutdown>,
    /// 🔴 **R11 / `req/240` M-04** — the two `Nature::Meta` files, injected. See [`ProjectMeta`].
    meta: Option<Arc<dyn ProjectMeta>>,
    /// 🔴 **R11 / `req/240` M-01** — the last answer `GET /v1/healthz` built, and when.
    health: Arc<Mutex<Option<HealthSnapshot>>>,
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
    /// * `expected_signing_keyid` — **E-M6-7**: `.gx/config.toml`'s "a reference to the public keyid only" (sem: SEM-gx-api-237). When
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
            drafts: Arc::new(NoDrafts),
            writer_lock: None,
            startup: serde_json::Value::Null,
            meta: None,
            health: Arc::new(Mutex::new(None)),
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

    /// 🔴 Where the intent bodies are kept between processes — `.gx/drafts/`, through the caller's
    /// own implementation (see [`crate::DraftArchive`] for why it is not a path).
    #[must_use]
    pub fn with_drafts(mut self, drafts: Arc<dyn DraftArchive>) -> Self {
        self.drafts = drafts;
        self
    }

    /// 🔴 What the start-up gate measured (`req/38` §148). See [`AppState::startup`].
    #[must_use]
    pub fn with_startup(mut self, startup: serde_json::Value) -> Self {
        self.startup = startup;
        self
    }

    /// What the start-up gate measured: `recover` counts and `ledger_agrees`.
    #[must_use]
    pub fn startup(&self) -> &serde_json::Value {
        &self.startup
    }

    /// 🔴 The project's writer lock (`req/38` §148, DR-43-2). See [`AppState::engine_for_write`].
    #[must_use]
    pub fn with_writer_lock(mut self, lock: Arc<ProcessLock>) -> Self {
        self.writer_lock = Some(lock);
        self
    }

    /// 🔴 **R11 / `req/240` M-04** — the project's `.gx/VERSION` and `.gx/config.toml`, asked at
    /// every write and at every health probe rather than once at start-up. See [`ProjectMeta`].
    #[must_use]
    pub fn with_project_meta(mut self, meta: Arc<dyn ProjectMeta>) -> Self {
        self.meta = Some(meta);
        self
    }

    /// 🔴 **R11 / `req/240` M-04** — the refusal a writer gets when one of the two files is gone.
    ///
    /// `None` for a server with no [`ProjectMeta`] (an embedder that has no `.gx/`) and for a
    /// project whose files are both there. The codes and the sentences are R10's, unchanged: this
    /// is the same fact the CLI refuses on, asked by the process that had asked it once.
    fn meta_refusal(&self) -> Option<ApiError> {
        let absent = self.meta.as_ref()?.absent()?;
        Some(if absent.declaration {
            ApiError::new(
                crate::gx_code::DECLARATION_ABSENT,
                crate::gx_code::DECLARATION_ABSENT_TITLE,
                format!(
                    "`{}` is not there. This server read it when it started and it has gone since:                      R7 binds that file's digest into the signed head, so a project without it                      cannot be written to until the declaration is back, and a server that went on                      writing would be adding leaves under a head whose declaration digest matches                      nothing. `gx repair` reports the whole project and `gx repair --yes` writes                      the declaration back and says that it did (req/240 M-04, req/238 H-01)",
                    absent.path
                ),
            )
        } else {
            ApiError::new(
                crate::gx_code::CONFIG_ABSENT,
                crate::gx_code::CONFIG_ABSENT_TITLE,
                format!(
                    "`{}` is not there. This server read it when it started and it has gone since:                      43 §7.9 (b) calls it the file that decides which key a recovery signs with, so                      the writer's door stays shut until it is back rather than let this project                      carry on under a key it did not choose. `gx repair --yes` writes the shipping                      default and says so; the `engine_signing_keyid` line is the operator's to put                      back (req/240 M-04, req/238 H-01)",
                    absent.path
                ),
            )
        })
    }

    /// 🔴 **R11 / `req/240` M-04** — the path of a `Nature::Meta` file this project has lost, if
    /// it has lost one.
    ///
    /// The read side of [`AppState::meta_refusal`], for `GET /v1/healthz`: the same `stat` the
    /// write door takes, so that a monitor and a writer cannot disagree about whether the project
    /// can be written to.
    #[must_use]
    pub fn meta_missing(&self) -> Option<String> {
        self.meta.as_ref()?.absent().map(|absent| absent.path)
    }

    /// 🔴 **R11 / `req/240` M-01** — what `GET /v1/healthz` answers from.
    ///
    /// # Why there is a snapshot at all
    ///
    /// `req/240` M-01 measured the endpoint 44 §2.5 keeps **outside** the bearer guard taking the
    /// engine's `Mutex` on every request: 200 concurrent probes on a 400-commit project answered
    /// in 1,597 ms wall — 125 req/s against a median of 8.81 ms, so a concurrency of exactly one —
    /// and an unauthenticated 16-thread flood moved an authenticated `POST /v1/candidates` from
    /// 40.78 ms to 168.06 ms (×4.1). The cost itself is R4's detector (the lockless catch-up that
    /// notices a journal rewritten under a running server) and R10 declined to cache it, in as many
    /// words: "caching its answer on an endpoint whose whole job is to notice a disk that changed
    /// would be a monitor reporting the last time it looked".
    ///
    /// That sentence is right about caching without a **bound** and about caching without an
    /// invalidation. This has both: [`HEALTH_SNAPSHOT_MAX_AGE`] is the whole of the staleness, and
    /// a write through this server drops the snapshot as its guard falls. So the detector still
    /// runs — four times a second under any load, and on every probe of a monitor slower than that
    /// — and what a flood can no longer do is decide how often the engine's lock is taken.
    ///
    /// # Errors
    /// Engine-mapped, if the journal or the ledger cannot be read at all.
    pub(crate) fn health_snapshot(&self) -> Result<HealthSnapshot, ApiError> {
        // Taken **before** the cache is consulted, and before the catch-up below, so the error is
        // always in the safe direction: a change that lands between this `stat` and the read
        // stamps the snapshot with the **older** witness, and the next probe rebuilds. The other
        // order would stamp a fresh witness on data read before it.
        let witness = self.meta.as_ref().and_then(|meta| meta.witness());
        if witness.is_some() {
            let cached = self.health.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(snapshot) = cached.as_ref() {
                if snapshot.witness == witness && snapshot.taken.elapsed() < HEALTH_SNAPSHOT_MAX_AGE
                {
                    return Ok(snapshot.clone());
                }
            }
        }
        let engine = self.engine_refreshed()?;
        let snapshot = HealthSnapshot {
            taken: std::time::Instant::now(),
            witness,
            agrees: engine.ledger_agrees(),
            rows: engine.shadow().len(),
            journal: engine.committed_len(),
            ledger: engine.ledger().log().len(),
            journal_departure: engine.journal_departure(),
            rolled_back: engine
                .rolled_back()
                .or_else(|| engine.head_invalid())
                .map(str::to_string),
        };
        drop(engine);
        *self.health.lock().unwrap_or_else(PoisonError::into_inner) = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// 🔴 **T6 conditions ① and ②** — the engine, locked against this process's other requests
    /// **and** against every other `gx` process, and caught up to the end of the log.
    ///
    /// The two locks are two different exclusions and both are needed. The `Mutex` keeps two of this
    /// server's own requests out of 43 T-8/T-9's critical section (that is [`AppState::engine`]'s
    /// job and has been since M6-06). `.gx/LOCK` keeps a **second process** out of it, which
    /// `req/182` H-01 measured nothing was doing: a `gx commit` run beside a live `gx serve`
    /// produced a ledger leaf at an index the server's in-memory tree had already staged, and the
    /// next open truncated it away — a commit that answered `200` and then could not prove its own
    /// inclusion.
    ///
    /// The order is `Mutex` first, then the file: the file lock is never waited on, so no thread can
    /// hold one and block on the other. Once both are held, [`Engine::catch_up`] reads whatever
    /// arrived while we were not looking, which is the half that makes the lock worth having — an
    /// excluded writer that wrote a stale index would be no better than an unexcluded one.
    ///
    /// # Read handlers deliberately do not take this
    ///
    /// `GET` is answered from [`AppState::engine`] with the `Mutex` alone. A read that took the file
    /// lock would answer `503` while a CLI verb was writing, which would make `/healthz` fail for a
    /// project that is perfectly well; and the freshness it would buy is bounded anyway, because
    /// this server catches up on every write it performs. The limit is declared rather than papered
    /// over: **between two writes, a `GET` can be one CLI commit behind** (`req/190` §10).
    ///
    /// # Errors
    /// [`ApiError`] `BUSY` (503, `Retry-After: 1`) if another process holds the lock;
    /// `INTERNAL`/engine-mapped if the catch-up cannot read the journal or the ledger.
    pub fn engine_for_write(&self) -> Result<WritingEngine<'_>, ApiError> {
        // 🔴 **R11 / `req/240` M-04** — the declaration and the settings, asked here rather than
        // only at start-up. A long-lived process reads `.gx/` once and then writes for days; the
        // CLI asks this question at every verb, and one project must not have two answers.
        if let Some(refusal) = self.meta_refusal() {
            return Err(refusal);
        }
        let mut engine = self.engine();
        let held = match &self.writer_lock {
            Some(lock) => Some(
                lock.acquire("gx serve")
                    .map_err(|e| ApiError::from_engine(&e))?,
            ),
            None => None,
        };
        engine.catch_up().map_err(|e| ApiError::from_engine(&e))?;
        // 🔴 **DR-43-6 (`req/38` §153), the asymmetry `req/215` H-01 measured.** `gx-cli`'s
        // `Session::settle` has asked this since R1 and refuses the verb when the answer is `false`.
        // This function did not ask at all, and `store.rs`'s own documentation said it did -- so the
        // claim "a writer reads to the end of the log before it writes" held on one of the two
        // writers. What that cost, measured: with the ledger cut to 100 bytes under a live server,
        // `POST /candidates` answered `201`, `POST .../verify` answered `200`, `POST .../commit`
        // answered `200` **with a signed receipt**, and `GET /ledger/checkpoint` **signed** a
        // checkpoint claiming three leaves over a file that held none. Only the next restart
        // refused. Every receipt issued inside that window names an inclusion nobody can prove.
        //
        // The refusal is `500` and not `BUSY`: `BUSY` means "send the same thing again in a moment",
        // and there is no moment at which this becomes true again without a repair. It is not
        // `VALIDATION_ERROR` either (which is where `gx_engine::Error::Malformed` maps, and which
        // would blame the caller for the state of the server's disk). ~~44 §2.3 has no code for
        // "this project's two files describe different trees"; minting one is a surface addition
        // and therefore a DR, so this rides on `INTERNAL` and says the whole of it in `detail`.
        // Raised in the DR-43-6 filing.~~
        //
        // 🔴 **The DR was ruled** (`req/38` §156 ruling 2(a)): the code is
        // `gx_code::LEDGER_DISAGREES` (500), the struck sentence is kept because a reader who
        // trusted it would look for `INTERNAL` on this path, and `detail` still says the whole of
        // it -- the word names the condition, it does not replace the explanation.
        //
        // 🔴 **R4 / `req/225` H-03** — and which of the two files moved. `ledger_agrees` is
        // `false` for a journal that was rewritten underneath this process as well as for two
        // files that count differently (`Engine::journal_intact` is folded into it), and an
        // operator sent to look at the ledger for damage that is in the journal loses the hour.
        if !engine.ledger_agrees() {
            // 🔴 **R6 / DR-43-11** — see `handlers::healthz` for why the two clauses are joined
            // here rather than inside the outer format's arguments.
            // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated. See
            // `crate::journal_and_head_note`: the roll-back clause exists because in its own
            // condition every word the journal clause prints is false, and `{}{}` printed both.
            // 🔴 **R7 / `req/232` H-01** — or the head this binary would not read numbers off.
            let note = crate::journal_and_head_note(
                engine.journal_departure(),
                engine.rolled_back().or_else(|| engine.head_invalid()),
            );
            return Err(ApiError::ledger_disagrees(format!(
                "this project's journal witnesses {} commit(s) and its ledger holds {} leaf/leaves, \
                 and `ledger_agrees` is false: the two files are describing different trees.{} \
                 Writing to a disagreement makes it worse, so this server refuses the write instead \
                 (req/215 H-01, req/38 §153 DR-43-6). `gx repair` reports what is wrong and `gx repair --yes` runs 43 §7's recovery under the project lock (DR-43-8); `gx replay <ID>` names the rows that differ",
                engine.sigma().ledger().len(),
                engine.ledger().log().len(),
                note,
            )));
        }
        Ok(WritingEngine {
            engine,
            _held: held,
            health: Arc::clone(&self.health),
        })
    }

    /// 🔴 **DR-43-4 — one sweep of 43 T-6, from the process that has a clock** (`req/38` §148
    /// ruling 1(iv), lane R2).
    ///
    /// 43 §9's INV-L1 and INV-L2 say every `Candidate`, `Verifying` and `Escalated` reaches a
    /// terminal state in finite time. Until this lane they were **sentences and not properties**:
    /// `Engine::reap` had zero production callers (`req/182` H-03), expiry happened only when
    /// somebody called `verify`, `escalation` or `cancel` on the row itself, and a transformation
    /// nobody ever called about waited for ever. `crates/gx-cli/src/serve.rs`'s start-up line said
    /// so in as many words — `reaper: "none: …"` — which is why the absence was findable and is now
    /// the thing being removed.
    ///
    /// # 🔴 Rule 2 is why this is a method and not a closure with a clock in it
    ///
    /// 41 §6 injects time at the engine boundary, and [`AppState::now`] is this crate's single
    /// reading of it (`crates/gx-api/tests/rule_two.rs` counts the names). The timer decides
    /// **when to ask**; it does not decide **what time it is**. A reaper that called
    /// `SystemTime::now()` for itself would be a second clock, and a receipt with two answers to
    /// "when" is a receipt nobody can order.
    ///
    /// # What a sweep may and may not do
    ///
    /// It takes the same writer's road every other write takes — [`AppState::engine_for_write`] —
    /// so it holds `.gx/LOCK`, catches up first, and refuses to run over a journal and a ledger
    /// that disagree. `BUSY` is **skipped and not retried**: another `gx` is writing, that writer
    /// sweeps at its own entry (`gx_cli::session`'s settle), and a reaper that queued behind a
    /// lock would be a background task holding a project open. Every other refusal is returned, so
    /// a caller can print it once rather than swallow it in a loop.
    ///
    /// It reaps the rows **this process holds**, which after `Engine::catch_up` are the rows no
    /// other writer has touched. A Σ-shadow row — one another process planned — has a state and no
    /// `since`, so 43 T-6's deadline is not computable for it at all; that is `StateRow`'s shape
    /// (`replay.rs`) and moving it changes Σ's canonical bytes and both sides of AC-039. Declared
    /// in `req/221` §7 rather than approximated.
    ///
    /// # Errors
    /// Engine-mapped. `BUSY` is not an error here — it answers `Ok(vec![])`.
    pub fn reap_once(&self) -> Result<Vec<gx_core::TransformationId>, ApiError> {
        let now = self.now();
        let mut engine = match self.engine_for_write() {
            Ok(engine) => engine,
            Err(e) if e.code == crate::gx_code::BUSY => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        engine.reap(now).map_err(|e| ApiError::from_engine(&e))
    }

    /// 🔴 **DR-43-6 / `req/215` H-05** -- the engine for a `GET`, caught up first, with no file lock.
    ///
    /// [`AppState::engine`] takes the `Mutex` and answers from whatever this process last read.
    /// `req/215` H-05 measured what that means once a second writer exists: five CLI commits later
    /// the list still held one row, three seconds of waiting changed nothing, and
    /// `GET /ledger/checkpoint` **signed** `tree_size: 1` over a ledger holding six leaves. The
    /// declared limit -- "between two writes, a `GET` can be one CLI commit behind" -- was neither a
    /// bound in commits nor a bound in time.
    ///
    /// This still does not take `.gx/LOCK` (the reason is unchanged: a read that took it would
    /// answer `503` while a CLI verb was writing). It takes the lockless read
    /// [`Engine::catch_up_unlocked`], which folds whole records, stops in front of a half-written
    /// one, and re-opens a moved ledger **read-only** so that a reader can never repair anything.
    ///
    /// # Errors
    /// Engine-mapped, if the journal or the ledger cannot be read at all.
    pub fn engine_refreshed(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Engine<RequestEvidence>>, ApiError> {
        let mut engine = self.engine();
        engine
            .catch_up_unlocked()
            .map_err(|e| ApiError::from_engine(&e))?;
        Ok(engine)
    }

    /// 🔴 **R7 / `req/232` M-08** — the engine for a handler that is about to **sign**, caught up
    /// under `.gx/LOCK`.
    ///
    /// # Why a read that signs is not a read
    ///
    /// [`AppState::engine_refreshed`]'s note explains why a `GET` does not take the file lock: it
    /// would answer `503` while a CLI verb was writing, and the staleness it buys back is bounded
    /// and declared. That reasoning holds for every read this server answers **except** one. The
    /// audit ran two servers on one project, committed through the second, and watched the first
    /// answer `GET /v1/ledger/checkpoint` with a **signed** checkpoint over the tree it last saw,
    /// stamped with the current time. A stale read is a read; a stale *signature* is a document
    /// that will outlive the moment and cannot be told apart from an honest one — `tree_size` and
    /// `root_hash` from one instant, `timestamp` from another, one key over both.
    ///
    /// ∴ the signing road takes the same lock every writer takes and catches up inside it. What it
    /// costs is stated rather than hidden: while another `gx` holds `.gx/LOCK`, this endpoint
    /// answers `BUSY` (503, `Retry-After: 1`) instead of a stale signature. `BUSY` is the right
    /// word here — unlike `LEDGER_DISAGREES`, there **is** a moment at which it becomes true again.
    ///
    /// # Errors
    /// [`ApiError`] `BUSY` if another process holds the lock; engine-mapped if the catch-up cannot
    /// read the journal or the ledger.
    pub fn engine_for_signing(&self) -> Result<WritingEngine<'_>, ApiError> {
        // 🔴 **R11 / `req/240` M-04** — a signature is a write, so it asks the same question.
        if let Some(refusal) = self.meta_refusal() {
            return Err(refusal);
        }
        let mut engine = self.engine();
        let held = match &self.writer_lock {
            Some(lock) => Some(
                lock.acquire("gx serve")
                    .map_err(|e| ApiError::from_engine(&e))?,
            ),
            None => None,
        };
        engine.catch_up().map_err(|e| ApiError::from_engine(&e))?;
        Ok(WritingEngine {
            engine,
            _held: held,
            health: Arc::clone(&self.health),
        })
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

    /// Where the intent bodies survive a restart (see [`crate::DraftArchive`]).
    #[must_use]
    pub fn drafts(&self) -> &dyn DraftArchive {
        self.drafts.as_ref()
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

    /// 🔴 The server's clock, read in **one** place (Rule 2 / M6-28's shape, one surface along; sem: SEM-gx-api-238).
    ///
    /// 41 §6: "randomness and time are injected at the engine boundary" (sem: SEM-gx-api-239). Every engine entry takes `at: Timestamp` and this is
    /// where the outside world's answer enters this crate. `m6_surface_doubt` counts gx-cli's
    /// `SystemTime::now(` call sites and asserts one; `crates/gx-api/tests/rule_two.rs` asserts the
    /// same about this crate, because 44 §2's fourteen endpoints are fourteen chances for a second
    /// clock and a receipt with two answers to "when" (sem: SEM-gx-api-240) is a receipt nobody can order.
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

    /// 🔴 The server's entropy, read in **one** place — Rule 2's other half (sem: SEM-gx-api-241).
    ///
    /// 41 §6: "randomness and time are injected at the engine boundary" (sem: SEM-gx-api-242), and `Engine::submit` is the one entry that takes a
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
            .field("drafts", &"<dyn DraftArchive>")
            .field("idempotency", &self.idempotency.dir())
            .field("origin", &self.origin)
            .field("bearer", &self.bearer)
            .finish_non_exhaustive()
    }
}

/// 🔴 The engine held for one write: this server's `Mutex` and the project's `.gx/LOCK` together.
///
/// Both are released when this value is dropped, in declaration order — the guard first, then the
/// file lock — which is the reverse of the order they were taken in and is what `Drop` gives for
/// free. A paired `unlock()` call would leak the file lock on every `?` between here and the answer,
/// and a leaked exclusive lock is the next writer's deadlock (see
/// [`gx_engine::store::LockHeld`]).
pub struct WritingEngine<'a> {
    engine: std::sync::MutexGuard<'a, Engine<RequestEvidence>>,
    _held: Option<gx_engine::store::LockHeld<'a>>,
    /// 🔴 **R11 / `req/240` M-01** — the health snapshot, dropped when this guard falls.
    ///
    /// The invalidation point, and it is a `Drop` rather than a call at the end of each handler
    /// because there are six of them plus the reaper plus the checkpoint road: a rule written in
    /// eight places is a rule the ninth writer will not know about. Whether the write succeeded is
    /// deliberately not consulted — a refused write can still have caught up, recovered or
    /// quarantined on its way in, and a snapshot rebuilt for nothing costs one catch-up.
    health: Arc<Mutex<Option<HealthSnapshot>>>,
}

impl Drop for WritingEngine<'_> {
    fn drop(&mut self) {
        *self.health.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

impl std::ops::Deref for WritingEngine<'_> {
    type Target = Engine<RequestEvidence>;
    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl std::ops::DerefMut for WritingEngine<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine
    }
}
