// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// what : AC-P5-5 -- `gx limits` and `docs/LIMITS.md` never say two different things.
// why  : `req/134` §1 item 7 / §2 AC-P5-5: "gx limits has the same content as LIMITS.md (machine-checked sync), an 8-line structure" (sem: SEM-gx-cli-1302).
//        21 §10-4's own header rules that this page creates no new limit; the check that matters
//        is that the **terminal** and the **doc** are the same claim, not that either is correct
//        in isolation (45 remains the one canon for that -- `limits.rs`'s own module header).
// how  : `docs/LIMITS.md` is written **verbatim** from `crates/gx-cli/src/limits.rs`'s `LIMITS`
//        constant (its own header says so), so the check is a plain substring match after
//        whitespace is collapsed -- a markdown reflow must not fail this probe, a changed word
//        must.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-cli sits at <root>/crates/gx-cli")
        .to_path_buf()
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn gx_limits_json_is_eight_entries_each_present_verbatim_in_docs_limits_md() {
    let output = Command::new(env!("CARGO_BIN_EXE_gx"))
        .args(["limits", "--json"])
        .output()
        .expect("gx limits --json runs");
    assert!(
        output.status.success(),
        "gx limits --json exited {:?}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("gx limits --json prints valid JSON");
    assert_eq!(
        json["count"].as_u64(),
        Some(8),
        "AC-P5-5: an 8-line structure -- `count` drifted from 8 (sem: SEM-gx-cli-1303)"
    );
    let limits = json["limits"].as_array().expect("`limits` is an array");
    assert_eq!(
        limits.len(),
        8,
        "AC-P5-5: an 8-line structure -- array length drifted from 8 (sem: SEM-gx-cli-1304)"
    );

    let doc_path = repo_root().join("docs/LIMITS.md");
    let doc = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("{} readable: {e}", doc_path.display()));
    let normalized_doc = collapse_whitespace(&doc);

    for entry in limits {
        let statement = entry["statement"]
            .as_str()
            .expect("each limit carries a `statement`");
        let citation = entry["citation"]
            .as_str()
            .expect("each limit carries a `citation`");
        let normalized_statement = collapse_whitespace(statement);
        assert!(
            normalized_doc.contains(&normalized_statement),
            "docs/LIMITS.md is missing (or has drifted from) this `gx limits` statement:\n{statement}"
        );
        assert!(
            doc.contains(citation),
            "docs/LIMITS.md is missing the citation for this statement:\n{citation}\n({statement})"
        );
    }
}

#[test]
fn gx_limits_plain_mode_numbers_all_eight_and_exits_ok() {
    let output = Command::new(env!("CARGO_BIN_EXE_gx"))
        .arg("limits")
        .output()
        .expect("gx limits runs");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for n in 1..=8 {
        assert!(
            text.contains(&format!("{n}. ")),
            "plain-mode `gx limits` output is missing entry {n}"
        );
    }
}

