// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 43 §7's recovery, from the source and from the crashes themselves.
//!
//! Spec: 43 §7 (the write-ahead rule and the three-step procedure), 51 §8.1 (the three injection
//! points), 34 AC-043, req/78 §3.2 Λ4 (the counter-example E-M5-1 closes), req/38 §37 M5-01
//! adopted (a) (sem: SEM-gx-engine-684), §40 M5H3-5, §41 M5H4-7.
//!
//! # The structural probes come first, and one of them is the whole hand
//!
//! `the_recovery_never_re_runs_the_cas` is Λ4 as a scan. The counter-example is not that recovery
//! *might* misread its own footprint — it is that the procedure 43 §7-3c writes down **does**, every
//! time, in the window between a successful `apply` and the `ledger.append` that records it. So the
//! defence cannot be a careful branch: it has to be that the comparison is not reachable from the
//! recovery at all. A scan says that and a behavioural probe cannot, because no input exercises the
//! road that was never built.
//!
//! The behavioural half is `the_recovery_of_λ4s_window_disagrees_with_the_recovery_43_wrote`: the
//! same crashed directory, recovered twice — once by this crate, and once by a shim that ignores
//! `ApplyStarted` and re-runs the CAS the way 43 §7-3c literally says. One reaches `Committed` with
//! a ledger entry; the other reaches `Aborted(PreconditionChanged)` with an empty ledger and a world
//! that has already changed. That second outcome is "applied, yet nothing recorded it" (sem:
//! SEM-gx-engine-685) — the thing gx exists
//! to make impossible — and it is produced here on purpose so that the difference is measured rather
//! than argued.

mod support;

use std::sync::Arc;

use gx_core::{AbortReason, Cid, FailPosture, Fingerprint, SubstrateKind, Timestamp};
use gx_engine::{
    reconstruct, Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle,
    UnreachableEvidence,
};

use support::{
    copy_tree, gate, intent, kill_at, need, probe, read_repo, record_boundaries, scratch,
    signing_key, value, CommitAdapter, PERMIT_ALL,
};

/// The clock the probe binary runs its pipeline under (`crash_probe`'s `RUN_AT`).
const RUN_AT: i64 = 1_754_000_000_000_000_000;
/// The clock its restart runs under (`crash_probe`'s `RECOVER_AT`), a day later.
const RECOVER_AT: i64 = 1_754_086_400_000_000_000;

/// The probe binary's digest, spelled again here.
///
/// The shim below is a **rival implementation of the recovery**, so it computes its own view of the
/// world the way the adapter does. Sharing the function would make the two agree for a reason that
/// is not the one under test.
fn digest_of(bytes: &[u8]) -> Cid {
    let mut raw = [0u8; 32];
    for (i, b) in bytes.iter().enumerate() {
        raw[i % 32] ^= *b;
    }
    Cid(raw)
}

/// Source lines of a file under `crates/gx-engine/src`, with comment-only lines dropped.
///
/// §30's lesson, which this crate has now paid for three times: this crate's prose *discusses*
/// `cas_eq`, `apply` and every record name at length, so a scan that counted documentation would
/// report roads that do not exist.
fn code(rel: &str) -> Vec<String> {
    read_repo(rel)
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(str::to_string)
        .collect()
}

/// The lines of `pipeline.rs` from the declaration of `pub fn recover` to the end of `fn resume`.
///
/// Delimited by the *next* function declaration after `resume` rather than by a brace count: the
/// engine's functions are separated by `fn` declarations at one indentation level, and a brace
/// counter would have to know about strings and comments to be right.
fn recovery_span() -> Vec<String> {
    let lines = code("crates/gx-engine/src/pipeline.rs");
    let start = lines
        .iter()
        .position(|l| l.contains("pub fn recover("))
        .expect("`pub fn recover(` is declared in pipeline.rs");
    let after_resume = lines
        .iter()
        .skip(start)
        .position(|l| l.contains("fn apply_once("))
        .expect("`fn apply_once(` follows the recovery in pipeline.rs");
    lines[start..start + after_resume].to_vec()
}

// ---------------------------------------------------------------------------
// 1. The recovery exists, and it is not one of 43's transitions
// ---------------------------------------------------------------------------

/// 43 §7 is a **procedure**, not a transition, and the two are declared differently.
///
/// `tests/engine_shape.rs` fixes the eight entry points of 43 §3 and this hand adds none of them:
/// recovery writes `Committed` and `Aborted` records, which are T-11's and T-10a/T-10c's, from a
/// road §7 describes separately. So the probe here is that the procedure has exactly one entry and
/// one worker, and that neither is spelled as a ninth transition.
#[test]
fn the_recovery_is_one_procedure_and_not_a_ninth_transition() {
    let lines = code("crates/gx-engine/src/pipeline.rs");
    let entries = lines
        .iter()
        .filter(|l| l.contains("pub fn recover("))
        .count();
    let workers = lines.iter().filter(|l| l.contains("fn resume(")).count();
    println!("RECOVER_ENTRIES={entries} RESUME_WORKERS={workers}");
    assert_eq!(entries, 1, "43 §7 has one road into it");
    assert_eq!(workers, 1, "and one worker for §7-3");
}

// ---------------------------------------------------------------------------
// 2. 🔴 Λ4, structurally: the CAS is not reachable from the recovery
// ---------------------------------------------------------------------------

