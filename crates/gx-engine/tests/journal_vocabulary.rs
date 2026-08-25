// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 The 1:1 comparison req/78 §6.2, hand 1, ⑥ asks for (sem: SEM-gx-engine-763): 42 §3.13's
//! journal record vocabulary, 43 §3's
//! journal column, and what this crate implements — read out of all three and compared.
//!
//! > ⑥ a **1:1 cross-check test** between 42 §3.13's and 43 §3's journal vocabulary (11 variants ×
//! > transition ids -- put the **shape where M5-06's discrepancy comes out RED** first)
//! > (sem: SEM-gx-engine-764)
//!
//! # Why the canon is parsed rather than transcribed
//!
//! The I-11 shape (`adapter_spec.rs` in gx-substrate: "parse 41 §4's markdown and cross-check it
//! against the real type", sem: SEM-gx-engine-765).
//! A test that restated 42 §3.13 in Rust would be a second copy of the table, and two copies of a
//! table are two things that drift. So the enum in the specification is parsed out of the markdown,
//! the enum in this crate is parsed out of `src/store.rs`, and the difference between them is a
//! measurement.
//!
//! # 🔴 The difference **was** the point, and v0.2.4 closed it
//!
//! §37 ruled two changes and this suite is where they stopped being prose:
//!
//! * **E-M5-1** — the implementation has a thirteenth… no: a **twelfth** variant, `ApplyStarted`,
//!   which 42 §3.13 did not list. Recovery cannot tell "the world did not move" from "the world
//!   moved and nothing recorded it" without it (req/78 §3.2 Λ4) (sem: SEM-gx-engine-766).
//! * **E-M5-3** — `DraftCreated` is keyed on `IntentId`, which 42 §3.13 contradicted and 43 T-1
//!   requires. 51 §8.1's precedence clause "when they conflict, 43 takes precedence" (sem:
//!   SEM-gx-engine-766) applied for the first time.
//!
//! **Both errata — and the four that followed them — have now landed in the canon.** `req/38` §68
//! #2 (B-1, adopted, sem: SEM-gx-engine-767) ruled the remaining six differences of 42 §3.13
//! into the specification, so this
//! suite's subject changed: it used to measure **a divergence the implementation was right about**,
//! and it now measures **an agreement**, in both directions, plus the source each side copied.
//!
//! 🔴 **discipline 55, ②** (`req/38` §68, sem: SEM-gx-engine-768): a canon sync of 42/43 and
//! the update of the probes that read them
//! must land in **one window**. The v0.2.3 batch moved `Verdict` in 42 §3.13 without touching this
//! file, and the tripwire below did exactly what it promised — F-1, 12 passed / 1 failed on a
//! landed tree. **A doc-only batch moves the floor whenever a test reads the doc.**
//!
//! [`draft_created_follows_43_and_not_42`] was the red-first probe the DoD asked for: it failed if
//! the implementation was ever made to match 42 §3.13's literal text. `tools/verify_m5h1.sh` §4
//! fires that mutation and prints which probes fall. Since the canon now carries E-M5-3, the same
//! probe falls the other way too: **if 42 §3.13 reverts, this file says so.**
//!
//! # What this suite does not check
//!
//! It compares **names and fields**, not types: 42 §3.13 writes `fp0: Fingerprint` and this crate
//! writes `fp0: FingerprintRecord`, which is the same field carrying the same three components
//! through a form that can be written down (see `store.rs`, and M5H1-4). A type comparison would
//! need a mapping table, and a mapping table maintained by hand is the drift this suite exists to
//! prevent. Recorded here so that the gap is declared rather than discovered.
//!
//! 🔴 **The example above is history since v0.2.5** (`req/38` §71, A-1 adopted, landed with P3, sem:
//! SEM-gx-engine-769): 42 §3.13 now
//! writes `fp0: FingerprintRecord` too, so the two agree and the paragraph describes a divergence
//! that has been closed. It stays word for word — a claim about why this suite compares names is
//! still the reason it does, and the instance that produced the reason is worth keeping — and this
//! line is the current form: **the gap the paragraph illustrates is closed for `fp0`, and the rule
//! (names and fields, never types) is unchanged.** A reader who checks the canon and finds agreement
//! has found the state the batch landed, not a stale suite.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use gx_engine::store::JOURNAL_RECORD_KINDS;
use support::read_repo;

const DATA_MODEL: &str = "req/spec/40-architecture/42-data-model.md";
const STATE_MACHINE: &str = "req/spec/40-architecture/43-state-machine.md";

// ---------------------------------------------------------------------------
// Readers
// ---------------------------------------------------------------------------

