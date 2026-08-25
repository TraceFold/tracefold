// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The shapes M5 hand 6 owes: T-5/T-5b, T-6, T-7, T-12, and the two errata that make them
//! writable (**E-M5-9**, **E-M5-11**).
//!
//! Spec: 43 §3 for the five transitions, 43 §4 for the two enforcement axes, 43 §5 for what an
//! undo is and is not, 43 §8 for the waiting that AC-045's second clause is about, 42 §3.12/§3.13
//! for the records, 34 AC-036/037/040/041/044/045/071/072/073 for the judgement.
//!
//! # Why this suite reads source instead of running the engine
//!
//! Every probe below is a **structural** claim -- "this shape exists" / "it exists in exactly one
//! place" (sem: SEM-gx-engine-816) -- and each one was written before the shape did, in the RED commit this hand opened with
//! (T-27). A behavioural probe cannot be written before the API it calls exists, because it does
//! not compile, and a RED commit whose suites fail to build hides every other RED in the same
//! binary behind one `error[E0599]`. So the behavioural half lives in `tests/ac_036.rs` ..
//! `tests/ac_073.rs` and arrived with the implementation; **the mutation battery of the report's
//! §4 is what says those are load-bearing**, and this suite is what says the shapes are where the
//! report claims they are.
//!
//! The counting probes are the more interesting half. "T-12 is fired in one place" and "the
//! supersede index has one home" (sem: SEM-gx-engine-817) are claims a running test cannot make: it can see that the edge
//! *was* drawn, never that there is no second road that would also draw it.

use gx_engine::InverseStatus;

const PIPELINE: &str = include_str!("../src/pipeline.rs");
const STORE: &str = include_str!("../src/store.rs");
const REPLAY: &str = include_str!("../src/replay.rs");

