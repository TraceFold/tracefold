// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`gx repair` — the only door into a project whose two files disagree** (**DR-43-8**,
//! `req/38` §160 ruling 2, `req/222` H-06).
//!
//! # The hole this fills
//!
//! `req/219` gave `LEDGER_DISAGREES` a name and put its gate in front of every writer, and
//! `req/221` put it on `/healthz`. `req/222` H-06 then asked the next question and measured the
//! answer: **once a project is in that state, what does an operator do?**
//!
//! * every CLI write verb goes through `Session::open`, whose gate refuses before the verb runs;
//! * `Engine::recover` — 43 §7's own repair — is wired into `verify`, `commit` and `undo`, all
//!   three of which sit *behind* that gate;
//! * the one road that always runs `recover` is `gx serve`'s start-up sequence, and `gx serve`
//!   **refuses to start** on exactly this condition;
//! * the refusal's own advice, `gx replay <ID>`, is a diagnostic: it names the rows that differ and
//!   changes nothing (DR-43-7 — a read does not repair).
//!
//! So the state was observable from four places and exitable from none. That is not a missing
//! feature, it is a trap: the `LEDGER_DISAGREES` gate exists so that a damaged project is not made
//! worse by more writing, and a gate with no door beside it turns "do not make it worse" into "do
//! nothing, forever".
//!
//! # What this verb is, and what it is careful not to be
//!
//! It is the *same* three steps `gx serve` runs at start-up — open through the writer's door
//! (which quarantines a torn tail before truncating it, DR-43-7), catch up, `recover` — with the
//! refusal at the end replaced by a **report**. It is not a `--force` on the gate: nothing here
//! writes a transition the engine would not have written at start-up, and a project that still
//! disagrees afterwards is reported as still disagreeing rather than declared repaired.
//!
//! Two deliberate refusals of scope, both stated rather than discovered:
//!
//! * **it does not fabricate leaves.** When the journal witnesses commits the ledger has no leaves
//!   for, the missing leaf's `receipt_digest` is not in the journal — 42 §3.13's `Committed` record
//!   carries `{transformation, ledger_seq, at}` and no digest — so there is nothing to rebuild a
//!   leaf *from* without inventing one, and an invented leaf is a signed lie about a Merkle tree.
//!   The commit receipts under `.gx/receipts/` do carry `ReceiptPayload::ledger_digest`, and
//!   rebuilding from them is a real road; it is DR-43-8's second half and it is not walked here,
//!   because it needs a ruling about what happens to the inclusion proofs in every *other* receipt
//!   when the tree they name is rebuilt.
//! * **it does not run without being asked.** `--yes` is required before anything is written. Not
//!   because repair is dangerous — it is the same code `gx serve` runs unasked — but because
//!   `req/222` H-06's operator arrives here holding a damaged project, and the first thing they
//!   need is to be told what is wrong in a form they can copy into a bug report. Without `--yes`
//!   this verb is `gx replay`'s missing half: the diagnosis, from inside the lock.
//!
//! # 🔴 **R4 / `req/225` H-01** — the second bullet was a sentence, not a property
//!
//! The paragraph above is kept because it is what this module claimed, and the claim was false in
//! the only situation this verb is ever used in. `run` opened the engine — through
//! [`crate::session::open_engine`], the **writer's** door — before it looked at `yes`, and the
//! writer's door is DR-43-7's "copy the tail that will not replay, then cut it". So the diagnosis
//! *was* a repair, on the one class of project that reaches this verb. Measured (`req/225` H-01,
//! probe F4a): three commits, a ledger of 522 bytes with one bit flipped at offset 40,
//! `gx repair --json` — and the ledger came out at **0 bytes**, with `journal.ledger.torn.0-522`
//! beside it. `gx repair` cannot rebuild a lost leaf (it says so, two bullets up), so that is not
//! a repair that failed, it is an amputation. Probe F4e ran the same command beside a live
//! `gx serve` and watched `/healthz` go from `200` to `500`.
//!
//! Two things changed, and one sentence was corrected:
//!
//! * without `--yes` the engine is opened through [`crate::session::open_engine_read_only`]
//!   (DR-43-7's reader's door on all three files): nothing is created, nothing is quarantined,
//!   nothing is truncated. The torn tail is **counted** and reported, which is the whole of what a
//!   diagnosis owed;
//! * with `--yes` the writer's door is taken, and it is taken **after** the flag is read, so the
//!   quarantine-and-cut DR-43-7 performs is something the operator asked for;
//! * 44 §1.2 said that a live `gx serve` makes this verb answer `BUSY`, so stop the server first.
//!   That was never true and could not be: DR-43-2 makes `.gx/LOCK` **per-operation** precisely so
//!   that a server and a CLI can share a project, so an idle server holds nothing. `BUSY` is what
//!   this verb answers when another `gx` is in the middle of a write, which is a real and useful
//!   refusal; what makes the report safe beside a running server is that it writes nothing, not
//!   that it excludes anybody. The clause is corrected in 44 §1.2's v0.4-q note rather than
//!   defended here.

use std::path::Path;
use std::sync::Arc;

use gx_engine::store::ProcessLock;
use gx_engine::{InjectedEvidence, RecoveryPath};

use crate::exit::Outcome;
use crate::keys::KeyStore;
use crate::layout::{Layout, MetaRepair};
use crate::session::LOCK_FILE;
use crate::{exit, Error, Result};

/// Diagnose — and, with `yes`, repair — a project whose journal and ledger disagree.
///
/// Answers 44 §1.3's single JSON object on stdout. The two facts a caller branches on are
/// `ledger_agrees_after` (did the project come out of the state) and `repaired` (was anything
/// attempted at all).
///
/// # Errors
/// [`Error::NotFound`] (44 §1.4's 6) for a directory with no `.gx/`, `Error::Engine` carrying
/// `Busy` when another `gx` holds the project's `LOCK`, [`Error::Usage`] when `--yes` was passed
/// and no signing key can be resolved, and whatever the engine refuses the recovery with.
pub fn run(project: &Path, signing_key: Option<&str>, yes: bool) -> Result<Outcome> {
    run_against(project, signing_key, yes, None)
}

/// 🔴 **R7 / `req/38` §171 ruling 2(c)** — the same verb, told to accept a rollback.
///
/// # Why accepting one needs a document from outside the project
///
/// `req/232` M-01 measured `gx repair --yes` on a project whose head had been deleted and whose two
/// files had been cut: the recovery re-applied an old delta (`three` → `two`), and then a **new**
/// head was written over the shortened tree, so the rollback became this project's attested past
/// with nothing anywhere saying it had ever been longer. That is laundering, and it happened
/// silently on the ordinary road.
///
/// The repair is not to forbid it — an operator restoring from a backup genuinely needs to move the
/// floor — but to make it a decision with evidence. `--accept-rollback` requires `--against <FILE>`,
/// the file has to be this project's (`origin` and `key_id` are compared, `req/232` M-04), and the
/// project has to be **at or ahead of** that checkpoint. The acceptance is written into the new
/// head (`accepted_rollback`), so the next reader can see what was given up and when.
///
/// # Errors
/// As [`run_against`], plus [`Error::Usage`] when `--accept-rollback` arrives without `--yes` or
/// without `--against`.
pub fn run_accepting(
    project: &Path,
    signing_key: Option<&str>,
    yes: bool,
    against: Option<&Path>,
    accept_rollback: bool,
    reissue_receipts: bool,
) -> Result<Outcome> {
    if reissue_receipts && !yes {
        return Err(Error::Usage {
            detail: "--reissue-receipts writes to `.gx/receipts/`. Pass --yes as well, or drop the flag to read `receipts_missing` in the diagnosis first (req/234 H-01)"
                .to_string(),
        });
    }
    if accept_rollback && !yes {
        return Err(Error::Usage {
            detail: "--accept-rollback moves this project's attested floor onto a shorter tree, \
                     which is a write. Pass --yes as well, or drop the flag to read the diagnosis \
                     first (req/38 §171 ruling 2(c))"
                .to_string(),
        });
    }
    if accept_rollback && against.is_none() {
        return Err(Error::Usage {
            detail: "--accept-rollback needs --against <FILE>: the only evidence that a shorter \
                     tree is the right one comes from outside the project, because everything \
                     inside it is inside the reach of whatever shortened it (43 §7.9 Model B, \
                     req/232 M-01). Export one with `gx checkpoint export` while a project is \
                     healthy and keep it somewhere the project cannot reach"
                .to_string(),
        });
    }
    run_the_repair(
        project,
        signing_key,
        yes,
        against,
        accept_rollback,
        reissue_receipts,
    )
}

/// 🔴 **R6 / DR-43-10 + `req/229` §7-4** — the same diagnosis, checked against a head the operator
/// kept **outside** this machine.
///
/// # Why the last defence cannot be inside `.gx/`
///
/// DR-43-11's persisted head closes `req/229` H-01 against an attacker who truncates the two
/// append-only files, and it closes nothing against one who deletes the head as well. That is not a
/// hole to be plugged one directory further in: **every** file in the project is inside the write
/// scope of whoever can write to the project. The audit reached the same conclusion from the other
/// side — "the artefact an auditor should hold is not in `.gx/` at all" — and named the two things
/// that are worth holding: a signed `Checkpoint` copied out when it was issued, and the commit
/// receipts themselves.
///
/// So this flag is the reading half of `gx checkpoint export`'s writing half. The external
/// checkpoint's signature is not what is compared — comparing it would need the key, and a third
/// party running this may hold none — the **tree** is: a project whose ledger is shorter than the
/// checkpoint, or whose root at the checkpoint's size is not the checkpoint's root, is behind a
/// statement it signed, and no amount of internal consistency can argue with that.
///
/// # Errors
/// As [`run`], plus [`Error::Io`]/[`Error::Malformed`] if `against` will not read as a checkpoint.
pub fn run_against(
    project: &Path,
    signing_key: Option<&str>,
    yes: bool,
    against: Option<&Path>,
) -> Result<Outcome> {
    run_the_repair(project, signing_key, yes, against, false, false)
}

fn run_the_repair(
    project: &Path,
    signing_key: Option<&str>,
    yes: bool,
    against: Option<&Path>,
    accept_rollback: bool,
    reissue_receipts: bool,
) -> Result<Outcome> {
    // 🔴 **R9 / `req/236` H-04** — the diagnosis opens even when the declaration does not read.
    //
    // `req/227` M-03's rule ("a reader's door must not be narrower than a writer's") and `req/222`
    // H-06's ("a state you can see must have a way out") were both broken by one line: this verb
    // used `Layout::open`, so a `.gx/VERSION` with a byte-order mark on it took `gx repair` down
    // with everything else and the operator's screen said nothing about the ledger, the journal,
    // the receipts or the head. The fault is now a value, reported below with its form and its
    // remedy, and everything else in this report is still measured.
    let (layout, declaration_fault) = Layout::open_reporting(project)?;
    // 🔴 **R10 / `req/238` H-01** — `gx repair --yes` is the road that writes a `Nature::Meta` file
    // back, and it runs **before** the engine opens.
    //
    // Order, not taste: `session::anchor_accepting` stamps the journal's framing into
    // `.gx/VERSION` on the writer's road, and since R10 that function refuses an absent or
    // unreadable declaration instead of writing over it. So a `--yes` that opened the engine first
    // would refuse the one project it exists to repair. The report mode writes nothing at all — the
    // module header's promise, and the reason the two arms are separated here rather than folded.
    //
    // What is written is composed out of the project's own facts (the layout version and the
    // framing sniffed off the journal's first eight bytes), and an unreadable file's bytes are
    // moved to `VERSION.pre-repair.<n>` before anything replaces them.
    let config_absent_before = !layout.config_path().exists();
    // 🔴 **R4 / `req/225` H-01, re-cut by R11 / `req/240` H-02** — a project with no journal is not
    // one to diagnose *through the engine*, and it is certainly not one to create one for.
    //
    // R4's early return was right about the door and wrong about the answer. What it printed was a
    // **constant** — `ledger_agrees_before: true`, `journal_commits: 0`, `ledger_leaves: 0`,
    // `remedy: null`, exit **0** — and `req/240` H-02 measured that constant being printed over a
    // project holding two committed leaves, two commit receipts and a signed head, whose next
    // `gx submit` was refused `LEDGER_DISAGREES`. A diagnosis that answers "nothing is wrong"
    // about a project it is about to call broken is worse than no diagnosis: it is one with gx's
    // authority behind it.
    //
    // R10 built the predicate this branch needed (`Layout::established`) and wired it into two
    // callers; this is the third. The answer is now measured off the files that are still there —
    // see [`report_without_engine`] — and this run still writes nothing at all, including the
    // `Nature::Meta` repair below: what `--yes` would write into `.gx/VERSION` is the framing
    // sniffed off the journal's first eight bytes, and a journal that is not there declares
    // nothing to sniff.
    // 🔴 **R13 / `req/244` H-02** — the early return moved **below** the lock, the key and the
    // `Nature::Meta` repair, and it kept everything it was right about.
    //
    // R4's branch was about the engine, and it is still about the engine: a project with no journal
    // is not one to diagnose *through* one, and it is certainly not one to create one for. What it
    // also did, by standing here, was skip [`repair_meta`] — so a project that had lost
    // `.gx/config.toml` **and** `.gx/ledger/journal` had no exit from gx at all. `req/244` H-02
    // measured both forms, three runs each, no variation: `gx submit` refused `CONFIG_ABSENT`,
    // `gx repair --yes` answered `meta_repaired: []` with `config_absent: true` and a remedy that
    // did not contain the word "config", and the next `gx submit` refused `CONFIG_ABSENT` again.
    // Forever. In the form that has never recorded a commit, both repair runs exited **0** — 44
    // §1.2's number for "this project can be written to".
    //
    // The flag travels; the decision is in [`repair_and_report`], after the lock and the key are in
    // hand, which is the only place `DeclarationWriter::for_repair` can be built. What is repaired
    // there is the **settings only**: what `--yes` would write into `.gx/VERSION` is the framing
    // sniffed off the journal's first eight bytes, and a journal that is not there declares nothing
    // to sniff — R4's sentence, unchanged. `.gx/config.toml`'s bytes are the shipped default and
    // ask the journal nothing, which is why the two files part company here.
    // 🔴 **R40 / `req/38` §328 ruling 2 ①** — the report's own copy of the predicate.
    //
    // This is the field an operator reads to decide whether their history is gone, and
    // `!path.exists()` made it lie in the one situation where the answer matters most: R40 made
    // `.gx/ledger/` unreadable and this line printed `journal_absent: true` beside an
    // `engine_open_failed` of `null`, about a journal holding 1,798 bytes. A diagnosis that folds
    // "I could not look" into "it is not there" sends the operator to restore from a backup over a
    // file that was never lost. Only `NotFound` answers `true` now; anything else leaves this
    // `false` and the reason travels in `engine_open_failed`, which is where a reason belongs.
    let journal_absent = crate::layout::presence_of(&layout.journal_path()).is_absent();
    // The same lock every writer takes, and the refusal it produces is the same one: `BUSY` when
    // another `gx` holds `.gx/LOCK` for an operation. 🔴 **R4** — what it is *not* is a way of
    // excluding a running `gx serve`. DR-43-2 makes the lock per-operation so that a server and a
    // CLI can share a project, so an idle server holds nothing and this take succeeds beside one.
    // The report is safe there because it writes nothing (see the module header); the lock is
    // still taken so that a diagnosis is never read off a file another process is halfway through
    // appending to.
    // 🔴 **R5 / `req/227` M-03** — the lock is taken where it can be taken, and its absence is a
    // sentence rather than an `INTERNAL`.
    //
    // `ProcessLock::open` creates the file if it is not there, so on a read-only filesystem — a
    // snapshot, a backup, an investigator's copy — the *first* thing this verb did was fail with
    // `{"gx_code":"INTERNAL","detail":"cannot open the writer lock … Permission denied"}`, which is
    // 44 §2.3's word for "not classifiable" and this is entirely classifiable. The reason to hold
    // the lock is not exclusion but a consistent read (see the module header); on a filesystem
    // nobody can write to there is no concurrent writer to be inconsistent with, so the report is
    // produced and says it was produced without the lock. `--yes` still refuses: that road appends.
    //
    // `BUSY` is untouched. Another `gx` holding the lock is a real refusal with a real word, and
    // this only widens the door for the case where the lock cannot be **made**.
    // 🔴 **R12 / `req/242` M-03** — a lock that cannot be **made** degrades on the `--yes`
    // road too.
    //
    // R11 widened this door for the report and left `--yes` raising, and `req/242` M-03 measured
    // what the asymmetry costs: `.gx/LOCK` is `Nature::Transient`, `GX_PATHS` says gx does not
    // create it, and it is therefore absent from every backup, every `git archive`, every
    // `rsync --exclude '*LOCK*'` and every project no writer has run in. On a read-only tree
    // without one, `gx repair --yes` answered `INTERNAL` "cannot open the writer lock … Permission
    // denied" with **zero** bytes on stdout, while `gx repair` on the same tree printed 3,825 of
    // report. `--yes` on a filesystem nobody can write to has nothing to do anyway; what it owed
    // was the diagnosis.
    //
    // `BUSY` is untouched and is still a refusal: another `gx` **holding** the lock is a real
    // exclusion with a moment at which it stops being true, which is what `acquire_owned` below
    // raises. R11's self-kill separating "cannot be made" from "is held" is the line this keeps.
    let lock = match ProcessLock::open(layout.join(LOCK_FILE)) {
        Ok(lock) => Some(Arc::new(lock)),
        Err(error) => {
            crate::note!(
                "gx repair: reporting without the project lock ({error}). A report writes no \
                 record either way; what the lock buys is a read that no concurrent writer is in \
                 the middle of. `--yes` writes nothing without it either — `meta_repair_refused` \
                 in the report below says so (req/227 M-03, req/242 M-03)"
            );
            None
        }
    };
    let held = match &lock {
        Some(lock) => Some(lock.acquire_owned("gx repair")?),
        None => None,
    };

    // 🔴 **R11 / `req/240` H-01 + M-05 (audit 10 M-03)** — the key is resolved **before** anything
    // is written, and a key that will not resolve makes this run a report rather than a refusal
    // with an empty stdout.
    //
    // What R10 built and where it stood: `gx repair --yes` is the one road that writes a
    // `Nature::Meta` file back, and R10 put that write at the top of this function — above
    // `ProcessLock::open`, above this key, above the engine. 43 §7.12 (a) 4 and `docs/LIMITS.md`
    // v0.4-w then told a buyer that the road "tells you it did, by name". `req/240` H-01 measured
    // the sentence being false of every early refusal, and false **by default**: a project created
    // by `gx submit` carries the shipping `config.toml`, which holds no `engine_signing_keyid`, so
    // the `gx repair --yes` that `DECLARATION_ABSENT`'s own remedy names always reaches
    // [`signing`], always exits 1 `VALIDATION_ERROR` with an empty stdout — and had already
    // written `.gx/VERSION` and `.gx/config.toml` on the way in (5 arms, byte-identical). An
    // operator reads "refused" and gx has written. Worse where it matters most: if the operator
    // had put a setting line of their own in the declaration, the silent write puts a **different**
    // declaration back, the head's recorded digest never matches again, and the second `--yes`
    // (this time with a key) answers `meta_repaired: []` — the fact that the file was ever gone,
    // and the fact that gx rewrote it, are nowhere.
    //
    // So: lock first (a foreign lock is still `BUSY` and still writes nothing — there is a moment
    // at which it becomes true again, and that is what `BUSY` means), then the key, and only then
    // the repair. A key that cannot be resolved is not raised: it is carried as a value into the
    // report — which is audit 10 M-03's fix in the same line — and this run reads everything a
    // report reads, says what it could not do and why, and exits 1.
    let mut key = None;
    let mut meta_repair_refused: Option<String> = None;
    // 🔴 **R13 / `req/244` M-05** — the exit a named-key fault carries, kept while the report is
    // composed. `None` for a run with no such fault, which is every run that reaches the engine.
    let mut key_fault: Option<u8> = None;
    if yes {
        match signing(&layout, signing_key) {
            Ok(resolved) => key = Some(resolved),
            // 🔴 The degradation is for **"no key was named at all"** and for nothing else.
            //
            // `Error::Usage` here is [`signing`]'s own sentence: neither `--signing-key` nor
            // `.gx/config.toml`'s `engine_signing_keyid` named one — which is the shipping
            // project, and therefore the road `DECLARATION_ABSENT`'s remedy sends every buyer
            // down (`req/240` H-01). Any **other** failure is a fault about a key that *was*
            // named — a file that is not there, a store whose name and contents disagree
            // (`req/227` M-06) — and those keep their exit and their problem object on stderr,
            // because the sentence is about a document the operator handed over and naming it is
            // the whole of the answer. Folding them into the report would move `req/227` M-06's
            // two key ids out of the place its probe reads them from.
            Err(Error::Usage { detail }) => {
                meta_repair_refused = Some(format!(
                    "nothing was written: this run was asked to repair and could not resolve a \
                     signing key ({detail}). Name one for this run with `gx repair --yes \
                     --signing-key <KEY_ID>` (`gx key list` prints the ids this machine holds, `gx \
                     key gen` makes one), or record one in `.gx/config.toml` as \
                     `engine_signing_keyid = \"…\"`. The report below was measured anyway and the \
                     project is exactly as this run found it (req/240 H-01)"
                ));
            }
            // 🔴 **R13 / `req/244` M-05 (audit 12 M-05, audit 10 M-03)** — the *other* key
            // failures keep their exit and their problem object, and stop throwing the report
            // away.
            //
            // R11 degraded one shape — "neither `--signing-key` nor `engine_signing_keyid` named a
            // key" — and kept the rest raising, on the argument that a named key that will not
            // resolve is a fault about a document the operator handed over, and that folding it
            // into the report would move `req/227` M-06's two key ids out of the place its probe
            // reads them from (stderr). The argument is right about **stderr** and was doing double
            // duty: `req/244` M-05 measured the consequence on stdout — `--signing-key
            // does-not-exist` answered rc 6 `NOT_FOUND` with **zero** bytes of report, and a key
            // store whose contents disagree with its name answered rc 1 with zero. Nothing was
            // written on either road (which is why this is M and not H), and nothing was said
            // either, about a project the operator was trying to diagnose.
            //
            // So both halves happen. The problem object goes to stderr here, verbatim — `req/227`
            // M-06's probe reads the same bytes from the same stream — and the run continues into
            // the report with the fault as a value, exiting with the number the refusal carries
            // rather than with the report's own. `gx repair` is the verb whose whole reason is to
            // answer about a project that is in trouble; a key it could not resolve is one more
            // fact about that project and not a reason to say nothing.
            //
            // 🔴 **R14 / `req/246` H-01** — and the road it goes out on is the typed one. This site
            // is not the last thing this run says (the report follows on stdout, and
            // `meta_repair_refused` below carries the same fact), so a stderr that will not take it
            // costs the verbatim object and nothing else — but `eprintln!` would have cost the whole
            // run a panic at exit 101, which is the finding.
            Err(other) => {
                let _stderr_took_it = crate::emit::problem_line(&other.problem());
                key_fault = Some(other.exit_code());
                meta_repair_refused = Some(format!(
                    "nothing was written: this run was asked to repair and the signing key it was \
                     given could not be resolved ({other}). The problem object for it is on \
                     stderr with its own `gx_code` and this run exits with its status; the report \
                     below was measured anyway and the project is exactly as this run found it. \
                     `gx key list` prints the ids this machine holds (req/244 M-05)"
                ));
            }
        }
    }
    Ok(repair_and_report(
        &layout,
        RunAsked {
            yes,
            against,
            accept_rollback,
            reissue_receipts,
        },
        Held {
            lock,
            held,
            key,
            key_fault,
        },
        Found {
            declaration_fault,
            config_absent_before,
            meta_repair_refused,
            journal_absent,
        },
    ))
}