/// The variants of an enum body, as `name -> [field names]`, in declaration order.
///
/// One parser for both sources, because 42 §3.13 writes its enum as Rust and `src/store.rs` writes
/// the same enum as Rust. Line comments are stripped first, so the `// T-1` annotations of the
/// specification and the doc comments of the implementation are both out of the way.
///
/// A field name is an identifier immediately after a `{` or a `,` at depth 1 and immediately before
/// a `:`. That rule, rather than "every identifier before a colon" (sem: SEM-gx-engine-770),
/// is what keeps a path type
/// (`std::io::ErrorKind`) from being read as a field; no variant here has one, and the parser is
/// written not to depend on that staying true.
fn variants(body: &str) -> Vec<(String, Vec<String>)> {
    let cleaned: String = body
        .lines()
        // Attribute lines are not fields: `InverseEscrowed.pending` carries a
        // `#[serde(default, skip_serializing_if = ..)]` line (two-phase escrow, req/38 §99 ruling
        // 2-①, sem: SEM-gx-engine-771) whose commas would otherwise read as field separators.
        // The canon's enum writes no attributes, so stripping them is a no-op on that side.
        .filter(|line| !line.trim_start().starts_with("#["))
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut depth = 0usize;
    let mut token = String::new();
    let mut expect_name = false;

    for ch in cleaned.chars() {
        match ch {
            '{' => {
                if depth == 0 {
                    out.push((token.trim().to_string(), Vec::new()));
                }
                depth += 1;
                expect_name = depth == 1;
                token.clear();
            }
            '}' => {
                depth = depth.saturating_sub(1);
                expect_name = false;
                token.clear();
            }
            ':' if depth == 1 && expect_name => {
                let name = token.trim().to_string();
                if !name.is_empty() {
                    out.last_mut().expect("a variant is open").1.push(name);
                }
                expect_name = false;
                token.clear();
            }
            ',' => {
                expect_name = depth == 1;
                token.clear();
            }
            _ => token.push(ch),
        }
    }

    out.retain(|(name, _)| !name.is_empty());
    out
}

/// 42 §3.13's `EngineJournalRecord`, parsed out of the canon.
fn canon_42() -> Vec<(String, Vec<String>)> {
    let text = read_repo(DATA_MODEL);
    let at = text
        .find("### 3.13")
        .expect("42 has a §3.13 (EngineJournal)");
    let block = &text[at..];
    let open = block.find("```").expect("§3.13 carries a code block");
    let rest = &block[open + 3..];
    let close = rest.find("```").expect("the code block closes");
    let code = &rest[..close];
    let start = code
        .find("pub enum EngineJournalRecord {")
        .expect("§3.13 declares `pub enum EngineJournalRecord`");
    let body = &code[start + "pub enum EngineJournalRecord {".len()..];
    let end = body.find("\n}").expect("the enum closes");
    variants(&body[..end])
}

/// The journal record names 43 §3's transition table writes, as `name -> [transition ids]`.
///
/// Read from the table's cells: every side-effect column that records a transition writes
/// ``journal: `Name{..}` ``, and the row's first cell is the transition id. That is the join 42
/// §3.13's own preamble promises — "what follows corresponds 1:1 to the vocabulary appearing in 43
/// §3's transition table's journal column" (sem: SEM-gx-engine-772) — and the reason this file
/// can check the promise instead of believing it.
fn canon_43() -> BTreeMap<String, Vec<String>> {
    let text = read_repo(STATE_MACHINE);
    let at = text
        .find("## 3. ")
        .expect("43 has a §3 (transition table, sem: SEM-gx-engine-773)");
    let section = &text[at..];
    let end = section[3..].find("\n## ").map_or(section.len(), |i| i + 3);
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for line in section[..end].lines() {
        let Some(rest) = line.strip_prefix("| ") else {
            continue;
        };
        let id = rest.split('|').next().unwrap_or("").trim().to_string();
        if !id.starts_with("T-") {
            continue;
        }
        let mut cursor = line;
        while let Some(i) = cursor.find("journal: `") {
            let tail = &cursor[i + "journal: `".len()..];
            let Some(j) = tail.find('`') else { break };
            let cell = &tail[..j];
            let name = cell.split('{').next().unwrap_or(cell).trim().to_string();
            out.entry(name).or_default().push(id.clone());
            cursor = &tail[j..];
        }
    }
    out
}

/// This crate's `EngineJournalRecord`, parsed out of its own source.
///
/// The source rather than a `match` over the type, for A-10's reason: a `match` is written by the
/// same hand that writes the enum and cannot notice a variant that hand forgot to register.
fn implemented() -> Vec<(String, Vec<String>)> {
    let text = read_repo("crates/gx-engine/src/store.rs");
    let start = text
        .find("pub enum EngineJournalRecord {")
        .expect("store.rs declares `pub enum EngineJournalRecord`");
    let body = &text[start + "pub enum EngineJournalRecord {".len()..];
    let end = body.find("\n}").expect("the enum closes");
    variants(&body[..end])
}

fn names(list: &[(String, Vec<String>)]) -> BTreeSet<String> {
    list.iter().map(|(n, _)| n.clone()).collect()
}

fn fields_of(list: &[(String, Vec<String>)], name: &str) -> Vec<String> {
    list.iter()
        .find(|(n, _)| n == name)
        .map(|(_, f)| f.clone())
        .unwrap_or_else(|| panic!("no variant named {name}"))
}

// ---------------------------------------------------------------------------
// The three lists
// ---------------------------------------------------------------------------

