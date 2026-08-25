// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R21 / `req/304` D5** — a declared refusal is not an internal error, on the CLI's face.
//!
//! # What the dogfood lane measured
//!
//! `req/304` is a first-contact walk of `gx` by a reader who had only `README.md`,
//! `docs/LIMITS.md` and `docs/TUTORIAL.md`. Three of its refusals came back wearing
//! `gx_code:"INTERNAL"` and the title *"the operation could not be completed"*, and its finding
//! D5 counted them: **three of three refusals observed used the same generic code**, so "a caller
//! or monitor cannot distinguish 'you made a declared mistake' from 'gx broke'".
//!
//! The three, verbatim from `req/304` §0.5 and §0.8:
//!
//! ```text
//! {"gx_code":"INTERNAL","detail":"the adapter refused to snapshot: the substrate would not
//!  answer for \"notes.md\": not a position from the root; v0.1 names positions absolutely
//!  (ASM-69-3)"}
//! {"gx_code":"INTERNAL","detail":"no escrowed inverse (42 §3.12 Unavailable: `invert` answered
//!  None) named TransformationId(Cid(opaque))"}
//! ```
//!
//! Both are entirely classified. The first is `gx-adapter-fs` reading its own declaration
//! (**ASM-69-3**: v0.1 names positions from the root) and refusing an argument; the second is
//! 42 §3.12's `InverseStatus::Unavailable`, a status `invert()` **was run** to obtain. 44 §2.3
//! keeps `INTERNAL` for what *cannot* be classified, which is the same sentence R12, R13 and R14
//! each took an `INTERNAL` back with.
//!
//! # 🔴 What was actually wrong, and it is bigger than two arms
//!
//! `gx-api`'s `gx_code::REFUSALS` maps all thirty-eight refusal kinds of the four lower crates onto
//! 44 §2.3's vocabulary, one row each, with `fold: None` on the rows that lose nothing —
//! **`Engine::Adapter` → `ADAPTER_ERROR`** and **`Engine::NotFound` → `NOT_FOUND`** are two of
//! them. `gx-cli::Error::problem` had no such map: one arm carried `Io | Witness | Log | Engine |
//! Gate` to `INTERNAL` wholesale. So the two faces of one binary held two names for one refusal,
//! which is exactly the defect `BUSY_TITLE` and DR-43-6 exist to forbid ("a proxy speaking both
//! must not hold two names for one refusal").
//!
//! # Denominator — what this suite does **not** claim
//!
//! * **No exit status moves.** `req/306` §1 fixes the state machine of 44 §1.4 (discipline 52's
//!   usage=1, the reservation of 2 for `Verdict::Deny`), and every code this lane assigns has a
//!   declared `cli_exit` equal to the number `Error::exit_code` already returned. That equality is
//!   the subject of the sibling suite `r21_refusal_map_is_whole.rs`; here the arms assert the exit
//!   they measured **unchanged** so that a repair which quietly moved one is red.
//! * **No new `gx_code` is minted.** Both words used here are 44 §2.3's own rows.
//!   `req/307` files the case for an `INVALID_LOCATOR` — the word `req/304`'s own remedy asks for —
//!   as a ruling, because minting one costs a `req/38` entry, a row in `gx-api`'s
//!   `RULED_ADDITIONS` and a word in `sdk/typescript/src/errors.ts`, none of which is R21's to
//!   write.
//! * The general `Engine::NotFound` road (a transformation, a draft, a blob) is **left alone**:
//!   `gx-api` answers it `NOT_FOUND`, whose declared `cli_exit` is **6**, and this binary exits
//!   **1** there. Moving the code without the number would put a code and an exit that disagree on
//!   one refusal, and moving the number is an exit-status change `req/306` §1 forbids. Filed in
//!   `req/307` for a ruling rather than taken here.
//!
//! # This file compiles against the pre-repair tree on purpose
//!
//! Every arm below names only symbols that exist at `1d1c145`, so its red is a failing assertion
//! and not a missing symbol — the distinction `r20_refusal_vocabulary_is_whole.rs` drew for the
//! same reason (a suite that will not compile measures nothing). The half that *cannot* be written
//! that way — the declared map and its exit agreement — is the sibling file.

