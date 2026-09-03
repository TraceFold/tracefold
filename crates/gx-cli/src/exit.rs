// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 44 §1.4's exit codes, and the per-subcommand lists of §1.2 that this hand implements.
//!
//! # 🔴 discipline 52 (req/38 §48 M6H1-1 adopted (a)) (sem: SEM-gx-cli-504)
//!
//! > every subcommand uses `try_parse()` + mapping (usage error → exit **1** "invalid input";
//! > `--help`/`--version` → 0). **44 §1.4's 2 is reserved for the state machine's "refused"**
//! > (E-M6-2). clap's default `parse()` is forbidden (sem: SEM-gx-cli-505).
//!
//! `clap`'s default exit status for a usage error is **2**, and 44 §1.4 gives 2 to "refused (denied)
//! — Verdict::Deny" (sem: SEM-gx-cli-506). A binary that took the default would answer a mistyped flag with the code that
//! means "the gate refused your change", and the entire reason exit statuses are specified is that
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
//! req/88 §6.2 hand 2's brief, item 8: "cross-checking 44 §1.2's exit column against §1.4's
//! common table is hand 3's subject (M6-25), but this hand writes its own subcommands' exit table
//! into the report doc (material for hand 3's cross-check)" (sem: SEM-gx-cli-507). A table that
//! lives only in a report is a table the next hand re-derives from the source anyway, so it lives
//! here and `probes/doubt/tests/m6_surface_doubt.rs` parses **both** this file and 44 §1.2's
//! markdown and asserts they are the same sets (the I-11 shape).

/// "normal termination" (sem: SEM-gx-cli-508) — 44 §1.4.
pub const OK: u8 = 0;
/// "error (invalid input / internal error / adapter error)" (sem: SEM-gx-cli-509) — and where discipline 52 sends a usage error.
pub const ERROR: u8 = 1;
/// 🔴 "refused (denied)" (sem: SEM-gx-cli-510) — `Verdict::Deny`, and nothing else. See the module documentation.
pub const DENIED: u8 = 2;
/// "precondition mismatch (precondition-changed)" (sem: SEM-gx-cli-511) — 44 §1.4's 3, `Aborted(PreconditionChanged)`.
pub const PRECONDITION_CHANGED: u8 = 3;
/// "escalation (escalated, `ESCALATION_PENDING`)" (sem: SEM-gx-cli-512) — 44 §1.4's 4.
pub const ESCALATED: u8 = 4;
/// "apply failed (apply-failed)" (sem: SEM-gx-cli-513) — 44 §1.4's 5, `Aborted(ApplyFailed)`.
pub const APPLY_FAILED: u8 = 5;
/// "not found (not-found)" (sem: SEM-gx-cli-514) — 44 §1.4's 6.
pub const NOT_FOUND: u8 = 6;
/// "offline verification failure" (sem: SEM-gx-cli-515) — 44 §1.4's 7, `gx receipt verify` only.
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
        meaning: "show: 0=exists / verify: 0=valid",
        common_table_note: "agrees with §1.4's \"reached the intended state\" (sem: SEM-gx-cli-520)",
    },
    ExitRow {
        group: "receipt",
        code: ERROR,
        meaning: "verify: 1=input error — and discipline 52's usage error, for both (sem: SEM-gx-cli-521)",
        common_table_note: "agrees with §1.4's \"error (invalid input…)\"",
    },
    ExitRow {
        group: "receipt",
        code: NOT_FOUND,
        meaning: "show: 6=not-found (sem: SEM-gx-cli-522)",
        common_table_note: "agrees with §1.4's \"not-found\"",
    },
    ExitRow {
        group: "receipt",
        code: VERIFY_FAILED,
        meaning: "verify: 7=invalid (mismatch in any of signature/CID/inclusion) (sem: SEM-gx-cli-523)",
        common_table_note: "🔴 §1.4 spells 7 as \"offline verification failure\" and this hand also returns it \
                            for `inclusion: unanchored`, which is \"not checked\" rather than \"mismatch\". \
                            The fold is deliberate (`Checks::verified` refuses to call an unchecked \
                            ledger claim a pass) and is written down rather than hidden — req/88 \
                            §6.0-10: \"if it is folded, put one line in the doc\" (sem: SEM-gx-cli-524). The JSON distinguishes the two; the exit \
                            status cannot. Raised as M6H2-5",
    },
    // ---- `gx replay` (44 §1.2) ----------------------------------------------------------
    ExitRow {
        group: "replay",
        code: OK,
        meaning: "0=match (sem: SEM-gx-cli-525)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "replay",
        code: ERROR,
        meaning: "1=mismatch or unable to execute (sem: SEM-gx-cli-526)",
        common_table_note: "🔴 44 §1.2 itself folds \"the journal and the ledger disagree\" into \
                            \"could not run\". M4H4-2's \"do not give the unimplemented and a failure the same face\" applies and \
                            the JSON `{matches, diffs}` is where the difference survives",
    },
    // ---- `gx log proof` / `gx log consistency` (44 §1.2) --------------------------------
    ExitRow {
        group: "log",
        code: OK,
        meaning: "0=success (sem: SEM-gx-cli-527)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "log",
        code: ERROR,
        meaning: "1=invalid input (44 §1.2's \"out-of-range/not-found\" half moved to 6 — see the row below) (sem: SEM-gx-cli-528)",
        common_table_note: "agrees; discipline 52 sends every usage error here (`gx log proof` with no \
                            `--leaf`, a `--from` that will not parse)",
    },
    ExitRow {
        group: "log",
        code: NOT_FOUND,
        meaning: "6=not-found (sem: SEM-gx-cli-529)",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. **E-M6-24** \
                            (req/38 §55, M6H8-14 ②): §1.4's common table gives \"not-found\" (sem: SEM-gx-cli-530) the code 6 \
                            and every other verb returns it; `gx log` returned 1 because §1.2's \
                            per-command line says \"1=out-of-range/not-found\". M6-25's principle — \"follow §1.4's \
                            common table; read §1.2's column as an excerpt\" (sem: SEM-gx-cli-531) — had been applied to `cancel`, \
                            `escalation` and `undo` (E-M6-13/16) and not here, so one principle was being \
                            applied verb by verb. Hand 2 said so in this table and did not repair \
                            it; the fix batch does",
    },
    // ---- `gx key gen` / `gx key list` (44 §1.2) -----------------------------------------
    ExitRow {
        group: "key",
        code: OK,
        meaning: "0=success (sem: SEM-gx-cli-532)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "key",
        code: ERROR,
        meaning: "1=key generation/access failure (sem: SEM-gx-cli-533)",
        common_table_note: "agrees with §1.4's 1",
    },
    // ---- 🔴 **M7 hand 2** — `gx key revoke` / `gx key rotate` name a key that is not there ----
    ExitRow {
        group: "key",
        code: NOT_FOUND,
        meaning: "6=not-found — the store holds no key of that id (sem: SEM-gx-cli-534)",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. 44 §1.2's key \
                            section is `gen|list` and neither can name a key that does not exist; \
                            ruling #6 (sem: SEM-gx-cli-535) (req/98 §3-2) added `revoke` and `rotate`, which take a \
                            `--key-id`, and a revocation is signed by the key it revokes so the \
                            **secret** has to be in the store. \"there is no such key\" is not-found, and \
                            **E-M6-24**'s reading (M6-25's principle: follow §1.4's common table, \
                            read §1.2's column as an excerpt (sem: SEM-gx-cli-536)) gives not-found the code 6 in every other verb of this binary. \
                            Folding it into 1 would make a script branching on \"not found\" \
                            special-case the key verbs, which is what the common table exists to \
                            prevent",
    },
];