/// 42 §3.13 and 43 §3 agree with each other on **eleven** names — and 42 now carries two more.
///
/// # What the preamble of 42 §3.13 claims, before and after v0.2.4
///
/// It used to claim a flat 1:1 — "what follows corresponds 1:1 to the vocabulary appearing in 43
/// §3's transition table's journal column" (sem: SEM-gx-engine-774) — and the old assertion
/// was the pair kept here verbatim:
///
/// ```text
///     assert_eq!(by_42.len(), 11, "42 §3.13 lists eleven variants: {by_42:?}");
///     assert_eq!(by_42, by_43, "42 §3.13 promises a 1:1 correspondence …; it does not hold");
/// ```
///
/// `req/38` §68 #2 landed `ApplyStarted` and `ProvenanceDerived` in 42 §3.13, and 42 changed its
/// own preamble in the same edit rather than leaving a false promise: **the eleven that transitions
/// write are 1:1, and two engine-internal records are not written by any transition at all.**
///
/// So the claim measured is now three things, and the middle one is the surviving 1:1: 43's column
/// is **contained in** 42's enum, the containment is exactly eleven, and the residue is exactly the
/// two errata. 43 §3 is **not** edited to match — a record written inside a transition is not a
/// transition, and inventing rows for them would put edges in the state machine that do not exist.
#[test]
fn the_two_canon_documents_name_the_same_eleven_records() {
    let by_42 = names(&canon_42());
    let by_43: BTreeSet<String> = canon_43().keys().cloned().collect();
    let shared: BTreeSet<String> = by_42.intersection(&by_43).cloned().collect();
    let only_42: Vec<&String> = by_42.difference(&by_43).collect();

    println!(
        "CANON_42_RECORDS={} CANON_43_RECORDS={} SHARED={} ONLY_IN_42={only_42:?}",
        by_42.len(),
        by_43.len(),
        shared.len()
    );
    assert_eq!(
        by_42.len(),
        15,
        "42 §3.13 lists fifteen variants since v0.3-a (§98 ruling 1, two-phase escrow, sem: \
         SEM-gx-engine-775): {by_42:?}"
    );
    assert_eq!(
        shared, by_43,
        "43 §3's journal column is no longer contained in 42 §3.13's enum"
    );
    assert_eq!(
        shared.len(),
        11,
        "the transition-written vocabulary is eleven: {shared:?}"
    );
    assert_eq!(
        only_42,
        vec![
            "ApplyObserved",
            "ApplyStarted",
            "InverseCompleted",
            "ProvenanceDerived"
        ],
        "the records 42 carries and no transition of 43 writes are E-M5-1's, M5-25 adopted (a)'s and \
         two-phase escrow's pair (req/38 §98 ruling 1, sem: SEM-gx-engine-776), and no fifth; \
         found {only_42:?}"
    );
}

/// The implementation is those eleven **plus two errata** — and since v0.2.4 the canon says so too.
///
/// Stated as a set difference in both directions, so that a variant dropped and a variant added are
/// both caught, and so that every permitted addition has to be named:
///
/// * **`ApplyStarted`** (**E-M5-1**, hand 1) — the record recovery needs in order to tell "the world
///   did not move" from "the world moved and nothing recorded it" (sem: SEM-gx-engine-777).
/// * **`ProvenanceDerived`** (**M5-25, adopted (a)**, hand 4, sem: SEM-gx-engine-777) — D-7's
///   third window, settled by making the
///   engine the producer and the journal the destination. No transition of 43 §3 writes one; which
///   record should carry it was raised as **M5H4-1**.
///
/// The two are the same category and not the same weight: E-M5-1 closes a counter-example (req/78
/// §3.2 Λ4), while `ProvenanceDerived` implements a ruling about where a value goes.
///
/// # 🔴 What changed in v0.2.4
///
/// Until `req/38` §68 #2 they were "an erratum this crate implements and the canon does not yet
/// carry" (sem: SEM-gx-engine-778), and the old assertion measured that gap verbatim:
///
/// ```text
///     assert_eq!(added, vec!["ApplyStarted", "ProvenanceDerived"],
///         "the additions are E-M5-1's and M5-25, adopted (a)'s, and no third; found {added:?}");
/// ```
///
/// 42 §3.13 now lists both (sem: SEM-gx-engine-779), so `added` is **empty** and the claim is
/// stronger rather than weaker:
/// the two sets are equal. Which of the thirteen have no transition is not this probe's question
/// any more — [`the_two_canon_documents_name_the_same_eleven_records`] owns it, against 43.
#[test]
fn the_implementation_is_the_canon_eleven_plus_two_errata() {
    let by_42 = names(&canon_42());
    let mine = names(&implemented());
    let added: Vec<&String> = mine.difference(&by_42).collect();
    let dropped: Vec<&String> = by_42.difference(&mine).collect();

    println!(
        "IMPLEMENTED_RECORDS={} ADDED={added:?} DROPPED={dropped:?}",
        mine.len()
    );
    assert_eq!(mine.len(), 15, "fifteen variants: {mine:?}");
    assert!(
        dropped.is_empty(),
        "these records of 42 §3.13 have no implementation: {dropped:?}"
    );
    assert!(
        added.is_empty(),
        "42 §3.13 carries every record this crate implements since v0.2.4 (§68 #2); these are not \
         in the canon: {added:?}"
    );
}

/// The declared list, the source and the `kind()` match are one list.
///
/// Three places name the twelve — `JOURNAL_RECORD_KINDS`, the enum itself, and the `match` in
/// `kind()` — and the third is held by the compiler (no `_` arm). This probe holds the first two
/// against each other, in order, so that a variant inserted in the middle without a row is caught.
#[test]
fn the_declared_kinds_are_the_variants_in_order() {
    let mine: Vec<String> = implemented().into_iter().map(|(n, _)| n).collect();
    println!(
        "JOURNAL_RECORD_KINDS={} SOURCE_VARIANTS={}",
        JOURNAL_RECORD_KINDS.len(),
        mine.len()
    );
    assert_eq!(
        JOURNAL_RECORD_KINDS.to_vec(),
        mine,
        "`JOURNAL_RECORD_KINDS` is not the variants of `EngineJournalRecord`, in order"
    );
}

// ---------------------------------------------------------------------------
// The two errata, measured
// ---------------------------------------------------------------------------

