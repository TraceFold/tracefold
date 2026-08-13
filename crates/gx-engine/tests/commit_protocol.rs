//! The shape of the commit critical section: what `src/` must contain, in what order, and what it
//! must contain **once**.
//!
//! Spec: 43 §3 T-9..T-11 for the transitions, 43 §7 for journal-first and the one place it is
//! reversed, 42 §3.10 for the receipt and §3.11 for the ledger. req/38 §37 rules M5-24 (`cas_eq`'s
//! `Err`) and M5-25 (the `Provenance` producer); §40 rules M5H3-4 (the frontier and the root) and
//! 規律48 (an intermediate stop where a terminal record repairs an earlier one).
//!
//! # Why this suite is scans and not only runs
//!
//! Three of hand 4's obligations are **absences or singletons**, and a run cannot see either:
//!
//! * 則 2 — 「`adapter.apply` の呼び出し箇所は engine 全体で **1 箇所**」. A counting adapter says how
//!   many times a call was made in one scenario; it cannot say how many roads exist.
//! * **E-M5-1** — every apply is preceded by an `ApplyStarted`. A run measures the pair it walked;
//!   the scan measures the pairing.
//! * 43 §7's journal-first — the ordering of two statements, which is invisible from outside until
//!   the power fails.
//!
//! §30's rule holds throughout: scans read **code lines**, never comments, because this crate's
//! documentation names `apply`, `Provenance` and the ledger at length while discussing them.

mod support;

use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{
    reconstruct, Engine, EngineJournalRecord, InjectedEvidence, Lifecycle, UnreachableEvidence,
};
use support::{
    gate, intent, read_repo, scratch, signing_key, CommitAdapter, Counts, StubAdapter, PERMIT_ALL,
};

const PIPELINE: &str = "crates/gx-engine/src/pipeline.rs";
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The code lines of a source file — comments and blanks removed (§30).
fn code(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with("*") && !l.is_empty())
        .collect()
}

/// Every `.rs` under the crate's `src/`, as `(name, text)`.
fn sources() -> Vec<(String, String)> {
    let dir = support::repo_root().join("crates/gx-engine/src");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("the crate has a src/")
        .map(|entry| {
            let path = entry.expect("a directory entry").path();
            (
                path.file_name()
                    .expect("a named file")
                    .to_string_lossy()
                    .to_string(),
                std::fs::read_to_string(&path).expect("a source file is readable"),
            )
        })
        .collect();
    out.sort();
    out
}

/// The index of the first code line satisfying `f`, or `None`.
fn first(lines: &[&str], f: impl Fn(&str) -> bool) -> Option<usize> {
    lines.iter().position(|l| f(l))
}

/// The code lines of `Engine::commit`, from its signature to the helper declared after it.
///
/// 🔴 Scoped rather than whole-file, and the first draft of this suite was not: a scan over
/// `pipeline.rs` finds `adapter.apply(` inside `Engine::apply_once`, which is **declared after**
/// `commit`, so an ordering probe over the whole file compared the position of a helper's body with
/// the position of a caller's statements and passed for a reason that had nothing to do with the
/// protocol. §30's disease, in this hand's own instrument. What the protocol cares about is the
/// order of the statements **inside the section**, so the section is what is read.
fn commit_body(text: &str) -> Vec<&str> {
    let lines = code(text);
    let start = first(&lines, |l| l.starts_with("pub fn commit(")).expect("`commit` is declared");
    // 🔴 M5 hand 5 moved this boundary. `apply_once` used to be the next function after `commit`;
    // 43 §7's recovery now sits between them, and it walks the same road (`self.apply_once`) and
    // writes the same record (`ApplyStarted`). Leaving the end at `apply_once` silently widened
    // every probe below from 「what `commit` does」 to 「what `commit` and the recovery do」, and the
    // counts moved from two to three without any of the sentences changing. The recovery's own
    // ordering is `tests/crash_recovery.rs`'s claim; this file stays about the critical section.
    let end = first(&lines, |l| l.starts_with("pub fn recover("))
        .expect("43 §7's recovery follows the commit");
    assert!(start < end, "the recovery is declared after the transition");
    lines[start..end].to_vec()
}

/// The indices, in `lines`, of every code line containing `needle`.
fn every(lines: &[&str], needle: &str) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(needle))
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// 則 2 -- one road to the substrate
// ---------------------------------------------------------------------------

/// 🔴 **則 2**: `adapter.apply` is invoked from exactly one line in the whole crate.
///
/// req/78 §3.3: 「**則 2(`S` への道は 1 本)**: `adapter.apply` の呼び出し箇所は engine 全体で **1
/// 箇所**でなければならない」, and AC-035's 「モック呼び出しカウンタ」 is the other instrument.
///
/// The claim is about *roads*, not about *walks*. Hand 4's rollback (43 T-10c) applies the escrowed
/// inverse, which is a second walk down the same road: both go through `Engine::apply_once`, so the
/// count below stays one while the counting adapter in `tests/ac_038.rs` reads two. Two instruments
/// that disagree on the number and agree on the claim is the point of having both.
#[test]
fn adapter_apply_is_invoked_from_one_line_in_the_crate() {
    let mut sites: Vec<String> = Vec::new();
    for (name, text) in sources() {
        for line in code(&text) {
            if line.contains(".apply(") {
                sites.push(format!("{name}: {line}"));
            }
        }
    }
    println!("APPLY_CALL_SITES={} {sites:?}", sites.len());
    assert_eq!(
        sites.len(),
        1,
        "則 2: the engine has one road to a substrate write, and these are the lines that write \
         one: {sites:?}"
    );
    assert!(
        sites[0].starts_with("pipeline.rs:"),
        "the one call belongs in the file the transitions live in: {sites:?}"
    );
}