/// What the operator asked this run to do.
struct RunAsked<'a> {
    yes: bool,
    against: Option<&'a Path>,
    accept_rollback: bool,
    reissue_receipts: bool,
}

/// What this run took before it was allowed to write anything.
///
/// 🔴 **R11 / `req/240` H-01, typed in R12** — the lock and the key are the two things
/// `req/240` H-01 measured the write happening in front of. `crate::declaration::DeclarationWriter::for_repair`
/// asks for both **by reference**, so the order is now the signature rather than the order of two
/// statements in one function.
struct Held {
    lock: Option<Arc<ProcessLock>>,
    held: Option<gx_engine::store::OwnedLock>,
    key: Option<gx_witness::KeyPair>,
    /// 🔴 **R13 / `req/244` M-05** — the exit status of a named key that would not resolve, whose
    /// problem object has already gone to stderr. The report is still composed; this is the number
    /// it leaves with.
    key_fault: Option<u8>,
}

/// What was already wrong when this run opened the project.
struct Found {
    declaration_fault: Option<Error>,
    config_absent_before: bool,
    meta_repair_refused: Option<String>,
    /// 🔴 **R13 / `req/244` H-02** — this project has no `.gx/ledger/journal`.
    ///
    /// Measured in `run_the_repair`, before the lock, and acted on here rather than there: the
    /// road that repairs `.gx/config.toml` needs the lock and the key, and R4's early return stood
    /// in front of both. See the note at the measurement.
    journal_absent: bool,
}

/// 🔴 **R13 / `req/244` H-02** — what [`report_without_engine`] measured before it lost the
/// engine.
///
/// A struct rather than six more parameters, for `RunAsked`/`Held`/`Found`'s reason (M5H5-1): a
/// parameter every caller has to read is a parameter every caller can pass in the wrong order. It
/// arrived when `lock_held` became the eighth — the journal-absent road moved behind
/// `ProcessLock::open` so that `--yes` could repair `.gx/config.toml` there, so the key is measured
/// rather than stated.
struct WithoutEngine<'a> {
    /// Whether `--yes` was asked for.
    yes: bool,
    /// The declaration's fault, if the door handed one back as a value.
    declaration_fault: &'a Option<Error>,
    /// Whether `.gx/config.toml` was absent when this run opened the project.
    config_absent_before: bool,
    /// What this run put back before it got here.
    meta_repaired: Vec<serde_json::Value>,
    /// Why it could not put something back, if it could not.
    meta_repair_refused: Option<String>,
    /// Whether this run holds `.gx/LOCK`.
    lock_held: bool,
    /// 🔴 **R14 / `req/246` M-01** — whether this run was allowed to put bytes in the project.
    ///
    /// R13 filed `.gx/repair/last.json` on the road that opened an engine and not on this one, and
    /// wrote down why: "a report this run could not compose in full is not one to hand the next
    /// `gx repair` as *the* record". `req/246` M-01 measured where that reasoning stops — a project
    /// that lost `.gx/config.toml` and `.gx/ledger/journal` together, where `--yes` **wrote the
    /// settings back**, 139 bytes, and filed nothing; the next `gx repair` answered
    /// `previous_repair: null`, and the `OUTPUT_FAILED` object told that same run to go read a file
    /// that was never made. This report is composed in full — forty-nine measured keys and `null`
    /// where the engine would have answered — so it is a record like any other.
    permitted_to_write: bool,
    /// How many bytes the record this run read on its way in was, for the reference filed in its
    /// place (`req/246` M-03).
    previous_bytes: Option<usize>,
    /// 🔴 **R14 / `req/246` M-04** — `.gx/repair` occupied by something that is not a directory.
    repair_dir_blocked: serde_json::Value,
}

/// 🔴 **R12 / `req/242` H-02** — everything after the lock and the key, in a function that
/// **cannot** return an error.
///
/// # Why the return type is `Outcome` and not `Result<Outcome>`
///
/// `req/242` H-02 measured `gx repair --yes` writing `.gx/VERSION` and `.gx/config.toml` and then
/// throwing the report away: four triggers (`chmod 000` on the ledger, `chmod 000` on the journal,
/// a regular file where `journal.blobs/` belongs, a directory where `journal.ledger` belongs),
/// twelve runs, no difference — rc **1** `INTERNAL`, **zero** bytes on stdout, and a twenty-five
/// byte declaration on the disk. R11 had moved the write below the lock and the key and written
/// 43 §7.13 (a) about it: "a write to a `Nature::Meta` file happens only after this run is certain
/// it can produce a report". The certainty was placed at the lock; the report is composed after the
/// engine opens, catches up and recovers, and three `?` sat in between.
///
/// A fourth "move the write" would be the fourth patch to the same shape (`req/38` §181 ruling 3).
/// So the guarantee is in the type: **after the lock and the key are in hand, this function has no
/// way to leave without an `Outcome`.** Every fallible step below is a value — the engine's open,
/// `catch_up`, `recover` and `accept_rollback` — and the compiler refuses a `?` that would skip the
/// print, in this function and in every function that grows here later.
///
/// `probes/doubt/tests/declaration_writer_doubt.rs` asserts the signature, so that a later hand
/// cannot buy convenience back by changing it to `Result`.
///
/// # `presence_of_as_json` (SS552 worst-3, `req/38_ERRATA_2026-08-07.md`)
///
/// [`crate::layout::presence_of`]'s three-way answer, turned into this report's wire shape:
/// `Absent -> false`, `Present(_) -> true` (whatever it points at — `attach.rs::present`'s rule,
/// R43 S-7), `Undetermined(_) -> null` (measured or `null`, never a constant — R11's rule for this
/// whole report, R45 / `req/621` M-1). `ledger_present` carried this exact match twice — once in
/// this function, once in [`report_without_engine`] two thousand lines down — "unified in
/// semantics but left as two typed match blocks" per SS552; `verdict_chain_present` beside each
/// carried the identical three arms over a different path. One function, so a fourth `Presence`
/// arm (if one is ever added) is one edit instead of four.
fn presence_of_as_json(path: &Path) -> serde_json::Value {
    match crate::layout::presence_of(path) {
        crate::layout::Presence::Absent => serde_json::Value::Bool(false),
        crate::layout::Presence::Present(_) => serde_json::Value::Bool(true),
        crate::layout::Presence::Undetermined(_) => serde_json::Value::Null,
    }
}