/// 🔴 **E-M5-3** — `DraftCreated` follows 43 T-1, and since v0.2.4 so does 42 §3.13.
///
/// This is the red-first probe req/78 §6.2, hand 1, ⑥ asks for: "**put the shape where M5-06's
/// discrepancy comes out RED** first" (sem: SEM-gx-engine-780). The second of its three
/// directions is the one that fired:
///
/// ```text
///     assert!(by_42.contains(&"transformation".to_string()),
///         "42 §3.13 no longer keys DraftCreated on `transformation`; E-M5-3's premise has moved");
/// ```
///
/// "the erratum was applied to the canon, and this comment is the thing to update" (sem:
/// SEM-gx-engine-781) — and that is what `req/38` §68 #2 did. `req/spec/` was not the M5
/// hand's to edit (52, contract 1); the v0.2.4 spec
/// window is the hand that has it.
///
/// It still fails in three separable ways, each a different mistake:
///
/// * the implementation grows `transformation` back — someone implemented 42's **old** literal
///   text, which mints a `TransformationId` before `plan` has run and breaks ASM-11's two-stage
///   identity;
/// * 42 §3.13 reverts to the old row — the erratum was backed out of the canon;
/// * 43 T-1's cell has stopped saying `DraftCreated{intent_id}` — the precedence clause of 51 §8.1
///   was resolved the other way, and the ruling has to be re-read.
#[test]
fn draft_created_follows_43_and_not_42() {
    let by_42 = fields_of(&canon_42(), "DraftCreated");
    let mine = fields_of(&implemented(), "DraftCreated");
    let t1 = canon_43();
    let ids = t1
        .get("DraftCreated")
        .expect("43 §3 writes a DraftCreated journal cell");

    println!("DRAFT_42={by_42:?} DRAFT_IMPL={mine:?} DRAFT_43_TRANSITIONS={ids:?}");
    assert!(
        !by_42.contains(&"transformation".to_string()),
        "42 §3.13 keys DraftCreated on `transformation` again; E-M5-3 has been backed out of the \
         canon (§68 #2 landed it)"
    );
    assert!(
        !mine.contains(&"transformation".to_string()),
        "E-M5-3: `DraftCreated` must not carry a `TransformationId` -- 43 T-1 is \"`TransformationId`\
         is not yet fixed (delta/target undetermined)\" (sem: SEM-gx-engine-782)"
    );
    assert!(
        mine.contains(&"intent_id".to_string()),
        "E-M5-3 keys the record on `IntentId`"
    );
    assert_eq!(
        by_42, mine,
        "canon and code disagree about `DraftCreated`; 51 §8.1's precedence clause is live again"
    );
    assert_eq!(
        ids,
        &vec!["T-1".to_string()],
        "DraftCreated is T-1's record"
    );
}

/// 🔴 `Planned` carries both ids, which is 43 T-2's cell — and since v0.2.4, 42 §3.13's row too.
///
/// The second half of E-M5-3 and the reason the two errata are one decision: with `DraftCreated` no
/// longer holding a `TransformationId`, `Planned` is the **only** record that ever holds both, and a
/// replay without it cannot say which draft became which candidate.
///
/// The old expectation, kept because it named the event that ended it:
///
/// ```text
///     assert!(!by_42.contains(&"intent_id".to_string()),
///         "42 §3.13's Planned row has gained an `intent_id`; the erratum's premise has moved");
/// ```
///
/// `req/38` §68 #2 gave it the `intent_id`. The premise did not *move* — it landed, which is what
/// the message anticipated. **The uniqueness claim is unaffected and is the real content here**: it
/// is a statement about the implementation's thirteen records, not about which document lists them.
#[test]
fn planned_is_the_only_record_holding_both_ids() {
    let mine = implemented();
    let by_42 = fields_of(&canon_42(), "Planned");
    let planned = fields_of(&mine, "Planned");

    let holding_both: Vec<&String> = mine
        .iter()
        .filter(|(_, f)| {
            f.contains(&"transformation".to_string()) && f.contains(&"intent_id".to_string())
        })
        .map(|(n, _)| n)
        .collect();

    println!(
        "PLANNED_42={by_42:?} PLANNED_IMPL={planned:?} RECORDS_WITH_BOTH_IDS={holding_both:?}"
    );
    assert!(
        by_42.contains(&"intent_id".to_string()),
        "42 §3.13's Planned row has lost its `intent_id`; §68 #2's E-M5-3 sync has been backed out"
    );
    assert!(
        planned.contains(&"intent_id".to_string()),
        "43 T-2's cell is `Planned{{intent_id, id, delta_cid, fp0}}`"
    );
    assert_eq!(
        holding_both,
        vec!["Planned"],
        "exactly one record binds an IntentId to a TransformationId"
    );
}