/// 🔴 **E-M5-1**: the `ApplyStarted` record is appended **before** the call it describes.
///
/// 43 §7's write-ahead rule, at the one place it protects the property gx exists to sell: a crash
/// inside `apply` must leave 「the adapter was asked」 on the device. The order of two statements is
/// not observable from outside the process — a caller cannot tell a record written before the call
/// from one written after until the power fails — so it is read off the source, which is the same
/// instrument `journal_roundtrip.rs` uses for the fsync ordering one layer down.
#[test]
fn every_apply_is_preceded_by_an_apply_started_record() {
    let text = read_repo(PIPELINE);
    let body = commit_body(&text);
    let records = every(&body, "EngineJournalRecord::ApplyStarted");
    let calls = every(&body, "self.apply_once(");

    println!("APPLY_STARTED_APPENDS={records:?} APPLY_ONCE_CALLS={calls:?}");
    assert_eq!(
        records.len(),
        2,
        "one record per application: the forward delta, and 43 T-10c's rollback of it"
    );
    assert_eq!(
        calls.len(),
        2,
        "two walks down 則 2's one road -- the forward delta and the rollback"
    );
    for (n, (record, call)) in records.iter().zip(calls.iter()).enumerate() {
        assert!(
            record < call,
            "E-M5-1: application {n} is made before its record is written"
        );
    }
    // Pairing, not just ordering: a second record written before *both* calls would satisfy the
    // comparison above while leaving the rollback unannounced. The record for application n has to
    // fall after the call for application n-1.
    assert!(
        calls[0] < records[1],
        "the rollback's record belongs to the rollback, not to a second announcement of the first \
         application"
    );
}

/// 🔴 43 §7's journal-first, transition by transition, in the order the section runs.
///
/// `CommittingStarted` (T-9) before the provenance, before the CAS, before the escrow, before the
/// apply. The one record that is **not** write-ahead is `Committed`, and that is 43 T-11's own cell:
/// it carries `ledger_seq`, which does not exist until the append has answered. The probe asserts
/// the exception as well as the rule, so that a hand which "fixed" the ordering would fail here
/// rather than silently break 43 §7-3b's recovery.
#[test]
fn the_critical_section_journals_before_each_side_effect() {
    let text = read_repo(PIPELINE);
    let body = commit_body(&text);
    let at = |needle: &str| {
        first(&body, |l| l.contains(needle)).unwrap_or_else(|| panic!("no code line has {needle}"))
    };

    let committing = at("EngineJournalRecord::CommittingStarted");
    let provenance = at("EngineJournalRecord::ProvenanceDerived");
    let cas = at(".cas_eq(");
    let escrowed = at("EngineJournalRecord::InverseEscrowed");
    let put = at("self.blobs.put(&inverse)");
    let apply = at("self.apply_once(");
    let append = at(".append(*id, receipt_digest");
    let committed = at("EngineJournalRecord::Committed");

    println!(
        "ORDER committing={committing} provenance={provenance} cas={cas} escrow={escrowed} \
         put={put} apply={apply} ledger_append={append} committed={committed}"
    );
    assert!(
        committing < provenance && provenance < cas,
        "43 T-9 opens the section and M5-25's record is written before the world can move"
    );
    assert!(
        escrowed < put,
        "43 §7: the journal record names the escrow before the body is stored"
    );
    assert!(
        escrowed < apply,
        "43 T-10b escrows the inverse before T-10c can need it"
    );
    assert!(
        append < committed,
        "🔴 43 T-11 is journal-first's one exception: `ledger_seq` does not exist until the append \
         has answered, which is why 43 §7-3b exists"
    );
}

// ---------------------------------------------------------------------------
// M5-24 -- where `cas_eq`'s third answer goes
// ---------------------------------------------------------------------------

/// 🔴 **M5-24 採(a)**: the CAS's `Err` becomes `InternalError` and never `PreconditionChanged`.
///
/// > `cas_eq` の `Err` は `Aborted(InternalError)`(43 T-13 逐語一致・T-13 を踏む 1 本目の経路)
///
/// The behavioural half cannot be written against a stub adapter that answers consistently — an
/// `Err` from `cas_eq` means two fingerprints over different scopes or from different substrates,
/// which is a wiring bug rather than a scenario — so what is measured here is that the two arms of
/// the `match` carry the two different reasons. A hand that folded them would have one.
#[test]
fn the_cas_has_three_answers_and_two_reasons() {
    let text = read_repo(PIPELINE);
    let lines = code(&text);
    let start =
        first(&lines, |l| l.contains("match fp0.cas_eq(&fp1)")).expect("the CAS is matched");
    let arms: Vec<&&str> = lines[start..start + 8].iter().take(8).collect();
    let joined = arms.iter().map(|l| **l).collect::<Vec<_>>().join(" ");

    println!("CAS_MATCH_ARMS={joined}");
    assert!(
        joined.contains("Ok(false) => return self.abort(id, AbortReason::PreconditionChanged"),
        "43 T-10a: 「動いた」 is PreconditionChanged"
    );
    assert!(
        joined.contains("Err(_) => return self.abort(id, AbortReason::InternalError"),
        "M5-24 採(a): 「その比較は意味を持たない」 is a wiring bug, and 43 T-13 is where bugs go"
    );
}