/// 🔴 **R12 / `req/242` L-09** — the newest buyer-facing block names the probes that make it
/// true, and the numbers in it move when they move.
///
/// # What the audit measured
///
/// `crates/gx-cli/tests/limits_sync.rs` compares `docs/LIMITS.md` with the **eight fixed lines** of
/// `crates/gx-cli/src/limits.rs`. Everything else in that file — the stacked `v0.4-*` blocks each
/// adversarial audit adds, which is where the sentences a buyer actually acts on live — is checked
/// by nobody. `req/242` H-01 measured why that matters: v0.4-w's "`gx repair --yes` is the one
/// command that writes them back, and it tells you it did, by name" was **false when it was
/// written**, and it stayed on the page for a release.
///
/// # What this can and cannot do
///
/// It cannot check prose against behaviour; nothing can. What it can do is stop the newest block
/// from going stale **silently**: the release's own probe counts appear in it, so a lane that adds
/// a gate and forgets the page is red, and a lane that edits the page down is red too. That is the
/// same shape `floor_doubt.rs::f6` uses for `README.md`'s floor sentence — bind the prose to a
/// number something else already enforces.
#[test]
fn the_newest_limits_block_names_the_gates_of_the_release_that_wrote_it() {
    let doc = std::fs::read_to_string(repo_root().join("docs/LIMITS.md")).expect("docs/LIMITS.md");
    // 🔴 **R14** — the anchor moves with the release, which is the point of the probe: a lane that
    // writes a block and forgets this line is red, and so is one that writes this line and no block.
    //
    // 🔴 **A3 / `req/38` §196** — the anchor moves again, to `v0.5-d`, and the suite it reports on
    // moves with it. This release's gates are not in `model_a_probes.rs`: read-by-tool escrow is an
    // adapter road, so its probes live beside the adapter. The Model A count below is asserted
    // **unchanged**, which is the other half of the same discipline — a lane that quietly grew the
    // Model A suite while writing about a different one would be red here.
    //
    // 🔴 **R17 / `req/38` §199** — the anchor moves to `v0.5-e` and gains a second suite, because
    // the repair the eighteenth audit forced is an adapter road too. `github16_read_by_tool.rs` is
    // asserted **unchanged at fourteen** for the same reason the Model A number is here: a lane
    // that grew a suite it was not writing about would be red.
    //
    // 🔴 **`req/288` item 6 (`req/285` §3's declaration, answered)** — the anchor moves to
    // `v0.5-f`. R18 wrote that block and could not wire it here, because `crates/gx-cli` was
    // outside its write scope; it said so instead of leaving the gap silent, and this is the lane
    // that closes it. Left at `v0.5-e` the probe was measuring a block that was no longer the
    // newest one, which is the precise failure mode `req/242` L-09 exists to prevent — a
    // buyer-facing paragraph nothing checks.
    // 🔴 **`req/291` (R20)** — the anchor moves to `v0.5-g`. Left at `v0.5-f` this probe would be
    // measuring a block that is no longer the newest one, which is the failure `req/242` L-09
    // exists to prevent; R18 could not move it (`crates/gx-cli` was outside its scope) and R20 can.
    // 🔴 **`req/303` (R22)** — the anchor moves to `v0.5-i`. DR-46-16 wrote `v0.5-h` and left the
    // anchor where R20 put it, so for one window "the newest block" meant *everything from v0.5-g
    // onwards* and the needles below could be satisfied by a block two releases old. Left at
    // `v0.5-g` after this release it would be worse: the block a buyer reads first would be the one
    // this probe is least holding.
    // 🔴 **`req/312` (R23)** — the anchor moves to `v0.5-j`. Left at `v0.5-i` the needles
    // below would be satisfiable by the block a release older, which is what "the newest block" has
    // to mean for this gate to be a gate at all.
    // 🔴 **`req/316` (R24)** — the anchor moves to `v0.5-k`. Left at `v0.5-j` the needles below
    // would be satisfiable by the block a release older, which is what "the newest block" has to
    // mean for this gate to be a gate at all.
    // 🔴 **`req/320` (R25)** — the anchor moves to `v0.5-l`. Same argument, and this window has a
    // second reason: `v0.5-l` **corrects two claims `v0.5-k` made about its own gates**, so a gate
    // still reading the older block would be holding the corrected sentences up as current.
    // 🔴 **`req/324` (R26)** — the anchor moves to `v0.5-m`, and this window has that second reason
    // twice over: `v0.5-m` corrects **three** claims `v0.5-l` made about its own gates — the width
    // of the invisible-edge axis ("exactly these five"), what the reach census counts ("every read
    // of the private map itself"), and the width of the accepted residual (one spelling). A gate
    // still anchored on `v0.5-l` would be holding all three corrected sentences up as current,
    // which is worse than holding nothing.
    // 🔴 **`req/329` L-01 (`req/38` §233 ruling 5) — the rule an anchor move now carries.** Moving
    // this anchor takes every number the outgoing block stated out of the checked set in the same
    // commit, and R26 did exactly that to a count its own release had falsified (the page said
    // eight, the file had nine). So a move is a decision, not a side effect: for every numeric claim
    // in the outgoing block, the lane moving the anchor either has it
    // **carried into `r27_limits_probe_counts.rs`'s registry or corrected on the page** in the
    // incoming block.
    // That registry outlives anchor moves by construction, and it is red until the page carries a
    // current statement for every suite in it.
    //
    // 🔴 **`req/331` (R27) — the anchor deliberately does not move, and that is the decision the
    // rule above asks for rather than an omission.** `v0.5-n` is stacked *after* `v0.5-m`, so the
    // window from `v0.5-m (` to the end of the file contains both and every needle below is still
    // held. Moving the anchor to `v0.5-n` would drop `v0.5-m`'s fifteen needles in this commit —
    // the exact shape that rotted `v0.5-l`'s probe count — in exchange for nothing, because
    // `v0.5-n` has a gate of its own: `r27_limits_probe_counts.rs` anchors on `v0.5-n (` and holds
    // every suite that block names. Two blocks held beats one block held and one dropped.
    let newest = doc
        .rsplit_once("v0.5-m (")
        .map(|(_, rest)| rest.to_string())
        .expect("docs/LIMITS.md carries the newest stacked block, v0.5-m (`req/324`)");
    println!("LIMITS_NEWEST_BLOCK_BYTES={}", newest.len());

    // The Model A suite, which this release did not touch: its number is here so that "unchanged"
    // is measured rather than assumed.
    let probes = std::fs::read_to_string(repo_root().join("crates/gx-cli/tests/model_a_probes.rs"))
        .expect("model_a_probes.rs");
    let count = probes.matches("#[test]").count();
    println!("MODEL_A_PROBE_COUNT={count}");
    assert_eq!(
        count, 41,
        "the Model A suite is the surface the buyer-facing block reports on; when it grows, the          block and this number move together (`req/38` §178 ruling 2 (ii))"
    );

    // The suite this release's gates do live in, counted from the source the same way.
    let escrow = std::fs::read_to_string(
        repo_root().join("crates/gx-adapter-mcp/tests/github16_read_by_tool.rs"),
    )
    .expect("github16_read_by_tool.rs");
    let escrow_count = escrow.matches("#[test]").count();
    println!("GITHUB16_PROBE_COUNT={escrow_count}");
    assert_eq!(
        escrow_count, 14,
        "the read-by-tool suite is what the v0.5-d block reports on, and R17 did not grow it;          when it moves, that block and this number move together"
    );

    // The suite R17 added, counted from the source the same way.
    let bound = std::fs::read_to_string(
        repo_root().join("crates/gx-adapter-mcp/tests/r17_attested_object_binding.rs"),
    )
    .expect("r17_attested_object_binding.rs");
    let bound_count = bound.matches("#[test]").count();
    println!("R17_PROBE_COUNT={bound_count}");
    assert_eq!(
        bound_count, 10,
        "the attested-object suite is what the v0.5-e block reports on, and R18 did not grow it;          when it moves, that block and this number move together"
    );

    // The suite R18 added, which the **previous** block reports on: asserted unchanged, for the
    // same reason the Model A number is here.
    let declared = std::fs::read_to_string(
        repo_root().join("crates/gx-adapter-mcp/tests/r18_declaration_soundness.rs"),
    )
    .expect("r18_declaration_soundness.rs");
    let declared_count = declared.matches("#[test]").count();
    println!("R18_PROBE_COUNT={declared_count}");
    assert_eq!(
        declared_count, 8,
        "the declaration-soundness suite is what the v0.5-f block reports on, and R20 did not grow          it; when it moves, that block and this number move together"
    );

    // 🔴 The four suites R20 added — the ones the **newest** block reports on, counted the same way.
    let mut r20 = 0usize;
    for (crate_dir, file, expected) in [
        ("gx-adapter-mcp", "r20_template_prior_soundness.rs", 10usize),
        ("gx-cli", "r20_undo_that_does_not_restore.rs", 4),
        ("gx-cli", "r20_refusal_vocabulary_is_whole.rs", 5),
        ("gx-cli", "r20_mcp_surface_sentences.rs", 4),
    ] {
        let path = repo_root().join(format!("crates/{crate_dir}/tests/{file}"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        let count = source.matches("#[test]").count();
        println!("R20_PROBE_COUNT {file}={count}");
        assert_eq!(
            count, expected,
            "the newest block reports {expected} tests in {file}; when the suite grows, the block              and this number move together (`req/38` §178 ruling 2 (ii))"
        );
        r20 += count;
    }
    println!("R20_PROBE_TOTAL={r20}");
    assert_eq!(
        r20, 23,
        "the v0.5-g block's total; a suite that moved without the prose moving is what this holds"
    );

    // 🔴 **`req/303` (R22)** — the four suites this release added, which the **newest** block now
    // reports on. The R20 numbers above are asserted **unchanged** for the reason the Model A
    // number is here: a lane that grew a suite it was not writing about is red.
    let mut r22 = 0usize;
    for (crate_dir, file, expected) in [
        ("gx-adapter-mcp", "r22_declaration_gates.rs", 15usize),
        ("gx-cli", "r22_wrap_road.rs", 8),
        ("gx-cli", "r22_serverless_surface.rs", 3),
        ("gx-cli", "r22_refusal_constant_census.rs", 4),
    ] {
        let path = repo_root().join(format!("crates/{crate_dir}/tests/{file}"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        let count = source.matches("#[test]").count();
        println!("R22_PROBE_COUNT {file}={count}");
        assert_eq!(
            count, expected,
            "the newest block reports {expected} tests in {file}; when the suite grows, the block \
             and this number move together (`req/38` §178 ruling 2 (ii))"
        );
        r22 += count;
    }
    println!("R22_PROBE_TOTAL={r22}");
    assert_eq!(
        r22, 30,
        "the v0.5-i block's total across the four suites it names"
    );

    // 🔴 **`req/312` (R23)** — the three suites this release added, which the **newest**
    // block reports on. The R20 and R22 numbers above are asserted **unchanged** for the reason the
    // Model A number is: a lane that grew a suite it was not writing about is red here.
    let mut r23 = 0usize;
    for (crate_dir, file, expected) in [
        ("gx-adapter-mcp", "r23_cas_declaration_gates.rs", 14usize),
        ("gx-cli", "r23_wrap_road.rs", 5),
        ("gx-cli", "r23_not_found_road.rs", 6),
    ] {
        let path = repo_root().join(format!("crates/{crate_dir}/tests/{file}"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        let count = source.matches("#[test]").count();
        println!("R23_PROBE_COUNT {file}={count}");
        assert_eq!(
            count, expected,
            "the newest block reports {expected} tests in {file}; when the suite grows, the block              and this number move together (`req/38` §178 ruling 2 (ii))"
        );
        r23 += count;
    }
    println!("R23_PROBE_TOTAL={r23}");
    assert_eq!(
        r23, 25,
        "the previous block's total across the three suites it names"
    );

    // 🔴 **`req/316` (R24)** — the three suites this release added, which the **newest** block
    // reports on. Every number above is asserted **unchanged** for the reason the Model A number
    // is: a lane that grew a suite it was not writing about is red here.
    let mut r24 = 0usize;
    for (crate_dir, file, expected) in [
        ("gx-adapter-mcp", "r24_predicate_unification.rs", 12usize),
        ("gx-adapter-mcp", "r24_absence_discrimination.rs", 8),
        ("gx-cli", "r24_record_only_and_removal.rs", 7),
    ] {
        let path = repo_root().join(format!("crates/{crate_dir}/tests/{file}"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        let count = source.matches("#[test]").count();
        println!("R24_PROBE_COUNT {file}={count}");
        assert_eq!(
            count, expected,
            "the newest block reports {expected} tests in {file}; when the suite grows, the block \
             and this number move together (`req/38` §178 ruling 2 (ii))"
        );
        r24 += count;
    }
    println!("R24_PROBE_TOTAL={r24}");
    assert_eq!(
        r24, 27,
        "the previous block's total across the three suites it names"
    );

    // 🔴 **`req/320` (R25)** — the two suites that release added, which the **previous** block
    // reports on.
    //
    // 🔴 `r25_abort_and_record_only.rs` moves from eight to **nine**, and the reason is on the page:
    // `req/324` H-01 measured the accepted residual at a second spelling, so the standing
    // measurement gets an arm per spelling rather than one arm the page then generalises from —
    // which is `req/38` §230 ruling 1's own lesson applied to the probe instead of only to the
    // prose. `r25_declaration_axes.rs` is asserted **unchanged at seven**: R26 widened two
    // predicates inside it and added no probe.
    let mut r25 = 0usize;
    for (crate_dir, file, expected) in [
        ("gx-adapter-mcp", "r25_declaration_axes.rs", 7usize),
        ("gx-cli", "r25_abort_and_record_only.rs", 9),
    ] {
        let path = repo_root().join(format!("crates/{crate_dir}/tests/{file}"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        let count = source.matches("#[test]").count();
        println!("R25_PROBE_COUNT {file}={count}");
        assert_eq!(
            count, expected,
            "the newest block reports {expected} tests in {file}; when the suite grows, the block              and this number move together (`req/38` §178 ruling 2 (ii))"
        );
        r25 += count;
    }
    println!("R25_PROBE_TOTAL={r25}");
    assert_eq!(
        r25, 16,
        "the previous block's total across the two suites it names"
    );

    // 🔴 **`req/324` (R26)** — the six suites this release added, which the **newest** block reports
    // on. Every number above is asserted **unchanged** for the reason the Model A number is: a lane
    // that grew a suite it was not writing about is red here.
    let mut r26 = 0usize;
    for (crate_dir, file, expected) in [
        ("gx-adapter-mcp", "r26_invisible_edge_axis.rs", 8usize),
        ("gx-adapter-mcp", "r26_preimage_funnel.rs", 6),
        ("gx-adapter-mcp", "r26_reach_census.rs", 4),
        ("gx-adapter-mcp", "r26_limits_family_sync.rs", 3),
        // 🔴 **R-1001-1 (`req/1001` §4, D-999-F2's else-arm, 2026-08-31)** — nine until that
        // ruling. It added the seventh `NotAttemptedBecause` cause and the behavioural probe for
        // its sentence, and the rule this list exists for is that the block and the number move
        // together: the 2026-08-31 R-1001-1 correction in `docs/LIMITS.md` carries it.
        ("gx-cli", "r26_not_attempted_causes.rs", 10),
        // 🔴 **R29 / `req/361` L-02** — four until this window. R29 added the behavioural arm that
        // drives the repaired const walk (a spliced non-`";`-terminated declaration is refused
        // rather than swallowed), and the rule this list exists for is that the block and the
        // number move together: `docs/LIMITS.md` v0.5-p carries the correction.
        ("gx-cli", "r26_refusal_remedy_parity.rs", 5),
    ] {
        let path = repo_root().join(format!("crates/{crate_dir}/tests/{file}"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        let count = source.matches("#[test]").count();
        println!("R26_PROBE_COUNT {file}={count}");
        assert_eq!(
            count, expected,
            "the newest block reports {expected} tests in {file}; when the suite grows, the block \
             and this number move together (`req/38` §178 ruling 2 (ii))"
        );
        r26 += count;
    }
    println!("R26_PROBE_TOTAL={r26}");
    assert_eq!(
        r26, 36,
        "the newest block's total across the six suites it names (🔴 R29: 34 until this window; \
         `r26_refusal_remedy_parity.rs` gained the arm that drives the repaired const walk, and \
         `docs/LIMITS.md` v0.5-p carries the correction the page owes for it. 🔴 R-1001-1, \
         2026-08-31: 35 until that ruling; `r26_not_attempted_causes.rs` gained the seventh \
         cause's behavioural probe, and the R-1001-1 correction on the page carries it)"
    );

    for (what, needle) in [
        ("the suite this release added", "r26_invisible_edge_axis.rs"),
        ("its probe count, in words", "**eight** tests, beside"),
        (
            "the suite beside it, by name",
            "r26_preimage_funnel.rs`'s six",
        ),
        (
            "the width of the accepted residual, in spellings",
            "it is two spellings wide, not one",
        ),
        (
            "the word the page had admitted and never named",
            "`const_json`",
        ),
        (
            "the member of the same family measured **not** to destroy",
            "its printed undo refused with `rc=1`",
        ),
        (
            "the class the invisible-edge axis became",
            "Default_Ignorable_Code_Point",
        ),
        (
            "the negative control that keeps the class from eating real names",
            "and all thirty-five parse",
        ),
        (
            "the two key slots the unnamed sweep had not reached",
            "Eighty-five cells of eighty-five are now the unnamed fault",
        ),
        (
            "the correction to what the previous block claimed about its own census",
            "The field is private; the question",
        ),
        (
            "what gx's own words did to gx's own token",
            "pushed gx's own token to offset 509",
        ),
        (
            "the cause travelling beside the value it explains",
            "The cause now travels with the value, the proxy has one arm",
        ),
        (
            "the roads this release repaired and did not drive",
            "the twenty-sixth audit is charged with driving",
        ),
        (
            "the axis of the byte comparison that is **not** closed",
            "Canonical equivalence",
        ),
        (
            "the substrates this release still measured zero times",
            "Windows, OneDrive and network shares\n   > are still measured **zero** times",
        ),
        // 🔴 **`req/331` (R27)** — the claims `v0.5-n` adds, held here because `v0.5-n` is inside
        // this window (see the anchor note above).
        (
            "the two categories the invisible-edge class grew into",
            "General_Category=Cc",
        ),
        (
            "the private-use half stated as what it is rather than as invisibility",
            "renders as whatever a private agreement says",
        ),
        (
            "that the accepted residual does not vary with the value's type",
            "the value's type is not one of its dimensions",
        ),
        (
            "the family member measured **not** to destroy",
            "It joins `do_result` on the short list of members **measured** not to destroy",
        ),
        (
            "the rule that keeps a numeric claim checked when the anchor moves",
            "an anchor move is a decision",
        ),
    ] {
        assert!(
            newest.contains(needle),
            "🔴 `req/242` L-09: the newest block does not name {what} (`{needle}`). A buyer-facing              paragraph with no machine gate is what let v0.4-w publish a sentence that was false              when it was written"
        );
    }
}

/// 🔴 **`req/291` M-06 and M-05** (`req/298` §1 items 6 and 7) — the two repairs this lane made to
/// **prose**, given a gate.
///
/// # Why a separate probe
///
/// Both are sentences, so neither has a binary to be red on: the pre-repair `gx` and the repaired
/// one print the same bytes. The gate is this file, and the red is measured with `docs/LIMITS.md`
/// put back to `20f0635` while the source stays — `_r20_scripts/40_red.sh` phase B. Without the
/// needles below, that phase would only prove the stacked block moved, which is a weaker claim
/// than either repair: the audit's finding was not *there is no v0.5-g block*, it was
/// **M-06** the rate is `measured` and its **unit** is unwritten, and **M-05** the product grew a
/// fourth outcome and this page never said it leaves nothing behind.
///
/// # What each needle holds
///
/// * The unit is spelled in **both** units, and the second is the one a rate limit counts. A
///   repair that deleted `a third` instead of qualifying it would be a weakening, so the old
///   number is held here too — `req/298` item 6 asked for the unit, explicitly not for a softer
///   number.
/// * The fourth outcome's cost is stated as an absence a reader can check: no receipt, no journal
///   record, nothing under `.gx/`, one line on a stderr that goes with the process.
#[test]
fn the_throughput_unit_and_the_fourth_outcome_s_cost_are_both_written_down() {
    let doc = std::fs::read_to_string(repo_root().join("docs/LIMITS.md")).expect("docs/LIMITS.md");

    for (item, what, needle) in [
        // M-06 — the unit, in both readings, with the old number intact.
        ("M-06", "the tool-call unit, unweakened", "a third"),
        (
            "M-06",
            "the tool-call row of the table",
            "**extra tool calls**",
        ),
        (
            "M-06",
            "the request unit, which is what a rate limit counts",
            "**requests reaching the server**",
        ),
        (
            "M-06",
            "the forward call's request count with its breakdown",
            "**10** — 7 snapshot/precondition/postcondition, 2 escrow reads, 1 effect",
        ),
        (
            "M-06",
            "the round trip's request count",
            "**21** — 19 reads and 2 effects",
        ),
        (
            "M-06",
            "which unit a rate limit is written in",
            "counts **requests**, not tool calls",
        ),
        (
            "M-06",
            "the rate in the unit the limit uses",
            "it costs you about **a tenth**",
        ),
        // M-05 — the fourth outcome, and what it does not leave behind.
        //
        // 🔴 **`req/303` M-02 (R22)** — these three needles are kept for one reason and it is not
        // the one they were written for: the sentence they look for is **false**, and it is still
        // on the page because `req/38` §221 ruling 3 asked for a correction rather than a deletion
        // (the additive-merge discipline: a limits page that quietly loses the claim it got wrong
        // is a limits page a reader cannot audit). They now hold the **quotation** inside the
        // v0.5-i correction. The rows beneath them hold the correction itself, and the thing that
        // holds the *facts* is no longer on this page at all — it is
        // `crates/gx-cli/tests/r22_wrap_road.rs::the_fourth_outcome_leaves_what_it_leaves_on_each_road`,
        // which drives both roads and counts journal records. A needle can only ever say that a
        // page carries a sentence; it took three lanes to notice that nothing was asking whether
        // the sentence was true.
        (
            "M-05",
            "the v0.5-g claim, quoted rather than deleted",
            "**before a verdict** leaves **no artifact**",
        ),
        (
            "M-05",
            "what that claim said the absence covered",
            "no receipt, no journal record, nothing under",
        ),
        (
            "M-05",
            "the one trace it named, and that it is not durable",
            "The only trace is a line on the proxy's stderr",
        ),
        // 🔴 R22's correction: the half that survived measurement, the half that did not, and the
        // gap that is actually left.
        (
            "M-02",
            "the half of the v0.5-g claim that measurement kept",
            "**What is true on both roads: no receipt is written.**",
        ),
        (
            "M-02",
            "the half measurement refuted, named as refuted",
            "**What is false: \"no journal record, nothing under `.gx/`\".**",
        ),
        (
            "M-02",
            "the two roads, told apart",
            "\"gx/verdict\": \"refused-before-verdict\"",
        ),
        (
            "M-02",
            "the records the second road leaves",
            "`DraftCreated`, `Planned`, `VerifyStarted`",
        ),
        (
            "M-02",
            "what the remaining gap actually is",
            "**a refusal before a verdict has no",
        ),
        (
            "L-03",
            "the eighth field on the session line",
            "`refused_before_verdict`",
        ),
        // 🔴 `req/309` §1 item 9, first half — the catalogue file's missing ceiling.
        (
            "L-05",
            "that a catalogue file has no size ceiling",
            "**A catalogue file has no size ceiling",
        ),
        (
            "L-05",
            "the measurement behind it",
            "**1,596,001-byte file\n   > declaring 12,000 tools**",
        ),
        // 🔴 `req/309` §1 item 9, second half — the setup the round-trip number was measured under.
        (
            "L-01",
            "the setup the round-trip count was measured under",
            "How the throughput table above was measured",
        ),
        (
            "L-01",
            "the independent re-count, and why it differs",
            "**23 is the number to use**",
        ),
        // 🔴 The accepted residual `req/38` §221 ruling 2 asked to be named in those words.
        (
            "H-02",
            "the constant-member residual, named as accepted rather than pending",
            "**accepted residual**",
        ),
    ] {
        let present = doc.contains(needle);
        println!("R20_{item}_NEEDLE present={present} {needle:?}");
        assert!(
            present,
            "🔴 `req/291` {item}: `docs/LIMITS.md` does not carry {what} (`{needle}`). This is the \
             half of the repair a reader is handed, and a page that carries the repair's number \
             without its unit — or a product that grew a fourth outcome without this page saying \
             what it costs — is the failure `req/242` L-09 exists to prevent"
        );
    }
}

/// 🔴 **`req/38` §207 ruling 2 / `req/288` items 1–3** — the Lean numbers on the public faces are
/// a **recount**, not a memory.
///
/// # What went wrong
///
/// Three faces carried three different answers to one question. `crates/gx-cli/src/limits.rs`
/// item 8 said *"eight theorems and five counterexamples"*, true when it was written and never
/// moved since. `README.md` said *"92 theorems, 2 axioms"*, which came from a pattern that
/// tolerates leading whitespace and therefore counted two lines of **English prose** inside a block
/// comment in `MinimalityF0.lean` — one beginning with the word `theorem`, one with `axiom`. The
/// primary count is neither. A buyer read both faces.
///
/// # What this probe does
///
/// It re-takes the count from `lean/` itself on every run — line-start declarations, the same rule
/// `req/285` §4(c) published its commands for — and holds `limits.rs`, `docs/LIMITS.md` and
/// `README.md` to what it finds. The point is not that today's numbers are right. It is that a lane
/// which adds a theorem cannot leave them wrong: the sentence a buyer reads is now derived from the
/// tree the buyer is being told about, and drift is a red probe rather than a correction three
/// releases later.
///
/// # What it cannot do
///
/// It counts declarations; it does not check that they are *about* the F0 specification, and
/// nothing here runs the Lean kernel — `lean-current` is the gate for that, and `docs/LIMITS.md`
/// says so in the same item. Two public faces are outside this crate and outside this lane's write
/// scope, so they are **not** held here: `public/README.md` and `public/org_profile/README.md`
/// (`req/289` §4 names them and what they still say).
/// 🔴 **`req/38` ERRATA SS1001** — is `line` a **line-start** `theorem`/`lemma` declaration,
/// tolerating any number of same-line `@[...]` attribute prefixes (`@[simp] theorem foo ...`)?
///
/// The naive form this suite used before SS1001, `line.starts_with("theorem") ||
/// line.starts_with("lemma")`, is attribute-prefix blind: `lane/r10_lean_invs`'s
/// `StateMachine.lean` added three column-zero declarations — `verdictEq_refl`,
/// `ownEntriesFaithful_congr`, `rcptOk_congr` — written `@[simp] theorem ...`, and the naive
/// predicate could not see any of them (154 real declarations undercounted as 151). The repair
/// widens the predicate to strip leading `@[...]` attribute groups (nesting-tolerant, so
/// `@[simp] @[reducible] theorem` and `@[simp, reducible] theorem` both work) before testing for
/// `theorem`/`lemma` — it does **not** relax column zero: an indented line, `@[...]`-prefixed or
/// not, is still inside a proof, a comment, or a block, and stays uncounted (that distinction is
/// the whole finding this file's header names).
fn is_theorem_or_lemma_declaration(line: &str) -> bool {
    let mut rest = line;
    loop {
        if rest.starts_with("theorem") || rest.starts_with("lemma") {
            return true;
        }
        let Some(after_open) = rest.strip_prefix("@[") else {
            return false;
        };
        // Find this attribute's matching `]`, tolerating nested `[...]` inside the attribute
        // arguments (Lean attribute syntax permits it, e.g. `@[simp (config := ...)]`-style args).
        let mut depth = 1usize;
        let mut close = None;
        for (i, ch) in after_open.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        match close {
            // Unterminated `@[` on this line -- not a declaration line (e.g. an attribute that
            // is itself split across lines, which the per-line `theorem`/`lemma` check above
            // already handles correctly on the line the keyword actually starts).
            None => return false,
            Some(i) => rest = after_open[i + 1..].trim_start(),
        }
    }
}

#[cfg(test)]
mod theorem_lemma_predicate {
    use super::is_theorem_or_lemma_declaration as is_decl;

    /// 🔴 **RED-FIRST (`req/38` ERRATA SS1001)** — before the attribute-stripping loop above
    /// existed, this exact assertion failed: the naive `starts_with` form does not see
    /// `@[simp] theorem ...` as a declaration. Recorded here as the naive form, side by side with
    /// the fixed predicate below, so the flip is visible in one file rather than only in a commit
    /// message.
    #[test]
    fn red_first_naive_starts_with_predicate_misses_attribute_prefixed_declarations() {
        let line = "@[simp] theorem verdictEq_refl (a b : Verdict) : a = b := by rfl";
        let naive = line.starts_with("theorem") || line.starts_with("lemma");
        assert!(
            !naive,
            "SS1001 premise check: the naive predicate should NOT see this line as a declaration \
             -- if it now does, the naive form changed and this probe's premise is stale (delete \
             it, do not invert it)."
        );
    }

    #[test]
    fn fixed_predicate_counts_single_line_attribute_prefixed_theorem_and_lemma() {
        assert!(is_decl(
            "@[simp] theorem verdictEq_refl (a b : Verdict) : a = b := by rfl"
        ));
        assert!(is_decl("@[simp] lemma foo (x : Nat) : x = x := rfl"));
    }

    #[test]
    fn fixed_predicate_counts_stacked_and_comma_separated_attributes() {
        assert!(is_decl("@[simp] @[reducible] theorem stacked_attrs : True := trivial"));
        assert!(is_decl("@[simp, reducible] theorem comma_attrs : True := trivial"));
    }

    #[test]
    fn fixed_predicate_still_counts_plain_column_zero_declarations() {
        assert!(is_decl("theorem plain_one : True := trivial"));
        assert!(is_decl("lemma plain_two : True := trivial"));
    }

    /// 🔴 negative control — an indented `theorem`-shaped line, attribute or not, must stay
    /// uncounted. This is the exact failure mode `README.md` used to have (`req/38` §207 ruling
    /// 2): a whitespace-tolerant pattern counted English prose inside a block comment. The fix
    /// must not reopen that hole while closing the attribute-prefix one.
    #[test]
    fn negative_control_indented_prose_is_never_counted_attribute_or_not() {
        assert!(!is_decl(
            "   theorem is a word this sentence uses, not a declaration."
        ));
        assert!(!is_decl("  @[simp] theorem indented_should_not_count : True := trivial"));
        assert!(!is_decl("  lemma indented_should_not_count : True := trivial"));
    }

    /// 🔴 negative control — an attribute whose body is not followed by `theorem`/`lemma` (e.g. a
    /// `def`) must not be counted, and a malformed/unterminated attribute must not panic or match.
    #[test]
    fn negative_control_attribute_prefixed_non_theorem_lines_are_never_counted() {
        assert!(!is_decl("@[simp] def helper (x : Nat) : Nat := x"));
        assert!(!is_decl("@[instance] def inst : Foo := ..."));
        assert!(!is_decl("@[simp"));
        assert!(!is_decl("@["));
    }
}

#[test]
fn the_lean_numbers_on_every_face_are_recounted_from_lean_itself() {
    let root = repo_root();
    let lean = root.join("lean");

    // The tree, gathered the way `find lean -name '*.lean'` gathers it.
    fn lean_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                lean_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "lean") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    lean_files(&lean, &mut files);
    files.sort();

    let (mut theorems, mut counterexamples, mut sorries, mut axioms) = (0usize, 0, 0usize, 0usize);
    for file in &files {
        let text =
            std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        for line in text.lines() {
            // `^(@[...] )*(theorem|lemma)` — a declaration starts at column zero, tolerating any
            // number of same-line `@[...]` attribute prefixes (SS1001); anything indented is
            // inside a proof, a comment, or a block, and that distinction is still the whole
            // finding -- see `is_theorem_or_lemma_declaration`'s doc comment above.
            if is_theorem_or_lemma_declaration(line) {
                theorems += 1;
            }
            // `^theorem [^ ]*counterexample` — the naming convention is the classifier.
            if let Some(name) = line.strip_prefix("theorem ") {
                if name
                    .split_whitespace()
                    .next()
                    .is_some_and(|n| n.contains("counterexample"))
                {
                    counterexamples += 1;
                }
            }
            if line.contains("sorry") {
                sorries += 1;
            }
            if line.starts_with("axiom") {
                axioms += 1;
            }
        }
    }
    println!(
        "LEAN_RECOUNT theorems={theorems} counterexamples={counterexamples} sorry={sorries} \
         axioms={axioms} files={}",
        files.len()
    );
    assert!(
        theorems > 0 && !files.is_empty(),
        "the recount found nothing under {}, which would make every assertion below vacuously \
         satisfiable by an empty tree",
        lean.display()
    );

    // `README.md`'s one-line status claim, generated from the count rather than compared to a
    // remembered string.
    let axiom_word = if axioms == 1 { "axiom" } else { "axioms" };
    let readme_sentence = format!(
        "**{theorems} theorems, {counterexamples} of them counterexamples, {axioms} {axiom_word}, \
         sorry {sorries}**"
    );
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README.md");
    assert!(
        readme.contains(&readme_sentence),
        "README.md does not carry the primary count. It must say:\n    {readme_sentence}\n\
         (`req/38` §207 ruling 2: the 92/2 it used to say counted two lines of English prose in a \
         block comment. If `lean/` grew, this message is the new sentence, not a complaint.)"
    );

    // `crates/gx-cli/src/limits.rs` item 8 — read through `gx limits --json`, so this holds the
    // **terminal**, and the first probe in this file holds `docs/LIMITS.md` to the same words.
    let output = Command::new(env!("CARGO_BIN_EXE_gx"))
        .args(["limits", "--json"])
        .output()
        .expect("gx limits --json runs");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("gx limits --json prints valid JSON");
    let item8 = json["limits"][7]["statement"]
        .as_str()
        .expect("item 8 carries a statement")
        .to_string();
    let item8 = collapse_whitespace(&item8);
    for phrase in [
        format!("proves {theorems} theorems"),
        format!("{counterexamples} of them named counterexamples"),
        format!("over {} files", files.len()),
        format!("{sorries} `sorry`"),
        format!("{axioms} `{axiom_word}`"),
    ] {
        assert!(
            item8.contains(&phrase),
            "`gx limits` item 8 does not say {phrase:?}. The terminal, docs/LIMITS.md and README.md \
             are one claim about one tree; this is the tree's own answer:\n    {item8}"
        );
    }
}
