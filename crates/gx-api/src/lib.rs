#![forbid(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The Glovrex HTTP surface (44 §2) — **thirteen synchronous endpoints**.
//!
//! # 🔴 Rule 1 (sem: SEM-gx-api-174) — this crate holds no semantic authority
//!
//! The same three absences gx-cli carries, checked by the same instrument
//! (`crates/gx-canon/tests/authority_boundary.rs`): no canonical encode (41 §6), no `Verdict`
//! construction (41 §4), no `Lifecycle` write (42 §1.3-3). req/88 §3 Λ1 explains why this is a
//! design claim and not hygiene — 48 §4 wrote it about the Console in TypeScript ("TS itself never
//! holds verification logic"; sem: SEM-gx-api-175) and Λ1 is the same sentence one language in.
//!
//! For this crate the rule has a sharper edge than for the CLI, because an HTTP handler is where
//! "just compute the answer here, it is faster" (sem: SEM-gx-api-176) is most tempting: `GET /candidates/{id}` returns
//! `{transformation, state, verdict, fingerprint}` and every one of those four has an engine
//! accessor already (req/88 §2.2). The surface's job is to call them.
//!
//! # What hand 5 builds
//!
//! Every endpoint of 44 §2.1 except `GET /stream`, plus the four decisions §47 put in front of them:
//!
//! * [`gx_code`] — **M6-09**'s one map, thirty-three refusal kinds onto twelve codes, with the folds
//!   listed rather than implied (req/88 §3 Λ4: "the discipline is not 'don't fold' but 'write down what was folded'"; sem: SEM-gx-api-177);
//! * [`auth`] — **M6-10**'s static Bearer, the loopback default, and "the check's absence" (sem: SEM-gx-api-178) as a string;
//! * [`idempotency`] — **M6-11**'s hand-written `Idempotency-Key`, persisted, with the one line that
//!   says what it does **not** protect;
//! * [`state`] — **M6-06, adopted (a)**'s single lock (sem: SEM-gx-api-179), and 45 §1's two keys as two methods (**E-M6-7** /
//!   **E-M6-15**).
//!
//! # 🔴 The count, and a disagreement with the requirement document
//!
//! req/88 §6.2 gives this hand "44 §2's synchronous face, **11** endpoints (everything except `/stream` and the list family)" (sem: SEM-gx-api-180) and
//! hand 1's `SPECIFIED_ENDPOINTS` array — read off 44 §2.1 — has **fourteen** rows. Fourteen minus
//! `/stream` is **thirteen**, and 44 has no list endpoints at all (§2.7: "the endpoints this document
//! specifies include no list-family endpoint"; sem: SEM-gx-api-181), so M6-05's three are hand 6's *additions* and
//! cannot be subtracted from a count of 44's own. The parenthetical is the operative text and the
//! numeral is an enumeration error; thirteen are implemented and the discrepancy is raised as
//! **M6H5-1** under discipline 49's lane-side reconciliation (sem: SEM-gx-api-182).
//!
//! # What hand 6 takes
//!
//! `GET /stream` (M6-12's event map, M6-13's resume cursor), `gx serve`'s runtime and graceful
//! shutdown, and M6-05's three list endpoints. This crate declares no `tokio` for that reason: the
//! hand that writes `#[tokio::main]` declares it (req/38 §38, adopted (a); sem: SEM-gx-api-183).

pub mod attach_sources;
pub mod auth;
pub mod extract;
pub mod gx_code;
pub mod handlers;
pub mod idempotency;
pub mod list;
pub mod observations;
// 🔴 **R16 / `req/262` H-01** — this crate's one road to a standard stream. The window of the
// census is the **binary**, and this crate is inside `gx`; see [`notes`] for why the road is per
// crate and where the denominator is written down.
pub mod notes;
pub mod problem;
pub mod rfc3339;
pub mod serve;
pub mod state;
pub mod stream;
pub mod verdict_checkpoints;