// ---------------------------------------------------------------------------
// M5-25 -- D-7's third window
// ---------------------------------------------------------------------------

/// 🔴 **M5-25 採(a)**: the engine is the `Provenance` producer, and there is exactly one of it.
///
/// D-7's rule is 「2 回 defer して消費者 0 なら作った物自体を疑う」, and §37 settled the third
/// window by making the engine the producer with the journal as the destination. This probe is the
/// count that decides whether the ruling landed: `Provenance::derive_from` had **zero** shipping
/// callers across M2, M3 and M4, and after this hand it has one.
#[test]
fn the_engine_is_the_one_producer_of_provenance() {
    let mut sites: Vec<String> = Vec::new();
    for (name, text) in sources() {
        for line in code(&text) {
            if line.contains("Provenance::derive_from") {
                sites.push(format!("{name}: {line}"));
            }
        }
    }
    println!("PROVENANCE_PRODUCERS={} {sites:?}", sites.len());
    assert_eq!(
        sites.len(),
        1,
        "M5-25 採(a): one producer, in the engine — {sites:?}"
    );
}

// ---------------------------------------------------------------------------
// I-3 -- the gx-gate accessors, counted rather than remembered
// ---------------------------------------------------------------------------

/// **I-3**: whether hand 4 became the consumer §26 reserved the decision for.
///
/// req/38 §26 I-3 逐語: 「accessor 3 件は survivor として数え続け(分類表つき)・**M5 の engine が消費者
/// にならなければ公開面から落とす**を D-7 形で予約」. The three are gx-gate's `Gate::policies`,
/// `Gate::invariants` and `PolicyEngine::is_empty` (req/78 §5's row 30 calls them gx-log's, which
/// is a mislabel — the audit that raised them is `req/67 §2.2`, and the crate is gx-gate).
///
/// This probe does not decide anything; it prints the count the ruling asked to be taken. Dropping
/// a public item is a gx-gate `src` change, which this hand may not make (req/38 §40's scope), so
/// the measurement goes in the report as a ticket rather than into an edit.
#[test]
fn the_three_gx_gate_accessors_are_counted_in_this_hand() {
    let names = ["policies(", "invariants(", "is_empty("];
    let mut consumed: Vec<String> = Vec::new();
    for (file, text) in sources() {
        for line in code(&text) {
            // Only calls **through the gate**: this crate has its own `is_empty` on the journal and
            // the blob store, and counting those would report the engine consuming itself.
            for name in names {
                if line.contains(&format!("gate.{name}"))
                    || line.contains(&format!("policies().{name}"))
                {
                    consumed.push(format!("{file}: {line}"));
                }
            }
        }
    }
    println!(
        "GX_GATE_ACCESSOR_CONSUMERS_IN_ENGINE={} {consumed:?}",
        consumed.len()
    );
    assert!(
        consumed.is_empty(),
        "I-3 predicted the engine would be the first consumer. If this fires, the engine has \
         become one and the D-7-shaped reservation resolves the other way: {consumed:?}"
    );
}

// ---------------------------------------------------------------------------
// The ledger, wired
// ---------------------------------------------------------------------------

/// 🔴 The engine holds a `LedgerStore`, and appends to it in exactly one place (43 T-11).
///
/// 42 §3.11's ledger is the public witness log, and INV-S3 is 「各`TransformationId`について
/// `ledger`entryは高々1件」. Idempotence lives in gx-log (ASM-43-1, keyed on the transformation), so
/// the engine's obligation is narrower and structural: one road in, so that there is one place for
/// a reviewer to check the key against.
#[test]
fn the_ledger_is_wired_and_appended_to_in_one_place() {
    let text = read_repo(PIPELINE);
    let lines = code(&text);
    let held = lines
        .iter()
        .filter(|l| l.contains("ledger: LedgerStore"))
        .count();
    // The call is written across four lines by rustfmt, so the needle is the argument list rather
    // than the receiver -- 「`.ledger` and `.append(` on one line」 was the first draft and it
    // reported zero for a call that is plainly there.
    //
    // 🔴 M5 hand 5: the needle used to be `.append(*id, receipt_digest`, and the recovery's call
    // passes an owned `id` rather than a reference. The old needle would have counted **one** while
    // the crate held two -- an absence probe reporting a road it could not see, which is §30's
    // disease in the instrument rather than in the code. The argument name is what both calls share.
    let appends = lines
        .iter()
        .filter(|l| l.contains(".append(") && l.contains("receipt_digest"))
        .count();
    let issues = lines
        .iter()
        .filter(|l| l.contains("Receipt::issue("))
        .count();

    println!("LEDGER_FIELDS={held} LEDGER_APPEND_SITES={appends} RECEIPT_ISSUE_SITES={issues}");
    assert_eq!(held, 1, "one ledger, held by the engine");
    // Two roads to the ledger, and INV-S3 survives both because the key idempotency is gx-log's
    // (ASM-43-1): T-11 appends what the commit witnessed, and 43 §7-3c appends what a restart
    // completed. The recovery's road is the one 43 §7-3b/c describes and cannot be shared with
    // T-11's -- it appends only after finding the ledger does *not* already hold the entry.
    assert_eq!(appends, 2, "43 T-11 appends once, and 43 §7-3c once");
    // 🔴 M5 hand 6 makes it three, and the third is a **different kind**: ASM-14's
    // `VerdictReceipt`, issued by `Engine::issue_verdict_receipt` for T-4a/b/c, T-4e and T-5/T-5b
    // (M5H4-6). The count is asserted rather than relaxed to `>= 2` so that a fourth road has to be
    // justified where it is added -- two of these three are `CommitReceipt`s and the split matters.
    assert_eq!(
        issues, 3,
        "43 T-11 issues a CommitReceipt, 43 §7-3b re-issues one, and 42 §3.10's VerdictReceipt is \
         issued in one more place (M5H4-6)"
    );
    let kinds = |needle: &str| lines.iter().filter(|l| l.contains(needle)).count();
    assert_eq!(kinds("ReceiptKind::CommitReceipt"), 2);
    assert_eq!(kinds("ReceiptKind::VerdictReceipt"), 1);
}

