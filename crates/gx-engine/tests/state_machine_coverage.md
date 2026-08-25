# state_machine_coverage -- 43 §3's 21 transitions -> the probe that walks each one (51 §14) (sem: SEM-gx-engine-862)

> What kind of artefact this is: a **CC-generated artefact** (51 §14, verbatim: "`tests/state_machine_coverage.md` (a CC-generated artefact, the same kind of thing as conformance-report.md)"). Written by hand, checked by a machine. The check is two probes in
> `crates/gx-engine/tests/state_machine_coverage.rs`, measuring (1) that 43 §3's transition-id set agrees with this table's rows and (2) that the probes this table names actually exist.
> 51 §14, verbatim: "if even one is unwalked, the M's completion condition is not met". (sem: SEM-gx-engine-862)
>
> **The denominator is 21** (**M5H2-7, adopted (a)**, req/38 §39). 51 §14's closing sentence lists nineteen ids, but
> `T-4e` and `T-13` are missing from it -- **the two hardest to reach** -- exactly the pair a hand counting from the sentence would drop.
> So neither this table nor the lint counts from the sentence: both parse 43 §3's table. (sem: SEM-gx-engine-862)

## 21/21

| transition | from -> to (43 §3) | probe walked (`suite::fn`) | hand | <!-- sem: SEM-gx-engine-863 -->
|---|---|---|---|
| T-1 | (start) → Draft | `ac_030::ac_030_the_same_intent_gets_the_same_two_ids_in_three_calls` | 2 |
| T-2 | Draft → Candidate | `ac_031::ac_031_plan_fixes_the_delta_the_fingerprint_and_the_state` | 2 |
| T-3 | Candidate → Verifying | `ac_032::ac_032_admit_goes_to_admitted` `ac_035::ac_035_the_verifying_state_is_covered_by_the_journal_rather_than_by_a_return` | 2 / 4 |
| T-4a | Verifying → Admitted | `ac_032::ac_032_admit_goes_to_admitted` | 2 |
| T-4b | Verifying → Denied | `ac_032::ac_032_deny_goes_to_denied` | 2 |
| T-4c | Verifying → Escalated | `ac_032::ac_032_escalate_goes_to_escalated` | 2 |
| T-4d | Verifying → Aborted(VerifierUnavailable) | `ac_032::t_4d_an_unreachable_collector_aborts_fail_closed` | 2 |
| T-4e | Verifying → Admitted (degraded, enforced=false) | `ac_032::t_4e_an_unreachable_collector_degrades_under_an_explicit_fail_open` `commit_protocol::a_degraded_admission_commits_and_its_receipt_says_no_gate_ran` | 2 / 6 | <!-- sem: SEM-gx-engine-864 -->
| T-5 | Escalated → Admitted (human ruling) | `ac_071::ac_071_an_approved_escalation_becomes_admitted_and_commits` | 6 | <!-- sem: SEM-gx-engine-864 -->
| T-5b | Escalated → Denied (human ruling) | `ac_072::ac_072_a_rejected_escalation_is_denied_and_goes_no_further` | 6 | <!-- sem: SEM-gx-engine-864 -->
| T-6 | {Candidate,Verifying,Escalated} → Aborted(Expired) | `ac_045::ac_045_an_untouched_candidate_is_expired_by_the_reaper` `ac_045::ac_045_an_escalated_transformation_expires_on_the_escalation_deadline` | 6 |
| T-7 | {Candidate,Verifying,Admitted,Canonicalized,Escalated} → Aborted(OwnerCancelled) | `ac_073::ac_073_every_state_before_committing_can_be_cancelled` | 6 |
| T-8 | Admitted → Canonicalized | `ac_033::ac_033_an_admitted_transformation_canonicalises` | 2 |
| T-8r | Denied → Canonicalized(RecordOnly) | `ac_033::t_8r_a_denied_transformation_canonicalises_under_record_only` | 2 |
| T-9 | Canonicalized → Committing | `commit_protocol::the_critical_section_journals_before_each_side_effect` | 4 |
| T-10a | Committing → Aborted(PreconditionChanged) | `ac_034::ac_034_a_concurrent_mutation_aborts_the_commit_without_applying` | 4 |
| T-10b | Committing → Committing (escrow, internal) | `ac_038::ac_038_the_escrowed_inverse_body_is_retrievable` `supersede::e_m5_9_a_commit_with_no_constructible_inverse_records_the_absence` | 4 / 6 | <!-- sem: SEM-gx-engine-865 -->
| T-10c | Committing → Aborted(ApplyFailed) | `ac_038::ac_038_a_failed_apply_rolls_back_and_the_outcome_is_journalled` | 4 |
| T-11 | Committing → Committed | `ac_034::ac_034_an_untouched_world_commits_and_applies_once` | 4 |
| T-12 | Committed(T_o) → Superseded | `ac_040::ac_040_an_undo_is_a_new_gated_transformation_that_supersedes_the_original` | 6 |
| T-13 | {8 states} → Aborted(InternalError) | `state_machine_coverage::t_13_a_miswired_adapter_is_an_internal_error_and_not_a_precondition_change` `ac_032::e_m5_5_the_gates_bottom_aborts_whatever_the_enforcement_mode_says` | 7 / 2 |