use axum::routing::{get, post};
use gx_engine::JournalDeparture;

pub use problem::{ApiError, RollbackFacts};
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

/// 🔴 **R4 / `req/225` H-03** — the clause a `LEDGER_DISAGREES` detail gains when it is the
/// **journal** that moved.
///
/// One spelling in one place, because the condition is answered from three handlers and
/// `AppState::engine_for_write`, and a sentence written four times is a sentence that will say
/// four things. Empty when the journal is intact, so the existing wording of the ordinary case —
/// two files that count differently — is unchanged byte for byte.
///
/// Why a clause rather than a code: `gx_code::LEDGER_DISAGREES` is 44's word for "this project's
/// two files describe different trees" (`req/38` §156 ruling 2(a)) and a rewritten journal is that
/// condition, reached from the other side. Minting a second code is a surface addition and
/// therefore a DR, and `probes/doubt`'s `m6_gx_code` census would be the first to say so. The
/// difference an operator has to act on is *which file to look at*, and that is what this says.
///
/// 🔴 **R32 / `req/392` M-02** — a `bool` until this lane, and one paragraph for all seven
/// conditions it folded.
///
/// `Engine::journal_intact` is a `&&` chain over seven facts (`pipeline.rs`, quoted in the audit's
/// §3-1). The paragraph this returned asserted **one** of them as the cause, in the indicative,
/// and the thirty-first audit drove three of the seven and measured it false on two of the three:
/// a journal whose eight marker bytes had been removed and one carrying `GXJRNL99` were both told
/// *"since DR-43-9 this is the per-record chain refusing to verify"* over files that carry no
/// chain, and both were sent to look for a `<journal>.torn.<n>-<m>` that `gx repair --yes` says in
/// its own remedy it will never write (`4/4` beds, `before=[] after=[]`).
///
/// One arm per [`JournalDeparture`], which is the shape `gx-cli`'s `not_attempted_cause_clause`
/// already uses for `gx_engine::NotAttemptedBecause` and the shape `gx_engine::pipeline::wrap`'s
/// refusals use — the house style for a fold that cannot be avoided. What is **not** done here is
/// the other tempting repair: softening the paragraph into "one of these seven things happened".
/// The engine knows which one; a disjunction would be honest and would still spend the reader's
/// time.
#[must_use]
pub fn journal_note(departure: Option<JournalDeparture>) -> &'static str {
    match departure {
        None => "",
        Some(JournalDeparture::ChainBroken) => JOURNAL_MOVED_NOTE,
        Some(JournalDeparture::PrefixRewritten) => JOURNAL_PREFIX_REWRITTEN_NOTE,
        Some(JournalDeparture::TailRewritten) => JOURNAL_TAIL_REWRITTEN_NOTE,
        Some(JournalDeparture::Shortened) => JOURNAL_SHORTER_NOTE,
        Some(JournalDeparture::Downgraded) => JOURNAL_DOWNGRADED_NOTE,
        Some(JournalDeparture::FromANewerGx) => JOURNAL_FROM_A_NEWER_GX_NOTE,
        Some(JournalDeparture::TornTail) => JOURNAL_TORN_TAIL_NOTE,
    }
}