// ---------------------------------------------------------------------------
// The section, run
// ---------------------------------------------------------------------------

/// A committed transformation, with the counters behind it.
fn committed(
    name: &str,
) -> (
    Engine<InjectedEvidence>,
    gx_core::TransformationId,
    Arc<Counts>,
) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &signing_key()).expect("commit");
    assert_eq!(state, Lifecycle::Committed, "the fixture commits");
    (engine, id, counts)
}

/// 🔴 **E-M4-31 / M5-18 採(a)**: `Timestamp(0)` never reaches the engine's record of the apply.
///
/// req/38 §31 逐語: 「engine が commit 時に上書きする」, and the adapter answers zero because 41 §6
/// injects the clock at the engine boundary — `gx-adapter-fs` does exactly this (`apply.rs:104`),
/// and [`support::CommitAdapter`] copies it so that the overwrite is measured rather than assumed.
///
/// Two claims, because 「the engine overwrote it」 and 「no zero reached the journal」 are different
/// facts and the ruling names the second: 「`Timestamp(0)` が journal に到達したら engine の bug」.
#[test]
fn no_timestamp_zero_reaches_the_engine_from_the_adapter() {
    let (engine, id, _counts) = committed("h4_applied_at");
    let applied_at = engine.applied_at(&id).expect("the commit recorded one");
    let zeros: Vec<&str> = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.at() == Timestamp(0))
        .map(EngineJournalRecord::kind)
        .collect();

    println!("APPLIED_AT={applied_at:?} INJECTED={AT:?} JOURNAL_ZERO_TIMESTAMPS={zeros:?}");
    assert_eq!(
        applied_at, AT,
        "E-M4-31: the moment is the engine's injected one, not the adapter's"
    );
    assert_ne!(applied_at, Timestamp(0), "the adapter's answer was zero");
    assert!(
        zeros.is_empty(),
        "E-M4-31: 「Timestamp(0) が journal に到達したら engine の bug」 -- {zeros:?}"
    );
}

/// 🔴 **M5H3-4**: the journal-witnessed frontier and the ledger's own log agree.
///
/// §40 rules 「手 4 が LedgerStore を結線した同 turn で『frontier と本物の root の一致』を probe 化
/// する」, which turns 43 §7-3b's recovery question into a standing check. The engine's own
/// `ledger_agrees` compares the count, each row and the root; this probe runs it, and then takes the
/// three comparisons apart so that a failure names which one moved.
#[test]
fn the_frontier_and_the_ledger_agree_after_a_commit() {
    let (engine, id, _counts) = committed("h4_frontier");
    let sigma = engine.sigma();
    let frontier = sigma.ledger();
    let log = engine.ledger().log();

    println!(
        "SIGMA_LEDGER_ROWS={} LEDGER_LEAVES={} LEDGER_AGREES={} ROOT={:?}",
        frontier.len(),
        log.len(),
        engine.ledger_agrees(),
        log.root()
    );
    assert!(engine.ledger_agrees(), "M5H3-4: the two disagree");
    assert_eq!(frontier.len() as u64, log.len(), "count");
    assert_eq!(frontier[0].transformation, id);
    assert_eq!(
        log.entry(frontier[0].ledger_seq)
            .expect("the frontier names a leaf the log holds")
            .transformation,
        id,
        "row"
    );
    assert_eq!(
        log.root_at(frontier.len() as u64),
        log.root(),
        "root: the frontier is a prefix of this tree and not a list of the same length"
    );
}