// ---------------------------------------------------------------------------
// 🔴 M6-25 — 44 §1.2's fourteen exit lists, and where they disagree with §1.4
// ---------------------------------------------------------------------------

/// 🔴 **44 §1.2's per-command exit lists, all fourteen** (M6-25).
///
/// req/38 §47 M6-25 adopted (a)+(c) put the comparison in this hand's DoD: "put the
/// cross-check table of §1.2's each exit column against §1.4's common table in hand 3's DoD (in a
/// shape a machine can see the gaps in)" (sem: SEM-gx-cli-537). Hand 2 declared its own four groups
/// in [`HAND2_EXITS`] and left the rest; this is the whole surface, **including the five verbs no
/// hand has written yet**, because the lists are 44's and are readable before any code exists.
///
/// The keys are the `####` headings of §1.2, character for character, so that
/// `probes/doubt/tests/m6_exit_matrix.rs` can parse both sides and compare sets rather than trust a
/// transcription. An edit to 44 turns that probe red on the commit that makes it.
///
/// 🔴 **v0.2.6 doc batch** (req/38 §72 assignment item 1; sem: SEM-gx-cli-538): thirteen became fourteen when 44 §1.2 grew a
/// dedicated section for `gx key revoke` / `gx key rotate` (v0.2.1 addendum, 2026-08-13) and this table
/// was not grown to match — the same discipline 55② shape `journal_vocabulary.rs`'s A-1 note names. The
/// row below is the growth; the size in the array's own type is the fact that forces it, the same
/// way `HAND2_EXITS: [ExitRow; 11]` became `[ExitRow; 12]` when M7 hand 2 added a row (this file's
/// own precedent, `git show 6d8b606`).
///
/// 🔴 **v0.2.6 doc batch, item 5** (req/38 §72 assignment 5, req/120 §1.1; sem: SEM-gx-cli-539): fourteen became sixteen when
/// P3 added `gx wrap` and `gx verdict-checkpoint issue|verify|list` as their own §1.2 sections. (sem: SEM-gx-cli-516, SEM-gx-cli-517, SEM-gx-cli-518)
/// 🔴 **P-2 I** (`req/571` Part I, `req/38` §341 ruling (A); 2026-08-23): seventeen became
/// **eighteen** when `gx receipt coverage` got its own §1.2 section. The verb shipped with P-1b
/// (`req/544` §3 R-3c) and 44 carried no section for it, so the one table that says what a caller
/// may branch on said nothing about a verb the binary already answers — the "the exhaustive table
/// is not exhaustive" shape of `req/38` §65 B-4, arriving from the other direction.
///
/// The row is its **own** section rather than a third verb folded into
/// "`gx receipt show` / `gx receipt verify`", because the exit sets are not the same set:
/// `coverage` cannot return **7**. 7 is 44 §1.4's "offline verification failure", and
/// `gx_witness::ReceiptCoverage::of` is a projection of the payload — it checks neither signature
/// nor inclusion, so there is no verification for it to fail. Folding the three verbs into one row
/// would have had to either write a 7 this verb never returns or narrow the row and take 7 away
/// from `verify`, which does. A heading per verb is what lets the difference be a fact rather than
/// a footnote.
///
/// 🔴 **R44 lane B, item 7** (`req/603` §8, `req/38` §369; 2026-08-23): eighteen became **nineteen**
/// when `gx attach` got its own §1.2 section. The verb shipped as P-1a (`req/535`, `req/38` §353-3)
/// and 44 carried no section for it either — the same "the exhaustive table is not exhaustive" shape
/// as `gx receipt coverage`'s arrival, found this time by `req/589` §8 rather than by this hand.
/// Placed first among the sections rather than kept in the order it landed in the binary: every
/// other verb's road to a working `.gx/` runs through this one first, so its section precedes theirs
/// in the document that describes them (`gx receipt coverage`'s own precedent for placing a section
/// by adjacency to what it is about, not by arrival date).
///
/// 🔴 **Three section strings below carried Japanese provenance markers until 2026-08-26**
/// (req/835 §4), and one of them recurs in [`SPEC_44_EXIT_ADDITIONS`]. They are exact-match
/// copies, character for character, of `req/spec/40-architecture/44-api-spec.md`'s own `#### `
/// lines: `probes/doubt/tests/m6_exit_matrix.rs` reads both sides and compares them verbatim, so
/// the canon headings, these literals and every verbatim quoter moved to English in one commit —
/// the same identity discipline `gx-substrate-conformance/src/contracts.rs` states for 51 §7's
/// table. The Japanese originals sit verbatim in the maintainers' ledger.
pub const SPEC_44_EXITS: [(&str, &[u8]); 19] = [
    // ---- 🔴 **R44 lane B, item 7** (`req/603` §8, `req/38` §369) — 44 §1.2's nineteenth section,
    // and the one placed first in the document: everything after it presumes the `.gx/` this verb
    // places. See `HAND_ATTACH_EXITS` for the empirical walk that grounds the three codes.
    ("`gx attach`", &[0, 1, 6]),
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
    // ---- 🔴 **P-2 I** (`req/571` Part I, `req/38` §341 ruling (A)) — 44 §1.2's ninth section ----
    //
    // No 7. See the note on this table: `coverage` reads a document and does not verify it, so the
    // status that means "offline verification failed" has nothing here to be about. `0` is an
    // answer (a table full of `unknown` is still an answer — `crates/gx-cli/src/receipt.rs`'s
    // `coverage` returns `Outcome::ok` for it), `1` is `Error::Usage`/`Witness`/`Malformed`
    // through `lib.rs`'s default arm, and `6` is `Error::Io{NotFound}` — the file is not there.
    ("`gx receipt coverage`", &[0, 1, 6]),
    ("`gx replay`", &[0, 1]),
    ("`gx log proof` / `gx log consistency`", &[0, 1]),
    ("`gx key gen` / `gx key list`", &[0, 1]),
    // ---- 🔴 **v0.2.6 doc batch** (req/38 §72 item 1) — 44 §1.2's fourteenth section ----
    (
        "🔴 `gx key revoke` / `gx key rotate` (v0.2.1 addendum, 2026-08-13)",
        &[0, 1],
    ),
    ("`gx policy lint` / `gx policy test`", &[0, 1]),
    ("`gx serve`", &[0, 1]),
    // ---- 🔴 **v0.2.6 doc batch** (req/38 §72 item 5) — P3's two new §1.2 sections ----
    ("🔴 `gx wrap` (v0.2.6 addendum, P3, req/119 §2)", &[0, 1, 7]),
    (
        "🔴 `gx verdict-checkpoint issue` / `gx verdict-checkpoint verify` / `gx verdict-checkpoint list` (v0.2.6 addendum, P3/FR-M04, req/119 §4)",
        &[0, 1, 7],
    ),
    // ---- 🔴 **R3** (`req/38` §160 ruling 2, DR-43-8) — 44 §1.2's seventeenth section ----
    //
    // No number is minted: **0** is 44 §1.4's "reached the intended state" (the project can be
    // written to again), **1** is its "unable to execute" (it still cannot, which is also what a
    // caller gets when another `gx` holds the lock), and **6** is "not found" for a directory with
    // no `.gx/`. A repair that could not repair is not a new kind of failure — it is the same
    // "unable to execute" every write verb answers on this project, said by the one verb that
    // looked.
    (
        "🔴 `gx repair` (v0.4-p addendum, 2026-08-16, **DR-43-8**, `req/38` §160 ruling 2, `req/222` H-06)",
        &[0, 1, 6],
    ),
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

/// 🔴 **M6-25's cross-check table, the part a parser cannot compute** (sem: SEM-gx-cli-541) — six divergences, named.
///
/// A machine can tell that every code §1.2 writes is one of §1.4's eight (it is, measured), and it
/// cannot tell that "not-found" (sem: SEM-gx-cli-542) in two sentences is one fact. So the readings live here, where a
/// reviewer can disagree with them, and `m6_exit_matrix.rs` checks that each row names a real
/// section and a real status and that the list is not empty — "zero divergences" (sem: SEM-gx-cli-543) being a claim this
/// hand would have to defend rather than a clean bill.
///
/// **Nothing here is repaired by this hand.** `req/spec/` is not written to; req/38 is where an
/// erratum lands, and the ones that need one are raised in req/91 as M6H3 tickets.
pub const EXIT_DIVERGENCES: [ExitDivergence; 6] = [
    ExitDivergence {
        section: "`gx undo`",
        common_table_code: DENIED,
        reading: "🔴 M6-25 adopted (a)+(c) (sem: SEM-gx-cli-544). §1.2 gives `undo` 0/1/3/5/6 and no seat for \"refused (denied)\", \
                  while AC-040's second case is exactly that — \"in the case where the invariant/policy \
                  corresponding to T_u is deliberately set to Deny, T_u fails to reach Committed and \
                  stays Denied\" — and the engine \
                  has implemented it since M5. Folding it into 1 would put \"could not run\" and \
                  \"the gate refused you\" under one face, which is M4H4-2's standing refusal. §1.4's \
                  2 is the answer and §1.2's list is read as an excerpt; hand 4 owns the verb.",
    },
    ExitDivergence {
        section: "`gx receipt show` / `gx receipt verify`",
        common_table_code: VERIFY_FAILED,
        reading: "§49 M6H2-5 (sem: SEM-gx-cli-545). 7 carries both \"mismatch\" and \"not checked\": an `inclusion: unanchored` \
                  receipt was never checked against a ledger and `Checks::verified` refuses to call \
                  that a pass (H5-9), so it exits 7 beside a receipt whose proof was refuted. The \
                  JSON distinguishes them and the status cannot.",
    },
    ExitDivergence {
        section: "`gx log proof` / `gx log consistency`",
        common_table_code: NOT_FOUND,
        reading: "§1.4 gives \"not-found\" the code 6 and §1.2's `log` line gives it 1 (sem: SEM-gx-cli-546) \
                  (\"1=out-of-range/not-found\"). Hand 2 took the per-command text as the more specific \
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
        reading: "44 §1.2 folds it itself: \"1=mismatch or unable to execute\" (sem: SEM-gx-cli-547). \"the journal and the ledger \
                  disagree\" and \"the replay could not run\" are one status, and the difference \
                  survives in the JSON `{matches, diffs, ledger_consulted}` alone.",
    },
    ExitDivergence {
        section: "`gx cancel`",
        common_table_code: ERROR,
        reading: "§1.2 gives 1 to \"insufficient authority or a state where execution is impossible \
                  (already at `Committing` or later — a terminal state)\" (sem: SEM-gx-cli-548), \
                  and §1.4's 1 is \"error (invalid input / internal error / adapter error)\". A cancel refused \
                  because the transformation has passed `Committing` is none of those three: it is a \
                  state machine saying no, which is what §1.4's 2 is for. 🔴 **Settled by E-M6-13** \
                  (req/38 §51 M6H4-1 adopted (a); sem: SEM-gx-cli-549) and implemented in hand 5: the state-machine half now \
                  exits 2 and `SPEC_44_EXIT_ADDITIONS` carries the citation. The row stays in this \
                  table because the divergence from 44 §1.2's own list is still a divergence — a \
                  reader of 44 alone would still expect 1, and a resolved disagreement that is \
                  deleted is a disagreement the next reader rediscovers.",
    },
    ExitDivergence {
        section: "`gx escalation approve` / `gx escalation reject`",
        common_table_code: ERROR,
        reading: "The same shape one verb down: \"the target is not `Escalated`\" (sem: SEM-gx-cli-550) is a state refusal wearing \
                  §1.4's \"error\". It and `cancel` were the two places where 44 folds a state \
                  refusal into the input-error code. 🔴 **Settled by E-M6-13** and implemented in \
                  hand 5, with a sharper consequence than `cancel`'s: 44 §1.2 wrote \"the ruler's key is \
                  invalid **or** the target is not `Escalated`\" (sem: SEM-gx-cli-551) as one status, so the split is a split of one \
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
        meaning: "0=success",
        common_table_note: "agrees with §1.4's \"reached the intended state\" (sem: SEM-gx-cli-552) — the state reached is `Draft`",
    },
    ExitRow {
        group: "submit",
        code: ERROR,
        meaning: "1=input error (invalid intent, unknown actor key, etc.) (sem: SEM-gx-cli-553)",
        common_table_note: "agrees; discipline 52 sends every usage error here too",
    },
    // ---- `gx plan` (44 §1.2) -------------------------------------------------------------
    ExitRow {
        group: "plan",
        code: OK,
        meaning: "0=success (Draft → Candidate) (sem: SEM-gx-cli-554)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "plan",
        code: ERROR,
        meaning: "1=adapter error (unable to plan) (sem: SEM-gx-cli-555)",
        common_table_note: "agrees with §1.4's \"adapter error\"",
    },
    ExitRow {
        group: "plan",
        code: NOT_FOUND,
        meaning: "6=specified ID not found (sem: SEM-gx-cli-556)",
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
        meaning: "1=internal/adapter error (sem: SEM-gx-cli-557)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "verify",
        code: DENIED,
        meaning: "2=Deny",
        common_table_note: "🔴 agrees, and this is the first command in the workspace to return \
                            §1.4's 2. discipline 52 exists so that the number means this and nothing else (sem: SEM-gx-cli-558)",
    },
    ExitRow {
        group: "verify",
        code: ESCALATED,
        meaning: "4=Escalate",
        common_table_note: "agrees with §1.4 (\"Verdict Escalate, awaiting human ruling\" (sem: SEM-gx-cli-559) — spelled without the path separator on purpose: Rule 1 (ii)'s scanner reads string literals, and a quotation is not a construction only if it does not look like one)",
    },
    ExitRow {
        group: "verify",
        code: NOT_FOUND,
        meaning: "6=not-found (sem: SEM-gx-cli-560)",
        common_table_note: "agrees",
    },
    // ---- `gx commit` (44 §1.2) -----------------------------------------------------------
    ExitRow {
        group: "commit",
        code: OK,
        meaning: "0=Committed (a record-only Committed is also 0) (sem: SEM-gx-cli-561)",
        common_table_note: "agrees. 🔴 §1.2's [DR-2 sensitivity] makes a record-only commit of a `Denied` \
                            transformation exit 0 with `enforced=false` on the receipt",
    },
    ExitRow {
        group: "commit",
        code: ERROR,
        meaning: "1=internal error (sem: SEM-gx-cli-562)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "commit",
        code: DENIED,
        meaning: "2=refused because Deny/not-Admit (outside record-only and Verdict≠Admit) (sem: SEM-gx-cli-563)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "commit",
        code: PRECONDITION_CHANGED,
        meaning: "3=Aborted(PreconditionChanged)",
        common_table_note: "agrees with §1.4's \"CON-2 CAS failure\" (sem: SEM-gx-cli-564)",
    },
    ExitRow {
        group: "commit",
        code: ESCALATED,
        meaning: "4=cannot operate while still Escalated (ESCALATION_PENDING) (sem: SEM-gx-cli-565)",
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
        meaning: "6=not-found (sem: SEM-gx-cli-566)",
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
/// req/38 §47 M6-25 adopted (a)+(c): "follow §1.4's common table and return 2 for deny
/// (erratum: read §1.2's column as an excerpt, not exhaustive)" + "erratum: add 2 to §1.2's
/// column" (sem: SEM-gx-cli-567). Both statuses below are the same reading applied to
/// the two things that can stop an undo short of `Committed` and that 44 §1.4 *does* have a code for.
pub const SPEC_44_EXIT_ADDITIONS: [ExitAddition; 7] = [
    // ---- **E-M6-24** (req/38 §55, M6H8-14 ②) — the third verb to receive M6-25's reading ----
    ExitAddition {
        section: "`gx log proof` / `gx log consistency`",
        code: NOT_FOUND,
        ruling: "🔴 **E-M6-24**, req/38 §55: \"`gx log proof/consistency`'s not-found exit is §1.4's \
                 common table's **6** (the 3rd example of M6-25's principle, the same shape as \
                 E-M6-13/16)\" (sem: SEM-gx-cli-568). 44 §1.2's `log` line reads \
                 \"exit: 0=success, 1=out-of-range/not-found\" while §1.4's common table gives 6 to \"not-found\", \
                 and every other verb in this binary returns 6. A script that \
                 branches on \"not found\" across verbs had to special-case one of them, which is \
                 what the common table exists to prevent. `1` keeps its ordinary meaning here \
                 (discipline 52's usage errors; sem: SEM-gx-cli-569), so the two facts stop sharing a status.",
    },
    // ---- **E-M6-13** (req/38 §51 M6H4-1 adopted (a); sem: SEM-gx-cli-570) — hand 4 raised these two and hand 5 pays them ----
    ExitAddition {
        section: "`gx cancel`",
        code: DENIED,
        ruling: "🔴 **E-M6-13**, req/38 §51 M6H4-1 adopted (a): \"add exit **2** to cancel/escalation's \
                 state-machine refusal (§1.2's column is an excerpt; applying the rest of M6-25's \
                 reading to the remaining 2 verbs). The implementation is the first hand from hand 5 \
                 onward to take this step\" (sem: SEM-gx-cli-571). 43 T-7's from-set stops before `Committing` and its guard is \
                 \"before reaching `Committing`\", so a cancel of a committed row is the state machine refusing \
                 — none of §1.4's three meanings for 1 (invalid input / internal error / adapter error). \
                 E-M6-1's Draft refusal stays on **1**, because a draft has no seat in T-7 at all \
                 and naming one is \"invalid input\" (sem: SEM-gx-cli-572) in the ordinary sense.",
    },
    ExitAddition {
        section: "`gx escalation approve` / `gx escalation reject`",
        code: DENIED,
        ruling: "🔴 **E-M6-13**, the same reading one verb along. INV-S6 is \"`Escalated` does not \
                 auto-transition to `Admitted`/`Denied` without going through T-5/T-5b's signed \
                 human-ruling receipt\" (sem: SEM-gx-cli-573) and its \
                 mirror is that a ruling may not be filed against a transformation nobody escalated; \
                 44 §1.2 gave that 1 beside \"the arbiter key is invalid\" (sem: SEM-gx-cli-574), which is a genuine invalid input and a \
                 different event. Splitting them is what makes either number readable: a script \
                 retrying on 1 (fix your key) must not retry on 2 (the row is not escalated).",
    },
    ExitAddition {
        section: "`gx undo`",
        code: DENIED,
        ruling: "M6-25 adopted (a)+(c) (sem: SEM-gx-cli-575). §1.2 gives `undo` 0/1/3/5/6 and no seat for \"refused (denied)\", \
                 while AC-040's second case is exactly that — \"in the case where the invariant/policy \
                 corresponding to T_u is deliberately set to Deny, T_u fails to reach Committed and \
                 stays Denied\" — and the engine \
                 has implemented it since M5. §1.4's 2 is \"refused (denied)\" and §1.2's list is read \
                 as an excerpt; folding a refused undo into 1 would give \"could not run\" and \"the \
                 gate said no\" one face, which is M4H4-2's standing refusal.",
    },
    ExitAddition {
        section: "`gx undo`",
        code: ESCALATED,
        ruling: "The same reading, one verdict along. An undo whose own inverse cannot be built \
                 reaches `Escalated` through E-M3-4 (the fs adapter answers `Ok(None)` above \
                 `MAX_INVERSE_PAYLOAD_BYTES`), and §1.4's 4 is \"escalation (escalated, \
                 ESCALATION_PENDING)\" (sem: SEM-gx-cli-576) — the state the transformation is actually in. §1.2's list \
                 omits it for the same reason it omits 2: it is the list of the ways the *escrow* \
                 can fail, not of the ways the pipeline it drives can stop.",
    },
    // ---- 🔴 **M7 hand 2** (ruling #6; sem: SEM-gx-cli-577) — the key section gains two verbs, and they can miss ----
    ExitAddition {
        section: "`gx key gen` / `gx key list`",
        code: NOT_FOUND,
        ruling: "🔴 **M7 hand 2**, ruling #6 (req/98 §3-2: \"U-06/13 key rotation = adopted for M7\"; sem: SEM-gx-cli-578). 44 §1.2's key \
                 section lists `gen` and `list`, and neither can name a key that is not there — \
                 `gen` makes one and `list` answers with an empty store. `revoke` and `rotate` take \
                 a `--key-id` and need the **secret**, because a revocation is signed by the key it \
                 revokes, so \"there is no such key\" (sem: SEM-gx-cli-579) becomes reachable in this section for the first time. \
                 It is not-found, and **E-M6-24** settled that not-found is §1.4's **6** across this \
                 binary (M6-25's principle: §1.2's column is an excerpt). `crates/gx-cli/tests/key_lifecycle_cli.rs::\
                 revoking_a_key_the_store_does_not_hold_is_not_found` measures the status rather \
                 than leaving this table to assert about itself.",
    },
    // ---- 🔴 **v0.2.6 doc batch** (req/38 §72 item 1) — the same fact, under its own heading (sem: SEM-gx-cli-519) ----
    ExitAddition {
        section: "🔴 `gx key revoke` / `gx key rotate` (v0.2.1 addendum, 2026-08-13)",
        code: NOT_FOUND,
        ruling: "🔴 **v0.2.6 doc batch**, req/38 §72 item 1. When the addition above was written \
                 (M7 hand 2, 2026-08-12) 44 §1.2 had one key section — `gen`/`list` — and the \
                 addition named it because it was the only heading there was. The next day 44 §1.2 \
                 gained a **second** `####` section for `revoke`/`rotate` (v0.2.1 addendum; sem: SEM-gx-cli-580), the very \
                 verbs that make not-found reachable, and `SPEC_44_EXITS` grew a row for it \
                 (immediately above in this file) without a matching addition. This entry is the \
                 same fact, named under the heading it is actually about, so \
                 `crates/gx-cli/tests/exit_matrix_cli.rs`'s per-section comparison — which reads \
                 `SPEC_44_EXIT_ADDITIONS` by **exact** section string, one spec section at a time \
                 — finds it under both headings. Twin, not replacement: the entry above stays \
                 (no-delete) because it is still the true record of what M7 hand 2 measured on the \
                 day only one heading existed.",
    },
];

/// 🔴 This hand's four sections, as the implementation returns them.
///
/// `undo` carries seven statuses and 44 §1.2 writes five of them; the other two are
/// [`SPEC_44_EXIT_ADDITIONS`]. `policy` matches 44 exactly.
///
/// 🔴 **`cancel` and `escalation` no longer do, and that is E-M6-13** (req/38 §51 M6H4-1 adopted (a); sem: SEM-gx-cli-581).
/// Hand 4 wrote "**including the two places where a state-machine refusal wears \"error\"**, which
/// this hand raises as M6H4-1 rather than repairing" (sem: SEM-gx-cli-582); §51 ruled it and named the hand — "the implementation is the first hand from
/// hand 5 onward to take this step" (sem: SEM-gx-cli-583) — so both verbs now carry a **2** as well, declared in
/// [`SPEC_44_EXIT_ADDITIONS`] with its citation.
pub const HAND4_EXITS: [ExitRow; 17] = [
    // ---- `gx undo` (44 §1.2) --------------------------------------------------------------
    ExitRow {
        group: "undo",
        code: OK,
        meaning: "0=success (new Transformation is Committed, T_o is Superseded) (sem: SEM-gx-cli-584)",
        common_table_note: "agrees with §1.4's \"the target state was reached\" (sem: SEM-gx-cli-584)",
    },
    ExitRow {
        group: "undo",
        code: ERROR,
        meaning: "1=unable to execute because `InverseStatus` is `Unavailable`/`Expired`/`Consumed` (sem: SEM-gx-cli-585)",
        common_table_note: "agrees; a second undo of one commit lands here through 42 §3.12's \
                            `Consumed`, which is what makes \"only once\" a fact (sem: SEM-gx-cli-586)",
    },
    ExitRow {
        group: "undo",
        code: DENIED,
        meaning: "2=the undo itself was refused by the gate (AC-040's second case) (sem: SEM-gx-cli-587)",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. 43 §5-2: \
                            \"an undo is not exempt from verification either\" (sem: SEM-gx-cli-588), so an undo has a verdict and a \
                            verdict can be `Deny`",
    },
    ExitRow {
        group: "undo",
        code: PRECONDITION_CHANGED,
        meaning: "3=commit failure (synonymous with `gx commit`): Aborted(PreconditionChanged) (sem: SEM-gx-cli-589)",
        common_table_note: "agrees with §1.4's \"CON-2 CAS failure\" (sem: SEM-gx-cli-589)",
    },
    ExitRow {
        group: "undo",
        code: ESCALATED,
        meaning: "4=the undo remains Escalated (awaiting human ruling) (sem: SEM-gx-cli-590)",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`",
    },
    ExitRow {
        group: "undo",
        code: APPLY_FAILED,
        meaning: "5=commit failure (synonymous with `gx commit`): Aborted(ApplyFailed) (sem: SEM-gx-cli-591)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "undo",
        code: NOT_FOUND,
        meaning: "6=not-found (sem: SEM-gx-cli-592)",
        common_table_note: "agrees; a transformation this `.gx/` never committed, and a committed \
                            one whose draft is gone, both land here (M6H4-5)",
    },
    // ---- `gx cancel` (44 §1.2) ------------------------------------------------------------
    ExitRow {
        group: "cancel",
        code: OK,
        meaning: "0=success (Aborted(OwnerCancelled)) (sem: SEM-gx-cli-593)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "cancel",
        code: ERROR,
        meaning: "1=insufficient permission, or an unexecutable state (already Committing or later, a terminal state) (sem: SEM-gx-cli-594)",
        common_table_note:
            "🔴 **E-M6-13 moved the state-machine half of this row to 2.** What is \
                            left on 1 is what §1.4's 1 actually means: **E-M6-1's Draft refusal** \
                            (a draft has no seat in 43 T-7, so naming one is \"invalid input\") (sem: SEM-gx-cli-595) and \
                            `--actor-key`, which selects nothing (M6H4-3)",
    },
    ExitRow {
        group: "cancel",
        code: DENIED,
        meaning: "2=refused by the state machine (already `Committing` or later, a terminal state) (sem: SEM-gx-cli-596)",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. E-M6-13 \
                            (req/38 §51 M6H4-1 adopted (a)). 43 T-7's guard is \"before reaching `Committing`\" (sem: SEM-gx-cli-597) and \
                            a row past it is refused **by the state machine**, which is §1.4's \
                            \"refused (denied)\" (sem: SEM-gx-cli-598)",
    },
    ExitRow {
        group: "cancel",
        code: NOT_FOUND,
        meaning: "6=not-found (sem: SEM-gx-cli-599)",
        common_table_note: "agrees",
    },
    // ---- `gx escalation approve` / `gx escalation reject` (44 §1.2) -----------------------
    ExitRow {
        group: "escalation",
        code: OK,
        meaning: "0=success (Admitted or Denied) (sem: SEM-gx-cli-600)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "escalation",
        code: ERROR,
        meaning: "1=invalid arbiter key (including a missing `--actor-key`: M6H4-6) (sem: SEM-gx-cli-601)",
        common_table_note:
                            "🔴 **E-M6-13 split this row.** 44 §1.2 wrote \"the arbiter key is invalid, or the target \
                            is not `Escalated`\" (sem: SEM-gx-cli-602) as one status; the first half is a genuine \
                            \"invalid input\" (sem: SEM-gx-cli-602) and stays here, the second half is the state machine and \
                            moved to 2. A script retrying on 1 fixes a key; one retrying on 2 would \
                            be retrying a refusal that will never change",
    },
    ExitRow {
        group: "escalation",
        code: DENIED,
        meaning: "2=the target is not `Escalated` (refused by the state machine) (sem: SEM-gx-cli-603)",
        common_table_note: "🔴 **not in §1.2's list** — see `SPEC_44_EXIT_ADDITIONS`. E-M6-13. \
                            INV-S6's mirror: a ruling may not be filed against a transformation \
                            nobody escalated",
    },
    ExitRow {
        group: "escalation",
        code: NOT_FOUND,
        meaning: "6=not-found (unknown ticket) (sem: SEM-gx-cli-604)",
        common_table_note: "agrees. 🔴 This is the status M6-04 made reachable at all: before \
                            `Engine::transformation_of_ticket` a `<TICKET_ID>` resolved to nothing \
                            **whatever it named**",
    },
    // ---- `gx policy lint` / `gx policy test` (44 §1.2) ------------------------------------
    ExitRow {
        group: "policy",
        code: OK,
        meaning: "0=no problems (sem: SEM-gx-cli-605)",
        common_table_note: "agrees. A **warning** (an empty policy set, an empty invariant \
                            registry) is reported in the JSON and does not move the status: 44 \
                            §1.2's 1 is \"a lint/test error exists\" (sem: SEM-gx-cli-606) and a warning is not an error",
    },
    ExitRow {
        group: "policy",
        code: ERROR,
        meaning: "1=a lint/test error exists (sem: SEM-gx-cli-607)",
        common_table_note: "agrees. A pack that will not parse and a scenario whose expectation \
                            was not met both land here — 44 §1.2 gives this command two statuses \
                            and the JSON is where the difference survives (Λ4)",
    },
];

/// 🔴 **P3's two sections, as the implementation returns them** (v0.3-d, `req/159` §D).
///
/// This table exists because `crates/gx-cli/tests/exit_matrix_cli.rs`'s own scoping note promised
/// it: when P3 grew 44 §1.2 to sixteen sections (v0.2.6, req/38 §72 assignment 5; sem: SEM-gx-cli-608), `gx wrap` and
/// `gx verdict-checkpoint` were added to [`SPEC_44_EXITS`] **without** a matching implementation
/// declaration — "doing so would need a new `HAND_P3_EXITS`-shaped declaration of what the P3 (sem: SEM-gx-cli-609)
/// binary actually returns, machine-checked against `codes_of()` the way `HAND2_EXITS` /
/// `HAND3_EXITS` / `HAND4_EXITS` are — a new piece of exit-contract infrastructure, not a doc
/// sync" (sem: SEM-gx-cli-609). That sentence was carried as v0.2.7 candidate-box #2 (sem: SEM-gx-cli-610) → carried to v0.2 (`req/141`) → carried to v0.3
/// (`req/38` §83, `req/157` §2-8(d)) and is paid here, so the chain 44 → [`SPEC_44_EXITS`] →
/// what the binary returns closes over all fifteen implemented sections rather than thirteen.
///
/// The codes are read from the implementation, not asserted about it:
///
/// * `wrap`: `crates/gx-cli/src/wrap.rs::run` returns `Outcome::ok` (0) for a completed proxy
///   session; every refusal on the way in (`<server cmd>` missing, handshake failure, an
///   `--otel-file` endpoint, a `--restore` with no `=`) is `Error::Usage` → 1 (discipline 52; sem: SEM-gx-cli-611);
///   `--check-config` with residual direct entries is `Outcome::refused(…, VERIFY_FAILED)` in
///   main.rs` — 44 §1.4's v0.2.6 addendum made 7 a **kind** ("an offline, self-contained verification failure"; sem: SEM-gx-cli-612)
///   rather than `gx receipt verify`'s own number, and this is one of the three verbs it names.
/// * `verdict-checkpoint`: `crates/gx-cli/src/verdict.rs` returns `Outcome::ok` (0) for
///   `issue`/`list` and for a `verify` that holds; a `verify` that does not is
///   `Outcome::refused(…, VERIFY_FAILED)` (7); everything the engine cannot do (key unreadable,
///   document unparseable) is an `Error` → 1.
///
/// 🔴 Neither section carries 44 §1.4's 2/3/4/5/6, and that is a fact worth machine-checking in
/// both directions (the equality in `exit_matrix_cli.rs` is `implemented == spec ∪ additions`):
/// a wrap session's per-call refusals (Deny, Escalate) travel as tool results with
/// `isError: true` (ruling ③; sem: SEM-gx-cli-613) **inside** a session that exits 0, because the proxy's exit status
/// describes the session, not the last verdict — a proxy that exited 2 on a Deny would take a
/// whole agent session down for one refused call.
pub const HAND_P3_EXITS: [ExitRow; 9] = [
    // ---- 🔴 `gx wrap` (44 §1.2, v0.2.6 addendum; sem: SEM-gx-cli-614) ----------------------------------------------
    ExitRow {
        group: "wrap",
        code: OK,
        meaning: "0=success (the proxy session terminated normally, or --adopt-config succeeded) (sem: SEM-gx-cli-615)",
        common_table_note: "agrees with §1.4's \"the target state was reached\" (sem: SEM-gx-cli-615) — the state reached is a \
                            completed session, and per-call verdicts travel inside it as ruling ③'s \
                            isError results rather than as the process status",
    },
    ExitRow {
        group: "wrap",
        code: ERROR,
        meaning: "1=input error (missing server cmd, handshake failure, missing --server-name, etc.) (sem: SEM-gx-cli-616)",
        common_table_note: "agrees; discipline 52 (sem: SEM-gx-cli-616) sends every usage error here, including an OTLP \
                            endpoint offered to --otel-file (refused by name, NFR-017)",
    },
    ExitRow {
        group: "wrap",
        code: VERIFY_FAILED,
        meaning: "7=--check-config detected a surviving direct-call path (B-1's machine check) (sem: SEM-gx-cli-617)",
        common_table_note: "agrees with 44 §1.4's v0.2.6 addendum: 7 is the **kind** \"an offline, \
                            self-contained verification failure\" (sem: SEM-gx-cli-617) and no longer `gx receipt verify`'s own \
                            number — a config that still routes an agent straight to a server is \
                            a self-contained check that failed, not a state machine saying no. \
                            Measured live (v0.4-e, `req/38` §107 residue 4; sem: SEM-gx-cli-618) by `tests/exit_map.rs`'s \
                            check-config pair — both failing halves exit 7 with the residual \
                            entry named, and the adopt-then-check road exits 0 — so this row \
                            stopped being the table's one source-reading-only 7",
    },
    // ---- 🔴 `gx verdict-checkpoint issue|verify|list` (44 §1.2, v0.2.6 addendum; sem: SEM-gx-cli-619) --------------
    ExitRow {
        group: "verdict-checkpoint",
        code: OK,
        meaning: "0=success (issue/list), valid (verify) (sem: SEM-gx-cli-620)",
        common_table_note: "agrees with §1.4's \"the target state was reached (…valid, etc.)\" (sem: SEM-gx-cli-620) — the parenthesis \
                            names this very verb's reading",
    },
    ExitRow {
        group: "verdict-checkpoint",
        code: ERROR,
        meaning: "1=input error (key unreadable, document unparseable, engine unable to sign/append, etc.) (sem: SEM-gx-cli-621)",
        common_table_note: "agrees",
    },
    ExitRow {
        group: "verdict-checkpoint",
        code: VERIFY_FAILED,
        meaning: "7=verify is invalid (a mismatch in signature/continuity/ledger binding/recomputation) (sem: SEM-gx-cli-622)",
        common_table_note: "agrees with the same v0.2.6 addendum (sem: SEM-gx-cli-622) as `wrap`'s 7: an offline, \
                            self-contained verification that did not hold. AC-P3-10's tamper arm \
                            (`tools/e2e_p3.sh`: one bit of the tally) measures this exit live",
    },
    // ---- 🔴 `gx repair` (44 §1.2, v0.4-p addendum, DR-43-8; `req/38` §160 ruling 2) -------------
    ExitRow {
        group: "repair",
        code: OK,
        meaning: "0=this project's journal and ledger describe one tree again, so it can be written to",
        common_table_note: "agrees with §1.4's \"the target state was reached\" — the state reached \
                            is a project the `LEDGER_DISAGREES` gate no longer refuses. Note what \
                            it is **not**: a claim that nothing was lost. A ledger whose torn tail \
                            was quarantined and cut agrees with its journal about a *shorter* tree, \
                            and the report says how many bytes went where (DR-43-7)",
    },
    ExitRow {
        group: "repair",
        code: ERROR,
        meaning: "1=the two files still describe different trees (or another gx holds the lock)",
        common_table_note: "agrees with §1.4's \"unable to execute\" — which is exactly what every \
                            write verb on this project answers, said by the one verb that looked. \
                            🔴 The number is deliberately the same for \"the repair ran and did not \
                            finish the job\" and \"the repair could not start\": both mean the next \
                            write will still be refused, which is the only thing a `&&` chain can \
                            act on. Which one it was is in the JSON on stdout (`repaired`, \
                            `ledger_agrees_after`, `remedy`), because 44 §1.3 puts the answer there \
                            and the status carries only the branch",
    },
    ExitRow {
        group: "repair",
        code: NOT_FOUND,
        meaning: "6=there is no `.gx/` at the named path",
        common_table_note: "agrees with §1.4's \"the named ID is absent\", read one noun up as \
                            `Layout::open`'s own refusal: a mistyped directory must not be repaired \
                            into existence any more than it may be submitted into one (E-M4-35)",
    },
];

/// 🔴 **R44 lane B, item 7** (`req/603` §8, `req/38` §369) — `gx attach`'s own exit set, measured
/// live rather than assumed from the shape of its errors (`req/637`'s empirical walk).
///
/// Three rows. A missing `--route-config` or `--declared` file takes [`NOT_FOUND`] through the same
/// `Error::Io` / `ErrorKind::NotFound` arm every other verb's read side does. A malformed route
/// document, a `--route-config` given without `--server-name`, a declared `.gx/` path occupied by
/// the wrong kind of thing (`LAYOUT_BLOCKED`) and a permission refusal on a path this operation
/// reads all land in [`ERROR`]'s default fold, because [`crate::Error::exit_code`] gives none of
/// them a code of its own. Nothing here reaches `Engine::verify` or `Engine::commit`, so
/// [`DENIED`], [`PRECONDITION_CHANGED`], [`ESCALATED`], [`APPLY_FAILED`] and [`VERIFY_FAILED`] are
/// structurally unreachable and are **not declared** — a code this verb can never return would be
/// the same invention discipline 52 forbids clap's 2, read from the other side.
pub const HAND_ATTACH_EXITS: [ExitRow; 3] = [
    ExitRow {
        group: "attach",
        code: OK,
        meaning: "0=placed (created rows, already-present rows, or both — an idempotent second \
                  attach is still 0)",
        common_table_note: "agrees with §1.4's \"the target state was reached\"",
    },
    ExitRow {
        group: "attach",
        code: ERROR,
        meaning:
            "1=input error (a `--route-config` document that will not parse, `--route-config` \
                  without `--server-name`, a declared `.gx/` path occupied by the wrong kind of \
                  thing (`LAYOUT_BLOCKED`), or a permission refusal on a path this operation reads)",
        common_table_note: "agrees with §1.4's \"error (invalid input / internal error / adapter \
                            error)\" — discipline 52's usage-error arm and `Error`'s default fold \
                            share the number, the same way every other verb's read side does",
    },
    ExitRow {
        group: "attach",
        code: NOT_FOUND,
        meaning: "6=not-found (`--route-config` or `--declared` names a file that is not there)",
        common_table_note: "agrees with §1.4's \"not-found\"",
    },
];

/// 🔴 **R44 lane F, item 6** (`req/603` §7, §10 lane F; `req/589` §4) — `gx receipt coverage`'s own
/// exit set, the per-code declaration `req/589` §4 named as the box the P-1b verb shipped without.
///
/// Three rows, and the point of the table is the code that is **not** here. `gx receipt coverage`
/// projects `gx_witness::ReceiptCoverage::of` over a payload; it checks neither signature nor
/// inclusion, so [`VERIFY_FAILED`] (44 §1.4's "offline verification failure") has nothing for it to
/// be about. `crates/gx-cli/tests/exit_map.rs`'s AC-2 arm measures that absence on the binary — a
/// tampered receipt whose `gx receipt verify` exits 7 has its `coverage` exit **0** — so this table
/// and that probe say the same thing from the two sides `codes_of()` closes over.
///
/// It is its **own** group rather than a row folded into `receipt`, because `show`/`verify`'s codes
/// are `[0, 1, 6, 7]` and coverage's are `[0, 1, 6]`: sharing the group would pair one section's
/// codes to another's in `exit_matrix_cli.rs` and turn `the_implemented_groups_return_the_codes_44_writes`
/// red for a true reason. That is the collision `req/589` §8 declined to resolve inside P-2 I's
/// coordinate budget, paid here with a separate group the way `req/159` §D paid P3's.
pub const HAND_COVERAGE_EXITS: [ExitRow; 3] = [
    ExitRow {
        group: "coverage",
        code: OK,
        meaning: "0=a coverage table was produced — an answer even when every row is `unknown` \
                  (`crates/gx-cli/src/receipt.rs`'s `coverage` returns `Outcome::ok` for it)",
        common_table_note:
            "agrees with §1.4's \"the target state was reached\" — the state reached \
                            is a projection computed, not a proof checked; the difference from \
                            `verify`'s 0 is exactly why this verb has no 7",
    },
    ExitRow {
        group: "coverage",
        code: ERROR,
        meaning: "1=input error (a receipt document that will not parse, a usage error, or a \
                  witness/malformed failure through `lib.rs`'s default arm)",
        common_table_note: "agrees with §1.4's \"error (invalid input / internal error / adapter \
                            error)\" — discipline 52's usage-error arm and `Error`'s default fold \
                            share the number, the same way every other verb's read side does",
    },
    ExitRow {
        group: "coverage",
        code: NOT_FOUND,
        meaning: "6=not-found (`Error::Io{NotFound}` — the receipt file is not there)",
        common_table_note: "agrees with §1.4's \"not-found\"",
    },
];

/// What a command produced: the JSON 44 §1.3 asks for, and the status it exits with.
///
/// One type rather than two return channels, because 44 §1.3 ties them together — "a command that (sem: SEM-gx-cli-623)
/// returns a single object emits, on stdout, **a single newline-terminated JSON**" — and a function that printed and a caller that
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

    /// A result that ran and answered "no" (sem: SEM-gx-cli-624): this object, and a status that is not 0.
    #[must_use]
    pub fn refused(json: serde_json::Value, code: u8) -> Self {
        Self { json, code }
    }

    /// 🔴 **R13 / `req/244` H-01** — put this answer on a destination, and say whether it landed.
    ///
    /// # Why this exists beside [`Outcome`] rather than inside `main`
    ///
    /// R12 made "a run that wrote and reported nothing" a compile error by making
    /// `repair::repair_and_report` return an `Outcome` rather than a `Result<Outcome>`, and 43
    /// §7.14 (b), `docs/LIMITS.md` v0.4-y and `req/243` §0 all said so to a buyer. `req/244` H-01
    /// measured the half that guarantee did not reach: the value was always produced, and the
    /// **delivery** was `println!`, which panics on a write error because Rust's `print!` family
    /// returns nothing at all. Three destinations reproduce it with no adversary — a reader that
    /// closed first, a filesystem with no room, a reader that takes one byte — and each ended at
    /// exit **101** with a Rust panic string where 44 §1.3's problem object belongs, over a
    /// `.gx/VERSION` that had been written.
    ///
    /// So the guarantee is finished here, in the form the finding names: **a report is delivered
    /// when it has been written through, not when it has been composed.** Every byte goes out
    /// through `write_all` and a `flush`, and every failure of either is a value the caller has to
    /// answer for. A `println!` in a verb is what `probes/doubt/tests/declaration_writer_doubt.rs`
    /// D-6 counts, so the road back is a red probe rather than a fourth audit finding.
    ///
    /// The serialisation failure is a value too. `print_json` used `text.unwrap_or_default()`,
    /// which printed an **empty line** and kept exit 0 (`req/244` L-03): no reachable input
    /// produces it today (`serde_json::Value` holds no NaN), and "no road was found" is not the
    /// same fact as "there is no road".
    ///
    /// # Errors
    /// [`std::io::Error`] from the serialiser (folded into [`std::io::ErrorKind::InvalidData`]),
    /// from any of the writes, or from the flush.
    pub fn emit(&self, out: &mut dyn std::io::Write, pretty: bool) -> std::io::Result<()> {
        let text = if pretty {
            serde_json::to_string_pretty(&self.json)
        } else {
            serde_json::to_string(&self.json)
        }
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // 44 §1.3: "a single newline-terminated JSON". One `write_all` for the body and one for the
        // newline rather than `writeln!`, because `writeln!`'s own error is the same value either
        // way and two calls are what the failure of each is reported against.
        out.write_all(text.as_bytes())?;
        out.write_all(b"\n")?;
        // 🔴 The flush is not decoration. A `LineWriter` over a pipe defers the write until a
        // newline **or** until the buffer fills, and a `BufWriter` over a file defers it until
        // drop — where the error has nowhere to go. `req/244` H-01's `> /dev/full` arm fails
        // exactly here on a buffered destination.
        out.flush()
    }
}