/// 🔴 **E-M5-13** — `Planned` carries `locator` and `parents`, and since v0.2.4 42 §3.13 does too.
///
/// The erratum was reserved twice and merged once: **M5H5-2** asked for the locator (43 §7-3c's
/// resume cannot re-run a plan whose target it cannot name, and v0.1 folds that into
/// `Aborted(InternalError)` — "lies that it's a wiring bug" (sem: SEM-gx-engine-783), received
/// in writing rather than fixed) and **M5H6-6** asked for the parents (T-12's guard "`T_u.parents`
/// contains `T_o.id`" is checkable from the journal alone only if the journal holds them). §43
/// merged the two: "**E-M5-13 is extended into one erratum: 'add locator (M5H5-2) and parents
/// (this matter) to the `Planned` record'**" (sem: SEM-gx-engine-783).
///
/// # Why here and not in M7
///
/// req/38 §47, M6-14, adopted (a), and the reason is a **window** rather than a cost. 47 §4 makes
/// journal schema compatibility an upgrade precondition ("makes it a precondition of pre-upgrade
/// verification that deterministic replay via `gx replay` **agrees between the old and new
/// binaries**", sem: SEM-gx-engine-784) and 33 NFR-024 allows a breaking change in
/// `0.y.z` only with a CHANGELOG entry. **M6 is the hand that builds the first distributable.**
/// Before distribution the cost of changing the record shape is zero; after it, every user's journal
/// is the cost. `.gx/VERSION` (req/56 §2) is where the receipt for that decision is kept.
///
/// # Why `parents` is a `Vec` that is almost always empty
///
/// Because the guard it serves is about the one case where it is not: an undo names what it undoes
/// (`vec![*original]`, `Engine::undo`), and every other `plan` writes an empty list. A field that is
/// empty on the common path is not a wasted field if the uncommon path is the one a crash loses.
///
/// # 🔴 v0.2.4: the instruction inside the old message was carried out
///
/// The old expectation told its reader what to do when it fell, and `req/38` §68 #2 did it:
///
/// ```text
///     assert!(!by_42.contains(&field.to_string()),
///         "42 §3.13's Planned row has gained `{field}`; E-M5-13 has landed in the canon and this
///          suite should compare against it rather than record the difference");
/// ```
///
/// ∴ this probe now **compares against it**. The record is asserted field-for-field in order, which
/// is strictly more than the old pair of membership checks: a rename or a reordering in either
/// document falls here.
#[test]
fn planned_carries_the_locator_and_the_parents() {
    let by_42 = fields_of(&canon_42(), "Planned");
    let mine = fields_of(&implemented(), "Planned");
    // 🔴 **DR-46-33** (`req/38` §413) added `input_generation` between `parents` and `at`, so
    // E-M5-13's seven are now eight.
    let eight = vec![
        "transformation",
        "intent_id",
        "locator",
        "delta_cid",
        "fp0",
        "parents",
        "input_generation",
        "at",
    ];

    println!("PLANNED_42_FIELDS={by_42:?} PLANNED_IMPL_FIELDS={mine:?}");
    for field in ["locator", "parents"] {
        assert!(
            by_42.contains(&field.to_string()),
            "42 §3.13's Planned row has lost `{field}`; §68 #2's E-M5-13 sync has been backed out"
        );
        assert!(
            mine.contains(&field.to_string()),
            "E-M5-13 (req/38 §42 M5H5-2 + §43 M5H6-6, implemented under §47 M6-14, adopted (a), \
             sem: SEM-gx-engine-785): \
             `Planned` must carry `{field}`; the implementation has {mine:?}"
        );
    }
    // The two ids stay where E-M5-3 put them: the erratum adds fields, it does not move any.
    assert!(
        mine.contains(&"transformation".to_string()) && mine.contains(&"intent_id".to_string()),
        "E-M5-13 must not disturb E-M5-3's pair: {mine:?}"
    );
    assert_eq!(
        mine, eight,
        "the implementation's `Planned` is not E-M5-13's seven plus DR-46-33's `input_generation`"
    );
    assert_eq!(
        by_42, mine,
        "42 §3.13's `Planned` row is not the implementation's"
    );
}

/// 🔴 **E-M5-1** — `ApplyStarted` sits between `InverseEscrowed` and `Committed`.
///
/// The position is the claim. 51 §8.1's third injection point is "after apply succeeds, before
/// `ledger.append`" (sem: SEM-gx-engine-786), and a record written *after* the apply would be
/// no use to a recovery that has to know
/// whether the call was made at all. Order in the enum is not the order of the protocol, so what is
/// asserted is the declaration order the source and `JOURNAL_RECORD_KINDS` agree on, plus its
/// absence from **43** — which is what still makes it an erratum rather than a transcription.
///
/// # 🔴 v0.2.4: 42 carries it now, 43 does not, and the difference is the definition
///
/// The old pair asserted absence from **both** canon documents:
///
/// ```text
///     assert!(!canon_43().contains_key("ApplyStarted"),
///         "43 §3 now names ApplyStarted; the erratum has landed in the canon and this suite should
///          compare against it instead");
///     assert!(!names(&canon_42()).contains("ApplyStarted"),
///         "42 §3.13 now names ApplyStarted; same");
/// ```
///
/// `req/38` §68 #2 landed it in 42 §3.13 and left 43 §3 untouched **on purpose**: `ApplyStarted` is
/// written inside T-10a/T-11's window, not at an edge of the state machine, so a row in the
/// transition table would invent a transition. ∴ the second assertion is inverted — 42 must name it
/// — and the first is kept as written: if 43 ever grows a cell for it, the record has become a
/// transition and this suite has to be re-read.
#[test]
fn apply_started_is_an_erratum_and_sits_between_escrow_and_commit() {
    let mine: Vec<String> = implemented().into_iter().map(|(n, _)| n).collect();
    let escrow = mine
        .iter()
        .position(|n| n == "InverseEscrowed")
        .expect("InverseEscrowed is implemented");
    let apply = mine
        .iter()
        .position(|n| n == "ApplyStarted")
        .expect("ApplyStarted is implemented");
    let committed = mine
        .iter()
        .position(|n| n == "Committed")
        .expect("Committed is implemented");

    println!(
        "ORDER_INVERSE_ESCROWED={escrow} ORDER_APPLY_STARTED={apply} ORDER_COMMITTED={committed}"
    );
    assert!(
        escrow < apply && apply < committed,
        "E-M5-1's record belongs between T-10b and T-11"
    );
    assert!(
        !canon_43().contains_key("ApplyStarted"),
        "43 §3 now names ApplyStarted; the record has become a transition and this suite should \
         compare against the table instead"
    );
    assert_eq!(
        fields_of(&canon_42(), "ApplyStarted"),
        fields_of(&implemented(), "ApplyStarted"),
        "42 §3.13's ApplyStarted row is not the implementation's; §68 #2 landed E-M5-1 in the canon"
    );
}

