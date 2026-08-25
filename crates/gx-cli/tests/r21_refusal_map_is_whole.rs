// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R21 / `req/306` §1 item 1** — the map is whole, its words exist, and its exits agree.
//!
//! The sibling of `r21_refusal_semantics.rs`. That file is the red instrument: it names only
//! symbols the pre-repair tree has, so its failures were assertions rather than missing symbols.
//! This one names `gx_cli::REFUSAL_MAP`, so it cannot compile against `1d1c145` and it is
//! **green-only by construction** — said out loud, because a suite whose red is a compiler error
//! measures nothing (`r20_refusal_vocabulary_is_whole.rs` drew the same line for the same reason).
//!
//! What it is *for* is the next lane rather than this one. Three properties, none of which a
//! reader can hold in their head:
//!
//! 1. **The census both ways.** Every row of the map is reachable from some `Error`, and every
//!    `Error` this crate can build reaches a row. A row nothing answers with is a word that
//!    accumulates; an arm with no row does not compile.
//! 2. **The vocabulary is 44's.** Every code is one of 44 §2.3's twelve or one of `gx-api`'s
//!    `RULED_ADDITIONS`, read out of `gx-api`'s own tables rather than transcribed — the shape
//!    `exit_map.rs` uses against the specification's markdown, one layer over.
//! 3. 🔴 **The code and the exit agree.** This is the property R21 needed and did not have. Every
//!    `gx_code` carries a declared `cli_exit` in 44 §2.3's third column, and every `Error` carries
//!    an `exit_code()`. Nothing had ever compared them, which is exactly how a lane could "fix" a
//!    refusal's word and leave a script branching on a number that now disagrees with it. R21
//!    assigns two words on the strength of this equality (`ADAPTER_ERROR` and
//!    `INVERSE_UNAVAILABLE` both declare `cli_exit` 1, the number those two roads already
//!    returned), so the equality is the reason no exit status moves in this lane and it belongs in
//!    a probe rather than in a paragraph.

mod support;

use gx_cli::{Error, RefusalClass, RefusalRow, REFUSAL_MAP};

/// 44 §2.3's third column for one code, from `gx-api`'s two tables and from nowhere else.
fn declared_cli_exit(code: &str) -> u8 {
    gx_api::gx_code::GX_CODES
        .iter()
        .chain(gx_api::gx_code::RULED_ADDITIONS.iter())
        .find(|row| row.code == code)
        .unwrap_or_else(|| {
            panic!(
                "`{code}` is in neither 44 §2.3's twelve nor `gx-api`'s `RULED_ADDITIONS`. A CLI \
                 that invented a code would be inventing a word the HTTP surface has to share (44 \
                 §0), and the SDK's census (`sdk/typescript/test/gx_code_census.test.mjs`) would \
                 not know it either"
            )
        })
        .cli_exit
}

