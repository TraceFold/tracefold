# state_machine_coverage — 43 §3 の 21 遷移 → それを踏む probe(51 §14)

> 生成物の性質: **CC 生成物**(51 §14 逐語「`tests/state_machine_coverage.md`（CC生成物、
> conformance-report.mdと同様の性質）」)。手で書き、機械が検査する。検査は
> `crates/gx-engine/tests/state_machine_coverage.rs` の 2 probe で、
> ①43 §3 の遷移 ID 集合とこの表の行が一致する事 ②この表が名指す probe が実在する事、を測る。
> 51 §14 逐語「1件でも未踏の場合、当該Mの完了条件を満たさない」。
>
> **分母は 21**(**M5H2-7 採(a)**・req/38 §39)。51 §14 の末尾文は 19 ID を並べるが、そこから
> `T-4e` と `T-13` が落ちている——**最も踏みにくい 2 本**であり、文から数えた手が落とす所そのものである。
> ∴ この表も lint も文からは数えない: 43 §3 の表を parse する。

## 21/21

| 遷移 | from → to(43 §3) | 踏んだ probe(`suite::fn`) | 手 |
|---|---|---|---|
| T-1 | (start) → Draft | `ac_030::ac_030_the_same_intent_gets_the_same_two_ids_in_three_calls` | 2 |
| T-2 | Draft → Candidate | `ac_031::ac_031_plan_fixes_the_delta_the_fingerprint_and_the_state` | 2 |
| T-3 | Candidate → Verifying | `ac_032::ac_032_admit_goes_to_admitted` `ac_035::ac_035_the_verifying_state_is_covered_by_the_journal_rather_than_by_a_return` | 2 / 4 |
| T-4a | Verifying → Admitted | `ac_032::ac_032_admit_goes_to_admitted` | 2 |
| T-4b | Verifying → Denied | `ac_032::ac_032_deny_goes_to_denied` | 2 |
| T-4c | Verifying → Escalated | `ac_032::ac_032_escalate_goes_to_escalated` | 2 |
| T-4d | Verifying → Aborted(VerifierUnavailable) | `ac_032::t_4d_an_unreachable_collector_aborts_fail_closed` | 2 |
| T-4e | Verifying → Admitted(降格・enforced=false) | `ac_032::t_4e_an_unreachable_collector_degrades_under_an_explicit_fail_open` `commit_protocol::a_degraded_admission_commits_and_its_receipt_says_no_gate_ran` | 2 / 6 |
| T-5 | Escalated → Admitted(人間裁定) | `ac_071::ac_071_an_approved_escalation_becomes_admitted_and_commits` | 6 |
| T-5b | Escalated → Denied(人間裁定) | `ac_072::ac_072_a_rejected_escalation_is_denied_and_goes_no_further` | 6 |
| T-6 | {Candidate,Verifying,Escalated} → Aborted(Expired) | `ac_045::ac_045_an_untouched_candidate_is_expired_by_the_reaper` `ac_045::ac_045_an_escalated_transformation_expires_on_the_escalation_deadline` | 6 |
| T-7 | {Candidate,Verifying,Admitted,Canonicalized,Escalated} → Aborted(OwnerCancelled) | `ac_073::ac_073_every_state_before_committing_can_be_cancelled` | 6 |
| T-8 | Admitted → Canonicalized | `ac_033::ac_033_an_admitted_transformation_canonicalises` | 2 |
| T-8r | Denied → Canonicalized(RecordOnly) | `ac_033::t_8r_a_denied_transformation_canonicalises_under_record_only` | 2 |
| T-9 | Canonicalized → Committing | `commit_protocol::the_critical_section_journals_before_each_side_effect` | 4 |
| T-10a | Committing → Aborted(PreconditionChanged) | `ac_034::ac_034_a_concurrent_mutation_aborts_the_commit_without_applying` | 4 |
| T-10b | Committing → Committing(escrow・内部) | `ac_038::ac_038_the_escrowed_inverse_body_is_retrievable` `supersede::e_m5_9_a_commit_with_no_constructible_inverse_records_the_absence` | 4 / 6 |
| T-10c | Committing → Aborted(ApplyFailed) | `ac_038::ac_038_a_failed_apply_rolls_back_and_the_outcome_is_journalled` | 4 |
| T-11 | Committing → Committed | `ac_034::ac_034_an_untouched_world_commits_and_applies_once` | 4 |
| T-12 | Committed(T_o) → Superseded | `ac_040::ac_040_an_undo_is_a_new_gated_transformation_that_supersedes_the_original` | 6 |
| T-13 | {8 states} → Aborted(InternalError) | `state_machine_coverage::t_13_a_miswired_adapter_is_an_internal_error_and_not_a_precondition_change` `ac_032::e_m5_5_the_gates_bottom_aborts_whatever_the_enforcement_mode_says` | 7 / 2 |

## この表が言わない事(分母の下の分母)

100% は **遷移 ID について** 100% であって、遷移の **腕**について 100% ではない。51 §14 が gate に
するのは前者で、後者は数えられていない。差が実在する所を隠さず書く。

1. **T-13 の from-set は 8 states で、v0.1 に道が在るのは 2 つ**(`Verifying`=gate の ⊥・
   `Committing`=`cas_eq` の `Err`)。残る 6 states に `InternalError` を書く producer は v0.1 に無い。
   `state_machine_coverage::t_13_is_entered_from_two_states_and_v0_1_has_no_road_into_the_other_six`
   がその 2 つを測り、8 でない事を印字する。
2. **T-7 の from-set は 43 では 6 states で、測れたのは 4**。`Draft` は id が無く record を書けない
   (**E-M5-14**=43 T-7 の from-set から `Draft` を除外する erratum・req/38 §43)。`Verifying` は
   `verify` が T-3 と T-4a〜e を 1 呼び出しで走らせるので resting state として到達不能
   (`ac_073::ac_073_verifying_is_never_a_resting_state` が形として測る)。
3. **T-6 の from-set 3 のうち `Verifying` は同じ理由で到達不能**。`Candidate` と `Escalated` の 2 期限
   (ASM-12)は別々に測られている。
4. **T-10b は 2 腕**(`Some`=escrow・`None`=**E-M5-9** の不在記録)。両方に probe が在る。
5. **T-12 は live 経路でのみ描かれる**。復旧は edge を描き直さない(**M5H6-6**)——
   `supersede::a_recovery_does_not_draw_the_edge_the_crash_interrupted` が窓を測り続ける。
6. **これは line coverage ではない**。`cargo llvm-cov` の数字は手 8 の窓であり、この表は
   「全遷移が最低1テストで踏まれる」(51 §14 逐語)だけを言う。

## 43 §7 の復旧は遷移ではない

`Engine::recover` は 43 §7 の手続きであって 43 §3 の行ではない(**M5H5-1**)。書く record は T-11 /
T-10a / T-10c のものであり、独立した ID を持たない。∴ この表に行が無い事は欠落ではない——
`crash_recovery::the_recovery_is_one_procedure_and_not_a_ninth_transition` がそれを測る。