mod support;

use std::path::Path;

use support::{oversized_before, pipeline, run};

/// 44 §2.3's word for "an internal error that cannot be classified", which none of these is.
const UNCLASSIFIED: &str = "INTERNAL";

/// The problem object on stderr, parsed. 44 §1.3 puts exactly one there.
///
/// 🔴 The **last** line and not the whole of stderr, and that is a finding rather than a
/// convenience. `gx undo` prints `"gx undo settle: skipped (…); firing as before"` as a plain
/// sentence on stderr **before** the problem object, so a caller piping stderr to a JSON reader —
/// which is what 44 §1.3's contract invites — gets a parse error rather than a refusal. It is the
/// stderr sibling of `req/304`'s D8 (`gx key gen --json` putting a human line on stdout beside the
/// JSON), it is outside `req/306` §1's three items, and `req/307` §3 carries it as a ledger row
/// rather than repairing it silently here.
fn problem(stderr: &str) -> serde_json::Value {
    let last = stderr
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or_default();
    serde_json::from_str(last)
        .unwrap_or_else(|e| panic!("44 §1.3 asks for a problem object on stderr: {stderr:?} ({e})"))
}

fn engine_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-engine/src")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

fn cli_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Class ① — a refusal that follows from a **declaration**
// ---------------------------------------------------------------------------

/// 🔴 **`req/304` §0.5, re-run** — a relative `--locator` is a declared refusal, not a fault.
///
/// The exact shape a newcomer produces: `--locator "notes.md"`, because every other command line
/// they have ever used takes a relative path. `gx submit` accepts it (the CLI does not own the
/// locator grammar — the adapter does, and `submit` never reaches one), and `gx plan` is where
/// `FsAdapter::snapshot` reads **ASM-69-3** and says no.
///
/// That sentence is `gx_substrate::Error::Unreadable` carried out through
/// `gx_engine::Error::Adapter`, whose own doc comment in `gx-engine/src/lib.rs` names the code it
/// is for: *"A `SubstrateAdapter` refused (44 §2.3's `ADAPTER_ERROR`, 502)"*. `gx-api` has been
/// answering that word since M6-09. This binary answered `INTERNAL`.
#[test]
fn a_relative_locator_is_a_declared_refusal_and_not_an_internal_error() {
    let fixture = pipeline("r21_relative_locator", "before any agent touched it\n");
    let goal = fixture.project.join("intent_v1.txt");
    std::fs::write(&goal, "hello\n").expect("write the goal");

    // `submit` takes it — `req/304` D2's "accepted at submit, only refused two verbs later".
    let submitted = run(fixture
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .args(["--locator", "target.txt"]) // 🔴 relative, and that is the point
        .arg("--intent")
        .arg(&goal)
        .args(["--context", "Substrate"])
        .args(["--actor-key", &fixture.key_id]));
    assert_eq!(
        submitted.code, 0,
        "`req/304` §0.5 measured `submit` accepting a relative locator at exit 0: {}",
        submitted.stderr
    );
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let planned = run(fixture.gx().args(["plan", &intent]));
    let problem = problem(&planned.stderr);
    println!(
        "R21_RELATIVE_LOCATOR exit={} gx_code={} detail={}",
        planned.code, problem["gx_code"], problem["detail"]
    );

    assert!(
        problem["detail"]
            .as_str()
            .expect("a detail string")
            .contains("not a position from the root"),
        "the arm must drive ASM-69-3's own refusal and not some other one: {problem}"
    );
    assert_ne!(
        problem["gx_code"], UNCLASSIFIED,
        "🔴 `req/304` D5: this refusal is completely classified — the adapter read its own \
         declaration (ASM-69-3) and refused an argument — and 44 §2.3 keeps `INTERNAL` for what \
         cannot be classified. Answering it `INTERNAL` tells a caller gx broke when what happened \
         is that they typed a relative path"
    );
    assert_eq!(
        problem["gx_code"], "ADAPTER_ERROR",
        "44 §2.3 row 9, and the word `gx-api`'s `REFUSALS` has answered for `Engine::Adapter` \
         since M6-09. Two faces of one binary may not hold two names for one refusal (DR-43-6, \
         `BUSY_TITLE`)"
    );
    assert_eq!(
        planned.code, 1,
        "🔴 no exit status moves in this lane (`req/306` §1): `ADAPTER_ERROR`'s declared \
         `cli_exit` is 1 and that is what this road already returned"
    );
    assert!(
        planned.stdout.trim().is_empty(),
        "44 §1.3: a refusal that did not run prints nothing on stdout"
    );
}

