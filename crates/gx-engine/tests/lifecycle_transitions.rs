//! The shapes M5 hand 6 owes: T-5/T-5b, T-6, T-7, T-12, and the two errata that make them
//! writable (**E-M5-9**, **E-M5-11**).
//!
//! Spec: 43 §3 for the five transitions, 43 §4 for the two enforcement axes, 43 §5 for what an
//! undo is and is not, 43 §8 for the waiting that AC-045's second clause is about, 42 §3.12/§3.13
//! for the records, 34 AC-036/037/040/041/044/045/071/072/073 for the judgement.
//!
//! # Why this suite reads source instead of running the engine
//!
//! Every probe below is a **structural** claim — 「this shape exists」 / 「it exists in exactly one
//! place」 — and each one was written before the shape did, in the RED commit this hand opened with
//! (T-27). A behavioural probe cannot be written before the API it calls exists, because it does
//! not compile, and a RED commit whose suites fail to build hides every other RED in the same
//! binary behind one `error[E0599]`. So the behavioural half lives in `tests/ac_036.rs` ..
//! `tests/ac_073.rs` and arrived with the implementation; **the mutation battery of the report's
//! §4 is what says those are load-bearing**, and this suite is what says the shapes are where the
//! report claims they are.
//!
//! The counting probes are the more interesting half. 「T-12 is fired in one place」 and 「the
//! supersede index has one home」 are claims a running test cannot make: it can see that the edge
//! *was* drawn, never that there is no second road that would also draw it.

use gx_engine::InverseStatus;

const PIPELINE: &str = include_str!("../src/pipeline.rs");
const STORE: &str = include_str!("../src/store.rs");
const REPLAY: &str = include_str!("../src/replay.rs");

/// Lines of a source file with the doc comments and the ordinary comments taken out (§30).
///
/// This crate's prose names every transition it has not implemented yet, at length, so a grep for
/// 「T-12」 over the whole file finds the module documentation of hand 2. Only code counts.
fn code(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// The three entry points 43 §3 still owes, and the reaper M5-10 採(a)+(b) adds
// ---------------------------------------------------------------------------

/// **T-5/T-5b, T-7, T-12**: the last three of the eight entry points exist.
///
/// req/78 §6.2 fixes the eight (`submit`/`plan`/`verify`/`canonicalize`/`commit`/`undo`/`cancel`/
/// `escalation`), and hands 2 and 4 wrote five. Hand 1's `tests/engine_shape.rs` asserted the
/// other three were **absent rather than stubbed**; this is the same measurement with the
/// expectation inverted, which is what makes 「hand 6 happened」 a number.
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

/// **T-6 / M5-10 採(a)+(b)**: TTL is both a lazy evaluation and an explicit sweep.
///
/// > **M5-10 採(a)+(b) 併用**: lazy TTL 評価(liveness)+明示的 `reap(now)` API(掃き)
///
/// (a) is what makes INV-L1/L2 hold without a resident process — v0.1 has none, `gx serve` is M6
/// (req/78 N-01) — and (b) is what sweeps a transformation nobody ever touches again. Both, or
/// the liveness claim is about transformations somebody happened to look at.
#[test]
fn the_ttl_is_both_a_lazy_check_and_an_explicit_reaper() {
    let code = code(PIPELINE);
    assert!(
        code.contains("pub fn reap("),
        "M5-10 採(b): the explicit sweep is an API"
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
        "M5-10 採(a): every entry point that reads or advances a row evaluates the deadline first; \
         found {lazy}"
    );
}

// ---------------------------------------------------------------------------
// E-M5-9 -- the escrowed inverse's CID becomes optional
// ---------------------------------------------------------------------------

/// 🔴 **E-M5-9** (§40, M5H3-2 採(a), implementation window = this hand).
///
/// > `InverseEscrowed.inverse_cid` の `Option` 化(42 §3.13 erratum・51 §8.1 優先条項の 4 度目)を、
/// > **手 6 の escalation 承認が経路を現実にする同 turn で実装**する
///
/// The path is the one T-5 opens: **E-M3-4** escalates a transformation whose `invert` answers
/// `None`, and until a human could approve one, nothing without a constructible inverse ever
/// reached `Committing`. Hand 4 wrote `None => None` there and journalled **nothing**, which is
/// 42 §3.12's `Unavailable` having no spelling in the log. With T-5 in place the case is live, and
/// a commit that recorded no `InverseEscrowed` at all would leave 「we asked and there is none」
/// and 「we never asked」 wearing one face (§32 M4H4-2).
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

/// **T-12 / M5-09 採(a) / M5-16 採(a)**: one edge, one index update, one status move.
///
/// > **M5-16 採(a)**: `Consumed{by}` は T-12 発火と同時(M5-09 と 1 箇所)
///
/// Three facts change when an inverse commits — `T_o.status`, `superseded_by`, and the escrow's
/// `InverseStatus` — and the ruling puts all three at one point. Counted rather than asserted from
/// a run, for 則 2's reason: a second road that also drew the edge would leave every behavioural
/// probe green.
#[test]
fn t_12_is_fired_in_exactly_one_place() {
    let code = code(PIPELINE);
    let edges = code.matches("EngineJournalRecord::Superseded {").count();
    // 🔴 Written, not merely named. `undo` **reads** the status to refuse a second undo of one
    // commit, and the first draft of this probe counted that read as a second writer — the needle
    // said 「the name appears」 where the claim is 「the value is assigned」. §30's disease in the
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
        "M5-16 採(a): the status moves where the edge is drawn and nowhere else"
    );
    assert!(
        read >= 1,
        "42 §3.12's `Consumed` has to be read somewhere, or the state it records constrains nothing"
    );
}

