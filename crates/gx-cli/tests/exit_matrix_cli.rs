//! 🔴 **M6-25, from the implementation's side** — the exit codes this binary returns are the ones
//! 44 §1.2 writes for the verbs it implements.
//!
//! `probes/doubt/tests/m6_exit_matrix.rs` compares [`gx_cli::exit::SPEC_44_EXITS`] against 44's
//! markdown, all thirteen sections. This compares the **two implementation tables** —
//! [`gx_cli::exit::HAND2_EXITS`] and [`gx_cli::exit::HAND3_EXITS`] — against that transcription, so
//! that the chain is closed: 44 → `SPEC_44_EXITS` → what the binary returns.
//!
//! Both halves are needed and neither is enough. A binary can return a status no section declares
//! (clap's 2 was that, and 規律52 is why it no longer is), and a section can declare a status the
//! binary never reaches — which is not an error and **is** something a reader should be able to see,
//! so it is printed rather than asserted away.

mod support;

use std::collections::BTreeMap;

use gx_cli::exit::{
    ExitRow, EXIT_DIVERGENCES, HAND2_EXITS, HAND3_EXITS, HAND4_EXITS, SPEC_44_EXITS,
    SPEC_44_EXIT_ADDITIONS,
};

/// The section of 44 §1.2 each implemented group belongs to.
///
/// A declared mapping rather than a string match, because 44's headings carry two verbs in five of
/// the thirteen (「`gx receipt show` / `gx receipt verify`」) and a prefix test would silently pair
/// `gx log` with `gx log proof` — or with nothing.
const GROUP_SECTIONS: [(&str, &str); 12] = [
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
    ("replay", "`gx replay`"),
    ("log", "`gx log proof` / `gx log consistency`"),
    ("key", "`gx key gen` / `gx key list`"),
    ("policy", "`gx policy lint` / `gx policy test`"),
];

/// The codes one group's rows declare, sorted and deduplicated.
fn codes_of(group: &str) -> Vec<u8> {
    let mut codes: Vec<u8> = HAND2_EXITS
        .iter()
        .chain(HAND3_EXITS.iter())
        .chain(HAND4_EXITS.iter())
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
/// passes, so 「the list is an excerpt」 costs a citation rather than a shrug.
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
/// 規律52 (req/38 §48 M6H1-1, E-M6-2) reserved the number for the state machine's 「拒否（denied）」
/// while hand 2 could return nothing but reads, and hand 1's note said [`gx_cli::exit::DENIED`] was
/// 「declared and never returned」. It is returned now — by `gx verify` on a `Verdict::Deny` and by
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
        .filter(|row| row.code == gx_cli::exit::DENIED)
        .map(|row| row.group)
        .collect();
    println!("DENIED_RETURNED_BY={owners:?}");
    assert_eq!(
        owners,
        vec!["verify", "commit", "undo", "cancel", "escalation"],
        "44 §1.2 gives 2 to `gx verify` (「2=Deny」) and `gx commit` (「2=Denyで未Admitのため拒否」); \
         `gx undo` is M6-25 採(a)+(c)'s third, because 43 §5-2 makes an undo carry its own verdict; \
         and `cancel` / `escalation` are **E-M6-13**'s (req/38 §51 M6H4-1 採(a), implemented in \
         hand 5), where a state-machine refusal stopped wearing 44 §1.4's 「エラー」. All five are \
         43 §3 saying no, which is what 規律52 reserved the number for — and no usage error takes \
         it, which is `exit_map.rs`'s half"
    );
}

/// 🔴 **Twelve of the thirteen**, and the one left is hand 6's.
///
/// Printed and asserted, so that the count is a number in the report rather than a sentence. Hand 3
/// left five pending; hand 4 owns four of them and `gx serve` is the whole of what remains.
#[test]
fn twelve_of_the_thirteen_sections_are_implemented() {
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
        "hand 4 implements undo/cancel/escalation/policy; hand 6 owns serve"
    );
}

/// Every declared divergence carries a reading somebody can disagree with.
///
/// The structural half is `m6_exit_matrix.rs`'s (a real section, a real status). This is the half a
/// parser cannot check and a reviewer can: an empty explanation would make the table a list of
/// disagreements nobody characterised, which is the shape 「畳んだ事を書け」 exists to prevent.
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