#[allow(clippy::too_many_lines)]
fn repair_and_report(layout: &Layout, asked: RunAsked<'_>, holding: Held, found: Found) -> Outcome {
    let RunAsked {
        yes,
        against,
        accept_rollback,
        reissue_receipts,
    } = asked;
    let Held {
        lock,
        held,
        key,
        key_fault,
    } = holding;
    let Found {
        declaration_fault,
        config_absent_before,
        mut meta_repair_refused,
        journal_absent,
    } = found;
    // The guard lives exactly as long as this function, which is as long as anything here writes.
    let _held = held;
    // Whether this run may touch the disk at all. `--yes` asked; a resolved key is what makes the
    // asking possible, because everything past this point that writes can end in a signature; and
    // 🔴 **R12 / `req/242` M-03** the lock, because a run that could not take one writes
    // nothing at all.
    let mut writing = yes && key.is_some() && _held.is_some();
    // 🔴 **R13 / `req/244` H-01** — whether this run was ever allowed to put bytes in the project,
    // remembered before `writing` can be turned off below.
    //
    // It is the condition the durable record is filed under. A `--yes` that could not take the lock
    // or resolve a key writes **nothing at all** and says so (`meta_repair_refused`), and filing a
    // record there would be a write on the one road that promises none.
    let permitted_to_write = writing;
    if yes && key.is_some() && _held.is_none() {
        meta_repair_refused = Some(
            "nothing was written: this run was asked to repair and could not take the project's \
             writer lock (`.gx/LOCK`), so it reported instead. That file is `Nature::Transient` — \
             gx does not ship it and a backup, a `git archive` or a read-only snapshot will not \
             have one — and creating it is the first write a repair makes. The report below was \
             measured anyway, and the project is exactly as this run found it (req/242 M-03)"
                .to_string(),
        );
    }
    // 🔴 **R14 / `req/246` M-04** — measured before anything else is read out of `.gx/repair/`, and
    // cleared here if this run may write, because everything below that files a record needs the
    // directory to be a directory.
    let repair_dir_blocked = repair_dir_state(layout, writing);
    // 🔴 **R13 / `req/244` H-01** — read before this run writes, so the key is about the *previous*
    // run and not about this one.
    let previous = previous_repair(layout);
    let previous_bytes = previous.as_ref().map(|p| p.bytes);
    let previous = previous.map(|p| p.report);
    let mut meta_repaired: Vec<serde_json::Value> = Vec::new();
    if writing {
        // 🔴 **R11 / `req/240` M-06 (audit 10 M-02) + M-05 (audit 10 M-03)** — a repair that
        // cannot write is a **fact in the report**, not an `INTERNAL` with an empty stdout.
        //
        // Measured on the R10 binary: `chmod 555 .gx` with the declaration gone answered
        // `{"gx_code":"INTERNAL","detail":"write …/.gx/VERSION: Permission denied (os error 13)"}`
        // and printed no report — 44 §2.3 keeps `INTERNAL` for what cannot be classified, and a
        // directory an operator (or their backup tool) left read-only is entirely classifiable.
        // The whole diagnosis is still available on such a project, which is `req/227` M-03's rule:
        // a reader's door must not be narrower than a writer's, and here the reader's door is the
        // one this verb exists to be.
        // 🔴 **R12 / `req/242` M-01** — count what was written **before** choosing the
        // sentence.
        //
        // `repair_meta` writes the declaration first and the settings second, and it used to
        // return `Result<Vec<_>>`: a failure on the second threw the first away, so
        // `meta_repaired` came out `[]` and the caller printed "nothing was written" over a
        // `.gx/VERSION` that was on the disk. Measured by the audit with `.gx/config.toml` as a
        // dangling symlink — report 3,680 B, `meta_repaired: []`, "nothing was written: …", and
        // twenty-five bytes of declaration beside it. The sentence went on to talk about
        // permissions, which was not the fault either.
        let (repaired, refusal) = repair_meta(
            layout,
            _held.as_ref(),
            key.as_ref(),
            // 🔴 **R13 / `req/244` H-02** — which of the two `Nature::Meta` files this run may put
            // back. With no journal, the declaration's `journal_format` line would be a guess and
            // the settings are the shipped default; see `MetaScope`.
            if journal_absent {
                MetaScope::SettingsOnly
            } else {
                MetaScope::Both
            },
        );
        meta_repaired = repaired;
        if let Some(why) = refusal {
            writing = false;
            let what = if meta_repaired.is_empty() {
                "nothing was written".to_string()
            } else {
                let names: Vec<&str> = meta_repaired
                    .iter()
                    .filter_map(|row| row["file"].as_str())
                    .collect();
                format!(
                    "this run wrote {} and then stopped (`meta_repaired` names what is on the \
                     disk)",
                    names.join(", ")
                )
            };
            meta_repair_refused = Some(format!(
                "{what}: gx could not put a `Nature::Meta` file back ({why}). `.gx/` and the file \
                 itself have to be writable by this user for a repair to write one — a snapshot, \
                 a backup copy or an investigator's tree is normally read-only, and on one of \
                 those this verb reports and does not repair. Other faults reach this line too \
                 (a path that does not resolve, a full disk); the parenthesis above is the \
                 operating system's own sentence and it is the one to read. The report below was \
                 measured anyway (req/240 M-06, req/227 M-03, req/242 M-01)"
            ));
        }
    }

    // 🔴 **R4 / `req/225` H-01, re-cut by R11 / `req/240` H-02, moved here by R13 / `req/244`
    // H-02** — a project with no journal is not one to diagnose *through the engine*, and it is
    // certainly not one to create one for.
    //
    // Everything R4 and R11 wrote about this branch still holds: the answer is measured off the
    // files that are still there (see [`report_without_engine`]) rather than printed as a constant,
    // and no journal is composed from the ledger's leaves. What has changed is only **where** it
    // stands. Above the lock it also skipped [`repair_meta`], which is how `req/244` H-02 found a
    // project with no exit: `.gx/config.toml` and `.gx/ledger/journal` gone together, `gx submit`
    // refusing `CONFIG_ABSENT` forever, and `gx repair --yes` unable to put back the one of the two
    // it has always known how to write. Below the lock, `--yes` repairs the settings first and then
    // this branch answers, with `meta_repaired` naming what is now on the disk.
    if journal_absent {
        return report_without_engine(
            layout,
            WithoutEngine {
                yes,
                declaration_fault: &declaration_fault,
                config_absent_before,
                meta_repaired,
                meta_repair_refused,
                lock_held: _held.is_some(),
                // 🔴 **R14 / `req/246` M-01 + M-04** — the two facts this road did not carry.
                permitted_to_write,
                previous_bytes,
                repair_dir_blocked: repair_dir_blocked.clone(),
            },
            &NoEngine::JournalAbsent,
        );
    }

    // 🔴 **R4 / `req/225` H-01** — the flag chooses the door, and it chooses it **first**.
    //
    // `--yes`: the writer's door, where `LedgerStore::open`/`EngineJournal::open` quarantine a
    // tail that will not replay and then remove it, so the next append lands where the tree
    // actually reached (DR-43-7). That is a repair, it happens before anything else can be
    // measured, and it is reported first and separately from what `recover` did.
    //
    // Without it: the reader's door. A torn tail is counted and left exactly where it lies.
    // 🔴 **R12 / `req/242` H-02** — the open is a **value**.
    //
    // This `?` was the first of the three the audit measured. `chmod 000 .gx/ledger/journal.ledger`
    // on a project holding a signing key answered rc 1 `INTERNAL` with an empty stdout and a
    // written declaration; the operator read "refused" and gx had written. What could not be opened
    // is now a key in the report, beside `meta_repaired`, which names what this run did put back.
    let opened = if writing {
        crate::session::open_engine_wired_accepting(
            layout,
            InjectedEvidence::none(),
            None,
            gx_core::FailPosture::FailClosed,
            None,
            &crate::session::McpWiring::default(),
            accept_rollback,
        )
    } else {
        crate::session::open_engine_read_only(
            layout,
            InjectedEvidence::none(),
            gx_core::FailPosture::FailClosed,
        )
    };
    let mut engine = match opened {
        Ok(engine) => engine,
        Err(why) => {
            return report_without_engine(
                layout,
                WithoutEngine {
                    yes,
                    declaration_fault: &declaration_fault,
                    config_absent_before,
                    meta_repaired,
                    meta_repair_refused,
                    lock_held: _held.is_some(),
                    permitted_to_write,
                    previous_bytes,
                    repair_dir_blocked: repair_dir_blocked.clone(),
                },
                &NoEngine::Refused {
                    stage: "open",
                    detail: why.to_string(),
                    // Neither of these two stages writes to a substrate, so the
                    // measurement is empty rather than absent (`req/476` H-01).
                    applied: Vec::new(),
                    recorded: Vec::new(),
                    finished: 0,
                },
            );
        }
    };
    let quarantine = serde_json::json!({
        "journal": {
            "torn_tail_bytes": engine.journal().recovery().torn_tail_bytes,
            "quarantined_to": engine
                .journal()
                .quarantined()
                .map(|p| p.display().to_string()),
        },
        "ledger": {
            "torn_tail_bytes": engine.ledger().recovery().torn_tail_bytes,
            "quarantined_to": engine
                .ledger()
                .quarantined()
                .map(|p| p.display().to_string()),
        },
    });

    // 🔴 **R6 / `req/229` M-05** — the two facts `repaired` is about, measured rather than assumed.
    let quarantined_any =
        engine.journal().quarantined().is_some() || engine.ledger().quarantined().is_some();
    let mut resumed_rows = 0usize;

    // 🔴 **R12 / `req/242` H-02** — the second and third `?`, as values.
    let caught = match engine.catch_up() {
        Ok(caught) => caught,
        Err(why) => {
            return report_without_engine(
                layout,
                WithoutEngine {
                    yes,
                    declaration_fault: &declaration_fault,
                    config_absent_before,
                    meta_repaired,
                    meta_repair_refused,
                    lock_held: _held.is_some(),
                    permitted_to_write,
                    previous_bytes,
                    repair_dir_blocked: repair_dir_blocked.clone(),
                },
                &NoEngine::Refused {
                    stage: "catch_up",
                    detail: why.to_string(),
                    // Neither of these two stages writes to a substrate, so the
                    // measurement is empty rather than absent (`req/476` H-01).
                    applied: Vec::new(),
                    recorded: Vec::new(),
                    finished: 0,
                },
            );
        }
    };
    let before = engine.ledger_agrees();
    let mut recovered_counts = serde_json::Value::Null;
    if let Some(key) = key.as_ref().filter(|_| writing) {
        let recovered = match engine.recover(crate::clock::now(), key) {
            Ok(recovered) => recovered,
            Err(why) => {
                // 🔴 **R36 / `req/476` H-01** — before the report is composed, on stderr, in this
                // verb's own name: the rows this recovery finished and the rows whose delta it
                // wrote without being able to record them. Audit 35 measured this arm answering
                // `rc 1` with **0 bytes on stderr** over a file it had just overwritten.
                crate::recovery::announce_interrupted_recovery("gx repair", &engine);
                let applied: Vec<String> = engine
                    .applied_unrecorded()
                    .iter()
                    .map(|id| id.0.to_text())
                    .collect();
                // 🔴 **R37 / `req/496` M-01** — the other half of what the recovery got done.
                let recorded: Vec<String> = engine
                    .recorded_without_head()
                    .iter()
                    .map(|id| id.0.to_text())
                    .collect();
                // 🔴 **R37 / `req/496` M-02** — the count of rows **this run** closed. `.len()` of
                // the vector counted the rows that were closed before the process opened the
                // journal.
                let finished = engine.recovery_before_error().closed_by_this_run();
                return report_without_engine(
                    layout,
                    WithoutEngine {
                        yes,
                        declaration_fault: &declaration_fault,
                        config_absent_before,
                        meta_repaired,
                        meta_repair_refused,
                        lock_held: _held.is_some(),
                        permitted_to_write,
                        previous_bytes,
                        repair_dir_blocked: repair_dir_blocked.clone(),
                    },
                    &NoEngine::Refused {
                        stage: "recover",
                        detail: why.to_string(),
                        applied,
                        recorded,
                        finished,
                    },
                );
            }
        };
        // 🔴 **R8 / `req/234` H-01 (b)** — see `Session::recover`.
        crate::session::file_recovered_receipts(layout, &recovered);
        // 🔴 **R35 / `req/470` H-01** — and what the recovery just did, in words, on stderr.
        //
        // This verb is the one `docs/LIMITS.md` v0.5-t named by hand while measuring it printing
        // nothing: "`gx repair --yes --json` answered `rc 0`, `repaired: true`, `recover.resumed:
        // 1`, `refused: 0`, `refusals: []`, **printed nothing on stderr**, and the file was `two`
        // again". Twenty lines later the same page said every row that walks that road prints a
        // sentence. It does now.
        //
        // 🔴 The counter is not the answer, and audit 34 §1-6 item 3 is where that was argued
        // hardest against this lane's own finding: this verb *does* already publish
        // `recover.apply_was_announced`, so it was the one of the four with a number to read. A
        // number says a row walked the road. It does not say **what could not be compared**, and
        // it does not say **that this run may have written over somebody else's bytes and cannot
        // tell you so** — and those two are the whole of what the page claims. stdout stays the
        // single JSON object 44 §1.3 promises; the sentence goes to stderr, where the other four
        // verbs put theirs.
        crate::recovery::announce_recovery("gx repair", &recovered);
        let mut resumed = 0usize;
        let mut terminal = 0usize;
        let mut nothing_applied = 0usize;
        let mut refused = 0usize;
        // 🔴 **R5 / `req/227` H-01** — the reasons, verbatim, so a report carries what a start-up
        // would have printed on stderr.
        let mut refusals: Vec<&'static str> = Vec::new();
        let mut payload_mismatch = 0usize;
        // 🔴 **R13 / `req/244` H-03** — rows closed from a receipt that was already on the disk,
        // counted apart from the ones that were re-applied.
        //
        // Both are 43 §7-3b and both end with a `Committed` record, and an operator has to be able
        // to tell them apart, because exactly one of them asked an adapter to touch their world.
        let mut closed_from_receipt = 0usize;
        // 🔴 **R13 / `req/244` H-03** — and the rows closed from the **leaf**, with no receipt
        // anywhere and no substrate read. The narrower half of §7-3b's window, and the half that
        // used to end in a terminal `Aborted` over a commit that had completed.
        let mut closed_from_leaf = 0usize;
        // 🔴 **`req/329` M-01 (`req/38` §233 ruling 2)** — the two halves of `resumed`, and the
        // causes the recovery itself set. See the arms below for why each is its own number.
        let mut ledger_held_the_commit = 0usize;
        let mut apply_was_announced = 0usize;
        let mut not_attempted_because: Vec<&'static str> = Vec::new();
        for row in &recovered {
            if row.payload_matched == Some(false) {
                payload_mismatch += 1;
            }
            match row.path {
                RecoveryPath::Terminal => terminal += 1,
                RecoveryPath::NothingWasApplied => nothing_applied += 1,
                RecoveryPath::ClosedFromFiledReceipt => {
                    closed_from_receipt += 1;
                    resumed += 1;
                }
                RecoveryPath::ClosedFromLedgerLeaf => {
                    closed_from_leaf += 1;
                    resumed += 1;
                }
                // 🔴 **`req/329` M-01 (`req/38` §233 ruling 2)** — counted apart, for the same
                // reason `closed_from_receipt` and `closed_from_leaf` are.
                //
                // Both are `resumed` and both stay in that total, but they are not the same news.
                // `LedgerHeldTheCommit` means *the commit completed before the crash*;
                // `ApplyWasAnnounced` means *the apply was announced, nothing was rebuilt in this
                // run, and what the server holds is a question this report and `gx log` answer
                // together*. Folded into one number, an operator deciding whether to trust a
                // recovered project cannot tell a finished commit from an announced one — and the
                // engine sets `NotAttemptedBecause::RecoveredWithoutRebuilding` on exactly the
                // second of them.
                RecoveryPath::LedgerHeldTheCommit => {
                    ledger_held_the_commit += 1;
                    resumed += 1;
                }
                RecoveryPath::ApplyWasAnnounced => {
                    apply_was_announced += 1;
                    resumed += 1;
                }
                RecoveryPath::NotResumed => refused += 1,
            }
            if let Some(why) = row.refusal {
                if !refusals.contains(&why) {
                    refusals.push(why);
                }
            }
            // 🔴 **`req/329` M-01** — the cause beside the count.
            //
            // The recovery that just ran is the process that reached `Rollback::NotAttempted`, so
            // it is the one process that can say **why**: the cause is not a component of Σ, and a
            // later reader of the same row gets an honest `null`. Read here, from the engine that
            // performed the recovery, it is a fact the report can carry — and the twenty-sixth
            // audit measured that it did not (`A26_REPAIR_CAUSE_IN_REPORT present=false`), so the
            // one number that says which road left the world where it is was nowhere an operator
            // could read it.
            if let Some(because) = engine.rollback_not_attempted_because(&row.transformation) {
                let kind = because.kind();
                if !not_attempted_because.contains(&kind) {
                    not_attempted_because.push(kind);
                }
            }
        }
        resumed_rows = resumed;
        recovered_counts = serde_json::json!({
            "terminal": terminal,
            "resumed": resumed,
            "nothing_applied": nothing_applied,
            "refused": refused,
            // 🔴 **R9 / `req/236` H-03** — the rows whose rebuilt payload did not reproduce the
            // leaf, counted apart from the ones that were resumed.
            //
            // The audit read `recover: {"terminal":1,"resumed":1,"refused":0}` off a run that had
            // just written `Aborted(InternalError)` over the row it was reporting as resumed. Since
            // R9 that row is `NotResumed` (no terminal record, still recoverable) and lands in
            // `refused`; this key says how many of those refusals were the **key**, which is the one
            // an operator can fix by re-running the same command.
            "payload_mismatch": payload_mismatch,
            // 🔴 **R13 / `req/244` H-03** — how many of `resumed` were closed **without asking an
            // adapter anything**, from the commit receipt the critical section had already filed.
            //
            // The audit's finding is that a `gx wrap` commit killed inside §7-3b's window could not
            // be closed at all: `gx repair` has no MCP server, the re-apply refused, and the old
            // road answered that with a terminal `Aborted`. A row counted here is one that needed
            // no world reading, and a report that folded it into `resumed` would hide the one fact
            // an operator cares about — whether this repair wrote to their substrate.
            "closed_from_receipt": closed_from_receipt,
            // 🔴 **R13 / `req/244` H-03** — how many of `resumed` were closed from the ledger's
            // leaf alone: no receipt was filed for them, no substrate was read, and **none was
            // issued**. `receipts_missing` below counts the same rows from the other side, and
            // `--reissue-receipts` from a process that can reach the substrate is the road to one.
            //
            // Its own key rather than a fold into `closed_from_receipt`, because the two carry
            // different amounts of evidence: one was checked against a signed document that
            // digests to the leaf, the other rests on the leaf and on `ApplyStarted` being in the
            // journal in front of it. An operator deciding whether to trust a recovered project
            // needs to see which.
            "closed_from_leaf": closed_from_leaf,
            // 🔴 **`req/329` M-01 (`req/38` §233 ruling 2)** — the two §7-3b/§7-3c arms that
            // `resumed` sums, told apart, and the causes the recovering process set on the rows it
            // closed without rebuilding anything.
            "ledger_held_the_commit": ledger_held_the_commit,
            "apply_was_announced": apply_was_announced,
            "not_attempted_because": not_attempted_because,
            "refusals": refusals,
        });
    }
    // 🔴 **R8 / `req/234` H-01 (c) + B-5** — the two subtractions gx could already have made.
    //
    // `req/234`'s closing sentence about H-01 is that "`leaves − commit receipts` is the
    // subtraction of two numbers gx already holds, and there is no verb that computes it". This is
    // that verb. The census walks the **ledger**, because the ledger is the list of commits a third
    // party can check, and asks the archive and the blob store about each one:
    //
    // * `receipts_missing` — a leaf whose commit receipt is not on the disk. Since R8 a live
    //   commit cannot produce one (the archive write is inside the section and in front of the
    //   `Committed` record), so a non-zero count means a project written by an older binary, an
    //   archive somebody removed a file from, or a recovery whose re-issue would not file. The
    //   remedy names `--reissue-receipts`.
    // * `escrow_bodies_missing` — an escrow row that names an inverse whose blob is gone
    //   (`.gx/ledger/journal.blobs/`, 43 §7.9 (b)'s new row). Model B: the receipt still proves the
    //   commit and the undo can no longer be run, and the remedy says exactly that rather than
    //   pretending there is one.
    let receipt_store = crate::receipt::ReceiptStore::in_layout(layout);
    let filed = |id: &gx_core::TransformationId| {
        matches!(
            receipt_store.get(id, crate::receipt::StoredKind::Commit),
            Ok(Some(_))
        )
    };
    let mut missing_receipts: Vec<gx_core::TransformationId> = engine
        .ledger()
        .log()
        .entries()
        .iter()
        .map(|entry| entry.transformation)
        .filter(|id| !filed(id))
        .collect();
    let missing_bodies: Vec<gx_core::TransformationId> = engine
        .sigma()
        .escrow()
        .iter()
        .map(|row| row.transformation)
        .filter(|id| {
            matches!(
                engine.inverse_status(id),
                Some(gx_engine::InverseStatus::BodyMissing)
            )
        })
        .collect();
    // 🔴 **R9 / `req/236` H-01 + M-04** — the two things a blob directory can hold that are not
    // bodies: a file that does not read back as its own name, and a staging file a crash left.
    //
    // `escrow_bodies_missing` above is about rows; these are about the **directory**. A pre-R9
    // binary on a full disk left `204,800` bytes of a `400,096`-byte body at its content address
    // and the next commit adopted it, so a project written before this release can still be
    // carrying one — and it is not enough to stop making them, an operator has to be told where
    // they are. The staging files are the residue of the atomic write that replaced that road
    // (`BlobStore::write_atomically`): nothing resolves their names, and they are reported here
    // rather than silently accumulating.
    let damaged_bodies = engine.blobs().unreadable_bodies();
    let staging = {
        let mut names: Vec<String> = engine
            .blobs()
            .staging_residue()
            .into_iter()
            .map(|n| format!(".gx/ledger/journal.blobs/{n}"))
            .collect();
        names.extend(tmp_files_in(&layout.join("receipts"), ".gx/receipts/"));
        names.extend(tmp_files_in(
            &layout.join("checkpoints"),
            ".gx/checkpoints/",
        ));
        names.sort();
        names
    };
    // Swept only by the writer's door, and only files no name resolves to and no record mentions —
    // DR-43-7 (1)'s "no verb removes evidence" is about the quarantined tails, not about these.
    let swept = if writing {
        let mut gone: Vec<String> = engine
            .blobs()
            .sweep_staging()
            .into_iter()
            .map(|n| format!(".gx/ledger/journal.blobs/{n}"))
            .collect();
        gone.extend(sweep_tmp_files(&layout.join("receipts"), ".gx/receipts/"));
        gone.extend(sweep_tmp_files(
            &layout.join("checkpoints"),
            ".gx/checkpoints/",
        ));
        gone.sort();
        gone
    } else {
        Vec::new()
    };
    // 🔴 **R8 / `req/234` H-01 (c)** — `--reissue-receipts`, the remedy the count names.
    //
    // Under `--yes` only, because it writes. `Engine::reissue_receipt` never asks the substrate to
    // change anything: it reads the world, rebuilds the payload, and refuses to sign unless the
    // rebuilt payload digests to exactly what the ledger witnessed at that leaf — so the document
    // it files is the one that was committed or no document at all.
    let mut reissued = serde_json::Value::Null;
    if writing && reissue_receipts && !missing_receipts.is_empty() {
        // 🔴 **R11 / `req/240` H-01** — the key resolved once, at the top, rather than asked for a
        // second time here: one run, one key, and no road on which the second ask can answer
        // differently from the first.
        let key = key
            .as_ref()
            .expect("`writing` is `yes` with a resolved key");
        let at = crate::clock::now();
        let mut wrote = 0usize;
        let mut refused: Vec<serde_json::Value> = Vec::new();
        for id in missing_receipts.clone() {
            // 🔴 **R12 / `req/242` H-02** — a re-issue that refused is a row in `refused`,
            // not an exit out of the report. This is the same rule the four `?` above now follow.
            let outcome = match engine.reissue_receipt(&id, at, key) {
                Ok(outcome) => outcome,
                Err(why) => {
                    refused.push(serde_json::json!({
                        "transformation": id.0.to_text(),
                        "why": why.to_string(),
                    }));
                    continue;
                }
            };
            match outcome {
                gx_engine::pipeline::Reissued::Filed(_) => wrote += 1,
                other => refused.push(serde_json::json!({
                    "transformation": id.0.to_text(),
                    "why": other.kind(),
                })),
            }
        }
        let store = crate::receipt::ReceiptStore::in_layout(layout);
        missing_receipts.retain(|id| {
            !matches!(
                store.get(id, crate::receipt::StoredKind::Commit),
                Ok(Some(_))
            )
        });
        reissued = serde_json::json!({ "filed": wrote, "refused": refused });
    }

    let mut after = engine.ledger_agrees();
    let mut frontier = engine.sigma().ledger().len();
    let mut leaves = engine.ledger().log().len();

    // 🔴 **R6 / DR-43-10** — the external head, compared with the tree in front of us.
    //
    // 🔴 **R7 / `req/232` M-04 + L-01** — and compared with the **project's own identity** first,
    // and read in a way that cannot take the diagnosis down with it.
    //
    // M-04: R6 read `origin` and `key_id` off the document, printed them, and compared neither. So
    // a healthy project handed *another* healthy project's export answered `rolled_back: true` and
    // the remedy said "it was signed by this project's own key" — a sentence that was false, with
    // gx's authority behind it. 42 §3.11 makes `origin` the field "that stops a checkpoint of one
    // log from verifying against another's key"; this is the caller that was ignoring it.
    //
    // L-01: an unreadable `--against` file used to raise, and a raise here loses the *project's*
    // diagnosis — which is `req/227` M-04's principle (a reader's door must not be narrower than a
    // writer's) applied to the auditor's own artefact. It is reported as a fact about the file.
    let against_report = match against {
        None => serde_json::Value::Null,
        Some(path) => match crate::receipt::read_checkpoint(path) {
            Err(e) => serde_json::json!({
                "file": path.display().to_string(),
                "readable": false,
                "detail": e.to_string(),
                "rolled_back": serde_json::Value::Null,
            }),
            Ok(checkpoint) => {
                let root_at = engine.ledger().log().root_at(checkpoint.tree_size);
                let behind = leaves < checkpoint.tree_size;
                let diverged = !behind && root_at != Some(checkpoint.root_hash);
                let our_origin = crate::ledger::DEFAULT_ORIGIN;
                let our_key = recorded_head_key(layout)
                    .or_else(|| crate::serve::recorded_signing_keyid(layout).ok().flatten());
                let foreign_origin = checkpoint.origin != our_origin;
                let foreign_key = our_key
                    .as_ref()
                    .is_some_and(|ours| &checkpoint.signature.keyid != ours);
                let foreign = foreign_origin || foreign_key;
                serde_json::json!({
                    "file": path.display().to_string(),
                    "readable": true,
                    "origin": checkpoint.origin,
                    "tree_size": checkpoint.tree_size,
                    "root_hash": checkpoint.root_hash.to_text(),
                    "key_id": checkpoint.signature.keyid,
                    "project_origin": our_origin,
                    "project_key_id": our_key,
                    "project_tree_size": leaves,
                    // 🔴 A checkpoint that is not this project's says nothing about this project,
                    // so it does not get to say `rolled_back` either — in **either** direction.
                    "foreign": foreign,
                    "rolled_back": if foreign {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::Bool(behind || diverged)
                    },
                })
            }
        },
    };
    let against_unusable = against.is_some()
        && against_report
            .get("rolled_back")
            .is_some_and(serde_json::Value::is_null);
    let against_refused = against_report
        .get("rolled_back")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    // 🔴 **R7 / `req/38` §171 ruling 2(c)** — the acceptance, and everything it is conditional on.
    //
    // Three conditions, and each of them is one of the audit's findings: the operator asked
    // (`--accept-rollback`, M-01's silent re-basing), the evidence is readable and **this
    // project's** (M-04), and the evidence does not itself say the project is behind. Only then is
    // a head written over the shorter tree, and it carries what it replaced.
    let mut accept_refused: Option<String> = None;
    if accept_rollback {
        if against_unusable || against_refused {
            accept_refused = Some(
                "--accept-rollback was not honoured: the checkpoint passed to --against is not \
                 evidence that this shorter tree is the right one. A document that is unreadable, \
                 that belongs to another project, or that attests a tree **longer** than this one \
                 says the opposite. Nothing was written (req/38 §171 ruling 2(c))"
                    .to_string(),
            );
        } else if engine.rolled_back().is_none() {
            accept_refused = Some(
                "--accept-rollback was not honoured: this project is not behind the head it \
                 published, so there is no rollback to accept and nothing was written"
                    .to_string(),
            );
        } else if let Some(key) = key.as_ref().filter(|_| writing) {
            let file = against.map_or_else(String::new, |p| p.display().to_string());
            // 🔴 **R12 / `req/242` H-02** — the fourth `?`, as a value. A head that could not
            // be written is `accept_refused` with the reason in it, which is the key an operator
            // already reads for the other three ways this can fail to happen.
            match engine.accept_rollback(&file, crate::clock::now(), key) {
                Ok(()) => {
                    after = engine.ledger_agrees();
                    frontier = engine.sigma().ledger().len();
                    leaves = engine.ledger().log().len();
                }
                Err(why) => {
                    accept_refused = Some(format!(
                        "--accept-rollback was not honoured: accepting a rollback writes a signed \
                         head and this run could not write one ({why}). Nothing was written by \
                         this step; what the rest of this report says was measured anyway \
                         (req/242 H-02)"
                    ));
                }
            }
        } else {
            // 🔴 **R11 / `req/240` H-01** — the run had no key, so it wrote nothing here either,
            // and says so rather than leaving `--accept-rollback` looking honoured.
            accept_refused = Some(
                "--accept-rollback was not honoured: accepting a rollback writes a head and a \
                 head is signed, and this run has no key (see `meta_repair_refused`). Nothing \
                 was written"
                    .to_string(),
            );
        }
    }

    // 🔴 The sentence an operator reads when the repair did not finish the job, and the one place
    // in this workspace that says what gx **cannot** put back. Silence here would be the same
    // failure `req/222` H-06 named one layer up: a state you can see and cannot leave.
    // 🔴 **R6** — the conditions R6 adds are **prepended**, not substituted.
    //
    // A rolled-back project's two files also count differently, a downgraded journal is also
    // shorter than its recorded head, and each of those facts has its own remedy. The first version
    // of this lane made the new sentence replace the old one, and `serve_runtime_r3`'s
    // `a_project_whose_two_files_disagree_has_a_door` went red — correctly: the sentence it was
    // asserting (which bytes to look for, and that gx cannot rebuild a leaf) is the **actionable**
    // half, and the new sentence is the diagnosis. An operator needs both, in that order.
    let declaration_fault_error = declaration_fault.unwrap_or(Error::Usage {
        detail: String::new(),
    });
    let mut remedy_parts: Vec<String> = Vec::new();
    // 🔴 **R11 / `req/240` L-08** — which of the two `Nature::Meta` files this run put back.
    //
    // The audit read `meta_repaired: [{"file":".gx/VERSION","what":"created"}]` on the same object
    // as `declaration_absent: true` and a remedy naming `gx repair --yes` — the verb that had just
    // run. `declaration_absent` stays as it is (it is the fact this run **opened** on, and a
    // monitor branching on it is asking what was wrong, which `repair.rs`'s own note fixed
    // deliberately); the remedy is the sentence an operator acts on, and there is nothing left to
    // act on once the file is back.
    let named = |file: &str| {
        meta_repaired
            .iter()
            .any(|entry| entry["file"].as_str() == Some(file))
    };
    let declaration_written = named(".gx/VERSION");
    let config_written = named(".gx/config.toml");
    // 🔴 **R11 / `req/240` H-01 + M-05** — a `--yes` that could not resolve a key says so first.
    if let Some(why) = meta_repair_refused.clone() {
        remedy_parts.push(why);
    }
    // 🔴 **R9 / `req/236` H-04** — the declaration comes first, because nothing else in this report
    // is a statement about the file that is actually broken.
    // 🔴 **R10 / `req/238` H-01** — the declaration that is **not there**, said first for the
    // reason the unreadable one is: nothing else in this report is a statement about the file that
    // is actually gone.
    if declaration_written {
        remedy_parts.push(format!(
            "`{}` was not there when this run opened the project, and this run wrote it back (`meta_repaired` names it, and `declaration_absent` is what was found rather than what was left). What it holds is this project's own facts — the layout version and the framing sniffed off the journal — so if you had settings of your own on that file they are not in the copy gx wrote; the bytes that were there, if any could be read, are beside it as `VERSION.pre-repair.<n>` (req/240 L-08)",
            layout.join("VERSION").display()
        ));
    }
    if let Error::DeclarationAbsent { path, remedy } = &declaration_fault_error {
        if !declaration_written {
            remedy_parts.push(format!(
            "`{path}` is not there. {remedy}. Everything else in this report was measured anyway \
             — the ledger, the journal, the receipts and the head are read out of their own files \
             and do not depend on this one — and no writer verb will run until the declaration is \
             back, because a writer that wrote one silently is what `req/238` H-01 measured taking \
             R7's declaration digest off at rc 0"
        ));
        }
    }
    if config_written {
        remedy_parts.push(format!(
            "`{}` was not there when this run opened the project, and this run wrote the shipping default back (`meta_repaired` names it). What the shipping default does **not** carry is `engine_signing_keyid`: if this project recorded one, that line is yours to put back, and until it is there every verb that signs needs `--signing-key` (req/240 L-08)",
            layout.config_path().display()
        ));
    }
    if config_absent_before && !config_written {
        remedy_parts.push(format!(
            "`{}` is not there. 43 §7.9 (b) calls it the file that decides which key a recovery \
             signs with (`engine_signing_keyid`), so gx will not write the shipping default back \
             on its own — that would put this project on a key it did not choose, silently, which \
             is what `req/238` H-01 measured `gx submit` doing at rc 0. `gx repair --yes` writes \
             the default file and says so; the `engine_signing_keyid` line is yours to put back, \
             or pass `--signing-key` for one run",
            layout.config_path().display()
        ));
    }
    if let Error::Declaration { path, form, remedy } = &declaration_fault_error {
        remedy_parts.push(format!(
            "`{path}` is there and does not read as a declaration: {form}. {remedy}. Everything \
             else in this report was measured anyway — the ledger, the journal, the receipts and \
             the head are read out of their own files and do not depend on this one — but the \
             **writer's** door stays shut until it parses, so no verb that writes will run \
             (req/236 H-04, req/227 M-03)"
        ));
    }
    if let Some(why) = against_remedy(against, against_refused) {
        remedy_parts.push(why);
    }
    // 🔴 **R7 / `req/232` M-04 + L-01** — a `--against` file that is not this project's, or is not
    // readable, gets its own sentence. It comes first because it is a fact about the **question**,
    // and answering a question nobody asked is worse than saying the question was wrong.
    if against_unusable {
        if against_report["readable"] == serde_json::Value::Bool(false) {
            remedy_parts.push(format!(
                "the checkpoint in {} could not be read ({}), so this run compared nothing against \
                 it. The project's own diagnosis is above and is unaffected. A `gx checkpoint \
                 export` file is a single JSON object; check the copy rather than the project \
                 (req/232 L-01)",
                against_report["file"].as_str().unwrap_or_default(),
                against_report["detail"].as_str().unwrap_or_default()
            ));
        } else {
            remedy_parts.push(format!(
                "the checkpoint in {} is not this project's: it names origin {:?} under key {:?}, \
                 and this project is {:?} under {:?}. A checkpoint of one log says nothing about \
                 another (42 §3.11's `origin` is what stops one verifying against the other's key), \
                 so this run refuses to call the project rolled back **or** healthy on its \
                 evidence. Point --against at an export taken from *this* project (req/232 M-04)",
                against_report["file"].as_str().unwrap_or_default(),
                against_report["origin"].as_str().unwrap_or_default(),
                against_report["key_id"].as_str().unwrap_or_default(),
                against_report["project_origin"].as_str().unwrap_or_default(),
                against_report["project_key_id"].as_str().unwrap_or("none recorded"),
            ));
        }
    }
    if let Some(why) = accept_refused.clone() {
        remedy_parts.push(why);
    }
    // 🔴 **R7 / `req/232` H-01** — the head document itself, when it is not one to read from.
    if let Some(why) = engine.head_invalid() {
        remedy_parts.push(format!(
            "{why}. What to fix: take a copy of `.gx/checkpoints/head.json` before anything else — \
             it is the evidence of what was done to this project — and then move it aside so that \
             this project reports honestly that it records no head (`head_recorded: false`), or \
             restore the file from a backup. gx does not overwrite a head it refused: replacing it \
             here would destroy the evidence and would turn `somebody replaced the detector` into \
             `the detector is fine now` (req/232 H-01, 43 §7.9)"
        ));
    }
    if engine.journal().downgraded() {
        // 🔴 **R6 / `req/229` H-02** — named before the chain-break sentence, because a downgraded
        // journal has **no** chain to break and the two would otherwise be told apart only by the
        // reader noticing that `journal_chain_break_at` is `null`.
        remedy_parts.push(format!(
            "the journal has no format marker and this project declares a chained one \
             (`.gx/VERSION`): the chain was taken off after this project was written. gx does not \
             cut it and does not put it back — everything on the file is whole and rewriting an \
             append-only file is the one operation this design forbids (req/229 H-02, DR-43-11). \
             Take a copy of `{}` before anything else and compare it with a backup; the records the \
             ledger backs are the only ones a journal in this state is evidence for (43 §7.7)",
            engine.journal().path().display(),
        ));
    }
    if let Some(why) = rolled_back_remedy(&engine) {
        remedy_parts.push(why);
    }
    // 🔴 **R8 / `req/234` H-01 (c)** — the commit that has no receipt, and the way back.
    //
    // `req/234` measured this project state answering `rc=0 remedy: null head_authenticity:
    // "verified"` while `gx undo` refused the row forever, `GET /v1/receipts/{tid}` answered 404
    // and `gx receipt verify` exited 6. The count is the finding; this is the remedy it owes.
    if !missing_receipts.is_empty() {
        remedy_parts.push(format!(
            "{} of this project's {} committed leaf/leaves have no commit receipt under \
             `.gx/receipts/`. The commits themselves are whole — the journal witnesses them and \
             the ledger holds their leaves — but until the receipt is filed those rows cannot be \
             undone (`gx undo` exits 3 by design: DR-43-1 will not apply an inverse over a world \
             it cannot compare) and cannot be proved to a third party. Run `gx repair --yes \
             --reissue-receipts`: it reads the world, rebuilds each payload and files it **only** \
             if it digests to what the ledger already witnessed, so nothing is invented and \
             nothing is applied. A row it answers `world_moved` about is one whose postcondition \
             is no longer observable — keep the ledger and the checkpoint, which still prove the \
             commit happened (req/234 H-01, 43 §7-3b)",
            missing_receipts.len(),
            leaves
        ));
    }
    // 🔴 **R8 / `req/234` B-5** — Model B, said out loud rather than answered with a repair.
    if !missing_bodies.is_empty() {
        remedy_parts.push(format!(
            "{} escrowed inverse/inverses name a body `.gx/ledger/journal.blobs/` cannot \
             answer for: it is gone, or — when `damaged_bodies` above is not `0` — it is there \
             and does not read back as its own name. **How it got that way is not something \
             this report knows**: 43 §7.9 (b) names a third party who can write inside `.gx/`, \
             and 43 §7.11 (b) is the clause saying an accident makes the same shape — an \
             interrupted copy, a synchronising client, a directory somebody cleaned out \
             (req/240 L-03, L-04). Either way there is no repair: an inverse is the only copy \
             of what a change replaced, \
             gx does not keep a second, and `gx checkpoint export` copies the head and not the \
             blobs, so the artifact kept outside this machine cannot restore one either. What \
             still holds: the commit receipts and the ledger prove what was done, and \
             `inverse_status` now answers `BodyMissing` instead of `Available` (req/234 B-5)",
            missing_bodies.len()
        ));
    }
    // 🔴 **R9 / `req/236` H-01** — a body that is here and is not itself.
    if !damaged_bodies.is_empty() {
        remedy_parts.push(format!(
            "{} file(s) under `.gx/ledger/journal.blobs/` do not read back as the name they are \
             filed under: {}. A body at a content address either rebuilds into that address or it \
             is not that body — and a fragment there is worse than an absence, because until R9 \
             `inverse_status` answered `Available` for one (`req/236` H-01: a full disk left \
             204,800 bytes of a 400,096-byte inverse at its own address, and the next entirely \
             successful commit adopted it). This binary cannot leave one behind (the write is \
             tmp + rename, and a re-put compares the bytes) so these were written by an older one \
             or by a third party. There is no repair: an inverse is the only copy of what a change \
             replaced. What still holds is what the receipts and the ledger prove — and the rows \
             that name these bodies now answer `BodyMissing` rather than `Available`",
            damaged_bodies.len(),
            damaged_bodies.join(", ")
        ));
    }
    // 🔴 **R12 / `req/242` L-05** — `gitignore_absent` had a key and no sentence anywhere.
    //
    // The audit measured the whole of it: `rm .gx/.gitignore` → `gx submit` rc 0 and the file
    // stays gone (R11's M-02 repair, correct) → `gx repair` reports `gitignore_absent: true` and
    // the word "gitignore" appears **nowhere** in `remedy` → `gx repair --yes` leaves it absent
    // too. req/56 §4 asks an operator to edit exactly this file, so "it is not there" without
    // "here is what belongs in it" is a fact with no way out of it (`req/222` H-06's rule, one
    // file down). `--yes` still does not write it: R11 stopped `Layout::create` re-creating it
    // under an operator's edit and that is the finding this must not undo.
    if layout.gitignore_absent() {
        remedy_parts.push(
            "`.gx/.gitignore` is not there. gx does not write one back into a project that
             already exists (`req/240` M-02: what came back over an operator's edit was gx's
             default, silently), so this is yours to restore if you want it. What gx ships in a
             new project is three lines — a comment naming req/56 §4, a comment saying to
             un-ignore what you want to share (`!config.toml`), and `*`. The `*` inside `.gx/`
             ignores the whole directory including the file itself, which is what keeps gx's
             state out of your history without touching your own `.gitignore`. Nothing in gx
             reads this file: it decides what git sees and nothing about what gx wrote
             (`req/242` L-05)"
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    // 🔴 **R9 / `req/236` M-04** — the residue, named rather than left for somebody to find.
    if !staging.is_empty() && !writing {
        remedy_parts.push(format!(
            "{} staging file(s) are still here: {}. ~~Nothing reads them and no record names \
             them.~~ 🔴 **R12 / `req/240` L-05**: the struck sentence is false of one of the \
             names it covers — `<id>.commit.json.tmp` is what `Engine::recover` **renames** into \
             the commit receipt it re-issues, so a file with that name is either a receipt this \
             run is about to finish or the residue of one that was interrupted, and the report \
             cannot tell you which from the name alone. What is true of all of them: no ledger \
             leaf and no journal record points at one. `gx repair --yes` removes the ones **gx \
             wrote** — `head.json.tmp` and \
             `<id>.commit.json.tmp`/`<id>.verdict.json.tmp`, which are what an interrupted \
             atomic write leaves behind — and leaves every other name where it lies, because a \
             file gx did not write is not gx's to remove (req/240 L-01). They are reported \
             rather than swept by this mode because a report writes nothing (req/236 M-04)",
            staging.len(),
            staging.join(", ")
        ));
    }
    // 🔴 **R8 / `req/234` M-03** — a hypothesis about two files that agree is not a remedy.
    //
    // Every sentence below is about the journal and the ledger describing different trees. When
    // they describe the same one — which is the case for **every** refusal that comes from the
    // declaration, from the head document, or from an accepted rollback — printing one of them is
    // `req/227` M-04's failure repeated: "a remedy that names the wrong file is worse than none:
    // it is a hypothesis with gx's authority behind it". `req/234` H-02 caught the exact sentence,
    // telling an operator that "a ledger ahead of its journal" was the trouble on an object
    // reporting `journal_commits: 2` and `ledger_leaves: 2`.
    //
    // `journal_intact` is asked beside it, because a journal that was rewritten without changing
    // its record count leaves the two files "agreeing" and is still exactly what the first branch
    // below is for.
    let base_remedy = if after || (engine.journal_intact() && engine.files_agree()) {
        None
    } else if let Some(departure) = engine.journal_departure() {
        // 🔴 **R5 / `req/227` M-01** — the file that moved is named first.
        //
        // The sentences below are all about the *ledger* being ahead of or behind the
        // journal, and `req/227` M-04's probe watched one of them tell an operator whose journal
        // had been rewritten to go and check that `.gx/ledger/` had not been copied in from
        // somewhere else. A remedy that names the wrong file is worse than none: it is a hypothesis
        // with gx's authority behind it.
        //
        // 🔴 **R32 / `req/392` M-02 (§3-5 site 3)** — the family's third site, swept with the
        // other two (`req/38` §227's rule). This branch used to ask `chain_break()` and offer a
        // three-way disjunction for everything else, which made it the most honest of the three
        // and still left it with **no arm** for the two conditions the audit measured: a
        // declaration that outranks the file's framing, and a marker from a build this one has
        // never heard of. Both were told their journal "has grown shorter, its last record has
        // been rewritten, or bytes on it did not replay (0 byte(s) did not)" and were sent to
        // `<journal>.torn.<n>-<m>`. They now have arms, and the arms are keyed on the same value
        // `gx_api::journal_note` reads, so the JSON's remedy and the stderr paragraph cannot come
        // apart.
        Some(match departure {
            gx_engine::JournalDeparture::ChainBroken => format!(
                "the journal is not the journal gx wrote: its chain does not verify from byte \
                 {}, which means the record there is whole and is not the record that belongs \
                 there (DR-43-9). The ledger is not what moved. gx does not repair this and does \
                 not cut it — everything after a chain break is a whole record, so truncating \
                 would delete what nobody asked to lose, and `--yes` leaves these bytes alone \
                 too. Take a copy of `{}` before anything else and compare it with a backup",
                engine
                    .journal()
                    .chain_break()
                    .map_or_else(|| "unknown".to_string(), |at| at.to_string()),
                engine.journal().path().display(),
            ),
            gx_engine::JournalDeparture::TornTail => format!(
                "the journal is not the journal this process read: bytes on it did not replay \
                 ({} byte(s) did not), which is the ordinary shape of a process that died while \
                 it was writing. The ledger is not what moved. `gx repair --yes` opens the \
                 journal through the writer's door, which copies a tail that will not replay to \
                 `<journal>.torn.<n>-<m>` and then removes it (DR-43-7); what it cannot do is put \
                 back a record that was there",
                engine.journal().recovery().torn_tail_bytes,
            ),
            gx_engine::JournalDeparture::Shortened => format!(
                "the journal is not the journal this process read: it has grown shorter than the \
                 bytes this run had already read off it, so records that were folded are no \
                 longer on the file. The ledger is not what moved, no chain stopped verifying and \
                 no tail was torn. Take a copy of `{}` and compare it with a backup; gx cannot \
                 put back a record that is gone",
                engine.journal().path().display(),
            ),
            gx_engine::JournalDeparture::TailRewritten => format!(
                "the journal is not the journal this process read: its last framed record no \
                 longer reads the way it read when this run read it, at the same length. The \
                 ledger is not what moved, and this is not a torn tail — gx removes nothing here \
                 and `--yes` leaves these bytes alone. Take a copy of `{}` before anything else \
                 and compare it with a backup",
                engine.journal().path().display(),
            ),
            gx_engine::JournalDeparture::PrefixRewritten => format!(
                "the journal is not the journal this process read: the bytes it had already read \
                 back no longer produce the chain head it was carrying, so something behind the \
                 frontier was rewritten. The comparison is over the whole consumed prefix, so \
                 there is no single byte to name. The ledger is not what moved, gx removes \
                 nothing here and `--yes` leaves these bytes alone. Take a copy of `{}` before \
                 anything else and compare it with a backup",
                engine.journal().path().display(),
            ),
            // 🔴 **R32 / `req/392` M-01 + M-02** — the arm this branch did not have. A journal of
            // zero bytes reaches it too, since `replay` stopped reporting a file with no marker as
            // `chained-v2`.
            gx_engine::JournalDeparture::Downgraded => format!(
                "the journal is not the journal this project declares: `.gx/VERSION` says \
                 `journal_format={}` and the first eight bytes of `{}` carry no framing marker \
                 (a file of zero bytes carries none either). **No byte of the journal is claimed \
                 to have been rewritten** and the ledger is not what moved: what disagrees is the \
                 declaration and the marker, so those are the two things to compare. gx cuts \
                 nothing and removes nothing in this state",
                layout
                    .declared_journal_format()
                    .ok()
                    .flatten()
                    .map_or("unset", gx_engine::JournalFormat::kind),
                engine.journal().path().display(),
            ),
            // 🔴 **R32 / `req/392` M-02** — and the arm whose absence made this build tell an
            // operator that a file it had just declared healthy had moved.
            gx_engine::JournalDeparture::FromANewerGx => format!(
                "`{}` carries a framing marker this build has never heard of, so it was written \
                 by a newer `gx` (req/372 M-02). **Nothing is wrong with the file**: this binary \
                 cannot verify it, and nothing was truncated, quarantined or appended — the bytes \
                 are where the newer binary left them. Do not repair it with this build; run the \
                 `gx` that wrote it",
                engine.journal().path().display(),
            ),
        })
    } else if frontier > leaves as usize {
        Some(format!(
            "the journal witnesses {frontier} commit(s) and the ledger holds {leaves}: {} leaf/leaves \
             are missing, and gx cannot rebuild them from the journal — 42 §3.13's `Committed` \
             record carries no receipt digest, so a leaf built here would be invented rather than \
             replayed. The bytes may still exist: look for `<ledger>.torn.<n>-<m>` beside the \
             ledger (DR-43-7 quarantines before it truncates) and for the commit receipts under \
             `.gx/receipts/`, which do carry `ledger_digest`. Rebuilding a tree from those is \
             DR-43-8's second half and is not implemented",
            frontier - leaves as usize
        ))
    } else {
        // 🔴 **R13 / `req/244` H-03** — the assertion is gone, because it was false.
        //
        // R4 wrote "if this persists, the two files are from different projects". `req/244` H-03
        // measured a `gx wrap` commit killed inside §7-3b's window — same project, same run, one
        // crash — and this sentence sent the operator to look for a `.gx/ledger/` somebody had
        // copied in. There was none, in every arm of the sweep. A remedy that asserts a cause it
        // has not measured is worse than one that says nothing: it spends the reader's time and it
        // carries gx's authority while doing it.
        //
        // What replaces it is the fact the report now holds beside it (`journal_behind_by`), the
        // two windows §7-3b actually has, and — for the half gx cannot close — what is still true
        // about those leaves. `recover`'s own refusal sentence (`refusals`) names which window this
        // project is in.
        Some(format!(
            "the ledger holds {leaves} leaf/leaves and the journal witnesses {frontier} commit(s), \
             so the journal is {behind} commit(s) behind (`journal_behind_by`). A ledger ahead of \
             its journal is 43 §7-3b's crash window: `ledger.append` landed and the `Committed` \
             record did not. There are two windows inside it and they close differently. If the \
             commit receipt was filed before the crash (`.gx/receipts/<TID>.commit.json`, written \
             inside the critical section since R8), `gx repair --yes` writes the missing record \
             from that document and asks no adapter anything — `recover.closed_from_receipt` \
             counts those. If the crash landed in the narrower window before the receipt was \
             filed, the payload has to be rebuilt, and rebuilding it needs a reading of the \
             substrate: run `gx repair --yes` from a process that can reach it (for a `gx wrap` \
             commit, the same `--mcp-server`). What is true either way, and does not need this to \
             be closed: every leaf that has a receipt is provable to a third party \
             (`gx receipt verify`), and gx will not write a terminal record over one of these rows \
             — the row stays resumable (req/244 H-03)",
            behind = (leaves as usize).saturating_sub(frontier)
        ))
    };
    remedy_parts.extend(base_remedy);
    // 🔴 **R14 / `req/246` M-04** — and the one fact that keeps every writer out of this project
    // says so in the remedy rather than only in a key.
    //
    // The audit's whole finding was the pair: `gx submit` refused `INTERNAL` "File exists" for ever
    // while this verb answered exit 0 with `remedy: null`, and the only trace was
    // `repair_record.written: false` — "a key that moves neither the status nor the remedy".
    // 🔴 **R15 / `req/259` M-01** — all of them, not the first one.
    if let Some(why) = blocked_why(&repair_dir_blocked) {
        remedy_parts.push(why);
    }
    let remedy = if remedy_parts.is_empty() {
        None
    } else {
        Some(remedy_parts.join(" "))
    };
    // 🔴 **R14 / `req/246` M-04** — a project whose declared directory is occupied cannot be
    // written to, and 44 §1.2 gives this verb's **0** to "this project can be written to".
    // 🔴 **R15 / `req/259` M-01** — any of the declared directories, not one of them.
    let repair_dir_still_blocked = still_blocked(&repair_dir_blocked);

    // 🔴 The status is the answer, not the effort. 44 §1.4's **1** is "unable to execute", and a
    // project that still disagrees is one where the next write is still going to be refused — so a
    // repair that ran cleanly and did not fix it exits 1, and a `&&` chain stops where it should.
    // The JSON is on stdout either way, because the diagnosis is the deliverable.
    // 🔴 **R7 / `req/232` M-04 + L-01** — and a question that could not be asked is not an answer of
    // `healthy` either: a `--against` file that is unreadable or belongs to another project exits 1
    // with the sentence above, rather than exiting 0 and letting an operator read the silence as a
    // clean bill.
    // 🔴 **R8 / `req/234` H-01** — and a committed leaf with no receipt is not `healthy` either.
    //
    // The audit's finding was a **pair**: `rc=0` and `remedy: null` over a row that `gx undo` would
    // refuse for ever. The remedy above closes the second half. This closes the first, for the
    // reason `--against`'s unusable-file arm already exits 1 (`req/232` L-01): a question that
    // could not be answered must not be read as a clean bill. A monitor branching on this verb's
    // status now sees the difference between "this project can prove what it did" and "this
    // project committed something it cannot show anybody".
    // 🔴 **R9 / `req/236` H-04** — a project whose declaration will not parse is not `healthy`.
    //
    // The report opens (that is the repair); the status still says that the next write will be
    // refused, for the reason `--against`'s unusable-file arm already exits 1: a question that
    // could not be answered must not be read as a clean bill.
    // 🔴 **R10 / `req/238` H-01** — and a project whose declaration or settings are **gone** is
    // not `healthy` either, for the reason an unreadable declaration is not: the report opens (that
    // is the repair), and the status still says that the next write will be refused. A `--yes` run
    // that put the file back has `meta_repaired` non-empty and `config_absent_before` false, so it
    // is judged on what it left rather than on what it found.
    let meta_still_wrong = matches!(declaration_fault_error, Error::DeclarationAbsent { .. })
        || (config_absent_before && !writing);
    // 🔴 **R11 / `req/240` H-01** — and a `--yes` that could not write is not `healthy` either.
    //
    // It exited 1 before this lane as well; what has changed is that the 1 now arrives with the
    // diagnosis on stdout instead of an empty one, so `gx repair --yes || gx repair` is no longer
    // the only way to see what a repair could not do.
    let code = if let Some(status) = key_fault {
        // 🔴 **R13 / `req/244` M-05** — a named key that would not resolve keeps its own number.
        //
        // `gx repair --signing-key does-not-exist` answered rc **6** `NOT_FOUND` before this lane
        // and it answers rc 6 now; what changed is that the report is on stdout beside it instead
        // of nothing at all. Folding it into `exit::ERROR` would move a status a script already
        // branches on, which is `req/38` §148's rule read the other way round: do not mint numbers,
        // and do not quietly retire them either.
        status
    } else if matches!(declaration_fault_error, Error::Declaration { .. })
        || meta_still_wrong
        || meta_repair_refused.is_some()
        || repair_dir_still_blocked
    {
        exit::ERROR
    } else if after && !against_refused && !against_unusable && missing_receipts.is_empty() {
        exit::OK
    } else {
        exit::ERROR
    };
    // 🔴 **R13 / `req/244` H-01** — the report is a value here before it is a stream anywhere.
    //
    // It has to be: `file_repair_record` writes this same object to `.gx/repair/last.json` so that
    // a run whose stdout dies leaves the fact behind, and a `json!` built straight into
    // `Outcome::refused` is an object nothing can hand to a second reader.
    let mut report = serde_json::json!({
        "project": layout.root().display().to_string(),
        // 🔴 **R6 / `req/229` M-05** — `repaired` is what happened, not what was asked for.
        //
        // It was `yes` — the flag, copied into the report. The audit ran `gx repair --yes` on a
        // project DR-43-9 (c-3) forbids touching, got `repaired: true`, and measured the journal's
        // and the ledger's md5 **unchanged** on both sides of the run, twice. 44 §1.2 publishes
        // this key, so a key that reports its own argument is a key that tells an operator a repair
        // happened where the correct behaviour was to do nothing. What was asked for is in `mode`.
        "repaired": quarantined_any || resumed_rows > 0,
        "mode": if yes { "yes" } else { "report" },
        "caught_up_records": caught.records,
        "recovery": quarantine,
        "recover": recovered_counts,
        "ledger_agrees_before": before,
        "ledger_agrees_after": after,
        // 🔴 **R4 / `req/225` H-03** — which of the two files moved. `ledger_agrees_after` is
        // `false` for a rewritten journal as well as for two files that count differently, and an
        // operator reading only that number would go and look at the wrong file. 44 §1.2's v0.4-q
        // note carries the key.
        "journal_intact": engine.journal_intact(),
        // 🔴 **R5 / DR-43-9** — which framing this project's journal is in, and where its chain
        // stopped verifying. `"legacy"` is a journal written before DR-43-9: it is read and
        // appended to, and 43 §7.6's R5 note says what its records are worth (only the ones the
        // ledger backs).
        "journal_format": engine.journal().format().kind(),
        // 🔴 **R6 / `req/229` H-02** — what the project *declared* it was, beside what it is.
        //
        // `journal_format` alone answered `"legacy"` for two entirely different projects: one
        // written before DR-43-9, and one whose chain an attacker removed this morning. 44 §1.2
        // v0.4-r told a machine to read that key to know which file to look at, and `req/229` M-07
        // measured it pointing at the wrong one. The declaration is what separates them.
        // 🔴 **R12 / `req/242` H-02** — `null` for a declaration this run could not read,
        // which is the same answer it gives for one that was never made. The `?` that was here
        // could take the whole report down over `.gx/VERSION` — the file `gx repair` exists to
        // report on.
        "journal_format_declared": layout
            .declared_journal_format()
            .ok()
            .flatten()
            .map(gx_engine::JournalFormat::kind),
        // 🔴 **R6 / `req/229` H-02** — the two above, compared, so a machine does not have to.
        "downgraded": engine.journal().downgraded(),
        "journal_chain_break_at": engine.journal().chain_break(),
        // 🔴 **R6 / DR-43-11 / `req/229` H-01** — why this project is behind its own signed head.
        //
        // `null` for a project that is not, **and** for one that has never recorded a head. A
        // reader must not read `null` as "this project has not moved backwards": it means "no
        // statement about the past was available to compare against", and `.gx/checkpoints/head.json`
        // is where a statement would be.
        "rolled_back": engine.rolled_back(),
        // 🔴 **R7 / `req/232` H-01** — "is a head **document** here", which is what the name says.
        //
        // R6 answered `head_floor().is_some()`, so the key meant "there is a head and it is one we
        // will compare against" — two facts under one name, and the audit's attack turned the
        // second one false while the key went on saying `true`. The two facts are now two keys:
        // this one is about the file, `head_authenticity` is about the document.
        "head_recorded": engine.head_authenticity() != gx_engine::HeadAuthenticity::Absent,
        // 🔴 **R7 / `req/232` H-01** — *and whether that head is one this binary checked.*
        //
        // `head_recorded: true` was the only signal R6 gave an operator, and the audit's cheapest
        // attack left it saying exactly that: the file was there, its numbers had been rewritten,
        // and nothing had ever looked at its signature. The two facts are now two keys, because
        // they are two facts: `absent`, `unverified` (no key for the id this document names — the
        // honest answer, and **not** a pass), `verified`, `refuted`.
        "head_authenticity": engine.head_authenticity().as_str(),
        // 🔴 **R7** — why a head was refused, when it was. `null` and `head_authenticity:
        // "refuted"` cannot both happen; a reader may branch on either.
        "head_invalid": engine.head_invalid(),
        // 🔴 **R7 / `req/38` §171 ruling 2(c)** — the rollback this run was told to accept.
        "accepted_rollback": engine
            .accepted_rollback()
            .map(|accepted| serde_json::json!({
                "was_tree_size": accepted.was_tree_size,
                "was_root_hash": accepted.was_root_hash,
            })),
        // 🔴 **R45-fast-follow / `req/654` M-1, ruling `req/38` §394** — the same key, the same
        // question, the same spelling as the road one function down.
        //
        // `report_without_engine`'s `ledger_present` (`repair.rs:2357`) asks
        // [`crate::layout::presence_of`], because a declared path holding a dangling symbolic link is
        // something that is there (`attach.rs::present`), whatever it points at. This road answered
        // the same key with `engine.ledger().present()` — `LedgerStore::present()`, `self.file
        // .is_some()`, set from [`gx_log::store::LedgerStore::open_read_only_or_absent`]'s
        // `path.exists()`, a call that **follows** the final link and folds a dangling one to `false`.
        // `req/650` measured the split live: the same dangling link at `.gx/ledger/journal.ledger`
        // read `false` here and `true` there, and which road a run took turned on whether the journal
        // was present — an unrelated fact. §394 ruled the meaning of this key ("something is at this
        // path", one key, no shape split), so both producers carry it now, in the same three arms.
        //
        // Unchanged for the two shapes already correct: a real file is `Present` → `true`, and a
        // genuinely absent ledger is `Absent` → `false` (`serve_runtime_r6::m02`, `req/229` M-02).
        // The engine's own fold in `open_read_only_or_absent` is left standing and named in
        // `docs/LIMITS.md` ("Unrepaired in the same round"); this key no longer reads through it.
        "ledger_present": presence_of_as_json(&layout.ledger_path()),
        "against": against_report,
        // 🔴 **R5 / `req/227` M-04** — a chain the reader's door found absent rather than empty.
        //
        // 🔴 **R43 / `req/578` §2, ruling `req/38` §350 item 1 (addendum S-7)** — and it asks
        // [`crate::layout::presence_of`] (via [`presence_of_as_json`]), for the reason its sibling two
        // thousand lines down already gives. R41's S-6 fixed *this expression* in
        // `report_without_engine`; the same characters stood here, in `repair_and_report`, one
        // function away and on the road an operator sees when the engine **did** open, so R41's
        // scope sentence ("the same fold in `report_without_engine`") left it standing.
        //
        // `Path::exists()` follows the link and folds every failure into "it is not there".
        // `attach.rs::present` writes the rule this door was breaking: a symbolic link where a
        // declared path belongs is something that **is** there, whatever it points at. Measured
        // (`tests/r43_presence_and_head.rs` bed-L): with a link at `.gx/ledger/journal.verdicts`
        // that resolves to nothing, the engine still opens and this key said `false` about a path
        // holding a link.
        //
        // 🔴 What the ruling's KA-1 bed could not be: "the `verdicts` file individually
        // unreadable" is **not constructible by file mode** — `stat(2)` does not consult the
        // file's own permission bits, so a `chmod 000` chain is still `stat`-able and `exists()`
        // still answers `true`. Only the directory around it gates the lookup, and taking
        // `.gx/ledger/` down takes the engine with it, which is bed-E and the other function's
        // road. `Undetermined` is therefore unreachable *here* today and is answered anyway: a
        // door that answers what it measured does not depend on which failures are reachable this
        // month.
        "verdict_chain_present": presence_of_as_json(&layout.ledger_path().with_extension("verdicts")),
        // 🔴 **R5 / `req/227` M-03** — whether this report was produced under the project lock.
        "lock_held": lock.is_some(),
        "journal_commits": frontier,
        "ledger_leaves": leaves,
        // 🔴 **R13 / `req/244` H-03** — the subtraction, as one number.
        //
        // `ledger_agrees_before`/`_after` are booleans that fold four conditions together, and
        // `journal_commits` and `ledger_leaves` are two numbers a reader has to subtract. The
        // audit's §10 asks for exactly this key on the GUI's behalf: "`ledger_leaves ≠
        // journal_commits` should be a red line of its own". `0` for a project whose journal is
        // level with or ahead of its ledger — the other direction is the `frontier > leaves`
        // remedy above and is a different fact.
        "journal_behind_by": (leaves as usize).saturating_sub(frontier),
        "journal_rows": engine.shadow().len(),
        // 🔴 **R8 / `req/234` M-03** — do the journal and the ledger agree **with each other**.
        //
        // `ledger_agrees_before`/`_after` above are the **gate**: R4, R6 and R7 folded a moved
        // journal, a rollback and an unbelievable head into them on purpose, so that every road
        // that must not write inherits all four conditions. `req/234` M-03 measured what that
        // costs a *report*: an editor's trailing newline in `.gx/VERSION` produced
        // `ledger_agrees_before: false` on the same object as `journal_commits: 2` and
        // `ledger_leaves: 2`. Both keys are published now, and nothing branches on this one.
        "files_agree": engine.files_agree(),
        // 🔴 **R8 / `req/234` H-01** — commit receipts held, and leaves that have none.
        "commit_receipts": receipt_store.commit_count(),
        "receipts_missing": missing_receipts.len(),
        "receipts_missing_ids": missing_receipts
            .iter()
            .map(|id| id.0.to_text())
            .collect::<Vec<_>>(),
        "reissued": reissued,
        // 🔴 **R8 / `req/234` B-5** — escrowed inverses whose body is no longer in the blob store.
        "escrow_bodies_missing": missing_bodies.len(),
        "escrow_bodies_missing_ids": missing_bodies
            .iter()
            .map(|id| id.0.to_text())
            .collect::<Vec<_>>(),
        // 🔴 **R9 / `req/236` H-01** — bodies that are filed and do not rebuild into their own name.
        "damaged_bodies": damaged_bodies.len(),
        "damaged_body_names": damaged_bodies,
        // 🔴 **R9 / `req/236` M-04** — the `.tmp` residue of interrupted writes, in all three
        // directories that make one, and what `--yes` removed.
        "staging_files": staging,
        "staging_files_swept": swept,
        // 🔴 **R9 / `req/236` H-04** — whether this project's own declaration reads.
        // 🔴 **R10 / `req/238` H-01** — a file that is not there does not read either.
        //
        // R9 minted this key for "present and does not parse", which was the only declaration
        // fault there was. With absence classified, leaving it `true` for a missing file would put
        // `declaration_readable: true` on the same object as `declaration_absent: true` and hand a
        // monitor two keys that contradict each other.
        "declaration_readable": !matches!(
            declaration_fault_error,
            Error::Declaration { .. } | Error::DeclarationAbsent { .. }
        ),
        // 🔴 **R10 / `req/238` H-01** — and whether it is there at all, which is a different fact.
        //
        // Before R10 this project answered exit 6 `NOT_FOUND` with **no report object**, and the
        // next `gx submit` wrote the file back in silence. Both keys are published so that a
        // monitor can tell "there and broken" from "gone", and `meta_repaired` says what `--yes`
        // did about it.
        "declaration_absent": matches!(declaration_fault_error, Error::DeclarationAbsent { .. }),
        "config_absent": config_absent_before,
        "meta_repaired": meta_repaired,
        // 🔴 **R11 / `req/240` H-01 + M-05 (audit 10 M-03)** — why a `--yes` run wrote nothing.
        //
        // `null` for a report and for a `--yes` that had a key. A sentence when `--yes` was asked
        // for and the key would not resolve: the run degrades to a report — it reads everything,
        // touches nothing, exits 1 — instead of raising `VALIDATION_ERROR` with an empty stdout
        // over a project it had already written to.
        "meta_repair_refused": meta_repair_refused,
        // 🔴 **R11 / `req/240` M-03** — the `*.pre-repair.<n>` copies this project is holding.
        //
        // Beside `staging_files` and for the opposite reason: those are residue and `--yes`
        // removes them, these are **evidence** and nothing removes them. Reported so that the
        // number is somebody's business before it reaches `layout::PRE_REPAIR_LIMIT` and the
        // repair itself stops.
        "kept_aside": layout.kept_aside(),
        // 🔴 **R11 / `req/240` M-02** — req/56 §4's file, which `Layout::create` used to write back
        // into an established project in silence.
        "gitignore_absent": layout.gitignore_absent(),
        // 🔴 **R11 / `req/240` H-02** — false here by construction: the branch above answers the
        // project whose journal is gone. Published on both objects so that a monitor reads one key
        // rather than inferring absence from a report that is missing keys.
        "journal_absent": false,
        // 🔴 **R10 / audit 8 M-02 (`req/234`), carried through `req/236` §6 and `req/238` §6** —
        // how far behind its own signed head this project is, as a number.
        //
        // The three head keys were `head_authenticity`, `head_invalid` and `head_recorded`, and
        // `rolled_back` is a **sentence**. A monitor that wants to alert on "this project is more
        // than n leaves behind what it published" had to parse prose. `null` when no head is
        // recorded, `0` for a project that is level with or ahead of it.
        "head_behind_by": engine
            .head_floor()
            .map(|floor| floor.tree_size.saturating_sub(leaves)),
        // 🔴 **R10 / audit 8 M-05** — what `journal_intact` is a statement **about**.
        //
        // `journal_intact: true` means two different things depending on the framing: for a
        // chained journal (DR-43-9) the records verify against one another, and for a legacy one
        // there is no chain and the only thing checked is that the file this process read has not
        // grown shorter or had its last framed record rewritten. Reading `true` without that
        // distinction is reading a weaker guarantee as the stronger one, which is what audit 8 M-05
        // named. Three values, so nothing has to be inferred: `"chain"`, `"length-only"`,
        // `"not-intact"`.
        "journal_intact_basis": if !engine.journal_intact() {
            "not-intact"
        // 🔴 **R30 / `req/372` M-02** — both chained framings carry a per-record link, so both
        // answer `"chain"`. The basis is about *what the intactness was established from*, and the
        // vocabulary version does not change that.
        } else if engine.journal().format().is_chained() {
            "chain"
        } else {
            "length-only"
        },
        "remedy": remedy,
        // 🔴 **R12 / `req/242` H-02** — the key that is `null` here and a sentence in the
        // two reports composed without an engine. It exists on all three objects so that the key
        // set a monitor branches on is one set (R11's forty-seven, now forty-eight).
        "engine_open_failed": serde_json::Value::Null,
        // 🔴 **R13 / `req/244` H-01** — the two keys that survive a dead stdout.
        //
        // `previous_repair` is the report the **last** `--yes` run filed, read back off
        // `.gx/repair/last.json`, or `null` for a project no repair has written in. It is what
        // makes "a run wrote `.gx/VERSION` and its report went into a closed pipe" a fact the next
        // command can still be told, which is precisely what the audit measured nobody being told.
        // `repair_record` says whether *this* run's copy landed; it is filled below rather than
        // here, because the object being filed is this one.
        "previous_repair": previous,
        "repair_record": serde_json::Value::Null,
        // 🔴 **R14 / `req/246` M-04** — `null` when `.gx/repair` is a directory or is not there,
        // and an object naming the path when something else is sitting in it. The fifty-first key,
        // in the same position on all three reports (`model_a_probes` compares them as an ordered
        // list, which is what catches a key that drifted into one of them and not the others).
        "repair_dir_blocked": repair_dir_blocked,
    });
    if permitted_to_write {
        let filed = file_repair_record(layout, &report, previous_bytes);
        if let Some(map) = report.as_object_mut() {
            map.insert("repair_record".to_string(), filed);
        }
    }
    Outcome::refused(report, code)
}

/// 🔴 **R11 / `req/240` H-02, generalised in R12 / `req/242` H-02** — why there is no engine.
///
/// Two shapes, one report. R11 built the first: a project whose `.gx/ledger/journal` is gone is
/// measured off the files that are still there rather than answered with a constant. `req/242`
/// H-02 produced the second: an engine that will not **open** — a ledger `chmod 000`, a regular
/// file where `journal.blobs/` belongs, a directory where `journal.ledger` belongs — took the whole
/// report down through a `?` after `gx repair --yes` had already written the declaration. Both are
/// "everything except the engine can still be read", so both get the same forty-eight keys.
enum NoEngine {
    /// `.gx/ledger/journal` is not there. `--yes` writes nothing at all on this road: what it puts
    /// into `.gx/VERSION` is the framing sniffed off the journal's first eight bytes.
    JournalAbsent,
    /// The engine's open, `catch_up` or `recover` refused. `stage` is which, and `detail` is the
    /// refusal's own sentence.
    Refused {
        /// `open`, `catch_up` or `recover`.
        stage: &'static str,
        /// What the refusal said.
        detail: String,
        /// 🔴 **R36 / `req/476` H-01** — the transformations whose delta this run **applied** and
        /// could not record, in 42 §1.2's text form. Empty on `open` and `catch_up`, which write
        /// nothing, and empty on a `recover` that raised before reaching an apply.
        ///
        /// The word `refused` was doing real damage while this was missing. This same binary, on
        /// 43 §7-3b's road, prints "**Nothing was applied** ... `adapter.apply` was never called" —
        /// so an operator who reads `stage: "recover"` beside `repaired: false` and `recover: null`
        /// has been told, in the product's own vocabulary, that their world was left alone. Audit
        /// 35 measured the file underneath saying otherwise.
        applied: Vec<String>,
        /// 🔴 **R37 / `req/496` M-01** — the transformations whose delta this run applied **and
        /// whose terminal `Committed` record it wrote**, failing only on the head.
        ///
        /// Disjoint from `applied` by construction (`RecoverPartial`'s two lists are separated by
        /// `journal_append(Committed)`), and it exists because the two need different remedies: a
        /// row in `applied` is unfinished and the next write verb closes it, while a row here is
        /// **closed** and the next write verb only records a head. Audit 36 measured the second
        /// being told the first — an instruction with nothing to act on.
        recorded: Vec<String>,
        /// How many rows this recovery closed before it raised.
        ///
        /// 🔴 **R37 / `req/496` M-02** — from `RecoverPartial::closed_by_this_run`, not from
        /// `finished.len()`. The vector also carries the rows that were already `Committed` when
        /// the journal was replayed, and this field said `1` on a bed where the recovery closed
        /// none while the remedy called that row one that "had already been finished by this same
        /// recovery".
        finished: usize,
    },
}
///
/// # What was here before, and what it cost
///
/// R4 (`req/225` H-01) answered this shape with a constant: `ledger_agrees_before: true`,
/// `journal_commits: 0`, `ledger_leaves: 0`, `remedy: null`, exit **0**, and thirteen keys where a
/// report has forty-seven. Its reason — "a project with no journal is not one to diagnose" — is
/// true of a **directory that is not a project**, and R10 built the predicate that says which is
/// which ([`Layout::established`]). `req/240` H-02 measured the branch on the other side of it: a
/// project with two committed leaves, two commit receipts and a signed head, whose journal a
/// backup restore had dropped, was told `repaired: false, remedy: null` at exit **0** — and was
/// refused `LEDGER_DISAGREES` by the very next `gx submit`. The two leaves that were on the disk
/// were printed as `0`. A diagnosis that answers "nothing is wrong" about a project it is about to
/// call broken is worse than no diagnosis: it is one with gx's authority behind it.
///
/// # The rule this follows
///
/// **Measured or `null`, never a constant.** Every fact that can be read without the journal is
/// read off its own file — the ledger through the reader's door (`crate::ledger::open`, which
/// creates nothing), the receipts off `.gx/receipts/`, the head off `.gx/checkpoints/head.json`,
/// the declaration off `.gx/VERSION` — and everything that needs the engine is `null`, which says
/// "not measured" where `false` and `0` would say "measured, and absent".
///
/// Nothing here writes, including under `--yes`: what `--yes` puts into `.gx/VERSION` is the
/// framing sniffed off the journal's first eight bytes, and there is no journal to sniff, so a
/// declaration written now would be gx's guess recorded as the project's own statement.
fn report_without_engine(layout: &Layout, found: WithoutEngine<'_>, why: &NoEngine) -> Outcome {
    let WithoutEngine {
        yes,
        declaration_fault,
        config_absent_before,
        meta_repaired,
        meta_repair_refused,
        lock_held,
        permitted_to_write,
        previous_bytes,
        repair_dir_blocked,
    } = found;
    // The ledger, through the door that creates nothing. `Err` is a ledger that is absent or will
    // not replay, and both are `null` rather than a number.
    let ledger = crate::ledger::open(layout).ok();
    let leaves = ledger.as_ref().map(|store| store.log().len());
    let receipts = crate::receipt::ReceiptStore::in_layout(layout);
    let missing_receipts: Option<Vec<String>> = ledger.as_ref().map(|store| {
        store
            .log()
            .entries()
            .iter()
            .map(|entry| entry.transformation)
            .filter(|id| {
                !matches!(
                    receipts.get(id, crate::receipt::StoredKind::Commit),
                    Ok(Some(_))
                )
            })
            .map(|id| id.0.to_text())
            .collect()
    });
    // 🔴 **R43 / `req/578` §3, ruling `req/38` §350 item 2 (addendum S-8)** — one `read`, and two
    // values that are deliberately **not** the same value.
    //
    // `HeadStore::read` separates its answers in its own words: `Ok(None)` is "this project never
    // recorded a head", and `Err` is a refusal — a head that will not parse, or one this process
    // could not read. `.ok().flatten()` erased that line, and `head_recorded` below reported
    // `false` for both. Measured (`tests/r43_presence_and_head.rs` bed-M): with a head that will
    // not parse, this report said the project records none.
    //
    // The fix is a split rather than a conversion, because the same bool had two jobs.
    // `head_recorded` is a key a monitor branches on and now answers `null` when the read failed
    // (R11's rule for this whole report: measured or `null`, never a constant). `witnessed` below
    // is an **exit** input, and it goes on reading exactly the value it read before — `Err` folded
    // to "no head", which for that question is the fail-safe direction and is the behaviour
    // `req/38` §148 forbids moving. Two purposes, two variables, one `read`.
    let head_read = gx_log::HeadStore::at(layout.head_path(), crate::ledger::DEFAULT_ORIGIN).read();
    let head_recorded = match &head_read {
        Ok(recorded) => serde_json::Value::Bool(recorded.is_some()),
        Err(_) => serde_json::Value::Null,
    };
    let head = head_read.ok().flatten();
    let head_tree_size = head
        .as_ref()
        .and_then(|head| head.floor().ok())
        .map(|floor| floor.tree_size);
    let declaration_absent = matches!(declaration_fault, Some(Error::DeclarationAbsent { .. }));
    let declaration_readable = !matches!(
        declaration_fault,
        Some(Error::Declaration { .. } | Error::DeclarationAbsent { .. })
    );
    // Is this a project that has lost something, or a directory nothing has written to yet?
    // `Layout::open_reporting` has already answered "it is a project"; this asks the second
    // question, the one the remedy and the status turn on: does it hold evidence that commits
    // happened. `gx key gen` in a fresh directory creates `.gx/` and no journal, and telling that
    // operator their project is damaged would be the mirror of the finding this branch closes.
    let witnessed = leaves.unwrap_or(0) > 0 || head.is_some() || receipts.commit_count() > 0;
    // 🔴 **R14 / `req/246` M-02** — and there is a **third** answer, which R13 built the writer's
    // half of and did not give this report.
    //
    // `Layout::create` refuses a project that holds no witness of a commit and holds entries in
    // `.gx/index/`, `.gx/evidence/` or `.gx/drafts/`: `HISTORY_LOST`, no `--yes` road, restore from
    // a backup (R13, `req/244` M-04). This function asked only `witnessed` and therefore answered
    // the same project with exit **0** and the sentence "this is what `.gx/` looks like after
    // `gx key gen` in a fresh directory" — three runs, no variation, while `gx submit` was refusing
    // it. 43 §7.15 (b)'s rule is one predicate per question, and "has this project been used" was
    // being answered by two doors; this is the same predicate, in the same words, on this one.
    //
    // **What is not changed**: the refusal itself. R13's judgement that there is no `--yes` road
    // out of a lost history stands — inventing one is worse than the loss. What moves is the exit
    // status and the sentence, which is where the audit put the finding.
    let history_lost = if witnessed {
        None
    } else {
        Layout::used_without_witness(layout.root())
    };
    let remedy = if let Some(evidence) = &history_lost {
        Some(format!(
            "`{}` is not there, and neither is anything that witnesses a commit — no leaf in the \
             ledger, no commit receipt under `.gx/receipts/`, no recorded head. What this directory \
             does hold is the trace of having been used: {evidence}. So this is not a project \
             nothing has been written to; it is one whose log is **gone**, and `gx submit` refuses \
             it by name (`HISTORY_LOST`) rather than starting a second history over the top of the \
             first. gx does not rebuild the log from what is left — a repair that invented a \
             history would be a worse answer than the loss — so what is left is the backup: restore \
             `.gx/` from the copy taken before this, and run this verb again. There is no \
             `gx repair --yes` road out of this state, and the exit status is 1 because no verb \
             that writes will run until the backup is back (req/246 M-02, req/244 M-04)",
            layout.journal_path().display()
        ))
    } else if witnessed {
        Some(format!(
            "`{}` is not there, and this project is one that has been written to: {} leaf/leaves \
             in the ledger, {} commit receipt(s) under `.gx/receipts/`, and {} recorded head. gx \
             does not rebuild the journal from them — the ledger's leaves were built **from** \
             those records, and a journal written here would be a witness statement gx composed \
             rather than one it kept — so what is left is the copy taken before it went: restore \
             `.gx/ledger/journal` from a backup, or from whatever removed it, and run this verb \
             again. What still holds without it: the commit receipts and the ledger prove what was \
             done and a third party can check them (`gx receipt verify --offline`), and the \
             recorded head is this project's own signed statement about its past. What does not: \
             no verb that writes will run, and `gx repair --yes` cannot help, because the repair \
             it runs is 43 §7's recovery **over a journal** (req/240 H-02, 43 §7.9 (b))",
            layout.journal_path().display(),
            leaves.map_or_else(|| "an unreadable number of".to_string(), |n| n.to_string()),
            receipts.commit_count(),
            if head.is_some() { "a" } else { "no" },
        ))
    } else {
        // 🔴 **R12 / `req/242` L-04** — exit 0 and `journal_absent: true` on one object.
        //
        // The judgement is right (a `gx key gen` in an empty directory is not a damaged project),
        // and `req/242` put it at L for that reason. What it costs is a monitor reading two keys
        // that look like they disagree, with `remedy: null` between them. The remedy says which of
        // the two readings is the one this run made.
        Some(format!(
            "`{}` is not there, and neither is anything that would say a commit ever happened: no \
             leaf in the ledger, no commit receipt under `.gx/receipts/`, no recorded head. This \
             is what `.gx/` looks like after `gx key gen` in a fresh directory, so gx reads it as \
             a project nothing has been written to rather than as one that lost its log — which \
             is why the exit status is 0 while `journal_absent` is `true`. If this directory did \
             hold work, the three witnesses are gone as well, and what is left is the backup \
             (`req/242` L-04). 🔴 That last sentence is now a **measured** branch rather than a \
             caveat: `.gx/index/`, `.gx/evidence/` and `.gx/drafts/` were looked at, they hold \
             nothing, and a directory that had been used would have been answered by name here \
             (`req/246` M-02)",
            layout.journal_path().display()
        ))
    };
    // 🔴 **R12 / `req/242` H-02** — an engine that refused is always exit 1, whatever the
    // project's own state: the question was asked and could not be answered, which is the rule
    // `--against`'s unusable-file arm already follows.
    // 🔴 **R14 / `req/246` M-02 + M-04** — two more ways this project is one no writer will open,
    // and 44 §1.2 gives this verb's 0 to "this project can be written to".
    // 🔴 **R15 / `req/259` M-01** — any of the declared directories, not one of them.
    let repair_dir_still_blocked = still_blocked(&repair_dir_blocked);
    let code = if witnessed
        || history_lost.is_some()
        || repair_dir_still_blocked
        || matches!(why, NoEngine::Refused { .. })
    {
        exit::ERROR
    } else {
        exit::OK
    };
    // 🔴 **R13 / `req/244` H-02** — and when the settings are gone too, the remedy names them.
    //
    // The audit's form ① is `.gx/config.toml` and `.gx/ledger/journal` deleted together, and what
    // it measured was a refusal that talked only about the journal: `remedy` held the word "config"
    // **zero** times, in every run of both forms, while `gx submit` was refusing `CONFIG_ABSENT`.
    // The two facts are now one sentence, and `--yes` has already acted on the half it can.
    let remedy = match (&remedy, config_absent_before) {
        // 🔴 **R14 / `req/246` L-03** — the same sentence, without the literal newline and thirteen
        // spaces a missing line continuation put inside a JSON string a buyer reads.
        (Some(text), true) => Some(format!(
            "{text} And `{}` is not there either. That is the file `gx submit` refuses on \
             (`CONFIG_ABSENT`), so it is the refusal an operator meets first even though the \
             journal is the larger loss. `gx repair --yes` writes it back — its bytes are the \
             shipped default and ask the journal nothing — and `meta_repaired` above says whether \
             this run did. Afterwards put your `engine_signing_keyid` line back, or pass \
             `--signing-key` for one run (req/244 H-02)",
            layout.config_path().display()
        )),
        _ => remedy,
    };
    let remedy = match why {
        NoEngine::JournalAbsent => remedy,
        // 🔴 **R36 / `req/476` H-01** — the road that wrote before it failed does not get the word
        // `refused`. R12 wrote one sentence for all three stages because all three were true of a
        // run that had changed nothing; audit 35 found the fourth shape — `recover` walking, an
        // adapter applying a delta, and a step after it raising — and it was being handed the
        // sentence for "nothing happened". The two are split here by the **measurement**
        // (`applied`), never by the stage name.
        // 🔴 **R37 / `req/496` M-01** — the **fifth** shape, and it is placed before the
        // `applied`-only arm rather than after it because the two are disjoint and the reader of
        // this `match` should meet the narrower one first. A row here is `Committed` on the disk:
        // R36's arm below tells an operator their objects "have been changed and no terminal record
        // says so" and sends them to close a row, and audit 36 carried that instruction out on a
        // row that was already closed — `terminal: 2, resumed: 0`, and nothing to do. What is
        // actually left undone is the head.
        NoEngine::Refused {
            stage,
            detail,
            applied,
            recorded,
            finished,
        } if applied.is_empty() && !recorded.is_empty() => Some(format!(
            "this run **wrote to your substrate and recorded it**, and then could not record a \
             head: 43 §7's recovery applied the delta of {} transformation(s) — {} — wrote 43 \
             §7-2's terminal `Committed` record for each, and `{stage}` then raised ({detail}) on \
             the last write of the sequence. Those row(s) are **closed**; nothing here should be \
             read as asking you to close them, and running a write verb again will report nothing \
             left to resume. What is undone is the signed head: it still describes the tree as it \
             was before those leaves (DR-43-11), so fix what the sentence in brackets names and run \
             a write verb again **so that a head is recorded over the tree that now holds them**. \
             What was not checked is what the road that succeeds does not check either — whether \
             anything had written to those objects between the crash and now — so if a third party \
             had, this run has written over it. {} row(s) had already been closed by this same \
             recovery before it raised. Everything above was measured without the engine — the \
             ledger's leaves, the commit receipts and the recorded head are read out of their own \
             files (`req/242` H-02, `req/476` H-01, `req/496` M-01)",
            recorded.len(),
            recorded.join(" "),
            finished
        )),
        NoEngine::Refused {
            stage,
            detail,
            applied,
            finished,
            ..
        } if !applied.is_empty() => Some(format!(
            "this run **wrote to your substrate** and could not record it: 43 §7's recovery \
             applied the delta of {} transformation(s) — {} — and then `{stage}` raised ({detail}). \
             This is not a refusal and nothing here should be read as one: those object(s) have \
             been changed and no terminal record says so. What was not checked is what the road \
             that succeeds does not check either — whether anything had written to those objects \
             between the crash and now — so if a third party had, this run has written over it. \
             {} row(s) had already been finished by this same recovery before it raised. The rows \
             stay resumable: fix what the sentence in brackets names and run a write verb again, \
             and 43 §7-3b's window closes them. Everything above was measured without the engine — \
             the ledger's leaves, the commit receipts and the recorded head are read out of their \
             own files (`req/242` H-02, `req/476` H-01)",
            applied.len(),
            applied.join(" "),
            finished
        )),
        NoEngine::Refused { stage, detail, .. } => Some(format!(
            "this project's log is where it should be and gx could not open it: the engine \
             refused at `{stage}` ({detail}). Nothing was applied on this road: `applied_before_\
             failure` under `engine_open_failed` is empty, and it is the measurement rather than \
             the stage name that says so. Everything above was measured without it — the \
             ledger's leaves, the commit receipts and the recorded head are read out of their own \
             files — and `meta_repaired` names whatever this run put back before it tried. What \
             this usually is: a permission an operator or a backup tool changed under `.gx/`, or a \
             restore that put a plain file where `.gx/ledger/journal.blobs/` is a directory (or the \
             other way round). Fix the path the sentence names and run this verb again; nothing in \
             `.gx/` was truncated (`req/242` H-02)"
        )),
    };
    // 🔴 **R14 / `req/246` M-04** — and the fact that keeps every writer out says so here too.
    // 🔴 **R15 / `req/259` M-01** — every blocked row's sentence, not the first one's.
    let remedy = match (&remedy, blocked_why(&repair_dir_blocked)) {
        (Some(text), Some(why)) => Some(format!("{text} {why}")),
        (None, Some(why)) => Some(why),
        _ => remedy,
    };
    let mut report = serde_json::json!({
        "project": layout.root().display().to_string(),
        "repaired": false,
        "mode": if yes { "yes" } else { "report" },
        "caught_up_records": serde_json::Value::Null,
        "recovery": serde_json::Value::Null,
        "recover": serde_json::Value::Null,
        // 🔴 `null` and not `true`. These two keys carry a comparison between the journal and
        // the ledger, and one of the two files is gone: there is no answer, and R4's `true`
        // was the wrong one.
        "ledger_agrees_before": serde_json::Value::Null,
        "ledger_agrees_after": serde_json::Value::Null,
        "journal_intact": serde_json::Value::Null,
        "journal_format": serde_json::Value::Null,
        "journal_format_declared": layout
            .declared_journal_format()
            .ok()
            .flatten()
            .map(gx_engine::JournalFormat::kind),
        "downgraded": serde_json::Value::Null,
        "journal_chain_break_at": serde_json::Value::Null,
        "rolled_back": serde_json::Value::Null,
        // 🔴 **R43 / `req/578` §3, ruling `req/38` §350 item 2** — the display half of the split
        // made where `head` is read. `witnessed` is computed from the other half and is unmoved.
        "head_recorded": head_recorded,
        "head_authenticity": serde_json::Value::Null,
        "head_invalid": serde_json::Value::Null,
        "accepted_rollback": serde_json::Value::Null,
        // 🔴 **R41 / `req/561` §11, audit 40 F-1 (`req/563` §2, ruling `req/38` §333)** — the fold
        // R40 removed from `journal_absent` above stood here, two keys below R40's own fix, in the
        // same function: `.is_file()` and `.exists()` both fold every `stat` failure into `false`,
        // so under an unreadable `.gx/ledger/` this report called a 348-byte ledger absent — the
        // exact sentence `run_the_repair`'s R40 comment names ("a diagnosis that folds 'I could
        // not look' into 'it is not there'"). These two keys are bools a monitor branches on, and
        // a field's contract stands alone (`req/38` §156 ruling 2(a)): the honest sibling
        // `engine_open_failed` beside them shrinks the harm and does not un-tell the lie. So each
        // asks [`crate::layout::presence_of`]'s three-way question: `Absent` and a successful
        // `stat` keep today's answers, and `Undetermined` answers `null` — the key stays, and
        // neither `true` nor `false` is claimed about a path this process could not look at
        // (R11's rule for this whole report: measured or `null`, never a constant).
        //
        // 🔴 **R45 / `req/621` M-1, ruling `req/38` §394** — the `Ok` side still folded, after the
        // comment above it named the fold and closed the `Err` side. `found.is_file()` answered
        // `false` about a declared path holding a dangling symbolic link — `attach.rs::present`'s
        // rule (`repair.rs:1810`, R43 S-7) and this key's own sibling four lines down
        // (`verdict_chain_present`) already say a symbolic link where a declared path belongs is
        // something that is there, whatever it points at. Unified to the same arm, same spelling
        // (`req/38` §394 ruling: statement 1 — shape stays out of this key; a separate key would be
        // a spec 44 §2.3 addition and was ruled YAGNI). The meaning this key carries moves with the
        // fix, from "a regular file is at this path" to "something is at this path" — `docs/LIMITS.md`
        // says so where the twenty-two-site census names `repair.rs` ×1.
        "ledger_present": presence_of_as_json(&layout.ledger_path()),
        "against": serde_json::Value::Null,
        "verdict_chain_present": presence_of_as_json(&layout.ledger_path().with_extension("verdicts")),
        // 🔴 **R13 / `req/244` H-02** — measured on **both** roads, because both are now
        // inside `repair_and_report` and both are behind `ProcessLock::open`.
        //
        // R12's value was a `matches!` on the reason, which was true while the journal-absent
        // branch returned in front of the lock. It does not any more: the branch moved below
        // the lock and the key so that `--yes` could repair `.gx/config.toml` there
        // (`req/244` H-02), so a stated `false` would now be a key that says the opposite of
        // what happened. What this costs is one honest line in the ledger of consequences: a
        // report over a journal-less project takes `.gx/LOCK` where it used to take nothing.
        // `.gx/LOCK` is `Nature::Transient` (DR-43-5 (2)), a repair that cannot make one
        // reports instead of raising (`req/242` M-03), and the exclusion is what makes the
        // count beside it a read no concurrent writer is halfway through.
        "lock_held": lock_held,
        "journal_commits": serde_json::Value::Null,
        "ledger_leaves": leaves,
        // 🔴 **R13 / `req/244` H-03** — `null` and not `0`: the subtraction has no answer
        // without the journal, and R11's rule for this whole report is "measured or `null`,
        // never a constant". The key is here so that the set a monitor branches on is one set.
        "journal_behind_by": serde_json::Value::Null,
        "journal_rows": serde_json::Value::Null,
        "files_agree": serde_json::Value::Null,
        "commit_receipts": receipts.commit_count(),
        "receipts_missing": missing_receipts.as_ref().map(Vec::len),
        "receipts_missing_ids": missing_receipts,
        "reissued": serde_json::Value::Null,
        "escrow_bodies_missing": serde_json::Value::Null,
        "escrow_bodies_missing_ids": serde_json::Value::Null,
        "damaged_bodies": serde_json::Value::Null,
        "damaged_body_names": serde_json::Value::Null,
        // The two `.tmp` directories could be walked here and the blob store's residue could
        // not (it is the engine's). A key that means "all three directories" everywhere else
        // must not quietly mean "two of them" here.
        "staging_files": serde_json::Value::Null,
        "staging_files_swept": serde_json::Value::Null,
        "declaration_readable": declaration_readable,
        "declaration_absent": declaration_absent,
        "config_absent": config_absent_before,
        "meta_repaired": meta_repaired,
        "meta_repair_refused": match (why, &meta_repair_refused) {
            // 🔴 **R11 / `req/240` H-02** — `--yes` writes nothing on a project with no
            // journal, and the key says why rather than being `null`.
            (NoEngine::JournalAbsent, _) if yes => serde_json::Value::String(format!(
                "nothing was written: `{}` is not there, and what `--yes` writes into \
                 `.gx/VERSION` is the framing read off that file's first eight bytes. gx does \
                 not guess it (req/240 H-02)",
                layout.journal_path().display()
            )),
            (_, Some(why)) => serde_json::Value::String(why.clone()),
            _ => serde_json::Value::Null,
        },
        "kept_aside": layout.kept_aside(),
        "gitignore_absent": layout.gitignore_absent(),
        // 🔴 **R11 / `req/240` H-02** — the fact this whole branch is about, as one key.
        "journal_absent": matches!(why, NoEngine::JournalAbsent),
        "head_behind_by": head_tree_size.map(|size| size.saturating_sub(leaves.unwrap_or(0))),
        "journal_intact_basis": serde_json::Value::Null,
        "remedy": remedy,
        // 🔴 **R12 / `req/242` H-02** — which of the engine's three steps refused, and
        // what it said. `null` on the journal-absent road: there was nothing to open.
        // 🔴 **R36 / `req/476` H-01** — two keys added **inside** this object rather than beside
        // it, and the position is not tidiness: `model_a_probes`'
        // `a_project_that_lost_its_journal_is_measured_and_not_called_healthy` compares the
        // top-level key list of this report against the full report's as an **ordered** list, and
        // a fiftieth top-level key here and not there would break the set a monitor branches on.
        // `applied_before_failure` is the answer to "did this run change my world", which
        // `stage: "recover"` was being read as answering and was not.
        "engine_open_failed": match why {
            NoEngine::JournalAbsent => serde_json::Value::Null,
            NoEngine::Refused { stage, detail, applied, recorded, finished } => serde_json::json!({
                "stage": stage,
                "reason": detail,
                "applied_before_failure": applied,
                // 🔴 **R37 / `req/496` M-01** — nested inside this object rather than beside it,
                // for the reason the two keys above are: `model_a_probes` compares the two
                // reports' **top-level** key lists as an ordered list.
                "recorded_before_failure": recorded,
                "finished_before_failure": finished,
            }),
        },
        // 🔴 **R13 / `req/244` H-01** — the same two keys as the full report, in the same
        // place, so the set a monitor branches on is one set (R11's forty-seven, R12's
        // forty-eight, now fifty).
        //
        // The position is not tidiness: `model_a_probes`'
        // `a_project_that_lost_its_journal_is_measured_and_not_called_healthy` compares the two
        // reports' keys as an **ordered** list, which is what catches a key that drifted into
        // one report and not the other. It caught these two when they were written four lines
        // higher.
        "previous_repair": previous_repair(layout).map(|p| p.report),
        // 🔴 **R14 / `req/246` M-01** — filled below, on this road too.
        //
        // R13 left this `null` and wrote the reason: "a report this run could not compose in
        // full is not one to hand the next `gx repair` as *the* record". `req/246` M-01
        // measured where that stops being true — a `--yes` that wrote `.gx/config.toml` back,
        // 139 bytes, on the very road R13 built for it, and filed nothing; the next
        // `gx repair` said `previous_repair: null`, and the `OUTPUT_FAILED` object on stderr
        // told that same run to go and read a file nobody had made. This report **is**
        // composed in full: every key the healthy report has, `null` where the engine would
        // have answered, which is R11's rule for this whole branch (measured or `null`, never
        // a constant). A record of it is a record.
        "repair_record": serde_json::Value::Null,
        // 🔴 **R14 / `req/246` M-04** — the fifty-first key, in the same position as on the
        // healthy report.
        "repair_dir_blocked": repair_dir_blocked,
    });
    if permitted_to_write {
        let filed = file_repair_record(layout, &report, previous_bytes);
        if let Some(map) = report.as_object_mut() {
            map.insert("repair_record".to_string(), filed);
        }
    }
    Outcome::refused(report, code)
}

/// 🔴 **R11 / `req/240` H-01** — the two `Nature::Meta` files, put back under the lock and after
/// the key, with what was done to each named.
///
/// One function so that the caller can hold the failure as a **value**: on a read-only `.gx/` this
/// used to raise through `?` from the top of `run_the_repair`, which took the report down with it
/// (audit 10 M-03) after the write had already happened (`req/240` H-01).
fn repair_meta(
    layout: &Layout,
    lock: Option<&gx_engine::store::OwnedLock>,
    key: Option<&gx_witness::KeyPair>,
    scope: MetaScope,
) -> (Vec<serde_json::Value>, Option<String>) {
    // 🔴 **R12 / `req/242` H-01** — the write goes through the one type that may write.
    //
    // `DeclarationWriter::for_repair` asks for the lock and the key **by reference**, so `req/240`
    // H-01's ordering ("the write happens after both, not before") is the signature rather than the
    // order of two statements. The two `Option`s cannot both be `Some` unless the caller took them,
    // and the caller only calls this when `writing` is true, which is the same condition.
    let (Some(lock), Some(key)) = (lock, key) else {
        return (
            Vec::new(),
            Some(
                "a repair reached the writer with no lock or no key. Both are taken before this \
                 point (req/240 H-01) and this arm is unreachable; it is here so that a later \
                 hand cannot make it reachable without a compile error"
                    .to_string(),
            ),
        );
    };
    let writer = crate::declaration::DeclarationWriter::for_repair(layout.root(), lock, key);
    let mut meta_repaired = Vec::new();
    // 🔴 **R13 / `req/244` H-02** — the declaration is repaired only where its bytes are a fact.
    //
    // What `--yes` writes into `.gx/VERSION` is the layout number and the framing **sniffed off the
    // journal's first eight bytes**; a journal that is not there declares nothing to sniff, and a
    // declaration composed anyway would be gx guessing what this project is and then signing heads
    // beside the guess. That is R4's argument and it is why the two files part company here rather
    // than travelling together as they did before R13.
    if scope == MetaScope::Both {
        let sniffed = crate::session::sniff_journal_format(&layout.journal_path());
        let declaration = match writer.repair_declaration(sniffed) {
            Ok(outcome) => outcome,
            Err(why) => return (meta_repaired, Some(why.to_string())),
        };
        if let Some(word) = declaration.as_str() {
            meta_repaired.push(serde_json::json!({
                "file": ".gx/VERSION",
                "what": word,
                "kept": match &declaration {
                    MetaRepair::Rewritten { kept } =>
                        serde_json::Value::String(kept.display().to_string()),
                    _ => serde_json::Value::Null,
                },
            }));
        }
    }
    match writer.repair_config() {
        Ok(config) => {
            if let Some(word) = config.as_str() {
                meta_repaired.push(serde_json::json!({
                    "file": ".gx/config.toml",
                    "what": word,
                    "kept": serde_json::Value::Null,
                }));
            }
        }
        Err(why) => return (meta_repaired, Some(why.to_string())),
    }
    (meta_repaired, None)
}

/// 🔴 **R13 / `req/244` H-01** — where `gx repair --yes` files the report it printed.
///
/// `.gx/repair/last.json`, declared in req/56 §2 and in [`crate::layout::GX_PATHS`] as R13's row.
///
/// # 🔴 **R15 / `req/259` M-01** — and it is the only name this module spells out
///
/// R14 added a second constant here, `REPAIR_DIR = "repair"`, and made it the **subject** of the
/// verb that clears a blocked directory. That is the finding: `Layout::create` refused all seven
/// declared directories and `repair` only ever looked at one of them, so six projects had a
/// refusal and no exit while gx's own remedy told the operator a rename had happened. The subject
/// is [`crate::layout::declared_directories`] now — one reading of req/56 §2's table — and the
/// constant that named a place is gone with the reasoning it stood for. What is left here is a
/// **file path**, which is what this constant was always for.
pub(crate) const REPAIR_RECORD: &str = "repair/last.json";

/// 🔴 **R13 / `req/244` H-01** — read back what the **previous** `--yes` run wrote.
///
/// `None` for a project no repair has written in, for a record that will not parse, and for one
/// that cannot be read. All three are the same answer to the caller — "there is no earlier run to
/// show you" — and none of them is a reason to refuse the report this run is composing.
/// 🔴 **R14 / `req/246` M-03** — what was read, and how many bytes it was.
///
/// The size travels with the value because [`without_generations`] states it in the reference it
/// files, and a second `stat` would be a second answer to a question this run has already asked.
struct PreviousRepair {
    /// The object the previous `--yes` run filed.
    report: serde_json::Value,
    /// How many bytes were read to get it.
    bytes: usize,
}

fn previous_repair(layout: &Layout) -> Option<PreviousRepair> {
    let raw = std::fs::read(layout.join(REPAIR_RECORD)).ok()?;
    let bytes = raw.len();
    serde_json::from_slice(&raw)
        .ok()
        .map(|report| PreviousRepair { report, bytes })
}

/// 🔴 **R14 / `req/246` M-04** — is something that is not a directory sitting where `.gx/repair/`
/// belongs, and did this run move it out of the way.
///
/// # What the fourteenth audit measured
///
/// R13 added the row and `Layout::create`'s loop asks the operating system for every `Shape::Dir`
/// in it. So one byte at `.gx/repair` — which is what a backup, a `tar` extraction or a restore of
/// a *file* by that name leaves — refused **every writer** with `INTERNAL` "create …/.gx/repair:
/// File exists (os error 17)": `gx submit`, `gx log head`, `gx receipt list`, three runs each. And
/// the verb whose whole job is to say what is wrong with a project answered exit **0**,
/// `ledger_agrees_after: true`, `remedy: null` — the one trace was `repair_record.written: false`,
/// a key that moved neither the status nor the remedy. There was no way out of gx.
///
/// # What this answers, and why `--yes` moves rather than removes
///
/// `null` for a project where the path is a directory or is not there at all. An object naming the
/// path otherwise, which the caller turns into exit **1** and a remedy — because "this project can
/// be written to" is exactly what 44 §1.2 gives `gx repair`'s 0 to, and it cannot.
///
/// A `--yes` that holds the lock and a key renames it to `.gx/repair.pre-repair.<n>` and makes the
/// directory. **Nothing is deleted**: those bytes are somebody's, gx did not write them, and
/// DR-43-7 (1)'s rule against destroying evidence is the standing one — `DeclarationWriter::aside`
/// takes the same shape for `.gx/VERSION`, and this is that shape applied to the row R13 added.
/// A path that is a **symlink** is moved too, and `symlink_metadata` is why: `Path::exists`
/// follows, so a dangling link reads as "not there" and then fails the `create_dir_all` anyway.
/// # 🔴 **R15 / `req/259` M-01** — and the subject is the **declared table**, not one name
///
/// R14 generalised the refusal and left the exit at a place. `Layout::create` pre-scans every
/// `Shape::Dir` row, so all seven refuse `LAYOUT_BLOCKED`; this function read `REPAIR_DIR` and
/// nothing else, so six of them had no way out. The fifteenth audit measured all seven, three runs
/// each: `.gx/evidence`, `.gx/index`, `.gx/drafts` and `.gx/receipts` blocked `gx submit` for ever
/// while `gx repair` answered exit **0**, `remedy: null`, `repair_dir_blocked: null` — the verb
/// whose job is to say what is wrong called the project healthy — and `--yes` moved nothing.
/// `.gx/checkpoints` refused with no exit; `.gx/ledger` came out as `HISTORY_LOST`. And the remedy
/// gx handed the operator said, verbatim and for every one of them, that `gx repair --yes` renames
/// the path to `.gx/<name>.pre-repair.<n>` and names the copy under `kept_aside` — three clauses
/// that were false for six of the seven. `req/227` M-04 is the standing rule that a remedy naming
/// the wrong file is worse than none, and R14's own new refusal had broken it.
///
/// So the answer is a **list**: `null` when no declared directory is blocked, and one object per
/// blocked row otherwise, in `GX_PATHS` order. A row is added to req/56 §2 by adding it to that
/// table, and the exit follows it there.
fn repair_dir_state(layout: &Layout, may_write: bool) -> serde_json::Value {
    let mut blocked: Vec<serde_json::Value> = Vec::new();
    for rel in crate::layout::declared_directories() {
        if let Some(state) = one_dir_state(layout, rel, may_write) {
            blocked.push(state);
        }
    }
    if blocked.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(blocked)
    }
}

/// 🔴 **R15 / `req/259` M-01** — every sentence [`repair_dir_state`] produced, in one string.
///
/// `None` when nothing is blocked. A project can have more than one declared directory occupied at
/// once (a restore that put files where two of them belong), and a remedy that named only the first
/// would send an operator back for a second round — which is `req/227` M-04's rule read forwards.
fn blocked_why(blocked: &serde_json::Value) -> Option<String> {
    let rows = blocked.as_array()?;
    let joined: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.get("why").and_then(serde_json::Value::as_str))
        .collect();
    if joined.is_empty() {
        None
    } else {
        Some(joined.join(" "))
    }
}