/// **M5-09 採(a)**: 「supersedes 索引は store.rs」.
///
/// The ruling names the module. A hand that put the index in `pipeline.rs` beside the state table
/// would have satisfied every behaviour and none of the ruling, so the placement is the probe.
#[test]
fn the_supersede_index_lives_where_m5_09_puts_it() {
    assert!(
        code(STORE).contains("pub struct SupersedeIndex"),
        "M5-09 採(a): 「supersedes 索引は store.rs」"
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
/// §41 rules it and dates it: 「`VerdictReceipt` は**手 6 の T-5/T-5b 実装と同 turn で T-4a/b/c 分も
/// 実装**する。それまで「**v0.1 に実在する receipt は `CommitReceipt` のみ**(ASM-14 の 2 種のうち 1 種は
/// 未実装)」を本 § が明文化」. 42 §3.10 says a verdict receipt is issued 「全`Verdict`＝Admit/Deny/
/// Escalateで発行、43 T-4a/T-4b/T-4c」, and 43 T-5's side effect is 「人間裁定receipt（署名済み）を
/// provenance鎖に追記」 — so five transitions issue one, and this hand is where the kind stops being
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
/// AC-071 逐語: 「発行されるreceipt trail（journal/Receiptメタデータ）に`Evidence(HumanDecision)`
/// （decision=Admit, reason, 裁定者actor）が含まれることを確認する」, and AC-072 asks the same of a
/// rejection. **E-M2-3** retired `Evidence(HumanDecision)` as a variant — 「43 T-5's 「人間裁定receipt
/// （署名済み）」 はreceipt」 — so the three facts have to live in the journal record and the signed
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
        "AC-071 asks for 裁定者actor"
    );
}

// ---------------------------------------------------------------------------
// The vocabularies that must not move
// ---------------------------------------------------------------------------

/// The journal vocabulary stays at thirteen: this hand adds **no** record.
///
/// T-5/T-5b write `HumanDecision`, T-6 and T-7 write `Aborted`, T-12 writes `Superseded` — all
/// four already exist (hand 1 declared the whole of 42 §3.13 plus E-M5-1's and hand 4's). A
/// fourteenth here would be a lane ruling on 42 §3.13, which 52 契約 forbids.
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
    assert_eq!(kinds, 13, "hand 5 left thirteen and this hand adds none");
}

/// 42 §3.12's four statuses are all reachable **except** `Expired`, and that is DR-9's doing.
///
/// `Available` is T-10b's, `Consumed` is T-12's (this hand), `Unavailable` is E-M5-9's (this hand).
/// `Expired` needs `retained_until` to be enforced, which DR-9 puts in the commercial tier and
/// req/78 N-06 keeps out of v0.1 — so it is named and never written, and this probe is what keeps
/// that a decision rather than an oversight.
#[test]
fn three_of_the_four_inverse_statuses_are_written_and_the_fourth_is_dr_9s() {
    let code = code(PIPELINE);
    for status in [
        "InverseStatus::Available",
        "InverseStatus::Consumed",
        "InverseStatus::Unavailable",
    ] {
        assert!(code.contains(status), "{status} has no producer");
    }
    assert!(
        !code.contains("InverseStatus::Expired"),
        "DR-9 / req/78 N-06: v0.1 does not enforce `retained_until`"
    );
    println!("INVERSE_STATUS_KINDS={:?}", InverseStatus::ALL_KINDS);
    assert_eq!(InverseStatus::ALL_KINDS.len(), 4);
}
