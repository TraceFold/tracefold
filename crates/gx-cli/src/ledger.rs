// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx log proof` / `gx log consistency` / `gx log checkpoint` — 44 §1.2 and **M6-24 adopted (b)** (sem: SEM-gx-cli-123).
//!
//! # 🔴 The third verb is not in 44, and AC-057 cannot be set up without it
//!
//! req/88 §4 M6-24 is a hand-2 blocker and §47 adopted (b):
//!
//! > **the CLI/API issues one on request** (equivalent to `gx log checkpoint` / `GET
//! > /ledger/checkpoint`'s handler calling `unsigned_checkpoint` → `sign_checkpoint`) -- `Σ` does
//! > not move, and the checkpoint becomes exactly what 42 §3.11 defines: "a signed statement about
//! > the tree at that moment" (sem: SEM-gx-cli-124)
//!
//! The measurement that made it a blocker: `gx_witness::dsse::sign_checkpoint` had **zero callers
//! outside gx-witness**, so no signed checkpoint existed anywhere in the shipping code — and
//! AC-057's Given is a receipt verified "against a known `Checkpoint` given via `--checkpoint`" (sem: SEM-gx-cli-125). No
//! producer, no Given.
//!
//! 44 §1.1's table gives `gx log` two verbs and this adds a third, which is a **surface addition to
//! a specification this lane may not edit**. Raised as **M6H2-7**: 44 §2.6 permits "a new
//! endpoint" (sem: SEM-gx-cli-126) as backward-compatible on the HTTP side and says nothing about the CLI, and the HTTP twin
//! (`GET /ledger/checkpoint`, hand 5) is already in 44 §2.2 — so the CLI is the asymmetric half.
//!
//! # 🔴 Who can produce one
//!
//! §47 M6-24's clause: "the signing key is the ledger signing key, and **in an environment where
//! the CLI holds no key (a third-party verifier) a checkpoint cannot be made** -- that is correct.
//! Only the ledger's owner can make one" (sem: SEM-gx-cli-127). `--key` is therefore
//! required and there is no fallback that invents one. A third party running AC-057 receives a
//! checkpoint; they do not mint one.
//!
//! # Nothing is written unless asked
//!
//! The signed head goes to stdout (44 §1.3's single JSON) and to `--out` if given. `.gx/checkpoints/`
//! gets its producer that way — an operator writes `--out .gx/checkpoints/<n>.json` — rather than by
//! this command deciding to store. Who runs it on a schedule is M5-10's shape of question and this
//! hand does not answer it.

use std::path::{Path, PathBuf};

use gx_core::{Checkpoint, Cid, Timestamp, TransformationId};
use gx_log::proof::{prove_consistency, prove_inclusion, unsigned_checkpoint};
use gx_log::LedgerStore;
use gx_witness::dsse::sign_checkpoint;
use gx_witness::KeyPair;

use crate::exit::{Outcome, VERIFY_FAILED};
use crate::{io, layout::Layout, Error, Result};

/// 42 §3.11's example namespace, and this version's default `origin`.
///
/// "The log's namespace… It is what stops a checkpoint of one log from verifying against another's
/// key" (sem: SEM-gx-cli-128). A default rather than a requirement: `--origin` overrides it, because an operator running
/// two logs needs two namespaces and a constant compiled into the binary would give them one.
pub const DEFAULT_ORIGIN: &str = "glovrex-ledger/v1";

/// Open the ledger a project's `.gx/` holds, **without creating one**.
///
/// `LedgerStore::open` creates the file if it is absent, which is right for the engine and wrong
/// here: every verb in this module reads, and a read that left an empty ledger behind would make
/// "there is no ledger" (sem: SEM-gx-cli-129) unobservable after the first attempt. So absence is checked first and
/// answered as absence.
///
/// # Errors
/// [`Error::NotFound`] if the project has no ledger file yet; [`Error::Log`] if it cannot be opened
/// or replayed.
///
/// 🔴 **R41 / `req/561`** — supplement: "has no ledger file yet" is established by
/// [`crate::layout::presence_of`] answering `Absent`, and by nothing else — the absence-first
/// reasoning above is about the case where the ledger **is** absent, and holds unchanged. A path
/// whose `stat` fails for any other reason, or that holds something other than a regular file,
/// falls through to the read-only open below and wears [`Error::Log`]'s existing words. This door
/// does not pass through `Layout::open`'s R40 gate (it is a direct read door), so the fold R40
/// removed there stood here until R41.
pub fn open(layout: &Layout) -> Result<LedgerStore> {
    let path = layout.ledger_path();
    if crate::layout::presence_of(&path).is_absent() {
        return Err(Error::NotFound {
            what: "ledger",
            id: path.display().to_string(),
        });
    }
    read_only(&path)
}

/// 🔴 **DR-43-7 (`req/38` §153) — a read verb opens the ledger read-only and refuses a torn tail.**
///
/// `LedgerStore::open` is a writer's door: it repairs. `req/215` H-03 walked three read verbs
/// through it and watched a 120-byte ledger become **0 bytes** each time — `gx log proof --leaf 0`,
/// `gx verdict-checkpoint list`, and `gx serve`'s start-up gate on its way to *refusing to start and
/// telling the operator to go and look*. None of the three holds `.gx/LOCK`, so what they were
/// cutting was not necessarily a crash's trace: it could be the record another `gx` was appending.
///
/// So the file is opened read-only and the tail is counted. A non-zero count is a **refusal** and
/// not a shorter answer: a proof or a checkpoint computed over a prefix of a tree is a true
/// statement about the wrong tree, and `req/190` F-5 is the measurement of what saying it anyway
/// costs. The refusal carries the numbers, because it is the diagnosis — and it says that nothing
/// was changed, because after `req/215` that is the fact an operator most needs to hear.
///
/// The repair stays where the lock is: any `gx` write verb, or `gx serve`, opens through the
/// writer's door, quarantines the tail to `<file>.torn.<replayed>-<total>` and truncates.
///
/// # Errors
/// [`Error::Log`] if the file cannot be opened or replayed; [`Error::Malformed`] for a torn tail.
fn read_only(path: &Path) -> Result<LedgerStore> {
    let store = LedgerStore::open_read_only(path)?;
    let torn = store.recovery().torn_tail_bytes;
    if torn > 0 {
        return Err(Error::Malformed {
            what: "ledger",
            path: path.display().to_string(),
            detail: format!(
                "{torn} byte(s) after the last whole record do not replay, so this file holds \
                 {} leaf/leaves and an unreadable tail. This verb reads and does not repair, so \
                 **the file was not changed** (DR-43-7, req/215 H-03). A `gx` write verb or `gx \
                 serve` opens the ledger as a writer: it copies the file to `<ledger>.torn.<replayed>-<total>` \
                 and then removes the tail",
                store.log().len(),
            ),
        });
    }
    Ok(store)
}