/// 🔴 **規律48**: the intermediate states of the critical section, reconstructed and stopped at.
///
/// §40 制定: 「終端 record が中間 record の情報を再供給する経路では、中間状態で止まる probe を必ず
/// 1 本置く」, with `Committed` over `CommittingStarted` named as the first case. The masking is
/// real: `reconstruct`'s `Committed` arm writes `state = Committed`, so a broken `CommittingStarted`
/// arm is invisible in any journal that reached T-11. The prefixes below are the same journal an
/// execution wrote, cut where a crash would have cut it.
#[test]
fn the_critical_sections_intermediate_states_survive_being_stopped_at() {
    let (engine, id, _counts) = committed("h4_regulation_48");
    let records = engine.journal().records();
    let kinds: Vec<&str> = records.iter().map(EngineJournalRecord::kind).collect();
    println!("COMMITTED_RUN_JOURNAL={kinds:?}");

    let upto = |kind: &str| {
        let at = kinds
            .iter()
            .position(|k| *k == kind)
            .expect("the record ran");
        reconstruct(&records[..=at])
    };

    // Stopped at T-9: `Committing`, and nothing about an apply.
    let at_t9 = upto("CommittingStarted");
    let row = at_t9.state_of(&id).expect("the row is there");
    println!(
        "AT_T9 state={:?} apply_started={:?} provenance={}",
        row.state,
        row.apply_started,
        row.provenance.is_some()
    );
    assert_eq!(
        row.state,
        Some(Lifecycle::Committing),
        "43 T-9 leaves the transformation in the critical section"
    );
    assert!(row.apply_started.is_none(), "nothing has been applied yet");
    assert!(
        row.provenance.is_none(),
        "M5-25's record is the next one, so it is not here yet"
    );

    // Stopped at the provenance: the value is there before the world can move.
    let at_provenance = upto("ProvenanceDerived");
    assert!(
        at_provenance
            .state_of(&id)
            .expect("row")
            .provenance
            .is_some(),
        "the provenance survives a crash before the apply"
    );

    // Stopped at E-M5-1's record: the state 43 had no way to describe before it existed.
    let at_apply = upto("ApplyStarted");
    let row = at_apply.state_of(&id).expect("row");
    println!(
        "AT_APPLY_STARTED state={:?} apply_started={:?}",
        row.state, row.apply_started
    );
    assert_eq!(
        row.state,
        Some(Lifecycle::Committing),
        "still inside the section: `ApplyStarted` fixes no state of its own"
    );
    assert_eq!(
        row.apply_started,
        Some(
            engine
                .planned_delta(&id)
                .expect("the delta")
                .reference()
                .cid
        ),
        "🔴 req/78 §3.2 Λ4: this is the fact that separates 「the world did not move」 from 「the \
         world moved and nothing recorded it」"
    );

    // And the terminal record, which is what would have hidden all three.
    let whole = reconstruct(records);
    assert_eq!(
        whole.state_of(&id).expect("row").state,
        Some(Lifecycle::Committed)
    );
}

/// 🔴 AC-039's comparison, over a journal an **execution** wrote (hand 3's other half).
///
/// req/81 §5.3 recorded the weakness plainly: Σ's `escrow` and `ledger` components were empty on the
/// live side, so `tests/sigma_replay.rs` measured the reconstruction against a journal a test had
/// assembled — 「2 経路の一致」ではなく「再構成そのもの」. T-10b and T-11 are this hand's, so both
/// components are live now and the comparison is the one AC-039 asks for, with four non-empty
/// components instead of two.
#[test]
fn sigma_agrees_with_its_reconstruction_after_a_real_commit() {
    let (engine, id, _counts) = committed("h4_sigma");
    let live = engine.sigma();
    let replayed = reconstruct(engine.journal().records());

    let live_bytes = live.canonical_bytes().expect("Σ has a canonical form");
    let replayed_bytes = replayed.canonical_bytes().expect("so does the replay");
    println!(
        "SIGMA_LIVE_BYTES={} SIGMA_REPLAYED_BYTES={} BIT_EQUAL={} ESCROW_ROWS={} LEDGER_ROWS={}",
        live_bytes.len(),
        replayed_bytes.len(),
        live_bytes == replayed_bytes,
        live.escrow().len(),
        live.ledger().len()
    );
    assert_eq!(
        live_bytes, replayed_bytes,
        "AC-039: the live Σ and the one rebuilt from the journal are bit-equal"
    );
    assert_eq!(live.escrow().len(), 1, "T-10b filled the escrow component");
    assert_eq!(live.ledger().len(), 1, "T-11 filled the ledger component");
    assert!(
        live.state_of(&id).expect("row").provenance.is_some(),
        "M5-25's value is part of the state a replay reproduces"
    );
}

/// 🔴 **M5-25 採(a)**, run: what the engine actually put in the record (42 §3.9).
///
/// D-7's third window closes here. The value is checked field by field rather than for existence,
/// because a `Provenance` with an empty environment and no inputs would satisfy 「a producer exists」
/// while recording nothing — which is what two milestones of 「the type is there」 already were.
#[test]
fn the_provenance_the_engine_derived_says_what_42_3_9_asks_for() {
    let (engine, id, _counts) = committed("h4_provenance");
    let p = engine
        .provenance(&id)
        .expect("M5-25: the engine derives one");
    let intent_id = engine.intent_of(&id).expect("T-2 recorded it");
    let pre = engine.precondition_snapshot(&id).expect("T-2 read one");

    println!(
        "PROVENANCE transformation={:?} intent_digest={:?} inputs={} engine_version={} \
         adapter_version={} host_id={:?} correlation_id={:?}",
        p.transformation,
        p.intent_digest,
        p.input_objects.len(),
        p.environment.engine_version,
        p.environment.adapter_version,
        p.environment.host_id,
        p.environment.correlation_id
    );
    assert_eq!(p.transformation, id);
    assert_eq!(
        p.intent_digest,
        Some(intent_id.0),
        "42 §3.9: 「`submit`時のIntentのcanonical digest」, and `parents` is empty here"
    );
    assert_eq!(
        p.input_objects,
        vec![*pre.id()],
        "the one snapshot the engine watched the adapter read (see `derive_provenance`)"
    );
    assert_eq!(
        p.environment.adapter_version, "commit-adapter-1",
        "M5H4-4: the version comes from the registration, because 41 §4's trait cannot answer it"
    );
    assert!(
        !p.environment.engine_version.is_empty(),
        "42 §3.9: 「gx-engineのビルドバージョン」"
    );
    assert_eq!(p.environment.host_id, None, "ASM-10: 単一ノード運用");

    let journalled = engine
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::ProvenanceDerived { provenance, .. } => Some(provenance.clone()),
            _ => None,
        })
        .expect("M5-25 採(a)+journal");
    assert_eq!(&journalled, p, "the record and the row are one value");
}

