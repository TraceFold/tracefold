// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx verdict-checkpoint issue|verify` — **FR-M04**'s surface, which 32 §D says did not exist.
//!
//! 32 §D's tail, verbatim: "**zero surfaces currently exist** -- `Engine::verdict_checkpoint` was
//! placed only as an API" (sem: SEM-gx-cli-077).
//! `req/119` §4 is the requirement definition for closing that, and `req/38` §71 ruling ⑤ fixed the
//! name: **`gx verdict-checkpoint`**, not `gx checkpoint`, because `gx log checkpoint` already
//! exists and is a different object — 42 §3.11's signed **tree head** rather than a count of
//! verdicts. Two things called "checkpoint" (sem: SEM-gx-cli-078) in one CLI is a trap the naming avoids rather than
//! documents.
//!
//! # What issuing buys, and what it does not
//!
//! The engine's own doc comment is the authority and is not softened here: a `VerdictReceipt` has
//! `inclusion_proof = None` (ASM-14), so an operator who never exports their refusals can show an
//! auditor a hundred-percent-Admit record. Publishing the **count** makes withholding them cost
//! something. It does not make the count true — ruling #3 (sem: SEM-gx-cli-079): a gate widened until nothing is refused
//! publishes `deny = 0` honestly; ruling #14: one key can sign two consistent chains for two
//! verifiers. Both limits are printed by `verify` rather than left in a specification.
//!
//! # 🔴 Issuing is manual (ruling ⑥; sem: SEM-gx-cli-080)
//!
//! `req/38` §71: "**checkpoint issuing = manual by default** (automatic issuing -- an in-serve
//! timer -- needs a design that goes through Rule 2's single point; reserved for v0.2)" (sem: SEM-gx-cli-081). So there is no `--checkpoint-every`, and `gx serve` says "verdict_checkpoints:
//! manual" in its start-up line rather than leaving an operator to infer it.

use std::path::{Path, PathBuf};

use gx_core::{Timestamp, VerdictKind, VerdictTally};
use gx_engine::EngineJournalRecord;
use gx_log::proof::{audit_verdict_chain, ChainBreak};
use gx_witness::{dsse, KeyPair, PublicKey};

use crate::exit::{Outcome, VERIFY_FAILED};
use crate::session::Session;
use crate::{Error, Result};

/// The log namespace a **verdict** chain is scoped to.
///
/// Not [`crate::ledger::DEFAULT_ORIGIN`]: 42 §3.11's `origin` is what stops one deployment's
/// statements from being read as another's, and a count of verdicts is not a tree head. The two
/// artefacts travel separately (different payload types, AC-VC-4), so they carry different
/// namespaces. `crates/gx-cli/tests/verdict_checkpoint_surface.rs` compares this spelling with
/// gx-api's, the way `ac_055.rs` compares the ledger origin.
pub const DEFAULT_VERDICT_ORIGIN: &str = "glovrex-verdicts/v1";

/// 🔴 `gx verdict-checkpoint issue` — close the window, sign the counts, append them.
///
/// # Errors
/// [`Error::Engine`] if the core cannot be signed or the chain cannot be appended,
/// [`Error::Malformed`] if the signed value has no JSON form, [`Error::Io`] for an `--out` that
/// cannot be written.
pub fn issue(
    session: &mut Session,
    key: &KeyPair,
    origin: &str,
    at: Timestamp,
    out: Option<&Path>,
) -> Result<Outcome> {
    let signed = session.engine().verdict_checkpoint(origin, at, key)?;
    let json = serde_json::to_value(&signed).map_err(|e| Error::Malformed {
        what: "verdict checkpoint",
        path: String::new(),
        detail: e.to_string(),
    })?;
    if let Some(path) = out {
        write_document(path, &json)?;
    }
    Ok(Outcome::ok(json))
}

/// The chain this deployment has published, as `GET /v1/verdict-checkpoints` serves it.
///
/// # Errors
/// [`Error::Malformed`] if the chain has no JSON form.
pub fn list(session: &Session) -> Result<Outcome> {
    render_chain(session.read().verdict_checkpoints())
}