/// 🔴 **DR-B / `req/38` §337 (`req/565` §3-2 (4))** — the journal is present, is the regular
/// file `req/56` §2 declares, and this process could not open it.
///
/// The predicate is narrow on purpose: [`refuse_if_the_two_files_disagree`] calls this only after
/// it has already established (a) the journal is not absent (`presence_of(..).is_absent()` was
/// false one line above the call) and (b) the underlying refusal is `gx_engine::Error::Io` with a
/// `kind` other than `NotFound` — i.e. the file exists, and the operating system's own `stat`
/// already told the engine it is a regular file it could not then open. A different `gx-engine`
/// refusal reaching the same call site (a malformed record, for instance) is a different fact —
/// "present and unreadable" is not "present and unparseable" — and stays wherever it already
/// folded, unminted.
pub(crate) fn journal_unreadable(path: &Path, kind: std::io::ErrorKind, detail: &str) -> Error {
    Error::JournalUnreadable {
        path: path.display().to_string(),
        reason: format!("{kind:?}: {detail}"),
        remedy: "the operating system refused to open `.gx/ledger/journal` for a reason other \
                 than \"not there\" (permissions, a filesystem that went away, or similar). Fix \
                 whatever the operating system's own message names — a file mode, an unmounted \
                 volume — and run the verb again. This is not `LAYOUT_BLOCKED`: the path holds \
                 exactly the regular file `req/56` §2 declares, so there is nothing here for `gx \
                 repair` to move aside. What still holds meanwhile: `gx repair` reads the ledger, \
                 the commit receipts and the recorded head out of their own files and reports all \
                 three, and `gx receipt verify --offline` still proves what was committed"
            .to_string(),
    }
}