/// 🔴 **R32 / `req/392` M-02** — the note a face prints, chosen rather than concatenated.
///
/// # What this replaces
///
/// Six sites spelled `format!("{}{}", journal_note(..), rolled_back_note(..))`, and the audit's
/// §3-3 measured what the `{}{}` costs: [`rolled_back_note`] exists **because** its own condition
/// makes every word [`journal_note`] would print false — that is verbatim in its doc, written when
/// `req/229` H-01 was repaired — and the implementation appended one to the other. An operator
/// read *"The journal is the file that moved ... run `gx repair` for the byte the chain stopped
/// verifying at"* immediately followed by *"Note that the journal and the ledger agree with each
/// other here ... Comparing the two files will find nothing"*, in one `detail` string, `4/4` beds.
///
/// # The four arms
///
/// The pair is not exclusive, so the fourth arm is written rather than assumed away. When the
/// journal **has** departed and the project is **also** behind its published head, the departure
/// is what the operator has to act on and the head is a second fact — so the departure's sentence
/// is printed and the roll-back's is reduced to the fact it carries, without the clause that says
/// comparing the two files will find nothing. That clause is true only when the files agree, which
/// is exactly the arm below it.
#[must_use]
pub fn journal_and_head_note(
    departure: Option<JournalDeparture>,
    rolled_back: Option<&str>,
) -> String {
    match (departure, rolled_back) {
        (None, None) => String::new(),
        (Some(departure), None) => journal_note(Some(departure)).to_string(),
        (None, Some(why)) => rolled_back_note(Some(why)),
        (Some(departure), Some(why)) => format!(
            "{} {why}. That is a second difference and not the same one: this project is also behind the signed head it already published (`.gx/checkpoints/head.json`). The sentence before it is about the journal file itself.",
            journal_note(Some(departure))
        ),
    }
}

/// 🔴 **R37 / `req/496` M-04** — the one refusal every ledger read owes a project whose two files
/// describe different trees.
///
/// # What was wrong
///
/// `GET /ledger/checkpoint` has asked `ledger_agrees()` since `req/215` H-01, and its sentence — a
/// long one, built twice in that handler — says why: *"there is no head this server can honestly
/// sign"*. Audit 36 cut a journal's last frame on a committed project, left the ledger whole, and
/// asked all three ledger reads of the same process at the same instant (`req/496` §4-4):
/// `checkpoint` answered `500 LEDGER_DISAGREES`, and `GET /ledger/proof` and
/// `GET /ledger/consistency` answered **byte-for-byte what they answer on a sound project**.
///
/// The gate was on the route that signs, and the two routes that do not sign are precisely the ones
/// a buyer uses to check this deployment **without trusting it**. `ledger_proof`'s own doc-string
/// carries 44 §2.2 with no degradation (`SEM-gx-api-154`): `404` for "an unknown leaf, **or not yet
/// committed**", and the cut row reads back `Committing` through `GET /candidates/{id}` in the same
/// breath.
///
/// # Why the sentence is shared rather than copied a third and fourth time
///
/// It was already written twice inside `ledger_checkpoint` (before and after the lock is taken).
/// Copying it to two more call sites is the shape `req/38` §227 keeps naming — one question asked
/// at four sites, drifting apart at three of them. The bytes are unchanged from what R4/R6/R7 left,
/// so every test that reads the refusal reads the same words it did.
///
/// # What this deliberately does not change
///
/// It is a refusal about the **project**, not about the caller's argument, so each caller places it
/// after the questions that are about the argument. An unknown leaf is a 404 whatever the journal
/// says — the size of the ledger file is a fact the journal's state does not alter — and a repair
/// that swallowed that 404 into this 500 would trade one wrong answer for another (`req/501` §0
/// declares it as a negative control, and `r37_ledger_gate_and_state_shape.rs` measures it on both
/// sides of the cut).
#[must_use]
pub fn ledger_disagrees_refusal<E, C>(engine: &gx_engine::pipeline::Engine<E, C>) -> ApiError
where
    E: gx_engine::EvidenceSource,
    C: gx_engine::Canonicalizer,
{
    // 🔴 **R32 / `req/392` M-02** — chosen, not concatenated.
    let note = journal_and_head_note(
        engine.journal_departure(),
        engine.rolled_back().or_else(|| engine.head_invalid()),
    );
    ApiError::ledger_disagrees(format!(
        "this project's journal witnesses {} commit(s) and its ledger holds {} leaf/leaves, \
         and `ledger_agrees` is false: the two files are describing different trees.{} So \
         there is no head this server can honestly sign (req/215 H-01, req/38 §153 \
         DR-43-6). `gx repair` reports what is wrong and `gx repair --yes` runs 43 §7's recovery under the project lock (DR-43-8); `gx replay <ID>` names the rows that differ",
        engine.sigma().ledger().len(),
        engine.ledger().log().len(),
        note,
    ))
}

