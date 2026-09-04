// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R27 item 4 (`req/331` §0-4, from `req/329` L-01, `req/38` §233 ruling 5)** — a numeric
//! claim on `docs/LIMITS.md` does not stop being checked when the anchor moves past it.
//!
//! # What broke
//!
//! `limits_sync.rs` holds *the newest stacked block* — everything after the last `v0.5-x (` — and
//! its needles are checked against that window only. When R26 moved the anchor from `v0.5-l` to
//! `v0.5-m`, every claim `v0.5-l` had made left the checked set in the same commit. One of them was
//! a probe count that R26 itself falsified: the same release added an arm to
//! `r25_abort_and_record_only.rs` and raised `limits_sync`'s own declaration from eight to nine,
//! while the sentence on the page went on saying **eight**. `req/326` §3 predicted this shape in a
//! 🔴 row; the twenty-sixth audit found it had happened.
//!
//! # The convention this file makes mechanical
//!
//! A stacked page is **additive**: an old block is a record of what was true when it was written,
//! and rewriting it would destroy the record. So a stale number in a historical block is not by
//! itself a defect. What is a defect is a stale number that nothing later corrects — the page's
//! **most recent** statement about a file has to be true of the tree.
//!
//! Rather than parse that out of prose (a scan loose enough to find every phrasing is loose enough
//! to read "two of them" as a probe count, and a gate with false positives makes a page worse), the
//! rule is a **registry**: each entry names a suite, the count this tree has, and the correction
//! line the page must carry. Anchor moves are then a decision rather than an accident — for every
//! numeric claim in the outgoing block, a lane either carries it into this registry or writes the
//! correction the registry demands.
//!
//! # What this file found beyond the audit's one
//!
//! The audit filed `r25_abort_and_record_only.rs`. Applying the same question to the whole page
//! turned up two more claims that no later block corrects, both older than the audit's:
//! `serve_runtime_r3.rs` (page **seven**, tree eight) and `serve_runtime_r6.rs` (page **fifteen**,
//! tree sixteen). `declaration_writer_doubt.rs` looked stale and is not — a later block says
//! *"still **seven**"* — which is the convention working, and is why the rule is *the last statement
//! must be true* rather than *every statement must be true*.

use std::path::{Path, PathBuf};

#[path = "support/probe_counts.rs"]
mod probe_counts;

use probe_counts::{probe_count, probe_names, substring_count};

fn record(line: &str) {
    println!("{line}");
    if let Ok(path) = std::env::var("R27_MEASUREMENTS") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/gx-cli -> repo root")
        .to_path_buf()
}

/// Registry rows that exist only in the private tree (req/833): `probes/doubt` is not in the
/// public sync set at all (req/789 §3), and `boundary_attest.rs` is on the canon-reading
/// exclusion set executed in the resync (req/38 SS772-773). In a tree whose workspace root does
/// not declare `probes/doubt` — the published tree's own 14-member root, req/817 — these two rows
/// are set aside loudly; in the private tree their absence still fails.
const PRIVATE_TREE_ONLY: &[&str] = &[
    "probes/doubt/tests/declaration_writer_doubt.rs",
    "crates/gx-witness/tests/boundary_attest.rs",
    // 🔴 **req/839** — the two req/824 fixture-driven suites read `req/wire/fixtures/*.jsonl`
    // at runtime and `req/wire/` does not ship, so the suites are withheld from the published
    // tree (`tools/pub_sync_dryrun.sh` HAND_FLOOR) and their rows are held against the private
    // tree, exactly as the two above.
    "crates/gx-core/tests/observation_class.rs",
    "crates/gx-api/tests/attach_sources.rs",
    // 🔴 **req/850** — A5's fixture-driven suite, withheld for exactly the two rows above's
    // reason (it reads `req/wire/fixtures/observation.jsonl` at runtime).
    "crates/gx-api/tests/observations.rs",
    // 🔴 **req/999 F-3 (2026-08-31)** — the two `probes/doubt` read-surface suites, held for
    // `declaration_writer_doubt.rs`'s reason exactly: `probes/doubt` is not in the public sync
    // set at all (req/789 §3), so in the published tree these rows are set aside loudly and in
    // the private tree their absence still fails.
    "probes/doubt/tests/inference_closed_doubt.rs",
    "probes/doubt/tests/read_surface_census_doubt.rs",
    // 🔴 **H-9 / `req/954` §3-1 / `req/983`** — the C-25 canon suite, held for the reason the rows
    // above are held: no commit reachable in the published tree carries the path, `docs/LIMITS.md`
    // never names it, and the requirements its registry row cites do not ship, so the whole H-9
    // material was withheld rather than this one file being dropped. That is inferred from the
    // published tree because the private tree is not on the machine this row was written on; if the
    // inference is wrong, the private tree fails on the absence, which is where it should surface.
    "crates/gx-witness/tests/r964_c25_canon.rs",
];

