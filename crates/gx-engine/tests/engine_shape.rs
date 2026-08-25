// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What this crate is allowed to be, measured: its modules, its dependencies, its error vocabulary,
//! and the four things it must not contain yet.
//!
//! req/78 §6.2 hand 1 ① is the mechanical half of this hand's DoD — "`cargo metadata` shows
//! **member 10**, `cargo build --workspace` exits 0, `cargo tree` shows 0 cycles, and the
//! shipping dependencies carry **zero `gx-adapter-fs`** (N-13's mechanical check)" (sem:
//! SEM-gx-engine-705) — and the membership and adapter halves live in
//! `probes/doubt/tests/workspace_doubt.rs`, outside the crate they judge. What is here is what only
//! this crate's own source can answer.
//!
//! # The "absence" scans (sem: SEM-gx-engine-706), and why they need a mutation to mean anything
//!
//! Three of the probes below assert an absence: no adapter named, no clock read, no transition
//! implemented. §30's lesson is that an absence scan which never sees a presence is a scan that
//! could be measuring the wrong thing, so each one is written against **invocation lines rather
//! than comments** (the file is full of prose naming `gx-adapter-fs` and `apply`), and
//! `tools/verify_m5h1.sh` §4 adds the presence by mutation and prints which probes notice.

mod support;

use gx_engine::ERROR_KINDS;
use support::{read_repo, repo_root};

/// The crate's own source files, as `(name, text)`.
fn sources() -> Vec<(String, String)> {
    let dir = repo_root().join("crates/gx-engine/src");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("the crate has a src/")
        .map(|entry| {
            let path = entry.expect("a directory entry").path();
            (
                path.file_name()
                    .expect("a named file")
                    .to_string_lossy()
                    .to_string(),
                std::fs::read_to_string(&path).expect("a source file is readable"),
            )
        })
        .collect();
    out.sort();
    out
}

/// The lines of a source file that are not comments.
///
/// "only invocation lines are the target" (sem: SEM-gx-engine-707) (§30's false-positive
/// lesson, reflected in the M4 instruments):
/// this crate's documentation names `gx-adapter-fs`, `apply` and `SystemTime` while discussing what
/// it does not do, and a scan that read prose would report the discussion as the thing.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with("*") && !l.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// 41 §2: the module list
// ---------------------------------------------------------------------------

/// The modules on disk are 41 §2's four, and `pipeline` is now one of them.
///
/// 41 §2 writes `src/{lib,pipeline,store,replay}.rs` for this crate. req/78 §2.1 proposed a
/// different split (`lifecycle`, `journal`, `recovery`, `engine`, `clock`, `rng`) on the stated
/// premise "41 §2's crate skeleton has no module breakdown for gx-engine" (sem:
/// SEM-gx-engine-708) — which is **not what 41 §2 says**;
/// the line is there, beside every other crate's. Hand 1 raised that as **M5H1-5** and req/38 §38
/// ruled adopted (a) (sem: SEM-gx-engine-708): "**41 §2's four modules
/// (lib/pipeline/store/replay) are canon** … hand 2's eight entry points go in
/// `pipeline.rs`," recording req/78's seven-module proposal as withdrawn.
///
/// So this hand adds the fourth file and the assertion flips: `pipeline.rs` was required to be
/// **absent** in hand 1 ("not one transition is implemented," sem: SEM-gx-engine-709) and is
/// required to be **present** now. The set
/// is checked in both directions, so a fifth module is as much a failure as a missing one.
#[test]
fn the_modules_are_the_ones_41_2_names() {
    let canon = read_repo("req/spec/40-architecture/41-architecture.md");
    let line = canon
        .lines()
        .find(|l| l.contains("src/{lib,pipeline,store,replay}.rs"))
        .expect("41 §2 gives gx-engine a module list");
    let declared: Vec<&str> = line
        .split_once('{')
        .and_then(|(_, r)| r.split_once('}'))
        .expect("the list is brace-delimited")
        .0
        .split(',')
        .collect();

    let on_disk: Vec<String> = sources()
        .into_iter()
        .map(|(name, _)| name.trim_end_matches(".rs").to_string())
        .collect();

    println!("CANON_MODULES={declared:?} MODULES_ON_DISK={on_disk:?}");
    assert_eq!(declared.len(), 4, "41 §2 names four modules");
    for module in &on_disk {
        assert!(
            declared.contains(&module.as_str()),
            "`{module}.rs` is not one of 41 §2's four; adding a module is an erratum (M5H1-5)"
        );
    }
    let mut expected: Vec<String> = declared.iter().map(|m| (*m).to_string()).collect();
    expected.sort();
    assert_eq!(
        on_disk, expected,
        "M5H1-5 adopted (a) (sem: SEM-gx-engine-710): the four 41 §2 names, all of them present and no fifth"
    );
}