/// 🔴 **M5H3-1(c)**: the receipt is issued, and the clock **still** does not reach Σ.
///
/// §40 sent the question here: 「clock が Σ に入る最初の窓は手 4 の receipt `issued_at`(42 §3.10)」.
/// The measurement says it is not, and for two separate reasons that both had to hold:
///
/// * **E-M2-6** keeps `issued_at` outside the signed payload, and a receipt is not a component of Σ
///   (E-M5-2 names three: the state table, the ledger, the escrow index);
/// * the ledger's own root does not carry a clock either — 42 §3.11's `LedgerLeaf` is
///   `{transformation, receipt_digest, index}`, and `appended_at` sits on the `LedgerEntry` beside
///   the leaf rather than inside the hash.
///
/// So Σ is clock-free after hand 4 as it was after hand 3, and both halves are measured: two runs
/// with different clocks reach the same Σ bytes, and their receipts differ.
#[test]
fn the_receipt_carries_the_clock_and_sigma_still_does_not() {
    let run = |name: &str, at: Timestamp| {
        let dir = scratch(name);
        let mut engine = Engine::open(
            dir.join("journal.bin"),
            gate(PERMIT_ALL),
            InjectedEvidence::none(),
        )
        .expect("a fresh journal opens");
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
        let i = intent("/tmp/target.txt", "after");
        engine.submit(&i, 42, at).expect("submit");
        let id = engine.plan(&i, at).expect("plan");
        engine
            .verify(&id, at, &signing_key(), None)
            .expect("verify");
        engine.canonicalize(&id, at, None).expect("canonicalize");
        engine.commit(&id, at, &signing_key()).expect("commit");
        let sigma = engine.sigma().canonical_bytes().expect("Σ encodes");
        let receipt = engine.receipt(&id).expect("T-11 issued one").clone();
        (sigma, receipt)
    };

    let later = Timestamp(AT.0 + 86_400_000_000_000);
    let (sigma_a, receipt_a) = run("h4_clock_a", AT);
    let (sigma_b, receipt_b) = run("h4_clock_b", later);

    println!(
        "ISSUED_AT_A={:?} ISSUED_AT_B={:?} RECEIPTS_EQUAL={} SIGMA_EQUAL={} PAYLOADS_EQUAL={} \
         LEDGER_DIGESTS_EQUAL={}",
        receipt_a.issued_at,
        receipt_b.issued_at,
        receipt_a == receipt_b,
        sigma_a == sigma_b,
        receipt_a.envelope.payload == receipt_b.envelope.payload,
        receipt_a.ledger_digest().expect("a") == receipt_b.ledger_digest().expect("b")
    );
    assert_eq!(
        receipt_a.issued_at, AT,
        "42 §3.10's `issued_at` is injected"
    );
    assert_eq!(receipt_b.issued_at, later);
    assert_ne!(
        receipt_a, receipt_b,
        "the receipts differ, and only in the clock"
    );
    assert_eq!(
        receipt_a.envelope.payload, receipt_b.envelope.payload,
        "E-M2-6: the clock is outside the signed core, so the signed bytes are identical"
    );
    assert_eq!(
        sigma_a, sigma_b,
        "🔴 M5H3-1: the clock still does not reach Σ after the receipt exists"
    );
}

/// The receipt says what 42 §3.10 says a `CommitReceipt` says, and verifies offline.
///
/// AC-018's verification path, reached from the engine for the first time: the signature covers the
/// payload, the payload names the transformation, and the inclusion proof resolves against the
/// ledger's root. A receipt the engine issued that its own ledger could not witness would be the two
/// halves of P-7 disagreeing.
#[test]
fn the_commit_receipt_is_a_commit_receipt_and_verifies_against_the_ledger() {
    let (engine, id, _counts) = committed("h4_receipt");
    let receipt = engine.receipt(&id).expect("T-11 issued one");
    let payload = receipt.payload().expect("the payload decodes");
    // The anchor a third party would hold: 42 §3.11's signed tree head, taken over the engine's own
    // log. `unsigned_checkpoint` is gx-log's builder for it, and the signature is not what this
    // probe is about -- `verify_offline` reads the size and the root out of it.
    let anchor = gx_log::proof::unsigned_checkpoint(engine.ledger().log(), "glovrex-ledger/v1", AT)
        .expect("a non-empty log has a head");

    let key = signing_key();
    let checks = gx_witness::verify_offline(receipt, &key.verifying(), Some(&anchor))
        .expect("the receipt verifies");
    println!(
        "ANCHOR_SIZE={} RECEIPT_KIND={:?} ENFORCED={} FPE={} INVERSE={:?} PROOF={} CHECKS={checks:?}",
        anchor.tree_size,
        payload.receipt_kind,
        payload.enforced,
        payload.fail_posture_engaged,
        payload.inverse_delta,
        payload.inclusion_proof.is_some()
    );
    assert_eq!(payload.receipt_kind, gx_witness::ReceiptKind::CommitReceipt);
    assert_eq!(payload.transformation, id);
    assert_eq!(
        payload.canonical_cid,
        engine.canonical_cid(&id).expect("T-8 fixed one"),
        "42 §3.10: 「`CommitReceipt`ではcanonicalizeステージ（43 T-8）で再確認された値」"
    );
    assert!(payload.enforced, "nothing degraded this transformation");
    assert!(!payload.fail_posture_engaged);
    assert_eq!(
        payload.inverse_delta,
        engine.escrowed_inverse(&id),
        "42 §3.10: 「エスクロー済み逆deltaのCID」"
    );
    assert!(
        payload.postcondition_fingerprint.is_some(),
        "42 §3.10: set once something was applied"
    );
    assert!(
        checks.verified(),
        "AC-018's offline verification: {checks:?}"
    );
}