## What this table does not say (the denominator beneath the denominator) (sem: SEM-gx-engine-866)

100% is 100% **of transition ids** -- it is not 100% of a transition's **arms**. What 51 §14 gates on is the former; the latter is not counted. Written down without hiding where the difference actually is. (sem: SEM-gx-engine-866)

1. **T-13's from-set is 8 states, and v0.1 has a road into 2 of them** (`Verifying` = the gate's bottom;
   `Committing` = `cas_eq`'s `Err`). No producer in v0.1 writes `InternalError` from the remaining 6 states.
   `state_machine_coverage::t_13_is_entered_from_two_states_and_v0_1_has_no_road_into_the_other_six`
   measures those two and prints that it is not 8. (sem: SEM-gx-engine-866)
2. **43's from-set for T-7 is 6 states, and 4 were measured.** `Draft` has no id, so no record can be written for it
   (**E-M5-14** = the erratum that excludes `Draft` from T-7's from-set, req/38 §43). `Verifying` is unreachable
   as a resting state because `verify` runs T-3 and T-4a through e in one call
   (`ac_073::ac_073_verifying_is_never_a_resting_state` measures the shape of that). (sem: SEM-gx-engine-866)
3. **Of T-6's from-set of 3, `Verifying` is unreachable for the same reason.** `Candidate`'s and `Escalated`'s two deadlines
   (ASM-12) are measured separately. (sem: SEM-gx-engine-866)
4. **T-10b has 2 arms** (`Some` = escrow, `None` = **E-M5-9**'s record of absence). Both have a probe. (sem: SEM-gx-engine-866)
5. **T-12 is drawn only on the live path.** Recovery does not redraw the edge (**M5H6-6**) --
   `supersede::a_recovery_does_not_draw_the_edge_the_crash_interrupted` keeps measuring the window. (sem: SEM-gx-engine-866)
6. **This is not line coverage.** `cargo llvm-cov`'s number is hand 8's window; this table says only
   "every transition is walked by at least one test" (51 §14, verbatim). (sem: SEM-gx-engine-866)

## 43 §7's recovery is not a transition (sem: SEM-gx-engine-866)

`Engine::recover` is 43 §7's procedure, not a row of 43 §3 (**M5H5-1**). The records it writes belong to T-11 /
T-10a / T-10c and carry no independent id. Therefore this table having no row for it is not an omission --
`crash_recovery::the_recovery_is_one_procedure_and_not_a_ninth_transition` measures that. (sem: SEM-gx-engine-866)