/// 🔴 **R38 / `req/513` M-01** — the question every other face of this product asks before it
/// answers about this project's tree, on the face that was not asking it.
///
/// # What audit 37 measured
///
/// `req/502` closed `req/496` M-04 by putting `Engine::ledger_agrees` in front of `GET
/// /ledger/proof`, `GET /ledger/consistency` and `GET /ledger/checkpoint`, and called the family
/// "four spellings of one sentence". Audit 37 cut a project's last `Committed` frame and asked the
/// **CLI** the same three questions. `gx log proof` returned bytes identical to the healthy
/// project's; `gx log consistency` returned `{"new_size":1,"old_size":1,"path":[]}` where the HTTP
/// twin returned `500`; `gx log checkpoint` returned a **signed** head. In the same project, at the
/// same instant, `gx repair --json` answered `ledger_agrees_before: false`.
///
/// The family was counted as four HTTP routes. It is eight mouths: those four, these three, and
/// `gx receipt verify`'s default anchor, which is this project's own ledger.
///
/// # Why this is a refusal and not a smaller answer
///
/// The two files describe different trees, so no answer about *which* tree this is can be honest —
/// which is the sentence `handlers.rs` already prints. A proof over a prefix of a tree is a true
/// statement about the wrong tree, and `read_only` above refuses a torn tail for exactly that
/// reason one file down. This is the same refusal about the other file.
///
/// # 🔴 Where it is called from, and why the position is load-bearing
///
/// Below the caller's argument and above the answer. An out-of-range `--leaf` and an id this ledger
/// never held are facts about the **ledger file's own size**, which a journal in any state does not
/// change; they keep their exit 6 on both sides of a cut. `req/501` §0 declared that as a negative
/// control precisely so that this gate cannot be written as "refuse everything and call it safe",
/// and `r38_ledger_face_width` drives it.
///
/// 🔴 **R39 / `req/533` L-03(b) — the but-for clause that control has always had.** Those two exit
/// 6 **while `Layout::open` succeeds**, and not otherwise. Audit 38 made `.gx/VERSION` unreadable
/// and both questions moved from 6 to 1: a project that never opens is never far enough along for
/// the ledger file's size to be a fact anybody consults, so the answer stops being about the
/// argument. The clause is written here, in `req/501` §0, and driven by
/// `r39_the_argument_questions_keep_their_exit_only_while_the_project_opens` — a negative control
/// whose precondition is unstated is a control that can be satisfied by an accident.
///
/// # 🔴 What is **not** covered, said here rather than implied
///
/// A project with **no journal** is answered from the ledger as before. That is not a hole this
/// overlooked: it is the third-party verifier `checkpoint`'s note above describes, who holds a
/// ledger file and no project, and there is no second file making a competing claim. What it does
/// mean is that this gate's reach is exactly "this project has a journal", and a caller who deleted
/// the journal outright gets the old behaviour. `Layout::create` refuses that state for a project
/// that has recorded commits (`req/244` L-04), which is what keeps the gap from being reachable
/// through gx's own verbs.
///
/// 🔴 **R40 / `req/553` M-01 — what this paragraph used to say, and why it is withdrawn rather
/// than edited in silence.** R39 wrote it as:
///
/// ```text
/// A project this binary cannot open an engine over — no journal, or a journal that will not
/// read — is answered from the ledger as before.
/// ```
///
/// The clause "**or a journal that will not read**" is the whole of audit 39 M-01. It made this
/// gate's reach a statement about this build's **capability** ("can I open an engine") when the
/// sentence it was defending is a statement about the project's **contents** ("is there a second
/// file"), and the two come apart the moment anyone makes a present file unopenable. `req/540`
/// R-1b's own note below the escape arm said the arm "rests on being able to tell 'there is no
/// second file' from 'there is a second file this build cannot read'" and booked the distinction as
/// unproven; R40 implements the distinction (`layout::presence_of`) instead of resting on it, so
/// the sentence above is now about existence and is true as written. Withdrawn, not deleted: a
/// later author needs to see that the hole was declared in the doc comment before it was measured.
///
/// # Errors
/// [`Error::Malformed`] with `what: "project"` — the **one** discriminator this crate maps to
/// `LEDGER_DISAGREES` (`crate::Error::refusal`), so the CLI and the HTTP face answer this condition
/// with one word, which is `req/38` §156 ruling 2(a).
pub fn refuse_if_the_two_files_disagree(layout: &Layout) -> Result<()> {
    let engine = match crate::session::open_engine_read_only(
        layout,
        gx_engine::InjectedEvidence::none(),
        gx_core::FailPosture::FailClosed,
    ) {
        Ok(engine) => engine,
        // Not `?`: see the note above. An engine that will not open is the absence of the second
        // file, not a disagreement between two, and answering the caller's question from the ledger
        // is what this module has always done for it.
        //
        // 🔴 **R39 / `req/540` R-1b — this is an asymmetry with the write road and not an
        // inconsistency with it.** Below this line the read road and `Session::settle` ask exactly
        // the same question of exactly the same value. Here they cannot: `settle` has no
        // counterpart to this arm, because a session whose engine will not open never reaches
        // `settle` at all. So there is no second answer for this one to disagree with. What the
        // arm costs is stated rather than implied: it rests on being able to tell "there is no
        // second file" from "there is a second file this build cannot read", and `Layout::open`
        // failing early — on `.gx/VERSION`, say — is outside that distinction. `req/540` §5 leaves
        // that unproven on purpose and `req/533` L-03(a) is where it is booked.
        // 🔴 **R40 / `req/553` M-01, `req/38` §328 ruling 2 ①③ — the escape, narrowed from a
        // capability to an existence.**
        //
        // R39 wrote this arm as "an engine that will not open is the absence of the second file",
        // and wrote the cost of it plainly one paragraph up: *"it rests on being able to tell
        // 'there is no second file' from 'there is a second file this build cannot read'"*. Audit
        // 39 built the second case and measured what the arm does with it — `chmod 0000` the
        // journal, or replace it with a directory of the same name, and `gx log proof`, `gx log
        // consistency` and `gx log checkpoint` all answer **exit 0**, `checkpoint` with a
        // **signature**, on a project the same binary had refused `LEDGER_DISAGREES` one second
        // earlier and refuses again the moment the file is readable. The reason R39 gave for the
        // arm — *"there is no second answer for this one to disagree with"* — is false there: the
        // second file is present, it holds the disagreement, and the product says so out of its
        // own mouth on the write road.
        //
        // So the arm now asks the question it always rested on, and `layout::presence_of` is where
        // that question is spelled. `Absent` is the third-party verifier `checkpoint`'s note
        // describes — a ledger file, no project, no competing claim — and keeps R39's answer
        // unchanged. Everything else fails closed with the engine's own refusal, which carries the
        // path and the operating system's reason (`Permission denied (os error 13)`), and which
        // `Error::refusal` answers with the same word `gx submit` answers on the same project in
        // the same second. **`INTERNAL` is not the right word for it** — the operating system
        // classified this and 44 §2.3 keeps that code for what cannot be classified — but it is a
        // generic rather than a falsehood, where `JOURNAL_ABSENT`'s "is not there" would be a
        // falsehood, and `LAYOUT_BLOCKED`'s "the path is there and is not what the declaration
        // says" would be a falsehood about a regular file that is exactly what was declared. §328
        // ruling 2 ③/④ took that trade deliberately: the signature stops today, and the missing
        // thirteenth word is filed as a DR against spec 44 §2.3 rather than minted here. See
        // `docs/LIMITS.md`, which carries the same sentence for a buyer.
        Err(e) => {
            if crate::layout::presence_of(&layout.journal_path()).is_absent() {
                return Ok(());
            }
            // 🔴 **DR-B / `req/38` §337 (`req/565` §3) — the thirteenth word, landed here.**
            //
            // §328 ruling 2 ③④ left this exact spot on `INTERNAL` and filed the DR the paragraph
            // above names. The journal is not absent (checked above); if the engine's own refusal
            // is an `Io` failure whose `kind` is not `NotFound`, the operating system has already
            // classified "present, right shape, will not open" — which is precisely `journal_
            // unreadable`'s predicate — and the fold stops here, the same way it stopped for
            // `BUSY` and `LEDGER_DISAGREES`. Any other `gx_engine::Error` (a malformed record, an
            // adapter refusal) is a different fact and keeps falling through unminted.
            if let Error::Engine(gx_engine::Error::Io { kind, detail, .. }) = &e {
                if *kind != std::io::ErrorKind::NotFound {
                    return Err(journal_unreadable(&layout.journal_path(), *kind, detail));
                }
            }
            return Err(e);
        }
    };
    if engine.ledger_agrees() {
        return Ok(());
    }
    // 🔴 **R39 / `req/533` M-01 — the second escape, and why it is gone.**
    //
    // R38 put a third clause here and it is withdrawn rather than deleted in silence, because the
    // reasoning that produced it is the thing a later author needs:
    //
    // ```text
    // let witnesses_a_commit = engine.sigma().ledger().len() > 0;
    // let has_published_a_head = layout.head_path().is_file();
    // if !witnesses_a_commit && !has_published_a_head { return Ok(()); }
    // ```
    //
    // Its argument was that `ledger_agrees` is also false for a ledger file standing beside a
    // journal that witnesses nothing, that this shape has two causes — a project whose journal was
    // destroyed, and a ledger nobody committed through — and that **evidence** separates them: a
    // project that has committed has published a head, so `.gx/checkpoints/head.json` is a fact
    // about its past that a truncated journal does not erase.
    //
    // The clause was right about the two causes and wrong about the evidence, because the evidence
    // is a file and a file can be removed. Audit 38 removed it and measured `gx log proof`, `gx log
    // consistency` and `gx log checkpoint` all answering exit 0 on a cut project — `checkpoint` with
    // a **signature** — while `gx submit` on the same project in the same second answered
    // `LEDGER_DISAGREES` and `gx repair --json` answered `ledger_agrees_before: false`. R38's own
    // claim that audit 37's bed was "refused twice over" was also false: `witnesses_a_commit` counts
    // Σ's committed rows and not the journal's records, so cutting the last `Committed` frame takes
    // it to zero and the head was carrying the refusal alone. `r38_ledger_face_width`'s `b1`..`b4`
    // and `c0`..`c3` are that measurement, moved inside the tree.
    //
    // 🔴 What replaces it is not a better test of the same kind. The discriminator is now
    // `ledger_agrees` **alone**, which is the discriminator `Session::settle` has always used
    // (`session.rs`'s `if !engine.ledger_agrees()`), so the read road and the write road answer one
    // state with one word rather than two. The fixture shape the withdrawn clause was protecting —
    // `tests/support::seed_ledger`, a ledger file cast directly for verbs that read a file — is
    // closed on the **fixture** side: it now builds a project with no journal, which is the
    // third-party shape the escape above already covers and the shape this function's own note two
    // paragraphs up describes. A product condition that exists to keep a fixture green is a product
    // condition an attacker can enter.
    //
    // 🔴 The same two clauses `Session::settle` and `handlers::healthz` join, chosen rather than
    // concatenated (`req/392` M-02), so that a reader who has seen one of these refusals recognises
    // this one.
    let note = gx_api::journal_and_head_note(engine.journal_departure(), engine.rolled_back());
    Err(Error::Malformed {
        what: "project",
        path: engine.journal().path().display().to_string(),
        detail: format!(
            "the journal witnesses {} commit(s) and the ledger holds {} leaf/leaves, and \
             `ledger_agrees` is false: the two files are describing different trees.{} A proof, a \
             consistency proof or a checkpoint computed here would be a true statement about the \
             wrong tree, so this verb refuses instead of answering (req/182 M-12/H-01, req/513 \
             M-01). Nothing was changed. `gx repair` reports what is wrong and `gx repair --yes` \
             runs 43 §7's recovery under the project lock (DR-43-8); `gx replay <ID>` names the \
             rows that differ",
            engine.sigma().ledger().len(),
            engine.ledger().log().len(),
            note,
        ),
    })
}