/// 🔴 **R15 / `req/259` M-01** — is any declared directory **still** not a directory.
///
/// One that is left is one the next writer is still refused by, and 44 §1.2 gives this verb's 0 to
/// "this project can be written to".
fn still_blocked(blocked: &serde_json::Value) -> bool {
    blocked.as_array().is_some_and(|rows| {
        rows.iter()
            .any(|row| row.get("cleared") == Some(&serde_json::Value::Bool(false)))
    })
}

/// One row of [`crate::layout::declared_directories`], and what this run could do about it.
///
/// `None` when the path is a directory or is not there at all — `Layout::create` makes it on the
/// next writer's way in, which is the ordinary case.
fn one_dir_state(layout: &Layout, rel: &'static str, may_write: bool) -> Option<serde_json::Value> {
    let path = layout.join(rel);
    // Absent, or unreadable in a way that is about `.gx/` itself rather than about this row.
    let found = std::fs::symlink_metadata(&path).ok()?;
    if found.is_dir() {
        return None;
    }
    let mut kept = serde_json::Value::Null;
    let mut cleared = false;
    let mut why = format!(
        "`.gx/{rel}` is declared as a directory (req/56 §2) and this path is not one. Every verb \
         that writes opens `.gx/` by asking for each declared directory, so this project refuses \
         `gx submit`, `gx log` and `gx receipt` until the path is a directory again. `gx repair \
         --yes` moves it aside — it is not gx's file and gx does not remove it — or move it \
         yourself and run this verb again (req/246 M-04, req/259 M-01)"
    );
    if may_write {
        match aside_of(layout, rel) {
            Ok(candidate) => match std::fs::create_dir_all(&path) {
                Ok(()) => {
                    cleared = true;
                    kept = serde_json::Value::String(format!(
                        ".gx/{}",
                        candidate
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().to_string())
                    ));
                    why = format!(
                        "`.gx/{rel}` was a path that is not a directory, so no verb that writes \
                         could open this project. This run moved those bytes to `{}` and made the \
                         directory; nothing was removed. The writer verbs work again (req/246 \
                         M-04, req/259 M-01)",
                        kept.as_str().unwrap_or_default()
                    );
                }
                Err(e) => {
                    // 🔴 **R16 / `req/262` L-02** — the copy is named here too.
                    //
                    // The audit read this arm and found `kept` left at `Null` while `why` named
                    // the path the bytes had already been moved to, so a report could say
                    // `cleared: false, kept: null` about a project whose bytes **had** moved. The
                    // rename is the half that succeeded; a reader deciding whether anything is on
                    // the disk under a new name asks `kept`, and it has to be able to.
                    kept = serde_json::Value::String(format!(
                        ".gx/{}",
                        candidate
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().to_string())
                    ));
                    why = format!(
                        "`.gx/{rel}` was moved to `{}` and the directory could not be made in its \
                         place ({e}). The bytes are under `kept` and nothing was removed. This \
                         project still refuses every verb that writes (req/246 M-04, req/262 L-02)",
                        candidate.display()
                    );
                }
            },
            Err(e) => {
                why = format!("{why}. This run tried and could not move it ({e})");
            }
        }
    }
    Some(serde_json::json!({
        "path": format!(".gx/{rel}"),
        "cleared": cleared,
        "kept": kept,
        "why": why,
    }))
}