/// 🔴 **req/78 §3.2 Λ4 / M5-01 adopted (a)** (sem: SEM-gx-engine-686): the recovery never re-runs the CAS.
///
/// 43 §7-3c says "`Fingerprint₁ := adapter.precondition(now)` is recomputed … the procedure from
/// T-10a onward … is re-run from the start" (sem: SEM-gx-engine-686) and Λ4 shows in three
/// lines what that costs when the crash happened *after* a successful
/// apply: the recomputed fingerprint differs **because of the engine's own write**, T-10a folds it
/// to `Aborted(PreconditionChanged)`, and INV-S4 then keeps it out of the ledger — a changed world
/// with no record of the change.
///
/// The two instruments this probe pairs: `precondition` and `cas_eq` are absent from the recovery,
/// and both are present in `commit`. An absence with no control measures nothing (§30).
#[test]
fn the_recovery_never_re_runs_the_cas() {
    let span = recovery_span();
    let in_recovery = span
        .iter()
        .filter(|l| l.contains("cas_eq(") || l.contains(".precondition("))
        .count();
    let whole = code("crates/gx-engine/src/pipeline.rs");
    let in_crate = whole
        .iter()
        .filter(|l| l.contains("cas_eq(") || l.contains(".precondition("))
        .count();
    println!("CAS_IN_RECOVERY={in_recovery} CAS_IN_CRATE={in_crate}  (0, and >=2 in commit)");
    assert_eq!(
        in_recovery, 0,
        "Λ4: a recovery that can reach the CAS can mistake its own footprint for interference"
    );
    assert!(
        in_crate >= 2,
        "the control: T-10a's CAS is still there, in `commit` -- {in_crate}"
    );
}

/// The second question E-M5-1 added, in the source: the recovery reads `apply_started`.
///
/// 43 §7-3 branches on the ledger alone. This engine asks "was the adapter asked" first (sem:
/// SEM-gx-engine-687), and the field it asks is the one hand 4 wrote and
/// `crate::replay::StateRow` carries.
#[test]
fn the_recovery_reads_the_record_e_m5_1_added() {
    let span = recovery_span();
    let reads = span.iter().filter(|l| l.contains("apply_started")).count();
    println!("APPLY_STARTED_READS_IN_RECOVERY={reads}");
    assert!(
        reads >= 1,
        "E-M5-1's whole purpose is that a recovery can read it"
    );
}

/// Every application inside the recovery is announced first, exactly as in `commit`.
///
/// A crash *inside* the re-application must not look like a crash before it — which is the same
/// sentence that made hand 4 announce the T-10c rollback, and it holds one layer up.
#[test]
fn the_recoverys_re_application_is_announced_before_it_happens() {
    let span = recovery_span();
    let announce = span
        .iter()
        .position(|l| l.contains("EngineJournalRecord::ApplyStarted"));
    let apply = span.iter().position(|l| l.contains("self.apply_once("));
    println!("ANNOUNCE_AT={announce:?} APPLY_AT={apply:?}");
    let (announce, apply) = (
        announce.expect("the recovery announces its application"),
        apply.expect("the recovery applies through the one door"),
    );
    assert!(
        announce < apply,
        "write-ahead: the record is appended before the call, not after it"
    );
}

// ---------------------------------------------------------------------------
// 3. The vocabulary of the procedure (E-M2-23 / A-10)
// ---------------------------------------------------------------------------

/// `RECOVERY_PATHS` is the variants of `RecoveryPath`, in order.
///
/// The same shape `ERROR_KINDS`, `JOURNAL_RECORD_KINDS` and `LIFECYCLE_STATES` carry: a road added
/// without a row is a failing probe rather than a silent addition.
#[test]
fn the_declared_recovery_paths_are_the_variants_in_order() {
    let text = read_repo("crates/gx-engine/src/pipeline.rs");
    let declared: Vec<String> = text
        .lines()
        .skip_while(|l| !l.contains("pub const RECOVERY_PATHS"))
        .take_while(|l| !l.contains("];"))
        .filter_map(|l| l.trim().strip_prefix('"').map(str::to_string))
        .map(|l| l.trim_end_matches("\",").to_string())
        .collect();
    let variants: Vec<String> = text
        .lines()
        .skip_while(|l| !l.contains("pub enum RecoveryPath"))
        .take_while(|l| !l.starts_with('}'))
        .filter(|l| l.starts_with("    ") && l.trim().ends_with(','))
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(|l| l.trim().trim_end_matches(',').to_string())
        .collect();
    println!("RECOVERY_PATHS={declared:?} VARIANTS={variants:?}");
    assert_eq!(
        declared.len(),
        7,
        "43 §7-2 plus §7-3's three answers, plus R5's refusal (req/227 H-01: a recovery that will \
         not act on a journal it cannot trust, or on a ledger that has moved past the commit it is \
         being asked to finish), plus R13's two ways of closing §7-3b's window without asking an \
         adapter anything (req/244 H-03: `ClosedFromFiledReceipt` writes the record from the \
         commit receipt the critical section already filed, `ClosedFromLedgerLeaf` writes it from \
         the leaf alone when no receipt was filed and the substrate cannot be read). The number \
         moved with the enum in one commit, which is what this probe is for"
    );
    assert_eq!(declared, variants, "the list and the enum are one thing");
}

// ---------------------------------------------------------------------------
// 4. The crash binary is declared, and `cargo package` will not see it
// ---------------------------------------------------------------------------