/// 🔴 **`req/304` D2's second shape** — an empty `--locator`, which normalises to `"."`.
///
/// The same class one spelling over, and it is here because a repair keyed on the literal string
/// `"notes.md"` would pass the arm above and change nothing. The predicate is "the adapter
/// refused", not "this path".
#[test]
fn an_empty_locator_is_a_declared_refusal_and_not_an_internal_error() {
    let fixture = pipeline("r21_empty_locator", "before\n");
    let goal = fixture.project.join("intent_v1.txt");
    std::fs::write(&goal, "hello\n").expect("write the goal");

    let submitted = run(fixture
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .args(["--locator", ""])
        .arg("--intent")
        .arg(&goal)
        .args(["--context", "Substrate"])
        .args(["--actor-key", &fixture.key_id]));
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let planned = run(fixture.gx().args(["plan", &intent]));
    let problem = problem(&planned.stderr);
    println!(
        "R21_EMPTY_LOCATOR exit={} gx_code={} detail={}",
        planned.code, problem["gx_code"], problem["detail"]
    );
    assert_ne!(problem["gx_code"], UNCLASSIFIED, "{problem}");
    assert_eq!(problem["gx_code"], "ADAPTER_ERROR", "{problem}");
    assert_eq!(planned.code, 1, "the exit is unchanged");
}

// ---------------------------------------------------------------------------
// Class ② — a refusal that follows from a **check that ran**
// ---------------------------------------------------------------------------

