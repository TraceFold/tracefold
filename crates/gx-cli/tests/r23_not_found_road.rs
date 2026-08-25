// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/38` §225 rulings 4 (L-04) and 5 (`Engine::NotFound`)** (`req/313` §1 items 5 and 6) —
//! one word for "the object you named is not here", on the two roads that did not have it.
//!
//! # What the twenty-second adversarial audit measured, verbatim
//!
//! ```text
//! A22_NOTFOUND verb="replay" rc=0 gx_code=<no problem object>
//! $ gx --project … replay gx1:3ilf…abq
//! {"matches":true,"diffs":[],"unchecked":["drafts","transformations","escrow"],
//!  "records_replayed":0,"ledger_consulted":true,"dry_run":false}
//!
//! A22_NOTFOUND_SUMMARY asked=11 internal=[]
//! ```
//!
//! Two roads, one word:
//!
//! * **L-04** — `gx replay <a well-formed id this project has never held>` answered `rc=0` and
//!   `matches: true`. The other **eleven** id-taking verbs of this binary answered `NOT_FOUND` /
//!   exit 6 to the same argument, so this was not merely a vacuous pass: one verb disagreed with
//!   the rest of the binary about what the id names. `records_replayed: 0` is printed beside it and
//!   a person can read it; a tool branching on `matches` cannot.
//! * **`Engine::NotFound`** — `gx-api` has answered `NOT_FOUND` (declared `cli_exit` **6**) for
//!   `gx_engine::Error::NotFound` since M6-09, and this binary carried it to `INTERNAL` / **1**.
//!   R21 measured the disagreement and declined to move it (`req/307` §5-1); R22 measured four
//!   verbs and could not produce the road (`req/310` §5-2); the twenty-second audit put **eleven**
//!   verbs against it and produced `INTERNAL` **zero** times, which is the measurement `req/38`
//!   §224 ruling 1 made the precondition of this repair.
//!
//! # What this file holds, and what it deliberately cannot
//!
//! The `Engine::NotFound` road is not reachable from a CLI verb — that is the measurement above,
//! and it is the reason the exit change is safe. So the word and the number are asked of the
//! **value**, through the public API this crate already exposes for exactly this (`Error::refusal`
//! and `Error::exit_code`, the pair `r21_refusal_map_is_whole.rs` was built on), and the road
//! measurement is redone here rather than trusted: every id-taking verb is put against a
//! well-formed absent id, and none of them may answer `INTERNAL`.
//!
//! # Red-first
//!
//! No symbol this lane created is named — `gx_cli::Error`, `gx_cli::exit` and `gx_api::gx_code` all
//! predate it — so the file compiles at `7261321` and fails on its assertions.

mod support;

use serde_json::Value;
use support::{keypair, project, run, tid};

/// A well-formed transformation id this project has never held.
///
/// Built the way the audit built it: from a real id's alphabet, so the argument is a spelling the
/// parser accepts and the journal has never seen — which is the case that was answered "match".
fn absent_id() -> String {
    tid(4242).0.to_text()
}

// ---------------------------------------------------------------------------
// L-04 — `gx replay <absent id>`
// ---------------------------------------------------------------------------