/// 🔴 **M5H4-3 → E-M5-11**: a T-4e degraded admission **is** written down now, and says so.
///
/// This probe was hand 4's refusal, inverted by the erratum hand 4 raised. Its old body asserted
/// `Error::Unrepresentable` and 「the refusal is before T-9: a section that cannot close is not
/// opened」, and the report called it 「A limitation measured, not a feature: AC-037 (hand 6) asks
/// for this path under `RecordOnly`, and it will need a ruling」. §41 gave the ruling:
///
/// > **M5H4-3 採(a)**=**E-M5-11・手 6 発射条件**: `ReceiptPayload.verdict` を **`Option`** にする…
/// > 実装窓=**手 6**(AC-037 がこの経路を正面から踏む)
///
/// So the assertion flips rather than being deleted, and what it now measures is the shape 43 T-4e
/// requires: 「`enforced=false`と`fail_posture_engaged=true`を必ずreceiptに刻む」, with **no
/// verdict** — because no gate ran and no proof exists to digest.
///
/// The `Unrepresentable` refusal did not vanish with it: `tests/ac_037.rs` reaches the half-filled
/// pair the engine still refuses.
#[test]
fn a_degraded_admission_commits_and_its_receipt_says_no_gate_ran() {
    let dir = scratch("h6_t4e");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        UnreachableEvidence::new("the collector is down"),
    )
    .expect("a fresh journal opens")
    .with_posture(gx_core::FailPosture::FailOpen);
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let state = engine
        .verify(&id, AT, &signing_key(), None)
        .expect("T-4e admits it, degraded");
    engine
        .canonicalize(&id, AT, None)
        .expect("T-8 with enforced=false");

    let records_before = engine.journal().len();
    let committed = engine.commit(&id, AT, &signing_key()).expect("T-11");
    let receipt = engine.receipt(&id).expect("a CommitReceipt was issued");
    let payload = receipt.payload().expect("the payload decodes");
    println!(
        "T4E_STATE={state:?} COMMITTED={committed:?} ENFORCED={:?} FPE={:?} VERDICT={:?}          PAYLOAD_VERDICT={:?} PAYLOAD_ENFORCED={} PAYLOAD_FPE={}          RECORDS_BEFORE={records_before} RECORDS_AFTER={} APPLY={} WORLD={:?}",
        engine.enforced(&id),
        engine.fail_posture_engaged(&id),
        engine.verdict(&id),
        payload.verdict,
        payload.enforced,
        payload.fail_posture_engaged,
        engine.journal().len(),
        counts.totals()[4],
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );
    assert_eq!(state, Lifecycle::Admitted);
    assert_eq!(committed, Lifecycle::Committed);
    assert_eq!(
        engine.verdict(&id),
        None,
        "no gate ran, so there is no verdict"
    );
    assert_eq!(
        payload.verdict, None,
        "E-M5-11: the receipt carries the absence instead of a minted digest"
    );
    assert!(
        !payload.enforced && payload.fail_posture_engaged,
        "43 T-4e: 「`enforced=false`と`fail_posture_engaged=true`を必ずreceiptに刻む」"
    );
    assert_eq!(counts.totals()[4], 1, "the world moved exactly once");

    // ASM-14's other kind was issued too, at T-4e itself, and it is the one that records the
    // degradation at the moment it happened rather than at the end.
    let verdicts = engine.verdict_receipts(&id);
    let at_verdict = verdicts[0].payload().expect("decodes");
    println!(
        "T4E_VERDICT_RECEIPTS={} KIND={:?} VERDICT={:?} FPE={}",
        verdicts.len(),
        at_verdict.receipt_kind,
        at_verdict.verdict,
        at_verdict.fail_posture_engaged
    );
    assert_eq!(verdicts.len(), 1, "one verdict-stage receipt, from T-4e");
    assert_eq!(
        at_verdict.receipt_kind,
        gx_witness::ReceiptKind::VerdictReceipt
    );
    assert_eq!(at_verdict.verdict, None);
    assert!(at_verdict.fail_posture_engaged);
}

