//! 43 §7's recovery, from the source and from the crashes themselves.
//!
//! Spec: 43 §7 (the write-ahead rule and the three-step procedure), 51 §8.1 (the three injection
//! points), 34 AC-043, req/78 §3.2 Λ4 (the counter-example E-M5-1 closes), req/38 §37 M5-01 採(a),
//! §40 M5H3-5, §41 M5H4-7.
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
//! that has already changed. That second outcome is 「適用されたのに記録が無い」 — the thing gx exists
//! to make impossible — and it is produced here on purpose so that the difference is measured rather
//! than argued.

mod support;

use gx_core::{AbortReason, Cid, Fingerprint, SubstrateKind, Timestamp};
use gx_engine::{
    reconstruct, Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle,
};

use support::{copy_tree, kill_at, need, probe, read_repo, record_boundaries, scratch, value};

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

/// 🔴 **req/78 §3.2 Λ4 / M5-01 採(a)**: the recovery never re-runs the CAS.
///
/// 43 §7-3c says 「`Fingerprint₁ := adapter.precondition(now)`を再計算し…T-10a以降の手順…を最初から
/// 再実行」 and Λ4 shows in three lines what that costs when the crash happened *after* a successful
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
/// 43 §7-3 branches on the ledger alone. This engine asks 「was the adapter asked」 first, and the
/// field it asks is the one hand 4 wrote and `crate::replay::StateRow` carries.
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
    assert_eq!(declared.len(), 4, "43 §7-2 plus §7-3's three answers");
    assert_eq!(declared, variants, "the list and the enum are one thing");
}

// ---------------------------------------------------------------------------
// 4. The crash binary is declared, and `cargo package` will not see it
// ---------------------------------------------------------------------------

/// 🔴 **M5-13 採(a)**: 51 §8.1's E2E needs a real process, so a second binary exists — and it is
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
        "51 §8.1 asks for 実バイナリ・実プロセス"
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
// 5. 規律48 — the crashed state itself, before anything repairs it
// ---------------------------------------------------------------------------

/// 🔴 **規律48** (req/38 §40 M5H3-6): 「終端 record が中間 record の情報を再供給する経路では、中間
/// 状態で止まる probe を必ず 1 本置く」.
///
/// The recovery's `Committed` record re-supplies everything `ApplyStarted` said, so a probe that
/// only looked at the repaired journal would pass with the intermediate record broken or missing —
/// which is precisely the state that decides whether Λ4 happens. This probe stops at the crash and
/// asserts the three faces **before** any recovery runs.
///
/// Hand 4's report predicted this exact need: 「手 5 の crash 窓は torn tail が偶然当たらない位置
/// (apply の途中)なので、そこでこの差が効く」.
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
/// The shim below is 43 §7-3c **as written**: 「`Fingerprint₁ := adapter.precondition(now)`を再計算
/// し…T-10a以降の手順…を最初から再実行」. It runs against the crashed directory itself (whose
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

/// 43 §7-3b: 「ledgerに該当entryが存在する場合 → commitはクラッシュ前に完了していた…既存の
/// `InclusionProof`からreceiptを（未発行なら）再発行し、journalへ`Committed`を追記」.
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
/// §41 asked hand 5 to measure 「payload/ledger_digest 不変・`issued_at` のみ異なる=冪等な再構成で
/// あって二重 commit ではない」 explicitly. `payload_matched` is that claim mechanically: the
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
    // Nine survived the cut and the recovery wrote two: its own `ApplyStarted` and the `Committed`
    // record 43 §7-3b asks for. The re-application is what supplies the postcondition fingerprint
    // 42 §3.10 requires and the journal has no seat for -- raised as **M5H5-3**.
    assert_eq!(
        need(&out, "JOURNAL_RECORDS"),
        "11",
        "the Committed record is back, and the re-application was announced"
    );
    assert_eq!(need(&out, "LEDGER_AGREES"), "true");

    // 🔴 **M5 hand 8, §44 ②** — the record *列* the reissue writes, printed rather than counted.
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

/// 🔴 **M5H3-5** (req/38 §40): 「`Engine::open` の状態表再構成は**手 5 が「復旧に何が要るか」を実測
/// してから**決める…見込みで型を増やさない」.
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
    assert!(engine.transformation_ids().is_empty(), "no state rows");
    assert_eq!(engine.state(&id), None);
    assert!(engine.precondition_fingerprint(&id).is_none());
    assert!(engine.planned_delta(&id).is_none());
    assert!(engine.transformation(&id).is_none());
    assert!(engine.precondition_snapshot(&id).is_none());
    // What it does: the draft phase, the blob bodies and the ledger's own file.
    //
    // 🔴 **M5H8-12 採(c)→(a)** (`req/38_ERRATA_2026-08-07.md` §45), verbatim:
    //
    // > **M5H8-12 採(c)→(a)**: まず gotcha50 の検出(`git diff --stat`)で変異が当たっている事を
    // > 確認→当たっていれば probe を読み直して撃ち直し=**fix 批**(手 4 (m) の手順)。
    //
    // req/86 §3.3 deleted the `DraftCreated` arm from `Engine::open`'s `filter_map` and this probe
    // stayed green, even though its **name** says 「restores the drafts」. The fix hand ran the
    // gotcha50 detector first (`MUTATION_DIFF_LINES=2`, so the mutation was landing) and then read
    // the body: every assertion above says what `open` did **not** rebuild, and the three below
    // reached for the blob store, the ledger and a child process. Nothing read the drafts. §30's
    // disease, fourth case — a probe whose name carries a claim its body never makes.
    //
    // The two lines that make the name true, and why both: `is_drafted` is 「the draft phase's
    // whole observable surface」 (M5-17 採(b)) and would hold if the *seed* were lost, so Σ's own
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
        "the restored draft is not visible through `is_drafted`, which M5-17 採(b) makes the \
         draft phase's whole observable surface"
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