/// What `--leaf` was given: 44 §1.2 accepts both ("`--leaf <INDEX|TRANSFORMATION_ID>`"; sem: SEM-gx-cli-130).
pub enum Leaf {
    /// A leaf index.
    Index(u64),
    /// A transformation, to be resolved to an index.
    Transformation(TransformationId),
}

impl Leaf {
    /// Parse 44's `<INDEX|TRANSFORMATION_ID>`.
    ///
    /// A bare integer is an index and a `gx1:` value is an id. The order matters and is the one 44
    /// §0 implies: base32 never renders a decimal-only string, so nothing is ambiguous.
    ///
    /// # Errors
    /// [`Error::Usage`] if the argument is neither.
    pub fn parse(text: &str) -> Result<Self> {
        if let Ok(index) = text.parse::<u64>() {
            return Ok(Leaf::Index(index));
        }
        // `Cid::from_text` and not a mint: Rule 1 (i) (sem: SEM-gx-cli-131). Parsing a name is not making one, and the
        // parser lives in gx-core precisely so that this line does not have to reach gx-canon.
        Cid::from_text(text)
            .map(|cid| Leaf::Transformation(TransformationId(cid)))
            .map_err(|e| Error::Usage {
                detail: format!("`{text}` is neither a leaf index nor a `gx1:` id: {e}"),
            })
    }
}

/// 🔴 `gx log proof --leaf <INDEX|TID>` (44 §1.2). stdout: an `InclusionProof` (42 §3.11).
///
/// # Errors
/// [`Error::Log`] if the tree refuses the index. A leaf that is not in the log is **not** an error
/// here: it is an answer with an object saying which leaf, for the reason [`crate::receipt::show`]
/// gives.
///
/// 🔴 **E-M6-24** (req/38 §55, M6H8-14 ②): that answer exits **6**, not 1. 44 §1.2's `log` line says
/// "1 = out-of-range/not-found" (sem: SEM-gx-cli-132) and §1.4's common table gives "not-found" the code 6; M6-25 ruled that
/// the common table wins and §1.2's per-command lists are excerpts, and E-M6-13/E-M6-16 applied that
/// to `cancel`, `escalation` and `undo`. This verb was the one place the same principle had not been
/// applied — hand 2 recorded the divergence in `exit::EXIT_DIVERGENCES` and left it standing.
///
/// 🔴 **R38 / `req/513` M-01** — `layout` is the project this ledger belongs to, and `None` is
/// `gx_log::head::compare`'s declared "the caller did not walk that half" rather than a silent
/// pass: a caller reading a bare ledger file has no journal to consult.
pub fn proof(store: &LedgerStore, leaf: &Leaf, layout: Option<&Layout>) -> Result<Outcome> {
    let log = store.log();
    let index = match leaf {
        Leaf::Index(i) => *i,
        Leaf::Transformation(tid) => {
            // The resolution req/88 §2.1 row 10 names: "the `--leaf` TID→index resolution can use
            // `CommittedRow{transformation, ledger_seq}`" (sem: SEM-gx-cli-133). Taken off the ledger's own entries
            // rather than off Σ, because this command has no engine and the ledger is the durable
            // artefact that answers the question directly.
            let Some(entry) = log.entries().iter().find(|e| e.transformation == *tid) else {
                return Ok(Outcome::refused(
                    serde_json::json!({
                        "leaf": tid.0.to_text(),
                        "found": false,
                        "tree_size": log.len(),
                    }),
                    crate::exit::NOT_FOUND,
                ));
            };
            entry.index
        }
    };
    if index >= log.len() {
        return Ok(Outcome::refused(
            serde_json::json!({
                "leaf": index,
                "found": false,
                "tree_size": log.len(),
            }),
            crate::exit::NOT_FOUND,
        ));
    }
    // 🔴 **R38 / `req/513` M-01** — here, and not above the two refusals: the position is the whole
    // of `refuse_if_the_two_files_disagree`'s negative control.
    if let Some(layout) = layout {
        refuse_if_the_two_files_disagree(layout)?;
    }
    let proof = prove_inclusion(log, index)?;
    Ok(Outcome::ok(serde_json::to_value(&proof).map_err(|e| {
        Error::Malformed {
            what: "inclusion proof",
            path: String::new(),
            detail: e.to_string(),
        }
    })?))
}

/// 🔴 `gx log consistency --from <SIZE> --to <SIZE>` (44 §1.2). stdout: a `ConsistencyProof`.
///
/// # Errors
/// [`Error::Log`] if the sizes are not a pair this tree can prove between. `gx_log` refuses
/// `old > new` and sizes beyond the tree, and that refusal is carried rather than re-decided — a
/// second opinion about what a tree contains is the drift E-M2-12 exists to prevent.
///
/// 🔴 **R38 / `req/513` M-01** — `layout` as in [`proof`].
pub fn consistency(
    store: &LedgerStore,
    from: u64,
    to: u64,
    layout: Option<&Layout>,
) -> Result<Outcome> {
    let log = store.log();
    match prove_consistency(log, from, to) {
        Ok(proof) => {
            // 🔴 **R38 / `req/513` M-01** — on the `Ok` arm only, which is this verb's spelling of
            // "below the caller's argument": a pair this tree cannot prove between keeps its exit 6
            // on both sides of a cut, because the tree's size is a fact about the ledger file.
            if let Some(layout) = layout {
                refuse_if_the_two_files_disagree(layout)?;
            }
            Ok(Outcome::ok(serde_json::to_value(&proof).map_err(|e| {
                Error::Malformed {
                    what: "consistency proof",
                    path: String::new(),
                    detail: e.to_string(),
                }
            })?))
        }
        // 44 §1.2 spells the whole of this command's failure as `out-of-range/not-found` (sem: SEM-gx-cli-134), so an out-of-range
        // pair is answered rather than raised: the object says what the tree's size actually is,
        // which is the one fact a caller who guessed wrong needs. The status is §1.4's **6**
        // (**E-M6-24**, req/38 §55) rather than §1.2's 1 — see `proof` above for the reading.
        Err(e) => Ok(Outcome::refused(
            serde_json::json!({
                "from": from,
                "to": to,
                "tree_size": log.len(),
                "refusal": e.to_string(),
            }),
            crate::exit::NOT_FOUND,
        )),
    }
}

