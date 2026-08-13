//! 44 §1.4's exit codes, and the per-subcommand lists of §1.2 that this hand implements.
//!
//! # 🔴 規律52 (req/38 §48 M6H1-1 採(a))
//!
//! > 全 subcommand は `try_parse()`+写像(usage error→exit **1**「入力不正」・`--help`/`--version`→0)。
//! > **44 §1.4 の 2 は状態機械の「拒否」専用**(E-M6-2)。clap 既定 `parse()` 禁。
//!
//! `clap`'s default exit status for a usage error is **2**, and 44 §1.4 gives 2 to 「拒否（denied）
//! — Verdict::Deny」. A binary that took the default would answer a mistyped flag with the code that
//! means 「the gate refused your change」, and the entire reason exit statuses are specified is that
//! something branches on them.
//!
//! 🔴 **Hand 2 declared [`DENIED`] and returned it from nothing** — no read-side verb reaches a
//! gate. **Hand 3 returns it**, from `gx verify` on a `Verdict::Deny` and from `gx commit` on a
//! transformation the gate did not admit, and that is what makes the reservation worth having:
//! `crates/gx-cli/tests/exit_matrix_cli.rs` asserts those two and no others own the number, while
//! `crates/gx-cli/tests/exit_map.rs` asserts no usage error takes it.
//!
//! # The table is data, because hand 3 has to compare it
//!
//! req/88 §6.2 手 2's brief, item 8: 「44 §1.2 の exit 列と §1.4 共通表の突合は手 3 の主題(M6-25)
//! だが、本手の subcommand 分は自分の exit 表を報告 doc に書く(手 3 が突合する材料)」. A table that
//! lives only in a report is a table the next hand re-derives from the source anyway, so it lives
//! here and `probes/doubt/tests/m6_surface_doubt.rs` parses **both** this file and 44 §1.2's
//! markdown and asserts they are the same sets (the I-11 shape).

/// 「正常終了」 — 44 §1.4.
pub const OK: u8 = 0;
/// 「エラー（入力不正・内部エラー・adapterエラー）」 — and where 規律52 sends a usage error.
pub const ERROR: u8 = 1;
/// 🔴 「拒否（denied）」 — `Verdict::Deny`, and nothing else. See the module documentation.
pub const DENIED: u8 = 2;
/// 「前提条件不一致（precondition-changed）」 — 44 §1.4's 3, `Aborted(PreconditionChanged)`.
pub const PRECONDITION_CHANGED: u8 = 3;
/// 「エスカレーション（escalated, `ESCALATION_PENDING`）」 — 44 §1.4's 4.
pub const ESCALATED: u8 = 4;
/// 「適用失敗（apply-failed）」 — 44 §1.4's 5, `Aborted(ApplyFailed)`.
pub const APPLY_FAILED: u8 = 5;
/// 「未検出（not-found）」 — 44 §1.4's 6.
pub const NOT_FOUND: u8 = 6;
/// 「オフライン検証失敗」 — 44 §1.4's 7, `gx receipt verify` only.
pub const VERIFY_FAILED: u8 = 7;

/// One row of this hand's exit table.
pub struct ExitRow {
    /// The `####` section of 44 §1.2 this row belongs to: `receipt`, `replay`, `log`, `key`.
    pub group: &'static str,
    /// The status the process exits with.
    pub code: u8,
    /// What it means here, in 44's own words where 44 has them.
    pub meaning: &'static str,
    /// 🔴 What 44 §1.4's common table gives the same number, when the two readings differ.
    ///
    /// M6-25 is the ticket for the divergences and hand 3 owns the full comparison; what this hand
    /// owes is its own rows with the difference **visible**, because a difference recorded as a
    /// footnote is a difference the next reader has to rediscover.
    pub common_table_note: &'static str,
}