/// Lines of a source file with the doc comments and the ordinary comments taken out (§30).
///
/// This crate's prose names every transition it has not implemented yet, at length, so a grep for
/// "T-12" (sem: SEM-gx-engine-818) over the whole file finds the module documentation of hand 2. Only code counts.
fn code(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// The three entry points 43 §3 still owes, and the reaper M5-10 adopted (a)+(b) adds (sem: SEM-gx-engine-819)
// ---------------------------------------------------------------------------

/// **T-5/T-5b, T-7, T-12**: the last three of the eight entry points exist.
///
/// req/78 §6.2 fixes the eight (`submit`/`plan`/`verify`/`canonicalize`/`commit`/`undo`/`cancel`/
/// `escalation`), and hands 2 and 4 wrote five. Hand 1's `tests/engine_shape.rs` asserted the
/// other three were **absent rather than stubbed**; this is the same measurement with the
/// expectation inverted, which is what makes "hand 6 happened" (sem: SEM-gx-engine-820) a number.
#[test]
fn the_last_three_entry_points_of_req_78_6_2_exist() {
    let code = code(PIPELINE);
    let found: Vec<&str> = ["pub fn escalation(", "pub fn cancel(", "pub fn undo("]
        .into_iter()
        .filter(|needle| code.contains(needle))
        .collect();
    println!("HAND6_ENTRY_POINTS={found:?}");
    assert_eq!(
        found.len(),
        3,
        "43 T-5/T-5b, T-7 and T-12 need entry points; found {found:?}"
    );
    let total = code
        .lines()
        .filter(|l| {
            l.trim_start().starts_with("pub fn ")
                && [
                    "submit(",
                    "plan(",
                    "verify(",
                    "canonicalize(",
                    "commit(",
                    "undo(",
                    "cancel(",
                    "escalation(",
                ]
                .iter()
                .any(|n| l.contains(n))
        })
        .count();
    println!("ENTRY_POINTS_TOTAL={total}");
    assert_eq!(total, 8, "req/78 §6.2 fixes the number at eight");
}

/// **T-6 / M5-10 adopted (a)+(b)**: TTL is both a lazy evaluation and an explicit sweep. (sem: SEM-gx-engine-821)
///
/// > **M5-10 adopted (a)+(b), used together**: lazy TTL evaluation (liveness) + an explicit
/// > `reap(now)` API (the sweep) (sem: SEM-gx-engine-821)
///
/// (a) is what makes INV-L1/L2 hold without a resident process -- v0.1 has none, `gx serve` is M6
/// (req/78 N-01) -- and (b) is what sweeps a transformation nobody ever touches again. Both, or
/// the liveness claim is about transformations somebody happened to look at.
#[test]
fn the_ttl_is_both_a_lazy_check_and_an_explicit_reaper() {
    let code = code(PIPELINE);
    assert!(
        code.contains("pub fn reap("),
        "M5-10 adopted (b): the explicit sweep is an API (sem: SEM-gx-engine-822)"
    );
    assert!(
        code.contains("verify_ttl") && code.contains("escalation_ttl"),
        "43 T-6 has two deadlines and 33 NFR-028 gives each a default"
    );
    let lazy = code.matches("self.expire_if_due(").count();
    println!(
        "TTL_LAZY_CALL_SITES={lazy}  REAPER={}",
        code.contains("pub fn reap(")
    );
    assert!(
        lazy >= 4,
        "M5-10 adopted (a): every entry point that reads or advances a row evaluates the deadline first \
         (sem: SEM-gx-engine-823); found {lazy}"
    );
}

// ---------------------------------------------------------------------------
// E-M5-9 -- the escrowed inverse's CID becomes optional
// ---------------------------------------------------------------------------

/// 🔴 **E-M5-9** (§40, M5H3-2 adopted (a), implementation window = this hand) (sem: SEM-gx-engine-824).
///
/// > Making `InverseEscrowed.inverse_cid` an `Option` (the 42 §3.13 erratum, the fourth
/// > invocation of 51 §8.1's priority clause) is **implemented in the same turn that hand 6's
/// > escalation approval makes the path real** (sem: SEM-gx-engine-824)
///
/// The path is the one T-5 opens: **E-M3-4** escalates a transformation whose `invert` answers
/// `None`, and until a human could approve one, nothing without a constructible inverse ever
/// reached `Committing`. Hand 4 wrote `None => None` there and journalled **nothing**, which is
/// 42 §3.12's `Unavailable` having no spelling in the log. With T-5 in place the case is live, and
/// a commit that recorded no `InverseEscrowed` at all would leave "we asked and there is none"
/// and "we never asked" (sem: SEM-gx-engine-825) wearing one face (§32 M4H4-2).
#[test]
fn e_m5_9_the_escrowed_inverse_cid_is_optional_in_the_journal() {
    let variant = STORE
        .split("InverseEscrowed {")
        .nth(1)
        .expect("store.rs declares the record")
        .split("},")
        .next()
        .expect("split always yields one");
    println!("INVERSE_ESCROWED_FIELDS={variant:?}");
    assert!(
        variant.contains("inverse_cid: Option<Cid>"),
        "E-M5-9: 42 §3.12's `Unavailable` needs a journal spelling"
    );
    assert!(
        code(REPLAY).contains("InverseStatus::Unavailable"),
        "a reconstruction that cannot rebuild `Unavailable` drops the fact the record now carries"
    );
}

// ---------------------------------------------------------------------------
// T-12 -- the supersede edge, and the one place it is drawn
// ---------------------------------------------------------------------------

/// **T-12 / M5-09 adopted (a) / M5-16 adopted (a)**: one edge, one index update, one status move. (sem: SEM-gx-engine-826)
///
/// > **M5-16 adopted (a)**: `Consumed{by}` is written at the same instant T-12 fires (one site,
/// > together with M5-09) (sem: SEM-gx-engine-826)
///
/// Three facts change when an inverse commits -- `T_o.status`, `superseded_by`, and the escrow's
/// `InverseStatus` -- and the ruling puts all three at one point. Counted rather than asserted from
/// a run, for Rule 2's reason (sem: SEM-gx-engine-827): a second road that also drew the edge would leave every behavioural
/// probe green.
#[test]
fn t_12_is_fired_in_exactly_one_place() {
    let code = code(PIPELINE);
    let edges = code.matches("EngineJournalRecord::Superseded {").count();
    // 🔴 Written, not merely named. `undo` **reads** the status to refuse a second undo of one
    // commit, and the first draft of this probe counted that read as a second writer -- the needle
    // said "the name appears" where the claim is "the value is assigned". §30's disease in the (sem: SEM-gx-engine-828)
    // instrument, fourth sighting in this milestone.
    let assigned = code
        .lines()
        .filter(|l| l.contains("= InverseStatus::Consumed"))
        .count();
    let read = code.matches("InverseStatus::Consumed").count() - assigned;
    println!("SUPERSEDE_JOURNAL_SITES={edges}  CONSUMED_WRITES={assigned}  CONSUMED_READS={read}");
    assert_eq!(edges, 1, "43 T-12's journal cell is written in one place");
    assert_eq!(
        assigned, 1,
        "M5-16 adopted (a): the status moves where the edge is drawn and nowhere else (sem: SEM-gx-engine-829)"
    );
    assert!(
        read >= 1,
        "42 §3.12's `Consumed` has to be read somewhere, or the state it records constrains nothing"
    );
}

/// **M5-09 adopted (a)**: "the supersedes index is store.rs" (sem: SEM-gx-engine-830).
///
/// The ruling names the module. A hand that put the index in `pipeline.rs` beside the state table
/// would have satisfied every behaviour and none of the ruling, so the placement is the probe.
#[test]
fn the_supersede_index_lives_where_m5_09_puts_it() {
    assert!(
        code(STORE).contains("pub struct SupersedeIndex"),
        "M5-09 adopted (a): \"the supersedes index is store.rs\" (sem: SEM-gx-engine-831)"
    );
    assert!(
        !code(PIPELINE).contains("struct SupersedeIndex"),
        "one home, not two"
    );
}

// ---------------------------------------------------------------------------
// M5H4-6 -- ASM-14's second receipt kind
// ---------------------------------------------------------------------------

/// 🔴 **M5H4-6**: the engine issues `VerdictReceipt`s as well as `CommitReceipt`s.
///
/// §41 rules it and dates it: "`VerdictReceipt` is **implemented for T-4a/b/c's share too, in the
/// same turn as hand 6's T-5/T-5b implementation**. Until then this § makes explicit that **the
/// only receipt that actually exists in v0.1 is `CommitReceipt`** (one of ASM-14's two kinds is
/// unimplemented)." 42 §3.10 says a verdict receipt is issued "for every `Verdict` -- Admit/Deny/
/// Escalate -- at 43 T-4a/T-4b/T-4c", and 43 T-5's side effect is "append a human-ruling receipt
/// (signed) to the provenance chain" (sem: SEM-gx-engine-832) -- so five transitions issue one, and this hand is where the kind stops being
/// a type nobody constructs.
#[test]
fn asm_14_has_two_kinds_and_both_are_issued() {
    let code = code(PIPELINE);
    let verdict = code.matches("ReceiptKind::VerdictReceipt").count();
    let commit = code.matches("ReceiptKind::CommitReceipt").count();
    println!("VERDICT_RECEIPT_SITES={verdict}  COMMIT_RECEIPT_SITES={commit}");
    assert!(
        verdict >= 1,
        "M5H4-6: ASM-14's second kind has no producer until this hand"
    );
    assert!(commit >= 1, "hand 4's kind is still issued");
}

// ---------------------------------------------------------------------------
// AC-071 / AC-072 -- what the journal has to carry about a human ruling
// ---------------------------------------------------------------------------

/// **AC-071/072**: the `HumanDecision` record names the reason and the actor.
///
/// AC-071, verbatim: "confirm that the receipt trail issued (journal/Receipt metadata) includes
/// `Evidence(HumanDecision)` (decision=Admit, reason, the ruling actor)", and AC-072 asks the same of a
/// rejection. **E-M2-3** retired `Evidence(HumanDecision)` as a variant -- "43 T-5's 'human-ruling
/// receipt (signed)' is a receipt" (sem: SEM-gx-engine-833) -- so the three facts have to live in the journal record and the signed
/// receipt instead. 42 §3.13's row for `HumanDecision` is `{transformation, kind, at}`: no reason
/// and no actor. The record gains both, and the report raises the divergence, which is the shape
/// **M5H2-1 / E-M5-7** took for `Verdict` and **M5H4-2** for `Aborted`.
#[test]
fn the_human_decision_record_carries_the_reason_and_the_ruler() {
    let variant = STORE
        .split("HumanDecision {")
        .nth(1)
        .expect("store.rs declares the record")
        .split("},")
        .next()
        .expect("split always yields one");
    println!("HUMAN_DECISION_FIELDS={variant:?}");
    assert!(
        variant.contains("reason: String"),
        "AC-071 asks for the reason; 42 §3.13's row has no seat for it"
    );
    assert!(
        variant.contains("actor: Actor"),
        "AC-071 asks for the ruling actor (sem: SEM-gx-engine-834)"
    );
}

// ---------------------------------------------------------------------------
// The vocabularies that must not move
// ---------------------------------------------------------------------------

/// The journal vocabulary was thirteen when this hand closed, and is fifteen since v0.3-a.
///
/// 🔴 v0.3-a (`req/38` §98 ruling 1): two-phase escrow added `ApplyObserved` / `InverseCompleted`
/// -- a ruled vocabulary change (the 52-contract road this probe guards was taken, not bypassed) (sem: SEM-gx-engine-835), so
/// the pin below moved 13 → 15 in the same window as the enum. The probe's job is unchanged: a
/// record added *without* a ruling still fails it.
///
/// T-5/T-5b write `HumanDecision`, T-6 and T-7 write `Aborted`, T-12 writes `Superseded` -- all
/// four already exist (hand 1 declared the whole of 42 §3.13 plus E-M5-1's and hand 4's). A
/// fourteenth here would be a lane ruling on 42 §3.13, which the 52 contract forbids (sem: SEM-gx-engine-836).
#[test]
fn no_journal_record_is_added_by_this_hand() {
    let kinds = STORE
        .split("pub const JOURNAL_RECORD_KINDS")
        .nth(1)
        .expect("store.rs declares the list")
        .split("];")
        .next()
        .expect("split always yields one")
        .matches("    \"")
        .count();
    println!("JOURNAL_RECORD_KINDS={kinds}");
    assert_eq!(
        kinds, 15,
        "thirteen through v0.2, plus two-phase escrow's ruled pair (req/38 §98 ruling 1) (sem: SEM-gx-engine-837)"
    );
}

/// 42 §3.12's statuses are all reachable **except** `Expired`, and that is DR-9's doing.
///
/// `Available` is T-10b's, `Consumed` is T-12's (this hand), `Unavailable` is E-M5-9's (this hand).
/// `Expired` needs `retained_until` to be enforced, which DR-9 puts in the commercial tier and
/// req/78 N-06 keeps out of v0.1 -- so it is named and never written, and this probe is what keeps
/// that a decision rather than an oversight.
#[test]
fn three_of_the_four_inverse_statuses_are_written_and_the_fourth_is_dr_9s() {
    let code = code(PIPELINE);
    for status in [
        "InverseStatus::Available",
        "InverseStatus::Consumed",
        "InverseStatus::Unavailable",
        // v0.3-a (req/38 §98 ruling 1) (sem: SEM-gx-engine-838): the partial escrow of two-phase escrow, written at T-10b
        // for a declared do-result pair and settled before T-11 (or by recovery).
        "InverseStatus::Pending",
    ] {
        assert!(code.contains(status), "{status} has no producer");
    }
    // 🔴 **DR-46-26 narrows this from an absence to a predicate, and the file already had the
    // shape.** DR-9's claim is that v0.1 never **writes** `Expired`, and until this lane the word
    // appeared nowhere at all, so its absence was a sufficient check. The rebuild's
    // status-to-verdict projection (`Engine::rebuilt_attest`, 43 §7-3b) now **reads** every word of
    // the vocabulary in an exhaustive `match` -- which is the property the codebase asks for
    // everywhere else ("No `_` arm", `InverseStatus::kind`), and a `_` arm here would silently give
    // an eighth word the `True` answer some day.
    //
    // So the same distinction this file already draws for `BodyMissing` two assertions above --
    // produced by a **read** accessor, never written into a row -- is drawn for `Expired`, and it
    // is drawn as a **stronger** claim rather than a weaker one: DR-9 is now checked against the
    // places a status can be written (an `EscrowRow` literal, an assignment to `row.status`) by
    // name, where before it was a substring's absence.
    for writer in [
        "status: InverseStatus::Expired",
        "row.status = InverseStatus::Expired",
        "InverseStatus::Expired,\n",
    ] {
        assert!(
            !code.contains(writer),
            "DR-9 / req/78 N-06: v0.1 does not enforce `retained_until`, so nothing writes             `Expired` into a row -- found {writer:?}"
        );
    }
    // 🔴 **R35 / `req/470` L-03** — and the one reader must not fold it in with the `True` words.
    //
    // The audit's point was narrow and correct: the exhaustive `match` exists so that an eighth
    // word cannot be given `True` by default, and it was giving the **seventh** word `True` by
    // exactly that means — listed in an or-pattern beside `Available`, `Consumed`, `Pending` and
    // `BodyMissing`, and absent from the doc's own "Where each value actually is". Now it has its
    // own arm and the arm refuses. This asserts the arm is separate, so that folding it back in
    // is a red test rather than a quiet re-merge.
    let folded =
        code.contains("| InverseStatus::Expired\n") || code.contains("InverseStatus::Expired |");
    assert!(
        !folded,
        "DR-9 / req/470 L-03: `Expired` must not share an arm with the words that answer `True`. \
         v0.1 cannot reach it, and the day a writer is added the folded form signs \
         `reversibility: true` over an escrow retention has already dropped"
    );
    let mentions = code.matches("InverseStatus::Expired").count();
    println!("INVERSE_STATUS_EXPIRED_MENTIONS={mentions}");
    assert_eq!(
        mentions, 1,
        "`Expired` is read in exactly one place (the rebuild's status-to-verdict projection) and             written in none; a second mention is a second reader or a writer, and both need saying"
    );
    // 🔴 **R8 / `req/234` B-5** — and a sixth that is produced by a **read** rather than
    // written into a row. `InverseStatus::BodyMissing` is what `Engine::inverse_status` answers
    // when the escrow row names an inverse and the blob store does not hold it, so it appears in
    // `pipeline.rs` exactly once (in that accessor) and never in the arm that inserts an
    // `EscrowRow`. That asymmetry is the point: a value recorded on the disk would be a fact about
    // a directory that can change after the recording.
    //
    // 🔴 **DR-46-26** — two occurrences now, and both are reads: the accessor above, and the
    // rebuild's status-to-verdict projection, which matches the vocabulary exhaustively. The count
    // moves and the **claim does not**, so the claim is asserted directly beside it: no occurrence
    // is a write.
    for writer in [
        "status: InverseStatus::BodyMissing",
        "row.status = InverseStatus::BodyMissing",
    ] {
        assert!(
            !code.contains(writer),
            "`BodyMissing` would be a fact recorded on disk about a directory that can change             afterwards (req/234 B-5) -- found {writer:?}"
        );
    }
    assert_eq!(
        code.matches("InverseStatus::BodyMissing").count(),
        2,
        "`BodyMissing` is answered by the read accessor, read once more by the rebuild's             projection, and written nowhere (req/234 B-5, DR-46-26)"
    );
    // 🔴 **DR-46-13 / §237-5, in DR-46-24(A)'s erratum batch** — a seventh word with no writer,
    // for the second time in this vocabulary and for a different reason than `Expired`'s.
    //
    // `Expired` has no writer because DR-9 puts deadline enforcement in the commercial tier: a
    // policy decision. `Undetermined` had no writer because of a **coordinate**, and the coordinate
    // was asserted here rather than described: `SubstrateAdapter::invert` returned
    // `Result<Option<PlannedDelta>>`, so C-25's third answer (`Reversibility::Unknown`, which
    // gx-adapter-mcp does compute) had nowhere to travel. Widening that declaration was the next
    // lane's first move and `req/441` §5 carried it.
    //
    // 🔴 **DR-46-26 did it.** The paragraph above is kept in its own tense (no-delete): it is the
    // record of what was true between D24 and this lane, and of why the block was a coordinate
    // rather than a memory. The two assertions below are its inverse — the same two facts, asserted
    // in the direction that says the producer exists.
    // 🔴 **DR-46-26** — and here is the trait change, so this assertion turns over. D24 wrote
    // the absence deliberately ("the day a hand adds a producer it has to say why"); the producer
    // exists now and this is the day. It is asserted by **arm** rather than by mere presence, and
    // by **one writer** rather than by one mention: `Undetermined` is written in exactly one place,
    // the `None` arm of T-10b's escrow, and a second writer elsewhere would be a second answer to
    // "who decides nobody found out". The other occurrence is the rebuild's status-to-verdict
    // projection, which reads the word and writes nothing -- the same reader/writer split this file
    // draws for `BodyMissing` and (since DR-46-26) for `Expired`.
    assert_eq!(
        code.matches("status: InverseStatus::Undetermined").count()
            + code.matches("row.status = InverseStatus::Undetermined").count()
            + code
                .matches("Reversibility::Unknown => InverseStatus::Undetermined")
                .count(),
        1,
        "DR-46-26: `Undetermined` has exactly one writer -- T-10b's escrow, where C-25's verdict          arrives from the adapter -- and a second one is a second decision-maker"
    );
    let undetermined_mentions = code.matches("InverseStatus::Undetermined").count();
    println!("INVERSE_STATUS_UNDETERMINED_MENTIONS={undetermined_mentions}");
    assert_eq!(
        undetermined_mentions, 2,
        "one writer (T-10b's escrow arm) and one reader (the rebuild's projection); a third          occurrence is one or the other and both need saying"
    );
    assert!(
        code.contains("Reversibility::Unknown => InverseStatus::Undetermined"),
        "DR-46-26: the writer is the arm that maps C-25's third value, not an unrelated line that          happens to name the word"
    );
    let trait_decl = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../gx-substrate/src/adapter.rs"),
    )
    .expect("gx-substrate declares the trait");
    // 🔴 **DR-46-26** — the coordinate D24 named, now asserted in its post-erratum form.
    // E-DR4626-1 widened this declaration and `gx-substrate/tests/adapter_spec.rs` holds it against
    // 41 §4's frozen text on both sides; here the engine asserts the same string for its own
    // reason, which is that the writer two assertions above exists **because** of it.
    assert!(
        trait_decl
            .contains("fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome>;"),
        "E-DR4626-1's declaration moved; the producer asserted above may no longer be writable"
    );
    assert!(
        !trait_decl
            .contains("fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>>;"),
        "the pre-DR-46-26 declaration is back, which would leave the writer above unreachable"
    );
    println!("INVERSE_STATUS_KINDS={:?}", InverseStatus::ALL_KINDS);
    assert_eq!(InverseStatus::ALL_KINDS.len(), 7);
}