/// 🔴 `req/312` L-04: an id this journal holds no record of is *not found*, not *a match*.
#[test]
fn replay_of_an_id_this_project_never_held_is_not_a_match() {
    let (dir, layout) = project("r23_replay_absent");
    let key = keypair(23);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 30, 6);
    // A journal that is not empty: the arm has to distinguish "this project holds nothing" from
    // "this project does not hold **that**".
    seed_journal(&layout, &[(30, index)]);

    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("replay")
        .arg(absent_id()));
    println!(
        "R23_REPLAY_ABSENT exit={} stdout={} stderr={}",
        out.code,
        out.stdout.trim(),
        out.stderr.trim()
    );
    assert_eq!(
        out.code,
        i32::from(gx_cli::exit::NOT_FOUND),
        "🔴 `req/312` L-04: `gx replay` answered **0** and `matches: true` about a transformation \
         this project has never held, while the other eleven id-taking verbs answered \
         `NOT_FOUND` / 6 to the same argument. `records_replayed: 0` is beside it and a tool \
         branching on `matches` does not read it. stdout: {}",
        out.stdout.trim()
    );
    let problem: Value = out
        .stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|v| v["gx_code"].is_string())
        .unwrap_or_else(|| panic!("44 §1.3's problem object on stderr: {}", out.stderr));
    assert_eq!(
        problem["gx_code"],
        Value::String("NOT_FOUND".to_string()),
        "and the word is the one every other id-taking verb of this binary uses: {problem}"
    );
    assert!(
        !out.stdout.contains("\"matches\":true") && !out.stdout.contains("\"matches\": true"),
        "and `matches: true` is not printed about a replay of nothing: {}",
        out.stdout
    );
}

/// The control, and the two things it protects.
///
/// A replay of an id the journal **does** hold still answers, and a replay of the whole journal is
/// untouched. Without this arm, refusing every `gx replay <id>` satisfies the arm above.
#[test]
fn replay_of_an_id_this_project_holds_and_of_the_whole_journal_are_unchanged() {
    let (dir, layout) = project("r23_replay_present");
    let key = keypair(24);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 30, 6);
    seed_journal(&layout, &[(30, index)]);

    let named = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("replay")
        .arg(tid(30).0.to_text()));
    println!(
        "R23_REPLAY_PRESENT exit={} {}",
        named.code,
        named.stdout.trim()
    );
    assert_eq!(
        named.code, 0,
        "an id this journal holds replays as it always did: {}",
        named.stderr
    );
    assert_eq!(named.json()["matches"], serde_json::json!(true));
    assert_eq!(named.json()["records_replayed"], serde_json::json!(1));

    let all = run(support::gx().arg("--project").arg(&dir).arg("replay"));
    println!("R23_REPLAY_ALL exit={} {}", all.code, all.stdout.trim());
    assert_eq!(
        all.code, 0,
        "the whole journal is untouched: {}",
        all.stderr
    );
    assert_eq!(all.json()["matches"], serde_json::json!(true));
    assert_eq!(all.json()["records_replayed"], serde_json::json!(2));
}

/// Write a journal holding one draft and `commits` commit records. The shape `replay_cmd.rs` uses,
/// spelled here rather than shared: a fixture two suites reach into is a fixture neither owns.
fn seed_journal(layout: &gx_cli::layout::Layout, commits: &[(u64, u64)]) {
    use gx_core::Timestamp;
    use gx_engine::store::{EngineJournal, EngineJournalRecord};

    let path = layout.journal_path();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create");
    let mut journal = EngineJournal::open(&path).expect("open the journal");
    journal
        .append(EngineJournalRecord::DraftCreated {
            intent_id: support::iid(1),
            rng_seed: 7,
            at: Timestamp(1),
        })
        .expect("append");
    for (seed, seq) in commits {
        journal
            .append(EngineJournalRecord::Committed {
                transformation: tid(*seed),
                ledger_seq: *seq,
                at: Timestamp(2),
            })
            .expect("append");
    }
}

// ---------------------------------------------------------------------------
// `Engine::NotFound` — the word and the number, in one commit
// ---------------------------------------------------------------------------

/// 44 §2.3's third column for one code, out of `gx-api`'s own tables and from nowhere else.
fn declared_cli_exit(code: &str) -> u8 {
    gx_api::gx_code::GX_CODES
        .iter()
        .chain(gx_api::gx_code::RULED_ADDITIONS.iter())
        .find(|row| row.code == code)
        .unwrap_or_else(|| panic!("`{code}` is in neither 44 §2.3's twelve nor `RULED_ADDITIONS`"))
        .cli_exit
}

fn engine_not_found(what: &'static str) -> gx_cli::Error {
    gx_cli::Error::Engine(gx_engine::Error::NotFound {
        what,
        id: "TransformationId(Cid(opaque))".to_string(),
    })
}