/// Whether a registry row is held against the **private** tree rather than against this one.
///
/// True only for a `PRIVATE_TREE_ONLY` path that this tree does not carry, in a tree whose
/// workspace does not declare `probes/doubt`. A suite this tree does carry is never set aside, so
/// `boundary_attest.rs` stays held here even though its row is on the list, and in the private tree
/// nothing is set aside at all.
/// What both probes say when they set a row aside, so that a skip reads the same in either log.
const SET_ASIDE: &str = "a private-tree registry row (req/789 §3 / SS772-773 canon exclusion), \
                         absent from this tree, whose workspace does not declare probes/doubt \
                         (published tree, req/817). Held against the private tree (req/833).";

fn held_against_the_private_tree(path: &str) -> bool {
    PRIVATE_TREE_ONLY.contains(&path)
        && !workspace_declares("probes/doubt")
        && !repo_root().join(path).exists()
}

/// Whether this tree's workspace declares `member` — read from the root `Cargo.toml`'s
/// `members` array, inside which nothing but member paths may be written (the public root's own
/// rule, `public/Cargo.toml`).
fn workspace_declares(member: &str) -> bool {
    let manifest = repo_root().join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", manifest.display()));
    let Some(start) = text.find("members = [") else {
        return false;
    };
    let Some(end) = text[start..].find(']') else {
        return false;
    };
    text[start..start + end]
        .lines()
        .any(|l| l.trim().trim_end_matches(',').trim_matches('"') == member)
}

fn limits() -> String {
    std::fs::read_to_string(repo_root().join("docs/LIMITS.md")).expect("docs/LIMITS.md is readable")
}

/// The number words this page writes counts in, so the registry states them the way a reader reads
/// them rather than as digits the prose never uses.
const WORDS: [(&str, usize); 21] = [
    ("zero", 0),
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
];

fn value_of(word: &str) -> usize {
    WORDS
        .iter()
        .find(|(w, _)| *w == word)
        .map(|(_, n)| *n)
        .unwrap_or_else(|| panic!("the registry writes counts in words this file knows: {word}"))
}

