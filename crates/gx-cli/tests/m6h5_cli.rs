// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 The three CLI-side claims **M6 hand 5** owes, measured through the binary.
//!
//! The hand's centre of gravity is 44 §2's HTTP surface, and three of its rulings land on 44 §1's
//! side as well. All three are red before the implementation and are therefore the part of this hand
//! that is red-first in the T-27 sense (the HTTP surface is not: a test client cannot call a router
//! that does not compile, and §1 of the report says so rather than dressing it up).
//!
//! * **E-M6-13** (req/38 §51 M6H4-1 adopted (a); sem: SEM-gx-cli-1670) — "add exit **2** to cancel/escalation's state-machine refusal
//!   (§1.2's column is an excerpt; applying the rest of M6-25's reading to the remaining 2 verbs). The implementation is the first hand from hand 5 onward to take this step." Hand 4
//!   raised it and left both verbs on **1**, where 44 §1.2 writes "insufficient permission, or an unexecutable state"
//!   under §1.4's "error (invalid input / internal error / adapter error)". A cancel refused because the row
//!   passed `Committing` is none of those three.
//! * **M6H4-7** (req/38 §51 adopted (a); sem: SEM-gx-cli-1692) — `.gx/receipts/<TID>.<kind>.json`, `kind ∈ {verdict, ruling,
//!   commit}`. One transformation issues up to three receipts (ASM-14: T-4a/b/c's verdict receipt,
//!   T-5/T-5b's ruling, T-11's commit) and a store keyed on the transformation alone could hold one
//!   of them. "migration backward-compatibility is unnecessary=not yet distributed" (sem: SEM-gx-cli-1671).
//! * **M6H3-2** (req/38 §50 adopted (a), implementation window=hand 5; sem: SEM-gx-cli-1672) — `Engine::admit_proof` / `Engine::deny_reasons`, of
//!   which 44 §1.2's `gx verify` stdout is one of the two consumers: "both 44 §1.2's verify stdout verbatim
//!   and the HTTP problem `detail` are consumers." Hand 3 printed a **digest** and raised the ticket.

mod support;

use support::{deny_writable_pack, pipeline, pipeline_named, run, DENIED_FRAGMENT};

// ---------------------------------------------------------------------------
// E-M6-13 — the two state-machine refusals 44 §1.2 leaves on 1
// ---------------------------------------------------------------------------

/// 🔴 **E-M6-13, first verb** — `gx cancel` on a committed transformation exits **2**.
///
/// 43 T-7's from-set stops at `Canonicalized`/`Escalated` and its guard is "before reaching `Committing`", so
/// a committed row is the state machine saying no. 44 §1.4's 2 is "refused (denied)" (sem: SEM-gx-cli-1673) and discipline 52
/// (E-M6-2) reserved the number for exactly that — the reservation exists so that a script can tell
/// "I typed something wrong" (sem: SEM-gx-cli-1674) from "the machine refused the operation", and folding this into 1
/// gives the two one face (M4H4-2).
///
/// AC-073's second half is the same Given — "for a T already at `Committing` or later (including Committed) ... refused as an invalid
/// operation and does not change existing state" (sem: SEM-gx-cli-1675) — and this measures the **status** that refusal carries.
#[test]
fn e_m6_13_cancel_of_a_committed_transformation_is_a_refusal_and_not_an_error() {
    let fixture = pipeline("m6h5_cancel_exit2", "before\n");
    let tid = fixture.commit_one("after\n");
    let before = fixture.target_contents();

    let cancelled = run(fixture.gx().args(["cancel", &tid]));
    println!(
        "E_M6_13_CANCEL exit={} stdout={} stderr={}",
        cancelled.code,
        cancelled.stdout.trim(),
        cancelled.stderr.trim()
    );
    assert_eq!(
        cancelled.code, 2,
        "E-M6-13 (req/38 §51 M6H4-1 adopted (a)): \"add exit 2 to cancel/escalation's state-machine refusal\" (sem: SEM-gx-cli-1676). \
         44 §1.2 writes 1 for this case and §1.4's 2 is \"refused (denied)\"; hand 4 raised the \
         divergence and this hand implements the ruling. stderr: {}",
        cancelled.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        before,
        "AC-073: \"does not change existing state\" (sem: SEM-gx-cli-1677) — a refusal changes nothing on the substrate"
    );
}