/// 🔴 `gx log checkpoint --key <FILE> [--origin <STR>] [--out <PATH>]` — **M6-24 adopted (b)** (sem: SEM-gx-cli-135).
///
/// `unsigned_checkpoint` then `sign_checkpoint`, which is the first call to the latter outside
/// gx-witness in this repository. `at` is an argument and not a clock read: 41 §6 injects time at
/// the engine boundary and [`crate::clock::now`] is the binary's one reader (Rule 2; sem: SEM-gx-cli-136).
///
/// # Errors
/// [`Error::Log`] if the log is empty — 42 §3.11's head is "a signed statement about the tree at
/// that moment" (sem: SEM-gx-cli-137) and a
/// tree of zero entries has no root, so `unsigned_checkpoint` refuses. [`Error::Witness`] if the
/// signing bytes have no canonical form. [`Error::Io`] if `--out` cannot be written.
pub fn checkpoint(
    store: &LedgerStore,
    key: &KeyPair,
    origin: &str,
    at: Timestamp,
    out: Option<&Path>,
    layout: Option<&Layout>,
) -> Result<Outcome> {
    // 🔴 **R7 / `req/232` M-05** — the verb that **mints** a signed head reads the head store that
    // exists to protect one.
    //
    // R6 put DR-43-11 (c)'s "no two signatures over one `tree_size`" inside `Engine::record_head`,
    // which is on the commit road. This verb is not on that road: it opened the ledger, signed
    // whatever tree was in front of it, and never touched `.gx/checkpoints/head.json`. The audit
    // walked straight through — a project cut back to two leaves, a fourth commit taking it to
    // three again, and `gx log checkpoint` signing a **second, different** root for `tree_size: 3`
    // under the same key. Equivocation is the one failure a transparency log exists to make
    // impossible, and gx was able to produce it with a published verb.
    //
    // So the same comparison happens here, and this verb refuses rather than signing. It does not
    // *write* a head — that is the commit road's business, and a mint is not a write to the two
    // append-only files — but it will not put a signature on a tree the project's own record
    // contradicts.
    if let Some(layout) = layout {
        // 🔴 **R38 / `req/513` M-01** — and before that, the question `GET /ledger/checkpoint` has
        // asked since `req/215` H-01. `head_agrees_with` compares this tree with the head this
        // project already published; it is silent about a project that has published none, which is
        // the state audit 37 signed a checkpoint in. A signature outlives the mistake that produced
        // it, so this verb owes both questions and not one.
        refuse_if_the_two_files_disagree(layout)?;
        head_agrees_with(store, layout)?;
    }
    let unsigned = unsigned_checkpoint(store.log(), origin, at)?;
    let signed = sign_checkpoint(&unsigned, key.signing_key(), key.key_id())?;
    let json = serde_json::to_value(&signed).map_err(|e| Error::Malformed {
        what: "checkpoint",
        path: String::new(),
        detail: e.to_string(),
    })?;
    if let Some(path) = out {
        write_checkpoint(path, &json)?;
    }
    Ok(Outcome::ok(json))
}

/// 🔴 **R7 / `req/232` M-05** — refuse to sign a tree the project's recorded head contradicts.
///
/// Three answers and one refusal. A project with **no** recorded head is signed for exactly as it
/// was before this release (there is no statement to contradict). A project whose tree is at or
/// beyond its head, with the head's root still the root at that size, is signed for. A project that
/// is **behind** its head, or that has a different history of the same length, is refused — because
/// a signature minted here is a document that outlives the mistake, and the one thing a
/// transparency log may never do is put two roots at one size under one key.
///
/// # Errors
/// [`Error::Log`] carrying `gx_log`'s own `rolled_back` sentence — the same words `gx repair`,
/// `gx serve` and `/healthz` print, for `req/38` §156 ruling 2(a)'s reason.
fn head_agrees_with(store: &LedgerStore, layout: &Layout) -> Result<()> {
    let head_store = gx_log::HeadStore::at(layout.head_path(), DEFAULT_ORIGIN);
    let Some(head) = head_store.read()? else {
        return Ok(());
    };
    let floor = head.floor()?;
    // The journal is not opened here: this verb has never opened one, and the two numbers that
    // would need it are the journal's. What is compared is the tree, which is the thing being
    // signed. `None` for both journal arguments is `gx_log::head::compare`'s declared "the caller
    // did not walk that half" rather than a silent pass.
    if let Some(why) = gx_log::head::compare(
        &floor,
        store.log().len(),
        store.log().root_at(floor.tree_size),
        floor.journal_len,
        None,
        floor.version_digest.as_deref(),
    ) {
        return Err(Error::Usage {
            detail: format!(
                "{}. `gx log checkpoint` signs a statement about this tree, and a signature \
                 outlives the mistake that produced it: two signed roots for one `tree_size` under \
                 one key is the failure a transparency log exists to make impossible (req/232 \
                 M-05, DR-43-11 (c)). `gx repair` reports what is wrong; \
                 `gx repair --against <FILE>` compares this project with a checkpoint you kept \
                 outside it",
                why.detail()
            ),
        });
    }
    Ok(())
}