/// 🔴 **R14 / `req/246` M-04** — a free `<name>.pre-repair.<n>` beside `.gx/`, and the rename to it.
///
/// The rule and the limit are `DeclarationWriter::aside`'s, deliberately: the copies are evidence,
/// gx never removes one to make room, and `Layout::kept_aside` counts exactly the names this shape
/// produces. What is not shared is the writer — `aside` is a method on the one type that may write
/// a `Nature::Meta` file, and `.gx/repair` is `Nature::Source`.
///
/// # Errors
/// [`Error::Usage`] when `PRE_REPAIR_LIMIT` copies already stand beside it, and [`Error::Io`] from
/// the rename.
fn aside_of(layout: &Layout, name: &str) -> Result<std::path::PathBuf> {
    let path = layout.join(name);
    for n in 0u32..crate::layout::PRE_REPAIR_LIMIT {
        let candidate = layout.join(&format!("{name}.pre-repair.{n}"));
        if std::fs::symlink_metadata(&candidate).is_err() {
            std::fs::rename(&path, &candidate).map_err(crate::io("rename", &path))?;
            return Ok(candidate);
        }
    }
    Err(Error::Usage {
        detail: format!(
            "`.gx/{name}` already has {limit} `.pre-repair.` copies beside it, and gx does not \
             remove one to make room — they are bytes gx did not write. Move them somewhere \
             outside `.gx/` and run this verb again (req/246 M-04)",
            limit = crate::layout::PRE_REPAIR_LIMIT
        ),
    })
}