/// 🔴 The read side's exit codes, exactly as 44 §1.2 writes them per section.
///
/// Ten rows over four sections. Equality with 44 is the probe's claim, in both directions: a code
/// 44 writes and this omits is an exit a script cannot branch on, and a code this carries and 44
/// does not is an invention — which is precisely what clap's 2 would be.
pub const HAND2_EXITS: [ExitRow; 12] = [
    // ---- `gx receipt show` / `gx receipt verify` (44 §1.2) ------------------------------
    ExitRow {
        group: "receipt",
        code: OK,
        meaning: "show: 0=存在 / verify: 0=valid",
        common_table_note: "agrees with §1.4's 「目的の状態に到達」",
    },
    ExitRow {
        group: "receipt",
        code: ERROR,
        meaning: "verify: 1=入力エラー — and 規律52's usage error, for both",
        common_table_note: "agrees with §1.4's 「エラー（入力不正…）」",
    },
    ExitRow {
        group: "receipt",
        code: NOT_FOUND,
        meaning: "show: 6=未検出",
        common_table_note: "agrees with §1.4's 「未検出（not-found）」",
    },
    ExitRow {
        group: "receipt",
        code: VERIFY_FAILED,
        meaning: "verify: 7=無効（署名/CID/包含いずれか不一致）",
        common_table_note: "🔴 §1.4 spells 7 as 「オフライン検証失敗」 and this hand also returns it \
                            for `inclusion: unanchored`, which is 「not checked」 rather than 「不一致」. \
                            The fold is deliberate (`Checks::verified` refuses to call an unchecked \
                            ledger claim a pass) and is written down rather than hidden — req/88 \
                            §6.0-10: 畳んだら doc に 1 行. The JSON distinguishes the two; the exit \
                            status cannot. Raised as M6H2-5",
    },
    // ---- `gx replay` (44 §1.2) ----------------------------------------------------------
    ExitRow {
        group: "replay",
        code: OK,
        meaning: "0=一致",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "replay",
        code: ERROR,
        meaning: "1=不一致または実行不能",
        common_table_note: "🔴 44 §1.2 itself folds 「the journal and the ledger disagree」 into \
                            「could not run」. M4H4-2's 「未実装と失敗を同じ顔にするな」 applies and \
                            the JSON `{matches, diffs}` is where the difference survives",
    },
    // ---- `gx log proof` / `gx log consistency` (44 §1.2) --------------------------------
    ExitRow {
        group: "log",
        code: OK,
        meaning: "0=成功",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "log",
        code: ERROR,
        meaning: "1=入力不正 (44 §1.2's 「範囲外/未検出」 half moved to 6 — see the row below)",
        common_table_note: "agrees; 規律52 sends every usage error here (`gx log proof` with no \
                            `--leaf`, a `--from` that will not parse)",
    },
    ExitRow {
        group: "log",
        code: NOT_FOUND,
        meaning: "6=未検出（not-found）",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. **E-M6-24** \
                            (req/38 §55, M6H8-14 ②): §1.4's common table gives 「未検出」 the code 6 \
                            and every other verb returns it; `gx log` returned 1 because §1.2's \
                            per-command line says 「1=範囲外/未検出」. M6-25's principle — 「§1.4 の \
                            共通表に従う・§1.2 の列は抜粋と読む」 — had been applied to `cancel`, \
                            `escalation` and `undo` (E-M6-13/16) and not here, so one原理 was being \
                            applied verb by verb. Hand 2 said so in this table and did not repair \
                            it; the fix batch does",
    },
    // ---- `gx key gen` / `gx key list` (44 §1.2) -----------------------------------------
    ExitRow {
        group: "key",
        code: OK,
        meaning: "0=成功",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "key",
        code: ERROR,
        meaning: "1=鍵生成/アクセス失敗",
        common_table_note: "agrees with §1.4's 1",
    },
    // ---- 🔴 **M7 hand 2** — `gx key revoke` / `gx key rotate` name a key that is not there ----
    ExitRow {
        group: "key",
        code: NOT_FOUND,
        meaning: "6=未検出（not-found）— the store holds no key of that id",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. 44 §1.2's key \
                            section is `gen|list` and neither can name a key that does not exist; \
                            裁定 #6 (req/98 §3-2) added `revoke` and `rotate`, which take a \
                            `--key-id`, and a revocation is signed by the key it revokes so the \
                            **secret** has to be in the store. 「そんな鍵は無い」 is 未検出, and \
                            **E-M6-24**'s reading (M6-25 の原理: §1.4 の共通表に従う・§1.2 の列は \
                            抜粋と読む) gives 未検出 the code 6 in every other verb of this binary. \
                            Folding it into 1 would make a script branching on 「not found」 \
                            special-case the key verbs, which is what the common table exists to \
                            prevent",
    },
];