/// 🔴 The registry: suites whose probe count the page states, and the word this tree's count is.
///
/// An entry is added when a block makes a numeric claim about a suite, and it is **never removed
/// when the anchor moves past that block** — which is the whole point. The correction the page owes
/// is checked by [`the_page_carries_a_current_statement_for_every_registered_suite`].
const REGISTERED: [(&str, &str); 42] = [
    // 🔴 **req/824 A5 / req/850** — the observation-ingest suite, registered in the commit that
    // writes it (the convention the three rows below were repaired into, applied on time for
    // once). Private-tree-only: its bed sits under `req/wire/`, which the published tree does
    // not carry.
    ("crates/gx-api/tests/observations.rs", "four"),
    // 🔴 **req/824 A1-A4 / req/839** — the three suites the 2026-08-26 blocks name. Found RED by
    // the first *public* fresh-clone run after the A1-A4 sync (arm d, three unregistered paths):
    // the LIMITS blocks landed in `e1a0ab71`/`16863593` without their registry rows, and the
    // private AFTER legs of both lanes ran targeted suites in which arm d's red was not read back
    // — the convention this file makes mechanical caught it at the next full run, which is its
    // job. Two of the three are private-tree-only rows (see `PRIVATE_TREE_ONLY`).
    ("crates/gx-core/tests/observation_class.rs", "seven"),
    ("crates/gx-canon/tests/authority_boundary.rs", "nine"),
    ("crates/gx-api/tests/attach_sources.rs", "four"),
    // 🔴 **R40 / `req/553` M-01 + L-02 (`req/38` §328)** — this release's two suites, registered in
    // the commit that writes them, which is the convention this file exists to make mechanical
    // rather than remembered. Both blocks R40 stacks state a probe count, and both counts are held
    // here: the first because the limit it drives (`INTERNAL` standing in for a word 44 §2.3 does
    // not have) is one a later DR is expected to move, and the second because a limit whose only
    // evidence is prose is the shape `req/329` L-01 was filed about.
    ("crates/gx-cli/tests/r40_journal_presence.rs", "eight"),
    ("crates/gx-cli/tests/r40_serving_routes.rs", "two"),
    // 🔴 **R33 / `req/397` H-01** — not a suite this release wrote, but the suite `v0.5-s` names
    // for the two fixtures it **rewrote**: their beds had been insulated from the world by the
    // very re-application the audit was about. A rewrite is a claim about a suite exactly as a
    // new suite is, so the obligation to keep saying something true about its size lives here.
    // 🔴 **P-1c / `req/551`** — the detach face, registered in the commit that writes it. The
    // newest block makes a numeric claim about it, and the claim it keeps honest is the one this
    // registry exists for: what a reverse operation does **not** put back.
    ("crates/gx-cli/tests/p1c_detach.rs", "eighteen"),
    ("crates/gx-engine/tests/crash_recovery.rs", "thirteen"),
    // 🔴 **R38 / `req/38` §294-2 (b)** — the frozen 2026-08-18 specimen, registered in the commit
    // that writes it. The newest block names it as the pair that keeps a **declared limit** honest
    // rather than as evidence of a repair, and that is exactly why its size has to stay in the
    // checked set: the day the limit moves, one of its probes changes, and a number nobody holds is
    // a number nobody notices moving.
    //
    // 🔴 **R39 / `req/533` M-02(a)** — three became six in the release that found the alarm was
    // half-silent. The page and this row moved in the same commit, which is the whole convention.
    ("crates/gx-witness/tests/frozen_receipt_corpus.rs", "six"),
    // 🔴 **`req/38` §324 ruling 3** — the leaf repair the corpus above spent three windows
    // pointing at. Registered in the commit that writes it, for the reason the row above gives one
    // layer up: this suite is what keeps a **declared limit** honest, and the limit it keeps honest
    // is the one that moved. The 2026-08-18 section of `docs/LIMITS.md` used to say "there is no
    // version of `gx` that can" confirm the inclusion proof; this suite is why that sentence is now
    // kept as a record rather than as a claim, and its size belongs in the checked set so that the
    // day somebody deletes a probe, the page and the number move together.
    ("crates/gx-witness/tests/leaf_from_signed_bytes.rs", "five"),
    // 🔴 **R39 / `req/533` M-02(b)** — the CLI half. `docs/LIMITS.md` said `gx receipt verify`
    // answers exit 7 for the frozen document and nothing ran the binary against it, because the
    // corpus suite lives in a crate that has no binary to run. This is the suite that does.
    ("crates/gx-cli/tests/r39_frozen_receipt_verdict.rs", "five"),
    // 🔴 **DR-46-24(A)** — registered in the commit that writes it, which is the convention this
    // file exists to make mechanical rather than remembered. The newest block cites it for the
    // measurement that decided where an inclusion path lives.
    ("crates/gx-witness/tests/d24_read_set_cost.rs", "five"),
    // 🔴 **R29** — this release's three suites, registered in the commit that writes them, which is
    // the convention this file exists to make mechanical rather than remembered.
    ("crates/gx-cli/tests/r29_rollback_is_verified.rs", "six"),
    // 🔴 **R31 / `req/378` M-01** — not a suite this release wrote, but the suite whose
    // numbers `v0.5-r` **withdraws**. A withdrawal is a numeric claim like any other, and the
    // registry is where the obligation to keep saying something true about it lives: the file
    // still exists and still holds the reconstruction, so it stays in the checked set.
    ("crates/gx-adapter-fs/tests/r30_rollback_window.rs", "one"),
    ("crates/gx-cli/tests/r29_instrument_repairs.rs", "five"),
    ("crates/gx-api/tests/r29_rollback_read_faces.rs", "four"),
    // 🔴 **R29** — not one of this release's own suites, but a suite this release **grew**, which
    // the convention treats identically: `limits_sync.rs` pinned it at four and `v0.5-m` states
    // four, so the page owes a current statement and the registry is where that obligation lives.
    ("crates/gx-cli/tests/r26_refusal_remedy_parity.rs", "five"),
    // 🔴 **R-1001-1 (`req/1001` §4, 2026-08-31)** — a suite that ruling **grew**, which the
    // convention treats identically (the row above is the precedent): `limits_sync.rs` pins it at
    // ten and the R-1001-1 correction block states ten, so the page owes a current statement and
    // the registry is where that obligation lives.
    ("crates/gx-cli/tests/r26_not_attempted_causes.rs", "ten"),
    // The three the page states and the tree contradicts, with nothing later correcting them.
    ("crates/gx-cli/tests/serve_runtime_r3.rs", "eight"),
    ("crates/gx-cli/tests/serve_runtime_r6.rs", "sixteen"),
    ("crates/gx-cli/tests/r25_abort_and_record_only.rs", "nine"),
    // Not stale — the page corrects it twice already. Registered because the newest block names
    // it as the example of the convention working, and a named number is a held number.
    ("probes/doubt/tests/declaration_writer_doubt.rs", "seven"),
    // This release's own suites, registered as they are written rather than after they rot.
    ("crates/gx-cli/tests/r27_reentrant_abort.rs", "six"),
    ("crates/gx-adapter-mcp/tests/r27_edge_class_width.rs", "six"),
    (
        "crates/gx-adapter-mcp/tests/r27_census_derivation.rs",
        "six",
    ),
    ("crates/gx-cli/tests/r27_parity_allowlist.rs", "six"),
    // 🔴 **R28 / `req/334` L-03** — `four`, not `five`. The `five` was this file's own counter
    // counting the needle it quotes; see `support/probe_counts.rs` for the whole account.
    ("crates/gx-cli/tests/r27_limits_probe_counts.rs", "four"),
    // R28's own suites, registered as they are written rather than after they rot — the convention
    // this file exists to make mechanical, applied to the release that repaired it.
    ("crates/gx-cli/tests/r28_abort_answer_sweep.rs", "four"),
    (
        "crates/gx-cli/tests/r28_probe_counter_discrimination.rs",
        "three",
    ),
    ("crates/gx-cli/tests/r28_remedy_marker.rs", "three"),
    ("crates/gx-api/tests/r28_rollback_members.rs", "three"),
    (
        "crates/gx-adapter-mcp/tests/r28_completion_facts.rs",
        "four",
    ),
    (
        "crates/gx-adapter-mcp/tests/r28_cell_count_claims.rs",
        "three",
    ),
    // 🔴 **DR-46-28** (`req/38` §255 ruling 4, `req/459`) — the KA battery for the boundary
    // attest. Registered because `docs/LIMITS.md`'s newest block names it, and it is named there
    // for the reason `req/329` L-01 gives: the page tells a buyer the field is not read back into
    // any decision, and the suite is where that is measured rather than promised.
    //
    // 🔴 **req/999 F-3 (2026-08-31)** — "eleven" became "ten": the tree lost
    // `the_boundary_does_not_reach_the_gate` in a merge resolution (mainline already counted ten
    // at `72308d32` while the `r973_undo_attest` lane branch still carried eleven at `ffa05d0b`;
    // no non-merge commit removes it, measured with `git log -S`). `req/999`'s census filed the
    // divergence — its attribution to `b5e3f5f7` was wrong, `b5e3f5f7^` already counted ten —
    // and this row syncs the registry to the tree the way the convention demands: the word and
    // the page's current statement move in the same commit. The old word was "eleven".
    ("crates/gx-witness/tests/boundary_attest.rs", "ten"),
    // 🔴 **P-1b / `req/544` AC-13** — the attach-face specimens of 2026-08-22, registered in the
    // commit that freezes them. The page names this suite for the same reason it names the corpus
    // one section up — it is the pair that keeps a declared limit honest — and the limit it holds
    // is a **weakness** of the specimen rather than a repair, which is exactly the kind of number
    // that rots unnoticed if nobody holds it.
    ("crates/gx-cli/tests/p1b_attach_face_frozen.rs", "three"),
    // 🔴 **P-1b / `req/544` AC-12** — the walk that gives every receipt document in the tree a
    // coverage table. The page names it for the fourth question's `unknown`, which is a limit
    // that will look like a defect to a reader who does not know it is measured on both kinds.
    ("crates/gx-witness/tests/p1b_coverage_totality.rs", "four"),
    // 🔴 **`req/859` G8 / `req/868`** (2026-08-26, seat=Opus, provisional — open to re-adjudication) — the atomicity of
    // `ObservationStore::put`. **Two**, and the pair is the point: the write half (no `.obs` file
    // is ever published holding other than the whole body, measured against a concurrent reader)
    // and `req/871` F4's verification half (a truncated body already at an address is republished
    // rather than trusted). Landing only the first was a real defect — it protected trees that had
    // never yet been hurt. Registered because the page states the numbers, and 485-before/0-after
    // is exactly the kind of number that rots into folklore if nobody holds it.
    ("crates/gx-engine/tests/g8_observation_atomicity.rs", "two"),
    // 🔴 **`req/859` G9 / `req/868`** — the platform boundary of name durability. **Three**: the
    // declaration answers to the same `cfg` as the implementation, the unheld arm says so in
    // words, and the marker recording that this is a declaration and not yet a warning.
    ("crates/gx-engine/tests/g9_name_durability.rs", "three"),
    // 🔴 **`req/868` R-868-5 / `req/919` W4** (2026-08-29) — the adapter's own directory-fsync
    // sibling of G9, registered because this suite's own ordering probe is what the F6/R-868-5
    // section of `docs/LIMITS.md` names, not a new count of its own: `apply_durability.rs` already
    // existed and this lane only changed the call the ordering probe looks for.
    ("crates/gx-adapter-fs/tests/apply_durability.rs", "nine"),
    // 🔴 **H-9 / `req/954` §3-1 / `req/983`** (2026-08-31) — the C-25 vocabulary ruling and the
    // spec corrections that follow from it. **Eight.** Registered in the commit that writes the
    // suite, which is the convention this file exists to make mechanical: the newest block of
    // `docs/LIMITS.md` names this suite as the half of the H-9 repair that *is* measured, beside a
    // paragraph that says the other half is not repaired at all. A number standing next to an
    // admission of a gap is exactly the number that must not be allowed to rot.
    ("crates/gx-witness/tests/r964_c25_canon.rs", "eight"),
    // 🔴 **req/999 F-3 / R-986-1 (2026-08-31)** — the two `probes/doubt` read-surface suites the
    // newest block names by path (the R-986-1 merge, `req/999_R986_1_C10C11C12_MERGE_2026-08-31.md`,
    // landed the naming without these rows; arm d caught it at the next full run, which is its
    // job). Private-tree-only rows, see `PRIVATE_TREE_ONLY`.
    ("probes/doubt/tests/inference_closed_doubt.rs", "fifteen"),
    ("probes/doubt/tests/read_surface_census_doubt.rs", "five"),
];