/// 🔴 **M5-13 adopted (a)** (sem: SEM-gx-engine-688): 51 §8.1's E2E needs a real process, so a
/// second binary exists — and it is
/// gated exactly the way AC-030's is, so that `cargo build`, `cargo install` and `cargo package`
/// never see it.
#[test]
fn the_crash_probe_is_a_test_only_binary() {
    let manifest = read_repo("crates/gx-engine/Cargo.toml");
    let bins: Vec<&str> = manifest
        .lines()
        .filter(|l| l.trim_start().starts_with("name = \""))
        .collect();
    println!("MANIFEST_NAMES={bins:?}");
    assert!(
        manifest.contains("name = \"crash_probe\""),
        "51 §8.1 asks for a real binary, a real process (sem: SEM-gx-engine-689)"
    );
    let gated = manifest
        .lines()
        .filter(|l| l.contains("required-features = [\"probe-bin\"]"))
        .count();
    assert_eq!(
        gated, 2,
        "both probe binaries are behind the same feature (req/38 §5's precedent)"
    );
    assert!(
        !read_repo("crates/gx-engine/tests/bin/crash_probe.rs").is_empty(),
        "the binary named in the manifest exists"
    );
}

// ---------------------------------------------------------------------------
// 5. discipline 48 (sem: SEM-gx-engine-690) — the crashed state itself, before anything repairs it
// ---------------------------------------------------------------------------

/// 🔴 **discipline 48** (req/38 §40 M5H3-6, sem: SEM-gx-engine-691): "wherever a terminal
/// record re-supplies what an intermediate record said, always place one probe that stops at the
/// intermediate state."
///
/// The recovery's `Committed` record re-supplies everything `ApplyStarted` said, so a probe that
/// only looked at the repaired journal would pass with the intermediate record broken or missing —
/// which is precisely the state that decides whether Λ4 happens. This probe stops at the crash and
/// asserts the three faces **before** any recovery runs.
///
/// Hand 4's report predicted this exact need: "hand 5's crash window sits where a torn tail does
/// not accidentally land (mid-apply), which is exactly where this difference bites" (sem:
/// SEM-gx-engine-692).
#[test]
fn the_crashed_state_is_measured_before_anything_repairs_it() {
    let dir = scratch("recovery_intermediate");
    let marker = kill_at(&dir, "applied", "after");
    assert_eq!(marker, "applied");

    let journal = EngineJournal::open(dir.join("journal.bin")).expect("the journal survives");
    let kinds: Vec<&str> = journal
        .records()
        .iter()
        .map(EngineJournalRecord::kind)
        .collect();
    let world = std::fs::read_to_string(dir.join("world")).expect("the world survives");
    let sigma = reconstruct(journal.records());
    let row = sigma
        .transformations()
        .first()
        .expect("one transformation is in flight");

    println!(
        "AT_CRASH TAIL={kinds:?} WORLD={world:?} STATE={:?} APPLY_STARTED={:?}",
        row.state,
        row.apply_started.is_some()
    );
    assert_eq!(
        kinds.last(),
        Some(&"ApplyStarted"),
        "the last thing written before the kill is the announcement of the apply"
    );
    assert!(
        !kinds.contains(&"Committed") && !kinds.contains(&"Aborted"),
        "the section is open: {kinds:?}"
    );
    assert_eq!(row.state, Some(Lifecycle::Committing));
    assert!(row.apply_started.is_some(), "E-M5-1's record is in Sigma");
    assert_eq!(
        world, "after",
        "🔴 the world moved, and nothing has recorded it"
    );
    // The ledger is the third face, and it is empty -- this is Lambda-4's window, on disk.
    let out = probe(&["recover", &dir.display().to_string()]);
    assert_eq!(need(&out, "BEFORE_LEDGER_AGREES"), "true");
}

// ---------------------------------------------------------------------------
// 6. 🔴 Λ4 — the same crashed bytes, recovered two ways
// ---------------------------------------------------------------------------