/// 🔴 **DR-43-7 / `req/215` M-02 + H-03** — the same answer, from the file, with no lock and no
/// repair.
///
/// `gx verdict-checkpoint list` is a read. Before this it opened a whole `Session`, which takes
/// `.gx/LOCK`, runs the catch-up and the `ledger_agrees` gate, and opens every store through its
/// writer's door. `req/215` measured both halves of the cost: with a third party holding the lock
/// the verb answered **exit 1 `BUSY`** (while `gx log proof`, an equally read-only verb, answered
/// `0`), and on a project with a torn ledger it left the file at **0 bytes**.
///
/// The chain is one file. This opens that file read-only and reads it, which is all the verb ever
/// needed, and it neither excludes a writer nor repairs anything.
///
/// # Errors
/// [`Error::NotFound`] if the project has no verdict chain yet; [`Error::Log`] if it cannot be
/// opened or replayed; [`Error::Malformed`] for a torn tail (see `crate::ledger::open`'s reasoning).
///
/// 🔴 **R41 / `req/561`** — two supplements, the old sentence kept for the record. First: the
/// absent-chain case in fact answers the empty `Ok` below rather than `Error::NotFound` — the
/// chain is issued by hand (`gx verdict-checkpoint`), so "no chain yet" is a normal answer, and
/// the `NotFound` claim above predates that ruling. Second: "has no verdict chain yet" is
/// established by [`crate::layout::presence_of`] answering `Absent`, and by nothing else. Any
/// other `stat` outcome falls through to the read-only open below and wears its existing words —
/// before R41, a directory standing at the chain's path was answered as an empty chain, a mild
/// falsehood this door no longer tells.
pub fn list_from_file(layout: &crate::layout::Layout) -> Result<Outcome> {
    let path = layout.verdict_log_path();
    if crate::layout::presence_of(&path).is_absent() {
        return Ok(Outcome::ok(serde_json::json!({
            "items": serde_json::Value::Array(Vec::new()),
            "count": 0,
        })));
    }
    let store = gx_log::store::VerdictCheckpointStore::open_read_only(&path)?;
    let torn = store.recovery().torn_tail_bytes;
    if torn > 0 {
        return Err(Error::Malformed {
            what: "verdict checkpoint log",
            path: path.display().to_string(),
            detail: format!(
                "{torn} byte(s) after the last whole record do not replay, so this file holds {} \
                 checkpoint(s) and an unreadable tail. This verb reads and does not repair, so \
                 **the file was not changed** (DR-43-7, req/215 H-03/M-02)",
                store.checkpoints().len(),
            ),
        });
    }
    render_chain(store.checkpoints())
}

/// The one JSON body both roads answer with.
fn render_chain(chain: &[gx_core::VerdictCheckpoint]) -> Result<Outcome> {
    let json = serde_json::to_value(chain).map_err(|e| Error::Malformed {
        what: "verdict checkpoint chain",
        path: String::new(),
        detail: e.to_string(),
    })?;
    Ok(Outcome::ok(serde_json::json!({
        "items": json,
        "count": chain.len(),
    })))
}

/// What `verify` was asked to check.
pub struct VerifySpec {
    /// The checkpoint documents, in chain order. `-` reads one from stdin.
    pub files: Vec<String>,
    /// The public key the chain was signed with. Without it the signature check is **skipped** and
    /// says so, rather than passing.
    pub key: Option<PathBuf>,
    /// A signed ledger head (`gx log checkpoint`'s output) to bind the chain against. Without it
    /// the binding is checked against the size the chain itself claims, which is weaker and is
    /// reported as such.
    pub ledger_checkpoint: Option<PathBuf>,
    /// Recount the verdicts from this project's journal and compare (AC-VC-2's half).
    pub recount_from_journal: bool,
}