/// 🔴 **R6 / DR-43-11** — the clause for a project that is behind its own signed head.
///
/// A third sentence beside [`journal_note`]'s two, and the reason it is separate is the whole of
/// `req/229` H-01: in this condition the journal and the ledger **agree**, so every word
/// [`journal_note`] would print is false. The code is still `LEDGER_DISAGREES` — 44 §2.3's
/// vocabulary does not move and `probes/doubt`'s census would be the first to say so if it did —
/// and the `detail` is where the difference lives, exactly as it has since `req/38` §156 ruling
/// 2(a).
#[must_use]
pub fn rolled_back_note(rolled_back: Option<&str>) -> String {
    match rolled_back {
        None => String::new(),
        Some(why) => format!(
            " {why}. Note that the journal and the ledger agree with each other here: what they \
             disagree with is the signed head this project already published \
             (`.gx/checkpoints/head.json`). Comparing the two files will find nothing, and \
             `gx repair --yes` cannot put back records that are gone."
        ),
    }
}

/// [`journal_note`]'s arm for a **chain break**, named so that [`rolled_back_note`] can sit beside
/// it.
///
/// 🔴 **R32 / `req/392` M-02** — this const kept its name and its two factual claims and lost
/// one. What went, verbatim: *"Look for `<journal>.torn.<n>-<m>` beside it after the next `gx
/// repair --yes`"*. It is false here, and this build says so in its own voice one crate away —
/// `gx repair`'s remedy for the same condition reads *"gx does not repair this and does not cut it
/// — everything after a chain break is a whole record, so truncating would delete what nobody
/// asked to lose, and `--yes` leaves these bytes alone too"* (`crates/gx-cli/src/repair.rs`). The
/// audit drove it on four beds and found `.torn.` files `before=[] after=[]` on all four, with the
/// signing key supplied so that the repair really ran. The clause the CLI already had takes its
/// place, so the two surfaces now say one thing.
///
/// What stayed: *"bytes this process had already read back no longer read the same"* and *"since
/// DR-43-9 this is the per-record chain refusing to verify rather than a count disagreeing"*. Both
/// are true of this arm — the audit's own control bed (one payload byte flipped) measured
/// `journal_chain_break_at: 834` beside them.
const JOURNAL_MOVED_NOTE: &str = " The journal is the file that moved: bytes this process had \
     already read back no longer read the same, so the records behind the frontier above are not \
     the records on the disk (req/225 H-03, and since DR-43-9 this is the per-record chain \
     refusing to verify rather than a count disagreeing — req/227 H-01). gx does not repair this \
     and does not cut it: everything after a chain break is a whole record, so truncating would \
     delete what nobody asked to lose, and `gx repair --yes` leaves these bytes alone too \
     (DR-43-9). Take a copy of the journal before anything else, treat any receipt this server \
     issued since the last healthy start-up as naming a tree nobody can prove, and run `gx repair` \
     for the byte the chain stopped verifying at. Nothing was re-applied to any substrate: 43 §7's \
     recovery refuses a journal in this state (req/227 H-01).";

/// 🔴 **R32 / `req/392` M-02** — the prefix this process consumed no longer produces the head
/// it has been carrying.
///
/// A chain break and this are not the same finding, and the audit's §3-2 table is why they are two
/// consts: a break is a *whole* record the chain reached that is not the record that belongs
/// there, and `gx repair` prints the byte. This one is the identity comparison
/// `EngineJournal::prefix_intact` runs over everything already read — there is no single byte to
/// name, so this sentence does not name one.
const JOURNAL_PREFIX_REWRITTEN_NOTE: &str = " The journal is the file that moved: the bytes this \
     process had already read back no longer produce the chain head it has been carrying, so \
     something behind the frontier above was rewritten after this process read it (req/225 H-03, \
     req/227 H-01). This comparison is over the whole consumed prefix and answers yes or no, so \
     there is no one byte to name here — `gx repair` reports the counts and the framing. gx \
     removes nothing in this state and `gx repair --yes` leaves these bytes alone. Take a copy of \
     the journal before anything else and compare it with a backup, and treat any receipt this \
     server issued since the last healthy start-up as naming a tree nobody can prove. Nothing was \
     re-applied to any substrate: 43 §7's recovery refuses a journal in this state.";