/// 🔴 `req/38` §225 ruling 5: the two subjects a **caller names** wear 44 §2.3's word for it.
#[test]
fn a_transformation_or_a_draft_the_engine_cannot_find_wears_the_word_the_http_face_uses() {
    for what in ["transformation", "draft"] {
        let error = engine_not_found(what);
        let refusal = error.refusal();
        println!(
            "R23_NOT_FOUND what={what} code={} exit={} arm={}",
            refusal.code,
            error.exit_code(),
            refusal.arm
        );
        assert_eq!(
            refusal.code, "NOT_FOUND",
            "🔴 `gx-api`'s `gx_code::REFUSALS` has answered `Engine::NotFound` with `NOT_FOUND` \
             since M6-09 and this binary answered `INTERNAL` — 44 §2.3's word for what **cannot be \
             classified**, over a refusal that is completely classified: the caller named a \
             {what} and it is not there"
        );
        assert_eq!(
            error.exit_code(),
            gx_cli::exit::NOT_FOUND,
            "and the number moves in the same commit as the word: a code and an exit that \
             disagree on one refusal is worse than either alone (`req/307` §5-1 (3), which is why \
             R21 left this road alone until the number could move with it)"
        );
        assert_eq!(
            u32::from(error.exit_code()),
            u32::from(declared_cli_exit(refusal.code)),
            "and 44 §2.3's own third column is what says which number that is"
        );
    }
}

/// 🔴 The residual, named rather than swept: `adapter` is **not** this word, and stays where it is.
///
/// `req/38` §224 ruling 1 keeps it `INTERNAL` / 1: "no adapter is registered for this substrate" is
/// a statement about something **nobody named**, which is the argument that split
/// `DECLARATION_ABSENT` and `JOURNAL_ABSENT` off `NOT_FOUND` (`req/238` H-01). Nine of the
/// engine's twenty `NotFound` sites carry it, so this is the largest subject of the family and the
/// one the repair deliberately does not take.
#[test]
fn the_subjects_a_caller_did_not_name_keep_the_word_they_had() {
    for what in [
        "adapter",
        "provenance for a committed transformation (42 §3.9, M5-25 adopted (a); sem: SEM-gx-engine-236)",
        "intent id",
        "the Planned record",
    ] {
        let error = engine_not_found(what);
        let refusal = error.refusal();
        println!(
            "R23_RESIDUAL what={:?} code={} exit={}",
            what.chars().take(40).collect::<String>(),
            refusal.code,
            error.exit_code()
        );
        assert_eq!(
            refusal.code, "INTERNAL",
            "the repair is an equality over two subjects and not a widening of the family: {what}"
        );
        assert_eq!(error.exit_code(), 1, "and the number stays with the word");
    }
    // And the escrow road R21 already took is untouched.
    let escrow =
        engine_not_found("escrowed inverse (42 §3.12 Unavailable: `invert` answered None)");
    println!(
        "R23_ESCROW code={} exit={}",
        escrow.refusal().code,
        escrow.exit_code()
    );
    assert_eq!(
        escrow.refusal().code,
        "INVERSE_UNAVAILABLE",
        "R21's arm runs before this lane's and keeps its road (`req/304` §0.8)"
    );
    assert_eq!(escrow.exit_code(), 1);
}