/// One `Error` per row of the map, in the map's own order.
///
/// Values rather than descriptions: a table compared against another table proves the two tables
/// agree, and what has to be true is that the **binary** does what the table says.
fn one_error_per_row() -> Vec<Error> {
    vec![
        // ROW_LEDGER_DISAGREES
        Error::Malformed {
            what: "project",
            path: "/tmp/p/.gx".into(),
            detail: "the frontier is 3 and the ledger holds 0 leaves".into(),
        },
        // ROW_INVERSE_UNAVAILABLE
        Error::InverseUnavailable {
            id: "gx1:aaa".into(),
            status: "BodyMissing",
        },
        // ROW_ESCROW_ABSENT
        Error::Engine(gx_engine::Error::NotFound {
            what: "escrowed inverse (42 §3.12 Unavailable: `invert` answered None)",
            id: "TransformationId(Cid(opaque))".into(),
        }),
        // ROW_ADAPTER_ERROR
        Error::Engine(gx_engine::Error::Adapter {
            action: "snapshot",
            detail: "the substrate would not answer for \"notes.md\": not a position from the \
                     root; v0.1 names positions absolutely (ASM-69-3)"
                .into(),
        }),
        // ROW_DECLARATION_UNREADABLE
        Error::Declaration {
            path: "/tmp/p/.gx/VERSION".into(),
            form: "not text".into(),
            remedy: "write these two lines".into(),
        },
        // ROW_DECLARATION_ABSENT
        Error::DeclarationAbsent {
            path: "/tmp/p/.gx/VERSION".into(),
            remedy: "`gx repair --yes`".into(),
        },
        // ROW_CONFIG_ABSENT
        Error::ConfigAbsent {
            path: "/tmp/p/.gx/config.toml".into(),
            remedy: "put `engine_signing_keyid` back".into(),
        },
        // ROW_JOURNAL_ABSENT
        Error::JournalAbsent {
            path: "/tmp/p/.gx/ledger/journal".into(),
            remedy: "restore the backup".into(),
        },
        // ROW_OUTPUT_FAILED
        Error::OutputFailed {
            detail: "Broken pipe (os error 32)".into(),
        },
        // ROW_HISTORY_LOST
        Error::HistoryLost {
            path: "/tmp/p/.gx".into(),
            evidence: "`.gx/index/` holds 2 entries".into(),
            remedy: "restore the backup".into(),
        },
        // ROW_LAYOUT_BLOCKED
        Error::LayoutBlocked {
            path: "/tmp/p/.gx/repair".into(),
            rel: "repair".into(),
            // 🔴 **R40 / `req/38` §328 ruling 2 ②** — the clause the sentence used to hard-code.
            expected: "`.gx/repair` has to be a directory".into(),
            found: "a file".into(),
            remedy: "move it".into(),
        },
        // ROW_JOURNAL_UNREADABLE (DR-B, `req/38` §337, `req/565` §3)
        Error::JournalUnreadable {
            path: "/tmp/p/.gx/ledger/journal".into(),
            reason: "PermissionDenied: Permission denied (os error 13)".into(),
            remedy: "fix the file mode and run the verb again".into(),
        },
        // ROW_VALIDATION_ERROR
        Error::Usage {
            detail: "--context takes one of 44 §1.2's six names".into(),
        },
        // ROW_NOT_FOUND
        Error::NotFound {
            what: "receipt",
            id: "gx1:aaa".into(),
        },
        // ROW_BUSY
        Error::Engine(gx_engine::Error::Busy {
            path: std::path::PathBuf::from("/tmp/p/.gx/LOCK"),
            holder: "1 gx submit".into(),
        }),
        // ROW_WORLD_MOVED
        Error::Engine(gx_engine::Error::WorldMoved {
            id: "gx1:aaa".into(),
            expected: support::fingerprint(1),
            found: support::fingerprint(2),
            scope: "/tmp/p/target.txt".into(),
        }),
        // ROW_WITNESS_MISSING
        Error::Engine(gx_engine::Error::WitnessMissing {
            id: "gx1:aaa".into(),
            reason: "no-receipt",
            remedy: "this deployment keeps no receipt archive",
        }),
        // ROW_INTERNAL
        Error::Io {
            action: "open",
            path: "/tmp/p/.gx/ledger".into(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        },
    ]
}

/// 🔴 Property 1 — every row is answered with, and every `Error` built here lands on its own row.
#[test]
fn the_census_closes_in_both_directions() {
    let errors = one_error_per_row();
    assert_eq!(
        errors.len(),
        REFUSAL_MAP.len(),
        "one value per row, in the map's order: {} values against {} rows",
        errors.len(),
        REFUSAL_MAP.len()
    );
    for (error, row) in errors.iter().zip(REFUSAL_MAP.iter()) {
        let answered = error.refusal();
        println!(
            "ROW arm={:?} class={:?} code={} exit={}",
            answered.arm,
            answered.class,
            answered.code,
            error.exit_code()
        );
        assert_eq!(
            &answered, row,
            "the value built for `{}` answered with `{}` instead — either the guard order in \
             `Error::refusal` moved or the value no longer drives the arm it was written for",
            row.arm, answered.arm
        );
    }
    let mut arms: Vec<&str> = REFUSAL_MAP.iter().map(|r| r.arm).collect();
    arms.sort_unstable();
    let before = arms.len();
    arms.dedup();
    assert_eq!(before, arms.len(), "two rows claim one arm: {arms:?}");
    println!("REFUSAL_MAP_ROWS={}", REFUSAL_MAP.len());
}

/// 🔴 Property 2 — the vocabulary is 44's, and 44 is read rather than remembered.
#[test]
fn every_word_in_the_map_is_one_this_system_already_has() {
    let mut codes: Vec<&str> = REFUSAL_MAP.iter().map(|r| r.code).collect();
    codes.sort_unstable();
    codes.dedup();
    println!("MAP_CODES={} {codes:?}", codes.len());
    for code in &codes {
        // Panics with the sentence a lane minting a word needs to read.
        let _ = declared_cli_exit(code);
    }
    // 🔴 The two rows that share a word, named rather than tolerated.
    //
    // `PRECONDITION_CHANGED` is R3's ruled "one code, two titles" (the client branches on the code,
    // the person reads the title). `INVERSE_UNAVAILABLE` is the other shape and the opposite
    // instruction: one code **and one title**, because "the escrow's status is not Available" is
    // one refusal whichever of R9's road and R21's road reached it, and `BUSY_TITLE`'s rule is that
    // one refusal does not get two names.
    for code in ["PRECONDITION_CHANGED", "INVERSE_UNAVAILABLE"] {
        let rows: Vec<&RefusalRow> = REFUSAL_MAP.iter().filter(|r| r.code == code).collect();
        assert_eq!(rows.len(), 2, "`{code}` is expected on exactly two rows");
        let titles: Vec<&str> = rows.iter().map(|r| r.title).collect();
        println!("SHARED_CODE {code} titles={titles:?}");
        if code == "INVERSE_UNAVAILABLE" {
            assert_eq!(titles[0], titles[1], "one refusal, one name (`BUSY_TITLE`)");
        } else {
            assert_ne!(
                titles[0], titles[1],
                "R3 ruled two titles here: \"the world moved\" and \"this process cannot tell \
                 whether the world moved\" are different facts"
            );
        }
    }
}

/// 🔴 Property 3 — the word a refusal wears declares the number it leaves with.
///
/// The equality R21 rests on. 44 §2.3's third column is the `cli_exit` for each code and
/// `Error::exit_code` is what the process returns; nothing had ever compared them on this face.
/// It is what makes "no exit status moves in this lane" a measurement: both words R21 assigns
/// declare **1**, and **1** is what `req/304` measured those two roads returning before the repair.
#[test]
fn the_code_a_refusal_wears_declares_the_exit_it_leaves_with() {
    let mut disagreements = Vec::new();
    for (error, row) in one_error_per_row().iter().zip(REFUSAL_MAP.iter()) {
        let declared = declared_cli_exit(row.code);
        let actual = error.exit_code();
        println!(
            "EXIT_AGREEMENT code={} declared={declared} actual={actual} arm={}",
            row.code, row.arm
        );
        if declared != actual {
            disagreements.push(format!(
                "{} ({}): 44 §2.3 declares cli_exit {declared}, this binary exits {actual}",
                row.code, row.arm
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "🔴 a code and an exit that disagree on one refusal is worse than either alone: a script \
         branching on the number and a monitor keying on the word would take different actions on \
         one event.\n{}",
        disagreements.join("\n")
    );
}

/// 🔴 Property 4 — the classes, counted, with the residual named.
///
/// `req/306` §1 item 1 asks for three classes. The table has a fourth, and this arm is where the
/// fourth is a number rather than a hedge: `RefusalClass::Operational` holds `BUSY` and
/// `OUTPUT_FAILED`, which are statements about this run's circumstances and not about the request
/// or about a check. `req/307` files it for a ruling; until one lands, the count is here so that a
/// lane quietly moving a row into or out of it is red.
///
/// And the one that matters most: **`INTERNAL` is on exactly one row**. `req/304` D5's finding was
/// three of three refusals collapsing onto it, which is req/88 §3 Λ4's quotient taken all the way
/// down.
#[test]
fn the_classes_are_counted_and_the_unclassified_bucket_holds_one_row() {
    let count = |c: RefusalClass| REFUSAL_MAP.iter().filter(|r| r.class == c).count();
    let (declared, verified, operational, internal) = (
        count(RefusalClass::Declared),
        count(RefusalClass::Verified),
        count(RefusalClass::Operational),
        count(RefusalClass::Internal),
    );
    println!(
        "CLASS_CENSUS declared={declared} verified={verified} operational={operational} \
         internal={internal} total={}",
        REFUSAL_MAP.len()
    );
    assert_eq!(
        declared + verified + operational + internal,
        REFUSAL_MAP.len(),
        "every row has a class"
    );
    assert_eq!(
        internal, 1,
        "🔴 `INTERNAL` is 44 §2.3's word for what cannot be classified and it belongs on one row. \
         A second row wearing it is a refusal somebody declined to name"
    );
    assert_eq!(
        REFUSAL_MAP
            .iter()
            .find(|r| r.class == RefusalClass::Internal)
            .map(|r| r.code),
        Some("INTERNAL"),
        "the one unclassified row is the one wearing 44 §2.3's unclassified word"
    );
    assert_eq!(
        operational, 2,
        "the residual `req/306` §1 did not name: `BUSY` and `OUTPUT_FAILED`, and `req/307` files \
         the question of whether it stays a class of its own"
    );
    assert!(
        declared >= 1 && verified >= 1,
        "`req/306` §1 item 1's two named classes are both populated"
    );
}

// ---------------------------------------------------------------------------
// 🔴 F-2 (`req/38` §337, `req/565` §4) — the title agrees across the crate boundary
// ---------------------------------------------------------------------------

/// 🔴 **F-2 / `req/38` §337 (adopted (a): equality test, `req/565` §4-1(a))** — the two faces'
/// title for one `gx_code`, compared by a gate rather than kept by hand.
///
/// `gx_api::gx_code::LAYOUT_BLOCKED_TITLE`'s own doc comment registered its own gap in words:
/// "no gate in this tree compares the two … so the equality is a convention a lane keeps by
/// hand". Audit 40 finding F-2 confirmed it two ways — a `grep` for readers of that constant
/// outside its own file (zero) and an isolated-clone fault injection (`gx_a40_inj`: the constant
/// edited to a divergent string, the suite run, and this file's four existing tests stayed green
/// because none of them reads `gx-api` at all). This function is the gate: it is a crate-crossing
/// comparison, and it costs nothing new to wire because `gx-cli` already depends on `gx-api` as a
/// **normal** dependency (`Cargo.toml`: `gx serve` embeds `gx_api::router`), the same fact
/// `declared_cli_exit` above already leans on for `GX_CODES`/`RULED_ADDITIONS`.
///
/// 🔴 F-2's own KA-4 (`req/565` §5): this is deliberately **not** a single line added to
/// [`every_word_in_the_map_is_one_this_system_already_has`]'s `PRECONDITION_CHANGED`/
/// `INVERSE_UNAVAILABLE` block above — that block compares two rows of `REFUSAL_MAP` against each
/// other (both faces of one crate); this compares a `REFUSAL_MAP` row against `gx_api::gx_code`'s
/// own constant (two crates), which needed the import and not a bigger `for` loop.
fn title_parity(code: &str, api_title: &str) {
    let cli_title = REFUSAL_MAP
        .iter()
        .find(|row| row.code == code)
        .unwrap_or_else(|| panic!("`{code}` has no row in `gx_cli::REFUSAL_MAP`"))
        .title;
    println!("TITLE_PARITY code={code} api={api_title:?} cli={cli_title:?}");
    assert_eq!(
        api_title, cli_title,
        "🔴 `{code}`'s title diverged between `gx_api::gx_code` and `gx_cli::REFUSAL_MAP` — the \
         two faces of one binary would print two different sentences about one refusal, which is \
         `BUSY_TITLE`'s rule broken silently rather than by a reviewer's choice"
    );
}

/// 🔴 F-2's body — `LAYOUT_BLOCKED`, the finding's own example.
#[test]
fn layout_blocked_title_agrees_across_the_two_faces() {
    title_parity(
        gx_api::gx_code::LAYOUT_BLOCKED,
        gx_api::gx_code::LAYOUT_BLOCKED_TITLE,
    );
}

/// 🔴 **AC-7 (`req/565` §4-2, §6) — F-2 applies to the new word from the commit that mints it.**
///
/// DR-B (`req/38` §337, `req/565` §3) mints `JOURNAL_UNREADABLE` in the same commit as this test,
/// which is the point of landing both in one lane: a new ruled addition is under the parity gate
/// from the moment it exists, rather than joining `LAYOUT_BLOCKED` in a follow-up lane someone has
/// to remember to file.
#[test]
fn journal_unreadable_title_agrees_across_the_two_faces() {
    title_parity(
        gx_api::gx_code::JOURNAL_UNREADABLE,
        gx_api::gx_code::JOURNAL_UNREADABLE_TITLE,
    );
}