/// 🔴 **M5-25, adopted (a)** (sem: SEM-gx-engine-787) — `ProvenanceDerived` is written inside
/// the critical section and before it.
///
/// The position carries the argument. A `provenance` field on `Committed` would keep the vocabulary
/// at twelve and lose the value in exactly the window 43 §7-3b exists for: a crash after
/// `ledger.append` and before the `Committed` record. Re-deriving it during recovery is not
/// available — `Provenance::derive_from` needs the `Transformation` body, and the journal holds
/// names and digests (ASM-9). So the record is written where a crash cannot reach past it, which in
/// declaration order means after `CommittingStarted` and before `Committed`.
///
/// # 🔴 v0.2.4: same split as `ApplyStarted`
///
/// The old pair asserted absence from both documents; `req/38` §68 #2 landed the record in 42 §3.13
/// and left 43 §3 alone, for the same reason (**M5H4-1** asked which record carries the value, not
/// which transition writes it — no transition does). The 42 assertion is inverted to a field-level
/// comparison; the 43 assertion is kept verbatim:
///
/// ```text
///     assert!(!names(&canon_42()).contains("ProvenanceDerived"), "42 §3.13 now names it; same");
/// ```
#[test]
fn provenance_is_an_erratum_and_sits_inside_the_critical_section() {
    let mine: Vec<String> = implemented().into_iter().map(|(n, _)| n).collect();
    let position = |name: &str| {
        mine.iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("{name} is implemented"))
    };
    let committing = position("CommittingStarted");
    let provenance = position("ProvenanceDerived");
    let committed = position("Committed");
    let fields = fields_of(&implemented(), "ProvenanceDerived");

    println!(
        "ORDER_COMMITTING_STARTED={committing} ORDER_PROVENANCE={provenance} \
         ORDER_COMMITTED={committed} PROVENANCE_FIELDS={fields:?}"
    );
    assert!(
        committing < provenance && provenance < committed,
        "M5-25's record belongs inside the section T-9 opens"
    );
    assert_eq!(
        fields,
        vec!["transformation", "provenance", "at"],
        "the record carries 42 §3.9's value itself, not a digest of it"
    );
    assert!(
        !canon_43().contains_key("ProvenanceDerived"),
        "43 §3 now names it; the record has become a transition and this suite should compare \
         against the table instead"
    );
    assert_eq!(
        fields_of(&canon_42(), "ProvenanceDerived"),
        fields,
        "42 §3.13's ProvenanceDerived row is not the implementation's; §68 #2 landed M5-25, adopted \
         (a) (sem: SEM-gx-engine-788) \
         in the canon"
    );
}

/// 🔴 Every record 42 §3.13 lists has the same fields in the implementation. **The exempt set is
/// empty.**
///
/// # What this probe was, and why its exemptions are gone
///
/// It was the residue after the errata — the check that stops a hand from quietly renaming
/// `ledger_seq` or dropping `enforced` while attention is on the interesting variants — and it
/// carried a growing skip list, because five records diverged from the canon on purpose:
///
/// ```text
///     if name == "DraftCreated" || name == "Planned" { continue; }  // E-M5-3
///     if name == "Verdict" { continue; }                            // 43 T-4e (M5H2-1)
///     if name == "Aborted" { continue; }                            // AC-038 (M5H4-2)
///     if name == "HumanDecision" { continue; }                      // AC-071/072 (M5H6-2)
///     …
///     assert_eq!(compared, 6, "six records are none of the errata");
/// ```
///
/// The reasons are worth keeping even though the skips are not:
///
/// * `DraftCreated` / `Planned` — **E-M5-3**: 43 T-1 does not have a `TransformationId` to key on.
/// * `Verdict` — **M5H2-1**: 43 T-4e's cell is `Verdict{id, Admit, fail_posture_engaged=true}` and
///   42's row had no such field; the same clause of 51 §8.1 ("when they conflict, 43 takes
///   precedence", sem: SEM-gx-engine-789) in a third
///   place. Closed by v0.2.3 (`req/38` §67 ①).
/// * `Aborted` — **M5H4-2**, and its case was **weaker on purpose**: 43 T-10c does not write the
///   field either, so 51 §8.1 did not decide it. What decided it was AC-038 against a receipt that
///   is never issued for an abort (ASM-14).
/// * `HumanDecision` — **M5H6-2**: AC-071/072 ask the trail to carry the ruler's `reason` and
///   `actor`, and **E-M2-3** retired the `Evidence(HumanDecision)` variant they name.
///
/// **v0.2.4 (`req/38` §68 #2) landed all five in 42 §3.13**, so every record is a transcription and
/// the skip list has nothing left to hold. `compared` is the full thirteen.
///
/// 🔴 **An exemption was not a silence, and this is the proof.** Each divergence had a probe that
/// stated it exactly, and each of those probes now states the agreement instead — which is how a
/// doc-only batch that landed the errata was able to break the floor (**F-1**) rather than leave a
/// divergence nobody re-reads.
#[test]
fn the_other_records_are_transcriptions() {
    let by_42 = canon_42();
    let mine = implemented();
    let mut compared = 0;
    let mut differing: Vec<String> = Vec::new();

    for (name, fields) in &by_42 {
        compared += 1;
        let ours = fields_of(&mine, name);
        if &ours != fields {
            differing.push(format!("{name}: 42={fields:?} impl={ours:?}"));
        }
    }

    println!("TRANSCRIBED_RECORDS={compared} DIFFERING={differing:?}");
    assert_eq!(
        compared, 15,
        "since §68 #2 every record of 42 §3.13 is compared; none is exempt"
    );
    assert!(
        differing.is_empty(),
        "these records differ from 42 §3.13 without an erratum: {differing:?}"
    );
}