/// 🔴 **`req/304` §0.8, re-run whole** — escalate → approve → commit → undo-refused.
///
/// The one road on the `fs` substrate that reaches a `Verdict::Escalate` in v0.1: an existing file
/// whose **old** content is over `MAX_INVERSE_PAYLOAD_BYTES` (1 MiB) makes `FsAdapter::invert`
/// answer `Ok(None)`, which **E-M3-4** escalates to a human rather than silently committing a
/// change nobody can undo. A person then rules on it, the change lands, and the undo is refused —
/// correctly, and that is the point: the refusal is the product working.
///
/// `req/304` recorded the last step as
/// `{"gx_code":"INTERNAL","detail":"no escrowed inverse (42 §3.12 Unavailable: `invert` answered
/// None) named …"}` and observed that "the JSON `detail` string is accurate and calm, but
/// `gx_code:"INTERNAL"` undercuts it". 44 §2.3's row 7 is `INVERSE_UNAVAILABLE`,
/// *"the inverse delta for the undo target is unavailable" (JP original moved verbatim to req/semantics/gx-cli.ja.md), `cli_exit` **1** — the number this road already returns — and
/// the very same word already appears **inside** the escalation ticket this arc printed two steps
/// earlier, as `reasons[0].code`. The lane's own R9 repair (`req/236` M-01) had taken this code
/// back for `Error::InverseUnavailable`, the road where the escrowed row exists and its **body** is
/// missing; the road where the escrow was never constructible was not repaired with it. Fixing the
/// row that was measured rather than the question it asked is the failure
/// `feedback_fix_the_question_not_the_row` names.
#[test]
fn an_undo_with_no_constructible_inverse_is_a_checked_refusal_and_not_an_internal_error() {
    let fixture = pipeline("r21_no_inverse_undo", &oversized_before());
    let tid = fixture.planned_one("small replacement");

    let verified = run(fixture.gx().args(["verify", &tid]));
    println!(
        "R21_ESCALATED exit={} kind={:?} reason={:?}",
        verified.code,
        verified.json()["kind"],
        verified.json()["ticket"]["reasons"][0]["code"]
    );
    assert_eq!(
        verified.code, 4,
        "the Given is `req/304` §0.8's `Escalate`: {}",
        verified.stderr
    );
    assert_eq!(
        verified.json()["ticket"]["reasons"][0]["code"],
        "INVERSE_UNAVAILABLE",
        "🔴 the word is already on the ticket. `req/304`'s remedy: it \"just isn't promoted to \
         the top-level gx_code on the later undo refusal\""
    );

    let ruler = fixture.another_key();
    let approved = run(fixture
        .gx()
        .args(["escalation", "approve", &tid])
        .args([
            "--reason",
            "dogfood: accepting an unrestorable overwrite on purpose",
        ])
        .args(["--actor-key", &ruler]));
    assert_eq!(approved.code, 0, "approve: {}", approved.stderr);
    let committed = run(fixture.gx().args(["commit", &tid]));
    assert_eq!(committed.code, 0, "commit: {}", committed.stderr);
    assert_eq!(
        fixture.target_contents(),
        "small replacement",
        "`req/304` §0.8: \"the overwrite really landed\""
    );

    let undone = run(fixture.gx().args(["undo", &tid]));
    let problem = problem(&undone.stderr);
    println!(
        "R21_UNDO_NO_INVERSE exit={} gx_code={} detail={}",
        undone.code, problem["gx_code"], problem["detail"]
    );
    assert!(
        problem["detail"]
            .as_str()
            .expect("a detail string")
            .contains("escrowed inverse"),
        "the arm must drive 42 §3.12's own refusal: {problem}"
    );
    assert_ne!(
        problem["gx_code"], UNCLASSIFIED,
        "🔴 `req/304` D5: `invert()` was **run** and answered `None`, 42 §3.12 wrote that down as \
         `InverseStatus::Unavailable`, and the escalation ticket had already published the word. A \
         checked, declared, documented outcome is not \"an internal error that cannot be \
         classified\""
    );
    assert_eq!(
        problem["gx_code"], "INVERSE_UNAVAILABLE",
        "44 §2.3 row 7, and `Error::InverseUnavailable`'s own word since R9"
    );
    assert_eq!(
        undone.code, 1,
        "🔴 no exit status moves: `INVERSE_UNAVAILABLE`'s declared `cli_exit` is 1, and 1 is what \
         `req/304` §0.8 measured"
    );
    assert_eq!(
        fixture.target_contents(),
        "small replacement",
        "a refused undo changes nothing"
    );
}

// ---------------------------------------------------------------------------
// The predicate, over the whole family rather than over the three rows measured
// ---------------------------------------------------------------------------

/// Every `what:` an `Error::NotFound` is built with in `gx-engine`, in declaration order.
///
/// Read out of the source rather than matched on, for `r20_refusal_vocabulary_is_whole.rs`'s
/// reason: the strings are `&'static str` literals set at one site each, and a lane that adds a
/// sixth spelling of "there is no escrowed inverse" must make this file red rather than quietly
/// rejoin the `INTERNAL` bucket.
fn engine_not_found_subjects() -> Vec<String> {
    let source = engine_src("pipeline.rs");
    let mut out = Vec::new();
    let mut rest = source.as_str();
    while let Some(at) = rest.find("Error::NotFound {") {
        rest = &rest[at + "Error::NotFound {".len()..];
        let Some(w) = rest.find("what:") else { break };
        let after = &rest[w + "what:".len()..];
        let Some(open) = after.find('"') else {
            continue;
        };
        let body = &after[open + 1..];
        let Some(close) = body.find("\",") else {
            continue;
        };
        out.push(body[..close].to_string());
    }
    out
}