/// 🔴 `gx verdict-checkpoint verify` — the five judgements of AC-VC-1..5, through a surface.
///
/// exit **0** = valid, **7** = invalid (the number `gx receipt verify` already uses for "invalid") (sem: SEM-gx-cli-082),
/// **1** = a document that could not be read at all.
///
/// # Errors
/// [`Error::Io`] / [`Error::Malformed`] for a file that is not a checkpoint, [`Error::Usage`] for a
/// key that will not load.
pub fn verify(session: Option<&Session>, spec: &VerifySpec) -> Result<Outcome> {
    let mut chain = Vec::new();
    for file in &spec.files {
        chain.push(read_checkpoint(file)?);
    }
    if chain.is_empty() {
        return Err(Error::Usage {
            detail: "`gx verdict-checkpoint verify` takes at least one document, or `-` for stdin"
                .to_string(),
        });
    }

    // --- the signature, which is the one check a key is needed for -------------------------------
    let public: Option<PublicKey> = match &spec.key {
        Some(path) => Some(crate::keys::read_public(path)?),
        None => None,
    };
    let mut findings: Vec<String> = Vec::new();
    let signature = match &public {
        Some(key) => {
            let mut all = true;
            for checkpoint in &chain {
                if let Err(e) = dsse::verify_verdict_checkpoint(checkpoint, &key.verifying()) {
                    all = false;
                    findings.push(format!(
                        "signature: the checkpoint closing at {} does not verify against {}: {e}",
                        checkpoint.window_end,
                        key.key_id()
                    ));
                }
            }
            serde_json::Value::Bool(all)
        }
        None => {
            findings.push(
                "signature: skipped — no --key was given, so nothing here says who signed these \
                 counts. That is a weaker answer than a failure and is reported as its own word \
                 (req/29 §4: do not give skip and pass the same face; sem: SEM-gx-cli-083)"
                    .to_string(),
            );
            serde_json::Value::String("skipped".to_string())
        }
    };

    // --- the arithmetic no signature can rescue --------------------------------------------------
    let (observed, recount) = match (spec.recount_from_journal, session) {
        (true, Some(session)) => {
            let tally = recount(session);
            (tally, serde_json::Value::Bool(true))
        }
        (true, None) => {
            findings
                .push("recount: asked for, and no project was open to recount from".to_string());
            (VerdictTally::default(), serde_json::Value::Bool(false))
        }
        (false, _) => (
            VerdictTally::default(),
            serde_json::Value::String("skipped".to_string()),
        ),
    };
    // 🔴 A verifier holding nothing counts zero, and zero is never above a claim — `audit_verdict_
    // chain`'s own sentence. So an unasked-for recount disables the under-reporting half, and the
    // JSON says `skipped` rather than `true`.
    let ledger_tree_size = match &spec.ledger_checkpoint {
        Some(path) => crate::receipt::read_checkpoint(path)?.tree_size,
        None => chain.last().map_or(0, |c| c.ledger_tree_size),
    };
    let breaks = audit_verdict_chain(&chain, &observed, ledger_tree_size);

    let mut contiguity = true;
    let mut ledger_binding = true;
    let mut recount_ok = true;
    for found in &breaks {
        findings.push(describe(found));
        match found {
            ChainBreak::Gap { .. } | ChainBreak::WindowDoesNotMatchTally { .. } => {
                contiguity = false;
            }
            ChainBreak::BehindTheLedger { .. } | ChainBreak::LedgerWentBackwards { .. } => {
                ledger_binding = false;
            }
            ChainBreak::Underreported { .. } => recount_ok = false,
        }
    }
    let recount_answer = match (&recount, recount_ok) {
        (serde_json::Value::Bool(true), ok) => serde_json::Value::Bool(ok),
        (other, _) => other.clone(),
    };
    let signature_ok = !matches!(signature, serde_json::Value::Bool(false));
    let valid = signature_ok && contiguity && ledger_binding && recount_ok;

    let json = serde_json::json!({
        "valid": valid,
        "checks": {
            "signature": signature,
            "contiguity": contiguity,
            "ledger_binding": ledger_binding,
            "recount": recount_answer,
        },
        "findings": findings,
        "chain": {
            "length": chain.len(),
            "window": [chain.first().map(|c| c.window_start), chain.last().map(|c| c.window_end)],
            "origin": chain.first().map(|c| c.origin.clone()),
            "ledger_tree_size": ledger_tree_size,
        },
        // 🔴 The two limits, printed on every run — ruling #3 and ruling #14 (sem: SEM-gx-cli-084). A verifier who reads
        // `valid: true` and stops has been told what it does not mean, in the same object.
        "not_detected": [
            "a gate widened until nothing is refused publishes deny=0 honestly (ruling #3)", // (sem: SEM-gx-cli-085)
            "one key can sign two internally consistent chains for two verifiers (ruling #14; the \
             consistency proof is v0.2)",
            "across a restart the producer rebuilds its counter from the same journal a recount \
             reads, so a journal that lost a Verdict record is invisible to both (AC-VC-1's \
             declared limit)",
        ],
    });
    Ok(if valid {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, VERIFY_FAILED)
    })
}