/// 🔴 **M5H2-1 — the erratum has landed, and this probe is what said it would.**
///
/// # What this probe measured until 2026-08-13, and what it measures now
///
/// It measured a **divergence**: 42 §3.13's `Verdict` row was four fields, the implementation wrote
/// five, and 43 T-4e's cell agreed with the implementation. The old expectation is kept verbatim
/// rather than deleted, because the reason it changed is the point:
///
/// ```text
///     assert_eq!(by_42, vec!["transformation", "kind", "verdict_digest", "at"],
///         "42 §3.13's Verdict row changed; if it grew `fail_posture_engaged` the erratum has landed");
/// ```
///
/// **v0.2.3 (`req/38` §67 ①) grew exactly that field**, so the assertion above went red on the
/// landed tree — 12 passed / 1 failed — and `req/38` §68 recorded it as **F-1: an intended red on
/// the floor** (sem: SEM-gx-engine-790).
/// That red is not a defect. The doc comment of [`the_other_records_are_transcriptions`] promised
/// it: "**An exemption is not a silence** … so that an erratum landing in 42 breaks a probe rather
/// than leaving a divergence nobody re-reads", and the message of the assertion itself named the
/// event that would break it. The probe did its job; **even a doc-only batch moves the floor, as
/// long as a test reads the doc** (§68, discipline 55, ①) (sem: SEM-gx-engine-790).
///
/// # The three claims now
///
/// 1. 42 §3.13's row **is** the five fields, `verdict_digest` among them as an `Option`.
/// 2. The implementation is the same five, in the same order — so canon and code are one text.
/// 3. 43 T-4e still writes `fail_posture_engaged=true`, which is the **source** both of them copy.
///
/// It still falls in three separable directions, and that is why the probe survives the erratum
/// rather than being deleted with it: if 42 reverts to the four fields, if the implementation drops
/// or reorders one, or if 43 T-4e stops asking for the flag — in which case canon and code would
/// agree with each other about something no transition requires.
#[test]
fn the_verdict_record_follows_43_t_4e_and_not_42() {
    let by_42 = fields_of(&canon_42(), "Verdict");
    let mine = fields_of(&implemented(), "Verdict");
    let five = vec![
        "transformation",
        "kind",
        "verdict_digest",
        "fail_posture_engaged",
        "at",
    ];

    println!("VERDICT_42={by_42:?} VERDICT_IMPL={mine:?}");
    assert_eq!(
        by_42, five,
        "42 §3.13's Verdict row is no longer 43 T-4e's cell; v0.2.3 §67 ① has been reverted"
    );
    assert_eq!(mine, five, "the implementation is not 43 T-4e's cell");
    assert_eq!(
        by_42, mine,
        "canon and code disagree about `Verdict` again; 51 §8.1's precedence clause is live once more"
    );

    let canon_43_text = read_repo("req/spec/40-architecture/43-state-machine.md");
    assert!(
        canon_43_text.contains("fail_posture_engaged=true"),
        "43 T-4e no longer asks for the flag, so the agreement of 42 and the implementation has no source"
    );
}

/// 🔴 **M5H4-2**: what `Aborted` carries that 42 §3.13 does not, and why it had nowhere else to go.
///
/// AC-038, verbatim: "an automatic rollback attempt occurs, and the result is recorded as
/// `Aborted(ApplyFailed)`, and **journal/Receipt records whether a rollback attempt occurred
/// (success/failure)**" (sem: SEM-gx-engine-791). Both seats are missing:
///
/// * the **receipt** — ASM-14 defines two kinds, and neither is issued for an aborted
///   transformation, so there is no receipt to record it on;
/// * the **journal** — 42 §3.13's `Aborted` row is `{transformation, reason, at}`, and 43 T-10c's
///   cell writes the same three.
///
/// So the field was added to the record 43 T-10c itself names, and the divergence was asserted here
/// rather than left to a reviewer.
///
/// # 🔴 v0.2.4: the third direction fired, and it was the intended one
///
/// ```text
///     assert_eq!(by_42, vec!["transformation", "reason", "at"],
///         "42 §3.13's Aborted row changed; if it grew `rollback` the erratum has landed");
/// ```
///
/// `req/38` §68 #2 grew it. The three directions are kept and re-aimed: the implementation
/// reverting, **42 losing the field again**, and AC-038 ceasing to ask — the last being the one
/// that would leave canon and code agreeing about something no criterion requires.
#[test]
fn the_aborted_record_carries_what_ac_038_asks_to_be_recorded() {
    let by_42 = fields_of(&canon_42(), "Aborted");
    let mine = fields_of(&implemented(), "Aborted");
    let four = vec!["transformation", "reason", "rollback", "at"];

    println!("ABORTED_42={by_42:?} ABORTED_IMPL={mine:?}");
    assert_eq!(
        by_42, four,
        "42 §3.13's Aborted row has lost `rollback`; §68 #2's M5H4-2 sync has been backed out"
    );
    assert_eq!(
        mine, four,
        "the implementation does not carry AC-038's outcome"
    );

    let acceptance = read_repo("req/spec/30-requirements/34-acceptance.md");
    assert!(
        acceptance.contains("Aborted(ApplyFailed)"), // (sem: SEM-gx-engine-792)
        "AC-038 no longer asks for the rollback outcome, so the record has no source"
    );
}

