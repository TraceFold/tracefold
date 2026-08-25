// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx replay` — 44 §1.2, **E-M5-2** and **M6-26 adopted (a)** (sem: SEM-gx-cli-068).
//!
//! # What replay is, after E-M5-2
//!
//! 44 §1.2: "deterministic replay using EngineJournal (42 §3.13) … re-inject `rng_seed` and
//! verify the same result reproduces. stdout: the diff between the replay result and the original
//! record (`{ "matches": bool, "diffs": [...] }`)" (sem: SEM-gx-cli-069).
//! **E-M5-2** narrowed it: replay "is a read-only operation that reconstructs only Σ" and calls no adapter. So
//! `gx_engine::reconstruct` is the whole of the replay — a pure function from records to Σ — and the
//! `rng_seed` is not re-injected but **read**, because it is in `DraftCreated`.
//!
//! # 🔴 What `matches` can honestly compare in a single-shot process
//!
//! A reconstruction compared against itself is a tautology, and `matches: true` from a tautology is
//! M6-26(c)'s "the shape that makes an absence look like a pass" (sem: SEM-gx-cli-070). What a single `gx` process actually holds is **two durable
//! artefacts written by different code**: the engine journal (42 §3.13) and the ledger
//! (`gx_log::LedgerStore`, 42 §3.11). Σ's `ledger` component is the journal's claim about the
//! ledger — `CommittedRow{transformation, ledger_seq}`, and its own type documentation says so:
//! "This is the journal's claim about the ledger, not the ledger's own root" (sem: SEM-gx-cli-071). Comparing the claim
//! with the log is a real question with a real answer.
//!
//! The other three components of Σ (`drafts`, `transformations`, `escrow`) have **no independent
//! witness** in v0.1: nothing else on disk holds them, so a comparison would be a reconstruction
//! against itself. `unchecked` in the output names them rather than letting `matches: true` imply
//! they were examined. That is the whole difference between this and (c).
//!
//! # M6-26 adopted (a): what `diffs` holds (sem: SEM-gx-cli-072)
//!
//! > `matches` answers by byte equality, and `diffs` is a list of "the first component name that
//! > disagreed, when there is a disagreement" (the 4 components: state table / ledger / escrow /
//! > draft) -- the 4 components are structure `Sigma` already has, so no new type is created (sem: SEM-gx-cli-073)
//!
//! So `diffs` is a list of component names and never a structural diff: E-M4-15 took `==` off
//! `Fingerprint` because 42 §3.5's comparison has three answers, and a structural differ over Σ
//! would need one. The component that can differ in this hand is `ledger`; the entry that differs
//! is named beside it, because "the ledger disagrees" (sem: SEM-gx-cli-074) without a name is not actionable.

use gx_engine::store::{EngineJournal, EngineJournalRecord};
use gx_engine::{reconstruct, Sigma};
use gx_log::LedgerStore;

use crate::exit::{Outcome, ERROR};
use crate::{layout::Layout, Error, Result};

/// Which records to replay.
pub enum Range {
    /// 44 §1.2's `gx replay <TRANSFORMATION_ID>`: the records naming one transformation.
    Transformation(gx_core::TransformationId),
    /// 44 §1.2's `--from <INDEX> --to <INDEX>`.
    ///
    /// 🔴 **Of the journal, not of the ledger.** 44 writes `<INDEX>` and names neither sequence;
    /// replay is defined on the journal (42 §3.13), so the journal's append order is the index that
    /// makes the sentence true. `to` is exclusive. Raised as **M6H2-8**, because the other reading —
    /// ledger index — is available to a reader of 44 alone and would silently replay a different
    /// set.
    Records { from: usize, to: usize },
    /// No argument: everything.
    All,
}

