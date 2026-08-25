// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-25, from the implementation's side** — the exit codes this binary returns are the ones
//! 44 §1.2 writes for the verbs it implements.
//!
//! `probes/doubt/tests/m6_exit_matrix.rs` compares [`gx_cli::exit::SPEC_44_EXITS`] against 44's
//! markdown, all fourteen sections. This compares the **two implementation tables** —
//! [`gx_cli::exit::HAND2_EXITS`] and [`gx_cli::exit::HAND3_EXITS`] — against that transcription, so
//! that the chain is closed: 44 → `SPEC_44_EXITS` → what the binary returns.
//!
//! Both halves are needed and neither is enough. A binary can return a status no section declares
//! (clap's 2 was that, and discipline 52 is why it no longer is; sem: SEM-gx-cli-2238), and a section can declare a status the
//! binary never reaches — which is not an error and **is** something a reader should be able to see,
//! so it is printed rather than asserted away.

mod support;

use std::collections::BTreeMap;

use gx_cli::exit::{
    ExitRow, EXIT_DIVERGENCES, HAND2_EXITS, HAND3_EXITS, HAND4_EXITS, HAND_ATTACH_EXITS,
    HAND_COVERAGE_EXITS, HAND_P3_EXITS, SPEC_44_EXITS, SPEC_44_EXIT_ADDITIONS,
};

/// The section of 44 §1.2 each implemented group belongs to.
///
/// A declared mapping rather than a string match, because 44's headings carry two verbs in six of
/// the fourteen ("`gx receipt show` / `gx receipt verify`" (sem: SEM-gx-cli-2239)) and a prefix test would silently pair
/// `gx log` with `gx log proof` — or with nothing. 🔴 **v0.2.6 doc batch** (req/38 §72 item 1): `key`
/// now names two sections — 44 §1.2 split `gx key revoke`/`gx key rotate` into its own `####`
/// heading (v0.2.1 addendum; sem: SEM-gx-cli-2240) after this mapping was written, and the CLI implements both under the same
/// Rust group (`crates/gx-cli/src/keys.rs`).
/// 🔴 **v0.3-d** (`req/159` §D, `HAND_P3_EXITS`): thirteen became fifteen. The two P3 sections
/// were declared in `SPEC_44_EXITS` at v0.2.6 with an explicit scoping note that pairing them
/// here "would need a new `HAND_P3_EXITS`-shaped declaration" (sem: SEM-gx-cli-2241) — that declaration now exists
/// (`crates/gx-cli/src/exit.rs`), so the pairing is made and `gx serve` is the whole of what
/// remains pending.
/// 🔴 **R44 lane B, item 7** (`req/603` §8, `req/38` §369): sixteen became seventeen when `gx
/// attach` got its own §1.2 section and `HAND_ATTACH_EXITS` gave it the same pairing every other
/// implemented section already has.
/// 🔴 **R44 lane F, item 6** (`req/603` §7, §10 lane F): seventeen became eighteen when `gx receipt
/// coverage` was paired to its own `coverage` group against `HAND_COVERAGE_EXITS`. P-2 I left it
/// pending on purpose — the `receipt` group's codes are `[0, 1, 6, 7]` and coverage's are
/// `[0, 1, 6]`, so folding it in would pair one section's codes to another's — and this lane pays
/// the separate group `req/589` §8 declined inside P-2 I's coordinate budget.
///
/// 🔴 **Three section strings below carried Japanese provenance markers until 2026-08-26**
/// (req/835 §4). They are exact-match twins of `SPEC_44_EXITS` in `crates/gx-cli/src/exit.rs`,
/// which itself quotes `req/spec/40-architecture/44-api-spec.md`'s `#### ` headings character for
/// character under `probes/doubt/tests/m6_exit_matrix.rs`'s verbatim comparison — so the canon
/// headings, exit.rs's literals and these twins moved to English in one commit, and the Japanese
/// originals sit verbatim in the maintainers' ledger.
const GROUP_SECTIONS: [(&str, &str); 18] = [
    ("attach", "`gx attach`"),
    ("submit", "`gx submit`"),
    ("plan", "`gx plan`"),
    ("verify", "`gx verify`"),
    ("commit", "`gx commit`"),
    ("undo", "`gx undo`"),
    ("cancel", "`gx cancel`"),
    (
        "escalation",
        "`gx escalation approve` / `gx escalation reject`",
    ),
    ("receipt", "`gx receipt show` / `gx receipt verify`"),
    // ---- 🔴 R44 lane F, item 6 (`req/603` §7, §10 lane F) — `gx receipt coverage`'s own group,
    // paired here at last against `HAND_COVERAGE_EXITS`. Beside `receipt` by adjacency to what it is
    // about, not folded into it: coverage's codes are `[0, 1, 6]` and `show`/`verify`'s are
    // `[0, 1, 6, 7]`, so one shared group would make the code cross-check red for a true reason ----
    ("coverage", "`gx receipt coverage`"),
    ("replay", "`gx replay`"),
    ("log", "`gx log proof` / `gx log consistency`"),
    ("key", "`gx key gen` / `gx key list`"),
    // ---- (sem: SEM-gx-cli-2242) exact-match twin of exit.rs SEM-gx-cli-516; English since 2026-08-26 (req/835 §4) ----
    (
        "key",
        "🔴 `gx key revoke` / `gx key rotate` (v0.2.1 addendum, 2026-08-13)",
    ),
    ("policy", "`gx policy lint` / `gx policy test`"),
    // ---- 🔴 v0.3-d (req/159 §D) — the two P3 sections, paired at last (sem: SEM-gx-cli-2243, SEM-gx-cli-2244) ----
    ("wrap", "🔴 `gx wrap` (v0.2.6 addendum, P3, req/119 §2)"),
    (
        "verdict-checkpoint",
        "🔴 `gx verdict-checkpoint issue` / `gx verdict-checkpoint verify` / `gx verdict-checkpoint list` (v0.2.6 addendum, P3/FR-M04, req/119 §4)",
    ),
    // ---- 🔴 R3 (`req/38` §160 ruling 2, DR-43-8) — the door beside the LEDGER_DISAGREES gate ----
    (
        "repair",
        "🔴 `gx repair` (v0.4-p addendum, 2026-08-16, **DR-43-8**, `req/38` §160 ruling 2, `req/222` H-06)",
    ),
];