/// 🔴 **R32 / `req/392` M-02** — the last framed record was rewritten, at the same length.
///
/// `req/225` H-03's own shape: the length check cannot see it and this one can.
const JOURNAL_TAIL_REWRITTEN_NOTE: &str = " The journal is the file that moved: its last framed \
     record no longer reads the way it read when this process read it, at the same length \
     (req/225 H-03). A rewritten record is not a torn tail — nothing here was cut short by a \
     crash — so gx removes nothing and `gx repair --yes` leaves these bytes alone. Take a copy of \
     the journal before anything else and compare it with a backup, and treat any receipt this \
     server issued since the last healthy start-up as naming a tree nobody can prove. Nothing was \
     re-applied to any substrate: 43 §7's recovery refuses a journal in this state (req/227 \
     H-01).";

/// 🔴 **R32 / `req/392` M-02** — the file is shorter than what this process read off it.
///
/// `req/222` M-01's condition. Nothing was rewritten: records are simply not there any more, and
/// telling an operator to look for a byte where the chain stopped verifying sends them to a
/// measurement that does not exist on this file.
const JOURNAL_SHORTER_NOTE: &str = " The journal is the file that moved: it is shorter than the \
     bytes this process had already read back from it, so records this process folded are no \
     longer on the disk (req/222 M-01, req/225 H-03). No chain stopped verifying and no tail was \
     torn — what happened is that bytes left the file — and gx cannot put back a record that is \
     gone. Take a copy of the journal and compare it with a backup; `gx repair` prints both \
     lengths. Nothing was re-applied to any substrate: 43 §7's recovery refuses a journal in this \
     state (req/227 H-01).";

/// 🔴 **R32 / `req/392` M-02** — the declaration says chained and the file carries no marker.
///
/// R6's condition (`req/229` H-02), and one of the two the audit measured the old paragraph being
/// false about. 🔴 **`req/392` M-01** — a journal of **zero** bytes arrives here too, since
/// `replay` stopped answering `ChainedV2` about a file with no marker on it.
const JOURNAL_DOWNGRADED_NOTE: &str = " The journal is not the journal this project declares: \
     `.gx/VERSION` says this project's journal is chained and the file on the disk carries no \
     framing marker — a file of zero bytes carries none either (req/229 H-02). **No byte of the \
     journal is claimed to have been rewritten**: what disagrees is the declaration and the \
     marker, so the two things to compare are `.gx/VERSION` and the first eight bytes of the \
     journal. gx cuts nothing and removes nothing in this state. Nothing was re-applied to any \
     substrate: 43 §7's recovery refuses a journal in this state (req/227 H-01).";

/// 🔴 **R32 / `req/392` M-02** — the marker belongs to a build this one has never heard of.
///
/// The other of the two. This build's own source says of this condition, on the line that folds it
/// into `journal_intact`, *"nothing is wrong with the file, and this binary cannot verify it"* —
/// and four lines later the old paragraph told the operator the journal had moved.
const JOURNAL_FROM_A_NEWER_GX_NOTE: &str = " The journal carries a framing marker this build has \
     never heard of, so the records inside it were written by a newer `gx` (req/372 M-02). \
     **Nothing is wrong with the file**: this binary cannot verify it, which is a different fact \
     from damage, and nothing was truncated, quarantined or appended — the bytes are exactly \
     where the newer binary left them. Do not repair it with this build; run the `gx` that wrote \
     it. Nothing was re-applied to any substrate: 43 §7's recovery refuses a journal it cannot \
     read (req/227 H-01).";