/// The oracle: a count from the journal, by 43's transition table.
///
/// The same fold `crates/gx-engine/tests/ac_vc.rs` writes, and it is written **again** here rather
/// than exported from the test, because the test's independence is the whole of AC-VC-1: a surface
/// that called the test's function would be checking the producer against itself one crate over.
/// What is shared is the rule (43), not the code.
fn recount(session: &Session) -> VerdictTally {
    let mut tally = VerdictTally::default();
    for record in session.read().journal().records() {
        match record {
            EngineJournalRecord::Verdict {
                kind,
                verdict_digest: Some(_),
                ..
            }
            | EngineJournalRecord::HumanDecision { kind, .. } => match kind {
                VerdictKind::Admit => tally.admit += 1,
                VerdictKind::Deny => tally.deny += 1,
                VerdictKind::Escalate => tally.escalate += 1,
            },
            // 43 T-4e: an admission no gate made. Folding it into `admit` would report a decision
            // nobody took (M4H4-2, refused twice), so it has its own bucket here as it does there.
            EngineJournalRecord::Verdict {
                verdict_digest: None,
                ..
            } => tally.unverdicted += 1,
            _ => {}
        }
    }
    tally
}

/// One break, as the sentence a person reads.
fn describe(found: &ChainBreak) -> String {
    match found {
        ChainBreak::Gap {
            closed_at,
            reopened_at,
        } => format!(
            "contiguity: one checkpoint closed at {closed_at} and the next opens at {reopened_at}; \
             the verdicts in between are in no document this verifier was handed"
        ),
        ChainBreak::WindowDoesNotMatchTally {
            window_width,
            tally_total,
        } => format!(
            "contiguity: a window {window_width} wide carries a tally of {tally_total}; the \
             checkpoint does not describe its own window"
        ),
        ChainBreak::Underreported {
            kind,
            claimed,
            observed,
        } => format!(
            "recount: this verifier counted {observed} {kind:?} and the chain admits to {claimed}"
        ),
        ChainBreak::BehindTheLedger {
            admits_claimed,
            ledger_tree_size,
        } => format!(
            "ledger_binding: the chain admits to {admits_claimed} admissions and the ledger holds \
             {ledger_tree_size} leaves; every leaf is downstream of an admission, so this chain has \
             stopped publishing"
        ),
        ChainBreak::LedgerWentBackwards { from, to } => format!(
            "ledger_binding: the bound ledger head moves from {from} to {to}; either the \
             checkpoints are out of order or two logs' checkpoints are folded into one chain"
        ),
    }
}

/// Read one checkpoint document, or `-` for stdin.
fn read_checkpoint(file: &str) -> Result<gx_core::VerdictCheckpoint> {
    let raw = if file == "-" {
        use std::io::Read;
        let mut buffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|e| Error::Io {
                action: "read",
                path: "<stdin>".to_string(),
                source: e,
            })?;
        buffer
    } else {
        std::fs::read(file).map_err(|e| Error::Io {
            action: "read",
            path: file.to_string(),
            source: e,
        })?
    };
    serde_json::from_slice(&raw).map_err(|e| Error::Malformed {
        what: "verdict checkpoint",
        path: file.to_string(),
        detail: e.to_string(),
    })
}

/// `--out`, writing the bytes stdout carries (`gx log checkpoint`'s rule, one artefact along).
fn write_document(path: &Path, json: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(crate::io("create", parent))?;
        }
    }
    let body = serde_json::to_vec_pretty(json).map_err(|e| Error::Malformed {
        what: "verdict checkpoint",
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    std::fs::write(path, body).map_err(crate::io("write", path))
}