/// The codes one group's rows declare, sorted and deduplicated.
fn codes_of(group: &str) -> Vec<u8> {
    let mut codes: Vec<u8> = HAND2_EXITS
        .iter()
        .chain(HAND3_EXITS.iter())
        .chain(HAND4_EXITS.iter())
        .chain(HAND_P3_EXITS.iter())
        .chain(HAND_ATTACH_EXITS.iter())
        .chain(HAND_COVERAGE_EXITS.iter())
        .filter(|row: &&ExitRow| row.group == group)
        .map(|row| row.code)
        .collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// 🔴 Every implemented group returns the codes 44 §1.2 gives its section, **plus the ruled ones**.
///
/// `implemented == spec ∪ additions`, and both directions matter. A code the spec writes and the
/// binary never returns is an exit a script cannot branch on; a code the binary returns and neither
/// the spec nor [`SPEC_44_EXIT_ADDITIONS`] declares is an invention — which is what clap's 2 was.
/// The additions are not a loophole: each one has to be written down with its ruling before this
/// passes, so "the list is an excerpt" (sem: SEM-gx-cli-2245) costs a citation rather than a shrug.
#[test]
fn the_implemented_groups_return_the_codes_44_writes() {
    let spec: BTreeMap<&str, &[u8]> = SPEC_44_EXITS.iter().copied().collect();
    let mut divergence = Vec::new();
    for (group, section) in GROUP_SECTIONS {
        let declared = codes_of(group);
        let base = spec
            .get(section)
            .unwrap_or_else(|| panic!("`SPEC_44_EXITS` has no row for {section}"));
        let mut expected: Vec<u8> = (*base).to_vec();
        let added: Vec<u8> = SPEC_44_EXIT_ADDITIONS
            .iter()
            .filter(|a| a.section == section)
            .map(|a| a.code)
            .collect();
        expected.extend(added.iter().copied());
        expected.sort_unstable();
        expected.dedup();
        println!("EXIT_GROUP {group}: SPEC_44={base:?} RULED_ADDITIONS={added:?} IMPLEMENTED={declared:?}");
        if declared != expected {
            divergence.push(format!(
                "{group}: 44 says {base:?} + ruled {added:?}, gx-cli returns {declared:?}"
            ));
        }
    }
    assert!(
        divergence.is_empty(),
        "the binary's exit table and 44 §1.2 disagree: {divergence:?}"
    );
}

/// 🔴 Every ruled addition names a real section of 44 §1.2 and a real status of §1.4, and says why.
#[test]
fn every_addition_to_44s_lists_carries_its_ruling() {
    let sections: Vec<&str> = SPEC_44_EXITS.iter().map(|(s, _)| *s).collect();
    println!("SPEC_44_EXIT_ADDITIONS={}", SPEC_44_EXIT_ADDITIONS.len());
    for addition in &SPEC_44_EXIT_ADDITIONS {
        println!(
            "    {} += {} ({} chars)",
            addition.section,
            addition.code,
            addition.ruling.len()
        );
        assert!(
            sections.contains(&addition.section),
            "{} is not a section of 44 §1.2",
            addition.section
        );
        assert!(
            addition.code <= 7,
            "44 §1.4 has eight statuses; {} is not one",
            addition.code
        );
        assert!(
            addition.ruling.len() > 80,
            "an addition without a ruling is a status somebody added: {}",
            addition.section
        );
    }
    assert!(
        !SPEC_44_EXIT_ADDITIONS.is_empty(),
        "M6-25's 2 on `gx undo` is one of them"
    );
}

/// 🔴 **44 §1.4's 2 is returned, and by exactly the two verbs that mean it.**
///
/// discipline 52 (req/38 §48 M6H1-1, E-M6-2) reserved the number for the state machine's "refused (denied)" (sem: SEM-gx-cli-2246)
/// while hand 2 could return nothing but reads, and hand 1's note said [`gx_cli::exit::DENIED`] was
/// "declared and never returned" (sem: SEM-gx-cli-2247). It is returned now — by `gx verify` on a `Verdict::Deny` and by
/// `gx commit` on a transformation the gate did not admit — and the value of a reserved number is
/// exactly that a script can now branch on it.
///
/// The negative half is `crates/gx-cli/tests/exit_map.rs`, which measures that **no usage error**
/// takes it. Both are needed: a reserved number nobody returns is a promise, and a number two
/// different events return is worse than an unreserved one.
#[test]
fn denied_is_returned_by_verify_and_commit_and_by_nothing_else() {
    let owners: Vec<&str> = HAND2_EXITS
        .iter()
        .chain(HAND3_EXITS.iter())
        .chain(HAND4_EXITS.iter())
        // v0.3-d: the P3 table is in the chain so that this stays a claim about the whole
        // surface — neither P3 verb owns a 2, and a wrap session's per-call Deny travels as a
        // tool result (ruling ③; sem: SEM-gx-cli-2248) rather than as the process status.
        .chain(HAND_P3_EXITS.iter())
        .filter(|row| row.code == gx_cli::exit::DENIED)
        .map(|row| row.group)
        .collect();
    println!("DENIED_RETURNED_BY={owners:?}");
    assert_eq!(
        owners,
        vec!["verify", "commit", "undo", "cancel", "escalation"],
        "44 §1.2 gives 2 to `gx verify` (\"2=Deny\") and `gx commit` (\"2=Deny, refused because not yet Admitted\") (sem: SEM-gx-cli-2249); \
         `gx undo` is M6-25 adopted (a)+(c)'s third (sem: SEM-gx-cli-2250), because 43 §5-2 makes an undo carry its own verdict; \
         and `cancel` / `escalation` are **E-M6-13**'s (req/38 §51 M6H4-1 adopted (a); sem: SEM-gx-cli-2251, implemented in \
         hand 5), where a state-machine refusal stopped wearing 44 §1.4's \"error\" (sem: SEM-gx-cli-2252). All five are \
         43 §3 saying no, which is what discipline 52 reserved the number for (sem: SEM-gx-cli-2253) — and no usage error takes \
         it, which is `exit_map.rs`'s half"
    );
}

/// 🔴 **Fifteen of the sixteen**, and the one left is hand 6's alone.
///
/// Printed and asserted, so that the count is a number in the report rather than a sentence. Hand 3
/// left five pending; hand 4 owns four of them and `gx serve` is the whole of what remains. 🔴
/// **v0.2.6 doc batch** (req/38 §72 item 1): 44 §1.2 grew a fourteenth section (`gx key revoke` /
/// `gx key rotate`, v0.2.1 addendum; sem: SEM-gx-cli-2254) after this test was written; `GROUP_SECTIONS` now carries it paired
/// with `gx key gen` / `gx key list` under the `key` group (both were already implemented, M7 hand
/// 2), so the count moves from thirteen to fourteen while `gx serve` stays the one pending section.
///
/// 🔴 **v0.2.6 doc batch, item 5** (req/38 §72 assignment 5; sem: SEM-gx-cli-2255): 44 §1.2 grew two more sections when P3
/// shipped — `gx wrap` and `gx verdict-checkpoint issue|verify|list`. Neither joined
/// `GROUP_SECTIONS` then: doing so would need a new `HAND_P3_EXITS`-shaped declaration of what the
/// P3 binary actually returns, machine-checked against `codes_of()` the way `HAND2_EXITS` /
/// `HAND3_EXITS` / `HAND4_EXITS` are — a new piece of exit-contract infrastructure, not a doc sync,
/// and out of that batch's scope (v0.2.7 candidate-box #2; sem: SEM-gx-cli-2256). The count stayed honest at thirteen and the
/// pending list held three.
///
/// 🔴 **v0.3-d** (`req/159` §D, carried v0.2.7 candidate-box #2 (sem: SEM-gx-cli-2257) → `req/141` → `req/38` §83 → here): the
/// declaration exists — `gx_cli::exit::HAND_P3_EXITS`, read by `codes_of()` beside the other three
/// tables — so both P3 sections join `GROUP_SECTIONS`, thirteen becomes fifteen, and the pending
/// list is `gx serve` alone. The old three-section assertion is this test's own history (git holds
/// it); the doc paragraphs above stay because the count's path — 13 → 14 → 13-of-16 → 15 — is the
/// record of *why* each move happened, and a resolved note that is deleted is one the next reader
/// rediscovers.
/// 🔴 **P-2 I** (`req/571` Part I, `req/38` §341 ruling (A); 2026-08-23): 44 §1.2 grew an
/// **eighteenth** section — `gx receipt coverage` — and it does **not** join `GROUP_SECTIONS`, so
/// the pending list becomes two and the count stays sixteen. That is a claim about declarations
/// and not about the verb: `gx receipt coverage` has been implemented and merged since P-1b
/// (`crates/gx-cli/src/receipt.rs`'s `coverage`), and this lane measures its exits directly
/// against the binary in `crates/gx-cli/tests/exit_map.rs` (`req/571` AC-2, five arms including
/// the one that watches 7 stay away). What is missing is the *per-code* declaration this test
/// pairs against — a `HAND_*_EXITS`-shaped table of what the binary returns, read by `codes_of()`.
///
/// It is missing deliberately. `receipt` already names a group whose codes are `[0, 1, 6, 7]`
/// (`show`/`verify`), and coverage's are `[0, 1, 6]`; pairing the new section to that group would
/// make `the_implemented_groups_return_the_codes_44_writes` red for a true reason. The honest
/// alternatives are a new group with its own hand table, which is new exit-contract infrastructure
/// rather than a doc sync — the exact shape the v0.2.6 batch declined for P3 and `req/159` §D paid
/// two versions later — or this: name it pending and say why. The pending list is therefore
/// `gx receipt coverage` **and** `gx serve`, and the entry that would remove the first is a
/// `HAND_*_EXITS` row, not another section heading.
#[test]
fn eighteen_of_the_nineteen_sections_are_implemented() {
    let implemented: Vec<&str> = GROUP_SECTIONS.iter().map(|(_, s)| *s).collect();
    let mut pending: Vec<&str> = SPEC_44_EXITS
        .iter()
        .map(|(section, _)| *section)
        .filter(|section| !implemented.contains(section))
        .collect();
    pending.sort_unstable();
    println!(
        "SECTIONS_IMPLEMENTED={} SECTIONS_PENDING={} ({pending:?})",
        implemented.len(),
        pending.len()
    );
    assert_eq!(
        pending,
        vec!["`gx serve`"],
        "hand 4 implements undo/cancel/escalation/policy; hand 6 owns serve; v0.3-d paired P3's \
         gx wrap and gx verdict-checkpoint through HAND_P3_EXITS (req/159 §D; the scoping note \
         that deferred them is quoted on that table). 🔴 R3 (`req/38` §160 ruling 2) added \
         `gx repair` and paired it in the same breath: a verb whose whole subject is a project \
         nobody can write to has no business being the one section whose exits nobody declared. \
         🔴 P-2 I (`req/571` Part I, `req/38` §341 ruling (A)) added `gx receipt coverage` to 44 \
         §1.2 and to `SPEC_44_EXITS` **without** pairing it here at first, because the `receipt` \
         group's codes are `show`/`verify`'s and coverage's are a different set, so the pairing \
         needed a hand table of its own. 🔴 R44 lane F (`req/603` §7, §10 lane F) is that table — \
         `HAND_COVERAGE_EXITS`, read by `codes_of()` beside the other five and paired to \
         `coverage`'s own group here — so `gx receipt coverage` leaves the pending list and \
         `gx serve` is the whole of what remains. The old two-section assertion is this test's \
         own history (git holds it); the doc paragraphs stay because the count's path is the \
         record of why each move happened"
    );
}

/// Every declared divergence carries a reading somebody can disagree with.
///
/// The structural half is `m6_exit_matrix.rs`'s (a real section, a real status). This is the half a
/// parser cannot check and a reviewer can: an empty explanation would make the table a list of
/// disagreements nobody characterised, which is the shape "write down what you folded" (sem: SEM-gx-cli-2258) exists to prevent.
#[test]
fn every_divergence_says_what_it_is() {
    println!("EXIT_DIVERGENCES={}", EXIT_DIVERGENCES.len());
    for row in &EXIT_DIVERGENCES {
        println!(
            "    {} -> {} ({} chars)",
            row.section,
            row.common_table_code,
            row.reading.len()
        );
        assert!(
            row.reading.len() > 80,
            "a divergence with a one-line note is a divergence nobody thought about: {}",
            row.section
        );
    }
    assert!(
        EXIT_DIVERGENCES.len() >= 2,
        "M6-25 and M6H2-5 are two of them"
    );
}