/// The bytes `--out` writes, which are the bytes stdout carries.
///
/// One serialisation for both, so that a checkpoint an operator stored and a checkpoint they piped
/// are the same document. Two writers would be two documents that verify differently the day one of
/// them gains a field.
fn write_checkpoint(path: &Path, json: &serde_json::Value) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(io("create", parent))?;
        }
    }
    let body = serde_json::to_vec_pretty(json).map_err(|e| Error::Malformed {
        what: "checkpoint",
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    std::fs::write(path, body).map_err(io("write", path))?;
    Ok(path.to_path_buf())
}

/// 🔴 `gx checkpoint export <FILE>` — **R6 / DR-43-10 minimal form** (`req/38` §167 ruling 1).
///
/// # The one artefact that survives an attacker with write access
///
/// `req/229` §7-4 answered a question `req/38` §166 ruling 2 asked by name — *what should an
/// auditor hold?* — and the answer is that **nothing inside `.gx/` qualifies.** The signed receipts
/// under `.gx/receipts/` cannot be forged (the key is in `~/.gx/keys/`, 0600, outside the project)
/// but they can be **deleted**; `.gx/checkpoints/head.json` is the detector DR-43-11 added and it
/// is inside the same write scope as the files it protects. What survives is a copy that left the
/// machine.
///
/// So this verb makes leaving cheap. It copies the **already-signed** `Checkpoint` out of the
/// project's recorded head, verbatim: no key is needed, nothing is re-signed, and the bytes are the
/// bytes `gx log checkpoint` would have printed — which is what makes `gx receipt verify --offline
/// --checkpoint <FILE>` accept the export with no new code at all. Held beside the commit receipts,
/// it is what turns "this project says it is healthy" into a claim a third party can refute: the
/// audit measured a removed commit's receipt answering `verified` against the exported checkpoint
/// and `refuted` (exit 7) against the project's own ledger.
///
/// A project that has never recorded a head has nothing to export, and that is answered as a
/// not-found rather than by minting one — minting would need the ledger key, and a checkpoint made
/// *now* over a tree that may already have been rolled back is exactly the document that must not
/// exist.
///
/// # `note_out` — the public-log-shaped body (**AC-B5 / `req/682` §2-3**)
///
/// When `note_out` is `Some`, the same already-signed head is *also* written there as a C2SP
/// tlog-checkpoint **body** (`checkpoint_note_body`): the three-line note text a public transparency
/// log ingests. It is generated only — never published; putting it onto an external log is the
/// operator's gated act (`req/682` §4). The JSON at `out` stays the verifiable artefact; the note is
/// the interchange shape, and it carries exactly the three fields the signature covers, no more.
///
/// # Errors
/// [`Error::NotFound`] if the project has recorded no head. [`Error::Io`] if the head cannot be
/// read or the file cannot be written. [`Error::Malformed`] if the head will not parse.
pub fn export(layout: &Layout, out: &Path, note_out: Option<&Path>) -> Result<Outcome> {
    let store = gx_log::HeadStore::at(layout.head_path(), DEFAULT_ORIGIN);
    // 🔴 **R7 / `req/232` M-07** — a head that will not parse is a **classified** state.
    //
    // 44 §2.3's `INTERNAL` is the word for "not classifiable", and R6 answered with it here: a
    // `head.json` holding one byte of rubbish took this verb down with `gx_code: "INTERNAL"`. The
    // same principle `req/227` M-04 and `req/229` M-02 closed for the ledger and the verdict chain
    // applies to the file R6 added, and the sentence it deserves names the file.
    let head = store
        .read()
        .map_err(|e| Error::Malformed {
            what: "recorded head",
            path: store.path().display().to_string(),
            detail: format!(
                "{e} `gx repair` reports the rest of this project's state without this file \
                 (req/232 M-07)"
            ),
        })?
        .ok_or_else(|| Error::NotFound {
            what: "recorded head",
            id: store.path().display().to_string(),
        })?;
    // 🔴 **R7 / `req/232` M-06** — the signature is checked **before** the copy is made.
    //
    // R6 copied whatever was in the file. The audit exported from a project whose head had been
    // rewritten to `tree_size: 0`, got exit 0, and got a document listing `key_id` and
    // `signed_fields` as if it were attested — the one artefact this product tells a buyer to carry
    // out of the box, forged, with gx's own verb printing the reassuring parts. A copy that is not
    // evidence must not leave looking like evidence.
    //
    // The key is not **required**: `gx checkpoint export` needs none by design (the document is
    // already signed, and a third party may hold no key of this project's). What changes is that
    // when a key *is* available and the signature does not check out, this refuses; and when no key
    // is available, the report says `signature_checked: false` rather than staying silent.
    let signature_checked = match crate::keys::KeyStore::user_default()
        .ok()
        .and_then(|store| store.load(&head.checkpoint.signature.keyid).ok())
    {
        None => false,
        Some(pair) => {
            gx_witness::dsse::verify_checkpoint(&head.checkpoint, &pair.verifying()).map_err(
                |e| Error::Malformed {
                    what: "recorded head",
                    path: store.path().display().to_string(),
                    detail: format!(
                        "the signed head in this project does not verify under the key it names \
                         ({}): {e}. Exporting it would put a document that is not evidence \
                         somewhere an auditor keeps evidence, so nothing was written. Take a copy \
                         of `.gx/checkpoints/head.json` and see `gx repair` (req/232 M-06, H-01)",
                        head.checkpoint.signature.keyid
                    ),
                },
            )?;
            true
        }
    };
    let json = serde_json::to_value(&head.checkpoint).map_err(|e| Error::Malformed {
        what: "checkpoint",
        path: store.path().display().to_string(),
        detail: e.to_string(),
    })?;
    write_checkpoint(out, &json)?;
    // 🔴 **AC-B5 / `req/682` §2-3** — the C2SP tlog-checkpoint body, generated beside the JSON when
    // asked. It leaves the machine only where the operator sends it; this verb never publishes.
    let note_exported_to = match note_out {
        None => None,
        Some(path) => {
            let body = checkpoint_note_body(&head.checkpoint);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(io("create", parent))?;
                }
            }
            std::fs::write(path, body.as_bytes()).map_err(io("write", path))?;
            Some(path.display().to_string())
        }
    };
    Ok(Outcome::ok(serde_json::json!({
        "exported_to": out.display().to_string(),
        "note_exported_to": note_exported_to,
        "from": store.path().display().to_string(),
        "origin": head.checkpoint.origin,
        "tree_size": head.checkpoint.tree_size,
        "root_hash": head.checkpoint.root_hash.to_text(),
        "key_id": head.checkpoint.signature.keyid,
        "journal_len": head.journal_len,
        "journal_format": head.journal_format,
        // The two numbers above the signature does **not** cover, said so rather than implied.
        // `gx_log::head`'s module note carries the whole of why.
        "signed_fields": ["origin", "tree_size", "root_hash", "timestamp"],
        // 🔴 **R7 / `req/232` M-06** — whether this run checked the signature it copied.
        //
        // `false` means "this environment holds no key for the id this document names", which is
        // the ordinary case for a third party and is **not** a statement that the document is bad.
        // It is here because the alternative — printing `key_id` and `signed_fields` and nothing
        // about whether either was checked — is what let a forged head leave the machine looking
        // attested.
        "signature_checked": signature_checked,
    })))
}