// ---------------------------------------------------------------------------
// 🔴 M6-25 — 44 §1.2's thirteen exit lists, and where they disagree with §1.4
// ---------------------------------------------------------------------------

/// 🔴 **44 §1.2's per-command exit lists, all thirteen** (M6-25).
///
/// req/38 §47 M6-25 採(a)+(c) put the comparison in this hand's DoD: 「§1.2 の各 exit 列と §1.4 の
/// 共通表の突合表を手 3 の DoD に置く(抜けが機械で見える形)」. Hand 2 declared its own four groups
/// in [`HAND2_EXITS`] and left the rest; this is the whole surface, **including the five verbs no
/// hand has written yet**, because the lists are 44's and are readable before any code exists.
///
/// The keys are the `####` headings of §1.2, character for character, so that
/// `probes/doubt/tests/m6_exit_matrix.rs` can parse both sides and compare sets rather than trust a
/// transcription. An edit to 44 turns that probe red on the commit that makes it.
pub const SPEC_44_EXITS: [(&str, &[u8]); 13] = [
    ("`gx submit`", &[0, 1]),
    ("`gx plan`", &[0, 1, 6]),
    ("`gx verify`", &[0, 1, 2, 4, 6]),
    ("`gx commit`", &[0, 1, 2, 3, 4, 5, 6]),
    ("`gx undo`", &[0, 1, 3, 5, 6]),
    ("`gx cancel`", &[0, 1, 6]),
    (
        "`gx escalation approve` / `gx escalation reject`",
        &[0, 1, 6],
    ),
    ("`gx receipt show` / `gx receipt verify`", &[0, 1, 6, 7]),
    ("`gx replay`", &[0, 1]),
    ("`gx log proof` / `gx log consistency`", &[0, 1]),
    ("`gx key gen` / `gx key list`", &[0, 1]),
    ("`gx policy lint` / `gx policy test`", &[0, 1]),
    ("`gx serve`", &[0, 1]),
];

/// One place where 44 §1.2 and 44 §1.4 do not say the same thing.
pub struct ExitDivergence {
    /// The `####` heading of §1.2 this is about.
    pub section: &'static str,
    /// The status of §1.4's common table the divergence turns on.
    ///
    /// Named `common_table_code` rather than `code` because `HAND2_EXITS`'s own reader parses this
    /// file line by line looking for `group:` and `code:` pairs, and a second field spelled `code:`
    /// was attributed to whichever group was declared last — a parser reading two tables as one.
    /// Measured, not guessed: it made `the_declared_exit_codes_are_the_ones_44_writes` red with
    /// `key: 44 says [0, 1], gx-cli declares [0, 1, 2, 6, 7]`.
    pub common_table_code: u8,
    /// What each of the two texts says, and which reading this workspace takes.
    pub reading: &'static str,
}