/// 🔴 **req/78 §3.2 Λ4, as an experiment**: a recovery that ignores `ApplyStarted` misreads its own
/// footprint; this one does not.
///
/// The shim below is 43 §7-3c **as written**: "`Fingerprint₁ := adapter.precondition(now)` is
/// recomputed … the procedure from T-10a onward … is re-run from the start" (sem:
/// SEM-gx-engine-693). It runs against the crashed directory itself (whose
/// `Fingerprint₀` names that directory's world) and gets `Ok(false)` from the CAS — not because
/// anybody interfered, but because the engine's own successful `apply` changed the digest. T-10a
/// folds that to `Aborted(PreconditionChanged)`, INV-S4 keeps an aborted transformation out of the
/// ledger, and what is left is a changed world with no record: the property gx exists to sell,
/// broken by the procedure meant to protect it.
///
/// The real recovery runs against a byte-identical copy taken before either ran. Same input, two
/// answers, both printed.
#[test]
fn the_recovery_that_ignores_apply_started_reproduces_lambda_4() {
    let dir = scratch("lambda4_shim");
    let marker = kill_at(&dir, "applied", "after");
    assert_eq!(marker, "applied");
    let copy = scratch("lambda4_real");
    copy_tree(&dir, &copy);

    // --- the shim: 43 §7-3c without E-M5-1 -------------------------------------------------
    let mut journal = EngineJournal::open(dir.join("journal.bin")).expect("the journal survives");
    let sigma = reconstruct(journal.records());
    let row = sigma
        .transformations()
        .first()
        .expect("one transformation is in flight")
        .clone();
    let id = row.transformation;
    let fp0 = row
        .fp0
        .clone()
        .expect("T-2 recorded Fingerprint-0")
        .into_fingerprint()
        .expect("it rebuilds");
    // The fresh read of the world, exactly as the adapter would have taken it.
    let locator = dir.join("world").display().to_string();
    let world_now = std::fs::read(dir.join("world")).expect("the world survives");
    let fp1 =
        Fingerprint::new(SubstrateKind::Fs, locator, digest_of(&world_now)).expect("a short scope");
    let cas = fp0.cas_eq(&fp1);
    println!("SHIM_CAS={cas:?}  (Ok(false) = the engine reads its own apply as interference)");
    assert_eq!(
        cas,
        Ok(false),
        "Lambda-4's premise: the fingerprint moved, and the engine itself moved it"
    );
    // T-10a's consequence, written down.
    journal
        .append(EngineJournalRecord::Aborted {
            transformation: id,
            reason: AbortReason::PreconditionChanged,
            rollback: None,
            at: Timestamp(RECOVER_AT),
        })
        .expect("the shim can write its abort");
    let shim_sigma = reconstruct(journal.records());
    let shim_state = shim_sigma
        .state_of(&id)
        .expect("the row is still there")
        .state;
    let shim_leaves = gx_log::LedgerStore::open(dir.join("journal.bin.ledger"))
        .expect("the ledger survives")
        .log()
        .len();
    let shim_world = std::fs::read_to_string(dir.join("world")).expect("the world survives");
    println!("SHIM STATE={shim_state:?} LEDGER_LEAVES={shim_leaves} WORLD={shim_world:?}");
    assert_eq!(
        shim_state,
        Some(Lifecycle::Aborted(AbortReason::PreconditionChanged))
    );
    assert_eq!(shim_leaves, 0);
    // 🔴 The conjunction. This is the failure being reproduced on purpose.
    assert_eq!(
        shim_world, "after",
        "the shim was supposed to reproduce a change with no record"
    );

    // --- the real recovery: the same bytes, E-M5-1 in force ---------------------------------
    let out = probe(&["recover", &copy.display().to_string()]);
    let real_world = std::fs::read_to_string(copy.join("world")).expect("the copy's world");
    println!(
        "REAL {}",
        out.lines()
            .find(|l| l.starts_with("RECOVERED id="))
            .unwrap_or("")
    );
    println!(
        "REAL LEDGER_LEAVES={} WORLD={real_world:?}",
        need(&out, "LEDGER_LEAVES")
    );
    assert_eq!(need(&out, "LEDGER_LEAVES"), "1");
    assert_eq!(real_world, "after");
    assert!(
        out.contains("path=ApplyWasAnnounced") && out.contains("state=Committed"),
        "the real recovery walks 43 7-3c with E-M5-1's second question first:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// 7. 43 §7-3b — the window no adapter seam can reach
// ---------------------------------------------------------------------------

/// 43 §7-3b: "when a matching entry exists in the ledger, the commit had already completed
/// before the crash … re-issue the receipt from the existing `InclusionProof` (if unissued) and
/// append `Committed` to the journal" (sem: SEM-gx-engine-694).
///
/// # Why this one is built with a truncation rather than with a kill
///
/// The window is between `ledger.append` returning and the `Committed` record reaching the journal,
/// and **no adapter call sits in it** — 51 §8.1 names three injection points and this is not one of
/// them, because a test-only hook could only be placed inside `commit` itself. So the crash is
/// simulated the way a crash actually looks from the outside: the journal file, cut one record
/// short. `EngineJournal::open` already treats a short journal as the ordinary shape of a crash.
///
/// # 🔴 M5H4-7, measured
///
/// §41 asked hand 5 to measure "the payload and ledger_digest are unchanged; only `issued_at`
/// differs — an idempotent reconstruction, not a duplicate commit" (sem: SEM-gx-engine-695)
/// explicitly. `payload_matched` is that claim mechanically: the
/// recovery rebuilds the payload from the journal, digests it, and compares it with what the ledger
/// already holds. A rebuild that had drifted in any field would produce a different digest — and
/// `gx_log`'s key idempotency would then refuse it as `Conflict` rather than accept it (ASM-43-1),
/// so the ledger is a second instrument for the same claim.
#[test]
fn the_ledger_window_reissues_the_receipt_and_appends_nothing() {
    let dir = scratch("recovery_ledger_window");
    let run = probe(&["run", &dir.display().to_string(), "none", "after"]);
    assert_eq!(need(&run, "LEDGER_LEAVES"), "1");
    assert_eq!(need(&run, "JOURNAL_RECORDS"), "10");

    // Cut the `Committed` record off the end: the append landed, the record did not.
    let path = dir.join("journal.bin");
    let bytes = std::fs::read(&path).expect("the journal is readable");
    let boundaries = record_boundaries(&bytes);
    let last = *boundaries.last().expect("ten records have ten boundaries");
    println!(
        "TRUNCATE from={} to={last} records={}",
        bytes.len(),
        boundaries.len()
    );
    std::fs::write(&path, &bytes[..last]).expect("the journal can be cut");

    let out = probe(&["recover", &dir.display().to_string()]);
    let row = out
        .lines()
        .find(|l| l.starts_with("RECOVERED id="))
        .unwrap_or_default()
        .to_string();
    let reissued = out
        .lines()
        .find(|l| l.starts_with("REISSUED "))
        .unwrap_or_default()
        .to_string();
    println!("LEDGER_WINDOW {row}");
    println!("LEDGER_WINDOW {reissued}");
    assert!(row.contains("path=LedgerHeldTheCommit"), "{row}");
    assert!(row.contains("state=Committed"), "{row}");
    assert!(
        row.contains("payload_matched=Some(true)"),
        "🔴 PAYLOADS_EQUAL: the rebuilt payload digests to what the ledger witnessed -- {row}"
    );
    assert!(
        row.contains("appended=None"),
        "43 7-3b appends nothing to the ledger -- {row}"
    );
    assert!(
        row.contains("receipt=true"),
        "re-issued if unissued -- {row}"
    );
    assert!(
        reissued.contains(&format!("issued_at=Timestamp({RECOVER_AT})")),
        "🔴 ISSUED_AT differs: the re-issue happens at the restart's clock, not the run's \
         ({RUN_AT}) -- {reissued}"
    );
    assert_eq!(need(&out, "LEDGER_LEAVES"), "1", "still exactly one entry");
    // Nine survived the cut and the recovery wrote one: the `Committed` record 43 §7-3b asks for.
    //
    // 🔴 **R33 / `req/397` H-01** — this number was **11**, and the sentence beside it read "the
    // Committed record is back, and the re-application was announced". Two records were written
    // because the road announced an `ApplyStarted` and re-applied the delta, which is how it
    // obtained the `postcondition_fingerprint` 42 §3.10 requires and the journal has no seat for
    // (raised as **M5H5-3**).
    //
    // `req/397` H-01 measured what the re-application cost: it happens **before** the rebuilt
    // payload is compared against the leaf, so a run that was about to refuse had already written
    // to a substrate, and the sentence it refused with said "Nothing was applied". Since R33 the
    // fingerprint on this road is a *reading* of the world rather than a rewriting of it, so there
    // is no application to announce and the count is the Committed record alone.
    //
    // The assertions above are unchanged and they are the ones that matter here: the payload still
    // digests to what the ledger witnessed (`payload_matched=Some(true)`), the ledger still gains
    // nothing (`appended=None`), and the receipt is still re-issued. What moved is one record and
    // one call to a substrate.
    assert_eq!(
        need(&out, "JOURNAL_RECORDS"),
        "10",
        "the Committed record is back, and nothing was applied to announce (R33 / req/397 H-01)"
    );
    assert_eq!(need(&out, "LEDGER_AGREES"), "true");

    // 🔴 **M5 hand 8, §44 ②** — the record *sequence* (sem: SEM-gx-engine-696) the reissue
    // writes, printed rather than counted.
    //
    // §44 asks whether 43 §3's table needs a row for the road 43 §7-3b walks, and the material for
    // that judgement is the sequence beside the cell. So the journal is read back here and its
    // kinds printed. The judgement itself is the Owner's/Fable's: this hand raises it and does not
    // touch 43 (req/86 §5, M5H8-*).
    let after = std::fs::read(&path).expect("the journal is readable after the recovery");
    let kinds: Vec<&'static str> = gx_engine::replay(&after)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    println!("LEDGER_WINDOW_RECORD_KINDS n={} {kinds:?}", kinds.len());
    println!(
        "LEDGER_WINDOW_APPENDED_BY_RECOVERY {:?}",
        &kinds[kinds.len() - 2..]
    );
    assert_eq!(
        &kinds[kinds.len() - 2..],
        ["ApplyStarted", "Committed"],
        "43 §7-3b's road writes `ApplyStarted` and `Committed` and nothing else -- no \
         `CommittingStarted` (T-9 already ran before the crash), no `InverseEscrowed` (T-10b's \
         escrow survived), no `ledger.append` (the entry was already there). 43 T-11's cell \
         describes a road whose side-effect column includes `ledger.append`, and this road \
         writes T-11's journal record without walking it"
    );
}

// ---------------------------------------------------------------------------
// 8. M5H3-5 — what a restart restores, and what the recovery actually needs
// ---------------------------------------------------------------------------

/// 🔴 **M5H3-5** (req/38 §40, sem: SEM-gx-engine-697): "`Engine::open`'s state-table
/// reconstruction is decided **only after hand 5 measures "what recovery actually needs"** …
/// no type is added on a guess."
///
/// The measurement, in one probe. After a crash, `Engine::open` restores the draft phase and the
/// ledger's own file, and **nothing else**: no state rows, no `Transformation` bodies, no
/// snapshots. The recovery then completes a commit anyway, which is the evidence that the state
/// table was not needed. What it reads instead is the journal (through Sigma), the blob store (the
/// delta body the `Planned` record names) and the ledger.
///
/// The one input that is genuinely missing is a **locator**, and it only matters on the road that
/// is not taken here: see `Engine::recover` and **M5H5-2**.
#[test]
fn a_restart_restores_the_drafts_and_the_recovery_needs_no_table() {
    let dir = scratch("recovery_open_needs");
    let marker = kill_at(&dir, "applied", "after");
    assert_eq!(marker, "applied");

    let engine = Engine::open(
        dir.join("journal.bin"),
        gx_gate::Gate::unconfigured(),
        InjectedEvidence::none(),
    )
    .expect("a crashed journal opens");
    let sigma_from_journal = reconstruct(engine.journal().records());
    let id = sigma_from_journal
        .transformations()
        .first()
        .expect("one transformation")
        .transformation;
    let delta_cid = sigma_from_journal
        .state_of(&id)
        .and_then(|r| r.delta_cid)
        .expect("T-2 named the delta");

    println!(
        "REOPENED_ROWS={} REOPENED_STATE={:?} REOPENED_FP0={} REOPENED_DELTA={} \
         BLOB_PRESENT={} LEDGER_LEAVES={} JOURNAL_RECORDS={}",
        engine.transformation_ids().len(),
        engine.state(&id),
        engine.precondition_fingerprint(&id).is_some(),
        engine.planned_delta(&id).is_some(),
        engine.blobs().contains(&delta_cid),
        engine.ledger().log().len(),
        engine.journal().len()
    );
    // What `open` does not rebuild.
    //
    // 🔴 **Narrowed by DR-43-2** (`req/38` §148), and the two halves are now measured separately
    // rather than as one. M5H3-5's claim was and remains "`open` rebuilds **no bodies**, and the
    // recovery completes a commit anyway" — that is what `held_ids` and the four `is_none()`
    // assertions below say, and none of them moved. What did move is the claim this line used to
    // make by accident: `transformation_ids()` was empty because *nothing* survived, and `req/182`
    // H-02 measured what that cost a restarted `gx serve` (a `404` for a row its own journal held).
    // The Σ-shadow now answers the state, so the id is here and the body is not — which is the
    // distinction M5H3-5 was about in the first place. The old assertion is kept, one accessor
    // narrower, rather than deleted.
    assert!(
        engine.transformation_ids().is_empty(),
        "no state rows: `open` rebuilds no `Transformation` body, and M5H3-5's measurement is that          the recovery does not need one"
    );
    assert!(
        !engine.shadow().is_empty(),
        "DR-43-2: the journal's rows are reachable across the restart (req/182 H-02), which is the          same fact `state` below reports"
    );
    assert_eq!(
        engine.state(&id),
        sigma_from_journal.state_of(&id).and_then(|r| r.state),
        "the state a restarted engine answers is the journal's own fold and nothing else — no          table, no body, no second reading (req/38 §148 T6 condition ①)"
    );
    assert!(engine.precondition_fingerprint(&id).is_none());
    assert!(engine.planned_delta(&id).is_none());
    assert!(engine.transformation(&id).is_none());
    assert!(engine.precondition_snapshot(&id).is_none());
    // What it does: the draft phase, the blob bodies and the ledger's own file.
    //
    // 🔴 **M5H8-12 adopted (c)→(a)** (`req/38_ERRATA_2026-08-07.md` §45, sem: SEM-gx-engine-698),
    // verbatim:
    //
    // > **M5H8-12 adopted (c)→(a)**: first confirm with the gotcha50 detector (`git diff --stat`)
    // > that the mutation is landing → if it is, re-read the probe and re-fire it = **fix batch**
    // > (hand 4 (m)'s procedure) (sem: SEM-gx-engine-698).
    //
    // req/86 §3.3 deleted the `DraftCreated` arm from `Engine::open`'s `filter_map` and this probe
    // stayed green, even though its **name** says "restores the drafts" (sem:
    // SEM-gx-engine-699). The fix hand ran the
    // gotcha50 detector first (`MUTATION_DIFF_LINES=2`, so the mutation was landing) and then read
    // the body: every assertion above says what `open` did **not** rebuild, and the three below
    // reached for the blob store, the ledger and a child process. Nothing read the drafts. §30's
    // disease, fourth case — a probe whose name carries a claim its body never makes.
    //
    // The two lines that make the name true, and why both: `is_drafted` is "the draft phase's
    // whole observable surface" (M5-17 adopted (b), sem: SEM-gx-engine-700) and would hold if
    // the *seed* were lost, so Σ's own
    // row is compared against the journal's reconstruction as well. The seed is 41 §6's injected
    // randomness; a restart that forgot it would let the same intent be drafted twice with two
    // different seeds, which is the one thing `submit`'s idempotency (`self.drafted.contains_key`)
    // exists to refuse.
    let drafts_from_journal = sigma_from_journal.drafts();
    let drafts_in_engine = engine.sigma();
    let drafts_in_engine = drafts_in_engine.drafts();
    println!(
        "RESTORED_DRAFTS_ENGINE={drafts_in_engine:?} RESTORED_DRAFTS_JOURNAL={drafts_from_journal:?}"
    );
    assert_eq!(
        drafts_from_journal.len(),
        1,
        "the crashed run drafted exactly one intent; if this is 0 the fixture changed, not the \
         engine"
    );
    assert_eq!(
        drafts_in_engine, drafts_from_journal,
        "`Engine::open` must restore the draft phase -- the same `IntentId` and the same seed the \
         journal's `DraftCreated` record holds. This is the assertion the probe's name promised \
         and did not make until the fix hand (M5H8-12)"
    );
    assert!(
        engine.is_drafted(&drafts_from_journal[0].intent_id),
        "the restored draft is not visible through `is_drafted`, which M5-17 adopted (b) makes \
         the draft phase's whole observable surface (sem: SEM-gx-engine-701)"
    );

    assert!(
        engine.blobs().contains(&delta_cid),
        "the delta body survives a restart"
    );
    // And the recovery completes the commit from exactly those, in a fresh process.
    let out = probe(&["recover", &dir.display().to_string()]);
    assert!(out.contains("state=Committed"), "{out}");
    assert_eq!(need(&out, "LEDGER_LEAVES"), "1");
    assert_eq!(value(&out, "RECOVER_REFUSED"), None);
}

// ---------------------------------------------------------------------------
// K6 mutant-kill (req/38 §73 priority-9 items, sem: SEM-gx-engine-702; req/159 §B-1): E-M5-11's guard, both halves
// ---------------------------------------------------------------------------

/// Cut a committed journal back to the middle of its critical section: everything up to and
/// including `ApplyStarted` survives, minus whatever `keep` refuses on the way.
///
/// The stores beside the journal (`*.blobs`, `*.ledger`, …) are left as the finished run wrote
/// them, which is a shape a real crash can leave too — T-11 appends to the ledger before the
/// journal's `Committed` record lands, so "ledger ahead of journal" (sem: SEM-gx-engine-703)
/// is §7-3b's own window, not
/// a fixture's invention.
fn cut_after_apply_started(
    journal_path: &std::path::Path,
    keep: impl Fn(&EngineJournalRecord) -> bool,
) {
    let records: Vec<EngineJournalRecord> = EngineJournal::open(journal_path)
        .expect("the committed journal reads back")
        .records()
        .to_vec();
    let cut = records
        .iter()
        .position(|r| r.kind() == "ApplyStarted")
        .expect("the commit announced its apply (E-M5-1)")
        + 1;
    std::fs::remove_file(journal_path).expect("replace the journal");
    let mut torn = EngineJournal::open(journal_path).expect("a fresh journal opens");
    for record in records[..cut].iter().filter(|r| keep(r)) {
        torn.append(record.clone()).expect("re-append survives");
    }
}

/// 🔴 K6 mutant-kill (resume's E-M5-11 guard rewritten to `false`; mutants run e, `req/38`
/// §73): a crash inside a T-4e commit's critical section is recoverable.
///
/// The comment beside the guard has promised this since §41 — "refusing here would make T-4e
/// the one transition a restart could not finish" (sem: SEM-gx-engine-704) — and run e showed
/// no probe holds the code to
/// it: both rewrites of `row.fail_posture_engaged` survived. This is the half that pins the
/// promise: a `Committing` row whose pair is `(None, None)` **with the posture flag raised**
/// resumes to `Committed` rather than being refused.
#[test]
fn the_recovery_finishes_a_t4e_commit_cut_mid_section() {
    let dir = scratch("recovery_t4e_guard");
    let journal_path = dir.join("journal.bin");
    let key = signing_key();

    // A full T-4e commit (collector down, FailOpen), driven to `Committed` in one session.
    //
    // `without_inverse`, so the receipt's `inverse_delta` seat is `None` on both roads: the
    // finished run escrowed nothing, and the §7-3b rebuild below will hold nothing — the digest
    // comparison then measures the guard and not the escrow's crash window (whose honest
    // `Unavailable` fold after a cut-away `ApplyObserved` is `two_phase_escrow.rs`'s subject,
    // not this probe's).
    // 🔴 **R33 / `req/397` H-01** — the world the commit left, kept for the restart.
    //
    // This binding was `_world` and the recovery below was handed a **fresh** `CommitAdapter` at
    // `"before"`, which is a fixture saying that the substrate forgot the commit at the same
    // moment the journal was cut. No crash does that: `Engine::commit` applies the delta and
    // fsyncs before `ledger.append`, so a `Committing` row whose leaf the ledger holds sits over a
    // world that is already at the postcondition — which is exactly why R33 can *read* the
    // fingerprint instead of re-applying to obtain it.
    //
    // The old fixture only passed because the re-application put the world back where the
    // recovery needed it, i.e. the probe was insulated from the world by the very write `req/397`
    // H-01 is about. `crates/gx-engine/tests/a32_recover_road.rs` (monitoring 32) built its beds
    // the other way — `CommitAdapter::new(&world_at_restart)` — and that is the shape copied here.
    let world_at_restart;
    let id = {
        let (adapter, _counts, world) = CommitAdapter::new("before");
        let adapter = adapter.without_inverse();
        let mut engine = Engine::open(
            journal_path.clone(),
            gate(PERMIT_ALL),
            UnreachableEvidence::new("the collector is down"),
        )
        .expect("a fresh journal opens")
        .with_posture(FailPosture::FailOpen);
        engine.register_adapter(Arc::new(adapter), "k6-t4e-commit");
        let i = intent("/tmp/k6-t4e-resume.txt", "after");
        engine.submit(&i, 42, Timestamp(RUN_AT)).expect("T-1");
        let id = engine.plan(&i, Timestamp(RUN_AT)).expect("T-2");
        let state = engine
            .verify(&id, Timestamp(RUN_AT), &key, None)
            .expect("T-4e admits it, degraded");
        assert_eq!(state, Lifecycle::Admitted, "T-4e admits under FailOpen");
        engine
            .canonicalize(&id, Timestamp(RUN_AT), None)
            .expect("T-8 with enforced=false");
        engine
            .commit(&id, Timestamp(RUN_AT), &key)
            .expect("hand 6 commits a degraded admission");
        world_at_restart =
            String::from_utf8_lossy(&world.lock().expect("the world").clone()).to_string();
        id
    };

    cut_after_apply_started(&journal_path, |_| true);
    {
        let torn = EngineJournal::open(&journal_path).expect("the torn journal reads back");
        let sigma = reconstruct(torn.records());
        let row = sigma.state_of(&id).expect("the row survived the cut");
        println!(
            "T4E_ROW state={:?} verdict={:?} digest={:?} posture={}",
            row.state, row.verdict, row.verdict_digest, row.fail_posture_engaged
        );
        assert_eq!(
            row.state,
            Some(Lifecycle::Committing),
            "the section is open"
        );
        assert!(
            row.verdict.is_none() && row.verdict_digest.is_none(),
            "no gate ran, so the pair is empty"
        );
        assert!(
            row.fail_posture_engaged,
            "43 T-4e's flag is the one truth the empty pair has"
        );
    }

    println!("T4E_WORLD at_restart={world_at_restart:?}");
    let (adapter, _counts, _world) = CommitAdapter::new(&world_at_restart);
    let mut engine = Engine::open(journal_path, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("the torn journal replays");
    engine.register_adapter(Arc::new(adapter), "k6-t4e-recover");
    let out = engine
        .recover(Timestamp(RECOVER_AT), &key)
        .expect("43 §7-3 finishes what T-4e began — the degraded admission is resumable");
    println!("T4E_RECOVERED n={} state={:?}", out.len(), out[0].state);
    assert_eq!(out.len(), 1, "one transformation was in flight");
    assert_eq!(out[0].transformation, id);
    assert_eq!(
        out[0].state,
        Lifecycle::Committed,
        "the one transition a restart must be able to finish, finished"
    );
}

/// 🔴 K6 mutant-kill (resume's guard rewritten to `true`; mutants run e, `req/38` §73), the
/// other half: the pair with **no** verdict and **no** engaged posture is refused at resume,
/// for `commit`'s reason and by `commit`'s name.
///
/// No live session writes this journal — T-4e raises the very flag the guard reads — so the
/// fixture hands the recovery a torn journal whose `Verdict` record is gone: the shape a
/// truncated or tampered journal can present, and the one input that separates
/// `row.fail_posture_engaged` from `true`.
#[test]
fn the_recovery_refuses_a_committing_row_with_no_verdict_and_no_posture() {
    let dir = scratch("recovery_half_filled_guard");
    let journal_path = dir.join("journal.bin");
    let key = signing_key();

    // A normal admitted commit, driven to `Committed` in one session.
    let id = {
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        let mut engine = Engine::open(
            journal_path.clone(),
            gate(PERMIT_ALL),
            InjectedEvidence::none(),
        )
        .expect("a fresh journal opens");
        engine.register_adapter(Arc::new(adapter), "k6-halfpair-commit");
        let i = intent("/tmp/k6-halfpair.txt", "after");
        engine.submit(&i, 42, Timestamp(RUN_AT)).expect("T-1");
        let id = engine.plan(&i, Timestamp(RUN_AT)).expect("T-2");
        let state = engine
            .verify(&id, Timestamp(RUN_AT), &key, None)
            .expect("T-4a");
        assert_eq!(state, Lifecycle::Admitted);
        engine
            .canonicalize(&id, Timestamp(RUN_AT), None)
            .expect("T-8");
        engine.commit(&id, Timestamp(RUN_AT), &key).expect("T-11");
        id
    };

    // Cut to mid-section AND drop the `Verdict` record: `(None, None)` with the flag down.
    cut_after_apply_started(&journal_path, |r| {
        !matches!(r, EngineJournalRecord::Verdict { .. })
    });
    {
        let torn = EngineJournal::open(&journal_path).expect("the torn journal reads back");
        let sigma = reconstruct(torn.records());
        let row = sigma.state_of(&id).expect("the row survived the cut");
        assert_eq!(
            row.state,
            Some(Lifecycle::Committing),
            "the section is open"
        );
        assert!(
            row.verdict.is_none() && row.verdict_digest.is_none() && !row.fail_posture_engaged,
            "the half-filled pair, on disk"
        );
    }

    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut engine = Engine::open(journal_path, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("the torn journal replays");
    engine.register_adapter(Arc::new(adapter), "k6-halfpair-recover");
    let refused = engine
        .recover(Timestamp(RECOVER_AT), &key)
        .expect_err("a receipt with nothing true to put in its verdict seat must not be rebuilt");
    println!("HALF_PAIR_REFUSAL kind={}", refused.kind());
    assert!(
        matches!(refused, gx_engine::Error::Unrepresentable { .. }),
        "the refusal is E-M5-11's, by name: {refused:?}"
    );
}

/// 🔴 K6 mutant-kill (commit's E-M5-11 guard rewritten to `true`, mutants run e, `req/38`
/// §73), **as a scan** — for Λ4's reason: no input can exercise the difference.
///
/// Every in-session road to a `Canonicalized` entry seats either a full pair (T-4a/T-4c/T-5
/// write kind and digest together) or T-4e's `(None, None)` with the flag **raised**; the
/// half-filled pair with the flag down exists only on the recovery's side of the door, where
/// the probe above refuses it behaviourally. So run e caught the guard's `false` rewrite (a
/// T-4e commit fails under it) and missed the `true` one — behaviourally invisible, exactly
/// like a recovery that could reach the CAS. The scan is the honest instrument left: both
/// arms must read their own row's flag, not a constant.
#[test]
fn the_half_filled_pair_guards_read_the_posture_flag_and_not_a_constant() {
    let lines = code("crates/gx-engine/src/pipeline.rs");
    let commit_guard = lines
        .iter()
        .filter(|l| l.contains("(None, None) if entry.fail_posture_engaged"))
        .count();
    let resume_guard = lines
        .iter()
        .filter(|l| l.contains("(None, None) if row.fail_posture_engaged"))
        .count();
    println!("COMMIT_GUARD={commit_guard} RESUME_GUARD={resume_guard}");
    assert_eq!(
        commit_guard, 1,
        "commit's E-M5-11 arm is guarded by the entry's own flag, not by a constant"
    );
    // 🔴 **R8 / `req/234` H-01 (b)** — two `row.fail_posture_engaged` guards now, and the
    // second is `Engine::reissue_receipt`'s. It rebuilds a terminal row's payload from the same Σ
    // fields `resume` uses, so it meets the same half-filled pair and has to refuse it for the same
    // reason: a receipt that says a change was allowed and cannot say by what is E-M5-11's
    // `Unrepresentable`. What differs is the answer — `resume` raises, and `reissue_receipt`
    // returns `Reissued::NoMaterial`, because it is a diagnosis-and-remedy road rather than a
    // start-up road and taking a project offline over one unrebuildable receipt would be the
    // `req/227` M-03 mistake (narrowing the only exit a damaged project has). The scan counts both
    // so that a constant substituted into **either** shows up here.
    assert_eq!(
        resume_guard, 2,
        "and resume's and reissue_receipt's by the row's own flag, not by a constant"
    );
}