/// 🔴 **E-M6-13, second verb** — `gx escalation approve` on a row that is not `Escalated` exits 2.
///
/// INV-S6 is why the refusal exists at all: "`Escalated` does not auto-transition to `Admitted`/`Denied` without going through
/// T-5/T-5b's signed human-ruling receipt" (sem: SEM-gx-cli-1678), and its mirror is that a ruling may not be recorded
/// against a transformation nobody escalated. 44 §1.2 gives that 1 and 44 §1.4 gives "refused" 2.
#[test]
fn e_m6_13_a_ruling_on_a_transformation_nobody_escalated_is_a_refusal() {
    let fixture = pipeline("m6h5_escalation_exit2", "before\n");
    let tid = fixture.planned_one("after\n");
    let ruler = fixture.another_key();

    let ruled = run(fixture.gx().args([
        "escalation",
        "approve",
        &tid,
        "--reason",
        "the row is a Candidate and nobody asked me",
        "--actor-key",
        &ruler,
    ]));
    println!(
        "E_M6_13_ESCALATION exit={} stderr={}",
        ruled.code,
        ruled.stderr.trim()
    );
    assert_eq!(
        ruled.code, 2,
        "E-M6-13: a ruling on a row that is not `Escalated` is 43 T-5's guard refusing, which is \
         44 §1.4's 2 and not its 1. stderr: {}",
        ruled.stderr
    );
}

/// A **usage** error on the same verb still exits 1, which is the whole point of the reservation.
///
/// discipline 52's negative half (sem: SEM-gx-cli-1679), one verb further out than `exit_map.rs` measures it: the number 2 has to
/// mean "the state machine refused" and nothing else, so an id that is not an id must not take it.
/// Without this probe an implementation that returned 2 from every refusal in the module would pass
/// the two above.
#[test]
fn the_reservation_holds_a_malformed_id_is_still_one() {
    let fixture = pipeline("m6h5_escalation_usage", "before\n");
    // A planned transformation first, so that the project has a `.gx/` and the refusal under test is
    // the id's rather than "there is no project here" (sem: SEM-gx-cli-1680) — which is 44 §1.4's **6** and would make this
    // probe pass for the wrong reason.
    let _ = fixture.planned_one("after\n");
    let ruler = fixture.another_key();
    let refused = run(fixture.gx().args([
        "escalation",
        "approve",
        "not-a-gx1-id",
        "--reason",
        "x",
        "--actor-key",
        &ruler,
    ]));
    println!("USAGE_STILL_ONE exit={}", refused.code);
    assert_eq!(
        refused.code, 1,
        "discipline 52: \"usage error→exit 1 'invalid input'\" (sem: SEM-gx-cli-1681). E-M6-13 moves the **state machine's** refusal to \
         2 and must not move this one with it"
    );
}

// ---------------------------------------------------------------------------
// M6H4-7 — `.gx/receipts/<TID>.<kind>.json`
// ---------------------------------------------------------------------------