/// 🔴 **Bed control** — the registry describes this tree, measured **two ways that fail
/// differently**.
///
/// Without this the arm below would hold the page to numbers that are themselves wrong, which is
/// the failure one level up from the one this file exists for. R27 had this arm and it was green
/// while the registry was wrong, because it re-ran the one computation the registry was built on
/// (`req/334` L-03). So the control now requires [`probe_count`] (attributes) and [`probe_names`]
/// (the items those attributes introduce) to agree, and records the **old** substring number beside
/// them so that the divergence this repair is about stays visible rather than being tidied away.
#[test]
fn a_bed_control_every_registered_count_is_this_trees_count() {
    let mut wrong: Vec<String> = Vec::new();
    let mut divergent: Vec<String> = Vec::new();
    for (path, word) in REGISTERED {
        if held_against_the_private_tree(path) {
            eprintln!("SKIP {path}: {SET_ASIDE}");
            continue;
        }
        let full = repo_root().join(path);
        let Ok(text) = std::fs::read_to_string(&full) else {
            wrong.push(format!("{path} is not in this tree"));
            continue;
        };
        let attributes = probe_count(&text);
        let names = probe_names(&text);
        let substring = substring_count(&text);
        if substring != attributes {
            divergent.push(format!(
                "{path}: substring {substring} vs attributes {attributes}"
            ));
        }
        if names.len() != attributes {
            wrong.push(format!(
                "{path}: {attributes} attributes introduce {} items — the two methods disagree, so \
                 neither is trusted",
                names.len()
            ));
            continue;
        }
        if attributes != value_of(word) {
            wrong.push(format!(
                "{path}: registry says {word} ({}), tree has {attributes}",
                value_of(word)
            ));
        }
    }
    record(&format!(
        "R28_LIMITS_REGISTRY entries={} wrong={wrong:?} \
         substring_vs_attribute_divergence={divergent:?}",
        REGISTERED.len()
    ));
    assert!(
        wrong.is_empty(),
        "🔴 the registry is the thing holding the page, so it is held against the tree first: \
         {wrong:?}"
    );
}