/// 🔴 **R14 / `req/246` M-03** — one generation, never a chain.
///
/// # What the fourteenth audit measured
///
/// R13 filed "the same object it printed", and that object carries `previous_repair`: the whole of
/// the report the run before it filed. So the file held generation n − 1, which held n − 2, which
/// held n − 3. `gx repair --yes` on one healthy project, no adversary, nothing to repair:
/// 1 run 1,718 B, 10 runs 23,444 B, 40 runs 177,764 B, 100 runs 864,404 B, 126 runs **1,318,468 B**
/// — and at **127** `serde_json`'s recursion limit refused the read, `previous_repair(layout)`'s
/// `.ok()?` became `None`, and the file was rewritten from an empty history. The record erased
/// itself, and "no repair has run here" and "126 of them have" became the same answer. The printed
/// report grew with it: 176,584 B at run 126, which is past a pipe's 64 KiB and straight into
/// `req/246` M-03's second half — a `| head` that took 70,000 bytes of a broken JSON object.
///
/// # The rule
///
/// **A durable record holds no generations.** What is filed is this run's report with its
/// `previous_repair` replaced by a reference: the file it came from, its size, and when this run
/// read it. The printed report is unchanged — it still carries the previous report in full, which
/// is R13's guarantee that a run whose stdout died is readable from the next command — and it is
/// now bounded, because the object it embeds no longer embeds another one.
///
/// # Why the reference carries no digest
///
/// `req/246`'s repair sketch asked for `{digest, path, taken_at}`. **`gx-cli` may not mint one**:
/// its manifest states the rule in as many words — "gx-canon is absent and may not be added
/// (41 §6 gives the canonical encode one door), a CLI that could mint a `Cid` could name a
/// transformation the engine never saw" — and this crate has no other hash of bytes on its
/// dependency list. Adding one for a copy of a report would buy a field by breaking Rule 1, so the
/// reference names what this crate can state without minting anything: the path, the size in bytes
/// of what was read there, and the moment it was read. Written down rather than quietly dropped.
fn without_generations(
    report: &serde_json::Value,
    previous_bytes: Option<usize>,
) -> serde_json::Value {
    let mut filed = report.clone();
    if let Some(map) = filed.as_object_mut() {
        map.insert(
            "previous_repair".to_string(),
            match previous_bytes {
                None => serde_json::Value::Null,
                Some(bytes) => serde_json::json!({
                    "path": format!(".gx/{REPAIR_RECORD}"),
                    "bytes": bytes,
                    "taken_at": crate::clock::now().0,
                    "kept": false,
                    "why": "the report this run printed carried the previous run's report in full; \
                            the file keeps a reference instead, because a record that holds its own \
                            history holds every one of them and stops being readable at 127 \
                            (req/246 M-03). This file is the only copy the project keeps",
                }),
            },
        );
    }
    filed
}