/// 🔴 **R32 / `req/392` M-02** — the reader's door met bytes that did not replay.
///
/// The one arm where `<journal>.torn.<n>-<m>` is the truth: DR-43-7's quarantine runs on the
/// writer's door for a torn tail and only when there is no chain break, which is exactly the
/// condition that reaches this value ([`JournalDeparture`] asks about the chain first).
const JOURNAL_TORN_TAIL_NOTE: &str =
    " The journal ends part-way through a record: the bytes after \
     its last whole record did not replay, which is the ordinary shape of a process that died \
     while it was writing (DR-43-7). This door read the file without the project lock, so it \
     removed nothing and those bytes are still on it. Look for `<journal>.torn.<n>-<m>` beside it \
     after the next `gx repair --yes`: the writer's door copies a tail that will not replay to \
     that name and then removes it. Nothing was re-applied to any substrate: 43 §7's recovery \
     refuses a journal in this state (req/227 H-01).";

/// 🔴 **M-14** (`req/182` §1-2, `req/189`): the largest request body this surface reads — 2 MiB.
///
/// This is axum's own default (`DefaultBodyLimit`), which the server inherited silently until
/// v0.4-l: 44 had no sentence about it, the CLI has no limit at all (a `gx submit --intent`
/// file may be any size), and a 3 MiB `POST /candidates` was answered 422 with a message about a
/// "length limit" nobody had declared. Declared here, mounted on the router, cited by 44 §2.2 and
/// answered as `413 PAYLOAD_TOO_LARGE`. The number is unchanged on purpose: choosing a new one is
/// a capacity decision (M6-06's single lock reads the whole body under it) and this window
/// declares the fact rather than moves it.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

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

    /// 🔴 **R3 / `req/222` H-02** — the **commit** receipt held for a transformation, and no other.
    ///
    /// Renamed from `load` and narrowed at the same time, because the width was the bug. `load`
    /// returned "the most specific receipt held", which for `gx serve`'s archive meant
    /// `ReceiptStore::first_available` — commit, then ruling, then verdict. The one caller is
    /// DR-43-1's CAS, and a verdict receipt carries no `postcondition_fingerprint`: so a project
    /// holding a verdict receipt and no commit receipt answered `Unobservable::NoPostcondition`,
    /// the CAS was skipped, and the undo fired over whatever was there. The CLI's own pre-flight
    /// read the commit slot alone, so the two surfaces disagreed about what evidence is
    /// (`req/222` H-02's "the asymmetry of the two faces").
    ///
    /// A caller that wants the disclosure order asks for it by name (`gx receipt show`); a caller
    /// that wants evidence asks for the document that carries it.
    fn load_commit(&self, id: &gx_core::TransformationId) -> Option<gx_witness::Receipt>;

    /// 🔴 **R3 / `req/222` H-01** — whether this deployment keeps receipts at all.
    ///
    /// [`NoArchive`] answers `false` and everything else answers `true`. The distinction is not
    /// decoration: after R3 a missing commit receipt **refuses** the undo, and the two ways to
    /// reach "no receipt" are not the same fact. A deployment that keeps an archive and has no
    /// receipt for a committed row is one somebody removed a file from — since R3 a commit whose
    /// receipt the archive would not take is a failed commit — and refusing there is the whole
    /// repair. A deployment that keeps no archive has made a standing choice, and refusing there is
    /// the same answer for a different reason, which is why the reason is carried
    /// ([`gx_engine::WitnessMissing::NoArchive`]) rather than folded.
    fn keeps_receipts(&self) -> bool {
        true
    }
}