/// 🔴 The question, not the row: **every** absence of an escrowed inverse the engine can name.
///
/// `req/304` measured one sentence — 42 §3.12's `Unavailable`. `gx-engine`'s `UndoRefusal::
/// into_error` builds `Error::NotFound` with **three** different subjects that each name an
/// escrowed inverse (`NoEscrow`, `InverseUnavailable`, and the one
/// `Engine::undo`'s intent builder raises), and a repair keyed on the sentence `req/304` printed
/// would have left the other two wearing `INTERNAL`. So the classifier is a predicate over the
/// subject and this arm holds it to the engine's own set.
#[test]
fn every_escrowed_inverse_the_engine_can_fail_to_find_is_classified() {
    let subjects = engine_not_found_subjects();
    let escrow: Vec<&String> = subjects
        .iter()
        .filter(|s| s.contains("escrowed inverse"))
        .collect();
    println!("ENGINE_NOT_FOUND_SUBJECTS={subjects:?}");
    println!("ESCROW_SUBJECTS={} {escrow:?}", escrow.len());
    assert_eq!(
        escrow.len(),
        3,
        "`gx-engine` names an absent escrowed inverse in three places; if this number moved, the \
         classifier below has to be looked at rather than trusted: {subjects:?}"
    );

    // The CLI's own discriminator, read from its source so that this arm builds on the pre-repair
    // tree and fails on its assertion. The constant is what `Error::problem` matches on.
    let cli = cli_src("lib.rs");
    assert!(
        cli.contains("ESCROWED_INVERSE_SUBJECT"),
        "🔴 `req/304` D5 / `req/306` §1 item 1: `gx-cli` declares no predicate for \"there is no \
         escrowed inverse\" at all — `Error::problem` carries every `Error::Engine(_)` to \
         `INTERNAL` in one arm, which is why three different documented refusals came back with \
         one generic word"
    );
    let declared = cli
        .split("ESCROWED_INVERSE_SUBJECT: &str = ")
        .nth(1)
        .and_then(|s| s.split('"').nth(1))
        .expect("the discriminator is a string literal")
        .to_string();
    println!("CLI_DISCRIMINATOR={declared:?}");
    for subject in &escrow {
        assert!(
            subject.contains(&declared),
            "`{subject}` is an absent escrowed inverse the discriminator {declared:?} does not \
             catch, so it would still answer `INTERNAL`"
        );
    }
    // And the discriminator must not be so wide it swallows the rest.
    for subject in &subjects {
        if subject.contains("escrowed inverse") {
            continue;
        }
        assert!(
            !subject.contains(&declared),
            "`{subject}` is not an escrowed inverse and the discriminator catches it: a \
             transformation that is not there would be answered `INVERSE_UNAVAILABLE`"
        );
    }
}

/// 🔴 **DR-43-6 / `BUSY_TITLE`'s rule, generalised** — one refusal, one word, on both faces.
///
/// The rule this repository already applies twice by hand (`BUSY`, `LEDGER_DISAGREES`) is that a
/// proxy holding both faces of this binary must not find two names for one refusal. `gx-api`'s
/// `gx_code::of_kind` is the map M6-09 built; this arm asks whether the CLI's map agrees with it on
/// the two kinds `req/304` reached. It reads `gx-api`'s own table rather than a literal, so the day
/// somebody re-rules `Engine::Adapter` the two move together.
#[test]
fn the_cli_and_the_http_face_answer_one_word_for_one_refusal() {
    let cases = [
        (
            "Adapter",
            gx_cli::Error::Engine(gx_engine::Error::Adapter {
                action: "snapshot",
                detail: "the substrate would not answer for \"notes.md\": not a position from the \
                         root; v0.1 names positions absolutely (ASM-69-3)"
                    .to_string(),
            }),
        ),
        (
            "Busy",
            gx_cli::Error::Engine(gx_engine::Error::Busy {
                path: std::path::PathBuf::from("/tmp/.gx/LOCK"),
                holder: "1 gx submit".to_string(),
            }),
        ),
    ];
    for (kind, error) in cases {
        let mine = error.problem()["gx_code"]
            .as_str()
            .expect("a string")
            .to_string();
        let theirs = gx_api::gx_code::of_kind(gx_api::gx_code::Origin::Engine, kind)
            .unwrap_or_else(|| panic!("`{kind}` has a row in M6-09's map"))
            .code;
        println!("ONE_WORD kind={kind} cli={mine} api={theirs}");
        assert_eq!(
            mine, theirs,
            "🔴 `gx_engine::Error::{kind}` wears `{theirs}` on the HTTP face and `{mine}` on the \
             command line. DR-43-6's ruling on `LEDGER_DISAGREES` is the standing one: \"a Tauri \
             proxy holding both had two names and neither was the fact\""
        );
    }
}