/// 🔴 **M6-25's 突合表, the part a parser cannot compute** — six divergences, named.
///
/// A machine can tell that every code §1.2 writes is one of §1.4's eight (it is, measured), and it
/// cannot tell that 「未検出」 in two sentences is one fact. So the readings live here, where a
/// reviewer can disagree with them, and `m6_exit_matrix.rs` checks that each row names a real
/// section and a real status and that the list is not empty — 「zero divergences」 being a claim this
/// hand would have to defend rather than a clean bill.
///
/// **Nothing here is repaired by this hand.** `req/spec/` is not written to; req/38 is where an
/// erratum lands, and the ones that need one are raised in req/91 as M6H3 tickets.
pub const EXIT_DIVERGENCES: [ExitDivergence; 6] = [
    ExitDivergence {
        section: "`gx undo`",
        common_table_code: DENIED,
        reading: "🔴 M6-25 採(a)+(c). §1.2 gives `undo` 0/1/3/5/6 and no seat for 「拒否（denied）」, \
                  while AC-040's second case is exactly that — 「T_uに対応するinvariant/policyを故意に \
                  DenyへセットしたケースではT_uがCommittedへ到達せずDeniedのまま」 — and the engine \
                  has implemented it since M5. Folding it into 1 would put 「could not run」 and \
                  「the gate refused you」 under one face, which is M4H4-2's standing refusal. §1.4's \
                  2 is the answer and §1.2's list is read as an excerpt; hand 4 owns the verb.",
    },
    ExitDivergence {
        section: "`gx receipt show` / `gx receipt verify`",
        common_table_code: VERIFY_FAILED,
        reading: "§49 M6H2-5. 7 carries both 「不一致」 and 「未検査」: an `inclusion: unanchored` \
                  receipt was never checked against a ledger and `Checks::verified` refuses to call \
                  that a pass (H5-9), so it exits 7 beside a receipt whose proof was refuted. The \
                  JSON distinguishes them and the status cannot.",
    },
    ExitDivergence {
        section: "`gx log proof` / `gx log consistency`",
        common_table_code: NOT_FOUND,
        reading: "§1.4 gives 「未検出」 the code 6 and §1.2's `log` line gives it 1 \
                  (「1=範囲外/未検出」). Hand 2 took the per-command text as the more specific \
                  statement and did not repair it. 🔴 **Settled by E-M6-24** (req/38 §55, M6H8-14 \
                  ②) and implemented in the M6 fix batch: an unknown leaf and an out-of-range pair \
                  now exit **6**, and `SPEC_44_EXIT_ADDITIONS` carries the citation. The row stays \
                  for `gx cancel`'s reason — a reader of 44 alone still expects 1, and a resolved \
                  disagreement that is deleted is one the next reader rediscovers. What made this \
                  the third application of one principle rather than a new decision: the audit \
                  found that M6-25's reading had been given to `cancel`, `escalation` and `undo` \
                  and withheld here, which is a principle applied verb by verb.",
    },
    ExitDivergence {
        section: "`gx replay`",
        common_table_code: ERROR,
        reading: "44 §1.2 folds it itself: 「1=不一致または実行不能」. 「the journal and the ledger \
                  disagree」 and 「the replay could not run」 are one status, and the difference \
                  survives in the JSON `{matches, diffs, ledger_consulted}` alone.",
    },
    ExitDivergence {
        section: "`gx cancel`",
        common_table_code: ERROR,
        reading: "§1.2 gives 1 to 「権限不足または実行不能な状態（既にCommitting以降・終端状態）」, \
                  and §1.4's 1 is 「エラー（入力不正・内部エラー・adapterエラー）」. A cancel refused \
                  because the transformation has passed `Committing` is none of those three: it is a \
                  state machine saying no, which is what §1.4's 2 is for. 🔴 **Settled by E-M6-13** \
                  (req/38 §51 M6H4-1 採(a)) and implemented in hand 5: the state-machine half now \
                  exits 2 and `SPEC_44_EXIT_ADDITIONS` carries the citation. The row stays in this \
                  table because the divergence from 44 §1.2's own list is still a divergence — a \
                  reader of 44 alone would still expect 1, and a resolved disagreement that is \
                  deleted is a disagreement the next reader rediscovers.",
    },
    ExitDivergence {
        section: "`gx escalation approve` / `gx escalation reject`",
        common_table_code: ERROR,
        reading: "The same shape one verb down: 「対象が`Escalated`でない」 is a state refusal wearing \
                  §1.4's 「エラー」. It and `cancel` were the two places where 44 folds a state \
                  refusal into the input-error code. 🔴 **Settled by E-M6-13** and implemented in \
                  hand 5, with a sharper consequence than `cancel`'s: 44 §1.2 wrote 「裁定者鍵不正 \
                  **または**対象が`Escalated`でない」 as one status, so the split is a split of one \
                  sentence — the key half stays on 1 and the state half moves to 2.",
    },
];