/// The head of the local ledger, **unsigned**, for `gx receipt verify` without `--offline`.
///
/// 44 §1.2 calls the non-offline path "verifying `inclusion_proof` via a ledger inquiry" (sem: SEM-gx-cli-138). There is no ledger
/// client in v0.1 — `gx-api` serves nothing until hands 5 and 6 — so the enquiry is against the
/// local store, and the caller reports which anchor it used. Unsigned because the anchor's role in
/// `verify_offline` is to supply a root and a size; the signature would be checked by
/// `gx_witness::dsse::verify_checkpoint`, and a local head has nobody to attest it to itself.
///
/// # Errors
/// [`Error::Log`] if the log is empty.
pub fn local_head(store: &LedgerStore, at: Timestamp) -> Result<Checkpoint> {
    Ok(unsigned_checkpoint(store.log(), DEFAULT_ORIGIN, at)?)
}

// ---------------------------------------------------------------------------
// AC-B5 (req/682 §2-3, §2-2) -- the C2SP tlog-checkpoint body, and the offline audit verb
// ---------------------------------------------------------------------------

/// Render a signed [`Checkpoint`]'s core as a C2SP tlog-checkpoint **body** (**AC-B5**).
///
/// The body is the three lines `c2sp.org/tlog-checkpoint` fixes — `origin`, the tree size in ASCII
/// decimal, and the RFC 6962 Merkle root as **standard base64** (`gx_core::b64`, RFC 4648 §4 with
/// padding) — each newline-terminated. These are exactly the three fields the checkpoint's signature
/// covers (`{origin, tree_size, root_hash}`, E-M2-19), and no more: `timestamp` is unsigned advisory
/// (CM-5) and the C2SP body has no place for it, so leaving it out here loses nothing the signature
/// protected.
///
/// # What this is, and what it is not
///
/// It is the shape a public transparency log ingests, so an operator can put the head onto one
/// without gx building a witness network of its own (`req/682` §2-3, §4659 Q3). It is **not** a
/// `c2sp.org/signed-note`: gx's attestation over these fields is the DSSE envelope in the JSON export
/// beside it, not an Ed25519 line over this note text. Producing a signed-note signature would mean
/// re-signing the note text with the ledger key — a new signature, and a publish-time act the
/// operator gates (`req/682` §4). This function only lays out the body; the signature stays where it
/// already is.
#[must_use]
pub fn checkpoint_note_body(checkpoint: &Checkpoint) -> String {
    format!(
        "{}\n{}\n{}\n",
        checkpoint.origin,
        checkpoint.tree_size,
        gx_core::b64::encode(&checkpoint.root_hash.0),
    )
}

/// The three signed-core fields recovered from a [`checkpoint_note_body`].
///
/// Not a whole `Checkpoint`: the note body carries only the signed core, so a round-trip recovers
/// the three fields a verifier checks a signature against and nothing it does not (there is no
/// signature and no timestamp in the body to recover).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointNote {
    /// The log's namespace, line one.
    pub origin: String,
    /// The tree size, line two, ASCII decimal.
    pub tree_size: u64,
    /// The Merkle root, line three, decoded from standard base64.
    pub root_hash: Cid,
}

/// Parse a C2SP tlog-checkpoint body back into its three signed-core fields (**AC-B5 round-trip**).
///
/// Strict for `Cid::from_text`'s reason: a body a verifier will check a signature against must have
/// one spelling, so a size that is not canonical decimal, a root that is not exactly 32 base64 bytes,
/// or fewer than three lines is refused rather than repaired. Extension lines past the third (which
/// `c2sp.org/tlog-checkpoint` permits and calls "not auditable by log monitors") are ignored: they
/// are outside the three fields the signature covers.
///
/// # Errors
/// [`Error::Malformed`] naming which line was missing or ill-formed.
pub fn parse_checkpoint_note(text: &str) -> Result<CheckpointNote> {
    let malformed = |detail: String| Error::Malformed {
        what: "checkpoint note",
        path: String::new(),
        detail,
    };
    let mut lines = text.lines();
    let origin = lines
        .next()
        .ok_or_else(|| malformed("empty: a checkpoint note is at least three lines".to_string()))?;
    if origin.is_empty() {
        return Err(malformed(
            "the origin (line one) is empty; a checkpoint of no log is not one".to_string(),
        ));
    }
    let size_line = lines
        .next()
        .ok_or_else(|| malformed("no tree-size line (line two)".to_string()))?;
    let root_line = lines
        .next()
        .ok_or_else(|| malformed("no root line (line three)".to_string()))?;
    // Canonical decimal only: `str::parse` already refuses a leading `+`, a sign, and whitespace, and
    // rejects a leading zero on anything but "0" is enforced by hand because `parse` accepts "007".
    if size_line != "0" && size_line.starts_with('0') {
        return Err(malformed(format!(
            "the tree size `{size_line}` is not canonical decimal (a leading zero)"
        )));
    }
    let tree_size = size_line.parse::<u64>().map_err(|e| {
        malformed(format!(
            "the tree size `{size_line}` is not a base-ten integer: {e}"
        ))
    })?;
    let bytes = gx_core::b64::decode(root_line)
        .map_err(|e| malformed(format!("the root `{root_line}` is not base64: {e}")))?;
    let raw: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
        malformed(format!(
            "the root decodes to {} bytes, not the 32 a SHA-256 Merkle root has",
            bytes.len()
        ))
    })?;
    Ok(CheckpointNote {
        origin: origin.to_string(),
        tree_size,
        root_hash: Cid(raw),
    })
}