/// 🔴 **M5H6-2**: what `HumanDecision` carries that 42 §3.13 does not, and who asked for it.
///
/// AC-071, verbatim: "confirm that the receipt trail issued (journal/Receipt metadata) includes
/// `Evidence(HumanDecision)` (decision=Admit, reason, ruler actor)" (sem: SEM-gx-engine-793),
/// and AC-072 asks for "journal record includes `Evidence(HumanDecision)` (decision=Deny,
/// reason)". Three seats are needed and
/// two of the three roads are closed:
///
/// * the **`Evidence` variant** those criteria name — retired by **E-M2-3**: "Evidence: 42's four
///   variants are canonical (43/44/34/35's `HumanDecision` references are the erratum...)" (sem:
///   SEM-gx-engine-794), whose own words send the fact to 43 T-5's "human-ruling receipt
///   (signed)";
/// * the **receipt** — which carries the ruler's `key_id` inside the signed payload, and no
///   `reason`: 42 §3.10's eleven fields have no seat for free text;
/// * the **journal** — 42 §3.13's row is `{transformation, kind, at}`.
///
/// So the record 43 T-5 itself names gained the two fields, and the divergence was asserted here in
/// three separable directions, as `Aborted`'s is.
///
/// # 🔴 v0.2.4: 42 grew the two fields
///
/// ```text
///     assert_eq!(by_42, vec!["transformation", "kind", "at"],
///         "42 §3.13's HumanDecision row changed; if it grew the two fields the erratum has landed");
/// ```
///
/// `req/38` §68 #2 landed **M5H6-2**. The three directions are kept and re-aimed, as `Aborted`'s
/// are — including the one that matters most here: AC-071's verbatim text (sem:
/// SEM-gx-engine-795) is the **only** source for
/// `reason` and `actor`, because **E-M2-3** closed the `Evidence` road and 42 §3.10's receipt has
/// no seat for free text. If the criterion stops asking, two fields are left with no requirement.
#[test]
fn the_human_decision_record_carries_what_ac_071_asks_to_be_recorded() {
    let by_42 = fields_of(&canon_42(), "HumanDecision");
    let mine = fields_of(&implemented(), "HumanDecision");
    // 🔴 **DR-46-31** (`req/38` §261 ruling 2b) — a sixth. AC-071 is still the only source for
    // `reason` and `actor`; `verdict_digest`'s source is a different kind of requirement, and it is
    // 43 §7-3b's rebuild: without the digest in the journal, Σ cannot name the verdict the receipt
    // was signed over and `Engine::reissue_receipt` refuses every escalated commit. The field is a
    // **canon addition** and not a preserved divergence, per `req/38` §265 ruling 2's second
    // clause: a field-shaped erratum goes into 42 §3.13, because the machine gate above parses at
    // field granularity and a canon left unmoved would make this test the divergence.
    let six = vec![
        "transformation",
        "kind",
        "reason",
        "actor",
        "verdict_digest",
        "at",
    ];

    println!("HUMAN_DECISION_42={by_42:?} HUMAN_DECISION_IMPL={mine:?}");
    assert_eq!(
        by_42, six,
        "42 §3.13's HumanDecision row has lost `reason`/`actor` (§68 #2's M5H6-2 sync) or \
         `verdict_digest` (E-DR4631-1)"
    );
    assert_eq!(
        mine, six,
        "the implementation does not carry AC-071's reason and ruler, or DR-46-31's ruling digest"
    );

    let acceptance = read_repo("req/spec/30-requirements/34-acceptance.md");
    assert!(
        acceptance.contains("decision=Admit, reason"), // (sem: SEM-gx-engine-796)
        "AC-071 no longer asks for the reason and the ruler, so the record has no source"
    );
}

/// Every record this crate implements is written by a transition 43 §3 defines — or is one of the
/// two errata.
///
/// The direction the probes above do not cover: a name can match and still be orphaned. Eleven of
/// the fifteen resolve to at least one `T-` id in the transition table, and the other four are
/// `ApplyStarted`, `ProvenanceDerived` (§37 rules both before 43 carries either) and two-phase
/// escrow's `ApplyObserved` / `InverseCompleted` (req/38 §98 ruling 1, sem: SEM-gx-engine-797
/// — moments inside T-9's
/// section, not transitions).
#[test]
fn every_record_has_a_transition_that_writes_it() {
    let by_43 = canon_43();
    let mut orphans: Vec<&str> = Vec::new();
    let mut total_ids = 0;
    for kind in JOURNAL_RECORD_KINDS {
        match by_43.get(kind) {
            Some(ids) => total_ids += ids.len(),
            None if kind == "ApplyStarted"
                || kind == "ProvenanceDerived"
                || kind == "ApplyObserved"
                || kind == "InverseCompleted" => {}
            None => orphans.push(kind),
        }
    }
    println!(
        "RECORDS={} TRANSITION_CELLS={total_ids} ORPHANS={orphans:?}",
        JOURNAL_RECORD_KINDS.len()
    );
    assert!(
        orphans.is_empty(),
        "these records are written by no transition of 43 §3: {orphans:?}"
    );
}