/// An archive that holds nothing, for a deployment that has none.
///
/// Named rather than an `Option<Arc<dyn ReceiptArchive>>`, because a `None` at every call site is a
/// question every call site has to answer and this is the answer once. It is also the honest default
/// for a server whose receipts live only in its own table: `load_commit` says so by answering
/// `None`.
///
/// # 🔴 What R3 changed for a deployment that runs this (`req/38` §160 ruling 2)
///
/// **`POST /transformations/{id}/undo` now refuses on this archive, always.** Before R3 the missing
/// receipt made DR-43-1's CAS `Unobservable` and the undo went ahead unchecked; `req/222` H-01
/// measured that the same road was reachable on a *real* archive by deleting one file, and §160
/// ruled the whole road fail-closed. An embedder that wants undo therefore has to supply an archive
/// — which is the true statement of what the feature costs, and it was always true: an undo whose
/// precondition nobody checked is not the product's claim. `gx serve` supplies one
/// (`.gx/receipts/`, req/56 §2), so the CLI and the server are unaffected.
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

    fn load_commit(&self, _id: &gx_core::TransformationId) -> Option<gx_witness::Receipt> {
        None
    }

    /// 🔴 **R3** — the one implementation that says `false`, and the reason this method exists.
    fn keeps_receipts(&self) -> bool {
        false
    }
}

/// 🔴 **T6 condition ① L2 — the body a restarted process rebuilds a row from** (`req/38` §148
/// ruling 1(iii), designed in `req/190` §4-1 L2, lane R2).
///
/// # What was missing, and why the Σ-shadow could not close it
///
/// R1 gave a restarted server every row the journal witnesses, and gave it **without a body**: 42
/// §3.13 records names and digests rather than bodies (ASM-9), so Σ knows that a transformation is
/// `Committed` and does not know the goal bytes it was planned from. Every write is therefore
/// refused on a row this process did not plan — `req/213` §7(a) measured the shape as a `409`
/// naming the missing body, and `req/216` recorded `undo`-of-`undo` stopping at exit 6 for the same
/// reason. The one thing that turns a name back into a body is the intent, and the intent is
/// exactly what `.gx/drafts/` already holds for the CLI (req/56 §2, **M6-01 adopted (a)**).
///
/// So this is [`ReceiptArchive`] again, one noun along, and deliberately the **same shape**: a
/// trait rather than a directory, because `.gx/drafts/`'s layout is req/56 §2's and is declared
/// once in gx-cli — a second spelling here would be a second list. `gx serve` injects the real
/// store; a fixture injects its own; an embedder that has none says so with [`NoDrafts`] and gets
/// R1's honest refusal back.
///
/// # Why the intent travels as a value and not as bytes
///
/// `gx_core::Intent` has no `Deserialize`, deliberately (see `gx_cli::draft`), so a trait that
/// took JSON would force this crate to invent the wire form of an intent. The five fields 42 §3.3
/// fixes are the archive's business; putting the whole value on both sides of the trait keeps that
/// decision inside the implementation, which is where req/56 §2 already put it.
pub trait DraftArchive: Send + Sync {
    /// File one intent under the id `Engine::submit` minted for it.
    ///
    /// # Errors
    /// Whatever the archive could not do, as a sentence. The caller decides whether it is fatal —
    /// `POST /candidates` does not fail a created candidate because the archive is unwritable, it
    /// answers `201` and the row simply cannot be rebuilt after a restart.
    fn store(&self, id: &gx_core::IntentId, intent: &gx_core::Intent) -> Result<(), String>;

    /// Read one back, if this archive holds it.
    fn load(&self, id: &gx_core::IntentId) -> Option<gx_core::Intent>;
}

/// An archive that holds no drafts, for a deployment that has none.
///
/// [`NoArchive`]'s reason exactly: a `None` at every call site is a question every call site has to
/// answer, and this is the answer once. It is also the honest default for an embedder whose engine
/// is the only writer and never restarts under a live row — `load` says so by answering `None`, and
/// the refusal a caller then reads is R1's, unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDrafts;

impl DraftArchive for NoDrafts {
    fn store(&self, _id: &gx_core::IntentId, _intent: &gx_core::Intent) -> Result<(), String> {
        Ok(())
    }

    fn load(&self, _id: &gx_core::IntentId) -> Option<gx_core::Intent> {
        None
    }
}