/// 🔴 This hand's four sections, as the implementation returns them.
///
/// The same shape as [`HAND2_EXITS`] and for the same reason: the table a reader wants is the one
/// beside the code. What the two tables measure is different, though — [`HAND2_EXITS`] and this are
/// **what the binary returns**, [`SPEC_44_EXITS`] is **what 44 writes** — and they are compared
/// against each other by `crates/gx-cli/tests/exit_matrix_cli.rs`.
pub const HAND3_EXITS: [ExitRow; 17] = [
    // ---- `gx submit` (44 §1.2) -----------------------------------------------------------
    ExitRow {
        group: "submit",
        code: OK,
        meaning: "0=成功",
        common_table_note: "agrees with §1.4's 「目的の状態に到達」 — the state reached is `Draft`",
    },
    ExitRow {
        group: "submit",
        code: ERROR,
        meaning: "1=入力エラー（intent不正・actor鍵不明等）",
        common_table_note: "agrees; 規律52 sends every usage error here too",
    },
    // ---- `gx plan` (44 §1.2) -------------------------------------------------------------
    ExitRow {
        group: "plan",
        code: OK,
        meaning: "0=成功（Draft → Candidate）",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "plan",
        code: ERROR,
        meaning: "1=adapterエラー（plan不能）",
        common_table_note: "agrees with §1.4's 「adapterエラー」",
    },
    ExitRow {
        group: "plan",
        code: NOT_FOUND,
        meaning: "6=指定ID未検出",
        common_table_note: "agrees; a draft this `.gx/` has never seen is 6 and not 1",
    },
    // ---- `gx verify` (44 §1.2) -----------------------------------------------------------
    ExitRow {
        group: "verify",
        code: OK,
        meaning: "0=Admit",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "verify",
        code: ERROR,
        meaning: "1=内部/adapterエラー",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "verify",
        code: DENIED,
        meaning: "2=Deny",
        common_table_note: "🔴 agrees, and this is the first command in the workspace to return \
                            §1.4's 2. 規律52 exists so that the number means this and nothing else",
    },
    ExitRow {
        group: "verify",
        code: ESCALATED,
        meaning: "4=Escalate",
        common_table_note: "agrees with §1.4 (「Verdict Escalate、人間裁定待ち」 — spelled without                             the path separator on purpose: 則 1 (ii)'s scanner reads string                             literals, and a quotation is not a construction only if it does not                             look like one)",
    },
    ExitRow {
        group: "verify",
        code: NOT_FOUND,
        meaning: "6=未検出",
        common_table_note: "agrees",
    },
    // ---- `gx commit` (44 §1.2) -----------------------------------------------------------
    ExitRow {
        group: "commit",
        code: OK,
        meaning: "0=Committed（record-only の Committed も 0）",
        common_table_note: "agrees. 🔴 §1.2's [DR-2感度] makes a record-only commit of a `Denied` \
                            transformation exit 0 with `enforced=false` on the receipt",
    },
    ExitRow {
        group: "commit",
        code: ERROR,
        meaning: "1=内部エラー",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "commit",
        code: DENIED,
        meaning: "2=Denyで未Admitのため拒否（non-record-onlyかつVerdict≠Admit）",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "commit",
        code: PRECONDITION_CHANGED,
        meaning: "3=Aborted(PreconditionChanged)",
        common_table_note: "agrees with §1.4's 「CON-2 CAS失敗」",
    },
    ExitRow {
        group: "commit",
        code: ESCALATED,
        meaning: "4=Escalated状態のまま操作不可（ESCALATION_PENDING）",
        common_table_note: "agrees; §1.4 names this case explicitly",
    },
    ExitRow {
        group: "commit",
        code: APPLY_FAILED,
        meaning: "5=Aborted(ApplyFailed)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "commit",
        code: NOT_FOUND,
        meaning: "6=未検出",
        common_table_note: "agrees",
    },
];

/// 🔴 **A status the implementation returns that 44 §1.2's per-command list does not write.**
///
/// One row per ruled extension. The comparison in
/// `crates/gx-cli/tests/exit_matrix_cli.rs` is `implemented == spec ∪ additions`, so an addition
/// **still has to be declared here** to pass — a verb that quietly grew a status is red, and one
/// that grew a ruled status is a row with a citation. That is the difference between reading §1.2's
/// list as an excerpt and reading it as whatever the code happens to do.
pub struct ExitAddition {
    /// The `####` heading of §1.2 this belongs to.
    pub section: &'static str,
    /// The status §1.2's list omits.
    pub code: u8,
    /// The ruling that put it there, and the reason.
    pub ruling: &'static str,
}