/// 🔴 **M5H3-7**: the mirror count, taken again now that receipts and provenance are persisted.
///
/// §40 asked for it here: 「mirror は現状 2 本。receipt/Provenance の永続化で 3 本目が要るかを数え
/// 直して起票」. The answer is **no third mirror**, and the reason is a property of the types rather
/// than a choice: a mirror exists in this crate when a lower crate's type has no serde face because
/// it is minted through a checked constructor (`Fingerprint`, `PlannedDelta`). `Provenance`,
/// `Environment`, `Receipt` and `ReceiptPayload` all derive `Serialize` and `Deserialize` in
/// gx-witness, so the journal writes them directly.
///
/// The set is asserted by name rather than by count, so that a third mirror has to be justified in
/// the place it is added.
#[test]
fn the_mirrors_are_still_the_two_hands_one_and_three_declared() {
    let mut mirrors: Vec<String> = Vec::new();
    for (file, text) in sources() {
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("#[derive(") && line.contains("Deserialize") {
                if let Some(next) = lines.get(i + 1) {
                    if let Some(name) = next
                        .strip_prefix("pub struct ")
                        .or_else(|| next.strip_prefix("struct "))
                    {
                        mirrors.push(format!(
                            "{file}::{}",
                            name.split([' ', '{', '(']).next().unwrap_or(name)
                        ));
                    }
                }
            }
            if let Some(rest) = line.strip_prefix("impl<'de> Deserialize<'de> for ") {
                let name = rest.split([' ', '{']).next().unwrap_or_default();
                mirrors.push(format!("{file}::{name}"));
            }
        }
    }
    mirrors.sort();
    println!("SERDE_MIRRORS_IN_ENGINE={} {mirrors:?}", mirrors.len());
    assert_eq!(
        mirrors,
        vec![
            "store.rs::BlobRecord".to_string(),
            "store.rs::FingerprintRecord".to_string(),
            "store.rs::PayloadBytes".to_string(),
        ],
        "M5H3-7: hand 4 persists receipts and provenance and adds no mirror -- a third one here \
         would be the count the ruling asked to be re-taken"
    );
}

/// A second engine over the same files sees the ledger the first one wrote, and not the table.
///
/// 42 §3.13 keeps the journal private to the engine and the ledger public, and the ledger's own
/// replay (`gx_log::LedgerStore::open`) is what makes 「公開witness台帳」 true across a restart. Σ's
/// view of it does **not** come back, which is M5H3-5's window and hand 5's work — asserted here so
/// that the partial recovery is a measured fact rather than a surprise for the hand that meets it.
#[test]
fn a_reopened_engine_finds_the_ledger_and_not_the_state_table() {
    let dir = scratch("h4_reopen");
    let path = dir.join("journal.bin");
    let key = signing_key();
    let id = {
        let mut engine = Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        engine.register_adapter(Arc::new(adapter), "commit-adapter-1");
        let i = intent("/tmp/target.txt", "after");
        engine.submit(&i, 42, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        engine
            .verify(&id, AT, &signing_key(), None)
            .expect("verify");
        engine.canonicalize(&id, AT, None).expect("canonicalize");
        engine.commit(&id, AT, &key).expect("commit");
        id
    };

    let reopened: Engine<InjectedEvidence> =
        Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none()).expect("the files reopen");
    let sigma = reopened.sigma();
    println!(
        "REOPENED_LEAVES={} REOPENED_ROWS={} REOPENED_DRAFTS={} JOURNAL_RECORDS={}",
        reopened.ledger().log().len(),
        sigma.transformations().len(),
        sigma.drafts().len(),
        reopened.journal().len()
    );
    assert_eq!(
        reopened.ledger().log().len(),
        1,
        "the ledger replays itself: the public log survives"
    );
    assert_eq!(
        reopened
            .ledger()
            .log()
            .entry(0)
            .expect("the leaf")
            .transformation,
        id
    );
    assert!(
        sigma.transformations().is_empty(),
        "M5H3-5: `Engine::open` does not rebuild the in-flight table, and hand 4 does not change \
         that -- a rebuilt ledger component beside an empty state table would be a partial Σ \
         presented as a whole one"
    );
    assert_eq!(
        reconstruct(reopened.journal().records())
            .transformations()
            .len(),
        1,
        "the journal on disk can rebuild the row; the engine has not been asked to"
    );
}

/// The stub adapter of hands 2 and 3 still refuses to apply, so their fixtures cannot commit.
///
/// Not a defect: `StubAdapter::apply` answers `Error::Unimplemented`, which is 「未実装」 rather than
/// 「失敗」 (§32 M4H4-2) and is what kept hand 2 from reaching past T-8. Hand 4 adds a second fixture
/// rather than changing the first, so the numbers hands 2 and 3 reported are still the numbers their
/// frozen instruments print (A-7).
#[test]
fn the_older_fixture_still_refuses_to_apply() {
    let dir = scratch("h4_stub_refuses");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    let i = intent("/tmp/x", "v1");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine
        .verify(&id, AT, &signing_key(), None)
        .expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");

    let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");
    println!(
        "STUB_COMMIT_STATE={state:?} ROLLBACK={:?}",
        engine.rollback(&id)
    );
    assert_eq!(
        state,
        Lifecycle::Aborted(gx_core::AbortReason::ApplyFailed),
        "an adapter that cannot apply is T-10c, whatever its reason for not applying"
    );
}