/// 🔴 The predicate is over the engine's **own** set of subjects, read from its source.
///
/// `r21_refusal_semantics.rs` established this shape for exactly this problem: a repair keyed on the
/// sentences one audit happened to print leaves the ones it did not. This arm holds the two words
/// `gx-cli` now equals against every subject `gx-engine` builds, so a lane that renames
/// `"transformation"` — or adds a second spelling of it — is red here rather than quietly back in
/// the `INTERNAL` bucket.
#[test]
fn every_subject_the_engine_can_fail_to_find_is_classified_one_way_or_the_other() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/")
            .join("gx-engine/src/pipeline.rs"),
    )
    .expect("the engine's pipeline is readable");
    let mut subjects: Vec<String> = Vec::new();
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
        subjects.push(body[..close].to_string());
    }
    println!("R23_ENGINE_SUBJECTS n={} {subjects:?}", subjects.len());
    assert!(
        subjects.len() >= 20,
        "🔴 the scan found {} subjects and `req/312` §2(e) counted twenty. A scan that found \
         nothing would satisfy the loop below: {subjects:?}",
        subjects.len()
    );
    let named = subjects
        .iter()
        .filter(|s| s.as_str() == "transformation" || s.as_str() == "draft")
        .count();
    assert!(
        named >= 2,
        "the two subjects this lane equals must exist in the engine's own source, or the arm is \
         a predicate over nothing: {subjects:?}"
    );
    for subject in &subjects {
        // Leaked into `Error::NotFound`'s `what` as a `&'static str`, so a `String` here is a
        // faithful stand-in for the value the engine builds.
        let leaked: &'static str = Box::leak(subject.clone().into_boxed_str());
        let error = engine_not_found(leaked);
        let code = error.refusal().code;
        let expected = match subject.as_str() {
            "transformation" | "draft" => "NOT_FOUND",
            s if s.contains("escrowed inverse") => "INVERSE_UNAVAILABLE",
            _ => "INTERNAL",
        };
        assert_eq!(
            code, expected,
            "{subject:?} answers {code} and this lane's table says {expected}"
        );
        assert_eq!(
            u32::from(error.exit_code()),
            u32::from(declared_cli_exit(code)),
            "and 44 §2.3's third column agrees with the number for {subject:?}"
        );
    }
}

/// 🔴 The blast radius, re-measured rather than quoted.
///
/// `req/312` §2(e) put eleven id-taking verbs against well-formed absent ids and produced
/// `INTERNAL` zero times — every one of them was answered by `gx_cli::Error::NotFound`, this
/// crate's own variant, before the engine was reached. That measurement is what `req/38` §224
/// ruling 1 made the precondition of moving an exit status, so it is a probe here rather than a
/// sentence in a report.
#[test]
fn no_id_taking_verb_answers_internal_for_an_id_that_is_not_there() {
    let (dir, _layout) = project("r23_verb_sweep");
    let seeded = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .args(["repair", "--yes"]));
    println!("R23_SWEEP_SEED exit={}", seeded.code);
    let id = absent_id();
    let verbs: Vec<Vec<String>> = vec![
        vec!["plan".into(), id.clone()],
        vec!["verify".into(), id.clone()],
        vec!["commit".into(), id.clone()],
        vec!["undo".into(), id.clone()],
        vec!["cancel".into(), id.clone()],
        vec!["receipt".into(), "verify".into(), id.clone()],
        vec!["draft".into(), "discard".into(), id.clone()],
        vec!["replay".into(), id.clone()],
    ];
    let mut internal: Vec<String> = Vec::new();
    let mut answers: Vec<(String, i32, String)> = Vec::new();
    for verb in &verbs {
        let out = run(support::gx().arg("--project").arg(&dir).args(verb));
        let code = out
            .stderr
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find_map(|v| v["gx_code"].as_str().map(str::to_string))
            .unwrap_or_default();
        answers.push((verb.join(" "), out.code, code.clone()));
        if code == "INTERNAL" {
            internal.push(format!("{}: exit {}", verb.join(" "), out.code));
        }
    }
    for (verb, exit, code) in &answers {
        println!("R23_SWEEP {verb:<28} exit={exit} gx_code={code}");
    }
    assert!(
        internal.is_empty(),
        "🔴 `req/38` §224 ruling 1 made \"can this road be produced from a verb\" the precondition \
         of moving an exit status, and the answer measured was **no**. If a verb reaches \
         `Engine::NotFound` now, the exit this lane moved is one a script has seen: {internal:?}"
    );
}