// ---------------------------------------------------------------------------
// K6 mutant-kill scans (mutants run e, req/38 §73): the equivalents, pinned
// ---------------------------------------------------------------------------

/// 🔴 K6 mutant-kill (`supersede_after_commit`'s guard `||`s, staging pipeline.rs:3038:51 /
/// :3046:17 / :3048:17 `|| -> &&`, mutants run e), **as a scan** -- for Λ4's reason: no
/// reachable input exercises the difference.
///
/// The loop iterates `T_u.parents`, and the only in-session producer of a non-empty parents
/// list is `undo` (E-M5-13: "the one producer of a non-empty list is `undo`" (sem: SEM-gx-engine-839)). An undo exists
/// only after its original's escrow row passed `undo`'s own refusals -- `Consumed`, `Pending`
/// and `Unavailable` are refused by name, and `T_u.delta` *is* the escrowed inverse -- while
/// between that plan and `T_u`'s commit the original can neither expire (T-6 reads
/// `Candidate`/`Verifying`/`Escalated` deadlines only), change subject (the id is
/// content-addressed, 42 §1.3), nor be superseded by anything but `T_u` itself. So every
/// evaluation of the guard sees CID-match ∧ `Available` ∧ `Committed` ∧ same-subject ∧
/// un-superseded -- the one valuation where `||` and `&&` agree. Run e corroborates the
/// reading: the rewrites that flip *reachable* behaviour on the same lines (3038:32 `!=`→`==`,
/// 3038:54 `delete !`, 3046:52 `!=`→`==`) were all caught. The scan pins the disjunctions so
/// the three equivalent mutants die in the next run instead of resurfacing as noise.
#[test]
fn the_supersede_guard_keeps_its_disjunctions() {
    let pipeline = code(PIPELINE);
    for needle in [
        "if row.inverse_cid != Some(delta_cid) || !matches!(row.status, InverseStatus::Available)",
        "|| original.transformation.subject != subject",
        "|| self.supersedes.superseded_by(&parent).is_some()",
    ] {
        let hits = pipeline.matches(needle).count();
        println!("SUPERSEDE_GUARD_HITS={hits} needle={needle:?}");
        assert_eq!(
            hits, 1,
            "T-12's guard reads its conditions disjunctively -- any one failing refuses the \
             edge (43 T-12: trigger, guard and idempotency are separate clauses)"
        );
    }
}