/// Open the journal a project's `.gx/` holds, **without creating one**.
///
/// `EngineJournal::open` creates the file when it is absent, which is right for an engine starting
/// up and wrong for a read-only command: a replay that left an empty journal behind would make
/// "this project has no journal" (sem: SEM-gx-cli-075) unobservable after the first run.
///
/// # Errors
/// [`Error::NotFound`] if there is no journal; [`Error::Engine`] if it cannot be opened or replayed.
///
/// 🔴 **R41 / `req/561`** — supplement: "there is no journal" is established by
/// [`crate::layout::presence_of`] answering `Absent`, and by nothing else. Any other `stat`
/// outcome — a shape that is not a regular file, or a `stat` this process may not make — falls
/// through to the read-only open below and wears [`Error::Engine`]'s existing words. This door
/// takes a `Layout` a caller already holds and does not re-ask `Layout::open`'s R40 questions, so
/// it answers the three-way question itself.
pub fn open(layout: &Layout) -> Result<EngineJournal> {
    let path = layout.journal_path();
    if crate::layout::presence_of(&path).is_absent() {
        return Err(Error::NotFound {
            what: "journal",
            id: path.display().to_string(),
        });
    }
    // 🔴 **DR-43-7 (`req/38` §153)** — read-only, and a torn tail is refused rather than removed.
    //
    // This verb is the one `gx serve`'s start-up refusal recommends ("`gx replay <ID>` names the
    // rows that differ"), and `req/215` H-03 measured it walking the writer's door and truncating
    // the very file it had been called to explain. It holds no `.gx/LOCK`, so the bytes at the end
    // may belong to a writer that is still writing them.
    //
    // The refusal is the diagnosis: how many bytes will not replay, how many records did, and that
    // the file is untouched. Repair belongs to whoever holds the lock.
    let journal = EngineJournal::open_read_only(&path)?;
    let torn = journal.recovery().torn_tail_bytes;
    if torn > 0 {
        return Err(Error::Malformed {
            what: "journal",
            path: path.display().to_string(),
            detail: format!(
                "{torn} byte(s) after the last whole record do not replay, so this file holds {} \
                 record(s) and an unreadable tail. This verb reads and does not repair, so **the \
                 file was not changed** (DR-43-7, req/215 H-03). A `gx` write verb or `gx serve` \
                 opens the journal as a writer: it copies the file to `<journal>.torn.<replayed>-<total>` and \
                 then removes the tail",
                journal.len(),
            ),
        });
    }
    Ok(journal)
}

/// The records the range selects, in journal order.
///
/// # Errors
/// [`Error::Usage`] if `from > to` or `to` is past the end. A range nobody can satisfy is a request
/// error rather than an empty answer: replaying zero records and reporting `matches: true` is the
/// vacuous pass this module's header refuses.
///
/// [`Error::NotFound`] (`req/312` L-04) for a transformation id this journal holds no record of --
/// the same argument one subject over, and the answer every other id-taking verb of this binary
/// already gives.
pub fn select<'a>(
    records: &'a [EngineJournalRecord],
    range: &Range,
) -> Result<Vec<&'a EngineJournalRecord>> {
    match range {
        Range::All => Ok(records.iter().collect()),
        Range::Transformation(tid) => {
            let selected: Vec<&EngineJournalRecord> = records
                .iter()
                .filter(|r| {
                    r.transformation().as_ref() == Some(tid)
                        || matches!(r, EngineJournalRecord::Superseded { by, .. } if by == tid)
                })
                .collect();
            // 🔴 **`req/312` L-04 (R23)** — a well-formed id this journal has never heard of is
            // *not found*, and until this line it was a **match**.
            //
            // The audit gave `gx replay` an id built by moving one character of a real one and got
            // `rc=0` with `{"matches": true, "records_replayed": 0, …}`. Every other verb that
            // takes an id answered `NOT_FOUND` / exit 6 to the same argument (eleven of them,
            // `req/312` §2(e)), so the answer was not merely vacuous — it disagreed with the rest
            // of the binary about what the id names. `records_replayed: 0` is printed beside it and
            // a reader can see it; a tool branching on `matches` cannot.
            //
            // This is the same refusal the `Records` arm one branch down already makes for a range
            // nobody can satisfy, and this module's own header calls that answer "the vacuous pass
            // M6-26(c) was refused for". The subject differs — a range is the caller's arithmetic,
            // an id is an object this project either holds or does not — so the word is
            // `NotFound` (44 §1.4's 6) rather than `Usage`.
            //
            // A transformation with a row in the journal always has records: `DraftCreated` is
            // written before anything else can be. So "zero records" and "no such transformation"
            // are one fact here, and the sentence says the second because that is the one an
            // operator can act on.
            if selected.is_empty() {
                return Err(Error::NotFound {
                    what: "transformation",
                    id: tid.0.to_text(),
                });
            }
            Ok(selected)
        }
        Range::Records { from, to } => {
            if from > to || *to > records.len() {
                return Err(Error::Usage {
                    detail: format!(
                        "--from {from} --to {to} is not a range of this journal's {} records \
                         (indices are of the journal, not the ledger — M6H2-8)",
                        records.len()
                    ),
                });
            }
            Ok(records[*from..*to].iter().collect())
        }
    }
}