// 🔴 **R28 / `req/334` L-03** — the discrimination control for these two counters lives in
// `r28_probe_counter_discrimination.rs`, deliberately **not** here: an arm added to this file would
// change this file's own probe count, which is the very number the repair above corrects.

/// 🔴 **`req/329` L-01** — the page's most recent statement about each registered suite is true.
///
/// The needle is a fixed shape rather than a guess at prose, so this gate cannot be satisfied by
/// accident and cannot fire on a sentence that merely mentions a number near a path.
///
/// A row this tree does not carry is set aside exactly as the bed control sets it aside. The two
/// probes read the same registry, and one of them being unable to say "this tree does not carry
/// that suite" was the asymmetry: it held the page to statements about files a reader of this tree
/// cannot open. A suite this tree does carry is still held, which is why `boundary_attest.rs` is
/// still checked here.
#[test]
fn b_the_page_carries_a_current_statement_for_every_registered_suite() {
    let page = limits();
    let mut missing: Vec<String> = Vec::new();
    for (path, word) in REGISTERED {
        if held_against_the_private_tree(path) {
            eprintln!("SKIP {path}: {SET_ASIDE}");
            continue;
        }
        let needle = format!("`{path}` has **{word}**");
        if !page.contains(&needle) {
            missing.push(needle);
        }
    }
    record(&format!("R27_LIMITS_CURRENT missing={missing:?}"));
    assert!(
        missing.is_empty(),
        "🔴 `req/329` L-01: `docs/LIMITS.md` makes a numeric claim about these suites and carries \
         no current statement for them. An additive page may leave an old block stale — that is the \
         record — but the reader's most recent answer has to be true of the tree: {missing:?}"
    );
}