/// 🔴 K6 mutant-kill (`InjectedEvidence::none`, staging pipeline.rs:344:9 `-> Default`,
/// mutants run e), **as a scan** -- the mutant is fully equivalent by construction.
///
/// `InjectedEvidence` derives `Default`, and its only field is a `Vec`, whose default *is*
/// `Vec::new()`: the rewrite to `Default::default()` denotes the same value on every call, so
/// no behavioural probe can exist (this is stronger than M7's reachable-space equivalence --
/// the two bodies are one function). The scan pins the literal so 44 §1.2's "when omitted" stays
/// spelled as the empty list it means (req/29 §4: do not let skip and pass wear the same face) (sem: SEM-gx-engine-840), and the
/// mutant dies in the next run.
#[test]
fn injected_evidence_none_answers_the_empty_list_by_construction() {
    let pipeline = code(PIPELINE);
    let hits = pipeline.matches("evidence: Vec::new(),").count();
    println!("NONE_LITERAL_HITS={hits}");
    assert_eq!(
        hits, 1,
        "`none()` constructs the empty evidence list explicitly, in one place"
    );
}

/// 🔴 K6 mutant-kill (replay's header boundary and `EngineJournal::open`'s two follow-ups,
/// staging replay.rs:109:30 `> -> >=`, store.rs:849:48 `> -> >=`, store.rs:854:12 `delete !`,
/// mutants run e), **as a scan** -- three refusals no input can observe.
///
/// * replay:109 -- with exactly `LENGTH_BYTES` left, every arm below the header read also
///   breaks before touching `records`/`good`/`at` (`length == 0` breaks, over-ceiling breaks,
///   and `end = at+4+length > len` breaks for any `length >= 1`), so `>` and `>=` compute the
///   same function of every byte string. Run e agrees: `<` and `==` here were caught, only
///   `>=` survived.
/// * store:849 -- `good + torn == total` is `replay`'s own arithmetic (`torn = total - good`),
///   so `torn == 0` ⟹ `set_len(good)` is a no-op and the branch's only residue is an extra
///   fsync: K5's class ("whether fsync happened is a layer observable only by cutting power",
///   req/38 §73) (sem: SEM-gx-engine-841), where run e likewise caught `<` and `==` and missed only the always-true `>=`.
/// * store:854 -- which open fsyncs the parent directory is durability sequencing on the same
///   K5 layer (and a no-op off unix); the flag flip is unobservable to any test that cannot
///   cut power between two syscalls.
///
/// The scans pin all three boundaries so the equivalents die in the next run.
#[test]
fn the_replay_and_open_boundaries_stay_strict() {
    let replay_code = code(REPLAY);
    let header = replay_code
        .matches("if at + LENGTH_BYTES > bytes.len() {")
        .count();
    let store_code = code(STORE);
    // 🔴 **R6 / `req/229` M-01** — the guard gained a second clause in this commit. A journal that
    // this project declared chained and that arrives with no marker is read as legacy, whose walk
    // stops after one record and calls the other 98% a tail; the audit measured `gx serve` cutting
    // 5,722 bytes to 93 on the way to refusing to start. The truncation is therefore skipped for a
    // downgrade exactly as DR-43-9 (c-3) skips it for a chain break, and this pin moves with it.
    // 🔴 **R30 / `req/372` M-02** — the needle gained a third conjunct. The property this arm is
    // about is unchanged and is now strictly stronger: the truncation runs only when there is a
    // torn tail to remove **and** the file is not a downgrade **and** it was not written by a gx
    // whose framing this build does not know. That last one is the twenty-ninth audit's finding
    // with the roles reversed -- this build eating a newer one's history -- and it is closed in
    // the same line, because there is only one line that cuts.
    let torn = store_code
        .matches("if replayed.recovery().torn_tail_bytes > 0 && !downgraded && !from_a_newer_gx {")
        .count();
    let fresh = store_code.matches("if !existed {").count();
    println!("BOUNDARY_HITS header={header} torn={torn} fresh={fresh}");
    assert_eq!(
        header, 2,
        "🔴 **R5 / DR-43-9** — a header needs strictly more bytes than remain to be torn, in \
         **both** walks of the same framing. `replay` decodes and `walk_links` only verifies the \
         chain (`EngineJournal::prefix_intact` runs the second on every write and every read, \
         where a CBOR decode per record is not affordable), and the two must stop at exactly the \
         same byte or the shape they agree on is not the same file. The count moved from 1 to 2 \
         with the second walk, in the same commit, which is what this scan is for"
    );
    assert_eq!(
        torn, 1,
        "the truncation runs only when there is a torn tail to remove"
    );
    assert_eq!(
        fresh, 1,
        "the parent directory is synced for a journal that did not exist"
    );
}