/// 🔴 `gx replay` (44 §1.2). stdout: `{ "matches": bool, "diffs": [...] }`.
///
/// `dry_run` is 44's `[--dry-run]` and is accepted for the synopsis's sake; **replay writes
/// nothing** under E-M5-2, so the flag names a difference this command does not have. It is
/// reported in the output rather than silently ignored — a flag that changes nothing and says
/// nothing teaches an operator that it did something. Raised as **M6H2-9**.
///
/// # Errors
/// [`Error::Usage`] for a range this journal has not got, and [`Error::NotFound`] for a
/// transformation id it holds no record of (`req/312` L-04).
pub fn replay(
    journal: &EngineJournal,
    ledger: Option<&LedgerStore>,
    range: &Range,
    dry_run: bool,
) -> Result<Outcome> {
    let selected = select(journal.records(), range)?;
    let owned: Vec<EngineJournalRecord> = selected.into_iter().cloned().collect();
    let sigma: Sigma = reconstruct(&owned);

    let mut diffs: Vec<serde_json::Value> = Vec::new();
    let checked;
    match ledger {
        Some(store) => {
            checked = true;
            let log = store.log();
            for row in sigma.ledger() {
                match log
                    .entries()
                    .iter()
                    .find(|e| e.transformation == row.transformation)
                {
                    // The journal says this transformation committed at this sequence number; the
                    // ledger is where it actually landed. A disagreement here is the one failure a
                    // replay can find without a substrate, and it is exactly what 43 §7's restart
                    // path is at risk of producing.
                    Some(entry) if entry.index == row.ledger_seq => {}
                    Some(entry) => diffs.push(serde_json::json!({
                        "component": "ledger",
                        "transformation": row.transformation.0.to_text(),
                        "journal_ledger_seq": row.ledger_seq,
                        "ledger_index": entry.index,
                    })),
                    None => diffs.push(serde_json::json!({
                        "component": "ledger",
                        "transformation": row.transformation.0.to_text(),
                        "journal_ledger_seq": row.ledger_seq,
                        "ledger_index": serde_json::Value::Null,
                    })),
                }
            }
        }
        None => checked = false,
    }

    let matches = checked && diffs.is_empty();
    let json = serde_json::json!({
        "matches": matches,
        "diffs": diffs,
        // 🔴 The denominator, and the honesty of `matches`. Three of Σ's four components have no
        // second copy on disk, so a comparison of them would be a reconstruction against itself.
        "unchecked": ["drafts", "transformations", "escrow"],
        "records_replayed": owned.len(),
        "ledger_consulted": checked,
        "dry_run": dry_run,
    });
    // 44 §1.2: "exit: 0 = match, 1 = mismatch or unable to execute". A replay with no ledger to compare against is
    // "unable to execute" rather than "a match" — the first reading this hand wrote answered 0 when there was
    // nothing to check, which is `matches` meaning "nothing disagreed with me". (sem: SEM-gx-cli-076)
    Ok(if matches {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, ERROR)
    })
}
