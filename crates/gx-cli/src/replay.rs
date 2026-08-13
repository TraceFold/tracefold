//! `gx replay` — 44 §1.2, **E-M5-2** and **M6-26 採(a)**.
//!
//! # What replay is, after E-M5-2
//!
//! 44 §1.2: 「EngineJournal（42 §3.13）を用いた決定的リプレイ…`rng_seed`を再注入し同一結果が再現され
//! ることを検証する。stdout: リプレイ結果と元記録との差分（`{ "matches": bool, "diffs": [...] }`）」.
//! **E-M5-2** narrowed it: replay 「は Σ のみを再構成する read-only 操作」 and calls no adapter. So
//! `gx_engine::reconstruct` is the whole of the replay — a pure function from records to Σ — and the
//! `rng_seed` is not re-injected but **read**, because it is in `DraftCreated`.
//!
//! # 🔴 What `matches` can honestly compare in a single-shot process
//!
//! A reconstruction compared against itself is a tautology, and `matches: true` from a tautology is
//! M6-26(c)'s 「不在を pass に見せる形」. What a single `gx` process actually holds is **two durable
//! artefacts written by different code**: the engine journal (42 §3.13) and the ledger
//! (`gx_log::LedgerStore`, 42 §3.11). Σ's `ledger` component is the journal's claim about the
//! ledger — `CommittedRow{transformation, ledger_seq}`, and its own type documentation says so:
//! 「This is the journal's claim about the ledger, not the ledger's own root」. Comparing the claim
//! with the log is a real question with a real answer.
//!
//! The other three components of Σ (`drafts`, `transformations`, `escrow`) have **no independent
//! witness** in v0.1: nothing else on disk holds them, so a comparison would be a reconstruction
//! against itself. `unchecked` in the output names them rather than letting `matches: true` imply
//! they were examined. That is the whole difference between this and (c).
//!
//! # M6-26 採(a): what `diffs` holds
//!
//! > `matches` は byte 一致で答え、`diffs` は「不一致の時に最初に食い違った成分名」の列(状態表 /
//! > ledger / escrow / draft の 4 成分)にする——4 成分は `Sigma` が既に持つ構造なので新しい型を作らない
//!
//! So `diffs` is a list of component names and never a structural diff: E-M4-15 took `==` off
//! `Fingerprint` because 42 §3.5's comparison has three answers, and a structural differ over Σ
//! would need one. The component that can differ in this hand is `ledger`; the entry that differs
//! is named beside it, because 「the ledger disagrees」 without a name is not actionable.

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
/// 「this project has no journal」 unobservable after the first run.
///
/// # Errors
/// [`Error::NotFound`] if there is no journal; [`Error::Engine`] if it cannot be opened or replayed.
pub fn open(layout: &Layout) -> Result<EngineJournal> {
    let path = layout.journal_path();
    if !path.is_file() {
        return Err(Error::NotFound {
            what: "journal",
            id: path.display().to_string(),
        });
    }
    Ok(EngineJournal::open(&path)?)
}

/// The records the range selects, in journal order.
///
/// # Errors
/// [`Error::Usage`] if `from > to` or `to` is past the end. A range nobody can satisfy is a request
/// error rather than an empty answer: replaying zero records and reporting `matches: true` is the
/// vacuous pass this module's header refuses.
pub fn select<'a>(
    records: &'a [EngineJournalRecord],
    range: &Range,
) -> Result<Vec<&'a EngineJournalRecord>> {
    match range {
        Range::All => Ok(records.iter().collect()),
        Range::Transformation(tid) => Ok(records
            .iter()
            .filter(|r| {
                r.transformation().as_ref() == Some(tid)
                    || matches!(r, EngineJournalRecord::Superseded { by, .. } if by == tid)
            })
            .collect()),
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
/// [`Error::Usage`] for a range this journal has not got.
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
    // 44 §1.2: 「exit: 0=一致, 1=不一致または実行不能」. A replay with no ledger to compare against is
    // 「実行不能」 rather than 「一致」 — the first reading this hand wrote answered 0 when there was
    // nothing to check, which is `matches` meaning 「nothing disagreed with me」.
    Ok(if matches {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, ERROR)
    })
}