/// 🔴 `gx checkpoint audit <FILE>...` — name any contradiction across collected checkpoints
/// (**AC-B5 / `req/682` §2-2**, offline).
///
/// # What it answers, and what it does not
///
/// The detector `gx_log::detect_equivocation` is the arithmetic no signature can rescue: two signed
/// checkpoints of one `origin` at the same `tree_size` with different roots are two attested
/// histories of one length, and this verb is how an operator or auditor reaches it — the call site
/// that keeps the library function from being unreachable code (`req/682` §6, the doctrine's
/// "a function split off needs a caller an E2E walks through"). It loads each JSON checkpoint (the
/// shape `gx checkpoint export` writes), optionally verifies each signature, runs the set-level scan,
/// and reports every `Equivocation` it names.
///
/// **Signatures.** Like `export`, the key is not required: pass `--key` and every checkpoint's
/// signature is verified first, and one that does not verify stops the audit rather than being
/// folded into the arithmetic (a forged checkpoint could otherwise manufacture a false equivocation);
/// omit it and the scan still runs, with `signatures_verified: false` said out loud rather than
/// implied. An **empty** result is the detector's soundness note — "nothing here contradicts itself",
/// never "this is the true history".
///
/// **Fork.** A **fork** (a non-prefix extension) needs the log's own consistency proof, which a bare
/// pair of collected heads does not carry on its own — pass `--proof` (🔴 **B-audit M-1 / N-47**,
/// `req/682` §2-2's second branch) with the `ConsistencyProof` bridging the two `--files` given, and
/// `gx_log::classify_extension` names it exactly as `phase_b_witness` already measures the library
/// call doing. Omitted, this verb still audits only the same-size arithmetic it always has
/// (`gx_log::detect_equivocation`) — `--proof` is additive, never required.
///
/// Exit is `VERIFY_FAILED` (7) when any contradiction is found — the same "ran and answered no" a
/// refuted receipt returns — and `0` with the soundness note when none is.
///
/// # Errors
/// [`Error::Usage`] if no files are given, the key will not load, or `--proof` is given with a file
/// count other than two (`classify_extension` takes one pair). [`Error::Io`] if a file cannot be
/// read. [`Error::Malformed`] if a file is not a checkpoint, does not verify (with `--key`), is not a
/// consistency proof (with `--proof`), or (with `--proof`) does not bridge the two checkpoints'
/// sizes.
pub fn audit(files: &[PathBuf], key: Option<&Path>, proof: Option<&Path>) -> Result<Outcome> {
    if files.is_empty() {
        return Err(Error::Usage {
            detail: "gx checkpoint audit needs at least one checkpoint file to audit".to_string(),
        });
    }
    if proof.is_some() && files.len() != 2 {
        return Err(Error::Usage {
            detail: format!(
                "--proof names a consistency proof over exactly one pair of checkpoints, but {} \
                 files were given; classify_extension takes one old checkpoint, one new checkpoint, \
                 and the proof between them -- more than a pair leaves ambiguous which two it bridges",
                files.len()
            ),
        });
    }
    let public = match key {
        Some(path) => Some(crate::keys::read_public(path)?),
        None => None,
    };
    let mut checkpoints: Vec<Checkpoint> = Vec::with_capacity(files.len());
    let mut inputs = Vec::with_capacity(files.len());
    for path in files {
        let raw = std::fs::read(path).map_err(io("read", path))?;
        let checkpoint: Checkpoint =
            serde_json::from_slice(&raw).map_err(|e| Error::Malformed {
                what: "checkpoint",
                path: path.display().to_string(),
                detail: format!(
                    "not a signed checkpoint (the shape `gx checkpoint export` writes): {e}"
                ),
            })?;
        if let Some(public) = &public {
            gx_witness::dsse::verify_checkpoint(&checkpoint, &public.verifying()).map_err(|e| {
                Error::Malformed {
                    what: "checkpoint",
                    path: path.display().to_string(),
                    detail: format!(
                        "does not verify under the key given to `--key` ({}): {e}. A checkpoint an \
                         audit cannot verify is not evidence, so the audit stopped rather than \
                         fold it into the arithmetic",
                        checkpoint.signature.keyid
                    ),
                }
            })?;
        }
        inputs.push(serde_json::json!({
            "file": path.display().to_string(),
            "origin": checkpoint.origin,
            "tree_size": checkpoint.tree_size,
            "root_hash": checkpoint.root_hash.to_text(),
            "key_id": checkpoint.signature.keyid,
        }));
        checkpoints.push(checkpoint);
    }
    let mut contradictions = gx_log::detect_equivocation(&checkpoints);
    // 🔴 **B-audit M-1 / N-47** — the detector's second branch: `--proof` names the consistency
    // proof between the two (and, by the `Usage` check above, only the two) checkpoints this call
    // was given, and `classify_extension` is the call site that keeps it reachable outside a test
    // (`req/682` §6's "a function split off needs a caller an E2E walks through").
    let proof_checked = proof.is_some();
    if let Some(proof_path) = proof {
        let (old, new) = match (checkpoints[0].tree_size, checkpoints[1].tree_size) {
            (a, b) if a < b => (&checkpoints[0], &checkpoints[1]),
            (a, b) if b < a => (&checkpoints[1], &checkpoints[0]),
            _ => {
                return Err(Error::Usage {
                    detail: "--proof pairs two checkpoints at different tree_sizes (a fork \
                             question); these two share one tree_size, which is equivocation's \
                             territory and needs no proof -- omit --proof to audit it"
                        .to_string(),
                });
            }
        };
        let consistency_proof = crate::receipt::read_consistency(proof_path)?;
        let fork = gx_log::classify_extension(old, new, &consistency_proof).map_err(|e| {
            Error::Malformed {
                what: "consistency proof",
                path: proof_path.display().to_string(),
                detail: e.to_string(),
            }
        })?;
        contradictions.extend(fork);
    }
    let named: Vec<serde_json::Value> = contradictions.iter().map(contradiction_json).collect();
    let report = serde_json::json!({
        "audited": inputs,
        "count": checkpoints.len(),
        "signatures_verified": public.is_some(),
        "proof_checked": proof_checked,
        "contradictions": named,
        // The soundness note the detector carries, made a field so a reader is not left to infer it.
        "note": "an empty `contradictions` means nothing in this set contradicts itself, never \
                 that this is the true history; a fork across differently-sized checkpoints is only \
                 checked when --proof names the consistency proof between them (see `gx-log` \
                 classify_extension)",
    });
    if contradictions.is_empty() {
        Ok(Outcome::ok(report))
    } else {
        Ok(Outcome::refused(report, VERIFY_FAILED))
    }
}

/// One [`gx_log::CheckpointContradiction`] as the JSON `audit` reports.
///
/// `#[non_exhaustive]` on the enum is why the wildcard arm is here rather than an error: a later
/// ruling may name a third shape, and an auditor should get "a contradiction this build does not
/// name" over a panic.
fn contradiction_json(c: &gx_log::CheckpointContradiction) -> serde_json::Value {
    use gx_log::CheckpointContradiction::{Equivocation, Fork};
    match c {
        Equivocation {
            origin,
            tree_size,
            root_a,
            root_b,
        } => serde_json::json!({
            "kind": "equivocation",
            "origin": origin,
            "tree_size": tree_size,
            "root_a": root_a.to_text(),
            "root_b": root_b.to_text(),
        }),
        Fork {
            origin,
            old_size,
            new_size,
            old_root,
            new_root,
        } => serde_json::json!({
            "kind": "fork",
            "origin": origin,
            "old_size": old_size,
            "new_size": new_size,
            "old_root": old_root.to_text(),
            "new_root": new_root.to_text(),
        }),
        _ => serde_json::json!({ "kind": "unknown", "detail": format!("{c:?}") }),
    }
}