/// 🔴 **R13 / `req/244` H-01** — file this run's report beside the project, and say whether it
/// landed.
///
/// # Why the disk and not only stdout
///
/// `Outcome::emit` makes the delivery a value, so a `gx repair --yes` whose stdout has gone now
/// exits **1** with a problem object instead of **101** with a Rust panic string. That closes the
/// lie; it does not close the loss. What `req/244` H-01 measured after the panic is the part that
/// costs an operator something: the run had written `.gx/VERSION`, and **the next `gx repair` said
/// `meta_repaired: []`, `meta_repair_refused: null`, `head_authenticity: verified`** — the fact
/// that gx had written a file was, at that point, nowhere at all. A report that only ever exists on
/// a stream is a report one closed pipe removes from the world.
///
/// # Why a failure here is a key and not a refusal
///
/// Raising would build the exact shape this lane is closing: a run that wrote `.gx/VERSION` and
/// then left without reporting, because a *second* write failed. The answer goes in the report
/// (`repair_record`), where the audit's rule for every other write on this road already puts it.
///
/// # Model A / Model B
///
/// Writing this file is Model B — gx putting bytes in a project — and it is declared as such
/// (req/56 §2's R13 row, 43 §7.9 (b)). Its **absence** is Model A: nothing infers a repair from a
/// missing record, `previous_repair` is `null`, and the file is a witness of nothing else. It is
/// deliberately outside `Layout::logged` and `Layout::established`, so a directory a repair has run
/// in does not start looking like a project that has been committed to.
/// 🔴 **R14 / `req/246` M-03 + L-05** — and it is filed **without generations**, through a
/// temporary name.
///
/// `previous_bytes` is the size of the record this run read on its way in, and it is what the
/// reference [`without_generations`] leaves in the file states. The write goes to
/// `last.json.tmp` and is renamed, so a reader that arrives mid-write sees the old file or the new
/// one and never half of either — `req/246` L-05, which measured the exposure rather than the tear
/// (the writers are all inside `.gx/LOCK`; the **report** mode's read is not).
fn file_repair_record(
    layout: &Layout,
    report: &serde_json::Value,
    previous_bytes: Option<usize>,
) -> serde_json::Value {
    let path = layout.join(REPAIR_RECORD);
    let staging = path.with_extension("json.tmp");
    let filed = without_generations(report, previous_bytes);
    let outcome = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| serde_json::to_vec_pretty(&filed).map_err(std::io::Error::other))
        .and_then(|bytes| std::fs::write(&staging, bytes))
        .and_then(|()| std::fs::rename(&staging, &path));
    match outcome {
        Ok(()) => serde_json::json!({
            "written": true,
            "path": format!(".gx/{REPAIR_RECORD}"),
            "why": serde_json::Value::Null,
        }),
        Err(why) => serde_json::json!({
            "written": false,
            "path": format!(".gx/{REPAIR_RECORD}"),
            "why": format!(
                "this run's report could not be filed beside the project ({why}). What is above is \
                 still what happened — the record is a copy for the next `gx repair` to read back, \
                 not the answer itself (req/244 H-01)"
            ),
        }),
    }
}