// ---------------------------------------------------------------------------
// The error vocabulary (E-M2-23 / H-3)
// ---------------------------------------------------------------------------

/// `ERROR_KINDS` is the variants of `Error`, read out of the source.
///
/// The same instrument gx-core, gx-gate and gx-substrate carry. The `match` in `Error::kind` is the
/// third place and is held by the compiler (no `_` arm), which is why this probe only has two lists
/// to compare.
#[test]
fn the_error_vocabulary_is_the_error_enum() {
    let source = read_repo("crates/gx-engine/src/lib.rs");
    let start = source
        .find("pub enum Error {")
        .expect("lib.rs declares `pub enum Error`");
    let body = &source[start..];
    let end = body.find("\n}").expect("the enum closes");
    let variants: Vec<String> = body[..end]
        .lines()
        .map(str::trim)
        .filter(|l| {
            l.chars().next().is_some_and(char::is_uppercase)
                && (l.ends_with('{') || l.ends_with(','))
        })
        .map(|l| {
            l.trim_end_matches(&[' ', '{', ','][..])
                .split(&['(', ' '][..])
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .collect();

    println!("GX_ENGINE_ERROR_KINDS={} ({variants:?})", ERROR_KINDS.len());
    assert_eq!(
        ERROR_KINDS.to_vec(),
        variants,
        "`ERROR_KINDS` is not the variants of `Error`, in order"
    );
}

// ---------------------------------------------------------------------------
// The three absences
// ---------------------------------------------------------------------------

/// 🔴 **N-13** from the inside: no source line in this crate names an adapter crate.
///
/// The manifest half is `workspace_doubt.rs`; this is the half that would catch a `use` added
/// against a dependency somebody put back. "the same engine regardless of substrate" (sem:
/// SEM-gx-engine-711) is a claim about the
/// artefact, and an artefact that mentions one substrate's crate is not it.
#[test]
fn no_source_line_names_an_adapter() {
    let mut offenders: Vec<String> = Vec::new();
    for (name, text) in sources() {
        for line in code_lines(&text) {
            if line.contains("gx_adapter") || line.contains("gx-adapter") {
                offenders.push(format!("{name}: {line}"));
            }
        }
    }
    println!("ADAPTER_MENTIONS_IN_CODE={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "N-13: the engine names an adapter in code: {offenders:?}"
    );
}

/// 41 §6: "randomness and clock time are injected at the engine boundary" (sem:
/// SEM-gx-engine-712) — so the engine reads neither.
///
/// The point of the sentence is FR-039: a replay that re-read the clock would not be deterministic,
/// and a `rng_seed` recorded in `DraftCreated` means nothing if something downstream reaches for
/// entropy of its own. `Timestamp` arrives as an argument in every journal record, and this probe
/// is what says there is no second road. The same scan gx-substrate carries (M4-17).
#[test]
fn the_engine_reads_no_clock_and_no_entropy() {
    let mut offenders: Vec<String> = Vec::new();
    for (name, text) in sources() {
        for line in code_lines(&text) {
            for needle in ["SystemTime", "Instant::now", "thread_rng", "random("] {
                if line.contains(needle) {
                    offenders.push(format!("{name}: {line}"));
                }
            }
        }
    }
    println!("CLOCK_AND_RNG_READS={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "41 §6 injects both at the engine boundary: {offenders:?}"
    );
}

/// 🔴 The scope boundary, as a measurement: **all eight entry points, and no ninth**.
///
/// The eight names are 41 §5's and req/78 §2.1's, and M5H1-5 adopted (a) (sem:
/// SEM-gx-engine-713) puts all eight in `pipeline.rs`
/// when they arrive. req/78 §6.2 splits them across hands by 43 §3's transition ids, and hand 4's
/// row is "**T-9/T-10a/T-10c/T-11** (commit's critical section)" (sem: SEM-gx-engine-713) —
/// which is `commit`, and T-10b with it,
/// since 43 T-10b is an internal step of the same section rather than an entry point of its own:
///
/// | entry point | transitions | hand |
/// |---|---|---|
/// | `submit` | T-1 | 2 |
/// | `plan` | T-2 | 2 |
/// | `verify` | T-3, T-4a..T-4e | 2 |
/// | `canonicalize` | T-8, T-8r | 2 |
/// | `commit` | T-9, T-10a/b/c, T-11 | **4** |
/// | `undo` | T-12 | 6 |
/// | `cancel` | T-7 | 6 |
/// | `escalation` | T-5, T-5b | 6 |
///
/// Hand 1 asserted all eight absent, hand 2 four of them present, hand 4 five — and **hand 6 flips
/// the last three**, which is why this probe changed rather than being satisfied. That is the
/// design working in both directions: the same assertion that stopped hand 4 from reaching into
/// T-12 is what now records that hand 6 arrived, and a hand 7 that dropped one would fail it.
/// Each name is searched for as a function declaration rather than as a word, because this crate's
/// documentation discusses all eight at length.
///
/// 🔴 `Engine::recover` (hand 5) and `Engine::reap` (hand 6, **M5-10 adopted (b)**, sem:
/// SEM-gx-engine-714) are **not** entry
/// points and are deliberately not in either list. 43 §7 is a procedure over the journal and 43 T-6
/// is a transition nobody triggers on purpose; neither is one of req/78 §6.2's eight, and counting
/// them would make "the eight" (sem: SEM-gx-engine-715) a number that grows whenever a hand
/// adds a public function.
#[test]
fn exactly_the_eight_entry_points_of_req_78_are_implemented() {
    let mine = [
        "submit",
        "plan",
        "verify",
        "canonicalize",
        "commit",
        "undo",
        "cancel",
        "escalation",
    ];
    let later: [&str; 0] = [];

    let mut declared: Vec<String> = Vec::new();
    for (name, text) in sources() {
        for line in code_lines(&text) {
            for entry in mine.iter().chain(later.iter()) {
                // The `(` is load-bearing: `pub fn plan` is a prefix of `pub fn planned_delta`, and
                // the first draft of this probe reported `plan` twice and failed its own
                // one-road-each assertion. §30's lesson, third sighting in this hand.
                if line.contains(&format!("pub fn {entry}(")) {
                    declared.push(format!("{name}::{entry}"));
                }
            }
        }
    }
    declared.sort();
    println!("TRANSITION_ENTRY_POINTS={} ({declared:?})", declared.len());

    for entry in mine {
        assert!(
            declared.iter().any(|d| d.ends_with(&format!("::{entry}"))),
            "hands 2 and 4 own T-1..T-4e, T-8/T-8r and T-9..T-11, and `{entry}` is one of their \
             five: {declared:?}"
        );
        assert!(
            declared
                .iter()
                .filter(|d| d.ends_with(&format!("::{entry}")))
                .count()
                == 1,
            "`{entry}` is declared more than once; 43's transitions have one road each"
        );
    }
    for entry in later {
        assert!(
            !declared.iter().any(|d| d.ends_with(&format!("::{entry}"))),
            "`{entry}` belongs to hand 6 (undo/cancel/escalation); reaching ahead is how a \
             transition gets written without the rulings that gate it: {declared:?}"
        );
    }
    assert!(
        declared.iter().all(|d| d.starts_with("pipeline.rs::")),
        "M5H1-5 adopted (a) (sem: SEM-gx-engine-716): \"hand 2's eight entry points go in \
         `pipeline.rs`\" -- {declared:?}"
    );
}

/// 🔴 **M5-03 adopted (a)** (sem: SEM-gx-engine-717): `EvidenceSource`'s `Err` is the **only**
/// producer of `VerifierUnavailable`.
///
/// req/38 §37 rules the evidence entry point as one trait and fixes what its failure means:
///
/// > **M5-03 adopted (a)** (sem: SEM-gx-engine-718): one `EvidenceSource` trait goes in
/// > gx-engine … **`Err` is the sole producer of `VerifierUnavailable`**
///
/// and **E-M5-4** (M5-19 adopted (a), sem: SEM-gx-engine-718) is the same sentence from 43's
/// side: "the only source of unreachability is the evidence collector," which is what makes
/// AC-036's "`kill -9` the gx-gate process" constructible
/// at all -- gx-gate is a library and cannot be unreachable.
///
/// "the only one" (sem: SEM-gx-engine-719) is a claim about absence, so §30's rule applies: it
/// needs a presence to be worth
/// anything. Two instruments carry it. This one is the source scan -- the reason word is written in
/// exactly one place in `src/` -- and the behavioural half is `tests/ac_032.rs`, where a run whose
/// collector answers `Ok` never reaches the reason and a run whose collector answers `Err` always
/// does. `tools/verify_m5h2.sh` §4 adds a second producer by mutation and prints which probes notice.
/// "One producer" (sem: SEM-gx-engine-720) is one *function*, not one line: the transition
/// writes the reason into the
/// journal and then into the in-memory state, and journal-first means those are the record and its
/// cache rather than two productions. So the scan finds every mention and then asks which function
/// each one is inside, which is the claim M5-03 actually makes.
#[test]
fn verifier_unavailable_has_exactly_one_producer() {
    let mut producers: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut mentions = 0usize;

    for (name, text) in sources() {
        let mut current = String::from("<file scope>");
        for line in code_lines(&text) {
            if let Some(rest) = line
                .strip_prefix("fn ")
                .or_else(|| line.strip_prefix("pub fn "))
            {
                current = format!(
                    "{name}::{}",
                    rest.split('(').next().unwrap_or_default().trim()
                );
            }
            if line.contains("VerifierUnavailable") {
                mentions += 1;
                producers.insert(current.clone());
            }
        }
    }

    println!("VERIFIER_UNAVAILABLE_MENTIONS={mentions} PRODUCERS={producers:?}");
    assert_eq!(
        producers.len(),
        1,
        "M5-03 adopted (a) (sem: SEM-gx-engine-721) / E-M5-4: one producer, and it is the arm that handles \
         `EvidenceSource::collect`'s Err -- {producers:?}"
    );
    let producer = producers.iter().next().expect("exactly one");
    assert_eq!(
        producer, "pipeline.rs::unreachable_collector",
        "the reason is produced somewhere other than the collector's failure arm"
    );
}

/// The crate root forbids `unsafe`, and says so in the file rather than in a manifest lint.
///
/// `unsafe_forbidden.rs` in gx-canon is the workspace-wide instrument and names this root in its
/// list; this is the local half, so that a hand working in this crate sees the failure without
/// running another package's suite.
#[test]
fn the_crate_root_forbids_unsafe() {
    assert!(
        read_repo("crates/gx-engine/src/lib.rs").contains("#![forbid(unsafe_code)]"),
        "41 §6: `#![forbid(unsafe_code)]` is per crate root"
    );
}

/// 🔴 **M6-02 adopted (a)** (sem: SEM-gx-engine-722) — the id-resolution accessor exists, and
/// it is an accessor.
///
/// 44 §0's id-resolution rule receives "either an `IntentId` or a `TransformationId`'s `gx1:...`
/// value is accepted … after `plan()` completes, it resolves to the canonical
/// `TransformationId`" (sem: SEM-gx-engine-722), and until M6 hand 1 the engine had only
/// the forward map (`intent_of(&TransformationId) -> Option<IntentId>`). req/88 M6-02 measured the
/// hole and §47 adopted (a)+(b): the inverse is the engine's, the `.gx/index/` copy is the CLI's
/// cache.
///
/// A **text** probe beside the behavioural one in `tests/id_resolution.rs`, because two of the three
/// claims are about the shape rather than the answer: that it takes `&self` (a read of the table, not
/// a transition — Rule 1 (sem: SEM-gx-engine-723) in the engine's own direction: an accessor
/// that took `&mut self` would be a
/// ninth entry point and 43 has eight), and that the CLI's `gx1:` parsing has a target here rather
/// than a scan. `engine_shape.rs` is where the other "the signature is the claim" (sem:
/// SEM-gx-engine-723) probes live.
#[test]
fn the_id_resolution_inverse_is_an_accessor() {
    let pipeline = read_repo("crates/gx-engine/src/pipeline.rs");
    let forward = pipeline.contains("pub fn intent_of(&self, id: &TransformationId)");
    let inverse = pipeline.contains("pub fn resolved(&self, intent_id: &IntentId)");
    let mutating = pipeline.contains("pub fn resolved(&mut self");
    println!(
        "ID_RESOLUTION_FORWARD={} INVERSE={} INVERSE_TAKES_MUT={}",
        u8::from(forward),
        u8::from(inverse),
        u8::from(mutating)
    );
    assert!(
        forward,
        "`intent_of` is the forward half and has been here since M5"
    );
    assert!(
        inverse,
        "M6-02 adopted (a) (sem: SEM-gx-engine-724): `pub fn resolved(&self, intent_id: &IntentId) -> Option<TransformationId>` \
         is the inverse 44 §0 needs; without it a CLI holding an `IntentId` can only scan"
    );
    assert!(
        !mutating,
        "the inverse is a read. An entry point that took `&mut self` would be a ninth road into \
         the state machine and 43 has eight"
    );
}