/// 🔴 The router: `/v1` + thirteen routes, with `/healthz` outside the Bearer guard.
///
/// 44 §2.5's "every endpoint (except `/healthz`) requires `Authorization: Bearer <token>`" (sem: SEM-gx-api-184) is a statement
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
        // 🔴 **P3 / FR-M04** (`req/119` §4): the verdict checkpoint's HTTP half. Three routes, and
        // the one that writes is a `POST` because ruling ⑥ (sem: SEM-gx-api-185) makes issuing a decision somebody takes
        // rather than a timer this server runs.
        .route(
            "/verdict-checkpoints",
            post(verdict_checkpoints::issue).get(verdict_checkpoints::list),
        )
        .route(
            "/verdict-checkpoints/{window_end}",
            get(verdict_checkpoints::get),
        )
        // 🔴 **`req/824` A4** (R1): the attach-source registry. Registered inside `guarded` so
        // that `tests/auth.rs`'s source-derived walk covers all three for free — the single most
        // valuable property of this router's shape, preserved deliberately (`req/824` §3).
        .route(
            "/attach-sources",
            post(attach_sources::register).get(attach_sources::list),
        )
        .route("/attach-sources/{id}", get(attach_sources::get))
        // 🔴 **`req/824` A5** (R2): the observation ingest road. Inside `guarded` for A4's
        // reason; the returned candidate is an ordinary candidate, so every route above already
        // answers about it (R-1: one road, not a second pipeline).
        .route(
            "/attach-sources/{id}/observations",
            post(observations::ingest),
        )
        // 🔴 **H-11 ①** (`req/189`): a known path asked with a method it has no handler for was
        // axum's bare `405` with an empty body — the fourth response writer outside 44 §2.3.
        // Named here, on the router that owns the routes, and answered in problem+json.
        .method_not_allowed_fallback(handlers::method_not_allowed)
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::guard,
        ));

    axum::Router::new()
        .nest(
            BASE_PATH,
            axum::Router::new()
                .route("/healthz", get(handlers::healthz))
                .merge(guarded)
                // 🔴 Hand 6: graceful shutdown's stages 1 and 2, **outside** the Bearer guard and
                // therefore covering `/healthz` too. A load balancer's health check is exactly the
                // request that must stop succeeding first when a server is going away: answering
                // "ok" (sem: SEM-gx-api-186) while refusing everything else would keep traffic arriving at a socket that
                // refuses it. See `serve::guard` for the three stages.
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    serve::guard,
                ))
                .with_state(state),
        )
        // 🔴 **H-11 ①** (`req/189`): an unrouted path was axum's bare `404` with an empty body.
        // Mounted on the outermost router so that `/nope` and `/v1/nope` answer alike (axum 0.8: a
        // nested router without a fallback of its own inherits the outer one).
        .fallback(handlers::not_found)
        // 🔴 **M-14** (`req/189`): the body limit, declared where a reader looks for the router's
        // shape rather than inherited from axum's default in silence. Same value as before.
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
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
/// A named list rather than a subtraction, so that "twelve are served" and "this is the missing
/// one" (sem: SEM-gx-api-187) are one statement. `/stream` is hand 6's for four rulings' worth of reasons (M6-12's event
/// map, M6-13's resume cursor, M6-06's runtime, and the fact that a JSONL stream over a mutex is a
/// design and not a route).
pub const HAND6_ENDPOINTS: [&str; 1] = ["GET /stream"];

/// How many of [`SPECIFIED_ENDPOINTS`] this crate serves.
///
/// **Fourteen** since hand 6: thirteen behind the Bearer guard and `/healthz` outside it. See the
/// crate header for why hand 5's thirteen was not req/88 §6.2's eleven (M6H5-1), and
/// [`list::EXTENSION_ENDPOINTS`] for the four this crate serves that 44 does not specify.
pub const IMPLEMENTED_ENDPOINTS: usize = 14;