/// 🔴 **R13 / `req/244` H-02** — which `Nature::Meta` files one repair run may put back.
///
/// Two values rather than a `bool` because the difference is an argument and not a switch: the
/// declaration's bytes carry the journal's framing and the settings' bytes carry nothing the journal
/// knows, so a project with no journal has one of the two files repairable and the other not. A
/// `bool` at this call site would read as "skip some of it" and the fact is narrower.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MetaScope {
    /// The ordinary road: `.gx/VERSION` from the framing on the journal, then `.gx/config.toml`.
    Both,
    /// 🔴 A project with no `.gx/ledger/journal`. `.gx/config.toml` only — its bytes are the
    /// shipped default and ask the journal nothing, which is what makes it writable here and what
    /// `req/244` H-02 measured nobody writing.
    SettingsOnly,
}

/// 🔴 **R9 / `req/236` M-04** — the `.tmp` files a directory is holding, with their project-relative
/// names.
///
/// `req/236` M-04 swept 33 mid-commit kills and found `*.commit.json.tmp` left behind in 5 of them
/// and `head.json.tmp` in 2, with `gx repair` saying nothing about either and `--yes` removing
/// neither. They cost nothing but disk; what they cost an operator is the belief that this verb
/// tells them everything it can see.
fn tmp_files_in(dir: &Path, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if name.ends_with(".tmp") {
            out.push(format!("{prefix}{name}"));
        }
    }
    out
}

/// 🔴 **R9 / `req/236` M-04** — remove them, and say which ones went.
///
/// The writer's door only. A `.tmp` here is a partial write nobody's name resolves to: it is not
/// evidence, and DR-43-7 (1)'s rule against removing evidence is about `<file>.torn.<n>-<m>`, which
/// this function does not match.
fn sweep_tmp_files(dir: &Path, prefix: &str) -> Vec<String> {
    let mut gone = Vec::new();
    for name in tmp_files_in(dir, prefix) {
        let bare = name.rsplit('/').next().unwrap_or(&name).to_string();
        // 🔴 **R11 / `req/240` L-01 (audit 10 L-01)** — only the names gx itself writes.
        if !gx_wrote(&bare) {
            continue;
        }
        if std::fs::remove_file(dir.join(&bare)).is_ok() {
            gone.push(name);
        }
    }
    gone
}

/// 🔴 **R11 / `req/240` L-01 (audit 10 L-01)** — is this `.tmp` name one gx makes.
///
/// The sweep matched `name.ends_with(".tmp")` and the report called every match "what a crash
/// leaves behind", with the cause stated rather than measured. The audit put
/// `.gx/receipts/operator-notes.tmp` in the directory and watched `gx repair --yes` remove it and
/// list it as swept. `.gx/` is gx's directory and an operator keeping a file there is doing
/// something unusual; removing it anyway is still gx deleting a file it did not write, on the one
/// verb whose standing is that it destroys no evidence (DR-43-7 (1)).
///
/// Three names, each from the writer that makes it: `head.json.tmp` (`gx_log::HeadStore::write`,
/// `path.with_extension("json.tmp")`), and `<id>.commit.json.tmp` / `<id>.verdict.json.tmp`
/// (`crate::receipt::ReceiptStore::put`, the same extension swap over `<id>.<kind>.json`).
/// Anything else in those two directories is **reported** — an operator has to be able to see it —
/// and left exactly where it lies.
fn gx_wrote(name: &str) -> bool {
    name == "head.json.tmp"
        || name.ends_with(".commit.json.tmp")
        || name.ends_with(".verdict.json.tmp")
}

/// 🔴 **R6 / DR-43-11** — the remedy for a project that has gone backwards, which is not the
/// remedy for two files that disagree.
///
/// It comes **first** among the remedies, because every other sentence in this module is a
/// hypothesis about a disagreement between the journal and the ledger, and a rolled-back project's
/// two files agree perfectly. Sending an operator to compare them would be `req/227` M-04's failure
/// again — "a remedy that names the wrong file is worse than none: it is a hypothesis with gx's
/// authority behind it".
fn rolled_back_remedy<E: gx_engine::EvidenceSource>(
    engine: &gx_engine::Engine<E>,
) -> Option<String> {
    let why = engine.rolled_back()?;
    // 🔴 **R8 / `req/234` H-02 + M-03** — the declaration is not the history, and the remedy for
    // one is not the remedy for the other.
    //
    // R7 gave both to `rolled_back` and therefore gave both this paragraph, whose advice is "take
    // a copy of `.gx/`, restore from a backup". For a `.gx/VERSION` whose *value* was changed that
    // is a destructive answer to a two-line text file, and `req/234` H-02 measured an operator
    // being sent down it by an editor's trailing newline. Since R8 the digest is over what the file
    // declares, so the whitespace cases no longer arrive here at all — and the ones that do get a
    // sentence that names the file, says what the correct contents are, and does not mention the
    // journal or the ledger.
    if engine.declaration_changed() {
        return Some(format!(
            "{why}. **The journal and the ledger are not what moved** — this report prints both \
             counts and `files_agree`, and if that is `true` then `.gx/VERSION` is the only \
             difference. Restoring `.gx/` from a backup is not the repair and may lose work. What \
             to fix: open `.gx/VERSION` and put back the declaration this project was written \
             under. gx writes it as two lines — the layout version on the first (`1`) and \
             `journal_format=chained` on the second — and `journal_format` is the value that \
             matters: `chained` for a project written by DR-43-9's binary or later, `legacy` for \
             one written before it. Since R8 the digest is taken over what those lines *declare*, \
             so line endings, trailing spaces and a trailing newline are not what brought you here \
             (req/234 H-02, req/232 M-02)"
        ));
    }
    Some(format!(
        "{why}. The two files agree with **each other** — that is what makes this different from \
         every other refusal this verb reports — so comparing them will find nothing. `gx repair \
         --yes` does not fix it and must not: the shorter history is internally perfect and gx \
         cannot invent the records that are gone. What to fix: take a copy of `.gx/` before \
         anything else, restore from a backup, and if you kept a checkpoint outside this machine \
         run `gx repair --against <FILE>` to see how far back it went (DR-43-11, req/229 H-01). \
         🔴 **And the way out, which R6 did not name (req/232 M-03).** If you conclude that the \
         recorded head is not this project's — a restore that brought somebody else's \
         `.gx/checkpoints/head.json`, a hand-edited file, a number nobody's tree ever reached — \
         then move that one file aside and open the project again: gx will report `head_recorded: \
         false`, which is the honest answer for a project that has made no statement about its \
         past. If instead the shorter tree really is the one you want to keep, say so with \
         evidence: `gx repair --yes --accept-rollback --against <FILE>` re-bases the floor and \
         records what it replaced (req/38 §171 ruling 2(c))"
    ))
}

/// 🔴 **R6 / DR-43-10** — the remedy when the refusal came from a document the operator kept
/// outside this machine.
///
/// First among all of them, because it is the only one whose evidence an attacker with write access
/// to the project could not have touched.
/// 🔴 **R7 / `req/232` M-04** — the key this project's own head was signed under, if it has one.
///
/// Read from the head document rather than from `.gx/config.toml` first, because the question
/// `--against` is answering is "did **this** log sign that checkpoint" and the log's own last
/// statement is the closest thing to an answer the project holds. The configured signing key is the
/// fallback for a project that has not recorded a head yet.
fn recorded_head_key(layout: &Layout) -> Option<String> {
    gx_log::HeadStore::at(layout.head_path(), crate::ledger::DEFAULT_ORIGIN)
        .read()
        .ok()
        .flatten()
        .map(|head| head.checkpoint.signature.keyid)
}

fn against_remedy(against: Option<&Path>, against_refused: bool) -> Option<String> {
    if against_refused {
        return Some(format!(
            "this project is behind the signed checkpoint in {}: that document attests a tree this \
             project no longer has, and it was signed by this project's own key. gx does not \
             rebuild the missing leaves — 42 §3.13's `Committed` record carries no receipt digest, \
             so a leaf built here would be invented (DR-43-8's second half is not implemented). \
             What to fix: keep the checkpoint and the commit receipts, restore `.gx/` from a backup, \
             and check every receipt against the checkpoint with `gx receipt verify --offline \
             --checkpoint <FILE>` — a receipt that verifies there and is `refuted` against this \
             project names a commit that was removed (req/229 §7-4, DR-43-10)",
            against.map_or_else(String::new, |p| p.display().to_string())
        ));
    }
    None
}

/// The key `recover` signs the receipts it issues with — `gx serve`'s resolution, one verb along.
///
/// `--signing-key`, then `.gx/config.toml`'s `engine_signing_keyid` (E-M6-7). A repair that picked
/// a key for itself would sign a commit receipt with a hand this project never named.
fn signing(layout: &Layout, flag: Option<&str>) -> Result<gx_witness::KeyPair> {
    let recorded = crate::serve::recorded_signing_keyid(layout)?;
    let id = flag
        .map(std::string::ToString::to_string)
        .or(recorded)
        .ok_or_else(|| Error::Usage {
            detail: "repairing this project may finish a commit that was interrupted, and 43 §7-3 \
                     issues a signed receipt when it does — so this verb needs the same key \
                     `gx serve` needs. Pass --signing-key <KEY_ID>, or record one in \
                     `.gx/config.toml` as `engine_signing_keyid = \"…\"`. Without --yes no key is \
                     needed and this verb only reports"
                .to_string(),
        })?;
    KeyStore::user_default()?.load(&id)
}