/// 🔴 The convention itself is written where the next lane will move the anchor.
///
/// `limits_sync.rs` is the file whose comment block records every anchor move and why. The rule
/// that a move is not free — the outgoing block's numbers are carried or corrected — belongs beside
/// those entries, or the next lane repeats `req/329` L-01 with a different number.
#[test]
fn c_the_anchor_move_convention_is_recorded_beside_the_anchor() {
    let sync = std::fs::read_to_string(repo_root().join("crates/gx-cli/tests/limits_sync.rs"))
        .expect("limits_sync.rs is readable");
    for (what, needle) in [
        (
            "the rule an anchor move now carries",
            "carried into `r27_limits_probe_counts.rs`'s registry or corrected on the page",
        ),
        ("the finding it comes from", "`req/329` L-01"),
    ] {
        assert!(
            sync.contains(needle),
            "🔴 the convention has to be recorded where the next anchor move is decided — {what} \
             (`{needle}`)"
        );
    }
}

/// 🔴 Every suite this release's own block names by path is registered.
///
/// Scoped to the newest block, where the prose is this lane's own, so the scan is precise rather
/// than clever: a claim added to the block a reader sees first cannot escape the registry.
#[test]
fn d_every_suite_named_in_the_newest_block_is_registered() {
    let page = limits();
    let newest = page
        .rsplit_once("v0.5-n (")
        .map(|(_, rest)| rest.to_string())
        .expect("docs/LIMITS.md carries the newest stacked block, v0.5-n (`req/329`)");
    let mut unregistered: Vec<String> = Vec::new();
    let mut seen = 0usize;
    for piece in newest.split('`') {
        // Suites only: the registry is about probe counts, and this block names `src/` files for
        // other reasons. The window runs to EOF (`limits_sync.rs`'s anchor shape), so the static
        // sections after the block are inside it.
        if !piece.ends_with(".rs") || !piece.contains("/tests/") {
            continue;
        }
        seen += 1;
        if !REGISTERED.iter().any(|(path, _)| *path == piece) {
            unregistered.push(piece.to_string());
        }
    }
    unregistered.sort();
    unregistered.dedup();
    record(&format!(
        "R27_LIMITS_NEWEST paths={seen} unregistered={unregistered:?}"
    ));
    assert!(
        unregistered.is_empty(),
        "🔴 the newest block names these suites and the registry does not hold them, so their \
         numbers leave the checked set the moment the next anchor moves — which is exactly \
         `req/329` L-01: {unregistered:?}"
    );
}