/// 🔴 **M6-25's body** — `gx undo`'s two.
///
/// req/38 §47 M6-25 採(a)+(c): 「§1.4 の共通表に従い deny で 2 を返す(§1.2 の列は網羅でなく抜粋と読む
/// erratum)」+「§1.2 の列に 2 を足す erratum」. Both statuses below are the same reading applied to
/// the two things that can stop an undo short of `Committed` and that 44 §1.4 *does* have a code for.
pub const SPEC_44_EXIT_ADDITIONS: [ExitAddition; 6] = [
    // ---- **E-M6-24** (req/38 §55, M6H8-14 ②) — the third verb to receive M6-25's reading ----
    ExitAddition {
        section: "`gx log proof` / `gx log consistency`",
        code: NOT_FOUND,
        ruling: "🔴 **E-M6-24**, req/38 §55: 「`gx log proof/consistency` の未検出 exit は §1.4 共通表 \
                 の **6**(M6-25 原理の 3 例目・E-M6-13/16 と同型)」. 44 §1.2's `log` line reads \
                 「exit: 0=成功, 1=範囲外/未検出」 while §1.4's common table gives 6 to 「未検出 \
                 （not-found）」, and every other verb in this binary returns 6. A script that \
                 branches on 「not found」 across verbs had to special-case one of them, which is \
                 what the common table exists to prevent. `1` keeps its ordinary meaning here \
                 (規律52's usage errors), so the two facts stop sharing a status.",
    },
    // ---- **E-M6-13** (req/38 §51 M6H4-1 採(a)) — hand 4 raised these two and hand 5 pays them ----
    ExitAddition {
        section: "`gx cancel`",
        code: DENIED,
        ruling: "🔴 **E-M6-13**, req/38 §51 M6H4-1 採(a): 「cancel/escalation の状態機械拒否に exit \
                 **2** を足す(§1.2 列は抜粋・M6-25 の読みの残り 2 verb への適用)。実装は手5 以降の \
                 最初に踏む手」. 43 T-7's from-set stops before `Committing` and its guard is \
                 「`Committing`到達前」, so a cancel of a committed row is the state machine refusing \
                 — none of §1.4's three meanings for 1 (入力不正 / 内部エラー / adapterエラー). \
                 E-M6-1's Draft refusal stays on **1**, because a draft has no seat in T-7 at all \
                 and naming one is 「入力不正」 in the ordinary sense.",
    },
    ExitAddition {
        section: "`gx escalation approve` / `gx escalation reject`",
        code: DENIED,
        ruling: "🔴 **E-M6-13**, the same reading one verb along. INV-S6 is 「`Escalated`はT-5/T-5bの \
                 署名済み人間裁定receiptを経由せずに`Admitted`/`Denied`へ自動遷移しない」 and its \
                 mirror is that a ruling may not be filed against a transformation nobody escalated; \
                 44 §1.2 gave that 1 beside 「裁定者鍵不正」, which is a genuine 入力不正 and a \
                 different event. Splitting them is what makes either number readable: a script \
                 retrying on 1 (fix your key) must not retry on 2 (the row is not escalated).",
    },
    ExitAddition {
        section: "`gx undo`",
        code: DENIED,
        ruling: "M6-25 採(a)+(c). §1.2 gives `undo` 0/1/3/5/6 and no seat for 「拒否（denied）」, \
                 while AC-040's second case is exactly that — 「T_uに対応するinvariant/policyを故意 \
                 にDenyへセットしたケースではT_uがCommittedへ到達せずDeniedのまま」 — and the engine \
                 has implemented it since M5. §1.4's 2 is 「拒否（denied）」 and §1.2's list is read \
                 as an excerpt; folding a refused undo into 1 would give 「could not run」 and 「the \
                 gate said no」 one face, which is M4H4-2's standing refusal.",
    },
    ExitAddition {
        section: "`gx undo`",
        code: ESCALATED,
        ruling: "The same reading, one verdict along. An undo whose own inverse cannot be built \
                 reaches `Escalated` through E-M3-4 (the fs adapter answers `Ok(None)` above \
                 `MAX_INVERSE_PAYLOAD_BYTES`), and §1.4's 4 is 「エスカレーション（escalated, \
                 ESCALATION_PENDING）」 — the state the transformation is actually in. §1.2's list \
                 omits it for the same reason it omits 2: it is the list of the ways the *escrow* \
                 can fail, not of the ways the pipeline it drives can stop.",
    },
    // ---- 🔴 **M7 hand 2** (裁定 #6) — the key section gains two verbs, and they can miss ----
    ExitAddition {
        section: "`gx key gen` / `gx key list`",
        code: NOT_FOUND,
        ruling: "🔴 **M7 手 2**, 裁定 #6 (req/98 §3-2: 「U-06/13 鍵ローテ=M7 採」). 44 §1.2's key \
                 section lists `gen` and `list`, and neither can name a key that is not there — \
                 `gen` makes one and `list` answers with an empty store. `revoke` and `rotate` take \
                 a `--key-id` and need the **secret**, because a revocation is signed by the key it \
                 revokes, so 「そんな鍵は無い」 becomes reachable in this section for the first time. \
                 It is 未検出, and **E-M6-24** settled that 未検出 is §1.4's **6** across this \
                 binary (M6-25 の原理: §1.2 の列は抜粋). `crates/gx-cli/tests/key_lifecycle_cli.rs::\
                 revoking_a_key_the_store_does_not_hold_is_not_found` measures the status rather \
                 than leaving this table to assert about itself.",
    },
];