/// The file names inside a project's `.gx/receipts/`, sorted.
fn receipt_files(project: &std::path::Path) -> Vec<String> {
    let dir = project.join(".gx").join("receipts");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// 🔴 **M6H4-7** — a committed transformation leaves **two** receipts, and each says which kind it is.
///
/// req/38 §51: "migrate `.gx/receipts/` to `<TID>.<kind>.json` (kind ∈ verdict/ruling/commit) (update both hand 3's
/// writer and hand 2's reader · migration backward-compatibility is unnecessary=not yet distributed)" (sem: SEM-gx-cli-1682).
///
/// The count is the argument. ASM-14 issues a `VerdictReceipt` for **every** verdict and 43 T-11
/// issues a `CommitReceipt`, so an admitted-then-committed transformation has two receipts that are
/// not each other — different `receipt_kind`, different payload, both signed. A store keyed on the
/// transformation alone had one slot for them, and whichever was written second was the one an
/// operator could read. That is not a naming preference: it is 42 §3.10's two kinds losing one.
#[test]
fn m6h4_7_a_committed_transformation_files_a_verdict_receipt_and_a_commit_receipt() {
    let fixture = pipeline("m6h5_receipt_kinds", "before\n");
    let tid = fixture.commit_one("after\n");
    let stem = tid.replace(':', "_");
    let names = receipt_files(&fixture.project);
    println!("RECEIPT_STORE_FILES={names:?}");

    assert!(
        names.contains(&format!("{stem}.commit.json")),
        "M6H4-7: T-11's `CommitReceipt` is filed under `<TID>.commit.json`; found {names:?}"
    );
    assert!(
        names.contains(&format!("{stem}.verdict.json")),
        "M6H4-7: ASM-14's `VerdictReceipt` is filed under `<TID>.verdict.json`. 42 §3.10 gives the \
         two kinds two shapes and a store with one slot per transformation kept whichever was \
         written last; found {names:?}"
    );
    assert!(
        !names.contains(&format!("{stem}.json")),
        "the untagged name is the one M6H4-7 migrates **away** from, and \"migration backward-compatibility is unnecessary=\
         not yet distributed\" (sem: SEM-gx-cli-1683) means nothing writes it any more; found {names:?}"
    );
}

/// 🔴 **M6H4-7's third kind** — a human ruling is filed as `ruling`, beside the verdict it overturns.
///
/// 43 T-5's side effect is "a human-ruling receipt (signed) is **appended to the provenance chain**" (sem: SEM-gx-cli-1684) and M5H4-6 made the
/// engine hold a **list** for exactly that reason: an escalated transformation ends with two verdict
/// receipts signed by **two different keys** — the engine's over T-4c, the ruler's over T-5. A store
/// that filed both under one name would make the second erase the first, which is the fact INV-S6
/// exists to keep: "who allowed this" is a separate signature from "what was decided" (sem: SEM-gx-cli-1685).
#[test]
fn m6h4_7_a_human_ruling_is_filed_under_its_own_kind() {
    let fixture = pipeline("m6h5_receipt_ruling", &support::oversized_before());
    let tid = fixture.planned_one("after\n");
    let verified = run(fixture.gx().args(["verify", &tid]));
    assert_eq!(
        verified.code, 4,
        "E-M3-4: a change whose inverse cannot be built escalates. stderr: {}",
        verified.stderr
    );

    let ruler = fixture.another_key();
    let ruled = run(fixture.gx().args([
        "escalation",
        "approve",
        &tid,
        "--reason",
        "the inverse is over the escrow ceiling and I accept that",
        "--actor-key",
        &ruler,
    ]));
    assert_eq!(ruled.code, 0, "AC-071: {}", ruled.stderr);

    let stem = tid.replace(':', "_");
    let names = receipt_files(&fixture.project);
    println!("RECEIPT_STORE_FILES_AFTER_RULING={names:?}");
    assert!(
        names.contains(&format!("{stem}.verdict.json")),
        "T-4c's own receipt survives the ruling: {names:?}"
    );
    assert!(
        names.contains(&format!("{stem}.ruling.json")),
        "M6H4-7: T-5's receipt is `<TID>.ruling.json`. Two receipts, two keys, two files — a single \
         slot would let the ruler's signature erase the engine's: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// M6H3-2 — `gx verify` says why
// ---------------------------------------------------------------------------

/// 🔴 **M6H3-2** — a denied `gx verify` prints 44 §1.2's `reasons`, not a digest.
///
/// > 44 §1.2's stdout for `gx verify` is `{"kind":"Deny","reasons":[Reason]}`
///
/// Hand 3 printed `kind` and the verdict's digest and raised the ticket in as many words: "an
/// operator asking 'why was I denied' gets a digest" (sem: SEM-gx-cli-1686). A digest is a proof that the value was
/// hashed and says nothing about the value; each [`gx_gate::Reason`] carries a `code` from the
/// declared vocabulary, a bounded `message` and a `ReasonSource`, which is the answer to "why" (sem: SEM-gx-cli-1687).
#[test]
fn m6h3_2_a_denied_verify_prints_the_reasons_and_not_only_a_digest() {
    let fixture = pipeline_named(
        "m6h5_deny_reasons",
        "before\n",
        &format!("{DENIED_FRAGMENT}.txt"),
    );
    let tid = fixture.planned_one("after\n");
    let verified = run(fixture
        .gx()
        .args(["verify", &tid])
        .arg("--policy")
        .arg(deny_writable_pack()));
    println!(
        "M6H3_2_DENY exit={} json={}",
        verified.code,
        verified.stdout.trim()
    );
    assert_eq!(
        verified.code, 2,
        "44 §1.2: \"2=Deny\" (sem: SEM-gx-cli-1688): {}",
        verified.stderr
    );

    let json = verified.json();
    let reasons = json["reasons"]
        .as_array()
        .unwrap_or_else(|| panic!("M6H3-2: 44 §1.2 writes `reasons` for a Deny; got {json}"));
    assert!(
        !reasons.is_empty(),
        "gx-gate refuses an empty `Verdict::Deny` at construction, so a denied verdict has at least \
         one reason and an empty array here would mean the surface dropped them"
    );
    assert!(
        reasons[0]["code"].is_string(),
        "each Reason carries a code from `REASON_CODES` — the vocabulary a machine branches on: {json}"
    );
    assert!(
        reasons[0]["message"].is_string(),
        "and a message, which is the half a person reads: {json}"
    );
}

/// 🔴 **M6H3-2's other arm** — an admitted `gx verify` prints the proof, and it is not empty.
///
/// 44 §1.2: `{"kind":"Admit","proof":AdmitProof}`. 42 §3.8's proof is five fields and the two that
/// carry the evaluation are `policy_decisions` and `invariant_results`; the shipped pack has a
/// permit rule, so an admitted transformation carries at least one **decision** and a `proof` whose
/// every list was empty would mean the surface was rendering a `Default` rather than the value T-4a
/// produced.
///
/// 🔴 `invariant_results` **is** empty, and that is a fact about this deployment rather than a gap
/// in the proof: `Session::open` builds the gate with `Gate::with_policies` and registers no
/// invariant, so FR-027's "`Verdict` is the composition of policy+invariant" (sem: SEM-gx-cli-1689) composes with an empty half today.
/// `gx policy lint` reports the same emptiness as a warning (M6-21's third consumer). Asserted as
/// zero rather than ignored, so that the day an invariant is registered this line is the reminder to
/// say so out loud.
#[test]
fn m6h3_2_an_admitted_verify_prints_the_proof() {
    let fixture = pipeline("m6h5_admit_proof", "before\n");
    let tid = fixture.planned_one("after\n");
    let verified = run(fixture.gx().args(["verify", &tid]));
    println!(
        "M6H3_2_ADMIT exit={} json={}",
        verified.code,
        verified.stdout.trim()
    );
    assert_eq!(
        verified.code, 0,
        "44 §1.2: \"0=Admit\" (sem: SEM-gx-cli-1690): {}",
        verified.stderr
    );

    let json = verified.json();
    let proof = &json["proof"];
    assert!(
        proof.is_object(),
        "M6H3-2: 44 §1.2 writes `proof` for an Admit; got {json}"
    );
    let decisions = proof["policy_decisions"]
        .as_array()
        .unwrap_or_else(|| panic!("42 §3.8's `policy_decisions`: {proof}"));
    println!(
        "ADMIT_PROOF policies={} invariants={} evidence={}",
        decisions.len(),
        proof["invariant_results"].as_array().map_or(0, Vec::len),
        proof["evidence_digests"].as_array().map_or(0, Vec::len),
    );
    assert!(
        !decisions.is_empty(),
        "the shipped pack has a permit rule and the gate records which policies answered, so an \
         admitted transformation has at least one decision; an empty proof would be a `Default` \
         wearing the value's name: {proof}"
    );
    assert!(
        decisions[0]["policy_id"].is_string(),
        "each decision names the policy that made it — the half of \"why\" (sem: SEM-gx-cli-1691) a machine branches on: \
         {proof}"
    );
    assert_eq!(
        proof["invariant_results"].as_array().map_or(1, Vec::len),
        0,
        "the CLI registers no invariants (see this test's note); if that changes, this line is \
         where the change gets said out loud"
    );
}