/// 🔴 This hand's four sections, as the implementation returns them.
///
/// `undo` carries seven statuses and 44 §1.2 writes five of them; the other two are
/// [`SPEC_44_EXIT_ADDITIONS`]. `policy` matches 44 exactly.
///
/// 🔴 **`cancel` and `escalation` no longer do, and that is E-M6-13** (req/38 §51 M6H4-1 採(a)).
/// Hand 4 wrote 「**including the two places where a state-machine refusal wears 「エラー」**, which
/// this hand raises as M6H4-1 rather than repairing」; §51 ruled it and named the hand — 「実装は手5
/// 以降の最初に踏む手」 — so both verbs now carry a **2** as well, declared in
/// [`SPEC_44_EXIT_ADDITIONS`] with its citation.
pub const HAND4_EXITS: [ExitRow; 17] = [
    // ---- `gx undo` (44 §1.2) --------------------------------------------------------------
    ExitRow {
        group: "undo",
        code: OK,
        meaning: "0=成功（新 Transformation が Committed・T_o は Superseded）",
        common_table_note: "agrees with §1.4's 「目的の状態に到達」",
    },
    ExitRow {
        group: "undo",
        code: ERROR,
        meaning: "1=`InverseStatus`が`Unavailable`/`Expired`/`Consumed`で実行不能",
        common_table_note: "agrees; a second undo of one commit lands here through 42 §3.12's \
                            `Consumed`, which is what makes 「一度だけ」 a fact",
    },
    ExitRow {
        group: "undo",
        code: DENIED,
        meaning: "2=undo そのものが gate に拒否された（AC-040 の第 2 ケース）",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. 43 §5-2: \
                            「undoであっても検証を免除されない」, so an undo has a verdict and a \
                            verdict can be `Deny`",
    },
    ExitRow {
        group: "undo",
        code: PRECONDITION_CHANGED,
        meaning: "3=commit失敗（`gx commit`と同義）: Aborted(PreconditionChanged)",
        common_table_note: "agrees with §1.4's 「CON-2 CAS失敗」",
    },
    ExitRow {
        group: "undo",
        code: ESCALATED,
        meaning: "4=undo が Escalated のまま（人間裁定待ち）",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`",
    },
    ExitRow {
        group: "undo",
        code: APPLY_FAILED,
        meaning: "5=commit失敗（`gx commit`と同義）: Aborted(ApplyFailed)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "undo",
        code: NOT_FOUND,
        meaning: "6=未検出",
        common_table_note: "agrees; a transformation this `.gx/` never committed, and a committed \
                            one whose draft is gone, both land here (M6H4-5)",
    },
    // ---- `gx cancel` (44 §1.2) ------------------------------------------------------------
    ExitRow {
        group: "cancel",
        code: OK,
        meaning: "0=成功（Aborted(OwnerCancelled)）",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "cancel",
        code: ERROR,
        meaning: "1=権限不足または実行不能な状態（既にCommitting以降・終端状態）",
        common_table_note:
            "🔴 **E-M6-13 moved the state-machine half of this row to 2.** What is \
                            left on 1 is what §1.4's 1 actually means: **E-M6-1's Draft refusal** \
                            (a draft has no seat in 43 T-7, so naming one is 「入力不正」) and \
                            `--actor-key`, which selects nothing (M6H4-3)",
    },
    ExitRow {
        group: "cancel",
        code: DENIED,
        meaning: "2=状態機械の拒否（既に`Committing`以降・終端状態）",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. E-M6-13 \
                            (req/38 §51 M6H4-1 採(a)). 43 T-7's guard is 「`Committing`到達前」 and \
                            a row past it is refused **by the state machine**, which is §1.4's \
                            「拒否（denied）」",
    },
    ExitRow {
        group: "cancel",
        code: NOT_FOUND,
        meaning: "6=未検出",
        common_table_note: "agrees",
    },
    // ---- `gx escalation approve` / `gx escalation reject` (44 §1.2) -----------------------
    ExitRow {
        group: "escalation",
        code: OK,
        meaning: "0=成功（Admitted または Denied）",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "escalation",
        code: ERROR,
        meaning: "1=裁定者鍵不正（`--actor-key` 欠落を含む: M6H4-6）",
        common_table_note:
            "🔴 **E-M6-13 split this row.** 44 §1.2 wrote 「裁定者鍵不正または対象 \
                            が`Escalated`でない」 as one status; the first half is a genuine \
                            「入力不正」 and stays here, the second half is the state machine and \
                            moved to 2. A script retrying on 1 fixes a key; one retrying on 2 would \
                            be retrying a refusal that will never change",
    },
    ExitRow {
        group: "escalation",
        code: DENIED,
        meaning: "2=対象が`Escalated`でない（状態機械の拒否）",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. E-M6-13. \
                            INV-S6's mirror: a ruling may not be filed against a transformation \
                            nobody escalated",
    },
    ExitRow {
        group: "escalation",
        code: NOT_FOUND,
        meaning: "6=未検出（チケット不明）",
        common_table_note: "agrees. 🔴 This is the status M6-04 made reachable at all: before \
                            `Engine::transformation_of_ticket` a `<TICKET_ID>` resolved to nothing \
                            **whatever it named**",
    },
    // ---- `gx policy lint` / `gx policy test` (44 §1.2) ------------------------------------
    ExitRow {
        group: "policy",
        code: OK,
        meaning: "0=問題なし",
        common_table_note: "agrees. A **warning** (an empty policy set, an empty invariant \
                            registry) is reported in the JSON and does not move the status: 44 \
                            §1.2's 1 is 「lint/testエラーあり」 and a warning is not an error",
    },
    ExitRow {
        group: "policy",
        code: ERROR,
        meaning: "1=lint/testエラーあり",
        common_table_note: "agrees. A pack that will not parse and a scenario whose expectation \
                            was not met both land here — 44 §1.2 gives this command two statuses \
                            and the JSON is where the difference survives (Λ4)",
    },
];

/// What a command produced: the JSON 44 §1.3 asks for, and the status it exits with.
///
/// One type rather than two return channels, because 44 §1.3 ties them together — 「単一オブジェクト
/// を返すコマンドはstdoutに**改行終端の単一JSON**」 — and a function that printed and a caller that
/// chose the status would put the two halves of one contract in two places. `main` prints; the
/// library decides. That split is also what makes the exit codes testable without a subprocess.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The object stdout carries.
    pub json: serde_json::Value,
    /// The process status.
    pub code: u8,
}

impl Outcome {
    /// A successful result: this object, exit 0.
    #[must_use]
    pub fn ok(json: serde_json::Value) -> Self {
        Self { json, code: OK }
    }

    /// A result that ran and answered 「no」: this object, and a status that is not 0.
    #[must_use]
    pub fn refused(json: serde_json::Value, code: u8) -> Self {
        Self { json, code }
    }
}
